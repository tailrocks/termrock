//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

#[path = "../common/mod.rs"]
mod common;
mod help_geometry;
mod master_detail_bounds;
mod nav_list_state;
mod pane_tree_pointer;
mod scroll_view_navigation;
mod smoke;
