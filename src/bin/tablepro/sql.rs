use crate::db::{Catalog, ColType, Table, Value};
use crate::model::quote_text;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokKind {
    Keyword,
    Ident,
    Number,

    Literal,
    String,
    Operator,
    Punct,
    Comment,
    Whitespace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokKind,
    pub start: usize,
    pub end: usize,
}

pub const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "IS",
    "IN",
    "LIKE",
    "ILIKE",
    "ORDER",
    "BY",
    "ASC",
    "DESC",
    "LIMIT",
    "OFFSET",
    "GROUP",
    "HAVING",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "FULL",
    "CROSS",
    "ON",
    "AS",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "DROP",
    "TABLE",
    "TRUNCATE",
    "ALTER",
    "CREATE",
    "INDEX",
    "VIEW",
    "DATABASE",
    "SCHEMA",
    "COLUMN",
    "ADD",
    "RENAME",
    "TO",
    "CASCADE",
    "RESTRICT",
    "EXPLAIN",
    "ANALYZE",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "DISTINCT",
    "COUNT",
    "SUM",
    "AVG",
    "MIN",
    "MAX",
    "BETWEEN",
    "EXISTS",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "UNION",
    "ALL",
    "WITH",
    "RETURNING",
    "TRUE",
    "FALSE",
    "INTERVAL",
    "NOW",
    "CAST",
    "COALESCE",
    "PRIMARY",
    "KEY",
    "REFERENCES",
    "DEFAULT",
    "UNIQUE",
    "CHECK",
    "CONSTRAINT",
    "IF",
    "USING",
    "GRANT",
    "REVOKE",
    "VACUUM",
    "REINDEX",
];

pub const FUNCTIONS: &[&str] = &[
    "count",
    "sum",
    "avg",
    "min",
    "max",
    "now",
    "coalesce",
    "lower",
    "upper",
    "length",
    "date_trunc",
    "extract",
    "to_char",
    "jsonb_extract_path_text",
    "array_agg",
    "string_agg",
    "row_number",
    "gen_random_uuid",
];

pub fn is_keyword(word: &str) -> bool {
    let up = word.to_ascii_uppercase();
    KEYWORDS.contains(&up.as_str())
}

pub fn is_literal(word: &str) -> bool {
    ["TRUE", "FALSE", "NULL"]
        .iter()
        .any(|l| word.eq_ignore_ascii_case(l))
}

/// The scalar starting at byte offset `at`, or `None` at end of input or when
/// `at` is not a char boundary. `str::get` performs the boundary check, so no
/// caller can slice inside a multi-byte scalar through this helper.
fn scalar_at(src: &str, at: usize) -> Option<char> {
    src.get(at..)?.chars().next()
}

/// Advances `at` past every leading scalar that satisfies `keep`. Returns the
/// new byte offset, which is always a char boundary.
fn scan_while(src: &str, mut at: usize, keep: impl Fn(char) -> bool) -> usize {
    while let Some(ch) = scalar_at(src, at) {
        if !keep(ch) {
            break;
        }
        at += ch.len_utf8();
    }
    at
}

/// End offset of a quoted run that opens at `start`. A doubled quote inside
/// the run is an escape and stays in the token. An unterminated run ends at
/// the end of input.
fn quoted_end(src: &str, start: usize, quote: char) -> usize {
    let mut at = start + quote.len_utf8();
    while let Some(ch) = scalar_at(src, at) {
        at += ch.len_utf8();
        if ch == quote {
            if scalar_at(src, at) == Some(quote) {
                at += quote.len_utf8();
            } else {
                break;
            }
        }
    }
    at
}

/// Splits `src` into contiguous, non-empty tokens whose byte ranges cover the
/// whole input. Every range starts and ends on a UTF-8 char boundary, so the
/// ranges can be used to slice `src` directly.
///
/// Identifiers accept an alphabetic or `_` scalar first, then alphanumeric
/// scalars or `_`. Any other scalar, such as an emoji or a combining mark,
/// becomes one punctuation token. This is a lexical policy, not full SQL
/// Unicode identifier support.
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(c) = scalar_at(src, i) {
        let start = i;
        let kind = if c.is_whitespace() {
            i = scan_while(src, i, char::is_whitespace);
            TokKind::Whitespace
        } else if c == '-' && src[i..].starts_with("--") {
            i = src[i..].find('\n').map_or(src.len(), |n| i + n);
            TokKind::Comment
        } else if c == '/' && src[i..].starts_with("/*") {
            i = src[i + 2..].find("*/").map_or(src.len(), |n| i + 2 + n + 2);
            TokKind::Comment
        } else if c == '\'' {
            i = quoted_end(src, i, '\'');
            TokKind::String
        } else if c == '"' {
            i = quoted_end(src, i, '"');
            TokKind::Ident
        } else if c.is_ascii_digit() {
            i = scan_while(src, i, |ch| ch.is_ascii_digit() || ch == '.');
            TokKind::Number
        } else if c.is_alphabetic() || c == '_' {
            i = scan_while(src, i, |ch| ch.is_alphanumeric() || ch == '_');
            let word = &src[start..i];
            if is_literal(word) {
                TokKind::Literal
            } else if is_keyword(word) {
                TokKind::Keyword
            } else {
                TokKind::Ident
            }
        } else if "=<>!+-*/%|".contains(c) {
            i = scan_while(src, i, |ch| "=<>!+-*/%|".contains(ch));
            TokKind::Operator
        } else {
            i += c.len_utf8();
            TokKind::Punct
        };
        out.push(Token {
            kind,
            start,
            end: i,
        });
    }
    out
}

pub fn split_statements(src: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for t in tokenize(src) {
        if t.kind == TokKind::Punct && &src[t.start..t.end] == ";" {
            let s = trim_range(src, start, t.start);
            if s.0 < s.1 {
                out.push(s);
            }
            start = t.end;
        }
    }
    let s = trim_range(src, start, src.len());
    if s.0 < s.1 {
        out.push(s);
    }
    out
}

fn trim_range(src: &str, mut a: usize, mut b: usize) -> (usize, usize) {
    while a < b && src.as_bytes()[a].is_ascii_whitespace() {
        a += 1;
    }
    while b > a && src.as_bytes()[b - 1].is_ascii_whitespace() {
        b -= 1;
    }
    (a, b)
}

/// A WHERE condition. `All` and `Any` are the only boolean connectives; each
/// UI filter and each query-editor conjunction becomes one child.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    All(Vec<Expr>),
    Any(Vec<Expr>),
    Not(Box<Expr>),
    Leaf(Predicate),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Predicate {
    pub column: String,
    pub op: PredicateOp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PredicateOp {
    Eq(String),
    Ne(String),
    Gt(String),
    Ge(String),
    Lt(String),
    Le(String),
    IsNull,
    IsNotNull,
    /// Literal, case-insensitive substring and prefix/suffix tests from the UI.
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    /// SQL LIKE and ILIKE: `%` and `_` are wildcards, `\` escapes the next
    /// character, and only ILIKE ignores case.
    Like {
        pattern: String,
        case_insensitive: bool,
    },
    In(Vec<String>),
    Regex(String),
}

impl Expr {
    pub fn leaves(&self) -> usize {
        match self {
            Expr::All(xs) | Expr::Any(xs) => xs.iter().map(Expr::leaves).sum(),
            Expr::Not(x) => x.leaves(),
            Expr::Leaf(_) => 1,
        }
    }

    pub fn to_sql(&self) -> String {
        match self {
            Expr::All(xs) => join_sql(xs, " AND "),
            Expr::Any(xs) => join_sql(xs, " OR "),
            Expr::Not(x) => match x.as_ref() {
                Expr::Leaf(p) => p
                    .to_sql_negated()
                    .unwrap_or_else(|| format!("NOT ({})", p.to_sql())),
                other => format!("NOT ({})", other.to_sql()),
            },
            Expr::Leaf(p) => p.to_sql(),
        }
    }
}

/// Only nested connectives need parentheses. Leaves and NOT bind tighter than
/// AND and OR, so they print bare.
fn join_sql(xs: &[Expr], sep: &str) -> String {
    let parts: Vec<String> = xs
        .iter()
        .map(|x| match x {
            Expr::All(_) | Expr::Any(_) => format!("({})", x.to_sql()),
            _ => x.to_sql(),
        })
        .collect();
    parts.join(sep)
}

/// PostgreSQL reserved words the simulator's keyword list leaves out. A column
/// named like one must be quoted to parse.
const RESERVED_IDENTS: &[&str] = &["user", "only", "for", "some", "array", "window"];

fn is_reserved_ident(name: &str) -> bool {
    is_keyword(name) || RESERVED_IDENTS.iter().any(|r| name.eq_ignore_ascii_case(r))
}

/// Plain lowercase names stay bare, so previews read as typed. Anything else
/// is double-quoted with embedded quotes doubled.
fn quote_ident(name: &str) -> String {
    let plain = name
        .chars()
        .enumerate()
        .all(|(i, c)| c == '_' || c.is_ascii_lowercase() || (i > 0 && c.is_ascii_digit()));
    if plain && !name.is_empty() && !is_reserved_ident(name) {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

fn like_keyword(case_insensitive: bool) -> &'static str {
    if case_insensitive { "ILIKE" } else { "LIKE" }
}

/// Escapes LIKE wildcards so a literal UI value matches itself.
fn escape_like(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// A UI contains, starts-with or ends-with value as a quoted LIKE pattern with
/// its explicit `ESCAPE` clause. `head` and `tail` are the only wildcards left
/// unescaped, so the value itself matches literally.
fn like_literal(v: &str, head: &str, tail: &str) -> String {
    let pattern = format!("{head}{}{tail}", escape_like(v));
    format!("{} ESCAPE '\\'", quote_text(&pattern))
}

impl Predicate {
    /// The SQL form of `NOT (predicate)` where SQL has a negated spelling, such
    /// as `NOT ILIKE` or `NOT IN`. Other operators keep the wrapping form.
    fn to_sql_negated(&self) -> Option<String> {
        let c = quote_ident(&self.column);
        Some(match &self.op {
            PredicateOp::Contains(v) => format!("{c} NOT ILIKE {}", like_literal(v, "%", "%")),
            PredicateOp::StartsWith(v) => format!("{c} NOT ILIKE {}", like_literal(v, "", "%")),
            PredicateOp::EndsWith(v) => format!("{c} NOT ILIKE {}", like_literal(v, "%", "")),
            PredicateOp::Like {
                pattern,
                case_insensitive,
            } => format!(
                "{c} NOT {} {}",
                like_keyword(*case_insensitive),
                quote_text(pattern)
            ),
            PredicateOp::In(items) => format!(
                "{c} NOT IN ({})",
                items
                    .iter()
                    .map(|i| quote_text(i))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PredicateOp::Regex(p) => format!("{c} !~ {}", quote_text(p)),
            _ => return None,
        })
    }

    fn to_sql(&self) -> String {
        let c = quote_ident(&self.column);
        match &self.op {
            PredicateOp::Eq(v) => format!("{c} = {}", quote_text(v)),
            PredicateOp::Ne(v) => format!("{c} != {}", quote_text(v)),
            PredicateOp::Gt(v) => format!("{c} > {}", quote_text(v)),
            PredicateOp::Ge(v) => format!("{c} >= {}", quote_text(v)),
            PredicateOp::Lt(v) => format!("{c} < {}", quote_text(v)),
            PredicateOp::Le(v) => format!("{c} <= {}", quote_text(v)),
            PredicateOp::IsNull => format!("{c} IS NULL"),
            PredicateOp::IsNotNull => format!("{c} IS NOT NULL"),
            PredicateOp::Contains(v) => format!("{c} ILIKE {}", like_literal(v, "%", "%")),
            PredicateOp::StartsWith(v) => format!("{c} ILIKE {}", like_literal(v, "", "%")),
            PredicateOp::EndsWith(v) => format!("{c} ILIKE {}", like_literal(v, "%", "")),
            PredicateOp::Like {
                pattern,
                case_insensitive,
            } => format!(
                "{c} {} {}",
                like_keyword(*case_insensitive),
                quote_text(pattern)
            ),
            PredicateOp::In(items) => format!(
                "{c} IN ({})",
                items
                    .iter()
                    .map(|i| quote_text(i))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PredicateOp::Regex(p) => format!("{c} ~ {}", quote_text(p)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Select {
    pub columns: Vec<String>,
    pub schema: Option<String>,
    pub table: String,
    /// `None` is unrestricted. A present expression is never an empty connective.
    pub filter: Option<Expr>,
    pub order: Option<(String, bool)>,
    pub limit: Option<usize>,
    pub count_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(Select),
    Update {
        table: String,
        has_where: bool,
    },
    Delete {
        table: String,
        has_where: bool,
    },
    Insert {
        table: String,
    },
    Drop {
        kind: String,
        name: String,
    },
    Truncate {
        table: String,
    },
    Alter {
        table: String,
        destructive: bool,
    },
    Create {
        kind: String,
        name: String,
    },
    Explain {
        analyze: bool,
        inner: Box<Statement>,
    },
    Other(String),
}

impl Statement {
    pub fn verb(&self) -> &'static str {
        match self {
            Statement::Select(_) => "SELECT",
            Statement::Update { .. } => "UPDATE",
            Statement::Delete { .. } => "DELETE",
            Statement::Insert { .. } => "INSERT",
            Statement::Drop { .. } => "DROP",
            Statement::Truncate { .. } => "TRUNCATE",
            Statement::Alter { .. } => "ALTER",
            Statement::Create { .. } => "CREATE",
            Statement::Explain { .. } => "EXPLAIN",
            Statement::Other(_) => "STATEMENT",
        }
    }

    pub fn target(&self) -> Option<&str> {
        match self {
            Statement::Select(s) => Some(&s.table),
            Statement::Update { table, .. }
            | Statement::Delete { table, .. }
            | Statement::Insert { table }
            | Statement::Truncate { table }
            | Statement::Alter { table, .. } => Some(table),
            Statement::Drop { name, .. } | Statement::Create { name, .. } => Some(name),
            Statement::Explain { inner, .. } => inner.target(),
            Statement::Other(_) => None,
        }
    }
}

struct Words<'a> {
    src: &'a str,
    toks: Vec<Token>,
    pos: usize,
}

impl<'a> Words<'a> {
    fn new(src: &'a str) -> Self {
        let toks = tokenize(src)
            .into_iter()
            .filter(|t| !matches!(t.kind, TokKind::Whitespace | TokKind::Comment))
            .collect();
        Self { src, toks, pos: 0 }
    }
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(|t| &self.src[t.start..t.end])
    }
    fn peek_up(&self) -> String {
        self.peek().unwrap_or("").to_ascii_uppercase()
    }
    fn next(&mut self) -> Option<&str> {
        let t = self.toks.get(self.pos)?;
        self.pos += 1;
        Some(&self.src[t.start..t.end])
    }
    fn accept(&mut self, kw: &str) -> bool {
        if self.peek_up() == kw {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn ident(&mut self) -> Option<String> {
        let t = self.toks.get(self.pos)?;
        if matches!(t.kind, TokKind::Ident | TokKind::Keyword | TokKind::Literal) {
            self.pos += 1;
            Some(identifier(&self.src[t.start..t.end]))
        } else {
            None
        }
    }

    fn qualified(&mut self) -> Option<(Option<String>, String)> {
        let first = self.ident()?;
        if self.peek() == Some(".") {
            self.pos += 1;
            let second = self.ident()?;
            Some((Some(first), second))
        } else {
            Some((None, first))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,

    pub at: usize,
}

pub fn parse(stmt: &str) -> Result<Statement, ParseError> {
    let mut w = Words::new(stmt);
    let err = |w: &Words, m: &str| ParseError {
        message: m.to_owned(),
        at: w.toks.get(w.pos).map(|t| t.start).unwrap_or(stmt.len()),
    };
    match w.peek_up().as_str() {
        "SELECT" => parse_select(&mut w).map(Statement::Select),
        "EXPLAIN" => {
            w.next();
            let mut analyze = false;
            loop {
                if w.accept("ANALYZE") {
                    analyze = true;
                } else if w.accept("VERBOSE") || w.accept("(") {
                    while let Some(t) = w.peek() {
                        if t == ")" {
                            w.next();
                            break;
                        }
                        if t.eq_ignore_ascii_case("ANALYZE") {
                            analyze = true;
                        }
                        w.next();
                    }
                } else {
                    break;
                }
            }
            let rest = &stmt[w.toks.get(w.pos).map(|t| t.start).unwrap_or(stmt.len())..];
            let inner = parse(rest).map_err(|e| ParseError {
                message: e.message,
                at: e.at + (stmt.len() - rest.len()),
            })?;
            Ok(Statement::Explain {
                analyze,
                inner: Box::new(inner),
            })
        }
        "UPDATE" => {
            w.next();
            let (_, table) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected a table name after UPDATE"))?;
            if !w.accept("SET") {
                return Err(err(&w, "Expected SET"));
            }
            let has_where = stmt.to_ascii_uppercase().contains(" WHERE ")
                || stmt.to_ascii_uppercase().contains("\nWHERE");
            Ok(Statement::Update { table, has_where })
        }
        "DELETE" => {
            w.next();
            if !w.accept("FROM") {
                return Err(err(&w, "Expected FROM after DELETE"));
            }
            let (_, table) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected a table name"))?;
            let up = stmt.to_ascii_uppercase();
            let has_where = up.contains(" WHERE ") || up.contains("\nWHERE");
            Ok(Statement::Delete { table, has_where })
        }
        "INSERT" => {
            w.next();
            if !w.accept("INTO") {
                return Err(err(&w, "Expected INTO after INSERT"));
            }
            let (_, table) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected a table name"))?;
            Ok(Statement::Insert { table })
        }
        "DROP" => {
            w.next();
            let kind = w.next().unwrap_or("TABLE").to_ascii_uppercase();
            w.accept("IF");
            w.accept("EXISTS");
            let (_, name) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected an object name"))?;
            Ok(Statement::Drop { kind, name })
        }
        "TRUNCATE" => {
            w.next();
            w.accept("TABLE");
            let (_, table) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected a table name"))?;
            Ok(Statement::Truncate { table })
        }
        "ALTER" => {
            w.next();
            w.accept("TABLE");
            w.accept("IF");
            w.accept("EXISTS");
            let (_, table) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected a table name"))?;
            let up = stmt.to_ascii_uppercase();
            let destructive = up.contains("DROP COLUMN")
                || up.contains("DROP CONSTRAINT")
                || up.contains(" TYPE ");
            Ok(Statement::Alter { table, destructive })
        }
        "CREATE" => {
            w.next();
            let kind = w.next().unwrap_or("TABLE").to_ascii_uppercase();
            let (_, name) = w
                .qualified()
                .ok_or_else(|| err(&w, "Expected an object name"))?;
            Ok(Statement::Create { kind, name })
        }
        "" => Err(ParseError {
            message: "Empty statement".into(),
            at: 0,
        }),
        other => {
            if is_keyword(other) {
                Ok(Statement::Other(other.to_owned()))
            } else {
                Err(err(&w, &format!("syntax error at or near \"{other}\"")))
            }
        }
    }
}

/// Strips the delimiters of a quoted identifier and undoes the doubled quotes
/// inside it. An unterminated identifier loses only its opening quote, and an
/// unquoted word is returned unchanged.
fn identifier(tok: &str) -> String {
    match tok.strip_prefix('"') {
        Some(rest) => rest.strip_suffix('"').unwrap_or(rest).replace("\"\"", "\""),
        None => tok.to_owned(),
    }
}

/// Strips one pair of SQL quotes and undoes doubled apostrophes inside it.
fn literal(tok: &str) -> String {
    match tok.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
        Some(inner) => inner.replace("''", "'"),
        None => tok.to_owned(),
    }
}

fn next_literal(w: &mut Words) -> String {
    literal(w.next().unwrap_or(""))
}

fn parse_select(w: &mut Words) -> Result<Select, ParseError> {
    let err = |w: &Words, m: &str| ParseError {
        message: m.to_owned(),
        at: w.toks.get(w.pos).map(|t| t.start).unwrap_or(w.src.len()),
    };
    w.accept("SELECT");
    w.accept("DISTINCT");
    let mut columns = Vec::new();
    let mut count_only = false;
    loop {
        if w.accept("FROM") {
            break;
        }
        let Some(tok) = w.next().map(str::to_owned) else {
            return Err(err(w, "Expected FROM"));
        };
        if tok == "," {
            continue;
        }
        if tok.eq_ignore_ascii_case("count") {
            count_only = true;

            while let Some(t) = w.next() {
                if t == ")" {
                    break;
                }
            }
            columns.push("count".into());
            continue;
        }
        if tok == "*" {
            columns.push("*".into());
            continue;
        }
        if w.accept("AS") {
            w.next();
        }
        columns.push(identifier(&tok));
    }
    let (schema, table) = w
        .qualified()
        .ok_or_else(|| err(w, "Expected a table name after FROM"))?;

    if w.peek()
        .is_some_and(|p| !is_keyword(p) && p != ";" && p != ")")
    {
        w.next();
    }
    let mut conjuncts = Vec::new();
    let mut order = None;
    let mut limit = None;
    if w.accept("WHERE") {
        loop {
            let column = w.ident().ok_or_else(|| err(w, "Expected a column name"))?;
            let column = if w.peek() == Some(".") {
                w.next();
                w.ident().ok_or_else(|| err(w, "Expected a column name"))?
            } else {
                column
            };
            let op = w
                .next()
                .map(|s| s.to_ascii_uppercase())
                .ok_or_else(|| err(w, "Expected an operator"))?;
            let op = match op.as_str() {
                "=" => PredicateOp::Eq(next_literal(w)),
                "!=" | "<>" => PredicateOp::Ne(next_literal(w)),
                ">" => PredicateOp::Gt(next_literal(w)),
                ">=" => PredicateOp::Ge(next_literal(w)),
                "<" => PredicateOp::Lt(next_literal(w)),
                "<=" => PredicateOp::Le(next_literal(w)),
                "LIKE" | "ILIKE" => PredicateOp::Like {
                    pattern: next_literal(w),
                    case_insensitive: op == "ILIKE",
                },
                "IS" => {
                    if w.accept("NOT") {
                        w.accept("NULL");
                        PredicateOp::IsNotNull
                    } else {
                        w.accept("NULL");
                        PredicateOp::IsNull
                    }
                }
                "IN" => {
                    let mut items = Vec::new();
                    w.accept("(");
                    while let Some(t) = w.next() {
                        if t == ")" {
                            break;
                        }
                        if t != "," {
                            items.push(literal(t));
                        }
                    }
                    PredicateOp::In(items)
                }
                _ => return Err(err(w, &format!("Unsupported operator {op}"))),
            };
            conjuncts.push(Expr::Leaf(Predicate { column, op }));
            if !w.accept("AND") {
                break;
            }
        }
    }
    if w.accept("ORDER") {
        if !w.accept("BY") {
            return Err(err(w, "Expected BY after ORDER"));
        }
        let c = w
            .ident()
            .ok_or_else(|| err(w, "Expected a column to order by"))?;
        let asc = !w.accept("DESC");
        w.accept("ASC");
        order = Some((c, asc));
    }
    if w.accept("LIMIT") {
        let n = w
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| err(w, "Expected a number after LIMIT"))?;
        limit = Some(n);
    }
    if let Some(extra) = w.peek()
        && extra != ";"
    {
        return Err(err(w, &format!("syntax error at or near \"{extra}\"")));
    }
    Ok(Select {
        columns,
        schema,
        table,
        filter: (!conjuncts.is_empty()).then_some(Expr::All(conjuncts)),
        order,
        limit,
        count_only,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Safe,
    Write,
    Destructive,
}

pub fn tier(stmt: &Statement) -> Tier {
    match stmt {
        Statement::Select(_) => Tier::Safe,
        Statement::Explain { inner, .. } => tier(inner),
        Statement::Drop { .. }
        | Statement::Truncate { .. }
        | Statement::Alter {
            destructive: true, ..
        } => Tier::Destructive,
        _ => Tier::Write,
    }
}

pub fn is_dangerous(stmt: &Statement) -> bool {
    match stmt {
        Statement::Explain { inner, .. } => is_dangerous(inner),
        _ => {
            tier(stmt) == Tier::Destructive
                || matches!(
                    stmt,
                    Statement::Delete {
                        has_where: false,
                        ..
                    }
                )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Run,

    Confirm { deliberate: bool },

    Deny,
}

pub fn gate(level: crate::db::SafeMode, stmt: &Statement) -> Decision {
    let t = tier(stmt);
    let write = t != Tier::Safe;
    if level == crate::db::SafeMode::ReadOnly && write {
        return Decision::Deny;
    }
    let confirm = is_dangerous(stmt)
        || (level.requires_confirmation() && (write || level.applies_to_all_queries()));
    if !confirm {
        return Decision::Run;
    }
    let deliberate = level.requires_authentication() && (write || level.applies_to_all_queries());
    Decision::Confirm { deliberate }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Risk {
    pub tier: Tier,
    pub dangerous: bool,
    pub action: String,
    pub scope: String,
    pub risk: String,
    pub reversible: &'static str,
}

pub fn fmt_rows(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1} M", n as f64 / 1e6)
    } else if n >= 1000 {
        format!("{:.1} k", n as f64 / 1e3)
    } else {
        n.to_string()
    }
}

pub fn assess(stmt: &Statement, table: Option<&Table>) -> Risk {
    let rows = table.map(|t| t.row_count).unwrap_or(0);
    let t = tier(stmt);
    let dangerous = is_dangerous(stmt);
    let (action, scope, risk, reversible) = match stmt {
        Statement::Update {
            table: tb,
            has_where: false,
        } => (
            "UPDATE without WHERE".to_owned(),
            format!("every row in {tb} ({} rows)", fmt_rows(rows)),
            "Overwrites the column values of all rows at once.".to_owned(),
            "Reversible only by a compensating UPDATE or a backup",
        ),
        Statement::Delete {
            table: tb,
            has_where: false,
        } => (
            "DELETE without WHERE".to_owned(),
            format!("every row in {tb} ({} rows)", fmt_rows(rows)),
            "Removes all rows; dependent rows may go with ON DELETE CASCADE.".to_owned(),
            "Not reversible without a backup",
        ),
        Statement::Update { table: tb, .. } => (
            "UPDATE".to_owned(),
            format!("matching rows in {tb}"),
            String::new(),
            "Reversible by a compensating UPDATE",
        ),
        Statement::Delete { table: tb, .. } => (
            "DELETE".to_owned(),
            format!("matching rows in {tb}"),
            String::new(),
            "Not reversible without a backup",
        ),
        Statement::Insert { table: tb } => (
            "INSERT".to_owned(),
            format!("new rows in {tb}"),
            String::new(),
            "Reversible by deleting the inserted rows",
        ),
        Statement::Drop { kind, name } => (
            format!("DROP {kind}"),
            if kind == "DATABASE" {
                format!("the whole database {name}")
            } else {
                format!(
                    "{name}, its {} rows, indexes and constraints",
                    fmt_rows(rows)
                )
            },
            "The object and everything stored in it disappears immediately.".to_owned(),
            "Not reversible",
        ),
        Statement::Truncate { table: tb } => (
            "TRUNCATE".to_owned(),
            format!("every row in {tb} ({} rows)", fmt_rows(rows)),
            "Removes all rows without firing row triggers; audit rows are not written.".to_owned(),
            "Not reversible without a backup",
        ),
        Statement::Alter {
            table: tb,
            destructive: true,
        } => (
            "ALTER TABLE with DROP".to_owned(),
            format!("{tb} structure"),
            "Dropping columns or constraints discards data and may break dependent views."
                .to_owned(),
            "Not reversible for dropped data",
        ),
        Statement::Alter { table: tb, .. } => (
            "ALTER TABLE".to_owned(),
            format!("{tb} structure"),
            "Schema changes lock the table while they run.".to_owned(),
            "Reversible by a compensating ALTER",
        ),
        Statement::Create { kind, name } => (
            format!("CREATE {kind}"),
            name.clone(),
            String::new(),
            "Reversible by DROP",
        ),
        Statement::Explain { inner, .. } => {
            let r = assess(inner, table);
            (
                format!("EXPLAIN ANALYZE runs {}", r.action),
                r.scope,
                r.risk,
                r.reversible,
            )
        }
        Statement::Select(s) => ("SELECT".to_owned(), s.table.clone(), String::new(), ""),
        Statement::Other(v) => (v.clone(), String::new(), String::new(), ""),
    };
    Risk {
        tier: t,
        dangerous,
        action,
        scope,
        risk,
        reversible,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResultSet {
    pub columns: Vec<(String, ColType)>,
    pub rows: Vec<Vec<Value>>,

    pub total: usize,
    pub source: Option<String>,
    pub duration_ms: u32,

    pub editable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecError {
    pub message: String,
    pub detail: Option<String>,

    pub at: Option<usize>,
}

pub const ROW_CAP: usize = 500;

/// SQL three-valued logic. A row is kept only when its condition is `True`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    False,
    Unknown,
    True,
}

impl Truth {
    fn from_bool(b: bool) -> Self {
        if b { Self::True } else { Self::False }
    }

    pub fn not(self) -> Self {
        match self {
            Self::False => Self::True,
            Self::True => Self::False,
            Self::Unknown => Self::Unknown,
        }
    }

    pub fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::True,
        }
    }

    pub fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::False,
        }
    }
}

/// An expression with its columns resolved and its patterns compiled, ready to
/// test every scanned row.
#[derive(Debug)]
pub enum Prepared {
    All(Vec<Prepared>),
    Any(Vec<Prepared>),
    Not(Box<Prepared>),
    Leaf { col: usize, test: Test },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

/// A comparison operand, lowercased once for text comparison and parsed once
/// for numeric comparison. Comparison and list membership both go through
/// `compare`, so `= x` and `IN (x)` always agree.
#[derive(Debug)]
pub struct Operand {
    lower: String,
    int: Option<i64>,
    num: Option<f64>,
}

impl Operand {
    fn new(text: &str) -> Self {
        Self {
            lower: text.to_lowercase(),
            int: text.parse::<i64>().ok(),
            num: text.parse::<f64>().ok().filter(|n| n.is_finite()),
        }
    }

    /// Integers compare as i64, so values past 2^53 stay distinct. Other
    /// numeric pairs compare as f64, and anything else as lowercase text.
    fn compare(&self, v: &Value) -> std::cmp::Ordering {
        match (v, self.int, self.num) {
            (Value::Int(a), Some(b), _) => a.cmp(&b),
            (Value::Int(a), None, Some(b)) => (*a as f64).total_cmp(&b),
            (Value::Num(a), _, Some(b)) => a.total_cmp(&b),
            _ => v.display().to_lowercase().cmp(&self.lower),
        }
    }
}

#[derive(Debug)]
pub enum Test {
    IsNull,
    IsNotNull,
    Cmp(CmpOp, Operand),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    Like {
        pattern: Vec<char>,
        case_insensitive: bool,
    },
    In(Vec<Operand>),
    Regex(regex::Regex),
}

// Regex compilations on this thread, so a test can prove a pattern compiles
// once per load and never per row.
#[cfg(test)]
thread_local! {
    pub(crate) static REGEX_COMPILES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Compiles a filter pattern once. The message names the regex syntax error
/// on one line, for the editor and the query error alike.
pub fn compile_regex(pattern: &str) -> Result<regex::Regex, String> {
    #[cfg(test)]
    REGEX_COMPILES.with(|n| n.set(n.get() + 1));
    regex::Regex::new(pattern).map_err(|e| {
        let text = e.to_string();
        let last = text.lines().last().unwrap_or("").trim();
        format!("invalid regular expression: {last}")
    })
}

fn exec_error(message: String) -> ExecError {
    ExecError {
        message,
        detail: None,
        at: None,
    }
}

/// Resolves columns and compiles patterns for one load. Errors name the first
/// unknown column or invalid pattern.
pub fn prepare(table: &Table, expr: &Expr) -> Result<Prepared, ExecError> {
    Ok(match expr {
        Expr::All(xs) => Prepared::All(
            xs.iter()
                .map(|x| prepare(table, x))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Any(xs) => Prepared::Any(
            xs.iter()
                .map(|x| prepare(table, x))
                .collect::<Result<_, _>>()?,
        ),
        Expr::Not(x) => Prepared::Not(Box::new(prepare(table, x)?)),
        Expr::Leaf(p) => {
            let col = table
                .columns
                .iter()
                .position(|c| c.name.eq_ignore_ascii_case(&p.column))
                .ok_or_else(|| ExecError {
                    message: format!("column \"{}\" does not exist", p.column),
                    detail: Some(format!(
                        "Columns of {}: {}",
                        table.name,
                        table
                            .columns
                            .iter()
                            .map(|c| c.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                    at: None,
                })?;
            Prepared::Leaf {
                col,
                test: prepare_test(&p.op)?,
            }
        }
    })
}

fn prepare_test(op: &PredicateOp) -> Result<Test, ExecError> {
    Ok(match op {
        PredicateOp::Eq(v) => Test::Cmp(CmpOp::Eq, Operand::new(v)),
        PredicateOp::Ne(v) => Test::Cmp(CmpOp::Ne, Operand::new(v)),
        PredicateOp::Gt(v) => Test::Cmp(CmpOp::Gt, Operand::new(v)),
        PredicateOp::Ge(v) => Test::Cmp(CmpOp::Ge, Operand::new(v)),
        PredicateOp::Lt(v) => Test::Cmp(CmpOp::Lt, Operand::new(v)),
        PredicateOp::Le(v) => Test::Cmp(CmpOp::Le, Operand::new(v)),
        PredicateOp::IsNull => Test::IsNull,
        PredicateOp::IsNotNull => Test::IsNotNull,
        PredicateOp::Contains(v) => Test::Contains(v.to_lowercase()),
        PredicateOp::StartsWith(v) => Test::StartsWith(v.to_lowercase()),
        PredicateOp::EndsWith(v) => Test::EndsWith(v.to_lowercase()),
        PredicateOp::Like {
            pattern,
            case_insensitive,
        } => Test::Like {
            pattern: if *case_insensitive {
                pattern.to_lowercase()
            } else {
                pattern.clone()
            }
            .chars()
            .collect(),
            case_insensitive: *case_insensitive,
        },
        PredicateOp::In(items) => Test::In(items.iter().map(|i| Operand::new(i)).collect()),
        PredicateOp::Regex(p) => Test::Regex(compile_regex(p).map_err(exec_error)?),
    })
}

impl Prepared {
    pub fn truth(&self, row: &[Value]) -> Truth {
        match self {
            Prepared::All(xs) => xs.iter().fold(Truth::True, |acc, x| acc.and(x.truth(row))),
            Prepared::Any(xs) => xs.iter().fold(Truth::False, |acc, x| acc.or(x.truth(row))),
            Prepared::Not(x) => x.truth(row).not(),
            Prepared::Leaf { col, test } => test.truth(&row[*col]),
        }
    }

    pub fn admits(&self, row: &[Value]) -> bool {
        self.truth(row) == Truth::True
    }
}

impl Test {
    fn truth(&self, v: &Value) -> Truth {
        match self {
            Test::IsNull => Truth::from_bool(*v == Value::Null),
            Test::IsNotNull => Truth::from_bool(*v != Value::Null),
            _ if *v == Value::Null => Truth::Unknown,
            Test::Cmp(op, operand) => {
                let ord = operand.compare(v);
                Truth::from_bool(match op {
                    CmpOp::Eq => ord.is_eq(),
                    CmpOp::Ne => !ord.is_eq(),
                    CmpOp::Gt => ord.is_gt(),
                    CmpOp::Ge => ord.is_ge(),
                    CmpOp::Lt => ord.is_lt(),
                    CmpOp::Le => ord.is_le(),
                })
            }
            Test::Contains(needle) => Truth::from_bool(v.display().to_lowercase().contains(needle)),
            Test::StartsWith(prefix) => {
                Truth::from_bool(v.display().to_lowercase().starts_with(prefix))
            }
            Test::EndsWith(suffix) => {
                Truth::from_bool(v.display().to_lowercase().ends_with(suffix))
            }
            Test::Like {
                pattern,
                case_insensitive,
            } => {
                let text = if *case_insensitive {
                    v.display().to_lowercase()
                } else {
                    v.display()
                };
                let text: Vec<char> = text.chars().collect();
                Truth::from_bool(like_match(pattern, &text))
            }
            Test::In(items) => Truth::from_bool(items.iter().any(|item| item.compare(v).is_eq())),
            Test::Regex(re) => Truth::from_bool(re.is_match(&v.display())),
        }
    }
}

/// SQL LIKE over characters: `%` matches any run, `_` one character, and a
/// backslash makes the next pattern character literal.
fn like_match(pat: &[char], s: &[char]) -> bool {
    match pat.split_first() {
        None => s.is_empty(),
        Some(('%', rest)) => (0..=s.len()).any(|k| like_match(rest, &s[k..])),
        Some(('_', rest)) => !s.is_empty() && like_match(rest, &s[1..]),
        Some(('\\', rest)) => match rest.split_first() {
            Some((c, after)) => s.first() == Some(c) && like_match(after, &s[1..]),
            None => s.first() == Some(&'\\') && like_match(&[], &s[1..]),
        },
        Some((c, rest)) => s.first() == Some(c) && like_match(rest, &s[1..]),
    }
}

pub fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => std::cmp::Ordering::Equal,
        (Value::Null, _) => std::cmp::Ordering::Greater,
        (_, Value::Null) => std::cmp::Ordering::Less,
        _ => match (a.as_f64(), b.as_f64()) {
            (Some(x), Some(y)) => x.total_cmp(&y),
            _ => a.display().to_lowercase().cmp(&b.display().to_lowercase()),
        },
    }
}

pub fn run_select(cat: &Catalog, sel: &Select) -> Result<ResultSet, ExecError> {
    let table = cat
        .find(sel.schema.as_deref(), &sel.table)
        .ok_or_else(|| ExecError {
            message: format!(
                "relation \"{}\" does not exist",
                match &sel.schema {
                    Some(s) => format!("{s}.{}", sel.table),
                    None => sel.table.clone(),
                }
            ),
            detail: Some("Check the schema search path or qualify the table name.".into()),
            at: None,
        })?;
    if table.columns.is_empty() {
        return Err(ExecError {
            message: format!("\"{}\" is not a table or view", table.name),
            detail: None,
            at: None,
        });
    }
    let prepared = sel.filter.as_ref().map(|f| prepare(table, f)).transpose()?;
    if let Some((c, _)) = &sel.order
        && table.column(c).is_none()
    {
        return Err(ExecError {
            message: format!("column \"{c}\" does not exist"),
            detail: None,
            at: None,
        });
    }

    let proj: Vec<usize> = if sel.columns.iter().any(|c| c == "*") || sel.count_only {
        (0..table.columns.len()).collect()
    } else {
        let mut idx = Vec::new();
        for c in &sel.columns {
            match table
                .columns
                .iter()
                .position(|tc| tc.name.eq_ignore_ascii_case(c))
            {
                Some(i) => idx.push(i),
                None => {
                    return Err(ExecError {
                        message: format!("column \"{c}\" does not exist"),
                        detail: Some(format!(
                            "Perhaps you meant one of: {}",
                            table
                                .columns
                                .iter()
                                .map(|c| c.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                        at: None,
                    });
                }
            }
        }
        idx
    };
    let scan = table.row_count.min(2_000);
    let mut all = crate::db::rows(table, 0, scan);
    if let Some(p) = &prepared {
        all.retain(|r| p.admits(r));
    }
    if let Some((c, asc)) = &sel.order {
        let ci = table
            .columns
            .iter()
            .position(|tc| tc.name.eq_ignore_ascii_case(c))
            .unwrap();
        all.sort_by(|a, b| {
            let o = cmp_values(&a[ci], &b[ci]);
            if *asc { o } else { o.reverse() }
        });
    }

    let total = if sel.filter.is_none() {
        table.row_count
    } else {
        ((all.len() as f64 / scan as f64) * table.row_count as f64).round() as usize
    };
    if sel.count_only {
        return Ok(ResultSet {
            columns: vec![("count".into(), ColType::Int)],
            rows: vec![vec![Value::Int(total as i64)]],
            total: 1,
            source: None,
            duration_ms: 12 + (table.row_count / 50_000) as u32,
            editable: false,
        });
    }
    let cap = sel.limit.unwrap_or(ROW_CAP).min(ROW_CAP);
    all.truncate(cap);
    let rows: Vec<Vec<Value>> = all
        .into_iter()
        .map(|r| proj.iter().map(|&i| r[i].clone()).collect())
        .collect();
    let columns = proj
        .iter()
        .map(|&i| (table.columns[i].name.clone(), table.columns[i].ty))
        .collect();
    let editable = table.kind == crate::db::ObjectKind::Table
        && table
            .primary_key()
            .iter()
            .all(|pk| proj.iter().any(|&i| table.columns[i].name == pk.name))
        && !table.primary_key().is_empty();
    Ok(ResultSet {
        columns,
        rows,
        total,
        source: Some(table.qualified()),
        duration_ms: 3
            + (total.min(5_000_000) / 20_000) as u32
            + sel.filter.as_ref().map_or(0, Expr::leaves) as u32 * 2,
        editable,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanNode {
    pub op: String,
    pub relation: Option<String>,
    pub detail: Vec<(String, String)>,
    pub cost: (f64, f64),
    pub rows: usize,
    pub actual_ms: Option<f64>,
    pub loops: usize,
    pub warning: Option<String>,
    pub children: Vec<PlanNode>,
}

pub fn explain(cat: &Catalog, sel: &Select, analyze: bool) -> Result<PlanNode, ExecError> {
    let table = cat
        .find(sel.schema.as_deref(), &sel.table)
        .ok_or_else(|| ExecError {
            message: format!("relation \"{}\" does not exist", sel.table),
            detail: None,
            at: None,
        })?;
    let n = table.row_count as f64;
    let leaves = sel.filter.as_ref().map_or(0, Expr::leaves);
    // Only a positive equality on an indexed column inside the top-level
    // conjunction can drive an index scan. Anything under OR or NOT is a filter.
    let conjuncts: Vec<&Expr> = match &sel.filter {
        Some(Expr::All(xs)) => xs.iter().collect(),
        Some(x) => vec![x],
        None => vec![],
    };
    let index_hit = conjuncts.iter().enumerate().find_map(|(at, x)| match x {
        Expr::Leaf(Predicate {
            column,
            op: PredicateOp::Eq(value),
        }) => table
            .indexes
            .iter()
            .find(|i| {
                i.columns
                    .first()
                    .is_some_and(|c| c.eq_ignore_ascii_case(column))
            })
            .map(|index| (at, index, column, value)),
        _ => None,
    });
    let selectivity = if leaves == 0 {
        1.0
    } else {
        0.08_f64.powi(leaves as i32).max(0.0005)
    };
    let out_rows = ((n * selectivity).round() as usize).max(1);
    let scan = if let Some((at, index, column, value)) = index_hit {
        PlanNode {
            op: "Index Scan".into(),
            relation: Some(table.qualified()),
            detail: vec![
                ("Index".into(), index.name.clone()),
                (
                    "Index Cond".into(),
                    format!("({} = {})", quote_ident(column), quote_text(value)),
                ),
            ]
            .into_iter()
            .chain(
                conjuncts
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != at)
                    .map(|(_, x)| ("Filter".into(), format!("({})", x.to_sql()))),
            )
            .collect(),
            cost: (0.56, 8.4 + out_rows as f64 * 0.012),
            rows: out_rows,
            actual_ms: analyze.then_some(0.3 + out_rows as f64 * 0.004),
            loops: 1,
            warning: None,
            children: vec![],
        }
    } else {
        let mut detail = Vec::new();
        if let Some(f) = &sel.filter {
            detail.push(("Filter".into(), format!("({})", f.to_sql())));
        }
        if sel.filter.is_some() {
            detail.push((
                "Rows Removed by Filter".into(),
                fmt_int((n - out_rows as f64).max(0.0) as usize),
            ));
        }
        let expensive = n > 500_000.0;
        PlanNode {
            op: if n > 100_000.0 {
                "Parallel Seq Scan".into()
            } else {
                "Seq Scan".into()
            },
            relation: Some(table.qualified()),
            detail,
            cost: (0.0, n * 0.0125 + 12.0),
            rows: out_rows,
            actual_ms: analyze.then_some(n * 0.00021),
            loops: 1,
            warning: expensive.then(|| {
                format!(
                    "sequential scan over {} rows; consider an index on {}",
                    fmt_int(table.row_count),
                    first_column(sel.filter.as_ref()).unwrap_or("the filter column")
                )
            }),
            children: vec![],
        }
    };
    let mut root = scan;
    if root.op.starts_with("Parallel") {
        root = PlanNode {
            op: "Gather".into(),
            relation: None,
            detail: vec![("Workers Planned".into(), "2".into())],
            cost: (root.cost.0 + 1000.0, root.cost.1 + 1200.0),
            rows: root.rows,
            actual_ms: root.actual_ms.map(|m| m + 4.2),
            loops: 1,
            warning: None,
            children: vec![root],
        };
    }
    if let Some((col, asc)) = &sel.order {
        let uses_index = table.indexes.iter().any(|i| {
            i.columns
                .first()
                .is_some_and(|c| c.eq_ignore_ascii_case(col))
        });
        let big = out_rows > 50_000;
        root = PlanNode {
            op: "Sort".into(),
            relation: None,
            detail: vec![
                (
                    "Sort Key".into(),
                    format!("{col}{}", if *asc { "" } else { " DESC" }),
                ),
                (
                    "Sort Method".into(),
                    if big {
                        format!("external merge  Disk: {}kB", out_rows / 8)
                    } else {
                        format!("quicksort  Memory: {}kB", (out_rows / 12).max(25))
                    },
                ),
            ],
            cost: (
                root.cost.1 + out_rows as f64 * 0.02,
                root.cost.1 + out_rows as f64 * 0.025,
            ),
            rows: out_rows,
            actual_ms: root.actual_ms.map(|m| m + out_rows as f64 * 0.0015),
            loops: 1,
            warning: (big && !uses_index)
                .then(|| "sort spills to disk; an index on the sort key would avoid it".into()),
            children: vec![root],
        };
    }
    if let Some(l) = sel.limit {
        root = PlanNode {
            op: "Limit".into(),
            relation: None,
            detail: vec![("Actual rows".into(), l.to_string())],
            cost: (
                root.cost.0,
                root.cost.0
                    + (root.cost.1 - root.cost.0) * (l as f64 / root.rows.max(1) as f64).min(1.0),
            ),
            rows: l.min(root.rows),
            actual_ms: root.actual_ms,
            loops: 1,
            warning: None,
            children: vec![root],
        };
    }
    if sel.count_only {
        root = PlanNode {
            op: "Aggregate".into(),
            relation: None,
            detail: vec![("Output".into(), "count(*)".into())],
            cost: (root.cost.1, root.cost.1 + 0.02),
            rows: 1,
            actual_ms: root.actual_ms.map(|m| m + 0.05),
            loops: 1,
            warning: None,
            children: vec![root],
        };
    }
    Ok(root)
}

fn first_column(e: Option<&Expr>) -> Option<&str> {
    match e? {
        Expr::All(xs) | Expr::Any(xs) => xs.iter().find_map(|x| first_column(Some(x))),
        Expr::Not(x) => first_column(Some(x)),
        Expr::Leaf(p) => Some(p.column.as_str()),
    }
}

pub fn fmt_int(n: usize) -> String {
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

pub fn plan_text(node: &PlanNode, depth: usize, out: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    let arrow = if depth == 0 { "" } else { "->  " };
    let rel = node
        .relation
        .as_ref()
        .map(|r| format!(" on {r}"))
        .unwrap_or_default();
    let actual = node
        .actual_ms
        .map(|m| {
            format!(
                " (actual time=0.031..{m:.3} rows={} loops={})",
                node.rows, node.loops
            )
        })
        .unwrap_or_default();
    out.push(format!(
        "{indent}{arrow}{}{rel}  (cost={:.2}..{:.2} rows={} width=64){actual}",
        node.op, node.cost.0, node.cost.1, node.rows
    ));
    for (k, v) in &node.detail {
        out.push(format!("{indent}      {k}: {v}"));
    }
    for c in &node.children {
        plan_text(c, depth + 1, out);
    }
}

#[cfg(test)]
mod issue10_sql_tests {
    use super::*;
    use crate::db::{Catalog, Table};
    use crate::tabs::{Filter, FilterOp};
    use junie_tui::core::id::WidgetId;

    fn orders(cat: &Catalog) -> &Table {
        cat.find(Some("public"), "orders").expect("orders fixture")
    }

    fn row_with(table: &Table, column: &str, value: Value) -> Vec<Value> {
        let mut row = vec![Value::Null; table.columns.len()];
        let i = table.columns.iter().position(|c| c.name == column).unwrap();
        row[i] = value;
        row
    }

    fn admits(table: &Table, expr: &Expr, row: &[Value]) -> bool {
        prepare(table, expr).expect("prepares").admits(row)
    }

    fn parsed_filter(sql: &str) -> Expr {
        match parse(sql).expect("parses") {
            Statement::Select(sel) => sel.filter.expect("has WHERE"),
            other => panic!("not a select: {other:?}"),
        }
    }

    fn leaf(column: &str, op: PredicateOp) -> Expr {
        Expr::Leaf(Predicate {
            column: column.into(),
            op,
        })
    }

    fn not(x: Expr) -> Expr {
        Expr::Not(Box::new(x))
    }

    fn truth_on(table: &Table, expr: &Expr, notes: Value) -> Truth {
        prepare(table, expr)
            .expect("prepares")
            .truth(&row_with(table, "notes", notes))
    }

    #[test]
    fn issue10_null_truth_survives_negation() {
        use Truth::{False as F, True as T, Unknown as U};
        let all = [F, U, T];
        let not_table = [T, U, F];
        let and_table = [[F, F, F], [F, U, U], [F, U, T]];
        let or_table = [[F, U, T], [U, U, T], [T, T, T]];
        for (i, a) in all.into_iter().enumerate() {
            assert_eq!(a.not(), not_table[i], "NOT {a:?}");
            for (j, b) in all.into_iter().enumerate() {
                assert_eq!(a.and(b), and_table[i][j], "{a:?} AND {b:?}");
                assert_eq!(a.or(b), or_table[i][j], "{a:?} OR {b:?}");
            }
        }

        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let x = || leaf("notes", PredicateOp::Eq("x".into()));
        let is_null = || leaf("notes", PredicateOp::IsNull);
        let is_not_null = || leaf("notes", PredicateOp::IsNotNull);
        let contains_x = || leaf("notes", PredicateOp::Contains("x".into()));
        let null = || Value::Null;

        assert_eq!(truth_on(table, &x(), null()), U);
        assert_eq!(truth_on(table, &not(x()), null()), U, "NOT unknown");
        assert_eq!(
            truth_on(table, &not(not(x())), null()),
            U,
            "NOT NOT unknown"
        );
        assert_eq!(truth_on(table, &not(not(x())), Value::Text("x".into())), T);
        assert_eq!(truth_on(table, &not(not(x())), Value::Text("y".into())), F);
        assert_eq!(truth_on(table, &not(is_null()), null()), F);
        assert_eq!(truth_on(table, &not(is_not_null()), null()), T);
        assert_eq!(
            truth_on(table, &Expr::Any(vec![x(), is_null()]), null()),
            T,
            "unknown OR true"
        );
        assert_eq!(
            truth_on(table, &Expr::Any(vec![x(), is_not_null()]), null()),
            U,
            "unknown OR false stays unknown"
        );
        assert_eq!(
            truth_on(
                table,
                &Expr::All(vec![is_null(), not(contains_x())]),
                null()
            ),
            U,
            "true AND NOT unknown"
        );
        assert_eq!(
            truth_on(table, &Expr::All(vec![is_not_null(), x()]), null()),
            F,
            "false AND unknown"
        );

        let nested = Expr::Any(vec![Expr::All(vec![x(), not(contains_x())]), is_null()]);
        assert!(admits(table, &nested, &row_with(table, "notes", null())));
        let excluded = Expr::All(vec![not(not(x())), Expr::Any(vec![x(), is_null()])]);
        assert!(!admits(table, &excluded, &row_with(table, "notes", null())));
    }

    #[test]
    fn issue10_empty_connectives_are_never_built_for_no_filters() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let row = row_with(table, "notes", Value::Text("x".into()));
        let any = prepare(table, &Expr::Any(vec![])).expect("prepares");
        let all = prepare(table, &Expr::All(vec![])).expect("prepares");
        assert_eq!(any.truth(&row), Truth::False, "an empty Any admits nothing");
        assert_eq!(
            all.truth(&row),
            Truth::True,
            "an empty All admits everything"
        );

        let mut tab = crate::tabs::TableTab::new(WidgetId::of("issue10-empty"), &cat, table, false);
        assert_eq!(tab.filter_expr(), None, "no filters is no clause");
        for match_all in [true, false] {
            tab.match_all = match_all;
            tab.filters = vec![Filter {
                column: "notes".into(),
                op: FilterOp::Eq,
                value: "x".into(),
                value2: String::new(),
                enabled: false,
            }];
            assert_eq!(tab.filter_expr(), None, "disabled filters add no child");
        }
    }

    #[test]
    fn issue10_null_rows_pass_only_the_null_tests() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let null_row = row_with(table, "notes", Value::Null);
        for op in FilterOp::ALL {
            let filter = Filter {
                column: "notes".into(),
                op,
                value: "x".into(),
                value2: "y".into(),
                enabled: true,
            };
            let expected = op == FilterOp::IsNull;
            assert_eq!(
                admits(table, &filter.expression(), &null_row),
                expected,
                "{op:?} on NULL",
            );
        }
    }

    #[test]
    fn issue10_negated_filters_do_not_admit_null() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let null_row = row_with(table, "notes", Value::Null);
        let not_contains = Filter {
            column: "notes".into(),
            op: FilterOp::NotContains,
            value: "gift".into(),
            value2: String::new(),
            enabled: true,
        };
        assert!(!admits(table, &not_contains.expression(), &null_row));
    }

    #[test]
    fn issue10_contains_escapes_wildcards_and_matches_them_literally() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let filter = Filter {
            column: "notes".into(),
            op: FilterOp::Contains,
            value: "a_b%".into(),
            value2: String::new(),
            enabled: true,
        };
        let sql = filter.to_sql();
        assert!(sql.contains(r"\_") && sql.contains(r"\%"), "{sql}");
        let expr = filter.expression();
        assert!(admits(
            table,
            &expr,
            &row_with(table, "notes", Value::Text("xa_b%y".into()))
        ));
        assert!(!admits(
            table,
            &expr,
            &row_with(table, "notes", Value::Text("axbc".into()))
        ));
    }

    #[test]
    fn issue10_quotes_in_filter_values_are_doubled_in_sql() {
        let filter = Filter {
            column: "notes".into(),
            op: FilterOp::Eq,
            value: "O'Reilly".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(filter.to_sql(), "notes = 'O''Reilly'");
        let odd = Expr::Leaf(Predicate {
            column: "Odd\"name".into(),
            op: PredicateOp::IsNull,
        });
        assert_eq!(odd.to_sql(), r#""Odd""name" IS NULL"#);
    }

    fn sql_of(column: &str, op: FilterOp, value: &str) -> String {
        filter_on(column, op, value).to_sql()
    }

    fn notes_admit(expr: &Expr, text: &str) -> bool {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        admits(
            table,
            expr,
            &row_with(table, "notes", Value::Text(text.into())),
        )
    }

    #[test]
    fn issue10_filter_sql_escapes_literals_and_patterns() {
        assert_eq!(
            sql_of("notes", FilterOp::Contains, r"a\b"),
            r"notes ILIKE '%a\\b%' ESCAPE '\'"
        );
        assert_eq!(
            sql_of("notes", FilterOp::StartsWith, r"50%_\"),
            r"notes ILIKE '50\%\_\\%' ESCAPE '\'"
        );
        assert_eq!(
            sql_of("notes", FilterOp::EndsWith, r"%_\"),
            r"notes ILIKE '%\%\_\\' ESCAPE '\'"
        );
        assert_eq!(
            sql_of("notes", FilterOp::Contains, "it's"),
            r"notes ILIKE '%it''s%' ESCAPE '\'"
        );
        assert_eq!(
            sql_of("notes", FilterOp::NotContains, "a_b"),
            r"notes NOT ILIKE '%a\_b%' ESCAPE '\'"
        );
        assert_eq!(
            sql_of("notes", FilterOp::NotIn, "a'b, c"),
            "notes NOT IN ('a''b', 'c')"
        );
        assert_eq!(
            sql_of("notes", FilterOp::Regex, "it's"),
            "notes ~ 'it''s'",
            "an apostrophe in a regex is doubled"
        );
        assert_eq!(
            sql_of("notes", FilterOp::Regex, r"\d+"),
            r"notes ~ '\d+'",
            "a regex keeps its backslash"
        );
        assert_eq!(
            sql_of("notes", FilterOp::Eq, r"back\slash"),
            r"notes = 'back\slash'"
        );
        let not_regex = not(leaf("notes", PredicateOp::Regex("it's".into())));
        assert_eq!(not_regex.to_sql(), "notes !~ 'it''s'");
        assert_eq!(
            parsed_filter("SELECT * FROM orders WHERE notes LIKE 'a_b%'").to_sql(),
            "notes LIKE 'a_b%'",
            "a query-editor pattern prints as typed"
        );

        let backslash = filter_on("notes", FilterOp::Contains, r"a\b");
        assert!(notes_admit(&backslash, r"xa\by"));
        assert!(!notes_admit(&backslash, "ab"));
        assert!(!notes_admit(&backslash, "a b"));
        let starts = filter_on("notes", FilterOp::StartsWith, "100%");
        assert!(notes_admit(&starts, "100% off"));
        assert!(!notes_admit(&starts, "1000"));
        assert!(!notes_admit(&starts, "x100%"));
        let ends = filter_on("notes", FilterOp::EndsWith, "_x");
        assert!(notes_admit(&ends, "a_x"));
        assert!(!notes_admit(&ends, "abx"));
        assert!(!notes_admit(&ends, "a_xy"));
        let trailing_slash = filter_on("notes", FilterOp::StartsWith, r"a\");
        assert!(notes_admit(&trailing_slash, r"a\z"));
        assert!(!notes_admit(&trailing_slash, "az"));

        let school = filter_on("notes", FilterOp::Contains, "ÉCOLE");
        assert!(notes_admit(&school, "école"));
        assert!(notes_admit(&school, "Rue de l'École"));
        assert!(!notes_admit(&school, "ecole"));
        let snowman = filter_on("notes", FilterOp::Eq, "naïve ☃");
        assert!(notes_admit(&snowman, "NAÏVE ☃"));
        assert_eq!(snowman.to_sql(), "notes = 'naïve ☃'");
        let sharp_s = filter_on("notes", FilterOp::Contains, "STRASSE");
        assert!(
            !notes_admit(&sharp_s, "straße"),
            "text is lowercased, not case-folded"
        );
        assert!(notes_admit(
            &filter_on("notes", FilterOp::Contains, "STRAßE"),
            "straße"
        ));
    }

    #[test]
    fn issue10_nested_connectives_are_parenthesized_in_sql() {
        let eq = |column: &str, v: &str| leaf(column, PredicateOp::Eq(v.into()));
        let between = Filter {
            column: "order_number".into(),
            op: FilterOp::Between,
            value: "1".into(),
            value2: "2".into(),
            enabled: true,
        };
        assert_eq!(
            between.to_sql(),
            "order_number >= '1' AND order_number <= '2'"
        );
        let any = Expr::Any(vec![between.expression(), eq("status", "x")]);
        assert_eq!(
            any.to_sql(),
            "(order_number >= '1' AND order_number <= '2') OR status = 'x'",
            "Between stays one child of the outer Any"
        );
        assert_eq!(
            not(Expr::Any(vec![eq("a", "x"), eq("b", "y")])).to_sql(),
            "NOT (a = 'x' OR b = 'y')"
        );
        assert_eq!(
            not(Expr::All(vec![eq("a", "x"), eq("b", "y")])).to_sql(),
            "NOT (a = 'x' AND b = 'y')"
        );
        assert_eq!(
            Expr::All(vec![
                Expr::Any(vec![eq("a", "x"), eq("b", "y")]),
                eq("c", "z")
            ])
            .to_sql(),
            "(a = 'x' OR b = 'y') AND c = 'z'",
            "an Any inside an All keeps its parentheses"
        );
        assert_eq!(
            Expr::Any(vec![
                eq("a", "x"),
                not(Expr::Any(vec![eq("b", "y"), eq("c", "z")]))
            ])
            .to_sql(),
            "a = 'x' OR NOT (b = 'y' OR c = 'z')"
        );
        assert_eq!(not(not(eq("a", "x"))).to_sql(), "NOT (NOT (a = 'x'))");
        assert_eq!(
            not(leaf("a", PredicateOp::IsNull)).to_sql(),
            "NOT (a IS NULL)"
        );
    }

    #[test]
    fn issue10_quoted_operand_unescapes_once() {
        assert_eq!(literal("'O''Reilly'"), "O'Reilly");
        assert_eq!(literal("'plain'"), "plain");
        assert_eq!(literal("42"), "42");
    }

    #[test]
    fn issue10_quoted_identifier_unescapes_doubled_quotes() {
        let col = |sql: &str| parsed_filter(sql);
        assert_eq!(
            col(r#"SELECT * FROM orders WHERE "Odd""name" IS NULL"#),
            Expr::All(vec![leaf("Odd\"name", PredicateOp::IsNull)])
        );
        assert_eq!(
            col(r#"SELECT * FROM orders WHERE "a""" IS NULL"#),
            Expr::All(vec![leaf("a\"", PredicateOp::IsNull)])
        );
        assert_eq!(
            col(r#"SELECT * FROM orders WHERE "plain" IS NULL"#),
            Expr::All(vec![leaf("plain", PredicateOp::IsNull)])
        );
    }

    #[test]
    fn issue10_parsed_where_literal_matches_the_unescaped_value() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let filter = parsed_filter("SELECT * FROM orders WHERE notes = 'O''Reilly'");
        assert!(admits(
            table,
            &filter,
            &row_with(table, "notes", Value::Text("O'Reilly".into())),
        ));
        assert!(!admits(
            table,
            &filter,
            &row_with(table, "notes", Value::Text("O''Reilly".into())),
        ));
    }

    #[test]
    fn issue10_sql_like_and_ui_contains_remain_distinct() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let like = parsed_filter("SELECT * FROM orders WHERE notes LIKE 'a_b%'");
        let contains = Filter {
            column: "notes".into(),
            op: FilterOp::Contains,
            value: "a_b%".into(),
            value2: String::new(),
            enabled: true,
        }
        .expression();
        let axbc = row_with(table, "notes", Value::Text("axbc".into()));
        assert!(admits(table, &like, &axbc), "LIKE wildcard `_` matches x");
        assert!(!admits(table, &contains, &axbc), "UI Contains is literal");
    }

    #[test]
    fn issue10_index_scan_needs_a_positive_equality_conjunct() {
        let cat = Catalog::acme_prod();
        let eq = |op| {
            Expr::Leaf(Predicate {
                column: "customer_id".into(),
                op,
            })
        };
        let base = |filter| Select {
            columns: vec!["*".into()],
            schema: Some("public".into()),
            table: "orders".into(),
            filter: Some(filter),
            order: None,
            limit: None,
            count_only: false,
        };
        let conjunction = base(Expr::All(vec![eq(PredicateOp::Eq("abc".into()))]));
        let plan = explain(&cat, &conjunction, false).unwrap();
        let mut lines = vec![];
        plan_text(&plan, 0, &mut lines);
        assert!(lines.iter().any(|l| l.contains("Index Scan")), "{lines:?}");

        let disjunction = base(Expr::Any(vec![
            eq(PredicateOp::Eq("abc".into())),
            eq(PredicateOp::Eq("def".into())),
        ]));
        let plan = explain(&cat, &disjunction, false).unwrap();
        let mut lines = vec![];
        plan_text(&plan, 0, &mut lines);
        assert!(!lines.iter().any(|l| l.contains("Index Scan")), "{lines:?}");
    }

    fn compiles() -> usize {
        REGEX_COMPILES.with(std::cell::Cell::get)
    }

    #[test]
    fn issue10_regex_compiles_once_per_load_not_per_row() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let rows = crate::db::rows(table, 0, 2_000);
        assert_eq!(rows.len(), 2_000, "the scan covers the full sample");
        let expr = filter_on("notes", FilterOp::Regex, "^Gift");

        let start = compiles();
        let prepared = prepare(table, &expr).expect("prepares");
        assert_eq!(compiles() - start, 1, "prepare compiles the pattern once");
        let admitted = rows.iter().filter(|r| prepared.admits(r)).count();
        assert!(
            admitted > 0 && admitted < rows.len(),
            "the pattern selects some rows"
        );
        assert_eq!(compiles() - start, 1, "no row compiles anything");

        let select = |filter| Select {
            columns: vec!["*".into()],
            schema: Some("public".into()),
            table: "orders".into(),
            filter: Some(filter),
            order: None,
            limit: None,
            count_only: false,
        };
        let start = compiles();
        run_select(&cat, &select(expr.clone())).expect("runs");
        assert_eq!(
            compiles() - start,
            1,
            "one load compiles once over 2000 rows"
        );

        let two = Expr::Any(vec![expr.clone(), not(expr)]);
        let start = compiles();
        run_select(&cat, &select(two)).expect("runs");
        assert_eq!(compiles() - start, 2, "one compile per regex leaf");

        let mut tab =
            crate::tabs::TableTab::new(WidgetId::of("issue10-compiles"), &cat, table, false);
        tab.filters = vec![Filter {
            column: "notes".into(),
            op: FilterOp::Regex,
            value: "^Gift".into(),
            value2: String::new(),
            enabled: true,
        }];
        let start = compiles();
        tab.load(&cat);
        assert_eq!(compiles() - start, 1, "a tab load compiles once");
    }

    #[test]
    fn issue10_invalid_pattern_is_reported_not_compiled_per_row() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let expr = Expr::Leaf(Predicate {
            column: "notes".into(),
            op: PredicateOp::Regex("(".into()),
        });
        let err = prepare(table, &expr).unwrap_err();
        assert!(
            err.message.starts_with("invalid regular expression:"),
            "{}",
            err.message
        );
    }

    fn filter_on(column: &str, op: FilterOp, value: &str) -> Expr {
        Filter {
            column: column.into(),
            op,
            value: value.into(),
            value2: String::new(),
            enabled: true,
        }
        .expression()
    }

    #[test]
    fn issue10_numeric_in_uses_the_same_equality_as_eq() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let row = row_with(table, "total_amount", Value::Num(25.9));
        let eq = parsed_filter("SELECT * FROM orders WHERE total_amount = 25.9");
        let list = parsed_filter("SELECT * FROM orders WHERE total_amount IN ('25.9')");
        let padded = parsed_filter("SELECT * FROM orders WHERE total_amount IN ('25.90', '1')");
        assert!(admits(table, &eq, &row), "= 25.9");
        assert!(admits(table, &list, &row), "IN ('25.9')");
        assert!(admits(table, &padded, &row), "IN ('25.90', '1')");
        assert!(
            !admits(
                table,
                &filter_on("total_amount", FilterOp::NotIn, "25.9"),
                &row
            ),
            "NOT IN (25.9)"
        );
    }

    #[test]
    fn issue10_integer_comparison_stays_exact_past_2_pow_53() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let row = row_with(table, "order_number", Value::Int(9_007_199_254_740_993));
        let near = "9007199254740992";
        let same = "9007199254740993";
        assert!(!admits(
            table,
            &filter_on("order_number", FilterOp::Eq, near),
            &row
        ));
        assert!(admits(
            table,
            &filter_on("order_number", FilterOp::Eq, same),
            &row
        ));
        assert!(!admits(
            table,
            &filter_on("order_number", FilterOp::In, near),
            &row
        ));
        assert!(admits(
            table,
            &filter_on("order_number", FilterOp::Gt, near),
            &row
        ));
    }

    #[test]
    fn issue10_ilike_ignores_case_and_like_does_not() {
        let cat = Catalog::acme_prod();
        let table = orders(&cat);
        let row = row_with(table, "notes", Value::Text("gift card".into()));
        let ilike = parsed_filter("SELECT * FROM orders WHERE notes ILIKE 'GIFT%'");
        let like = parsed_filter("SELECT * FROM orders WHERE notes LIKE 'GIFT%'");
        assert!(admits(table, &ilike, &row), "ILIKE matches across case");
        assert!(!admits(table, &like, &row), "LIKE is case-sensitive");
        assert_eq!(ilike.to_sql(), "notes ILIKE 'GIFT%'");
        assert_eq!(like.to_sql(), "notes LIKE 'GIFT%'");
        let lower = parsed_filter("SELECT * FROM orders WHERE notes LIKE 'gift%'");
        assert!(admits(table, &lower, &row));
    }

    #[test]
    fn issue10_reserved_column_names_are_quoted() {
        for name in ["user", "only", "for", "some", "array", "window"] {
            let leaf = Expr::Leaf(Predicate {
                column: name.into(),
                op: PredicateOp::IsNull,
            });
            assert_eq!(leaf.to_sql(), format!("\"{name}\" IS NULL"), "{name}");
        }
    }

    #[test]
    fn issue10_filter_text_literals_use_the_shared_model_policy() {
        for value in ["O'Reilly", "''", "back\\slash", "100%_", "naïve ☃"] {
            let quoted = crate::model::quote_text(value);
            assert_eq!(
                Ok(quoted.clone()),
                crate::model::sql_literal(&Value::Text(value.into()))
            );
            assert_eq!(
                filter_on("notes", FilterOp::Eq, value).to_sql(),
                format!("notes = {quoted}")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts the tokenizer's core contract: contiguous, non-empty ranges
    /// that cover `src` exactly, with both endpoints on char boundaries.
    fn assert_partition(src: &str) {
        let toks = tokenize(src);
        let mut at = 0;
        for t in &toks {
            assert_eq!(t.start, at, "gap or overlap before {t:?} in {src:?}");
            assert!(t.start < t.end, "empty token {t:?} in {src:?}");
            assert!(
                src.is_char_boundary(t.start) && src.is_char_boundary(t.end),
                "token {t:?} is not on char boundaries in {src:?}"
            );
            at = t.end;
        }
        assert_eq!(at, src.len(), "tokens stop early in {src:?}");
        let joined: String = toks.iter().map(|t| &src[t.start..t.end]).collect();
        assert_eq!(joined, src);
    }

    const UNICODE_CORPUS: &[&str] = &[
        "SELECT é",
        "SELECT é FROM t",
        "SELECT 名 FROM 表 WHERE 名 = '中;文'",
        "SELECT \"é\"\"中\" FROM t",
        "SELECT 💾, 'x''é'; /*中*/",
        "SELECT e\u{301} FROM t",
        "SELECT\u{a0}1\u{2003}FROM t",
        "-- 💾 comment\nSELECT 1",
        "SELECT 'unterminated é",
        "SELECT /* unterminated 中",
        "SELECT \"unterminated 名",
        "💾💾💾",
        "_é1 = 2",
        "",
    ];

    #[test]
    fn issue9_tokens_partition_utf8() {
        for src in UNICODE_CORPUS {
            assert_partition(src);
        }
    }

    #[test]
    fn issue9_every_scalar_prefix_makes_progress() {
        let src = "SELECT é中💾, 'x''é'; /*中*/";
        for (end, _) in src.char_indices().chain([(src.len(), ' ')]) {
            assert_partition(&src[..end]);
        }
    }

    #[test]
    fn issue9_ascii_token_kinds_are_unchanged() {
        let kinds: Vec<TokKind> = tokenize("SELECT id, 1.5 FROM t WHERE x <> NULL")
            .into_iter()
            .filter(|t| t.kind != TokKind::Whitespace)
            .map(|t| t.kind)
            .collect();
        use TokKind::*;
        assert_eq!(
            kinds,
            [
                Keyword, Ident, Punct, Number, Keyword, Ident, Keyword, Ident, Operator, Literal
            ]
        );
    }

    #[test]
    fn issue9_doubled_quotes_stay_one_token() {
        let src = "'it''s' \"a\"\"é\"";
        let toks = tokenize(src);
        assert_eq!(toks.len(), 3);
        assert_eq!(&src[toks[0].start..toks[0].end], "'it''s'");
        assert_eq!(toks[0].kind, TokKind::String);
        assert_eq!(&src[toks[2].start..toks[2].end], "\"a\"\"é\"");
        assert_eq!(toks[2].kind, TokKind::Ident);
    }

    #[test]
    fn issue9_unicode_quotes_comments_segments() {
        let src = "SELECT 'é;中'; -- 💾\nSELECT \"名\";";
        let texts: Vec<&str> = split_statements(src)
            .into_iter()
            .map(|(a, b)| &src[a..b])
            .collect();
        assert_eq!(texts, ["SELECT 'é;中'", "-- 💾\nSELECT \"名\""]);
    }

    #[test]
    fn issue9_unterminated_input_extends_to_eof() {
        let quote = "SELECT 'é;中";
        assert_eq!(split_statements(quote), [(0, quote.len())]);
        let block = "SELECT /* é; 中";
        assert_eq!(split_statements(block), [(0, block.len())]);
    }

    #[test]
    fn issue9_consumers_slice_only_token_boundaries() {
        for src in UNICODE_CORPUS {
            for range in crate::tabs::segment_sql(src) {
                assert!(src.is_char_boundary(range.start));
                assert!(src.is_char_boundary(range.end));
            }
            for (range, _) in crate::tabs::highlight_sql(src) {
                assert!(src.is_char_boundary(range.start));
                assert!(src.is_char_boundary(range.end));
            }
            let _ = parse(src);
        }
    }
}
