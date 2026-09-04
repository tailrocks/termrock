// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/tables.rs (MIT).
// Demo rows copied from junie-tui src/bin/showcase/data.rs (MIT).

//! Sortable task table plus an empty-state table.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use termrock::style::JunieTheme;
use termrock::widgets::{
    CellAlignment, CellOverflow, Column, ColumnKind, ColumnWidth, SortDirection, Table,
    TableOutcome, TableRow, TableState,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};

const ID: WidgetId = WidgetId::of("tables");
const TASKS: WidgetId = ID.sub("tasks");
const EMPTY: WidgetId = ID.sub("empty");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Done,
    Failed,
    Paused,
}

#[derive(Debug, Clone)]
pub struct TaskRowData {
    pub id: u32,
    pub name: String,
    pub owner: String,
    pub status: TaskStatus,
    pub branch: String,
    pub changes: u32,
    pub duration_s: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Id,
    Task,
    Owner,
    Status,
    Branch,
    Changes,
    Duration,
}

const COLS: [Col; 7] = [
    Col::Id,
    Col::Task,
    Col::Owner,
    Col::Status,
    Col::Branch,
    Col::Changes,
    Col::Duration,
];

/// Showcase task rows (source `data::tasks`).
#[must_use]
pub fn tasks() -> Vec<TaskRowData> {
    let raw: &[(&str, &str, TaskStatus, &str, u32, u32)] = &[
        (
            "Add rate limiting to auth endpoints",
            "mira",
            TaskStatus::Done,
            "feat/rate-limit",
            14,
            412,
        ),
        (
            "Migrate sessions table to UUID keys",
            "jonas",
            TaskStatus::Running,
            "chore/uuid-sessions",
            31,
            96,
        ),
        (
            "Fix flaky checkout integration test",
            "ana",
            TaskStatus::Failed,
            "fix/checkout-flake",
            3,
            58,
        ),
        (
            "Write release notes for 3.2",
            "mira",
            TaskStatus::Queued,
            "docs/release-3.2",
            0,
            0,
        ),
        (
            "Replace deprecated Vue mixins",
            "kai",
            TaskStatus::Done,
            "refactor/mixins",
            87,
            1330,
        ),
        (
            "Upgrade Postgres driver to 0.9",
            "jonas",
            TaskStatus::Paused,
            "chore/pg-driver",
            5,
            240,
        ),
        (
            "Extract billing service module",
            "sofia",
            TaskStatus::Done,
            "refactor/billing",
            52,
            908,
        ),
        (
            "Add OpenTelemetry tracing spans",
            "kai",
            TaskStatus::Running,
            "feat/otel",
            22,
            130,
        ),
        (
            "Remove legacy feature flags",
            "ana",
            TaskStatus::Queued,
            "chore/flags",
            0,
            0,
        ),
        (
            "Generate API client from OpenAPI",
            "sofia",
            TaskStatus::Done,
            "feat/api-client",
            118,
            2210,
        ),
        (
            "Harden CSP headers",
            "mira",
            TaskStatus::Done,
            "sec/csp",
            4,
            77,
        ),
        (
            "Speed up cold start of worker",
            "jonas",
            TaskStatus::Failed,
            "perf/worker-boot",
            9,
            601,
        ),
        (
            "Localize onboarding emails",
            "kai",
            TaskStatus::Queued,
            "feat/i18n-emails",
            0,
            0,
        ),
        (
            "Add pagination to audit log",
            "ana",
            TaskStatus::Done,
            "feat/audit-pages",
            16,
            344,
        ),
        (
            "Refactor retry helper into crate",
            "sofia",
            TaskStatus::Running,
            "refactor/retry",
            11,
            45,
        ),
        (
            "Rotate signing keys quarterly",
            "mira",
            TaskStatus::Queued,
            "sec/key-rotation",
            0,
            0,
        ),
        (
            "Fix timezone bug in scheduler",
            "jonas",
            TaskStatus::Done,
            "fix/tz-scheduler",
            7,
            188,
        ),
        (
            "Document webhook retry semantics",
            "kai",
            TaskStatus::Done,
            "docs/webhooks",
            2,
            65,
        ),
        (
            "Add dark mode to admin panel",
            "ana",
            TaskStatus::Paused,
            "feat/admin-dark",
            40,
            720,
        ),
        (
            "Bump minimum Node to 22",
            "sofia",
            TaskStatus::Queued,
            "chore/node-22",
            0,
            0,
        ),
        (
            "Cache dependency graph between runs",
            "mira",
            TaskStatus::Running,
            "perf/dep-cache",
            19,
            210,
        ),
        (
            "Clean up unused SQL views",
            "jonas",
            TaskStatus::Done,
            "chore/sql-views",
            12,
            155,
        ),
        (
            "Add health endpoint for gateway",
            "kai",
            TaskStatus::Done,
            "feat/health",
            3,
            42,
        ),
        (
            "Investigate memory growth in parser",
            "ana",
            TaskStatus::Running,
            "perf/parser-mem",
            6,
            380,
        ),
    ];
    raw.iter()
        .enumerate()
        .map(
            |(i, (name, owner, status, branch, changes, duration_s))| TaskRowData {
                id: 1040 + i as u32,
                name: (*name).to_owned(),
                owner: (*owner).to_owned(),
                status: *status,
                branch: (*branch).to_owned(),
                changes: *changes,
                duration_s: *duration_s,
            },
        )
        .collect()
}

#[must_use]
pub fn format_duration(duration_s: u32) -> String {
    if duration_s == 0 {
        "0s".to_owned()
    } else if duration_s >= 60 {
        format!("{}m {:02}s", duration_s / 60, duration_s % 60)
    } else {
        format!("{duration_s}s")
    }
}

#[must_use]
pub fn status_text(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Running => "▸ Running",
        TaskStatus::Failed => "Failed",
        TaskStatus::Paused => "Paused",
        TaskStatus::Queued => "Queued",
        TaskStatus::Done => "Done",
    }
}

#[must_use]
pub fn status_line(s: TaskStatus, t: &JunieTheme) -> Line<'static> {
    let (text, style) = match s {
        TaskStatus::Running => ("▸ Running", t.primary()),
        TaskStatus::Failed => ("Failed", t.error_fg()),
        TaskStatus::Paused => ("Paused", Style::new().fg(t.warning)),
        TaskStatus::Queued => ("Queued", t.muted()),
        TaskStatus::Done => ("Done", t.secondary()),
    };
    Line::from(Span::styled(format!("{text:<9}"), style))
}

fn col_title(col: Col) -> &'static str {
    match col {
        Col::Id => "ID",
        Col::Task => "Task",
        Col::Owner => "Owner",
        Col::Status => "Status",
        Col::Branch => "Branch",
        Col::Changes => "Changes",
        Col::Duration => "Duration",
    }
}

fn col_width(col: Col) -> ColumnWidth {
    match col {
        Col::Id => ColumnWidth::Fixed(5),
        Col::Task => ColumnWidth::Min(24),
        Col::Owner => ColumnWidth::Fixed(7),
        Col::Status => ColumnWidth::Fixed(9),
        Col::Branch => ColumnWidth::Fixed(20),
        Col::Changes | Col::Duration => ColumnWidth::Fixed(9),
    }
}

fn col_kind(col: Col) -> ColumnKind {
    match col {
        Col::Id => ColumnKind::Id,
        Col::Status => ColumnKind::Status,
        Col::Changes | Col::Duration => ColumnKind::Numeric,
        Col::Task | Col::Owner | Col::Branch => ColumnKind::Text,
    }
}

fn sort_tasks(rows: &mut [TaskRowData], col: Col, dir: SortDirection) {
    rows.sort_by(|a, b| {
        let ord = match col {
            Col::Id => a.id.cmp(&b.id),
            Col::Task => a.name.cmp(&b.name),
            Col::Owner => a.owner.cmp(&b.owner),
            Col::Status => (a.status as u8).cmp(&(b.status as u8)),
            Col::Branch => a.branch.cmp(&b.branch),
            Col::Changes => a.changes.cmp(&b.changes),
            Col::Duration => a.duration_s.cmp(&b.duration_s),
        };
        match dir {
            SortDirection::Ascending => ord,
            SortDirection::Descending => ord.reverse(),
        }
    });
}

pub struct TablesPage {
    tasks: Vec<TaskRowData>,
    tasks_state: TableState<u32, Col>,
    empty_state: TableState<u32, &'static str>,
    sort: Option<(Col, SortDirection)>,
    tasks_view: usize,
    capture_cursor: Option<Position>,
}

impl TablesPage {
    #[must_use]
    pub fn new() -> Self {
        let mut tasks = tasks();
        sort_tasks(&mut tasks, Col::Id, SortDirection::Ascending);
        Self {
            tasks,
            tasks_state: TableState::new(None),
            empty_state: TableState::new(None),
            sort: Some((Col::Id, SortDirection::Ascending)),
            tasks_view: 0,
            capture_cursor: None,
        }
    }

    fn apply_sort(&mut self, col: Col) {
        let dir = match self.sort {
            Some((c, SortDirection::Ascending)) if c == col => SortDirection::Descending,
            _ => SortDirection::Ascending,
        };
        self.sort = Some((col, dir));
        sort_tasks(&mut self.tasks, col, dir);
    }

    fn handle_tasks_outcome(&mut self, out: TableOutcome<u32, Col>, cx: &mut PageCtx<'_>) -> Route {
        match out {
            TableOutcome::Ignored => Route::Ignored,
            TableOutcome::Selected(_) => Route::Changed,
            TableOutcome::Activated(id) => {
                if let Some(t) = self.tasks.iter().find(|t| t.id == id) {
                    cx.status(format!("Selected {}", t.name));
                }
                Route::Changed
            }
            TableOutcome::SortRequested(col) => {
                self.apply_sort(col);
                Route::Changed
            }
            TableOutcome::Cancelled => Route::Consumed,
            _ => Route::Changed,
        }
    }
}

impl Page for TablesPage {
    fn title(&self) -> &'static str {
        "Tables"
    }
    fn blurb(&self) -> &'static str {
        "Sort by header, hover rows, select with Enter, overflow scrolls"
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let rows = layout::rows(area, &[area.height.saturating_sub(9), 1, 0]);
        let pos =
            layout::overflow_label(self.tasks_state.offset(), self.tasks_view, self.tasks.len());
        let sort = match self.sort {
            Some((c, d)) => format!(
                "sorted by {} {}",
                col_title(c).to_lowercase(),
                if d == SortDirection::Ascending {
                    "▴"
                } else {
                    "▾"
                }
            ),
            None => "unsorted".to_owned(),
        };
        let meta = if pos.is_empty() {
            sort
        } else {
            format!("{sort} · {pos}")
        };
        let focused = ctx.interaction.focused(TASKS);
        let (inner, bg) = layout::card(rows[0], buf, t, Some("Tasks"), Some(&meta), focused);
        self.capture_cursor = ctx.interaction.pointer;

        // The source treats the right padding cell as part of the row hover
        // band. Map that half-open boundary into the painted row before the
        // stateful table resolves hover.
        let pointer = ctx.interaction.pointer.map(|position| {
            if position.x == inner.right() && position.y >= inner.y && position.y < inner.bottom() {
                Position::new(position.x.saturating_sub(2), position.y)
            } else {
                position
            }
        });
        if let Some(pointer) = pointer {
            self.tasks_state.hover(pointer);
        }

        let columns: Vec<Column<'_, Col>> = COLS
            .iter()
            .copied()
            .map(|col| {
                let sort = self.sort.filter(|(c, _)| *c == col).map(|(_, d)| d);
                let mut c = Column::new(col, col_title(col), col_width(col))
                    .kind(col_kind(col))
                    .sortable(sort);
                if matches!(col, Col::Changes | Col::Duration) {
                    c = c.alignment(CellAlignment::Right);
                }
                c
            })
            .collect();

        let id_s: Vec<String> = self.tasks.iter().map(|r| format!("#{}", r.id)).collect();
        let dur_s: Vec<String> = self
            .tasks
            .iter()
            .map(|r| format_duration(r.duration_s))
            .collect();
        let chg_s: Vec<String> = self.tasks.iter().map(|r| r.changes.to_string()).collect();
        let cell_lines: Vec<[Line<'_>; 7]> = self
            .tasks
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let id_tone = t.muted();
                let branch_tone = t.muted();
                let chg_tone = if r.changes == 0 {
                    t.muted()
                } else {
                    t.primary()
                };
                [
                    Line::from(Span::styled(id_s[i].as_str(), id_tone)),
                    Line::from(r.name.as_str()),
                    Line::from(r.owner.as_str()),
                    status_line(r.status, t),
                    Line::from(Span::styled(format!("{:<20}", r.branch), branch_tone)),
                    Line::from(Span::styled(chg_s[i].as_str(), chg_tone)),
                    Line::from(dur_s[i].as_str()),
                ]
            })
            .collect();
        let table_rows: Vec<TableRow<'_, u32>> = self
            .tasks
            .iter()
            .zip(cell_lines.iter())
            .map(|(r, cells)| TableRow::new(r.id, cells.as_slice()))
            .collect();

        StatefulWidget::render(
            &Table::new(&columns, &table_rows, ctx.system)
                .focused(focused)
                .overflow(CellOverflow::Ellipsis),
            inner,
            buf,
            &mut self.tasks_state,
        );
        ctx.control(TASKS, inner, false);
        ctx.scrollable(TASKS, inner);
        self.tasks_view = usize::from(inner.height.saturating_sub(1));

        let (empty_inner, _) = layout::card(
            rows[2],
            buf,
            t,
            Some("Checks"),
            None,
            ctx.interaction.focused(EMPTY),
        );
        let empty_cols = [
            Column::new("check", "Check", ColumnWidth::Min(12)),
            Column::new("result", "Result", ColumnWidth::Fixed(8)),
        ];
        let empty_rows: [TableRow<'_, u32>; 0] = [];
        StatefulWidget::render(
            &Table::new(&empty_cols, &empty_rows, ctx.system)
                .focused(ctx.interaction.focused(EMPTY))
                .overflow(CellOverflow::Ellipsis)
                .empty_message(Line::from("No checks have run yet")),
            empty_inner,
            buf,
            &mut self.empty_state,
        );
        ctx.control(EMPTY, empty_inner, false);
        let _ = bg;
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if cx.focus_id() == Some(TASKS) {
                    let ids: Vec<u32> = self.tasks.iter().map(|t| t.id).collect();
                    let rows: Vec<TableRow<'static, u32>> =
                        ids.into_iter().map(|id| TableRow::new(id, &[])).collect();
                    let out = self.tasks_state.handle_key(&rows, *key);
                    return self.handle_tasks_outcome(out, cx);
                }
                if cx.focus_id() == Some(EMPTY) {
                    let empty: [TableRow<'_, u32>; 0] = [];
                    return match self.empty_state.handle_key(&empty, *key) {
                        TableOutcome::Ignored => Route::Ignored,
                        _ => Route::Changed,
                    };
                }
                Route::Ignored
            }
            PageEvent::Click { id, pos } => {
                if *id == TASKS
                    || self
                        .tasks_state
                        .row_regions
                        .iter()
                        .any(|r| r.area.contains(*pos))
                    || self
                        .tasks_state
                        .header_regions
                        .iter()
                        .any(|r| r.area.contains(*pos))
                {
                    cx.set_focus(TASKS);
                    let out = self.tasks_state.click(*pos);
                    return self.handle_tasks_outcome(out, cx);
                }
                if *id == EMPTY {
                    cx.set_focus(EMPTY);
                    let _ = self.empty_state.click(*pos);
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } if *id == TASKS => {
                let _ = self
                    .tasks_state
                    .scroll_by(*delta as isize, self.tasks.len());
                Route::Changed
            }
            PageEvent::Drag { pressed, pos } if *pressed == TASKS => {
                let ev = MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: *pos,
                    modifiers: termrock::input::KeyModifiers::NONE,
                };
                let _ = self.tasks_state.handle_mouse(ev, self.tasks.len());
                Route::Changed
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        vec![
            ("↑ ↓", "Move"),
            ("← →", "Columns"),
            ("s", "Sort column"),
            ("Enter", "Select"),
        ]
    }

    fn capture_cursor(&self) -> Option<Position> {
        self.capture_cursor
    }
}
