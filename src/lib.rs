pub mod catalog;
#[cfg(feature = "cli")]
pub mod cli;
pub mod clock;
pub mod core;
pub mod docs;
pub mod metrics;
pub mod motion;
pub mod runtime;
pub mod shell;
#[cfg(feature = "terminal")]
pub mod terminal;
#[cfg(test)]
mod test_alloc;
pub mod theme;
pub mod ui;
pub mod widgets;

// Transitional: `crate::record_work!` keeps resolving for the files that have not moved yet (#452 removes this).
pub use termrock_metrics::record_work;

#[cfg(test)]
mod catalog_contract;
