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
        OverlayStack, UiIntent, place_overlay,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{ListRow, Panel, PanelChrome, Picker, PickerOutcome, PickerState, TextInputState},
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
    /// Result cursor moved (not scene focus).
    CursorMoved,
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
            PickerOutcome::CursorMoved => Self::CursorMoved,
            PickerOutcome::Activated(id) => Self::Activated(id),
            PickerOutcome::Cancelled => Self::Cancelled,
        }
    }
}

/// State alias for command palette (picker-backed).
pub type CommandPaletteState<Id> = PickerState<Id>;

/// Floating command palette chrome around a [`Picker`].
///
/// **Surface focus** via [`Self::focused`] + host [`PickerState::set_accepts_input`].
/// **Result cursor** is list-local inside picker state.
#[derive(Debug, Clone, Copy)]
pub struct CommandPalette<'a, Id> {
    title: &'a str,
    rows: &'a [ListRow<'a, Id>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
    footer_hint: Option<&'a str>,
    empty_message: &'a str,
}

impl<'a, Id> CommandPalette<'a, Id> {
    /// Creates a palette with a title and visible result rows.
    #[must_use]
    pub const fn new(
        title: &'a str,
        rows: &'a [ListRow<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            title,
            rows,
            system,
            focused: true,
            ascii: false,
            colorless: false,
            footer_hint: Some("↑↓ navigate · enter run · esc clear/close"),
            empty_message: "No commands",
        }
    }

    /// Scene/overlay surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII empty / panel recipes.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Footer keymap hint (dropped when height is tight).
    #[must_use]
    pub const fn footer_hint(mut self, hint: Option<&'a str>) -> Self {
        self.footer_hint = hint;
        self
    }

    /// Empty projection message.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
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

    /// Intent path (results list; query still via raw keys / TextInput).
    pub fn handle_intent(
        state: &mut CommandPaletteState<Id>,
        intent: UiIntent,
        rows: &[ListRow<'_, Id>],
    ) -> CommandPaletteOutcome<Id> {
        CommandPaletteOutcome::from(state.handle_intent(rows, intent))
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
        let surface = self.focused && state.accepts_input();
        let emphasis = if surface {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system).title(self.title).emphasis(emphasis);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let narrow = area.width < 28;
        let tiny = area.height < 6;
        let show_footer = self.footer_hint.is_some() && !tiny && area.height >= 8 && !narrow;
        let body = if show_footer {
            Rect::new(
                inner.x,
                inner.y,
                inner.width,
                inner.height.saturating_sub(1),
            )
        } else {
            inner
        };

        let picker = Picker::new(self.rows, self.system)
            .label("Command")
            .placeholder(if narrow {
                "Filter…"
            } else {
                "Type a command"
            })
            .empty_message(self.empty_message)
            .focused(surface)
            .ascii(self.ascii)
            .colorless(self.colorless);
        StatefulWidget::render(&picker, body, buffer, state);

        if show_footer {
            if let Some(hint) = self.footer_hint {
                let y = inner.bottom().saturating_sub(1);
                let style = self.system.style(Role::TextMuted);
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(hint, usize::from(inner.width)),
                    usize::from(inner.width),
                    style,
                );
            }
        }
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
    use crate::interaction::NavigationMove;
    use crate::style::DesignSystem;

    fn row(id: &'static str, label: &'static str) -> ListRow<'static, &'static str> {
        ListRow::item(id, Line::from(label))
    }

    #[test]
    fn activation_maps_from_picker() {
        let rows = [row("quit", "Quit")];
        let mut state = CommandPaletteState::new(Some("quit"));
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            CommandPalette::handle_key(&mut state, key, &rows),
            CommandPaletteOutcome::Activated("quit")
        );
    }

    #[test]
    fn cursor_moved_not_selection_changed() {
        let rows = [row("a", "A"), row("b", "B")];
        let mut state = CommandPaletteState::new(Some("a"));
        assert_eq!(
            CommandPalette::handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &rows
            ),
            CommandPaletteOutcome::CursorMoved
        );
        let src = include_str!("command_palette.rs");
        let head = src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(src)
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//") && !t.starts_with("//!")
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!head.contains("SelectionChanged"));
        assert!(head.contains("CursorMoved"));
    }

    #[test]
    fn accepts_input_gate() {
        let rows = [row("quit", "Quit")];
        let mut state = CommandPaletteState::new(Some("quit"));
        state.set_accepts_input(false);
        assert_eq!(
            CommandPalette::handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &rows
            ),
            CommandPaletteOutcome::Ignored
        );
    }

    #[test]
    fn intent_activate() {
        let rows = [row("run", "Run")];
        let mut state = CommandPaletteState::new(Some("run"));
        assert_eq!(
            CommandPalette::handle_intent(&mut state, UiIntent::Activate, &rows),
            CommandPaletteOutcome::Activated("run")
        );
        // Single-item list: move may wrap or no-op depending on List.
        let _ =
            CommandPalette::handle_intent(&mut state, UiIntent::Move(NavigationMove::Next), &rows);
    }

    #[test]
    fn paint_empty_footer_ascii_narrow() {
        let system = DesignSystem::default();
        let mut state = CommandPaletteState::<&str>::new(None);
        let area = Rect::new(0, 0, 40, 10);
        let mut buffer = Buffer::empty(area);
        let palette = CommandPalette::new("Commands", &[], &system);
        StatefulWidget::render(&palette, area, &mut buffer, &mut state);
        let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(text.contains("Commands"), "{text:?}");
        assert!(
            text.contains('∅') || text.contains("No commands") || text.contains("empty"),
            "{text:?}"
        );
        assert!(
            text.contains("navigate") || text.contains("esc"),
            "{text:?}"
        );

        let narrow = Rect::new(0, 0, 22, 8);
        let mut nbuf = Buffer::empty(narrow);
        StatefulWidget::render(&palette, narrow, &mut nbuf, &mut state);
        let ntext: String = nbuf.content().iter().map(|c| c.symbol()).collect();
        // Footer dropped on narrow.
        assert!(!ntext.contains("navigate"), "{ntext:?}");

        let mut abuf = Buffer::empty(area);
        let ascii = CommandPalette::new("Commands", &[], &system).ascii(true);
        StatefulWidget::render(&ascii, area, &mut abuf, &mut state);
        let atext: String = abuf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            atext.contains('[') || atext.contains("empty") || atext.contains("No"),
            "{atext:?}"
        );
    }

    #[test]
    fn unfocused_uses_normal_chrome() {
        let system = DesignSystem::default();
        let rows = [row("a", "Alpha")];
        let mut state = CommandPaletteState::new(Some("a"));
        let area = Rect::new(0, 0, 36, 8);
        let mut buffer = Buffer::empty(area);
        let palette = CommandPalette::new("Commands", &rows, &system).focused(false);
        StatefulWidget::render(&palette, area, &mut buffer, &mut state);
        // Border should use Border role not BorderFocused when unfocused.
        assert_eq!(buffer[(0, 0)].fg, system.style(Role::Border).fg.unwrap());
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
