// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/editable.rs (MIT).

//! Cell-nav table: reversed cursor for navigation, underline cursor for edit.

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use termrock::widgets::{
    ColumnKind, ColumnModel, DataColumn, DataColumnWidth, DataTable, DataTableNavMode,
    DataTableOutcome, DataTableState, LoadState, SortSpec,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::pages::tables::{TaskRowData, status_text, tasks};
use crate::text;

const ID: WidgetId = WidgetId::of("editable");
const TABLE: WidgetId = ID.sub("table");

fn validate(col: &str, s: &str) -> Option<String> {
    match col {
        "task" if s.trim().is_empty() => Some("Task name cannot be empty".into()),
        "owner" if s.trim().is_empty() || s.contains(' ') => {
            Some("Owner is a single handle".into())
        }
        "branch" if s.contains(' ') => Some("Branch names cannot contain spaces".into()),
        "changes" if s.parse::<u32>().is_err() => Some("Changes must be a whole number".into()),
        _ => None,
    }
}

fn is_editable(col: &str) -> bool {
    matches!(col, "task" | "owner" | "branch" | "changes")
}

fn col_id(index: usize) -> &'static str {
    ["id", "task", "owner", "status", "branch", "changes"][index]
}

pub struct EditablePage {
    tasks: Vec<TaskRowData>,
    columns: ColumnModel<&'static str>,
    state: DataTableState<u32, &'static str>,
    edits: u32,
    errors: HashMap<(u32, &'static str), String>,
}

impl EditablePage {
    #[must_use]
    pub fn new() -> Self {
        let mut tasks: Vec<TaskRowData> = tasks().into_iter().take(14).collect();
        let mut errors = HashMap::new();
        if let Some(row) = tasks.get_mut(2) {
            row.branch = "fix/checkout flake".into();
            errors.insert(
                (row.id, "branch"),
                "Branch names cannot contain spaces".into(),
            );
        }
        let columns = ColumnModel::new(vec![
            DataColumn::new("id", "ID", DataColumnWidth::Fixed(5))
                .kind(ColumnKind::Id)
                .sortable(),
            DataColumn::new("task", "Task", DataColumnWidth::Min(24))
                .sortable()
                .editable(),
            DataColumn::new("owner", "Owner", DataColumnWidth::Fixed(8))
                .sortable()
                .editable(),
            DataColumn::new("status", "Status", DataColumnWidth::Fixed(9))
                .kind(ColumnKind::Status)
                .sortable(),
            DataColumn::new("branch", "Branch", DataColumnWidth::Fixed(22))
                .sortable()
                .editable(),
            DataColumn::new("changes", "Changes", DataColumnWidth::Fixed(8))
                .kind(ColumnKind::Numeric)
                .sortable()
                .editable(),
        ]);
        let mut state = DataTableState::new();
        state.nav_mode = DataTableNavMode::Cell;
        state.striped = false;
        state.cursor_col = 1;
        state.load = LoadState::Ready {
            count: tasks.len() as u64,
        };
        state.set_logical_rows(tasks.len() as u64);
        Self {
            tasks,
            columns,
            state,
            edits: 0,
            errors,
        }
    }

    fn cell_text(&self, row: &TaskRowData, col: &str) -> String {
        match col {
            "id" => format!("#{}", row.id),
            "task" => row.name.clone(),
            "owner" => row.owner.clone(),
            "status" => status_text(row.status).to_owned(),
            "branch" => row.branch.clone(),
            "changes" => row.changes.to_string(),
            _ => String::new(),
        }
    }

    fn apply_value(&mut self, id: u32, col: &'static str, text: String) -> Option<String> {
        if let Some(err) = validate(col, &text) {
            self.errors.insert((id, col), err.clone());
            return Some(err);
        }
        self.errors.remove(&(id, col));
        if let Some(row) = self.tasks.iter_mut().find(|r| r.id == id) {
            match col {
                "task" => row.name = text,
                "owner" => row.owner = text,
                "branch" => row.branch = text,
                "changes" => {
                    if let Ok(n) = text.parse::<u32>() {
                        row.changes = n;
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn current_col(&self) -> &'static str {
        col_id(self.state.cursor_col.min(5))
    }

    fn current_row_id(&self) -> Option<u32> {
        self.tasks.get(self.state.cursor_row).map(|r| r.id)
    }

    fn begin_edit(&mut self) {
        let col = self.current_col();
        if !is_editable(col) {
            return;
        }
        let Some(id) = self.current_row_id() else {
            return;
        };
        let draft = self
            .tasks
            .iter()
            .find(|r| r.id == id)
            .map(|r| match col {
                "task" => r.name.clone(),
                "owner" => r.owner.clone(),
                "branch" => r.branch.clone(),
                "changes" => r.changes.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        self.state.editing = true;
        self.state.edit_draft = draft;
    }

    fn sort_by(&mut self, spec: SortSpec<&'static str>) {
        self.tasks.sort_by(|a, b| {
            let ord = match spec.column {
                "id" => a.id.cmp(&b.id),
                "task" => a.name.cmp(&b.name),
                "owner" => a.owner.cmp(&b.owner),
                "status" => (a.status as u8).cmp(&(b.status as u8)),
                "branch" => a.branch.cmp(&b.branch),
                "changes" => a.changes.cmp(&b.changes),
                _ => a.id.cmp(&b.id),
            };
            if spec.ascending { ord } else { ord.reverse() }
        });
        self.state.sort = Some(spec);
    }

    fn row_ids(&self) -> Vec<u32> {
        self.tasks.iter().map(|t| t.id).collect()
    }

    fn handle_outcome(
        &mut self,
        out: DataTableOutcome<u32, &'static str>,
        cx: &mut PageCtx<'_>,
    ) -> Route {
        match out {
            DataTableOutcome::Ignored => Route::Ignored,
            DataTableOutcome::SortSpec(spec) => {
                self.sort_by(spec);
                Route::Changed
            }
            DataTableOutcome::SortRequested(col) => {
                let ascending = match &self.state.sort {
                    Some(s) if s.column == col => !s.ascending,
                    _ => true,
                };
                self.sort_by(SortSpec {
                    column: col,
                    ascending,
                });
                Route::Changed
            }
            DataTableOutcome::EditStarted { .. } => {
                if self.state.edit_draft.is_empty() {
                    self.begin_edit();
                }
                Route::Changed
            }
            DataTableOutcome::EditCommitted { row, column, text } => {
                if let Some(err) = self.apply_value(row, column, text) {
                    cx.status(err);
                } else {
                    self.edits += 1;
                    cx.status("Cell saved");
                }
                Route::Changed
            }
            DataTableOutcome::EditCancelled => {
                cx.status("Edit cancelled");
                Route::Changed
            }
            DataTableOutcome::Activate(_) => {
                self.begin_edit();
                Route::Changed
            }
            DataTableOutcome::CursorMoved
            | DataTableOutcome::Scrolled
            | DataTableOutcome::HoverChanged
            | DataTableOutcome::SelectionChanged
            | DataTableOutcome::ToggleRow(_) => Route::Changed,
            _ => Route::Changed,
        }
    }
}

impl Page for EditablePage {
    fn title(&self) -> &'static str {
        "Editable tables"
    }
    fn blurb(&self) -> &'static str {
        "Navigation is reversed cell; editing is a cursor. They never look alike."
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let focused = ctx.interaction.focused(TABLE);
        self.state.set_accepts_input(focused);
        let view = usize::from(area.height.saturating_sub(6));
        let pos = if self.tasks.len() <= view {
            String::new()
        } else {
            let start = self.state.window.offset as usize + 1;
            let end = (start + view - 1).min(self.tasks.len());
            format!("{start}–{end} of {}", self.tasks.len())
        };
        let err = self
            .current_row_id()
            .and_then(|id| self.errors.get(&(id, self.current_col())))
            .cloned()
            .or_else(|| {
                if self.state.editing {
                    validate(self.current_col(), &self.state.edit_draft)
                } else {
                    None
                }
            });
        let meta = match &err {
            Some(e) => e.clone(),
            None if pos.is_empty() => format!("{} edits", self.edits),
            None => format!("{} edits · {pos}", self.edits),
        };
        let card_h = (self.tasks.len() as u16 + 4).min(area.height.saturating_sub(4));
        let card = Rect::new(area.x, area.y, area.width, card_h);
        let (inner, bg) = layout::card(card, buf, t, Some("Tasks"), Some(&meta), focused);
        if let Some(e) = &err {
            let x = card.right().saturating_sub(2 + text::width(e) as u16);
            if x >= card.x {
                buf.set_string(x, card.y, e, t.error_fg().bg(bg));
            }
        }

        let texts: Vec<[String; 6]> = self
            .tasks
            .iter()
            .map(|r| {
                [
                    self.cell_text(r, "id"),
                    self.cell_text(r, "task"),
                    self.cell_text(r, "owner"),
                    self.cell_text(r, "status"),
                    self.cell_text(r, "branch"),
                    self.cell_text(r, "changes"),
                ]
            })
            .collect();
        let refs: Vec<[&str; 6]> = texts
            .iter()
            .map(|r| {
                [
                    r[0].as_str(),
                    r[1].as_str(),
                    r[2].as_str(),
                    r[3].as_str(),
                    r[4].as_str(),
                    r[5].as_str(),
                ]
            })
            .collect();
        let table_rows: Vec<(u32, &[&str])> = self
            .tasks
            .iter()
            .zip(refs.iter())
            .map(|(t, c)| (t.id, c.as_slice()))
            .collect();

        StatefulWidget::render(
            &DataTable::new(ctx.system, &self.columns, &table_rows)
                .focused(focused)
                .row_numbers(false),
            inner,
            buf,
            &mut self.state,
        );
        let error_style = t.error_fg().add_modifier(Modifier::BOLD);
        for ((id, col), _) in &self.errors {
            let Some(region) = self
                .state
                .cell_regions
                .iter()
                .find(|region| region.row == *id && region.column == *col)
            else {
                continue;
            };
            for x in region.area.x..region.area.right() {
                if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                    cell.set_fg(t.error);
                }
            }
            let bang_x = region.area.right().saturating_sub(1);
            if bang_x >= region.area.x {
                buf.set_string(bang_x, region.area.y, "!", error_style);
            }
        }
        ctx.control(TABLE, inner, false);
        ctx.scrollable(TABLE, inner);

        let y = area.y + card_h + 1;
        let legend = [
            (
                "reversed",
                "cell cursor (navigation)",
                t.on(t.text_primary).fg(t.canvas),
            ),
            ("▁", "editing cursor + accent underline", t.primary()),
            ("!", "validation error", t.error_fg()),
        ];
        for (i, (g, body, st)) in legend.iter().enumerate() {
            let yy = y + i as u16;
            if yy < area.bottom() {
                buf.set_string(area.x, yy, g, *st);
                buf.set_string(area.x + 10, yy, body, t.muted());
            }
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if cx.focus_id() != Some(TABLE) {
                    return Route::Ignored;
                }
                if self.state.editing {
                    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                        let col = self.current_col();
                        let Some(id) = self.current_row_id() else {
                            return Route::Consumed;
                        };
                        let text = self.state.edit_draft.clone();
                        if let Some(err) = self.apply_value(id, col, text) {
                            cx.status(err);
                            return Route::Changed;
                        }
                        self.edits += 1;
                        self.state.editing = false;
                        self.state.edit_draft.clear();
                        cx.status("Cell saved");
                        let ids = self.row_ids();
                        let _ = self.state.handle_key(
                            termrock::input::KeyEvent::new(
                                if matches!(key.code, KeyCode::BackTab) {
                                    KeyCode::Left
                                } else {
                                    KeyCode::Right
                                },
                                termrock::input::KeyModifiers::NONE,
                            ),
                            &ids,
                            &self.columns,
                        );
                        self.begin_edit();
                        return Route::Changed;
                    }
                    let ids = self.row_ids();
                    let out = self.state.handle_key(*key, &ids, &self.columns);
                    return self.handle_outcome(out, cx);
                }
                if matches!(key.code, KeyCode::Enter) {
                    self.begin_edit();
                    return Route::Changed;
                }
                if !matches!(
                    key.code,
                    KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Home
                        | KeyCode::End
                        | KeyCode::PageUp
                        | KeyCode::PageDown
                        | KeyCode::Char('s')
                        | KeyCode::Char('k')
                        | KeyCode::Char('j')
                        | KeyCode::Char('h')
                        | KeyCode::Char('l')
                ) {
                    return Route::Ignored;
                }
                let ids = self.row_ids();
                let out = self.state.handle_key(*key, &ids, &self.columns);
                self.handle_outcome(out, cx)
            }
            PageEvent::Paste(text) if self.state.editing => {
                self.state.edit_draft.push_str(text);
                Route::Changed
            }
            PageEvent::Click { id, pos } if *id == TABLE => {
                cx.set_focus(TABLE);
                let ids = self.row_ids();
                let was_row = self.state.cursor_row;
                let was_col = self.state.cursor_col;
                let ev = MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: *pos,
                    modifiers: termrock::input::KeyModifiers::NONE,
                };
                let out = self.state.handle_mouse(ev, &ids, &mut self.columns);
                if matches!(out, DataTableOutcome::CursorMoved)
                    && self.state.cursor_row == was_row
                    && self.state.cursor_col == was_col
                {
                    self.begin_edit();
                    return Route::Changed;
                }
                self.handle_outcome(out, cx)
            }
            PageEvent::Wheel { id, delta } if *id == TABLE => {
                let ids = self.row_ids();
                let ev = MouseEvent {
                    kind: if *delta < 0 {
                        MouseEventKind::ScrollUp
                    } else {
                        MouseEventKind::ScrollDown
                    },
                    position: ratatui::layout::Position {
                        x: self
                            .state
                            .cell_regions
                            .first()
                            .map(|r| r.area.x)
                            .unwrap_or(0),
                        y: self
                            .state
                            .cell_regions
                            .first()
                            .map(|r| r.area.y)
                            .unwrap_or(0),
                    },
                    modifiers: termrock::input::KeyModifiers::NONE,
                };
                let out = self.state.handle_mouse(ev, &ids, &mut self.columns);
                self.handle_outcome(out, cx)
            }
            _ => Route::Ignored,
        }
    }

    fn editing(&self) -> bool {
        self.state.editing
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        if self.state.editing {
            vec![("Enter", "Commit"), ("Esc", "Cancel"), ("Tab", "Next cell")]
        } else {
            vec![
                ("↑ ↓ ← →", "Cell"),
                ("Enter", "Edit"),
                ("s", "Sort"),
                ("click twice", "Edit"),
            ]
        }
    }
}
