// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared scrollbar state, metrics, and offset adapters.
//!
//! `tui-scrollbar` owns proportional metrics and pointer interaction math.
//! Consumers own rendering, so this module exposes small helpers that convert
//! those metrics into the full-cell thumbs and clamped offsets used by the
//! terminal surfaces and renderers.
//!
//! `TailScroll` + the `is_scrollable` / `max_line_width` / `max_offset`
//! helpers are reimplemented from the donor compatibility layer and owned here.
mod render;

pub use render::{
    SCROLLBAR_HORIZONTAL_THUMB, SCROLLBAR_TRACK, ScrollbarGeometry, ScrollbarSpec, ScrollbarStyle,
    paint_overflow_scrollbar, paint_scrollbar, viewport_height, viewport_width,
};

use ratatui_core::{layout::Rect, text::Line};
use tui_scrollbar::{SUBCELL, ScrollLengths, ScrollMetrics};

use crate::input::{KeyModifiers, MouseEventKind};

/// Revision value that disables measurement reuse.
///
/// Widgets use this default until a consumer opts into caching with a stable
/// content revision.
pub const UNCACHED_REVISION: u64 = u64::MAX;

/// Cached content dimensions keyed by content length and caller revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measured {
    len: usize,
    revision: u64,
    /// Widest measured content row in terminal columns.
    pub width: usize,
    /// Measured content height in terminal rows.
    pub height: usize,
    valid: bool,
}

impl Default for Measured {
    fn default() -> Self {
        Self {
            len: 0,
            revision: UNCACHED_REVISION,
            width: 0,
            height: 0,
            valid: false,
        }
    }
}

impl Measured {
    /// Creates an empty measurement cache.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            len: 0,
            revision: UNCACHED_REVISION,
            width: 0,
            height: 0,
            valid: false,
        }
    }

    pub(crate) const fn is_current(&self, len: usize, revision: u64) -> bool {
        revision != UNCACHED_REVISION && self.valid && self.len == len && self.revision == revision
    }

    pub(crate) fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Returns cached dimensions or computes and stores a cache miss.
    ///
    /// [`UNCACHED_REVISION`] always invokes `measure`. Other revisions reuse
    /// dimensions only while both the revision and content length match.
    pub fn get_or_measure(
        &mut self,
        len: usize,
        revision: u64,
        measure: impl FnOnce() -> (usize, usize),
    ) -> (usize, usize) {
        if revision != UNCACHED_REVISION
            && self.valid
            && self.len == len
            && self.revision == revision
        {
            return (self.width, self.height);
        }
        let (width, height) = measure();
        self.width = width;
        self.height = height;
        self.len = len;
        self.revision = revision;
        self.valid = revision != UNCACHED_REVISION;
        (width, height)
    }
}

/// Tail-relative scroll offset helper.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TailScroll {
    offset: usize,
}

impl TailScroll {
    #[must_use]
    /// Creates scroll state following the live tail.
    pub const fn new(offset: usize) -> Self {
        Self { offset }
    }

    #[must_use]
    /// Returns the signed distance from the live tail in rows.
    pub const fn offset(self) -> usize {
        self.offset
    }

    /// Moves the scroll position by a signed delta and clamps it to valid content.
    pub fn scroll_by(&mut self, filled: usize, delta: isize) -> usize {
        let current = self.offset.min(filled);
        self.offset = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.unsigned_abs()).min(filled)
        };
        self.offset
    }

    /// Clamps the current scroll state to the supplied content and viewport bounds.
    pub fn clamp(&mut self, filled: usize) -> usize {
        self.offset = self.offset.min(filled);
        self.offset
    }

    #[must_use]
    /// Converts tail-relative state to a zero-based viewport top row.
    pub fn to_top_offset(self, content_len: usize, viewport_len: usize) -> usize {
        let max = max_offset(content_len, viewport_len);
        max.saturating_sub(self.offset.min(max))
    }
}

#[must_use]
/// Returns the widest line in terminal display columns.
pub fn max_line_width(lines: &[Line<'_>]) -> usize {
    lines.iter().map(Line::width).max().unwrap_or(0)
}

#[must_use]
/// Returns whether content exceeds a non-empty viewport.
pub const fn is_scrollable(content_len: usize, viewport_len: usize) -> bool {
    viewport_len > 0 && content_len > viewport_len
}

/// Returns the final column of an area as its reserved vertical gutter.
#[must_use]
pub const fn gutter_column(area: Rect) -> Rect {
    Rect::new(area.right().saturating_sub(1), area.y, 1, area.height)
}

/// Full-cell overflow thumb matching junie `ScrollState::thumb`.
///
/// `len = (viewport * track) / content`. Returns `None` when content fits, so a
/// reserved gutter stays blank. Line widgets (framed panel, picker) must use
/// this geometry; [`full_cell_thumb`] is the subcell `tui-scrollbar` rounding
/// used by other surfaces.
#[must_use]
pub const fn overflow_thumb(
    content_len: usize,
    viewport_len: usize,
    track_len: usize,
    offset: usize,
) -> Option<(usize, usize)> {
    if !is_scrollable(content_len, viewport_len) || track_len == 0 {
        return None;
    }
    let len = match viewport_len
        .saturating_mul(track_len)
        .checked_div(content_len)
    {
        Some(value) if value > 0 => value,
        _ => 1,
    };
    let len = if len > track_len { track_len } else { len };
    let max_off = content_len - viewport_len;
    let travel = track_len - len;
    let start = if max_off == 0 {
        0
    } else {
        match offset
            .saturating_mul(travel)
            .saturating_add(max_off / 2)
            .checked_div(max_off)
        {
            Some(value) => value,
            None => 0,
        }
    };
    let max_start = track_len - len;
    let start = if start > max_start { max_start } else { start };
    Some((start, len))
}

#[must_use]
/// Returns the greatest valid viewport offset.
pub const fn max_offset(content_len: usize, viewport_len: usize) -> usize {
    if viewport_len == 0 || content_len <= viewport_len {
        0
    } else {
        content_len - viewport_len
    }
}

/// Columns scrolled per horizontal wheel notch in shared scroll regions.
pub const DEFAULT_HORIZONTAL_SCROLL_STEP: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The terminal axis affected by scrolling.
pub enum ScrollAxis {
    /// The vertical terminal axis.
    Vertical,
    /// The horizontal terminal axis.
    Horizontal,
}

/// Axes that can actually move for the current content/viewport pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollAxes {
    /// Whether vertical scrolling is available.
    pub vertical: bool,
    /// Whether horizontal scrolling is available.
    pub horizontal: bool,
}

impl ScrollAxes {
    #[must_use]
    /// Returns a value with both scroll axes disabled.
    pub const fn none() -> Self {
        Self {
            vertical: false,
            horizontal: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A signed scroll amount on one axis.
pub struct ScrollDelta {
    /// Axis affected by the delta.
    pub axis: ScrollAxis,
    /// Signed movement in terminal cells or rows.
    pub amount: i16,
}

/// Two-axis scroll state for dialog bodies and other bounded viewports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogScroll {
    /// Scroll x in terminal cells or rows.
    pub scroll_x: u16,
    /// Scroll y in terminal cells or rows.
    pub scroll_y: u16,
    /// Cached dimensions used by revision-aware viewport widgets.
    pub(crate) measurement: Measured,
}

impl DialogScroll {
    #[must_use]
    /// Creates zero-offset dialog scroll state.
    pub const fn new() -> Self {
        Self {
            scroll_x: 0,
            scroll_y: 0,
            measurement: Measured::new(),
        }
    }

    /// Applies navigation keys only on axes currently allowed to move.
    pub fn handle_key_for_axes(
        &mut self,
        key: crate::input::KeyEvent,
        content_height: usize,
        viewport_height: usize,
        content_width: usize,
        viewport_width: usize,
        axes: ScrollAxes,
    ) -> bool {
        use crate::input::KeyCode;
        match key.code {
            KeyCode::Up | KeyCode::Char('k' | 'K') if axes.vertical => {
                self.scroll_y = self.scroll_y.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j' | 'J') if axes.vertical => {
                self.scroll_y = self
                    .scroll_y
                    .saturating_add(1)
                    .min(max_offset_u16(content_height, viewport_height));
            }
            KeyCode::PageUp if axes.vertical => {
                self.scroll_y = self
                    .scroll_y
                    .saturating_sub(viewport_height.min(u16::MAX as usize) as u16);
            }
            KeyCode::PageDown if axes.vertical => {
                self.scroll_y = self
                    .scroll_y
                    .saturating_add(viewport_height.min(u16::MAX as usize) as u16)
                    .min(max_offset_u16(content_height, viewport_height));
            }
            KeyCode::Left | KeyCode::Char('h' | 'H') if axes.horizontal => {
                self.scroll_x = self.scroll_x.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l' | 'L') if axes.horizontal => {
                self.scroll_x = self
                    .scroll_x
                    .saturating_add(1)
                    .min(max_offset_u16(content_width, viewport_width));
            }
            _ => return false,
        }
        true
    }

    /// Applies a wheel gesture using vertical or Shift-horizontal semantics.
    pub fn handle_mouse(
        &mut self,
        kind: crate::input::MouseEventKind,
        modifiers: crate::input::KeyModifiers,
        axes: ScrollAxes,
    ) -> bool {
        let Some(delta) = mouse_scroll_delta(kind, modifiers, axes) else {
            return false;
        };
        match delta.axis {
            ScrollAxis::Vertical => {
                apply_delta_unclamped_u16(&mut self.scroll_y, i32::from(delta.amount))
            }
            ScrollAxis::Horizontal => {
                apply_delta_unclamped_u16(&mut self.scroll_x, i32::from(delta.amount))
            }
        }
        true
    }

    /// Clamps the current scroll state to the supplied content and viewport bounds.
    pub fn clamp(
        &mut self,
        content_height: usize,
        viewport_height: usize,
        content_width: usize,
        viewport_width: usize,
    ) {
        self.scroll_y = self
            .scroll_y
            .min(max_offset_u16(content_height, viewport_height));
        self.scroll_x = self
            .scroll_x
            .min(max_offset_u16(content_width, viewport_width));
    }
}

/// Full-cell thumb geometry for downstream renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullCellThumb {
    /// 0-based cell inside the track where the thumb starts.
    pub start: u16,
    /// Number of cells the thumb spans.
    pub len: u16,
}

/// Tail-relative scroll offset used by live surfaces.
///
/// Externally `0` means "live tail / newest content". Internally the helper
/// clamps through the same top-relative `tui-scrollbar` metrics used by normal
/// panels before converting back to the tail-relative representation.
fn metrics(
    content_len: usize,
    viewport_len: usize,
    offset: usize,
    track_cells: u16,
) -> ScrollMetrics {
    ScrollMetrics::new(
        ScrollLengths {
            content_len,
            viewport_len,
        },
        offset,
        track_cells,
    )
}

#[must_use]
fn offset_after_delta(
    content_len: usize,
    viewport_len: usize,
    offset: usize,
    delta: isize,
) -> usize {
    let current = offset.min(max_offset(content_len, viewport_len));
    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current
            .saturating_add(delta.unsigned_abs())
            .min(max_offset(content_len, viewport_len))
    }
}

/// Applies a signed delta to a bounded `u16` offset.
pub fn apply_delta_u16(
    content_len: usize,
    viewport_len: usize,
    offset: &mut u16,
    delta: isize,
) -> u16 {
    let next = offset_after_delta(content_len, viewport_len, usize::from(*offset), delta)
        .min(usize::from(u16::MAX)) as u16;
    *offset = next;
    next
}

/// Convert a terminal mouse wheel event into one visible-axis scroll delta.
///
/// Horizontal scroll is either native `ScrollLeft` / `ScrollRight`, or
/// `Shift` + vertical wheel. Some terminals encode touchpad horizontal swipes
/// as shifted vertical wheel events, so every surface should use this helper
/// instead of matching `MouseEventKind` locally.
#[must_use]
pub fn mouse_scroll_delta(
    kind: MouseEventKind,
    modifiers: KeyModifiers,
    axes: ScrollAxes,
) -> Option<ScrollDelta> {
    mouse_scroll_delta_with_step(kind, modifiers, axes, DEFAULT_HORIZONTAL_SCROLL_STEP)
}

/// Same as [`mouse_scroll_delta`] but with a caller-chosen horizontal step.
///
/// Surfaces whose horizontal scroll advances by a different column count than
/// [`DEFAULT_HORIZONTAL_SCROLL_STEP`] (e.g. the host console panels, which step
/// by one column) pass their own step here so they share the axis/modifier
/// classification without inheriting the default magnitude.
#[must_use]
pub fn mouse_scroll_delta_with_step(
    kind: MouseEventKind,
    modifiers: KeyModifiers,
    axes: ScrollAxes,
    horizontal_step: u16,
) -> Option<ScrollDelta> {
    let horizontal = i16::try_from(horizontal_step).unwrap_or(i16::MAX);
    let shift = modifiers.contains(KeyModifiers::SHIFT);
    match kind {
        MouseEventKind::ScrollUp if shift && axes.horizontal => Some(ScrollDelta {
            axis: ScrollAxis::Horizontal,
            amount: -horizontal,
        }),
        MouseEventKind::ScrollDown if shift && axes.horizontal => Some(ScrollDelta {
            axis: ScrollAxis::Horizontal,
            amount: horizontal,
        }),
        MouseEventKind::ScrollUp if axes.vertical => Some(ScrollDelta {
            axis: ScrollAxis::Vertical,
            amount: -1,
        }),
        MouseEventKind::ScrollDown if axes.vertical => Some(ScrollDelta {
            axis: ScrollAxis::Vertical,
            amount: 1,
        }),
        MouseEventKind::ScrollLeft if axes.horizontal => Some(ScrollDelta {
            axis: ScrollAxis::Horizontal,
            amount: -horizontal,
        }),
        MouseEventKind::ScrollRight if axes.horizontal => Some(ScrollDelta {
            axis: ScrollAxis::Horizontal,
            amount: horizontal,
        }),
        _ => None,
    }
}

/// Scroll a selectable list by wheel while keeping selection and viewport
/// coherent.
///
/// Plain cursor-follow renderers undo manual scroll when the selected row is
/// pinned at the old viewport edge. This helper moves the viewport first, then
/// clamps the selected row into the new visible window so the next render
/// cannot snap the scroll position back.
pub fn scroll_selectable_list(
    selected: &mut usize,
    offset: &mut u16,
    item_count: usize,
    viewport_len: usize,
    delta: isize,
) -> bool {
    if item_count == 0 {
        *offset = 0;
        *selected = 0;
        return false;
    }
    if viewport_len == 0 || !is_scrollable(item_count, viewport_len) {
        *offset = 0;
        *selected = (*selected).min(item_count.saturating_sub(1));
        return false;
    }

    let before = *offset;
    apply_delta_u16(item_count, viewport_len, offset, delta);
    let start = usize::from(*offset);
    let end = start
        .saturating_add(viewport_len)
        .saturating_sub(1)
        .min(item_count.saturating_sub(1));
    *selected = (*selected).clamp(start, end);
    before != *offset
}

#[must_use]
/// Returns the greatest representable viewport offset.
pub const fn max_offset_u16(content_len: usize, viewport_len: usize) -> u16 {
    let max = max_offset(content_len, viewport_len);
    if max > u16::MAX as usize {
        u16::MAX
    } else {
        max as u16
    }
}

#[must_use]
/// Clamps an offset to the current scroll span.
pub const fn effective_offset(content_len: usize, viewport_len: usize, offset: u16) -> u16 {
    let max = max_offset_u16(content_len, viewport_len);
    if offset > max { max } else { offset }
}

/// Clamps the supplied offset to the current scroll span.
pub const fn clamp_offset_u16(content_len: usize, viewport_len: usize, offset: &mut u16) -> u16 {
    let effective = effective_offset(content_len, viewport_len, *offset);
    *offset = effective;
    effective
}

/// No upper clamp: render paths that know viewport/content clamp later.
///
/// Accepts `i32` so surfaces scrolling by page (viewport-height deltas) or by
/// an unbounded host value share one offset stepper; magnitudes beyond the
/// `u16` offset space saturate.
pub fn apply_delta_unclamped_u16(offset: &mut u16, delta: i32) {
    let mag = u16::try_from(delta.unsigned_abs()).unwrap_or(u16::MAX);
    *offset = if delta.is_negative() {
        offset.saturating_sub(mag)
    } else {
        offset.saturating_add(mag)
    };
}

#[must_use]
/// Maps a scrollbar-track position to a content offset.
pub fn offset_for_track_position(
    content_len: usize,
    viewport_len: usize,
    track_cells: u16,
    track_position: usize,
) -> usize {
    if !is_scrollable(content_len, viewport_len) || track_cells <= 1 {
        return 0;
    }

    let metrics = metrics(content_len, viewport_len, 0, track_cells);
    let position = track_position
        .min(usize::from(track_cells).saturating_sub(1))
        .saturating_mul(SUBCELL)
        .saturating_add(SUBCELL / 2);
    let thumb_start = position.saturating_sub(metrics.thumb_len() / 2);
    metrics.offset_for_thumb_start(thumb_start)
}

#[must_use]
/// Maps a `u16` scrollbar-track position to a bounded offset.
pub fn offset_for_track_position_u16(
    content_len: usize,
    viewport_len: usize,
    track_cells: usize,
    track_position: usize,
) -> u16 {
    if !is_scrollable(content_len, viewport_len) || track_cells <= 1 {
        return 0;
    }

    offset_for_track_position(
        content_len,
        viewport_len,
        track_cells.min(usize::from(u16::MAX)) as u16,
        track_position,
    )
    .min(usize::from(u16::MAX)) as u16
}

#[must_use]
/// Returns the offset that keeps a selected row visible.
pub fn cursor_follow_offset(
    cursor: usize,
    content_len: usize,
    viewport_len: usize,
    stored_offset: usize,
) -> usize {
    if viewport_len == 0 {
        return 0;
    }

    let max = max_offset(content_len, viewport_len);
    let stored = stored_offset.min(max);
    let raw = if cursor < stored {
        cursor
    } else if is_scrollable(content_len, viewport_len)
        && cursor >= stored.saturating_add(viewport_len)
    {
        cursor.saturating_add(1).saturating_sub(viewport_len)
    } else {
        stored
    };
    raw.min(max)
}

#[must_use]
/// Resolves full-cell scrollbar thumb position and length.
pub fn full_cell_thumb(
    content_len: usize,
    viewport_len: usize,
    track_cells: u16,
    offset: usize,
) -> Option<FullCellThumb> {
    if !is_scrollable(content_len, viewport_len) || track_cells == 0 {
        return None;
    }
    let max = max_offset(content_len, viewport_len);
    let metrics = metrics(content_len, viewport_len, offset, track_cells);
    let len = metrics
        .thumb_len()
        .saturating_add(SUBCELL - 1)
        .saturating_div(SUBCELL)
        .max(1)
        .min(usize::from(track_cells).saturating_sub(1).max(1));
    let max_start = usize::from(track_cells).saturating_sub(len);
    let rounded_start = metrics
        .thumb_start()
        .saturating_add(SUBCELL / 2)
        .saturating_div(SUBCELL)
        .min(max_start);
    let clamped_offset = offset.min(max);
    let start = if clamped_offset == 0 {
        0
    } else if clamped_offset == max {
        max_start
    } else {
        rounded_start
    };
    (len > 0).then_some(FullCellThumb {
        start: start as u16,
        len: len as u16,
    })
}

/// Full-cell vertical thumb for tail-relative scrollback surfaces.
#[must_use]
pub fn tail_vertical_thumb(
    track_rows: u16,
    filled: usize,
    tail_offset: usize,
) -> Option<FullCellThumb> {
    if track_rows == 0 || filled == 0 {
        return None;
    }
    let content_len = filled.saturating_add(usize::from(track_rows));
    let viewport_len = usize::from(track_rows);
    let top_offset = TailScroll::new(tail_offset).to_top_offset(content_len, viewport_len);
    full_cell_thumb(content_len, viewport_len, track_rows, top_offset)
}

#[cfg(test)]
mod tests;
