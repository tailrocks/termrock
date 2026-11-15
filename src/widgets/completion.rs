use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

use crate::core::event::{Key, Outcome};
use crate::core::id::{ItemKey, WidgetId};
use crate::core::keymap::{Nav, NavKeys};
use crate::core::list::{Cursor, Moved, Navigable, RowSource};
use crate::theme::CursorStyle;
use crate::ui::ctx::RenderCtx;
use crate::ui::popup::{Dismiss, Placement, backdrop_id, place_with, surface_for};
use crate::ui::text::{truncate, width};
use crate::widgets::option_list::{Item, OptionList};

pub const TEXT_INSET: u16 = 4;

#[derive(Debug, Clone)]
pub struct Completion {
    pub id: WidgetId,

    pub items: Vec<Item>,

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
        self.items = items;
        self.list.reset(self.items.len());
        self.list.cursor.index = (0..self.items.len())
            .find(|&i| self.can_accept(i))
            .unwrap_or(0);
        self.anchor = anchor;
        self.replace_len = replace_len;
    }

    pub fn is_open(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn close(&mut self) {
        self.items.clear();
    }

    pub fn current(&self) -> Option<&Item> {
        self.items.get(self.list.cursor.index)
    }

    fn can_accept(&self, index: usize) -> bool {
        let source = &self.items[..];
        index < source.len() && source.enabled(index)
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
        let (min_w, max_w) = self.list.width;
        let w = (label_w + detail_w + 8).clamp(min_w as usize, max_w as usize) as u16;
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

        let me = self.clone();
        ctx.overlay(move |buf, ctx| me.paint(area, buf, ctx));
    }

    fn paint(&self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let inner = surface_for(area, buf, ctx, t, Some((self.id, self.list.dismiss)));
        let bg = t.surface_elevated;
        let items = &self.items[..];
        self.list
            .paint_rows(inner, bg, items, buf, ctx, |i, r, buf, ctx| {
                let it = &items[i];
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

                let mut x = row.x + 3;
                let avail = row.width.saturating_sub(3) as usize;
                let show_detail =
                    !it.detail.is_empty() && avail > width(&it.label) + width(&it.detail) + 2;
                let label = truncate(
                    &it.label,
                    if show_detail {
                        avail - width(&it.detail) - 2
                    } else {
                        avail
                    },
                );
                for (bi, ch) in label.char_indices() {
                    let mut cs = st;
                    if it.matched.contains(&bi) {
                        cs = cs.add_modifier(Modifier::BOLD);
                    } else if !focused {
                        cs = cs.remove_modifier(Modifier::BOLD);
                    }
                    let g = ch.to_string();
                    buf.set_string(x, y, &g, cs);
                    x += width(&g) as u16;
                }
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
