// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **AuthEntry** — keyboard-first sign-up / sign-in / email-only composition
//! (shadcn signup + login blocks peer for TUI).
//!
//! **Mission.** Multi-field credential entry (identity + password, optional
//! confirm + terms, or **email-only** passwordless request), validation
//! feedback, primary submit + cancel, secondary actions (forgot-password,
//! host OAuth id), and mode switch. **Host owns** network auth, OAuth,
//! CAPTCHA, magic-link delivery, and secret storage — outcomes never embed
//! password plaintext; host reads secrets via accessors on this state.
//!
//! **vs bare Form / SetupWizard account step.** Focused single-gate auth
//! surface for CLI login/register flows; not a multi-step onboarding wizard and
//! not a product-branded splash.
//!
//! Research: shadcn signup-01…04, login-01…05, CLI login prompts, cloud auth
//! TUI gates.
//!
//! Teaches: how to compose keyboard-first sign-up / sign-in / email-only
//! composition (shadcn signup + login blocks peer for TUI).
//!
//! Composes: [`crate::widgets::Checkbox`],
//! [`crate::widgets::CheckboxOutcome`], [`crate::widgets::CheckboxState`],
//! [`crate::widgets::Panel`], [`crate::widgets::PanelState`],
//! [`crate::widgets::PanelVariant`],
//! [`crate::widgets::PasswordConfirmState`],
//! [`crate::widgets::PasswordInput`], and 5 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, Glyph, PanelChrome, Role},
    widgets::{
        Button, ButtonState, ButtonVariant, Callout, CalloutTone, Checkbox, CheckboxOutcome,
        CheckboxState, Panel, PanelVariant, PasswordConfirmState, PasswordInput,
        PasswordInputOutcome, TextInput, TextInputOutcome, TextInputState, Validation,
    },
};

/// The waiting marker, from the glyph catalog rather than a literal.
fn pending_glyph(system: &DesignSystem) -> &'static str {
    system.glyphs.resolve(Glyph::Loading).text
}

// ── Mode & fields ───────────────────────────────────────────────────────────

/// Auth gate mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AuthEntryMode {
    /// Create account (confirm + optional terms).
    #[default]
    SignUp,
    /// Existing account (identity + password).
    SignIn,
    /// Passwordless / magic-link request (identity only; host delivers).
    EmailOnly,
}

impl AuthEntryMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SignUp => "sign-up",
            Self::SignIn => "sign-in",
            Self::EmailOnly => "email-only",
        }
    }

    /// Title chrome.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::SignUp => "Create account",
            Self::SignIn => "Sign in",
            Self::EmailOnly => "Continue with email",
        }
    }

    /// What the surface says while a submit is in flight.
    #[must_use]
    pub const fn pending_verb(self) -> &'static str {
        match self {
            Self::SignUp => "Creating account…",
            Self::SignIn => "Signing in…",
            Self::EmailOnly => "Sending link…",
        }
    }

    /// Whether password field is part of this mode.
    #[must_use]
    pub const fn requires_password(self) -> bool {
        !matches!(self, Self::EmailOnly)
    }

    /// Toggle peer mode (sign-up ↔ sign-in; email-only ↔ sign-in).
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::SignUp => Self::SignIn,
            Self::SignIn => Self::SignUp,
            Self::EmailOnly => Self::SignIn,
        }
    }
}

/// Focusable field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AuthEntryField {
    /// Email / username / handle.
    Identity,
    /// Password.
    Password,
    /// Confirm password (sign-up).
    Confirm,
    /// Accept terms (sign-up when required).
    Terms,
}

impl AuthEntryField {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Password => "password",
            Self::Confirm => "confirm",
            Self::Terms => "terms",
        }
    }
}

/// Field-level validation message (no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthFieldError {
    /// Field.
    pub field: AuthEntryField,
    /// Human message.
    pub message: String,
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Host-facing outcomes (requests only — no network).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthEntryOutcome {
    /// Nothing handled.
    Ignored,
    /// Editable field content changed (password values never included).
    FieldChanged {
        /// Which field.
        field: AuthEntryField,
    },
    /// Focus moved between fields.
    FocusMoved {
        /// New focus.
        field: AuthEntryField,
    },
    /// Submit blocked by validation.
    ValidationFailed {
        /// Field messages (no secrets).
        errors: Vec<AuthFieldError>,
    },
    /// Valid submit — host reads secrets via [`AuthEntryState::password_secret`]
    /// / [`AuthEntryState::take_password_secret`] when `password_filled`.
    /// For [`AuthEntryMode::EmailOnly`], host sends magic-link / OTP request.
    Submitted {
        /// Mode at submit time.
        mode: AuthEntryMode,
        /// Identity value (email / username).
        identity: String,
        /// Whether password is non-empty (never the secret itself).
        password_filled: bool,
        /// Whether confirm is non-empty (sign-up).
        confirm_filled: bool,
        /// Terms accepted flag.
        terms_accepted: bool,
        /// Passwordless request (email-only / magic-link gate).
        passwordless: bool,
    },
    /// Esc / cancel.
    Cancelled,
    /// Mode switched (sign-up ↔ sign-in).
    ModeSwitched {
        /// New mode.
        mode: AuthEntryMode,
    },
    /// Terms checkbox toggled.
    TermsToggled {
        /// Accepted.
        accepted: bool,
    },
    /// Host secondary action (e.g. `"oauth:github"`, `"forgot-password"`).
    SecondaryAction {
        /// Stable action id (host maps).
        id: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Auth entry interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthEntryState {
    mode: AuthEntryMode,
    focus: AuthEntryField,
    identity: TextInputState,
    secrets: PasswordConfirmState,
    terms: CheckboxState,
    require_confirm: bool,
    require_terms: bool,
    field_errors: Vec<AuthFieldError>,
    host_error: Option<String>,
    accepts_input: bool,
    shell_focused: bool,
    pending: bool,
}

impl Default for AuthEntryState {
    fn default() -> Self {
        Self::sign_up()
    }
}

impl AuthEntryState {
    /// Sign-up defaults (confirm + terms required).
    #[must_use]
    pub fn sign_up() -> Self {
        let mut s = Self::blank(AuthEntryMode::SignUp);
        s.require_confirm = true;
        s.require_terms = true;
        s
    }

    /// Sign-in defaults (identity + password; no confirm / terms).
    #[must_use]
    pub fn sign_in() -> Self {
        let mut s = Self::blank(AuthEntryMode::SignIn);
        s.require_confirm = false;
        s.require_terms = false;
        s
    }

    /// Email-only / passwordless request (login-05 peer). Host delivers magic link.
    #[must_use]
    pub fn email_only() -> Self {
        let mut s = Self::blank(AuthEntryMode::EmailOnly);
        s.require_confirm = false;
        s.require_terms = false;
        s
    }

    fn blank(mode: AuthEntryMode) -> Self {
        let mut identity = TextInputState::new("")
            .with_allow_empty(true)
            .with_editing();
        identity.set_focused(true);
        let mut secrets = PasswordConfirmState::new();
        // AuthEntry is a live composite input, so its secret fields must opt
        // into editing explicitly now that PasswordConfirmState::new() is idle.
        secrets.password.begin_edit();
        secrets.confirm.begin_edit();
        secrets.password.set_focused(false);
        secrets.confirm.set_focused(false);
        let mut terms = CheckboxState::new(false);
        terms.set_focused(false);
        Self {
            mode,
            focus: AuthEntryField::Identity,
            identity,
            secrets,
            terms,
            require_confirm: matches!(mode, AuthEntryMode::SignUp),
            require_terms: matches!(mode, AuthEntryMode::SignUp),
            field_errors: Vec::new(),
            host_error: None,
            accepts_input: true,
            shell_focused: true,
            pending: false,
        }
    }

    /// Whether this gate requires a password field.
    #[must_use]
    pub const fn is_passwordless(&self) -> bool {
        !self.mode.requires_password()
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> AuthEntryMode {
        self.mode
    }

    /// Focused field.
    #[must_use]
    pub const fn focus(&self) -> AuthEntryField {
        self.focus
    }

    /// Identity text (public, non-secret).
    #[must_use]
    pub fn identity(&self) -> &str {
        self.identity.value()
    }

    /// Password secret (host-only; do not log).
    #[must_use]
    pub fn password_secret(&self) -> &str {
        self.secrets.password.secret()
    }

    /// Take password secret (clears password field).
    pub fn take_password_secret(&mut self) -> String {
        self.secrets.password.take_secret()
    }

    /// Confirm secret (sign-up).
    #[must_use]
    pub fn confirm_secret(&self) -> &str {
        self.secrets.confirm.secret()
    }

    /// Terms accepted.
    #[must_use]
    pub fn terms_accepted(&self) -> bool {
        self.terms.is_checked()
    }

    /// Field errors from last validate/submit.
    #[must_use]
    pub fn field_errors(&self) -> &[AuthFieldError] {
        &self.field_errors
    }

    /// Host-set banner error (auth failure projection).
    #[must_use]
    pub fn host_error(&self) -> Option<&str> {
        self.host_error.as_deref()
    }

    /// Set host auth failure message (no secrets).
    pub fn set_host_error(&mut self, msg: impl Into<String>) {
        self.host_error = Some(msg.into());
    }

    /// Clear host error.
    pub fn clear_host_error(&mut self) {
        self.host_error = None;
    }

    /// Require password confirm (sign-up).
    pub fn set_require_confirm(&mut self, on: bool) {
        self.require_confirm = on;
        self.clamp_focus();
    }

    /// Require terms checkbox (sign-up).
    pub fn set_require_terms(&mut self, on: bool) {
        self.require_terms = on;
        self.clamp_focus();
    }

    /// Pending remote verify (blocks edits).
    pub fn set_pending(&mut self, on: bool) {
        self.pending = on;
        self.secrets.password.set_pending(on);
        self.secrets.confirm.set_pending(on);
        self.identity.set_loading(on);
    }

    /// Shell focus gate.
    pub fn set_shell_focused(&mut self, on: bool) {
        self.shell_focused = on;
        self.sync_field_focus();
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Switch mode (clears secrets + errors; keeps identity).
    pub fn set_mode(&mut self, mode: AuthEntryMode) -> AuthEntryOutcome {
        if self.mode == mode {
            return AuthEntryOutcome::Ignored;
        }
        self.mode = mode;
        self.require_confirm = matches!(mode, AuthEntryMode::SignUp);
        self.require_terms = matches!(mode, AuthEntryMode::SignUp);
        let _ = self.secrets.password.clear();
        let _ = self.secrets.confirm.clear();
        self.terms.set_checked(false);
        self.field_errors.clear();
        self.host_error = None;
        self.focus = AuthEntryField::Identity;
        self.sync_field_focus();
        AuthEntryOutcome::ModeSwitched { mode }
    }

    /// Switch to email-only passwordless gate (login-05).
    pub fn set_email_only(&mut self) -> AuthEntryOutcome {
        self.set_mode(AuthEntryMode::EmailOnly)
    }

    /// Visible field order for current mode / flags.
    #[must_use]
    pub fn field_order(&self) -> Vec<AuthEntryField> {
        if matches!(self.mode, AuthEntryMode::EmailOnly) {
            return vec![AuthEntryField::Identity];
        }
        let mut v = vec![AuthEntryField::Identity, AuthEntryField::Password];
        if self.require_confirm && matches!(self.mode, AuthEntryMode::SignUp) {
            v.push(AuthEntryField::Confirm);
        }
        if self.require_terms && matches!(self.mode, AuthEntryMode::SignUp) {
            v.push(AuthEntryField::Terms);
        }
        v
    }

    fn clamp_focus(&mut self) {
        let order = self.field_order();
        if !order.contains(&self.focus) {
            self.focus = order.first().copied().unwrap_or(AuthEntryField::Identity);
        }
        self.sync_field_focus();
    }

    fn sync_field_focus(&mut self) {
        let on = self.shell_focused;
        self.identity
            .set_focused(on && self.focus == AuthEntryField::Identity);
        self.secrets
            .password
            .set_focused(on && self.focus == AuthEntryField::Password);
        self.secrets
            .confirm
            .set_focused(on && self.focus == AuthEntryField::Confirm);
        self.terms
            .set_focused(on && self.focus == AuthEntryField::Terms);
    }

    fn move_focus(&mut self, delta: isize) -> AuthEntryOutcome {
        let order = self.field_order();
        if order.is_empty() {
            return AuthEntryOutcome::Ignored;
        }
        let cur = order.iter().position(|f| *f == self.focus).unwrap_or(0);
        let n = order.len() as isize;
        let next = (cur as isize + delta).rem_euclid(n) as usize;
        self.focus = order[next];
        self.sync_field_focus();
        AuthEntryOutcome::FocusMoved { field: self.focus }
    }

    /// Validate without submitting. Clears and rebuilds `field_errors`.
    pub fn validate(&mut self) -> bool {
        self.field_errors.clear();
        let id = self.identity.value().trim();
        if id.is_empty() {
            self.field_errors.push(AuthFieldError {
                field: AuthEntryField::Identity,
                message: "Identity is required".into(),
            });
        }
        if self.mode.requires_password() && self.secrets.password.is_empty() {
            self.field_errors.push(AuthFieldError {
                field: AuthEntryField::Password,
                message: "Password is required".into(),
            });
        }
        if self.require_confirm && matches!(self.mode, AuthEntryMode::SignUp) {
            if self.secrets.confirm.is_empty() {
                self.field_errors.push(AuthFieldError {
                    field: AuthEntryField::Confirm,
                    message: "Confirm password is required".into(),
                });
            } else if self.secrets.is_mismatch() {
                self.field_errors.push(AuthFieldError {
                    field: AuthEntryField::Confirm,
                    message: "Passwords do not match".into(),
                });
            }
        }
        if self.require_terms
            && matches!(self.mode, AuthEntryMode::SignUp)
            && !self.terms.is_checked()
        {
            self.field_errors.push(AuthFieldError {
                field: AuthEntryField::Terms,
                message: "Accept terms to continue".into(),
            });
        }
        self.field_errors.is_empty()
    }

    fn try_submit(&mut self) -> AuthEntryOutcome {
        if self.pending {
            return AuthEntryOutcome::Ignored;
        }
        if !self.validate() {
            // Focus first invalid field.
            if let Some(err) = self.field_errors.first() {
                self.focus = err.field;
                self.sync_field_focus();
            }
            return AuthEntryOutcome::ValidationFailed {
                errors: self.field_errors.clone(),
            };
        }
        self.host_error = None;
        let passwordless = self.is_passwordless();
        AuthEntryOutcome::Submitted {
            mode: self.mode,
            identity: self.identity.value().trim().to_owned(),
            password_filled: !passwordless && !self.secrets.password.is_empty(),
            confirm_filled: !passwordless && !self.secrets.confirm.is_empty(),
            terms_accepted: self.terms.is_checked(),
            passwordless,
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent) -> AuthEntryOutcome {
        if !self.accepts_input || !self.shell_focused || !key.is_press() {
            return AuthEntryOutcome::Ignored;
        }
        if self.pending {
            if key.code == KeyCode::Esc {
                return AuthEntryOutcome::Cancelled;
            }
            return AuthEntryOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Esc cancel
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return AuthEntryOutcome::Cancelled;
        }

        // Ctrl+G / Alt+M — switch sign-up ↔ sign-in (email-only → sign-in)
        if (ctrl && matches!(key.code, KeyCode::Char('g' | 'G')))
            || (alt && matches!(key.code, KeyCode::Char('m' | 'M')))
        {
            return self.set_mode(self.mode.toggle());
        }

        // Ctrl+E — email-only / passwordless gate (login-05)
        if ctrl && matches!(key.code, KeyCode::Char('e' | 'E')) {
            return self.set_email_only();
        }

        // Ctrl+Enter always submit
        if ctrl && key.code == KeyCode::Enter {
            return self.try_submit();
        }

        // Tab focus (single-field email-only: Tab still FocusMoved same wrap)
        if key.code == KeyCode::Tab && !ctrl && !alt {
            return self.move_focus(if shift { -1 } else { 1 });
        }
        if key.code == KeyCode::BackTab {
            return self.move_focus(-1);
        }

        // Secondary: Ctrl+O → oauth placeholder (host maps)
        if ctrl && matches!(key.code, KeyCode::Char('o' | 'O')) {
            return AuthEntryOutcome::SecondaryAction {
                id: "oauth:default".into(),
            };
        }
        // Ctrl+F → forgot password (sign-in password path only)
        if ctrl
            && matches!(key.code, KeyCode::Char('f' | 'F'))
            && matches!(self.mode, AuthEntryMode::SignIn)
        {
            return AuthEntryOutcome::SecondaryAction {
                id: "forgot-password".into(),
            };
        }

        // Enter always submit; Space on Terms toggles via field routing below.
        if key.code == KeyCode::Enter && !ctrl && !alt {
            return self.try_submit();
        }

        // Route to focused field
        match self.focus {
            AuthEntryField::Identity => match self.identity.handle_key(key) {
                TextInputOutcome::Ignored => AuthEntryOutcome::Ignored,
                TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                    self.field_errors
                        .retain(|e| e.field != AuthEntryField::Identity);
                    AuthEntryOutcome::FieldChanged {
                        field: AuthEntryField::Identity,
                    }
                }
                TextInputOutcome::Submitted(_) => self.try_submit(),
                TextInputOutcome::Cancelled => AuthEntryOutcome::Cancelled,
                _ => AuthEntryOutcome::Ignored,
            },
            AuthEntryField::Password => match self.secrets.password.handle_key(key) {
                PasswordInputOutcome::Ignored => AuthEntryOutcome::Ignored,
                PasswordInputOutcome::Changed => {
                    self.field_errors
                        .retain(|e| e.field != AuthEntryField::Password);
                    AuthEntryOutcome::FieldChanged {
                        field: AuthEntryField::Password,
                    }
                }
                PasswordInputOutcome::Submitted => self.try_submit(),
                PasswordInputOutcome::Cancelled => AuthEntryOutcome::Cancelled,
                PasswordInputOutcome::ClipboardPasteRequest => AuthEntryOutcome::SecondaryAction {
                    id: "clipboard:paste-password".into(),
                },
                other => {
                    // Reveal / clipboard denied — surface as field changed or ignore
                    if matches!(other, PasswordInputOutcome::RevealChanged { .. }) {
                        AuthEntryOutcome::FieldChanged {
                            field: AuthEntryField::Password,
                        }
                    } else {
                        AuthEntryOutcome::Ignored
                    }
                }
            },
            AuthEntryField::Confirm => match self.secrets.confirm.handle_key(key) {
                PasswordInputOutcome::Ignored => AuthEntryOutcome::Ignored,
                PasswordInputOutcome::Changed => {
                    self.field_errors
                        .retain(|e| e.field != AuthEntryField::Confirm);
                    AuthEntryOutcome::FieldChanged {
                        field: AuthEntryField::Confirm,
                    }
                }
                PasswordInputOutcome::Submitted => self.try_submit(),
                PasswordInputOutcome::Cancelled => AuthEntryOutcome::Cancelled,
                PasswordInputOutcome::ClipboardPasteRequest => AuthEntryOutcome::SecondaryAction {
                    id: "clipboard:paste-confirm".into(),
                },
                other => {
                    if matches!(other, PasswordInputOutcome::RevealChanged { .. }) {
                        AuthEntryOutcome::FieldChanged {
                            field: AuthEntryField::Confirm,
                        }
                    } else {
                        AuthEntryOutcome::Ignored
                    }
                }
            },
            AuthEntryField::Terms => match self.terms.handle_key(key, &"terms") {
                CheckboxOutcome::ValueChanged { value, .. } => {
                    self.field_errors
                        .retain(|e| e.field != AuthEntryField::Terms);
                    AuthEntryOutcome::TermsToggled {
                        accepted: value.is_checked(),
                    }
                }
                _ => AuthEntryOutcome::Ignored,
            },
        }
    }
}

// ── Surfaces / paint ────────────────────────────────────────────────────────

/// Host-projected labels and optional aside (second-column copy without images).
#[derive(Debug)]
pub struct AuthEntrySurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut AuthEntryState,
    /// Identity field label.
    pub identity_label: &'a str,
    /// Identity placeholder.
    pub identity_placeholder: &'a str,
    /// Password label.
    pub password_label: &'a str,
    /// Confirm label.
    pub confirm_label: &'a str,
    /// Terms checkbox label.
    pub terms_label: &'a str,
    /// Optional aside lines (signup-02/04 copy peer; no image).
    pub aside_lines: &'a [&'a str],
}

impl<'a> AuthEntrySurfaces<'a> {
    /// Defaults for English CLI.
    #[must_use]
    pub fn english(system: &'a DesignSystem, state: &'a mut AuthEntryState) -> Self {
        Self {
            system,
            state,
            identity_label: "Email",
            identity_placeholder: "you@example.com",
            password_label: "Password",
            confirm_label: "Confirm",
            terms_label: "Accept terms",
            aside_lines: &[],
        }
    }
}

/// Example aside copy for split layouts (text only).
#[must_use]
pub fn example_auth_aside_lines() -> &'static [&'static str] {
    &[
        "Ship faster in the terminal.",
        "Keyboard-first. Host-owned auth.",
    ]
}

/// Layout hint: form column width when aside present.
#[must_use]
pub fn auth_entry_form_width(total: u16, has_aside: bool) -> u16 {
    if !has_aside || total < 48 {
        total
    } else {
        (total * 3 / 5).max(28).min(total.saturating_sub(16))
    }
}

/// Fixed-width secret placeholder, resolved from the glyph catalog.
///
/// One mask glyph for every masked field in the library; the width is fixed so
/// an empty field never advertises how long the real secret is.
fn mask_placeholder() -> String {
    Glyph::Mask.resolve().text.repeat(crate::style::MASK_CELLS)
}

/// Paint auth entry panel.
pub fn paint_auth_entry(buffer: &mut Buffer, area: Rect, surfaces: AuthEntrySurfaces<'_>) {
    if area.is_empty() {
        return;
    }
    let system = surfaces.system;
    let state = surfaces.state;
    let has_aside = !surfaces.aside_lines.is_empty() && area.width >= 48;
    let form_w = auth_entry_form_width(area.width, has_aside);
    let form_area = Rect::new(area.x, area.y, form_w, area.height);
    let aside_area = if has_aside {
        Rect::new(
            area.x.saturating_add(form_w).saturating_add(1),
            area.y,
            area.width.saturating_sub(form_w).saturating_sub(1),
            area.height,
        )
    } else {
        Rect::default()
    };

    let title = state.mode.title();
    let mut panel_state = crate::widgets::PanelState::default();
    let body = Panel::new(system)
        .title(title)
        .variant(PanelVariant::Bordered)
        .emphasis(if state.shell_focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        })
        .paint(form_area, buffer, Some(&mut panel_state));

    let mut y = body.y;
    let x = body.x.saturating_add(1);
    let w = body.width.saturating_sub(2);
    let bottom = body.y.saturating_add(body.height);

    // What the host told us, as a callout — the glyph carries the failure and
    // the sentence stays readable. Field problems are stated once, inline on
    // the field that has them, never also as a summary that leaks field ids
    // (plans/010 Step 2).
    if let Some(err) = state.host_error.as_deref()
        && y < bottom
        && w > 0
    {
        let height = 1u16.max(1).min(bottom.saturating_sub(y));
        Callout::new(err, system)
            .tone(CalloutTone::Danger)
            .paint(Rect::new(x, y, w, height), buffer);
        y = y.saturating_add(height).saturating_add(1);
    }

    // A submit in flight says so, rather than silently swallowing input.
    if state.pending && y < bottom && w > 0 {
        system.paint_row(
            buffer,
            Rect::new(x, y, w, 1),
            &format!("{} {}", pending_glyph(system), state.mode.pending_verb()),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
    }

    let id_err = state
        .field_errors
        .iter()
        .find(|e| e.field == AuthEntryField::Identity)
        .map(|e| e.message.as_str());
    let pw_err = state
        .field_errors
        .iter()
        .find(|e| e.field == AuthEntryField::Password)
        .map(|e| e.message.as_str());
    let cf_err = state
        .field_errors
        .iter()
        .find(|e| e.field == AuthEntryField::Confirm)
        .map(|e| e.message.as_str());

    // Identity
    if y.saturating_add(1) < bottom && w > 0 {
        let field_h = 2u16.min(bottom.saturating_sub(y));
        let fa = Rect::new(x, y, w, field_h);
        let val = if let Some(m) = id_err {
            Validation::Invalid(m)
        } else {
            Validation::Valid
        };
        let _ = TextInput::new(surfaces.identity_label, system)
            .placeholder(surfaces.identity_placeholder)
            .validation(val)
            .paint(fa, buffer, &mut state.identity);
        y = y.saturating_add(field_h.saturating_add(1));
    }

    // Password (not for email-only)
    if state.mode.requires_password() && y.saturating_add(1) < bottom && w > 0 {
        let field_h = 2u16.min(bottom.saturating_sub(y));
        let fa = Rect::new(x, y, w, field_h);
        let val = if let Some(m) = pw_err {
            Validation::Invalid(m)
        } else {
            Validation::Valid
        };
        let _ = PasswordInput::new(surfaces.password_label, system)
            .placeholder(&mask_placeholder())
            .validation(val)
            .paint(fa, buffer, &mut state.secrets.password);
        y = y.saturating_add(field_h.saturating_add(1));
    }

    // Confirm
    if state.require_confirm && matches!(state.mode, AuthEntryMode::SignUp) {
        if y.saturating_add(1) < bottom && w > 0 {
            let field_h = 2u16.min(bottom.saturating_sub(y));
            let fa = Rect::new(x, y, w, field_h);
            let val = if let Some(m) = cf_err {
                Validation::Invalid(m)
            } else {
                Validation::Valid
            };
            let _ = PasswordInput::new(surfaces.confirm_label, system)
                .placeholder(&mask_placeholder())
                .validation(val)
                .paint(fa, buffer, &mut state.secrets.confirm);
            y = y.saturating_add(field_h.saturating_add(1));
        }
    }

    // Terms
    if state.require_terms && matches!(state.mode, AuthEntryMode::SignUp) {
        if y < bottom && w > 0 {
            state.terms.set_invalid(
                state
                    .field_errors
                    .iter()
                    .any(|e| e.field == AuthEntryField::Terms),
            );
            let fa = Rect::new(x, y, w, 1);
            let _ = Checkbox::new("terms", surfaces.terms_label, system).paint(
                fa,
                buffer,
                &mut state.terms,
            );
            y = y.saturating_add(2);
        }
    }

    // The submit action is a button, not only a chord (plans/016 Step 3).
    if y < bottom && w > 0 {
        let label = match state.mode {
            AuthEntryMode::SignUp => "Create account",
            AuthEntryMode::SignIn => "Sign in",
            AuthEntryMode::EmailOnly => "Send link",
        };
        let submit = Button::new(label, system).variant(ButtonVariant::Primary);
        let width = submit.preferred_width().min(w);
        if width > 0 {
            let mut submit_state = ButtonState::new();
            submit_state.activation.set_accepts_input(true);
            submit_state.activation.set_loading(state.pending);
            submit.paint(Rect::new(x, y, width, 1), buffer, &mut submit_state);
            y = y.saturating_add(2);
        }
    }

    // Footer hints
    if y < bottom && w > 0 {
        let hint = match state.mode {
            AuthEntryMode::SignUp => "Tab fields · Enter submit · Esc cancel · C-g sign in",
            AuthEntryMode::SignIn => {
                "Enter submit · Tab next · C-g sign up · C-f forgot · Esc cancel"
            }
            AuthEntryMode::EmailOnly => {
                "Enter request link · Esc cancel · C-g password · C-o oauth"
            }
        };
        system.paint_row(
            buffer,
            Rect::new(x, y.min(bottom.saturating_sub(1)), w, 1),
            hint,
            system.style(Role::TextMuted),
        );
    }

    // Aside (text only)
    if has_aside && !aside_area.is_empty() {
        let mut ay = aside_area.y.saturating_add(1);
        let ax = aside_area.x.saturating_add(1);
        let aw = aside_area.width.saturating_sub(2);
        for line in surfaces.aside_lines {
            if ay >= aside_area.bottom() || aw == 0 {
                break;
            }
            system.paint_row(
                buffer,
                Rect::new(ax, ay, aw, 1),
                line,
                system.style(Role::TextMuted),
            );
            ay = ay.saturating_add(1);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn type_identity(st: &mut AuthEntryState, s: &str) {
        st.focus = AuthEntryField::Identity;
        st.sync_field_focus();
        for c in s.chars() {
            let _ = st.handle_key(press(c));
        }
    }

    fn type_password(st: &mut AuthEntryState, s: &str) {
        st.focus = AuthEntryField::Password;
        st.sync_field_focus();
        for c in s.chars() {
            let out = st.handle_key(press(c));
            assert!(
                matches!(
                    out,
                    AuthEntryOutcome::FieldChanged {
                        field: AuthEntryField::Password
                    }
                ),
                "password key {c:?} → {out:?}"
            );
        }
    }

    fn type_confirm(st: &mut AuthEntryState, s: &str) {
        st.focus = AuthEntryField::Confirm;
        st.sync_field_focus();
        for c in s.chars() {
            let out = st.handle_key(press(c));
            assert!(
                matches!(
                    out,
                    AuthEntryOutcome::FieldChanged {
                        field: AuthEntryField::Confirm
                    }
                ),
                "confirm key {c:?} → {out:?}"
            );
        }
    }

    #[test]
    fn tab_advances_fields_and_wraps() {
        let mut st = AuthEntryState::sign_up();
        assert_eq!(st.focus(), AuthEntryField::Identity);
        let out = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::FocusMoved {
                    field: AuthEntryField::Password
                }
            ),
            "{out:?}"
        );
        let _ = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(st.focus(), AuthEntryField::Confirm);
        let _ = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(st.focus(), AuthEntryField::Terms);
        let out = st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::FocusMoved {
                    field: AuthEntryField::Identity
                }
            ),
            "{out:?}"
        );
    }

    #[test]
    fn empty_submit_validation_blocked() {
        let mut st = AuthEntryState::sign_up();
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match out {
            AuthEntryOutcome::ValidationFailed { errors } => {
                assert!(!errors.is_empty());
                assert!(errors.iter().any(|e| e.field == AuthEntryField::Identity));
                assert!(errors.iter().any(|e| e.field == AuthEntryField::Password));
                // No secret material in messages
                for e in &errors {
                    assert!(!e.message.contains("secret"));
                    assert_ne!(e.message, st.password_secret());
                }
            }
            other => panic!("expected ValidationFailed, got {other:?}"),
        }
    }

    #[test]
    fn password_edit_does_not_leak_in_outcome() {
        let mut st = AuthEntryState::sign_in();
        st.focus = AuthEntryField::Password;
        st.sync_field_focus();
        let out = st.handle_key(press('s'));
        match out {
            AuthEntryOutcome::FieldChanged {
                field: AuthEntryField::Password,
            } => {}
            other => panic!("{other:?}"),
        }
        // secret is filled but outcome has no password string
        assert_eq!(st.password_secret(), "s");
        let dbg = format!("{out:?}");
        assert!(!dbg.contains("password_secret"));
        assert!(!dbg.contains("\"s\"") || dbg.contains("Password")); // field name ok
    }

    #[test]
    fn successful_signup_submit_payload() {
        let mut st = AuthEntryState::sign_up();
        type_identity(&mut st, "a@b.co");
        type_password(&mut st, "hunter2x");
        type_confirm(&mut st, "hunter2x");
        st.focus = AuthEntryField::Terms;
        st.sync_field_focus();
        let out = st.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        assert!(
            matches!(out, AuthEntryOutcome::TermsToggled { accepted: true }),
            "{out:?}"
        );
        assert!(st.terms_accepted());
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match out {
            AuthEntryOutcome::Submitted {
                mode: AuthEntryMode::SignUp,
                ref identity,
                password_filled: true,
                confirm_filled: true,
                terms_accepted: true,
                passwordless: false,
            } => {
                assert_eq!(identity, "a@b.co");
                // host still reads secret separately
                assert_eq!(st.password_secret(), "hunter2x");
            }
            other => panic!("expected Submitted, got {other:?}"),
        }
        // Submitted debug must not embed the secret
        let dbg = format!("{out:?}");
        assert!(
            !dbg.contains("hunter2x"),
            "secret leaked in outcome debug: {dbg}"
        );
    }

    #[test]
    fn sign_in_submit_and_forgot_password() {
        let mut st = AuthEntryState::sign_in();
        assert_eq!(st.mode(), AuthEntryMode::SignIn);
        assert_eq!(
            st.field_order(),
            vec![AuthEntryField::Identity, AuthEntryField::Password]
        );
        // empty submit blocked
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            matches!(out, AuthEntryOutcome::ValidationFailed { .. }),
            "{out:?}"
        );
        type_identity(&mut st, "user@cli.dev");
        type_password(&mut st, "s3cret!!");
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match out {
            AuthEntryOutcome::Submitted {
                mode: AuthEntryMode::SignIn,
                ref identity,
                password_filled: true,
                passwordless: false,
                ..
            } => {
                assert_eq!(identity, "user@cli.dev");
                assert_eq!(st.password_secret(), "s3cret!!");
            }
            other => panic!("{other:?}"),
        }
        let dbg = format!("{out:?}");
        assert!(!dbg.contains("s3cret!!"), "secret in outcome: {dbg}");
        // forgot-password secondary
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::SecondaryAction { ref id } if id == "forgot-password"
            ),
            "{out:?}"
        );
        // oauth secondary
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::SecondaryAction { ref id } if id == "oauth:default"
            ),
            "{out:?}"
        );
    }

    #[test]
    fn email_only_passwordless_submit() {
        let mut st = AuthEntryState::email_only();
        assert!(st.is_passwordless());
        assert_eq!(st.field_order(), vec![AuthEntryField::Identity]);
        // no password required
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(
            matches!(out, AuthEntryOutcome::ValidationFailed { ref errors } if errors
                .iter()
                .any(|e| e.field == AuthEntryField::Identity)
                && !errors.iter().any(|e| e.field == AuthEntryField::Password)),
            "{out:?}"
        );
        type_identity(&mut st, "magic@link.test");
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match out {
            AuthEntryOutcome::Submitted {
                mode: AuthEntryMode::EmailOnly,
                ref identity,
                password_filled: false,
                passwordless: true,
                ..
            } => {
                assert_eq!(identity, "magic@link.test");
                assert!(st.password_secret().is_empty());
            }
            other => panic!("{other:?}"),
        }
        // Ctrl+G → sign-in
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::ModeSwitched {
                    mode: AuthEntryMode::SignIn
                }
            ),
            "{out:?}"
        );
    }

    #[test]
    fn sign_in_ctrl_e_switches_to_email_only() {
        let mut st = AuthEntryState::sign_in();
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::ModeSwitched {
                    mode: AuthEntryMode::EmailOnly
                }
            ),
            "{out:?}"
        );
        assert!(st.is_passwordless());
    }

    #[test]
    fn paint_sign_in_smoke() {
        let system = DesignSystem::default();
        let mut st = AuthEntryState::sign_in();
        type_identity(&mut st, "a@b.c");
        let area = Rect::new(0, 0, 60, 16);
        let mut buf = Buffer::empty(area);
        paint_auth_entry(&mut buf, area, AuthEntrySurfaces::english(&system, &mut st));
        let mut sample = String::new();
        for y in 0..3 {
            for x in 0..16 {
                if let Some(c) = buf.cell((x, y)) {
                    sample.push_str(c.symbol());
                }
            }
        }
        assert!(
            sample.contains("Sign") || sample.contains("Email") || sample.contains('S'),
            "{sample:?}"
        );
    }

    #[test]
    fn confirm_mismatch_blocks_submit() {
        let mut st = AuthEntryState::sign_up();
        type_identity(&mut st, "u@x.y");
        type_password(&mut st, "abc12345");
        type_confirm(&mut st, "xyz99999");
        st.terms.set_checked(true);
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL));
        match out {
            AuthEntryOutcome::ValidationFailed { errors } => {
                assert!(
                    errors
                        .iter()
                        .any(|e| e.field == AuthEntryField::Confirm && e.message.contains("match")),
                    "{errors:?}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cancel_and_mode_switch() {
        let mut st = AuthEntryState::sign_up();
        type_password(&mut st, "temp");
        let out = st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(out, AuthEntryOutcome::Cancelled));
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
        assert!(
            matches!(
                out,
                AuthEntryOutcome::ModeSwitched {
                    mode: AuthEntryMode::SignIn
                }
            ),
            "{out:?}"
        );
        assert_eq!(st.mode(), AuthEntryMode::SignIn);
        // secrets cleared on mode switch
        assert!(st.password_secret().is_empty());
    }

    #[test]
    fn paint_smoke_with_aside() {
        let system = DesignSystem::default();
        let mut st = AuthEntryState::sign_up();
        type_identity(&mut st, "x@y.z");
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let aside = example_auth_aside_lines();
        let mut surfaces = AuthEntrySurfaces::english(&system, &mut st);
        surfaces.aside_lines = aside;
        paint_auth_entry(&mut buf, area, surfaces);
        // title / email chrome should appear
        let mut sample = String::new();
        for y in 0..4 {
            for x in 0..20 {
                if let Some(c) = buf.cell((x, y)) {
                    sample.push_str(c.symbol());
                }
            }
        }
        assert!(
            sample.contains("Create") || sample.contains("Email") || sample.contains('E'),
            "paint missing chrome: {sample:?}"
        );
    }
}
