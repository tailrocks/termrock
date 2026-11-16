//! Lane `state` of the #14 burn-down: this lane's callable scenarios.
//!
//! Each scenario builds real components, sends real input, reads typed
//! outcomes and renders to a `Buffer`. Register one `ScenarioCase` per
//! `ScenarioId` variant from the `lane:state` block in `../coverage.rs`. Edit only
//! this file, `../coverage/lane_state.rs` and `../coverage_debt/state.txt`.

use super::ScenarioCase;

#[rustfmt::skip]
pub const CASES: &[ScenarioCase] = &[
];
