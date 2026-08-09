//! Application-loop infrastructure with immutable frame time and motion.
//!
//! - [`FrameTick`] / [`FrameClock`] — injectable per-frame time  
//! - [`Presence`] — delayed show / TTL / exit without hidden focus  
//! - [`AnimationDemand`] — when to wake; never idle decorative redraw  

mod motion;
mod time;

#[cfg(feature = "crossterm")]
mod runner;

#[cfg(feature = "crossterm")]
pub use runner::{RunOptions, run};
pub use motion::{
    AnimationDemand, Presence, PresenceChange, PresencePhase, earliest_deadline, min_deadline,
    pulse_fraction, spinner_demand, spinner_step,
};
pub use time::{FrameClock, FrameTick};
