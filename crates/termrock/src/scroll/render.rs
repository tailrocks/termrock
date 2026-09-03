// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Scrollbar painting.
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{scroll, style::DesignSystem};

/// Fades the rows a scrolled region is cut at, so the cut reads as "more".
///
/// A scrollbar answers *where* you are; it does not answer whether the row
/// under your eye is the end of the content or the end of the pane. The edge
/// fade answers that in the one channel a terminal has to spare — colour —
/// because position is cell-quantised and cannot express a partial row.
///
/// `above` and `below` say whether content continues past that edge. Reduced
/// motion is not consulted: this is a static gradient, not a transition, and a
/// tier that forbids animation still wants to know the list continues.
/// Unicode-profile dim track glyph shared by every scrollbar.
///
/// junie: a one-column `│` track, `┃` thumb, painted only on overflow.
pub const SCROLLBAR_TRACK: &str = "│";
/// Unicode-profile heavy horizontal scrollbar thumb glyph.
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

/// Content and viewport dimensions used to size a scrollbar thumb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollbarGeometry {
    content_length: usize,
    viewport_length: usize,
    offset: u16,
}

impl ScrollbarGeometry {
    /// Creates explicit scrollbar geometry.
    #[must_use]
    pub const fn new(content_length: usize, viewport_length: usize, offset: u16) -> Self {
        Self {
            content_length,
            viewport_length,
            offset,
        }
    }
}

/// Declarative scrollbar paint request for an explicit track rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollbarSpec {
    axis: scroll::ScrollAxis,
    geometry: ScrollbarGeometry,
    style: ScrollbarStyle,
    focused: bool,
    hovered: bool,
}

impl ScrollbarSpec {
    /// Creates a line-style scrollbar request.
    #[must_use]
    pub const fn new(axis: scroll::ScrollAxis, geometry: ScrollbarGeometry) -> Self {
        Self {
            axis,
            geometry,
            style: ScrollbarStyle::Line,
            focused: false,
            hovered: false,
        }
    }

    /// Sets the vertical thumb glyph style.
    #[must_use]
    pub const fn style(mut self, style: ScrollbarStyle) -> Self {
        self.style = style;
        self
    }

    /// Whether the scrolled surface owns the keyboard (the thumb brightens).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Whether the pointer is over the track (the thumb lifts one plane).
    #[must_use]
    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }
}

/// Paints the junie line-cell overflow scrollbar (`│` track, `┃` thumb).
///
/// Geometry is [`crate::scroll::overflow_thumb`], not [`crate::scroll::full_cell_thumb`].
/// Nothing is painted when content already fits.
pub fn paint_overflow_scrollbar(
    buffer: &mut Buffer,
    gutter: Rect,
    total: usize,
    viewport: usize,
    offset: u16,
    focused: bool,
    system: &DesignSystem,
) {
    if gutter.is_empty() {
        return;
    }
    let track = usize::from(gutter.height);
    let Some((start, len)) = scroll::overflow_thumb(total, viewport, track, usize::from(offset))
    else {
        return;
    };
    let thumb = ScrollbarStyle::Line.vertical_thumb();
    for index in 0..track {
        let on_thumb = index >= start && index < start + len;
        buffer.set_string(
            gutter.x,
            gutter.y + index as u16,
            if on_thumb { thumb } else { SCROLLBAR_TRACK },
            if on_thumb {
                system.scrollbar_thumb(focused, false)
            } else {
                system.scrollbar_track()
            },
        );
    }
}

/// Paints a themed full-cell scrollbar into an explicit track rectangle.
pub fn paint_scrollbar(
    buffer: &mut Buffer,
    area: Rect,
    spec: ScrollbarSpec,
    system: &DesignSystem,
) {
    Scrollbar { spec, system }.render(area, buffer);
}

#[derive(Debug, Clone, Copy)]
struct Scrollbar<'a> {
    spec: ScrollbarSpec,
    system: &'a DesignSystem,
}

impl Scrollbar<'_> {
    /// Paint (single public entry; the [`Widget`] impl delegates here).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        let track_len = match self.spec.axis {
            scroll::ScrollAxis::Horizontal => usize::from(area.width),
            scroll::ScrollAxis::Vertical => usize::from(area.height),
        };
        let Some(thumb) = scroll::full_cell_thumb(
            self.spec.geometry.content_length,
            self.spec.geometry.viewport_length,
            u16::try_from(track_len).unwrap_or(u16::MAX),
            usize::from(self.spec.geometry.offset),
        ) else {
            return;
        };
        let thumb_range = usize::from(thumb.start)..usize::from(thumb.start + thumb.len);
        let track_symbol = SCROLLBAR_TRACK;
        for index in 0..track_len {
            let (x, y, thumb_symbol) = match self.spec.axis {
                scroll::ScrollAxis::Horizontal => {
                    (area.x + index as u16, area.y, SCROLLBAR_HORIZONTAL_THUMB)
                }
                scroll::ScrollAxis::Vertical => (
                    area.x,
                    area.y + index as u16,
                    self.spec.style.vertical_thumb(),
                ),
            };
            let in_thumb = thumb_range.contains(&index);
            // M12: the thumb states the surface's focus and hover through the
            // one scrollbar resolver; the track stays the quiet rail.
            let style = if in_thumb {
                self.system
                    .scrollbar_thumb(self.spec.focused, self.spec.hovered)
            } else {
                self.system.scrollbar_track()
            };
            buffer.set_string(
                x,
                y,
                if in_thumb { thumb_symbol } else { track_symbol },
                style,
            );
        }
    }
}

impl Widget for Scrollbar<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

#[cfg(test)]
mod tests;
