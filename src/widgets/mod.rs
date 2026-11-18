pub mod breadcrumb;
pub mod choice_dialog;
pub mod detail_card;
pub mod dialog;
pub mod document_workspace;
pub mod file_browser;
pub mod form_dialog;
pub mod grid;
#[cfg(test)]
mod grid_sort_tests;
mod grid_specimen;
#[cfg(test)]
mod grid_undo_tests;
pub mod help;
pub mod info_dialog;
pub mod inspect_changes;
pub mod list;
pub mod list_detail;
pub mod nav_list;
pub mod props;
pub mod rail;
pub mod search_detail;
pub mod splitter;
pub mod steps;
pub mod table;
mod table_specimen;
pub mod tabs;
pub mod toolbar;
pub mod tree;

#[cfg(test)]
mod collection_edit_tests;
#[cfg(all(test, feature = "work-metrics"))]
mod keyed_cost_tests;

// Transitional: the line-edit fields live in `termrock-core` (#444); #452 removes this.
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
pub use termrock_overlays::{completion, menu, option_editor, option_list, picker, row_menu};
pub use termrock_viewport::{panel, viewport, viewport_ops};
