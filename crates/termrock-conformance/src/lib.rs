//! Proof code of the Termrock library that drives other crates: the work-metrics
//! scenario harness and its count-only report, and the cross-crate regression
//! tests (#449).
//!
//! The harness drives an `Application`, so it sits above `termrock-shell` and
//! cannot live with the counters in `termrock-metrics`. the `termrock` facade re-exports
//! `harness` and `report` under its `work-metrics` feature, which is how
//! `termrock::metrics::harness` and `termrock::metrics::report` keep their
//! spelling. No library crate depends on this one.

#[cfg(feature = "work-metrics")]
pub mod harness;
#[cfg(feature = "work-metrics")]
pub mod report;

#[cfg(all(test, feature = "work-metrics"))]
mod metrics_tests;
