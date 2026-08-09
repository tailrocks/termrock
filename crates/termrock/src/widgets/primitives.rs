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
    keymap::KeyChord,
    runtime::FrameTick,
    style::{DesignTokens, Motion, Role},
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
    /// Control activated once.
    Activated,
}

/// Armed/loading/disabled activation model shared by Button and IconButton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ActivationState {
    enabled: bool,
    loading: bool,
    focused: bool,
    armed: bool,
}

impl ActivationState {
    /// Enabled, not loading, unfocused.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: true,
            loading: false,
            focused: false,
            armed: false,
        }
    }

    /// Whether the control can activate.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        self.enabled && !self.loading
    }

    /// Focus flag for chrome (consumer/scene-owned).
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.armed = false;
        }
    }

    /// Enabled flag.
    pub const fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.armed = false;
        }
    }

    /// Loading flag (blocks activation).
    pub const fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.armed = false;
        }
    }

    #[must_use]
    /// Focused.
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    #[must_use]
    /// Enabled.
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    /// Loading.
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Keyboard activation law.
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        if !self.can_activate() || !self.focused {
            return ActivationOutcome::Ignored;
        }
        match key.kind {
            KeyEventKind::Press => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => {
                    self.armed = false;
                    ActivationOutcome::Activated
                }
                _ => ActivationOutcome::Ignored,
            },
            KeyEventKind::Repeat | KeyEventKind::Release => ActivationOutcome::Ignored,
        }
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
                ActivationOutcome::Ignored
            }
            MouseEventKind::Up(MouseButton::Left)
                if self.armed && area.contains(event.position) =>
            {
                self.armed = false;
                ActivationOutcome::Activated
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

/// Product-neutral text button.
#[derive(Debug, Clone, Copy)]
pub struct Button<'a> {
    label: &'a str,
    tokens: &'a DesignTokens,
    primary: bool,
}

impl<'a> Button<'a> {
    /// Label + design tokens.
    #[must_use]
    pub const fn new(label: &'a str, tokens: &'a DesignTokens) -> Self {
        Self {
            label,
            tokens,
            primary: false,
        }
    }

    /// Emphasize as primary action (Accent role).
    #[must_use]
    pub const fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }

    /// Preferred width in cells (label + chrome).
    #[must_use]
    pub fn preferred_width(&self) -> u16 {
        (display_cols(self.label) + 4).min(u16::MAX as usize) as u16
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
    /// Fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            activation: ActivationState::new(),
            region: None,
        }
    }

    /// Key routing.
    pub fn handle_key(&mut self, key: KeyEvent) -> ActivationOutcome {
        self.activation.handle_key(key)
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
        let theme = &self.tokens.theme;
        let style = if !state.activation.is_enabled() || state.activation.is_loading() {
            theme.style(Role::ActionDisabled)
        } else if state.activation.is_focused() {
            theme.style(Role::ActionFocused)
        } else if self.primary {
            theme.style(Role::Accent)
        } else {
            theme.style(Role::Text)
        };
        let label = if state.activation.is_loading() {
            format!(" … {} ", self.label)
        } else {
            format!(" {} ", self.label)
        };
        let text = take_display_cols(&label, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        state.region = Some(Rect::new(
            area.x,
            area.y,
            area.width.min(self.preferred_width()),
            1,
        ));
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

/// Icon-only button (glyph + optional tooltip label for a11y string).
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    glyph: &'a str,
    tokens: &'a DesignTokens,
}

impl<'a> IconButton<'a> {
    /// Glyph cell(s).
    #[must_use]
    pub const fn new(glyph: &'a str, tokens: &'a DesignTokens) -> Self {
        Self { glyph, tokens }
    }
}

/// State for [`IconButton`] (same activation law as Button).
pub type IconButtonState = ButtonState;

impl IconButton<'_> {
    /// Paint icon button.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut IconButtonState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        let theme = &self.tokens.theme;
        let style = if !state.activation.is_enabled() {
            theme.style(Role::ActionDisabled)
        } else if state.activation.is_focused() {
            theme.style(Role::ActionFocused)
        } else {
            theme.style(Role::Text)
        };
        let text = take_display_cols(self.glyph, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        state.region = Some(Rect::new(area.x, area.y, 3.min(area.width), 1));
    }
}

// ── Badge / Tag / Chip ──────────────────────────────────────────────────────

/// Non-interactive status badge.
#[derive(Debug, Clone, Copy)]
pub struct Badge<'a> {
    label: &'a str,
    tokens: &'a DesignTokens,
    role: Role,
}

impl<'a> Badge<'a> {
    /// Label with info role by default.
    #[must_use]
    pub const fn new(label: &'a str, tokens: &'a DesignTokens) -> Self {
        Self {
            label,
            tokens,
            role: Role::Info,
        }
    }

    /// Semantic role for paint.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
}

impl Widget for &Badge<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let text = format!("[{}]", self.label);
        let text = take_display_cols(&text, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.theme.style(self.role),
        );
    }
}

/// Removable tag outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TagOutcome<Id> {
    /// No change.
    Ignored,
    /// Removal requested for stable id.
    Remove(Id),
}

/// Removable tag with stable id.
#[derive(Debug, Clone, Copy)]
pub struct Tag<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Label.
    label: &'a str,
    tokens: &'a DesignTokens,
    removable: bool,
}

impl<'a, Id: Clone> Tag<'a, Id> {
    /// Tag projection.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, tokens: &'a DesignTokens) -> Self {
        Self {
            id,
            label,
            tokens,
            removable: true,
        }
    }

    /// Whether Backspace/Delete or × activates remove.
    #[must_use]
    pub const fn removable(mut self, removable: bool) -> Self {
        self.removable = removable;
        self
    }
}

/// Tag interaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TagState {
    /// Focused for keyboard remove.
    pub focused: bool,
    /// Hit region of remove affordance.
    pub remove_region: Option<Rect>,
    /// Whole tag region.
    pub region: Option<Rect>,
}

impl TagState {
    /// Keyboard: Delete/Backspace removes when focused.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        id: &Id,
        removable: bool,
    ) -> TagOutcome<Id> {
        if !self.focused || !removable || key.kind != KeyEventKind::Press {
            return TagOutcome::Ignored;
        }
        match key.code {
            KeyCode::Delete | KeyCode::Backspace => TagOutcome::Remove(id.clone()),
            _ => TagOutcome::Ignored,
        }
    }

    /// Mouse: click remove region.
    pub fn handle_mouse<Id: Clone>(
        &mut self,
        event: MouseEvent,
        id: &Id,
        removable: bool,
    ) -> TagOutcome<Id> {
        if !removable || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return TagOutcome::Ignored;
        }
        if self
            .remove_region
            .is_some_and(|r| r.contains(event.position))
        {
            TagOutcome::Remove(id.clone())
        } else {
            TagOutcome::Ignored
        }
    }
}

impl<Id: Clone> Tag<'_, Id> {
    /// Paint tag.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut TagState) {
        state.region = None;
        state.remove_region = None;
        if area.is_empty() {
            return;
        }
        let style = if state.focused {
            self.tokens.theme.style(Role::Selection)
        } else {
            self.tokens.theme.style(Role::TextMuted)
        };
        let body = if self.removable {
            format!(" {} × ", self.label)
        } else {
            format!(" {} ", self.label)
        };
        let text = take_display_cols(&body, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        let w = display_cols(&text).min(usize::from(area.width)) as u16;
        state.region = Some(Rect::new(area.x, area.y, w, 1));
        if self.removable && w >= 2 {
            state.remove_region = Some(Rect::new(area.x + w.saturating_sub(2), area.y, 1, 1));
        }
    }
}

/// Toggle chip outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChipOutcome<Id> {
    /// No change.
    Ignored,
    /// Toggled to selected.
    Selected(Id),
    /// Toggled to unselected.
    Unselected(Id),
}

/// Toggleable filter chip.
#[derive(Debug, Clone, Copy)]
pub struct Chip<'a, Id> {
    /// Stable identity.
    pub id: Id,
    /// Label.
    label: &'a str,
    tokens: &'a DesignTokens,
}

impl<'a, Id> Chip<'a, Id> {
    /// Chip projection.
    #[must_use]
    pub const fn new(id: Id, label: &'a str, tokens: &'a DesignTokens) -> Self {
        Self { id, label, tokens }
    }
}

/// Chip selection state (controlled value owned by consumer via set/get).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChipState {
    selected: bool,
    focused: bool,
    region: Option<Rect>,
}

impl ChipState {
    /// Creates state with selection.
    #[must_use]
    pub const fn new(selected: bool) -> Self {
        Self {
            selected,
            focused: false,
            region: None,
        }
    }

    #[must_use]
    /// Selected.
    pub const fn is_selected(&self) -> bool {
        self.selected
    }

    /// Controlled set.
    pub const fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// Focus for keyboard toggle.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Toggle via Space/Enter when focused.
    pub fn handle_key<Id: Clone>(&mut self, key: KeyEvent, id: &Id) -> ChipOutcome<Id> {
        if !self.focused || key.kind != KeyEventKind::Press {
            return ChipOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.selected = !self.selected;
                if self.selected {
                    ChipOutcome::Selected(id.clone())
                } else {
                    ChipOutcome::Unselected(id.clone())
                }
            }
            _ => ChipOutcome::Ignored,
        }
    }

    /// Click toggles.
    pub fn handle_mouse<Id: Clone>(&mut self, event: MouseEvent, id: &Id) -> ChipOutcome<Id> {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return ChipOutcome::Ignored;
        }
        if self.region.is_some_and(|r| r.contains(event.position)) {
            self.selected = !self.selected;
            if self.selected {
                ChipOutcome::Selected(id.clone())
            } else {
                ChipOutcome::Unselected(id.clone())
            }
        } else {
            ChipOutcome::Ignored
        }
    }
}

impl<Id> Chip<'_, Id> {
    /// Paint chip.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ChipState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        let mark = if state.selected { "●" } else { "○" };
        let body = format!("{mark} {}", self.label);
        let style = if state.selected {
            self.tokens.theme.style(Role::Selection)
        } else if state.focused {
            self.tokens.theme.style(Role::Focus)
        } else {
            self.tokens.theme.style(Role::Text)
        };
        let text = take_display_cols(&body, usize::from(area.width));
        buffer.set_stringn(area.x, area.y, &text, usize::from(area.width), style);
        let w = display_cols(&text).min(usize::from(area.width)) as u16;
        state.region = Some(Rect::new(area.x, area.y, w, 1));
    }
}

// ── Kbd / Separator / Spinner ───────────────────────────────────────────────

/// Renders a key chord for hints (Keymap projection helper).
#[derive(Debug, Clone, Copy)]
pub struct Kbd<'a> {
    label: &'a str,
    tokens: &'a DesignTokens,
}

impl<'a> Kbd<'a> {
    /// Explicit chord label (e.g. from Keymap glyph).
    #[must_use]
    pub const fn new(label: &'a str, tokens: &'a DesignTokens) -> Self {
        Self { label, tokens }
    }

    /// Format a [`KeyChord`] into a short display label.
    #[must_use]
    pub fn from_chord(chord: KeyChord, buf: &'a mut String, tokens: &'a DesignTokens) -> Self {
        buf.clear();
        if chord.mods.contains(KeyModifiers::CONTROL) {
            buf.push_str("C-");
        }
        if chord.mods.contains(KeyModifiers::ALT) {
            buf.push_str("A-");
        }
        if chord.mods.contains(KeyModifiers::SHIFT) {
            buf.push_str("S-");
        }
        match chord.key {
            KeyCode::Char(c) => buf.push(c),
            KeyCode::Enter => buf.push_str("Enter"),
            KeyCode::Esc => buf.push_str("Esc"),
            KeyCode::Tab => buf.push_str("Tab"),
            KeyCode::Backspace => buf.push_str("BS"),
            KeyCode::Up => buf.push('↑'),
            KeyCode::Down => buf.push('↓'),
            KeyCode::Left => buf.push('←'),
            KeyCode::Right => buf.push('→'),
            other => buf.push_str(&format!("{other:?}")),
        }
        Self {
            label: buf.as_str(),
            tokens,
        }
    }
}

impl Widget for &Kbd<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let text = format!(" {} ", self.label);
        let text = take_display_cols(&text, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &text,
            usize::from(area.width),
            self.tokens.theme.style(Role::HintKey),
        );
    }
}

/// Separator with borrowed tokens (does not redefine focus borders).
#[derive(Debug, Clone, Copy)]
pub struct SeparatorLine<'a> {
    tokens: &'a DesignTokens,
    vertical: bool,
}

impl<'a> SeparatorLine<'a> {
    /// Horizontal rule.
    #[must_use]
    pub const fn horizontal(tokens: &'a DesignTokens) -> Self {
        Self {
            tokens,
            vertical: false,
        }
    }

    /// Vertical rule.
    #[must_use]
    pub const fn vertical(tokens: &'a DesignTokens) -> Self {
        Self {
            tokens,
            vertical: true,
        }
    }
}

/// Alias used in catalogs.
pub type Separator<'a> = SeparatorLine<'a>;

impl Widget for &SeparatorLine<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let style = self.tokens.theme.style(Role::Border);
        let glyph = if self.vertical { "│" } else { "─" };
        if self.vertical {
            for y in area.y..area.bottom() {
                buffer.set_stringn(area.x, y, glyph, 1, style);
            }
        } else {
            let line = glyph.repeat(usize::from(area.width));
            buffer.set_stringn(area.x, area.y, &line, usize::from(area.width), style);
        }
    }
}

/// FrameTick-driven spinner frames.
#[derive(Debug, Clone, Copy)]
pub struct Spinner<'a> {
    tokens: &'a DesignTokens,
    ascii: bool,
}

impl<'a> Spinner<'a> {
    /// Unicode braille spinner by default.
    #[must_use]
    pub const fn new(tokens: &'a DesignTokens) -> Self {
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
        if !motion.animate_spinners() {
            return if self.ascii { "o" } else { "●" };
        }
        let frames: &[&str] = if self.ascii {
            &["|", "/", "-", "\\"]
        } else {
            &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        };
        let step = (tick.elapsed().as_millis() / 80) as usize;
        frames[step % frames.len()]
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
            self.tokens.theme.style(Role::TextMuted),
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
    use crate::style::DesignTokens;
    use ratatui_core::layout::Position;
    use std::time::{Duration, Instant};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn button_enter_activates_once_when_focused() {
        let mut state = ButtonState::new();
        state.activation.set_focused(true);
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
        state.activation.set_focused(true);
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
    fn button_loading_never_activates() {
        let mut state = ButtonState::new();
        state.activation.set_focused(true);
        state.activation.set_loading(true);
        assert_eq!(
            state.handle_key(press(KeyCode::Char(' '))),
            ActivationOutcome::Ignored
        );
    }

    #[test]
    fn button_mouse_down_up_activates() {
        let mut state = ButtonState::new();
        state.activation.set_enabled(true);
        state.region = Some(Rect::new(0, 0, 8, 1));
        let down = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(down), ActivationOutcome::Ignored);
        let up = MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            position: Position::new(1, 0),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(state.handle_mouse(up), ActivationOutcome::Activated);
    }

    #[test]
    fn chip_toggle_space() {
        let mut state = ChipState::new(false);
        state.set_focused(true);
        let id = "f1";
        assert!(matches!(
            state.handle_key(press(KeyCode::Char(' ')), &id),
            ChipOutcome::Selected("f1")
        ));
        assert!(state.is_selected());
        assert!(matches!(
            state.handle_key(press(KeyCode::Char(' ')), &id),
            ChipOutcome::Unselected("f1")
        ));
    }

    #[test]
    fn tag_remove_on_delete() {
        let mut state = TagState {
            focused: true,
            ..Default::default()
        };
        assert!(matches!(
            state.handle_key(press(KeyCode::Delete), &"t", true),
            TagOutcome::Remove("t")
        ));
    }

    #[test]
    fn spinner_motion_off_static() {
        let tokens = DesignTokens::default();
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
        let tokens = DesignTokens::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        Widget::render(
            &Badge::new("NEW", &tokens),
            Rect::new(0, 0, 10, 1),
            &mut buf,
        );
        Widget::render(
            &SeparatorLine::horizontal(&tokens),
            Rect::new(0, 1, 20, 1),
            &mut buf,
        );
        assert!(!buf[(0, 0)].symbol().is_empty());
    }
}
