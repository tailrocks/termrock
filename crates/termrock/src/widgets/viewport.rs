#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, borders::Borders, paragraph::Paragraph};

use crate::{
    scroll::{DialogScroll, UNCACHED_REVISION, max_line_width},
    style::{DesignSystem, Role, RolePalette},
};

use super::PanelChrome;

#[derive(Debug, Clone, Copy)]
/// A scrollable view over borrowed terminal lines.
pub struct Viewport<'a> {
    lines: &'a [Line<'a>],
    title: Option<&'a str>,
    emphasis: PanelChrome,
    system: &'a DesignSystem,
    content_style: Option<Style>,
    content_revision: u64,
    padded_content: bool,
}

impl<'a> Viewport<'a> {
    #[must_use]
    /// Creates a viewport over borrowed lines with zero scroll offset.
    pub const fn new(lines: &'a [Line<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            title: None,
            emphasis: PanelChrome::Normal,
            system,
            content_style: None,
            content_revision: UNCACHED_REVISION,
            padded_content: false,
        }
    }

    #[must_use]
    /// Sets the optional visible title.
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    #[must_use]
    /// Selects the border emphasis for the active interaction owner.
    pub const fn emphasis(mut self, emphasis: PanelChrome) -> Self {
        self.emphasis = emphasis;
        self
    }

    #[must_use]
    /// Sets the style applied to dialog content.
    pub const fn content_style(mut self, content_style: Style) -> Self {
        self.content_style = Some(content_style);
        self
    }

    /// Insets content horizontally by the density's `pad_x`.
    ///
    /// A viewport owns no rhythm rows, so the inset is horizontal only:
    /// content stays flush with the border on Y while its X column matches
    /// the body column a [`super::Panel`] would give the same frame. Hosts
    /// migrating framed bodies from `Panel` to `Viewport` opt in here to
    /// keep their content column stable.
    #[must_use]
    pub const fn padded_content(mut self) -> Self {
        self.padded_content = true;
        self
    }

    /// Enables measurement reuse for unchanged content.
    ///
    /// Bump `revision` whenever line contents change. Length changes invalidate
    /// the cache automatically. Omitting this builder measures every frame.
    #[must_use]
    pub const fn content_revision(mut self, revision: u64) -> Self {
        self.content_revision = revision;
        self
    }
}

impl StatefulWidget for &Viewport<'_> {
    type State = DialogScroll;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let pad_x = if self.padded_content {
            self.system.spacing.pad_x
        } else {
            0
        };
        // Border ring first; content rect sits inside it, inset by pad_x
        // when the host asked for Panel-aligned padding.
        let content = Rect::new(
            area.x.saturating_add(1).saturating_add(pad_x),
            area.y.saturating_add(1),
            area.width.saturating_sub(2).saturating_sub(pad_x),
            area.height.saturating_sub(2),
        );
        let viewport_width = usize::from(content.width);
        let viewport_height = usize::from(content.height);
        let (content_width, _) =
            state
                .measurement
                .get_or_measure(self.lines.len(), self.content_revision, || {
                    (max_line_width(self.lines), self.lines.len())
                });
        state.clamp(
            self.lines.len(),
            viewport_height,
            content_width,
            viewport_width,
        );
        let border_role = if self.emphasis == PanelChrome::Focused {
            Role::BorderFocused
        } else {
            Role::Border
        };
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.system.style(border_role));
        if let Some(title) = self.title {
            block = block.title(Span::styled(
                format!(" {} ", title.trim()),
                self.system.style(Role::TextStrong),
            ));
        }
        block.render(area, buffer);
        // Vertical slicing keeps frame cost proportional to the painted
        // window. Paragraph owns horizontal scrolling only after the slice.
        let start = usize::from(state.scroll_y).min(self.lines.len());
        let visible = self.lines[start..]
            .iter()
            .take(viewport_height)
            .cloned()
            .collect::<Vec<_>>();
        Paragraph::new(visible)
            .style(
                self.content_style
                    .unwrap_or_else(|| self.system.style(Role::Text)),
            )
            .scroll((0, state.scroll_x))
            .render(content, buffer);
        // The fade belongs to the content, never to the chrome: a dimmed
        // border reads as a disabled pane, not as "there is more below".
        crate::scroll::paint_scrolled_region(
            buffer,
            content,
            Rect::new(
                area.right().saturating_sub(1),
                area.y.saturating_add(1),
                1,
                area.height.saturating_sub(2),
            ),
            self.lines.len(),
            viewport_height,
            state.scroll_y,
            self.system,
        );
    }
}

impl StatefulWidget for Viewport<'_> {
    type State = DialogScroll;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scroll::DialogScroll;

    fn lines() -> [Line<'static>; 3] {
        [Line::from("alpha"), Line::from("beta"), Line::from("gamma")]
    }

    #[test]
    fn content_is_flush_with_the_border_by_default() {
        let lines = lines();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        let mut scroll = DialogScroll::default();
        (&Viewport::new(&lines, &system)).render(area, &mut buffer, &mut scroll);
        assert_eq!(
            buffer[(1, 1)].symbol(),
            "a",
            "content starts at the inner column"
        );
    }

    #[test]
    fn padded_content_insets_x_but_stays_flush_on_y() {
        let lines = lines();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        let mut scroll = DialogScroll::default();
        (&Viewport::new(&lines, &system).padded_content()).render(area, &mut buffer, &mut scroll);
        let pad_x = system.spacing.pad_x;
        assert_eq!(buffer[(1, 1)].symbol(), " ", "the pad column stays empty");
        assert_eq!(
            buffer[(1 + pad_x, 1)].symbol(),
            "a",
            "content aligns with the Panel body column"
        );
        assert_eq!(
            buffer[(1 + pad_x, 2)].symbol(),
            "b",
            "rows stay flush on Y — no rhythm row is inserted"
        );
    }
}
