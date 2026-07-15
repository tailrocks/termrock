use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, borders::Borders, paragraph::Paragraph};

use crate::scroll::{DialogScroll, full_cell_thumb, is_scrollable, max_line_width};

#[derive(Debug, Clone, Copy)]
pub struct Viewport<'a> {
    pub lines: &'a [Line<'a>],
    pub title: Option<&'a str>,
    pub content_style: Style,
    pub border_style: Style,
    pub title_style: Style,
    pub scroll_track_style: Style,
    pub scroll_thumb_style: Style,
}

impl StatefulWidget for &Viewport<'_> {
    type State = DialogScroll;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let viewport_width = usize::from(area.width.saturating_sub(2));
        let viewport_height = usize::from(area.height.saturating_sub(2));
        let content_width = max_line_width(self.lines);
        state.clamp(
            self.lines.len(),
            viewport_height,
            content_width,
            viewport_width,
        );
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_style(self.border_style);
        if let Some(title) = self.title {
            block = block.title(Span::styled(
                format!(" {} ", title.trim()),
                self.title_style,
            ));
        }
        Paragraph::new(self.lines.to_vec())
            .block(block)
            .style(self.content_style)
            .scroll((state.scroll_y, state.scroll_x))
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
                self.scroll_track_style,
                self.scroll_thumb_style,
            );
        }
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
