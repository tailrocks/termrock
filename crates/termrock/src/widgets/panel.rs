use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use ratatui_widgets::block::Block;

use crate::style::{Role, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEmphasis {
    Normal,
    Focused,
}

#[derive(Debug, Clone)]
pub struct Panel<'a> {
    pub title: Option<&'a str>,
    pub emphasis: PanelEmphasis,
    pub style: Option<Style>,
    pub theme: &'a Theme,
}

impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let role = if self.emphasis == PanelEmphasis::Focused {
            Role::BorderFocused
        } else {
            Role::Border
        };
        let mut block =
            Block::bordered().border_style(self.style.unwrap_or(self.theme.style(role)));
        if let Some(title) = self.title {
            block = block.title(title);
        }
        block.render(area, buffer);
    }
}
