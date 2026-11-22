use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{
    Actions, KeyBinding, KeyMap, NavKeys, lookup, select_all_bindings, unassigned_chord,
};
use crate::core::list::{Cursor, KeyStep, Moved, Navigable, RowSource, TypeAheadPrecedence};
use crate::core::refusal::{Control, Reason};
use crate::core::response::Response;
use crate::core::scroll::{ScrollPane, Scrollable};
use crate::core::selection::{ClickPolicy, RowSelection, SelectGesture};
use crate::theme::Tone;
use crate::ui::ctx::{RenderCtx, fill};
use crate::ui::list_core::ListCore;
use crate::ui::reveal::{Reveal, RevealLine};
use crate::ui::row::{MarkKind, RowLook, RowUi};
use crate::widgets::scrollbar;

#[derive(Debug, Clone)]
pub struct ListItem {
    pub label: String,
    pub meta: Option<String>,

    pub meta_tone: Tone,
    pub disabled: bool,
    pub control: Control,
}

impl ListItem {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_owned(),
            meta: None,
            meta_tone: Tone::Muted,
            disabled: false,
            control: Control::Live,
        }
    }

    pub fn control(mut self, control: Control) -> Self {
        self.control = control;
        self
    }

    pub fn fixed(self, reason: Reason) -> Self {
        self.control(Control::fixed(reason))
    }

    fn settable(&self) -> bool {
        !self.disabled && !self.control.is_fixed()
    }

    pub fn meta(mut self, m: &str) -> Self {
        self.meta = Some(m.to_owned());
        self
    }

    pub fn meta_tone(mut self, tone: Tone) -> Self {
        self.meta_tone = tone;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Single,
    Multi,
}

#[derive(Debug, Clone)]
pub struct ListBox {
    pub id: WidgetId,
    pub items: Vec<ListItem>,
    pub mode: SelectMode,
    pub chosen: Option<usize>,

    pub core: ListCore,
    pub area: Rect,

    pub scrollbar_gap: u16,

    pub toggle_mods: KeyModifiers,

    pub keymap: KeyMap<ListCmd>,
}

impl std::ops::Deref for ListBox {
    type Target = ListCore;
    fn deref(&self) -> &ListCore {
        &self.core
    }
}

impl std::ops::DerefMut for ListBox {
    fn deref_mut(&mut self) -> &mut ListCore {
        &mut self.core
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListCmd {
    ToggleAll,

    SelectAll,
}

impl Actions for ListCmd {
    fn label(self) -> &'static str {
        match self {
            ListCmd::ToggleAll => "Toggle all",
            ListCmd::SelectAll => "Select all",
        }
    }

    fn default_keymap() -> KeyMap<Self> {
        let mut map = vec![(KeyBinding::char('a'), ListCmd::ToggleAll)];
        map.extend(select_all_bindings(ListCmd::SelectAll));
        map
    }
}

impl ListBox {
    pub fn new(id: WidgetId, items: Vec<ListItem>, mode: SelectMode) -> Self {
        let n = items.len();
        Self {
            id,
            items,
            mode,
            chosen: None,
            core: ListCore::new(n),
            area: Rect::ZERO,
            scrollbar_gap: scrollbar::GAP,
            toggle_mods: SelectGesture::toggle_mods(),
            keymap: ListCmd::default_keymap(),
        }
    }

    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.core.nav = nav;
        self
    }

    pub fn inactive_selection(mut self, look: crate::theme::InactiveSelection) -> Self {
        self.core.inactive_selection = Some(look);
        self
    }

    pub fn keymap(mut self, keymap: KeyMap<ListCmd>) -> Self {
        self.keymap = keymap;
        self
    }

    pub fn wrap(mut self, on: bool) -> Self {
        self.core.wrap = on;
        self
    }

    pub fn type_ahead_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.core.typeahead.timeout = timeout;
        self
    }

    pub fn type_ahead(mut self, on: bool) -> Self {
        self.core.type_ahead = on;
        self
    }

    pub fn type_ahead_precedence(mut self, precedence: TypeAheadPrecedence) -> Self {
        self.core.type_ahead_precedence = Some(precedence);
        self
    }

    pub fn skip_disabled(mut self, on: bool) -> Self {
        self.core.skip_disabled = on;
        self
    }

    pub fn click(mut self, click: ClickPolicy) -> Self {
        self.core.click = click;
        self
    }

    pub fn toggle_mods(mut self, mods: KeyModifiers) -> Self {
        self.toggle_mods = mods;
        self
    }

    fn order(&self) -> Vec<usize> {
        (0..self.items.len()).collect()
    }

    pub fn select(&mut self, g: SelectGesture, to: usize) -> bool {
        let order = self.order();
        let items = &self.items;
        self.core.selection.apply(g, to, &order, |i| {
            items.get(i).is_some_and(ListItem::settable)
        })
    }

    pub fn remove_rows(&mut self, rows: &[usize]) -> usize {
        let gone: std::collections::BTreeSet<usize> = rows
            .iter()
            .copied()
            .filter(|&r| r < self.items.len())
            .collect();
        if gone.is_empty() {
            return 0;
        }
        let map = |r: usize| (!gone.contains(&r)).then(|| r - gone.range(..r).count());
        let mut i = 0;
        self.items.retain(|_| {
            let keep = !gone.contains(&i);
            i += 1;
            keep
        });
        self.core.selection.remap(map);
        self.chosen = self.chosen.and_then(map);
        let n = self.items.len();
        self.core.cursor = map(self.core.cursor)
            .unwrap_or_else(|| self.core.cursor - gone.range(..self.core.cursor).count())
            .min(n.saturating_sub(1));
        gone.len()
    }

    pub fn is_checked(&self, i: usize) -> bool {
        self.core.selection.contains(i)
    }

    pub fn set_checked(&mut self, i: usize, on: bool) {
        if i < self.items.len() {
            self.core.selection.set(i, on);
        }
    }

    pub fn mode(mut self, mode: SelectMode) -> Self {
        self.mode = mode;
        self
    }

    fn nav_cursor(&self) -> Cursor {
        Cursor::new(self.core.cursor)
            .wrap(self.core.wrap)
            .skip_disabled(self.core.skip_disabled)
    }

    pub fn scrollbar_gap(mut self, gap: u16) -> Self {
        self.scrollbar_gap = gap;
        self
    }

    pub fn reveal(mut self, config: Reveal) -> Self {
        self.core.reveal.config = Some(config);
        self
    }

    pub fn reveal_hint(&self) -> Option<(&'static str, &'static str)> {
        self.core.reveal.hint()
    }

    fn label_width(item: &ListItem, text_w: usize) -> (usize, usize) {
        let label_w = text_w.saturating_sub(2);
        let meta_w = item
            .meta
            .as_ref()
            .map(|m| crate::ui::text::width(m))
            .unwrap_or(0);
        let meta_w = if label_w.saturating_sub(meta_w + 2) < 12 {
            0
        } else {
            meta_w
        };
        let lw = label_w.saturating_sub(if meta_w > 0 { meta_w + 2 } else { 0 });
        (lw, meta_w)
    }

    fn inline_rows(
        &mut self,
        ctx: &mut RenderCtx,
        text_w: usize,
        room: u16,
    ) -> Option<Vec<RevealLine>> {
        let item = self.items.get(self.core.cursor).filter(|it| !it.disabled)?;
        let (lw, _) = Self::label_width(item, text_w);
        let rid = self.row_id(self.core.cursor);
        let label = item.label.clone();
        self.core
            .reveal
            .inline(ctx, rid, lw as u16, &label, true, room)
    }

    pub fn empty(mut self, state: crate::widgets::empty::EmptyState) -> Self {
        self.core.empty = Some(state);
        self
    }

    pub fn empty_text(mut self, s: &str) -> Self {
        self.core.empty = Some(crate::widgets::empty::EmptyState::new(s));
        self
    }

    pub fn content(mut self, content: crate::ui::content::ContentState) -> Self {
        self.core.content = content;
        self
    }

    pub fn skip_when_empty(mut self, on: bool) -> Self {
        self.core.skip_when_empty = on;
        self
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::row_part(i))
    }

    fn row_part(i: usize) -> PartRef {
        PartRef::item(Part::ROW, ItemKey::index(i))
    }

    pub fn mark_id(&self, i: usize) -> WidgetId {
        self.id.part(Self::mark_part(i))
    }

    fn mark_part(i: usize) -> PartRef {
        PartRef::item(Part::MARKER, ItemKey::index(i))
    }

    pub fn checked_count(&self) -> usize {
        self.core.selection.len()
    }

    fn move_cursor(&mut self, to: usize, extend: bool) {
        let to = to.min(self.items.len().saturating_sub(1));
        let from = self.core.cursor;
        self.core.cursor = to;
        if self.mode == SelectMode::Multi && extend {
            self.select(SelectGesture::Extend { from }, to);
        }
        self.core.scroll.ensure_visible(self.core.cursor);
    }

    pub fn activate(&mut self, i: usize) -> Outcome {
        if i >= self.items.len() || self.items[i].disabled || self.items[i].control.refuse() {
            return Outcome::Consumed;
        }
        match self.mode {
            SelectMode::Single => self.chosen = Some(i),
            SelectMode::Multi => {
                self.select(SelectGesture::Toggle, i);
            }
        }
        Outcome::Changed
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        if self.items.is_empty() {
            return Outcome::Ignored;
        }
        if let Some(o) = self.core.reveal.on_key(key) {
            return o;
        }
        let shift = key.shift();
        if unassigned_chord(key, &self.keymap) {
            return Outcome::Ignored;
        }
        if self.mode == SelectMode::Single && lookup(&self.keymap, key) == Some(ListCmd::SelectAll)
        {
            return Outcome::Ignored;
        }

        if let Some(action) = lookup(&self.keymap, key)
            && self.mode == SelectMode::Multi
        {
            return match action {
                ListCmd::ToggleAll => {
                    let items = &self.items;
                    self.core
                        .selection
                        .toggle_all(0..items.len(), |i| items[i].settable());
                    Outcome::Changed
                }
                ListCmd::SelectAll => {
                    if self.select(SelectGesture::All, 0) {
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    }
                }
            };
        }
        let items = &self.items[..];
        match self.core.key_step(key, items) {
            Some((KeyStep::Type(_), moved)) => {
                if let Moved::To(i) = moved {
                    self.move_cursor(i, false);
                }
                return moved.outcome();
            }
            Some((KeyStep::Nav(_), moved)) => {
                let Some(to) = moved.index() else {
                    return Outcome::Ignored;
                };
                self.move_cursor(to, shift);
                return Outcome::Changed;
            }
            None => {}
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.activate(self.core.cursor),
            _ => Outcome::Ignored,
        }
    }

    pub fn on_click(&mut self, row: usize) -> Outcome {
        self.on_click_count(row, 1)
    }

    pub fn on_click_count(&mut self, row: usize, count: u8) -> Outcome {
        self.on_click_at(row, count, KeyModifiers::NONE, false)
    }

    pub fn on_click_at(
        &mut self,
        row: usize,
        count: u8,
        mods: KeyModifiers,
        on_box: bool,
    ) -> Outcome {
        if row >= self.items.len() {
            return Outcome::Consumed;
        }
        let from = self.core.cursor;
        let again = from == row;
        self.core.cursor = row;
        self.core.scroll.pointer_move();
        self.core.scroll.ensure_visible(row);
        match self.mode {
            SelectMode::Single => {
                if self.core.click.activates(count.max(1), again) {
                    return self.activate(row);
                }
            }
            SelectMode::Multi => {
                let plain = !mods.intersects(self.toggle_mods | KeyModifiers::SHIFT);
                let g = if plain && self.core.click.toggles(on_box) {
                    SelectGesture::Toggle
                } else {
                    SelectGesture::from_mods(mods, self.toggle_mods, from)
                };

                if g == SelectGesture::Only && self.items[row].disabled {
                    return Outcome::Changed;
                }
                if self.items[row].control.refuse() {
                    return Outcome::Changed;
                }
                self.select(g, row);
            }
        }
        Outcome::Changed
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.core.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.core.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        self.core.reveal.begin(ctx);
        self.core.scroll.set_content(self.items.len());
        self.core.scroll.set_viewport(area.height as usize);
        self.core.scroll.follow(self.core.cursor);
        let n = self.items.len();

        if self.core.is_focus_stop(n, focused) {
            ctx.control(self.id, area, false);
            ctx.scrollable(self.id, area);
        }
        if self
            .core
            .draw_content(n, area, buf, ctx, bg, Some(t.strings.empty_list))
        {
            return;
        }
        let mut has_sb = self.core.scroll.overflows();

        let mut sp = scrollbar::split(area, has_sb, self.scrollbar_gap);
        let mut text_w = sp.content.width.saturating_sub(1) as usize;

        let mut expanded = None;
        if focused {
            expanded = self.inline_rows(ctx, text_w, area.height);
            if let Some(rows) = &expanded {
                let extra = rows.len().saturating_sub(1);
                self.core
                    .scroll
                    .set_viewport((area.height as usize).saturating_sub(extra).max(1));
                self.core.scroll.ensure_visible(self.core.cursor);
                if self.core.scroll.overflows() != has_sb {
                    has_sb = self.core.scroll.overflows();
                    sp = scrollbar::split(area, has_sb, self.scrollbar_gap);
                    text_w = sp.content.width.saturating_sub(1) as usize;
                    expanded = self.inline_rows(ctx, text_w, area.height);
                }
            }
        }
        let mut y = area.y;
        for li in self.core.scroll.visible_range() {
            if y >= area.bottom() {
                break;
            }
            let item = &self.items[li];
            let block = expanded.as_ref().filter(|_| li == self.core.cursor);
            let h = block
                .map_or(1, |r| r.len() as u16)
                .clamp(1, area.bottom() - y);
            let row = Rect::new(area.x, y, sp.body.width, h);
            let rid = self.row_id(li);
            let mut s = ctx.child_state(self.id, rid, li == self.core.cursor);
            if self.mode == SelectMode::Multi {
                let mark = self.mark_id(li);
                s.hovered |= ctx.interaction.hovered(mark);
                s.pressed |= ctx.interaction.pressed(mark);
            }
            s.focused = focused && li == self.core.cursor;
            s.fixed = item.control.is_fixed();
            s.disabled = item.disabled || s.fixed;
            s.selected = match self.mode {
                SelectMode::Single => self.chosen == Some(li),
                SelectMode::Multi => self.core.selection.contains(li),
            };
            s.inactive_selection = self.core.inactive_selection;
            let rp = RowUi::paint(ctx, buf, row, s, RowLook::Bar, bg);
            let st = rp.style();
            if s.selected {
                let kind = match self.mode {
                    SelectMode::Single => MarkKind::Current,
                    SelectMode::Multi => MarkKind::Tick,
                };
                rp.mark(ctx, buf, row.x + 1, y, kind, focused);
            }
            let label_w = text_w.saturating_sub(2);
            let (lw, meta_w) = Self::label_width(item, text_w);
            if let Some(rows) = block {
                for (k, line) in rows.iter().take(h as usize).enumerate() {
                    let ls = if line.more { st.fg(t.text_muted) } else { st };
                    let text = crate::ui::text::fit(&line.text, lw);
                    buf.set_string(row.x + 3, y + k as u16, text, ls);
                }
            } else {
                let label = Rect::new(row.x + 3, y, lw as u16, 1);
                rp.label(ctx, buf, &mut self.core.reveal, rid, label, &item.label, st);
            }
            if let Some(m) = &item.meta
                && meta_w > 0
                && meta_w + 4 < label_w
            {
                let mx = sp
                    .content
                    .right()
                    .min(row.right().saturating_sub(1))
                    .saturating_sub(meta_w as u16);
                let ms = if s.disabled {
                    st
                } else if item.meta_tone == Tone::Muted {
                    st.fg(t.text_muted)
                } else {
                    st.fg(t.tone_on(item.meta_tone, st.bg.unwrap_or(t.canvas)))
                };
                buf.set_string(mx, y, m, ms);
            }
            ctx.register_part(self.id, Self::row_part(li), row);
            if self.mode == SelectMode::Multi {
                let mark = Rect::new(row.x, row.y, row.width.min(3), row.height);
                ctx.register_part(self.id, Self::mark_part(li), mark);
            }
            y += h;
        }

        ctx.scrollable(self.id, area);
        if let Some(sb) = sp.track {
            crate::ui::fade::scroll_edges_except(
                buf,
                ctx,
                sp.body,
                &self.core.scroll,
                &self
                    .scroll
                    .visible_range()
                    .position(|i| i == self.core.cursor)
                    .map(|k| area.y + k as u16)
                    .into_iter()
                    .collect::<Vec<_>>(),
            );
            self.core.scroll.draw_v(sb, buf, ctx, self.id, focused);
        }
    }
}

impl crate::ui::measure::Measure for ListBox {
    fn measure(&self, avail: ratatui::layout::Size, ctx: &RenderCtx) -> ratatui::layout::Size {
        if self.items.is_empty() {
            return match &self.core.empty {
                Some(e) => e.measure(avail, ctx),
                None => ratatui::layout::Size::new(0, 0),
            };
        }
        let width = self
            .items
            .iter()
            .map(|item| {
                let label = crate::ui::text::width(&item.label) as u16;

                let text = match item.meta.as_deref().filter(|m| !m.is_empty()) {
                    Some(m) => 2 + label.max(12) + 2 + crate::ui::text::width(m) as u16,
                    None => 2 + label,
                };
                text + 1
            })
            .max()
            .unwrap_or(0);
        ratatui::layout::Size::new(width, self.items.len() as u16)
    }
}

impl Scrollable for ListBox {
    fn pane(&mut self) -> &mut ScrollPane {
        &mut self.core.scroll
    }

    fn wheel_first(&mut self, axis: Axis, delta: i32) -> Option<Response<()>> {
        if axis != Axis::Vertical {
            return None;
        }
        self.core.reveal.on_wheel(delta).map(Response::from)
    }
}

impl RowSource for [ListItem] {
    fn len(&self) -> usize {
        <[ListItem]>::len(self)
    }
    fn enabled(&self, i: usize) -> bool {
        !self[i].disabled
    }
    fn label(&self, i: usize) -> &str {
        &self[i].label
    }
}

impl RowSource for ListBox {
    fn len(&self) -> usize {
        self.items.len()
    }
    fn enabled(&self, i: usize) -> bool {
        self.items[..].enabled(i)
    }
    fn label(&self, i: usize) -> &str {
        &self.items[i].label
    }
}

impl Navigable for ListBox {
    fn cursor(&self) -> Cursor {
        self.nav_cursor()
    }
}

impl crate::ui::specimen::Specimen for ListBox {
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
                | Empty
                | Loading
                | Error
                | Overflow
                | Narrow
        )
    }

    fn specimen_height(&self, state: crate::ui::specimen::SpecimenState) -> u16 {
        use crate::ui::specimen::SpecimenState::*;
        if matches!(state, Empty | Loading | Error) {
            1
        } else {
            self.items.len().clamp(1, 3) as u16
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
        use crate::ui::specimen::SpecimenState::*;
        let mut l = self.clone();
        l.items.truncate(3);
        l.selection = RowSelection::new();
        l.chosen = None;
        l.cursor = 1.min(l.items.len().saturating_sub(1));
        l.scroll = ScrollPane::new(l.items.len());
        let c = l.cursor;
        match state {
            Selected => match l.mode {
                SelectMode::Single => l.chosen = Some(c),
                SelectMode::Multi => l.set_checked(c, true),
            },
            Disabled => {
                if let Some(it) = l.items.get_mut(c) {
                    it.disabled = true;
                }
            }
            Empty => {
                l.items.clear();
                l.selection = RowSelection::new();
            }
            Loading | Error => l.content = crate::ui::content::specimen(state),
            Overflow => {
                if let Some(it) = l.items.get_mut(c) {
                    it.label = format!("{} and a label far longer than its row", it.label);
                }
            }
            _ => {}
        }

        let v = state.visual();
        if (v.hovered || state == Disabled) && l.has_rows(l.items.len()) {
            ctx.interaction.hover = Some(l.row_id(c));
            if v.pressed {
                ctx.interaction.pressed = Some(l.row_id(c));
            }
        }
        fill(buf, area, ratatui::style::Style::new().bg(bg));
        l.render(area, buf, ctx, bg);
    }
}

impl crate::core::pointer::MouseTarget for ListBox {
    type Action = ();

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(&mut self, ev: &crate::core::pointer::PointerEvent) -> Response<()> {
        use crate::core::intent::Phase;
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return r;
        }
        if !ev.is(Phase::Click) {
            return Response::ignored();
        }
        let hit = match (ev.index_of(Part::MARKER), ev.index_of(Part::ROW)) {
            (Some(row), _) => Some((row, true)),
            (None, Some(row)) => Some((row, false)),
            _ => None,
        };
        match hit {
            Some((row, on_box)) => self
                .on_click_at(row, ev.count().max(1), ev.mods(), on_box)
                .into(),
            None => Response::ignored(),
        }
    }
}
