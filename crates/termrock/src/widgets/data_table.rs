// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **DataTable** — category-leading interactive / virtualized grid for pro tools.
//!
//! **Mission.** Sort, filter/search, column resize/visibility/pin/reorder hooks,
//! grouping, row/cell/range selection, inline edit, copy, context actions,
//! sticky header + pin strips, remote/partial load, unknown totals, and
//! million-row logical datasets via consumer projection.
//!
//! **Law.** Paint and select-all touch only the **projected** slice. For 1M
//! logical rows, call `window.visible_range()` and project that window only.
//!
//! **Cursor vs scene focus.** [`DataTableState::cursor_row`] / [`cursor_col`] are
//! the in-table cursor. Scene focus is host-owned; pass [`DataTable::focused`].
//! Outcomes use [`DataTableOutcome::CursorMoved`] — not scene FocusChanged.
//!
//! Research: VisiData, Textual DataTable, DB clients, k9s, btop, spreadsheets.
//! Display-only moderate tables use [`super::Table`]; this is the interactive kit.
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Glyph, ListRowVisualState, Role},
    text::{display_cols, take_display_cols, truncate_cols},
    widgets::data_view::{
        CellCoord, ColumnKind, ColumnModel, ColumnPin, CopyPayload, ExpandState, FilterSpec,
        GroupHeader, LoadState, SelectionMode, SelectionModel, SortSpec, VirtualWindow,
    },
};

/// Decimal digit count of `n` (at least one digit). Paint calls the chrome
/// width several times per frame; `to_string().len()` would allocate each time.
fn decimal_digits(n: usize) -> u16 {
    let mut n = n.max(1);
    let mut digits = 1u16;
    while n >= 10 {
        n /= 10;
        digits += 1;
    }
    digits
}

/// Junie grid chrome: `▎` + select `✓` + change slot + optional row numbers
/// + a pad column. Matches `junie-tui` `gutter_w = 3 + num_w + row_numbers`.
fn grid_chrome_width(row_count: usize, row_numbers: bool) -> u16 {
    let num_w = if row_numbers {
        decimal_digits(row_count).max(2)
    } else {
        0
    };
    3 + num_w + u16::from(row_numbers)
}

/// Column separator, from the glyph catalog rather than a file-local literal.
const RESIZE_HIT: u16 = 1;

/// Keyboard navigation mode (VisiData-like layers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DataTableNavMode {
    /// Arrow keys move the cell cursor (junie `cell_nav: true`).
    Cell,
    /// Primary axis is rows; Left/Right h-scroll (junie `cell_nav: false`).
    #[default]
    Row,
    /// Shift-extend builds a rectangular cell range from the anchor.
    Range,
}

impl DataTableNavMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cell => "cell",
            Self::Row => "row",
            Self::Range => "range",
        }
    }

    /// Cycle Cell → Row → Range → Cell.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Cell => Self::Row,
            Self::Row => Self::Range,
            Self::Range => Self::Cell,
        }
    }
}

/// Header / body hit geometry from the last paint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableHeaderRegion<ColId> {
    /// Column id.
    pub id: ColId,
    /// Painted header rect (title area).
    pub area: Rect,
    /// Resize handle at the right edge of the column.
    pub resize_handle: Rect,
    /// Whether header click may sort.
    pub sortable: bool,
}

/// One body cell hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableCellRegion<RowId, ColId> {
    /// Row id.
    pub row: RowId,
    /// Column id.
    pub column: ColId,
    /// Projected row index.
    pub row_index: usize,
    /// Visible column ordinal.
    pub col_index: usize,
    /// Painted cell rect.
    pub area: Rect,
}

/// Toolbar action ids are consumer-owned strings/labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableToolbar<'a> {
    /// Leading action labels (projected).
    pub actions: &'a [&'a str],
}

/// DataTable outcomes — never silent full-scan select-all of unloaded rows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DataTableOutcome<RowId, ColId> {
    /// No change.
    Ignored,
    /// Viewport scrolled (row or column window).
    Scrolled,
    /// Cursor moved within projected slice.
    CursorMoved,
    /// The pointer moved onto (or off) a row.
    HoverChanged,
    /// Sort requested for column (consumer sorts / re-projects).
    SortRequested(ColId),
    /// Sort with direction (toggle chrome).
    SortSpec(SortSpec<ColId>),
    /// Filter / search changed.
    FilterChanged(FilterSpec),
    /// Row activated.
    Activate(RowId),
    /// Selection changed for one row.
    ToggleRow(RowId),
    /// Cell / range selection changed.
    SelectionChanged,
    /// Select-all **requested** for currently projected/visible scope only.
    SelectAllRequested,
    /// Copy cursor row/cell/range (consumer supplies text via projected data).
    Copy(CopyPayload),
    /// Expand/collapse detail for row.
    ExpandToggled(RowId),
    /// Group header toggled.
    GroupToggled(RowId),
    /// Context menu at cursor.
    ContextMenu {
        /// Cursor row.
        row: RowId,
        /// Column when known.
        column: Option<ColId>,
    },
    /// Inline edit requested.
    EditStarted {
        /// Row.
        row: RowId,
        /// Column id when known.
        column: Option<ColId>,
    },
    /// Inline edit committed (host applies domain write).
    EditCommitted {
        /// Row.
        row: RowId,
        /// Column.
        column: ColId,
        /// Proposed text (host may validate).
        text: String,
    },
    /// Inline edit cancelled.
    EditCancelled,
    /// Retry load.
    RetryLoad,
    /// Bulk action index from toolbar.
    ToolbarAction(usize),
    /// Column resized by pointer / keys.
    ColumnResized {
        /// Column.
        column: ColId,
        /// New width in cells.
        width: u16,
    },
    /// Column visibility toggled.
    ColumnVisibility {
        /// Column.
        column: ColId,
        /// Visible after toggle.
        visible: bool,
    },
    /// Host should reorder columns (`from` → `to` display indices).
    ColumnReorderRequested {
        /// Source display index among all columns.
        from: usize,
        /// Target display index.
        to: usize,
    },
    /// Promote surface to fullscreen / focus workspace (host policy).
    FullscreenRequested,
    /// Navigation mode cycled.
    NavModeChanged(DataTableNavMode),
}

/// DataTable interaction + geometry state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableState<RowId: Clone + Ord, ColId: Clone + PartialEq> {
    /// Selection model (row / multi / cell / range).
    pub selection: SelectionModel<RowId>,
    /// Vertical virtual window over logical rows.
    pub window: VirtualWindow,
    /// Horizontal scroll offset in display columns among unpinned center columns.
    pub h_offset: u16,
    /// Cursor row index in projected slice (0..projected.len()).
    pub cursor_row: usize,
    /// Cursor visible-column ordinal among paint order.
    pub cursor_col: usize,
    /// Keyboard navigation mode.
    pub nav_mode: DataTableNavMode,
    /// Load projection.
    pub load: LoadState,
    /// Expand detail rows.
    pub expand: ExpandState<RowId>,
    /// Active sort (chrome marker; consumer applies).
    pub sort: Option<SortSpec<ColId>>,
    /// Active filter (chrome; consumer applies).
    pub filter: FilterSpec,
    /// Stripes.
    pub striped: bool,
    /// Suppress chromatic roles (Text / TextMuted / TextStrong only).
    pub colorless: bool,
    /// Host grants keyboard/pointer input to this surface (scene-focused).
    pub accepts_input: bool,
    /// Whether an inline edit session is open (host owns text buffer).
    pub editing: bool,
    /// Pending edit draft (host may mirror).
    pub edit_draft: String,
    /// Header hit regions from last paint.
    pub header_regions: Vec<DataTableHeaderRegion<ColId>>,
    /// Body cell hit regions from last paint.
    pub cell_regions: Vec<DataTableCellRegion<RowId, ColId>>,
    /// Row the pointer is over. Hover washes; it never selects.
    pub hovered_row: Option<RowId>,
    /// Active column resize drag (column id + start width + start x).
    resize_drag: Option<(ColId, u16, u16)>,
    /// Range-selection drag anchor cell.
    range_anchor: Option<CellCoord>,
    /// Painted body origin (for mouse hit testing).
    body_origin: (u16, u16),
    /// Painted body height in rows.
    body_rows: u16,
    /// Painted body width.
    body_width: u16,
    /// Scratch: resolved (col_index, width) for paint.
    paint_widths: Vec<(usize, u16)>,
    /// Scratch: clipped physical rect for each resolved paint column.
    paint_rects: Vec<Rect>,
    /// Last host-column identities, used to remap cell coordinates after reorder.
    last_column_ids: Vec<ColId>,
    /// Last visible-column identities, used to preserve cursor identity.
    last_visible_column_ids: Vec<ColId>,
    /// Logical column count for h-scroll max.
    content_width: u16,
    /// Viewport width for columns (area − gutter).
    viewport_width: u16,
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> DataTableState<RowId, ColId> {
    /// Fresh multi-select table in row navigation (junie `cell_nav: false`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            selection: SelectionModel::multi_row(),
            window: VirtualWindow::default(),
            h_offset: 0,
            cursor_row: 0,
            cursor_col: 0,
            nav_mode: DataTableNavMode::Row,
            load: LoadState::Ready { count: 0 },
            expand: ExpandState::default(),
            sort: None,
            filter: FilterSpec::default(),
            striped: true,
            colorless: false,
            accepts_input: true,
            editing: false,
            edit_draft: String::new(),
            header_regions: Vec::new(),
            cell_regions: Vec::new(),
            hovered_row: None,
            resize_drag: None,
            range_anchor: None,
            body_origin: (0, 0),
            body_rows: 0,
            body_width: 0,
            paint_widths: Vec::new(),
            paint_rects: Vec::new(),
            last_column_ids: Vec::new(),
            last_visible_column_ids: Vec::new(),
            content_width: 0,
            viewport_width: 0,
        }
    }

    /// Configure logical universe size (e.g. 1_000_000) without allocating rows.
    pub fn set_logical_rows(&mut self, logical_len: u64) {
        self.window.logical_len = logical_len;
        self.window.clamp();
        self.sync_cursor_focus();
    }

    /// Host surface input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Sets navigation mode.
    pub fn set_nav_mode(&mut self, mode: DataTableNavMode) {
        self.nav_mode = mode;
        match mode {
            DataTableNavMode::Cell => {
                if !matches!(
                    self.selection.mode,
                    SelectionMode::Cell | SelectionMode::CellRange | SelectionMode::MultiRow
                ) {
                    // keep multi-row; cell chrome overlays cursor
                }
            }
            DataTableNavMode::Row => {
                self.selection.mode = SelectionMode::MultiRow;
            }
            DataTableNavMode::Range => {
                self.selection.mode = SelectionMode::CellRange;
            }
        }
    }

    /// Horizontal scroll by display columns.
    pub fn scroll_horizontal(&mut self, delta: i16) -> bool {
        let max = self.content_width.saturating_sub(self.viewport_width);
        let next = if delta >= 0 {
            self.h_offset.saturating_add(delta as u16).min(max)
        } else {
            self.h_offset.saturating_sub((-delta) as u16)
        };
        let changed = next != self.h_offset;
        self.h_offset = next;
        changed
    }

    /// Keys over projected row ids (visible slice only).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        if !self.accepts_input || key.is_release() {
            return DataTableOutcome::Ignored;
        }
        let is_press = key.is_press();

        if self.editing {
            if visible_rows.is_empty()
                || !matches!(
                    self.load,
                    LoadState::Ready { .. } | LoadState::Partial { .. }
                )
            {
                return self.cancel_edit();
            }
            return self.handle_edit_key(key, visible_rows, columns);
        }

        // Empty / error / loading: only retry
        if matches!(
            self.load,
            LoadState::Empty { .. } | LoadState::Error { .. } | LoadState::Loading { .. }
        ) {
            if is_press && matches!(key.code, KeyCode::Char('r' | 'R') | KeyCode::Enter) {
                return DataTableOutcome::RetryLoad;
            }
            return DataTableOutcome::Ignored;
        }

        if visible_rows.is_empty() {
            return DataTableOutcome::Ignored;
        }
        self.cursor_row = self.cursor_row.min(visible_rows.len() - 1);

        self.sync_cursor_focus_to_columns(columns);

        // Mode cycle (Tab with no modifiers while table owns input — VisiData-ish layer)
        if is_press && matches!(key.code, KeyCode::Char('\\')) {
            self.nav_mode = self.nav_mode.cycle();
            self.set_nav_mode(self.nav_mode);
            return DataTableOutcome::NavModeChanged(self.nav_mode);
        }

        if is_press
            && matches!(key.code, KeyCode::Char('f') | KeyCode::Char('F'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return DataTableOutcome::FullscreenRequested;
        }

        if let Some(intent) = crate::interaction::default_data_table_intent(key) {
            let out = self.handle_intent(intent, visible_rows, columns);
            if !matches!(out, DataTableOutcome::Ignored) {
                return out;
            }
        }

        // Product chords not in the generic intent map.
        match key.code {
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.shift_extend_or_expand(visible_rows, columns, -1, 0)
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.shift_extend_or_expand(visible_rows, columns, 1, 0)
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.shift_extend_or_expand(visible_rows, columns, 0, -1)
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.shift_extend_or_expand(visible_rows, columns, 0, 1)
            }
            KeyCode::Left | KeyCode::Char('h') => self.move_horizontal(columns, -1),
            KeyCode::Right | KeyCode::Char('l') => self.move_horizontal(columns, 1),
            KeyCode::Char('a') if is_press && key.modifiers.contains(KeyModifiers::CONTROL) => {
                DataTableOutcome::SelectAllRequested
            }
            KeyCode::Char('s') if is_press => self.request_sort(columns),
            KeyCode::Char('/') if is_press => DataTableOutcome::FilterChanged(self.filter.clone()),
            KeyCode::Char('c') if is_press && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.copy_payload(visible_rows, columns)
            }
            KeyCode::Char('e') if is_press => {
                let col = self.cursor_column_id(columns);
                // Read-only columns never enter edit mode: the host never has
                // to defend against EditStarted on a column it cannot apply.
                let editable = col.as_ref().is_some_and(|id| {
                    columns
                        .columns
                        .iter()
                        .find(|c| &c.id == id)
                        .is_some_and(|c| c.editable)
                });
                if !editable {
                    return DataTableOutcome::Ignored;
                }
                self.editing = true;
                self.edit_draft.clear();
                DataTableOutcome::EditStarted {
                    row: visible_rows[self.cursor_row].clone(),
                    column: col,
                }
            }
            KeyCode::Char('x') if is_press => DataTableOutcome::ContextMenu {
                row: visible_rows[self.cursor_row].clone(),
                column: self.cursor_column_id(columns),
            },
            KeyCode::Char('[') if is_press => {
                // Nudge shrink focused column
                self.resize_cursor_column(columns, -1)
            }
            KeyCode::Char(']') if is_press => self.resize_cursor_column(columns, 1),
            KeyCode::Char(',') if is_press => {
                // Reorder: move cursor column left
                self.reorder_cursor_column(columns, -1)
            }
            KeyCode::Char('.') if is_press => self.reorder_cursor_column(columns, 1),
            KeyCode::Char('v') | KeyCode::Char('V') if is_press => {
                // Toggle visibility of lowest-priority unpinned (request only if we hide)
                DataTableOutcome::Ignored
            }
            KeyCode::Esc if is_press => {
                self.selection.clear_selection();
                self.range_anchor = None;
                DataTableOutcome::SelectionChanged
            }
            _ => DataTableOutcome::Ignored,
        }
    }

    fn handle_edit_key(
        &mut self,
        key: KeyEvent,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        if !key.is_press() {
            return DataTableOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => self.cancel_edit(),
            KeyCode::Enter => self.commit_edit(visible_rows, columns),
            KeyCode::Backspace => {
                self.edit_draft.pop();
                let col = self.cursor_column_id(columns);
                DataTableOutcome::EditStarted {
                    row: visible_rows[self.cursor_row.min(visible_rows.len().saturating_sub(1))]
                        .clone(),
                    column: col,
                }
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_draft.push(ch);
                let col = self.cursor_column_id(columns);
                DataTableOutcome::EditStarted {
                    row: visible_rows[self.cursor_row.min(visible_rows.len().saturating_sub(1))]
                        .clone(),
                    column: col,
                }
            }
            _ => DataTableOutcome::Ignored,
        }
    }

    fn cancel_edit(&mut self) -> DataTableOutcome<RowId, ColId> {
        self.editing = false;
        self.edit_draft.clear();
        DataTableOutcome::EditCancelled
    }

    fn commit_edit(
        &mut self,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        let Some(row) = visible_rows.get(self.cursor_row) else {
            return self.cancel_edit();
        };
        let Some(col) = self.cursor_column_id(columns) else {
            return self.cancel_edit();
        };
        let text = std::mem::take(&mut self.edit_draft);
        self.editing = false;
        DataTableOutcome::EditCommitted {
            row: row.clone(),
            column: col,
            text,
        }
    }

    /// Return the source identity of the current projected cursor column.
    pub fn cursor_column_id(&self, columns: &ColumnModel<ColId>) -> Option<ColId>
    where
        ColId: Clone,
    {
        let visible_count = columns.visible().count();
        let projection_changed = self.last_visible_column_ids.len() != visible_count
            || self
                .last_visible_column_ids
                .iter()
                .zip(columns.visible())
                .any(|(old, (_, current))| old != &current.id);
        if projection_changed
            && let Some(old_id) = self.last_visible_column_ids.get(self.cursor_col)
            && let Some((_, column)) = columns.visible().find(|(_, column)| &column.id == old_id)
        {
            return Some(column.id.clone());
        }
        columns
            .visible()
            .nth(self.cursor_col)
            .map(|(_, column)| column.id.clone())
    }

    fn sync_cursor_focus(&mut self) {
        self.selection.focus_row = self.window.offset.saturating_add(self.cursor_row as u64);
        self.selection.focus_col = self.cursor_col;
    }

    fn sync_cursor_focus_to_columns(&mut self, columns: &ColumnModel<ColId>) {
        let old_cursor_column = self.last_visible_column_ids.get(self.cursor_col).cloned();
        let columns_changed = self.last_column_ids.len() != columns.columns.len()
            || self
                .last_column_ids
                .iter()
                .zip(&columns.columns)
                .any(|(old, current)| old != &current.id);
        if columns_changed && !self.last_column_ids.is_empty() {
            remap_cell_selection_columns(
                &mut self.selection,
                &mut self.range_anchor,
                &self.last_column_ids,
                columns,
            );
        }

        let visible_count = columns.visible().count();
        let visible_columns_changed = self.last_visible_column_ids.len() != visible_count
            || self
                .last_visible_column_ids
                .iter()
                .zip(columns.visible())
                .any(|(old, (_, current))| old != &current.id);
        if let Some(old_id) = old_cursor_column
            && visible_columns_changed
            && let Some(next) = columns
                .visible()
                .position(|(_, column)| column.id == old_id)
        {
            self.cursor_col = next;
        } else {
            self.cursor_col = self.cursor_col.min(visible_count.saturating_sub(1));
        }

        if columns_changed {
            self.last_column_ids.clear();
            self.last_column_ids
                .extend(columns.columns.iter().map(|column| column.id.clone()));
        }
        if visible_columns_changed {
            self.last_visible_column_ids.clear();
            self.last_visible_column_ids
                .extend(columns.visible().map(|(_, column)| column.id.clone()));
        }
        self.cursor_col = self.cursor_col.min(visible_count.saturating_sub(1));
        self.sync_cursor_focus();
    }

    fn cursor_column_index(&self, columns: &ColumnModel<ColId>) -> Option<usize> {
        columns
            .visible()
            .nth(self.cursor_col)
            .map(|(index, _)| index)
    }

    fn sync_cursor_focus_to_paint(&mut self, columns: &ColumnModel<ColId>) {
        let Some(cursor_index) = self.cursor_column_index(columns) else {
            self.cursor_col = 0;
            self.sync_cursor_focus();
            return;
        };
        if self
            .paint_widths
            .iter()
            .any(|(index, _)| *index == cursor_index)
        {
            return;
        }
        let Some((next_index, _)) = self.paint_widths.iter().min_by_key(|(index, _)| {
            let visible_index = visible_column_ordinal(columns, *index).unwrap_or(0);
            (visible_index.abs_diff(self.cursor_col), visible_index)
        }) else {
            self.cursor_col = 0;
            self.sync_cursor_focus();
            return;
        };
        if let Some(next) = visible_column_ordinal(columns, *next_index) {
            self.cursor_col = next;
            self.sync_cursor_focus();
        }
    }

    fn ensure_cursor_column_projected(
        &mut self,
        columns: &ColumnModel<ColId>,
        viewport_width: u16,
        gap: u16,
    ) -> bool {
        if !matches!(
            self.nav_mode,
            DataTableNavMode::Cell | DataTableNavMode::Range
        ) {
            return false;
        }
        let Some(cursor_index) = self.cursor_column_index(columns) else {
            return false;
        };
        if self
            .paint_widths
            .iter()
            .any(|(index, _)| *index == cursor_index)
        {
            return false;
        }
        let visible_count = columns.visible().count();
        let total_width = columns
            .visible()
            .map(|(index, _)| u64::from(columns.effective_width(index)))
            .sum::<u64>()
            .saturating_add(u64::from(gap.saturating_mul(
                u16::try_from(visible_count.saturating_sub(1)).unwrap_or(0),
            )));
        let required = total_width.saturating_sub(u64::from(viewport_width));
        let next = self
            .h_offset
            .max(u16::try_from(required).unwrap_or(u16::MAX));
        let changed = next != self.h_offset;
        self.h_offset = next;
        changed
    }

    fn reveal_cursor_column(
        &mut self,
        columns: &ColumnModel<ColId>,
        center_left: u16,
        center_right: u16,
        max_h: u16,
        gap: u16,
    ) -> bool {
        if !matches!(
            self.nav_mode,
            DataTableNavMode::Cell | DataTableNavMode::Range
        ) {
            return false;
        }
        let Some(cursor_index) = self.cursor_column_index(columns) else {
            return false;
        };
        let mut center_offset = 0i64;
        let mut target = None;
        for &(index, width) in &self.paint_widths {
            if columns.columns[index].pin == ColumnPin::None {
                if index == cursor_index {
                    let left = i64::from(center_left) + center_offset - i64::from(self.h_offset);
                    target = Some((left, left + i64::from(width)));
                }
                center_offset += i64::from(width) + i64::from(gap);
            }
        }
        let Some((target_left, target_right)) = target else {
            return false;
        };
        let left = i64::from(center_left);
        let right = i64::from(center_right);
        let next = if target_left < left {
            self.h_offset
                .saturating_sub(u16::try_from(left - target_left).unwrap_or(u16::MAX))
        } else if target_right > right {
            self.h_offset
                .saturating_add(u16::try_from(target_right - right).unwrap_or(u16::MAX))
                .min(max_h)
        } else {
            self.h_offset
        };
        let changed = next != self.h_offset;
        self.h_offset = next;
        changed
    }

    fn request_sort(&mut self, columns: &ColumnModel<ColId>) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        let col_id = self
            .cursor_column_id(columns)
            .or_else(|| columns.visible().next().map(|(_, c)| c.id.clone()));
        let Some(col) = col_id else {
            return DataTableOutcome::Ignored;
        };
        let ascending = match &self.sort {
            Some(s) if s.column == col => !s.ascending,
            _ => true,
        };
        let spec = SortSpec {
            column: col,
            ascending,
        };
        self.sort = Some(spec.clone());
        DataTableOutcome::SortSpec(spec)
    }

    fn copy_payload(
        &self,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        // Host fills text; we emit shape based on nav mode / selection.
        match self.nav_mode {
            DataTableNavMode::Cell | DataTableNavMode::Range
                if matches!(
                    self.selection.mode,
                    SelectionMode::Cell | SelectionMode::CellRange
                ) =>
            {
                DataTableOutcome::Copy(CopyPayload::Cell {
                    text: String::new(),
                })
            }
            _ => {
                let _ = (visible_rows, columns);
                DataTableOutcome::Copy(CopyPayload::Row { cells: Vec::new() })
            }
        }
    }

    /// Column width implied by a drag gesture: start width plus horizontal
    /// delta, clamped to the same `2..=80` band the keyboard resize uses.
    fn drag_width(start_w: u16, start_x: u16, x: u16) -> u16 {
        let dx = x as i32 - start_x as i32;
        (start_w as i32 + dx).clamp(2, 80) as u16
    }

    fn resize_cursor_column(
        &mut self,
        columns: &ColumnModel<ColId>,
        delta: i16,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        let Some((_, col)) = columns.visible().nth(self.cursor_col) else {
            return DataTableOutcome::Ignored;
        };
        let id = col.id.clone();
        let idx = columns.index_of(&id).unwrap_or(0);
        let cur = columns.effective_width(idx);
        let next = if delta >= 0 {
            cur.saturating_add(delta as u16).min(80)
        } else {
            cur.saturating_sub((-delta) as u16).max(2)
        };
        DataTableOutcome::ColumnResized {
            column: id,
            width: next,
        }
    }

    fn reorder_cursor_column(
        &mut self,
        columns: &ColumnModel<ColId>,
        delta: i16,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        let Some((from, _)) = columns.visible().nth(self.cursor_col) else {
            return DataTableOutcome::Ignored;
        };
        let to = if delta < 0 {
            from.saturating_sub(1)
        } else {
            (from + 1).min(columns.columns.len().saturating_sub(1))
        };
        if to == from {
            return DataTableOutcome::Ignored;
        }
        DataTableOutcome::ColumnReorderRequested { from, to }
    }

    fn move_horizontal(
        &mut self,
        columns: &ColumnModel<ColId>,
        delta: i16,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        let vis_n = columns.visible().count();
        if vis_n == 0 {
            return DataTableOutcome::Ignored;
        }
        match self.nav_mode {
            DataTableNavMode::Row => {
                if self.scroll_horizontal(delta * 4) {
                    DataTableOutcome::Scrolled
                } else {
                    DataTableOutcome::Ignored
                }
            }
            DataTableNavMode::Cell | DataTableNavMode::Range => {
                let next = if delta < 0 {
                    self.cursor_col.saturating_sub(1)
                } else {
                    (self.cursor_col + 1).min(vis_n - 1)
                };
                if next != self.cursor_col {
                    self.cursor_col = next;
                    self.selection.focus_col = next;
                    return DataTableOutcome::CursorMoved;
                }
                if self.scroll_horizontal(delta) {
                    DataTableOutcome::Scrolled
                } else {
                    DataTableOutcome::Ignored
                }
            }
        }
    }

    fn shift_extend_or_expand(
        &mut self,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
        d_col: i16,
        d_row: i16,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        if matches!(self.nav_mode, DataTableNavMode::Range)
            || matches!(self.selection.mode, SelectionMode::CellRange)
        {
            let Some(column_index) = self.cursor_column_index(columns) else {
                return DataTableOutcome::Ignored;
            };
            if d_row != 0 {
                let _ = self.move_cursor_row(i64::from(d_row), visible_rows.len());
            }
            if d_col != 0 {
                let next = if d_col < 0 {
                    self.cursor_col.saturating_sub(1)
                } else {
                    (self.cursor_col + 1).min(columns.visible().count() - 1)
                };
                self.cursor_col = next;
            }
            let column_index = self.cursor_column_index(columns).unwrap_or(column_index);
            let cell = CellCoord {
                row: self.window.offset.saturating_add(self.cursor_row as u64),
                col: column_index,
            };
            if self.range_anchor.is_none() {
                self.range_anchor = Some(CellCoord {
                    row: self.window.offset.saturating_add(self.cursor_row as u64),
                    col: column_index,
                });
                self.selection.select_cell(cell);
            } else {
                self.selection.extend_cell(cell);
            }
            self.sync_cursor_focus();
            return DataTableOutcome::SelectionChanged;
        }
        // Default: expand/collapse detail
        if d_col != 0 {
            let id = visible_rows[self.cursor_row].clone();
            let _ = self.expand.toggle(id.clone());
            return DataTableOutcome::ExpandToggled(id);
        }
        DataTableOutcome::Ignored
    }

    /// Semantic intent routing for navigation / activate / toggle.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        if !self.accepts_input {
            return DataTableOutcome::Ignored;
        }
        if self.editing {
            if visible_rows.is_empty()
                || !matches!(
                    self.load,
                    LoadState::Ready { .. } | LoadState::Partial { .. }
                )
            {
                return self.cancel_edit();
            }
            return match intent {
                UiIntent::Submit => self.commit_edit(visible_rows, columns),
                UiIntent::Cancel => self.cancel_edit(),
                _ => DataTableOutcome::Ignored,
            };
        }
        if matches!(
            self.load,
            LoadState::Empty { .. } | LoadState::Error { .. } | LoadState::Loading { .. }
        ) {
            if matches!(intent, UiIntent::Activate | UiIntent::Submit) {
                return DataTableOutcome::RetryLoad;
            }
            return DataTableOutcome::Ignored;
        }
        if visible_rows.is_empty() {
            return DataTableOutcome::Ignored;
        }
        self.cursor_row = self.cursor_row.min(visible_rows.len() - 1);
        self.sync_cursor_focus_to_columns(columns);
        match intent {
            UiIntent::Move(NavigationMove::Next) | UiIntent::Move(NavigationMove::Down) => {
                self.move_cursor_row(1, visible_rows.len())
            }
            UiIntent::Move(NavigationMove::Previous) | UiIntent::Move(NavigationMove::Up) => {
                self.move_cursor_row(-1, visible_rows.len())
            }
            UiIntent::Move(NavigationMove::First) => {
                self.cursor_row = 0;
                self.sync_cursor_focus();
                DataTableOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.cursor_row = visible_rows.len().saturating_sub(1);
                self.sync_cursor_focus();
                DataTableOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::Left) => self.move_horizontal(columns, -1),
            UiIntent::Move(NavigationMove::Right) => self.move_horizontal(columns, 1),
            UiIntent::Page(PageMove::Forward) => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(step) {
                    self.sync_cursor_focus();
                    DataTableOutcome::Scrolled
                } else {
                    self.move_cursor_row(step, visible_rows.len())
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(-step) {
                    self.sync_cursor_focus();
                    DataTableOutcome::Scrolled
                } else {
                    self.move_cursor_row(-step, visible_rows.len())
                }
            }
            UiIntent::Activate | UiIntent::Submit => {
                DataTableOutcome::Activate(visible_rows[self.cursor_row].clone())
            }
            UiIntent::Toggle => {
                let id = visible_rows[self.cursor_row].clone();
                self.selection.toggle_row(id.clone());
                DataTableOutcome::ToggleRow(id)
            }
            UiIntent::Expand => {
                let id = visible_rows[self.cursor_row].clone();
                let _ = self.expand.toggle(id.clone());
                DataTableOutcome::ExpandToggled(id)
            }
            UiIntent::Collapse => {
                let id = visible_rows[self.cursor_row].clone();
                if self.expand.expanded.contains(&id) {
                    let _ = self.expand.toggle(id.clone());
                    DataTableOutcome::ExpandToggled(id)
                } else {
                    DataTableOutcome::Ignored
                }
            }
            UiIntent::Cancel => {
                self.selection.clear_selection();
                self.range_anchor = None;
                DataTableOutcome::SelectionChanged
            }
            UiIntent::Open | UiIntent::Close => DataTableOutcome::Ignored,
            _ => DataTableOutcome::Ignored,
        }
    }

    /// Mouse: wheel, click cursor/select, header sort, resize drag, context.
    ///
    /// Like every other handler, this never writes the host's
    /// [`ColumnModel`]: a resize drag reports [`DataTableOutcome::ColumnResized`]
    /// and the host applies it with [`ColumnModel::set_width_override`], the
    /// same contract as the keyboard path. The reported width is derived from
    /// the gesture itself, so it stays correct even if intermediate drag
    /// outcomes were dropped.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        if !self.accepts_input {
            return DataTableOutcome::Ignored;
        }
        let (ox, oy) = self.body_origin;
        let body = Rect {
            x: ox,
            y: oy,
            width: self.body_width,
            height: self.body_rows,
        };

        // Resize drag in progress
        if let Some((ref col_id, start_w, start_x)) = self.resize_drag.clone() {
            match event.kind {
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
                    let width = Self::drag_width(start_w, start_x, event.position.x);
                    return DataTableOutcome::ColumnResized {
                        column: col_id.clone(),
                        width,
                    };
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    let width = Self::drag_width(start_w, start_x, event.position.x);
                    self.resize_drag = None;
                    return DataTableOutcome::ColumnResized {
                        column: col_id.clone(),
                        width,
                    };
                }
                _ => {}
            }
        }

        if matches!(event.kind, MouseEventKind::Moved) {
            // Hover is stated every event, so leaving the body clears it.
            let was = self.hovered_row.clone();
            self.hovered_row = self
                .cell_regions
                .iter()
                .find(|region| region.area.contains(event.position))
                .map(|region| region.row.clone());
            if was != self.hovered_row {
                return DataTableOutcome::HoverChanged;
            }
        }

        if body.contains(event.position)
            && let Some(crate::scroll::ScrollDelta {
                axis: crate::scroll::ScrollAxis::Horizontal,
                amount,
            }) = crate::scroll::mouse_scroll_delta_with_step(
                event.kind,
                event.modifiers,
                crate::scroll::ScrollAxes {
                    vertical: true,
                    horizontal: true,
                },
                4,
            )
        {
            return if self.scroll_horizontal(amount) {
                DataTableOutcome::Scrolled
            } else {
                DataTableOutcome::Ignored
            };
        }

        match event.kind {
            MouseEventKind::ScrollUp if body.contains(event.position) => {
                if self.window.scroll_by(-1) {
                    self.sync_cursor_focus();
                    DataTableOutcome::Scrolled
                } else if !visible_rows.is_empty() {
                    self.move_cursor_row(-1, visible_rows.len())
                } else {
                    DataTableOutcome::Ignored
                }
            }
            MouseEventKind::ScrollDown if body.contains(event.position) => {
                if self.window.scroll_by(1) {
                    self.sync_cursor_focus();
                    DataTableOutcome::Scrolled
                } else if !visible_rows.is_empty() {
                    self.move_cursor_row(1, visible_rows.len())
                } else {
                    DataTableOutcome::Ignored
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(cell) = self.hit_cell(event.position) {
                    self.cursor_row = cell.row_index;
                    self.cursor_col = cell.col_index;
                    self.sync_cursor_focus();
                    return DataTableOutcome::ContextMenu {
                        row: cell.row,
                        column: Some(cell.column),
                    };
                }
                DataTableOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Resize handle first
                if let Some(region) = self
                    .header_regions
                    .iter()
                    .find(|r| r.resize_handle.contains(event.position))
                {
                    let idx = columns.index_of(&region.id).unwrap_or(0);
                    let w = columns.effective_width(idx);
                    self.resize_drag = Some((region.id.clone(), w, event.position.x));
                    return DataTableOutcome::Ignored;
                }
                // Header sort
                if let Some(region) = self
                    .header_regions
                    .iter()
                    .find(|r| r.sortable && r.area.contains(event.position))
                {
                    let col = region.id.clone();
                    let ascending = match &self.sort {
                        Some(s) if s.column == col => !s.ascending,
                        _ => true,
                    };
                    let spec = SortSpec {
                        column: col,
                        ascending,
                    };
                    self.sort = Some(spec.clone());
                    return DataTableOutcome::SortSpec(spec);
                }
                // Body cell
                if let Some(cell) = self.hit_cell(event.position) {
                    let Some(column_index) = columns.index_of(&cell.column) else {
                        return DataTableOutcome::Ignored;
                    };
                    self.cursor_row = cell.row_index;
                    self.cursor_col = cell.col_index;
                    self.sync_cursor_focus();
                    if event.modifiers.contains(KeyModifiers::SHIFT)
                        || matches!(self.nav_mode, DataTableNavMode::Range)
                    {
                        let coord = CellCoord {
                            row: self.window.offset.saturating_add(cell.row_index as u64),
                            col: column_index,
                        };
                        if self.range_anchor.is_none() {
                            self.range_anchor = Some(coord);
                            self.selection.select_cell(coord);
                        } else {
                            self.selection.extend_cell(coord);
                        }
                        self.sync_cursor_focus();
                        return DataTableOutcome::SelectionChanged;
                    }
                    if matches!(
                        self.selection.mode,
                        SelectionMode::Cell | SelectionMode::CellRange
                    ) {
                        self.selection.select_cell(CellCoord {
                            row: self.window.offset.saturating_add(cell.row_index as u64),
                            col: column_index,
                        });
                        self.sync_cursor_focus();
                    }
                    return DataTableOutcome::CursorMoved;
                }
                // Fallback: body row by y
                if body.contains(event.position) && !visible_rows.is_empty() {
                    let row = usize::from(event.position.y.saturating_sub(oy));
                    if row < visible_rows.len() {
                        if self.cursor_row == row {
                            return DataTableOutcome::Activate(visible_rows[row].clone());
                        }
                        self.cursor_row = row;
                        self.sync_cursor_focus();
                        return DataTableOutcome::CursorMoved;
                    }
                }
                DataTableOutcome::Ignored
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(cell) = self.hit_cell(event.position) {
                    let Some(column_index) = columns.index_of(&cell.column) else {
                        return DataTableOutcome::Ignored;
                    };
                    self.cursor_row = cell.row_index;
                    self.cursor_col = cell.col_index;
                    self.sync_cursor_focus();
                    let coord = CellCoord {
                        row: self.window.offset.saturating_add(cell.row_index as u64),
                        col: column_index,
                    };
                    if self.range_anchor.is_none() {
                        self.range_anchor = Some(coord);
                        self.selection.select_cell(coord);
                    } else {
                        self.selection.extend_cell(coord);
                    }
                    self.sync_cursor_focus();
                    return DataTableOutcome::SelectionChanged;
                }
                DataTableOutcome::Ignored
            }
            _ => DataTableOutcome::Ignored,
        }
    }

    fn hit_cell(&self, position: Position) -> Option<DataTableCellRegion<RowId, ColId>>
    where
        RowId: Clone,
        ColId: Clone,
    {
        self.cell_regions
            .iter()
            .find(|r| r.area.contains(position))
            .cloned()
    }

    fn move_cursor_row(&mut self, delta: i64, len: usize) -> DataTableOutcome<RowId, ColId> {
        if len == 0 {
            return DataTableOutcome::Ignored;
        }
        let cur = self.cursor_row as i64;
        let next = (cur + delta).clamp(0, (len as i64) - 1) as usize;
        if next == self.cursor_row {
            if delta > 0 && self.window.scroll_by(1) {
                self.sync_cursor_focus();
                return DataTableOutcome::Scrolled;
            }
            if delta < 0 && self.window.scroll_by(-1) {
                self.sync_cursor_focus();
                return DataTableOutcome::Scrolled;
            }
            return DataTableOutcome::Ignored;
        }
        self.cursor_row = next;
        self.sync_cursor_focus();
        DataTableOutcome::CursorMoved
    }
}

fn remap_cell_coord<ColId: PartialEq>(
    cell: CellCoord,
    old_column_ids: &[ColId],
    columns: &ColumnModel<ColId>,
) -> Option<CellCoord> {
    let old_id = old_column_ids.get(cell.col)?;
    let col = columns.index_of(old_id)?;
    Some(CellCoord { row: cell.row, col })
}

fn visible_column_ordinal<ColId: PartialEq>(
    columns: &ColumnModel<ColId>,
    source_index: usize,
) -> Option<usize> {
    columns
        .visible()
        .position(|(index, _)| index == source_index)
}

fn remap_cell_selection_columns<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    selection: &mut SelectionModel<RowId>,
    range_anchor: &mut Option<CellCoord>,
    old_column_ids: &[ColId],
    columns: &ColumnModel<ColId>,
) {
    let active = selection.cells.active();
    let anchor = selection.cells.anchor();
    let extent = selection.cells.extent();
    let mapped_active = active.and_then(|cell| remap_cell_coord(cell, old_column_ids, columns));
    let mapped_anchor = anchor.and_then(|cell| remap_cell_coord(cell, old_column_ids, columns));
    let mapped_extent = extent.and_then(|cell| remap_cell_coord(cell, old_column_ids, columns));
    let cell_coords_are_valid = active.is_none_or(|_| mapped_active.is_some())
        && anchor.is_none_or(|_| mapped_anchor.is_some())
        && extent.is_none_or(|_| mapped_extent.is_some());
    let mapped_range_anchor = range_anchor
        .as_ref()
        .and_then(|cell| remap_cell_coord(*cell, old_column_ids, columns));

    if !cell_coords_are_valid {
        selection.cells.clear();
        *range_anchor = None;
        return;
    }

    match selection.cells.mode() {
        crate::interaction::CellSelectionMode::None => {}
        crate::interaction::CellSelectionMode::Single => {
            selection.cells.clear();
            if let Some(cell) = mapped_active {
                selection.cells.select_cell(cell);
            }
        }
        crate::interaction::CellSelectionMode::Range => {
            selection.cells.clear();
            if let Some(start) = mapped_anchor.or(mapped_active) {
                selection.cells.select_cell(start);
                if let Some(end) = mapped_extent.or(mapped_active) {
                    selection.cells.extend_to(end);
                }
            }
        }
    }
    *range_anchor = mapped_range_anchor;
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> Default for DataTableState<RowId, ColId> {
    fn default() -> Self {
        Self::new()
    }
}

/// DataTable chrome: toolbar + sticky header + virtual body + optional footer.
#[derive(Debug, Clone)]
pub struct DataTable<'a, RowId, ColId> {
    system: &'a DesignSystem,
    columns: &'a ColumnModel<ColId>,
    /// Projected visible row labels (caller projects cells for the window only).
    rows: &'a [(RowId, &'a [&'a str])],
    /// Optional group headers whose ids appear in the projected stream.
    groups: Option<&'a [GroupHeader<RowId>]>,
    toolbar: Option<&'a DataTableToolbar<'a>>,
    /// Host scene owns keyboard focus on this surface.
    focused: bool,
    /// Request host fullscreen promotion affordance chrome.
    fullscreen_hint: bool,
    /// 1-based row index column (junie grid default).
    row_numbers: bool,
    /// Status footer (`N rows · nav:cell`). Junie showcase tables have none.
    show_footer: bool,
    /// Source DataGrid cell tones (row fill, not quiet numbers).
    datagrid: bool,
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> DataTable<'a, RowId, ColId> {
    /// Columns + visible projected rows.
    #[must_use]
    pub const fn new(
        system: &'a DesignSystem,
        columns: &'a ColumnModel<ColId>,
        rows: &'a [(RowId, &'a [&'a str])],
    ) -> Self {
        Self {
            system,
            columns,
            rows,
            groups: None,
            toolbar: None,
            focused: false,
            fullscreen_hint: false,
            row_numbers: true,
            show_footer: false,
            datagrid: false,
        }
    }

    /// Group headers that match projected row ids (full-width band paint).
    #[must_use]
    pub const fn groups(mut self, groups: &'a [GroupHeader<RowId>]) -> Self {
        self.groups = Some(groups);
        self
    }

    /// Toolbar.
    #[must_use]
    pub const fn toolbar(mut self, toolbar: &'a DataTableToolbar<'a>) -> Self {
        self.toolbar = Some(toolbar);
        self
    }

    /// Scene focus chrome for the table surface.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Show fullscreen promotion hint in footer.
    #[must_use]
    pub const fn fullscreen_hint(mut self, on: bool) -> Self {
        self.fullscreen_hint = on;
        self
    }

    /// 1-based row index column after the change slot.
    #[must_use]
    pub const fn row_numbers(mut self, on: bool) -> Self {
        self.row_numbers = on;
        self
    }

    /// Paint as source DataGrid: cells inherit the row, numbers stay loud.
    #[must_use]
    pub const fn datagrid(mut self, on: bool) -> Self {
        self.datagrid = on;
        self
    }

    /// Status footer (`N rows · nav:cell`). Off matches junie showcase tables.
    #[must_use]
    pub const fn footer(mut self, on: bool) -> Self {
        self.show_footer = on;
        self
    }

    fn chrome_width(&self) -> u16 {
        grid_chrome_width(self.rows.len(), self.row_numbers)
    }

    /// Paint O(visible) rows only.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut DataTableState<RowId, ColId>)
    where
        ColId: Clone,
    {
        state.header_regions.clear();
        state.cell_regions.clear();
        state.paint_rects.clear();
        if area.is_empty() {
            state.paint_widths.clear();
            state.body_origin = (0, 0);
            state.body_rows = 0;
            state.body_width = 0;
            return;
        }
        // Input permission and scene focus are separate authorities. Neither
        // one alone may paint active focus chrome.
        let surface_focused = self.focused && state.accepts_input;
        let has_toolbar = self.toolbar.is_some();
        let has_footer = self.show_footer;
        let chrome_rows = 1u16 // header
            + u16::from(has_toolbar)
            + u16::from(has_footer);
        state.window.viewport = area.height.saturating_sub(chrome_rows).max(1);
        state.window.clamp();
        state.sync_cursor_focus_to_columns(self.columns);

        let mut y = area.y;
        if let Some(tb) = self.toolbar
            && y < area.bottom()
        {
            let line = tb.actions.join(" · ");
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &text,
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Source DataGrid: `width - gutter_w - 4 - scrollbar`.
        // Source DataTable: `width - 5 - scrollbar` (gutter 3 + 2-cell `…`).
        // Row-number grids (lookbook grid-ids 42-wide) keep chrome only:
        // extra trailing drops the customer column the crop still shows.
        let total = usize::try_from(state.window.logical_len.max(self.rows.len() as u64))
            .unwrap_or(self.rows.len())
            .max(self.rows.len());
        let has_sb = crate::scroll::is_scrollable(total, usize::from(state.window.viewport.max(1)));
        let trailing = if self.datagrid {
            4
        } else if self.row_numbers {
            0
        } else {
            2
        };
        let col_budget = area
            .width
            .saturating_sub(self.chrome_width())
            .saturating_sub(trailing)
            .saturating_sub(u16::from(has_sb));
        state.viewport_width = col_budget;
        let layout_budget = col_budget.saturating_add(state.h_offset);
        self.columns.resolve_paint_widths_with_gap(
            layout_budget,
            self.system.spacing.column_gap,
            &mut state.paint_widths,
        );
        if state.ensure_cursor_column_projected(
            self.columns,
            col_budget,
            self.system.spacing.column_gap,
        ) {
            let layout_budget = col_budget.saturating_add(state.h_offset);
            self.columns.resolve_paint_widths_with_gap(
                layout_budget,
                self.system.spacing.column_gap,
                &mut state.paint_widths,
            );
        }
        state.sync_cursor_focus_to_paint(self.columns);
        // Pin bookkeeping
        let mut pin_start = 0usize;
        let mut pin_end = 0usize;
        for &(idx, _) in &state.paint_widths {
            match self.columns.columns[idx].pin {
                ColumnPin::Start => pin_start += 1,
                ColumnPin::End => pin_end += 1,
                ColumnPin::None => {}
            }
        }
        let gap = self.system.spacing.column_gap;
        let start_extent = column_extent(&state.paint_widths, self.columns, ColumnPin::Start, gap);
        let end_extent = column_extent(&state.paint_widths, self.columns, ColumnPin::End, gap);
        let center_extent = column_extent(&state.paint_widths, self.columns, ColumnPin::None, gap);
        let center_viewport = col_budget
            .saturating_sub(start_extent)
            .saturating_sub(end_extent)
            .saturating_sub(if pin_start > 0 && center_extent > 0 {
                gap
            } else {
                0
            })
            .saturating_sub(if pin_end > 0 && center_extent > 0 {
                gap
            } else {
                0
            });
        let max_h = center_extent.saturating_sub(center_viewport);
        // Keep the public viewport width as the full post-chrome budget while
        // normalizing the scroll extent to the unpinned center strip.
        state.content_width = col_budget.saturating_add(max_h);
        state.h_offset = state.h_offset.min(max_h);
        let origin = area.x.saturating_add(self.chrome_width());
        let clip_right = origin
            .saturating_add(state.viewport_width)
            .min(area.right());
        let center_left = origin.saturating_add(start_extent).saturating_add(
            if pin_start > 0 && center_extent > 0 {
                gap
            } else {
                0
            },
        );
        let center_right = clip_right.saturating_sub(end_extent).saturating_sub(
            if pin_end > 0 && center_extent > 0 {
                gap
            } else {
                0
            },
        );
        let _ = state.reveal_cursor_column(self.columns, center_left, center_right, max_h, gap);
        resolve_column_rects(
            &state.paint_widths,
            self.columns,
            origin,
            clip_right,
            state.h_offset,
            gap,
            &mut state.paint_rects,
        );

        // Sticky header
        if y < area.bottom() {
            paint_header_row(self, area, y, buffer, state, surface_focused);
            y = y.saturating_add(1);
        }

        if let Some(chrome) =
            super::data_view::data_load_chrome(&state.load, self.system, state.colorless, "No rows")
        {
            paint_status_line(
                self,
                area,
                y,
                buffer,
                chrome.prefix,
                &chrome.message,
                chrome.role,
            );
            state.body_origin = (area.x, y);
            state.body_rows = 0;
            state.body_width = area.width;
            return;
        }

        state.body_origin = (area.x, y);
        state.body_width = area.width;
        let body_start = y;
        let body_bottom = area.bottom().saturating_sub(u16::from(has_footer));

        for (i, (id, cells)) in self.rows.iter().enumerate() {
            if y >= body_bottom {
                break;
            }
            if let Some(groups) = self.groups
                && let Some(g) = groups.iter().find(|g| &g.id == id)
            {
                paint_group_band(self, area, y, buffer, g);
                y = y.saturating_add(1);
                continue;
            }
            paint_data_row(self, area, y, buffer, state, i, id, cells, surface_focused);
            y = y.saturating_add(1);
        }
        state.body_rows = y.saturating_sub(body_start);
        let body_h = body_bottom.saturating_sub(body_start);
        let total = usize::try_from(state.window.logical_len.max(self.rows.len() as u64))
            .unwrap_or(self.rows.len())
            .max(self.rows.len());
        if body_h > 0 {
            crate::scroll::paint_overflow_scrollbar(
                buffer,
                Rect::new(area.right().saturating_sub(1), body_start, 1, body_h),
                total,
                usize::from(state.window.viewport.max(1)),
                u16::try_from(state.window.offset.min(u64::from(u16::MAX))).unwrap_or(u16::MAX),
                surface_focused,
                self.system,
            );
        }

        // Footer
        if self.show_footer && (y < area.bottom() || body_bottom < area.bottom()) {
            let fy = area.bottom().saturating_sub(1);
            let mut parts = Vec::new();
            match &state.load {
                LoadState::Partial { resident, total } => match total {
                    Some(t) => parts.push(format!("partial {resident}/{t}")),
                    None => parts.push(format!("partial {resident}+")),
                },
                LoadState::Ready { count } => parts.push(format!("{count} rows")),
                _ => {}
            }
            let sel_n = state.selection.selected_rows().len();
            if sel_n > 0 {
                parts.push(format!("{sel_n} selected"));
            }
            if !state.filter.query.is_empty() {
                parts.push(format!("/{}", state.filter.query));
            }
            parts.push(format!("nav:{}", state.nav_mode.id()));
            if self.fullscreen_hint {
                parts.push("C-f full".into());
            }
            if state.editing {
                parts.push(format!("edit:{}", state.edit_draft));
            }
            let footer = parts.join(" · ");
            if !footer.is_empty() {
                let text = take_display_cols(&footer, usize::from(area.width));
                buffer.set_stringn(
                    area.x,
                    fy,
                    &text,
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
            }
        }
    }
}

fn column_extent<ColId>(
    widths: &[(usize, u16)],
    columns: &ColumnModel<ColId>,
    pin: ColumnPin,
    gap: u16,
) -> u16 {
    let mut count = 0usize;
    let mut width = 0u16;
    for &(index, column_width) in widths {
        if columns.columns[index].pin == pin {
            count += 1;
            width = width.saturating_add(column_width);
        }
    }
    width.saturating_add(
        gap.saturating_mul(u16::try_from(count.saturating_sub(1)).unwrap_or(u16::MAX)),
    )
}

/// Resolve one physical rect per projected column.
///
/// Start pins anchor to the left edge, end pins to the right edge, and only
/// unpinned columns consume horizontal scroll. The same rects feed header and
/// body hit regions, so pointer geometry cannot diverge from painted cells.
fn resolve_column_rects<ColId>(
    widths: &[(usize, u16)],
    columns: &ColumnModel<ColId>,
    origin: u16,
    clip_right: u16,
    h_offset: u16,
    gap: u16,
    out: &mut Vec<Rect>,
) {
    out.clear();
    out.resize(widths.len(), Rect::new(0, 0, 0, 0));
    if widths.is_empty() || clip_right <= origin {
        return;
    }

    let start_extent = i64::from(column_extent(widths, columns, ColumnPin::Start, gap));
    let end_extent = i64::from(column_extent(widths, columns, ColumnPin::End, gap));
    let center_extent = i64::from(column_extent(widths, columns, ColumnPin::None, gap));
    let has_start = start_extent > 0;
    let has_end = end_extent > 0;
    let has_center = center_extent > 0;
    let origin = i64::from(origin);
    let clip_right = i64::from(clip_right);
    let gap = i64::from(gap);
    let center_left = origin + start_extent + if has_start && has_center { gap } else { 0 };
    let center_right = clip_right - end_extent - if has_end && has_center { gap } else { 0 };
    let mut start_x = origin;
    let mut end_x = clip_right - end_extent;
    let mut center_offset = 0i64;

    for (ordinal, &(index, width)) in widths.iter().enumerate() {
        let pin = columns.columns[index].pin;
        let left = match pin {
            ColumnPin::Start => {
                let left = start_x;
                start_x += i64::from(width) + gap;
                left
            }
            ColumnPin::End => {
                let left = end_x;
                end_x += i64::from(width) + gap;
                left
            }
            ColumnPin::None => {
                let left = center_left + center_offset - i64::from(h_offset);
                center_offset += i64::from(width) + gap;
                left
            }
        };
        let right = left + i64::from(width);
        let (bounds_left, bounds_right) = match pin {
            ColumnPin::None => (center_left, center_right),
            ColumnPin::Start | ColumnPin::End => (origin, clip_right),
        };
        let visible_left = left.max(bounds_left);
        let visible_right = right.min(bounds_right);
        if visible_right > visible_left {
            out[ordinal] = Rect::new(
                u16::try_from(visible_left).unwrap_or(u16::MAX),
                0,
                u16::try_from(visible_right - visible_left).unwrap_or(u16::MAX),
                1,
            );
        }
    }
}

/// Source DataGrid `fit` / `fit_right`: truncate (tail `…`) then pad.
fn paint_plain_cell(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    text: &str,
    style: Style,
    kind: ColumnKind,
    ellipsis: &str,
) {
    let w = usize::from(width);
    let shown = if kind.clips_instead_of_ellipsizing() {
        take_display_cols(text, w).into_owned()
    } else {
        truncate_cols(text, w, ellipsis).into_owned()
    };
    let shown_w = display_cols(&shown);
    if shown_w == 0 {
        return;
    }
    let paint_x = if kind.right_aligned() {
        x.saturating_add(width.saturating_sub(u16::try_from(shown_w).unwrap_or(width)))
    } else {
        x
    };
    buffer.set_stringn(paint_x, y, &shown, shown_w, style);
}

fn paint_status_line<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    table: &DataTable<'_, RowId, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    glyph: &str,
    message: &str,
    role: Role,
) {
    if y >= area.bottom() {
        return;
    }
    let line = format!("{glyph}{message}");
    buffer.set_stringn(
        area.x,
        y,
        take_display_cols(&line, usize::from(area.width)).as_ref(),
        usize::from(area.width),
        table.system.style(role),
    );
}

fn paint_group_band<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    table: &DataTable<'_, RowId, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    group: &GroupHeader<RowId>,
) {
    let mark = if group.expanded { "▾ " } else { "▸ " };
    let line = format!("{mark}{} ({})", group.label, group.count);
    let style = table
        .system
        .style(Role::TextStrong)
        .add_modifier(Modifier::BOLD);
    buffer.set_stringn(
        area.x,
        y,
        take_display_cols(&line, usize::from(area.width)).as_ref(),
        usize::from(area.width),
        style,
    );
}

fn paint_header_row<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    table: &DataTable<'_, RowId, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    state: &mut DataTableState<RowId, ColId>,
    surface_focused: bool,
) where
    ColId: Clone,
{
    buffer.set_style(
        Rect::new(area.x, y, area.width, 1),
        super::table_chrome::header_band(table.system),
    );
    let chrome = table.chrome_width();
    buffer.set_stringn(
        area.x,
        y,
        &" ".repeat(usize::from(chrome)),
        usize::from(chrome),
        super::table_chrome::header_band(table.system),
    );
    // Apply h_offset only to unpinned center columns. The physical rects are
    // shared with body painting so header hit regions match the cells.
    for (paint_ord, &(col_idx, _width)) in state.paint_widths.iter().enumerate() {
        let paint_rect = state.paint_rects[paint_ord];
        if paint_rect.width == 0 {
            continue;
        }
        let col = &table.columns.columns[col_idx];
        let paint_x = paint_rect.x;
        let paint_w = paint_rect.width;
        let paint_end = paint_rect.right();
        let mut title = col.title.clone();
        let sorted = state.sort.as_ref().is_some_and(|s| s.column == col.id);
        if sorted {
            // Source DataGrid suffix is `" ▴"` / `" ▾"` (leading space).
            if table.datagrid {
                title.push(' ');
            }
            title.push_str(super::table_chrome::sort_marker(
                state.sort.as_ref().is_some_and(|s| s.ascending),
            ));
        }
        let visible_ord = visible_column_ordinal(table.columns, col_idx).unwrap_or(paint_ord);
        let on_cursor = surface_focused && visible_ord == state.cursor_col;
        let col_style = super::table_chrome::header_label_style(
            table.system,
            sorted || on_cursor,
            false,
            col.sortable,
        );
        let cell = Rect::new(paint_x, y, paint_w, 1);
        buffer.set_style(
            cell,
            if on_cursor {
                col_style
            } else {
                super::table_chrome::header_style(table.system)
            },
        );
        if col.primary {
            // junie: title prefix `"▪ "` then overdraw `⚷` at the origin.
            let marked = format!("{} {title}", super::table_chrome::primary_key_mark());
            let text = take_display_cols(&marked, usize::from(paint_w));
            buffer.set_stringn(paint_x, y, &text, usize::from(paint_w), col_style);
            buffer.set_stringn(
                paint_x,
                y,
                super::table_chrome::primary_key_mark(),
                1,
                col_style.fg(table.system.junie_theme().text_faint),
            );
        } else {
            paint_plain_cell(
                buffer,
                paint_x,
                y,
                paint_w,
                &title,
                col_style,
                col.kind,
                table.system.glyphs.ellipsis(),
            );
        }
        let handle_x = paint_end.saturating_sub(RESIZE_HIT);
        state.header_regions.push(DataTableHeaderRegion {
            id: col.id.clone(),
            area: Rect::new(paint_x, y, paint_w.saturating_sub(RESIZE_HIT).max(1), 1),
            resize_handle: Rect::new(handle_x, y, RESIZE_HIT, 1),
            // A column is sortable when the host says so. `|| true` made
            // every column advertise sorting and emit sort requests the host
            // never asked for (plans/021 Step 3).
            sortable: col.sortable,
        });
    }

    paint_clip_chevrons(table, area, y, buffer, state);
}

/// Marks a horizontally clipped header with the direction of what is cut.
///
/// A table scrolled sideways gave no sign that columns existed off-screen —
/// the row simply stopped. The edge cells state it (plans/022 Step 2).
fn paint_clip_chevrons<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    table: &DataTable<'_, RowId, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    state: &DataTableState<RowId, ColId>,
) {
    let style = table.system.junie_theme().faint();
    let ellipsis = table.system.glyphs.ellipsis();
    if state.h_offset > 0 {
        let x = area.x.saturating_add(1);
        if x < area.right() {
            buffer.set_stringn(x, y, ellipsis, 1, style);
        }
    }
    let hidden = table.columns.visible().count() > state.paint_widths.len();
    if hidden || state.content_width.saturating_sub(state.h_offset) > state.viewport_width {
        let x = area
            .x
            .saturating_add(table.chrome_width())
            .saturating_add(state.viewport_width)
            .saturating_add(1);
        if x < area.right() {
            buffer.set_stringn(x, y, ellipsis, 1, style);
        }
    }
}

fn paint_data_row<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    table: &DataTable<'_, RowId, ColId>,
    area: Rect,
    y: u16,
    buffer: &mut Buffer,
    state: &mut DataTableState<RowId, ColId>,
    row_index: usize,
    id: &RowId,
    cells: &[&str],
    surface_focused: bool,
) where
    ColId: Clone,
    RowId: Clone,
{
    let cursor = state.cursor_row == row_index;
    let selected = state.selection.is_row_selected(id);
    let logical_row = state.window.offset.saturating_add(row_index as u64);

    let chrome = super::row_chrome::RowChrome::resolve(
        table.system,
        ListRowVisualState {
            selected,
            focused: cursor && surface_focused,
            hovered: state.hovered_row.as_ref() == Some(id),
            enabled: true,
            loading: false,
            checked: selected,

            ..ListRowVisualState::default()
        },
    )
    .colorless(state.colorless || table.system.mono());
    let base = if state.striped && row_index % 2 == 1 {
        table.system.style(Role::TextMuted)
    } else {
        table.system.style(Role::Text)
    };
    let style = chrome.label_style(base);

    chrome.paint_wash(buffer, Rect::new(area.x, y, area.width, 1));
    if table.datagrid && cursor && surface_focused {
        // Source `t.row` fills the focused row primary+BOLD; pads/gaps inherit.
        for x in area.x..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_style(style);
            }
        }
    }
    let theme = table.system.junie_theme();
    let visual = crate::style::VisualState {
        focused: cursor && surface_focused,
        selected,
        hovered: state.hovered_row.as_ref() == Some(id),
        ..crate::style::VisualState::default()
    };
    // Inherit the row ground (card chrome on `s_grid`, canvas on `t_100`).
    let ground = buffer.cell((area.x, y)).and_then(|c| match c.bg {
        Color::Reset => None,
        other => Some(other),
    });
    let bg = style.bg.or(ground).unwrap_or(theme.surface);
    let gutter_w = table.chrome_width();
    let mut gutter_style = table.system.gutter(visual, bg, false);
    if visual.focused {
        // Source `t.row` puts BOLD on the fill; the bar inherits it.
        gutter_style = gutter_style.add_modifier(Modifier::BOLD);
    }
    buffer.set_stringn(
        area.x,
        y,
        table.system.glyphs.selection_gutter(),
        1,
        gutter_style,
    );
    if gutter_w > 1 {
        let mark = if selected {
            table.system.glyphs.resolve(Glyph::Success).text
        } else {
            " "
        };
        let mark_style = if selected {
            style.fg(if cursor && surface_focused {
                theme.accent
            } else {
                theme.text_secondary
            })
        } else {
            style
        };
        buffer.set_stringn(area.x.saturating_add(1), y, mark, 1, mark_style);
    }
    if gutter_w > 2 {
        buffer.set_stringn(area.x.saturating_add(2), y, " ", 1, style);
    }
    if table.row_numbers && gutter_w > 3 {
        let num_w = gutter_w.saturating_sub(4).max(2);
        let n = logical_row.saturating_add(1);
        let label = format!("{n:>width$}", width = usize::from(num_w));
        let nstyle = style
            .fg(if cursor && surface_focused {
                theme.text_secondary
            } else {
                theme.text_faint
            })
            .remove_modifier(Modifier::BOLD);
        buffer.set_stringn(
            area.x.saturating_add(3),
            y,
            &crate::text::take_display_cols(&label, usize::from(num_w)),
            usize::from(num_w),
            nstyle,
        );
    }

    for (paint_ord, &(col_idx, _width)) in state.paint_widths.iter().enumerate() {
        let paint_rect = state.paint_rects[paint_ord];
        if paint_rect.width == 0 {
            continue;
        }
        let col = &table.columns.columns[col_idx];
        let paint_x = paint_rect.x;
        let paint_w = paint_rect.width;
        // `paint_widths` is a responsive projection, so its ordinal is not
        // necessarily the source cell index when a middle column dropped.
        let cell_text = cells.get(col_idx).copied().unwrap_or("");
        let cell_nav = matches!(
            state.nav_mode,
            DataTableNavMode::Cell | DataTableNavMode::Range
        );
        let visible_ord = visible_column_ordinal(table.columns, col_idx).unwrap_or(paint_ord);
        let cell_focused = cell_nav && cursor && surface_focused && state.cursor_col == visible_ord;
        let cell_selected = state.selection.is_cell_selected(CellCoord {
            row: logical_row,
            col: col_idx,
        });
        let mut cell_style = if table.datagrid {
            if col.primary && !(cursor && surface_focused) {
                style.fg(theme.text_secondary)
            } else {
                style
            }
        } else {
            let quiet = if state.colorless || table.system.mono() {
                style
            } else if matches!(col.kind, ColumnKind::Id) {
                table.system.style(Role::TextSecondary)
            } else {
                chrome.secondary_style(style)
            };
            col.kind.cell_style(style, quiet)
        };
        if cell_selected {
            cell_style = cell_style.patch(table.system.style(Role::SelectionTint));
        }
        if cell_focused {
            // A cell cursor is a cell: the explicit reversal pair.
            cell_style = table.system.reversed();
        }
        let cell = Rect::new(paint_x, y, paint_w, 1);
        if cell_focused
            || cell_selected
            || (!table.datagrid && matches!(col.kind, ColumnKind::Id))
            || (table.datagrid && col.primary && !(cursor && surface_focused))
        {
            buffer.set_style(cell, cell_style);
        }
        if state.editing && cell_focused {
            paint_plain_cell(
                buffer,
                paint_x,
                y,
                paint_w,
                &state.edit_draft,
                cell_style,
                col.kind,
                table.system.glyphs.ellipsis(),
            );
        } else {
            paint_plain_cell(
                buffer,
                paint_x,
                y,
                paint_w,
                cell_text,
                cell_style,
                col.kind,
                table.system.glyphs.ellipsis(),
            );
        }
        state.cell_regions.push(DataTableCellRegion {
            row: id.clone(),
            column: col.id.clone(),
            row_index,
            col_index: visible_ord,
            area: Rect::new(paint_x, y, paint_w, 1),
        });
    }
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> StatefulWidget
    for DataTable<'a, RowId, ColId>
{
    type State = DataTableState<RowId, ColId>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DataTable::paint(&self, area, buffer, state);
    }
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> StatefulWidget
    for &DataTable<'a, RowId, ColId>
{
    type State = DataTableState<RowId, ColId>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DataTable::paint(self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{MouseButton, MouseEvent, MouseEventKind};
    use crate::widgets::data_view::{
        ColumnKind, ColumnPin, DataColumn, DataColumnWidth, LoadState, bench,
    };
    use crate::widgets::tests::click;
    use crate::widgets::tests::mouse;
    use ratatui_core::layout::Position;

    #[test]
    fn select_all_is_request_not_scan() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64, 2, 3];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::SelectAllRequested));
        assert!(state.selection.selected_rows().is_empty());
    }

    #[test]
    fn space_toggles_visible_row_only() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [10u64, 20];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::ToggleRow(10)));
        assert!(state.selection.is_row_selected(&10));
    }

    #[test]
    fn large_projected_set_focus_bounded() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let rows: Vec<u64> = (0..bench::ROWS_10K as u64).collect();
        let visible = &rows[..40];
        let mut state = DataTableState::<u64, &str>::new();
        for _ in 0..100 {
            let _ = state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                visible,
                &cols,
            );
        }
        assert!(state.cursor_row < 40);
    }

    #[test]
    fn million_logical_rows_only_project_window() {
        let mut state = DataTableState::<u64, &str>::new();
        state.set_logical_rows(bench::ROWS_1M);
        state.window.viewport = bench::VIEWPORT_ROWS;
        state.window.clamp();
        let (start, end) = state.window.visible_range();
        assert_eq!(end - start, u64::from(bench::VIEWPORT_ROWS));
        let projected: Vec<u64> = (start..end).collect();
        assert_eq!(projected.len(), usize::from(bench::VIEWPORT_ROWS));
        assert!(state.window.scroll_by(10_000));
        let (s2, e2) = state.window.visible_range();
        assert!(e2 - s2 <= u64::from(bench::VIEWPORT_ROWS));
        assert!(e2 <= bench::ROWS_1M);
    }

    #[test]
    fn sort_toggle_emits_spec() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(4)).priority(100),
            DataColumn::new("b", "B", DataColumnWidth::Min(4)).priority(50),
        ]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            DataTableOutcome::SortSpec(SortSpec {
                column: "a",
                ascending: true
            })
        ));
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            DataTableOutcome::SortSpec(SortSpec {
                column: "a",
                ascending: false
            })
        ));
    }

    #[test]
    fn expand_shift_right() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [7u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::ExpandToggled(7)));
        assert!(state.expand.expanded.contains(&7));
    }

    #[test]
    fn page_down_scrolls_logical_window() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        state.set_logical_rows(10_000);
        state.window.viewport = 20;
        let rows: Vec<u64> = (0..20).collect();
        let out = state.handle_key(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::Scrolled));
        assert!(state.window.offset >= 20);
    }

    #[test]
    fn virtual_window_moves_keep_selection_focus_absolute() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let rows = [0u64, 1];
        let mut state = DataTableState::<u64, &str>::new();
        state.set_logical_rows(100);
        state.window.viewport = 2;
        state.cursor_row = 1;
        state.selection.focus_row = 1;

        assert!(matches!(
            state.handle_intent(UiIntent::Move(NavigationMove::Next), &rows, &cols),
            DataTableOutcome::Scrolled
        ));
        assert_eq!(state.window.offset, 1);
        assert_eq!(state.cursor_row, 1);
        assert_eq!(state.selection.focus_row, 2);

        assert!(matches!(
            state.handle_intent(UiIntent::Move(NavigationMove::First), &rows, &cols),
            DataTableOutcome::CursorMoved
        ));
        assert_eq!(state.selection.focus_row, 1);
        assert!(matches!(
            state.handle_intent(UiIntent::Move(NavigationMove::Last), &rows, &cols),
            DataTableOutcome::CursorMoved
        ));
        assert_eq!(state.selection.focus_row, 2);
    }

    #[test]
    fn render_window_clamp_refreshes_absolute_cursor_focus() {
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let cells: &[&str] = &["row"];
        let rows = [(97u64, cells), (98, cells), (99, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.set_logical_rows(100);
        state.window.viewport = 2;
        state.window.offset = 99;
        state.cursor_row = 2;
        state.selection.focus_row = 2;
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &cols, &rows).render(area, &mut buffer, &mut state);

        assert_eq!(state.window.viewport, 3);
        assert_eq!(state.window.offset, 97);
        assert_eq!(state.selection.focus_row, 99);
    }

    #[test]
    fn zero_visible_columns_reset_cursor_focus_across_visibility_transition() {
        let system = DesignSystem::default();
        let mut cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(8)),
            DataColumn::new("b", "B", DataColumnWidth::Min(8)),
        ]);
        let cells: &[&str] = &["a", "b"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.cursor_col = 1;
        state.selection.focus_col = 1;
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);

        cols.columns
            .iter_mut()
            .for_each(|column| column.visible = false);
        DataTable::new(&system, &cols, &rows).render(area, &mut buffer, &mut state);
        assert_eq!(state.cursor_col, 0);
        assert_eq!(state.selection.focus_col, 0);

        cols.columns[0].visible = true;
        DataTable::new(&system, &cols, &rows).render(area, &mut buffer, &mut state);
        assert_eq!(state.cursor_col, 0);
        assert_eq!(state.selection.focus_col, 0);
    }

    #[test]
    fn narrow_column_contract_keeps_primary() {
        let mut cols = ColumnModel::new(vec![
            DataColumn::new("id", "ID", DataColumnWidth::Fixed(6))
                .priority(100)
                .pin(ColumnPin::Start),
            DataColumn::new("meta", "Meta", DataColumnWidth::Min(12)).priority(10),
            DataColumn::new("extra", "X", DataColumnWidth::Min(8)).priority(5),
        ]);
        cols.contract_to_budget(1, 90);
        let visible: Vec<_> = cols.visible().map(|(_, c)| c.id).collect();
        assert_eq!(visible, vec!["id"]);
    }

    #[test]
    fn retry_on_error_load() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Error {
            message: "fail".into(),
            retryable: true,
        };
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &[],
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::RetryLoad));
    }

    #[test]
    fn cursor_moved_not_focus_changed() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64, 2, 3];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::CursorMoved));
        assert_eq!(state.cursor_row, 1);
        assert_eq!(state.selection.focus_row, 1);
    }

    #[test]
    fn accepts_input_gate() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        state.set_accepts_input(false);
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::Ignored));
    }

    #[test]
    fn visual_focus_requires_scene_focus_and_input_authority() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let system = DesignSystem::junie();
        let columns = ColumnModel::new(vec![DataColumn::new(
            "name",
            "Name",
            DataColumnWidth::Fixed(8),
        )]);
        let cells: &[&str] = &["alpha"];
        let rows = [(1u64, cells)];
        let area = Rect::new(0, 0, 24, 4);

        let mut focused_state = DataTableState::<u64, &str>::new();
        focused_state.set_accepts_input(true);
        let mut focused = Buffer::empty(area);
        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .render(area, &mut focused, &mut focused_state);

        let mut scene_unfocused_state = DataTableState::<u64, &str>::new();
        scene_unfocused_state.set_accepts_input(true);
        let mut scene_unfocused = Buffer::empty(area);
        DataTable::new(&system, &columns, &rows)
            .focused(false)
            .render(area, &mut scene_unfocused, &mut scene_unfocused_state);

        let mut input_disabled_state = DataTableState::<u64, &str>::new();
        input_disabled_state.set_accepts_input(false);
        let mut input_disabled = Buffer::empty(area);
        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .render(area, &mut input_disabled, &mut input_disabled_state);

        assert_ne!(focused.content(), scene_unfocused.content());
        assert_eq!(scene_unfocused.content(), input_disabled.content());
    }

    #[test]
    fn mouse_click_sets_cursor() {
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [10u64, 20, 30];
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        state.body_origin = (0, 2);
        state.body_rows = 3;
        state.body_width = 40;
        let event = click(0, 3);
        let out = state.handle_mouse(event, &rows, &cols);
        assert!(matches!(out, DataTableOutcome::CursorMoved));
        assert_eq!(state.cursor_row, 1);
    }

    #[test]
    fn horizontal_mouse_wheel_scrolls_painted_body() {
        let system = DesignSystem::default();
        let columns = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Fixed(12)),
            DataColumn::new("b", "B", DataColumnWidth::Fixed(12)),
            DataColumn::new("c", "C", DataColumnWidth::Fixed(12)),
        ]);
        let cells: &[&str] = &["a", "b", "c"];
        let rows = [(1u64, cells)];
        let area = Rect::new(0, 0, 24, 4);
        let mut state = DataTableState::<u64, &str>::new();
        state.set_nav_mode(DataTableNavMode::Cell);
        state.cursor_col = 2;
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &columns, &rows)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        assert!(state.content_width > state.viewport_width);
        let initial_offset = state.h_offset;
        assert!(initial_offset > 0);

        let event = mouse(MouseEventKind::ScrollLeft, 1, 1);
        assert!(matches!(
            state.handle_mouse(event, &[1], &columns),
            DataTableOutcome::Scrolled
        ));
        assert!(state.h_offset < initial_offset);

        let event = mouse(MouseEventKind::ScrollRight, 1, 1);
        assert!(matches!(
            state.handle_mouse(event, &[1], &columns),
            DataTableOutcome::Scrolled
        ));
        assert_eq!(state.h_offset, initial_offset);

        let event = mouse(MouseEventKind::ScrollLeft, 1, 1);
        assert!(matches!(
            state.handle_mouse(event, &[1], &columns),
            DataTableOutcome::Scrolled
        ));

        let event = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            position: Position::new(1, 1),
            modifiers: KeyModifiers::SHIFT,
        };
        assert!(matches!(
            state.handle_mouse(event, &[1], &columns),
            DataTableOutcome::Scrolled
        ));
        assert_eq!(state.h_offset, initial_offset);
    }

    #[test]
    fn mouse_ignores_body_after_empty_or_status_paint() {
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let cells: &[&str] = &["row"];
        let rows = [(1u64, cells)];
        let area = Rect::new(0, 0, 24, 4);

        let mut state = DataTableState::<u64, &str>::new();
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows).render(area, &mut buffer, &mut state);
        assert!(state.body_rows > 0);

        let mut empty_buffer = Buffer::empty(Rect::new(0, 0, 0, 0));
        DataTable::new(&system, &cols, &rows).render(
            Rect::new(0, 0, 0, 0),
            &mut empty_buffer,
            &mut state,
        );
        let event = click(1, 2);
        assert!(matches!(
            state.handle_mouse(event, &[1], &cols),
            DataTableOutcome::Ignored
        ));

        state.load = LoadState::Empty {
            message: Some("no rows".into()),
        };
        let mut status_buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows).render(area, &mut status_buffer, &mut state);
        assert_eq!(state.body_rows, 0);
        assert!(matches!(
            state.handle_mouse(event, &[1], &cols),
            DataTableOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_resize_reports_outcome_without_touching_host_model() {
        let cols = ColumnModel::new(vec![DataColumn::new("a", "A", DataColumnWidth::Fixed(8))]);
        let mut state = DataTableState::<u64, &str>::new();
        state.resize_drag = Some(("a", 8, 10));

        let drag = mouse(MouseEventKind::Drag(MouseButton::Left), 14, 0);
        let out = state.handle_mouse(drag, &[], &cols);
        match out {
            DataTableOutcome::ColumnResized { column, width } => {
                assert_eq!(column, "a");
                assert_eq!(width, 12);
            }
            other => panic!("expected resize, got {other:?}"),
        }
        // The state never wrote the override; the host owns that step.
        assert_eq!(cols.effective_width(0), 8);
        assert!(state.resize_drag.is_some());

        let up = mouse(MouseEventKind::Up(MouseButton::Left), 14, 0);
        let out = state.handle_mouse(up, &[], &cols);
        match out {
            DataTableOutcome::ColumnResized { column, width } => {
                assert_eq!(column, "a");
                assert_eq!(width, 12);
            }
            other => panic!("expected resize, got {other:?}"),
        }
        assert!(state.resize_drag.is_none(), "release ends the drag");
        assert_eq!(cols.effective_width(0), 8);
    }

    #[test]
    fn empty_state_paint_has_non_color_glyph() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let rows: [(u64, &[&str]); 0] = [];
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Empty {
            message: Some("no data".into()),
        };
        let table = DataTable::new(&system, &cols, &rows).focused(true);
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
        terminal
            .draw(|f| {
                table.render(f.area(), f.buffer_mut(), &mut state);
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("no data") || text.contains("∅") || text.contains("empty"));
    }

    #[test]
    fn no_focus_changed_variant_name() {
        let src = include_str!("data_table.rs");
        let head = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(src)
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!head.contains("FocusChanged"));
        assert!(head.contains("CursorMoved"));
        assert!(head.contains("cursor_row"));
    }

    #[test]
    fn nav_mode_cycle_and_range_extend() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(4)),
            DataColumn::new("b", "B", DataColumnWidth::Min(4)),
        ]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64, 2, 3];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('\\'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            DataTableOutcome::NavModeChanged(DataTableNavMode::Range)
        ));
        state.set_nav_mode(DataTableNavMode::Range);
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::SelectionChanged));
    }

    #[test]
    fn row_nav_mode_does_not_reverse_cursor_cell() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;
        let tokens = crate::style::DesignSystem::junie();
        let reversed = tokens.reversed();
        let columns = ColumnModel::new(vec![
            DataColumn::new("id", "id", DataColumnWidth::Fixed(9)),
            DataColumn::new("name", "name", DataColumnWidth::Fixed(16)),
        ]);
        let c0: &[&str] = &["1001", "Northwind"];
        let rows = [(0u64, c0)];
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Ready { count: 1 };
        assert!(matches!(state.nav_mode, DataTableNavMode::Row));
        let area = Rect::new(0, 0, 42, 6);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&tokens, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        let reversed_bg = reversed.bg.unwrap();
        assert!(
            buffer.content().iter().all(|cell| cell.bg != reversed_bg),
            "row nav must not reverse a cell"
        );
        state.set_nav_mode(DataTableNavMode::Cell);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&tokens, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        assert!(
            buffer.content().iter().any(|cell| cell.bg == reversed_bg),
            "cell nav reverses the cursor cell"
        );
    }

    #[test]
    fn column_resize_outcome_and_override() {
        let mut cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Fixed(8)).sortable(),
            DataColumn::new("b", "B", DataColumnWidth::Min(6)),
        ]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        match out {
            DataTableOutcome::ColumnResized { column, width } => {
                assert_eq!(column, "a");
                assert!(width >= 8);
                assert!(cols.set_width_override(&column, width));
                assert_eq!(cols.effective_width(0), width);
            }
            other => panic!("expected resize, got {other:?}"),
        }
    }

    #[test]
    fn column_reorder_request() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(4)),
            DataColumn::new("b", "B", DataColumnWidth::Min(4)),
        ]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            DataTableOutcome::ColumnReorderRequested { from: 0, to: 1 }
        ));
    }

    #[test]
    fn edit_never_starts_on_read_only_column() {
        let cols = ColumnModel::new(vec![DataColumn::new("a", "A", DataColumnWidth::Min(4))]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [9u64];
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
                &rows,
                &cols,
            ),
            DataTableOutcome::Ignored
        );
        assert!(!state.editing);
    }

    #[test]
    fn edit_commit_cancel() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(4)).editable(),
        ]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [9u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            DataTableOutcome::EditStarted {
                row: 9,
                column: Some("a")
            }
        ));
        assert!(state.editing);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            &rows,
            &cols,
        );
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            DataTableOutcome::EditCommitted {
                row: 9,
                column: "a",
                text
            } if text == "z"
        ));
        state.editing = true;
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::EditCancelled));
    }

    #[test]
    fn edit_mode_cancels_when_projection_loses_rows_or_columns() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(4)).editable(),
        ]);
        let mut state = DataTableState::<u64, &str>::new();
        state.editing = true;
        state.edit_draft = "stale".into();

        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &[],
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::EditCancelled));
        assert!(!state.editing);
        assert!(state.edit_draft.is_empty());

        let empty_columns = ColumnModel::<&str>::new(Vec::new());
        state.editing = true;
        state.edit_draft = "stale".into();
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &rows,
            &empty_columns,
        );
        assert!(matches!(out, DataTableOutcome::EditCancelled));
        assert!(!state.editing);
        assert!(state.edit_draft.is_empty());
    }

    #[test]
    fn edit_mode_owns_semantic_intents() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Min(4)).editable(),
        ]);
        let rows = [1u64, 2];
        let mut state = DataTableState::<u64, &str>::new();
        state.editing = true;
        state.edit_draft = "draft".into();

        assert!(matches!(
            state.handle_intent(UiIntent::Move(NavigationMove::Next), &rows, &cols),
            DataTableOutcome::Ignored
        ));
        assert_eq!(state.cursor_row, 0);
        assert!(state.selection.selected_rows().is_empty());

        let out = state.handle_intent(UiIntent::Submit, &rows, &cols);
        assert!(matches!(
            out,
            DataTableOutcome::EditCommitted {
                row: 1,
                column: "a",
                text
            } if text == "draft"
        ));
        assert!(!state.editing);

        state.editing = true;
        state.edit_draft = "stale".into();
        state.load = LoadState::Error {
            message: "gone".into(),
            retryable: false,
        };
        assert!(matches!(
            state.handle_intent(UiIntent::Submit, &rows, &cols),
            DataTableOutcome::EditCancelled
        ));
        assert!(state.edit_draft.is_empty());
    }

    #[test]
    fn fullscreen_chord() {
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(4))]);
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
            &rows,
            &cols,
        );
        assert!(matches!(out, DataTableOutcome::FullscreenRequested));
    }

    #[test]
    fn junie_row_chrome_is_bar_check_and_numbers() {
        let system = DesignSystem::junie();
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "id", DataColumnWidth::Fixed(4)).kind(ColumnKind::Id),
        ]);
        let c0: &[&str] = &["1001"];
        let c1: &[&str] = &["1002"];
        let rows = [(0u64, c0), (1u64, c1)];
        let mut state = DataTableState::<u64, &str>::new();
        state.accepts_input = false;
        state.striped = false;
        let area = Rect::new(0, 0, 20, 6);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows)
            .focused(false)
            .row_numbers(true)
            .render(area, &mut buffer, &mut state);
        let text: String = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains(system.glyphs.selection_gutter()),
            "col0 ▎\n{text}"
        );
        assert!(text.contains("1001"), "{text}");
        assert!(text.contains('1'), "row numbers\n{text}");
    }

    #[test]
    fn junie_columns_are_two_cells_apart_and_id_padding_is_quiet() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "id", DataColumnWidth::Fixed(9)).kind(ColumnKind::Id),
            DataColumn::new("customer", "customer", DataColumnWidth::Fixed(25)),
        ]);
        let c0: &[&str] = &["1001", "Northwind Traders"];
        let rows = [(0u64, c0)];
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Ready { count: 1 };
        state.accepts_input = false;
        state.striped = false;
        let area = Rect::new(0, 0, 50, 4);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows)
            .focused(false)
            .row_numbers(true)
            .render(area, &mut buffer, &mut state);
        let chrome = 3 + 2 + 1; // gutter + num_w + pad for 1 row
        let y = 1; // header then body
        assert_eq!(buffer[(chrome, y)].symbol(), "1");
        assert_eq!(buffer[(chrome, y)].fg, theme.text_secondary);
        // padding inside the id column stays quiet, not leftover canvas
        assert_eq!(buffer[(chrome + 4, y)].symbol(), " ");
        assert_eq!(buffer[(chrome + 4, y)].fg, theme.text_secondary);
        let customer_x = chrome + 9 + system.spacing.column_gap;
        assert_eq!(buffer[(customer_x, y)].symbol(), "N");
        assert_eq!(buffer[(customer_x, y)].fg, theme.text_primary);
    }

    #[test]
    fn grid_ids_42_wide_paints_customer_at_column_17() {
        let system = DesignSystem::junie();
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "id", DataColumnWidth::Fixed(9)).kind(ColumnKind::Id),
            DataColumn::new("customer", "customer", DataColumnWidth::Fixed(25)),
        ]);
        let names = [
            "Northwind Traders",
            "Blue Yonder Airlines",
            "Contoso Pharmaceuticals",
            "Fabrikam Robotics",
            "Litware Analytics",
            "Tailspin Toys",
            "Wide World Importers",
        ];
        let id_cells = ["1001", "1002", "1003", "1004", "1005", "1006", "1007"];
        let cell_rows: Vec<[&str; 2]> = id_cells
            .iter()
            .zip(names.iter())
            .map(|(id, name)| [*id, *name])
            .collect();
        let rows: Vec<(u64, &[&str])> = cell_rows
            .iter()
            .enumerate()
            .map(|(i, cells)| (i as u64, cells.as_slice()))
            .collect();
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Ready { count: 7 };
        state.accepts_input = false;
        state.striped = false;
        let area = Rect::new(0, 0, 42, 9);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows)
            .focused(false)
            .row_numbers(true)
            .render(area, &mut buffer, &mut state);
        let row = |y: u16| -> String {
            (0..42u16)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        };
        let cells = |y: u16, start: u16, len: u16| -> String {
            (start..start.saturating_add(len))
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        };
        assert_eq!(
            cells(1, 17, 17),
            "Northwind Traders",
            "customer starts at C17, got {}",
            row(1)
        );
        assert_eq!(
            cells(3, 17, 23),
            "Contoso Pharmaceuticals",
            "clipped remainder still paints the full 23-cell name, got {}",
            row(3)
        );
    }

    #[test]
    fn primary_key_header_overdraws_the_key_mark() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "id", DataColumnWidth::Fixed(9))
                .kind(ColumnKind::Id)
                .primary(),
        ]);
        let c0: &[&str] = &["1001"];
        let rows = [(0u64, c0)];
        let mut state = DataTableState::<u64, &str>::new();
        state.accepts_input = false;
        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows)
            .focused(false)
            .row_numbers(true)
            .render(area, &mut buffer, &mut state);
        let chrome = 3 + 2 + 1;
        assert_eq!(buffer[(chrome, 0)].symbol(), "⚷");
        assert_eq!(buffer[(chrome, 0)].fg, theme.text_faint);
        assert_eq!(buffer[(chrome + 2, 0)].symbol(), "i");
    }

    #[test]
    fn paint_columnar_with_pins_and_cells() {
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "ID", DataColumnWidth::Fixed(4))
                .priority(100)
                .pin(ColumnPin::Start)
                .sortable(),
            DataColumn::new("name", "Name", DataColumnWidth::Min(8)).priority(80),
            DataColumn::new("meta", "Meta", DataColumnWidth::Min(6)).priority(10),
        ]);
        let c0: &[&str] = &["1", "alpha", "x"];
        let c1: &[&str] = &["2", "beta", "y"];
        let rows = [(1u64, c0), (2u64, c1)];
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Ready { count: 2 };
        let table = DataTable::new(&system, &cols, &rows).focused(true);
        let area = Rect::new(0, 0, 40, 8);
        let mut buffer = Buffer::empty(area);
        table.render(area, &mut buffer, &mut state);
        assert!(!state.header_regions.is_empty());
        assert!(!state.cell_regions.is_empty());
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("ID") || text.contains("Name"), "{text}");
        assert!(text.contains("alpha") || text.contains("beta"), "{text}");
    }

    #[test]
    fn pinned_columns_anchor_to_opposite_edges_and_share_hit_geometry() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let system = DesignSystem::default();
        let columns = ColumnModel::new(vec![
            DataColumn::new("start", "Start", DataColumnWidth::Fixed(4)).pin(ColumnPin::Start),
            DataColumn::new("center", "Center", DataColumnWidth::Fixed(6)),
            DataColumn::new("end", "End", DataColumnWidth::Fixed(4)).pin(ColumnPin::End),
        ]);
        let cells: &[&str] = &["S", "C", "E"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.accepts_input = false;
        let area = Rect::new(0, 0, 30, 4);
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &columns, &rows)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);

        let region = |column: &str| {
            state
                .cell_regions
                .iter()
                .find(|region| region.column == column)
                .map(|region| region.area)
                .expect("column is painted")
        };
        assert_eq!(region("start").x, 3);
        assert_eq!(region("center").x, 9);
        assert_eq!(region("end").x, 24);
        assert_eq!(buffer[(region("start").x, 1)].symbol(), "S");
        assert_eq!(buffer[(region("center").x, 1)].symbol(), "C");
        assert_eq!(buffer[(region("end").x, 1)].symbol(), "E");
        assert_eq!(state.header_regions[2].area.x, region("end").x);
    }

    #[test]
    fn dropped_middle_column_does_not_shift_later_cell_values() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let system = DesignSystem::default();
        let columns = ColumnModel::new(vec![
            DataColumn::new("left", "Left", DataColumnWidth::Fixed(4)).priority(100),
            DataColumn::new("dropped", "Dropped", DataColumnWidth::Fixed(12)).priority(1),
            DataColumn::new("right", "Right", DataColumnWidth::Fixed(4)).priority(100),
        ]);
        let cells: &[&str] = &["L", "wrong", "R"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.accepts_input = false;
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &columns, &rows)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);

        assert_eq!(state.paint_widths, vec![(0, 4), (2, 4)]);
        let right = state
            .cell_regions
            .iter()
            .find(|region| region.column == "right")
            .expect("right column remains in the responsive projection");
        assert_eq!(buffer[(right.area.x, right.area.y)].symbol(), "R");
    }

    #[test]
    fn responsive_projection_reanchors_cursor_and_hit_ordinals() {
        let system = DesignSystem::default();
        let columns = ColumnModel::new(vec![
            DataColumn::new("left", "Left", DataColumnWidth::Fixed(4)).priority(100),
            DataColumn::new("dropped", "Dropped", DataColumnWidth::Fixed(12)).priority(1),
            DataColumn::new("right", "Right", DataColumnWidth::Fixed(4)).priority(100),
        ]);
        let cells: &[&str] = &["L", "wrong", "R"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.cursor_col = 1;
        state.selection.focus_col = 1;
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);

        assert_eq!(state.paint_widths, vec![(0, 4), (2, 4)]);
        assert_eq!(state.cursor_col, 0);
        assert_eq!(state.selection.focus_col, 0);
        let right = state
            .cell_regions
            .iter()
            .find(|region| region.column == "right")
            .expect("right cell is painted");
        assert_eq!(right.col_index, 2);
        let out = state.handle_mouse(click(right.area.x, right.area.y), &[1], &columns);
        assert!(matches!(out, DataTableOutcome::CursorMoved));
        assert_eq!(state.cursor_col, 2);
        assert_eq!(state.selection.focus_col, 2);
    }

    #[test]
    fn cell_navigation_reveals_a_column_beyond_the_initial_projection() {
        let system = DesignSystem::default();
        let columns = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Fixed(6)),
            DataColumn::new("b", "B", DataColumnWidth::Fixed(6)),
            DataColumn::new("c", "C", DataColumnWidth::Fixed(6)),
            DataColumn::new("d", "D", DataColumnWidth::Fixed(6)),
        ]);
        let cells: &[&str] = &["A", "B", "C", "D"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.nav_mode = DataTableNavMode::Cell;
        state.cursor_col = 3;
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);

        assert_eq!(state.paint_widths.len(), 4);
        assert!(state.h_offset > 0);
        assert_eq!(state.cursor_col, 3);
        assert_eq!(state.selection.focus_col, 3);
        let focused = state
            .cell_regions
            .iter()
            .find(|region| region.column == "d")
            .expect("focused column is painted");
        assert_eq!(focused.col_index, 3);
        assert!(focused.area.width > 0);
    }

    #[test]
    fn cell_selection_tracks_column_identity_across_visibility_and_reorder() {
        let system = DesignSystem::default();
        let mut columns = ColumnModel::new(vec![
            DataColumn::new("left", "Left", DataColumnWidth::Fixed(6)),
            DataColumn::new("middle", "Middle", DataColumnWidth::Fixed(6)),
            DataColumn::new("right", "Right", DataColumnWidth::Fixed(6)),
        ]);
        let cells: &[&str] = &["L", "M", "R"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.nav_mode = DataTableNavMode::Cell;
        state.selection = SelectionModel::cell();
        let area = Rect::new(0, 0, 40, 4);
        let mut buffer = Buffer::empty(area);

        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        let right = state
            .cell_regions
            .iter()
            .find(|region| region.column == "right")
            .expect("right cell is painted");
        let click = click(right.area.x, right.area.y);
        assert!(matches!(
            state.handle_mouse(click, &[1], &columns),
            DataTableOutcome::CursorMoved
        ));
        assert_eq!(state.selection.cells.active(), Some(CellCoord::new(0, 2)));

        assert!(columns.set_visible(&"middle", false));
        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        assert_eq!(state.selection.cells.active(), Some(CellCoord::new(0, 2)));
        assert!(state.selection.is_cell_selected(CellCoord::new(0, 2)));

        assert!(columns.move_column(2, 0));
        DataTable::new(&system, &columns, &rows)
            .focused(true)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        assert_eq!(state.selection.cells.active(), Some(CellCoord::new(0, 0)));
        assert_eq!(state.selection.focus_col, 0);
    }

    #[test]
    fn header_click_sorts() {
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Fixed(8)).sortable(),
            DataColumn::new("b", "B", DataColumnWidth::Fixed(8)),
        ]);
        let c0: &[&str] = &["1", "2"];
        let rows = [(1u64, c0)];
        let mut state = DataTableState::<u64, &str>::new();
        let table = DataTable::new(&system, &cols, &rows).focused(true);
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        table.render(area, &mut buffer, &mut state);
        let header = state.header_regions[0].area;
        let event = click(header.x, header.y);
        let out = state.handle_mouse(event, &[1u64], &cols);
        assert!(matches!(out, DataTableOutcome::SortSpec(_)));
    }

    #[test]
    fn resolve_paint_widths_deterministic() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Fixed(4)),
            DataColumn::new(
                "b",
                "B",
                DataColumnWidth::Fill(std::num::NonZeroU16::new(1).unwrap()),
            ),
            DataColumn::new("c", "C", DataColumnWidth::Min(6)),
        ]);
        let mut out_a = Vec::new();
        let mut out_b = Vec::new();
        cols.resolve_paint_widths(30, &mut out_a);
        cols.resolve_paint_widths(30, &mut out_b);
        assert_eq!(out_a, out_b);
        assert_eq!(out_a.len(), 3);
    }

    #[test]
    fn group_band_paints() {
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let c0: &[&str] = &["g"];
        let c1: &[&str] = &["row"];
        let rows = [(100u64, c0), (1u64, c1)];
        let groups = [GroupHeader {
            id: 100,
            label: "Cluster A".into(),
            count: 12,
            expanded: true,
        }];
        let mut state = DataTableState::<u64, &str>::new();
        let table = DataTable::new(&system, &cols, &rows)
            .groups(&groups)
            .focused(true);
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        table.render(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Cluster"), "{text}");
    }

    #[test]
    fn numeric_cells_and_headers_right_align() {
        let system = DesignSystem::junie();
        let cols = ColumnModel::new(vec![
            DataColumn::new("name", "Name", DataColumnWidth::Fixed(8)),
            DataColumn::new("n", "N", DataColumnWidth::Fixed(4)).kind(ColumnKind::Numeric),
        ]);
        let cells: &[&str] = &["ab", "9"];
        let rows = [(1u64, cells)];
        let mut state = DataTableState::<u64, &str>::new();
        state.set_accepts_input(false);
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        let chrome = 3u16;
        let num_x = chrome + 8 + 2;
        let header: String = (num_x..num_x + 4)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        let numeric: String = (num_x..num_x + 4)
            .map(|x| buffer[(x, 1)].symbol().to_string())
            .collect();
        assert_eq!(header, "   N", "{header}");
        assert_eq!(numeric, "   9", "{numeric}");
        assert_eq!(buffer[(chrome, 1)].symbol(), "a");
    }

    #[test]
    fn overflow_gutter_paints_line_thumb_on_body_not_header() {
        let system = DesignSystem::junie();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        let c0: &[&str] = &["a"];
        let rows: Vec<(u64, &[&str])> = (0..20).map(|i| (i, c0)).collect();
        let mut state = DataTableState::<u64, &str>::new();
        state.set_logical_rows(20);
        state.set_accepts_input(false);
        let area = Rect::new(0, 0, 20, 6);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows)
            .row_numbers(false)
            .render(area, &mut buffer, &mut state);
        assert_ne!(
            buffer[(19, 0)].symbol(),
            "┃",
            "header must not wear the body thumb"
        );
        assert_eq!(buffer[(19, 1)].symbol(), "┃");
        assert_eq!(buffer[(19, 5)].symbol(), "│");
    }

    #[test]
    fn numeric_columns_read_quieter_than_text_columns() {
        let system = DesignSystem::default();
        let cols = ColumnModel::new(vec![
            DataColumn::new("name", "Name", DataColumnWidth::Min(8)).priority(100),
            DataColumn::new("size", "Size", DataColumnWidth::Fixed(6))
                .priority(50)
                .kind(ColumnKind::Numeric),
        ]);
        let c0: &[&str] = &["deploy", "1024"];
        let rows = [(1u64, c0)];
        let mut state = DataTableState::<u64, &str>::new();
        state.load = LoadState::Ready { count: 1 };
        state.set_accepts_input(false);
        let area = Rect::new(0, 0, 30, 6);
        let mut buffer = Buffer::empty(area);
        DataTable::new(&system, &cols, &rows).render(area, &mut buffer, &mut state);

        let row_y = (0..area.height)
            .find(|y| (0..area.width).any(|x| buffer[(x, *y)].symbol().starts_with('d')))
            .expect("the data row must be painted");
        let at = |needle: char| {
            let x = (0..area.width)
                .find(|x| buffer[(*x, row_y)].symbol().starts_with(needle))
                .unwrap_or_else(|| panic!("{needle:?} must be painted"));
            buffer[(x, row_y)].style().fg
        };
        assert_ne!(
            at('d'),
            at('2'),
            "a count must not read as loudly as the identity beside it"
        );
        assert_eq!(at('2'), system.style(Role::TextMuted).fg);
    }

    #[test]
    fn selected_row_copy_stays_visible_in_named_and_no_color_profiles() {
        let render = |system: &DesignSystem| {
            let cols = ColumnModel::new(vec![
                DataColumn::new("name", "Name", DataColumnWidth::Fixed(8)),
                DataColumn::new("count", "Count", DataColumnWidth::Fixed(6))
                    .kind(ColumnKind::Numeric),
            ]);
            let cells: &[&str] = &["alpha", "42"];
            let rows = [(1u64, cells)];
            let mut state = DataTableState::<u64, &str>::new();
            state.set_nav_mode(DataTableNavMode::Cell);
            state.selection.select_row(1);
            let area = Rect::new(0, 0, 24, 4);
            let mut buffer = Buffer::empty(area);

            DataTable::new(system, &cols, &rows).focused(true).render(
                area,
                &mut buffer,
                &mut state,
            );
            (buffer, state)
        };

        let junie = DesignSystem::junie();
        let (buffer, state) = render(&junie);
        let row_y = state.body_origin.1;
        let label_x = (0..buffer.area.width)
            .find(|x| buffer[(*x, row_y)].symbol() == "l")
            .expect("selected label copy must remain painted");
        let number_x = (0..buffer.area.width)
            .find(|x| buffer[(*x, row_y)].symbol() == "4")
            .expect("selected numeric copy must remain painted");
        let label = &buffer[(label_x, row_y)];
        let number = &buffer[(number_x, row_y)];
        // The keyboard's cell cursor is the explicit reversal pair; the rest
        // of the selected row keeps the tint and its own copy tone.
        assert_eq!(label.fg, junie.junie_theme().canvas);
        assert_eq!(label.bg, junie.junie_theme().text_primary);
        assert_eq!(number.bg, junie.style(Role::SelectionTint).bg.unwrap());
        assert_ne!(number.fg, number.bg);
        assert!(label.modifier.contains(Modifier::BOLD));
        assert!(!number.modifier.contains(Modifier::BOLD));

        let no_color = DesignSystem::junie().no_color();
        let (buffer, state) = render(&no_color);
        let row_y = state.body_origin.1;
        let label_x = (0..buffer.area.width)
            .find(|x| buffer[(*x, row_y)].symbol() == "l")
            .expect("ASCII/no-color selected label must remain painted");
        let label = &buffer[(label_x, row_y)];
        // A colourless terminal keeps the pair as named colours: the copy
        // stays readable and the row never wears the tint.
        assert!(label.modifier.contains(Modifier::BOLD));
        assert_ne!(label.fg, label.bg, "the copy stays readable");
        assert_ne!(
            label.bg,
            no_color.style(Role::SelectionTint).bg.unwrap(),
            "the cursor pair is not the tint"
        );
        assert_eq!(
            buffer[(buffer.area.x, row_y)].symbol(),
            no_color.glyphs.selection_gutter()
        );
    }

    #[test]
    fn layout_fuzz_widths() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("a", "A", DataColumnWidth::Fixed(10)).priority(100),
            DataColumn::new("b", "B", DataColumnWidth::Min(5)).priority(20),
            DataColumn::new(
                "c",
                "C",
                DataColumnWidth::Fill(std::num::NonZeroU16::new(2).unwrap()),
            )
            .priority(50),
        ]);
        let mut out = Vec::new();
        for budget in 0..=60 {
            cols.resolve_paint_widths(budget, &mut out);
            let sum: u16 = out.iter().map(|(_, w)| *w).sum();
            if budget > 0 && !out.is_empty() {
                assert!(sum > 0);
            }
        }
    }
}
