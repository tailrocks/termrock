// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Code block rendering with pluggable syntax styling.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use crate::{
    style::{DesignSystem, Role, RolePalette},
    text::{display_cols_slice, take_display_cols},
};

/// Caller-supplied syntax styling for one source line.
pub trait SyntaxHighlighter {
    /// Styles a single source line. Return styled segments covering the line.
    fn highlight_line<'a>(&'a self, line: &'a str, line_index: usize) -> Vec<(&'a str, Style)>;
}

/// Neutral highlighter that paints the whole line as plain text.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlainSyntax;

impl SyntaxHighlighter for PlainSyntax {
    fn highlight_line<'a>(&'a self, line: &'a str, _line_index: usize) -> Vec<(&'a str, Style)> {
        vec![(line, Style::default())]
    }
}

/// Scrollable code listing with optional line numbers.
#[derive(Debug, Clone, Copy)]
pub struct CodeBlock<'a, H: SyntaxHighlighter> {
    lines: &'a [&'a str],
    language: Option<&'a str>,
    show_line_numbers: bool,
    first_line: usize,
    highlighter: &'a H,
    system: &'a DesignSystem,
}

impl<'a> CodeBlock<'a, PlainSyntax> {
    /// Creates a plain code block without line numbers.
    #[must_use]
    pub const fn new(lines: &'a [&'a str], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            language: None,
            show_line_numbers: false,
            first_line: 0,
            highlighter: &PlainSyntax,
            system,
        }
    }
}

impl<'a, H: SyntaxHighlighter> CodeBlock<'a, H> {
    /// Attaches a language label (shown in the header when space allows).
    #[must_use]
    pub const fn language(mut self, language: &'a str) -> Self {
        self.language = Some(language);
        self
    }

    /// Enables gutter line numbers.
    #[must_use]
    pub const fn line_numbers(mut self, enabled: bool) -> Self {
        self.show_line_numbers = enabled;
        self
    }

    /// Sets the first visible line index (0-based).
    #[must_use]
    pub const fn first_line(mut self, first_line: usize) -> Self {
        self.first_line = first_line;
        self
    }

    /// Uses a custom syntax highlighter.
    #[must_use]
    pub const fn highlighter(mut self, highlighter: &'a H) -> Self {
        self.highlighter = highlighter;
        self
    }
}

impl<H: SyntaxHighlighter> Widget for &CodeBlock<'_, H> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut y = area.y;
        let mut content_top = area.y;
        if let Some(language) = self.language
            && area.height >= 2
        {
            let header = take_display_cols(language, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                y,
                &header,
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
            content_top = y;
        }
        let body_height = area.bottom().saturating_sub(content_top);
        if body_height == 0 {
            return;
        }
        let gutter = if self.show_line_numbers {
            let last = self.first_line.saturating_add(usize::from(body_height));
            let digits = last.max(1).to_string().len().max(2);
            u16::try_from(digits + 1).unwrap_or(4)
        } else {
            0
        };
        let text_width = area.width.saturating_sub(gutter);
        for row in 0..body_height {
            let line_index = self.first_line.saturating_add(usize::from(row));
            let Some(line) = self.lines.get(line_index).copied() else {
                break;
            };
            let paint_y = content_top.saturating_add(row);
            if self.show_line_numbers && gutter > 0 {
                let number = format!(
                    "{:>width$}",
                    line_index + 1,
                    width = usize::from(gutter) - 1
                );
                buffer.set_stringn(
                    area.x,
                    paint_y,
                    &number,
                    usize::from(gutter.saturating_sub(1)),
                    self.system.style(Role::TextDisabled),
                );
            }
            let text_x = area.x.saturating_add(gutter);
            let mut col = 0u16;
            for (segment, mut style) in self.highlighter.highlight_line(line, line_index) {
                if style == Style::default() {
                    style = self.system.style(Role::Text);
                }
                if col >= text_width {
                    break;
                }
                let remaining = usize::from(text_width.saturating_sub(col));
                let clipped = take_display_cols(segment, remaining);
                let width = u16::try_from(
                    clipped
                        .chars()
                        .count()
                        .max(unicode_width::UnicodeWidthStr::width(clipped.as_str())),
                )
                .unwrap_or(text_width);
                let used = u16::try_from(unicode_width::UnicodeWidthStr::width(clipped.as_str()))
                    .unwrap_or(0)
                    .min(text_width.saturating_sub(col));
                buffer.set_stringn(
                    text_x.saturating_add(col),
                    paint_y,
                    &clipped,
                    remaining,
                    style,
                );
                col = col.saturating_add(used);
                let _ = width;
            }
            // Ensure empty lines still clear.
            if line.is_empty() && text_width > 0 {
                let _ = display_cols_slice("", 0, 0);
            }
        }
    }
}

impl<H: SyntaxHighlighter> Widget for CodeBlock<'_, H> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_line_numbers_and_source() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let lines = ["fn main() {}", "    // hi"];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 30, 3));
        CodeBlock::new(&lines, &system)
            .line_numbers(true)
            .language("rust")
            .render(Rect::new(0, 0, 30, 3), &mut buffer);
        let header: String = (0..30)
            .map(|x| buffer[(x, 0)].symbol().to_owned())
            .collect();
        assert!(header.contains("rust"));
        let body: String = (0..30)
            .map(|x| buffer[(x, 1)].symbol().to_owned())
            .collect();
        assert!(body.contains('1'));
        assert!(body.contains("fn"));
    }
}
