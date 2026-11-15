use std::cell::Cell;
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::{CodeEditor, Highlighter, Segmenter};
use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::WidgetId;
use crate::test_alloc;
use crate::theme::{ColorLevel, SyntaxTone, Theme};
use crate::ui::ctx::{Interaction, RenderCtx};

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
    editor.highlighter = Some(f);
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
            *bytes <= small + small / 2 + 4096,
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
    let keyword = frame.fg(4, 0);
    assert_eq!(keyword, Some(frame.theme.syntax_keyword));

    set_highlighter(&mut e, strings);
    frame.paint(&mut e, Interaction::default());
    assert_eq!(
        frame.fg(4, 0),
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
