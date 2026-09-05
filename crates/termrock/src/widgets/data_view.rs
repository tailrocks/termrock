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
//! - [`Density`] — compact vs comfortable row chrome
//! - [`CopyPayload`] — cell/range copy requests (consumer writes clipboard)
//!
//! See `docs/design/data-presentation.md` for the full component redesign.
use std::collections::BTreeSet;

use ratatui_core::style::Style;
use std::num::NonZeroU16;

use crate::style::{DesignSystem, Role};

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

/// Canonical non-ready dataset chrome shared by data owners.
pub(crate) struct DataLoadChrome {
    pub(crate) prefix: &'static str,
    pub(crate) message: String,
    pub(crate) role: Role,
}

/// Resolves loading/empty/error copy and non-color semantics once.
pub(crate) fn data_load_chrome(
    load: &LoadState,
    system: &DesignSystem,
    colorless: bool,
    empty_message: &str,
) -> Option<DataLoadChrome> {
    match load {
        LoadState::Loading { message } => Some(DataLoadChrome {
            prefix: "… ",
            message: message.clone().unwrap_or_else(|| "Loading…".into()),
            role: Role::TextMuted,
        }),
        LoadState::Empty { message } => Some(DataLoadChrome {
            prefix: "∅ ",
            message: message.clone().unwrap_or_else(|| empty_message.into()),
            role: Role::TextMuted,
        }),
        LoadState::Error { message, retryable } => Some(DataLoadChrome {
            prefix: "✗ ",
            message: if *retryable {
                format!("{message}  (r retry)")
            } else {
                message.clone()
            },
            role: if colorless || system.mono() {
                Role::TextStrong
            } else {
                Role::Danger
            },
        }),
        LoadState::Idle | LoadState::Partial { .. } | LoadState::Ready { .. } => None,
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
    ///
    /// Prefer [`crate::widgets::Virtualizer`] when you need overscan, sticky
    /// regions, variable extents, or semantic budgets.
    #[must_use]
    pub const fn visible_range(self) -> (u64, u64) {
        super::virtualizer::fixed_visible_range(self.offset, self.viewport, self.logical_len)
    }

    /// Lift into the canonical [`crate::widgets::Virtualizer`] (fixed extent 1).
    #[must_use]
    pub fn to_virtualizer(self) -> super::virtualizer::Virtualizer {
        super::virtualizer::Virtualizer::from_fixed_slots(
            self.offset,
            self.viewport,
            self.logical_len,
        )
    }

    /// Project from a fixed-extent virtualizer.
    #[must_use]
    pub fn from_virtualizer(v: &super::virtualizer::Virtualizer) -> Self {
        let (offset, viewport, logical_len) = v.to_fixed_slots();
        Self {
            offset,
            viewport,
            logical_len,
        }
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

/// What a column holds, which decides how loudly it reads.
///
/// A table of ten columns painted in one tone is ten equals: the identity you
/// scan for and the byte count you rarely read arrive with the same weight.
/// The kind is the column's tier — the design language's numeric-faint /
/// status-letter rule (`docs/design/termrock-design-language.md` §4.2) — and
/// it is the host's to state, because only the host knows what the column is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ColumnKind {
    /// Prose, names, paths: the row's own tone.
    #[default]
    Text,
    /// Counts, sizes, durations: the quiet tone, right-aligned, so the
    /// identity stays loud.
    Numeric,
    /// A state letter or glyph. Keeps the row tone and, unlike every other
    /// column, contracts to its first grapheme instead of ellipsizing — a
    /// one-cell status column shows the letter, not an ellipsis.
    Status,
    /// Keys / surrogates: secondary text, not the muted count tier.
    Id,
}

/// How an over-wide cell contracts (source `cell_text`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellEllipsisPolicy {
    /// Cut at the column edge with no marker.
    Clip,
    /// Ellipsize at the end.
    End,
    /// Keep both ends around an ellipsis.
    Middle,
}

impl ColumnKind {
    /// Picks between the row tone and the quiet tone for this column's cells.
    ///
    /// The caller supplies both because only it knows the row's visual state:
    /// a selected row's quiet tier is not the canvas's quiet tier.
    #[must_use]
    pub const fn cell_style(self, base: Style, quiet: Style) -> Style {
        match self {
            Self::Text | Self::Status => base,
            Self::Numeric | Self::Id => quiet,
        }
    }

    /// How an over-wide cell contracts (source `cell_text`): status clips,
    /// key/surrogate columns keep both ends around an ellipsis, the rest
    /// ellipsize at the end.
    #[must_use]
    pub const fn ellipsis_policy(self) -> CellEllipsisPolicy {
        match self {
            Self::Status => CellEllipsisPolicy::Clip,
            Self::Id => CellEllipsisPolicy::Middle,
            Self::Text | Self::Numeric => CellEllipsisPolicy::End,
        }
    }

    /// Whether header and body cells sit on the right edge of the column.
    ///
    /// Source DataGrid `CellKind::right_aligned` is Number-only; the same
    /// rule is the kind, not a second alignment field.
    #[must_use]
    pub const fn right_aligned(self) -> bool {
        matches!(self, Self::Numeric)
    }
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
    /// What the column holds, which decides its tone.
    pub kind: ColumnKind,
    /// Primary key: header paints junie `⚷` over the title origin.
    pub primary: bool,
    /// Filter active: header wears the junie `" ∇"` suffix.
    pub filtered: bool,
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
            kind: ColumnKind::Text,
            primary: false,
            filtered: false,
        }
    }

    /// States what the column holds, which decides its tone and alignment.
    #[must_use]
    pub const fn kind(mut self, kind: ColumnKind) -> Self {
        self.kind = kind;
        self
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

    /// Marks the column sortable (header / `s` chrome).
    #[must_use]
    pub const fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }

    /// Marks the column inline-editable (`e` / edit outcomes).
    #[must_use]
    pub const fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    /// Marks a primary-key column. Header origin is `⚷` (faint), not `▪`.
    #[must_use]
    pub const fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    /// Marks the column as carrying an active filter (header `" ∇"` suffix).
    #[must_use]
    pub const fn filtered(mut self) -> Self {
        self.filtered = true;
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
        self.columns.iter().enumerate().filter(|(_, c)| c.visible)
    }

    /// Index of column by id.
    #[must_use]
    pub fn index_of(&self, id: &Id) -> Option<usize> {
        self.columns.iter().position(|c| &c.id == id)
    }

    /// Effective width for a column index (override or policy floor).
    #[must_use]
    pub fn effective_width(&self, index: usize) -> u16 {
        if let Some(Some(w)) = self.width_overrides.get(index) {
            return (*w).max(1);
        }
        match self.columns.get(index).map(|c| c.width) {
            Some(DataColumnWidth::Fixed(w) | DataColumnWidth::Min(w)) => w.max(1),
            Some(DataColumnWidth::Fill(_)) => 8,
            None => 1,
        }
    }

    /// Set a resized width override (min 1).
    pub fn set_width_override(&mut self, id: &Id, width: u16) -> bool {
        let Some(i) = self.index_of(id) else {
            return false;
        };
        if self.width_overrides.len() <= i {
            self.width_overrides.resize(i + 1, None);
        }
        self.width_overrides[i] = Some(width.max(1));
        true
    }

    /// Clear width override for a column.
    pub fn clear_width_override(&mut self, id: &Id) -> bool {
        let Some(i) = self.index_of(id) else {
            return false;
        };
        if let Some(slot) = self.width_overrides.get_mut(i) {
            *slot = None;
            return true;
        }
        false
    }

    /// Reorder: move column at `from` so it lands at `to` (display order).
    pub fn move_column(&mut self, from: usize, to: usize) -> bool {
        if from >= self.columns.len() || to >= self.columns.len() || from == to {
            return false;
        }
        self.width_overrides.resize(self.columns.len(), None);
        let col = self.columns.remove(from);
        let w = self.width_overrides.remove(from);
        let insert_at = if from < to { to - 1 } else { to }.min(self.columns.len());
        self.columns.insert(insert_at, col);
        self.width_overrides.insert(insert_at, w);
        true
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

    /// Builder-style sortable / editable markers.
    pub fn set_sortable(&mut self, id: &Id, sortable: bool) -> bool {
        if let Some(col) = self.columns.iter_mut().find(|c| &c.id == id) {
            col.sortable = sortable;
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

    /// Resolve paint widths for visible columns into `out` (declaration index, width).
    /// Fills share remaining budget after fixed/min/overrides.
    ///
    /// Uses a two-cell gap (junie `gap = 2` / [`crate::style::SpacingScale::column_gap`]).
    pub fn resolve_paint_widths(&self, budget: u16, out: &mut Vec<(usize, u16)>) {
        self.resolve_paint_widths_with_gap(budget, 2, out);
    }

    /// [`Self::resolve_paint_widths`] with an explicit inter-column gap.
    pub fn resolve_paint_widths_with_gap(
        &self,
        budget: u16,
        gap: u16,
        out: &mut Vec<(usize, u16)>,
    ) {
        out.clear();
        let mut visible: Vec<usize> = self.visible().map(|(i, _)| i).collect();
        if visible.is_empty() || budget == 0 {
            return;
        }
        let floor = |index: usize| -> u16 {
            if let Some(Some(w)) = self.width_overrides.get(index).copied() {
                return w.max(1);
            }
            match self.columns[index].width {
                DataColumnWidth::Fixed(w) | DataColumnWidth::Min(w) => w.max(1),
                DataColumnWidth::Fill(_) => 0,
            }
        };
        loop {
            if visible.len() <= 1 {
                break;
            }
            let gaps =
                gap.saturating_mul(u16::try_from(visible.len().saturating_sub(1)).unwrap_or(0));
            let mandatory: u64 = visible.iter().map(|&i| u64::from(floor(i))).sum();
            if mandatory + u64::from(gaps) <= u64::from(budget) {
                break;
            }
            let Some(drop) = visible
                .iter()
                .copied()
                .min_by_key(|i| (self.columns[*i].priority, usize::MAX - i))
            else {
                break;
            };
            visible.retain(|&index| index != drop);
        }
        let gaps = gap.saturating_mul(u16::try_from(visible.len().saturating_sub(1)).unwrap_or(0));
        let mut remaining = budget.saturating_sub(gaps);
        // (index, assigned_or_weight, is_fill)
        let mut bases: Vec<(usize, u16, bool)> = Vec::with_capacity(visible.len());
        let mut fill_weight = 0u32;
        for &i in &visible {
            if let Some(Some(w)) = self.width_overrides.get(i).copied() {
                let take = w.max(1).min(remaining);
                remaining = remaining.saturating_sub(take);
                bases.push((i, take, false));
                continue;
            }
            match self.columns[i].width {
                DataColumnWidth::Fixed(w) | DataColumnWidth::Min(w) => {
                    let take = w.max(1).min(remaining);
                    remaining = remaining.saturating_sub(take);
                    bases.push((i, take, false));
                }
                DataColumnWidth::Fill(weight) => {
                    fill_weight += u32::from(weight.get());
                    bases.push((i, weight.get(), true));
                }
            }
        }
        let fill_positions: Vec<usize> = bases
            .iter()
            .enumerate()
            .filter_map(|(pos, (_, _, f))| (*f).then_some(pos))
            .collect();
        if fill_positions.is_empty() {
            // junie `Constraint::Min` grows into leftover; Fixed does not.
            let mins: Vec<usize> = bases
                .iter()
                .enumerate()
                .filter_map(|(pos, (index, _, _))| {
                    matches!(self.columns[*index].width, DataColumnWidth::Min(_)).then_some(pos)
                })
                .collect();
            if !mins.is_empty() && remaining > 0 {
                let n = u64::try_from(mins.len()).unwrap_or(1);
                let total = u64::from(remaining);
                let mut left = total;
                for (k, &pos) in mins.iter().enumerate() {
                    let extra = if k + 1 == mins.len() { left } else { total / n };
                    bases[pos].1 = bases[pos]
                        .1
                        .saturating_add(u16::try_from(extra).unwrap_or(u16::MAX));
                    left = left.saturating_sub(extra);
                }
            }
        } else if remaining == 0 {
            for pos in fill_positions {
                bases[pos].1 = 1;
            }
        } else {
            let total_w = fill_weight.max(1);
            let mut distributed = 0u16;
            for (n, &pos) in fill_positions.iter().enumerate() {
                let weight = u32::from(bases[pos].1.max(1));
                let share = if n + 1 == fill_positions.len() {
                    remaining.saturating_sub(distributed).max(1)
                } else {
                    let s =
                        ((u64::from(remaining) * u64::from(weight)) / u64::from(total_w)) as u16;
                    s.max(1)
                };
                bases[pos].1 = share;
                distributed = distributed.saturating_add(share);
            }
        }
        for (i, w, _) in bases {
            out.push((i, w.max(1)));
        }
    }
}

// ── Selection ───────────────────────────────────────────────────────────────

/// Cell coordinate in logical space (shared with [`crate::interaction::CellCoord`]).
pub type CellCoord = crate::interaction::CellCoord;

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

/// Selection state for data grids (cursor + row membership + cell chrome).
///
/// - **Rows:** [`crate::interaction::SelectionModel`] (stable IDs, range/select-all).
/// - **Cells:** [`crate::interaction::CellSelectionModel`] (single / rect).
/// - **focus_row/col:** grid cursor (not FocusGraph).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionModel<RowId: Ord = u64> {
    /// Mode.
    pub mode: SelectionMode,
    /// Focused row index (keyboard cursor — not graph focus).
    pub focus_row: u64,
    /// Focused column ordinal among visible columns.
    pub focus_col: usize,
    /// Row selection membership (ordered; multi/single via interaction model).
    pub rows: crate::interaction::SelectionModel<RowId>,
    /// Cell selection (single / rectangular range).
    pub cells: crate::interaction::CellSelectionModel,
}

impl<RowId: Ord> Default for SelectionModel<RowId> {
    fn default() -> Self {
        Self {
            mode: SelectionMode::None,
            focus_row: 0,
            focus_col: 0,
            rows: crate::interaction::SelectionModel::new(crate::interaction::SelectionKind::None),
            cells: crate::interaction::CellSelectionModel::new(),
        }
    }
}

impl<RowId: Ord + Clone> SelectionModel<RowId> {
    /// Single-row mode.
    #[must_use]
    pub fn row() -> Self {
        Self {
            mode: SelectionMode::Row,
            rows: crate::interaction::SelectionModel::single(),
            cells: crate::interaction::CellSelectionModel::new(),
            ..Self::default()
        }
    }

    /// Multi-row mode.
    #[must_use]
    pub fn multi_row() -> Self {
        Self {
            mode: SelectionMode::MultiRow,
            rows: crate::interaction::SelectionModel::range(),
            cells: crate::interaction::CellSelectionModel::new(),
            ..Self::default()
        }
    }

    /// Cell mode.
    #[must_use]
    pub fn cell() -> Self {
        Self {
            mode: SelectionMode::Cell,
            rows: crate::interaction::SelectionModel::new(crate::interaction::SelectionKind::None),
            cells: crate::interaction::CellSelectionModel::single(),
            ..Self::default()
        }
    }

    /// Cell-range mode.
    #[must_use]
    pub fn cell_range() -> Self {
        Self {
            mode: SelectionMode::CellRange,
            rows: crate::interaction::SelectionModel::new(crate::interaction::SelectionKind::None),
            cells: crate::interaction::CellSelectionModel::range(),
            ..Self::default()
        }
    }

    /// Selected row ids (ordered).
    #[must_use]
    pub fn selected_rows(&self) -> &[RowId] {
        self.rows.selected()
    }

    /// BTreeSet view for callers that need set semantics (allocates).
    #[must_use]
    pub fn selected_rows_set(&self) -> BTreeSet<RowId> {
        self.rows.selected().iter().cloned().collect()
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
        let _ = self.rows.toggle(&id);
    }

    /// Select a single row (single mode) or add (multi).
    pub fn select_row(&mut self, id: RowId) {
        let _ = self.rows.select(id);
    }

    /// Range-select rows along `order` from anchor to `to`.
    pub fn select_row_range(&mut self, order: &[RowId], to: &RowId) {
        let _ = self.rows.set_range(order, to);
    }

    /// Select all visible row ids.
    pub fn select_all_rows(&mut self, visible: &[RowId]) {
        let _ = self.rows.select_all(visible);
    }

    /// Drop row selection not in `still_valid`.
    pub fn reconcile_rows(&mut self, still_valid: &[RowId]) {
        let _ = self.rows.reconcile(still_valid);
    }

    /// Clear selection sets (keeps focus cursor).
    pub fn clear_selection(&mut self) {
        let _ = self.rows.clear();
        self.cells.clear();
    }

    /// Whether row id is selected.
    #[must_use]
    pub fn is_row_selected(&self, id: &RowId) -> bool {
        self.rows.is_selected(id)
    }

    /// Select / focus a single cell.
    pub fn select_cell(&mut self, cell: CellCoord) {
        self.cells.select_cell(cell);
        self.focus_row = cell.row;
        self.focus_col = cell.col;
    }

    /// Extend cell rect to `cell` (Shift-style).
    pub fn extend_cell(&mut self, cell: CellCoord) {
        self.cells.extend_to(cell);
        self.focus_row = cell.row;
        self.focus_col = cell.col;
    }

    /// Whether a cell is in the cell selection.
    #[must_use]
    pub fn is_cell_selected(&self, cell: CellCoord) -> bool {
        self.cells.contains(cell)
    }
}

/// Compatibility: field-like access for older code using `selected_rows` as set.
impl<RowId: Ord + Clone> SelectionModel<RowId> {
    /// Insert row id into multi selection (BTree-style).
    pub fn insert_row(&mut self, id: RowId) {
        let _ = self.rows.select(id);
    }

    /// Remove row id.
    pub fn remove_row(&mut self, id: &RowId) {
        let _ = self.rows.deselect(id);
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
    fn min_width_absorbs_leftover_after_fixed() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "ID", DataColumnWidth::Fixed(5)),
            DataColumn::new("task", "Task", DataColumnWidth::Min(24)),
            DataColumn::new("owner", "Owner", DataColumnWidth::Fixed(8)),
        ]);
        let mut out = Vec::new();
        cols.resolve_paint_widths_with_gap(51, 2, &mut out);
        assert_eq!(out, vec![(0, 5), (1, 34), (2, 8)]);
    }

    #[test]
    fn resolve_drops_rightmost_when_over_budget_then_min_grows() {
        let cols = ColumnModel::new(vec![
            DataColumn::new("id", "ID", DataColumnWidth::Fixed(5)),
            DataColumn::new("task", "Task", DataColumnWidth::Min(24)),
            DataColumn::new("owner", "Owner", DataColumnWidth::Fixed(8)),
            DataColumn::new("status", "Status", DataColumnWidth::Fixed(9)),
            DataColumn::new("branch", "Branch", DataColumnWidth::Fixed(22)),
            DataColumn::new("changes", "Changes", DataColumnWidth::Fixed(8)),
        ]);
        let mut out = Vec::new();
        cols.resolve_paint_widths_with_gap(85, 2, &mut out);
        assert!(
            !out.iter().any(|(index, _)| *index == 5),
            "Changes must drop: {out:?}"
        );
        assert_eq!(out[1], (1, 33), "Task Min absorbs leftover after the drop");
    }

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
            DataColumn::new(
                "name",
                "Name",
                DataColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
            )
            .priority(80),
            DataColumn::new("extra", "Extra", DataColumnWidth::Min(10)).priority(5),
        ]);
        model.contract_to_budget(2, 90);
        let visible: Vec<_> = model.visible().map(|(_, c)| c.id).collect();
        assert!(visible.contains(&"id"));
        assert!(!visible.contains(&"extra"));
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn selection_multi_toggle() {
        let mut sel = SelectionModel::multi_row();
        sel.toggle_row(1u64);
        sel.toggle_row(2);
        assert_eq!(sel.selected_rows().len(), 2);
        sel.toggle_row(1);
        assert_eq!(sel.selected_rows().len(), 1);
        assert!(sel.is_row_selected(&2));
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
        assert!(
            LoadState::Error {
                message: "x".into(),
                retryable: true
            }
            .shows_error()
        );
    }

    #[test]
    fn load_state_chrome_is_shared_ascii_and_non_color_semantics() {
        let system = DesignSystem::junie().no_color();
        let loading = data_load_chrome(
            &LoadState::Loading { message: None },
            &system,
            true,
            "No rows",
        )
        .unwrap();
        let empty = data_load_chrome(
            &LoadState::Empty { message: None },
            &system,
            true,
            "No rows",
        )
        .unwrap();
        let error = data_load_chrome(
            &LoadState::Error {
                message: "failed".into(),
                retryable: false,
            },
            &system,
            true,
            "No rows",
        )
        .unwrap();

        assert_eq!(
            (loading.prefix, loading.message.as_str()),
            ("… ", "Loading…")
        );
        assert_eq!((empty.prefix, empty.message.as_str()), ("∅ ", "No rows"));
        assert_eq!((error.prefix, error.message.as_str()), ("✗ ", "failed"));
        assert_eq!(error.role, Role::TextStrong);
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
