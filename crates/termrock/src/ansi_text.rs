// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Safe ANSI parse + paint for TermRock surfaces.
//!
//! **Mission.** Ingest untrusted terminal output (agent tools, PTY logs, CI)
//! without letting escape sequences leak to the **host** terminal. Parse SGR
//! styles, resets, CR/BS/tabs, OSC 8 hyperlinks, and malformed sequences into
//! owned display structures; paint via DesignSystem / ratatui spans only.
//!
//! **Not responsible for:** host PTY lifecycle, scroll chrome policy (use
//! [`crate::widgets::LogStream`] / [`crate::widgets::LogPane`]), or full VT100
//! emulator modes (cursor save, alternate screen, DECCKM, …).
//!
//! References: terminal emulators, ansi-to-tui, Rich, agent command panes.

use std::collections::VecDeque;

use anstyle_parse::{DefaultCharAccumulator, Params, Parser, Perform};
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

use crate::style::{ColorCapability, DesignSystem, Role, quantize_color};
use crate::text::{display_cols, expand_tabs, take_display_cols};

// ── Public free functions (preserved) ───────────────────────────────────────

/// Removes ANSI escape sequences while preserving printable bytes and basic whitespace.
///
/// Control bytes that affect host terminals (ESC, CSI, OSC, DCS, …) are dropped.
/// Keeps `\n`, `\r`, `\t` so strip results remain useful for plain logs.
#[must_use]
pub fn strip_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut parser = Parser::<DefaultCharAccumulator>::default();
    let mut performer = PlainPerformer { output: Vec::new() };
    for &byte in bytes {
        parser.advance(&mut performer, byte);
    }
    performer.output
}

/// UTF-8 strip helper.
#[must_use]
pub fn strip_str(input: &str) -> String {
    String::from_utf8_lossy(&strip_bytes(input.as_bytes())).into_owned()
}

/// Parses supported ANSI SGR sequences into owned styled spans.
///
/// Escape sequences never appear in span content (injection-safe for paint).
pub fn styled_spans(input: &str, default_style: Style) -> Vec<Span<'static>> {
    parse_to_line(
        input,
        &AnsiParseOptions::default().with_default_style(default_style),
    )
    .to_spans()
}

/// Parses ANSI SGR text into one owned line for append-time ingestion.
///
/// Parse once before appending to a scrollback buffer; rendering the returned
/// line does not re-run the ANSI parser.
#[must_use]
pub fn line_from_ansi(input: &str, default_style: Style) -> Line<'static> {
    Line::from(styled_spans(input, default_style))
}

/// Parse with full options (CR/BS, hyperlinks, no-color, tabs).
#[must_use]
pub fn parse_to_line(input: &str, options: &AnsiParseOptions) -> AnsiLine {
    let mut stream = AnsiStream::new(options.clone());
    stream.feed_str(input);
    stream.finish_line();
    stream
        .lines()
        .back()
        .cloned()
        .unwrap_or_else(|| AnsiLine::empty(options.default_style))
}

/// Parse multi-line input into completed lines (trailing partial kept only if
/// no final newline — call [`AnsiStream`] for true streaming).
#[must_use]
pub fn parse_lines(input: &str, options: &AnsiParseOptions) -> Vec<AnsiLine> {
    let mut stream = AnsiStream::new(options.clone());
    stream.feed_str(input);
    if !input.ends_with('\n') && !input.ends_with('\r') {
        stream.finish_line();
    }
    stream.into_lines()
}

// ── Options ─────────────────────────────────────────────────────────────────

/// Parse / paint policy.
#[derive(Debug, Clone, PartialEq)]
pub struct AnsiParseOptions {
    /// Base style when SGR resets or at stream start.
    pub default_style: Style,
    /// Tab stop width (0 → 4).
    pub tab_width: u8,
    /// Drop all color; keep bold/dim/underline/italic as non-color cues.
    pub no_color: bool,
    /// Capture OSC 8 hyperlinks onto segments.
    pub hyperlinks: bool,
    /// Apply CR (cursor home) and BS (backspace) on the current line.
    pub apply_cr_bs: bool,
    /// Expand tabs to spaces in segment text (recommended for paint).
    pub expand_tabs: bool,
}

impl Default for AnsiParseOptions {
    fn default() -> Self {
        Self {
            default_style: Style::default(),
            tab_width: 4,
            no_color: false,
            hyperlinks: true,
            apply_cr_bs: true,
            expand_tabs: true,
        }
    }
}

impl AnsiParseOptions {
    /// Builder default style.
    #[must_use]
    pub const fn with_default_style(mut self, style: Style) -> Self {
        self.default_style = style;
        self
    }

    /// Force monochrome SGR (modifiers only).
    #[must_use]
    pub const fn no_color(mut self, on: bool) -> Self {
        self.no_color = on;
        self
    }

    /// From DesignSystem capability.
    #[must_use]
    pub fn for_system(system: &DesignSystem) -> Self {
        Self {
            default_style: system.style(Role::Text),
            no_color: matches!(system.capability, ColorCapability::Monochrome),
            ..Self::default()
        }
    }

    /// Strip mode: ignore colors and hyperlinks (plain text path).
    #[must_use]
    pub const fn strip_only(mut self) -> Self {
        self.no_color = true;
        self.hyperlinks = false;
        self
    }
}

// ── Parsed model ────────────────────────────────────────────────────────────

/// One styled run (owned; safe to append to history).
#[derive(Debug, Clone, PartialEq)]
pub struct AnsiSegment {
    /// Display text (tabs expanded when option set; no ESC bytes).
    pub text: String,
    /// Resolved style (already no-color filtered when requested).
    pub style: Style,
    /// Active OSC 8 URL when hyperlinks enabled.
    pub href: Option<String>,
}

impl AnsiSegment {
    /// Plain segment.
    #[must_use]
    pub fn plain(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
            href: None,
        }
    }
}

/// One logical output line after CR/BS resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct AnsiLine {
    /// Styled segments covering the line left-to-right.
    pub segments: Vec<AnsiSegment>,
    /// Copy-safe plain text (no styles, no ESC).
    pub plain: String,
}

impl AnsiLine {
    /// Empty line.
    #[must_use]
    pub fn empty(style: Style) -> Self {
        let _ = style;
        Self {
            segments: Vec::new(),
            plain: String::new(),
        }
    }

    /// Ratatui spans for append-once ingestion (LogPane / LogStream).
    #[must_use]
    pub fn to_spans(&self) -> Vec<Span<'static>> {
        if self.segments.is_empty() {
            return vec![Span::raw("")];
        }
        self.segments
            .iter()
            .map(|s| Span::styled(s.text.clone(), s.style))
            .collect()
    }

    /// Owned ratatui line.
    #[must_use]
    pub fn to_line(&self) -> Line<'static> {
        Line::from(self.to_spans())
    }

    /// Whether any segment carries a hyperlink.
    #[must_use]
    pub fn has_hyperlinks(&self) -> bool {
        self.segments.iter().any(|s| s.href.is_some())
    }

    /// Hyperlink regions as (label, url) for host activation.
    #[must_use]
    pub fn hyperlinks(&self) -> Vec<(String, String)> {
        self.segments
            .iter()
            .filter_map(|s| s.href.as_ref().map(|u| (s.text.clone(), u.clone())))
            .collect()
    }

    /// Display column width of plain text.
    #[must_use]
    pub fn width(&self) -> usize {
        display_cols(&self.plain)
    }
}

// ── Streaming parser ────────────────────────────────────────────────────────

/// Incremental ANSI stream with bounded completed-line history.
///
/// Feed bytes as they arrive from a PTY or tool. Completed lines (on `\n`) are
/// pushed into a ring; incomplete line state survives across feeds.
#[derive(Debug, Clone)]
pub struct AnsiStream {
    options: AnsiParseOptions,
    lines: VecDeque<AnsiLine>,
    max_lines: Option<usize>,
    /// Incomplete line cells (CR/BS model).
    cells: Vec<LineCell>,
    cursor: usize,
    style: Style,
    href: Option<String>,
    /// Carry parser? anstyle_parse Parser is not Clone easily — reparse approach:
    /// we use a fresh Parser each feed but keep line cell state. Truncated CSI
    /// at feed boundary is handled by keeping a byte buffer.
    pending: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
struct LineCell {
    ch: char,
    style: Style,
    href: Option<String>,
}

impl AnsiStream {
    /// New unbounded stream.
    #[must_use]
    pub fn new(options: AnsiParseOptions) -> Self {
        let style = options.default_style;
        Self {
            options,
            lines: VecDeque::new(),
            max_lines: None,
            cells: Vec::new(),
            cursor: 0,
            style,
            href: None,
            pending: Vec::new(),
        }
    }

    /// Bounded completed-line history (oldest dropped).
    #[must_use]
    pub fn with_max_lines(mut self, max: usize) -> Self {
        self.max_lines = Some(max.max(1));
        self
    }

    /// Options borrow.
    #[must_use]
    pub const fn options(&self) -> &AnsiParseOptions {
        &self.options
    }

    /// Completed lines (oldest first).
    #[must_use]
    pub fn lines(&self) -> &VecDeque<AnsiLine> {
        &self.lines
    }

    /// Incomplete line plain preview.
    #[must_use]
    pub fn pending_plain(&self) -> String {
        self.cells.iter().map(|c| c.ch).collect()
    }

    /// Whether the current line has unflushed cells.
    #[must_use]
    pub fn has_partial(&self) -> bool {
        !self.cells.is_empty() || !self.pending.is_empty()
    }

    /// Feed raw bytes (may include partial escape at end).
    pub fn feed(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        // Concatenate with pending truncated sequence bytes
        let mut buf = std::mem::take(&mut self.pending);
        buf.extend_from_slice(bytes);
        // Keep a trailing incomplete ESC sequence in pending when needed.
        let (consume, hold) = split_complete_prefix(&buf);
        self.pending = hold;
        if consume.is_empty() {
            return;
        }
        self.feed_complete(&consume);
    }

    fn feed_complete(&mut self, bytes: &[u8]) {
        // Local state for Perform
        let mut style = self.style;
        let mut href = self.href.clone();
        let mut cells = std::mem::take(&mut self.cells);
        let mut cursor = self.cursor;
        let options = self.options.clone();
        let mut completed = Vec::new();

        {
            let mut performer = CellPerformer {
                options: &options,
                style: &mut style,
                href: &mut href,
                cells: &mut cells,
                cursor: &mut cursor,
                completed: &mut completed,
            };
            let mut parser = Parser::<DefaultCharAccumulator>::default();
            for &b in bytes {
                parser.advance(&mut performer, b);
            }
        }

        self.style = style;
        self.href = href;
        self.cells = cells;
        self.cursor = cursor;
        for line in completed {
            self.push_line(line);
        }
    }

    /// UTF-8 convenience.
    pub fn feed_str(&mut self, s: &str) {
        self.feed(s.as_bytes());
    }

    /// Force-complete the current line (EOF / flush).
    pub fn finish_line(&mut self) {
        if self.cells.is_empty() && self.pending.is_empty() {
            return;
        }
        // Drop incomplete escape pending (do not inject into host)
        self.pending.clear();
        let line = cells_to_line(&self.cells, &self.options);
        self.cells.clear();
        self.cursor = 0;
        self.push_line(line);
    }

    /// Reset style/href to defaults; clear partial line (keeps history).
    pub fn reset_style(&mut self) {
        self.style = self.options.default_style;
        self.href = None;
    }

    /// Clear history and partial state.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.cells.clear();
        self.cursor = 0;
        self.pending.clear();
        self.reset_style();
    }

    /// Drain completed lines into a Vec.
    #[must_use]
    pub fn into_lines(mut self) -> Vec<AnsiLine> {
        self.finish_line();
        self.lines.into_iter().collect()
    }

    /// Take completed lines, leave stream ready for more feed.
    pub fn drain_lines(&mut self) -> Vec<AnsiLine> {
        self.lines.drain(..).collect()
    }

    /// Append completed lines to a LogPane-style sink via callback.
    pub fn drain_into_lines<F>(&mut self, mut f: F)
    where
        F: FnMut(Line<'static>),
    {
        for line in self.drain_lines() {
            f(line.to_line());
        }
    }

    /// Plain history + pending for copy.
    #[must_use]
    pub fn plain(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(&line.plain);
        }
        let pending = self.pending_plain();
        if !pending.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&pending);
        }
        out
    }

    fn push_line(&mut self, line: AnsiLine) {
        self.lines.push_back(line);
        if let Some(max) = self.max_lines {
            while self.lines.len() > max {
                self.lines.pop_front();
            }
        }
    }
}

// ── Performer with CR/BS/OSC ────────────────────────────────────────────────

struct CellPerformer<'a> {
    options: &'a AnsiParseOptions,
    style: &'a mut Style,
    href: &'a mut Option<String>,
    cells: &'a mut Vec<LineCell>,
    cursor: &'a mut usize,
    completed: &'a mut Vec<AnsiLine>,
}

impl CellPerformer<'_> {
    fn put_char(&mut self, c: char) {
        if c == '\n' {
            self.finish_line();
            return;
        }
        if c == '\r' {
            if self.options.apply_cr_bs {
                *self.cursor = 0;
            } else {
                // treat as printable discard for host safety — don't emit raw CR to paint
            }
            return;
        }
        if c == '\t' {
            let tab_w = if self.options.tab_width == 0 {
                4
            } else {
                usize::from(self.options.tab_width)
            };
            let col = self.cursor_cols();
            let spaces = tab_w - (col % tab_w);
            for _ in 0..spaces {
                self.write_cell(' ');
            }
            return;
        }
        if c == '\u{08}' {
            // BS already handled in execute
            return;
        }
        self.write_cell(c);
    }

    fn cursor_cols(&self) -> usize {
        self.cells
            .iter()
            .take(*self.cursor)
            .map(|c| unicode_width::UnicodeWidthChar::width(c.ch).unwrap_or(0))
            .sum()
    }

    fn write_cell(&mut self, ch: char) {
        let style = filter_style(*self.style, self.options.no_color);
        let cell = LineCell {
            ch,
            style,
            href: self.href.clone(),
        };
        if *self.cursor >= self.cells.len() {
            self.cells.push(cell);
            *self.cursor = self.cells.len();
        } else {
            self.cells[*self.cursor] = cell;
            *self.cursor += 1;
        }
    }

    fn backspace(&mut self) {
        if !self.options.apply_cr_bs {
            return;
        }
        if *self.cursor == 0 {
            return;
        }
        *self.cursor -= 1;
        // Erase character under new cursor (classic BS + overwrite model uses
        // destructive BS when combined with DEL; we erase one cell).
        if *self.cursor < self.cells.len() {
            self.cells.remove(*self.cursor);
        }
    }

    fn finish_line(&mut self) {
        let line = cells_to_line(self.cells, self.options);
        self.cells.clear();
        *self.cursor = 0;
        self.completed.push(line);
    }
}

impl Perform for CellPerformer<'_> {
    fn print(&mut self, c: char) {
        self.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => self.finish_line(),
            b'\r' => {
                if self.options.apply_cr_bs {
                    *self.cursor = 0;
                }
            }
            b'\t' => self.put_char('\t'),
            0x08 => self.backspace(), // BS
            0x7f => {
                // DEL — ignore or erase at cursor
                if self.options.apply_cr_bs && *self.cursor < self.cells.len() {
                    self.cells.remove(*self.cursor);
                }
            }
            // BEL, other C0 — drop (never forward to host)
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: u8) {
        if action != b'm' {
            // Other CSI (cursor moves, erase) — intentionally ignored for
            // safety inside embedded surfaces (no host terminal side effects).
            return;
        }
        apply_sgr(
            params,
            self.style,
            self.options.default_style,
            self.options.no_color,
        );
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if !self.options.hyperlinks {
            return;
        }
        // OSC 8 ; params ; URI
        if params.is_empty() {
            return;
        }
        let p0 = params[0];
        if p0 != b"8" {
            return;
        }
        // Close: OSC 8 ; ; ST  → params [b"8"] or [b"8", b"", b""]
        if params.len() < 3 {
            *self.href = None;
            return;
        }
        let uri = params[2];
        if uri.is_empty() {
            *self.href = None;
            return;
        }
        // Only allow safe schemes (same policy as osc::encode_hyperlink_open)
        if let Ok(s) = std::str::from_utf8(uri) {
            if hyperlink_uri_allowed(s) {
                *self.href = Some(s.to_string());
            } else {
                *self.href = None;
            }
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        // Drop all ESC final sequences (no host leak)
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: u8) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
}

// ── Plain strip performer ───────────────────────────────────────────────────

struct PlainPerformer {
    output: Vec<u8>,
}

impl Perform for PlainPerformer {
    fn print(&mut self, c: char) {
        let mut buf = [0u8; 4];
        self.output
            .extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }

    fn execute(&mut self, byte: u8) {
        if matches!(byte, b'\n' | b'\r' | b'\t') {
            self.output.push(byte);
        }
        // BS not preserved in strip (optional); keep for plain logs? drop.
    }
}

// ── Legacy one-shot styled performer (used only via parse path) ─────────────
// Kept logic unified through AnsiStream.

// ── SGR ─────────────────────────────────────────────────────────────────────

fn apply_sgr(params: &Params, style: &mut Style, default_style: Style, no_color: bool) {
    let mut values: Vec<u16> = params.iter().flatten().copied().collect();
    if values.is_empty() {
        values.push(0);
    }
    let mut i = 0;
    while i < values.len() {
        let value = values[i];
        match value {
            0 => *style = default_style,
            1 => *style = style.add_modifier(Modifier::BOLD),
            2 => *style = style.add_modifier(Modifier::DIM),
            3 => *style = style.add_modifier(Modifier::ITALIC),
            4 => *style = style.add_modifier(Modifier::UNDERLINED),
            7 => *style = style.add_modifier(Modifier::REVERSED),
            8 => *style = style.add_modifier(Modifier::HIDDEN),
            9 => *style = style.add_modifier(Modifier::CROSSED_OUT),
            22 => *style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => *style = style.remove_modifier(Modifier::ITALIC),
            24 => *style = style.remove_modifier(Modifier::UNDERLINED),
            27 => *style = style.remove_modifier(Modifier::REVERSED),
            28 => *style = style.remove_modifier(Modifier::HIDDEN),
            29 => *style = style.remove_modifier(Modifier::CROSSED_OUT),
            30..=37 => {
                if !no_color {
                    *style = style.fg(ansi_color(value - 30, false));
                }
            }
            39 => {
                *style = style.fg(default_style.fg.unwrap_or(Color::Reset));
            }
            40..=47 => {
                if !no_color {
                    *style = style.bg(ansi_color(value - 40, false));
                }
            }
            49 => {
                *style = style.bg(default_style.bg.unwrap_or(Color::Reset));
            }
            90..=97 => {
                if !no_color {
                    *style = style.fg(ansi_color(value - 90, true));
                }
            }
            100..=107 => {
                if !no_color {
                    *style = style.bg(ansi_color(value - 100, true));
                }
            }
            38 | 48 => {
                if let Some((color, consumed)) = parse_extended_color(&values[i + 1..]) {
                    if !no_color {
                        *style = if value == 38 {
                            style.fg(color)
                        } else {
                            style.bg(color)
                        };
                    }
                    i += consumed;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if no_color {
        *style = filter_style(*style, true);
    }
}

fn filter_style(mut style: Style, no_color: bool) -> Style {
    if no_color {
        style = Style {
            fg: None,
            bg: None,
            ..style
        };
    }
    style
}

const fn ansi_color(index: u16, bright: bool) -> Color {
    match (index, bright) {
        (0, false) => Color::Black,
        (1, false) => Color::Red,
        (2, false) => Color::Green,
        (3, false) => Color::Yellow,
        (4, false) => Color::Blue,
        (5, false) => Color::Magenta,
        (6, false) => Color::Cyan,
        (7, false) => Color::Gray,
        (0, true) => Color::DarkGray,
        (1, true) => Color::LightRed,
        (2, true) => Color::LightGreen,
        (3, true) => Color::LightYellow,
        (4, true) => Color::LightBlue,
        (5, true) => Color::LightMagenta,
        (6, true) => Color::LightCyan,
        (7, true) => Color::White,
        _ => Color::Reset,
    }
}

fn parse_extended_color(values: &[u16]) -> Option<(Color, usize)> {
    match values {
        [5, idx, ..] => Some((Color::Indexed((*idx).min(255) as u8), 2)),
        [2, r, g, b, ..] => Some((
            Color::Rgb(
                (*r).min(255) as u8,
                (*g).min(255) as u8,
                (*b).min(255) as u8,
            ),
            4,
        )),
        _ => None,
    }
}

fn hyperlink_uri_allowed(url: &str) -> bool {
    let lower = url.as_bytes();
    // Match osc encode policy: http(s), mailto, file
    starts_with_ignore_ascii_case(lower, b"https://")
        || starts_with_ignore_ascii_case(lower, b"http://")
        || starts_with_ignore_ascii_case(lower, b"mailto:")
        || starts_with_ignore_ascii_case(lower, b"file:")
}

fn starts_with_ignore_ascii_case(hay: &[u8], prefix: &[u8]) -> bool {
    hay.len() >= prefix.len() && hay[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn cells_to_line(cells: &[LineCell], options: &AnsiParseOptions) -> AnsiLine {
    if cells.is_empty() {
        return AnsiLine::empty(options.default_style);
    }
    let mut segments = Vec::new();
    let mut plain = String::with_capacity(cells.len());
    let mut cur_text = String::new();
    let mut cur_style = cells[0].style;
    let mut cur_href = cells[0].href.clone();

    let flush =
        |text: &mut String, style: Style, href: &Option<String>, segs: &mut Vec<AnsiSegment>| {
            if text.is_empty() {
                return;
            }
            let mut t = std::mem::take(text);
            if options.expand_tabs {
                t = expand_tabs(&t, usize::from(options.tab_width.max(1)));
            }
            segs.push(AnsiSegment {
                text: t,
                style,
                href: href.clone(),
            });
        };

    for cell in cells {
        plain.push(cell.ch);
        if cell.style != cur_style || cell.href != cur_href {
            flush(&mut cur_text, cur_style, &cur_href, &mut segments);
            cur_style = cell.style;
            cur_href = cell.href.clone();
        }
        cur_text.push(cell.ch);
    }
    flush(&mut cur_text, cur_style, &cur_href, &mut segments);

    if options.expand_tabs {
        plain = expand_tabs(&plain, usize::from(options.tab_width.max(1)));
    }

    AnsiLine { segments, plain }
}

/// Split buffer so incomplete ESC/CSI/OSC at the end is held for next feed.
fn split_complete_prefix(buf: &[u8]) -> (Vec<u8>, Vec<u8>) {
    if buf.is_empty() {
        return (Vec::new(), Vec::new());
    }
    // Find last ESC; if sequence after it is incomplete, hold from that ESC.
    if let Some(esc_pos) = buf.iter().rposition(|&b| b == 0x1b) {
        let tail = &buf[esc_pos..];
        if incomplete_escape(tail) {
            return (buf[..esc_pos].to_vec(), tail.to_vec());
        }
    }
    (buf.to_vec(), Vec::new())
}

fn incomplete_escape(seq: &[u8]) -> bool {
    if seq.is_empty() || seq[0] != 0x1b {
        return false;
    }
    if seq.len() == 1 {
        return true;
    }
    // CSI: ESC [
    if seq[1] == b'[' {
        // complete when final byte 0x40-0x7E appears after [
        return !seq[2..].iter().any(|&b| (0x40..=0x7e).contains(&b));
    }
    // OSC: ESC ]
    if seq[1] == b']' {
        // complete on BEL or ST (ESC \)
        if seq.contains(&0x07) {
            return false;
        }
        if seq.windows(2).any(|w| w == [0x1b, b'\\']) {
            return false;
        }
        return true;
    }
    // Other ESC: treat as complete once we have ESC + final (len>=2)
    false
}

// ── Widget: AnsiText ────────────────────────────────────────────────────────

/// Paint policy for embedded ANSI output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AnsiTextMode {
    /// Full SGR colors (within DesignSystem capability at host edge).
    #[default]
    Color,
    /// Strip colors; keep modifiers.
    NoColor,
    /// Strip all styling — plain text only.
    Plain,
}

impl AnsiTextMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::NoColor => "no-color",
            Self::Plain => "plain",
        }
    }
}

/// Interaction state for multi-line ANSI views.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnsiTextState {
    /// First visible line index.
    pub scroll_y: usize,
    /// Focus for keyboard scroll.
    pub focused: bool,
    /// Viewport height from last paint.
    viewport: u16,
    total: usize,
}

impl AnsiTextState {
    /// Fresh.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scroll_y: 0,
            focused: false,
            viewport: 0,
            total: 0,
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Scroll clamp.
    pub fn clamp(&mut self) {
        let max = self.total.saturating_sub(usize::from(self.viewport.max(1)));
        if self.scroll_y > max {
            self.scroll_y = max;
        }
    }

    /// Scroll by lines.
    pub fn scroll_by(&mut self, delta: isize) -> bool {
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
        self.clamp();
        before != self.scroll_y
    }
}

/// Widget: paint pre-parsed lines or raw ANSI source (reparsed each paint —
/// prefer [`AnsiStream`] for streaming ingest).
#[derive(Debug, Clone, Copy)]
pub struct AnsiText<'a> {
    lines: &'a [AnsiLine],
    system: &'a DesignSystem,
    mode: AnsiTextMode,
    first: usize,
}

impl<'a> AnsiText<'a> {
    /// From pre-parsed lines (preferred for LogStream / TerminalOutput).
    #[must_use]
    pub const fn lines(lines: &'a [AnsiLine], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            system,
            mode: AnsiTextMode::Color,
            first: 0,
        }
    }

    /// Paint mode.
    #[must_use]
    pub const fn mode(mut self, mode: AnsiTextMode) -> Self {
        self.mode = mode;
        self
    }

    /// First visible line (Widget path).
    #[must_use]
    pub const fn first(mut self, first: usize) -> Self {
        self.first = first;
        self
    }

    /// Line count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.lines.len()
    }

    /// Empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Plain join for copy.
    #[must_use]
    pub fn plain(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.plain.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Paint with scroll state.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut AnsiTextState) {
        if area.is_empty() {
            return;
        }
        state.total = self.lines.len();
        state.viewport = area.height;
        if state.scroll_y == 0 && self.first > 0 {
            state.scroll_y = self.first;
        }
        state.clamp();
        let start = state.scroll_y;
        for row in 0..area.height {
            let idx = start.saturating_add(usize::from(row));
            let Some(line) = self.lines.get(idx) else {
                break;
            };
            let y = area.y.saturating_add(row);
            self.paint_line(line, area.x, y, area.width, buffer);
        }
    }

    fn paint_line(&self, line: &AnsiLine, x: u16, y: u16, width: u16, buffer: &mut Buffer) {
        if width == 0 {
            return;
        }
        if matches!(self.mode, AnsiTextMode::Plain) {
            let clipped = take_display_cols(&line.plain, usize::from(width));
            buffer.set_stringn(
                x,
                y,
                &clipped,
                usize::from(width),
                self.system.style(Role::Text),
            );
            return;
        }
        let mut col = 0u16;
        for seg in &line.segments {
            if col >= width {
                break;
            }
            let mut style = seg.style;
            if matches!(self.mode, AnsiTextMode::NoColor) {
                style = filter_style(style, true);
                // Ensure some non-color cue if segment had color
                if style.add_modifier == Modifier::empty() && seg.style.fg.is_some() {
                    style = style.add_modifier(Modifier::BOLD);
                }
            }
            // Prefer default text when style fully empty
            if style.fg.is_none() && style.bg.is_none() && style.add_modifier == Modifier::empty() {
                style = self.system.style(Role::Text);
            }
            if seg.href.is_some() {
                style = style
                    .fg(self.system.style(Role::Link).fg.unwrap_or(Color::Blue))
                    .add_modifier(Modifier::UNDERLINED);
            }
            // Embedded output is data, not a theme escape hatch. Preserve the
            // source hue as closely as the named terminal palette permits,
            // but never let RGB or indexed colors bypass the ANSI-16 runtime
            // contract at the paint edge.
            style = if matches!(self.mode, AnsiTextMode::NoColor)
                || matches!(self.system.capability, ColorCapability::Monochrome)
            {
                monochrome_style(style, seg.style)
            } else {
                ansi16_style(style)
            };
            let remaining = usize::from(width.saturating_sub(col));
            let clipped = take_display_cols(&seg.text, remaining);
            let used = u16::try_from(display_cols(&clipped))
                .unwrap_or(0)
                .min(width.saturating_sub(col));
            buffer.set_stringn(x.saturating_add(col), y, &clipped, remaining, style);
            col = col.saturating_add(used);
        }
    }
}

fn ansi16_style(mut style: Style) -> Style {
    if let Some(fg) = style.fg {
        style.fg = Some(quantize_color(fg, ColorCapability::Ansi16));
    }
    if let Some(bg) = style.bg {
        style.bg = Some(quantize_color(bg, ColorCapability::Ansi16));
    }
    style
}

fn monochrome_style(mut style: Style, source: Style) -> Style {
    style.fg = None;
    style.bg = None;
    if source
        .bg
        .is_some_and(|background| background != Color::Reset)
    {
        style = style.add_modifier(Modifier::REVERSED);
    } else if source
        .fg
        .is_some_and(|foreground| foreground != Color::Reset)
        && style.add_modifier == Modifier::empty()
    {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}

impl Widget for &AnsiText<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = AnsiTextState::new();
        state.scroll_y = self.first;
        self.paint(area, buffer, &mut state);
    }
}

impl Widget for AnsiText<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Integration helpers ─────────────────────────────────────────────────────

/// Parse a chunk and return ratatui lines for LogPane / LogStream append.
#[must_use]
pub fn lines_for_log(input: &str, default_style: Style) -> Vec<Line<'static>> {
    let opts = AnsiParseOptions::default().with_default_style(default_style);
    parse_lines(input, &opts)
        .into_iter()
        .map(|l| l.to_line())
        .collect()
}

/// Assert content has no ESC (host-injection guard for tests / fuzz).
#[must_use]
pub fn is_paint_safe(s: &str) -> bool {
    !s.as_bytes().contains(&0x1b)
}

#[cfg(test)]
mod tests;
