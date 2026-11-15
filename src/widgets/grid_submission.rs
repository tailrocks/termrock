//! Locking a grid's draft while one captured save is reviewed or in flight.
//!
//! A host captures [`DataGrid::snapshot_changes`] once, shows exactly that to
//! the user, and asks the grid to [`begin_submission`](DataGrid::begin_submission)
//! it. The grid refuses a snapshot that is not its own current capture, then
//! rejects every draft or source mutation (`Busy`) until the host cancels the
//! submission, completes it with a [`GridCommitReceipt`], or reports a
//! [`GridCommitFailure`]. Navigation, selection and copy keep working.
//!
//! A [`GridCommitToken`] is the only way to act on the lock. It carries the grid's
//! [`GridInstanceId`], so a token or receipt built for another grid, including a
//! clone of this one, never matches.

use std::collections::{HashMap, HashSet};

use super::{DataGrid, GridChangeSet, GridCommitReceipt, GridInstanceId, GridRevision};
use crate::core::id::ItemKey;

/// Proof that one captured change set is locked in a particular grid. It can
/// only be obtained from [`DataGrid::begin_submission`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridCommitToken {
    origin: GridInstanceId,
    nonce: u64,
    revision: GridRevision,
}

impl GridCommitToken {
    /// The grid that issued this token.
    pub fn origin(&self) -> GridInstanceId {
        self.origin
    }
    /// The grid revision the locked snapshot was captured at.
    pub fn revision(&self) -> GridRevision {
        self.revision
    }
}

/// A backend refusal of a submitted save. The draft stays and is unlocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridCommitFailure {
    /// The snapshot row the failure belongs to, when it names one.
    pub key: Option<ItemKey>,
    pub message: String,
}

/// What is wrong with a [`GridCommitReceipt`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptFault {
    /// An inserted or updated snapshot row has no receipt row.
    Missing(ItemKey),
    /// A receipt row names a key that is not an inserted or updated snapshot row.
    Unexpected(ItemKey),
    /// Two receipt rows name the same snapshot key.
    Duplicate(ItemKey),
    /// A receipt row is not as wide as the schema.
    Width {
        key: ItemKey,
        expected: usize,
        actual: usize,
    },
    /// An updated row came back under a different key.
    UpdatedKeyChanged(ItemKey),
    /// A committed key is used by another row of the grid or receipt.
    KeyCollision(ItemKey),
}

/// Why a submission call was refused. A refusal never changes the grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridCommitError {
    /// Another submission is already locked.
    Busy,
    /// A text or choice session is still open; resolve it with
    /// [`DataGrid::finish_edit_session`] before capturing.
    Editing,
    /// The snapshot has no changes.
    Empty,
    /// The snapshot, token or receipt belongs to another grid.
    WrongOrigin,
    /// The grid changed after the snapshot was captured, or the receipt answers
    /// another capture.
    StaleRevision,
    /// No submission is locked, or the token is not the locked one.
    StaleToken,
    /// The receipt does not match the locked snapshot.
    InvalidReceipt(ReceiptFault),
    /// A private counter ran out; the grid is unchanged.
    Exhausted,
}

impl std::fmt::Display for GridCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("Another save is already in progress"),
            Self::Editing => f.write_str("Finish the open cell first"),
            Self::Empty => f.write_str("There are no changes to save"),
            Self::WrongOrigin => f.write_str("This save belongs to another grid"),
            Self::StaleRevision => f.write_str("The rows changed after they were reviewed"),
            Self::StaleToken => f.write_str("This save is no longer the active one"),
            Self::InvalidReceipt(fault) => write!(f, "The save result does not match: {fault:?}"),
            Self::Exhausted => f.write_str("The grid cannot track more changes"),
        }
    }
}

impl std::error::Error for GridCommitError {}

#[derive(Debug)]
struct Locked {
    snapshot: GridChangeSet,
    token: GridCommitToken,
}

/// A grid's submission state. Cloning a grid makes an independent grid, so the
/// clone starts unlocked with its own nonce sequence, like its fresh origin.
#[derive(Debug, Default)]
pub(super) struct Submissions {
    active: Option<Locked>,
    next_nonce: u64,
    failure: Option<String>,
}

impl Clone for Submissions {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl Submissions {
    pub(super) fn is_locked(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn clear_failure(&mut self) {
        self.failure = None;
    }
}

impl DataGrid {
    /// Whether a captured save is reviewed or in flight, so every draft and
    /// source mutation is refused.
    pub fn submission_pending(&self) -> bool {
        self.submission.is_locked()
    }

    /// The message of the last refused save that named no row, until the next
    /// submission or discard.
    pub fn commit_failure(&self) -> Option<&str> {
        self.submission.failure.as_deref()
    }

    /// Locks `snapshot`, which must be this grid's own current capture.
    ///
    /// A live text or choice session is never cancelled or committed here: the
    /// host resolves it with [`finish_edit_session`](Self::finish_edit_session)
    /// before capturing, and an open one is refused with `Editing`.
    pub fn begin_submission(
        &mut self,
        snapshot: GridChangeSet,
    ) -> Result<GridCommitToken, GridCommitError> {
        if self.submission.is_locked() {
            return Err(GridCommitError::Busy);
        }
        if self.is_editing() {
            return Err(GridCommitError::Editing);
        }
        if snapshot.is_empty() {
            return Err(GridCommitError::Empty);
        }
        if snapshot.origin() != self.instance_id() {
            return Err(GridCommitError::WrongOrigin);
        }
        if snapshot.revision() != self.revision()
            || !snapshot.same_capture(&self.snapshot_changes())
        {
            return Err(GridCommitError::StaleRevision);
        }
        let nonce = self.submission.next_nonce;
        let next = nonce.checked_add(1).ok_or(GridCommitError::Exhausted)?;
        let token = GridCommitToken {
            origin: self.instance_id(),
            nonce,
            revision: snapshot.revision(),
        };
        self.submission.next_nonce = next;
        self.submission.failure = None;
        self.submission.active = Some(Locked { snapshot, token });
        Ok(token)
    }

    /// Unlocks the submission `token` names. The draft is untouched.
    pub fn cancel_submission(&mut self, token: GridCommitToken) -> Result<(), GridCommitError> {
        self.locked(token)?;
        self.submission.active = None;
        Ok(())
    }

    fn locked(&self, token: GridCommitToken) -> Result<&Locked, GridCommitError> {
        if token.origin != self.instance_id() {
            return Err(GridCommitError::WrongOrigin);
        }
        match &self.submission.active {
            Some(locked) if locked.token == token => Ok(locked),
            _ => Err(GridCommitError::StaleToken),
        }
    }

    /// Checks `receipt` against the locked snapshot without changing anything,
    /// so a host can validate before it lets an external store mutate. It runs
    /// no parser and makes no grid change. [`complete_submission`] runs the same
    /// check again before it applies the receipt.
    ///
    /// [`complete_submission`]: Self::complete_submission
    pub fn validate_submission_receipt(
        &self,
        token: GridCommitToken,
        receipt: &GridCommitReceipt,
    ) -> Result<(), GridCommitError> {
        let locked = self.locked(token)?;
        if receipt.origin() != self.instance_id() {
            return Err(GridCommitError::WrongOrigin);
        }
        if receipt.revision() != locked.snapshot.revision() {
            return Err(GridCommitError::StaleRevision);
        }
        let snapshot = &locked.snapshot;
        let width = self.columns.len();
        let updated: HashSet<ItemKey> = snapshot.updated().iter().map(|r| r.key()).collect();
        let expected: HashSet<ItemKey> = snapshot
            .inserted()
            .iter()
            .map(|r| r.key())
            .chain(updated.iter().copied())
            .collect();
        let mut seen = HashSet::new();
        let mut committed: HashMap<ItemKey, ItemKey> = HashMap::new();
        for row in &receipt.rows {
            if !expected.contains(&row.key) {
                return Err(fault(ReceiptFault::Unexpected(row.key)));
            }
            if !seen.insert(row.key) {
                return Err(fault(ReceiptFault::Duplicate(row.key)));
            }
            if row.values.len() != width {
                return Err(fault(ReceiptFault::Width {
                    key: row.key,
                    expected: width,
                    actual: row.values.len(),
                }));
            }
            if updated.contains(&row.key) && row.committed_key != row.key {
                return Err(fault(ReceiptFault::UpdatedKeyChanged(row.key)));
            }
            committed.insert(row.key, row.committed_key);
        }
        if let Some(missing) = snapshot
            .inserted()
            .iter()
            .map(|r| r.key())
            .chain(snapshot.updated().iter().map(|r| r.key()))
            .find(|key| !seen.contains(key))
        {
            return Err(fault(ReceiptFault::Missing(missing)));
        }
        let deleted: HashSet<ItemKey> = snapshot.deleted().iter().map(|r| r.key()).collect();
        let mut final_keys = HashSet::new();
        for &key in &self.row_keys {
            if deleted.contains(&key) {
                continue;
            }
            let landed = committed.get(&key).copied().unwrap_or(key);
            if !final_keys.insert(landed) {
                return Err(fault(ReceiptFault::KeyCollision(landed)));
            }
        }
        self.checked_revision(true, self.draft_open())?;
        Ok(())
    }

    fn checked_revision(&self, source: bool, draft: bool) -> Result<GridRevision, GridCommitError> {
        let bump = |v: u64, on: bool| {
            if on {
                v.checked_add(1).ok_or(GridCommitError::Exhausted)
            } else {
                Ok(v)
            }
        };
        Ok(GridRevision {
            source: bump(self.revision().source, source)?,
            draft: bump(self.revision().draft, draft)?,
        })
    }

    /// Ends the locked submission with the backend's answer.
    ///
    /// `Ok(receipt)` is validated completely first; a receipt that does not
    /// match leaves the lock and the draft in place and returns the error. A
    /// valid receipt applies only the captured rows: updated and inserted rows
    /// take the receipt's values, inserted rows take their committed keys,
    /// deleted rows are removed, and the draft and its undo history are cleared.
    /// `Err(failure)` unlocks and keeps the draft, marking the named row or
    /// recording [`commit_failure`](Self::commit_failure).
    pub fn complete_submission(
        &mut self,
        token: GridCommitToken,
        outcome: Result<GridCommitReceipt, GridCommitFailure>,
    ) -> Result<(), GridCommitError> {
        self.locked(token)?;
        match outcome {
            Err(failure) => {
                self.submission.active = None;
                match failure.key.and_then(|key| self.source_index(key)) {
                    Some(src) => {
                        self.row_errors.insert(src, failure.message);
                    }
                    None => self.submission.failure = Some(failure.message),
                }
                Ok(())
            }
            Ok(receipt) => {
                self.validate_submission_receipt(token, &receipt)?;
                let revision = self.checked_revision(true, self.draft_open())?;
                let locked = self
                    .submission
                    .active
                    .take()
                    .ok_or(GridCommitError::StaleToken)?;
                self.apply_receipt(&locked.snapshot, receipt, revision);
                Ok(())
            }
        }
    }

    fn apply_receipt(
        &mut self,
        snapshot: &GridChangeSet,
        receipt: GridCommitReceipt,
        revision: GridRevision,
    ) {
        self.structural_change();
        let landed: HashMap<ItemKey, ItemKey> = receipt
            .rows
            .iter()
            .map(|row| (row.key, row.committed_key))
            .collect();
        let landed_key = |key: ItemKey| landed.get(&key).copied().unwrap_or(key);
        let cursor_key = self
            .view_key(self.cursor.0, self.cursor.1)
            .map(|(key, col)| (landed_key(key), col));
        let anchor_key = self
            .anchor
            .and_then(|(display, col)| self.view_key(display, col))
            .map(|(key, col)| (landed_key(key), col));
        let fallback_key = self.key_after_removal().map(landed_key);
        self.pending = super::PendingChanges::default();
        let mut deleted: Vec<usize> = snapshot
            .deleted()
            .iter()
            .filter_map(|row| self.source_index(row.key()))
            .collect();
        deleted.sort_unstable();
        self.remove_rows(&deleted);
        for row in receipt.rows {
            if let Some(&src) = self.key_to_source.get(&row.key) {
                self.rows[src] = row.values;
                self.row_keys[src] = row.committed_key;
            }
        }
        self.reindex_keys();
        self.undo.clear();
        self.row_errors.clear();
        self.cell_errors.clear();
        self.rebuild_order();
        let survivor = |key: Option<(ItemKey, usize)>| {
            key.filter(|&(key, _)| self.source_index(key).is_some())
        };
        let cursor = survivor(cursor_key).or_else(|| fallback_key.map(|key| (key, self.cursor.1)));
        self.restore_view(cursor, anchor_key);
        self.revision = revision;
        self.scroll.set_content(self.content_rows());
        self.cursor.0 = self.cursor.0.min(self.rows.len().saturating_sub(1));
        self.scroll.ensure_visible(self.cursor.0);
    }
}

fn fault(why: ReceiptFault) -> GridCommitError {
    GridCommitError::InvalidReceipt(why)
}

#[cfg(test)]
#[path = "grid_submission_tests.rs"]
mod tests;
