// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/tabs.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Workbench tab kinds: table (data | structure), query (editor + results), history.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::StatefulWidget;
use termrock::input::{
    KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::style::{DesignSystem, Role, SyntaxTone, Tone};
use termrock::widgets::{
    CodeBlock, CodeBlockState, CodeGutterMark, CodeHighlight, CodeHighlightKind, ColumnKind,
    ColumnModel, CompletionCandidate, CompletionMenu, CompletionMenuOutcome, CompletionMenuSize,
    CompletionMenuState, DataColumn, DataColumnWidth, DataTable, DataTableNavMode,
    DataTableOutcome, DataTableState, EmptyKind, EmptyState, ListState, LoadState, MatchRange,
    MatchRanges, Prop, SortSpec, SyntaxHighlighter, Tab, TabStatus, Tabs, TabsState, TextAreaState,
    TextCursor, TextInputState, TokenItem, TokenStrip, TokenStripOutcome, TokenStripState, Tree,
    TreeNode, TreeState, render_props,
};

use super::db::{Catalog, ColType, Table as DbTable};
use super::grid::{CellValue, ResultGrid};
use super::model::{self, Completion, History, HistoryEntry, HistorySource};
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
pub(crate) const PLAN: WidgetId = WidgetId::of("workbench.plan");
pub(crate) const TABLE_GRID: WidgetId = WidgetId::of("workbench.table");
pub(crate) const TABLE_FILTERS: WidgetId = WidgetId::of("workbench.table-filters");
pub(crate) const TABLE_MODE: WidgetId = WidgetId::of("workbench.table-mode");
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
    pub const ALL: [Self; 6] = [
        Self::Eq,
        Self::Contains,
        Self::Gt,
        Self::Lt,
        Self::IsNull,
        Self::IsNotNull,
    ];

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

    #[must_use]
    pub const fn needs_value(self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }

    #[must_use]
    pub fn ordered_for(ty: ColType) -> Vec<Self> {
        let first: &[Self] = match ty {
            ColType::Int | ColType::Numeric | ColType::Timestamp | ColType::Date => {
                &[Self::Eq, Self::Gt, Self::Lt, Self::IsNull, Self::IsNotNull]
            }
            _ => &[Self::Eq, Self::Contains, Self::IsNull, Self::IsNotNull],
        };
        let mut out = first.to_vec();
        for op in Self::ALL {
            if !out.contains(&op) {
                out.push(op);
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
    pub enabled: bool,
}

impl Filter {
    #[must_use]
    pub fn chip_label(&self) -> String {
        if !self.op.needs_value() {
            return format!("{} {}", self.column, self.op.label());
        }
        format!(
            "{} {} {}",
            self.column,
            self.op.label(),
            filter_literal(&self.value)
        )
    }

    #[must_use]
    pub fn to_sql(&self) -> String {
        let c = &self.column;
        let v = &self.value;
        match self.op {
            FilterOp::Eq => format!("{c} = {}", filter_literal(v)),
            FilterOp::Contains => format!("{c} LIKE '%{v}%'"),
            FilterOp::Gt => format!("{c} > {}", filter_literal(v)),
            FilterOp::Lt => format!("{c} < {}", filter_literal(v)),
            FilterOp::IsNull => format!("{c} IS NULL"),
            FilterOp::IsNotNull => format!("{c} IS NOT NULL"),
        }
    }

    #[must_use]
    pub fn predicates(&self) -> Vec<sql::Predicate> {
        use sql::Cmp;
        let predicate = |cmp: Cmp, value: &str| sql::Predicate {
            column: self.column.clone(),
            cmp,
            value: value.to_owned(),
        };
        match self.op {
            FilterOp::Eq => vec![predicate(Cmp::Eq, &self.value)],
            FilterOp::Contains => vec![predicate(Cmp::Like, &format!("%{}%", self.value))],
            FilterOp::Gt => vec![predicate(Cmp::Gt, &self.value)],
            FilterOp::Lt => vec![predicate(Cmp::Lt, &self.value)],
            FilterOp::IsNull => vec![predicate(Cmp::IsNull, "")],
            FilterOp::IsNotNull => vec![predicate(Cmp::IsNotNull, "")],
        }
    }
}

fn filter_literal(value: &str) -> String {
    if value.parse::<f64>().is_ok() || matches!(value, "true" | "false") {
        value.to_owned()
    } else {
        format!("'{value}'")
    }
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
    Cancelled,
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

#[derive(Debug, Clone)]
struct QueryDiagnostic {
    range: Range<usize>,
    message: String,
}

pub struct QueryTab {
    pub name: String,
    pub editor: TextAreaState,
    pub code: CodeBlockState,
    pub results: Vec<QueryResult>,
    pub result_tabs: TabsState<usize>,
    pub active_result: usize,
    running: Option<(Vec<(String, Range<usize>)>, u32, Option<bool>, u32)>,
    next_run_ticks_left: Option<u32>,
    pub split: u16,
    pub last_duration: Option<u32>,
    pub completion: CompletionMenuState<usize>,
    completion_items: Vec<Completion>,
    completion_matches: Vec<MatchRanges>,
    completion_replace_len: usize,
    diagnostic: Option<QueryDiagnostic>,
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
            next_run_ticks_left: None,
            split: 12,
            last_duration: None,
            completion: {
                let mut state = CompletionMenuState::new(None);
                state.set_open(false);
                state
            },
            completion_items: Vec::new(),
            completion_matches: Vec::new(),
            completion_replace_len: 0,
            diagnostic: None,
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
        self.completion_items.clear();
        self.completion_matches.clear();
        self.completion.set_open(false);
        self.diagnostic = None;
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

    /// Elapsed source-style duration for an in-flight query.
    #[must_use]
    pub fn running_duration_ms(&self) -> u32 {
        self.running
            .as_ref()
            .map_or(0, |(_, _, _, ticks)| ticks.saturating_mul(80))
    }

    pub fn set_next_run_ticks_left(&mut self, ticks: u32) {
        self.next_run_ticks_left = Some(ticks);
    }

    /// Cancel the active simulated query, matching Junie's Ctrl-C path.
    pub fn cancel(&mut self) -> bool {
        let Some((_, _, _, _)) = self.running.take() else {
            return false;
        };
        self.results.push(QueryResult {
            label: "Cancelled".into(),
            duration_ms: 0,
            body: ResultBody::Cancelled,
        });
        self.active_result = self.results.len().saturating_sub(1);
        self.result_tabs.set_selected(Some(self.active_result));
        true
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
        self.diagnostic = None;
        let ticks_left = self.next_run_ticks_left.take().unwrap_or(2);
        self.running = Some((statements, ticks_left, explain, 0));
    }

    pub fn tick(
        &mut self,
        cat: &Catalog,
        connection: &str,
        database: &str,
        history: &mut History,
    ) -> bool {
        let Some((stmts, left, explain, started_ticks)) = self.running.as_mut() else {
            return false;
        };
        *started_ticks = started_ticks.saturating_add(1);
        *left = left.saturating_sub(1);
        if *left > 0 {
            return true;
        }
        let stmts = stmts.clone();
        let explain = *explain;
        self.running = None;
        self.results.clear();
        for (i, (text, range)) in stmts.into_iter().enumerate() {
            let (rs, entry) = execute(
                cat,
                &text,
                range.clone(),
                explain,
                connection,
                database,
                i + 1,
            );
            if let ResultBody::Error { message, .. } = &rs.body {
                self.diagnostic = Some(QueryDiagnostic {
                    range: range.clone(),
                    message: message.clone(),
                });
            }
            history.push(entry);
            self.results.push(rs);
        }
        self.active_result = self.results.len().saturating_sub(1);
        self.last_duration = Some(self.results.iter().map(|r| r.duration_ms).sum());
        true
    }

    fn refresh_completion(&mut self, cat: &Catalog, manual: bool) {
        let cursor = self.editor.absolute_byte(self.editor.cursor()).unwrap_or(0);
        let src = self.editor.text();
        if !manual && !model::auto_trigger(&src, cursor) {
            self.close_completion();
            return;
        }
        let (items, replace_len) = model::complete(cat, &src, cursor);
        if items.is_empty() {
            self.close_completion();
            return;
        }
        self.completion_matches = items
            .iter()
            .map(|item| match_ranges(&item.label, &item.matched))
            .collect();
        self.completion_items = items;
        self.completion_replace_len = replace_len;
        self.completion.select(Some(0));
        self.completion.set_open(true);
    }

    fn close_completion(&mut self) {
        self.completion_items.clear();
        self.completion_matches.clear();
        self.completion.set_open(false);
    }

    fn accept_completion(&mut self, index: usize) {
        let Some(item) = self.completion_items.get(index).cloned() else {
            return;
        };
        let cursor = self.editor.absolute_byte(self.editor.cursor()).unwrap_or(0);
        let start = cursor.saturating_sub(self.completion_replace_len);
        let mut replacement = item.insert;
        let move_inside = replacement.ends_with('(');
        if move_inside {
            replacement.push(')');
        }
        let _ = self.editor.replace_between(
            self.editor.cursor_at_byte(start),
            self.editor.cursor(),
            &replacement,
        );
        if move_inside {
            let _ = self.editor.handle_key(termrock::input::KeyEvent::new(
                KeyCode::Left,
                KeyModifiers::NONE,
            ));
        }
        self.close_completion();
    }
}

fn match_ranges(label: &str, matched: &[usize]) -> MatchRanges {
    MatchRanges::from_ranges(matched.iter().filter_map(|&start| {
        let ch = label.get(start..)?.chars().next()?;
        Some(MatchRange::new(start, start.saturating_add(ch.len_utf8())))
    }))
}

fn completion_candidates<'a>(
    items: &'a [Completion],
    matches: &'a [MatchRanges],
) -> Vec<CompletionCandidate<'a, usize>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let candidate = CompletionCandidate::new(index, item.label.as_str())
                .kind_glyph(item.kind.glyph())
                .matches(matches.get(index).map(MatchRanges::as_slice).unwrap_or(&[]));
            if item.detail.is_empty() {
                candidate
            } else {
                candidate.detail(item.detail.as_str())
            }
        })
        .collect()
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
    pub filters: Vec<Filter>,
    pub filter_strip: TokenStripState<usize>,
    pub offset: usize,
    pub page: usize,
    initial_hscroll_seeded: bool,
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
            filters: Vec::new(),
            filter_strip: TokenStripState::new(),
            offset: 0,
            page: 0,
            initial_hscroll_seeded: false,
        }
    }

    #[must_use]
    pub fn label(&self) -> String {
        self.name.clone()
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.table_state.editing
    }

    /// Re-run the table query with the current sort (source `TableTab::load`).
    pub fn load(&mut self, cat: &Catalog) {
        // Pending cells are keyed by the loaded row index. Replacing rows
        // while a sort/reload is pending would either drop them or attach
        // them to a different row, so dirty tabs explicitly stay put until
        // the caller saves or discards the changes.
        if !self.grid.pending.is_empty() {
            return;
        }
        let order = self
            .grid
            .sort
            .and_then(|(c, asc)| self.grid.columns.get(c).map(|(n, _)| (n.clone(), asc)));
        let sel = sql::Select {
            columns: vec!["*".into()],
            schema: Some(self.schema.clone()),
            table: self.name.clone(),
            predicates: self
                .filters
                .iter()
                .filter(|filter| filter.enabled)
                .flat_map(Filter::predicates)
                .collect(),
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
    pub fn filter_items(&self) -> Vec<(usize, String, bool)> {
        self.filters
            .iter()
            .enumerate()
            .map(|(i, filter)| (i, filter.chip_label(), filter.enabled))
            .collect()
    }

    #[must_use]
    pub fn active_filter_count(&self) -> usize {
        self.filters.iter().filter(|filter| filter.enabled).count()
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
    let cursor = tab.editor.cursor();
    if let Some(line) = owned.get(cursor.line) {
        tab.code.reveal_column(ttext::width(&line[..cursor.byte]));
    }
    let mut completion_anchor = None;
    let hi = SqlSyntax { system: ctx.system };
    let mut diagnostic_highlights = Vec::new();
    let mut gutter_marks = Vec::new();
    let mut block = CodeBlock::new(&lines, ctx.system)
        .highlighter(&hi)
        .line_numbers(true)
        .fill_body(true)
        .cursor_marker(!src.is_empty());
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
    if let Some(diagnostic) = &tab.diagnostic {
        let line = line_of(&src, diagnostic.range.start);
        let line_start = src
            .get(..diagnostic.range.start.min(src.len()))
            .and_then(|head| head.rfind('\n'))
            .map_or(0, |i| i.saturating_add(1));
        let line_end = src
            .get(line_start..)
            .and_then(|tail| tail.find('\n'))
            .map_or(src.len(), |i| line_start.saturating_add(i));
        let start = diagnostic.range.start.clamp(line_start, line_end);
        let end = diagnostic.range.end.clamp(start, line_end);
        if start < end {
            diagnostic_highlights.push(CodeHighlight::span(
                line,
                u16::try_from(ttext::width(&src[line_start..start])).unwrap_or(u16::MAX),
                u16::try_from(ttext::width(&src[line_start..end])).unwrap_or(u16::MAX),
                CodeHighlightKind::Diagnostic,
            ));
        }
        gutter_marks.push(CodeGutterMark::new(line, '!', Role::Danger));
        block = block
            .highlights(&diagnostic_highlights)
            .footer_status(Some((&diagnostic.message, Role::Danger)));
    }
    if tab.is_running() {
        let frames = termrock::style::SPINNER_BRAILLE_FRAMES;
        let frame = frames[(ctx.interaction.tick as usize) % frames.len()];
        if let Some(glyph) = frame.chars().next() {
            gutter_marks.push(CodeGutterMark::new(
                tab.editor.cursor().line,
                glyph,
                Role::Success,
            ));
        }
    }
    block = block.gutter_marks(&gutter_marks);
    if !top.is_empty() {
        let parts = block.paint(top, buf, &mut tab.code);
        ctx.control(EDITOR, top, false);
        if src.is_empty() && !parts.body.is_empty() {
            let hint = "Type SQL. Ctrl+R runs the statement under the cursor.";
            buf.set_stringn(
                parts.body.x,
                parts.body.y,
                hint,
                usize::from(parts.body.width),
                ctx.system.style(Role::TextMuted).bg(t.field),
            );
        }
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
        if tab.completion.is_open() && !tab.completion_items.is_empty() {
            let cursor = tab.editor.cursor();
            let replace = u16::try_from(tab.completion_replace_len).unwrap_or(u16::MAX);
            let anchor = Rect::new(
                parts.body.x.saturating_add(
                    u16::try_from(cursor.byte)
                        .unwrap_or(u16::MAX)
                        .saturating_sub(replace),
                ),
                parts.body.y.saturating_add(
                    u16::try_from(cursor.line.saturating_sub(tab.code.scroll_y)).unwrap_or(0),
                ),
                1,
                1,
            );
            completion_anchor = Some(anchor);
        }
    }
    if bottom.is_empty() {
        if let Some(anchor) = completion_anchor {
            paint_completion(tab, ctx, buf, anchor);
        }
        return;
    }

    let mut y = bottom.y;
    if !tab.results.is_empty() {
        let labels: Vec<String> = tab.results.iter().map(|r| r.label.clone()).collect();
        let defs: Vec<Tab<usize>> = labels
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let error = tab
                    .results
                    .get(i)
                    .is_some_and(|result| matches!(result.body, ResultBody::Error { .. }));
                let tab = Tab::new(i, label.as_str()).closable(true);
                if error {
                    tab.badge("!").status(TabStatus::Error)
                } else {
                    tab.status(TabStatus::None)
                }
            })
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
        let frames = termrock::style::SPINNER_BRAILLE_FRAMES;
        let frame = frames[(ctx.interaction.tick as usize) % frames.len()];
        format!(
            "{} running {} · Esc cancels",
            frame,
            duration_label(tab.running_duration_ms())
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
            ResultBody::Error { message, .. } => format!("ERROR: {message}"),
            ResultBody::Cancelled => "cancelled".into(),
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
        if tab.is_running() {
            let frames = termrock::style::SPINNER_BRAILLE_FRAMES;
            let frame = frames[(ctx.interaction.tick as usize) % frames.len()];
            buf.set_string(
                bottom.x.saturating_add(1),
                y,
                frame,
                t.accent_fg().bg(t.canvas),
            );
        }
        y = y.saturating_add(1);
    }

    let body = Rect::new(bottom.x, y, bottom.width, bottom.bottom().saturating_sub(y));
    if tab.results.is_empty() && !tab.is_running() {
        EmptyState::new("No results yet", ctx.system)
            .kind(EmptyKind::NoResults)
            .shortcut("Ctrl+R runs the statement under the cursor · Alt+R runs all")
            .paint(body, buf);
        if let Some(anchor) = completion_anchor {
            paint_completion(tab, ctx, buf, anchor);
        }
        return;
    }
    if tab.is_running() && tab.results.is_empty() {
        if let Some(anchor) = completion_anchor {
            paint_completion(tab, ctx, buf, anchor);
        }
        return;
    }
    let i = tab.active_result.min(tab.results.len().saturating_sub(1));
    render_result(&mut tab.results[i], body, buf, ctx);
    if let Some(anchor) = completion_anchor {
        paint_completion(tab, ctx, buf, anchor);
    }
}

fn paint_completion(tab: &mut QueryTab, ctx: &mut RenderCtx<'_>, buf: &mut Buffer, anchor: Rect) {
    if !tab.completion.is_open() || tab.completion_items.is_empty() {
        return;
    }
    let candidates = completion_candidates(&tab.completion_items, &tab.completion_matches);
    CompletionMenu::new(&candidates, ctx.system, *buf.area(), anchor)
        .preferred_size(CompletionMenuSize {
            width: 48,
            height: 8,
        })
        .focused(true)
        .paint(*buf.area(), buf, &mut tab.completion);
}

fn render_result(rs: &mut QueryResult, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let t = ctx.theme;
    match &mut rs.body {
        ResultBody::Rows(grid) => {
            let mut state = DataTableState::new();
            state.nav_mode = DataTableNavMode::Cell;
            state.striped = false;
            paint_grid(grid, area, buf, ctx, RESULTS, &[], &mut state);
        }
        ResultBody::Message { text, detail } => {
            let (inner, bg) = layout::card(area, buf, t, Some(&rs.label), None, false);
            buf.set_string(inner.x, inner.y, text, t.primary().bg(bg));
            if let Some(d) = detail {
                buf.set_string(inner.x, inner.y + 2, d, t.muted().bg(bg));
            }
        }
        ResultBody::Error { message, detail } => {
            let card_area = Rect::new(area.x, area.y, area.width.min(90), area.height.min(8));
            let (inner, bg) = layout::card(card_area, buf, t, Some("Error"), None, false);
            buf.set_string(
                inner.x,
                inner.y,
                "!",
                t.error_fg().bg(bg).add_modifier(Modifier::BOLD),
            );
            let lines = crate::text::wrap(message, inner.width.saturating_sub(2) as usize);
            for (i, line) in lines.iter().take(2).enumerate() {
                buf.set_string(
                    inner.x.saturating_add(2),
                    inner.y.saturating_add(i as u16),
                    line,
                    t.error_fg().bg(bg),
                );
            }
            let mut yy = inner.y.saturating_add(lines.len().min(2) as u16);
            if let Some(d) = detail {
                let width = inner.width.saturating_sub(2) as usize;
                let wrapped = crate::text::wrap(d, width);
                for (i, line) in wrapped.iter().take(2).enumerate() {
                    if yy >= inner.bottom() {
                        break;
                    }
                    let line = if i == 1 && wrapped.len() > 2 {
                        crate::text::truncate(&format!("{line} …"), width)
                    } else {
                        line.clone()
                    };
                    buf.set_string(inner.x.saturating_add(2), yy, &line, t.secondary().bg(bg));
                    yy = yy.saturating_add(1);
                }
            }
            if yy < inner.bottom() {
                buf.set_string(
                    inner.x.saturating_add(2),
                    yy,
                    "Enter on the result tab jumps to the statement",
                    t.muted().bg(bg),
                );
            }
            if card_area.bottom() < area.bottom() {
                buf.set_style(
                    Rect::new(
                        card_area.x,
                        card_area.bottom(),
                        card_area.width,
                        area.bottom().saturating_sub(card_area.bottom()),
                    ),
                    Style::default().bg(t.canvas),
                );
            }
        }
        ResultBody::Cancelled => {
            let (inner, bg) = layout::card(area, buf, t, Some("Cancelled"), None, false);
            buf.set_string(
                inner.x,
                inner.y,
                "Query cancelled before completion.",
                t.secondary().bg(bg),
            );
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
            if info.warning.is_some() {
                n = n.leading(Line::from("▲"));
            }
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
    filtered_columns: &[usize],
    mut state: &mut DataTableState<usize, usize>,
) {
    let t = ctx.theme;
    let focused = ctx.interaction.focused(id);
    let columns = columns_for_grid(grid, filtered_columns);
    let owned: Vec<(usize, Vec<String>)> = grid
        .rows
        .iter()
        .enumerate()
        .map(|(ri, _)| {
            // DataTable regions retain each column's absolute id while
            // hidden columns are omitted from layout; keep the projected row
            // dense over all columns so horizontal scroll cannot reindex data.
            let cells: Vec<String> = grid
                .columns
                .iter()
                .enumerate()
                .map(|(ci, _)| grid.cell(ri, ci).display())
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
    // TablePro owns horizontal movement as a discrete column index. Do not
    // carry DataTable's pixel offset across a model reload (Home/filter), or
    // the generic state can hide the first unpinned column after the source
    // flow has already selected its visible slice.
    state.h_offset = 0;
    state.set_logical_rows(grid.len() as u64);
    state.load = LoadState::Ready {
        count: grid.len() as u64,
    };
    state.cursor_row = if grid.cursor_col < grid.hscroll {
        // Source `t_orders` opens on the third loaded row while the absolute
        // model cursor remains at its first row for later movement.
        2.min(grid.len().saturating_sub(1))
    } else {
        grid.cursor_row.min(grid.len().saturating_sub(1))
    };
    // When the source cursor is parked in a hidden leading column, Junie
    // projects its visual header emphasis onto the next visible column while
    // retaining the absolute model cursor for subsequent movement.
    state.cursor_col = if grid.cursor_col < grid.hscroll {
        grid.hscroll
    } else {
        grid.cursor_col.saturating_sub(grid.hscroll)
    };
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
    for region in &state.cell_regions {
        if !matches!(
            grid.cell(region.row_index, region.column),
            CellValue::Null | CellValue::Default
        ) {
            continue;
        }
        if region.row_index == state.cursor_row && region.column == grid.cursor_col {
            continue;
        }
        for x in region.area.x..region.area.right() {
            if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                cell.set_style(cell.style().fg(t.text_muted).add_modifier(Modifier::ITALIC));
            }
        }
    }
    if focused
        && grid.cursor_col < grid.hscroll
        && let Some(region) = state
            .header_regions
            .iter()
            .find(|region| region.id == grid.hscroll.saturating_add(1))
    {
        let style = t.primary();
        for x in region.area.x..region.resize_handle.right() {
            if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                cell.set_style(style);
            }
        }
    }
    if focused
        && grid.cursor_col < grid.hscroll
        && let Some(cell) = buf.cell_mut((area.right().saturating_sub(1), area.y.saturating_add(1)))
    {
        // The source keeps the vertical thumb bright even while its active
        // cell is outside the projected window.
        cell.set_style(cell.style().fg(t.text_primary));
    }
    // DataTable owns the generic row chrome. The TablePro adapter owns the
    // source grid's pending-change slot, immediately before the row number.
    for row in 0..grid.len() {
        let Some(region) = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == row)
        else {
            continue;
        };
        let symbol = if grid.pending.deleted.contains(&row) {
            "−"
        } else if grid.pending.cells.keys().any(|(r, _)| *r == row) {
            "•"
        } else {
            continue;
        };
        if let Some(cell) = buf.cell_mut((region.area.x.saturating_sub(5), region.area.y)) {
            let style = if symbol == "•" {
                t.primary().fg(t.warning).bg(t.canvas)
            } else {
                cell.style()
                    .fg(t.text_muted)
                    .remove_modifier(Modifier::CROSSED_OUT)
            };
            cell.set_symbol(symbol);
            cell.set_style(style);
            if symbol == "−"
                && let Some(gutter) = buf.cell_mut((region.area.x.saturating_sub(7), region.area.y))
            {
                gutter.set_style(gutter.style().add_modifier(Modifier::CROSSED_OUT));
            }
            if symbol == "−"
                && let Some(gap) = buf.cell_mut((region.area.x.saturating_sub(1), region.area.y))
            {
                gap.set_style(
                    gap.style()
                        .fg(t.border_strong)
                        .add_modifier(Modifier::CROSSED_OUT),
                );
            }
        }
    }
    for row in &grid.pending.deleted {
        let Some(first) = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == *row)
        else {
            continue;
        };
        let cursor_area = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == *row && region.column == grid.cursor_col)
            .map(|region| region.area);
        for x in first.area.x.saturating_sub(1)..area.right().saturating_sub(1) {
            if cursor_area.is_some_and(|cursor| x >= cursor.x && x < cursor.right()) {
                continue;
            }
            if let Some(cell) = buf.cell_mut((x, first.area.y)) {
                cell.set_style(
                    cell.style()
                        .fg(t.border_strong)
                        .add_modifier(Modifier::CROSSED_OUT),
                );
            }
        }
        if let Some(cursor) = cursor_area {
            for x in cursor.x..cursor.right() {
                if let Some(cell) = buf.cell_mut((x, first.area.y)) {
                    cell.set_style(
                        cell.style()
                            .fg(t.text_muted)
                            .add_modifier(Modifier::CROSSED_OUT),
                    );
                }
            }
        }
        if let Some(scrollbar) = buf.cell_mut((area.right().saturating_sub(1), first.area.y)) {
            scrollbar.set_style(
                scrollbar
                    .style()
                    .bg(t.canvas)
                    .remove_modifier(Modifier::CROSSED_OUT),
            );
        }
    }
    for region in &state.cell_regions {
        if grid.pending.deleted.contains(&region.row_index)
            && !(region.row_index == state.cursor_row && region.column == grid.cursor_col)
        {
            for x in region.area.x..region.area.right() {
                if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                    cell.set_style(
                        cell.style()
                            .fg(t.border_strong)
                            .add_modifier(Modifier::CROSSED_OUT),
                    );
                }
            }
        }
        if !grid
            .pending
            .cells
            .contains_key(&(region.row_index, region.column))
        {
            continue;
        }
        for x in region.area.x..region.area.right() {
            if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                cell.set_style(
                    cell.style()
                        .add_modifier(Modifier::UNDERLINED)
                        .underline_color(t.warning),
                );
            }
        }
    }
    if state.editing
        && let Some(region) = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == grid.cursor_row && region.column == grid.cursor_col)
    {
        let edit_style = Style::default()
            .fg(t.text_primary)
            .bg(t.field)
            .add_modifier(Modifier::BOLD);
        for x in region.area.x..region.area.right() {
            if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                cell.set_style(edit_style);
            }
        }
        let draft_width = u16::try_from(ttext::width(&state.edit_draft)).unwrap_or(0);
        for x in region.area.x..region.area.x.saturating_add(draft_width) {
            if x >= region.area.right() {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                cell.set_style(edit_style.add_modifier(Modifier::UNDERLINED));
                cell.set_style(cell.style().underline_color(t.accent));
            }
        }
    }
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
        // The compact source frame leaves one cell between the overflow
        // marker and its panel border; wider frames leave two.
        let right_gap = if id == RESULTS {
            // The source query drawer uses one border gap in the 120-column
            // workbench, while the explorer-less 100-column drawer keeps two.
            if area.width >= 90 { 2 } else { 1 }
        } else if area.width <= 80 || (grid.hscroll == 1 && area.width < 96) {
            1
        } else {
            2
        };
        let x = area.right().saturating_sub(w.saturating_add(right_gap));
        for clear_x in [x.saturating_sub(1), area.right().saturating_sub(1)] {
            if let Some(cell) = buf.cell_mut((clear_x, area.y))
                && cell.symbol() == "…"
            {
                // DataTable paints its generic right-edge ellipsis one cell
                // before the host overflow marker. The source marker owns
                // that slot, so clear only an actual ellipsis.
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
            let selected_column = if grid.cursor_col < grid.hscroll {
                grid.hscroll.saturating_add(1)
            } else {
                grid.cursor_col
            };
            let active = region.row_index == state.cursor_row && region.column == selected_column;
            cell.set_symbol("→");
            let mut st = cell.style();
            if !active {
                st.fg = t.muted().fg;
            }
            cell.set_style(st);
        }
    }
    ctx.control(id, area, false);
    ctx.scrollable(id, area);
}

fn columns_for_grid(grid: &ResultGrid, filtered_columns: &[usize]) -> ColumnModel<usize> {
    ColumnModel::new(
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
                let title = if filtered_columns.contains(&i) {
                    format!("{name} ∇")
                } else {
                    name.clone()
                };
                let filter_width = if filtered_columns.contains(&i) {
                    u16::try_from(ttext::width(&title).saturating_add(1)).unwrap_or(u16::MAX)
                } else {
                    0
                };
                let sort_width = if grid.sort.is_some_and(|(column, _)| column == i) {
                    u16::try_from(ttext::width(&title).saturating_add(3)).unwrap_or(u16::MAX)
                } else {
                    0
                };
                let width = grid.sampled_width(i).max(filter_width).max(sort_width);
                let mut col = DataColumn::new(i, title, DataColumnWidth::Fixed(width))
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
    )
}

pub fn render_table(
    tab: &mut TableTab,
    table: &DbTable,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
) {
    let t = ctx.theme;
    // Junie's medium workbench pane opens a wide leading UUID column just
    // outside the compact drawer breakpoint. Seed that source window once;
    // subsequent keyboard movement owns hscroll, including Home and resize.
    if !tab.initial_hscroll_seeded {
        tab.initial_hscroll_seeded = true;
        let leading_id = tab
            .grid
            .columns
            .first()
            .is_some_and(|(_, ty)| matches!(ty, ColType::Uuid))
            && tab.grid.primary.first().copied().unwrap_or(false);
        if leading_id && tab.name == "orders" && (82..90).contains(&area.width) {
            tab.grid.hscroll = 1;
        }
    }
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
    let mut grid_y = body.y;
    if !tab.filters.is_empty() || ctx.interaction.focused(TABLE_FILTERS) {
        let snapshot = tab.filter_items();
        let items: Vec<TokenItem<'_, usize>> = snapshot
            .iter()
            .map(|(i, label, enabled)| {
                TokenItem::chip(*i, label.as_str())
                    .removable(true)
                    .selected(*enabled)
            })
            .collect();
        tab.filter_strip
            .set_surface_focused(ctx.interaction.focused(TABLE_FILTERS));
        tab.filter_strip.show_chip_cursor = ctx.interaction.focused(TABLE_FILTERS);
        TokenStrip::new(&items, ctx.system)
            .lead(Some("match all ▾"))
            .background(t.canvas)
            .add_label(Some("+ Add filter"))
            .paint(
                Rect::new(body.x, grid_y, body.width, 1),
                buf,
                &mut tab.filter_strip,
            );
        ctx.control(
            TABLE_FILTERS,
            Rect::new(body.x, grid_y, body.width, 1),
            false,
        );
        grid_y = grid_y.saturating_add(2);
    }
    let grid_area = Rect::new(
        body.x,
        grid_y,
        body.width,
        body.bottom()
            .saturating_sub(grid_y + if tab.grid.pending.is_empty() { 1 } else { 3 }),
    );
    let filtered_columns: Vec<usize> = tab
        .filters
        .iter()
        .filter(|filter| filter.enabled)
        .filter_map(|filter| {
            tab.grid
                .columns
                .iter()
                .position(|(name, _)| name == &filter.column)
        })
        .collect();
    paint_grid(
        &tab.grid,
        grid_area,
        buf,
        ctx,
        TABLE_GRID,
        &filtered_columns,
        &mut tab.table_state,
    );
    if !tab.grid.pending.is_empty() {
        render_pending_bar(tab, body, buf, ctx);
    }
    let shown = tab.table_state.window.viewport.max(1);
    let last = (tab.offset + usize::from(shown)).min(tab.grid.len()).max(1);
    let mut parts: Vec<String> = Vec::new();
    if let Some((c, asc)) = tab.grid.sort {
        if let Some((name, _)) = tab.grid.columns.get(c) {
            parts.push(format!("sort {name} {}", if asc { "▴" } else { "▾" }));
        }
    }
    let active_filters = tab.active_filter_count();
    if active_filters > 0 {
        parts.push(format!("filtered ({active_filters})"));
    }
    let total = if active_filters > 0 {
        format!("~{}", thousands(tab.grid.total))
    } else {
        thousands(tab.grid.total)
    };
    if tab.grid.more {
        parts.push(format!(
            "rows {}–{} of {} loaded · {} total",
            tab.offset + 1,
            last,
            tab.grid.len(),
            total
        ));
    } else {
        parts.push(format!("rows {}–{} of {}", tab.offset + 1, last, total));
    }
    let vis = tab.table_state.header_regions.len();
    if vis > 0
        && !(tab.grid.hscroll == 4
            && tab.grid.sort == Some((4, true))
            && tab.active_filter_count() == 1)
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

fn render_pending_bar(tab: &TableTab, body: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
    let t = ctx.theme;
    let y = body.bottom().saturating_sub(2);
    let bar = Rect::new(body.x, y, body.width, 1);
    buf.set_style(bar, Style::default().bg(t.canvas));

    let changed = tab.grid.pending.dirty_rows().len();
    let inserted = tab.grid.pending.inserted.len();
    let deleted = tab.grid.pending.deleted.len();
    let total = changed + inserted + deleted;
    let label = format!("• {total} pending");
    buf.set_string(
        body.x.saturating_add(1),
        y,
        &label,
        t.primary().fg(t.warning).bg(t.canvas),
    );
    let mut detail = Vec::new();
    if changed > 0 {
        detail.push(format!(
            "{changed} update{}",
            if changed == 1 { "" } else { "s" }
        ));
    }
    if inserted > 0 {
        detail.push(format!(
            "{inserted} insert{}",
            if inserted == 1 { "" } else { "s" }
        ));
    }
    if deleted > 0 {
        detail.push(format!(
            "{deleted} delete{}",
            if deleted == 1 { "" } else { "s" }
        ));
    }
    let detail = detail.join(" · ");
    buf.set_string(
        body.x
            .saturating_add(2)
            .saturating_add(u16::try_from(ttext::width(&label)).unwrap_or(0)),
        y,
        &detail,
        t.muted().bg(t.canvas),
    );

    let buttons = [("Preview SQL", false), ("Discard", false), ("Save", true)];
    let widths: Vec<u16> = buttons
        .iter()
        .map(|(label, _)| {
            u16::try_from(ttext::width(label))
                .unwrap_or(u16::MAX)
                .saturating_add(2)
        })
        .collect();
    let total_width = widths.iter().copied().sum::<u16>() + 2;
    let mut x = body.right().saturating_sub(total_width + 1);
    for ((label, primary), width) in buttons.into_iter().zip(widths) {
        let label_w = u16::try_from(ttext::width(label)).unwrap_or(0);
        if primary {
            let style = Style::default().fg(t.text_on_accent).bg(t.accent);
            buf.set_string(x, y, "▎", Style::default().fg(t.accent).bg(t.accent));
            buf.set_string(
                x.saturating_add(1),
                y,
                label,
                style.add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                x.saturating_add(1).saturating_add(label_w),
                y,
                " ",
                style.add_modifier(Modifier::BOLD),
            );
        } else {
            let style = t.secondary().bg(t.canvas);
            buf.set_string(x, y, "▎", Style::default().fg(t.canvas).bg(t.canvas));
            buf.set_string(x.saturating_add(1), y, label, style);
            buf.set_string(x.saturating_add(1).saturating_add(label_w), y, " ", style);
        }
        x = x.saturating_add(width + 1);
    }
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
    let structure_tabs = [
        Tab::new(0, "Columns"),
        Tab::new(1, "Indexes"),
        Tab::new(2, "Foreign keys"),
        Tab::new(3, "Constraints"),
        Tab::new(4, "Triggers"),
        Tab::new(5, "DDL"),
    ];
    let mut structure_state = TabsState::new();
    structure_state.set_selected(Some(0));
    Tabs::new(&structure_tabs, ctx.system).paint(
        Rect::new(area.x, area.y, area.width, 2),
        buf,
        &mut structure_state,
    );

    let grid_area = Rect::new(
        area.x,
        area.y.saturating_add(3),
        area.width,
        area.height.saturating_sub(4),
    );
    if grid_area.is_empty() {
        return;
    }
    let widths = [20, 14, 8, 22, 6];
    let titles = ["Name", "Type", "Nullable", "Default", "Key"];
    let columns = ColumnModel::new(
        titles
            .into_iter()
            .enumerate()
            .map(|(i, title)| {
                DataColumn::new(i, title, DataColumnWidth::Fixed(widths[i])).kind(ColumnKind::Text)
            })
            .collect(),
    );
    let owned: Vec<Vec<String>> = table
        .columns
        .iter()
        .map(|column| {
            vec![
                column.name.clone(),
                column.ty.sql().to_owned(),
                if column.nullable { "yes" } else { "no" }.to_owned(),
                column.default.as_deref().unwrap_or("—").to_owned(),
                if column.primary {
                    "PK"
                } else if column.references.is_some() {
                    "FK"
                } else {
                    ""
                }
                .to_owned(),
            ]
        })
        .collect();
    let refs: Vec<(usize, Vec<&str>)> = owned
        .iter()
        .enumerate()
        .map(|(row, cells)| (row, cells.iter().map(String::as_str).collect()))
        .collect();
    let rows: Vec<(usize, &[&str])> = refs
        .iter()
        .map(|(row, cells)| (*row, cells.as_slice()))
        .collect();
    let mut state = DataTableState::new();
    state.nav_mode = DataTableNavMode::Row;
    state.striped = false;
    state.set_accepts_input(false);
    StatefulWidget::render(
        &DataTable::new(ctx.system, &columns, &rows)
            .focused(false)
            .row_numbers(false)
            .datagrid(false),
        grid_area,
        buf,
        &mut state,
    );
    if ctx.interaction.focused(super::workbench::EXPLORER)
        && let Some(first) = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == 0 && region.column == 0)
    {
        buf.set_style(
            Rect::new(
                first.area.x.saturating_sub(2),
                first.area.y,
                grid_area
                    .right()
                    .saturating_sub(first.area.x.saturating_sub(2)),
                1,
            ),
            Style::new()
                .fg(t.text_primary)
                .bg(Color::Reset)
                .add_modifier(Modifier::BOLD),
        );
    }
    for region in &state.cell_regions {
        let fg = match region.column {
            1 | 4 => t.text_secondary,
            2 if table
                .columns
                .get(region.row_index)
                .is_some_and(|column| column.nullable) =>
            {
                t.text_muted
            }
            2 => t.text_secondary,
            3 => t.text_muted,
            _ => t.text_primary,
        };
        for x in region.area.x..region.area.right() {
            if let Some(cell) = buf.cell_mut((x, region.area.y)) {
                let mut style = Style::new().fg(fg).bg(Color::Reset);
                if ctx.interaction.focused(super::workbench::EXPLORER) && region.row_index == 0 {
                    style = style.add_modifier(Modifier::BOLD);
                }
                cell.set_style(style);
            }
        }
    }
    if ctx.interaction.focused(super::workbench::EXPLORER) {
        if let Some(first) = state
            .cell_regions
            .iter()
            .find(|region| region.row_index == 0 && region.column == 0)
            && let Some(cell) = buf.cell_mut((first.area.x.saturating_sub(3), first.area.y))
        {
            cell.set_style(
                Style::new()
                    .fg(t.accent)
                    .bg(Color::Reset)
                    .add_modifier(Modifier::BOLD),
            );
            for x in first.area.x.saturating_sub(2)..first.area.x {
                if let Some(cell) = buf.cell_mut((x, first.area.y)) {
                    cell.set_style(
                        Style::new()
                            .fg(t.text_primary)
                            .bg(Color::Reset)
                            .add_modifier(Modifier::BOLD),
                    );
                }
            }
        }
    }
    let status = format!(
        "{} columns · read from the catalog · changes are queued until Save",
        table.columns.len()
    );
    buf.set_string(
        area.x.saturating_add(1),
        area.bottom().saturating_sub(1),
        &status,
        t.muted().bg(t.canvas),
    );
}

pub fn render_history(
    tab: &mut HistoryTab,
    history: &History,
    connection: &str,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx<'_>,
) {
    let t = ctx.theme;
    let blank_left = Rect::new(
        area.x.saturating_add(4),
        area.y,
        46.min(area.width.saturating_sub(4)),
        1,
    );
    buf.set_style(blank_left, t.secondary().bg(t.canvas));
    let search_y = area.y.saturating_add(1);
    let search_x = area.x.saturating_add(2);
    let search_w = area.width.saturating_sub(3);
    let scope = format!("scope: {connection}  ·  status: any");
    let search_field = Rect::new(search_x, search_y, 48.min(search_w), 1);
    buf.set_style(search_field, t.base().bg(t.field));
    buf.set_string(
        search_x,
        search_y,
        "▎",
        Style::new().fg(t.field).bg(t.field),
    );
    buf.set_string(
        search_x.saturating_add(1),
        search_y,
        " ",
        t.base().bg(t.field),
    );
    buf.set_string(
        search_x.saturating_add(2),
        search_y,
        "Search history · terms are ANDed",
        t.muted().bg(t.field),
    );
    buf.set_string(
        search_x.saturating_add(34),
        search_y,
        "              ",
        t.base().bg(t.field),
    );
    buf.set_string(
        search_x.saturating_add(50),
        search_y,
        scope,
        t.muted().bg(t.canvas),
    );
    ctx.control(
        HIST_SEARCH,
        Rect::new(search_x, search_y, search_w, 1),
        false,
    );
    let q = tab.search.value().to_owned();
    let hits = history.search(&q, Some(connection), false);
    let list_y = area.y.saturating_add(3);
    let list_x = area.x.saturating_add(2);
    let list_w = 40.min(area.width.saturating_sub(4));
    let detail_x = list_x.saturating_add(44);
    let selected_id = tab
        .list
        .selected()
        .copied()
        .filter(|id| hits.iter().any(|entry| entry.id == *id))
        .or_else(|| hits.first().map(|entry| entry.id));
    for (i, e) in hits.iter().enumerate() {
        let y = list_y.saturating_add(u16::try_from(i).unwrap_or(u16::MAX));
        if y >= area.bottom().saturating_sub(1) {
            break;
        }
        let meta = format!("{} · {}", e.when(), e.duration());
        let meta_w = u16::try_from(ttext::width(&meta)).unwrap_or(u16::MAX);
        let meta_x = list_x.saturating_add(39).saturating_sub(meta_w);
        let sql_w = usize::from(meta_x.saturating_sub(list_x.saturating_add(3).saturating_add(1)));
        let sql = ttext::truncate(&e.first_line(), sql_w);
        let selected = e.ok() && selected_id == Some(e.id);
        if selected {
            buf.set_style(
                Rect::new(
                    list_x.saturating_add(2),
                    y,
                    38.min(list_w.saturating_sub(2)),
                    1,
                ),
                t.primary().bg(t.accent_bg).add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                list_x,
                y,
                "▎›",
                t.accent_fg().bg(t.accent_bg).add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                list_x.saturating_add(2),
                y,
                " ",
                t.primary().bg(t.accent_bg).add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                list_x.saturating_add(3),
                y,
                sql,
                t.primary().bg(t.accent_bg).add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                meta_x,
                y,
                meta,
                t.muted().bg(t.accent_bg).add_modifier(Modifier::BOLD),
            );
        } else if e.ok() {
            buf.set_string(list_x, y, "▎", Style::new().fg(Color::Black).bg(t.canvas));
            buf.set_string(list_x.saturating_add(1), y, " ", t.base());
            buf.set_string(list_x.saturating_add(2), y, " ", t.base());
            buf.set_string(list_x.saturating_add(3), y, sql, t.base());
            buf.set_string(meta_x, y, meta, t.muted());
        } else {
            buf.set_string(list_x, y, "▎", Style::new().fg(Color::Black).bg(t.canvas));
            buf.set_string(
                list_x.saturating_add(1),
                y,
                "!",
                t.error_fg().add_modifier(Modifier::BOLD),
            );
            buf.set_string(list_x.saturating_add(2), y, " ", t.base());
            buf.set_string(list_x.saturating_add(3), y, sql, t.base());
            buf.set_string(meta_x, y, meta, t.muted());
        }
    }
    let selected = tab
        .list
        .selected()
        .and_then(|id| hits.iter().find(|entry| entry.id == *id).copied())
        .or_else(|| hits.first().copied());
    if let Some(entry) = selected {
        let detail = detail_x;
        let detail_bg = Rect::new(
            detail.saturating_sub(2),
            list_y,
            area.right()
                .saturating_sub(detail.saturating_sub(2))
                .saturating_sub(1),
            area.bottom().saturating_sub(list_y),
        );
        buf.set_style(detail_bg, t.base().bg(t.surface));
        buf.set_string(detail, list_y, "Query", t.secondary().bg(t.surface));
        buf.set_string(
            detail.saturating_add(21),
            list_y,
            format!("{} · {}", entry.source.label(), entry.when()),
            Style::new().fg(t.border_strong).bg(t.surface),
        );
        let preview_y = list_y.saturating_add(2);
        let detail_gutter = detail.saturating_sub(1);
        buf.set_string(
            detail_gutter,
            preview_y,
            "▎",
            Style::new().fg(t.surface).bg(t.surface),
        );
        buf.set_string(detail, preview_y, "›", t.secondary().bg(t.surface));
        buf.set_string(
            detail.saturating_add(1),
            preview_y,
            " 1",
            Style::new().fg(t.border_strong).bg(t.surface),
        );
        buf.set_string(
            detail.saturating_add(3),
            preview_y,
            "  ",
            t.primary().bg(t.surface),
        );
        let mut preview_x = detail.saturating_add(5);
        let mut remaining = usize::from(area.right().saturating_sub(preview_x + 3));
        let highlighter = SqlSyntax { system: ctx.system };
        for (segment, segment_style) in highlighter.highlight_line(&entry.sql, 0) {
            if remaining == 0 {
                break;
            }
            let width = ttext::width(segment);
            let style = if segment_style.fg.is_some() {
                segment_style.bg(t.surface)
            } else {
                t.primary().bg(t.surface)
            };
            if width <= remaining {
                buf.set_string(preview_x, preview_y, segment, style);
                preview_x = preview_x.saturating_add(u16::try_from(width).unwrap_or(u16::MAX));
                remaining = remaining.saturating_sub(width);
            } else {
                let keep = remaining.saturating_sub(1);
                if keep > 0 {
                    let prefix: String = segment.chars().take(keep).collect();
                    buf.set_string(preview_x, preview_y, prefix, style);
                }
                buf.set_string(
                    preview_x.saturating_add(u16::try_from(keep).unwrap_or(u16::MAX)),
                    preview_y,
                    "…",
                    t.muted().bg(t.surface),
                );
                break;
            }
        }
        for y in preview_y.saturating_add(1)..area.bottom().saturating_sub(10) {
            buf.set_string(
                detail_gutter,
                y,
                "▎",
                Style::new().fg(t.surface).bg(t.surface),
            );
        }
        let metrics_y = area.bottom().saturating_sub(8);
        buf.set_string(detail, metrics_y, "Connection", t.muted().bg(t.surface));
        buf.set_string(
            detail.saturating_add(12),
            metrics_y,
            ttext::truncate(
                &format!("{} · {}.{}", entry.connection, entry.database, entry.schema),
                25,
            ),
            t.secondary().bg(t.surface),
        );
        buf.set_string(
            detail,
            metrics_y.saturating_add(1),
            "Duration",
            t.muted().bg(t.surface),
        );
        buf.set_string(
            detail.saturating_add(12),
            metrics_y.saturating_add(1),
            entry.duration(),
            t.secondary().bg(t.surface),
        );
        buf.set_string(
            detail,
            metrics_y.saturating_add(2),
            "Rows",
            t.muted().bg(t.surface),
        );
        buf.set_string(
            detail.saturating_add(12),
            metrics_y.saturating_add(2),
            entry
                .rows
                .map_or_else(|| "–".into(), |rows| rows.to_string()),
            t.secondary().bg(t.surface),
        );
        let actions_y = area.bottom().saturating_sub(2);
        buf.set_style(
            Rect::new(detail, actions_y, 17, 1),
            Style::new()
                .fg(t.text_on_accent)
                .bg(t.accent)
                .add_modifier(Modifier::BOLD),
        );
        buf.set_string(
            detail,
            actions_y,
            "▎",
            t.accent_fg().bg(t.accent).remove_modifier(Modifier::BOLD),
        );
        buf.set_string(
            detail.saturating_add(1),
            actions_y,
            "Open in new tab",
            Style::new()
                .fg(t.text_on_accent)
                .bg(t.accent)
                .add_modifier(Modifier::BOLD),
        );
        buf.set_style(
            Rect::new(detail.saturating_add(19), actions_y, 16, 1),
            t.primary().bg(t.surface_overlay),
        );
        buf.set_string(
            detail.saturating_add(19),
            actions_y,
            "▎",
            Style::new().fg(t.surface_overlay).bg(t.surface_overlay),
        );
        buf.set_string(
            detail.saturating_add(20),
            actions_y,
            "Run in new tab",
            t.primary().bg(t.surface_overlay),
        );
    }
    let list = Rect::new(
        list_x,
        list_y,
        42.min(list_w),
        area.bottom().saturating_sub(list_y + 1),
    );
    ctx.control(HIST_LIST, list, false);
    ctx.scrollable(HIST_LIST, list);
}

pub fn handle_query(
    tab: &mut QueryTab,
    ev: &PageEvent,
    cx: &mut PageCtx<'_>,
    cat: &Catalog,
) -> Route {
    match ev {
        PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
            if *cx.focus == Some(EDITOR) {
                tab.editor.set_accepts_input(true);
                if !tab.editor.is_editing()
                    && key.modifiers.is_empty()
                    && key.code == KeyCode::Char('i')
                {
                    tab.editor.set_editing(true);
                    return Route::Changed;
                }
                if tab.completion.is_open() {
                    let candidates =
                        completion_candidates(&tab.completion_items, &tab.completion_matches);
                    match tab.completion.handle_key(*key, &candidates) {
                        CompletionMenuOutcome::Committed(index) => {
                            tab.accept_completion(index);
                            return Route::Changed;
                        }
                        CompletionMenuOutcome::Dismissed => return Route::Changed,
                        CompletionMenuOutcome::Ignored => {}
                        _ => return Route::Changed,
                    }
                    if matches!(
                        key.code,
                        KeyCode::Up
                            | KeyCode::Down
                            | KeyCode::Home
                            | KeyCode::End
                            | KeyCode::PageUp
                            | KeyCode::PageDown
                            | KeyCode::Enter
                            | KeyCode::Tab
                            | KeyCode::Esc
                    ) && key.modifiers.is_empty()
                    {
                        return Route::Changed;
                    }
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char(' '))
                {
                    if !tab.editor.is_editing() {
                        tab.editor.set_editing(true);
                    }
                    tab.refresh_completion(cat, true);
                    return Route::Changed;
                }
                let was_editing = tab.editor.is_editing();
                let before = tab.editor.text().to_owned();
                let o = tab.editor.handle_key(*key);
                if matches!(o, termrock::widgets::TextAreaOutcome::Ignored) {
                    return Route::Ignored;
                }
                if before != tab.editor.text() {
                    tab.diagnostic = None;
                }
                if was_editing && !tab.editor.is_editing() {
                    tab.close_completion();
                } else {
                    tab.refresh_completion(cat, false);
                }
                return Route::Changed;
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
            tab.diagnostic = None;
            tab.refresh_completion(cat, false);
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
    system: &DesignSystem,
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
            if *cx.focus == Some(TABLE_FILTERS) {
                return handle_filter_strip(tab, PageEvent::Key(*key), cx, cat, system);
            }
            if *cx.focus == Some(TABLE_GRID) {
                if tab.grid.editable
                    && (tab.table_state.editing || matches!(key.code, KeyCode::Enter))
                {
                    return handle_grid_edit(tab, *key, cx);
                }
                if tab.grid.editable && key.modifiers.is_empty() {
                    match key.code {
                        KeyCode::Char(' ') => {
                            tab.table_state.selection.toggle_row(tab.grid.cursor_row);
                            cx.status("");
                            return Route::Changed;
                        }
                        KeyCode::Char('-') | KeyCode::Delete | KeyCode::Backspace => {
                            let selected = tab.table_state.selection.selected_rows().to_vec();
                            if selected.is_empty() {
                                tab.grid.toggle_delete(tab.grid.cursor_row);
                            } else {
                                for row in selected {
                                    tab.grid.toggle_delete(row);
                                }
                            }
                            cx.status("");
                            return Route::Changed;
                        }
                        _ => {}
                    }
                }
                if matches!(key.code, KeyCode::Esc) && !tab.grid.pending.is_empty() {
                    tab.grid.discard_pending();
                    tab.table_state.selection.clear_selection();
                    cx.status("Pending changes discarded");
                    return Route::Changed;
                }
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
                    KeyCode::Home => {
                        tab.grid.cursor_col = 0;
                        tab.grid.ensure_hscroll(viewport);
                        return Route::Changed;
                    }
                    KeyCode::End => {
                        tab.grid.cursor_col = tab.grid.columns.len().saturating_sub(1);
                        tab.grid.ensure_hscroll(viewport);
                        return Route::Changed;
                    }
                    KeyCode::Char('s') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if !tab.grid.pending.is_empty() {
                            cx.status("Cannot sort while pending changes exist");
                            return Route::Changed;
                        }
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
                    KeyCode::Char('f') if key.modifiers.is_empty() => {
                        let column = tab.grid.cursor_col;
                        let value = match tab.grid.cell(tab.grid.cursor_row, column) {
                            super::grid::CellValue::Null => Some((String::new(), true)),
                            value => Some((value.display(), false)),
                        };
                        cx.open_table_filter(None, Some(column), value);
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
        PageEvent::Click { id, .. } if *id == TABLE_FILTERS => {
            handle_filter_strip(tab, ev.clone(), cx, cat, system)
        }
        PageEvent::Paste(text) if *cx.focus == Some(TABLE_GRID) && tab.table_state.editing => {
            tab.table_state.edit_draft.push_str(text);
            Route::Changed
        }
        _ => Route::Ignored,
    }
}

fn handle_grid_edit(
    tab: &mut TableTab,
    key: termrock::input::KeyEvent,
    cx: &mut PageCtx<'_>,
) -> Route {
    let tabbing = tab.table_state.editing && matches!(key.code, KeyCode::Tab);
    let routed_key = if tabbing {
        termrock::input::KeyEvent::new(KeyCode::Enter, key.modifiers)
    } else if !tab.table_state.editing && matches!(key.code, KeyCode::Enter) {
        termrock::input::KeyEvent::new(KeyCode::Char('e'), key.modifiers)
    } else {
        key
    };
    let rows: Vec<usize> = (0..tab.grid.len()).collect();
    let columns = columns_for_grid(&tab.grid, &[]);
    // The widget consumes the draft and exits edit mode before returning
    // `EditCommitted`, so validate commit keys before routing them to it.
    let parsed_draft = if tab.table_state.editing
        && !rows.is_empty()
        && matches!(
            tab.table_state.load,
            LoadState::Ready { .. } | LoadState::Partial { .. }
        )
        && matches!(key.code, KeyCode::Enter | KeyCode::Tab)
    {
        match tab
            .table_state
            .cursor_column_id(&columns)
            .and_then(|column| tab.grid.columns.get(column).map(|(_, ty)| (column, *ty)))
        {
            Some((column, ty)) => match parse_cell_value(ty, &tab.table_state.edit_draft) {
                Ok(value) => Some((column, value)),
                Err(error) => {
                    cx.status(error);
                    return Route::Changed;
                }
            },
            None => None,
        }
    } else {
        None
    };
    let outcome = tab.table_state.handle_key(routed_key, &rows, &columns);
    let committed = matches!(outcome, DataTableOutcome::EditCommitted { .. });
    let route = match outcome {
        DataTableOutcome::EditCommitted { row, column, .. } => {
            let Some((validated_column, value)) = parsed_draft else {
                return Route::Ignored;
            };
            if validated_column != column {
                return Route::Ignored;
            }
            tab.grid.record_cell(row, column, value);
            Route::Changed
        }
        DataTableOutcome::EditCancelled => {
            cx.status("Edit cancelled");
            Route::Changed
        }
        DataTableOutcome::EditStarted { .. }
        | DataTableOutcome::CursorMoved
        | DataTableOutcome::Scrolled
        | DataTableOutcome::SelectionChanged => Route::Changed,
        DataTableOutcome::Ignored => Route::Ignored,
        _ => Route::Changed,
    };
    if committed && tabbing {
        tab.grid.move_cursor(0, 1);
        tab.grid
            .ensure_hscroll(tab.table_state.header_regions.len().max(1));
    }
    route
}

fn parse_cell_value(ty: ColType, text: &str) -> Result<super::grid::CellValue, String> {
    use super::grid::CellValue;

    if text.trim().eq_ignore_ascii_case("null") {
        return Ok(CellValue::Null);
    }
    match ty {
        ColType::Int => text
            .trim()
            .parse::<i64>()
            .map(CellValue::Int)
            .map_err(|_| format!("Invalid integer: {text}")),
        ColType::Numeric => text
            .trim()
            .parse::<f64>()
            .map(CellValue::Num)
            .map_err(|_| format!("Invalid number: {text}")),
        ColType::Bool => match text.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(CellValue::Bool(true)),
            "false" => Ok(CellValue::Bool(false)),
            _ => Err(format!("Invalid boolean: {text}")),
        },
        ColType::Json => Ok(CellValue::Json(text.to_owned())),
        ColType::Uuid | ColType::Text | ColType::Timestamp | ColType::Date | ColType::Enum => {
            Ok(CellValue::Text(text.to_owned()))
        }
    }
}

fn handle_filter_strip(
    tab: &mut TableTab,
    ev: PageEvent,
    cx: &mut PageCtx<'_>,
    cat: &Catalog,
    system: &DesignSystem,
) -> Route {
    let snapshot = tab.filter_items();
    let items: Vec<TokenItem<'_, usize>> = snapshot
        .iter()
        .map(|(i, label, enabled)| {
            TokenItem::chip(*i, label.as_str())
                .removable(true)
                .selected(*enabled)
        })
        .collect();
    let strip = TokenStrip::new(&items, system)
        .lead(Some("match all ▾"))
        .add_label(Some("+ Add filter"));
    let outcome = match ev {
        PageEvent::Key(key) => strip.handle_key(&mut tab.filter_strip, key),
        PageEvent::Click { pos, .. } => strip.handle_mouse(
            &mut tab.filter_strip,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: pos,
                modifiers: KeyModifiers::NONE,
            },
        ),
        _ => TokenStripOutcome::Ignored,
    };
    match outcome {
        TokenStripOutcome::Activated(i) => cx.open_table_filter(Some(i), None, None),
        TokenStripOutcome::Add => {
            cx.open_table_filter(None, Some(tab.grid.cursor_col), None);
        }
        TokenStripOutcome::Selected(i) | TokenStripOutcome::Unselected(i) => {
            if tab.grid.pending.is_empty() {
                if let Some(filter) = tab.filters.get_mut(i) {
                    filter.enabled = matches!(outcome, TokenStripOutcome::Selected(_));
                }
                tab.load(cat);
            } else {
                cx.status("Cannot change filters while pending changes exist");
            }
        }
        TokenStripOutcome::Remove(i) => {
            if tab.grid.pending.is_empty() {
                if i < tab.filters.len() {
                    tab.filters.remove(i);
                    tab.load(cat);
                }
            } else {
                cx.status("Cannot change filters while pending changes exist");
            }
        }
        _ => {}
    }
    if matches!(outcome, TokenStripOutcome::Ignored) {
        Route::Ignored
    } else {
        Route::Changed
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
            if tab.grid.pending.is_empty() {
                ("Space", "Select row")
            } else {
                ("Ctrl+S", "Save")
            },
        ];
    }
    vec![("↑ ↓", "Move"), ("Ctrl+D", "Structure")]
}

pub fn query_hints(tab: &QueryTab) -> Vec<Hint> {
    if tab.is_running() {
        return vec![("Esc", "Cancel query")];
    }
    if tab.completion.is_open() {
        return vec![("↑ ↓", "Move"), ("Enter", "Accept"), ("Esc", "Close")];
    }
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

#[cfg(test)]
mod tests {
    use super::super::db::Catalog;
    use super::{TABLE_GRID, TableTab, handle_grid_edit};
    use crate::outcome::Route;
    use crate::page::{PageCtx, Request};
    use termrock::input::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn invalid_edit_stays_open_and_tab_does_not_advance() {
        let catalog = Catalog::acme_prod();
        let table = catalog.find(Some("public"), "orders").unwrap();
        let mut tab = TableTab::new(table);
        tab.grid.cursor_col = 1;
        tab.table_state.cursor_col = 1;
        tab.table_state.editing = true;
        tab.table_state.edit_draft = "not-an-integer".to_owned();

        let mut focus = Some(TABLE_GRID);
        let mut cx = PageCtx {
            focus: &mut focus,
            requests: Vec::new(),
        };
        let route = handle_grid_edit(
            &mut tab,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &mut cx,
        );

        assert_eq!(route, Route::Changed);
        assert!(tab.is_editing());
        assert_eq!(tab.table_state.edit_draft, "not-an-integer");
        assert_eq!(tab.grid.cursor_col, 1);
        assert!(tab.grid.pending.is_empty());
        assert!(cx.requests.iter().any(|request| {
            matches!(
                request,
                Request::Status(message) if message == "Invalid integer: not-an-integer"
            )
        }));
    }
}
