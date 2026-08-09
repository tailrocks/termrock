// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! DataTable chrome on stable-ID tables + data_view kits (Plan 052).

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{DesignTokens, Role},
    text::take_display_cols,
    widgets::data_view::{ColumnModel, LoadState, SelectionModel, VirtualWindow},
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
    /// Sort requested for column (consumer sorts).
    SortRequested(ColId),
    /// Row activated.
    Activate(RowId),
    /// Selection changed for one row.
    ToggleRow(RowId),
    /// Select-all **requested** for currently projected/visible scope only.
    SelectAllRequested,
    /// Bulk action index from toolbar.
    ToolbarAction(usize),
}

/// DataTable interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableState<RowId: Clone + Ord, ColId: Clone + PartialEq> {
    /// Selection model.
    pub selection: SelectionModel<RowId>,
    /// Virtual window.
    pub window: VirtualWindow,
    /// Focused row index in projected slice.
    pub focus_row: usize,
    /// Load projection.
    pub load: LoadState,
    /// Stripes.
    pub striped: bool,
    _col: core::marker::PhantomData<ColId>,
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> DataTableState<RowId, ColId> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selection: SelectionModel::multi_row(),
            window: VirtualWindow::default(),
            focus_row: 0,
            load: LoadState::Ready { count: 0 },
            striped: true,
            _col: core::marker::PhantomData,
        }
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
        if visible_rows.is_empty() {
            return DataTableOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down => {
                self.focus_row = (self.focus_row + 1).min(visible_rows.len() - 1);
                DataTableOutcome::Ignored
            }
            KeyCode::Up => {
                self.focus_row = self.focus_row.saturating_sub(1);
                DataTableOutcome::Ignored
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
                if let Some((_idx, col)) = columns.visible().next() {
                    DataTableOutcome::SortRequested(col.id.clone())
                } else {
                    DataTableOutcome::Ignored
                }
            }
            _ => DataTableOutcome::Ignored,
        }
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
    tokens: &'a DesignTokens,
    columns: &'a ColumnModel<ColId>,
    /// Projected visible row labels (caller projects cells).
    rows: &'a [(RowId, &'a [&'a str])],
    toolbar: Option<&'a DataTableToolbar<'a>>,
}

impl<'a, RowId: Clone + Ord, ColId: Clone + PartialEq> DataTable<'a, RowId, ColId> {
    /// Columns + visible projected rows.
    #[must_use]
    pub const fn new(
        tokens: &'a DesignTokens,
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
                self.tokens.theme.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }
        // Header
        if y < area.bottom() {
            let headers: Vec<&str> = self
                .columns
                .visible()
                .map(|(_i, c)| c.title.as_str())
                .collect();
            let head = headers.join(" │ ");
            let text = take_display_cols(&head, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &text,
                usize::from(area.width),
                self.tokens.theme.style(Role::TextStrong),
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
                        self.tokens.theme.style(Role::TextMuted),
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
                        self.tokens.theme.style(Role::TextMuted),
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
                        self.tokens.theme.style(Role::Danger),
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
            let style = if selected {
                self.tokens.theme.style(Role::Selection)
            } else if focused {
                self.tokens.theme.style(Role::Focus)
            } else if state.striped && i % 2 == 1 {
                self.tokens.theme.style(Role::Surface)
            } else {
                self.tokens.theme.style(Role::Text)
            };
            let line = cells.join(" │ ");
            let text = take_display_cols(&line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &text, usize::from(area.width), style);
            y = y.saturating_add(1);
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
    use crate::widgets::data_view::{DataColumn, DataColumnWidth};

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
        let rows: Vec<u64> = (0..10_000).collect();
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
}
