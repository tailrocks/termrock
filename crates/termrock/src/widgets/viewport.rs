use ratatui_core::{
    buffer::{Buffer, CellWidth},
    layout::{Alignment, Position, Rect},
    style::Style,
    text::Line,
    widgets::StatefulWidget,
};
use std::collections::{BTreeMap, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    interaction::Outcome,
    scroll::{DialogScroll, UNCACHED_REVISION},
    style::{DesignSystem, Role},
};

use super::{PanelChrome, Surface, SurfaceFill, SurfaceRecipe};

/// Logical line and display-column position in a viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellPos {
    /// Zero-based logical line.
    pub line: usize,
    /// Zero-based terminal display column.
    pub col: usize,
}

/// Events emitted by viewport interaction handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ViewportEvent {
    /// The host should put the selected text on its clipboard.
    Copy(String),
    /// The text selection changed.
    SelectionChanged,
}

#[derive(Debug, Clone, Copy)]
struct ViewportCell {
    span: Option<usize>,
    start: usize,
    end: usize,
    width: usize,
    style: Style,
}

#[derive(Debug, Clone)]
struct ViewportLine {
    cells: Vec<ViewportCell>,
    width: usize,
    alignment: Option<Alignment>,
    content_hash: u64,
}

/// Persistent interaction and layout state for [`Viewport`].
///
/// `Viewport` is a borrowed render configuration and is commonly rebuilt on
/// every frame. Selection, drag state, and scroll therefore live here rather
/// than in the widget value.
#[derive(Debug, Clone, Default)]
pub struct ViewportState {
    /// Shared two-axis scroll offsets.
    pub scroll: DialogScroll,
    area: Rect,
    selection: Option<(CellPos, CellPos)>,
    drag_anchor: Option<CellPos>,
    cells: Vec<ViewportLine>,
    cached_start: usize,
    cached_len: usize,
    cached_revision: u64,
    cached_width: usize,
    cached_widths: BTreeMap<usize, usize>,
    cached_base_style: Style,
    cache_valid: bool,
    pending_appends: usize,
    preserve_selection_on_change: bool,
}

impl ViewportState {
    /// Creates empty viewport interaction state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scroll: DialogScroll::new(),
            area: Rect::ZERO,
            selection: None,
            drag_anchor: None,
            cells: Vec::new(),
            cached_start: 0,
            cached_len: 0,
            cached_revision: UNCACHED_REVISION,
            cached_width: 0,
            cached_widths: BTreeMap::new(),
            cached_base_style: Style::new(),
            cache_valid: false,
            pending_appends: 0,
            preserve_selection_on_change: false,
        }
    }

    /// Rebases logical line positions after a host removes a prefix.
    pub(crate) fn rebase_lines(&mut self, removed: usize) {
        if removed == 0 {
            return;
        }
        if let Some((start, end)) = self.selection {
            if start.line < removed || end.line < removed {
                self.selection = None;
            } else {
                self.selection = Some((
                    CellPos {
                        line: start.line - removed,
                        col: start.col,
                    },
                    CellPos {
                        line: end.line - removed,
                        col: end.col,
                    },
                ));
            }
        }
        if let Some(anchor) = self.drag_anchor {
            if anchor.line < removed {
                self.drag_anchor = None;
            } else {
                self.drag_anchor = Some(CellPos {
                    line: anchor.line - removed,
                    col: anchor.col,
                });
            }
        }

        if self.cache_valid {
            if removed >= self.cached_len {
                self.clear_cached_cells();
            } else {
                for index in 0..removed {
                    let width = self.cells[self.cached_start + index].width;
                    self.remove_cached_width(width);
                }
                self.cached_start = self.cached_start.saturating_add(removed);
                self.cached_len = self.cached_len.saturating_sub(removed);
                if self.cached_start >= 1_024 {
                    self.compact_cached_prefix();
                }
                self.cached_width = self
                    .cached_widths
                    .last_key_value()
                    .map_or(0, |(width, _)| *width);
            }
        }
    }

    /// Preserve selection coordinates when a host appends lines in place.
    pub(crate) fn note_content_append(&mut self) {
        self.pending_appends = self.pending_appends.saturating_add(1);
        self.preserve_selection_on_change = true;
    }

    pub(crate) fn same_interaction(&self, other: &Self) -> bool {
        self.selection == other.selection && self.drag_anchor == other.drag_anchor
    }

    fn build_line(line: &Line<'_>, base_style: Style) -> ViewportLine {
        let line_style = base_style.patch(line.style);
        let mut column = 0;
        let mut cells = Vec::new();
        let mut hasher = DefaultHasher::new();
        for (span_index, span) in line.spans.iter().enumerate() {
            let style = line_style.patch(span.style);
            for (start, grapheme) in span.content.as_ref().grapheme_indices(true) {
                for byte in grapheme.as_bytes() {
                    byte.hash(&mut hasher);
                }
                let end = start + grapheme.len();
                if grapheme == "\t" {
                    let tab_spaces = 4 - (column % 4);
                    for _ in 0..tab_spaces {
                        cells.push(ViewportCell {
                            span: None,
                            start: 0,
                            end: 0,
                            width: 1,
                            style,
                        });
                        column += 1;
                    }
                    continue;
                }
                if grapheme.contains(char::is_control) {
                    continue;
                }
                let width = usize::from(grapheme.cell_width());
                if width == 0 {
                    continue;
                }
                cells.push(ViewportCell {
                    span: Some(span_index),
                    start,
                    end,
                    width,
                    style,
                });
                column += width;
            }
        }
        ViewportLine {
            cells,
            width: column,
            alignment: line.alignment,
            content_hash: hasher.finish(),
        }
    }

    fn build_lines(lines: &[Line<'_>], base_style: Style) -> Vec<ViewportLine> {
        lines
            .iter()
            .map(|line| Self::build_line(line, base_style))
            .collect()
    }

    fn active_cells(&self) -> &[ViewportLine] {
        let end = self.cached_start.saturating_add(self.cached_len);
        self.cells.get(self.cached_start..end).unwrap_or_default()
    }

    fn clear_cached_cells(&mut self) {
        self.cells.clear();
        self.cached_start = 0;
        self.cached_len = 0;
        self.cached_width = 0;
        self.cached_widths.clear();
        self.cache_valid = false;
    }

    fn add_cached_width(&mut self, width: usize) {
        *self.cached_widths.entry(width).or_default() += 1;
    }

    fn remove_cached_width(&mut self, width: usize) {
        if let Some(count) = self.cached_widths.get_mut(&width) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.cached_widths.remove(&width);
            }
        }
    }

    fn compact_cached_prefix(&mut self) {
        if self.cached_start == 0 {
            return;
        }
        self.cells.drain(..self.cached_start);
        self.cached_start = 0;
    }

    fn content_matches(&self, lines: &[ViewportLine]) -> bool {
        self.cached_len == lines.len()
            && self
                .active_cells()
                .iter()
                .zip(lines)
                .all(|(old, new)| old.content_hash == new.content_hash)
    }

    fn finish_cache_metadata(&mut self, lines: &[Line<'_>], base_style: Style, revision: u64) {
        self.cached_len = lines.len();
        self.cached_revision = revision;
        self.cached_base_style = base_style;
        self.cache_valid = true;
        self.pending_appends = 0;
        self.preserve_selection_on_change = false;
        self.cached_width = self
            .cached_widths
            .last_key_value()
            .map_or(0, |(width, _)| *width);
    }

    fn validate_interaction(&mut self) {
        let line_count = self.cached_len;
        if line_count == 0 {
            self.selection = None;
            self.drag_anchor = None;
            return;
        }
        let invalid_selection = self.selection.is_some_and(|(start, end)| {
            start.line >= line_count
                || end.line >= line_count
                || start.col > self.line_width(start.line)
                || end.col > self.line_width(end.line)
        });
        if invalid_selection {
            self.selection = None;
        } else if let Some((start, end)) = self.selection.as_mut() {
            start.line = start.line.min(line_count - 1);
            end.line = end.line.min(line_count - 1);
        }
        let invalid_anchor = self.drag_anchor.is_some_and(|anchor| {
            anchor.line >= line_count || anchor.col > self.line_width(anchor.line)
        });
        if invalid_anchor {
            self.drag_anchor = None;
        } else if let Some(anchor) = self.drag_anchor.as_mut() {
            anchor.line = anchor.line.min(line_count - 1);
        }
    }

    fn try_append_cells(&mut self, lines: &[Line<'_>], base_style: Style, revision: u64) -> bool {
        if !self.cache_valid
            || revision == UNCACHED_REVISION
            || self.pending_appends == 0
            || self.cached_base_style != base_style
            || self.cached_len.saturating_add(self.pending_appends) != lines.len()
        {
            return false;
        }
        let appended = Self::build_lines(&lines[self.cached_len..], base_style);
        for line in &appended {
            self.add_cached_width(line.width);
        }
        self.cells.extend(appended);
        self.finish_cache_metadata(lines, base_style, revision);
        self.validate_interaction();
        true
    }

    fn ensure_cells(&mut self, lines: &[Line<'_>], base_style: Style, revision: u64) {
        if self.try_append_cells(lines, base_style, revision) {
            return;
        }
        let cache_matches = self.cache_valid
            && revision != UNCACHED_REVISION
            && self.cached_len == lines.len()
            && self.cached_revision == revision
            && self.cached_base_style == base_style;
        if cache_matches {
            return;
        }

        let rebuilt = Self::build_lines(lines, base_style);
        let uncached_content_changed = revision == UNCACHED_REVISION
            && self.cache_valid
            && (self.selection.is_some() || self.drag_anchor.is_some())
            && !self.content_matches(&rebuilt);
        let content_changed =
            revision != UNCACHED_REVISION && self.cache_valid && !self.content_matches(&rebuilt);
        if (uncached_content_changed || content_changed) && !self.preserve_selection_on_change {
            self.selection = None;
            self.drag_anchor = None;
        }

        self.cells = rebuilt;
        self.cached_start = 0;
        self.cached_widths.clear();
        for index in 0..self.cells.len() {
            let width = self.cells[index].width;
            self.add_cached_width(width);
        }
        self.finish_cache_metadata(lines, base_style, revision);
        self.validate_interaction();
    }

    fn normalized_selection(&self) -> Option<(CellPos, CellPos)> {
        let (start, end) = self.selection?;
        (start != end).then_some((start.min(end), start.max(end)))
    }

    fn line_width(&self, line: usize) -> usize {
        self.active_cells()
            .get(line)
            .map(|line| line.width)
            .unwrap_or(0)
    }

    fn max_line_width(&self) -> usize {
        self.cached_width
    }

    fn column_of(&self, line: usize, cell: usize) -> usize {
        self.active_cells()
            .get(line)
            .map(|line| line.cells.iter().take(cell).map(|cell| cell.width).sum())
            .unwrap_or(0)
    }

    fn cell_at(&self, line: usize, column: usize) -> usize {
        let Some(line) = self.active_cells().get(line) else {
            return 0;
        };
        let mut current: usize = 0;
        for (index, cell) in line.cells.iter().enumerate() {
            if current.saturating_add(cell.width) > column {
                return index;
            }
            current += cell.width;
        }
        line.cells.len()
    }

    fn line_alignment(&self, line: usize) -> Alignment {
        self.active_cells()
            .get(line)
            .and_then(|line| line.alignment)
            .unwrap_or(Alignment::Left)
    }

    fn line_scroll(&self, line: usize) -> usize {
        if self.line_alignment(line) == Alignment::Left
            || self.line_width(line) > usize::from(self.area.width)
        {
            usize::from(self.scroll.scroll_x)
        } else {
            0
        }
    }

    fn line_offset(&self, line: usize) -> usize {
        let Some(line) = self.active_cells().get(line) else {
            return 0;
        };
        let viewport_width = usize::from(self.area.width);
        if line.width > viewport_width {
            return 0;
        }
        match line.alignment.unwrap_or(Alignment::Left) {
            Alignment::Center => viewport_width.saturating_sub(line.width) / 2,
            Alignment::Right => viewport_width.saturating_sub(line.width),
            Alignment::Left => 0,
        }
    }

    fn raw_column_at(&self, position: Position, line: usize) -> usize {
        let visual_column = usize::from(position.x.saturating_sub(self.area.x));
        let offset = self.line_offset(line);
        let relative = visual_column.saturating_sub(offset);
        let horizontal = self.line_scroll(line);
        relative
            .saturating_add(horizontal)
            .min(self.line_width(line))
    }

    fn cell_index_at(&self, position: Position) -> Option<(usize, usize)> {
        if self.area.is_empty() || self.cached_len == 0 || !self.area.contains(position) {
            return None;
        }
        let row = usize::from(position.y.saturating_sub(self.area.y))
            .saturating_add(usize::from(self.scroll.scroll_y))
            .min(self.cached_len - 1);
        let visual_column = usize::from(position.x.saturating_sub(self.area.x));
        let offset = self.line_offset(row);
        if visual_column < offset {
            return None;
        }
        let relative = visual_column - offset;
        let horizontal = self.line_scroll(row);
        let visible_width = self
            .line_width(row)
            .saturating_sub(horizontal)
            .min(usize::from(self.area.width));
        if relative >= visible_width {
            return None;
        }
        let raw_column = self.raw_column_at(position, row);
        let cell = self.cell_at(row, raw_column);
        let cell_start = self.column_of(row, cell);
        let cell_end = self.column_of(row, cell.saturating_add(1));
        if cell_start < horizontal
            || cell_end.saturating_sub(horizontal) > usize::from(self.area.width)
        {
            return None;
        }
        (cell < self.active_cells()[row].cells.len()).then_some((row, cell))
    }

    fn pos_at(&self, position: Position) -> Option<CellPos> {
        if self.area.is_empty() || self.cached_len == 0 || !self.area.contains(position) {
            return None;
        }
        let row = usize::from(position.y.saturating_sub(self.area.y))
            .saturating_add(usize::from(self.scroll.scroll_y))
            .min(self.cached_len - 1);
        let visual_column = usize::from(position.x.saturating_sub(self.area.x));
        let offset = self.line_offset(row);
        if visual_column < offset {
            return None;
        }
        let relative = visual_column - offset;
        let horizontal = self.line_scroll(row);
        let line_width = self.line_width(row);
        let visible_width = line_width
            .saturating_sub(horizontal)
            .min(usize::from(self.area.width));
        let rejects_padding = horizontal > 0
            || (self.line_alignment(row) != Alignment::Left
                && line_width <= usize::from(self.area.width));
        if rejects_padding && relative >= visible_width {
            return None;
        }
        let raw_column = self.raw_column_at(position, row);
        let cell = self.cell_at(row, raw_column);
        let cell_start = self.column_of(row, cell);
        let cell_end = self.column_of(row, cell.saturating_add(1));
        if cell_start < horizontal
            || cell_end.saturating_sub(horizontal) > usize::from(self.area.width)
        {
            return None;
        }
        let column = if raw_column.saturating_sub(cell_start).saturating_mul(2)
            >= cell_end.saturating_sub(cell_start)
        {
            cell_end
        } else {
            cell_start
        };
        Some(CellPos {
            line: row,
            col: column,
        })
    }

    fn max_scroll_y(&self) -> u16 {
        u16::try_from(
            self.cached_len
                .saturating_sub(usize::from(self.area.height)),
        )
        .unwrap_or(u16::MAX)
    }

    fn scroll_axes(&self) -> crate::scroll::ScrollAxes {
        crate::scroll::ScrollAxes {
            vertical: crate::scroll::is_scrollable(self.cached_len, usize::from(self.area.height)),
            horizontal: crate::scroll::is_scrollable(
                self.max_line_width(),
                usize::from(self.area.width),
            ),
        }
    }

    fn set_scroll_y(&mut self, offset: u16) {
        let next = offset.min(self.max_scroll_y());
        self.scroll.scroll_y = next;
    }

    fn scroll_by(&mut self, delta: isize) {
        let current = usize::from(self.scroll.scroll_y);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current
                .saturating_add(delta.unsigned_abs())
                .min(usize::from(self.max_scroll_y()))
        };
        self.set_scroll_y(u16::try_from(next).unwrap_or(u16::MAX));
    }

    fn scrollbar_area(&self) -> Rect {
        Rect::new(self.area.right(), self.area.y, 1, self.area.height)
    }

    fn scroll_to_track(&mut self, position: Position) -> bool {
        let track = self.scrollbar_area();
        if !track.contains(position) || self.max_scroll_y() == 0 {
            return false;
        }
        let track_len = usize::from(track.height.saturating_sub(1)).max(1);
        let along = usize::from(position.y.saturating_sub(track.y)).min(track_len);
        let target = along
            .saturating_mul(usize::from(self.max_scroll_y()))
            .checked_div(track_len)
            .unwrap_or(0);
        let before = self.scroll.scroll_y;
        self.set_scroll_y(u16::try_from(target).unwrap_or(u16::MAX));
        before != self.scroll.scroll_y
    }
}

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
    ///
    /// Line alignment applies while a line fits the content width. Overlong
    /// lines use the shared horizontal scroll offset. Raw tabs expand to the
    /// next four-column terminal stop, matching TermRock's text renderers.
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

    /// Enables layout reuse for unchanged content.
    ///
    /// Bump `revision` monotonically whenever borrowed line text, styles,
    /// alignment, or layout-affecting content changes. Explicit revisions let
    /// the state reuse its cell layout and support incremental append updates.
    /// The default uncached revision rebuilds layout on every call so recreated
    /// or mutated borrowed storage cannot reuse stale cell metadata.
    #[must_use]
    pub const fn content_revision(mut self, revision: u64) -> Self {
        self.content_revision = revision;
        self
    }

    fn base_content_style(&self) -> Style {
        self.content_style
            .unwrap_or_else(|| self.system.style(Role::Text))
    }

    fn ensure_interaction_layout(&self, state: &mut ViewportState) {
        state.ensure_cells(self.lines, self.base_content_style(), self.content_revision);
    }

    fn cell_symbol(&self, line: usize, cell: &ViewportCell) -> &str {
        let Some(span_index) = cell.span else {
            return " ";
        };
        self.lines
            .get(line)
            .and_then(|line| line.spans.get(span_index))
            .and_then(|span| span.content.as_ref().get(cell.start..cell.end))
            .unwrap_or("")
    }

    /// Lays out the selectable body for pointer/key routing.
    ///
    /// Rendering calls this automatically. Hosts that route an event before
    /// the first paint can call it explicitly with the same body rectangle.
    pub fn set_area(&self, area: Rect, state: &mut ViewportState) {
        self.ensure_interaction_layout(state);
        state.area = area;
        if area.is_empty() {
            state.drag_anchor = None;
            return;
        }
        state.scroll.clamp(
            self.lines.len(),
            usize::from(area.height),
            state.max_line_width(),
            usize::from(area.width),
        );
    }

    /// Returns the logical position under a terminal cell.
    #[must_use]
    pub fn pos_at(&self, state: &ViewportState, position: Position) -> Option<CellPos> {
        state.pos_at(position)
    }

    /// Returns the normalized logical selection, if it spans at least one cell.
    #[must_use]
    pub fn selection(&self, state: &ViewportState) -> Option<(CellPos, CellPos)> {
        state.normalized_selection()
    }

    /// Returns whether a drag anchor is active.
    #[must_use]
    pub fn has_anchor(&self, state: &ViewportState) -> bool {
        state.drag_anchor.is_some()
    }

    /// Returns whether text is selected.
    #[must_use]
    pub fn has_selection(&self, state: &ViewportState) -> bool {
        self.selection(state).is_some()
    }

    /// Clears text selection. The drag anchor is retained like the reference.
    pub fn clear_selection(&self, state: &mut ViewportState) -> Outcome<()> {
        if state.selection.take().is_some() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Returns the selected text, preserving line breaks and terminal spaces.
    #[must_use]
    pub fn selected_text(&self, state: &ViewportState) -> Option<String> {
        let (start, end) = state.normalized_selection()?;
        let mut text = String::new();
        for line in start.line..=end.line {
            let line_layout = state.active_cells().get(line)?;
            let cells = &line_layout.cells;
            let from = if line == start.line {
                state.cell_at(line, start.col)
            } else {
                0
            };
            let to = if line == end.line {
                state.cell_at(line, end.col)
            } else {
                cells.len()
            };
            for cell in &cells[from.min(cells.len())..to.min(cells.len())] {
                text.push_str(self.cell_symbol(line, cell));
            }
            if line != end.line {
                text.push('\n');
            }
        }
        Some(text)
    }

    /// Returns a fresh copy payload for the current selection.
    #[must_use]
    pub fn copy_selection(&self, state: &ViewportState) -> Option<String> {
        self.selected_text(state)
    }

    /// Mouse down: anchor a selection drag.
    pub fn on_click(&self, state: &mut ViewportState, position: Position) -> Outcome<()> {
        self.ensure_interaction_layout(state);
        if !state.area.contains(position) {
            return Outcome::Ignored;
        }
        let had_selection = state.selection.is_some();
        state.selection = None;
        state.drag_anchor = state.pos_at(position);
        if state.drag_anchor.is_some() || had_selection {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Drag from the current anchor, auto-scrolling at vertical edges.
    pub fn on_drag(&self, state: &mut ViewportState, position: Position) -> Outcome<()> {
        self.ensure_interaction_layout(state);
        let Some(anchor) = state.drag_anchor else {
            return Outcome::Ignored;
        };
        if state.area.is_empty() {
            state.drag_anchor = None;
            return Outcome::Changed;
        }
        let before_scroll_y = state.scroll.scroll_y;
        let before_selection = state.selection;
        if position.y < state.area.y {
            state.scroll_by(-1);
        } else if position.y >= state.area.bottom() {
            state.scroll_by(1);
        }
        let clamped = Position::new(
            position
                .x
                .clamp(state.area.x, state.area.right().saturating_sub(1)),
            position
                .y
                .clamp(state.area.y, state.area.bottom().saturating_sub(1)),
        );
        let Some(head) = state.pos_at(clamped) else {
            return if state.scroll.scroll_y != before_scroll_y {
                Outcome::Changed
            } else {
                Outcome::Ignored
            };
        };
        state.selection = Some((anchor, head));
        if state.scroll.scroll_y != before_scroll_y || state.selection != before_selection {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Double-click: select the word under the pointer.
    pub fn select_word_at(&self, state: &mut ViewportState, position: Position) -> Outcome<()> {
        self.ensure_interaction_layout(state);
        let Some((line, index)) = state.cell_index_at(position) else {
            return Outcome::Ignored;
        };
        let Some(cells) = state.active_cells().get(line).map(|line| &line.cells) else {
            return Outcome::Ignored;
        };
        if cells.is_empty() {
            return Outcome::Ignored;
        }
        let is_word = |cell: &ViewportCell| {
            self.cell_symbol(line, cell)
                .chars()
                .next()
                .is_some_and(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '-' | '/' | '.')
                })
        };
        if !is_word(&cells[index]) {
            return self.clear_selection(state);
        }
        let mut start = index;
        while start > 0 && is_word(&cells[start - 1]) {
            start -= 1;
        }
        let mut end = index + 1;
        while end < cells.len() && is_word(&cells[end]) {
            end += 1;
        }
        state.selection = Some((
            CellPos {
                line,
                col: state.column_of(line, start),
            },
            CellPos {
                line,
                col: state.column_of(line, end),
            },
        ));
        state.drag_anchor = None;
        Outcome::Changed
    }

    /// Routes pointer scrolling and left-button selection gestures.
    pub fn on_mouse(
        &self,
        state: &mut ViewportState,
        event: crate::input::MouseEvent,
    ) -> Outcome<()> {
        match event.kind {
            crate::input::MouseEventKind::ScrollUp
            | crate::input::MouseEventKind::ScrollDown
            | crate::input::MouseEventKind::ScrollLeft
            | crate::input::MouseEventKind::ScrollRight => {
                self.ensure_interaction_layout(state);
                if !state.area.contains(event.position)
                    && !state.scrollbar_area().contains(event.position)
                {
                    return Outcome::Ignored;
                }
                let axes = state.scroll_axes();
                let before = state.scroll.clone();
                let handled = state.scroll.handle_mouse(event.kind, event.modifiers, axes);
                state.scroll.clamp(
                    self.lines.len(),
                    usize::from(state.area.height),
                    state.max_line_width(),
                    usize::from(state.area.width),
                );
                if handled && state.scroll != before {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            crate::input::MouseEventKind::Down(crate::input::MouseButton::Left) => {
                if state.scroll_to_track(event.position) {
                    return Outcome::Changed;
                }
                self.on_click(state, event.position)
            }
            crate::input::MouseEventKind::Drag(crate::input::MouseButton::Left) => {
                if state.scrollbar_area().contains(event.position) {
                    return if state.scroll_to_track(event.position) {
                        Outcome::Changed
                    } else {
                        Outcome::Ignored
                    };
                }
                self.on_drag(state, event.position)
            }
            crate::input::MouseEventKind::Up(crate::input::MouseButton::Left) => {
                if state.drag_anchor.take().is_some() {
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            _ => Outcome::Ignored,
        }
    }

    /// Handles viewport navigation, copy (`y`), and selection clearing (`Esc`).
    pub fn on_key(
        &self,
        state: &mut ViewportState,
        key: crate::input::KeyEvent,
    ) -> (Outcome<()>, Option<ViewportEvent>) {
        use crate::input::KeyCode;

        if key.is_release() {
            return (Outcome::Ignored, None);
        }
        self.ensure_interaction_layout(state);
        if key.is_press() && matches!(key.code, KeyCode::Char('y')) && key.modifiers.is_empty() {
            return match self.selected_text(state) {
                Some(text) => (Outcome::Changed, Some(ViewportEvent::Copy(text))),
                None => (Outcome::Ignored, None),
            };
        }
        if key.code == KeyCode::Esc {
            return match self.clear_selection(state) {
                Outcome::Changed => (Outcome::Changed, Some(ViewportEvent::SelectionChanged)),
                Outcome::Ignored => (Outcome::Ignored, None),
                _ => unreachable!("clear_selection only returns Changed or Ignored"),
            };
        }

        let axes = state.scroll_axes();
        let before = state.scroll.clone();
        let handled = state.scroll.handle_key_for_axes(
            key,
            self.lines.len(),
            usize::from(state.area.height),
            state.max_line_width(),
            usize::from(state.area.width),
            axes,
        );
        if handled && state.scroll != before {
            (Outcome::Changed, None)
        } else {
            (Outcome::Ignored, None)
        }
    }

    /// Renders the viewport without consuming the reusable interaction state.
    pub fn render(
        &self,
        area: Rect,
        buffer: &mut ratatui_core::buffer::Buffer,
        state: &mut ViewportState,
    ) {
        <&Self as StatefulWidget>::render(self, area, buffer, state);
    }
}

impl StatefulWidget for &Viewport<'_> {
    type State = ViewportState;

    fn render(self, requested_area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let area = requested_area.intersection(buffer.area);
        if area.is_empty() {
            state.area = Rect::ZERO;
            state.drag_anchor = None;
            return;
        }
        let pad_x = if self.padded_content {
            self.system.spacing.card_inset
        } else {
            0
        };
        // Surface owns the structural ring; the viewport owns its asymmetric
        // content inset so the scrollbar keeps the trailing border column.
        let surface_recipe = match self.emphasis {
            PanelChrome::Focused => SurfaceRecipe::Focused,
            PanelChrome::Danger => SurfaceRecipe::Destructive,
            PanelChrome::Normal => SurfaceRecipe::Interactive,
        };
        let surface_content = Surface::new(self.system)
            .recipe(surface_recipe)
            .bordered(true)
            .fill(SurfaceFill::Transparent)
            .padding(0, 0)
            .paint(area, buffer);
        let content = Rect::new(
            surface_content.x.saturating_add(pad_x),
            surface_content.y,
            surface_content.width.saturating_sub(pad_x),
            surface_content.height,
        );
        let viewport_height = usize::from(content.height);
        self.set_area(content, state);
        buffer.set_style(content, self.base_content_style());
        if let Some(title) = self.title {
            let budget = usize::from(area.width.saturating_sub(2));
            let clipped = crate::text::truncate_cols(
                title.trim(),
                budget.saturating_sub(2),
                self.system.glyphs.ellipsis(),
            );
            let label = format!(" {clipped} ");
            buffer.set_stringn(
                area.x.saturating_add(1),
                area.y,
                label,
                budget,
                self.system.style(Role::TextStrong),
            );
        }
        // Paint only visible logical lines and cells. This keeps the hot path
        // proportional to the viewport while allowing selection backgrounds.
        let selection = state.normalized_selection();
        let start = usize::from(state.scroll.scroll_y).min(state.cached_len);
        for (row, line) in state
            .active_cells()
            .iter()
            .skip(start)
            .take(viewport_height)
            .enumerate()
        {
            let line_index = start + row;
            let mut column: usize = 0;
            let horizontal = state.line_scroll(line_index);
            let offset = state.line_offset(line_index);
            for cell in &line.cells {
                let cell_end = column.saturating_add(cell.width);
                if cell_end <= horizontal {
                    column = cell_end;
                    continue;
                }
                if column < horizontal {
                    column = cell_end;
                    continue;
                }
                let visible_column = offset.saturating_add(column.saturating_sub(horizontal));
                let x = content
                    .x
                    .saturating_add(u16::try_from(visible_column).unwrap_or(u16::MAX));
                if x >= content.right() {
                    break;
                }
                let selected = selection.is_some_and(|(a, b)| {
                    let position = CellPos {
                        line: line_index,
                        col: column,
                    };
                    position >= a && position < b
                });
                let style = if selected {
                    self.system
                        .style(Role::Selection)
                        .add_modifier(cell.style.add_modifier)
                } else {
                    cell.style
                };
                let symbol = self.cell_symbol(line_index, cell);
                buffer.set_stringn(
                    x,
                    content
                        .y
                        .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
                    symbol,
                    usize::from(content.right().saturating_sub(x)),
                    style,
                );
                column = cell_end;
            }
            if let Some((a, b)) = selection
                && line_index >= a.line
                && line_index < b.line
                && horizontal < state.line_width(line_index)
            {
                let tail =
                    offset.saturating_add(state.line_width(line_index).saturating_sub(horizontal));
                let tail_x = content
                    .x
                    .saturating_add(u16::try_from(tail).unwrap_or(u16::MAX));
                if tail_x < content.right() {
                    buffer.set_style(
                        Rect::new(
                            tail_x,
                            content
                                .y
                                .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
                            1,
                            1,
                        ),
                        self.system.style(Role::Selection),
                    );
                }
            }
        }
        // The scrollbar belongs to the reserved gutter, never to content.
        crate::scroll::paint_overflow_scrollbar(
            buffer,
            crate::scroll::gutter_column(Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(2),
            )),
            self.lines.len(),
            viewport_height,
            state.scroll.scroll_y,
            false,
            self.system,
        );
    }
}

impl StatefulWidget for Viewport<'_> {
    type State = ViewportState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> [Line<'static>; 3] {
        [Line::from("alpha"), Line::from("beta"), Line::from("gamma")]
    }

    #[test]
    fn content_is_flush_with_the_border_by_default() {
        let lines = lines();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = Buffer::empty(area);
        let mut scroll = ViewportState::default();
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
        let mut scroll = ViewportState::default();
        (&Viewport::new(&lines, &system).padded_content()).render(area, &mut buffer, &mut scroll);
        let pad_x = crate::style::SpacingScale::junie().card_inset;
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

    #[test]
    fn appended_content_extends_cached_layout_without_rebuilding_prefix() {
        let initial = vec![Line::from("alpha"), Line::from("beta")];
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut state = ViewportState::default();
        {
            let viewport = Viewport::new(&initial, &system).content_revision(1);
            viewport.render(area, &mut Buffer::empty(area), &mut state);
        }
        let prefix_cells = state.active_cells()[0].cells.as_ptr();
        state.note_content_append();

        let appended = vec![initial[0].clone(), initial[1].clone(), Line::from("gamma")];
        let viewport = Viewport::new(&appended, &system).content_revision(2);
        viewport.render(area, &mut Buffer::empty(area), &mut state);

        assert_eq!(state.active_cells().len(), 3);
        assert_eq!(state.active_cells()[0].cells.as_ptr(), prefix_cells);
    }

    #[test]
    fn evicted_prefix_is_rebased_without_rebuilding_surviving_layout() {
        let initial = vec![Line::from("alpha"), Line::from("beta"), Line::from("gamma")];
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut state = ViewportState::default();
        {
            let viewport = Viewport::new(&initial, &system).content_revision(1);
            viewport.render(area, &mut Buffer::empty(area), &mut state);
        }
        let surviving_cells = state.active_cells()[1].cells.as_ptr();
        state.note_content_append();
        state.rebase_lines(1);

        let appended = vec![initial[1].clone(), initial[2].clone(), Line::from("delta")];
        let viewport = Viewport::new(&appended, &system).content_revision(2);
        viewport.render(area, &mut Buffer::empty(area), &mut state);

        assert_eq!(state.active_cells().len(), 3);
        assert_eq!(state.active_cells()[0].cells.as_ptr(), surviving_cells);
    }
}
