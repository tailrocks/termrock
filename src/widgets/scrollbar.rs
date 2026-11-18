use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;

use crate::core::event::Outcome;
use crate::core::id::WidgetId;
use crate::core::pointer::{BarPhase, PointerEvent};
use crate::core::scroll::{ScrollPaint, ScrollState, TrackClick};
use crate::theme::{Glyphs, Kind, Plane};
use crate::ui::ctx::{RenderCtx, VisualState};

pub const TRACK: &str = Glyphs::UNICODE.scroll_track;
pub const THUMB: &str = Glyphs::UNICODE.scroll_thumb;

pub const H_TRACK: &str = Glyphs::UNICODE.scroll_h_track;
pub const H_THUMB: &str = Glyphs::UNICODE.scroll_h_thumb;

pub fn id_for(container: WidgetId) -> WidgetId {
    container.sub("scrollbar")
}

pub fn h_id_for(container: WidgetId) -> WidgetId {
    container.sub("hscrollbar")
}

pub fn is_bar_of(id: WidgetId, container: WidgetId) -> bool {
    id == id_for(container) || id == h_id_for(container)
}

fn thumb_style(ctx: &RenderCtx, sb_id: WidgetId) -> Style {
    let s = VisualState {
        hovered: ctx.interaction.hovered(sb_id),
        pressed: ctx.interaction.pressed == Some(sb_id),
        ..Default::default()
    };
    ctx.theme.interactive(Kind::Handle, s, Plane::Canvas).style
}

pub fn render_divider(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    container: WidgetId,
    scroll: &ScrollState,
    focused: bool,
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return;
    }
    if scroll.overflows() {
        render_vertical(area, buf, ctx, container, scroll, focused);
        return;
    }
    let line = ctx.theme.border(false);
    let g = ctx.theme.glyphs;
    for y in area.top()..area.bottom() {
        buf.set_string(area.x, y, g.divider, line);
    }
}

pub fn render_vertical(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    container: WidgetId,
    scroll: &ScrollState,
    _focused: bool,
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() || !scroll.overflows() {
        return;
    }
    let sb_id = id_for(container);
    let thumb = thumb_style(ctx, sb_id);
    let track = ctx.theme.scrollbar_track();
    let g = ctx.theme.glyphs;
    let track_len = area.height as usize;
    let (start, len) = scroll.thumb(track_len);
    for i in 0..track_len {
        let y = area.y + i as u16;
        if i >= start && i < start + len {
            buf.set_string(area.x, y, g.scroll_thumb, thumb);
        } else {
            buf.set_string(area.x, y, g.scroll_track, track);
        }
    }
    ctx.scrollbar(sb_id, area, container, crate::core::hit::Axis::Vertical);
    let thumb_rect = Rect::new(area.x, area.y + start as u16, 1, len as u16);
    ctx.scrollbar_thumb(sb_id, thumb_rect, container);
}

pub fn render_horizontal(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    container: WidgetId,
    scroll: &ScrollState,
    focused: bool,
) {
    render_horizontal_with(area, buf, ctx, container, scroll, focused, false);
}

pub fn render_horizontal_with(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    container: WidgetId,
    scroll: &ScrollState,
    _focused: bool,
    always: bool,
) {
    let area = area.intersection(*buf.area());
    if area.is_empty() || !(scroll.overflows() || always) {
        return;
    }
    let sb_id = h_id_for(container);
    let thumb = thumb_style(ctx, sb_id);
    let track = ctx.theme.scrollbar_track();
    let g = ctx.theme.glyphs;
    let track_len = area.width as usize;
    let (start, len) = scroll.thumb(track_len);
    for i in 0..track_len {
        let x = area.x + i as u16;
        if i >= start && i < start + len {
            buf.set_string(x, area.y, g.scroll_h_thumb, thumb);
        } else {
            buf.set_string(x, area.y, g.scroll_h_track, track);
        }
    }
    if scroll.overflows() {
        ctx.scrollbar(sb_id, area, container, crate::core::hit::Axis::Horizontal);
        let thumb_rect = Rect::new(area.x + start as u16, area.y, len as u16, 1);
        ctx.scrollbar_thumb(sb_id, thumb_rect, container);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bars {
    pub content: Rect,
    pub vertical: Option<Rect>,
    pub horizontal: Option<Rect>,
}

impl Bars {
    pub fn split(area: Rect, vertical: bool, horizontal: bool) -> Self {
        let v_w = u16::from(vertical && area.width > 1);
        let h_h = u16::from(horizontal && area.height > 1);
        let content = Rect::new(area.x, area.y, area.width - v_w, area.height - h_h);
        Self {
            content,
            vertical: (v_w == 1).then(|| Rect::new(area.right() - 1, area.y, 1, content.height)),
            horizontal: (h_h == 1).then(|| Rect::new(area.x, area.bottom() - 1, content.width, 1)),
        }
    }
}

pub const GAP: u16 = 1;

pub const fn reserve(gap: u16) -> u16 {
    1 + gap
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VSplit {
    pub body: Rect,

    pub content: Rect,

    pub track: Option<Rect>,
}

pub fn split(area: Rect, overflows: bool, gap: u16) -> VSplit {
    if !overflows || area.width < 2 {
        return VSplit {
            body: area,
            content: area,
            track: None,
        };
    }
    let body = Rect::new(area.x, area.y, area.width - 1, area.height);
    let gap = gap.min(body.width.saturating_sub(1));
    VSplit {
        body,
        content: Rect::new(area.x, area.y, body.width - gap, area.height),
        track: Some(Rect::new(area.right() - 1, area.y, 1, area.height)),
    }
}

fn track_row(track: Rect, pos: Position) -> usize {
    pos.y
        .saturating_sub(track.y)
        .min(track.height.saturating_sub(1)) as usize
}

fn track_col(track: Rect, pos: Position) -> usize {
    pos.x
        .saturating_sub(track.x)
        .min(track.width.saturating_sub(1)) as usize
}

pub fn press(track: Rect, pos: Position, scroll: &mut ScrollState) -> bool {
    press_with(track, pos, scroll, TrackClick::default())
}

pub fn press_with(track: Rect, pos: Position, scroll: &mut ScrollState, click: TrackClick) -> bool {
    scroll.press_track_with(track_row(track, pos), track.height as usize, click)
}

pub fn drag(track: Rect, pos: Position, scroll: &mut ScrollState) -> bool {
    scroll.drag_track(track_row(track, pos), track.height as usize)
}

pub fn press_h(track: Rect, pos: Position, scroll: &mut ScrollState, click: TrackClick) -> bool {
    scroll.press_track_with(track_col(track, pos), track.width as usize, click)
}

pub fn drag_h(track: Rect, pos: Position, scroll: &mut ScrollState) -> bool {
    scroll.drag_track(track_col(track, pos), track.width as usize)
}

pub fn apply(
    bar: &crate::core::hit::Bar,
    pos: Position,
    pressed: bool,
    scroll: &mut ScrollState,
) -> bool {
    apply_with(bar, pos, pressed, scroll, TrackClick::default())
}

pub fn apply_with(
    bar: &crate::core::hit::Bar,
    pos: Position,
    pressed: bool,
    scroll: &mut ScrollState,
    click: TrackClick,
) -> bool {
    use crate::core::hit::Axis;
    match (bar.axis, pressed) {
        (Axis::Vertical, true) => press_with(bar.track, pos, scroll, click),
        (Axis::Vertical, false) => drag(bar.track, pos, scroll),
        (Axis::Horizontal, true) => press_h(bar.track, pos, scroll, click),
        (Axis::Horizontal, false) => drag_h(bar.track, pos, scroll),
    }
}

pub fn pointer(
    ev: &PointerEvent,
    container: WidgetId,
    scroll: &mut ScrollState,
    click: TrackClick,
) -> Option<Outcome> {
    let bar = ev.bar.filter(|b| is_bar_of(b.id, container))?;
    let moved = match BarPhase::of(ev)? {
        BarPhase::Press => apply_with(&bar, ev.pos(), true, scroll, click),
        BarPhase::Drag => apply_with(&bar, ev.pos(), false, scroll, click),
        BarPhase::Release => {
            scroll.release_track();
            false
        }
    };
    Some(if moved {
        Outcome::Changed
    } else {
        Outcome::Consumed
    })
}

pub fn render_with_fade(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    container: WidgetId,
    scroll: &ScrollState,
    focused: bool,
) -> Rect {
    render_with_fade_except(area, buf, ctx, container, scroll, focused, &[])
}

pub fn render_with_fade_except(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    container: WidgetId,
    scroll: &ScrollState,
    focused: bool,
    keep: &[u16],
) -> Rect {
    if !scroll.overflows() || area.width < 2 {
        return area;
    }
    let content = Rect::new(area.x, area.y, area.width - 1, area.height);
    crate::ui::fade::scroll_edges_except(buf, ctx, content, scroll, keep);
    render_vertical(
        Rect::new(area.right() - 1, area.y, 1, area.height),
        buf,
        ctx,
        container,
        scroll,
        focused,
    );
    content
}

impl ScrollPaint for RenderCtx<'_> {
    fn paint_vertical(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        owner: WidgetId,
        scroll: &ScrollState,
        focused: bool,
    ) {
        render_vertical(area, buf, self, owner, scroll, focused);
    }

    fn paint_divider(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        owner: WidgetId,
        scroll: &ScrollState,
        focused: bool,
    ) {
        render_divider(area, buf, self, owner, scroll, focused);
    }

    fn paint_horizontal(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        owner: WidgetId,
        scroll: &ScrollState,
        focused: bool,
        always: bool,
    ) {
        render_horizontal_with(area, buf, self, owner, scroll, focused, always);
    }

    fn paint_with_fade(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        owner: WidgetId,
        scroll: &ScrollState,
        focused: bool,
        keep: &[u16],
    ) -> Rect {
        render_with_fade_except(area, buf, self, owner, scroll, focused, keep)
    }

    fn scrollable_under(&mut self, owner: WidgetId, area: Rect) {
        RenderCtx::scrollable_under(self, owner, area);
    }
}

pub fn position_label(scroll: &ScrollState) -> String {
    if !scroll.overflows() {
        return String::new();
    }
    let r = scroll.visible_range();
    format!("{}–{} of {}", r.start + 1, r.end, scroll.content_len)
}
