use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::core::event::{Key, Outcome};
use crate::core::hints::{HasHints, HintCx, nav_hints};
use crate::core::id::{ItemKey, WidgetId};
use crate::core::keymap::{Nav, NavKeys};
use crate::core::list::{Cursor, Moved, Navigable, RowSource};
use crate::core::refusal::Reason;
use crate::theme::CursorStyle;
use crate::ui::ctx::RenderCtx;
use crate::ui::popup::{Dismiss, Placement, backdrop_id, place_with, surface_for};
use crate::ui::text::{TextClip, paint_graphemes, width};
use crate::widgets::keyhint::{Hint, hint};
use crate::widgets::option_list::{Item, OptionList, PopupSnapshot, paint_reason};

pub const TEXT_INSET: u16 = 4;

#[derive(Debug, Clone)]
pub struct Completion {
    pub id: WidgetId,

    items: Vec<Item>,
    widths: Option<(usize, usize)>,

    pub list: OptionList,
    pub anchor: Rect,

    pub replace_len: usize,
    pub area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionAction {
    Accept(usize),
    Dismiss,
}

impl Completion {
    pub const NAV: NavKeys = NavKeys::NO_VIM.home_end(false);

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.list.nav = nav;
        self
    }

    pub fn max_rows(mut self, rows: u16) -> Self {
        self.list.max_rows = rows;
        self
    }

    /// Bounds the popup width from its labels and details. A disabled entry's
    /// reason may widen it past `max` so the reason shows whole, up to the screen.
    pub fn popup_width(mut self, min: u16, max: u16) -> Self {
        self.list = self.list.width(min, max);
        self
    }

    pub fn new(id: WidgetId) -> Self {
        let list = OptionList::new(id)
            .width(24, 48)
            .fit(crate::ui::popup::Place::DEFAULT.text_inset(TEXT_INSET))
            .dismiss(Dismiss::LIGHT.pass_through(true))
            .nav(Self::NAV)
            .type_ahead(false)
            .hover_moves_cursor(false);
        Self {
            id,
            items: vec![],
            widths: None,
            list,
            anchor: Rect::ZERO,
            replace_len: 0,
            area: Rect::ZERO,
        }
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.list.placement = placement;
        self
    }

    pub fn fit(mut self, fit: crate::ui::popup::Place) -> Self {
        self.list.fit = fit;
        self
    }

    pub fn cursor_style(mut self, cursor_style: CursorStyle) -> Self {
        self.list.look = cursor_style;
        self
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.list.dismiss = dismiss;
        self
    }

    pub fn open(&mut self, items: Vec<Item>, anchor: Rect, replace_len: usize) {
        self.set_items(items);
        self.list.reset(self.items.len());
        self.list.cursor.index = (0..self.items.len())
            .find(|&i| self.can_accept(i))
            .unwrap_or(0);
        self.anchor = anchor;
        self.replace_len = replace_len;
    }

    /// The completion rows. They change only through [`Completion::open`],
    /// [`Completion::set_items`] and [`Completion::close`], which keep the
    /// cached aggregate widths in step.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn set_items(&mut self, items: Vec<Item>) {
        self.items = items;
        self.widths = None;
    }

    pub fn is_open(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn close(&mut self) {
        self.items.clear();
        self.widths = None;
    }

    pub fn current(&self) -> Option<&Item> {
        self.items.get(self.list.cursor.index)
    }

    fn can_accept(&self, index: usize) -> bool {
        let source = &self.items[..];
        index < source.len() && source.enabled(index)
    }

    pub fn has_eligible(&self) -> bool {
        (0..self.items.len()).any(|i| self.can_accept(i))
    }

    fn accepted(&self, index: usize) -> Option<CompletionAction> {
        self.can_accept(index)
            .then_some(CompletionAction::Accept(index))
    }

    pub fn apply(&self, i: usize, buffer: &mut crate::core::text::TextBuffer) -> bool {
        if !self.can_accept(i) {
            return false;
        }
        let item = &self.items[i];
        let insert = item.insertion();
        let cur = buffer.cursor_offset();
        buffer.remove_range(cur.saturating_sub(self.replace_len)..cur);
        buffer.insert_str(insert);
        if insert.ends_with('(') {
            buffer.insert_char(')');
            buffer.move_left(false);
        }
        true
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.list.row_id(i)
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<CompletionAction>) {
        if !self.is_open() {
            return (Outcome::Ignored, None);
        }
        let up_down = key.plain() || key.ctrl();
        let moved = match (self.list.nav.action(key), key.code) {
            (Some(Nav::Down | Nav::Up), _) if !up_down => Moved::Ignored,
            (Some(_), _) => self.list.key(key, &self.items[..]),

            (_, KeyCode::Char('n')) if key.ctrl() => self.list.step(1, &self.items[..]),
            (_, KeyCode::Char('p')) if key.ctrl() => self.list.step(-1, &self.items[..]),
            _ => Moved::Ignored,
        };
        if moved.index().is_some() {
            return (Outcome::Changed, None);
        }
        match key.code {
            KeyCode::Tab | KeyCode::Enter => match self.accepted(self.list.cursor.index) {
                Some(accept) => (Outcome::Changed, Some(accept)),
                None => (Outcome::Consumed, None),
            },
            KeyCode::Esc if self.list.dismiss.escape => {
                self.close();
                (Outcome::Changed, Some(CompletionAction::Dismiss))
            }
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn on_click(&mut self, id: WidgetId) -> Option<CompletionAction> {
        if id == backdrop_id(self.id) {
            if !self.list.dismiss.outside_click {
                return None;
            }
            self.close();
            return Some(CompletionAction::Dismiss);
        }
        let i = self.list.row_of(id, self.items.len())?;
        let accepted = self.accepted(i)?;
        self.list.select(i, &self.items[..]);
        Some(accepted)
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.list.scroll.begin(ctx.track_click);
        if !self.is_open() {
            return;
        }
        let (label_w, detail_w) = self.aggregate_widths();
        // A disabled entry's reason is why it cannot be accepted, so it must fit
        // whole. `menu::reason_room` leaves 8 columns of chrome around it. It may
        // widen the popup past the `popup_width` maximum (that bounds the label and
        // detail columns; `Reason::TEXT_MAX` is 60, so 68 columns), but never past the
        // screen: `place_with` clamps, and `paint_reason` then ellipsizes. `Item::reason`
        // does not enforce `TEXT_MAX`, so the widening is capped at `TEXT_MAX + 8`.
        let reason_w = self
            .items
            .iter()
            .filter(|i| i.disabled)
            .filter_map(|i| i.reason.as_deref())
            .map(width)
            .max()
            .map_or(0, |w| w.min(Reason::TEXT_MAX) + 8);
        let (min_w, max_w) = self.list.width;
        let w = (label_w + detail_w + 8)
            .clamp(min_w as usize, max_w as usize)
            .max(reason_w)
            .min(usize::from(screen.width)) as u16;
        let rows = (self.items.len() as u16).min(self.list.max_rows);
        let h = rows + 2;
        let area = place_with(
            screen,
            self.anchor,
            w,
            h,
            self.list.placement,
            self.list.fit,
        );
        self.area = area;
        let inner = area
            .intersection(*buf.area())
            .inner(ratatui::layout::Margin::new(1, 1));

        self.list.layout(inner, &self.items[..], ctx);

        let items = &self.items;
        let snapshot = self.list.snapshot(inner, &items[..], |i| {
            crate::record_work!(PayloadBytesCloned, items[i].payload_bytes());
            items[i].clone()
        });
        let (id, dismiss) = (self.id, self.list.dismiss);
        let reason = self.list.status(&self.items[..]).map(str::to_owned);
        ctx.overlay(move |buf, ctx| {
            Self::paint(&snapshot, reason.as_deref(), id, dismiss, area, buf, ctx);
        });
    }

    fn aggregate_widths(&mut self) -> (usize, usize) {
        if let Some(widths) = self.widths {
            return widths;
        }
        crate::record_work!(WidthRowsMeasured, self.items.len());
        let label_w = self
            .items
            .iter()
            .map(|i| width(&i.label))
            .max()
            .unwrap_or(4);
        let detail_w = self
            .items
            .iter()
            .map(|i| width(&i.detail))
            .max()
            .unwrap_or(0);
        *self.widths.insert((label_w, detail_w))
    }

    fn paint(
        snapshot: &PopupSnapshot<Item>,
        reason: Option<&str>,
        id: WidgetId,
        dismiss: Dismiss,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
    ) {
        let t = ctx.theme;
        let inner = surface_for(area, buf, ctx, t, Some((id, dismiss)));
        let bg = t.surface_elevated;
        snapshot.paint_rows(inner, bg, buf, ctx, |pr, r, buf, ctx| {
            let it = &pr.payload;
            let (row, y) = (r.area, r.area.y);
            let st = r.style();
            let focused = r.state.focused;
            buf.set_string(
                row.x + 1,
                y,
                &it.glyph,
                st.fg(if focused {
                    t.text_primary
                } else {
                    r.ink(ctx, t.text_muted)
                })
                .remove_modifier(Modifier::BOLD),
            );

            let avail = row.width.saturating_sub(3) as usize;
            let show_detail =
                !it.detail.is_empty() && avail > width(&it.label) + width(&it.detail) + 2;
            let label_w = if show_detail {
                avail - width(&it.detail) - 2
            } else {
                avail
            };
            paint_graphemes(
                buf,
                Rect::new(row.x + 3, y, label_w as u16, 1).intersection(row),
                (0, 0),
                &it.label,
                TextClip::Ellipsis,
                |source, _| {
                    let matched =
                        source.is_some_and(|range| it.matched.iter().any(|m| range.contains(m)));
                    if matched {
                        st.add_modifier(Modifier::BOLD)
                    } else if !focused {
                        st.remove_modifier(Modifier::BOLD)
                    } else {
                        st
                    }
                },
            );
            if show_detail {
                let dx = row.right().saturating_sub(width(&it.detail) as u16 + 1);
                buf.set_string(
                    dx,
                    y,
                    &it.detail,
                    st.fg(r.ink(ctx, t.text_muted))
                        .remove_modifier(Modifier::BOLD),
                );
            }
        });
        paint_reason(reason, snapshot.more_below(), area, bg, buf, t);
    }
}

impl RowSource for Completion {
    fn len(&self) -> usize {
        self.items.len()
    }
    fn enabled(&self, i: usize) -> bool {
        self.items[..].enabled(i)
    }
    fn label(&self, i: usize) -> &str {
        &self.items[i].label
    }
    fn key(&self, i: usize) -> ItemKey {
        self.items[..].key(i)
    }
}

impl Navigable for Completion {
    fn cursor(&self) -> Cursor {
        self.list.cursor
    }
}

impl crate::core::pointer::MouseTarget for Completion {
    type Action = CompletionAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<CompletionAction> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;
        if let Some(r) = crate::core::pointer::scroll_input(&mut self.list, ev) {
            return r.lift();
        }
        match ev.phase() {
            Some(Phase::Click) => match self.on_click(ev.id) {
                Some(a) => Response::emit(a),
                None => Response::consumed(),
            },
            _ => Response::ignored(),
        }
    }
}

impl HasHints for Completion {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if !self.is_open() {
            return vec![];
        }

        let mut v = nav_hints(&self.list.nav, false, false);
        if self.has_eligible() {
            v.push(hint("Enter", "Accept"));
        }
        v.push(hint("Esc", "Close"));
        v
    }
}
