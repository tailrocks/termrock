use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use unicode_segmentation::UnicodeSegmentation;

use termrock_controls::scrollbar;
use termrock_core::edit::TextCmd;
use termrock_core::event::{Key, Outcome};
use termrock_core::field::field_common::PointerSelect;
use termrock_core::field::text_select::{self, TextSelect};
use termrock_core::hit::Axis;
use termrock_core::id::WidgetId;
use termrock_core::keymap::{Actions, KeyBinding, KeyMap, Nav, NavKeys, lookup};
use termrock_core::response::Response;
use termrock_core::scroll::{HScroll, ScrollPane, ScrollState, Scrollable, TrackClick};
use termrock_overlays::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_menu_key, edit_menu_pointer, edit_menu_route,
};
use termrock_runtime::ctx::{RenderCtx, fill};
use termrock_text::cells::width;
use termrock_theme::Tone;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,

    pub reversed: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
            bold: false,
            italic: false,
            underline: false,
            reversed: false,
        }
    }
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Normal)
    }
    pub fn muted(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Muted)
    }
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
    pub fn reversed(mut self) -> Self {
        self.reversed = true;
        self
    }
}

pub type Line = Vec<Span>;

pub fn line_text(line: &[Span]) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}

pub fn render_spans(
    buf: &mut ratatui::buffer::Buffer,
    x: u16,
    y: u16,
    max: u16,
    t: &termrock_theme::Theme,
    line: &[Span],
    bg: Color,
) -> u16 {
    let full = line_text(line);
    let shown = termrock_text::cells::truncate(&full, max as usize);
    let w = width(&shown) as u16;
    if w == 0 {
        return 0;
    }
    buf.set_string(x, y, &shown, Style::new());
    let all = Modifier::BOLD | Modifier::ITALIC | Modifier::UNDERLINED | Modifier::REVERSED;
    let mut at = x;
    for s in line {
        let sw = width(&s.text) as u16;
        let to = at.saturating_add(sw).min(x + w);
        if to > at {
            let mut m = Modifier::empty();
            if s.bold {
                m |= Modifier::BOLD;
            }
            if s.italic {
                m |= Modifier::ITALIC;
            }
            if s.underline {
                m |= Modifier::UNDERLINED;
            }
            if s.reversed {
                m |= Modifier::REVERSED;
            }
            let st = Style::new()
                .fg(t.tone_on(s.tone, bg))
                .add_modifier(m)
                .remove_modifier(all - m);
            buf.set_style(ratatui::layout::Rect::new(at, y, to - at, 1), st);
        }
        at = at.saturating_add(sw);
    }
    w
}

pub fn wrap_spans(line: &[Span], w: usize) -> Vec<Line> {
    let full: Vec<(char, usize)> = line
        .iter()
        .enumerate()
        .flat_map(|(i, s)| s.text.chars().map(move |c| (c, i)))
        .collect();
    let text: String = full.iter().map(|(c, _)| c).collect();
    let mut at = 0;
    let mut out = Vec::new();
    for part in termrock_text::cells::wrap_with(&text, w, termrock_text::cells::WrapOpts::PLAIN) {
        let mut pieces: Line = Vec::new();
        for pc in part.chars() {
            let found = full[at..].iter().position(|(c, _)| *c == pc);
            let si = match found {
                Some(k) => {
                    at += k + 1;
                    full[at - 1].1
                }

                None => full[at.saturating_sub(1).min(full.len().saturating_sub(1))].1,
            };
            let src = &line[si];
            match pieces.last_mut() {
                Some(p) if same_style(p, src) => p.text.push(pc),
                _ => pieces.push(Span {
                    text: pc.to_string(),
                    ..src.clone()
                }),
            }
        }
        out.push(pieces);
    }
    out
}

fn same_style(a: &Span, b: &Span) -> bool {
    a.tone == b.tone
        && a.bold == b.bold
        && a.italic == b.italic
        && a.underline == b.underline
        && a.reversed == b.reversed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellPos {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    pub line: usize,
    pub range: Range<usize>,
    pub current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewportAction {
    Copy(String),
    SelectionChanged,
    FollowChanged(bool),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkCounters {
    pub segmented_lines: usize,

    pub reflowed_lines: usize,

    pub index_rebuilds: usize,

    pub index_extends: usize,

    pub index_shifts: usize,

    /// Draws from a shared reference that had to deep-clone the view because
    /// it was not prepared for the draw (see `ViewportOps::draw`).
    pub draw_fallbacks: usize,

    /// Logical lines copied by those clones.
    pub draw_cloned_lines: usize,

    /// Source text bytes copied by those clones.
    pub draw_cloned_bytes: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct DrawWork {
    fallbacks: usize,
    lines: usize,
    bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellStyle {
    tone: Tone,
    bold: bool,
    italic: bool,
    underline: bool,
    reversed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellKind {
    Text,

    TabPad,

    Control,
}

#[derive(Debug, Clone)]
struct Cell {
    g: String,
    w: usize,

    src: Range<usize>,
    style: CellStyle,
    kind: CellKind,
}

type RowCache = ((u16, bool), Vec<(usize, usize)>);

#[derive(Debug)]
struct Logical {
    id: u64,
    spans: Line,

    text: String,
    cells: Vec<Cell>,

    width: usize,

    segmented: bool,

    rows: Vec<RowCache>,
}

#[cfg(test)]
thread_local! {
    static LOGICAL_CLONES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Clone for Logical {
    fn clone(&self) -> Self {
        #[cfg(test)]
        LOGICAL_CLONES.with(|n| n.set(n.get() + 1));
        Self {
            id: self.id,
            spans: self.spans.clone(),
            text: self.text.clone(),
            cells: self.cells.clone(),
            width: self.width,
            segmented: self.segmented,
            rows: self.rows.clone(),
        }
    }
}

impl Logical {
    fn new(id: u64, spans: Line) -> Self {
        let text = line_text(&spans);
        Self {
            id,
            spans,
            text,
            cells: vec![],
            width: 0,
            segmented: false,
            rows: vec![],
        }
    }

    fn set(&mut self, spans: Line) {
        self.text = line_text(&spans);
        self.spans = spans;
        self.segmented = false;
        self.rows.clear();
    }

    fn has_rows(&self, key: (u16, bool)) -> bool {
        self.rows.iter().any(|(k, _)| *k == key)
    }

    fn rows_at(&self, key: (u16, bool)) -> &[(usize, usize)] {
        self.rows
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, r)| r.as_slice())
            .unwrap_or(&[])
    }

    fn segment(&mut self) {
        let mut cells: Vec<Cell> = vec![];

        let mut bounds: Vec<(usize, CellStyle)> = vec![];
        let mut at = 0;
        for sp in &self.spans {
            bounds.push((
                at,
                CellStyle {
                    tone: sp.tone,
                    bold: sp.bold,
                    italic: sp.italic,
                    underline: sp.underline,
                    reversed: sp.reversed,
                },
            ));
            at += sp.text.len();
        }
        let style_at = |b: usize| -> CellStyle {
            bounds
                .iter()
                .rev()
                .find(|(s, _)| *s <= b)
                .map(|(_, st)| *st)
                .unwrap_or(CellStyle {
                    tone: Tone::Normal,
                    bold: false,
                    italic: false,
                    underline: false,
                    reversed: false,
                })
        };
        for (start, g) in self.text.grapheme_indices(true) {
            let src = start..start + g.len();
            let style = style_at(start);
            if g == "\t" {
                for _ in 0..termrock_text::cells::TAB_SPACES.len() {
                    cells.push(Cell {
                        g: " ".into(),
                        w: 1,
                        src: src.clone(),
                        style,
                        kind: CellKind::TabPad,
                    });
                }
                continue;
            }
            let first = g.chars().next().unwrap_or(' ');
            if g.chars().count() == 1 && (first.is_control() || first == '\u{7f}') {
                let shown = if first == '\u{7f}' {
                    "^?".to_owned()
                } else {
                    format!("^{}", ((first as u8) + 0x40) as char)
                };
                cells.push(Cell {
                    g: shown,
                    w: 2,
                    src,
                    style,
                    kind: CellKind::Control,
                });
                continue;
            }
            let w = width(g);
            if w == 0 {
                if let Some(prev) = cells.last_mut() {
                    prev.g.push_str(g);
                    prev.src.end = src.end;
                    continue;
                }
                cells.push(Cell {
                    g: g.to_owned(),
                    w: 1,
                    src,
                    style,
                    kind: CellKind::Text,
                });
                continue;
            }
            cells.push(Cell {
                g: g.to_owned(),
                w,
                src,
                style,
                kind: CellKind::Text,
            });
        }
        self.width = cells.iter().map(|c| c.w).sum();
        self.cells = cells;
        self.segmented = true;
        self.rows.clear();
    }

    fn reflow(&mut self, text_w: u16, wrap: bool) {
        let w = text_w.max(1) as usize;
        let mut rows = vec![];
        if !wrap || self.cells.is_empty() {
            rows.push((0, self.cells.len()));
        } else {
            let mut start = 0;
            let mut acc = 0;
            for (ci, c) in self.cells.iter().enumerate() {
                if acc + c.w > w && ci > start {
                    rows.push((start, ci));
                    start = ci;
                    acc = 0;
                }
                acc += c.w;
            }
            rows.push((start, self.cells.len()));
        }
        if self.rows.len() >= 2 {
            self.rows.remove(0);
        }
        self.rows.push(((text_w, wrap), rows));
    }

    fn col_of(&self, cell: usize) -> usize {
        self.cells.iter().take(cell).map(|c| c.w).sum()
    }

    fn cell_at(&self, col: usize) -> usize {
        let mut acc = 0;
        for (i, c) in self.cells.iter().enumerate() {
            if acc + c.w > col {
                return i;
            }
            acc += c.w;
        }
        self.cells.len()
    }

    fn total_width(&self) -> usize {
        self.width
    }
}

#[derive(Debug, Clone, Copy)]
struct VisualRow {
    line: usize,
    start: usize,
    end: usize,
}

/// The inputs a layout was built for, plus what it derived from them.
/// `set_area` records it after a layout is rebuilt, and a shared-reference
/// draw may skip the deep clone only while the view still matches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedKey {
    revision: u64,
    area: Rect,
    wrap: bool,
    h_scroll: HScroll,
    scrollbar_gap: u16,
    layout_width: u16,
    rows_h: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Anchor {
    id: u64,
    col: usize,
}

#[derive(Debug, Clone)]
pub struct TextViewport {
    pub id: WidgetId,

    pub scroll: ScrollPane,
    pub follow: bool,
    pub wrap: bool,

    pub caret: Option<CellPos>,
    pub caret_visible: bool,
    pub area: Rect,
    lines: Vec<Logical>,
    max_lines: Option<usize>,
    next_id: u64,
    revision: u64,

    select: TextSelect<Anchor>,

    browse: Option<Anchor>,
    marks: Vec<Mark>,
    visual: Vec<VisualRow>,
    layout_width: u16,
    layout_wrap: bool,
    index_valid: bool,

    indexed_lines: usize,

    prepared: Option<PreparedKey>,

    rows_h: u16,

    reading: Option<(u64, usize)>,
    work: WorkCounters,
    draw_work: std::cell::Cell<DrawWork>,

    pub scrollbar_gap: u16,

    pub horizontal: Option<HScroll>,

    pub nav: NavKeys,

    pub keymap: KeyMap<ViewportCmd>,

    pub pointer_select: PointerSelect,

    pub edit_menu: EditMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportCmd {
    ToggleFollow,

    Copy,

    SelectAll,
}

impl Actions for ViewportCmd {
    fn label(self) -> &'static str {
        match self {
            ViewportCmd::ToggleFollow => "Follow",
            ViewportCmd::Copy => "Copy",
            ViewportCmd::SelectAll => "Select all",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        let mut map = vec![
            (KeyBinding::char('f'), ViewportCmd::ToggleFollow),
            (KeyBinding::cmd('c'), ViewportCmd::Copy),
            (KeyBinding::char('y'), ViewportCmd::Copy),
        ];
        map.extend(termrock_core::keymap::select_all_bindings(
            ViewportCmd::SelectAll,
        ));
        map
    }
}

impl TextViewport {
    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<ViewportCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            scroll: ScrollPane::default(),
            follow: true,
            wrap: false,
            caret: None,
            caret_visible: true,
            area: Rect::ZERO,
            lines: vec![],
            max_lines: None,
            next_id: 1,
            revision: 0,
            select: TextSelect::default(),
            pointer_select: PointerSelect::DEFAULT,
            edit_menu: EditMenu::default(),
            browse: None,
            marks: vec![],
            visual: vec![],
            layout_width: 0,
            layout_wrap: false,
            index_valid: false,
            indexed_lines: 0,
            prepared: None,
            reading: None,
            work: WorkCounters::default(),
            draw_work: std::cell::Cell::default(),
            scrollbar_gap: scrollbar::GAP,
            rows_h: 0,
            horizontal: None,
            nav: NavKeys::default(),
            keymap: ViewportCmd::default_keymap(),
        }
    }

    pub fn horizontal(mut self, policy: HScroll) -> Self {
        self.horizontal = Some(policy);
        self
    }

    pub fn pointer_select(mut self, policy: PointerSelect) -> Self {
        self.pointer_select = policy;
        self
    }

    pub fn track_click(mut self, click: TrackClick) -> Self {
        self.scroll.track_click = Some(click);
        self
    }

    pub fn h_enabled(&self) -> bool {
        self.projection().h_enabled(&self.scroll)
    }

    fn hoff(&self) -> usize {
        self.projection().hoff(&self.scroll)
    }

    fn text_w(&self) -> usize {
        self.layout_width as usize
    }

    fn text_rect(&self) -> Rect {
        self.projection().text_rect()
    }

    pub fn reveal_cols(&mut self, cols: Range<usize>) -> bool {
        if !self.h_enabled() {
            return false;
        }
        let before = self.scroll.h.offset;
        if cols.end > cols.start {
            self.scroll.h.ensure_visible(cols.end - 1);
        }
        self.scroll.h.ensure_visible(cols.start);
        self.scroll.h.offset != before
    }

    fn cols_of_bytes(&self, line: usize, range: &Range<usize>) -> Option<Range<usize>> {
        let l = self.lines.get(line)?;
        let mut col = 0;
        let mut start = None;
        let mut end = 0;
        for c in &l.cells {
            if c.src.start < range.end && range.start < c.src.end {
                start.get_or_insert(col);
                end = col + c.w;
            }
            col += c.w;
        }
        start.map(|s| s..end)
    }

    pub fn reveal_mark(&mut self, line: usize, range: Range<usize>) {
        self.reveal_line(line);
        if let Some(cols) = self.cols_of_bytes(line, &range) {
            self.reveal_cols(cols);
        }
    }

    fn scroll_h(&mut self, delta: isize) -> Outcome {
        self.wheel(Axis::Horizontal, delta as i32).outcome()
    }

    pub fn scrollbar_gap(mut self, gap: u16) -> Self {
        self.scrollbar_gap = gap;
        self
    }

    pub fn with_lines(id: WidgetId, lines: Vec<Line>) -> Self {
        let mut v = Self::new(id);
        v.set_lines(lines);
        v
    }

    pub fn wrap(mut self, w: bool) -> Self {
        self.wrap = w;
        self
    }

    pub fn max_lines(mut self, n: usize) -> Self {
        self.set_max_lines(Some(n));
        self
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn work_counters(&self) -> WorkCounters {
        let draw = self.draw_work.get();
        WorkCounters {
            draw_fallbacks: draw.fallbacks,
            draw_cloned_lines: draw.lines,
            draw_cloned_bytes: draw.bytes,
            ..self.work
        }
    }

    pub(crate) fn clone_for_draw(&self) -> Self {
        let mut work = self.draw_work.get();
        work.fallbacks += 1;
        work.lines += self.lines.len();
        let bytes = self.lines.iter().map(|l| l.text.len()).sum::<usize>();
        work.bytes += bytes;
        termrock_metrics::record_work!(PayloadBytesCloned, bytes);
        self.draw_work.set(work);
        self.clone()
    }

    pub fn retention(&self) -> Option<usize> {
        self.max_lines
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn lines(&self) -> impl Iterator<Item = &Line> {
        self.lines.iter().map(|l| &l.spans)
    }

    pub fn line(&self, i: usize) -> Option<&Line> {
        self.lines.get(i).map(|l| &l.spans)
    }

    pub fn line_text(&self, i: usize) -> Option<&str> {
        self.lines.get(i).map(|l| l.text.as_str())
    }

    pub fn last(&self) -> Option<&Line> {
        self.lines.last().map(|l| &l.spans)
    }

    fn alloc(&mut self, spans: Line) -> Logical {
        let id = self.next_id;
        self.next_id += 1;
        Logical::new(id, spans)
    }

    fn touched(&mut self) {
        self.revision += 1;
        self.index_valid = false;
    }

    fn appended(&mut self) {
        self.revision += 1;
    }

    fn shift_index(&mut self, drop: usize) {
        if !self.index_valid {
            return;
        }
        let gone = self.visual.iter().take_while(|v| v.line < drop).count();
        self.visual.drain(..gone);
        for v in &mut self.visual {
            v.line -= drop;
        }
        self.indexed_lines = self.indexed_lines.saturating_sub(drop);
        self.work.index_shifts += 1;
        termrock_metrics::record_work!(IndexShifts, 1usize);
        self.scroll.set_content(self.visual.len());
        if !self.follow {
            self.scroll.offset = self.scroll.offset.saturating_sub(gone);
        }
    }

    fn enforce_cap(&mut self) -> usize {
        let Some(max) = self.max_lines else {
            return 0;
        };
        if self.lines.len() <= max {
            return 0;
        }
        let drop = self.lines.len() - max;
        let evicted: Vec<u64> = self.lines.drain(..drop).map(|l| l.id).collect();
        self.reconcile(drop, &evicted);
        self.shift_index(drop);
        drop
    }

    fn reconcile(&mut self, drop: usize, evicted: &[u64]) {
        let gone = |id: u64| evicted.contains(&id);

        if let Some((a, b)) = self.select.sel {
            let (lo, hi) = if (a.id, a.col) <= (b.id, b.col) {
                (a, b)
            } else {
                (b, a)
            };
            if gone(hi.id) {
                self.select.sel = None;
            } else if gone(lo.id) {
                let first = self.lines.first().map(|l| l.id).unwrap_or(hi.id);
                let clipped = Anchor { id: first, col: 0 };
                self.select.sel = Some(if a.id == lo.id {
                    (clipped, b)
                } else {
                    (a, clipped)
                });
                if self.select.sel.is_some_and(|(x, y)| x == y) {
                    self.select.sel = None;
                }
            }
        }
        if self
            .select
            .word
            .is_some_and(|(a, b)| gone(a.id) || gone(b.id))
        {
            self.select.word = None;
        }
        if self.select.press.is_some_and(|d| gone(d.id)) {
            self.select.press = self.lines.first().map(|l| Anchor { id: l.id, col: 0 });
        }
        if self.browse.is_some_and(|b| gone(b.id)) {
            self.browse = None;
        }
        if let Some(c) = self.caret {
            if c.line < drop {
                self.caret = None;
            } else {
                self.caret = Some(CellPos {
                    line: c.line - drop,
                    col: c.col,
                });
            }
        }
        if self.reading.is_some_and(|(id, _)| gone(id)) {
            self.reading = self.lines.first().map(|l| (l.id, 0));
        }
        self.marks.retain(|m| m.line >= drop);
        for m in &mut self.marks {
            m.line -= drop;
        }
    }

    pub fn set_lines(&mut self, lines: Vec<Line>) {
        let mut fresh = Vec::with_capacity(lines.len());
        for l in lines {
            let lg = self.alloc(l);
            fresh.push(lg);
        }
        self.lines = fresh;
        self.select.sel = None;
        self.select.press = None;
        self.select.word = None;
        self.browse = None;
        self.caret = None;
        self.reading = None;
        self.marks.clear();
        self.touched();
        self.enforce_cap();
    }

    pub fn push(&mut self, line: Line) {
        let lg = self.alloc(line);
        self.lines.push(lg);
        self.appended();
        self.enforce_cap();
    }

    pub fn extend(&mut self, lines: Vec<Line>) {
        if lines.is_empty() {
            return;
        }
        for l in lines {
            let lg = self.alloc(l);
            self.lines.push(lg);
        }
        self.appended();
        self.enforce_cap();
    }

    pub fn replace_last(&mut self, line: Line) {
        match self.lines.last_mut() {
            Some(last) => {
                if last.spans == line {
                    return;
                }
                last.set(line);
                self.touched();
            }
            None => self.push(line),
        }
    }

    pub fn pop(&mut self) -> Option<Line> {
        let l = self.lines.pop()?;
        let evicted = [l.id];
        self.reconcile_removed_tail(&evicted, self.lines.len());
        self.touched();
        Some(l.spans)
    }

    fn reconcile_removed_tail(&mut self, evicted: &[u64], new_len: usize) {
        let gone = |id: u64| evicted.contains(&id);
        if self
            .select
            .sel
            .is_some_and(|(a, b)| gone(a.id) || gone(b.id))
        {
            self.select.sel = None;
        }
        if self
            .select
            .word
            .is_some_and(|(a, b)| gone(a.id) || gone(b.id))
        {
            self.select.word = None;
        }
        if self.select.press.is_some_and(|d| gone(d.id)) {
            self.select.press = None;
        }
        if self.browse.is_some_and(|b| gone(b.id)) {
            self.browse = None;
        }
        if self.caret.is_some_and(|c| c.line >= new_len) {
            self.caret = None;
        }
        if self.reading.is_some_and(|(id, _)| gone(id)) {
            self.reading = self.lines.last().map(|l| (l.id, 0));
        }
        self.marks.retain(|m| m.line < new_len);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.select.sel = None;
        self.select.press = None;
        self.select.word = None;
        self.browse = None;
        self.caret = None;
        self.reading = None;
        self.marks.clear();
        self.touched();
    }

    pub fn set_max_lines(&mut self, n: Option<usize>) {
        self.max_lines = n;
        if self.enforce_cap() > 0 {
            self.touched();
        }
    }

    pub fn set_marks(&mut self, marks: Vec<Mark>) {
        self.marks = marks;
    }

    pub fn clear_marks(&mut self) {
        self.marks.clear();
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn reveal_line(&mut self, line: usize) {
        self.ensure_index();
        if let Some(vi) = self.visual.iter().position(|v| v.line == line) {
            self.follow = false;
            self.scroll.ensure_visible(vi);
            self.remember_reading();
        }
        let current = self
            .marks
            .iter()
            .find(|m| m.line == line && m.current)
            .map(|m| m.range.clone());
        if let Some(cols) = current.and_then(|r| self.cols_of_bytes(line, &r)) {
            self.reveal_cols(cols);
        }
    }

    fn ensure_layout(&mut self, text_w: u16) {
        let wrap = self.wrap;
        let same_key = self.layout_width == text_w && self.layout_wrap == wrap;
        if self.index_valid && same_key {
            let from = self.indexed_lines.min(self.lines.len());
            let key = (text_w, wrap);
            let mut extended = false;
            for li in from..self.lines.len() {
                let l = &mut self.lines[li];
                if !l.segmented {
                    l.segment();
                    self.work.segmented_lines += 1;
                    termrock_metrics::record_work!(SegmentedLines, 1usize);
                }
                if !l.has_rows(key) {
                    l.reflow(text_w, wrap);
                    self.work.reflowed_lines += 1;
                    termrock_metrics::record_work!(ReflowedLines, 1usize);
                }
                for &(s, e) in self.lines[li].rows_at(key) {
                    self.visual.push(VisualRow {
                        line: li,
                        start: s,
                        end: e,
                    });
                }
                extended = true;
            }
            self.indexed_lines = self.lines.len();
            if extended {
                self.work.index_extends += 1;
                termrock_metrics::record_work!(IndexExtends, 1usize);
                self.scroll.set_content(self.visual.len());
                self.clip_selection();
            }
            return;
        }
        for l in &mut self.lines {
            if !l.segmented {
                l.segment();
                self.work.segmented_lines += 1;
                termrock_metrics::record_work!(SegmentedLines, 1usize);
            }
            if !l.has_rows((text_w, wrap)) {
                l.reflow(text_w, wrap);
                self.work.reflowed_lines += 1;
                termrock_metrics::record_work!(ReflowedLines, 1usize);
            }
        }
        self.layout_width = text_w;
        self.layout_wrap = wrap;
        self.rebuild_index();
    }

    fn rebuild_index(&mut self) {
        let key = (self.layout_width, self.layout_wrap);
        let mut visual = Vec::with_capacity(self.lines.len());
        for (li, l) in self.lines.iter().enumerate() {
            for &(s, e) in l.rows_at(key) {
                visual.push(VisualRow {
                    line: li,
                    start: s,
                    end: e,
                });
            }
        }
        self.visual = visual;
        self.index_valid = true;
        self.indexed_lines = self.lines.len();
        self.work.index_rebuilds += 1;
        termrock_metrics::record_work!(IndexRebuilds, 1usize);
        self.scroll.set_content(self.visual.len());
        self.clip_selection();
    }

    fn clip_selection(&mut self) {
        if let Some((a, b)) = self.select.sel {
            let clip = |x: Anchor, lines: &[Logical]| -> Anchor {
                match lines.iter().find(|l| l.id == x.id) {
                    Some(l) => Anchor {
                        id: x.id,
                        col: x.col.min(l.total_width()),
                    },
                    None => x,
                }
            };
            let na = clip(a, &self.lines);
            let nb = clip(b, &self.lines);
            self.select.sel = if na == nb { None } else { Some((na, nb)) };
        }
        if !self.follow {
            self.restore_reading();
        }
    }

    fn ensure_index(&mut self) {
        if !self.index_valid || self.indexed_lines < self.lines.len() {
            let w = if self.layout_width == 0 {
                self.area.width.max(1)
            } else {
                self.layout_width
            };
            self.ensure_layout(w);
        }
    }

    fn remember_reading(&mut self) {
        if self.follow || !self.index_valid {
            self.reading = None;
            return;
        }
        let Some(vr) = self.visual.get(self.scroll.offset) else {
            self.reading = None;
            return;
        };
        let Some(line) = self.lines.get(vr.line) else {
            self.reading = None;
            return;
        };
        let key = (self.layout_width, self.layout_wrap);
        let sub = line
            .rows_at(key)
            .iter()
            .position(|&(s, _)| s == vr.start)
            .unwrap_or(0);
        self.reading = Some((line.id, sub));
    }

    fn restore_reading(&mut self) {
        let Some((id, sub)) = self.reading else {
            return;
        };
        let Some(li) = self.lines.iter().position(|l| l.id == id) else {
            self.scroll.jump_start();
            return;
        };
        let key = (self.layout_width, self.layout_wrap);
        let rows = self.lines[li].rows_at(key);
        let sub = sub.min(rows.len().saturating_sub(1));
        let start = rows.get(sub).map(|r| r.0).unwrap_or(0);
        let target = self
            .visual
            .iter()
            .position(|v| v.line == li && v.start == start)
            .unwrap_or(0);
        self.scroll.scroll_to(target);
    }

    /// Prepares the layout for `area`. A view prepared for exactly this
    /// area, content revision, wrap, scrollbar gap and horizontal policy can
    /// be drawn from a shared reference without copying its history.
    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
        self.adopt_policy();
        let h_on = self.h_enabled();
        if !self.is_prepared(area) {
            let mut rows = area.height;
            self.lay_out(area.width, rows);
            if h_on {
                self.sync_h();
                if area.height > 1 && self.scroll.h_cfg.shows_bar(&self.scroll.h) {
                    rows = area.height - 1;
                    self.lay_out(area.width, rows);
                    self.sync_h();
                }
            } else {
                self.scroll.h = ScrollState::default();
            }
            self.rows_h = rows;
            self.prepared = Some(self.prepared_key(area));

            if rows < area.height && h_on {
                let w = area
                    .width
                    .saturating_sub(u16::from(self.scroll.overflows()));
                let track = Rect::new(area.x, area.y + rows, w, 1);
                self.scroll.set_track(self.id, Axis::Horizontal, track);
            } else {
                self.scroll.clear_track(Axis::Horizontal);
            }
        } else {
            self.scroll.set_viewport(self.rows_h as usize);
        }
        if self.follow {
            self.scroll.jump_end();
        }
    }

    fn prepared_key(&self, area: Rect) -> PreparedKey {
        PreparedKey {
            revision: self.revision,
            area,
            wrap: self.wrap,
            h_scroll: self.h_policy(),
            scrollbar_gap: self.scrollbar_gap,
            layout_width: self.layout_width,
            rows_h: self.rows_h,
        }
    }

    fn is_prepared(&self, area: Rect) -> bool {
        self.index_valid
            && self.indexed_lines == self.lines.len()
            && self.scroll.h_cfg == self.h_policy()
            && self.prepared == Some(self.prepared_key(area))
    }

    fn sync_h(&mut self) {
        let widest = self.lines.iter().map(|l| l.width).max().unwrap_or(0);
        self.scroll.h.set_content(widest);
        self.scroll.h.set_viewport(self.text_w());
    }

    fn lay_out(&mut self, width: u16, rows: u16) {
        let narrow = width.saturating_sub(scrollbar::reserve(self.scrollbar_gap));
        let preferred = if self.layout_width == narrow && self.wrap == self.layout_wrap {
            narrow
        } else {
            width
        };
        self.ensure_layout(preferred);
        self.scroll.set_viewport(rows as usize);
        if self.scroll.overflows() && preferred == width {
            self.ensure_layout(narrow);
        } else if !self.scroll.overflows() && preferred == narrow {
            self.ensure_layout(width);
            self.scroll.set_viewport(rows as usize);
            if self.scroll.overflows() {
                self.ensure_layout(narrow);
            }
        }
    }

    fn resolve(&self, a: Anchor) -> Option<CellPos> {
        resolve_in(&self.lines, a)
    }

    fn anchor_at(&self, p: CellPos) -> Option<Anchor> {
        let l = self.lines.get(p.line)?;
        Some(Anchor {
            id: l.id,
            col: p.col,
        })
    }

    pub fn has_anchor(&self) -> bool {
        self.select.press.is_some()
    }

    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    pub fn selection(&self) -> Option<(CellPos, CellPos)> {
        self.projection().selection()
    }

    pub fn select_all(&mut self) -> Outcome {
        self.ensure_index();
        let (Some(first), Some(last)) = (self.lines.first(), self.lines.last()) else {
            return Outcome::Consumed;
        };
        let all = (
            Anchor {
                id: first.id,
                col: 0,
            },
            Anchor {
                id: last.id,
                col: last.col_of(last.cells.len()),
            },
        );
        if self.select.sel == Some(all) {
            return Outcome::Consumed;
        }
        self.select.set(all.0, all.1);
        self.browse = Some(all.1);
        Outcome::Changed
    }

    pub fn clear_selection(&mut self) -> Outcome {
        self.browse = None;
        self.select.clear()
    }

    pub fn is_at_tail(&self) -> bool {
        self.scroll.offset >= self.scroll.max_offset()
    }

    pub fn scrollback_depth(&self) -> usize {
        self.scroll.max_offset().saturating_sub(self.scroll.offset)
    }

    pub fn set_follow(&mut self, on: bool) {
        self.follow = on;
        if on {
            self.scroll.jump_end();
            self.reading = None;
        } else {
            self.remember_reading();
        }
    }

    fn col_of(&self, line: usize, cell: usize) -> usize {
        self.lines.get(line).map(|l| l.col_of(cell)).unwrap_or(0)
    }

    fn cell_at(&self, line: usize, col: usize) -> usize {
        self.lines.get(line).map(|l| l.cell_at(col)).unwrap_or(0)
    }

    pub fn pos_at(&self, pos: Position) -> Option<CellPos> {
        if self.area.is_empty() || self.visual.is_empty() {
            return None;
        }
        let text = self.text_rect();
        let y = pos.y.min(text.bottom().saturating_sub(1));
        let row = (y.saturating_sub(self.area.y) as usize + self.scroll.offset)
            .min(self.visual.len().saturating_sub(1));
        let vr = self.visual.get(row)?;
        let x = pos.x.saturating_sub(self.area.x) as usize + self.hoff();
        let base = self.col_of(vr.line, vr.start);
        let line_w = self.col_of(vr.line, vr.end);
        Some(CellPos {
            line: vr.line,
            col: (base + x).min(line_w),
        })
    }

    fn source_slice(&self, line: usize, from: usize, to: usize) -> String {
        let Some(l) = self.lines.get(line) else {
            return String::new();
        };
        let from = from.min(l.cells.len());
        let to = to.min(l.cells.len());
        if from >= to {
            return String::new();
        }
        let start = l.cells[from].src.start;
        let end = l.cells[to - 1].src.end;
        l.text[start..end].to_owned()
    }

    pub fn selected_text(&self) -> Option<String> {
        let (a, b) = self.selection()?;
        let mut out = String::new();
        for li in a.line..=b.line {
            let Some(l) = self.lines.get(li) else { break };
            let from = if li == a.line {
                self.cell_at(li, a.col)
            } else {
                0
            };
            let to = if li == b.line {
                self.cell_at(li, b.col)
            } else {
                l.cells.len()
            };
            let text = self.source_slice(li, from, to);
            out.push_str(text.trim_end_matches(' '));
            if li != b.line {
                out.push('\n');
            }
        }
        Some(out)
    }

    fn is_word(c: &Cell) -> bool {
        c.kind == CellKind::Text && text_select::is_word(&c.g)
    }

    fn word_bounds(&self, line: usize, ci: usize) -> Option<(usize, usize)> {
        text_select::word_bounds(&self.lines.get(line)?.cells, ci, Self::is_word)
    }

    fn word_at(&self, p: CellPos) -> Option<(Anchor, Anchor)> {
        let ci = self.cell_at(p.line, p.col);
        let (s, e) = self.word_bounds(p.line, ci)?;
        let a = CellPos {
            line: p.line,
            col: self.col_of(p.line, s),
        };
        let b = CellPos {
            line: p.line,
            col: self.col_of(p.line, e),
        };
        self.anchor_at(a).zip(self.anchor_at(b))
    }

    fn line_at(&self, p: CellPos) -> Option<(Anchor, Anchor)> {
        let l = self.lines.get(p.line)?;
        (!l.cells.is_empty()).then_some((
            Anchor { id: l.id, col: 0 },
            Anchor {
                id: l.id,
                col: l.total_width(),
            },
        ))
    }

    fn follow_head(&mut self) {
        if let Some((_, head)) = self.select.sel {
            self.browse = Some(head);
        }
    }

    pub fn select_word_at(&mut self, pos: Position) -> Outcome {
        let Some(p) = self.pos_at(pos) else {
            return Outcome::Ignored;
        };
        let Some((a, b)) = self.word_at(p) else {
            return if self.lines.get(p.line).is_some_and(|l| l.cells.is_empty()) {
                Outcome::Ignored
            } else {
                Outcome::Consumed
            };
        };
        self.select.set(a, b);
        self.browse = Some(b);
        Outcome::Changed
    }

    pub fn on_click(&mut self, pos: Position) -> Outcome {
        if self.select.dragged.take() == Some(pos) && self.select.sel.is_some() {
            return Outcome::Consumed;
        }
        self.on_press_with(pos, 1, ratatui::crossterm::event::KeyModifiers::NONE)
    }

    pub fn on_press_with(
        &mut self,
        pos: Position,
        count: u8,
        mods: ratatui::crossterm::event::KeyModifiers,
    ) -> Outcome {
        let p = self.pos_at(pos);
        let at = p.and_then(|p| self.anchor_at(p));
        let word = p.and_then(|p| self.word_at(p));
        let line = p.and_then(|p| self.line_at(p));
        self.browse = None;
        let o = self
            .select
            .press(self.pointer_select, at, count, mods, word, line);
        self.follow_head();
        o
    }

    pub fn on_click_with(&mut self, pos: Position, count: u8) -> Outcome {
        let p = self.pos_at(pos);
        let word = p.and_then(|p| self.word_at(p));
        let line = p.and_then(|p| self.line_at(p));
        let o = self
            .select
            .click(self.pointer_select, pos, count, word, line);
        self.follow_head();
        o
    }

    pub fn on_drag(&mut self, pos: Position) -> Outcome {
        if self.select.press.is_none() {
            return Outcome::Ignored;
        }
        let text = self.text_rect();
        if text.is_empty() {
            self.select.press = None;
            return Outcome::Ignored;
        }

        if pos.y < text.y {
            self.scroll.scroll_by(-((text.y - pos.y) as isize));
            self.follow = false;
            self.remember_reading();
        } else if pos.y >= text.bottom() {
            self.scroll.scroll_by((pos.y + 1 - text.bottom()) as isize);
            self.remember_reading();
        }

        let text_right = if self.h_enabled() {
            self.area.x + (self.text_w() as u16).min(self.area.width)
        } else {
            self.area.right()
        };
        if self.h_enabled() {
            if pos.x < self.area.x {
                self.scroll_h(-1);
            } else if pos.x >= text_right {
                self.scroll_h(1);
            }
        }
        let clamped = Position::new(
            pos.x.clamp(self.area.x, text_right.saturating_sub(1)),
            pos.y.clamp(text.y, text.bottom().saturating_sub(1)),
        );
        let p = self.pos_at(clamped);
        let head = p.and_then(|p| self.anchor_at(p));
        let word = if self.select.by_word() {
            p.and_then(|p| self.word_at(p))
        } else {
            None
        };
        let o = self.select.drag(self.pointer_select, head, pos, word);
        self.follow_head();
        o
    }

    fn browse_caret(&self) -> Option<CellPos> {
        if let Some(b) = self.browse.and_then(|b| self.resolve(b)) {
            return Some(b);
        }
        if self.lines.is_empty() {
            return None;
        }
        let last_visible = self
            .visual
            .get(self.scroll.visible_range().end.saturating_sub(1))
            .map(|v| v.line)
            .unwrap_or(self.lines.len() - 1);
        Some(CellPos {
            line: last_visible,
            col: self.lines[last_visible].total_width(),
        })
    }

    fn move_caret(&self, from: CellPos, code: KeyCode, word: bool) -> CellPos {
        let n = self.lines.len();
        let line_w = |li: usize| self.lines.get(li).map(|l| l.total_width()).unwrap_or(0);
        match code {
            KeyCode::Left => {
                let ci = self.cell_at(from.line, from.col);
                if ci == 0 {
                    if from.line > 0 {
                        return CellPos {
                            line: from.line - 1,
                            col: line_w(from.line - 1),
                        };
                    }
                    return from;
                }
                let target = if word {
                    let cs = &self.lines[from.line].cells;
                    let mut i = ci;
                    while i > 0 && !Self::is_word(&cs[i - 1]) {
                        i -= 1;
                    }
                    while i > 0 && Self::is_word(&cs[i - 1]) {
                        i -= 1;
                    }
                    i
                } else {
                    ci - 1
                };
                CellPos {
                    line: from.line,
                    col: self.col_of(from.line, target),
                }
            }
            KeyCode::Right => {
                let cs_len = self
                    .lines
                    .get(from.line)
                    .map(|l| l.cells.len())
                    .unwrap_or(0);
                let ci = self.cell_at(from.line, from.col);
                if ci >= cs_len {
                    if from.line + 1 < n {
                        return CellPos {
                            line: from.line + 1,
                            col: 0,
                        };
                    }
                    return from;
                }
                let target = if word {
                    let cs = &self.lines[from.line].cells;
                    let mut i = ci;
                    while i < cs.len() && !Self::is_word(&cs[i]) {
                        i += 1;
                    }
                    while i < cs.len() && Self::is_word(&cs[i]) {
                        i += 1;
                    }
                    i
                } else {
                    ci + 1
                };
                CellPos {
                    line: from.line,
                    col: self.col_of(from.line, target),
                }
            }
            KeyCode::Up => {
                if from.line == 0 {
                    return CellPos { line: 0, col: 0 };
                }
                CellPos {
                    line: from.line - 1,
                    col: from.col.min(line_w(from.line - 1)),
                }
            }
            KeyCode::Down => {
                if from.line + 1 >= n {
                    return CellPos {
                        line: from.line,
                        col: line_w(from.line),
                    };
                }
                CellPos {
                    line: from.line + 1,
                    col: from.col.min(line_w(from.line + 1)),
                }
            }
            KeyCode::Home => {
                if word {
                    CellPos { line: 0, col: 0 }
                } else {
                    CellPos {
                        line: from.line,
                        col: 0,
                    }
                }
            }
            KeyCode::End => {
                if word {
                    CellPos {
                        line: n.saturating_sub(1),
                        col: line_w(n.saturating_sub(1)),
                    }
                } else {
                    CellPos {
                        line: from.line,
                        col: line_w(from.line),
                    }
                }
            }
            _ => from,
        }
    }

    fn extend_selection(&mut self, code: KeyCode, word: bool) -> Outcome {
        self.ensure_index();
        let Some(from) = self.browse_caret() else {
            return Outcome::Consumed;
        };
        let to = self.move_caret(from, code, word);
        let anchor = match self.select.sel {
            Some((a, _)) => a,
            None => match self.anchor_at(from) {
                Some(a) => a,
                None => return Outcome::Consumed,
            },
        };
        let Some(head) = self.anchor_at(to) else {
            return Outcome::Consumed;
        };
        self.select.sel = Some((anchor, head));
        self.browse = Some(head);
        self.select.press = None;

        if let Some(vi) = self.visual.iter().position(|v| {
            v.line == to.line
                && self.col_of(v.line, v.start) <= to.col
                && (to.col < self.col_of(v.line, v.end) || v.end == self.lines[v.line].cells.len())
        }) {
            self.follow = false;
            self.scroll.ensure_visible(vi);
            self.remember_reading();
        }

        self.reveal_cols(to.col..to.col + 1);
        Outcome::Changed
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<ViewportAction>) {
        if let Some(o) = edit_menu_key(self, key) {
            return (o, None);
        }
        if key.shift()
            && matches!(
                key.code,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
            )
        {
            let word = key.ctrl() || key.alt();
            let o = self.extend_selection(key.code, word);
            return (o, Some(ViewportAction::SelectionChanged));
        }

        let cmd = lookup(&self.keymap, key)
            .or_else(|| (key.ctrl_char('c') && self.has_selection()).then_some(ViewportCmd::Copy));
        match cmd {
            Some(ViewportCmd::ToggleFollow) => {
                self.set_follow(!self.follow);
                return (
                    Outcome::Changed,
                    Some(ViewportAction::FollowChanged(self.follow)),
                );
            }
            Some(ViewportCmd::SelectAll) => {
                return match self.select_all() {
                    Outcome::Changed => (Outcome::Changed, Some(ViewportAction::SelectionChanged)),
                    o => (o, None),
                };
            }
            Some(ViewportCmd::Copy) => {
                return match self.selected_text() {
                    Some(t) => (Outcome::Changed, Some(ViewportAction::Copy(t))),
                    None => (Outcome::Consumed, None),
                };
            }
            None => {}
        }
        let nav = self.nav.action(key).filter(|_| key.plain());
        let moved = match (nav, key.code) {
            (Some(Nav::Left), _) if self.caret.is_none() => {
                return (self.scroll_h(-1), None);
            }
            (Some(Nav::Right), _) if self.caret.is_none() => {
                return (self.scroll_h(1), None);
            }
            (Some(Nav::Up), _) => self.scroll.scroll_by(-1),
            (Some(Nav::Down), _) => self.scroll.scroll_by(1),
            (Some(Nav::PageUp), _) => self.scroll.page_up(),
            (Some(Nav::PageDown), _) => self.scroll.page_down(),
            (Some(Nav::Home), _) => self.scroll.jump_start(),
            (Some(Nav::End), _) => {
                self.scroll.jump_end();
                self.follow = true;
                self.reading = None;
                return (Outcome::Changed, Some(ViewportAction::FollowChanged(true)));
            }
            (_, KeyCode::Esc) if key.plain() => {
                return match self.clear_selection() {
                    Outcome::Changed => (Outcome::Changed, Some(ViewportAction::SelectionChanged)),
                    _ => (Outcome::Ignored, None),
                };
            }
            _ => return (Outcome::Ignored, None),
        };
        if !moved {
            return (Outcome::Consumed, None);
        }
        let was = self.follow;
        self.follow = self.is_at_tail();
        self.remember_reading();
        let ev = (was != self.follow).then_some(ViewportAction::FollowChanged(self.follow));
        (Outcome::Changed, ev)
    }

    /// Prepares the layout for `area`, then paints through the one borrowed
    /// painter and keeps the scroll and menu geometry it produced.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            self.edit_menu.render(self.area, buf, ctx);
            return;
        }
        self.scroll.note_policy(ctx.track_click, ctx.hscroll);
        self.adopt_policy();
        self.scroll.begin(ctx.track_click);
        self.set_area(area);
        let mut state = PaintState {
            scroll: std::mem::take(&mut self.scroll),
            edit_menu: std::mem::take(&mut self.edit_menu),
            caret_visible: self.caret_visible,
        };
        paint_projection(&self.projection(), &mut state, buf, ctx, bg);
        self.scroll = state.scroll;
        self.edit_menu = state.edit_menu;
    }

    /// Paints a view the host already prepared with `set_area` without
    /// copying its history. The scroll and menu geometry of the frame is
    /// local and dropped, as a deep clone's was. `None` means the view is not
    /// prepared for `area` and the caller must clone and prepare instead.
    pub(crate) fn draw_prepared(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        caret_allowed: bool,
    ) -> Option<PaintResult> {
        if area.is_empty() || !self.is_prepared(area) {
            return None;
        }
        let mut state = PaintState {
            scroll: self.scroll.clone(),
            edit_menu: self.edit_menu.clone(),
            caret_visible: self.caret_visible && caret_allowed,
        };
        state.scroll.begin(ctx.track_click);
        state.scroll.set_viewport(self.rows_h as usize);
        if self.follow {
            state.scroll.jump_end();
        }
        Some(paint_projection(
            &self.projection(),
            &mut state,
            buf,
            ctx,
            bg,
        ))
    }

    fn projection(&self) -> ViewportProjection<'_> {
        ViewportProjection {
            id: self.id,
            area: self.area,
            lines: &self.lines,
            visual: &self.visual,
            marks: &self.marks,
            select: self.select.sel,
            browse: self.browse,
            horizontal: self.horizontal,
            wrap: self.wrap,
            follow: self.follow,
            caret: self.caret,
            layout_width: self.layout_width,
            rows_h: self.rows_h,
        }
    }

    fn h_policy(&self) -> HScroll {
        effective_hscroll(self.horizontal, &self.scroll)
    }

    fn adopt_policy(&mut self) {
        let h_cfg = self.h_policy();
        if h_cfg != self.scroll.h_cfg {
            self.scroll.h_cfg = h_cfg;
            self.prepared = None;
        }
    }

    pub fn screen_pos(&self, c: CellPos) -> Option<Position> {
        self.projection().screen_pos(&self.scroll, c)
    }

    pub fn context_point(&self) -> Position {
        if let Some((_, end)) = self.selection()
            && let Some(p) = self.screen_pos(end)
        {
            return p;
        }
        if self.caret_visible
            && let Some(c) = self.caret
            && let Some(p) = self.screen_pos(c)
        {
            return p;
        }
        let top = Position::new(self.area.x, self.area.y);
        if self.follow || self.is_at_tail() {
            let drawn = self.scroll.visible_range().len() as u16;
            if drawn > 0 {
                return Position::new(self.area.x, self.area.y + drawn.min(self.area.height) - 1);
            }
        }
        top
    }

    pub fn hbar(&self) -> Rect {
        self.scroll.h_track()
    }

    pub(crate) fn vbar(&self) -> Rect {
        if self.scroll.v.overflows() && self.area.width >= 2 {
            Rect::new(
                self.area.right() - 1,
                self.area.y,
                1,
                self.text_rect().height,
            )
        } else {
            Rect::ZERO
        }
    }
}

/// Borrowed view of everything a paint reads from a `TextViewport`, so the
/// painter can run from a shared reference while the scroll pane and edit
/// menu it mutates live in a separate `PaintState`.
struct ViewportProjection<'a> {
    id: WidgetId,
    area: Rect,
    lines: &'a [Logical],
    visual: &'a [VisualRow],
    marks: &'a [Mark],
    select: Option<(Anchor, Anchor)>,
    browse: Option<Anchor>,
    horizontal: Option<HScroll>,
    wrap: bool,
    follow: bool,
    caret: Option<CellPos>,
    layout_width: u16,
    rows_h: u16,
}

/// The state a paint mutates. A deep clone used to carry it implicitly.
struct PaintState {
    scroll: ScrollPane,
    edit_menu: EditMenu,
    caret_visible: bool,
}

pub(crate) struct PaintResult {
    pub(crate) hardware_cursor: Option<Position>,
}

fn effective_hscroll(horizontal: Option<HScroll>, scroll: &ScrollPane) -> HScroll {
    horizontal.unwrap_or_else(|| scroll.app_hscroll())
}

fn resolve_in(lines: &[Logical], a: Anchor) -> Option<CellPos> {
    let line = lines.iter().position(|l| l.id == a.id)?;
    Some(CellPos { line, col: a.col })
}

impl ViewportProjection<'_> {
    fn h_enabled(&self, scroll: &ScrollPane) -> bool {
        !self.wrap && effective_hscroll(self.horizontal, scroll).enabled
    }

    fn hoff(&self, scroll: &ScrollPane) -> usize {
        if self.h_enabled(scroll) {
            scroll.h.offset
        } else {
            0
        }
    }

    fn text_rect(&self) -> Rect {
        let h = if self.rows_h == 0 {
            self.area.height
        } else {
            self.rows_h.min(self.area.height)
        };
        Rect::new(self.area.x, self.area.y, self.area.width, h)
    }

    fn resolve(&self, a: Anchor) -> Option<CellPos> {
        resolve_in(self.lines, a)
    }

    fn selection(&self) -> Option<(CellPos, CellPos)> {
        let (a, b) = self.select?;
        let a = self.resolve(a)?;
        let b = self.resolve(b)?;
        if a == b {
            return None;
        }
        Some((a.min(b), a.max(b)))
    }

    fn col_of(&self, line: usize, cell: usize) -> usize {
        self.lines.get(line).map(|l| l.col_of(cell)).unwrap_or(0)
    }

    fn screen_pos(&self, scroll: &ScrollPane, c: CellPos) -> Option<Position> {
        let vi = self.visual.iter().position(|v| {
            v.line == c.line
                && self.col_of(v.line, v.start) <= c.col
                && (c.col < self.col_of(v.line, v.end)
                    || v.end == self.lines.get(v.line).map_or(0, |l| l.cells.len()))
        })?;
        if !scroll.visible_range().contains(&vi) {
            return None;
        }
        let vr = self.visual[vi];
        let hoff = self.hoff(scroll);
        let base = self.col_of(vr.line, vr.start) + hoff;
        let text_w = self.layout_width;

        let inside = c.col >= base && c.col - base < text_w as usize;
        if !inside && hoff > 0 {
            return None;
        }
        let cx = self.area.x + (c.col.saturating_sub(base) as u16).min(text_w.saturating_sub(1));
        let cy = self.area.y + (vi - scroll.offset) as u16;
        Some(Position::new(cx, cy))
    }
}

fn paint_projection(
    p: &ViewportProjection<'_>,
    state: &mut PaintState,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    bg: Color,
) -> PaintResult {
    let area = p.area;
    let t = ctx.theme;
    let focused = ctx
        .interaction
        .focused_under(p.id, state.edit_menu.is_open());
    let has_sb = state.scroll.overflows();
    let text_w = p.layout_width;
    let hoff = p.hoff(&state.scroll);
    let text = p.text_rect();
    let right = area.x + text_w;
    ctx.control(p.id, area, false);
    ctx.scrollable(p.id, area);
    ctx.autoscroll(text);
    let sel = p.selection();
    if sel.is_some() {
        ctx.selection(p.id);
    }
    fill(buf, area, Style::new().bg(bg));
    for (k, vi) in state.scroll.visible_range().enumerate() {
        let y = area.y + k as u16;
        if y >= text.bottom() {
            break;
        }
        let Some(vr) = p.visual.get(vi).copied() else {
            break;
        };
        let l = &p.lines[vr.line];
        let line_marks: Vec<&Mark> = p.marks.iter().filter(|m| m.line == vr.line).collect();

        let left = l.col_of(vr.start) + hoff;
        let mut col = l.col_of(vr.start);
        for c in &l.cells[vr.start..vr.end] {
            let at = col;
            col += c.w;

            if at < left {
                continue;
            }
            let x = area.x + (at - left).min(u16::MAX as usize) as u16;
            if x + c.w as u16 > right {
                break;
            }
            let mut st = Style::new().fg(t.tone(c.style.tone)).bg(bg);
            if c.style.bold {
                st = st.add_modifier(Modifier::BOLD);
            }
            if c.style.italic {
                st = st.add_modifier(Modifier::ITALIC);
            }
            if c.style.underline {
                st = st.add_modifier(Modifier::UNDERLINED);
            }
            if c.kind == CellKind::Control {
                st = st.fg(t.text_muted);
            }
            if c.style.reversed {
                st = Style::new().fg(t.canvas).bg(t.text_primary);
            }
            if let Some(m) = line_marks
                .iter()
                .find(|m| m.range.start < c.src.end && c.src.start < m.range.end)
            {
                st = if m.current {
                    t.match_current()
                } else {
                    st.add_modifier(Modifier::UNDERLINED).bg(t.lift(bg))
                };
            }
            let at_pos = CellPos {
                line: vr.line,
                col: at,
            };
            if let Some((a, b)) = sel
                && at_pos >= a
                && at_pos < b
            {
                st = t.selection().add_modifier(st.add_modifier);
            }
            buf.set_string(x, y, &c.g, st);
        }

        let end_col = l.col_of(vr.end);
        if let Some((a, b)) = sel
            && vr.line >= a.line
            && vr.line < b.line
            && vr.end == l.cells.len()
            && end_col >= left
        {
            let x = area.x + (end_col - left).min(u16::MAX as usize) as u16;
            if x < right {
                fill(buf, Rect::new(x, y, 1, 1), t.selection());
            }
        }
    }
    if has_sb {
        let body = Rect::new(area.x, area.y, area.width, text.height);
        state
            .scroll
            .draw_with_fade(body, buf, ctx, p.id, focused, &[]);
    }
    if text.height < area.height && p.h_enabled(&state.scroll) {
        let w = area.width.saturating_sub(u16::from(has_sb));
        let hb = Rect::new(area.x, text.bottom(), w, 1);
        state.scroll.draw_h(hb, buf, ctx, p.id, focused);
    }

    let cursor_pos = if focused {
        match p.browse.and_then(|b| p.resolve(b)) {
            Some(b) if p.select.is_some() => Some(b),
            _ if p.follow && state.caret_visible => p.caret,
            _ => None,
        }
    } else {
        None
    };
    if let Some(pos) = cursor_pos.and_then(|c| p.screen_pos(&state.scroll, c)) {
        ctx.set_cursor(pos);
    }
    state.edit_menu.render(area, buf, ctx);

    let hardware_cursor = if p.follow && state.caret_visible {
        p.caret.and_then(|at| p.screen_pos(&state.scroll, at))
    } else {
        None
    };
    PaintResult { hardware_cursor }
}

impl termrock_runtime::measure::Measure for TextViewport {
    fn measure(&self, _avail: ratatui::layout::Size, _ctx: &RenderCtx) -> ratatui::layout::Size {
        let widest = self.lines.iter().map(|l| width(&l.text)).max().unwrap_or(0) as u16;
        let bar = 1 + self.scrollbar_gap;
        ratatui::layout::Size::new(
            widest.saturating_add(bar),
            self.lines.len().min(u16::MAX as usize) as u16,
        )
    }

    fn min_width(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> u16 {
        self.measure(avail, ctx).width.min(MIN_W)
    }
}

const MIN_W: u16 = 24;

impl Scrollable for TextViewport {
    fn pane(&mut self) -> &mut ScrollPane {
        self.adopt_policy();
        &mut self.scroll
    }

    fn wheel_first(&mut self, axis: Axis, _delta: i32) -> Option<Response<()>> {
        (axis == Axis::Horizontal && !self.h_enabled()).then(Response::ignored)
    }

    fn after_scroll(&mut self, axis: Axis, moved: bool) -> bool {
        if axis == Axis::Vertical && moved {
            self.follow = self.is_at_tail();
            self.remember_reading();
        }
        false
    }
}

impl termrock_core::pointer::MouseTarget for TextViewport {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &termrock_core::pointer::PointerEvent,
    ) -> termrock_core::response::Response<()> {
        use termrock_core::intent::Phase;
        use termrock_core::response::Response;
        if let Some(o) = edit_menu_pointer(self, ev) {
            return o.into();
        }
        if let Some(r) = termrock_core::pointer::scroll_input(self, ev) {
            return r;
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_press_with(ev.pos(), ev.count(), ev.mods()).into(),
            Some(Phase::Drag) => self.on_drag(ev.pos()).into(),

            Some(Phase::Click) => self.on_click_with(ev.pos(), ev.count()).into(),
            _ => Response::ignored(),
        }
    }

    fn route(&mut self, ev: &termrock_core::pointer::PointerEvent) -> Option<Response<()>> {
        edit_menu_route(self, ev)
    }
}

impl EditMenuHost for TextViewport {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.edit_menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        EditCaps::read_only(self.select.range().is_some(), !self.is_empty())
    }
    fn edit_shortcut(&self, cmd: TextCmd) -> Option<String> {
        let cmd = match cmd {
            TextCmd::Copy => ViewportCmd::Copy,
            TextCmd::SelectAll => ViewportCmd::SelectAll,
            _ => return None,
        };
        termrock_core::keymap::label_for(&self.keymap, cmd)
    }
    fn edit_target(&self, ev: &termrock_core::pointer::PointerEvent) -> bool {
        ev.id == self.id
    }
    fn edit_run(&mut self, cmd: TextCmd) -> bool {
        match cmd {
            TextCmd::SelectAll => {
                self.select_all();
            }
            TextCmd::Copy => {
                if let Some(t) = self.selected_text() {
                    termrock_core::edit::clipboard::copy(&t);
                }
            }
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport_ops::ViewportOps;
    use termrock_core::focus::FocusRing;
    use termrock_core::hit::HitRegistry;
    use termrock_runtime::ctx::Interaction;
    use termrock_theme::{ColorLevel, Theme};

    const AREA: Rect = Rect::new(0, 0, 80, 24);

    fn draw_frame(v: &TextViewport) {
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buf = Buffer::empty(AREA);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        v.draw(AREA, &mut buf, &mut ctx, theme.canvas, true);
    }

    fn clones() -> usize {
        LOGICAL_CLONES.with(std::cell::Cell::get)
    }

    #[test]
    fn prepared_draw_clones_no_logical_line_at_any_history_size() {
        for n in [1_000usize, 100_000] {
            let lines = (0..n)
                .map(|i| vec![Span::plain(format!("row {i} of the retained history"))])
                .collect();
            let mut v = TextViewport::with_lines(WidgetId::of("t.cost"), lines);
            v.set_area(AREA);
            let before = clones();
            for _ in 0..3 {
                draw_frame(&v);
            }
            assert_eq!(
                clones() - before,
                0,
                "{n} lines: unchanged prepared frames copy no logical line"
            );

            v.wrap = true;
            draw_frame(&v);
            assert_eq!(
                clones() - before,
                n,
                "{n} lines: the counter sees the fallback copy it is meant to catch"
            );
        }
    }
}
