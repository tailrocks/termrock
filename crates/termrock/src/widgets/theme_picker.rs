// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Live theme picker: select a named preset; caller re-renders with that theme.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyEvent, KeyEventKind},
    interaction::{EventResult, NavigationMove, OverlayRequest, UiIntent, default_list_intent},
    style::{Density, DesignSystem, Role, RolePalette},
    text::take_display_cols,
    widgets::{Panel, PanelChrome},
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
        label: "Phosphor Obsidian",
        requires_truecolor: false,
    },
    ThemePreset {
        id: "slate",
        label: "Slate",
        requires_truecolor: true,
    },
    ThemePreset {
        id: "paper",
        label: "Paper",
        requires_truecolor: true,
    },
    ThemePreset {
        id: "ansi",
        label: "ANSI 16",
        requires_truecolor: false,
    },
    ThemePreset {
        id: "high-contrast",
        label: "High Contrast",
        requires_truecolor: false,
    },
    ThemePreset {
        id: "adaptive",
        label: "Terminal Adaptive",
        requires_truecolor: false,
    },
];

/// Resolves a built-in theme by preset id.
#[must_use]
pub fn theme_from_preset_id(id: &str) -> Option<RolePalette> {
    match id {
        "phosphor" | "tailrocks_phosphor" | "obsidian" | "dark" => {
            Some(RolePalette::tailrocks_phosphor())
        }
        "slate" => Some(RolePalette::slate()),
        "paper" | "light" => Some(RolePalette::paper()),
        "ansi" | "ansi16" => Some(RolePalette::ansi()),
        "high-contrast" | "hc" | "high_contrast" => Some(RolePalette::high_contrast()),
        "adaptive" => Some(DesignSystem::adaptive().palette),
        _ => None,
    }
}

/// Resolves a full [`DesignSystem`] for a preset id.
#[must_use]
pub fn system_from_preset_id(id: &str) -> Option<DesignSystem> {
    match id {
        "phosphor" | "tailrocks_phosphor" | "obsidian" | "dark" => Some(DesignSystem::phosphor()),
        "slate" => Some(DesignSystem::slate()),
        "paper" | "light" => Some(DesignSystem::paper()),
        "ansi" | "ansi16" => Some(DesignSystem::ansi()),
        "high-contrast" | "hc" | "high_contrast" => Some(DesignSystem::high_contrast()),
        "adaptive" => Some(DesignSystem::adaptive()),
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

    /// Handles navigation / confirm / cancel via default list intents (no raw key match).
    pub fn handle_key(&mut self, key: KeyEvent, preset_count: usize) -> ThemePickerOutcome {
        if key.kind != KeyEventKind::Press || preset_count == 0 {
            return ThemePickerOutcome::Ignored;
        }
        match default_list_intent(key) {
            Some(intent) => self.handle_intent(intent, preset_count),
            None => ThemePickerOutcome::Ignored,
        }
    }

    /// Semantic intent path (preferred when the host owns keymaps).
    pub fn handle_intent(&mut self, intent: UiIntent, preset_count: usize) -> ThemePickerOutcome {
        if preset_count == 0 {
            return ThemePickerOutcome::Ignored;
        }
        match intent {
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                self.selected = self.selected.saturating_sub(1);
                ThemePickerOutcome::SelectionChanged
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                self.selected = (self.selected + 1).min(preset_count - 1);
                ThemePickerOutcome::SelectionChanged
            }
            UiIntent::Move(NavigationMove::First) => {
                self.selected = 0;
                ThemePickerOutcome::SelectionChanged
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.selected = preset_count - 1;
                ThemePickerOutcome::SelectionChanged
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Open => {
                ThemePickerOutcome::ConfirmIndex(self.selected)
            }
            UiIntent::Cancel | UiIntent::Close => ThemePickerOutcome::Cancelled,
            _ => ThemePickerOutcome::Ignored,
        }
    }

    /// Key path with standard [`EventResult`] envelope (domain = [`ThemePickerOutcome`]).
    pub fn handle_key_result(
        &mut self,
        key: KeyEvent,
        preset_count: usize,
    ) -> EventResult<ThemePickerOutcome> {
        Self::outcome_to_result(self.handle_key(key, preset_count))
    }

    /// Intent path with [`EventResult`]. Cancel attaches dismiss-top overlay request.
    pub fn handle_intent_result(
        &mut self,
        intent: UiIntent,
        preset_count: usize,
    ) -> EventResult<ThemePickerOutcome> {
        Self::outcome_to_result(self.handle_intent(intent, preset_count))
    }

    fn outcome_to_result(outcome: ThemePickerOutcome) -> EventResult<ThemePickerOutcome> {
        match outcome {
            ThemePickerOutcome::Ignored => EventResult::ignored(),
            ThemePickerOutcome::Cancelled => {
                EventResult::emit(outcome).with_overlay(OverlayRequest::DismissTop)
            }
            other => EventResult::emit(other),
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
    paint_theme: &'a DesignSystem,
}

impl<'a> ThemePicker<'a> {
    /// Creates a picker.
    #[must_use]
    pub const fn new(presets: &'a [ThemePreset], paint_theme: &'a DesignSystem) -> Self {
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
        let tokens = (*self.paint_theme).clone();
        let panel = Panel::new(&tokens)
            .title("Theme")
            .emphasis(PanelChrome::Focused);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        for (index, preset) in self
            .presets
            .iter()
            .enumerate()
            .take(usize::from(inner.height))
        {
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
                take_display_cols(&line, usize::from(inner.width)),
                usize::from(inner.width),
                self.paint_theme.style(role),
            );
        }
    }
}

impl StatefulWidget for ThemePicker<'_> {
    type State = ThemePickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::interaction::{OverlayRequest, Propagation};

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
        assert_eq!(theme_from_preset_id("slate"), Some(RolePalette::slate()));
    }

    #[test]
    fn event_result_cancel_requests_dismiss_top() {
        let mut state = ThemePickerState::new(0);
        let r = state.handle_key_result(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            BUILTIN_THEME_PRESETS.len(),
        );
        assert_eq!(r.propagation(), Propagation::Stop);
        assert_eq!(r.message(), Some(&ThemePickerOutcome::Cancelled));
        assert_eq!(r.overlay(), Some(&OverlayRequest::DismissTop));
    }
}
