// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Shared **data presentation** abstractions for tables, grids, logs, and
//! inspectors.
//!
//! These types deliberately do **not** form one mega-trait. Components pick the
//! pieces they need:
//!
//! - [`SelectionModel`] — row / cell / multi selection
//! - [`ColumnModel`] — width, pin, visibility, responsive priority
//! - [`VirtualWindow`] — offset + viewport for O(visible) paint
//! - [`LoadState`] — empty / loading / partial / error / ready
//! - [`DataDensity`] — compact vs comfortable row chrome
//! - [`CopyPayload`] — cell/range copy requests (consumer writes clipboard)
//!
//! See `docs/design/data-presentation.md` for the full component redesign.

use std::collections::BTreeSet;
use std::num::NonZeroU16;

use crate::style::Density;

// ── Density ─────────────────────────────────────────────────────────────────

/// Row chrome density for data surfaces (orthogonal to global [`Density`] gaps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DataDensity {
    /// Tighter rows (ops / dense grids).
    Compact,
    /// Default readable row padding.
    #[default]
    Comfortable,
}

impl DataDensity {
    /// Extra horizontal pad cells inside a cell.
    #[must_use]
    pub const fn cell_pad_x(self) -> u16 {
        match self {
            Self::Compact => 0,
            Self::Comfortable => 1,
        }
    }

    /// Body row height in terminal rows (1 = single-line cells).
    #[must_use]
    pub const fn row_height(self) -> u16 {
        1
    }

    /// Maps from design-system density.
    #[must_use]
    pub const fn from_design(density: Density) -> Self {
        match density {
            Density::Comfortable => Self::Comfortable,
            Density::Compact | Density::Dashboard => Self::Compact,
        }
    }
}

// ── Load / empty / error ────────────────────────────────────────────────────

/// Loading and readiness for projected datasets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum LoadState {
    /// No query yet / cleared.
    #[default]
    Idle,
    /// Full-surface loading (no rows to show).
    Loading {
        /// Optional status line.
        message: Option<String>,
    },
    /// Some rows resident; more may stream in.
    Partial {
        /// Resident row count (not total universe).
        resident: u64,
        /// Optional known total.
        total: Option<u64>,
    },
    /// Projection ready (resident == painted universe for this query).
    Ready {
        /// Row count in the current projection.
        count: u64,
    },
    /// Empty result set (successful query, zero rows).
    Empty {
        /// Optional guidance.
        message: Option<String>,
    },
    /// Failed load.
    Error {
        /// Error summary (domain-neutral).
        message: String,
        /// Whether retry is meaningful.
        retryable: bool,
    },
}

impl LoadState {
    /// Whether the body should paint a spinner/skeleton instead of rows.
    #[must_use]
    pub fn shows_loading_chrome(&self) -> bool {
        matches!(self, Self::Loading { .. })
    }

    /// Whether an empty-state panel should paint.
    #[must_use]
    pub fn shows_empty(&self) -> bool {
        matches!(self, Self::Empty { .. })
    }

    /// Whether an error panel should paint.
    #[must_use]
    pub fn shows_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

// ── Virtual window ──────────────────────────────────────────────────────────

/// Scroll window over a logical axis (rows or columns).
///
/// Paint cost must stay **O(viewport)**, never O(logical_len), even when
/// `logical_len` is 1_000_000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VirtualWindow {
    /// First visible logical index.
    pub offset: u64,
    /// Visible slots (rows or columns).
    pub viewport: u16,
    /// Total logical length (may be unknown → 0 means “unknown/unbounded”).
    pub logical_len: u64,
}

impl VirtualWindow {
    /// Creates a window.
    #[must_use]
    pub const fn new(logical_len: u64, viewport: u16) -> Self {
        Self {
            offset: 0,
            viewport: if viewport == 0 { 1 } else { viewport },
            logical_len,
        }
    }

    /// Maximum legal offset.
    #[must_use]
    pub const fn max_offset(self) -> u64 {
        if self.logical_len == 0 {
            return 0;
        }
        let vp = self.viewport as u64;
        if self.logical_len <= vp {
            0
        } else {
            self.logical_len.saturating_sub(vp)
        }
    }

    /// Clamps offset into range.
    pub fn clamp(&mut self) {
        let max = self.max_offset();
        if self.offset > max {
            self.offset = max;
        }
        if self.viewport == 0 {
            self.viewport = 1;
        }
    }

    /// Scroll by signed delta (rows/cols).
    pub fn scroll_by(&mut self, delta: i64) -> bool {
        let before = self.offset;
        if delta >= 0 {
            self.offset = self.offset.saturating_add(delta as u64);
        } else {
            self.offset = self.offset.saturating_sub((-delta) as u64);
        }
        self.clamp();
        before != self.offset
    }

    /// Ensure `index` is visible.
    pub fn reveal(&mut self, index: u64) -> bool {
        let before = self.offset;
        let vp = self.viewport.max(1) as u64;
        if index < self.offset {
            self.offset = index;
        } else if index >= self.offset.saturating_add(vp) {
            self.offset = index.saturating_add(1).saturating_sub(vp);
        }
        self.clamp();
        before != self.offset
    }

    /// Inclusive start / exclusive end of visible logical indices.
    #[must_use]
    pub const fn visible_range(self) -> (u64, u64) {
        let start = self.offset;
        let end = if self.logical_len == 0 {
            start.saturating_add(self.viewport as u64)
        } else {
            let e = start.saturating_add(self.viewport as u64);
            if e > self.logical_len {
                self.logical_len
            } else {
                e
            }
        };
        (start, end)
    }
}

// ── Column model ────────────────────────────────────────────────────────────

/// Width policy shared by DataTable / TreeTable / VirtualGrid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DataColumnWidth {
    /// Exact cells when possible.
    Fixed(u16),
    /// Preferred minimum under pressure.
    Min(u16),
    /// Share remainder by weight.
    Fill(NonZeroU16),
}

/// Horizontal pin edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ColumnPin {
    /// Scrolls with body.
    #[default]
    None,
    /// Sticky at start (left in LTR).
    Start,
    /// Sticky at end (right in LTR).
    End,
}

/// One column descriptor (id is consumer-owned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataColumn<Id> {
    /// Stable column id.
    pub id: Id,
    /// Header label.
    pub title: String,
    /// Width policy.
    pub width: DataColumnWidth,
    /// Visible in body.
    pub visible: bool,
    /// Pin behavior.
    pub pin: ColumnPin,
    /// Responsive drop priority: **lower drops first** under narrow pressure.
    /// Primary identity columns should use high values (e.g. 100).
    pub priority: u8,
    /// Sortable.
    pub sortable: bool,
    /// Inline editable.
    pub editable: bool,
}

impl<Id> DataColumn<Id> {
    /// Visible, unpinned, medium priority column.
    #[must_use]
    pub fn new(id: Id, title: impl Into<String>, width: DataColumnWidth) -> Self {
        Self {
            id,
            title: title.into(),
            width,
            visible: true,
            pin: ColumnPin::None,
            priority: 50,
            sortable: false,
            editable: false,
        }
    }

    /// Sets responsive priority (higher survives longer).
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Pins the column.
    #[must_use]
    pub const fn pin(mut self, pin: ColumnPin) -> Self {
        self.pin = pin;
        self
    }

    /// Hides the column.
    #[must_use]
    pub const fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

/// Column layout state: order, visibility, widths, pins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColumnModel<Id> {
    /// Columns in display order (including hidden).
    pub columns: Vec<DataColumn<Id>>,
    /// Optional resized widths overriding policy (by column id index).
    pub width_overrides: Vec<Option<u16>>,
}

impl<Id: PartialEq> ColumnModel<Id> {
    /// Creates from columns.
    #[must_use]
    pub fn new(columns: Vec<DataColumn<Id>>) -> Self {
        let n = columns.len();
        Self {
            columns,
            width_overrides: vec![None; n],
        }
    }

    /// Visible columns only, preserving order.
    pub fn visible(&self) -> impl Iterator<Item = (usize, &DataColumn<Id>)> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.visible)
    }

    /// Toggle visibility by id.
    pub fn set_visible(&mut self, id: &Id, visible: bool) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| &c.id == id) {
            if col.visible == visible {
                return false;
            }
            col.visible = visible;
            return true;
        }
        false
    }

    /// Drop lowest-priority unpinned columns until `budget` visible columns remain
    /// (or only essential priority ≥ `keep_min_priority` left).
    pub fn contract_to_budget(&mut self, budget: usize, keep_min_priority: u8) {
        loop {
            let visible: Vec<usize> = self
                .columns
                .iter()
                .enumerate()
                .filter(|(_, c)| c.visible)
                .map(|(i, _)| i)
                .collect();
            if visible.len() <= budget {
                break;
            }
            // Drop lowest priority among unpinned, not protected.
            let victim = visible
                .into_iter()
                .filter(|&i| {
                    self.columns[i].pin == ColumnPin::None
                        && self.columns[i].priority < keep_min_priority
                })
                .min_by_key(|&i| self.columns[i].priority);
            let Some(i) = victim else {
                break;
            };
            self.columns[i].visible = false;
        }
    }
}

// ── Selection ───────────────────────────────────────────────────────────────

/// Cell coordinate in logical space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CellCoord {
    /// Logical row index in the current projection.
    pub row: u64,
    /// Column index in the column model (including hidden? usually visible ordinal).
    pub col: usize,
}

/// Selection mode for data surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectionMode {
    /// No selection chrome.
    #[default]
    None,
    /// Single row.
    Row,
    /// Multiple rows.
    MultiRow,
    /// Single cell.
    Cell,
    /// Rectangular cell range.
    CellRange,
}

/// Selection state (ids optional — some grids are index-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionModel<RowId: Ord = u64> {
    /// Mode.
    pub mode: SelectionMode,
    /// Focused row index (keyboard cursor).
    pub focus_row: u64,
    /// Focused column ordinal among visible columns.
    pub focus_col: usize,
    /// Selected row ids (multi).
    pub selected_rows: BTreeSet<RowId>,
    /// Anchor for range selection.
    pub anchor: Option<CellCoord>,
    /// Active cell (cell modes).
    pub active_cell: Option<CellCoord>,
}

impl<RowId: Ord> Default for SelectionModel<RowId> {
    fn default() -> Self {
        Self {
            mode: SelectionMode::None,
            focus_row: 0,
            focus_col: 0,
            selected_rows: BTreeSet::new(),
            anchor: None,
            active_cell: None,
        }
    }
}

impl<RowId: Ord + Clone> SelectionModel<RowId> {
    /// Single-row mode.
    #[must_use]
    pub fn row() -> Self {
        Self {
            mode: SelectionMode::Row,
            ..Self::default()
        }
    }

    /// Multi-row mode.
    #[must_use]
    pub fn multi_row() -> Self {
        Self {
            mode: SelectionMode::MultiRow,
            ..Self::default()
        }
    }

    /// Cell mode.
    #[must_use]
    pub fn cell() -> Self {
        Self {
            mode: SelectionMode::Cell,
            ..Self::default()
        }
    }

    /// Move focus by delta; returns whether focus changed.
    pub fn move_focus(&mut self, d_row: i64, d_col: i32, max_row: u64, max_col: usize) -> bool {
        let before = (self.focus_row, self.focus_col);
        if d_row >= 0 {
            self.focus_row = (self.focus_row.saturating_add(d_row as u64)).min(max_row);
        } else {
            self.focus_row = self.focus_row.saturating_sub((-d_row) as u64);
        }
        if max_col == 0 {
            self.focus_col = 0;
        } else if d_col >= 0 {
            self.focus_col = (self.focus_col.saturating_add(d_col as usize)).min(max_col - 1);
        } else {
            self.focus_col = self.focus_col.saturating_sub((-d_col) as usize);
        }
        before != (self.focus_row, self.focus_col)
    }

    /// Toggle row id in multi selection.
    pub fn toggle_row(&mut self, id: RowId) {
        if !self.selected_rows.remove(&id) {
            self.selected_rows.insert(id);
        }
    }

    /// Clear selection sets (keeps focus).
    pub fn clear_selection(&mut self) {
        self.selected_rows.clear();
        self.anchor = None;
        self.active_cell = None;
    }
}

// ── Sort / filter / search (policy hooks) ───────────────────────────────────

/// Visible sort key (consumer applies to data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortSpec<ColId> {
    /// Column.
    pub column: ColId,
    /// Ascending when true.
    pub ascending: bool,
}

/// Filter / search request emitted by chrome (consumer executes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilterSpec {
    /// Free-text search.
    pub query: String,
    /// Optional column-scoped filters as opaque key=value (consumer parses).
    pub clauses: Vec<(String, String)>,
}

// ── Copy ────────────────────────────────────────────────────────────────────

/// Copy request payload (consumer owns clipboard / OSC 52 policy).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CopyPayload {
    /// Single cell text.
    Cell {
        /// Text.
        text: String,
    },
    /// TSV/CSV-ish range.
    Range {
        /// Rows of cells.
        rows: Vec<Vec<String>>,
        /// `true` = tab-separated.
        tsv: bool,
    },
    /// Whole focused row.
    Row {
        /// Cells.
        cells: Vec<String>,
    },
}

// ── Grouping / expand ───────────────────────────────────────────────────────

/// Group header in a projected row stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupHeader<Id> {
    /// Group id.
    pub id: Id,
    /// Label.
    pub label: String,
    /// Child count.
    pub count: u64,
    /// Expanded.
    pub expanded: bool,
}

/// Detail expansion for a row (master–detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandState<RowId: Ord> {
    /// Expanded row ids.
    pub expanded: BTreeSet<RowId>,
}

impl<RowId: Ord + Clone> Default for ExpandState<RowId> {
    fn default() -> Self {
        Self {
            expanded: BTreeSet::new(),
        }
    }
}

impl<RowId: Ord + Clone> ExpandState<RowId> {
    /// Toggle expand.
    pub fn toggle(&mut self, id: RowId) -> bool {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
            true
        } else {
            false
        }
    }
}

// ── Shared outcomes (non-exhaustive building blocks) ────────────────────────

/// Common navigation / chrome outcomes data views may emit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DataViewOutcome<RowId, ColId> {
    /// Ignored.
    Ignored,
    /// Viewport scrolled.
    Scrolled,
    /// Focus moved.
    FocusChanged,
    /// Selection changed.
    SelectionChanged,
    /// Sort requested (consumer re-projects).
    SortRequested(SortSpec<ColId>),
    /// Filter/search changed.
    FilterChanged(FilterSpec),
    /// Column visibility toggled.
    ColumnVisibility {
        /// Column.
        column: ColId,
        /// Visible.
        visible: bool,
    },
    /// Column resize.
    ColumnResized {
        /// Column.
        column: ColId,
        /// New width.
        width: u16,
    },
    /// Row activated (Enter / double-click).
    RowActivated(RowId),
    /// Cell activated.
    CellActivated {
        /// Row.
        row: RowId,
        /// Column.
        column: ColId,
    },
    /// Context menu requested at focus.
    ContextMenu {
        /// Row if any.
        row: Option<RowId>,
        /// Column if any.
        column: Option<ColId>,
    },
    /// Inline edit started.
    EditStarted {
        /// Row.
        row: RowId,
        /// Column.
        column: ColId,
    },
    /// Inline edit committed.
    EditCommitted {
        /// Row.
        row: RowId,
        /// Column.
        column: ColId,
        /// New text.
        text: String,
    },
    /// Inline edit cancelled.
    EditCancelled,
    /// Copy requested.
    Copy(CopyPayload),
    /// Expand toggled.
    ExpandToggled(RowId),
    /// Retry load.
    RetryLoad,
}

// ── Benchmark targets (documentation constants) ─────────────────────────────

/// Story / bench row counts for data surfaces.
pub mod bench {
    /// Tiny fixture.
    pub const ROWS_10: u64 = 10;
    /// Interactive medium table.
    pub const ROWS_10K: u64 = 10_000;
    /// Logical universe for virtualization (must not allocate per row).
    pub const ROWS_1M: u64 = 1_000_000;
    /// Wide table column count target.
    pub const COLS_WIDE: usize = 64;
    /// Paint budget: body rows visible on a large terminal.
    pub const VIEWPORT_ROWS: u16 = 40;
    /// Target: frame paint O(viewport), not O(logical).
    pub const MAX_PAINT_CELLS: u32 = 40 * 64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_window_clamps_and_reveals() {
        let mut w = VirtualWindow::new(bench::ROWS_1M, 20);
        assert_eq!(w.max_offset(), bench::ROWS_1M - 20);
        assert!(w.scroll_by(100));
        assert_eq!(w.offset, 100);
        assert!(w.reveal(500));
        assert!(w.offset <= 500);
        assert!(w.offset + 20 > 500);
        w.offset = u64::MAX;
        w.clamp();
        assert_eq!(w.offset, w.max_offset());
    }

    #[test]
    fn virtual_window_visible_range_bounded() {
        let w = VirtualWindow {
            offset: 50,
            viewport: 10,
            logical_len: 100,
        };
        assert_eq!(w.visible_range(), (50, 60));
        let end = VirtualWindow {
            offset: 95,
            viewport: 10,
            logical_len: 100,
        };
        assert_eq!(end.visible_range(), (95, 100));
    }

    #[test]
    fn column_contract_drops_low_priority_first() {
        let mut model = ColumnModel::new(vec![
            DataColumn::new("id", "ID", DataColumnWidth::Fixed(8)).priority(100),
            DataColumn::new("meta", "Meta", DataColumnWidth::Min(12)).priority(10),
            DataColumn::new("name", "Name", DataColumnWidth::Fill(NonZeroU16::new(1).unwrap()))
                .priority(80),
            DataColumn::new("extra", "Extra", DataColumnWidth::Min(10)).priority(5),
        ]);
        model.contract_to_budget(2, 90);
        let visible: Vec<_> = model
            .visible()
            .map(|(_, c)| c.id)
            .collect();
        assert!(visible.contains(&"id"));
        assert!(!visible.contains(&"extra"));
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn selection_multi_toggle() {
        let mut sel = SelectionModel::multi_row();
        sel.toggle_row(1u64);
        sel.toggle_row(2);
        assert_eq!(sel.selected_rows.len(), 2);
        sel.toggle_row(1);
        assert_eq!(sel.selected_rows.len(), 1);
    }

    #[test]
    fn selection_move_focus_clamps() {
        let mut sel = SelectionModel::<u64>::cell();
        assert!(sel.move_focus(5, 2, 10, 4));
        assert_eq!(sel.focus_row, 5);
        assert_eq!(sel.focus_col, 2);
        assert!(sel.move_focus(100, 100, 10, 4));
        assert_eq!(sel.focus_row, 10);
        assert_eq!(sel.focus_col, 3);
    }

    #[test]
    fn load_state_flags() {
        assert!(LoadState::Loading { message: None }.shows_loading_chrome());
        assert!(LoadState::Empty { message: None }.shows_empty());
        assert!(LoadState::Error {
            message: "x".into(),
            retryable: true
        }
        .shows_error());
    }

    #[test]
    fn expand_toggle() {
        let mut e = ExpandState::default();
        assert!(e.toggle("a"));
        assert!(e.expanded.contains(&"a"));
        assert!(!e.toggle("a"));
    }

    #[test]
    fn million_row_window_does_not_need_allocation() {
        // Sanity: window math only — no Vec of 1M.
        let w = VirtualWindow::new(bench::ROWS_1M, bench::VIEWPORT_ROWS);
        let (a, b) = w.visible_range();
        assert_eq!(b - a, u64::from(bench::VIEWPORT_ROWS));
        assert!(b <= bench::ROWS_1M);
    }
}
