// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ProgressBar** — the junie progress bar and its task/transfer model.
//!
//! Paint is a one-to-one port of the reference `src/widgets/progress.rs`
//! (reference-spec §4.14):
//!
//! - determinate bar: `label ━━━━━───── 64% ` — fill `━`, track `─` in
//!   `border_subtle`, percentage right-aligned in `text_secondary`, and a
//!   fixed 2-cell lifecycle column (` ✓` done, ` !` error, ` ‖` paused).
//! - **green is reserved for completion; a running bar is white 70 %**
//!   (`text_secondary`), a paused one drops another step (`text_muted`).
//! - indeterminate: a short accent `━` segment sweeping a `─` track.
//! - the activity glyph is the one 10-frame braille vocabulary at 80 ms; there
//!   is no block ramp, no ASCII twin, no partial cell, and no second cadence.
//!
//! The host-facing model (`ProgressBarState`) keeps units/rate/ETA so builds,
//! downloads, and transfers can project onto the same bar.
//!
//! **vs Spinner.** Spinner is glyph + verb activity without completion.
//! **vs TokenMeter.** Token usage domain meter; this is generic progress.
use std::time::Duration;
use web_time::Instant;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::{AnimationDemand, FrameTick, spinner_demand},
    style::{DesignSystem, MotionPolicy, Role},
    text::{display_cols, take_display_cols},
};

use super::SemanticStatus;

/// The one activity frame vocabulary (D6) — re-exported under the progress
/// name so an indeterminate bar and a spinner can never drift apart.
pub use crate::style::SPINNER_BRAILLE_FRAMES as DEFAULT_PROGRESS_FRAMES;
/// Fixed trailing lifecycle glyph column: `" ✓"` done, `" !"` error,
/// `" ‖"` paused, `"  "` running (reference `progress.rs:48-53`).
const SUFFIX_WIDTH: u16 = 2;
/// Width of the right-aligned percentage: `" {pct}"` where `pct` is `{:>4}`.
const PERCENTAGE_WIDTH: u16 = 5;
/// A track narrower than this carries no bar: percentage only (reference).
const MIN_TRACK_WIDTH: u16 = 6;
/// Default throttle for state-driven updates (ms).
pub const PROGRESS_DEFAULT_THROTTLE_MS: u64 = 50;

// ── Kind / status / recipe / units ──────────────────────────────────────────

/// Determinate and caller-ticked indeterminate progress modes.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ProgressKind {
    /// A progress bar with a known completion fraction.
    Determinate {
        /// Completed fraction; rendering clamps finite values to `0.0..=1.0`.
        fraction: f64,
    },
    /// A caller-ticked progress indicator with no known completion fraction.
    Indeterminate {
        /// Caller-owned deterministic animation tick.
        tick: u64,
    },
}

impl ProgressKind {
    /// Indeterminate frame index from [`FrameTick`] + [`MotionPolicy`].
    ///
    /// `Off` parks on the first frame: the frozen frame is the deterministic
    /// answer of a reduced-motion terminal (D7).
    #[must_use]
    pub fn indeterminate_from(tick: FrameTick, motion: MotionPolicy) -> Self {
        // Sweep position is elapsed/80 ms, not the 10-frame spinner index —
        // wrapping to 10 frames parks the segment in one of ten slots and
        // can make Full and Off paint identically.
        let step = match motion {
            MotionPolicy::Off => 0,
            MotionPolicy::Full => tick.elapsed_ms() / 80,
        };
        Self::Indeterminate { tick: step }
    }

    /// Fraction if determinate.
    #[must_use]
    pub fn fraction(self) -> Option<f64> {
        match self {
            Self::Determinate { fraction } => Some(clamp_fraction(fraction)),
            Self::Indeterminate { .. } => None,
        }
    }
}

/// Lifecycle / outcome of a progress operation.
///
/// The reference knows four states (Active, Done, Error, Paused); TermRock
/// splits two of them for host bookkeeping and maps both back onto the
/// reference paint: `Buffering` is still *active*, `Cancelled` is *halted*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressStatus {
    /// Actively advancing.
    #[default]
    Running,
    /// Host paused (ETA frozen).
    Paused,
    /// Buffering / stalled wait without cancel — paints as running.
    Buffering,
    /// User or host cancelled — paints as halted.
    Cancelled,
    /// Finished successfully (fraction → 1).
    Complete,
    /// Finished with failure.
    Failed,
}

impl ProgressStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Buffering => "buffering",
            Self::Cancelled => "cancelled",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    /// Whether the sweep should advance (the reference `Active` state).
    #[must_use]
    pub const fn animates(self) -> bool {
        matches!(self, Self::Running | Self::Buffering)
    }

    /// Fixed 2-cell lifecycle column (reference `progress.rs:48-53`).
    ///
    /// ` ✓` done, ` !` error, ` ‖` paused, `  ` running. `Cancelled` derives
    /// `×` — the close glyph — because a cancelled bar is one that stopped.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Complete => " \u{2713}",
            Self::Failed => " !",
            Self::Paused => " \u{2016}",
            Self::Cancelled => " ×",
            Self::Running | Self::Buffering => "  ",
        }
    }

    /// Fill tone (reference: green is reserved for completion).
    #[must_use]
    pub const fn fill_role(self) -> Role {
        match self {
            Self::Complete => Role::Success,
            Self::Failed => Role::Danger,
            Self::Running | Self::Buffering => Role::TextSecondary,
            Self::Paused | Self::Cancelled => Role::TextMuted,
        }
    }

    /// Shared lifecycle projection used by status recipes.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Running => SemanticStatus::Running,
            Self::Paused | Self::Cancelled => SemanticStatus::Paused,
            Self::Buffering => SemanticStatus::Waiting,
            Self::Complete => SemanticStatus::Success,
            Self::Failed => SemanticStatus::Failed,
        }
    }
}

/// Visual recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressRecipe {
    /// Single row: label + track + optional %.
    #[default]
    Compact,
    /// Single row with units/rate/ETA when width allows.
    Detailed,
    /// Multi-line: title, track, meta (phase/rate/ETA/status).
    MultiLine,
}

impl ProgressRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Detailed => "detailed",
            Self::MultiLine => "multi-line",
        }
    }

    /// Preferred height.
    #[must_use]
    pub const fn preferred_height(self) -> u16 {
        match self {
            Self::Compact | Self::Detailed => 1,
            Self::MultiLine => 3,
        }
    }
}

/// Unit system for transfer / task models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressUnit {
    /// Unitless fraction only.
    #[default]
    None,
    /// Bytes.
    Bytes,
    /// Items / counts.
    Items,
    /// Custom unit label from host (`ProgressBarState::unit_label`).
    Custom,
}

impl ProgressUnit {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bytes => "bytes",
            Self::Items => "items",
            Self::Custom => "custom",
        }
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Host-driven progress model (task / transfer projection).
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressBarState {
    value: f64,
    total: f64,
    unit: ProgressUnit,
    unit_label: String,
    status: ProgressStatus,
    /// Phase name (e.g. "compile", "upload").
    phase: String,
    /// Label / task title.
    label: String,
    /// Instantaneous rate in units/sec (host-supplied).
    rate: Option<f64>,
    /// ETA seconds remaining (host-supplied or derived).
    eta_secs: Option<u64>,
    recipe: ProgressRecipe,
    /// Throttle: min interval between accepted value updates.
    throttle: Duration,
    last_update: Option<Instant>,
    /// Generation bumps when paint-relevant data changes after throttle.
    generation: u64,
    last_painted_generation: u64,
    /// Active for indeterminate redraw demand.
    active: bool,
    visible: bool,
}

impl Default for ProgressBarState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressBarState {
    /// Zero of zero, running, compact.
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: 0.0,
            total: 0.0,
            unit: ProgressUnit::None,
            unit_label: String::new(),
            status: ProgressStatus::Running,
            phase: String::new(),
            label: String::new(),
            rate: None,
            eta_secs: None,
            recipe: ProgressRecipe::Compact,
            throttle: Duration::from_millis(PROGRESS_DEFAULT_THROTTLE_MS),
            last_update: None,
            generation: 0,
            last_painted_generation: 0,
            active: true,
            visible: true,
        }
    }

    /// Transfer helper: bytes value/total.
    #[must_use]
    pub fn transfer(value: u64, total: u64) -> Self {
        let mut s = Self::new();
        s.unit = ProgressUnit::Bytes;
        s.value = value as f64;
        s.total = total as f64;
        s.recipe = ProgressRecipe::Detailed;
        s
    }

    /// Task helper: completed of total items.
    #[must_use]
    pub fn task(done: u64, total: u64) -> Self {
        let mut s = Self::new();
        s.unit = ProgressUnit::Items;
        s.value = done as f64;
        s.total = total as f64;
        s
    }

    /// Fraction 0..=1.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        if self.total > 0.0 && self.total.is_finite() && self.value.is_finite() {
            clamp_fraction(self.value / self.total)
        } else if matches!(self.status, ProgressStatus::Complete) {
            1.0
        } else {
            0.0
        }
    }

    /// Whether determinate (known total > 0).
    #[must_use]
    pub fn is_determinate(&self) -> bool {
        self.total > 0.0 && self.total.is_finite()
    }

    fn is_determinate_raw(total: f64) -> bool {
        total > 0.0 && total.is_finite()
    }

    /// Kind projection for the paint path.
    ///
    /// The bar paints the reported fraction directly: a progress bar has no
    /// easing, no spring, no trailing value (D7 — motion is {Full, Off} and
    /// neither of them interpolates a number).
    pub fn kind(&mut self, tick: FrameTick, motion: MotionPolicy) -> ProgressKind {
        if self.is_determinate() {
            ProgressKind::Determinate {
                fraction: self.fraction(),
            }
        } else {
            ProgressKind::indeterminate_from(tick, motion)
        }
    }

    /// Status.
    #[must_use]
    pub const fn status(&self) -> ProgressStatus {
        self.status
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(&self) -> ProgressRecipe {
        self.recipe
    }

    /// Generation (for host dirty checks).
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Whether paint should run (dirty vs last paint).
    #[must_use]
    pub const fn needs_paint(&self) -> bool {
        self.generation != self.last_painted_generation
    }

    /// Mark painted (host or widget after paint).
    pub fn mark_painted(&mut self) {
        self.last_painted_generation = self.generation;
    }

    /// Active for animation demand.
    ///
    /// Only an indeterminate bar asks for frames: a determinate fraction is a
    /// number, and numbers do not tick.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
            && self.visible
            && self.status.animates()
            && !Self::is_determinate_raw(self.total)
    }

    /// Animation demand (indeterminate only).
    #[must_use]
    pub fn animation_demand(&self, tick: FrameTick, motion: MotionPolicy) -> AnimationDemand {
        spinner_demand(tick, motion, self.is_active())
    }

    fn bump(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    /// Throttled value update. Returns false if dropped by throttle.
    pub fn set_value_throttled(&mut self, value: f64, now: Instant) -> bool {
        if let Some(last) = self.last_update {
            if now.duration_since(last) < self.throttle
                && !matches!(
                    self.status,
                    ProgressStatus::Complete | ProgressStatus::Failed | ProgressStatus::Cancelled
                )
            {
                // Always allow terminal values through
                let at_end = self.total > 0.0 && value >= self.total;
                if !at_end {
                    return false;
                }
            }
        }
        self.last_update = Some(now);
        self.value = value.max(0.0);
        self.bump();
        true
    }

    /// Unthrottled set value.
    pub fn set_value(&mut self, value: f64) {
        self.value = value.max(0.0);
        self.bump();
    }

    /// Set total (0 = indeterminate).
    pub fn set_total(&mut self, total: f64) {
        self.total = total.max(0.0);
        self.bump();
    }

    /// Set fraction directly (sets total=1, value=fraction).
    pub fn set_fraction(&mut self, fraction: f64) {
        self.total = 1.0;
        self.value = clamp_fraction(fraction);
        self.bump();
    }

    /// Unit.
    pub fn set_unit(&mut self, unit: ProgressUnit) {
        self.unit = unit;
        self.bump();
    }

    /// Custom unit label.
    pub fn set_unit_label(&mut self, label: impl Into<String>) {
        self.unit_label = label.into();
        self.unit = ProgressUnit::Custom;
        self.bump();
    }

    /// Status.
    pub fn set_status(&mut self, status: ProgressStatus) {
        self.status = status;
        if matches!(status, ProgressStatus::Complete) && self.total > 0.0 {
            self.value = self.total;
        }
        self.bump();
    }

    /// Phase.
    pub fn set_phase(&mut self, phase: impl Into<String>) {
        self.phase = phase.into();
        self.bump();
    }

    /// Label.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
        self.bump();
    }

    /// Rate units/sec.
    pub fn set_rate(&mut self, rate: Option<f64>) {
        self.rate = rate.filter(|r| r.is_finite() && *r >= 0.0);
        self.bump();
    }

    /// ETA seconds.
    pub fn set_eta_secs(&mut self, eta: Option<u64>) {
        self.eta_secs = eta;
        self.bump();
    }

    /// Derive ETA from remaining / rate when possible.
    pub fn recompute_eta(&mut self) {
        if let (Some(rate), true) = (self.rate, self.is_determinate()) {
            if rate > 0.0 {
                let rem = (self.total - self.value).max(0.0);
                self.eta_secs = Some((rem / rate).ceil() as u64);
                self.bump();
            }
        }
    }

    /// Throttle interval.
    pub fn set_throttle(&mut self, d: Duration) {
        self.throttle = d;
    }

    /// Active / visible for animation.
    pub fn set_active(&mut self, on: bool) {
        self.active = on;
    }

    /// Visible.
    pub fn set_visible(&mut self, on: bool) {
        self.visible = on;
    }

    /// Visual recipe (compact / detailed / multi-line).
    pub fn set_recipe(&mut self, recipe: ProgressRecipe) {
        self.recipe = recipe;
        self.bump();
    }

    /// Current completed amount.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Total amount (`0` means indeterminate).
    #[must_use]
    pub const fn total(&self) -> f64 {
        self.total
    }

    /// Task / bar label.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Phase name (e.g. compile step).
    #[must_use]
    pub fn phase(&self) -> &str {
        &self.phase
    }

    /// Format percentage string — the reference `{:>4}` column.
    #[must_use]
    pub fn percentage_text(&self) -> String {
        percentage_text(self.fraction())
    }

    /// Format units current/total.
    #[must_use]
    pub fn units_text(&self) -> Option<String> {
        if !self.is_determinate() {
            return None;
        }
        match self.unit {
            ProgressUnit::None => None,
            ProgressUnit::Bytes => Some(format!(
                "{}/{}",
                format_bytes(self.value as u64),
                format_bytes(self.total as u64)
            )),
            ProgressUnit::Items => Some(format!("{}/{}", self.value as u64, self.total as u64)),
            ProgressUnit::Custom => {
                let u = if self.unit_label.is_empty() {
                    ""
                } else {
                    self.unit_label.as_str()
                };
                Some(format!("{:.0}/{:.0}{u}", self.value, self.total))
            }
        }
    }

    /// Rate text.
    #[must_use]
    pub fn rate_text(&self) -> Option<String> {
        let rate = self.rate?;
        match self.unit {
            ProgressUnit::Bytes => Some(format!("{}/s", format_bytes(rate as u64))),
            ProgressUnit::Items => Some(format!("{rate:.0}/s")),
            ProgressUnit::Custom => {
                let u = self.unit_label.as_str();
                Some(format!("{rate:.1}{u}/s"))
            }
            ProgressUnit::None => Some(format!("{rate:.2}/s")),
        }
    }

    /// ETA text.
    #[must_use]
    pub fn eta_text(&self) -> Option<String> {
        let s = self.eta_secs?;
        Some(format_eta(s))
    }

    /// Meta line for detailed/multi-line recipes.
    #[must_use]
    pub fn meta_line(&self) -> String {
        let mut parts = Vec::new();
        if !self.phase.is_empty() {
            parts.push(self.phase.clone());
        }
        if let Some(u) = self.units_text() {
            parts.push(u);
        }
        if let Some(r) = self.rate_text() {
            parts.push(r);
        }
        if let Some(e) = self.eta_text() {
            parts.push(format!("ETA {e}"));
        }
        if !matches!(self.status, ProgressStatus::Running) {
            parts.push(self.status.id().into());
        }
        parts.join(" · ")
    }
}

fn clamp_fraction(fraction: f64) -> f64 {
    if fraction.is_finite() {
        fraction.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Reference percentage column: `pct = format!("{:>4}", "{n}%")`.
fn percentage_text(fraction: f64) -> String {
    format!("{:>4}", format!("{}%", (fraction * 100.0).round() as u32))
}

fn format_bytes(n: u64) -> String {
    const K: f64 = 1024.0;
    let n = n as f64;
    if n < K {
        format!("{}B", n as u64)
    } else if n < K * K {
        format!("{:.1}K", n / K)
    } else if n < K * K * K {
        format!("{:.1}M", n / (K * K))
    } else {
        format!("{:.1}G", n / (K * K * K))
    }
}

fn format_eta(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Progress bar with optional label (junie `render_bar` + rich state paint).
#[derive(Debug, Clone, Copy)]
pub struct ProgressBar<'a> {
    kind: ProgressKind,
    label: Option<&'a str>,
    system: &'a DesignSystem,
    recipe: ProgressRecipe,
    status: ProgressStatus,
    phase: Option<&'a str>,
    meta: Option<&'a str>,
}

impl<'a> ProgressBar<'a> {
    /// Creates an unlabeled progress indicator in the supplied mode.
    #[must_use]
    pub const fn new(kind: ProgressKind, system: &'a DesignSystem) -> Self {
        Self {
            kind,
            label: None,
            system,
            recipe: ProgressRecipe::Compact,
            status: ProgressStatus::Running,
            phase: None,
            meta: None,
        }
    }

    /// From state + tick (preferred for task/transfer).
    ///
    /// Strings owned by the state cannot be borrowed here; use
    /// [`Self::paint_state`] when the bar must carry its label, phase, and
    /// meta line.
    #[must_use]
    pub fn from_state(
        state: &mut ProgressBarState,
        system: &'a DesignSystem,
        tick: FrameTick,
        motion: MotionPolicy,
    ) -> Self {
        Self {
            kind: state.kind(tick, motion),
            label: None,
            system,
            recipe: state.recipe,
            status: state.status,
            phase: None,
            meta: None,
        }
    }

    /// Optional visible label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: ProgressRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Status (affects fill tone and the lifecycle column).
    #[must_use]
    pub const fn status(mut self, status: ProgressStatus) -> Self {
        self.status = status;
        self
    }

    /// Phase label (detailed meta).
    #[must_use]
    pub const fn phase(mut self, phase: &'a str) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Preformatted meta line.
    #[must_use]
    pub const fn meta(mut self, meta: &'a str) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Paint from full state (handles owned strings).
    pub fn paint_state(
        system: &DesignSystem,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ProgressBarState,
        tick: FrameTick,
        motion: MotionPolicy,
    ) {
        if area.is_empty() || !state.visible {
            return;
        }
        let kind = state.kind(tick, motion);
        let label = if state.label.is_empty() {
            None
        } else {
            Some(state.label.as_str())
        };
        let meta = state.meta_line();
        let meta_ref = if meta.is_empty() {
            None
        } else {
            Some(meta.as_str())
        };
        let phase = if state.phase.is_empty() {
            None
        } else {
            Some(state.phase.as_str())
        };
        let bar = ProgressBar {
            kind,
            label,
            system,
            recipe: state.recipe,
            status: state.status,
            phase,
            meta: meta_ref,
        };
        bar.paint(area, buffer);
        state.mark_painted();
    }

    /// Paint using builders on this widget.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        match self.recipe {
            ProgressRecipe::MultiLine => self.paint_multiline(area, buffer),
            ProgressRecipe::Compact | ProgressRecipe::Detailed => {
                self.paint_row(
                    area,
                    buffer,
                    matches!(self.recipe, ProgressRecipe::Detailed),
                );
            }
        }
    }

    fn paint_multiline(&self, area: Rect, buffer: &mut Buffer) {
        let h = area.height.min(3);
        if h == 0 {
            return;
        }
        // Title
        let title = self.label.unwrap_or("");
        if !title.is_empty() {
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(title, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextStrong),
            );
        }
        // Track row
        if h >= 2 {
            let track = Rect::new(area.x, area.y + 1, area.width, 1);
            self.paint_kind(track, buffer, None, false);
        }
        // Meta
        if h >= 3 {
            let meta = self.meta.or(self.phase).unwrap_or(self.status.id());
            buffer.set_stringn(
                area.x,
                area.y + 2,
                &take_display_cols(meta, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_row(&self, area: Rect, buffer: &mut Buffer, detailed: bool) {
        self.paint_kind(area, buffer, self.label, detailed);
    }

    fn paint_kind(&self, area: Rect, buffer: &mut Buffer, label: Option<&str>, detailed: bool) {
        match self.kind {
            ProgressKind::Determinate { fraction } => render_determinate(
                area,
                buffer,
                label,
                fraction,
                self.system,
                self.status,
                detailed,
                self.meta,
            ),
            ProgressKind::Indeterminate { tick } => {
                render_indeterminate(area, buffer, label, tick, self.system);
            }
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() {
            return;
        }
        let frac = self.kind.fraction().unwrap_or(-1.0);
        let desc = format!(
            "progress status={} recipe={} fraction={frac:.2} label={}",
            self.status.id(),
            self.recipe.id(),
            self.label.unwrap_or(""),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Progress)
                .label("progress-bar")
                .description(desc)
                .focusable(false)
                .state(SemanticState {
                    busy: matches!(
                        self.status,
                        ProgressStatus::Running | ProgressStatus::Buffering
                    ),
                    ..Default::default()
                }),
        );
    }
}

impl Widget for &ProgressBar<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for ProgressBar<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Render helpers (reference `render_bar` / `render_indeterminate`) ────────

/// Determinate bar: `label ━━━━━───── 64% `.
///
/// Reference geometry: percentage column `{:>4}` plus one leading cell, a
/// fixed 2-cell lifecycle column, and a track that takes what is left. A
/// track narrower than [`MIN_TRACK_WIDTH`] carries percentage only, and a
/// label is painted only when the row is wide enough to keep the bar useful.
fn render_determinate(
    area: Rect,
    buffer: &mut Buffer,
    label: Option<&str>,
    fraction: f64,
    system: &DesignSystem,
    status: ProgressStatus,
    detailed: bool,
    meta: Option<&str>,
) {
    let fraction = clamp_fraction(fraction);
    let pct = percentage_text(fraction);
    let mut x = area.x;
    if let Some(label) = label {
        let label_w = u16::try_from(display_cols(label)).unwrap_or(u16::MAX);
        if label_w > 0 && area.width > label_w.saturating_add(8) {
            buffer.set_stringn(
                x,
                area.y,
                label,
                usize::from(label_w),
                system.style(Role::Text),
            );
            x = x.saturating_add(label_w + 2);
        }
    }

    // Detailed recipes spend the right-hand meta before the percentage column.
    let mut right = area.right();
    if detailed
        && let Some(m) = meta
        && !m.is_empty()
    {
        let room = right.saturating_sub(x).saturating_sub(PERCENTAGE_WIDTH + 2);
        let mw = u16::try_from(display_cols(m))
            .unwrap_or(u16::MAX)
            .min(area.width / 3)
            .min(room);
        if mw > 0 {
            let mx = right.saturating_sub(mw);
            buffer.set_stringn(
                mx,
                area.y,
                &take_display_cols(m, usize::from(mw)),
                usize::from(mw),
                system.style(Role::TextMuted),
            );
            right = mx;
        }
    }

    let suffix = status.suffix();
    let track_w = right
        .saturating_sub(x)
        .saturating_sub(PERCENTAGE_WIDTH + SUFFIX_WIDTH);
    if track_w < MIN_TRACK_WIDTH {
        // Too narrow for a meaningful bar: percentage only (reference).
        buffer.set_stringn(
            x,
            area.y,
            pct.trim_start(),
            usize::from(right.saturating_sub(x).min(area.right().saturating_sub(x))),
            system.style(Role::TextSecondary),
        );
        return;
    }

    let filled = ((track_w as f64) * fraction).round() as u16;
    let fill_style = system.style(status.fill_role());
    let track_style = system.style(Role::Border);
    for i in 0..track_w {
        buffer.set_string(
            x.saturating_add(i),
            area.y,
            if i < filled { "\u{2501}" } else { "\u{2500}" },
            if i < filled { fill_style } else { track_style },
        );
    }
    x = x.saturating_add(track_w);
    buffer.set_stringn(
        x,
        area.y,
        format!(" {pct}"),
        usize::from(PERCENTAGE_WIDTH),
        system.style(Role::TextSecondary),
    );
    buffer.set_stringn(
        x.saturating_add(PERCENTAGE_WIDTH),
        area.y,
        suffix,
        usize::from(SUFFIX_WIDTH),
        fill_style,
    );
}

/// Indeterminate bar: a short accent segment sweeping a quiet track.
///
/// The sweep *is* the animation; the caller owns the tick through
/// [`ProgressKind::indeterminate_from`], whose `Off` answer parks the segment.
fn render_indeterminate(
    area: Rect,
    buffer: &mut Buffer,
    label: Option<&str>,
    tick: u64,
    system: &DesignSystem,
) {
    let mut x = area.x;
    if let Some(label) = label {
        let label_w = u16::try_from(display_cols(label)).unwrap_or(u16::MAX);
        if label_w > 0 && area.width > label_w.saturating_add(8) {
            buffer.set_stringn(
                x,
                area.y,
                label,
                usize::from(label_w),
                system.style(Role::Text),
            );
            x = x.saturating_add(label_w + 2);
        }
    }
    let track_w = area.right().saturating_sub(x);
    if track_w == 0 {
        return;
    }
    let track = i64::from(track_w);
    // Segment length is a fraction of the track, clamped so it stays readable
    // at both extremes; the sweep period wraps the segment past the far edge.
    let seg = i64::from((track_w / 5).clamp(2, 8));
    let period = track + seg;
    let pos = i64::try_from(tick % period as u64).unwrap_or(0) - seg;
    let fill_style = system.style(Role::Accent);
    let track_style = system.style(Role::Border);
    for i in 0..track {
        let in_seg = i >= pos && i < pos + seg;
        buffer.set_string(
            x.saturating_add(i as u16),
            area.y,
            if in_seg { "\u{2501}" } else { "\u{2500}" },
            if in_seg { fill_style } else { track_style },
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn system() -> DesignSystem {
        DesignSystem::junie()
    }

    fn rendered(buffer: &Buffer) -> String {
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }

    fn determinate(fraction: f64, width: u16) -> Buffer {
        let area = Rect::new(0, 0, width, 1);
        let mut buffer = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction }, &system()))
            .render(area, &mut buffer);
        buffer
    }

    #[test]
    fn track_is_rule_glyphs_not_blocks() {
        let buffer = determinate(0.5, 30);
        let row = rendered(&buffer);
        assert!(row.contains('\u{2501}'), "fill is ━: {row:?}");
        assert!(row.contains('\u{2500}'), "track is ─: {row:?}");
        assert!(!row.contains('█'), "no block fill: {row:?}");
        assert!(!row.contains('░'), "no shade track: {row:?}");
        assert!(!row.contains('▄'), "no partial ramp cell: {row:?}");
    }

    #[test]
    fn percentage_column_and_status_suffix() {
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.643 }, &system()))
            .render(area, &mut buffer);
        let row = rendered(&buffer);
        assert!(
            row.contains(" 64%"),
            "percentage is the {{:>4}} column: {row:?}"
        );

        for (status, suffix) in [
            (ProgressStatus::Complete, " \u{2713}"),
            (ProgressStatus::Failed, " !"),
            (ProgressStatus::Paused, " \u{2016}"),
            (ProgressStatus::Running, "  "),
        ] {
            let mut buffer = Buffer::empty(area);
            (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system())
                .status(status))
                .render(area, &mut buffer);
            assert!(
                rendered(&buffer).contains(suffix),
                "{status:?} must paint {suffix:?}: {row:?}"
            );
        }
    }

    #[test]
    fn running_reserves_the_two_cell_suffix_column() {
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.0 }, &system())
            .label("Building  "))
            .render(area, &mut buffer);
        let row = rendered(&buffer);
        let last_track = (0..area.width)
            .rev()
            .find(|&x| buffer[(x, 0)].symbol() == "\u{2500}")
            .expect("track");
        let pct = (0..area.width)
            .find(|&x| buffer[(x, 0)].symbol() == "0")
            .expect("percent");
        assert!(
            pct > last_track.saturating_add(1),
            "percent sits after a pad cell, not packed against the track: {row:?}"
        );
        assert_eq!(
            area.width.saturating_sub(last_track.saturating_add(1)),
            PERCENTAGE_WIDTH + SUFFIX_WIDTH,
            "running still reserves pct+suffix, got {row:?}"
        );
    }

    #[test]
    fn running_bar_is_never_green_and_complete_is() {
        let system = system();
        let green = system.style(Role::Accent).fg.expect("accent");
        let area = Rect::new(0, 0, 40, 1);
        let is_green = |buffer: &Buffer| {
            buffer
                .content()
                .iter()
                .any(|cell| cell.fg == green || cell.bg == green)
        };
        let mut running = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system))
            .render(area, &mut running);
        assert!(!is_green(&running), "a running bar is white 70%");
        assert_eq!(
            running[(0, 0)].fg,
            system
                .style(Role::TextSecondary)
                .fg
                .expect("text_secondary"),
            "running fill is text_secondary"
        );

        let mut complete = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 1.0 }, &system)
            .status(ProgressStatus::Complete))
            .render(area, &mut complete);
        assert!(is_green(&complete), "completion spends the one green");
        assert_eq!(
            complete[(0, 0)].fg,
            system.style(Role::Success).fg.expect("success"),
            "completed fill is success"
        );
    }

    #[test]
    fn track_is_border_subtle() {
        let system = system();
        let buffer = determinate(0.0, 40);
        let track = system.style(Role::Border).fg.expect("border");
        assert!(
            (0..30).all(|x| buffer[(x, 0)].fg == track),
            "empty track is border_subtle"
        );
    }

    #[test]
    fn zero_and_one_boundaries() {
        let empty = determinate(0.0, 40);
        assert_eq!((empty[(0, 0)]).symbol(), "\u{2500}");
        let full = determinate(1.0, 40);
        assert_eq!((full[(0, 0)]).symbol(), "\u{2501}");
        assert_eq!((full[(28, 0)]).symbol(), "\u{2501}");
    }

    #[test]
    fn clamps_out_of_range_fractions() {
        for fraction in [1.5, 42.0] {
            let buffer = determinate(fraction, 40);
            assert!(
                rendered(&buffer).contains("100%"),
                "{fraction} must clamp to 100%"
            );
        }
        // Non-finite values are not a ratio; they paint as empty, not 100%.
        for fraction in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let buffer = determinate(fraction, 40);
            assert!(
                rendered(&buffer).contains("  0%"),
                "{fraction} must clamp to 0%"
            );
        }
        for fraction in [-0.25, -7.0] {
            let buffer = determinate(fraction, 40);
            assert!(
                rendered(&buffer).contains("  0%"),
                "{fraction} must clamp to 0%"
            );
            assert_eq!(buffer[(0, 0)].symbol(), "\u{2500}", "clamped empty");
        }
    }

    #[test]
    fn label_drops_when_it_would_starve_the_track() {
        let system = system();
        let area = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system)
            .label("An extremely long build label"))
            .render(area, &mut buffer);
        let row = rendered(&buffer);
        assert!(
            !row.contains("extremely"),
            "label must drop, not clip: {row:?}"
        );
        assert!(row.contains('\u{2501}'), "the bar survives: {row:?}");
        assert!(row.contains('%'));

        let mut wide = Buffer::empty(Rect::new(0, 0, 60, 1));
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system).label("Build"))
            .render(Rect::new(0, 0, 60, 1), &mut wide);
        let row = rendered(&wide);
        assert!(row.contains("Build"), "a fitting label is painted: {row:?}");
        assert_eq!(
            wide[(0, 0)].fg,
            system.style(Role::Text).fg.expect("text"),
            "label is text_primary"
        );
    }

    #[test]
    fn indeterminate_is_an_accent_sweep_on_a_quiet_track() {
        let system = system();
        let accent = system.style(Role::Accent).fg.expect("accent");
        let border = system.style(Role::Border).fg.expect("border");
        let area = Rect::new(0, 0, 20, 1);
        let mut ever_fill = false;
        let mut ever_track = false;
        for tick in 0..40u64 {
            let mut buffer = Buffer::empty(area);
            (&ProgressBar::new(ProgressKind::Indeterminate { tick }, &system))
                .render(area, &mut buffer);
            for x in 0..area.width {
                match buffer[(x, 0)].fg {
                    c if c == accent => ever_fill = true,
                    c if c == border => ever_track = true,
                    _ => {}
                }
            }
        }
        assert!(ever_fill, "sweep never painted an accent cell");
        assert!(ever_track, "sweep never painted a track cell");
    }

    #[test]
    fn indeterminate_tick_is_deterministic_and_off_parks_the_segment() {
        let system = system();
        let area = Rect::new(0, 0, 20, 1);
        // Same tick, same picture.
        let bar = ProgressBar::new(ProgressKind::Indeterminate { tick: 3 }, &system);
        let mut first = Buffer::empty(area);
        let mut second = Buffer::empty(area);
        (&bar).render(area, &mut first);
        (&bar).render(area, &mut second);
        assert_eq!(first, second);

        // Off parks on the first frame, so however far apart two ticks are the
        // segment sits in the same place.
        let start = web_time::Instant::now();
        let at = |ms: u64| {
            FrameTick::manual(
                start + Duration::from_millis(ms),
                Duration::from_millis(ms),
                Duration::from_millis(16),
            )
        };
        let parked = ProgressBar::new(
            ProgressKind::indeterminate_from(at(9_600), MotionPolicy::Off),
            &system,
        );
        let moved = ProgressBar::new(
            ProgressKind::indeterminate_from(at(400), MotionPolicy::Full),
            &system,
        );
        let mut a = Buffer::empty(area);
        let mut b = Buffer::empty(area);
        (&parked).render(area, &mut a);
        (&moved).render(area, &mut b);
        assert_eq!(
            ProgressKind::indeterminate_from(at(0), MotionPolicy::Off),
            ProgressKind::Indeterminate { tick: 0 },
            "Off freezes the cadence on frame zero"
        );
        assert_ne!(a, b, "Full must actually sweep");
    }

    #[test]
    fn tiny_widths_are_safe() {
        let system = system();
        for width in [0u16, 1, 4, 9] {
            let area = Rect::new(0, 0, width, 1);
            let mut buffer = Buffer::empty(area);
            (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system))
                .render(area, &mut buffer);
            let mut sweep = Buffer::empty(area);
            (&ProgressBar::new(ProgressKind::Indeterminate { tick: 2 }, &system))
                .render(area, &mut sweep);
        }
    }

    #[test]
    fn narrow_row_paints_percentage_only() {
        let system = system();
        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);
        (&ProgressBar::new(ProgressKind::Determinate { fraction: 0.62 }, &system))
            .render(area, &mut buffer);
        let row = rendered(&buffer);
        assert!(
            row.contains("62%"),
            "percentage survives the squeeze: {row:?}"
        );
        assert!(!row.contains('\u{2501}'), "no track fits: {row:?}");
    }

    #[test]
    fn detailed_recipe_carries_muted_meta() {
        let system = system();
        let mut state = ProgressBarState::transfer(512, 1024);
        state.set_label("Download");
        state.set_recipe(ProgressRecipe::Detailed);
        let area = Rect::new(0, 0, 60, 1);
        let mut buffer = Buffer::empty(area);
        ProgressBar::paint_state(
            &system,
            area,
            &mut buffer,
            &mut state,
            FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO),
            MotionPolicy::Off,
        );
        let row = rendered(&buffer);
        assert!(row.contains("Download"), "{row:?}");
        assert!(row.contains("512B/1.0K"), "units meta survives: {row:?}");
        assert_eq!(
            buffer[(area.right() - 1, 0)].fg,
            system.style(Role::TextMuted).fg.expect("text_muted"),
            "meta is metadata"
        );
    }

    #[test]
    fn multiline_recipe_paints_title_track_meta() {
        let system = system();
        let area = Rect::new(0, 0, 40, 3);
        let mut buffer = Buffer::empty(area);
        ProgressBar::new(ProgressKind::Determinate { fraction: 0.4 }, &system)
            .label("Download")
            .recipe(ProgressRecipe::MultiLine)
            .meta("12M/30M · 2.1M/s · ETA 9s")
            .paint(area, &mut buffer);
        let row = rendered(&buffer);
        assert!(row.contains("Download"), "{row:?}");
        assert!(row.contains("ETA 9s"), "{row:?}");
        assert!(
            rendered(&buffer).contains('\u{2501}'),
            "track row carries fill: {}",
            rendered(&buffer)
        );
    }

    #[test]
    fn state_transfer_units_and_eta() {
        let mut s = ProgressBarState::transfer(512, 1024);
        s.set_rate(Some(256.0));
        s.recompute_eta();
        assert!((s.fraction() - 0.5).abs() < 0.001);
        assert!(s.units_text().unwrap().contains('K') || s.units_text().unwrap().contains('B'));
        assert!(s.eta_secs.is_some());
        assert!(s.eta_text().is_some());
        assert_eq!(s.percentage_text(), " 50%");
    }

    #[test]
    fn throttle_drops_rapid_updates() {
        let mut s = ProgressBarState::new();
        s.set_total(100.0);
        s.set_throttle(Duration::from_millis(100));
        let t0 = Instant::now();
        assert!(s.set_value_throttled(10.0, t0));
        assert!(!s.set_value_throttled(11.0, t0 + Duration::from_millis(10)));
        assert!(s.set_value_throttled(12.0, t0 + Duration::from_millis(150)));
        // terminal value always accepted
        assert!(s.set_value_throttled(100.0, t0 + Duration::from_millis(151)));
    }

    #[test]
    fn only_indeterminate_asks_for_frames() {
        let s = ProgressBarState::task(1, 2);
        let tick = FrameTick::manual(Instant::now(), Duration::from_millis(100), Duration::ZERO);
        assert!(!s.animation_demand(tick, MotionPolicy::Full).needs_redraw);

        let mut ind = ProgressBarState::new(); // total 0 → indeterminate
        assert!(ind.animation_demand(tick, MotionPolicy::Full).needs_redraw);
        assert!(
            !ind.animation_demand(tick, MotionPolicy::Off).needs_redraw,
            "reduced motion never asks for frames"
        );
        ind.set_active(false);
        assert!(!ind.animation_demand(tick, MotionPolicy::Full).needs_redraw);
    }

    #[test]
    fn paused_buffering_cancelled_statuses() {
        for st in [
            ProgressStatus::Paused,
            ProgressStatus::Buffering,
            ProgressStatus::Cancelled,
            ProgressStatus::Complete,
        ] {
            assert!(!st.id().is_empty());
        }
        assert!(!ProgressStatus::Paused.animates());
        assert!(ProgressStatus::Buffering.animates());
        assert_eq!(ProgressStatus::Cancelled.suffix(), " ×");
        assert_eq!(
            ProgressStatus::Buffering.fill_role(),
            Role::TextSecondary,
            "buffering still paints as running"
        );
    }

    #[test]
    fn needs_paint_generation() {
        let mut s = ProgressBarState::new();
        assert!(s.needs_paint() || s.generation() == 0);
        s.set_value(1.0);
        assert!(s.needs_paint());
        s.mark_painted();
        assert!(!s.needs_paint());
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let mut scene = SemanticScene::<&str, ()>::default();
        ProgressBar::new(ProgressKind::Determinate { fraction: 0.2 }, &system)
            .label("x")
            .register_semantic(&mut scene, "p", Rect::new(0, 0, 20, 1));
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("progress-bar"))
        );
    }

    #[test]
    fn fuzz_fractions_and_widths() {
        let system = DesignSystem::default();
        let mut seed = 3u64;
        for _ in 0..80 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let w = (seed % 40) as u16 + 1;
            let f = (seed % 2001) as f64 / 1000.0 - 0.5;
            let area = Rect::new(0, 0, w, 1);
            let mut buf = Buffer::empty(area);
            ProgressBar::new(ProgressKind::Determinate { fraction: f }, &system)
                .label("T")
                .paint(area, &mut buf);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut s = ProgressBarState::transfer(0, 10_000_000);
        s.set_label("dl");
        s.set_rate(Some(1_000_000.0));
        s.set_recipe(ProgressRecipe::Detailed);
        let mut terminal = Terminal::new(TestBackend::new(60, 4)).unwrap();
        let start = Instant::now();
        for i in 0..150u64 {
            s.set_value((i * 50_000) as f64);
            s.recompute_eta();
            terminal
                .draw(|f| {
                    ProgressBar::paint_state(
                        &system,
                        f.area(),
                        f.buffer_mut(),
                        &mut s,
                        FrameTick::manual(
                            Instant::now(),
                            Duration::from_millis(i * 16),
                            Duration::ZERO,
                        ),
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
            let mut t = Terminal::new(TestBackend::new(32, 1)).unwrap();
            t.draw(|f| {
                ProgressBar::new(ProgressKind::Determinate { fraction: 0.62 }, &system)
                    .label("Build")
                    .paint(f.area(), f.buffer_mut());
            })
            .unwrap();
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }
}
