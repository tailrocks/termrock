//! Source-owned SettingsScreen block (elevated 0237; package id settings-shell).

use termrock::input::KeyEvent;
use termrock::patterns::{
    SettingsScreenOutcome, SettingsScreenState,
};
use termrock::widgets::{Fieldset, NavItem, ThemePreset, BUILTIN_THEME_PRESETS};

/// Keyboard routing for the elevated settings screen.
pub fn handle_key<SectionId: Clone + PartialEq>(
    state: &mut SettingsScreenState<SectionId>,
    key: KeyEvent,
    nav: &[NavItem<SectionId>],
    fieldsets: &[Fieldset<'_, &'static str>],
    theme_presets: &[ThemePreset],
) -> SettingsScreenOutcome<SectionId, &'static str> {
    state.handle_key(key, nav, fieldsets, theme_presets)
}

/// Select a settings section (controlled / deep link).
pub fn select_section<SectionId: Clone + PartialEq>(
    state: &mut SettingsScreenState<SectionId>,
    id: SectionId,
) -> SettingsScreenOutcome<SectionId, &'static str> {
    state.select_section(id)
}

/// Builtin theme presets for theme body mode.
#[must_use]
pub fn theme_presets() -> &'static [ThemePreset] {
    BUILTIN_THEME_PRESETS
}
