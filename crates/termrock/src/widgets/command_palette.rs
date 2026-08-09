// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Command palette composition over query + results list.
//!
//! Domain filtering stays caller-owned; TermRock owns chrome, query routing,
//! and activation outcomes via [`Picker`]. Open/dismiss/placement go through
//! [`crate::interaction::OverlayStack`] with [`OverlayKind::CommandPalette`].

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::KeyEvent,
    interaction::{
        OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy, OverlaySize, OverlaySpec,
        OverlayStack, place_overlay,
    },
    widgets::{ListRow, Panel, PanelEmphasis, Picker, PickerOutcome, PickerState, TextInputState},
};

/// Default overlay id for a command palette on an [`OverlayStack`].
pub const COMMAND_PALETTE_OVERLAY_ID: &str = "termrock.command_palette";

/// Preferred palette size (width × height in cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteSize {
    /// Preferred width.
    pub width: u16,
    /// Preferred height.
    pub height: u16,
}

impl Default for CommandPaletteSize {
    fn default() -> Self {
        Self {
            width: 56,
            height: 16,
        }
    }
}

impl From<CommandPaletteSize> for OverlaySize {
    fn from(value: CommandPaletteSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 24,
            min_height: 6,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Centered command-palette rectangle inside `bounds` (policy-aware).
#[must_use]
pub fn place_command_palette(bounds: Rect, preferred: CommandPaletteSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        None,
        OverlaySize::from(preferred),
        OverlayPolicy::for_kind(OverlayKind::CommandPalette),
    )
}

/// Opens (or replaces) the command palette overlay and returns its outcome.
pub fn open_command_palette_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: CommandPaletteSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::command_palette(
            COMMAND_PALETTE_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Dismisses the default command-palette overlay when present.
pub fn dismiss_command_palette_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(COMMAND_PALETTE_OVERLAY_ID))
}

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
    tokens: &'a crate::style::DesignTokens,
}

impl<'a, Id> CommandPalette<'a, Id> {
    /// Creates a palette with a title and visible result rows.
    #[must_use]
    pub const fn new(
        title: &'a str,
        rows: &'a [ListRow<'a, Id>],
        tokens: &'a crate::style::DesignTokens,
    ) -> Self {
        Self {
            title,
            rows,
            tokens,
        }
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
        let panel = Panel::new(self.tokens)
            .title(self.title)
            .emphasis(PanelEmphasis::Focused);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let picker = Picker::new(self.rows, self.tokens).label("Command");
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
    use crate::input::{KeyCode, KeyModifiers};

    #[test]
    fn activation_maps_from_picker() {
        let rows = [ListRow::item("quit", Line::from("Quit"))];
        let mut state = CommandPaletteState::new(Some("quit"));
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            CommandPalette::handle_key(&mut state, key, &rows),
            CommandPaletteOutcome::Activated("quit")
        );
    }

    #[test]
    fn opens_on_overlay_stack_and_restores_opener() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let out = open_command_palette_overlay(
            &mut stack,
            bounds,
            CommandPaletteSize::default(),
            Some("editor"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::CommandPalette);
        let rect = stack.top().unwrap().rect;
        assert_eq!(
            rect,
            place_command_palette(bounds, CommandPaletteSize::default())
        );
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("editor"),
                ..
            }
        ));
    }

    #[test]
    fn narrow_terminal_promotes_palette() {
        let bounds = Rect::new(0, 0, 40, 12);
        let mut stack = OverlayStack::<()>::new();
        let _ =
            open_command_palette_overlay(&mut stack, bounds, CommandPaletteSize::default(), None);
        assert!(stack.top().unwrap().fullscreen_promoted);
        assert_eq!(stack.top().unwrap().rect, bounds);
    }
}
