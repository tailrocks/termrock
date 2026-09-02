//! **Table** — polished static / moderate-size columnar presentation.
//!
//! **Mission.** Headers, alignment, widths, truncation, row focus/selection,
//! empty/loading/error, responsive column priorities, quiet/bordered/striped/
//! compact recipes, sticky header, and horizontal scroll. Display-oriented:
//! **not** the interactive kit in [`super::DataTable`] (cursor, sort/filter
//! execution, 1M virtual windows).
//!
//! Research: Rich tables, Glow, DB clients, btop, TermRock DataTable.
use std::num::NonZeroU16;

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    text::Line,
    widgets::StatefulWidget,
};

use super::data_view::ColumnKind;
use crate::{
    input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{LinePlacement, paint_line_overflow},
};

pub use crate::text::{CellAlignment, CellOverflow};

const MARKER_WIDTH: u16 = 2;

/// Presentation recipe (visual chrome without domain noise).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TableRecipe {
    /// Minimal chrome: gutter selection, no grid lines (default).
    #[default]
    Quiet,
    /// Light column/header separators (`│` / rule under header).
    Bordered,
    /// Alternate-row dimming via spacing role (no heavy fill).
    Striped,
    /// Tight gap + compact density feel.
    Compact,
}

impl TableRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Bordered => "bordered",
            Self::Striped => "striped",
            Self::Compact => "compact",
        }
    }

    /// Default inter-column gap.
    #[must_use]
    pub const fn default_gap(self) -> u16 {
        match self {
            Self::Compact => 1,
            _ => 2,
        }
    }
}

/// Load / empty presentation for the table body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TableBodyState {
    /// Show rows (or empty message if no rows).
    #[default]
    Ready,
    /// Body loading placeholder.
    Loading,
    /// Body error placeholder.
    Error,
}

impl TableBodyState {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Error => "error",
        }
    }
}

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

/// Visible sort direction for a sortable column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SortDirection {
    /// Ascending order, rendered as `↑` / `^`.
    Ascending,
    /// Descending order, rendered as `↓` / `v`.
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
    /// Drop priority under width pressure (higher kept longer; default 50).
    pub priority: u8,
    /// What the column holds, which decides its tone.
    pub kind: ColumnKind,
}

impl<'a, Id> Column<'a, Id> {
    /// Creates a left-aligned, non-sortable column (priority 50).
    #[must_use]
    pub fn new(id: Id, title: impl Into<Line<'a>>, width: ColumnWidth) -> Self {
        Self {
            id,
            title: title.into(),
            width,
            alignment: CellAlignment::Left,
            sortable: false,
            sort: None,
            priority: 50,
            kind: ColumnKind::Text,
        }
    }

    /// States what the column holds, which decides its tone.
    ///
    /// A numeric column reads quieter than the name beside it; a status
    /// column contracts to its letter instead of to an ellipsis.
    #[must_use]
    pub const fn kind(mut self, kind: ColumnKind) -> Self {
        self.kind = kind;
        self
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

    /// Responsive priority (higher = survive longer when columns must drop).
    #[must_use]
    pub const fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
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
    /// Optional leading status/icon painted before the first cell.
    pub leading: Option<Line<'a>>,
    /// Optional trailing badge after the last cell (composed badge).
    pub badge: Option<Line<'a>>,
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
            leading: None,
            badge: None,
            enabled: true,
            emphasis: false,
            style: None,
        }
    }

    /// Projects identity anatomy for narrow / status chrome.
    #[must_use]
    pub fn composed(&self) -> super::ComposedRow<'a, ()>
    where
        Id: Clone,
    {
        let primary = self
            .cells
            .first()
            .cloned()
            .unwrap_or_else(|| Line::from(""));
        super::ComposedRow {
            id: (),
            leading: self.leading.clone(),
            primary,
            secondary: self.cells.get(1).cloned(),
            badge: self
                .badge
                .clone()
                .or_else(|| self.cells.last().filter(|_| self.cells.len() > 2).cloned()),
            shortcut: None,
            enabled: self.enabled,
            loading: false,
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
    /// Focused cell column within the selected row (optional cell navigation).
    focused_column: Option<ColumnId>,
    pointer: Option<Position>,
    offset: usize,
    /// Horizontal content offset in display columns (sticky header stays put).
    h_offset: u16,
    viewport_rows: usize,
    /// Last painted body content width (columns + gaps).
    content_width: u16,
    /// Last painted horizontal budget for columns (area minus gutter).
    viewport_width: u16,
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
    priorities: Vec<u8>,
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
            focused_column: None,
            pointer: None,
            offset: 0,
            h_offset: 0,
            viewport_rows: 0,
            content_width: 0,
            viewport_width: 0,
            previous_index: None,
            painted_area: Rect::default(),
            row_regions: Vec::new(),
            header_regions: Vec::new(),
            resolved_widths: Vec::new(),
            visible_columns: Vec::new(),
            policies: Vec::new(),
            priorities: Vec::new(),
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

    /// Returns the focused cell column identity.
    #[must_use]
    pub const fn focused_column(&self) -> Option<&ColumnId> {
        self.focused_column.as_ref()
    }

    /// Sets the focused cell column (row selection stays independent).
    pub fn set_focused_column(&mut self, column: Option<ColumnId>) {
        self.focused_column = column;
    }

    /// Returns the first visible body-row offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Horizontal content scroll offset in display columns.
    #[must_use]
    pub const fn h_offset(&self) -> u16 {
        self.h_offset
    }

    /// Sets horizontal content scroll (clamped on next paint).
    pub fn set_h_offset(&mut self, offset: u16) {
        self.h_offset = offset;
    }

    /// Scrolls horizontally by `delta` display columns. Returns whether offset changed.
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

    /// Indices of columns kept after the last layout pass (declaration order).
    #[must_use]
    pub fn visible_column_indices(&self) -> &[usize] {
        &self.visible_columns
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
    ///
    /// Keys are mapped through [`crate::interaction::default_table_intent`]; activation is
    /// Press-only so held Enter cannot multi-fire.
    pub fn handle_key(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        key: KeyEvent,
    ) -> TableOutcome<RowId, ColumnId> {
        if key.kind == KeyEventKind::Release || !key.modifiers.is_empty() {
            return TableOutcome::Ignored;
        }
        let Some(intent) = crate::interaction::default_table_intent(key) else {
            return TableOutcome::Ignored;
        };
        if matches!(intent, crate::interaction::UiIntent::Activate)
            && key.kind != KeyEventKind::Press
        {
            return TableOutcome::Ignored;
        }
        self.handle_intent(rows, intent)
    }

    /// Applies a semantic collection intent.
    ///
    /// Left/Right: when a cell is focused (or becomes focused), move column focus among
    /// visible columns; otherwise scroll horizontally when content overflows.
    pub fn handle_intent(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        intent: crate::interaction::UiIntent,
    ) -> TableOutcome<RowId, ColumnId> {
        use crate::interaction::{NavigationMove, PageMove, UiIntent};
        match intent {
            UiIntent::Move(NavigationMove::Previous) | UiIntent::Move(NavigationMove::Up) => {
                self.move_by(rows, -1, true)
            }
            UiIntent::Move(NavigationMove::Next) | UiIntent::Move(NavigationMove::Down) => {
                self.move_by(rows, 1, true)
            }
            UiIntent::Move(NavigationMove::First) => self.select_edge(rows, false),
            UiIntent::Move(NavigationMove::Last) => self.select_edge(rows, true),
            UiIntent::Move(NavigationMove::Left) | UiIntent::Collapse => {
                self.horizontal_nav(rows, -1)
            }
            UiIntent::Move(NavigationMove::Right) | UiIntent::Expand => {
                self.horizontal_nav(rows, 1)
            }
            UiIntent::Page(PageMove::Backward) => self.move_by(
                rows,
                -isize::try_from(self.viewport_rows.max(1)).unwrap_or(isize::MAX),
                false,
            ),
            UiIntent::Page(PageMove::Forward) => self.move_by(
                rows,
                isize::try_from(self.viewport_rows.max(1)).unwrap_or(isize::MAX),
                false,
            ),
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => self
                .selected
                .as_ref()
                .and_then(|id| rows.iter().find(|row| row.enabled && row.id == *id))
                .map(|row| TableOutcome::Activated(row.id.clone()))
                .unwrap_or(TableOutcome::Ignored),
            UiIntent::Cancel | UiIntent::Close => TableOutcome::Cancelled,
            UiIntent::Toggle => TableOutcome::Ignored,
            _ => TableOutcome::Ignored,
        }
    }

    fn horizontal_nav(
        &mut self,
        rows: &[TableRow<'_, RowId>],
        delta: i16,
    ) -> TableOutcome<RowId, ColumnId> {
        // Prefer cell focus when already active or when columns are available and
        // the host has selected a row; otherwise H-scroll.
        let can_cell = self.selected.is_some()
            && !self.header_regions.is_empty()
            && rows
                .iter()
                .any(|row| row.enabled && self.selected.as_ref().is_some_and(|id| &row.id == id));
        if can_cell && (self.focused_column.is_some() || self.content_width <= self.viewport_width)
        {
            return self.move_cell_focus(delta);
        }
        if self.scroll_horizontal(delta) {
            // Geometry-only change; selection unchanged.
            return TableOutcome::Ignored;
        }
        if can_cell {
            return self.move_cell_focus(delta);
        }
        TableOutcome::Ignored
    }

    fn move_cell_focus(&mut self, delta: i16) -> TableOutcome<RowId, ColumnId> {
        if self.header_regions.is_empty() {
            return TableOutcome::Ignored;
        }
        let current = self.focused_column.as_ref().and_then(|id| {
            self.header_regions
                .iter()
                .position(|region| &region.id == id)
        });
        let next = match current {
            Some(index) if delta < 0 => index.saturating_sub(1),
            Some(index) => (index + 1).min(self.header_regions.len() - 1),
            None if delta < 0 => self.header_regions.len() - 1,
            None => 0,
        };
        let id = self.header_regions[next].id.clone();
        if self.focused_column.as_ref() == Some(&id) {
            // At edge: fall through to H-scroll when overflow remains.
            let _ = self.scroll_horizontal(delta);
            return TableOutcome::Ignored;
        }
        self.focused_column = Some(id);
        TableOutcome::Ignored
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

/// Borrowed columnar table renderer (display / moderate-size; not [`super::DataTable`]).
#[derive(Debug, Clone)]
pub struct Table<'a, RowId, ColumnId> {
    focused: bool,
    columns: &'a [Column<'a, ColumnId>],
    rows: &'a [TableRow<'a, RowId>],
    tokens: &'a crate::style::DesignSystem,
    /// `None` uses the recipe default gap.
    column_gap: Option<u16>,
    recipe: TableRecipe,
    body_state: TableBodyState,
    sticky_header: bool,
    overflow: CellOverflow,
    empty_message: Option<Line<'a>>,
    loading_message: Option<Line<'a>>,
    error_message: Option<Line<'a>>,
}

impl<'a, RowId, ColumnId> Table<'a, RowId, ColumnId> {
    /// Creates a quiet table from caller-owned columns and rows.
    #[must_use]
    pub const fn new(
        columns: &'a [Column<'a, ColumnId>],
        rows: &'a [TableRow<'a, RowId>],
        tokens: &'a crate::style::DesignSystem,
    ) -> Self {
        Self {
            focused: true,
            columns,
            rows,
            tokens,
            column_gap: None,
            recipe: TableRecipe::Quiet,
            body_state: TableBodyState::Ready,
            sticky_header: true,
            overflow: CellOverflow::Clip,
            empty_message: None,
            loading_message: None,
            error_message: None,
        }
    }

    /// Whether this surface owns keyboard focus this frame (host / scene).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Presentation recipe (quiet / bordered / striped / compact).
    #[must_use]
    pub const fn recipe(mut self, recipe: TableRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Body load presentation (ready / loading / error).
    #[must_use]
    pub const fn body_state(mut self, body_state: TableBodyState) -> Self {
        self.body_state = body_state;
        self
    }

    /// Keep the header row pinned while the body scrolls (default true).
    #[must_use]
    pub const fn sticky_header(mut self, sticky: bool) -> Self {
        self.sticky_header = sticky;
        self
    }

    /// Cell overflow policy.
    #[must_use]
    pub const fn overflow(mut self, overflow: CellOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Overrides the blank gap between visible columns (else recipe default).
    #[must_use]
    pub const fn column_gap(mut self, gap: u16) -> Self {
        self.column_gap = Some(gap);
        self
    }

    /// Message when ready and the row projection is empty.
    #[must_use]
    pub fn empty_message(mut self, message: Line<'a>) -> Self {
        self.empty_message = Some(message);
        self
    }

    /// Message when [`TableBodyState::Loading`].
    #[must_use]
    pub fn loading_message(mut self, message: Line<'a>) -> Self {
        self.loading_message = Some(message);
        self
    }

    /// Message when [`TableBodyState::Error`].
    #[must_use]
    pub fn error_message(mut self, message: Line<'a>) -> Self {
        self.error_message = Some(message);
        self
    }

    fn effective_gap(&self) -> u16 {
        self.column_gap.unwrap_or(self.recipe.default_gap())
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

        let gap = self.effective_gap();
        let bordered = matches!(self.recipe, TableRecipe::Bordered);
        let header_h = if self.sticky_header {
            if bordered && area.height >= 2 {
                2
            } else if area.height >= 1 {
                1
            } else {
                0
            }
        } else {
            0
        };
        state.viewport_rows = usize::from(area.height.saturating_sub(header_h));
        state.offset = state
            .offset
            .min(self.rows.len().saturating_sub(state.viewport_rows));

        state.policies.clear();
        state
            .policies
            .extend(self.columns.iter().map(|column| column.width));
        state.priorities.clear();
        state
            .priorities
            .extend(self.columns.iter().map(|column| column.priority));
        let column_budget = area.width.saturating_sub(MARKER_WIDTH);
        state.viewport_width = column_budget;
        resolve_layout_into(
            &state.policies,
            &state.priorities,
            column_budget,
            gap,
            &mut state.resolved_widths,
            &mut state.visible_columns,
            &mut state.scratch_widths,
            &mut state.scratch_policies,
        );
        state.content_width = content_width(&state.visible_columns, &state.resolved_widths, gap);
        let max_h = state.content_width.saturating_sub(column_budget);
        state.h_offset = state.h_offset.min(max_h);

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
        if area.is_empty() {
            return;
        }

        if self.sticky_header && header_h > 0 && !state.visible_columns.is_empty() {
            paint_header_row(self, area, buffer, state, gap, bordered);
        }

        let body_y = area.y.saturating_add(header_h);
        let body_h = area.height.saturating_sub(header_h);
        if body_h == 0 {
            return;
        }

        // Loading / error / empty body messages (header may still show).
        let placeholder = match self.body_state {
            TableBodyState::Loading => Some((
                self.loading_message
                    .clone()
                    .unwrap_or_else(|| Line::from("Loading…")),
                Role::TextMuted,
            )),
            TableBodyState::Error => Some((
                self.error_message
                    .clone()
                    .unwrap_or_else(|| Line::from("Failed to load")),
                Role::Danger,
            )),
            TableBodyState::Ready if self.rows.is_empty() => self
                .empty_message
                .as_ref()
                .map(|m| (m.clone(), Role::TextMuted)),
            TableBodyState::Ready => None,
        };
        if let Some((message, role)) = placeholder {
            let msg_area = Rect::new(
                area.x.saturating_add(MARKER_WIDTH),
                body_y,
                column_budget,
                1,
            );
            render_line(
                &message,
                msg_area,
                CellAlignment::Left,
                self.tokens.palette.style(role),
                buffer,
                &mut state.scratch_text,
            );
            return;
        }

        if state.visible_columns.is_empty() {
            return;
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
            let y = body_y.saturating_add(u16::try_from(painted).unwrap_or(u16::MAX));
            let owns_id = state.first_row_ids.get(row_index).copied().unwrap_or(true);
            let selected = owns_id && !selected_painted && state.selected.as_ref() == Some(&row.id);
            selected_painted |= selected;
            let row_area = Rect::new(area.x, y, area.width, 1);
            let hovered = row.enabled
                && state
                    .pointer
                    .is_some_and(|position| row_area.contains(position));
            let striped = matches!(self.recipe, TableRecipe::Striped) && painted % 2 == 1;
            let chrome = super::row_chrome::RowChrome::resolve(
                self.tokens,
                ListRowVisualState {
                    selected,
                    focused: selected && self.focused,
                    hovered,
                    enabled: row.enabled,
                    loading: false,
                    checked: false,

                    ..ListRowVisualState::default()
                },
            );
            let style = row_style(self.tokens, row, &chrome, striped);
            let quiet = if !row.enabled || row.style.is_some() {
                style
            } else {
                chrome.secondary_style(style)
            };
            // The wash goes down before the cells, so a cell that states
            // itself with an explicit pair (the cursor) keeps its ground.
            chrome.paint_wash(buffer, row_area);

            // Shared responsive anatomy (ContentPriority), not magic width cutoffs.
            let (show_leading_tier, show_badge_tier) =
                crate::layout::table_row_shows_optional(area.width);
            let show_leading = row.leading.is_some() && show_leading_tier;
            let show_badge = row.badge.is_some() && show_badge_tier;
            let mut content_x = area.x.saturating_add(MARKER_WIDTH);
            if show_leading && let Some(leading) = row.leading.as_ref() {
                let lw = u16::try_from(leading.width())
                    .unwrap_or(u16::MAX)
                    .min(area.right().saturating_sub(content_x));
                if lw > 0 {
                    buffer.set_style(Rect::new(content_x, y, lw, 1), style);
                    buffer.set_line(content_x, y, leading, lw);
                    content_x = content_x.saturating_add(lw).saturating_add(1);
                }
            }
            let badge_reserve = if show_badge {
                row.badge
                    .as_ref()
                    .map(|b| {
                        u16::try_from(b.width())
                            .unwrap_or(u16::MAX)
                            .saturating_add(1)
                    })
                    .unwrap_or(0)
                    .min(area.right().saturating_sub(content_x))
            } else {
                0
            };
            let columns_right = area.right().saturating_sub(badge_reserve);
            paint_data_cells(
                self,
                buffer,
                state,
                row,
                y,
                content_x,
                columns_right,
                gap,
                style,
                quiet,
                selected,
            );
            if show_badge && let Some(badge) = row.badge.as_ref() {
                let bw = u16::try_from(badge.width())
                    .unwrap_or(u16::MAX)
                    .min(area.width);
                if bw > 0 {
                    let bx = area.right().saturating_sub(bw);
                    buffer.set_style(Rect::new(bx, y, bw, 1), quiet);
                    buffer.set_line(bx, y, badge, bw);
                }
            }
            chrome.paint_gutter(buffer, row_area);
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

fn row_style(
    tokens: &DesignSystem,
    row: &TableRow<'_, impl Clone>,
    chrome: &super::row_chrome::RowChrome,
    striped: bool,
) -> Style {
    let base = if let Some(style) = row.style {
        style
    } else if !row.enabled {
        tokens.palette.style(Role::TextDisabled)
    } else if row.emphasis {
        tokens.palette.style(Role::Accent)
    } else if striped {
        tokens.palette.style(Role::TextMuted)
    } else {
        tokens.palette.style(Role::Text)
    };
    chrome.label_style(base)
}

fn content_width(visible: &[usize], widths: &[u16], gap: u16) -> u16 {
    if visible.is_empty() {
        return 0;
    }
    let cols: u16 = visible
        .iter()
        .map(|i| widths.get(*i).copied().unwrap_or(0))
        .fold(0u16, u16::saturating_add);
    let gaps = gap.saturating_mul(u16::try_from(visible.len().saturating_sub(1)).unwrap_or(0));
    cols.saturating_add(gaps)
}

fn paint_header_row<RowId: Clone + Eq, ColumnId: Clone + Eq>(
    table: &Table<'_, RowId, ColumnId>,
    area: Rect,
    buffer: &mut Buffer,
    state: &mut TableState<RowId, ColumnId>,
    gap: u16,
    bordered: bool,
) {
    let header_style = super::table_chrome::header_style(table.tokens);
    buffer.set_style(
        Rect::new(area.x, area.y, area.width, 1),
        super::table_chrome::header_band(table.tokens),
    );
    let origin_x = area.x.saturating_add(MARKER_WIDTH);
    // Clear gutter under header for alignment.
    buffer.set_stringn(
        area.x,
        area.y,
        "  ",
        usize::from(MARKER_WIDTH),
        header_style,
    );
    let mut logical_x: i32 = i32::from(origin_x) - i32::from(state.h_offset);
    let mut shown_sort = false;
    let clip_left = origin_x;
    let clip_right = area.right();
    for (visible_index, column_index) in state.visible_columns.iter().copied().enumerate() {
        let column = &table.columns[column_index];
        let width = state.resolved_widths[column_index];
        let col_left = logical_x;
        let col_right = logical_x + i32::from(width);
        if col_right > i32::from(clip_left) && col_left < i32::from(clip_right) {
            let paint_x = col_left.max(i32::from(clip_left)) as u16;
            let paint_end = col_right.min(i32::from(clip_right)) as u16;
            let paint_w = paint_end.saturating_sub(paint_x);
            if paint_w > 0 {
                let sort = column.sort.filter(|_| column.sortable && !shown_sort);
                shown_sort |= sort.is_some();
                // A sortable column says so before it is sorted (plans/021
                // Step 3): the neutral marker states the capability, the
                // direction arrow replaces it once a sort is applied.
                let sortable_hint = column.sortable && sort.is_none();
                let sort_width = u16::from(sort.is_some() || sortable_hint)
                    .saturating_mul(2)
                    .min(paint_w);
                // Only show title when the left edge of the column is visible enough.
                let skip_left = paint_x.saturating_sub(col_left.max(0) as u16);
                let title_w = paint_w.saturating_sub(sort_width);
                if title_w > 0 && skip_left == 0 {
                    let title_rect = Rect::new(paint_x, area.y, title_w, 1);
                    render_line(
                        &column.title,
                        title_rect,
                        column.alignment,
                        header_style,
                        buffer,
                        &mut state.scratch_text,
                    );
                }
                if sort_width > 0 && col_right as u16 <= clip_right {
                    let sort_x = paint_end.saturating_sub(sort_width);
                    buffer.set_stringn(sort_x, area.y, " ", 1, header_style);
                    if sort_width > 1 {
                        let (marker, marker_style) = match sort {
                            Some(direction) => (sort_glyph(table.tokens, direction), header_style),
                            None => (
                                super::table_chrome::sortable_marker(table.tokens),
                                super::table_chrome::sortable_marker_style(table.tokens),
                            ),
                        };
                        buffer.set_stringn(
                            sort_x.saturating_add(1),
                            area.y,
                            marker,
                            1,
                            marker_style,
                        );
                    }
                }
                if !state
                    .header_regions
                    .iter()
                    .any(|region| region.id == column.id)
                {
                    state.header_regions.push(TableHeaderRegion {
                        id: column.id.clone(),
                        area: Rect::new(paint_x, area.y, paint_w, 1),
                        sortable: column.sortable,
                    });
                }
            }
        }
        logical_x = col_right;
        if visible_index + 1 < state.visible_columns.len() {
            if bordered {
                let sep_x = logical_x;
                if sep_x >= i32::from(clip_left) && sep_x < i32::from(clip_right) {
                    buffer.set_stringn(
                        sep_x as u16,
                        area.y,
                        table.tokens.glyphs.rule_v(),
                        1,
                        table.tokens.palette.style(Role::Border),
                    );
                }
            }
            logical_x += i32::from(gap);
        }
    }
    if bordered && area.height >= 2 {
        let rule_y = area.y.saturating_add(1);
        let rule = table.tokens.glyphs.rule();
        let style = table.tokens.palette.style(Role::Border);
        for x in area.x..area.right() {
            buffer.set_stringn(x, rule_y, rule, 1, style);
        }
    }
}

fn paint_data_cells<RowId: Clone + Eq, ColumnId: Clone + Eq>(
    table: &Table<'_, RowId, ColumnId>,
    buffer: &mut Buffer,
    state: &mut TableState<RowId, ColumnId>,
    row: &TableRow<'_, RowId>,
    y: u16,
    content_x: u16,
    columns_right: u16,
    gap: u16,
    style: Style,
    quiet: Style,
    selected: bool,
) {
    let bordered = matches!(table.recipe, TableRecipe::Bordered);
    let mut logical_x: i32 = i32::from(content_x) - i32::from(state.h_offset);
    let clip_left = content_x;
    let clip_right = columns_right;
    for (visible_index, column_index) in state.visible_columns.iter().copied().enumerate() {
        let width = state.resolved_widths[column_index];
        let col_left = logical_x;
        let col_right = logical_x + i32::from(width);
        if col_right > i32::from(clip_left) && col_left < i32::from(clip_right) {
            let paint_x = col_left.max(i32::from(clip_left)) as u16;
            let paint_end = col_right.min(i32::from(clip_right)) as u16;
            let paint_w = paint_end.saturating_sub(paint_x);
            if paint_w > 0 {
                let rect = Rect::new(paint_x, y, paint_w, 1);
                let cell_focused = selected
                    && table.focused
                    && state
                        .focused_column
                        .as_ref()
                        .is_some_and(|id| id == &table.columns[column_index].id);
                let kind = table.columns[column_index].kind;
                let mut cell_style = kind.cell_style(style, quiet);
                if cell_focused {
                    // A cell cursor is a cell: the explicit reversal pair.
                    // Rows use gutter + tint and never reverse.
                    cell_style = table.tokens.reversed();
                }
                // Only paint text when column left edge is in view (avoid partial misalignment).
                let fully_left = col_left >= i32::from(clip_left);
                if fully_left && let Some(value) = row.cells.get(column_index) {
                    let overflow = if kind.clips_instead_of_ellipsizing() {
                        CellOverflow::Clip
                    } else {
                        table.overflow
                    };
                    render_line_overflow(
                        value,
                        rect,
                        table.columns[column_index].alignment,
                        cell_style,
                        overflow,
                        buffer,
                        &mut state.scratch_text,
                        table.tokens,
                    );
                } else if !fully_left {
                    buffer.set_style(rect, cell_style);
                } else {
                    buffer.set_style(rect, cell_style);
                }
            }
        }
        logical_x = col_right;
        if visible_index + 1 < state.visible_columns.len() {
            if bordered {
                let sep_x = logical_x;
                if sep_x >= i32::from(clip_left) && sep_x < i32::from(clip_right) {
                    buffer.set_stringn(
                        sep_x as u16,
                        y,
                        table.tokens.glyphs.rule_v(),
                        1,
                        table.tokens.palette.style(Role::Border),
                    );
                }
            }
            logical_x += i32::from(gap);
        }
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
    priorities: &[u8],
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

    // Drop lowest-priority columns while mandatory mins + gaps exceed the budget
    // (or a solve still zeros a column). Ties break rightmost-first.
    loop {
        if visible.len() <= 1 {
            break;
        }
        let gaps =
            gap.saturating_mul(u16::try_from(visible.len().saturating_sub(1)).unwrap_or(u16::MAX));
        let mandatory: u64 = visible
            .iter()
            .map(|&i| match columns[i] {
                ColumnWidth::Fixed(w) | ColumnWidth::Min(w) => u64::from(w),
                ColumnWidth::Fill(_) => 0,
            })
            .sum();
        let over_budget = mandatory + u64::from(gaps) > u64::from(available);
        solve_visible(columns, visible, available, gap, scratch, scratch_policies);
        let zeros: Vec<usize> = visible
            .iter()
            .enumerate()
            .filter_map(|(pos, &col)| (scratch[pos] == 0).then_some(col))
            .collect();
        if !over_budget && zeros.is_empty() {
            break;
        }
        // Over budget: drop lowest priority among all. Fitting but zeroed: drop
        // among zeroed only (squeezed columns).
        let candidates: &[usize] = if over_budget {
            visible.as_slice()
        } else {
            zeros.as_slice()
        };
        let drop_col = candidates
            .iter()
            .copied()
            .min_by_key(|&col| {
                let prio = priorities.get(col).copied().unwrap_or(50);
                (prio, usize::MAX - col)
            })
            .expect("candidates non-empty");
        visible.retain(|&col| col != drop_col);
    }

    if visible.is_empty() {
        visible.extend(
            columns
                .iter()
                .enumerate()
                .filter_map(|(index, policy)| match policy {
                    ColumnWidth::Fixed(0) | ColumnWidth::Min(0) => None,
                    _ => Some(index),
                })
                .max_by_key(|&index| {
                    (
                        priorities.get(index).copied().unwrap_or(50),
                        usize::MAX - index,
                    )
                }),
        );
    }
    solve_visible(columns, visible, available, gap, scratch, scratch_policies);
    // Final pass: still-zero columns drop (keep highest priority survivor).
    let mut position = 0;
    visible.retain(|_| {
        let keep = scratch[position] > 0;
        position += 1;
        keep
    });
    if visible.is_empty() {
        if let Some(index) = columns
            .iter()
            .enumerate()
            .find_map(|(index, policy)| match policy {
                ColumnWidth::Fixed(0) | ColumnWidth::Min(0) => None,
                _ => Some(index),
            })
        {
            visible.push(index);
            solve_visible(columns, visible, available, gap, scratch, scratch_policies);
        }
    } else {
        solve_visible(columns, visible, available, gap, scratch, scratch_policies);
    }
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
    render_line_overflow(
        line,
        area,
        alignment,
        style,
        CellOverflow::Clip,
        buffer,
        scratch,
        &DesignSystem::default(),
    );
}

fn render_line_overflow(
    line: &Line<'_>,
    area: Rect,
    alignment: CellAlignment,
    style: Style,
    overflow: CellOverflow,
    buffer: &mut Buffer,
    scratch: &mut String,
    tokens: &DesignSystem,
) {
    paint_line_overflow(
        buffer,
        area,
        line,
        style,
        LinePlacement {
            alignment,
            overflow,
            ellipsis: tokens.glyphs.ellipsis(),
        },
        scratch,
    );
}

fn sort_glyph(_system: &DesignSystem, direction: SortDirection) -> &'static str {
    super::table_chrome::sort_marker(matches!(direction, SortDirection::Ascending))
}

#[cfg(test)]
mod tests {
    use ratatui_core::{style::Color, text::Span};

    use crate::input::{KeyCode, KeyModifiers};

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
        let priorities = [50u8, 50];

        resolve_layout_into(
            &[ColumnWidth::Fixed(4), ColumnWidth::Min(3)],
            &priorities,
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
            &priorities,
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

    #[test]
    fn layout_drops_lowest_priority_first() {
        let mut resolved = Vec::new();
        let mut visible = Vec::new();
        let mut policies = Vec::new();
        let mut scratch = Vec::new();
        // Low priority middle column drops before high-priority edges.
        resolve_layout_into(
            &[
                ColumnWidth::Fixed(6),
                ColumnWidth::Fixed(6),
                ColumnWidth::Fixed(6),
            ],
            &[100, 10, 90],
            14,
            2,
            &mut resolved,
            &mut visible,
            &mut policies,
            &mut scratch,
        );
        assert!(
            !visible.contains(&1),
            "low priority col dropped: {visible:?}"
        );
        assert!(visible.contains(&0));
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
                priority: 50,
                kind: ColumnKind::Text,
            },
            Column {
                id: "region",
                title: Line::from("Region"),
                width: ColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
                alignment: CellAlignment::Center,
                sortable: false,
                sort: None,
                priority: 50,
                kind: ColumnKind::Text,
            },
            Column {
                id: "cpu",
                title: Line::from("CPU"),
                width: ColumnWidth::Fixed(6),
                alignment: CellAlignment::Right,
                sortable: true,
                sort: Some(SortDirection::Descending),
                priority: 50,
                kind: ColumnKind::Text,
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
                leading: None,
                badge: None,
                enabled: true,
                emphasis: false,
                style: None,
            },
            TableRow {
                id: 2,
                cells: &cells[1],
                leading: None,
                badge: None,
                enabled: false,
                emphasis: false,
                style: None,
            },
            TableRow {
                id: 3,
                cells: &cells[2],
                leading: None,
                badge: None,
                enabled: true,
                emphasis: true,
                style: None,
            },
            TableRow {
                id: 4,
                cells: &cells[3],
                leading: None,
                badge: None,
                enabled: true,
                emphasis: false,
                style: None,
            },
        ]
    }

    #[test]
    fn render_preserves_styles_alignment_unicode_and_canonical_regions() {
        let tokens = DesignSystem::default();
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let mut state = TableState::new(Some(1));
        let area = Rect::new(0, 0, 30, 4);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);

        assert_eq!(state.header_regions.len(), 3);
        assert_eq!(
            state
                .row_regions
                .iter()
                .map(|region| region.id)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        // Phosphor gutter selection marker (not a noisy chevron).
        assert_eq!(
            buffer[(0, 1)].symbol(),
            DesignSystem::default().glyphs.selection_gutter()
        );
        assert_eq!(buffer[(2, 1)].fg, Color::Red);
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("CPU ▾"));
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
        // Host gates focus; table handlers always apply when dispatched.
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
        let tokens = DesignSystem::default();
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let area = Rect::new(5, 7, 30, 5);
        let mut state = TableState::new(Some(1));
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
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
        let tokens = DesignSystem::default();
        let area = Rect::new(0, 0, 30, 4);
        let mut state = TableState::new(None);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
        state.hover(Position::new(0, 3));
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(0, 3)].bg,
            tokens
                .resolve_list_row(ListRowVisualState {
                    hovered: true,
                    enabled: true,
                    ..ListRowVisualState::default()
                })
                .hover_wash
                .bg
                .unwrap()
        );
        state.scroll_by(1, rows.len());
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(state.hovered(), Some(&4));
        assert_eq!(
            buffer[(0, 3)].bg,
            tokens
                .resolve_list_row(ListRowVisualState {
                    hovered: true,
                    enabled: true,
                    ..ListRowVisualState::default()
                })
                .hover_wash
                .bg
                .unwrap()
        );
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
        let tokens = DesignSystem::default();
        let mut columns = columns();
        columns[1].sort = Some(SortDirection::Ascending);
        let rows = [];
        let mut state = TableState::<u8, &str>::default();
        let area = Rect::new(0, 0, 30, 2);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
    }

    #[test]
    #[should_panic(expected = "table column IDs must be unique")]
    fn rejects_duplicate_column_ids_in_debug_builds() {
        let tokens = DesignSystem::default();
        let mut columns = columns();
        columns[1].id = "name";
        let rows = [];
        let mut state = TableState::<u8, &str>::default();
        let area = Rect::new(0, 0, 30, 2);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
    }

    #[test]
    #[should_panic(expected = "table row IDs must be unique")]
    fn rejects_duplicate_painted_row_ids_in_debug_builds() {
        let tokens = DesignSystem::default();
        let columns = columns();
        let cells = cells();
        let mut rows = rows(&cells);
        rows[2].id = 1;
        let mut state = TableState::default();
        let area = Rect::new(0, 0, 30, 5);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
    }

    #[test]
    fn clipping_preserves_combining_clusters_and_rejects_partial_wide_graphemes() {
        let tokens = DesignSystem::default();
        let columns = [Column {
            id: "value",
            title: Line::from("V"),
            width: ColumnWidth::Fixed(1),
            alignment: CellAlignment::Left,
            sortable: false,
            sort: None,
            priority: 50,
            kind: ColumnKind::Text,
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
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(2, 1)].symbol(), "e\u{301}");
        assert_eq!(buffer[(2, 2)].symbol(), " ");
        assert_eq!(buffer[(2, 3)].symbol(), "a");
    }

    #[test]
    fn empty_zero_and_narrow_tables_are_safe_and_remove_phantom_gaps() {
        let tokens = DesignSystem::default();
        let columns = [
            Column {
                id: 0,
                title: Line::from("hidden"),
                width: ColumnWidth::Fixed(0),
                alignment: CellAlignment::Left,
                sortable: false,
                sort: None,
                priority: 50,
                kind: ColumnKind::Text,
            },
            Column {
                id: 1,
                title: Line::from("visible"),
                width: fill(1),
                alignment: CellAlignment::Left,
                sortable: false,
                sort: None,
                priority: 50,
                kind: ColumnKind::Text,
            },
        ];
        let rows: [TableRow<'_, u8>; 0] = [];
        for area in [Rect::new(0, 0, 0, 0), Rect::new(0, 0, 3, 1)] {
            let mut state = TableState::default();
            let mut buffer = Buffer::empty(area);
            (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
            assert!(state.row_regions.is_empty());
            assert!(
                state
                    .resolved_widths
                    .first()
                    .is_none_or(|width| *width == 0)
            );
        }
    }

    #[test]
    fn empty_loading_error_body_states() {
        let tokens = DesignSystem::default();
        let columns = columns();
        let rows: [TableRow<'_, u8>; 0] = [];
        let area = Rect::new(0, 0, 40, 5);
        let mut state = TableState::<u8, &str>::default();
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens).empty_message(Line::from("No rows"))).render(
            area,
            &mut buffer,
            &mut state,
        );
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("No rows"), "{text}");

        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)
            .body_state(TableBodyState::Loading)
            .loading_message(Line::from("Wait")))
            .render(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Wait"), "{text}");

        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)
            .body_state(TableBodyState::Error)
            .error_message(Line::from("Boom")))
            .render(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Boom"), "{text}");
    }

    #[test]
    fn recipes_bordered_striped_compact() {
        let tokens = DesignSystem::default();
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let area = Rect::new(0, 0, 36, 6);
        for recipe in [
            TableRecipe::Quiet,
            TableRecipe::Bordered,
            TableRecipe::Striped,
            TableRecipe::Compact,
        ] {
            let mut state = TableState::new(Some(1));
            let mut buffer = Buffer::empty(area);
            (&Table::new(&columns, &rows, &tokens).recipe(recipe)).render(
                area,
                &mut buffer,
                &mut state,
            );
            assert!(!state.header_regions.is_empty(), "{recipe:?}");
            assert!(!state.row_regions.is_empty(), "{recipe:?}");
        }
        assert_eq!(TableRecipe::Compact.default_gap(), 1);
        assert_eq!(TableRecipe::Quiet.default_gap(), 2);
    }

    #[test]
    fn horizontal_scroll_and_cell_focus() {
        let tokens = DesignSystem::default();
        let columns = [
            Column::new("a", "A", ColumnWidth::Fixed(12)).priority(100),
            Column::new("b", "B", ColumnWidth::Fixed(12)).priority(90),
            Column::new("c", "C", ColumnWidth::Fixed(12)).priority(80),
        ];
        let cells = [[
            Line::from("alpha-long"),
            Line::from("beta-long"),
            Line::from("gamma-long"),
        ]];
        let rows = [TableRow::new(1, &cells[0])];
        let area = Rect::new(0, 0, 20, 3);
        let mut state = TableState::new(Some(1));
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
        assert!(state.content_width > state.viewport_width || !state.visible_columns.is_empty());
        let before = state.h_offset();
        // Force overflow path: set content wider than viewport after paint.
        state.content_width = state.viewport_width.saturating_add(10);
        assert!(state.scroll_horizontal(3));
        assert_eq!(state.h_offset(), before.saturating_add(3));

        state.set_focused_column(Some("a"));
        assert_eq!(state.focused_column(), Some(&"a"));
        let _ = state.handle_key(&rows, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        // Cell focus advances among header regions after paint.
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);
        state.set_focused_column(Some(state.header_regions[0].id));
        let _ = state.handle_intent(
            &rows,
            crate::interaction::UiIntent::Move(crate::interaction::NavigationMove::Right),
        );
        assert!(state.focused_column().is_some());
    }

    #[test]
    fn row_selection_is_stable_gutter_and_tint() {
        let columns = columns();
        let cells = cells();
        let rows = rows(&cells);
        let area = Rect::new(0, 0, 30, 4);

        let gutter = DesignSystem::default().selection(crate::style::SelectionChrome::Gutter);
        let mut state = TableState::new(Some(1));
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &gutter)).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 1)].symbol(), gutter.glyphs.selection_gutter());

        let tint_sys = DesignSystem::junie().selection(crate::style::SelectionChrome::Tint);
        let mut state = TableState::new(Some(1));
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tint_sys)).render(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(0, 1)].symbol(),
            " ",
            "tint chrome leaves the gutter slot empty"
        );
        assert_eq!(
            buffer[(5, 1)].bg,
            tint_sys.style(Role::SelectionTint).bg.unwrap(),
            "the focused row still wears the tint"
        );
    }

    #[test]
    fn table_intent_maps_horizontal_keys() {
        use crate::interaction::{NavigationMove, UiIntent, default_table_intent};
        assert_eq!(
            default_table_intent(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Left))
        );
        assert_eq!(
            default_table_intent(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            Some(UiIntent::Move(NavigationMove::Right))
        );
        assert_eq!(
            default_table_intent(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            None
        );
    }

    /// Property-style: layout is deterministic and never panics for narrow budgets.
    #[test]
    fn numeric_columns_read_quieter_than_text_columns() {
        let tokens = DesignSystem::default();
        let columns = [
            Column::new("name", Line::from("Name"), ColumnWidth::Fixed(8)),
            Column::new("size", Line::from("Size"), ColumnWidth::Fixed(6))
                .kind(ColumnKind::Numeric),
            Column::new("state", Line::from("S"), ColumnWidth::Fixed(3)).kind(ColumnKind::Status),
        ];
        let cells = [[
            Line::from("deploy"),
            Line::from("1024"),
            Line::from("running"),
        ]];
        let rows = [TableRow::new(1, &cells[0])];
        let mut state = TableState::default();
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        (&Table::new(&columns, &rows, &tokens)).render(area, &mut buffer, &mut state);

        let row_y = (0..area.height)
            .find(|y| (0..area.width).any(|x| buffer[(x, *y)].symbol().starts_with('d')))
            .expect("the data row must be painted");
        let find = |needle: char| {
            (0..area.width)
                .find(|x| buffer[(*x, row_y)].symbol().starts_with(needle))
                .unwrap_or_else(|| panic!("{needle:?} must be painted: {buffer:?}"))
        };
        let name = buffer[(find('d'), row_y)].style().fg;
        let size = buffer[(find('1'), row_y)].style().fg;
        assert_ne!(
            name, size,
            "a count must not read as loudly as the identity beside it"
        );
        assert_eq!(size, tokens.style(Role::TextMuted).fg);

        // A status column contracts to its letter, never to an ellipsis.
        let status_x = find('r');
        let painted: String = (status_x..area.width)
            .map(|x| buffer[(x, row_y)].symbol())
            .collect();
        assert!(
            !painted.contains(tokens.glyphs.ellipsis()),
            "status column must clip, not ellipsize: {painted:?}"
        );
    }

    #[test]
    fn layout_fuzz_narrow_budgets_are_deterministic() {
        let policies = [
            ColumnWidth::Fixed(8),
            ColumnWidth::Min(6),
            ColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
            ColumnWidth::Fixed(10),
            ColumnWidth::Min(4),
        ];
        let priorities = [100u8, 20, 50, 80, 10];
        let mut resolved_a = Vec::new();
        let mut resolved_b = Vec::new();
        let mut visible_a = Vec::new();
        let mut visible_b = Vec::new();
        let mut scratch = Vec::new();
        let mut policies_scratch = Vec::new();
        for available in 0u16..=40 {
            for gap in [0u16, 1, 2] {
                resolve_layout_into(
                    &policies,
                    &priorities,
                    available,
                    gap,
                    &mut resolved_a,
                    &mut visible_a,
                    &mut scratch,
                    &mut policies_scratch,
                );
                resolve_layout_into(
                    &policies,
                    &priorities,
                    available,
                    gap,
                    &mut resolved_b,
                    &mut visible_b,
                    &mut scratch,
                    &mut policies_scratch,
                );
                assert_eq!(resolved_a, resolved_b, "avail={available} gap={gap}");
                assert_eq!(visible_a, visible_b, "avail={available} gap={gap}");
                assert!(
                    visible_a
                        .iter()
                        .all(|&i| resolved_a[i] > 0 || available == 0),
                    "visible zeros avail={available}"
                );
            }
        }
    }
}
