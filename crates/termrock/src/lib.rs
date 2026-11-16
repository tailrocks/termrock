//! Supported import path for the Termrock library.
//!
//! Every module here is a re-export of the implementation package
//! `junie-tui`, so a type has one identity under both paths. Features
//! default to none; enable `cli` or `terminal` to get those modules.

pub use junie_tui::{clock, core, motion, runtime, shell, theme, ui, widgets};

#[cfg(feature = "cli")]
pub use junie_tui::cli;
#[cfg(feature = "terminal")]
pub use junie_tui::terminal;

pub mod prelude {
    pub use junie_tui::core::id::{ItemKey, WidgetId};
    pub use junie_tui::core::list::RowSource;
    pub use junie_tui::core::response::{Flow, Invalidate, Response};
}
