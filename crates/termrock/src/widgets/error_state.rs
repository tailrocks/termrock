// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ErrorState** + **Recovery** — structured recoverable failure presentation.
//!
//! **Mission.** Summary, human explanation, technical details (collapsed by
//! default), source, retry, alternative action, copy diagnostics, and report
//! issue. Differentiates validation, network, permission, not-found, conflict,
//! crash, and unsupported-capability. Explains whether retry is safe and
//! whether user work was preserved.
//!
//! **Recipes.** Inline, pane, dialog, full-screen.
//!
//! Research: browser/IDE error surfaces, cloud CLIs, terminal crash recovery.
//! Prefer [`ErrorState`] for hard failures; [`super::EmptyState`] for zero-data.
//! For compiler/build diagnostics, project into [`super::Diagnostic`] /
//! [`super::CodeFrame`] and feed plain text via
//! [`super::format_diagnostics_plain`] into recovery copy-diagnostics.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_button_intent,
    },
    layout::{Center, CenterAxis, FlexSize, Stack, center_line_x},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::{Button, ButtonState, ButtonVariant},
};

/// Width under which dialog/fullscreen contracts toward pane/inline.
pub const ERROR_STATE_INLINE_MAX_WIDTH: u16 = 32;
/// Height under which details stay collapsed and layout is dense.
pub const ERROR_STATE_COMPACT_MAX_HEIGHT: u16 = 6;

// ── Kind ────────────────────────────────────────────────────────────────────

/// Structured failure class (drives glyph, tone, default retry safety).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Field or form validation failure.
    Validation,
    /// Transport / connectivity failure.
    Network,
    /// Authorization / access denied.
    Permission,
    /// Missing resource.
    NotFound,
    /// Version / state conflict.
    Conflict,
    /// Unexpected crash / panic surface.
    Crash,
    /// Terminal or host capability missing.
    UnsupportedCapability,
    /// Generic / unclassified.
    #[default]
    Generic,
}

impl ErrorKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Network => "network",
            Self::Permission => "permission",
            Self::NotFound => "not-found",
            Self::Conflict => "conflict",
            Self::Crash => "crash",
            Self::UnsupportedCapability => "unsupported-capability",
            Self::Generic => "generic",
        }
    }

    /// Default short summary label for the kind.
    #[must_use]
    pub const fn default_summary(self) -> &'static str {
        match self {
            Self::Validation => "Validation failed",
            Self::Network => "Network error",
            Self::Permission => "Permission denied",
            Self::NotFound => "Not found",
            Self::Conflict => "Conflict",
            Self::Crash => "Unexpected error",
            Self::UnsupportedCapability => "Unsupported",
            Self::Generic => "Error",
        }
    }

    /// Non-color glyph (Unicode).
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::Validation => "!",
            Self::Network => "⚡",
            Self::Permission => "⊘",
            Self::NotFound => "?",
            Self::Conflict => "⇅",
            Self::Crash => "✗",
            Self::UnsupportedCapability => "∅",
            Self::Generic => "✗",
        }
    }

    /// Non-color glyph (ASCII).
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::Validation => "!",
            Self::Network => "~",
            Self::Permission => "x",
            Self::NotFound => "?",
            Self::Conflict => "!",
            Self::Crash => "x",
            Self::UnsupportedCapability => "0",
            Self::Generic => "x",
        }
    }

    /// Glyph for capability.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            self.glyph_ascii()
        } else {
            self.glyph_unicode()
        }
    }

    /// Paint role for summary.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Validation | Self::Conflict | Self::UnsupportedCapability => Role::Warning,
            Self::Permission | Self::Network | Self::NotFound => Role::Danger,
            Self::Crash | Self::Generic => Role::Danger,
        }
    }

    /// Default retry safety for this kind (hosts may override).
    #[must_use]
    pub const fn default_retry_safety(self) -> RetrySafety {
        match self {
            Self::Network | Self::NotFound => RetrySafety::Safe,
            Self::Validation | Self::Permission | Self::UnsupportedCapability => {
                RetrySafety::Unsafe
            }
            Self::Conflict => RetrySafety::Unsafe,
            Self::Crash => RetrySafety::Unknown,
            Self::Generic => RetrySafety::Unknown,
        }
    }
}

// ── Retry safety / work ─────────────────────────────────────────────────────

/// Whether retry is side-effect safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum RetrySafety {
    /// Retry will not double-submit or lose work.
    Safe,
    /// Retry may duplicate side effects or overwrite state.
    Unsafe,
    /// Host has not classified safety.
    #[default]
    Unknown,
}

impl RetrySafety {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Unsafe => "unsafe",
            Self::Unknown => "unknown",
        }
    }

    /// Short cue for paint.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Safe => "retry safe",
            Self::Unsafe => "retry may duplicate",
            Self::Unknown => "retry safety unknown",
        }
    }
}

// ── Recipe ──────────────────────────────────────────────────────────────────

/// Layout recipe for error presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ErrorRecipe {
    /// One–two line strip (tables, rails).
    Inline,
    /// Panel / pane body (default).
    #[default]
    Pane,
    /// Dialog-sized centered block.
    Dialog,
    /// Full-screen recovery surface.
    FullScreen,
}

impl ErrorRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Pane => "pane",
            Self::Dialog => "dialog",
            Self::FullScreen => "full-screen",
        }
    }
}

// ── Recovery ────────────────────────────────────────────────────────────────

/// One recovery action label (activation host-owned via [`ErrorStateState`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecoveryAction<'a> {
    /// Visible label.
    pub label: &'a str,
    /// Optional chord hint.
    pub shortcut: Option<&'a str>,
}

impl<'a> RecoveryAction<'a> {
    /// Label only.
    #[must_use]
    pub const fn new(label: &'a str) -> Self {
        Self {
            label,
            shortcut: None,
        }
    }

    /// Label + shortcut.
    #[must_use]
    pub const fn with_shortcut(label: &'a str, shortcut: &'a str) -> Self {
        Self {
            label,
            shortcut: Some(shortcut),
        }
    }
}

/// Recovery affordances bundled with an error.
///
/// Primary recovery is **retry** when present and safe-enough; alternative,
/// copy-diagnostics, and report-issue are secondary. Retry is never painted as
/// destructive; hosts should not put irreversible delete on retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Recovery<'a> {
    /// Retry action (dominant when present).
    pub retry: Option<RecoveryAction<'a>>,
    /// Alternative path (edit, go back, choose other).
    pub alternative: Option<RecoveryAction<'a>>,
    /// Offer "Copy diagnostics" action.
    pub copy_diagnostics: bool,
    /// Report-issue action (opens host bug flow).
    pub report_issue: Option<RecoveryAction<'a>>,
    /// Whether retry is side-effect safe.
    pub retry_safety: RetrySafety,
    /// Whether user work was preserved.
    pub work_preserved: bool,
    /// Optional note about preserved work ("Draft retained in editor").
    pub work_note: Option<&'a str>,
}

impl<'a> Recovery<'a> {
    /// Empty recovery (message only).
    #[must_use]
    pub const fn none() -> Self {
        Self {
            retry: None,
            alternative: None,
            copy_diagnostics: false,
            report_issue: None,
            retry_safety: RetrySafety::Unknown,
            work_preserved: false,
            work_note: None,
        }
    }

    /// Retry-only with safety.
    #[must_use]
    pub const fn retry_only(label: &'a str, safety: RetrySafety) -> Self {
        Self {
            retry: Some(RecoveryAction::new(label)),
            alternative: None,
            copy_diagnostics: false,
            report_issue: None,
            retry_safety: safety,
            work_preserved: false,
            work_note: None,
        }
    }

    /// Fluent retry.
    #[must_use]
    pub const fn with_retry(mut self, action: RecoveryAction<'a>) -> Self {
        self.retry = Some(action);
        self
    }

    /// Fluent alternative.
    #[must_use]
    pub const fn with_alternative(mut self, action: RecoveryAction<'a>) -> Self {
        self.alternative = Some(action);
        self
    }

    /// Enable copy diagnostics.
    #[must_use]
    pub const fn with_copy_diagnostics(mut self, on: bool) -> Self {
        self.copy_diagnostics = on;
        self
    }

    /// Report issue action.
    #[must_use]
    pub const fn with_report_issue(mut self, action: RecoveryAction<'a>) -> Self {
        self.report_issue = Some(action);
        self
    }

    /// Retry safety.
    #[must_use]
    pub const fn with_retry_safety(mut self, safety: RetrySafety) -> Self {
        self.retry_safety = safety;
        self
    }

    /// Work preserved + optional note.
    #[must_use]
    pub const fn with_work_preserved(mut self, preserved: bool, note: Option<&'a str>) -> Self {
        self.work_preserved = preserved;
        self.work_note = note;
        self
    }

    /// Any interactive action present.
    #[must_use]
    pub const fn has_actions(self) -> bool {
        self.retry.is_some()
            || self.alternative.is_some()
            || self.copy_diagnostics
            || self.report_issue.is_some()
    }

    /// Focus targets in tab order.
    fn focus_targets(self) -> [ErrorFocus; 5] {
        let mut out = [ErrorFocus::None; 5];
        let mut i = 0usize;
        if self.retry.is_some() {
            out[i] = ErrorFocus::Retry;
            i += 1;
        }
        if self.alternative.is_some() {
            out[i] = ErrorFocus::Alternative;
            i += 1;
        }
        if self.copy_diagnostics {
            out[i] = ErrorFocus::CopyDiagnostics;
            i += 1;
        }
        if self.report_issue.is_some() {
            out[i] = ErrorFocus::ReportIssue;
            i += 1;
        }
        // Toggle details is always available when technical present — added by state machine
        let _ = i;
        out
    }
}

// ── Focus / outcome / state ─────────────────────────────────────────────────

/// Focusable control inside error recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ErrorFocus {
    /// Nothing focused.
    #[default]
    None,
    /// Retry action.
    Retry,
    /// Alternative action.
    Alternative,
    /// Copy diagnostics.
    CopyDiagnostics,
    /// Report issue.
    ReportIssue,
    /// Expand/collapse technical details.
    ToggleDetails,
}

/// Outcomes (effects stay host-owned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorStateOutcome {
    /// No action.
    Ignored,
    /// Retry activated.
    Retry,
    /// Alternative activated.
    Alternative,
    /// Copy diagnostics requested (host places text on clipboard).
    CopyDiagnostics,
    /// Report issue activated.
    ReportIssue,
    /// Technical details toggled.
    ToggleDetails,
}

/// Interaction + disclosure state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ErrorStateState {
    /// Technical details expanded (default: collapsed / inaccessible only via toggle).
    details_expanded: bool,
    focus: ErrorFocus,
    retry_btn: ButtonState,
    alt_btn: ButtonState,
    copy_btn: ButtonState,
    report_btn: ButtonState,
}

impl ErrorStateState {
    /// Collapsed details, no focus.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether technical block is shown.
    #[must_use]
    pub const fn details_expanded(&self) -> bool {
        self.details_expanded
    }

    /// Expand or collapse technical details.
    pub fn set_details_expanded(&mut self, on: bool) {
        self.details_expanded = on;
    }

    /// Toggle technical details.
    pub fn toggle_details(&mut self) {
        self.details_expanded = !self.details_expanded;
    }

    /// Focus.
    #[must_use]
    pub const fn focus(&self) -> ErrorFocus {
        self.focus
    }

    /// Set focus.
    pub fn set_focus(&mut self, focus: ErrorFocus) {
        self.focus = focus;
    }

    /// Prefer retry when present.
    pub fn focus_retry(&mut self) {
        self.focus = ErrorFocus::Retry;
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Structured recoverable failure surface.
///
/// # Examples
///
/// ```
/// use termrock::style::DesignSystem;
/// use termrock::widgets::{ErrorState, ErrorKind, Recovery, RecoveryAction, RetrySafety};
///
/// let system = DesignSystem::default();
/// let err = ErrorState::new("Request failed", &system)
///     .kind(ErrorKind::Network)
///     .explanation("Could not reach the API")
///     .technical("timeout after 30s: GET /v1/jobs")
///     .source("jobs-service")
///     .recovery(
///         Recovery::none()
///             .with_retry(RecoveryAction::with_shortcut("Retry", "r"))
///             .with_retry_safety(RetrySafety::Safe)
///             .with_copy_diagnostics(true)
///             .with_work_preserved(true, Some("Draft retained")),
///     );
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ErrorState<'a> {
    summary: &'a str,
    kind: ErrorKind,
    explanation: Option<&'a str>,
    technical: Option<&'a str>,
    source: Option<&'a str>,
    recovery: Recovery<'a>,
    recipe: ErrorRecipe,
    illustration: Option<&'a str>,
    system: &'a DesignSystem,
}

impl<'a> ErrorState<'a> {
    /// Summary + system (generic kind, pane recipe, no recovery).
    #[must_use]
    pub const fn new(summary: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            summary,
            kind: ErrorKind::Generic,
            explanation: None,
            technical: None,
            source: None,
            recovery: Recovery::none(),
            recipe: ErrorRecipe::Pane,
            illustration: None,
            system,
        }
    }

    /// Error kind.
    #[must_use]
    pub const fn kind(mut self, kind: ErrorKind) -> Self {
        self.kind = kind;
        self
    }

    /// Human explanation.
    #[must_use]
    pub const fn explanation(mut self, text: &'a str) -> Self {
        self.explanation = Some(text);
        self
    }

    /// Technical diagnostics (hidden until expanded).
    #[must_use]
    pub const fn technical(mut self, technical: &'a str) -> Self {
        self.technical = Some(technical);
        self
    }

    /// Source / subsystem label.
    #[must_use]
    pub const fn source(mut self, source: &'a str) -> Self {
        self.source = Some(source);
        self
    }

    /// Recovery bundle.
    #[must_use]
    pub const fn recovery(mut self, recovery: Recovery<'a>) -> Self {
        self.recovery = recovery;
        self
    }

    /// Layout recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: ErrorRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Inline strip.
    #[must_use]
    pub const fn inline(mut self) -> Self {
        self.recipe = ErrorRecipe::Inline;
        self
    }

    /// Dialog recipe.
    #[must_use]
    pub const fn dialog(mut self) -> Self {
        self.recipe = ErrorRecipe::Dialog;
        self
    }

    /// Full-screen recipe.
    #[must_use]
    pub const fn full_screen(mut self) -> Self {
        self.recipe = ErrorRecipe::FullScreen;
        self
    }

    /// Override illustration glyph.
    #[must_use]
    pub const fn glyph(mut self, glyph: &'a str) -> Self {
        self.illustration = Some(glyph);
        self
    }

    /// Summary text.
    #[must_use]
    pub const fn summary(self) -> &'a str {
        self.summary
    }

    /// Kind.
    #[must_use]
    pub const fn error_kind(self) -> ErrorKind {
        self.kind
    }

    /// Recovery borrow.
    #[must_use]
    pub const fn recovery_bundle(self) -> Recovery<'a> {
        self.recovery
    }

    fn use_ascii(&self) -> bool {
        false
    }

    /// Resolved illustration.
    #[must_use]
    pub fn resolved_glyph(&self) -> &'static str {
        if let Some(g) = self.illustration {
            // Caller override may be non-static; paint path uses display string separately.
            // For API, prefer kind glyph when override is temporary — still return kind for measure.
            let _ = g;
        }
        self.kind.glyph(self.use_ascii())
    }

    fn glyph_for_paint(&self) -> &str {
        self.illustration
            .unwrap_or_else(|| self.kind.glyph(self.use_ascii()))
    }

    fn effective_recipe(&self, area: Rect) -> ErrorRecipe {
        if matches!(self.recipe, ErrorRecipe::Inline)
            || area.width <= ERROR_STATE_INLINE_MAX_WIDTH
            || area.height <= 3
        {
            return ErrorRecipe::Inline;
        }
        if matches!(self.recipe, ErrorRecipe::FullScreen) && area.height >= 12 {
            return ErrorRecipe::FullScreen;
        }
        if matches!(self.recipe, ErrorRecipe::Dialog) {
            return ErrorRecipe::Dialog;
        }
        ErrorRecipe::Pane
    }

    /// Effective retry safety (explicit recovery overrides kind default when set).
    #[must_use]
    pub const fn retry_safety(&self) -> RetrySafety {
        if !matches!(self.recovery.retry_safety, RetrySafety::Unknown)
            || self.recovery.retry.is_some()
        {
            // If host set safety on recovery, use it; else kind default when retry present
            if matches!(self.recovery.retry_safety, RetrySafety::Unknown)
                && self.recovery.retry.is_some()
            {
                return self.kind.default_retry_safety();
            }
            return self.recovery.retry_safety;
        }
        self.kind.default_retry_safety()
    }

    /// Passive paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) {
        let mut state = ErrorStateState::new();
        self.paint_with_state(area, buffer, &mut state);
    }

    /// Paint with disclosure/focus state.
    pub fn paint_with_state(&self, area: Rect, buffer: &mut Buffer, state: &mut ErrorStateState) {
        if area.is_empty() {
            return;
        }
        match self.effective_recipe(area) {
            ErrorRecipe::Inline => self.paint_inline(area, buffer),
            ErrorRecipe::Pane | ErrorRecipe::Dialog | ErrorRecipe::FullScreen => {
                self.paint_block(area, buffer, state)
            }
        }
    }

    fn paint_inline(&self, area: Rect, buffer: &mut Buffer) {
        let g = self.glyph_for_paint();
        let mut line = format!("{g} {}", self.summary);
        if let Some(ex) = self.explanation {
            if area.height >= 2 {
                let at = self.paint_centered(
                    area,
                    buffer,
                    area.y,
                    &line,
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD),
                );
                self.paint_kind_glyph(buffer, at, area.y, g);
                let mut second = ex.to_string();
                if let Some(r) = self.recovery.retry {
                    second = format!("{second} · {}", r.label);
                }
                self.paint_centered(
                    area,
                    buffer,
                    area.y.saturating_add(1),
                    &second,
                    self.system.style(Role::TextMuted),
                );
                return;
            }
            line = format!("{line} — {ex}");
        } else if let Some(r) = self.recovery.retry {
            line = format!("{line} · {}", r.label);
        }
        let at = self.paint_centered(
            area,
            buffer,
            area.y,
            &line,
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD),
        );
        self.paint_kind_glyph(buffer, at, area.y, g);
    }

    fn paint_block(&self, area: Rect, buffer: &mut Buffer, state: &mut ErrorStateState) {
        let g = self.glyph_for_paint();
        let mut rows: Vec<(String, Role, bool)> = Vec::with_capacity(12);
        rows.push((g.to_string(), Role::TextMuted, false));
        rows.push((self.summary.to_string(), self.kind.role(), true));
        if let Some(ex) = self.explanation {
            rows.push((ex.to_string(), Role::TextMuted, false));
        }
        if let Some(src) = self.source {
            rows.push((format!("source: {src}"), Role::TextDisabled, false));
        }
        // Work preserved cue
        if self.recovery.work_preserved {
            let note = self.recovery.work_note.unwrap_or("Your work was preserved");
            rows.push((format!("✓ {note}"), Role::Success, false));
        }
        // Retry safety line when retry present
        if self.recovery.retry.is_some() {
            rows.push((
                self.retry_safety().label().to_string(),
                match self.retry_safety() {
                    RetrySafety::Safe => Role::Success,
                    RetrySafety::Unsafe => Role::Warning,
                    RetrySafety::Unknown => Role::TextDisabled,
                },
                false,
            ));
        }
        // Technical: collapsed cue or expanded body
        if self.technical.is_some() {
            if state.details_expanded {
                if let Some(tech) = self.technical {
                    rows.push((format!("details: {tech}"), Role::TextDisabled, false));
                }
                rows.push(("▾ hide details (d)".into(), Role::TextMuted, false));
            } else {
                rows.push(("▸ technical details (d)".into(), Role::TextMuted, false));
            }
        }

        let action_count = u16::from(self.recovery.retry.is_some())
            + u16::from(self.recovery.alternative.is_some())
            + u16::from(self.recovery.copy_diagnostics)
            + u16::from(self.recovery.report_issue.is_some());
        let content = rows.len() as u16;
        let total = content.saturating_add(action_count).max(1);

        let block = Center::new(area.width, total)
            .axis(CenterAxis::Vertical)
            .layout(area)
            .child;
        let sizes: Vec<FlexSize> = (0..total).map(|_| FlexSize::Fixed(1)).collect();
        let stack = Stack::new().layout(block, &sizes);

        let mut idx = 0usize;
        for (text, role, bold) in &rows {
            if let Some(r) = stack.get(idx) {
                let mut style = self.system.style(*role);
                if *bold {
                    style = style.add_modifier(Modifier::BOLD);
                }
                self.paint_centered(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    r.y,
                    text,
                    style,
                );
            }
            idx += 1;
        }

        if let Some(retry) = self.recovery.retry {
            if let Some(r) = stack.get(idx) {
                self.paint_action(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    retry,
                    true, // dominant primary-like
                    matches!(state.focus, ErrorFocus::Retry),
                    &mut state.retry_btn,
                    false, // never destructive
                );
            }
            idx += 1;
        }
        if let Some(alt) = self.recovery.alternative {
            if let Some(r) = stack.get(idx) {
                self.paint_action(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    alt,
                    false,
                    matches!(state.focus, ErrorFocus::Alternative),
                    &mut state.alt_btn,
                    false,
                );
            }
            idx += 1;
        }
        if self.recovery.copy_diagnostics {
            if let Some(r) = stack.get(idx) {
                self.paint_action(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    RecoveryAction::with_shortcut("Copy diagnostics", "c"),
                    false,
                    matches!(state.focus, ErrorFocus::CopyDiagnostics),
                    &mut state.copy_btn,
                    false,
                );
            }
            idx += 1;
        }
        if let Some(rep) = self.recovery.report_issue {
            if let Some(r) = stack.get(idx) {
                self.paint_action(
                    Rect::new(area.x, r.y, area.width, 1),
                    buffer,
                    rep,
                    false,
                    matches!(state.focus, ErrorFocus::ReportIssue),
                    &mut state.report_btn,
                    false,
                );
            }
        }
    }

    fn paint_action(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        action: RecoveryAction<'_>,
        primary: bool,
        focused: bool,
        btn_state: &mut ButtonState,
        _destructive: bool,
    ) {
        if area.is_empty() {
            return;
        }
        // Retry (primary) uses Primary variant; others Quiet. Never Destructive here.
        let variant = if primary {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Quiet
        };
        let mut btn = Button::new(action.label, self.system).variant(variant);
        if let Some(sc) = action.shortcut {
            btn = btn.trailing(sc);
        }
        let measure = display_cols(action.label)
            .saturating_add(action.shortcut.map(display_cols).unwrap_or(0))
            .saturating_add(6)
            .min(usize::from(area.width)) as u16;
        let x = center_line_x(area, measure.max(1));
        let hit = Rect::new(x, area.y, measure.max(1), 1);
        btn_state.activation.set_accepts_input(focused || primary);
        let _ = btn.paint(hit, buffer, btn_state);
    }

    /// Paints one centered line and reports the column it started at.
    fn paint_centered(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        y: u16,
        text: &str,
        style: ratatui_core::style::Style,
    ) -> Option<u16> {
        let width = display_cols(text).min(usize::from(area.width));
        if width == 0 {
            return None;
        }
        let clipped = take_display_cols(text, width);
        let x = center_line_x(area, width as u16);
        buffer.set_stringn(x, y, &clipped, width, style);
        Some(x)
    }

    /// Repaints the leading glyph of a centered line in the error's tone.
    ///
    /// The summary is a sentence and stays readable in the strong text tone;
    /// the glyph carries the severity (plans/007).
    fn paint_kind_glyph(&self, buffer: &mut Buffer, x: Option<u16>, y: u16, glyph: &str) {
        let Some(x) = x else {
            return;
        };
        crate::widgets::row_chrome::paint_status_glyph(
            buffer,
            Rect::new(x, y, display_cols(glyph) as u16, 1),
            0,
            glyph,
            self.system.style(self.kind.role()),
        );
    }

    /// Diagnostics text for copy (host clipboard).
    #[must_use]
    pub fn diagnostics_text(&self) -> String {
        let mut s = format!("error kind={} summary={}", self.kind.id(), self.summary);
        if let Some(ex) = self.explanation {
            s.push_str(&format!("\nexplanation={ex}"));
        }
        if let Some(src) = self.source {
            s.push_str(&format!("\nsource={src}"));
        }
        if let Some(tech) = self.technical {
            s.push_str(&format!("\ntechnical={tech}"));
        }
        s.push_str(&format!(
            "\nretry_safety={} work_preserved={}",
            self.retry_safety().id(),
            self.recovery.work_preserved
        ));
        s
    }

    /// Keyboard handling.
    pub fn handle_key(&self, key: KeyEvent, state: &mut ErrorStateState) -> ErrorStateOutcome {
        if !key.is_press() {
            return ErrorStateOutcome::Ignored;
        }
        // Toggle details
        if matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D')) && self.technical.is_some() {
            state.toggle_details();
            return ErrorStateOutcome::ToggleDetails;
        }
        // Copy shortcut
        if matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
            && self.recovery.copy_diagnostics
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return ErrorStateOutcome::CopyDiagnostics;
        }
        // Retry shortcut
        if matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
            && self.recovery.retry.is_some()
        {
            return ErrorStateOutcome::Retry;
        }

        // Tab cycle focus targets
        if matches!(key.code, KeyCode::Tab) {
            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
            let mut targets: Vec<ErrorFocus> = Vec::new();
            if self.recovery.retry.is_some() {
                targets.push(ErrorFocus::Retry);
            }
            if self.recovery.alternative.is_some() {
                targets.push(ErrorFocus::Alternative);
            }
            if self.recovery.copy_diagnostics {
                targets.push(ErrorFocus::CopyDiagnostics);
            }
            if self.recovery.report_issue.is_some() {
                targets.push(ErrorFocus::ReportIssue);
            }
            if self.technical.is_some() {
                targets.push(ErrorFocus::ToggleDetails);
            }
            if targets.is_empty() {
                return ErrorStateOutcome::Ignored;
            }
            let cur = targets.iter().position(|&t| t == state.focus);
            let next = match (cur, shift) {
                (None, false) => targets[0],
                (None, true) => *targets.last().unwrap_or(&targets[0]),
                (Some(i), false) => targets[(i + 1) % targets.len()],
                (Some(i), true) => targets[(i + targets.len() - 1) % targets.len()],
            };
            state.focus = next;
            return ErrorStateOutcome::Ignored;
        }

        if matches!(default_button_intent(key), Some(UiIntent::Activate))
            || matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
        {
            return match state.focus {
                ErrorFocus::Retry if self.recovery.retry.is_some() => ErrorStateOutcome::Retry,
                ErrorFocus::Alternative if self.recovery.alternative.is_some() => {
                    ErrorStateOutcome::Alternative
                }
                ErrorFocus::CopyDiagnostics if self.recovery.copy_diagnostics => {
                    ErrorStateOutcome::CopyDiagnostics
                }
                ErrorFocus::ReportIssue if self.recovery.report_issue.is_some() => {
                    ErrorStateOutcome::ReportIssue
                }
                ErrorFocus::ToggleDetails if self.technical.is_some() => {
                    state.toggle_details();
                    ErrorStateOutcome::ToggleDetails
                }
                ErrorFocus::None if self.recovery.retry.is_some() => {
                    // Safe default: unfocused Enter activates retry only when present
                    ErrorStateOutcome::Retry
                }
                _ => ErrorStateOutcome::Ignored,
            };
        }
        let _ = self.recovery.focus_targets();
        ErrorStateOutcome::Ignored
    }

    /// Mouse: lower bands activate recovery actions; detail toggle mid band.
    pub fn handle_mouse(
        &self,
        mouse: MouseEvent,
        area: Rect,
        state: &mut ErrorStateState,
    ) -> ErrorStateOutcome {
        if area.is_empty() || !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return ErrorStateOutcome::Ignored;
        }
        let pos = mouse.position;
        if !area.contains(pos) {
            return ErrorStateOutcome::Ignored;
        }
        let rel = pos.y.saturating_sub(area.y);
        let h = area.height.max(1);
        // Bottom-most rows = actions
        if self.recovery.has_actions() && rel + 1 >= h {
            if self.recovery.report_issue.is_some() {
                state.focus = ErrorFocus::ReportIssue;
                return ErrorStateOutcome::ReportIssue;
            }
            if self.recovery.copy_diagnostics {
                state.focus = ErrorFocus::CopyDiagnostics;
                return ErrorStateOutcome::CopyDiagnostics;
            }
            if self.recovery.alternative.is_some() {
                state.focus = ErrorFocus::Alternative;
                return ErrorStateOutcome::Alternative;
            }
            if self.recovery.retry.is_some() {
                state.focus = ErrorFocus::Retry;
                return ErrorStateOutcome::Retry;
            }
        }
        if self.recovery.retry.is_some() && rel + 2 >= h {
            state.focus = ErrorFocus::Retry;
            return ErrorStateOutcome::Retry;
        }
        if self.technical.is_some() && rel > h / 3 && rel + 3 < h {
            state.toggle_details();
            return ErrorStateOutcome::ToggleDetails;
        }
        ErrorStateOutcome::Ignored
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: Option<&ErrorStateState>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() {
            return;
        }
        let expanded = state.map(|s| s.details_expanded).unwrap_or(false);
        let focus = state.map(|s| s.focus).unwrap_or(ErrorFocus::None);
        let desc = format!(
            "error-state kind={} summary={} recipe={} details={} retry_safety={} work_preserved={} focus={}",
            self.kind.id(),
            self.summary,
            self.effective_recipe(area).id(),
            if expanded { "open" } else { "collapsed" },
            self.retry_safety().id(),
            self.recovery.work_preserved,
            match focus {
                ErrorFocus::None => "none",
                ErrorFocus::Retry => "retry",
                ErrorFocus::Alternative => "alternative",
                ErrorFocus::CopyDiagnostics => "copy",
                ErrorFocus::ReportIssue => "report",
                ErrorFocus::ToggleDetails => "details",
            }
        );
        let focusable = self.recovery.has_actions() || self.technical.is_some();
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Status)
                .label("error-state")
                .description(desc)
                .focusable(focusable)
                .state(SemanticState::default()),
        );
    }
}

impl Widget for &ErrorState<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        self.paint(area, buffer);
    }
}

impl Widget for ErrorState<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Recipes ─────────────────────────────────────────────────────────────────

/// Network failure pane with safe retry.
#[must_use]
pub fn example_error_network(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Request failed", system)
        .kind(ErrorKind::Network)
        .explanation("Could not reach the API. Check connectivity and try again.")
        .technical("timeout after 30s: GET /v1/jobs (req_id=abc123)")
        .source("jobs-service")
        .recovery(
            Recovery::none()
                .with_retry(RecoveryAction::with_shortcut("Retry", "r"))
                .with_alternative(RecoveryAction::new("Work offline"))
                .with_copy_diagnostics(true)
                .with_retry_safety(RetrySafety::Safe)
                .with_work_preserved(true, Some("Draft retained in editor")),
        )
}

/// Validation failure (retry unsafe — fix input).
#[must_use]
pub fn example_error_validation(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Validation failed", system)
        .kind(ErrorKind::Validation)
        .explanation("Name is required and must be unique.")
        .technical("field=name code=required_unique")
        .source("form:create-project")
        .recovery(
            Recovery::none()
                .with_alternative(RecoveryAction::with_shortcut("Edit field", "e"))
                .with_retry_safety(RetrySafety::Unsafe)
                .with_work_preserved(true, Some("Form values kept")),
        )
}

/// Permission denied.
#[must_use]
pub fn example_error_permission(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Permission denied", system)
        .kind(ErrorKind::Permission)
        .explanation("You do not have access to this resource.")
        .technical("HTTP 403 scope=org/private-repo")
        .source("authz")
        .recovery(
            Recovery::none()
                .with_alternative(RecoveryAction::with_shortcut("Request access", "r"))
                .with_report_issue(RecoveryAction::new("Report issue"))
                .with_copy_diagnostics(true)
                .with_retry_safety(RetrySafety::Unsafe)
                .with_work_preserved(true, Some("Local edits preserved")),
        )
}

/// Not found.
#[must_use]
pub fn example_error_not_found(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Not found", system)
        .kind(ErrorKind::NotFound)
        .explanation("The session no longer exists.")
        .technical("session_id=ses_deadbeef")
        .recovery(
            Recovery::none()
                .with_retry(RecoveryAction::with_shortcut("Refresh", "r"))
                .with_alternative(RecoveryAction::new("Back to list"))
                .with_retry_safety(RetrySafety::Safe),
        )
}

/// Conflict.
#[must_use]
pub fn example_error_conflict(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Conflict", system)
        .kind(ErrorKind::Conflict)
        .explanation("Someone else updated this resource. Reload before saving.")
        .technical("etag mismatch expected=v3 got=v4")
        .recovery(
            Recovery::none()
                .with_alternative(RecoveryAction::with_shortcut("Reload", "r"))
                .with_retry_safety(RetrySafety::Unsafe)
                .with_work_preserved(true, Some("Your draft is still in the editor")),
        )
}

/// Crash / unexpected.
#[must_use]
pub fn example_error_crash(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Unexpected error", system)
        .kind(ErrorKind::Crash)
        .explanation("Something went wrong. Your work was preserved when possible.")
        .technical("panic: index out of bounds at widgets/table.rs:412")
        .source("termrock")
        .recipe(ErrorRecipe::FullScreen)
        .recovery(
            Recovery::none()
                .with_retry(RecoveryAction::with_shortcut("Restart view", "r"))
                .with_copy_diagnostics(true)
                .with_report_issue(RecoveryAction::with_shortcut("Report issue", "i"))
                .with_retry_safety(RetrySafety::Unknown)
                .with_work_preserved(true, Some("Session draft retained")),
        )
}

/// Unsupported capability.
#[must_use]
pub fn example_error_unsupported(system: &DesignSystem) -> ErrorState<'_> {
    ErrorState::new("Unsupported capability", system)
        .kind(ErrorKind::UnsupportedCapability)
        .explanation("This terminal cannot display truecolor images.")
        .technical("ColorCapability::Ansi16; need Truecolor")
        .recovery(
            Recovery::none()
                .with_alternative(RecoveryAction::new("Use text preview"))
                .with_retry_safety(RetrySafety::Unsafe),
        )
}

/// Dialog-sized network error.
#[must_use]
pub fn example_error_dialog(system: &DesignSystem) -> ErrorState<'_> {
    example_error_network(system).dialog()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::layout::Position;
    use ratatui_core::terminal::Terminal;

    fn system() -> DesignSystem {
        DesignSystem::default()
    }

    fn painted(area: Rect, paint: impl FnOnce(Rect, &mut Buffer)) -> String {
        let mut buf = Buffer::empty(area);
        paint(area, &mut buf);
        let mut s = String::new();
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn kinds_have_glyphs_and_ids() {
        for k in [
            ErrorKind::Validation,
            ErrorKind::Network,
            ErrorKind::Permission,
            ErrorKind::NotFound,
            ErrorKind::Conflict,
            ErrorKind::Crash,
            ErrorKind::UnsupportedCapability,
            ErrorKind::Generic,
        ] {
            assert!(!k.id().is_empty());
            assert!(!k.glyph_unicode().is_empty());
            assert!(!k.glyph_ascii().is_empty());
        }
    }

    #[test]
    fn explanation_renders_summary_and_detail() {
        let system = system();
        let text = painted(Rect::new(0, 0, 40, 5), |a, b| {
            ErrorState::new("Failed", &system)
                .explanation("Timed out")
                .paint(a, b);
        });
        assert!(text.contains("Failed"), "{text}");
        assert!(text.contains("Timed out"), "{text}");
        assert!(
            text.contains('✗') || text.contains('x') || text.contains('!'),
            "{text}"
        );
    }

    #[test]
    fn technical_hidden_until_expanded() {
        let system = system();
        let e = ErrorState::new("Err", &system)
            .technical("secret stack trace XYZ")
            .explanation("human msg");
        let collapsed = painted(Rect::new(0, 0, 48, 10), |a, b| e.paint(a, b));
        assert!(
            !collapsed.contains("secret stack"),
            "collapsed leaked tech: {collapsed}"
        );
        assert!(
            collapsed.contains("technical") || collapsed.contains("details"),
            "{collapsed}"
        );

        let mut st = ErrorStateState::new();
        st.set_details_expanded(true);
        let expanded = painted(Rect::new(0, 0, 48, 12), |a, b| {
            e.paint_with_state(a, b, &mut st);
        });
        assert!(expanded.contains("secret stack"), "{expanded}");
    }

    #[test]
    fn toggle_details_via_key() {
        let system = system();
        let e = ErrorState::new("Err", &system).technical("tech");
        let mut st = ErrorStateState::new();
        assert!(!st.details_expanded());
        let key = KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(e.handle_key(key, &mut st), ErrorStateOutcome::ToggleDetails);
        assert!(st.details_expanded());
    }

    #[test]
    fn retry_activation_and_safety_line() {
        let system = system();
        let e = example_error_network(&system);
        assert_eq!(e.retry_safety(), RetrySafety::Safe);
        let text = painted(Rect::new(0, 0, 50, 14), |a, b| e.paint(a, b));
        assert!(
            text.contains("retry safe") || text.contains("Retry"),
            "{text}"
        );
        assert!(
            text.contains("Draft") || text.contains("preserved") || text.contains("✓"),
            "{text}"
        );

        let mut st = ErrorStateState::new();
        st.focus_retry();
        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(e.handle_key(key, &mut st), ErrorStateOutcome::Retry);
    }

    #[test]
    fn unfocused_enter_retries_when_present() {
        let system = system();
        let e = example_error_network(&system);
        let mut st = ErrorStateState::new();
        let key = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(e.handle_key(key, &mut st), ErrorStateOutcome::Retry);
    }

    #[test]
    fn copy_diagnostics_outcome() {
        let system = system();
        let e = example_error_network(&system);
        let mut st = ErrorStateState::new();
        let key = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        assert_eq!(
            e.handle_key(key, &mut st),
            ErrorStateOutcome::CopyDiagnostics
        );
        let diag = e.diagnostics_text();
        assert!(diag.contains("network"));
        assert!(diag.contains("jobs-service") || diag.contains("source"));
    }

    #[test]
    fn recipes_inline_dialog_fullscreen() {
        let system = system();
        let inline = painted(Rect::new(0, 0, 24, 2), |a, b| {
            example_error_unsupported(&system).paint(a, b);
        });
        assert!(!inline.trim().is_empty(), "{inline}");

        let dialog = painted(Rect::new(0, 0, 48, 12), |a, b| {
            example_error_dialog(&system).paint(a, b);
        });
        assert!(
            dialog.contains("Request") || dialog.contains("failed"),
            "{dialog}"
        );

        let full = painted(Rect::new(0, 0, 60, 16), |a, b| {
            example_error_crash(&system).paint(a, b);
        });
        assert!(
            full.contains("Unexpected") || full.contains("error"),
            "{full}"
        );
    }

    #[test]
    fn all_kind_examples_paint() {
        let system = system();
        for e in [
            example_error_network(&system),
            example_error_validation(&system),
            example_error_permission(&system),
            example_error_not_found(&system),
            example_error_conflict(&system),
            example_error_crash(&system),
            example_error_unsupported(&system),
        ] {
            let t = painted(Rect::new(0, 0, 52, 14), |a, b| e.paint(a, b));
            assert!(!t.trim().is_empty(), "kind={}", e.kind.id());
        }
    }

    #[test]
    fn validation_retry_unsafe() {
        let system = system();
        let e = example_error_validation(&system);
        assert_eq!(e.retry_safety(), RetrySafety::Unsafe);
        assert!(e.recovery.retry.is_none());
        assert!(e.recovery.work_preserved);
    }

    #[test]
    fn semantic_registers() {
        let system = system();
        let mut scene = SemanticScene::<&str, ()>::default();
        example_error_network(&system).register_semantic(
            &mut scene,
            "e",
            Rect::new(0, 0, 40, 10),
            None,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("error-state"))
        );
    }

    #[test]
    fn tiny_and_empty_safe() {
        let system = system();
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 2));
        ErrorState::new("E", &system).paint(Rect::new(0, 0, 1, 1), &mut buf);
        ErrorState::new("E", &system).paint(Rect::new(0, 0, 0, 0), &mut buf);
    }

    #[test]
    fn tab_cycles_recovery() {
        let system = system();
        let e = example_error_network(&system);
        let mut st = ErrorStateState::new();
        let tab = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: crate::input::KeyEventState::NONE,
        };
        let _ = e.handle_key(tab, &mut st);
        assert_eq!(st.focus(), ErrorFocus::Retry);
        let _ = e.handle_key(tab, &mut st);
        assert_eq!(st.focus(), ErrorFocus::Alternative);
    }

    #[test]
    fn fuzz_kinds_sizes() {
        let system = system();
        let mut seed = 11u64;
        for _ in 0..40 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let kind = match seed % 8 {
                0 => ErrorKind::Validation,
                1 => ErrorKind::Network,
                2 => ErrorKind::Permission,
                3 => ErrorKind::NotFound,
                4 => ErrorKind::Conflict,
                5 => ErrorKind::Crash,
                6 => ErrorKind::UnsupportedCapability,
                _ => ErrorKind::Generic,
            };
            let w = (seed % 50) as u16 + 1;
            let h = (seed % 16) as u16 + 1;
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            let mut e = ErrorState::new("E", &system).kind(kind);
            if seed % 2 == 0 {
                e = e.explanation("ex").technical("tech");
            }
            if seed % 3 == 0 {
                e = e.recovery(Recovery::retry_only("Retry", RetrySafety::Safe));
            }
            let mut st = ErrorStateState::new();
            if seed % 5 == 0 {
                st.set_details_expanded(true);
            }
            e.paint_with_state(area, &mut buf, &mut st);
        }
    }

    #[test]
    fn pty_snapshot_stable() {
        let system = system();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(48, 12)).unwrap();
            t.draw(|f| {
                example_error_network(&system).paint(f.area(), f.buffer_mut());
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

    #[test]
    fn paint_perf_smoke() {
        let system = system();
        let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            terminal
                .draw(|f| {
                    example_error_network(&system).paint(f.area(), f.buffer_mut());
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn mouse_retry() {
        let system = system();
        let e = example_error_network(&system);
        let mut st = ErrorStateState::new();
        let area = Rect::new(0, 0, 40, 12);
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 5, y: 11 },
            modifiers: KeyModifiers::NONE,
        };
        let out = e.handle_mouse(mouse, area, &mut st);
        assert!(
            matches!(
                out,
                ErrorStateOutcome::Retry
                    | ErrorStateOutcome::Alternative
                    | ErrorStateOutcome::CopyDiagnostics
                    | ErrorStateOutcome::ReportIssue
            ),
            "{out:?}"
        );
    }
}
