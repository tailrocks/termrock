use std::sync::Arc;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::TextBuffer;
use crate::text_index::work;

const SAMPLES: &[&str] = &[
    "",
    "\n",
    "abc",
    "abc\n",
    "\nabc",
    "a\n\nb\n",
    "日本語\nテキスト\n",
    "e\u{301}x\u{301}\u{301}\nz",
    "👨‍👩‍👧‍👦 family\n🇯🇵🇺🇸 flags\n",
    "tab\there\t\n\tx",
    "a\u{200b}b\u{200d}c\n\u{200b}",
    "wide: ａｂｃ ✔\u{fe0f} ❤\u{fe0f}\n",
];

fn old_floor(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .take_while(|i| *i <= offset)
        .last()
        .unwrap_or(0)
}

fn old_ceil(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .find(|i| *i >= offset)
        .unwrap_or(text.len())
}

fn old_offset_at(text: &str, line: usize, col: usize) -> usize {
    let mut start = 0;
    for (i, l) in text.split('\n').enumerate() {
        if i == line {
            let mut width = 0;
            for (gi, g) in l.grapheme_indices(true) {
                let w = UnicodeWidthStr::width(g);
                if width + w > col {
                    return start + gi;
                }
                width += w;
            }
            return start + l.len();
        }
        start += l.len() + 1;
    }
    text.len()
}

fn long_document(lines: usize) -> String {
    (0..lines)
        .map(|i| format!("line {i} 日本語 e\u{301}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn text_revision_tracks_content_not_cursor() {
    let mut buf = TextBuffer::multi("hello\nworld");
    let start = buf.revision();
    buf.move_doc_start(false);
    buf.move_right(true);
    buf.select_all();
    buf.clear_selection();
    buf.select_range(1, 3);
    assert_eq!(buf.revision(), start, "caret and selection are not content");

    buf.set_text("hello\nworld");
    assert_eq!(buf.revision(), start, "equal replacement is not an edit");
    buf.insert_str("");
    buf.insert_at(2, "");
    buf.remove_range(2..2);
    buf.select_range(1, 3);
    buf.insert_str("el");
    assert_eq!(
        buf.revision(),
        start,
        "equal selection replacement is no edit"
    );
    buf.move_doc_end(false);
    buf.delete();
    buf.delete_to_line_end();
    assert_eq!(buf.revision(), start, "deleting nothing is not an edit");
    buf.move_doc_start(false);
    buf.backspace();
    buf.delete_word_left();
    buf.delete_to_line_start();
    assert_eq!(
        buf.revision(),
        start,
        "deleting before the start is no edit"
    );

    let mut seen = vec![start];
    let mut edit = |buf: &mut TextBuffer, op: &dyn Fn(&mut TextBuffer)| {
        op(buf);
        let rev = buf.revision();
        assert!(!seen.contains(&rev), "a real edit gets a fresh ticket");
        seen.push(rev);
    };
    edit(&mut buf, &|b| b.insert_char('!'));
    edit(&mut buf, &|b| b.insert_str("ab"));
    edit(&mut buf, &|b| b.backspace());
    edit(&mut buf, &|b| b.insert_at(0, ">"));
    edit(&mut buf, &|b| b.remove_range(0..1));
    edit(&mut buf, &|b| b.set_text("fresh"));
    edit(&mut buf, &|b| {
        b.move_doc_start(false);
        b.delete();
    });
    edit(&mut buf, &|b| {
        b.move_doc_end(false);
        b.delete_word_left();
    });
    edit(&mut buf, &|b| {
        b.set_text("one two");
        b.delete_to_line_start();
    });
    edit(&mut buf, &|b| {
        b.set_text("one two");
        b.move_doc_start(false);
        b.delete_to_line_end();
    });
    edit(&mut buf, &|b| {
        b.set_text("sel");
        b.select_all();
        b.insert_str("x");
    });
}

#[test]
fn text_replacement_cannot_reuse_cache_identity() {
    let a = TextBuffer::multi("same");
    let b = TextBuffer::multi("same");
    assert_eq!(a, b, "equality stays semantic");
    assert_ne!(
        a.revision(),
        b.revision(),
        "distinct constructions never share"
    );
    assert_ne!(
        TextBuffer::default().revision(),
        TextBuffer::default().revision()
    );

    let mut c = a.clone();
    assert_eq!(
        c.revision(),
        a.revision(),
        "an unchanged clone keeps its ticket"
    );
    c.insert_str("!");
    assert_ne!(c.revision(), a.revision());
    assert_eq!(a.text(), "same");
    assert_eq!(a.line_count(), 1);
    assert_eq!(c.line_range(0), Some(0..5));
}

#[test]
fn text_index_preserves_unicode_and_trailing_line() {
    for text in SAMPLES {
        let buf = TextBuffer::multi(*text);
        let text = buf.text();
        assert_eq!(buf.line_count(), text.split('\n').count(), "{text:?}");
        for (i, line) in text.split('\n').enumerate() {
            assert_eq!(buf.line_width(i), Some(UnicodeWidthStr::width(line)));
            assert_eq!(&text[buf.line_range(i).unwrap()], line);
        }
        assert_eq!(buf.line_range(buf.line_count()), None);
        assert_eq!(
            buf.max_line_width(),
            text.split('\n').map(UnicodeWidthStr::width).max().unwrap()
        );
        for offset in 0..=text.len() + 2 {
            assert_eq!(
                buf.position_at(offset),
                TextBuffer::pos_of(text, offset),
                "position_at({offset}) in {text:?}"
            );
            let mut moved = buf.clone();
            moved.select_range(offset, offset);
            assert_eq!(
                moved.cursor_offset(),
                old_floor(text, offset),
                "{text:?} {offset}"
            );
            assert_eq!(
                TextBuffer::floor_boundary(text, offset),
                old_floor(text, offset)
            );
            assert_eq!(
                TextBuffer::ceil_boundary(text, offset),
                old_ceil(text, offset),
                "ceil {offset} in {text:?}"
            );
        }
        for line in 0..buf.line_count() + 2 {
            for col in 0..40 {
                assert_eq!(
                    buf.offset_at(line, col),
                    old_offset_at(text, line, col),
                    "offset_at({line}, {col}) in {text:?}"
                );
            }
        }
    }
}

#[test]
fn text_index_selection_line_helpers_match_the_text_scan() {
    for text in SAMPLES {
        let base = TextBuffer::multi(*text);
        let len = base.text().len();
        for a in 0..=len {
            for b in 0..=len {
                let mut buf = base.clone();
                buf.select_range(a, b);
                let Some(r) = buf.selection() else { continue };
                let before = &buf.text()[..r.end];
                let expect_last =
                    before.matches('\n').count() - usize::from(before.ends_with('\n'));
                let expect_first = TextBuffer::pos_of(buf.text(), r.start).line;
                assert_eq!(
                    buf.selection_lines(),
                    (expect_first, expect_last),
                    "{text:?} {r:?}"
                );
                assert_eq!(buf.has_selection_lines(), buf.text()[r].contains('\n'));
            }
        }
    }
}

#[test]
fn issue40_cloned_buffer_does_not_deep_clone_index() {
    let buf = TextBuffer::multi(long_document(2_000));
    let far = buf.text().len() - 3;
    let warm = buf.position_at(far);
    let before = work::snapshot();
    let clone = buf.clone();
    assert!(
        Arc::ptr_eq(buf.index.get().unwrap(), clone.index.get().unwrap()),
        "a clone shares the immutable index"
    );
    assert_eq!(clone.position_at(far), warm);
    assert_eq!(
        work::snapshot(),
        before,
        "a clone does not rebuild or copy tables"
    );

    let mut edited = clone.clone();
    edited.insert_str("x");
    assert!(
        edited.index.get().is_none(),
        "an edit drops only its own index"
    );
    assert!(buf.index.get().is_some() && clone.index.get().is_some());
}

#[test]
fn issue40_selection_only_frame_skips_prefix_scan() {
    let mut buf = TextBuffer::multi(long_document(20_000));
    assert_eq!(buf.line_count(), 20_000);
    let built = work::snapshot();
    assert_eq!(built.index_builds, 1);

    let caret_frame = |buf: &mut TextBuffer| {
        buf.select_range(buf.text().len() / 2, buf.text().len() - 4);
        buf.selection_lines();
        buf.has_selection_lines();
        buf.cursor_pos();
        buf.set_cursor_line_col(12_345, 7);
        buf.move_up(false);
        buf.move_down(true);
        buf.cursor_pos();
    };
    caret_frame(&mut buf);
    let first = work::snapshot();
    assert_eq!(
        first.index_builds, 1,
        "caret-only events never rebuild the index"
    );
    assert!(
        first.grapheme_tables <= 8,
        "only the touched lines build tables: {first:?}"
    );
    caret_frame(&mut buf);
    caret_frame(&mut buf);
    assert_eq!(work::snapshot(), first, "repeated caret frames add no work");

    buf.clear_selection();
    buf.insert_str("x");
    assert_eq!(buf.line_count(), 20_000);
    assert_eq!(
        work::snapshot().index_builds,
        2,
        "a real edit rebuilds exactly once"
    );
}
