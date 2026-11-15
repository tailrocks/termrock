//! Shared target for regression tests.
//!
//! Add a test as `<topic>.rs` next to this file and one `mod <topic>;` line
//! here. Never add a new top-level `tests/*.rs`: each target changes the
//! generated CI workflows (see `docs/ci.md`).

mod button_row_focus;
mod chip_add_hotkey;
mod chip_hidden_cursor;
mod collection_edit_lifecycle;
mod collection_menu_lifecycle;
#[path = "../common/mod.rs"]
mod common;
mod data_shape;
mod deferred_api_calls;
mod document_workspace;
mod file_browser_paths;
mod focus_edit;
mod form_dialog_geometry;
mod form_value;
mod grapheme_paint;
mod grid_parsers;
mod grid_validation;
mod help_geometry;
mod item_eligibility;
mod keyed_collections;
mod keyed_costs;
mod loading_label;
mod master_detail_bounds;
mod nav_list_state;
mod nav_list_width;
mod pane_rig;
mod pane_tree_minima;
mod pane_tree_pointer;
mod picker_row_limits;
mod picker_rows;
mod response_batches;
mod scroll_view_navigation;
mod search_detail;
mod search_detail_view;
mod smoke;
mod tabs_overflow_mutation;
mod tiny_dialogs;
mod work_budgets;
