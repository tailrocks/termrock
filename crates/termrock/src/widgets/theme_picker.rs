// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Live theme picker: select a named preset; caller re-renders with that theme.
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::{
    input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    interaction::{EventResult, NavigationMove, OverlayRequest, UiIntent, default_list_intent},
    style::{DesignSystem, ListRowVisualState},
    text::take_display_cols,
    widgets::{Panel, PanelChrome, PanelVariant},
};

/// One selectable theme preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePreset {
    /// Stable key (`junie`).
    pub id: &'static str,
    /// Human label.
    pub label: &'static str,
    /// Whether the preset needs truecolor to retain character.
    pub requires_truecolor: bool,
}

/// The one built-in TermRock theme. There are no alternates to swap to.
pub const BUILTIN_THEME_PRESETS: &[ThemePreset] = &[ThemePreset {
    id: "junie",
    label: "Junie",
    requires_truecolor: true,
}];

/// Resolves a full [`DesignSystem`] for a preset id. There are no aliases:
/// only the canonical id `junie` resolves; anything else returns `None`.
#[must_use]
pub fn system_from_preset_id(id: &str) -> Option<DesignSystem> {
    match id {
        "junie" => Some(DesignSystem::junie()),
        _ => None,
    }
}

/// Picker state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemePickerState {
    selected: usize,
    confirmed: Option<&'static str>,
    focused: bool,
    enabled: bool,
    row_regions: Vec<(usize, Rect)>,
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
            focused: true,
            enabled: true,
            row_regions: Vec::new(),
        }
    }

    /// Currently highlighted index.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Focus-visible interaction ownership.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on && self.enabled;
    }

    /// Enables navigation, confirmation, and pointer activation.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.focused = false;
        }
    }

    /// Whether the picker can accept interaction.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Handles navigation / confirm / cancel via default list intents (no raw key match).
    pub fn handle_key(&mut self, key: KeyEvent, preset_count: usize) -> ThemePickerOutcome {
        if !self.enabled || !self.focused || !key.is_press() || preset_count == 0 {
            return ThemePickerOutcome::Ignored;
        }
        match default_list_intent(key) {
            Some(intent) => self.handle_intent(intent, preset_count),
            None => ThemePickerOutcome::Ignored,
        }
    }

    /// Semantic intent path (preferred when the host owns keymaps).
    pub fn handle_intent(&mut self, intent: UiIntent, preset_count: usize) -> ThemePickerOutcome {
        if !self.enabled || !self.focused || preset_count == 0 {
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
    /// Pointer activation over exact row geometry published by the last render.
    pub fn handle_mouse(&mut self, event: MouseEvent, preset_count: usize) -> ThemePickerOutcome {
        if !self.enabled
            || !matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
            || preset_count == 0
        {
            return ThemePickerOutcome::Ignored;
        }
        let Some(index) = self
            .row_regions
            .iter()
            .find(|(_, rect)| rect.contains(event.position))
            .map(|(index, _)| *index)
            .filter(|index| *index < preset_count)
        else {
            return ThemePickerOutcome::Ignored;
        };
        self.focused = true;
        self.selected = index;
        ThemePickerOutcome::ConfirmIndex(index)
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
        state.row_regions.clear();
        if area.is_empty() {
            return;
        }
        let tokens = (*self.paint_theme).clone();
        let panel = Panel::new(&tokens)
            .variant(PanelVariant::Bordered)
            .title("Theme")
            .emphasis(PanelChrome::for_focus(state.focused && state.enabled));
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
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
            let recipe = tokens.resolve_list_row(ListRowVisualState {
                selected,
                focused: state.focused && selected,
                hovered: false,
                enabled: state.enabled,
                checked: false,
                ..ListRowVisualState::default()
            });
            let marker = recipe.gutter.0;
            let tc = if preset.requires_truecolor {
                " · truecolor"
            } else {
                ""
            };
            let line = format!("{marker} {}{tc}", preset.label);
            let row = Rect::new(inner.x, y, inner.width, 1);
            state.row_regions.push((index, row));
            if recipe.use_tint {
                buffer.set_style(row, recipe.tint);
            }
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&line, usize::from(inner.width)),
                usize::from(inner.width),
                recipe.label,
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
    use crate::widgets::tests::click;

    #[test]
    fn navigation_and_confirm_index() {
        // junie ships one theme; the picker still navigates whatever list the
        // host hands it, so drive it with a two-entry projection.
        let presets = [BUILTIN_THEME_PRESETS[0], BUILTIN_THEME_PRESETS[0]];
        let mut state = ThemePickerState::new(0);
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                presets.len()
            ),
            ThemePickerOutcome::SelectionChanged
        );
        assert_eq!(state.selected(), 1);
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                presets.len()
            ),
            ThemePickerOutcome::ConfirmIndex(1)
        );
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

    #[test]
    fn mouse_confirms_painted_row_and_disabled_blocks_all_activation() {
        let system = DesignSystem::default();
        let presets = [BUILTIN_THEME_PRESETS[0], BUILTIN_THEME_PRESETS[0]];
        let picker = ThemePicker::new(&presets, &system);
        let area = Rect::new(0, 0, 32, 8);
        let mut buffer = Buffer::empty(area);
        let mut state = ThemePickerState::new(0);
        StatefulWidget::render(&picker, area, &mut buffer, &mut state);
        let (_, row) = state.row_regions[1];
        let click = click(row.x, row.y);

        assert_eq!(
            state.handle_mouse(click, presets.len()),
            ThemePickerOutcome::ConfirmIndex(1)
        );
        assert_eq!(state.selected(), 1);

        state.set_enabled(false);
        assert!(!state.is_enabled());
        StatefulWidget::render(&picker, area, &mut buffer, &mut state);
        assert_eq!(
            state.handle_mouse(click, presets.len()),
            ThemePickerOutcome::Ignored
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                presets.len(),
            ),
            ThemePickerOutcome::Ignored
        );
    }
}
