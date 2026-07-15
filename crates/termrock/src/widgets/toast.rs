use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use ratatui_widgets::{block::Block, paragraph::Paragraph};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy)]
pub struct Toast<'a> {
    pub message: &'a str,
    pub severity: Severity,
    pub anchor: Anchor,
    pub style: Style,
}

impl Widget for &Toast<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Paragraph::new(self.message)
            .style(self.style)
            .block(Block::bordered())
            .render(area, buffer);
    }
}
