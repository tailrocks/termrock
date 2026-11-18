use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::theme::{BadgeKind, HintAlign, Theme, Tone};
use crate::widgets::keyhint::{self, Hint};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HintLayer {
    pub hints: Vec<Hint>,
    pub badge: Option<(&'static str, BadgeKind)>,
    pub status: Option<(String, Tone)>,

    pub status_parts: Vec<(String, Tone)>,

    pub align: Option<HintAlign>,
}

impl HintLayer {
    pub fn new(hints: Vec<Hint>) -> Self {
        Self {
            hints,
            badge: None,
            status: None,
            status_parts: vec![],
            align: None,
        }
    }

    pub fn align(mut self, align: HintAlign) -> Self {
        self.align = Some(align);
        self
    }
    pub fn badge(mut self, text: &'static str, kind: BadgeKind) -> Self {
        self.badge = Some((text, kind));
        self
    }
    pub fn status(mut self, s: impl Into<String>, tone: Tone) -> Self {
        self.status = Some((s.into(), tone));
        self.status_parts.clear();
        self
    }

    pub fn status_parts(mut self, parts: Vec<(String, Tone)>, tone: Tone) -> Self {
        let text: String = parts.iter().map(|(p, _)| p.as_str()).collect();
        self.status = Some((text, tone));
        self.status_parts = parts;
        self
    }
}

impl crate::ui::measure::Measure for HintLayer {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let status = self.status.as_ref().map(|(text, tone)| match tone {
            Tone::Error => format!("! {text}"),
            Tone::Warning => format!("{} {text}", ctx.theme.glyphs.warning),
            _ => text.clone(),
        });
        let w = keyhint::width(&self.hints, self.badge.map(|(b, _)| b), status.as_deref());
        ratatui::layout::Size::new(w, 1)
    }
}

pub struct HintBar;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HintKeys(Vec<Option<crate::core::event::Key>>);

impl HintKeys {
    pub fn register(
        ctx: &mut crate::ui::ctx::RenderCtx,
        buf: &mut Buffer,
        hints: &[keyhint::Hint],
        spots: Vec<(usize, Rect)>,
    ) -> Self {
        use crate::core::id::{ItemKey, Part, PartRef};
        use ratatui::style::{Modifier, Style};
        let mut keys = vec![None; hints.len()];
        for (i, cell) in spots {
            let Some(key) = hints
                .get(i)
                .and_then(|h| h.click_key.or_else(|| keyhint::key_of(h.key.as_ref())))
            else {
                continue;
            };
            keys[i] = Some(key);
            let part = PartRef::item(Part::KEY, ItemKey::index(i));
            let id = ctx.register_part(HintBar::ID, part, cell);
            if ctx.interaction.pressed(id) {
                buf.set_style(cell, Style::new().add_modifier(Modifier::REVERSED));
            } else if ctx.interaction.hovered(id) {
                buf.set_style(cell, Style::new().add_modifier(Modifier::UNDERLINED));
            }
        }
        HintKeys(keys)
    }

    pub fn clicked(
        &self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::event::Key> {
        if !ev.on(HintBar::ID) || !ev.is(crate::core::intent::Phase::Click) {
            return None;
        }
        let i = ev.index_of(crate::core::id::Part::KEY)?;
        self.0.get(i).copied().flatten()
    }
}

impl HintBar {
    pub const ID: crate::core::id::WidgetId = crate::core::id::WidgetId::of("hintbar");

    pub fn render_ctx(
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut crate::ui::ctx::RenderCtx,
        layer: &HintLayer,
    ) -> HintKeys {
        let spots = Self::render_at(area, buf, ctx.theme, layer);
        HintKeys::register(ctx, buf, &layer.hints, spots)
    }

    pub fn render_at(
        area: Rect,
        buf: &mut Buffer,
        t: &Theme,
        layer: &HintLayer,
    ) -> Vec<(usize, Rect)> {
        let style = crate::theme::HintBarStyle {
            align: layer.align.unwrap_or(t.hintbar.align),
            ..t.hintbar
        };
        let (_, spots) = keyhint::render_parts_at(
            area,
            buf,
            t,
            &layer.hints,
            layer.badge,
            layer.status.as_ref().map(|(s, tone)| (s.as_str(), *tone)),
            &layer.status_parts,
            &style,
        );
        spots
    }

    pub fn resolve(layers: &[Option<HintLayer>]) -> HintLayer {
        layers.iter().flatten().next().cloned().unwrap_or_default()
    }

    pub fn render(area: Rect, buf: &mut Buffer, t: &Theme, layer: &HintLayer) -> usize {
        let style = crate::theme::HintBarStyle {
            align: layer.align.unwrap_or(t.hintbar.align),
            ..t.hintbar
        };
        keyhint::render_parts(
            area,
            buf,
            t,
            &layer.hints,
            layer.badge,
            layer.status.as_ref().map(|(s, tone)| (s.as_str(), *tone)),
            &layer.status_parts,
            &style,
        )
    }
}

impl crate::ui::specimen::Specimen for HintLayer {
    fn specimen_id(&self) -> crate::core::id::WidgetId {
        crate::core::id::WidgetId::of("hintbar.specimen")
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(state, Rest | Overflow | Error | Editing | Narrow)
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut crate::ui::ctx::RenderCtx,
        bg: ratatui::style::Color,
    ) {
        use crate::ui::specimen::SpecimenState::*;
        let mut layer = self.clone();
        match state {
            Overflow => {
                layer.status = Some((
                    "Saved 3 rows to payments.settlements · 2 warnings".into(),
                    Tone::Secondary,
                ));
            }
            Error => layer.status = Some(("Connection lost".into(), Tone::Error)),
            Editing => layer.badge = Some(("EDIT", BadgeKind::Edit)),
            _ => {}
        }
        crate::ui::ctx::fill(buf, area, ratatui::style::Style::new().bg(bg));
        HintBar::render(area, buf, ctx.theme, &layer);
    }
}
