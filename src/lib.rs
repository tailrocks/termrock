pub mod catalog;
#[cfg(feature = "cli")]
pub mod cli;
pub mod core;
pub mod docs;
pub mod metrics;
pub use termrock_core::motion;
pub use termrock_session::clock;
pub mod runtime;
pub mod shell;
#[cfg(feature = "terminal")]
pub use termrock_terminal as terminal;
#[cfg(test)]
mod test_alloc;
pub use termrock_theme as theme;
pub mod ui;
pub mod widgets;

// Transitional: `crate::record_work!` keeps resolving for the files that have not moved yet (#452 removes this).
pub use termrock_metrics::record_work;

#[cfg(test)]
mod catalog_contract;
