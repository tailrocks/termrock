use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Text,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, clear::Clear, paragraph::Paragraph};

use crate::{
    input::{KeyCode, KeyEvent},
    interaction::{HitRegion, Outcome},
    style::Theme,
};

use super::{Action, ActionBar, ActionBarState, DetailRow, DetailTable, DetailTableState};

#[derive(Debug, Clone, Copy)]
pub struct Backdrop {
    pub symbol: char,
    pub style: Style,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            symbol: ' ',
            style: Style::new()
                .fg(Color::Reset)
                .bg(crate::theme::DIALOG_BACKDROP),
        }
    }
}

impl Widget for &Backdrop {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buffer[(x, y)].set_char(self.symbol).set_style(self.style);
            }
        }
    }
}

#[cfg(test)]
mod backdrop_tests {
    use super::*;
    use ratatui_core::{layout::Position, widgets::StatefulWidget};

    use crate::input::KeyModifiers;

    #[test]
    fn default_backdrop_uses_terminal_background() {
        let backdrop = Backdrop::default();
        assert_eq!(backdrop.symbol, ' ');
        assert_eq!(backdrop.style.fg, Some(Color::Reset));
        assert_eq!(backdrop.style.bg, Some(Color::Reset));
    }

    #[test]
    fn choice_dialog_skips_disabled_actions_and_returns_semantic_outcomes() {
        let actions = [
            Action {
                id: "accept",
                label: "Accept",
                enabled: true,
                style: None,
            },
            Action {
                id: "blocked",
                label: "Blocked",
                enabled: false,
                style: None,
            },
            Action {
                id: "cancel",
                label: "Cancel",
                enabled: true,
                style: None,
            },
        ];
        let mut state = ChoiceDialogState::new(Some("accept"));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &actions),
            Outcome::Changed
        );
        assert_eq!(state.focused, Some("cancel"));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &actions),
            Outcome::Activated("cancel")
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &actions),
            Outcome::Cancelled
        );
    }

    #[test]
    fn choice_dialog_mouse_outcomes_follow_enabled_painted_regions() {
        let actions = [Action {
            id: "accept",
            label: "Accept",
            enabled: true,
            style: None,
        }];
        let dialog = ChoiceDialog {
            dialog: Dialog {
                title: "Choose",
                body: Text::from("Continue?"),
                style: Style::new(),
            },
            actions: &actions,
            gap: " ",
        };
        let area = Rect::new(3, 2, 30, 6);
        let mut buffer = Buffer::empty(area);
        let mut state = ChoiceDialogState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions.len(), 1);
        let region = state.regions[0].area;
        assert_eq!(
            state.activate_at(Position::new(region.x, region.y)),
            Outcome::Activated("accept")
        );
    }
}

#[derive(Debug, Clone)]
pub struct Dialog<'a> {
    pub title: &'a str,
    pub body: Text<'a>,
    pub style: Style,
}
impl Widget for &Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Clear.render(area, buffer);
        Paragraph::new(self.body.clone())
            .block(Block::bordered().title(self.title))
            .style(self.style)
            .render(area, buffer);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChoiceDialogState<Id> {
    pub focused: Option<Id>,
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id: Clone + PartialEq> ChoiceDialogState<Id> {
    #[must_use]
    pub const fn new(focused: Option<Id>) -> Self {
        Self {
            focused,
            regions: Vec::new(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        match key.code {
            KeyCode::Esc => Outcome::Cancelled,
            KeyCode::Enter => self.activate_selected(actions),
            KeyCode::Left | KeyCode::Up | KeyCode::BackTab => self.select_relative(actions, -1),
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => self.select_relative(actions, 1),
            _ => Outcome::Ignored,
        }
    }

    pub fn select_next(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.select_relative(actions, 1)
    }

    pub fn select_previous(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.select_relative(actions, -1)
    }

    fn select_relative(&mut self, actions: &[Action<'_, Id>], direction: isize) -> Outcome<Id> {
        let enabled: Vec<&Action<'_, Id>> =
            actions.iter().filter(|action| action.enabled).collect();
        if enabled.is_empty() {
            self.focused = None;
            return Outcome::Ignored;
        }
        let current = self
            .focused
            .as_ref()
            .and_then(|focused| enabled.iter().position(|action| &action.id == focused));
        let next = match (current, direction.is_negative()) {
            (Some(0), true) | (None, true) => enabled.len() - 1,
            (Some(index), true) => index - 1,
            (Some(index), false) => (index + 1) % enabled.len(),
            (None, false) => 0,
        };
        self.focused = Some(enabled[next].id.clone());
        Outcome::Changed
    }

    #[must_use]
    pub fn activate_selected(&self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.focused
            .as_ref()
            .and_then(|focused| {
                actions
                    .iter()
                    .find(|action| action.enabled && &action.id == focused)
            })
            .map_or(Outcome::Ignored, |action| {
                Outcome::Activated(action.id.clone())
            })
    }

    #[must_use]
    pub fn activate_at(&mut self, position: ratatui_core::layout::Position) -> Outcome<Id> {
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return Outcome::Ignored;
        };
        self.focused = Some(region.id.clone());
        Outcome::Activated(region.id.clone())
    }
}

#[derive(Debug, Clone)]
pub struct ChoiceDialog<'a, Id> {
    pub dialog: Dialog<'a>,
    pub actions: &'a [Action<'a, Id>],
    pub gap: &'a str,
}

impl<Id: Clone + PartialEq> StatefulWidget for &ChoiceDialog<'_, Id> {
    type State = ChoiceDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self.dialog).render(area, buffer);
        if area.height < 3 {
            state.regions.clear();
            return;
        }
        let action_area = Rect::new(
            area.x.saturating_add(1),
            area.bottom().saturating_sub(2),
            area.width.saturating_sub(2),
            1,
        );
        let mut action_state = ActionBarState {
            focused: state.focused.clone(),
            regions: Vec::new(),
        };
        (&ActionBar {
            actions: self.actions,
            gap: self.gap,
        })
            .render(action_area, buffer, &mut action_state);
        state.focused = action_state.focused;
        state.regions = action_state.regions;
    }
}

#[derive(Debug, Clone)]
pub struct MessageDialog<'a, Id> {
    pub dialog: Dialog<'a>,
    pub details: &'a [DetailRow<'a, Id>],
    pub label_width: u16,
    pub wrap: bool,
    pub theme: &'a Theme,
}

impl<Id: Clone + PartialEq> StatefulWidget for &MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self.dialog).render(area, buffer);
        if area.width < 3 || area.height < 3 {
            state.regions.clear();
            return;
        }
        let inner = Rect::new(area.x + 1, area.y + 2, area.width - 2, area.height - 3);
        (&DetailTable {
            rows: self.details,
            label_width: self.label_width,
            wrap: self.wrap,
            theme: self.theme,
        })
            .render(inner, buffer, state);
    }
}
