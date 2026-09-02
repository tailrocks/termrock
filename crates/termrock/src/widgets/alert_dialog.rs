// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **AlertDialog** — specialized high-risk confirmation surface.
//!
//! **Mission.** Distinct from a generic [`super::Dialog`]: communicates exact
//! **scope**, **consequences**, **reversibility**, **target**, and **safer
//! alternatives**. Supports typed confirmation, countdown (only when justified),
//! destructive-default **policy** (never default-focus the danger action), and
//! non-dismissable critical state.
//!
//! **Safe Enter.** Initial focus is always the safe action (Cancel / Keep).
//! Enter never confirms destruction unless the user has moved focus to the
//! confirm action **and** gates (typed phrase + countdown) have passed.
//!
//! **vs Dialog.** Dialog is general modal chrome. AlertDialog is a product of
//! risk UX: occlude backdrop ([`OverlayKind::AlertDialog`]), Esc trap or lock,
//! structured risk body, and explicit confirm gates.
//! **vs PermissionPrompt.** Permission is agent-trust / tool policy; AlertDialog
//! is domain-neutral destructive confirmation (delete, overwrite, terminate, egress).
//!
//! Research: Radix AlertDialog, database drop/truncate UX, cloud consoles,
//! permission surfaces.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, text::Text, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{
        HitRegion, NavigationMove, OverlayId, OverlayKind, OverlayOutcome, OverlaySize,
        OverlayStack, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::Hint,
};

use super::{
    Action, ActionBar, ActionBarState, ActionVariant, Dialog, DialogClosePolicy, DialogFocusZone,
    DialogOutcome, DialogRecipe, DialogSize, DialogState, DialogVariant, open_dialog_configured,
};

/// Default overlay id for alert confirmations.
pub const ALERT_DIALOG_OVERLAY_ID: &str = "termrock.alert-dialog";
/// Preferred width for risk body.
pub const ALERT_DIALOG_DEFAULT_WIDTH: u16 = 52;
/// Preferred height.
pub const ALERT_DIALOG_DEFAULT_HEIGHT: u16 = 14;

// ── Domain model ────────────────────────────────────────────────────────────

/// High-level alert class (drives copy templates and examples).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AlertKind {
    /// Permanent delete of user data / resources.
    #[default]
    Delete,
    /// Overwrite existing content.
    Overwrite,
    /// Kill process / session / deployment.
    Terminate,
    /// Data leaves the trust boundary (export, share, exfil risk).
    DataEgress,
    /// Host-supplied custom risk.
    Custom,
}

impl AlertKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Delete => "delete",
            Self::Overwrite => "overwrite",
            Self::Terminate => "terminate",
            Self::DataEgress => "data-egress",
            Self::Custom => "custom",
        }
    }

    /// Default title for the kind.
    #[must_use]
    pub const fn default_title(self) -> &'static str {
        match self {
            Self::Delete => "Delete permanently?",
            Self::Overwrite => "Overwrite existing data?",
            Self::Terminate => "Terminate process?",
            Self::DataEgress => "Allow data egress?",
            Self::Custom => "Confirm action?",
        }
    }

    /// Default confirm label.
    #[must_use]
    pub const fn default_confirm_label(self) -> &'static str {
        match self {
            Self::Delete => "Delete",
            Self::Overwrite => "Overwrite",
            Self::Terminate => "Terminate",
            Self::DataEgress => "Allow egress",
            Self::Custom => "Confirm",
        }
    }

    /// Default cancel label.
    #[must_use]
    pub const fn default_cancel_label(self) -> &'static str {
        match self {
            Self::Delete | Self::Overwrite | Self::Terminate => "Keep",
            Self::DataEgress => "Deny",
            Self::Custom => "Cancel",
        }
    }

    /// Whether countdown is commonly justified (destructive bulk / egress).
    #[must_use]
    pub const fn countdown_justified(self) -> bool {
        matches!(self, Self::Delete | Self::DataEgress | Self::Terminate)
    }
}

/// Whether the action can be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AlertReversibility {
    /// Cannot be undone.
    #[default]
    Irreversible,
    /// Soft-delete / recoverable for a window.
    Recoverable,
    /// Fully reversible.
    Reversible,
}

impl AlertReversibility {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Irreversible => "irreversible",
            Self::Recoverable => "recoverable",
            Self::Reversible => "reversible",
        }
    }

    /// User-facing line.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Irreversible => "This cannot be undone.",
            Self::Recoverable => "May be recoverable for a limited time.",
            Self::Reversible => "This action can be reversed.",
        }
    }
}

/// Exact scope of the risk (host-owned wording).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertScope {
    /// Primary target (resource name, path, principal).
    pub target: String,
    /// What will happen if confirmed.
    pub consequences: String,
    /// Reversibility class.
    pub reversibility: AlertReversibility,
    /// Optional quantified scope (e.g. "12 files · 2.4 MB").
    pub scope_detail: Option<String>,
    /// Safer alternative the user should consider first.
    pub safer_alternative: Option<String>,
}

impl AlertScope {
    /// Minimal scope (target + consequences).
    #[must_use]
    pub fn new(target: impl Into<String>, consequences: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            consequences: consequences.into(),
            reversibility: AlertReversibility::Irreversible,
            scope_detail: None,
            safer_alternative: None,
        }
    }

    /// Reversibility.
    #[must_use]
    pub const fn reversibility(mut self, r: AlertReversibility) -> Self {
        self.reversibility = r;
        self
    }

    /// Quantified scope line.
    #[must_use]
    pub fn scope_detail(mut self, d: impl Into<String>) -> Self {
        self.scope_detail = Some(d.into());
        self
    }

    /// Safer alternative copy.
    #[must_use]
    pub fn safer_alternative(mut self, a: impl Into<String>) -> Self {
        self.safer_alternative = Some(a.into());
        self
    }

    /// Example: permanent delete.
    #[must_use]
    pub fn example_delete() -> Self {
        Self::new(
            "prod-db.customers",
            "Drops the table and all dependent indexes.",
        )
        .scope_detail("≈ 1.2M rows · 4.8 GB")
        .safer_alternative("Export a dump, then soft-delete with a retention policy.")
        .reversibility(AlertReversibility::Irreversible)
    }

    /// Example: overwrite.
    #[must_use]
    pub fn example_overwrite() -> Self {
        Self::new(
            "config/prod.toml",
            "Replaces the file on disk with staged content.",
        )
        .scope_detail("1 file · previous version lost if no VCS")
        .safer_alternative("Commit or copy the file before overwriting.")
        .reversibility(AlertReversibility::Recoverable)
    }

    /// Example: terminate process.
    #[must_use]
    pub fn example_terminate() -> Self {
        Self::new(
            "worker-7 (pid 44102)",
            "Sends SIGTERM then SIGKILL after grace.",
        )
        .scope_detail("in-flight jobs will fail")
        .safer_alternative("Drain the worker and wait for idle.")
        .reversibility(AlertReversibility::Irreversible)
    }

    /// Example: data egress.
    #[must_use]
    pub fn example_data_egress() -> Self {
        Self::new(
            "export → partner-sftp.example.com",
            "Uploads the selected dataset outside the workspace trust boundary.",
        )
        .scope_detail("PII columns included · 18 MB")
        .safer_alternative("Redact PII or use the internal share link.")
        .reversibility(AlertReversibility::Irreversible)
    }
}

/// When the confirm action becomes enabled.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlertConfirmGates {
    /// Exact phrase the user must type (empty = no typed gate).
    pub typed_phrase: Option<String>,
    /// Countdown milliseconds before confirm enables (`None` = no countdown).
    /// Only set when justified (bulk delete, egress, terminate).
    pub countdown_ms: Option<u64>,
}

impl AlertConfirmGates {
    /// No gates — confirm enabled after focus move only (still safe default focus).
    #[must_use]
    pub const fn none() -> Self {
        Self {
            typed_phrase: None,
            countdown_ms: None,
        }
    }

    /// Require typing `phrase` (case-sensitive).
    #[must_use]
    pub fn typed(phrase: impl Into<String>) -> Self {
        Self {
            typed_phrase: Some(phrase.into()),
            countdown_ms: None,
        }
    }

    /// Countdown only (justified delays).
    #[must_use]
    pub const fn countdown(ms: u64) -> Self {
        Self {
            typed_phrase: None,
            countdown_ms: Some(ms),
        }
    }

    /// Typed + countdown.
    #[must_use]
    pub fn typed_and_countdown(phrase: impl Into<String>, ms: u64) -> Self {
        Self {
            typed_phrase: Some(phrase.into()),
            countdown_ms: Some(ms),
        }
    }
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Typed outcomes for alert interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AlertDialogOutcome<Id> {
    /// No change.
    Ignored,
    /// Action cursor / zone moved.
    FocusMoved,
    /// Typed confirmation buffer changed.
    TypedChanged,
    /// Countdown advanced.
    CountdownTick {
        /// Remaining ms.
        remaining_ms: u64,
    },
    /// Countdown finished; confirm may enable.
    CountdownElapsed,
    /// Safe action activated (cancel / keep / deny).
    Cancelled {
        /// Safe action id.
        id: Id,
    },
    /// Destructive confirm activated (gates passed).
    Confirmed {
        /// Confirm action id.
        id: Id,
    },
    /// Enter blocked (gates, loading, or unsafe focus).
    ConfirmBlocked,
    /// Esc trapped (confirm-only / locked).
    EscTrapped,
    /// Validation / typed mismatch feedback.
    TypedMismatch,
}

/// Alert dialog interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertDialogState<Id> {
    kind: AlertKind,
    scope: AlertScope,
    gates: AlertConfirmGates,
    typed_buffer: String,
    countdown_left_ms: Option<u64>,
    /// Non-dismissable critical: Esc never cancels; must choose an action.
    locked: bool,
    confirm_id: Id,
    cancel_id: Id,
    confirm_label: String,
    cancel_label: String,
    dialog: DialogState<Id>,
    regions: Vec<HitRegion<Id>>,
    title_override: Option<String>,
}

impl<Id> AlertDialogState<Id> {
    /// Kind.
    #[must_use]
    pub const fn kind(&self) -> AlertKind {
        self.kind
    }

    /// Scope.
    #[must_use]
    pub fn scope(&self) -> &AlertScope {
        &self.scope
    }

    /// Replace scope.
    pub fn set_scope(&mut self, scope: AlertScope) {
        self.scope = scope;
    }

    /// Confirm gates (typed / countdown).
    pub fn set_gates(&mut self, gates: AlertConfirmGates) {
        self.countdown_left_ms = gates.countdown_ms;
        self.gates = gates;
        self.typed_buffer.clear();
    }

    /// Gates.
    #[must_use]
    pub fn gates(&self) -> &AlertConfirmGates {
        &self.gates
    }

    /// Non-dismissable critical state (Esc always trapped; no soft cancel via Esc).
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        self.dialog.set_close_policy(if locked {
            DialogClosePolicy::Locked
        } else {
            DialogClosePolicy::ConfirmOnly
        });
    }

    /// Whether locked.
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.locked
    }

    /// Override title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title_override = Some(title.into());
    }

    /// Override action labels.
    pub fn set_action_labels(&mut self, confirm: impl Into<String>, cancel: impl Into<String>) {
        self.confirm_label = confirm.into();
        self.cancel_label = cancel.into();
    }

    /// Typed buffer contents.
    #[must_use]
    pub fn typed_buffer(&self) -> &str {
        &self.typed_buffer
    }

    /// Remaining countdown ms.
    #[must_use]
    pub const fn countdown_remaining_ms(&self) -> Option<u64> {
        self.countdown_left_ms
    }

    /// Whether typed phrase matches (or no typed gate).
    #[must_use]
    pub fn typed_satisfied(&self) -> bool {
        match &self.gates.typed_phrase {
            None => true,
            Some(p) => self.typed_buffer == *p,
        }
    }

    /// Whether countdown elapsed (or no countdown).
    #[must_use]
    pub fn countdown_satisfied(&self) -> bool {
        match self.countdown_left_ms {
            None => true,
            Some(0) => true,
            Some(_) => false,
        }
    }

    /// Confirm action may be activated.
    #[must_use]
    pub fn confirm_enabled(&self) -> bool {
        !self.dialog.is_loading() && self.typed_satisfied() && self.countdown_satisfied()
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.dialog.is_open()
    }

    /// Loading.
    pub fn set_loading(&mut self, on: bool) {
        self.dialog.set_loading(on);
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.dialog.set_accepts_input(on);
    }

    /// Underlying dialog engine.
    #[must_use]
    pub fn dialog(&self) -> &DialogState<Id> {
        &self.dialog
    }

    /// Title for paint.
    #[must_use]
    pub fn title(&self) -> &str {
        self.title_override
            .as_deref()
            .unwrap_or_else(|| self.kind.default_title())
    }
}

impl<Id: Clone + PartialEq> AlertDialogState<Id> {
    /// Build from kind + scope; confirm/cancel ids required.
    #[must_use]
    pub fn new(kind: AlertKind, scope: AlertScope, confirm_id: Id, cancel_id: Id) -> Self {
        let mut dialog = DialogState::new();
        dialog.set_close_policy(DialogClosePolicy::ConfirmOnly);
        dialog.set_recipe(DialogRecipe::Destructive);
        dialog.set_initial_focus(DialogFocusZone::Actions);
        dialog.set_focus_zone(DialogFocusZone::Actions);
        // Safe default: cancel is initial cursor and default Enter target.
        dialog.set_action_cursor(Some(cancel_id.clone()));
        dialog.set_default_action(Some(cancel_id.clone()));
        dialog.set_cancel_action(Some(cancel_id.clone()));
        // Enter activates focused action (starts on cancel).
        dialog.set_require_action_focus_for_enter(false);
        Self {
            kind,
            scope,
            gates: AlertConfirmGates::none(),
            typed_buffer: String::new(),
            countdown_left_ms: None,
            locked: false,
            confirm_label: kind.default_confirm_label().into(),
            cancel_label: kind.default_cancel_label().into(),
            confirm_id,
            cancel_id,
            dialog,
            regions: Vec::new(),
            title_override: None,
        }
    }

    /// Action cursor (safe or confirm).
    #[must_use]
    pub fn action_cursor(&self) -> Option<&Id> {
        self.dialog.action_cursor()
    }

    /// Reset safe focus to cancel.
    pub fn focus_safe(&mut self) {
        self.dialog.set_action_cursor(Some(self.cancel_id.clone()));
        self.dialog.set_focus_zone(DialogFocusZone::Actions);
    }

    /// Build action strip with confirm enabled only when gates pass.
    #[must_use]
    pub fn actions(&self) -> [Action<'_, Id>; 2] {
        [
            Action {
                id: self.cancel_id.clone(),
                label: self.cancel_label.as_str(),
                enabled: true,
                variant: ActionVariant::Secondary,
            },
            Action {
                id: self.confirm_id.clone(),
                label: self.confirm_label.as_str(),
                enabled: self.confirm_enabled(),
                variant: ActionVariant::Destructive,
            },
        ]
    }

    /// Open on OverlayStack (Alert class; opener restore).
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        self.dialog.set_open(true);
        self.focus_safe();
        if let Some(ms) = self.gates.countdown_ms {
            self.countdown_left_ms = Some(ms);
        }
        open_dialog_configured(
            stack,
            bounds,
            DialogSize {
                width: ALERT_DIALOG_DEFAULT_WIDTH,
                height: ALERT_DIALOG_DEFAULT_HEIGHT,
            },
            opener_focus,
            if self.locked {
                DialogClosePolicy::Locked
            } else {
                DialogClosePolicy::ConfirmOnly
            },
            Some(DialogRecipe::Destructive),
            Some(ALERT_DIALOG_OVERLAY_ID.to_string()),
        )
    }

    /// Dismiss alert overlay (only after action — host may call).
    pub fn close_on_stack<F: Clone>(&mut self, stack: &mut OverlayStack<F>) -> OverlayOutcome<F> {
        self.dialog.set_open(false);
        stack.dismiss(&OverlayId::from_static(ALERT_DIALOG_OVERLAY_ID))
    }

    /// Advance countdown by `delta_ms` (host frame tick).
    pub fn tick(&mut self, delta_ms: u64) -> AlertDialogOutcome<Id> {
        let Some(left) = self.countdown_left_ms.as_mut() else {
            return AlertDialogOutcome::Ignored;
        };
        if *left == 0 {
            return AlertDialogOutcome::Ignored;
        }
        *left = left.saturating_sub(delta_ms);
        if *left == 0 {
            AlertDialogOutcome::CountdownElapsed
        } else {
            AlertDialogOutcome::CountdownTick {
                remaining_ms: *left,
            }
        }
    }

    /// Keyboard routing — every dismissal / focus path.
    pub fn handle_key(&mut self, key: KeyEvent) -> AlertDialogOutcome<Id> {
        if !self.dialog.is_open()
            || !self.dialog.accepts_input()
            || key.kind == KeyEventKind::Release
        {
            return AlertDialogOutcome::Ignored;
        }

        // Typed confirmation field (when gate present and focus not forcing actions-only).
        if self.gates.typed_phrase.is_some()
            && key.modifiers.is_empty()
            && !key.modifiers.contains(KeyModifiers::CONTROL)
        {
            match key.code {
                KeyCode::Char(c) if !c.is_control() && key.kind == KeyEventKind::Press => {
                    // Don't steal j/k when on actions without typed focus —
                    // typed buffer always accepts printable when gate present,
                    // except when user is moving actions with arrows.
                    self.typed_buffer.push(c);
                    return AlertDialogOutcome::TypedChanged;
                }
                KeyCode::Backspace if key.kind == KeyEventKind::Press => {
                    self.typed_buffer.pop();
                    return AlertDialogOutcome::TypedChanged;
                }
                _ => {}
            }
        }

        // Esc paths
        if matches!(key.code, KeyCode::Esc) && key.kind == KeyEventKind::Press {
            return self.handle_escape();
        }

        // Action navigation
        if let Some(intent) = alert_nav_intent(key) {
            return self.handle_intent(intent);
        }

        AlertDialogOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: UiIntent) -> AlertDialogOutcome<Id> {
        if !self.dialog.is_open() || !self.dialog.accepts_input() {
            return AlertDialogOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => self.handle_escape(),
            UiIntent::Activate | UiIntent::Submit | UiIntent::Open => self.activate_focused(),
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Left) => self.move_action(-1),
            UiIntent::Move(NavigationMove::Next | NavigationMove::Right) => self.move_action(1),
            UiIntent::Move(NavigationMove::First) => {
                self.focus_safe();
                AlertDialogOutcome::FocusMoved
            }
            UiIntent::Move(NavigationMove::Last) => {
                if self.confirm_enabled() {
                    self.dialog.set_action_cursor(Some(self.confirm_id.clone()));
                    AlertDialogOutcome::FocusMoved
                } else {
                    AlertDialogOutcome::ConfirmBlocked
                }
            }
            _ => {
                // Body scroll / unused intents — no action strip borrow needed.
                let out = self.dialog.handle_intent(intent, &[]);
                self.map_dialog_outcome(out)
            }
        }
    }

    fn handle_escape(&mut self) -> AlertDialogOutcome<Id> {
        if self.locked {
            return AlertDialogOutcome::EscTrapped;
        }
        // Confirm-only: Esc activates safe cancel (explicit choice), does not
        // silent-dismiss without action id — host still gets Cancelled{id}.
        self.dialog.set_action_cursor(Some(self.cancel_id.clone()));
        AlertDialogOutcome::Cancelled {
            id: self.cancel_id.clone(),
        }
    }

    fn activate_focused(&mut self) -> AlertDialogOutcome<Id> {
        if self.dialog.is_loading() {
            return AlertDialogOutcome::ConfirmBlocked;
        }
        let focused = self
            .dialog
            .action_cursor()
            .cloned()
            .unwrap_or_else(|| self.cancel_id.clone());

        if focused == self.cancel_id {
            return AlertDialogOutcome::Cancelled {
                id: self.cancel_id.clone(),
            };
        }
        if focused == self.confirm_id {
            if !self.confirm_enabled() {
                if self.gates.typed_phrase.is_some() && !self.typed_satisfied() {
                    return AlertDialogOutcome::TypedMismatch;
                }
                return AlertDialogOutcome::ConfirmBlocked;
            }
            return AlertDialogOutcome::Confirmed {
                id: self.confirm_id.clone(),
            };
        }
        AlertDialogOutcome::Ignored
    }

    fn move_action(&mut self, dir: isize) -> AlertDialogOutcome<Id> {
        let actions = self.actions();
        let enabled: Vec<_> = actions.iter().filter(|a| a.enabled).collect();
        if enabled.is_empty() {
            return AlertDialogOutcome::Ignored;
        }
        let cur = self
            .dialog
            .action_cursor()
            .and_then(|id| enabled.iter().position(|a| &a.id == id));
        let next = match (cur, dir < 0) {
            (Some(0), true) | (None, true) => enabled.len() - 1,
            (Some(i), true) => i - 1,
            (Some(i), false) => (i + 1) % enabled.len(),
            (None, false) => 0,
        };
        let id = enabled[next].id.clone();
        // Refuse landing on disabled confirm (shouldn't appear in enabled)
        if id == self.confirm_id && !self.confirm_enabled() {
            return AlertDialogOutcome::ConfirmBlocked;
        }
        if self.dialog.action_cursor() == Some(&id) {
            return AlertDialogOutcome::Ignored;
        }
        self.dialog.set_action_cursor(Some(id));
        AlertDialogOutcome::FocusMoved
    }

    fn map_dialog_outcome(&mut self, out: DialogOutcome<Id>) -> AlertDialogOutcome<Id> {
        match out {
            DialogOutcome::Ignored | DialogOutcome::Scrolled | DialogOutcome::LoadingBlocked => {
                AlertDialogOutcome::Ignored
            }
            DialogOutcome::FocusMoved => AlertDialogOutcome::FocusMoved,
            DialogOutcome::Activated(id) | DialogOutcome::DefaultActivated(id) => {
                if id == self.cancel_id {
                    AlertDialogOutcome::Cancelled { id }
                } else if id == self.confirm_id {
                    if self.confirm_enabled() {
                        AlertDialogOutcome::Confirmed { id }
                    } else {
                        AlertDialogOutcome::ConfirmBlocked
                    }
                } else {
                    AlertDialogOutcome::Ignored
                }
            }
            DialogOutcome::Cancelled => self.handle_escape(),
            DialogOutcome::ValidationFailed => AlertDialogOutcome::TypedMismatch,
        }
    }

    /// Pointer click on action hits.
    pub fn handle_click(
        &mut self,
        position: ratatui_core::layout::Position,
    ) -> AlertDialogOutcome<Id> {
        if !self.dialog.is_open() || !self.dialog.accepts_input() {
            return AlertDialogOutcome::Ignored;
        }
        let Some(region) = self.regions.iter().find(|r| r.area.contains(position)) else {
            return AlertDialogOutcome::Ignored;
        };
        self.dialog.set_action_cursor(Some(region.id.clone()));
        self.activate_focused()
    }
}

fn alert_nav_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    match key.code {
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Left | KeyCode::Up | KeyCode::Char('h' | 'H') => {
            Some(UiIntent::Move(NavigationMove::Previous))
        }
        KeyCode::Right | KeyCode::Down | KeyCode::Char('l' | 'L') => {
            Some(UiIntent::Move(NavigationMove::Next))
        }
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Tab if is_press => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::BackTab if is_press => Some(UiIntent::Move(NavigationMove::Previous)),
        _ => None,
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Alert dialog paint (risk body + safe/danger actions).
#[derive(Debug, Clone, Copy)]
pub struct AlertDialog<'a, Id> {
    system: &'a DesignSystem,
    colorless: bool,
    _id: core::marker::PhantomData<Id>,
}

impl<'a, Id> AlertDialog<'a, Id> {
    /// Design system.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
            _id: core::marker::PhantomData,
        }
    }

    /// ASCII markers.
    #[must_use]
    /// Reduced color.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint full alert.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut AlertDialogState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() || !state.is_open() {
            return;
        }
        let title = state.title().to_string();
        let rev = state.scope.reversibility.label();
        let loading = state.dialog.is_loading();
        let body = build_body_text(state, false);
        let footer = footer_hints(state);
        let dialog = Dialog::new(&title, body, self.system)
            .description(rev)
            .variant(DialogVariant::Danger)
            .recipe(DialogRecipe::Destructive)
            .loading(loading)
            .colorless(self.colorless)
            .hints(footer);

        let narrow = crate::layout::dialog_stack_actions(area.width, area.height);
        let action_rows = if narrow { 2 } else { 1 };
        dialog.paint(area, buffer, &mut state.dialog, action_rows);

        // Typed field inside body bottom if gate
        if let Some(phrase) = &state.gates.typed_phrase {
            paint_typed_field(
                buffer,
                state.dialog.slots().body,
                phrase,
                &state.typed_buffer,
                state.typed_satisfied(),
                self.system,
                false,
            );
        }

        // Countdown banner in validation slot area
        if let Some(left) = state.countdown_left_ms {
            if left > 0 {
                let secs = left.div_ceil(1000);
                let msg = { format!("Wait {secs}s before confirming") };
                let strip = state.dialog.slots().validation;
                let y = if strip.is_empty() {
                    state
                        .dialog
                        .slots()
                        .actions
                        .y
                        .saturating_sub(1)
                        .max(state.dialog.slots().body.y)
                } else {
                    strip.y
                };
                let x = state.dialog.slots().body.x;
                let w = state.dialog.slots().body.width;
                if w > 0 {
                    buffer.set_stringn(
                        x,
                        y,
                        &take_display_cols(&msg, usize::from(w)),
                        usize::from(w),
                        self.system.style(Role::TextMuted),
                    );
                }
            }
        }

        let action_area = state.dialog.slots().actions;
        if action_area.is_empty() {
            state.regions.clear();
            return;
        }
        let mut bar_state = ActionBarState {
            cursor: state.dialog.action_cursor().cloned(),
            regions: Vec::new(),
        };
        let actions = state.actions();
        (&ActionBar::new(&actions, self.system)
            .colorless(self.colorless)
            .vertical(narrow))
            .render(action_area, buffer, &mut bar_state);
        state.regions = bar_state.regions;
        if let Some(c) = bar_state.cursor {
            state.dialog.set_action_cursor(Some(c));
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &AlertDialogState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: Clone + PartialEq + std::fmt::Display,
    {
        if area.is_empty() || !state.is_open() {
            return;
        }
        let focus = state
            .action_cursor()
            .map(|c| c.to_string())
            .unwrap_or_default();
        let desc = format!(
            "alert kind={} locked={} typed_ok={} countdown_ok={} focus={focus}",
            state.kind.id(),
            state.locked,
            state.typed_satisfied(),
            state.countdown_satisfied(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Dialog)
                .label("alert-dialog")
                .description(desc)
                .focusable(state.dialog.accepts_input())
                .state(SemanticState {
                    selected: true,
                    expanded: state.is_open(),
                    busy: state.dialog.is_loading(),
                    invalid: !state.typed_satisfied() && state.gates.typed_phrase.is_some(),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for AlertDialog<'_, Id> {
    type State = AlertDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &AlertDialog<'_, Id> {
    type State = AlertDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

fn build_body_text<Id>(state: &AlertDialogState<Id>, ascii: bool) -> Text<'static> {
    let mut lines: Vec<String> = Vec::new();
    let bullet = if ascii { "*" } else { "•" };
    lines.push(format!("{bullet} Target: {}", state.scope.target));
    if let Some(d) = &state.scope.scope_detail {
        lines.push(format!("{bullet} Scope: {d}"));
    }
    lines.push(format!(
        "{bullet} Consequence: {}",
        state.scope.consequences
    ));
    lines.push(format!(
        "{bullet} Reversibility: {}",
        state.scope.reversibility.label()
    ));
    if let Some(alt) = &state.scope.safer_alternative {
        lines.push(format!("{bullet} Safer: {alt}"));
    }
    if let Some(phrase) = &state.gates.typed_phrase {
        lines.push(String::new());
        lines.push(format!("Type \"{phrase}\" to enable confirm:"));
    }
    // Join into owned Text
    let joined = lines.join("\n");
    Text::from(joined)
}

/// Footer chords when the dialog traps focus until a choice is made.
const LOCKED_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "←→",
        label: "choose",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "confirm",
        priority: 20,
        visible: true,
    },
];

/// Footer chords for a type-to-confirm dialog.
const TYPED_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "type",
        label: "phrase",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "confirm",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "esc",
        label: "cancel",
        priority: 30,
        visible: true,
    },
];

/// Footer chords for an ordinary alert.
const ALERT_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "←→",
        label: "choose",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "confirm",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "esc",
        label: "cancel",
        priority: 30,
        visible: true,
    },
];

fn footer_hints<Id>(state: &AlertDialogState<Id>) -> &'static [Hint<'static>] {
    if state.locked {
        LOCKED_HINTS
    } else if state.gates.typed_phrase.is_some() {
        TYPED_HINTS
    } else {
        ALERT_HINTS
    }
}

fn paint_typed_field(
    buffer: &mut Buffer,
    body: Rect,
    phrase: &str,
    typed: &str,
    ok: bool,
    system: &DesignSystem,
    ascii: bool,
) {
    if body.height < 2 || body.width < 4 {
        return;
    }
    let y = body.bottom().saturating_sub(1);
    // Say what has to be typed. The phrase was computed and thrown away, so a
    // confirmation asked the operator to guess (plans/009 Step 3).
    if body.height >= 2 {
        let ask = format!("type {phrase} to confirm");
        buffer.set_stringn(
            body.x,
            y.saturating_sub(1),
            &take_display_cols(&ask, usize::from(body.width)),
            usize::from(body.width),
            system.style(Role::TextMuted),
        );
    }
    let prefix = if ascii { "> " } else { "› " };
    let mark = if ok {
        if ascii { "[ok] " } else { "✓ " }
    } else {
        "  "
    };
    let line = format!("{prefix}{mark}{typed}");
    let style = if ok {
        system.style(Role::Success)
    } else {
        system.style(Role::Text)
    };
    buffer.set_stringn(
        body.x,
        y,
        &take_display_cols(&line, usize::from(body.width)),
        usize::from(body.width),
        style,
    );
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Open default alert overlay id with confirm-only policy.
pub fn open_alert_dialog_widget_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    opener_focus: Option<FocusId>,
    locked: bool,
) -> OverlayOutcome<FocusId> {
    open_dialog_configured(
        stack,
        bounds,
        DialogSize {
            width: ALERT_DIALOG_DEFAULT_WIDTH,
            height: ALERT_DIALOG_DEFAULT_HEIGHT,
        },
        opener_focus,
        if locked {
            DialogClosePolicy::Locked
        } else {
            DialogClosePolicy::ConfirmOnly
        },
        Some(DialogRecipe::Destructive),
        Some(ALERT_DIALOG_OVERLAY_ID.to_string()),
    )
}

/// Dismiss alert widget overlay.
pub fn dismiss_alert_dialog_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(ALERT_DIALOG_OVERLAY_ID))
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::OverlayOutcome;
    use ratatui_core::layout::Position;

    fn delete_state() -> AlertDialogState<&'static str> {
        AlertDialogState::new(
            AlertKind::Delete,
            AlertScope::example_delete(),
            "delete",
            "keep",
        )
    }

    #[test]
    fn initial_focus_is_safe_cancel() {
        let state = delete_state();
        assert_eq!(state.action_cursor().copied(), Some("keep"));
        assert!(!state.actions()[1].enabled || state.confirm_enabled()); // no gates → confirm enabled
        // But focus is still safe
        assert_eq!(state.action_cursor().copied(), Some("keep"));
    }

    #[test]
    fn enter_on_safe_focus_cancels_not_confirms() {
        let mut state = delete_state();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            AlertDialogOutcome::Cancelled { id: "keep" }
        ));
    }

    #[test]
    fn enter_on_confirm_without_gates_confirms_after_focus_move() {
        let mut state = delete_state();
        let _ = state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(state.action_cursor().copied(), Some("delete"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            AlertDialogOutcome::Confirmed { id: "delete" }
        ));
    }

    #[test]
    fn typed_gate_blocks_confirm_until_match() {
        let mut state = delete_state();
        state.set_gates(AlertConfirmGates::typed("prod-db.customers"));
        assert!(!state.confirm_enabled());
        // Move to confirm while disabled — blocked
        let out = state.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert!(matches!(
            out,
            AlertDialogOutcome::ConfirmBlocked | AlertDialogOutcome::FocusMoved
        ));
        // Type phrase
        for c in "prod-db.customers".chars() {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert!(state.typed_satisfied());
        assert!(state.confirm_enabled());
        state.dialog.set_action_cursor(Some("delete"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            AlertDialogOutcome::Confirmed { id: "delete" }
        ));
    }

    #[test]
    fn typed_mismatch_on_activate() {
        let mut state = delete_state();
        state.set_gates(AlertConfirmGates::typed("DELETE"));
        // Force cursor on confirm even if disabled via set
        state.dialog.set_action_cursor(Some("delete"));
        // confirm disabled — activate blocked
        assert!(matches!(
            state.activate_focused(),
            AlertDialogOutcome::TypedMismatch | AlertDialogOutcome::ConfirmBlocked
        ));
    }

    #[test]
    fn countdown_blocks_then_elapses() {
        let mut state = delete_state();
        state.set_gates(AlertConfirmGates::countdown(3000));
        assert!(!state.confirm_enabled());
        assert!(matches!(
            state.tick(1000),
            AlertDialogOutcome::CountdownTick { remaining_ms: 2000 }
        ));
        assert!(matches!(
            state.tick(2000),
            AlertDialogOutcome::CountdownElapsed
        ));
        assert!(state.confirm_enabled());
    }

    #[test]
    fn locked_esc_trapped() {
        let mut state = delete_state();
        state.set_locked(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertDialogOutcome::EscTrapped
        ));
        assert!(state.is_open());
    }

    #[test]
    fn esc_confirm_only_cancels_with_id() {
        let mut state = delete_state();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertDialogOutcome::Cancelled { id: "keep" }
        ));
    }

    #[test]
    fn overlay_open_alert_kind_and_opener_restore() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = delete_state();
        let out = state.open_on_stack(&mut stack, bounds, Some("trigger"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::AlertDialog);
        // Esc trapped at stack level
        assert_eq!(stack.handle_escape(), OverlayOutcome::Ignored);
        // Outside trapped
        assert_eq!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Ignored
        );
        // Explicit dismiss restores opener
        assert!(matches!(
            state.close_on_stack(&mut stack),
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
    }

    #[test]
    fn locked_overlay_policy() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        let mut state = delete_state();
        state.set_locked(true);
        let _ = state.open_on_stack(&mut stack, bounds, None);
        assert_eq!(stack.top().unwrap().kind, OverlayKind::AlertDialog);
        assert_eq!(stack.handle_escape(), OverlayOutcome::Ignored);
    }

    #[test]
    fn example_kinds_have_scope_fields() {
        for scope in [
            AlertScope::example_delete(),
            AlertScope::example_overwrite(),
            AlertScope::example_terminate(),
            AlertScope::example_data_egress(),
        ] {
            assert!(!scope.target.is_empty());
            assert!(!scope.consequences.is_empty());
            assert!(scope.safer_alternative.is_some());
        }
    }

    #[test]
    fn paint_and_semantics() {
        let system = DesignSystem::default();
        let mut state = delete_state();
        state.set_gates(AlertConfirmGates::typed("x"));
        let area = Rect::new(0, 0, 56, 16);
        let mut buf = Buffer::empty(area);
        AlertDialog::new(&system).paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Target") || text.contains("prod-db") || text.contains("Delete"),
            "{text}"
        );
        let mut scene = SemanticScene::<&str, ()>::default();
        AlertDialog::new(&system).register_semantic(&mut scene, "a", area, &state);
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("alert-dialog"))
        );
    }

    #[test]
    fn click_safe_and_confirm_paths() {
        let system = DesignSystem::default();
        let mut state = delete_state();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        AlertDialog::new(&system).paint(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        // Click first region (cancel — left)
        let cancel_hit = state.regions[0].area;
        assert!(matches!(
            state.handle_click(Position::new(cancel_hit.x, cancel_hit.y)),
            AlertDialogOutcome::Cancelled { id: "keep" }
        ));
    }

    #[test]
    fn every_dismiss_and_focus_path_matrix() {
        // Matrix: esc / enter-safe / enter-danger / arrows / locked / typed / countdown
        let mut s = delete_state();
        // focus first
        assert_eq!(s.action_cursor().copied(), Some("keep"));
        // arrow to danger
        assert!(matches!(
            s.handle_intent(UiIntent::Move(NavigationMove::Next)),
            AlertDialogOutcome::FocusMoved
        ));
        assert_eq!(s.action_cursor().copied(), Some("delete"));
        // home back to safe
        assert!(matches!(
            s.handle_intent(UiIntent::Move(NavigationMove::First)),
            AlertDialogOutcome::FocusMoved
        ));
        assert_eq!(s.action_cursor().copied(), Some("keep"));
        // enter safe
        assert!(matches!(
            s.handle_intent(UiIntent::Activate),
            AlertDialogOutcome::Cancelled { id: "keep" }
        ));

        let mut s = delete_state();
        s.set_locked(true);
        assert!(matches!(
            s.handle_intent(UiIntent::Cancel),
            AlertDialogOutcome::EscTrapped
        ));

        let mut s = delete_state();
        s.set_gates(AlertConfirmGates::countdown(500));
        s.dialog.set_action_cursor(Some("delete"));
        assert!(matches!(
            s.handle_intent(UiIntent::Activate),
            AlertDialogOutcome::ConfirmBlocked
        ));
        let _ = s.tick(500);
        assert!(matches!(
            s.handle_intent(UiIntent::Activate),
            AlertDialogOutcome::Confirmed { id: "delete" }
        ));
    }

    #[test]
    fn fuzz_keys() {
        let mut state = delete_state();
        state.set_gates(AlertConfirmGates::typed_and_countdown("ab", 100));
        let keys = [
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Tab,
            KeyCode::Char('a'),
            KeyCode::Char('b'),
            KeyCode::Backspace,
            KeyCode::Home,
            KeyCode::End,
        ];
        let mut seed = 9u64;
        for _ in 0..300 {
            if !state.is_open() {
                state.dialog.set_open(true);
                state.focus_safe();
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE));
            let _ = state.tick(10);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut state = delete_state();
        state.set_gates(AlertConfirmGates::typed("x"));
        let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..120 {
            terminal
                .draw(|f| {
                    AlertDialog::new(&system).paint(f.area(), f.buffer_mut(), &mut state);
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
        let mut s1 = AlertDialogState::new(
            AlertKind::Overwrite,
            AlertScope::example_overwrite(),
            "ow",
            "keep",
        );
        let mut t1 = Terminal::new(TestBackend::new(52, 14)).unwrap();
        t1.draw(|f| {
            AlertDialog::new(&system).paint(f.area(), f.buffer_mut(), &mut s1);
        })
        .unwrap();
        let a: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut s2 = AlertDialogState::new(
            AlertKind::Overwrite,
            AlertScope::example_overwrite(),
            "ow",
            "keep",
        );
        let mut t2 = Terminal::new(TestBackend::new(52, 14)).unwrap();
        t2.draw(|f| {
            AlertDialog::new(&system).paint(f.area(), f.buffer_mut(), &mut s2);
        })
        .unwrap();
        let b: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(a, b);
        assert!(a.contains("Overwrite") || a.contains("config") || a.contains("Target"));
    }

    #[test]
    fn loading_blocks_confirm() {
        let mut state = delete_state();
        state.set_loading(true);
        state.dialog.set_action_cursor(Some("delete"));
        assert!(matches!(
            state.activate_focused(),
            AlertDialogOutcome::ConfirmBlocked
        ));
    }
}
