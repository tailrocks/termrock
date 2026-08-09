// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lightweight markdown projection for agent streams and docs.
//!
//! TermRock owns display roles for common block kinds. Callers own full
//! markdown parsing if they need CommonMark fidelity — this widget accepts an
//! already-projected block list so parsing stays optional and dependency-free.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{
        DesignSystem,
        Role,
        RolePalette,
    },
    text::{
        display_cols,
        take_display_cols,
    },
};

/// Semantic block kinds in a markdown-like stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MarkdownBlockKind {
    /// Ordinary paragraph.
    Paragraph,
    /// Heading level 1–3 (caller chooses emphasis).
    Heading,
    /// Fenced or indented code line.
    Code,
    /// Quote line.
    Quote,
    /// Unordered list item.
    ListItem,
    /// Horizontal rule.
    Rule,
}

/// One borrowed markdown block line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownBlock<'a> {
    /// Semantic kind.
    pub kind: MarkdownBlockKind,
    /// Visible text (already unwrapped to a single terminal line or short wrap unit).
    pub text: &'a str,
}

/// Viewport over projected markdown blocks.
#[derive(Debug, Clone, Copy)]
pub struct MarkdownView<'a> {
    blocks: &'a [MarkdownBlock<'a>],
    first: usize,
    system: &'a DesignSystem,
}

impl<'a> MarkdownView<'a> {
    /// Creates a markdown view starting at the first block.
    #[must_use]
    pub const fn new(blocks: &'a [MarkdownBlock<'a>], system: &'a DesignSystem) -> Self {
        Self {
            blocks,
            first: 0,
            system,
        }
    }

    /// Sets the first visible block index.
    #[must_use]
    pub const fn first(mut self, first: usize) -> Self {
        self.first = first;
        self
    }
}

impl Widget for &MarkdownView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        for row in 0..area.height {
            let index = self.first.saturating_add(usize::from(row));
            let Some(block) = self.blocks.get(index) else {
                break;
            };
            let y = area.y.saturating_add(row);
            let (prefix, role) = match block.kind {
                MarkdownBlockKind::Paragraph => ("", Role::Text),
                MarkdownBlockKind::Heading => ("", Role::TextStrong),
                MarkdownBlockKind::Code => (" ", Role::Info),
                MarkdownBlockKind::Quote => ("│ ", Role::TextMuted),
                MarkdownBlockKind::ListItem => ("• ", Role::Text),
                MarkdownBlockKind::Rule => ("", Role::Border),
            };
            let content = if matches!(block.kind, MarkdownBlockKind::Rule) {
                "─".repeat(usize::from(area.width))
            } else {
                format!("{prefix}{}", block.text)
            };
            let clipped = take_display_cols(&content, usize::from(area.width));
            // Code lines get a muted background stripe when width allows.
            if matches!(block.kind, MarkdownBlockKind::Code) {
                buffer.set_style(
                    Rect::new(area.x, y, area.width, 1),
                    self.system.style(Role::Surface),
                );
            }
            buffer.set_stringn(
                area.x,
                y,
                &clipped,
                usize::from(area.width),
                self.system.style(role),
            );
            let _ = display_cols(&clipped);
        }
    }
}

impl Widget for MarkdownView<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Projects plain text paragraphs into [`MarkdownBlock`]s (split on newlines).
#[must_use]
pub fn project_plain_lines(text: &str) -> Vec<MarkdownBlock<'_>> {
    text.lines()
        .map(|line| {
            let kind = if line.starts_with('#') {
                MarkdownBlockKind::Heading
            } else if line.starts_with("```") || line.starts_with("    ") {
                MarkdownBlockKind::Code
            } else if line.starts_with('>') {
                MarkdownBlockKind::Quote
            } else if line.starts_with("- ") || line.starts_with("* ") {
                MarkdownBlockKind::ListItem
            } else if line.chars().all(|c| c == '-' || c == '─') && display_cols(line) >= 3 {
                MarkdownBlockKind::Rule
            } else {
                MarkdownBlockKind::Paragraph
            };
            let text = match kind {
                MarkdownBlockKind::Heading => line.trim_start_matches('#').trim(),
                MarkdownBlockKind::Quote => line.trim_start_matches('>').trim(),
                MarkdownBlockKind::ListItem => line.trim_start_matches(['-', '*']).trim_start(),
                _ => line,
            };
            MarkdownBlock { kind, text }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_plain_lines_classifies_common_markers() {
        let blocks = project_plain_lines("# Title\n- item\n> quote\n---\nbody");
        assert_eq!(blocks[0].kind, MarkdownBlockKind::Heading);
        assert_eq!(blocks[0].text, "Title");
        assert_eq!(blocks[1].kind, MarkdownBlockKind::ListItem);
        assert_eq!(blocks[2].kind, MarkdownBlockKind::Quote);
        assert_eq!(blocks[3].kind, MarkdownBlockKind::Rule);
        assert_eq!(blocks[4].kind, MarkdownBlockKind::Paragraph);
    }

    #[test]
    fn renders_heading_strong() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let blocks = [MarkdownBlock {
            kind: MarkdownBlockKind::Heading,
            text: "Hello",
        }];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));
        MarkdownView::new(&blocks, &system).render(Rect::new(0, 0, 20, 1), &mut buffer);
        let row: String = (0..20)
            .map(|x| buffer[(x, 0)].symbol().to_owned())
            .collect();
        assert!(row.contains("Hello"));
    }
}
