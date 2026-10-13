use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Line, widgets::Widget};
use ratatui_widgets::{block::Block, clear::Clear, paragraph::Paragraph};

use super::Action;

#[derive(Debug, Clone, Copy)]
pub struct Backdrop {
    pub symbol: char,
    pub style: Style,
}
impl Widget for &Backdrop {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buffer[(x, y)].set_char(self.symbol).set_style(self.style);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dialog<'a> {
    pub title: &'a str,
    pub body: Line<'a>,
    pub style: Style,
}
impl Widget for &Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Clear.render(area, buffer);
        Paragraph::new(self.body.clone())
            .block(Block::bordered().title(self.title))
            .style(self.style)
            .render(area, buffer);
    }
}

#[derive(Debug, Clone)]
pub struct DialogAction<'a, Id> {
    pub action: Action<'a, Id>,
    pub destructive: bool,
}
#[derive(Debug, Clone)]
pub struct ChoiceDialog<'a, Id> {
    pub dialog: Dialog<'a>,
    pub actions: &'a [DialogAction<'a, Id>],
}
#[derive(Debug, Clone)]
pub struct MessageDialog<'a, Id> {
    pub dialog: Dialog<'a>,
    pub details: &'a [super::DetailRow<'a, Id>],
}
