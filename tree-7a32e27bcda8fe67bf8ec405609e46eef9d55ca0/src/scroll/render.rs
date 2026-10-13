// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Scrollbar painting and the fixed-prefix line primitive.

use ratatui_core::{
    buffer::Buffer, layout::Rect, style::Style, terminal::Frame, text::Line, widgets::Widget,
};

use crate::{
    scroll,
    style::{DIALOG_SCROLL_THUMB, DIALOG_SCROLL_TRACK},
    text::{display_cols, fixed_prefix_scroll_segments},
};

/// Dim track glyph shared by every scrollbar.
pub const SCROLLBAR_TRACK: &str = "·";
/// Heavy horizontal scrollbar thumb glyph.
pub const SCROLLBAR_HORIZONTAL_THUMB: &str = "━";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Visual weight of the vertical scrollbar thumb.
pub enum ScrollbarStyle {
    /// Thin heavy-line thumb.
    #[default]
    Line,
    /// Solid block thumb.
    Block,
}

impl ScrollbarStyle {
    #[must_use]
    /// Return the vertical thumb glyph.
    pub const fn vertical_thumb(self) -> &'static str {
        match self {
            Self::Line => "┃",
            Self::Block => "█",
        }
    }
}

#[must_use]
/// Width inside a one-cell bordered block.
pub const fn viewport_width(area: Rect) -> usize {
    area.width.saturating_sub(2) as usize
}

#[must_use]
/// Height inside a one-cell bordered block.
pub const fn viewport_height(area: Rect) -> usize {
    area.height.saturating_sub(2) as usize
}

/// Clamp and store a `u16` scroll offset.
pub const fn clamp_scroll_offset(content_len: usize, viewport: usize, offset: &mut u16) -> u16 {
    scroll::clamp_offset_u16(content_len, viewport, offset)
}

/// Apply an unclamped signed delta.
pub const fn apply_scroll_delta_unclamped(value: &mut u16, delta: i16) {
    scroll::apply_delta_unclamped_u16(value, delta);
}

/// Apply a signed delta and clamp to content.
pub fn apply_scroll_delta(value: &mut u16, delta: i16, viewport: usize, content_len: usize) {
    scroll::apply_delta_u16(content_len, viewport, value, isize::from(delta));
}

/// Apply horizontal scrolling using a bordered terminal width.
pub fn apply_term_width_scroll_delta(
    value: &mut u16,
    delta: i16,
    term_width: u16,
    content_width: usize,
) {
    apply_scroll_delta(
        value,
        delta,
        usize::from(term_width.saturating_sub(2)),
        content_width,
    );
}

/// Map a pointer position on a scrollbar track to a content offset.
pub fn scrollbar_offset_for_track_position(
    content_length: usize,
    viewport: usize,
    track_len: usize,
    track_position: usize,
) -> u16 {
    scroll::offset_for_track_position_u16(content_length, viewport, track_len, track_position)
}

/// Paint a line whose prefix remains fixed while its suffix scrolls.
pub fn render_line_with_fixed_prefix_scroll(
    frame: &mut Frame<'_>,
    area: Rect,
    row: u16,
    line: Line<'static>,
    fixed_prefix_cols: usize,
    scroll_x: usize,
) {
    let mut fill_style = line.style;
    let mut styled_spans = Vec::new();
    let mut base_col = 0usize;
    for span in line.spans {
        let style = line.style.patch(span.style);
        if fill_style.bg.is_none() && style.bg.is_some() {
            fill_style = style;
        }
        let span_width = display_cols(&span.content);
        styled_spans.push((span.content.into_owned(), style, base_col));
        base_col += span_width;
    }
    let width = usize::from(area.width);
    for col in 0..width {
        frame
            .buffer_mut()
            .set_string(area.x + col as u16, area.y + row, " ", fill_style);
    }
    for (text, style, base_col) in styled_spans {
        for segment in
            fixed_prefix_scroll_segments(&text, base_col, fixed_prefix_cols, scroll_x, width)
        {
            frame.buffer_mut().set_string(
                area.x + segment.target_col as u16,
                area.y + row,
                &text[segment.start_byte..segment.end_byte],
                style,
            );
            for col in segment.target_col..segment.target_col + segment.display_cols {
                frame.buffer_mut()[(area.x + col as u16, area.y + row)].set_style(style);
            }
        }
    }
}

#[must_use]
/// Horizontal track inside the bottom border.
pub const fn horizontal_scrollbar_area(block_area: Rect) -> Rect {
    Rect::new(
        block_area.x + 1,
        block_area.y + block_area.height.saturating_sub(1),
        block_area.width.saturating_sub(2),
        1,
    )
}

#[must_use]
/// Vertical track inside the right border.
pub const fn vertical_scrollbar_area(block_area: Rect) -> Rect {
    Rect::new(
        block_area.x + block_area.width.saturating_sub(1),
        block_area.y + 1,
        1,
        block_area.height.saturating_sub(2),
    )
}

/// Render a horizontal scrollbar on a block border.
pub fn render_horizontal_scrollbar(
    frame: &mut Frame<'_>,
    block_area: Rect,
    content_width: usize,
    scroll_x: u16,
) {
    let viewport = viewport_width(block_area);
    if !scroll::is_scrollable(content_width, viewport) {
        return;
    }
    frame.render_widget(
        FixedScrollbar {
            content_length: content_width,
            viewport,
            offset: scroll_x,
            orientation: FixedScrollbarOrientation::Horizontal,
            style: ScrollbarStyle::Line,
        },
        horizontal_scrollbar_area(block_area),
    );
}

/// Render a line-style vertical scrollbar on a block border.
pub fn render_vertical_scrollbar(
    frame: &mut Frame<'_>,
    block_area: Rect,
    content_height: usize,
    scroll_y: u16,
) {
    render_vertical_scrollbar_with_style(
        frame,
        block_area,
        content_height,
        scroll_y,
        ScrollbarStyle::Line,
    );
}

/// Render a styled vertical scrollbar on a block border.
pub fn render_vertical_scrollbar_with_style(
    frame: &mut Frame<'_>,
    block_area: Rect,
    content_height: usize,
    scroll_y: u16,
    style: ScrollbarStyle,
) {
    let viewport = viewport_height(block_area);
    render_vertical_scrollbar_in_area_with_style(
        frame,
        vertical_scrollbar_area(block_area),
        content_height,
        viewport,
        scroll_y,
        style,
    );
}

/// Render a line-style vertical scrollbar in an explicit track.
pub fn render_vertical_scrollbar_in_area(
    frame: &mut Frame<'_>,
    area: Rect,
    content_height: usize,
    viewport: usize,
    scroll_y: u16,
) {
    render_vertical_scrollbar_in_area_with_style(
        frame,
        area,
        content_height,
        viewport,
        scroll_y,
        ScrollbarStyle::Line,
    );
}

/// Render a styled vertical scrollbar in an explicit track.
pub fn render_vertical_scrollbar_in_area_with_style(
    frame: &mut Frame<'_>,
    area: Rect,
    content_height: usize,
    viewport: usize,
    scroll_y: u16,
    style: ScrollbarStyle,
) {
    render_vertical_scrollbar_to_buffer(
        frame.buffer_mut(),
        area,
        content_height,
        viewport,
        scroll_y,
        style,
    );
}

/// Paint a vertical scrollbar directly into a widget buffer.
pub fn render_vertical_scrollbar_to_buffer(
    buffer: &mut Buffer,
    area: Rect,
    content_height: usize,
    viewport: usize,
    scroll_y: u16,
    style: ScrollbarStyle,
) {
    if !scroll::is_scrollable(content_height, viewport) || area.height == 0 {
        return;
    }
    FixedScrollbar {
        content_length: content_height,
        viewport,
        offset: scroll_y,
        orientation: FixedScrollbarOrientation::Vertical,
        style,
    }
    .render(area, buffer);
}

#[derive(Clone, Copy, Debug)]
enum FixedScrollbarOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
struct FixedScrollbar {
    content_length: usize,
    viewport: usize,
    offset: u16,
    orientation: FixedScrollbarOrientation,
    style: ScrollbarStyle,
}

impl Widget for FixedScrollbar {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let track_len = match self.orientation {
            FixedScrollbarOrientation::Horizontal => usize::from(area.width),
            FixedScrollbarOrientation::Vertical => usize::from(area.height),
        };
        let Some(thumb) = scroll::full_cell_thumb(
            self.content_length,
            self.viewport,
            u16::try_from(track_len).unwrap_or(u16::MAX),
            usize::from(self.offset),
        ) else {
            return;
        };
        let thumb_range = usize::from(thumb.start)..usize::from(thumb.start + thumb.len);
        let thumb_style = Style::new().fg(DIALOG_SCROLL_THUMB);
        let track_style = Style::new().fg(DIALOG_SCROLL_TRACK);
        for index in 0..track_len {
            let (x, y, thumb_symbol) = match self.orientation {
                FixedScrollbarOrientation::Horizontal => {
                    (area.x + index as u16, area.y, SCROLLBAR_HORIZONTAL_THUMB)
                }
                FixedScrollbarOrientation::Vertical => {
                    (area.x, area.y + index as u16, self.style.vertical_thumb())
                }
            };
            let in_thumb = thumb_range.contains(&index);
            buffer.set_string(
                x,
                y,
                if in_thumb {
                    thumb_symbol
                } else {
                    SCROLLBAR_TRACK
                },
                if in_thumb { thumb_style } else { track_style },
            );
        }
    }
}

#[cfg(test)]
mod tests;
