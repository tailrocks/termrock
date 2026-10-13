// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};
use termrock::{
    Theme,
    widgets::{Tree, TreeNode, TreeOutcome, TreeState},
};

use crate::stories::tree_nodes;

pub(crate) trait StoryInteraction {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect);
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn handle_mouse(&mut self, mouse: MouseEvent, preview_area: Rect) -> bool;
}

pub(crate) struct StaticStory {
    pub(crate) render_fn: fn(&mut Frame<'_>, Rect),
}

impl StoryInteraction for StaticStory {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        (self.render_fn)(frame, area);
    }
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        false
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
            return false;
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
