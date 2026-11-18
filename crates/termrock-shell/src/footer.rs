use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::theme::{BadgeKind, HintAlign, StatusKind, Tone};
use crate::ui::ctx::{RenderCtx, fill};
use crate::widgets::keyhint::{self, Hint, HintPriority, hint};

use super::Shell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FooterStatus {
    Toned {
        text: String,
        tone: Tone,
        parts: Vec<(String, Tone)>,
    },

    Kind {
        text: String,
        kind: Option<StatusKind>,
    },
}

impl FooterStatus {
    pub fn is_error(&self) -> bool {
        match self {
            Self::Toned { tone, .. } => *tone == Tone::Error,
            Self::Kind { kind, .. } => *kind == Some(StatusKind::Fail),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Footer {
    pub hints: Option<Vec<Hint>>,

    pub fallback: Vec<Hint>,

    pub editing: bool,

    pub tab_next: bool,

    pub status: Option<FooterStatus>,

    pub align: Option<HintAlign>,

    pub pin_dirty: Option<&'static str>,

    pub dirty: bool,

    pub hide_peek: bool,
}

impl Footer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn layer(mut self, hints: Option<Vec<Hint>>) -> Self {
        if self.hints.is_none() {
            self.hints = hints.filter(|h| !h.is_empty());
        }
        self
    }

    pub fn fallback(mut self, hints: Vec<Hint>) -> Self {
        self.fallback = hints;
        self
    }

    pub fn editing(mut self, on: bool) -> Self {
        self.editing = on;
        self
    }

    pub fn tab_next(mut self, on: bool) -> Self {
        self.tab_next = on;
        self
    }

    pub fn align(mut self, align: HintAlign) -> Self {
        self.align = Some(align);
        self
    }

    pub fn peek(mut self, on: bool) -> Self {
        self.hide_peek = !on;
        self
    }

    pub fn pin_dirty(mut self, key: Option<&'static str>) -> Self {
        self.pin_dirty = key;
        self
    }

    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn status(mut self, status: Option<(&str, Tone)>) -> Self {
        self.status = status.map(|(text, tone)| FooterStatus::Toned {
            text: text.to_owned(),
            tone,
            parts: vec![],
        });
        self
    }

    pub fn status_parts(
        mut self,
        status: Option<(&str, Tone)>,
        parts: Vec<(String, Tone)>,
    ) -> Self {
        self.status = status.map(|(text, tone)| FooterStatus::Toned {
            text: text.to_owned(),
            tone,
            parts,
        });
        self
    }

    pub fn status_kind(mut self, status: Option<(&str, Option<StatusKind>)>) -> Self {
        self.status = status.map(|(text, kind)| FooterStatus::Kind {
            text: text.to_owned(),
            kind,
        });
        self
    }

    pub fn compose(&self, reveal: Option<(&'static str, &'static str)>) -> Vec<Hint> {
        let mut hints = self.hints.clone().unwrap_or_else(|| self.fallback.clone());
        crate::ui::reveal::advertise(&mut hints, reveal, |h| h.key.as_ref(), hint);
        if self.tab_next && !self.editing && !hints.iter().any(|h| h.key == "Tab") {
            hints.push(hint("Tab", "Next"));
        }
        hints.retain(|h| deliverable(h.key.as_ref()));
        if self.dirty
            && let Some(key) = self.pin_dirty
        {
            for h in hints.iter_mut().filter(|h| h.key == key) {
                h.priority = HintPriority::Pinned;
            }
        }
        if self.status.as_ref().is_some_and(FooterStatus::is_error) {
            for h in &mut hints {
                h.priority = HintPriority::Normal;
            }
        }
        hints
    }

    pub fn render<P>(
        mut self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &RenderCtx,
        shell: &mut Shell<P>,
    ) {
        let t = ctx.theme;
        fill(buf, area, t.base());
        let hints = self.compose(ctx.reveal_hint);
        let badge = self.editing.then_some(("EDIT", BadgeKind::Edit));
        let style = crate::theme::HintBarStyle {
            align: self.align.unwrap_or(t.hintbar.align),
            ..t.hintbar
        };

        if !self.hide_peek
            && self.status.is_none()
            && let Some(text) = &ctx.peek
        {
            let need = keyhint::width(&hints, badge.map(|(b, _)| b), None);

            let room = area.width.saturating_sub(need + 3) as usize;
            if room >= PEEK_MIN {
                self.status = Some(FooterStatus::Toned {
                    text: crate::ui::text::truncate(text, room),
                    tone: Tone::Secondary,
                    parts: vec![],
                });
            }
        }
        let spots = match &self.status {
            Some(FooterStatus::Kind { text, kind }) => {
                keyhint::render_status_styled_at(
                    area,
                    buf,
                    t,
                    &hints,
                    badge,
                    Some((text.as_str(), *kind)),
                    &style,
                )
                .1
            }
            Some(FooterStatus::Toned { text, tone, parts }) => {
                keyhint::render_parts_at(
                    area,
                    buf,
                    t,
                    &hints,
                    badge,
                    Some((text.as_str(), *tone)),
                    parts,
                    &style,
                )
                .1
            }
            None => keyhint::render_parts_at(area, buf, t, &hints, badge, None, &[], &style).1,
        };

        if !ctx.inert {
            shell.footer_drawn(hints, spots);
        }
    }
}

const PEEK_MIN: usize = 8;

pub fn deliverable(key: &str) -> bool {
    crate::core::keyboard::label_deliverable(key)
}
