// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::{Frame, layout::Rect};

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
