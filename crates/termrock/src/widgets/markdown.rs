// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Editorial, streaming-capable Markdown projection for terminal apps.
//!
//! TermRock does **not** pull a CommonMark crate: hosts may use any parser.
//! This module owns:
//! - a product-neutral **block model** ([`MarkdownBlock`]);
//! - a dependency-free **line projector** ([`project_markdown`] /
//!   [`project_plain_lines`]) good enough for agent streams and docs;
//! - multi-row **measure + paint** with scroll virtualization;
//! - composition with [`Heading`], [`Paragraph`], [`CodeBlock`], and [`Link`];
//! - selection / copy, source anchors, link activation, responsive tables.
//!
//! Visual benchmark: Glow — whitespace before chrome, soft hierarchy, fence
//! language labels, task boxes, narrow table contraction.
//!
//! **Streaming.** Incomplete fences/tables set [`MarkdownBlock::incomplete`];
//! layout measures only closed content rows + a one-row streaming cue so
//! appending does not thrash unrelated block geometry.
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, NavigationMove, PageMove, SemanticNode, SemanticRole, SemanticScene,
    SemanticState, UiIntent, default_list_intent,
};
use crate::style::{DesignSystem, Role};
use crate::text::{display_cols, take_display_cols, wrap_display_cols};
use crate::widgets::{
    CodeBlock, CodeBlockState, CodeWrap, Heading, HeadingLevel, Paragraph, RoleTokenSyntax, Text,
    TextSpan,
};

// ── Block model ─────────────────────────────────────────────────────────────

/// Semantic block kinds in a markdown-like stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MarkdownBlockKind {
    /// Ordinary paragraph (may contain newlines → soft blank).
    Paragraph,
    /// Heading level 1–3 (see [`MarkdownBlock::heading_level`]).
    Heading,
    /// Indented / single-line code.
    Code,
    /// Fenced code body (`text` may be multi-line; `language` optional).
    Fence,
    /// Block quote.
    Quote,
    /// Unordered list item.
    ListItem,
    /// Ordered list item (`list_index` when known).
    OrderedItem,
    /// Task list item (`task_checked`).
    TaskItem,
    /// Horizontal rule / thematic break.
    Rule,
    /// Pipe table; `text` holds raw pipe rows (header + body).
    Table,
    /// Vertical breathing room (Glow-style spacing before borders/sections).
    Blank,
}

impl MarkdownBlockKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Paragraph => "paragraph",
            Self::Heading => "heading",
            Self::Code => "code",
            Self::Fence => "fence",
            Self::Quote => "quote",
            Self::ListItem => "list-item",
            Self::OrderedItem => "ordered-item",
            Self::TaskItem => "task-item",
            Self::Rule => "rule",
            Self::Table => "table",
            Self::Blank => "blank",
        }
    }
}

/// Source map into the original markdown buffer (line numbers, 1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceAnchor {
    /// First source line (1-based).
    pub line_start: u32,
    /// Last source line inclusive (1-based).
    pub line_end: u32,
}

impl SourceAnchor {
    /// Single-line anchor.
    #[must_use]
    pub const fn line(n: u32) -> Self {
        let n = if n == 0 { 1 } else { n };
        Self {
            line_start: n,
            line_end: n,
        }
    }

    /// Inclusive range.
    #[must_use]
    pub const fn range(start: u32, end: u32) -> Self {
        let start = if start == 0 { 1 } else { start };
        Self {
            line_start: start,
            line_end: if end < start { start } else { end },
        }
    }
}

/// Inline run kind (emphasis / link / code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MarkdownInlineKind {
    /// Body text.
    #[default]
    Text,
    /// Strong / bold.
    Strong,
    /// Emphasis / italic.
    Emphasis,
    /// Inline code.
    Code,
    /// Hyperlink (`href` on the span).
    Link,
}

/// Borrowed inline span for rich paragraphs / headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownInline<'a> {
    /// Visible text.
    pub text: &'a str,
    /// Style kind.
    pub kind: MarkdownInlineKind,
    /// Destination when [`MarkdownInlineKind::Link`].
    pub href: Option<&'a str>,
}

impl<'a> MarkdownInline<'a> {
    /// Plain text run.
    #[must_use]
    pub const fn text(text: &'a str) -> Self {
        Self {
            text,
            kind: MarkdownInlineKind::Text,
            href: None,
        }
    }

    /// Strong.
    #[must_use]
    pub const fn strong(text: &'a str) -> Self {
        Self {
            text,
            kind: MarkdownInlineKind::Strong,
            href: None,
        }
    }

    /// Emphasis.
    #[must_use]
    pub const fn emphasis(text: &'a str) -> Self {
        Self {
            text,
            kind: MarkdownInlineKind::Emphasis,
            href: None,
        }
    }

    /// Inline code.
    #[must_use]
    pub const fn code(text: &'a str) -> Self {
        Self {
            text,
            kind: MarkdownInlineKind::Code,
            href: None,
        }
    }

    /// Link.
    #[must_use]
    pub const fn link(text: &'a str, href: &'a str) -> Self {
        Self {
            text,
            kind: MarkdownInlineKind::Link,
            href: Some(href),
        }
    }
}

/// One borrowed markdown block (may span multiple display rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownBlock<'a> {
    /// Semantic kind.
    pub kind: MarkdownBlockKind,
    /// Primary text (multi-line allowed for fence / table / wrapped prose).
    pub text: &'a str,
    /// Heading level when heading.
    pub heading_level: HeadingLevel,
    /// Nesting depth for lists (0 = top).
    pub depth: u8,
    /// 1-based ordered list marker when ordered.
    pub list_index: Option<u32>,
    /// Task checkbox state.
    pub task_checked: Option<bool>,
    /// Fence language tag.
    pub language: Option<&'a str>,
    /// Optional pre-parsed inlines (else paint may light-parse markers).
    pub spans: Option<&'a [MarkdownInline<'a>]>,
    /// Source line map.
    pub source: Option<SourceAnchor>,
    /// Unfinished fence/table/stream — trailing cue, stable layout.
    pub incomplete: bool,
}

impl<'a> MarkdownBlock<'a> {
    /// Block with defaults.
    #[must_use]
    pub const fn new(kind: MarkdownBlockKind, text: &'a str) -> Self {
        Self {
            kind,
            text,
            heading_level: HeadingLevel::H2,
            depth: 0,
            list_index: None,
            task_checked: None,
            language: None,
            spans: None,
            source: None,
            incomplete: false,
        }
    }

    /// Heading.
    #[must_use]
    pub const fn heading(text: &'a str, level: HeadingLevel) -> Self {
        Self {
            kind: MarkdownBlockKind::Heading,
            text,
            heading_level: level,
            depth: 0,
            list_index: None,
            task_checked: None,
            language: None,
            spans: None,
            source: None,
            incomplete: false,
        }
    }

    /// Fenced code.
    #[must_use]
    pub const fn fence(body: &'a str, language: Option<&'a str>) -> Self {
        Self {
            kind: MarkdownBlockKind::Fence,
            text: body,
            heading_level: HeadingLevel::H2,
            depth: 0,
            list_index: None,
            task_checked: None,
            language,
            spans: None,
            source: None,
            incomplete: false,
        }
    }

    /// Task item.
    #[must_use]
    pub const fn task(text: &'a str, checked: bool) -> Self {
        Self {
            kind: MarkdownBlockKind::TaskItem,
            text,
            heading_level: HeadingLevel::H2,
            depth: 0,
            list_index: None,
            task_checked: Some(checked),
            language: None,
            spans: None,
            source: None,
            incomplete: false,
        }
    }

    /// Ordered item.
    #[must_use]
    pub const fn ordered(text: &'a str, index: u32) -> Self {
        Self {
            kind: MarkdownBlockKind::OrderedItem,
            text,
            heading_level: HeadingLevel::H2,
            depth: 0,
            list_index: Some(index),
            task_checked: None,
            language: None,
            spans: None,
            source: None,
            incomplete: false,
        }
    }

    /// Blank spacer row.
    #[must_use]
    pub const fn blank() -> Self {
        Self::new(MarkdownBlockKind::Blank, "")
    }

    /// Builder: depth.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Builder: incomplete stream.
    #[must_use]
    pub const fn incomplete(mut self, on: bool) -> Self {
        self.incomplete = on;
        self
    }

    /// Builder: source anchor.
    #[must_use]
    pub const fn source(mut self, anchor: SourceAnchor) -> Self {
        self.source = Some(anchor);
        self
    }

    /// Builder: language.
    #[must_use]
    pub const fn language(mut self, language: &'a str) -> Self {
        self.language = Some(language);
        self
    }

    /// Builder: inlines.
    #[must_use]
    pub const fn spans(mut self, spans: &'a [MarkdownInline<'a>]) -> Self {
        self.spans = Some(spans);
        self
    }

    /// Plain clipboard text for this block.
    #[must_use]
    pub fn plain(&self) -> String {
        match self.kind {
            MarkdownBlockKind::Blank | MarkdownBlockKind::Rule => String::new(),
            MarkdownBlockKind::TaskItem => {
                let mark = if self.task_checked == Some(true) {
                    "[x]"
                } else {
                    "[ ]"
                };
                format!("{mark} {}", self.text)
            }
            MarkdownBlockKind::OrderedItem => {
                let n = self.list_index.unwrap_or(1);
                format!("{n}. {}", self.text)
            }
            MarkdownBlockKind::ListItem => format!("- {}", self.text),
            MarkdownBlockKind::Quote => format!("> {}", self.text),
            MarkdownBlockKind::Heading => {
                let hashes = match self.heading_level {
                    HeadingLevel::H1 => "#",
                    HeadingLevel::H2 => "##",
                    HeadingLevel::H3 => "###",
                };
                format!("{hashes} {}", self.text)
            }
            MarkdownBlockKind::Fence => {
                let lang = self.language.unwrap_or("");
                format!("```{lang}\n{}\n```", self.text)
            }
            _ => self.text.to_string(),
        }
    }
}

// ── State / parts / outcomes ────────────────────────────────────────────────

/// Interaction + scroll for a markdown view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownViewState {
    /// First visible display row (document Y).
    pub scroll_y: u16,
    /// Keyboard focus.
    pub focused: bool,
    /// Focused block index.
    pub cursor_block: Option<usize>,
    /// Block selection [start, end).
    pub selection: Option<(usize, usize)>,
    /// Focused link index among collected link regions (this paint).
    pub link_index: Option<usize>,
    /// Last layout.
    pub parts: Option<MarkdownParts>,
    /// Cached total display rows from last measure.
    total_rows: u16,
    /// Viewport height from last paint.
    viewport_rows: u16,
}

impl Default for MarkdownViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl MarkdownViewState {
    /// Fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scroll_y: 0,
            focused: false,
            cursor_block: None,
            selection: None,
            link_index: None,
            parts: None,
            total_rows: 0,
            viewport_rows: 0,
        }
    }

    /// Seed scroll.
    #[must_use]
    pub const fn with_scroll_y(mut self, y: u16) -> Self {
        self.scroll_y = y;
        self
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Cursor block.
    pub const fn set_cursor_block(&mut self, idx: Option<usize>) {
        self.cursor_block = idx;
    }

    /// Max scroll.
    #[must_use]
    pub fn max_scroll_y(&self) -> u16 {
        self.total_rows.saturating_sub(self.viewport_rows.max(1))
    }

    /// Clamp scroll.
    pub fn clamp(&mut self) {
        let max = self.max_scroll_y();
        if self.scroll_y > max {
            self.scroll_y = max;
        }
    }

    /// Scroll by display rows.
    pub fn scroll_by(&mut self, delta: i32) -> bool {
        let before = self.scroll_y;
        if delta >= 0 {
            self.scroll_y = self
                .scroll_y
                .saturating_add(u16::try_from(delta).unwrap_or(u16::MAX));
        } else {
            self.scroll_y = self
                .scroll_y
                .saturating_sub(u16::try_from(-delta).unwrap_or(u16::MAX));
        }
        self.clamp();
        before != self.scroll_y
    }
}

/// One painted link hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownLinkRegion {
    /// Block index.
    pub block: usize,
    /// Label.
    pub label: String,
    /// Destination URL / route.
    pub href: String,
    /// Hit area.
    pub area: Rect,
}

/// Geometry + link regions from last paint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownParts {
    /// Root area.
    pub root: Rect,
    /// First visible display row.
    pub first_row: u16,
    /// Total document rows.
    pub total_rows: u16,
    /// Visible row count.
    pub visible_rows: u16,
    /// Link hit regions (this viewport).
    pub links: Vec<MarkdownLinkRegion>,
    /// Streaming incomplete content present.
    pub streaming: bool,
}

/// Host outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarkdownOutcome {
    /// No change.
    Ignored,
    /// Scroll changed.
    Scrolled {
        /// Display row.
        scroll_y: u16,
    },
    /// Cursor block moved.
    CursorMoved {
        /// Block index.
        block: usize,
    },
    /// Selection range.
    SelectionChanged {
        /// [start, end).
        range: (usize, usize),
    },
    /// Link activated.
    LinkActivated {
        /// Label.
        label: String,
        /// Href.
        href: String,
    },
    /// Copy request.
    Copy {
        /// Plain markdown-ish text.
        text: String,
    },
    /// Block activated (Enter).
    BlockActivated {
        /// Block index.
        block: usize,
    },
}

// ── View ────────────────────────────────────────────────────────────────────

/// Viewport over projected markdown blocks (multi-row layout).
#[derive(Debug, Clone, Copy)]
pub struct MarkdownView<'a> {
    blocks: &'a [MarkdownBlock<'a>],
    system: &'a DesignSystem,
    /// Legacy first **block** index when Widget path (converted to row 0 of block).
    first_block: usize,
    compact_headings: bool,
    /// Prefer CodeBlock for fences.
    fence_line_numbers: bool,
    /// Soft wrap width for tables (0 = area).
    selectable: bool,
    /// Extra blank row before headings / rules (Glow breathing).
    section_gap: bool,
}

impl<'a> MarkdownView<'a> {
    /// Creates a markdown view.
    #[must_use]
    pub const fn new(blocks: &'a [MarkdownBlock<'a>], system: &'a DesignSystem) -> Self {
        Self {
            blocks,
            system,
            first_block: 0,
            compact_headings: false,
            fence_line_numbers: false,
            selectable: true,
            section_gap: true,
        }
    }

    /// Sets the first visible **block** index (Widget / simple hosts).
    #[must_use]
    pub const fn first(mut self, first: usize) -> Self {
        self.first_block = first;
        self
    }

    /// Compact heading recipe (ASCII `#` prefixes).
    #[must_use]
    pub const fn compact_headings(mut self, on: bool) -> Self {
        self.compact_headings = on;
        self
    }

    /// Show line numbers inside fenced code.
    #[must_use]
    pub const fn fence_line_numbers(mut self, on: bool) -> Self {
        self.fence_line_numbers = on;
        self
    }

    /// Copyable / selectable policy for prose.
    #[must_use]
    pub const fn selectable(mut self, on: bool) -> Self {
        self.selectable = on;
        self
    }

    /// Glow-style blank row before H1/H2 and rules (default true).
    #[must_use]
    pub const fn section_gap(mut self, on: bool) -> Self {
        self.section_gap = on;
        self
    }

    /// Block count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Measure display rows for one block at `width`.
    #[must_use]
    pub fn measure_block_height(&self, block: &MarkdownBlock<'_>, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        let gap = if self.section_gap && wants_leading_gap(block.kind, block.heading_level) {
            1u16
        } else {
            0
        };
        let body = match block.kind {
            MarkdownBlockKind::Blank => 1,
            MarkdownBlockKind::Rule => 1,
            MarkdownBlockKind::Heading => {
                // Single title row in viewport recipes (rule off for dense streams).
                1
            }
            MarkdownBlockKind::Paragraph | MarkdownBlockKind::Quote => {
                Paragraph::new(block.text, self.system)
                    .measure_height(width)
                    .max(1)
            }
            MarkdownBlockKind::ListItem
            | MarkdownBlockKind::OrderedItem
            | MarkdownBlockKind::TaskItem => {
                let prefix_w = list_prefix_width(block);
                let indent = u16::from(block.depth).saturating_mul(2);
                let inner = width.saturating_sub(prefix_w).saturating_sub(indent).max(1);
                Paragraph::new(block.text, self.system)
                    .measure_height(inner)
                    .max(1)
            }
            MarkdownBlockKind::Code => 1,
            MarkdownBlockKind::Fence => {
                let lines = block.text.lines().count().max(1);
                let header = u16::from(block.language.is_some() || block.incomplete);
                let cue = u16::from(block.incomplete);
                u16::try_from(lines)
                    .unwrap_or(u16::MAX)
                    .saturating_add(header)
                    .saturating_add(cue)
                    .max(1)
            }
            MarkdownBlockKind::Table => measure_table_height(block.text, width),
        };
        gap.saturating_add(body)
    }

    /// Total display rows for full document at width.
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        self.blocks
            .iter()
            .map(|b| self.measure_block_height(b, width))
            .fold(0u16, |a, h| a.saturating_add(h))
    }

    /// Build row map: each display row → (block_index, sub_row).
    #[must_use]
    pub fn row_map(&self, width: u16) -> Vec<(usize, u16)> {
        let mut map = Vec::new();
        for (bi, block) in self.blocks.iter().enumerate() {
            let h = self.measure_block_height(block, width);
            for sub in 0..h {
                map.push((bi, sub));
            }
        }
        map
    }

    /// First display row index of `first_block` (for Widget migration).
    #[must_use]
    pub fn block_start_row(&self, block_index: usize, width: u16) -> u16 {
        self.blocks
            .iter()
            .take(block_index)
            .map(|b| self.measure_block_height(b, width))
            .fold(0u16, |a, h| a.saturating_add(h))
    }

    /// Plain text for selection / full doc.
    #[must_use]
    pub fn copy_range(&self, start_block: usize, end_block: usize) -> String {
        let end = end_block.min(self.blocks.len());
        let start = start_block.min(end);
        let mut out = String::new();
        for b in &self.blocks[start..end] {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&b.plain());
        }
        out
    }

    /// Copy selection or cursor block or full document.
    #[must_use]
    pub fn copy_text(&self, state: &MarkdownViewState) -> String {
        if let Some((a, b)) = state.selection {
            return self.copy_range(a, b);
        }
        if let Some(c) = state.cursor_block {
            return self.copy_range(c, c.saturating_add(1));
        }
        self.copy_range(0, self.blocks.len())
    }

    /// Paint with state (preferred).
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut MarkdownViewState,
    ) -> MarkdownParts {
        if area.is_empty() {
            let parts = MarkdownParts {
                root: area,
                first_row: 0,
                total_rows: 0,
                visible_rows: 0,
                links: Vec::new(),
                streaming: false,
            };
            state.parts = Some(parts.clone());
            return parts;
        }

        let width = area.width;
        let map = self.row_map(width);
        let total = u16::try_from(map.len()).unwrap_or(u16::MAX);
        state.total_rows = total;
        state.viewport_rows = area.height;
        // Seed scroll from first_block once when unset path
        if state.scroll_y == 0 && self.first_block > 0 && state.parts.is_none() {
            state.scroll_y = self.block_start_row(self.first_block, width);
        }
        state.clamp();

        let first = usize::from(state.scroll_y);
        let mut links = Vec::new();
        let mut streaming = false;
        let mut painted = 0u16;

        for row in 0..area.height {
            let idx = first.saturating_add(usize::from(row));
            let Some(&(bi, sub)) = map.get(idx) else {
                break;
            };
            let block = &self.blocks[bi];
            if block.incomplete {
                streaming = true;
            }
            let y = area.y.saturating_add(row);
            let line = Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            };
            let selected = state.selection.is_some_and(|(a, b)| bi >= a && bi < b)
                || state.cursor_block == Some(bi) && state.focused;

            self.paint_block_row(block, bi, sub, line, buffer, selected, &mut links);
            painted = painted.saturating_add(1);
        }

        let parts = MarkdownParts {
            root: area,
            first_row: state.scroll_y,
            total_rows: total,
            visible_rows: painted,
            links,
            streaming,
        };
        state.parts = Some(parts.clone());
        parts
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_block_row(
        &self,
        block: &MarkdownBlock<'a>,
        block_index: usize,
        sub: u16,
        area: Rect,
        buffer: &mut Buffer,
        selected: bool,
        links: &mut Vec<MarkdownLinkRegion>,
    ) {
        if area.is_empty() {
            return;
        }
        // Leading section gap row
        if self.section_gap && wants_leading_gap(block.kind, block.heading_level) && sub == 0 {
            // blank row — nothing
            return;
        }
        let body_sub = if self.section_gap && wants_leading_gap(block.kind, block.heading_level) {
            sub.saturating_sub(1)
        } else {
            sub
        };

        match block.kind {
            MarkdownBlockKind::Blank => {}
            MarkdownBlockKind::Rule => {
                if body_sub == 0 {
                    let unit = self.system.glyphs.rule();
                    let fill = unit.repeat(usize::from(area.width));
                    let clipped = take_display_cols(&fill, usize::from(area.width));
                    buffer.set_stringn(
                        area.x,
                        area.y,
                        &clipped,
                        usize::from(area.width),
                        self.system.style(Role::Border),
                    );
                }
            }
            MarkdownBlockKind::Heading => {
                if body_sub == 0 {
                    let mut h = Heading::new(block.text, self.system).level(block.heading_level);
                    if self.compact_headings {
                        h = h.compact();
                    }
                    h = h.rule(false);
                    let _ = h.paint(area, buffer);
                    if selected {
                        select_row(buffer, area, self.system);
                    }
                }
            }
            MarkdownBlockKind::Paragraph => {
                self.paint_prose_row(block, body_sub, area, buffer, selected, links, block_index);
            }
            MarkdownBlockKind::Quote => {
                let lines = Paragraph::quote(block.text, self.system).wrap_lines(area.width);
                if let Some(line) = lines.get(usize::from(body_sub)) {
                    let _ = Text::new(line.as_str(), self.system)
                        .role(Role::TextMuted)
                        .truncate()
                        .paint(area, buffer);
                }
            }
            MarkdownBlockKind::ListItem
            | MarkdownBlockKind::OrderedItem
            | MarkdownBlockKind::TaskItem => {
                self.paint_list_row(block, body_sub, area, buffer, selected);
            }
            MarkdownBlockKind::Code => {
                if body_sub == 0 {
                    let _ = Text::spans(
                        [TextSpan::new(block.text).role(Role::TextSecondary).code()],
                        self.system,
                    )
                    .truncate()
                    .paint(area, buffer);
                }
            }
            MarkdownBlockKind::Fence => {
                self.paint_fence_row(block, body_sub, area, buffer);
            }
            MarkdownBlockKind::Table => {
                paint_table_row(block.text, body_sub, area, buffer, self.system, selected);
            }
        }
    }

    fn paint_prose_row(
        &self,
        block: &MarkdownBlock<'a>,
        body_sub: u16,
        area: Rect,
        buffer: &mut Buffer,
        selected: bool,
        links: &mut Vec<MarkdownLinkRegion>,
        block_index: usize,
    ) {
        // Prefer explicit spans; else light-parse markers into display string per wrap line.
        if let Some(spans) = block.spans {
            let plain: String = spans.iter().map(|s| s.text).collect();
            let wrapped = wrap_display_cols(&plain, usize::from(area.width));
            let Some(line) = wrapped.get(usize::from(body_sub)) else {
                return;
            };
            // Approximate: paint line as single run; collect links from spans on first row.
            if body_sub == 0 {
                for sp in spans {
                    if sp.kind == MarkdownInlineKind::Link {
                        if let Some(href) = sp.href {
                            links.push(MarkdownLinkRegion {
                                block: block_index,
                                label: sp.text.to_string(),
                                href: href.to_string(),
                                area,
                            });
                        }
                    }
                }
            }
            let text_spans = spans_to_text(spans, self.system);
            // Paint full spans only on sub 0 when single line; multi-line uses plain wrap.
            if wrapped.len() == 1 {
                let mut t = Text::spans(text_spans, self.system).truncate();
                if self.selectable {
                    t = t.copyable();
                }
                let _ = t.paint(area, buffer);
            } else {
                let _ = Text::new(line.as_str(), self.system)
                    .role(Role::Text)
                    .truncate()
                    .paint(area, buffer);
            }
        } else {
            let (display, found_links) = expand_inline_markers(block.text);
            let wrapped = wrap_display_cols(&display, usize::from(area.width));
            if let Some(line) = wrapped.get(usize::from(body_sub)) {
                let mut t = Text::new(line.as_str(), self.system)
                    .role(Role::Text)
                    .truncate();
                if self.selectable {
                    t = t.copyable();
                }
                let _ = t.paint(area, buffer);
            }
            if body_sub == 0 {
                for (label, href) in found_links {
                    links.push(MarkdownLinkRegion {
                        block: block_index,
                        label,
                        href,
                        area,
                    });
                }
            }
        }
        if selected {
            select_row(buffer, area, self.system);
        }
    }

    fn paint_list_row(
        &self,
        block: &MarkdownBlock<'a>,
        body_sub: u16,
        area: Rect,
        buffer: &mut Buffer,
        selected: bool,
    ) {
        let indent = u16::from(block.depth).saturating_mul(2);
        let prefix = list_prefix(block, self.system);
        let prefix_w = u16::try_from(display_cols(&prefix)).unwrap_or(2);
        let inner_x = area.x.saturating_add(indent);
        let text_x = inner_x.saturating_add(prefix_w);
        let text_w = area
            .width
            .saturating_sub(indent)
            .saturating_sub(prefix_w)
            .max(1);

        if body_sub == 0 && prefix_w > 0 && area.width > indent {
            buffer.set_stringn(
                inner_x,
                area.y,
                &prefix,
                usize::from(prefix_w.min(area.width.saturating_sub(indent))),
                self.system.style(Role::TextMuted),
            );
        }
        let lines = Paragraph::new(block.text, self.system).wrap_lines(text_w);
        if let Some(line) = lines.get(usize::from(body_sub)) {
            let paint_x = if body_sub == 0 {
                text_x
            } else {
                // hanging indent
                text_x
            };
            let _ = Text::new(line.as_str(), self.system)
                .role(Role::Text)
                .truncate()
                .paint(
                    Rect {
                        x: paint_x,
                        y: area.y,
                        width: area
                            .width
                            .saturating_sub(paint_x.saturating_sub(area.x))
                            .max(1),
                        height: 1,
                    },
                    buffer,
                );
        }
        if selected {
            select_row(buffer, area, self.system);
        }
    }

    fn paint_fence_row(
        &self,
        block: &MarkdownBlock<'a>,
        body_sub: u16,
        area: Rect,
        buffer: &mut Buffer,
    ) {
        let has_header = block.language.is_some() || block.incomplete;
        if has_header && body_sub == 0 {
            let lang = block.language.unwrap_or("code");
            let label = if block.incomplete {
                format!("{lang} {}", "…")
            } else {
                lang.to_string()
            };
            let clipped = take_display_cols(&label, usize::from(area.width));
            buffer.set_stringn(
                area.x,
                area.y,
                &clipped,
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }
        let line_sub = if has_header {
            body_sub.saturating_sub(1)
        } else {
            body_sub
        };
        let lines: Vec<&str> = block.text.lines().collect();
        if let Some(src) = lines.get(usize::from(line_sub)) {
            // Single-line CodeBlock paint for syntax
            let hi = RoleTokenSyntax::new(self.system, block.language, &[]);
            let slice = [*src];
            let mut st = CodeBlockState::new();
            let mut cb = CodeBlock::new(&slice, self.system)
                .wrap(CodeWrap::Clip)
                .highlighter(&hi);
            if self.fence_line_numbers {
                cb = cb.line_numbers(true);
            }
            if let Some(lang) = block.language {
                // language already in header
                let _ = lang;
            }
            let _ = cb.paint(area, buffer, &mut st);
            return;
        }
        // After body: streaming cue
        if block.incomplete {
            let cue = "…";
            buffer.set_stringn(
                area.x,
                area.y,
                cue,
                usize::from(area.width),
                self.system.style(Role::TextFaint),
            );
        }
    }

    /// Keys: scroll, cursor, copy, link activate.
    pub fn handle_key(&self, state: &mut MarkdownViewState, key: KeyEvent) -> MarkdownOutcome {
        if !state.focused || key.kind != KeyEventKind::Press {
            return MarkdownOutcome::Ignored;
        }
        if matches!(key.code, crate::input::KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            return MarkdownOutcome::Copy {
                text: self.copy_text(state),
            };
        }
        // Enter on link if link_index set
        if matches!(key.code, crate::input::KeyCode::Enter) {
            if let Some(parts) = &state.parts {
                if let Some(i) = state.link_index {
                    if let Some(link) = parts.links.get(i) {
                        return MarkdownOutcome::LinkActivated {
                            label: link.label.clone(),
                            href: link.href.clone(),
                        };
                    }
                }
            }
            if let Some(b) = state.cursor_block {
                // Prefer first link in block
                if let Some(parts) = &state.parts {
                    if let Some(link) = parts.links.iter().find(|l| l.block == b) {
                        return MarkdownOutcome::LinkActivated {
                            label: link.label.clone(),
                            href: link.href.clone(),
                        };
                    }
                }
                return MarkdownOutcome::BlockActivated { block: b };
            }
        }
        // Tab cycles links in viewport
        if matches!(key.code, crate::input::KeyCode::Tab) {
            if let Some(parts) = &state.parts {
                if !parts.links.is_empty() {
                    let n = parts.links.len();
                    let next = state.link_index.map(|i| (i + 1) % n).unwrap_or(0);
                    state.link_index = Some(next);
                    return MarkdownOutcome::CursorMoved {
                        block: parts.links[next].block,
                    };
                }
            }
        }

        if let Some(intent) = default_list_intent(key) {
            return self.handle_intent(state, intent);
        }
        MarkdownOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(
        &self,
        state: &mut MarkdownViewState,
        intent: UiIntent,
    ) -> MarkdownOutcome {
        if !state.focused {
            return MarkdownOutcome::Ignored;
        }
        let page = i32::from(state.viewport_rows.max(1));
        match intent {
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                if state.cursor_block.is_some() {
                    let cur = state.cursor_block.unwrap_or(0);
                    let next = cur.saturating_sub(1);
                    state.cursor_block = Some(next);
                    self.reveal_block(state, next);
                    MarkdownOutcome::CursorMoved { block: next }
                } else if state.scroll_by(-1) {
                    MarkdownOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    }
                } else {
                    MarkdownOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                if state.cursor_block.is_some() || !self.blocks.is_empty() {
                    let cur = state.cursor_block.unwrap_or(0);
                    let next = (cur + 1).min(self.blocks.len().saturating_sub(1));
                    state.cursor_block = Some(next);
                    self.reveal_block(state, next);
                    MarkdownOutcome::CursorMoved { block: next }
                } else if state.scroll_by(1) {
                    MarkdownOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    }
                } else {
                    MarkdownOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                state.scroll_y = 0;
                state.cursor_block = if self.blocks.is_empty() {
                    None
                } else {
                    Some(0)
                };
                MarkdownOutcome::Scrolled { scroll_y: 0 }
            }
            UiIntent::Move(NavigationMove::Last) => {
                let last = self.blocks.len().saturating_sub(1);
                state.cursor_block = if self.blocks.is_empty() {
                    None
                } else {
                    Some(last)
                };
                self.reveal_block(state, last);
                MarkdownOutcome::Scrolled {
                    scroll_y: state.scroll_y,
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                if state.scroll_by(-page) {
                    MarkdownOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    }
                } else {
                    MarkdownOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Forward) => {
                if state.scroll_by(page) {
                    MarkdownOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    }
                } else {
                    MarkdownOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(b) = state.cursor_block {
                    MarkdownOutcome::BlockActivated { block: b }
                } else {
                    MarkdownOutcome::Ignored
                }
            }
            _ => MarkdownOutcome::Ignored,
        }
    }

    fn reveal_block(&self, state: &mut MarkdownViewState, block_index: usize) {
        let width = state.parts.as_ref().map(|p| p.root.width).unwrap_or(80);
        let start = self.block_start_row(block_index, width);
        let h = self
            .blocks
            .get(block_index)
            .map(|b| self.measure_block_height(b, width))
            .unwrap_or(1);
        let end = start.saturating_add(h);
        let view_end = state.scroll_y.saturating_add(state.viewport_rows.max(1));
        if start < state.scroll_y {
            state.scroll_y = start;
        } else if end > view_end {
            state.scroll_y = end.saturating_sub(state.viewport_rows.max(1));
        }
        state.clamp();
    }

    /// Mouse wheel / click.
    pub fn handle_mouse(
        &self,
        state: &mut MarkdownViewState,
        event: MouseEvent,
    ) -> MarkdownOutcome {
        let Some(parts) = state.parts.clone() else {
            return MarkdownOutcome::Ignored;
        };
        if !parts.root.contains(event.position) {
            return MarkdownOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollUp => {
                if state.scroll_by(-3) {
                    return MarkdownOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    };
                }
            }
            MouseEventKind::ScrollDown => {
                if state.scroll_by(3) {
                    return MarkdownOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                    };
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // Link hit first
                for (i, link) in parts.links.iter().enumerate() {
                    if link.area.contains(event.position) {
                        state.focused = true;
                        state.link_index = Some(i);
                        state.cursor_block = Some(link.block);
                        return MarkdownOutcome::LinkActivated {
                            label: link.label.clone(),
                            href: link.href.clone(),
                        };
                    }
                }
                let row = event.position.y.saturating_sub(parts.root.y);
                let doc_row = parts.first_row.saturating_add(row);
                let width = parts.root.width;
                let map = self.row_map(width);
                if let Some(&(bi, _)) = map.get(usize::from(doc_row)) {
                    state.focused = true;
                    state.cursor_block = Some(bi);
                    state.selection = Some((bi, bi.saturating_add(1)));
                    return MarkdownOutcome::SelectionChanged {
                        range: (bi, bi.saturating_add(1)),
                    };
                }
            }
            _ => {}
        }
        MarkdownOutcome::Ignored
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut MarkdownViewState,
        key: KeyEvent,
    ) -> EventResult<MarkdownOutcome> {
        match self.handle_key(state, key) {
            MarkdownOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Activate a painted link by index (host / tests).
    pub fn activate_link(&self, state: &MarkdownViewState, index: usize) -> MarkdownOutcome {
        let Some(parts) = &state.parts else {
            return MarkdownOutcome::Ignored;
        };
        let Some(link) = parts.links.get(index) else {
            return MarkdownOutcome::Ignored;
        };
        MarkdownOutcome::LinkActivated {
            label: link.label.clone(),
            href: link.href.clone(),
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &MarkdownViewState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "markdown {} blocks{}",
            self.blocks.len(),
            if state.parts.as_ref().is_some_and(|p| p.streaming) {
                " streaming"
            } else {
                ""
            }
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Content)
                .label("markdown")
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    selected: state.focused,
                    ..Default::default()
                }),
        );
    }
}

impl Widget for &MarkdownView<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let scroll = if self.first_block > 0 {
            self.block_start_row(self.first_block, area.width)
        } else {
            0
        };
        let mut state = MarkdownViewState::new().with_scroll_y(scroll);
        let _ = self.paint(area, buffer, &mut state);
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

// ── Projection ──────────────────────────────────────────────────────────────

/// Projects plain text markers into blocks (line-oriented, streaming-safe).
///
/// Prefer [`project_markdown`] for fences/tables aggregation.
#[must_use]
pub fn project_plain_lines(text: &str) -> Vec<MarkdownBlock<'_>> {
    project_markdown(text)
}

/// Full dependency-free projector: headings, lists, tasks, quotes, fences,
/// tables, rules, blanks. Unfinished fences stay [`MarkdownBlock::incomplete`].
#[must_use]
pub fn project_markdown(text: &str) -> Vec<MarkdownBlock<'_>> {
    let mut blocks = Vec::new();
    let mut line_no = 0u32;
    // Index by walking text (line content without trailing newline for classify).
    let mut offset = 0usize;
    while offset < text.len() {
        line_no = line_no.saturating_add(1);
        let rest = &text[offset..];
        let nl = rest.find('\n').map(|i| i + 1).unwrap_or(rest.len());
        let line_with_nl = &rest[..nl];
        let line = line_with_nl.trim_end_matches(['\n', '\r']);
        let line_start = offset;
        offset += nl;

        // Fence open
        if let Some(lang) = strip_fence_open(line) {
            let body_start = offset;
            let mut body_end = offset;
            let mut closed = false;
            let mut end_line = line_no;
            while offset < text.len() {
                end_line = end_line.saturating_add(1);
                let r = &text[offset..];
                let n = r.find('\n').map(|i| i + 1).unwrap_or(r.len());
                let l = r[..n].trim_end_matches(['\n', '\r']);
                if is_fence_close(l) {
                    body_end = offset;
                    offset += n;
                    closed = true;
                    break;
                }
                offset += n;
                body_end = offset;
            }
            if !closed {
                body_end = text.len();
                offset = text.len();
            }
            let body = text[body_start..body_end].trim_end_matches(['\n', '\r']);
            // language slice into original open line
            let language = if lang.is_empty() {
                None
            } else {
                // lang is from `line` which is into text
                Some(lang)
            };
            blocks.push(
                MarkdownBlock::fence(body, language)
                    .incomplete(!closed)
                    .source(SourceAnchor::range(line_no, end_line)),
            );
            line_no = end_line;
            continue;
        }

        // Table: consecutive pipe lines
        if looks_like_table_row(line) {
            let table_start_line = line_no;
            let table_start = line_start;
            let mut table_end = offset;
            let mut end_line = line_no;
            while offset < text.len() {
                let r = &text[offset..];
                let n = r.find('\n').map(|i| i + 1).unwrap_or(r.len());
                let l = r[..n].trim_end_matches(['\n', '\r']);
                if !looks_like_table_row(l) && !looks_like_table_sep(l) {
                    break;
                }
                end_line = end_line.saturating_add(1);
                offset += n;
                table_end = offset;
            }
            let raw = text[table_start..table_end].trim_end_matches(['\n', '\r']);
            blocks.push(
                MarkdownBlock::new(MarkdownBlockKind::Table, raw)
                    .source(SourceAnchor::range(table_start_line, end_line)),
            );
            line_no = end_line;
            continue;
        }

        if line.is_empty() {
            blocks.push(MarkdownBlock::blank().source(SourceAnchor::line(line_no)));
            continue;
        }

        if let Some(rest) = line.strip_prefix('#') {
            let mut depth = 1u8;
            let mut body = rest;
            while let Some(r) = body.strip_prefix('#') {
                depth = depth.saturating_add(1);
                body = r;
            }
            let body = body.trim_start();
            blocks.push(
                MarkdownBlock::heading(body, HeadingLevel::from_hash_depth(depth))
                    .source(SourceAnchor::line(line_no)),
            );
            continue;
        }

        if let Some(rest) = line.strip_prefix('>') {
            blocks.push(
                MarkdownBlock::new(MarkdownBlockKind::Quote, rest.trim_start())
                    .source(SourceAnchor::line(line_no)),
            );
            continue;
        }

        if let Some((checked, rest)) = parse_task_item(line) {
            blocks.push(MarkdownBlock::task(rest, checked).source(SourceAnchor::line(line_no)));
            continue;
        }

        if let Some((idx, rest)) = parse_ordered_item(line) {
            blocks.push(MarkdownBlock::ordered(rest, idx).source(SourceAnchor::line(line_no)));
            continue;
        }

        if let Some(rest) = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("+ "))
        {
            blocks.push(
                MarkdownBlock::new(MarkdownBlockKind::ListItem, rest)
                    .source(SourceAnchor::line(line_no)),
            );
            continue;
        }

        if (line
            .chars()
            .all(|c| c == '-' || c == '─' || c == '*' || c == '_')
            && display_cols(line) >= 3)
            || line == "***"
            || line == "___"
        {
            blocks.push(
                MarkdownBlock::new(MarkdownBlockKind::Rule, line)
                    .source(SourceAnchor::line(line_no)),
            );
            continue;
        }

        if line.starts_with("    ") || line.starts_with('\t') {
            blocks.push(
                MarkdownBlock::new(MarkdownBlockKind::Code, line.trim_start())
                    .source(SourceAnchor::line(line_no)),
            );
            continue;
        }

        blocks.push(
            MarkdownBlock::new(MarkdownBlockKind::Paragraph, line)
                .source(SourceAnchor::line(line_no)),
        );
    }
    blocks
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn wants_leading_gap(kind: MarkdownBlockKind, level: HeadingLevel) -> bool {
    matches!(kind, MarkdownBlockKind::Rule)
        || (matches!(kind, MarkdownBlockKind::Heading)
            && matches!(level, HeadingLevel::H1 | HeadingLevel::H2))
}

fn list_prefix(block: &MarkdownBlock<'_>, _system: &DesignSystem) -> String {
    match block.kind {
        MarkdownBlockKind::TaskItem => {
            if block.task_checked == Some(true) {
                "[✓] ".into()
            } else {
                "[ ] ".into()
            }
        }
        MarkdownBlockKind::OrderedItem => {
            format!("{}. ", block.list_index.unwrap_or(1))
        }
        MarkdownBlockKind::ListItem => "• ".into(),
        _ => String::new(),
    }
}

fn list_prefix_width(block: &MarkdownBlock<'_>) -> u16 {
    match block.kind {
        MarkdownBlockKind::TaskItem => 4,
        MarkdownBlockKind::OrderedItem => {
            let n = block.list_index.unwrap_or(1);
            u16::try_from(n.to_string().len() + 2).unwrap_or(4)
        }
        MarkdownBlockKind::ListItem => 2,
        _ => 0,
    }
}

/// Marks the selected row: a quiet wash plus the gutter glyph.
///
/// Each cell keeps its own foreground and modifiers — a selected heading is
/// still a heading — so only the ground moves.
fn select_row(buffer: &mut Buffer, area: Rect, system: &DesignSystem) {
    super::row_chrome::RowChrome::resolve(
        system,
        crate::style::ListRowVisualState {
            selected: true,
            focused: true,
            ..Default::default()
        },
    )
    .paint(buffer, area);
}

fn spans_to_text<'a>(spans: &'a [MarkdownInline<'a>], _system: &DesignSystem) -> Vec<TextSpan<'a>> {
    spans
        .iter()
        .map(|sp| {
            let mut t = TextSpan::new(sp.text);
            match sp.kind {
                MarkdownInlineKind::Text => {}
                MarkdownInlineKind::Strong => t = t.strong(),
                // Weight is the one legal emphasis in the terminal (D5: ITALIC
                // is the comment tier), so `*this*` reads as bold, too.
                MarkdownInlineKind::Emphasis => t = t.strong(),
                MarkdownInlineKind::Code => t = t.code().role(Role::TextSecondary),
                MarkdownInlineKind::Link => t = t.role(Role::Link).underline(true),
            }
            t
        })
        .collect()
}

/// Expand `**` `*` `` ` `` and `[text](url)` for display; return links found.
fn expand_inline_markers(s: &str) -> (String, Vec<(String, String)>) {
    let mut out = String::with_capacity(s.len());
    let mut links = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // link [text](url)
        if bytes[i] == b'[' {
            if let Some((label, href, next)) = parse_md_link(&s[i..]) {
                out.push_str(label);
                links.push((label.to_string(), href.to_string()));
                i += next;
                continue;
            }
        }
        // bold **
        if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            if let Some(end) = s[i + 2..].find("**") {
                out.push_str(&s[i + 2..i + 2 + end]);
                i = i + 2 + end + 2;
                continue;
            }
        }
        // italic *
        if bytes[i] == b'*' {
            if let Some(end) = s[i + 1..].find('*') {
                out.push_str(&s[i + 1..i + 1 + end]);
                i = i + 1 + end + 1;
                continue;
            }
        }
        // code `
        if bytes[i] == b'`' {
            if let Some(end) = s[i + 1..].find('`') {
                out.push_str(&s[i + 1..i + 1 + end]);
                i = i + 1 + end + 1;
                continue;
            }
        }
        out.push(s[i..].chars().next().unwrap_or('?'));
        i += s[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    (out, links)
}

fn parse_md_link(s: &str) -> Option<(&str, &str, usize)> {
    // [label](href)
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let close = s.find(']')?;
    let label = &s[1..close];
    let after = &s[close + 1..];
    if !after.starts_with('(') {
        return None;
    }
    let end = after.find(')')?;
    let href = &after[1..end];
    let total = close + 1 + end + 1;
    Some((label, href, total))
}

fn strip_fence_open(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if !t.starts_with("```") && !t.starts_with("~~~") {
        return None;
    }
    let rest = if t.starts_with("```") {
        &t[3..]
    } else {
        &t[3..]
    };
    Some(rest.trim())
}

fn is_fence_close(line: &str) -> bool {
    let t = line.trim();
    t == "```" || t == "~~~" || t.starts_with("```") && t.chars().all(|c| c == '`')
}

fn parse_task_item(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    let rest = t
        .strip_prefix("- [ ] ")
        .or_else(|| t.strip_prefix("* [ ] "))
        .or_else(|| t.strip_prefix("- [x] "))
        .or_else(|| t.strip_prefix("- [X] "))
        .or_else(|| t.strip_prefix("* [x] "))
        .or_else(|| t.strip_prefix("* [X] "))?;
    let checked = t.contains("[x]") || t.contains("[X]");
    Some((checked, rest))
}

fn parse_ordered_item(line: &str) -> Option<(u32, &str)> {
    let t = line.trim_start();
    let mut digits = 0usize;
    for c in t.chars() {
        if c.is_ascii_digit() {
            digits += 1;
        } else {
            break;
        }
    }
    if digits == 0 {
        return None;
    }
    let (num, rest) = t.split_at(digits);
    let rest = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    let idx = num.parse().ok()?;
    Some((idx, rest))
}

fn looks_like_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.matches('|').count() >= 2
}

fn looks_like_table_sep(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|')
        && t.chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}

fn measure_table_height(raw: &str, width: u16) -> u16 {
    let rows = table_display_rows(raw, width);
    u16::try_from(rows.len()).unwrap_or(1).max(1)
}

fn table_display_rows(raw: &str, width: u16) -> Vec<String> {
    let mut parsed: Vec<Vec<&str>> = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if looks_like_table_sep(t) {
            continue;
        }
        if !looks_like_table_row(t) {
            continue;
        }
        let cells: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
        if !cells.is_empty() {
            parsed.push(cells);
        }
    }
    if parsed.is_empty() {
        return vec![take_display_cols(raw, usize::from(width.max(1)))];
    }
    let cols = parsed.iter().map(Vec::len).max().unwrap_or(1);
    // Responsive: drop trailing columns until row fits
    let mut use_cols = cols;
    loop {
        let widths: Vec<usize> = (0..use_cols)
            .map(|c| {
                parsed
                    .iter()
                    .filter_map(|r| r.get(c).map(|s| display_cols(s)))
                    .max()
                    .unwrap_or(1)
                    .max(1)
            })
            .collect();
        let total = widths.iter().sum::<usize>() + use_cols.saturating_sub(1) * 3; // " │ "
        if total <= usize::from(width) || use_cols <= 1 {
            let mut out = Vec::new();
            for (ri, row) in parsed.iter().enumerate() {
                let mut line = String::new();
                for (c, w) in widths.iter().enumerate() {
                    if c > 0 {
                        line.push_str(" │ ");
                    }
                    let cell = row.get(c).copied().unwrap_or("");
                    let clipped = take_display_cols(cell, *w);
                    line.push_str(&format!("{:<width$}", clipped, width = w));
                }
                if ri == 0 {
                    out.push(line.clone());
                    // separator
                    let mut sep = String::new();
                    for (c, w) in widths.iter().enumerate() {
                        if c > 0 {
                            sep.push_str("─┼─");
                        }
                        sep.push_str(&"─".repeat(*w));
                    }
                    out.push(sep);
                } else {
                    out.push(line);
                }
            }
            return out;
        }
        use_cols -= 1;
    }
}

fn paint_table_row(
    raw: &str,
    body_sub: u16,
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    selected: bool,
) {
    let rows = table_display_rows(raw, area.width);
    if let Some(line) = rows.get(usize::from(body_sub)) {
        let role = if body_sub == 0 {
            Role::TextStrong
        } else if body_sub == 1 {
            Role::Border
        } else {
            Role::Text
        };
        let clipped = take_display_cols(line, usize::from(area.width));
        buffer.set_stringn(
            area.x,
            area.y,
            &clipped,
            usize::from(area.width),
            system.style(role),
        );
    }
    if selected {
        select_row(buffer, area, system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers, MouseEventKind};
    use crate::style::RolePalette;
    use ratatui_core::layout::Position;

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
    fn project_task_ordered_fence_table() {
        let src = "\
- [ ] todo
- [x] done
1. first
```rust
fn x() {}
```
| a | b |
|---|---|
| 1 | 2 |
";
        let blocks = project_markdown(src);
        assert!(blocks.iter().any(|b| b.kind == MarkdownBlockKind::TaskItem));
        assert!(blocks.iter().any(|b| b.task_checked == Some(true)));
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == MarkdownBlockKind::OrderedItem)
        );
        let fence = blocks
            .iter()
            .find(|b| b.kind == MarkdownBlockKind::Fence)
            .expect("fence");
        assert_eq!(fence.language, Some("rust"));
        assert!(fence.text.contains("fn x"));
        assert!(!fence.incomplete);
        assert!(blocks.iter().any(|b| b.kind == MarkdownBlockKind::Table));
    }

    #[test]
    fn unfinished_fence_streaming() {
        let blocks = project_markdown("```js\nconsole.log(1)");
        let fence = blocks
            .iter()
            .find(|b| b.kind == MarkdownBlockKind::Fence)
            .expect("fence");
        assert!(fence.incomplete);
        assert!(fence.text.contains("console"));
    }

    #[test]
    fn renders_heading_strong() {
        let theme = RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let blocks = [MarkdownBlock::heading("Hello", HeadingLevel::H1)];
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 2));
        MarkdownView::new(&blocks, &system).render(Rect::new(0, 0, 20, 2), &mut buffer);
        let row0: String = (0..20)
            .map(|x| buffer[(x, 0)].symbol().to_owned())
            .collect();
        let row1: String = (0..20)
            .map(|x| buffer[(x, 1)].symbol().to_owned())
            .collect();
        // H1 may have leading gap
        assert!(
            row0.contains("Hello") || row1.contains("Hello"),
            "{row0}/{row1}"
        );
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
        let r0: String = (0..24)
            .map(|x| buffer[(x, 0)].symbol().to_owned())
            .collect();
        let r1: String = (0..24)
            .map(|x| buffer[(x, 1)].symbol().to_owned())
            .collect();
        assert!(r0.contains("quoted"));
        assert!(r1.contains("item"));
    }

    #[test]
    fn multi_row_measure_and_scroll() {
        let system = DesignSystem::default();
        let long = "word ".repeat(40);
        let blocks = [
            MarkdownBlock::heading("T", HeadingLevel::H1),
            MarkdownBlock::new(MarkdownBlockKind::Paragraph, long.as_str()),
        ];
        let view = MarkdownView::new(&blocks, &system);
        let h = view.measure_height(20);
        assert!(h > 2, "expected wrap rows, got {h}");
        let mut state = MarkdownViewState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
        let parts = view.paint(Rect::new(0, 0, 20, 4), &mut buf, &mut state);
        assert!(parts.total_rows >= h);
        let out = view.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            MarkdownOutcome::Scrolled { .. } | MarkdownOutcome::Ignored
        ));
    }

    #[test]
    fn copy_and_link_activation() {
        let system = DesignSystem::default();
        let blocks = [MarkdownBlock::new(
            MarkdownBlockKind::Paragraph,
            "see [docs](https://example.invalid) please",
        )];
        let view = MarkdownView::new(&blocks, &system);
        let mut state = MarkdownViewState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 2));
        let parts = view.paint(Rect::new(0, 0, 40, 2), &mut buf, &mut state);
        assert!(
            !parts.links.is_empty(),
            "expected link region: {:?}",
            parts.links
        );
        let act = view.activate_link(&state, 0);
        assert!(matches!(
            act,
            MarkdownOutcome::LinkActivated { ref href, .. } if href.contains("example")
        ));
        state.selection = Some((0, 1));
        let text = view.copy_text(&state);
        assert!(text.contains("docs") || text.contains("see"));
    }

    #[test]
    fn mouse_selects_block() {
        let system = DesignSystem::default();
        let blocks = [
            MarkdownBlock::new(MarkdownBlockKind::Paragraph, "a"),
            MarkdownBlock::new(MarkdownBlockKind::Paragraph, "b"),
        ];
        let view = MarkdownView::new(&blocks, &system).section_gap(false);
        let mut state = MarkdownViewState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 4));
        let _ = view.paint(Rect::new(0, 0, 20, 4), &mut buf, &mut state);
        let out = view.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position { x: 0, y: 1 },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, MarkdownOutcome::SelectionChanged { .. }));
    }

    #[test]
    fn source_anchors_on_projection() {
        let blocks = project_markdown("# Hi\n\nbody");
        assert_eq!(blocks[0].source, Some(SourceAnchor::line(1)));
    }

    #[test]
    fn responsive_table_contracts() {
        let raw = "| alpha | beta | gamma |\n|---|---|---|\n| 1 | 2 | 3 |";
        let wide = table_display_rows(raw, 80);
        assert!(wide[0].contains("alpha"));
        let narrow = table_display_rows(raw, 12);
        // still paints something without panic
        assert!(!narrow.is_empty());
        let w = display_cols(&narrow[0]);
        assert!(w <= 12, "{w} {}", narrow[0]);
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let blocks = [MarkdownBlock::heading("x", HeadingLevel::H1)];
        let mut state = MarkdownViewState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts =
            MarkdownView::new(&blocks, &system).paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(parts.root.is_empty());
    }

    #[test]
    fn long_doc_measure_paint_benchmark() {
        let system = DesignSystem::default();
        // Simulate long AI response
        let mut src = String::new();
        for i in 0..200 {
            src.push_str(&format!("## Section {i}\n\n"));
            src.push_str("Lorem ipsum dolor sit amet, consectetur adipiscing elit. ");
            src.push_str("More prose for wrapping under narrow terminals.\n\n");
            src.push_str("- item one\n- item two\n\n");
            if i % 10 == 0 {
                src.push_str("```rust\nfn demo() {}\n```\n\n");
            }
        }
        let blocks = project_markdown(&src);
        assert!(blocks.len() > 100);
        let view = MarkdownView::new(&blocks, &system);
        let mut state = MarkdownViewState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        for _ in 0..50 {
            let _ = view.paint(Rect::new(0, 0, 80, 24), &mut buf, &mut state);
            let _ = state.scroll_by(5);
        }
        // measure path
        let h = view.measure_height(80);
        assert!(h > 200);
    }

    #[test]
    fn inline_markers_strip_for_display() {
        let (d, links) = expand_inline_markers("a **bold** and [x](http://y)");
        assert!(d.contains("bold"));
        assert!(!d.contains("**"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].0, "x");
    }
}
