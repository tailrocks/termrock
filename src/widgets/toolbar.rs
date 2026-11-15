use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::NavKeys;
use crate::core::list::{Cursor, Navigable, RowFn, RowSource};
use crate::theme::{ButtonKind, Kind};
use crate::ui::ctx::RenderCtx;
use crate::ui::measure::terminal_extent;
use crate::ui::text::width;
use crate::ui::tooltip::Tip;
use crate::widgets::icon_button::IconButton;
use crate::widgets::menu::{ContextMenu, MenuAction, MenuItem};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolItem {
    pub label: String,

    pub shortcut: Option<String>,
    pub disabled: bool,

    pub on: Option<bool>,

    pub separator_after: bool,

    pub tip: Option<String>,

    pub reason: Option<String>,
}

impl ToolItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            ..Default::default()
        }
    }
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = Some(s.to_owned());
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }

    pub fn toggle(mut self, on: bool) -> Self {
        self.on = Some(on);
        self
    }
    pub fn separator(mut self) -> Self {
        self.separator_after = true;
        self
    }
    pub fn tip(mut self, tip: &str) -> Self {
        self.tip = Some(tip.to_owned());
        self
    }

    pub fn reason(mut self, why: &str) -> Self {
        self.reason = Some(why.to_owned());
        self.disabled = true;
        self
    }

    fn intrinsic_width(&self) -> usize {
        let mut w = 1usize.saturating_add(width(&self.label)).saturating_add(1);
        if self.on.is_some() {
            w = w.saturating_add(2);
        }
        if let Some(s) = &self.shortcut {
            w = w.saturating_add(width(s)).saturating_add(1);
        }
        w
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAction {
    Activated(usize),

    Toggled(usize, bool),
}

/// Scalar fields that shape the rendered geometry or the overflow menu. Direct
/// writes to these public fields are caught by comparing against the key
/// captured at render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ToolbarPolicy {
    overflow: bool,
    gap: u16,
}

#[derive(Debug, Clone)]
pub struct Toolbar {
    pub id: WidgetId,
    items: Vec<ToolItem>,

    pub cursor: usize,

    pub nav: NavKeys,

    pub wrap: bool,

    pub skip_disabled: bool,

    pub gap: u16,

    pub areas: Vec<Rect>,

    pub overflow: bool,

    pub more_button: IconButton,

    menu: Option<(ContextMenu, Vec<usize>)>,

    more_area: Rect,

    hidden: Vec<usize>,

    marks: (&'static str, &'static str),

    /// The scalar policy the last complete render laid out with. `None` after
    /// any collection change, so stale hit targets cannot act.
    layout: Option<ToolbarPolicy>,
}

impl Toolbar {
    pub fn new(id: WidgetId, items: Vec<ToolItem>) -> Self {
        let mut t = Self {
            id,
            items,
            cursor: 0,
            nav: NavKeys::default(),
            wrap: false,
            skip_disabled: true,
            gap: 1,
            areas: Vec::new(),
            overflow: true,
            more_button: IconButton {
                glyph: None,
                ..IconButton::new(WidgetId::of("toolbar.more"), "")
            },
            menu: None,
            more_area: Rect::ZERO,
            hidden: Vec::new(),
            marks: ("●", "○"),
            layout: None,
        };
        t.cursor = (0..t.items.len())
            .find(|&i| !t.items[i].disabled)
            .unwrap_or(0);
        t
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    pub fn skip_disabled(mut self, on: bool) -> Self {
        self.skip_disabled = on;
        self
    }

    pub fn gap(mut self, cells: u16) -> Self {
        self.gap = cells;
        self
    }

    pub fn overflow(mut self, on: bool) -> Self {
        self.overflow = on;
        self
    }

    pub fn more_button(mut self, button: IconButton) -> Self {
        self.more_button = button;
        self
    }

    pub fn more_glyph(mut self, glyph: &str) -> Self {
        self.more_button.glyph = Some(glyph.to_owned());
        self
    }

    pub fn more_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::OVERFLOW))
    }

    pub fn menu_id(&self) -> WidgetId {
        self.id.sub("more-menu")
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    pub fn hidden(&self) -> &[usize] {
        &self.hidden
    }

    pub fn on_more(&self) -> bool {
        self.cursor == self.items.len() && !self.more_area.is_empty()
    }

    pub fn items(&self) -> &[ToolItem] {
        &self.items
    }

    /// Replaces every item. Returns `false` when the collection is unchanged,
    /// which keeps an open menu and the current layout.
    pub fn set_items(&mut self, items: Vec<ToolItem>) -> bool {
        if self.items == items {
            return false;
        }
        self.items = items;
        self.invalidate_items();
        true
    }

    /// Inserts at `index`, which may equal the length. Returns `false` for a
    /// position past the end.
    pub fn insert_item(&mut self, index: usize, item: ToolItem) -> bool {
        if index > self.items.len() {
            return false;
        }
        self.items.insert(index, item);
        self.invalidate_items();
        true
    }

    pub fn remove_item(&mut self, index: usize) -> Option<ToolItem> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        self.invalidate_items();
        Some(item)
    }

    /// Edits a copy of one item and commits it only when the copy differs. A
    /// panicking `update` leaves the collection untouched.
    pub fn update_item(&mut self, index: usize, update: impl FnOnce(&mut ToolItem)) -> bool {
        let Some(current) = self.items.get(index) else {
            return false;
        };
        let mut next = current.clone();
        update(&mut next);
        if next == *current {
            return false;
        }
        self.items[index] = next;
        self.invalidate_items();
        true
    }

    /// Drops every target derived from the collection: the open menu, the
    /// overflow geometry and the hit regions. Only a fresh render re-enables
    /// them.
    fn invalidate_items(&mut self) {
        self.menu = None;
        self.hidden.clear();
        self.more_area = Rect::ZERO;
        self.areas.clear();
        self.layout = None;
        // `items.len()` is the More stop, so it stays a valid position.
        self.cursor = self.cursor.min(self.items.len());
    }

    fn policy(&self) -> ToolbarPolicy {
        ToolbarPolicy {
            overflow: self.overflow,
            gap: self.gap,
        }
    }

    fn layout_current(&self) -> bool {
        self.layout == Some(self.policy())
    }

    /// `overflow` and `gap` are public fields, so a direct write can change
    /// the policy without passing through a checked method. Close the menu on
    /// the next input rather than acting on its stale rows.
    fn close_stale_menu(&mut self) {
        if self.menu.is_some() && !self.layout_current() {
            self.menu = None;
        }
    }

    fn owns(&self, id: WidgetId) -> bool {
        id == self.more_id() || self.item_at(id).is_some()
    }

    fn stops(&self) -> Vec<usize> {
        let n = self.items.len();
        let drawn = |i: usize| self.areas.get(i).is_none_or(|r| !r.is_empty());
        let mut v: Vec<usize> = (0..n).filter(|&i| drawn(i)).collect();
        if !self.more_area.is_empty() {
            v.push(n);
        }
        v
    }

    fn open_menu(&mut self) {
        if self.hidden.is_empty() || !self.layout_current() {
            return;
        }
        let (on, off) = self.marks;
        let rows: Option<Vec<MenuItem>> = self
            .hidden
            .iter()
            .map(|&i| {
                let it = self.items.get(i)?;
                let label = match it.on {
                    Some(true) => format!("{on} {}", it.label),
                    Some(false) => format!("{off} {}", it.label),
                    None => it.label.clone(),
                };
                let mut m = MenuItem::new(label)
                    .disabled(it.disabled)
                    .separator_if(it.separator_after);
                if let Some(sc) = &it.shortcut {
                    m = m.shortcut(sc.clone());
                }
                if it.disabled
                    && let Some(r) = &it.reason
                {
                    m = m.reason(r.clone());
                }
                Some(m)
            })
            .collect();
        // One missing target abandons the whole menu; a partial row list would
        // misalign with the parallel `hidden` indices.
        let Some(rows) = rows else {
            return;
        };
        let menu =
            ContextMenu::new(self.menu_id(), rows).anchor_control(self.more_id(), self.more_area);
        self.menu = Some((menu, self.hidden.clone()));
    }

    fn menu_event(&mut self, ev: Option<MenuAction>) -> Option<ToolbarAction> {
        match ev? {
            MenuAction::Chosen(k) => {
                let (_, rows) = self.menu.take()?;
                let a = self.activate(*rows.get(k)?);

                self.cursor = self.items.len();
                a
            }
            MenuAction::Dismissed => {
                self.menu = None;
                None
            }
        }
    }

    pub fn item_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::ROW, ItemKey::index(i)))
    }

    fn item_at(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::ROW, 0..self.items.len())
    }

    /// Natural width of every item, gap and separator, saturating at `u16::MAX`.
    ///
    /// Overflow decisions use the unsaturated intrinsic width, so a saturated result
    /// never hides the More route.
    pub fn width(&self) -> u16 {
        terminal_extent(self.intrinsic_width())
    }

    /// Display width of every item, measured once per layout.
    fn item_widths(&self) -> Vec<usize> {
        self.items.iter().map(ToolItem::intrinsic_width).collect()
    }

    fn total_width(&self, widths: &[usize]) -> usize {
        let gap = usize::from(self.gap);
        let seps = self.items.iter().filter(|i| i.separator_after).count();
        widths
            .iter()
            .fold(0usize, |a, &w| a.saturating_add(w))
            .saturating_add(gap.saturating_mul(self.items.len().saturating_sub(1)))
            .saturating_add(seps.saturating_mul(1 + gap))
    }

    pub(crate) fn intrinsic_width(&self) -> usize {
        self.total_width(&self.item_widths())
    }

    fn activate(&mut self, i: usize) -> Option<ToolbarAction> {
        let it = self.items.get_mut(i)?;
        if it.disabled {
            return None;
        }
        self.cursor = i;
        let action = match it.on.as_mut() {
            Some(on) => {
                *on = !*on;
                ToolbarAction::Toggled(i, *on)
            }
            None => ToolbarAction::Activated(i),
        };
        // A toggle changes the item's on-mark, so an open menu row and the
        // rendered geometry both show the old state. The action is already
        // captured above.
        if matches!(action, ToolbarAction::Toggled(..)) {
            self.invalidate_items();
        }
        Some(action)
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<ToolbarAction>) {
        self.close_stale_menu();
        if let Some((menu, _)) = self.menu.as_mut() {
            let (o, ev) = menu.on_key(key);
            return (o, self.menu_event(ev));
        }
        if self.items.is_empty() {
            return (Outcome::Ignored, None);
        }
        if self.on_more()
            && key.plain()
            && matches!(
                key.code,
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Down
            )
        {
            self.open_menu();
            return (Outcome::Changed, None);
        }
        if key.plain() && matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
            return match self.activate(self.cursor) {
                Some(a) => (Outcome::Changed, Some(a)),
                None => (Outcome::Consumed, None),
            };
        }
        let Some(nav) = self.nav.action(key).filter(|_| key.plain()) else {
            return (Outcome::Ignored, None);
        };

        let n = self.items.len();
        let stops = self.stops();
        let at = stops
            .iter()
            .position(|&i| i == self.cursor)
            .or_else(|| stops.iter().position(|&i| i == n))
            .unwrap_or(0);
        let enabled = |k: usize| stops[k] == n || !self.items[stops[k]].disabled;
        let label = |k: usize| {
            self.items
                .get(stops[k])
                .map_or("", |it: &ToolItem| it.label.as_str())
        };
        let rows = RowFn {
            len: stops.len(),
            enabled: &enabled,
            label: &label,
        };
        let moved = Cursor::new(at)
            .wrap(self.wrap)
            .axis(Axis::Horizontal)
            .skip_disabled(self.skip_disabled)
            .apply(nav, &rows, 1);
        if let Some(k) = moved.index() {
            self.cursor = stops[k];
        }
        (moved.outcome(), None)
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<ToolbarAction>) {
        self.close_stale_menu();
        if let Some((menu, _)) = self.menu.as_mut()
            && menu.contains_id(id)
        {
            let ev = menu.on_click(id);
            return (Outcome::Changed, self.menu_event(ev));
        }
        if !self.layout_current() && self.owns(id) {
            return (Outcome::Consumed, None);
        }
        if id == self.more_id() {
            self.cursor = self.items.len();
            if self.menu.take().is_none() {
                self.open_menu();
            }
            return (Outcome::Changed, None);
        }

        self.menu = None;
        match self.item_at(id) {
            Some(i) => match self.activate(i) {
                Some(a) => (Outcome::Changed, Some(a)),
                None => (Outcome::Consumed, None),
            },
            None => (Outcome::Ignored, None),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        let n = self.items.len();
        self.areas = vec![Rect::ZERO; n];
        self.hidden.clear();
        self.more_area = Rect::ZERO;
        self.layout = None;
        if area.is_empty() {
            self.menu = None;
            return;
        }
        let t = ctx.theme;
        self.marks = (t.glyphs.dot_on, t.glyphs.dot_off);
        let focused = ctx.interaction.focused_under(self.id, self.menu.is_some());

        ctx.control(self.id, area, false);
        let more = match &self.more_button.glyph {
            Some(_) => self.more_button.clone(),
            None => self.more_button.with_glyph(t.glyphs.more_titles),
        };
        let more_w = more.width(t);
        let widths = self.item_widths();
        let overflowing =
            self.overflow && n > 0 && self.total_width(&widths) > usize::from(area.width);

        let limit = if overflowing {
            area.right().saturating_sub(more_w.saturating_add(self.gap))
        } else {
            area.right()
        };
        let y = area.y;
        let mut x = area.x;

        let mut sep_at: Option<u16> = None;
        for (i, w) in widths.iter().copied().enumerate() {
            crate::record_work!(FitVisits, 1usize);
            let it = &self.items[i];
            if w > usize::from(limit.saturating_sub(x)) {
                if overflowing {
                    self.hidden.extend(i..n);
                    if let Some(sx) = sep_at {
                        buf.set_string(sx, y, " ", ratatui::style::Style::new().bg(bg));
                    }
                }
                break;
            }
            sep_at = None;
            let w = terminal_extent(w);
            let r = Rect::new(x, y, w, 1);
            self.areas[i] = r;
            let iid = self.item_id(i);
            let mut s = ctx.child_state(self.id, iid, i == self.cursor);
            s.focused = focused && i == self.cursor;
            s.disabled = it.disabled;
            s.selected = it.on.unwrap_or(false);
            if it.disabled {
                s.hovered = false;
                s.pressed = false;
            }
            let kind = if it.on.is_some() {
                ButtonKind::Toggle
            } else {
                ButtonKind::Subtle
            };
            let look = t.interactive(Kind::Button(kind), s, t.plane_of(bg));
            let st = look.style;
            crate::ui::ctx::fill(buf, r, st);
            buf.set_string(x, y, look.gutter, look.gutter_style);
            let mut cx = x + 1;
            if let Some(on) = it.on {
                let dot = if on {
                    t.glyphs.dot_on
                } else {
                    t.glyphs.dot_off
                };
                let ink = if it.disabled || s.pressed {
                    st
                } else {
                    st.fg(if on { t.accent } else { t.text_muted })
                };
                buf.set_string(cx, y, dot, ink);
                cx += 2;
            }
            buf.set_string(cx, y, &it.label, st);
            cx += width(&it.label) as u16;
            if let Some(sc) = &it.shortcut {
                let ink = if look.on_fill || it.disabled {
                    st
                } else {
                    st.fg(t.text_muted)
                };
                buf.set_string(cx + 1, y, sc, ink.remove_modifier(Modifier::BOLD));
            }
            ctx.register_part(self.id, PartRef::item(Part::ROW, ItemKey::index(i)), r);
            if let Some(tip) = &it.tip {
                let tip = match &it.shortcut {
                    Some(sc) => Tip::new(tip).shortcut(sc),
                    None => Tip::new(tip),
                };
                crate::ui::tooltip::show(ctx, iid, r, &tip, None);
            }
            x = x.saturating_add(w).saturating_add(self.gap);
            if it.separator_after && x < limit {
                sep_at = Some(x);
                buf.set_string(
                    x,
                    y,
                    t.glyphs.divider,
                    ratatui::style::Style::new().fg(t.border_subtle).bg(bg),
                );
                x = x.saturating_add(1).saturating_add(self.gap);
            }
        }
        if self.hidden.is_empty() {
            self.menu = None;
            self.cursor = self.cursor.min(n.saturating_sub(1));
        } else {
            let mx = area.right().saturating_sub(more_w);
            let host = ratatui::style::Style::new().bg(bg);
            let plate = more.part_of(self.id, PartRef::of(Part::OVERFLOW)).render(
                mx,
                y,
                buf,
                ctx,
                host,
                Some(Tip::new(t.strings.tip_overflow)),
            );
            self.more_area = plate;

            if self.cursor > n || self.hidden.contains(&self.cursor) {
                self.cursor = n;
            }
            if focused && self.cursor == n {
                let mut s = ctx.state(self.more_id());
                s.focused = true;
                let under = buf[(mx, y)].bg;
                buf.set_string(mx, y, t.gutter_symbol(s), t.gutter(s, under, false));
            }
        }
        self.layout = Some(self.policy());
        if let Some((menu, _)) = self.menu.as_mut() {
            menu.render(*buf.area(), buf, ctx);
        }
    }
}

impl RowSource for Toolbar {
    fn len(&self) -> usize {
        self.items.len()
    }
    fn enabled(&self, i: usize) -> bool {
        !self.items[i].disabled
    }
    fn label(&self, i: usize) -> &str {
        &self.items[i].label
    }
}

impl Navigable for Toolbar {
    fn cursor(&self) -> Cursor {
        Cursor::new(self.cursor)
            .wrap(self.wrap)
            .axis(Axis::Horizontal)
            .skip_disabled(self.skip_disabled)
    }
}

impl crate::ui::specimen::Specimen for Toolbar {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: crate::ui::specimen::SpecimenState) -> bool {
        use crate::ui::specimen::SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Narrow => {
                self.items.len() > 1
            }
            Selected => self.items.iter().any(|i| i.on.is_some()),
            Overflow => self.items.len() > 1 && self.overflow,
            Busy | Error | Editing | Empty | Loading => false,
        }
    }

    fn specimen(
        &self,
        state: crate::ui::specimen::SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut t = self.clone();
        for it in &mut t.items {
            it.tip = None;
        }
        let v = state.visual();
        let target = 1.min(t.items.len() - 1);
        if v.hovered {
            ctx.interaction.hover = Some(t.item_id(target));
        }
        if v.pressed {
            ctx.interaction.pressed = Some(t.item_id(target));
        }
        if v.focused {
            t.cursor = target;
        }
        if v.disabled {
            t.items[target].disabled = true;
        }
        if v.selected
            && let Some(it) = t.items.iter_mut().find(|i| i.on.is_some())
        {
            it.on = Some(true);
        }
        let area = if state == crate::ui::specimen::SpecimenState::Overflow {
            Rect {
                width: area.width.min(t.width().saturating_sub(1)),
                ..area
            }
        } else {
            area
        };
        t.render(area, buf, ctx, bg);
    }
}

impl crate::core::pointer::MouseTarget for Toolbar {
    type Action = ToolbarAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<ToolbarAction>> {
        let menu = self.menu_open() && ev.on(self.menu_id());
        (ev.on(self.id) || menu).then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<ToolbarAction> {
        if !ev.is(crate::core::intent::Phase::Click) {
            return crate::core::response::Response::ignored();
        }
        self.on_click(ev.id).into()
    }
}
