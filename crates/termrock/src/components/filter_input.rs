// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical single-row filter input component.

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Modifier, Style};
use ratatui_core::text::{Line, Span};
use ratatui_core::widgets::Widget;
use ratatui_widgets::paragraph::Paragraph;

use crate::theme::{INK, PHOSPHOR_DARK, PHOSPHOR_GREEN, WHITE};

#[derive(Debug, Clone, Copy)]
pub struct FilterInput<'a> {
    filter: &'a str,
}

impl<'a> FilterInput<'a> {
    #[must_use]
    pub const fn new(filter: &'a str) -> Self {
        Self { filter }
    }
}

impl Widget for FilterInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(filter_input_line(self.filter)).render(area, buf);
    }
}

#[must_use]
pub fn filter_input_line(filter: &str) -> Line<'static> {
    if filter.is_empty() {
        Line::from(vec![
            Span::styled("Filter: ", crate::theme::DIM),
            Span::styled("\u{2591}".repeat(20), Style::default().fg(PHOSPHOR_DARK)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Filter: ", crate::theme::DIM),
            Span::styled(filter.to_owned(), Style::default().fg(WHITE)),
            Span::styled(
                "\u{2588}",
                Style::default()
                    .fg(INK)
                    .bg(PHOSPHOR_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    }
}

pub fn render_filter_input(
    frame: &mut ratatui_core::terminal::Frame<'_>,
    area: Rect,
    filter: &str,
) {
    frame.render_widget(FilterInput::new(filter), area);
}

#[cfg(test)]
mod tests;
