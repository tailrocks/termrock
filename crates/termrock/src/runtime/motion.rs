// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Deterministic motion helpers on top of [`FrameTick`].
//!
//! Law: never redraw idle screens solely for decorative animation. Hosts
//! poll [`AnimationDemand`] / [`Presence::next_deadline`] and only wake when
//! something timed is active and [`crate::style::MotionPolicy`] allows motion.
use std::time::Duration;

use super::Instant;

use crate::style::MotionPolicy;

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
    motion: MotionPolicy,
) -> usize {
    if frame_count == 0 {
        return 0;
    }
    if !motion.animate_spinners() {
        return 0;
    }
    let period = period_ms.max(1);
    let step = tick.elapsed().as_millis() as u64 / period;
    (step as usize) % frame_count
}

/// Demand for an active spinner while work is in progress.
#[must_use]
pub fn spinner_demand(tick: FrameTick, motion: MotionPolicy, active: bool) -> AnimationDemand {
    if !active || !motion.animate_spinners() {
        return AnimationDemand::idle();
    }
    // Wake about once per frame period (junie spinner cadence).
    let period = Duration::from_millis(80);
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
    /// When timed progression was paused, if any.
    paused_since: Option<Instant>,
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
            paused_since: None,
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
            paused_since: None,
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
            paused_since: None,
        }
    }

    /// Persistent toast until dismiss.
    #[must_use]
    pub const fn persistent() -> Self {
        Self::immediate()
    }

    /// Give this presence an exit phase.
    ///
    /// Every constructor starts at `Duration::ZERO`, which made
    /// [`PresencePhase::Exiting`] unreachable — a surface could only vanish.
    /// The motion SoT §6 asks overlays to fade out over ~120 ms; the tier still
    /// decides, so `MotionPolicy::Off` collapses this to an instant hide.
    #[must_use]
    pub const fn with_exit(mut self, duration: Duration) -> Self {
        self.exit_duration = duration;
        self
    }

    /// Progress `0.0..=1.0` through the current timed phase.
    ///
    /// `Pending` counts toward the show delay, `Exiting` toward the exit, and
    /// `Visible` toward its TTL. Untimed phases report `1.0` — fully arrived —
    /// so a caller can multiply an alpha by this without special cases.
    #[must_use]
    pub fn phase_fraction(self, tick: FrameTick) -> f32 {
        let (since, span) = match self.phase {
            PresencePhase::Hidden => return 1.0,
            PresencePhase::Pending { since } => (since, self.show_delay),
            PresencePhase::Visible { since } => match self.visible_ttl {
                Some(ttl) => (since, ttl),
                None => return 1.0,
            },
            PresencePhase::Exiting { since } => (since, self.exit_duration),
        };
        if span.is_zero() {
            return 1.0;
        }
        let now = self.paused_since.unwrap_or(tick.now());
        let elapsed = now.saturating_duration_since(since);
        (elapsed.as_secs_f32() / span.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// Paint alpha for a fading exit, `1.0` whenever the surface is not leaving.
    #[must_use]
    pub fn exit_alpha(self, tick: FrameTick) -> f32 {
        match self.phase {
            PresencePhase::Exiting { .. } => 1.0 - self.phase_fraction(tick),
            _ => 1.0,
        }
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
        if self.paused_since.is_some() {
            self.paused_since = Some(tick.now());
        }
    }

    /// Request hide (instant or Exiting).
    pub fn request_hide(&mut self, tick: FrameTick, motion: MotionPolicy) {
        if matches!(
            self.phase,
            PresencePhase::Hidden | PresencePhase::Pending { .. }
        ) {
            // A surface that never became visible has no painted transition
            // to animate. Cancellation must stay hidden, including when an
            // exit duration is configured.
            self.phase = PresencePhase::Hidden;
            self.paused_since = None;
            return;
        }
        // Exits are transitions, not spinners: `Basic` still fades.
        if self.exit_duration.is_zero() || !motion.allows_transitions() {
            self.phase = PresencePhase::Hidden;
            self.paused_since = None;
        } else {
            self.phase = PresencePhase::Exiting { since: tick.now() };
            if self.paused_since.is_some() {
                self.paused_since = Some(tick.now());
            }
        }
    }

    /// Hard clear (no exit phase).
    pub const fn force_hide(&mut self) {
        self.phase = PresencePhase::Hidden;
        self.paused_since = None;
    }

    /// Pause timed progression at this frame.
    ///
    /// The timestamp is required so resuming can extend every timed phase by
    /// the exact time spent paused. Pending show and exit phases use the same
    /// origin shift as visible TTLs.
    pub fn set_paused(&mut self, tick: FrameTick, on: bool) {
        match (self.paused_since, on) {
            (None, true) => self.paused_since = Some(tick.now()),
            (Some(paused_since), false) => {
                let paused_for = tick.now().saturating_duration_since(paused_since);
                self.phase = match self.phase {
                    PresencePhase::Hidden => PresencePhase::Hidden,
                    PresencePhase::Pending { since } => PresencePhase::Pending {
                        since: since.checked_add(paused_for).unwrap_or(since),
                    },
                    PresencePhase::Visible { since } => PresencePhase::Visible {
                        since: since.checked_add(paused_for).unwrap_or(since),
                    },
                    PresencePhase::Exiting { since } => PresencePhase::Exiting {
                        since: since.checked_add(paused_for).unwrap_or(since),
                    },
                };
                self.paused_since = None;
            }
            (Some(_), true) | (None, false) => {}
        }
    }

    /// Whether timed progression is paused.
    #[must_use]
    pub const fn is_paused(self) -> bool {
        self.paused_since.is_some()
    }

    /// Advance phase from frame time; returns whether visibility changed.
    pub fn advance(&mut self, tick: FrameTick, motion: MotionPolicy) -> PresenceChange {
        if self.is_paused() {
            return PresenceChange::None;
        }
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
        if self.is_paused() {
            return None;
        }
        match self.phase {
            PresencePhase::Hidden => None,
            PresencePhase::Pending { since } => since.checked_add(self.show_delay),
            PresencePhase::Visible { since } => {
                self.visible_ttl.and_then(|ttl| since.checked_add(ttl))
            }
            PresencePhase::Exiting { since } => since.checked_add(self.exit_duration),
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
    /// Spinner frame index.
    #[must_use]
    pub fn spinner_step(self, frame_count: usize, period_ms: u64, motion: MotionPolicy) -> usize {
        spinner_step(self, frame_count, period_ms, motion)
    }
}

#[cfg(test)]
mod tests {
    use super::super::FrameClock;
    use super::*;

    fn tick_at(clock: &mut FrameClock, start: Instant, ms: u64) -> FrameTick {
        clock.tick_at(start + Duration::from_millis(ms))
    }

    #[test]
    fn spinner_static_when_motion_off() {
        let start = Instant::now();
        let tick = FrameTick::manual(
            start + Duration::from_millis(500),
            Duration::from_millis(500),
            Duration::from_millis(16),
        );
        assert_eq!(spinner_step(tick, 4, 80, MotionPolicy::Off), 0);
        assert!(spinner_step(tick, 4, 80, MotionPolicy::Full) > 0);
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
        assert_eq!(p.advance(t1, MotionPolicy::Full), PresenceChange::None);
        let t2 = tick_at(&mut clock, start, 400);
        assert_eq!(
            p.advance(t2, MotionPolicy::Full),
            PresenceChange::BecameVisible
        );
        assert!(p.is_focusable());
    }

    #[test]
    fn presence_toast_ttl_hides() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut p = Presence::toast(Duration::from_secs(2));
        p.request_show(tick_at(&mut clock, start, 0));
        assert!(p.is_visible());
        let change = p.advance(tick_at(&mut clock, start, 2000), MotionPolicy::Full);
        assert_eq!(change, PresenceChange::BecameHidden);
        assert!(!p.is_focusable());
    }

    #[test]
    fn presence_pause_shifts_timed_phase_origin() {
        let start = Instant::now();
        let mut p = Presence::toast(Duration::from_secs(2));
        p.request_show(FrameTick::manual(start, Duration::ZERO, Duration::ZERO));
        p.set_paused(
            FrameTick::manual(
                start + Duration::from_secs(1),
                Duration::from_secs(1),
                Duration::from_secs(1),
            ),
            true,
        );
        assert!(p.is_paused());
        assert_eq!(p.next_deadline(), None);
        p.set_paused(
            FrameTick::manual(
                start + Duration::from_secs(10),
                Duration::from_secs(10),
                Duration::from_secs(9),
            ),
            false,
        );
        assert!(!p.is_paused());
        assert_eq!(
            p.phase(),
            PresencePhase::Visible {
                since: start + Duration::from_secs(9)
            }
        );
        assert_eq!(p.next_deadline(), Some(start + Duration::from_secs(11)));
    }

    #[test]
    fn exit_phase_is_reachable_and_fades() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut presence =
            Presence::toast(Duration::from_secs(2)).with_exit(Duration::from_millis(120));

        presence.request_show(tick_at(&mut clock, start, 0));
        presence.request_hide(tick_at(&mut clock, start, 10), MotionPolicy::Full);
        assert!(
            matches!(presence.phase(), PresencePhase::Exiting { .. }),
            "every constructor used to zero the exit duration, so Exiting was unreachable"
        );
        assert!(presence.is_visible(), "an exiting surface is still painted");
        assert!(!presence.is_focusable(), "but it must not accept focus");

        let mid = tick_at(&mut clock, start, 70);
        let alpha = presence.exit_alpha(mid);
        assert!(alpha > 0.0 && alpha < 1.0, "exit alpha stuck at {alpha}");

        let after = tick_at(&mut clock, start, 200);
        assert_eq!(
            presence.advance(after, MotionPolicy::Full),
            PresenceChange::BecameHidden
        );
    }

    #[test]
    fn exits_follow_the_transition_tier_not_the_spinner_tier() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let build = || Presence::immediate().with_exit(Duration::from_millis(120));

        // `Off` hides instantly: the tier owns the exit, not the spinner.
        let mut off = build();
        off.request_show(tick_at(&mut clock, start, 0));
        off.request_hide(tick_at(&mut clock, start, 1), MotionPolicy::Off);
        assert_eq!(off.phase(), PresencePhase::Hidden);
    }

    #[test]
    fn cancelling_pending_presence_never_paints_exit() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut presence =
            Presence::tooltip(Duration::from_millis(400)).with_exit(Duration::from_millis(120));

        presence.request_show(tick_at(&mut clock, start, 0));
        presence.request_hide(tick_at(&mut clock, start, 100), MotionPolicy::Full);

        assert_eq!(presence.phase(), PresencePhase::Hidden);
        assert!(!presence.is_visible());
        assert_eq!(presence.next_deadline(), None);
    }

    #[test]
    fn phase_fraction_reports_progress_through_timed_phases() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut presence = Presence::tooltip(Duration::from_millis(400));
        presence.request_show(tick_at(&mut clock, start, 0));

        let quarter = tick_at(&mut clock, start, 100);
        assert!((presence.phase_fraction(quarter) - 0.25).abs() < 0.01);
        assert_eq!(
            presence.exit_alpha(quarter),
            1.0,
            "only an exiting surface fades"
        );

        // An untimed visible phase is fully arrived.
        let mut visible = Presence::immediate();
        visible.request_show(tick_at(&mut clock, start, 200));
        assert_eq!(visible.phase_fraction(tick_at(&mut clock, start, 900)), 1.0);
    }

    #[test]
    fn idle_spinner_demands_no_redraw() {
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        assert!(!spinner_demand(tick, MotionPolicy::Full, false).needs_redraw);
        assert!(spinner_demand(tick, MotionPolicy::Full, true).needs_redraw);
        assert!(!spinner_demand(tick, MotionPolicy::Off, true).needs_redraw);
    }

    #[test]
    fn earliest_deadline_picks_min() {
        let a = Instant::now();
        let b = a + Duration::from_secs(1);
        assert_eq!(earliest_deadline([Some(b), Some(a), None]), Some(a));
    }
}
