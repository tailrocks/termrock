// Lane `theme` of the #14 burn-down: this lane's proofs and exceptions.
//
// Edit only this file, `../app_tests_catalog/theme.rs`, `../coverage_debt/theme.txt`
// and the `lane:theme` block of `ScenarioId` in `../coverage.rs`. One record per
// line keeps a restack mechanical. A record is also a claim the gate checks:
// a proof names scenarios that really exercise the API, an exception names a
// live `#[test]` that uses it. Delete the API's line from the debt file in the
// same commit.

use super::{CapabilityException, CapabilityProof};

#[rustfmt::skip]
pub const PROOFS: &[CapabilityProof] = &[
];

#[rustfmt::skip]
pub const EXCEPTIONS: &[CapabilityException] = &[
];
