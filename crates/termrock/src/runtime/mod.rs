//! Application-loop infrastructure with immutable frame time.

mod time;

#[cfg(feature = "crossterm")]
mod runner;

#[cfg(feature = "crossterm")]
pub use runner::{RunOptions, run};
pub use time::FrameTick;
