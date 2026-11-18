use unicode_segmentation::UnicodeSegmentation;

use super::{WrapOpts, wrap_mapped, wrap_with};

const WRAP_CORPUS: &[&str] = &[
    "abcdefghijkl",
    "hello world foo bar baz",
    "  indented text that wraps around here",
    "a  b   c    d     e",
    "trailing spaces   ",
    "     ",
    "",
    "a\nb",
    "a\n\nb c d e f g",
    "key   value text that is long enough to wrap",
    "日本語のテキストです",
    "e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}",
    "👨‍👩‍👧‍👦 family 👨‍👩‍👧‍👦 x",
    "x 日本 y 語 z",
    "long    gap   with   many   spaces inside   it",
    "\ttab\tseparated words",
    "   leading and trailing   ",
    "https://example.com/a/very/long/url/that/never/breaks?x=1&y=2",
    "word word word word word word word word",
    "    four-space indent with a    hang gap here",
    "a b",
    " x",
    "ab  ",
    "ab\n  cd  ef",
    "a\u{200b}b c",
];

const GOLDEN: &str = include_str!("wrap_golden.txt");

fn corpus_runs() -> Vec<(usize, usize, &'static str, WrapOpts)> {
    let mut runs = Vec::new();
    for i in 0..WRAP_CORPUS.len() {
        for w in 0..=14 {
            runs.push((i, w, "default", WrapOpts::default()));
            runs.push((i, w, "plain", WrapOpts::PLAIN));
        }
    }
    runs
}

#[test]
fn wrap_strings_match_the_output_recorded_before_provenance() {
    let mut now = String::new();
    for (i, w, name, opts) in corpus_runs() {
        let rows = wrap_with(WRAP_CORPUS[i], w, opts);
        now.push_str(&format!("{i}|{w}|{name}|{}\n", rows.join("\u{23ce}")));
    }
    assert_eq!(now, GOLDEN);
}

#[test]
fn mapped_rows_carry_the_same_text_and_faithful_provenance() {
    for (i, w, name, opts) in corpus_runs() {
        let src = WRAP_CORPUS[i];
        let label = format!("{src:?} w={w} {name}");
        let rows = wrap_mapped(src, w, opts);
        let texts: Vec<_> = rows.iter().map(|r| r.text.clone()).collect();
        assert_eq!(texts, wrap_with(src, w, opts), "{label}");

        let boundaries: Vec<usize> = src
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .chain([src.len()])
            .collect();
        let mut cursor = 0;
        for row in &rows {
            let mut col = 0;
            let mut shown = String::new();
            for cell in &row.cells {
                assert!(cell.columns.start >= col, "{label}: columns ordered");
                col = cell.columns.end;
                assert!(cell.bytes.start >= cursor, "{label}: bytes ordered");
                cursor = cell.bytes.end;
                assert!(boundaries.contains(&cell.bytes.start), "{label}");
                assert!(boundaries.contains(&cell.bytes.end), "{label}");
                if cell.bytes.is_empty() {
                    assert!(!cell.columns.is_empty(), "{label}: synthetic indent");
                    shown.push_str(&" ".repeat(cell.columns.len()));
                } else {
                    let original = &src[cell.bytes.clone()];
                    let ellipsis = original.width_cells() != cell.columns.len();
                    shown.push_str(if ellipsis { "…" } else { original });
                }
            }
            assert_eq!(shown, row.text, "{label}: cells reproduce the row");
        }
    }
}

trait WidthCells {
    fn width_cells(&self) -> usize;
}

impl WidthCells for str {
    fn width_cells(&self) -> usize {
        super::width(self)
    }
}
