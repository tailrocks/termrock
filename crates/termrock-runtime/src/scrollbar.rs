//! Painting of scroll bars on a `RenderCtx`, and the `ScrollPaint` impl that lets
//! `ScrollPane::draw_*` (in `termrock-core`) call it. `widgets::scrollbar`
//! re-exports these functions at their old paths.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use termrock_core::id::WidgetId;
use termrock_core::scroll::{ScrollPaint, ScrollState};
use termrock_theme::{Kind, Plane};

use crate::ctx::{RenderCtx, VisualState};

pub fn id_for(container: WidgetId) -> WidgetId {
    container.sub("scrollbar")
}

pub fn h_id_for(container: WidgetId) -> WidgetId {
    container.sub("hscrollbar")
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
    ctx.scrollbar(sb_id, area, container, termrock_core::hit::Axis::Vertical);
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
        ctx.scrollbar(sb_id, area, container, termrock_core::hit::Axis::Horizontal);
        let thumb_rect = Rect::new(area.x + start as u16, area.y, len as u16, 1);
        ctx.scrollbar_thumb(sb_id, thumb_rect, container);
    }
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
    crate::fade::scroll_edges_except(buf, ctx, content, scroll, keep);
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
