// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/showcase/pages/editor.rs (MIT).

//! Blocks, tones, diagnostics and completion; the gutter says where you are.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use termrock::input::{KeyCode, KeyEventKind, KeyModifiers};
use termrock::style::{DesignSystem, SyntaxTone};
use termrock::widgets::{
    CodeBlock, CodeBlockState, CodeHighlight, CodeHighlightKind, CompletionCandidate,
    CompletionMenu, CompletionMenuOutcome, CompletionMenuState, EmptyKind, EmptyState,
    SyntaxHighlighter, TextAreaOutcome, TextAreaState, TextCursor, fuzzy_match_label,
};

use crate::ctx::RenderCtx;
use crate::id::WidgetId;
use crate::layout;
use crate::outcome::Route;
use crate::page::{Hint, Page, PageCtx, PageEvent};
use crate::text;

const ID: WidgetId = WidgetId::of("editor");

const SAMPLE: &str = "\
// Retry a request with exponential backoff.
pub async fn fetch(url: &str) -> Result<Body, Error> {
    let mut delay = 200;
    for attempt in 1..=5 {
        match client().get(url).await {
            Ok(body) => return Ok(body),
            Err(e) if e.is_transient() => {
                log::warn!(\"attempt {attempt} failed: {e}\");
                sleep(delay).await;
                delay *= 2;
            }
            Err(e) => return Err(e),
        }
    }
    Err(Error::Exhausted)
}

fn client() -> Client {
    Client::builder().timeout(10).build().unwrap()
}

#[test]
fn backoff_doubles() {
    assert_eq!(schedule(3), vec![200, 400, 800]);
}
";

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "else", "enum", "fn", "for", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
    "Self", "static", "struct", "trait", "true", "false", "type", "use", "where", "while",
];

const CANDIDATES: &[(&str, &str, &str)] = &[
    ("fetch(", "async fn (url: &str) -> Result<Body, Error>", "ƒ"),
    ("client(", "fn () -> Client", "ƒ"),
    ("sleep(", "async fn (ms: u64)", "ƒ"),
    ("schedule(", "fn (attempts: u32) -> Vec<u64>", "ƒ"),
    ("Client", "struct", "T"),
    ("Body", "struct", "T"),
    ("Error", "enum · Transient, Exhausted", "T"),
    ("Result<T, E>", "enum", "T"),
    ("Option<T>", "enum", "T"),
    ("String", "struct", "T"),
    ("Vec<T>", "struct", "T"),
    ("delay", "local · u64", "v"),
    ("attempt", "local · u32", "v"),
    ("url", "param · &str", "v"),
    ("assert_eq!(", "macro", "m"),
    ("format!(", "macro", "m"),
    ("println!(", "macro", "m"),
    ("log::warn!(", "macro", "m"),
    ("await", "keyword", "k"),
    ("async", "keyword", "k"),
    ("match", "keyword", "k"),
    ("return", "keyword", "k"),
];

/// Source showcase `highlight()` — attributes `#[]` are comments, calls and
/// `Upper` names are ident, keywords from [`KEYWORDS`].
fn highlight(src: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !src.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = src[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
            out.push((i..end, SyntaxTone::Comment));
            i = end;
            continue;
        }
        if c == b'#' && b.get(i + 1) == Some(&b'[') {
            let end = src[i..].find(']').map(|n| i + n + 1).unwrap_or(b.len());
            out.push((i..end, SyntaxTone::Comment));
            i = end;
            continue;
        }
        if c == b'"' {
            let end = src[i + 1..].find('"').map(|n| i + n + 2).unwrap_or(b.len());
            out.push((i..end, SyntaxTone::Str));
            i = end;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'_' || b[j] == b'.') {
                j += 1;
            }
            out.push((i..j, SyntaxTone::Number));
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = i;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            let word = &src[i..j];
            let next = b.get(j).copied();
            let tone = if KEYWORDS.contains(&word) {
                SyntaxTone::Keyword
            } else if next == Some(b'(')
                || next == Some(b'!')
                || word.starts_with(|ch: char| ch.is_ascii_uppercase())
            {
                SyntaxTone::Ident
            } else {
                SyntaxTone::Plain
            };
            out.push((i..j, tone));
            i = j;
            continue;
        }
        let tone = match c {
            b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' => SyntaxTone::Punct,
            b'=' | b'+' | b'-' | b'*' | b'/' | b'<' | b'>' | b'!' | b'&' | b'|' | b':' | b'?'
            | b'.' => SyntaxTone::Operator,
            _ => {
                i += 1;
                continue;
            }
        };
        out.push((i..i + 1, tone));
        i += 1;
    }
    out
}

struct SampleSyntax<'a> {
    system: &'a DesignSystem,
}

impl SyntaxHighlighter for SampleSyntax<'_> {
    fn highlight_line<'line>(
        &self,
        line: &'line str,
        _line_index: usize,
    ) -> Vec<(&'line str, Style)> {
        let theme = self.system.junie_theme();
        let spans = highlight(line);
        let mut out = Vec::new();
        let mut at = 0usize;
        for (range, tone) in spans {
            if range.start > at && range.start <= line.len() {
                out.push((&line[at..range.start], Style::default()));
            }
            let end = range.end.min(line.len());
            if range.start < end {
                out.push((&line[range.start..end], theme.syntax(tone)));
            }
            at = end;
        }
        if at < line.len() {
            out.push((&line[at..], Style::default()));
        }
        if out.is_empty() {
            out.push((line, Style::default()));
        }
        out
    }
}

fn blocks(src: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut end = 0;
    let mut off = 0;
    for line in src.split_inclusive('\n') {
        if line.trim().is_empty() {
            if let Some(s) = start.take() {
                out.push(s..end);
            }
        } else {
            if start.is_none() {
                start = Some(off);
            }
            end = off + line.trim_end_matches('\n').len();
        }
        off += line.len();
    }
    if let Some(s) = start {
        out.push(s..end);
    }
    out
}

fn line_of(src: &str, byte: usize) -> usize {
    src.get(..byte.min(src.len()))
        .unwrap_or("")
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
}

#[derive(Clone)]
struct Diag {
    range: Range<usize>,
    error: bool,
    message: String,
}

pub struct EditorPage {
    editor: TextAreaState,
    code: CodeBlockState,
    completion: CompletionMenuState<usize>,
    complete_open: bool,
    complete_items: Vec<(String, String, &'static str)>,
    run_ticks: u8,
    runs: u32,
    last_ms: Option<u32>,
    running: Option<Range<usize>>,
    diagnostics: Vec<Diag>,
    replace_len: usize,
}

impl EditorPage {
    #[must_use]
    pub fn new() -> Self {
        let mut editor = TextAreaState::new(SAMPLE);
        editor.set_accepts_input(true);
        // Source CodeEditor opens at the document start, not EOF.
        let _ = editor.set_cursor(TextCursor { line: 0, byte: 0 });
        Self {
            editor,
            code: CodeBlockState::new(),
            completion: CompletionMenuState::new(None),
            complete_open: false,
            complete_items: Vec::new(),
            run_ticks: 0,
            runs: 0,
            last_ms: None,
            running: None,
            diagnostics: Vec::new(),
            replace_len: 0,
        }
    }

    fn cursor_offset(&self) -> usize {
        self.editor.absolute_byte(self.editor.cursor()).unwrap_or(0)
    }

    fn word_before_cursor(&self) -> (usize, String) {
        let cur = self.cursor_offset();
        let src = self.editor.text();
        let head = &src[..cur.min(src.len())];
        let start = head
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
            .map(|p| p + head[p..].chars().next().map_or(1, char::len_utf8))
            .unwrap_or(0);
        (start, head[start..].to_owned())
    }

    fn trigger(&mut self, manual: bool) {
        let (_, word) = self.word_before_cursor();
        if !manual && word.len() < 2 {
            self.complete_open = false;
            self.completion.set_open(false);
            return;
        }
        let mut ranked: Vec<(u32, usize)> = CANDIDATES
            .iter()
            .enumerate()
            .filter_map(|(i, (label, _, _))| {
                let (penalty, _) = fuzzy_match_label(&word, label)?;
                Some((penalty, i))
            })
            .collect();
        ranked.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| CANDIDATES[a.1].0.cmp(CANDIDATES[b.1].0))
        });
        self.complete_items = ranked
            .into_iter()
            .map(|(_, i)| {
                let (l, d, g) = CANDIDATES[i];
                (l.to_owned(), d.to_owned(), g)
            })
            .collect();
        self.replace_len = word.len();
        if self.complete_items.is_empty() {
            self.complete_open = false;
            self.completion.set_open(false);
        } else {
            self.complete_open = true;
            self.completion.set_open(true);
            self.completion = CompletionMenuState::new(Some(0));
            self.completion.set_open(true);
        }
    }

    fn accept(&mut self, i: usize) {
        let Some(item) = self.complete_items.get(i).cloned() else {
            return;
        };
        let replace = self.replace_len;
        let cur = self.cursor_offset();
        let start = cur.saturating_sub(replace);
        let a = self.editor.cursor_at_byte(start);
        let b = self.editor.cursor();
        let mut insert = item.0;
        let paren = insert.ends_with('(');
        if paren {
            insert.push(')');
        }
        let _ = self.editor.replace_between(a, b, &insert);
        if paren {
            let _ = self.editor.handle_key(termrock::input::KeyEvent::new(
                KeyCode::Left,
                KeyModifiers::NONE,
            ));
        }
        self.complete_open = false;
        self.completion.set_open(false);
    }

    fn run(&mut self, cx: &mut PageCtx<'_>) {
        let src = self.editor.text();
        let cur = self.cursor_offset();
        let Some(block) = blocks(&src)
            .into_iter()
            .find(|b| b.start <= cur && cur <= b.end)
        else {
            cx.status("Nothing to run: the cursor is between blocks");
            return;
        };
        self.complete_open = false;
        self.completion.set_open(false);
        self.running = Some(block);
        self.run_ticks = 10;
    }

    fn finish_run(&mut self, cx: &mut PageCtx<'_>) {
        if let Some(block) = self.running.clone() {
            let src = self.editor.text();
            let text = src.get(block.clone()).unwrap_or("").to_owned();
            if let Some(p) = text.find(".unwrap()") {
                self.diagnostics.push(Diag {
                    range: block.start + p + 1..block.start + p + 9,
                    error: false,
                    message: "unwrap() panics on Err; propagate with ? instead".into(),
                });
            }
            if let Some(p) = text.find("todo!") {
                self.diagnostics.push(Diag {
                    range: block.start + p..block.start + p + 5,
                    error: true,
                    message: "not yet implemented".into(),
                });
            }
        }
        self.running = None;
        self.runs += 1;
        let ms = 40 + (self.runs * 37) % 90;
        self.last_ms = Some(ms);
        cx.status(format!("Block ran in {ms} ms"));
    }
}

impl Page for EditorPage {
    fn title(&self) -> &'static str {
        "Code editor"
    }
    fn blurb(&self) -> &'static str {
        "Blocks, tones, diagnostics and completion; the gutter says where you are"
    }
    fn editing(&self) -> bool {
        self.editor.is_editing()
    }
    fn animating(&self) -> bool {
        self.run_ticks > 0
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx<'_>) {
        let t = ctx.theme;
        let (l, r) = layout::columns(area, (area.width * 62 / 100).max(40), 2);
        let focused = ctx.interaction.focused(ID.sub("code"));
        self.editor
            .set_accepts_input(focused && self.editor.is_editing());
        self.code.focused = focused;
        self.code.set_editing(self.editor.is_editing());
        self.code.set_cursor_line(Some(self.editor.cursor().line));
        self.code.set_cursor_col(self.editor.cursor().byte);
        let src = self.editor.text();
        let all = blocks(&src);
        let meta = if self.run_ticks > 0 {
            "running".to_owned()
        } else {
            format!("{} blocks", all.len())
        };
        let (inner, _bg) = layout::card(
            Rect::new(l.x, l.y, l.width, l.height.min(26)),
            buf,
            t,
            Some("retry.rs"),
            Some(&meta),
            focused,
        );
        let lines: Vec<&str> = self.editor.lines().collect();
        let cur = self.cursor_offset();
        let current = all.iter().find(|b| b.start <= cur && cur <= b.end);
        let mut highlights: Vec<CodeHighlight> = Vec::new();
        if let Some(run) = &self.running {
            let a = line_of(&src, run.start);
            let b = line_of(&src, run.end);
            for ln in a..=b {
                highlights.push(CodeHighlight::line(ln, CodeHighlightKind::Emphasis));
            }
        }
        let hi = SampleSyntax { system: ctx.system };
        let mut block = CodeBlock::new(&lines, ctx.system)
            .highlighter(&hi)
            .line_numbers(true);
        if let Some(b) = current {
            block = block.current_block(line_of(&src, b.start), line_of(&src, b.end) + 1);
        }
        if !highlights.is_empty() {
            block = block.highlights(&highlights);
        }
        let parts = block.paint(inner, buf, &mut self.code);
        ctx.control(ID.sub("code"), inner, false);
        ctx.scrollable(ID.sub("code"), inner);
        if self.editor.is_editing() {
            let c = self.editor.cursor();
            let y = parts.body.y.saturating_add(
                u16::try_from(c.line.saturating_sub(self.code.scroll_y)).unwrap_or(0),
            );
            if y < parts.body.bottom() {
                ctx.set_cursor(Position::new(
                    parts
                        .body
                        .x
                        .saturating_add(u16::try_from(c.byte.min(255)).unwrap_or(0)),
                    y,
                ));
            }
        }

        let (inner, bg) = layout::card(
            Rect::new(r.x, r.y, r.width, r.height.min(11)),
            buf,
            t,
            Some("State"),
            None,
            false,
        );
        let pos = self.editor.cursor();
        let block_s = all
            .iter()
            .position(|b| b.start <= cur && cur <= b.end)
            .map(|i| format!("{} of {}", i + 1, all.len()))
            .unwrap_or_else(|| "between blocks".into());
        let diags = self.diagnostics.len();
        let mode = if self.editor.is_editing() {
            "editing"
        } else {
            "navigating"
        };
        let cursor = format!("ln {} · col {}", pos.line + 1, pos.byte + 1);
        let runs = self.runs.to_string();
        let last = self
            .last_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "—".into());
        let diag_s = diags.to_string();
        let comp = if self.complete_open {
            format!("{} items", self.complete_items.len())
        } else {
            "closed".into()
        };
        let props: [(&str, &str, Style); 7] = [
            (
                "Mode",
                mode,
                if self.editor.is_editing() {
                    Style::new().fg(t.success).bg(bg)
                } else {
                    t.primary().bg(bg)
                },
            ),
            ("Cursor", cursor.as_str(), t.primary().bg(bg)),
            ("Block", block_s.as_str(), t.primary().bg(bg)),
            ("Runs", runs.as_str(), t.primary().bg(bg)),
            ("Last run", last.as_str(), t.primary().bg(bg)),
            (
                "Diagnostics",
                diag_s.as_str(),
                if diags > 0 {
                    Style::new().fg(t.warning).bg(bg)
                } else {
                    t.primary().bg(bg)
                },
            ),
            ("Completion", comp.as_str(), t.primary().bg(bg)),
        ];
        let label_w = props
            .iter()
            .map(|(l, _, _)| text::width(l) as u16)
            .max()
            .unwrap_or(0)
            + 2;
        for (i, (label, value, style)) in props.iter().enumerate() {
            let y = inner.y.saturating_add(i as u16);
            if y >= inner.bottom() {
                break;
            }
            buf.set_string(inner.x, y, label, t.muted().bg(bg));
            buf.set_string(
                inner.x.saturating_add(label_w),
                y,
                text::truncate(value, inner.width.saturating_sub(label_w) as usize),
                *style,
            );
        }

        let y = r.y + 12;
        if y + 4 < r.bottom() {
            let (inner, bg) = layout::card(
                Rect::new(r.x, y, r.width, (r.bottom() - y).min(9)),
                buf,
                t,
                Some("Diagnostics"),
                None,
                false,
            );
            if self.diagnostics.is_empty() {
                EmptyState::new("Nothing flagged", ctx.system)
                    .kind(EmptyKind::NoData)
                    .explanation("Run the second block: its unwrap() gets a warning")
                    .paint(inner, buf);
            } else {
                for (i, d) in self.diagnostics.iter().enumerate() {
                    let yy = inner.y + i as u16;
                    if yy >= inner.bottom() {
                        break;
                    }
                    let line = line_of(&src, d.range.start) + 1;
                    let (glyph, st) = if d.error {
                        ("!", Style::new().fg(t.error))
                    } else {
                        ("!", Style::new().fg(t.warning))
                    };
                    buf.set_string(inner.x, yy, glyph, st.bg(bg));
                    buf.set_string(
                        inner.x + 2,
                        yy,
                        text::truncate(
                            &format!("ln {line} · {}", d.message),
                            inner.width.saturating_sub(2) as usize,
                        ),
                        t.secondary().bg(bg),
                    );
                }
            }
        }

        if self.complete_open {
            let cands: Vec<CompletionCandidate<'_, usize>> = self
                .complete_items
                .iter()
                .enumerate()
                .map(|(i, (l, d, g))| {
                    CompletionCandidate::new(i, l.as_str())
                        .detail(d.as_str())
                        .kind_glyph(g)
                })
                .collect();
            // Source completion anchors at `cursor_cell.x - replace_len`.
            let replace = u16::try_from(self.replace_len).unwrap_or(0);
            let col = u16::try_from(pos.byte.min(usize::from(u16::MAX))).unwrap_or(0);
            let anchor = Rect::new(
                parts.body.x.saturating_add(col.saturating_sub(replace)),
                parts.body.y.saturating_add(
                    u16::try_from(pos.line.saturating_sub(self.code.scroll_y)).unwrap_or(0),
                ),
                1,
                1,
            );
            CompletionMenu::new(&cands, ctx.system, *buf.area(), anchor).paint(
                *buf.area(),
                buf,
                &mut self.completion,
            );
        }
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut PageCtx<'_>) -> Route {
        match ev {
            PageEvent::Tick => {
                if self.run_ticks == 0 {
                    return Route::Ignored;
                }
                self.run_ticks -= 1;
                if self.run_ticks == 0 {
                    self.finish_run(cx);
                }
                Route::Changed
            }
            PageEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if *cx.focus != Some(ID.sub("code")) {
                    return Route::Ignored;
                }
                // `set_editing` is a no-op unless the host already granted
                // `accepts_input`. Render later keeps the gate as focused∧editing.
                self.editor.set_accepts_input(true);
                if self.complete_open {
                    let cands: Vec<CompletionCandidate<'_, usize>> = self
                        .complete_items
                        .iter()
                        .enumerate()
                        .map(|(i, (l, _, _))| CompletionCandidate::new(i, l.as_str()))
                        .collect();
                    match self.completion.handle_key(*key, &cands) {
                        CompletionMenuOutcome::Committed(i) => {
                            self.accept(i);
                            return Route::Changed;
                        }
                        CompletionMenuOutcome::Dismissed => {
                            self.complete_open = false;
                            self.completion.set_open(false);
                            return Route::Changed;
                        }
                        CompletionMenuOutcome::Ignored => {}
                        _ => return Route::Changed,
                    }
                }
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                if ctrl && matches!(key.code, KeyCode::Char(' ')) {
                    if !self.editor.is_editing() {
                        self.editor.set_editing(true);
                    }
                    self.trigger(true);
                    return Route::Changed;
                }
                if ctrl && matches!(key.code, KeyCode::Char('r' | 'R')) {
                    self.run(cx);
                    return Route::Changed;
                }
                if !self.editor.is_editing() && key.modifiers.is_empty() {
                    match key.code {
                        KeyCode::Enter | KeyCode::Char('i') => {
                            self.editor.set_editing(true);
                            return Route::Changed;
                        }
                        KeyCode::Char('a') => {
                            self.editor.set_editing(true);
                            let _ = self.editor.handle_key(termrock::input::KeyEvent::new(
                                KeyCode::Right,
                                KeyModifiers::NONE,
                            ));
                            self.editor.set_editing(true);
                            return Route::Changed;
                        }
                        KeyCode::Char('{') => {
                            let cur = self.cursor_offset();
                            if let Some(b) = blocks(&self.editor.text())
                                .into_iter()
                                .rev()
                                .find(|b| b.start < cur)
                            {
                                let _ = self.editor.set_cursor(self.editor.cursor_at_byte(b.start));
                            }
                            return Route::Changed;
                        }
                        KeyCode::Char('}') => {
                            let cur = self.cursor_offset();
                            if let Some(b) = blocks(&self.editor.text())
                                .into_iter()
                                .find(|b| b.start > cur)
                            {
                                let _ = self.editor.set_cursor(self.editor.cursor_at_byte(b.start));
                            }
                            return Route::Changed;
                        }
                        _ => {}
                    }
                }
                let was_editing = self.editor.is_editing();
                let nav = matches!(
                    key.code,
                    KeyCode::Up
                        | KeyCode::Down
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Home
                        | KeyCode::End
                        | KeyCode::Char('h' | 'j' | 'k' | 'l' | 'g' | 'G')
                );
                let out = if !was_editing && nav {
                    let mapped = match key.code {
                        KeyCode::Char('h') => KeyCode::Left,
                        KeyCode::Char('j') => KeyCode::Down,
                        KeyCode::Char('k') => KeyCode::Up,
                        KeyCode::Char('l') => KeyCode::Right,
                        KeyCode::Char('g') => KeyCode::Home,
                        KeyCode::Char('G') => KeyCode::End,
                        other => other,
                    };
                    self.editor.set_editing(true);
                    let o = self
                        .editor
                        .handle_key(termrock::input::KeyEvent::new(mapped, KeyModifiers::NONE));
                    self.editor.set_editing(false);
                    o
                } else {
                    self.editor.handle_key(*key)
                };
                match out {
                    TextAreaOutcome::Ignored => Route::Ignored,
                    TextAreaOutcome::Cancelled => Route::Ignored,
                    _ => {
                        if was_editing && self.editor.is_editing() {
                            self.diagnostics.clear();
                            self.trigger(false);
                        } else if self.complete_open {
                            self.trigger(false);
                        }
                        if was_editing && !self.editor.is_editing() {
                            self.complete_open = false;
                            self.completion.set_open(false);
                        }
                        Route::Changed
                    }
                }
            }
            PageEvent::Paste(text) => {
                let _ = self.editor.insert_text(text);
                Route::Changed
            }
            PageEvent::Click { id, pos } => {
                if self.complete_open {
                    let cands: Vec<CompletionCandidate<'_, usize>> = self
                        .complete_items
                        .iter()
                        .enumerate()
                        .map(|(i, (l, _, _))| CompletionCandidate::new(i, l.as_str()))
                        .collect();
                    match self.completion.handle_mouse(
                        termrock::input::MouseEvent {
                            kind: termrock::input::MouseEventKind::Down(
                                termrock::input::MouseButton::Left,
                            ),
                            position: *pos,
                            modifiers: KeyModifiers::NONE,
                        },
                        &cands,
                    ) {
                        CompletionMenuOutcome::Committed(i) => {
                            self.accept(i);
                            return Route::Changed;
                        }
                        CompletionMenuOutcome::Dismissed => {
                            self.complete_open = false;
                            self.completion.set_open(false);
                        }
                        _ => {}
                    }
                }
                if *id == ID.sub("code") {
                    let was = *cx.focus == Some(*id);
                    cx.set_focus(*id);
                    if was && !self.editor.is_editing() {
                        self.editor.set_editing(true);
                    }
                    return Route::Changed;
                }
                Route::Ignored
            }
            PageEvent::Wheel { id, delta } if *id == ID.sub("code") => {
                let _ = self.editor.scroll_by(0, *delta as isize);
                Route::Changed
            }
            _ => Route::Ignored,
        }
    }

    fn hints(&self, _focus: Option<WidgetId>) -> Vec<Hint> {
        if self.complete_open {
            vec![("↑ ↓", "Move"), ("Enter", "Accept"), ("Esc", "Close")]
        } else if self.editor.is_editing() {
            vec![
                ("Ctrl+Space", "Complete"),
                ("Ctrl+R", "Run block"),
                ("Esc", "Done"),
            ]
        } else {
            vec![
                ("i", "Edit"),
                ("Ctrl+R", "Run block"),
                ("{ }", "Blocks"),
                ("/", "Find"),
            ]
        }
    }
}
