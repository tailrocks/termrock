use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::intent::Phase;
use crate::core::keymap::{Nav, NavKeys};
use crate::core::pointer::PointerEvent;
use crate::core::response::Response;
use crate::core::scroll::{ScrollState, TrackClick};
use crate::ui::ctx::{RenderCtx, VisualState, fill};
use crate::ui::text::{paint_clipped, paint_truncated};
use crate::widgets::scrollbar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem {
    pub label: String,

    pub icon: String,

    pub section: String,

    pub badge: Option<String>,

    pub disabled: bool,
}

impl NavItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            icon: String::new(),
            section: String::new(),
            badge: None,
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = icon.to_owned();
        self
    }

    pub fn section(mut self, section: &str) -> Self {
        self.section = section.to_owned();
        self
    }

    pub fn badge(mut self, badge: &str) -> Self {
        self.badge = Some(badge.to_owned());
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    Activated(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NavRow {
    Section(usize),
    Blank,
    Item(usize),
}

#[derive(Debug, Clone)]
pub struct NavList {
    pub id: WidgetId,
    /// Rows by position. Replace the collection with [`NavList::set_items`];
    /// direct edits are safe because every entry point reconciles the indices.
    pub items: Vec<NavItem>,

    /// Index of the activated item. A stale value is clamped on the next key,
    /// click or render.
    pub current: usize,

    /// Index of the keyboard cursor. A stale or disabled-only value is repaired
    /// on the next key, click or render; see `reconcile_indices`.
    pub cursor: usize,

    pub collapsed: bool,

    pub nav: NavKeys,

    pub widths: (u16, u16),

    pub scroll: ScrollState,

    pub inactive_selection: Option<crate::theme::InactiveSelection>,
    area: Rect,
}

impl NavList {
    pub fn new(id: WidgetId, items: Vec<NavItem>) -> Self {
        let cursor = items.iter().position(|i| !i.disabled).unwrap_or(0);
        Self {
            id,
            items,
            current: cursor,
            cursor,
            collapsed: false,
            nav: NavKeys::DEFAULT,
            widths: (24, 6),
            scroll: ScrollState::default(),
            inactive_selection: None,
            area: Rect::ZERO,
        }
    }

    pub fn inactive_selection(mut self, look: crate::theme::InactiveSelection) -> Self {
        self.inactive_selection = Some(look);
        self
    }

    pub fn current(mut self, i: usize) -> Self {
        self.current = i;
        self.cursor = i;
        self
    }

    pub fn collapsed(mut self, on: bool) -> Self {
        self.collapsed = on;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn widths(mut self, open: u16, collapsed: u16) -> Self {
        self.widths = (open, collapsed);
        self
    }

    pub fn item_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::item_part(i))
    }

    fn item_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    pub fn width(&self) -> u16 {
        if self.collapsed {
            self.widths.1
        } else {
            self.widths.0
        }
    }

    fn rows(&self) -> Vec<NavRow> {
        let mut rows = vec![];
        let mut section: Option<&str> = None;
        for (i, it) in self.items.iter().enumerate() {
            if section != Some(it.section.as_str()) {
                if !rows.is_empty() {
                    rows.push(NavRow::Blank);
                }

                if !it.section.is_empty() {
                    rows.push(NavRow::Section(i));
                }
                section = Some(it.section.as_str());
            }
            rows.push(NavRow::Item(i));
        }
        rows
    }

    fn row_of(&self, item: usize) -> usize {
        self.rows()
            .iter()
            .position(|r| *r == NavRow::Item(item))
            .unwrap_or(0)
    }

    /// Brings `cursor` and `current` back to valid positions after the items
    /// changed or a public index was set out of range. Returns whether either
    /// index moved. A valid cursor is kept even when its item is disabled, so
    /// disabling an item never silently activates another one. An out-of-range
    /// cursor falls back to the last enabled item, or to the last row when every
    /// item is disabled (never an activation). The check is O(1) on a valid
    /// state; the search runs only when an index is invalid.
    fn reconcile_indices(&mut self) -> bool {
        let before = (self.cursor, self.current);
        match self.items.len() {
            0 => {
                self.cursor = 0;
                self.current = 0;
            }
            len => {
                self.current = self.current.min(len - 1);
                if self.cursor >= len {
                    self.cursor = self
                        .items
                        .iter()
                        .rposition(|i| !i.disabled)
                        .unwrap_or(len - 1);
                }
            }
        }
        if self.cursor != before.0 {
            self.reveal_cursor();
        }
        before != (self.cursor, self.current)
    }

    /// Outcome for a key that changed nothing except possibly a repaired index.
    fn idle(repaired: bool) -> Outcome {
        if repaired {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Outcome for a recognised key that is a no-op on a valid index.
    fn handled(repaired: bool) -> Outcome {
        if repaired {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    /// Scrolls the cursor row into view. Returns whether the scroll offset moved.
    fn reveal_cursor(&mut self) -> bool {
        let row = self.row_of(self.cursor);
        self.scroll.set_content(self.rows().len());
        self.scroll.ensure_visible(row)
    }

    fn scrolled(moved: bool) -> Outcome {
        if moved {
            Outcome::Changed
        } else {
            Outcome::Consumed
        }
    }

    fn step(&self, steps: isize) -> usize {
        let mut c = self.cursor;
        let mut left = steps.unsigned_abs();
        let mut i = c;
        while left > 0 {
            let next = if steps < 0 {
                i.checked_sub(1)
            } else {
                i.checked_add(1).filter(|&n| n < self.items.len())
            };
            let Some(n) = next else { break };
            i = n;
            if !self.items[i].disabled {
                c = i;
                left -= 1;
            }
        }
        c
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<NavAction>) {
        let repaired = self.reconcile_indices();
        if self.items.is_empty() {
            return (Self::idle(repaired), None);
        }
        if let Some(nav) = self.nav.action(key) {
            let page = (self.scroll.viewport_len.max(1) as isize - 1).max(1);
            let before = self.cursor;
            self.cursor = match nav {
                Nav::Up => self.step(-1),
                Nav::Down => self.step(1),
                Nav::PageUp => self.step(-page),
                Nav::PageDown => self.step(page),
                Nav::Home => self
                    .items
                    .iter()
                    .position(|i| !i.disabled)
                    .unwrap_or(self.cursor),
                Nav::End => self
                    .items
                    .iter()
                    .rposition(|i| !i.disabled)
                    .unwrap_or(self.cursor),
                Nav::Left | Nav::Right => return (Self::idle(repaired), None),
            };
            let scrolled = self.reveal_cursor();
            let moved = repaired || self.cursor != before || scrolled;
            let outcome = if moved {
                Outcome::Changed
            } else {
                Outcome::Consumed
            };
            return (outcome, None);
        }
        match key.code {
            ratatui::crossterm::event::KeyCode::Enter
            | ratatui::crossterm::event::KeyCode::Char(' ') => match self.items.get(self.cursor) {
                Some(item) if !item.disabled => {
                    self.current = self.cursor;
                    (Outcome::Changed, Some(NavAction::Activated(self.cursor)))
                }
                Some(_) => (Self::handled(repaired), None),
                None => (Self::idle(repaired), None),
            },
            _ => (Self::idle(repaired), None),
        }
    }

    /// Replaces the items. Indices are positional: `cursor` and `current` keep
    /// their numbers, clamped to the new length, so a same-length reorder keeps
    /// the row and not the item. Hosts that need a stable selection should find
    /// the new index from their own identity and assign `cursor` and `current`.
    /// Filling an empty list starts on the first enabled item, like
    /// [`NavList::new`]. Replacement never emits an action. Returns `false` when
    /// `items` is equal to the current collection, which leaves all state
    /// untouched.
    pub fn set_items(&mut self, items: Vec<NavItem>) -> bool {
        if self.items == items {
            return false;
        }
        let was_empty = self.items.is_empty();
        self.items = items;
        if was_empty {
            let first = self.items.iter().position(|i| !i.disabled).unwrap_or(0);
            self.cursor = first;
            self.current = first;
        }
        self.reconcile_indices();
        true
    }

    pub fn on_click(&mut self, i: usize) -> (Outcome, Option<NavAction>) {
        self.reconcile_indices();
        if i < self.items.len() && !self.items[i].disabled {
            self.cursor = i;
            self.current = i;
            self.reveal_cursor();
            return (Outcome::Changed, Some(NavAction::Activated(i)));
        }
        (Outcome::Changed, None)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.reconcile_indices();
        let t = ctx.theme;
        let area = area.intersection(*buf.area());
        self.area = area;
        let rows = self.rows();
        self.scroll.set_content(rows.len());
        if area.is_empty() {
            self.scroll.set_viewport(0);
            return;
        }
        self.scroll.set_viewport(usize::from(area.height));
        let focused = ctx.interaction.focused(self.id);
        let has_sb = self.scroll.overflows();

        let row_w = area.width.saturating_sub(1);
        let mut cursor_y = None;
        for (k, ri) in self.scroll.visible_range().enumerate() {
            let y = area.y + k as u16;
            let row = Rect::new(area.x, y, row_w, 1);
            match rows[ri] {
                NavRow::Blank => {}
                NavRow::Section(i) => {
                    if self.collapsed {
                        paint_clipped(buf, row, 1, "····", t.faint().bg(bg));
                    } else {
                        paint_truncated(buf, row, 3, &self.items[i].section, t.faint().bg(bg));
                    }
                }
                NavRow::Item(i) => {
                    let it = &self.items[i];
                    let rid = self.item_id(i);
                    let mut s = ctx.state(rid);
                    s.focused = focused && i == self.cursor;
                    s.disabled = it.disabled;

                    s.selected = i == self.current;
                    s.inactive_selection = self.inactive_selection;
                    if it.disabled {
                        s.hovered = false;
                    }
                    if i == self.cursor {
                        cursor_y = Some(y);
                    }
                    let current = i == self.current;
                    let st = t.row(s, bg);
                    fill(buf, row, st);
                    paint_clipped(
                        buf,
                        row,
                        0,
                        t.gutter_symbol(s),
                        t.gutter(s, st.bg.unwrap_or(bg), false),
                    );
                    if current {
                        let ink = t.chosen_in(VisualState { focused, ..s });
                        paint_clipped(buf, row, 1, "›", st.fg(t.legible(ink, st.bg.unwrap_or(bg))));
                    }
                    let label_style = if it.disabled {
                        st
                    } else if current || s.focused || s.hovered {
                        st.fg(t.text_primary)
                    } else {
                        st.fg(t.text_secondary)
                    };
                    let icon_cell = Rect::new(row.x.saturating_add(3), y, 2, 1).intersection(row);
                    if self.collapsed {
                        paint_clipped(buf, row, 3, &it.icon, label_style);
                    } else {
                        paint_clipped(
                            buf,
                            icon_cell,
                            0,
                            &it.icon,
                            label_style.fg(if it.disabled {
                                t.disabled
                            } else {
                                t.text_muted
                            }),
                        );
                        let content = usize::from(row_w).saturating_sub(5);
                        let badge_w = it.badge.as_deref().map_or(0, crate::ui::text::width);
                        let label_room = if badge_w > 0 {
                            content.saturating_sub(badge_w + 2)
                        } else {
                            content
                        };
                        let label_cell = Rect::new(
                            row.x.saturating_add(5),
                            y,
                            u16::try_from(label_room).unwrap_or(u16::MAX),
                            1,
                        )
                        .intersection(row);
                        paint_truncated(buf, label_cell, 0, &it.label, label_style);
                        if let Some(b) = &it.badge {
                            let room = badge_w.min(content.saturating_sub(1));
                            let room = u16::try_from(room).unwrap_or(u16::MAX);
                            let bs = if it.disabled { st } else { st.fg(t.accent) };
                            let badge_cell =
                                Rect::new(row.right().saturating_sub(room + 1), y, room, 1)
                                    .intersection(row);
                            paint_truncated(buf, badge_cell, 0, b, bs);
                        }
                    }
                    if !it.disabled && !row.is_empty() {
                        ctx.register_part(self.id, Self::item_part(i), row);
                    }
                }
            }
        }
        ctx.scrollable(self.id, area);
        if has_sb {
            crate::ui::fade::scroll_edges_except(
                buf,
                ctx,
                Rect::new(area.x, area.y, row_w, area.height),
                &self.scroll,
                &cursor_y.into_iter().collect::<Vec<_>>(),
            );
        }
        scrollbar::render_divider(
            Rect::new(area.right().saturating_sub(1), area.y, 1, area.height),
            buf,
            ctx,
            self.id,
            &self.scroll,
            focused,
        );
        if !ctx.inert {
            ctx.ring.register(self.id);
        }
    }
}

impl crate::ui::specimen::Specimen for NavList {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        matches!(
            state,
            Rest | Hover
                | Pressed
                | Focused
                | FocusedHover
                | Selected
                | Disabled
                | Overflow
                | Narrow
        ) && !(self.collapsed && state == Overflow)
    }

    fn specimen_height(&self, _state: crate::ui::specimen::SpecimenState) -> u16 {
        self.sample().rows().len() as u16
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use crate::ui::specimen::SpecimenState::*;
        let mut l = self.sample();
        fill(buf, area, ratatui::style::Style::new().bg(bg));
        if l.items.is_empty() {
            return;
        }
        let c = l.cursor;
        match state {
            Selected => l.current = c,
            Disabled => l.items[c].disabled = true,
            Overflow => {
                let it = &mut l.items[c];
                it.label = format!("{} and a label far longer than its row", it.label);
                it.badge = Some("12".into());
            }
            _ => {}
        }

        let v = state.visual();
        if v.hovered || state == Disabled {
            ctx.interaction.hover = Some(l.item_id(c));
            if v.pressed {
                ctx.interaction.pressed = Some(l.item_id(c));
            }
        }
        let w = l.width().min(area.width);
        l.render(Rect::new(area.x, area.y, w, area.height), buf, ctx, bg);
    }
}

impl NavList {
    fn sample(&self) -> Self {
        let mut l = self.clone();
        l.items.truncate(3);
        let section = l
            .items
            .first()
            .map(|i| i.section.clone())
            .unwrap_or_default();
        for it in &mut l.items {
            it.section.clone_from(&section);
            it.disabled = false;
        }
        l.current = 0;
        l.cursor = 1.min(l.items.len().saturating_sub(1));
        l.scroll = ScrollState::default();
        l
    }
}

impl crate::core::pointer::MouseTarget for NavList {
    type Action = NavAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(&mut self, ev: &PointerEvent) -> Option<Response<NavAction>> {
        (ev.on(self.id) || ev.id == scrollbar::id_for(self.id)).then(|| self.pointer(ev))
    }

    fn pointer(&mut self, ev: &PointerEvent) -> Response<NavAction> {
        let bar = scrollbar::id_for(self.id);
        if let Some((axis, delta)) = ev.wheel() {
            return match axis {
                Axis::Vertical if self.scroll.overflows() => {
                    Self::scrolled(self.scroll.scroll_by(delta as isize)).into()
                }
                _ => Response::ignored(),
            };
        }
        if ev.id == bar
            && let Some(o) =
                scrollbar::pointer(ev, self.id, &mut self.scroll, TrackClick::default())
        {
            return o.into();
        }
        match ev.index_of(Part::ROW) {
            Some(i) if ev.is(Phase::Click) && i < self.items.len() => {
                let (o, a) = self.on_click(i);
                Response::from((o, a))
            }
            _ => Response::ignored(),
        }
    }
}
