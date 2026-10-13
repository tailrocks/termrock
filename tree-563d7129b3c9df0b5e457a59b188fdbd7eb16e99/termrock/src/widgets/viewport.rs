use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, borders::Borders, paragraph::Paragraph};

use crate::{
    scroll::{DialogScroll, UNCACHED_REVISION, full_cell_thumb, is_scrollable, max_line_width},
    style::{Role, Theme},
};

use super::PanelEmphasis;

#[derive(Debug, Clone, Copy)]
/// A scrollable view over borrowed terminal lines.
pub struct Viewport<'a> {
    lines: &'a [Line<'a>],
    title: Option<&'a str>,
    emphasis: PanelEmphasis,
    theme: &'a Theme,
    content_style: Option<Style>,
    content_revision: u64,
}

impl<'a> Viewport<'a> {
    #[must_use]
    /// Creates a viewport over borrowed lines with zero scroll offset.
    pub const fn new(lines: &'a [Line<'a>], theme: &'a Theme) -> Self {
        Self {
            lines,
            title: None,
            emphasis: PanelEmphasis::Normal,
            theme,
            content_style: None,
            content_revision: UNCACHED_REVISION,
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
    pub const fn emphasis(mut self, emphasis: PanelEmphasis) -> Self {
        self.emphasis = emphasis;
        self
    }

    #[must_use]
    /// Sets the style applied to dialog content.
    pub const fn content_style(mut self, content_style: Style) -> Self {
        self.content_style = Some(content_style);
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
        let viewport_width = usize::from(area.width.saturating_sub(2));
        let viewport_height = usize::from(area.height.saturating_sub(2));
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
        let border_role = if self.emphasis == PanelEmphasis::Focused {
            Role::BorderFocused
        } else {
            Role::Border
        };
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.theme.style(border_role));
        if let Some(title) = self.title {
            block = block.title(Span::styled(
                format!(" {} ", title.trim()),
                self.theme.style(Role::TextStrong),
            ));
        }
        // Vertical slicing keeps frame cost proportional to the painted
        // window. Paragraph owns horizontal scrolling only after the slice.
        let start = usize::from(state.scroll_y).min(self.lines.len());
        let visible = self.lines[start..]
            .iter()
            .take(viewport_height)
            .cloned()
            .collect::<Vec<_>>();
        Paragraph::new(visible)
            .block(block)
            .style(
                self.content_style
                    .unwrap_or_else(|| self.theme.style(Role::Text)),
            )
            .scroll((0, state.scroll_x))
            .render(area, buffer);
        if is_scrollable(self.lines.len(), viewport_height) {
            render_vertical_scrollbar(
                buffer,
                Rect::new(
                    area.right().saturating_sub(1),
                    area.y.saturating_add(1),
                    1,
                    area.height.saturating_sub(2),
                ),
                self.lines.len(),
                viewport_height,
                state.scroll_y,
                self.theme.style(Role::ScrollTrack),
                self.theme.style(Role::ScrollThumb),
            );
        }
    }
}

impl StatefulWidget for Viewport<'_> {
    type State = DialogScroll;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

fn render_vertical_scrollbar(
    buffer: &mut Buffer,
    area: Rect,
    content_len: usize,
    viewport_len: usize,
    offset: u16,
    track_style: Style,
    thumb_style: Style,
) {
    let Some(thumb) = full_cell_thumb(content_len, viewport_len, area.height, usize::from(offset))
    else {
        return;
    };
    let thumb_end = thumb.start.saturating_add(thumb.len);
    for row in 0..area.height {
        let in_thumb = (thumb.start..thumb_end).contains(&row);
        buffer.set_string(
            area.x,
            area.y.saturating_add(row),
            if in_thumb { "┃" } else { "·" },
            if in_thumb { thumb_style } else { track_style },
        );
    }
}
