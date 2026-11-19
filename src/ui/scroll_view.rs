use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::widgets::Padding;

use crate::core::event::{Key, Outcome};
use crate::core::hit::{Axis, HitMark};
use crate::core::id::WidgetId;
use crate::core::keymap::{Actions, KeyMap};
use crate::core::pointer::PointerEvent;
use crate::core::response::Response;
use crate::core::scroll::{ScrollPane, Scrollable, ScrollbarVisibility, TrackClick};
use crate::ui::ctx::{RenderCtx, Shift, copy_cells, scratch_buffer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Axes {
    pub vertical: bool,
    pub horizontal: bool,
}

impl Axes {
    pub const BOTH: Self = Self {
        vertical: true,
        horizontal: true,
    };

    pub const VERTICAL: Self = Self {
        vertical: true,
        horizontal: false,
    };

    pub const HORIZONTAL: Self = Self {
        vertical: false,
        horizontal: true,
    };
}

impl Default for Axes {
    fn default() -> Self {
        Self::BOTH
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollCmd {
    PageUp,

    PageDown,

    PageLeft,

    PageRight,

    /// Resets both axes to the origin when either one overflows.
    Top,

    /// Moves the vertical axis to its end. The horizontal offset is kept.
    Bottom,
}

impl Actions for ScrollCmd {
    fn default_keymap() -> KeyMap<Self> {
        use crate::core::keymap::KeyBinding;
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};
        let none = KeyModifiers::NONE;
        let alt = KeyModifiers::ALT;
        vec![
            (KeyBinding::new(KeyCode::PageUp, none), ScrollCmd::PageUp),
            (
                KeyBinding::new(KeyCode::PageDown, none),
                ScrollCmd::PageDown,
            ),
            (KeyBinding::new(KeyCode::PageUp, alt), ScrollCmd::PageLeft),
            (
                KeyBinding::new(KeyCode::PageDown, alt),
                ScrollCmd::PageRight,
            ),
            (KeyBinding::new(KeyCode::Home, none), ScrollCmd::Top),
            (KeyBinding::new(KeyCode::End, none), ScrollCmd::Bottom),
        ]
    }

    fn label(self) -> &'static str {
        match self {
            ScrollCmd::PageUp => "Scroll up a page",
            ScrollCmd::PageDown => "Scroll down a page",
            ScrollCmd::PageLeft => "Scroll left a page",
            ScrollCmd::PageRight => "Scroll right a page",
            ScrollCmd::Top => "Scroll to the top",
            ScrollCmd::Bottom => "Scroll to the bottom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewLayout {
    pub viewport: Rect,

    pub content: Rect,

    pub inner: Rect,

    pub v_track: Option<Rect>,

    pub h_track: Option<Rect>,

    pub corner: Option<Rect>,
}

impl ViewLayout {
    pub fn overflows(&self) -> bool {
        self.content.width > self.viewport.width || self.content.height > self.viewport.height
    }
}

/// Scratch-buffer work of the latest frame, in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollWork {
    /// Cells of scratch storage allocated.
    pub allocated: usize,
    /// Cells written to initialise that storage.
    pub filled: usize,
    /// Cells copied between the screen buffer and scratch storage.
    pub copied: usize,
}

/// The slice of a padded content canvas that [`ScrollView::render_window`] shows.
///
/// `logical` is in padded-canvas coordinates: model coordinate (0, 0) sits at
/// `content_origin`. `viewport` and `screen` are in terminal coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleWindow {
    pub logical: Rect,
    pub viewport: Rect,
    pub screen: Rect,
    pub content_origin: Position,
    content_rows: (u16, u16),
}

impl VisibleWindow {
    fn empty(screen: Rect) -> Self {
        Self {
            logical: Rect::ZERO,
            viewport: Rect::ZERO,
            screen,
            content_origin: Position::ORIGIN,
            content_rows: (0, 0),
        }
    }

    /// Maps a padded-canvas rectangle to the screen, clipped to the window.
    pub fn project_rect(self, rect: Rect) -> Rect {
        let part = rect.intersection(self.logical);
        if part.is_empty() {
            return Rect::ZERO;
        }
        Rect::new(
            self.viewport.x + (part.x - self.logical.x),
            self.viewport.y + (part.y - self.logical.y),
            part.width,
            part.height,
        )
    }

    /// Maps a padded-canvas point to the screen, or `None` outside the window.
    pub fn project_point(self, point: Position) -> Option<Position> {
        self.logical.contains(point).then(|| {
            Position::new(
                self.viewport.x + (point.x - self.logical.x),
                self.viewport.y + (point.y - self.logical.y),
            )
        })
    }

    /// Maps a model-relative rectangle (origin at the content, not the padding).
    pub fn project_content_rect(self, rect: Rect) -> Rect {
        let moved = Rect::new(
            rect.x.saturating_add(self.content_origin.x),
            rect.y.saturating_add(self.content_origin.y),
            rect.width,
            rect.height,
        );
        self.project_rect(moved)
    }

    /// Visible padded-canvas rows.
    pub fn rows(self) -> std::ops::Range<usize> {
        usize::from(self.logical.y)..usize::from(self.logical.bottom())
    }

    /// Visible model rows, without the padding rows.
    pub fn content_rows(self) -> std::ops::Range<usize> {
        let (top, len) = self.content_rows;
        let rows = self.rows();
        let (first, end) = (usize::from(top), usize::from(top) + usize::from(len));
        let start = rows.start.max(first);
        let stop = rows.end.min(end);
        if start >= stop {
            return 0..0;
        }
        (start - first)..(stop - first)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Followed {
    focus: Option<WidgetId>,
    target: Rect,
}

#[derive(Debug, Clone)]
pub struct ScrollView {
    pub id: WidgetId,

    pub pane: ScrollPane,

    pub axes: Axes,

    pub bars: ScrollbarVisibility,

    pub padding: Padding,

    pub scrollbar_gap: u16,

    pub keys: KeyMap<ScrollCmd>,

    pub follow_focus: bool,

    pub context: u16,
    followed: Option<Followed>,
    layout: ViewLayout,
    work: ScrollWork,
}

impl ScrollView {
    pub fn new(id: WidgetId) -> Self {
        Self {
            id,
            pane: ScrollPane::default(),
            axes: Axes::BOTH,
            bars: ScrollbarVisibility::Auto,
            padding: Padding::ZERO,
            scrollbar_gap: crate::widgets::scrollbar::GAP,
            keys: ScrollCmd::default_keymap(),
            follow_focus: true,
            context: 1,
            followed: None,
            layout: ViewLayout::default(),
            work: ScrollWork::default(),
        }
    }

    pub fn axes(mut self, axes: Axes) -> Self {
        self.axes = axes;
        self
    }

    pub fn bars(mut self, bars: ScrollbarVisibility) -> Self {
        self.bars = bars;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn scrollbar_gap(mut self, cols: u16) -> Self {
        self.scrollbar_gap = cols;
        self
    }

    pub fn keys(mut self, keys: KeyMap<ScrollCmd>) -> Self {
        self.keys = keys;
        self
    }

    pub fn follow_focus(mut self, on: bool) -> Self {
        self.follow_focus = on;
        self
    }

    pub fn context(mut self, cells: u16) -> Self {
        self.context = cells;
        self
    }

    pub fn track_click(mut self, click: TrackClick) -> Self {
        self.pane = self.pane.track_click(click);
        self.pane.h_cfg.track_click = click;
        self
    }

    pub fn last_layout(&self) -> ViewLayout {
        self.layout
    }

    pub fn last_work(&self) -> ScrollWork {
        self.work
    }

    pub fn offset(&self) -> (usize, usize) {
        (self.pane.h.offset, self.pane.v.offset)
    }

    pub fn scroll_to(&mut self, x: usize, y: usize) -> bool {
        let h = self.pane.h.scroll_to(x);
        let v = self.pane.v.scroll_to(y);
        h || v
    }

    pub fn reset(&mut self) {
        self.pane.v.offset = 0;
        self.pane.h.offset = 0;
        self.followed = None;
    }

    pub fn fit(&self, area: Size, measure: impl Fn(Size) -> Size) -> Size {
        let size = measure(area);
        let pad = self.padding;
        let (pad_w, pad_h) = (
            pad.left.saturating_add(pad.right),
            pad.top.saturating_add(pad.bottom),
        );
        let room = 1u16.saturating_add(self.scrollbar_gap);
        let bar = self.axes.vertical
            && self.bars != ScrollbarVisibility::Never
            && area.width > room
            && (self.bars == ScrollbarVisibility::Always
                || size.height.saturating_add(pad_h) > area.height);
        if bar && size.width.saturating_add(pad_w) <= area.width {
            let narrower = measure(Size::new(area.width - room, area.height));
            if narrower.width.saturating_add(pad_w) <= area.width - room {
                return narrower;
            }
        }
        size
    }

    pub fn layout(&self, area: Rect, content: Size) -> ViewLayout {
        let pad = self.padding;
        let need_w = content
            .width
            .saturating_add(pad.left)
            .saturating_add(pad.right);
        let need_h = content
            .height
            .saturating_add(pad.top)
            .saturating_add(pad.bottom);
        let shows = self.bars != ScrollbarVisibility::Never;
        let always = self.bars == ScrollbarVisibility::Always;
        let v_room = 1u16.saturating_add(self.scrollbar_gap);
        let (mut v, mut h) = (false, false);
        for _ in 0..3 {
            let vw = area.width.saturating_sub(if v { v_room } else { 0 });
            let vh = area.height.saturating_sub(u16::from(h));
            let nv = self.axes.vertical && shows && (always || need_h > vh);
            let nh = self.axes.horizontal && shows && (always || need_w > vw);
            if (nv, nh) == (v, h) {
                break;
            }
            (v, h) = (nv, nh);
        }

        let v = v && area.width > v_room;
        let h = h && area.height > 1;
        let vw = area.width.saturating_sub(if v { v_room } else { 0 });
        let vh = area.height.saturating_sub(u16::from(h));
        let viewport = Rect::new(area.x, area.y, vw, vh);
        let cw = if self.axes.horizontal {
            need_w.max(vw)
        } else {
            vw
        };
        let ch = if self.axes.vertical {
            need_h.max(vh)
        } else {
            vh
        };
        let content = Rect::new(area.x, area.y, cw, ch);
        let inner = Rect::new(
            content.x.saturating_add(pad.left),
            content.y.saturating_add(pad.top),
            cw.saturating_sub(pad.left.saturating_add(pad.right)),
            ch.saturating_sub(pad.top.saturating_add(pad.bottom)),
        );
        let v_track = v.then(|| Rect::new(area.right() - 1, area.y, 1, vh));
        let h_track = h.then(|| Rect::new(area.x, area.bottom() - 1, area.width - u16::from(v), 1));
        let corner = (v && h).then(|| Rect::new(area.right() - 1, area.bottom() - 1, 1, 1));
        ViewLayout {
            viewport,
            content,
            inner,
            v_track,
            h_track,
            corner,
        }
    }

    /// Legacy arbitrary-composition path. When the content overflows, the
    /// callback paints a canvas as large as the content, so its cost grows
    /// with the content size; use [`ScrollView::render_window`] for rows a
    /// model can supply on demand.
    pub fn render(
        &mut self,
        area: Rect,
        content: Size,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        draw: impl FnOnce(Rect, &mut Buffer, &mut RenderCtx),
    ) {
        let mut frame = self.begin(area, content, buf, ctx);
        let inner = frame.area();
        draw(inner, frame.buffer(buf), ctx);
        self.end(frame, buf, ctx);
    }

    /// Paints only the visible window of a padded content canvas.
    ///
    /// `screen` is the root terminal rectangle, passed unchanged through nested
    /// windows. `area` is clipped to `buf.area` and `screen`. `content` is the
    /// unpadded model extent and `focus_target` a padded-canvas rectangle to
    /// reveal before painting. The callback runs once with the final window,
    /// paints in screen coordinates into scratch storage the size of the
    /// viewport, and its hits, cursor and cuts are clipped to the viewport.
    /// Floating content is rendered by the host afterwards against the real
    /// buffer, using the returned `window.screen`.
    #[allow(clippy::too_many_arguments)]
    pub fn render_window(
        &mut self,
        screen: Rect,
        area: Rect,
        content: Size,
        focus_target: Option<Rect>,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        draw: impl FnOnce(VisibleWindow, &mut Buffer, &mut RenderCtx),
    ) -> VisibleWindow {
        self.work = ScrollWork::default();
        let area = area.intersection(buf.area).intersection(screen);
        let l = self.prepare(area, content, ctx);
        if self.follow_focus {
            self.reveal_target(l, focus_target, ctx.interaction.focus);
        }
        let mut window = VisibleWindow::empty(screen);
        if !l.viewport.is_empty() {
            let canvas = Rect::new(0, 0, l.content.width, l.content.height);
            let offset = (self.pane.h.offset as u16, self.pane.v.offset as u16);
            window = VisibleWindow {
                logical: Rect::new(offset.0, offset.1, l.viewport.width, l.viewport.height)
                    .intersection(canvas),
                viewport: l.viewport,
                screen,
                content_origin: Position::new(self.padding.left, self.padding.top),
                content_rows: (self.padding.top, content.height),
            };
            self.paint_window(window, l.overflows(), buf, ctx, draw);
        }
        self.draw_tracks(l, buf, ctx);
        window
    }

    fn paint_window(
        &mut self,
        window: VisibleWindow,
        overflows: bool,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        draw: impl FnOnce(VisibleWindow, &mut Buffer, &mut RenderCtx),
    ) {
        let viewport = window.viewport;
        let mut blank = Cell::default();
        if let Some(c) = buf.cell(viewport.as_position()) {
            blank.set_style(c.style());
        }
        let mut scratch = Buffer::filled(viewport, blank);
        self.work.allocated = scratch.content().len();
        self.work.filled = scratch.content().len();
        crate::record_work!(AllocatedBufferCells, self.work.allocated);
        crate::record_work!(BufferCellsFilled, self.work.filled);
        if overflows {
            ctx.scrollable(self.id, viewport);
        }
        let mark = ctx.hits.mark();
        let cuts = crate::ui::reveal::cuts_len();
        let cursor = ctx.cursor;
        draw(window, &mut scratch, ctx);
        ctx.hits.clip_mark(mark, viewport);
        crate::ui::reveal::clip_cuts_since(cuts, viewport);
        if let Some(p) = ctx.cursor
            && ctx.cursor != cursor
            && !viewport.contains(p)
        {
            ctx.cursor = cursor;
        }
        self.work.copied = copy_cells(&scratch, viewport, buf, viewport.as_position());
    }

    fn reveal_target(&mut self, l: ViewLayout, target: Option<Rect>, focus: Option<WidgetId>) {
        let canvas = Rect::new(0, 0, l.content.width, l.content.height);
        let Some(target) = target
            .map(|t| t.intersection(canvas))
            .filter(|t| !t.is_empty())
        else {
            self.followed = None;
            return;
        };
        let now = Followed { focus, target };
        if self.followed == Some(now) {
            return;
        }
        self.followed = Some(now);
        self.pane
            .v
            .reveal_span(usize::from(target.y), usize::from(target.height));
        self.pane
            .h
            .reveal_span(usize::from(target.x), usize::from(target.width));
    }

    fn prepare(&mut self, area: Rect, content: Size, ctx: &RenderCtx) -> ViewLayout {
        let l = self.layout(area, content);
        self.layout = l;
        let p = &mut self.pane;
        p.begin(ctx.track_click);
        p.h_cfg.enabled = self.axes.horizontal;
        p.h_cfg.bar = self.bars;
        p.v.content_len = l.content.height as usize;
        p.v.set_viewport(l.viewport.height as usize);
        p.h.content_len = l.content.width as usize;
        p.h.set_viewport(l.viewport.width as usize);
        p.v.context = self.context as usize;
        p.h.context = self.context as usize;
        if !l.overflows() {
            p.v.offset = 0;
            p.h.offset = 0;
        }
        l
    }

    /// Legacy arbitrary-composition path; see [`ScrollView::render`] for its
    /// full-canvas cost.
    pub fn begin(
        &mut self,
        area: Rect,
        content: Size,
        buf: &Buffer,
        ctx: &mut RenderCtx,
    ) -> ScrollFrame {
        self.work = ScrollWork::default();
        let l = self.prepare(area, content, ctx);
        if !l.overflows() {
            return ScrollFrame {
                layout: l,
                off: None,
                mark: ctx.hits.mark(),
                cuts: 0,
                cursor: ctx.cursor,
                shift: None,
            };
        }

        ctx.scrollable(self.id, l.viewport);
        let shift = Shift::default();
        ctx.push_shift(shift.clone());

        let whole = buf.area.union(l.content);
        let mut off = scratch_buffer(whole);
        self.work.allocated = off.content().len();
        self.work.copied += copy_cells(buf, buf.area, &mut off, buf.area.as_position());
        let mut blank = Cell::default();
        if let Some(c) = buf.cell(l.viewport.as_position()) {
            blank.set_style(c.style());
        }
        for y in l.content.top()..l.content.bottom() {
            for x in l.content.left()..l.content.right() {
                off[(x, y)] = blank.clone();
                self.work.filled += 1;
                crate::record_work!(BufferCellsFilled, 1usize);
            }
        }
        ScrollFrame {
            layout: l,
            off: Some(off),
            mark: ctx.hits.mark(),
            cuts: crate::ui::reveal::cuts_len(),
            cursor: ctx.cursor,
            shift: Some(shift),
        }
    }

    pub fn end(&mut self, frame: ScrollFrame, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let l = frame.layout;
        if let (Some(off), Some(shift)) = (frame.off, frame.shift) {
            ctx.pop_shift();
            if self.follow_focus {
                self.follow(&off, l, frame.mark, frame.cursor, ctx);
            }
            let (ox, oy) = (self.pane.h.offset as u16, self.pane.v.offset as u16);
            let window = Rect::new(
                l.viewport.x + ox,
                l.viewport.y + oy,
                l.viewport.width,
                l.viewport.height,
            );
            let to = l.viewport.as_position();

            let screen = buf.area;
            for y in screen.top()..screen.bottom() {
                for x in screen.left()..screen.right() {
                    let p = Position::new(x, y);
                    if !l.content.contains(p) {
                        buf[(x, y)] = off[(x, y)].clone();
                        self.work.copied += 1;
                        crate::record_work!(BufferCellsCopied, 1usize);
                    }
                }
            }
            self.work.copied += copy_cells(&off, window, buf, to);
            ctx.hits.scroll_since(frame.mark, l.content, window, to);
            crate::ui::reveal::scroll_cuts_since(frame.cuts, l.content, window, to);
            if ctx.cursor != frame.cursor {
                ctx.cursor = match ctx.cursor {
                    Some(p) if window.contains(p) => Some(Position::new(p.x - ox, p.y - oy)),
                    Some(p) if !l.content.contains(p) => Some(p),
                    _ => frame.cursor,
                };
            }
            shift.set((ox, oy));
        }
        self.draw_tracks(l, buf, ctx);
    }

    fn draw_tracks(&mut self, l: ViewLayout, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let id = self.id;
        match l.v_track {
            Some(track) => self.pane.draw_v(track, buf, ctx, id, false),
            None => self.pane.clear_track(Axis::Vertical),
        }
        match l.h_track {
            Some(track) => self.pane.draw_h(track, buf, ctx, id, false),
            None => self.pane.clear_track(Axis::Horizontal),
        }
        if let Some(c) = l.corner
            && let Some(cell) = buf.cell_mut(c.as_position())
        {
            cell.set_symbol(" ");
        }
    }

    fn follow(
        &mut self,
        off: &Buffer,
        l: ViewLayout,
        mark: HitMark,
        cursor_before: Option<Position>,
        ctx: &RenderCtx,
    ) {
        let focus = ctx.interaction.focus;
        let control = focus.and_then(|f| {
            ctx.hits.regions()[mark_regions(ctx, mark)..]
                .iter()
                .rev()
                .find(|r| r.id == f && r.area.intersects(l.content))
                .map(|r| r.area.intersection(l.content))
        });
        let caret = ctx
            .cursor
            .filter(|p| ctx.cursor != cursor_before && l.content.contains(*p))
            .map(|p| Rect::new(p.x, p.y, 1, 1));
        let fits = |r: Rect| r.width <= l.viewport.width && r.height <= l.viewport.height;
        let target = match (caret, control) {
            (Some(c), _) => Some(c),
            (None, Some(r)) if fits(r) => Some(r),

            (None, Some(r)) => Some(gutter(off, r, ctx).unwrap_or(Rect::new(r.x, r.y, 1, 1))),
            (None, None) => None,
        };
        let Some(target) = target else {
            self.followed = None;
            return;
        };
        let now = Followed { focus, target };
        if self.followed == Some(now) {
            return;
        }
        self.followed = Some(now);
        self.pane
            .v
            .reveal_span((target.y - l.content.y) as usize, target.height as usize);
        self.pane
            .h
            .reveal_span((target.x - l.content.x) as usize, target.width as usize);
    }

    pub fn on_key(&mut self, key: &Key) -> Option<Outcome> {
        let cmd = crate::core::keymap::lookup(&self.keys, key)?;
        let (v, h) = (&mut self.pane.v, &mut self.pane.h);
        let moved = match cmd {
            ScrollCmd::PageUp | ScrollCmd::PageDown | ScrollCmd::Bottom if !v.overflows() => {
                return None;
            }
            ScrollCmd::Top if !v.overflows() && !h.overflows() => return None,
            ScrollCmd::PageLeft | ScrollCmd::PageRight if !h.overflows() => return None,
            ScrollCmd::PageUp => v.page_up(),
            ScrollCmd::PageDown => v.page_down(),
            ScrollCmd::PageLeft => h.page_up(),
            ScrollCmd::PageRight => h.page_down(),
            ScrollCmd::Top => v.jump_start() | h.jump_start(),
            ScrollCmd::Bottom => v.jump_end(),
        };
        Some(if moved {
            Outcome::Changed
        } else {
            Outcome::Consumed
        })
    }

    pub fn pointer(&mut self, ev: &PointerEvent) -> Option<Response<()>> {
        if !ev.on(self.id) {
            return None;
        }
        crate::core::pointer::scroll_input(self, ev)
    }
}

fn mark_regions(ctx: &RenderCtx, mark: HitMark) -> usize {
    mark.regions_len().min(ctx.hits.regions().len())
}

fn gutter(off: &Buffer, content: Rect, ctx: &RenderCtx) -> Option<Rect> {
    let glyph = ctx.theme.glyphs.focus_bar;
    let area = content.intersection(off.area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if off[(x, y)].symbol() == glyph {
                let mut h = 1;
                while y + h < area.bottom() && off[(x, y + h)].symbol() == glyph {
                    h += 1;
                }
                return Some(Rect::new(x, y, 1, h));
            }
        }
    }
    None
}

impl Scrollable for ScrollView {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.pane
    }
}

#[derive(Debug)]
pub struct ScrollFrame {
    layout: ViewLayout,

    off: Option<Buffer>,
    mark: HitMark,
    cuts: usize,
    cursor: Option<Position>,
    shift: Option<Shift>,
}

impl ScrollFrame {
    pub fn area(&self) -> Rect {
        self.layout.inner
    }

    pub fn layout(&self) -> ViewLayout {
        self.layout
    }

    pub fn buffer<'a>(&'a mut self, buf: &'a mut Buffer) -> &'a mut Buffer {
        match self.off.as_mut() {
            Some(off) => off,
            None => buf,
        }
    }
}

#[cfg(test)]
mod window_tests;
