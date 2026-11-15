//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

mod chip_add_hotkey;
mod collection_edit_lifecycle;
mod collection_menu_lifecycle;
#[path = "../common/mod.rs"]
mod common;
mod data_shape;
mod document_workspace;
mod file_browser_paths;
mod focus_edit;
mod form_value;
mod grapheme_paint;
mod grid_parsers;
mod grid_validation;
mod help_geometry;
mod keyed_collections;
mod keyed_costs;
mod master_detail_bounds;
mod nav_list_state;
mod pane_rig;
mod pane_tree_minima;
mod pane_tree_pointer;
mod picker_rows;
mod response_batches;
mod scroll_view_navigation;
mod smoke;
mod tabs_overflow_mutation;
mod work_budgets;
