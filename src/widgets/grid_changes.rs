//! Immutable, normalized snapshots of a [`DataGrid`]'s staged changes.
//!
//! [`PendingChanges`](super::PendingChanges) is the grid's working draft: for an
//! inserted row it holds only the cells a user typed, and the rest of the row
//! lives in the stored base cells. A consumer that serializes a commit must not
//! interpret it. [`DataGrid::snapshot_changes`] resolves the draft once into
//! owned, keyed, complete rows:
//!
//! - an inserted row carries every effective value for the whole schema width;
//! - an updated row carries its complete base row and only the cells whose
//!   effective value differs from the base;
//! - a deleted row carries its complete base row, and its staged cells never
//!   become an update.
//!
//! The three classes are disjoint and listed in source order. A snapshot owns its
//! data, so later edits, sorting or cursor moves never change it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{CellValue, DataGrid, GridRevision, same_edit_value};
use crate::core::id::ItemKey;

static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Identity of one independent grid. Revisions only mean something together
/// with the origin that produced them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridInstanceId(u64);

impl GridInstanceId {
    fn allocate() -> Self {
        let mut current = NEXT_INSTANCE.load(Ordering::Relaxed);
        loop {
            let next = current
                .checked_add(1)
                .expect("DataGrid instance ids exhausted");
            match NEXT_INSTANCE.compare_exchange_weak(
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

/// The grid's own [`GridInstanceId`]. Cloning a grid makes an independent grid
/// holding the same draft, so the clone takes a fresh id instead of sharing one.
#[derive(Debug)]
pub(super) struct GridOrigin(GridInstanceId);

impl GridOrigin {
    pub(super) fn new() -> Self {
        Self(GridInstanceId::allocate())
    }

    pub(super) fn id(&self) -> GridInstanceId {
        self.0
    }
}

impl Clone for GridOrigin {
    fn clone(&self) -> Self {
        Self::new()
    }
}

/// One edited cell: the base value and the value the draft now holds.
#[derive(Debug, Clone, PartialEq)]
pub struct CellChange {
    column: usize,
    before: CellValue,
    after: CellValue,
}

impl CellChange {
    pub fn column(&self) -> usize {
        self.column
    }
    pub fn before(&self) -> &CellValue {
        &self.before
    }
    pub fn after(&self) -> &CellValue {
        &self.after
    }
}

/// A new row with every effective value, `Default` and `Null` kept distinct.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertedRow {
    key: ItemKey,
    values: Vec<CellValue>,
}

impl InsertedRow {
    pub fn key(&self) -> ItemKey {
        self.key
    }
    pub fn values(&self) -> &[CellValue] {
        &self.values
    }
}

/// An existing row with at least one genuinely changed cell.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdatedRow {
    key: ItemKey,
    before: Vec<CellValue>,
    cells: Vec<CellChange>,
}

impl UpdatedRow {
    pub fn key(&self) -> ItemKey {
        self.key
    }
    /// The complete base row, the identity a predicate matches on.
    pub fn before(&self) -> &[CellValue] {
        &self.before
    }
    /// Changed cells in ascending column order.
    pub fn cells(&self) -> &[CellChange] {
        &self.cells
    }
}

/// An existing row marked for deletion.
#[derive(Debug, Clone, PartialEq)]
pub struct DeletedRow {
    key: ItemKey,
    before: Vec<CellValue>,
}

impl DeletedRow {
    pub fn key(&self) -> ItemKey {
        self.key
    }
    /// The complete base row, never the retained staged draft.
    pub fn before(&self) -> &[CellValue] {
        &self.before
    }
}

/// Everything a commit would send, captured at one grid revision.
///
/// `==` follows `f64` rules, so a `NaN` cell never equals itself. Admission
/// checks use [`GridChangeSet::same_capture`] instead.
#[derive(Debug, Clone, PartialEq)]
pub struct GridChangeSet {
    origin: GridInstanceId,
    revision: GridRevision,
    inserted: Vec<InsertedRow>,
    updated: Vec<UpdatedRow>,
    deleted: Vec<DeletedRow>,
}

impl GridChangeSet {
    pub fn origin(&self) -> GridInstanceId {
        self.origin
    }
    pub fn revision(&self) -> GridRevision {
        self.revision
    }
    pub fn inserted(&self) -> &[InsertedRow] {
        &self.inserted
    }
    pub fn updated(&self) -> &[UpdatedRow] {
        &self.updated
    }
    pub fn deleted(&self) -> &[DeletedRow] {
        &self.deleted
    }
    /// `(updated, inserted, deleted)` row counts, matching
    /// [`PendingChanges::counts`](super::PendingChanges::counts).
    pub fn counts(&self) -> (usize, usize, usize) {
        (self.updated.len(), self.inserted.len(), self.deleted.len())
    }
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }

    /// Whether `other` is the same capture bit for bit: same grid, same
    /// revision and identical rows, where a `Num` compares by its bits so `NaN`
    /// payloads and the sign of zero count. This is the equality for admitting
    /// a saved capture; do not use `==` for it.
    pub fn same_capture(&self, other: &Self) -> bool {
        fn same_cells(a: &[CellValue], b: &[CellValue]) -> bool {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| bit_identical(x, y))
        }
        fn same_rows<T>(a: &[T], b: &[T], same: impl Fn(&T, &T) -> bool) -> bool {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y))
        }
        self.origin == other.origin
            && self.revision == other.revision
            && same_rows(&self.inserted, &other.inserted, |a, b| {
                a.key == b.key && same_cells(&a.values, &b.values)
            })
            && same_rows(&self.updated, &other.updated, |a, b| {
                a.key == b.key
                    && same_cells(&a.before, &b.before)
                    && same_rows(&a.cells, &b.cells, |x, y| {
                        x.column == y.column
                            && bit_identical(&x.before, &y.before)
                            && bit_identical(&x.after, &y.after)
                    })
            })
            && same_rows(&self.deleted, &other.deleted, |a, b| {
                a.key == b.key && same_cells(&a.before, &b.before)
            })
    }
}

fn bit_identical(a: &CellValue, b: &CellValue) -> bool {
    match (a, b) {
        (CellValue::Num(x), CellValue::Num(y)) => x.to_bits() == y.to_bits(),
        _ => a == b,
    }
}

/// One accepted row of a commit acknowledgement.
#[derive(Debug, Clone, PartialEq)]
pub struct CommittedRow {
    /// The key the row had in the captured snapshot.
    pub key: ItemKey,
    /// The key the row has once committed. It equals `key` for an update and may
    /// be a source-provided key for a generated insert.
    pub committed_key: ItemKey,
    /// The accepted values at the full schema width.
    pub values: Vec<CellValue>,
}

/// Acknowledgement of one captured [`GridChangeSet`].
///
/// It is data, not a second pending store. A valid receipt has exactly one row
/// for each inserted and updated row of its snapshot, keyed by the snapshot key,
/// and none for a deleted row. The receipt records which grid and revision it
/// answers so a consumer can reject one built for another grid or capture.
/// Coverage, uniqueness and shape checks belong to the submission code that
/// applies it.
#[derive(Debug, Clone, PartialEq)]
pub struct GridCommitReceipt {
    origin: GridInstanceId,
    revision: GridRevision,
    pub rows: Vec<CommittedRow>,
}

impl GridCommitReceipt {
    pub fn new(snapshot: &GridChangeSet, rows: Vec<CommittedRow>) -> Self {
        Self {
            origin: snapshot.origin,
            revision: snapshot.revision,
            rows,
        }
    }
    pub fn origin(&self) -> GridInstanceId {
        self.origin
    }
    pub fn revision(&self) -> GridRevision {
        self.revision
    }
}

#[cfg(test)]
thread_local! {
    static COPIED_CELLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn owned(value: &CellValue) -> CellValue {
    #[cfg(test)]
    COPIED_CELLS.with(|n| n.set(n.get() + 1));
    value.clone()
}

impl DataGrid {
    /// The grid's own identity. A clone has a different one.
    pub fn instance_id(&self) -> GridInstanceId {
        self.origin.id()
    }

    /// Captures the staged changes as owned, complete, source-ordered rows.
    /// Pure: pending cells, errors and history are untouched, and cursor, sort
    /// or selection changes never alter the result.
    pub fn snapshot_changes(&self) -> GridChangeSet {
        let width = self.columns.len();
        let mut set = GridChangeSet {
            origin: self.instance_id(),
            revision: self.revision,
            inserted: Vec::new(),
            updated: Vec::new(),
            deleted: Vec::new(),
        };
        if self.pending.is_empty() {
            return set;
        }
        let mut edited: HashMap<ItemKey, Vec<(usize, &CellValue)>> = HashMap::new();
        for (key, col, value) in self.pending.cells() {
            if col < width {
                edited.entry(key).or_default().push((col, value));
            }
        }
        let base = |src: usize, col: usize| self.rows[src].get(col).unwrap_or(&CellValue::Null);
        let base_row = |src: usize| (0..width).map(|c| owned(base(src, c))).collect();
        for (src, &key) in self.row_keys.iter().enumerate() {
            if self.pending.is_inserted(key) {
                let values = (0..width).map(|c| owned(self.value(src, c))).collect();
                set.inserted.push(InsertedRow { key, values });
            } else if self.pending.is_deleted(key) {
                set.deleted.push(DeletedRow {
                    key,
                    before: base_row(src),
                });
            } else if let Some(cells) = edited.get_mut(&key) {
                cells.sort_unstable_by_key(|(col, _)| *col);
                let cells: Vec<CellChange> = cells
                    .iter()
                    .filter(|(col, after)| !same_edit_value(after, base(src, *col)))
                    .map(|&(col, after)| CellChange {
                        column: col,
                        before: owned(base(src, col)),
                        after: owned(after),
                    })
                    .collect();
                if !cells.is_empty() {
                    set.updated.push(UpdatedRow {
                        key,
                        before: base_row(src),
                        cells,
                    });
                }
            }
        }
        set
    }
}

#[cfg(test)]
#[path = "grid_changeset_tests.rs"]
mod tests;
