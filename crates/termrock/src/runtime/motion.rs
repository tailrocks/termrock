// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Deterministic motion helpers on top of [`FrameTick`].
//!
//! Law: never redraw idle screens solely for decorative animation. Hosts
//! poll [`AnimationDemand`] / [`Presence::next_deadline`] and only wake when
//! something timed is active and [`crate::style::Motion`] allows motion.

use std::time::{Duration, Instant};

use crate::style::Motion;

use super::FrameTick;

/// Whether animation requires a future frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AnimationDemand {
    /// Host should redraw soon for this source.
    pub needs_redraw: bool,
    /// Earliest useful wakeup (None = no timed follow-up from this source).
    pub next_deadline: Option<Instant>,
}

impl AnimationDemand {
    /// Idle — no animation.
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            needs_redraw: false,
            next_deadline: None,
        }
    }

    /// Merge two demands (OR redraw; earliest deadline).
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            needs_redraw: self.needs_redraw || other.needs_redraw,
            next_deadline: min_deadline(self.next_deadline, other.next_deadline),
        }
    }
}

/// Earliest of two optional instants.
#[must_use]
pub fn min_deadline(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if x <= y { x } else { y }),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

/// Combine many deadlines.
#[must_use]
pub fn earliest_deadline(deadlines: impl IntoIterator<Item = Option<Instant>>) -> Option<Instant> {
    let mut best = None;
    for d in deadlines {
        best = min_deadline(best, d);
    }
    best
}

/// Spinner / indeterminate frame index from elapsed time.
#[must_use]
pub fn spinner_step(
    tick: FrameTick,
    frame_count: usize,
    period_ms: u64,
    motion: Motion,
) -> usize {
    if frame_count == 0 {
        return 0;
    }
    if !motion.animate_spinners() {
        return 0;
    }
    let period = period_ms.max(1).saturating_mul(motion.spinner_divisor().max(1));
    let step = tick.elapsed().as_millis() as u64 / period;
    (step as usize) % frame_count
}

/// Soft pulse 0.0..1.0 for reduced-friendly indeterminate bars (static when Off).
#[must_use]
pub fn pulse_fraction(tick: FrameTick, period_ms: u64, motion: Motion) -> f64 {
    match motion {
        Motion::Off => 0.5,
        Motion::Reduced | Motion::Full => {
            let period = period_ms.max(1) as f64
                * if matches!(motion, Motion::Reduced) {
                    2.0
                } else {
                    1.0
                };
            let t = tick.elapsed().as_secs_f64() * (std::f64::consts::TAU / period);
            0.5 + 0.5 * t.sin()
        }
    }
}

/// Demand for an active spinner while work is in progress.
#[must_use]
pub fn spinner_demand(tick: FrameTick, motion: Motion, active: bool) -> AnimationDemand {
    if !active || !motion.animate_spinners() {
        return AnimationDemand::idle();
    }
    // Wake about once per frame period (~80ms default).
    let period = Duration::from_millis(80 * motion.spinner_divisor().max(1));
    AnimationDemand {
        needs_redraw: true,
        next_deadline: Some(tick.now() + period),
    }
}

/// Presence phase for transient UI (tooltip, toast, delayed chrome).
///
/// **Focus law:** only [`PresencePhase::Visible`] is focusable. Delayed enter
/// and exit never keep hidden focusable elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PresencePhase {
    /// Not shown; not focusable.
    #[default]
    Hidden,
    /// Waiting for show delay (not painted / not focusable).
    Pending {
        /// When pending started.
        since: Instant,
    },
    /// Fully shown (may be focusable if the surface allows).
    Visible {
        /// When became visible.
        since: Instant,
    },
    /// Exit hold (optional; reduced motion skips to Hidden immediately).
    Exiting {
        /// When exit started.
        since: Instant,
    },
}

/// Timed presence controller (show delay, TTL, exit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Presence {
    phase: PresencePhase,
    /// Delay after `request_show` before Visible.
    show_delay: Duration,
    /// Auto-hide after this long in Visible (`None` = until dismiss).
    visible_ttl: Option<Duration>,
    /// Exit phase duration (`Duration::ZERO` = instant hide).
    exit_duration: Duration,
}

impl Default for Presence {
    fn default() -> Self {
        Self::immediate()
    }
}

impl Presence {
    /// Visible immediately on show; no auto-hide; instant dismiss.
    #[must_use]
    pub const fn immediate() -> Self {
        Self {
            phase: PresencePhase::Hidden,
            show_delay: Duration::ZERO,
            visible_ttl: None,
            exit_duration: Duration::ZERO,
        }
    }

    /// Tooltip-style: delay before show, no TTL.
    #[must_use]
    pub const fn tooltip(delay: Duration) -> Self {
        Self {
            phase: PresencePhase::Hidden,
            show_delay: delay,
            visible_ttl: None,
            exit_duration: Duration::ZERO,
        }
    }

    /// Toast-style: immediate show, expires after `ttl`.
    #[must_use]
    pub const fn toast(ttl: Duration) -> Self {
        Self {
            phase: PresencePhase::Hidden,
            show_delay: Duration::ZERO,
            visible_ttl: Some(ttl),
            exit_duration: Duration::ZERO,
        }
    }

    /// Persistent toast until dismiss.
    #[must_use]
    pub const fn persistent() -> Self {
        Self::immediate()
    }

    /// Current phase.
    #[must_use]
    pub const fn phase(self) -> PresencePhase {
        self.phase
    }

    /// Painted (Visible or optional Exiting chrome).
    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(
            self.phase,
            PresencePhase::Visible { .. } | PresencePhase::Exiting { .. }
        )
    }

    /// May accept focus / hit targets (Visible only).
    #[must_use]
    pub const fn is_focusable(self) -> bool {
        matches!(self.phase, PresencePhase::Visible { .. })
    }

    /// Request show (starts Pending or Visible).
    pub fn request_show(&mut self, tick: FrameTick) {
        if self.show_delay.is_zero() {
            self.phase = PresencePhase::Visible { since: tick.now() };
        } else {
            self.phase = PresencePhase::Pending { since: tick.now() };
        }
    }

    /// Request hide (instant or Exiting).
    pub fn request_hide(&mut self, tick: FrameTick, motion: Motion) {
        if matches!(self.phase, PresencePhase::Hidden) {
            return;
        }
        if self.exit_duration.is_zero() || !motion.animate_spinners() {
            self.phase = PresencePhase::Hidden;
        } else {
            self.phase = PresencePhase::Exiting { since: tick.now() };
        }
    }

    /// Hard clear (no exit phase).
    pub const fn force_hide(&mut self) {
        self.phase = PresencePhase::Hidden;
    }

    /// Advance phase from frame time; returns whether visibility changed.
    pub fn advance(&mut self, tick: FrameTick, motion: Motion) -> PresenceChange {
        let before = self.is_visible();
        match self.phase {
            PresencePhase::Hidden => {}
            PresencePhase::Pending { since } => {
                if tick.now().saturating_duration_since(since) >= self.show_delay {
                    self.phase = PresencePhase::Visible { since: tick.now() };
                }
            }
            PresencePhase::Visible { since } => {
                if let Some(ttl) = self.visible_ttl
                    && tick.now().saturating_duration_since(since) >= ttl
                {
                    self.request_hide(tick, motion);
                }
            }
            PresencePhase::Exiting { since } => {
                if tick.now().saturating_duration_since(since) >= self.exit_duration {
                    self.phase = PresencePhase::Hidden;
                }
            }
        }
        let after = self.is_visible();
        if before == after {
            PresenceChange::None
        } else if after {
            PresenceChange::BecameVisible
        } else {
            PresenceChange::BecameHidden
        }
    }

    /// Next deadline for pending / TTL / exit (for host poll).
    #[must_use]
    pub fn next_deadline(self) -> Option<Instant> {
        match self.phase {
            PresencePhase::Hidden => None,
            PresencePhase::Pending { since } => since.checked_add(self.show_delay),
            PresencePhase::Visible { since } => self
                .visible_ttl
                .and_then(|ttl| since.checked_add(ttl)),
            PresencePhase::Exiting { since } => since.checked_add(self.exit_duration),
        }
    }

    /// Animation demand while present.
    #[must_use]
    pub fn demand(self) -> AnimationDemand {
        match self.next_deadline() {
            Some(d) => AnimationDemand {
                needs_redraw: true,
                next_deadline: Some(d),
            },
            None => AnimationDemand::idle(),
        }
    }
}

/// Visibility transition from [`Presence::advance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PresenceChange {
    /// No paint visibility change.
    #[default]
    None,
    /// Became painted.
    BecameVisible,
    /// No longer painted.
    BecameHidden,
}

/// FrameTick helpers for motion.
impl FrameTick {
    /// Milliseconds into a repeating period.
    #[must_use]
    pub fn phase_ms(self, period_ms: u64) -> u64 {
        let p = period_ms.max(1);
        self.elapsed().as_millis() as u64 % p
    }

    /// Spinner frame index.
    #[must_use]
    pub fn spinner_step(self, frame_count: usize, period_ms: u64, motion: Motion) -> usize {
        spinner_step(self, frame_count, period_ms, motion)
    }

    /// Pulse fraction 0..1.
    #[must_use]
    pub fn pulse_fraction(self, period_ms: u64, motion: Motion) -> f64 {
        pulse_fraction(self, period_ms, motion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::FrameClock;

    fn tick_at(clock: &mut FrameClock, start: Instant, ms: u64) -> FrameTick {
        clock.tick_at(start + Duration::from_millis(ms))
    }

    #[test]
    fn spinner_static_when_motion_off() {
        let start = Instant::now();
        let tick = FrameTick::manual(start + Duration::from_millis(500), Duration::from_millis(500), Duration::from_millis(16));
        assert_eq!(spinner_step(tick, 4, 80, Motion::Off), 0);
        assert!(spinner_step(tick, 4, 80, Motion::Full) > 0);
    }

    #[test]
    fn presence_tooltip_delay_not_focusable() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut p = Presence::tooltip(Duration::from_millis(400));
        let t0 = tick_at(&mut clock, start, 0);
        p.request_show(t0);
        assert!(!p.is_visible());
        assert!(!p.is_focusable());
        let t1 = tick_at(&mut clock, start, 200);
        assert_eq!(p.advance(t1, Motion::Full), PresenceChange::None);
        let t2 = tick_at(&mut clock, start, 400);
        assert_eq!(p.advance(t2, Motion::Full), PresenceChange::BecameVisible);
        assert!(p.is_focusable());
    }

    #[test]
    fn presence_toast_ttl_hides() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut p = Presence::toast(Duration::from_secs(2));
        p.request_show(tick_at(&mut clock, start, 0));
        assert!(p.is_visible());
        let change = p.advance(tick_at(&mut clock, start, 2000), Motion::Full);
        assert_eq!(change, PresenceChange::BecameHidden);
        assert!(!p.is_focusable());
    }

    #[test]
    fn idle_spinner_demands_no_redraw() {
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        assert!(!spinner_demand(tick, Motion::Full, false).needs_redraw);
        assert!(spinner_demand(tick, Motion::Full, true).needs_redraw);
        assert!(!spinner_demand(tick, Motion::Off, true).needs_redraw);
    }

    #[test]
    fn earliest_deadline_picks_min() {
        let a = Instant::now();
        let b = a + Duration::from_secs(1);
        assert_eq!(earliest_deadline([Some(b), Some(a), None]), Some(a));
    }
}
