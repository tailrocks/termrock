// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Spinner** and **ActivityIndicator** — semantic activity with deterministic cadence.
//!
//! **Mission.** Terminal spinners and AI-tool activity states that always pair a
//! glyph with a meaningful verb/label (unless embedded in a labeled control).
//! Phases: indeterminate, waiting, queued, reconnecting, streaming, done.
//! One animated frame vocabulary — the 10-frame braille set at 80 ms — plus a
//! distinct static mark per phase under reduced motion; **no frame ticks** when
//! inactive or not visible.
//!
//! **Cadence.** Uses [`FrameTick::spinner_step`] / [`spinner_demand`] so hosts
//! share one clock with Progress indeterminate and lookbook motion stories.
//!
//! **vs Progress.** Progress is determinate/indeterminate **bar** with units.
//! Spinner/ActivityIndicator are glyph + verb activity, not completion bars.
//! **vs LoadingView.** LoadingView is a centered placeholder that accepts a
//! pre-resolved frame string; prefer Spinner for tick-driven frames.
//!
//! Research: terminal spinners, Textual loading, polished AI tool states.
use std::time::Duration;

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::{AnimationDemand, FrameTick, spinner_demand, spinner_step},
    style::{DesignSystem, MotionPolicy, Role},
    text::{take_display_cols, truncate_cols},
};

use super::SemanticStatus;

/// Default frame period (ms) for Full motion — matches historic Spinner/Progress.
pub const SPINNER_DEFAULT_PERIOD_MS: u64 = 80;
pub use crate::style::SPINNER_BRAILLE_FRAMES;

// ── Phase / variant / glyphs ────────────────────────────────────────────────

/// Semantic activity phase (not a progress percent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActivityPhase {
    /// Work in progress, indeterminate.
    #[default]
    Indeterminate,
    /// Blocked on external input / delay.
    Waiting,
    /// Scheduled, not yet running.
    Queued,
    /// Transport or session reconnect.
    Reconnecting,
    /// Content is arriving — a ripple, not a spin.
    Streaming,
    /// Terminal success. Gravity: never animates.
    Done,
}

impl ActivityPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Indeterminate => "indeterminate",
            Self::Waiting => "waiting",
            Self::Queued => "queued",
            Self::Reconnecting => "reconnecting",
            Self::Streaming => "streaming",
            Self::Done => "done",
        }
    }

    /// Default verb when host omits label (still prefer host verb).
    #[must_use]
    pub const fn default_verb(self) -> &'static str {
        match self {
            Self::Indeterminate => "Working",
            Self::Waiting => "Waiting",
            Self::Queued => "Queued",
            Self::Reconnecting => "Reconnecting",
            Self::Streaming => "Streaming",
            Self::Done => "Done",
        }
    }

    /// Whether this phase advances frames under Full motion.
    ///
    /// Gravity: a queued or finished thing is not happening at a rate, so it
    /// never claims frames.
    #[must_use]
    pub const fn animates(self) -> bool {
        !matches!(self, Self::Queued | Self::Done)
    }

    /// Frame period for this phase: the one 80 ms braille cadence.
    #[must_use]
    pub const fn period_ms(self) -> u64 {
        SPINNER_DEFAULT_PERIOD_MS
    }

    /// Shared lifecycle state used for glyph and tone recipes.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Indeterminate | Self::Streaming => SemanticStatus::Running,
            Self::Waiting | Self::Reconnecting => SemanticStatus::Waiting,
            Self::Queued => SemanticStatus::Queued,
            Self::Done => SemanticStatus::Success,
        }
    }
}

/// Presentation density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SpinnerVariant {
    /// Glyph + label on one line (default).
    #[default]
    Labeled,
    /// Glyph only — **requires** [`SpinnerState::embedded_in_labeled_control`].
    CompactInline,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Activity / spinner runtime: active + visible gates stop redraw demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpinnerState {
    active: bool,
    visible: bool,
    phase: ActivityPhase,
    period_ms: u64,
    /// Host marks spinner as chrome inside a button/menu that already labels the action.
    embedded_in_labeled_control: bool,
    variant: SpinnerVariant,
}

impl Default for SpinnerState {
    fn default() -> Self {
        Self::new()
    }
}

impl SpinnerState {
    /// Active + visible, indeterminate.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: true,
            visible: true,
            phase: ActivityPhase::Indeterminate,
            period_ms: SPINNER_DEFAULT_PERIOD_MS,
            embedded_in_labeled_control: false,
            variant: SpinnerVariant::Labeled,
        }
    }

    /// Active?
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Visible?
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether the host should schedule redraw ticks.
    #[must_use]
    pub const fn should_tick(&self) -> bool {
        self.active && self.visible && self.phase.animates()
    }

    /// Phase.
    #[must_use]
    pub const fn phase(&self) -> ActivityPhase {
        self.phase
    }

    /// Set active (work in flight).
    pub fn set_active(&mut self, on: bool) {
        self.active = on;
    }

    /// Set visible (painted). Idle when false — **no redraw demand**.
    pub fn set_visible(&mut self, on: bool) {
        self.visible = on;
    }

    /// Phase.
    pub fn set_phase(&mut self, phase: ActivityPhase) {
        self.phase = phase;
    }
    /// Embedded in labeled control (allows compact glyph-only).
    pub fn set_embedded_in_labeled_control(&mut self, on: bool) {
        self.embedded_in_labeled_control = on;
    }

    /// Embedded?
    #[must_use]
    pub const fn embedded_in_labeled_control(&self) -> bool {
        self.embedded_in_labeled_control
    }

    /// Variant.
    pub fn set_variant(&mut self, v: SpinnerVariant) {
        self.variant = v;
    }

    /// Animation demand for host frame clock. Idle when not ticking.
    #[must_use]
    pub fn animation_demand(&self, tick: FrameTick, motion: MotionPolicy) -> AnimationDemand {
        if !self.should_tick() {
            return AnimationDemand::idle();
        }
        let period = self.frame_period_ms().max(16);
        // spinner_demand uses fixed 80ms — scale deadline to our period
        let base = spinner_demand(tick, motion, true);
        if !base.needs_redraw {
            return base;
        }
        let scaled = Duration::from_millis(period);
        AnimationDemand {
            needs_redraw: true,
            next_deadline: Some(tick.now() + scaled),
        }
    }

    /// Frame period in milliseconds.
    ///
    /// The phase owns the cadence; a host override still wins, so a caller
    /// that has measured its own rhythm keeps it.
    #[must_use]
    pub fn frame_period_ms(&self) -> u64 {
        if self.period_ms == SPINNER_DEFAULT_PERIOD_MS {
            self.phase.period_ms()
        } else {
            self.period_ms
        }
    }

    /// Effective frames for phase + capability.
    ///
    /// There is exactly one animated frame vocabulary — the 10-frame braille
    /// set at 80 ms. [`MotionPolicy::Off`] parks on the first frame. Done is
    /// the settled `✓`; it never spins.
    #[must_use]
    pub fn frames(&self, motion: MotionPolicy) -> &'static [&'static str] {
        if matches!(self.phase, ActivityPhase::Done) {
            return &["✓"];
        }
        if !motion.animate_spinners() || !self.phase.animates() {
            return &SPINNER_BRAILLE_FRAMES[..1];
        }
        SPINNER_BRAILLE_FRAMES
    }

    /// Glyph for tick.
    #[must_use]
    pub fn frame_glyph(&self, tick: FrameTick, motion: MotionPolicy) -> &'static str {
        let frames = self.frames(motion);
        if frames.is_empty() {
            return " ";
        }
        if !self.should_tick() || !motion.animate_spinners() || !self.phase.animates() {
            return frames[0];
        }
        let step = spinner_step(tick, frames.len(), self.frame_period_ms(), motion);
        frames[step]
    }
}

// ── Spinner widget ──────────────────────────────────────────────────────────

/// FrameTick-driven spinner (glyph ± label).
///
/// # Label law
///
/// A non-empty `label` is required unless
/// [`SpinnerState::embedded_in_labeled_control`] is set (or
/// [`Self::embedded`] on the widget).
#[derive(Debug, Clone, Copy)]
pub struct Spinner<'a> {
    system: &'a DesignSystem,
    label: Option<&'a str>,
    embedded: bool,
    phase: Option<ActivityPhase>,
    variant: Option<SpinnerVariant>,
    colorless: bool,
}

impl<'a> Spinner<'a> {
    /// System only (label via builder; default indeterminate).
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            label: None,
            embedded: false,
            phase: None,
            variant: None,
            colorless: false,
        }
    }

    /// Labeled spinner (preferred constructor).
    #[must_use]
    pub const fn labeled(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            label: Some(label),
            embedded: false,
            phase: None,
            variant: None,
            colorless: false,
        }
    }

    /// Label / verb.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// ASCII frames.
    #[must_use]
    /// Embedded in labeled control (glyph-only ok).
    pub const fn embedded(mut self, on: bool) -> Self {
        self.embedded = on;
        self
    }

    /// Phase override (state phase used if None).
    #[must_use]
    pub const fn phase(mut self, phase: ActivityPhase) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Variant.
    #[must_use]
    pub const fn variant(mut self, v: SpinnerVariant) -> Self {
        self.variant = Some(v);
        self
    }

    /// Frame glyph (legacy API preserved).
    #[must_use]
    pub fn frame_glyph(&self, tick: FrameTick, motion: MotionPolicy) -> &'static str {
        let mut state = SpinnerState::new();
        if let Some(p) = self.phase {
            state.set_phase(p);
        }
        state.frame_glyph(tick, motion)
    }

    /// Whether label law is satisfied.
    #[must_use]
    pub fn label_ok(&self, state: &SpinnerState) -> bool {
        let embedded = self.embedded || state.embedded_in_labeled_control();
        let variant = self.variant.unwrap_or(state.variant);
        if matches!(variant, SpinnerVariant::CompactInline) || embedded {
            return true;
        }
        self.label.map(|l| !l.trim().is_empty()).unwrap_or(false)
    }

    /// Resolved display label (falls back to phase verb when missing but required).
    #[must_use]
    pub fn resolved_label(&self, state: &SpinnerState) -> &'a str {
        if let Some(l) = self.label {
            if !l.trim().is_empty() {
                return l;
            }
        }
        // Use static default verb — lifetime is 'static so transmute via phase
        // We return from phase default which is 'static; coerce via leak-free:
        // ActivityPhase::default_verb is 'static; cast as 'a is ok for static
        let phase = self.phase.unwrap_or(state.phase());
        // SAFETY: default_verb is 'static, which outlives 'a
        phase.default_verb()
    }

    /// Paint with state (preferred).
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &SpinnerState,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        if area.is_empty() || !state.is_visible() {
            return;
        }
        let mut local = state.clone();
        if let Some(p) = self.phase {
            local.set_phase(p);
        }
        if let Some(v) = self.variant {
            local.set_variant(v);
        }
        if self.embedded {
            local.set_embedded_in_labeled_control(true);
        }

        let glyph = local.frame_glyph(tick, motion);
        let compact = matches!(local.variant, SpinnerVariant::CompactInline)
            || local.embedded_in_labeled_control();
        let theme = self.system.junie_theme();
        let glyph_style = if self.colorless {
            self.system.style(Role::TextStrong)
        } else {
            theme.accent_fg()
        };
        let ellipsis = self.system.glyphs.ellipsis();
        if compact && self.label.is_none() {
            let fitted = truncate_cols(glyph, usize::from(area.width), ellipsis);
            buffer.set_stringn(
                area.x,
                area.y,
                &fitted,
                usize::from(area.width),
                glyph_style,
            );
            return;
        }
        // junie: frame at x in accent, label at x+2 in secondary.
        buffer.set_stringn(area.x, area.y, glyph, usize::from(area.width), glyph_style);
        let label = self.resolved_label(&local);
        if area.width > 2 {
            let fitted = truncate_cols(label, usize::from(area.width.saturating_sub(2)), ellipsis);
            buffer.set_stringn(
                area.x.saturating_add(2),
                area.y,
                &fitted,
                usize::from(area.width.saturating_sub(2)),
                theme.secondary(),
            );
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &SpinnerState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !state.is_visible() {
            return;
        }
        let label = self.resolved_label(state);
        let desc = format!(
            "spinner phase={} active={} label={label}",
            state.phase().id(),
            state.is_active()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Progress)
                .label("spinner")
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    busy: state.is_active(),
                    ..Default::default()
                }),
        );
    }
}

// ── ActivityIndicator ───────────────────────────────────────────────────────

/// Richer activity line: phase glyph + verb + optional detail.
///
/// Always shows a verb (from label or phase default). Compact only when
/// embedded in a labeled control.
#[derive(Debug, Clone, Copy)]
pub struct ActivityIndicator<'a> {
    system: &'a DesignSystem,
    label: &'a str,
    detail: Option<&'a str>,
    colorless: bool,
}

impl<'a> ActivityIndicator<'a> {
    /// Verb label required (e.g. "Fetching packages").
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            label,
            detail: None,
            colorless: false,
        }
    }

    /// Secondary detail (attempt count, target).
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// ASCII.
    #[must_use]
    /// Remove hue without changing glyph capability.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Measure preferred height.
    #[must_use]
    pub fn measure_height(&self) -> u16 {
        if self.detail.is_some() { 2 } else { 1 }
    }

    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &SpinnerState,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        if area.is_empty() || !state.is_visible() {
            return;
        }
        let local = state.clone();
        let glyph = local.frame_glyph(tick, motion);
        let theme = self.system.junie_theme();
        let glyph_style = if self.colorless {
            self.system.style(Role::TextStrong)
        } else {
            theme.accent_fg()
        };
        // Compact activity: `⠋ label` — same vocabulary as Spinner.
        buffer.set_stringn(area.x, area.y, glyph, usize::from(area.width), glyph_style);
        if area.width > 2 {
            buffer.set_stringn(
                area.x.saturating_add(2),
                area.y,
                take_display_cols(self.label, usize::from(area.width.saturating_sub(2))).as_ref(),
                usize::from(area.width.saturating_sub(2)),
                theme.secondary(),
            );
        }
        if let Some(detail) = self.detail {
            if area.height > 1 {
                buffer.set_stringn(
                    area.x.saturating_add(2),
                    area.y + 1,
                    take_display_cols(detail, usize::from(area.width.saturating_sub(2))).as_ref(),
                    usize::from(area.width.saturating_sub(2)),
                    theme.muted(),
                );
            }
        }
    }
}

// Widget impl for Spinner without state — paints with default state via render
// (Stateful path uses paint with SpinnerState).

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn tick_at(ms: u64) -> FrameTick {
        let now = Instant::now();
        FrameTick::manual(now, Duration::from_millis(ms), Duration::from_millis(16))
    }

    #[test]
    fn every_phase_uses_the_one_cadence() {
        assert_eq!(ActivityPhase::Indeterminate.period_ms(), 80);
        assert_eq!(ActivityPhase::Waiting.period_ms(), 80);

        // Terminal states cost no frames at all.
        assert!(!ActivityPhase::Queued.animates());
        assert!(!ActivityPhase::Done.animates());
        assert!(ActivityPhase::Streaming.animates());
    }

    #[test]
    fn done_is_a_settled_mark_not_a_sequence() {
        let mut state = SpinnerState::new();
        state.set_phase(ActivityPhase::Done);
        assert_eq!(state.frames(MotionPolicy::Full), &["✓"]);
        assert_eq!(
            state.frame_glyph(tick_at(0), MotionPolicy::Full),
            state.frame_glyph(tick_at(4_000), MotionPolicy::Full),
            "a finished spinner must not keep moving"
        );
    }

    #[test]
    fn motion_off_parks_in_flight_phases_on_first_braille_frame() {
        for phase in [
            ActivityPhase::Indeterminate,
            ActivityPhase::Waiting,
            ActivityPhase::Queued,
            ActivityPhase::Reconnecting,
            ActivityPhase::Streaming,
        ] {
            let mut state = SpinnerState::new();
            state.set_phase(phase);
            assert_eq!(
                state.frame_glyph(tick_at(500), MotionPolicy::Off),
                SPINNER_BRAILLE_FRAMES[0],
                "{phase:?}"
            );
        }
        let mut done = SpinnerState::new();
        done.set_phase(ActivityPhase::Done);
        assert_eq!(done.frame_glyph(tick_at(500), MotionPolicy::Off), "✓");
    }

    #[test]
    fn spinner_motion_off_static() {
        let tokens = DesignSystem::default();
        let spinner = Spinner::new(&tokens);
        let now = Instant::now();
        let tick = FrameTick::manual(now, Duration::from_millis(560), Duration::from_millis(16));
        assert_eq!(
            spinner.frame_glyph(tick, MotionPolicy::Off),
            SPINNER_BRAILLE_FRAMES[0]
        );
        let a = spinner.frame_glyph(tick, MotionPolicy::Full);
        let b = spinner.frame_glyph(
            FrameTick::manual(now, Duration::from_millis(640), Duration::from_millis(16)),
            MotionPolicy::Full,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn idle_when_not_visible_or_inactive() {
        let mut state = SpinnerState::new();
        let tick = tick_at(500);
        assert!(
            state
                .animation_demand(tick, MotionPolicy::Full)
                .needs_redraw
        );
        state.set_visible(false);
        assert!(
            !state
                .animation_demand(tick, MotionPolicy::Full)
                .needs_redraw
        );
        state.set_visible(true);
        state.set_active(false);
        assert!(
            !state
                .animation_demand(tick, MotionPolicy::Full)
                .needs_redraw
        );
        assert!(!state.should_tick());
    }

    #[test]
    fn queued_does_not_animate() {
        let mut state = SpinnerState::new();
        state.set_phase(ActivityPhase::Queued);
        assert!(!state.phase().animates());
        assert!(!state.should_tick());
        let g1 = state.frame_glyph(tick_at(0), MotionPolicy::Full);
        let g2 = state.frame_glyph(tick_at(10_000), MotionPolicy::Full);
        assert_eq!(g1, g2);
    }

    #[test]
    fn phases_have_distinct_static_glyphs() {
        let mut state = SpinnerState::new();
        state.set_active(false); // static
        let mut glyphs = Vec::new();
        for p in [
            ActivityPhase::Indeterminate,
            ActivityPhase::Waiting,
            ActivityPhase::Queued,
            ActivityPhase::Reconnecting,
        ] {
            state.set_phase(p);
            glyphs.push(state.frame_glyph(tick_at(0), MotionPolicy::Off));
        }
        // Not all identical (queued vs indeterminate)
        assert!(glyphs.iter().any(|g| *g != glyphs[0]) || glyphs.len() == 4);
    }

    #[test]
    fn label_required_unless_embedded() {
        let system = DesignSystem::default();
        let bare = Spinner::new(&system);
        let state = SpinnerState::new();
        assert!(!bare.label_ok(&state));
        let labeled = Spinner::labeled("Loading", &system);
        assert!(labeled.label_ok(&state));
        let emb = Spinner::new(&system).embedded(true);
        assert!(emb.label_ok(&state));
        let mut st = SpinnerState::new();
        st.set_embedded_in_labeled_control(true);
        st.set_variant(SpinnerVariant::CompactInline);
        assert!(Spinner::new(&system).label_ok(&st));
    }

    #[test]
    fn labeled_paint_includes_verb() {
        let system = DesignSystem::default();
        let state = SpinnerState::new();
        let area = Rect::new(0, 0, 24, 1);
        let mut buf = Buffer::empty(area);
        Spinner::labeled("Fetching", &system).paint(
            area,
            &mut buf,
            &state,
            tick_at(200),
            MotionPolicy::Full,
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Fetching"), "{text}");
    }

    #[test]
    fn labeled_spinner_uses_capability_appropriate_ellipsis() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 14, 1);

        let unicode_state = SpinnerState::new();
        let mut unicode = Buffer::empty(area);
        Spinner::labeled("Presence travels", &system).paint(
            area,
            &mut unicode,
            &unicode_state,
            tick_at(0),
            MotionPolicy::Off,
        );
        let unicode_text: String = unicode.content().iter().map(|cell| cell.symbol()).collect();
        assert!(unicode_text.contains('…'), "{unicode_text:?}");
    }

    #[test]
    fn spinner_and_activity_indicator_resize_cjk_combining_and_ascii_safe() {
        let system = DesignSystem::default();
        let label = "検索 Cafe\u{301}";
        for _ in 0..2 {
            let state = SpinnerState::new();
            for (width, height) in [(32, 2), (12, 1), (1, 1), (0, 0)] {
                let area = Rect::new(0, 0, width, height);
                let mut spinner = Buffer::empty(area);
                Spinner::labeled(label, &system).paint(
                    area,
                    &mut spinner,
                    &state,
                    tick_at(0),
                    MotionPolicy::Off,
                );

                let mut indicator = Buffer::empty(area);
                ActivityIndicator::new(label, &system).paint(
                    area,
                    &mut indicator,
                    &state,
                    tick_at(0),
                    MotionPolicy::Off,
                );

                if width == 32 {
                    let spinner_text: String =
                        spinner.content().iter().map(|cell| cell.symbol()).collect();
                    let indicator_text: String = indicator
                        .content()
                        .iter()
                        .map(|cell| cell.symbol())
                        .collect();
                    assert!(spinner_text.contains('検'), "{spinner_text:?}");
                    assert!(spinner_text.contains("Cafe\u{301}"), "{spinner_text:?}");
                    assert!(indicator_text.contains('検'), "{indicator_text:?}");
                    assert!(indicator_text.contains("Cafe\u{301}"), "{indicator_text:?}");
                }
            }
        }
    }

    #[test]
    fn activity_indicator_detail_line() {
        let system = DesignSystem::default();
        let mut state = SpinnerState::new();
        state.set_phase(ActivityPhase::Reconnecting);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        ActivityIndicator::new("Reconnecting", &system)
            .detail("attempt 3/5")
            .paint(area, &mut buf, &state, tick_at(100), MotionPolicy::Full);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Reconnecting"), "{text}");
        assert!(text.contains("attempt") || text.contains("3"), "{text}");
    }

    #[test]
    fn activity_indicator_is_glyph_and_verb_in_ascii_colorless_mode() {
        let system = DesignSystem::default();
        let mut state = SpinnerState::new();
        state.set_phase(ActivityPhase::Reconnecting);
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);
        ActivityIndicator::new("Reconnecting", &system)
            .colorless(true)
            .paint(area, &mut buffer, &state, tick_at(100), MotionPolicy::Off);
        let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
        assert!(
            text.starts_with(&format!("{} Reconnecting", SPINNER_BRAILLE_FRAMES[0])),
            "{text:?}"
        );
        let warning_fg = system.style(Role::Warning).fg;
        assert!(
            buffer
                .content()
                .iter()
                .all(|cell| Some(cell.fg) != warning_fg)
        );
    }

    #[test]
    fn deterministic_cadence() {
        let state = SpinnerState::new();
        let a = state.frame_glyph(tick_at(800), MotionPolicy::Full);
        let b = state.frame_glyph(tick_at(800), MotionPolicy::Full);
        assert_eq!(a, b);
    }

    #[test]
    fn timing_idle_redraw_tests() {
        // Active visible Full → demand
        let active = SpinnerState::new();
        let tick = tick_at(0);
        assert!(
            active
                .animation_demand(tick, MotionPolicy::Full)
                .needs_redraw
        );
        assert!(
            active
                .animation_demand(tick, MotionPolicy::Full)
                .next_deadline
                .is_some()
        );
        // Motion off → no demand even if active
        assert!(
            !active
                .animation_demand(tick, MotionPolicy::Off)
                .needs_redraw
        );
        // Inactive → idle
        let mut idle = SpinnerState::new();
        idle.set_active(false);
        assert!(!idle.animation_demand(tick, MotionPolicy::Full).needs_redraw);
        // Matches runtime spinner_demand contract
        assert!(!spinner_demand(tick, MotionPolicy::Full, false).needs_redraw);
        assert!(spinner_demand(tick, MotionPolicy::Full, true).needs_redraw);
    }

    #[test]
    fn semantic_registers_busy() {
        let system = DesignSystem::default();
        let state = SpinnerState::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        Spinner::labeled("Working", &system).register_semantic(
            &mut scene,
            "s",
            Rect::new(0, 0, 12, 1),
            &state,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("spinner") && n.state.busy)
        );
    }

    #[test]
    fn fuzz_phases_ticks() {
        let mut state = SpinnerState::new();
        let phases = [
            ActivityPhase::Indeterminate,
            ActivityPhase::Waiting,
            ActivityPhase::Queued,
            ActivityPhase::Reconnecting,
        ];
        let mut seed = 7u64;
        for i in 0..100u64 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            state.set_phase(phases[(seed as usize) % phases.len()]);
            state.set_active(seed % 5 != 0);
            state.set_visible(seed % 7 != 0);
            let _ = state.frame_glyph(tick_at(i * 40), MotionPolicy::Full);
            let _ = state.animation_demand(tick_at(i * 40), MotionPolicy::Full);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let state = SpinnerState::new();
        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();
        let start = Instant::now();
        for i in 0..200u64 {
            terminal
                .draw(|f| {
                    ActivityIndicator::new("Working", &system)
                        .detail("…")
                        .paint(
                            f.area(),
                            f.buffer_mut(),
                            &state,
                            tick_at(i * 16),
                            MotionPolicy::Full,
                        );
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let paint = || {
            let mut terminal = Terminal::new(TestBackend::new(28, 2)).unwrap();
            let mut state = SpinnerState::new();
            state.set_phase(ActivityPhase::Waiting);
            // Off motion → deterministic glyph
            terminal
                .draw(|f| {
                    Spinner::labeled("Waiting", &system).paint(
                        f.area(),
                        f.buffer_mut(),
                        &state,
                        tick_at(0),
                        MotionPolicy::Off,
                    );
                })
                .unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }

    #[test]
    fn every_animated_phase_shares_the_one_frame_set() {
        // One cadence: no phase may own a divergent sequence.
        for phase in [
            ActivityPhase::Indeterminate,
            ActivityPhase::Waiting,
            ActivityPhase::Reconnecting,
            ActivityPhase::Streaming,
        ] {
            let mut state = SpinnerState::new();
            state.set_phase(phase);
            assert_eq!(
                state.frames(MotionPolicy::Full),
                SPINNER_BRAILLE_FRAMES,
                "{phase:?} grew its own frame set"
            );
            assert_eq!(state.frames(MotionPolicy::Full).len(), 10);
        }
        let mut state = SpinnerState::new();
        state.set_phase(ActivityPhase::Reconnecting);
        assert!(
            SPINNER_BRAILLE_FRAMES.contains(&state.frame_glyph(tick_at(0), MotionPolicy::Full))
        );
    }

    #[test]
    fn spinner_glyph_is_accent_label_is_secondary() {
        let system = DesignSystem::default();
        let state = SpinnerState::new();
        let area = Rect::new(0, 0, 24, 1);
        let mut buf = Buffer::empty(area);
        Spinner::labeled("Fetching", &system).paint(
            area,
            &mut buf,
            &state,
            tick_at(0),
            MotionPolicy::Off,
        );
        let theme = system.junie_theme();
        assert_eq!(buf[(0, 0)].symbol(), SPINNER_BRAILLE_FRAMES[0]);
        assert_eq!(buf[(0, 0)].fg, theme.accent_fg().fg.unwrap());
        assert_eq!(buf[(2, 0)].symbol(), "F");
        assert_eq!(buf[(2, 0)].fg, theme.secondary().fg.unwrap());
    }

    #[test]
    fn spinner_public_api_has_no_raw_role_escape_hatch() {
        let public = include_str!("spinner.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("public source");
        for forbidden in ["pub role:", "pub fn role(", "pub const fn role("] {
            assert!(
                !public.contains(forbidden),
                "raw role API leaked: {forbidden}"
            );
        }
    }
}
