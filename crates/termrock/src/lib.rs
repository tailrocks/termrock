//! Supported import path for the Termrock library.
//!
//! Every module here is a `pub use` of the crate that owns it (`termrock-core`,
//! `termrock-runtime`, `termrock-controls`, and so on), so a type has one
//! identity under `termrock::` and under its owning crate. `core`, `metrics`,
//! `ui` and `widgets` are assembled from several crates, and each list is
//! explicit: a crate's `#[doc(hidden)]` items stay out of the facade. Features
//! default to none; enable `cli` or `terminal` to get those modules.

#[cfg(feature = "cli")]
pub use termrock_cli as cli;
pub mod core;
pub mod metrics;
pub use termrock_core::motion;
pub use termrock_metrics::record_work;
pub use termrock_session::clock;
pub use termrock_shell as shell;
pub use termrock_shell::runtime;
#[cfg(feature = "terminal")]
pub use termrock_terminal as terminal;
pub use termrock_theme as theme;
pub mod ui;
pub mod widgets;

/// Tooling surfaces: the public-API catalog, the `reach_tests!` macro and the generated documentation
/// helpers. Applications such as Showcase use them to prove coverage; ordinary
/// consumers do not need them.
pub use termrock_catalog::{catalog, docs};
pub use termrock_shell::reach_tests;

pub mod prelude {
    pub use termrock_core::id::{ItemKey, WidgetId};
    pub use termrock_core::list::{KeyedRowSource, RowSource, SourceStamp};
    pub use termrock_core::response::{Actions, Flow, Invalidate, Response};
}
