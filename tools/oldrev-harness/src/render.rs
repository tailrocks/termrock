// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Clear},
};
use termrock::{Theme, style::PREVIEW_CARD};

use crate::stories::Story;

const STORY_PAD: u16 = 1;

/// Render with the Old-rev export ground. `Color::Reset` becomes white
/// foreground and black background in `termrock-raster`.
pub(crate) fn render_story_to_buffer(story: Story, theme: &Theme) -> Buffer {
    let width = story.width.saturating_add(STORY_PAD * 2);
    let height = story.height.saturating_add(STORY_PAD * 2);
    let backend = TestBackend::new(width, height);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => match error {},
    };
    match terminal.draw(|frame| {
        let area = frame.area();
        frame.render_widget(
            Block::default().style(Style::default().bg(PREVIEW_CARD)),
            area,
        );
        let inner = Rect {
            x: STORY_PAD,
            y: STORY_PAD,
            width: story.width,
            height: story.height,
        };
        frame.render_widget(Clear, inner);
        story.render(frame, inner, theme);
    }) {
        Ok(_) => {}
        Err(error) => match error {},
    }
    terminal.backend().buffer().clone()
}
