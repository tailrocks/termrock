use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};

use crate::core::event::{Key, Outcome};

use crate::core::hints::{HasHints, HintCx, nav_hints};
use crate::core::hit::{Axis, ReleaseFrom};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Nav, NavKeys, unassigned_chord};
use crate::core::list::{Cursor, Moved, Navigable, RowFn, RowSource};
use crate::core::response::Response;
use crate::core::scroll::Scrollable;
use crate::theme::{CursorStyle, Kind, Plane};
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::layer::{DismissReason, LayerSpec, Overlay};
use crate::ui::popup::{Dismiss, Place, backdrop_id, place_with};
use crate::ui::specimen::{Specimen, SpecimenState};
use crate::ui::text::{truncate, width};
use crate::widgets::brand::Lockup;
use crate::widgets::keyhint::Hint;
use crate::widgets::option_list::{AsItem, Item, OptionList, WheelMode};
use ratatui::crossterm::event::KeyCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub item: Item,

    pub shortcut: Option<std::borrow::Cow<'static, str>>,
    pub danger: bool,
    pub separator_after: bool,
}

impl AsItem for MenuItem {
    fn as_item(&self) -> &Item {
        &self.item
    }
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            item: Item::new(label),
            shortcut: None,
            danger: false,
            separator_after: false,
        }
    }
    pub fn shortcut(mut self, s: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.item.disabled = d;
        self
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.item = self.item.reason(reason);
        self
    }
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn danger_if(mut self, on: bool) -> Self {
        self.danger = on;
        self
    }
    pub fn separator(mut self) -> Self {
        self.separator_after = true;
        self
    }

    pub fn separator_if(mut self, on: bool) -> Self {
        self.separator_after = on;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnknownKey {
    #[default]
    CloseAndPass,

    Swallow,
}

pub use crate::ui::popup::Placement;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Chosen(usize),
    Dismissed,
}

#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub id: WidgetId,
    pub items: Vec<MenuItem>,

    pub list: OptionList,
    pub anchor: Rect,

    pub area: Rect,

    pub title: Option<String>,

    pub(crate) pending: crate::ui::layer::ResultQueue<MenuAction>,

    pub hide_owner_focus: bool,

    pub reveal: Option<crate::ui::reveal::Reveal>,

    pub min_width: u16,

    pub anchor_to: Option<WidgetId>,

    pub unknown_key: UnknownKey,

    pub release_selects: bool,

    release_from: Vec<WidgetId>,

    scrolled_tick: Option<u64>,
}

impl HasHints for ContextMenu {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        let mut v = nav_hints(&self.list.nav, self.list.type_ahead, false);
        v.push(Hint::new("Enter", "Choose"));
        if self.list.dismiss.escape {
            v.push(Hint::new("Esc", "Close"));
        }
        v
    }
}

impl ContextMenu {
    pub fn release_selects(mut self, on: bool) -> Self {
        self.release_selects = on;
        self
    }

    pub fn more_above_id(&self) -> WidgetId {
        self.id.part(PartRef::OVERFLOW_BEFORE)
    }

    pub fn more_below_id(&self) -> WidgetId {
        self.id.part(PartRef::OVERFLOW_AFTER)
    }

    pub fn highlighted(&self) -> Option<usize> {
        self.list.highlighted().filter(|_| !self.items.is_empty())
    }

    fn page_marker(&mut self, id: WidgetId) -> bool {
        let page = self.viewport().max(1) as isize;
        if id == self.more_above_id() {
            self.list.scroll.scroll_by(-page);
            true
        } else if id == self.more_below_id() {
            self.list.scroll.scroll_by(page);
            true
        } else {
            false
        }
    }

    pub fn max_rows(mut self, rows: u16) -> Self {
        self.list.max_rows = rows;
        self
    }

    fn line_of(&self, i: usize) -> usize {
        i + self.items[..i.min(self.items.len())]
            .iter()
            .filter(|it| it.separator_after)
            .count()
    }

    fn lines(&self) -> usize {
        self.line_of(self.items.len())
    }

    fn viewport(&self) -> usize {
        let title = usize::from(self.title.is_some());
        (self.area.height as usize).saturating_sub(2 + title)
    }

    pub fn min_width(mut self, cols: u16) -> Self {
        self.min_width = cols;
        self
    }

    pub fn new(id: WidgetId, items: Vec<MenuItem>) -> Self {
        let cursor = items.iter().position(|i| !i.item.disabled).unwrap_or(0);
        let mut list = OptionList::new(id)
            .look(CursorStyle::Fill)
            .max_rows(u16::MAX)
            .fit(Place::MENU);

        list.cursor = Cursor::new(cursor).wrap(true).skip_disabled(false);

        list.scroll.context = 0;
        list.set_gaps(items.iter().map(|it| it.separator_after));
        Self {
            id,
            items,
            list,
            anchor: Rect::ZERO,
            area: Rect::ZERO,
            title: None,
            pending: crate::ui::layer::ResultQueue::default(),
            hide_owner_focus: true,
            reveal: None,
            min_width: 8,
            anchor_to: None,
            unknown_key: UnknownKey::CloseAndPass,
            release_selects: true,
            release_from: Vec::new(),
            scrolled_tick: None,
        }
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.list.nav = nav;
        self
    }

    pub fn edge_margin(mut self, cells: u16) -> Self {
        self.list.fit.margin = cells;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.list.cursor.wrap = on;
        self
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.list = self.list.type_ahead_timeout(timeout);
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.list.type_ahead = on;
        self
    }

    pub fn skip_disabled(mut self, on: bool) -> Self {
        self.list.cursor.skip_disabled = on;
        self
    }

    pub fn wheel_mode(mut self, mode: WheelMode) -> Self {
        self.list.wheel = mode;
        self
    }

    pub fn fit(mut self, fit: Place) -> Self {
        self.list.fit = fit;
        self
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.list.dismiss = dismiss;
        self
    }

    pub fn hide_owner_focus(mut self, on: bool) -> Self {
        self.hide_owner_focus = on;
        self
    }

    pub fn hides_page_focus(&self) -> bool {
        self.hide_owner_focus
    }

    pub fn anchor(mut self, anchor: Rect, placement: Placement) -> Self {
        self.anchor = anchor;
        self.list.placement = placement;
        self
    }

    pub fn at(self, pos: Position) -> Self {
        self.anchor(Rect::new(pos.x, pos.y, 1, 1), Placement::Below)
    }

    pub fn anchor_control(mut self, id: WidgetId, fallback: Rect) -> Self {
        self.anchor_to = Some(id);
        self.anchor(fallback, Placement::Below)
    }

    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.list.row_id(i)
    }

    fn row_of(&self, id: WidgetId) -> Option<usize> {
        self.list.row_of(id, self.items.len())
    }

    pub(crate) fn contains_id(&self, id: WidgetId) -> bool {
        id == self.id
            || id == backdrop_id(self.id)
            || id == self.more_above_id()
            || id == self.more_below_id()
            || self.row_of(id).is_some()
    }

    pub fn reason_room(&self, more_below: bool) -> usize {
        reason_room(self.size().0, more_below)
    }

    pub fn size(&self) -> (u16, u16) {
        let label_w = self
            .items
            .iter()
            .map(|i| {
                width(&i.item.label) as u16
                    + i.shortcut
                        .as_deref()
                        .map(|s| width(s) as u16 + 3)
                        .unwrap_or(0)
            })
            .max()
            .unwrap_or(4);
        let title_w = self.title.as_ref().map(|t| width(t) as u16).unwrap_or(0);

        let w = label_w.max(title_w).max(self.min_width) + 6;
        let rows = (self.lines() as u16).min(self.list.max_rows.max(1));
        let title_rows = if self.title.is_some() { 1 } else { 0 };
        let h = rows + title_rows + 2;
        (w, h)
    }

    fn placed(&self, screen: Rect) -> Rect {
        let (w, h) = self.size();
        place_with(
            screen,
            self.anchor,
            w,
            h,
            self.list.placement,
            self.list.fit,
        )
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<MenuAction>) {
        if unassigned_chord::<()>(key, &[]) {
            return (Outcome::Ignored, None);
        }

        if self.highlighted().is_none() && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            return (Outcome::Consumed, None);
        }

        if let KeyCode::Char(c) = key.code
            && let Some(i) = self.accelerator(c)
        {
            self.list.select(i, &self.items[..]);
            return (Outcome::Changed, Some(MenuAction::Chosen(i)));
        }
        match self.list.key(key, &self.items[..]) {
            Moved::To(_) => return (Outcome::Changed, None),
            Moved::Stayed(_) => return (Outcome::Consumed, None),

            Moved::Ignored if self.list.nav.action(key).is_some() => {
                return (Outcome::Consumed, None);
            }
            Moved::Ignored => {}
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                let i = self.list.cursor.index;
                if self.items.get(i).is_some_and(|it| !it.item.disabled) {
                    (Outcome::Changed, Some(MenuAction::Chosen(i)))
                } else {
                    (Outcome::Consumed, None)
                }
            }
            KeyCode::Esc if self.list.dismiss.escape => {
                (Outcome::Changed, Some(MenuAction::Dismissed))
            }
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => (Outcome::Consumed, None),

            _ => match self.unknown_key {
                UnknownKey::CloseAndPass => (Outcome::Ignored, Some(MenuAction::Dismissed)),
                UnknownKey::Swallow => (Outcome::Consumed, None),
            },
        }
    }

    fn accelerator(&self, c: char) -> Option<usize> {
        self.items.iter().position(|it| {
            !it.item.disabled
                && it
                    .shortcut
                    .as_deref()
                    .is_some_and(|s| s.chars().eq(std::iter::once(c)))
        })
    }

    pub fn unknown_key(mut self, unknown: UnknownKey) -> Self {
        self.unknown_key = unknown;
        self
    }

    pub fn on_click(&mut self, id: WidgetId) -> Option<MenuAction> {
        if self.page_marker(id) {
            return None;
        }
        match self.row_of(id) {
            Some(i) => self.choose(i),
            None if id == self.id => None,
            None => self.on_click_outside(),
        }
    }

    fn choose(&mut self, i: usize) -> Option<MenuAction> {
        if self.items.get(i).is_none_or(|it| it.item.disabled) {
            return None;
        }
        self.list.select(i, &self.items[..]);
        Some(MenuAction::Chosen(i))
    }

    pub fn on_click_outside(&mut self) -> Option<MenuAction> {
        self.list
            .dismiss
            .outside_click
            .then_some(MenuAction::Dismissed)
    }

    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        self.prepare(screen.intersection(*buf.area()), ctx);
        let menu = self.clone();
        ctx.overlay(move |buf, ctx| {
            ctx.hits.push_barrier();
            crate::ui::popup::backdrop(ctx, menu.id, menu.list.dismiss, *buf.area());
            menu.paint(buf, ctx);
        });
    }

    fn rows_inner(&self, area: Rect) -> Rect {
        let title = u16::from(self.title.is_some());
        Rect::new(
            area.x + 1,
            area.y + 1 + title,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2 + title),
        )
    }

    fn prepare(&mut self, screen: Rect, ctx: &RenderCtx) {
        if let Some(r) = self.anchor_to.and_then(|id| ctx.hits.area_of(id)) {
            self.anchor = r;
        }
        self.area = self.placed(screen).intersection(screen);

        self.list
            .set_gaps(self.items.iter().map(|it| it.separator_after));
        let inner = self.rows_inner(self.area);
        self.list.layout(inner, &self.items[..], ctx);

        let hover = ctx.interaction.live_hover();
        let tick = ctx.interaction.tick;
        let step = if hover == Some(self.more_above_id()) {
            -1
        } else if hover == Some(self.more_below_id()) {
            1
        } else {
            0
        };
        if step == 0 {
            self.scrolled_tick = None;
        } else if self.scrolled_tick != Some(tick) {
            self.scrolled_tick = Some(tick);
            self.list.scroll.scroll_by(step);
        }
    }

    fn paint(&self, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let t = ctx.theme;
        let area = self.area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }

        let bg = t.popover;
        fill(buf, area, Style::new().bg(bg));
        let block = ratatui::widgets::Block::new()
            .borders(ratatui::widgets::Borders::ALL)
            .border_set(t.glyphs.border)
            .border_style(Style::new().fg(t.frame_on(bg)).bg(bg));
        ratatui::widgets::Widget::render(block, area, buf);

        ctx.clickable(self.id, area);
        let inner = Rect::new(
            area.x + 1,
            area.y + 1,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        if let Some(title) = self.title.as_ref().filter(|_| !inner.is_empty()) {
            buf.set_string(
                inner.x + 2,
                inner.y,
                truncate(title, inner.width.saturating_sub(3) as usize),
                t.muted().bg(bg),
            );
        }
        let rows = self.rows_inner(area);
        let marker_row = |y: u16| Rect::new(area.x + 1, y, area.width.saturating_sub(2), 1);
        let marker = |ctx: &RenderCtx, id: WidgetId| {
            let s = ctx.state(id);
            if s.pressed {
                t.primary().bg(bg).add_modifier(Modifier::BOLD)
            } else if s.hovered {
                t.primary().bg(bg)
            } else {
                t.muted().bg(bg)
            }
        };
        if self.list.scroll.offset > 0 {
            let id = self.more_above_id();
            buf.set_string(
                area.right().saturating_sub(3),
                area.y,
                t.glyphs.more_above,
                marker(ctx, id),
            );
            ctx.register_part(self.id, PartRef::OVERFLOW_BEFORE, marker_row(area.y));
        }
        let cursor = self.highlighted();

        for (k, line) in self.list.visible_lines() {
            if k >= rows.height {
                break;
            }
            let y = rows.y + k;
            let Some(i) = line else {
                for x in rows.x + 1..rows.right().saturating_sub(1) {
                    buf.set_string(
                        x,
                        y,
                        t.glyphs.bar_empty,
                        Style::new().fg(t.rule_on(bg)).bg(bg),
                    );
                }
                continue;
            };
            let Some(item) = self.items.get(i) else {
                break;
            };
            let rid = self.row_id(i);
            let mut s = ctx.child_state(self.id, rid, cursor == Some(i));
            s.focused = cursor == Some(i);
            s.disabled = item.item.disabled;
            s.fixed = item.item.disabled && item.item.reason.is_some();
            s.error = item.danger;
            let row = Rect::new(rows.x, y, rows.width, 1);

            let it = crate::ui::row::RowUi::resolve(
                ctx,
                row,
                s,
                crate::ui::row::RowLook::Fill(CursorStyle::Fill),
                bg,
            )
            .look;
            let mut st = it.style;
            if s.fixed && s.focused {
                st = st.fg(t.legible(t.text_muted, st.bg.unwrap_or(bg)));
            }
            if item.danger && !it.on_fill && !item.item.disabled {
                st = st.fg(t.legible(t.error_soft, st.bg.unwrap_or(bg)));
            }
            fill(buf, row, st);
            let label_w = rows.width.saturating_sub(3) as usize;
            buf.set_string(row.x + 2, y, truncate(&item.item.label, label_w), st);
            if !item.item.disabled {
                crate::ui::reveal::truncated(
                    ctx,
                    self.reveal,
                    rid,
                    Rect::new(row.x + 2, y, label_w as u16, 1),
                    &item.item.label,
                );
            }

            if let Some(sc) = item
                .shortcut
                .as_deref()
                .filter(|s| crate::core::keyboard::label_deliverable(s))
            {
                let sw = width(sc) as u16;
                if sw + width(&item.item.label) as u16 + 5 <= rows.width {
                    let scs = if item.item.disabled {
                        st
                    } else {
                        let ink = t.row_ink(&it, t.text_secondary);
                        st.fg(t.legible(ink, st.bg.unwrap_or(bg)))
                            .remove_modifier(Modifier::BOLD)
                    };
                    buf.set_string(row.right().saturating_sub(sw + 1), y, sc, scs);
                }
            }
            ctx.register_part(self.id, OptionList::row_part(i), row);
            if !item.item.disabled {
                if self.release_selects {
                    ctx.hits.accept_release(rid, ReleaseFrom::Secondary);
                }
                for &from in &self.release_from {
                    ctx.hits.accept_release(rid, ReleaseFrom::Press(from));
                }
            }
        }
        let more_below = self.list.more_below(self.items.len());
        if more_below {
            let id = self.more_below_id();
            let yb = area.bottom().saturating_sub(1);
            buf.set_string(
                area.right().saturating_sub(3),
                yb,
                t.glyphs.more_below,
                marker(ctx, id),
            );
            ctx.register_part(self.id, PartRef::OVERFLOW_AFTER, marker_row(yb));
        }

        self.list.paint_status(&self.items, area, bg, buf, t);
    }
}

pub fn reason_room(menu_width: u16, more_below: bool) -> usize {
    usize::from(menu_width).saturating_sub(if more_below { 8 } else { 6 })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuBarAction {
    Opened(usize),

    Chosen(usize, usize),
    Closed,

    Brand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpenOn {
    #[default]
    Press,

    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    #[default]
    Hide,

    Chevron,
}

impl crate::ui::measure::Measure for MenuBar {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let mut w = ctx.theme.spacing.chrome_edge.saturating_sub(1);
        if let Some(b) = &self.brand {
            w += b.width() + 2;
        }
        let titles: u16 = self.labels.iter().map(|l| width(l) as u16 + 3).sum();
        ratatui::layout::Size::new(w + titles.saturating_sub(1), 1)
    }
}

#[derive(Debug, Clone)]
pub struct MenuBar {
    pub id: WidgetId,
    pub labels: Vec<String>,
    pub menus: Vec<Vec<MenuItem>>,
    pub brand: Option<Lockup>,

    pub cursor: usize,
    pub open: Option<ContextMenu>,
    pub areas: Vec<Rect>,
    pub brand_area: Rect,

    pub dismiss: Dismiss,

    pub hover_switch: bool,

    pub wrap: bool,

    pub hide_owner_focus: bool,

    pub nav: NavKeys,

    pub overflow: Overflow,

    pub type_ahead: bool,

    pub type_ahead_timeout: std::time::Duration,

    pub more_area: Rect,

    pub open_on: OpenOn,

    pub press_drag: bool,

    press_handled: Option<WidgetId>,
}

impl HasHints for MenuBar {
    fn hints(&self, _: &HintCx) -> Vec<Hint> {
        if !self.is_open() {
            return vec![];
        }
        let mut v = vec![Hint::new("←→", "Menu")];
        v.extend(nav_hints(&self.nav, self.type_ahead, false));
        v.extend([Hint::new("Enter", "Choose"), Hint::new("Esc", "Close")]);
        v
    }
}

impl MenuBar {
    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn new(id: WidgetId, entries: Vec<(&str, Vec<MenuItem>)>) -> Self {
        Self {
            id,
            labels: entries.iter().map(|(l, _)| (*l).to_owned()).collect(),
            menus: entries.into_iter().map(|(_, m)| m).collect(),
            brand: None,
            cursor: 0,
            open: None,
            areas: vec![],
            brand_area: Rect::ZERO,
            dismiss: Dismiss::LIGHT,
            hover_switch: true,
            wrap: true,
            hide_owner_focus: true,
            nav: NavKeys::default(),
            overflow: Overflow::Hide,
            more_area: Rect::ZERO,
            type_ahead: crate::core::list::NavProfile::current().type_ahead(),
            type_ahead_timeout: crate::core::typeahead::TypeAhead::DEFAULT_TIMEOUT,
            open_on: OpenOn::Press,
            press_drag: true,
            press_handled: None,
        }
    }

    pub fn open_on(mut self, open_on: OpenOn) -> Self {
        self.open_on = open_on;
        self
    }

    pub fn press_drag(mut self, on: bool) -> Self {
        self.press_drag = on;
        self
    }

    pub fn on_press(&mut self, id: WidgetId) -> (Outcome, Option<MenuBarAction>) {
        self.press_handled = None;
        if self.open_on != OpenOn::Press {
            return (Outcome::Ignored, None);
        }
        let Some(i) = self.title_at(id) else {
            return (Outcome::Ignored, None);
        };
        self.press_handled = Some(id);
        if self.open_index() == Some(i) {
            self.close();
            return (Outcome::Changed, Some(MenuBarAction::Closed));
        }
        self.open_menu(i);
        (Outcome::Changed, Some(MenuBarAction::Opened(i)))
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.type_ahead_timeout = timeout;
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.type_ahead = on;
        self
    }

    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn more_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::OVERFLOW))
    }

    fn chevron(&self) -> bool {
        !self.more_area.is_empty()
    }

    pub fn hidden(&self) -> Vec<usize> {
        if !self.chevron() {
            return vec![];
        }
        (0..self.labels.len())
            .filter(|&i| self.areas.get(i).is_none_or(|r| r.is_empty()))
            .collect()
    }

    fn ring(&self) -> Vec<usize> {
        let n = self.labels.len();
        let mut v: Vec<usize> = (0..n)
            .filter(|&i| self.areas.get(i).is_some_and(|r| !r.is_empty()))
            .collect();
        v.push(n);
        v
    }

    fn step(&self, delta: isize) -> usize {
        let ring = self.ring();
        let n = self.labels.len();
        let at = ring
            .iter()
            .position(|&i| i == self.cursor)
            .or_else(|| ring.iter().position(|&i| i == n))
            .unwrap_or(0);
        let titles = RowFn {
            len: ring.len(),
            enabled: &|_| true,
            label: &|_| "",
        };
        let to = self.title_cursor(at).step(delta, &titles).index();
        ring[to.unwrap_or(at)]
    }

    fn title_cursor(&self, at: usize) -> Cursor {
        Cursor::new(at).wrap(self.wrap).axis(Axis::Horizontal)
    }

    fn title_step(&self, delta: isize) -> usize {
        self.title_cursor(self.cursor)
            .step(delta, &self.labels[..])
            .index()
            .unwrap_or(self.cursor)
    }

    fn title_at(&self, id: WidgetId) -> Option<usize> {
        if self.chevron() && id == self.more_id() {
            return Some(self.labels.len());
        }
        self.id.item_of(id, Part::TAB, 0..self.labels.len())
    }

    fn chevron_open(&self) -> bool {
        self.open.is_some()
            && self.chevron()
            && (self.cursor == self.labels.len()
                || self.areas.get(self.cursor).is_none_or(|r| r.is_empty()))
    }

    pub fn hide_owner_focus(mut self, on: bool) -> Self {
        self.hide_owner_focus = on;
        self
    }

    pub fn hides_page_focus(&self) -> bool {
        self.hide_owner_focus && self.open.is_some()
    }

    pub fn dismiss(mut self, dismiss: Dismiss) -> Self {
        self.dismiss = dismiss;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    pub fn hover_switch(mut self, on: bool) -> Self {
        self.hover_switch = on;
        self
    }

    pub fn brand(mut self, lockup: Lockup) -> Self {
        self.brand = Some(lockup);
        self
    }

    pub fn label_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::title_part(i))
    }

    fn title_part(i: usize) -> PartRef {
        PartRef::item(Part::TAB, ItemKey::index(i))
    }

    pub fn brand_id(&self) -> WidgetId {
        self.id.sub("brand")
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub fn open_index(&self) -> Option<usize> {
        let m = self.open.as_ref()?;
        if m.id == self.more_id().sub("menu") {
            return Some(self.labels.len());
        }
        (0..self.labels.len()).find(|i| m.id == self.label_id(*i).sub("menu"))
    }

    pub fn open_menu(&mut self, i: usize) {
        if i == self.labels.len() && self.chevron() {
            self.cursor = i;
            let items = self
                .hidden()
                .into_iter()
                .map(|h| MenuItem::new(self.labels[h].clone()))
                .collect();
            let menu = ContextMenu::new(self.more_id().sub("menu"), items)
                .anchor(self.more_area, Placement::Below)
                .nav(self.nav)
                .type_ahead(self.type_ahead)
                .type_ahead_timeout(self.type_ahead_timeout);
            self.open = Some(menu);
            return;
        }
        if i >= self.labels.len() {
            return;
        }
        self.cursor = i;
        let anchor = match self.areas.get(i).copied() {
            Some(r) if !r.is_empty() => r,

            _ if self.chevron() => self.more_area,
            r => r.unwrap_or(Rect::new(0, 0, 1, 1)),
        };
        let menu = ContextMenu::new(self.label_id(i).sub("menu"), self.menus[i].clone())
            .anchor(anchor, Placement::Below)
            .nav(self.nav)
            .type_ahead(self.type_ahead)
            .type_ahead_timeout(self.type_ahead_timeout);
        self.open = Some(menu);
    }

    pub fn close(&mut self) {
        self.open = None;
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<MenuBarAction>) {
        let n = self.labels.len();
        if unassigned_chord::<()>(key, &[]) {
            return (Outcome::Ignored, None);
        }

        let typing = self.type_ahead && self.open.is_some() && matches!(key.code, KeyCode::Char(_));
        let nav = if typing { None } else { self.nav.action(key) };
        let chevron = self.chevron();
        let step = match nav {
            Some(Nav::Left) => Some(-1),
            Some(Nav::Right) => Some(1),
            _ => None,
        };
        if chevron
            && self.open.is_some()
            && let Some(d) = step
        {
            let i = self.step(d);
            self.open_menu(i);
            return (Outcome::Changed, Some(MenuBarAction::Opened(i)));
        }
        let from_chevron = self.cursor == n && chevron;
        if let Some(menu) = self.open.as_mut() {
            match nav {
                Some(Nav::Left) if n > 1 => {
                    let i = self.title_step(-1);
                    self.open_menu(i);
                    return (Outcome::Changed, Some(MenuBarAction::Opened(i)));
                }
                Some(Nav::Right) if n > 1 => {
                    let i = self.title_step(1);
                    self.open_menu(i);
                    return (Outcome::Changed, Some(MenuBarAction::Opened(i)));
                }
                _ => {}
            }
            let (o, ev) = menu.on_key(key);
            return match ev {
                Some(MenuAction::Chosen(i)) if from_chevron => self.choose_hidden(i),
                Some(MenuAction::Chosen(i)) => {
                    let m = self.cursor;
                    self.close();
                    (Outcome::Changed, Some(MenuBarAction::Chosen(m, i)))
                }

                Some(MenuAction::Dismissed) if o == Outcome::Ignored => {
                    self.close();
                    (Outcome::Ignored, Some(MenuBarAction::Closed))
                }
                Some(MenuAction::Dismissed) if !self.dismiss.escape => (Outcome::Consumed, None),
                Some(MenuAction::Dismissed) => {
                    self.close();
                    (Outcome::Changed, Some(MenuBarAction::Closed))
                }
                None => (o, None),
            };
        }
        match nav {
            Some(nav @ (Nav::Home | Nav::End)) if n > 0 => {
                self.cursor = if chevron {
                    let ring = self.ring();
                    if nav == Nav::Home {
                        ring[0]
                    } else {
                        ring[ring.len() - 1]
                    }
                } else {
                    self.title_cursor(self.cursor)
                        .apply(nav, &self.labels[..], 1)
                        .index()
                        .unwrap_or(self.cursor)
                };
                return (Outcome::Changed, None);
            }
            Some(Nav::Left) if chevron => {
                self.cursor = self.step(-1);
                return (Outcome::Changed, None);
            }
            Some(Nav::Right) if chevron => {
                self.cursor = self.step(1);
                return (Outcome::Changed, None);
            }
            Some(Nav::Left) if n > 0 => {
                self.cursor = self.title_step(-1);
                return (Outcome::Changed, None);
            }
            Some(Nav::Right) if n > 0 => {
                self.cursor = self.title_step(1);
                return (Outcome::Changed, None);
            }
            _ => {}
        }
        match key.code {
            _ if n > 0
                && (matches!(key.code, KeyCode::Enter | KeyCode::Char(' '))
                    || nav == Some(Nav::Down)) =>
            {
                let i = self.cursor;
                self.open_menu(i);
                (Outcome::Changed, Some(MenuBarAction::Opened(i)))
            }
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<MenuBarAction>) {
        let handled = self.press_handled.take();
        if handled.is_some() && handled == Some(id) && self.title_at(id).is_some() {
            return (Outcome::Consumed, None);
        }
        if self.open.is_some() && id == backdrop_id(self.id) {
            if !self.dismiss.outside_click {
                return (Outcome::Consumed, None);
            }
            self.close();
            return (Outcome::Changed, Some(MenuBarAction::Closed));
        }
        if id == self.brand_id() {
            self.close();
            return (Outcome::Changed, Some(MenuBarAction::Brand));
        }
        if let Some(i) = self.title_at(id) {
            if self.open_index() == Some(i) {
                self.close();
                return (Outcome::Changed, Some(MenuBarAction::Closed));
            }
            self.open_menu(i);
            return (Outcome::Changed, Some(MenuBarAction::Opened(i)));
        }
        let from_chevron = self.chevron() && self.cursor == self.labels.len();
        if let Some(menu) = self.open.as_mut() {
            if menu.page_marker(id) {
                return (Outcome::Changed, None);
            }
            return match menu.on_click(id) {
                Some(MenuAction::Chosen(item)) if from_chevron => self.choose_hidden(item),
                Some(MenuAction::Chosen(item)) => {
                    let m = self.cursor;
                    self.close();
                    (Outcome::Changed, Some(MenuBarAction::Chosen(m, item)))
                }
                Some(MenuAction::Dismissed) if !self.dismiss.outside_click => {
                    (Outcome::Consumed, None)
                }
                Some(MenuAction::Dismissed) => {
                    self.close();
                    (Outcome::Changed, Some(MenuBarAction::Closed))
                }
                None => (Outcome::Consumed, None),
            };
        }
        (Outcome::Ignored, None)
    }

    fn choose_hidden(&mut self, i: usize) -> (Outcome, Option<MenuBarAction>) {
        let Some(&title) = self.hidden().get(i) else {
            return (Outcome::Consumed, None);
        };
        self.open_menu(title);
        (Outcome::Changed, Some(MenuBarAction::Opened(title)))
    }

    pub fn on_hover(&mut self, id: Option<WidgetId>) -> Outcome {
        if self.open.is_none() || !self.hover_switch {
            return Outcome::Ignored;
        }
        if let Some(id) = id
            && let Some(i) = self.title_at(id)
            && self.open_index() != Some(i)
            && !(i == self.labels.len() && self.chevron_open())
        {
            self.open_menu(i);
            return Outcome::Changed;
        }
        Outcome::Ignored
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }

        let dragging = ctx
            .interaction
            .pressed
            .is_some_and(|p| self.title_at(p).is_some());
        if !ctx.interaction.motion_unreported || dragging {
            self.on_hover(ctx.interaction.live_hover());
        }
        let t = ctx.theme;

        fill(buf, area, Style::new().bg(bg));
        ctx.control(self.id, area, false);
        let focused = ctx.interaction.focused(self.id);
        let plane = t.plane_of(bg);

        let mut x = area.x + t.spacing.chrome_edge.saturating_sub(1);
        if let Some(b) = &self.brand {
            let w = b.render_clickable(x, area.y, buf, ctx, self.brand_id());
            self.brand_area = Rect::new(x, area.y, w, 1);
            x += w + 2;
        }
        self.areas.clear();
        let open = self.open_index();
        let more_text = format!(" {} ", t.glyphs.more_titles);
        let more_w = width(&more_text) as u16;

        let limit = if self.overflow == Overflow::Chevron && {
            let all: u16 = self
                .labels
                .iter()
                .map(|l| width(l) as u16 + 3)
                .sum::<u16>()
                .saturating_sub(1);
            x + all > area.right()
        } {
            area.right().saturating_sub(more_w + 1)
        } else {
            area.right()
        };
        let chevron_open = self.chevron_open();
        let mut hiding = false;
        for (i, label) in self.labels.iter().enumerate() {
            let lid = self.label_id(i);
            let text = format!(" {label} ");
            let w = width(&text) as u16;
            if hiding || x + w > limit {
                self.areas.push(Rect::ZERO);

                hiding = limit < area.right();
                continue;
            }
            let is_open = open == Some(i);
            let mut s = ctx.state(lid);
            s.focused = focused && self.cursor == i && open.is_none();
            s.selected = is_open;

            let on = if is_open { Plane::Popover } else { plane };
            let it = t.interactive(Kind::Title, s, on);
            if s.focused {
                buf.set_string(
                    x.saturating_sub(1),
                    area.y,
                    it.gutter,
                    it.gutter_style.bg(bg),
                );
            }
            buf.set_string(x, area.y, &text, it.style);
            let r = Rect::new(x, area.y, w, 1);
            self.areas.push(r);
            ctx.register_part(self.id, Self::title_part(i), r);
            x += w + 1;
        }
        self.more_area = Rect::ZERO;
        if limit < area.right() && x + more_w <= area.right() {
            let n = self.labels.len();
            let mid = self.more_id();
            let mut s = ctx.state(mid);
            s.focused = focused && self.cursor == n && open.is_none();
            s.selected = chevron_open;
            let on = if chevron_open { Plane::Popover } else { plane };
            let it = t.interactive(Kind::Title, s, on);
            if s.focused {
                buf.set_string(
                    x.saturating_sub(1),
                    area.y,
                    it.gutter,
                    it.gutter_style.bg(bg),
                );
            }
            buf.set_string(x, area.y, &more_text, it.style);
            self.more_area = Rect::new(x, area.y, more_w, 1);
            ctx.register_part(self.id, PartRef::of(Part::OVERFLOW), self.more_area);
        }
        let Some(m) = self.open.as_mut() else {
            return;
        };

        if let Some(i) = open {
            match self.areas.get(i) {
                Some(r) if !r.is_empty() => m.anchor = *r,
                _ if !self.more_area.is_empty() => m.anchor = self.more_area,
                Some(r) => m.anchor = *r,
                None => {}
            }
        }

        m.release_from = if self.press_drag {
            let mut from: Vec<WidgetId> = (0..self.labels.len())
                .filter(|&i| self.areas.get(i).is_some_and(|r| !r.is_empty()))
                .map(|i| self.id.part(Self::title_part(i)))
                .collect();
            if !self.more_area.is_empty() {
                from.push(self.id.part(PartRef::of(Part::OVERFLOW)));
            }
            from
        } else {
            Vec::new()
        };
        m.prepare(*buf.area(), ctx);
        let menu = m.clone();
        let (id, dismiss) = (self.id, self.dismiss);

        let mut titles: Vec<(PartRef, Rect)> = (0..self.labels.len())
            .map(|i| (Self::title_part(i), self.areas[i]))
            .collect();
        if !self.more_area.is_empty() {
            titles.push((PartRef::of(Part::OVERFLOW), self.more_area));
        }
        let brand = self
            .brand
            .is_some()
            .then(|| (self.brand_id(), self.brand_area));
        ctx.overlay(move |buf, ctx| {
            ctx.hits.push_barrier();
            crate::ui::popup::backdrop(ctx, id, dismiss, *buf.area());
            for (part, r) in titles {
                ctx.register_part(id, part, r);
            }
            if let Some((bid, r)) = brand {
                ctx.clickable(bid, r);
            }
            menu.paint(buf, ctx);
        });
    }
}

impl RowSource for ContextMenu {
    fn len(&self) -> usize {
        self.items.len()
    }
    fn enabled(&self, i: usize) -> bool {
        self.items[..].enabled(i)
    }
    fn label(&self, i: usize) -> &str {
        &self.items[i].item.label
    }
}

impl Navigable for ContextMenu {
    fn cursor(&self) -> Cursor {
        self.list.cursor
    }
}

impl RowSource for MenuBar {
    fn len(&self) -> usize {
        self.labels.len()
    }
    fn label(&self, i: usize) -> &str {
        &self.labels[i]
    }
}

impl Navigable for MenuBar {
    fn cursor(&self) -> Cursor {
        self.title_cursor(self.cursor)
    }
}

impl crate::core::pointer::MouseTarget for ContextMenu {
    type Action = MenuAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<MenuAction> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;
        if ev.wheel().is_some() && self.list.on_backdrop(ev) {
            return Response::ignored();
        }
        if let Some(r) = crate::core::pointer::scroll_input(&mut self.list, ev) {
            return r.lift();
        }
        match ev.phase() {
            Some(Phase::Click) => {
                let action = if let Some(i) = self.list.row_of_event(ev) {
                    self.choose(i)
                } else if self.list.on_backdrop(ev) {
                    self.on_click_outside()
                } else {
                    self.page_marker(ev.id);
                    None
                };
                match action {
                    Some(a) => Response::emit(a),
                    None => Response::changed(),
                }
            }
            Some(Phase::Press | Phase::Drag | Phase::Secondary) => Response::consumed(),
            _ => Response::ignored(),
        }
    }
}

impl crate::core::pointer::MouseTarget for MenuBar {
    type Action = MenuBarAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<MenuBarAction>> {
        let owner = ev.owner();
        let mine = owner == self.id
            || owner == self.brand_id()
            || self.open.as_ref().is_some_and(|m| m.id == owner);
        mine.then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<MenuBarAction> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;
        if let Some((axis, delta)) = ev.wheel() {
            return match self.open.as_mut() {
                Some(m) => m.list.wheel(axis, delta).lift(),
                None => Response::ignored(),
            };
        }
        match ev.phase() {
            Some(Phase::Press) => self.on_press(ev.id).into(),
            Some(Phase::Click) => self.on_click(ev.id).into(),
            _ => Response::ignored(),
        }
    }
}

impl Specimen for ContextMenu {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Disabled | Overflow | Narrow => self.items.len() > 1,
            Focused | FocusedHover | Selected | Busy | Error | Editing | Empty | Loading => false,
        }
    }

    fn specimen_height(&self, state: SpecimenState) -> u16 {
        let mut m = self.clone();
        if state == SpecimenState::Overflow {
            m = m.max_rows(2);
        }
        m.size().1
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let _ = bg;
        let mut m = self
            .clone()
            .anchor(
                Rect::new(area.x, area.y, 1, 0),
                crate::ui::popup::Placement::Below,
            )
            .edge_margin(0);
        m.anchor_to = None;
        let v = state.visual();
        let row = 1.min(m.items.len() - 1);
        if v.disabled {
            m.items[row].item.disabled = true;
        }
        if v.hovered {
            ctx.interaction.hover = Some(m.row_id(row));
        }
        if v.pressed {
            ctx.interaction.pressed = Some(m.row_id(row));
        }
        if state == SpecimenState::Overflow {
            m = m.max_rows(2);
        }
        m.render(area, buf, ctx);
        ctx.flush_overlays(buf);
    }
}

impl Overlay for ContextMenu {
    type Action = MenuAction;
    fn dismiss(&self) -> Dismiss {
        self.list.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.list.dismiss)
            .initial_focus(self.id)
            .hides_page_focus(self.hides_page_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<MenuAction> {
        Response::emit(MenuAction::Dismissed)
    }
}

impl Overlay for MenuBar {
    type Action = MenuBarAction;
    fn dismiss(&self) -> Dismiss {
        self.dismiss
    }
    fn spec(&self) -> LayerSpec {
        LayerSpec::popover(self.dismiss).hides_page_focus(self.hides_page_focus())
    }
    fn on_dismiss(&mut self, _: DismissReason) -> Response<MenuBarAction> {
        self.close();
        Response::emit(MenuBarAction::Closed)
    }
}
