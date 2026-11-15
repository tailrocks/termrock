//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

// Shared support module, kept at the crate root so moved tests can use `crate::common`.
#[path = "../common/mod.rs"]
mod common;

mod file_browser_paths;
mod smoke;
