// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! DataTable chrome on stable-ID tables + [`data_view`](super::data_view) kits.
//!
//! **Law:** paint and select-all only touch the **projected** slice. For 1M logical
//! rows, consumers call `window.visible_range()` and project that window only.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
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
    /// Focus moved within projected slice.
    FocusChanged,
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
    /// Copy focused row cells (consumer supplies text via projected data).
    Copy(CopyPayload),
    /// Expand/collapse detail for row.
    ExpandToggled(RowId),
    /// Context menu at focus.
    ContextMenu {
        /// Focused row.
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
    /// Focused row index in projected slice (0..projected.len()).
    pub focus_row: usize,
    /// Focused visible-column ordinal.
    pub focus_col: usize,
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
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> DataTableState<RowId, ColId> {
    /// Fresh multi-select table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selection: SelectionModel::multi_row(),
            window: VirtualWindow::default(),
            col_window: VirtualWindow::default(),
            focus_row: 0,
            focus_col: 0,
            load: LoadState::Ready { count: 0 },
            density: DataDensity::Comfortable,
            expand: ExpandState::default(),
            sort: None,
            filter: FilterSpec::default(),
            striped: true,
            ascii: false,
        }
    }

    /// Configure logical universe size (e.g. 1_000_000) without allocating rows.
    pub fn set_logical_rows(&mut self, logical_len: u64) {
        self.window.logical_len = logical_len;
        self.window.clamp();
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
        if key.kind == KeyEventKind::Release {
            return DataTableOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        // Empty / error: only retry
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
        self.focus_row = self.focus_row.min(visible_rows.len() - 1);

        let vis_cols: Vec<_> = columns.visible().map(|(_, c)| c.id.clone()).collect();
        if !vis_cols.is_empty() {
            self.focus_col = self.focus_col.min(vis_cols.len() - 1);
            self.col_window.logical_len = vis_cols.len() as u64;
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.move_focus_row(1, visible_rows.len()),
            KeyCode::Up | KeyCode::Char('k') => self.move_focus_row(-1, visible_rows.len()),
            KeyCode::PageDown => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(step) {
                    DataTableOutcome::Scrolled
                } else {
                    self.move_focus_row(step, visible_rows.len())
                }
            }
            KeyCode::PageUp => {
                let step = i64::from(self.window.viewport.max(1));
                if self.window.scroll_by(-step) {
                    DataTableOutcome::Scrolled
                } else {
                    self.move_focus_row(-step, visible_rows.len())
                }
            }
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.window.offset = 0;
                self.window.clamp();
                self.focus_row = 0;
                DataTableOutcome::Scrolled
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.window.offset = self.window.max_offset();
                self.focus_row = visible_rows.len().saturating_sub(1);
                DataTableOutcome::Scrolled
            }
            KeyCode::Home => {
                self.focus_row = 0;
                DataTableOutcome::FocusChanged
            }
            KeyCode::End => {
                self.focus_row = visible_rows.len().saturating_sub(1);
                DataTableOutcome::FocusChanged
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    let id = visible_rows[self.focus_row].clone();
                    let _ = self.expand.toggle(id.clone());
                    return DataTableOutcome::ExpandToggled(id);
                }
                if self.focus_col > 0 {
                    self.focus_col -= 1;
                    DataTableOutcome::FocusChanged
                } else if self.col_window.scroll_by(-1) {
                    DataTableOutcome::Scrolled
                } else {
                    DataTableOutcome::Ignored
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    let id = visible_rows[self.focus_row].clone();
                    let _ = self.expand.toggle(id.clone());
                    return DataTableOutcome::ExpandToggled(id);
                }
                if !vis_cols.is_empty() && self.focus_col + 1 < vis_cols.len() {
                    self.focus_col += 1;
                    DataTableOutcome::FocusChanged
                } else if self.col_window.scroll_by(1) {
                    DataTableOutcome::Scrolled
                } else {
                    DataTableOutcome::Ignored
                }
            }
            KeyCode::Enter if is_press => {
                DataTableOutcome::Activate(visible_rows[self.focus_row].clone())
            }
            KeyCode::Char(' ') if is_press => {
                let id = visible_rows[self.focus_row].clone();
                self.selection.toggle_row(id.clone());
                DataTableOutcome::ToggleRow(id)
            }
            KeyCode::Char('a') if is_press && key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Never invent unloaded rows — request only.
                DataTableOutcome::SelectAllRequested
            }
            KeyCode::Char('s') if is_press => {
                let col_id = vis_cols
                    .get(self.focus_col)
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
                // Consumer should re-project cell text for the focused row; empty
                // range signals "copy focus row" without requiring RowId: Debug.
                DataTableOutcome::Copy(CopyPayload::Row { cells: Vec::new() })
            }
            KeyCode::Char('e') if is_press => {
                let col = vis_cols.get(self.focus_col).cloned();
                DataTableOutcome::EditStarted {
                    row: visible_rows[self.focus_row].clone(),
                    column: col,
                }
            }
            KeyCode::Char('x') if is_press => DataTableOutcome::ContextMenu {
                row: visible_rows[self.focus_row].clone(),
            },
            _ => DataTableOutcome::Ignored,
        }
    }

    fn move_focus_row(&mut self, delta: i64, len: usize) -> DataTableOutcome<RowId, ColId> {
        if len == 0 {
            return DataTableOutcome::Ignored;
        }
        let cur = self.focus_row as i64;
        let next = (cur + delta).clamp(0, (len as i64) - 1) as usize;
        if next == self.focus_row {
            // At edge of slice: scroll logical window
            if delta > 0 && self.window.scroll_by(1) {
                return DataTableOutcome::Scrolled;
            }
            if delta < 0 && self.window.scroll_by(-1) {
                return DataTableOutcome::Scrolled;
            }
            return DataTableOutcome::Ignored;
        }
        self.focus_row = next;
        DataTableOutcome::FocusChanged
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
    tokens: &'a DesignSystem,
    columns: &'a ColumnModel<ColId>,
    /// Projected visible row labels (caller projects cells).
    rows: &'a [(RowId, &'a [&'a str])],
    toolbar: Option<&'a DataTableToolbar<'a>>,
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> DataTable<'a, RowId, ColId> {
    /// Columns + visible projected rows.
    #[must_use]
    pub const fn new(
        tokens: &'a DesignSystem,
        columns: &'a ColumnModel<ColId>,
        rows: &'a [(RowId, &'a [&'a str])],
    ) -> Self {
        Self {
            tokens,
            columns,
            rows,
            toolbar: None,
        }
    }

    /// Toolbar.
    #[must_use]
    pub const fn toolbar(mut self, toolbar: &'a DataTableToolbar<'a>) -> Self {
        self.toolbar = Some(toolbar);
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
                self.tokens.style(Role::TextMuted),
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
                self.tokens.style(Role::TextStrong),
            );
            y = y.saturating_add(1);
        }
        match &state.load {
            LoadState::Empty { message } => {
                let msg = message.as_deref().unwrap_or("(empty)");
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        msg,
                        usize::from(area.width),
                        self.tokens.style(Role::TextMuted),
                    );
                }
                return;
            }
            LoadState::Loading { message } => {
                let msg = message.as_deref().unwrap_or("Loading…");
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        msg,
                        usize::from(area.width),
                        self.tokens.style(Role::TextMuted),
                    );
                }
                return;
            }
            LoadState::Error { message, .. } => {
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        message,
                        usize::from(area.width),
                        self.tokens.style(Role::Danger),
                    );
                }
                return;
            }
            _ => {}
        }
        for (i, (id, cells)) in self.rows.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let focused = state.focus_row == i;
            let selected = state.selection.selected_rows.contains(id);
            let expanded = state.expand.expanded.contains(id);
            let style = if selected {
                self.tokens.style(Role::Selection)
            } else if focused {
                self.tokens.style(Role::Focus)
            } else if state.striped && i % 2 == 1 {
                self.tokens.style(Role::Surface)
            } else {
                self.tokens.style(Role::Text)
            };
            // Colorless selection/focus gutter
            let gutter = if selected {
                if state.ascii { "*" } else { "›" }
            } else if focused {
                ">"
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
                    self.tokens.style(Role::TextMuted),
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
    use crate::widgets::data_view::{ColumnPin, DataColumn, DataColumnWidth, bench};

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
        assert!(state.focus_row < 40);
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
}
