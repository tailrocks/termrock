use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::core::id::WidgetId;
use crate::theme::{CursorRow, CursorStyle, Kind};
use crate::ui::ctx::{RenderCtx, VisualState, fill};
use crate::ui::reveal::RevealState;
use crate::ui::text::truncate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowLook {
    #[default]
    Bar,

    Fill(CursorStyle),
}

impl From<CursorStyle> for RowLook {
    fn from(c: CursorStyle) -> Self {
        RowLook::Fill(c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    Current,

    Tick,

    Check,

    Glyph(&'static str),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RowUi;

impl RowUi {
    pub fn paint(
        ctx: &RenderCtx,
        buf: &mut Buffer,
        area: Rect,
        state: VisualState,
        look: RowLook,
        bg: Color,
    ) -> RowPaint {
        let p = Self::resolve(ctx, area, state, look, bg);
        p.draw(buf);
        p
    }

    pub fn resolve(
        ctx: &RenderCtx,
        area: Rect,
        state: VisualState,
        look: RowLook,
        bg: Color,
    ) -> RowPaint {
        let t = ctx.theme;
        let resolved = match look {
            RowLook::Bar => t.interactive(Kind::Row, state, t.plane_of(bg)),
            RowLook::Fill(c) => t.cursor_row(state, bg, c),
        };
        RowPaint {
            area,
            state,
            look: resolved,
            plane: bg,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RowPaint {
    pub area: Rect,

    pub state: VisualState,

    pub look: CursorRow,

    pub plane: Color,
}

impl RowPaint {
    pub fn draw(&self, buf: &mut Buffer) {
        fill(buf, self.area, self.look.style);
        for y in self.area.top()..self.area.bottom() {
            buf.set_string(self.area.x, y, self.look.gutter, self.look.gutter_style);
        }
    }

    pub fn style(&self) -> Style {
        self.look.style
    }

    pub fn bg(&self) -> Color {
        self.look.style.bg.unwrap_or(self.plane)
    }

    pub fn ink(&self, ctx: &RenderCtx, ink: Color) -> Color {
        ctx.theme.row_ink(&self.look, ink)
    }

    pub fn mark(
        &self,
        ctx: &RenderCtx,
        buf: &mut Buffer,
        x: u16,
        y: u16,
        kind: MarkKind,
        focused: bool,
    ) {
        let t = ctx.theme;
        let glyph = match kind {
            MarkKind::Current => t.glyphs.current,
            MarkKind::Tick => t.glyphs.tick,
            MarkKind::Check => t.glyphs.check_on,
            MarkKind::Glyph(g) => g,
        };
        let style = if self.state.disabled {
            self.style()
        } else {
            let s = VisualState {
                focused,
                ..self.state
            };
            self.style().fg(t.legible(t.chosen_in(s), self.bg()))
        };
        buf.set_string(x, y, glyph, style);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn label(
        &self,
        ctx: &mut RenderCtx,
        buf: &mut Buffer,
        reveal: &mut RevealState,
        id: WidgetId,
        cell: Rect,
        text: &str,
        style: Style,
    ) -> bool {
        buf.set_string(cell.x, cell.y, truncate(text, cell.width as usize), style);
        if self.state.disabled {
            return false;
        }
        reveal.cell(ctx, id, cell, text, self.state.focused)
    }
}
