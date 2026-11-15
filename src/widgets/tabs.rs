use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

use crate::core::event::{Key, Outcome};

use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{Actions, KeyBinding, KeyMap, NavKeys, lookup, unassigned_chord};
use crate::core::list::{Cursor, Navigable, RowSource};
use crate::theme::Kind;
use crate::ui::ctx::RenderCtx;
use crate::ui::tooltip::Tip;
use crate::widgets::icon_button::IconButton;
use crate::widgets::menu::{ContextMenu, MenuAction, MenuItem};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TabItem {
    pub label: String,
    pub dirty: bool,
    pub busy: bool,
    pub error: bool,

    pub done: bool,
    pub closable: bool,

    pub prefix: Option<String>,

    pub suffix: Option<String>,
}

impl TabItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            ..Default::default()
        }
    }
    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
    pub fn prefix(mut self, p: &str) -> Self {
        self.prefix = Some(p.to_owned());
        self
    }
    pub fn suffix(mut self, s: &str) -> Self {
        self.suffix = Some(s.to_owned());
        self
    }

    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    pub fn error(mut self, error: bool) -> Self {
        self.error = error;
        self
    }
}

impl crate::ui::measure::Measure for Tabs {
    fn measure(
        &self,
        _avail: ratatui::layout::Size,
        _ctx: &crate::ui::ctx::RenderCtx,
    ) -> ratatui::layout::Size {
        let tabs: u16 = self.items.iter().map(|it| self.tab_width(it) + 1).sum();
        let new = if self.allow_new { 4 } else { 0 };
        ratatui::layout::Size::new(tabs + new, 2)
    }

    fn min_width(&self, _avail: ratatui::layout::Size, _ctx: &crate::ui::ctx::RenderCtx) -> u16 {
        let widest = self
            .items
            .iter()
            .map(|it| self.tab_width(it))
            .max()
            .unwrap_or(0);
        let more = 3 + self.items.len().to_string().len() as u16;
        widest + 1 + 8 + more + if self.allow_new { 4 } else { 0 }
    }
}

#[derive(Debug, Clone)]
pub struct Tabs {
    pub id: WidgetId,
    items: Vec<TabItem>,
    pub active: usize,
    pub cursor: usize,

    pub first: usize,
    pub areas: Vec<Rect>,

    pub allow_new: bool,

    pub rule: TabRule,

    pub middle_click_closes: bool,

    pub wrap: bool,

    pub nav: NavKeys,

    pub keymap: KeyMap<TabCmd>,

    pub close_button: IconButton,

    pub close_reveal: CloseReveal,

    pub overflow_menu: bool,

    pub more_button: IconButton,

    pub passive: bool,

    pub passive_focused: bool,

    menu: Option<(ContextMenu, Vec<usize>)>,

    more_area: Rect,

    fit: usize,

    max_first: usize,

    revealed: Option<usize>,

    layout_valid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CloseReveal {
    #[default]
    Always,

    Hover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabRule {
    #[default]
    Auto,

    Accent,

    Neutral,

    FocusOnly,
}

pub const DEFAULT_CLOSE_KEYS: &[KeyCode] = &[KeyCode::Char('x'), KeyCode::Delete];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabCmd {
    Close,

    New,

    Jump(usize),

    ShowHidden,
}

impl TabCmd {
    pub fn digit_keys() -> KeyMap<Self> {
        ('1'..='9')
            .enumerate()
            .map(|(i, c)| (KeyBinding::char(c), TabCmd::Jump(i)))
            .collect()
    }
}

impl Actions for TabCmd {
    fn label(self) -> &'static str {
        match self {
            TabCmd::Close => "Close tab",
            TabCmd::New => "New tab",
            TabCmd::Jump(_) => "Jump to tab",
            TabCmd::ShowHidden => "Show hidden",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        let mut map: KeyMap<Self> = DEFAULT_CLOSE_KEYS
            .iter()
            .map(|&code| (KeyBinding::key(code), TabCmd::Close))
            .collect();
        map.push((KeyBinding::char('n'), TabCmd::New));
        map.push((KeyBinding::key(KeyCode::Down), TabCmd::ShowHidden));
        map.extend(Self::digit_keys());
        map
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabAction {
    Activated(usize),
    Close(usize),
    New,
}

impl Tabs {
    pub fn new(id: WidgetId, labels: &[&str]) -> Self {
        Self {
            id,
            items: labels.iter().map(|l| TabItem::new(l)).collect(),
            active: 0,
            cursor: 0,
            first: 0,
            areas: vec![],
            allow_new: false,
            rule: TabRule::Auto,
            middle_click_closes: true,
            wrap: false,
            nav: NavKeys::default(),
            keymap: TabCmd::default_keymap(),
            close_button: IconButton::close(WidgetId::of("tabs.close")),
            close_reveal: CloseReveal::Always,
            overflow_menu: true,
            more_button: IconButton::new(WidgetId::of("tabs.more"), "+"),
            passive: false,
            passive_focused: false,
            menu: None,
            more_area: Rect::ZERO,
            fit: 0,
            max_first: 0,
            revealed: None,
            layout_valid: false,
        }
    }

    pub fn with_items(id: WidgetId, items: Vec<TabItem>) -> Self {
        Self {
            id,
            items,
            active: 0,
            cursor: 0,
            first: 0,
            areas: vec![],
            allow_new: false,
            rule: TabRule::Auto,
            middle_click_closes: true,
            wrap: false,
            nav: NavKeys::default(),
            keymap: TabCmd::default_keymap(),
            close_button: IconButton::close(WidgetId::of("tabs.close")),
            close_reveal: CloseReveal::Always,
            overflow_menu: true,
            more_button: IconButton::new(WidgetId::of("tabs.more"), "+"),
            passive: false,
            passive_focused: false,
            menu: None,
            more_area: Rect::ZERO,
            fit: 0,
            max_first: 0,
            revealed: None,
            layout_valid: false,
        }
    }

    pub const STRIP_H: u16 = 2;

    pub fn layout(area: Rect) -> (Rect, Rect) {
        Self::layout_gap(area, 1)
    }

    pub fn layout_gap(area: Rect, gap: u16) -> (Rect, Rect) {
        let strip_h = Self::STRIP_H.min(area.height);
        let strip = Rect::new(area.x, area.y, area.width, strip_h);
        let off = (Self::STRIP_H + gap).min(area.height);
        let body = Rect::new(area.x, area.y + off, area.width, area.height - off);
        (strip, body)
    }

    pub fn passive(mut self, on: bool) -> Self {
        self.passive = on;
        self
    }

    pub fn passive_focused(mut self, on: bool) -> Self {
        self.passive_focused = on;
        self
    }

    pub fn rule(mut self, rule: TabRule) -> Self {
        self.rule = rule;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.nav = nav;
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<TabCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn digit_jump(mut self, on: bool) -> Self {
        self.keymap.retain(|(_, a)| !matches!(a, TabCmd::Jump(_)));
        if on {
            self.keymap.extend(TabCmd::digit_keys());
        }
        self
    }

    pub fn close_keys(mut self, keys: &[KeyCode]) -> Self {
        self.keymap.retain(|(_, a)| *a != TabCmd::Close);
        self.keymap.extend(
            keys.iter()
                .map(|&code| (KeyBinding::key(code), TabCmd::Close)),
        );
        self
    }

    pub fn new_key(mut self, key: Option<char>) -> Self {
        self.keymap.retain(|(_, a)| *a != TabCmd::New);
        if let Some(c) = key {
            self.keymap.push((KeyBinding::char(c), TabCmd::New));
        }
        self
    }

    pub fn allow_new(mut self, on: bool) -> Self {
        self.allow_new = on;
        self
    }

    pub fn overflow_menu(mut self, on: bool) -> Self {
        self.overflow_menu = on;
        self
    }

    fn overflow_tip(&self, t: &crate::theme::Theme) -> Tip {
        let tip = Tip::new(t.strings.tip_overflow);
        match crate::core::keymap::label_for(&self.keymap, TabCmd::ShowHidden) {
            Some(k) => tip.shortcut(&k),
            None => tip,
        }
    }

    pub fn more_button(mut self, button: IconButton) -> Self {
        self.more_button = button;
        self
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_some()
    }

    pub fn more_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::OVERFLOW))
    }

    pub fn menu_id(&self) -> WidgetId {
        self.id.sub("more-menu")
    }

    fn hidden_tabs(&self) -> Vec<usize> {
        let end = (self.first + self.fit).min(self.items.len());
        (0..self.first.min(self.items.len()))
            .chain(end..self.items.len())
            .collect()
    }

    fn open_menu(&mut self) {
        let tabs = self.hidden_tabs();
        if tabs.is_empty() {
            return;
        }
        let items = tabs
            .iter()
            .map(|&i| {
                let it = &self.items[i];
                match &it.prefix {
                    Some(p) => MenuItem::new(format!("{p} {}", it.label)),
                    None => MenuItem::new(it.label.clone()),
                }
            })
            .collect();
        let menu =
            ContextMenu::new(self.menu_id(), items).anchor_control(self.more_id(), self.more_area);
        self.menu = Some((menu, tabs));
    }

    fn menu_event(&mut self, ev: Option<MenuAction>) -> Option<TabAction> {
        match ev? {
            MenuAction::Chosen(k) => {
                let (_, tabs) = self.menu.take()?;
                let i = *tabs.get(k)?;
                self.activate_index(i)
            }
            MenuAction::Dismissed => {
                self.menu = None;
                None
            }
        }
    }

    pub fn close_target_w(mut self, cells: u16) -> Self {
        self.close_button = self.close_button.plate(cells);
        self
    }

    pub fn close_button(mut self, button: IconButton) -> Self {
        self.close_button = button;
        self
    }

    pub fn close_reveal(mut self, reveal: CloseReveal) -> Self {
        self.close_reveal = reveal;
        self
    }

    pub fn items(&self) -> &[TabItem] {
        &self.items
    }

    /// Replaces the whole collection. An equal replacement changes nothing.
    pub fn set_items(&mut self, items: Vec<TabItem>) {
        if self.items == items {
            return;
        }
        self.items = items;
        self.invalidate_items();
    }

    pub fn push(&mut self, item: TabItem) {
        self.items.push(item);
        self.invalidate_items();
    }

    /// Moves the tab at `from` to `to`. `active` and `cursor` follow the moved tab
    /// or shift with the tabs they pass. Returns `false` when nothing changed.
    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        let len = self.items.len();
        if from >= len || to >= len || from == to {
            return false;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
        let moved = |i: usize| {
            if i == from {
                to
            } else if from < to && i > from && i <= to {
                i - 1
            } else if from > to && i >= to && i < from {
                i + 1
            } else {
                i
            }
        };
        self.active = moved(self.active);
        self.cursor = moved(self.cursor);
        self.invalidate_items();
        true
    }

    /// Edits one tab's metadata. The closure runs on a copy, so a panic leaves
    /// the tab unchanged. Returns `false` when the edit changed nothing.
    pub fn update_item(&mut self, index: usize, edit: impl FnOnce(&mut TabItem)) -> bool {
        let Some(current) = self.items.get(index) else {
            return false;
        };
        let mut next = current.clone();
        edit(&mut next);
        if next == *current {
            return false;
        }
        self.items[index] = next;
        self.invalidate_items();
        true
    }

    /// Closes any open overflow menu and drops geometry taken from the old
    /// collection. Pointer hits stay ignored until the next render.
    fn invalidate_items(&mut self) {
        self.menu = None;
        self.areas.clear();
        self.revealed = None;
        self.fit = 0;
        self.max_first = 0;
        self.layout_valid = false;
        let last = self.items.len().saturating_sub(1);
        self.active = self.active.min(last);
        self.cursor = self.cursor.min(last);
        self.first = self.first.min(last);
    }

    fn activate_index(&mut self, index: usize) -> Option<TabAction> {
        if index >= self.items.len() {
            return None;
        }
        self.set_active(index);
        Some(TabAction::Activated(index))
    }

    pub(crate) fn layout_valid(&self) -> bool {
        self.layout_valid
    }

    pub fn on_wheel(&mut self, dx: i32, dy: i32) -> Outcome {
        if let Some((menu, _)) = self.menu.as_mut() {
            return if dy != 0 {
                crate::core::scroll::Scrollable::wheel(&mut menu.list, Axis::Vertical, dy).outcome()
            } else {
                Outcome::Consumed
            };
        }
        let delta = if dx != 0 { dx } else { dy };
        if delta == 0 || self.max_first == 0 {
            return Outcome::Ignored;
        }
        let to = (self.first as i64 + delta as i64).clamp(0, self.max_first as i64) as usize;
        if to == self.first {
            return Outcome::Consumed;
        }
        self.first = to;
        Outcome::Changed
    }

    pub fn middle_click_closes(mut self, on: bool) -> Self {
        self.middle_click_closes = on;
        self
    }

    pub fn on_middle_click(&self, id: WidgetId) -> Option<TabAction> {
        if !self.middle_click_closes || !self.layout_valid {
            return None;
        }
        let i = self.tab_of(id).or_else(|| self.close_of(id))?;
        self.items[i].closable.then_some(TabAction::Close(i))
    }

    pub fn tab_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::TAB, ItemKey::index(i)))
    }
    pub fn close_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::CLOSE, ItemKey::index(i)))
    }
    pub fn new_id(&self) -> WidgetId {
        self.id.part(PartRef::of(Part::NEW))
    }
    pub fn left_id(&self) -> WidgetId {
        self.id.part(PartRef::OVERFLOW_BEFORE)
    }
    pub fn right_id(&self) -> WidgetId {
        self.id.part(PartRef::OVERFLOW_AFTER)
    }

    fn close_of(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::CLOSE, 0..self.items.len())
    }

    fn tab_of(&self, id: WidgetId) -> Option<usize> {
        self.id.item_of(id, Part::TAB, 0..self.items.len())
    }

    pub fn hidden(&self) -> (usize, usize) {
        let after = self.items.len().saturating_sub(self.first + self.fit);
        (self.first, after)
    }

    pub fn set_active(&mut self, i: usize) {
        if self.items.is_empty() {
            self.active = 0;
            self.cursor = 0;
            return;
        }
        let to = i.min(self.items.len() - 1);
        self.cursor = to;
        if to != self.active {
            self.active = to;
            self.ensure_visible(to);
        }
    }

    pub fn remove(&mut self, i: usize) {
        if i >= self.items.len() {
            return;
        }
        self.items.remove(i);
        if self.active >= self.items.len() {
            self.active = self.items.len().saturating_sub(1);
        } else if self.active > i {
            self.active -= 1;
        }
        self.cursor = self.active;
        self.invalidate_items();
    }

    fn ensure_visible(&mut self, i: usize) {
        if i < self.first {
            self.first = i;
        } else if self.fit > 0 && i >= self.first + self.fit {
            self.first = i + 1 - self.fit;
        }
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<TabAction>) {
        if let Some((menu, _)) = self.menu.as_mut() {
            let (o, ev) = menu.on_key(key);
            return (o, self.menu_event(ev));
        }
        let action = lookup(&self.keymap, key);
        if unassigned_chord(key, &self.keymap) {
            return (Outcome::Ignored, None);
        }
        if self.items.is_empty() {
            return match (action, key.code) {
                (_, KeyCode::Enter) | (Some(TabCmd::New), _) if self.allow_new => {
                    (Outcome::Changed, Some(TabAction::New))
                }
                _ => (Outcome::Ignored, None),
            };
        }
        match action {
            Some(TabCmd::Jump(i)) => {
                return if i < self.items.len() {
                    self.set_active(i);
                    (Outcome::Changed, Some(TabAction::Activated(i)))
                } else {
                    (Outcome::Ignored, None)
                };
            }
            Some(TabCmd::Close) if self.items[self.cursor].closable => {
                return (Outcome::Changed, Some(TabAction::Close(self.cursor)));
            }
            Some(TabCmd::New) if self.allow_new => {
                return (Outcome::Changed, Some(TabAction::New));
            }
            Some(TabCmd::ShowHidden) if self.overflow_menu && !self.hidden_tabs().is_empty() => {
                self.open_menu();
                return (Outcome::Changed, None);
            }
            _ => {}
        }
        let to = match self.nav.action(key).filter(|_| key.plain()) {
            Some(nav) => self.cursor().apply(nav, self, 1).index(),
            None => None,
        };
        if let Some(to) = to {
            self.set_active(to);
            return (Outcome::Changed, Some(TabAction::Activated(self.active)));
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.set_active(self.cursor);
                (Outcome::Changed, Some(TabAction::Activated(self.active)))
            }
            _ => (Outcome::Ignored, None),
        }
    }

    pub fn on_click(&mut self, id: WidgetId) -> (Outcome, Option<TabAction>) {
        if let Some((menu, _)) = self.menu.as_mut()
            && menu.contains_id(id)
        {
            let ev = menu.on_click(id);
            return (Outcome::Changed, self.menu_event(ev));
        }
        if id == self.more_id() {
            if !self.layout_valid {
                return (Outcome::Ignored, None);
            }
            if self.menu.take().is_none() {
                self.open_menu();
            }
            return (Outcome::Changed, None);
        }

        self.menu = None;
        if id == self.new_id() {
            return (Outcome::Changed, Some(TabAction::New));
        }
        if !self.layout_valid {
            return (Outcome::Ignored, None);
        }
        if id == self.left_id() {
            self.first = self.first.saturating_sub(1);
            return (Outcome::Changed, None);
        }
        if id == self.right_id() {
            self.first = (self.first + 1).min(self.max_first);
            return (Outcome::Changed, None);
        }
        if let Some(i) = self.close_of(id) {
            return (Outcome::Changed, Some(TabAction::Close(i)));
        }
        if let Some(i) = self.tab_of(id) {
            self.set_active(i);
            return (Outcome::Changed, Some(TabAction::Activated(i)));
        }
        (Outcome::Ignored, None)
    }

    fn tab_width(&self, it: &TabItem) -> u16 {
        let mut w = 1 + crate::ui::text::width(&it.label) as u16 + 2;
        if let Some(p) = &it.prefix {
            w += crate::ui::text::width(p) as u16 + 1;
        }
        if let Some(x) = &it.suffix {
            w += crate::ui::text::width(x) as u16 + 1;
        }
        if it.dirty || it.busy || it.error || it.done {
            w += 2;
        }
        if it.closable {
            w += 2;
        }
        w
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        let t = ctx.theme;
        let focused = if self.passive {
            self.passive_focused
        } else {
            ctx.interaction.focused_under(self.id, self.menu.is_some())
        };

        if !self.passive {
            ctx.control(self.id, area, false);
            ctx.scrollable(self.id, area);
        }
        self.areas = vec![Rect::ZERO; self.items.len()];
        let y = area.y;
        if area.height >= 2 {
            for xx in area.left()..area.right() {
                buf.set_string(
                    xx,
                    y + 1,
                    t.glyphs.bar_empty,
                    ratatui::style::Style::new().fg(t.border_subtle).bg(bg),
                );
            }
        }

        let new_w: u16 = if self.allow_new { 4 } else { 0 };
        let mut x = area.x;

        let widths: Vec<u16> = self.items.iter().map(|it| self.tab_width(it)).collect();
        let total: u16 = widths.iter().map(|w| w + 1).sum();
        let overflow = total + new_w > area.width;

        let left_w: u16 = if overflow { 4 } else { 0 };
        let right_w: u16 = if overflow { 4 } else { 0 };

        let more_w: u16 = if overflow && self.overflow_menu {
            3 + self.items.len().to_string().len() as u16
        } else {
            0
        };
        let avail = area.width.saturating_sub(left_w + right_w + more_w + new_w);

        self.max_first = {
            let mut used = 0u16;
            let mut f = self.items.len();
            while f > 0 && used + widths[f - 1] < avail {
                used += widths[f - 1] + 1;
                f -= 1;
            }
            if overflow {
                f.min(self.items.len().saturating_sub(1))
            } else {
                0
            }
        };

        let reveal = self.revealed != Some(self.active);
        self.revealed = Some(self.active);
        self.first = self.first.min(self.max_first);
        let mut fit;
        if reveal && self.active < self.first {
            self.first = self.active;
        }
        loop {
            fit = 0;
            let mut used = 0u16;
            for w in widths.iter().skip(self.first) {
                if used + w + 1 > avail {
                    break;
                }
                used += w + 1;
                fit += 1;
            }
            if fit == 0 && !self.items.is_empty() {
                fit = 1;
            }
            if reveal && self.active >= self.first + fit && self.first + 1 < self.items.len() {
                self.first += 1;
                continue;
            }
            break;
        }
        self.fit = fit;
        if overflow {
            let more_left = self.first > 0;
            let st = if more_left {
                ctx.title_style(self.left_id(), bg)
            } else {
                t.faint().bg(bg)
            };
            let text = if more_left {
                format!("{}{:<3}", t.glyphs.overflow_left, self.first.min(99))
            } else {
                "    ".into()
            };
            buf.set_string(x, y, &text, st);
            if more_left {
                ctx.register_part(self.id, PartRef::OVERFLOW_BEFORE, Rect::new(x, y, 4, 1));
            }
            x += 4;
        }

        #[allow(clippy::needless_range_loop)]
        for i in self.first..(self.first + fit).min(self.items.len()) {
            let it = &self.items[i];
            let w = widths[i];
            if x + w > area.right() {
                break;
            }
            let r = Rect::new(x, y, w, 1);
            self.areas[i] = r;
            let tid = self.tab_id(i);

            let mut s = ctx.child_state(self.id, tid, i == self.cursor);
            s.focused = focused && i == self.cursor;
            let active = i == self.active;
            s.selected = active;

            let look = t.interactive(Kind::Tab, s, t.plane_of(bg));
            let st = look.style;
            crate::ui::ctx::fill(buf, r, st);
            if s.focused {
                buf.set_string(x, y, look.gutter, look.gutter_style);
            }
            let mut cx = x + 1;

            let plane = st.bg.unwrap_or(bg);
            if let Some(p) = &it.prefix {
                let label = st.fg.unwrap_or(t.text_primary);
                buf.set_string(
                    cx,
                    y,
                    p,
                    st.fg(t.beside_label(t.text_muted, plane, label))
                        .remove_modifier(Modifier::BOLD),
                );
                cx += crate::ui::text::width(p) as u16 + 1;
            }
            buf.set_string(cx, y, &it.label, st);
            cx += crate::ui::text::width(&it.label) as u16;
            if let Some(x) = &it.suffix {
                buf.set_string(
                    cx + 1,
                    y,
                    x,
                    st.fg(t.legible_mark(
                        if active {
                            t.text_secondary
                        } else {
                            t.text_muted
                        },
                        plane,
                    ))
                    .remove_modifier(Modifier::BOLD),
                );
                cx += crate::ui::text::width(x) as u16 + 1;
            }
            if it.busy {
                buf.set_string(
                    cx + 1,
                    y,
                    t.glyphs.spinner_frame(ctx.interaction.tick),
                    st.fg(t.legible_mark(t.accent, plane)),
                );
                cx += 2;
            } else if it.error {
                let ink = t.tone_on(crate::theme::Tone::Error, plane);
                buf.set_string(cx + 1, y, "!", st.fg(ink));
                cx += 2;
            } else if it.dirty {
                buf.set_string(
                    cx + 1,
                    y,
                    t.glyphs.dirty,
                    st.fg(t.legible_mark(t.warning, plane)),
                );
                cx += 2;
            } else if it.done {
                let ink = t.tone_on(crate::theme::Tone::Success, plane);
                buf.set_string(
                    cx + 1,
                    y,
                    t.glyphs.done,
                    st.fg(ink).remove_modifier(Modifier::BOLD),
                );
                cx += 2;
            }

            let cid = self.close_id(i);
            let show_close = it.closable
                && match self.close_reveal {
                    CloseReveal::Always => true,
                    CloseReveal::Hover => {
                        let cs = ctx.state(cid);
                        active || s.focused || s.hovered || cs.hovered || cs.pressed
                    }
                };
            let close_plate = {
                let cw = self.close_button.width(t).max(1).min(w);
                let left = (cx + 1)
                    .saturating_sub(self.close_button.pad)
                    .max(x)
                    .min(x + w - cw);
                Rect::new(left, y, cw, 1)
            };
            if active && area.height >= 2 {
                let accent = match self.rule {
                    TabRule::Auto => !ctx.interaction.focus_hidden,
                    TabRule::Accent => true,
                    TabRule::Neutral => false,
                    TabRule::FocusOnly => focused,
                };
                let rule = if accent { t.accent } else { t.border_strong };

                for xx in x..x + w {
                    buf.set_string(
                        xx,
                        y + 1,
                        t.glyphs.bar_fill,
                        ratatui::style::Style::new().fg(rule).bg(bg),
                    );
                }
            }
            ctx.register_part(self.id, PartRef::item(Part::TAB, ItemKey::index(i)), r);

            if show_close {
                let tip = Tip::new(t.strings.tip_close_tab);
                let tip = match crate::core::keymap::label_for(&self.keymap, TabCmd::Close) {
                    Some(k) => tip.shortcut(&k),
                    None => tip,
                };
                self.close_button
                    .part_of(self.id, PartRef::item(Part::CLOSE, ItemKey::index(i)))
                    .render_in(close_plate, cx + 1, buf, ctx, st, Some(tip));
            }
            x += w + 1;
        }
        if overflow {
            let hidden_right = self.items.len().saturating_sub(self.first + fit);
            let more_right = hidden_right > 0;
            let rx = area.right().saturating_sub(right_w + more_w + new_w);
            let st = if more_right {
                ctx.title_style(self.right_id(), bg)
            } else {
                t.faint().bg(bg)
            };
            let text = if more_right {
                format!("{:>3}{}", hidden_right.min(99), t.glyphs.overflow_right)
            } else {
                "    ".into()
            };
            buf.set_string(rx, y, &text, st);
            if more_right {
                ctx.register_part(self.id, PartRef::OVERFLOW_AFTER, Rect::new(rx, y, 4, 1));
            }
        }
        if more_w > 0 {
            let hidden = self.hidden_tabs().len();
            let mx = area.right().saturating_sub(more_w + new_w);
            let r = Rect::new(mx, y, more_w, 1);
            self.more_area = r;

            let host = ratatui::style::Style::new().bg(bg);
            crate::ui::ctx::fill(buf, r, host);
            self.more_button
                .with_glyph(&format!("+{hidden}"))
                .part_of(self.id, PartRef::of(Part::OVERFLOW))
                .render_in(r, mx + 1, buf, ctx, host, Some(self.overflow_tip(t)));
        } else {
            self.more_area = Rect::ZERO;

            self.menu = None;
        }
        if self.allow_new {
            let nx = area.right().saturating_sub(new_w);
            let nid = self.new_id();
            let tip = Tip::new(t.strings.tip_new_tab);
            let tip = match crate::core::keymap::label_for(&self.keymap, TabCmd::New) {
                Some(k) => tip.shortcut(&k),
                None => tip,
            };
            IconButton::new(nid, "+")
                .part_of(self.id, PartRef::of(Part::NEW))
                .render(
                    nx,
                    y,
                    buf,
                    ctx,
                    ratatui::style::Style::new().bg(bg),
                    Some(tip),
                );
        }
        self.layout_valid = true;
        if let Some((menu, _)) = self.menu.as_mut() {
            menu.render(*buf.area(), buf, ctx);
        }
    }
}

impl RowSource for Tabs {
    fn len(&self) -> usize {
        self.items.len()
    }
    fn label(&self, i: usize) -> &str {
        &self.items[i].label
    }
}

impl Navigable for Tabs {
    fn cursor(&self) -> Cursor {
        Cursor::new(self.cursor)
            .wrap(self.wrap)
            .axis(Axis::Horizontal)
    }
}

impl crate::core::pointer::MouseTarget for Tabs {
    type Action = TabAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<TabAction>> {
        let menu = self.menu.as_ref().map(|(m, _)| m.id);
        (ev.on(self.id) || menu.is_some_and(|m| ev.on(m))).then(|| self.pointer(ev))
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<TabAction> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;
        if let Some((axis, delta)) = ev.wheel() {
            let step = delta.signum();
            return match axis {
                Axis::Horizontal => self.on_wheel(step, 0).into(),
                Axis::Vertical => self.on_wheel(0, step).into(),
            };
        }
        match ev.phase() {
            Some(Phase::Click) => self.on_click(ev.id).into(),
            Some(Phase::Middle) => match self.on_middle_click(ev.id) {
                Some(a) => Response::emit(a),
                None => Response::ignored(),
            },
            _ => Response::ignored(),
        }
    }
}
