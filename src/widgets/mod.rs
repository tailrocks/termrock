pub mod choice_dialog;
pub mod dialog;
pub mod file_browser;
pub mod form_dialog;
pub mod help;
pub mod info_dialog;
pub mod inspect_changes;

#[cfg(all(test, feature = "work-metrics"))]
mod keyed_cost_tests;
// Drives a `Select` and a `TextInput`, so it sits above the collections crate; #449 moves it to `termrock-conformance`.
#[cfg(all(test, feature = "work-metrics"))]
#[path = "search_detail/cost_tests.rs"]
mod search_detail_cost_tests;

// Transitional: the line-edit fields live in `termrock-core` (#444); #452 removes this.
pub use termrock_collections::{detail_card, list, list_detail, props, search_detail, tree};
pub use termrock_controls::{
    action_row, brand, button, choice, hintbar, icon_button, keyhint, keyref, scrollbar, segments,
    widget_hints,
};
pub(crate) use termrock_core::field::text_select;
pub use termrock_core::field::{edit, field_common};
pub use termrock_editors::{code, diff, textarea};
pub use termrock_feedback::{
    empty, meter_list, notice, page_header, progress, statusbar, too_small,
};
pub use termrock_fields::{chips, input, select};
pub use termrock_grid::{grid, table};
pub use termrock_layout::splitter;
pub use termrock_navigation::{
    breadcrumb, document_workspace, nav_list, rail, steps, tabs, toolbar,
};
pub use termrock_overlays::{completion, menu, option_editor, option_list, picker, row_menu};
pub use termrock_viewport::{panel, viewport, viewport_ops};
