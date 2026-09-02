// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! CodeBlock — production code / command rendering.
//!
//! Anatomy: `root` · optional `header` (language / path) · `gutter` · `body` lines.
//!
//! Hosts own source buffers and syntax engines. This widget:
//! - paints a **visible window** only (large-file virtualization via `line_base` +
//!   `logical_len` + [`CodeBlockState`] scroll);
//! - expands tabs and optionally **shows** control characters (caret notation);
//! - supports clip vs wrap, line numbers, highlight ranges, selection, copy;
//! - streams unfinished fences (`streaming` trailing cue);
//! - composes with diagnostics / diff / plan / terminal output via highlight
//!   marks, ANSI highlighter, and plain copy text.
//!
//! References: Glow, bat, delta, Rich Syntax, lazygit code panes.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

use crate::input::{KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, NavigationMove, PageMove, SemanticNode, SemanticRole, SemanticScene,
    SemanticState, UiIntent, default_list_intent,
};
use crate::style::{DesignSystem, Role, SyntaxTone, VisualState};
use crate::text::{
    display_cols, expand_tabs, is_terminal_control_char, take_display_cols, wrap_display_cols,
};

// ── Syntax ──────────────────────────────────────────────────────────────────

/// Caller-supplied syntax styling for one source line.
///
/// Segments should cover the full **display-prepared** line (tabs expanded /
/// controls rendered as visible glyphs when that policy is active). Highlighters
/// that tokenize raw source should re-apply the same preparation the widget
/// uses, or highlight against prepared text via [`prepare_code_display`].
pub trait SyntaxHighlighter {
    /// Styles a single prepared source line. Return styled segments covering the line.
    fn highlight_line<'line>(
        &self,
        line: &'line str,
        line_index: usize,
    ) -> Vec<(&'line str, Style)>;
}

/// Neutral highlighter — whole line as plain text role at paint time.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlainSyntax;

impl SyntaxHighlighter for PlainSyntax {
    fn highlight_line<'line>(
        &self,
        line: &'line str,
        _line_index: usize,
    ) -> Vec<(&'line str, Style)> {
        vec![(line, Style::default())]
    }
}

/// What a token *is*, before anyone decides how it looks.
///
/// The tokenizer used to encode "string" as green-plus-underline and the
/// role-aware highlighter decoded that back by sniffing modifiers, so a paint
/// change silently rewrote classification. The kind is the fact; each
/// highlighter maps it to its own presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CodeTokenKind {
    /// Ordinary source text / whitespace.
    #[default]
    Plain,
    /// Identifier (not a keyword, not a call).
    Ident,
    /// Line comment through end of line.
    Comment,
    /// Quoted string literal.
    String,
    /// Numeric literal.
    Number,
    /// Language keyword (host-supplied set).
    Keyword,
    /// Identifier immediately followed by `(`.
    Function,
    /// Operator (`+`, `=`, `->`, …).
    Operator,
    /// Punctuation (`()`, `{}`, `,`, `;`, `.`).
    Punct,
}

impl CodeTokenKind {
    /// Semantic role this kind paints through.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Plain | Self::Ident | Self::Function => Role::Text,
            Self::Comment => Role::SyntaxComment,
            Self::String => Role::SyntaxString,
            Self::Number => Role::SyntaxNumber,
            Self::Keyword => Role::SyntaxKeyword,
            Self::Operator | Self::Punct => Role::TextMuted,
        }
    }

    /// junie syntax class — weight + text ladder, never hue.
    #[must_use]
    pub const fn syntax_tone(self) -> SyntaxTone {
        match self {
            Self::Keyword => SyntaxTone::Keyword,
            Self::Ident | Self::Function => SyntaxTone::Ident,
            Self::String => SyntaxTone::Str,
            Self::Number => SyntaxTone::Number,
            Self::Operator => SyntaxTone::Operator,
            Self::Punct => SyntaxTone::Punct,
            Self::Comment => SyntaxTone::Comment,
            Self::Plain => SyntaxTone::Plain,
        }
    }
}

/// ANSI-aware highlighter marker for terminal-captured output composition.
///
/// CSI SGR segments own their text inside [`crate::ansi_text::styled_spans`], so
/// this highlighter paints the full line with the default style. Prefer host
/// pre-parse into owned styled runs, or use [`RoleTokenSyntax`] on stripped
/// text. Kept as a stable composition hook for TerminalOutput hosts.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnsiSyntax;

impl SyntaxHighlighter for AnsiSyntax {
    fn highlight_line<'line>(
        &self,
        line: &'line str,
        _line_index: usize,
    ) -> Vec<(&'line str, Style)> {
        // Detect ANSI but cannot re-borrow owned span content; paint plain.
        // Hosts that need SGR fidelity should expand via ansi_text before
        // CodeBlock or provide a custom highlighter with owned styles.
        let _ = line.as_bytes().contains(&0x1b);
        vec![(line, Style::default())]
    }
}

// ── Policies ────────────────────────────────────────────────────────────────

/// Horizontal overflow policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CodeWrap {
    /// Clip to width; use horizontal scroll (`scroll_x`).
    #[default]
    Clip,
    /// Soft-wrap at display columns; continuation rows share the same gutter number.
    Wrap,
}

impl CodeWrap {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Wrap => "wrap",
        }
    }
}

/// How tabs and control characters become display cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ControlRender {
    /// Expand tabs; show other C0/DEL as caret notation (`^C`, `^?`).
    #[default]
    Visible,
    /// Expand tabs; drop other controls (clipboard-safe display).
    ExpandTabs,
    /// Expand tabs; replace other controls with `·` (U+00B7).
    Placeholder,
}

impl ControlRender {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::ExpandTabs => "expand-tabs",
            Self::Placeholder => "placeholder",
        }
    }
}

/// Kind of line / column highlight overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CodeHighlightKind {
    /// Selection (primary).
    #[default]
    Selection,
    /// Search / find match.
    Search,
    /// Diagnostic underline band (compose with [`super::Diagnostic`] /
    /// [`super::diagnostics_to_highlights`]).
    Diagnostic,
    /// Emphasis / plan step / review pin.
    Emphasis,
    /// Diff-added tint (compose with DiffReview).
    DiffAdd,
    /// Diff-removed tint.
    DiffRemove,
}

impl CodeHighlightKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::Search => "search",
            Self::Diagnostic => "diagnostic",
            Self::Emphasis => "emphasis",
            Self::DiffAdd => "diff-add",
            Self::DiffRemove => "diff-remove",
        }
    }

    fn role(self) -> Role {
        match self {
            // A selected range washes; the code keeps its syntax tones.
            Self::Selection => Role::SelectionTint,
            Self::Search | Self::Emphasis => Role::Accent,
            Self::Diagnostic => Role::Warning,
            Self::DiffAdd => Role::DiffAdded,
            Self::DiffRemove => Role::DiffRemoved,
        }
    }
}

/// Highlight on one logical line (optional display-column span).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodeHighlight {
    /// 0-based absolute logical line.
    pub line: usize,
    /// Inclusive start display column in prepared text (None = whole line).
    pub start_col: Option<u16>,
    /// Exclusive end display column (None = end of line / whole).
    pub end_col: Option<u16>,
    /// Overlay kind.
    pub kind: CodeHighlightKind,
}

impl CodeHighlight {
    /// Whole-line highlight.
    #[must_use]
    pub const fn line(line: usize, kind: CodeHighlightKind) -> Self {
        Self {
            line,
            start_col: None,
            end_col: None,
            kind,
        }
    }

    /// Column span on a line.
    #[must_use]
    pub const fn span(line: usize, start: u16, end: u16, kind: CodeHighlightKind) -> Self {
        Self {
            line,
            start_col: Some(start),
            end_col: Some(end),
            kind,
        }
    }
}

/// Neutral gutter mark (diagnostics, plan pins, breakpoints).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeGutterMark {
    /// 0-based absolute logical line.
    pub line: usize,
    /// Single-cell mark glyph (host picks ASCII-safe when needed).
    pub glyph: char,
    /// Semantic role for the mark.
    pub role: Role,
}

impl CodeGutterMark {
    /// Mark constructor.
    #[must_use]
    pub const fn new(line: usize, glyph: char, role: Role) -> Self {
        Self { line, glyph, role }
    }
}

/// Source metadata chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CodeSourceMeta<'a> {
    /// Language / fence tag (`rust`, `bash`).
    pub language: Option<&'a str>,
    /// Path or resource label.
    pub path: Option<&'a str>,
    /// 1-based display offset for first logical line number (default 1).
    pub start_line_number: usize,
}

impl<'a> CodeSourceMeta<'a> {
    /// Empty meta.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            language: None,
            path: None,
            start_line_number: 1,
        }
    }

    /// Language only.
    #[must_use]
    pub const fn language(language: &'a str) -> Self {
        Self {
            language: Some(language),
            path: None,
            start_line_number: 1,
        }
    }

    /// Path + language.
    #[must_use]
    pub const fn with_path(mut self, path: &'a str) -> Self {
        self.path = Some(path);
        self
    }

    /// 1-based starting line number for gutter.
    #[must_use]
    pub const fn start_line_number(mut self, n: usize) -> Self {
        self.start_line_number = if n == 0 { 1 } else { n };
        self
    }

    /// Header text when space allows.
    #[must_use]
    pub fn header_text(&self) -> String {
        match (self.path, self.language) {
            (Some(p), Some(l)) => format!("{p} · {l}"),
            (Some(p), None) => p.to_string(),
            (None, Some(l)) => l.to_string(),
            (None, None) => String::new(),
        }
    }
}

// ── Display prepare ─────────────────────────────────────────────────────────

/// Expand tabs / render controls for code display.
///
/// Unlike prose [`expand_tabs`], **Visible** keeps control identity as caret
/// notation so binary-ish or log-tainted snippets stay inspectable (bat-style).
#[must_use]
pub fn prepare_code_display(s: &str, tab_width: usize, controls: ControlRender) -> String {
    let tab_w = if tab_width == 0 { 4 } else { tab_width };
    match controls {
        ControlRender::ExpandTabs => expand_tabs(s, tab_w),
        ControlRender::Placeholder => {
            let mut out = String::with_capacity(s.len());
            let mut col = 0usize;
            for c in s.chars() {
                if c == '\t' {
                    let spaces = tab_w - (col % tab_w);
                    for _ in 0..spaces {
                        out.push(' ');
                    }
                    col += spaces;
                    continue;
                }
                if is_terminal_control_char(c) {
                    out.push('·');
                    col += 1;
                    continue;
                }
                out.push(c);
                col += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            }
            out
        }
        ControlRender::Visible => {
            let mut out = String::with_capacity(s.len().saturating_mul(2));
            let mut col = 0usize;
            for c in s.chars() {
                if c == '\t' {
                    let spaces = tab_w - (col % tab_w);
                    for _ in 0..spaces {
                        out.push(' ');
                    }
                    col += spaces;
                    continue;
                }
                if c == '\n' || c == '\r' {
                    // Line-oriented widget — treat as placeholder.
                    out.push('·');
                    col += 1;
                    continue;
                }
                if is_terminal_control_char(c) {
                    let (a, b) = caret_notation(c);
                    out.push(a);
                    out.push(b);
                    col += 2;
                    continue;
                }
                out.push(c);
                col += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            }
            out
        }
    }
}

fn caret_notation(c: char) -> (char, char) {
    let u = c as u32;
    if c == '\u{7f}' {
        return ('^', '?');
    }
    if u < 0x20 {
        // ASCII C0 → ^@..^_
        let letter = char::from_u32(u + 64).unwrap_or('?');
        return ('^', letter);
    }
    // C1 and other controls
    ('^', '?')
}

// ── State / parts / outcomes ────────────────────────────────────────────────

/// Interaction + scroll state for a code block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockState {
    /// First visible absolute logical line (0-based).
    pub scroll_y: usize,
    /// Horizontal offset in display columns when wrap is Clip.
    pub scroll_x: u16,
    /// Focused keyboard cursor line (absolute).
    pub cursor_line: Option<usize>,
    /// Cursor column in display cells (0-based), for the `ln · col` footer.
    pub cursor_col: usize,
    /// Inclusive start / exclusive end absolute line selection.
    pub selection: Option<(usize, usize)>,
    /// Keyboard focus owner.
    pub focused: bool,
    /// Document is being edited (cursor line wears the field underline).
    pub editing: bool,
    /// Hovered absolute line.
    pub hovered_line: Option<usize>,
    /// Last painted geometry.
    pub parts: Option<CodeBlockParts>,
    /// Acknowledgement owed after a copy fired.
    pub copied: crate::style::ActionFlash,
    /// Viewport body height in rows (set by paint).
    viewport_rows: u16,
    /// Body text width in columns (set by paint).
    body_width: u16,
}

impl Default for CodeBlockState {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeBlockState {
    /// Fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scroll_y: 0,
            scroll_x: 0,
            cursor_line: None,
            cursor_col: 0,
            selection: None,
            focused: false,
            editing: false,
            hovered_line: None,
            parts: None,
            copied: crate::style::ActionFlash::new(),
            viewport_rows: 0,
            body_width: 0,
        }
    }

    /// Seed vertical scroll (virtualization host window).
    #[must_use]
    pub const fn with_scroll_y(mut self, y: usize) -> Self {
        self.scroll_y = y;
        self
    }

    /// Focus flag.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Editing flag (cursor-line underline).
    pub const fn set_editing(&mut self, on: bool) {
        self.editing = on;
    }

    /// Cursor line.
    pub const fn set_cursor_line(&mut self, line: Option<usize>) {
        self.cursor_line = line;
    }

    /// Cursor column (0-based display cells).
    pub const fn set_cursor_col(&mut self, col: usize) {
        self.cursor_col = col;
    }

    /// Select exclusive-end line range.
    pub const fn set_selection(&mut self, range: Option<(usize, usize)>) {
        self.selection = range;
    }

    /// Max scroll_y for `logical_len` lines and current viewport.
    #[must_use]
    pub fn max_scroll_y(&self, logical_len: usize) -> usize {
        let vh = usize::from(self.viewport_rows.max(1));
        logical_len.saturating_sub(vh)
    }

    /// Clamp scroll to document.
    pub fn clamp(&mut self, logical_len: usize) {
        let max_y = self.max_scroll_y(logical_len);
        if self.scroll_y > max_y {
            self.scroll_y = max_y;
        }
        if let Some(c) = self.cursor_line {
            if logical_len == 0 {
                self.cursor_line = None;
            } else if c >= logical_len {
                self.cursor_line = Some(logical_len - 1);
            }
        }
    }

    /// Scroll by line delta; returns whether changed.
    pub fn scroll_by_lines(&mut self, delta: isize, logical_len: usize) -> bool {
        let before = self.scroll_y;
        if delta >= 0 {
            self.scroll_y = self
                .scroll_y
                .saturating_add(usize::try_from(delta).unwrap_or(usize::MAX));
        } else {
            self.scroll_y = self
                .scroll_y
                .saturating_sub(usize::try_from(-delta).unwrap_or(usize::MAX));
        }
        self.clamp(logical_len);
        before != self.scroll_y
    }

    /// Horizontal scroll.
    pub fn scroll_by_cols(&mut self, delta: i16) -> bool {
        let before = self.scroll_x;
        if delta >= 0 {
            self.scroll_x = self
                .scroll_x
                .saturating_add(u16::try_from(delta).unwrap_or(u16::MAX));
        } else {
            self.scroll_x = self
                .scroll_x
                .saturating_sub(u16::try_from(-delta).unwrap_or(u16::MAX));
        }
        before != self.scroll_x
    }

    /// Reveal absolute line in viewport.
    pub fn reveal_line(&mut self, line: usize, logical_len: usize) {
        let vh = usize::from(self.viewport_rows.max(1));
        if line < self.scroll_y {
            self.scroll_y = line;
        } else if line >= self.scroll_y.saturating_add(vh) {
            self.scroll_y = line.saturating_add(1).saturating_sub(vh);
        }
        self.clamp(logical_len);
    }
}

/// Painted geometry for hit testing / composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockParts {
    /// Full widget area used.
    pub root: Rect,
    /// Language / path header (may be empty height 0).
    pub header: Rect,
    /// Line-number gutter.
    pub gutter: Rect,
    /// Source body.
    pub body: Rect,
    /// First absolute logical line painted.
    pub first_line: usize,
    /// Number of logical lines that contributed rows.
    pub visible_lines: usize,
    /// Whether streaming cue was painted.
    pub streaming: bool,
}

/// Outcomes for host effects.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CodeBlockOutcome {
    /// No change.
    Ignored,
    /// Scroll offsets changed.
    Scrolled {
        /// Vertical first line.
        scroll_y: usize,
        /// Horizontal columns.
        scroll_x: u16,
    },
    /// Cursor moved.
    CursorMoved {
        /// Absolute line.
        line: usize,
    },
    /// Line activated (Enter / double policy).
    LineActivated {
        /// Absolute line.
        line: usize,
    },
    /// Selection updated.
    SelectionChanged {
        /// Inclusive start, exclusive end.
        range: (usize, usize),
    },
    /// Copy request — host emits clipboard / OSC 52.
    Copy {
        /// Prepared plain text (tabs expanded; controls stripped for safety).
        text: String,
    },
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Production code listing.
#[derive(Debug, Clone, Copy)]
pub struct CodeBlock<'a, H: SyntaxHighlighter = PlainSyntax> {
    lines: &'a [&'a str],
    /// Absolute index of `lines[0]` in the logical document.
    line_base: usize,
    /// Total logical line count when virtualizing; `None` ⇒ `line_base + lines.len()`.
    logical_len: Option<usize>,
    meta: CodeSourceMeta<'a>,
    show_line_numbers: bool,
    highlighter: &'a H,
    system: &'a DesignSystem,
    wrap: CodeWrap,
    tab_width: u8,
    controls: ControlRender,
    highlights: &'a [CodeHighlight],
    gutter_marks: &'a [CodeGutterMark],
    /// Unfinished fence / streaming append.
    streaming: bool,
    /// Legacy first-line when painting without state.
    first_line: usize,
    /// Absolute inclusive-start / exclusive-end of the current statement block.
    /// `›` is painted on the first line; `▎` is focus-only.
    current_block: Option<(usize, usize)>,
}

impl<'a> CodeBlock<'a, PlainSyntax> {
    /// Plain code block (no line numbers).
    #[must_use]
    pub const fn new(lines: &'a [&'a str], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            line_base: 0,
            logical_len: None,
            meta: CodeSourceMeta::new(),
            show_line_numbers: false,
            highlighter: &PlainSyntax,
            system,
            wrap: CodeWrap::Clip,
            tab_width: 4,
            controls: ControlRender::Visible,
            highlights: &[],
            gutter_marks: &[],
            streaming: false,
            first_line: 0,
            current_block: None,
        }
    }
}

impl<'a, H: SyntaxHighlighter> CodeBlock<'a, H> {
    /// Language label (header when space allows).
    #[must_use]
    pub const fn language(mut self, language: &'a str) -> Self {
        self.meta.language = Some(language);
        self
    }

    /// Source path metadata.
    #[must_use]
    pub const fn path(mut self, path: &'a str) -> Self {
        self.meta.path = Some(path);
        self
    }

    /// Full metadata.
    #[must_use]
    pub const fn meta(mut self, meta: CodeSourceMeta<'a>) -> Self {
        self.meta = meta;
        self
    }

    /// Gutter line numbers.
    #[must_use]
    pub const fn line_numbers(mut self, enabled: bool) -> Self {
        self.show_line_numbers = enabled;
        self
    }

    /// Legacy first visible line (Widget path / seed). Prefer [`CodeBlockState::scroll_y`].
    #[must_use]
    pub const fn first_line(mut self, first_line: usize) -> Self {
        self.first_line = first_line;
        self
    }

    /// Absolute index of `lines[0]` when the host windows the buffer.
    #[must_use]
    pub const fn line_base(mut self, base: usize) -> Self {
        self.line_base = base;
        self
    }

    /// Logical document length for scroll clamp (virtualization).
    #[must_use]
    pub const fn logical_len(mut self, len: usize) -> Self {
        self.logical_len = Some(len);
        self
    }

    /// Custom highlighter (may change highlighter type).
    #[must_use]
    pub const fn highlighter<H2: SyntaxHighlighter>(
        self,
        highlighter: &'a H2,
    ) -> CodeBlock<'a, H2> {
        CodeBlock {
            lines: self.lines,
            line_base: self.line_base,
            logical_len: self.logical_len,
            meta: self.meta,
            show_line_numbers: self.show_line_numbers,
            highlighter,
            system: self.system,
            wrap: self.wrap,
            tab_width: self.tab_width,
            controls: self.controls,
            highlights: self.highlights,
            gutter_marks: self.gutter_marks,
            streaming: self.streaming,
            first_line: self.first_line,
            current_block: self.current_block,
        }
    }

    /// Wrap policy.
    #[must_use]
    pub const fn wrap(mut self, wrap: CodeWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// Tab stop width (0 → 4).
    #[must_use]
    pub const fn tab_width(mut self, width: u8) -> Self {
        self.tab_width = width;
        self
    }

    /// Control-character render policy.
    #[must_use]
    pub const fn controls(mut self, controls: ControlRender) -> Self {
        self.controls = controls;
        self
    }

    /// Overlay highlights (selection / search / diagnostic / diff tint).
    #[must_use]
    pub const fn highlights(mut self, highlights: &'a [CodeHighlight]) -> Self {
        self.highlights = highlights;
        self
    }

    /// Gutter marks for diagnostics / plan pins.
    #[must_use]
    pub const fn gutter_marks(mut self, marks: &'a [CodeGutterMark]) -> Self {
        self.gutter_marks = marks;
        self
    }

    /// Streaming / unfinished fence cue.
    #[must_use]
    pub const fn streaming(mut self, on: bool) -> Self {
        self.streaming = on;
        self
    }

    /// Current statement block (absolute lines, exclusive end). Marker `›`
    /// lands on the first line; the focus bar `▎` is independent.
    #[must_use]
    pub const fn current_block(mut self, start: usize, end: usize) -> Self {
        self.current_block = Some((start, if end < start { start } else { end }));
        self
    }

    /// Resolved logical length.
    #[must_use]
    pub fn document_len(&self) -> usize {
        self.logical_len
            .unwrap_or_else(|| self.line_base.saturating_add(self.lines.len()))
    }

    /// Absolute line → index into `lines`, if present in this window.
    #[must_use]
    pub fn window_index(&self, absolute: usize) -> Option<usize> {
        absolute
            .checked_sub(self.line_base)
            .filter(|i| *i < self.lines.len())
    }

    /// Clipboard-safe plain text for a range of absolute lines (controls stripped).
    #[must_use]
    pub fn copy_range(&self, start: usize, end: usize) -> String {
        let end = end.min(self.document_len());
        let start = start.min(end);
        let tab = usize::from(self.tab_width);
        let mut out = String::new();
        for abs in start..end {
            if let Some(i) = self.window_index(abs) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&expand_tabs(self.lines[i], tab));
            }
        }
        out
    }

    /// Paints the copy acknowledgement at the trailing edge of the header.
    ///
    /// The mark is a fact, so a tier that forbids motion still shows it; only
    /// the fade is a transition.
    fn paint_copy_flash(&self, header: Rect, buffer: &mut Buffer, state: &CodeBlockState) {
        let elapsed = self.system.elapsed_ms();
        if header.height == 0 || !state.copied.is_lit(elapsed) {
            return;
        }
        let mark = self
            .system
            .glyphs
            .resolve(crate::style::Glyph::Success)
            .text;
        let width = u16::try_from(crate::text::display_cols(mark)).unwrap_or(1);
        if header.width <= width {
            return;
        }
        let style = self.system.style(Role::Success);
        buffer.set_stringn(
            header.right().saturating_sub(width),
            header.y,
            mark,
            usize::from(width),
            style,
        );
    }

    /// Copy selection or full window.
    #[must_use]
    pub fn copy_text(&self, state: &CodeBlockState) -> String {
        if let Some((a, b)) = state.selection {
            return self.copy_range(a, b);
        }
        if let Some(c) = state.cursor_line {
            return self.copy_range(c, c.saturating_add(1));
        }
        self.copy_range(
            self.line_base,
            self.line_base.saturating_add(self.lines.len()),
        )
    }

    fn number_width(&self, body_rows: u16, first: usize) -> u16 {
        let last = first.saturating_add(usize::from(body_rows)).max(1);
        let display_last = self
            .meta
            .start_line_number
            .saturating_add(last.saturating_sub(1));
        u16::try_from(display_last.max(1).to_string().len().max(2)).unwrap_or(2)
    }

    fn gutter_width(&self, body_rows: u16, first: usize) -> u16 {
        // junie: bar(1) marker(1) space(1) numbers(num_w) space(1)
        if self.show_line_numbers {
            return 1 + 1 + self.number_width(body_rows, first) + 1 + 1;
        }
        if !self.gutter_marks.is_empty() || self.current_block.is_some() {
            2
        } else {
            0
        }
    }

    fn resolved_block(&self, state: &CodeBlockState) -> Option<(usize, usize)> {
        if let Some(b) = self.current_block {
            return Some(b);
        }
        state.cursor_line.map(|line| (line, line.saturating_add(1)))
    }

    fn mark_for(&self, abs_line: usize) -> Option<&CodeGutterMark> {
        self.gutter_marks.iter().find(|m| m.line == abs_line)
    }

    fn highlights_for(&self, abs_line: usize, state: &CodeBlockState) -> Vec<CodeHighlightKind> {
        let mut kinds = Vec::new();
        if let Some((a, b)) = state.selection
            && abs_line >= a
            && abs_line < b
        {
            kinds.push(CodeHighlightKind::Selection);
        }
        // Focus is the `▎` bar, never a second current-line wash.
        for h in self.highlights {
            if h.line == abs_line {
                kinds.push(h.kind);
            }
        }
        kinds
    }

    fn line_style_overlay(
        &self,
        base: Style,
        kinds: &[CodeHighlightKind],
        monochrome_syntax: bool,
    ) -> Style {
        let mut style = base;
        for kind in kinds {
            let role_style = self.system.style(kind.role());
            if let Some(fg) = role_style.fg {
                style = style.fg(fg);
            }
            // Prefer non-fill cues: a diagnostic keeps the squiggle
            // substitute; everything else reads through weight or ground.
            match kind {
                CodeHighlightKind::Diagnostic => {
                    // Error underline — never a hue wash on the line.
                    let theme = self.system.junie_theme();
                    style = style
                        .add_modifier(Modifier::UNDERLINED)
                        .underline_color(theme.error);
                }
                CodeHighlightKind::Search => {
                    style = style.add_modifier(Modifier::BOLD);
                    // junie: a find match sits on the selection ground.
                    if let Some(bg) = self.system.style(Role::Selection).bg {
                        style = style.bg(bg);
                    }
                }
                CodeHighlightKind::Selection | CodeHighlightKind::Emphasis => {
                    style = style.add_modifier(Modifier::BOLD);
                    if let Some(bg) = role_style.bg {
                        style = style.bg(bg);
                    }
                }
                CodeHighlightKind::DiffAdd | CodeHighlightKind::DiffRemove => {
                    if let Some(bg) = role_style.bg {
                        style = style.bg(bg);
                    } else {
                        style = style.add_modifier(Modifier::BOLD);
                    }
                }
            }
        }
        if monochrome_syntax && style.add_modifier == Modifier::empty() {
            // Ensure colorless path still differentiates via underline on keywords
            // when highlighter returns Syntax roles that quantize to same gray.
            let _ = monochrome_syntax;
        }
        style
    }

    fn is_monochrome(&self) -> bool {
        matches!(
            self.system.capability,
            crate::style::ColorCapability::Monochrome
        )
    }

    /// Layout geometry without painting body text.
    #[must_use]
    pub fn layout(&self, area: Rect, state: &CodeBlockState) -> CodeBlockParts {
        if area.is_empty() {
            return CodeBlockParts {
                root: area,
                header: Rect::default(),
                gutter: Rect::default(),
                body: Rect::default(),
                first_line: state.scroll_y,
                visible_lines: 0,
                streaming: false,
            };
        }
        let header_text = self.meta.header_text();
        let show_header = !header_text.is_empty() && area.height >= 2;
        let header = if show_header {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            }
        } else {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 0,
            }
        };
        let content_y = area.y.saturating_add(header.height);
        let content_h = area.height.saturating_sub(header.height);
        // junie CodeEditor keeps one footer row (`1–22 of 26` / ln·col)
        // when there is room for a body line plus that row.
        let body_h = if content_h > 1 {
            content_h.saturating_sub(1)
        } else {
            content_h
        };
        let first = if state.parts.is_some() || state.viewport_rows > 0 {
            state.scroll_y
        } else {
            state.scroll_y.max(self.first_line)
        };
        let gutter_w = self.gutter_width(body_h, first);
        let gutter = Rect {
            x: area.x,
            y: content_y,
            width: gutter_w.min(area.width),
            height: body_h,
        };
        let v_scroll =
            crate::scroll::is_scrollable(self.document_len(), usize::from(body_h).max(1));
        let body = Rect {
            x: area.x.saturating_add(gutter.width),
            y: content_y,
            width: area
                .width
                .saturating_sub(gutter.width)
                .saturating_sub(u16::from(v_scroll)),
            height: body_h,
        };
        CodeBlockParts {
            root: area,
            header,
            gutter,
            body,
            first_line: first,
            visible_lines: 0,
            streaming: self.streaming,
        }
    }

    /// Paint and update state geometry.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut CodeBlockState,
    ) -> CodeBlockParts {
        let mut parts = self.layout(area, state);
        if area.is_empty() {
            state.parts = Some(parts.clone());
            state.viewport_rows = 0;
            state.body_width = 0;
            return parts;
        }
        state.viewport_rows = parts.body.height;
        state.body_width = parts.body.width;
        state.clamp(self.document_len());
        parts.first_line = state.scroll_y.max(self.first_line);
        // Prefer state.scroll_y after clamp
        parts.first_line = state.scroll_y;

        let header_text = self.meta.header_text();
        if parts.header.height > 0 && !header_text.is_empty() {
            let h = take_display_cols(&header_text, usize::from(parts.header.width));
            buffer.set_stringn(
                parts.header.x,
                parts.header.y,
                &h,
                usize::from(parts.header.width),
                self.system.style(Role::TextMuted),
            );
        }
        self.paint_copy_flash(parts.header, buffer, state);

        let theme = self.system.junie_theme();
        let visual = VisualState {
            focused: state.focused,
            ..VisualState::default()
        };
        let fs = theme.field_style(visual);
        let field_bg = fs.bg.unwrap_or(theme.field);
        // Source CodeEditor fills the whole editor rect (scrollbar column and
        // footer included) with field_style. Restricting the well to
        // gutter+body left ┃ on the card surface.
        let fill_y = area.y.saturating_add(parts.header.height);
        let well = Rect::new(
            area.x,
            fill_y,
            area.width,
            area.bottom().saturating_sub(fill_y),
        );
        if !well.is_empty() {
            buffer.set_style(well, fs);
        }
        let block = self.resolved_block(state);

        let mono = self.is_monochrome();
        let tab = usize::from(self.tab_width);
        let mut row = 0u16;
        let mut visible = 0usize;
        let first = parts.first_line;
        let body_h = parts.body.height;
        let body_w = parts.body.width;

        // Absolute line walk — only lines present in window paint content.
        let mut abs = first;
        while row < body_h {
            let Some(win_i) = self.window_index(abs) else {
                // Outside provided window — blank row (host should refill window).
                abs = abs.saturating_add(1);
                if abs >= self.document_len() && abs > first.saturating_add(usize::from(body_h)) {
                    break;
                }
                if abs > first.saturating_add(10_000) {
                    break;
                }
                row = row.saturating_add(1);
                continue;
            };
            let raw = self.lines[win_i];
            let prepared = prepare_code_display(raw, tab, self.controls);
            let kinds = self.highlights_for(abs, state);

            // Junie: every row keeps the gutter slot (`▎`). The cursor line
            // uses the focus bar colour; other rows paint fg=bg so the glyph
            // is still in the cell (txt goldens) but invisible. Marker is `›`
            // for the current block or `!` for a diagnostic — never a second `▎`.
            if parts.gutter.width > 0 {
                let y = parts.body.y.saturating_add(row);
                let gx = parts.gutter.x;
                let line_gutter = self.system.gutter(
                    VisualState {
                        focused: state.focused,
                        ..visual
                    },
                    field_bg,
                    false,
                );
                buffer.set_stringn(gx, y, self.system.glyphs.selection_gutter(), 1, line_gutter);
                if let Some(m) = self.mark_for(abs) {
                    // Diagnostic `!` is bold; running spinner is accent only.
                    let mut mark_style = fs.patch(self.system.style(m.role));
                    if m.glyph == '!' {
                        mark_style = mark_style.add_modifier(Modifier::BOLD);
                    }
                    buffer.set_stringn(gx.saturating_add(1), y, m.glyph.to_string(), 1, mark_style);
                } else if let Some((start, end)) = block
                    && abs == start
                    && abs < end
                {
                    buffer.set_stringn(
                        gx.saturating_add(1),
                        y,
                        self.system.glyphs.selection_marker(),
                        1,
                        fs.fg(if state.focused {
                            theme.accent
                        } else {
                            theme.text_secondary
                        }),
                    );
                }
                if self.show_line_numbers && parts.gutter.width > 3 {
                    // bar + marker, then right-aligned numbers, then trailing spaces.
                    // Captured shots: `▎› 1  //` and `▎ 10 ` (numbers at x+2).
                    let num_w = parts.gutter.width.saturating_sub(4);
                    let display_n = self.meta.start_line_number.saturating_add(abs);
                    let number =
                        format!("{:>width$}", display_n, width = usize::from(num_w.max(1)));
                    let in_block = block.is_some_and(|(start, end)| abs >= start && abs < end);
                    let nstyle = if state.cursor_line == Some(abs) && state.focused {
                        fs.fg(theme.text_primary).add_modifier(Modifier::BOLD)
                    } else if in_block {
                        fs.fg(theme.text_secondary)
                    } else {
                        fs.fg(theme.text_muted)
                    };
                    buffer.set_stringn(
                        gx.saturating_add(2),
                        y,
                        &number,
                        usize::from(num_w),
                        nstyle,
                    );
                }
            }

            // Body rows for this logical line
            let display_rows: Vec<String> = match self.wrap {
                CodeWrap::Clip => {
                    let sliced = horizontal_slice(&prepared, state.scroll_x, body_w);
                    vec![sliced]
                }
                CodeWrap::Wrap => {
                    if body_w == 0 {
                        vec![String::new()]
                    } else {
                        wrap_display_cols(&prepared, usize::from(body_w))
                    }
                }
            };

            for (wrap_i, display_row) in display_rows.iter().enumerate() {
                if row >= body_h {
                    break;
                }
                // Only number first wrap row was already painted; continuation blank gutter
                if wrap_i > 0 && parts.gutter.width > 0 {
                    // leave gutter as-is (already empty for this row)
                }
                self.paint_body_row(
                    buffer,
                    parts.body.x,
                    parts.body.y.saturating_add(row),
                    body_w,
                    display_row,
                    &prepared,
                    abs,
                    &kinds,
                    mono,
                    fs,
                    display_cols(&prepared).saturating_sub(usize::from(state.scroll_x))
                        > usize::from(body_w),
                );
                if wrap_i == 0 && state.editing && state.cursor_line == Some(abs) {
                    let y = parts.body.y.saturating_add(row);
                    for x in parts.body.x..parts.body.right() {
                        if let Some(cell) = buffer.cell_mut((x, y)) {
                            cell.set_style(
                                cell.style()
                                    .add_modifier(Modifier::UNDERLINED)
                                    .underline_color(theme.border_strong),
                            );
                        }
                    }
                }
                row = row.saturating_add(1);
            }
            visible = visible.saturating_add(1);
            abs = abs.saturating_add(1);
            if abs >= self.line_base.saturating_add(self.lines.len()) && abs >= self.document_len()
            {
                break;
            }
        }

        if self.streaming && row < body_h && parts.body.width > 0 {
            let cue = "…";
            buffer.set_stringn(
                parts.body.x,
                parts.body.y.saturating_add(row),
                cue,
                usize::from(parts.body.width),
                self.system.style(Role::TextMuted),
            );
        }

        let fy = parts.body.y.saturating_add(parts.body.height);
        if fy < area.bottom() {
            let doc = self.document_len();
            let pos = if state.focused {
                let line = state.cursor_line.unwrap_or(0).saturating_add(1);
                let col = state.cursor_col.saturating_add(1);
                format!("ln {line}/{doc} · col {col}")
            } else if crate::scroll::is_scrollable(doc, usize::from(body_h).max(1)) {
                let first = state.scroll_y.saturating_add(1);
                let last = (state.scroll_y.saturating_add(usize::from(body_h))).min(doc);
                if last >= first && doc > 0 {
                    format!("{first}–{last} of {doc}")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            if !pos.is_empty() {
                let theme = self.system.junie_theme();
                let w = display_cols(&pos) as u16;
                let x = area.right().saturating_sub(w.saturating_add(1));
                buffer.set_stringn(
                    x.max(area.x),
                    fy,
                    &pos,
                    usize::from(area.width),
                    theme.faint().bg(theme.field),
                );
            }
        }

        if crate::scroll::is_scrollable(self.document_len(), usize::from(body_h).max(1)) {
            crate::scroll::paint_overflow_scrollbar(
                buffer,
                Rect::new(
                    area.right().saturating_sub(1),
                    parts.body.y,
                    1,
                    parts.body.height,
                ),
                self.document_len(),
                usize::from(body_h).max(1),
                u16::try_from(state.scroll_y).unwrap_or(u16::MAX),
                state.focused,
                self.system,
            );
        }

        parts.visible_lines = visible;
        parts.streaming = self.streaming;
        state.parts = Some(parts.clone());
        parts
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_body_row(
        &self,
        buffer: &mut Buffer,
        x: u16,
        y: u16,
        width: u16,
        display_row: &str,
        prepared_full: &str,
        abs_line: usize,
        kinds: &[CodeHighlightKind],
        mono: bool,
        field: Style,
        overflow: bool,
    ) {
        if width == 0 {
            return;
        }
        // Highlight against full prepared line; paint only display_row segment styles.
        let segments = self.highlighter.highlight_line(prepared_full, abs_line);
        let paint_segments = if display_row == prepared_full {
            segments
        } else {
            self.highlighter.highlight_line(display_row, abs_line)
        };
        let unstyled = paint_segments.iter().all(|(_, s)| *s == Style::default());
        let fallback: Vec<(&str, Style)>;
        let paint_segments = if unstyled {
            // Fallback: language-agnostic tokens through `theme.syntax()` —
            // weight + text ladder, never historical ANSI hues.
            fallback = tokenize_line(
                display_row,
                self.meta.language,
                keywords_for(self.meta.language),
            )
            .into_iter()
            .map(|(seg, kind)| (seg, self.system.junie_theme().syntax(kind.syntax_tone())))
            .collect();
            fallback
        } else {
            paint_segments
        };

        let mut col = 0u16;
        for (segment, mut style) in paint_segments {
            if style == Style::default() {
                style = self.system.junie_theme().syntax(SyntaxTone::Plain);
            } else {
                style = monochrome_syntax_style(style, mono);
            }
            style = field.patch(style);
            style = self.line_style_overlay(style, kinds, mono);
            if !kinds.iter().any(|k| {
                matches!(
                    k,
                    CodeHighlightKind::Selection
                        | CodeHighlightKind::DiffAdd
                        | CodeHighlightKind::DiffRemove
                )
            }) {
                style = Style {
                    bg: field.bg,
                    ..style
                };
            }
            if col >= width {
                break;
            }
            let remaining = usize::from(width.saturating_sub(col));
            let clipped = take_display_cols(segment, remaining);
            let used = u16::try_from(display_cols(&clipped))
                .unwrap_or(0)
                .min(width.saturating_sub(col));
            buffer.set_stringn(x.saturating_add(col), y, &clipped, remaining, style);
            col = col.saturating_add(used);
        }
        if overflow && width > 0 {
            let theme = self.system.junie_theme();
            buffer.set_stringn(
                x.saturating_add(width.saturating_sub(1)),
                y,
                self.system.glyphs.ellipsis(),
                1,
                field.fg(theme.text_muted),
            );
        }
        let _ = abs_line;
    }

    /// Key handling (scroll / cursor / copy / activate).
    pub fn handle_key(&self, state: &mut CodeBlockState, key: KeyEvent) -> CodeBlockOutcome {
        if !state.focused || key.kind != KeyEventKind::Press {
            return CodeBlockOutcome::Ignored;
        }
        let doc = self.document_len();
        // Copy
        if matches!(key.code, crate::input::KeyCode::Char('c' | 'C')) && key.modifiers.is_empty() {
            // Nothing on screen changes when text reaches the clipboard, so
            // the header owes an acknowledgement or the operator presses again.
            state.copied.fire(self.system.elapsed_ms());
            return CodeBlockOutcome::Copy {
                text: self.copy_text(state),
            };
        }
        // Horizontal when clip
        if matches!(self.wrap, CodeWrap::Clip) {
            match key.code {
                crate::input::KeyCode::Left | crate::input::KeyCode::Char('h' | 'H')
                    if state.cursor_line.is_none()
                        || key.modifiers.contains(crate::input::KeyModifiers::SHIFT) =>
                {
                    if state.scroll_by_cols(-1) {
                        return CodeBlockOutcome::Scrolled {
                            scroll_y: state.scroll_y,
                            scroll_x: state.scroll_x,
                        };
                    }
                }
                crate::input::KeyCode::Right | crate::input::KeyCode::Char('l' | 'L')
                    if state.cursor_line.is_none()
                        || key.modifiers.contains(crate::input::KeyModifiers::SHIFT) =>
                {
                    if state.scroll_by_cols(1) {
                        return CodeBlockOutcome::Scrolled {
                            scroll_y: state.scroll_y,
                            scroll_x: state.scroll_x,
                        };
                    }
                }
                _ => {}
            }
        }

        if let Some(intent) = default_list_intent(key) {
            return self.handle_intent(state, intent, doc);
        }
        CodeBlockOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(
        &self,
        state: &mut CodeBlockState,
        intent: UiIntent,
        doc: usize,
    ) -> CodeBlockOutcome {
        if !state.focused {
            return CodeBlockOutcome::Ignored;
        }
        let page = isize::try_from(state.viewport_rows.max(1)).unwrap_or(1);
        match intent {
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                self.move_cursor(state, -1, doc)
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                self.move_cursor(state, 1, doc)
            }
            UiIntent::Move(NavigationMove::First) => {
                state.scroll_y = 0;
                state.cursor_line = if doc == 0 { None } else { Some(0) };
                CodeBlockOutcome::Scrolled {
                    scroll_y: 0,
                    scroll_x: state.scroll_x,
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                let last = doc.saturating_sub(1);
                state.cursor_line = if doc == 0 { None } else { Some(last) };
                state.reveal_line(last, doc);
                CodeBlockOutcome::Scrolled {
                    scroll_y: state.scroll_y,
                    scroll_x: state.scroll_x,
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                if state.scroll_by_lines(-page, doc) {
                    CodeBlockOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                        scroll_x: state.scroll_x,
                    }
                } else {
                    CodeBlockOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Forward) => {
                if state.scroll_by_lines(page, doc) {
                    CodeBlockOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                        scroll_x: state.scroll_x,
                    }
                } else {
                    CodeBlockOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(line) = state.cursor_line {
                    CodeBlockOutcome::LineActivated { line }
                } else {
                    CodeBlockOutcome::Ignored
                }
            }
            _ => CodeBlockOutcome::Ignored,
        }
    }

    fn move_cursor(
        &self,
        state: &mut CodeBlockState,
        delta: isize,
        doc: usize,
    ) -> CodeBlockOutcome {
        if doc == 0 {
            return CodeBlockOutcome::Ignored;
        }
        let cur = state.cursor_line.unwrap_or(state.scroll_y);
        let next = if delta >= 0 {
            cur.saturating_add(usize::try_from(delta).unwrap_or(0))
                .min(doc - 1)
        } else {
            cur.saturating_sub(usize::try_from(-delta).unwrap_or(0))
        };
        state.cursor_line = Some(next);
        state.reveal_line(next, doc);
        CodeBlockOutcome::CursorMoved { line: next }
    }

    /// Mouse: wheel scroll, click select line.
    pub fn handle_mouse(&self, state: &mut CodeBlockState, event: MouseEvent) -> CodeBlockOutcome {
        let Some(parts) = state.parts.clone() else {
            return CodeBlockOutcome::Ignored;
        };
        let doc = self.document_len();
        match event.kind {
            MouseEventKind::ScrollUp => {
                if state.scroll_by_lines(-3, doc) {
                    return CodeBlockOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                        scroll_x: state.scroll_x,
                    };
                }
            }
            MouseEventKind::ScrollDown => {
                if state.scroll_by_lines(3, doc) {
                    return CodeBlockOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                        scroll_x: state.scroll_x,
                    };
                }
            }
            MouseEventKind::ScrollLeft => {
                if matches!(self.wrap, CodeWrap::Clip) && state.scroll_by_cols(-4) {
                    return CodeBlockOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                        scroll_x: state.scroll_x,
                    };
                }
            }
            MouseEventKind::ScrollRight => {
                if matches!(self.wrap, CodeWrap::Clip) && state.scroll_by_cols(4) {
                    return CodeBlockOutcome::Scrolled {
                        scroll_y: state.scroll_y,
                        scroll_x: state.scroll_x,
                    };
                }
            }
            MouseEventKind::Moved => {
                if parts.body.contains(event.position) || parts.gutter.contains(event.position) {
                    let row = event.position.y.saturating_sub(parts.body.y);
                    state.hovered_line = Some(parts.first_line.saturating_add(usize::from(row)));
                } else {
                    state.hovered_line = None;
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if parts.body.contains(event.position) || parts.gutter.contains(event.position) {
                    let row = event.position.y.saturating_sub(parts.body.y);
                    let line = parts.first_line.saturating_add(usize::from(row));
                    if line < doc {
                        state.focused = true;
                        state.cursor_line = Some(line);
                        state.selection = Some((line, line.saturating_add(1)));
                        return CodeBlockOutcome::SelectionChanged {
                            range: (line, line.saturating_add(1)),
                        };
                    }
                }
            }
            _ => {}
        }
        CodeBlockOutcome::Ignored
    }

    /// EventResult wrapper.
    pub fn handle_key_result(
        &self,
        state: &mut CodeBlockState,
        key: KeyEvent,
    ) -> EventResult<CodeBlockOutcome> {
        match self.handle_key(state, key) {
            CodeBlockOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Semantic registration (content + focusable when interactive).
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &CodeBlockState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let parts = self.layout(area, state);
        if parts.root.is_empty() {
            return;
        }
        let desc = format!(
            "code block {} lines{}",
            self.document_len(),
            if self.streaming { " streaming" } else { "" }
        );
        let _ = scene.register(
            SemanticNode::control(id, parts.root)
                .role(SemanticRole::Content)
                .label("code")
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    selected: state.focused,
                    ..Default::default()
                }),
        );
    }
}

impl<H: SyntaxHighlighter> Widget for &CodeBlock<'_, H> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = CodeBlockState::new().with_scroll_y(self.first_line);
        let _ = self.paint(area, buffer, &mut state);
    }
}

impl<H: SyntaxHighlighter> Widget for CodeBlock<'_, H> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn horizontal_slice(s: &str, scroll_x: u16, width: u16) -> String {
    if width == 0 {
        return String::new();
    }
    // Skip scroll_x columns then take width.
    let skipped = take_display_cols_from(s, usize::from(scroll_x));
    take_display_cols(&skipped, usize::from(width))
}

/// Drop the first `skip` display columns of `s`, return remainder.
fn take_display_cols_from(s: &str, skip: usize) -> String {
    if skip == 0 {
        return s.to_string();
    }
    let total = display_cols(s);
    if skip >= total {
        return String::new();
    }
    // Walk graphemes
    use unicode_segmentation::UnicodeSegmentation;
    let mut used = 0usize;
    let mut start = 0usize;
    for (idx, g) in s.grapheme_indices(true) {
        if used >= skip {
            start = idx;
            break;
        }
        let w = display_cols(g);
        if used + w > skip {
            // Mid-wide skip — start after this grapheme
            start = idx + g.len();
            break;
        }
        used += w;
        start = idx + g.len();
    }
    s[start..].to_string()
}

fn monochrome_syntax_style(style: Style, mono: bool) -> Style {
    if !mono {
        return style;
    }
    // Reinforce non-color cues when palette is mono. Never introduce hue.
    let mut s = style;
    if s.add_modifier.contains(Modifier::BOLD) {
        s = s.add_modifier(Modifier::BOLD);
    }
    s
}

const fn is_operator_byte(b: u8) -> bool {
    matches!(
        b,
        b'+' | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'='
            | b'<'
            | b'>'
            | b'!'
            | b'&'
            | b'|'
            | b'^'
            | b'~'
            | b'?'
            | b':'
    )
}

const fn is_punct_byte(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';' | b'.'
    )
}

fn keywords_for(language: Option<&str>) -> &'static [&'static str] {
    match language {
        Some("rust") => &[
            "fn", "let", "mut", "const", "pub", "struct", "enum", "impl", "use", "mod", "if",
            "else", "match", "for", "while", "loop", "return", "async", "await", "self", "Self",
            "true", "false", "where", "type", "trait", "in", "ref", "move",
        ],
        Some("sh" | "bash" | "zsh" | "shell") => &[
            "if", "then", "else", "fi", "for", "do", "done", "while", "case", "esac", "function",
            "export", "local", "return", "exit", "cd", "echo", "cargo", "git",
        ],
        _ => &[],
    }
}

fn tokenize_line<'a>(
    line: &'a str,
    language: Option<&str>,
    keywords: &[&str],
) -> Vec<(&'a str, CodeTokenKind)> {
    if line.is_empty() {
        return vec![("", CodeTokenKind::Plain)];
    }
    // Line comment
    let comment_prefix = match language {
        Some("rust" | "c" | "cpp" | "js" | "ts" | "go" | "java") => Some("//"),
        Some("sh" | "bash" | "zsh" | "shell" | "python" | "py" | "toml" | "yaml" | "yml") => {
            Some("#")
        }
        _ => None,
    };
    if let Some(prefix) = comment_prefix
        && let Some(idx) = line.find(prefix)
    {
        let mut out = Vec::new();
        if idx > 0 {
            out.extend(tokenize_code_part(&line[..idx], keywords));
        }
        out.push((&line[idx..], CodeTokenKind::Comment));
        return out;
    }
    tokenize_code_part(line, keywords)
}

fn tokenize_code_part<'a>(line: &'a str, keywords: &[&str]) -> Vec<(&'a str, CodeTokenKind)> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // String
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push((&line[start..i], CodeTokenKind::String));
            continue;
        }
        // Number
        if bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            out.push((&line[start..i], CodeTokenKind::Number));
            continue;
        }
        // Ident
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            let kind = if keywords.contains(&word) {
                CodeTokenKind::Keyword
            } else if i < bytes.len() && bytes[i] == b'(' {
                CodeTokenKind::Function
            } else {
                CodeTokenKind::Ident
            };
            out.push((word, kind));
            continue;
        }
        // Operator run
        if is_operator_byte(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_operator_byte(bytes[i]) {
                i += 1;
            }
            out.push((&line[start..i], CodeTokenKind::Operator));
            continue;
        }
        // Punctuation run
        if is_punct_byte(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_punct_byte(bytes[i]) {
                i += 1;
            }
            out.push((&line[start..i], CodeTokenKind::Punct));
            continue;
        }
        // Whitespace / other
        let start = i;
        i += 1;
        while i < bytes.len()
            && !bytes[i].is_ascii_alphanumeric()
            && bytes[i] != b'_'
            && bytes[i] != b'"'
            && bytes[i] != b'\''
            && !is_operator_byte(bytes[i])
            && !is_punct_byte(bytes[i])
        {
            i += 1;
        }
        out.push((&line[start..i], CodeTokenKind::Plain));
    }
    if out.is_empty() {
        out.push((line, CodeTokenKind::Plain));
    }
    out
}

// ── Role-aware token paint helper for hosts ─────────────────────────────────

/// Map a token kind's [`Role`] through `theme.syntax()` (weight + ladder, no hue).
///
/// Call from custom highlighters when building segments with [`Role`].
#[must_use]
pub fn syntax_role_style(system: &DesignSystem, role: Role) -> Style {
    let theme = system.junie_theme();
    let tone = match role {
        Role::SyntaxKeyword => SyntaxTone::Keyword,
        Role::SyntaxString => SyntaxTone::Str,
        Role::SyntaxNumber => SyntaxTone::Number,
        Role::SyntaxComment => SyntaxTone::Comment,
        Role::SyntaxFunction => SyntaxTone::Ident,
        Role::TextMuted => SyntaxTone::Operator,
        _ => SyntaxTone::Plain,
    };
    let mut style = theme.syntax(tone);
    style = Style { bg: None, ..style };
    if matches!(system.capability, crate::style::ColorCapability::Monochrome)
        && matches!(tone, SyntaxTone::Keyword)
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

/// Role-based token highlighter (uses DesignSystem colors / mono fallbacks).
#[derive(Debug, Clone, Copy)]
pub struct RoleTokenSyntax<'a> {
    system: &'a DesignSystem,
    language: Option<&'a str>,
    keywords: &'a [&'a str],
}

impl<'a> RoleTokenSyntax<'a> {
    /// Construct with system for role resolution.
    #[must_use]
    pub const fn new(
        system: &'a DesignSystem,
        language: Option<&'a str>,
        keywords: &'a [&'a str],
    ) -> Self {
        Self {
            system,
            language,
            keywords,
        }
    }

    /// Rust keywords + system.
    #[must_use]
    pub const fn rust(system: &'a DesignSystem) -> Self {
        const KW: &[&str] = &[
            "fn", "let", "mut", "const", "pub", "struct", "enum", "impl", "use", "mod", "if",
            "else", "match", "for", "while", "loop", "return", "async", "await", "self", "Self",
            "true", "false", "where", "type", "trait", "in", "ref", "move",
        ];
        Self {
            system,
            language: Some("rust"),
            keywords: KW,
        }
    }

    /// Shell / command fence keywords + system.
    #[must_use]
    pub const fn shell(system: &'a DesignSystem) -> Self {
        const KW: &[&str] = &[
            "if", "then", "else", "fi", "for", "do", "done", "while", "case", "esac", "function",
            "export", "local", "return", "exit", "cd", "echo", "cargo", "git",
        ];
        Self {
            system,
            language: Some("sh"),
            keywords: KW,
        }
    }

    /// Tokenizes one prepared line into classified kinds.
    ///
    /// The kind is the fact; [`DesignSystem`] owns the only presentation.
    #[must_use]
    pub fn tokens_for_line<'line>(&self, line: &'line str) -> Vec<(&'line str, CodeTokenKind)> {
        tokenize_line(line, self.language, self.keywords)
    }
}

impl SyntaxHighlighter for RoleTokenSyntax<'_> {
    fn highlight_line<'line>(
        &self,
        line: &'line str,
        _line_index: usize,
    ) -> Vec<(&'line str, Style)> {
        self.tokens_for_line(line)
            .into_iter()
            .map(|(seg, kind)| (seg, self.system.junie_theme().syntax(kind.syntax_tone())))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};

    #[test]
    fn paints_line_numbers_and_source() {
        let system = DesignSystem::default();
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

    #[test]
    fn visible_controls_use_caret_notation() {
        let s = prepare_code_display("a\x01b\t c", 4, ControlRender::Visible);
        assert!(s.contains("^A"), "{s}");
        assert!(!s.contains('\x01'));
        // tab expanded
        assert!(s.contains(' '));
    }

    #[test]
    fn expand_tabs_policy_strips_controls() {
        let s = prepare_code_display("a\x01\tb", 4, ControlRender::ExpandTabs);
        assert!(!s.contains('\x01'));
        assert!(!s.contains('^'));
    }

    #[test]
    fn virtualization_window_line_base() {
        let system = DesignSystem::default();
        // Host provides lines 100..103 of a 1000-line file
        let window = ["line100", "line101", "line102"];
        let mut state = CodeBlockState::new().with_scroll_y(100);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 4));
        let parts = CodeBlock::new(&window, &system)
            .line_base(100)
            .logical_len(1000)
            .line_numbers(true)
            .paint(Rect::new(0, 0, 40, 4), &mut buf, &mut state);
        assert_eq!(parts.first_line, 100);
        let row: String = (0..40).map(|x| buf[(x, 0)].symbol().to_owned()).collect();
        // start_line_number default 1 → display 101 for abs 100
        assert!(row.contains("101") || row.contains("line100"), "{row}");
    }

    #[test]
    fn scroll_and_copy() {
        let system = DesignSystem::default();
        let lines = ["a", "b", "c", "d", "e"];
        let block = CodeBlock::new(&lines, &system);
        let mut state = CodeBlockState::new();
        state.set_focused(true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        let _ = block.paint(Rect::new(0, 0, 20, 3), &mut buf, &mut state);
        let out = block.handle_key(&mut state, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(
            out,
            CodeBlockOutcome::CursorMoved { line: 1 } | CodeBlockOutcome::Scrolled { .. }
        ));
        state.selection = Some((1, 3));
        let text = block.copy_text(&state);
        assert_eq!(text, "b\nc");
        let copy = block.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        );
        assert!(matches!(copy, CodeBlockOutcome::Copy { .. }));
    }

    #[test]
    fn streaming_cue() {
        let system = DesignSystem::default();
        let lines = ["partial"];
        let mut state = CodeBlockState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        let parts = CodeBlock::new(&lines, &system).streaming(true).paint(
            Rect::new(0, 0, 20, 3),
            &mut buf,
            &mut state,
        );
        assert!(parts.streaming);
    }

    #[test]
    fn highlights_and_gutter_marks() {
        let system = DesignSystem::default();
        let lines = ["ok", "err", "ok"];
        let marks = [CodeGutterMark::new(1, '!', Role::Danger)];
        let highs = [CodeHighlight::line(1, CodeHighlightKind::Diagnostic)];
        let mut state = CodeBlockState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 4));
        let _ = CodeBlock::new(&lines, &system)
            .line_numbers(true)
            .gutter_marks(&marks)
            .highlights(&highs)
            .paint(Rect::new(0, 0, 30, 4), &mut buf, &mut state);
        assert_eq!(buf[(1, 1)].symbol(), "!");
        assert!(
            buf[(6, 1)]
                .style()
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn syntax_uses_theme_ladder_not_hue() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let hi = RoleTokenSyntax::rust(&system);
        let segs = hi.highlight_line("fn main() { let x = 1; // c", 0);
        let keyword = segs.iter().find(|(s, _)| *s == "fn").map(|(_, st)| *st);
        let ident = segs.iter().find(|(s, _)| *s == "main").map(|(_, st)| *st);
        let stringish = hi.highlight_line("\"hi\"", 0);
        let number = hi.highlight_line("42", 0);
        let comment = hi.highlight_line("// x", 0);
        assert_eq!(keyword, Some(theme.syntax(SyntaxTone::Keyword)));
        assert_eq!(ident, Some(theme.syntax(SyntaxTone::Ident)));
        assert_eq!(stringish[0].1, theme.syntax(SyntaxTone::Str));
        assert_eq!(number[0].1, theme.syntax(SyntaxTone::Number));
        assert_eq!(comment[0].1, theme.syntax(SyntaxTone::Comment));
        assert_eq!(
            theme.syntax(SyntaxTone::Keyword).fg,
            Some(theme.text_primary)
        );
        assert!(
            theme
                .syntax(SyntaxTone::Keyword)
                .add_modifier
                .contains(Modifier::BOLD)
        );
        assert_eq!(theme.syntax(SyntaxTone::Comment).fg, Some(theme.text_faint));
        assert!(
            theme
                .syntax(SyntaxTone::Comment)
                .add_modifier
                .contains(Modifier::ITALIC)
        );
    }

    #[test]
    fn current_block_marker_is_chevron_not_second_bar() {
        let system = DesignSystem::junie();
        let lines = ["fn main() {", "    let x = 1;", "}"];
        let mut state = CodeBlockState::new();
        state.set_focused(true);
        state.set_cursor_line(Some(0));
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 4));
        let _ = CodeBlock::new(&lines, &system)
            .line_numbers(true)
            .language("rust")
            .current_block(0, 1)
            .paint(Rect::new(0, 0, 40, 4), &mut buf, &mut state);
        // header is rust on row 0; body starts row 1
        assert_eq!(buf[(0, 1)].symbol(), "▎", "focus bar on cursor line");
        assert_eq!(buf[(1, 1)].symbol(), "›", "block marker, not a second bar");
        assert_ne!(buf[(1, 1)].symbol(), "▎");
        assert_ne!(
            buf[(0, 2)].symbol(),
            "▎",
            "non-cursor body row has no focus bar"
        );
        // line numbers are muted off the current block
        assert_eq!(buf[(3, 2)].fg, system.junie_theme().text_secondary);
    }

    #[test]
    fn unfocused_cursor_line_still_paints_gutter_glyph() {
        let system = DesignSystem::junie();
        let lines = ["// Retry a request with exponential backoff."];
        let mut state = CodeBlockState::new();
        state.set_focused(false);
        state.set_cursor_line(Some(0));
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 2));
        let _ = CodeBlock::new(&lines, &system)
            .line_numbers(true)
            .current_block(0, 1)
            .paint(Rect::new(0, 0, 60, 2), &mut buf, &mut state);
        assert_eq!(buf[(0, 0)].symbol(), "▎", "cursor line keeps the bar glyph");
        assert_eq!(buf[(1, 0)].symbol(), "›", "current block marker");
    }

    #[test]
    fn fallback_highlighter_has_no_ansi_hues() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let lines = ["fn foo() { let s = \"x\"; }"];
        let mut state = CodeBlockState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 2));
        let _ = CodeBlock::new(&lines, &system).language("rust").paint(
            Rect::new(0, 0, 40, 2),
            &mut buf,
            &mut state,
        );
        // Keyword `fn` is primary+bold, never a green/cyan ANSI hue.
        let cell = &buf[(0, 1)];
        assert_eq!(cell.fg, theme.text_primary);
        assert_ne!(cell.fg, theme.accent);
    }
}
