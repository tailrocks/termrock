//! Virtualized two-axis grid over caller-projected visible cells.
//!
//! TermRock owns viewport, selection, hit regions, and column widths.
//! Callers own data fetching, editing, sort/filter policy, and page models.
//! The grid never allocates the full data set; render cost is bounded by the
//! painted viewport.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role, RolePalette},
    text::take_display_cols,
    widgets::virtualizer::Virtualizer2D,
};

/// Width policy for one grid column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridColumnWidth {
    /// Fixed display columns.
    Fixed(u16),
    /// Preferred minimum; may shrink under pressure.
    Min(u16),
}

/// Borrowed column header and width policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridColumn<'a, ColId> {
    /// Stable column identity.
    pub id: ColId,
    /// Header label (display-width measured).
    pub title: &'a str,
    /// Width policy.
    pub width: GridColumnWidth,
}

impl<'a, ColId> GridColumn<'a, ColId> {
    /// Creates a fixed-width column.
    #[must_use]
    pub const fn fixed(id: ColId, title: &'a str, width: u16) -> Self {
        Self {
            id,
            title,
            width: GridColumnWidth::Fixed(width),
        }
    }

    /// Creates a min-width column.
    #[must_use]
    pub const fn min(id: ColId, title: &'a str, width: u16) -> Self {
        Self {
            id,
            title,
            width: GridColumnWidth::Min(width),
        }
    }
}

/// One borrowed cell projection for the current paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCell<'a> {
    /// Visible text (Unicode display-width measured).
    pub text: &'a str,
    /// Optional style override.
    pub style: Option<Style>,
    /// When true, render a distinct placeholder (data not yet resident).
    pub pending: bool,
}

impl<'a> GridCell<'a> {
    /// Ordinary resident cell.
    #[must_use]
    pub const fn text(text: &'a str) -> Self {
        Self {
            text,
            style: None,
            pending: false,
        }
    }

    /// Placeholder for non-resident data.
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            text: "…",
            style: None,
            pending: true,
        }
    }

    /// Optional style override.
    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }
}

/// One visible body row: stable id + cells in column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRow<'a, RowId> {
    /// Stable row identity.
    pub id: RowId,
    /// Absolute dataset row index (for viewport math and selection).
    pub index: u64,
    /// Cells aligned with the column list (missing → pending).
    pub cells: &'a [GridCell<'a>],
    /// Whether the row accepts selection.
    pub enabled: bool,
}

impl<'a, RowId> GridRow<'a, RowId> {
    /// Creates an enabled row.
    #[must_use]
    pub const fn new(id: RowId, index: u64, cells: &'a [GridCell<'a>]) -> Self {
        Self {
            id,
            index,
            cells,
            enabled: true,
        }
    }

    /// Disables interaction.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Semantic result of grid interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VirtualGridOutcome<RowId, ColId> {
    /// Input did not apply.
    Ignored,
    /// Cursor moved to a cell.
    CursorMoved {
        /// Absolute row index.
        row: u64,
        /// Column index in the column list.
        col: usize,
        /// Stable row id when the row is resident.
        row_id: Option<RowId>,
        /// Stable column id.
        col_id: ColId,
    },
    /// Range selection changed (anchor + cursor).
    RangeChanged {
        /// Range start (inclusive).
        start: (u64, usize),
        /// Range end (inclusive, cursor).
        end: (u64, usize),
    },
    /// Enter/activate on the cursor cell.
    Activated {
        /// Absolute row index.
        row: u64,
        /// Column index.
        col: usize,
        /// Stable row id when resident.
        row_id: Option<RowId>,
        /// Stable column id.
        col_id: ColId,
    },
    /// Viewport origin changed (caller should reproject visible cells).
    ViewportChanged {
        /// First absolute row in the body viewport.
        first_row: u64,
        /// First column index in the horizontal viewport.
        first_col: usize,
    },
    /// Escape / cancel.
    Cancelled,
}

/// Painted body cell geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCellRegion<RowId, ColId> {
    /// Stable row id when the row was resident.
    pub row_id: Option<RowId>,
    /// Absolute row index.
    pub row_index: u64,
    /// Stable column id.
    pub col_id: ColId,
    /// Column list index.
    pub col_index: usize,
    /// Painted rectangle.
    pub area: Rect,
}

/// Painted header geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridHeaderRegion<ColId> {
    /// Stable column id.
    pub id: ColId,
    /// Column list index.
    pub index: usize,
    /// Painted rectangle.
    pub area: Rect,
}

/// Interaction and viewport state for [`VirtualGrid`].
///
/// Row/column windows use [`Virtualizer2D`] (canonical large-grid math).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualGridState<RowId, ColId> {
    cursor_row: u64,
    cursor_col: usize,
    anchor: Option<(u64, usize)>,
    /// Shared row/col virtualizer (offset, capacity, overscan, anchors).
    virt: Virtualizer2D,
    column_widths: Vec<u16>,
    body_rows: u16,
    body_cols_visible: usize,
    total_rows: Option<u64>,
    total_cols: usize,
    /// Exact body cell regions from the latest render.
    pub cell_regions: Vec<GridCellRegion<RowId, ColId>>,
    /// Exact header regions from the latest render.
    pub header_regions: Vec<GridHeaderRegion<ColId>>,
    gutter_width: u16,
}

impl<RowId, ColId> Default for VirtualGridState<RowId, ColId> {
    fn default() -> Self {
        Self {
            cursor_row: 0,
            cursor_col: 0,
            anchor: None,
            virt: Virtualizer2D::fixed_cells(),
            column_widths: Vec::new(),
            body_rows: 0,
            body_cols_visible: 0,
            total_rows: None,
            total_cols: 0,
            cell_regions: Vec::new(),
            header_regions: Vec::new(),
            gutter_width: 0,
        }
    }
}

impl<RowId, ColId> VirtualGridState<RowId, ColId> {
    /// Creates empty grid state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Absolute cursor row.
    #[must_use]
    pub const fn cursor_row(&self) -> u64 {
        self.cursor_row
    }

    /// Cursor column list index.
    #[must_use]
    pub const fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// Range selection anchor, when active.
    #[must_use]
    pub const fn anchor(&self) -> Option<(u64, usize)> {
        self.anchor
    }

    /// First absolute body row in the viewport.
    #[must_use]
    pub const fn first_row(&self) -> u64 {
        self.virt.rows.offset()
    }

    /// First column list index in the horizontal viewport.
    #[must_use]
    pub const fn first_col(&self) -> usize {
        self.virt.cols.offset() as usize
    }

    /// Canonical 2D virtualizer (row/col windows, semantic budget).
    #[must_use]
    pub const fn virtualizer(&self) -> &Virtualizer2D {
        &self.virt
    }

    /// Mutable virtualizer (overscan, sticky, anchors).
    pub fn virtualizer_mut(&mut self) -> &mut Virtualizer2D {
        &mut self.virt
    }

    /// Caller-persisted column widths (display columns).
    #[must_use]
    pub fn column_widths(&self) -> &[u16] {
        &self.column_widths
    }

    /// Replaces column widths (caller-owned persistence).
    pub fn set_column_widths(&mut self, widths: Vec<u16>) {
        self.column_widths = widths;
    }

    /// Clears range selection anchor.
    pub fn clear_anchor(&mut self) {
        self.anchor = None;
    }

    fn clamp_cursor(&mut self) {
        match self.total_rows {
            Some(0) => self.cursor_row = 0,
            Some(total) => {
                self.cursor_row = self.cursor_row.min(total.saturating_sub(1));
            }
            None => {}
        }
        if self.total_cols > 0 {
            self.cursor_col = self.cursor_col.min(self.total_cols.saturating_sub(1));
        } else {
            self.cursor_col = 0;
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let _ = self.virt.rows.reveal(self.cursor_row);
        if self.body_cols_visible > 0 {
            let first = self.virt.cols.offset() as usize;
            if self.cursor_col < first {
                self.virt.cols.set_offset(self.cursor_col as u64);
            } else if self.cursor_col >= first.saturating_add(self.body_cols_visible) {
                let next = self
                    .cursor_col
                    .saturating_sub(self.body_cols_visible.saturating_sub(1));
                self.virt.cols.set_offset(next as u64);
            }
        }
        self.sync_virt_bounds();
    }

    /// Push total_rows/cols into the virtualizer and clamp.
    fn sync_virt_bounds(&mut self) {
        let rows = match self.total_rows {
            Some(total) => total,
            // Unknown total: enough headroom for cursor + viewport + scroll.
            None => self
                .cursor_row
                .max(self.virt.rows.offset())
                .saturating_add(u64::from(self.body_rows.max(1)))
                .saturating_add(4_096),
        };
        self.virt.rows.set_len(rows);
        self.virt.cols.set_len(self.total_cols as u64);
        if self.body_rows > 0 {
            self.virt.rows.set_viewport_extent(self.body_rows);
        }
        // Column viewport is in "items" with fixed extent 1; visible count
        // is refined after width layout — use at least body_cols_visible.
        let col_vp = self.body_cols_visible.max(1) as u16;
        self.virt.cols.set_viewport_extent(col_vp);
    }

    fn resolve_widths_from_policy(columns: &[GridColumn<'_, ColId>], available: u16) -> Vec<u16> {
        if columns.is_empty() {
            return Vec::new();
        }
        let mut widths: Vec<u16> = columns
            .iter()
            .map(|column| match column.width {
                GridColumnWidth::Fixed(width) | GridColumnWidth::Min(width) => width.max(1),
            })
            .collect();
        let mut total: u32 = widths.iter().map(|width| u32::from(*width)).sum();
        let available = u32::from(available.max(1));
        while total > available {
            let mut shrunk = false;
            for (index, column) in columns.iter().enumerate() {
                if total <= available {
                    break;
                }
                if let GridColumnWidth::Min(min) = column.width
                    && widths[index] > min.max(1)
                {
                    widths[index] -= 1;
                    total -= 1;
                    shrunk = true;
                }
            }
            if !shrunk {
                break;
            }
        }
        widths
    }

    fn fill_visible_columns(
        &self,
        content_width: u16,
        gap_width: u16,
        out: &mut Vec<(usize, u16)>,
    ) {
        out.clear();
        let mut used = 0u16;
        for (index, width) in self.column_widths.iter().enumerate().skip(self.first_col()) {
            let gap = if out.is_empty() { 0 } else { gap_width };
            if used.saturating_add(gap) >= content_width {
                break;
            }
            used = used.saturating_add(gap);
            if used >= content_width {
                break;
            }
            let take = (*width).min(content_width.saturating_sub(used)).max(1);
            out.push((index, take));
            used = used.saturating_add(take);
        }
    }
}

/// Resolves a resident row by absolute index.
///
/// Prefer ordered projections (`binary_search`); falls back to a linear scan
/// when the slice is not sorted by `index`.
fn resident_at<'a, RowId>(
    rows: &'a [GridRow<'a, RowId>],
    index: u64,
) -> Option<&'a GridRow<'a, RowId>> {
    match rows.binary_search_by_key(&index, |row| row.index) {
        Ok(pos) => Some(&rows[pos]),
        Err(_) => rows.iter().find(|row| row.index == index),
    }
}

/// Whether an absolute row is inside the known dataset boundary.
fn row_in_bounds(total_rows: Option<u64>, row: u64) -> bool {
    match total_rows {
        Some(0) => false,
        Some(total) => row < total,
        None => true,
    }
}

impl<RowId: Clone + Eq, ColId: Clone + Eq> VirtualGridState<RowId, ColId> {
    /// Handles a key event. Call only when focused.
    ///
    /// Vertical navigation/page/activate/cancel route through
    /// [`crate::interaction::default_table_intent`]. Horizontal arrows and Shift range
    /// remain grid geometry (2-axis). Activate is Press-only.
    ///
    /// `rows` is the current borrowed resident projection used to resolve
    /// stable IDs and enabled state. It is not stored.
    pub fn handle_key(
        &mut self,
        event: KeyEvent,
        columns: &[GridColumn<'_, ColId>],
        rows: &[GridRow<'_, RowId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if event.kind == KeyEventKind::Release {
            return VirtualGridOutcome::Ignored;
        }
        let extend = event.modifiers.contains(KeyModifiers::SHIFT);
        let control = event.modifiers.contains(KeyModifiers::CONTROL);
        let before = (self.first_row(), self.first_col());
        // Prefer universal intents for shared collection actions.
        if !control && let Some(intent) = crate::interaction::default_table_intent(event) {
            if matches!(intent, crate::interaction::UiIntent::Activate)
                && event.kind != KeyEventKind::Press
            {
                return VirtualGridOutcome::Ignored;
            }
            // Page/Activate/Cancel/vertical Move via intent; Left/Right not in table map.
            if !matches!(
                intent,
                crate::interaction::UiIntent::Move(crate::interaction::NavigationMove::Previous)
                    | crate::interaction::UiIntent::Move(crate::interaction::NavigationMove::Next)
            ) || matches!(
                event.code,
                KeyCode::Up | KeyCode::Down | KeyCode::Char('k' | 'j')
            ) {
                let outcome = self.handle_intent(intent, extend, columns, rows);
                if (self.first_row(), self.first_col()) != before
                    && !matches!(outcome, VirtualGridOutcome::Ignored)
                {
                    return VirtualGridOutcome::ViewportChanged {
                        first_row: self.first_row(),
                        first_col: self.first_col(),
                    };
                }
                return outcome;
            }
        }
        let outcome = match event.code {
            KeyCode::Left => self.move_cursor(0, -1, extend, columns, rows),
            KeyCode::Right => self.move_cursor(0, 1, extend, columns, rows),
            KeyCode::Home if control => {
                self.cursor_row = 0;
                self.cursor_col = 0;
                self.skip_disabled_from(rows, 1);
                if !extend {
                    self.anchor = None;
                } else if self.anchor.is_none() {
                    self.anchor = Some((self.cursor_row, self.cursor_col));
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            KeyCode::Home => {
                self.cursor_col = 0;
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            KeyCode::End if control => {
                if let Some(total) = self.total_rows
                    && total > 0
                {
                    self.cursor_row = total - 1;
                }
                if self.total_cols > 0 {
                    self.cursor_col = self.total_cols - 1;
                }
                self.skip_disabled_from(rows, -1);
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            KeyCode::End => {
                if self.total_cols > 0 {
                    self.cursor_col = self.total_cols - 1;
                }
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            _ => VirtualGridOutcome::Ignored,
        };
        if (self.first_row(), self.first_col()) != before
            && !matches!(outcome, VirtualGridOutcome::Ignored)
        {
            // Prefer viewport notice when the window moved; cursor still valid.
            return VirtualGridOutcome::ViewportChanged {
                first_row: self.first_row(),
                first_col: self.first_col(),
            };
        }
        outcome
    }

    /// Applies a semantic collection intent (row axis + activate/cancel).
    pub fn handle_intent(
        &mut self,
        intent: crate::interaction::UiIntent,
        extend: bool,
        columns: &[GridColumn<'_, ColId>],
        rows: &[GridRow<'_, RowId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        use crate::interaction::{NavigationMove, PageMove, UiIntent};
        match intent {
            UiIntent::Move(NavigationMove::Previous) | UiIntent::Move(NavigationMove::Up) => {
                self.move_cursor(-1, 0, extend, columns, rows)
            }
            UiIntent::Move(NavigationMove::Next) | UiIntent::Move(NavigationMove::Down) => {
                self.move_cursor(1, 0, extend, columns, rows)
            }
            UiIntent::Move(NavigationMove::Left) => self.move_cursor(0, -1, extend, columns, rows),
            UiIntent::Move(NavigationMove::Right) => self.move_cursor(0, 1, extend, columns, rows),
            UiIntent::Move(NavigationMove::First) => {
                self.cursor_row = 0;
                self.skip_disabled_from(rows, 1);
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            UiIntent::Move(NavigationMove::Last) => {
                if let Some(total) = self.total_rows
                    && total > 0
                {
                    self.cursor_row = total - 1;
                }
                self.skip_disabled_from(rows, -1);
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i64::from(self.body_rows.max(1));
                self.move_cursor(-step, 0, extend, columns, rows)
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = i64::from(self.body_rows.max(1));
                self.move_cursor(step, 0, extend, columns, rows)
            }
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => self.activate(columns, rows),
            UiIntent::Cancel | UiIntent::Close => {
                if self.anchor.take().is_some() {
                    VirtualGridOutcome::RangeChanged {
                        start: (self.cursor_row, self.cursor_col),
                        end: (self.cursor_row, self.cursor_col),
                    }
                } else {
                    VirtualGridOutcome::Cancelled
                }
            }
            UiIntent::Toggle | UiIntent::Expand | UiIntent::Collapse => VirtualGridOutcome::Ignored,
            _ => VirtualGridOutcome::Ignored,
        }
    }

    /// Handles a mouse event against the last painted geometry.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        columns: &[GridColumn<'_, ColId>],
        rows: &[GridRow<'_, RowId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() {
            return VirtualGridOutcome::Ignored;
        }
        let position = event.position;
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.sync_virt_bounds();
                let _ = self.virt.rows.scroll_by(1);
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row(),
                    first_col: self.first_col(),
                }
            }
            MouseEventKind::ScrollUp => {
                self.sync_virt_bounds();
                let _ = self.virt.rows.scroll_by(-1);
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row(),
                    first_col: self.first_col(),
                }
            }
            MouseEventKind::ScrollRight => {
                self.sync_virt_bounds();
                let _ = self.virt.cols.scroll_by(1);
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row(),
                    first_col: self.first_col(),
                }
            }
            MouseEventKind::ScrollLeft => {
                self.sync_virt_bounds();
                let _ = self.virt.cols.scroll_by(-1);
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row(),
                    first_col: self.first_col(),
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(region) = self
                    .cell_regions
                    .iter()
                    .find(|region| region.area.contains(position))
                    .cloned()
                else {
                    return VirtualGridOutcome::Ignored;
                };
                if let Some(row) = resident_at(rows, region.row_index)
                    && !row.enabled
                {
                    return VirtualGridOutcome::Ignored;
                }
                if !row_in_bounds(self.total_rows, region.row_index) {
                    return VirtualGridOutcome::Ignored;
                }
                self.cursor_row = region.row_index;
                self.cursor_col = region.col_index;
                self.anchor = None;
                self.ensure_cursor_visible();
                self.cursor_outcome(columns, rows)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                let Some(region) = self
                    .cell_regions
                    .iter()
                    .find(|region| region.area.contains(position))
                    .cloned()
                else {
                    return VirtualGridOutcome::Ignored;
                };
                if let Some(row) = resident_at(rows, region.row_index)
                    && !row.enabled
                {
                    return VirtualGridOutcome::Ignored;
                }
                if !row_in_bounds(self.total_rows, region.row_index) {
                    return VirtualGridOutcome::Ignored;
                }
                if self.anchor.is_none() {
                    self.anchor = Some((self.cursor_row, self.cursor_col));
                }
                self.cursor_row = region.row_index;
                self.cursor_col = region.col_index;
                self.ensure_cursor_visible();
                VirtualGridOutcome::RangeChanged {
                    start: self.anchor.unwrap_or((self.cursor_row, self.cursor_col)),
                    end: (self.cursor_row, self.cursor_col),
                }
            }
            _ => VirtualGridOutcome::Ignored,
        }
    }

    fn activate(
        &self,
        columns: &[GridColumn<'_, ColId>],
        rows: &[GridRow<'_, RowId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() || !row_in_bounds(self.total_rows, self.cursor_row) {
            return VirtualGridOutcome::Ignored;
        }
        if let Some(row) = resident_at(rows, self.cursor_row)
            && !row.enabled
        {
            return VirtualGridOutcome::Ignored;
        }
        let col = self.cursor_col.min(columns.len() - 1);
        VirtualGridOutcome::Activated {
            row: self.cursor_row,
            col,
            row_id: resident_at(rows, self.cursor_row).map(|row| row.id.clone()),
            col_id: columns[col].id.clone(),
        }
    }

    fn move_cursor(
        &mut self,
        d_row: i64,
        d_col: i64,
        extend: bool,
        columns: &[GridColumn<'_, ColId>],
        rows: &[GridRow<'_, RowId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() {
            return VirtualGridOutcome::Ignored;
        }
        if self.total_rows == Some(0) {
            return VirtualGridOutcome::Ignored;
        }
        if extend && self.anchor.is_none() {
            self.anchor = Some((self.cursor_row, self.cursor_col));
        }
        if !extend {
            self.anchor = None;
        }
        if d_row < 0 {
            self.cursor_row = self.cursor_row.saturating_sub((-d_row) as u64);
        } else if d_row > 0 {
            self.cursor_row = self.cursor_row.saturating_add(d_row as u64);
        }
        if d_col < 0 {
            self.cursor_col = self.cursor_col.saturating_sub((-d_col) as usize);
        } else if d_col > 0 {
            self.cursor_col = self.cursor_col.saturating_add(d_col as usize);
        }
        self.clamp_cursor();
        let row_dir = if d_row > 0 {
            1
        } else if d_row < 0 {
            -1
        } else {
            0
        };
        if row_dir != 0 {
            self.skip_disabled_from(rows, row_dir);
        }
        self.ensure_cursor_visible();
        if extend {
            VirtualGridOutcome::RangeChanged {
                start: self.anchor.unwrap_or((self.cursor_row, self.cursor_col)),
                end: (self.cursor_row, self.cursor_col),
            }
        } else {
            self.cursor_outcome(columns, rows)
        }
    }

    /// Skips resident disabled rows in `direction` (+1 forward, -1 backward).
    fn skip_disabled_from(&mut self, rows: &[GridRow<'_, RowId>], direction: i8) {
        if direction == 0 {
            return;
        }
        let max_steps = self.total_rows.unwrap_or(10_000).min(10_000) as usize + rows.len() + 1;
        for _ in 0..max_steps {
            if !row_in_bounds(self.total_rows, self.cursor_row) {
                self.clamp_cursor();
                return;
            }
            match resident_at(rows, self.cursor_row) {
                Some(row) if !row.enabled => {
                    if direction > 0 {
                        self.cursor_row = self.cursor_row.saturating_add(1);
                    } else if self.cursor_row == 0 {
                        // Nowhere left; clamp will leave 0 — try forward for a
                        // usable row if every lower row is disabled.
                        break;
                    } else {
                        self.cursor_row = self.cursor_row.saturating_sub(1);
                    }
                    self.clamp_cursor();
                }
                _ => return,
            }
        }
    }

    fn cursor_outcome(
        &self,
        columns: &[GridColumn<'_, ColId>],
        rows: &[GridRow<'_, RowId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() || !row_in_bounds(self.total_rows, self.cursor_row) {
            return VirtualGridOutcome::Ignored;
        }
        let col = self.cursor_col.min(columns.len() - 1);
        VirtualGridOutcome::CursorMoved {
            row: self.cursor_row,
            col,
            row_id: resident_at(rows, self.cursor_row).map(|row| row.id.clone()),
            col_id: columns[col].id.clone(),
        }
    }

    fn range_contains(&self, row: u64, col: usize) -> bool {
        let Some((ar, ac)) = self.anchor else {
            return false;
        };
        let (cr, cc) = (self.cursor_row, self.cursor_col);
        let (r0, r1) = (ar.min(cr), ar.max(cr));
        let (c0, c1) = (ac.min(cc), ac.max(cc));
        row >= r0 && row <= r1 && col >= c0 && col <= c1
    }
}

/// Borrowed virtualized grid widget.
#[derive(Debug, Clone)]
pub struct VirtualGrid<'a, RowId, ColId> {
    empty_message: &'a str,
    focused: bool,
    columns: &'a [GridColumn<'a, ColId>],
    rows: &'a [GridRow<'a, RowId>],
    /// Known total row count, or `None` for unknown/unbounded.
    total_rows: Option<u64>,
    system: &'a DesignSystem,
    show_gutter: bool,
    show_header: bool,
}

impl<'a, RowId, ColId> VirtualGrid<'a, RowId, ColId> {
    /// Creates a grid over the given columns and currently resident rows.
    #[must_use]
    pub const fn new(
        columns: &'a [GridColumn<'a, ColId>],
        rows: &'a [GridRow<'a, RowId>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            empty_message: "No rows",
            focused: true,
            columns,
            rows,
            total_rows: None,
            system,
            show_gutter: true,
            show_header: true,
        }
    }

    /// Line shown when there is nothing to show.
    ///
    /// A collection that paints nothing when empty reads as broken; it has to
    /// say that it is empty.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    /// Whether this surface owns keyboard focus this frame (host / scene).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Declares a known total row count (unknown totals omit this).
    #[must_use]
    pub const fn total_rows(mut self, total: u64) -> Self {
        self.total_rows = Some(total);
        self
    }

    /// Shows or hides the row-index gutter.
    #[must_use]
    pub const fn gutter(mut self, show: bool) -> Self {
        self.show_gutter = show;
        self
    }

    /// Shows or hides the header row.
    #[must_use]
    pub const fn header(mut self, show: bool) -> Self {
        self.show_header = show;
        self
    }
}

impl<RowId: Clone + Eq, ColId: Clone + Eq> StatefulWidget for &VirtualGrid<'_, RowId, ColId> {
    type State = VirtualGridState<RowId, ColId>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.cell_regions.clear();
        state.header_regions.clear();
        state.total_rows = self.total_rows;
        state.total_cols = self.columns.len();
        if area.width == 0 || area.height == 0 || self.columns.is_empty() {
            state.body_rows = 0;
            state.body_cols_visible = 0;
            return;
        }

        if self.rows.is_empty() && self.total_rows == Some(0) {
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(self.empty_message, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }
        let header_rows: u16 = u16::from(self.show_header);
        let body_height = area.height.saturating_sub(header_rows);
        state.body_rows = body_height;
        state.gutter_width = if self.show_gutter { 6 } else { 0 };
        let content_x = area.x.saturating_add(state.gutter_width);
        let content_width = area.width.saturating_sub(state.gutter_width);

        if state.column_widths.len() != self.columns.len() {
            state.column_widths = VirtualGridState::<RowId, ColId>::resolve_widths_from_policy(
                self.columns,
                content_width,
            );
        }

        // Visible column window from virtualizer first_col.
        let mut visible: Vec<(usize, u16)> = Vec::new();
        state.fill_visible_columns(content_width, self.system.spacing.column_gap, &mut visible);
        state.body_cols_visible = visible.len();
        let first_col = state.first_col();
        state.sync_virt_bounds();
        state.clamp_cursor();

        state.ensure_cursor_visible();

        // Recompute only when revealing the cursor moved the horizontal window.
        if state.first_col() != first_col {
            state.fill_visible_columns(content_width, self.system.spacing.column_gap, &mut visible);
        }
        state.body_cols_visible = visible.len();

        let header_style = super::table_chrome::header_style(self.system);
        let cell_style = self.system.style(Role::Text);
        // The cursor is one cell, and it reverses. Unfocused it stops
        // reversing but keeps its gutter marker, so the position is never
        // invisible — losing focus must not lose your place.
        let cursor_style = if self.focused {
            // The cursor cell is the explicit reversal pair.
            self.system.reversed()
        } else {
            self.system.style(Role::TextStrong)
        };
        let pending_style = self.system.style(Role::TextMuted);
        let gutter_style = self.system.style(Role::TextMuted);

        if self.show_header && area.height > 0 {
            let mut x = content_x;
            let gap = self.system.spacing.column_gap;
            for (visible_index, &(col_index, width)) in visible.iter().enumerate() {
                if visible_index > 0 {
                    x = x.saturating_add(gap);
                }
                let column = &self.columns[col_index];
                let region = Rect {
                    x,
                    y: area.y,
                    width,
                    height: 1,
                };
                state.header_regions.push(GridHeaderRegion {
                    id: column.id.clone(),
                    index: col_index,
                    area: region,
                });
                let label = take_display_cols(column.title, usize::from(width));
                buffer.set_stringn(region.x, region.y, &label, usize::from(width), header_style);
                x = x.saturating_add(width);
            }
            if self.show_gutter {
                buffer.set_stringn(
                    area.x,
                    area.y,
                    "#",
                    usize::from(state.gutter_width),
                    gutter_style,
                );
            }
        }

        let body_y = area.y.saturating_add(header_rows);
        for row_slot in 0..body_height {
            let y = body_y.saturating_add(row_slot);
            let abs_row = state.first_row().saturating_add(u64::from(row_slot));
            // Known totals are authoritative: never paint or hit-test past total.
            if let Some(total) = self.total_rows
                && abs_row >= total
            {
                break;
            }
            let resident = resident_at(self.rows, abs_row);
            let cursor_row = abs_row == state.cursor_row;
            if self.show_gutter {
                let label = format!("{abs_row}");
                buffer.set_stringn(
                    area.x,
                    y,
                    &label,
                    usize::from(state.gutter_width.saturating_sub(1)),
                    if cursor_row {
                        self.system.style(Role::TextStrong)
                    } else {
                        gutter_style
                    },
                );
                if cursor_row && state.gutter_width > 0 {
                    let marker_x = area.x.saturating_add(state.gutter_width.saturating_sub(1));
                    if let Some(cell) = buffer.cell_mut((marker_x, y)) {
                        cell.set_symbol(self.system.glyphs.selection_gutter());
                        cell.set_style(if self.focused {
                            self.system.style(Role::Accent)
                        } else {
                            self.system.style(Role::TextMuted)
                        });
                    }
                }
            }
            let mut x = content_x;
            let gap = self.system.spacing.column_gap;
            for (visible_index, &(col_index, width)) in visible.iter().enumerate() {
                if visible_index > 0 {
                    x = x.saturating_add(gap);
                }
                let region = Rect {
                    x,
                    y,
                    width,
                    height: 1,
                };
                let column = &self.columns[col_index];
                let cell = resident
                    .and_then(|row| row.cells.get(col_index).copied())
                    .unwrap_or(GridCell::pending());
                let is_cursor = abs_row == state.cursor_row && col_index == state.cursor_col;
                let in_range = state.range_contains(abs_row, col_index);
                let disabled = resident.is_some_and(|row| !row.enabled);
                let style = if is_cursor && !disabled {
                    cursor_style
                } else if disabled {
                    self.system.style(Role::TextMuted)
                } else if cell.pending {
                    pending_style
                } else if in_range {
                    // A range is a ground, not a wall of accent.
                    cell_style.patch(self.system.style(Role::SelectionTint))
                } else {
                    cell.style.unwrap_or(cell_style)
                };
                let text = if cell.pending { "…" } else { cell.text };
                let label = take_display_cols(text, usize::from(width));
                // Clear then paint (avoids leftover glyphs on narrow columns).
                for dx in 0..width {
                    if let Some(cell_buf) = buffer.cell_mut((x.saturating_add(dx), y)) {
                        cell_buf.set_symbol(" ");
                        cell_buf.set_style(style);
                    }
                }
                buffer.set_stringn(x, y, &label, usize::from(width), style);
                state.cell_regions.push(GridCellRegion {
                    row_id: resident.map(|row| row.id.clone()),
                    row_index: abs_row,
                    col_id: column.id.clone(),
                    col_index,
                    area: region,
                });
                x = x.saturating_add(width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
    use ratatui_core::{backend::TestBackend, layout::Position, terminal::Terminal};

    fn columns() -> Vec<GridColumn<'static, &'static str>> {
        vec![
            GridColumn::fixed("a", "A", 8),
            GridColumn::fixed("b", "B", 8),
            GridColumn::min("c", "C", 4),
        ]
    }

    fn cells(a: &'static str, b: &'static str, c: &'static str) -> [GridCell<'static>; 3] {
        [GridCell::text(a), GridCell::text(b), GridCell::text(c)]
    }

    #[test]
    fn empty_and_min_rect_do_not_panic() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let rows: [GridRow<'_, u64>; 0] = [];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(0);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(0, 0)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::default(), &mut state);
            })
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(3, 1)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 3, 1), &mut state);
            })
            .unwrap();
    }

    #[test]
    fn keyboard_moves_cursor_and_viewport() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let cell_store = cells("1", "2", "3");
        let rows = [GridRow::new(0, 0, &cell_store)];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(100);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 8), &mut state);
            })
            .unwrap();
        assert!(state.body_rows > 0);

        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved { row: 1, .. }
                | VirtualGridOutcome::ViewportChanged { .. }
        ));
        assert_eq!(state.cursor_row(), 1);

        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved { col: 1, .. }
                | VirtualGridOutcome::ViewportChanged { .. }
        ));
    }

    #[test]
    fn offscreen_horizontal_cursor_reveals_header_and_cell_regions() {
        let system = DesignSystem::default();
        let columns = [
            GridColumn::fixed(0, "A", 3),
            GridColumn::fixed(1, "B", 5),
            GridColumn::fixed(2, "C", 2),
            GridColumn::fixed(3, "D", 4),
            GridColumn::fixed(4, "E", 3),
        ];
        let cells = [
            GridCell::text("a"),
            GridCell::text("b"),
            GridCell::text("c"),
            GridCell::text("d"),
            GridCell::text("e"),
        ];
        let rows = [GridRow::new(0, 0, &cells)];
        let grid = VirtualGrid::new(&columns, &rows, &system)
            .total_rows(1)
            .gutter(false);
        let area = Rect::new(0, 0, 12, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = VirtualGridState::new();
        state.cursor_col = 4;

        StatefulWidget::render(&grid, area, &mut buffer, &mut state);

        assert_eq!(state.first_col(), 3);
        assert_eq!(state.body_cols_visible, 2);
        assert_eq!(
            state
                .header_regions
                .iter()
                .map(|region| region.index)
                .collect::<Vec<_>>(),
            vec![3, 4]
        );
        assert_eq!(
            state
                .cell_regions
                .iter()
                .map(|region| region.col_index)
                .collect::<Vec<_>>(),
            vec![3, 4]
        );
    }

    #[test]
    fn column_count_shrink_reprojects_after_offset_clamp() {
        let system = DesignSystem::default();
        let columns = [
            GridColumn::fixed(0, "A", 3),
            GridColumn::fixed(1, "B", 5),
            GridColumn::fixed(2, "C", 2),
            GridColumn::fixed(3, "D", 4),
            GridColumn::fixed(4, "E", 3),
            GridColumn::fixed(5, "F", 4),
        ];
        let cells = [
            GridCell::text("a"),
            GridCell::text("b"),
            GridCell::text("c"),
            GridCell::text("d"),
            GridCell::text("e"),
            GridCell::text("f"),
        ];
        let rows = [GridRow::new(0, 0, &cells)];
        let area = Rect::new(0, 0, 12, 2);
        let mut state = VirtualGridState::new();
        let initial_grid = VirtualGrid::new(&columns, &rows, &system)
            .total_rows(1)
            .gutter(false);
        let mut buffer = Buffer::empty(area);
        StatefulWidget::render(&initial_grid, area, &mut buffer, &mut state);

        state.virtualizer_mut().cols.set_viewport_extent(1);
        state.virtualizer_mut().cols.set_offset(5);
        let shrunken_columns = &columns[..4];
        let shrunken_cells = &cells[..4];
        let shrunken_rows = [GridRow::new(0, 0, shrunken_cells)];
        let shrunken_grid = VirtualGrid::new(shrunken_columns, &shrunken_rows, &system)
            .total_rows(1)
            .gutter(false);
        StatefulWidget::render(&shrunken_grid, area, &mut buffer, &mut state);

        assert_eq!(state.first_col(), 3);
        assert_eq!(state.body_cols_visible, 1);
        assert_eq!(
            state
                .header_regions
                .iter()
                .map(|region| region.index)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(
            state
                .cell_regions
                .iter()
                .map(|region| region.col_index)
                .collect::<Vec<_>>(),
            vec![3]
        );
    }

    #[test]
    fn shift_extends_range_and_escape_clears() {
        let columns = columns();
        let rows: [GridRow<'_, u64>; 0] = [];
        let mut state = VirtualGridState::<u64, &str>::new();
        state.total_rows = Some(50);
        state.total_cols = columns.len();
        state.body_rows = 10;
        state.body_cols_visible = 3;
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
            &columns,
            &rows,
        );
        assert!(state.anchor().is_some());
        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::RangeChanged { .. } | VirtualGridOutcome::Cancelled
        ));
    }

    #[test]
    fn mouse_click_selects_painted_cell() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let cell0 = cells("x", "y", "z");
        let cell1 = cells("p", "q", "r");
        let rows = [GridRow::new(10, 0, &cell0), GridRow::new(11, 1, &cell1)];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(2);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 6), &mut state);
            })
            .unwrap();
        assert!(!state.cell_regions.is_empty());
        let target = state.cell_regions[0].area;
        let outcome = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: target.x,
                    y: target.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            &columns,
            &rows,
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved {
                row: 0,
                row_id: Some(10),
                ..
            }
        ));
    }

    #[test]
    fn pending_cells_render_without_panic() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let pending = [
            GridCell::pending(),
            GridCell::pending(),
            GridCell::pending(),
        ];
        let rows = [GridRow::new(0, 5, &pending)];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(1_000_000);
        let mut state = VirtualGridState::new();
        state.virtualizer_mut().rows.set_offset(5);
        let mut terminal = Terminal::new(TestBackend::new(50, 10)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 50, 10), &mut state);
            })
            .unwrap();
        assert!(!state.cell_regions.is_empty());
    }

    #[test]
    fn unicode_header_and_cell_width_is_safe() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = [GridColumn::fixed("u", "日本語", 6)];
        let cells = [GridCell::text("🚀ok")];
        let rows = [GridRow::new(0, 0, &cells)];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(1);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 20, 4), &mut state);
            })
            .unwrap();
    }

    #[test]
    fn adjacent_columns_keep_a_stable_blank_gap() {
        let system = DesignSystem::default();
        let columns = [
            GridColumn::fixed("a", "Alpha", 5),
            GridColumn::fixed("b", "Beta", 4),
        ];
        let cells = [GridCell::text("aaaaa"), GridCell::text("bbbb")];
        let rows = [GridRow::new(7, 0, &cells)];
        let grid = VirtualGrid::new(&columns, &rows, &system)
            .total_rows(1)
            .gutter(false);
        let area = Rect::new(0, 0, 12, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = VirtualGridState::new();
        StatefulWidget::render(&grid, area, &mut buffer, &mut state);

        let first = &state.cell_regions[0];
        let second = &state.cell_regions[1];
        let gap = system.spacing.column_gap;
        assert_eq!(second.area.x, first.area.right().saturating_add(gap));
        assert_eq!(buffer[(first.area.right(), 1)].symbol(), " ");
        assert_eq!(
            state.header_regions[1].area.x,
            state.header_regions[0].area.right().saturating_add(gap)
        );
    }

    #[test]
    fn zero_total_has_no_body_hits_and_enter_ignored() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let rows: [GridRow<'_, u64>; 0] = [];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(0);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 10)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 10), &mut state);
            })
            .unwrap();
        assert!(
            state.cell_regions.is_empty(),
            "zero total must not publish body hit regions"
        );
        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert_eq!(outcome, VirtualGridOutcome::Ignored);
    }

    #[test]
    fn short_total_paints_only_existing_rows() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let cell0 = cells("a", "b", "c");
        let cell1 = cells("d", "e", "f");
        let rows = [GridRow::new(0, 0, &cell0), GridRow::new(1, 1, &cell1)];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(2);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 12), &mut state);
            })
            .unwrap();
        let painted_rows: Vec<u64> = state
            .cell_regions
            .iter()
            .map(|region| region.row_index)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        assert_eq!(painted_rows, vec![0, 1]);
    }

    #[test]
    fn disabled_resident_rows_cannot_be_selected_or_activated() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let cell0 = cells("a", "b", "c");
        let cell1 = cells("d", "e", "f");
        let cell2 = cells("g", "h", "i");
        let rows = [
            GridRow::new(0, 0, &cell0),
            GridRow::new(1, 1, &cell1).enabled(false),
            GridRow::new(2, 2, &cell2),
        ];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(3);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 8), &mut state);
            })
            .unwrap();

        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved {
                row: 2,
                row_id: Some(2),
                ..
            } | VirtualGridOutcome::ViewportChanged { .. }
        ));
        assert_eq!(state.cursor_row(), 2);

        // Land on disabled via direct cursor, activate must ignore.
        state.cursor_row = 1;
        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert_eq!(outcome, VirtualGridOutcome::Ignored);

        let disabled_region = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == 1)
            .expect("disabled row painted");
        let click = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position {
                    x: disabled_region.area.x,
                    y: disabled_region.area.y,
                },
                modifiers: KeyModifiers::NONE,
            },
            &columns,
            &rows,
        );
        assert_eq!(click, VirtualGridOutcome::Ignored);
    }

    #[test]
    fn range_paint_uses_anchor_to_cursor_rectangle() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::new(theme.clone());
        let columns = columns();
        let shared = cells("x", "y", "z");
        let rows = [
            GridRow::new(0, 0, &shared),
            GridRow::new(1, 1, &shared),
            GridRow::new(2, 2, &shared),
            GridRow::new(3, 3, &shared),
            GridRow::new(4, 4, &shared),
        ];
        let grid = VirtualGrid::new(&columns, &rows, &system).total_rows(5);
        let mut state = VirtualGridState::new();
        state.anchor = Some((0, 0));
        state.cursor_row = 2;
        state.cursor_col = 1;
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 8), &mut state);
            })
            .unwrap();
        // Rectangle rows 0..=2, cols 0..=1 — not the full viewport.
        assert!(state.range_contains(0, 0));
        assert!(state.range_contains(2, 1));
        assert!(!state.range_contains(3, 0));
        assert!(!state.range_contains(0, 2));
    }

    #[test]
    fn resident_keyboard_outcomes_carry_row_id() {
        let columns = columns();
        let shared = cells("x", "y", "z");
        let rows = [GridRow::new(99, 0, &shared)];
        let mut state = VirtualGridState::<u64, &str>::new();
        state.total_rows = Some(5);
        state.total_cols = columns.len();
        state.body_rows = 5;
        state.body_cols_visible = 3;
        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &columns,
            &rows,
        );
        assert_eq!(
            outcome,
            VirtualGridOutcome::Activated {
                row: 0,
                col: 0,
                row_id: Some(99),
                col_id: "a",
            }
        );
        // Non-resident in-bounds row keeps None id.
        let empty: [GridRow<'_, u64>; 0] = [];
        state.cursor_row = 3;
        let outcome = state.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &columns,
            &empty,
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::Activated {
                row: 3,
                row_id: None,
                ..
            }
        ));
    }
}
