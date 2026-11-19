use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termrock_core::id::WidgetId;
use termrock_runtime::ctx::{RenderCtx, VisualState};
use termrock_runtime::measure::terminal_extent;
use termrock_theme::{ButtonKind, Kind, Plane, Theme};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockup {
    pub text: String,

    pub compact: bool,
}

impl Lockup {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            compact: false,
        }
    }

    pub fn compact(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            compact: true,
        }
    }

    fn label(&self) -> String {
        if self.compact {
            self.text.clone()
        } else {
            format!(" {} ", self.text)
        }
    }

    /// Natural width of the lockup, saturating at `u16::MAX`.
    pub fn width(&self) -> u16 {
        terminal_extent(termrock_text::cells::width(&self.label()))
    }

    pub fn style(t: &Theme) -> Style {
        Self::state_style(t, VisualState::default())
    }

    fn state_style(t: &Theme, s: VisualState) -> Style {
        t.interactive(Kind::Button(ButtonKind::Primary), s, Plane::Canvas)
            .style
    }

    pub fn render(&self, x: u16, y: u16, buf: &mut Buffer, t: &Theme) -> u16 {
        let label = self.label();
        buf.set_string(x, y, &label, Self::style(t));
        terminal_extent(termrock_text::cells::width(&label))
    }

    pub fn render_clickable(
        &self,
        x: u16,
        y: u16,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        id: WidgetId,
    ) -> u16 {
        let t = ctx.theme;
        let label = self.label();
        let mut s = ctx.state(id);
        s.focused = false;
        let st = Self::state_style(t, s);
        buf.set_string(x, y, &label, st);
        let w = terminal_extent(termrock_text::cells::width(&label));
        ctx.clickable(id, Rect::new(x, y, w, 1).intersection(*buf.area()));
        w
    }
}
