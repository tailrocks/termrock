use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};
use ratatui_widgets::{block::Block, clear::Clear, paragraph::Paragraph};

use crate::{
    style::{Role, Theme},
    text::display_cols,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic status severities used by status-bar slots.
pub enum Severity {
    /// Informational content with no success or failure implication.
    Info,
    /// Successful completion or a healthy state.
    Success,
    /// A condition requiring attention but not an error.
    Warning,
    /// A failed operation or invalid state.
    Error,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Corners used to anchor a toast within its containing rectangle.
pub enum Anchor {
    /// Places content at the top left.
    TopLeft,
    /// Places content at the top right.
    TopRight,
    /// Places content at the bottom left.
    BottomLeft,
    /// Places content at the bottom right.
    BottomRight,
}

#[derive(Debug, Clone, Copy)]
/// Transient notification overlay with caller-owned lifetime and placement.
///
/// See the `toast/severity` lookbook story for semantic variants.
///
/// # Examples
///
/// ```
/// use ratatui_core::layout::Rect;
/// use termrock::{Theme, widgets::{Anchor, Severity, Toast}};
///
/// let theme = Theme::default();
/// let toast = Toast::new(&theme, "Saved", Severity::Success)
///     .anchor(Anchor::BottomRight)
///     .margins(1, 1);
/// assert!(toast.rect(Rect::new(0, 0, 40, 8)).is_some());
/// ```
pub struct Toast<'a> {
    message: &'a str,
    severity: Severity,
    anchor: Anchor,
    style: Option<Style>,
    horizontal_margin: u16,
    vertical_margin: u16,
    theme: &'a Theme,
}

impl<'a> Toast<'a> {
    #[must_use]
    /// Creates a toast with default top-right anchoring and margins.
    pub const fn new(theme: &'a Theme, message: &'a str, severity: Severity) -> Self {
        Self {
            message,
            severity,
            anchor: Anchor::TopRight,
            style: None,
            horizontal_margin: 2,
            vertical_margin: 1,
            theme,
        }
    }

    #[must_use]
    /// Sets the corner used to anchor this content.
    pub const fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    #[must_use]
    /// Sets horizontal and vertical margins in terminal cells.
    pub const fn margins(mut self, horizontal: u16, vertical: u16) -> Self {
        self.horizontal_margin = horizontal;
        self.vertical_margin = vertical;
        self
    }

    #[must_use]
    /// Overrides the theme-derived toast style.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    #[must_use]
    /// Returns the resolved outer dialog rectangle.
    pub fn rect(&self, area: Rect) -> Option<Rect> {
        if area.is_empty() || self.message.is_empty() {
            return None;
        }
        let width = u16::try_from(display_cols(self.message).saturating_add(4))
            .unwrap_or(u16::MAX)
            .min(area.width);
        let height = 3.min(area.height);
        let x = match self.anchor {
            Anchor::TopLeft | Anchor::BottomLeft => area
                .left()
                .saturating_add(self.horizontal_margin)
                .min(area.right().saturating_sub(width)),
            Anchor::TopRight | Anchor::BottomRight => area
                .right()
                .saturating_sub(self.horizontal_margin)
                .saturating_sub(width)
                .max(area.left()),
        };
        let y = match self.anchor {
            Anchor::TopLeft | Anchor::TopRight => area
                .top()
                .saturating_add(self.vertical_margin)
                .min(area.bottom().saturating_sub(height)),
            Anchor::BottomLeft | Anchor::BottomRight => area
                .bottom()
                .saturating_sub(self.vertical_margin)
                .saturating_sub(height)
                .max(area.top()),
        };
        Some(Rect::new(x, y, width, height))
    }
}

impl Widget for &Toast<'_> {
    fn render(self, outer: Rect, buffer: &mut Buffer) {
        let Some(area) = self.rect(outer) else {
            return;
        };
        Clear.render(area, buffer);
        let border_role = match self.severity {
            Severity::Info => Role::Info,
            Severity::Success => Role::Success,
            Severity::Warning => Role::Warning,
            Severity::Error => Role::Danger,
        };
        Paragraph::new(self.message)
            .style(self.style.unwrap_or(self.theme.style(Role::Text)))
            .block(Block::bordered().border_style(self.theme.style(border_role)))
            .render(area, buffer);
    }
}

impl Widget for Toast<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_and_margins_resolve_inside_the_outer_area() {
        let theme = Theme::default();
        let outer = Rect::new(10, 5, 30, 12);
        let top_right = Toast::new(&theme, "Saved", Severity::Success)
            .anchor(Anchor::TopRight)
            .margins(2, 1)
            .rect(outer)
            .expect("visible toast");
        let bottom_left = Toast::new(&theme, "Saved", Severity::Success)
            .anchor(Anchor::BottomLeft)
            .margins(2, 1)
            .rect(outer)
            .expect("visible toast");

        assert_eq!(top_right, Rect::new(29, 6, 9, 3));
        assert_eq!(bottom_left, Rect::new(12, 13, 9, 3));
    }
}
