use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::Text,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{clear::Clear, paragraph::Paragraph};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{HitRegion, Outcome},
    style::Theme,
};

use super::{
    Action, ActionBar, ActionBarState, DetailRow, DetailTable, DetailTableState, Panel,
    PanelEmphasis,
};

#[derive(Debug, Clone, Copy)]
/// A themed fill painted behind modal content.
pub struct Backdrop {
    symbol: char,
    style: Style,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            symbol: ' ',
            style: Style::new()
                .fg(Color::Reset)
                .bg(crate::style::DIALOG_BACKDROP),
        }
    }
}

impl Backdrop {
    #[must_use]
    /// Creates a fully opaque backdrop from a semantic theme.
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    /// Sets the fill symbol used across the backdrop.
    pub const fn symbol(mut self, symbol: char) -> Self {
        self.symbol = symbol;
        self
    }

    #[must_use]
    /// Sets the style used to fill the backdrop.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
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

impl Widget for Backdrop {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
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
            state.handle_key(&actions, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.focused, Some("cancel"));
        assert_eq!(
            state.handle_key(&actions, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Outcome::Activated("cancel")
        );
        assert_eq!(
            state.handle_key(&actions, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
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
        let theme = Theme::default();
        let dialog = ChoiceDialog::new(
            Dialog::new("Choose", Text::from("Continue?"), &theme).emphasis(PanelEmphasis::Focused),
            &actions,
        );
        let area = Rect::new(3, 2, 30, 6);
        let mut buffer = Buffer::empty(area);
        let mut state = ChoiceDialogState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions.len(), 1);
        let region = state.regions[0].area;
        assert_eq!(
            state.click(Position::new(region.x, region.y)),
            Outcome::Activated("accept")
        );
    }

    #[test]
    fn message_details_start_after_wrapped_body() {
        let details = [DetailRow {
            id: "stage",
            label: "Stage",
            value: "Build",
            href: None,
            capability: super::super::DetailCapability::None,
            emphasis: false,
            style: None,
        }];
        let theme = Theme::default();
        let dialog = MessageDialog::new(
            Dialog::new("Failure", Text::from("a message that wraps"), &theme)
                .emphasis(PanelEmphasis::Focused),
            &details,
            &theme,
        )
        .wrap(true);
        let area = Rect::new(0, 0, 12, 8);
        let mut buffer = Buffer::empty(area);
        let mut state = DetailTableState::default();
        (&dialog).render(area, &mut buffer, &mut state);
        assert_eq!(state.viewport.y, 3);
    }

    #[test]
    fn dialog_uses_semantic_focused_panel_chrome() {
        let theme = Theme::default();
        let dialog =
            Dialog::new(" Notice ", Text::from("Done"), &theme).emphasis(PanelEmphasis::Focused);
        let area = Rect::new(0, 0, 18, 4);
        let mut buffer = Buffer::empty(area);
        (&dialog).render(area, &mut buffer);

        assert_eq!(
            buffer[(0, 0)].fg,
            theme.style(crate::style::Role::BorderFocused).fg.unwrap()
        );
        assert!(
            buffer
                .content()
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .contains(" Notice ")
        );
    }
}

#[derive(Debug, Clone)]
/// A framed modal surface with resolved geometry.
pub struct Dialog<'a> {
    title: &'a str,
    body: Text<'a>,
    style: Style,
    theme: &'a Theme,
    emphasis: PanelEmphasis,
}

impl<'a> Dialog<'a> {
    #[must_use]
    /// Creates a dialog from a geometry specification and semantic theme.
    pub const fn new(title: &'a str, body: Text<'a>, theme: &'a Theme) -> Self {
        Self {
            title,
            body,
            style: Style::new(),
            theme,
            emphasis: PanelEmphasis::Normal,
        }
    }

    #[must_use]
    /// Overrides the theme-derived dialog body style.
    pub const fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    #[must_use]
    /// Sets the semantic panel emphasis.
    pub const fn emphasis(mut self, emphasis: PanelEmphasis) -> Self {
        self.emphasis = emphasis;
        self
    }
}
impl Widget for &Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Clear.render(area, buffer);
        let panel = Panel::new(self.theme)
            .title(self.title)
            .emphasis(self.emphasis);
        Paragraph::new(self.body.clone())
            .block(panel.block())
            .style(self.style)
            .render(area, buffer);
    }
}

impl Widget for Dialog<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `ChoiceDialog`.
pub struct ChoiceDialogState<Id> {
    /// Whether this item is focused.
    pub focused: Option<Id>,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for ChoiceDialogState<Id> {
    fn default() -> Self {
        Self {
            focused: None,
            regions: Vec::new(),
        }
    }
}

impl<Id: Clone + PartialEq> ChoiceDialogState<Id> {
    #[must_use]
    /// Creates choice-dialog state with no focused or hovered action.
    pub const fn new(focused: Option<Id>) -> Self {
        Self {
            focused,
            regions: Vec::new(),
        }
    }

    /// Handles the `handle_key` interaction.
    pub fn handle_key(&mut self, actions: &[Action<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => Outcome::Cancelled,
            KeyCode::Enter => self.activate_selected(actions),
            KeyCode::Left | KeyCode::Up | KeyCode::BackTab => self.select_relative(actions, -1),
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => self.select_relative(actions, 1),
            _ => Outcome::Ignored,
        }
    }

    /// Moves selection to the next enabled item, wrapping at the end.
    pub fn select_next(&mut self, actions: &[Action<'_, Id>]) -> Outcome<Id> {
        self.select_relative(actions, 1)
    }

    /// Moves selection to the previous enabled item, wrapping at the start.
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
    /// Returns the semantic outcome for the currently selected item.
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
    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: ratatui_core::layout::Position) -> Outcome<Id> {
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
/// A modal choice prompt with stable action identities.
pub struct ChoiceDialog<'a, Id> {
    dialog: Dialog<'a>,
    actions: &'a [Action<'a, Id>],
    gap: &'a str,
}

impl<'a, Id> ChoiceDialog<'a, Id> {
    #[must_use]
    /// Creates a choice dialog over borrowed actions and mutable state.
    pub const fn new(dialog: Dialog<'a>, actions: &'a [Action<'a, Id>]) -> Self {
        Self {
            dialog,
            actions,
            gap: " ",
        }
    }

    #[must_use]
    /// Sets spacing between adjacent items in terminal cells.
    pub const fn gap(mut self, gap: &'a str) -> Self {
        self.gap = gap;
        self
    }
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
        (&ActionBar::new(self.actions, self.dialog.theme).gap(self.gap)).render(
            action_area,
            buffer,
            &mut action_state,
        );
        state.focused = action_state.focused;
        state.regions = action_state.regions;
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for ChoiceDialog<'_, Id> {
    type State = ChoiceDialogState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[derive(Debug, Clone)]
/// A message dialog with optional scrollable details.
pub struct MessageDialog<'a, Id> {
    dialog: Dialog<'a>,
    details: &'a [DetailRow<'a, Id>],
    label_width: u16,
    wrap: bool,
    theme: &'a Theme,
}

impl<'a, Id> MessageDialog<'a, Id> {
    #[must_use]
    /// Creates a message dialog with no details and zero scroll offset.
    pub const fn new(
        dialog: Dialog<'a>,
        details: &'a [DetailRow<'a, Id>],
        theme: &'a Theme,
    ) -> Self {
        Self {
            dialog,
            details,
            label_width: 0,
            wrap: false,
            theme,
        }
    }

    #[must_use]
    /// Reserves a fixed label width in terminal display columns.
    pub const fn label_width(mut self, label_width: u16) -> Self {
        self.label_width = label_width;
        self
    }

    #[must_use]
    /// Sets whether long content wraps instead of scrolling horizontally.
    pub const fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        (&self.dialog).render(area, buffer);
        if area.width < 3 || area.height < 3 {
            state.regions.clear();
            return;
        }
        let content_width = usize::from(area.width.saturating_sub(2)).max(1);
        let body_height = self
            .dialog
            .body
            .lines
            .iter()
            .map(|line| line.width().div_ceil(content_width).max(1))
            .sum::<usize>()
            .min(usize::from(area.height.saturating_sub(2)));
        let body_height = u16::try_from(body_height).unwrap_or(u16::MAX);
        let inner = Rect::new(
            area.x + 1,
            area.y.saturating_add(1).saturating_add(body_height),
            area.width - 2,
            area.height.saturating_sub(body_height).saturating_sub(2),
        );
        (&DetailTable::new(self.details, self.theme)
            .label_width(self.label_width)
            .wrap(self.wrap))
            .render(inner, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for MessageDialog<'_, Id> {
    type State = DetailTableState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}
