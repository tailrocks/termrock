//! Lane `gridtable` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:gridtable` block in `../coverage.rs`. Edit only
//! this file, `../coverage/lane_gridtable.rs` and `../coverage_debt/gridtable.txt`.

use super::ScenarioCase;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
];
