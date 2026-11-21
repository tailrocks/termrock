//! Paste-protocol strings the frozen reference binaries still read.
//!
//! The binaries under `reference-bins/` were built before the functional
//! harness requests became `termrock:*`, and they answer only the old
//! `junie-tui:*` strings. A Reference-mode session therefore pastes these
//! requests instead of the `termrock:*` ones that `TestRequest` builds.
//! Candidate sessions never paste these strings, and no library or app code
//! reads them. This is the only place in the tree that spells the old
//! protocol prefix; `legacy_reference_env.rs` is the matching module for the
//! environment names.

pub const ADVANCE_PREFIX: &str = "junie-tui:advance-clock:";

pub const FENCE: &str = "junie-tui:fence";

/// The stepped-clock request for `milliseconds` that a frozen binary reads.
pub fn advance(milliseconds: u64) -> String {
    format!("{ADVANCE_PREFIX}{milliseconds}")
}
