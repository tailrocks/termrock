use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use unicode_segmentation::UnicodeSegmentation;

use termrock_core::event::{Key, Outcome};
use termrock_core::hit::Axis;
use termrock_core::id::WidgetId;
use termrock_core::response::Response;
use termrock_core::scroll::{ScrollPane, Scrollable};
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::specimen::{Specimen, SpecimenState};
use termrock_text::cells::{expand_tabs, fit, truncate, width};
use termrock_theme::{Glyphs, Strings, Tone};
use termrock_viewport::viewport::{Line, Span, TextViewport, ViewportAction};
use termrock_viewport::viewport_specimen::SAMPLE_ROWS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Add,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

impl DiffLine {
    pub fn context(text: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Context,
            text: text.into(),
        }
    }
    pub fn add(text: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Add,
            text: text.into(),
        }
    }
    pub fn remove(text: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Remove,
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    pub fn old_len(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind != DiffLineKind::Add)
            .count()
    }

    pub fn new_len(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind != DiffLineKind::Remove)
            .count()
    }
    pub fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old_start,
            self.old_len(),
            self.new_start,
            self.new_len()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
}

impl DiffStatus {
    pub fn marker(&self) -> &'static str {
        match self {
            DiffStatus::Added => "A",
            DiffStatus::Modified => "M",
            DiffStatus::Deleted => "D",
            DiffStatus::Renamed { .. } => "R",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            DiffStatus::Added => "added",
            DiffStatus::Modified => "modified",
            DiffStatus::Deleted => "deleted",
            DiffStatus::Renamed { .. } => "renamed",
        }
    }

    pub fn tone(&self) -> Tone {
        match self {
            DiffStatus::Added => Tone::Success,
            DiffStatus::Deleted => Tone::Error,
            DiffStatus::Modified => Tone::Warning,
            DiffStatus::Renamed { .. } => Tone::Secondary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile {
    pub path: String,
    pub status: DiffStatus,
    pub hunks: Vec<DiffHunk>,
}

impl DiffFile {
    pub fn additions(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.kind == DiffLineKind::Add)
            .count()
    }
    pub fn deletions(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| h.lines.iter())
            .filter(|l| l.kind == DiffLineKind::Remove)
            .count()
    }

    pub fn summary(&self) -> String {
        self.summary_with(&Glyphs::UNICODE)
    }

    pub fn summary_with(&self, g: &Glyphs) -> String {
        let n = self.hunks.len();
        format!(
            "+{} {}{}{}{} hunk{}",
            self.additions(),
            g.removed,
            self.deletions(),
            g.separator,
            n,
            if n == 1 { "" } else { "s" }
        )
    }

    pub fn header(&self) -> String {
        self.header_with(&Glyphs::UNICODE)
    }

    pub fn header_with(&self, g: &Glyphs) -> String {
        match &self.status {
            DiffStatus::Renamed { from } => {
                format!("{} {from} {} {}", self.status.marker(), g.arrow, self.path)
            }
            _ => format!("{} {}", self.status.marker(), self.path),
        }
    }
}

/// A changed file the list names but whose content was not read, so it has
/// no hunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadFile {
    pub path: String,
    pub status: DiffStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    #[default]
    Unified,

    Review,
}

impl DiffMode {
    pub fn label(self) -> &'static str {
        match self {
            DiffMode::Unified => "unified",
            DiffMode::Review => "review",
        }
    }
    pub fn toggled(self) -> Self {
        match self {
            DiffMode::Unified => DiffMode::Review,
            DiffMode::Review => DiffMode::Unified,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiffView {
    pub term: TextViewport,
    pub mode: DiffMode,
    file: Option<DiffFile>,
    laid_width: u16,
    laid_mode: DiffMode,
    dirty: bool,
    rendered_size: Option<(u16, u16, DiffMode, bool)>,

    glyphs: Glyphs,
    strings: Strings,
}

impl DiffView {
    pub fn new(id: WidgetId) -> Self {
        Self {
            term: TextViewport::new(id),
            mode: DiffMode::Unified,
            file: None,
            laid_width: 0,
            laid_mode: DiffMode::Unified,
            dirty: true,
            rendered_size: None,
            glyphs: Glyphs::UNICODE,
            strings: Strings::DEFAULT,
        }
    }

    pub fn id(&self) -> WidgetId {
        self.term.id
    }

    pub fn file(&self) -> Option<&DiffFile> {
        self.file.as_ref()
    }

    pub fn set_file(&mut self, file: Option<DiffFile>) {
        if self.file != file {
            self.file = file;
            self.dirty = true;
            self.term.scroll.jump_start();
            self.term.scroll.h.jump_start();
            self.term.set_follow(false);
        }
    }

    pub fn set_mode(&mut self, mode: DiffMode) {
        if self.mode != mode {
            self.mode = mode;
            self.dirty = true;
        }
    }

    pub fn toggle_mode(&mut self) -> DiffMode {
        self.set_mode(self.mode.toggled());
        self.mode
    }

    pub fn layout_mode(&self, width: u16) -> DiffMode {
        match &self.file {
            Some(file) if self.mode == DiffMode::Review && review_fits(file, width) => {
                DiffMode::Review
            }
            _ => DiffMode::Unified,
        }
    }

    pub fn layout(&mut self, width: u16) {
        let mode = self.layout_mode(width);
        if !self.dirty && self.laid_width == width && self.laid_mode == mode {
            return;
        }
        self.rendered_size = None;
        let lines = match &self.file {
            Some(f) => match mode {
                DiffMode::Unified => unified_lines_with(f, &self.glyphs, &self.strings),
                DiffMode::Review => review_lines_with(f, width, &self.glyphs, &self.strings),
            },
            None => vec![vec![Span::muted(self.strings.diff_no_file)]],
        };
        let offset = self.term.scroll.offset;
        self.term.set_lines(lines);
        self.term.set_follow(false);
        self.term.scroll.scroll_to(offset);
        self.laid_width = width;
        self.laid_mode = mode;
        self.dirty = false;
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<ViewportAction>) {
        self.term.on_key(key)
    }

    pub fn on_click(&mut self, pos: Position) -> Outcome {
        self.term.on_click(pos)
    }

    pub fn on_drag(&mut self, pos: Position) -> Outcome {
        self.term.on_drag(pos)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        if self.glyphs != ctx.theme.glyphs || self.strings != ctx.theme.strings {
            self.glyphs = ctx.theme.glyphs;
            self.strings = ctx.theme.strings;
            self.dirty = true;
        }
        let size = (area.width, area.height, self.mode, self.term.wrap);
        if self.dirty || self.rendered_size != Some(size) {
            self.layout(area.width);
            self.term.set_area(area);
            if self.term.scroll.overflows() {
                self.layout(
                    area.width
                        .saturating_sub(termrock_controls::scrollbar::reserve(
                            self.term.scrollbar_gap,
                        )),
                );
            }

            self.rendered_size = Some(size);
        }
        self.term.render(area, buf, ctx, bg);
    }
}

fn gutter(n: Option<usize>, w: usize) -> Span {
    let text = match n {
        Some(n) => format!("{n:>w$}"),
        None => " ".repeat(w),
    };
    Span::new(text, Tone::Faint)
}

fn num_width(f: &DiffFile) -> usize {
    let max = f
        .hunks
        .iter()
        .map(|h| (h.old_start + h.old_len()).max(h.new_start + h.new_len()))
        .max()
        .unwrap_or(1);
    max.to_string().len().max(3)
}

pub fn unified_lines(f: &DiffFile) -> Vec<Line> {
    unified_lines_with(f, &Glyphs::UNICODE, &Strings::DEFAULT)
}

pub fn unified_lines_with(f: &DiffFile, g: &Glyphs, s: &Strings) -> Vec<Line> {
    let nw = num_width(f);
    let mut out: Vec<Line> = vec![];
    out.push(vec![
        Span::new(f.status.marker(), f.status.tone()).bold(),
        Span::plain(" "),
        Span::plain(f.header_with(g)[2..].to_owned()).bold(),
        Span::muted(format!("  {}", f.summary_with(g))),
    ]);
    for h in &f.hunks {
        out.push(vec![Span::muted(h.header())]);
        let mut old = h.old_start;
        let mut new = h.new_start;
        for l in &h.lines {
            let (o, n, marker, tone) = match l.kind {
                DiffLineKind::Context => {
                    let r = (Some(old), Some(new), " ", Tone::Secondary);
                    old += 1;
                    new += 1;
                    r
                }
                DiffLineKind::Add => {
                    let r = (None, Some(new), "+", Tone::Success);
                    new += 1;
                    r
                }
                DiffLineKind::Remove => {
                    let r = (Some(old), None, "-", Tone::Error);
                    old += 1;
                    r
                }
            };
            out.push(vec![
                gutter(o, nw),
                Span::plain(" "),
                gutter(n, nw),
                Span::plain(" "),
                Span::new(marker, tone).bold(),
                Span::new(format!(" {}", l.text), tone),
            ]);
        }
    }
    if f.hunks.is_empty() {
        out.push(vec![Span::muted(s.diff_no_changes)]);
    }
    out
}

fn changed_range(a: &str, b: &str) -> (usize, usize) {
    let ac: Vec<&str> = a.graphemes(true).collect();
    let bc: Vec<&str> = b.graphemes(true).collect();
    let mut p = 0;
    while p < ac.len() && p < bc.len() && ac[p] == bc[p] {
        p += 1;
    }
    let mut s = 0;
    while s < ac.len() - p && s < bc.len() - p && ac[ac.len() - 1 - s] == bc[bc.len() - 1 - s] {
        s += 1;
    }
    (p, s)
}

fn emphasised(text: &str, p: usize, s: usize, tone: Tone, col_w: usize) -> Vec<Span> {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let end = graphemes.len().saturating_sub(s).max(p);
    let head = graphemes[..p.min(graphemes.len())].concat();
    let mid = graphemes[p.min(graphemes.len())..end].concat();
    let tail = graphemes[end..].concat();

    let mut spans = vec![];
    let mut used = 0usize;
    for (text, bold) in [(head, false), (mid, true), (tail, false)] {
        if used >= col_w {
            break;
        }
        let t = truncate(&text, col_w - used);
        used += width(&t);
        if !t.is_empty() {
            let sp = Span::new(t, tone);
            spans.push(if bold { sp.bold() } else { sp });
        }
    }
    if used < col_w {
        spans.push(Span::plain(" ".repeat(col_w - used)));
    }
    spans
}

pub fn review_lines(f: &DiffFile, width: u16) -> Vec<Line> {
    review_lines_with(f, width, &Glyphs::UNICODE, &Strings::DEFAULT)
}

pub fn review_lines_with(f: &DiffFile, width: u16, g: &Glyphs, s: &Strings) -> Vec<Line> {
    if !review_fits(f, width) {
        return unified_lines_with(f, g, s);
    }
    let nw = num_width(f);
    let total = width as usize;

    let col = total.saturating_sub(3) / 2;
    let text_w = col.saturating_sub(nw + 1).max(4);
    let mut out: Vec<Line> = vec![];
    out.push(vec![
        Span::new(f.status.marker(), f.status.tone()).bold(),
        Span::plain(" "),
        Span::plain(f.header_with(g)[2..].to_owned()).bold(),
        Span::muted(format!(
            "  {}{}{}",
            f.summary_with(g),
            g.separator,
            s.diff_review
        )),
    ]);
    let sep = || Span::new(format!(" {} ", g.divider), Tone::Faint);
    out.push(vec![
        Span::muted(fit(s.diff_old, col)),
        sep(),
        Span::muted(fit(s.diff_new, col)),
    ]);
    let blank_col =
        |nw: usize, text_w: usize| -> Vec<Span> { vec![Span::plain(" ".repeat(nw + 1 + text_w))] };
    let side = |n: usize, text: &str, tone: Tone, bold: Option<(usize, usize)>| -> Vec<Span> {
        let mut v = vec![Span::new(format!("{n:>nw$} "), Tone::Faint)];
        match bold {
            Some((p, s)) => v.extend(emphasised(text, p, s, tone, text_w)),
            None => v.push(Span::new(fit(&truncate(text, text_w), text_w), tone)),
        }
        v
    };
    for (hi, h) in f.hunks.iter().enumerate() {
        if hi > 0 {
            out.push(vec![Span::new(
                g.bar_empty.repeat(total.min(200)),
                Tone::Faint,
            )]);
        }
        out.push(vec![Span::muted(h.header())]);
        let mut old = h.old_start;
        let mut new = h.new_start;
        let mut i = 0;
        while i < h.lines.len() {
            match h.lines[i].kind {
                DiffLineKind::Context => {
                    let text = expand_tabs(&h.lines[i].text);
                    let mut row = side(old, &text, Tone::Secondary, None);
                    row.push(sep());
                    row.extend(side(new, &text, Tone::Secondary, None));
                    out.push(row);
                    old += 1;
                    new += 1;
                    i += 1;
                }
                _ => {
                    let mut removes = vec![];
                    while i < h.lines.len() && h.lines[i].kind == DiffLineKind::Remove {
                        removes.push(expand_tabs(&h.lines[i].text));
                        i += 1;
                    }
                    let mut adds = vec![];
                    while i < h.lines.len() && h.lines[i].kind == DiffLineKind::Add {
                        adds.push(expand_tabs(&h.lines[i].text));
                        i += 1;
                    }
                    let n = removes.len().max(adds.len());
                    for k in 0..n {
                        let mut row: Vec<Span> = vec![];
                        match (removes.get(k), adds.get(k)) {
                            (Some(r), Some(a)) => {
                                let (p, s) = changed_range(r, a);
                                row.extend(side(old, r, Tone::Error, Some((p, s))));
                                row.push(sep());
                                row.extend(side(new, a, Tone::Success, Some((p, s))));
                                old += 1;
                                new += 1;
                            }
                            (Some(r), None) => {
                                row.extend(side(old, r, Tone::Error, None));
                                row.push(sep());
                                row.extend(blank_col(nw, text_w));
                                old += 1;
                            }
                            (None, Some(a)) => {
                                row.extend(blank_col(nw, text_w));
                                row.push(sep());
                                row.extend(side(new, a, Tone::Success, None));
                                new += 1;
                            }
                            (None, None) => {}
                        }
                        out.push(row);
                    }
                }
            }
        }
    }
    if f.hunks.is_empty() {
        out.push(vec![Span::muted(s.diff_no_changes)]);
    }
    out
}

fn review_fits(file: &DiffFile, width: u16) -> bool {
    usize::from(width) >= 2 * (num_width(file) + 1 + 16) + 3
}

impl Scrollable for DiffView {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.term.scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        self.term.wheel_first(axis, delta)
    }

    fn after_scroll(&mut self, axis: Axis, moved: bool) -> bool {
        self.term.after_scroll(axis, moved)
    }
}

impl termrock_core::pointer::MouseTarget for DiffView {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id()
    }

    fn pointer(
        &mut self,
        ev: &termrock_core::pointer::PointerEvent,
    ) -> termrock_core::response::Response<()> {
        use termrock_core::intent::Phase;
        use termrock_core::response::Response;

        if ev.is(Phase::Secondary) || self.term.edit_menu.owns(ev) {
            return self.term.pointer(ev);
        }
        if let Some(r) = termrock_core::pointer::scroll_input(self, ev) {
            return r;
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_click(ev.pos()).into(),
            Some(Phase::Drag) => self.on_drag(ev.pos()).into(),
            Some(Phase::Click) => Response::consumed(),
            _ => Response::ignored(),
        }
    }

    fn route(&mut self, ev: &termrock_core::pointer::PointerEvent) -> Option<Response<()>> {
        (ev.on(self.id()) || self.term.edit_menu.owns(ev)).then(|| self.pointer(ev))
    }
}

impl Specimen for DiffView {
    fn specimen_id(&self) -> WidgetId {
        self.id()
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        matches!(state, Rest | Focused | Empty | Narrow)
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        SAMPLE_ROWS + 1
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut d = self.clone();
        if state == SpecimenState::Empty {
            d.set_file(None);
        }
        d.render(area, buf, ctx, bg);
    }
}
