pub mod breadcrumb;
pub mod chips;
pub mod choice_dialog;
pub mod code;
mod code_specimen;
pub mod completion;
pub mod detail_card;
pub mod dialog;
pub mod diff;
pub mod document_workspace;
pub mod empty;
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
pub mod input;
pub mod inspect_changes;
pub mod list;
pub mod list_detail;
pub mod menu;
pub mod meter_list;
pub mod nav_list;
pub mod notice;
pub mod option_editor;
pub mod option_list;
pub mod page_header;
pub mod panel;
pub mod picker;
pub mod progress;
pub mod props;
pub mod rail;
pub mod row_menu;
pub mod search_detail;
pub mod select;
pub mod splitter;
pub mod statusbar;
pub mod steps;
pub mod table;
mod table_specimen;
pub mod tabs;
pub mod textarea;
pub mod too_small;
pub mod toolbar;
pub mod tree;
pub mod viewport;
pub mod viewport_ops;
mod viewport_specimen;

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
