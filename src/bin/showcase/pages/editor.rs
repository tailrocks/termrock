use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;

use crate::app::PageId;
use crate::data::GRAPHEME_SAMPLES;
use crate::options::{PageValues, page_options};
use crate::pages::inputs::{custom_text_keymap, field_keys};
use crate::pages::template::{Section, Template};
use crate::pages::{Cx, Hint, Page, PageEvent};
use junie_tui::core::edit::EditEntry;
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::core::intent::Phase;
use junie_tui::core::keymap::{Actions, KeyBinding, KeyMap, NavKeys};
use junie_tui::core::pointer::MouseTarget;
use junie_tui::core::scroll::{HScroll, TrackClick};
use junie_tui::theme::{CursorStyle, SyntaxTone, Tone};
use junie_tui::ui::ctx::RenderCtx;
use junie_tui::ui::measure::Measure;
use junie_tui::ui::popup::{Dismiss, Place, Placement};
use junie_tui::ui::specimen::{SpecimenState, StateGrid};
use junie_tui::ui::text::fuzzy;
use junie_tui::widgets::button::Button;
use junie_tui::widgets::code::{
    CodeCmd, CodeEditor, Diagnostic, EditorAction, Highlighter, Segmenter, Severity,
};
use junie_tui::widgets::completion::{Completion, CompletionAction, TEXT_INSET};
use junie_tui::widgets::empty::{self, EmptyState};
use junie_tui::widgets::option_list::Item;
use junie_tui::widgets::panel::Panel;
use junie_tui::widgets::props::{self, Prop};

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
    Client::builder().timeout(10).gzip(true).build().unwrap()
}

#[test]
fn backoff_doubles() {
    assert_eq!(schedule(3), vec![200, 400, 800]);
}
";

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "else", "enum", "fn", "for", "if",
    "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
    "Self", "static", "struct", "trait", "type", "use", "where", "while",
];

const LITERALS: &[&str] = &["true", "false"];

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

fn international_candidates() -> Vec<(String, String, &'static str)> {
    GRAPHEME_SAMPLES
        .iter()
        .map(|(kind, sample)| (format!("go_{sample}("), format!("fn () · {kind}"), "ƒ"))
        .collect()
}
const UNAVAILABLE: &[(&str, &str)] = &[("url", "Moved into the request above")];

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
            let tone = if LITERALS.contains(&word) {
                SyntaxTone::Literal
            } else if KEYWORDS.contains(&word) {
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

/// Find presets: the needle `set_find_query` installs and the match
/// `set_find_current` marks (0-based, clamped to the matches found).
fn find_preset(value: &str) -> Option<(&'static str, usize)> {
    match value {
        "delay" => Some(("delay", 0)),
        "delay-3rd" => Some(("delay", 2)),
        "missing" => Some(("missing", 0)),
        _ => None,
    }
}

/// How often the demo editor's providers have run. The providers are pure,
/// so these counters are the only place an invalidation shows: the cache
/// reuses the last result until the text changes or F5 invalidates it.
static TONE_RUNS: AtomicUsize = AtomicUsize::new(0);
static BLOCK_RUNS: AtomicUsize = AtomicUsize::new(0);

fn counted_highlight(src: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    TONE_RUNS.fetch_add(1, Ordering::Relaxed);
    highlight(src)
}

fn counted_blocks(src: &str) -> Vec<Range<usize>> {
    BLOCK_RUNS.fetch_add(1, Ordering::Relaxed);
    blocks(src)
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

const LARGE_REPEATS: usize = 800;

fn large_document() -> String {
    SAMPLE.repeat(LARGE_REPEATS)
}

fn seed_text(document: &str, previous: &str, current: &str) -> String {
    match document {
        "empty" => String::new(),
        "large" if previous != "large" => large_document(),
        "sample" if previous == "large" => SAMPLE.to_owned(),
        "large" if current.is_empty() => large_document(),
        _ if current.is_empty() => SAMPLE.to_owned(),
        _ => current.to_owned(),
    }
}

pub struct EditorPage {
    template: Template,
    opts: PageValues,
    document: &'static str,
    editor: CodeEditor,
    completion: Completion,
    find: Button,
    run_button: Button,
    run_ticks: u8,
    runs: u32,
    last_ms: Option<u32>,
}

fn custom_keymap() -> KeyMap<CodeCmd> {
    vec![
        (KeyBinding::key(KeyCode::Enter), CodeCmd::Edit),
        (KeyBinding::char('A'), CodeCmd::Append),
        (KeyBinding::char('['), CodeCmd::PrevBlock),
        (KeyBinding::char(']'), CodeCmd::NextBlock),
        (KeyBinding::ctrl('f'), CodeCmd::Find),
        (KeyBinding::key(KeyCode::F(3)), CodeCmd::NextMatch),
        (KeyBinding::char('N'), CodeCmd::PrevMatch),
    ]
}

impl EditorPage {
    pub fn new() -> Self {
        let mut page = Self {
            template: Template::new(ID, states_grid()),
            opts: PageValues::new(page_options(PageId::Editor)),
            document: "sample",
            editor: CodeEditor::new(ID.sub("code"), SAMPLE),
            completion: Completion::new(ID.sub("complete")),
            find: Button::secondary(ID.sub("find"), "Find").shortcut("/"),
            run_button: Button::secondary(ID.sub("run"), "Run block").shortcut("Ctrl+R"),
            run_ticks: 0,
            runs: 0,
            last_ms: None,
        };
        page.rebuild();
        page
    }

    fn rebuild(&mut self) {
        let o = &self.opts;
        let document = o.value("codeeditor.document");
        let text = seed_text(document, self.document, self.editor.text());
        let cursor = if document == self.document {
            self.editor.cursor_offset().min(text.len())
        } else {
            0
        };
        self.document = document;
        let keys = field_keys(o, "codeeditor.field_keys");
        let mut e = CodeEditor::new(ID.sub("code"), &text)
            .placeholder("Type code. i edits, Ctrl+R runs the block under the cursor.")
            .read_only(o.on("codeeditor.read_only"))
            .tab_leaves(o.on("codeeditor.tab_leaves"))
            .esc_reverts(o.on("codeeditor.esc_reverts"))
            .nav(match o.value("codeeditor.nav") {
                "vim" => NavKeys::VIM,
                "no-page" => NavKeys::default().page(false),
                _ => NavKeys::default(),
            })
            .keymap(match o.value("codeeditor.keymap") {
                "custom" => custom_keymap(),
                _ => CodeCmd::default_keymap(),
            })
            .scrollbar_gap(o.number("codeeditor.scrollbar_gap").unwrap_or(1))
            .track_click(match o.value("codeeditor.track_click") {
                "jump" => TrackClick::Jump,
                _ => TrackClick::Page,
            });
        if o.on("codeeditor.highlighter") {
            e = e.highlighter(counted_highlight);
        }
        if o.on("codeeditor.segmenter") {
            e = e.segmenter(counted_blocks);
        }
        match o.value("codeeditor.edit_entry") {
            "immediate" => e = e.edit_entry(EditEntry::Immediate),
            "type-to-edit" => e = e.edit_entry(EditEntry::TypeToEdit),
            "explicit" => e = e.edit_entry(EditEntry::Explicit),
            _ => {}
        }
        if let Some(k) = keys {
            e = e.field_keys(k);
        }
        if o.value("codeeditor.text_keymap") == "custom" {
            e = e.text_keymap(custom_text_keymap(keys.unwrap_or_default()));
        }
        match o.value("codeeditor.horizontal") {
            "on" => e = e.horizontal(HScroll::default()),
            "off" => e = e.horizontal(HScroll::off()),
            _ => {}
        }
        e.indent = o.number("codeeditor.indent").unwrap_or(4) as usize;
        e.jump_to(cursor);
        e.set_diagnostics(self.editor.diagnostics().to_vec());
        if let Some((query, current)) = find_preset(o.value("codeeditor.find_preset")) {
            e.set_find_query(query);
            e.set_find_current(current);
        }
        self.editor = e;

        let wide = o.value("codeeditor.completion_width") == "wide";
        self.completion = Completion::new(ID.sub("complete"))
            .max_rows(o.number("codeeditor.completion_rows").unwrap_or(8))
            .popup_width(if wide { 40 } else { 24 }, if wide { 64 } else { 48 })
            .placement(match o.value("codeeditor.completion_placement") {
                "above" => Placement::Above,
                _ => Placement::Below,
            })
            .fit(match o.value("codeeditor.completion_fit") {
                "flush" => Place::FLUSH.text_inset(TEXT_INSET),
                _ => Place::DEFAULT.text_inset(TEXT_INSET),
            })
            .cursor_style(match o.value("codeeditor.completion_cursor") {
                "bar" => CursorStyle::Bar,
                _ => CursorStyle::Fill,
            })
            .dismiss(match o.value("codeeditor.completion_dismiss") {
                "escape" => Dismiss::ESCAPE,
                _ => Dismiss::LIGHT.pass_through(true),
            })
            .nav(match o.value("codeeditor.completion_nav") {
                "vim" => NavKeys::VIM,
                _ => Completion::NAV,
            });
    }

    fn word_before_cursor(&self) -> (usize, String) {
        let cur = self.editor.cursor_offset();
        let head = &self.editor.text()[..cur];
        let start = head
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':'))
            .map(|p| p + head[p..].chars().next().map_or(1, char::len_utf8))
            .unwrap_or(0);
        (start, head[start..].to_owned())
    }

    fn trigger(&mut self, manual: bool) {
        let (_, word) = self.word_before_cursor();
        if !manual && word.len() < 2 {
            self.completion.close();
            return;
        }
        let international = if self.opts.on("codeeditor.completion_international") {
            international_candidates()
        } else {
            Vec::new()
        };
        let mut ranked: Vec<(u32, Item)> = CANDIDATES
            .iter()
            .map(|(label, detail, glyph)| (label.to_string(), detail.to_string(), *glyph))
            .chain(international)
            .filter_map(|(label, detail, glyph)| {
                let (penalty, matched) = fuzzy(&label, &word)?;
                let why = UNAVAILABLE.iter().find(|(name, _)| *name == label.as_str());
                let item = Item::new(label)
                    .glyph(glyph)
                    .detail(detail)
                    .matched(matched);
                let item = match why {
                    Some((_, why)) => item.reason(*why),
                    None => item,
                };
                Some((penalty, item))
            })
            .collect();
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.label.cmp(&b.1.label)));
        let items: Vec<Item> = ranked.into_iter().map(|r| r.1).collect();
        let replace = word.len();
        let anchor = self
            .editor
            .cursor_cell()
            .map(|c| Rect::new(c.x.saturating_sub(replace as u16), c.y, 1, 1))
            .unwrap_or(Rect::ZERO);
        if items.is_empty() {
            self.completion.close();
        } else {
            self.completion.open(items, anchor, replace);
        }
    }

    fn accept(&mut self, i: usize) {
        if self.completion.apply(i, &mut self.editor.field.buffer) {
            self.completion.close();
        }
    }

    fn run(&mut self, cx: &mut Cx) {
        let Some(block) = self.editor.current_block() else {
            cx.status("Nothing to run: the cursor is between blocks");
            return;
        };
        self.completion.close();
        self.editor.set_running(Some(block));
        self.run_ticks = 10;
    }

    fn open_find(&mut self, cx: &mut Cx) {
        self.completion.close();
        cx.focus.focus(self.editor.id);
        self.editor.open_find();
    }

    fn rescan(&mut self, cx: &mut Cx) {
        let mut providers = Vec::new();
        if self.editor.highlighter_fn().is_some() {
            self.editor.invalidate_highlighter();
            providers.push("tones");
        }
        if self.editor.segmenter_fn().is_some() {
            self.editor.invalidate_segmenter();
            providers.push("blocks");
        }
        if providers.is_empty() {
            cx.status("Nothing to re-scan: syntax tones and blocks are both off");
        } else {
            cx.status(format!("Re-scanned {}", providers.join(" and ")));
        }
    }

    fn finish_run(&mut self, cx: &mut Cx) {
        if let Some(block) = self.editor.running.clone() {
            let text = self.editor.text()[block.clone()].to_owned();
            if let Some(p) = text.find(".unwrap()") {
                self.editor.push_diagnostic(Diagnostic {
                    range: block.start + p + 1..block.start + p + 9,
                    severity: Severity::Warning,
                    message: "unwrap() panics on Err; propagate with ? instead".into(),
                });
            }
            if let Some(p) = text.find("todo!") {
                self.editor.push_diagnostic(Diagnostic {
                    range: block.start + p..block.start + p + 5,
                    severity: Severity::Error,
                    message: "not yet implemented".into(),
                });
            }
        }
        self.editor.set_running(None);
        self.runs += 1;
        let ms = 40 + (self.runs * 37) % 90;
        self.last_ms = Some(ms);
        cx.status(format!("Block ran in {ms} ms"));
    }

    fn state_props(&self) -> Vec<Prop> {
        let pos = self.editor.field.buffer.cursor_pos();
        let all = self.editor.blocks();
        let cur = self.editor.cursor_offset();
        let block = all
            .iter()
            .position(|b| b.start <= cur && cur <= b.end)
            .map(|i| format!("{} of {}", i + 1, all.len()))
            .unwrap_or_else(|| "between blocks".into());
        let width = self
            .editor
            .field
            .buffer
            .line_width(pos.line)
            .map_or_else(|| "—".to_owned(), |w| format!("{w} cells"));
        let providers = match (
            self.editor.highlighter_fn().is_some(),
            self.editor.segmenter_fn().is_some(),
        ) {
            (true, true) => "tones · blocks",
            (true, false) => "tones",
            (false, true) => "blocks",
            (false, false) => "none",
        };
        let diags = self.editor.diagnostics().len();
        let find = match self.editor.find_state() {
            None => "closed".to_owned(),
            Some(f) if f.needle.text().is_empty() => "type to find".to_owned(),
            Some(f) if f.matches.is_empty() => format!("“{}” · no matches", f.needle.text()),
            Some(f) => format!(
                "“{}” · {} of {}",
                f.needle.text(),
                f.current + 1,
                f.matches.len()
            ),
        };
        let props = vec![
            Prop::new(
                "Mode",
                if self.editor.editing {
                    "editing"
                } else {
                    "navigating"
                },
            )
            .tone(if self.editor.editing {
                Tone::Success
            } else {
                Tone::Normal
            }),
            Prop::new(
                "Cursor",
                format!("ln {} · col {}", pos.line + 1, pos.col + 1),
            ),
            Prop::new("Line width", width),
            Prop::new("Block", block),
            Prop::new("Runs", self.runs.to_string()),
            Prop::new(
                "Last run",
                self.last_ms
                    .map(|ms| format!("{ms} ms"))
                    .unwrap_or_else(|| "—".into()),
            ),
            Prop::new("Providers", providers),
            // Same order as the Providers row above: tone runs, then block runs.
            Prop::new(
                "Scans",
                format!(
                    "{} · {}",
                    TONE_RUNS.load(Ordering::Relaxed),
                    BLOCK_RUNS.load(Ordering::Relaxed)
                ),
            ),
            Prop::new("Diagnostics", diags.to_string()).tone(if diags > 0 {
                Tone::Warning
            } else {
                Tone::Normal
            }),
            Prop::new(
                "Completion",
                if self.completion.is_open() {
                    format!("{} items", self.completion.items().len())
                } else {
                    "closed".into()
                },
            ),
            Prop::new("Find", find),
        ];
        props
    }

    fn no_diagnostics() -> EmptyState {
        EmptyState::new("Nothing flagged").hint("Run the second block: its unwrap() gets a warning")
    }

    fn right_need(&self, ctx: &RenderCtx) -> u16 {
        Panel::card(Some("State"))
            .outer(Size::new(props::width(&self.state_props()), 0))
            .width
            .max(
                Panel::card(Some("Diagnostics"))
                    .outer(Size::new(
                        Self::no_diagnostics().min_width(Size::ZERO, ctx),
                        0,
                    ))
                    .width,
            )
            .max(self.find.width() + 2 + self.run_button.width())
    }

    fn render_playground(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;

        let right = self.right_need(ctx);
        let (l, r) = if area.width >= EDITOR_MIN + 2 + right {
            let left = (area.width * 62 / 100)
                .max(EDITOR_MIN)
                .min(area.width - 2 - right);
            crate::pages::layout::columns(area, left, 2)
        } else {
            let below = (EDITOR_H + 1).min(area.height);
            (
                Rect::new(area.x, area.y, area.width, EDITOR_H.min(area.height)),
                Rect::new(area.x, area.y + below, area.width, area.height - below),
            )
        };
        let focused = ctx.interaction.focused(self.editor.id);
        let meta = if self.run_ticks > 0 {
            "running".to_owned()
        } else if self.editor.read_only {
            "read-only".to_owned()
        } else {
            format!("{} blocks", self.editor.blocks().len())
        };
        let panel = Panel::card(Some("retry.rs")).focused(focused).meta(&meta);
        let bg = panel.bg(t);
        let inner = panel.render(Rect::new(l.x, l.y, l.width, l.height.min(EDITOR_H)), buf, t);
        self.editor.render(inner, buf, ctx, bg);

        let widths = [self.find.width(), self.run_button.width()];
        let rects =
            junie_tui::widgets::button::row_layout(Rect::new(r.x, r.y, r.width, 1), &widths, 2);
        self.find.render(rects[0], buf, ctx, t.canvas);
        self.run_button.render(rects[1], buf, ctx, t.canvas);
        let r = Rect::new(r.x, r.y + 2, r.width, r.height.saturating_sub(2));
        let panel = Panel::card(Some("State"));
        let bg = panel.bg(t);
        let inner = panel.render(Rect::new(r.x, r.y, r.width, r.height.min(14)), buf, t);
        let props = self.state_props();
        props::render(inner, buf, t, &props, bg);

        let y = r.y + 15;
        if y + 4 < r.bottom() {
            let panel = Panel::card(Some("Diagnostics"));
            let bg = panel.bg(t);
            let inner = panel.render(Rect::new(r.x, y, r.width, (r.bottom() - y).min(9)), buf, t);
            if self.editor.diagnostics().is_empty() {
                empty::render(inner, buf, t, &Self::no_diagnostics(), bg);
            } else {
                for (i, d) in self.editor.diagnostics().iter().enumerate() {
                    let yy = inner.y + i as u16;
                    if yy >= inner.bottom() {
                        break;
                    }
                    let line = junie_tui::core::text::TextBuffer::pos_of(
                        self.editor.text(),
                        d.range.start,
                    )
                    .line
                        + 1;
                    let (glyph, st) = match d.severity {
                        Severity::Error => ("!", Style::new().fg(t.error)),
                        Severity::Warning => ("!", Style::new().fg(t.warning)),
                    };
                    buf.set_string(inner.x, yy, glyph, st.bg(bg));
                    buf.set_string(
                        inner.x + 2,
                        yy,
                        junie_tui::ui::text::truncate(
                            &format!("ln {line} · {}", d.message),
                            inner.width.saturating_sub(2) as usize,
                        ),
                        t.secondary().bg(bg),
                    );
                }
            }
        }

        if self.completion.is_open() {
            let screen = *buf.area();
            self.completion.render(screen, buf, ctx);
        }
    }
}

fn states_grid() -> StateGrid {
    use SpecimenState::*;
    let id = ID.sub("states");
    let src = "let delay = 200;\nretry(delay);";
    let mut warned = CodeEditor::new(id.child(2), src).highlighter(highlight);
    warned.push_diagnostic(Diagnostic {
        range: 4..9,
        severity: Severity::Warning,
        message: "unused variable".into(),
    });
    StateGrid::new(id)
        .label_width(11)
        .column_width(28)
        .rows(&[
            Rest, Focused, Editing, Disabled, Busy, Error, Empty, Overflow,
        ])
        .column(
            "Tones",
            CodeEditor::new(id.child(0), src)
                .highlighter(highlight)
                .placeholder("Type code"),
        )
        .column(
            "Plain",
            CodeEditor::new(id.child(1), src).placeholder("Type code"),
        )
        .column("Warning", warned.placeholder("Type code"))
}

const EDITOR_MIN: u16 = 40;

const EDITOR_H: u16 = 26;

const RIGHT_H: u16 = 2 + 14 + 1 + 9;

impl Page for EditorPage {
    fn title(&self) -> &'static str {
        "Code editor"
    }
    fn blurb(&self) -> &'static str {
        "Blocks, tones, diagnostics and completion; the gutter says where you are"
    }
    fn editing(&self) -> bool {
        self.editor.editing
    }
    fn animating(&self) -> bool {
        self.run_ticks > 0
    }

    fn measure(&self, avail: Size, ctx: &RenderCtx) -> Size {
        let right = self.right_need(ctx);
        if avail.width >= EDITOR_MIN + 2 + right {
            Size::new(EDITOR_MIN + 2 + right, EDITOR_H)
        } else {
            Size::new(
                avail.width.max(EDITOR_MIN).max(right),
                EDITOR_H + 1 + RIGHT_H,
            )
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let body = self.template.render(area, buf, ctx);
        match self.template.section() {
            Section::Playground => self.render_playground(body, buf, ctx),
            Section::Options => self.template.render_options(body, buf, ctx),
            Section::States => self.template.render_states(body, buf, ctx),
        }
    }

    fn layer_open(&self) -> bool {
        self.completion.is_open()
    }

    fn handle(&mut self, ev: &PageEvent, cx: &mut Cx) -> Outcome {
        if let PageEvent::Tick = ev {
            if self.run_ticks == 0 {
                return Outcome::Ignored;
            }
            self.run_ticks -= 1;
            if self.run_ticks == 0 {
                self.finish_run(cx);
            }
            return Outcome::Changed;
        }
        if let Some(o) = self.template.handle(ev, cx) {
            return o;
        }
        if self.template.section() != Section::Playground {
            return Outcome::Ignored;
        }
        match ev {
            PageEvent::Key(key) => {
                if cx.focus.is(self.find.id) {
                    let (o, act) = self.find.on_key(key);
                    if act {
                        self.open_find(cx);
                    }
                    return o;
                }
                if cx.focus.is(self.run_button.id) {
                    let (o, act) = self.run_button.on_key(key);
                    if act {
                        self.run(cx);
                    }
                    return o;
                }
                if !cx.focus.is(self.editor.id) {
                    return Outcome::Ignored;
                }
                if self.completion.is_open() {
                    let (o, ev) = self.completion.on_key(key);
                    match ev {
                        Some(CompletionAction::Accept(i)) => {
                            self.accept(i);
                            return Outcome::Changed;
                        }
                        Some(CompletionAction::Dismiss) => return Outcome::Changed,
                        None => {
                            if o.consumed() {
                                return o;
                            }
                        }
                    }
                }
                if !self.editor.read_only && (key.ctrl_char(' ') || key.code == KeyCode::Null) {
                    if !self.editor.editing {
                        self.editor.begin_edit();
                    }
                    self.trigger(true);
                    return Outcome::Changed;
                }
                if key.ctrl_char('r') {
                    self.run(cx);
                    return Outcome::Changed;
                }
                if key.code == KeyCode::F(5) {
                    self.rescan(cx);
                    return Outcome::Changed;
                }
                let (o, ev) = self.editor.on_key(key);
                match ev {
                    Some(EditorAction::Changed) => {
                        self.editor.set_diagnostics(Vec::new());
                        self.trigger(false);
                    }
                    Some(EditorAction::CursorMoved) => {
                        if self.completion.is_open() {
                            self.trigger(false);
                        }
                    }
                    Some(EditorAction::Committed) => self.completion.close(),
                    Some(EditorAction::Leave { backward }) => {
                        if backward {
                            cx.focus_prev();
                        } else {
                            cx.focus_next();
                        }
                    }
                    None => {}
                }
                o
            }
            PageEvent::Paste(text) => {
                let o = self.editor.on_paste(text);
                if self.completion.is_open() {
                    // The paste moved the cursor past the word the popup was
                    // opened for, so its replace span and rows are stale.
                    self.trigger(false);
                }
                o
            }
            PageEvent::Pointer(ev) => {
                if ev.wheel().is_some()
                    && let Some(r) = self.completion.route(ev)
                {
                    return r.outcome();
                }
                if ev.is(Phase::Click) && self.completion.is_open() {
                    if ev.on(self.completion.id) {
                        let actions = self
                            .completion
                            .route(ev)
                            .map(|r| r.into_actions())
                            .unwrap_or_default();
                        for action in actions {
                            if let CompletionAction::Accept(i) = action {
                                self.accept(i);
                                return Outcome::Changed;
                            }
                        }
                    } else {
                        self.completion.close();
                    }
                }
                if ev.is(Phase::Click) {
                    if let Some(r) = self.find.route(ev) {
                        if r.has_actions() {
                            self.open_find(cx);
                        }
                        return Outcome::Changed;
                    }
                    if let Some(r) = self.run_button.route(ev) {
                        cx.focus.focus(self.run_button.id);
                        if r.has_actions() {
                            self.run(cx);
                        }
                        return Outcome::Changed;
                    }
                    if ev.on(self.editor.id) && ev.bar.is_none() {
                        cx.focus.focus(self.editor.id);
                    }
                }
                self.editor
                    .route(ev)
                    .map_or(Outcome::Ignored, |r| r.outcome())
            }
            _ => Outcome::Ignored,
        }
    }

    fn hints(&self, focus: Option<WidgetId>) -> Vec<Hint> {
        if let Some(h) = self.template.hints(focus) {
            return h;
        }
        if focus == Some(self.find.id) || focus == Some(self.run_button.id) {
            return vec![("Enter", "Activate"), ("Tab", "Next")];
        }
        if self.completion.is_open() {
            let mut v = vec![("↑↓", "Move")];
            if self.completion.has_eligible() {
                v.push(("Enter", "Accept"));
            }
            v.push(("Esc", "Close"));
            v
        } else if self.editor.editing {
            vec![
                ("Ctrl+Space", "Complete"),
                ("Ctrl+R", "Run block"),
                ("F5", "Re-scan"),
                if self.editor.tab_leaves {
                    ("Tab", "Next field")
                } else {
                    ("Tab", "Indent")
                },
                (
                    "Esc",
                    if self.editor.esc_reverts {
                        "Cancel"
                    } else {
                        "Done"
                    },
                ),
            ]
        } else {
            let find = if self.opts.value("codeeditor.keymap") == "custom" {
                "Ctrl+F"
            } else {
                "/"
            };
            let mut h = vec![("↑↓", "Move"), (find, "Find")];
            if !self.editor.read_only {
                h.insert(0, ("i", "Edit"));
                h.push(("Ctrl+R", "Run block"));
            }
            h.push(("F5", "Re-scan"));
            h
        }
    }

    fn option_value(&self, key: &str) -> Option<usize> {
        self.opts.get(key)
    }

    fn set_option(&mut self, key: &str, value: usize) {
        if !self.opts.set(key, value) {
            return;
        }
        match key {
            "codeeditor.highlighter" => {
                let provider = self.opts.on(key).then_some(highlight as Highlighter);
                self.editor.set_highlighter(provider);
            }
            "codeeditor.segmenter" => {
                let provider = self.opts.on(key).then_some(blocks as Segmenter);
                self.editor.set_segmenter(provider);
            }
            _ => self.rebuild(),
        }
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn template(&self) -> Option<&Template> {
        Some(&self.template)
    }

    fn template_mut(&mut self) -> Option<&mut Template> {
        Some(&mut self.template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hints_while_completing(word: &str) -> Vec<Hint> {
        let mut page = EditorPage::new();
        page.editor.field.buffer.insert_str(word);
        page.trigger(true);
        assert!(page.completion.is_open(), "{word} opens the popup");
        page.hints(None)
    }

    fn drawn(page: &mut EditorPage) -> String {
        use junie_tui::core::focus::FocusRing;
        use junie_tui::core::hit::HitRegistry;
        use junie_tui::theme::{ColorLevel, Theme};
        use junie_tui::ui::ctx::Interaction;

        let theme = Theme::for_level(ColorLevel::TrueColor);
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        page.render(area, &mut buf, &mut ctx);
        ctx.flush_overlays(&mut buf);
        buf.content().iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn completion_draws_why_the_highlighted_entry_is_unavailable() {
        let (_, why) = UNAVAILABLE[0];
        let mut page = EditorPage::new();
        page.editor.field.buffer.insert_str("url");
        drawn(&mut page);
        page.trigger(true);
        assert!(drawn(&mut page).contains(why));

        // "url" is listed but disabled, and "return" is enabled. Keyboard
        // navigation skips disabled rows, so the highlight starts on "return"
        // and the reason of the unhighlighted "url" must stay undrawn.
        let mut page = EditorPage::new();
        page.editor.field.buffer.insert_str("ur");
        drawn(&mut page);
        page.trigger(true);
        assert!(page.completion.items().iter().any(|i| i.label == "url"));
        assert_eq!(
            page.completion.current().map(|i| i.label.as_str()),
            Some("return")
        );
        assert!(!drawn(&mut page).contains(why));
    }

    #[test]
    fn a_paste_with_the_popup_open_refreshes_what_accept_replaces() {
        use junie_tui::core::focus::{Focus, FocusRing};
        let mut page = EditorPage::new();
        page.editor.field.buffer.insert_str("fet");
        page.editor.begin_edit();
        page.trigger(true);
        assert!(page.completion.is_open());
        let mut focus = Focus::default();
        focus.focus(page.editor.id);
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.handle(&PageEvent::Paste("ch".into()), &mut cx);
        let (_, word) = page.word_before_cursor();
        assert_eq!(word, "fetch");
        // Accepting must replace the whole word the user now sees, not the
        // three characters the popup was opened with.
        let enter = junie_tui::core::event::Key {
            code: KeyCode::Enter,
            mods: ratatui::crossterm::event::KeyModifiers::NONE,
        };
        page.handle(&PageEvent::Key(enter), &mut cx);
        assert!(
            !page.editor.text().contains("fefetch"),
            "stale replace span corrupted the word: {:?}",
            &page.editor.text()[..16.min(page.editor.text().len())]
        );
    }

    #[test]
    fn accept_hint_follows_whether_a_candidate_is_eligible() {
        let only_disabled = hints_while_completing("url");
        assert!(
            !only_disabled.contains(&("Enter", "Accept")),
            "{only_disabled:?}"
        );
        assert!(only_disabled.contains(&("Esc", "Close")));

        let eligible = hints_while_completing("fet");
        assert!(eligible.contains(&("Enter", "Accept")), "{eligible:?}");
    }

    #[test]
    fn a_cleared_large_document_reloads_as_large_not_as_the_sample() {
        assert_eq!(seed_text("large", "large", ""), large_document());
        assert_eq!(seed_text("sample", "sample", ""), SAMPLE);
    }

    #[test]
    fn switching_documents_loads_the_chosen_one_and_other_options_keep_edits() {
        assert_eq!(seed_text("large", "sample", "x"), large_document());
        assert_eq!(seed_text("sample", "large", "x"), SAMPLE);
        assert_eq!(seed_text("empty", "sample", "x"), "");
        assert_eq!(seed_text("large", "large", "edited"), "edited");
        assert_eq!(seed_text("sample", "sample", "edited"), "edited");
    }
}
