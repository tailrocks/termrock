//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

#[path = "../common/mod.rs"]
mod common;
mod form_value;
mod grapheme_paint;
mod scroll_view_navigation;
mod help_geometry;
mod nav_list_state;
mod master_detail_bounds;
mod pane_tree_minima;
mod pane_tree_pointer;
// Shared support module, kept at the crate root so moved tests can use `crate::common`.

mod file_browser_paths;
mod smoke;
mod tabs_overflow_mutation;
