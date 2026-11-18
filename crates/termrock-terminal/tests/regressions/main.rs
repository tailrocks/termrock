//! Regression tests of `termrock-terminal`.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

mod forward_policy;
mod pane_edge;
mod session;
mod terminal;
mod terminal_guard;
mod terminal_viewport;
