//! The widgets, each a `pub use` of the crate that owns it. The line-edit fields
//! (`edit`, `field_common`) live in `termrock-core::field`.

pub use termrock_collections::{detail_card, list, list_detail, props, search_detail, tree};
pub use termrock_controls::{
    action_row, brand, button, choice, hintbar, icon_button, keyhint, keyref, scrollbar, segments,
    widget_hints,
};
pub use termrock_core::field::{edit, field_common};
pub use termrock_dialogs::{
    choice_dialog, dialog, file_browser, form_dialog, help, info_dialog, inspect_changes,
};
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
