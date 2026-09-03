// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Text — canonical styled text primitive for semantic content.
//!
//! **Anatomy:** one or more [`TextSpan`] runs · optional layout (wrap / truncate /
//! align). Paint resolves [`Role`] + emphasis through [`DesignSystem`]; background
//! is left unset when [`Text::preserve_bg`] is true so the terminal default shows
//! through.
//!
//! Handles graphemes, combining marks, CJK, emoji, tabs, and control stripping via
//! [`crate::text`]. Syntax-independent [`TextSpan::annotation`] tags and
//! [`TextSpan::highlight`] marks are host-defined (search hits, diagnostics).
//!
//! References: Rich Text, Textual Static, Glow typography, Ratatui Line/Span.
use std::borrow::Cow;

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

use crate::style::{DesignSystem, Role};
use crate::text::{
    TruncateMode, display_cols, expand_tabs, take_display_cols, truncate_display_cols,
};

/// Horizontal alignment within the paint rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TextAlign {
    /// Left / start edge (default).
    #[default]
    Start,
    /// Centered in the available width.
    Center,
    /// Right / end edge.
    End,
}

impl TextAlign {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}

/// Overflow when content exceeds the allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TextOverflow {
    /// Clip at bounds (no ellipsis). Default for single-line paint.
    #[default]
    Clip,
    /// Soft-wrap on display columns across available height.
    Wrap,
    /// Ellipsis at end.
    Truncate,
    /// Ellipsis at start.
    TruncateStart,
    /// Ellipsis in the middle.
    TruncateMiddle,
}

impl TextOverflow {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Wrap => "wrap",
            Self::Truncate => "truncate",
            Self::TruncateStart => "truncate-start",
            Self::TruncateMiddle => "truncate-middle",
        }
    }
}

/// Host clipboard / selection policy (paint is always non-interactive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectablePolicy {
    /// No copy contract (default).
    #[default]
    None,
    /// Host may use [`Text::plain`] for clipboard (controls stripped).
    Copyable,
    /// Same plain text path; reserved for future selection chrome.
    Selectable,
}

impl SelectablePolicy {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Copyable => "copyable",
            Self::Selectable => "selectable",
        }
    }

    /// Whether plain text is intended for copy.
    #[must_use]
    pub const fn copyable(self) -> bool {
        matches!(self, Self::Copyable | Self::Selectable)
    }
}

/// Visual emphasis layered on a semantic [`Role`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TextEmphasis {
    /// Role style only.
    #[default]
    Normal,
    /// Bold weight.
    Strong,
    /// Dim / secondary (modifier + optional muted role).
    Dim,
    /// Inline code cue (no filled background — preserves terminal default bg).
    Code,
}

impl TextEmphasis {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Strong => "strong",
            Self::Dim => "dim",
            Self::Code => "code",
        }
    }
}

/// One styled run of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan<'a> {
    content: Cow<'a, str>,
    role: Role,
    emphasis: TextEmphasis,
    /// Author-set underline for content that is underlined (composes with
    /// emphasis). This is content, not a state cue.
    underline: bool,
    reverse: bool,
    /// Syntax-independent annotation tag (search, diagnostic, link id, …).
    annotation: Option<Cow<'a, str>>,
    /// Highlight mark (search hit, selection preview) without requiring bg fill.
    highlight: bool,
}

impl<'a> TextSpan<'a> {
    /// Plain run with body text role.
    #[must_use]
    pub fn new(content: impl Into<Cow<'a, str>>) -> Self {
        Self {
            content: content.into(),
            role: Role::Text,
            emphasis: TextEmphasis::Normal,
            underline: false,
            reverse: false,
            annotation: None,
            highlight: false,
        }
    }

    /// Semantic role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Strong weight.
    #[must_use]
    pub const fn strong(mut self) -> Self {
        self.emphasis = TextEmphasis::Strong;
        self
    }

    /// Dim secondary.
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.emphasis = TextEmphasis::Dim;
        self
    }

    /// Inline code cue.
    #[must_use]
    pub const fn code(mut self) -> Self {
        self.emphasis = TextEmphasis::Code;
        self
    }

    /// Underline this run (composes with emphasis).
    #[must_use]
    pub const fn underline(mut self, on: bool) -> Self {
        self.underline = on;
        self
    }

    /// Reverse this run — the colorless way to say "this one".
    #[must_use]
    pub const fn reverse(mut self, on: bool) -> Self {
        self.reverse = on;
        self
    }

    /// Annotation tag (owned or borrowed).
    #[must_use]
    pub fn annotation(mut self, tag: impl Into<Cow<'a, str>>) -> Self {
        self.annotation = Some(tag.into());
        self
    }

    /// Highlight this run.
    #[must_use]
    pub const fn highlight(mut self, on: bool) -> Self {
        self.highlight = on;
        self
    }

    /// Content borrow.
    #[must_use]
    pub fn content(&self) -> &str {
        self.content.as_ref()
    }
    /// Annotation borrow.
    #[must_use]
    pub fn annotation_of(&self) -> Option<&str> {
        self.annotation.as_deref()
    }

    /// Whether highlighted.
    #[must_use]
    pub const fn is_highlight(&self) -> bool {
        self.highlight
    }
}

/// Resolved segment after layout (for tests / host tools).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSegment {
    /// Visible text (controls stripped, tabs expanded).
    pub text: String,
    /// Resolved paint style.
    pub style: Style,
    /// Optional annotation carried from the source span.
    pub annotation: Option<String>,
    /// Highlight flag.
    pub highlight: bool,
}

/// One laid-out visual line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextLine {
    /// Segments left-to-right.
    pub segments: Vec<TextSegment>,
    /// Display columns used (before alignment padding).
    pub width: usize,
}

impl TextLine {
    /// Concatenated plain text for the line.
    #[must_use]
    pub fn plain(&self) -> String {
        self.segments.iter().map(|s| s.text.as_str()).collect()
    }
}

/// Layout result for a given width/height budget.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextLayout {
    /// Visual lines that fit in the height budget (may be truncated).
    pub lines: Vec<TextLine>,
    /// Full plain text (all spans, controls stripped, tabs expanded).
    pub plain: String,
    /// True when content was ellipsized or height-clipped.
    pub truncated: bool,
    /// Ideal height in rows if fully wrapped (ignoring height budget).
    pub natural_height: u16,
}

/// Canonical semantic text primitive.
#[derive(Debug, Clone)]
pub struct Text<'a> {
    spans: Vec<TextSpan<'a>>,
    system: &'a DesignSystem,
    overflow: TextOverflow,
    align: TextAlign,
    selectable: SelectablePolicy,
    /// When true (default), strip background from resolved styles.
    preserve_bg: bool,
    /// Tab stop width (default 4).
    tab_width: usize,
    /// Ellipsis glyph (Unicode `…` by default; host may pass `...` for ASCII).
    ellipsis: Cow<'a, str>,
}

impl<'a> Text<'a> {
    /// Single plain body span.
    #[must_use]
    pub fn new(content: impl Into<Cow<'a, str>>, system: &'a DesignSystem) -> Self {
        Self {
            spans: vec![TextSpan::new(content)],
            system,
            overflow: TextOverflow::Clip,
            align: TextAlign::Start,
            selectable: SelectablePolicy::None,
            preserve_bg: true,
            tab_width: 4,
            ellipsis: Cow::Borrowed("…"),
        }
    }

    /// Multi-span constructor.
    #[must_use]
    pub fn spans<I>(spans: I, system: &'a DesignSystem) -> Self
    where
        I: IntoIterator<Item = TextSpan<'a>>,
    {
        Self {
            spans: spans.into_iter().collect(),
            system,
            overflow: TextOverflow::Clip,
            align: TextAlign::Start,
            selectable: SelectablePolicy::None,
            preserve_bg: true,
            tab_width: 4,
            ellipsis: Cow::Borrowed("…"),
        }
    }
    /// Role for the first span (convenience for single-span text).
    #[must_use]
    pub fn role(mut self, role: Role) -> Self {
        if let Some(s) = self.spans.first_mut() {
            s.role = role;
        }
        self
    }

    /// Emphasis for the first span.
    #[must_use]
    pub fn emphasis(mut self, emphasis: TextEmphasis) -> Self {
        if let Some(s) = self.spans.first_mut() {
            s.emphasis = emphasis;
        }
        self
    }

    /// Soft wrap.
    #[must_use]
    pub const fn wrap(mut self) -> Self {
        self.overflow = TextOverflow::Wrap;
        self
    }

    /// Overflow policy.
    #[must_use]
    pub const fn overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Truncate end with ellipsis.
    #[must_use]
    pub const fn truncate(mut self) -> Self {
        self.overflow = TextOverflow::Truncate;
        self
    }

    /// Center.
    #[must_use]
    pub const fn center(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    /// Selectable / copy policy.
    #[must_use]
    pub const fn selectable(mut self, policy: SelectablePolicy) -> Self {
        self.selectable = policy;
        self
    }

    /// Mark copyable.
    #[must_use]
    pub const fn copyable(mut self) -> Self {
        self.selectable = SelectablePolicy::Copyable;
        self
    }

    /// Preserve terminal-default background (default true).
    #[must_use]
    pub const fn preserve_bg(mut self, preserve: bool) -> Self {
        self.preserve_bg = preserve;
        self
    }

    /// Selectable policy.
    #[must_use]
    pub const fn policy(&self) -> SelectablePolicy {
        self.selectable
    }
    /// Spans borrow.
    #[must_use]
    pub fn spans_ref(&self) -> &[TextSpan<'a>] {
        &self.spans
    }

    /// Copy-safe plain text (tabs expanded, controls stripped).
    #[must_use]
    pub fn plain(&self) -> String {
        let mut out = String::new();
        for span in &self.spans {
            out.push_str(&expand_tabs(span.content.as_ref(), self.tab_width));
        }
        out
    }

    fn resolve_style(&self, span: &TextSpan<'_>) -> Style {
        let role = match span.emphasis {
            TextEmphasis::Dim if matches!(span.role, Role::Text) => Role::TextMuted,
            _ => span.role,
        };
        let mut style = self.system.style(role);
        if self.preserve_bg {
            // Drop explicit background so canvas / parent surface shows through.
            style = ratatui_core::style::Style { bg: None, ..style };
        }
        match span.emphasis {
            TextEmphasis::Normal => {}
            TextEmphasis::Strong => {
                style = style.add_modifier(Modifier::BOLD);
            }
            TextEmphasis::Dim => {
                // "Quieter" is a ladder step, not a dimmed copy of the tone.
                style = style.patch(self.system.style(Role::TextMuted));
            }
            TextEmphasis::Code => {
                // Inline code reads through the syntax tone, keeping the
                // "no filled background" promise this widget makes.
                style = style.patch(self.system.style(Role::TextSecondary));
            }
        }
        if span.underline {
            style = style.add_modifier(Modifier::UNDERLINED);
        }
        if span.reverse {
            // Reversal is the explicit pair, applied whole.
            style = self.system.reversed();
        }
        if span.highlight {
            // Non-bg highlight: accent foreground plus weight.
            let accent = self.system.style(Role::Accent);
            if let Some(fg) = accent.fg {
                style = style.fg(fg);
            }
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    /// Layout into lines for `width` × `height` cells.
    #[must_use]
    pub fn layout(&self, width: u16, height: u16) -> TextLayout {
        let w = usize::from(width);
        let h = usize::from(height);
        let plain = self.plain();
        if w == 0 || h == 0 {
            return TextLayout {
                lines: Vec::new(),
                plain,
                truncated: !self.plain().is_empty(),
                natural_height: 0,
            };
        }

        // Expand each span independently, then either join for wrap/truncate or paint as runs.
        let expanded: Vec<(String, Style, Option<String>, bool)> = self
            .spans
            .iter()
            .map(|s| {
                (
                    expand_tabs(s.content.as_ref(), self.tab_width),
                    self.resolve_style(s),
                    s.annotation.as_ref().map(|a| a.to_string()),
                    s.highlight,
                )
            })
            .collect();

        match self.overflow {
            TextOverflow::Wrap => self.layout_wrap(&expanded, plain, w, h),
            TextOverflow::Clip => self.layout_single_line(&expanded, plain, w, h, None),
            TextOverflow::Truncate => {
                self.layout_single_line(&expanded, plain, w, h, Some(TruncateMode::End))
            }
            TextOverflow::TruncateStart => {
                self.layout_single_line(&expanded, plain, w, h, Some(TruncateMode::Start))
            }
            TextOverflow::TruncateMiddle => {
                self.layout_single_line(&expanded, plain, w, h, Some(TruncateMode::Middle))
            }
        }
    }

    fn layout_single_line(
        &self,
        expanded: &[(String, Style, Option<String>, bool)],
        plain: String,
        w: usize,
        h: usize,
        truncate: Option<TruncateMode>,
    ) -> TextLayout {
        // Join plain for truncation, but try to preserve first-span style for simple cases.
        let joined: String = expanded.iter().map(|(t, _, _, _)| t.as_str()).collect();
        let natural_h = 1u16;
        let (visible, truncated) = if let Some(mode) = truncate {
            let t = truncate_display_cols(&joined, w, mode, self.ellipsis.as_ref());
            let was = display_cols(&joined) > w;
            (t, was)
        } else {
            let t = take_display_cols(&joined, w).into_owned();
            let was = display_cols(&joined) > w;
            (t, was)
        };
        // Multi-span clip: walk spans until width exhausted (style-accurate).
        let mut segments = Vec::new();
        if expanded.len() == 1 || truncate.is_some() {
            let style = expanded
                .first()
                .map(|(_, s, _, _)| *s)
                .unwrap_or_else(|| self.system.style(Role::Text));
            let annotation = expanded.first().and_then(|(_, _, a, _)| a.clone());
            let highlight = expanded.first().is_some_and(|(_, _, _, h)| *h);
            if !visible.is_empty() {
                segments.push(TextSegment {
                    text: visible,
                    style,
                    annotation,
                    highlight,
                });
            }
        } else {
            let mut used = 0usize;
            for (text, style, annotation, highlight) in expanded {
                if used >= w {
                    break;
                }
                let remain = w - used;
                let piece = take_display_cols(text, remain).into_owned();
                let pw = display_cols(&piece);
                if piece.is_empty() && !text.is_empty() {
                    break;
                }
                if !piece.is_empty() {
                    segments.push(TextSegment {
                        text: piece,
                        style: *style,
                        annotation: annotation.clone(),
                        highlight: *highlight,
                    });
                    used += pw;
                }
            }
        }
        let line_w = segments.iter().map(|s| display_cols(&s.text)).sum();
        let lines = if h == 0 {
            Vec::new()
        } else {
            vec![TextLine {
                segments,
                width: line_w,
            }]
        };
        TextLayout {
            lines,
            plain,
            truncated,
            natural_height: natural_h,
        }
    }

    fn layout_wrap(
        &self,
        expanded: &[(String, Style, Option<String>, bool)],
        plain: String,
        w: usize,
        h: usize,
    ) -> TextLayout {
        // Wrap the joined plain for line breaks, then re-apply first span style for body text.
        // Multi-span wrap: wrap each span's text, concatenating lines carefully.
        // Strategy: treat as a stream of (grapheme cluster, style, annotation, highlight).
        let mut stream: Vec<(String, Style, Option<String>, bool)> = Vec::new();
        for (text, style, annotation, highlight) in expanded {
            for g in unicode_segmentation::UnicodeSegmentation::graphemes(text.as_str(), true) {
                if g.is_empty() {
                    continue;
                }
                stream.push((g.to_string(), *style, annotation.clone(), *highlight));
            }
        }

        let mut all_lines: Vec<TextLine> = Vec::new();
        let mut cur_segs: Vec<TextSegment> = Vec::new();
        let mut cur_w = 0usize;
        let mut cur_text = String::new();
        let mut cur_style: Option<Style> = None;
        let mut cur_ann: Option<String> = None;
        let mut cur_hl = false;

        let flush_run = |segs: &mut Vec<TextSegment>,
                         text: &mut String,
                         style: &mut Option<Style>,
                         ann: &mut Option<String>,
                         hl: &mut bool| {
            if text.is_empty() {
                return;
            }
            segs.push(TextSegment {
                text: std::mem::take(text),
                style: style.unwrap_or_default(),
                annotation: ann.clone(),
                highlight: *hl,
            });
        };

        let flush_line =
            |lines: &mut Vec<TextLine>, segs: &mut Vec<TextSegment>, width: &mut usize| {
                let w = segs.iter().map(|s| display_cols(&s.text)).sum();
                *width = 0;
                lines.push(TextLine {
                    segments: std::mem::take(segs),
                    width: w,
                });
            };

        for (g, style, ann, hl) in stream {
            let gw = display_cols(&g);
            if gw > w {
                // Flush current, place oversized alone.
                flush_run(
                    &mut cur_segs,
                    &mut cur_text,
                    &mut cur_style,
                    &mut cur_ann,
                    &mut cur_hl,
                );
                if !cur_segs.is_empty() {
                    flush_line(&mut all_lines, &mut cur_segs, &mut cur_w);
                }
                all_lines.push(TextLine {
                    segments: vec![TextSegment {
                        text: g,
                        style,
                        annotation: ann,
                        highlight: hl,
                    }],
                    width: gw,
                });
                cur_w = 0;
                continue;
            }
            if cur_w + gw > w && cur_w > 0 {
                flush_run(
                    &mut cur_segs,
                    &mut cur_text,
                    &mut cur_style,
                    &mut cur_ann,
                    &mut cur_hl,
                );
                flush_line(&mut all_lines, &mut cur_segs, &mut cur_w);
                cur_style = None;
            }
            // Same style run merge
            let same =
                cur_style == Some(style) && cur_ann == ann && cur_hl == hl && !cur_text.is_empty();
            if !same && !cur_text.is_empty() {
                flush_run(
                    &mut cur_segs,
                    &mut cur_text,
                    &mut cur_style,
                    &mut cur_ann,
                    &mut cur_hl,
                );
            }
            if cur_text.is_empty() {
                cur_style = Some(style);
                cur_ann = ann;
                cur_hl = hl;
            }
            cur_text.push_str(&g);
            cur_w += gw;
        }
        flush_run(
            &mut cur_segs,
            &mut cur_text,
            &mut cur_style,
            &mut cur_ann,
            &mut cur_hl,
        );
        if !cur_segs.is_empty() || all_lines.is_empty() {
            let width = cur_segs.iter().map(|s| display_cols(&s.text)).sum();
            all_lines.push(TextLine {
                segments: cur_segs,
                width,
            });
        }

        let natural_height = u16::try_from(all_lines.len()).unwrap_or(u16::MAX);
        let truncated = all_lines.len() > h;
        let lines: Vec<TextLine> = all_lines.into_iter().take(h).collect();
        TextLayout {
            lines,
            plain,
            truncated,
            natural_height,
        }
    }

    /// Natural wrapped height for `width` (no height cap).
    #[must_use]
    pub fn measure_height(&self, width: u16) -> u16 {
        self.layout(width, u16::MAX).natural_height.max(1)
    }

    /// Paint into `area` (does not fill background cells).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer) -> TextLayout {
        let layout = self.layout(area.width, area.height);
        if area.is_empty() {
            return layout;
        }
        for (row, line) in layout.lines.iter().enumerate() {
            let y = area.y.saturating_add(u16::try_from(row).unwrap_or(0));
            if y >= area.bottom() {
                break;
            }
            let line_w = line.width.min(usize::from(area.width));
            let start_x = match self.align {
                TextAlign::Start => area.x,
                TextAlign::Center => {
                    let pad = usize::from(area.width).saturating_sub(line_w) / 2;
                    area.x.saturating_add(u16::try_from(pad).unwrap_or(0))
                }
                TextAlign::End => {
                    let pad = usize::from(area.width).saturating_sub(line_w);
                    area.x.saturating_add(u16::try_from(pad).unwrap_or(0))
                }
            };
            let mut x = start_x;
            let right = area.right();
            for seg in &line.segments {
                if x >= right {
                    break;
                }
                let remain = usize::from(right.saturating_sub(x));
                let text = take_display_cols(&seg.text, remain);
                let tw = display_cols(&text);
                if text.is_empty() {
                    continue;
                }
                buffer.set_stringn(x, y, &text, remain, seg.style);
                x = x.saturating_add(u16::try_from(tw).unwrap_or(0));
            }
        }
        layout
    }
}

impl Widget for &Text<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let _ = self.paint(area, buffer);
    }
}

impl Widget for Text<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::display_cols;

    #[test]
    fn plain_strips_controls_and_expands_tabs() {
        let system = DesignSystem::default();
        let t = Text::new("a\tb\u{1b}c", &system);
        assert_eq!(t.plain(), "a   bc");
    }

    #[test]
    fn wrap_produces_multiple_lines() {
        let system = DesignSystem::default();
        let t = Text::new("hello world friends", &system).wrap();
        let layout = t.layout(5, 10);
        assert!(layout.lines.len() >= 3);
        assert!(layout.lines.iter().all(|l| l.width <= 5));
    }

    #[test]
    fn truncate_end_ellipsis() {
        let system = DesignSystem::default();
        let t = Text::new("abcdefghij", &system).truncate();
        let layout = t.layout(6, 1);
        assert!(layout.truncated);
        let plain = layout.lines[0].plain();
        assert!(plain.contains('…') || plain.contains('.'));
        assert!(display_cols(&plain) <= 6);
    }

    #[test]
    fn multi_span_roles() {
        let system = DesignSystem::default();
        let t = Text::spans(
            [
                TextSpan::new("ok").role(Role::Success).strong(),
                TextSpan::new(" · ").dim(),
                TextSpan::new("cached")
                    .role(Role::TextMuted)
                    .annotation("meta"),
            ],
            &system,
        );
        assert_eq!(t.plain(), "ok · cached");
        assert_eq!(t.spans_ref()[2].annotation_of(), Some("meta"));
        let layout = t.layout(40, 1);
        assert!(layout.lines[0].segments.len() >= 2);
    }

    #[test]
    fn highlight_and_annotation() {
        let system = DesignSystem::default();
        let t = Text::spans(
            [TextSpan::new("hit").highlight(true).annotation("search")],
            &system,
        );
        let layout = t.layout(10, 1);
        assert!(layout.lines[0].segments[0].highlight);
        assert_eq!(
            layout.lines[0].segments[0].annotation.as_deref(),
            Some("search")
        );
    }

    #[test]
    fn preserve_bg_clears_background() {
        let system = DesignSystem::default();
        // Input role normally has bg — Text should strip it when preserve_bg.
        let t = Text::new("x", &system).role(Role::Input).preserve_bg(true);
        let layout = t.layout(4, 1);
        assert!(layout.lines[0].segments[0].style.bg.is_none());
    }

    #[test]
    fn align_center_offsets_paint() {
        let system = DesignSystem::default();
        let t = Text::new("hi", &system).center();
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        let _ = t.paint(Rect::new(0, 0, 10, 1), &mut buf);
        // "hi" width 2, center pad 4 → starts at col 4
        assert_eq!(buf[(4, 0)].symbol(), "h");
    }

    #[test]
    fn cjk_emoji_combining() {
        let system = DesignSystem::default();
        let t = Text::new("日e\u{301}🧪", &system);
        assert_eq!(display_cols(&t.plain()), 2 + 1 + 2);
        let layout = t.layout(3, 1);
        assert!(layout.lines[0].width <= 3);
    }

    #[test]
    fn selectable_policy() {
        let system = DesignSystem::default();
        let t = Text::new("copy me", &system).copyable();
        assert!(t.policy().copyable());
        assert_eq!(t.plain(), "copy me");
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let t = Text::new("x", &system);
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let layout = t.paint(Rect::new(0, 0, 0, 0), &mut buf);
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn measure_height_wrap() {
        let system = DesignSystem::default();
        let t = Text::new("abcdefghij", &system).wrap();
        assert!(t.measure_height(3) >= 3);
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let t = Text::spans(
            [
                TextSpan::new("Status: ").role(Role::TextMuted),
                TextSpan::new("READY").role(Role::Success).strong(),
            ],
            &system,
        )
        .wrap();
        for _ in 0..20_000 {
            let _ = t.layout(40, 8);
        }
    }

    #[test]
    fn ids_stable() {
        assert_eq!(TextOverflow::Wrap.id(), "wrap");
        assert_eq!(TextAlign::Center.id(), "center");
        assert_eq!(TextEmphasis::Code.id(), "code");
    }
}
