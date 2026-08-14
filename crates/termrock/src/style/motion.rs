// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Motion policy, channel vocabulary, and pure frame-driven helpers.
//!
//! Implements `docs/design/tui-motion-system.md` §3 (tiers), §4 (the glue
//! rule), and the ambient half of §5. Callers supply deterministic ticks;
//! nothing here reads a clock, so every helper is snapshot-testable.
//!
//! **The glue rule (§4):** ambient loops phase on *wall clock* (they must
//! survive an fps change), transitions advance on *tick counts*. Both bases are
//! exposed — [`MotionChannel::phase`] for the former, elapsed-delta arithmetic
//! for the latter — and mixing them up is the bug the rule exists to prevent.

use std::time::Duration;

use ratatui_core::style::{Color, Style};

use crate::runtime::FrameRate;

/// Motion tier (`docs/design/tui-motion-system.md` §3).
///
/// Reduced is never frozen: status stays readable at every tier. `Basic` keeps
/// transitions and parks ambient loops at their bright end; `Off` makes state
/// changes instant and leans on glyph, verb, and weight instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum MotionPolicy {
    /// Everything: ambient loops, transitions, spinners.
    #[default]
    Full,
    /// Transitions ≤ 120 ms only; ambient loops static at the bright end.
    Basic,
    /// Instant state changes; status carried by non-motion channels.
    Off,
}

/// Longest transition `Basic` will run (§3).
pub const BASIC_TRANSITION_CAP: Duration = Duration::from_millis(120);

impl MotionPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Basic => "basic",
            Self::Off => "off",
        }
    }

    /// Parse `full` / `basic` / `none` (also `off`, `reduced`, `1`, `true`).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" | "all" | "on" => Some(Self::Full),
            "basic" | "reduced" | "1" | "true" => Some(Self::Basic),
            "none" | "off" | "0" | "false" => Some(Self::Off),
            _ => None,
        }
    }

    /// Resolve from the environment.
    ///
    /// `TERMROCK_ANIMATIONS` wins because it is the explicit, TermRock-specific
    /// answer; `REDUCE_MOTION` (any non-empty value that is not a disable word)
    /// then downgrades to [`Self::Basic`]. Hosts may override afterwards —
    /// this reads the environment once, at startup, never during paint.
    #[must_use]
    pub fn from_env() -> Self {
        if let Ok(raw) = std::env::var("TERMROCK_ANIMATIONS")
            && let Some(policy) = Self::parse(&raw)
        {
            return policy;
        }
        if let Ok(raw) = std::env::var("REDUCE_MOTION") {
            return match Self::parse(&raw) {
                // `REDUCE_MOTION=0`/`false` means "do not reduce".
                Some(Self::Off) => Self::Full,
                Some(_) | None if !raw.trim().is_empty() => Self::Basic,
                _ => Self::Full,
            };
        }
        Self::Full
    }

    /// Whether indeterminate spinners should advance frames.
    #[must_use]
    pub const fn animate_spinners(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Frame step divisor for reduced motion (slower advance).
    #[must_use]
    pub const fn spinner_divisor(self) -> u64 {
        match self {
            Self::Full => 1,
            Self::Basic => 4,
            Self::Off => u64::MAX,
        }
    }

    /// Whether state changes may cross-fade rather than snap.
    ///
    /// `animate_spinners` used to answer this by accident, which forced
    /// transitions to be gated on a spinner-shaped question.
    #[must_use]
    pub const fn allows_transitions(self) -> bool {
        matches!(self, Self::Full | Self::Basic)
    }

    /// Whether ambient loops (breathe, shimmer, wave) may run.
    #[must_use]
    pub const fn allows_ambient(self) -> bool {
        matches!(self, Self::Full)
    }

    /// Clamp a transition duration to this tier.
    ///
    /// `Off` collapses every transition to zero; `Basic` caps at
    /// [`BASIC_TRANSITION_CAP`]; `Full` passes the duration through.
    #[must_use]
    pub const fn clamp_duration(self, duration: Duration) -> Duration {
        match self {
            Self::Full => duration,
            Self::Basic => {
                if duration.as_millis() > BASIC_TRANSITION_CAP.as_millis() {
                    BASIC_TRANSITION_CAP
                } else {
                    duration
                }
            }
            Self::Off => Duration::ZERO,
        }
    }
}

/// What a moving thing is *saying* (`docs/design/tui-motion-system.md` §4).
///
/// Every animated surface declares a channel instead of inventing tick math.
/// The channel fixes the period and the frame-rate rung, so two widgets in the
/// same state breathe together instead of drifting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[non_exhaustive]
pub enum MotionChannel {
    /// Work is in progress and bounded (spinner, braille frames).
    Work,
    /// Waiting on something external, no progress to report (dot pulse).
    Wait,
    /// Content is arriving (shimmer, caret).
    Stream,
    /// Alive and idle — presence, not progress (slow breathe).
    Live,
    /// Terminal state: done, failed, offline. Gravity — never animates.
    #[default]
    Static,
}

/// Period of the presence heartbeat (§1) — slower than the [`MotionChannel::Live`] breathe.
pub const HEARTBEAT_PERIOD_MS: u64 = 5_000;

impl MotionChannel {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Wait => "wait",
            Self::Stream => "stream",
            Self::Live => "live",
            Self::Static => "static",
        }
    }

    /// Loop period in milliseconds (`0` for [`Self::Static`]).
    #[must_use]
    pub const fn period_ms(self) -> u64 {
        match self {
            Self::Work => 80,
            Self::Wait => 240,
            Self::Stream => 120,
            Self::Live => 2_000,
            Self::Static => 0,
        }
    }

    /// Frame-rate rung this channel needs while it runs.
    #[must_use]
    pub const fn frame_rate(self) -> FrameRate {
        match self {
            Self::Work | Self::Stream => FrameRate::Active,
            Self::Wait | Self::Live => FrameRate::Ambient,
            Self::Static => FrameRate::Idle,
        }
    }

    /// Wall-clock phase in `0.0..1.0` for this channel.
    ///
    /// Ambient loops phase on elapsed wall time so the animation keeps its
    /// shape when the frame rate changes (§4, §7 anti-pattern 7).
    #[must_use]
    pub fn phase(self, elapsed_ms: u64) -> f32 {
        let period = self.period_ms();
        if period == 0 {
            return 0.0;
        }
        (elapsed_ms % period) as f32 / period as f32
    }

    /// Frame index for a frame-based channel (spinners).
    #[must_use]
    pub fn frame(self, elapsed_ms: u64, frames: usize) -> usize {
        if frames == 0 || self.period_ms() == 0 {
            return 0;
        }
        ((elapsed_ms / self.period_ms()) as usize) % frames
    }
}

/// Easing curves permitted on a cell grid (`docs/design/tui-motion-system.md` §5).
///
/// **No overshoot family.** `Elastic`, `Bounce`, and strong `Back` curves are
/// deliberately absent: on a grid an overshoot quantizes to a row popping past
/// its target and back, plus a color flicker on the way. There is no variant to
/// add them with, which is the point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Easing {
    /// Constant rate — micro feedback only.
    #[default]
    Linear,
    /// Gentle start.
    SineIn,
    /// Gentle stop — the default for anything entering.
    SineOut,
    /// Symmetric, gentle at both ends.
    SineInOut,
    /// Accelerating.
    QuadIn,
    /// Decelerating.
    QuadOut,
    /// Symmetric quadratic.
    QuadInOut,
    /// Sharp acceleration — exits.
    CubicIn,
    /// Fast start, long settle — entrances and scroll.
    CubicOut,
    /// Symmetric cubic — screen transitions.
    CubicInOut,
}

impl Easing {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::SineIn => "sine-in",
            Self::SineOut => "sine-out",
            Self::SineInOut => "sine-in-out",
            Self::QuadIn => "quad-in",
            Self::QuadOut => "quad-out",
            Self::QuadInOut => "quad-in-out",
            Self::CubicIn => "cubic-in",
            Self::CubicOut => "cubic-out",
            Self::CubicInOut => "cubic-in-out",
        }
    }

    /// Every curve, in catalog order.
    pub const ALL: [Self; 10] = [
        Self::Linear,
        Self::SineIn,
        Self::SineOut,
        Self::SineInOut,
        Self::QuadIn,
        Self::QuadOut,
        Self::QuadInOut,
        Self::CubicIn,
        Self::CubicOut,
        Self::CubicInOut,
    ];

    /// Map linear progress `0.0..=1.0` onto the curve.
    ///
    /// Every curve is anchored at `f(0) = 0` and `f(1) = 1` and stays inside
    /// `0..=1` throughout — that containment is what keeps geometry from
    /// popping.
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        use std::f32::consts::PI;
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::SineIn => 1.0 - (t * PI / 2.0).cos(),
            Self::SineOut => (t * PI / 2.0).sin(),
            Self::SineInOut => 0.5 * (1.0 - (PI * t).cos()),
            Self::QuadIn => t * t,
            Self::QuadOut => t * (2.0 - t),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            Self::CubicIn => t * t * t,
            Self::CubicOut => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let inv = -2.0 * t + 2.0;
                    1.0 - inv * inv * inv / 2.0
                }
            }
        }
    }
}

/// Shortest scroll ease (§5: short hops read instant).
pub const SCROLL_EASE_MIN: Duration = Duration::from_millis(80);
/// Longest scroll ease (§5: long jumps stay capped).
pub const SCROLL_EASE_MAX: Duration = Duration::from_millis(200);

/// Distance-scaled scroll duration (§5 scroll row).
///
/// A one-row hop must feel instant and a page jump must not feel sluggish, so
/// the duration scales with distance and saturates — never "one duration for
/// every scroll", which makes short hops laggy and long ones frantic.
#[must_use]
pub fn scroll_ease_duration(rows: u16) -> Duration {
    const SATURATION_ROWS: f32 = 40.0;
    let t = (f32::from(rows) / SATURATION_ROWS).clamp(0.0, 1.0);
    let span = SCROLL_EASE_MAX.as_millis() as f32 - SCROLL_EASE_MIN.as_millis() as f32;
    Duration::from_millis(SCROLL_EASE_MIN.as_millis() as u64 + (span * t) as u64)
}

/// Peak amplitude of any ambient loop (§1 peak restraint).
///
/// Ambient motion whispers: a third of the way toward the accent, never a
/// full-swing flash.
pub const AMBIENT_PEAK: f32 = 0.33;

#[must_use]
/// Temporal sine-squared pulse for a deterministic tick and period.
pub fn pulse_brightness(tick: u64, period: u64) -> f32 {
    if period == 0 {
        return 1.0;
    }
    let phase = (tick % period) as f32 / period as f32;
    (std::f32::consts::PI * phase).sin().powi(2)
}

#[must_use]
/// Spatial sine-squared wave flowing along a row axis.
pub fn wave_brightness(tick: u64, row: u16, wave_rows: u16, speed: f32) -> f32 {
    if wave_rows == 0 {
        return 1.0;
    }
    let phase = (f32::from(row) - tick as f32 * speed) / f32::from(wave_rows);
    (std::f32::consts::PI * phase).sin().powi(2).clamp(0.0, 1.0)
}

#[must_use]
/// Clamped cubic smoothstep over `0..=1`.
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[must_use]
/// Symmetric smooth edge alpha for a bounded row of cells.
pub fn edge_fade(col: u16, width: u16, fade_cols: u16) -> f32 {
    if width == 0 || fade_cols == 0 {
        return 1.0;
    }
    let left = f32::from(col.saturating_add(1)) / f32::from(fade_cols);
    let right = f32::from(width.saturating_sub(col)) / f32::from(fade_cols);
    smoothstep(left.min(right))
}

#[must_use]
/// Blend RGB `from` toward RGB `to`; preserve unsupported color forms.
pub fn blend_toward(from: Color, to: Color, t: f32) -> Color {
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => {
            let t = t.clamp(0.0, 1.0);
            let lerp =
                |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
            Color::Rgb(lerp(fr, tr), lerp(fg, tg), lerp(fb, tb))
        }
        (other, _) => other,
    }
}

#[must_use]
/// Blend every color channel in a style toward an explicit canvas color.
pub fn fade_style(mut style: Style, alpha: f32, canvas: Color) -> Style {
    let toward = 1.0 - alpha.clamp(0.0, 1.0);
    if let Some(color) = style.fg {
        style.fg = Some(blend_toward(color, canvas, toward));
    }
    if let Some(color) = style.bg {
        style.bg = Some(blend_toward(color, canvas, toward));
    }
    if let Some(color) = style.underline_color {
        style.underline_color = Some(blend_toward(color, canvas, toward));
    }
    style
}

/// Ambient brightness for a channel at this moment, in `0.0..=1.0`.
///
/// A sin² breathe whose trough is [`AMBIENT_PEAK`] below full — motion that
/// whispers (§1 peak restraint). Terminal channels and reduced tiers return
/// `1.0`, so a caller can multiply unconditionally and a settled state simply
/// stays at full brightness.
///
/// [`MotionChannel::Live`] on a presence dot wants the slower
/// [`HEARTBEAT_PERIOD_MS`]; pass it explicitly rather than the channel period.
#[must_use]
pub fn channel_brightness(policy: MotionPolicy, channel: MotionChannel, elapsed_ms: u64) -> f32 {
    if !policy.allows_ambient() || matches!(channel, MotionChannel::Static) {
        return 1.0;
    }
    breathe(channel.phase(elapsed_ms))
}

/// Same breathe over an explicit period (heartbeats, host-tuned loops).
#[must_use]
pub fn breathe_over(policy: MotionPolicy, elapsed_ms: u64, period_ms: u64) -> f32 {
    if !policy.allows_ambient() || period_ms == 0 {
        return 1.0;
    }
    breathe((elapsed_ms % period_ms) as f32 / period_ms as f32)
}

/// sin² over one phase, scaled into `1.0 - AMBIENT_PEAK ..= 1.0`.
fn breathe(phase: f32) -> f32 {
    let wave = (std::f32::consts::PI * phase).sin().powi(2);
    1.0 - AMBIENT_PEAK + AMBIENT_PEAK * wave
}

/// Raised-cosine shimmer band travelling across `cols` (§1, §6 skeletons).
///
/// The band peaks at [`AMBIENT_PEAK`] and falls to zero at its edges, so a
/// skeleton *sweeps* instead of pulsing as one block — the pulse reads as a
/// spinner, which the skeleton contract forbids.
///
/// Static under `Basic` (parked at the bright end, per §3) and dark under
/// `Off`, so two ticks render identical cells at either tier.
#[must_use]
pub fn shimmer_at(
    policy: MotionPolicy,
    elapsed_ms: u64,
    col: u16,
    cols: u16,
    period_ms: u64,
) -> f32 {
    match policy {
        MotionPolicy::Off => return 0.0,
        MotionPolicy::Basic => return AMBIENT_PEAK,
        MotionPolicy::Full => {}
    }
    if cols == 0 || period_ms == 0 {
        return 0.0;
    }
    let width = f32::from(cols);
    // The band starts fully off-screen on the left and leaves on the right, so
    // the sweep has a gap rather than wrapping mid-band.
    let half = (width / 3.0).max(1.0);
    let phase = (elapsed_ms % period_ms) as f32 / period_ms as f32;
    let center = -half + phase * (width + 2.0 * half);
    let distance = (f32::from(col) - center).abs();
    if distance >= half {
        return 0.0;
    }
    let falloff = 0.5 * (1.0 + (std::f32::consts::PI * distance / half).cos());
    AMBIENT_PEAK * falloff
}

/// Shimmer alpha for every column of a row.
///
/// Allocation-free: the iterator borrows nothing and is consumed during paint.
pub fn shimmer_cells(
    policy: MotionPolicy,
    elapsed_ms: u64,
    cols: u16,
    period_ms: u64,
) -> impl Iterator<Item = f32> {
    (0..cols).map(move |col| shimmer_at(policy, elapsed_ms, col, cols, period_ms))
}

#[must_use]
/// Resolve animated alpha to a static visible fallback under reduced motion.
pub fn effective_alpha(motion: MotionPolicy, animated: f32) -> f32 {
    if matches!(motion, MotionPolicy::Full) {
        animated.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

#[must_use]
/// Merge adjacent equal-style cells into minimal text runs.
pub fn coalesce_cells(cells: impl IntoIterator<Item = (char, Style)>) -> Vec<(String, Style)> {
    let mut runs: Vec<(String, Style)> = Vec::new();
    for (cell, style) in cells {
        if let Some((text, previous)) = runs.last_mut()
            && *previous == style
        {
            text.push(cell);
        } else {
            runs.push((cell.to_string(), style));
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_math_is_bounded_and_symmetric() {
        assert!(pulse_brightness(0, 8).abs() < 0.001);
        assert!((pulse_brightness(4, 8) - 1.0).abs() < 0.001);
        assert!((smoothstep(0.5) - 0.5).abs() < 0.001);
        assert_eq!(edge_fade(0, 10, 3), edge_fade(9, 10, 3));
        assert!((wave_brightness(3, 2, 8, 1.0) - wave_brightness(3, 10, 8, 1.0)).abs() < 0.001);
    }

    #[test]
    fn reduced_motion_is_static() {
        assert_eq!(effective_alpha(MotionPolicy::Basic, 0.2), 1.0);
        assert_eq!(effective_alpha(MotionPolicy::Off, 0.0), 1.0);
    }

    #[test]
    fn every_easing_is_anchored_and_contained() {
        for easing in Easing::ALL {
            assert!(
                easing.apply(0.0).abs() < 0.001,
                "{easing:?} does not start at 0"
            );
            assert!(
                (easing.apply(1.0) - 1.0).abs() < 0.001,
                "{easing:?} does not end at 1"
            );
            for step in 0..=100 {
                let t = step as f32 / 100.0;
                let v = easing.apply(t);
                assert!(
                    (-0.001..=1.001).contains(&v),
                    "{easing:?} overshoots at {t}: {v} — overshoot pops rows on a cell grid"
                );
            }
            // Clamped outside the domain rather than extrapolating.
            assert_eq!(easing.apply(-1.0), easing.apply(0.0));
            assert_eq!(easing.apply(2.0), easing.apply(1.0));
        }
    }

    #[test]
    fn scroll_duration_scales_with_distance_and_saturates() {
        assert_eq!(scroll_ease_duration(0), SCROLL_EASE_MIN);
        assert!(scroll_ease_duration(5) > SCROLL_EASE_MIN);
        assert!(scroll_ease_duration(5) < scroll_ease_duration(30));
        assert_eq!(scroll_ease_duration(40), SCROLL_EASE_MAX);
        assert_eq!(
            scroll_ease_duration(u16::MAX),
            SCROLL_EASE_MAX,
            "a page jump must not become sluggish"
        );
    }

    #[test]
    fn channels_map_to_periods_and_rungs() {
        assert_eq!(MotionChannel::Work.period_ms(), 80);
        assert_eq!(MotionChannel::Wait.period_ms(), 240);
        assert_eq!(MotionChannel::Stream.period_ms(), 120);
        assert_eq!(MotionChannel::Live.period_ms(), 2_000);
        assert_eq!(MotionChannel::Static.period_ms(), 0);

        assert_eq!(MotionChannel::Work.frame_rate(), FrameRate::Active);
        assert_eq!(MotionChannel::Stream.frame_rate(), FrameRate::Active);
        assert_eq!(MotionChannel::Wait.frame_rate(), FrameRate::Ambient);
        assert_eq!(MotionChannel::Live.frame_rate(), FrameRate::Ambient);
        assert_eq!(
            MotionChannel::Static.frame_rate(),
            FrameRate::Idle,
            "terminal states must not cost a single frame"
        );
    }

    #[test]
    fn ambient_phase_rides_wall_clock_not_frame_count() {
        // Same elapsed time, any frame rate: the phase is identical.
        let live = MotionChannel::Live;
        assert!((live.phase(0) - 0.0).abs() < 0.001);
        assert!((live.phase(1_000) - 0.5).abs() < 0.001);
        assert!((live.phase(3_000) - 0.5).abs() < 0.001, "phase must wrap");
        assert_eq!(MotionChannel::Static.phase(1_234), 0.0);
    }

    #[test]
    fn work_channel_indexes_spinner_frames() {
        assert_eq!(MotionChannel::Work.frame(0, 4), 0);
        assert_eq!(MotionChannel::Work.frame(80, 4), 1);
        assert_eq!(MotionChannel::Work.frame(400, 4), 1);
        assert_eq!(MotionChannel::Static.frame(400, 4), 0);
        assert_eq!(MotionChannel::Work.frame(400, 0), 0);
    }

    #[test]
    fn shimmer_band_travels_and_stays_within_peak() {
        let cols = 24;
        let period = 1_500;
        let sample =
            |ms| -> Vec<f32> { shimmer_cells(MotionPolicy::Full, ms, cols, period).collect() };
        // Sample two phases where the band is on screen; at phase 0 it is
        // still entirely off the left edge (that gap is intentional).
        let early = sample(period / 4);
        let mid = sample(period / 2);
        assert_ne!(early, mid, "the band must travel");

        let peak_col = |row: &[f32]| {
            row.iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(i, _)| i)
                .expect("non-empty row")
        };
        assert!(
            peak_col(&mid) > peak_col(&early),
            "the band must travel left to right"
        );
        for value in early.iter().chain(&mid) {
            assert!(
                (0.0..=AMBIENT_PEAK + f32::EPSILON).contains(value),
                "ambient motion must whisper: {value}"
            );
        }
    }

    #[test]
    fn shimmer_is_static_under_basic_and_off() {
        for policy in [MotionPolicy::Basic, MotionPolicy::Off] {
            let a: Vec<f32> = shimmer_cells(policy, 0, 24, 1_500).collect();
            let b: Vec<f32> = shimmer_cells(policy, 700, 24, 1_500).collect();
            assert_eq!(a, b, "{policy:?} shimmer moved");
            assert!(
                a.iter().all(|v| (*v - a[0]).abs() < f32::EPSILON),
                "{policy:?} shimmer is not flat"
            );
        }
        assert_eq!(
            shimmer_at(MotionPolicy::Basic, 0, 0, 24, 1_500),
            AMBIENT_PEAK
        );
        assert_eq!(shimmer_at(MotionPolicy::Off, 0, 0, 24, 1_500), 0.0);
    }

    #[test]
    fn policy_tiers_gate_transitions_and_ambient() {
        assert!(MotionPolicy::Full.allows_ambient());
        assert!(!MotionPolicy::Basic.allows_ambient());
        assert!(MotionPolicy::Basic.allows_transitions());
        assert!(!MotionPolicy::Off.allows_transitions());

        let long = Duration::from_millis(300);
        assert_eq!(MotionPolicy::Full.clamp_duration(long), long);
        assert_eq!(
            MotionPolicy::Basic.clamp_duration(long),
            BASIC_TRANSITION_CAP
        );
        assert_eq!(
            MotionPolicy::Basic.clamp_duration(Duration::from_millis(80)),
            Duration::from_millis(80),
            "a short transition is already within the cap"
        );
        assert_eq!(MotionPolicy::Off.clamp_duration(long), Duration::ZERO);
    }

    #[test]
    fn policy_parses_the_documented_spellings() {
        assert_eq!(MotionPolicy::parse("full"), Some(MotionPolicy::Full));
        assert_eq!(MotionPolicy::parse(" Basic "), Some(MotionPolicy::Basic));
        assert_eq!(MotionPolicy::parse("none"), Some(MotionPolicy::Off));
        assert_eq!(MotionPolicy::parse("sparkly"), None);
    }

    #[test]
    fn coalesces_equal_style_runs() {
        let a = Style::new().fg(Color::Red);
        let b = Style::new().fg(Color::Blue);
        let cells = (0..1000).map(|i| ('x', if i < 300 || i >= 700 { a } else { b }));
        assert_eq!(coalesce_cells(cells).len(), 3);
    }
}
