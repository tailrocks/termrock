use ratatui::buffer::Buffer;
use ratatui::layout::{Margin, Rect};
use ratatui::style::Style;

use crate::core::focus::{Focus, FocusFallback, FocusRing};
use crate::core::id::WidgetId;
use crate::ui::ctx::{Backdrop, RenderCtx, fill};
use crate::ui::popup::{Dismiss, Place, Placement, place_with};
use crate::ui::text::{truncate, width};
use crate::ui::tooltip::Tip;
use crate::widgets::icon_button::IconButton;

pub const CLOSE: WidgetId = WidgetId::of("modal.close");

pub const SURFACE: WidgetId = WidgetId::of("modal.surface");

pub const BACKDROP: WidgetId = WidgetId::of("modal.backdrop");

pub const CLOSE_WIDTH: u16 = 3;

pub const OUTER_MARGIN: Margin = Margin {
    horizontal: 3,
    vertical: 1,
};

pub fn body_bounds(screen: Rect, top: u16, bottom: u16) -> Rect {
    let h = screen.height.saturating_sub(top + bottom);
    if h == 0 {
        return screen;
    }
    Rect::new(screen.x, screen.y + top, screen.width, h)
}

pub fn refuse_outside_click(
    pointer: &mut crate::core::pointer::PointerState,
    dismiss: Dismiss,
    now_ms: u64,
) -> bool {
    if dismiss.outside_click || dismiss.refuse_feedback != crate::ui::popup::RefuseFeedback::Flash {
        return false;
    }
    pointer.flash(SURFACE, now_ms);
    true
}

pub fn border_style(ctx: &RenderCtx, bg: ratatui::style::Color) -> Style {
    let t = ctx.theme;
    if ctx.interaction.flash == Some(SURFACE) {
        Style::new()
            .fg(t.warning)
            .bg(bg)
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        t.border(true).bg(bg)
    }
}

pub fn close_button(
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    bg: ratatui::style::Color,
) -> Rect {
    close_button_with(&IconButton::close(CLOSE), area, buf, ctx, bg)
}

pub fn close_button_with(
    button: &IconButton,
    area: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    bg: ratatui::style::Color,
) -> Rect {
    let t = ctx.theme;
    let w = button.width(t);
    if area.width < 16 || area.height < 3 || area.width < w + 10 {
        return Rect::ZERO;
    }
    let host = t.border(true).bg(bg);
    let tip = Tip::new(t.strings.tip_close).shortcut("Esc");
    button
        .with_id(CLOSE)
        .render(area.right() - 2 - w, area.y, buf, ctx, host, Some(tip))
}

pub trait Modal {
    fn dismiss(&self) -> Dismiss;
}

impl<O: crate::ui::layer::Overlay> Modal for O {
    fn dismiss(&self) -> Dismiss {
        crate::ui::layer::Overlay::dismiss(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dismissal {
    None,

    Close,

    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusOnOpen {
    Preserve,

    Set(Option<WidgetId>),
}

struct Entry<M> {
    modal: M,
    saved_focus: Option<WidgetId>,
    restore_focus: bool,
}

pub struct ModalStack<M> {
    entries: Vec<Entry<M>>,
}

impl<M> Default for ModalStack<M> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<M> ModalStack<M> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, modal: M, focus: &mut Focus, initial: Option<WidgetId>) {
        self.push_with_focus_policy(modal, focus, FocusOnOpen::Set(initial), true);
    }

    pub fn push_with_focus_policy(
        &mut self,
        modal: M,
        focus: &mut Focus,
        initial: FocusOnOpen,
        restore_focus: bool,
    ) {
        self.entries.push(Entry {
            modal,
            saved_focus: focus.current(),
            restore_focus,
        });
        if let FocusOnOpen::Set(target) = initial {
            focus.set(target);
        }
    }

    pub fn pop(&mut self, focus: &mut Focus) -> Option<M> {
        let e = self.entries.pop()?;
        if e.restore_focus {
            focus.set(e.saved_focus);
        }
        Some(e.modal)
    }

    pub fn clear(&mut self, focus: &mut Focus) {
        let mut target = focus.current();
        for e in self.entries.iter().rev() {
            if e.restore_focus {
                target = e.saved_focus;
            }
        }
        self.entries.clear();
        if focus.current() != target {
            focus.set(target);
        }
    }

    pub fn top(&self) -> Option<&M> {
        self.entries.last().map(|e| &e.modal)
    }
    pub fn top_mut(&mut self) -> Option<&mut M> {
        self.entries.last_mut().map(|e| &mut e.modal)
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &M> {
        self.entries.iter().map(|e| &e.modal)
    }

    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut M> {
        self.entries.iter_mut().map(|e| &mut e.modal)
    }

    pub fn retain(&mut self, mut pred: impl FnMut(&M) -> bool) {
        let mut removed = false;
        self.entries.retain_mut(|e| {
            let keep = pred(&e.modal);
            if !keep {
                removed = true;
            } else if removed {
                e.saved_focus = None;
            }
            keep
        });
    }
}

impl<M: Modal> ModalStack<M> {
    pub fn outside_click(&self) -> Dismissal {
        match self.top() {
            None => Dismissal::None,
            Some(m) if m.dismiss().outside_click => Dismissal::Close,
            Some(_) => Dismissal::Keep,
        }
    }

    pub fn escape(&self) -> Dismissal {
        match self.top() {
            None => Dismissal::None,
            Some(m) if m.dismiss().escape => Dismissal::Close,
            Some(_) => Dismissal::Keep,
        }
    }
}

pub fn ensure_focus(focus: &mut Focus, ring: &FocusRing, fallback: FocusFallback) {
    focus.ensure_valid_with(ring, fallback);
}

#[deprecated(note = "use `popup::Placement` (`Placement::Center`) with `Frame::placement`")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramePlacement {
    Upper,

    Middle,
}

#[allow(deprecated, clippy::derivable_impls)]
impl Default for FramePlacement {
    fn default() -> Self {
        Self::Upper
    }
}

#[allow(deprecated)]
impl From<FramePlacement> for Placement {
    fn from(_: FramePlacement) -> Self {
        Placement::Center
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame<'a> {
    pub width: u16,
    pub height: u16,
    pub min_width: u16,
    pub min_height: u16,
    pub title: Option<&'a str>,
    pub meta: Option<&'a str>,

    pub placement: Placement,

    pub anchor: Rect,

    pub fit: Place,

    pub margin: Margin,

    pub top: u16,

    pub bottom: u16,

    pub backdrop: bool,

    pub dim: Backdrop,

    pub dismiss: Option<Dismiss>,

    pub close_button: IconButton,
}

impl<'a> Frame<'a> {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            min_width: 20,
            min_height: 5,
            title: None,
            meta: None,
            placement: Placement::Center,
            anchor: Rect::ZERO,
            fit: Place::DEFAULT,
            margin: Margin::new(3, 1),
            top: 1,
            bottom: 1,
            backdrop: true,
            dim: Backdrop::DEFAULT,
            dismiss: None,
            close_button: IconButton::close(CLOSE),
        }
    }

    pub fn close_button(mut self, button: IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.dismiss = Some(dismiss);
        self
    }

    pub fn overlay<O: crate::ui::layer::Overlay + ?Sized>(mut self, overlay: &O) -> Self {
        self.dismiss = Some(overlay.dismiss());
        if let Some(b) = overlay.close_button() {
            self.close_button = b.clone();
        }
        self
    }
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }
    pub fn meta(mut self, meta: Option<&'a str>) -> Self {
        self.meta = meta;
        self
    }

    pub fn placement(mut self, placement: impl Into<Placement>) -> Self {
        self.placement = placement.into();
        self
    }

    pub fn anchor(mut self, anchor: Rect, placement: Placement) -> Self {
        self.anchor = anchor;
        self.placement = placement;
        self
    }
    pub fn fit(mut self, fit: Place) -> Self {
        self.fit = fit;
        self
    }
    pub fn margin(mut self, margin: Margin) -> Self {
        self.margin = margin;
        self
    }
    pub fn min_size(mut self, width: u16, height: u16) -> Self {
        self.min_width = width;
        self.min_height = height;
        self
    }

    pub fn chrome(mut self, top: u16, bottom: u16) -> Self {
        self.top = top;
        self.bottom = bottom;
        self
    }
    pub fn backdrop(mut self, on: bool) -> Self {
        self.backdrop = on;
        self
    }

    pub fn dim(mut self, dim: Backdrop) -> Self {
        self.dim = dim;
        self
    }

    pub fn area(&self, screen: Rect) -> Rect {
        let bounds = body_bounds(screen, self.top, self.bottom);
        let m = OUTER_MARGIN;
        let w = self
            .width
            .min(bounds.width.saturating_sub(2 * m.horizontal))
            .max(self.min_width)
            .min(bounds.width);
        let h = self
            .height
            .min(bounds.height.saturating_sub(2 * m.vertical))
            .max(self.min_height)
            .min(bounds.height);
        place_with(bounds, self.anchor, w, h, self.placement, self.fit)
    }

    /// Width of the content region on `screen`. It does not depend on the
    /// frame height, so a dialog can measure a wrapped footer before it
    /// chooses that height.
    pub(crate) fn inner_width(&self, screen: Rect) -> u16 {
        self.area(screen).inner(self.margin).width
    }

    pub fn render(&self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) -> (Rect, Rect) {
        if screen.intersection(*buf.area()).is_empty() {
            return (Rect::ZERO, Rect::ZERO);
        }
        let t = ctx.theme;
        if self.backdrop {
            ctx.modal_backdrop_with(buf, screen, self.dim);

            if self.dismiss.is_some() && !ctx.inert {
                ctx.hits.register_part_as(
                    BACKDROP,
                    BACKDROP,
                    crate::core::id::PartRef::of(crate::core::id::Part::BACKDROP),
                    screen,
                );
            }
        }
        let area = self.area(screen);
        let bg = t.surface_elevated;
        fill(buf, area, Style::new().bg(bg));
        let block = ratatui::widgets::Block::new()
            .borders(ratatui::widgets::Borders::ALL)
            .border_set(t.glyphs.border)
            .border_style(border_style(ctx, bg));
        ratatui::widgets::Widget::render(block, area, buf);

        if !ctx.inert {
            ctx.hits.register_surface(SURFACE, area);
        }
        let close = match self.dismiss {
            Some(d) if d.shows_close() => close_button_with(&self.close_button, area, buf, ctx, bg),
            _ => Rect::ZERO,
        };

        let reserve = if close.is_empty() { 0 } else { close.width + 1 };
        if let Some(title) = self.title
            && area.width > 6 + reserve
        {
            let tt = format!(
                " {} ",
                truncate(title, area.width.saturating_sub(6 + reserve) as usize)
            );
            buf.set_string(area.x + 2, area.y, &tt, t.title().bg(bg));
            if let Some(m) = self.meta {
                let mt = format!(" {m} ");
                let mw = width(&mt) as u16;
                if area.width > width(&tt) as u16 + mw + 4 + reserve {
                    buf.set_string(
                        area.right() - 2 - reserve - mw,
                        area.y,
                        &mt,
                        t.faint().bg(bg),
                    );
                }
            }
        }
        (area, area.inner(self.margin))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn modal_frame(
    screen: Rect,
    buf: &mut Buffer,
    ctx: &mut RenderCtx,
    width_: u16,
    height: u16,
    title: &str,
    meta: Option<&str>,

    dismiss: Dismiss,
) -> (Rect, Rect) {
    Frame::new(width_, height)
        .title(title)
        .meta(meta)
        .placement(Placement::Center)
        .dismiss(dismiss)
        .render(screen, buf, ctx)
}
