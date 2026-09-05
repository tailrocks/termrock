// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SetupWizard** (onboarding) — premium first-run flow composed from **public**
//! TermRock widgets only (source-owned registry block).
//!
//! **Mission.** Welcome → capability check → account/connection → choices →
//! validation → permissions → theme preview → summary → recovery, on top of
//! [`FormWizard`] + [`Stepper`] chrome. Keyboard-only completion, resume via
//! [`WizardProgress`], safe two-step cancel, inline and fullscreen modes.
//! Domain values, auth I/O, and restart stay **host-owned**.
//!
//! Research: CLI installers, Huh, cloud auth flows, native onboarding
//! (experience references — not marketing splash screens).
//!
//! Teaches: how to compose a first-run onboarding flow: steps, capability
//! findings, per-step validation and an explicit continue.
//!
//! Composes: [`crate::widgets::BUILTIN_THEME_PRESETS`],
//! [`crate::widgets::EmptyKind`], [`crate::widgets::EmptyState`],
//! [`crate::widgets::Field`], [`crate::widgets::Fieldset`],
//! [`crate::widgets::Form`], [`crate::widgets::FormOutcome`],
//! [`crate::widgets::FormState`], and 18 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{
        BUILTIN_THEME_PRESETS, Button, ButtonState, ButtonVariant, ConfirmPrompt, EmptyKind,
        EmptyState, Field, Fieldset, Form, FormOutcome, FormState, FormWizard, FormWizardOutcome,
        FormWizardState, KeyValueList, KeyValueListState, KeybindingRecorderState, KvEntry,
        KvStatus, PermissionPrompt, PermissionPromptState, StepChangeReason, ThemePicker,
        ThemePickerOutcome, ThemePickerState, ThemePreset, WizardGate, WizardPhase, WizardProgress,
        WizardStep,
    },
};

// ── Modes & step kinds ──────────────────────────────────────────────────────

/// Presentation shell for first-run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SetupWizardMode {
    /// Full-area first-run (app root).
    #[default]
    Fullscreen,
    /// Embedded in an existing shell pane (tighter chrome).
    Inline,
}

impl SetupWizardMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fullscreen => "fullscreen",
            Self::Inline => "inline",
        }
    }
}

/// Semantic kind for an onboarding step (drives body paint + defaults).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SetupStepKind {
    /// Compact welcome (not marketing splash).
    Welcome,
    /// Terminal capability / doctor projection.
    Capability,
    /// Account identity form.
    Account,
    /// Connection / endpoint form.
    Connection,
    /// Product choices (plan, region, …).
    Choices,
    /// Validation / async gate step.
    Validation,
    /// Permission / trust intro (host may open PermissionPrompt).
    Permission,
    /// Theme picker preview.
    Theme,
    /// Review / summary before finish.
    Summary,
    /// Failure recovery surface (also WizardPhase::Failed).
    Recovery,
    /// Host paints arbitrary body.
    Custom,
}

impl SetupStepKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::Capability => "capability",
            Self::Account => "account",
            Self::Connection => "connection",
            Self::Choices => "choices",
            Self::Validation => "validation",
            Self::Permission => "permission",
            Self::Theme => "theme",
            Self::Summary => "summary",
            Self::Recovery => "recovery",
            Self::Custom => "custom",
        }
    }

    /// Default wizard gate when entering the step (host may override).
    #[must_use]
    pub const fn default_gate(self) -> WizardGate {
        match self {
            Self::Welcome | Self::Capability | Self::Summary | Self::Theme | Self::Permission => {
                WizardGate::Valid
            }
            Self::Account | Self::Connection | Self::Choices | Self::Validation | Self::Custom => {
                WizardGate::Invalid
            }
            Self::Recovery => WizardGate::Valid,
        }
    }
}

/// One step definition for the setup flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetupStep {
    /// Wizard step chrome.
    pub step: WizardStep,
    /// Body kind.
    pub kind: SetupStepKind,
}

impl SetupStep {
    /// Titled step.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, kind: SetupStepKind) -> Self {
        Self {
            step: WizardStep::new(id, title),
            kind,
        }
    }

    /// Description.
    #[must_use]
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.step = self.step.description(d);
        self
    }

    /// Optional (skippable when wizard allows).
    #[must_use]
    pub fn optional(mut self, on: bool) -> Self {
        self.step = self.step.optional(on);
        self
    }
}

// ── Capability line (host-projected) ────────────────────────────────────────

/// One capability doctor row (host-projected; no detection in paint).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityLine<'a> {
    /// Label (e.g. "truecolor").
    pub label: &'a str,
    /// Status glyph text (e.g. "ok", "missing").
    pub status: &'a str,
    /// Whether this row is a problem.
    pub problem: bool,
}

impl<'a> CapabilityLine<'a> {
    /// Create.
    #[must_use]
    pub const fn new(label: &'a str, status: &'a str) -> Self {
        Self {
            label,
            status,
            problem: false,
        }
    }

    /// Mark as problem.
    #[must_use]
    pub const fn problem(mut self, on: bool) -> Self {
        self.problem = on;
        self
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Setup wizard outcomes (requests only — no auth / disk / process).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SetupWizardOutcome {
    /// Nothing handled.
    Ignored,
    /// Underlying FormWizard outcome.
    Wizard(FormWizardOutcome),
    /// Form body interaction.
    Form(FormOutcome<&'static str>),
    /// Theme picker.
    Theme(ThemePickerOutcome),
    /// Permission surface.
    Permission,
    /// First Esc — cancel confirmation required (safe cancel).
    CancelConfirmOpen,
    /// User backed out of cancel confirm.
    CancelConfirmDismissed,
    /// User confirmed cancel (host closes flow).
    CancelConfirmed,
    /// Finish / submit (alias of wizard SubmitRequested).
    Finished,
    /// Resume applied.
    Resumed {
        /// Step index after restore.
        step: usize,
    },
    /// Review step switched between changed-only and every value.
    SummaryDetailToggled {
        /// Showing every value after the toggle.
        all: bool,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Consumer-owned setup / onboarding interaction state.
///
/// **Host owns:** form values, auth, capability detection, permission grants,
/// theme apply, progress persistence bytes.
#[derive(Debug)]
pub struct SetupWizardState {
    /// Embedded FormWizard (stepper + nav + gates).
    pub wizard: FormWizardState,
    /// Parallel kinds (same length as wizard steps).
    kinds: Vec<SetupStepKind>,
    /// Fullscreen vs inline.
    pub mode: SetupWizardMode,
    /// Form body state when step is form-like.
    pub form: FormState<&'static str>,
    /// Focused field id.
    pub focused_field: Option<&'static str>,
    /// Theme picker.
    pub theme: ThemePickerState,
    /// Optional keybinding recorder for advanced setup.
    pub keybinding: KeybindingRecorderState,
    /// Permission prompt (host may enqueue).
    pub permission: PermissionPromptState,
    /// Safe-cancel confirmation open.
    pub cancel_confirm: bool,
    /// Colorless preference (reserved for child chrome).
    pub colorless: bool,
    /// Title override.
    pub title: String,
    /// Whether the review step shows every value or only what changed.
    show_all_summary: bool,
}

impl Default for SetupWizardState {
    fn default() -> Self {
        Self::from_steps(example_setup_steps())
    }
}

impl SetupWizardState {
    /// Build from setup step definitions.
    #[must_use]
    pub fn from_steps(steps: impl IntoIterator<Item = SetupStep>) -> Self {
        let collected: Vec<SetupStep> = steps.into_iter().collect();
        let kinds: Vec<SetupStepKind> = collected.iter().map(|s| s.kind).collect();
        let wizard_steps: Vec<WizardStep> = collected.into_iter().map(|s| s.step).collect();
        let mut wizard = FormWizardState::with_steps(wizard_steps).with_review(true);
        wizard.set_focused(true);
        // Apply default gates for first step
        if let Some(kind) = kinds.first() {
            wizard.set_gate(kind.default_gate());
        }
        Self {
            wizard,
            kinds,
            mode: SetupWizardMode::Fullscreen,
            form: FormState::new(),
            focused_field: None,
            theme: ThemePickerState::default(),
            keybinding: KeybindingRecorderState::new("setup.action", "Setup action"),
            permission: PermissionPromptState::new(),
            cancel_confirm: false,
            colorless: false,
            title: "Setup".into(),
            show_all_summary: false,
        }
    }

    /// Quick start with N custom steps (all Custom).
    #[must_use]
    pub fn new(step_count: usize) -> Self {
        let steps: Vec<SetupStep> = (0..step_count.max(1))
            .map(|i| {
                SetupStep::new(
                    format!("step-{i}"),
                    format!("Step {}", i + 1),
                    SetupStepKind::Custom,
                )
            })
            .collect();
        Self::from_steps(steps)
    }

    /// Mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: SetupWizardMode) -> Self {
        self.mode = mode;
        self
    }

    /// Title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Step kinds.
    #[must_use]
    pub fn kinds(&self) -> &[SetupStepKind] {
        &self.kinds
    }

    /// Kind of current step (or Recovery if failed phase).
    #[must_use]
    pub fn current_kind(&self) -> SetupStepKind {
        if matches!(self.wizard.phase(), WizardPhase::Failed) {
            return SetupStepKind::Recovery;
        }
        if matches!(self.wizard.phase(), WizardPhase::Review) {
            return SetupStepKind::Summary;
        }
        self.kinds
            .get(self.wizard.step())
            .copied()
            .unwrap_or(SetupStepKind::Custom)
    }

    /// Resume from saved progress (chrome only).
    pub fn resume(&mut self, progress: &WizardProgress) -> SetupWizardOutcome {
        let out = self.wizard.restore_progress(progress);
        self.cancel_confirm = false;
        let step = self.wizard.step();
        if let Some(kind) = self.kinds.get(step) {
            self.wizard.set_gate(kind.default_gate());
        }
        let _ = out;
        SetupWizardOutcome::Resumed { step }
    }

    /// Progress snapshot for host persistence.
    #[must_use]
    pub fn progress(&self) -> WizardProgress {
        self.wizard.progress()
    }

    /// Project gate from host validity.
    pub fn set_step_valid(&mut self, valid: bool) {
        self.wizard.set_step_valid(valid);
    }

    /// Set gate.
    pub fn set_gate(&mut self, gate: WizardGate) {
        self.wizard.set_gate(gate);
    }

    /// Safe cancel: first call opens confirm; second confirms.
    pub fn request_cancel(&mut self) -> SetupWizardOutcome {
        if self.cancel_confirm {
            self.cancel_confirm = false;
            let _ = self.wizard.cancel();
            SetupWizardOutcome::CancelConfirmed
        } else {
            self.cancel_confirm = true;
            SetupWizardOutcome::CancelConfirmOpen
        }
    }

    /// Dismiss cancel confirm without leaving.
    pub fn dismiss_cancel_confirm(&mut self) -> SetupWizardOutcome {
        if self.cancel_confirm {
            self.cancel_confirm = false;
            SetupWizardOutcome::CancelConfirmDismissed
        } else {
            SetupWizardOutcome::Ignored
        }
    }

    /// Route keys: cancel confirm → body-specific → FormWizard nav.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        fieldsets: &[Fieldset<'_, &'static str>],
        theme_presets: &[ThemePreset],
    ) -> SetupWizardOutcome {
        if key.is_release() {
            return SetupWizardOutcome::Ignored;
        }

        // Safe cancel confirmation layer
        if self.cancel_confirm {
            match key.code {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    return self.dismiss_cancel_confirm();
                }
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    return self.request_cancel();
                }
                _ => return SetupWizardOutcome::Ignored,
            }
        }

        // Esc → safe cancel (never one-shot leave)
        if matches!(key.code, KeyCode::Esc) {
            return self.request_cancel();
        }

        // Review step: `a` opens the values the diet held back.
        if matches!(self.current_kind(), SetupStepKind::Summary)
            && key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Char('a' | 'A'))
        {
            self.show_all_summary = !self.show_all_summary;
            return SetupWizardOutcome::SummaryDetailToggled {
                all: self.show_all_summary,
            };
        }

        // Ctrl+S progress save (wizard)
        if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let out = self.wizard.save_progress();
            return SetupWizardOutcome::Wizard(out);
        }

        // Body-owned keys when not on pure nav chords that wizard needs
        let kind = self.current_kind();
        let wizard_nav = matches!(
            key.code,
            KeyCode::Enter
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Char('n')
                | KeyCode::Char('N')
                | KeyCode::Char('p')
                | KeyCode::Char('P')
                | KeyCode::Char('s')
                | KeyCode::Char('S')
                | KeyCode::BackTab
                | KeyCode::Tab
        ) && !matches!(kind, SetupStepKind::Theme)
            || (matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
                && matches!(self.wizard.phase(), WizardPhase::Failed));

        // Theme body: arrow keys for picker; Enter can advance if Valid
        if matches!(kind, SetupStepKind::Theme)
            && !key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(
                key.code,
                KeyCode::Up | KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('k')
            )
        {
            let out = self.theme.handle_key(key, theme_presets.len());
            if !matches!(out, ThemePickerOutcome::Ignored) {
                return SetupWizardOutcome::Theme(out);
            }
        }

        // Form-like bodies: route non-nav keys to form
        let formish = matches!(
            kind,
            SetupStepKind::Account
                | SetupStepKind::Connection
                | SetupStepKind::Choices
                | SetupStepKind::Validation
                | SetupStepKind::Custom
        );
        if formish && !wizard_nav && !fieldsets.is_empty() {
            let out = self
                .form
                .handle_key(fieldsets, key, self.focused_field.as_ref());
            if !matches!(out, FormOutcome::Ignored) {
                if let FormOutcome::Activated(id) = &out {
                    self.focused_field = Some(*id);
                }
                return SetupWizardOutcome::Form(out);
            }
        }

        // Permission body: leave Esc to cancel; other keys to permission if non-empty
        if matches!(kind, SetupStepKind::Permission) && !self.permission.is_empty() {
            let out = self.permission.handle_key(key);
            if !matches!(out, crate::widgets::PermissionOutcome::Ignored) {
                return SetupWizardOutcome::Permission;
            }
        }

        // Wizard chrome / advance
        let out = self.wizard.handle_key(key);
        match &out {
            FormWizardOutcome::Cancelled => {
                // FormWizard Esc — convert to safe cancel
                self.cancel_confirm = true;
                SetupWizardOutcome::CancelConfirmOpen
            }
            FormWizardOutcome::SubmitRequested => SetupWizardOutcome::Finished,
            FormWizardOutcome::StepChanged { to, reason, .. } => {
                if let Some(kind) = self.kinds.get(*to) {
                    // Preserve resume gates; for Next set kind default if still default
                    let _ = reason;
                    if matches!(
                        reason,
                        StepChangeReason::Next | StepChangeReason::Back | StepChangeReason::Skip
                    ) {
                        self.wizard.set_gate(kind.default_gate());
                    }
                }
                SetupWizardOutcome::Wizard(out)
            }
            FormWizardOutcome::Ignored => SetupWizardOutcome::Ignored,
            _ => SetupWizardOutcome::Wizard(out),
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Geometry for setup wizard frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupWizardSlots {
    /// Outer frame (fullscreen = full area; inline may be inset).
    pub frame: Rect,
    /// Body content (inside FormWizard body after paint).
    pub body: Rect,
    /// Cancel-confirm strip (bottom overlay when open).
    pub cancel_confirm: Option<Rect>,
}

/// Resolve frame rect for mode (tight margins; no marketing splash).
#[must_use]
pub fn layout_setup_wizard(area: Rect, mode: SetupWizardMode) -> SetupWizardSlots {
    if area.is_empty() {
        return SetupWizardSlots {
            frame: area,
            body: area,
            cancel_confirm: None,
        };
    }
    let frame = match mode {
        SetupWizardMode::Fullscreen => {
            // Small safe margin so chrome isn't edge-glued on large terms
            let pad_x = if area.width > 80 { 2 } else { 0 };
            let pad_y = if area.height > 24 { 1 } else { 0 };
            Rect {
                x: area.x.saturating_add(pad_x),
                y: area.y.saturating_add(pad_y),
                width: area.width.saturating_sub(pad_x.saturating_mul(2)),
                height: area.height.saturating_sub(pad_y.saturating_mul(2)),
            }
        }
        SetupWizardMode::Inline => area,
    };
    SetupWizardSlots {
        frame,
        body: frame, // refined after FormWizard paint
        cancel_confirm: None,
    }
}

/// One line of the review step.
///
/// `changed` is the host's answer to "did the operator touch this?" — the
/// pattern cannot know, and a review that repeats the twenty defaults it was
/// handed buries the two values that were actually chosen. Everything the
/// operator did not change is one keypress away, never deleted (plans/017 §B2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SetupSummaryLine<'a> {
    /// What the value is called.
    pub label: &'a str,
    /// The value, formatted by the host.
    pub value: &'a str,
    /// Whether the operator changed it during this run.
    pub changed: bool,
}

impl<'a> SetupSummaryLine<'a> {
    /// A value the operator chose.
    #[must_use]
    pub const fn edited(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            changed: true,
        }
    }

    /// A value left at what the host proposed.
    #[must_use]
    pub const fn untouched(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            changed: false,
        }
    }
}

// ── Surfaces & paint ────────────────────────────────────────────────────────

/// Borrowed surfaces for one setup paint.
pub struct SetupWizardSurfaces<'a> {
    /// Design system (chrome).
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut SetupWizardState,
    /// Form fieldsets for form-like steps (host selects by step).
    pub fieldsets: &'a [Fieldset<'a, &'static str>],
    /// Capability doctor lines (Capability step).
    pub capabilities: &'a [CapabilityLine<'a>],
    /// Review lines, each stating whether the operator changed it.
    pub summary_lines: &'a [SetupSummaryLine<'a>],
    /// Welcome title / body.
    pub welcome_title: &'a str,
    /// Welcome explanation (one short line — not marketing paragraphs).
    pub welcome_detail: &'a str,
    /// Theme presets.
    pub theme_presets: &'a [ThemePreset],
    /// Live theme paint system for preview.
    pub theme_paint: Option<&'a DesignSystem>,
    /// Permission painter (optional).
    pub permission: Option<&'a PermissionPrompt<'a>>,
}

/// Paint setup wizard chrome + body content from public widgets.
pub fn render_setup_wizard(buffer: &mut Buffer, area: Rect, surfaces: SetupWizardSurfaces<'_>) {
    let SetupWizardSurfaces {
        system,
        state,
        fieldsets,
        capabilities,
        summary_lines,
        welcome_title,
        welcome_detail,
        theme_presets,
        theme_paint,
        permission,
    } = surfaces;

    let slots = layout_setup_wizard(area, state.mode);
    let frame = slots.frame;
    if frame.is_empty() {
        return;
    }

    // FormWizard chrome (stepper + nav)
    let title = state.title.as_str();
    FormWizard::new(system)
        .title(title)
        .show_stepper(!matches!(state.mode, SetupWizardMode::Inline) || frame.width >= 40)
        .paint(frame, buffer, &mut state.wizard);

    let body = state.wizard.body_area();
    if body.is_empty() {
        paint_cancel_confirm(buffer, frame, system, state);
        return;
    }

    // Failed phase forces recovery body
    let kind = state.current_kind();

    match kind {
        SetupStepKind::Welcome => {
            EmptyState::new(welcome_title, system)
                .kind(EmptyKind::FirstUse)
                .explanation(welcome_detail)
                .shortcut("Enter continue · Esc cancel")
                .paint(body, buffer);
        }
        SetupStepKind::Capability => {
            paint_capability_list(buffer, body, system, capabilities, false);
        }
        SetupStepKind::Account
        | SetupStepKind::Connection
        | SetupStepKind::Choices
        | SetupStepKind::Validation
        | SetupStepKind::Custom => {
            if fieldsets.is_empty() {
                paint_body_hint(
                    buffer,
                    body,
                    system,
                    "Host form fields for this step",
                    false,
                );
            } else {
                StatefulWidget::render(
                    &Form::new(fieldsets, system).focused_field(state.focused_field.as_ref()),
                    body,
                    buffer,
                    &mut state.form,
                );
            }
        }
        SetupStepKind::Permission => {
            if let Some(prompt) = permission {
                if !state.permission.is_empty() {
                    StatefulWidget::render(prompt, body, buffer, &mut state.permission);
                } else {
                    EmptyState::new("Permissions", system)
                        .kind(EmptyKind::PermissionLimited)
                        .explanation("Review tool trust on first use. Host opens gates when ready.")
                        .shortcut("Enter continue")
                        .paint(body, buffer);
                }
            } else {
                EmptyState::new("Permissions", system)
                    .kind(EmptyKind::PermissionLimited)
                    .explanation("Host will request elevated tools before first run.")
                    .paint(body, buffer);
            }
        }
        SetupStepKind::Theme => {
            let paint = theme_paint.unwrap_or(system);
            StatefulWidget::render(
                &ThemePicker::new(theme_presets, paint),
                body,
                buffer,
                &mut state.theme,
            );
        }
        SetupStepKind::Summary => {
            paint_summary(
                buffer,
                body,
                system,
                summary_lines,
                state.show_all_summary,
                false,
            );
        }
        SetupStepKind::Recovery => {
            let msg = state
                .wizard
                .progress()
                .failure_message
                .unwrap_or_else(|| "Setup failed — retry or cancel".into());
            EmptyState::new("Recovery", system)
                .kind(EmptyKind::NoData)
                .explanation(msg.as_str())
                .shortcut("r retry · Esc cancel")
                .paint(body, buffer);
        }
    }

    paint_primary_action(buffer, body, system, state);
    paint_cancel_confirm(buffer, frame, system, state);
}

/// The step's shippable action, as a button rather than only a chord.
///
/// A wizard whose only way forward is Enter teaches that the action is
/// invisible; the chord still works, and now the action is visible too
/// (plans/016 Step 3).
fn paint_primary_action(
    buffer: &mut Buffer,
    body: Rect,
    system: &DesignSystem,
    state: &SetupWizardState,
) {
    if state.cancel_confirm || body.height < 4 {
        return;
    }
    let last = state.wizard.step().saturating_add(1) >= state.wizard.step_count();
    let label = if last { "Finish" } else { "Continue" };
    let button = Button::new(label, system).variant(ButtonVariant::Primary);
    let width = button.preferred_width().min(body.width);
    if width == 0 {
        return;
    }
    let rect = Rect::new(
        body.right().saturating_sub(width),
        body.bottom().saturating_sub(1),
        width,
        1,
    );
    let mut button_state = ButtonState::new();
    button_state.activation.set_accepts_input(true);
    button.paint(rect, buffer, &mut button_state);
}

fn paint_cancel_confirm(
    buffer: &mut Buffer,
    frame: Rect,
    system: &DesignSystem,
    state: &SetupWizardState,
) {
    if !state.cancel_confirm || frame.height < 2 {
        return;
    }
    let strip = Rect {
        x: frame.x,
        y: frame.y.saturating_add(frame.height.saturating_sub(1)),
        width: frame.width,
        height: 1,
    };
    // One confirm surface for the whole library (plans/016 Step 1).
    let prompt = Rect {
        x: strip.x,
        y: strip.y.saturating_sub(1),
        width: strip.width,
        height: 2,
    };
    ConfirmPrompt::new("Leave setup?", "Leave", system)
        .detail("the steps you have finished are kept")
        .cancel_label("Stay")
        .colorless(state.colorless)
        .paint(prompt, buffer);
}

fn paint_capability_list(
    buffer: &mut Buffer,
    area: Rect,
    system: &DesignSystem,
    lines: &[CapabilityLine<'_>],
    _ascii: bool,
) {
    if area.is_empty() {
        return;
    }
    // A capability report is key/value data, so it reads as key/value data:
    // labels strong, findings quiet, and the severity on the row's own status
    // rather than on the whole sentence (plans/010 Step 3).
    system.paint_row(
        buffer,
        Rect::new(area.x, area.y, area.width, 1),
        "Terminal capabilities",
        system.style(Role::TextStrong),
    );
    let body = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    );
    if body.height == 0 {
        return;
    }
    if lines.is_empty() {
        system.paint_row(
            buffer,
            Rect::new(body.x, body.y, body.width, 1),
            "(host projects doctor rows)",
            system.style(Role::TextMuted),
        );
        return;
    }
    let entries: Vec<KvEntry<'_, usize>> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            KvEntry::pair(i, line.label, line.status).status(if line.problem {
                KvStatus::Danger
            } else {
                KvStatus::Success
            })
        })
        .collect();
    let mut state = KeyValueListState::new();
    KeyValueList::new(&entries, system).paint(body, buffer, &mut state);
}

fn paint_summary(
    buffer: &mut Buffer,
    area: Rect,
    system: &DesignSystem,
    lines: &[SetupSummaryLine<'_>],
    show_all: bool,
    _ascii: bool,
) {
    if area.is_empty() {
        return;
    }
    system.paint_row(
        buffer,
        Rect::new(area.x, area.y, area.width, 1),
        "Review",
        system.style(Role::TextStrong),
    );
    let body = Rect::new(
        area.x,
        area.y.saturating_add(1),
        area.width,
        area.height.saturating_sub(1),
    );
    if body.height == 0 {
        return;
    }
    if lines.is_empty() {
        system.paint_row(
            buffer,
            Rect::new(body.x, body.y, body.width, 1),
            "Host summary projection",
            system.style(Role::TextMuted),
        );
        return;
    }
    // The default frame is what the operator decided; `a` opens the rest.
    let shown: Vec<&SetupSummaryLine<'_>> = lines
        .iter()
        .filter(|line| show_all || line.changed)
        .collect();
    let hidden = lines.len().saturating_sub(shown.len());
    let note_rows = u16::from(hidden > 0);
    let list_area = Rect::new(
        body.x,
        body.y,
        body.width,
        body.height.saturating_sub(note_rows),
    );
    if shown.is_empty() {
        system.paint_row(
            buffer,
            Rect::new(body.x, body.y, body.width, 1),
            "Nothing changed from the defaults",
            system.style(Role::TextMuted),
        );
    } else if list_area.height > 0 {
        let entries: Vec<KvEntry<'_, usize>> = shown
            .iter()
            .enumerate()
            .map(|(i, line)| KvEntry::pair(i, line.label, line.value))
            .collect();
        let mut state = KeyValueListState::new();
        KeyValueList::new(&entries, system).paint(list_area, buffer, &mut state);
    }
    if let Some(note) = crate::text::more_note(hidden) {
        let y = body.y.saturating_add(list_area.height.min(body.height));
        if y < body.bottom() {
            system.paint_row(
                buffer,
                Rect::new(body.x, y, body.width, 1),
                &format!("{note} · a all"),
                system.style(Role::TextMuted),
            );
        }
    }
}

fn paint_body_hint(
    buffer: &mut Buffer,
    area: Rect,
    system: &DesignSystem,
    text: &str,
    _ascii: bool,
) {
    if area.is_empty() {
        return;
    }
    system.paint_row(
        buffer,
        Rect::new(area.x, area.y, area.width, 1),
        text,
        system.style(Role::TextMuted),
    );
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Canonical first-run step sequence (dense, not marketing-heavy).
#[must_use]
pub fn example_setup_steps() -> Vec<SetupStep> {
    vec![
        SetupStep::new("welcome", "Welcome", SetupStepKind::Welcome).description("Start setup"),
        SetupStep::new("caps", "Terminal", SetupStepKind::Capability)
            .description("Capability check"),
        SetupStep::new("account", "Account", SetupStepKind::Account).description("Identity"),
        SetupStep::new("connection", "Connection", SetupStepKind::Connection)
            .description("Endpoint")
            .optional(true),
        SetupStep::new("choices", "Choices", SetupStepKind::Choices)
            .description("Preferences")
            .optional(true),
        SetupStep::new("permissions", "Trust", SetupStepKind::Permission)
            .description("Permissions"),
        SetupStep::new("theme", "Theme", SetupStepKind::Theme).description("Appearance"),
        SetupStep::new("summary", "Review", SetupStepKind::Summary).description("Confirm"),
    ]
}

/// Alias for product docs that say "onboarding".
#[must_use]
pub fn example_onboarding_setup_steps() -> Vec<SetupStep> {
    example_setup_steps()
}

/// Demo capability rows.
#[must_use]
pub fn example_capability_lines() -> Vec<CapabilityLine<'static>> {
    vec![
        CapabilityLine::new("truecolor", "ok"),
        CapabilityLine::new("unicode", "ok"),
        CapabilityLine::new("kitty keyboard", "partial").problem(true),
        CapabilityLine::new("sixel", "missing").problem(true),
    ]
}

/// Demo account fields.
#[must_use]
pub fn example_setup_account_fields() -> [Field<'static, &'static str>; 2] {
    [
        Field::new("email", "Email", "")
            .required(true)
            .error("required")
            .touched(true),
        Field::new("name", "Display name", "Ada"),
    ]
}

/// Demo connection fields.
#[must_use]
pub fn example_setup_connection_fields() -> [Field<'static, &'static str>; 2] {
    [
        Field::new("endpoint", "Endpoint", "https://api.example")
            .required(true)
            .dirty(true),
        Field::new("token", "Token", "••••••••").required(true),
    ]
}

/// Demo choices fields.
#[must_use]
pub fn example_setup_choices_fields() -> [Field<'static, &'static str>; 2] {
    [
        Field::new("region", "Region", "us-east").dirty(true),
        Field::new("plan", "Plan", "hobby"),
    ]
}

/// Demo summary lines.
#[must_use]
pub fn example_setup_summary_lines() -> Vec<SetupSummaryLine<'static>> {
    vec![
        SetupSummaryLine::edited("Account", "Ada <ada@example>"),
        SetupSummaryLine::edited("Endpoint", "https://api.example"),
        SetupSummaryLine::untouched("Theme", "junie"),
        SetupSummaryLine::untouched("Trust", "default-deny tools"),
    ]
}

/// Build WizardStep list only (for hosts that only need FormWizard).
#[must_use]
pub fn setup_steps_to_wizard_steps(steps: &[SetupStep]) -> Vec<WizardStep> {
    steps.iter().map(|s| s.step.clone()).collect()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    #[test]
    fn example_steps_cover_required_kinds() {
        let steps = example_setup_steps();
        let kinds: Vec<_> = steps.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SetupStepKind::Welcome));
        assert!(kinds.contains(&SetupStepKind::Capability));
        assert!(kinds.contains(&SetupStepKind::Account));
        assert!(kinds.contains(&SetupStepKind::Permission));
        assert!(kinds.contains(&SetupStepKind::Theme));
        assert!(kinds.contains(&SetupStepKind::Summary));
    }

    #[test]
    fn layout_fullscreen_has_margin_on_large() {
        let large = layout_setup_wizard(Rect::new(0, 0, 120, 40), SetupWizardMode::Fullscreen);
        assert!(large.frame.width < 120 || large.frame.x > 0);
        let inline = layout_setup_wizard(Rect::new(0, 0, 40, 12), SetupWizardMode::Inline);
        assert_eq!(inline.frame, Rect::new(0, 0, 40, 12));
    }

    #[test]
    fn safe_cancel_two_step() {
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        let out = st.request_cancel();
        assert!(matches!(out, SetupWizardOutcome::CancelConfirmOpen));
        assert!(st.cancel_confirm);
        let out = st.dismiss_cancel_confirm();
        assert!(matches!(out, SetupWizardOutcome::CancelConfirmDismissed));
        let out = st.request_cancel();
        assert!(matches!(out, SetupWizardOutcome::CancelConfirmOpen));
        let out = st.request_cancel();
        assert!(matches!(out, SetupWizardOutcome::CancelConfirmed));
    }

    #[test]
    fn esc_opens_cancel_confirm() {
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        let fields = example_setup_account_fields();
        let sets = [Fieldset::new("Account", &fields)];
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(out, SetupWizardOutcome::CancelConfirmOpen));
    }

    #[test]
    fn resume_restores_step() {
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        st.set_step_valid(true);
        let _ = st.wizard.next();
        let _ = st.wizard.next();
        let snap = st.progress();
        let mut other = SetupWizardState::from_steps(example_setup_steps());
        let out = other.resume(&snap);
        assert!(matches!(out, SetupWizardOutcome::Resumed { step } if step >= 1));
    }

    #[test]
    fn welcome_enter_advances() {
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        assert_eq!(st.current_kind(), SetupStepKind::Welcome);
        st.set_gate(WizardGate::Valid);
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &[],
            BUILTIN_THEME_PRESETS,
        );
        assert!(matches!(
            out,
            SetupWizardOutcome::Wizard(FormWizardOutcome::StepChanged { .. })
                | SetupWizardOutcome::Wizard(_)
        ));
    }

    #[test]
    fn form_step_validation_blocks() {
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        // jump to account
        while st.current_kind() != SetupStepKind::Account && st.wizard.step() < 10 {
            st.set_gate(WizardGate::Valid);
            let _ = st.wizard.next();
        }
        st.set_gate(WizardGate::Invalid);
        let fields = example_setup_account_fields();
        let sets = [Fieldset::new("Account", &fields)];
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &sets,
            BUILTIN_THEME_PRESETS,
        );
        assert!(
            matches!(
                out,
                SetupWizardOutcome::Wizard(FormWizardOutcome::BlockedInvalid { .. })
            ),
            "{out:?}"
        );
    }

    #[test]
    fn paint_welcome_capability_theme_summary() {
        let system = DesignSystem::default();
        let caps = example_capability_lines();
        let summary = example_setup_summary_lines();
        let fields = example_setup_account_fields();
        let sets = [Fieldset::new("Account", &fields)];
        let mut st = SetupWizardState::from_steps(example_setup_steps()).with_title("First run");
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render_setup_wizard(
            &mut buf,
            area,
            SetupWizardSurfaces {
                system: &system,
                state: &mut st,
                fieldsets: &sets,
                capabilities: &caps,
                summary_lines: &summary,
                welcome_title: "TermRock setup",
                welcome_detail: "Configure once. Keyboard-first.",
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: Some(&system),
                permission: None,
            },
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Welcome")
                || text.contains("setup")
                || text.contains("Setup")
                || text.contains("TermRock"),
            "{text}"
        );

        // Advance to theme and paint
        while !matches!(st.current_kind(), SetupStepKind::Theme) && st.wizard.step() < 20 {
            st.set_gate(WizardGate::Valid);
            let _ = st.wizard.next();
            if matches!(st.wizard.phase(), WizardPhase::Review) {
                break;
            }
        }
        let mut buf = Buffer::empty(area);
        render_setup_wizard(
            &mut buf,
            area,
            SetupWizardSurfaces {
                system: &system,
                state: &mut st,
                fieldsets: &[],
                capabilities: &caps,
                summary_lines: &summary,
                welcome_title: "TermRock setup",
                welcome_detail: "Configure once.",
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: Some(&system),
                permission: None,
            },
        );
    }

    #[test]
    fn paint_inline_mode() {
        let system = DesignSystem::default();
        let mut st = SetupWizardState::from_steps(example_setup_steps())
            .with_mode(SetupWizardMode::Inline)
            .with_title("Inline setup");
        let area = Rect::new(0, 0, 48, 14);
        let mut buf = Buffer::empty(area);
        render_setup_wizard(
            &mut buf,
            area,
            SetupWizardSurfaces {
                system: &system,
                state: &mut st,
                fieldsets: &[],
                capabilities: &[],
                summary_lines: &[],
                welcome_title: "Hi",
                welcome_detail: "Short",
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: None,
                permission: None,
            },
        );
    }

    #[test]
    fn recovery_phase_paint() {
        let system = DesignSystem::default();
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        let _ = st.wizard.fail("network unreachable");
        assert_eq!(st.current_kind(), SetupStepKind::Recovery);
        let area = Rect::new(0, 0, 60, 16);
        let mut buf = Buffer::empty(area);
        render_setup_wizard(
            &mut buf,
            area,
            SetupWizardSurfaces {
                system: &system,
                state: &mut st,
                fieldsets: &[],
                capabilities: &[],
                summary_lines: &[],
                welcome_title: "x",
                welcome_detail: "y",
                theme_presets: BUILTIN_THEME_PRESETS,
                theme_paint: None,
                permission: None,
            },
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Recovery") || text.contains("retry") || text.contains("network"),
            "{text}"
        );
    }

    #[test]
    fn terminal_smoke() {
        let system = DesignSystem::default();
        let mut st = SetupWizardState::from_steps(example_setup_steps());
        let caps = example_capability_lines();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_setup_wizard(
                    f.buffer_mut(),
                    area,
                    SetupWizardSurfaces {
                        system: &system,
                        state: &mut st,
                        fieldsets: &[],
                        capabilities: &caps,
                        summary_lines: &[],
                        welcome_title: "Setup",
                        welcome_detail: "Keyboard only",
                        theme_presets: BUILTIN_THEME_PRESETS,
                        theme_paint: None,
                        permission: None,
                    },
                );
            })
            .unwrap();
    }

    #[test]
    fn public_api_no_process() {
        let src = include_str!("setup_wizard.rs");
        assert!(src.contains("public"));
        assert!(src.contains("host-owned") || src.contains("Host owns"));
        let forbidden = [format!("{}::process", "std"), format!("{}::new", "Command")];
        for f in &forbidden {
            assert!(!src.contains(f.as_str()), "{f}");
        }
    }

    #[test]
    fn fixtures_non_empty() {
        assert!(!example_setup_steps().is_empty());
        assert!(!example_capability_lines().is_empty());
        assert!(!example_setup_summary_lines().is_empty());
    }
}
