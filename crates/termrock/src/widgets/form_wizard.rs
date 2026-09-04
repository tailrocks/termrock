// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Multi-step form flow for setup, onboarding, connections, and migrations.
//!
//! **Mission.** Wizards need step navigation, validation gates, optional
//! steps, review, async checks, failure/retry, and saved progress — without
//! owning domain field values. Hosts own form data; FormWizard owns chrome,
//! gates, and typed side-effect-free outcomes.
//!
//! **vs [`Form`](super::Form).** Form is single-surface field chrome. Wizard
//! sequences multiple host-owned step surfaces with a stepper and nav.
//! **vs [`super::Stepper`].** Stepper is the reusable step chrome; FormWizard
//! embeds it for paint and projects [`StepItem`] steps onto it.
//!
//! Research: Huh forms, installers, cloud CLIs, onboarding wizards.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

use super::{
    Panel, PanelChrome, PanelVariant, StepItem, StepStatus, Stepper, StepperNavPolicy,
    StepperOrientation, StepperPresentation, StepperState,
};

/// Width under which stepper collapses to title-only / single-step layout.
pub const FORM_WIZARD_NARROW_MAX_WIDTH: u16 = 36;
/// Height under which chrome is minimized.
pub const FORM_WIZARD_COMPACT_MAX_HEIGHT: u16 = 10;

// ── Step model (shared with Stepper) ─────────────────────────────────────────

/// Host-projected gate for the **current** step (or review).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WizardGate {
    /// May advance / finish.
    #[default]
    Valid,
    /// Blocking validation errors (host messages separate).
    Invalid,
    /// Async check in flight — block advance.
    Pending,
}

impl WizardGate {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::Pending => "pending",
        }
    }

    /// Whether advance is allowed.
    #[must_use]
    pub const fn allows_advance(self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Wizard high-level phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WizardPhase {
    /// On a form step (or review-as-step if review disabled and last).
    #[default]
    Step,
    /// Review screen before submit.
    Review,
    /// Failure surface with retry.
    Failed,
}

impl WizardPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Review => "review",
            Self::Failed => "failed",
        }
    }
}

/// Presentation / layout density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FormWizardPresentation {
    /// Full stepper + body + nav.
    #[default]
    Full,
    /// Narrow: current step title only (single-step layout).
    Narrow,
    /// Compact chrome for short terminals.
    Compact,
}

/// Why the step index changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StepChangeReason {
    /// Next / continue.
    Next,
    /// Back.
    Back,
    /// Jump via stepper click / host.
    Jump,
    /// Skip optional.
    Skip,
    /// Resume from snapshot.
    Resume,
}

// ── Progress snapshot (host may persist) ────────────────────────────────────

/// Serializable navigation progress (not domain field values).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardProgress {
    /// Current step index (clamped on restore).
    pub step_index: usize,
    /// Phase.
    pub phase: WizardPhase,
    /// Step ids marked complete.
    pub completed: Vec<String>,
    /// Step ids skipped.
    pub skipped: Vec<String>,
    /// Failure message if phase is Failed.
    pub failure_message: Option<String>,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Side-effect-free wizard outcomes. Host owns persistence and submit I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormWizardOutcome {
    /// No effect.
    Ignored,
    /// Chrome / gate projection changed.
    Changed,
    /// Step index or phase navigation.
    StepChanged {
        /// Previous index.
        from: usize,
        /// New index.
        to: usize,
        /// Reason.
        reason: StepChangeReason,
    },
    /// Entered review phase.
    ReviewOpened,
    /// Left review back to last step.
    ReviewClosed {
        /// Step index restored.
        index: usize,
    },
    /// Host should focus first relevant field on this step.
    FocusFieldRequested {
        /// Step index.
        step: usize,
        /// Optional host field id hint (first invalid / first field).
        field_hint: Option<String>,
    },
    /// Advance blocked by invalid gate.
    BlockedInvalid {
        /// Step.
        step: usize,
        /// Host message if any.
        message: Option<String>,
    },
    /// Advance blocked while async pending.
    BlockedPending {
        /// Step.
        step: usize,
    },
    /// Host should run async validation for step (`generation` race gate).
    AsyncCheckRequested {
        /// Step index.
        step: usize,
        /// Generation.
        generation: u64,
    },
    /// Finish / submit (from review or last step when review disabled).
    SubmitRequested,
    /// Entered failed phase (host or internal).
    Failed {
        /// Message.
        message: String,
    },
    /// User requested retry after failure.
    RetryRequested {
        /// Step to return to.
        step: usize,
    },
    /// Cancel wizard.
    Cancelled,
    /// Progress snapshot for host persistence.
    ProgressSaved {
        /// Snapshot.
        progress: WizardProgress,
    },
    /// Presentation auto-changed.
    PresentationChanged {
        /// Presentation.
        presentation: FormWizardPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`FormWizard`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormWizardState {
    steps: Vec<StepItem>,
    index: usize,
    phase: WizardPhase,
    /// Per-step gate (host projects; length == steps).
    gates: Vec<WizardGate>,
    /// Per-step status for stepper (derived + host overrides).
    statuses: Vec<StepStatus>,
    completed: Vec<String>,
    skipped: Vec<String>,
    /// Enable review screen before submit.
    review_enabled: bool,
    /// Allow skipping optional steps.
    allow_skip: bool,
    /// Linear only (cannot jump ahead of first incomplete).
    linear: bool,
    gate_message: Option<String>,
    failure_message: Option<String>,
    /// First field hint for FocusFieldRequested.
    field_hint: Option<String>,
    /// Async generation.
    generation: u64,
    presentation: FormWizardPresentation,
    focused: bool,
    enabled: bool,
    // geometry
    stepper_hits: Vec<(usize, Rect)>,
    nav_back: Rect,
    nav_next: Rect,
    nav_skip: Rect,
    nav_cancel: Rect,
    body_area: Rect,
    root: Rect,
}

impl Default for FormWizardState {
    fn default() -> Self {
        Self::new(1)
    }
}

impl FormWizardState {
    /// Wizard with N untitled placeholder steps (`step-0`…).
    ///
    /// Prefer [`Self::with_steps`] for titled optional steps.
    #[must_use]
    pub fn new(step_count: usize) -> Self {
        let n = step_count.max(1);
        let steps = (0..n)
            .map(|i| StepItem::new(format!("step-{i}"), format!("Step {}", i + 1)))
            .collect::<Vec<_>>();
        Self::with_steps(steps)
    }

    /// Wizard from step definitions.
    #[must_use]
    pub fn with_steps(steps: impl IntoIterator<Item = StepItem>) -> Self {
        let steps: Vec<_> = steps.into_iter().collect();
        let steps = if steps.is_empty() {
            vec![StepItem::new("step-0", "Step 1")]
        } else {
            steps
        };
        let n = steps.len();
        let mut statuses = vec![StepStatus::Future; n];
        statuses[0] = StepStatus::Current;
        Self {
            steps,
            index: 0,
            phase: WizardPhase::Step,
            gates: vec![WizardGate::Valid; n],
            statuses,
            completed: Vec::new(),
            skipped: Vec::new(),
            review_enabled: true,
            allow_skip: true,
            linear: true,
            gate_message: None,
            failure_message: None,
            field_hint: None,
            generation: 0,
            presentation: FormWizardPresentation::Full,
            focused: false,
            enabled: true,
            stepper_hits: Vec::new(),
            nav_back: Rect::default(),
            nav_next: Rect::default(),
            nav_skip: Rect::default(),
            nav_cancel: Rect::default(),
            body_area: Rect::default(),
            root: Rect::default(),
        }
    }

    /// Review screen before submit.
    #[must_use]
    pub const fn with_review(mut self, on: bool) -> Self {
        self.review_enabled = on;
        self
    }

    /// Allow skipping optional steps.
    #[must_use]
    pub const fn with_allow_skip(mut self, on: bool) -> Self {
        self.allow_skip = on;
        self
    }
    // ── accessors ───────────────────────────────────────────────────────────

    /// Current index.
    #[must_use]
    pub const fn step(&self) -> usize {
        self.index
    }

    /// Step count.
    #[must_use]
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Phase.
    #[must_use]
    pub const fn phase(&self) -> WizardPhase {
        self.phase
    }

    /// Current step def.
    #[must_use]
    pub fn current_step(&self) -> Option<&StepItem> {
        self.steps.get(self.index)
    }

    /// Gate for current step.
    #[must_use]
    pub fn current_gate(&self) -> WizardGate {
        self.gates
            .get(self.index)
            .copied()
            .unwrap_or(WizardGate::Valid)
    }

    /// Body paint area (host renders step form here after paint).
    #[must_use]
    pub const fn body_area(&self) -> Rect {
        self.body_area
    }

    /// Progress snapshot.
    #[must_use]
    pub fn progress(&self) -> WizardProgress {
        WizardProgress {
            step_index: self.index,
            phase: self.phase,
            completed: self.completed.clone(),
            skipped: self.skipped.clone(),
            failure_message: self.failure_message.clone(),
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Host projects gate for current step (or all via set_gates).
    pub fn set_gate(&mut self, gate: WizardGate) {
        if let Some(g) = self.gates.get_mut(self.index) {
            *g = gate;
        }
    }

    /// Compatibility: `set_step_valid(true/false)`.
    pub fn set_step_valid(&mut self, valid: bool) {
        self.set_gate(if valid {
            WizardGate::Valid
        } else {
            WizardGate::Invalid
        });
    }

    /// Field focus hint for next FocusFieldRequested.
    pub fn set_field_hint(&mut self, hint: Option<String>) {
        self.field_hint = hint;
    }

    /// Mark current step pending and request async check.
    pub fn request_async_check(&mut self) -> FormWizardOutcome {
        self.set_gate(WizardGate::Pending);
        self.generation = self.generation.saturating_add(1);
        FormWizardOutcome::AsyncCheckRequested {
            step: self.index,
            generation: self.generation,
        }
    }

    /// Apply async result (race-safe).
    pub fn apply_async_result(&mut self, generation: u64, gate: WizardGate) -> bool {
        if generation != self.generation {
            return false;
        }
        self.set_gate(gate);
        true
    }

    /// Enter failed phase with message.
    pub fn fail(&mut self, message: impl Into<String>) -> FormWizardOutcome {
        let message = message.into();
        self.phase = WizardPhase::Failed;
        self.failure_message = Some(message.clone());
        FormWizardOutcome::Failed { message }
    }

    /// Resume from saved progress (preserves host domain data separately).
    pub fn restore_progress(&mut self, progress: &WizardProgress) -> FormWizardOutcome {
        let max = self.steps.len().saturating_sub(1);
        self.index = progress.step_index.min(max);
        self.phase = progress.phase;
        self.completed = progress.completed.clone();
        self.skipped = progress.skipped.clone();
        self.failure_message = progress.failure_message.clone();
        self.rebuild_statuses();
        FormWizardOutcome::StepChanged {
            from: 0,
            to: self.index,
            reason: StepChangeReason::Resume,
        }
    }

    /// Snapshot outcome for host persistence.
    pub fn save_progress(&self) -> FormWizardOutcome {
        FormWizardOutcome::ProgressSaved {
            progress: self.progress(),
        }
    }

    fn rebuild_statuses(&mut self) {
        let n = self.steps.len();
        self.statuses = vec![StepStatus::Future; n];
        for (i, step) in self.steps.iter().enumerate() {
            if self.skipped.iter().any(|s| s == &step.id) {
                self.statuses[i] = StepStatus::Skipped;
            } else if self.completed.iter().any(|s| s == &step.id) {
                self.statuses[i] = StepStatus::Complete;
            }
        }
        match self.phase {
            WizardPhase::Step => {
                let gate = self.current_gate();
                if let Some(s) = self.statuses.get_mut(self.index) {
                    *s = if matches!(gate, WizardGate::Invalid) {
                        StepStatus::Error
                    } else {
                        StepStatus::Current
                    };
                }
            }
            WizardPhase::Review | WizardPhase::Failed => {
                // leave completed marks
            }
        }
    }

    fn mark_current_complete(&mut self) {
        if let Some(step) = self.steps.get(self.index) {
            let id = step.id.clone();
            if !self.completed.iter().any(|s| s == &id) {
                self.completed.push(id);
            }
            self.skipped.retain(|s| s != &step.id);
        }
    }

    fn mark_current_skipped(&mut self) {
        if let Some(step) = self.steps.get(self.index) {
            if !step.optional {
                return;
            }
            let id = step.id.clone();
            if !self.skipped.iter().any(|s| s == &id) {
                self.skipped.push(id);
            }
            self.completed.retain(|s| s != &step.id);
        }
    }

    /// Auto presentation from bounds.
    #[must_use]
    pub fn presentation_for_bounds(bounds: Rect) -> FormWizardPresentation {
        if bounds.width < FORM_WIZARD_NARROW_MAX_WIDTH {
            FormWizardPresentation::Narrow
        } else if bounds.height < FORM_WIZARD_COMPACT_MAX_HEIGHT {
            FormWizardPresentation::Compact
        } else {
            FormWizardPresentation::Full
        }
    }

    fn focus_outcome(&self) -> FormWizardOutcome {
        FormWizardOutcome::FocusFieldRequested {
            step: self.index,
            field_hint: self.field_hint.clone(),
        }
    }

    /// Go to next (or review / submit).
    pub fn next(&mut self) -> FormWizardOutcome {
        if !self.enabled {
            return FormWizardOutcome::Ignored;
        }
        match self.phase {
            WizardPhase::Failed => FormWizardOutcome::Ignored,
            WizardPhase::Review => FormWizardOutcome::SubmitRequested,
            WizardPhase::Step => {
                match self.current_gate() {
                    WizardGate::Invalid => {
                        return FormWizardOutcome::BlockedInvalid {
                            step: self.index,
                            message: self.gate_message.clone(),
                        };
                    }
                    WizardGate::Pending => {
                        return FormWizardOutcome::BlockedPending { step: self.index };
                    }
                    WizardGate::Valid => {}
                }
                self.mark_current_complete();
                let from = self.index;
                if self.index + 1 >= self.steps.len() {
                    if self.review_enabled {
                        self.phase = WizardPhase::Review;
                        self.rebuild_statuses();
                        return FormWizardOutcome::ReviewOpened;
                    }
                    self.rebuild_statuses();
                    return FormWizardOutcome::SubmitRequested;
                }
                self.index += 1;
                self.rebuild_statuses();
                // Emit step change; host should also handle focus
                FormWizardOutcome::StepChanged {
                    from,
                    to: self.index,
                    reason: StepChangeReason::Next,
                }
            }
        }
    }

    /// Back.
    pub fn back(&mut self) -> FormWizardOutcome {
        if !self.enabled {
            return FormWizardOutcome::Ignored;
        }
        match self.phase {
            WizardPhase::Failed => {
                self.phase = WizardPhase::Step;
                self.failure_message = None;
                self.rebuild_statuses();
                FormWizardOutcome::RetryRequested { step: self.index }
            }
            WizardPhase::Review => {
                self.phase = WizardPhase::Step;
                let index = self.index;
                self.rebuild_statuses();
                FormWizardOutcome::ReviewClosed { index }
            }
            WizardPhase::Step => {
                if self.index == 0 {
                    return FormWizardOutcome::Ignored;
                }
                let from = self.index;
                self.index -= 1;
                // data preserved: we never clear host state
                self.rebuild_statuses();
                FormWizardOutcome::StepChanged {
                    from,
                    to: self.index,
                    reason: StepChangeReason::Back,
                }
            }
        }
    }

    /// Skip optional current step.
    pub fn skip(&mut self) -> FormWizardOutcome {
        if !self.enabled || !self.allow_skip {
            return FormWizardOutcome::Ignored;
        }
        if !matches!(self.phase, WizardPhase::Step) {
            return FormWizardOutcome::Ignored;
        }
        let Some(step) = self.steps.get(self.index) else {
            return FormWizardOutcome::Ignored;
        };
        if !step.optional {
            return FormWizardOutcome::Ignored;
        }
        self.mark_current_skipped();
        let from = self.index;
        if self.index + 1 >= self.steps.len() {
            if self.review_enabled {
                self.phase = WizardPhase::Review;
                self.rebuild_statuses();
                return FormWizardOutcome::ReviewOpened;
            }
            self.rebuild_statuses();
            return FormWizardOutcome::SubmitRequested;
        }
        self.index += 1;
        self.rebuild_statuses();
        FormWizardOutcome::StepChanged {
            from,
            to: self.index,
            reason: StepChangeReason::Skip,
        }
    }

    /// Jump to step (stepper click).
    pub fn jump_to(&mut self, index: usize) -> FormWizardOutcome {
        if !self.enabled || index >= self.steps.len() {
            return FormWizardOutcome::Ignored;
        }
        if matches!(self.phase, WizardPhase::Failed) {
            return FormWizardOutcome::Ignored;
        }
        if self.linear {
            // only completed, skipped, current, or previous
            let allowed = index <= self.index
                || self
                    .statuses
                    .get(index)
                    .is_some_and(|s| matches!(s, StepStatus::Complete | StepStatus::Skipped));
            if !allowed && index > self.index {
                // allow only if all prior complete/skipped
                let prior_ok = (0..index).all(|i| {
                    matches!(
                        self.statuses.get(i),
                        Some(StepStatus::Complete | StepStatus::Skipped)
                    ) || i == self.index && self.current_gate().allows_advance()
                });
                if !prior_ok {
                    return FormWizardOutcome::Ignored;
                }
            }
        }
        self.phase = WizardPhase::Step;
        let from = self.index;
        self.index = index;
        self.rebuild_statuses();
        FormWizardOutcome::StepChanged {
            from,
            to: self.index,
            reason: StepChangeReason::Jump,
        }
    }

    /// Retry after failure (same as back from Failed).
    pub fn retry(&mut self) -> FormWizardOutcome {
        if !matches!(self.phase, WizardPhase::Failed) {
            return FormWizardOutcome::Ignored;
        }
        self.back()
    }

    /// Cancel.
    pub fn cancel(&mut self) -> FormWizardOutcome {
        FormWizardOutcome::Cancelled
    }

    /// After StepChanged, host may call to get focus request.
    pub fn request_focus_field(&self) -> FormWizardOutcome {
        self.focus_outcome()
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> FormWizardOutcome {
        if !key.is_press() || !self.enabled {
            return FormWizardOutcome::Ignored;
        }
        if !self.focused {
            return FormWizardOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Esc cancel always (never trap)
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return self.cancel();
        }

        match self.phase {
            WizardPhase::Failed => match key.code {
                KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),
                _ => FormWizardOutcome::Ignored,
            },
            WizardPhase::Review => match key.code {
                KeyCode::Enter if !ctrl => FormWizardOutcome::SubmitRequested,
                KeyCode::Left | KeyCode::Backspace | KeyCode::Char('p') | KeyCode::Char('P') => {
                    self.back()
                }
                _ => FormWizardOutcome::Ignored,
            },
            WizardPhase::Step => match key.code {
                KeyCode::Right | KeyCode::Char('n') | KeyCode::Char('N') => {
                    let out = self.next();
                    // Host should also focus first field — emit as secondary via Changed?
                    // We return StepChanged; host calls request_focus_field.
                    out
                }
                KeyCode::Enter if !ctrl => {
                    // Enter = next / submit path
                    self.next()
                }
                KeyCode::Left | KeyCode::Char('p') | KeyCode::Char('P') => self.back(),
                KeyCode::Char('s') | KeyCode::Char('S') if !ctrl => self.skip(),
                KeyCode::Char('s') | KeyCode::Char('S') if ctrl => self.save_progress(),
                _ => FormWizardOutcome::Ignored,
            },
        }
    }

    /// Mouse (stepper + nav hits after paint).
    pub fn handle_mouse(&mut self, event: MouseEvent) -> FormWizardOutcome {
        if !self.enabled {
            return FormWizardOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return FormWizardOutcome::Ignored;
        }
        self.focused = true;
        for (i, rect) in &self.stepper_hits {
            if rect.contains(event.position) {
                let i = *i;
                return self.jump_to(i);
            }
        }
        if self.nav_back.contains(event.position) {
            return self.back();
        }
        if self.nav_next.contains(event.position) {
            return self.next();
        }
        if self.nav_skip.contains(event.position) {
            return self.skip();
        }
        if self.nav_cancel.contains(event.position) {
            return self.cancel();
        }
        FormWizardOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Form wizard chrome: stepper + body slot + nav.
///
/// Host paints domain fields into [`FormWizardState::body_area`] after
/// [`Self::paint`].
#[derive(Debug, Clone, Copy)]
pub struct FormWizard<'a> {
    system: &'a DesignSystem,
    title: &'a str,
    show_stepper: bool,
    show_nav: bool,
}

impl<'a> FormWizard<'a> {
    /// Create.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            title: "Setup",
            show_stepper: true,
            show_nav: true,
        }
    }

    /// Compatibility: `FormWizard::new(tokens, label)`.
    #[must_use]
    pub const fn with_label(system: &'a DesignSystem, label: &'a str) -> Self {
        Self {
            system,
            title: label,
            show_stepper: true,
            show_nav: true,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// ASCII marks.
    #[must_use]
    /// Show stepper row.
    pub const fn show_stepper(mut self, on: bool) -> Self {
        self.show_stepper = on;
        self
    }

    /// Paint chrome; updates `body_area` for host content.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut FormWizardState) {
        state.stepper_hits.clear();
        state.nav_back = Rect::default();
        state.nav_next = Rect::default();
        state.nav_skip = Rect::default();
        state.nav_cancel = Rect::default();
        state.root = area;
        if area.is_empty() {
            state.body_area = area;
            return;
        }

        let pres = FormWizardState::presentation_for_bounds(area);
        if pres != state.presentation {
            state.presentation = pres;
        }

        let panel = Panel::new(self.system)
            .variant(PanelVariant::Bordered)
            .overlay(true)
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        panel.title(self.title).paint(area, buffer, None);
        if inner.is_empty() {
            state.body_area = inner;
            return;
        }

        let mut y = inner.y;

        // Stepper or narrow title
        if self.show_stepper && y < inner.bottom() {
            match state.presentation {
                FormWizardPresentation::Narrow | FormWizardPresentation::Compact => {
                    let label = match state.phase {
                        WizardPhase::Review => "Review".to_owned(),
                        WizardPhase::Failed => "Failed".to_owned(),
                        WizardPhase::Step => state
                            .current_step()
                            .map(|s| {
                                format!("{}/{} {}", state.index + 1, state.steps.len(), s.title)
                            })
                            .unwrap_or_else(|| {
                                format!("{}/{}", state.index + 1, state.steps.len())
                            }),
                    };
                    buffer.set_stringn(
                        inner.x,
                        y,
                        take_display_cols(&label, usize::from(inner.width)),
                        usize::from(inner.width),
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD),
                    );
                    y = y.saturating_add(1);
                }
                FormWizardPresentation::Full => {
                    y = self.paint_stepper(inner, y, buffer, state);
                }
            }
        }

        // Phase / gate banner
        if y < inner.bottom() {
            let banner = match state.phase {
                WizardPhase::Review => Some(("Review your answers", Role::TextMuted)),
                WizardPhase::Failed => Some((
                    state
                        .failure_message
                        .as_deref()
                        .unwrap_or("Something went wrong"),
                    Role::Danger,
                )),
                WizardPhase::Step => match state.current_gate() {
                    WizardGate::Invalid => Some((
                        state
                            .gate_message
                            .as_deref()
                            .unwrap_or("Fix errors to continue"),
                        Role::Danger,
                    )),
                    WizardGate::Pending => Some(({ "Checking…" }, Role::TextMuted)),
                    WizardGate::Valid => state
                        .current_step()
                        .and_then(|s| s.description.as_deref())
                        .map(|d| (d, Role::TextMuted)),
                },
            };
            if let Some((text, role)) = banner {
                if matches!(role, Role::Danger) {
                    super::field_message::paint_field_message(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        self.system,
                        super::DescriptionKind::Error,
                        text,
                    );
                } else {
                    buffer.set_stringn(
                        inner.x,
                        y,
                        take_display_cols(text, usize::from(inner.width)),
                        usize::from(inner.width),
                        self.system.style(role),
                    );
                }
            }
            // Gate feedback owns a permanent inline row. Pending/invalid
            // transitions therefore cannot move host fields or navigation.
            y = y.saturating_add(1);
        }

        // Nav row at bottom
        let nav_h = if self.show_nav { 1u16 } else { 0 };
        let body_bottom = inner.bottom().saturating_sub(nav_h);
        let body = Rect::new(inner.x, y, inner.width, body_bottom.saturating_sub(y));
        state.body_area = body;

        // Optional body placeholder chrome for empty host
        if !body.is_empty() && matches!(state.phase, WizardPhase::Review) {
            self.paint_review(body, buffer, state);
        } else if !body.is_empty() && matches!(state.phase, WizardPhase::Failed) {
            buffer.set_stringn(
                body.x,
                body.y,
                take_display_cols(
                    "Press Enter or r to retry · Esc cancel",
                    usize::from(body.width),
                ),
                usize::from(body.width),
                self.system.style(Role::TextMuted),
            );
        }

        if self.show_nav && nav_h > 0 {
            let nav = Rect::new(inner.x, body_bottom, inner.width, 1);
            self.paint_nav(nav, buffer, state);
        }
    }

    fn paint_stepper(
        &self,
        inner: Rect,
        y: u16,
        buffer: &mut Buffer,
        state: &mut FormWizardState,
    ) -> u16 {
        // Embed shared Stepper chrome; Host policy — FormWizard handles jumps.
        let mut st = StepperState::with_len(state.steps.len())
            .policy(StepperNavPolicy::Host)
            .orientation(StepperOrientation::Horizontal);
        st.set_statuses(state.statuses.iter().copied());
        st.set_current(state.index, state.steps.len(), true);
        st.set_focused(state.focused);
        st.set_accepts_input(state.focused);
        // Map FormWizard presentation → stepper presentation
        let override_pres = match state.presentation {
            FormWizardPresentation::Narrow => Some(StepperPresentation::Menu),
            FormWizardPresentation::Compact => Some(StepperPresentation::Compact),
            FormWizardPresentation::Full => None,
        };
        st.set_presentation_override(override_pres);
        let area = Rect::new(inner.x, y, inner.width, 1);
        Stepper::new(&state.steps, self.system).paint(area, buffer, &mut st);
        state.stepper_hits = st.hits().to_vec();
        y.saturating_add(1)
    }

    fn paint_review(&self, area: Rect, buffer: &mut Buffer, state: &FormWizardState) {
        let mut y = area.y;
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols("Summary", usize::from(area.width)),
            usize::from(area.width),
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD),
        );
        y = y.saturating_add(1);
        for (i, step) in state.steps.iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let st = state.statuses.get(i).copied().unwrap_or_default();
            let line = format!("{} {} {} {}", st.mark(), step.title, { "—" }, st.id());
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }
        if y < area.bottom() {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(
                    "(host projects field values into review)",
                    usize::from(area.width),
                ),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_nav(&self, area: Rect, buffer: &mut Buffer, state: &mut FormWizardState) {
        if area.is_empty() {
            return;
        }
        let mut x = area.x;
        // Back
        let back = { "← Back" };
        let bw = display_cols(back) as u16 + 1;
        let back_enabled = match state.phase {
            WizardPhase::Step => state.index > 0,
            WizardPhase::Review | WizardPhase::Failed => true,
        };
        let br = Rect::new(x, area.y, bw.min(area.right().saturating_sub(x)), 1);
        let back_recipe = self.system.button_recipe(
            ButtonRecipeVariant::Quiet,
            if back_enabled {
                ControlState::Default
            } else {
                ControlState::Disabled
            },
            self.system.junie_theme().surface,
        );
        buffer.set_style(br, back_recipe.fill);
        buffer.set_stringn(br.x, br.y, back, usize::from(br.width), back_recipe.label);
        if back_enabled {
            state.nav_back = br;
        }
        x = x.saturating_add(bw).saturating_add(1);

        // Skip
        let can_skip = state.allow_skip
            && matches!(state.phase, WizardPhase::Step)
            && state.current_step().is_some_and(|s| s.optional);
        if x < area.right() {
            let skip = "Skip";
            let sw = display_cols(skip) as u16 + 1;
            let sr = Rect::new(x, area.y, sw.min(area.right().saturating_sub(x)), 1);
            if can_skip {
                let recipe = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    ControlState::Default,
                    self.system.junie_theme().surface,
                );
                buffer.set_style(sr, recipe.fill);
                buffer.set_stringn(sr.x, sr.y, skip, usize::from(sr.width), recipe.label);
                state.nav_skip = sr;
            }
            // Keep this slot even on required steps; Cancel never jumps when
            // optionality changes.
            x = x.saturating_add(sw).saturating_add(1);
        }

        // Cancel far left-ish already have back; cancel on far right start
        // Next / Finish / Retry on right
        let next_label = match state.phase {
            WizardPhase::Failed => "Retry ↵",
            WizardPhase::Review => "Finish ↵",
            WizardPhase::Step if state.index + 1 >= state.steps.len() && !state.review_enabled => {
                "Finish ↵"
            }
            WizardPhase::Step if state.index + 1 >= state.steps.len() => "Review →",
            WizardPhase::Step => "Next →",
        };
        let nw = (display_cols(next_label) as u16).max(10);
        let nx = area.right().saturating_sub(nw).saturating_sub(1);
        let nr = Rect::new(
            nx.max(x),
            area.y,
            nw.min(area.right().saturating_sub(nx.max(x))),
            1,
        );
        let next_enabled = match state.phase {
            WizardPhase::Step => state.current_gate().allows_advance(),
            _ => true,
        };
        let next_recipe = self.system.button_recipe(
            ButtonRecipeVariant::Primary,
            if next_enabled {
                ControlState::Default
            } else {
                ControlState::Disabled
            },
            self.system.junie_theme().surface,
        );
        buffer.set_style(nr, next_recipe.fill);
        buffer.set_stringn(
            nr.x,
            nr.y,
            next_label,
            usize::from(nr.width),
            next_recipe.label.add_modifier(Modifier::BOLD),
        );
        if next_enabled {
            state.nav_next = nr;
        }

        // Cancel at end of left cluster
        if x < nr.x.saturating_sub(8) {
            let cancel = "Esc";
            let cw = 3u16;
            let cr = Rect::new(x, area.y, cw, 1);
            let recipe = self.system.button_recipe(
                ButtonRecipeVariant::Quiet,
                ControlState::Default,
                self.system.junie_theme().surface,
            );
            buffer.set_style(cr, recipe.fill);
            buffer.set_stringn(cr.x, cr.y, cancel, usize::from(cr.width), recipe.label);
            state.nav_cancel = cr;
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &FormWizardState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "form-wizard phase={} step={}/{} gate={}",
            state.phase.id(),
            state.index + 1,
            state.steps.len(),
            state.current_gate().id()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label(self.title)
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: matches!(state.current_gate(), WizardGate::Pending),
                    invalid: matches!(state.current_gate(), WizardGate::Invalid)
                        || matches!(state.phase, WizardPhase::Failed),
                    expanded: true,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &FormWizard<'_> {
    type State = FormWizardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for FormWizard<'_> {
    type State = FormWizardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Compatibility constructor used by old `FormWizard::new(tokens, label)` ──

impl<'a> FormWizard<'a> {
    /// Old signature adapter: `FormWizard::new(system, label)`.
    #[must_use]
    pub const fn labeled(system: &'a DesignSystem, label: &'a str) -> Self {
        Self::with_label(system, label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::RolePalette;
    use crate::widgets::tests::click;

    fn three_steps() -> FormWizardState {
        FormWizardState::with_steps([
            StepItem::new("account", "Account"),
            StepItem::new("region", "Region").optional(true),
            StepItem::new("confirm", "Confirm"),
        ])
        .with_review(true)
        .with_allow_skip(true)
    }

    #[test]
    fn blocks_invalid_next() {
        let mut w = three_steps();
        w.set_focused(true);
        w.set_step_valid(false);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            FormWizardOutcome::BlockedInvalid { .. }
        ));
        w.set_step_valid(true);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            FormWizardOutcome::StepChanged {
                to: 1,
                reason: StepChangeReason::Next,
                ..
            }
        ));
    }

    #[test]
    fn back_preserves_index_and_progress() {
        let mut w = three_steps();
        w.set_focused(true);
        let _ = w.next();
        assert_eq!(w.step(), 1);
        assert!(!w.progress().completed.is_empty());
        assert!(matches!(
            w.back(),
            FormWizardOutcome::StepChanged {
                to: 0,
                reason: StepChangeReason::Back,
                ..
            }
        ));
        // completed still has account
        assert!(w.progress().completed.iter().any(|s| s == "account"));
    }

    #[test]
    fn skip_optional() {
        let mut w = three_steps();
        w.set_focused(true);
        let _ = w.next(); // to region (optional)
        assert_eq!(w.current_step().map(|s| s.id.as_str()), Some("region"));
        assert!(matches!(
            w.skip(),
            FormWizardOutcome::StepChanged {
                reason: StepChangeReason::Skip,
                to: 2,
                ..
            }
        ));
        assert!(w.progress().skipped.iter().any(|s| s == "region"));
    }

    #[test]
    fn review_then_submit() {
        let mut w = three_steps().with_review(true);
        w.set_focused(true);
        let _ = w.next();
        let _ = w.next();
        assert!(matches!(w.next(), FormWizardOutcome::ReviewOpened));
        assert_eq!(w.phase(), WizardPhase::Review);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            FormWizardOutcome::SubmitRequested
        ));
    }

    #[test]
    fn no_review_submits_on_last() {
        let mut w = three_steps().with_review(false);
        w.set_focused(true);
        let _ = w.next();
        let _ = w.next();
        assert!(matches!(w.next(), FormWizardOutcome::SubmitRequested));
    }

    #[test]
    fn async_gate_and_race() {
        let mut w = three_steps();
        w.set_focused(true);
        let FormWizardOutcome::AsyncCheckRequested { generation, .. } = w.request_async_check()
        else {
            panic!("expected async");
        };
        assert_eq!(w.current_gate(), WizardGate::Pending);
        assert!(matches!(w.next(), FormWizardOutcome::BlockedPending { .. }));
        assert!(!w.apply_async_result(0, WizardGate::Valid));
        assert!(w.apply_async_result(generation, WizardGate::Valid));
        assert!(matches!(w.next(), FormWizardOutcome::StepChanged { .. }));
    }

    #[test]
    fn failure_retry_resume() {
        let mut w = three_steps();
        w.set_focused(true);
        let _ = w.next();
        assert!(matches!(
            w.fail("network down"),
            FormWizardOutcome::Failed { .. }
        ));
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            FormWizardOutcome::RetryRequested { step: 1 }
        ));
        let snap = w.progress();
        let mut w2 = three_steps();
        assert!(matches!(
            w2.restore_progress(&snap),
            FormWizardOutcome::StepChanged {
                reason: StepChangeReason::Resume,
                ..
            }
        ));
        assert_eq!(w2.step(), 1);
    }

    #[test]
    fn save_progress_outcome() {
        let mut w = three_steps();
        w.set_focused(true);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            FormWizardOutcome::ProgressSaved { .. }
        ));
    }

    #[test]
    fn esc_cancels() {
        let mut w = three_steps();
        w.set_focused(true);
        assert!(matches!(
            w.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            FormWizardOutcome::Cancelled
        ));
    }

    #[test]
    fn repeated_actions_are_ignored_across_wizard_phases() {
        let mut step = three_steps();
        step.set_focused(true);
        for (code, modifiers) in [
            (KeyCode::Right, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char('s'), KeyModifiers::CONTROL),
            (KeyCode::Esc, KeyModifiers::NONE),
        ] {
            let before = step.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(
                step.handle_key(key),
                FormWizardOutcome::Ignored,
                "{code:?} repeat emitted a wizard action"
            );
            assert_eq!(step, before, "{code:?} repeat mutated step state");
        }

        let mut review = three_steps();
        review.set_focused(true);
        let _ = review.next();
        let _ = review.next();
        assert!(matches!(review.next(), FormWizardOutcome::ReviewOpened));
        let before = review.clone();
        let mut repeat_submit = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        repeat_submit.kind = KeyEventKind::Repeat;
        assert_eq!(review.handle_key(repeat_submit), FormWizardOutcome::Ignored);
        assert_eq!(review, before);

        let mut failed = three_steps();
        failed.set_focused(true);
        let _ = failed.fail("network down");
        let before = failed.clone();
        let mut repeat_retry = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        repeat_retry.kind = KeyEventKind::Repeat;
        assert_eq!(failed.handle_key(repeat_retry), FormWizardOutcome::Ignored);
        assert_eq!(failed, before);
    }

    #[test]
    fn focus_field_after_step() {
        let mut w = three_steps();
        w.set_field_hint(Some("email".into()));
        assert!(matches!(
            w.request_focus_field(),
            FormWizardOutcome::FocusFieldRequested {
                field_hint: Some(h),
                ..
            } if h == "email"
        ));
    }

    #[test]
    fn narrow_presentation() {
        let tiny = Rect::new(0, 0, 20, 8);
        assert_eq!(
            FormWizardState::presentation_for_bounds(tiny),
            FormWizardPresentation::Narrow
        );
    }

    #[test]
    fn paint_and_mouse_nav() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state = three_steps();
        state.set_focused(true);
        let area = Rect::new(0, 0, 60, 14);
        let mut buf = Buffer::empty(area);
        FormWizard::new(&system)
            .title("Connect")
            .paint(area, &mut buf, &mut state);
        assert!(!state.body_area.is_empty());
        assert!(!state.stepper_hits.is_empty());
        // click next
        let nr = state.nav_next;
        assert!(matches!(
            state.handle_mouse(click(nr.x, nr.y)),
            FormWizardOutcome::StepChanged { .. }
        ));
    }

    #[test]
    fn count_constructor_yields_placeholder_steps() {
        let w = FormWizardState::new(3);
        assert_eq!(w.step_count(), 3);
        let empty = FormWizardState::new(0);
        assert_eq!(empty.step_count(), 1, "count clamps to at least one step");
    }

    #[test]
    fn fuzz_keys() {
        let mut w = three_steps();
        w.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(30) {
            let _ = w.handle_key(*key);
            // re-focus if cancelled mid-fuzz
            w.set_focused(true);
            w.set_step_valid(true);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = three_steps();
        state.set_focused(true);
        let area = Rect::new(0, 0, 56, 12);
        let mut buf = Buffer::empty(area);
        let w = FormWizard::new(&system);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let state = three_steps();
        let mut scene = SemanticScene::<&str, ()>::default();
        FormWizard::new(&system).register_semantic(
            &mut scene,
            "wiz",
            Rect::new(0, 0, 40, 10),
            &state,
        );
        assert!(scene.get(&"wiz").is_some());
    }

    #[test]
    fn labeled_constructor() {
        let system = DesignSystem::default();
        let _ = FormWizard::with_label(&system, "Wizard");
        let _ = FormWizard::labeled(&system, "Wizard");
    }
}
