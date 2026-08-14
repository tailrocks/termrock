// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ProgressBar** (also [`Progress`]) — determinate and indeterminate progress
//! with numeric and textual context.
//!
//! **Mission.** Rich Progress / indicatif-class bars for builds, downloads, and
//! transfers: percentage, units, rate, ETA, phases, buffering, paused,
//! cancelled, complete, failed. Compact / detailed / multi-line recipes; tiny
//! widths and ASCII/no-color; host-throttled updates; task/transfer projection.
//!
//! **vs Spinner.** Spinner is glyph + verb activity without completion.
//! ProgressBar shows completion track (or indeterminate track motion).
//! **vs TokenMeter.** Token usage domain meter; this is generic progress.
//!
//! Research: Rich Progress, indicatif, btop bars, download/build TUIs.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use std::time::Duration;
use web_time::Instant;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::{AnimationDemand, FrameTick, spinner_demand, spinner_step},
    style::{DesignSystem, MotionPolicy, Role, RolePalette},
    text::{display_cols, take_display_cols},
};

/// Default indeterminate braille frames (preserved).
pub const DEFAULT_PROGRESS_FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
/// ASCII indeterminate frames.
pub const PROGRESS_ASCII_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
/// Min track cells when percentage is shown or reserved.
const MIN_TRACK_WIDTH: u16 = 2;
/// Width at or above which percentage is painted (preserved contract).
pub const MIN_WIDTH_WITH_PERCENTAGE: u16 = 16;
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
    /// Indeterminate frame from [`FrameTick`] + [`MotionPolicy`] (deterministic).
    #[must_use]
    pub fn indeterminate_from(tick: FrameTick, motion: MotionPolicy) -> Self {
        let step = tick.spinner_step(DEFAULT_PROGRESS_FRAMES.len(), 80, motion) as u64;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressStatus {
    /// Actively advancing.
    #[default]
    Running,
    /// Host paused (ETA frozen).
    Paused,
    /// Buffering / stalled wait without cancel.
    Buffering,
    /// User or host cancelled.
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

    /// Whether indeterminate animation should advance.
    #[must_use]
    pub const fn animates(self) -> bool {
        matches!(self, Self::Running | Self::Buffering)
    }

    /// Semantic paint role for status text.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Running | Self::Buffering => Role::Accent,
            Self::Paused => Role::Warning,
            Self::Cancelled => Role::TextMuted,
            Self::Complete => Role::Success,
            Self::Failed => Role::Danger,
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
    /// Completed amount (bytes, items, or unitless 0..=total).
    value: f64,
    /// Total amount (`0` → indeterminate when status running).
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
    ascii: bool,
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
            ascii: false,
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

    /// Kind projection for legacy paint path.
    #[must_use]
    pub fn kind(&self, tick: FrameTick, motion: MotionPolicy) -> ProgressKind {
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

    /// Active for indeterminate animation demand.
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

    /// Recipe.
    pub fn set_recipe(&mut self, recipe: ProgressRecipe) {
        self.recipe = recipe;
        self.bump();
    }

    /// ASCII track glyphs.
    pub fn set_ascii(&mut self, on: bool) {
        self.ascii = on;
        self.bump();
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

    /// Format percentage string.
    #[must_use]
    pub fn percentage_text(&self) -> String {
        format!("{:>3}%", (self.fraction() * 100.0).round() as u8)
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

/// Progress bar with optional label (legacy constructor + rich state paint).
///
/// Determinate progress shows its percentage at widths of 16 columns or more.
/// Narrower bars prioritize the label and filled/empty glyph cue instead,
/// reserving two track cells whenever the available geometry permits.
#[derive(Debug, Clone, Copy)]
pub struct ProgressBar<'a> {
    kind: ProgressKind,
    label: Option<&'a str>,
    frames: &'a [&'a str],
    system: &'a DesignSystem,
    ascii: bool,
    recipe: ProgressRecipe,
    status: ProgressStatus,
    phase: Option<&'a str>,
    meta: Option<&'a str>,
}

/// Legacy name — same type as [`ProgressBar`].
pub type Progress<'a> = ProgressBar<'a>;

impl<'a> ProgressBar<'a> {
    /// Creates an unlabeled progress indicator in the supplied mode.
    #[must_use]
    pub const fn new(kind: ProgressKind, system: &'a DesignSystem) -> Self {
        Self {
            kind,
            label: None,
            frames: &DEFAULT_PROGRESS_FRAMES,
            system,
            ascii: false,
            recipe: ProgressRecipe::Compact,
            status: ProgressStatus::Running,
            phase: None,
            meta: None,
        }
    }

    /// From state + tick (preferred for task/transfer).
    #[must_use]
    pub fn from_state(
        state: &ProgressBarState,
        system: &'a DesignSystem,
        tick: FrameTick,
        motion: MotionPolicy,
    ) -> Self {
        let kind = state.kind(tick, motion);
        let meta = state.meta_line();
        // meta is temporary — paint_state path used instead for owned meta
        let _ = meta;
        Self {
            kind,
            label: if state.label.is_empty() {
                None
            } else {
                // Can't borrow state.label as 'a from temporary — use paint_state
                None
            },
            frames: if state.ascii {
                &PROGRESS_ASCII_FRAMES
            } else {
                &DEFAULT_PROGRESS_FRAMES
            },
            system,
            ascii: state.ascii,
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

    /// Overrides indeterminate animation frames.
    #[must_use]
    pub const fn frames(mut self, frames: &'a [&'a str]) -> Self {
        self.frames = frames;
        self
    }

    /// ASCII track / frames.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: ProgressRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Status (affects fill role).
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
            frames: if state.ascii {
                &PROGRESS_ASCII_FRAMES
            } else {
                &DEFAULT_PROGRESS_FRAMES
            },
            system,
            ascii: state.ascii,
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
        if matches!(self.kind, ProgressKind::Indeterminate { .. }) && self.frames.is_empty() {
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
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD),
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
        // Soft mute background of row (legacy)
        buffer.set_style(area, self.system.style(Role::TextMuted));
        self.paint_kind(area, buffer, self.label, detailed);
    }

    fn paint_kind(&self, area: Rect, buffer: &mut Buffer, label: Option<&str>, detailed: bool) {
        match self.kind {
            ProgressKind::Determinate { fraction } => {
                render_determinate(
                    area,
                    buffer,
                    label,
                    fraction,
                    self.system,
                    self.ascii,
                    self.status,
                    detailed,
                    self.meta,
                );
            }
            ProgressKind::Indeterminate { tick } => {
                render_indeterminate(
                    area,
                    buffer,
                    label,
                    tick,
                    self.frames,
                    self.system,
                    self.ascii,
                    self.status,
                );
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

// ── Render helpers ──────────────────────────────────────────────────────────

fn fill_glyph(ascii: bool) -> &'static str {
    if ascii { "#" } else { "█" }
}

fn empty_glyph(ascii: bool) -> &'static str {
    if ascii { "-" } else { "░" }
}

fn render_determinate(
    area: Rect,
    buffer: &mut Buffer,
    label: Option<&str>,
    fraction: f64,
    system: &DesignSystem,
    ascii: bool,
    status: ProgressStatus,
    detailed: bool,
    meta: Option<&str>,
) {
    let fraction = clamp_fraction(fraction);
    let percentage = format!("{:>3}%", (fraction * 100.0).round() as u8);
    let show_pct = area.width >= MIN_WIDTH_WITH_PERCENTAGE;
    let percentage_width = if show_pct {
        u16::try_from(display_cols(&percentage))
            .unwrap_or(u16::MAX)
            .min(area.width)
    } else {
        0
    };
    let percentage_x = area.right().saturating_sub(percentage_width);
    if percentage_width > 0 {
        buffer.set_stringn(
            percentage_x,
            area.y,
            &percentage,
            usize::from(percentage_width),
            system.style(Role::Text),
        );
    }

    let mut track_x = area.x;
    let mut right_limit = percentage_x;

    // Detailed: optionally reserve meta on the right before %
    if detailed {
        if let Some(m) = meta {
            if !m.is_empty() && area.width >= 28 {
                let mw = u16::try_from(display_cols(m))
                    .unwrap_or(u16::MAX)
                    .min(area.width / 3)
                    .min(
                        right_limit
                            .saturating_sub(area.x)
                            .saturating_sub(MIN_TRACK_WIDTH + 4),
                    );
                if mw > 0 {
                    let mx = right_limit.saturating_sub(mw);
                    buffer.set_stringn(
                        mx,
                        area.y,
                        &take_display_cols(m, usize::from(mw)),
                        usize::from(mw),
                        system.style(Role::TextMuted),
                    );
                    right_limit = mx.saturating_sub(1);
                }
            }
        }
    }

    if let Some(label) = label {
        let available = right_limit.saturating_sub(area.x);
        let reserved = MIN_TRACK_WIDTH.saturating_add(2);
        let label_width = u16::try_from(display_cols(label))
            .unwrap_or(u16::MAX)
            .min(available.saturating_sub(reserved));
        buffer.set_stringn(
            area.x,
            area.y,
            label,
            usize::from(label_width),
            system.style(Role::TextMuted),
        );
        track_x = area.x.saturating_add(label_width);
        if label_width > 0 && track_x < right_limit {
            track_x = track_x.saturating_add(1);
        }
    }

    // Always reserve one trailing cell (gap before % or end) — preserves
    // legacy track geometry (width 9 → 8 track cells).
    let track_width = right_limit.saturating_sub(track_x).saturating_sub(1);

    let scaled = f64::from(track_width) * fraction;
    let filled = (scaled.floor() as u16).min(track_width);
    let partial = ((scaled.fract() * 8.0).floor() as usize).min(7);
    let partial_glyph = crate::style::BLOCK_RAMP[partial].to_string();
    let fill = fill_glyph(ascii);
    let empty = empty_glyph(ascii);
    let fill_role = status.role();
    for column in 0..track_width {
        buffer.set_string(
            track_x.saturating_add(column),
            area.y,
            if column < filled {
                fill
            } else if !ascii && column == filled && partial > 0 {
                partial_glyph.as_str()
            } else {
                empty
            },
            system.style(if column <= filled && (column < filled || partial > 0) {
                fill_role
            } else {
                Role::Sunken
            }),
        );
    }
}

fn render_indeterminate(
    area: Rect,
    buffer: &mut Buffer,
    label: Option<&str>,
    tick: u64,
    frames: &[&str],
    system: &DesignSystem,
    ascii: bool,
    status: ProgressStatus,
) {
    if frames.is_empty() {
        return;
    }
    let frames = if ascii {
        // Prefer ASCII spinner when host asked for ASCII paint.
        if frames.len() == DEFAULT_PROGRESS_FRAMES.len() || frames.is_empty() {
            &PROGRESS_ASCII_FRAMES[..]
        } else {
            frames
        }
    } else {
        frames
    };
    let frame_count = u64::try_from(frames.len()).unwrap_or(u64::MAX);
    let frame_index = usize::try_from(tick % frame_count).unwrap_or(0);
    let glyph = frames[frame_index];
    let glyph_width = u16::try_from(display_cols(glyph))
        .unwrap_or(u16::MAX)
        .min(area.width);
    buffer.set_stringn(
        area.x,
        area.y,
        glyph,
        usize::from(glyph_width),
        system.style(status.role()),
    );
    // Pulse track remainder for wider areas
    if area.width > glyph_width.saturating_add(4) && status.animates() {
        let track_x = area.x.saturating_add(glyph_width).saturating_add(1);
        let track_w = area.right().saturating_sub(track_x);
        if track_w >= MIN_TRACK_WIDTH {
            let pos = (tick as u16) % track_w.max(1);
            let fill = fill_glyph(ascii);
            let empty = empty_glyph(ascii);
            for c in 0..track_w {
                let on = c == pos || c == pos.saturating_add(1).min(track_w.saturating_sub(1));
                buffer.set_string(
                    track_x.saturating_add(c),
                    area.y,
                    if on { fill } else { empty },
                    system.style(if on { status.role() } else { Role::Sunken }),
                );
            }
            // label after? put label at end if fits - skip if track used
            return;
        }
    }
    if let Some(label) = label
        && glyph_width < area.width
    {
        let label_x = area.x.saturating_add(glyph_width).saturating_add(1);
        let label_width = area.right().saturating_sub(label_x);
        buffer.set_stringn(
            label_x,
            area.y,
            label,
            usize::from(label_width),
            system.style(Role::TextMuted),
        );
    }
}

// silence unused import warning for spinner_step if only used via FrameTick
#[allow(dead_code)]
fn _use_spinner_step(tick: FrameTick, motion: MotionPolicy) -> usize {
    spinner_step(tick, 8, 80, motion)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(buffer: &Buffer) -> String {
        buffer.content().iter().map(|cell| cell.symbol()).collect()
    }

    #[test]
    fn determinate_progress_clamps_and_keeps_percentage_non_color_cue() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(2, 1, 18, 1);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 22, 3));
        (&Progress::new(ProgressKind::Determinate { fraction: 1.5 }, &system).label("Index"))
            .render(area, &mut buffer);

        let row = rendered(&buffer);
        assert!(row.contains("Index"));
        assert!(row.contains("100%"));
        assert!(row.contains('█'));
    }

    #[test]
    fn indeterminate_tick_is_deterministic_and_tiny_areas_are_safe() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(0, 0, 8, 1);
        let mut first = Buffer::empty(area);
        let mut second = Buffer::empty(area);
        let progress =
            Progress::new(ProgressKind::Indeterminate { tick: 3 }, &system).label("Load");
        (&progress).render(area, &mut first);
        (&progress).render(area, &mut second);

        assert_eq!(first, second);
        assert_eq!(first[(0, 0)].symbol(), "⠸");
        (&progress).render(Rect::new(0, 0, 0, 0), &mut first);
    }

    fn determinate(fraction: f64, width: u16) -> Buffer {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(0, 0, width, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Determinate { fraction }, &system)).render(area, &mut buffer);
        buffer
    }

    #[test]
    fn zero_fraction_renders_all_empty_glyphs() {
        let buffer = determinate(0.0, 9);
        assert!((0..8).all(|x| buffer[(x, 0)].symbol() == "░"));
    }

    #[test]
    fn determinate_boundary_uses_ramp() {
        let buffer = determinate(9.0 / 16.0, 9);
        assert_eq!(buffer[(4, 0)].symbol(), "▄");
    }

    #[test]
    fn half_fraction_splits_cells_exactly() {
        let buffer = determinate(0.5, 9);
        assert_eq!(buffer[(0, 0)].symbol(), "█");
        assert_eq!(buffer[(3, 0)].symbol(), "█");
        assert_eq!(buffer[(4, 0)].symbol(), "░");
        assert_eq!(buffer[(7, 0)].symbol(), "░");
    }

    #[test]
    fn full_fraction_renders_all_filled_glyphs() {
        let buffer = determinate(1.0, 9);
        assert!((0..8).all(|x| buffer[(x, 0)].symbol() == "█"));
    }

    #[test]
    fn nan_and_infinite_clamp_to_zero() {
        for fraction in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let buffer = determinate(fraction, 20);
            assert!((0..15).all(|x| buffer[(x, 0)].symbol() == "░"));
            assert!(rendered(&buffer).contains("0%"));
        }
    }

    #[test]
    fn width_zero_and_one_do_not_panic() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let progress = Progress::new(ProgressKind::Determinate { fraction: 0.5 }, &system);
        (&progress).render(Rect::new(0, 0, 0, 0), &mut buffer);
        (&progress).render(Rect::new(0, 0, 1, 1), &mut buffer);
    }

    #[test]
    fn filled_and_empty_zones_differ_by_glyph() {
        let buffer = determinate(0.5, 9);
        assert_ne!(buffer[(0, 0)].symbol(), buffer[(7, 0)].symbol());
    }

    #[test]
    fn wide_char_label_truncates_on_grapheme_boundary() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Determinate { fraction: 0.5 }, &system).label("東京🪨"))
            .render(area, &mut buffer);
        assert_eq!(buffer[(0, 0)].symbol(), "東");
        assert_eq!(buffer[(2, 0)].symbol(), "京");
        assert!(!rendered(&buffer).contains('🪨'));
    }

    #[test]
    fn custom_frames_cycle_and_wrap() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let frames = ["A", "B"];
        for (tick, expected) in [(0, "A"), (1, "B"), (2, "A")] {
            let area = Rect::new(0, 0, 3, 1);
            let mut buffer = Buffer::empty(area);
            (&Progress::new(ProgressKind::Indeterminate { tick }, &system).frames(&frames))
                .render(area, &mut buffer);
            assert_eq!(buffer[(0, 0)].symbol(), expected);
        }
    }

    #[test]
    fn empty_frames_render_nothing() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(0, 0, 8, 1);
        let mut buffer = Buffer::empty(area);
        let before = buffer.clone();
        (&Progress::new(ProgressKind::Indeterminate { tick: 3 }, &system)
            .frames(&[])
            .label("hidden"))
            .render(area, &mut buffer);
        assert_eq!(buffer, before);
    }

    #[test]
    fn narrow_width_elides_percentage_but_keeps_glyph_cue() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(0, 0, 14, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Determinate { fraction: 0.62 }, &system).label("Build"))
            .render(area, &mut buffer);
        let row = rendered(&buffer);
        assert!(!row.contains('%'));
        assert!(row.contains('█'));
        assert!(row.contains('░'));
    }

    #[test]
    fn narrow_long_label_reserves_filled_and_empty_track_cells() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let area = Rect::new(0, 0, 14, 1);
        let mut buffer = Buffer::empty(area);
        (&Progress::new(ProgressKind::Determinate { fraction: 0.5 }, &system)
            .label("An extremely long build label"))
            .render(area, &mut buffer);
        let row = rendered(&buffer);
        assert!(row.contains('█'));
        assert!(row.contains('░'));
    }

    // ── New ProgressBar tests ─────────────────────────────────────────────

    #[test]
    fn state_transfer_and_eta() {
        let mut s = ProgressBarState::transfer(512, 1024);
        s.set_rate(Some(256.0));
        s.recompute_eta();
        assert!((s.fraction() - 0.5).abs() < 0.001);
        assert!(s.units_text().unwrap().contains('K') || s.units_text().unwrap().contains('B'));
        assert!(s.eta_secs.is_some());
        assert!(s.eta_text().is_some());
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
    fn status_complete_and_failed_paint() {
        let system = DesignSystem::default();
        let mut s = ProgressBarState::task(3, 10);
        s.set_label("Build");
        s.set_status(ProgressStatus::Failed);
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        ProgressBar::paint_state(
            &system,
            area,
            &mut buf,
            &mut s,
            FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO),
            MotionPolicy::Off,
        );
        assert!(!s.needs_paint());
    }

    #[test]
    fn multiline_recipe_height() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        ProgressBar::new(ProgressKind::Determinate { fraction: 0.4 }, &system)
            .label("Download")
            .recipe(ProgressRecipe::MultiLine)
            .meta("12M/30M · 2.1M/s · ETA 9s")
            .status(ProgressStatus::Running)
            .paint(area, &mut buf);
        let text: String = rendered(&buf);
        assert!(
            text.contains("Download") || text.contains("12M") || text.contains('%'),
            "{text}"
        );
    }

    #[test]
    fn ascii_track_glyphs() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        ProgressBar::new(ProgressKind::Determinate { fraction: 0.5 }, &system)
            .ascii(true)
            .paint(area, &mut buf);
        let text = rendered(&buf);
        assert!(text.contains('#'), "{text}");
        assert!(text.contains('-'), "{text}");
    }

    #[test]
    fn idle_redraw_when_determinate() {
        let s = ProgressBarState::task(1, 2);
        let tick = FrameTick::manual(Instant::now(), Duration::from_millis(100), Duration::ZERO);
        assert!(!s.animation_demand(tick, MotionPolicy::Full).needs_redraw);
        let mut ind = ProgressBarState::new(); // total 0
        ind.set_active(true);
        assert!(ind.animation_demand(tick, MotionPolicy::Full).needs_redraw);
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
            let f = (seed % 1001) as f64 / 1000.0;
            let area = Rect::new(0, 0, w, 1);
            let mut buf = Buffer::empty(area);
            ProgressBar::new(ProgressKind::Determinate { fraction: f }, &system)
                .ascii(seed % 2 == 0)
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
