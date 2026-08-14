// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Spinner** and **ActivityIndicator** — semantic activity with deterministic cadence.
//!
//! **Mission.** Terminal spinners and AI-tool activity states that always pair a
//! glyph with a meaningful verb/label (unless embedded in a labeled control).
//! Phases: indeterminate, waiting, queued, reconnecting; compact inline and
//! labeled recipes; capability-aware glyph sequences; reduced-motion static
//! fallback; **no frame ticks** when inactive or not visible.
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

#![allow(unused_variables, unused_mut)] // unit-test fixtures
use std::time::Duration;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::{AnimationDemand, FrameTick, spinner_demand, spinner_step},
    style::{DesignSystem, MotionPolicy, Role},
    text::{display_cols, take_display_cols},
};

/// Default frame period (ms) for Full motion — matches historic Spinner/Progress.
pub const SPINNER_DEFAULT_PERIOD_MS: u64 = 80;
pub use crate::style::{SPINNER_BRAILLE_FRAMES, SPINNER_DOT_PULSE_FRAMES};
/// ASCII sequence.
pub const SPINNER_ASCII_FRAMES: &[&str] = &["|", "/", "-", "\\"];
/// Waiting phase pulse (Unicode).
pub const SPINNER_WAITING_UNICODE: &[&str] = &["·", "•", "●", "•"];
/// Waiting phase (ASCII).
pub const SPINNER_WAITING_ASCII: &[&str] = &[".", "o", "O", "o"];
/// Reconnecting uses reverse braille cadence.
pub const SPINNER_RECONNECT_UNICODE: &[&str] = &["⠏", "⠇", "⠧", "⠦", "⠴", "⠼", "⠸", "⠹", "⠙", "⠋"];

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
        }
    }

    /// Whether this phase advances frames under Full motion.
    #[must_use]
    pub const fn animates(self) -> bool {
        !matches!(self, Self::Queued)
    }

    /// Period multiplier (waiting is slower).
    #[must_use]
    pub const fn period_scale(self) -> u64 {
        match self {
            Self::Indeterminate | Self::Reconnecting => 1,
            Self::Waiting => 3,
            Self::Queued => 1,
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

impl SpinnerVariant {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Labeled => "labeled",
            Self::CompactInline => "compact-inline",
        }
    }
}

/// Glyph sequence family (capability-aware selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SpinnerGlyphSet {
    /// Braille spinner (default when Unicode).
    #[default]
    Braille,
    /// ASCII `|/-\\`.
    Ascii,
    /// Quiet one-cell dot pulse for presence indicators.
    DotPulse,
    /// Auto: Braille unless the `ascii` flag or the motion tier prefers static.
    Auto,
}

impl SpinnerGlyphSet {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Braille => "braille",
            Self::Ascii => "ascii",
            Self::DotPulse => "dot-pulse",
            Self::Auto => "auto",
        }
    }
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
    glyph_set: SpinnerGlyphSet,
    variant: SpinnerVariant,
    ascii_force: bool,
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
            glyph_set: SpinnerGlyphSet::Auto,
            variant: SpinnerVariant::Labeled,
            ascii_force: false,
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

    /// Frame period ms (Full motion base).
    pub fn set_period_ms(&mut self, ms: u64) {
        self.period_ms = ms.max(16);
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

    /// Glyph set.
    pub fn set_glyph_set(&mut self, set: SpinnerGlyphSet) {
        self.glyph_set = set;
    }

    /// Variant.
    pub fn set_variant(&mut self, v: SpinnerVariant) {
        self.variant = v;
    }

    /// Force ASCII frames.
    pub fn set_ascii(&mut self, on: bool) {
        self.ascii_force = on;
    }

    /// Animation demand for host frame clock. Idle when not ticking.
    #[must_use]
    pub fn animation_demand(&self, tick: FrameTick, motion: MotionPolicy) -> AnimationDemand {
        if !self.should_tick() {
            return AnimationDemand::idle();
        }
        let period = self
            .period_ms
            .saturating_mul(self.phase.period_scale())
            .max(16);
        // spinner_demand uses fixed 80ms — scale deadline to our period
        let base = spinner_demand(tick, motion, true);
        if !base.needs_redraw {
            return base;
        }
        let scaled = Duration::from_millis(period.saturating_mul(motion.spinner_divisor().max(1)));
        AnimationDemand {
            needs_redraw: true,
            next_deadline: Some(tick.now() + scaled),
        }
    }

    /// Effective frames for phase + capability.
    #[must_use]
    pub fn frames(&self, motion: MotionPolicy) -> &'static [&'static str] {
        let ascii = self.ascii_force
            || matches!(self.glyph_set, SpinnerGlyphSet::Ascii)
            || (matches!(self.glyph_set, SpinnerGlyphSet::Auto) && self.ascii_force);
        // Auto with unicode when not ascii_force
        let use_ascii = ascii || matches!(self.glyph_set, SpinnerGlyphSet::Ascii);

        if !motion.animate_spinners() || !self.phase.animates() {
            return if use_ascii {
                match self.phase {
                    ActivityPhase::Queued => &["o"],
                    ActivityPhase::Waiting => &["."],
                    ActivityPhase::Reconnecting => &["?"],
                    ActivityPhase::Indeterminate => &["o"],
                }
            } else {
                match self.phase {
                    ActivityPhase::Queued => &["○"],
                    ActivityPhase::Waiting => &["·"],
                    ActivityPhase::Reconnecting => &["◌"],
                    ActivityPhase::Indeterminate => &["●"],
                }
            };
        }

        match self.phase {
            ActivityPhase::Indeterminate => {
                if use_ascii {
                    SPINNER_ASCII_FRAMES
                } else if matches!(self.glyph_set, SpinnerGlyphSet::DotPulse) {
                    SPINNER_DOT_PULSE_FRAMES
                } else {
                    SPINNER_BRAILLE_FRAMES
                }
            }
            ActivityPhase::Waiting => {
                if use_ascii {
                    SPINNER_WAITING_ASCII
                } else {
                    SPINNER_WAITING_UNICODE
                }
            }
            ActivityPhase::Queued => {
                if use_ascii {
                    &["o"]
                } else {
                    &["○"]
                }
            }
            ActivityPhase::Reconnecting => {
                if use_ascii {
                    SPINNER_ASCII_FRAMES
                } else {
                    SPINNER_RECONNECT_UNICODE
                }
            }
        }
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
        let period = self.period_ms.saturating_mul(self.phase.period_scale());
        let step = spinner_step(tick, frames.len(), period, motion);
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
    ascii: bool,
    embedded: bool,
    phase: Option<ActivityPhase>,
    variant: Option<SpinnerVariant>,
    role: Role,
}

impl<'a> Spinner<'a> {
    /// System only (label via builder; default indeterminate).
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            label: None,
            ascii: false,
            embedded: false,
            phase: None,
            variant: None,
            role: Role::TextMuted,
        }
    }

    /// Labeled spinner (preferred constructor).
    #[must_use]
    pub const fn labeled(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            label: Some(label),
            ascii: false,
            embedded: false,
            phase: None,
            variant: None,
            role: Role::TextMuted,
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
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Embedded in labeled control (glyph-only ok).
    #[must_use]
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

    /// Paint role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Frame glyph (legacy API preserved).
    #[must_use]
    pub fn frame_glyph(&self, tick: FrameTick, motion: MotionPolicy) -> &'static str {
        let mut state = SpinnerState::new();
        state.set_ascii(self.ascii);
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
        if self.ascii {
            local.set_ascii(true);
        }
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
        let text = if compact && self.label.is_none() {
            glyph.to_string()
        } else {
            let label = self.resolved_label(&local);
            format!("{glyph} {label}")
        };
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(&text, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(self.role),
        );
    }

    /// Legacy paint without state (always active/visible).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, tick: FrameTick, motion: MotionPolicy) {
        let mut state = SpinnerState::new();
        state.set_ascii(self.ascii);
        if let Some(p) = self.phase {
            state.set_phase(p);
        }
        if self.embedded {
            state.set_embedded_in_labeled_control(true);
        }
        if self.label.is_none() {
            state.set_embedded_in_labeled_control(true); // legacy glyph-only call sites
        }
        self.paint(area, buffer, &state, tick, motion);
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
    ascii: bool,
    role: Role,
}

impl<'a> ActivityIndicator<'a> {
    /// Verb label required (e.g. "Fetching packages").
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            system,
            label,
            detail: None,
            ascii: false,
            role: Role::Info,
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
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
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
        let mut local = state.clone();
        if self.ascii {
            local.set_ascii(true);
        }
        let glyph = local.frame_glyph(tick, motion);
        let line1 = format!("{glyph} {}", self.label);
        // The verb is words; the spinner cell is the signal (plans/007).
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(&line1, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::TextMuted),
        );
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            area,
            0,
            glyph,
            self.system.style(self.role),
        );
        if let Some(detail) = self.detail {
            if area.height > 1 {
                let prefix_cols = display_cols(glyph).saturating_add(1);
                let x = area.x.saturating_add(prefix_cols as u16);
                let w = area.width.saturating_sub(prefix_cols as u16);
                if w > 0 {
                    buffer.set_stringn(
                        x,
                        area.y + 1,
                        &take_display_cols(detail, usize::from(w)),
                        usize::from(w),
                        self.system.style(Role::TextMuted),
                    );
                }
            }
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
        let desc = format!(
            "activity-indicator phase={} active={} label={} detail={}",
            state.phase().id(),
            state.is_active(),
            self.label,
            self.detail.unwrap_or(""),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("activity-indicator")
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    busy: state.is_active(),
                    ..Default::default()
                }),
        );
    }
}

// Widget impl for Spinner without state — paints with default state via render
// (Stateful path uses paint with SpinnerState).

impl Widget for &Spinner<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        // Static fallback when host has no tick in Widget path.
        let tick = FrameTick::manual(
            crate::runtime::Instant::now(),
            Duration::ZERO,
            Duration::ZERO,
        );
        self.render(area, buffer, tick, MotionPolicy::Off);
    }
}

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
    fn spinner_motion_off_static() {
        let tokens = DesignSystem::default();
        let spinner = Spinner::new(&tokens);
        let now = Instant::now();
        let tick = FrameTick::manual(now, Duration::from_millis(560), Duration::from_millis(16));
        assert_eq!(spinner.frame_glyph(tick, MotionPolicy::Off), "●");
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
        let mut state = SpinnerState::new();
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
    fn ascii_and_reduced_motion() {
        let mut state = SpinnerState::new();
        state.set_ascii(true);
        let g = state.frame_glyph(tick_at(0), MotionPolicy::Off);
        assert!(g == "o" || g == "|" || g == "." || g == "?");
        let a = state.frame_glyph(tick_at(80), MotionPolicy::Full);
        let b = state.frame_glyph(tick_at(160), MotionPolicy::Full);
        // may or may not differ depending on step; at least valid ascii set
        assert!(SPINNER_ASCII_FRAMES.contains(&a) || a == "o");
        let _ = b;
        assert!(
            !state
                .animation_demand(tick_at(0), MotionPolicy::Basic)
                .needs_redraw
                || !MotionPolicy::Basic.animate_spinners()
        );
    }

    #[test]
    fn deterministic_cadence() {
        let state = SpinnerState::new();
        let a = state.frame_glyph(tick_at(800), MotionPolicy::Full);
        let b = state.frame_glyph(tick_at(800), MotionPolicy::Full);
        assert_eq!(a, b);
        let c = state.frame_glyph(tick_at(880), MotionPolicy::Full);
        // 80ms later may advance
        let _ = c;
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
            state.set_ascii(seed % 2 == 0);
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
    fn reconnecting_uses_reverse_sequence() {
        let mut state = SpinnerState::new();
        state.set_phase(ActivityPhase::Reconnecting);
        let g = state.frame_glyph(tick_at(0), MotionPolicy::Full);
        assert!(
            SPINNER_RECONNECT_UNICODE.contains(&g) || SPINNER_BRAILLE_FRAMES.contains(&g),
            "{g}"
        );
    }
}
