// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/tabs.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Workbench tab kinds: table (data | structure), query (editor + results), history.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind, KeyModifiers};
use termrock::widgets::{
    ButtonState, ButtonVariant, Column, ColumnWidth, ListRow, ListState, Tab, Table, TableRow,
    TableState, Tabs, TabsState, TextArea, TextAreaState, TextInput, TextInputState,
};

use super::db::{Catalog, ColType, Table as DbTable};
use super::grid::ResultGrid;
use super::model::{History, HistoryEntry, HistorySource};
use super::sql::{self, ResultSet as SqlResult, Statement};
use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, PageCtx, PageEvent};
use crate::text as ttext;

const EDITOR: WidgetId = WidgetId::of("workbench.editor");
const RESULTS: WidgetId = WidgetId::of("workbench.results");
const TABLE_GRID: WidgetId = WidgetId::of("workbench.table");
const TABLE_MODE: WidgetId = WidgetId::of("workbench.table-mode");
const HIST_SEARCH: WidgetId = WidgetId::of("workbench.history-search");
const HIST_LIST: WidgetId = WidgetId::of("workbench.history-list");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Contains,
    Gt,
    Lt,
    IsNull,
    IsNotNull,
}

impl FilterOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Contains => "contains",
            Self::Gt => ">",
            Self::Lt => "<",
            Self::IsNull => "is NULL",
            Self::IsNotNull => "is not NULL",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Filter {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
}

pub enum ResultBody {
    Rows(ResultGrid),
    Message {
        text: String,
        detail: Option<String>,
    },
    Error {
        message: String,
        detail: Option<String>,
    },
    Plan {
        lines: Vec<String>,
    },
}

pub struct QueryResult {
    pub label: String,
    pub duration_ms: u32,
    pub body: ResultBody,
}

pub struct QueryTab {
    pub name: String,
    pub editor: TextAreaState,
    pub results: Vec<QueryResult>,
    pub active_result: usize,
    running: Option<(Vec<(String, Range<usize>)>, u32, Option<bool>)>,
    pub split: u16,
}

impl QueryTab {
    #[must_use]
    pub fn new(name: String, sql: &str) -> Self {
        let mut editor = TextAreaState::new(sql);
        editor.set_accepts_input(false);
        editor.set_editing(false);
        Self {
            name,
            editor,
            results: vec![],
            active_result: 0,
            running: None,
            split: 12,
        }
    }

    #[must_use]
    pub fn dirty(&self) -> bool {
        !self.editor.text().trim().is_empty() && self.results.is_empty()
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.editor.is_editing()
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    pub fn statements_to_run(&self, all: bool) -> Vec<(String, Range<usize>)> {
        let src = self.editor.text();
        if all {
            sql::split_statements(&src)
                .into_iter()
                .map(|(a, b)| (src[a..b].to_owned(), a..b))
                .collect()
        } else {
            let cursor = self.editor.absolute_byte(self.editor.cursor()).unwrap_or(0);
            sql::statement_at(&src, cursor)
                .map(|(a, b)| vec![(src[a..b].to_owned(), a..b)])
                .unwrap_or_default()
        }
    }

    pub fn start(&mut self, statements: Vec<(String, Range<usize>)>, explain: Option<bool>) {
        self.running = Some((statements, 2, explain));
    }

    pub fn tick(
        &mut self,
        cat: &Catalog,
        connection: &str,
        database: &str,
        history: &mut History,
    ) -> bool {
        let Some((stmts, left, explain)) = self.running.as_mut() else {
            return false;
        };
        *left = left.saturating_sub(1);
        if *left > 0 {
            return true;
        }
        let stmts = stmts.clone();
        let explain = *explain;
        self.running = None;
        self.results.clear();
        for (i, (text, range)) in stmts.into_iter().enumerate() {
            let (rs, entry) = execute(cat, &text, range, explain, connection, database, i + 1);
            history.push(entry);
            self.results.push(rs);
        }
        self.active_result = self.results.len().saturating_sub(1);
        true
    }
}

pub struct TableTab {
    pub schema: String,
    pub name: String,
    pub mode: TabsState<u8>,
    pub grid: ResultGrid,
    pub table_state: TableState<usize, usize>,
    pub offset: usize,
    pub page: usize,
}

impl TableTab {
    #[must_use]
    pub fn new(table: &DbTable) -> Self {
        let cols: Vec<(String, ColType)> = table
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.ty))
            .collect();
        let n = 40.min(table.row_count);
        let rows = super::db::rows(table, 0, n);
        let mut grid = ResultGrid::from_values(cols, rows, table.row_count, true);
        grid.more = table.row_count > n;
        let mut mode = TabsState::new();
        mode.set_selected(Some(0));
        Self {
            schema: table.schema.clone(),
            name: table.name.clone(),
            mode,
            grid,
            table_state: TableState::new(Some(0)),
            offset: 0,
            page: 0,
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        false
    }

    #[must_use]
    pub fn dirty_count(&self) -> usize {
        self.grid.pending.count()
    }
}

pub struct HistoryTab {
    pub search: TextInputState,
    pub list: ListState<usize>,
}

impl HistoryTab {
    #[must_use]
    pub fn new() -> Self {
        let mut search = TextInputState::new("").with_allow_empty(true);
        search.set_editing(false);
        Self {
            search,
            list: ListState::new(Some(0)),
        }
    }
}

fn execute(
    cat: &Catalog,
    text: &str,
    _range: Range<usize>,
    explain: Option<bool>,
    connection: &str,
    database: &str,
    n: usize,
) -> (QueryResult, HistoryEntry) {
    let mut entry = HistoryEntry {
        id: 0,
        sql: text.to_owned(),
        connection: connection.to_owned(),
        database: database.to_owned(),
        schema: "public".into(),
        minutes_ago: 0,
        duration_ms: None,
        rows: None,
        error: None,
        source: if explain.is_some() {
            HistorySource::Explain
        } else {
            HistorySource::Editor
        },
    };
    let parsed = match sql::parse(text) {
        Ok(p) => p,
        Err(e) => {
            entry.error = Some(e.message.clone());
            return (
                QueryResult {
                    label: format!("Error {n}"),
                    duration_ms: 1,
                    body: ResultBody::Error {
                        message: e.message,
                        detail: Some("syntax error".into()),
                    },
                },
                entry,
            );
        }
    };
    let stmt = match (explain, parsed) {
        (Some(analyze), Statement::Select(s)) => Statement::Explain {
            analyze,
            inner: Box::new(Statement::Select(s)),
        },
        (Some(analyze), other) => Statement::Explain {
            analyze,
            inner: Box::new(other),
        },
        (None, p) => p,
    };
    match stmt {
        Statement::Select(sel) => match sql::run_select(cat, &sel) {
            Ok(rs) => {
                entry.duration_ms = Some(rs.duration_ms);
                entry.rows = Some(rs.rows.len());
                (
                    QueryResult {
                        label: format!("SELECT {} ({})", sel.table, rs.rows.len()),
                        duration_ms: rs.duration_ms,
                        body: ResultBody::Rows(grid_from_sql(rs)),
                    },
                    entry,
                )
            }
            Err(e) => {
                entry.error = Some(e.message.clone());
                (
                    QueryResult {
                        label: format!("Error {n}"),
                        duration_ms: 2,
                        body: ResultBody::Error {
                            message: e.message,
                            detail: e.detail,
                        },
                    },
                    entry,
                )
            }
        },
        Statement::Explain { analyze, inner } => match *inner {
            Statement::Select(sel) => match sql::explain(cat, &sel, analyze) {
                Ok(plan) => {
                    let mut lines = vec![];
                    sql::plan_text(&plan, 0, &mut lines);
                    let planning = 0.21 + sel.predicates.len() as f64 * 0.09;
                    lines.push(format!("Planning Time: {planning:.3} ms"));
                    if let Some(e) = plan.actual_ms {
                        lines.push(format!("Execution Time: {e:.3} ms"));
                    }
                    entry.duration_ms = Some(1);
                    entry.rows = Some(lines.len());
                    (
                        QueryResult {
                            label: if analyze {
                                "EXPLAIN ANALYZE".into()
                            } else {
                                "EXPLAIN".into()
                            },
                            duration_ms: 1,
                            body: ResultBody::Plan { lines },
                        },
                        entry,
                    )
                }
                Err(e) => {
                    entry.error = Some(e.message.clone());
                    (
                        QueryResult {
                            label: format!("Error {n}"),
                            duration_ms: 2,
                            body: ResultBody::Error {
                                message: e.message,
                                detail: e.detail,
                            },
                        },
                        entry,
                    )
                }
            },
            other => {
                let msg = format!("{} is not EXPLAIN-able in the demo engine", other.verb());
                entry.error = Some(msg.clone());
                (
                    QueryResult {
                        label: "Error".into(),
                        duration_ms: 1,
                        body: ResultBody::Error {
                            message: msg,
                            detail: None,
                        },
                    },
                    entry,
                )
            }
        },
        other => {
            let verb = other.verb();
            let target = other.target().unwrap_or("");
            let text = format!("{verb} {target} — 1 row affected (demo)");
            entry.duration_ms = Some(4);
            entry.rows = Some(1);
            (
                QueryResult {
                    label: format!("{verb} {target}"),
                    duration_ms: 4,
                    body: ResultBody::Message {
                        text,
                        detail: Some("Writes are simulated against the in-memory catalog.".into()),
                    },
                },
                entry,
            )
        }
    }
}

fn grid_from_sql(rs: SqlResult) -> ResultGrid {
    let mut g = ResultGrid::from_values(rs.columns, rs.rows, rs.total, rs.editable);
    g.more = rs.total > g.len();
    g
}

pub fn render_query(tab: &mut QueryTab, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let t = ctx.theme;
    let split = tab.split.min(area.height.saturating_sub(6)).max(4);
    let rows = layout::rows(area, &[split, 0]);
    tab.editor
        .set_accepts_input(ctx.interaction.focused(EDITOR));
    StatefulWidget::render(
        &TextArea::new(ctx.system)
            .title(&tab.name)
            .placeholder("SELECT …"),
        rows[0],
        buf,
        &mut tab.editor,
    );
    ctx.control(EDITOR, rows[0], false);
    if let Some(c) = tab.editor.cursor_cell() {
        ctx.set_cursor(c);
    }

    if tab.is_running() {
        let (inner, bg) = layout::card(rows[1], buf, t, Some("Running"), None, false);
        buf.set_string(inner.x, inner.y, "Executing…", t.secondary().bg(bg));
        ProgressBarWrap::paint(inner.x, inner.y + 2, inner.width.min(40), buf, ctx);
        return;
    }
    if tab.results.is_empty() {
        let (inner, bg) = layout::card(rows[1], buf, t, Some("Results"), None, false);
        buf.set_string(
            inner.x,
            inner.y,
            "Ctrl+R runs the statement at the cursor.",
            t.muted().bg(bg),
        );
        return;
    }
    let i = tab.active_result.min(tab.results.len() - 1);
    render_result(&mut tab.results[i], rows[1], buf, ctx);
}

struct ProgressBarWrap;
impl ProgressBarWrap {
    fn paint(x: u16, y: u16, w: u16, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        termrock::widgets::ProgressBar::new(
            termrock::widgets::ProgressKind::Indeterminate {
                tick: ctx.interaction.tick,
            },
            ctx.system,
        )
        .paint(Rect::new(x, y, w, 1), buf);
    }
}

fn render_result(rs: &mut QueryResult, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let t = ctx.theme;
    match &mut rs.body {
        ResultBody::Rows(grid) => {
            let meta = format!("{} rows · {} ms", grid.len(), rs.duration_ms);
            let (inner, _bg) = layout::card(
                area,
                buf,
                t,
                Some(&rs.label),
                Some(&meta),
                ctx.interaction.focused(RESULTS),
            );
            paint_grid(grid, inner, buf, ctx, RESULTS);
        }
        ResultBody::Message { text, detail } => {
            let (inner, bg) = layout::card(area, buf, t, Some(&rs.label), None, false);
            buf.set_string(inner.x, inner.y, text, t.primary().bg(bg));
            if let Some(d) = detail {
                buf.set_string(inner.x, inner.y + 2, d, t.muted().bg(bg));
            }
        }
        ResultBody::Error { message, detail } => {
            let (inner, bg) = layout::card(area, buf, t, Some("Error"), None, false);
            buf.set_string(inner.x, inner.y, message, t.error_fg().bg(bg));
            if let Some(d) = detail {
                for (i, line) in crate::text::wrap(d, inner.width as usize)
                    .iter()
                    .take(3)
                    .enumerate()
                {
                    buf.set_string(inner.x, inner.y + 2 + i as u16, line, t.secondary().bg(bg));
                }
            }
        }
        ResultBody::Plan { lines } => {
            let (inner, bg) = layout::card(area, buf, t, Some(&rs.label), None, false);
            for (i, line) in lines.iter().enumerate() {
                let y = inner.y + i as u16;
                if y >= inner.bottom() {
                    break;
                }
                buf.set_string(
                    inner.x,
                    y,
                    ttext::truncate(line, inner.width as usize),
                    t.secondary().bg(bg),
                );
            }
        }
    }
}

fn paint_grid(
    grid: &ResultGrid,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    id: WidgetId,
) {
    let cols: Vec<Column<usize>> = grid
        .columns
        .iter()
        .enumerate()
        .map(|(i, (n, _))| Column::new(i, n.as_str(), ColumnWidth::Min(10)))
        .collect();
    let cells: Vec<Vec<Line>> = grid
        .rows
        .iter()
        .enumerate()
        .map(|(ri, row)| {
            row.iter()
                .enumerate()
                .map(|(ci, _)| Line::from(grid.cell(ri, ci).display()))
                .collect()
        })
        .collect();
    let rows: Vec<TableRow<usize>> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| TableRow::new(i, c))
        .collect();
    let mut state = TableState::new(Some(grid.cursor_row.min(grid.len().saturating_sub(1))));
    StatefulWidget::render(
        &Table::new(&cols, &rows, ctx.system).focused(ctx.interaction.focused(id)),
        area,
        buf,
        &mut state,
    );
    ctx.control(id, area, false);
    ctx.scrollable(id, area);
}

pub fn render_table(
    tab: &mut TableTab,
    table: &DbTable,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
) {
    let t = ctx.theme;
    let tabs = [Tab::new(0, "Data"), Tab::new(1, "Structure")];
    tab.mode.set_focused(ctx.interaction.focused(TABLE_MODE));
    Tabs::new(&tabs, ctx.system).quiet(true).paint(
        Rect::new(area.x, area.y, area.width, 2),
        buf,
        &mut tab.mode,
    );
    ctx.control(TABLE_MODE, Rect::new(area.x, area.y, area.width, 2), false);
    let body = Rect::new(
        area.x,
        area.y + 3,
        area.width,
        area.height.saturating_sub(3),
    );
    if tab.mode.selected == Some(1) {
        render_structure(table, body, buf, ctx);
        return;
    }
    let meta = format!(
        "{}–{} of {}",
        tab.offset + 1,
        tab.offset + tab.grid.len(),
        sql::fmt_rows(tab.grid.total)
    );
    let (inner, _bg) = layout::card(body, buf, t, Some(&tab.name), Some(&meta), false);
    paint_grid(&tab.grid, inner, buf, ctx, TABLE_GRID);
}

fn render_structure(table: &DbTable, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let t = ctx.theme;
    let (inner, bg) = layout::card(area, buf, t, Some("Columns"), None, false);
    buf.set_string(
        inner.x,
        inner.y,
        format!("{:<16} {:<14} {:<8} {}", "name", "type", "null", "default"),
        t.muted().bg(bg),
    );
    for (i, c) in table.columns.iter().enumerate() {
        let y = inner.y + 1 + i as u16;
        if y >= inner.bottom() {
            break;
        }
        let line = format!(
            "{:<16} {:<14} {:<8} {}",
            ttext::truncate(&c.name, 16),
            c.ty.sql(),
            if c.nullable { "yes" } else { "no" },
            c.default.as_deref().unwrap_or("")
        );
        let style = if c.primary {
            t.primary().bg(bg)
        } else {
            t.secondary().bg(bg)
        };
        buf.set_string(
            inner.x,
            y,
            ttext::truncate(&line, inner.width as usize),
            style,
        );
    }
}

pub fn render_history(
    tab: &mut HistoryTab,
    history: &History,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
) {
    let t = ctx.theme;
    let rows = layout::rows(area, &[2, 0]);
    tab.search.set_focused(ctx.interaction.focused(HIST_SEARCH));
    let _ = TextInput::new("", ctx.system)
        .placeholder("Search history")
        .paint(rows[0], buf, &mut tab.search);
    ctx.control(HIST_SEARCH, rows[0], false);
    let q = tab.search.value().to_owned();
    let hits = history.search(&q, None, false);
    let list_rows: Vec<ListRow<usize>> = hits
        .iter()
        .map(|e| ListRow::item(e.id, Line::from(e.first_line())).secondary(Line::from(e.when())))
        .collect();
    // first_line() returns String - lifetime issue. Paint simply:
    let (inner, bg) = layout::card(
        rows[1],
        buf,
        t,
        Some("History"),
        Some(&format!("{} statements", hits.len())),
        ctx.interaction.focused(HIST_LIST),
    );
    for (i, e) in hits.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.bottom() {
            break;
        }
        let marker = if tab.list.selected() == Some(&e.id) {
            "› "
        } else {
            "  "
        };
        let line = format!("{marker}{}", e.first_line());
        let style = if e.ok() { t.primary() } else { t.error_fg() };
        buf.set_string(
            inner.x,
            y,
            ttext::truncate(&line, inner.width as usize),
            style.bg(bg),
        );
    }
    ctx.control(HIST_LIST, inner, false);
    ctx.scrollable(HIST_LIST, inner);
    let _ = list_rows;
}

pub fn handle_query(tab: &mut QueryTab, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
    match ev {
        PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
            if *cx.focus == Some(EDITOR) {
                tab.editor.set_accepts_input(true);
                let o = tab.editor.handle_key(*key);
                return if matches!(o, termrock::widgets::TextAreaOutcome::Ignored) {
                    Route::Ignored
                } else {
                    Route::Changed
                };
            }
            Route::Ignored
        }
        PageEvent::Click { id, .. } if *id == EDITOR => {
            cx.set_focus(EDITOR);
            tab.editor.set_accepts_input(true);
            tab.editor.set_editing(true);
            Route::Changed
        }
        PageEvent::Paste(text) if tab.editor.is_editing() => {
            let _ = tab.editor.insert_text(text);
            Route::Changed
        }
        PageEvent::Wheel { id, delta } if *id == EDITOR => {
            let _ = tab.editor.scroll_by(0, *delta as isize);
            Route::Changed
        }
        _ => Route::Ignored,
    }
}

pub fn handle_table(tab: &mut TableTab, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
    match ev {
        PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
            if *cx.focus == Some(TABLE_MODE) {
                let tabs = [Tab::new(0, "Data"), Tab::new(1, "Structure")];
                return match tab.mode.handle_key(*key, &tabs) {
                    termrock::widgets::TabsOutcome::Ignored => Route::Ignored,
                    _ => Route::Changed,
                };
            }
            if *cx.focus == Some(TABLE_GRID) {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        tab.grid.move_cursor(-1, 0);
                        return Route::Changed;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        tab.grid.move_cursor(1, 0);
                        return Route::Changed;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        tab.grid.move_cursor(0, -1);
                        return Route::Changed;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        tab.grid.move_cursor(0, 1);
                        return Route::Changed;
                    }
                    _ => return Route::Ignored,
                }
            }
            Route::Ignored
        }
        PageEvent::Click { id, .. } if *id == TABLE_MODE => {
            cx.set_focus(TABLE_MODE);
            Route::Changed
        }
        PageEvent::Click { id, .. } if *id == TABLE_GRID => {
            cx.set_focus(TABLE_GRID);
            Route::Changed
        }
        _ => Route::Ignored,
    }
}

pub fn handle_history(tab: &mut HistoryTab, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
    match ev {
        PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
            if *cx.focus == Some(HIST_SEARCH) {
                let o = tab.search.handle_key(*key);
                return if matches!(o, termrock::widgets::TextInputOutcome::Ignored) {
                    Route::Ignored
                } else {
                    Route::Changed
                };
            }
            if *cx.focus == Some(HIST_LIST) {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        if let Some(&i) = tab.list.selected() {
                            tab.list.select(Some(i.saturating_sub(1)));
                        }
                        return Route::Changed;
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if let Some(&i) = tab.list.selected() {
                            tab.list.select(Some(i.saturating_add(1)));
                        }
                        return Route::Changed;
                    }
                    _ => return Route::Ignored,
                }
            }
            Route::Ignored
        }
        PageEvent::Click { id, .. } if *id == HIST_SEARCH => {
            cx.set_focus(HIST_SEARCH);
            tab.search.begin_edit();
            Route::Changed
        }
        PageEvent::Click { id, .. } if *id == HIST_LIST => {
            cx.set_focus(HIST_LIST);
            Route::Changed
        }
        _ => Route::Ignored,
    }
}

pub fn query_hints(tab: &QueryTab) -> Vec<Hint> {
    if tab.is_editing() {
        vec![("Esc", "Leave edit"), ("Ctrl+R", "Run")]
    } else {
        vec![
            ("Enter", "Edit"),
            ("Ctrl+R", "Run"),
            ("Ctrl+T", "New query"),
        ]
    }
}

// Keep unused imports from being flagged if Button leftovers remain.
#[allow(dead_code)]
fn _unused_buttons(_: ButtonState, _: ButtonVariant, _: KeyModifiers) {}
