use std::cell::Cell;
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::index::{BracketMap, DiagnosticIndex, ProviderKey, RunIndex, SegmentSnapshot, work};
use super::{CodeEditor, Diagnostic, Highlighter, Segmenter, Severity};
use crate::core::edit::EditEntry;
use crate::core::event::Key;
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::WidgetId;
use crate::core::text::TextBuffer;
use crate::test_alloc;
use crate::theme::{ColorLevel, SyntaxTone, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};
use termrock_text::text_index::work as text_work;

thread_local! {
    static HIGHLIGHTS: Cell<usize> = const { Cell::new(0) };
    static SEGMENTS: Cell<usize> = const { Cell::new(0) };
}

fn bump(counter: &'static std::thread::LocalKey<Cell<usize>>) {
    counter.with(|c| c.set(c.get() + 1));
}

fn count(counter: &'static std::thread::LocalKey<Cell<usize>>) -> usize {
    counter.with(Cell::get)
}

fn reset() {
    HIGHLIGHTS.with(|c| c.set(0));
    SEGMENTS.with(|c| c.set(0));
    work::tally(|c| *c = work::Counts::default());
}

fn keywords(text: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    bump(&HIGHLIGHTS);
    text.match_indices("SELECT")
        .map(|(i, m)| (i..i + m.len(), SyntaxTone::Keyword))
        .collect()
}

fn strings(text: &str) -> Vec<(Range<usize>, SyntaxTone)> {
    bump(&HIGHLIGHTS);
    text.match_indices("SELECT")
        .map(|(i, m)| (i..i + m.len(), SyntaxTone::Str))
        .collect()
}

fn statements(text: &str) -> Vec<Range<usize>> {
    bump(&SEGMENTS);
    let mut blocks = Vec::new();
    let mut start = 0;
    for part in text.split("\n\n") {
        blocks.push(start..start + part.len());
        start += part.len() + 2;
    }
    blocks
}

fn set_highlighter(editor: &mut CodeEditor, f: Highlighter) {
    editor.set_highlighter(Some(f));
}

fn document(lines: usize) -> String {
    let mut out = String::new();
    for i in 0..lines {
        out.push_str(&format!(
            "SELECT col_{i} FROM t{i} WHERE x = ({i} + [1]);\n"
        ));
        if i % 8 == 7 {
            out.push('\n');
        }
    }
    out
}

struct Frame {
    theme: Theme,
    hits: HitRegistry,
    ring: FocusRing,
    buf: Buffer,
}

impl Frame {
    fn new(width: u16, height: u16) -> Self {
        Self {
            theme: Theme::for_level(ColorLevel::TrueColor),
            hits: HitRegistry::default(),
            ring: FocusRing::default(),
            buf: Buffer::empty(Rect::new(0, 0, width, height)),
        }
    }

    fn paint(&mut self, editor: &mut CodeEditor, interaction: Interaction) {
        let area = self.buf.area;
        let mut ctx = RenderCtx::new(&self.theme, interaction, &mut self.hits, &mut self.ring);
        editor.render(area, &mut self.buf, &mut ctx, Color::Reset);
    }

    fn fg(&self, x: u16, y: u16) -> Option<Color> {
        self.buf.cell((x, y)).and_then(|c| c.style().fg)
    }
}

fn editor(lines: usize) -> CodeEditor {
    CodeEditor::new(WidgetId::of("code-test"), &document(lines))
        .highlighter(keywords)
        .segmenter(statements)
}

#[test]
fn editor_prepared_frames_do_no_full_document_work() {
    let id = WidgetId::of("code-test");
    let focused = Interaction {
        focus: Some(id),
        ..Default::default()
    };
    let hovered = Interaction {
        hover: Some(id),
        ..Default::default()
    };
    let mut per_size = Vec::new();
    for lines in [1_000usize, 10_000, 100_000] {
        reset();
        let mut frame = Frame::new(100, 24);
        let mut e = editor(lines);
        frame.paint(&mut e, Interaction::default());
        let warm_highlights = count(&HIGHLIGHTS);
        let warm_segments = count(&SEGMENTS);
        assert_eq!(warm_highlights, 1, "{lines} lines: one highlight to warm");
        assert_eq!(warm_segments, 1, "{lines} lines: one segmentation to warm");

        let mut worst = 0;
        for interaction in [
            Interaction::default(),
            focused,
            hovered,
            Interaction::default(),
        ] {
            let ((), bytes) = test_alloc::measure(|| frame.paint(&mut e, interaction));
            worst = worst.max(bytes);
        }
        assert_eq!(
            count(&HIGHLIGHTS),
            warm_highlights,
            "{lines} lines: unchanged frames must not call the highlighter"
        );
        assert_eq!(
            count(&SEGMENTS),
            warm_segments,
            "{lines} lines: unchanged frames must not call the segmenter"
        );
        per_size.push((lines, worst));
    }
    let (_, small) = per_size[0];
    for (lines, bytes) in &per_size {
        assert!(
            *bytes <= small + 1024,
            "{lines} lines allocated {bytes} bytes per unchanged frame against {small} at 1000 lines: {per_size:?}"
        );
    }
}

#[test]
fn editor_provider_change_invalidates_unchanged_text() {
    reset();
    let id = WidgetId::of("code-test");
    let mut frame = Frame::new(100, 12);
    let mut e = CodeEditor::new(id, "SELECT 1;\n").highlighter(keywords);
    frame.paint(&mut e, Interaction::default());
    let keyword = frame.fg(7, 0);
    assert_eq!(keyword, Some(frame.theme.syntax_keyword));

    set_highlighter(&mut e, strings);
    frame.paint(&mut e, Interaction::default());
    assert_eq!(
        frame.fg(7, 0),
        Some(frame.theme.syntax_string),
        "a new provider must repaint unchanged text"
    );
    let _ = (keyword, count(&HIGHLIGHTS));
}

#[test]
fn editor_block_apis_do_not_rerun_the_segmenter_per_call() {
    reset();
    let seg: Segmenter = statements;
    let mut e = CodeEditor::new(WidgetId::of("code-test"), &document(64)).segmenter(seg);
    e.jump_to(0);
    let first = e.current_block();
    let blocks = e.blocks();
    let again = e.current_block();
    assert_eq!(first, again);
    assert!(!blocks.is_empty());
    assert_eq!(
        count(&SEGMENTS),
        1,
        "current_block, blocks and current_block share one segmentation"
    );
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as usize) % bound
    }

    fn range(&mut self, limit: usize) -> Range<usize> {
        let a = self.next(limit);
        let b = match self.next(5) {
            0 => a,
            1 => self.next(limit),
            _ => a + self.next(limit / 3 + 1),
        };
        a..b
    }
}

fn old_block_at(blocks: &[Range<usize>], cur: usize) -> Option<Range<usize>> {
    blocks
        .iter()
        .find(|b| cur >= b.start && cur <= b.end)
        .or_else(|| blocks.iter().rev().find(|b| b.end <= cur))
        .cloned()
}

fn old_bracket_pair(text: &[u8], cur: usize) -> Option<(usize, usize)> {
    let at = |i: usize| text.get(i).copied();
    for p in [cur, cur.wrapping_sub(1)] {
        let Some(c) = at(p) else { continue };
        let (open, close, forward) = match c {
            b'(' => (b'(', b')', true),
            b')' => (b'(', b')', false),
            b'[' => (b'[', b']', true),
            b']' => (b'[', b']', false),
            _ => continue,
        };
        let mut depth = 0i32;
        if forward {
            for (i, &b) in text.iter().enumerate().skip(p) {
                if b == open {
                    depth += 1;
                } else if b == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some((p, i));
                    }
                }
            }
        } else {
            for i in (0..=p).rev() {
                let b = text[i];
                if b == close {
                    depth += 1;
                } else if b == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some((i, p));
                    }
                }
            }
        }
    }
    None
}

fn diagnostic(range: Range<usize>, severity: Severity) -> Diagnostic {
    Diagnostic {
        range,
        severity,
        message: String::new(),
    }
}

fn key(code: KeyCode) -> Key {
    Key {
        code,
        mods: KeyModifiers::NONE,
    }
}

fn focused(id: WidgetId) -> Interaction {
    Interaction {
        focus: Some(id),
        ..Default::default()
    }
}

fn builds() -> work::Counts {
    work::counts()
}

#[test]
#[allow(clippy::reversed_empty_ranges)]
fn editor_overlap_preserves_first_provider_ordinal() {
    let mut rng = Lcg(7);
    for round in 0..300 {
        let limit = 6 + round % 40;
        let spans: Vec<_> = (0..1 + rng.next(8)).map(|_| rng.range(limit)).collect();
        let runs = RunIndex::build(spans.iter());
        for off in 0..limit + 3 {
            let old = spans.iter().position(|r| r.contains(&off));
            assert_eq!(runs.at(off), old, "{spans:?} at {off}");
        }
    }
    let spans = [5..9, 2..7, 6..6, 0..3, 8..4];
    let runs = RunIndex::build(spans.iter());
    assert_eq!(
        runs.at(6),
        Some(0),
        "the earlier provider span wins on overlap"
    );
    assert_eq!(runs.at(2), Some(1));
    assert_eq!(runs.at(1), Some(3));
    assert_eq!(runs.at(9), None, "ranges are half open");
}

#[test]
#[allow(clippy::reversed_empty_ranges)]
fn editor_overlapping_highlights_paint_the_first_provider_tone() {
    fn overlapping(_: &str) -> Vec<(Range<usize>, SyntaxTone)> {
        vec![
            (4..9, SyntaxTone::Str),
            (0..6, SyntaxTone::Keyword),
            (7..7, SyntaxTone::Comment),
            (8..3, SyntaxTone::Comment),
        ]
    }
    let mut frame = Frame::new(40, 6);
    let mut e = CodeEditor::new(WidgetId::of("code-test"), "abcdefghij").highlighter(overlapping);
    frame.paint(&mut e, Interaction::default());
    let theme = &frame.theme;
    let fg = |off: u16| frame.fg(6 + off, 0);
    assert_eq!(fg(0), Some(theme.syntax_keyword));
    assert_eq!(fg(3), Some(theme.syntax_keyword));
    assert_eq!(fg(4), Some(theme.syntax_string), "first provider span wins");
    assert_eq!(fg(8), Some(theme.syntax_string));
    assert_eq!(
        fg(9),
        Some(theme.text_primary),
        "outside every span is plain"
    );
}

#[test]
fn editor_blocks_preserve_inclusive_end_and_reverse_ordinal() {
    let key = ProviderKey {
        text: 1,
        provider: 1,
    };
    let mut rng = Lcg(99);
    for round in 0..500 {
        let limit = 8 + round % 30;
        let blocks: Vec<_> = (0..rng.next(7)).map(|_| rng.range(limit)).collect();
        let snapshot = SegmentSnapshot::build(key, blocks.clone());
        for cur in 0..limit + 4 {
            assert_eq!(
                snapshot.block_at(cur),
                old_block_at(&blocks, cur),
                "{blocks:?} at {cur}"
            );
        }
    }
    let blocks = vec![10..20, 0..10, 5..5, 20..30, 3..8];
    let snapshot = SegmentSnapshot::build(key, blocks.clone());
    assert_eq!(
        snapshot.block_at(10),
        Some(10..20),
        "adjacent end/start: first ordinal"
    );
    assert_eq!(snapshot.block_at(20), Some(10..20));
    assert_eq!(snapshot.block_at(5), Some(0..10));
    assert_eq!(
        snapshot.block_at(40),
        Some(3..8),
        "after all: greatest original ordinal, not the greatest end"
    );
    let huge = vec![0..usize::MAX, 3..4];
    let snapshot = SegmentSnapshot::build(key, huge.clone());
    assert_eq!(
        snapshot.block_at(usize::MAX),
        old_block_at(&huge, usize::MAX)
    );
    assert_eq!(snapshot.block_at(2), Some(0..usize::MAX));
}

#[test]
fn editor_bracket_and_block_lookup_is_cached() {
    let mut rng = Lcg(3);
    for _ in 0..200 {
        let text: String = (0..rng.next(30))
            .map(|_| ['(', ')', '[', ']', 'x', '\n', '日'][rng.next(7)])
            .collect();
        let map = BracketMap::build(1, &text);
        for cur in 0..=text.len() + 1 {
            assert_eq!(
                map.pair_at(cur),
                old_bracket_pair(text.as_bytes(), cur),
                "{text:?} at {cur}"
            );
        }
    }

    reset();
    let id = WidgetId::of("code-test");
    let mut frame = Frame::new(100, 24);
    let mut e = editor(20_000);
    frame.paint(&mut e, focused(id));
    let warm = builds();
    assert_eq!(
        (warm.bracket_builds, warm.segment_builds, warm.syntax_builds),
        (1, 1, 1)
    );
    let last = e.text().len();
    for target in [last / 2, 5, last - 3, last / 3, 0] {
        e.jump_to(target);
        e.on_key(&key(KeyCode::Down));
        frame.paint(&mut e, focused(id));
        e.field.buffer.move_doc_end(false);
        frame.paint(&mut e, focused(id));
        e.on_key(&key(KeyCode::Left));
        frame.paint(&mut e, focused(id));
    }
    assert_eq!(
        builds(),
        work::Counts {
            rows: builds().rows,
            graphemes: builds().graphemes,
            ..warm
        },
        "caret movement across the document rebuilds no projection"
    );
    assert_eq!(count(&SEGMENTS), 1);
    assert_eq!(count(&HIGHLIGHTS), 1);
}

#[test]
fn editor_shared_block_cache_before_render_and_replacement() {
    reset();
    let mut e = CodeEditor::new(WidgetId::of("code-test"), "one\n\ntwo\n\nthree")
        .segmenter(statements as Segmenter);
    e.jump_to(6);
    assert_eq!(e.current_block(), Some(5..8));
    assert_eq!(e.blocks(), vec![0..3, 5..8, 10..15]);
    assert_eq!(e.selection_or_block(), Some(("two".to_owned(), 5..8)));
    assert_eq!(e.current_block(), Some(5..8));
    assert_eq!(count(&SEGMENTS), 1, "one segmentation per distinct key");

    e.field.buffer = TextBuffer::multi("alpha\n\nbeta");
    e.jump_to(8);
    assert_eq!(e.current_block(), Some(7..11));
    assert_eq!(e.blocks(), vec![0..5, 7..11]);
    assert_eq!(count(&SEGMENTS), 2, "a replaced buffer is a new key");

    e.field.buffer.select_range(0, 5);
    assert_eq!(e.selection_or_block(), Some(("alpha".to_owned(), 0..5)));
    assert_eq!(
        count(&SEGMENTS),
        2,
        "selection does not consult the segmenter"
    );

    e.invalidate_segmenter();
    e.current_block();
    e.current_block();
    assert_eq!(count(&SEGMENTS), 3, "an explicit invalidation reruns once");
    e.set_segmenter(Some(statements));
    e.blocks();
    assert_eq!(count(&SEGMENTS), 4, "every setter call is a new generation");
    e.set_segmenter(None);
    assert_eq!(e.blocks(), Vec::<Range<usize>>::new());
    assert_eq!(e.current_block(), None);
    assert!(e.segmenter_fn().is_none());
}

#[test]
fn editor_block_navigation_reads_the_cached_blocks() {
    reset();
    let mut e = CodeEditor::new(WidgetId::of("code-test"), "one\n\ntwo\n\nthree")
        .segmenter(statements as Segmenter)
        .edit_entry(EditEntry::Explicit);
    e.jump_to(0);
    assert_eq!(count(&SEGMENTS), 0);
    for expected in [5, 10] {
        e.on_key(&key(KeyCode::Char('}')));
        assert_eq!(e.cursor_offset(), expected);
    }
    for expected in [5, 0] {
        e.on_key(&key(KeyCode::Char('{')));
        assert_eq!(e.cursor_offset(), expected);
    }
    assert_eq!(count(&SEGMENTS), 1);
}

#[test]
fn empty_diagnostic_index_does_not_allocate_per_line() {
    let mut sizes = Vec::new();
    for lines in [1_000usize, 10_000, 100_000] {
        let buffer = TextBuffer::multi(document(lines));
        assert!(buffer.line_count() > lines);
        let (index, bytes) = test_alloc::measure(|| DiagnosticIndex::build(1, 1, &[], &buffer));
        assert_eq!(index.gutter(0), None);
        assert_eq!(index.at(0), None);
        sizes.push(bytes);
    }
    assert!(
        sizes.iter().all(|b| *b <= sizes[0] + 1024),
        "an index without diagnostics must not grow with the document: {sizes:?}"
    );
}

#[test]
fn editor_diagnostics_reads_back_the_diagnostics_in_the_order_they_were_set() {
    let mut e = CodeEditor::new(WidgetId::of("code-test"), "abcdefgh");
    assert!(e.diagnostics().is_empty(), "a new editor has none");
    e.set_diagnostics(vec![
        diagnostic(4..6, Severity::Warning),
        diagnostic(1..2, Severity::Error),
    ]);
    let got: Vec<_> = e
        .diagnostics()
        .iter()
        .map(|d| (d.range.clone(), d.severity))
        .collect();
    assert_eq!(got, [(4..6, Severity::Warning), (1..2, Severity::Error)]);
}

#[test]
fn editor_diagnostics_merge_exact_and_range_ordinals() {
    let buffer = TextBuffer::multi("abcdefgh");
    let range_first = vec![
        diagnostic(1..5, Severity::Warning),
        diagnostic(3..3, Severity::Error),
        diagnostic(3..3, Severity::Warning),
    ];
    let index = DiagnosticIndex::build(1, 1, &range_first, &buffer);
    assert_eq!(
        index.at(3),
        Some(0),
        "the earlier range beats a later exact one"
    );
    let exact_first = vec![
        diagnostic(3..3, Severity::Error),
        diagnostic(1..5, Severity::Warning),
        diagnostic(3..3, Severity::Warning),
    ];
    let index = DiagnosticIndex::build(1, 1, &exact_first, &buffer);
    assert_eq!(
        index.at(3),
        Some(0),
        "the earlier exact beats a later range"
    );
    assert_eq!(index.at(2), Some(1));
    assert_eq!(index.at(5), None);

    let mut rng = Lcg(21);
    for round in 0..300 {
        let limit = 6 + round % 20;
        let diags: Vec<_> = (0..rng.next(7))
            .map(|_| diagnostic(rng.range(limit), Severity::Warning))
            .collect();
        let index = DiagnosticIndex::build(1, 1, &diags, &buffer);
        for off in 0..limit + 3 {
            let old = diags.iter().position(|d| {
                d.range.contains(&off) || (d.range.is_empty() && d.range.start == off)
            });
            assert_eq!(index.at(off), old, "{diags:?} at {off}");
        }
        for cursor in 0..limit + 5 {
            let old = diags
                .iter()
                .enumerate()
                .min_by_key(|(_, d)| d.range.start.abs_diff(cursor))
                .map(|(i, _)| i);
            assert_eq!(index.nearest(cursor), old, "{diags:?} nearest {cursor}");
        }
    }

    let mut frame = Frame::new(40, 6);
    let mut e = CodeEditor::new(WidgetId::of("code-test"), "abcdefgh");
    e.set_diagnostics(vec![
        diagnostic(2..2, Severity::Error),
        diagnostic(1..4, Severity::Warning),
    ]);
    frame.paint(&mut e, Interaction::default());
    let underline = |frame: &Frame, off: u16| {
        frame
            .buf
            .cell((6 + off, 0))
            .unwrap()
            .style()
            .underline_color
    };
    assert_eq!(underline(&frame, 2), Some(frame.theme.error));
    assert_eq!(underline(&frame, 3), Some(frame.theme.warning));
    e.set_diagnostics(vec![
        diagnostic(1..4, Severity::Warning),
        diagnostic(2..2, Severity::Error),
    ]);
    frame.paint(&mut e, Interaction::default());
    assert_eq!(underline(&frame, 2), Some(frame.theme.warning));
}

#[test]
fn editor_diagnostic_gutter_uses_cached_first_ordinal() {
    let text = "aa\nbb\n\ncc\n";
    let buffer = TextBuffer::multi(text);
    let diags = vec![
        diagnostic(7..9, Severity::Warning),
        diagnostic(5..5, Severity::Error),
        diagnostic(3..4, Severity::Error),
        diagnostic(2..2, Severity::Warning),
        diagnostic(0..1, Severity::Error),
        diagnostic(10..10, Severity::Error),
        diagnostic(12..12, Severity::Error),
        diagnostic(99..100, Severity::Error),
    ];
    let index = DiagnosticIndex::build(1, 1, &diags, &buffer);
    let mut start = 0;
    for (line, l) in text.split('\n').enumerate() {
        let end = start + l.len();
        let old = diags
            .iter()
            .position(|d| d.range.start >= start && d.range.start <= end);
        assert_eq!(index.gutter(line), old, "line {line}");
        start = end + 1;
    }
    assert_eq!(
        index.gutter(1),
        Some(1),
        "the newline byte belongs to its own line"
    );
    assert_eq!(
        index.gutter(4),
        Some(5),
        "the trailing empty line has a gutter"
    );

    let id = WidgetId::of("code-test");
    let mut sizes = Vec::new();
    for count in [1_000usize, 10_000, 100_000] {
        reset();
        let lines = count;
        let mut frame = Frame::new(100, 24);
        let mut e = editor(lines);
        e.set_diagnostics(
            (0..count)
                .map(|i| diagnostic(i * 40..i * 40 + 3, Severity::Warning))
                .collect(),
        );
        frame.paint(&mut e, focused(id));
        assert_eq!(builds().diagnostic_builds, 1);
        let ((), bytes) = test_alloc::measure(|| frame.paint(&mut e, focused(id)));
        sizes.push(bytes);
        assert_eq!(builds().diagnostic_builds, 1, "repaint reuses the index");

        e.set_diagnostics(vec![diagnostic(0..1, Severity::Error)]);
        frame.paint(&mut e, focused(id));
        frame.paint(&mut e, focused(id));
        assert_eq!(
            builds().diagnostic_builds,
            2,
            "one rebuild per diagnostic revision"
        );
        e.field.buffer.set_text("changed\ntext");
        frame.paint(&mut e, focused(id));
        frame.paint(&mut e, focused(id));
        assert_eq!(
            builds().diagnostic_builds,
            3,
            "one rebuild per text revision"
        );
    }
    assert!(
        sizes.iter().all(|b| *b <= sizes[0] + 1024),
        "per-frame allocation must not grow with diagnostics: {sizes:?}"
    );
}

#[test]
fn closing_find_releases_the_match_runs() {
    let id = WidgetId::of("code-test");
    for close_after_enter in [false, true] {
        let mut frame = Frame::new(100, 24);
        let mut e = editor(200).edit_entry(EditEntry::Explicit);
        frame.paint(&mut e, focused(id));
        e.open_find();
        for c in "col_1".chars() {
            e.on_key(&key(KeyCode::Char(c)));
        }
        if close_after_enter {
            e.on_key(&key(KeyCode::Enter));
        }
        frame.paint(&mut e, focused(id));
        assert!(e.find_runs.is_some(), "an open find paints its matches");

        e.on_key(&key(KeyCode::Esc));
        assert!(e.find.is_none(), "esc closes the find");
        assert!(
            e.find_runs.is_none(),
            "the run index of a closed find is dropped (after enter: {close_after_enter})"
        );
    }
}

#[test]
fn editor_jump_to_uses_indexed_position_lookup() {
    fn doc(lines: usize) -> String {
        (0..lines)
            .map(|i| format!("{i} 日本語 e\u{301} SELECT 👨\u{200d}👩 end"))
            .collect::<Vec<_>>()
            .join("\n")
    }
    let text = doc(20_000);
    let mut e = CodeEditor::new(WidgetId::of("code-test"), &text);
    let mut frame = Frame::new(100, 24);
    frame.paint(&mut e, Interaction::default());
    let in_grapheme = text.find('e').unwrap() + 1;
    let targets = [
        0,
        text.len() / 2,
        text.len() / 2 + 1,
        text.len() - 2,
        text.len() + 50,
        text.find("日").unwrap() + 1,
        in_grapheme,
    ];
    for &target in &targets {
        let clamped = target.min(text.len());
        let pos = TextBuffer::pos_of(&text, clamped);
        e.jump_to(target);
        let mut expected = TextBuffer::multi(text.clone());
        expected.set_cursor_line_col(pos.line, pos.col);
        assert_eq!(
            e.cursor_offset(),
            expected.cursor_offset(),
            "target {target}"
        );
        assert!(e.scroll.visible_range().contains(&pos.line));
    }
    let warm = text_work::snapshot();
    for _ in 0..3 {
        for &target in &targets {
            e.jump_to(target);
        }
    }
    assert_eq!(
        text_work::snapshot(),
        warm,
        "repeat navigation builds no index or line table"
    );
}

#[test]
fn editor_find_scans_once_per_text_and_query() {
    reset();
    let id = WidgetId::of("code-test");
    let mut frame = Frame::new(100, 24);
    let mut e = editor(5_000).edit_entry(EditEntry::Explicit);
    frame.paint(&mut e, focused(id));
    e.open_find();
    for c in "col_4".chars() {
        e.on_key(&key(KeyCode::Char(c)));
    }
    let typed = builds().find_scans;
    assert_eq!(
        typed,
        1 + 5,
        "one scan on open and one per distinct query text"
    );
    e.on_key(&key(KeyCode::Enter));
    for _ in 0..4 {
        e.on_key(&key(KeyCode::Char('n')));
        frame.paint(&mut e, focused(id));
    }
    e.on_key(&key(KeyCode::Char('N')));
    e.set_find_current(2);
    frame.paint(&mut e, focused(id));
    e.field.buffer.move_doc_end(false);
    frame.paint(&mut e, focused(id));
    assert_eq!(
        builds().find_scans,
        typed,
        "match navigation and repaints never rescan"
    );
    let state = e.find_state().expect("find open");
    assert_eq!(state.current, 2);
    assert!(state.matches.len() > 100);

    e.set_find_query("col_49");
    assert_eq!(builds().find_scans, typed + 1);
    assert_eq!(e.find_state().unwrap().needle.text(), "col_49");
    e.set_find_query("col_49");
    assert_eq!(
        builds().find_scans,
        typed + 1,
        "an equal query is not rescanned"
    );
    e.field.buffer.insert_str("x");
    e.set_find_query("col_49");
    assert_eq!(
        builds().find_scans,
        typed + 2,
        "an edit invalidates the scan"
    );
}

#[test]
fn editor_edit_rebuilds_each_projection_once() {
    reset();
    let id = WidgetId::of("code-test");
    let mut frame = Frame::new(100, 24);
    let mut e = editor(2_000);
    e.push_diagnostic(diagnostic(4..8, Severity::Warning));
    frame.paint(&mut e, focused(id));
    let warm = builds();
    let text_warm = text_work::snapshot();

    e.on_key(&key(KeyCode::Down));
    e.on_key(&key(KeyCode::Right));
    frame.paint(&mut e, focused(id));
    assert_eq!(
        builds().syntax_builds,
        warm.syntax_builds,
        "a caret key is not an edit"
    );

    e.on_key(&key(KeyCode::Char('z')));
    assert!(e.text().contains('z'));
    frame.paint(&mut e, focused(id));
    frame.paint(&mut e, focused(id));
    let after = builds();
    assert_eq!(after.syntax_builds, warm.syntax_builds + 1);
    assert_eq!(after.diagnostic_builds, warm.diagnostic_builds + 1);
    assert_eq!(after.bracket_builds, warm.bracket_builds + 1);
    assert_eq!(after.segment_builds, warm.segment_builds + 1);
    assert_eq!(count(&HIGHLIGHTS), 2);
    assert_eq!(count(&SEGMENTS), 2);
    assert_eq!(
        text_work::snapshot().index_builds,
        text_warm.index_builds + 1
    );
}

#[test]
fn editor_measure_uses_the_line_index() {
    use crate::ui::measure::Measure;
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let e = editor(50_000);
    let size = e.measure(ratatui::layout::Size::new(80, 24), &ctx);
    assert_eq!(size.height, e.text().split('\n').count() as u16 + 1);
    let warm = text_work::snapshot();
    let (_, bytes) = test_alloc::measure(|| e.measure(ratatui::layout::Size::new(80, 24), &ctx));
    assert!(bytes < 256, "measure allocates {bytes} bytes");
    assert_eq!(text_work::snapshot(), warm);
}

#[test]
fn editor_caret_keys_do_not_copy_the_document() {
    let id = WidgetId::of("code-test");
    let mut per_size = Vec::new();
    for lines in [1_000usize, 10_000, 100_000] {
        let mut frame = Frame::new(100, 24);
        let mut e = editor(lines);
        frame.paint(&mut e, focused(id));
        let revision = e.field.buffer.revision();
        let mut worst = 0;
        for code in [
            KeyCode::Down,
            KeyCode::Right,
            KeyCode::Left,
            KeyCode::Up,
            KeyCode::End,
            KeyCode::Home,
        ] {
            let ((), bytes) = test_alloc::measure(|| {
                e.on_key(&key(code));
                frame.paint(&mut e, focused(id));
            });
            worst = worst.max(bytes);
        }
        assert_eq!(
            e.field.buffer.revision(),
            revision,
            "caret keys are not edits"
        );
        per_size.push((lines, worst));
    }
    let (_, small) = per_size[0];
    assert!(
        per_size.iter().all(|(_, bytes)| *bytes <= small + 4096),
        "a caret key and its frame must not allocate with the document: {per_size:?}"
    );
}

#[test]
fn highlighter_fn_returns_the_installed_provider() {
    let id = WidgetId::of("code-test");
    let plain = CodeEditor::new(id, "SELECT 1;\n");
    assert!(plain.highlighter_fn().is_none());
    let colored = CodeEditor::new(id, "SELECT 1;\n").highlighter(keywords);
    assert!(colored.highlighter_fn().is_some());
}

#[test]
fn invalidate_highlighter_makes_the_next_paint_recompute_unchanged_text() {
    reset();
    let mut frame = Frame::new(100, 12);
    let mut e = CodeEditor::new(WidgetId::of("code-test"), "SELECT 1;\n").highlighter(keywords);
    frame.paint(&mut e, Interaction::default());
    let after_first = count(&HIGHLIGHTS);
    frame.paint(&mut e, Interaction::default());
    assert_eq!(
        count(&HIGHLIGHTS),
        after_first,
        "an unchanged frame reuses the scan"
    );

    e.invalidate_highlighter();
    frame.paint(&mut e, Interaction::default());
    assert!(
        count(&HIGHLIGHTS) > after_first,
        "invalidate_highlighter drops the cached scan, so the provider runs again"
    );
}
