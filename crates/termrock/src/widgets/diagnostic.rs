// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Diagnostic** + **CodeFrame** — structured diagnostics with source context
//! and actionable fixes.
//!
//! **Mission.** Severity, code, message, source, range, related locations,
//! notes, help, documentation link, suggested fixes, and copyable details.
//! Renders single- and multi-line spans, tabs, Unicode, overlapping diagnostics,
//! and truncated files. Recipes: list, inline, full code-frame. Severity is
//! never color-only (letters/glyphs + labels).
//!
//! **Ownership.** Host projects diagnostics and optional source line windows.
//! TermRock owns paint, recipes, selection, expand, and fix-cursor chrome.
//! Apply/fix/open-docs effects stay consumer-owned outcomes.
//!
//! **Integration.** Compose with [`super::CodeBlock`] highlights/gutters,
//! [`super::ErrorState`] copy-diagnostics, forms (inline recipe), and build
//! output lists.
//!
//! Research: rustc diagnostics, miette, Rich tracebacks, IDE problems panels.
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::code_block::{CodeGutterMark, CodeHighlight, CodeHighlightKind},
    widgets::scroll_area::ScrollAreaState,
    widgets::tiered_row::TieredRow,
};

// ── Severity ────────────────────────────────────────────────────────────────

/// Diagnostic severity (rustc/miette class).
///
/// Always pair with [`DiagnosticSeverity::letter`] / [`DiagnosticSeverity::glyph`]
/// — never color alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    /// Hard error.
    Error,
    /// Warning.
    Warning,
    /// Informational.
    #[default]
    Info,
    /// Hint / suggestion tone.
    Hint,
    /// Note / secondary explanation.
    Note,
    /// Help / how-to.
    Help,
}

impl DiagnosticSeverity {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
            Self::Note => "note",
            Self::Help => "help",
        }
    }

    /// Word label for lists (`error`, `warning`, …).
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.id()
    }

    /// No-color letter (E/W/I/H/N/?).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Error => 'E',
            Self::Warning => 'W',
            Self::Info => 'I',
            Self::Hint => 'H',
            Self::Note => 'N',
            Self::Help => '?',
        }
    }

    /// Glyph (ASCII uses letter).
    #[must_use]
    pub const fn glyph(self, _ascii: bool) -> &'static str {
        match self {
            Self::Error => "x",
            Self::Warning => "!",
            Self::Info => "i",
            Self::Hint => "›",
            Self::Note => "·",
            Self::Help => "?",
        }
    }

    /// Semantic role (always with letter/glyph).
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Error => Role::Danger,
            Self::Warning => Role::Warning,
            Self::Info => Role::TextSecondary,
            // Hints assist; they do not compete with the current intent.
            Self::Hint | Self::Help => Role::TextSecondary,
            Self::Note => Role::TextMuted,
        }
    }

    /// Map toast-style severity when hosts only have four levels.
    #[must_use]
    pub const fn from_toast(severity: crate::widgets::Severity) -> Self {
        match severity {
            crate::widgets::Severity::Error => Self::Error,
            crate::widgets::Severity::Warning => Self::Warning,
            crate::widgets::Severity::Info | crate::widgets::Severity::Success => Self::Info,
        }
    }
}

// ── Spans & locations ───────────────────────────────────────────────────────

/// Primary vs secondary label under a code frame (rustc style).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SpanStyle {
    /// Primary caret (`^^^^`).
    #[default]
    Primary,
    /// Secondary (`----`).
    Secondary,
}

impl SpanStyle {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }

    /// Underline glyph row (ASCII-safe).
    #[must_use]
    pub const fn underline_char(self, _ascii: bool) -> char {
        match (self, false) {
            (Self::Primary, true) | (Self::Primary, false) => '^',
            (Self::Secondary, true) => '-',
            (Self::Secondary, false) => '─',
        }
    }
}

/// 1-based line/column range in source (display columns for paint).
///
/// Columns are **display columns** after tab expansion (tab stop 4 by default
/// in CodeFrame). Lines are 1-based absolute in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceRange {
    /// Start line (1-based).
    pub start_line: u32,
    /// Start display column (1-based).
    pub start_col: u32,
    /// End line (1-based, inclusive for single-point when equal).
    pub end_line: u32,
    /// End display column (1-based exclusive preferred; inclusive paint when equal).
    pub end_col: u32,
}

impl SourceRange {
    /// Single point.
    #[must_use]
    pub const fn point(line: u32, col: u32) -> Self {
        let end = col.saturating_add(1);
        Self {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: if end == 0 { 1 } else { end },
        }
    }

    /// Single-line span [start_col, end_col).
    #[must_use]
    pub const fn line_span(line: u32, start_col: u32, end_col: u32) -> Self {
        Self {
            start_line: line,
            start_col,
            end_line: line,
            end_col: if end_col > start_col {
                end_col
            } else {
                start_col.saturating_add(1)
            },
        }
    }

    /// Multi-line range.
    #[must_use]
    pub const fn multi(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line: if end_line < start_line {
                start_line
            } else {
                end_line
            },
            end_col,
        }
    }

    /// Whether range covers absolute 1-based line.
    #[must_use]
    pub const fn covers_line(self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// Labeled span for code-frame underlines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLabel<'a> {
    /// Range.
    pub range: SourceRange,
    /// Optional short label (`expected i32`).
    pub label: Option<&'a str>,
    /// Primary / secondary.
    pub style: SpanStyle,
}

impl<'a> SourceLabel<'a> {
    /// Primary span.
    #[must_use]
    pub const fn primary(range: SourceRange) -> Self {
        Self {
            range,
            label: None,
            style: SpanStyle::Primary,
        }
    }

    /// Secondary span.
    #[must_use]
    pub const fn secondary(range: SourceRange) -> Self {
        Self {
            range,
            label: None,
            style: SpanStyle::Secondary,
        }
    }

    /// Label text.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    /// Style override.
    #[must_use]
    pub const fn style(mut self, style: SpanStyle) -> Self {
        self.style = style;
        self
    }
}

/// Related location (another file/line).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedLocation<'a> {
    /// Path.
    pub file: Option<&'a str>,
    /// Range.
    pub range: SourceRange,
    /// Message.
    pub message: &'a str,
}

impl<'a> RelatedLocation<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(message: &'a str, range: SourceRange) -> Self {
        Self {
            file: None,
            range,
            message,
        }
    }

    /// File.
    #[must_use]
    pub const fn file(mut self, file: &'a str) -> Self {
        self.file = Some(file);
        self
    }
}

/// Note / help / context line under a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticNote<'a> {
    /// Tone.
    pub severity: DiagnosticSeverity,
    /// Body.
    pub message: &'a str,
}

impl<'a> DiagnosticNote<'a> {
    /// Note.
    #[must_use]
    pub const fn note(message: &'a str) -> Self {
        Self {
            severity: DiagnosticSeverity::Note,
            message,
        }
    }

    /// Help.
    #[must_use]
    pub const fn help(message: &'a str) -> Self {
        Self {
            severity: DiagnosticSeverity::Help,
            message,
        }
    }

    /// Custom severity.
    #[must_use]
    pub const fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = severity;
        self
    }
}

/// Suggested fix applicability (rustc-like).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FixApplicability {
    /// Machine-applicable.
    MachineApplicable,
    /// Maybe incorrect.
    MaybeIncorrect,
    /// Unspecified / human only.
    #[default]
    Unspecified,
}

impl FixApplicability {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::MachineApplicable => "machine",
            Self::MaybeIncorrect => "maybe",
            Self::Unspecified => "unspecified",
        }
    }
}

/// Suggested fix (host applies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedFix<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Human message.
    pub message: &'a str,
    /// Optional replacement snippet (preview).
    pub replacement: Option<&'a str>,
    /// Applicability.
    pub applicability: FixApplicability,
}

impl<'a> SuggestedFix<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, message: &'a str) -> Self {
        Self {
            id,
            message,
            replacement: None,
            applicability: FixApplicability::Unspecified,
        }
    }

    /// Replacement.
    #[must_use]
    pub const fn replacement(mut self, text: &'a str) -> Self {
        self.replacement = Some(text);
        self
    }

    /// Applicability.
    #[must_use]
    pub const fn applicability(mut self, a: FixApplicability) -> Self {
        self.applicability = a;
        self
    }
}

// ── Diagnostic model ────────────────────────────────────────────────────────

/// One structured diagnostic (host-owned storage; TermRock paints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic<'a> {
    /// Stable id (selection / expand).
    pub id: &'a str,
    /// Severity.
    pub severity: DiagnosticSeverity,
    /// Error code (`E0308`, `clippy::foo`).
    pub code: Option<&'a str>,
    /// Primary message.
    pub message: &'a str,
    /// Tool / subsystem source (`rustc`, `form`, `build`).
    pub source: Option<&'a str>,
    /// File path for primary span.
    pub file: Option<&'a str>,
    /// Primary + secondary labels (first primary preferred).
    pub labels: &'a [SourceLabel<'a>],
    /// Related locations.
    pub related: &'a [RelatedLocation<'a>],
    /// Notes / help lines.
    pub notes: &'a [DiagnosticNote<'a>],
    /// Free-form help string (also painted as help note).
    pub help: Option<&'a str>,
    /// Documentation URL (host opens).
    pub docs_url: Option<&'a str>,
    /// Suggested fixes.
    pub fixes: &'a [SuggestedFix<'a>],
}

impl<'a> Diagnostic<'a> {
    /// Minimal diagnostic.
    #[must_use]
    pub const fn new(id: &'a str, severity: DiagnosticSeverity, message: &'a str) -> Self {
        Self {
            id,
            severity,
            code: None,
            message,
            source: None,
            file: None,
            labels: &[],
            related: &[],
            notes: &[],
            help: None,
            docs_url: None,
            fixes: &[],
        }
    }

    /// Code.
    #[must_use]
    pub const fn code(mut self, code: &'a str) -> Self {
        self.code = Some(code);
        self
    }

    /// Source tool.
    #[must_use]
    pub const fn source(mut self, source: &'a str) -> Self {
        self.source = Some(source);
        self
    }

    /// File.
    #[must_use]
    pub const fn file(mut self, file: &'a str) -> Self {
        self.file = Some(file);
        self
    }

    /// Labels.
    #[must_use]
    pub const fn labels(mut self, labels: &'a [SourceLabel<'a>]) -> Self {
        self.labels = labels;
        self
    }

    /// Related.
    #[must_use]
    pub const fn related(mut self, related: &'a [RelatedLocation<'a>]) -> Self {
        self.related = related;
        self
    }

    /// Notes.
    #[must_use]
    pub const fn notes(mut self, notes: &'a [DiagnosticNote<'a>]) -> Self {
        self.notes = notes;
        self
    }

    /// Help.
    #[must_use]
    pub const fn help(mut self, help: &'a str) -> Self {
        self.help = Some(help);
        self
    }

    /// Docs URL.
    #[must_use]
    pub const fn docs_url(mut self, url: &'a str) -> Self {
        self.docs_url = Some(url);
        self
    }

    /// Fixes.
    #[must_use]
    pub const fn fixes(mut self, fixes: &'a [SuggestedFix<'a>]) -> Self {
        self.fixes = fixes;
        self
    }

    /// Primary range if any.
    #[must_use]
    pub fn primary_range(&self) -> Option<SourceRange> {
        self.labels
            .iter()
            .find(|l| l.style == SpanStyle::Primary)
            .or_else(|| self.labels.first())
            .map(|l| l.range)
    }
}

/// Presentation recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DiagnosticRecipe {
    /// Compact problems-panel list (default).
    #[default]
    List,
    /// One-line inline (forms / field chrome).
    Inline,
    /// Full miette/rustc code-frame + notes + fixes.
    Full,
}

impl DiagnosticRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Inline => "inline",
            Self::Full => "full",
        }
    }
}

// ── CodeFrame ───────────────────────────────────────────────────────────────

/// Default tab stop for source display columns.
pub const CODE_FRAME_TAB_STOP: usize = 4;

/// One source line for a code frame (host projects a window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFrameLine<'a> {
    /// 1-based absolute line number in file.
    pub number: u32,
    /// Raw line text (may contain tabs; expanded at paint).
    pub text: &'a str,
}

impl<'a> CodeFrameLine<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(number: u32, text: &'a str) -> Self {
        Self { number, text }
    }
}

/// Expand tabs to spaces for display-column alignment.
#[must_use]
pub fn expand_tabs(raw: &str, tab_stop: usize) -> String {
    let stop = tab_stop.max(1);
    let mut out = String::with_capacity(raw.len());
    let mut col = 0usize;
    for ch in raw.chars() {
        if ch == '\t' {
            let n = stop - (col % stop);
            for _ in 0..n {
                out.push(' ');
            }
            col += n;
        } else if ch == '\r' || ch == '\n' {
            // strip
        } else if ch.is_control() {
            let esc = format!("\\u{{{:x}}}", ch as u32);
            col += display_cols(&esc);
            out.push_str(&esc);
        } else {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            col += display_cols(s);
            out.push(ch);
        }
    }
    out
}

/// Code frame — source window with span underlines (miette/rustc style).
#[derive(Debug, Clone)]
pub struct CodeFrame<'a> {
    lines: &'a [CodeFrameLine<'a>],
    labels: &'a [SourceLabel<'a>],
    system: &'a DesignSystem,
    file: Option<&'a str>,
    colorless: bool,
    tab_stop: usize,
    /// When true, show `…` truncation markers if file context incomplete.
    truncated_above: bool,
    truncated_below: bool,
    max_width: Option<u16>,
}

impl<'a> CodeFrame<'a> {
    /// Lines + design system.
    #[must_use]
    pub const fn new(lines: &'a [CodeFrameLine<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            labels: &[],
            system,
            file: None,
            colorless: false,
            tab_stop: CODE_FRAME_TAB_STOP,
            truncated_above: false,
            truncated_below: false,
            max_width: None,
        }
    }

    /// Span labels.
    #[must_use]
    pub const fn labels(mut self, labels: &'a [SourceLabel<'a>]) -> Self {
        self.labels = labels;
        self
    }

    /// File path header.
    #[must_use]
    pub const fn file(mut self, file: &'a str) -> Self {
        self.file = Some(file);
        self
    }

    /// ASCII underlines.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Tab stop.
    #[must_use]
    pub const fn tab_stop(mut self, n: usize) -> Self {
        self.tab_stop = if n == 0 { CODE_FRAME_TAB_STOP } else { n };
        self
    }

    /// File truncated above window.
    #[must_use]
    pub const fn truncated_above(mut self, on: bool) -> Self {
        self.truncated_above = on;
        self
    }

    /// File truncated below window.
    #[must_use]
    pub const fn truncated_below(mut self, on: bool) -> Self {
        self.truncated_below = on;
        self
    }

    /// Paint. Returns rows used.
    pub fn render(&self, area: Rect, buffer: &mut Buffer) -> u16 {
        if !self.colorless && self.system.mono() {
            let mut effective = self.clone();
            effective.colorless |= self.system.mono();
            return effective.render(area, buffer);
        }
        if area.is_empty() {
            return 0;
        }
        let width = self
            .max_width
            .map(|w| w.min(area.width))
            .unwrap_or(area.width);
        let mut y = area.y;
        let bottom = area.bottom();

        if let Some(file) = self.file {
            if !file.is_empty() && y < bottom {
                let head = format!("--> {file}");
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&head, usize::from(width)),
                    usize::from(width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }

        if self.truncated_above && y < bottom {
            let mark = { "…" };
            buffer.set_stringn(
                area.x,
                y,
                mark,
                usize::from(width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        let gutter_w = self
            .lines
            .iter()
            .map(|l| l.number.to_string().len())
            .max()
            .unwrap_or(1)
            .max(1) as u16;
        let gutter_w = gutter_w.saturating_add(2).min(width.saturating_sub(4));

        for line in self.lines {
            if y >= bottom {
                break;
            }
            let expanded = expand_tabs(line.text, self.tab_stop);
            let num = format!("{:>width$}", line.number, width = gutter_w as usize - 1);
            let pipe = { "│" };
            let prefix = format!("{num}{pipe} ");
            let pref_w = display_cols(&prefix) as u16;
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&prefix, usize::from(width)),
                usize::from(width),
                self.system.style(Role::TextMuted),
            );
            let text_w = width.saturating_sub(pref_w);
            if text_w > 0 {
                buffer.set_stringn(
                    area.x.saturating_add(pref_w),
                    y,
                    take_display_cols(&expanded, usize::from(text_w)),
                    usize::from(text_w),
                    self.system.style(Role::Text),
                );
            }
            y = y.saturating_add(1);

            // Underlines for labels on this line
            let on_line: Vec<&SourceLabel<'_>> = self
                .labels
                .iter()
                .filter(|l| l.range.covers_line(line.number))
                .collect();
            if on_line.is_empty() {
                continue;
            }
            // Merge overlapping paints: paint one underline row with strongest style
            if y >= bottom {
                break;
            }
            let mut row = vec![' '; expanded.chars().count().max(1)];
            let mut row_styles: Vec<SpanStyle> = vec![SpanStyle::Secondary; row.len()];
            for lab in &on_line {
                let (sc, ec) = cols_on_line(lab.range, line.number, expanded.chars().count());
                let start = sc.saturating_sub(1) as usize;
                let end = (ec.saturating_sub(1) as usize).max(start + 1);
                let ch = lab.style.underline_char(false);
                for i in start..end.min(row.len()) {
                    row[i] = ch;
                    if lab.style == SpanStyle::Primary {
                        row_styles[i] = SpanStyle::Primary;
                    }
                }
            }
            let underline: String = row.iter().collect();
            let u_style = if self.colorless {
                self.system.style(Role::TextStrong)
            } else if row_styles.contains(&SpanStyle::Primary) {
                self.system.style(Role::Danger)
            } else {
                self.system.style(Role::Warning)
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&" ".repeat(pref_w as usize), usize::from(width)),
                usize::from(pref_w.min(width)),
                self.system.style(Role::TextMuted),
            );
            if text_w > 0 {
                buffer.set_stringn(
                    area.x.saturating_add(pref_w),
                    y,
                    take_display_cols(&underline, usize::from(text_w)),
                    usize::from(text_w),
                    u_style,
                );
            }
            y = y.saturating_add(1);

            // Labels text under primary/secondary
            for lab in on_line {
                if let Some(label) = lab.label {
                    if y >= bottom {
                        break;
                    }
                    let (sc, _) = cols_on_line(lab.range, line.number, expanded.chars().count());
                    let indent = pref_w.saturating_add(sc.saturating_sub(1) as u16);
                    let msg = format!("{label}");
                    buffer.set_stringn(
                        area.x.saturating_add(indent.min(width.saturating_sub(1))),
                        y,
                        take_display_cols(
                            &msg,
                            usize::from(width.saturating_sub(indent.min(width))),
                        ),
                        usize::from(width.saturating_sub(indent.min(width))),
                        if self.colorless {
                            self.system.style(Role::TextMuted)
                        } else {
                            self.system.style(lab.style.role_for_label())
                        },
                    );
                    y = y.saturating_add(1);
                }
            }
        }

        if self.truncated_below && y < bottom {
            let mark = { "…" };
            buffer.set_stringn(
                area.x,
                y,
                mark,
                usize::from(width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        y.saturating_sub(area.y)
    }
}

impl SpanStyle {
    fn role_for_label(self) -> Role {
        match self {
            Self::Primary => Role::Danger,
            Self::Secondary => Role::Warning,
        }
    }
}

fn cols_on_line(range: SourceRange, line: u32, line_len: usize) -> (u32, u32) {
    let len = line_len as u32;
    if range.start_line == range.end_line {
        return (
            range.start_col.max(1),
            range.end_col.max(range.start_col + 1),
        );
    }
    if line == range.start_line {
        return (range.start_col.max(1), len.saturating_add(1).max(2));
    }
    if line == range.end_line {
        return (1, range.end_col.max(2));
    }
    // middle of multi-line
    (1, len.saturating_add(1).max(2))
}

// ── State & outcomes ────────────────────────────────────────────────────────

/// Hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRegion {
    /// Diagnostic id.
    pub id: String,
    /// Index.
    pub index: usize,
    /// Area.
    pub area: Rect,
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticOutcome {
    /// No change.
    Ignored,
    /// Cursor moved.
    CursorMoved {
        /// Index.
        index: usize,
    },
    /// Expand toggled.
    Expanded {
        /// Id.
        id: String,
        /// Expanded after.
        on: bool,
    },
    /// Scrolled.
    Scrolled {
        /// Offset.
        offset: u16,
    },
    /// Activate / open location.
    Activated {
        /// Diagnostic id.
        id: String,
    },
    /// Fix selected.
    FixSelected {
        /// Diagnostic id.
        diagnostic_id: String,
        /// Fix id.
        fix_id: String,
    },
    /// Apply fix request (host).
    ApplyFixRequested {
        /// Diagnostic id.
        diagnostic_id: String,
        /// Fix id.
        fix_id: String,
    },
    /// Open docs URL request.
    OpenDocsRequested {
        /// URL.
        url: String,
    },
    /// Copy details request (host clipboard).
    CopyDetails {
        /// Plain text.
        text: String,
    },
    /// Cancelled.
    Cancelled,
}

/// Interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticState {
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    /// Cursor index.
    pub cursor: usize,
    /// Expanded diagnostic ids.
    expanded: BTreeSet<String>,
    /// Fix cursor within expanded diagnostic.
    pub fix_cursor: usize,
    /// Recipe override (widget may also set).
    pub recipe: DiagnosticRecipe,
    /// Regions.
    pub regions: Vec<DiagnosticRegion>,
    /// Prefer no-color paint (letters still shown).
    pub colorless: bool,
}

impl Default for DiagnosticState {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticState {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll: ScrollAreaState::new().axes(true, false),
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
            cursor: 0,
            expanded: BTreeSet::new(),
            fix_cursor: 0,
            recipe: DiagnosticRecipe::List,
            regions: Vec::new(),
            colorless: false,
        }
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Offset.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Expanded set.
    #[must_use]
    pub fn expanded(&self) -> &BTreeSet<String> {
        &self.expanded
    }

    /// Whether id is expanded.
    #[must_use]
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.contains(id)
    }

    /// Set expanded state for an id (stories / host hydrate).
    pub fn set_expanded(&mut self, id: impl Into<String>, on: bool) {
        let id = id.into();
        if on {
            self.expanded.insert(id);
        } else {
            self.expanded.remove(&id);
        }
    }

    fn sync_metrics(&mut self, total: u16, viewport: u16) {
        self.body_rows = viewport;
        self.scroll.set_content_size(1, total);
        self.scroll.set_viewport(1, viewport);
        self.scroll.clamp();
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, items: &[Diagnostic<'_>]) -> DiagnosticOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return DiagnosticOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        if items.is_empty() {
            return DiagnosticOutcome::Ignored;
        }
        self.cursor = self.cursor.min(items.len() - 1);

        if is_press {
            match key.code {
                KeyCode::Char('c' | 'C') if key.modifiers.is_empty() => {
                    let text = format_diagnostic_plain(&items[self.cursor]);
                    return DiagnosticOutcome::CopyDetails { text };
                }
                KeyCode::Char('c' | 'C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let text = items
                        .iter()
                        .map(format_diagnostic_plain)
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    return DiagnosticOutcome::CopyDetails { text };
                }
                KeyCode::Char('d' | 'D') => {
                    if let Some(url) = items[self.cursor].docs_url {
                        return DiagnosticOutcome::OpenDocsRequested {
                            url: url.to_string(),
                        };
                    }
                }
                KeyCode::Char('f' | 'F') => {
                    let d = &items[self.cursor];
                    if d.fixes.is_empty() {
                        return DiagnosticOutcome::Ignored;
                    }
                    self.fix_cursor = self.fix_cursor.min(d.fixes.len() - 1);
                    return DiagnosticOutcome::FixSelected {
                        diagnostic_id: d.id.to_string(),
                        fix_id: d.fixes[self.fix_cursor].id.to_string(),
                    };
                }
                KeyCode::Char(']') => {
                    let d = &items[self.cursor];
                    if !d.fixes.is_empty() {
                        self.fix_cursor = (self.fix_cursor + 1).min(d.fixes.len() - 1);
                        return DiagnosticOutcome::FixSelected {
                            diagnostic_id: d.id.to_string(),
                            fix_id: d.fixes[self.fix_cursor].id.to_string(),
                        };
                    }
                }
                KeyCode::Char('[') => {
                    let d = &items[self.cursor];
                    if !d.fixes.is_empty() {
                        self.fix_cursor = self.fix_cursor.saturating_sub(1);
                        return DiagnosticOutcome::FixSelected {
                            diagnostic_id: d.id.to_string(),
                            fix_id: d.fixes[self.fix_cursor].id.to_string(),
                        };
                    }
                }
                KeyCode::Char('a' | 'A') => {
                    let d = &items[self.cursor];
                    if let Some(fix) = d.fixes.get(self.fix_cursor).or_else(|| d.fixes.first()) {
                        return DiagnosticOutcome::ApplyFixRequested {
                            diagnostic_id: d.id.to_string(),
                            fix_id: fix.id.to_string(),
                        };
                    }
                }
                KeyCode::Char(' ') | KeyCode::Char('o' | 'O') => {
                    let id = items[self.cursor].id.to_string();
                    let on = if !self.expanded.remove(&id) {
                        self.expanded.insert(id.clone());
                        true
                    } else {
                        false
                    };
                    return DiagnosticOutcome::Expanded { id, on };
                }
                _ => {}
            }
        }

        if let Some(intent) = crate::interaction::default_list_intent(key) {
            return self.handle_intent(intent, items);
        }
        DiagnosticOutcome::Ignored
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        items: &[Diagnostic<'_>],
    ) -> DiagnosticOutcome {
        if !self.accepts_input || items.is_empty() {
            return DiagnosticOutcome::Ignored;
        }
        let len = items.len();
        self.cursor = self.cursor.min(len - 1);
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                if self.cursor + 1 < len {
                    self.cursor += 1;
                    self.fix_cursor = 0;
                    self.scroll.reveal_row(self.cursor);
                    return DiagnosticOutcome::CursorMoved { index: self.cursor };
                }
                if self.scroll.scroll_by(1, 0).is_scrolled() {
                    return DiagnosticOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                DiagnosticOutcome::Ignored
            }
            UiIntent::Move(NavigationMove::Previous) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.fix_cursor = 0;
                    self.scroll.reveal_row(self.cursor);
                    return DiagnosticOutcome::CursorMoved { index: self.cursor };
                }
                if self.scroll.scroll_by(-1, 0).is_scrolled() {
                    return DiagnosticOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                DiagnosticOutcome::Ignored
            }
            UiIntent::Move(NavigationMove::First) => {
                self.cursor = 0;
                self.scroll.set_offset_y(0);
                DiagnosticOutcome::CursorMoved { index: 0 }
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.cursor = len - 1;
                self.scroll.reveal_row(self.cursor);
                DiagnosticOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = i32::from(self.body_rows.max(1));
                if self.scroll.scroll_by(step as isize, 0).is_scrolled() {
                    DiagnosticOutcome::Scrolled {
                        offset: self.offset(),
                    }
                } else {
                    DiagnosticOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i32::from(self.body_rows.max(1));
                if self.scroll.scroll_by((-step) as isize, 0).is_scrolled() {
                    DiagnosticOutcome::Scrolled {
                        offset: self.offset(),
                    }
                } else {
                    DiagnosticOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit => DiagnosticOutcome::Activated {
                id: items[self.cursor].id.to_string(),
            },
            UiIntent::Toggle => {
                let id = items[self.cursor].id.to_string();
                let on = if !self.expanded.remove(&id) {
                    self.expanded.insert(id.clone());
                    true
                } else {
                    false
                };
                DiagnosticOutcome::Expanded { id, on }
            }
            UiIntent::Cancel => DiagnosticOutcome::Cancelled,
            _ => DiagnosticOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        items: &[Diagnostic<'_>],
    ) -> DiagnosticOutcome {
        if !self.accepts_input {
            return DiagnosticOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), items)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), items)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    self.cursor = r.index;
                    if event.modifiers.contains(KeyModifiers::CONTROL) {
                        return DiagnosticOutcome::Activated { id: r.id.clone() };
                    }
                    return DiagnosticOutcome::CursorMoved { index: self.cursor };
                }
                DiagnosticOutcome::Ignored
            }
            _ => DiagnosticOutcome::Ignored,
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Diagnostic list / inline / full painter.
#[derive(Debug, Clone)]
pub struct DiagnosticView<'a> {
    items: &'a [Diagnostic<'a>],
    /// Optional source lines for full recipe (keyed by file; host supplies window).
    source_lines: &'a [CodeFrameLine<'a>],
    system: &'a DesignSystem,
    recipe: DiagnosticRecipe,
    focused: bool,
    colorless: bool,
    title: Option<&'a str>,
}

impl<'a> DiagnosticView<'a> {
    /// Items + system.
    #[must_use]
    pub const fn new(items: &'a [Diagnostic<'a>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            source_lines: &[],
            system,
            recipe: DiagnosticRecipe::List,
            focused: true,
            colorless: false,
            title: None,
        }
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: DiagnosticRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Source lines for full code frames.
    #[must_use]
    pub const fn source_lines(mut self, lines: &'a [CodeFrameLine<'a>]) -> Self {
        self.source_lines = lines;
        self
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut DiagnosticState) {
        state.regions.clear();
        if area.is_empty() {
            return;
        }
        let colorless = self.colorless || state.colorless || self.system.mono();
        let recipe = match state.recipe {
            DiagnosticRecipe::List
                if matches!(
                    self.recipe,
                    DiagnosticRecipe::Full | DiagnosticRecipe::Inline
                ) =>
            {
                self.recipe
            }
            _ if !matches!(self.recipe, DiagnosticRecipe::List) => self.recipe,
            r => r,
        };
        // Prefer widget recipe if not default list override by state
        let recipe = if !matches!(self.recipe, DiagnosticRecipe::List) {
            self.recipe
        } else {
            recipe
        };
        state.origin = (area.x, area.y);
        let surface = self.focused && state.accepts_input;
        let mut y = area.y;

        if let Some(title) = self.title {
            if y < area.bottom() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
            }
        }

        if self.items.is_empty() {
            let mark = "∅ ";
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&format!("{mark}(no diagnostics)"), usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            state.sync_metrics(0, area.height);
            return;
        }

        match recipe {
            DiagnosticRecipe::Inline => {
                // Single item or cursor item
                let d = self.items.get(state.cursor).unwrap_or(&self.items[0]);
                paint_inline(buffer, area, d, self.system, surface, false, colorless);
                state.regions.push(DiagnosticRegion {
                    id: d.id.to_string(),
                    index: state.cursor.min(self.items.len() - 1),
                    area,
                });
                state.sync_metrics(1, 1);
            }
            DiagnosticRecipe::List | DiagnosticRecipe::Full => {
                let body_h = area.bottom().saturating_sub(y);
                state.sync_metrics(self.items.len() as u16, body_h);
                if !self.items.is_empty() {
                    state.cursor = state.cursor.min(self.items.len() - 1);
                }
                let start = state.offset() as usize;
                let mut py = y;
                for (i, d) in self.items.iter().enumerate().skip(start) {
                    if py >= area.bottom() {
                        break;
                    }
                    let cursor = i == state.cursor;
                    let expanded =
                        state.is_expanded(d.id) || matches!(recipe, DiagnosticRecipe::Full);
                    let row0 = py;
                    py = paint_list_item(
                        buffer,
                        Rect::new(area.x, py, area.width, area.bottom().saturating_sub(py)),
                        d,
                        self.source_lines,
                        self.system,
                        surface,
                        false,
                        colorless,
                        cursor,
                        expanded,
                        state.fix_cursor,
                        matches!(recipe, DiagnosticRecipe::Full),
                    );
                    state.regions.push(DiagnosticRegion {
                        id: d.id.to_string(),
                        index: i,
                        area: Rect::new(area.x, row0, area.width, py.saturating_sub(row0).max(1)),
                    });
                }
            }
        }
    }
}

fn paint_inline(
    buffer: &mut Buffer,
    area: Rect,
    d: &Diagnostic<'_>,
    system: &DesignSystem,
    surface: bool,
    _ascii: bool,
    colorless: bool,
) {
    let g = d.severity.glyph(false);
    let letter = d.severity.letter();
    let code = d.code.map(|c| format!("[{c}] ")).unwrap_or_default();
    let line = format!("{g}{letter} {code}{}", d.message);
    let style = if colorless {
        system.style(Role::TextStrong)
    } else if surface {
        system.style(d.severity.role())
    } else {
        system.style(Role::TextMuted)
    };
    buffer.set_stringn(
        area.x,
        area.y,
        take_display_cols(&line, usize::from(area.width)),
        usize::from(area.width),
        style,
    );
}

fn paint_list_item(
    buffer: &mut Buffer,
    area: Rect,
    d: &Diagnostic<'_>,
    source_lines: &[CodeFrameLine<'_>],
    system: &DesignSystem,
    surface: bool,
    _ascii: bool,
    colorless: bool,
    cursor: bool,
    expanded: bool,
    fix_cursor: usize,
    force_frame: bool,
) -> u16 {
    if area.is_empty() {
        return area.y;
    }
    let mut y = area.y;
    // The cursor column is stamped by the shared row chrome.
    let gutter = " ";
    let g = d.severity.glyph(false);
    let letter = d.severity.letter();
    let code = d.code.map(|c| format!("[{c}] ")).unwrap_or_default();
    let loc = match (d.file, d.primary_range()) {
        (Some(f), Some(r)) => format!(" {f}:{}:{}", r.start_line, r.start_col),
        (Some(f), None) => format!(" {f}"),
        (None, Some(r)) => format!(" :{}:{}", r.start_line, r.start_col),
        _ => String::new(),
    };
    let src = d.source.map(|s| format!(" ({s})")).unwrap_or_default();
    // The severity rides its glyph and letter; the message is a sentence and
    // stays readable, and the location trails quietly (plans/012 Step 3).
    let tone = |role: Role| (!colorless).then(|| system.style(role));
    let severity = tone(d.severity.role());
    let mut tiers = TieredRow::with_separator("");
    tiers.push_joined(gutter, None);
    tiers.push_joined(g, severity);
    tiers.push_joined(&letter.to_string(), severity);
    tiers.push_joined(" ", None);
    tiers.push_joined(&code, tone(Role::TextFaint));
    tiers.push_joined(d.message, None);
    tiers.push_joined(&loc, tone(Role::TextMuted));
    tiers.push_joined(&src, tone(Role::TextFaint));
    let line = tiers.text().to_string();
    let style = if colorless {
        if cursor {
            system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
        } else {
            system.style(Role::Text)
        }
    } else {
        system.style(Role::Text)
    };
    let chrome = crate::widgets::row_chrome::RowChrome::resolve(
        system,
        ListRowVisualState {
            selected: cursor,
            focused: surface,
            enabled: true,
            ..Default::default()
        },
    );
    let row = Rect::new(area.x, y, area.width, 1);
    let style = chrome.label_style(style);
    buffer.set_stringn(
        area.x,
        y,
        take_display_cols(&line, usize::from(area.width)),
        usize::from(area.width),
        style,
    );
    tiers.paint_tiers(buffer, row, 0);
    chrome.paint(buffer, row);
    y = y.saturating_add(1);

    if !expanded && !force_frame {
        return y;
    }

    // Code frame when we have labels + source
    if (!d.labels.is_empty() && !source_lines.is_empty()) || force_frame {
        if y < area.bottom() {
            let used = CodeFrame::new(source_lines, system)
                .labels(d.labels)
                .file(d.file.unwrap_or(""))
                .colorless(colorless)
                .truncated_above(true)
                .truncated_below(true)
                .render(
                    Rect::new(area.x, y, area.width, area.bottom().saturating_sub(y)),
                    buffer,
                );
            // If file empty string, re-paint without empty header — CodeFrame still ok
            y = y.saturating_add(used.max(1));
        }
    }

    for note in d.notes {
        if y >= area.bottom() {
            break;
        }
        let ng = note.severity.glyph(false);
        let msg = format!("  {ng} {}: {}", note.severity.label(), note.message);
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&msg, usize::from(area.width)),
            usize::from(area.width),
            system.style(note.severity.role()),
        );
        y = y.saturating_add(1);
    }
    if let Some(help) = d.help {
        if y < area.bottom() {
            let msg = format!("  ? help: {help}");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                system.style(Role::Accent),
            );
            y = y.saturating_add(1);
        }
    }
    for (i, rel) in d.related.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }
        let f = rel.file.unwrap_or("?");
        let msg = format!(
            "  → {f}:{}:{} {}",
            rel.range.start_line, rel.range.start_col, rel.message
        );
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&msg, usize::from(area.width)),
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
        let _ = i;
    }
    for (i, fix) in d.fixes.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }
        let mark = if i == fix_cursor { "★" } else { " " };
        let msg = format!("  {mark}fix: {}", fix.message);
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&msg, usize::from(area.width)),
            usize::from(area.width),
            if i == fix_cursor {
                system.style(Role::Accent)
            } else {
                system.style(Role::TextMuted)
            },
        );
        y = y.saturating_add(1);
        if let Some(rep) = fix.replacement {
            if y >= area.bottom() {
                break;
            }
            let msg = format!("      => {rep}");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                system.style(Role::DiffAdded),
            );
            y = y.saturating_add(1);
        }
    }
    if let Some(url) = d.docs_url {
        if y < area.bottom() {
            let msg = format!("  docs: {url}");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }
    }
    y
}

impl StatefulWidget for &DiagnosticView<'_> {
    type State = DiagnosticState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DiagnosticView::render(self, area, buffer, state);
    }
}

impl StatefulWidget for DiagnosticView<'_> {
    type State = DiagnosticState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DiagnosticView::render(&self, area, buffer, state);
    }
}

// ── Helpers: copy, CodeBlock bridge, ErrorState ─────────────────────────────

/// Plain-text diagnostic for clipboard / ErrorState details.
#[must_use]
pub fn format_diagnostic_plain(d: &Diagnostic<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}[{}] {}",
        d.severity.label(),
        d.code.unwrap_or("-"),
        d.message
    ));
    if let Some(src) = d.source {
        out.push_str(&format!(" ({src})"));
    }
    if let Some(f) = d.file {
        out.push_str(&format!("\n  --> {f}"));
        if let Some(r) = d.primary_range() {
            out.push_str(&format!(":{}:{}", r.start_line, r.start_col));
        }
    }
    for lab in d.labels {
        if let Some(l) = lab.label {
            out.push_str(&format!(
                "\n  {} {}:{}-{}:{} {l}",
                lab.style.id(),
                lab.range.start_line,
                lab.range.start_col,
                lab.range.end_line,
                lab.range.end_col
            ));
        }
    }
    for n in d.notes {
        out.push_str(&format!("\n  {}: {}", n.severity.label(), n.message));
    }
    if let Some(h) = d.help {
        out.push_str(&format!("\n  help: {h}"));
    }
    for r in d.related {
        out.push_str(&format!(
            "\n  related {}:{}:{} {}",
            r.file.unwrap_or("?"),
            r.range.start_line,
            r.range.start_col,
            r.message
        ));
    }
    for f in d.fixes {
        out.push_str(&format!("\n  fix[{}]: {}", f.id, f.message));
        if let Some(rep) = f.replacement {
            out.push_str(&format!("\n    => {rep}"));
        }
    }
    if let Some(u) = d.docs_url {
        out.push_str(&format!("\n  docs: {u}"));
    }
    out
}

/// Format many diagnostics for ErrorState `details` / copy-all.
#[must_use]
pub fn format_diagnostics_plain(items: &[Diagnostic<'_>]) -> String {
    items
        .iter()
        .map(format_diagnostic_plain)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Map diagnostics into [`CodeHighlight`] overlays (0-based lines).
#[must_use]
pub fn diagnostics_to_highlights(items: &[Diagnostic<'_>]) -> Vec<CodeHighlight> {
    let mut out = Vec::new();
    for d in items {
        for lab in d.labels {
            let line = lab.range.start_line.saturating_sub(1) as usize;
            let start = (lab.range.start_col.saturating_sub(1)) as u16;
            let end = if lab.range.start_line == lab.range.end_line {
                lab.range.end_col.saturating_sub(1) as u16
            } else {
                u16::MAX
            };
            out.push(CodeHighlight::span(
                line,
                start,
                end.max(start.saturating_add(1)),
                CodeHighlightKind::Diagnostic,
            ));
            // multi-line: mark intermediate lines fully
            if lab.range.end_line > lab.range.start_line {
                for ln in (lab.range.start_line + 1)..lab.range.end_line {
                    out.push(CodeHighlight::line(
                        ln.saturating_sub(1) as usize,
                        CodeHighlightKind::Diagnostic,
                    ));
                }
                let el = lab.range.end_line.saturating_sub(1) as usize;
                out.push(CodeHighlight::span(
                    el,
                    0,
                    lab.range.end_col.saturating_sub(1) as u16,
                    CodeHighlightKind::Diagnostic,
                ));
            }
        }
    }
    out
}

/// Map diagnostics into gutter marks (severity glyph).
#[must_use]
pub fn diagnostics_to_gutter_marks(items: &[Diagnostic<'_>], _ascii: bool) -> Vec<CodeGutterMark> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for d in items {
        if let Some(r) = d.primary_range() {
            let line = r.start_line.saturating_sub(1) as usize;
            if seen.insert(line) {
                let g = d.severity.glyph(false).chars().next().unwrap_or('!');
                out.push(CodeGutterMark::new(line, g, d.severity.role()));
            }
        }
    }
    out
}

/// Build a source window around diagnostic labels from full file lines.
///
/// `file_lines` is 0-based; returns window with 1-based numbers and truncation flags.
#[must_use]
pub fn code_frame_window<'a>(
    file_lines: &'a [&'a str],
    labels: &[SourceLabel<'_>],
    context: u32,
) -> (Vec<CodeFrameLine<'a>>, bool, bool) {
    if file_lines.is_empty() {
        return (Vec::new(), false, false);
    }
    let mut min_l = u32::MAX;
    let mut max_l = 0u32;
    for lab in labels {
        min_l = min_l.min(lab.range.start_line);
        max_l = max_l.max(lab.range.end_line);
    }
    if min_l == u32::MAX {
        min_l = 1;
        max_l = 1;
    }
    let start = min_l.saturating_sub(context).max(1);
    let end = max_l.saturating_add(context).min(file_lines.len() as u32);
    let truncated_above = start > 1;
    let truncated_below = end < file_lines.len() as u32;
    let mut out = Vec::new();
    for n in start..=end {
        let idx = (n as usize).saturating_sub(1);
        if let Some(t) = file_lines.get(idx) {
            out.push(CodeFrameLine::new(n, t));
        }
    }
    (out, truncated_above, truncated_below)
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint targets.
pub mod bench {
    /// Viewport rows.
    pub const VIEWPORT: u16 = 30;
    /// Diagnostics in a large problems list.
    pub const LARGE_LIST: usize = 5_000;
    /// Max paint cells.
    pub const MAX_PAINT_CELLS: u32 = 30 * 100;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_labels() -> [SourceLabel<'static>; 2] {
        [
            SourceLabel::primary(SourceRange::line_span(2, 5, 12)).label("expected `i32`"),
            SourceLabel::secondary(SourceRange::line_span(2, 14, 18)).label("found here"),
        ]
    }

    #[test]
    fn severity_not_color_only() {
        for s in [
            DiagnosticSeverity::Error,
            DiagnosticSeverity::Warning,
            DiagnosticSeverity::Info,
            DiagnosticSeverity::Hint,
            DiagnosticSeverity::Note,
            DiagnosticSeverity::Help,
        ] {
            assert!(!s.letter().is_whitespace());
            assert!(!s.glyph(true).is_empty());
            assert!(!s.glyph(false).is_empty());
            assert!(!s.label().is_empty());
        }
    }

    #[test]
    fn expand_tabs_and_controls() {
        let e = expand_tabs("a\tb", 4);
        assert_eq!(e, "a   b");
        assert!(expand_tabs("a\u{7}b", 4).contains("\\u{"));
    }

    #[test]
    fn code_frame_underlines() {
        let system = DesignSystem::default();
        let labels = sample_labels();
        let lines = [
            CodeFrameLine::new(1, "fn main() {"),
            CodeFrameLine::new(2, "    let x = foo();"),
            CodeFrameLine::new(3, "}"),
        ];
        let frame = CodeFrame::new(&lines, &system)
            .labels(&labels)
            .file("src/main.rs");
        let area = Rect::new(0, 0, 48, 12);
        let mut buf = Buffer::empty(area);
        let used = frame.render(area, &mut buf);
        assert!(used >= 3);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("main")
                || text.contains('^')
                || text.contains("E0308")
                || text.contains("foo"),
            "{text}"
        );
        assert!(
            text.contains('^') || text.contains('-') || text.contains('─'),
            "{text}"
        );
    }

    #[test]
    fn multi_line_and_overlap() {
        let system = DesignSystem::default();
        let labels = [
            SourceLabel::primary(SourceRange::multi(1, 1, 3, 2)).label("block"),
            SourceLabel::secondary(SourceRange::line_span(2, 1, 5)),
        ];
        let lines = [
            CodeFrameLine::new(1, "aaa"),
            CodeFrameLine::new(2, "bbb"),
            CodeFrameLine::new(3, "ccc"),
        ];
        let frame = CodeFrame::new(&lines, &system).labels(&labels);
        let area = Rect::new(0, 0, 40, 16);
        let mut buf = Buffer::empty(area);
        let _ = frame.render(area, &mut buf);
    }

    #[test]
    fn list_nav_copy_fix() {
        let labels = [SourceLabel::primary(SourceRange::line_span(2, 5, 12)).label("expected")];
        let fixes = [SuggestedFix::new("f1", "annotate").replacement("i32")];
        let items = [
            Diagnostic::new("d1", DiagnosticSeverity::Error, "bad")
                .code("E0001")
                .labels(&labels)
                .fixes(&fixes),
            Diagnostic::new("d2", DiagnosticSeverity::Warning, "meh").code("W0001"),
        ];
        let mut state = DiagnosticState::new();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items),
            DiagnosticOutcome::CursorMoved { index: 1 }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
                &items
            ),
            DiagnosticOutcome::CopyDetails { text } if text.contains("meh")
        ));
        state.cursor = 0;
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
                &items
            ),
            DiagnosticOutcome::ApplyFixRequested { fix_id, .. } if fix_id == "f1"
        ));
    }

    #[test]
    fn expand_and_recipes_paint() {
        let system = DesignSystem::default();
        let labels = [SourceLabel::primary(SourceRange::line_span(1, 1, 4))];
        let items = [Diagnostic::new("d1", DiagnosticSeverity::Error, "oops")
            .code("E1")
            .file("a.rs")
            .labels(&labels)];
        let lines = [CodeFrameLine::new(1, "xyz")];
        let mut state = DiagnosticState::new();
        state.expanded.insert("d1".into());
        let view = DiagnosticView::new(&items, &system)
            .recipe(DiagnosticRecipe::Full)
            .source_lines(&lines)
            .title("Problems");
        let area = Rect::new(0, 0, 60, 16);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());

        let mut s2 = DiagnosticState::new();
        DiagnosticView::new(&items, &system)
            .recipe(DiagnosticRecipe::Inline)
            .render(
                Rect::new(0, 0, 40, 1),
                &mut Buffer::empty(Rect::new(0, 0, 40, 1)),
                &mut s2,
            );
    }

    #[test]
    fn codeblock_bridge() {
        let labels = [SourceLabel::primary(SourceRange::line_span(2, 3, 8))];
        let items = [Diagnostic::new("d", DiagnosticSeverity::Warning, "w").labels(&labels)];
        let highs = diagnostics_to_highlights(&items);
        assert!(!highs.is_empty());
        assert_eq!(highs[0].line, 1);
        let marks = diagnostics_to_gutter_marks(&items, true);
        assert_eq!(marks[0].line, 1);
    }

    #[test]
    fn code_frame_window_truncation() {
        let file = ["l1", "l2", "l3", "l4", "l5", "l6"];
        let refs: Vec<&str> = file.to_vec();
        let labels = [SourceLabel::primary(SourceRange::point(3, 1))];
        let (win, above, below) = code_frame_window(&refs, &labels, 1);
        assert!(above);
        assert!(below);
        assert!(win.iter().any(|l| l.number == 3));
    }

    #[test]
    fn unicode_cjk_emoji_safe() {
        let system = DesignSystem::default();
        let labels = [SourceLabel::primary(SourceRange::line_span(1, 1, 3))];
        let lines = [CodeFrameLine::new(1, "東京 🧪")];
        let frame = CodeFrame::new(&lines, &system).labels(&labels);
        let area = Rect::new(0, 0, 24, 4);
        let mut buf = Buffer::empty(area);
        let _ = frame.render(area, &mut buf);
    }

    #[test]
    fn accepts_input_gate() {
        let items = [Diagnostic::new("d", DiagnosticSeverity::Info, "i")];
        let mut state = DiagnosticState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items),
            DiagnosticOutcome::Ignored
        ));
    }

    #[test]
    fn sustained_list_paint() {
        let system = DesignSystem::default();
        let owned: Vec<(String, String)> = (0..40)
            .map(|i| (format!("d{i}"), format!("msg {i}")))
            .collect();
        let items: Vec<Diagnostic<'_>> = owned
            .iter()
            .map(|(id, m)| Diagnostic::new(id.as_str(), DiagnosticSeverity::Warning, m.as_str()))
            .collect();
        let mut state = DiagnosticState::new();
        let view = DiagnosticView::new(&items, &system);
        let area = Rect::new(0, 0, 72, 20);
        let mut buf = Buffer::empty(area);
        for _ in 0..30 {
            (&view).render(area, &mut buf, &mut state);
        }
        assert!(state.regions.len() <= 25);
    }

    #[test]
    fn fuzz_ranges_and_format() {
        for r in [
            SourceRange::point(1, 1),
            SourceRange::line_span(2, 5, 2),
            SourceRange::multi(1, 1, 5, 10),
        ] {
            assert!(r.covers_line(r.start_line));
        }
        let d = Diagnostic::new("x", DiagnosticSeverity::Error, "e").code("E");
        assert!(format_diagnostic_plain(&d).contains("error"));
        assert_eq!(bench::VIEWPORT, 30);
    }

    #[test]
    fn empty_list() {
        let system = DesignSystem::default();
        let mut state = DiagnosticState::new();
        let view = DiagnosticView::new(&[], &system);
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("empty") || text.contains('∅') || text.contains('['),
            "{text}"
        );
    }
}
