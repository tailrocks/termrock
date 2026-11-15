use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::event::{Key, Outcome};
use crate::core::hit::Axis;
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keyed::{KeyedSourceError, KeyedState};
use crate::core::keymap::{
    Actions, KeyBinding, KeyMap, NavKeys, lookup, select_all_bindings, unassigned_chord,
};
use crate::core::list::{
    Cursor, KeyStep, KeyedRowSource, Moved, Navigable, RowSource, TypeAheadPrecedence,
};
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

#[derive(Debug, Clone, PartialEq)]
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

impl ListItem {
    /// The borrowed facet the shared list engine reads.
    pub fn borrowed(&self) -> ListItemRef<'_> {
        ListItemRef {
            label: &self.label,
            meta: self.meta.as_deref(),
            meta_tone: self.meta_tone,
            disabled: self.disabled,
            control: &self.control,
        }
    }
}

impl crate::core::keyed::KeyedRecord for ListItem {
    fn label(&self) -> &str {
        &self.label
    }

    fn enabled(&self) -> bool {
        !self.disabled
    }
}

/// One list row borrowed from its provider.
#[derive(Debug, Clone, Copy)]
pub struct ListItemRef<'a> {
    pub label: &'a str,
    pub meta: Option<&'a str>,
    pub meta_tone: Tone,
    pub disabled: bool,
    pub control: &'a Control,
}

/// A keyed provider of list rows. `enabled(i)` must equal `!item(i).disabled`.
pub trait ListSource: KeyedRowSource {
    fn item(&self, visible: usize) -> ListItemRef<'_>;
}

impl ListSource for crate::core::keyed::KeyedVec<ListItem> {
    fn item(&self, visible: usize) -> ListItemRef<'_> {
        self.value_at(visible)
            .expect("visible row within the store")
            .borrowed()
    }
}

/// What a keyed list reports. A single-select list emits `Activated` for the
/// row an activation chose; any change of the selected key set, including a
/// multi-select toggle, emits `SelectionChanged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListAction {
    Activated(ItemKey),
    SelectionChanged,
}

/// The rows the shared list engine works over: an owned slice or a borrowed
/// keyed provider.
trait ListRows: RowSource {
    fn item(&self, i: usize) -> ListItemRef<'_>;

    /// Identity of the row's hit region.
    fn hit_key(&self, i: usize) -> ItemKey;
}

impl ListRows for [ListItem] {
    fn item(&self, i: usize) -> ListItemRef<'_> {
        self[i].borrowed()
    }

    fn hit_key(&self, i: usize) -> ItemKey {
        ItemKey::index(i)
    }
}

struct Borrowed<'a, S: ?Sized>(&'a S);

impl<S: ListSource + ?Sized> RowSource for Borrowed<'_, S> {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn enabled(&self, i: usize) -> bool {
        self.0.enabled(i)
    }
    fn label(&self, i: usize) -> &str {
        self.0.label(i)
    }
    fn key(&self, i: usize) -> ItemKey {
        self.0.key(i)
    }
}

impl<S: ListSource + ?Sized> ListRows for Borrowed<'_, S> {
    fn item(&self, i: usize) -> ListItemRef<'_> {
        self.0.item(i)
    }

    fn hit_key(&self, i: usize) -> ItemKey {
        self.0.key(i)
    }
}

/// What one engine step did, before it is shaped into an API result.
struct Step {
    outcome: Outcome,
    activated: Option<usize>,
}

impl From<Outcome> for Step {
    fn from(outcome: Outcome) -> Self {
        Self {
            outcome,
            activated: None,
        }
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

    keyed: KeyedState,
    layout_generation: u64,
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
            keyed: KeyedState::new(),
            layout_generation: 0,
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

    /// Runs `f` over the owned items without copying them.
    fn with_items<T>(&mut self, f: impl FnOnce(&mut Self, &[ListItem]) -> T) -> T {
        let items = std::mem::take(&mut self.items);
        let out = f(self, &items);
        self.items = items;
        out
    }

    fn settable<R: ListRows + ?Sized>(rows: &R, i: usize) -> bool {
        i < rows.len() && {
            let item = rows.item(i);
            !item.disabled && !item.control.is_fixed()
        }
    }

    fn select_in<R: ListRows + ?Sized>(&mut self, rows: &R, g: SelectGesture, to: usize) -> bool {
        let order: Vec<usize> = (0..rows.len()).collect();
        self.core
            .selection
            .apply(g, to, &order, |i| Self::settable(rows, i))
    }

    pub fn select(&mut self, g: SelectGesture, to: usize) -> bool {
        self.with_items(|me, rows| me.select_in(rows, g, to))
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

    fn label_width(item: &ListItemRef<'_>, text_w: usize) -> (usize, usize) {
        let label_w = text_w.saturating_sub(2);
        let meta_w = item.meta.map(crate::ui::text::width).unwrap_or(0);
        let meta_w = if label_w.saturating_sub(meta_w + 2) < 12 {
            0
        } else {
            meta_w
        };
        let lw = label_w.saturating_sub(if meta_w > 0 { meta_w + 2 } else { 0 });
        (lw, meta_w)
    }

    fn inline_rows<R: ListRows + ?Sized>(
        &mut self,
        rows: &R,
        ctx: &mut RenderCtx,
        text_w: usize,
        room: u16,
    ) -> Option<Vec<RevealLine>> {
        let cursor = self.core.cursor;
        let item = (cursor < rows.len())
            .then(|| rows.item(cursor))
            .filter(|it| !it.disabled)?;
        let (lw, _) = Self::label_width(&item, text_w);
        let rid = self.hit_id(rows.hit_key(cursor));
        self.core
            .reveal
            .inline(ctx, rid, lw as u16, item.label, true, room)
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
        self.hit_id(ItemKey::index(i))
    }

    fn hit_id(&self, key: ItemKey) -> WidgetId {
        self.id.part(PartRef::item(Part::ROW, key))
    }

    pub fn mark_id(&self, i: usize) -> WidgetId {
        self.id.part(PartRef::item(Part::MARKER, ItemKey::index(i)))
    }

    pub fn checked_count(&self) -> usize {
        self.core.selection.len()
    }

    fn move_cursor<R: ListRows + ?Sized>(&mut self, rows: &R, to: usize, extend: bool) {
        let to = to.min(rows.len().saturating_sub(1));
        let from = self.core.cursor;
        self.core.cursor = to;
        if self.mode == SelectMode::Multi && extend {
            self.select_in(rows, SelectGesture::Extend { from }, to);
        }
        self.core.scroll.ensure_visible(self.core.cursor);
    }

    fn activate_in<R: ListRows + ?Sized>(&mut self, rows: &R, i: usize) -> Step {
        if i >= rows.len() {
            return Outcome::Consumed.into();
        }
        let item = rows.item(i);
        if item.disabled || item.control.refuse() {
            return Outcome::Consumed.into();
        }
        match self.mode {
            SelectMode::Single => self.chosen = Some(i),
            SelectMode::Multi => {
                self.select_in(rows, SelectGesture::Toggle, i);
            }
        }
        Step {
            outcome: Outcome::Changed,
            activated: Some(i),
        }
    }

    pub fn activate(&mut self, i: usize) -> Outcome {
        self.with_items(|me, rows| me.activate_in(rows, i).outcome)
    }

    pub fn on_key(&mut self, key: &Key) -> Outcome {
        self.with_items(|me, rows| me.on_key_in(rows, key).outcome)
    }

    fn on_key_in<R: ListRows + ?Sized>(&mut self, rows: &R, key: &Key) -> Step {
        if rows.len() == 0 {
            return Outcome::Ignored.into();
        }
        if let Some(o) = self.core.reveal.on_key(key) {
            return o.into();
        }
        let shift = key.shift();
        if unassigned_chord(key, &self.keymap) {
            return Outcome::Ignored.into();
        }
        if self.mode == SelectMode::Single && lookup(&self.keymap, key) == Some(ListCmd::SelectAll)
        {
            return Outcome::Ignored.into();
        }

        if let Some(action) = lookup(&self.keymap, key)
            && self.mode == SelectMode::Multi
        {
            return match action {
                ListCmd::ToggleAll => {
                    self.core
                        .selection
                        .toggle_all(0..rows.len(), |i| Self::settable(rows, i));
                    Outcome::Changed
                }
                ListCmd::SelectAll => {
                    if self.select_in(rows, SelectGesture::All, 0) {
                        Outcome::Changed
                    } else {
                        Outcome::Consumed
                    }
                }
            }
            .into();
        }
        match self.core.key_step(key, rows) {
            Some((KeyStep::Type(_), moved)) => {
                if let Moved::To(i) = moved {
                    self.move_cursor(rows, i, false);
                }
                return moved.outcome().into();
            }
            Some((KeyStep::Nav(_), moved)) => {
                let Some(to) = moved.index() else {
                    return Outcome::Ignored.into();
                };
                self.move_cursor(rows, to, shift);
                return Outcome::Changed.into();
            }
            None => {}
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_in(rows, self.core.cursor),
            _ => Outcome::Ignored.into(),
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
        self.with_items(|me, rows| me.on_click_at_in(rows, row, count, mods, on_box).outcome)
    }

    fn on_click_at_in<R: ListRows + ?Sized>(
        &mut self,
        rows: &R,
        row: usize,
        count: u8,
        mods: KeyModifiers,
        on_box: bool,
    ) -> Step {
        if row >= rows.len() {
            return Outcome::Consumed.into();
        }
        let from = self.core.cursor;
        let again = from == row;
        self.core.cursor = row;
        self.core.scroll.pointer_move();
        self.core.scroll.ensure_visible(row);
        match self.mode {
            SelectMode::Single => {
                if self.core.click.activates(count.max(1), again) {
                    return self.activate_in(rows, row);
                }
            }
            SelectMode::Multi => {
                let plain = !mods.intersects(self.toggle_mods | KeyModifiers::SHIFT);
                let g = if plain && self.core.click.toggles(on_box) {
                    SelectGesture::Toggle
                } else {
                    SelectGesture::from_mods(mods, self.toggle_mods, from)
                };

                if g == SelectGesture::Only && rows.item(row).disabled {
                    return Outcome::Changed.into();
                }
                if rows.item(row).control.refuse() {
                    return Outcome::Changed.into();
                }
                self.select_in(rows, g, row);
            }
        }
        Outcome::Changed.into()
    }

    /// The stable-identity state of a list driven through the `*_source`
    /// methods: cursor, selection and pointer capture by key.
    pub fn keyed(&self) -> &KeyedState {
        &self.keyed
    }

    fn bind<S: ListSource + ?Sized>(&mut self, source: &S) -> Result<(), KeyedSourceError> {
        let first = self.keyed.stamp().is_none();
        let out = self.keyed.reconcile(source)?;
        if first || out.data_changed || out.view_changed || out.cursor_changed {
            let n = self.keyed.visible_len();
            self.core.cursor = self
                .keyed
                .cursor_view()
                .unwrap_or(0)
                .min(n.saturating_sub(1));
            let rows = self.keyed.selected_view();
            match self.mode {
                SelectMode::Single => self.chosen = rows.first().copied(),
                SelectMode::Multi => self.core.selection.replace(rows, self.keyed.anchor_view()),
            }
            self.core.scroll.ensure_visible(self.core.cursor);
        }
        Ok(())
    }

    /// Writes the engine's view-position state back to keys.
    fn adopt(&mut self, step: Step, chosen_before: Option<usize>) -> Response<ListAction> {
        let n = self.keyed.visible_len();
        if n > 0 {
            self.keyed.adopt_cursor_view(self.core.cursor);
        }
        let key_at = |keyed: &KeyedState, i: usize| keyed.visible_key(i);
        let selection_changed = match self.mode {
            SelectMode::Single if self.chosen != chosen_before => {
                let keys = self.chosen.and_then(|i| key_at(&self.keyed, i));
                self.keyed.replace_selection(keys)
            }
            SelectMode::Single => false,
            SelectMode::Multi => self
                .keyed
                .adopt_selection_view(self.core.selection.iter(), self.core.selection.anchor()),
        };
        let mut response: Response<ListAction> = step.outcome.into();
        if self.mode == SelectMode::Single
            && let Some(key) = step.activated.and_then(|i| key_at(&self.keyed, i))
        {
            response.push_action(ListAction::Activated(key));
        }
        if selection_changed {
            response.push_action(ListAction::SelectionChanged);
        }
        response
    }

    /// Keyboard input over a borrowed keyed provider.
    ///
    /// The provider is used for this call only. A source that breaks the keyed
    /// contract is rejected and leaves the previous state untouched.
    pub fn on_key_source<S: ListSource + ?Sized>(
        &mut self,
        key: &Key,
        source: &S,
    ) -> Result<Response<ListAction>, KeyedSourceError> {
        self.bind(source)?;
        let chosen = self.chosen;
        let step = self.on_key_in(&Borrowed(source), key);
        Ok(self.adopt(step, chosen))
    }

    /// Pointer input over a borrowed keyed provider. Hit regions carry the row
    /// key, so a press can only complete on the row it started on.
    pub fn on_pointer_source<S: ListSource + ?Sized>(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
        source: &S,
    ) -> Result<Response<ListAction>, KeyedSourceError> {
        use crate::core::intent::Phase;
        self.bind(source)?;
        if let Some(r) = crate::core::pointer::scroll_input(self, ev) {
            return Ok(r.lift());
        }
        let hit = match (ev.key_of(Part::MARKER), ev.key_of(Part::ROW)) {
            (Some(key), _) => Some((key, true)),
            (None, Some(key)) => Some((key, false)),
            _ => None,
        };
        let Some((key, on_box)) = hit.filter(|_| ev.on(self.id)) else {
            return Ok(Response::ignored());
        };
        match ev.phase() {
            Some(Phase::Press) => {
                self.keyed.capture_press(key, self.layout_generation);
                Ok(Response::ignored())
            }
            Some(Phase::Click) => {
                if self.keyed.pressed().is_some()
                    && !self.keyed.release_press(key, self.layout_generation)
                {
                    return Ok(Response::consumed());
                }
                let Some(row) = self.keyed.visible_index(key) else {
                    return Ok(Response::consumed());
                };
                let chosen = self.chosen;
                let step = self.on_click_at_in(
                    &Borrowed(source),
                    row,
                    ev.count().max(1),
                    ev.mods(),
                    on_box,
                );
                Ok(self.adopt(step, chosen))
            }
            _ => Ok(Response::ignored()),
        }
    }

    /// Paints rows borrowed from a keyed provider.
    pub fn render_source<S: ListSource + ?Sized>(
        &mut self,
        area: Rect,
        source: &S,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) -> Result<(), KeyedSourceError> {
        self.bind(source)?;
        let visible = area.intersection(*buf.area());
        if visible != self.area {
            self.layout_generation += 1;
            self.keyed.invalidate_layout(self.layout_generation);
        }
        self.render_in(&Borrowed(source), area, buf, ctx, bg);
        Ok(())
    }

    pub fn track_click(mut self, click: crate::core::scroll::TrackClick) -> Self {
        self.core.scroll.track_click = Some(click);
        self
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, bg: Color) {
        self.with_items(|me, rows| me.render_in(rows, area, buf, ctx, bg));
    }

    fn render_in<R: ListRows + ?Sized>(
        &mut self,
        rows: &R,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        self.core.scroll.begin(ctx.track_click);
        let area = area.intersection(*buf.area());
        if area.is_empty() {
            return;
        }
        self.area = area;
        let t = ctx.theme;
        let focused = ctx.interaction.focused(self.id);
        self.core.reveal.begin(ctx);
        self.core.scroll.set_content(rows.len());
        self.core.scroll.set_viewport(area.height as usize);
        let n = rows.len();

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
            expanded = self.inline_rows(rows, ctx, text_w, area.height);
            if let Some(lines) = &expanded {
                let extra = lines.len().saturating_sub(1);
                self.core
                    .scroll
                    .set_viewport((area.height as usize).saturating_sub(extra).max(1));
                self.core.scroll.ensure_visible(self.core.cursor);
                if self.core.scroll.overflows() != has_sb {
                    has_sb = self.core.scroll.overflows();
                    sp = scrollbar::split(area, has_sb, self.scrollbar_gap);
                    text_w = sp.content.width.saturating_sub(1) as usize;
                    expanded = self.inline_rows(rows, ctx, text_w, area.height);
                }
            }
        }
        let mut y = area.y;
        for li in self.core.scroll.visible_range() {
            if y >= area.bottom() {
                break;
            }
            crate::record_work!(VisibleRowsPainted, 1usize);
            let item = rows.item(li);
            let hit_key = rows.hit_key(li);
            let block = expanded.as_ref().filter(|_| li == self.core.cursor);
            let h = block
                .map_or(1, |r| r.len() as u16)
                .clamp(1, area.bottom() - y);
            let row = Rect::new(area.x, y, sp.body.width, h);
            let rid = self.hit_id(hit_key);
            let mut s = ctx.child_state(self.id, rid, li == self.core.cursor);
            if self.mode == SelectMode::Multi {
                let mark = self.id.part(PartRef::item(Part::MARKER, hit_key));
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
            let (lw, meta_w) = Self::label_width(&item, text_w);
            if let Some(lines) = block {
                for (k, line) in lines.iter().take(h as usize).enumerate() {
                    let ls = if line.more { st.fg(t.text_muted) } else { st };
                    let text = crate::ui::text::fit(&line.text, lw);
                    buf.set_string(row.x + 3, y + k as u16, text, ls);
                }
            } else {
                let label = Rect::new(row.x + 3, y, lw as u16, 1);
                rp.label(ctx, buf, &mut self.core.reveal, rid, label, item.label, st);
            }
            if let Some(m) = item.meta
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
            ctx.register_part(self.id, PartRef::item(Part::ROW, hit_key), row);
            if self.mode == SelectMode::Multi {
                let mark = Rect::new(row.x, row.y, row.width.min(3), row.height);
                ctx.register_part(self.id, PartRef::item(Part::MARKER, hit_key), mark);
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
