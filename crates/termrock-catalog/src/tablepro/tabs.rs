// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/tabs.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Workbench tab kinds: table (data | structure), query (editor + results), history.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{KeyCode, KeyEventKind};
use termrock::style::{DesignSystem, SyntaxTone};
use termrock::widgets::{
    CodeBlock, CodeBlockState, ColumnKind, ColumnModel, DataColumn, DataColumnWidth, DataTable,
    DataTableNavMode, DataTableState, ListRow, ListState, LoadState, SyntaxHighlighter, Tab,
    TableState, Tabs, TabsState, TextAreaState, TextCursor, TextInput, TextInputState,
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
    pub code: CodeBlockState,
    pub results: Vec<QueryResult>,
    pub result_tabs: TabsState<usize>,
    pub active_result: usize,
    running: Option<(Vec<(String, Range<usize>)>, u32, Option<bool>)>,
    pub split: u16,
    pub last_duration: Option<u32>,
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
        grid.annotate(table);
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
            ResultBody::Plan { .. } => duration_label(rs.duration_ms),
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
            paint_grid(grid, area, buf, ctx, RESULTS);
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
                col
            })
            .collect(),
    );
    let owned: Vec<(usize, Vec<String>)> = grid
        .rows
        .iter()
        .enumerate()
        .map(|(ri, _)| {
            let cells: Vec<String> = (0..grid.columns.len())
                .map(|ci| grid.cell(ri, ci).display())
                .collect();
            (ri, cells)
        })
        .collect();
    let refs: Vec<(usize, Vec<&str>)> = owned
        .iter()
        .map(|(r, cells)| (*r, cells.iter().map(String::as_str).collect()))
        .collect();
    let projected: Vec<(usize, &[&str])> = refs.iter().map(|(r, c)| (*r, c.as_slice())).collect();
    let mut state = DataTableState::new();
    state.nav_mode = DataTableNavMode::Cell;
    state.striped = false;
    state.set_logical_rows(grid.len() as u64);
    state.load = LoadState::Ready {
        count: grid.len() as u64,
    };
    state.cursor_row = grid.cursor_row.min(grid.len().saturating_sub(1));
    state.cursor_col = grid.cursor_col;
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
    let hidden = columns
        .columns
        .len()
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
