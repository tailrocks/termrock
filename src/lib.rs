pub mod catalog;
#[cfg(feature = "cli")]
pub mod cli;
pub mod clock;
pub mod core;
pub mod docs;
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
