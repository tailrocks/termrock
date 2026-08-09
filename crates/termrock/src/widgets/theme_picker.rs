// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Live theme picker: select a named preset; caller re-renders with that theme.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    style::{Role, Theme},
    text::take_display_cols,
    widgets::{Panel, PanelEmphasis},
};

/// One selectable theme preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePreset {
    /// Stable key (`phosphor`, `slate`, …).
    pub id: &'static str,
    /// Human label.
    pub label: &'static str,
    /// Whether the preset needs truecolor to retain character.
    pub requires_truecolor: bool,
}

/// Built-in TermRock presets.
pub const BUILTIN_THEME_PRESETS: &[ThemePreset] = &[
    ThemePreset {
        id: "phosphor",
        label: "Phosphor",
        requires_truecolor: false,
    },
    ThemePreset {
        id: "slate",
        label: "Slate",
        requires_truecolor: true,
    },
];

/// Resolves a built-in theme by preset id.
#[must_use]
pub fn theme_from_preset_id(id: &str) -> Option<Theme> {
    match id {
        "phosphor" | "tailrocks_phosphor" | "dark" => Some(Theme::tailrocks_phosphor()),
        "slate" | "light" => Some(Theme::slate()),
        _ => None,
    }
}

/// Picker state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePickerState {
    selected: usize,
    confirmed: Option<&'static str>,
}

impl Default for ThemePickerState {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ThemePickerState {
    /// Creates state with an initial selection index.
    #[must_use]
    pub const fn new(selected: usize) -> Self {
        Self {
            selected,
            confirmed: None,
        }
    }

    /// Currently highlighted index.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Last confirmed preset id, if any.
    #[must_use]
    pub const fn confirmed(&self) -> Option<&'static str> {
        self.confirmed
    }

    /// Clears the confirmation latch.
    pub fn clear_confirmed(&mut self) {
        self.confirmed = None;
    }

    /// Handles navigation / confirm / cancel.
    pub fn handle_key(&mut self, key: KeyEvent, preset_count: usize) -> ThemePickerOutcome {
        if key.kind != KeyEventKind::Press || preset_count == 0 {
            return ThemePickerOutcome::Ignored;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                ThemePickerOutcome::SelectionChanged
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = (self.selected + 1).min(preset_count - 1);
                ThemePickerOutcome::SelectionChanged
            }
            KeyCode::Home => {
                self.selected = 0;
                ThemePickerOutcome::SelectionChanged
            }
            KeyCode::End => {
                self.selected = preset_count - 1;
                ThemePickerOutcome::SelectionChanged
            }
            KeyCode::Enter => {
                // Caller supplies presets at render; confirmation uses index only here.
                ThemePickerOutcome::ConfirmIndex(self.selected)
            }
            KeyCode::Esc => ThemePickerOutcome::Cancelled,
            _ => ThemePickerOutcome::Ignored,
        }
    }

    /// Confirms a preset id after the caller resolves index → id.
    pub fn confirm_id(&mut self, id: &'static str) {
        self.confirmed = Some(id);
    }
}

/// Outcomes from the theme picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ThemePickerOutcome {
    /// No action.
    Ignored,
    /// Highlight moved (caller should live-preview).
    SelectionChanged,
    /// Enter pressed on index.
    ConfirmIndex(usize),
    /// Esc cancelled.
    Cancelled,
}

/// Theme list chrome. Paint uses `paint_theme` for live-preview coloring.
#[derive(Debug, Clone, Copy)]
pub struct ThemePicker<'a> {
    presets: &'a [ThemePreset],
    paint_theme: &'a Theme,
}

impl<'a> ThemePicker<'a> {
    /// Creates a picker.
    #[must_use]
    pub const fn new(presets: &'a [ThemePreset], paint_theme: &'a Theme) -> Self {
        Self {
            presets,
            paint_theme,
        }
    }
}

impl StatefulWidget for &ThemePicker<'_> {
    type State = ThemePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let panel = Panel::new(self.paint_theme)
            .title("Theme")
            .emphasis(PanelEmphasis::Focused);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        for (index, preset) in self.presets.iter().enumerate().take(usize::from(inner.height)) {
            let y = inner.y.saturating_add(index as u16);
            let selected = index == state.selected;
            let marker = if selected { "›" } else { " " };
            let tc = if preset.requires_truecolor {
                " · truecolor"
            } else {
                ""
            };
            let line = format!("{marker} {}{tc}", preset.label);
            let role = if selected {
                Role::Selection
            } else {
                Role::Text
            };
            buffer.set_stringn(
                inner.x,
                y,
                &take_display_cols(&line, usize::from(inner.width)),
                usize::from(inner.width),
                self.paint_theme.style(role),
            );
        }
    }
}

impl StatefulWidget for ThemePicker<'_> {
    type State = ThemePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn navigation_and_confirm_index() {
        let mut state = ThemePickerState::new(0);
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                BUILTIN_THEME_PRESETS.len()
            ),
            ThemePickerOutcome::SelectionChanged
        );
        assert_eq!(state.selected(), 1);
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                BUILTIN_THEME_PRESETS.len()
            ),
            ThemePickerOutcome::ConfirmIndex(1)
        );
        assert_eq!(theme_from_preset_id("slate"), Some(Theme::slate()));
    }
}
