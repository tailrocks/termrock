//! Virtualized two-axis grid over caller-projected visible cells.
//!
//! TermRock owns viewport, selection, hit regions, and column widths.
//! Callers own data fetching, editing, sort/filter policy, and page models.
//! The grid never allocates the full data set; render cost is bounded by the
//! painted viewport.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{Role, Theme},
    text::{display_cols, take_display_cols},
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualGridState<RowId, ColId> {
    cursor_row: u64,
    cursor_col: usize,
    anchor: Option<(u64, usize)>,
    first_row: u64,
    first_col: usize,
    column_widths: Vec<u16>,
    focused: bool,
    body_rows: u16,
    body_cols_visible: usize,
    total_rows: Option<u64>,
    total_cols: usize,
    painted_area: Rect,
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
            first_row: 0,
            first_col: 0,
            column_widths: Vec::new(),
            focused: false,
            body_rows: 0,
            body_cols_visible: 0,
            total_rows: None,
            total_cols: 0,
            painted_area: Rect::default(),
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

    /// Sets keyboard focus ownership.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns whether this grid owns keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
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
        self.first_row
    }

    /// First column list index in the horizontal viewport.
    #[must_use]
    pub const fn first_col(&self) -> usize {
        self.first_col
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
        if let Some(total) = self.total_rows
            && total > 0
        {
            self.cursor_row = self.cursor_row.min(total.saturating_sub(1));
        }
        if self.total_cols > 0 {
            self.cursor_col = self.cursor_col.min(self.total_cols.saturating_sub(1));
        } else {
            self.cursor_col = 0;
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let body = u64::from(self.body_rows.max(1));
        if self.cursor_row < self.first_row {
            self.first_row = self.cursor_row;
        } else if self.cursor_row >= self.first_row.saturating_add(body) {
            self.first_row = self.cursor_row.saturating_sub(body.saturating_sub(1));
        }
        if self.cursor_col < self.first_col {
            self.first_col = self.cursor_col;
        } else if self.body_cols_visible > 0
            && self.cursor_col >= self.first_col.saturating_add(self.body_cols_visible)
        {
            self.first_col = self
                .cursor_col
                .saturating_sub(self.body_cols_visible.saturating_sub(1));
        }
        if let Some(total) = self.total_rows
            && total > 0
        {
            let max_first = total.saturating_sub(body);
            self.first_row = self.first_row.min(max_first);
        }
        if self.total_cols > 0 {
            let max_first = self
                .total_cols
                .saturating_sub(self.body_cols_visible.max(1));
            self.first_col = self.first_col.min(max_first);
        } else {
            self.first_col = 0;
        }
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
}

impl<RowId: Clone + Eq, ColId: Clone + Eq> VirtualGridState<RowId, ColId> {
    /// Handles a key event. Call only when focused.
    pub fn handle_key(
        &mut self,
        event: KeyEvent,
        columns: &[GridColumn<'_, ColId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if !self.focused || event.kind != KeyEventKind::Press {
            return VirtualGridOutcome::Ignored;
        }
        let extend = event.modifiers.contains(KeyModifiers::SHIFT);
        let control = event.modifiers.contains(KeyModifiers::CONTROL);
        let before = (self.first_row, self.first_col);
        let outcome = match event.code {
            KeyCode::Up => self.move_cursor(-1, 0, extend, columns),
            KeyCode::Down => self.move_cursor(1, 0, extend, columns),
            KeyCode::Left => self.move_cursor(0, -1, extend, columns),
            KeyCode::Right => self.move_cursor(0, 1, extend, columns),
            KeyCode::PageUp => {
                let step = i64::from(self.body_rows.max(1));
                self.move_cursor(-step, 0, extend, columns)
            }
            KeyCode::PageDown => {
                let step = i64::from(self.body_rows.max(1));
                self.move_cursor(step, 0, extend, columns)
            }
            KeyCode::Home if control => {
                self.cursor_row = 0;
                self.cursor_col = 0;
                if !extend {
                    self.anchor = None;
                } else if self.anchor.is_none() {
                    self.anchor = Some((self.cursor_row, self.cursor_col));
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns)
            }
            KeyCode::Home => {
                self.cursor_col = 0;
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns)
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
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns)
            }
            KeyCode::End => {
                if self.total_cols > 0 {
                    self.cursor_col = self.total_cols - 1;
                }
                if !extend {
                    self.anchor = None;
                }
                self.ensure_cursor_visible();
                self.cursor_outcome(columns)
            }
            KeyCode::Enter => {
                if columns.is_empty() {
                    VirtualGridOutcome::Ignored
                } else {
                    VirtualGridOutcome::Activated {
                        row: self.cursor_row,
                        col: self.cursor_col,
                        row_id: None,
                        col_id: columns[self.cursor_col.min(columns.len() - 1)].id.clone(),
                    }
                }
            }
            KeyCode::Esc => {
                if self.anchor.take().is_some() {
                    VirtualGridOutcome::RangeChanged {
                        start: (self.cursor_row, self.cursor_col),
                        end: (self.cursor_row, self.cursor_col),
                    }
                } else {
                    VirtualGridOutcome::Cancelled
                }
            }
            _ => VirtualGridOutcome::Ignored,
        };
        if (self.first_row, self.first_col) != before
            && !matches!(outcome, VirtualGridOutcome::Ignored)
        {
            // Prefer viewport notice when the window moved; cursor still valid.
            return VirtualGridOutcome::ViewportChanged {
                first_row: self.first_row,
                first_col: self.first_col,
            };
        }
        outcome
    }

    /// Handles a mouse event against the last painted geometry.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        columns: &[GridColumn<'_, ColId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() {
            return VirtualGridOutcome::Ignored;
        }
        let position = event.position;
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.first_row = self.first_row.saturating_add(1);
                if let Some(total) = self.total_rows {
                    let max_first = total.saturating_sub(u64::from(self.body_rows.max(1)));
                    self.first_row = self.first_row.min(max_first);
                }
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row,
                    first_col: self.first_col,
                }
            }
            MouseEventKind::ScrollUp => {
                self.first_row = self.first_row.saturating_sub(1);
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row,
                    first_col: self.first_col,
                }
            }
            MouseEventKind::ScrollRight => {
                if self.total_cols > 0 {
                    let max_first = self
                        .total_cols
                        .saturating_sub(self.body_cols_visible.max(1));
                    self.first_col = (self.first_col + 1).min(max_first);
                }
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row,
                    first_col: self.first_col,
                }
            }
            MouseEventKind::ScrollLeft => {
                self.first_col = self.first_col.saturating_sub(1);
                VirtualGridOutcome::ViewportChanged {
                    first_row: self.first_row,
                    first_col: self.first_col,
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(region) = self
                    .cell_regions
                    .iter()
                    .find(|region| region.area.contains(position))
                {
                    self.cursor_row = region.row_index;
                    self.cursor_col = region.col_index;
                    self.anchor = None;
                    self.ensure_cursor_visible();
                    self.cursor_outcome(columns)
                } else {
                    VirtualGridOutcome::Ignored
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(region) = self
                    .cell_regions
                    .iter()
                    .find(|region| region.area.contains(position))
                {
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
                } else {
                    VirtualGridOutcome::Ignored
                }
            }
            _ => VirtualGridOutcome::Ignored,
        }
    }

    fn move_cursor(
        &mut self,
        d_row: i64,
        d_col: i64,
        extend: bool,
        columns: &[GridColumn<'_, ColId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() {
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
        self.ensure_cursor_visible();
        if extend {
            VirtualGridOutcome::RangeChanged {
                start: self.anchor.unwrap_or((self.cursor_row, self.cursor_col)),
                end: (self.cursor_row, self.cursor_col),
            }
        } else {
            self.cursor_outcome(columns)
        }
    }

    fn cursor_outcome(
        &self,
        columns: &[GridColumn<'_, ColId>],
    ) -> VirtualGridOutcome<RowId, ColId> {
        if columns.is_empty() {
            return VirtualGridOutcome::Ignored;
        }
        let col = self.cursor_col.min(columns.len() - 1);
        VirtualGridOutcome::CursorMoved {
            row: self.cursor_row,
            col,
            row_id: None,
            col_id: columns[col].id.clone(),
        }
    }
}

/// Borrowed virtualized grid widget.
#[derive(Debug, Clone)]
pub struct VirtualGrid<'a, RowId, ColId> {
    columns: &'a [GridColumn<'a, ColId>],
    rows: &'a [GridRow<'a, RowId>],
    /// Known total row count, or `None` for unknown/unbounded.
    total_rows: Option<u64>,
    theme: &'a Theme,
    show_gutter: bool,
    show_header: bool,
}

impl<'a, RowId, ColId> VirtualGrid<'a, RowId, ColId> {
    /// Creates a grid over the given columns and currently resident rows.
    #[must_use]
    pub const fn new(
        columns: &'a [GridColumn<'a, ColId>],
        rows: &'a [GridRow<'a, RowId>],
        theme: &'a Theme,
    ) -> Self {
        Self {
            columns,
            rows,
            total_rows: None,
            theme,
            show_gutter: true,
            show_header: true,
        }
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
        state.painted_area = area;
        state.cell_regions.clear();
        state.header_regions.clear();
        state.total_rows = self.total_rows;
        state.total_cols = self.columns.len();
        if area.width == 0 || area.height == 0 || self.columns.is_empty() {
            state.body_rows = 0;
            state.body_cols_visible = 0;
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

        // Visible column window from first_col.
        let mut visible: Vec<(usize, u16)> = Vec::new();
        let mut used = 0u16;
        for (index, width) in state.column_widths.iter().enumerate().skip(state.first_col) {
            if used >= content_width {
                break;
            }
            let take = (*width).min(content_width.saturating_sub(used)).max(1);
            visible.push((index, take));
            used = used.saturating_add(take);
        }
        state.body_cols_visible = visible.len();
        state.clamp_cursor();
        state.ensure_cursor_visible();

        // Recompute visible after clamp may have changed first_col.
        visible.clear();
        used = 0;
        for (index, width) in state.column_widths.iter().enumerate().skip(state.first_col) {
            if used >= content_width {
                break;
            }
            let take = (*width).min(content_width.saturating_sub(used)).max(1);
            visible.push((index, take));
            used = used.saturating_add(take);
        }
        state.body_cols_visible = visible.len();

        let header_style = self.theme.style(Role::TextMuted);
        let cell_style = self.theme.style(Role::Text);
        let cursor_style = if state.focused {
            self.theme.style(Role::Accent)
        } else {
            self.theme.style(Role::Text)
        };
        let pending_style = self.theme.style(Role::TextMuted);
        let gutter_style = self.theme.style(Role::TextMuted);

        if self.show_header && area.height > 0 {
            let mut x = content_x;
            for &(col_index, width) in &visible {
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
            let abs_row = state.first_row.saturating_add(u64::from(row_slot));
            let resident = self.rows.iter().find(|row| row.index == abs_row);
            if self.show_gutter {
                let label = format!("{abs_row}");
                buffer.set_stringn(
                    area.x,
                    y,
                    &label,
                    usize::from(state.gutter_width.saturating_sub(1)),
                    gutter_style,
                );
            }
            let mut x = content_x;
            for &(col_index, width) in &visible {
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
                let in_range = state.anchor.is_some_and(|(ar, ac)| {
                    let (r0, r1) = if ar <= abs_row {
                        (ar, abs_row)
                    } else {
                        (abs_row, ar)
                    };
                    let (c0, c1) = if ac <= col_index {
                        (ac, col_index)
                    } else {
                        (col_index, ac)
                    };
                    abs_row >= r0 && abs_row <= r1 && col_index >= c0 && col_index <= c1
                });
                let style = if is_cursor {
                    cursor_style
                } else if cell.pending {
                    pending_style
                } else if in_range {
                    self.theme.style(Role::Accent)
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
                let _ = display_cols(&label);
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
        let theme = Theme::default();
        let columns = columns();
        let rows: [GridRow<'_, u64>; 0] = [];
        let grid = VirtualGrid::new(&columns, &rows, &theme).total_rows(0);
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
        let theme = Theme::default();
        let columns = columns();
        let cell_store = cells("1", "2", "3");
        let rows = [GridRow::new(0, 0, &cell_store)];
        let grid = VirtualGrid::new(&columns, &rows, &theme).total_rows(100);
        let mut state = VirtualGridState::new();
        state.set_focused(true);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 40, 8), &mut state);
            })
            .unwrap();
        assert!(state.body_rows > 0);

        let outcome = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &columns);
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved { row: 1, .. }
                | VirtualGridOutcome::ViewportChanged { .. }
        ));
        assert_eq!(state.cursor_row(), 1);

        let outcome = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &columns);
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved { col: 1, .. }
                | VirtualGridOutcome::ViewportChanged { .. }
        ));
    }

    #[test]
    fn shift_extends_range_and_escape_clears() {
        let columns = columns();
        let mut state = VirtualGridState::<u64, &str>::new();
        state.set_focused(true);
        state.total_rows = Some(50);
        state.total_cols = columns.len();
        state.body_rows = 10;
        state.body_cols_visible = 3;
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT), &columns);
        assert!(state.anchor().is_some());
        let outcome = state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &columns);
        assert!(matches!(
            outcome,
            VirtualGridOutcome::RangeChanged { .. } | VirtualGridOutcome::Cancelled
        ));
    }

    #[test]
    fn mouse_click_selects_painted_cell() {
        let theme = Theme::default();
        let columns = columns();
        let cell0 = cells("x", "y", "z");
        let cell1 = cells("p", "q", "r");
        let rows = [GridRow::new(10, 0, &cell0), GridRow::new(11, 1, &cell1)];
        let grid = VirtualGrid::new(&columns, &rows, &theme).total_rows(2);
        let mut state = VirtualGridState::new();
        state.set_focused(true);
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
        );
        assert!(matches!(
            outcome,
            VirtualGridOutcome::CursorMoved { row: 0, .. }
        ));
    }

    #[test]
    fn pending_cells_render_without_panic() {
        let theme = Theme::default();
        let columns = columns();
        let pending = [
            GridCell::pending(),
            GridCell::pending(),
            GridCell::pending(),
        ];
        let rows = [GridRow::new(0, 5, &pending)];
        let grid = VirtualGrid::new(&columns, &rows, &theme).total_rows(1_000_000);
        let mut state = VirtualGridState::new();
        state.first_row = 5;
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
        let theme = Theme::default();
        let columns = [GridColumn::fixed("u", "日本語", 6)];
        let cells = [GridCell::text("🚀ok")];
        let rows = [GridRow::new(0, 0, &cells)];
        let grid = VirtualGrid::new(&columns, &rows, &theme).total_rows(1);
        let mut state = VirtualGridState::new();
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(&grid, Rect::new(0, 0, 20, 4), &mut state);
            })
            .unwrap();
    }

    #[test]
    fn unfocused_keys_are_ignored() {
        let columns = columns();
        let mut state = VirtualGridState::<u64, &str>::new();
        state.total_cols = 3;
        state.total_rows = Some(10);
        let outcome = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &columns);
        assert_eq!(outcome, VirtualGridOutcome::Ignored);
    }
}
