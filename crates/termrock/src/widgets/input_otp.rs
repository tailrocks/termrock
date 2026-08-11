// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **InputOtp** — one-time / PIN code entry for TUI (shadcn Input OTP peer).
//!
//! **Mission.** Fixed-length digit (or alphanumeric) slots with keyboard-first
//! entry, paste fill, caret navigation, and mask option. Host owns validation
//! and submission side effects.
//!
//! **vs TextInput.** Specialized slot chrome and auto-advance; never a free
//! multiline field.
//!
//! Research: shadcn Input OTP, CLI pin prompts, 2FA terminal flows.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{DesignSystem, Role},
    text::take_display_cols,
};

// ── Config ──────────────────────────────────────────────────────────────────

/// Character class accepted in each slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OtpCharset {
    /// Digits 0–9 only (default).
    #[default]
    Digits,
    /// ASCII letters + digits.
    Alphanumeric,
}

impl OtpCharset {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Digits => "digits",
            Self::Alphanumeric => "alphanumeric",
        }
    }

    /// Whether `c` is accepted.
    #[must_use]
    pub fn accepts(self, c: char) -> bool {
        match self {
            Self::Digits => c.is_ascii_digit(),
            Self::Alphanumeric => c.is_ascii_alphanumeric(),
        }
    }

    /// Normalize for storage (digits unchanged; alnum lowercased).
    #[must_use]
    pub fn normalize(self, c: char) -> char {
        match self {
            Self::Digits => c,
            Self::Alphanumeric => c.to_ascii_lowercase(),
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Host-facing outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputOtpOutcome {
    /// No change.
    Ignored,
    /// Digits/slots changed.
    Changed {
        /// Current joined value.
        value: String,
        /// Filled slot count.
        filled: usize,
    },
    /// All slots filled (auto or Enter).
    Completed {
        /// Full code.
        value: String,
    },
    /// Explicit submit with incomplete fill (host may reject).
    SubmittedIncomplete {
        /// Partial value.
        value: String,
    },
    /// Cleared.
    Cleared,
    /// Esc blur / cancel.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// OTP interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputOtpState {
    slots: Vec<Option<char>>,
    cursor: usize,
    length: usize,
    charset: OtpCharset,
    masked: bool,
    focused: bool,
    enabled: bool,
    accepts_input: bool,
}

impl Default for InputOtpState {
    fn default() -> Self {
        Self::new(6)
    }
}

impl InputOtpState {
    /// `length` slots (clamped 1..=12).
    #[must_use]
    pub fn new(length: usize) -> Self {
        let length = length.clamp(1, 12);
        Self {
            slots: vec![None; length],
            cursor: 0,
            length,
            charset: OtpCharset::Digits,
            masked: false,
            focused: true,
            enabled: true,
            accepts_input: true,
        }
    }

    /// Charset.
    pub fn set_charset(&mut self, c: OtpCharset) {
        self.charset = c;
    }

    /// Mask paint (`•` instead of char).
    pub fn set_masked(&mut self, on: bool) {
        self.masked = on;
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Length.
    #[must_use]
    pub const fn length(&self) -> usize {
        self.length
    }

    /// Cursor index.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether all slots filled (no holes).
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
    }

    /// Contiguous prefix until the first empty slot.
    ///
    /// Does **not** collapse holes: `[Some('1'), None, Some('3')]` → `"1"`,
    /// never `"13"`. Typing/paste fill left-to-right, so normal entry yields a
    /// dense prefix; navigated deletes can leave holes and stop the prefix.
    #[must_use]
    pub fn value(&self) -> String {
        self.slots
            .iter()
            .take_while(|s| s.is_some())
            .filter_map(|s| *s)
            .collect()
    }

    /// Contiguous filled count (length of [`Self::value`]), not total non-empty
    /// slots after a hole.
    #[must_use]
    pub fn filled(&self) -> usize {
        self.slots.iter().take_while(|s| s.is_some()).count()
    }

    /// Clear all.
    pub fn clear(&mut self) -> InputOtpOutcome {
        for s in &mut self.slots {
            *s = None;
        }
        self.cursor = 0;
        InputOtpOutcome::Cleared
    }

    /// Set value from string (truncates to length; filters charset).
    pub fn set_value(&mut self, raw: &str) -> InputOtpOutcome {
        for s in &mut self.slots {
            *s = None;
        }
        let mut i = 0;
        for c in raw.chars() {
            if i >= self.length {
                break;
            }
            if self.charset.accepts(c) {
                self.slots[i] = Some(self.charset.normalize(c));
                i += 1;
            }
        }
        self.cursor = i.min(self.length.saturating_sub(1));
        if self.is_complete() {
            InputOtpOutcome::Completed {
                value: self.value(),
            }
        } else {
            InputOtpOutcome::Changed {
                value: self.value(),
                filled: self.filled(),
            }
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent) -> InputOtpOutcome {
        if !self.enabled || !self.accepts_input || !self.focused || key.kind != KeyEventKind::Press
        {
            return InputOtpOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => InputOtpOutcome::Cancelled,
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                InputOtpOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Tab if key.modifiers.is_empty() => {
                if self.cursor + 1 < self.length {
                    self.cursor += 1;
                }
                InputOtpOutcome::Ignored
            }
            KeyCode::Home => {
                self.cursor = 0;
                InputOtpOutcome::Ignored
            }
            KeyCode::End => {
                self.cursor = self.length.saturating_sub(1);
                InputOtpOutcome::Ignored
            }
            KeyCode::Backspace => {
                if self.slots[self.cursor].is_some() {
                    self.slots[self.cursor] = None;
                } else if self.cursor > 0 {
                    self.cursor -= 1;
                    self.slots[self.cursor] = None;
                }
                InputOtpOutcome::Changed {
                    value: self.value(),
                    filled: self.filled(),
                }
            }
            KeyCode::Delete => {
                self.slots[self.cursor] = None;
                InputOtpOutcome::Changed {
                    value: self.value(),
                    filled: self.filled(),
                }
            }
            KeyCode::Enter => {
                if self.is_complete() {
                    InputOtpOutcome::Completed {
                        value: self.value(),
                    }
                } else {
                    InputOtpOutcome::SubmittedIncomplete {
                        value: self.value(),
                    }
                }
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                // Paste-like multi-char is host-driven via set_value; single char here
                if !self.charset.accepts(c) {
                    return InputOtpOutcome::Ignored;
                }
                let n = self.charset.normalize(c);
                self.slots[self.cursor] = Some(n);
                if self.cursor + 1 < self.length {
                    self.cursor += 1;
                }
                if self.is_complete() {
                    InputOtpOutcome::Completed {
                        value: self.value(),
                    }
                } else {
                    InputOtpOutcome::Changed {
                        value: self.value(),
                        filled: self.filled(),
                    }
                }
            }
            // Ctrl+V paste request is host-owned — treat long paste via set_value from host
            _ => InputOtpOutcome::Ignored,
        }
    }

    /// Host paste of full code.
    pub fn paste(&mut self, raw: &str) -> InputOtpOutcome {
        self.set_value(raw)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// OTP slot paint.
#[derive(Debug, Clone, Copy)]
pub struct InputOtp<'a> {
    system: &'a DesignSystem,
    ascii: bool,
    label: Option<&'a str>,
}

impl<'a> InputOtp<'a> {
    /// Design system.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            ascii: false,
            label: None,
        }
    }

    /// ASCII box glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Optional label row.
    #[must_use]
    pub const fn label(mut self, l: &'a str) -> Self {
        self.label = Some(l);
        self
    }

    /// Preferred width for `n` slots.
    #[must_use]
    pub fn preferred_width(n: usize) -> u16 {
        // each slot: [X] + gap = 4, last no gap
        (n as u16).saturating_mul(4).saturating_sub(1).max(3)
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &InputOtpState) {
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        if let Some(label) = self.label {
            if area.height >= 2 {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(label, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }
        let mut x = area.x;
        let end_x = area.x.saturating_add(area.width);
        for (i, slot) in state.slots.iter().enumerate() {
            if x.saturating_add(3) > end_x {
                break;
            }
            let ch = match slot {
                Some(c) if state.masked => '•',
                Some(c) => *c,
                None => {
                    if self.ascii {
                        '_'
                    } else {
                        '·'
                    }
                }
            };
            let focused_slot = state.focused && i == state.cursor;
            let style = if focused_slot {
                self.system
                    .style(Role::Accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else if slot.is_some() {
                self.system.style(Role::TextStrong)
            } else {
                self.system.style(Role::TextMuted)
            };
            let cell = if self.ascii {
                format!("[{ch}]")
            } else {
                format!("[{ch}]")
            };
            buffer.set_stringn(x, y, &cell, 3, style);
            x = x.saturating_add(4);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn type_digits_auto_advance_and_complete() {
        let mut st = InputOtpState::new(4);
        assert!(matches!(
            st.handle_key(press('1')),
            InputOtpOutcome::Changed { filled: 1, .. }
        ));
        assert_eq!(st.cursor(), 1);
        let _ = st.handle_key(press('2'));
        let _ = st.handle_key(press('3'));
        let out = st.handle_key(press('4'));
        assert!(
            matches!(out, InputOtpOutcome::Completed { ref value } if value == "1234"),
            "{out:?}"
        );
        assert!(st.is_complete());
    }

    #[test]
    fn reject_non_digit_in_digits_mode() {
        let mut st = InputOtpState::new(6);
        assert!(matches!(
            st.handle_key(press('a')),
            InputOtpOutcome::Ignored
        ));
        assert_eq!(st.filled(), 0);
    }

    #[test]
    fn paste_and_clear() {
        let mut st = InputOtpState::new(6);
        let out = st.paste("42ab99");
        // digits only — filters non-digits
        assert!(
            matches!(out, InputOtpOutcome::Completed { ref value } if value == "4299")
                || matches!(out, InputOtpOutcome::Changed { .. }),
            "{out:?}"
        );
        // force full paste of 6 digits
        let out = st.paste("654321");
        assert!(
            matches!(out, InputOtpOutcome::Completed { ref value } if value == "654321"),
            "{out:?}"
        );
        assert!(matches!(st.clear(), InputOtpOutcome::Cleared));
        assert_eq!(st.filled(), 0);
    }

    #[test]
    fn backspace_edits_previous() {
        let mut st = InputOtpState::new(3);
        let _ = st.handle_key(press('9'));
        let _ = st.handle_key(press('8'));
        // after '9','8': slots [9,8,None], cursor advanced to empty slot 2
        assert_eq!(st.cursor(), 2);
        assert_eq!(st.value(), "98");
        let out = st.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert!(
            matches!(out, InputOtpOutcome::Changed { filled: 1, .. }),
            "{out:?}"
        );
        // empty slot 2 → move to 1 and clear
        assert!(st.slots[1].is_none());
        assert_eq!(st.cursor(), 1);
        assert_eq!(st.value(), "9");
        assert_eq!(st.filled(), 1);
        assert!(!st.is_complete());
    }

    #[test]
    fn value_stops_at_first_hole() {
        let mut st = InputOtpState::new(4);
        let _ = st.set_value("12");
        assert_eq!(st.value(), "12");
        assert_eq!(st.filled(), 2);
        // Navigate to slot 0, clear it — leaves hole before remaining digits
        st.cursor = 0;
        let _ = st.handle_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        // slots [None, Some('2'), None, None] — must not collapse to "2"
        assert!(st.slots[0].is_none());
        assert_eq!(st.slots[1], Some('2'));
        assert_eq!(st.value(), "");
        assert_eq!(st.filled(), 0);
    }

    #[test]
    fn paint_smoke() {
        let system = DesignSystem::default();
        let mut st = InputOtpState::new(4);
        let _ = st.set_value("12");
        let area = Rect::new(0, 0, 20, 2);
        let mut buf = Buffer::empty(area);
        InputOtp::new(&system)
            .label("Code")
            .paint(area, &mut buf, &st);
    }
}
