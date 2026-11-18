use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};

use super::{Defaults, RowWrap, TERMINAL_DEFAULTS, TermEvent, TermView, TerminalState, paint_view};
use termrock_controls::scrollbar;
use termrock_core::event::Outcome;
use termrock_core::field::field_common::PointerSelect;
use termrock_core::field::text_select::{TextSelect, is_word, word_bounds};
use termrock_core::hit::{Axis, Bar};
use termrock_core::id::WidgetId;
use termrock_core::intent::Phase;
use termrock_core::pointer::{MouseTarget, PointerEvent, scroll_input};
use termrock_core::response::Response;
use termrock_core::scroll::{ScrollPane, Scrollable};
use termrock_runtime::ctx::{RenderCtx, fill};
use termrock_text::cells::thousands;
use termrock_theme::ColorLevel;
use termrock_viewport::viewport::CellPos;
use termrock_viewport::viewport_ops::ViewportOps;

const TRACK_COLUMNS: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GridPos {
    line: u64,
    col: u16,
}

/// The terminal counterpart of `TextViewport`: it owns a [`TerminalState`] and
/// adds the scroll position, the selection and the copy text a pane needs.
///
/// Rows are numbered from the first row the state ever pushed into
/// scrollback (`TerminalState::scrolled_off` plus the row index), so the
/// scroll position and the selection stay on the same text while new output
/// arrives. The last column of the area is the scroll track; the program gets
/// `area.width - 1` columns whether or not there is history, so its width
/// never changes when the first history appears (see [`Self::grid_size`]).
#[derive(Debug)]
pub struct TerminalViewport {
    pub id: WidgetId,
    pub pointer_select: PointerSelect,
    state: TerminalState,
    offset: usize,
    scroll: ScrollPane,
    select: TextSelect<GridPos>,
    area: Rect,
}

impl TerminalViewport {
    pub fn new(id: WidgetId, state: TerminalState) -> Self {
        let mut viewport = Self {
            id,
            pointer_select: PointerSelect::DEFAULT,
            state,
            offset: 0,
            scroll: ScrollPane::default(),
            select: TextSelect::default(),
            area: Rect::ZERO,
        };
        viewport.settle();
        viewport
    }

    pub fn pointer_select(mut self, policy: PointerSelect) -> Self {
        self.pointer_select = policy;
        self
    }

    /// The `(rows, cols)` a program gets in `area`: every row, and every
    /// column but the track column.
    pub fn grid_size(area: Rect) -> (u16, u16) {
        (
            area.height.max(1),
            area.width.saturating_sub(TRACK_COLUMNS).max(1),
        )
    }

    pub fn state(&self) -> &TerminalState {
        &self.state
    }

    /// Rows between the view and the live tail; 0 is the live tail.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Feeds program output. New output does not move what the user reads
    /// while the view is scrolled back, and the alternate screen and a
    /// cleared scrollback start over with offset 0 and no selection.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<TermEvent> {
        let alt = self.state.modes().alt_screen;
        let pushed = self.state.scrolled_off();
        let events = self.state.feed(bytes);
        if self.state.modes().alt_screen != alt || events.contains(&TermEvent::ScrollbackCleared) {
            self.offset = 0;
            self.select = TextSelect::default();
        } else if self.offset > 0 {
            let rows = self.state.scrolled_off() - pushed;
            self.offset = self
                .offset
                .saturating_add(usize::try_from(rows).unwrap_or(usize::MAX));
        }
        self.settle();
        events
    }

    /// Resizes the grid; a selection does not survive it.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if self.state.size() == (rows.max(1), cols.max(1)) {
            return;
        }
        self.state.resize(rows, cols);
        self.select = TextSelect::default();
        self.settle();
    }

    /// Drops the history and returns to the live tail. The screen is kept, a
    /// selection keeps the part that is on it, and no byte goes to the
    /// program.
    pub fn clear_scrollback(&mut self) {
        self.state.clear_scrollback();
        self.offset = 0;
        self.settle();
    }

    pub fn reset(&mut self) {
        self.state.reset();
        self.offset = 0;
        self.select = TextSelect::default();
        self.settle();
    }

    /// `a–b of n` for the rows on screen while scrolled back, else empty.
    pub fn position_label(&self) -> String {
        if self.offset == 0 {
            return String::new();
        }
        let range = self.scroll.visible_range();
        format!(
            "{}–{} of {}",
            thousands(range.start + 1),
            thousands(range.end),
            thousands(self.scroll.content_len)
        )
    }

    pub fn on_press_with(&mut self, pos: Position, count: u8, mods: KeyModifiers) -> Outcome {
        let at = self.pos_at(pos);
        let word = at.and_then(|at| self.word_at(at));
        let line = at.and_then(|at| self.line_at(at));
        self.select
            .press(self.pointer_select, at, count, mods, word, line)
    }

    pub fn on_click_with(&mut self, pos: Position, count: u8) -> Outcome {
        let at = self.pos_at(pos);
        let word = at.and_then(|at| self.word_at(at));
        let line = at.and_then(|at| self.line_at(at));
        self.select
            .click(self.pointer_select, pos, count, word, line)
    }

    fn history(&self) -> usize {
        if self.state.modes().alt_screen {
            0
        } else {
            self.state.scrollback_len()
        }
    }

    fn rows(&self) -> usize {
        usize::from(self.state.size().0)
    }

    fn top_line(&self) -> u64 {
        self.state
            .scrolled_off()
            .saturating_sub(u64::try_from(self.offset).unwrap_or(u64::MAX))
    }

    fn settle(&mut self) {
        let history = self.history();
        self.offset = self.offset.min(history);
        self.scroll.set_content(history + self.rows());
        self.scroll.set_viewport(self.rows());
        self.scroll.scroll_to(history - self.offset);
        self.clip_selection();
    }

    fn pull(&mut self) {
        let history = self.history();
        self.offset = history - self.scroll.offset.min(history);
        self.clip_selection();
    }

    fn clip_selection(&mut self) {
        let oldest = self
            .state
            .scrolled_off()
            .saturating_sub(u64::try_from(self.history()).unwrap_or(u64::MAX));
        if let Some((a, b)) = self.select.sel
            && (a.line < oldest || b.line < oldest)
        {
            let keep = |p: GridPos| {
                if p.line < oldest {
                    GridPos {
                        line: oldest,
                        col: 0,
                    }
                } else {
                    p
                }
            };
            let (a, b) = (keep(a), keep(b));
            self.select.sel = (a != b).then_some((a, b));
        }
    }

    /// The view that holds `line`, and the row of `line` in it. Reading a row
    /// of the history goes through `TerminalState::view` like drawing does,
    /// so a row is read the way it is shown.
    fn locate(&self, line: u64) -> Option<(TermView<'_>, u16)> {
        let scrolled = self.state.scrolled_off();
        let history = u64::try_from(self.history()).ok()?;
        let rows = u64::from(self.state.size().0);
        if line + history < scrolled || line >= scrolled + rows {
            return None;
        }
        let back = scrolled.saturating_sub(line).min(history);
        let row = line - (scrolled - back);
        termrock_metrics::record_work!(SourceRowsRead, 1usize);
        Some((
            self.state.view(usize::try_from(back).ok()?),
            u16::try_from(row).ok()?,
        ))
    }

    fn pos_at(&self, pos: Position) -> Option<GridPos> {
        if self.area.is_empty() {
            return None;
        }
        let (rows, cols) = self.state.size();
        let row = pos
            .y
            .saturating_sub(self.area.y)
            .min(rows.saturating_sub(1))
            .min(self.area.height - 1);
        let line = self.top_line() + u64::from(row);
        let mut col = pos.x.saturating_sub(self.area.x).min(cols);
        if col > 0
            && col < cols
            && let Some((view, at)) = self.locate(line)
            && view
                .cell(at, col)
                .is_some_and(|cell| cell.is_wide_continuation())
        {
            col -= 1;
        }
        Some(GridPos { line, col })
    }

    fn word_at(&self, at: GridPos) -> Option<(GridPos, GridPos)> {
        let (view, row) = self.locate(at.line)?;
        let cols = view.cols();
        let cells: Vec<(u16, String)> = (0..cols)
            .filter_map(|col| {
                let cell = view.cell(row, col)?;
                (!cell.is_wide_continuation()).then(|| {
                    let text = cell.text();
                    (col, if text.is_empty() { " " } else { text }.to_owned())
                })
            })
            .collect();
        let index = cells.iter().rposition(|(col, _)| *col <= at.col)?;
        let (first, end) = word_bounds(&cells, index, |(_, text)| is_word(text))?;
        let from = cells[first].0;
        let to = cells.get(end).map_or(cols, |(col, _)| *col);
        Some((
            GridPos {
                line: at.line,
                col: from,
            },
            GridPos {
                line: at.line,
                col: to,
            },
        ))
    }

    fn is_soft(&self, line: u64) -> bool {
        self.locate(line)
            .is_some_and(|(view, row)| view.row_wrap(row) == Some(RowWrap::Soft))
    }

    fn line_at(&self, at: GridPos) -> Option<(GridPos, GridPos)> {
        self.locate(at.line)?;
        let cols = self.state.size().1;
        let mut first = at.line;
        while first > 0 && self.is_soft(first) && self.locate(first - 1).is_some() {
            first -= 1;
        }
        let mut last = at.line;
        while self.is_soft(last + 1) {
            last += 1;
        }
        Some((
            GridPos {
                line: first,
                col: 0,
            },
            GridPos {
                line: last,
                col: cols,
            },
        ))
    }

    fn text_between(&self, a: GridPos, b: GridPos) -> String {
        let cols = self.state.size().1;
        let mut out = String::new();
        for line in a.line..=b.line {
            let Some((view, row)) = self.locate(line) else {
                continue;
            };
            let from = if line == a.line { a.col } else { 0 };
            let to = if line == b.line { b.col } else { cols };
            let segment = row_text(&view, row, from, to);
            if line < b.line && self.is_soft(line + 1) {
                out.push_str(&segment);
            } else {
                out.push_str(segment.trim_end_matches(' '));
                if line < b.line {
                    out.push('\n');
                }
            }
        }
        out
    }

    fn is_blank(&self, line: u64) -> bool {
        self.locate(line).is_none_or(|(view, row)| {
            (0..view.cols()).all(|col| {
                view.cell(row, col)
                    .is_none_or(|cell| cell.text().trim().is_empty())
            })
        })
    }

    fn grid(&self, area: Rect) -> Rect {
        let width = if area.width > TRACK_COLUMNS {
            area.width - TRACK_COLUMNS
        } else {
            area.width
        };
        Rect::new(area.x, area.y, width, area.height)
    }
}

fn row_text(view: &TermView<'_>, row: u16, from: u16, to: u16) -> String {
    let to = to.min(view.cols());
    let from = from.min(to);
    let mut out = String::new();
    for col in from..to {
        let Some(cell) = view.cell(row, col) else {
            break;
        };
        if cell.is_wide_continuation() {
            continue;
        }
        match cell.text() {
            "" => out.push(' '),
            text => out.push_str(text),
        }
    }
    out
}

impl Scrollable for TerminalViewport {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.scroll
    }

    fn wheel_first(&mut self, axis: Axis, _delta: i32) -> Option<Response<()>> {
        (axis == Axis::Horizontal).then(Response::ignored)
    }

    fn after_scroll(&mut self, axis: Axis, moved: bool) -> bool {
        if axis == Axis::Vertical && moved {
            self.pull();
        }
        false
    }

    fn bar_input(&mut self, bar: &Bar, pos: Position, pressed: bool) -> Response<()> {
        let response = self.scroll.bar(bar, pos, pressed);
        if bar.axis == Axis::Vertical && response.is_changed() {
            self.pull();
        }
        response
    }
}

impl MouseTarget for TerminalViewport {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(&mut self, ev: &PointerEvent) -> Response<()> {
        if let Some(response) = scroll_input(self, ev) {
            return response;
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_press_with(ev.pos(), ev.count(), ev.mods()).into(),
            Some(Phase::Drag) => self.on_drag(ev.pos()).into(),
            Some(Phase::Click) => self.on_click_with(ev.pos(), ev.count()).into(),
            _ => Response::ignored(),
        }
    }
}

impl ViewportOps for TerminalViewport {
    fn id(&self) -> WidgetId {
        self.id
    }

    fn set_area(&mut self, area: Rect) {
        self.area = area;
        self.settle();
    }

    fn hbar(&self) -> Rect {
        Rect::ZERO
    }

    fn vbar(&self) -> Rect {
        if self.area.width > TRACK_COLUMNS && self.scroll.v.overflows() {
            Rect::new(self.area.right() - 1, self.area.y, 1, self.area.height)
        } else {
            Rect::ZERO
        }
    }

    fn bar_input(&mut self, bar: &Bar, pos: Position, pressed: bool) -> Outcome {
        Scrollable::bar_input(self, bar, pos, pressed).outcome()
    }

    fn scroll(&self) -> &ScrollPane {
        &self.scroll
    }

    fn follow(&self) -> bool {
        self.offset == 0
    }

    fn set_follow(&mut self, on: bool) {
        if on {
            self.offset = 0;
            self.settle();
        }
    }

    fn wheel(&mut self, axis: Axis, delta: i32) -> Outcome {
        Scrollable::wheel(self, axis, delta).outcome()
    }

    fn page_up(&mut self) {
        self.offset = self.offset.saturating_add(self.rows());
        self.settle();
    }

    fn page_down(&mut self) {
        self.offset = self.offset.saturating_sub(self.rows());
        self.settle();
    }

    fn jump_start(&mut self) {
        self.offset = self.history();
        self.settle();
    }

    fn has_selection(&self) -> bool {
        self.select.range().is_some()
    }

    fn selected_text(&self) -> Option<String> {
        let (a, b) = self.select.range()?;
        let text = self.text_between(a, b);
        (!text.is_empty()).then_some(text)
    }

    fn clear_selection(&mut self) -> Outcome {
        self.select.clear()
    }

    fn select_word_at(&mut self, pos: Position) -> Outcome {
        let Some(at) = self.pos_at(pos) else {
            return Outcome::Ignored;
        };
        let Some((a, b)) = self.word_at(at) else {
            return if self.is_blank(at.line) {
                Outcome::Ignored
            } else {
                Outcome::Consumed
            };
        };
        self.select.set(a, b);
        Outcome::Changed
    }

    fn on_click(&mut self, pos: Position) -> Outcome {
        if self.select.dragged.take() == Some(pos) && self.select.sel.is_some() {
            return Outcome::Consumed;
        }
        self.on_press_with(pos, 1, KeyModifiers::NONE)
    }

    fn on_press_with(&mut self, pos: Position, count: u8, mods: KeyModifiers) -> Outcome {
        TerminalViewport::on_press_with(self, pos, count, mods)
    }

    fn on_click_with(&mut self, pos: Position, count: u8) -> Outcome {
        TerminalViewport::on_click_with(self, pos, count)
    }

    fn on_drag(&mut self, pos: Position) -> Outcome {
        if self.select.press.is_none() {
            return Outcome::Ignored;
        }
        if self.area.is_empty() {
            self.select.press = None;
            return Outcome::Ignored;
        }
        let top = self.area.y;
        let bottom = self.area.bottom();
        if pos.y < top {
            self.offset = self.offset.saturating_add(usize::from(top - pos.y));
        } else if pos.y >= bottom {
            self.offset = self.offset.saturating_sub(usize::from(pos.y + 1 - bottom));
        }
        self.settle();
        let inside = Position::new(
            pos.x
                .clamp(self.area.x, self.area.right().saturating_sub(1)),
            pos.y.clamp(top, bottom.saturating_sub(1)),
        );
        let head = self.pos_at(inside);
        let word = if self.select.by_word() {
            head.and_then(|head| self.word_at(head))
        } else {
            None
        };
        self.select.drag(self.pointer_select, head, pos, word)
    }

    fn context_point(&self) -> Position {
        if let Some((_, end)) = self.select.range()
            && let Some(at) = self.screen_pos(CellPos {
                line: usize::try_from(end.line).unwrap_or(usize::MAX),
                col: usize::from(end.col.min(self.state.size().1.saturating_sub(1))),
            })
        {
            return at;
        }
        if self.caret_visible()
            && self.offset == 0
            && let Some(at) = self.caret().and_then(|caret| self.screen_pos(caret))
        {
            return at;
        }
        if self.offset == 0 {
            let drawn = u16::try_from(self.rows())
                .unwrap_or(u16::MAX)
                .min(self.area.height);
            if drawn > 0 {
                return Position::new(self.area.x, self.area.y + drawn - 1);
            }
        }
        Position::new(self.area.x, self.area.y)
    }

    fn keyboard_copy(&self) -> bool {
        false
    }

    fn draws_track(&self) -> bool {
        true
    }

    fn caret(&self) -> Option<CellPos> {
        let (row, col) = self.state.view(0).cursor();
        Some(CellPos {
            line: usize::try_from(self.state.scrolled_off()).ok()? + usize::from(row),
            col: usize::from(col),
        })
    }

    fn caret_visible(&self) -> bool {
        !self.state.modes().hide_cursor
    }

    fn screen_pos(&self, at: CellPos) -> Option<Position> {
        let row = u64::try_from(at.line).ok()?.checked_sub(self.top_line())?;
        if row >= u64::from(self.state.size().0) || row >= u64::from(self.area.height) {
            return None;
        }
        let cols = self.state.size().1.min(self.grid(self.area).width);
        let col = u16::try_from(at.col).ok().filter(|&col| col < cols)?;
        Some(Position::new(
            self.area.x + col,
            self.area.y + u16::try_from(row).ok()?,
        ))
    }

    fn draw(
        &self,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
        caret_allowed: bool,
    ) -> Option<Position> {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return None;
        }
        let t = ctx.theme;
        let grid = self.grid(area);
        let focused = ctx.interaction.focused(self.id);
        self.scroll.note_policy(ctx.track_click, ctx.hscroll);
        ctx.control(self.id, area, false);
        ctx.scrollable(self.id, area);
        ctx.autoscroll(grid);
        if self.has_selection() {
            ctx.selection(self.id);
        }
        fill(buf, area, Style::new().bg(bg));

        let color = t.level != ColorLevel::Mono;
        let defaults = if color {
            Defaults {
                fg: t.text_primary,
                bg,
            }
        } else {
            TERMINAL_DEFAULTS
        };
        paint_view(&self.state.view(self.offset), grid, buf, color, defaults);

        if let Some((a, b)) = self.select.range() {
            let cols = self.state.size().1.min(grid.width);
            let top = self.top_line();
            for row in 0..grid.height.min(self.state.size().0) {
                let line = top + u64::from(row);
                if line < a.line || line > b.line {
                    continue;
                }
                let from = if line == a.line { a.col } else { 0 };
                let to = if line == b.line { b.col } else { cols };
                for col in from..to.min(cols) {
                    let cell = &mut buf[(grid.x + col, grid.y + row)];
                    cell.set_style(t.selection().add_modifier(cell.modifier));
                }
            }
        }
        if self.offset > 0 {
            termrock_runtime::fade::scroll_edges(buf, ctx, grid, &self.scroll.v);
        }

        if area.width > TRACK_COLUMNS {
            let track = Rect::new(area.right() - 1, area.y, 1, area.height);
            if self.scroll.v.overflows() {
                scrollbar::render_vertical(track, buf, ctx, self.id, &self.scroll.v, focused);
            } else {
                let rest = t.scrollbar_track();
                for y in track.top()..track.bottom() {
                    buf.set_string(track.x, y, t.glyphs.scroll_track, rest);
                }
            }
        }

        if caret_allowed && self.offset == 0 {
            self.state.cursor(grid)
        } else {
            None
        }
    }
}
