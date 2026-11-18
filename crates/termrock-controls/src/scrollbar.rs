use ratatui::layout::{Position, Rect};

use termrock_core::event::Outcome;
use termrock_core::id::WidgetId;
use termrock_core::pointer::{BarPhase, PointerEvent};
use termrock_core::scroll::{ScrollState, TrackClick};
use termrock_theme::Glyphs;

pub use termrock_runtime::scrollbar::{
    h_id_for, id_for, render_divider, render_horizontal, render_horizontal_with, render_vertical,
    render_with_fade, render_with_fade_except,
};

pub const TRACK: &str = Glyphs::UNICODE.scroll_track;
pub const THUMB: &str = Glyphs::UNICODE.scroll_thumb;

pub const H_TRACK: &str = Glyphs::UNICODE.scroll_h_track;
pub const H_THUMB: &str = Glyphs::UNICODE.scroll_h_thumb;

pub fn is_bar_of(id: WidgetId, container: WidgetId) -> bool {
    id == id_for(container) || id == h_id_for(container)
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
    bar: &termrock_core::hit::Bar,
    pos: Position,
    pressed: bool,
    scroll: &mut ScrollState,
) -> bool {
    apply_with(bar, pos, pressed, scroll, TrackClick::default())
}

pub fn apply_with(
    bar: &termrock_core::hit::Bar,
    pos: Position,
    pressed: bool,
    scroll: &mut ScrollState,
    click: TrackClick,
) -> bool {
    use termrock_core::hit::Axis;
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

pub fn position_label(scroll: &ScrollState) -> String {
    if !scroll.overflows() {
        return String::new();
    }
    let r = scroll.visible_range();
    format!("{}–{} of {}", r.start + 1, r.end, scroll.content_len)
}
