//! Regression tests of `termrock-layout`.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

mod master_detail_bounds;
mod pane_rig;
mod pane_tree_minima;
mod pane_tree_pointer;
