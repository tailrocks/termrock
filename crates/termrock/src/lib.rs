//! Supported import path for the Termrock library.
//!
//! Every module here is a re-export of the implementation package
//! `junie-tui`, so a type has one identity under both paths. Features
//! default to none; enable `cli` or `terminal` to get those modules.

pub use junie_tui::{
    clock, core, metrics, motion, record_work, runtime, shell, theme, ui, widgets,
};

/// Tooling surfaces: the public-API catalog and the generated documentation
/// helpers. Applications such as Showcase use them to prove coverage; ordinary
/// consumers do not need them.
pub use junie_tui::{catalog, docs};

#[cfg(feature = "cli")]
pub use junie_tui::cli;
#[cfg(feature = "terminal")]
pub use junie_tui::terminal;

pub mod prelude {
    pub use junie_tui::core::id::{ItemKey, WidgetId};
    pub use junie_tui::core::list::{KeyedRowSource, RowSource, SourceStamp};
    pub use junie_tui::core::response::{Actions, Flow, Invalidate, Response};
}
