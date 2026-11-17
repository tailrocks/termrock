//! Deterministic work counters.
//!
//! A counter records a unit of work (a line segmented, a cell filled, a label
//! lowercased), never elapsed time. Production code marks a work site with
//! [`record_work!`](crate::record_work). Without the `work-metrics` feature
//! the macro expands to an empty block and does not evaluate its arguments, so
//! a disabled build pays no cost. With it, counts land in a thread-local table
//! that a test reads through `snapshot` and [`WorkSnapshot::delta`].
//!
//! Counters are scoped to the executing thread. Work on another thread or in a
//! child process never appears in the calling thread's delta. A zero is
//! therefore not proof that a background task did nothing: collect the worker's
//! own [`WorkDelta`] and combine it with [`WorkDelta::merged`].
//!
//! Only metrics that have a production work site are listed in [`Metric`]. A
//! metric that is not listed is not measured, which is different from zero.

#[cfg(feature = "work-metrics")]
mod state;
#[cfg(feature = "work-metrics")]
pub use state::{measure, record, reset, snapshot, suspended};

pub mod measurement;

#[cfg(feature = "work-metrics")]
pub mod harness;
#[cfg(feature = "work-metrics")]
pub mod report;

/// Whether this build records work. A report states this so a reader can tell
/// a measured zero from a build that measured nothing.
pub const ENABLED: bool = cfg!(feature = "work-metrics");

macro_rules! metrics {
    ($($(#[$doc:meta])* $name:ident => $label:literal,)+) => {
        /// One kind of work. Each variant names the unit it counts.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(usize)]
        pub enum Metric {
            $($(#[$doc])* $name,)+
        }

        impl Metric {
            /// Every metric, in index order.
            pub const ALL: [Metric; METRIC_COUNT] = [$(Metric::$name,)+];

            /// The stable snake_case name used in reports.
            pub const fn name(self) -> &'static str {
                match self {
                    $(Metric::$name => $label,)+
                }
            }
        }
    };
}

metrics! {
    /// Text lines split into graphemes or words by a `TextViewport`.
    SegmentedLines => "segmented_lines",
    /// Segmented lines wrapped to a width by a `TextViewport`.
    ReflowedLines => "reflowed_lines",
    /// Full rebuilds of a `TextViewport` line index.
    IndexRebuilds => "index_rebuilds",
    /// Appends that extended a `TextViewport` line index.
    IndexExtends => "index_extends",
    /// Retention trims that shifted a `TextViewport` line index.
    IndexShifts => "index_shifts",
    /// Text payload bytes copied by a deep clone of retained text.
    PayloadBytesCloned => "payload_bytes_cloned",
    /// Cells allocated for an off-screen `Buffer`, counted at allocation.
    AllocatedBufferCells => "allocated_buffer_cells",
    /// Cells written by a fill loop over an off-screen `Buffer`.
    BufferCellsFilled => "buffer_cells_filled",
    /// Cells copied from one `Buffer` to another.
    BufferCellsCopied => "buffer_cells_copied",
    /// Rows visited by a list, table or grid paint loop.
    VisibleRowsPainted => "visible_rows_painted",
    /// Calls to the shared display-width helper.
    IntrinsicWidthCalls => "intrinsic_width_calls",
    /// Input bytes traversed by those width calls.
    TextBytesMeasured => "text_bytes_measured",
    /// Labels lowercased to test a filter query.
    LabelsNormalized => "labels_normalized",
    /// Bytes of lowercased labels produced by that normalization.
    LabelBytesNormalized => "label_bytes_normalized",
    /// Full rebuilds of a projection (rows derived from source records).
    ProjectionRebuilds => "projection_rebuilds",
    /// Source items visited by a ranking pass.
    RankRowsVisited => "rank_rows_visited",
    /// Retained records visited by a control scheduling traversal. Counted
    /// whether or not a record needs work.
    SchedulingRecordsVisited => "scheduling_records_visited",
    /// Polls a scheduling traversal issued to a live source.
    SchedulingPolls => "scheduling_polls",
    /// Calls to an application-supplied annotation provider (highlighter or
    /// block segmenter) over a whole text.
    AnnotationQueries => "annotation_queries",
    /// Stable-key comparisons made by a linear scan for an item. Picker key
    /// lookups go through a key index and compare nothing, so no widget
    /// records this today; a scan added later must record it.
    IdentityComparisons => "identity_comparisons",
    /// Elements visited by a responsive-fitting loop (chips, breadcrumb,
    /// segments, status bar, key hints, toolbar).
    FitVisits => "fit_visits",
    /// Key comparisons made while ordering a fitter's removal candidates.
    FitCompares => "fit_compares",
    /// Probes of the UTF-8 boundary rollback at the end of a bounded prefix,
    /// one byte examined each.
    PrefixRollbackProbes => "prefix_rollback_probes",
    /// Bytes traversed by a NUL probe over a bounded prefix, stopping at the
    /// first NUL.
    NulBytesProbed => "nul_bytes_probed",
    /// Bytes checked by a UTF-8 validation pass over a bounded prefix.
    Utf8BytesValidated => "utf8_bytes_validated",
    /// Bytes of preview lines passed through control-character sanitizing.
    TextBytesSanitized => "text_bytes_sanitized",
    /// Characters counted to apply a display line cap.
    LineCharsCounted => "line_chars_counted",
    /// Row payloads retrieved from a source: a `ScrollPanel` line read while
    /// it wraps, measures or copies its content, and a terminal grid or scrollback row
    /// read by a `TerminalView` or `TerminalViewport` paint, copy or lookup.
    SourceRowsRead => "source_rows_read",
    /// Bytes handed to a terminal emulator's `feed`, before the feed guard
    /// filters them.
    TerminalBytesFed => "terminal_bytes_fed",
    /// Cells the frame diff handed to the runtime's backend to draw. The diff's
    /// own comparison of the two buffers happens inside `ratatui` and is not
    /// counted.
    DiffCellsVisited => "diff_cells_visited",
    /// Bytes the runtime's backend wrote to its terminal writer, escape
    /// sequences included.
    TerminalBytesWritten => "terminal_bytes_written",
    /// Raw terminal events the input turn took from the reader (`scan_next`),
    /// filtered ones included. The discard of stale events at startup
    /// (`discard_ready`) is not input work and is not counted.
    RawEventsRead => "raw_events_read",
    /// Raw events dropped before they became an input (focus guard, release
    /// and other unhandled kinds).
    RawEventsFiltered => "raw_events_filtered",
    /// Inputs, external messages, resizes and ticks the runtime delivered to
    /// `Application::handle`.
    InputDispatches => "input_dispatches",
    /// Source rows enumerated to validate or reconcile a keyed provider,
    /// walk a keyed tree, or rebuild a filtered option list.
    SourceRowsEnumerated => "source_rows_enumerated",
    /// Authoritative keys inserted into a key index.
    KeyIndexInserts => "key_index_inserts",
    /// Key resolutions through a key index, one per lookup.
    KeyLookups => "key_lookups",
    /// Rows measured to size an aggregate width (list detail, completion and
    /// choice editors).
    WidthRowsMeasured => "width_rows_measured",
    /// Visible paint records prepared for a deferred overlay.
    OverlayRowsSnapshotted => "overlay_rows_snapshotted",
    /// Labels read from a `[String]` row source, one per `label` call.
    LabelsRead => "labels_read",
}

/// Number of [`Metric`] variants.
pub const METRIC_COUNT: usize = Metric::LabelsRead as usize + 1;

/// Raw counts of one thread at one instant, tagged so a delta can reject a
/// snapshot that is not comparable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkSnapshot {
    pub thread: std::thread::ThreadId,
    pub epoch: u64,
    pub counts: [u64; METRIC_COUNT],
}

/// Work done between two comparable snapshots, indexed by [`Metric`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkDelta(pub [u64; METRIC_COUNT]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricsError {
    /// The snapshots came from different threads.
    DifferentThread,
    /// A `reset` ran between the snapshots.
    DifferentEpoch,
    /// A counter is lower in the later snapshot.
    CounterWentBackwards { index: usize },
    /// Merging two deltas overflowed a counter.
    Overflow { index: usize },
}

impl std::fmt::Display for MetricsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DifferentThread => f.write_str("snapshots are from different threads"),
            Self::DifferentEpoch => f.write_str("a reset ran between the snapshots"),
            Self::CounterWentBackwards { index } => {
                write!(f, "counter {} went backwards", Metric::ALL[*index].name())
            }
            Self::Overflow { index } => {
                write!(f, "counter {} overflowed", Metric::ALL[*index].name())
            }
        }
    }
}

impl std::error::Error for MetricsError {}

impl WorkSnapshot {
    /// The work between `before` and `self`. Thread, then epoch, then each
    /// counter is checked: subtraction alone cannot see a reset when the
    /// post-reset count happens to exceed the old one.
    pub fn delta(self, before: WorkSnapshot) -> Result<WorkDelta, MetricsError> {
        if self.thread != before.thread {
            return Err(MetricsError::DifferentThread);
        }
        if self.epoch != before.epoch {
            return Err(MetricsError::DifferentEpoch);
        }
        let mut out = [0; METRIC_COUNT];
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = self.counts[index]
                .checked_sub(before.counts[index])
                .ok_or(MetricsError::CounterWentBackwards { index })?;
        }
        Ok(WorkDelta(out))
    }
}

impl WorkDelta {
    pub const ZERO: WorkDelta = WorkDelta([0; METRIC_COUNT]);

    pub fn get(&self, metric: Metric) -> u64 {
        self.0[metric as usize]
    }

    /// Metrics with a nonzero count, in index order.
    pub fn nonzero(&self) -> impl Iterator<Item = (Metric, u64)> + '_ {
        Metric::ALL
            .into_iter()
            .map(|m| (m, self.get(m)))
            .filter(|&(_, n)| n != 0)
    }

    /// Checks this delta against `budgets`, which are ceilings: each listed
    /// metric may not exceed its budget and every unlisted metric must be
    /// zero. A ceiling cannot see a counter that stopped counting, so use it
    /// only where the number is intentionally a bound, and prefer
    /// [`check_exact`](Self::check_exact). The error names each violation, so
    /// a new kind of work cannot slip in unnoticed.
    pub fn check_budget(&self, budgets: &[(Metric, u64)]) -> Result<(), String> {
        let mut violations = Vec::new();
        for metric in Metric::ALL {
            let got = self.get(metric);
            let budget = budgets
                .iter()
                .find(|(m, _)| *m == metric)
                .map_or(0, |&(_, b)| b);
            if got > budget {
                violations.push(format!("{} was {got}, budget {budget}", metric.name()));
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations.join("; "))
        }
    }

    /// Checks this delta against `expected` counts: each listed metric must
    /// equal its count and every unlisted metric must be zero. Unlike a
    /// ceiling this fails when a counted site stops recording, so a dead
    /// counter cannot pass as cheap work.
    pub fn check_exact(&self, expected: &[(Metric, u64)]) -> Result<(), String> {
        let mut violations = Vec::new();
        for metric in Metric::ALL {
            let got = self.get(metric);
            let want = expected
                .iter()
                .find(|(m, _)| *m == metric)
                .map_or(0, |&(_, n)| n);
            if got != want {
                violations.push(format!("{} was {got}, expected {want}", metric.name()));
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations.join("; "))
        }
    }

    /// Adds a worker thread's already validated delta to this one.
    pub fn merged(self, other: WorkDelta) -> Result<WorkDelta, MetricsError> {
        let mut out = [0; METRIC_COUNT];
        for (index, slot) in out.iter_mut().enumerate() {
            *slot = self.0[index]
                .checked_add(other.0[index])
                .ok_or(MetricsError::Overflow { index })?;
        }
        Ok(WorkDelta(out))
    }
}

/// Counts `$amount` units of `$metric` on the current thread.
///
/// `$amount` is converted with `u64::try_from`; a value that does not fit
/// panics instead of wrapping. Without the `work-metrics` feature the macro
/// expands to an empty block and `$amount` is never evaluated.
#[cfg(feature = "work-metrics")]
#[macro_export]
macro_rules! record_work {
    ($metric:ident, $amount:expr) => {{
        $crate::metrics::record(
            $crate::metrics::Metric::$metric,
            u64::try_from($amount).expect("work metric amount must fit u64"),
        );
    }};
}

#[cfg(not(feature = "work-metrics"))]
#[macro_export]
macro_rules! record_work {
    ($metric:ident, $amount:expr) => {{}};
}

#[cfg(all(test, not(feature = "work-metrics")))]
mod disabled_tests {
    #[test]
    fn metrics_disabled_does_not_evaluate_arguments() {
        let evaluated = std::cell::Cell::new(false);
        crate::record_work!(SegmentedLines, {
            evaluated.set(true);
            1usize
        });
        assert!(
            !evaluated.get(),
            "a disabled build must not evaluate the amount"
        );
        const { assert!(!super::ENABLED) };
    }
}

#[cfg(all(test, feature = "work-metrics"))]
mod tests;
