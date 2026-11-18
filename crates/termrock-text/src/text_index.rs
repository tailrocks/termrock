use std::ops::Range;
use std::sync::OnceLock;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[cfg(test)]
pub(crate) mod work {
    use std::cell::Cell;

    thread_local! {
        static INDEX_BUILDS: Cell<usize> = const { Cell::new(0) };
        static INDEX_BYTES: Cell<usize> = const { Cell::new(0) };
        static TABLES: Cell<usize> = const { Cell::new(0) };
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct Snapshot {
        pub index_builds: usize,
        pub index_bytes: usize,
        pub grapheme_tables: usize,
    }

    pub(super) fn index_built(bytes: usize) {
        INDEX_BUILDS.with(|c| c.set(c.get() + 1));
        INDEX_BYTES.with(|c| c.set(c.get() + bytes));
    }

    pub(super) fn table_built() {
        TABLES.with(|c| c.set(c.get() + 1));
    }

    pub fn snapshot() -> Snapshot {
        Snapshot {
            index_builds: INDEX_BUILDS.with(Cell::get),
            index_bytes: INDEX_BYTES.with(Cell::get),
            grapheme_tables: TABLES.with(Cell::get),
        }
    }
}

#[derive(Debug)]
pub(crate) struct LineIndex {
    lines: Vec<LineMeta>,
    max_width: usize,
}

#[derive(Debug)]
struct LineMeta {
    range: Range<usize>,
    width: usize,
    graphemes: OnceLock<Graphemes>,
}

#[derive(Debug)]
struct Graphemes {
    starts: Vec<usize>,
    cols: Vec<usize>,
}

impl Graphemes {
    fn build(text: &str, range: &Range<usize>) -> Self {
        #[cfg(test)]
        work::table_built();
        let line = &text[range.clone()];
        let mut starts = Vec::new();
        let mut cols = Vec::new();
        let mut col = 0;
        for (i, g) in line.grapheme_indices(true) {
            starts.push(range.start + i);
            cols.push(col);
            col += UnicodeWidthStr::width(g);
        }
        cols.push(col);
        Self { starts, cols }
    }

    fn first_ending_after(&self, col: usize) -> usize {
        self.cols[1..].partition_point(|end| *end <= col)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphemeAt<'a> {
    pub offset: usize,
    pub text: &'a str,
    pub col: usize,
    pub width: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LineView<'a> {
    line: &'a str,
    start: usize,
    table: &'a Graphemes,
}

impl<'a> LineView<'a> {
    pub fn text(&self) -> &'a str {
        self.line
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn graphemes_from_col(&self, col: usize) -> impl Iterator<Item = GraphemeAt<'a>> + use<'a> {
        let (line, start, table) = (self.line, self.start, self.table);
        let first = table.first_ending_after(col);
        (first..table.starts.len()).map(move |i| {
            let offset = table.starts[i];
            let end = table
                .starts
                .get(i + 1)
                .copied()
                .unwrap_or(start + line.len());
            GraphemeAt {
                offset,
                text: &line[offset - start..end - start],
                col: table.cols[i],
                width: table.cols[i + 1] - table.cols[i],
            }
        })
    }
}

impl LineIndex {
    pub fn build(text: &str) -> Self {
        #[cfg(test)]
        work::index_built(text.len());
        let mut start = 0;
        let mut max_width = 0;
        let lines = text
            .split('\n')
            .map(|line| {
                let range = start..start + line.len();
                start = range.end.saturating_add(1).min(text.len());
                let width = UnicodeWidthStr::width(line);
                max_width = max_width.max(width);
                LineMeta {
                    range,
                    width,
                    graphemes: OnceLock::new(),
                }
            })
            .collect();
        Self { lines, max_width }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn max_width(&self) -> usize {
        self.max_width
    }

    pub fn range(&self, line: usize) -> Option<Range<usize>> {
        self.lines.get(line).map(|l| l.range.clone())
    }

    pub fn width(&self, line: usize) -> Option<usize> {
        self.lines.get(line).map(|l| l.width)
    }

    pub fn line_of(&self, offset: usize) -> usize {
        self.lines
            .partition_point(|l| l.range.start <= offset)
            .saturating_sub(1)
    }

    fn table(&self, text: &str, line: usize) -> &Graphemes {
        let meta = &self.lines[line];
        meta.graphemes
            .get_or_init(|| Graphemes::build(text, &meta.range))
    }

    pub fn view<'a>(&'a self, text: &'a str, line: usize) -> Option<LineView<'a>> {
        let meta = self.lines.get(line)?;
        Some(LineView {
            line: &text[meta.range.clone()],
            start: meta.range.start,
            table: self.table(text, line),
        })
    }

    pub fn floor(&self, text: &str, offset: usize) -> usize {
        if offset >= text.len() {
            return text.len();
        }
        let line = self.line_of(offset);
        let meta = &self.lines[line];
        if offset >= meta.range.end {
            return meta.range.end;
        }
        let table = self.table(text, line);
        let g = table.starts.partition_point(|s| *s <= offset);
        table.starts[g.saturating_sub(1)]
    }

    pub fn position(&self, text: &str, offset: usize) -> (usize, usize) {
        let offset = offset.min(text.len());
        let line = self.line_of(offset);
        let meta = &self.lines[line];
        if offset >= meta.range.end {
            return (line, meta.width);
        }
        let table = self.table(text, line);
        let g = table.starts.partition_point(|s| *s <= offset);
        (line, table.cols[g.saturating_sub(1)])
    }

    pub fn offset_at(&self, text: &str, line: usize, col: usize) -> usize {
        let Some(meta) = self.lines.get(line) else {
            return text.len();
        };
        let table = self.table(text, line);
        let g = table.first_ending_after(col);
        table.starts.get(g).copied().unwrap_or(meta.range.end)
    }
}
