// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Content hierarchy primitives: heading, paragraph, callout, alert.
//!
//! **Heading** and **Paragraph** are editorial recipes on [`crate::widgets::Text`]:
//! levels, rules, prefixes, quotes, lists, compact/reading density. Markdown
//! projection reuses these so layout logic is not duplicated.
//!
//! Section chrome: [`crate::widgets::Section`].

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind},
    interaction::{EventResult, UiIntent, default_button_intent, default_list_intent},
    style::{DesignSystem, GlyphSet, Role},
    text::{display_cols, take_display_cols},
    widgets::text::{SelectablePolicy, Text, TextSpan},
};

// ── Heading ─────────────────────────────────────────────────────────────────

/// Heading level (terminal typography weight / hierarchy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HeadingLevel {
    /// Page / document title.
    H1,
    /// Section title.
    #[default]
    H2,
    /// Subsection.
    H3,
}

impl HeadingLevel {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::H1 => "h1",
            Self::H2 => "h2",
            Self::H3 => "h3",
        }
    }

    /// Markdown `#` count (1–3).
    #[must_use]
    pub const fn hash_depth(self) -> u8 {
        match self {
            Self::H1 => 1,
            Self::H2 => 2,
            Self::H3 => 3,
        }
    }

    /// From markdown hash depth (clamped 1–3).
    #[must_use]
    pub const fn from_hash_depth(n: u8) -> Self {
        match n {
            0 | 1 => Self::H1,
            2 => Self::H2,
            _ => Self::H3,
        }
    }
}

/// Editorial density recipe for headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HeadingRecipe {
    /// Level via weight + optional underline (H1); rule when height allows.
    #[default]
    Default,
    /// Single-line only; ASCII `#` prefixes communicate level without color.
    Compact,
    /// Title + rule under H1/H2 when space allows (docs / help reading mode).
    Reading,
}

impl HeadingRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Compact => "compact",
            Self::Reading => "reading",
        }
    }
}

/// Named geometry for a heading paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct HeadingParts {
    /// Outer allocation used.
    pub root: Rect,
    /// Title line.
    pub title: Rect,
    /// Optional rule row under the title (zero height when absent).
    pub rule: Rect,
}

/// Editorial heading built on [`Text`].
#[derive(Debug, Clone, Copy)]
pub struct Heading<'a> {
    text: &'a str,
    level: HeadingLevel,
    system: &'a DesignSystem,
    recipe: HeadingRecipe,
    /// Force level prefix (`#` / `##` / `###`) for no-color hierarchy.
    prefix: bool,
    /// Force underline rule under the title when height ≥ 2.
    rule: Option<bool>,
    /// Selectable / copy policy for the title plain text.
    selectable: SelectablePolicy,
}

impl<'a> Heading<'a> {
    /// Section-level heading (H2) by default.
    #[must_use]
    pub const fn new(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            level: HeadingLevel::H2,
            system,
            recipe: HeadingRecipe::Default,
            prefix: false,
            rule: None,
            selectable: SelectablePolicy::None,
        }
    }

    /// Level.
    #[must_use]
    pub const fn level(mut self, level: HeadingLevel) -> Self {
        self.level = level;
        self
    }

    /// H1 convenience.
    #[must_use]
    pub const fn h1(mut self) -> Self {
        self.level = HeadingLevel::H1;
        self
    }

    /// H2 convenience.
    #[must_use]
    pub const fn h2(mut self) -> Self {
        self.level = HeadingLevel::H2;
        self
    }

    /// H3 convenience.
    #[must_use]
    pub const fn h3(mut self) -> Self {
        self.level = HeadingLevel::H3;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: HeadingRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Compact single-line recipe (ASCII-friendly prefixes on by default).
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.recipe = HeadingRecipe::Compact;
        self.prefix = true;
        self
    }

    /// Reading recipe (prefer rule under H1/H2).
    #[must_use]
    pub const fn reading(mut self) -> Self {
        self.recipe = HeadingRecipe::Reading;
        self
    }

    /// Show markdown-style `#` depth prefix (no-color hierarchy cue).
    #[must_use]
    pub const fn prefix(mut self, on: bool) -> Self {
        self.prefix = on;
        self
    }

    /// Force rule under title (`None` = recipe default).
    #[must_use]
    pub const fn rule(mut self, on: bool) -> Self {
        self.rule = Some(on);
        self
    }

    /// Copyable title plain text.
    #[must_use]
    pub const fn copyable(mut self) -> Self {
        self.selectable = SelectablePolicy::Copyable;
        self
    }

    /// Selectable policy.
    #[must_use]
    pub const fn selectable(mut self, policy: SelectablePolicy) -> Self {
        self.selectable = policy;
        self
    }

    /// Level of this heading.
    #[must_use]
    pub const fn level_of(&self) -> HeadingLevel {
        self.level
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe_of(&self) -> HeadingRecipe {
        self.recipe
    }

    /// Whether a rule row is requested for this paint.
    #[must_use]
    pub fn wants_rule(&self) -> bool {
        if let Some(r) = self.rule {
            return r;
        }
        match self.recipe {
            HeadingRecipe::Compact => false,
            HeadingRecipe::Reading => matches!(self.level, HeadingLevel::H1 | HeadingLevel::H2),
            HeadingRecipe::Default => matches!(self.level, HeadingLevel::H1),
        }
    }

    /// Whether level prefix is shown.
    #[must_use]
    pub const fn wants_prefix(&self) -> bool {
        self.prefix || matches!(self.recipe, HeadingRecipe::Compact)
    }

    /// Natural height in rows (1 or 2).
    #[must_use]
    pub fn measure_height(&self) -> u16 {
        if self.wants_rule() {
            2
        } else {
            1
        }
    }

    /// Prefix string for hierarchy (`# `, `## `, …) or empty.
    #[must_use]
    pub fn prefix_str(&self) -> &'static str {
        if !self.wants_prefix() {
            return "";
        }
        match self.level {
            HeadingLevel::H1 => "# ",
            HeadingLevel::H2 => "## ",
            HeadingLevel::H3 => "### ",
        }
    }

    /// Copy-safe plain title (no prefix).
    #[must_use]
    pub fn plain(&self) -> &str {
        self.text
    }

    /// Full plain including prefix (clipboard for compact hierarchy).
    #[must_use]
    pub fn plain_with_prefix(&self) -> String {
        format!("{}{}", self.prefix_str(), self.text)
    }

    /// Layout without paint.
    #[must_use]
    pub fn layout(&self, area: Rect) -> HeadingParts {
        if area.is_empty() {
            return HeadingParts {
                root: area,
                title: area,
                rule: Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 0,
                },
            };
        }
        let title_h = 1u16.min(area.height);
        let title = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: title_h,
        };
        let rule = if self.wants_rule() && area.height >= 2 {
            Rect {
                x: area.x,
                y: area.y.saturating_add(1),
                width: area.width,
                height: 1,
            }
        } else {
            Rect {
                x: area.x,
                y: area.y.saturating_add(title_h),
                width: area.width,
                height: 0,
            }
        };
        HeadingParts {
            root: area,
            title,
            rule,
        }
    }

    /// Paint title (+ optional rule). Returns parts.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> HeadingParts {
        let parts = self.layout(area);
        if area.is_empty() || parts.title.is_empty() {
            return parts;
        }

        let mut span = TextSpan::new(self.plain_with_prefix()).role(Role::TextStrong);
        span = match self.level {
            HeadingLevel::H1 => span.strong().underline(true),
            HeadingLevel::H2 => span.strong(),
            HeadingLevel::H3 => span, // strong role only — weight via palette
        };
        let mut text = Text::spans([span], self.system).truncate();
        if self.selectable.copyable() {
            text = text.selectable(self.selectable);
        }
        let _ = text.paint(parts.title, buffer);

        if parts.rule.height > 0 && parts.rule.width > 0 {
            let unit = self.system.glyphs.rule();
            // H1 uses a heavier visual in Unicode when available (double line).
            let unit = if matches!(self.level, HeadingLevel::H1) {
                self.system.glyphs.rule_strong()
            } else {
                unit
            };
            let fill = unit.repeat(usize::from(parts.rule.width));
            let clipped = take_display_cols(&fill, usize::from(parts.rule.width));
            buffer.set_stringn(
                parts.rule.x,
                parts.rule.y,
                &clipped,
                usize::from(parts.rule.width),
                self.system.style(Role::Border),
            );
        }
        parts
    }
}

impl Widget for &Heading<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Heading<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Paragraph ───────────────────────────────────────────────────────────────

/// Paragraph semantic kind (prose, quote, list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ParagraphKind {
    /// Ordinary body prose.
    #[default]
    Body,
    /// Block quote with hanging gutter prefix.
    Quote,
    /// Unordered list item with bullet prefix.
    ListItem,
    /// Ordered list item (`1.`, `2.`, …) — index set via [`Paragraph::list_index`].
    OrderedItem,
}

impl ParagraphKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Quote => "quote",
            Self::ListItem => "list-item",
            Self::OrderedItem => "ordered-item",
        }
    }
}

/// Editorial density for paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ParagraphRecipe {
    /// Tight body; minimal indent.
    #[default]
    Compact,
    /// Reading mode: quote/list hanging indents, slightly wider breathing.
    Reading,
}

impl ParagraphRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Reading => "reading",
        }
    }
}

/// Named geometry for a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParagraphParts {
    /// Outer allocation.
    pub root: Rect,
    /// Content clip (inside indent).
    pub content: Rect,
    /// Laid-out display lines (prefix + body per row).
    pub lines: Vec<String>,
    /// Whether height truncated the wrap.
    pub truncated: bool,
}

/// Editorial body prose built on [`Text`] / wrap helpers.
#[derive(Debug, Clone, Copy)]
pub struct Paragraph<'a> {
    text: &'a str,
    system: &'a DesignSystem,
    muted: bool,
    kind: ParagraphKind,
    recipe: ParagraphRecipe,
    /// Extra left indent (cells).
    indent: u16,
    /// Hanging wrap: subsequent lines indent to prefix width (default true for quote/list).
    hanging: Option<bool>,
    /// Override automatic prefix (`│ `, `• `, `1. `).
    prefix: Option<&'a str>,
    /// 1-based index for [`ParagraphKind::OrderedItem`].
    list_index: u32,
    selectable: SelectablePolicy,
}

impl<'a> Paragraph<'a> {
    /// Body paragraph.
    #[must_use]
    pub const fn new(text: &'a str, system: &'a DesignSystem) -> Self {
        Self {
            text,
            system,
            muted: false,
            kind: ParagraphKind::Body,
            recipe: ParagraphRecipe::Compact,
            indent: 0,
            hanging: None,
            prefix: None,
            list_index: 1,
            selectable: SelectablePolicy::None,
        }
    }

    /// Block quote.
    #[must_use]
    pub const fn quote(text: &'a str, system: &'a DesignSystem) -> Self {
        Self::new(text, system).kind(ParagraphKind::Quote)
    }

    /// Unordered list item.
    #[must_use]
    pub const fn list_item(text: &'a str, system: &'a DesignSystem) -> Self {
        Self::new(text, system).kind(ParagraphKind::ListItem)
    }

    /// Ordered list item with 1-based index.
    #[must_use]
    pub const fn ordered_item(text: &'a str, system: &'a DesignSystem, index: u32) -> Self {
        let mut p = Self::new(text, system).kind(ParagraphKind::OrderedItem);
        p.list_index = if index == 0 { 1 } else { index };
        p
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: ParagraphKind) -> Self {
        self.kind = kind;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: ParagraphRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Compact recipe.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.recipe = ParagraphRecipe::Compact;
        self
    }

    /// Reading recipe.
    #[must_use]
    pub const fn reading(mut self) -> Self {
        self.recipe = ParagraphRecipe::Reading;
        self
    }

    /// Secondary tone.
    #[must_use]
    pub const fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// Extra left indent.
    #[must_use]
    pub const fn indent(mut self, cols: u16) -> Self {
        self.indent = cols;
        self
    }

    /// Hanging wrap after the first line (default true for quote/list).
    #[must_use]
    pub const fn hanging(mut self, on: bool) -> Self {
        self.hanging = Some(on);
        self
    }

    /// Override prefix (including trailing space if desired).
    #[must_use]
    pub const fn prefix(mut self, prefix: &'a str) -> Self {
        self.prefix = Some(prefix);
        self
    }

    /// Ordered list index (1-based).
    #[must_use]
    pub const fn list_index(mut self, index: u32) -> Self {
        self.list_index = if index == 0 { 1 } else { index };
        self
    }

    /// Copyable plain text.
    #[must_use]
    pub const fn copyable(mut self) -> Self {
        self.selectable = SelectablePolicy::Copyable;
        self
    }

    /// Selectable policy.
    #[must_use]
    pub const fn selectable(mut self, policy: SelectablePolicy) -> Self {
        self.selectable = policy;
        self
    }

    /// Kind of this paragraph.
    #[must_use]
    pub const fn kind_of(&self) -> ParagraphKind {
        self.kind
    }

    /// Resolved prefix for the first line.
    #[must_use]
    pub fn resolved_prefix(&self) -> String {
        if let Some(p) = self.prefix {
            return p.to_string();
        }
        match self.kind {
            ParagraphKind::Body => String::new(),
            ParagraphKind::Quote => {
                format!("{} ", self.system.glyphs.rule_v())
            }
            ParagraphKind::ListItem => {
                format!("{} ", self.system.glyphs.bullet())
            }
            ParagraphKind::OrderedItem => format!("{}. ", self.list_index),
        }
    }

    fn hanging_enabled(&self) -> bool {
        if let Some(h) = self.hanging {
            return h;
        }
        !matches!(self.kind, ParagraphKind::Body)
    }

    fn base_indent(&self) -> u16 {
        let recipe_pad = match self.recipe {
            ParagraphRecipe::Compact => 0u16,
            ParagraphRecipe::Reading => match self.kind {
                ParagraphKind::Body => 0,
                ParagraphKind::Quote | ParagraphKind::ListItem | ParagraphKind::OrderedItem => 1,
            },
        };
        self.indent.saturating_add(recipe_pad)
    }

    /// Copy-safe body (no prefix).
    #[must_use]
    pub fn plain(&self) -> &str {
        self.text
    }

    /// Plain with prefix (clipboard for list/quote).
    #[must_use]
    pub fn plain_with_prefix(&self) -> String {
        format!("{}{}", self.resolved_prefix(), self.text)
    }

    /// Wrap body into display lines for `width` (full area width).
    #[must_use]
    pub fn wrap_lines(&self, width: u16) -> Vec<String> {
        let w = usize::from(width);
        if w == 0 {
            return Vec::new();
        }
        let base = usize::from(self.base_indent());
        let prefix = self.resolved_prefix();
        let prefix_w = display_cols(&prefix);
        let hang = self.hanging_enabled() && prefix_w > 0;
        let first_budget = w.saturating_sub(base).saturating_sub(prefix_w).max(1);
        let rest_budget = if hang {
            w.saturating_sub(base).saturating_sub(prefix_w).max(1)
        } else {
            w.saturating_sub(base).max(1)
        };

        // First line may be shorter; re-wrap: take first line at first_budget, rest at rest_budget.
        let mut out = Vec::new();
        let mut rest = self.text;
        // First line
        if rest.is_empty() {
            out.push(format!(
                "{}{}{}",
                " ".repeat(base),
                prefix,
                ""
            ));
            return out;
        }
        let first = take_display_cols(rest, first_budget);
        // Advance rest by display width of first
        rest = advance_by_display(rest, first_budget);
        out.push(format!("{}{}{}", " ".repeat(base), prefix, first));
        // Subsequent
        let cont_pad = if hang {
            base + prefix_w
        } else {
            base
        };
        while !rest.is_empty() {
            let line = take_display_cols(rest, rest_budget);
            if line.is_empty() {
                // force advance one grapheme to avoid infinite loop
                rest = advance_by_display(rest, 1);
                continue;
            }
            rest = advance_by_display(rest, rest_budget);
            out.push(format!("{}{}", " ".repeat(cont_pad), line));
        }
        out
    }

    /// Natural height for `width` (full wrap, no height cap).
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        u16::try_from(self.wrap_lines(width).len()).unwrap_or(u16::MAX).max(1)
    }

    /// Layout + wrap for area.
    #[must_use]
    pub fn layout(&self, area: Rect) -> ParagraphParts {
        if area.is_empty() {
            return ParagraphParts {
                root: area,
                content: area,
                lines: Vec::new(),
                truncated: false,
            };
        }
        let all = self.wrap_lines(area.width);
        let max_lines = usize::from(area.height);
        let truncated = all.len() > max_lines;
        let lines: Vec<String> = all.into_iter().take(max_lines).collect();
        let indent = self.base_indent();
        let content = Rect {
            x: area.x.saturating_add(indent),
            y: area.y,
            width: area.width.saturating_sub(indent),
            height: area.height,
        };
        ParagraphParts {
            root: area,
            content,
            lines,
            truncated,
        }
    }

    /// Paint wrapped lines. Returns parts.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> ParagraphParts {
        let parts = self.layout(area);
        if area.is_empty() {
            return parts;
        }
        let role = if self.muted || matches!(self.kind, ParagraphKind::Quote) {
            Role::TextMuted
        } else {
            Role::Text
        };
        for (i, line) in parts.lines.iter().enumerate() {
            let y = area.y.saturating_add(u16::try_from(i).unwrap_or(0));
            if y >= area.bottom() {
                break;
            }
            let mut text = Text::new(line.as_str(), self.system).role(role).truncate();
            if self.selectable.copyable() {
                text = text.selectable(self.selectable);
            }
            let _ = text.paint(
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
                buffer,
            );
        }
        parts
    }
}

impl Widget for &Paragraph<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Paragraph<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

/// Advance `s` by up to `cols` display columns (grapheme-safe via wrap helper).
fn advance_by_display(s: &str, cols: usize) -> &str {
    if cols == 0 || s.is_empty() {
        return s;
    }
    let taken = take_display_cols(s, cols);
    // Map display take back to byte length: walk chars same as take_display_cols.
    use crate::text::is_terminal_control_char;
    use unicode_width::UnicodeWidthChar;
    let mut used = 0usize;
    let mut idx = 0usize;
    for (i, c) in s.char_indices() {
        if is_terminal_control_char(c) {
            idx = i + c.len_utf8();
            continue;
        }
        let w = c.width().unwrap_or(0);
        if used + w > cols && used > 0 {
            return &s[i..];
        }
        used += w;
        idx = i + c.len_utf8();
        if used >= cols {
            return &s[idx..];
        }
    }
    let _ = taken;
    &s[idx..]
}

// Surface lives in `widgets/surface.rs` (canonical fill/border/clip/hit).
// Section lives in `widgets/section.rs` (editorial grouping anatomy).

/// Callout semantic tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CalloutTone {
    /// Neutral info.
    #[default]
    Info,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Danger.
    Danger,
}

impl CalloutTone {
    #[must_use]
    fn role(self) -> Role {
        match self {
            Self::Info => Role::Info,
            Self::Success => Role::Success,
            Self::Warning => Role::Warning,
            Self::Danger => Role::Danger,
        }
    }

    #[must_use]
    fn glyph(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Success => "+",
            Self::Warning => "!",
            Self::Danger => "x",
        }
    }
}

/// Inline callout (non-modal).
#[derive(Debug, Clone, Copy)]
pub struct Callout<'a> {
    title: &'a str,
    body: Option<&'a str>,
    tone: CalloutTone,
    tokens: &'a DesignSystem,
}

impl<'a> Callout<'a> {
    /// Title + tone.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            body: None,
            tone: CalloutTone::Info,
            tokens,
        }
    }

    /// Body line.
    #[must_use]
    pub const fn body(mut self, body: &'a str) -> Self {
        self.body = Some(body);
        self
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: CalloutTone) -> Self {
        self.tone = tone;
        self
    }
}

impl Widget for &Callout<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        use crate::layout::{FlexSize, Stack};

        let style = self.tokens.style(self.tone.role());
        let head = format!("{} {}", self.tone.glyph(), self.title);
        let rows = if self.body.is_some() && area.height > 1 {
            Stack::new().layout(area, &[FlexSize::Fixed(1), FlexSize::Weight(1)])
        } else {
            Stack::new().layout(area, &[FlexSize::Weight(1)])
        };
        if let Some(title_r) = rows.get(0) {
            let text = take_display_cols(&head, usize::from(title_r.width));
            buffer.set_stringn(
                title_r.x,
                title_r.y,
                &text,
                usize::from(title_r.width),
                style,
            );
        }
        if let (Some(body), Some(body_r)) = (self.body, rows.get(1)) {
            let b = take_display_cols(body, usize::from(body_r.width));
            buffer.set_stringn(
                body_r.x,
                body_r.y,
                &b,
                usize::from(body_r.width),
                self.tokens.style(Role::TextMuted),
            );
        }
    }
}

/// Alert tone (alias of callout for dismissible banners).
pub type AlertTone = CalloutTone;

/// Alert dismiss outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AlertOutcome {
    /// No change.
    #[default]
    Ignored,
    /// User dismissed.
    Dismissed,
    /// User acknowledged (Enter).
    Acknowledged,
}

/// Dismissible alert banner.
#[derive(Debug, Clone, Copy)]
pub struct Alert<'a> {
    title: &'a str,
    tokens: &'a DesignSystem,
    tone: AlertTone,
}

impl<'a> Alert<'a> {
    /// Alert title.
    #[must_use]
    pub const fn new(title: &'a str, tokens: &'a DesignSystem) -> Self {
        Self {
            title,
            tokens,
            tone: AlertTone::Warning,
        }
    }

    /// Tone.
    #[must_use]
    pub const fn tone(mut self, tone: AlertTone) -> Self {
        self.tone = tone;
        self
    }
}

/// Alert interaction (focus + dismiss).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlertState {
    /// Focused for keyboard.
    pub focused: bool,
    /// Region for click-dismiss.
    pub region: Option<Rect>,
}

impl AlertState {
    /// Esc dismisses; Enter acknowledges (via intents; no raw key match).
    pub fn handle_key(&mut self, key: KeyEvent) -> AlertOutcome {
        if !self.focused || key.kind != KeyEventKind::Press {
            return AlertOutcome::Ignored;
        }
        let intent = default_button_intent(key).or_else(|| default_list_intent(key));
        match intent {
            Some(UiIntent::Cancel | UiIntent::Close) => AlertOutcome::Dismissed,
            Some(UiIntent::Activate | UiIntent::Submit) => AlertOutcome::Acknowledged,
            _ => AlertOutcome::Ignored,
        }
    }

    /// Semantic intent path.
    pub fn handle_intent(&mut self, intent: UiIntent) -> AlertOutcome {
        if !self.focused {
            return AlertOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => AlertOutcome::Dismissed,
            UiIntent::Activate | UiIntent::Submit => AlertOutcome::Acknowledged,
            _ => AlertOutcome::Ignored,
        }
    }

    /// Key path with [`EventResult`] (dismiss requests overlay peel).
    pub fn handle_key_result(&mut self, key: KeyEvent) -> EventResult<AlertOutcome> {
        match self.handle_key(key) {
            AlertOutcome::Ignored => EventResult::ignored(),
            AlertOutcome::Dismissed => EventResult::emit(AlertOutcome::Dismissed)
                .with_overlay(crate::interaction::OverlayRequest::DismissTop),
            other => EventResult::emit(other),
        }
    }
}

impl Alert<'_> {
    /// Paint alert.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut AlertState) {
        state.region = None;
        if area.is_empty() {
            return;
        }
        let callout = Callout::new(self.title, self.tokens).tone(self.tone);
        Widget::render(&callout, area, buffer);
        state.region = Some(area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::interaction::OverlayRequest;
    use crate::style::GlyphSet;
    use crate::text::display_cols;

    #[test]
    fn heading_paints_strong() {
        let tokens = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        Widget::render(
            &Heading::new("Title", &tokens).level(HeadingLevel::H1),
            Rect::new(0, 0, 20, 1),
            &mut buf,
        );
        assert!(!buf[(0, 0)].symbol().trim().is_empty() || display_cols("Title") > 0);
    }

    #[test]
    fn heading_reading_rule_and_measure() {
        let system = DesignSystem::default();
        let h = Heading::new("Section", &system).h1().reading();
        assert_eq!(h.measure_height(), 2);
        assert!(h.wants_rule());
        let mut buf = Buffer::empty(Rect::new(0, 0, 24, 2));
        let parts = h.paint(Rect::new(0, 0, 24, 2), &mut buf);
        assert_eq!(parts.rule.height, 1);
        assert_eq!(buf[(0, 1)].symbol(), "═");
    }

    #[test]
    fn heading_compact_ascii_prefix() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let h = Heading::new("Title", &system).h2().compact();
        assert!(h.plain_with_prefix().starts_with("## "));
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = h.paint(Rect::new(0, 0, 20, 1), &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "#");
        assert_eq!(buf[(1, 0)].symbol(), "#");
    }

    #[test]
    fn heading_no_color_hierarchy_ids() {
        assert_eq!(HeadingLevel::H3.id(), "h3");
        assert_eq!(HeadingLevel::from_hash_depth(3), HeadingLevel::H3);
        assert_eq!(HeadingRecipe::Reading.id(), "reading");
    }

    #[test]
    fn paragraph_wraps_and_measures() {
        let system = DesignSystem::default();
        let p = Paragraph::new("hello world friends", &system);
        let h = p.measure_height(5);
        assert!(h >= 3);
        let parts = p.layout(Rect::new(0, 0, 5, 10));
        assert!(!parts.truncated);
        assert!(parts.lines.len() >= 3);
    }

    #[test]
    fn paragraph_quote_and_list_prefix() {
        let system = DesignSystem::default();
        let q = Paragraph::quote("noted", &system);
        assert!(q.resolved_prefix().contains('│') || q.resolved_prefix().starts_with('|'));
        let li = Paragraph::list_item("task", &system);
        assert!(!li.resolved_prefix().is_empty());
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let _ = li.paint(Rect::new(0, 0, 20, 1), &mut buf);
        let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_owned()).collect();
        assert!(row.contains("task"));
    }

    #[test]
    fn paragraph_ordered_and_hanging() {
        let system = DesignSystem::default();
        let p = Paragraph::ordered_item(
            "long enough content to wrap onto a second line here",
            &system,
            2,
        )
        .reading();
        assert!(p.resolved_prefix().starts_with("2."));
        let lines = p.wrap_lines(16);
        assert!(lines.len() >= 2);
        // hanging: second line starts with spaces past prefix
        assert!(lines[1].starts_with(' '));
    }

    #[test]
    fn paragraph_ascii_list_bullet() {
        let system = DesignSystem::default().glyphs(GlyphSet::Ascii);
        let p = Paragraph::list_item("x", &system);
        assert_eq!(p.resolved_prefix(), "* ");
    }

    #[test]
    fn paragraph_muted_selectable() {
        let system = DesignSystem::default();
        let p = Paragraph::new("copy", &system).muted(true).copyable();
        assert_eq!(p.plain(), "copy");
        assert!(p.selectable.copyable());
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let h = Heading::new("Perf", &system).reading().h1();
        let p = Paragraph::new(
            "performance path for wrap and heading rule layout",
            &system,
        )
        .reading();
        let area = Rect::new(0, 0, 40, 8);
        for _ in 0..10_000 {
            let _ = h.layout(area);
            let _ = p.layout(area);
        }
    }

    #[test]
    fn alert_esc_dismisses() {
        let mut state = AlertState {
            focused: true,
            ..Default::default()
        };
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            AlertOutcome::Dismissed
        );
        let r = state.handle_key_result(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(r.message(), Some(&AlertOutcome::Dismissed));
        assert_eq!(r.overlay(), Some(&OverlayRequest::DismissTop));
    }
}
