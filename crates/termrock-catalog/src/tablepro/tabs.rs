// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/tabs.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Workbench tab kinds: table (data | structure), query (editor + results), history.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind, KeyModifiers};
use termrock::style::{DesignSystem, SyntaxTone, Tone};
use termrock::widgets::{
    CodeBlock, CodeBlockState, ColumnKind, ColumnModel, DataColumn, DataColumnWidth, DataTable,
    DataTableNavMode, DataTableState, ListRow, ListState, LoadState, Prop, SortSpec,
    SyntaxHighlighter, Tab, Tabs, TabsState, TextAreaState, TextCursor, TextInput, TextInputState,
    Tree, TreeNode, TreeState, render_props,
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

pub(crate) const EDITOR: WidgetId = WidgetId::of("workbench.editor");
const RESULTS: WidgetId = WidgetId::of("workbench.results");
const RESULT_TABS: WidgetId = WidgetId::of("workbench.result-tabs");
const PLAN: WidgetId = WidgetId::of("workbench.plan");
pub(crate) const TABLE_GRID: WidgetId = WidgetId::of("workbench.table");
const TABLE_MODE: WidgetId = WidgetId::of("workbench.table-mode");
const HIST_SEARCH: WidgetId = WidgetId::of("workbench.history-search");
pub(crate) const HIST_LIST: WidgetId = WidgetId::of("workbench.history-list");

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
        root: sql::PlanNode,
        planning_ms: f64,
        execution_ms: Option<f64>,
        tree: Box<TreeState<usize>>,
    },
}

struct PlanInfo {
    op: String,
    relation: Option<String>,
    cost: (f64, f64),
    rows: usize,
    actual_ms: Option<f64>,
    loops: usize,
    detail: Vec<(String, String)>,
    warning: Option<String>,
    share: f64,
    depth: u16,
    branch: bool,
}

pub struct QueryResult {
    pub label: String,
    pub duration_ms: u32,
    pub body: ResultBody,
}

pub struct QueryTab {
    pub name: String,
    pub editor: TextAreaState,
    pub code: CodeBlockState,
    pub results: Vec<QueryResult>,
    pub result_tabs: TabsState<usize>,
    pub active_result: usize,
    running: Option<(Vec<(String, Range<usize>)>, u32, Option<bool>)>,
    pub split: u16,
    pub last_duration: Option<u32>,
    saved_text: String,
}

impl QueryTab {
    #[must_use]
    pub fn new(name: String, sql: &str) -> Self {
        let mut tab = Self {
            name,
            editor: TextAreaState::new(""),
            code: CodeBlockState::new(),
            results: vec![],
            result_tabs: TabsState::new(),
            active_result: 0,
            running: None,
            split: 12,
            last_duration: None,
            saved_text: String::new(),
        };
        tab.set_sql(sql);
        tab
    }

    /// Load a SQL document and park the caret at the end of the first line.
    pub fn set_sql(&mut self, sql: &str) {
        let mut editor = TextAreaState::new(sql);
        editor.set_accepts_input(true);
        let first = sql.lines().next().unwrap_or(sql);
        let _ = editor.set_cursor(TextCursor {
            line: 0,
            byte: first.len(),
        });
        self.editor = editor;
        self.code.set_cursor_line(Some(0));
        self.code.set_cursor_col(first.len());
        self.saved_text = sql.to_owned();
    }

    #[must_use]
    pub fn dirty(&self) -> bool {
        self.editor.text() != self.saved_text
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.editor.is_editing()
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    /// Cancel the active simulated query, matching Junie's Ctrl-C path.
    pub fn cancel(&mut self) -> bool {
        self.running.take().is_some()
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
        self.last_duration = Some(self.results.iter().map(|r| r.duration_ms).sum());
        true
    }
}

/// Source `duration_label` for panel meta and history.
#[must_use]
pub fn duration_label(ms: u32) -> String {
    match ms {
        0 => "<1 ms".into(),
        ms if ms < 1000 => format!("{ms} ms"),
        ms if ms < 60_000 => format!("{:.2} s", ms as f64 / 1000.0),
        ms => format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000),
    }
}

fn line_of(src: &str, byte: usize) -> usize {
    src.get(..byte.min(src.len()))
        .map(|head| head.bytes().filter(|&b| b == b'\n').count())
        .unwrap_or(0)
}

fn highlight_sql_line(line: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    sql::tokenize(line)
        .into_iter()
        .filter_map(|tok| {
            let tone = match tok.kind {
                sql::TokKind::Keyword => SyntaxTone::Keyword,
                sql::TokKind::Ident => SyntaxTone::Ident,
                sql::TokKind::Number => SyntaxTone::Number,
                sql::TokKind::String => SyntaxTone::Str,
                sql::TokKind::Operator => SyntaxTone::Operator,
                sql::TokKind::Punct => SyntaxTone::Punct,
                sql::TokKind::Comment => SyntaxTone::Comment,
                sql::TokKind::Whitespace => return None,
            };
            Some((tok.start..tok.end, tone))
        })
        .collect()
}

struct SqlSyntax<'a> {
    system: &'a DesignSystem,
}

impl SyntaxHighlighter for SqlSyntax<'_> {
    fn highlight_line<'line>(
        &self,
        line: &'line str,
        _line_index: usize,
    ) -> Vec<(&'line str, Style)> {
        let theme = self.system.junie_theme();
        let spans = highlight_sql_line(line);
        let mut out = Vec::new();
        let mut at = 0usize;
        for (range, tone) in spans {
            if range.start > at && range.start <= line.len() {
                out.push((&line[at..range.start], Style::default()));
            }
            let end = range.end.min(line.len());
            if range.start < end {
                out.push((&line[range.start..end], theme.syntax(tone)));
            }
            at = end;
        }
        if at < line.len() {
            out.push((&line[at..], Style::default()));
        }
        if out.is_empty() {
            out.push((line, Style::default()));
        }
        out
    }
}

pub struct TableTab {
    pub schema: String,
    pub name: String,
    pub mode: TabsState<u8>,
    pub grid: ResultGrid,
    pub table_state: DataTableState<usize, usize>,
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
        let n = sql::ROW_CAP.min(table.row_count);
        let rows = super::db::rows(table, 0, n);
        let mut grid = ResultGrid::from_values(cols, rows, table.row_count, true);
        grid.more = table.row_count > n;
        grid.annotate(table);
        let mut mode = TabsState::new();
        mode.set_selected(Some(0));
        let mut table_state = DataTableState::new();
        table_state.nav_mode = DataTableNavMode::Cell;
        table_state.striped = false;
        table_state.set_logical_rows(n as u64);
        Self {
            schema: table.schema.clone(),
            name: table.name.clone(),
            mode,
            grid,
            table_state,
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

    /// Re-run the table query with the current sort (source `TableTab::load`).
    pub fn load(&mut self, cat: &Catalog) {
        let order = self
            .grid
            .sort
            .and_then(|(c, asc)| self.grid.columns.get(c).map(|(n, _)| (n.clone(), asc)));
        let sel = sql::Select {
            columns: vec!["*".into()],
            schema: Some(self.schema.clone()),
            table: self.name.clone(),
            predicates: vec![],
            order,
            limit: Some(sql::ROW_CAP),
            count_only: false,
        };
        let Ok(rs) = sql::run_select(cat, &sel) else {
            return;
        };
        let more = rs.total > rs.rows.len();
        let sort = self.grid.sort;
        let hscroll = self.grid.hscroll;
        let cursor_row = self.grid.cursor_row;
        let cursor_col = self.grid.cursor_col;
        let mut grid = ResultGrid::from_values(rs.columns, rs.rows, rs.total, rs.editable);
        grid.more = more;
        if let Some(t) = cat.find(Some(&self.schema), &self.name) {
            grid.annotate(t);
        }
        grid.sort = sort;
        grid.hscroll = hscroll;
        grid.cursor_row = cursor_row.min(grid.len().saturating_sub(1));
        grid.cursor_col = cursor_col.min(grid.columns.len().saturating_sub(1));
        self.grid = grid;
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
                let table = cat.find(sel.schema.as_deref(), &sel.table);
                let more = rs.total > rs.rows.len() && sel.limit.is_none_or(|l| l > rs.rows.len());
                (
                    QueryResult {
                        label: format!("SELECT {} ({})", sel.table, rs.rows.len()),
                        duration_ms: rs.duration_ms,
                        body: ResultBody::Rows(grid_from_sql(rs, table, more)),
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
                    let planning = 0.21 + sel.predicates.len() as f64 * 0.09;
                    let exec = analyze.then(|| plan.actual_ms.unwrap_or(0.0) + 0.4);
                    let duration_ms = exec.map(|e| e as u32).unwrap_or(1).saturating_add(1);
                    entry.duration_ms = Some(duration_ms);
                    entry.rows = Some(1);
                    (
                        QueryResult {
                            label: if analyze {
                                "EXPLAIN ANALYZE".into()
                            } else {
                                "EXPLAIN".into()
                            },
                            duration_ms,
                            body: ResultBody::Plan {
                                root: plan,
                                planning_ms: planning,
                                execution_ms: exec,
                                tree: Box::new(TreeState::new(Some(0))),
                            },
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

fn grid_from_sql(rs: SqlResult, table: Option<&DbTable>, more: bool) -> ResultGrid {
    let mut g = ResultGrid::from_values(rs.columns, rs.rows, rs.total, rs.editable);
    g.more = more;
    if !more {
        g.total = g.len();
    }
    if let Some(t) = table {
        g.annotate(t);
    }
    g
}

/// Source `Split::vertical`: percent of (height − gap), clamped to mins.
fn split_vertical(
    area: Rect,
    percent: u16,
    min_first: u16,
    min_second: u16,
    gap: u16,
) -> (Rect, Rect) {
    let usable = area.height.saturating_sub(gap);
    if usable < min_first.saturating_add(min_second) {
        return (area, Rect::default());
    }
    let mut first = ((u32::from(usable) * u32::from(percent)) / 100) as u16;
    first = first.clamp(min_first, usable.saturating_sub(min_second));
    (
        Rect::new(area.x, area.y, area.width, first),
        Rect::new(
            area.x,
            area.y.saturating_add(first).saturating_add(gap),
            area.width,
            usable.saturating_sub(first),
        ),
    )
}

pub fn render_query(tab: &mut QueryTab, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let t = ctx.theme;
    let (top, bottom) = split_vertical(area, 38, 4, 6, 1);
    let focused = ctx.interaction.focused(EDITOR);
    tab.editor.set_accepts_input(focused);
    tab.code.set_focused(focused);
    tab.code.set_editing(tab.editor.is_editing());
    tab.code.set_cursor_line(Some(tab.editor.cursor().line));
    tab.code.set_cursor_col(tab.editor.cursor().byte);
    let src = tab.editor.text();
    let owned: Vec<String> = if src.is_empty() {
        vec![String::new()]
    } else {
        src.lines().map(str::to_owned).collect()
    };
    let lines: Vec<&str> = owned.iter().map(String::as_str).collect();
    let hi = SqlSyntax { system: ctx.system };
    let mut block = CodeBlock::new(&lines, ctx.system)
        .highlighter(&hi)
        .line_numbers(true);
    let cur = tab.editor.absolute_byte(tab.editor.cursor()).unwrap_or(0);
    if let Some((a, b)) = sql::statement_at(&src, cur).or_else(|| {
        sql::split_statements(&src)
            .into_iter()
            .next()
            .map(|(start, end)| (start, end))
    }) {
        let start = line_of(&src, a);
        let end = line_of(&src, b.saturating_sub(1).max(a)).saturating_add(1);
        block = block.current_block(start, end);
    }
    if src.is_empty() {
        block = block.footer_status(Some((
            "Type SQL. Ctrl+R runs the statement under the cursor.",
            termrock::style::Role::TextMuted,
        )));
    }
    if !top.is_empty() {
        let parts = block.paint(top, buf, &mut tab.code);
        ctx.control(EDITOR, top, false);
        if tab.editor.is_editing() {
            let c = tab.editor.cursor();
            let y = parts.body.y.saturating_add(
                u16::try_from(c.line.saturating_sub(tab.code.scroll_y)).unwrap_or(0),
            );
            if y < parts.body.bottom() {
                ctx.set_cursor(Position::new(
                    parts
                        .body
                        .x
                        .saturating_add(u16::try_from(c.byte.min(255)).unwrap_or(0)),
                    y,
                ));
            }
        }
    }
    if bottom.is_empty() {
        return;
    }

    let mut y = bottom.y;
    if !tab.results.is_empty() {
        let labels: Vec<String> = tab.results.iter().map(|r| r.label.clone()).collect();
        let defs: Vec<Tab<usize>> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| Tab::new(i, label.as_str()).closable(true))
            .collect();
        tab.result_tabs
            .set_focused(ctx.interaction.focused(RESULT_TABS));
        tab.result_tabs.set_selected(Some(tab.active_result));
        Tabs::new(&defs, ctx.system).show_close(true).paint(
            Rect::new(bottom.x, y, bottom.width, 2),
            buf,
            &mut tab.result_tabs,
        );
        ctx.control(RESULT_TABS, Rect::new(bottom.x, y, bottom.width, 2), false);
        y = y.saturating_add(2);
    }

    let status_line = if tab.is_running() {
        format!(
            "{} running {} · Esc cancels",
            ctx.system.glyphs.ellipsis(),
            duration_label(0)
        )
    } else if let Some(rs) = tab.results.get(tab.active_result) {
        match &rs.body {
            ResultBody::Rows(g) => {
                let rows = if g.more {
                    format!("Showing {} rows", g.len())
                } else if g.len() == 1 {
                    "1 row".into()
                } else if g.is_empty() {
                    "No rows".into()
                } else {
                    format!("{} rows", g.len())
                };
                format!("{rows} · {}", duration_label(rs.duration_ms))
            }
            ResultBody::Message { text, .. } => text.clone(),
            ResultBody::Error { .. } => format!("failed · {}", duration_label(rs.duration_ms)),
            ResultBody::Plan {
                planning_ms,
                execution_ms,
                ..
            } => match execution_ms {
                Some(e) => format!("Planning {planning_ms:.3} ms · Execution {e:.3} ms"),
                None => format!("Planning {planning_ms:.3} ms · r Raw"),
            },
        }
    } else {
        String::new()
    };
    if !status_line.is_empty() {
        let is_err = tab
            .results
            .get(tab.active_result)
            .is_some_and(|r| matches!(r.body, ResultBody::Error { .. }))
            && !tab.is_running();
        let st = if is_err {
            t.error_fg()
        } else if tab.is_running() {
            t.secondary()
        } else {
            t.muted()
        };
        buf.set_string(
            bottom.x.saturating_add(1),
            y,
            ttext::truncate(&status_line, bottom.width.saturating_sub(2) as usize),
            st.bg(t.canvas),
        );
        y = y.saturating_add(1);
    }

    let body = Rect::new(bottom.x, y, bottom.width, bottom.bottom().saturating_sub(y));
    if tab.results.is_empty() && !tab.is_running() {
        buf.set_string(
            body.x.saturating_add(1),
            body.y,
            "Ctrl+R runs the statement under the cursor.",
            t.muted(),
        );
        return;
    }
    if tab.is_running() && tab.results.is_empty() {
        buf.set_string(
            body.x.saturating_add(1),
            body.y,
            "Executing…",
            t.secondary(),
        );
        ProgressBarWrap::paint(
            body.x.saturating_add(1),
            body.y.saturating_add(2),
            body.width.min(40),
            buf,
            ctx,
        );
        return;
    }
    let i = tab.active_result.min(tab.results.len().saturating_sub(1));
    render_result(&mut tab.results[i], body, buf, ctx);
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
            let mut state = DataTableState::new();
            state.nav_mode = DataTableNavMode::Cell;
            state.striped = false;
            paint_grid(grid, area, buf, ctx, RESULTS, &mut state);
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
        ResultBody::Plan {
            root,
            tree,
            planning_ms: _,
            execution_ms: _,
        } => {
            paint_plan(root, tree, area, buf, ctx);
        }
    }
}

fn max_total_cost(node: &sql::PlanNode) -> f64 {
    node.children
        .iter()
        .map(max_total_cost)
        .fold(node.cost.1, f64::max)
}

fn flatten_plan(node: &sql::PlanNode, depth: u16, root_total: f64, out: &mut Vec<PlanInfo>) {
    let children_total: f64 = node.children.iter().map(|c| c.cost.1).sum();
    let exclusive = (node.cost.1 - children_total).max(0.0);
    let share = if root_total > 0.0 {
        (exclusive / root_total).min(1.0)
    } else {
        0.0
    };
    out.push(PlanInfo {
        op: node.op.clone(),
        relation: node.relation.clone(),
        cost: node.cost,
        rows: node.rows,
        actual_ms: node.actual_ms,
        loops: node.loops,
        detail: node.detail.clone(),
        warning: node.warning.clone(),
        share,
        depth,
        branch: !node.children.is_empty(),
    });
    for c in &node.children {
        flatten_plan(c, depth.saturating_add(1), root_total, out);
    }
}

fn paint_plan(
    root: &sql::PlanNode,
    tree: &mut TreeState<usize>,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
) {
    let t = ctx.theme;
    let bg = t.canvas;
    let mut infos = Vec::new();
    flatten_plan(root, 0, max_total_cost(root), &mut infos);
    let labels: Vec<String> = infos
        .iter()
        .map(|info| match &info.relation {
            Some(r) => format!("{} on {r}", info.op),
            None => info.op.clone(),
        })
        .collect();
    let nodes: Vec<TreeNode<'_, usize>> = infos
        .iter()
        .enumerate()
        .map(|(i, info)| {
            let mut n = TreeNode::new(i, Line::from(labels[i].as_str()), info.depth);
            if info.branch {
                n = n.branch().expanded();
            }
            n
        })
        .collect();
    let detail_w: u16 = if area.width >= 110 { 40 } else { 0 };
    let tree_area = Rect::new(
        area.x,
        area.y,
        area.width
            .saturating_sub(detail_w.saturating_add(if detail_w > 0 { 2 } else { 0 })),
        area.height,
    );
    let cols_x = tree_area.right().saturating_sub(38);
    buf.set_string(
        tree_area.x.saturating_add(3),
        tree_area.y,
        "Operation",
        t.muted().bg(bg),
    );
    if cols_x > tree_area.x.saturating_add(20) {
        buf.set_string(
            cols_x,
            tree_area.y,
            format!("{:>13} {:>8} {:>10} {:>4}", "cost", "rows", "actual", "%"),
            t.muted().bg(bg),
        );
    }
    let tree_body = Rect::new(
        tree_area.x,
        tree_area.y.saturating_add(1),
        tree_area.width,
        tree_area.height.saturating_sub(1),
    );
    // The source frame emphasizes the active plan pane while Explorer keeps
    // keyboard navigation; the footer carries the actual focus cue.
    let focused = true;
    StatefulWidget::render(
        &Tree::new(&nodes, ctx.system)
            .focused(focused)
            .background(bg),
        tree_body,
        buf,
        tree,
    );
    if let Some(selected) = tree.selected()
        && *selected < nodes.len()
    {
        let y = tree_body
            .y
            .saturating_add(u16::try_from(*selected).unwrap_or(u16::MAX));
        if y < tree_body.bottom() {
            // Source plan rows retain the active-row weight but stay on the
            // canvas plane; remove only the shared tree's selection wash.
            for x in tree_body.x..tree_body.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    let mut style = cell.style().bg(bg);
                    if x > tree_body.x.saturating_add(2) && style.fg == Some(t.accent) {
                        style = style.fg(t.text_primary);
                    }
                    cell.set_style(style);
                }
            }
        }
    }
    ctx.control(PLAN, tree_body, false);
    for (i, info) in infos.iter().enumerate() {
        let y = tree_body.y.saturating_add(i as u16);
        if y >= tree_body.bottom() || cols_x <= tree_area.x.saturating_add(20) {
            continue;
        }
        let focused_row = focused && tree.selected() == Some(&i);
        let base = if focused_row {
            t.primary().add_modifier(Modifier::BOLD)
        } else {
            t.secondary()
        };
        let share = info.share * 100.0;
        let share_style = if share > 50.0 {
            t.primary().fg(t.warning).add_modifier(Modifier::BOLD)
        } else if share > 20.0 {
            t.primary().add_modifier(Modifier::BOLD)
        } else if share > 5.0 {
            t.secondary()
        } else {
            t.muted()
        };
        let actual = info
            .actual_ms
            .map(|m| format!("{m:.1} ms"))
            .unwrap_or_else(|| "—".into());
        let text = format!(
            "{:>13} {:>8} {:>10}",
            format!("{:.0}..{:.0}", info.cost.0, info.cost.1),
            sql::fmt_rows(info.rows),
            actual
        );
        let bgc = buf[(cols_x, y)].bg;
        buf.set_string(cols_x, y, &text, base.bg(bgc));
        let sh = if share > 50.0 {
            format!("{:>3}▲", share.round() as u32)
        } else {
            format!("{:>3} ", share.round() as u32)
        };
        buf.set_string(cols_x.saturating_add(34), y, &sh, share_style.bg(bgc));
    }
    if detail_w == 0 {
        return;
    }
    let d = Rect::new(
        tree_area.right().saturating_add(2),
        area.y,
        detail_w,
        // Keep the facts card on its fixed source plane; the remaining plan
        // viewport is canvas, not an extended elevated surface.
        area.height.min(17),
    );
    let cursor = tree.selected().copied().unwrap_or(0);
    let Some(info) = infos.get(cursor) else {
        return;
    };
    let (inner, cbg) = layout::card(d, buf, t, Some(&info.op), None, false);
    let mut facts = Vec::new();
    if let Some(r) = &info.relation {
        facts.push(Prop::new("Relation", r.clone()));
    }
    facts.push(
        Prop::new("Cost", format!("{:.2}..{:.2}", info.cost.0, info.cost.1)).tone(Tone::Secondary),
    );
    facts.push(Prop::new("Rows", format!("{} est.", info.rows)).tone(Tone::Secondary));
    if let Some(a) = info.actual_ms {
        facts.push(
            Prop::new(
                "Actual",
                format!(
                    "{a:.3} ms · {} loop{}",
                    info.loops,
                    if info.loops == 1 { "" } else { "s" }
                ),
            )
            .tone(Tone::Secondary),
        );
    }
    for (k, v) in &info.detail {
        // Junie presents Limit's "Actual rows" fact under the compact
        // repeated "Rows" label used by the reference frame.
        let label = if k == "Actual rows" { "Rows" } else { k };
        facts.push(Prop::new(label, v.clone()).tone(Tone::Muted));
    }
    if let Some(w) = &info.warning {
        facts.push(Prop::new("Warning", w.clone()));
    }
    let _ = render_props(inner, buf, t, &facts, cbg);
    // Keep the one spare row below the facts card on the canvas plane.
    let tail = Rect::new(d.x, d.bottom().saturating_sub(1), d.width, 1);
    if !tail.is_empty() {
        buf.set_style(tail, t.primary().bg(t.canvas));
    }
}

fn paint_grid(
    grid: &ResultGrid,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
    id: WidgetId,
    mut state: &mut DataTableState<usize, usize>,
) {
    let t = ctx.theme;
    let focused = ctx.interaction.focused(id);
    let columns = ColumnModel::new(
        grid.columns
            .iter()
            .enumerate()
            .map(|(i, (name, ty))| {
                let kind = match ty {
                    ColType::Uuid => ColumnKind::Id,
                    ColType::Int | ColType::Numeric => ColumnKind::Numeric,
                    _ => ColumnKind::Text,
                };
                let primary = grid.primary.get(i).copied().unwrap_or(false);
                let mut col = DataColumn::new(
                    i,
                    name.as_str(),
                    DataColumnWidth::Fixed(grid.sampled_width(i)),
                )
                .kind(kind)
                .priority(if primary { 100 } else { 50 });
                if primary {
                    col = col.primary();
                }
                if !matches!(ty, ColType::Json) {
                    col = col.sortable();
                }
                if grid.editable && !matches!(ty, ColType::Json) {
                    col = col.editable();
                }
                if i < grid.hscroll {
                    col = col.hidden();
                }
                col
            })
            .collect(),
    );
    let visible_idx: Vec<usize> = columns.visible().map(|(i, _)| i).collect();
    let owned: Vec<(usize, Vec<String>)> = grid
        .rows
        .iter()
        .enumerate()
        .map(|(ri, _)| {
            let cells: Vec<String> = visible_idx
                .iter()
                .map(|&ci| grid.cell(ri, ci).display())
                .collect();
            (ri, cells)
        })
        .collect();
    let refs: Vec<(usize, Vec<&str>)> = owned
        .iter()
        .map(|(r, cells)| (*r, cells.iter().map(String::as_str).collect()))
        .collect();
    let projected: Vec<(usize, &[&str])> = refs.iter().map(|(r, c)| (*r, c.as_slice())).collect();
    state.nav_mode = DataTableNavMode::Cell;
    state.striped = false;
    state.set_logical_rows(grid.len() as u64);
    state.load = LoadState::Ready {
        count: grid.len() as u64,
    };
    state.cursor_row = grid.cursor_row.min(grid.len().saturating_sub(1));
    state.cursor_col = grid.cursor_col.saturating_sub(grid.hscroll);
    state.sort = grid.sort.map(|(c, ascending)| SortSpec {
        column: c,
        ascending,
    });
    state.set_accepts_input(focused);
    StatefulWidget::render(
        &DataTable::new(ctx.system, &columns, &projected)
            .focused(focused)
            .row_numbers(true)
            .datagrid(true),
        area,
        buf,
        &mut state,
    );
    if grid.hscroll > 0 {
        let lbl = format!("‹{}", grid.hscroll);
        buf.set_string(
            area.x.saturating_add(1),
            area.y,
            &lbl,
            t.faint().bg(t.canvas),
        );
    }
    let hidden = grid
        .columns
        .len()
        .saturating_sub(grid.hscroll)
        .saturating_sub(state.header_regions.len());
    if hidden > 0 {
        let lbl = format!("{hidden}›");
        let w = u16::try_from(lbl.chars().count()).unwrap_or(2);
        let x = area.right().saturating_sub(w.saturating_add(2));
        if let Some(cell) = buf.cell_mut((area.right().saturating_sub(1), area.y)) {
            if cell.symbol() == "…" {
                cell.set_symbol(" ");
            }
        }
        buf.set_string(x, area.y, &lbl, t.faint().bg(t.canvas));
    }
    for region in &state.cell_regions {
        let Some(&is_ref) = grid.references.get(region.column) else {
            continue;
        };
        if !is_ref || region.area.width <= 6 {
            continue;
        }
        let x = region.area.right().saturating_sub(1);
        if let Some(cell) = buf.cell_mut((x, region.area.y)) {
            cell.set_symbol("→");
            let mut st = cell.style();
            st.fg = t.muted().fg;
            cell.set_style(st);
        }
    }
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
    // Source sets `mode_tabs.quiet = true` (white rule). The t_100_table
    // golden still has the accent `━` under Data — match the shot.
    Tabs::new(&tabs, ctx.system).paint(
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
    let grid_area = Rect::new(body.x, body.y, body.width, body.height.saturating_sub(1));
    paint_grid(
        &tab.grid,
        grid_area,
        buf,
        ctx,
        TABLE_GRID,
        &mut tab.table_state,
    );
    let shown = tab.table_state.window.viewport.max(1);
    let last = (tab.offset + usize::from(shown)).min(tab.grid.len()).max(1);
    let total = tab.grid.total;
    let mut parts: Vec<String> = Vec::new();
    if let Some((c, asc)) = tab.grid.sort {
        if let Some((name, _)) = tab.grid.columns.get(c) {
            parts.push(format!("sort {name} {}", if asc { "▴" } else { "▾" }));
        }
    }
    if tab.grid.more {
        parts.push(format!(
            "rows {}–{} of {} loaded · {} total",
            tab.offset + 1,
            last,
            tab.grid.len(),
            thousands(total)
        ));
    } else {
        parts.push(format!(
            "rows {}–{} of {}",
            tab.offset + 1,
            last,
            thousands(total)
        ));
    }
    let vis = tab.table_state.header_regions.len();
    if vis > 0
        && (tab.grid.hscroll > 0 || vis < tab.grid.columns.len().saturating_sub(tab.grid.hscroll))
    {
        let c0 = tab.grid.hscroll.saturating_add(1);
        let c1 = tab
            .grid
            .hscroll
            .saturating_add(vis)
            .min(tab.grid.columns.len());
        parts.push(format!("cols {c0}–{c1} of {}", tab.grid.columns.len()));
    }
    let status = parts.join(" · ");
    buf.set_string(
        body.x.saturating_add(1),
        body.bottom().saturating_sub(1),
        ttext::truncate(&status, body.width.saturating_sub(2) as usize),
        t.muted(),
    );
}

fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
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

pub fn handle_table(
    tab: &mut TableTab,
    ev: &PageEvent,
    cx: &mut PageCtx<'_>,
    cat: &Catalog,
) -> Route {
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
                let viewport = tab.table_state.header_regions.len().max(1);
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
                        tab.grid.ensure_hscroll(viewport);
                        return Route::Changed;
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        tab.grid.move_cursor(0, 1);
                        tab.grid.ensure_hscroll(viewport);
                        return Route::Changed;
                    }
                    KeyCode::Char('s') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let c = tab.grid.cursor_col;
                        tab.grid.sort = match tab.grid.sort {
                            Some((sc, true)) if sc == c => Some((c, false)),
                            Some((sc, false)) if sc == c => None,
                            _ => Some((c, true)),
                        };
                        tab.load(cat);
                        match tab.grid.sort {
                            Some((col, asc)) => {
                                if let Some((name, _)) = tab.grid.columns.get(col) {
                                    cx.status(format!(
                                        "Sorted by {name} {}",
                                        if asc { "ascending" } else { "descending" }
                                    ));
                                }
                            }
                            None => cx.status("Sort cleared"),
                        }
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

pub fn table_hints(tab: &TableTab, focus: Option<WidgetId>) -> Vec<Hint> {
    if tab.is_editing() {
        return vec![("Enter", "Commit"), ("Esc", "Cancel"), ("Tab", "Next cell")];
    }
    if focus == Some(TABLE_MODE) {
        return vec![("← →", "Data / Structure"), ("Ctrl+D", "Toggle")];
    }
    if focus == Some(TABLE_GRID) {
        return vec![
            ("↑↓←→", "Cell"),
            ("Enter", "Edit"),
            ("s", "Sort"),
            ("f", "Filter"),
            ("Space", "Select row"),
        ];
    }
    vec![("↑ ↓", "Move"), ("Ctrl+D", "Structure")]
}

pub fn query_hints(tab: &QueryTab) -> Vec<Hint> {
    if tab.is_editing() {
        vec![
            ("Ctrl+R", "Run"),
            ("Alt+R", "Run all"),
            ("Ctrl+Space", "Complete"),
            ("Esc", "Done"),
        ]
    } else {
        vec![
            ("Enter", "Edit"),
            ("Ctrl+R", "Run"),
            ("Alt+R", "Run all"),
            ("Ctrl+X", "Explain"),
            ("/", "Find"),
        ]
    }
}
