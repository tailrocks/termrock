// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Command palette composition over query + results list.
//!
//! Domain filtering stays caller-owned; TermRock owns chrome, query routing,
//! and activation outcomes via [`Picker`].

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::KeyEvent,
    style::Theme,
    widgets::{ListRow, Panel, PanelEmphasis, Picker, PickerOutcome, PickerState, TextInputState},
};

/// Semantic palette outcomes (mirrors picker with palette naming).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandPaletteOutcome<Id> {
    /// No action.
    Ignored,
    /// Query changed; rebuild filtered projection.
    QueryChanged,
    /// Highlight moved.
    SelectionChanged,
    /// Command activated.
    Activated(Id),
    /// Palette dismissed.
    Cancelled,
}

impl<Id> From<PickerOutcome<Id>> for CommandPaletteOutcome<Id> {
    fn from(value: PickerOutcome<Id>) -> Self {
        match value {
            PickerOutcome::Ignored => Self::Ignored,
            PickerOutcome::QueryChanged => Self::QueryChanged,
            PickerOutcome::SelectionChanged => Self::SelectionChanged,
            PickerOutcome::Activated(id) => Self::Activated(id),
            PickerOutcome::Cancelled => Self::Cancelled,
        }
    }
}

/// State alias for command palette (picker-backed).
pub type CommandPaletteState<Id> = PickerState<Id>;

/// Floating command palette chrome around a [`Picker`].
#[derive(Debug, Clone, Copy)]
pub struct CommandPalette<'a, Id> {
    title: &'a str,
    rows: &'a [ListRow<'a, Id>],
    theme: &'a Theme,
}

impl<'a, Id> CommandPalette<'a, Id> {
    /// Creates a palette with a title and visible result rows.
    #[must_use]
    pub const fn new(title: &'a str, rows: &'a [ListRow<'a, Id>], theme: &'a Theme) -> Self {
        Self { title, rows, theme }
    }
}

impl<Id: Clone + PartialEq> CommandPalette<'_, Id> {
    /// Routes a key through the underlying picker.
    pub fn handle_key(
        state: &mut CommandPaletteState<Id>,
        key: KeyEvent,
        rows: &[ListRow<'_, Id>],
    ) -> CommandPaletteOutcome<Id> {
        CommandPaletteOutcome::from(state.handle_key(rows, key))
    }

    /// Accesses the query for caller filtering.
    #[must_use]
    pub fn query(state: &CommandPaletteState<Id>) -> &TextInputState {
        state.query()
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &CommandPalette<'_, Id> {
    type State = CommandPaletteState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let panel = Panel::new(self.theme)
            .title(self.title)
            .emphasis(PanelEmphasis::Focused);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let picker = Picker::new(self.rows, self.theme).label("Command");
        StatefulWidget::render(&picker, inner, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for CommandPalette<'_, Id> {
    type State = CommandPaletteState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::text::Line;

    use super::*;
    use crate::{
        input::{KeyCode, KeyModifiers},
        widgets::RowRole,
    };

    #[test]
    fn activation_maps_from_picker() {
        let rows = [ListRow {
            id: "quit",
            label: Line::from("Quit"),
            trailing: None,
            enabled: true,
            role: RowRole::Item,
        }];
        let mut state = CommandPaletteState::new(Some("quit"));
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            CommandPalette::handle_key(&mut state, key, &rows),
            CommandPaletteOutcome::Activated("quit")
        );
    }
}
