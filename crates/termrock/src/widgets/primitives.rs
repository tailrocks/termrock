// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal-native activation and pure chrome primitives (Plan 050).
//!
//! **Law:** Enter/Space or one pointer gesture activates once; disabled and
//! loading never activate. Press confirms; Release never activates. Effects
//! remain consumer-owned outcomes.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::HitRegion,

    runtime::FrameTick,
    style::{DesignSystem, Motion, Role},
    text::{display_cols, take_display_cols},
};

// ── Shared activation ───────────────────────────────────────────────────────

/// Typed result of an activation gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActivationOutcome {
    /// Gesture did not activate.
    #[default]
    Ignored,
    /// Control activated once (or confirm after pending).
    Activated,
    /// First press while pending-confirmation required (await second Activate).
    ConfirmRequired,
    /// Visual press/arm state changed without activation.
    Pressed,
}

impl ActivationOutcome {
    /// Wraps in the standard [`crate::interaction::EventResult`] envelope.
    #[must_use]
    pub fn into_event_result(self) -> crate::interaction::EventResult<Self> {
        match self {
            Self::Ignored => crate::interaction::EventResult::ignored(),
            other => crate::interaction::EventResult::emit(other),
        }
    }
}

/// Armed/loading/disabled activation model shared by Button and IconButton.
///
/// **Input gate** is [`Self::set_accepts_input`] (host/scene ownership). Do not
/// treat it as a second scene focus authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActivationState {
    enabled: bool,
    loading: bool,
    /// Host grants keyboard/pointer (overlay/scene ownership).
    accepts_input: bool,
    /// Pointer or Space is armed (await Up / release).
    armed: bool,
    /// Requires two Activate intents (destructive confirm).
    pending_confirmation: bool,
    /// First activate received; next Activate fires.
    confirm_armed: bool,
}

impl ActivationState {
    /// Enabled, not loading, does not accept input until host grants.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: true,
            loading: false,
            accepts_input: false,
            armed: false,
            pending_confirmation: false,
            confirm_armed: false,
        }
    }

    /// Whether the control can activate.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.loading && self.accepts_input
    }

    /// Host input gate without side effects (draft/state preserved elsewhere).
    pub const fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
        if !accepts {
            self.armed = false;
            self.confirm_armed = false;
        }
    }

    /// Deprecated name for [`Self::set_accepts_input`].
    #[deprecated(note = "use set_accepts_input")]
    pub const fn set_focused(&mut self, focused: bool) {
        self.set_accepts_input(focused);
    }

    /// Enabled flag.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.armed = false;
            self.confirm_armed = false;
        }
    }

    /// Loading flag (blocks activation; distinct from disabled paint).
    pub const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.armed = false;
            self.confirm_armed = false;
        }
    }

    /// Require two Activate intents (e.g. destructive).
    pub const fn set_pending_confirmation(&mut self, pending: bool) {
        self.pending_confirmation = pending;
        if !pending {
            self.confirm_armed = false;
        }
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Deprecated name for [`Self::accepts_input`].
    #[deprecated(note = "use accepts_input")]
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.accepts_input
    }

    #[must_use]
    /// Enabled.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    /// Loading (not the same as disabled).
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Whether pointer/Space is armed.
    #[must_use]
    pub const fn is_armed(&self) -> bool {
        self.armed
    }

    /// Awaiting second confirm.
    #[must_use]
    pub const fn is_confirm_armed(&self) -> bool {
        self.confirm_armed
    }

    fn fire_or_confirm(&mut self) -> ActivationOutcome {
        if self.pending_confirmation && !self.confirm_armed {
            self.confirm_armed = true;
            return ActivationOutcome::ConfirmRequired;
        }
        self.confirm_armed = false;
        self.armed = false;
        ActivationOutcome::Activated
    }

    /// Semantic intent routing (prefer over raw Enter/Space matching).
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> ActivationOutcome {
        if !self.can_activate() {
            return ActivationOutcome::Ignored;
        }
        match intent {
            crate::interaction::UiIntent::Activate
            | crate::interaction::UiIntent::Submit
            | crate::interaction::UiIntent::Toggle => self.fire_or_confirm(),
            _ => ActivationOutcome::Ignored,
        }
    }

    /// Keyboard activation via [`crate::interaction::default_button_intent`].
    ///
    /// Enter activates on Press. Space arms on Press and activates on Release
    /// (hold-repeat does not multi-fire).
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        if !self.can_activate() {
            return ActivationOutcome::Ignored;
        }
        // Space: arm on press, fire on release (desktop dialog discipline).
        if matches!(key.code, KeyCode::Char(' ')) {
            match key.kind {
                KeyEventKind::Press => {
                    self.armed = true;
                    return ActivationOutcome::Pressed;
                }
                KeyEventKind::Release if self.armed => {
                    self.armed = false;
                    return self.fire_or_confirm();
                }
                KeyEventKind::Repeat | KeyEventKind::Release => {
                    return ActivationOutcome::Ignored;
                }
                _ => {}
            }
        }
        if key.kind == KeyEventKind::Release || key.kind == KeyEventKind::Repeat {
            return ActivationOutcome::Ignored;
        }
        if let Some(intent) = crate::interaction::default_button_intent(key) {
            return self.handle_intent(intent);
        }
        ActivationOutcome::Ignored
    }

    /// Key path with [`crate::interaction::EventResult`].
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
    ) -> crate::interaction::EventResult<ActivationOutcome> {
        self.handle_key(key).into_event_result()
    }

    /// Intent path with [`crate::interaction::EventResult`].
    pub fn handle_intent_result(
        &mut self,
        intent: crate::interaction::UiIntent,
    ) -> crate::interaction::EventResult<ActivationOutcome> {
        self.handle_intent(intent).into_event_result()
    }

    /// Pointer activation: Down arms; Up inside region activates once.
    pub fn handle_mouse(&mut self, event: MouseEvent, region: Option<Rect>) -> ActivationOutcome {
        if !self.can_activate() {
            return ActivationOutcome::Ignored;
        }
        let Some(area) = region else {
            self.armed = false;
            return ActivationOutcome::Ignored;
        };
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if area.contains(event.position) => {
                self.armed = true;
                ActivationOutcome::Pressed
            }
            MouseEventKind::Up(MouseButton::Left)
                if self.armed && area.contains(event.position) =>
            {
                self.armed = false;
                self.fire_or_confirm()
            }
            MouseEventKind::Up(_) | MouseEventKind::Moved | MouseEventKind::Drag(_) => {
                if !area.contains(event.position) {
                    self.armed = false;
                }
                ActivationOutcome::Ignored
            }
            _ => ActivationOutcome::Ignored,
        }
    }
}

// ── Button / IconButton ─────────────────────────────────────────────────────

/// Visual / semantic button chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonVariant {
    /// Strong recommended action (Accent).
    Primary,
    /// Default secondary action.
    #[default]
    Secondary,
    /// Quiet / ghost (minimal chrome).
    Quiet,
    /// Outline (bracket + role border).
    Outline,
    /// Destructive (Danger); must not be unsafe default focus.
    Destructive,
    /// Link-like (underline / Link role).
    Link,
    /// Success-affirming action.
    Success,
    /// Command / palette-style action.
    Command,
}

/// Horizontal density of button chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ButtonSize {
    /// Tighter pad.
    Compact,
    /// Default pad.
    #[default]
    Normal,
}

/// Canonical primary action control.
///
/// Outcomes are pure ([`ActivationOutcome`]); effects stay consumer-owned.
#[derive(Debug, Clone, Copy)]
pub struct Button<'a> {
    label: &'a str,
    system: &'a DesignSystem,
    variant: ButtonVariant,
    size: ButtonSize,
    leading: Option<&'a str>,
    trailing: Option<&'a str>,
    full_width: bool,
    /// Required when label is empty (icon-only).
    accessible_label: Option<&'a str>,
    ascii: bool,
    colorless: bool,
}

impl<'a> Button<'a> {
    /// Label + design system.
    #[must_use]
    pub const fn new(label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            label,
            system,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Normal,
            leading: None,
            trailing: None,
            full_width: false,
            accessible_label: None,
            ascii: false,
            colorless: false,
        }
    }

    /// Variant chrome.
    #[must_use]
    pub const fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Emphasize as primary (compat helper).
    #[must_use]
    pub const fn primary(mut self, primary: bool) -> Self {
        if primary {
            self.variant = ButtonVariant::Primary;
        }
        self
    }

    /// Size / pad.
    #[must_use]
    pub const fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Leading glyph (dropped before label on narrow).
    #[must_use]
    pub const fn leading(mut self, glyph: &'a str) -> Self {
        self.leading = Some(glyph);
        self
    }

    /// Trailing glyph (dropped first on narrow).
    #[must_use]
    pub const fn trailing(mut self, glyph: &'a str) -> Self {
        self.trailing = Some(glyph);
        self
    }

    /// Fill paint width.
    #[must_use]
    pub const fn full_width(mut self, full: bool) -> Self {
        self.full_width = full;
        self
    }

    /// Accessible name (required for empty / icon-only labels).
    #[must_use]
    pub const fn accessible_label(mut self, label: &'a str) -> Self {
        self.accessible_label = Some(label);
        self
    }

    /// ASCII chrome (brackets, loading `...`).
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Whether this variant is a safe dialog default focus candidate.
    ///
    /// **Destructive returns false** — host must not default-focus it.
    #[must_use]
    pub const fn is_safe_default_focus(self) -> bool {
        !matches!(self.variant, ButtonVariant::Destructive)
    }

    /// Semantic name for a11y / HintBar (label or accessible_label).
    #[must_use]
    pub fn a11y_name(&self) -> &str {
        if !self.label.is_empty() {
            self.label
        } else {
            self.accessible_label.unwrap_or("")
        }
    }

    /// Preferred width in cells (label + chrome).
    #[must_use]
    pub fn preferred_width(&self) -> u16 {
        let pad = match self.size {
            ButtonSize::Compact => 2usize,
            ButtonSize::Normal => 4,
        };
        let mut w = display_cols(self.label).saturating_add(pad);
        if let Some(g) = self.leading {
            w = w.saturating_add(display_cols(g).saturating_add(1));
        }
        if let Some(g) = self.trailing {
            w = w.saturating_add(display_cols(g).saturating_add(1));
        }
        w.min(u16::MAX as usize) as u16
    }

    fn base_role(&self) -> Role {
        match self.variant {
            ButtonVariant::Primary | ButtonVariant::Command => Role::Accent,
            ButtonVariant::Destructive => Role::Danger,
            ButtonVariant::Success => Role::Success,
            ButtonVariant::Link => Role::Link,
            ButtonVariant::Secondary | ButtonVariant::Quiet | ButtonVariant::Outline => Role::Text,
        }
    }
}

/// Stateful paint for [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonState {
    /// Activation model.
    pub activation: ActivationState,
    /// Last painted hit region.
    pub region: Option<Rect>,
}

impl ButtonState {
    /// Fresh state (does **not** accept input until host grants).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            activation: ActivationState::new(),
            region: None,
        }
    }

    /// Key routing (intent-backed).
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        self.activation.handle_key(key)
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> ActivationOutcome {
        self.activation.handle_intent(intent)
    }

    /// Mouse routing against last paint.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> ActivationOutcome {
        self.activation.handle_mouse(event, self.region)
    }
}

impl Button<'_> {
    /// Paint button and update hit region on state.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ButtonState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        // Icon-only without a11y name: refuse silent activation paint (dim + !).
        let a11y_ok = !self.label.is_empty() || self.accessible_label.is_some();
        let theme = self.system;
        let loading = state.activation.is_loading();
        let disabled = !state.activation.is_enabled();
        let surface = state.activation.accepts_input() && !disabled;
        let armed = state.activation.is_armed() || state.activation.is_confirm_armed();

        let style = if !a11y_ok {
            theme.style(Role::Danger)
        } else if disabled {
            theme.style(Role::ActionDisabled)
        } else if loading {
            // Loading distinct from disabled: muted strong, not ActionDisabled alone.
            if self.colorless {
                theme.style(Role::TextMuted)
            } else {
                theme.style(Role::Info)
            }
        } else if surface && armed {
            theme.style(Role::ActionFocused)
        } else if surface {
            theme.style(Role::ActionFocused)
        } else if self.colorless {
            match self.variant {
                ButtonVariant::Primary | ButtonVariant::Destructive | ButtonVariant::Success => {
                    theme.style(Role::TextStrong)
                }
                _ => theme.style(Role::Text),
            }
        } else {
            theme.style(self.base_role())
        };

        let narrow = area.width < 12;
        let tiny = area.width < 6;
        let show_trailing = self.trailing.is_some() && !narrow && !tiny;
        let show_leading =
            self.leading.is_some() && !tiny && (area.width >= 10 || self.label.is_empty());

        let load_g = if self.ascii { "..." } else { "…" };
        let mut body = String::new();
        if loading {
            body.push_str(load_g);
            body.push(' ');
        } else if show_leading {
            if let Some(g) = self.leading {
                body.push_str(g);
                body.push(' ');
            }
        }
        if tiny && !self.label.is_empty() {
            // Prefer start of label.
            body.push_str(self.label);
        } else if self.label.is_empty() {
            // Icon-only: show leading or a11y mark.
            if let Some(g) = self.leading {
                body.push_str(g);
            } else if !a11y_ok {
                body.push_str(if self.ascii { "[!]" } else { "⚠" });
            }
        } else {
            body.push_str(self.label);
        }
        if show_trailing {
            if let Some(g) = self.trailing {
                body.push(' ');
                body.push_str(g);
            }
        }
        if state.activation.is_confirm_armed() {
            body.push_str(if self.ascii { " ?" } else { " ?" });
        }

        let (open, close) = match self.variant {
            ButtonVariant::Outline | ButtonVariant::Command if self.ascii || self.colorless => {
                ("[", "]")
            }
            ButtonVariant::Outline | ButtonVariant::Command => ("[ ", " ]"),
            ButtonVariant::Link => ("", ""),
            ButtonVariant::Quiet => (" ", " "),
            ButtonVariant::Destructive if self.colorless || self.ascii => ("!", " "),
            _ => (" ", " "),
        };
        // Affordance is role + pad + optional outline — not brackets alone.
        let label = format!("{open}{body}{close}");
        let paint_w = if self.full_width {
            area.width
        } else {
            area.width.min(self.preferred_width().max(3))
        };
        let text = take_display_cols(&label, usize::from(paint_w));
        buffer.set_stringn(area.x, area.y, &text, usize::from(paint_w), style);
        // Link underline cue via trailing underscore when focused and colorless.
        if matches!(self.variant, ButtonVariant::Link) && surface && self.colorless && paint_w > 1 {
            let y = area.y;
            let x = area.x.saturating_add(1);
            if x < area.right() {
                buffer[(x, y)].set_style(theme.style(Role::TextStrong));
            }
        }
        state.region = Some(Rect::new(area.x, area.y, paint_w, 1));
    }
}

impl StatefulWidget for Button<'_> {
    type State = ButtonState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Button::render(&self, area, buffer, state);
    }
}

impl StatefulWidget for &Button<'_> {
    type State = ButtonState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Button::render(self, area, buffer, state);
    }
}

/// Icon-only button; **requires** [`Self::accessible_label`].
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    glyph: &'a str,
    system: &'a DesignSystem,
    accessible_label: &'a str,
    variant: ButtonVariant,
    ascii: bool,
    colorless: bool,
}

impl<'a> IconButton<'a> {
    /// Glyph + required accessible name.
    #[must_use]
    pub const fn new(glyph: &'a str, accessible_label: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            glyph,
            system,
            accessible_label,
            variant: ButtonVariant::Quiet,
            ascii: false,
            colorless: false,
        }
    }

    /// Variant (quiet/primary/destructive common).
    #[must_use]
    pub const fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// ASCII glyph path.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Accessible name for HintBar / a11y.
    #[must_use]
    pub const fn a11y_name(&self) -> &'a str {
        self.accessible_label
    }
}

/// State for [`IconButton`] (same activation law as Button).
pub type IconButtonState = ButtonState;

impl IconButton<'_> {
    /// Paint icon button (min 3-cell hit).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut IconButtonState) {
        // Reuse Button paint with empty label + leading glyph + a11y.
        Button::new("", self.system)
            .variant(self.variant)
            .leading(self.glyph)
            .accessible_label(self.accessible_label)
            .ascii(self.ascii)
            .colorless(self.colorless)
            .render(area, buffer, state);
        // Enforce min hit width 3.
        if let Some(r) = state.region.as_mut() {
            r.width = r.width.max(3).min(area.width);
        }
    }
}

// ── Badge / Tag / Chip ──────────────────────────────────────────────────────
// Badge: `widgets/badge.rs`. Tag / Chip / TokenStrip: `widgets/tag_chip.rs`.

// ── Kbd / Separator / Spinner ───────────────────────────────────────────────
// Kbd + ShortcutHint live in `widgets/kbd.rs`.

// Separator lives in `widgets/separator.rs` (variants, labels, ASCII glyphs).

/// FrameTick-driven spinner frames.
#[derive(Debug, Clone, Copy)]
pub struct Spinner<'a> {
    tokens: &'a DesignSystem,
    ascii: bool,
}

impl<'a> Spinner<'a> {
    /// Unicode braille spinner by default.
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self {
            tokens,
            ascii: false,
        }
    }

    /// ASCII fallback frames.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Frame glyph for tick + motion policy.
    #[must_use]
    pub fn frame_glyph(&self, tick: FrameTick, motion: Motion) -> &'static str {
        let frames: &[&str] = if self.ascii {
            &["|", "/", "-", "\\"]
        } else {
            &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        };
        if !motion.animate_spinners() {
            return if self.ascii { "o" } else { "●" };
        }
        let step = tick.spinner_step(frames.len(), 80, motion);
        frames[step]
    }

    /// Paint spinner at `area`.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, tick: FrameTick, motion: Motion) {
        if area.is_empty() {
            return;
        }
        let g = self.frame_glyph(tick, motion);
        buffer.set_stringn(
            area.x,
            area.y,
            g,
            usize::from(area.width),
            self.tokens.style(Role::TextMuted),
        );
    }
}

/// Hit helper for button registration.
#[must_use]
pub fn button_hit<Id: Clone>(id: Id, state: &ButtonState) -> Option<HitRegion<Id>> {
    state.region.map(|area| HitRegion { id, area })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use ratatui_core::layout::Position;
    use std::time::{Duration, Instant};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn button_enter_activates_once_when_accepts_input() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        assert_eq!(
            state.handle_key(press(KeyCode::Enter)),
            ActivationOutcome::Activated
        );
        // Repeat must not multi-fire
        let mut rep = press(KeyCode::Enter);
        rep.kind = KeyEventKind::Repeat;
        assert_eq!(state.handle_key(rep), ActivationOutcome::Ignored);
    }

    #[test]
    fn button_disabled_never_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_enabled(false);
        assert_eq!(
            state.handle_key(press(KeyCode::Enter)),
            ActivationOutcome::Ignored
        );
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Ignored
        );
    }

    #[test]
    fn button_loading_never_activates_and_differs_from_disabled() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_loading(true);
        assert!(state.activation.is_enabled());
        assert!(state.activation.is_loading());
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Ignored
        );
    }

    #[test]
    fn button_space_arms_then_release_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Pressed
        );
        let mut rel = press(KeyCode::Char(' '));
        rel.kind = KeyEventKind::Release;
        assert_eq!(state.handle_key(rel), ActivationOutcome::Activated);
    }

    #[test]
    fn button_pending_confirmation_requires_two_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_pending_confirmation(true);
        assert_eq!(
            state.handle_intent(crate::interaction::UiIntent::Activate),
            ActivationOutcome::ConfirmRequired
        );
        assert_eq!(
            state.handle_intent(crate::interaction::UiIntent::Activate),
            ActivationOutcome::Activated
        );
    }

    #[test]
    fn destructive_is_not_safe_default_focus() {
        let system = DesignSystem::default();
        assert!(
            Button::new("Save", &system)
                .variant(ButtonVariant::Primary)
                .is_safe_default_focus()
        );
        assert!(
            !Button::new("Delete", &system)
                .variant(ButtonVariant::Destructive)
                .is_safe_default_focus()
        );
    }

    #[test]
    fn button_mouse_down_up_activates() {
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        state.activation.set_enabled(true);
        state.region = Some(Rect::new(0, 0, 8, 1));
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(down), ActivationOutcome::Pressed);
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(up), ActivationOutcome::Activated);
    }

    #[test]
    fn button_paint_variants_and_narrow() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(area);
        Button::new("Save", &system)
            .variant(ButtonVariant::Primary)
            .leading("✓")
            .render(area, &mut buffer, &mut state);
        assert!(state.region.is_some());
        let text: String = (0..20)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(text.contains("Save"), "{text:?}");

        let tiny = Rect::new(0, 0, 5, 1);
        let mut tbuf = Buffer::empty(tiny);
        Button::new("SaveChanges", &system).render(tiny, &mut tbuf, &mut state);
        assert!(state.region.is_some());
    }

    #[test]
    fn icon_button_requires_accessible_label() {
        let system = DesignSystem::default();
        let mut state = ButtonState::new();
        state.activation.set_accepts_input(true);
        let area = Rect::new(0, 0, 4, 1);
        let mut buffer = Buffer::empty(area);
        IconButton::new("×", "Close", &system).render(area, &mut buffer, &mut state);
        assert_eq!(IconButton::new("×", "Close", &system).a11y_name(), "Close");
        assert!(state.region.map(|r| r.width >= 3).unwrap_or(false));
    }

    #[test]
    fn chip_toggle_space() {
        use crate::widgets::{Chip, ChipOutcome, ChipState};
        let system = DesignSystem::default();
        let chip = Chip::new("f1", "rust", &system);
        let mut state = ChipState::new(false);
        state.set_focused(true);
        assert!(matches!(
            chip.handle_key(&mut state, press(KeyCode::Char(' '))),
            ChipOutcome::Selected("f1")
        ));
        assert!(state.is_selected());
        assert!(matches!(
            chip.handle_key(&mut state, press(KeyCode::Char(' '))),
            ChipOutcome::Unselected("f1")
        ));
    }

    #[test]
    fn tag_remove_on_delete() {
        use crate::widgets::{Tag, TagOutcome, TagState};
        let system = DesignSystem::default();
        let tag = Tag::removable_tag("t", "file", &system);
        let mut state = TagState::new();
        state.set_focused(true);
        assert!(matches!(
            tag.handle_key(&mut state, press(KeyCode::Delete)),
            TagOutcome::Remove("t")
        ));
    }

    #[test]
    fn spinner_motion_off_static() {
        let tokens = DesignSystem::default();
        let spinner = Spinner::new(&tokens);
        let now = Instant::now();
        let tick = FrameTick::manual(now, Duration::from_millis(560), Duration::from_millis(16));
        assert_eq!(spinner.frame_glyph(tick, Motion::Off), "●");
        let a = spinner.frame_glyph(tick, Motion::Full);
        let b = spinner.frame_glyph(
            FrameTick::manual(now, Duration::from_millis(640), Duration::from_millis(16)),
            Motion::Full,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn badge_and_separator_paint() {
        let tokens = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        Widget::render(
            &crate::widgets::Badge::new("NEW", &tokens),
            Rect::new(0, 0, 10, 1),
            &mut buf,
        );
        Widget::render(
            &crate::widgets::Separator::horizontal(&tokens),
            Rect::new(0, 1, 20, 1),
            &mut buf,
        );
        assert!(!buf[(0, 0)].symbol().is_empty());
    }
}
