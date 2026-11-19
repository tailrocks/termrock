//! The layer stack, the modal frame, menus, the option list, completion, context
//! and edit menus, row menus and the picker.
//!
//! `termrock::ui::{context, edit_menu, layer, modal}` and `termrock::widgets::
//! {completion, menu, option_editor, option_list, picker, row_menu}` are the modules of the same
//! name here; the `junie-tui` package re-exports each at that path.

pub mod completion;
pub mod context;
pub mod edit_menu;
pub mod layer;
pub mod menu;
pub mod modal;
pub mod option_editor;
pub mod option_list;
pub mod picker;
pub mod row_menu;
