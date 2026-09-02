// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/model.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

//! Application models that carry TablePro's semantics independent of the
//! UI: pending changes, history, completion ranking, open-quickly ranking.

use super::db::{Catalog, ColType, Table, Value};
use super::grid::{CellValue, ResultGrid, from_cell};
use super::sql::{FUNCTIONS, KEYWORDS, TokKind, tokenize};
use super::text::fuzzy;

// ------------------------------------------------------------ pending edits

/// Statements TablePro would run on save (parameters inlined), built from
/// a grid's pending changes: updates by row, then inserts, then deletes.
#[must_use]
pub fn preview_sql(table: &Table, columns: &[(String, ColType)], grid: &ResultGrid) -> Vec<String> {
    let pk: Vec<&str> = table
        .primary_key()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    let pk_where = |src: usize| -> String {
        if pk.is_empty() {
            return columns
                .iter()
                .enumerate()
                .map(|(ci, c)| {
                    let v = grid
                        .rows
                        .get(src)
                        .and_then(|r| r.get(ci))
                        .cloned()
                        .unwrap_or(CellValue::Null);
                    match v {
                        CellValue::Null => format!("{} IS NULL", c.0),
                        other => format!("{} = {}", c.0, sql_literal(&from_cell(&other))),
                    }
                })
                .collect::<Vec<_>>()
                .join(" AND ");
        }
        pk.iter()
            .map(|k| {
                let ci = columns.iter().position(|c| c.0 == *k).unwrap_or(0);
                let v = grid
                    .rows
                    .get(src)
                    .and_then(|r| r.get(ci))
                    .cloned()
                    .unwrap_or(CellValue::Null);
                format!("{k} = {}", sql_literal(&from_cell(&v)))
            })
            .collect::<Vec<_>>()
            .join(" AND ")
    };
    let pending = &grid.pending;
    let mut out = Vec::new();
    for r in pending.dirty_rows() {
        let mut sets: Vec<(usize, String)> = pending
            .cells
            .iter()
            .filter(|((row, _), _)| *row == r)
            .map(|((_, col), v)| {
                (
                    *col,
                    format!("{} = {}", columns[*col].0, sql_literal(&from_cell(v))),
                )
            })
            .collect();
        sets.sort_by_key(|(c, _)| *c);
        let sets: Vec<String> = sets.into_iter().map(|(_, s)| s).collect();
        out.push(format!(
            "UPDATE {} SET {} WHERE {};",
            table.qualified(),
            sets.join(", "),
            pk_where(r)
        ));
    }
    for &r in &pending.inserted {
        let mut names = vec![];
        let mut vals = vec![];
        for (ci, c) in columns.iter().enumerate() {
            let v = pending.value(r, ci).cloned().unwrap_or(CellValue::Default);
            if matches!(v, CellValue::Default) {
                continue;
            }
            names.push(c.0.clone());
            vals.push(sql_literal(&from_cell(&v)));
        }
        if names.is_empty() {
            out.push(format!("INSERT INTO {} DEFAULT VALUES;", table.qualified()));
        } else {
            out.push(format!(
                "INSERT INTO {} ({}) VALUES ({});",
                table.qualified(),
                names.join(", "),
                vals.join(", ")
            ));
        }
    }
    for &r in &pending.deleted {
        out.push(format!(
            "DELETE FROM {} WHERE {};",
            table.qualified(),
            pk_where(r)
        ));
    }
    out
}

#[must_use]
pub fn sql_literal(v: &Value) -> String {
    match v {
        Value::Null => "NULL".into(),
        Value::Int(i) => i.to_string(),
        Value::Num(n) => format!("{n:.2}"),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Json(j) => format!("'{}'::jsonb", j.replace('\'', "''")),
    }
}

// ------------------------------------------------------------ history

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistorySource {
    Editor,
    Explain,
    Browsing,
    RowEdits,
    Structure,
}

impl HistorySource {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            HistorySource::Editor => "Editor",
            HistorySource::Explain => "Explain",
            HistorySource::Browsing => "Table Browsing",
            HistorySource::RowEdits => "Row Edits",
            HistorySource::Structure => "Structure Changes",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: usize,
    pub sql: String,
    pub connection: String,
    pub database: String,
    pub schema: String,
    /// Minutes ago (deterministic demo clock).
    pub minutes_ago: u32,
    pub duration_ms: Option<u32>,
    pub rows: Option<usize>,
    pub error: Option<String>,
    pub source: HistorySource,
}

impl HistoryEntry {
    #[must_use]
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
    #[must_use]
    pub fn first_line(&self) -> String {
        let l = self.sql.lines().next().unwrap_or("").trim().to_owned();
        if self.sql.lines().count() > 1 {
            format!("{l} ⋯")
        } else {
            l
        }
    }
    #[must_use]
    pub fn when(&self) -> String {
        match self.minutes_ago {
            0 => "just now".into(),
            m if m < 60 => format!("{m} min ago"),
            m if m < 24 * 60 => format!("{} h ago", m / 60),
            m => format!("{} d ago", m / (24 * 60)),
        }
    }
    #[must_use]
    pub fn duration(&self) -> String {
        match self.duration_ms {
            None => "–".into(),
            Some(0) => "<1 ms".into(),
            Some(ms) if ms < 1000 => format!("{ms} ms"),
            Some(ms) if ms < 60_000 => format!("{:.2} s", ms as f64 / 1000.0),
            Some(ms) => format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
    next_id: usize,
}

impl History {
    #[must_use]
    pub fn seeded() -> Self {
        let mut h = Self::default();
        type Seed = (
            &'static str,
            &'static str,
            &'static str,
            u32,
            Option<u32>,
            Option<usize>,
            Option<&'static str>,
            HistorySource,
        );
        let seed: Vec<Seed> = vec![
            (
                "SELECT * FROM orders WHERE status = 'pending' ORDER BY created_at DESC LIMIT 200",
                "Production",
                "acme_prod",
                4,
                Some(38),
                Some(200),
                None,
                HistorySource::Editor,
            ),
            (
                "SELECT count(*) FROM orders WHERE created_at >= '2025-06-01'",
                "Production",
                "acme_prod",
                12,
                Some(412),
                Some(1),
                None,
                HistorySource::Editor,
            ),
            (
                "EXPLAIN ANALYZE SELECT * FROM orders WHERE customer_id = '3f1a…' ",
                "Production",
                "acme_prod",
                15,
                Some(9),
                Some(4),
                None,
                HistorySource::Explain,
            ),
            (
                "SELECT * FROM customers ORDER BY created_at DESC",
                "Production",
                "acme_prod",
                40,
                Some(21),
                Some(1000),
                None,
                HistorySource::Browsing,
            ),
            (
                "UPDATE orders SET status = 'shipped' WHERE id = '9c2e…'",
                "Production",
                "acme_prod",
                58,
                Some(3),
                Some(1),
                None,
                HistorySource::RowEdits,
            ),
            (
                "SELECT o.id, o.total_amount, c.email\nFROM orders o\nJOIN customers c ON c.id = o.customer_id\nWHERE o.total_amount > 500\n  AND o.status IN ('paid', 'shipped')\nORDER BY o.total_amount DESC\nLIMIT 50",
                "Production",
                "acme_prod",
                95,
                Some(61),
                Some(50),
                None,
                HistorySource::Editor,
            ),
            (
                "SELECT * FROM ordres",
                "Production",
                "acme_prod",
                96,
                None,
                None,
                Some("relation \"ordres\" does not exist"),
                HistorySource::Editor,
            ),
            (
                "SELECT day, revenue, refunds FROM analytics.daily_revenue WHERE day >= '2025-01-01' ORDER BY day",
                "Production",
                "acme_prod",
                3 * 60,
                Some(14),
                Some(244),
                None,
                HistorySource::Editor,
            ),
            (
                "DELETE FROM audit.login_attempts WHERE attempted_at < '2024-01-01'",
                "Development",
                "acme_dev",
                5 * 60,
                Some(1_820),
                Some(41_002),
                None,
                HistorySource::Editor,
            ),
            (
                "ALTER TABLE orders ADD COLUMN is_gift boolean NOT NULL DEFAULT false",
                "Development",
                "acme_dev",
                26 * 60,
                Some(88),
                Some(0),
                None,
                HistorySource::Structure,
            ),
            (
                "SELECT email, full_name FROM customers WHERE email LIKE '%@northwind.io'",
                "Development",
                "acme_dev",
                27 * 60,
                Some(7),
                Some(412),
                None,
                HistorySource::Editor,
            ),
            (
                "SELECT * FROM payments WHERE status = 'failed' AND created_at > now() - interval '7 days'",
                "Production",
                "acme_prod",
                30 * 60,
                Some(24),
                Some(318),
                None,
                HistorySource::Editor,
            ),
            (
                "SELECT plan, count(*) FROM subscriptions GROUP BY plan",
                "Production",
                "acme_prod",
                3 * 24 * 60,
                Some(5),
                Some(3),
                None,
                HistorySource::Editor,
            ),
            (
                "TRUNCATE analytics.events",
                "Development",
                "acme_dev",
                4 * 24 * 60,
                Some(210),
                Some(0),
                None,
                HistorySource::Editor,
            ),
            (
                "SELECT * FROM events WHERE event_type = 'checkout' LIMIT 100",
                "Development",
                "acme_dev",
                5 * 24 * 60,
                None,
                None,
                Some("canceling statement due to user request"),
                HistorySource::Editor,
            ),
        ];
        for (sql, conn, dbn, min, dur, rows, err, src) in seed.into_iter().rev() {
            h.push(HistoryEntry {
                id: 0,
                sql: sql.into(),
                connection: conn.into(),
                database: dbn.into(),
                schema: "public".into(),
                minutes_ago: min,
                duration_ms: dur,
                rows,
                error: err.map(str::to_owned),
                source: src,
            });
        }
        h.entries.reverse();
        h
    }

    pub fn push(&mut self, mut e: HistoryEntry) {
        self.next_id += 1;
        e.id = self.next_id;
        self.entries.insert(0, e);
        self.entries.truncate(10_000);
    }

    /// Multi-term AND, partial-word, case-insensitive; optional filters.
    #[must_use]
    pub fn search<'a>(
        &'a self,
        query: &str,
        connection: Option<&str>,
        failed_only: bool,
    ) -> Vec<&'a HistoryEntry> {
        let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
        self.entries
            .iter()
            .filter(|e| connection.is_none_or(|c| e.connection == c))
            .filter(|e| !failed_only || !e.ok())
            .filter(|e| {
                let hay = e.sql.to_lowercase();
                terms.iter().all(|t| hay.contains(t))
            })
            .collect()
    }
}

// ------------------------------------------------------------ completion

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Keyword,
    Table,
    View,
    Column,
    Function,
    Schema,
    Alias,
}

impl CompletionKind {
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            CompletionKind::Keyword => "K",
            CompletionKind::Table => "T",
            CompletionKind::View => "V",
            CompletionKind::Column => "C",
            CompletionKind::Function => "F",
            CompletionKind::Schema => "S",
            CompletionKind::Alias => "A",
        }
    }
    fn priority(self) -> u32 {
        match self {
            CompletionKind::Column => 100,
            CompletionKind::Alias => 150,
            CompletionKind::Table => 200,
            CompletionKind::View => 210,
            CompletionKind::Function => 300,
            CompletionKind::Keyword => 400,
            CompletionKind::Schema => 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub kind: CompletionKind,
    pub label: String,
    pub detail: String,
    pub insert: String,
    pub score: u32,
    pub matched: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clause {
    Start,
    SelectList,
    From,
    Where,
    OrderBy,
    Member,
}

/// The word being typed at `cursor` and the clause context before it.
#[must_use]
pub fn context(src: &str, cursor: usize) -> (String, Clause, Option<String>) {
    let cursor = cursor.min(src.len());
    let before = &src[..cursor];
    let word_start = before
        .char_indices()
        .rev()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let word = before[word_start..].to_owned();
    let mut qualifier = None;
    if before[..word_start].ends_with('.') {
        let q_end = word_start - 1;
        let q_start = before[..q_end]
            .char_indices()
            .rev()
            .find(|(_, c)| !(c.is_alphanumeric() || *c == '_'))
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        qualifier = Some(before[q_start..q_end].to_owned());
    }
    if qualifier.is_some() {
        return (word, Clause::Member, qualifier);
    }
    let stmt_start = super::sql::statement_at(src, cursor)
        .map(|(a, _)| a)
        .unwrap_or(0);
    let mut clause = Clause::Start;
    for t in tokenize(&src[stmt_start..word_start]) {
        if t.kind == TokKind::Keyword {
            let kw = src[stmt_start + t.start..stmt_start + t.end].to_ascii_uppercase();
            clause = match kw.as_str() {
                "SELECT" => Clause::SelectList,
                "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE" => Clause::From,
                "WHERE" | "AND" | "OR" | "ON" | "HAVING" | "SET" => Clause::Where,
                "BY" => Clause::OrderBy,
                _ => clause,
            };
        }
    }
    (word, clause, None)
}

/// Tables referenced in the statement (with aliases), FROM-anywhere.
#[must_use]
pub fn tables_in_statement<'a>(cat: &'a Catalog, stmt: &str) -> Vec<(&'a Table, Option<String>)> {
    let toks: Vec<(TokKind, String)> = tokenize(stmt)
        .into_iter()
        .filter(|t| !matches!(t.kind, TokKind::Whitespace | TokKind::Comment))
        .map(|t| (t.kind, stmt[t.start..t.end].to_owned()))
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let up = toks[i].1.to_ascii_uppercase();
        if toks[i].0 == TokKind::Keyword
            && matches!(up.as_str(), "FROM" | "JOIN" | "INTO" | "UPDATE")
            && let Some((k, name)) = toks.get(i + 1)
            && *k == TokKind::Ident
        {
            let (schema, tname) = if toks.get(i + 2).map(|t| t.1.as_str()) == Some(".") {
                (
                    Some(name.as_str()),
                    toks.get(i + 3).map(|t| t.1.clone()).unwrap_or_default(),
                )
            } else {
                (None, name.clone())
            };
            let after = if schema.is_some() { i + 4 } else { i + 2 };
            let mut alias = None;
            if let Some((ak, a)) = toks.get(after) {
                if a.eq_ignore_ascii_case("AS") {
                    alias = toks.get(after + 1).map(|t| t.1.clone());
                } else if *ak == TokKind::Ident {
                    alias = Some(a.clone());
                }
            }
            if let Some(t) = cat.find(schema, &tname) {
                out.push((t, alias));
            }
        }
        i += 1;
    }
    out
}

/// Suggestions for the cursor position, ranked.
#[must_use]
pub fn complete(cat: &Catalog, src: &str, cursor: usize) -> (Vec<Completion>, usize) {
    let (word, clause, qualifier) = context(src, cursor);
    let stmt = super::sql::statement_at(src, cursor)
        .map(|(a, b)| &src[a..b])
        .unwrap_or("");
    let in_stmt = tables_in_statement(cat, stmt);
    let mut pool: Vec<Completion> = Vec::new();
    let mut push =
        |kind: CompletionKind, label: &str, detail: String, insert: Option<String>, boost: i32| {
            if let Some((penalty, matched)) = fuzzy(label, &word) {
                let score = (kind.priority() as i32 + penalty as i32 + boost).max(0) as u32;
                pool.push(Completion {
                    kind,
                    label: label.to_owned(),
                    detail,
                    insert: insert.unwrap_or_else(|| label.to_owned()),
                    score,
                    matched,
                });
            }
        };
    match clause {
        Clause::Member => {
            let q = qualifier.unwrap_or_default();
            let table = in_stmt
                .iter()
                .find(|(t, a)| {
                    a.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(&q))
                        || t.name.eq_ignore_ascii_case(&q)
                })
                .map(|(t, _)| *t)
                .or_else(|| cat.find(None, &q));
            if let Some(t) = table {
                for c in &t.columns {
                    push(CompletionKind::Column, &c.name, col_detail(c), None, 0);
                }
            } else if cat.schemas.iter().any(|s| s.eq_ignore_ascii_case(&q)) {
                for t in cat
                    .tables
                    .iter()
                    .filter(|t| t.schema.eq_ignore_ascii_case(&q))
                {
                    push(
                        kind_of(t),
                        &t.name,
                        format!("{} · {}", t.schema, super::sql::fmt_rows(t.row_count)),
                        None,
                        0,
                    );
                }
            }
        }
        Clause::From => {
            for t in &cat.tables {
                if matches!(
                    t.kind,
                    super::db::ObjectKind::Table | super::db::ObjectKind::View
                ) {
                    let boost = if t.schema == "public" { -50 } else { 0 };
                    let insert = if t.schema == "public" {
                        None
                    } else {
                        Some(t.qualified())
                    };
                    push(
                        kind_of(t),
                        &t.name,
                        format!("{} · {} rows", t.schema, super::sql::fmt_rows(t.row_count)),
                        insert,
                        boost,
                    );
                }
            }
            for s in &cat.schemas {
                push(CompletionKind::Schema, s, "schema".into(), None, 0);
            }
            for k in [
                "WHERE",
                "ORDER BY",
                "LIMIT",
                "JOIN",
                "LEFT JOIN",
                "GROUP BY",
            ] {
                push(CompletionKind::Keyword, k, String::new(), None, 0);
            }
        }
        Clause::SelectList | Clause::Where | Clause::OrderBy => {
            let sources: Vec<&Table> = if in_stmt.is_empty() {
                cat.tables
                    .iter()
                    .filter(|t| !t.columns.is_empty())
                    .collect()
            } else {
                in_stmt.iter().map(|(t, _)| *t).collect()
            };
            let ambiguous = sources.len() > 1;
            for t in &sources {
                for c in &t.columns {
                    let label = if ambiguous && in_stmt.is_empty() {
                        format!("{}.{}", t.name, c.name)
                    } else {
                        c.name.clone()
                    };
                    let detail = if ambiguous {
                        format!("{} · {}", t.name, col_detail(c))
                    } else {
                        col_detail(c)
                    };
                    push(
                        CompletionKind::Column,
                        &label,
                        detail,
                        None,
                        if in_stmt.is_empty() { 40 } else { 0 },
                    );
                }
            }
            for (t, a) in &in_stmt {
                if let Some(a) = a {
                    push(
                        CompletionKind::Alias,
                        a,
                        format!("alias of {}", t.name),
                        None,
                        0,
                    );
                }
            }
            for f in FUNCTIONS {
                push(
                    CompletionKind::Function,
                    f,
                    "function".into(),
                    Some(format!("{f}(")),
                    0,
                );
            }
            let kws: &[&str] = match clause {
                Clause::SelectList => &[
                    "FROM", "DISTINCT", "AS", "CASE", "COUNT", "SUM", "AVG", "MAX", "MIN", "*",
                ],
                Clause::Where => &[
                    "AND",
                    "OR",
                    "NOT",
                    "IS NULL",
                    "IS NOT NULL",
                    "IN",
                    "LIKE",
                    "ILIKE",
                    "BETWEEN",
                    "ORDER BY",
                    "LIMIT",
                    "TRUE",
                    "FALSE",
                    "NULL",
                ],
                _ => &["ASC", "DESC", "LIMIT", "OFFSET"],
            };
            for k in kws {
                push(CompletionKind::Keyword, k, String::new(), None, 0);
            }
        }
        Clause::Start => {
            for k in [
                "SELECT",
                "SELECT * FROM",
                "INSERT INTO",
                "UPDATE",
                "DELETE FROM",
                "EXPLAIN",
                "EXPLAIN ANALYZE",
                "WITH",
                "CREATE TABLE",
                "ALTER TABLE",
                "DROP TABLE",
                "TRUNCATE",
                "BEGIN",
                "COMMIT",
                "ROLLBACK",
            ] {
                push(CompletionKind::Keyword, k, String::new(), None, 0);
            }
            for k in KEYWORDS.iter().filter(|_| !word.is_empty()) {
                push(CompletionKind::Keyword, k, String::new(), None, 20);
            }
        }
    }
    pool.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| a.label.len().cmp(&b.label.len()))
            .then_with(|| a.label.cmp(&b.label))
    });
    pool.dedup_by(|a, b| a.label == b.label && a.kind == b.kind);
    pool.truncate(60);
    (pool, word.len())
}

fn kind_of(t: &Table) -> CompletionKind {
    if t.kind == super::db::ObjectKind::View {
        CompletionKind::View
    } else {
        CompletionKind::Table
    }
}

fn col_detail(c: &super::db::Column) -> String {
    let mut d = c.ty.sql().to_owned();
    if c.primary {
        d.push_str(" · pk");
    }
    if c.references.is_some() {
        d.push_str(" · fk");
    }
    if c.nullable {
        d.push_str(" · null");
    }
    d
}

/// Should the popup open automatically at this position?
#[must_use]
pub fn auto_trigger(src: &str, cursor: usize) -> bool {
    let (word, clause, _) = context(src, cursor);
    match clause {
        Clause::Member => true,
        Clause::From => true,
        Clause::Where | Clause::OrderBy | Clause::SelectList => word.len() >= 2,
        _ => word.len() >= 3,
    }
}

// ------------------------------------------------------------ open quickly

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchTarget {
    Table { schema: String, name: String },
    View { schema: String, name: String },
    Schema(String),
    Database(String),
    OpenTab(usize),
    RecentQuery(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchItem {
    pub target: SwitchTarget,
    pub label: String,
    pub path: String,
    pub group: &'static str,
    pub open: bool,
    pub score: u32,
    pub matched: Vec<usize>,
}

pub struct SwitcherIndex {
    pub items: Vec<SwitchItem>,
}

impl SwitcherIndex {
    #[must_use]
    pub fn build(
        cat: &Catalog,
        connection: &str,
        open_tabs: &[(usize, String)],
        history: &History,
    ) -> Self {
        let mut items = Vec::new();
        for t in &cat.tables {
            let (target, group) = match t.kind {
                super::db::ObjectKind::Table => (
                    SwitchTarget::Table {
                        schema: t.schema.clone(),
                        name: t.name.clone(),
                    },
                    "Tables",
                ),
                super::db::ObjectKind::View => (
                    SwitchTarget::View {
                        schema: t.schema.clone(),
                        name: t.name.clone(),
                    },
                    "Views",
                ),
                _ => continue,
            };
            let open = open_tabs
                .iter()
                .any(|(_, l)| l == &t.name || l == &t.qualified());
            items.push(SwitchItem {
                target,
                label: t.name.clone(),
                path: format!("{} · {connection}", t.schema),
                group,
                open,
                score: 0,
                matched: vec![],
            });
        }
        for s in &cat.schemas {
            items.push(SwitchItem {
                target: SwitchTarget::Schema(s.clone()),
                label: s.clone(),
                path: format!("{} · {connection}", cat.database),
                group: "Schemas",
                open: false,
                score: 0,
                matched: vec![],
            });
        }
        items.push(SwitchItem {
            target: SwitchTarget::Database(cat.database.clone()),
            label: cat.database.clone(),
            path: connection.to_owned(),
            group: "Databases",
            open: true,
            score: 0,
            matched: vec![],
        });
        for (i, label) in open_tabs {
            items.push(SwitchItem {
                target: SwitchTarget::OpenTab(*i),
                label: label.clone(),
                path: "open tab".into(),
                group: "Open tabs",
                open: true,
                score: 0,
                matched: vec![],
            });
        }
        for e in history.entries.iter().take(50) {
            items.push(SwitchItem {
                target: SwitchTarget::RecentQuery(e.id),
                label: e.first_line(),
                path: format!("{} · {}", e.connection, e.when()),
                group: "Recent queries",
                open: false,
                score: 0,
                matched: vec![],
            });
        }
        Self { items }
    }

    /// Rank: name match beats path match; tables above other kinds; open
    /// tabs boosted; empty query lists everything grouped.
    #[must_use]
    pub fn query(&self, q: &str) -> Vec<SwitchItem> {
        let q = q.trim();
        let mut out: Vec<SwitchItem> = self
            .items
            .iter()
            .filter_map(|it| {
                let mut it = it.clone();
                if q.is_empty() {
                    it.score = group_rank(it.group) * 10;
                    return Some(it);
                }
                if let Some((pen, m)) = fuzzy(&it.label, q) {
                    it.score = pen + group_rank(it.group) * 5 + if it.open { 0 } else { 3 };
                    it.matched = m;
                    Some(it)
                } else if it.path.to_lowercase().contains(&q.to_lowercase()) {
                    it.score = 120 + group_rank(it.group) * 5;
                    Some(it)
                } else {
                    None
                }
            })
            .collect();
        out.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.label.cmp(&b.label)));
        out.truncate(200);
        out
    }
}

fn group_rank(g: &str) -> u32 {
    match g {
        "Tables" => 0,
        "Views" => 1,
        "Open tabs" => 2,
        "Schemas" => 3,
        "Databases" => 4,
        _ => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::super::db::Catalog;
    use super::super::grid::CellValue;
    use super::*;

    #[test]
    fn preview_sql_orders_updates_inserts_deletes() {
        let cat = Catalog::acme_prod();
        let t = cat.find(None, "orders").unwrap();
        let cols: Vec<(String, ColType)> =
            t.columns.iter().map(|c| (c.name.clone(), c.ty)).collect();
        let raw = super::super::db::rows(t, 0, 3);
        let mut g = ResultGrid::from_values(cols.clone(), raw, 3, true);
        let status = cols.iter().position(|c| c.0 == "status").unwrap();
        g.record_cell(1, status, CellValue::Text("shipped".into()));
        g.toggle_delete(2);
        let sql = preview_sql(t, &cols, &g);
        assert_eq!(sql.len(), 2);
        assert!(
            sql[0].starts_with("UPDATE public.orders SET status = 'shipped' WHERE id = '"),
            "{}",
            sql[0]
        );
        assert!(sql[1].starts_with("DELETE FROM public.orders WHERE id = '"));
        let original = g.rows[1][status].clone();
        g.record_cell(1, status, original);
        assert_eq!(preview_sql(t, &cols, &g).len(), 1);
    }

    #[test]
    fn history_search_is_multi_term_and() {
        let h = History::seeded();
        let r = h.search("orders pending", None, false);
        assert_eq!(r.len(), 1);
        let r = h.search("", Some("Development"), false);
        assert!(r.iter().all(|e| e.connection == "Development"));
        let r = h.search("", None, true);
        assert!(!r.is_empty() && r.iter().all(|e| !e.ok()));
    }

    #[test]
    fn completion_is_context_aware() {
        let cat = Catalog::acme_prod();
        let src = "SELECT o. FROM orders o WHERE ";
        let (items, _) = complete(&cat, src, 9);
        assert_eq!(
            items[0].kind,
            CompletionKind::Column,
            "alias member → its columns"
        );
        assert!(items.iter().any(|c| c.label == "total_amount"));
        let src2 = "SELECT * FROM ord";
        let (items, replace) = complete(&cat, src2, src2.len());
        assert_eq!(replace, 3);
        assert_eq!(items[0].label, "orders");
        assert_eq!(items[0].kind, CompletionKind::Table);
        assert_eq!(items[0].matched, vec![0, 1, 2]);
        let src3 = "SELECT * FROM orders WHERE st";
        let (items, _) = complete(&cat, src3, src3.len());
        assert_eq!(items[0].label, "status");
        let src4 = "SELECT * FROM orders WHERE status = 'x' ORDER BY cre";
        let (items, _) = complete(&cat, src4, src4.len());
        assert_eq!(items[0].label, "created_at");
        let src5 = "SELECT * FROM analytics.";
        let (items, _) = complete(&cat, src5, src5.len());
        assert!(items.iter().any(|c| c.label == "events"));
        assert!(auto_trigger("SELECT * FROM ", 14));
        assert!(!auto_trigger("SELECT * FROM orders WHERE s", 28));
    }

    #[test]
    fn switcher_ranks_tables_first_and_prefix_first() {
        let cat = Catalog::acme_prod();
        let h = History::seeded();
        let idx = SwitcherIndex::build(&cat, "Production", &[(0, "orders".into())], &h);
        let r = idx.query("ord");
        assert_eq!(r[0].label, "orders");
        assert!(r[0].open);
        assert!(r.iter().any(|i| i.label == "order_items"));
        assert!(r.iter().any(|i| i.group == "Recent queries"));
        let all = idx.query("");
        assert!(all.len() > 15);
        assert_eq!(all[0].group, "Tables");
    }
}
