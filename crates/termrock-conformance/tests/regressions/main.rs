//! Shared target for the regression tests of `termrock-conformance`.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`). Never install a
//! `#[global_allocator]` here: every test in this binary would then count
//! allocations (see the #449 decision in `docs/decisions.md`).

mod intrinsic;
mod intrinsic_wide;
#[cfg(feature = "work-metrics")]
mod keyed_cost;
mod overflow_parity;
#[cfg(feature = "work-metrics")]
mod overflow_work;
mod scroll_view_window;
#[cfg(feature = "work-metrics")]
mod search_detail_cost;
