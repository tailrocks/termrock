#[cfg(feature = "cli")]
pub use termrock_cli as cli;
pub mod core;
pub mod metrics;
pub use termrock_catalog::{catalog, docs};
pub use termrock_core::motion;
pub use termrock_session::clock;
pub mod runtime;
pub mod shell;
#[cfg(feature = "terminal")]
pub use termrock_terminal as terminal;
pub use termrock_theme as theme;
pub mod ui;
pub mod widgets;

// Transitional: `crate::record_work!` keeps resolving for the files that have not moved yet (#452 removes this).
pub use termrock_metrics::record_work;
