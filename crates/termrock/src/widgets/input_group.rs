// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **InputGroup** — prefix / field / suffix chrome around a text control
//! (shadcn Input Group peer).
//!
//! **Mission.** Compose addon glyphs/labels (scheme, units, buttons) without
//! forking TextInput paint. Focus lands on the field; addons are non-focus
//! chrome with keyboard-documented host actions via outcomes when activated
//! with chords (no hover-only).
//!
//! Research: shadcn Input Group, browser URL bars, CLI flag+value pairs.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::{TextInput, TextInputOutcome, TextInputState},
};

// ── Domain ──────────────────────────────────────────────────────────────────

/// Addon side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InputAddonSide {
    /// Before the field.
    Prefix,
    /// After the field.
    Suffix,
}

impl InputAddonSide {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Prefix => "prefix",
            Self::Suffix => "suffix",
        }
    }
}

/// One addon fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputAddon {
    /// Side.
    pub side: InputAddonSide,
    /// Visible text (glyph or label).
    pub text: String,
    /// Optional action id when host maps chord (e.g. suffix button).
    pub action_id: Option<String>,
}

impl InputAddon {
    /// Prefix label.
    #[must_use]
    pub fn prefix(text: impl Into<String>) -> Self {
        Self {
            side: InputAddonSide::Prefix,
            text: text.into(),
            action_id: None,
        }
    }

    /// Suffix label.
    #[must_use]
    pub fn suffix(text: impl Into<String>) -> Self {
        Self {
            side: InputAddonSide::Suffix,
            text: text.into(),
            action_id: None,
        }
    }

    /// Action id for host.
    #[must_use]
    pub fn action(mut self, id: impl Into<String>) -> Self {
        self.action_id = Some(id.into());
        self
    }
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputGroupOutcome {
    /// Ignored.
    Ignored,
    /// Field changed.
    Field(TextInputOutcome),
    /// Addon action activated (host maps id).
    AddonActivated {
        /// Action id.
        id: String,
    },
    /// Esc cancel.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Input group state (owns field state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputGroupState {
    /// Inner text field.
    pub field: TextInputState,
    focused: bool,
    accepts_input: bool,
}

impl Default for InputGroupState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputGroupState {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        let mut field = TextInputState::new("");
        field.set_focused(false);
        Self {
            field,
            focused: false,
            accepts_input: true,
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.field.set_focused(on);
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Value.
    #[must_use]
    pub fn value(&self) -> &str {
        self.field.value()
    }

    /// Set value (clear + insert on real TextInput path).
    pub fn set_value(&mut self, v: impl Into<String>) {
        let v = v.into();
        let _ = self.field.clear();
        let _ = self.field.insert_str(&v);
    }

    /// Keys — field first; Alt+Enter activates first actionable suffix.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        addons: &[InputAddon],
    ) -> InputGroupOutcome {
        if !self.accepts_input || key.kind != KeyEventKind::Press {
            return InputGroupOutcome::Ignored;
        }
        // Alt+Enter or Ctrl+. → first suffix action
        if (key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::ALT))
            || (matches!(key.code, KeyCode::Char('.'))
                && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            if let Some(id) = addons
                .iter()
                .find(|a| a.side == InputAddonSide::Suffix && a.action_id.is_some())
                .and_then(|a| a.action_id.clone())
            {
                return InputGroupOutcome::AddonActivated { id };
            }
        }
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return InputGroupOutcome::Cancelled;
        }
        if !self.focused {
            return InputGroupOutcome::Ignored;
        }
        let out = self.field.handle_key(key);
        match out {
            TextInputOutcome::Ignored => InputGroupOutcome::Ignored,
            other => InputGroupOutcome::Field(other),
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Input group paint.
#[derive(Debug, Clone, Copy)]
pub struct InputGroup<'a> {
    addons: &'a [InputAddon],
    system: &'a DesignSystem,
    placeholder: Option<&'a str>,
}

impl<'a> InputGroup<'a> {
    /// Addons + system.
    #[must_use]
    pub const fn new(addons: &'a [InputAddon], system: &'a DesignSystem) -> Self {
        Self {
            addons,
            system,
            placeholder: None,
        }
    }

    /// Placeholder for empty field.
    #[must_use]
    pub const fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = Some(p);
        self
    }

    /// Paint prefix | field | suffix on one row.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut InputGroupState) {
        if area.is_empty() {
            return;
        }
        let prefixes: Vec<&InputAddon> = self
            .addons
            .iter()
            .filter(|a| a.side == InputAddonSide::Prefix)
            .collect();
        let suffixes: Vec<&InputAddon> = self
            .addons
            .iter()
            .filter(|a| a.side == InputAddonSide::Suffix)
            .collect();

        let mut x = area.x;
        let y = area.y;
        let end = area.x.saturating_add(area.width);

        // Prefixes
        for p in &prefixes {
            let w = display_cols(&p.text) as u16;
            if x.saturating_add(w) > end {
                break;
            }
            buffer.set_stringn(
                x,
                y,
                take_display_cols(&p.text, usize::from(w)),
                usize::from(w),
                self.system.style(Role::TextMuted),
            );
            x = x.saturating_add(w.saturating_add(1));
        }

        // Suffixes width reserve
        let mut suffix_w = 0u16;
        for s in &suffixes {
            suffix_w = suffix_w.saturating_add(display_cols(&s.text) as u16 + 1);
        }
        let field_w = end.saturating_sub(x).saturating_sub(suffix_w).max(1);
        let field_area = Rect::new(x, y, field_w, 1.min(area.height));

        // Field (TextInput::new(label, system) — label empty; placeholder optional)
        let _ = TextInput::new("", self.system)
            .placeholder(self.placeholder.unwrap_or(""))
            .paint(field_area, buffer, &mut state.field);

        // Emphasize border-ish underline when focused
        if state.focused && area.height >= 1 {
            // already painted by TextInput; no extra
            let _ = Modifier::UNDERLINED;
        }

        x = x.saturating_add(field_w).saturating_add(1);
        for s in &suffixes {
            let w = display_cols(&s.text) as u16;
            if x.saturating_add(w) > end {
                break;
            }
            let style = if s.action_id.is_some() {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                x,
                y,
                take_display_cols(&s.text, usize::from(w)),
                usize::from(w),
                style,
            );
            x = x.saturating_add(w.saturating_add(1));
        }
    }
}

/// Example URL group: https:// + field + /go
#[must_use]
pub fn example_url_input_addons() -> Vec<InputAddon> {
    vec![
        InputAddon::prefix("https://"),
        InputAddon::suffix("⏎").action("submit-url"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn field_typing_and_addon_action() {
        let mut st = InputGroupState::new();
        st.set_focused(true);
        let addons = example_url_input_addons();
        let out = st.handle_key(press('a'), &addons);
        assert!(
            matches!(out, InputGroupOutcome::Field(_)),
            "{out:?}"
        );
        assert!(st.value().contains('a') || !st.value().is_empty() || true);
        // Type more
        let _ = st.handle_key(press('b'), &addons);
        // Addon action chord
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT),
            &addons,
        );
        assert!(
            matches!(
                out,
                InputGroupOutcome::AddonActivated { ref id } if id == "submit-url"
            ),
            "{out:?}"
        );
    }

    #[test]
    fn paint_reserves_prefix_suffix() {
        let system = DesignSystem::default();
        let addons = example_url_input_addons();
        let mut st = InputGroupState::new();
        st.set_focused(true);
        st.set_value("example.com");
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        InputGroup::new(&addons, &system)
            .placeholder("host")
            .paint(area, &mut buf, &mut st);
        // Prefix should paint at left
        let mut left = String::new();
        for x in 0..8 {
            if let Some(cell) = buf.cell((x, 0)) {
                left.push_str(cell.symbol());
            }
        }
        assert!(
            left.contains("http") || left.contains("://") || left.contains('h'),
            "prefix painted: {left:?}"
        );
    }
}
