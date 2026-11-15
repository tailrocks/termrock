//! Deterministic work counters for collection costs. Compiled for unit tests
//! only; #56 consolidates counters into one metrics module.
//!
//! Counters are per thread and count units of work (rows enumerated, payload
//! rows copied, comparisons), never time. A test reads a [`Work`] snapshot
//! before and after an operation and asserts on the difference.

use std::cell::Cell;

/// One kind of counted work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counter {
    /// Source rows enumerated while validating or reconciling a provider.
    SourceRows,
    /// Authoritative keys inserted into a key index.
    IndexBuilds,
    /// Key resolutions: one per map lookup or per comparison of a linear search.
    JoinProbes,
    /// Rows measured for an aggregate width.
    WidthRows,
    /// Visible paint records prepared for a deferred overlay.
    SnapshotRows,
    /// Selectable projections built from a submitted result source.
    ProjectionRebuilds,
    /// Master rows handed to a paint callback.
    VisibleRowsPainted,
    /// Option payload rows copied by cloning a whole `Select`.
    OptionClones,
    /// Labels read from a `String` option slice.
    LabelVisits,
}

const COUNTERS: usize = Counter::LabelVisits as usize + 1;

thread_local! {
    static CELLS: [Cell<u64>; COUNTERS] = const { [const { Cell::new(0) }; COUNTERS] };
}

/// Adds `n` units of `counter` on the current thread.
pub fn add(counter: Counter, n: usize) {
    CELLS.with(|cells| {
        let cell = &cells[counter as usize];
        cell.set(cell.get().saturating_add(n as u64));
    });
}

/// A snapshot of every counter on the current thread.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work {
    pub source_rows: u64,
    pub index_builds: u64,
    pub join_probes: u64,
    pub width_rows: u64,
    pub snapshot_rows: u64,
    pub projection_rebuilds: u64,
    pub visible_rows_painted: u64,
    pub option_clones: u64,
    pub label_visits: u64,
}

impl Work {
    /// Reads the current thread's counters.
    pub fn now() -> Self {
        CELLS.with(|c| Self {
            source_rows: c[Counter::SourceRows as usize].get(),
            index_builds: c[Counter::IndexBuilds as usize].get(),
            join_probes: c[Counter::JoinProbes as usize].get(),
            width_rows: c[Counter::WidthRows as usize].get(),
            snapshot_rows: c[Counter::SnapshotRows as usize].get(),
            projection_rebuilds: c[Counter::ProjectionRebuilds as usize].get(),
            visible_rows_painted: c[Counter::VisibleRowsPainted as usize].get(),
            option_clones: c[Counter::OptionClones as usize].get(),
            label_visits: c[Counter::LabelVisits as usize].get(),
        })
    }

    /// Work done since `before`.
    pub fn since(self, before: Self) -> Self {
        Self {
            source_rows: self.source_rows - before.source_rows,
            index_builds: self.index_builds - before.index_builds,
            join_probes: self.join_probes - before.join_probes,
            width_rows: self.width_rows - before.width_rows,
            snapshot_rows: self.snapshot_rows - before.snapshot_rows,
            projection_rebuilds: self.projection_rebuilds - before.projection_rebuilds,
            visible_rows_painted: self.visible_rows_painted - before.visible_rows_painted,
            option_clones: self.option_clones - before.option_clones,
            label_visits: self.label_visits - before.label_visits,
        }
    }

    /// Runs `f` and returns its result with the work it did.
    pub fn measure<T>(f: impl FnOnce() -> T) -> (T, Self) {
        let before = Self::now();
        let out = f();
        (out, Self::now().since(before))
    }
}
