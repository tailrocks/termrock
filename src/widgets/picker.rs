use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::core::edit::TextCmd;
use crate::core::event::{Key, Outcome};
use crate::core::hints::{HasHints, HintCx, nav_hints};
use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
use crate::core::keymap::{
    Actions, KeyBinding, KeyMap, KeyboardCapabilities, Nav, NavKeys, deliverable_binding_for_with,
};
use crate::core::list::{Cursor, KeyStep, Navigable, RowSource};
use crate::theme::CursorStyle;
use crate::ui::columns::{self, Col};
use crate::ui::ctx::{Backdrop, RenderCtx, fill};
use crate::ui::edit_menu::{
    EditCaps, EditMenu, EditMenuHost, edit_menu_key, edit_menu_pointer, edit_menu_route,
    point_field,
};
use crate::ui::popup::{Placement, place_with};
use crate::ui::text::{truncate, wider_than, width};
use crate::widgets::edit::EditField;
use crate::widgets::keyhint::Hint;
use crate::widgets::option_list::{Item, OptionList};

pub const DEFAULT_GLYPH: &str = "·";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickerCmd {
    Toggle,
}

impl Actions for PickerCmd {
    fn default_keymap() -> KeyMap<Self> {
        vec![(KeyBinding::key(KeyCode::Insert), Self::Toggle)]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Toggle => "Toggle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum PickerKeyProfile {
    #[default]
    Desktop,
    Vim,
}

thread_local! {
    static PICKER_KEY_PROFILE: std::cell::Cell<PickerKeyProfile> = const { std::cell::Cell::new(PickerKeyProfile::Desktop) };
}

impl PickerKeyProfile {
    pub fn install(self) {
        PICKER_KEY_PROFILE.with(|profile| profile.set(self));
    }

    pub fn current() -> Self {
        PICKER_KEY_PROFILE.with(std::cell::Cell::get)
    }

    fn toggle_keymap(self, searchable: bool) -> KeyMap<PickerCmd> {
        let binding = match self {
            Self::Desktop if searchable => KeyBinding::key(KeyCode::Insert),
            Self::Desktop => KeyBinding::char(' '),
            Self::Vim => KeyBinding::ctrl('t'),
        };
        vec![(binding, PickerCmd::Toggle)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PickerStatus {
    #[default]
    Ready,

    Loading(String),

    Error {
        message: String,
        detail: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Picker {
    pub id: WidgetId,
    pub status: PickerStatus,
    pub title: String,
    pub placeholder: String,

    items: Vec<Item>,

    authoritative_items: Vec<Item>,

    list: OptionList,

    pub anchor: Rect,

    pub hints: String,
    hints_overridden: bool,

    pub backdrop: Backdrop,

    pub scope: Option<String>,
    pub empty_text: String,

    strings: crate::theme::Strings,

    pub empty: Option<crate::widgets::empty::EmptyState>,
    pub area: Rect,

    pub reveal: Option<crate::ui::reveal::Reveal>,

    pub(crate) pending: Vec<PickerAction>,

    pub delete_is_secondary: bool,
    multi: bool,

    pub choose_hint: std::borrow::Cow<'static, str>,

    pub edit_menu: EditMenu,

    toggle_keymap: KeyMap<PickerCmd>,
    toggle_keymap_overridden: bool,
    key_profile: PickerKeyProfile,
    key_profile_configured: bool,
}

impl HasHints for Picker {
    fn hints(&self, cx: &HintCx) -> Vec<Hint> {
        let searchable = self.is_searchable();
        let mut v = vec![];
        if searchable {
            v.push(Hint::new("Type", "Filter"));
        }

        v.extend(
            nav_hints(&self.list.nav, searchable, false)
                .into_iter()
                .filter(|h| h.key != "Type"),
        );
        if self.multi
            && let Some(binding) =
                deliverable_binding_for_with(&self.toggle_keymap, PickerCmd::Toggle, cx.keyboard)
        {
            let label = if binding.code == KeyCode::Insert && binding.mods.is_empty() {
                "Insert".to_owned()
            } else {
                binding.label_on(cx.platform)
            };
            v.push(Hint::new(label, "Toggle").click_key(Key {
                code: binding.code,
                mods: binding.mods,
            }));
        }
        v.push(Hint::new(
            "Enter",
            if self.multi {
                std::borrow::Cow::Borrowed("Confirm")
            } else {
                self.choose_hint.clone()
            },
        ));
        if !self.query().is_empty() {
            v.push(Hint::new("Esc", "Clear"));
        } else if self.list.dismiss.escape {
            v.push(Hint::new("Esc", self.strings.hint_cancel));
        }
        v
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerAction {
    QueryChanged,
    Chosen(usize),
    ChosenMany(Vec<ItemKey>),
    Toggled,
    SelectionConflict {
        attempted: ItemKey,
        selected: ItemKey,
    },

    ChosenAlt(usize),

    Submit,

    Secondary(usize),

    NextScope,

    PrevScope,
    Cancelled,

    Back,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerItemsError {
    DuplicateKey(ItemKey),
    MissingStableKey,
    UnknownVisibleKey(ItemKey),
}

impl Picker {
    pub const MAX_ROWS: u16 = 12;

    pub const WIDTH: u16 = 64;

    pub fn new(id: WidgetId, title: &str) -> Self {
        let list = OptionList::new(id)
            .query(EditField::single(""))
            .max_rows(Self::MAX_ROWS)
            .width(Self::WIDTH, Self::WIDTH)
            .placement(Placement::Center)
            .hover_moves_cursor(false);
        let key_profile = PickerKeyProfile::current();
        let key_profile_configured = key_profile != PickerKeyProfile::Desktop;
        Self {
            id,
            status: PickerStatus::Ready,
            title: title.to_owned(),
            placeholder: crate::theme::Strings::DEFAULT.search_placeholder.into(),
            items: vec![],
            authoritative_items: vec![],
            list,
            anchor: Rect::ZERO,
            hints: String::new(),
            hints_overridden: false,
            backdrop: Backdrop::DEFAULT,
            scope: None,
            empty_text: crate::theme::Strings::DEFAULT.no_matches.into(),
            strings: crate::theme::Strings::DEFAULT,
            empty: None,
            area: Rect::ZERO,
            reveal: None,
            pending: Vec::new(),
            delete_is_secondary: true,
            multi: false,
            choose_hint: std::borrow::Cow::Borrowed("Choose"),
            edit_menu: EditMenu::default(),
            toggle_keymap: key_profile.toggle_keymap(true),
            toggle_keymap_overridden: false,
            key_profile,
            key_profile_configured,
        }
    }

    pub fn choose_hint(mut self, action: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.choose_hint = action.into();
        self
    }

    pub fn multi_select(mut self, on: bool) -> Result<Self, PickerItemsError> {
        if on && let Some(error) = Self::validate_items(&self.items, self.is_searchable()) {
            return Err(error);
        }
        self.multi = on;
        if on {
            self.choose_hint = std::borrow::Cow::Borrowed("Confirm");
        } else {
            self.list.checked.clear();
        }
        Ok(self)
    }

    pub fn keymap(mut self, keymap: KeyMap<PickerCmd>) -> Self {
        self.toggle_keymap = keymap;
        self.toggle_keymap_overridden = true;
        self
    }

    pub fn toggle_binding(mut self, binding: KeyBinding) -> Self {
        self.toggle_keymap = vec![(binding, PickerCmd::Toggle)];
        self.toggle_keymap_overridden = true;
        self
    }

    pub fn key_profile(mut self, profile: PickerKeyProfile) -> Self {
        self.key_profile = profile;
        self.key_profile_configured = true;
        if !self.toggle_keymap_overridden {
            self.toggle_keymap = profile.toggle_keymap(self.is_searchable());
        }
        self
    }
    pub fn is_multi(&self) -> bool {
        self.multi
    }
    pub fn checked(&self) -> &[ItemKey] {
        &self.list.checked
    }
    pub fn toggle_checked(&mut self, i: usize) -> Option<PickerAction> {
        self.eligible(i)
            .and_then(|index| self.toggle_item(self.item_key(index)))
    }
    fn item_key(&self, i: usize) -> ItemKey {
        Self::key_of(&self.items[i], i)
    }

    fn item_for_key_in(items: &[Item], key: ItemKey) -> Option<&Item> {
        items
            .iter()
            .enumerate()
            .find(|(index, item)| Self::key_of(item, *index) == key)
            .map(|(_, item)| item)
    }

    fn key_of(item: &Item, index: usize) -> ItemKey {
        item.key.unwrap_or(ItemKey::index(index))
    }
    pub fn nav(mut self, nav: NavKeys) -> Self {
        self.list.nav = nav;
        self
    }

    pub fn delete_is_secondary(mut self, on: bool) -> Self {
        self.delete_is_secondary = on;
        self
    }

    pub fn searchable(mut self, on: bool) -> Result<Self, PickerItemsError> {
        self.set_searchable(on)?;
        Ok(self)
    }

    pub fn set_searchable(&mut self, on: bool) -> Result<(), PickerItemsError> {
        if self.multi
            && on
            && let Some(error) = Self::validate_items(&self.items, true)
        {
            return Err(error);
        }
        match (on, self.list.query.is_some()) {
            (true, false) => self.list.query = Some(Box::new(EditField::single(""))),
            (false, true) => self.list.query = None,
            _ => {}
        }
        if !self.toggle_keymap_overridden {
            self.toggle_keymap = self.key_profile.toggle_keymap(on);
        }
        Ok(())
    }

    pub fn is_searchable(&self) -> bool {
        self.list.query.is_some()
    }

    pub fn query(&self) -> &str {
        self.list.query.as_deref().map_or("", EditField::text)
    }

    pub fn set_query(&mut self, text: &str) {
        if let Some(q) = &mut self.list.query {
            q.set_text(text);
        }
    }

    pub fn caret(&self) -> usize {
        self.list
            .query
            .as_ref()
            .map_or(0, |q| q.buffer.cursor_offset())
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn list(&self) -> &OptionList {
        &self.list
    }

    pub fn set_width(&mut self, width: u16) {
        self.list.width = (width, width);
    }

    pub fn set_max_rows(&mut self, rows: u16) {
        self.list.max_rows = rows;
    }

    pub fn set_placement(&mut self, placement: Placement) {
        self.list.placement = placement;
    }

    pub fn set_chosen(&mut self, chosen: Option<usize>) {
        self.list.chosen = chosen.filter(|index| *index < self.items.len());
    }

    pub fn wheel(&mut self, axis: crate::core::hit::Axis, delta: i32) -> Outcome {
        crate::core::scroll::Scrollable::wheel(&mut self.list, axis, delta).outcome()
    }

    pub fn width(mut self, width: u16) -> Self {
        self.list = self.list.width(width, width);
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.list.placement = placement;
        self
    }

    pub fn anchor(mut self, anchor: Rect, placement: Placement) -> Self {
        self.anchor = anchor;
        self.list.placement = placement;
        self
    }

    pub fn fit(mut self, fit: crate::ui::popup::Place) -> Self {
        self.list.fit = fit;
        self
    }

    pub fn backdrop(mut self, backdrop: Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub fn hints(mut self, hints: &str) -> Self {
        self.hints = hints.to_owned();
        self.hints_overridden = true;
        self
    }

    pub fn max_rows(mut self, rows: u16) -> Self {
        self.list.max_rows = rows;
        self
    }

    pub fn placeholder(mut self, text: &str) -> Self {
        self.placeholder = text.to_owned();
        self
    }

    pub fn empty(mut self, state: crate::widgets::empty::EmptyState) -> Self {
        self.empty = Some(state);
        self
    }

    pub fn empty_text(mut self, text: &str) -> Self {
        self.empty_text = text.to_owned();
        self
    }

    pub fn dismiss(mut self, dismiss: crate::ui::popup::Dismiss) -> Self {
        self.list.dismiss = dismiss;
        self
    }

    pub fn on_click_outside(&mut self) -> Option<PickerAction> {
        self.list
            .dismiss
            .outside_click
            .then_some(PickerAction::Cancelled)
    }

    pub fn cursor_style(mut self, cursor_style: CursorStyle) -> Self {
        self.list.look = cursor_style;
        self
    }

    pub fn height(&self) -> u16 {
        let max = self.list.max_rows;
        let rows = if self.status == PickerStatus::Ready {
            (self.items.len() as u16).clamp(1, max)
        } else {
            (self.items.len() as u16).clamp(2, max)
        };
        let query_rows = if self.is_searchable() { 2 } else { 0 };
        2 + 1 + query_rows + rows + 2
    }

    fn first_enabled(&self) -> usize {
        self.items.iter().position(|i| !i.disabled).unwrap_or(0)
    }

    pub fn set_items(&mut self, items: Vec<Item>) -> Result<(), PickerItemsError> {
        if let Some(error) = Self::validate_items(&items, self.multi && self.is_searchable()) {
            return Err(error);
        }
        self.authoritative_items.clone_from(&items);
        self.items = items;
        self.list.checked.clear();
        self.list.reset(self.items.len());
        self.list.cursor.index = self.first_enabled();
        self.status = PickerStatus::Ready;
        Ok(())
    }

    pub fn replace_items(&mut self, items: Vec<Item>) -> Result<(), PickerItemsError> {
        if let Some(error) = Self::validate_items(&items, self.multi && self.is_searchable()) {
            return Err(error);
        }
        let keep = self
            .items
            .get(self.list.cursor.index)
            .and_then(|item| item.key);
        let offset = self.list.scroll.offset;
        let stable_checked: std::collections::HashSet<ItemKey> = self
            .authoritative_items
            .iter()
            .filter_map(|item| item.key)
            .collect();
        self.authoritative_items.clone_from(&items);
        self.items = items;
        self.list.checked.retain(|key| {
            stable_checked.contains(key)
                && self
                    .authoritative_items
                    .iter()
                    .any(|item| item.key == Some(*key) && !item.disabled)
        });
        self.list.cursor.index = keep
            .and_then(|key| {
                self.items
                    .iter()
                    .position(|item| item.key == Some(key) && !item.disabled)
            })
            .unwrap_or_else(|| self.first_enabled());
        self.list.scroll.set_content(self.items.len());
        self.list.scroll.scroll_to(offset);
        self.status = PickerStatus::Ready;
        Ok(())
    }

    pub fn set_visible_items(&mut self, items: Vec<Item>) -> Result<(), PickerItemsError> {
        if let Some(error) = Self::validate_items(&items, true) {
            return Err(error);
        }
        let mut visible = Vec::with_capacity(items.len());
        for item in items {
            let Some(key) = item.key else {
                return Err(PickerItemsError::MissingStableKey);
            };
            let Some(mut authoritative) = self
                .authoritative_items
                .iter()
                .find(|candidate| candidate.key == Some(key))
                .cloned()
            else {
                return Err(PickerItemsError::UnknownVisibleKey(key));
            };
            authoritative.matched = item.matched;
            visible.push(authoritative);
        }
        let keep = self
            .items
            .get(self.list.cursor.index)
            .and_then(|item| item.key);
        let offset = self.list.scroll.offset;
        self.items = visible;
        self.list.cursor.index = keep
            .and_then(|key| {
                self.items
                    .iter()
                    .position(|item| item.key == Some(key) && !item.disabled)
            })
            .unwrap_or_else(|| self.first_enabled());
        self.list.scroll.set_content(self.items.len());
        self.list.scroll.scroll_to(offset);
        self.status = PickerStatus::Ready;
        Ok(())
    }

    pub fn item_for_key(&self, key: ItemKey) -> Option<&Item> {
        Self::item_for_key_in(&self.authoritative_items, key)
    }

    fn toggle_item(&mut self, key: ItemKey) -> Option<PickerAction> {
        let group = Self::item_for_key_in(&self.authoritative_items, key)
            .and_then(|item| item.selection_group);
        if !self.list.checked.contains(&key)
            && let Some(group) = group
            && let Some(selected) =
                self.authoritative_items
                    .iter()
                    .enumerate()
                    .find(|(index, item)| {
                        item.selection_group == Some(group)
                            && Self::key_of(item, *index) != key
                            && self.list.checked.contains(&Self::key_of(item, *index))
                    })
        {
            return Some(PickerAction::SelectionConflict {
                attempted: key,
                selected: Self::key_of(selected.1, selected.0),
            });
        }
        self.list.toggle_checked(key);
        None
    }

    pub fn report_item_error(&mut self, error: PickerItemsError) {
        self.status = PickerStatus::Error {
            message: "Invalid picker items".into(),
            detail: Some(format!("{error:?}")),
        };
    }

    pub fn reset_selection(&mut self) {
        self.list.checked.clear();
    }

    fn validate_items(items: &[Item], require_stable: bool) -> Option<PickerItemsError> {
        let mut keys = std::collections::HashSet::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            if require_stable && item.key.is_none() {
                return Some(PickerItemsError::MissingStableKey);
            }
            let key = Self::key_of(item, index);
            if !keys.insert(key) {
                return Some(PickerItemsError::DuplicateKey(key));
            }
        }
        None
    }

    pub fn eligible(&self, i: usize) -> Option<usize> {
        if self.status != PickerStatus::Ready {
            return None;
        }
        self.items.get(i).filter(|it| !it.disabled).map(|_| i)
    }

    pub fn current(&self) -> Option<&Item> {
        self.eligible(self.list.cursor.index)
            .map(|i| &self.items[i])
    }

    pub fn current_key(&self) -> Option<ItemKey> {
        self.current().and_then(|item| item.key)
    }

    pub fn on_paste(&mut self, text: &str) -> (Outcome, Option<PickerAction>) {
        if self.status == PickerStatus::Loading(String::new()) {
            return (Outcome::Consumed, None);
        }
        let Some(query) = &mut self.list.query else {
            return (Outcome::Consumed, None);
        };

        let flat = text.replace('\t', " ");
        if flat.is_empty() || !query.paste(&flat) {
            return (Outcome::Consumed, None);
        }
        (Outcome::Changed, Some(PickerAction::QueryChanged))
    }

    pub fn set_cursor(&mut self, i: usize) {
        self.list.cursor.index = i.min(self.items.len().saturating_sub(1));
        self.list.scroll.ensure_visible(self.list.cursor.index);
    }

    pub fn row_id(&self, i: usize) -> WidgetId {
        self.list.row_id(i)
    }

    pub fn on_key(&mut self, key: &Key) -> (Outcome, Option<PickerAction>) {
        self.on_key_with_capabilities(key, KeyboardCapabilities::current())
    }

    fn on_key_with_capabilities(
        &mut self,
        key: &Key,
        capabilities: KeyboardCapabilities,
    ) -> (Outcome, Option<PickerAction>) {
        let toggle = self.toggle_keymap.iter().any(|(binding, command)| {
            *command == PickerCmd::Toggle
                && binding.deliverable_with(capabilities)
                && binding.normalized_mods() == crate::core::keymap::normalized_key_mods(key)
                && binding.matches_with(key, capabilities)
        });
        if self.multi && toggle {
            return match self.eligible(self.list.cursor.index) {
                Some(i) => match self.toggle_checked(i) {
                    Some(action) => (Outcome::Consumed, Some(action)),
                    None => (Outcome::Changed, Some(PickerAction::Toggled)),
                },
                None => (Outcome::Consumed, None),
            };
        }
        if self.list.query.is_some()
            && let Some(o) = edit_menu_key(self, key)
        {
            let ev = self
                .edit_menu
                .take_edited()
                .then_some(PickerAction::QueryChanged);
            return (o, ev);
        }
        if self.multi && key.code == KeyCode::Enter && !key.alt() {
            return if self.list.checked.is_empty() {
                (Outcome::Consumed, None)
            } else {
                (
                    Outcome::Changed,
                    Some(PickerAction::ChosenMany(self.list.checked.clone())),
                )
            };
        }
        let typing = self.is_searchable();

        if !typing
            && self.list.type_ahead
            && matches!(
                KeyStep::of(key, &self.list.nav, true),
                Some(KeyStep::Type(_))
            )
        {
            let moved = self.list.key(key, &self.items[..]);
            if moved.index().is_some() {
                let o = if moved.moved() {
                    Outcome::Changed
                } else {
                    Outcome::Consumed
                };
                return (o, None);
            }
        }
        let nav = self.list.nav.action(key).filter(|n| {
            !(typing
                && (matches!(key.code, KeyCode::Char(_))
                    || matches!(n, Nav::Home | Nav::End | Nav::Left | Nav::Right)))
        });
        let nav = nav.filter(|n| key.plain() || !matches!(n, Nav::Home | Nav::End));
        let Some(nav) = nav else {
            return self.on_other_key(key);
        };
        match self.list.nav_step(nav, &self.items[..]).index() {
            Some(_) => (Outcome::Changed, None),
            None => self.on_other_key(key),
        }
    }

    fn on_other_key(&mut self, key: &Key) -> (Outcome, Option<PickerAction>) {
        let searchable = self.is_searchable();
        match key.code {
            KeyCode::Esc => {
                if let Some(q) = &mut self.list.query
                    && !q.text().is_empty()
                {
                    q.record(crate::core::edit::EditKind::Other);
                    q.buffer.set_text("");
                    return (Outcome::Changed, Some(PickerAction::QueryChanged));
                }
                if !self.list.dismiss.escape {
                    return (Outcome::Consumed, None);
                }
                (Outcome::Changed, Some(PickerAction::Cancelled))
            }
            KeyCode::Enter => match self.eligible(self.list.cursor.index) {
                Some(i) => {
                    let ev = if key.alt() {
                        PickerAction::ChosenAlt(i)
                    } else {
                        PickerAction::Chosen(i)
                    };
                    (Outcome::Changed, Some(ev))
                }
                None if searchable && !self.query().trim().is_empty() => {
                    (Outcome::Changed, Some(PickerAction::Submit))
                }
                None => (Outcome::Consumed, None),
            },
            KeyCode::Char('n') | KeyCode::Char('j') if key.ctrl() => {
                self.list.step(1, &self.items[..]);
                (Outcome::Changed, None)
            }
            KeyCode::Char('p') | KeyCode::Char('k') if key.ctrl() => {
                self.list.step(-1, &self.items[..]);
                (Outcome::Changed, None)
            }
            KeyCode::Tab => (Outcome::Changed, Some(PickerAction::NextScope)),
            KeyCode::BackTab => (Outcome::Changed, Some(PickerAction::PrevScope)),
            KeyCode::Delete if self.delete_is_secondary || !searchable => {
                match self.eligible(self.list.cursor.index) {
                    Some(i) => (Outcome::Changed, Some(PickerAction::Secondary(i))),
                    None => (Outcome::Consumed, None),
                }
            }
            KeyCode::Backspace if self.query().is_empty() => {
                (Outcome::Changed, Some(PickerAction::Back))
            }

            _ => match &mut self.list.query {
                Some(q) => {
                    let r = q.on_key(key);
                    if r.edited {
                        (Outcome::Changed, Some(PickerAction::QueryChanged))
                    } else if r.outcome == Outcome::Changed && r.exit.is_none() {
                        (Outcome::Changed, None)
                    } else {
                        (Outcome::Consumed, None)
                    }
                }
                None => (Outcome::Consumed, None),
            },
        }
    }

    pub fn on_click(&mut self, id: WidgetId) -> Option<PickerAction> {
        if id == crate::ui::popup::backdrop_id(self.id) {
            return self.on_click_outside();
        }
        let i = self.eligible(self.list.row_of(id, self.items.len())?)?;
        self.list.select(i, &self.items[..]);
        if self.multi {
            match self.toggle_checked(i) {
                Some(action) => Some(action),
                None => Some(PickerAction::Toggled),
            }
        } else {
            Some(PickerAction::Chosen(i))
        }
    }

    pub fn draw(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx) {
        let hints = self.hints_for_draw(&HintCx::current(None));
        self.paint(
            screen,
            buf,
            ctx,
            &crate::widgets::keyhint::deliverable_line(&hints),
        );
        self.hints = hints;

        self.edit_menu.render(self.area, buf, ctx);
    }

    fn hints_for_draw(&self, cx: &HintCx) -> String {
        self.hints_for_render(cx, &self.hints, self.hints_overridden)
    }

    fn hints_for_render(&self, cx: &HintCx, custom_hints: &str, overridden: bool) -> String {
        let generated = HasHints::hints(self, cx);
        if !self.multi {
            return custom_hints.to_owned();
        }
        if !overridden && !self.toggle_keymap_overridden && !self.key_profile_configured {
            return custom_hints.to_owned();
        }
        if !overridden {
            return generated
                .into_iter()
                .map(|hint| format!("{} {}", hint.key, hint.action))
                .collect::<Vec<_>>()
                .join(" · ");
        }
        let toggle = generated
            .iter()
            .find(|hint| hint.action == "Toggle")
            .map(|hint| format!("{} {}", hint.key, hint.action));
        let mut inserted_toggle = false;
        let mut hints = custom_hints
            .split(" · ")
            .filter_map(|hint| {
                if hint.split_whitespace().last() == Some("Toggle") {
                    if inserted_toggle {
                        None
                    } else {
                        inserted_toggle = true;
                        toggle.clone()
                    }
                } else {
                    Some(hint.to_owned())
                }
            })
            .collect::<Vec<_>>();
        if !inserted_toggle && let Some(toggle) = toggle {
            hints.push(toggle);
        }
        hints.join(" · ")
    }

    #[deprecated(note = "set `Picker::hints` and call `Picker::draw`")]
    pub fn render(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, hints: &str) {
        let hints = self.hints_for_render(
            &HintCx::current(None),
            hints,
            self.hints_overridden || !hints.trim().is_empty(),
        );
        self.paint(screen, buf, ctx, &hints);
    }

    fn paint(&mut self, screen: Rect, buf: &mut Buffer, ctx: &mut RenderCtx, hints: &str) {
        self.list.scroll.begin(ctx.track_click);
        let s = ctx.theme.strings;
        let old = std::mem::replace(&mut self.strings, s);
        use crate::theme::Strings;
        Strings::follow(
            &mut self.placeholder,
            old.search_placeholder,
            s.search_placeholder,
        );
        Strings::follow(&mut self.empty_text, old.no_matches, s.no_matches);
        let t = ctx.theme;

        ctx.modal_backdrop_with(buf, screen, self.backdrop);

        let modal = crate::ui::popup::Dismiss {
            pass_through: false,
            ..self.list.dismiss
        };
        crate::ui::popup::backdrop(ctx, self.id, modal, screen);
        let h = self.height().min(screen.height.saturating_sub(2));
        let w = self.list.width.1.min(screen.width.saturating_sub(4));
        let area = place_with(
            screen,
            self.anchor,
            w,
            h,
            self.list.placement,
            self.list.fit,
        );
        self.area = area;
        let bg = t.surface_elevated;
        fill(buf, area, Style::new().bg(bg));
        let block = ratatui::widgets::Block::new()
            .borders(ratatui::widgets::Borders::ALL)
            .border_set(t.glyphs.border)
            .border_style(t.border(true).bg(bg));
        ratatui::widgets::Widget::render(block, area, buf);
        ctx.hits.register(self.id, area);
        let inner = area.inner(ratatui::layout::Margin::new(2, 1));
        if inner.is_empty() {
            return;
        }
        let mut y = inner.y;
        buf.set_string(
            inner.x,
            y,
            truncate(&self.title, inner.width as usize),
            t.title().bg(bg),
        );
        if let Some(scope) = &self.scope {
            let room = inner.width.saturating_sub(width(&self.title) as u16 + 3) as usize;
            if room >= 6 {
                let scope = truncate(scope, room);
                let sw = width(&scope) as u16;
                buf.set_string(
                    inner.right().saturating_sub(sw),
                    y,
                    &scope,
                    t.muted().bg(bg),
                );
            }
        }
        y += 1;
        if let Some(query) = &mut self.list.query {
            let field = Rect::new(inner.x, y, inner.width, 1);
            let fstate = crate::ui::ctx::VisualState {
                focused: true,
                editing: true,
                ..Default::default()
            };
            let fs = t.field_style(fstate);
            fill(buf, field, fs);
            buf.set_string(
                field.x,
                y,
                t.gutter_symbol(fstate),
                t.gutter(fstate, fs.bg.unwrap_or(bg), false),
            );
            query.keys = ctx.theme.field_keys.unwrap_or_default();
            let w = field.width.saturating_sub(3) as usize;

            let line = Rect::new(field.x + 2, y, field.width.saturating_sub(2), 1);
            let caret = if query.text().is_empty() {
                buf.set_string(
                    line.x,
                    y,
                    truncate(&self.placeholder, w),
                    fs.fg(t.text_muted),
                );
                line.as_position()
            } else {
                let look = crate::widgets::edit::LineLook {
                    text: fs,
                    selection: t.selection(),
                    ellipsis: fs.fg(t.text_muted),
                    ellipsis_glyph: t.glyphs.ellipsis,
                    underline: Some(t.text_secondary),
                    follow_caret: true,
                };
                query.draw_line(buf, line, &look)
            };
            if line.width > 0 {
                let id = ctx.register_part(self.id, PartRef::of(Part::QUERY), line);
                ctx.pointer_shape(id, crate::core::shape::PointerShape::Text);
            }
            ctx.set_cursor(caret);
            y += 2;
        }

        let hints_fit = inner.bottom() > y + 1;
        let hints = if hints_fit { hints } else { "" };
        let list = Rect::new(
            inner.x,
            y,
            inner.width,
            inner.bottom().saturating_sub(y + u16::from(hints_fit)),
        );

        self.list.layout(list, &self.items[..], ctx);
        let hint_row = |buf: &mut Buffer| {
            if !hints.is_empty() {
                let hy = inner.bottom().saturating_sub(1);
                buf.set_string(
                    inner.x,
                    hy,
                    truncate(hints, inner.width as usize),
                    t.faint().bg(bg),
                );
            }
        };
        match &self.status {
            PickerStatus::Loading(label) => {
                crate::widgets::progress::render_spinner(
                    Rect::new(list.x + 1, list.y, list.width.saturating_sub(1), 1),
                    buf,
                    ctx,
                    label,
                    bg,
                );
                hint_row(buf);
                return;
            }
            PickerStatus::Error { message, detail } => {
                let mut e = crate::widgets::empty::EmptyState::error(message);
                if let Some(d) = detail {
                    e = e.hint(d);
                }
                crate::widgets::empty::render(list, buf, t, &e, bg);
                hint_row(buf);
                return;
            }
            PickerStatus::Ready => {}
        }
        if self.items.is_empty() {
            crate::widgets::empty::render(
                list,
                buf,
                t,
                &self
                    .empty
                    .clone()
                    .unwrap_or_else(|| crate::widgets::empty::EmptyState::new(&self.empty_text)),
                bg,
            );
        }
        let has_sb = self.list.scroll.overflows();
        let row_w = list.width.saturating_sub(u16::from(has_sb));

        let measure = |f: &dyn Fn(&Item) -> u16| self.items.iter().map(f).max().unwrap_or(0);
        let label_need = measure(&|i| width(&i.label) as u16).max(6);
        let detail_need = measure(&|i| width(&i.detail) as u16);
        let tag_col = measure(&|i| i.tag.as_deref().map_or(0, |t| width(t) as u16));
        let group_col = measure(&|i| width(&i.group) as u16);
        let label_cap = label_need.min((row_w * 45 / 100).max(6));
        let mut cols = vec![
            Col::content(label_cap.min(12), label_cap),
            Col::content(detail_need.min(4), detail_need)
                .flex(1, u16::MAX)
                .droppable(0),
        ];
        if tag_col > 0 {
            cols.push(Col::fixed(tag_col));
        }
        if group_col > 0 {
            cols.push(Col::fixed(group_col));
        }
        let widths = columns::solve(&cols, row_w.saturating_sub(4), 2);
        let label_col = widths[0];
        let detail_col = widths[1];
        let items = &self.items[..];
        let reveal = self.reveal;
        let mut last_group = "";
        self.list
            .paint_rows(list, bg, items, buf, ctx, |i, r, buf, ctx| {
                let it = &items[i];
                let (row, ry) = (r.area, r.area.y);
                let rid = self.list.row_id(i);
                let st = r.style();
                let focused = r.state.focused;

                let show_group = it.group != last_group && !it.group.is_empty();
                last_group = it.group.as_ref();
                let glyph: &str = if it.glyph.is_empty() || it.glyph == DEFAULT_GLYPH {
                    t.glyphs.bullet
                } else {
                    &it.glyph
                };
                buf.set_string(
                    row.x + 1,
                    ry,
                    glyph,
                    st.fg(if focused {
                        t.text_primary
                    } else {
                        r.ink(ctx, t.text_muted)
                    })
                    .remove_modifier(Modifier::BOLD),
                );

                if self.list.checked.contains(&self.item_key(i)) {
                    r.mark(
                        ctx,
                        buf,
                        row.x + 1,
                        ry,
                        crate::ui::row::MarkKind::Tick,
                        focused,
                    );
                }
                let mut x = row.x + 3;
                let label = truncate(&it.label, label_col as usize);
                let label_cut = !it.disabled
                    && crate::ui::reveal::truncated(
                        ctx,
                        reveal,
                        rid,
                        Rect::new(row.x + 3, ry, label_col, 1),
                        &it.label,
                    );
                for (bi, ch) in label.char_indices() {
                    let mut cs = st;
                    if it.matched.contains(&bi) {
                        cs = cs.add_modifier(Modifier::BOLD);
                    } else if !focused {
                        cs = cs.remove_modifier(Modifier::BOLD);
                    }
                    let g = ch.to_string();
                    buf.set_string(x, ry, &g, cs);
                    x += width(&g) as u16;
                }
                let mut rx = row.right();
                if group_col > 0 {
                    rx = rx.saturating_sub(group_col + 1);
                    if show_group {
                        buf.set_string(
                            rx,
                            ry,
                            &it.group,
                            st.fg(r.ink(ctx, t.legible(t.text_muted, r.bg())))
                                .remove_modifier(Modifier::BOLD),
                        );
                    }
                }
                if tag_col > 0 {
                    rx = rx.saturating_sub(tag_col + 2);
                    if let Some(tag) = &it.tag {
                        buf.set_string(
                            rx,
                            ry,
                            tag,
                            st.fg(r.ink(ctx, t.text_secondary))
                                .remove_modifier(Modifier::BOLD),
                        );
                    }
                }
                let mut detail_cut = false;
                if !it.detail.is_empty() {
                    let dx = row.x + 3 + label_col + 2;
                    let room = (detail_col as usize).min(rx.saturating_sub(dx + 1) as usize);
                    if room >= 4 {
                        buf.set_string(
                            dx,
                            ry,
                            truncate(&it.detail, room),
                            st.fg(r.ink(ctx, t.text_muted))
                                .remove_modifier(Modifier::BOLD),
                        );
                        detail_cut = wider_than(&it.detail, room);
                        if !it.disabled && !label_cut {
                            crate::ui::reveal::truncated(
                                ctx,
                                reveal,
                                rid,
                                Rect::new(dx, ry, room as u16, 1),
                                &it.detail,
                            );
                        }
                    } else {
                        detail_cut = true;
                    }
                }

                if focused && !it.disabled && (label_cut || detail_cut) {
                    let full = if it.detail.is_empty() {
                        it.label.clone()
                    } else {
                        format!("{} — {}", it.label, it.detail)
                    };
                    crate::ui::reveal::peek(ctx, &full);
                }
            });
        hint_row(buf);
    }
}

impl RowSource for Picker {
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

impl Navigable for Picker {
    fn cursor(&self) -> Cursor {
        self.list.cursor
    }
}

impl crate::core::pointer::MouseTarget for Picker {
    type Action = PickerAction;

    fn mouse_id(&self) -> WidgetId {
        self.id
    }

    fn pointer(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> crate::core::response::Response<PickerAction> {
        use crate::core::intent::Phase;
        use crate::core::response::Response;

        if let Some(o) = edit_menu_pointer(self, ev) {
            if self.edit_menu.take_edited() {
                return Response::emit(PickerAction::QueryChanged);
            }
            return o.into();
        }

        if ev.wheel().is_some() && ev.part().part == Part::BACKDROP {
            return Response::ignored();
        }
        if let Some(r) = crate::core::pointer::scroll_input(&mut self.list, ev) {
            return r.lift();
        }

        if let Some(query) = &mut self.list.query
            && let Some(r) = crate::widgets::option_list::query_pointer(query, ev)
        {
            return r.lift();
        }
        if !ev.is(Phase::Click) {
            return Response::ignored();
        }
        let chosen = if ev.part().part == Part::BACKDROP {
            self.on_click_outside()
        } else {
            match self.on_click(ev.id) {
                Some(PickerAction::Chosen(i))
                    if ev
                        .mods()
                        .contains(ratatui::crossterm::event::KeyModifiers::ALT) =>
                {
                    Some(PickerAction::ChosenAlt(i))
                }
                other => other,
            }
        };
        match chosen {
            Some(a) => Response::emit(a),
            None => Response::consumed(),
        }
    }

    fn route(
        &mut self,
        ev: &crate::core::pointer::PointerEvent,
    ) -> Option<crate::core::response::Response<PickerAction>> {
        edit_menu_route(self, ev)
    }
}

impl EditMenuHost for Picker {
    fn edit_menu(&mut self) -> &mut EditMenu {
        &mut self.edit_menu
    }
    fn edit_owner(&self) -> WidgetId {
        self.id
    }
    fn edit_caps(&self) -> EditCaps {
        self.list
            .query
            .as_ref()
            .map(|q| EditCaps::of(q))
            .unwrap_or_default()
    }
    fn edit_shortcut(&self, cmd: TextCmd) -> Option<String> {
        self.list.query.as_ref().and_then(|q| q.shortcut(cmd))
    }
    fn edit_target(&self, ev: &crate::core::pointer::PointerEvent) -> bool {
        self.list.query.is_some() && ev.part().part == Part::QUERY
    }
    fn edit_point(&mut self, pos: ratatui::layout::Position) {
        if let Some(q) = self.list.query.as_mut() {
            let at = q.line_offset_at(pos);
            point_field(q, at);
        }
    }
    fn edit_run(&mut self, cmd: TextCmd) -> bool {
        self.list.query.as_mut().is_some_and(|q| q.run(cmd).edited)
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    use super::{Item, Picker, PickerAction, PickerKeyProfile};
    use crate::core::edit::TextCmd;
    use crate::core::event::{Key, Outcome};
    use crate::core::focus::{Focus, FocusRing};
    use crate::core::hints::{HasHints, HintCx};
    use crate::core::hit::HitRegistry;
    use crate::core::id::{ItemKey, Part, PartRef, WidgetId};
    use crate::core::intent::Phase;
    use crate::core::keymap::{KeyBinding, KeyboardCapabilities};
    use crate::core::pointer::PointerEvent;
    use crate::theme::{ColorLevel, Theme};
    use crate::ui::context::ContextAnchor;
    use crate::ui::ctx::{Interaction, RenderCtx};
    use crate::ui::layer::{Layer, LayerCx, LayerOut};
    use crate::ui::layers::LayerResult;
    use crate::widgets::hintbar::{HintBar, HintKeys, HintLayer};
    use crate::widgets::keyhint::key_of;
    use crate::widgets::menu::MenuItem;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Position, Rect};

    #[test]
    fn searchable_multi_keeps_hidden_identities_across_filter_and_reorder() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        let alpha = Item::new("Alpha").key(ItemKey::text("alpha"));
        let beta = Item::new("Beta").key(ItemKey::text("beta"));
        let same_a = Item::new("Same label").key(ItemKey::text("same-a"));
        let same_b = Item::new("Same label").key(ItemKey::text("same-b"));
        picker
            .set_items(vec![
                alpha.clone(),
                beta.clone(),
                same_a.clone(),
                same_b.clone(),
            ])
            .expect("valid picker update");
        assert!(
            HasHints::hints(&picker, &HintCx::new(None))
                .iter()
                .any(|hint| hint.key == "Insert" && hint.action == "Toggle")
        );

        assert_eq!(
            picker
                .on_key(&Key {
                    code: KeyCode::Insert,
                    mods: KeyModifiers::NONE
                })
                .0,
            Outcome::Changed
        );
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
        let (outcome, action) = picker.on_key(&Key {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::NONE,
        });
        assert_eq!(outcome, Outcome::Changed);
        assert_eq!(action, Some(PickerAction::QueryChanged));
        assert_eq!(picker.query(), " ");
        picker
            .set_visible_items(vec![beta.clone()])
            .expect("valid picker update");
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
        picker.set_cursor(0);
        picker.on_key(&Key {
            code: KeyCode::Insert,
            mods: KeyModifiers::NONE,
        });
        picker
            .replace_items(vec![same_b.clone(), beta, alpha, same_a.clone()])
            .expect("valid picker update");
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("alpha"), ItemKey::text("beta")]
        );
        let area = Rect::new(0, 0, 100, 30);
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buffer = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        picker.draw(area, &mut buffer, &mut ctx);
        ctx.flush_overlays(&mut buffer);
        assert_eq!(
            buffer
                .content()
                .iter()
                .filter(|cell| cell.symbol() == theme.glyphs.tick)
                .count(),
            2
        );
        picker
            .set_visible_items(vec![same_a.clone(), same_b.clone()])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker.toggle_checked(1);
        assert_eq!(
            picker.checked(),
            &[
                ItemKey::text("alpha"),
                ItemKey::text("beta"),
                ItemKey::text("same-a"),
                ItemKey::text("same-b")
            ]
        );
        picker
            .set_visible_items(vec![same_a, same_b])
            .expect("valid picker update");
        assert_eq!(
            picker
                .on_key(&Key {
                    code: KeyCode::Enter,
                    mods: KeyModifiers::NONE
                })
                .1,
            Some(PickerAction::ChosenMany(vec![
                ItemKey::text("alpha"),
                ItemKey::text("beta"),
                ItemKey::text("same-a"),
                ItemKey::text("same-b")
            ]))
        );
    }

    #[test]
    fn hidden_deleted_and_disabled_choices_are_pruned_and_restoration_does_not_reselect() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        let alpha = Item::new("Alpha").key(ItemKey::text("alpha"));
        let beta = Item::new("Beta").key(ItemKey::text("beta"));
        picker
            .set_items(vec![alpha.clone(), beta.clone()])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker
            .set_visible_items(vec![beta.clone()])
            .expect("valid picker update");
        picker.toggle_checked(0);

        picker
            .replace_items(vec![beta.clone()])
            .expect("valid picker update");
        assert_eq!(picker.checked(), &[ItemKey::text("beta")]);
        picker
            .replace_items(vec![alpha.clone(), beta.clone()])
            .expect("valid picker update");
        assert_eq!(picker.checked(), &[ItemKey::text("beta")]);

        picker.toggle_checked(0);
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("beta"), ItemKey::text("alpha")]
        );
        picker
            .set_visible_items(vec![beta.clone()])
            .expect("valid picker update");
        picker
            .replace_items(vec![alpha.clone().disabled(true), beta.clone()])
            .expect("valid picker update");
        assert_eq!(picker.checked(), &[ItemKey::text("beta")]);
        picker
            .replace_items(vec![alpha, beta])
            .expect("valid picker update");
        assert_eq!(picker.checked(), &[ItemKey::text("beta")]);

        picker.reset_selection();
        assert!(picker.checked().is_empty());
    }

    #[test]
    fn refreshing_a_nonsearch_multi_picker_clears_position_keys() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .searchable(false)
            .expect("disabling search is valid");
        picker
            .set_items(vec![Item::new("Alpha")])
            .expect("valid picker update");
        picker.toggle_checked(0);
        assert_eq!(picker.checked(), &[ItemKey::index(0)]);

        picker
            .replace_items(vec![Item::new("Beta")])
            .expect("valid picker update");
        assert!(picker.checked().is_empty());
        assert_eq!(
            picker.on_key(&Key {
                code: KeyCode::Enter,
                mods: KeyModifiers::NONE,
            }),
            (Outcome::Consumed, None)
        );
    }

    #[test]
    fn multi_toggle_skips_disabled_items_and_non_search_space_toggles() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .searchable(false)
            .expect("disabling search is valid");
        picker
            .set_items(vec![
                Item::new("Unavailable").disabled(true),
                Item::new("Available"),
            ])
            .expect("valid picker update");
        assert!(
            HasHints::hints(&picker, &HintCx::new(None))
                .iter()
                .any(|hint| hint.key == "Space" && hint.action == "Toggle")
        );
        picker.set_cursor(0);
        assert_eq!(
            picker
                .on_key(&Key {
                    code: KeyCode::Char(' '),
                    mods: KeyModifiers::NONE
                })
                .0,
            Outcome::Consumed
        );
        picker.set_cursor(1);
        assert_eq!(
            picker
                .on_key(&Key {
                    code: KeyCode::Char(' '),
                    mods: KeyModifiers::NONE
                })
                .0,
            Outcome::Changed
        );
        assert_eq!(picker.checked(), &[ItemKey::index(1)]);
    }

    #[test]
    fn searchable_multi_profile_binding_is_visible_exact_and_preserves_space_input() {
        let group = ItemKey::text("provider");
        let mut picker = Picker::new(WidgetId::of("picker-test-remapped"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .key_profile(PickerKeyProfile::Vim);
        picker
            .set_items(vec![
                Item::new("Alpha")
                    .key(ItemKey::text("alpha"))
                    .selection_group(group),
                Item::new("Beta")
                    .key(ItemKey::text("beta"))
                    .selection_group(group),
                Item::new("Unavailable")
                    .key(ItemKey::text("unavailable"))
                    .disabled(true),
            ])
            .expect("valid picker update");
        let hints = HasHints::hints(&picker, &HintCx::current(None));
        assert!(
            hints
                .iter()
                .any(|hint| { hint.key == "Ctrl+T" && hint.action == "Toggle" })
        );
        assert!(!hints.iter().any(|hint| hint.key == "Insert"));

        let area = Rect::new(0, 0, 100, 30);
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buffer = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        picker.draw(area, &mut buffer, &mut ctx);
        ctx.flush_overlays(&mut buffer);
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Ctrl+T Toggle"));

        let mut focus = Focus::default();
        let app = ();
        let insert = Key {
            code: KeyCode::Insert,
            mods: KeyModifiers::NONE,
        };
        let inserted = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &insert, &mut cx)
        };
        assert_eq!(inserted, Outcome::Consumed);
        assert!(picker.checked().is_empty());

        let ctrl_t = Key {
            code: KeyCode::Char('t'),
            mods: KeyModifiers::CONTROL,
        };
        let toggled = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &ctrl_t, &mut cx)
        };
        assert_eq!(toggled, Outcome::Changed);
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
        assert_eq!(
            <Picker as Layer<LayerResult, ()>>::take_result(&mut picker),
            Some(LayerOut::Event(LayerResult::Picker(PickerAction::Toggled)))
        );

        let shifted_ctrl_t = Key {
            code: KeyCode::Char('t'),
            mods: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        };
        picker.on_key(&shifted_ctrl_t);
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

        let space = Key {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::NONE,
        };
        assert_eq!(
            picker.on_key(&space),
            (Outcome::Changed, Some(PickerAction::QueryChanged))
        );
        assert_eq!(picker.query(), " ");
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

        picker.set_cursor(1);
        assert_eq!(
            picker.on_key(&ctrl_t),
            (
                Outcome::Consumed,
                Some(PickerAction::SelectionConflict {
                    attempted: ItemKey::text("beta"),
                    selected: ItemKey::text("alpha"),
                })
            )
        );
        picker.set_cursor(2);
        assert_eq!(picker.on_key(&ctrl_t), (Outcome::Consumed, None));
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
    }

    #[test]
    fn custom_multi_hints_follow_the_effective_binding_and_capabilities() {
        struct RestoreAlt(bool);

        impl Drop for RestoreAlt {
            fn drop(&mut self) {
                crate::core::keyboard::set_alt_delivered(self.0);
            }
        }

        let original_alt = KeyboardCapabilities::current().alt;
        let _restore_alt = RestoreAlt(original_alt);
        let mut remapped = Picker::new(WidgetId::of("picker-custom-hints"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .toggle_binding(KeyBinding::ctrl('t'))
            .hints("Insert Toggle · Enter Confirm · Esc Close");
        remapped
            .set_items(vec![Item::new("Alpha").key(ItemKey::text("alpha"))])
            .expect("valid picker update");

        let area = Rect::new(0, 0, 100, 30);
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut buffer = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        remapped.draw(area, &mut buffer, &mut ctx);
        ctx.flush_overlays(&mut buffer);
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(rendered.contains("Ctrl+T Toggle"));
        assert!(rendered.contains("Enter Confirm"));
        assert!(rendered.contains("Esc Close"));
        assert!(!rendered.contains("Insert Toggle"));

        remapped.on_key(&Key {
            code: KeyCode::Insert,
            mods: KeyModifiers::NONE,
        });
        assert!(remapped.checked().is_empty());
        remapped.on_key(&Key {
            code: KeyCode::Char('t'),
            mods: KeyModifiers::CONTROL,
        });
        assert_eq!(remapped.checked(), &[ItemKey::text("alpha")]);

        crate::core::keyboard::set_alt_delivered(false);
        let mut undeliverable = Picker::new(WidgetId::of("picker-custom-undeliverable"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .toggle_binding(KeyBinding::alt('t'))
            .hints("Alt+T Toggle · Enter Confirm · Esc Close");
        undeliverable
            .set_items(vec![Item::new("Alpha").key(ItemKey::text("alpha"))])
            .expect("valid picker update");
        let mut buffer = Buffer::empty(area);
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        undeliverable.draw(area, &mut buffer, &mut ctx);
        ctx.flush_overlays(&mut buffer);
        let rendered = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!rendered.contains("Alt+T Toggle"));
        assert!(rendered.contains("Enter Confirm"));
        undeliverable.on_key(&Key {
            code: KeyCode::Char('t'),
            mods: KeyModifiers::ALT,
        });
        assert!(undeliverable.checked().is_empty());
        assert_eq!(
            undeliverable.on_key(&Key {
                code: KeyCode::Char('x'),
                mods: KeyModifiers::NONE,
            }),
            (Outcome::Changed, Some(super::PickerAction::QueryChanged))
        );
        assert_eq!(undeliverable.query(), "x");
    }

    #[test]
    fn corrected_items_restore_readiness_after_report_item_error() {
        let items = || vec![Item::new("Alpha").key(ItemKey::text("alpha"))];
        let mut picker = Picker::new(WidgetId::of("picker-readiness"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker.set_items(items()).expect("valid initial items");

        picker.report_item_error(super::PickerItemsError::MissingStableKey);
        assert!(
            picker.eligible(0).is_none(),
            "error state must block interaction"
        );

        picker
            .set_items(items())
            .expect("corrected items are valid");
        assert!(
            picker.status == super::PickerStatus::Ready,
            "set_items must restore readiness"
        );
        assert_eq!(
            picker.eligible(0),
            Some(0),
            "restored rows must be interactive again"
        );

        picker.report_item_error(super::PickerItemsError::MissingStableKey);
        picker
            .replace_items(items())
            .expect("corrected items are valid");
        assert!(
            picker.status == super::PickerStatus::Ready,
            "replace_items must restore readiness"
        );
        assert_eq!(picker.eligible(0), Some(0));

        picker.report_item_error(super::PickerItemsError::MissingStableKey);
        picker
            .set_visible_items(items())
            .expect("corrected items are valid");
        assert!(
            picker.status == super::PickerStatus::Ready,
            "set_visible_items must restore readiness"
        );
        assert_eq!(picker.eligible(0), Some(0));
    }

    #[test]
    fn legacy_shifted_character_events_toggle_the_picker() {
        use ratatui::crossterm::event::{Event, KeyEvent};

        let mut picker = Picker::new(WidgetId::of("picker-legacy-shift"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .toggle_binding(KeyBinding::char('t').shift());
        picker
            .set_items(vec![Item::new("Alpha").key(ItemKey::text("alpha"))])
            .expect("valid picker update");
        let legacy = Event::Key(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT));
        let input = crate::core::event::Input::from_crossterm_with(
            legacy,
            crate::core::event::InputConfig::default(),
        )
        .expect("legacy key event decodes");
        let crate::core::event::Input::Key(key) = input else {
            panic!("legacy key event must decode to a key");
        };
        assert_eq!(
            key.code,
            KeyCode::Char('T'),
            "legacy input preserves the produced character code"
        );
        let (outcome, _action) = picker.on_key(&key);
        assert!(
            matches!(outcome, Outcome::Changed | Outcome::Consumed),
            "the advertised shift toggle must consume the legacy event"
        );
        assert_eq!(
            picker.checked(),
            vec![ItemKey::text("alpha")],
            "the legacy Shift+T encoding must toggle the picker"
        );
    }

    #[test]
    fn remapped_toggle_hint_clicks_deliver_the_bound_key() {
        struct RestoreCaps {
            alt: bool,
            super_key: bool,
        }

        impl Drop for RestoreCaps {
            fn drop(&mut self) {
                crate::core::keyboard::set_alt_delivered(self.alt);
                crate::core::keyboard::set_super_delivered(self.super_key);
            }
        }

        let _restore = RestoreCaps {
            alt: KeyboardCapabilities::current().alt,
            super_key: KeyboardCapabilities::current().super_key,
        };
        crate::core::keyboard::set_alt_delivered(true);
        crate::core::keyboard::set_super_delivered(true);

        let cases = [
            (
                KeyBinding::cmd('t'),
                Key {
                    code: KeyCode::Char('t'),
                    mods: KeyModifiers::SUPER,
                },
            ),
            (
                KeyBinding::alt('t'),
                Key {
                    code: KeyCode::Char('t'),
                    mods: KeyModifiers::ALT,
                },
            ),
            (
                KeyBinding::char('t').shift(),
                Key {
                    code: KeyCode::Char('t'),
                    mods: KeyModifiers::SHIFT,
                },
            ),
        ];
        for (binding, expected) in cases {
            let mut picker = Picker::new(WidgetId::of("picker-remapped-hint"), "Choose")
                .multi_select(true)
                .expect("empty picker accepts multi-selection")
                .toggle_binding(binding);
            picker
                .set_items(vec![Item::new("Alpha").key(ItemKey::text("alpha"))])
                .expect("valid picker update");
            let hints = HasHints::hints(&picker, &HintCx::current(None));
            let toggle_at = hints
                .iter()
                .position(|hint| hint.action == "Toggle")
                .expect("remapped binding advertises a toggle hint");

            let area = Rect::new(0, 0, 100, 1);
            let theme = Theme::for_level(ColorLevel::TrueColor);
            let layer = HintLayer::new(hints.clone());
            let mut buffer = Buffer::empty(area);
            let mut hits = HitRegistry::default();
            let mut ring = FocusRing::default();
            let spots;
            let keys;
            {
                let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
                spots = HintBar::render_at(area, &mut buffer, &theme, &layer);
                keys = HintKeys::register(&mut ctx, &mut buffer, &layer.hints, spots.clone());
            }
            let cell = spots
                .iter()
                .find(|(i, _)| *i == toggle_at)
                .map(|(_, cell)| *cell)
                .expect("toggle hint renders a clickable cell");
            let id = HintBar::ID.part(PartRef::item(Part::KEY, ItemKey::index(toggle_at)));
            let ev = PointerEvent::new(
                &hits,
                Phase::Click,
                id,
                Position::new(cell.x, cell.y),
                1,
                KeyModifiers::NONE,
            );
            let clicked = keys
                .clicked(&ev)
                .expect("clicking the remapped toggle hint must yield a key");
            assert_eq!(
                clicked, expected,
                "the hint click for {binding:?} must deliver the key the binding sends"
            );

            assert!(
                key_of("Insertion").is_none(),
                "near-miss labels must not parse"
            );
            assert_ne!(
                clicked,
                Key {
                    code: KeyCode::Insert,
                    mods: KeyModifiers::NONE,
                },
                "a remapped hint must not fall back to the default Insert toggle"
            );
            if let Some(naive) = key_of(layer.hints[toggle_at].key.as_ref()) {
                assert_ne!(
                    naive, expected,
                    "the display label must not re-parse to the bound key by accident"
                );
                assert_ne!(
                    picker.on_key(&naive).1,
                    Some(PickerAction::Toggled),
                    "the label-reparsed key {naive:?} must not toggle"
                );
                assert!(picker.checked().is_empty());
            }
            assert_ne!(
                picker
                    .on_key(&Key {
                        code: KeyCode::Insert,
                        mods: KeyModifiers::NONE,
                    })
                    .1,
                Some(PickerAction::Toggled)
            );
            assert!(picker.checked().is_empty());

            assert_eq!(
                picker.on_key(&clicked),
                (Outcome::Changed, Some(PickerAction::Toggled))
            );
            assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);
        }
    }

    #[test]
    fn searchable_layer_routes_edit_menu_enter_before_multi_confirm_and_insert() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker
            .set_items(vec![
                Item::new("Alpha").key(ItemKey::text("alpha")),
                Item::new("Beta").key(ItemKey::text("beta")),
                Item::new("Unavailable")
                    .key(ItemKey::text("unavailable"))
                    .disabled(true),
            ])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker.set_query("Alpha");
        picker.edit_menu.open(
            picker.id,
            ContextAnchor::Control {
                id: picker.id,
                fallback: Rect::new(0, 0, 30, 1),
            },
            vec![(TextCmd::SelectAll, MenuItem::new("Select all"))],
        );

        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let app = ();
        let enter = Key {
            code: KeyCode::Enter,
            mods: KeyModifiers::NONE,
        };
        let out = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &enter, &mut cx)
        };
        assert_eq!(out, Outcome::Changed);
        assert!(!picker.edit_menu.is_open());
        assert_eq!(
            picker.list.query.as_ref().unwrap().buffer.selected_text(),
            Some("Alpha")
        );
        assert!(<Picker as Layer<LayerResult, ()>>::take_result(&mut picker).is_none());

        picker.set_query("Alpha");
        picker.set_cursor(1);
        let modified_insert = Key {
            code: KeyCode::Insert,
            mods: KeyModifiers::CONTROL,
        };
        let out = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &modified_insert, &mut cx)
        };
        assert_eq!(out, Outcome::Consumed);
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

        let insert = Key {
            code: KeyCode::Insert,
            mods: KeyModifiers::NONE,
        };
        let out = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &insert, &mut cx)
        };
        assert_eq!(out, Outcome::Changed);
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("alpha"), ItemKey::text("beta")]
        );

        let space = Key {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::NONE,
        };
        let out = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &space, &mut cx)
        };
        assert_eq!(out, Outcome::Changed);
        assert_eq!(picker.query(), "Alpha ");

        let out = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut picker, &enter, &mut cx)
        };
        assert_eq!(out, Outcome::Changed);
        assert!(matches!(
            <Picker as Layer<LayerResult, ()>>::take_result(&mut picker),
            Some(LayerOut::Close(LayerResult::Picker(PickerAction::ChosenMany(keys))))
                if keys == [ItemKey::text("alpha"), ItemKey::text("beta")]
        ));

        let mut plain = Picker::new(WidgetId::of("picker-test-plain"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .searchable(false)
            .expect("disabling search is valid");
        plain
            .set_items(vec![Item::new("Available")])
            .expect("valid picker update");
        let space = Key {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::NONE,
        };
        let out = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut plain, &space, &mut cx)
        };
        assert_eq!(out, Outcome::Changed);
        assert_eq!(plain.checked(), &[ItemKey::index(0)]);
    }

    #[test]
    fn multi_picker_rejects_a_second_choice_in_the_same_selection_group() {
        let group = ItemKey::text("omp");
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker
            .set_items(vec![
                Item::new("OMP · Kimi")
                    .key(ItemKey::text("omp-kimi"))
                    .selection_group(group),
                Item::new("OMP · MiMo")
                    .key(ItemKey::text("omp-mimo"))
                    .selection_group(group),
            ])
            .expect("valid picker update");
        picker.toggle_checked(0);
        assert_eq!(
            picker.toggle_checked(1),
            Some(PickerAction::SelectionConflict {
                attempted: ItemKey::text("omp-mimo"),
                selected: ItemKey::text("omp-kimi"),
            })
        );
        assert_eq!(picker.checked(), &[ItemKey::text("omp-kimi")]);
    }

    #[test]
    fn disabling_multi_select_clears_hidden_selections() {
        let mut picker = Picker::new(WidgetId::of("picker-test-multi-off"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker
            .set_items(vec![
                Item::new("Alpha").key(ItemKey::text("alpha")),
                Item::new("Beta").key(ItemKey::text("beta")),
            ])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker.toggle_checked(1);
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("alpha"), ItemKey::text("beta")]
        );

        picker = picker
            .multi_select(false)
            .expect("disabling multi-select is valid");
        assert!(
            picker.checked().is_empty(),
            "hidden selections must not survive multi-select being disabled"
        );
        assert_eq!(
            <Picker as HasHints>::hints(&picker, &HintCx::current(None))
                .iter()
                .filter(|hint| hint.action == "Toggle")
                .count(),
            0
        );
    }

    #[test]
    fn grouped_selections_can_be_deselected_after_groups_change() {
        let group = ItemKey::text("provider");
        let mut picker = Picker::new(WidgetId::of("picker-test-group-off"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker
            .set_items(vec![
                Item::new("Alpha").key(ItemKey::text("alpha")),
                Item::new("Beta").key(ItemKey::text("beta")),
            ])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker.toggle_checked(1);
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("alpha"), ItemKey::text("beta")]
        );

        picker
            .replace_items(vec![
                Item::new("Alpha")
                    .key(ItemKey::text("alpha"))
                    .selection_group(group),
                Item::new("Beta")
                    .key(ItemKey::text("beta"))
                    .selection_group(group),
            ])
            .expect("valid picker update");
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("alpha"), ItemKey::text("beta")],
            "replace_items preserves both checked keys"
        );

        assert_eq!(picker.toggle_checked(0), None);
        assert_eq!(
            picker.checked(),
            &[ItemKey::text("beta")],
            "removing an already-checked key must not report a group conflict"
        );
        assert_eq!(picker.toggle_checked(1), None);
        assert!(picker.checked().is_empty());
    }

    #[test]
    fn keyboard_toggles_emit_events_and_hidden_group_conflicts_keep_selection() {
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let app = ();
        let mut plain = Picker::new(WidgetId::of("picker-test-plain"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .searchable(false)
            .expect("disabling search is valid");
        plain
            .set_items(vec![Item::new("Alpha")])
            .expect("valid picker update");
        let space = Key {
            code: KeyCode::Char(' '),
            mods: KeyModifiers::NONE,
        };
        for (selected, expected) in [(true, &[ItemKey::index(0)][..]), (false, &[][..])] {
            let outcome = {
                let mut cx = LayerCx {
                    focus: &mut focus,
                    ring: &ring,
                    app: &app,
                };
                <Picker as Layer<LayerResult, ()>>::on_key(&mut plain, &space, &mut cx)
            };
            assert_eq!(outcome, Outcome::Changed);
            assert_eq!(plain.checked(), expected);
            assert_eq!(
                <Picker as Layer<LayerResult, ()>>::take_result(&mut plain),
                Some(LayerOut::Event(LayerResult::Picker(PickerAction::Toggled)))
            );
            assert_eq!(plain.checked().is_empty(), !selected);
        }

        let group = ItemKey::text("provider");
        let alpha = Item::new("Alpha")
            .key(ItemKey::text("alpha"))
            .selection_group(group);
        let beta = Item::new("Beta")
            .key(ItemKey::text("beta"))
            .selection_group(group);
        let mut searchable = Picker::new(WidgetId::of("picker-test-searchable"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        searchable
            .set_items(vec![alpha.clone(), beta.clone()])
            .expect("valid picker update");
        searchable.toggle_checked(0);
        searchable
            .set_visible_items(vec![beta])
            .expect("valid picker update");

        let insert = Key {
            code: KeyCode::Insert,
            mods: KeyModifiers::NONE,
        };
        let outcome = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::on_key(&mut searchable, &insert, &mut cx)
        };
        assert_eq!(outcome, Outcome::Consumed);
        assert_eq!(searchable.checked(), &[ItemKey::text("alpha")]);
        assert_eq!(
            <Picker as Layer<LayerResult, ()>>::take_result(&mut searchable),
            Some(LayerOut::Event(LayerResult::Picker(
                PickerAction::SelectionConflict {
                    attempted: ItemKey::text("beta"),
                    selected: ItemKey::text("alpha"),
                }
            )))
        );

        searchable
            .set_visible_items(vec![alpha])
            .expect("valid picker update");
        for (selected, expected) in [(false, &[][..]), (true, &[ItemKey::text("alpha")][..])] {
            let outcome = {
                let mut cx = LayerCx {
                    focus: &mut focus,
                    ring: &ring,
                    app: &app,
                };
                <Picker as Layer<LayerResult, ()>>::on_key(&mut searchable, &insert, &mut cx)
            };
            assert_eq!(outcome, Outcome::Changed);
            assert_eq!(searchable.checked(), expected);
            assert_eq!(
                <Picker as Layer<LayerResult, ()>>::take_result(&mut searchable),
                Some(LayerOut::Event(LayerResult::Picker(PickerAction::Toggled)))
            );
            assert_eq!(searchable.checked().is_empty(), !selected);
        }
    }

    #[test]
    fn searchable_refresh_prunes_disabled_selections_and_new_session_resets_them() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker
            .set_items(vec![Item::new("Alpha").key(ItemKey::text("alpha"))])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker
            .replace_items(vec![
                Item::new("Alpha")
                    .key(ItemKey::text("alpha"))
                    .disabled(true),
            ])
            .expect("valid picker update");
        assert!(picker.checked().is_empty());

        picker
            .set_items(vec![Item::new("Beta").key(ItemKey::text("beta"))])
            .expect("valid picker update");
        picker.toggle_checked(0);
        picker
            .set_items(vec![Item::new("Gamma").key(ItemKey::text("gamma"))])
            .expect("valid picker update");
        assert!(picker.checked().is_empty());
    }

    #[test]
    fn searchable_multi_rejects_position_keys() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        assert_eq!(
            picker.set_items(vec![Item::new("Alpha")]),
            Err(super::PickerItemsError::MissingStableKey)
        );
    }

    #[test]
    fn searchable_multi_rejects_duplicate_stable_keys() {
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        assert_eq!(
            picker.set_items(vec![
                Item::new("Same label").key(ItemKey::text("same-id")),
                Item::new("Same label").key(ItemKey::text("same-id")),
            ]),
            Err(super::PickerItemsError::DuplicateKey(ItemKey::text(
                "same-id"
            )))
        );
    }

    #[test]
    fn picker_layer_pointer_toggle_repaints_stays_open_and_skips_disabled_rows() {
        let area = Rect::new(0, 0, 100, 30);
        let mut picker = Picker::new(WidgetId::of("picker-test"), "Choose")
            .multi_select(true)
            .expect("empty picker accepts multi-selection")
            .searchable(false)
            .expect("disabling search is valid");
        picker
            .set_items(vec![
                Item::new("Alpha").key(ItemKey::text("alpha")),
                Item::new("Unavailable")
                    .key(ItemKey::text("unavailable"))
                    .disabled(true),
            ])
            .expect("valid picker update");
        let theme = Theme::for_level(ColorLevel::TrueColor);
        let mut focus = Focus::default();
        let app = ();
        let draw = |picker: &mut Picker, ring: &mut FocusRing| {
            let mut buffer = Buffer::empty(area);
            let mut hits = HitRegistry::default();
            let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, ring);
            picker.draw(area, &mut buffer, &mut ctx);
            ctx.flush_overlays(&mut buffer);
            (buffer, hits)
        };

        let mut ring = FocusRing::default();
        let (_, hits) = draw(&mut picker, &mut ring);
        let click = PointerEvent::new(
            &hits,
            Phase::Click,
            picker.row_id(0),
            Position::new(1, 1),
            1,
            KeyModifiers::NONE,
        );
        let outcome = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::pointer(&mut picker, &click, &mut cx)
        };
        assert_eq!(outcome, Outcome::Changed);
        assert!(matches!(
            <Picker as Layer<LayerResult, ()>>::take_result(&mut picker),
            Some(LayerOut::Event(LayerResult::Picker(PickerAction::Toggled)))
        ));
        assert_eq!(picker.checked(), &[ItemKey::text("alpha")]);

        let (buffer, hits) = draw(&mut picker, &mut ring);
        assert_eq!(
            buffer
                .content()
                .iter()
                .filter(|cell| cell.symbol() == theme.glyphs.tick)
                .count(),
            1
        );
        let click = PointerEvent::new(
            &hits,
            Phase::Click,
            picker.row_id(0),
            Position::new(1, 1),
            1,
            KeyModifiers::NONE,
        );
        let outcome = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::pointer(&mut picker, &click, &mut cx)
        };
        assert_eq!(outcome, Outcome::Changed);
        assert!(matches!(
            <Picker as Layer<LayerResult, ()>>::take_result(&mut picker),
            Some(LayerOut::Event(LayerResult::Picker(PickerAction::Toggled)))
        ));
        assert!(picker.checked().is_empty());

        let (_, hits) = draw(&mut picker, &mut ring);
        let disabled = PointerEvent::new(
            &hits,
            Phase::Click,
            picker.row_id(1),
            Position::new(1, 1),
            1,
            KeyModifiers::NONE,
        );
        let outcome = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::pointer(&mut picker, &disabled, &mut cx)
        };
        assert_eq!(outcome, Outcome::Consumed);
        assert!(<Picker as Layer<LayerResult, ()>>::take_result(&mut picker).is_none());
        assert!(picker.checked().is_empty());

        let backdrop = PointerEvent::new(
            &hits,
            Phase::Click,
            crate::ui::popup::backdrop_id(picker.id),
            Position::new(0, 0),
            1,
            KeyModifiers::NONE,
        );
        let outcome = {
            let mut cx = LayerCx {
                focus: &mut focus,
                ring: &ring,
                app: &app,
            };
            <Picker as Layer<LayerResult, ()>>::pointer(&mut picker, &backdrop, &mut cx)
        };
        assert_eq!(outcome, Outcome::Ignored);
        assert!(<Picker as Layer<LayerResult, ()>>::take_result(&mut picker).is_none());
        assert!(picker.checked().is_empty());
    }
    #[test]
    fn duplicate_picker_keys_are_rejected_without_mutation_or_panic() {
        for searchable in [false, true] {
            let mut picker = Picker::new(WidgetId::of("duplicate"), "Duplicate")
                .multi_select(true)
                .expect("empty picker accepts multi-selection");
            picker
                .set_searchable(searchable)
                .expect("valid picker update");
            picker
                .set_items(vec![Item::new("First").key(ItemKey::text("first"))])
                .expect("valid picker update");
            picker.toggle_checked(0);
            let before = picker.checked().to_vec();
            let cursor = picker.list.cursor.index;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                picker.set_items(vec![
                    Item::new("A").key(ItemKey::text("same")),
                    Item::new("B").key(ItemKey::text("same")),
                ])
            }));
            assert_eq!(
                result.unwrap(),
                Err(super::PickerItemsError::DuplicateKey(ItemKey::text("same")))
            );
            assert_eq!(picker.items[0].label, "First");
            assert_eq!(picker.list.cursor.index, cursor);
            assert_eq!(picker.checked(), before);
        }
    }

    #[test]
    fn explicit_index_picker_keys_survive_reorder() {
        let mut picker = Picker::new(WidgetId::of("explicit-index"), "Index")
            .multi_select(true)
            .expect("empty picker accepts multi-selection");
        picker.set_searchable(false).expect("valid picker update");
        let a = Item::new("A").key(ItemKey::index(17));
        let b = Item::new("B").key(ItemKey::index(29));
        picker
            .set_items(vec![a.clone(), b.clone()])
            .expect("valid picker update");
        picker.toggle_checked(0);
        assert_eq!(picker.checked(), &[ItemKey::index(17)]);
        picker
            .replace_items(vec![b, a])
            .expect("valid picker update");
        assert_eq!(picker.checked(), &[ItemKey::index(17)]);
        assert_eq!(picker.item_for_key(ItemKey::index(17)).unwrap().label, "A");
    }
    #[test]
    fn picker_rejects_ambiguous_and_unknown_keys_atomically() {
        let mut picker = Picker::new(WidgetId::of("keys"), "Keys")
            .multi_select(true)
            .expect("empty picker")
            .searchable(false)
            .expect("disable search");
        let first = Item::new("First").key(ItemKey::index(17));
        let second = Item::new("Second").key(ItemKey::index(29));
        assert_eq!(
            picker.set_items(vec![first.clone(), second.clone()]),
            Ok(())
        );
        picker.toggle_checked(0);
        let cursor = picker.list.cursor.index;
        assert_eq!(
            picker.replace_items(vec![first.clone(), first.clone()]),
            Err(super::PickerItemsError::DuplicateKey(ItemKey::index(17)))
        );
        assert_eq!(picker.list.cursor.index, cursor);
        assert_eq!(picker.checked(), &[ItemKey::index(17)]);
        assert_eq!(picker.items.len(), 2);
        assert_eq!(
            picker.set_visible_items(vec![Item::new("Unknown").key(ItemKey::index(41))]),
            Err(super::PickerItemsError::UnknownVisibleKey(ItemKey::index(
                41
            )))
        );
        assert_eq!(
            picker.set_visible_items(vec![first.clone(), first.clone()]),
            Err(super::PickerItemsError::DuplicateKey(ItemKey::index(17)))
        );
        assert_eq!(picker.set_visible_items(vec![second.clone()]), Ok(()));
        assert_eq!(picker.checked(), &[ItemKey::index(17)]);
        assert_eq!(
            picker.replace_items(vec![second, first.disabled(true)]),
            Ok(())
        );
        assert!(picker.checked().is_empty());
    }

    #[test]
    fn picker_implicit_positions_cannot_alias_explicit_indices() {
        let mut picker = Picker::new(WidgetId::of("positions"), "Positions")
            .multi_select(true)
            .expect("empty picker")
            .searchable(false)
            .expect("disable search");
        assert_eq!(
            picker.set_items(vec![
                Item::new("Implicit"),
                Item::new("Explicit").key(ItemKey::index(0))
            ]),
            Err(super::PickerItemsError::DuplicateKey(ItemKey::index(0)))
        );
        assert_eq!(picker.set_items(vec![Item::new("Implicit")]), Ok(()));
        picker.toggle_checked(0);
        assert_eq!(picker.checked(), &[ItemKey::index(0)]);
        assert_eq!(
            picker.replace_items(vec![Item::new("Explicit").key(ItemKey::index(0))]),
            Ok(())
        );
        assert!(picker.checked().is_empty());
    }

    #[test]
    fn enabling_searchable_multi_rejects_unstable_items() {
        let mut picker = Picker::new(WidgetId::of("mode"), "Mode");
        picker
            .set_items(vec![Item::new("Implicit")])
            .expect("valid picker update");
        let result = picker.multi_select(true);
        assert!(matches!(
            result,
            Err(super::PickerItemsError::MissingStableKey)
        ));
        let mut picker = Picker::new(WidgetId::of("mode2"), "Mode")
            .multi_select(true)
            .expect("empty picker")
            .searchable(false)
            .expect("disable search");
        picker
            .set_items(vec![Item::new("Implicit")])
            .expect("valid picker update");
        assert_eq!(
            picker.set_searchable(true),
            Err(super::PickerItemsError::MissingStableKey)
        );
        assert!(!picker.is_searchable());
        assert_eq!(picker.items[0].label, "Implicit");
    }
}
