use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::core::id::WidgetId;
use crate::theme::Theme;
use crate::ui::ctx::{RenderCtx, fill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dismiss {
    pub escape: bool,

    pub outside_click: bool,

    pub pass_through: bool,

    pub close_button: bool,

    pub refuse_feedback: RefuseFeedback,

    pub focus_out: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RefuseFeedback {
    #[default]
    Flash,

    None,
}

impl Dismiss {
    pub const LIGHT: Self = Self {
        escape: true,
        outside_click: true,
        pass_through: false,
        close_button: true,
        refuse_feedback: RefuseFeedback::Flash,
        focus_out: true,
    };

    pub const ESCAPE: Self = Self {
        escape: true,
        outside_click: false,
        pass_through: false,
        close_button: true,
        refuse_feedback: RefuseFeedback::Flash,
        focus_out: false,
    };

    pub const NEVER: Self = Self {
        escape: false,
        outside_click: false,
        pass_through: false,
        close_button: false,
        refuse_feedback: RefuseFeedback::Flash,
        focus_out: false,
    };
}

impl Dismiss {
    pub const fn pass_through(mut self, on: bool) -> Self {
        self.pass_through = on;
        self
    }

    pub const fn close_button(mut self, on: bool) -> Self {
        self.close_button = on;
        self
    }

    pub const fn refuse_feedback(mut self, feedback: RefuseFeedback) -> Self {
        self.refuse_feedback = feedback;
        self
    }

    pub const fn focus_out(mut self, on: bool) -> Self {
        self.focus_out = on;
        self
    }

    pub const fn allows(self, reason: crate::core::intent::DismissReason) -> bool {
        reason.permitted_by(self.escape, self.outside_click, self.focus_out)
    }

    pub const fn shows_close(self) -> bool {
        self.close_button && self.escape
    }
}

impl Default for Dismiss {
    fn default() -> Self {
        Self::LIGHT
    }
}

pub const fn backdrop_id(owner: WidgetId) -> WidgetId {
    owner.sub("popup.backdrop")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Below,

    Above,

    Right,

    Left,

    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    Shrink,

    Slide,
}

pub fn backdrop(ctx: &mut RenderCtx, owner: WidgetId, dismiss: Dismiss, screen: Rect) {
    if !ctx.inert {
        ctx.hits.register_backdrop_of(
            backdrop_id(owner),
            owner,
            screen,
            dismiss.outside_click && dismiss.pass_through,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Center,

    Third,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub margin: u16,

    pub v_align: VAlign,

    pub text_inset: u16,

    pub vertical_margin: Option<u16>,

    pub overflow: Overflow,
}

impl Place {
    pub const DEFAULT: Self = Self {
        margin: 1,
        v_align: VAlign::Center,
        text_inset: 0,
        vertical_margin: None,
        overflow: Overflow::Shrink,
    };

    pub const FLUSH: Self = Self {
        margin: 0,
        v_align: VAlign::Center,
        text_inset: 0,
        vertical_margin: None,
        overflow: Overflow::Shrink,
    };

    pub const MENU: Self = Self {
        margin: 1,
        v_align: VAlign::Center,
        text_inset: 0,
        vertical_margin: Some(0),
        overflow: Overflow::Slide,
    };

    pub const fn vertical_margin(mut self, margin: Option<u16>) -> Self {
        self.vertical_margin = margin;
        self
    }

    pub const fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub const fn margin(mut self, margin: u16) -> Self {
        self.margin = margin;
        self
    }

    pub const fn v_align(mut self, v_align: VAlign) -> Self {
        self.v_align = v_align;
        self
    }

    pub const fn text_inset(mut self, inset: u16) -> Self {
        self.text_inset = inset;
        self
    }
}

impl Default for Place {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub fn place(screen: Rect, anchor: Rect, width: u16, height: u16, placement: Placement) -> Rect {
    place_with(screen, anchor, width, height, placement, Place::DEFAULT)
}

pub fn place_with(
    screen: Rect,
    anchor: Rect,
    width: u16,
    height: u16,
    placement: Placement,
    fit: Place,
) -> Rect {
    let width = width.min(screen.width).max(1);
    let height = height.min(screen.height).max(1);

    let mx = fit.margin.min(screen.width.saturating_sub(width) / 2);
    let my = fit
        .vertical_margin
        .unwrap_or(fit.margin)
        .min(screen.height.saturating_sub(height) / 2);
    let bounds = Rect::new(
        screen.x + mx,
        screen.y + my,
        screen.width - 2 * mx,
        screen.height - 2 * my,
    );
    match placement {
        Placement::Center => {
            let x = bounds.x + bounds.width.saturating_sub(width) / 2;
            let y = match fit.v_align {
                VAlign::Center => bounds.y + bounds.height.saturating_sub(height) / 2,
                VAlign::Third => bounds.y + (bounds.height.saturating_sub(height) / 3).max(1),
            };
            Rect::new(
                x,
                y.min(bounds.bottom().saturating_sub(height)),
                width,
                height,
            )
        }
        Placement::Below | Placement::Above => {
            let (y, height) = side(
                (bounds.y, bounds.bottom()),
                (anchor.y, anchor.bottom()),
                height,
                placement == Placement::Below,
                fit.overflow,
            );
            let x = anchor
                .x
                .saturating_sub(fit.text_inset)
                .min(bounds.right().saturating_sub(width))
                .max(bounds.x);
            Rect::new(x, y, width, height)
        }
        Placement::Right | Placement::Left => {
            let (x, width) = side(
                (bounds.x, bounds.right()),
                (anchor.x, anchor.right()),
                width,
                placement == Placement::Right,
                fit.overflow,
            );
            let y = anchor
                .y
                .min(bounds.bottom().saturating_sub(height))
                .max(bounds.y);
            Rect::new(x, y, width, height)
        }
    }
}

fn side(
    bounds: (u16, u16),
    anchor: (u16, u16),
    len: u16,
    after: bool,
    overflow: Overflow,
) -> (u16, u16) {
    let (lo, hi) = bounds;
    // An anchor a resize left outside the bounds acts as if it sat on the
    // nearest edge, so the room before it never exceeds the bounds.
    let anchor = (anchor.0.clamp(lo, hi), anchor.1.clamp(lo, hi));
    let past = anchor.1;
    let room_after = hi.saturating_sub(past);
    let room_before = anchor.0.saturating_sub(lo).min(hi - lo);
    let at_after = |n: u16| (past, n);
    let at_before = |n: u16| (anchor.0 - n, n);
    let (first_room, second_room) = if after {
        (room_after, room_before)
    } else {
        (room_before, room_after)
    };
    let pick = |is_after: bool, n: u16| if is_after { at_after(n) } else { at_before(n) };
    if first_room >= len {
        return pick(after, len);
    }
    if second_room >= len {
        return pick(!after, len);
    }
    match overflow {
        Overflow::Slide => {
            if after {
                (hi.saturating_sub(len).max(lo), len)
            } else {
                (lo, len)
            }
        }
        Overflow::Shrink => {
            let after_wins = if after {
                room_after >= room_before
            } else {
                room_after > room_before
            };
            if after_wins && room_after > 0 {
                at_after(room_after)
            } else if room_before > 0 {
                at_before(room_before)
            } else if room_after > 0 {
                at_after(room_after)
            } else {
                (hi.saturating_sub(len), len)
            }
        }
    }
}

pub fn surface(area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, t: &Theme) -> Rect {
    surface_for(area, buf, ctx, t, None)
}

pub fn surface_for(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    t: &Theme,
    owner: Option<(WidgetId, Dismiss)>,
) -> Rect {
    let area = area.intersection(*buf.area());
    if area.is_empty() {
        return area;
    }
    fill(buf, area, Style::new().bg(t.surface_elevated));
    let block = ratatui::widgets::Block::new()
        .borders(ratatui::widgets::Borders::ALL)
        .border_set(t.glyphs.border)
        .border_style(t.border(true).bg(t.surface_elevated));
    ratatui::widgets::Widget::render(block, area, buf);

    ctx.hits.push_barrier();
    if let Some((owner, dismiss)) = owner {
        backdrop(ctx, owner, dismiss, *buf.area());
    }
    ctx.hits.register(WidgetId::of("popup.surface"), area);
    area.inner(ratatui::layout::Margin::new(1, 1))
}
