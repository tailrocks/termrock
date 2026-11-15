use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(crate) const TAB_SPACES: &str = "    ";

pub(crate) fn expand_tabs(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains('\t') {
        text.replace('\t', TAB_SPACES).into()
    } else {
        text.into()
    }
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn width(s: &str) -> usize {
    crate::ui::fitting::work::width();
    UnicodeWidthStr::width(s)
}

pub fn wider_than(s: &str, max: usize) -> bool {
    if s.len() <= max {
        return false;
    }
    let mut end = max.saturating_mul(4).saturating_add(64);
    loop {
        if end >= s.len() {
            return width(s) > max;
        }
        let mut cut = end;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        if width(&s[..cut]) > max.saturating_add(2) {
            return true;
        }
        end = end.saturating_mul(2);
    }
}

pub fn width_capped(s: &str, cap: usize) -> usize {
    if wider_than(s, cap) {
        cap.saturating_add(1)
    } else {
        width(s)
    }
}

pub fn clip(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut w = 0;
    for g in s.graphemes(true) {
        let gw = width(g);
        if w + gw > max {
            break;
        }
        out.push_str(g);
        w += gw;
    }
    out
}

pub fn truncate(s: &str, max: usize) -> String {
    if !wider_than(s, max) {
        return s.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for g in s.graphemes(true) {
        let gw = width(g);
        if w + gw > max - 1 {
            break;
        }
        out.push_str(g);
        w += gw;
    }
    out.push('…');
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextClip {
    Clip,
    Ellipsis,
}

fn paintable_cells(grapheme: &str) -> usize {
    if grapheme.contains(char::is_control) {
        0
    } else {
        width(grapheme)
    }
}

fn clusters_fit(source: &str, budget: usize) -> bool {
    let mut used = 0usize;
    for g in source.graphemes(true) {
        used = used.saturating_add(paintable_cells(g));
        if used > budget {
            return false;
        }
    }
    true
}

/// Paints `source` as whole grapheme clusters on row `offset.1` of `owner`,
/// starting at column `offset.0`, and returns the cells consumed (synthetic
/// ellipsis included). Writes never leave `owner` intersected with `buf`.
///
/// `style` receives each cluster's byte range in `source` and its column
/// relative to the first painted cell. The synthetic ellipsis gets `None`, so
/// a match in hidden text never styles it. A cluster too wide for the room
/// left is dropped and later clusters do not shift back into its cells. A wide
/// cluster is styled across all of its cells with its own style. A dropped wide
/// cluster leaves blank cells after the ellipsis: `ab界界` in 4 cells paints
/// `ab…` and one blank cell, because later clusters do not shift back.
pub(crate) fn paint_graphemes(
    buf: &mut Buffer,
    owner: Rect,
    offset: (usize, usize),
    source: &str,
    mode: TextClip,
    mut style: impl FnMut(Option<Range<usize>>, usize) -> Style,
) -> usize {
    let clip = owner.intersection(buf.area);
    let x0 = usize::from(owner.x).saturating_add(offset.0);
    let y = usize::from(owner.y).saturating_add(offset.1);
    if offset.0 >= usize::from(owner.width)
        || offset.1 >= usize::from(owner.height)
        || clip.is_empty()
        || y < usize::from(clip.y)
        || y >= usize::from(clip.bottom())
        || x0 >= usize::from(clip.right())
    {
        return 0;
    }
    let y = y as u16;
    let budget = usize::from(clip.right()) - x0;
    let ellipsis = mode == TextClip::Ellipsis && !clusters_fit(source, budget);
    let content = budget - usize::from(ellipsis);
    let mut col = 0usize;
    for (byte, g) in source.grapheme_indices(true) {
        let cells = paintable_cells(g);
        if cells == 0 {
            continue;
        }
        if cells > content - col {
            break;
        }
        let x = x0 + col;
        if x >= usize::from(clip.x) {
            let ink = style(Some(byte..byte + g.len()), col);
            put(buf, x as u16, y, g, cells, ink);
        }
        col += cells;
    }
    if ellipsis {
        let x = x0 + col;
        if x >= usize::from(clip.x) {
            put(buf, x as u16, y, "…", 1, style(None, col));
        }
        col += 1;
    }
    col
}

/// Paints `source` as whole clusters from column `col` of `owner`'s top row,
/// dropping what does not fit and never writing outside `owner`. Returns the
/// cells consumed.
pub(crate) fn paint_clipped(
    buf: &mut Buffer,
    owner: Rect,
    col: usize,
    source: &str,
    ink: Style,
) -> usize {
    paint_graphemes(buf, owner, (col, 0), source, TextClip::Clip, |_, _| ink)
}

/// Like [`paint_clipped`], with a one-cell ellipsis when `source` is cut.
pub(crate) fn paint_truncated(
    buf: &mut Buffer,
    owner: Rect,
    col: usize,
    source: &str,
    ink: Style,
) -> usize {
    paint_graphemes(buf, owner, (col, 0), source, TextClip::Ellipsis, |_, _| ink)
}

fn put(buf: &mut Buffer, x: u16, y: u16, symbol: &str, cells: usize, ink: Style) {
    buf.set_stringn(x, y, symbol, cells, ink);
    for dx in 1..cells as u16 {
        buf[(x + dx, y)].set_style(ink);
    }
}

pub fn slice_cells(s: &str, offset: usize, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let clipped_right = width(s) > offset.saturating_add(max);
    let visible = max - usize::from(clipped_right);
    if visible == 0 {
        return "…".to_owned();
    }
    let end = offset.saturating_add(visible);
    let mut out = String::new();
    let mut col = 0;
    let mut out_width = 0;
    for g in s.graphemes(true) {
        let start = col;
        col += width(g);
        if col <= offset {
            continue;
        }
        if start >= end {
            break;
        }
        if start <= offset && offset > 0 {
            let cells = col.min(end) - offset;
            out.push('…');
            out.push_str(&" ".repeat(cells - 1));
            out_width += cells;
        } else if col <= end {
            out.push_str(g);
            out_width += col - start;
        } else {
            break;
        }
    }
    if clipped_right {
        out.push_str(&" ".repeat(visible.saturating_sub(out_width)));
        out.push('…');
    }
    out
}

pub fn truncate_middle(s: &str, max: usize) -> String {
    if !wider_than(s, max) {
        return s.to_owned();
    }
    if max < 5 {
        return truncate(s, max);
    }
    let keep_end = (max - 1) / 3;
    let keep_start = max - 1 - keep_end;

    let gs = s.graphemes(true);
    let mut head = String::new();
    let mut w = 0;
    for g in gs.clone() {
        let gw = width(g);
        if w + gw > keep_start {
            break;
        }
        head.push_str(g);
        w += gw;
    }
    let mut tail = String::new();
    let mut w = 0;
    for g in gs.rev() {
        let gw = width(g);
        if w + gw > keep_end {
            break;
        }
        tail.insert_str(0, g);
        w += gw;
    }
    format!("{head}…{tail}")
}

pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

pub fn fit(s: &str, w: usize) -> String {
    let t = truncate(s, w);
    let pad = w.saturating_sub(width(&t));
    format!("{t}{}", " ".repeat(pad))
}

pub fn fit_right(s: &str, w: usize) -> String {
    let t = truncate(s, w);
    let pad = w.saturating_sub(width(&t));
    format!("{}{t}", " ".repeat(pad))
}

pub fn center_x(area: ratatui::layout::Rect, text: &str) -> u16 {
    let w = width_capped(text, usize::from(u16::MAX)) as u16;
    area.x + area.width.saturating_sub(w) / 2
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

pub fn truncate_sep(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_owned();
    }
    if max < 4 {
        return truncate(s, max);
    }
    let mut best: Option<usize> = None;
    let mut pos = 0;
    while let Some(i) = s[pos..].find(" · ") {
        let at = pos + i;
        let w = width(&s[..at]);
        if w < max && w * 3 >= max {
            best = Some(at);
        }
        if w + 1 > max {
            break;
        }
        pos = at + 3;
    }
    match best {
        Some(at) => format!("{}…", &s[..at]),
        None => truncate(s, max),
    }
}

pub fn crumb_fit(label: &str, max: usize) -> String {
    if width(label) <= max {
        return label.to_owned();
    }
    let parts: Vec<&str> = label.split(" › ").collect();
    if parts.len() >= 3 {
        let first = parts[0];
        let last = parts[parts.len() - 1];
        let short = format!("{first} › … › {last}");
        if width(&short) <= max {
            return short;
        }
        let room = max.saturating_sub(width(first) + 8);
        return format!("{first} › … › {}", truncate_middle(last, room.max(6)));
    }
    truncate_middle(label, max)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DurationStyle {
    Compact,

    #[default]
    Spoken,
}

pub fn duration(elapsed: std::time::Duration, style: DurationStyle) -> String {
    match style {
        DurationStyle::Compact => {
            duration_compact(elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
        }
        DurationStyle::Spoken => duration_spoken(elapsed.as_secs()),
    }
}

fn duration_compact(ms: u64) -> String {
    match ms {
        0 => "<1 ms".into(),
        ms if ms < 1000 => format!("{ms} ms"),
        ms if ms < 60_000 => format!("{:.2} s", ms as f64 / 1000.0),
        ms => format!("{}m {}s", ms / 60_000, (ms % 60_000) / 1000),
    }
}

fn duration_spoken(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let pair = |big: u64, big_unit: &str, small: u64, small_unit: &str| {
        if small > 0 {
            format!("{big} {big_unit} {small} {small_unit}")
        } else {
            format!("{big} {big_unit}")
        }
    };
    if d > 0 {
        pair(d, "d", h, "h")
    } else if h > 0 {
        pair(h, "h", m, "min")
    } else if m > 0 {
        pair(m, "min", s, "s")
    } else {
        format!("{s} s")
    }
}

pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrapOpts {
    pub keep_indent: bool,

    pub hang_at_gap: Option<u8>,
}

impl Default for WrapOpts {
    fn default() -> Self {
        Self {
            keep_indent: true,
            hang_at_gap: Some(3),
        }
    }
}

impl WrapOpts {
    pub const PLAIN: Self = Self {
        keep_indent: false,
        hang_at_gap: None,
    };
}

pub fn wrap(s: &str, w: usize) -> Vec<String> {
    wrap_with(s, w, WrapOpts::default())
}

pub fn wrap_with(s: &str, w: usize, opts: WrapOpts) -> Vec<String> {
    wrap_rows(s, w, opts, false)
        .into_iter()
        .map(|row| row.text)
        .collect()
}

/// One displayed grapheme (or synthetic indent) and the source bytes it
/// stands for. `bytes` is empty for continuation indent, which maps to the
/// boundary before the row's first real grapheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MappedCell {
    pub columns: Range<usize>,
    pub bytes: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MappedRow {
    pub text: String,
    pub cells: Vec<MappedCell>,
    /// Source byte after the row's last grapheme, or where an empty row sits.
    pub end: usize,
}

/// The same line breaking as [`wrap_with`], with each row's source provenance.
pub(crate) fn wrap_mapped(s: &str, w: usize, opts: WrapOpts) -> Vec<MappedRow> {
    wrap_rows(s, w, opts, true)
}

fn wrap_rows(s: &str, w: usize, opts: WrapOpts, track: bool) -> Vec<MappedRow> {
    let w = w.max(1);
    let mut rows = Vec::new();
    let mut offset = 0;
    for para in s.split('\n') {
        let body = para.trim_start_matches(' ');
        let lead = para.len() - body.len();
        let first = if opts.keep_indent { lead } else { 0 };
        let mut cont = first;
        if let Some(gap) = opts.hang_at_gap.filter(|g| *g > 0)
            && let Some(col) = hang_column(body, gap as usize)
        {
            cont = first + col;
        }

        let first = if first * 2 <= w { first } else { 0 };
        let cont = if cont * 2 <= w { cont } else { first };
        wrap_para(
            body,
            ParaSpec {
                w,
                first,
                cont,
                at: offset,
                body_at: offset + lead,
                track,
            },
            &mut rows,
        );
        offset += para.len() + 1;
    }
    rows
}

fn hang_column(s: &str, gap: usize) -> Option<usize> {
    let at = s.find(&" ".repeat(gap))?;
    let rest = s[at..].trim_start_matches(' ');
    if rest.is_empty() {
        return None;
    }
    Some(width(&s[..s.len() - rest.len()]))
}

struct ParaSpec {
    w: usize,
    first: usize,
    cont: usize,
    at: usize,
    body_at: usize,
    track: bool,
}

struct Para<'a> {
    rows: &'a mut Vec<MappedRow>,
    line: String,
    cells: Vec<MappedCell>,
    row_at: usize,
    track: bool,
    lw: usize,
    base: usize,
    cont: usize,
    w: usize,
}

impl Para<'_> {
    fn empty(&self) -> bool {
        self.lw == self.base
    }

    fn finish_row(&mut self) {
        let end = self.cells.last().map_or(self.row_at, |c| c.bytes.end);
        self.rows.push(MappedRow {
            text: std::mem::take(&mut self.line),
            cells: std::mem::take(&mut self.cells),
            end,
        });
    }

    fn break_line(&mut self, at: usize) {
        self.finish_row();
        self.line = " ".repeat(self.cont);
        self.lw = self.cont;
        self.base = self.cont;
        self.row_at = at;
        if self.track && self.cont > 0 {
            self.cells.push(MappedCell {
                columns: 0..self.cont,
                bytes: at..at,
            });
        }
    }

    fn put(&mut self, shown: &str, bytes: Range<usize>) {
        let gw = width(shown);
        if self.track {
            self.cells.push(MappedCell {
                columns: self.lw..self.lw + gw,
                bytes,
            });
        }
        self.line.push_str(shown);
        self.lw += gw;
    }

    fn put_word(&mut self, word: &str, ww: usize, at: usize) {
        if self.track {
            let start = self.lw;
            let mut col = start;
            for (i, g) in word.grapheme_indices(true) {
                let gw = width(g);
                self.cells.push(MappedCell {
                    columns: col..col + gw,
                    bytes: at + i..at + i + g.len(),
                });
                col += gw;
            }
        }
        self.line.push_str(word);
        self.lw += ww;
    }

    fn push_word(&mut self, word: &str, ww: usize, at: usize) {
        if self.lw + ww <= self.w {
            self.put_word(word, ww, at);
        } else {
            self.hard_wrap(word, at);
        }
    }

    fn hard_wrap(&mut self, word: &str, at: usize) {
        for (i, g) in word.grapheme_indices(true) {
            let room = (self.w - self.base).max(1);
            let shown = if width(g) > room { "…" } else { g };
            let gw = width(shown);
            if self.lw + gw > self.w && !self.empty() {
                self.break_line(at + i);
            }
            self.put(shown, at + i..at + i + g.len());
        }
    }
}

fn wrap_para(body: &str, spec: ParaSpec, rows: &mut Vec<MappedRow>) {
    let ParaSpec {
        w,
        first,
        cont,
        at,
        body_at,
        track,
    } = spec;
    let mut p = Para {
        rows,
        line: " ".repeat(first),
        cells: Vec::new(),
        row_at: at,
        track,
        lw: first,
        base: first,
        cont,
        w,
    };
    if track {
        for i in 0..first {
            p.cells.push(MappedCell {
                columns: i..i + 1,
                bytes: at + i..at + i + 1,
            });
        }
    }
    let mut next = body_at;
    for word in body.split(' ') {
        let word_at = next;
        next += word.len() + 1;
        let ww = width(word);
        if p.empty() {
            p.push_word(word, ww, word_at);
        } else if p.lw + 1 + ww <= w {
            p.put(" ", word_at - 1..word_at);
            p.put_word(word, ww, word_at);
        } else {
            p.break_line(word_at);
            p.push_word(word, ww, word_at);
        }
    }
    p.finish_row();
}

struct SearchText {
    text: String,
    origins: Vec<(Range<usize>, Range<usize>)>,
}

impl SearchText {
    fn new(source: &str, lowercase: bool) -> Self {
        let text = if lowercase {
            source.to_lowercase()
        } else {
            source.to_owned()
        };
        let mut chars = text.char_indices().peekable();
        let mut origins = Vec::new();
        for (start, grapheme) in source.grapheme_indices(true) {
            let mapped_start = chars.peek().map_or(text.len(), |(offset, _)| *offset);
            let count = if lowercase {
                grapheme.chars().map(|c| c.to_lowercase().count()).sum()
            } else {
                grapheme.chars().count()
            };
            for _ in 0..count {
                chars.next();
            }
            let mapped_end = chars.peek().map_or(text.len(), |(offset, _)| *offset);
            origins.push((mapped_start..mapped_end, start..start + grapheme.len()));
        }
        Self { text, origins }
    }

    fn original_range(&self, range: Range<usize>) -> Range<usize> {
        let first = self
            .origins
            .partition_point(|(mapped, _)| mapped.end <= range.start);
        let end = self
            .origins
            .partition_point(|(mapped, _)| mapped.start < range.end);
        self.origins[first].1.start..self.origins[end - 1].1.end
    }

    fn matched_offsets(&self, range: Range<usize>) -> Vec<usize> {
        self.origins
            .iter()
            .filter(|(mapped, _)| mapped.start < range.end && range.start < mapped.end)
            .map(|(_, original)| original.start)
            .collect()
    }
}

pub fn find_ranges(text: &str, needle: &str, case_sensitive: bool) -> Vec<Range<usize>> {
    if needle.is_empty() {
        return Vec::new();
    }
    let hay = SearchText::new(text, !case_sensitive);
    let needle = if case_sensitive {
        needle.to_owned()
    } else {
        needle.to_lowercase()
    };
    let mut ranges: Vec<_> = hay
        .text
        .match_indices(&needle)
        .map(|(start, matched)| hay.original_range(start..start + matched.len()))
        .collect();

    ranges.dedup();
    ranges
}

pub fn fuzzy(label: &str, word: &str) -> Option<(u32, Vec<usize>)> {
    if word.is_empty() {
        return Some((0, vec![]));
    }
    let l = SearchText::new(label, true);
    let w = word.to_lowercase();
    if l.text.starts_with(&w) {
        return Some((0, l.matched_offsets(0..w.len())));
    }
    if let Some(p) = l.text.find(&w) {
        let boundary = p == 0 || matches!(l.text.as_bytes()[p - 1], b'_' | b'.');
        return Some((
            if boundary { 10 } else { 30 },
            l.matched_offsets(p..p + w.len()),
        ));
    }
    let mut matched = Vec::new();
    let mut chars = l.text.char_indices();
    for wc in w.chars() {
        let (offset, ch) = chars.find(|(_, ch)| *ch == wc)?;
        let original = l.original_range(offset..offset + ch.len_utf8()).start;
        if matched.last() != Some(&original) {
            matched.push(original);
        }
    }
    Some((60 + (matched.last().copied().unwrap_or(0) as u32), matched))
}

#[cfg(test)]
mod paint_tests {
    use ratatui::style::{Color, Modifier};

    use super::*;

    const SENTINEL: &str = "#";
    const FAMILY: &str = "👨‍👩‍👧";
    const E_ACUTE: &str = "e\u{301}";

    fn sentinel_buffer(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        for cell in &mut buf.content {
            cell.set_symbol(SENTINEL).set_fg(Color::Red);
        }
        buf
    }

    fn row_symbols(buf: &Buffer, y: u16, x0: u16, x1: u16) -> Vec<String> {
        (x0..x1).map(|x| buf[(x, y)].symbol().to_owned()).collect()
    }

    #[test]
    fn bounded_grapheme_paint_never_writes_outside_owner() {
        let sources = [
            "plain ascii label",
            "界A界A界A",
            "e\u{301}e\u{301}e\u{301}e\u{301}",
            "👨‍👩‍👧 family 🇫🇷 flag 👍🏽",
            "a\u{200b}b\u{0}c\td",
        ];
        let buf_area = Rect::new(3, 1, 20, 5);
        for source in sources {
            for mode in [TextClip::Clip, TextClip::Ellipsis] {
                for owner_x in [0u16, 3, 6, 18, 25] {
                    for owner_w in 0..=8u16 {
                        for offset in [(0, 0), (1, 0), (0, 1), (3, 2), (9, 0), (0, 9)] {
                            let owner = Rect::new(owner_x, 2, owner_w, 2);
                            let mut buf = sentinel_buffer(buf_area);
                            let before = buf.clone();
                            let used =
                                paint_graphemes(&mut buf, owner, offset, source, mode, |_, _| {
                                    Style::new().fg(Color::Green)
                                });
                            let clip = owner.intersection(buf_area);
                            for y in buf_area.top()..buf_area.bottom() {
                                for x in buf_area.left()..buf_area.right() {
                                    if !clip.contains((x, y).into()) {
                                        assert_eq!(
                                            buf[(x, y)],
                                            before[(x, y)],
                                            "{source:?} {mode:?} owner {owner:?} offset {offset:?} wrote ({x},{y})"
                                        );
                                    }
                                }
                            }
                            assert!(
                                used <= usize::from(owner.width.saturating_sub(offset.0 as u16)),
                                "{source:?} consumed {used} cells in {owner:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn whole_clusters_advance_by_display_width_and_wide_cells_share_style() {
        let mut buf = sentinel_buffer(Rect::new(0, 0, 12, 1));
        let used = paint_graphemes(
            &mut buf,
            Rect::new(1, 0, 10, 1),
            (0, 0),
            &format!("界{E_ACUTE}{FAMILY}A"),
            TextClip::Clip,
            |_, _| Style::new().bg(Color::Blue),
        );
        assert_eq!(used, 2 + 1 + 2 + 1);
        assert_eq!(buf[(1, 0)].symbol(), "界");
        assert_eq!(buf[(3, 0)].symbol(), E_ACUTE);
        assert_eq!(buf[(4, 0)].symbol(), FAMILY);
        assert_eq!(buf[(6, 0)].symbol(), "A");
        for x in 1..=6 {
            assert_eq!(buf[(x, 0)].bg, Color::Blue, "cell {x}");
        }
        assert_eq!(buf[(7, 0)].symbol(), SENTINEL);
    }

    #[test]
    fn matched_grapheme_uses_source_byte_range() {
        let source = format!("a{E_ACUTE}b界");
        let matched = [2usize, 5];
        let mut buf = sentinel_buffer(Rect::new(0, 0, 10, 1));
        paint_graphemes(
            &mut buf,
            Rect::new(0, 0, 10, 1),
            (0, 0),
            &source,
            TextClip::Clip,
            |range, _| {
                let hit = range.is_some_and(|r| matched.iter().any(|m| r.contains(m)));
                if hit {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                }
            },
        );
        let bold = |x: u16| buf[(x, 0)].modifier.contains(Modifier::BOLD);
        assert_eq!(row_symbols(&buf, 0, 0, 5), ["a", E_ACUTE, "b", "界", " "]);
        assert!(!bold(0), "a");
        assert!(
            bold(1),
            "byte 2 lies inside the combining mark of the e+acute cluster"
        );
        assert!(!bold(2), "b");
        assert!(bold(3), "byte 5 lies inside the wide cluster");
        assert!(bold(4), "continuation cell of the wide cluster");
    }

    #[test]
    fn synthetic_ellipsis_has_no_source_match() {
        let source = "abcdefgh";
        let mut calls = Vec::new();
        let mut buf = sentinel_buffer(Rect::new(0, 0, 10, 1));
        let used = paint_graphemes(
            &mut buf,
            Rect::new(0, 0, 5, 1),
            (0, 0),
            source,
            TextClip::Ellipsis,
            |range, col| {
                calls.push((range.clone(), col));
                if range.is_none() {
                    Style::new()
                } else {
                    Style::new().add_modifier(Modifier::BOLD)
                }
            },
        );
        assert_eq!(used, 5);
        assert_eq!(
            row_symbols(&buf, 0, 0, 6),
            ["a", "b", "c", "d", "…", SENTINEL]
        );
        assert_eq!(calls.last(), Some(&(None, 4)));
        assert!(!buf[(4, 0)].modifier.contains(Modifier::BOLD));

        let mut buf = sentinel_buffer(Rect::new(0, 0, 10, 1));
        let used = paint_graphemes(
            &mut buf,
            Rect::new(0, 0, 1, 1),
            (0, 0),
            source,
            TextClip::Ellipsis,
            |range, _| {
                assert!(range.is_none(), "width 1 paints only the ellipsis");
                Style::new()
            },
        );
        assert_eq!(used, 1);
        assert_eq!(row_symbols(&buf, 0, 0, 2), ["…", SENTINEL]);
    }

    #[test]
    fn oversized_cluster_is_dropped_without_pulling_neighbours_back() {
        let mut buf = sentinel_buffer(Rect::new(0, 0, 8, 1));
        paint_graphemes(
            &mut buf,
            Rect::new(0, 0, 3, 1),
            (0, 0),
            "ab界c",
            TextClip::Clip,
            |_, _| Style::new(),
        );
        assert_eq!(row_symbols(&buf, 0, 0, 4), ["a", "b", SENTINEL, SENTINEL]);
    }

    #[test]
    fn exact_fit_source_gets_no_ellipsis() {
        let mut buf = sentinel_buffer(Rect::new(0, 0, 8, 1));
        let used = paint_graphemes(
            &mut buf,
            Rect::new(0, 0, 4, 1),
            (0, 0),
            "ab界",
            TextClip::Ellipsis,
            |_, _| Style::new(),
        );
        assert_eq!(used, 4);
        assert_eq!(row_symbols(&buf, 0, 0, 5), ["a", "b", "界", " ", SENTINEL]);
    }
}

#[cfg(test)]
mod wrap_tests;
