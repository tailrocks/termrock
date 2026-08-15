// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Layer 1 — typed value animation (`docs/design/tui-motion-system.md` §4).
//!
//! This layer animates *numbers*: scroll offsets, progress values, widths,
//! counters. Layer 2 (buffer effects) is a separate, later concern; nothing
//! here touches cells.
//!
//! Two shapes, chosen by whether the target can move mid-flight:
//!
//! - [`Tween`] for a known start and end over a known duration.
//! - [`Spring`] for values that retarget while running — streaming progress,
//!   a counter that keeps climbing. A tween would restart on every retarget and
//!   pop; a spring keeps its velocity and simply bends toward the new target.
//!
//! The kernel owns the clock: hosts advance an [`Animator`] once per frame with
//! the frame delta. An empty animator asks for no frames at all, which is what
//! keeps an idle screen at zero fps.

use std::collections::HashMap;
use std::time::Duration;

use crate::style::Easing;

use super::FrameRate;

/// Caller-assigned animation identity.
///
/// Hosts key animations by their own meaning (row 7's height, the progress of
/// job 3), so an update retargets the right one instead of stacking duplicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AnimId(pub u64);

/// A value moving from `from` to `to` over a fixed duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    from: f32,
    to: f32,
    elapsed: Duration,
    duration: Duration,
    easing: Easing,
}

impl Tween {
    /// Animate `from → to` over `(milliseconds, easing)`.
    ///
    /// A zero duration is legal and lands on `to` immediately — that is how
    /// `MotionPolicy::Off` collapses a transition without a special case at
    /// every call site.
    #[must_use]
    pub const fn to(from: f32, to: f32, spec: (u64, Easing)) -> Self {
        Self {
            from,
            to,
            elapsed: Duration::ZERO,
            duration: Duration::from_millis(spec.0),
            easing: spec.1,
        }
    }

    /// Current value.
    #[must_use]
    pub fn value(&self) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }
        let t = (self.elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * self.easing.apply(t)
    }

    /// Target value.
    #[must_use]
    pub const fn target(&self) -> f32 {
        self.to
    }

    /// Whether the tween has reached its target.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// Advance by a frame delta and return the new value.
    pub fn advance(&mut self, dt: Duration) -> f32 {
        self.elapsed = (self.elapsed + dt).min(self.duration);
        self.value()
    }

    /// Aim at a new target, starting from wherever the value is now.
    ///
    /// Honest but not smooth — the rate resets. Anything that retargets often
    /// wants a [`Spring`].
    pub fn retarget(&mut self, to: f32) {
        self.from = self.value();
        self.to = to;
        self.elapsed = Duration::ZERO;
    }
}

/// Critically damped spring — retarget-safe (harmonica model, §4/§5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    position: f32,
    velocity: f32,
    target: f32,
    frequency: f32,
    damping: f32,
}

/// Default angular frequency for retargeting values (§5).
pub const SPRING_FREQUENCY: f32 = 18.0;
/// Critical damping — settles without overshoot, which a cell grid cannot show.
pub const SPRING_DAMPING: f32 = 1.0;
/// Below this distance and speed the spring is considered settled.
const SPRING_EPSILON: f32 = 0.001;

impl Spring {
    /// A spring resting at `value`.
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self {
            position: value,
            velocity: 0.0,
            target: value,
            frequency: SPRING_FREQUENCY,
            damping: SPRING_DAMPING,
        }
    }

    /// Override frequency and damping.
    ///
    /// Damping below 1.0 overshoots; on a cell grid that quantizes to a row
    /// popping past its target, so the default stays critically damped.
    #[must_use]
    pub const fn tuned(mut self, frequency: f32, damping: f32) -> Self {
        self.frequency = frequency;
        self.damping = damping;
        self
    }

    /// Current value.
    #[must_use]
    pub const fn value(&self) -> f32 {
        self.position
    }

    /// Target value.
    #[must_use]
    pub const fn target(&self) -> f32 {
        self.target
    }

    /// Aim at a new target, keeping current velocity — no restart pop.
    pub const fn retarget(&mut self, target: f32) {
        self.target = target;
    }

    /// Snap to a value, cancelling motion (reduced-motion path).
    pub const fn settle(&mut self, value: f32) {
        self.position = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// Whether the spring has come to rest at its target.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        (self.target - self.position).abs() < SPRING_EPSILON && self.velocity.abs() < SPRING_EPSILON
    }

    /// Advance by a frame delta and return the new value.
    ///
    /// Semi-implicit Euler: stable at the frame deltas a terminal sees, and it
    /// cannot explode when a frame is late (the step is clamped).
    pub fn advance(&mut self, dt: Duration) -> f32 {
        // A stalled frame must not launch the spring across the screen.
        let dt = dt.as_secs_f32().min(1.0 / 30.0);
        if dt <= 0.0 {
            return self.position;
        }
        let stiffness = self.frequency * self.frequency;
        let damping = 2.0 * self.damping * self.frequency;
        let acceleration = stiffness * (self.target - self.position) - damping * self.velocity;
        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;
        if self.is_settled() {
            self.position = self.target;
            self.velocity = 0.0;
        }
        self.position
    }
}

/// One animated value in an [`Animator`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Animation {
    /// Fixed-duration transition.
    Tween(Tween),
    /// Retarget-safe physical settle.
    Spring(Spring),
}

impl Animation {
    /// Current value.
    #[must_use]
    pub fn value(&self) -> f32 {
        match self {
            Self::Tween(tween) => tween.value(),
            Self::Spring(spring) => spring.value(),
        }
    }

    /// Whether this animation has finished.
    #[must_use]
    pub fn is_done(&self) -> bool {
        match self {
            Self::Tween(tween) => tween.is_done(),
            Self::Spring(spring) => spring.is_settled(),
        }
    }

    fn advance(&mut self, dt: Duration) -> f32 {
        match self {
            Self::Tween(tween) => tween.advance(dt),
            Self::Spring(spring) => spring.advance(dt),
        }
    }
}

/// The kernel's set of running value animations.
///
/// Finished animations are dropped as they complete, so `is_empty` is a truthful
/// "nothing is moving" and [`Self::frame_rate`] can hand the presenter
/// [`FrameRate::Idle`] the moment the last one settles.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Animator {
    running: HashMap<AnimId, Animation>,
}

impl Animator {
    /// Empty animator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start (or replace) the animation under `id`.
    pub fn insert(&mut self, id: AnimId, animation: Animation) {
        self.running.insert(id, animation);
    }

    /// Retarget an existing spring, or start one at `value`.
    ///
    /// This is the streaming-progress path: the target moves constantly and the
    /// value must not restart on each update.
    pub fn retarget_spring(&mut self, id: AnimId, value: f32) {
        match self.running.get_mut(&id) {
            Some(Animation::Spring(spring)) => spring.retarget(value),
            _ => {
                let mut spring = Spring::new(value);
                spring.retarget(value);
                self.running.insert(id, Animation::Spring(spring));
            }
        }
    }

    /// Stop and forget one animation.
    pub fn remove(&mut self, id: AnimId) -> Option<Animation> {
        self.running.remove(&id)
    }

    /// Current value under `id`, if it is running.
    #[must_use]
    pub fn value(&self, id: AnimId) -> Option<f32> {
        self.running.get(&id).map(Animation::value)
    }

    /// Whether nothing is running.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.running.is_empty()
    }

    /// How many animations are running.
    #[must_use]
    pub fn len(&self) -> usize {
        self.running.len()
    }

    /// Frame-rate rung this animator needs — `Idle` when empty.
    #[must_use]
    pub fn frame_rate(&self) -> FrameRate {
        if self.running.is_empty() {
            FrameRate::Idle
        } else {
            FrameRate::Active
        }
    }

    /// Advance every animation and report the new values.
    ///
    /// Finished animations emit their final value once and are then dropped, so
    /// a caller never has to poll for completion.
    pub fn tick(&mut self, dt: Duration) -> Vec<(AnimId, f32)> {
        let mut out = Vec::with_capacity(self.running.len());
        let mut finished = Vec::new();
        for (id, animation) in &mut self.running {
            let value = animation.advance(dt);
            out.push((*id, value));
            if animation.is_done() {
                finished.push(*id);
            }
        }
        for id in finished {
            self.running.remove(&id);
        }
        out.sort_by_key(|(id, _)| *id);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Duration = Duration::from_millis(16);

    #[test]
    fn tween_runs_from_start_to_target() {
        let mut tween = Tween::to(0.0, 10.0, (100, Easing::Linear));
        assert_eq!(tween.value(), 0.0);
        assert!((tween.advance(Duration::from_millis(50)) - 5.0).abs() < 0.01);
        assert!(!tween.is_done());
        assert!((tween.advance(Duration::from_millis(50)) - 10.0).abs() < 0.001);
        assert!(tween.is_done());
    }

    #[test]
    fn zero_duration_tween_lands_immediately() {
        // How `MotionPolicy::Off` collapses a transition without a branch at
        // every call site.
        let tween = Tween::to(0.0, 10.0, (0, Easing::CubicOut));
        assert_eq!(tween.value(), 10.0);
        assert!(tween.is_done());
    }

    #[test]
    fn spring_retarget_does_not_pop() {
        let mut spring = Spring::new(0.0);
        spring.retarget(10.0);
        for _ in 0..6 {
            spring.advance(FRAME);
        }
        let mid = spring.value();
        assert!(mid > 0.0 && mid < 10.0, "spring should be in flight: {mid}");

        // Retarget mid-flight: the value must continue from where it is, not
        // jump back to a new start.
        spring.retarget(20.0);
        let after = spring.advance(FRAME);
        assert!(
            after >= mid,
            "retarget restarted the spring: {mid} then {after}"
        );
        assert!(after < 20.0);
    }

    #[test]
    fn spring_settles_without_overshoot() {
        let mut spring = Spring::new(0.0);
        spring.retarget(1.0);
        let mut peak = 0.0_f32;
        for _ in 0..200 {
            peak = peak.max(spring.advance(FRAME));
        }
        assert!(
            spring.is_settled(),
            "spring never settled: {}",
            spring.value()
        );
        assert!(
            peak <= 1.0 + f32::EPSILON,
            "critically damped spring overshot to {peak}"
        );
        assert!((spring.value() - 1.0).abs() < 0.01);
    }

    #[test]
    fn late_frame_does_not_launch_the_spring() {
        let mut spring = Spring::new(0.0);
        spring.retarget(1.0);
        let value = spring.advance(Duration::from_secs(5));
        assert!(
            (0.0..=1.0).contains(&value),
            "a stalled frame threw the spring to {value}"
        );
    }

    #[test]
    fn empty_animator_asks_for_no_frames() {
        let mut animator = Animator::new();
        assert!(animator.is_empty());
        assert_eq!(animator.frame_rate(), FrameRate::Idle);
        assert!(animator.tick(FRAME).is_empty());
    }

    #[test]
    fn animator_drops_finished_animations() {
        let mut animator = Animator::new();
        animator.insert(
            AnimId(1),
            Animation::Tween(Tween::to(0.0, 1.0, (32, Easing::Linear))),
        );
        assert_eq!(animator.frame_rate(), FrameRate::Active);

        assert_eq!(animator.tick(FRAME).len(), 1);
        assert_eq!(animator.len(), 1, "still running at half the duration");

        let last = animator.tick(FRAME);
        assert_eq!(last, vec![(AnimId(1), 1.0)], "final value is reported once");
        assert!(animator.is_empty());
        assert_eq!(
            animator.frame_rate(),
            FrameRate::Idle,
            "a settled animator must stop costing frames"
        );
    }

    #[test]
    fn retarget_spring_keeps_one_entry_per_id() {
        let mut animator = Animator::new();
        animator.retarget_spring(AnimId(7), 10.0);
        animator.retarget_spring(AnimId(7), 20.0);
        assert_eq!(
            animator.len(),
            1,
            "a retarget must not stack a second spring"
        );
        // The spring was created at 10.0 and re-aimed at 20.0; the *value*
        // stays where it is until it is advanced.
        assert_eq!(animator.value(AnimId(7)), Some(10.0));
    }
}
