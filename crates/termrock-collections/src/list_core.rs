use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::Key;
use crate::core::keymap::NavKeys;
use crate::core::list::{Cursor, KeyStep, Moved, NavProfile, RowSource, TypeAheadPrecedence};
use crate::core::scroll::ScrollPane;
use crate::core::selection::{ClickPolicy, RowSelection};
use crate::core::typeahead::TypeAhead;
use crate::ui::content::{self, ContentState};
use crate::ui::ctx::RenderCtx;
use crate::ui::reveal::RevealState;
use crate::widgets::empty::EmptyState;

#[derive(Debug, Clone)]
pub struct ListCore {
    pub cursor: usize,

    pub wrap: bool,

    pub skip_disabled: bool,

    pub nav: NavKeys,

    pub scroll: ScrollPane,

    pub selection: RowSelection,

    pub click: ClickPolicy,

    pub reveal: RevealState,

    pub type_ahead: bool,

    pub type_ahead_precedence: Option<TypeAheadPrecedence>,

    pub typeahead: TypeAhead,

    pub content: ContentState,

    pub empty: Option<EmptyState>,

    pub skip_when_empty: bool,

    pub inactive_selection: Option<crate::theme::InactiveSelection>,
}

impl ListCore {
    pub fn new(len: usize) -> Self {
        Self {
            cursor: 0,
            wrap: false,
            skip_disabled: true,
            nav: NavKeys::default(),
            scroll: ScrollPane::new(len),
            selection: RowSelection::new(),
            click: ClickPolicy::DATA,
            reveal: RevealState::default(),
            type_ahead: NavProfile::current().type_ahead(),
            type_ahead_precedence: None,
            typeahead: TypeAhead::default(),
            content: ContentState::Ready,
            empty: None,
            skip_when_empty: true,
            inactive_selection: None,
        }
    }

    pub fn cursor_model(&self) -> Cursor {
        Cursor::new(self.cursor)
            .wrap(self.wrap)
            .skip_disabled(self.skip_disabled)
    }

    pub fn key_step<S: RowSource + ?Sized>(
        &mut self,
        key: &Key,
        src: &S,
    ) -> Option<(KeyStep, Moved)> {
        let precedence = self
            .type_ahead_precedence
            .unwrap_or_else(TypeAheadPrecedence::current);
        let step = KeyStep::ordered(key, &self.nav, self.type_ahead, precedence)?;
        let moved = match step {
            KeyStep::Type(c) => self.cursor_model().type_ahead(c, &mut self.typeahead, src),
            KeyStep::Nav(nav) => self
                .cursor_model()
                .apply(nav, src, self.scroll.viewport_len),
        };
        Some((step, moved))
    }

    pub fn has_rows(&self, len: usize) -> bool {
        self.content.is_ready() && len > 0
    }

    pub fn is_focus_stop(&self, len: usize, focused: bool) -> bool {
        focused || !self.skip_when_empty || self.has_rows(len)
    }

    pub fn draw_content(
        &self,
        len: usize,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        default_empty: Option<&str>,
    ) -> bool {
        match &self.content {
            ContentState::Loading(label) => {
                content::draw_loading(area, buf, ctx, label, bg);
                true
            }
            ContentState::Error(e) => {
                crate::widgets::empty::render_ctx(area, buf, ctx, e, bg);
                true
            }
            ContentState::Ready if len == 0 => {
                if let Some(e) = &self.empty {
                    content::draw_empty(area, buf, ctx, e, bg);
                } else if let Some(text) = default_empty {
                    content::draw_line(area, buf, ctx.theme, text, bg);
                }
                true
            }
            ContentState::Ready => false,
        }
    }
}
