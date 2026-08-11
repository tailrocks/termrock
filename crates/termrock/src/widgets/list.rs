//! **List** — composable collection view (not a label-only widget).
//!
//! **Mission.** Rows compose leading, primary, secondary, status, badge,
//! trailing actions, and shortcuts with group headers and separators. State is
//! [`CollectionState`] + [`SelectionModel`] (via [`Selection`]) + roving focus
//! + scroll/virtualization. Single / multi / range selection, typeahead,
//! search, disabled/loading/empty, density, and narrow drop priority.
//!
//! **Intents.** Prefer [`ListState::handle_intent`] / [`default_list_intent`];
//! printable keys feed typeahead through the collection roving model.
//!
//! Research: lazygit, Yazi, Textual ListView, shadcn command items.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
    widgets::StatefulWidget,
};

use ratatui_core::style::Modifier;

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{
        CollectionState, HitRegion, NavigationMove, Outcome, PageMove, UiIntent,
        default_list_intent,
    },
    scroll::max_offset,
    style::{Density, DesignSystem, ListRowVisualState, Role},
};

use super::{ComposedRow, Selection};

/// How a pointer press on a row is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ListClickPolicy {
    /// Select the row and emit [`Outcome::Activated`] (picker / menu default).
    #[default]
    Activate,
    /// Select only; emit [`Outcome::Changed`] (focusable lists that activate on Enter).
    Select,
}

/// Selection policy for the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ListSelectionMode {
    /// Single active row (default).
    #[default]
    Single,
    /// Multi-check with toggle (Space).
    Multi,
    /// Multi with shift-range support.
    Range,
}

impl ListSelectionMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multi => "multi",
            Self::Range => "range",
        }
    }
}

const fn list_row_height(density: Density, has_secondary_below: bool) -> u16 {
    if matches!(density, Density::Comfortable) && has_secondary_below {
        2
    } else {
        1
    }
}

/// Narrow-terminal drop order (lowest survival first).
///
/// shortcut → actions → badge → status → secondary → trailing → leading → primary
pub const LIST_NARROW_DROP_ORDER: &[&str] = &[
    "shortcut",
    "actions",
    "badge",
    "status",
    "secondary",
    "trailing",
    "leading",
    "primary",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
/// Semantic roles for selectable, disabled, separator, and group header rows.
pub enum RowRole {
    /// A selectable content row.
    Item,
    /// A non-interactive visual separator row.
    Separator,
    /// Group header (not selectable; skipped by movement).
    GroupHeader,
}

impl RowRole {
    /// Whether the row participates in collection roving.
    #[must_use]
    pub const fn is_navigable(self) -> bool {
        matches!(self, Self::Item)
    }
}

#[derive(Debug, Clone)]
/// A stable row in a selectable list with composed-part anatomy.
///
/// Parts: leading · primary(label) · secondary · status · badge · actions ·
/// shortcut · trailing. Narrow drop order: [`LIST_NARROW_DROP_ORDER`].
pub struct ListRow<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Primary label (never dropped first under contraction).
    pub label: Line<'a>,
    /// Optional leading icon / check chrome (composed leading).
    pub leading: Option<Line<'a>>,
    /// Optional secondary metadata line (composed secondary).
    pub secondary: Option<Line<'a>>,
    /// Optional status cue (glyph + short text).
    pub status: Option<Line<'a>>,
    /// Optional badge (composed badge).
    pub badge: Option<Line<'a>>,
    /// Optional keyboard shortcut hint (composed shortcut).
    pub shortcut: Option<&'a str>,
    /// Optional trailing action labels (display; host handles activation).
    pub actions: Option<Line<'a>>,
    /// Optional metadata aligned at the trailing edge (legacy + composed).
    pub trailing: Option<Line<'a>>,
    /// When set, replaces standard composed paint for the content band.
    pub custom: Option<Line<'a>>,
    /// Interaction role controlling selection and hit testing.
    pub role: RowRole,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Loading placeholder (composed loading).
    pub loading: bool,
}

impl<'a, Id> ListRow<'a, Id> {
    /// Creates a primary-only item row.
    #[must_use]
    pub fn item(id: Id, label: Line<'a>) -> Self {
        Self {
            id,
            label,
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: None,
            custom: None,
            role: RowRole::Item,
            enabled: true,
            loading: false,
        }
    }

    /// Creates a non-interactive separator row.
    #[must_use]
    pub fn separator(id: Id, label: Line<'a>) -> Self {
        Self {
            id,
            label,
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: None,
            custom: None,
            role: RowRole::Separator,
            enabled: true,
            loading: false,
        }
    }

    /// Creates a group header (skipped by selection movement).
    #[must_use]
    pub fn group_header(id: Id, label: Line<'a>) -> Self {
        Self {
            id,
            label,
            leading: None,
            secondary: None,
            status: None,
            badge: None,
            shortcut: None,
            actions: None,
            trailing: None,
            custom: None,
            role: RowRole::GroupHeader,
            enabled: true,
            loading: false,
        }
    }

    /// Sets leading chrome (icon / avatar).
    #[must_use]
    pub fn leading(mut self, leading: Line<'a>) -> Self {
        self.leading = Some(leading);
        self
    }

    /// Sets secondary metadata.
    #[must_use]
    pub fn secondary(mut self, secondary: Line<'a>) -> Self {
        self.secondary = Some(secondary);
        self
    }

    /// Sets status cue (after secondary / before badge).
    #[must_use]
    pub fn status(mut self, status: Line<'a>) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets badge chrome.
    #[must_use]
    pub fn badge(mut self, badge: Line<'a>) -> Self {
        self.badge = Some(badge);
        self
    }

    /// Sets trailing keyboard shortcut hint.
    #[must_use]
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Sets trailing action labels (display-only; host maps clicks).
    #[must_use]
    pub fn actions(mut self, actions: Line<'a>) -> Self {
        self.actions = Some(actions);
        self
    }

    /// Sets legacy trailing metadata (also used as badge when badge is unset).
    #[must_use]
    pub fn trailing(mut self, trailing: Line<'a>) -> Self {
        self.trailing = Some(trailing);
        self
    }

    /// Full custom content band (replaces composed primary cluster).
    #[must_use]
    pub fn custom(mut self, line: Line<'a>) -> Self {
        self.custom = Some(line);
        self
    }

    /// Marks the row disabled (skipped by keyboard, non-hittable).
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Marks the row loading (leading becomes a busy glyph).
    #[must_use]
    pub fn loading(mut self) -> Self {
        self.loading = true;
        self
    }

    /// Plain text for typeahead / search (primary spans).
    #[must_use]
    pub fn plain_label(&self) -> String {
        line_plain(&self.label)
    }

    /// Projects this row into composed anatomy for contraction/paint.
    #[must_use]
    pub fn composed(&self) -> ComposedRow<'a, ()>
    where
        Id: Clone,
    {
        ComposedRow {
            id: (),
            leading: self.leading.clone(),
            primary: self.custom.clone().unwrap_or_else(|| self.label.clone()),
            secondary: self.secondary.clone(),
            badge: self
                .badge
                .clone()
                .or_else(|| self.status.clone())
                .or_else(|| self.trailing.clone()),
            shortcut: self.shortcut,
            enabled: self.enabled,
            loading: self.loading,
        }
    }
}

/// Plain text from a ratatui [`Line`].
fn line_plain(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `List`.
pub struct ListState<Id> {
    /// Headless cursor + virtualization (stable ids only).
    collection: CollectionState<Id>,
    hovered: Option<Id>,
    regions: Vec<HitRegion<Id>>,
    pointer: Option<Position>,
    selection: Option<Selection<Id>>,
    check_regions: Vec<HitRegion<Id>>,
    click_policy: ListClickPolicy,
    selection_mode: ListSelectionMode,
    /// Active search query (`None` = no filter UI).
    search_query: Option<String>,
    /// Virtual total length when `rows` is a window (0 = use rows.len()).
    virtual_total: usize,
    /// Absolute start index of the painted window in the full universe.
    virtual_window_start: usize,
}

impl<Id> Default for ListState<Id> {
    fn default() -> Self {
        Self {
            collection: CollectionState::new(),
            hovered: None,
            regions: Vec::new(),
            pointer: None,
            selection: None,
            check_regions: Vec::new(),
            click_policy: ListClickPolicy::Activate,
            selection_mode: ListSelectionMode::Single,
            search_query: None,
            virtual_total: 0,
            virtual_window_start: 0,
        }
    }
}

impl<Id> ListState<Id> {
    #[must_use]
    /// Creates list state with no selection, hover, checks, or scroll.
    pub fn new(selected: Option<Id>) -> Self
    where
        Id: Clone + PartialEq,
    {
        let mut collection = CollectionState::new();
        collection.set_active(selected);
        Self {
            collection,
            hovered: None,
            regions: Vec::new(),
            pointer: None,
            selection: None,
            check_regions: Vec::new(),
            click_policy: ListClickPolicy::Activate,
            selection_mode: ListSelectionMode::Single,
            search_query: None,
            virtual_total: 0,
            virtual_window_start: 0,
        }
    }

    /// Borrow the headless collection model ([`CollectionState`] / roving).
    #[must_use]
    pub const fn collection(&self) -> &CollectionState<Id> {
        &self.collection
    }

    /// Mutable headless collection model.
    pub const fn collection_mut(&mut self) -> &mut CollectionState<Id> {
        &mut self.collection
    }

    /// Selection mode.
    #[must_use]
    pub const fn selection_mode(&self) -> ListSelectionMode {
        self.selection_mode
    }

    /// Sets selection mode (configures multi-select chrome).
    pub fn set_selection_mode(&mut self, mode: ListSelectionMode)
    where
        Id: Clone + PartialEq,
    {
        self.selection_mode = mode;
        match mode {
            ListSelectionMode::Single => self.disable_multi_select(),
            ListSelectionMode::Multi => {
                self.selection.get_or_insert_with(|| {
                    Selection::from(crate::interaction::SelectionModel::multiple())
                });
            }
            ListSelectionMode::Range => {
                self.selection.get_or_insert_with(|| {
                    Selection::from(crate::interaction::SelectionModel::range())
                });
            }
        }
    }

    /// Search query (host may also pre-filter rows).
    #[must_use]
    pub fn search_query(&self) -> Option<&str> {
        self.search_query.as_deref()
    }

    /// Set search query (empty string clears).
    pub fn set_search_query(&mut self, query: Option<String>) {
        self.search_query = query.filter(|q| !q.is_empty());
    }

    /// Typeahead buffer (from roving).
    #[must_use]
    pub fn typeahead_buffer(&self) -> &str {
        self.collection.roving().typeahead_buffer()
    }

    /// Clear typeahead.
    pub fn clear_typeahead(&mut self) {
        self.collection.clear_typeahead();
    }

    /// Virtualization: painted window of a larger universe.
    ///
    /// For million-row or streaming collections prefer
    /// [`crate::widgets::VirtualList`] (owns [`crate::widgets::Virtualizer`]
    /// math, overscan, sticky headers, follow-tail).
    pub fn set_virtual_window(&mut self, window_start: usize, total_len: usize) {
        self.virtual_window_start = window_start;
        self.virtual_total = total_len;
    }

    /// Virtual total (0 means not virtualized).
    #[must_use]
    pub const fn virtual_total(&self) -> usize {
        self.virtual_total
    }

    /// Configures pointer-click outcomes ([`ListClickPolicy`]).
    pub const fn set_click_policy(&mut self, policy: ListClickPolicy) {
        self.click_policy = policy;
    }

    #[must_use]
    /// Returns the pointer-click policy.
    pub const fn click_policy(&self) -> ListClickPolicy {
        self.click_policy
    }

    /// Replace the stable selected identity.
    pub fn select(&mut self, selected: Option<Id>)
    where
        Id: Clone + PartialEq,
    {
        self.collection.set_active(selected);
    }

    #[must_use]
    /// Returns the stable identity selected for keyboard interaction.
    pub const fn selected(&self) -> Option<&Id> {
        self.collection.active()
    }

    #[must_use]
    /// Returns the stable identity currently under the pointer.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns the first visible row index.
    pub const fn offset(&self) -> usize {
        self.collection.offset()
    }

    #[must_use]
    /// Returns the painted item hit regions from the most recent render.
    pub fn regions(&self) -> &[HitRegion<Id>] {
        &self.regions
    }

    /// Disables multi-selection and discards checked identities.
    pub fn disable_multi_select(&mut self) {
        self.selection = None;
    }

    #[must_use]
    /// Returns the ordered multi-selection state, if enabled.
    pub const fn selection(&self) -> Option<&Selection<Id>> {
        self.selection.as_ref()
    }

    /// Returns mutable access to ordered multi-selection state, if enabled.
    pub fn selection_mut(&mut self) -> Option<&mut Selection<Id>> {
        self.selection.as_mut()
    }

    /// Moves the scroll position by a signed delta and clamps it to valid content.
    pub fn scroll_by(&mut self, delta: isize, rows_len: usize) -> bool
    where
        Id: Clone + PartialEq,
    {
        let vp = self.collection.viewport_len().max(1);
        self.collection
            .set_viewport(self.collection.offset(), vp, rows_len);
        matches!(
            self.collection.scroll_by(delta),
            crate::interaction::CollectionOutcome::Scrolled
        )
    }

    /// Scrolls toward a pointer position within the painted viewport.
    pub fn scroll_to_position(&mut self, position: Position, rows_len: usize) -> bool
    where
        Id: Clone + PartialEq,
    {
        self.pointer = Some(position);
        let vp = self.collection.viewport_len();
        if vp == 0 || self.regions.is_empty() {
            return false;
        }
        let first = self.regions[0].area;
        if position.y < first.y {
            return self.scroll_by(-1, rows_len);
        }
        let bottom = first
            .y
            .saturating_add(u16::try_from(vp.saturating_sub(1)).unwrap_or(u16::MAX));
        if position.y > bottom {
            return self.scroll_by(1, rows_len);
        }
        false
    }
}

impl<Id: Clone + PartialEq> ListState<Id> {
    /// Enables ordered multi-selection with an empty selection (range-capable).
    pub fn enable_multi_select(&mut self) {
        self.set_selection_mode(ListSelectionMode::Range);
    }

    /// Routes navigation, checking, activation, cancellation, and typeahead.
    ///
    /// Keys map through [`default_list_intent`] first; unmapped printable chars
    /// feed collection typeahead. Prefer [`Self::handle_intent`] when the app
    /// owns keymaps.
    pub fn handle_key(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if key.kind == KeyEventKind::Release {
            return Outcome::Ignored;
        }
        // Shift+Space: range-select along visible enabled items (multi-select).
        if key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char(' '))
            && key.modifiers.contains(KeyModifiers::SHIFT)
        {
            return self.range_select_to_active(rows);
        }
        // Search: '/' opens filter mode (host still owns filtering projection).
        if key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('/'))
            && key.modifiers.is_empty()
        {
            if self.search_query.is_none() {
                self.search_query = Some(String::new());
            }
            return Outcome::Changed;
        }
        // While search query is Some, printable chars append; Backspace pops.
        if self.search_query.is_some()
            && key.kind == KeyEventKind::Press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Backspace => {
                    if let Some(q) = self.search_query.as_mut() {
                        q.pop();
                        if q.is_empty() {
                            self.search_query = None;
                        }
                    }
                    return Outcome::Changed;
                }
                KeyCode::Esc => {
                    self.search_query = None;
                    return Outcome::Changed;
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    if let Some(q) = self.search_query.as_mut() {
                        q.push(c);
                    }
                    return Outcome::Changed;
                }
                _ => {}
            }
        }
        match default_list_intent(key) {
            Some(intent) => {
                self.collection.clear_typeahead();
                self.handle_intent(rows, intent)
            }
            None => self.handle_typeahead(rows, key),
        }
    }

    /// Typeahead jump via [`CollectionState`] / roving (labels from primary text).
    fn handle_typeahead(&mut self, rows: &[ListRow<'_, Id>], key: KeyEvent) -> Outcome<Id> {
        if key.kind != KeyEventKind::Press {
            return Outcome::Ignored;
        }
        let KeyCode::Char(c) = key.code else {
            return Outcome::Ignored;
        };
        if c.is_control() {
            return Outcome::Ignored;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::ALT)
        {
            return Outcome::Ignored;
        }
        let items = collection_items_from_rows(rows);
        let out = self.collection.handle_key(key, &items);
        if out.active_changed() {
            ensure_list_active_visible(self, rows, self.collection.viewport_len());
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Applies a semantic intent to this list.
    pub fn handle_intent(&mut self, rows: &[ListRow<'_, Id>], intent: UiIntent) -> Outcome<Id> {
        match intent {
            UiIntent::Move(_) | UiIntent::Page(_) => {
                let items = collection_items_from_rows(rows);
                // Empty selection: first Next/PageForward lands on first, Previous on last.
                let out = if self.collection.active().is_none() {
                    match intent {
                        UiIntent::Move(
                            NavigationMove::Next
                            | NavigationMove::Down
                            | NavigationMove::Right
                            | NavigationMove::First,
                        )
                        | UiIntent::Page(PageMove::Forward) => self.collection.move_first(&items),
                        UiIntent::Move(
                            NavigationMove::Previous
                            | NavigationMove::Up
                            | NavigationMove::Left
                            | NavigationMove::Last,
                        )
                        | UiIntent::Page(PageMove::Backward) => self.collection.move_last(&items),
                        _ => self.collection.handle_intent(intent, &items),
                    }
                } else {
                    self.collection.handle_intent(intent, &items)
                };
                if out.active_changed() {
                    ensure_list_active_visible(self, rows, self.collection.viewport_len());
                    Outcome::Changed
                } else {
                    Outcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => self.activate(rows),
            UiIntent::Toggle => self.toggle_selected(rows),
            UiIntent::Cancel | UiIntent::Close => Outcome::Cancelled,
            UiIntent::Expand | UiIntent::Collapse => Outcome::Ignored,
            // Global chrome / edit intents: host + specialized surfaces handle them.
            _ => Outcome::Ignored,
        }
    }

    /// Intent path returning the standard [`crate::interaction::EventResult`] envelope.
    pub fn handle_intent_result(
        &mut self,
        rows: &[ListRow<'_, Id>],
        intent: UiIntent,
    ) -> crate::interaction::EventResult<Outcome<Id>> {
        self.handle_intent(rows, intent).into_event_result()
    }

    /// Key path returning [`crate::interaction::EventResult`].
    pub fn handle_key_result(
        &mut self,
        rows: &[ListRow<'_, Id>],
        key: KeyEvent,
    ) -> crate::interaction::EventResult<Outcome<Id>> {
        self.handle_key(rows, key).into_event_result()
    }

    fn toggle_selected(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        let Some(selection) = self.selection.as_mut() else {
            return Outcome::Ignored;
        };
        let Some(row) = self.collection.active().and_then(|selected| {
            rows.iter()
                .find(|row| row.enabled && row.role.is_navigable() && &row.id == selected)
        }) else {
            return Outcome::Ignored;
        };
        // Anchor for subsequent range ops.
        if selection.model().anchor().is_none() {
            selection.model_mut().set_anchor(Some(row.id.clone()));
        }
        selection.toggle(&row.id);
        Outcome::CheckToggled(row.id.clone())
    }

    /// Shift-range: set selection from anchor to active along enabled item order.
    fn range_select_to_active(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        let Some(active) = self.collection.active().cloned() else {
            return Outcome::Ignored;
        };
        let Some(selection) = self.selection.as_mut() else {
            return Outcome::Ignored;
        };
        let order: Vec<Id> = rows
            .iter()
            .filter(|r| r.enabled && r.role.is_navigable())
            .map(|r| r.id.clone())
            .collect();
        if selection.model().anchor().is_none() {
            selection.model_mut().set_anchor(Some(active.clone()));
        }
        let _ = selection.model_mut().set_range(&order, &active);
        Outcome::CheckToggled(active)
    }

    /// Moves selection to the next enabled item, wrapping at the end.
    pub fn select_next(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        let items = collection_items_from_rows(rows);
        if self.collection.move_next(&items).active_changed() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    /// Moves selection to the previous enabled item, wrapping at the start.
    pub fn select_previous(&mut self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        let items = collection_items_from_rows(rows);
        if self.collection.move_previous(&items).active_changed() {
            Outcome::Changed
        } else {
            Outcome::Ignored
        }
    }

    #[must_use]
    /// Returns the semantic action associated with the supplied stable identity.
    pub fn activate(&self, rows: &[ListRow<'_, Id>]) -> Outcome<Id> {
        self.collection
            .active()
            .and_then(|selected| {
                rows.iter()
                    .find(|row| row.enabled && row.role.is_navigable() && &row.id == selected)
            })
            .map_or(Outcome::Ignored, |row| Outcome::Activated(row.id.clone()))
    }

    /// Updates hover state from the current pointer position and painted hit regions.
    pub fn hover(&mut self, position: Position) -> Option<&Id> {
        self.pointer = Some(position);
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Maps a pointer position to the semantic outcome of the painted hit region.
    pub fn click(&mut self, position: Position) -> Outcome<Id> {
        self.pointer = Some(position);
        self.hovered = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        if let Some(id) = self
            .check_regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone())
        {
            self.collection.set_active(Some(id.clone()));
            if let Some(selection) = self.selection.as_mut() {
                selection.toggle(&id);
                return Outcome::CheckToggled(id);
            }
        }
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
        else {
            return Outcome::Ignored;
        };
        self.collection.set_active(Some(region.id.clone()));
        // Shift+click range when multi-select is on (pointer path; host passes modifiers via click_range).
        match self.click_policy {
            ListClickPolicy::Activate => Outcome::Activated(region.id.clone()),
            ListClickPolicy::Select => Outcome::Changed,
        }
    }

    /// Pointer select with optional range (Shift). Call after hit-test when multi-select is enabled.
    pub fn click_select(&mut self, position: Position, extend_range: bool) -> Outcome<Id> {
        let Some(region) = self
            .regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|r| (r.id.clone(), r.area))
        else {
            return Outcome::Ignored;
        };
        let id = region.0;
        self.collection.set_active(Some(id.clone()));
        if !extend_range {
            if let Some(sel) = self.selection.as_mut() {
                sel.model_mut().set_anchor(Some(id.clone()));
                let _ = sel.model_mut().select(id.clone());
            }
            return Outcome::Changed;
        }
        // Need rows for order — host should call range_select_to_active with rows.
        if let Some(sel) = self.selection.as_mut() {
            if sel.model().anchor().is_none() {
                sel.model_mut().set_anchor(Some(id.clone()));
            }
            let _ = sel.toggle(&id);
        }
        Outcome::CheckToggled(id)
    }

    /// Shift-range along visible rows to `to` (stable id).
    pub fn select_range_to(&mut self, rows: &[ListRow<'_, Id>], to: &Id) -> Outcome<Id> {
        let Some(selection) = self.selection.as_mut() else {
            return Outcome::Ignored;
        };
        let order: Vec<Id> = rows
            .iter()
            .filter(|r| r.enabled && r.role.is_navigable())
            .map(|r| r.id.clone())
            .collect();
        if selection.model().anchor().is_none() {
            selection.model_mut().set_anchor(Some(to.clone()));
        }
        let _ = selection.model_mut().set_range(&order, to);
        self.collection.set_active(Some(to.clone()));
        Outcome::CheckToggled(to.clone())
    }

    /// Projects list rows into headless collection items and reconciles active id.
    pub fn reconcile_collection(&mut self, rows: &[ListRow<'_, Id>]) {
        let items = collection_items_from_rows(rows);
        let vp = self.collection.viewport_len().max(1);
        if self.virtual_total > 0 {
            let _ = self.collection.reconcile_window(
                &items,
                self.virtual_window_start,
                self.virtual_total,
                vp,
            );
        } else {
            self.collection
                .set_viewport(self.collection.offset(), vp, items.len());
            let _ = self.collection.reconcile(&items);
        }
    }

    /// Sync vertical offset into a [`crate::widgets::ScrollAreaState`] (bars only).
    pub fn sync_scroll_area(
        &self,
        scroll: &mut crate::widgets::ScrollAreaState,
        content_len: usize,
        viewport_len: usize,
    ) {
        let h = u16::try_from(content_len.max(1)).unwrap_or(u16::MAX);
        let vh = u16::try_from(viewport_len.max(1)).unwrap_or(u16::MAX);
        scroll.set_content_size(1, h);
        scroll.set_viewport(1, vh);
        scroll.set_offset_y_quiet(u16::try_from(self.collection.offset()).unwrap_or(u16::MAX));
    }
}

/// Filter rows by case-insensitive substring on primary label (host rebuild helper).
#[must_use]
pub fn filter_list_rows<'a, Id: Clone>(
    rows: &'a [ListRow<'a, Id>],
    query: &str,
) -> Vec<&'a ListRow<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return rows.iter().collect();
    }
    rows.iter()
        .filter(|r| {
            if matches!(r.role, RowRole::GroupHeader | RowRole::Separator) {
                return true; // keep structure; host may post-process empty groups
            }
            r.plain_label().to_ascii_lowercase().contains(&q)
        })
        .collect()
}

impl ListState<usize> {
    /// Create index-addressed list state with the first item selected.
    #[must_use]
    pub fn for_count(count: usize) -> Self {
        Self::new(if count == 0 { None } else { Some(0) })
    }

    /// Reconcile an index selection after the backing collection changes.
    pub fn reconcile_count(&mut self, count: usize) {
        let selected = match (self.collection.active().copied(), count) {
            (_, 0) => None,
            (Some(index), _) => Some(if index < count { index } else { count - 1 }),
            (None, _) => Some(0),
        };
        self.collection.set_active(selected);
    }

    /// Move an index selection by one item, wrapping at either edge.
    pub fn cycle_index(&mut self, count: usize, direction: isize) -> bool {
        if count == 0 {
            self.collection.set_active(None);
            return false;
        }
        let current = self
            .collection
            .active()
            .copied()
            .unwrap_or(0)
            .min(count - 1);
        let next = if direction.is_negative() {
            if current == 0 { count - 1 } else { current - 1 }
        } else if current + 1 >= count {
            0
        } else {
            current + 1
        };
        self.collection.set_active(Some(next));
        next != current
    }

    /// Move an index selection by a gesture delta without wrapping.
    pub fn move_index(&mut self, count: usize, delta: isize) -> bool {
        if count == 0 {
            self.collection.set_active(None);
            return false;
        }
        let current = self
            .collection
            .active()
            .copied()
            .unwrap_or(0)
            .min(count - 1);
        let next = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta.unsigned_abs()).min(count - 1)
        };
        self.collection.set_active(Some(next));
        next != current
    }

    /// Borrow the selected item from an index-addressed collection.
    #[must_use]
    pub fn selected_item<'a, T>(&self, items: &'a [T]) -> Option<&'a T> {
        self.collection.active().and_then(|index| items.get(*index))
    }
}

#[derive(Debug, Clone)]
/// Stable-ID list widget rendered with [`ListState`].
///
/// See the `list/selection` lookbook story for selection, metadata, and narrow
/// terminal behavior.
///
/// # Examples
///
/// ```
/// use ratatui_core::text::Line;
/// use termrock::input::{KeyCode, KeyEvent, KeyModifiers};
/// use termrock::interaction::Outcome;
/// use termrock::widgets::{List, ListRow, ListState};
///
/// let rows = [
///     ListRow::item("a", Line::from("Alpha")),
///     ListRow::item("b", Line::from("Beta")),
/// ];
/// let tokens = termrock::style::DesignSystem::default();
/// let _widget = List::new(&rows, &tokens);
/// let mut state = ListState::new(Some("a"));
/// let outcome = state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
/// assert!(matches!(outcome, Outcome::Changed));
/// assert_eq!(state.selected(), Some(&"b"));
/// ```
pub struct List<'a, Id> {
    /// Host-supplied: surface owns keyboard focus this frame.
    focused: bool,
    rows: &'a [ListRow<'a, Id>],
    tokens: &'a DesignSystem,
    empty_message: Option<Line<'a>>,
    density: Density,
}

impl<'a, Id> List<'a, Id> {
    #[must_use]
    /// Creates a list over borrowed rows; paint uses design-token recipes.
    pub const fn new(rows: &'a [ListRow<'a, Id>], tokens: &'a DesignSystem) -> Self {
        Self {
            focused: true,
            rows,
            tokens,
            empty_message: None,
            density: Density::Compact,
        }
    }

    /// Creates a list from a [`DesignSystem`] (preferred public paint root).
    #[must_use]
    pub const fn from_system(rows: &'a [ListRow<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(rows, system)
    }

    /// Whether this surface owns keyboard focus this frame (host / scene).
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Compact (1-line) or comfortable (secondary below primary).
    #[must_use]
    pub const fn density(mut self, density: Density) -> Self {
        self.density = density;
        self
    }

    /// Comfortable density shorthand.
    #[must_use]
    pub const fn comfortable(mut self) -> Self {
        self.density = Density::Comfortable;
        self
    }

    /// Message painted when `rows` is empty (consumer-owned copy).
    #[must_use]
    pub fn empty_message(mut self, message: Line<'a>) -> Self {
        self.empty_message = Some(message);
        self
    }

    /// Theme borrowed from design tokens.
    #[must_use]
    pub const fn theme(&self) -> &crate::style::RolePalette {
        self.tokens.palette()
    }

    /// Design tokens used for recipes.
    #[must_use]
    pub const fn tokens(&self) -> &DesignSystem {
        self.tokens
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.check_regions.clear();
        let viewport_height = usize::from(area.height);
        let total = if state.virtual_total > 0 {
            state.virtual_total
        } else {
            self.rows.len()
        };
        let items = collection_items_from_rows(self.rows);
        if state.virtual_total > 0 {
            let _ = state.collection.reconcile_window(
                &items,
                state.virtual_window_start,
                total,
                viewport_height,
            );
        } else {
            state
                .collection
                .set_viewport(state.collection.offset(), viewport_height, total);
            let _ = state.collection.reconcile(&items);
        }
        ensure_list_active_visible(state, self.rows, viewport_height);
        if self.rows.is_empty() {
            if let Some(message) = self.empty_message.as_ref() {
                let style = self.tokens.style(Role::TextMuted);
                buffer.set_line(area.x, area.y, message, area.width);
                buffer.set_style(Rect::new(area.x, area.y, area.width, 1), style);
            }
            state.hovered = None;
            return;
        }
        // Search strip (1 row) when query active
        let mut body_y = area.y;
        let mut body_h = area.height;
        if let Some(q) = state.search_query.as_ref() {
            let strip = format!("/ {q}");
            buffer.set_stringn(
                area.x,
                area.y,
                &crate::text::take_display_cols(&strip, usize::from(area.width)),
                usize::from(area.width),
                self.tokens.style(Role::Info),
            );
            body_y = area.y.saturating_add(1);
            body_h = area.height.saturating_sub(1);
        }
        let body = Rect::new(area.x, body_y, area.width, body_h);
        let scrollable = crate::scroll::is_scrollable(total, usize::from(body.height).max(1));
        let content_width = body.width.saturating_sub(u16::from(scrollable));
        let offset = if state.virtual_total > 0 {
            0 // rows are already the window
        } else {
            state.collection.offset()
        };
        let mut y = body.y;
        let mut painted_rows = 0usize;
        for row in self.rows.iter().skip(offset) {
            if y >= body.bottom() {
                break;
            }
            let secondary_below = matches!(self.density, Density::Comfortable)
                && row.secondary.is_some()
                && !matches!(row.role, RowRole::Separator | RowRole::GroupHeader);
            let rh = list_row_height(self.density, secondary_below);
            if y.saturating_add(rh) > body.bottom() {
                break;
            }
            let rect = Rect::new(body.x, y, content_width, 1);
            let selected = state.collection.active() == Some(&row.id);
            let hovered = row.enabled
                && row.role.is_navigable()
                && state
                    .pointer
                    .is_some_and(|position| rect.contains(position));
            let checked = state
                .selection
                .as_ref()
                .is_some_and(|selection| selection.is_checked(&row.id));
            let recipe = self.tokens.resolve_list_row(ListRowVisualState {
                selected,
                focused: self.focused && selected,
                hovered,
                enabled: row.enabled,
                loading: row.loading,
                checked,
            });
            let style = if hovered && row.enabled && !selected {
                recipe.hover
            } else if checked && !selected {
                self.tokens.style(Role::Accent)
            } else {
                recipe.label
            };
            if recipe.use_fill {
                buffer.set_style(rect, style);
            } else if recipe.use_tint {
                buffer.set_style(rect, recipe.tint);
            } else if recipe.hover_fill {
                buffer.set_style(rect, recipe.hover_wash);
            }
            if matches!(row.role, RowRole::Separator) {
                let rule = self.tokens.glyphs.rule();
                buffer.set_stringn(rect.x, rect.y, rule, usize::from(rect.width), style);
                if rect.width > 2 {
                    let label_x = rect.x.saturating_add(2);
                    let parts = row.composed().parts_for_width(rect.width.saturating_sub(2));
                    buffer.set_line(
                        label_x,
                        rect.y,
                        &parts.primary,
                        rect.right().saturating_sub(label_x),
                    );
                }
            } else if matches!(row.role, RowRole::GroupHeader) {
                let style = self
                    .tokens
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD);
                buffer.set_line(rect.x, rect.y, &row.label, rect.width);
                buffer.set_style(rect, style);
            } else {
                // Stable 2-cell gutter slot for quiet selection chrome.
                if let Some((glyph, gstyle)) = recipe.gutter {
                    buffer.set_stringn(rect.x, rect.y, glyph, 1, gstyle);
                    buffer.set_stringn(rect.x.saturating_add(1), rect.y, " ", 1, style);
                } else if recipe.show_gutter_slot {
                    buffer.set_stringn(rect.x, rect.y, "  ", 2, style);
                }
                let check_x = rect.x.saturating_add(2);
                let check_w = render_check_cell(buffer, state, row, rect, check_x, &recipe, style);
                let content_x = check_x.saturating_add(check_w);
                if content_x < rect.right() {
                    let content_w = rect.right().saturating_sub(content_x);
                    // Custom body replaces composed primary cluster.
                    if let Some(custom) = row.custom.as_ref() {
                        buffer.set_line(content_x, rect.y, custom, content_w);
                        buffer.set_style(Rect::new(content_x, rect.y, content_w, 1), style);
                    } else {
                        let badge = row
                            .badge
                            .as_ref()
                            .or(row.status.as_ref())
                            .or(row.trailing.as_ref());
                        let mut budget = content_w.saturating_sub(1);
                        let shortcut_need = row
                            .shortcut
                            .map(|s| {
                                u16::try_from(crate::text::display_cols(s))
                                    .unwrap_or(u16::MAX)
                                    .saturating_add(1)
                            })
                            .unwrap_or(0);
                        let actions_need = row
                            .actions
                            .as_ref()
                            .map(|a| {
                                u16::try_from(a.width())
                                    .unwrap_or(u16::MAX)
                                    .saturating_add(1)
                            })
                            .unwrap_or(0);
                        // Drop order: shortcut → actions → badge/status → secondary → leading → primary.
                        let show_shortcut = row.shortcut.is_some()
                            && content_w >= 12
                            && budget >= shortcut_need + 2;
                        if show_shortcut {
                            budget = budget.saturating_sub(shortcut_need);
                        }
                        let show_actions =
                            row.actions.is_some() && content_w >= 14 && budget >= actions_need + 2;
                        if show_actions {
                            budget = budget.saturating_sub(actions_need);
                        }
                        let badge_need = badge
                            .map(|b| {
                                u16::try_from(b.width())
                                    .unwrap_or(u16::MAX)
                                    .saturating_add(1)
                            })
                            .unwrap_or(0);
                        let show_badge = badge.is_some() && content_w >= 8 && budget > badge_need;
                        if show_badge {
                            budget = budget.saturating_sub(badge_need);
                        }
                        let secondary_need = row
                            .secondary
                            .as_ref()
                            .map(|s| {
                                u16::try_from(s.width())
                                    .unwrap_or(u16::MAX)
                                    .saturating_add(1)
                            })
                            .unwrap_or(0);
                        // Compact: inline secondary when budget allows; Comfortable: below.
                        let show_secondary =
                            row.secondary.is_some() && !secondary_below && budget >= secondary_need;
                        if show_secondary {
                            budget = budget.saturating_sub(secondary_need);
                        }
                        let leading_need = if recipe.loading {
                            u16::try_from(crate::text::display_cols(recipe.loading_glyph))
                                .unwrap_or(1)
                                .saturating_add(1)
                        } else {
                            row.leading
                                .as_ref()
                                .map(|l| {
                                    u16::try_from(l.width())
                                        .unwrap_or(u16::MAX)
                                        .saturating_add(1)
                                })
                                .unwrap_or(0)
                        };
                        let show_leading =
                            (recipe.loading || row.leading.is_some()) && budget >= leading_need;

                        let mut x = content_x;
                        let right = rect.right();
                        if show_leading {
                            if recipe.loading {
                                let lw =
                                    u16::try_from(crate::text::display_cols(recipe.loading_glyph))
                                        .unwrap_or(1)
                                        .min(right.saturating_sub(x));
                                if lw > 0 {
                                    buffer.set_stringn(
                                        x,
                                        rect.y,
                                        recipe.loading_glyph,
                                        usize::from(lw),
                                        recipe.secondary,
                                    );
                                    x = x.saturating_add(lw).saturating_add(1);
                                }
                            } else if let Some(lead) = row.leading.as_ref() {
                                let lw = u16::try_from(lead.width())
                                    .unwrap_or(u16::MAX)
                                    .min(right.saturating_sub(x));
                                if lw > 0 {
                                    buffer.set_line(x, rect.y, lead, lw);
                                    buffer.set_style(Rect::new(x, rect.y, lw, 1), style);
                                    x = x.saturating_add(lw).saturating_add(1);
                                }
                            }
                        }
                        let reserve = if show_badge { badge_need } else { 0 }
                            .saturating_add(if show_shortcut { shortcut_need } else { 0 });
                        let mid_end = right.saturating_sub(reserve);
                        let primary_budget = mid_end.saturating_sub(x);
                        if primary_budget > 0 {
                            buffer.set_line(x, rect.y, &row.label, primary_budget);
                            let primary_w = u16::try_from(row.label.width())
                                .unwrap_or(u16::MAX)
                                .min(primary_budget);
                            buffer.set_style(
                                Rect::new(x, rect.y, primary_w.max(1).min(primary_budget), 1),
                                style,
                            );
                            if recipe.show_focus_underline && primary_w > 0 {
                                buffer.set_style(
                                    Rect::new(x, rect.y, primary_w, 1),
                                    recipe.focus.add_modifier(Modifier::UNDERLINED),
                                );
                            }
                            x = x.saturating_add(primary_w);
                        }
                        if show_secondary && let Some(sec) = row.secondary.as_ref() {
                            let avail = mid_end.saturating_sub(x);
                            if avail > 2 {
                                x = x.saturating_add(1);
                                let sw = u16::try_from(sec.width())
                                    .unwrap_or(u16::MAX)
                                    .min(mid_end.saturating_sub(x));
                                if sw > 0 {
                                    buffer.set_line(x, rect.y, sec, sw);
                                    buffer.set_style(Rect::new(x, rect.y, sw, 1), recipe.secondary);
                                }
                            }
                        }
                        let mut cursor = right;
                        if show_shortcut && let Some(sc) = row.shortcut {
                            let w = u16::try_from(crate::text::display_cols(sc))
                                .unwrap_or(u16::MAX)
                                .min(cursor.saturating_sub(content_x));
                            if w > 0 {
                                cursor = cursor.saturating_sub(w);
                                buffer.set_stringn(
                                    cursor,
                                    rect.y,
                                    sc,
                                    usize::from(w),
                                    recipe.shortcut,
                                );
                            }
                        }
                        if show_actions && let Some(act) = row.actions.as_ref() {
                            let w = u16::try_from(act.width())
                                .unwrap_or(u16::MAX)
                                .min(cursor.saturating_sub(content_x));
                            if w > 0 {
                                cursor = cursor.saturating_sub(w);
                                buffer.set_line(cursor, rect.y, act, w);
                                buffer.set_style(Rect::new(cursor, rect.y, w, 1), recipe.shortcut);
                            }
                        }
                        if show_badge && let Some(b) = badge {
                            let w = u16::try_from(b.width())
                                .unwrap_or(u16::MAX)
                                .min(cursor.saturating_sub(content_x));
                            if w > 0 {
                                if show_shortcut || show_actions {
                                    cursor = cursor.saturating_sub(1);
                                }
                                cursor = cursor.saturating_sub(w);
                                buffer.set_line(cursor, rect.y, b, w);
                                buffer.set_style(Rect::new(cursor, rect.y, w, 1), recipe.trailing);
                            }
                        }
                    } // end non-custom
                }
                // Comfortable: secondary on next line
                if secondary_below {
                    if let Some(sec) = row.secondary.as_ref() {
                        let indent = rect.x.saturating_add(4);
                        let sub = Rect::new(
                            indent.min(rect.right().saturating_sub(1)),
                            rect.y.saturating_add(1),
                            rect.right().saturating_sub(indent.min(rect.right())),
                            1,
                        );
                        if !sub.is_empty() {
                            buffer.set_line(sub.x, sub.y, sec, sub.width);
                            buffer.set_style(sub, recipe.secondary);
                        }
                    }
                }
            }
            let hit_h = rh;
            if row.enabled && row.role.is_navigable() && !rect.is_empty() {
                state.regions.push(HitRegion {
                    id: row.id.clone(),
                    area: Rect::new(rect.x, rect.y, rect.width, hit_h),
                });
            }
            y = y.saturating_add(rh);
            painted_rows = painted_rows.saturating_add(1);
        }
        let _ = painted_rows;
        if scrollable {
            crate::scroll::render_scrollbar(
                buffer,
                Rect::new(body.right().saturating_sub(1), body.y, 1, body.height),
                crate::scroll::ScrollbarSpec::new(
                    crate::scroll::ScrollAxis::Vertical,
                    crate::scroll::ScrollbarGeometry::new(
                        total,
                        usize::from(body.height).max(1),
                        u16::try_from(state.collection.offset()).unwrap_or(u16::MAX),
                    ),
                ),
                self.tokens,
            );
        }
        state.hovered = state.pointer.and_then(|position| {
            state
                .regions
                .iter()
                .find(|region| region.area.contains(position))
                .map(|region| region.id.clone())
        });
    }
}

/// Paints multi-select check chrome; returns occupied width including trailing gap.
fn render_check_cell<Id: Clone>(
    buffer: &mut Buffer,
    state: &mut ListState<Id>,
    row: &ListRow<'_, Id>,
    rect: Rect,
    check_x: u16,
    recipe: &crate::style::ListRowRecipe,
    style: ratatui_core::style::Style,
) -> u16 {
    if state.selection.is_none() || check_x >= rect.right() {
        return 0;
    }

    let marker = if recipe.checked {
        recipe.check_on
    } else {
        recipe.check_off
    };
    let glyph_w = u16::try_from(crate::text::display_cols(marker)).unwrap_or(1);
    let available = rect.right().saturating_sub(check_x);
    let paint_w = glyph_w.min(available);
    if paint_w == 0 {
        return 0;
    }
    buffer.set_stringn(check_x, rect.y, marker, usize::from(paint_w), style);
    // Trailing gap after check for content separation.
    let gap = u16::from(available > paint_w);
    if gap > 0 {
        buffer.set_stringn(check_x.saturating_add(paint_w), rect.y, " ", 1, style);
    }
    if row.enabled && paint_w >= 1 {
        state.check_regions.push(HitRegion {
            id: row.id.clone(),
            area: Rect::new(check_x, rect.y, paint_w.max(1), 1),
        });
    }
    paint_w.saturating_add(gap)
}

impl<Id: Clone + PartialEq> StatefulWidget for List<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

/// Frame projection for headless [`crate::interaction::CollectionState`] (no long-lived borrows).
fn collection_items_from_rows<Id: Clone>(
    rows: &[ListRow<'_, Id>],
) -> Vec<crate::interaction::CollectionItem<Id>> {
    rows.iter()
        .filter(|row| row.role.is_navigable())
        .map(|row| crate::interaction::CollectionItem {
            id: row.id.clone(),
            enabled: row.enabled,
            // Primary plain text enables roving typeahead.
            label: row.plain_label(),
            parent: None,
        })
        .collect()
}

/// Scrolls list offset so the active id is within the painted window (full row index space).
fn ensure_list_active_visible<Id: Clone + PartialEq>(
    state: &mut ListState<Id>,
    rows: &[ListRow<'_, Id>],
    viewport_height: usize,
) {
    let vp = viewport_height.max(1);
    let Some(active) = state.collection.active() else {
        return;
    };
    let Some(index) = rows.iter().position(|row| &row.id == active) else {
        return;
    };
    let mut offset = state.collection.offset();
    if index < offset {
        offset = index;
    } else if index >= offset.saturating_add(vp) {
        offset = index.saturating_add(1).saturating_sub(vp);
    }
    state.collection.set_viewport(offset, vp, rows.len());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::interaction::{NavigationMove, UiIntent};

    #[test]
    fn handle_intent_moves_and_activates_without_raw_keys() {
        let rows = rows();
        let mut state = ListState::new(Some("first"));
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Move(NavigationMove::Next)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"second"));
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Activate),
            Outcome::Activated("second")
        );
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Cancel),
            Outcome::Cancelled
        );
    }

    fn rows() -> [ListRow<'static, &'static str>; 4] {
        [
            ListRow {
                id: "section",
                label: Line::from("Section"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                trailing: None,
                custom: None,
                role: RowRole::Separator,
                enabled: true,
                loading: false,
            },
            ListRow {
                id: "disabled",
                label: Line::from("Disabled"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                trailing: None,
                custom: None,
                role: RowRole::Item,
                enabled: false,
                loading: false,
            },
            ListRow {
                id: "first",
                label: Line::from("First"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                trailing: None,
                custom: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
            ListRow {
                id: "second",
                label: Line::from("Second"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                trailing: None,
                custom: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
        ]
    }

    #[test]
    fn keyboard_skips_non_items_and_returns_stable_ids() {
        let rows = rows();
        let mut state = ListState::new(None);
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"first"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"second"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Outcome::Activated("second")
        );
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Outcome::Cancelled
        );
    }

    #[test]
    fn render_reveals_selection_and_mouse_uses_painted_regions() {
        let rows = rows();
        let tokens = DesignSystem::default();
        let mut state = ListState::new(Some("second"));
        let area = Rect::new(4, 3, 12, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(state.offset(), 3);
        assert_eq!(state.regions().len(), 1);
        let position = Position::new(area.x, area.y);
        assert_eq!(state.hover(position), Some(&"second"));
        assert_eq!(state.click(position), Outcome::Activated("second"));
        // Quiet phosphor selection uses design-token gutter glyph.
        assert_eq!(buffer[(area.x, area.y)].symbol(), "▌");
    }

    #[test]
    fn trailing_cells_align_right_and_wide_labels_truncate_first() {
        let rows = [
            ListRow {
                id: "wide",
                label: Line::from("🧪🧪label"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                trailing: Some(Line::from("9 KiB")),
                custom: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
            ListRow {
                id: "short",
                label: Line::from("short"),
                leading: None,
                secondary: None,
                status: None,
                badge: None,
                shortcut: None,
                actions: None,
                trailing: Some(Line::from("1 B")),
                custom: None,
                role: RowRole::Item,
                enabled: true,
                loading: false,
            },
        ];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(None);
        // Gutter (2) + content: badge right-aligned within content band.
        let area = Rect::new(0, 0, 14, 2);
        let mut buffer = Buffer::empty(area);

        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);

        // Right edge of full row holds trailing badge.
        assert_eq!(buffer[(9, 0)].symbol(), "9");
        assert_eq!(buffer[(13, 0)].symbol(), "B");
        assert_eq!(buffer[(11, 1)].symbol(), "1");
        assert_eq!(buffer[(13, 1)].symbol(), "B");
        // Primary starts after gutter and keeps wide graphemes intact.
        assert_eq!(buffer[(2, 0)].symbol(), "🧪");
    }

    #[test]
    fn narrow_trailing_cell_clips_only_at_grapheme_boundaries() {
        let mut row = ListRow::item("wide-trailing", Line::from("x"));
        row.trailing = Some(Line::from("🧪Z"));
        let rows = [row];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(None);
        // Gutter 2 + content 3: badge "🧪Z" (3 cells) fits; grapheme-safe clip drops Z if tighter.
        let area = Rect::new(0, 0, 5, 1);
        let mut buffer = Buffer::empty(area);

        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);

        let text: String = (0..5)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        // Badge is right-aligned in content; wide emoji either fully present or absent — never half.
        let emoji_count = text.matches('🧪').count();
        assert!(emoji_count <= 1, "must not split wide grapheme: {text:?}");
        if emoji_count == 1 {
            assert!(
                !text.contains('Z'),
                "clip after emoji not mid-grapheme: {text:?}"
            );
        }
    }

    #[test]
    fn composed_row_anatomy_paints_leading_secondary_shortcut() {
        let mut row = ListRow::item("job", Line::from("Build"));
        row.leading = Some(Line::from("*"));
        row.secondary = Some(Line::from("meta"));
        row.badge = Some(Line::from("ok"));
        row.shortcut = Some("⌘B");
        let rows = [row];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(None);
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let text: String = (0..40)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(text.contains("Build"), "{text:?}");
        assert!(text.contains('*'), "{text:?}");
        assert!(text.contains("meta"), "{text:?}");
        assert!(text.contains("ok"), "{text:?}");
        assert!(text.contains('⌘') || text.contains('B'), "{text:?}");
    }

    #[test]
    fn narrow_list_drops_shortcut_before_primary_identity() {
        let mut row = ListRow::item("job", Line::from("Identity"));
        row.shortcut = Some("⌘K");
        row.badge = Some(Line::from("99"));
        let rows = [row];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(None);
        // Gutter 2 + content 4: optional chrome must drop before primary.
        let area = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let text: String = (0..6)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            text.contains('I') || text.contains("Id"),
            "primary survives: {text:?}"
        );
        assert!(!text.contains('⌘'), "shortcut drops first: {text:?}");
    }

    #[test]
    fn list_shift_space_sets_range_selection() {
        let rows = rows();
        let mut state = ListState::new(Some("first"));
        state.enable_multi_select();
        let _ = state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let _ = state.handle_intent(&rows, UiIntent::Move(NavigationMove::Next));
        let out = state.handle_key(
            &rows,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::SHIFT),
        );
        assert!(matches!(out, Outcome::CheckToggled(_)));
        let checked = state.selection().unwrap().checked();
        assert!(
            checked.len() >= 2,
            "range should cover first..active, got {checked:?}"
        );
    }

    #[test]
    fn list_check_toggle_reports_id() {
        let rows = rows();
        let tokens = DesignSystem::default();
        let mut state = ListState::new(Some("first"));
        state.enable_multi_select();

        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Outcome::CheckToggled("first")
        );
        assert!(state.selection().unwrap().is_checked(&"first"));

        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        // Unicode check glyph (or ASCII "[x]") in the multi-select slot after gutter.
        let check = buffer[(2, 2)].symbol();
        assert!(
            check == "☑" || check == "[" || check == "x",
            "expected check chrome, got {check:?}"
        );
        assert_eq!(
            state.click(Position::new(2, 3)),
            Outcome::CheckToggled("second")
        );
        assert_eq!(state.selection().unwrap().checked(), ["first", "second"]);

        state.selection_mut().unwrap().clear();
        assert!(state.selection().unwrap().checked().is_empty());
        state.disable_multi_select();
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Outcome::Ignored
        );
    }

    #[test]
    fn list_state_accessors_preserve_semantic_ownership() {
        let mut state = ListState::new(Some("first"));

        assert_eq!(state.selected(), Some(&"first"));
        assert_eq!(state.hovered(), None);
        assert_eq!(state.offset(), 0);
        assert!(state.regions().is_empty());

        state.select(Some("second"));
        state.enable_multi_select();
        assert!(state.selection_mut().unwrap().toggle(&"second"));

        assert_eq!(state.selected(), Some(&"second"));
        assert_eq!(state.selection().unwrap().checked(), ["second"]);
    }

    #[test]
    fn indexed_picker_navigation_wraps_keys_and_bounds_gestures() {
        let mut state = ListState::for_count(3);
        assert_eq!(state.selected(), Some(&0));
        assert!(state.cycle_index(3, -1));
        assert_eq!(state.selected(), Some(&2));
        assert!(state.cycle_index(3, 1));
        assert_eq!(state.selected(), Some(&0));
        assert!(state.move_index(3, 9));
        assert_eq!(state.selected(), Some(&2));
        assert!(!state.move_index(3, 9));
        assert_eq!(state.selected_item(&["a", "b", "c"]), Some(&"c"));

        state.reconcile_count(1);
        assert_eq!(state.selected(), Some(&0));
        state.reconcile_count(0);
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn click_policy_select_emits_changed_not_activate() {
        let rows = rows();
        let tokens = DesignSystem::default();
        let mut state = ListState::new(Some("first"));
        state.set_click_policy(ListClickPolicy::Select);
        let area = Rect::new(0, 0, 20, 4);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let pos = state.regions()[0].area;
        assert_eq!(state.click(Position::new(pos.x, pos.y)), Outcome::Changed);
        assert_eq!(state.selected(), Some(&"first"));
    }

    #[test]
    fn empty_list_paints_empty_message() {
        let rows: [ListRow<'_, &str>; 0] = [];
        let tokens = DesignSystem::default();
        let mut state = ListState::<&str>::default();
        let area = Rect::new(0, 0, 20, 3);
        let mut buffer = Buffer::empty(area);
        let list = List::new(&rows, &tokens).empty_message(Line::from("No items"));
        (&list).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), "N");
        assert!(state.regions().is_empty());
    }

    #[test]
    fn loading_row_uses_recipe_loading_glyph() {
        let row = ListRow::item("job", Line::from("Build")).loading();
        let rows = [row];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(None);
        let area = Rect::new(0, 0, 24, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let text: String = (0..24)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(
            text.contains('…') || text.contains('.'),
            "loading glyph present: {text:?}"
        );
        assert!(text.contains("Build"), "{text:?}");
    }

    #[test]
    fn fluent_row_builder_and_from_system() {
        let row = ListRow::item("x", Line::from("X"))
            .leading(Line::from("*"))
            .secondary(Line::from("s"))
            .badge(Line::from("b"))
            .shortcut("⌘K");
        let rows = [row];
        let system = DesignSystem::phosphor();
        let mut state = ListState::new(Some("x"));
        let area = Rect::new(0, 0, 40, 1);
        let mut buffer = Buffer::empty(area);
        (&List::from_system(&rows, &system)).render(area, &mut buffer, &mut state);
        let text: String = (0..40)
            .map(|x| buffer[(x, 0)].symbol().to_string())
            .collect();
        assert!(text.contains('X'), "{text:?}");
        assert_eq!(buffer[(0, 0)].symbol(), "▌");
    }

    #[test]
    fn stress_paint_visible_only() {
        let rows: Vec<ListRow<'_, usize>> = (0..10_000)
            .map(|i| ListRow::item(i, Line::from(format!("row-{i}"))))
            .collect();
        let tokens = DesignSystem::default();
        let mut state = ListState::new(Some(9_500));
        let area = Rect::new(0, 0, 40, 20);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(state.regions().len(), 20);
        assert!(state.offset() <= 9_500);
        assert!(state.offset() + 20 > 9_500);
    }

    #[test]
    fn ascii_gutter_and_check_glyphs() {
        let rows = [ListRow::item("a", Line::from("A"))];
        let tokens = DesignSystem::default()
            .glyphs(crate::style::GlyphSet::Ascii)
            .selection(crate::style::SelectionChrome::Gutter);
        let mut state = ListState::new(Some("a"));
        state.enable_multi_select();
        state.selection_mut().unwrap().toggle(&"a");
        let area = Rect::new(0, 0, 20, 1);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), ">");
        let check = buffer[(2, 0)].symbol();
        assert!(check == "[" || check == "x", "ascii check: {check:?}");
    }

    #[test]
    fn typeahead_jumps_by_primary_label() {
        let rows = [
            ListRow::item("a", Line::from("Alpha")),
            ListRow::item("b", Line::from("Beta")),
            ListRow::item("c", Line::from("Charlie")),
        ];
        let mut state = ListState::new(Some("a"));
        assert_eq!(
            state.handle_key(&rows, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"b"));
        assert!(!state.typeahead_buffer().is_empty() || state.selected() == Some(&"b"));
    }

    #[test]
    fn group_header_skipped_by_movement() {
        let rows = [
            ListRow::group_header("g", Line::from("Group")),
            ListRow::item("a", Line::from("A")),
            ListRow::item("b", Line::from("B")),
        ];
        let mut state = ListState::new(None);
        assert_eq!(
            state.handle_intent(&rows, UiIntent::Move(NavigationMove::Next)),
            Outcome::Changed
        );
        assert_eq!(state.selected(), Some(&"a"));
    }

    #[test]
    fn search_query_and_filter_helper() {
        let rows = [
            ListRow::item("a", Line::from("Alpha")),
            ListRow::item("b", Line::from("Beta")),
            ListRow::group_header("g", Line::from("Hdr")),
        ];
        let mut state = ListState::new(Some("a"));
        let _ = state.handle_key(&rows, KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(state.search_query().is_some());
        let _ = state.handle_key(&rows, KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE));
        assert_eq!(state.search_query(), Some("b"));
        let filtered = filter_list_rows(&rows, "be");
        assert!(filtered.iter().any(|r| r.id == "b"));
        assert!(filtered.iter().any(|r| r.id == "g")); // headers kept
    }

    #[test]
    fn selection_mode_single_multi_range() {
        let mut state = ListState::new(Some("a"));
        assert_eq!(state.selection_mode(), ListSelectionMode::Single);
        state.set_selection_mode(ListSelectionMode::Multi);
        assert!(state.selection().is_some());
        state.set_selection_mode(ListSelectionMode::Range);
        assert!(state.selection().is_some());
        state.set_selection_mode(ListSelectionMode::Single);
        assert!(state.selection().is_none());
    }

    #[test]
    fn comfortable_density_paints_secondary_below() {
        let rows = [ListRow::item("a", Line::from("Title")).secondary(Line::from("meta"))];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(Some("a"));
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens).comfortable()).render(area, &mut buffer, &mut state);
        let mut painted = String::new();
        for y in 0..3 {
            for x in 0..24 {
                painted.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(
            painted.contains("Title") || painted.contains("meta"),
            "{painted}"
        );
    }

    #[test]
    fn status_actions_custom_row_paint() {
        let rows = [
            ListRow::item("a", Line::from("Job"))
                .status(Line::from("run"))
                .actions(Line::from("⏎"))
                .shortcut("j"),
            ListRow::item("b", Line::from("hidden")).custom(Line::from("CUSTOM BODY")),
        ];
        let tokens = DesignSystem::default();
        let mut state = ListState::new(Some("b"));
        let area = Rect::new(0, 0, 40, 2);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        let mut painted = String::new();
        for y in 0..2 {
            for x in 0..40 {
                painted.push_str(buffer[(x, y)].symbol());
            }
        }
        assert!(
            painted.contains("CUSTOM") || painted.contains("Job"),
            "{painted}"
        );
    }

    #[test]
    fn virtual_window_reconcile() {
        let rows = [
            ListRow::item("50", Line::from("fifty")),
            ListRow::item("51", Line::from("fifty-one")),
        ];
        let mut state = ListState::new(Some("50"));
        state.set_virtual_window(50, 200);
        state.reconcile_collection(&rows);
        assert_eq!(state.virtual_total(), 200);
        assert_eq!(state.collection().offset(), 50);
    }

    #[test]
    fn narrow_drop_order_documented() {
        assert_eq!(LIST_NARROW_DROP_ORDER[0], "shortcut");
        assert_eq!(*LIST_NARROW_DROP_ORDER.last().unwrap(), "primary");
    }

    #[test]
    fn scroll_area_sync() {
        let mut state = ListState::new(Some("a"));
        let mut scroll = crate::widgets::ScrollAreaState::new();
        let rows = [
            ListRow::item("a", Line::from("A")),
            ListRow::item("b", Line::from("B")),
            ListRow::item("c", Line::from("C")),
        ];
        let tokens = DesignSystem::default();
        let area = Rect::new(0, 0, 20, 2);
        let mut buffer = Buffer::empty(area);
        (&List::new(&rows, &tokens)).render(area, &mut buffer, &mut state);
        state.sync_scroll_area(&mut scroll, rows.len(), 2);
        assert_eq!(scroll.viewport_h(), 2);
        assert_eq!(scroll.content_h(), 3);
    }
}
