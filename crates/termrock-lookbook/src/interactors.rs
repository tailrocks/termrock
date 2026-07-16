// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use termrock::{
    Theme,
    interaction::Outcome,
    widgets::{
        ChoiceDialogState, Form, FormOutcome, FormSection, FormState, List, ListOutcome, ListState,
        SplitDirection, SplitPane, SplitPaneOutcome, SplitPaneState, SplitRatio, TextInput,
        TextInputOutcome, TextInputState, Tree, TreeNode, TreeOutcome, TreeState, Validation,
    },
};

use crate::stories::{
    choice_actions, form_fields, list_rows, render_choice_dialog, render_split_pane, tree_nodes,
};

pub(crate) trait StoryInteraction {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect);
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool;
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
            self.state.handle_key(key.into(), &choice_actions()),
            Outcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !preview_area.contains(position) {
            return false;
        }
        if mouse.kind == crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        {
            return !matches!(self.state.activate_at(position), Outcome::Ignored);
        }
        false
    }
}

pub(crate) struct ListInteractor {
    state: ListState<&'static str>,
    theme: Theme,
}

impl ListInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: ListState::new(Some("beta")),
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for ListInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let rows = list_rows();
        frame.render_stateful_widget(
            &List {
                rows: &rows,
                theme: &self.theme,
            },
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(
            self.state.handle_key(&list_rows(), key.into()),
            ListOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !preview_area.contains(position) {
            let changed = self.state.hovered.is_some();
            self.state.hover(position);
            return changed;
        }
        match mouse.kind {
            crossterm::event::MouseEventKind::Moved => {
                self.state.hover(position);
                true
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                !matches!(self.state.click(position), ListOutcome::Ignored)
            }
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                self.state.scroll_to_position(position, list_rows().len())
            }
            crossterm::event::MouseEventKind::ScrollUp => {
                self.state.scroll_by(-1, list_rows().len())
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                self.state.scroll_by(1, list_rows().len())
            }
            _ => false,
        }
    }
}

pub(crate) struct TextInputInteractor {
    state: TextInputState,
    theme: Theme,
}

impl TextInputInteractor {
    pub(crate) fn new() -> Self {
        Self {
            state: TextInputState::new("search").with_max_graphemes(32),
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for TextInputInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &TextInput {
                label: "Filter",
                placeholder: "Type to filter",
                validation: Validation::Valid,
                theme: &self.theme,
            },
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(self.state.handle_key(key.into()), TextInputOutcome::Ignored)
    }

    fn handle_mouse(&mut self, _mouse: MouseEvent, _preview_area: Rect) -> bool {
        false
    }
}

pub(crate) struct TreeInteractor {
    nodes: Vec<TreeNode<'static, &'static str>>,
    state: TreeState<&'static str>,
    theme: Theme,
}

impl TreeInteractor {
    pub(crate) fn new() -> Self {
        Self {
            nodes: tree_nodes(),
            state: TreeState::new(Some("workspace")),
            theme: Theme::default(),
        }
    }
}

impl StoryInteraction for TreeInteractor {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        frame.render_stateful_widget(
            &Tree {
                nodes: &self.nodes,
                theme: &self.theme,
            },
            area,
            &mut self.state,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        !matches!(
            self.state.handle_key(&self.nodes, key.into()),
            TreeOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !preview_area.contains(position) {
            let changed = self.state.hovered().is_some();
            self.state.hover(position);
            return changed;
        }
        match mouse.kind {
            crossterm::event::MouseEventKind::Moved => {
                self.state.hover(position);
                true
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                self.state.scroll_to_position(position, self.nodes.len())
                    || !matches!(self.state.click(position), TreeOutcome::Ignored)
            }
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                self.state.scroll_to_position(position, self.nodes.len())
            }
            crossterm::event::MouseEventKind::ScrollUp => {
                self.state.scroll_by(-1, self.nodes.len());
                true
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                self.state.scroll_by(1, self.nodes.len());
                true
            }
            _ => false,
        }
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
        !matches!(
            self.state.handle_key(&sections, key.into()),
            FormOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        if !preview_area.contains(position) {
            let changed = self.state.hovered().is_some();
            self.state.hover(position);
            return changed;
        }
        match mouse.kind {
            crossterm::event::MouseEventKind::Moved => {
                self.state.hover(position);
                true
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                self.state.scroll_to_position(position)
                    || !matches!(self.state.click(position), FormOutcome::Ignored)
            }
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                self.state.scroll_to_position(position)
            }
            crossterm::event::MouseEventKind::ScrollUp => {
                self.state.scroll_by(-1);
                true
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                self.state.scroll_by(1);
                true
            }
            _ => false,
        }
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
        let split = SplitPane::new(SplitDirection::Horizontal, 12, 16, &self.theme);
        !matches!(
            split.handle_key(&mut self.state, key.into()),
            SplitPaneOutcome::Ignored
        )
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        let position = ratatui::layout::Position::new(mouse.column, mouse.row);
        let split = SplitPane::new(SplitDirection::Horizontal, 12, 16, &self.theme);
        match mouse.kind {
            crossterm::event::MouseEventKind::Moved => {
                split.pointer_move(&mut self.state, position)
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                !matches!(
                    split.pointer_down(&mut self.state, position),
                    SplitPaneOutcome::Ignored
                )
            }
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                !matches!(
                    split.pointer_drag(&mut self.state, position),
                    SplitPaneOutcome::Ignored
                )
            }
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                let changed = self.state.is_dragging();
                split.pointer_up(&mut self.state);
                changed
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};

    use super::{FormInteractor, SplitPaneInteractor, StoryInteraction};

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
                column: 0,
                row: 2,
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert_eq!(interactor.state.hovered(), Some(&"name"));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Moved,
                column: area.right(),
                row: area.bottom(),
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
                kind: MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column: divider.x,
                row: divider.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Drag(crossterm::event::MouseButton::Left),
                column: 50,
                row: divider.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Up(crossterm::event::MouseButton::Left),
                column: 50,
                row: divider.y,
                modifiers: KeyModifiers::NONE,
            },
            area,
        ));
        assert!(interactor.state.ratio() > before);
    }
}
