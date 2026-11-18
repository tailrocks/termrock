//! Dialogs, forms, the file browser, the change inspector and the leave guard.
//!
//! `termrock::widgets::{choice_dialog, dialog, file_browser, form_dialog, help,
//! info_dialog, inspect_changes}` and `termrock::ui::{layers, leave}` are the
//! modules of the same name here; the `junie-tui` package re-exports each at
//! that path.

pub mod choice_dialog;
pub mod dialog;
pub mod file_browser;
pub mod form_dialog;
pub mod help;
pub mod info_dialog;
pub mod inspect_changes;
pub mod layers;
pub mod leave;
