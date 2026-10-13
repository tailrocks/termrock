use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Span, widgets::Widget};
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

impl<'a> Panel<'a> {
    #[must_use]
    pub const fn new(theme: &'a Theme) -> Self {
        Self {
            title: None,
            emphasis: PanelEmphasis::Normal,
            style: None,
            theme,
        }
    }

    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    #[must_use]
    pub const fn emphasis(mut self, emphasis: PanelEmphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    #[must_use]
    pub fn block(&self) -> Block<'a> {
        let role = if self.emphasis == PanelEmphasis::Focused {
            Role::BorderFocused
        } else {
            Role::Border
        };
        let mut block =
            Block::bordered().border_style(self.style.unwrap_or(self.theme.style(role)));
        if let Some(title) = self.title {
            block = block.title(Span::styled(
                format!(" {} ", title.trim()),
                self.theme.style(Role::Text),
            ));
        }
        block
    }

    #[must_use]
    pub fn inner(&self, area: Rect) -> Rect {
        self.block().inner(area)
    }
}

impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.block().render(area, buffer);
    }
}
