//! Borrowed columnar data with deterministic width negotiation and stable-ID interaction.

use std::num::NonZeroU16;

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::StatefulWidget,
};

use crate::{
    Theme,
    input::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    style::Role,
};

const MARKER_WIDTH: u16 = 2;

/// Width policy for one table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnWidth {
    /// Reserve an exact width whenever the viewport can honor it.
    Fixed(u16),
    /// Reserve a preferred minimum that shrinks before fixed columns.
    Min(u16),
    /// Share remaining width using a non-zero weight.
    Fill(NonZeroU16),
}

/// Horizontal alignment of a table cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellAlignment {
    /// Align content to the left edge.
    #[default]
    Left,
    /// Center content in the resolved width.
    Center,
    /// Align content to the right edge.
    Right,
}

/// Visible sort direction for a sortable column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending order, rendered as `▲`.
    Ascending,
    /// Descending order, rendered as `▼`.
    Descending,
}

/// Borrowed description of one table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column<'a, Id> {
    /// Stable identity used by sort outcomes and header regions.
    pub id: Id,
    /// Styled header content.
    pub title: Line<'a>,
    /// Width negotiation policy.
    pub width: ColumnWidth,
    /// Cell and header alignment.
    pub alignment: CellAlignment,
    /// Whether pointer activation may request sorting.
    pub sortable: bool,
    /// Current caller-owned sort projection.
    pub sort: Option<SortDirection>,
}

impl<'a, Id> Column<'a, Id> {
    /// Creates a left-aligned, non-sortable column.
    #[must_use]
    pub fn new(id: Id, title: impl Into<Line<'a>>, width: ColumnWidth) -> Self {
        Self {
            id,
            title: title.into(),
            width,
            alignment: CellAlignment::Left,
            sortable: false,
            sort: None,
        }
    }

    /// Sets cell and header alignment.
    #[must_use]
    pub const fn alignment(mut self, alignment: CellAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Enables sorting and projects the caller-owned direction.
    #[must_use]
    pub const fn sortable(mut self, sort: Option<SortDirection>) -> Self {
        self.sortable = true;
        self.sort = sort;
        self
    }
}

/// Borrowed projection of one table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow<'a, Id> {
    /// Stable row identity.
    pub id: Id,
    /// Styled cells in column order.
    pub cells: &'a [Line<'a>],
    /// Whether selection, activation, and pointer input may reach this row.
    pub enabled: bool,
    /// Whether ordinary rendering uses the semantic accent role.
    pub emphasis: bool,
    /// Optional row-wide style override.
    pub style: Option<Style>,
}

impl<'a, Id> TableRow<'a, Id> {
    /// Creates an enabled row with ordinary semantic emphasis.
    #[must_use]
    pub const fn new(id: Id, cells: &'a [Line<'a>]) -> Self {
        Self {
            id,
            cells,
            enabled: true,
            emphasis: false,
            style: None,
        }
    }

    /// Sets whether interaction may reach the row.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets semantic accent emphasis.
    #[must_use]
    pub const fn emphasis(mut self, emphasis: bool) -> Self {
        self.emphasis = emphasis;
        self
    }

    /// Overrides the row-wide style.
    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }
}

/// Semantic result of table interaction.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TableOutcome<RowId, ColumnId> {
    /// Input did not apply.
    Ignored,
    /// Selection moved to a row.
    Selected(RowId),
    /// The selected row was activated.
    Activated(RowId),
    /// A sortable header requested caller-owned sorting.
    SortRequested(ColumnId),
    /// Interaction requested cancellation.
    Cancelled,
}

/// Painted row geometry used for pointer routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRowRegion<Id> {
    /// Stable row identity.
    pub id: Id,
    /// Projected row index represented by this region.
    pub index: usize,
    /// Painted row rectangle.
    pub area: Rect,
}

/// Painted header geometry used for sort routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableHeaderRegion<Id> {
    /// Stable column identity.
    pub id: Id,
    /// Painted header rectangle.
    pub area: Rect,
    /// Whether the region emits sort requests.
    pub sortable: bool,
}

/// Interaction and viewport state for [`Table`].
///
/// Call [`Self::reconcile`] after mutating a row projection in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableState<RowId, ColumnId> {
    selected: Option<RowId>,
    hovered: Option<RowId>,
    hovered_column: Option<ColumnId>,
    pointer: Option<Position>,
    focused: bool,
    offset: usize,
    viewport_rows: usize,
    previous_index: Option<usize>,
    painted_area: Rect,
    /// Exact enabled row regions from the latest render.
    pub row_regions: Vec<TableRowRegion<RowId>>,
    /// Exact header regions from the latest render.
    pub header_regions: Vec<TableHeaderRegion<ColumnId>>,
    /// Resolved width for every declared column from the latest render.
    pub resolved_widths: Vec<u16>,
    visible_columns: Vec<usize>,
    policies: Vec<ColumnWidth>,
    scratch_widths: Vec<u16>,
    scratch_policies: Vec<ColumnWidth>,
    scratch_text: String,
    validated_rows_ptr: usize,
    validated_rows_len: usize,
    first_row_ids: Vec<bool>,
}

impl<RowId, ColumnId> Default for TableState<RowId, ColumnId> {
    fn default() -> Self {
        Self {
            selected: None,
            hovered: None,
            hovered_column: None,
            pointer: None,
            focused: false,
            offset: 0,
            viewport_rows: 0,
            previous_index: None,
            painted_area: Rect::default(),
            row_regions: Vec::new(),
            header_regions: Vec::new(),
            resolved_widths: Vec::new(),
            visible_columns: Vec::new(),
            policies: Vec::new(),
            scratch_widths: Vec::new(),
            scratch_policies: Vec::new(),
            scratch_text: String::new(),
            validated_rows_ptr: 0,
            validated_rows_len: 0,
            first_row_ids: Vec::new(),
        }
    }
}

impl<RowId: Clone + Eq, ColumnId: Clone + Eq> TableState<RowId, ColumnId> {
    /// Creates state with an optional stable selected identity.
    #[must_use]
    pub fn new(selected: Option<RowId>) -> Self {
        Self {
            selected,
            ..Self::default()
        }
    }

    /// Sets whether this table owns keyboard focus.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Returns whether this table owns keyboard focus.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Returns the selected row identity.
    #[must_use]
    pub const fn selected(&self) -> Option<&RowId> {
        self.selected.as_ref()
    }

    /// Returns the hovered row identity.
    #[must_use]
    pub const fn hovered(&self) -> Option<&RowId> {
        self.hovered.as_ref()
    }

    /// Returns the hovered header identity.
    #[must_use]
    pub const fn hovered_column(&self) -> Option<&ColumnId> {
        self.hovered_column.as_ref()
    }

    /// Returns the first visible body-row offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Reconciles selection after caller sorting, filtering, or replacement.
    ///
    /// Call this after every in-place change to row identity, order, or enabled
    /// state. It also rebuilds first-occurrence routing for stable row IDs.
    pub fn reconcile(&mut self, rows: &[TableRow<'_, RowId>]) {
        self.project_row_identities(rows);
        if let Some(selected) = self.selected.as_ref()
            && let Some(index) = rows
                .iter()
                .position(|row| row.enabled && &row.id == selected)
        {
            self.previous_index = Some(index);
            self.reveal(index, rows.len());
            return;
        }
        let anchor = self.previous_index.unwrap_or(0);
        let Some(index) = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.enabled)
            .min_by_key(|(index, _)| index.abs_diff(anchor))
            .map(|(index, _)| index)
        else {
            self.selected = None;
            self.offset = 0;
            return;
        };
        self.selected = Some(rows[index].id.clone());
        self.previous_index = Some(index);
        self.reveal(index, rows.len());
    }

    /// Handles focused keyboard navigation and semantic activation.
    pub fn handle_key(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        key: KeyEvent,
    ) -> TableOutcome<RowId, ColumnId> {
        if !self.focused || key.kind == KeyEventKind::Release || !key.modifiers.is_empty() {
            return TableOutcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_by(rows, -1, true),
            KeyCode::Down | KeyCode::Char('j') => self.move_by(rows, 1, true),
            KeyCode::Home => self.select_edge(rows, false),
            KeyCode::End => self.select_edge(rows, true),
            KeyCode::PageUp => self.move_by(
                rows,
                -isize::try_from(self.viewport_rows.max(1)).unwrap_or(isize::MAX),
                false,
            ),
            KeyCode::PageDown => self.move_by(
                rows,
                isize::try_from(self.viewport_rows.max(1)).unwrap_or(isize::MAX),
                false,
            ),
            KeyCode::Enter => self
                .selected
                .as_ref()
                .and_then(|id| rows.iter().find(|row| row.enabled && row.id == *id))
                .map(|row| TableOutcome::Activated(row.id.clone()))
                .unwrap_or(TableOutcome::Ignored),
            KeyCode::Esc => TableOutcome::Cancelled,
            _ => TableOutcome::Ignored,
        }
    }

    /// Applies a bounded wheel-style row delta.
    pub fn scroll_by(&mut self, delta: isize, row_count: usize) -> bool {
        let maximum = row_count.saturating_sub(self.viewport_rows);
        let next = self.offset.saturating_add_signed(delta).min(maximum);
        let changed = next != self.offset;
        self.offset = next;
        changed
    }

    /// Updates hover from canonical painted regions.
    pub fn hover(&mut self, position: Position) -> bool {
        self.pointer = Some(position);
        let row = self
            .row_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        let column = self
            .header_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        let changed = row != self.hovered || column != self.hovered_column;
        self.hovered = row;
        self.hovered_column = column;
        changed
    }

    /// Routes a primary click through canonical header and row geometry.
    pub fn click(&mut self, position: Position) -> TableOutcome<RowId, ColumnId> {
        self.pointer = Some(position);
        if let Some(region) = self
            .header_regions
            .iter()
            .find(|region| region.sortable && region.area.contains(position))
        {
            return TableOutcome::SortRequested(region.id.clone());
        }
        if let Some(region) = self
            .row_regions
            .iter()
            .find(|region| region.area.contains(position))
        {
            self.selected = Some(region.id.clone());
            self.previous_index = Some(region.index);
            return TableOutcome::Selected(region.id.clone());
        }
        TableOutcome::Ignored
    }

    /// Routes neutral pointer hover, primary click, and wheel input.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        row_count: usize,
    ) -> TableOutcome<RowId, ColumnId> {
        match event.kind {
            MouseEventKind::Moved => {
                self.hover(event.position);
                TableOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => self.click(event.position),
            MouseEventKind::ScrollUp if self.painted_area.contains(event.position) => {
                self.scroll_by(-1, row_count);
                TableOutcome::Ignored
            }
            MouseEventKind::ScrollDown if self.painted_area.contains(event.position) => {
                self.scroll_by(1, row_count);
                TableOutcome::Ignored
            }
            _ => TableOutcome::Ignored,
        }
    }

    fn move_by(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        delta: isize,
        wrap: bool,
    ) -> TableOutcome<RowId, ColumnId> {
        let enabled_count = rows.iter().filter(|row| row.enabled).count();
        if enabled_count == 0 {
            return TableOutcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|id| {
                rows.iter()
                    .filter(|row| row.enabled)
                    .position(|row| row.id == *id)
            })
            .unwrap_or(0);
        let next = if wrap && delta == -1 && current == 0 {
            enabled_count - 1
        } else if wrap && delta == 1 && current + 1 == enabled_count {
            0
        } else {
            current.saturating_add_signed(delta).min(enabled_count - 1)
        };
        let index = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.enabled)
            .nth(next)
            .map_or(0, |(index, _)| index);
        self.select_index(rows, index)
    }

    fn select_edge(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        last: bool,
    ) -> TableOutcome<RowId, ColumnId> {
        let enabled = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.enabled)
            .map(|(index, _)| index);
        let Some(index) = (if last {
            enabled.last()
        } else {
            enabled.into_iter().next()
        }) else {
            return TableOutcome::Ignored;
        };
        self.select_index(rows, index)
    }

    fn select_index(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        index: usize,
    ) -> TableOutcome<RowId, ColumnId> {
        let id = rows[index].id.clone();
        self.selected = Some(id.clone());
        self.previous_index = Some(index);
        self.reveal(index, rows.len());
        TableOutcome::Selected(id)
    }

    fn reveal(&mut self, index: usize, row_count: usize) {
        if self.viewport_rows == 0 {
            return;
        }
        if index < self.offset {
            self.offset = index;
        } else if index >= self.offset + self.viewport_rows {
            self.offset = index + 1 - self.viewport_rows;
        }
        self.offset = self
            .offset
            .min(row_count.saturating_sub(self.viewport_rows));
    }

    fn project_row_identities(&mut self, rows: &[TableRow<'_, RowId>]) {
        self.first_row_ids.clear();
        self.first_row_ids.reserve(rows.len());
        for (index, row) in rows.iter().enumerate() {
            self.first_row_ids
                .push(rows[..index].iter().all(|previous| previous.id != row.id));
        }
        debug_assert!(
            self.first_row_ids.iter().all(|first| *first),
            "table row IDs must be unique"
        );
        self.validated_rows_ptr = rows.as_ptr() as usize;
        self.validated_rows_len = rows.len();
    }
}

/// Borrowed columnar table renderer.
#[derive(Debug, Clone, Copy)]
pub struct Table<'a, RowId, ColumnId> {
    columns: &'a [Column<'a, ColumnId>],
    rows: &'a [TableRow<'a, RowId>],
    theme: &'a Theme,
    column_gap: u16,
}

impl<'a, RowId, ColumnId> Table<'a, RowId, ColumnId> {
    /// Creates a table from caller-owned columns and rows.
    #[must_use]
    pub const fn new(
        columns: &'a [Column<'a, ColumnId>],
        rows: &'a [TableRow<'a, RowId>],
        theme: &'a Theme,
    ) -> Self {
        Self {
            columns,
            rows,
            theme,
            column_gap: 2,
        }
    }

    /// Overrides the blank gap between visible columns.
    #[must_use]
    pub const fn column_gap(mut self, gap: u16) -> Self {
        self.column_gap = gap;
        self
    }
}

impl<RowId: Clone + Eq, ColumnId: Clone + Eq> StatefulWidget for &Table<'_, RowId, ColumnId> {
    type State = TableState<RowId, ColumnId>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.row_regions.clear();
        state.header_regions.clear();
        state.painted_area = area;
        if state.validated_rows_ptr != self.rows.as_ptr() as usize
            || state.validated_rows_len != self.rows.len()
        {
            state.project_row_identities(self.rows);
        }
        state.viewport_rows = usize::from(area.height.saturating_sub(1));
        state.offset = state
            .offset
            .min(self.rows.len().saturating_sub(state.viewport_rows));
        state.policies.clear();
        state
            .policies
            .extend(self.columns.iter().map(|column| column.width));
        resolve_layout_into(
            &state.policies,
            area.width.saturating_sub(MARKER_WIDTH),
            self.column_gap,
            &mut state.resolved_widths,
            &mut state.visible_columns,
            &mut state.scratch_widths,
            &mut state.scratch_policies,
        );
        debug_assert!(
            self.columns
                .iter()
                .filter(|column| column.sortable && column.sort.is_some())
                .count()
                <= 1,
            "at most one sortable column may show a sort direction"
        );
        debug_assert!(
            self.columns
                .iter()
                .all(|column| column.sort.is_none() || column.sortable),
            "a sorted table column must be sortable"
        );
        debug_assert!(
            self.columns
                .iter()
                .enumerate()
                .all(|(index, column)| self.columns[..index]
                    .iter()
                    .all(|previous| previous.id != column.id)),
            "table column IDs must be unique"
        );
        if area.is_empty() || state.visible_columns.is_empty() {
            return;
        }
        let mut x = area.x.saturating_add(MARKER_WIDTH);
        let mut shown_sort = false;
        for (visible_index, column_index) in state.visible_columns.iter().copied().enumerate() {
            let column = &self.columns[column_index];
            let width = state.resolved_widths[column_index];
            let rect = Rect::new(x, area.y, width, 1);
            let sort = column.sort.filter(|_| column.sortable && !shown_sort);
            shown_sort |= sort.is_some();
            let sort_width = u16::from(sort.is_some()).saturating_mul(2).min(rect.width);
            let title_rect = Rect::new(rect.x, rect.y, rect.width.saturating_sub(sort_width), 1);
            render_line(
                &column.title,
                title_rect,
                column.alignment,
                self.theme.style(Role::TextStrong),
                buffer,
                &mut state.scratch_text,
            );
            if let Some(direction) = sort {
                let sort_x = rect.right().saturating_sub(sort_width);
                buffer.set_stringn(sort_x, rect.y, " ", 1, self.theme.style(Role::TextStrong));
                buffer.set_stringn(
                    sort_x.saturating_add(1),
                    rect.y,
                    sort_glyph(direction),
                    1,
                    self.theme.style(Role::TextStrong),
                );
            }
            if !state
                .header_regions
                .iter()
                .any(|region| region.id == column.id)
            {
                state.header_regions.push(TableHeaderRegion {
                    id: column.id.clone(),
                    area: rect,
                    sortable: column.sortable,
                });
            }
            x = x.saturating_add(width);
            if visible_index + 1 < state.visible_columns.len() {
                x = x.saturating_add(self.column_gap);
            }
        }

        let end = (state.offset + state.viewport_rows).min(self.rows.len());
        let mut selected_painted = false;
        for (painted, row_index) in (state.offset..end).enumerate() {
            let row = &self.rows[row_index];
            debug_assert_eq!(
                row.cells.len(),
                self.columns.len(),
                "table row cell count must match columns"
            );
            let y = area
                .y
                .saturating_add(1 + u16::try_from(painted).unwrap_or(u16::MAX));
            let owns_id = state.first_row_ids.get(row_index).copied().unwrap_or(true);
            let selected = owns_id && !selected_painted && state.selected.as_ref() == Some(&row.id);
            selected_painted |= selected;
            let row_area = Rect::new(area.x, y, area.width, 1);
            let hovered = row.enabled
                && state
                    .pointer
                    .is_some_and(|position| row_area.contains(position));
            let role = if !row.enabled {
                Role::TextDisabled
            } else if selected {
                Role::Selection
            } else if hovered {
                Role::Focus
            } else if row.emphasis {
                Role::Accent
            } else {
                Role::Text
            };
            let style = row.style.unwrap_or_else(|| self.theme.style(role));
            buffer.set_stringn(
                area.x,
                y,
                if selected { "▸ " } else { "  " },
                usize::from(MARKER_WIDTH),
                style,
            );
            let mut x = area.x.saturating_add(MARKER_WIDTH);
            for (visible_index, column_index) in state.visible_columns.iter().copied().enumerate() {
                let rect = Rect::new(x, y, state.resolved_widths[column_index], 1);
                if let Some(value) = row.cells.get(column_index) {
                    render_line(
                        value,
                        rect,
                        self.columns[column_index].alignment,
                        style,
                        buffer,
                        &mut state.scratch_text,
                    );
                } else {
                    buffer.set_style(rect, style);
                }
                x = x.saturating_add(rect.width);
                if visible_index + 1 < state.visible_columns.len() {
                    x = x.saturating_add(self.column_gap);
                }
            }
            if owns_id && row.enabled {
                state.row_regions.push(TableRowRegion {
                    id: row.id.clone(),
                    index: row_index,
                    area: row_area,
                });
            }
        }
        state.hovered = state.pointer.and_then(|position| {
            state
                .row_regions
                .iter()
                .find(|region| region.area.contains(position))
                .map(|region| region.id.clone())
        });
        state.hovered_column = state.pointer.and_then(|position| {
            state
                .header_regions
                .iter()
                .find(|region| region.area.contains(position))
                .map(|region| region.id.clone())
        });
    }
}

impl<RowId: Clone + Eq, ColumnId: Clone + Eq> StatefulWidget for Table<'_, RowId, ColumnId> {
    type State = TableState<RowId, ColumnId>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self).render(area, buffer, state);
    }
}

/// Resolves column widths against a cell budget.
#[must_use]
pub fn resolve_widths(columns: &[ColumnWidth], available: u16) -> Vec<u16> {
    let mut widths = Vec::with_capacity(columns.len());
    resolve_widths_into(columns, available, &mut widths);
    widths
}

fn resolve_widths_into(columns: &[ColumnWidth], available: u16, widths: &mut Vec<u16>) {
    widths.clear();
    widths.extend(columns.iter().map(|column| match column {
        ColumnWidth::Fixed(width) | ColumnWidth::Min(width) => *width,
        ColumnWidth::Fill(_) => 0,
    }));
    let mandatory = widths.iter().map(|width| u64::from(*width)).sum::<u64>();
    if mandatory > u64::from(available) {
        let mut deficit = mandatory - u64::from(available);
        shrink(columns, widths, &mut deficit, false);
        shrink(columns, widths, &mut deficit, true);
        return;
    }
    let remainder = u64::from(available) - mandatory;
    let total_weight = columns
        .iter()
        .map(|column| match column {
            ColumnWidth::Fill(weight) => u64::from(weight.get()),
            _ => 0,
        })
        .sum::<u64>();
    if remainder == 0 || total_weight == 0 {
        return;
    }
    let mut distributed = 0;
    for (index, column) in columns.iter().enumerate() {
        if let ColumnWidth::Fill(weight) = column {
            let share = remainder * u64::from(weight.get()) / total_weight;
            widths[index] = u16::try_from(share).unwrap_or(u16::MAX);
            distributed += share;
        }
    }
    let mut leftover = remainder - distributed;
    for (index, column) in columns.iter().enumerate() {
        if leftover == 0 {
            break;
        }
        if matches!(column, ColumnWidth::Fill(_)) {
            widths[index] += 1;
            leftover -= 1;
        }
    }
}

fn shrink(columns: &[ColumnWidth], widths: &mut [u16], deficit: &mut u64, fixed: bool) {
    for (index, column) in columns.iter().enumerate().rev() {
        let eligible = matches!(column, ColumnWidth::Fixed(_) if fixed)
            || matches!(column, ColumnWidth::Min(_) if !fixed);
        if eligible && *deficit > 0 {
            let amount = u64::from(widths[index]).min(*deficit);
            widths[index] -= amount as u16;
            *deficit -= amount;
        }
    }
}

fn resolve_layout_into(
    columns: &[ColumnWidth],
    available: u16,
    gap: u16,
    widths: &mut Vec<u16>,
    visible: &mut Vec<usize>,
    scratch: &mut Vec<u16>,
    scratch_policies: &mut Vec<ColumnWidth>,
) {
    visible.clear();
    visible.extend(
        columns
            .iter()
            .enumerate()
            .filter_map(|(index, width)| match width {
                ColumnWidth::Fixed(0) | ColumnWidth::Min(0) => None,
                _ => Some(index),
            }),
    );
    widths.clear();
    widths.resize(columns.len(), 0);
    if visible.is_empty() || available == 0 {
        visible.clear();
        return;
    }
    solve_visible(columns, visible, available, gap, scratch, scratch_policies);
    let mut position = 0;
    visible.retain(|_| {
        let keep = scratch[position] > 0;
        position += 1;
        keep
    });
    if visible.is_empty() {
        visible.extend(
            columns
                .iter()
                .enumerate()
                .find_map(|(index, policy)| match policy {
                    ColumnWidth::Fixed(0) | ColumnWidth::Min(0) => None,
                    _ => Some(index),
                }),
        );
    }
    solve_visible(columns, visible, available, gap, scratch, scratch_policies);
    for (index, width) in visible.iter().zip(scratch.iter().copied()) {
        widths[*index] = width;
    }
}

fn solve_visible(
    columns: &[ColumnWidth],
    visible: &[usize],
    available: u16,
    gap: u16,
    scratch: &mut Vec<u16>,
    policies: &mut Vec<ColumnWidth>,
) {
    let gaps =
        gap.saturating_mul(u16::try_from(visible.len().saturating_sub(1)).unwrap_or(u16::MAX));
    policies.clear();
    policies.extend(visible.iter().map(|index| columns[*index]));
    resolve_widths_into(policies, available.saturating_sub(gaps), scratch);
}

fn render_line(
    line: &Line<'_>,
    area: Rect,
    alignment: CellAlignment,
    style: Style,
    buffer: &mut Buffer,
    scratch: &mut String,
) {
    if area.is_empty() {
        return;
    }
    buffer.set_style(area, style);
    let line_width = line
        .spans
        .iter()
        .map(|span| crate::text::display_cols(span.content.as_ref()))
        .sum::<usize>();
    let painted = u16::try_from(line_width)
        .unwrap_or(u16::MAX)
        .min(area.width);
    let left = match alignment {
        CellAlignment::Left => 0,
        CellAlignment::Center => area.width.saturating_sub(painted) / 2,
        CellAlignment::Right => area.width.saturating_sub(painted),
    };
    let available = usize::from(area.width.saturating_sub(left));
    let mut logical_col = 0usize;
    let mut painted_col = 0usize;
    for span in &line.spans {
        if logical_col >= available {
            break;
        }
        let span_width = crate::text::display_cols(span.content.as_ref());
        crate::text::display_cols_slice_into(
            span.content.as_ref(),
            0,
            available - logical_col,
            scratch,
        );
        let scratch_width = crate::text::display_cols(scratch);
        buffer.set_stringn(
            area.x
                .saturating_add(left)
                .saturating_add(u16::try_from(painted_col).unwrap_or(u16::MAX)),
            area.y,
            scratch.as_str(),
            available.saturating_sub(painted_col),
            style.patch(span.style),
        );
        painted_col += scratch_width;
        logical_col += span_width;
    }
}

const fn sort_glyph(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "▲",
        SortDirection::Descending => "▼",
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::{style::Color, text::Span};

    use crate::input::KeyModifiers;

    use super::*;
    fn fill(weight: u16) -> ColumnWidth {
        ColumnWidth::Fill(NonZeroU16::new(weight).unwrap())
    }

    #[test]
    fn solver_contract_is_deterministic() {
        let cases: &[(&[ColumnWidth], u16, &[u16])] = &[
            (&[], 10, &[]),
            (&[fill(1)], 0, &[0]),
            (&[fill(1)], 7, &[7]),
            (&[fill(1), fill(1)], 5, &[3, 2]),
            (&[fill(1), fill(2)], 9, &[3, 6]),
            (&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 7, &[4, 3]),
            (
                &[ColumnWidth::Fixed(4), fill(1), ColumnWidth::Min(3)],
                12,
                &[4, 5, 3],
            ),
            (&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 6, &[4, 2]),
            (&[ColumnWidth::Fixed(4), ColumnWidth::Min(3)], 3, &[3, 0]),
            (&[ColumnWidth::Min(100)], 9, &[9]),
            (
                &[
                    ColumnWidth::Min(2),
                    ColumnWidth::Min(3),
                    ColumnWidth::Min(4),
                ],
                6,
                &[2, 3, 1],
            ),
            (&[ColumnWidth::Fixed(0), fill(2)], 7, &[0, 7]),
            (
                &[fill(u16::MAX), fill(u16::MAX), fill(u16::MAX)],
                u16::MAX,
                &[21_845, 21_845, 21_845],
            ),
            (
                &[
                    ColumnWidth::Fixed(2),
                    ColumnWidth::Fixed(3),
                    ColumnWidth::Fixed(4),
                ],
                4,
                &[2, 2, 0],
            ),
        ];
        for (columns, available, expected) in cases {
            assert_eq!(resolve_widths(columns, *available), *expected);
        }
    }

    #[test]
    fn layout_collapses_rightmost_columns_without_phantom_gaps() {
        let mut resolved = Vec::new();
        let mut visible = Vec::new();
        let mut policies = Vec::new();
        let mut scratch = Vec::new();

        resolve_layout_into(
            &[ColumnWidth::Fixed(4), ColumnWidth::Min(3)],
            5,
            2,
            &mut resolved,
            &mut visible,
            &mut policies,
            &mut scratch,
        );
        assert_eq!(resolved, [4, 0]);
        assert_eq!(visible, [0]);

        resolve_layout_into(
            &[fill(1), fill(1)],
            2,
            2,
            &mut resolved,
            &mut visible,
            &mut policies,
            &mut scratch,
        );
        assert_eq!(resolved, [2, 0]);
        assert_eq!(visible, [0]);
    }

    fn columns() -> [Column<'static, &'static str>; 3] {
        [
            Column {
                id: "name",
                title: Line::from("Name"),
                width: ColumnWidth::Fixed(8),
                alignment: CellAlignment::Left,
                sortable: true,
                sort: None,
            },
            Column {
                id: "region",
                title: Line::from("Region"),
                width: ColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
                alignment: CellAlignment::Center,
                sortable: false,
                sort: None,
            },
            Column {
                id: "cpu",
                title: Line::from("CPU"),
                width: ColumnWidth::Fixed(6),
                alignment: CellAlignment::Right,
                sortable: true,
                sort: Some(SortDirection::Descending),
            },
        ]
    }

    fn cells() -> [[Line<'static>; 3]; 4] {
        [
            [
                Line::from(Span::styled("alpha", Style::default().fg(Color::Red))),
                Line::from("東京🧪"),
                Line::from("10%"),
            ],
            [
                Line::from("disabled"),
                Line::from("west"),
                Line::from("20%"),
            ],
            [Line::from("gamma"), Line::from("north"), Line::from("30%")],
            [Line::from("delta"), Line::from("south"), Line::from("40%")],
        ]
    }

    fn rows<'a>(cells: &'a [[Line<'static>; 3]; 4]) -> [TableRow<'a, u8>; 4] {
        [
            TableRow {
                id: 1,
                cells: &cells[0],
                enabled: true,
                emphasis: false,
                style: None,
            },
            TableRow {
                id: 2,
                cells: &cells[1],
                enabled: false,
                emphasis: false,
                style: None,
            },
            TableRow {
                id: 3,
                cells: &cells[2],
                enabled: true,
                emphasis: true,
                style: None,
            },
            TableRow {
                id: 4,
                cells: &cells[3],
                enabled: true,
                emphasis: false,
                style: None,
            },
        ]
    }

    #[test]
    fn render_preserves_styles_alignment_unicode_and_canonical_regions() {
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let mut state = TableState::new(Some(1));
        state.set_focused(true);
        let area = Rect::new(0, 0, 30, 4);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);

        assert_eq!(state.header_regions.len(), 3);
        assert_eq!(
            state
                .row_regions
                .iter()
                .map(|region| region.id)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(buffer[(0, 1)].symbol(), "▸");
        assert_eq!(buffer[(2, 1)].fg, Color::Red);
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("CPU ▼"));
        assert!(text.contains("東 京 🧪"));
        assert!(
            state
                .header_regions
                .windows(2)
                .all(|pair| pair[0].area.right() < pair[1].area.x)
        );
    }

    #[test]
    fn keyboard_skips_disabled_wraps_pages_activates_and_respects_focus_modifiers() {
        let cells = cells();
        let rows = rows(&cells);
        let mut state = TableState::<u8, &str>::new(Some(1));
        state.viewport_rows = 2;
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            TableOutcome::Ignored
        );
        state.set_focused(true);
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            TableOutcome::Selected(3)
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
            TableOutcome::Selected(4)
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            TableOutcome::Selected(1)
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE)),
            TableOutcome::Selected(4)
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TableOutcome::Activated(4)
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            TableOutcome::Cancelled
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL)),
            TableOutcome::Ignored
        );

        state.selected = Some(2);
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TableOutcome::Ignored
        );
        state.selected = Some(99);
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TableOutcome::Ignored
        );
    }

    #[test]
    fn pointer_uses_only_painted_enabled_rows_and_sortable_headers() {
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let area = Rect::new(5, 7, 30, 5);
        let mut state = TableState::new(Some(1));
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);
        let cpu = state
            .header_regions
            .iter()
            .find(|region| region.id == "cpu")
            .unwrap()
            .area;
        assert_eq!(
            state.click(Position::new(cpu.x, cpu.y)),
            TableOutcome::SortRequested("cpu")
        );
        let inert = state
            .header_regions
            .iter()
            .find(|region| region.id == "region")
            .unwrap()
            .area;
        assert_eq!(
            state.click(Position::new(inert.x, inert.y)),
            TableOutcome::Ignored
        );
        assert_eq!(
            state.click(Position::new(area.x, area.y + 2)),
            TableOutcome::Ignored
        );
        assert_eq!(
            state.click(Position::new(area.x, area.y + 3)),
            TableOutcome::Selected(3)
        );
        assert!(state.hover(Position::new(cpu.x, cpu.y)));
        assert_eq!(state.hovered_column(), Some(&"cpu"));

        state.offset = 1;
        let outside_wheel = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            position: Position::new(0, 0),
            modifiers: KeyModifiers::NONE,
        };
        let _ = state.handle_mouse(outside_wheel, rows.len());
        assert_eq!(state.offset(), 1);
    }

    #[test]
    fn hovered_enabled_row_uses_semantic_focus_style() {
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 30, 4);
        let mut state = TableState::new(None);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &theme)).render(area, &mut buffer, &mut state);
        state.hover(Position::new(0, 3));
        (&Table::new(&columns, &rows, &theme)).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 3)].fg, theme.style(Role::Focus).fg.unwrap());
        state.scroll_by(1, rows.len());
        (&Table::new(&columns, &rows, &theme)).render(area, &mut buffer, &mut state);
        assert_eq!(state.hovered(), Some(&4));
        assert_eq!(buffer[(0, 3)].fg, theme.style(Role::Focus).fg.unwrap());
    }

    #[test]
    fn reconcile_is_id_sticky_then_nearest_enabled_index() {
        let cells = cells();
        let mut rows = rows(&cells);
        let mut state = TableState::<u8, &str>::new(Some(3));
        state.reconcile(&rows);
        rows.swap(0, 2);
        state.reconcile(&rows);
        assert_eq!(state.selected(), Some(&3));
        rows[0].enabled = false;
        state.reconcile(&rows);
        assert_eq!(state.selected(), Some(&1));

        let mut state = TableState::<u8, &str>::new(Some(4));
        state.reconcile(&rows);
        rows[0].enabled = false;
        rows[1].enabled = false;
        rows[3].enabled = false;
        state.reconcile(&rows);
        assert_eq!(state.selected(), Some(&1));
    }

    #[test]
    #[should_panic(expected = "sorted table column must be sortable")]
    fn rejects_sort_direction_on_inert_column_in_debug_builds() {
        let mut columns = columns();
        columns[1].sort = Some(SortDirection::Ascending);
        let rows = [];
        let mut state = TableState::<u8, &str>::default();
        let area = Rect::new(0, 0, 30, 2);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);
    }

    #[test]
    #[should_panic(expected = "table column IDs must be unique")]
    fn rejects_duplicate_column_ids_in_debug_builds() {
        let mut columns = columns();
        columns[1].id = "name";
        let rows = [];
        let mut state = TableState::<u8, &str>::default();
        let area = Rect::new(0, 0, 30, 2);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);
    }

    #[test]
    #[should_panic(expected = "table row IDs must be unique")]
    fn rejects_duplicate_painted_row_ids_in_debug_builds() {
        let columns = columns();
        let cells = cells();
        let mut rows = rows(&cells);
        rows[2].id = 1;
        let mut state = TableState::default();
        let area = Rect::new(0, 0, 30, 5);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);
    }

    #[test]
    fn clipping_preserves_combining_clusters_and_rejects_partial_wide_graphemes() {
        let columns = [Column {
            id: "value",
            title: Line::from("V"),
            width: ColumnWidth::Fixed(1),
            alignment: CellAlignment::Left,
            sortable: false,
            sort: None,
        }];
        let cells = [
            [Line::from("e\u{301}")],
            [Line::from("🧪")],
            [Line::from("a\u{7}b")],
        ];
        let rows = [
            TableRow::new(1, &cells[0]),
            TableRow::new(2, &cells[1]),
            TableRow::new(3, &cells[2]),
        ];
        let mut state = TableState::default();
        let area = Rect::new(0, 0, 3, 4);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(2, 1)].symbol(), "e\u{301}");
        assert_eq!(buffer[(2, 2)].symbol(), " ");
        assert_eq!(buffer[(2, 3)].symbol(), "a");
    }

    #[test]
    fn empty_zero_and_narrow_tables_are_safe_and_remove_phantom_gaps() {
        let columns = [
            Column {
                id: 0,
                title: Line::from("hidden"),
                width: ColumnWidth::Fixed(0),
                alignment: CellAlignment::Left,
                sortable: false,
                sort: None,
            },
            Column {
                id: 1,
                title: Line::from("visible"),
                width: fill(1),
                alignment: CellAlignment::Left,
                sortable: false,
                sort: None,
            },
        ];
        let rows: [TableRow<'_, u8>; 0] = [];
        for area in [Rect::new(0, 0, 0, 0), Rect::new(0, 0, 3, 1)] {
            let mut state = TableState::default();
            let mut buffer = Buffer::empty(area);
            (&Table::new(&columns, &rows, &Theme::default())).render(area, &mut buffer, &mut state);
            assert!(state.row_regions.is_empty());
            assert!(
                state
                    .resolved_widths
                    .first()
                    .is_none_or(|width| *width == 0)
            );
        }
    }
}
