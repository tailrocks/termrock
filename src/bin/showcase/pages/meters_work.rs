//! The Meters page's "Work counters" card: the #56 work counters and the
//! contract that reads them.
//!
//! The counters compile away without the `work-metrics` feature, and the
//! Showcase's default build does not enable it. The card therefore says which
//! build it is in and shows every part that does not need a recording build
//! (the catalog, the delta, budget and merge contract on hand-built
//! snapshots), next to the live parts that do (this frame's own work and the
//! `record_work!` probe).

use junie_tui::metrics::{ENABLED, METRIC_COUNT, Metric, WorkDelta, WorkSnapshot};
use junie_tui::theme::Tone;
use junie_tui::widgets::props::Prop;

/// Rows the card paints; a card is always this tall.
pub const ROWS: u16 = 12;

/// The work one frame of the page did, from a snapshot taken when it began.
pub struct Frame {
    #[cfg(feature = "work-metrics")]
    start: WorkSnapshot,
}

impl Frame {
    pub fn begin() -> Self {
        Self {
            #[cfg(feature = "work-metrics")]
            start: junie_tui::metrics::snapshot(),
        }
    }

    /// `None` in a build that does not record.
    pub fn delta(&self) -> Option<WorkDelta> {
        #[cfg(feature = "work-metrics")]
        {
            junie_tui::metrics::snapshot().delta(self.start).ok()
        }
        #[cfg(not(feature = "work-metrics"))]
        {
            None
        }
    }
}

fn probe_site() {
    junie_tui::record_work!(SegmentedLines, 3usize);
}

/// Runs one `record_work!` site and returns what it counted, or `None` when
/// the macro compiled to nothing.
pub fn probe() -> Option<u64> {
    #[cfg(feature = "work-metrics")]
    {
        let ((), delta) = junie_tui::metrics::measure(probe_site);
        Some(delta.get(Metric::SegmentedLines))
    }
    #[cfg(not(feature = "work-metrics"))]
    {
        probe_site();
        None
    }
}

/// The card's meta text: which kind of build this is.
pub fn meta() -> &'static str {
    if ENABLED { "recording" } else { "compiled out" }
}

fn snapshot(thread: std::thread::ThreadId, epoch: u64, rows: u64, widths: u64) -> WorkSnapshot {
    let mut counts = [0; METRIC_COUNT];
    counts[Metric::VisibleRowsPainted as usize] = rows;
    counts[Metric::IntrinsicWidthCalls as usize] = widths;
    WorkSnapshot {
        thread,
        epoch,
        counts,
    }
}

/// The delta, budget, exact, merge and refusal rows. They run the real
/// `WorkSnapshot` and `WorkDelta` API over hand-built snapshots, so they read
/// the same in a build that records and in one that does not.
pub fn contract() -> &'static [Prop] {
    static CONTRACT: std::sync::OnceLock<Vec<Prop>> = std::sync::OnceLock::new();
    CONTRACT.get_or_init(build_contract)
}

fn build_contract() -> Vec<Prop> {
    let thread = std::thread::current().id();
    let before = snapshot(thread, 0, 10, 0);
    let after = snapshot(thread, 0, 16, 40);
    let delta = after.delta(before).expect("same thread and epoch");
    let moved = delta
        .nonzero()
        .map(|(metric, n)| format!("{} +{n}", metric.name()))
        .collect::<Vec<_>>()
        .join(", ");

    let budget = delta.check_budget(&[
        (Metric::VisibleRowsPainted, 6),
        (Metric::IntrinsicWidthCalls, 30),
    ]);
    let exact = delta.check_exact(&[
        (Metric::VisibleRowsPainted, 6),
        (Metric::IntrinsicWidthCalls, 40),
    ]);

    let mut worker = WorkDelta::ZERO;
    worker.0[Metric::IntrinsicWidthCalls as usize] = 2;
    let merged = delta
        .merged(worker)
        .expect("a small merge does not overflow");

    let other = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("the probe thread returns its id");
    let refusals = [
        ("Thread", snapshot(other, 0, 16, 40).delta(before)),
        ("Epoch", snapshot(thread, 1, 16, 40).delta(before)),
        ("Backwards", snapshot(thread, 0, 4, 0).delta(before)),
        (
            "Overflow",
            WorkDelta([u64::MAX; METRIC_COUNT]).merged(worker),
        ),
    ];

    let mut rows = vec![
        Prop::new("Delta", moved),
        match budget {
            Ok(()) => Prop::new("Budget", "within budget").tone(Tone::Success),
            Err(violations) => Prop::new("Budget", violations).tone(Tone::Warning),
        },
        match exact {
            Ok(()) => Prop::new("Exact", "every count matches").tone(Tone::Success),
            Err(violations) => Prop::new("Exact", violations).tone(Tone::Warning),
        },
        Prop::new(
            "Merged",
            format!(
                "{} {} (40 + a worker's 2)",
                Metric::IntrinsicWidthCalls.name(),
                merged.get(Metric::IntrinsicWidthCalls)
            ),
        ),
    ];
    for (label, outcome) in refusals {
        let refused = match outcome {
            Ok(_) => "accepted".to_owned(),
            Err(error) => error.to_string(),
        };
        rows.push(Prop::new(label, refused).tone(Tone::Warning));
    }
    rows
}

/// The card's rows: the build, the catalog, this frame's work, the probe and
/// the cached contract rows. Exactly [`ROWS`] long.
pub fn facts(frame: Option<&WorkDelta>, probe: Option<u64>, contract: &[Prop]) -> Vec<Prop> {
    let first = Metric::ALL[0].name();
    let last = Metric::ALL[METRIC_COUNT - 1].name();
    let mut facts = vec![
        if ENABLED {
            Prop::new("Counters", "recording (work-metrics on)").tone(Tone::Success)
        } else {
            Prop::new(
                "Counters",
                "compiled out (build with --features work-metrics)",
            )
            .tone(Tone::Muted)
        },
        Prop::new(
            "Catalog",
            format!("{METRIC_COUNT} metrics, {first} to {last}"),
        ),
        match frame {
            Some(delta) => {
                let moved = delta
                    .nonzero()
                    .take(2)
                    .map(|(metric, n)| format!("{} {n}", metric.name()))
                    .collect::<Vec<_>>();
                if moved.is_empty() {
                    Prop::new("This frame", "no work counted")
                } else {
                    Prop::new("This frame", moved.join(", "))
                }
            }
            None => Prop::new("This frame", "not recorded in this build").tone(Tone::Muted),
        },
        match probe {
            Some(n) => Prop::new(
                "Probe",
                format!("record_work!(SegmentedLines, 3) counted {n}"),
            ),
            None => Prop::new("Probe", "record_work! expands to nothing here").tone(Tone::Muted),
        },
    ];
    facts.extend(contract.iter().cloned());
    facts
}
