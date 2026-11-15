use crate::db::{Catalog, ColType, Table, Value};
use crate::sql::{FUNCTIONS, KEYWORDS, TokKind, tokenize};
use junie_tui::core::id::ItemKey;
use junie_tui::ui::text::fuzzy;
use junie_tui::widgets::grid::{CellValue, DataGrid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlLiteralError {
    NonFiniteNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRow {
    Source(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSqlCause {
    Literal(SqlLiteralError),
    MissingColumn(String),
    InvalidDefaultPredicate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSqlError {
    pub row: PreviewRow,
    pub column: Option<String>,
    pub cause: PreviewSqlCause,
}

impl PreviewSqlError {
    fn at(row: usize, column: &str, cause: PreviewSqlCause) -> Self {
        Self {
            row: PreviewRow::Source(row),
            column: Some(column.to_owned()),
            cause,
        }
    }
}

fn cell_literal(row: usize, column: &str, value: &CellValue) -> Result<String, PreviewSqlError> {
    sql_literal(&crate::tabs::from_cell(value))
        .map_err(|e| PreviewSqlError::at(row, column, PreviewSqlCause::Literal(e)))
}

/// Matches the row as it was loaded: base values only, so an edited key still
/// finds its original record.
fn row_predicate(
    key_columns: &[&str],
    columns: &[(String, ColType)],
    grid: &DataGrid,
    src: usize,
) -> Result<String, PreviewSqlError> {
    let keyed: Vec<(usize, &str)> = if key_columns.is_empty() {
        columns
            .iter()
            .enumerate()
            .map(|(ci, c)| (ci, c.0.as_str()))
            .collect()
    } else {
        key_columns
            .iter()
            .map(|k| {
                columns
                    .iter()
                    .position(|c| c.0 == *k)
                    .map(|ci| (ci, *k))
                    .ok_or_else(|| {
                        PreviewSqlError::at(src, k, PreviewSqlCause::MissingColumn((*k).to_owned()))
                    })
            })
            .collect::<Result<_, _>>()?
    };
    let terms: Vec<String> = keyed
        .into_iter()
        .map(|(ci, name)| {
            let base = grid
                .rows()
                .get(src)
                .and_then(|r| r.get(ci))
                .cloned()
                .unwrap_or(CellValue::Null);
            match base {
                CellValue::Null => Ok(format!("{name} IS NULL")),
                CellValue::Default => Err(PreviewSqlError::at(
                    src,
                    name,
                    PreviewSqlCause::InvalidDefaultPredicate,
                )),
                other => Ok(format!("{name} = {}", cell_literal(src, name, &other)?)),
            }
        })
        .collect::<Result<_, _>>()?;
    Ok(terms.join(" AND "))
}

fn in_source_order(grid: &DataGrid, keys: impl Iterator<Item = ItemKey>) -> Vec<(usize, ItemKey)> {
    let mut rows: Vec<(usize, ItemKey)> = keys
        .filter_map(|key| Some((grid.source_index(key)?, key)))
        .collect();
    rows.sort_unstable();
    rows
}

/// Complete statement list for the pending changes, or the first
/// serialization error: a partial list is never returned.
pub fn preview_sql(
    table: &Table,
    columns: &[(String, ColType)],
    grid: &DataGrid,
) -> Result<Vec<String>, PreviewSqlError> {
    let pk: Vec<&str> = table
        .primary_key()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    let pending = grid.pending();
    let mut out = Vec::new();
    for (r, key) in in_source_order(grid, pending.dirty_rows().into_iter()) {
        let mut cells: Vec<(usize, &CellValue)> = pending
            .cells()
            .filter(|(row, _, _)| *row == key)
            .map(|(_, col, v)| (col, v))
            .collect();
        cells.sort_by_key(|(c, _)| *c);
        let sets: Vec<String> = cells
            .into_iter()
            .map(|(c, v)| {
                let name = &columns[c].0;
                Ok(format!("{name} = {}", cell_literal(r, name, v)?))
            })
            .collect::<Result<_, PreviewSqlError>>()?;
        out.push(format!(
            "UPDATE {} SET {} WHERE {};",
            table.qualified(),
            sets.join(", "),
            row_predicate(&pk, columns, grid, r)?
        ));
    }
    for (r, key) in in_source_order(grid, pending.inserted()) {
        let mut names = vec![];
        let mut vals = vec![];
        for (ci, c) in columns.iter().enumerate() {
            let v = pending
                .value(key, ci)
                .cloned()
                .unwrap_or(CellValue::Default);
            if matches!(v, CellValue::Default) {
                continue;
            }
            names.push(c.0.clone());
            vals.push(cell_literal(r, &c.0, &v)?);
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
    for (r, _) in in_source_order(grid, pending.deleted()) {
        out.push(format!(
            "DELETE FROM {} WHERE {};",
            table.qualified(),
            row_predicate(&pk, columns, grid, r)?
        ));
    }
    Ok(out)
}

/// The one text-literal policy for SQL the app prints: single quotes, with
/// embedded apostrophes doubled.
pub fn quote_text(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Mutation serialization, never display formatting. A finite number prints
/// the shortest text that parses back to the stored `f64` bits; digits the
/// model discarded before storing the value cannot be recovered.
pub fn sql_literal(v: &Value) -> Result<String, SqlLiteralError> {
    Ok(match v {
        Value::Null => "NULL".to_owned(),
        Value::Int(i) => i.to_string(),
        Value::Num(n) => {
            if !n.is_finite() {
                return Err(SqlLiteralError::NonFiniteNumber);
            }
            let mut text = n.to_string();
            if !text.contains(['.', 'e', 'E']) {
                text.push_str(".0");
            }
            text
        }
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => quote_text(s),
        Value::Json(j) => format!("{}::jsonb", quote_text(j)),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistorySource {
    Editor,
    Explain,
    Browsing,
    RowEdits,
    Structure,
}

impl HistorySource {
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

    pub minutes_ago: u32,
    pub duration_ms: Option<u32>,
    pub rows: Option<usize>,
    pub error: Option<String>,
    pub source: HistorySource,
}

impl HistoryEntry {
    pub fn key(&self) -> ItemKey {
        ItemKey::num(u64::try_from(self.id).expect("history ids fit in u64"))
    }
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }
    pub fn first_line(&self) -> String {
        let l = self.sql.lines().next().unwrap_or("").trim().to_owned();
        if self.sql.lines().count() > 1 {
            format!("{l} ⋯")
        } else {
            l
        }
    }
    pub fn when(&self) -> String {
        match self.minutes_ago {
            0 => "just now".into(),
            m if m < 60 => format!("{m} min ago"),
            m if m < 24 * 60 => format!("{} h ago", m / 60),
            m => format!("{} d ago", m / (24 * 60)),
        }
    }
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

    pub fn get(&self, key: ItemKey) -> Option<&HistoryEntry> {
        self.entries.iter().find(|e| e.key() == key)
    }

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

/// The byte cursor a completion request uses: clamped to the text, then
/// floored to a char boundary so every prefix slice is valid.
fn completion_cursor(src: &str, requested: usize) -> usize {
    let mut at = requested.min(src.len());
    while !src.is_char_boundary(at) {
        at -= 1;
    }
    at
}

pub fn context(src: &str, requested: usize) -> (String, Clause, Option<String>) {
    let cursor = completion_cursor(src, requested);
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

    // The clause comes only from the current statement: the tokens after the
    // last real `;` before the word. A `;` inside a string or comment is not a
    // delimiter, and nothing after the cursor takes part.
    let prefix = &src[..word_start];
    let tokens = tokenize(prefix);
    let first = tokens
        .iter()
        .rposition(|t| t.kind == TokKind::Punct && &prefix[t.start..t.end] == ";")
        .map_or(0, |i| i + 1);
    let mut clause = Clause::Start;
    for t in &tokens[first..] {
        if t.kind == TokKind::Keyword {
            let kw = prefix[t.start..t.end].to_ascii_uppercase();
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

/// The untrimmed text between the last real `;` that ends at or before
/// `cursor` and the next real `;` that starts at or after it. Whitespace the
/// user is typing into belongs to the statement, so a cursor after `WHERE `
/// still sees that statement's tables. A cursor right after a `;` starts the
/// next segment and never sees the previous statement.
fn statement_around(src: &str, cursor: usize) -> &str {
    let (mut start, mut end) = (0, src.len());
    for t in tokenize(src) {
        if t.kind != TokKind::Punct || &src[t.start..t.end] != ";" {
            continue;
        }
        if t.end <= cursor {
            start = t.end;
        } else {
            end = t.start;
            break;
        }
    }
    &src[start..end]
}

pub fn complete(cat: &Catalog, src: &str, requested: usize) -> (Vec<Completion>, usize) {
    let cursor = completion_cursor(src, requested);
    let (word, clause, qualifier) = context(src, cursor);
    let in_stmt = tables_in_statement(cat, statement_around(src, cursor));
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
                        format!("{} · {}", t.schema, crate::sql::fmt_rows(t.row_count)),
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
                    crate::db::ObjectKind::Table | crate::db::ObjectKind::View
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
                        format!("{} · {} rows", t.schema, crate::sql::fmt_rows(t.row_count)),
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
    if t.kind == crate::db::ObjectKind::View {
        CompletionKind::View
    } else {
        CompletionKind::Table
    }
}

fn col_detail(c: &crate::db::Column) -> String {
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

pub fn auto_trigger(src: &str, cursor: usize) -> bool {
    let (word, clause, _) = context(src, cursor);
    match clause {
        Clause::Member => true,
        Clause::From => true,
        Clause::Where | Clause::OrderBy | Clause::SelectList => word.len() >= 2,
        _ => word.len() >= 3,
    }
}

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
    pub fn build(
        cat: &Catalog,
        connection: &str,
        open_tabs: &[(usize, String)],
        history: &History,
    ) -> Self {
        let mut items = Vec::new();
        for t in &cat.tables {
            let (target, group) = match t.kind {
                crate::db::ObjectKind::Table => (
                    SwitchTarget::Table {
                        schema: t.schema.clone(),
                        name: t.name.clone(),
                    },
                    "Tables",
                ),
                crate::db::ObjectKind::View => (
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
mod completion_prefix_tests {
    use super::*;

    fn cat() -> Catalog {
        Catalog::acme_prod()
    }

    #[test]
    fn completion_prefix_before_first_statement() {
        let cat = cat();
        for src in [" SELECT id FROM users", "\n\tSELECT id FROM users"] {
            let lead = src.len() - src.trim_start().len();
            for cursor in 0..=lead {
                let (word, clause, qualifier) = context(src, cursor);
                assert_eq!(
                    (word.as_str(), clause, qualifier),
                    ("", Clause::Start, None)
                );
                assert_eq!(complete(&cat, src, cursor).1, 0);
            }
        }
    }

    #[test]
    fn completion_prefix_invalid_byte_cursor() {
        let cat = cat();
        let src = "é中💾 SELECT";
        for cursor in 0..=src.len() + 2 {
            let at = completion_cursor(src, cursor);
            assert!(src.is_char_boundary(at));
            assert_eq!(context(src, cursor), context(src, at));
            let (word, _, _) = context(src, cursor);
            let (_, replace) = complete(&cat, src, cursor);
            assert_eq!(replace, word.len());
            assert!(replace <= at);
        }
    }

    #[test]
    fn completion_prefix_after_semicolon_is_start() {
        let end = |src: &str| context(src, src.len());
        assert_eq!(
            end("SELECT id FROM users WHERE x = 1;\n"),
            (String::new(), Clause::Start, None)
        );
        assert_eq!(
            end("SELECT id FROM users WHERE x = 1;\n;\n"),
            (String::new(), Clause::Start, None)
        );
        assert_eq!(
            end("SELECT id FROM users WHERE x = 1;\n\nSEL"),
            ("SEL".to_owned(), Clause::Start, None)
        );
    }

    #[test]
    fn completion_prefix_ignores_semicolons_in_strings_and_comments() {
        assert_eq!(
            context("SELECT 'a;b' FROM ", 18).1,
            Clause::From,
            "a `;` inside a string is not a delimiter"
        );
        assert_eq!(
            context("-- a;\nSELECT id FROM ", 22).1,
            Clause::From,
            "a `;` inside a comment is not a delimiter"
        );
    }

    #[test]
    fn completion_prefix_gap_gets_no_neighbour_aliases() {
        let cat = cat();
        let src = "SELECT u.id FROM users u;\n\n";
        let alone = "\n\n";
        let labels = |src: &str| {
            complete(&cat, src, src.len())
                .0
                .into_iter()
                .map(|c| (c.kind, c.label))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            labels(src),
            labels(alone),
            "the previous statement's alias `u` must not change the list"
        );
    }

    #[test]
    fn completion_prefix_trailing_whitespace_keeps_the_statement_sources() {
        let cat = cat();
        for src in [
            "SELECT u.id FROM customers u WHERE ",
            "SELECT u.id FROM customers u ORDER BY ",
            "SELECT u.id FROM customers u WHERE \n  ",
        ] {
            let (rows, replace) = complete(&cat, src, src.len());
            assert_eq!(replace, 0, "{src:?}");
            let columns: Vec<&str> = rows
                .iter()
                .filter(|c| c.kind == CompletionKind::Column)
                .map(|c| c.label.as_str())
                .collect();
            assert!(columns.contains(&"email"), "{src:?}: {columns:?}");
            assert!(
                columns.iter().all(|l| !l.contains('.')),
                "{src:?} offered other tables' columns: {columns:?}"
            );
            assert!(
                rows.iter()
                    .any(|c| c.kind == CompletionKind::Alias && c.label == "u"),
                "{src:?} lost the alias `u`"
            );
        }
    }

    #[test]
    fn completion_prefix_cursor_before_next_semicolon_keeps_its_statement() {
        let cat = cat();
        let src = "SELECT 1; SELECT u.id FROM customers u WHERE  ; SELECT 2";
        let cursor = src.find("  ;").unwrap() + 1;
        let (rows, _) = complete(&cat, src, cursor);
        assert!(
            rows.iter()
                .any(|c| c.kind == CompletionKind::Alias && c.label == "u")
        );
    }

    #[test]
    fn completion_prefix_ordinary_clauses_are_unchanged() {
        assert_eq!(context("SELECT id FROM users WHERE ", 28).1, Clause::Where);
        assert_eq!(
            context("SELECT u.", 9),
            ("".to_owned(), Clause::Member, Some("u".to_owned()))
        );
        assert_eq!(
            context("SELECT id FROM users u WHERE u.n", 33),
            ("n".to_owned(), Clause::Member, Some("u".to_owned()))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Column, ObjectKind};
    use junie_tui::core::id::WidgetId;
    use junie_tui::widgets::grid::{CellKind, ColumnSpec, GridRows, RowTotal};

    fn column(name: &str, ty: ColType, primary: bool) -> Column {
        Column {
            name: name.into(),
            ty,
            nullable: true,
            default: None,
            primary,
            references: None,
            enum_values: vec![],
            generated: false,
        }
    }

    fn table(columns: Vec<Column>) -> Table {
        Table {
            schema: "public".into(),
            name: "ledger".into(),
            kind: ObjectKind::Table,
            columns,
            indexes: vec![],
            constraints: vec![],
            triggers: vec![],
            row_count: 0,
            comment: None,
        }
    }

    fn grid_of(table: &Table, rows: Vec<Vec<CellValue>>) -> (Vec<(String, ColType)>, DataGrid) {
        let columns: Vec<(String, ColType)> = table
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.ty))
            .collect();
        let specs = columns
            .iter()
            .map(|(name, _)| ColumnSpec::new(name, CellKind::Number))
            .collect();
        let mut grid = DataGrid::new(WidgetId::of("issue49"), specs);
        grid.set_rows(GridRows {
            total: RowTotal::Exact(rows.len()),
            rows,
            more: false,
        });
        (columns, grid)
    }

    fn keyed() -> Table {
        table(vec![
            column("ref", ColType::Numeric, true),
            column("amount", ColType::Numeric, false),
        ])
    }

    fn unkeyed() -> Table {
        table(vec![
            column("ref", ColType::Numeric, false),
            column("note", ColType::Text, false),
            column("amount", ColType::Numeric, false),
        ])
    }

    #[test]
    fn issue49_numeric_literal_roundtrip() {
        let finite = [
            1.2345,
            0.001,
            1.234_567,
            0.1 + 0.2,
            1.0,
            100.0,
            -0.0,
            0.0,
            f64::MIN_POSITIVE,
            5e-324,
            -5e-324,
            f64::MAX,
            f64::MIN,
            f64::EPSILON,
            1e21,
            123_456_789.123_456_79,
            9_007_199_254_740_993.0,
        ];
        for n in finite {
            let token = sql_literal(&Value::Num(n)).expect("finite numbers serialize");
            let parsed: f64 = token.parse().expect("the token is a numeric literal");
            assert_eq!(parsed.to_bits(), n.to_bits(), "{n:?} became {token}");
            assert!(
                token.contains(['.', 'e', 'E']),
                "{token} must read as a float, not an integer"
            );
        }
        for i in [0, 1, -1, i64::MAX, i64::MIN, 9_007_199_254_740_993] {
            assert_eq!(sql_literal(&Value::Int(i)), Ok(i.to_string()));
        }
        assert_eq!(sql_literal(&Value::Num(-0.0)), Ok("-0.0".to_owned()));
        assert_eq!(sql_literal(&Value::Num(1.0)), Ok("1.0".to_owned()));
        assert_eq!(sql_literal(&Value::Null), Ok("NULL".to_owned()));
        assert_eq!(sql_literal(&Value::Bool(true)), Ok("true".to_owned()));
        assert_eq!(
            sql_literal(&Value::Text("it's".into())),
            Ok("'it''s'".to_owned())
        );
        assert_eq!(
            sql_literal(&Value::Json("{\"a\":\"b'c\"}".into())),
            Ok("'{\"a\":\"b''c\"}'::jsonb".to_owned())
        );
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                sql_literal(&Value::Num(n)),
                Err(SqlLiteralError::NonFiniteNumber)
            );
        }
    }

    #[test]
    fn issue49_update_where_uses_original_precision() {
        let t = keyed();
        let (columns, mut grid) = grid_of(
            &t,
            vec![
                vec![CellValue::Num(1.234_567), CellValue::Num(2.5)],
                vec![CellValue::Null, CellValue::Num(7.0)],
            ],
        );
        grid.record_cell(0, 0, CellValue::Num(9.876_543));
        grid.record_cell(0, 1, CellValue::Num(0.001));
        grid.record_cell(1, 1, CellValue::Num(8.0));
        assert_eq!(
            preview_sql(&t, &columns, &grid).unwrap(),
            vec![
                "UPDATE public.ledger SET ref = 9.876543, amount = 0.001 WHERE ref = 1.234567;"
                    .to_owned(),
                "UPDATE public.ledger SET amount = 8.0 WHERE ref IS NULL;".to_owned(),
            ]
        );
        grid.discard();
        grid.toggle_delete(0);
        grid.toggle_delete(1);
        assert_eq!(
            preview_sql(&t, &columns, &grid).unwrap(),
            vec![
                "DELETE FROM public.ledger WHERE ref = 1.234567;".to_owned(),
                "DELETE FROM public.ledger WHERE ref IS NULL;".to_owned(),
            ]
        );
    }

    #[test]
    fn issue49_no_pk_predicate_and_insert_precision() {
        let t = unkeyed();
        let (columns, mut grid) = grid_of(
            &t,
            vec![vec![
                CellValue::Num(1.2345),
                CellValue::Text("O'Brien".into()),
                CellValue::Null,
            ]],
        );
        grid.record_cell(0, 2, CellValue::Num(0.001));
        grid.insert_row();
        grid.record_cell(1, 0, CellValue::Num(1.234_567_8));
        grid.record_cell(1, 1, CellValue::Text("it's".into()));
        grid.record_cell(1, 2, CellValue::Num(-0.000_1));
        assert_eq!(
            preview_sql(&t, &columns, &grid).unwrap(),
            vec![
                "UPDATE public.ledger SET amount = 0.001 WHERE ref = 1.2345 AND note = 'O''Brien' AND amount IS NULL;"
                    .to_owned(),
                "INSERT INTO public.ledger (ref, note, amount) VALUES (1.2345678, 'it''s', -0.0001);"
                    .to_owned(),
            ]
        );
    }

    #[test]
    fn issue49_nonfinite_aborts_entire_preview() {
        let t = keyed();
        let (columns, mut grid) = grid_of(
            &t,
            vec![
                vec![CellValue::Num(1.5), CellValue::Num(2.5)],
                vec![CellValue::Num(3.5), CellValue::Num(4.5)],
            ],
        );
        grid.record_cell(0, 1, CellValue::Num(6.5));
        grid.record_cell(1, 1, CellValue::Num(f64::NAN));
        let err = preview_sql(&t, &columns, &grid).unwrap_err();
        assert_eq!(
            err,
            PreviewSqlError {
                row: PreviewRow::Source(1),
                column: Some("amount".into()),
                cause: PreviewSqlCause::Literal(SqlLiteralError::NonFiniteNumber),
            }
        );

        grid.undo();
        grid.insert_row();
        grid.record_cell(2, 0, CellValue::Num(f64::INFINITY));
        let err = preview_sql(&t, &columns, &grid).unwrap_err();
        assert_eq!(err.row, PreviewRow::Source(2));
        assert_eq!(err.column.as_deref(), Some("ref"));
    }

    #[test]
    fn issue49_error_context_survives_snapshot_migration() {
        let t = keyed();
        let (columns, mut grid) = grid_of(
            &t,
            vec![
                vec![CellValue::Num(f64::NEG_INFINITY), CellValue::Num(2.5)],
                vec![CellValue::Default, CellValue::Num(2.5)],
            ],
        );
        grid.record_cell(0, 1, CellValue::Num(6.5));
        assert_eq!(
            preview_sql(&t, &columns, &grid).unwrap_err(),
            PreviewSqlError {
                row: PreviewRow::Source(0),
                column: Some("ref".into()),
                cause: PreviewSqlCause::Literal(SqlLiteralError::NonFiniteNumber),
            },
            "an invalid before-image names its own column"
        );

        grid.discard();
        grid.toggle_delete(1);
        assert_eq!(
            preview_sql(&t, &columns, &grid).unwrap_err(),
            PreviewSqlError {
                row: PreviewRow::Source(1),
                column: Some("ref".into()),
                cause: PreviewSqlCause::InvalidDefaultPredicate,
            }
        );

        let orphan = table(vec![
            column("id", ColType::Int, true),
            column("amount", ColType::Numeric, false),
        ]);
        let (_, grid) = grid_of(&orphan, vec![vec![CellValue::Int(1), CellValue::Num(1.0)]]);
        let narrow = vec![("amount".to_owned(), ColType::Numeric)];
        let mut grid = grid;
        grid.toggle_delete(0);
        assert_eq!(
            preview_sql(&orphan, &narrow, &grid).unwrap_err(),
            PreviewSqlError {
                row: PreviewRow::Source(0),
                column: Some("id".into()),
                cause: PreviewSqlCause::MissingColumn("id".into()),
            }
        );
    }
}
