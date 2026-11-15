//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

#![cfg(feature = "demo")]

// Support modules live beside the top-level targets and are shared with them.
// They sit at the crate root because `capsule_support` imports `crate::pane_support`.
#[path = "../capsule_support/mod.rs"]
mod capsule_support;
#[path = "../pane_support/mod.rs"]
mod pane_support;
#[path = "../product_support/mod.rs"]
mod product_support;

mod navigation_focus;
mod smoke;
