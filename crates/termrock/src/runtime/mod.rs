//! Application-loop infrastructure with immutable frame time and motion.
//!
//! - [`FrameTick`] / [`FrameClock`] — injectable per-frame time
//! - [`Presence`] — delayed show / TTL / exit without hidden focus
//! - [`AnimationDemand`] — when to wake; never idle decorative redraw
//! - [`Presenter`] — the single wire seam: dirty coalescing, backpressure,
//!   cursor de-dup, the [`FrameRate`] ladder, and a separate scroll clock
//! - [`Animator`] / [`Tween`] / [`Spring`] — typed value animation (layer 1)

mod animate;
mod motion;
mod presenter;
mod time;

#[cfg(feature = "crossterm")]
mod runner;

pub use animate::{AnimId, Animation, Animator, SPRING_DAMPING, SPRING_FREQUENCY, Spring, Tween};
pub use motion::{
    AnimationDemand, Presence, PresenceChange, PresencePhase, earliest_deadline, min_deadline,
    pulse_fraction, spinner_demand, spinner_step,
};
pub use presenter::{
    DEFAULT_MIN_DRAW_INTERVAL, FrameRate, Presenter, QuietBackend, SCROLL_FLUSH_INTERVAL,
    ScrollClock, TickLadder,
};
#[cfg(feature = "crossterm")]
pub use runner::{RunOptions, run};
pub use time::{FrameClock, FrameTick, Instant};
