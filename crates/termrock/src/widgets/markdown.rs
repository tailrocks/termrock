// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lightweight markdown projection for agent streams and docs.
//!
//! TermRock owns display roles for common block kinds. Callers own full
//! markdown parsing if they need CommonMark fidelity — this widget accepts an
//! already-projected block list so parsing stays optional and dependency-free.
//!
//! **Layout reuse.** Heading / paragraph / quote / list rows paint through
//! [`crate::widgets::Heading`] and [`crate::widgets::Paragraph`] so editorial
//! recipes stay single-sourced (no duplicated wrap/prefix logic).

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    style::{DesignSystem, Role, RolePalette},
    text::{display_cols, take_display_cols},
    widgets::{Heading, HeadingLevel, Paragraph, Text, TextSpan},
};

/// Semantic block kinds in a markdown-like stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MarkdownBlockKind {
    /// Ordinary paragraph.
    Paragraph,
    /// Heading level 1–3 (see [`MarkdownBlock::heading_level`]).
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
    /// Heading level when [`MarkdownBlockKind::Heading`] (default H2).
    pub heading_level: HeadingLevel,
}

impl<'a> MarkdownBlock<'a> {
    /// Block with default heading level H2.
    #[must_use]
    pub const fn new(kind: MarkdownBlockKind, text: &'a str) -> Self {
        Self {
            kind,
            text,
            heading_level: HeadingLevel::H2,
        }
    }

    /// Heading block with explicit level.
    #[must_use]
    pub const fn heading(text: &'a str, level: HeadingLevel) -> Self {
        Self {
            kind: MarkdownBlockKind::Heading,
            text,
            heading_level: level,
        }
    }
}

/// Viewport over projected markdown blocks.
#[derive(Debug, Clone, Copy)]
pub struct MarkdownView<'a> {
    blocks: &'a [MarkdownBlock<'a>],
    first: usize,
    system: &'a DesignSystem,
    /// When true, headings use compact prefixes for no-color hierarchy.
    compact_headings: bool,
}

impl<'a> MarkdownView<'a> {
    /// Creates a markdown view starting at the first block.
    #[must_use]
    pub const fn new(blocks: &'a [MarkdownBlock<'a>], system: &'a DesignSystem) -> Self {
        Self {
            blocks,
            first: 0,
            system,
            compact_headings: false,
        }
    }

    /// Sets the first visible block index.
    #[must_use]
    pub const fn first(mut self, first: usize) -> Self {
        self.first = first;
        self
    }

    /// Compact heading recipe (ASCII `#` prefixes).
    #[must_use]
    pub const fn compact_headings(mut self, on: bool) -> Self {
        self.compact_headings = on;
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
            let line = Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            };
            match block.kind {
                MarkdownBlockKind::Paragraph => {
                    let _ = Paragraph::new(block.text, self.system).paint(line, buffer);
                }
                MarkdownBlockKind::Heading => {
                    let mut h = Heading::new(block.text, self.system).level(block.heading_level);
                    if self.compact_headings {
                        h = h.compact();
                    }
                    // Single-row viewport: no rule row (would need multi-row blocks).
                    h = h.rule(false);
                    let _ = h.paint(line, buffer);
                }
                MarkdownBlockKind::Quote => {
                    let _ = Paragraph::quote(block.text, self.system).paint(line, buffer);
                }
                MarkdownBlockKind::ListItem => {
                    let _ = Paragraph::list_item(block.text, self.system).paint(line, buffer);
                }
                MarkdownBlockKind::Code => {
                    buffer.set_style(line, self.system.style(Role::Surface));
                    let _ = Text::spans(
                        [TextSpan::new(block.text).role(Role::Info).code()],
                        self.system,
                    )
                    .truncate()
                    .paint(line, buffer);
                }
                MarkdownBlockKind::Rule => {
                    let unit = self.system.glyphs.rule();
                    let fill = unit.repeat(usize::from(area.width));
                    let clipped = take_display_cols(&fill, usize::from(area.width));
                    buffer.set_stringn(
                        area.x,
                        y,
                        &clipped,
                        usize::from(area.width),
                        self.system.style(Role::Border),
                    );
                }
            }
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
            let (kind, heading_level, text) = if let Some(rest) = line.strip_prefix('#') {
                let mut depth = 1u8;
                let mut body = rest;
                while let Some(r) = body.strip_prefix('#') {
                    depth = depth.saturating_add(1);
                    body = r;
                }
                let body = body.trim_start();
                (
                    MarkdownBlockKind::Heading,
                    HeadingLevel::from_hash_depth(depth),
                    body,
                )
            } else if line.starts_with("```") || line.starts_with("    ") {
                (MarkdownBlockKind::Code, HeadingLevel::H2, line)
            } else if let Some(rest) = line.strip_prefix('>') {
                (
                    MarkdownBlockKind::Quote,
                    HeadingLevel::H2,
                    rest.trim_start(),
                )
            } else if let Some(rest) = line
                .strip_prefix("- ")
                .or_else(|| line.strip_prefix("* "))
            {
                (MarkdownBlockKind::ListItem, HeadingLevel::H2, rest)
            } else if line.chars().all(|c| c == '-' || c == '─') && display_cols(line) >= 3 {
                (MarkdownBlockKind::Rule, HeadingLevel::H2, line)
            } else {
                (MarkdownBlockKind::Paragraph, HeadingLevel::H2, line)
            };
            MarkdownBlock {
                kind,
                text,
                heading_level,
            }
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
        assert_eq!(blocks[0].heading_level, HeadingLevel::H1);
        assert_eq!(blocks[1].kind, MarkdownBlockKind::ListItem);
        assert_eq!(blocks[2].kind, MarkdownBlockKind::Quote);
        assert_eq!(blocks[3].kind, MarkdownBlockKind::Rule);
        assert_eq!(blocks[4].kind, MarkdownBlockKind::Paragraph);
    }

    #[test]
    fn project_heading_levels() {
        let blocks = project_plain_lines("# A\n## B\n### C");
        assert_eq!(blocks[0].heading_level, HeadingLevel::H1);
        assert_eq!(blocks[1].heading_level, HeadingLevel::H2);
        assert_eq!(blocks[2].heading_level, HeadingLevel::H3);
    }

    #[test]
    fn renders_heading_strong() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let blocks = [MarkdownBlock::heading("Hello", HeadingLevel::H1)];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 1));
        MarkdownView::new(&blocks, &system).render(Rect::new(0, 0, 20, 1), &mut buffer);
        let row: String = (0..20)
            .map(|x| buffer[(x, 0)].symbol().to_owned())
            .collect();
        assert!(row.contains("Hello"));
    }

    #[test]
    fn renders_quote_and_list_via_paragraph() {
        let system = DesignSystem::default();
        let blocks = [
            MarkdownBlock::new(MarkdownBlockKind::Quote, "quoted"),
            MarkdownBlock::new(MarkdownBlockKind::ListItem, "item"),
        ];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 24, 2));
        MarkdownView::new(&blocks, &system).render(Rect::new(0, 0, 24, 2), &mut buffer);
        let r0: String = (0..24).map(|x| buffer[(x, 0)].symbol().to_owned()).collect();
        let r1: String = (0..24).map(|x| buffer[(x, 1)].symbol().to_owned()).collect();
        assert!(r0.contains("quoted"));
        assert!(r1.contains("item"));
    }
}
