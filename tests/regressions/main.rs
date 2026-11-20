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
mod core_extension_points;
mod data_shape;
mod document_workspace;
mod facade_contract;
mod file_browser_paths;
mod focus_edit;
mod form_dialog_geometry;
mod form_value;
mod foundation_paths;
mod grapheme_paint;
mod grid_parsers;
mod grid_validation;
mod help_geometry;
mod keyed_collections;
mod keyed_costs;
mod layering;
mod loading_label;
mod nav_list_state;
mod nav_list_width;
mod popup_stale_anchor;
mod response_batches;
mod scroll_view_navigation;
mod search_detail;
mod search_detail_view;
mod tiny_dialogs;
mod toolbar_hidden_cursor;
mod tree_lazy_root;
