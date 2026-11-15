use std::ops::Range;

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
    let w = w.max(1);
    let mut lines = Vec::new();
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
        wrap_para(body, w, first, cont, &mut lines);
    }
    lines
}

fn hang_column(s: &str, gap: usize) -> Option<usize> {
    let at = s.find(&" ".repeat(gap))?;
    let rest = s[at..].trim_start_matches(' ');
    if rest.is_empty() {
        return None;
    }
    Some(width(&s[..s.len() - rest.len()]))
}

struct Para<'a> {
    lines: &'a mut Vec<String>,
    line: String,
    lw: usize,
    base: usize,
    cont: usize,
    w: usize,
}

impl Para<'_> {
    fn empty(&self) -> bool {
        self.lw == self.base
    }

    fn break_line(&mut self) {
        self.lines.push(std::mem::take(&mut self.line));
        self.line = " ".repeat(self.cont);
        self.lw = self.cont;
        self.base = self.cont;
    }

    fn push_word(&mut self, word: &str, ww: usize) {
        if self.lw + ww <= self.w {
            self.line.push_str(word);
            self.lw += ww;
        } else {
            self.hard_wrap(word);
        }
    }

    fn hard_wrap(&mut self, word: &str) {
        for g in word.graphemes(true) {
            let room = (self.w - self.base).max(1);
            let g = if width(g) > room { "…" } else { g };
            let gw = width(g);
            if self.lw + gw > self.w && !self.empty() {
                self.break_line();
            }
            self.line.push_str(g);
            self.lw += gw;
        }
    }
}

fn wrap_para(body: &str, w: usize, first: usize, cont: usize, lines: &mut Vec<String>) {
    let mut p = Para {
        lines,
        line: " ".repeat(first),
        lw: first,
        base: first,
        cont,
        w,
    };
    for word in body.split(' ') {
        let ww = width(word);
        if p.empty() {
            p.push_word(word, ww);
        } else if p.lw + 1 + ww <= w {
            p.line.push(' ');
            p.line.push_str(word);
            p.lw += 1 + ww;
        } else {
            p.break_line();
            p.push_word(word, ww);
        }
    }
    let last = std::mem::take(&mut p.line);
    p.lines.push(last);
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
