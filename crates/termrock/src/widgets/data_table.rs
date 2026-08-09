// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! DataTable chrome on stable-ID tables + [`data_view`](super::data_view) kits.
//!
//! **Law:** paint and select-all only touch the **projected** slice. For 1M logical
//! rows, consumers call `window.visible_range()` and project that window only.
//!
//! **Cursor vs scene focus:** [`DataTableState::cursor_row`] / [`cursor_col`] are the
//! in-table cell cursor (valid when the host grants input). Scene focus is host-owned;
//! pass [`DataTable::focused`] for surface chrome. Outcomes use [`DataTableOutcome::CursorMoved`]
//! — not scene FocusChanged.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::data_view::{
        ColumnModel, CopyPayload, DataDensity, ExpandState, FilterSpec, LoadState, SelectionModel,
        SortSpec, VirtualWindow,
    },
};

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
    /// Viewport scrolled (row window).
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
    /// Select-all **requested** for currently projected/visible scope only.
    SelectAllRequested,
    /// Copy cursor row cells (consumer supplies text via projected data).
    Copy(CopyPayload),
    /// Expand/collapse detail for row.
    ExpandToggled(RowId),
    /// Context menu at cursor row.
    ContextMenu {
        /// Cursor row.
        row: RowId,
    },
    /// Inline edit requested.
    EditStarted {
        /// Row.
        row: RowId,
        /// Column id when known.
        column: Option<ColId>,
    },
    /// Retry load.
    RetryLoad,
    /// Bulk action index from toolbar.
    ToolbarAction(usize),
}

/// DataTable interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableState<RowId: Clone + Ord, ColId: Clone + PartialEq> {
    /// Selection model.
    pub selection: SelectionModel<RowId>,
    /// Vertical virtual window over logical rows.
    pub window: VirtualWindow,
    /// Horizontal virtual window over visible columns (ordinal).
    pub col_window: VirtualWindow,
    /// Cursor row index in projected slice (0..projected.len()).
    pub cursor_row: usize,
    /// Cursor visible-column ordinal.
    pub cursor_col: usize,
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
    /// Painted body origin (for mouse hit testing).
    body_origin: (u16, u16),
    /// Painted body height in rows.
    body_rows: u16,
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> DataTableState<RowId, ColId> {
    /// Fresh multi-select table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selection: SelectionModel::multi_row(),
            window: VirtualWindow::default(),
            col_window: VirtualWindow::default(),
            cursor_row: 0,
            cursor_col: 0,
            load: LoadState::Ready { count: 0 },
            density: DataDensity::Comfortable,
            expand: ExpandState::default(),
            sort: None,
            filter: FilterSpec::default(),
            striped: true,
            ascii: false,
            colorless: false,
            accepts_input: true,
            body_origin: (0, 0),
            body_rows: 0,
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

        let vis_cols: Vec<_> = columns.visible().map(|(_, c)| c.id.clone()).collect();
        if !vis_cols.is_empty() {
            self.cursor_col = self.cursor_col.min(vis_cols.len() - 1);
            self.col_window.logical_len = vis_cols.len() as u64;
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
                let id = visible_rows[self.cursor_row].clone();
                let _ = self.expand.toggle(id.clone());
                DataTableOutcome::ExpandToggled(id)
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let id = visible_rows[self.cursor_row].clone();
                let _ = self.expand.toggle(id.clone());
                DataTableOutcome::ExpandToggled(id)
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                    DataTableOutcome::CursorMoved
                } else if self.col_window.scroll_by(-1) {
                    DataTableOutcome::Scrolled
                } else {
                    DataTableOutcome::Ignored
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if !vis_cols.is_empty() && self.cursor_col + 1 < vis_cols.len() {
                    self.cursor_col += 1;
                    DataTableOutcome::CursorMoved
                } else if self.col_window.scroll_by(1) {
                    DataTableOutcome::Scrolled
                } else {
                    DataTableOutcome::Ignored
                }
            }
            KeyCode::Char('a') if is_press && key.modifiers.contains(KeyModifiers::CONTROL) => {
                DataTableOutcome::SelectAllRequested
            }
            KeyCode::Char('s') if is_press => {
                let col_id = vis_cols
                    .get(self.cursor_col)
                    .cloned()
                    .or_else(|| columns.visible().next().map(|(_, c)| c.id.clone()));
                let Some(col) = col_id else {
                    return DataTableOutcome::Ignored;
                };
                let ascending = match &self.sort {
                    Some(s) if s.column == col => !s.ascending,
                    _ => true,
                };
                let spec = SortSpec {
                    column: col.clone(),
                    ascending,
                };
                self.sort = Some(spec.clone());
                DataTableOutcome::SortSpec(spec)
            }
            KeyCode::Char('/') if is_press => DataTableOutcome::FilterChanged(self.filter.clone()),
            KeyCode::Char('c') if is_press && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                DataTableOutcome::Copy(CopyPayload::Row { cells: Vec::new() })
            }
            KeyCode::Char('e') if is_press => {
                let col = vis_cols.get(self.cursor_col).cloned();
                DataTableOutcome::EditStarted {
                    row: visible_rows[self.cursor_row].clone(),
                    column: col,
                }
            }
            KeyCode::Char('x') if is_press => DataTableOutcome::ContextMenu {
                row: visible_rows[self.cursor_row].clone(),
            },
            _ => DataTableOutcome::Ignored,
        }
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
        let vis_cols: Vec<_> = columns.visible().map(|(_, c)| c.id.clone()).collect();
        if !vis_cols.is_empty() {
            self.cursor_col = self.cursor_col.min(vis_cols.len() - 1);
            self.col_window.logical_len = vis_cols.len() as u64;
        }
        match intent {
            UiIntent::Move(NavigationMove::Next) => self.move_cursor_row(1, visible_rows.len()),
            UiIntent::Move(NavigationMove::Previous) => {
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
            UiIntent::Open | UiIntent::Close | UiIntent::Cancel => DataTableOutcome::Ignored,
        }
    }

    /// Mouse wheel scroll and click-to-cursor over the painted body.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        visible_rows: &[RowId],
    ) -> DataTableOutcome<RowId, ColId> {
        if !self.accepts_input || visible_rows.is_empty() {
            return DataTableOutcome::Ignored;
        }
        let (ox, oy) = self.body_origin;
        let body = Rect {
            x: ox,
            y: oy,
            width: 200,
            height: self.body_rows.max(1),
        };
        match event.kind {
            MouseEventKind::ScrollUp if body.contains(event.position) => {
                if self.window.scroll_by(-1) {
                    DataTableOutcome::Scrolled
                } else {
                    self.move_cursor_row(-1, visible_rows.len())
                }
            }
            MouseEventKind::ScrollDown if body.contains(event.position) => {
                if self.window.scroll_by(1) {
                    DataTableOutcome::Scrolled
                } else {
                    self.move_cursor_row(1, visible_rows.len())
                }
            }
            MouseEventKind::Down(MouseButton::Left) if body.contains(event.position) => {
                let row = usize::from(event.position.y.saturating_sub(oy));
                if row >= visible_rows.len() {
                    return DataTableOutcome::Ignored;
                }
                if self.cursor_row == row {
                    return DataTableOutcome::Activate(visible_rows[row].clone());
                }
                self.cursor_row = row;
                DataTableOutcome::CursorMoved
            }
            _ => DataTableOutcome::Ignored,
        }
    }

    fn move_cursor_row(&mut self, delta: i64, len: usize) -> DataTableOutcome<RowId, ColId> {
        if len == 0 {
            return DataTableOutcome::Ignored;
        }
        let cur = self.cursor_row as i64;
        let next = (cur + delta).clamp(0, (len as i64) - 1) as usize;
        if next == self.cursor_row {
            // At edge of slice: scroll logical window
            if delta > 0 && self.window.scroll_by(1) {
                return DataTableOutcome::Scrolled;
            }
            if delta < 0 && self.window.scroll_by(-1) {
                return DataTableOutcome::Scrolled;
            }
            return DataTableOutcome::Ignored;
        }
        self.cursor_row = next;
        DataTableOutcome::CursorMoved
    }
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> Default for DataTableState<RowId, ColId> {
    fn default() -> Self {
        Self::new()
    }
}

/// DataTable chrome: toolbar + header + virtual body projection.
#[derive(Debug, Clone, Copy)]
pub struct DataTable<'a, RowId, ColId> {
    system: &'a DesignSystem,
    columns: &'a ColumnModel<ColId>,
    /// Projected visible row labels (caller projects cells).
    rows: &'a [(RowId, &'a [&'a str])],
    toolbar: Option<&'a DataTableToolbar<'a>>,
    /// Host scene owns keyboard focus on this surface.
    focused: bool,
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
            toolbar: None,
            focused: false,
        }
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

    /// Paint O(visible) rows only.
    pub fn render(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut DataTableState<RowId, ColId>,
    ) {
        if area.is_empty() {
            return;
        }
        let surface_focused = self.focused || state.accepts_input;
        state.window.viewport = area.height.saturating_sub(2).max(1);
        state.window.clamp();
        let mut y = area.y;
        if let Some(tb) = self.toolbar
            && y < area.bottom()
        {
            let line = tb.actions.join(" | ");
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
        // Sticky header (outside vertical scroll of projected body)
        if y < area.bottom() {
            let mut headers: Vec<String> = Vec::new();
            for (_i, c) in self.columns.visible() {
                let mut title = c.title.clone();
                if let Some(sort) = &state.sort
                    && sort.column == c.id
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
                headers.push(title);
            }
            let head = headers.join(" │ ");
            let text = take_display_cols(&head, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &text,
                usize::from(area.width),
                self.system.style(Role::TextStrong),
            );
            y = y.saturating_add(1);
        }
        match &state.load {
            LoadState::Empty { message } => {
                let glyph = if state.ascii { "[ ] " } else { "∅ " };
                let msg = message.as_deref().unwrap_or("(empty)");
                let line = format!("{glyph}{msg}");
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::TextMuted),
                    );
                }
                state.body_origin = (area.x, y);
                state.body_rows = 0;
                return;
            }
            LoadState::Loading { message } => {
                let glyph = if state.ascii { "... " } else { "… " };
                let msg = message.as_deref().unwrap_or("Loading…");
                let line = format!("{glyph}{msg}");
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::TextMuted),
                    );
                }
                state.body_origin = (area.x, y);
                state.body_rows = 0;
                return;
            }
            LoadState::Error { message, .. } => {
                let glyph = if state.ascii { "! " } else { "✗ " };
                let line = format!("{glyph}{message}  (r retry)");
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(if state.colorless {
                            Role::TextStrong
                        } else {
                            Role::Danger
                        }),
                    );
                }
                state.body_origin = (area.x, y);
                state.body_rows = 0;
                return;
            }
            _ => {}
        }
        state.body_origin = (area.x, y);
        let body_start = y;
        for (i, (id, cells)) in self.rows.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let cursor = state.cursor_row == i;
            let selected = state.selection.selected_rows.contains(id);
            let expanded = state.expand.expanded.contains(id);
            let style = if state.colorless {
                if selected || (cursor && surface_focused) {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else if selected {
                self.system.style(Role::Selection)
            } else if cursor && surface_focused {
                self.system.style(Role::Focus)
            } else if state.striped && i % 2 == 1 {
                self.system.style(Role::Surface)
            } else {
                self.system.style(Role::Text)
            };
            let gutter = if selected {
                if state.ascii { "*" } else { "✓" }
            } else if cursor && surface_focused {
                if state.ascii { ">" } else { "›" }
            } else if expanded {
                if state.ascii { "v" } else { "▾" }
            } else {
                " "
            };
            let line = format!("{gutter} {}", cells.join(" │ "));
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
        }
        state.body_rows = y.saturating_sub(body_start);
        // Footer: load / count hint when space remains
        if y < area.bottom() {
            let footer = match &state.load {
                LoadState::Partial { resident, total } => match total {
                    Some(t) => format!("partial {resident}/{t}"),
                    None => format!("partial {resident}+"),
                },
                LoadState::Ready { count } => format!("{count} rows"),
                _ => String::new(),
            };
            if !footer.is_empty() {
                let text = take_display_cols(&footer, usize::from(area.width));
                buffer.set_stringn(
                    area.x,
                    y,
                    &text,
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
            }
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
        assert!(state.selection.selected_rows.is_empty());
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
        assert!(state.selection.selected_rows.contains(&10));
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
        // Project only the window — allocation size == viewport
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
        state.body_origin = (0, 2);
        state.body_rows = 3;
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 0, y: 3 },
            modifiers: KeyModifiers::NONE,
        };
        let out = state.handle_mouse(event, &rows);
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
}
