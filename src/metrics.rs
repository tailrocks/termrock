//! Deterministic work counters.
//!
//! The counters live in `termrock-metrics`; this module keeps every old path
//! valid while the extraction runs (#444) and adds the scenario harness, which
//! drives an `Application` and so cannot sit below `runtime` (#449 moves it).

#[cfg(feature = "work-metrics")]
pub mod harness;
#[cfg(feature = "work-metrics")]
pub mod report;
#[cfg(all(test, feature = "work-metrics"))]
mod tests;

pub use termrock_metrics::{
    ENABLED, METRIC_COUNT, Metric, MetricsError, WorkDelta, WorkSnapshot, measurement,
};
#[cfg(feature = "work-metrics")]
pub use termrock_metrics::{measure, record, reset, snapshot, suspended};
