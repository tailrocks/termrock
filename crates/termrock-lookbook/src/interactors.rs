// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use ratatui::{
    Frame,
    layout::{Position, Rect},
};
use termrock::{
    Theme,
    input::{Event, KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    interaction::Outcome,
    widgets::{
        Anchor, ChoiceDialogState, Form, FormOutcome, FormSection, FormState, List, ListState,
        LogPane, LogPaneState, Picker, PickerOutcome, PickerState, Severity, SplitDirection,
        SplitPane, SplitPaneOutcome, SplitPaneState, SplitRatio, TextArea, TextAreaOutcome,
        TextAreaState, TextInput, TextInputOutcome, TextInputState, Toast, Tree, TreeNode,
        TreeOutcome, TreeState,
    },
};

use crate::knobs::{Knob, KnobValue};
use crate::stories::{
    SPLIT_PANE_MAX, SPLIT_PANE_MIN, choice_actions, form_fields, list_rows, picker_rows,
    render_choice_dialog, render_split_pane, tree_nodes,
};

trait PointerTarget {
    fn hover(&mut self, _position: Position) -> bool {
        false
    }
    fn click_at(&mut self, _position: Position) -> bool {
        false
    }
    fn drag_to(&mut self, _position: Position) -> bool {
        false
    }
    fn wheel(&mut self, _delta: isize) -> bool {
        false
    }
}

fn route_pointer(target: &mut impl PointerTarget, mouse: MouseEvent, preview_area: Rect) -> bool {
    let position = mouse.position;
    if !preview_area.contains(position) {
        return target.hover(position);
    }
    match mouse.kind {
        MouseEventKind::Moved => target.hover(position),
        MouseEventKind::Down(MouseButton::Left) => target.click_at(position),
        MouseEventKind::Drag(MouseButton::Left) => target.drag_to(position),
        MouseEventKind::ScrollUp => target.wheel(-1),
        MouseEventKind::ScrollDown => target.wheel(1),
        _ => false,
    }
}

pub(crate) trait StoryInteraction {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect);
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool;
    fn set_theme(&mut self, theme: Theme);
    fn knobs(&self) -> &[Knob] {
        &[]
    }
    fn handle_knob_key(&mut self, _selected: usize, _key: KeyEvent) -> bool {
        false
    }
    fn render_knob_editor(&mut self, _selected: usize, _frame: &mut Frame<'_>, _area: Rect) {}
    fn handle_preview_escape(&mut self, _key: KeyEvent) -> bool {
        false
    }
    fn captures_text_input(&self) -> bool {
        false
    }
    fn knob_captures_text_input(&self, _selected: usize) -> bool {
        false
    }
}

pub(crate) struct StaticStory {
    pub(crate) render_fn: fn(&mut Frame<'_>, Rect, &Theme),
    pub(crate) theme: Theme,
}

impl StoryInteraction for StaticStory {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        (self.render_fn)(frame, area, &self.theme);
    }
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }
    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }
    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

pub(crate) struct TextAreaInteractor {
    state: TextAreaState,
    theme: Theme,
}

impl TextAreaInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TextAreaState::new("First line\nSecond line");
        state.set_focused(true);
        Self {
            state,
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for TextAreaInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &TextArea::new(&self.theme).title("Compose"),
            area,
            &mut self.state,
        );
    }
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(key), TextAreaOutcome::Ignored)
    }
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        preview_area.contains(mouse.position)
            && !matches!(
                self.state.handle_event(Event::Mouse(mouse)),
                TextAreaOutcome::Ignored
            )
    }
    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
    fn captures_text_input(&self) -> bool {
        true
    }
}

pub(crate) struct ChoiceDialogInteractor {
    state: ChoiceDialogState<&'static str>,
    theme: Theme,
}

impl ChoiceDialogInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: ChoiceDialogState::new(Some("continue")),
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for ChoiceDialogInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        render_choice_dialog(frame, area, &mut self.state, &self.theme);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(
            self.state.handle_key(&choice_actions(), key),
            Outcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let position = mouse.position;
        if !preview_area.contains(position) {
            return false;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            return !matches!(self.state.click(position), Outcome::Ignored);
        }
        false
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

pub(crate) struct ListInteractor {
    state: ListState<&'static str>,
    theme: Theme,
}

impl ListInteractor {
    pub(crate) fn new() -> Self {
        let mut state = ListState::new(Some("beta"));
        state.enable_multi_select();
        state.selection_mut().unwrap().toggle(&"alpha");
        Self {
            state,
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for ListInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = list_rows();
        frame.render_stateful_widget(&List::new(&rows, &self.theme), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(&list_rows(), key), Outcome::Ignored)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

impl PointerTarget for ListInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.hovered().cloned();
        self.state.hover(position);
        self.state.hovered() != before.as_ref()
    }
    fn click_at(&mut self, position: Position) -> bool {
        !matches!(self.state.click(position), Outcome::Ignored)
    }
    fn drag_to(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position, list_rows().len())
    }
    fn wheel(&mut self, delta: isize) -> bool {
        self.state.scroll_by(delta, list_rows().len())
    }
}

pub(crate) struct PickerInteractor {
    state: PickerState<&'static str>,
    theme: Theme,
    activated: Option<&'static str>,
}

impl PickerInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: PickerState::new(Some("alpha")),
            theme: Theme::default(),
            activated: None,
        }
    }
}

impl StoryInteraction for PickerInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = picker_rows(self.state.query_text());
        frame.render_stateful_widget(&Picker::new(&rows, &self.theme), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let rows = picker_rows(self.state.query_text());
        match self.state.handle_key(&rows, key) {
            PickerOutcome::QueryChanged => {
                let rows = picker_rows(self.state.query_text());
                self.state.reconcile(&rows);
                true
            }
            PickerOutcome::Activated(id) => {
                self.activated = Some(id);
                true
            }
            PickerOutcome::SelectionChanged => true,
            PickerOutcome::Ignored | PickerOutcome::Cancelled => false,
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        self.handle_key(key)
    }

    fn captures_text_input(&self) -> bool {
        true
    }
}

impl PointerTarget for PickerInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.list().hovered().cloned();
        self.state.hover(position);
        self.state.list().hovered() != before.as_ref()
    }

    fn click_at(&mut self, position: Position) -> bool {
        if let PickerOutcome::Activated(id) = self.state.click(position) {
            self.activated = Some(id);
            true
        } else {
            false
        }
    }

    fn wheel(&mut self, delta: isize) -> bool {
        self.state
            .scroll_by(delta, picker_rows(self.state.query_text()).len())
    }
}

pub(crate) struct LogPaneInteractor {
    state: LogPaneState,
    theme: Theme,
}

impl LogPaneInteractor {
    pub(crate) fn new() -> Self {
        let mut state = LogPaneState::new().with_max_lines(200);
        for line in [
            "[12:04:01] resolving workspace",
            "[12:04:02] compiling termrock",
            "[12:04:03] running 205 tests",
            "[12:04:04] result: ok ✓",
            "[12:04:05] preview ready",
            "[12:04:06] waiting for changes",
        ] {
            state.append(line);
        }
        Self {
            state,
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for LogPaneInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &LogPane::new(&self.theme).title("Build log"),
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(key), Outcome::Ignored)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

impl PointerTarget for LogPaneInteractor {
    fn wheel(&mut self, delta: isize) -> bool {
        self.state.scroll_by(delta)
    }
}

pub(crate) struct TreeInteractor {
    nodes: Vec<TreeNode<'static, &'static str>>,
    state: TreeState<&'static str>,
    theme: Theme,
}

impl TreeInteractor {
    pub(crate) fn new() -> Self {
        let mut state = TreeState::new(Some("workspace"));
        state.enable_multi_select();
        state.selection_mut().unwrap().toggle(&"notes");
        Self {
            nodes: tree_nodes(),
            state,
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for TreeInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(&Tree::new(&self.nodes, &self.theme), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(
            self.state.handle_key(&self.nodes, key),
            TreeOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

impl PointerTarget for TreeInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.hovered().cloned();
        self.state.hover(position);
        self.state.hovered() != before.as_ref()
    }
    fn click_at(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position, self.nodes.len())
            || !matches!(self.state.click(position), TreeOutcome::Ignored)
    }
    fn drag_to(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position, self.nodes.len())
    }
    fn wheel(&mut self, delta: isize) -> bool {
        self.state.scroll_by(delta, self.nodes.len());
        true
    }
}

pub(crate) struct FormInteractor {
    state: FormState<&'static str>,
    theme: Theme,
}

impl FormInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: FormState::new(Some("name")),
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for FormInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        // FormSection borrows its fields, so storing both in the interactor
        // would be self-referential. Rebuild this tiny fixture at each call.
        let fields = form_fields();
        let sections = [FormSection {
            title: ratatui::text::Line::from("General"),
            fields: &fields,
        }];
        frame.render_stateful_widget(&Form::new(&sections, &self.theme), area, &mut self.state);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let fields = form_fields();
        let sections = [FormSection {
            title: ratatui::text::Line::from("General"),
            fields: &fields,
        }];
        !matches!(self.state.handle_key(&sections, key), FormOutcome::Ignored)
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        route_pointer(self, mouse, preview_area)
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

impl PointerTarget for FormInteractor {
    fn hover(&mut self, position: Position) -> bool {
        let before = self.state.hovered().cloned();
        self.state.hover(position);
        self.state.hovered() != before.as_ref()
    }
    fn click_at(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position)
            || !matches!(self.state.click(position), FormOutcome::Ignored)
    }
    fn drag_to(&mut self, position: Position) -> bool {
        self.state.scroll_to_position(position)
    }
    fn wheel(&mut self, delta: isize) -> bool {
        let content_len = self.state.content_height();
        self.state.scroll_by(delta, content_len);
        true
    }
}

pub(crate) struct SplitPaneInteractor {
    state: SplitPaneState,
    theme: Theme,
}

impl SplitPaneInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: SplitPaneState::new(SplitRatio::from_percent(38)),
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for SplitPaneInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        render_split_pane(frame, area, &mut self.state, &self.theme);
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let split = SplitPane::new(
            SplitDirection::Horizontal,
            SPLIT_PANE_MIN,
            SPLIT_PANE_MAX,
            &self.theme,
        );
        !matches!(
            self.state.handle_key(&split, key),
            SplitPaneOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let position = mouse.position;
        let split = SplitPane::new(
            SplitDirection::Horizontal,
            SPLIT_PANE_MIN,
            SPLIT_PANE_MAX,
            &self.theme,
        );
        match mouse.kind {
            MouseEventKind::Moved => self.state.hover(&split, position),
            MouseEventKind::Down(MouseButton::Left) => !matches!(
                self.state.drag_start(&split, position),
                SplitPaneOutcome::Ignored
            ),
            MouseEventKind::Drag(MouseButton::Left) => !matches!(
                self.state.drag_move(&split, position),
                SplitPaneOutcome::Ignored
            ),
            MouseEventKind::Up(MouseButton::Left) => {
                let changed = self.state.is_dragging();
                self.state.drag_end();
                changed
            }
            _ => false,
        }
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}

pub(crate) struct ToastInteractor {
    knobs: Vec<Knob>,
    message: TextInputState,
    theme: Theme,
}

impl ToastInteractor {
    pub(crate) fn new() -> Self {
        Self {
            knobs: vec![
                Knob {
                    id: "severity",
                    label: "Severity",
                    value: KnobValue::Choice(1),
                    choices: &["Info", "Success", "Warning", "Error"],
                },
                Knob {
                    id: "anchor",
                    label: "Anchor",
                    value: KnobValue::Choice(1),
                    choices: &["Top left", "Top right", "Bottom left", "Bottom right"],
                },
                Knob {
                    id: "message",
                    label: "Message",
                    value: KnobValue::Text("Updated".to_owned()),
                    choices: &[],
                },
            ],
            message: TextInputState::new("Updated").with_max_graphemes(48),
            theme: Theme::default(),
        }
    }

    fn severity(&self) -> Severity {
        match self.knobs[0].value {
            KnobValue::Choice(0) => Severity::Info,
            KnobValue::Choice(2) => Severity::Warning,
            KnobValue::Choice(3) => Severity::Error,
            _ => Severity::Success,
        }
    }

    fn anchor(&self) -> Anchor {
        match self.knobs[1].value {
            KnobValue::Choice(0) => Anchor::TopLeft,
            KnobValue::Choice(2) => Anchor::BottomLeft,
            KnobValue::Choice(3) => Anchor::BottomRight,
            _ => Anchor::TopRight,
        }
    }
}

impl StoryInteraction for ToastInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_widget(
            Toast::new(&self.theme, self.message.value(), self.severity()).anchor(self.anchor()),
            area,
        );
    }

    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }

    fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    fn knobs(&self) -> &[Knob] {
        &self.knobs
    }

    fn handle_knob_key(&mut self, selected: usize, key: KeyEvent) -> bool {
        let Some(knob) = self.knobs.get_mut(selected) else {
            return false;
        };
        match &mut knob.value {
            KnobValue::Choice(index) if matches!(key.code, KeyCode::Left | KeyCode::Right) => {
                let count = knob.choices.len();
                if count == 0 {
                    return false;
                }
                *index = if key.code == KeyCode::Right {
                    (*index + 1) % count
                } else {
                    (*index + count - 1) % count
                };
                true
            }
            KnobValue::Text(value) => {
                let changed = !matches!(self.message.handle_key(key), TextInputOutcome::Ignored);
                *value = self.message.value().to_owned();
                changed
            }
            KnobValue::Bool(_) | KnobValue::Number(_) | KnobValue::Choice(_) => false,
        }
    }

    fn render_knob_editor(&mut self, selected: usize, frame: &mut Frame<'_>, area: Rect) {
        if selected == 2 {
            frame.render_stateful_widget(
                &TextInput::new("Message", &self.theme).placeholder("Toast message"),
                area,
                &mut self.message,
            );
        }
    }

    fn knob_captures_text_input(&self, selected: usize) -> bool {
        selected == 2
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::{Position, Rect},
    };
    use termrock::input::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    use termrock::input::{KeyCode, KeyEvent};

    use super::{
        FormInteractor, LogPaneInteractor, SplitPaneInteractor, StoryInteraction, ToastInteractor,
    };

    #[test]
    fn log_pane_wheel_freezes_tail_following() {
        let area = Rect::new(0, 0, 52, 5);
        let mut interactor = LogPaneInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();

        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                position: Position::new(1, 1),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(!interactor.state.is_following());
    }

    #[test]
    fn form_hover_clears_when_pointer_leaves_preview() {
        let area = Rect::new(0, 0, 68, 12);
        let mut interactor = FormInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();

        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                position: Position::new(0, 2),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert_eq!(interactor.state.hovered(), Some(&"name"));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                position: Position::new(area.right(), area.bottom()),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert_eq!(interactor.state.hovered(), None);
    }

    #[test]
    fn split_pane_interactor_drags_only_from_painted_divider() {
        let area = Rect::new(0, 0, 68, 10);
        let mut interactor = SplitPaneInteractor::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
        terminal
            .draw(|frame| interactor.render(frame, area))
            .unwrap();
        let divider = interactor.state.layout().divider;
        let before = interactor.state.ratio();

        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(divider.x, divider.y),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                position: Position::new(50, divider.y),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Up(MouseButton::Left),
                position: Position::new(50, divider.y),
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.state.ratio() > before);
    }

    #[test]
    fn toast_knobs_keep_golden_defaults_and_edit_live() {
        let mut interactor = ToastInteractor::new();
        assert_eq!(interactor.knobs()[0].display_value(), "Success");
        assert_eq!(interactor.knobs()[1].display_value(), "Top right");
        assert_eq!(interactor.knobs()[2].display_value(), "Updated");

        assert!(interactor.handle_knob_key(0, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)));
        assert_eq!(interactor.knobs()[0].display_value(), "Warning");
        assert!(
            interactor.handle_knob_key(2, KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE))
        );
        assert_eq!(interactor.knobs()[2].display_value(), "Updated!");
    }
}
