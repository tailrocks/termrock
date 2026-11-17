//! The simulated database every read of one connection session shares.
//!
//! Rows are never stored whole. The deterministic seed generator in `db` still
//! produces the baseline rows on demand, and the store keeps only what a save
//! changed on top of it: replaced rows, deleted baseline keys and inserted rows.
//! A table with a million seed rows and three saved edits holds three entries.
//!
//! Row keys name a row for the whole session and never derive from its primary
//! key. A baseline row is `Num(ordinal * 2)` and an inserted row is
//! `Num(counter * 2 + 1)`, so the two domains cannot meet.
//!
//! A save is two steps. [`SessionStore::prepare_commit`] checks everything and
//! builds the complete result without touching the store, and
//! [`SessionStore::apply_prepared`] installs exactly that result with nothing
//! left that can fail. The store simulates storage only: foreign keys, unique
//! indexes and CHECK constraints are not enforced, and nothing is written to
//! disk. A new connection starts from the seed again.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use termrock::core::id::ItemKey;
use termrock::widgets::grid::{CellValue, CommittedRow, GridChangeSet, GridCommitReceipt};

use crate::db::{Catalog, ColType, Column, Table, Value};
use crate::tabs::to_cell;

/// How many baseline rows a read looks at. A filter over a larger table is
/// answered from this sample and its total is an estimate.
pub const SAMPLE_CAP: usize = 2_000;

/// The frozen clock of the simulator. It is not wall-clock time.
pub const SIMULATOR_TIMESTAMP: &str = "2025-06-01 12:30:00+00";

const MAX_INSERTS: u64 = 0xffff_ffff_ffff;

static NEXT_STORE: AtomicU64 = AtomicU64::new(1);

/// One table of one database. The connection generation is not part of it: it
/// belongs to the [`SessionStore`] that holds the table.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TableId {
    pub database: String,
    pub schema: String,
    pub table: String,
}

impl TableId {
    pub fn new(database: &str, schema: &str, table: &str) -> Self {
        Self {
            database: database.to_owned(),
            schema: schema.to_owned(),
            table: table.to_owned(),
        }
    }
}

/// Identity of one store. Two stores of the same connection generation and
/// table revision are still different stores, so a plan made for one cannot be
/// applied to the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoreInstanceId(u64);

impl StoreInstanceId {
    fn allocate() -> Self {
        let mut current = NEXT_STORE.load(Ordering::Relaxed);
        loop {
            let next = current
                .checked_add(1)
                .expect("store instance ids exhausted");
            match NEXT_STORE.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Self(current),
                Err(seen) => current = seen,
            }
        }
    }
}

/// The time `now()` resolves to. Changing it is explicit configuration, never a
/// side effect of preparing or applying a save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitClock {
    timestamp: String,
}

impl CommitClock {
    #[cfg(test)]
    pub fn new(timestamp: &str) -> Result<Self, StoreError> {
        validate_timestamp(timestamp)?;
        Ok(Self {
            timestamp: timestamp.to_owned(),
        })
    }

    pub fn simulator() -> Self {
        Self {
            timestamp: SIMULATOR_TIMESTAMP.to_owned(),
        }
    }
}

/// The clock reading one save was reviewed with. Every `now()` of the save
/// resolves to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitContext {
    timestamp: String,
}

impl CommitContext {
    pub fn new(timestamp: &str) -> Self {
        Self {
            timestamp: timestamp.to_owned(),
        }
    }

    #[cfg(test)]
    pub fn timestamp(&self) -> &str {
        &self.timestamp
    }
}

/// Why the store refused a save. A refusal changes nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The plan was made by another store.
    WrongStore,
    /// The plan was made in another connection session.
    WrongSession {
        expected: u64,
        actual: u64,
    },
    /// The table changed after the rows were read.
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    /// The grid's columns are not the table's columns.
    SchemaChanged,
    /// A row the save names is not in the table any more.
    MissingRow(ItemKey),
    /// A row the save names holds other values than the grid read.
    BeforeImageChanged(ItemKey),
    /// A row is named twice.
    DuplicateRow(ItemKey),
    GeneratedColumn {
        key: ItemKey,
        column: String,
    },
    WrongKind {
        key: ItemKey,
        column: String,
        expected: ColType,
    },
    NonFinite {
        key: ItemKey,
        column: String,
    },
    /// A value a column cannot be left empty with, for a column that has no
    /// default: an explicit `NULL`, or `DEFAULT`, has nothing to store.
    Required {
        key: ItemKey,
        column: String,
    },
    /// An explicit `NULL` for a column that cannot be empty but has a default,
    /// which `DEFAULT` stores instead.
    NullNotAllowed {
        key: ItemKey,
        column: String,
    },
    NotInEnum {
        key: ItemKey,
        column: String,
        value: String,
    },
    UnsupportedDefault {
        key: ItemKey,
        column: String,
        expression: String,
    },
    SequenceExhausted {
        key: ItemKey,
        column: String,
    },
    KeysExhausted,
    RevisionExhausted,
    BadTimestamp(String),
}

impl StoreError {
    /// The captured row the refusal belongs to, when it names one.
    pub fn row(&self) -> Option<ItemKey> {
        match self {
            Self::MissingRow(key) | Self::BeforeImageChanged(key) | Self::DuplicateRow(key) => {
                Some(*key)
            }
            Self::GeneratedColumn { key, .. }
            | Self::WrongKind { key, .. }
            | Self::NonFinite { key, .. }
            | Self::Required { key, .. }
            | Self::NullNotAllowed { key, .. }
            | Self::NotInEnum { key, .. }
            | Self::UnsupportedDefault { key, .. }
            | Self::SequenceExhausted { key, .. } => Some(*key),
            _ => None,
        }
    }

    /// Whether the refusal means the rows on screen are older than the table.
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::StaleRevision { .. } | Self::MissingRow(_) | Self::BeforeImageChanged(_)
        )
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongStore => f.write_str("the save was prepared for another session"),
            Self::WrongSession { .. } => f.write_str("the save belongs to an earlier connection"),
            Self::StaleRevision { .. } => {
                f.write_str("Source changed; save requires reload/review")
            }
            Self::SchemaChanged => f.write_str("the table's columns changed"),
            Self::MissingRow(_) => {
                f.write_str("Source changed; a row of this save is no longer in the table")
            }
            Self::BeforeImageChanged(_) => {
                f.write_str("Source changed; a row of this save holds other values now")
            }
            Self::DuplicateRow(_) => f.write_str("a row is part of the save twice"),
            Self::GeneratedColumn { column, .. } => {
                write!(f, "{column} is generated and cannot be changed")
            }
            Self::WrongKind {
                column, expected, ..
            } => write!(f, "{column} must be {}", expected.sql()),
            Self::NonFinite { column, .. } => write!(f, "{column} must be a finite number"),
            Self::Required { column, .. } => write!(f, "{column} is required"),
            Self::NullNotAllowed { column, .. } => {
                write!(
                    f,
                    "{column} is required (type DEFAULT to use the column default)"
                )
            }
            Self::NotInEnum { column, value, .. } => {
                write!(f, "{value:?} is not a value of {column}")
            }
            Self::UnsupportedDefault {
                column, expression, ..
            } => write!(f, "the default of {column} ({expression}) is not supported"),
            Self::SequenceExhausted { column, .. } => write!(f, "{column} ran out of values"),
            Self::KeysExhausted => f.write_str("the table cannot hold more new rows"),
            Self::RevisionExhausted => f.write_str("the table cannot record more saves"),
            Self::BadTimestamp(text) => write!(f, "{text:?} is not a timestamp"),
        }
    }
}

impl std::error::Error for StoreError {}

#[derive(Debug, Clone)]
struct TableStore {
    seed_count: usize,
    revision: u64,
    updated: HashMap<ItemKey, Vec<Value>>,
    deleted: BTreeSet<ItemKey>,
    inserted: BTreeMap<ItemKey, Vec<Value>>,
    next_insert: u64,
    sequences: BTreeMap<usize, i64>,
}

impl TableStore {
    fn new(seed_count: usize) -> Self {
        Self {
            seed_count,
            revision: 0,
            updated: HashMap::new(),
            deleted: BTreeSet::new(),
            inserted: BTreeMap::new(),
            next_insert: 1,
            sequences: BTreeMap::new(),
        }
    }
}

/// What a read of one table sees: the sampled baseline rows with every saved
/// change applied, then every inserted row.
#[derive(Debug, Clone)]
pub struct Scan {
    pub rows: Vec<(ItemKey, Vec<Value>)>,
    /// Baseline rows the sample holds after deletions.
    pub sampled_baseline: usize,
    /// Baseline rows the table holds after deletions, sampled or not.
    pub live_baseline: usize,
    pub revision: u64,
}

impl Scan {
    /// How many rows `matched` selects in the whole table. The inserted rows
    /// are all in the scan, so they are counted exactly. The baseline rows are
    /// scaled from the sample and the result is an estimate.
    pub fn estimate_total(&self, matched: &[ItemKey]) -> usize {
        let inserted = matched.iter().filter(|k| is_inserted_key(**k)).count();
        estimate_total(
            self.live_baseline,
            self.sampled_baseline,
            matched.len() - inserted,
            inserted,
        )
    }
}

/// The matching share of the sampled baseline scaled to the live baseline, plus
/// the inserted matches. An empty sample estimates no baseline rows.
pub fn estimate_total(
    live_baseline: usize,
    sampled_baseline: usize,
    matched_baseline: usize,
    matched_inserted: usize,
) -> usize {
    let baseline = if sampled_baseline == 0 {
        0
    } else {
        ((matched_baseline as f64 / sampled_baseline as f64) * live_baseline as f64).round()
            as usize
    };
    baseline.saturating_add(matched_inserted)
}

pub fn is_inserted_key(key: ItemKey) -> bool {
    matches!(key, ItemKey::Num(n) if n % 2 == 1)
}

fn baseline_key(ordinal: usize) -> ItemKey {
    ItemKey::num(ordinal as u64 * 2)
}

fn inserted_key(counter: u64) -> Option<ItemKey> {
    counter
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .map(ItemKey::num)
}

/// Everything one connection session knows about its tables' rows.
#[derive(Debug)]
pub struct SessionStore {
    instance: StoreInstanceId,
    session_generation: u64,
    tables: HashMap<TableId, TableStore>,
    commit_clock: CommitClock,
}

impl SessionStore {
    pub fn new(session_generation: u64) -> Self {
        Self::with_clock(session_generation, CommitClock::simulator())
    }

    pub fn with_clock(session_generation: u64, commit_clock: CommitClock) -> Self {
        Self {
            instance: StoreInstanceId::allocate(),
            session_generation,
            tables: HashMap::new(),
            commit_clock,
        }
    }

    #[cfg(test)]
    pub fn session_generation(&self) -> u64 {
        self.session_generation
    }

    /// The clock reading a save captures when it is reviewed.
    pub fn commit_context(&self) -> CommitContext {
        CommitContext::new(&self.commit_clock.timestamp)
    }

    /// How many saves the table has accepted this session.
    pub fn revision(&self, id: &TableId) -> u64 {
        self.tables.get(id).map_or(0, |t| t.revision)
    }

    fn seed_count(&self, id: &TableId, table: &Table) -> usize {
        self.tables
            .get(id)
            .map_or(table.row_count, |t| t.seed_count)
    }

    /// The exact number of rows the table holds now.
    pub fn row_count(&self, id: &TableId, table: &Table) -> usize {
        match self.tables.get(id) {
            None => table.row_count,
            Some(t) => {
                (t.seed_count.saturating_sub(t.deleted.len())).saturating_add(t.inserted.len())
            }
        }
    }

    /// One live row, or `None` for a key the table does not hold.
    pub fn row(&self, id: &TableId, table: &Table, key: ItemKey) -> Option<Vec<Value>> {
        let store = self.tables.get(id);
        let ItemKey::Num(n) = key else {
            return None;
        };
        if n % 2 == 1 {
            return store?.inserted.get(&key).cloned();
        }
        let ordinal = usize::try_from(n / 2).ok()?;
        if ordinal >= self.seed_count(id, table) {
            return None;
        }
        if let Some(store) = store {
            if store.deleted.contains(&key) {
                return None;
            }
            if let Some(row) = store.updated.get(&key) {
                return Some(row.clone());
            }
        }
        crate::db::rows(table, ordinal, 1).pop()
    }

    /// The rows a query reads: the bounded baseline sample with saved changes
    /// applied, then every inserted row.
    pub fn scan(&self, id: &TableId, table: &Table) -> Scan {
        let store = self.tables.get(id);
        let seed = self.seed_count(id, table);
        let sample = seed.min(SAMPLE_CAP);
        let mut rows = Vec::with_capacity(sample);
        for (ordinal, row) in crate::db::rows(table, 0, sample).into_iter().enumerate() {
            let key = baseline_key(ordinal);
            match store {
                Some(s) if s.deleted.contains(&key) => {}
                Some(s) => rows.push((key, s.updated.get(&key).cloned().unwrap_or(row))),
                None => rows.push((key, row)),
            }
        }
        let sampled_baseline = rows.len();
        if let Some(s) = store {
            rows.extend(s.inserted.iter().map(|(k, v)| (*k, v.clone())));
        }
        Scan {
            rows,
            sampled_baseline,
            live_baseline: seed.saturating_sub(store.map_or(0, |s| s.deleted.len())),
            revision: self.revision(id),
        }
    }

    /// Checks `changes` against the table as it is now and builds the complete
    /// result of applying them. The store is not touched, so any error leaves it
    /// exactly as it was.
    pub fn prepare_commit(
        &self,
        table: &Table,
        id: &TableId,
        expected_revision: u64,
        source_columns: &[(String, ColType)],
        changes: &GridChangeSet,
        context: &CommitContext,
    ) -> Result<PreparedCommit, StoreError> {
        validate_timestamp(&context.timestamp)?;
        let revision = self.revision(id);
        if revision != expected_revision {
            return Err(StoreError::StaleRevision {
                expected: expected_revision,
                actual: revision,
            });
        }
        let authoritative: Vec<(String, ColType)> = table
            .columns
            .iter()
            .map(|c| (c.name.clone(), c.ty))
            .collect();
        if source_columns != authoritative.as_slice() {
            return Err(StoreError::SchemaChanged);
        }
        let next_revision = revision
            .checked_add(1)
            .ok_or(StoreError::RevisionExhausted)?;
        let store = self.tables.get(id);
        let seed_count = self.seed_count(id, table);

        let mut named = HashSet::new();
        for key in changes
            .updated()
            .iter()
            .map(|r| r.key())
            .chain(changes.deleted().iter().map(|r| r.key()))
        {
            if !named.insert(key) {
                return Err(StoreError::DuplicateRow(key));
            }
        }

        let mut resolver = Resolver {
            columns: &table.columns,
            seed_count,
            now: &context.timestamp,
            sequences: store.map(|s| s.sequences.clone()).unwrap_or_default(),
        };
        let mut receipt = Vec::new();
        let mut updates = Vec::new();
        for row in changes.updated() {
            let key = row.key();
            let stored = self
                .row(id, table, key)
                .ok_or(StoreError::MissingRow(key))?;
            check_before(key, &stored, row.before())?;
            let mut values = stored;
            for cell in row.cells() {
                let column = table
                    .columns
                    .get(cell.column())
                    .ok_or(StoreError::SchemaChanged)?;
                if column.generated {
                    return Err(StoreError::GeneratedColumn {
                        key,
                        column: column.name.clone(),
                    });
                }
                values[cell.column()] = match cell.after() {
                    CellValue::Default => resolver.default(key, cell.column(), None)?,
                    other => convert(key, column, other)?,
                };
            }
            receipt.push(committed(key, key, &values));
            updates.push((key, values));
        }

        let mut deletes = Vec::new();
        for row in changes.deleted() {
            let key = row.key();
            let stored = self
                .row(id, table, key)
                .ok_or(StoreError::MissingRow(key))?;
            check_before(key, &stored, row.before())?;
            deletes.push(key);
        }

        let mut next_insert = store.map_or(1, |s| s.next_insert);
        let mut inserts = Vec::new();
        for row in changes.inserted() {
            let provisional = row.key();
            if row.values().len() != table.columns.len() {
                return Err(StoreError::SchemaChanged);
            }
            let counter = next_insert;
            if counter > MAX_INSERTS {
                return Err(StoreError::KeysExhausted);
            }
            let key = inserted_key(counter).ok_or(StoreError::KeysExhausted)?;
            next_insert = counter + 1;
            let mut values = Vec::with_capacity(table.columns.len());
            for (ci, column) in table.columns.iter().enumerate() {
                let value = match &row.values()[ci] {
                    _ if column.generated => resolver.default(provisional, ci, Some(counter))?,
                    CellValue::Default => resolver.default(provisional, ci, Some(counter))?,
                    other => convert(provisional, column, other)?,
                };
                values.push(value);
            }
            receipt.push(committed(provisional, key, &values));
            inserts.push((key, values));
        }

        Ok(PreparedCommit {
            table: id.clone(),
            store_instance: self.instance,
            session_generation: self.session_generation,
            seed_count,
            expected_revision,
            updates,
            deletes,
            inserts,
            next_insert,
            next_sequences: resolver.sequences,
            next_revision,
            receipt: GridCommitReceipt::new(changes, receipt),
        })
    }

    /// Installs a prepared save in the table it names and returns its
    /// acknowledgement. Everything that can fail is checked before the first
    /// change, and nothing after it can.
    pub fn apply_prepared(
        &mut self,
        plan: PreparedCommit,
    ) -> Result<GridCommitReceipt, StoreError> {
        if plan.store_instance != self.instance {
            return Err(StoreError::WrongStore);
        }
        if plan.session_generation != self.session_generation {
            return Err(StoreError::WrongSession {
                expected: self.session_generation,
                actual: plan.session_generation,
            });
        }
        let revision = self.revision(&plan.table);
        if revision != plan.expected_revision {
            return Err(StoreError::StaleRevision {
                expected: plan.expected_revision,
                actual: revision,
            });
        }
        let store = self
            .tables
            .entry(plan.table)
            .or_insert_with(|| TableStore::new(plan.seed_count));
        for (key, values) in plan.updates {
            if is_inserted_key(key) {
                store.inserted.insert(key, values);
            } else {
                store.updated.insert(key, values);
            }
        }
        for key in plan.deletes {
            if is_inserted_key(key) {
                store.inserted.remove(&key);
            } else {
                store.updated.remove(&key);
                store.deleted.insert(key);
            }
        }
        store.inserted.extend(plan.inserts);
        store.next_insert = plan.next_insert;
        store.sequences = plan.next_sequences;
        store.revision = plan.next_revision;
        Ok(plan.receipt)
    }

    #[cfg(test)]
    fn overlay_len(&self, id: &TableId) -> usize {
        self.tables
            .get(id)
            .map_or(0, |t| t.updated.len() + t.deleted.len() + t.inserted.len())
    }
}

/// A save that was checked against one store and is ready to be installed. It
/// names its destination itself, so a caller cannot retarget it.
#[derive(Debug, Clone)]
pub struct PreparedCommit {
    table: TableId,
    store_instance: StoreInstanceId,
    session_generation: u64,
    seed_count: usize,
    expected_revision: u64,
    updates: Vec<(ItemKey, Vec<Value>)>,
    deletes: Vec<ItemKey>,
    inserts: Vec<(ItemKey, Vec<Value>)>,
    next_insert: u64,
    next_sequences: BTreeMap<usize, i64>,
    next_revision: u64,
    receipt: GridCommitReceipt,
}

impl PreparedCommit {
    /// The acknowledgement the store will return once the plan is applied, for
    /// the grid to check before the store changes.
    pub fn receipt(&self) -> &GridCommitReceipt {
        &self.receipt
    }
}

fn committed(key: ItemKey, committed_key: ItemKey, values: &[Value]) -> CommittedRow {
    CommittedRow {
        key,
        committed_key,
        values: values.iter().map(to_cell).collect(),
    }
}

fn check_before(key: ItemKey, stored: &[Value], before: &[CellValue]) -> Result<(), StoreError> {
    let same =
        stored.len() == before.len() && stored.iter().zip(before).all(|(s, b)| to_cell(s) == *b);
    if same {
        Ok(())
    } else {
        Err(StoreError::BeforeImageChanged(key))
    }
}

/// An explicit cell value as the column stores it.
fn convert(key: ItemKey, column: &Column, cell: &CellValue) -> Result<Value, StoreError> {
    let wrong = || StoreError::WrongKind {
        key,
        column: column.name.clone(),
        expected: column.ty,
    };
    let finite = |n: f64| {
        if n.is_finite() {
            Ok(n)
        } else {
            Err(StoreError::NonFinite {
                key,
                column: column.name.clone(),
            })
        }
    };
    Ok(match (cell, column.ty) {
        (CellValue::Default, _) => return Err(wrong()),
        (CellValue::Null, _) => {
            if column.nullable {
                Value::Null
            } else if column.default.is_some() {
                return Err(StoreError::NullNotAllowed {
                    key,
                    column: column.name.clone(),
                });
            } else {
                return Err(StoreError::Required {
                    key,
                    column: column.name.clone(),
                });
            }
        }
        (CellValue::Text(s), ColType::Enum) => {
            if column.enum_values.contains(&s.as_str()) {
                Value::Text(s.clone())
            } else {
                return Err(StoreError::NotInEnum {
                    key,
                    column: column.name.clone(),
                    value: s.clone(),
                });
            }
        }
        (
            CellValue::Text(s),
            ColType::Uuid | ColType::Text | ColType::Timestamp | ColType::Date,
        ) => Value::Text(s.clone()),
        (CellValue::Text(s) | CellValue::Json(s), ColType::Json) => Value::Json(s.clone()),
        (CellValue::Int(i), ColType::Int) => Value::Int(*i),
        (CellValue::Int(i), ColType::Numeric) => Value::Num(finite(*i as f64)?),
        (CellValue::Num(n), ColType::Numeric) => Value::Num(finite(*n)?),
        (CellValue::Num(n), ColType::Int) => {
            finite(*n)?;
            return Err(wrong());
        }
        (CellValue::Bool(b), ColType::Bool) => Value::Bool(*b),
        _ => return Err(wrong()),
    })
}

/// Resolves the defaults of one save. A sequence advances only the tentative
/// copy it owns, which the plan installs when it is applied.
struct Resolver<'a> {
    columns: &'a [Column],
    seed_count: usize,
    now: &'a str,
    sequences: BTreeMap<usize, i64>,
}

impl Resolver<'_> {
    /// The value column `ci` takes when the row says `DEFAULT`. `counter` is the
    /// inserted-row counter the row is allocated, and `None` for an update.
    fn default(
        &mut self,
        key: ItemKey,
        ci: usize,
        counter: Option<u64>,
    ) -> Result<Value, StoreError> {
        let column = &self.columns[ci];
        let Some(expression) = column.default.as_deref().map(str::trim) else {
            return if column.nullable {
                Ok(Value::Null)
            } else {
                Err(StoreError::Required {
                    key,
                    column: column.name.clone(),
                })
            };
        };
        let unsupported = || StoreError::UnsupportedDefault {
            key,
            column: column.name.clone(),
            expression: expression.to_owned(),
        };
        match (expression, column.ty) {
            ("gen_random_uuid()", ColType::Uuid) => {
                let n = counter.ok_or_else(unsupported)?;
                Ok(Value::Text(format!("00000000-0000-8000-8000-{n:012x}")))
            }
            ("now()", ColType::Timestamp) => Ok(Value::Text(self.now.to_owned())),
            ("identity", ColType::Int) => self.next_value(key, ci, 1),
            ("nextval('orders_number_seq')", ColType::Int) => self.next_value(key, ci, 10_000),
            (literal, ty) => {
                literal_default(literal, ty, &column.enum_values).ok_or_else(unsupported)
            }
        }
    }

    /// Hands out the next value of a sequence that starts `offset` after the
    /// seed row count, reserving it before it is accepted.
    fn next_value(&mut self, key: ItemKey, ci: usize, offset: i64) -> Result<Value, StoreError> {
        let exhausted = || StoreError::SequenceExhausted {
            key,
            column: self.columns[ci].name.clone(),
        };
        let initial = i64::try_from(self.seed_count)
            .ok()
            .and_then(|n| n.checked_add(offset))
            .ok_or_else(exhausted)?;
        let current = self.sequences.get(&ci).copied().unwrap_or(initial);
        let next = current.checked_add(1).ok_or_else(exhausted)?;
        self.sequences.insert(ci, next);
        Ok(Value::Int(current))
    }
}

fn literal_default(literal: &str, ty: ColType, enum_values: &[&'static str]) -> Option<Value> {
    if let Some(inner) = literal
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        let text = inner.replace("''", "'");
        return match ty {
            ColType::Text | ColType::Uuid | ColType::Timestamp | ColType::Date => {
                Some(Value::Text(text))
            }
            ColType::Enum => enum_values
                .contains(&text.as_str())
                .then_some(Value::Text(text)),
            ColType::Json => Some(Value::Json(text)),
            _ => None,
        };
    }
    match ty {
        ColType::Bool => match literal {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        ColType::Int => literal.parse().ok().map(Value::Int),
        ColType::Numeric => literal
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(Value::Num),
        _ => None,
    }
}

/// `YYYY-MM-DD HH:MM:SS` followed by a UTC offset (`+00`, `-05:30` or `Z`).
fn validate_timestamp(text: &str) -> Result<(), StoreError> {
    let bad = || StoreError::BadTimestamp(text.to_owned());
    let b = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<u32> {
        let part = b.get(range)?;
        part.iter()
            .all(u8::is_ascii_digit)
            .then(|| part.iter().fold(0, |n, d| n * 10 + u32::from(*d - b'0')))
    };
    let shape = b.len() >= 20
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b' '
        && b[13] == b':'
        && b[16] == b':';
    if !shape {
        return Err(bad());
    }
    let (month, day) = (
        digits(5..7).ok_or_else(bad)?,
        digits(8..10).ok_or_else(bad)?,
    );
    let (hour, minute, second) = (
        digits(11..13).ok_or_else(bad)?,
        digits(14..16).ok_or_else(bad)?,
        digits(17..19).ok_or_else(bad)?,
    );
    digits(0..4).ok_or_else(bad)?;
    let offset_ok = match &text[19..] {
        "Z" => true,
        rest => {
            let mut chars = rest.chars();
            matches!(chars.next(), Some('+' | '-'))
                && match chars.as_str().len() {
                    2 => digits(20..22).is_some_and(|h| h < 24),
                    5 => {
                        b[22] == b':'
                            && digits(20..22).is_some_and(|h| h < 24)
                            && digits(23..25).is_some_and(|m| m < 60)
                    }
                    _ => false,
                }
        }
    };
    if (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60
        && offset_ok
    {
        Ok(())
    } else {
        Err(bad())
    }
}

/// The catalog and the store a read goes through together.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    pub catalog: &'a Catalog,
    pub store: &'a SessionStore,
}

impl Source<'_> {
    pub fn table_id(&self, table: &Table) -> TableId {
        TableId::new(&self.catalog.database, &table.schema, &table.name)
    }

    /// The exact number of rows `table` holds in this session.
    pub fn row_count(&self, table: &Table) -> usize {
        self.store.row_count(&self.table_id(table), table)
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
