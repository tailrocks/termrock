//! Source-owned SettingsShell block (Plan 053).

use termrock::input::KeyEvent;
use termrock::widgets::{SettingsShellOutcome, SettingsShellState};

/// Keyboard routing (Ctrl+S save, search field when focused).
pub fn handle_key<SectionId: Clone + PartialEq>(
    state: &mut SettingsShellState<SectionId>,
    key: KeyEvent,
) -> SettingsShellOutcome<SectionId> {
    state.handle_key(key)
}

/// Select a settings section (controlled).
pub fn select_section<SectionId: Clone + PartialEq>(
    state: &mut SettingsShellState<SectionId>,
    id: SectionId,
) -> SettingsShellOutcome<SectionId> {
    state.select_section(id)
}
