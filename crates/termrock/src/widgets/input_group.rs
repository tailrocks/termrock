// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **InputGroup** — multi-addon prefix / field / suffix chrome around a text
//! control (shadcn Input Group peer).
//!
//! **Mission.** Compose **multiple** addon glyphs/labels (scheme, units,
//! actionable suffix buttons) without forking TextInput field editing. Focus
//! lands on the field; addons are non-focus chrome. Host maps
//! [`InputGroupOutcome::AddonActivated`] (Alt+Enter / Ctrl+.) — no hover-only.
//!
//! **vs [`TextInput`] prefix/suffix.** Use `TextInput::prefix` / `suffix` for a
//! **single** decorative chrome string on each side (no action ids, no multi-
//! fragment layout). Use **InputGroup** when you need multiple addons, mixed
//! prefix+suffix fragments, or keyboard-activatable suffix actions. Do not
//! dual-paint: pick one surface per control.
//!
//! Research: shadcn Input Group, browser URL bars, CLI flag+value pairs.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{ButtonRecipeVariant, ControlState, DesignSystem},
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
    enabled: bool,
    parts: Option<InputGroupParts>,
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
            enabled: true,
            parts: None,
        }
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on && self.enabled;
        self.field.set_focused(self.focused);
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Enables both field editing and addon activation.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.field.set_enabled(on);
        if !on {
            self.set_focused(false);
        }
    }

    /// Whether the group can accept field or addon input.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
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
    pub fn handle_key(&mut self, key: KeyEvent, addons: &[InputAddon]) -> InputGroupOutcome {
        if !self.enabled || !self.accepts_input || key.kind != KeyEventKind::Press {
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

    /// Mouse path over geometry published by the last paint.
    pub fn handle_mouse(&mut self, event: MouseEvent, addons: &[InputAddon]) -> InputGroupOutcome {
        if !self.enabled
            || !self.accepts_input
            || !matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
        {
            return InputGroupOutcome::Ignored;
        }
        let Some(parts) = self.parts.clone() else {
            return InputGroupOutcome::Ignored;
        };
        if let Some((id, _)) = parts
            .addon_regions
            .iter()
            .find(|(_, rect)| rect.contains(event.position))
            && addons
                .iter()
                .any(|addon| addon.action_id.as_deref() == Some(id.as_str()))
        {
            self.set_focused(true);
            return InputGroupOutcome::AddonActivated { id: id.clone() };
        }
        if parts.field.contains(event.position) {
            self.set_focused(true);
            return match self.field.handle_mouse(event, parts.field) {
                TextInputOutcome::Ignored => InputGroupOutcome::Ignored,
                other => InputGroupOutcome::Field(other),
            };
        }
        InputGroupOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Geometry published by [`InputGroup::paint`] for internal pointer routing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InputGroupParts {
    field: Rect,
    addon_regions: Vec<(String, Rect)>,
}

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
        state.parts = None;
        if area.is_empty() {
            state.parts = Some(InputGroupParts {
                field: area,
                addon_regions: Vec::new(),
            });
            return;
        }
        let input_recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            false,
            state.focused,
        );
        buffer.set_style(area, input_recipe.fill);
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
        let mut addon_regions = Vec::new();

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
                input_recipe.placeholder,
            );
            if let Some(id) = &p.action_id {
                addon_regions.push((id.clone(), Rect::new(x, y, w, 1.min(area.height))));
            }
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

        x = x.saturating_add(field_w).saturating_add(1);
        for s in &suffixes {
            let w = display_cols(&s.text) as u16;
            if x.saturating_add(w) > end {
                break;
            }
            let style = if s.action_id.is_some() {
                let action = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    if state.enabled {
                        ControlState::Default
                    } else {
                        ControlState::Disabled
                    },
                    self.system.junie_theme().surface,
                );
                action.fill.patch(action.label)
            } else {
                input_recipe.placeholder
            };
            buffer.set_stringn(
                x,
                y,
                take_display_cols(&s.text, usize::from(w)),
                usize::from(w),
                style,
            );
            if let Some(id) = &s.action_id {
                addon_regions.push((id.clone(), Rect::new(x, y, w, 1.min(area.height))));
            }
            x = x.saturating_add(w.saturating_add(1));
        }
        let parts = InputGroupParts {
            field: field_area,
            addon_regions,
        };
        state.parts = Some(parts.clone());
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
            matches!(out, InputGroupOutcome::Field(TextInputOutcome::Changed)),
            "{out:?}"
        );
        assert_eq!(st.value(), "a");
        let out = st.handle_key(press('b'), &addons);
        assert!(
            matches!(out, InputGroupOutcome::Field(TextInputOutcome::Changed)),
            "{out:?}"
        );
        assert_eq!(st.value(), "ab");
        // Addon action chord
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT), &addons);
        assert!(
            matches!(
                out,
                InputGroupOutcome::AddonActivated { ref id } if id == "submit-url"
            ),
            "{out:?}"
        );
        // field value preserved after addon chord
        assert_eq!(st.value(), "ab");
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

    #[test]
    fn escape_cancels_without_mutating_the_field() {
        let addons = example_url_input_addons();
        let mut state = InputGroupState::new();
        state.set_focused(true);
        state.set_value("example.com");

        let outcome = state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &addons);

        assert_eq!(outcome, InputGroupOutcome::Cancelled);
        assert_eq!(state.value(), "example.com");
    }

    #[test]
    fn accepts_input_gate_blocks_field_and_addon_actions() {
        let addons = example_url_input_addons();
        let mut state = InputGroupState::new();
        state.set_focused(true);
        state.set_accepts_input(false);

        assert_eq!(
            state.handle_key(press('a'), &addons),
            InputGroupOutcome::Ignored
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT), &addons),
            InputGroupOutcome::Ignored
        );
        assert!(state.value().is_empty());
    }

    #[test]
    fn mouse_routes_painted_field_and_addon_while_disabled_blocks_both() {
        let system = DesignSystem::default();
        let addons = example_url_input_addons();
        let widget = InputGroup::new(&addons, &system);
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        let mut state = InputGroupState::new();
        widget.paint(area, &mut buffer, &mut state);
        let parts = state.parts.clone().expect("painted geometry");
        let (id, addon) = parts.addon_regions[0].clone();
        let click = |rect: Rect| MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: ratatui_core::layout::Position::new(rect.x, rect.y),
            modifiers: KeyModifiers::NONE,
        };

        assert!(matches!(
            state.handle_mouse(click(parts.field), &addons),
            InputGroupOutcome::Field(TextInputOutcome::Changed)
        ));
        assert_eq!(
            state.handle_mouse(click(addon), &addons),
            InputGroupOutcome::AddonActivated { id }
        );

        state.set_enabled(false);
        assert!(!state.is_enabled());
        assert_eq!(
            state.handle_mouse(click(addon), &addons),
            InputGroupOutcome::Ignored
        );
        assert_eq!(
            state.handle_key(press('x'), &addons),
            InputGroupOutcome::Ignored
        );
        widget.paint(area, &mut buffer, &mut state);
        assert_eq!(state.parts.as_ref(), Some(&parts));
    }
}
