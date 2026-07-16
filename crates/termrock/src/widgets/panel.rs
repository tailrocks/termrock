use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, text::Span, widgets::Widget};
use ratatui_widgets::block::Block;

use crate::style::{Role, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic chrome emphasis for focused and inactive panels.
pub enum PanelEmphasis {
    /// Normal panel chrome.
    Normal,
    /// Focused panel chrome.
    Focused,
}

#[derive(Debug, Clone)]
/// A themed bordered container for product-owned content.
pub struct Panel<'a> {
    title: Option<&'a str>,
    emphasis: PanelEmphasis,
    style: Option<Style>,
    theme: &'a Theme,
}

impl<'a> Panel<'a> {
    #[must_use]
    /// Creates an untitled panel with normal emphasis.
    pub const fn new(theme: &'a Theme) -> Self {
        Self {
            title: None,
            emphasis: PanelEmphasis::Normal,
            style: None,
            theme,
        }
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    #[must_use]
    /// Sets the semantic panel emphasis.
    pub const fn emphasis(mut self, emphasis: PanelEmphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    #[must_use]
    /// Overrides the theme-derived panel style.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    #[must_use]
    /// Builds the surrounding block from the current title, emphasis, and style.
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
                self.theme.style(Role::TextStrong),
            ));
        }
        block
    }

    #[must_use]
    /// Returns the content rectangle inside dialog chrome.
    pub fn inner(&self, area: Rect) -> Rect {
        self.block().inner(area)
    }
}

impl Widget for &Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.block().render(area, buffer);
    }
}

impl Widget for Panel<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}
