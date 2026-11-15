use crate::db::{Catalog, ColType, Table, Value};
use crate::sql::{FUNCTIONS, KEYWORDS, TokKind, tokenize};
use crate::store::Source;
use junie_tui::core::id::ItemKey;
use junie_tui::ui::text::fuzzy;
use junie_tui::widgets::grid::{CellValue, GridChangeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlLiteralError {
    NonFiniteNumber,
}

/// The snapshot row a serialization error belongs to. A key is stable across
/// sorting and edits, so the error names the same row the reviewer saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewRow {
    Key(ItemKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSqlCause {
    Literal(SqlLiteralError),
    MissingColumn(String),
    InvalidDefaultPredicate,
    /// A draft changes a column the schema generates.
    GeneratedColumn,
    /// A snapshot row is not as wide as the projected columns.
    RowShape {
        expected: usize,
        actual: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSqlError {
    pub row: PreviewRow,
    pub column: Option<String>,
    pub cause: PreviewSqlCause,
}

impl PreviewSqlError {
    fn at(row: ItemKey, column: &str, cause: PreviewSqlCause) -> Self {
        Self {
            row: PreviewRow::Key(row),
            column: Some(column.to_owned()),
            cause,
        }
    }

    fn row(row: ItemKey, cause: PreviewSqlCause) -> Self {
        Self {
            row: PreviewRow::Key(row),
            column: None,
            cause,
        }
    }
}

/// `DEFAULT` and `NULL` are decided here, before the value reaches the typed
/// literal serializer, so the two stay distinct.
fn cell_literal(row: ItemKey, column: &str, value: &CellValue) -> Result<String, PreviewSqlError> {
    match value {
        CellValue::Default => Ok("DEFAULT".to_owned()),
        CellValue::Null => Ok("NULL".to_owned()),
        other => sql_literal(&crate::tabs::from_cell(other))
            .map_err(|e| PreviewSqlError::at(row, column, PreviewSqlCause::Literal(e))),
    }
}

fn check_width(
    row: ItemKey,
    columns: &[(String, ColType)],
    values: &[CellValue],
) -> Result<(), PreviewSqlError> {
    if values.len() == columns.len() {
        return Ok(());
    }
    Err(PreviewSqlError::row(
        row,
        PreviewSqlCause::RowShape {
            expected: columns.len(),
            actual: values.len(),
        },
    ))
}

/// Matches the row as it was loaded: the snapshot's base values, so an edited
/// key still finds its original record.
fn row_predicate(
    key_columns: &[&str],
    columns: &[(String, ColType)],
    row: ItemKey,
    before: &[CellValue],
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
                        PreviewSqlError::at(row, k, PreviewSqlCause::MissingColumn((*k).to_owned()))
                    })
            })
            .collect::<Result<_, _>>()?
    };
    let terms: Vec<String> = keyed
        .into_iter()
        .map(|(ci, name)| match &before[ci] {
            CellValue::Null => Ok(format!("{name} IS NULL")),
            CellValue::Default => Err(PreviewSqlError::at(
                row,
                name,
                PreviewSqlCause::InvalidDefaultPredicate,
            )),
            other => Ok(format!("{name} = {}", cell_literal(row, name, other)?)),
        })
        .collect::<Result<_, _>>()?;
    Ok(terms.join(" AND "))
}

/// Complete statement list for a captured change set, or the first
/// serialization error: a partial list is never returned. The snapshot is the
/// only input, so the SQL cannot drift from what was captured.
///
/// - INSERT lists every column except an explicit `Default` cell and a column
///   the schema generates; an explicit `Null` is written as `NULL`.
/// - UPDATE sets only the changed columns, writing an explicit `Default` as
///   `DEFAULT`, and matches the base row. A change to a generated column is an
///   error rather than a dropped statement.
/// - DELETE matches the base row.
/// - A row predicate uses the primary key when the projection has it, else every
///   projected column, and fails if the key is missing or holds `Default`.
pub fn preview_sql(
    table: &Table,
    columns: &[(String, ColType)],
    changes: &GridChangeSet,
) -> Result<Vec<String>, PreviewSqlError> {
    let pk: Vec<&str> = table
        .primary_key()
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    let generated = |ci: usize| table.column(&columns[ci].0).is_some_and(|c| c.generated);
    let mut out = Vec::new();
    for row in changes.updated() {
        let key = row.key();
        check_width(key, columns, row.before())?;
        let sets: Vec<String> = row
            .cells()
            .iter()
            .map(|change| {
                let name = &columns[change.column()].0;
                if generated(change.column()) {
                    return Err(PreviewSqlError::at(
                        key,
                        name,
                        PreviewSqlCause::GeneratedColumn,
                    ));
                }
                Ok(format!(
                    "{name} = {}",
                    cell_literal(key, name, change.after())?
                ))
            })
            .collect::<Result<_, PreviewSqlError>>()?;
        out.push(format!(
            "UPDATE {} SET {} WHERE {};",
            table.qualified(),
            sets.join(", "),
            row_predicate(&pk, columns, key, row.before())?
        ));
    }
    for row in changes.inserted() {
        let key = row.key();
        check_width(key, columns, row.values())?;
        let mut names = vec![];
        let mut vals = vec![];
        for (ci, (name, _)) in columns.iter().enumerate() {
            let v = &row.values()[ci];
            if matches!(v, CellValue::Default) || generated(ci) {
                continue;
            }
            names.push(name.clone());
            vals.push(cell_literal(key, name, v)?);
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
    for row in changes.deleted() {
        check_width(row.key(), columns, row.before())?;
        out.push(format!(
            "DELETE FROM {} WHERE {};",
            table.qualified(),
            row_predicate(&pk, columns, row.key(), row.before())?
        ));
    }
    Ok(out)
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
        Value::Text(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Json(j) => format!("'{}'::jsonb", j.replace('\'', "''")),
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

    let stmt_start = crate::sql::statement_at(src, cursor)
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

pub fn complete(source: Source, src: &str, cursor: usize) -> (Vec<Completion>, usize) {
    let cat = source.catalog;
    let (word, clause, qualifier) = context(src, cursor);
    let stmt = crate::sql::statement_at(src, cursor)
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
                        format!(
                            "{} · {}",
                            t.schema,
                            crate::sql::fmt_rows(source.row_count(t))
                        ),
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
                        format!(
                            "{} · {} rows",
                            t.schema,
                            crate::sql::fmt_rows(source.row_count(t))
                        ),
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
mod tests {
    use super::*;
    use crate::db::{Column, ObjectKind};
    use junie_tui::core::id::WidgetId;
    use junie_tui::widgets::grid::{CellKind, ColumnSpec, DataGrid, GridRows, RowTotal};

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
        })
        .unwrap();
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
        grid.record_cell(0, 0, CellValue::Num(9.876_543)).unwrap();
        grid.record_cell(0, 1, CellValue::Num(0.001)).unwrap();
        grid.record_cell(1, 1, CellValue::Num(8.0)).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap(),
            vec![
                "UPDATE public.ledger SET ref = 9.876543, amount = 0.001 WHERE ref = 1.234567;"
                    .to_owned(),
                "UPDATE public.ledger SET amount = 8.0 WHERE ref IS NULL;".to_owned(),
            ]
        );
        grid.discard().unwrap();
        grid.toggle_delete(0).unwrap();
        grid.toggle_delete(1).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap(),
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
        grid.record_cell(0, 2, CellValue::Num(0.001)).unwrap();
        grid.insert_row().unwrap();
        grid.record_cell(1, 0, CellValue::Num(1.234_567_8)).unwrap();
        grid.record_cell(1, 1, CellValue::Text("it's".into()))
            .unwrap();
        grid.record_cell(1, 2, CellValue::Num(-0.000_1)).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap(),
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
        grid.record_cell(0, 1, CellValue::Num(6.5)).unwrap();
        grid.record_cell(1, 1, CellValue::Num(f64::NAN)).unwrap();
        let err = preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap_err();
        assert_eq!(
            err,
            PreviewSqlError {
                row: PreviewRow::Key(grid.row_key(1).unwrap()),
                column: Some("amount".into()),
                cause: PreviewSqlCause::Literal(SqlLiteralError::NonFiniteNumber),
            }
        );

        grid.undo().unwrap();
        grid.insert_row().unwrap();
        grid.record_cell(2, 0, CellValue::Num(f64::INFINITY))
            .unwrap();
        let err = preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap_err();
        assert_eq!(err.row, PreviewRow::Key(grid.row_key(2).unwrap()));
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
        grid.record_cell(0, 1, CellValue::Num(6.5)).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap_err(),
            PreviewSqlError {
                row: PreviewRow::Key(grid.row_key(0).unwrap()),
                column: Some("ref".into()),
                cause: PreviewSqlCause::Literal(SqlLiteralError::NonFiniteNumber),
            },
            "an invalid before-image names its own column"
        );

        grid.discard().unwrap();
        grid.toggle_delete(1).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap_err(),
            PreviewSqlError {
                row: PreviewRow::Key(grid.row_key(1).unwrap()),
                column: Some("ref".into()),
                cause: PreviewSqlCause::InvalidDefaultPredicate,
            }
        );

        let orphan = table(vec![
            column("id", ColType::Int, true),
            column("amount", ColType::Numeric, false),
        ]);
        let projected = table(vec![column("amount", ColType::Numeric, false)]);
        let (narrow, mut grid) = grid_of(&projected, vec![vec![CellValue::Num(1.0)]]);
        grid.toggle_delete(0).unwrap();
        assert_eq!(
            preview_sql(&orphan, &narrow, &grid.snapshot_changes()).unwrap_err(),
            PreviewSqlError {
                row: PreviewRow::Key(grid.row_key(0).unwrap()),
                column: Some("id".into()),
                cause: PreviewSqlCause::MissingColumn("id".into()),
            }
        );
    }

    fn generated_id(name: &str) -> Column {
        Column {
            generated: true,
            ..column(name, ColType::Int, true)
        }
    }

    fn notes() -> Table {
        table(vec![
            generated_id("id"),
            column("name", ColType::Text, false),
            column("note", ColType::Text, false),
        ])
    }

    fn notes_grid(rows: Vec<Vec<CellValue>>) -> (Vec<(String, ColType)>, DataGrid) {
        let t = notes();
        let columns: Vec<(String, ColType)> =
            t.columns.iter().map(|c| (c.name.clone(), c.ty)).collect();
        let specs = vec![
            ColumnSpec::new("id", CellKind::Id).primary().read_only(),
            ColumnSpec::new("name", CellKind::Text),
            ColumnSpec::new("note", CellKind::Text),
        ];
        let mut grid = DataGrid::new(WidgetId::of("issue50"), specs);
        grid.set_rows(GridRows {
            total: RowTotal::Exact(rows.len()),
            rows,
            more: false,
        })
        .unwrap();
        (columns, grid)
    }

    fn text(s: &str) -> CellValue {
        CellValue::Text(s.into())
    }

    fn one_note() -> Vec<Vec<CellValue>> {
        vec![vec![CellValue::Int(7), text("a'b"), CellValue::Null]]
    }

    #[test]
    fn issue50_duplicate_sql_includes_copied_values() {
        let (columns, mut grid) = notes_grid(one_note());
        grid.duplicate_row().expect("row duplicates");
        let sql = preview_sql(&notes(), &columns, &grid.snapshot_changes()).unwrap();
        assert_eq!(
            sql,
            vec!["INSERT INTO public.ledger (name, note) VALUES ('a''b', NULL);".to_owned()],
            "the copied text and the copied NULL are written, the generated id is not"
        );

        let copy = grid.len() - 1;
        grid.record_cell(copy, 2, text("edited")).unwrap();
        let sql = preview_sql(&notes(), &columns, &grid.snapshot_changes()).unwrap();
        assert_eq!(
            sql,
            vec!["INSERT INTO public.ledger (name, note) VALUES ('a''b', 'edited');".to_owned()],
            "one edit changes one column and keeps the other copies"
        );
    }

    #[test]
    fn issue50_sql_default_null_and_original_identity() {
        let t = notes();
        let (columns, mut grid) = notes_grid(one_note());
        grid.insert_row().expect("row inserts");
        let fresh = grid.len() - 1;
        grid.record_cell(fresh, 1, text("x")).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap(),
            vec!["INSERT INTO public.ledger (name, note) VALUES ('x', NULL);".to_owned()],
            "Default is omitted, Null is written"
        );

        grid.discard().unwrap();
        grid.record_cell(0, 2, CellValue::Default).unwrap();
        grid.record_cell(0, 1, CellValue::Null).unwrap();
        assert_eq!(
            preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap(),
            vec!["UPDATE public.ledger SET name = NULL, note = DEFAULT WHERE id = 7;".to_owned()],
            "an explicit Default stays DEFAULT, an explicit Null stays NULL"
        );

        let keyed = keyed();
        let (kcolumns, mut kgrid) =
            grid_of(&keyed, vec![vec![CellValue::Num(1.5), CellValue::Num(2.0)]]);
        kgrid.record_cell(0, 0, CellValue::Num(8.25)).unwrap();
        let captured = kgrid.snapshot_changes();
        kgrid.discard().unwrap();
        assert_eq!(
            preview_sql(&keyed, &kcolumns, &captured).unwrap(),
            vec!["UPDATE public.ledger SET ref = 8.25 WHERE ref = 1.5;".to_owned()],
            "the predicate names the original key and a retained capture needs no grid"
        );

        let (orphans, mut grid) = notes_grid(vec![vec![CellValue::Default, text("a"), text("b")]]);
        grid.toggle_delete(0).unwrap();
        let err = preview_sql(&t, &orphans, &grid.snapshot_changes()).unwrap_err();
        assert_eq!(err.column.as_deref(), Some("id"));
        assert_eq!(err.cause, PreviewSqlCause::InvalidDefaultPredicate);
        let narrow = vec![("name".to_owned(), ColType::Text)];
        let (_, mut grid) = notes_grid(vec![vec![CellValue::Int(1), text("a"), text("b")]]);
        grid.toggle_delete(0).unwrap();
        let err = preview_sql(&t, &narrow, &grid.snapshot_changes()).unwrap_err();
        assert_eq!(
            err.cause,
            PreviewSqlCause::RowShape {
                expected: 1,
                actual: 3
            },
            "a snapshot wider than the projection is refused, not indexed past"
        );
    }

    #[test]
    fn issue50_generated_column_update_is_rejected_not_dropped() {
        let t = notes();
        let (columns, mut grid) = notes_grid(one_note());
        grid.record_cell(0, 0, CellValue::Int(99)).unwrap();
        grid.record_cell(0, 1, text("kept")).unwrap();
        let err = preview_sql(&t, &columns, &grid.snapshot_changes()).unwrap_err();
        assert_eq!(err.cause, PreviewSqlCause::GeneratedColumn);
        assert_eq!(err.column.as_deref(), Some("id"));
        assert_eq!(
            err.row,
            PreviewRow::Key(grid.row_key(0).unwrap()),
            "the error names the row by key"
        );
    }
}
