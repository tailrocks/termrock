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
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Role, SelectionChrome},
    text::take_display_cols,
    widgets::data_view::{
        CellCoord, ColumnModel, ColumnPin, CopyPayload, DataDensity, ExpandState, FilterSpec,
        GroupHeader, LoadState, SelectionMode, SelectionModel, SortSpec, VirtualWindow,
    },
};

const GUTTER_W: u16 = 2;
const SEP: &str = "│";
const RESIZE_HIT: u16 = 1;

/// Keyboard navigation mode (VisiData-like layers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DataTableNavMode {
    /// Arrow keys move row cursor; Left/Right move column or h-scroll.
    #[default]
    Cell,
    /// Primary axis is rows; horizontal keys page columns only.
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
    /// Row chrome density.
    pub density: DataDensity,
    /// Expand detail rows.
    pub expand: ExpandState<RowId>,
    /// Active sort (chrome marker; consumer applies).
    pub sort: Option<SortSpec<ColId>>,
    /// Active filter (chrome; consumer applies).
    pub filter: FilterSpec,
    /// Stripes.
    pub striped: bool,
    /// ASCII sort markers / gutters.
    pub ascii: bool,
    /// Suppress chromatic roles (Text / TextMuted / TextStrong only).
    pub colorless: bool,
    /// Host grants keyboard/pointer input to this surface (scene-focused).
    pub accepts_input: bool,
    /// Whether an inline edit session is open (host owns text buffer).
    pub editing: bool,
    /// Pending edit draft (host may mirror).
    pub edit_draft: String,
    /// Sticky first N paint columns are pin-start (resolved each frame).
    pub pin_start_count: usize,
    /// Sticky last N paint columns are pin-end.
    pub pin_end_count: usize,
    /// Header hit regions from last paint.
    pub header_regions: Vec<DataTableHeaderRegion<ColId>>,
    /// Body cell hit regions from last paint.
    pub cell_regions: Vec<DataTableCellRegion<RowId, ColId>>,
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
    /// Logical column count for h-scroll max.
    content_width: u16,
    /// Viewport width for columns (area − gutter).
    viewport_width: u16,
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> DataTableState<RowId, ColId> {
    /// Fresh multi-select table in cell navigation mode.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selection: SelectionModel::multi_row(),
            window: VirtualWindow::default(),
            h_offset: 0,
            cursor_row: 0,
            cursor_col: 0,
            nav_mode: DataTableNavMode::Cell,
            load: LoadState::Ready { count: 0 },
            density: DataDensity::Comfortable,
            expand: ExpandState::default(),
            sort: None,
            filter: FilterSpec::default(),
            striped: true,
            ascii: false,
            colorless: false,
            accepts_input: true,
            editing: false,
            edit_draft: String::new(),
            pin_start_count: 0,
            pin_end_count: 0,
            header_regions: Vec::new(),
            cell_regions: Vec::new(),
            resize_drag: None,
            range_anchor: None,
            body_origin: (0, 0),
            body_rows: 0,
            body_width: 0,
            paint_widths: Vec::new(),
            content_width: 0,
            viewport_width: 0,
        }
    }

    /// Configure logical universe size (e.g. 1_000_000) without allocating rows.
    pub fn set_logical_rows(&mut self, logical_len: u64) {
        self.window.logical_len = logical_len;
        self.window.clamp();
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
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return DataTableOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        if self.editing {
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

        let vis_n = columns.visible().count();
        if vis_n > 0 {
            self.cursor_col = self.cursor_col.min(vis_n - 1);
        }

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
            KeyCode::Left | KeyCode::Char('h') => self.move_horizontal(visible_rows, columns, -1),
            KeyCode::Right | KeyCode::Char('l') => self.move_horizontal(visible_rows, columns, 1),
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
                let editable = col.as_ref().is_some_and(|id| {
                    columns
                        .columns
                        .iter()
                        .find(|c| &c.id == id)
                        .is_some_and(|c| c.editable)
                });
                if !editable && col.is_some() {
                    // Still emit; host may allow all columns.
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
        if key.kind != KeyEventKind::Press {
            return DataTableOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                self.editing = false;
                self.edit_draft.clear();
                DataTableOutcome::EditCancelled
            }
            KeyCode::Enter => {
                let Some(col) = self.cursor_column_id(columns) else {
                    self.editing = false;
                    return DataTableOutcome::EditCancelled;
                };
                let text = std::mem::take(&mut self.edit_draft);
                self.editing = false;
                DataTableOutcome::EditCommitted {
                    row: visible_rows[self.cursor_row.min(visible_rows.len().saturating_sub(1))]
                        .clone(),
                    column: col,
                    text,
                }
            }
            KeyCode::Backspace => {
                self.edit_draft.pop();
                DataTableOutcome::Ignored
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.edit_draft.push(ch);
                DataTableOutcome::Ignored
            }
            _ => DataTableOutcome::Ignored,
        }
    }

    fn cursor_column_id(&self, columns: &ColumnModel<ColId>) -> Option<ColId>
    where
        ColId: Clone,
    {
        columns
            .visible()
            .nth(self.cursor_col)
            .map(|(_, c)| c.id.clone())
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
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
        delta: i16,
    ) -> DataTableOutcome<RowId, ColId>
    where
        ColId: Clone,
    {
        let _ = visible_rows;
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
            let vis_n = columns.visible().count().max(1);
            if d_row != 0 {
                let _ = self.move_cursor_row(i64::from(d_row), visible_rows.len());
            }
            if d_col != 0 {
                let next = if d_col < 0 {
                    self.cursor_col.saturating_sub(1)
                } else {
                    (self.cursor_col + 1).min(vis_n - 1)
                };
                self.cursor_col = next;
            }
            let cell = CellCoord {
                row: self.window.offset.saturating_add(self.cursor_row as u64),
                col: self.cursor_col,
            };
            if self.range_anchor.is_none() {
                self.range_anchor = Some(CellCoord {
                    row: self.window.offset.saturating_add(self.cursor_row as u64),
                    col: self.cursor_col,
                });
                self.selection.select_cell(cell);
            } else {
                self.selection.extend_cell(cell);
            }
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
        let vis_n = columns.visible().count();
        if vis_n > 0 {
            self.cursor_col = self.cursor_col.min(vis_n - 1);
        }
        match intent {
            UiIntent::Move(NavigationMove::Next) | UiIntent::Move(NavigationMove::Down) => {
                self.move_cursor_row(1, visible_rows.len())
            }
            UiIntent::Move(NavigationMove::Previous) | UiIntent::Move(NavigationMove::Up) => {
                self.move_cursor_row(-1, visible_rows.len())
            }
            UiIntent::Move(NavigationMove::First) => {
                self.cursor_row = 0;
                DataTableOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.cursor_row = visible_rows.len().saturating_sub(1);
                DataTableOutcome::CursorMoved
            }
            UiIntent::Move(NavigationMove::Left) => self.move_horizontal(visible_rows, columns, -1),
            UiIntent::Move(NavigationMove::Right) => self.move_horizontal(visible_rows, columns, 1),
            UiIntent::Page(PageMove::Forward) => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(step) {
                    DataTableOutcome::Scrolled
                } else {
                    self.move_cursor_row(step, visible_rows.len())
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(-step) {
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
                if self.editing {
                    self.editing = false;
                    self.edit_draft.clear();
                    return DataTableOutcome::EditCancelled;
                }
                self.selection.clear_selection();
                self.range_anchor = None;
                DataTableOutcome::SelectionChanged
            }
            UiIntent::Open | UiIntent::Close => DataTableOutcome::Ignored,
            _ => DataTableOutcome::Ignored,
        }
    }

    /// Mouse: wheel, click cursor/select, header sort, resize drag, context.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        visible_rows: &[RowId],
        columns: &mut ColumnModel<ColId>,
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
            width: self.body_width.max(1),
            height: self.body_rows.max(1),
        };

        // Resize drag in progress
        if let Some((ref col_id, start_w, start_x)) = self.resize_drag.clone() {
            match event.kind {
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Moved => {
                    let dx = event.position.x as i32 - start_x as i32;
                    let next = (start_w as i32 + dx).clamp(2, 80) as u16;
                    let _ = columns.set_width_override(&col_id, next);
                    return DataTableOutcome::ColumnResized {
                        column: col_id.clone(),
                        width: next,
                    };
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    let width = columns
                        .index_of(&col_id)
                        .map(|i| columns.effective_width(i))
                        .unwrap_or(start_w);
                    self.resize_drag = None;
                    return DataTableOutcome::ColumnResized {
                        column: col_id.clone(),
                        width,
                    };
                }
                _ => {}
            }
        }

        match event.kind {
            MouseEventKind::ScrollUp if body.contains(event.position) => {
                if self.window.scroll_by(-1) {
                    DataTableOutcome::Scrolled
                } else if !visible_rows.is_empty() {
                    self.move_cursor_row(-1, visible_rows.len())
                } else {
                    DataTableOutcome::Ignored
                }
            }
            MouseEventKind::ScrollDown if body.contains(event.position) => {
                if self.window.scroll_by(1) {
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
                    self.cursor_row = cell.row_index;
                    self.cursor_col = cell.col_index;
                    if event.modifiers.contains(KeyModifiers::SHIFT)
                        || matches!(self.nav_mode, DataTableNavMode::Range)
                    {
                        let coord = CellCoord {
                            row: self.window.offset.saturating_add(cell.row_index as u64),
                            col: cell.col_index,
                        };
                        if self.range_anchor.is_none() {
                            self.range_anchor = Some(coord);
                            self.selection.select_cell(coord);
                        } else {
                            self.selection.extend_cell(coord);
                        }
                        return DataTableOutcome::SelectionChanged;
                    }
                    if matches!(
                        self.selection.mode,
                        SelectionMode::Cell | SelectionMode::CellRange
                    ) {
                        self.selection.select_cell(CellCoord {
                            row: self.window.offset.saturating_add(cell.row_index as u64),
                            col: cell.col_index,
                        });
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
                        return DataTableOutcome::CursorMoved;
                    }
                }
                DataTableOutcome::Ignored
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(cell) = self.hit_cell(event.position) {
                    self.cursor_row = cell.row_index;
                    self.cursor_col = cell.col_index;
                    let coord = CellCoord {
                        row: self.window.offset.saturating_add(cell.row_index as u64),
                        col: cell.col_index,
                    };
                    if self.range_anchor.is_none() {
                        self.range_anchor = Some(coord);
                        self.selection.select_cell(coord);
                    } else {
                        self.selection.extend_cell(coord);
                    }
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
                return DataTableOutcome::Scrolled;
            }
            if delta < 0 && self.window.scroll_by(-1) {
                return DataTableOutcome::Scrolled;
            }
            return DataTableOutcome::Ignored;
        }
        self.cursor_row = next;
        self.selection.focus_row = self.window.offset.saturating_add(next as u64);
        DataTableOutcome::CursorMoved
    }
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> Default for DataTableState<RowId, ColId> {
    fn default() -> Self {
        Self::new()
    }
}

/// DataTable chrome: toolbar + sticky header + virtual body + footer.
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

    /// Paint O(visible) rows only.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut DataTableState<RowId, ColId>)
    where
        ColId: Clone,
    {
        state.header_regions.clear();
        state.cell_regions.clear();
        if area.is_empty() {
            return;
        }
        let surface_focused = self.focused || state.accepts_input;
        let has_toolbar = self.toolbar.is_some();
        let has_footer = true;
        let chrome_rows = 1u16 // header
            + u16::from(has_toolbar)
            + u16::from(has_footer);
        state.window.viewport = area.height.saturating_sub(chrome_rows).max(1);
        state.window.clamp();

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

        let col_budget = area.width.saturating_sub(GUTTER_W);
        state.viewport_width = col_budget;
        self.columns.resolve_paint_widths(
            col_budget.saturating_add(state.h_offset),
            &mut state.paint_widths,
        );
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
        state.pin_start_count = pin_start;
        state.pin_end_count = pin_end;
        state.content_width = state
            .paint_widths
            .iter()
            .map(|(_, w)| *w)
            .fold(0u16, u16::saturating_add)
            .saturating_add(u16::try_from(state.paint_widths.len().saturating_sub(1)).unwrap_or(0));
        let max_h = state.content_width.saturating_sub(col_budget);
        state.h_offset = state.h_offset.min(max_h);

        // Sticky header
        if y < area.bottom() {
            paint_header_row(self, area, y, buffer, state, surface_focused);
            y = y.saturating_add(1);
        }

        match &state.load {
            LoadState::Empty { message } => {
                paint_status_line(
                    self,
                    area,
                    y,
                    buffer,
                    if state.ascii { "[ ] " } else { "∅ " },
                    message.as_deref().unwrap_or("(empty)"),
                    Role::TextMuted,
                );
                state.body_origin = (area.x, y);
                state.body_rows = 0;
                state.body_width = area.width;
                return;
            }
            LoadState::Loading { message } => {
                paint_status_line(
                    self,
                    area,
                    y,
                    buffer,
                    if state.ascii { "... " } else { "… " },
                    message.as_deref().unwrap_or("Loading…"),
                    Role::TextMuted,
                );
                state.body_origin = (area.x, y);
                state.body_rows = 0;
                state.body_width = area.width;
                return;
            }
            LoadState::Error { message, .. } => {
                let role = if state.colorless {
                    Role::TextStrong
                } else {
                    Role::Danger
                };
                paint_status_line(
                    self,
                    area,
                    y,
                    buffer,
                    if state.ascii { "! " } else { "✗ " },
                    &format!("{message}  (r retry)"),
                    role,
                );
                state.body_origin = (area.x, y);
                state.body_rows = 0;
                state.body_width = area.width;
                return;
            }
            _ => {}
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
                paint_group_band(self, area, y, buffer, g, state);
                y = y.saturating_add(1);
                continue;
            }
            paint_data_row(self, area, y, buffer, state, i, id, cells, surface_focused);
            y = y.saturating_add(1);
        }
        state.body_rows = y.saturating_sub(body_start);

        // Footer
        if y < area.bottom() || body_bottom < area.bottom() {
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
        &take_display_cols(&line, usize::from(area.width)),
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
    state: &DataTableState<RowId, ColId>,
) {
    let mark = if group.expanded {
        if state.ascii { "v " } else { "▾ " }
    } else if state.ascii {
        "> "
    } else {
        "▸ "
    };
    let line = format!("{mark}{} ({})", group.label, group.count);
    let style = table
        .system
        .style(Role::TextStrong)
        .add_modifier(Modifier::BOLD);
    buffer.set_stringn(
        area.x,
        y,
        &take_display_cols(&line, usize::from(area.width)),
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
    let style = if surface_focused {
        table.system.style(Role::TextStrong)
    } else {
        table.system.style(Role::TextMuted)
    };
    buffer.set_stringn(area.x, y, "  ", usize::from(GUTTER_W), style);
    let origin = area.x.saturating_add(GUTTER_W);
    let clip_right = area.right();
    let mut x = origin;
    // Apply h_offset only to unpinned center columns; pin start paints first.
    let widths = &state.paint_widths;
    let pin_start_w: u16 = widths
        .iter()
        .filter(|(i, _)| table.columns.columns[*i].pin == ColumnPin::Start)
        .map(|(_, w)| *w + 1)
        .sum::<u16>()
        .saturating_sub(u16::from(
            widths
                .iter()
                .any(|(i, _)| table.columns.columns[*i].pin == ColumnPin::Start),
        ));
    let _ = pin_start_w;

    let mut logical = 0i32;
    let h_off = i32::from(state.h_offset);
    for (paint_ord, &(col_idx, width)) in widths.iter().enumerate() {
        let col = &table.columns.columns[col_idx];
        let pinned_start = col.pin == ColumnPin::Start;
        let pinned_end = col.pin == ColumnPin::End;
        let skip_scroll = pinned_start || pinned_end;
        let col_left = if skip_scroll {
            // Pins: place at current x without h_offset
            i32::from(x)
        } else {
            i32::from(origin) + logical - h_off
        };
        let col_right = col_left + i32::from(width);
        if !skip_scroll {
            logical += i32::from(width) + 1;
        }
        if col_right <= i32::from(origin) || col_left >= i32::from(clip_right) {
            if skip_scroll {
                x = (col_right as u16).min(clip_right);
            }
            continue;
        }
        let paint_x = col_left.max(i32::from(origin)) as u16;
        let paint_end = col_right.min(i32::from(clip_right)) as u16;
        let paint_w = paint_end.saturating_sub(paint_x);
        if paint_w == 0 {
            continue;
        }
        let mut title = col.title.clone();
        if let Some(sort) = &state.sort
            && sort.column == col.id
        {
            let mark = if state.ascii {
                if sort.ascending { "^" } else { "v" }
            } else if sort.ascending {
                "▲"
            } else {
                "▼"
            };
            title.push_str(mark);
        }
        let text = take_display_cols(&title, usize::from(paint_w));
        buffer.set_stringn(paint_x, y, &text, usize::from(paint_w), style);
        let handle_x = paint_end.saturating_sub(RESIZE_HIT);
        state.header_regions.push(DataTableHeaderRegion {
            id: col.id.clone(),
            area: Rect::new(paint_x, y, paint_w.saturating_sub(RESIZE_HIT).max(1), 1),
            resize_handle: Rect::new(handle_x, y, RESIZE_HIT, 1),
            sortable: col.sortable || true, // allow sort chrome by default for pro tables
        });
        // Separator
        if paint_end < clip_right && paint_ord + 1 < widths.len() {
            buffer.set_stringn(
                paint_end.min(clip_right.saturating_sub(1)),
                y,
                SEP,
                1,
                table.system.style(Role::Border),
            );
        }
        if skip_scroll {
            x = paint_end.saturating_add(1);
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
    let expanded = state.expand.expanded.contains(id);
    let logical_row = state.window.offset.saturating_add(row_index as u64);

    let style = if state.colorless {
        if selected || (cursor && surface_focused) {
            table.system.style(Role::TextStrong)
        } else {
            table.system.style(Role::Text)
        }
    } else if selected {
        match table.system.selection {
            SelectionChrome::Fill => table.system.style(Role::Selection),
            SelectionChrome::Tint => table.system.style(Role::Focus),
            SelectionChrome::Gutter => {
                if cursor && surface_focused {
                    table.system.style(Role::TextStrong)
                } else {
                    table.system.style(Role::Text)
                }
            }
        }
    } else if cursor && surface_focused {
        table.system.style(Role::Focus)
    } else if state.striped && row_index % 2 == 1 {
        table.system.style(Role::TextMuted)
    } else {
        table.system.style(Role::Text)
    };

    let gutter = if selected {
        if state.ascii { "*" } else { "▌" }
    } else if cursor && surface_focused {
        if state.ascii { ">" } else { "›" }
    } else if expanded {
        if state.ascii { "v" } else { "▾" }
    } else {
        " "
    };
    let gstyle = if selected {
        table.system.style(Role::Accent)
    } else {
        style
    };
    buffer.set_stringn(area.x, y, gutter, 1, gstyle);
    buffer.set_stringn(area.x.saturating_add(1), y, " ", 1, style);

    let origin = area.x.saturating_add(GUTTER_W);
    let clip_right = area.right();
    let h_off = i32::from(state.h_offset);
    let mut logical = 0i32;
    let mut x_pin = origin;

    for (paint_ord, &(col_idx, width)) in state.paint_widths.iter().enumerate() {
        let col = &table.columns.columns[col_idx];
        let pinned_start = col.pin == ColumnPin::Start;
        let pinned_end = col.pin == ColumnPin::End;
        let skip_scroll = pinned_start || pinned_end;
        let col_left = if skip_scroll {
            i32::from(x_pin)
        } else {
            i32::from(origin) + logical - h_off
        };
        let col_right = col_left + i32::from(width);
        if !skip_scroll {
            logical += i32::from(width) + 1;
        }
        if col_right <= i32::from(origin) || col_left >= i32::from(clip_right) {
            if skip_scroll {
                x_pin = (col_right as u16).min(clip_right);
            }
            continue;
        }
        let paint_x = col_left.max(i32::from(origin)) as u16;
        let paint_end = col_right.min(i32::from(clip_right)) as u16;
        let paint_w = paint_end.saturating_sub(paint_x);
        if paint_w == 0 {
            continue;
        }
        let cell_text = cells.get(paint_ord).copied().unwrap_or("");
        let cell_focused = cursor && surface_focused && state.cursor_col == paint_ord;
        let cell_selected = state.selection.is_cell_selected(CellCoord {
            row: logical_row,
            col: paint_ord,
        });
        let mut cell_style = style;
        if cell_selected {
            cell_style = table.system.style(Role::Selection);
        }
        if cell_focused {
            cell_style = cell_style.add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
        }
        if state.editing && cell_focused {
            let draft = take_display_cols(&state.edit_draft, usize::from(paint_w));
            buffer.set_stringn(paint_x, y, &draft, usize::from(paint_w), cell_style);
        } else {
            let text = take_display_cols(cell_text, usize::from(paint_w));
            buffer.set_stringn(paint_x, y, &text, usize::from(paint_w), cell_style);
        }
        state.cell_regions.push(DataTableCellRegion {
            row: id.clone(),
            column: col.id.clone(),
            row_index,
            col_index: paint_ord,
            area: Rect::new(paint_x, y, paint_w, 1),
        });
        if paint_end < clip_right && paint_ord + 1 < state.paint_widths.len() {
            buffer.set_stringn(
                paint_end.min(clip_right.saturating_sub(1)),
                y,
                SEP,
                1,
                table.system.style(Role::Border),
            );
        }
        if skip_scroll {
            x_pin = paint_end.saturating_add(1);
        }
    }
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> StatefulWidget
    for DataTable<'a, RowId, ColId>
{
    type State = DataTableState<RowId, ColId>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DataTable::render(&self, area, buffer, state);
    }
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> StatefulWidget
    for &DataTable<'a, RowId, ColId>
{
    type State = DataTableState<RowId, ColId>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DataTable::render(self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{MouseButton, MouseEvent, MouseEventKind};
    use crate::widgets::data_view::{ColumnPin, DataColumn, DataColumnWidth, LoadState, bench};
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
    fn mouse_click_sets_cursor() {
        let mut state = DataTableState::<u64, &str>::new();
        let rows = [10u64, 20, 30];
        let mut cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(8))]);
        state.body_origin = (0, 2);
        state.body_rows = 3;
        state.body_width = 40;
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 0, y: 3 },
            modifiers: KeyModifiers::NONE,
        };
        let out = state.handle_mouse(event, &rows, &mut cols);
        assert!(matches!(out, DataTableOutcome::CursorMoved));
        assert_eq!(state.cursor_row, 1);
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
            DataTableOutcome::NavModeChanged(DataTableNavMode::Row)
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
    fn header_click_sorts() {
        let system = DesignSystem::default();
        let mut cols = ColumnModel::new(vec![
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
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position {
                x: header.x,
                y: header.y,
            },
            modifiers: KeyModifiers::NONE,
        };
        let out = state.handle_mouse(event, &[1u64], &mut cols);
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
