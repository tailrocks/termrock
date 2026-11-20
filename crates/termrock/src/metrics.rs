//! Deterministic work counters.
//!
//! The counters live in `termrock-metrics`. The scenario harness and its report
//! drive an `Application`, so they live in `termrock-conformance` and appear here
//! as `metrics::harness` and `metrics::report` under `work-metrics`.

#[cfg(feature = "work-metrics")]
pub use termrock_conformance::{harness, report};

pub use termrock_metrics::{
    ENABLED, METRIC_COUNT, Metric, MetricsError, WorkDelta, WorkSnapshot, measurement,
};
#[cfg(feature = "work-metrics")]
pub use termrock_metrics::{measure, record, reset, snapshot, suspended};
