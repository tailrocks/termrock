// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Secure single-line secret entry derived from [`TextInput`](super::TextInput).
//!
//! **Mission.** Password / token fields must never leak the secret through
//! paint (when masked), semantic scene, `Debug`, snapshots, or clipboard
//! outcomes unless the host explicitly opts into a dangerous policy.
//!
//! **vs TextInput.** `TextInput::secret` only masks paint; it still Debug-dumps
//! the value, may emit clipboard copy, and embeds secrets in
//! [`TextInputOutcome::Submitted`](super::TextInputOutcome). Prefer
//! [`PasswordInput`] for any real credential.
//!
//! Research: secure CLI prompts, password managers, desktop secret fields.
use std::fmt;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent},
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Role},
    text::take_display_cols,
};

use super::{
    TextInput, TextInputOutcome, TextInputParts, TextInputState, TextInputValidity, Validation,
};

// ── Policies ────────────────────────────────────────────────────────────────

/// When (if ever) the secret may be shown in the clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum RevealPolicy {
    /// Always masked (default).
    #[default]
    Never,
    /// Host/user toggles reveal (Alt+R or reveal glyph).
    Explicit,
    /// Revealed only while the hold chord is pressed (Alt+H press/release).
    Hold,
}

impl RevealPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Explicit => "explicit",
            Self::Hold => "hold",
        }
    }
}

/// Clipboard / OSC-52 policy for secrets.
///
/// **Default [`PasteOnly`](Self::PasteOnly):** paste may request host paste;
/// copy and cut never embed secret text in outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ClipboardPolicy {
    /// Block copy, cut, and paste requests.
    DenyAll,
    /// Allow paste request only (default).
    #[default]
    PasteOnly,
    /// Host may copy via `secret()` after a denied outcome probe — still never
    /// embeds the secret in an outcome. Prefer avoiding this.
    AllowHostCopy,
}

impl ClipboardPolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::DenyAll => "deny-all",
            Self::PasteOnly => "paste-only",
            Self::AllowHostCopy => "allow-host-copy",
        }
    }
}

/// Host-owned strength / quality cue (never derived by logging the secret).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PasswordStrengthHint {
    /// No status row.
    #[default]
    None,
    /// Empty field.
    Empty,
    /// Weak.
    Weak,
    /// Fair / medium.
    Fair,
    /// Good.
    Good,
    /// Strong.
    Strong,
    /// Pending external check.
    Pending,
}

impl PasswordStrengthHint {
    /// Meter glyphs for this level, drawn from the shared block ramp.
    ///
    /// Strength is a quantity, so it reads as a filled meter rather than as a
    /// word alone — three cells of `▁▃▅` say more at a glance than "fair"
    /// (plans/008 Step 5).
    #[must_use]
    pub fn meter(self, ascii: bool) -> String {
        let filled = match self {
            Self::None | Self::Empty | Self::Pending => return String::new(),
            Self::Weak => 1usize,
            Self::Fair => 2,
            Self::Good => 3,
            Self::Strong => 4,
        };
        if ascii {
            let mut out = String::from("[");
            for i in 0..4 {
                out.push(if i < filled { '#' } else { '-' });
            }
            out.push(']');
            return out;
        }
        let ramp = crate::style::BLOCK_RAMP;
        (0..4)
            .map(|i| {
                if i < filled {
                    ramp[(i + 1) * 2]
                } else {
                    ramp[0]
                }
            })
            .collect()
    }

    /// Semantic tone for this level.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Weak => Role::Danger,
            Self::Fair => Role::Warning,
            Self::Good | Self::Strong => Role::Success,
            Self::None | Self::Empty | Self::Pending => Role::TextMuted,
        }
    }

    /// Short label for paint / a11y (never contains the secret).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Empty => "empty",
            Self::Weak => "weak",
            Self::Fair => "fair",
            Self::Good => "good",
            Self::Strong => "strong",
            Self::Pending => "checking…",
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Empty => "empty",
            Self::Weak => "weak",
            Self::Fair => "fair",
            Self::Good => "good",
            Self::Strong => "strong",
            Self::Pending => "pending",
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Semantic results. **Never embeds the secret string.**
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PasswordInputOutcome {
    /// No effect.
    Ignored,
    /// Value, caret, selection, or chrome changed.
    Changed,
    /// Submit requested; host reads via [`PasswordInputState::secret`] or
    /// [`PasswordInputState::take_secret`].
    Submitted,
    /// Cancel / Esc.
    Cancelled,
    /// Host may resolve paste and call [`PasswordInputState::insert_str`].
    ClipboardPasteRequest,
    /// Copy/cut blocked by [`ClipboardPolicy`] (or never emitted with secret).
    ClipboardDenied,
    /// Host may call [`PasswordInputState::secret`] to perform a careful copy
    /// (only when policy is [`ClipboardPolicy::AllowHostCopy`]).
    ClipboardCopyAllowed,
    /// Reveal visibility flipped.
    RevealChanged {
        /// Whether plaintext is currently shown.
        revealed: bool,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`PasswordInput`].
///
/// `Debug` is redacted: never prints the secret or undo contents. Emptiness
/// and policy flags are visible; byte/grapheme length is omitted to reduce
/// log side-channels.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordInputState {
    editor: TextInputState,
    reveal_policy: RevealPolicy,
    /// Explicit-mode latch.
    revealed: bool,
    /// Hold-mode while key is down.
    hold_reveal: bool,
    clipboard: ClipboardPolicy,
    enabled: bool,
    read_only: bool,
    pending: bool,
    focused: bool,
}

impl fmt::Debug for PasswordInputState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordInputState")
            .field("filled", &(!self.is_empty()))
            .field("reveal_policy", &self.reveal_policy)
            .field("revealed", &self.is_revealed())
            .field("clipboard", &self.clipboard)
            .field("enabled", &self.enabled)
            .field("read_only", &self.read_only)
            .field("pending", &self.pending)
            .field("focused", &self.focused)
            .finish()
    }
}

impl Default for PasswordInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PasswordInputState {
    fn drop(&mut self) {
        self.editor.secure_clear();
    }
}

impl PasswordInputState {
    /// Empty secret field (empty allowed for progressive typing).
    #[must_use]
    pub fn new() -> Self {
        let mut editor = TextInputState::new("").with_allow_empty(true);
        editor.set_enabled(true);
        editor.set_focused(false);
        Self {
            editor,
            reveal_policy: RevealPolicy::Never,
            revealed: false,
            hold_reveal: false,
            clipboard: ClipboardPolicy::PasteOnly,
            enabled: true,
            read_only: false,
            pending: false,
            focused: false,
        }
    }

    /// Seed with a secret (prefer empty + user entry in production).
    #[must_use]
    pub fn with_secret(secret: impl Into<String>) -> Self {
        let mut state = Self::new();
        state.editor.set_enabled(true);
        state.editor.set_focused(false);
        state.editor = state.editor.reseed(secret);
        state
    }

    /// Live typing. [`Self::new`] stays idle (`editing: false`).
    #[must_use]
    pub fn with_editing(mut self) -> Self {
        self.editor.begin_edit();
        self
    }

    /// Start the insert session (Junie Enter on an idle field).
    pub fn begin_edit(&mut self) {
        self.editor.begin_edit();
    }

    /// Max graphemes.
    #[must_use]
    pub fn with_max_graphemes(mut self, max: usize) -> Self {
        let editor = std::mem::replace(&mut self.editor, TextInputState::new(""));
        self.editor = editor.with_max_graphemes(max);
        self
    }

    fn sync_editor_gates(&mut self) {
        self.editor.set_enabled(self.enabled);
        self.editor.set_read_only(self.read_only);
        self.editor.set_loading(self.pending);
        self.editor.set_focused(self.focused);
    }

    /// Reveal policy.
    pub fn set_reveal_policy(&mut self, policy: RevealPolicy) {
        self.reveal_policy = policy;
        if matches!(policy, RevealPolicy::Never) {
            self.revealed = false;
            self.hold_reveal = false;
        }
    }

    /// Reveal policy (builder).
    #[must_use]
    pub fn with_reveal_policy(mut self, policy: RevealPolicy) -> Self {
        self.set_reveal_policy(policy);
        self
    }
    /// Clipboard policy (builder).
    #[must_use]
    pub const fn with_clipboard_policy(mut self, policy: ClipboardPolicy) -> Self {
        self.clipboard = policy;
        self
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.sync_editor_gates();
    }

    /// Read-only.
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        self.sync_editor_gates();
    }

    /// Pending / verifying (blocks edits; loading cue).
    pub fn set_pending(&mut self, on: bool) {
        self.pending = on;
        self.sync_editor_gates();
    }

    /// Focus paint flag.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.sync_editor_gates();
    }

    /// Whether the field is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.editor.value().is_empty()
    }
    /// Whether plaintext is currently visible per policy.
    #[must_use]
    pub const fn is_revealed(&self) -> bool {
        match self.reveal_policy {
            RevealPolicy::Never => false,
            RevealPolicy::Explicit => self.revealed,
            RevealPolicy::Hold => self.hold_reveal,
        }
    }

    /// Enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Pending.
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        self.pending
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Whether edits allowed.
    #[must_use]
    pub fn can_edit(&self) -> bool {
        self.editor.can_edit()
    }

    /// Borrow secret for validation / submit (prefer [`Self::take_secret`]).
    ///
    /// **Do not log, format, or put this into semantic scenes.**
    #[must_use]
    pub fn secret(&self) -> &str {
        self.editor.value()
    }

    /// Take ownership of the secret and securely clear the field.
    #[must_use]
    pub fn take_secret(&mut self) -> String {
        let value = self.editor.value().to_owned();
        self.editor.secure_clear();
        self.revealed = false;
        self.hold_reveal = false;
        value
    }

    /// Equality for confirmation pairing.
    #[must_use]
    pub fn secrets_equal(&self, other: &str) -> bool {
        self.editor.value() == other
    }

    /// Validity.
    #[must_use]
    pub fn validity(&self) -> TextInputValidity {
        self.editor.validity()
    }

    /// Last paint geometry from the inner editor.
    #[must_use]
    pub fn parts(&self) -> Option<&TextInputParts> {
        self.editor.parts()
    }

    /// Clear secret (secure).
    pub fn clear(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        self.editor.secure_clear();
        true
    }

    /// Securely clear.
    pub fn secure_clear(&mut self) {
        self.editor.secure_clear();
        self.revealed = false;
        self.hold_reveal = false;
    }

    /// Toggle explicit reveal.
    pub fn toggle_reveal(&mut self) -> PasswordInputOutcome {
        if !matches!(self.reveal_policy, RevealPolicy::Explicit) {
            return PasswordInputOutcome::Ignored;
        }
        self.revealed = !self.revealed;
        PasswordInputOutcome::RevealChanged {
            revealed: self.revealed,
        }
    }
    /// Insert paste payload.
    pub fn insert_str(&mut self, text: &str) -> PasswordInputOutcome {
        if matches!(self.clipboard, ClipboardPolicy::DenyAll) {
            return PasswordInputOutcome::ClipboardDenied;
        }
        self.editor.begin_edit();
        match self.editor.insert_str(text) {
            TextInputOutcome::Changed => PasswordInputOutcome::Changed,
            _ => PasswordInputOutcome::Ignored,
        }
    }

    /// Default key adapter with secret policies.
    pub fn handle_key(&mut self, key: KeyEvent) -> PasswordInputOutcome {
        // Hold reveal: Alt+H press/release (neutral KeyCode has no function keys).
        let alt_hold = key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('h' | 'H'));
        if matches!(self.reveal_policy, RevealPolicy::Hold) && alt_hold {
            self.sync_editor_gates();
            match key.kind {
                KeyEventKind::Press | KeyEventKind::Repeat => {
                    if !self.hold_reveal {
                        self.hold_reveal = true;
                        return PasswordInputOutcome::RevealChanged { revealed: true };
                    }
                    return PasswordInputOutcome::Ignored;
                }
                KeyEventKind::Release => {
                    if self.hold_reveal {
                        self.hold_reveal = false;
                        return PasswordInputOutcome::RevealChanged { revealed: false };
                    }
                    return PasswordInputOutcome::Ignored;
                }
            }
        }

        if key.is_release() {
            return PasswordInputOutcome::Ignored;
        }

        if !self.enabled {
            return PasswordInputOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let explicit_reveal = alt && !ctrl && matches!(key.code, KeyCode::Char('r' | 'R'));

        // Explicit reveal, submit/cancel, clipboard outcomes, and destructive
        // editor chords are one-shot physical actions. Keep ordinary editor
        // repeats and Hold reveal above.
        if !key.is_press()
            && (matches!(
                key.code,
                KeyCode::Enter | KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab
            ) || (ctrl
                && matches!(
                    key.code,
                    KeyCode::Char(
                        'c' | 'C'
                            | 'k'
                            | 'K'
                            | 'm'
                            | 'M'
                            | 'u'
                            | 'U'
                            | 'v'
                            | 'V'
                            | 'w'
                            | 'W'
                            | 'x'
                            | 'X',
                    )
                ))
                || explicit_reveal)
        {
            return PasswordInputOutcome::Ignored;
        }

        self.sync_editor_gates();

        // Explicit reveal: Alt+R
        if alt
            && !ctrl
            && matches!(key.code, KeyCode::Char('r' | 'R'))
            && matches!(self.reveal_policy, RevealPolicy::Explicit)
        {
            return self.toggle_reveal();
        }

        // Clipboard gate — never let TextInput put secret text in outcomes
        if ctrl && !alt {
            match key.code {
                KeyCode::Char('c' | 'C') => {
                    return match self.clipboard {
                        ClipboardPolicy::AllowHostCopy if !self.is_empty() => {
                            PasswordInputOutcome::ClipboardCopyAllowed
                        }
                        _ => PasswordInputOutcome::ClipboardDenied,
                    };
                }
                KeyCode::Char('x' | 'X') => {
                    // Cut always denied for secrets (would need copy + delete)
                    return PasswordInputOutcome::ClipboardDenied;
                }
                KeyCode::Char('v' | 'V') => {
                    return match self.clipboard {
                        ClipboardPolicy::DenyAll => PasswordInputOutcome::ClipboardDenied,
                        ClipboardPolicy::PasteOnly | ClipboardPolicy::AllowHostCopy => {
                            if self.can_edit() {
                                PasswordInputOutcome::ClipboardPasteRequest
                            } else {
                                PasswordInputOutcome::Ignored
                            }
                        }
                    };
                }
                _ => {}
            }
        }

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if self.editor.is_editing() {
                let _ = self.editor.handle_key(key);
            }
            return PasswordInputOutcome::Cancelled;
        }

        // Submit without embedding secret
        if matches!(key.code, KeyCode::Enter)
            || (ctrl && matches!(key.code, KeyCode::Char('m' | 'M')))
        {
            if !self.editor.is_editing() {
                self.editor.begin_edit();
                return PasswordInputOutcome::Changed;
            }
            if !self.editor.is_valid() {
                return PasswordInputOutcome::Ignored;
            }
            return PasswordInputOutcome::Submitted;
        }

        match self.editor.handle_key(key) {
            TextInputOutcome::Ignored => PasswordInputOutcome::Ignored,
            TextInputOutcome::Changed | TextInputOutcome::Cleared => PasswordInputOutcome::Changed,
            TextInputOutcome::Submitted(_) => PasswordInputOutcome::Submitted,
            TextInputOutcome::Cancelled => PasswordInputOutcome::Cancelled,
            TextInputOutcome::ClipboardCopy { .. } | TextInputOutcome::ClipboardCut { .. } => {
                PasswordInputOutcome::ClipboardDenied
            }
            TextInputOutcome::ClipboardPasteRequest => match self.clipboard {
                ClipboardPolicy::DenyAll => PasswordInputOutcome::ClipboardDenied,
                _ => PasswordInputOutcome::ClipboardPasteRequest,
            },
        }
    }

    /// Intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> PasswordInputOutcome {
        self.sync_editor_gates();
        if !self.enabled {
            return PasswordInputOutcome::Ignored;
        }
        match intent {
            UiIntent::Submit | UiIntent::Activate => {
                if !self.editor.is_editing() {
                    self.editor.begin_edit();
                    PasswordInputOutcome::Changed
                } else if self.editor.is_valid() {
                    PasswordInputOutcome::Submitted
                } else {
                    PasswordInputOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => PasswordInputOutcome::Cancelled,
            other => match self.editor.handle_intent(other) {
                TextInputOutcome::Changed => PasswordInputOutcome::Changed,
                TextInputOutcome::Cancelled => PasswordInputOutcome::Cancelled,
                TextInputOutcome::Submitted(_) => PasswordInputOutcome::Submitted,
                _ => PasswordInputOutcome::Ignored,
            },
        }
    }

    /// Mouse on field / reveal hit.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        field: Rect,
        reveal_hit: Option<Rect>,
    ) -> PasswordInputOutcome {
        self.sync_editor_gates();
        if let Some(hit) = reveal_hit {
            if hit.contains(event.position)
                && matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
                && matches!(self.reveal_policy, RevealPolicy::Explicit)
            {
                return self.toggle_reveal();
            }
        }
        match self.editor.handle_mouse(event, field) {
            TextInputOutcome::Changed => PasswordInputOutcome::Changed,
            _ => PasswordInputOutcome::Ignored,
        }
    }
}

// ── Confirm pair ────────────────────────────────────────────────────────────

/// Password + confirmation pair for sign-up / change-password flows.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordConfirmState {
    /// Primary secret.
    pub password: PasswordInputState,
    /// Confirmation secret.
    pub confirm: PasswordInputState,
}

impl fmt::Debug for PasswordConfirmState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordConfirmState")
            .field("password", &self.password)
            .field("confirm", &self.confirm)
            .field("match", &self.secrets_match())
            .finish()
    }
}

impl Default for PasswordConfirmState {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordConfirmState {
    /// Empty pair.
    #[must_use]
    pub fn new() -> Self {
        Self {
            password: PasswordInputState::new(),
            confirm: PasswordInputState::new(),
        }
    }

    /// Both non-empty and equal.
    #[must_use]
    pub fn secrets_match(&self) -> bool {
        !self.password.is_empty()
            && !self.confirm.is_empty()
            && self.password.secrets_equal(self.confirm.secret())
    }

    /// Both filled but unequal.
    #[must_use]
    pub fn is_mismatch(&self) -> bool {
        !self.password.is_empty()
            && !self.confirm.is_empty()
            && !self.password.secrets_equal(self.confirm.secret())
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Secure password / token field.
#[derive(Debug, Clone, Copy)]
pub struct PasswordInput<'a> {
    label: &'a str,
    placeholder: &'a str,
    validation: Validation<'a>,
    system: &'a DesignSystem,
    mask: char,
    strength: PasswordStrengthHint,
    show_reveal: bool,
}

/// Hit geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordInputParts {
    /// Root.
    pub root: Rect,
    /// Field.
    pub field: Rect,
    /// Reveal toggle hit (when policy Explicit and shown).
    pub reveal: Option<Rect>,
    /// Cursor.
    pub cursor: Option<Rect>,
}

impl<'a> PasswordInput<'a> {
    /// Create labeled password field.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            placeholder: "",
            validation: Validation::Valid,
            system,
            mask: '●',
            strength: PasswordStrengthHint::None,
            show_reveal: true,
        }
    }

    /// Placeholder when empty.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Validation projection.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// Mask grapheme (default `●`). Length is never tracked; paint uses [`MASK_CELLS`].
    #[must_use]
    pub const fn mask(mut self, mask: char) -> Self {
        self.mask = mask;
        self
    }

    /// Host strength / status cue.
    #[must_use]
    pub const fn strength(mut self, hint: PasswordStrengthHint) -> Self {
        self.strength = hint;
        self
    }

    /// Show reveal glyph when policy is Explicit.
    #[must_use]
    pub const fn show_reveal(mut self, on: bool) -> Self {
        self.show_reveal = on;
        self
    }

    /// ASCII-safe mask.
    #[must_use]
    /// Paint (masked unless revealed).
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut PasswordInputState,
    ) -> PasswordInputParts {
        state.sync_editor_gates();
        let mask = { self.mask };
        let revealed = state.is_revealed();

        let show_reveal = self.show_reveal
            && matches!(state.reveal_policy, RevealPolicy::Explicit)
            && area.width > 4;
        let paint_area = if show_reveal {
            Rect::new(area.x, area.y, area.width.saturating_sub(2), area.height)
        } else {
            area
        };

        let input = TextInput::new(self.label, self.system)
            .placeholder(self.placeholder)
            .validation(self.validation)
            .secret(!revealed)
            .secret_mask(mask);
        let ti_parts = input.paint(paint_area, buffer, &mut state.editor);

        let mut reveal_rect = None;
        if show_reveal {
            let rx = area.right().saturating_sub(1);
            let ry = ti_parts.field.y;
            reveal_rect = Some(Rect::new(rx, ry, 1, 1));
            let mark = self
                .system
                .glyphs
                .resolve(if revealed {
                    crate::style::Glyph::EmptyCircle
                } else {
                    crate::style::Glyph::Mask
                })
                .text;
            let reveal_recipe = self.system.button_recipe(
                ButtonRecipeVariant::Quiet,
                if !state.enabled {
                    ControlState::Disabled
                } else if state.pending {
                    ControlState::Loading
                } else if state.focused {
                    ControlState::Focused
                } else {
                    ControlState::Default
                },
                self.system.junie_theme().surface,
            );
            buffer.set_stringn(
                rx,
                ry,
                mark,
                1,
                reveal_recipe.fill.patch(reveal_recipe.label),
            );
        }

        if ti_parts.field.y.saturating_add(1) < area.bottom() {
            let status = if state.pending {
                PasswordStrengthHint::Pending.label()
            } else {
                self.strength.label()
            };
            if !status.is_empty() && !matches!(self.validation, Validation::Invalid(_)) {
                let y = ti_parts.field.y.saturating_add(1);
                let meter = if state.pending {
                    String::new()
                } else {
                    self.strength.meter(false)
                };
                let mut x = area.x;
                if !meter.is_empty() {
                    let w = crate::text::display_cols(&meter) as u16;
                    buffer.set_stringn(
                        x,
                        y,
                        &meter,
                        usize::from(w),
                        self.system.style(self.strength.role()),
                    );
                    x = x.saturating_add(w.saturating_add(1));
                }
                let room = area.right().saturating_sub(x);
                buffer.set_stringn(
                    x,
                    y,
                    take_display_cols(status, usize::from(room)),
                    usize::from(room),
                    self.system.style(Role::TextMuted),
                );
            }
        }

        PasswordInputParts {
            root: area,
            field: ti_parts.field,
            reveal: reveal_rect,
            cursor: ti_parts.cursor,
        }
    }

    /// Semantic registration — **never** includes the secret value.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &PasswordInputState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = if state.pending {
            "password pending"
        } else if state.is_empty() {
            "password empty"
        } else if state.is_revealed() {
            "password revealed"
        } else {
            "password masked"
        };
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "password"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    invalid: matches!(self.validation, Validation::Invalid(_))
                        || state.validity() == TextInputValidity::Forbidden,
                    busy: state.pending,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &PasswordInput<'_> {
    type State = PasswordInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for PasswordInput<'_> {
    type State = PasswordInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{MASK_CELLS, RolePalette};
    use crate::widgets::edit_core;
    use crate::widgets::tests::click;

    #[test]
    fn debug_never_contains_secret() {
        let state = PasswordInputState::with_secret("hunter2-super-secret");
        let dbg = format!("{state:?}");
        assert!(!dbg.contains("hunter2"), "debug leaked: {dbg}");
        assert!(!dbg.contains("super-secret"), "debug leaked: {dbg}");
        assert!(dbg.contains("PasswordInputState"));
        assert!(dbg.contains("filled: true"));
    }

    #[test]
    fn outcome_submitted_has_no_secret_payload() {
        let mut state = PasswordInputState::with_secret("abc");
        state.set_focused(true);
        state.begin_edit();
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(out, PasswordInputOutcome::Submitted);
        let s = format!("{out:?}");
        assert!(!s.contains("abc"));
    }

    #[test]
    fn clipboard_copy_denied_by_default() {
        let mut state = PasswordInputState::with_secret("secret");
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            PasswordInputOutcome::ClipboardDenied
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            PasswordInputOutcome::ClipboardDenied
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
            PasswordInputOutcome::ClipboardPasteRequest
        );
    }

    #[test]
    fn allow_host_copy_signals_without_embedding() {
        let mut state = PasswordInputState::with_secret("secret")
            .with_clipboard_policy(ClipboardPolicy::AllowHostCopy);
        state.set_focused(true);
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(out, PasswordInputOutcome::ClipboardCopyAllowed);
        assert!(!format!("{out:?}").contains("secret"));
    }

    #[test]
    fn deny_all_blocks_paste() {
        let mut state = PasswordInputState::new().with_clipboard_policy(ClipboardPolicy::DenyAll);
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::CONTROL)),
            PasswordInputOutcome::ClipboardDenied
        );
        assert_eq!(
            state.insert_str("paste"),
            PasswordInputOutcome::ClipboardDenied
        );
    }

    #[test]
    fn paint_masks_secret() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state = PasswordInputState::with_secret("hunter2");
        state.set_focused(true);
        let area = Rect::new(0, 0, 24, 2);
        let mut buf = Buffer::empty(area);
        let _ = PasswordInput::new("Password", &system).paint(area, &mut buf, &mut state);
        let mut row = String::new();
        for x in 0..area.width {
            row.push_str(buf[(x, 1)].symbol());
        }
        assert!(!row.contains('h'), "painted secret: {row:?}");
        assert!(!row.contains("hunter"), "painted secret: {row:?}");
        assert!(
            row.contains(&"●".repeat(MASK_CELLS)),
            "expected {MASK_CELLS} mask glyphs: {row:?}"
        );
        assert!(
            !row.contains(&"●".repeat(MASK_CELLS + 1)),
            "mask must not track secret length: {row:?}"
        );
    }

    #[test]
    fn masked_paint_is_field_plane_and_eight_dots() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let mut state = PasswordInputState::with_secret("hunter2");
        state.set_focused(true);
        let area = Rect::new(0, 0, 28, 3);
        let mut buf = Buffer::empty(area);
        let _ = PasswordInput::new("Password", &system).paint(area, &mut buf, &mut state);
        let field_y = 1u16;
        assert_eq!(buf[(0, field_y)].symbol(), "▎");
        assert_eq!(buf[(0, field_y)].fg, theme.accent);
        assert_eq!(buf[(0, field_y)].bg, theme.field);
        let row: String = (0..area.width)
            .map(|x| buf[(x, field_y)].symbol().to_string())
            .collect();
        assert!(row.contains(&"●".repeat(MASK_CELLS)), "{row:?}");
        assert!(!row.contains('h'), "{row:?}");
    }

    #[test]
    fn semantic_description_has_no_secret() {
        let system = DesignSystem::default();
        let state = PasswordInputState::with_secret("top-secret-value");
        let mut scene = SemanticScene::<&str, ()>::default();
        PasswordInput::new("Password", &system).register_semantic(
            &mut scene,
            "pw",
            Rect::new(0, 0, 20, 2),
            &state,
        );
        let node = scene.get(&"pw").expect("registered");
        let dump = format!("{node:?}");
        assert!(!dump.contains("top-secret"));
        assert!(!dump.contains("top-secret-value"));
    }

    #[test]
    fn explicit_reveal_toggles() {
        let mut state =
            PasswordInputState::with_secret("ab").with_reveal_policy(RevealPolicy::Explicit);
        state.set_focused(true);
        assert!(!state.is_revealed());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT)),
            PasswordInputOutcome::RevealChanged { revealed: true }
        );
        assert!(state.is_revealed());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PasswordInputOutcome::Cancelled
        );
    }

    #[test]
    fn repeated_lifecycle_reveal_and_clipboard_actions_are_ignored() {
        let mut state =
            PasswordInputState::with_secret("abc").with_reveal_policy(RevealPolicy::Explicit);
        state.set_focused(true);
        state.begin_edit();
        let actions = [
            (KeyCode::Char('r'), KeyModifiers::ALT),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::BackTab, KeyModifiers::NONE),
            (KeyCode::Char('m'), KeyModifiers::CONTROL),
            (
                KeyCode::Char('m'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            (KeyCode::Char('k'), KeyModifiers::CONTROL),
            (KeyCode::Char('u'), KeyModifiers::CONTROL),
            (KeyCode::Char('w'), KeyModifiers::CONTROL),
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
            (KeyCode::Char('v'), KeyModifiers::CONTROL),
        ];
        for (code, modifiers) in actions {
            let before = state.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(state.handle_key(key), PasswordInputOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated password state");
        }
    }

    #[test]
    fn mouse_reveal_uses_explicit_policy_and_exact_hit_region() {
        let mut state =
            PasswordInputState::with_secret("ab").with_reveal_policy(RevealPolicy::Explicit);
        let reveal = Rect::new(12, 3, 2, 1);
        assert_eq!(
            state.handle_mouse(
                click(reveal.x, reveal.y),
                Rect::new(0, 3, 12, 1),
                Some(reveal),
            ),
            PasswordInputOutcome::RevealChanged { revealed: true }
        );
        assert!(state.is_revealed());
    }

    #[test]
    fn hold_reveal_press_release() {
        let mut state =
            PasswordInputState::with_secret("ab").with_reveal_policy(RevealPolicy::Hold);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT)),
            PasswordInputOutcome::RevealChanged { revealed: true }
        );
        assert!(state.is_revealed());
        let mut repeat = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT);
        repeat.kind = KeyEventKind::Repeat;
        assert_eq!(state.handle_key(repeat), PasswordInputOutcome::Ignored);
        assert!(state.is_revealed());
        let mut release = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::ALT);
        release.kind = KeyEventKind::Release;
        assert_eq!(
            state.handle_key(release),
            PasswordInputOutcome::RevealChanged { revealed: false }
        );
        assert!(!state.is_revealed());
    }

    #[test]
    fn confirm_pair_match() {
        let mut pair = PasswordConfirmState::new();
        let _ = pair.password.insert_str("same");
        let _ = pair.confirm.insert_str("same");
        assert!(pair.secrets_match());
        let _ = pair.confirm.insert_str("x");
        assert!(pair.is_mismatch());
    }

    #[test]
    fn take_secret_clears() {
        let mut state = PasswordInputState::with_secret("xyz");
        let taken = state.take_secret();
        assert_eq!(taken, "xyz");
        assert!(state.is_empty());
    }

    #[test]
    fn disabled_and_pending_block_edit() {
        let mut state = PasswordInputState::new();
        state.set_focused(true);
        state.set_enabled(false);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            PasswordInputOutcome::Ignored
        );
        state.set_enabled(true);
        state.set_pending(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)),
            PasswordInputOutcome::Ignored
        );
    }

    #[test]
    fn read_only_allows_nav_not_insert() {
        let mut state = PasswordInputState::with_secret("ab");
        state.set_focused(true);
        state.begin_edit();
        state.set_read_only(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)),
            PasswordInputOutcome::Ignored
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            PasswordInputOutcome::Changed
        );
    }

    #[test]
    fn invalid_validation_paint_safe() {
        let system = DesignSystem::default();
        let mut state = PasswordInputState::with_secret("x");
        state.set_focused(true);
        let area = Rect::new(0, 0, 30, 3);
        let mut buf = Buffer::empty(area);
        let _ = PasswordInput::new("Password", &system)
            .validation(Validation::Invalid("too short"))
            .strength(PasswordStrengthHint::Weak)
            .paint(area, &mut buf, &mut state);
        let mut all = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                all.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(!all.contains("secret"));
        assert!(all.contains("too") || all.contains("short") || all.contains('*'));
    }

    #[test]
    fn unicode_fuzz_keeps_boundary() {
        let mut state = PasswordInputState::with_secret("e\u{301}東京");
        state.set_focused(true);
        state.begin_edit();
        let keys = [
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('あ'), KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(30) {
            let _ = state.handle_key(*key);
            let v = state.secret();
            let c = state.editor.cursor_byte();
            assert!(edit_core::is_boundary(v, c), "cursor {c} in {v:?}");
        }
    }

    #[test]
    fn paint_hot_path_masked() {
        let system = DesignSystem::default();
        let mut state = PasswordInputState::with_secret("x".repeat(64));
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        let widget = PasswordInput::new("Password", &system);
        for _ in 0..200 {
            let _ = widget.paint(area, &mut buf, &mut state);
        }
    }
}
