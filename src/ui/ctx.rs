use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

use crate::core::focus::FocusRing;
use crate::core::hit::HitRegistry;
use crate::core::id::WidgetId;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, Default)]
pub struct Interaction {
    pub focus: Option<WidgetId>,
    pub hover: Option<WidgetId>,
    pub pressed: Option<WidgetId>,

    pub flash: Option<WidgetId>,

    pub focus_hidden: bool,

    pub hover_suppressed: bool,

    pub pointer_focus: bool,
    pub tick: u64,

    pub motion_unreported: bool,

    pub hover_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusVisible {
    #[default]
    Always,

    KeyboardOnly,
}

impl Interaction {
    pub fn focused(&self, id: WidgetId) -> bool {
        !self.focus_hidden && self.has_focus(id)
    }

    pub fn focused_under(&self, id: WidgetId, layer_open: bool) -> bool {
        !layer_open && self.focused(id)
    }

    pub fn shown_focus(&self) -> Option<WidgetId> {
        self.focus.filter(|_| !self.focus_hidden)
    }

    pub fn has_focus(&self, id: WidgetId) -> bool {
        self.focus == Some(id)
    }

    pub fn live_hover(&self) -> Option<WidgetId> {
        self.hover.filter(|_| !self.hover_suppressed)
    }

    pub fn focus_visible(&self, id: WidgetId, mode: FocusVisible) -> bool {
        self.focused(id) && (mode == FocusVisible::Always || !self.pointer_focus)
    }
    pub fn hovered(&self, id: WidgetId) -> bool {
        !self.hover_suppressed && self.hover == Some(id)
    }
    pub fn pressed(&self, id: WidgetId) -> bool {
        (self.pressed == Some(id) && self.hover == Some(id)) || self.flash == Some(id)
    }

    pub fn pressed_any(&self, ids: impl IntoIterator<Item = WidgetId>) -> bool {
        ids.into_iter().any(|id| self.pressed(id))
    }

    pub fn hovered_any(&self, ids: impl IntoIterator<Item = WidgetId>) -> bool {
        ids.into_iter().any(|id| self.hovered(id))
    }

    pub fn flash_within(&self, container: WidgetId) -> bool {
        self.flash == Some(container)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VisualState {
    pub focused: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub selected: bool,
    pub disabled: bool,
    pub fixed: bool,
    pub error: bool,
    pub editing: bool,
    pub busy: bool,

    pub backdrop: bool,

    pub inactive_selection: Option<crate::theme::InactiveSelection>,
}

type OverlayFn = Box<dyn for<'r> FnOnce(&mut Buffer, &mut RenderCtx<'r>)>;

pub struct RenderCtx<'a> {
    pub theme: &'a Theme,
    pub interaction: Interaction,
    pub hits: &'a mut HitRegistry,
    pub ring: &'a mut FocusRing,

    pub cursor: Option<Position>,

    pub inert: bool,

    overlays: Vec<(bool, OverlayFn)>,

    pub reveal: crate::ui::reveal::Reveal,

    pub tooltips: crate::ui::tooltip::Tooltips,

    pub hscroll: crate::core::scroll::HScroll,

    pub track_click: crate::core::scroll::TrackClick,

    pub(crate) reveal_slot: crate::ui::reveal::Slot,

    pub reveal_hint: Option<(&'static str, &'static str)>,

    pub peek: Option<String>,

    pub(crate) shifts: Vec<Shift>,
}

pub(crate) type Shift = std::rc::Rc<std::cell::Cell<(u16, u16)>>;

pub(crate) fn total_shift(shifts: &[Shift]) -> (u16, u16) {
    shifts.iter().fold((0, 0), |(x, y), s| {
        let (a, b) = s.get();
        (x.saturating_add(a), y.saturating_add(b))
    })
}

fn shifted(shifts: Vec<Shift>, draw: OverlayFn) -> OverlayFn {
    Box::new(move |buf: &mut Buffer, ctx: &mut RenderCtx| {
        let (dx, dy) = total_shift(&shifts);
        if dx == 0 && dy == 0 {
            draw(buf, ctx);
            return;
        }
        let screen = buf.area;
        let view = Rect::new(
            screen.x.saturating_add(dx),
            screen.y.saturating_add(dy),
            screen.width,
            screen.height,
        );
        let mut off = scratch_buffer(view);
        copy_cells(buf, screen, &mut off, view.as_position());
        let mark = ctx.hits.mark();
        let cursor = ctx.cursor;
        let queued = ctx.overlays.len();
        draw(&mut off, ctx);
        copy_cells(&off, view, buf, screen.as_position());
        ctx.hits
            .scroll_since(mark, view, view, screen.as_position());
        if ctx.cursor != cursor {
            ctx.cursor = ctx
                .cursor
                .and_then(|p| view.contains(p).then(|| Position::new(p.x - dx, p.y - dy)));
        }

        for item in ctx.overlays.iter_mut().skip(queued) {
            let f = std::mem::replace(&mut item.1, Box::new(|_, _| {}));
            item.1 = shifted(shifts.clone(), f);
        }
    })
}

/// An off-screen buffer. Every production scratch allocation goes through here
/// so `AllocatedBufferCells` counts it.
pub(crate) fn scratch_buffer(area: Rect) -> Buffer {
    crate::record_work!(AllocatedBufferCells, area.area());
    Buffer::empty(area)
}

pub(crate) fn copy_cells(from: &Buffer, area: Rect, to: &mut Buffer, at: Position) {
    let area = area.intersection(from.area);
    for y in 0..area.height {
        for x in 0..area.width {
            let (tx, ty) = (at.x as u32 + x as u32, at.y as u32 + y as u32);
            if tx > u16::MAX as u32 || ty > u16::MAX as u32 {
                continue;
            }
            let p = Position::new(tx as u16, ty as u16);
            if to.area.contains(p) {
                to[(p.x, p.y)] = from[(area.x + x, area.y + y)].clone();
                crate::record_work!(BufferCellsCopied, 1usize);
            }
        }
    }
}

impl<'a> RenderCtx<'a> {
    pub fn new(
        theme: &'a Theme,
        interaction: Interaction,
        hits: &'a mut HitRegistry,
        ring: &'a mut FocusRing,
    ) -> Self {
        Self {
            theme,
            interaction,
            hits,
            ring,
            cursor: None,
            inert: false,
            overlays: Vec::new(),
            reveal: crate::ui::reveal::Reveal::default(),
            tooltips: crate::ui::tooltip::Tooltips::default(),
            hscroll: crate::core::scroll::HScroll::default(),
            track_click: crate::core::scroll::TrackClick::Page,
            reveal_hint: None,
            peek: None,
            shifts: Vec::new(),
            reveal_slot: {
                crate::ui::reveal::clear_cuts();
                crate::ui::reveal::begin_frame();
                crate::ui::tooltip::begin_frame();
                crate::ui::reveal::Slot::default()
            },
        }
    }

    pub(crate) fn detached(
        theme: &'a Theme,
        interaction: Interaction,
        hits: &'a mut HitRegistry,
        ring: &'a mut FocusRing,
    ) -> Self {
        Self {
            theme,
            interaction,
            hits,
            ring,
            cursor: None,
            inert: true,
            overlays: Vec::new(),
            reveal: crate::ui::reveal::Reveal::OFF,
            tooltips: crate::ui::tooltip::Tooltips::OFF,
            hscroll: crate::core::scroll::HScroll::default(),
            track_click: crate::core::scroll::TrackClick::Page,
            reveal_hint: None,
            peek: None,
            shifts: Vec::new(),
            reveal_slot: crate::ui::reveal::Slot::default(),
        }
    }

    pub fn control(&mut self, id: WidgetId, area: Rect, disabled: bool) {
        if self.inert {
            return;
        }
        self.hits.register(id, area);
        if !disabled {
            self.ring.register(id);
        }
    }

    pub fn focus_stop(&mut self, id: WidgetId) {
        if !self.inert {
            self.ring.register(id);
        }
    }

    pub fn selection(&mut self, id: WidgetId) {
        if !self.inert {
            self.ring.mark_selection(id);
        }
    }

    pub fn clickable(&mut self, id: WidgetId, area: Rect) {
        if !self.inert {
            self.hits.register(id, area);
        }
    }

    pub fn register_part(
        &mut self,
        owner: WidgetId,
        part: crate::core::id::PartRef,
        area: Rect,
    ) -> WidgetId {
        let id = owner.part(part);
        self.register_part_as(id, owner, part, area);
        id
    }

    pub fn register_part_as(
        &mut self,
        id: WidgetId,
        owner: WidgetId,
        part: crate::core::id::PartRef,
        area: Rect,
    ) {
        if !self.inert {
            self.hits.register_part_as(id, owner, part, area);
        }
    }

    pub fn pointer_shape(&mut self, id: WidgetId, shape: crate::core::shape::PointerShape) {
        if !self.inert {
            self.hits.set_shape(id, shape);
        }
    }

    pub fn clickable_under(&mut self, id: WidgetId, area: Rect) {
        if !self.inert {
            self.hits.register_under(id, area);
        }
    }

    pub fn scrollbar(
        &mut self,
        id: WidgetId,
        track: Rect,
        owner: WidgetId,
        axis: crate::core::hit::Axis,
    ) {
        if !self.inert {
            self.hits.register_bar(crate::core::hit::Bar {
                id,
                owner,
                axis,
                track,
            });
        }
    }

    pub fn scrollbar_thumb(&mut self, id: WidgetId, thumb: Rect, owner: WidgetId) {
        if !self.inert {
            self.hits.register_thumb(id, owner, thumb);
        }
    }

    pub fn scrollable(&mut self, id: WidgetId, area: Rect) {
        if !self.inert {
            self.hits.register_scroll(id, area);
        }
    }

    pub fn scrollable_under(&mut self, id: WidgetId, area: Rect) {
        if !self.inert {
            self.hits.register_scroll_under(id, area);
        }
    }

    pub fn autoscroll(&mut self, area: Rect) {
        if !self.inert {
            self.hits.register_autoscroll(area);
        }
    }

    pub fn state(&self, id: WidgetId) -> VisualState {
        VisualState {
            focused: self.interaction.focused(id),
            hovered: self.interaction.hovered(id),
            pressed: self.interaction.pressed(id),
            backdrop: self.interaction.focus_hidden,
            ..Default::default()
        }
    }

    pub fn chosen(&self, focused: bool) -> ratatui::style::Color {
        self.theme.chosen_in(VisualState {
            focused,
            backdrop: self.interaction.focus_hidden,
            ..Default::default()
        })
    }

    pub fn child_state(&self, container: WidgetId, child: WidgetId, cursor: bool) -> VisualState {
        let mut s = self.state(child);
        s.pressed |= cursor && self.interaction.flash_within(container);
        s
    }

    pub fn title_style(&self, id: WidgetId, bg: ratatui::style::Color) -> ratatui::style::Style {
        let t = self.theme;
        let mut s = self.state(id);
        s.focused = false;
        t.interactive(crate::theme::Kind::Title, s, t.plane_of(bg))
            .style
    }

    pub fn set_cursor(&mut self, pos: Position) {
        if !self.inert {
            self.cursor = Some(pos);
        }
    }

    pub fn overlay(
        &mut self,
        draw: impl for<'r> FnOnce(&mut Buffer, &mut RenderCtx<'r>) + 'static,
    ) {
        let draw: OverlayFn = Box::new(draw);
        let draw = if self.shifts.is_empty() {
            draw
        } else {
            shifted(self.shifts.clone(), draw)
        };
        self.overlays.push((self.inert, draw));
    }

    pub(crate) fn overlay_unshifted(
        &mut self,
        draw: impl for<'r> FnOnce(&mut Buffer, &mut RenderCtx<'r>) + 'static,
    ) {
        self.overlays.push((self.inert, Box::new(draw)));
    }

    pub fn clip<R>(
        &mut self,
        buf: &mut Buffer,
        area: Rect,
        draw: impl FnOnce(&mut Buffer, &mut Self) -> R,
    ) -> R {
        let area = area.intersection(buf.area);
        let before = buf.clone();
        let hits_from = self.hits.len();
        let cursor = self.cursor;
        let out = draw(buf, self);
        let full = buf.area;
        for y in full.top()..full.bottom() {
            for x in full.left()..full.right() {
                let p = Position::new(x, y);
                if !area.contains(p) {
                    buf[(x, y)] = before[(x, y)].clone();
                }
            }
        }
        self.hits.clip_since(hits_from, area);
        if let Some(p) = self.cursor
            && self.cursor != cursor
            && !area.contains(p)
        {
            self.cursor = cursor;
        }
        out
    }

    pub fn flush_overlays(&mut self, buf: &mut Buffer) {
        crate::ui::reveal::flush_cuts(self);
        while !self.overlays.is_empty() {
            let queued = std::mem::take(&mut self.overlays);
            for (inert, draw) in queued {
                let outer = std::mem::replace(&mut self.inert, inert);
                draw(buf, self);
                self.inert = outer;
            }
        }
    }

    pub fn page_under_layer(&mut self, open: bool) {
        self.interaction.focus_hidden = open;
    }

    pub fn begin_modal(&mut self, buf: &mut Buffer) {
        self.flush_overlays(buf);
        self.hits.push_barrier();
        self.ring.push_barrier();
        self.inert = false;
        self.interaction.focus_hidden = false;
    }

    pub fn visual_order<T>(&mut self, draw: impl FnOnce(&mut Self) -> T) -> T {
        let start = self.ring.len();
        let out = draw(self);
        let hits = &self.hits;
        self.ring.sort_from(start, |id| {
            hits.regions()
                .iter()
                .rev()
                .find(|r| r.id == *id)
                .map_or((u16::MAX, u16::MAX), |r| (r.area.y, r.area.x))
        });
        out
    }

    pub fn cover(&mut self, buf: &mut Buffer) {
        self.flush_overlays(buf);
        self.hits.push_barrier();
        self.ring.push_barrier();
    }

    pub fn modal_backdrop(&mut self, buf: &mut Buffer, screen: Rect) {
        self.modal_backdrop_with(buf, screen, Backdrop::DEFAULT);
    }

    pub fn modal_backdrop_with(&mut self, buf: &mut Buffer, screen: Rect, backdrop: Backdrop) {
        self.flush_overlays(buf);
        let t = self.theme;
        let top = backdrop.keep_top.min(screen.height);
        let dim = Rect::new(
            screen.x,
            screen.y + top,
            screen.width,
            screen
                .height
                .saturating_sub(top)
                .saturating_sub(backdrop.keep_bottom),
        );
        for pos in dim.positions() {
            if let Some(c) = buf.cell_mut(pos) {
                let bar = t.glyphs.focus_bar;
                let is_bar = c.symbol() == bar
                    && (!bar.is_ascii() || c.fg == t.focus || c.fg == t.text_on_accent);
                let st = t.backdrop(c.style());
                c.set_style(st);

                c.modifier = st.add_modifier;
                if is_bar {
                    c.set_symbol(" ");
                }
            }
        }
        self.begin_modal(buf);
    }
}

pub fn legible_on_fill(buf: &mut Buffer, area: Rect, t: &Theme, plane: ratatui::style::Color) {
    for pos in area.positions() {
        if let Some(c) = buf.cell_mut(pos)
            && c.bg != plane
            && !c.symbol().trim().is_empty()
            && !(c.symbol() == t.glyphs.focus_bar && c.fg == t.focus)
        {
            c.fg = t.legible(c.fg, c.bg);
        }
    }
}

pub fn fill(buf: &mut Buffer, area: Rect, style: ratatui::style::Style) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_symbol(" ");
                cell.set_style(style);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backdrop {
    pub keep_bottom: u16,

    pub keep_top: u16,
}

impl Backdrop {
    pub const DEFAULT: Self = Self {
        keep_bottom: 1,
        keep_top: 0,
    };

    pub const FULL: Self = Self {
        keep_bottom: 0,
        keep_top: 0,
    };

    pub const fn keep_bottom(mut self, rows: u16) -> Self {
        self.keep_bottom = rows;
        self
    }

    pub const fn keep_top(mut self, rows: u16) -> Self {
        self.keep_top = rows;
        self
    }
}

impl Default for Backdrop {
    fn default() -> Self {
        Self::DEFAULT
    }
}
