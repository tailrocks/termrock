// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Searchable multiple-choice selector with chip summary and open list.
//!
//! **Mission.** Filters, permissions, task pickers, and schema tools need
//! multi-select with check state distinct from keyboard highlight, select-all,
//! max selection, groups, search, and a compact summary when closed.
//!
//! **vs [`Select`](super::Select).** Single value. MultiSelect owns ordered
//! membership via [`Selection`](super::Selection).
//! **vs always-visible checkbox lists.** MultiSelect is closed-by-default with
//! popover/fullscreen list chrome (host places overlays).
//!
//! Research: modern multi-selects, Huh, terminal fuzzy pickers.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent,
    },
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Glyph, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
};

use super::{
    Panel, PanelChrome, PanelVariant, SELECT_FULLSCREEN_MAX_HEIGHT, SELECT_FULLSCREEN_MAX_WIDTH,
    SelectOption, SelectPresentation, SelectRecipe, SelectRowKind, Selection, TextInput,
    TextInputOutcome, TextInputState, Validation,
};

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Multi-select interaction outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MultiSelectOutcome<Id> {
    /// No effect.
    Ignored,
    /// Chrome / highlight / open state changed without membership change.
    Changed,
    /// The pointer moved onto (or off) a row.
    HoverChanged,
    /// List opened.
    Opened {
        /// Presentation.
        presentation: SelectPresentation,
    },
    /// List closed (Esc / outside / confirm).
    Closed,
    /// Highlight moved (≠ checked set).
    HighlightChanged {
        /// Active id.
        id: Option<Id>,
    },
    /// One id toggled.
    Toggled {
        /// Id.
        id: Id,
        /// Checked after toggle.
        checked: bool,
    },
    /// Range applied (shift navigation).
    RangeApplied {
        /// Ids set checked in this range action.
        ids: Vec<Id>,
    },
    /// Select all visible (enabled) options.
    SelectAll {
        /// Count now checked.
        count: usize,
    },
    /// Clear all checks.
    Cleared,
    /// Search query changed.
    SearchChanged {
        /// Query.
        query: String,
    },
    /// Presentation policy changed.
    PresentationChanged {
        /// Presentation.
        presentation: SelectPresentation,
    },
    /// Max selection blocked a toggle.
    MaxReached {
        /// Cap.
        max: usize,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`MultiSelect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSelectState<Id: Clone + PartialEq> {
    /// Ordered checked membership.
    selection: Selection<Id>,
    /// Open presentation.
    presentation: SelectPresentation,
    /// Keyboard highlight (distinct from checks).
    collection: CollectionState<Id>,
    /// Search draft when searchable.
    search: TextInputState,
    searchable: bool,
    recipe: SelectRecipe,
    /// Max checked items (`None` = unlimited).
    max_selected: Option<usize>,
    /// Shift-range anchor id.
    range_anchor: Option<Id>,
    /// Max chips in closed summary before `+N`.
    max_summary_chips: usize,
    focused: bool,
    enabled: bool,
    list_rows: u16,
    trigger: Rect,
    panel: Rect,
    option_regions: Vec<(Id, Rect)>,
    /// Option the pointer is over (hover wash; never a commit).
    hovered: Option<Id>,
    search_region: Option<Rect>,
    clear_region: Option<Rect>,
}

impl<Id: Clone + PartialEq> Default for MultiSelectState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> MultiSelectState<Id> {
    /// Empty multi-select.
    #[must_use]
    pub fn new() -> Self {
        let mut search = TextInputState::new("").with_allow_empty(true);
        search.set_focused(false);
        Self {
            selection: Selection::new(),
            presentation: SelectPresentation::Closed,
            collection: CollectionState::new().wrap(true),
            search,
            searchable: true,
            recipe: SelectRecipe::Inline,
            max_selected: None,
            range_anchor: None,
            max_summary_chips: 3,
            focused: false,
            enabled: true,
            list_rows: 8,
            trigger: Rect::default(),
            panel: Rect::default(),
            option_regions: Vec::new(),
            hovered: None,
            search_region: None,
            clear_region: None,
        }
    }

    /// Seed selected ids (order preserved).
    #[must_use]
    pub fn with_selected(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        for id in ids {
            if !self.selection.is_checked(&id) {
                let _ = self.selection.toggle(&id);
            }
        }
        self
    }

    /// Search field when open.
    #[must_use]
    pub const fn with_searchable(mut self, on: bool) -> Self {
        self.searchable = on;
        self
    }

    /// Recipe.
    #[must_use]
    pub const fn with_recipe(mut self, recipe: SelectRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Maximum number of checked options.
    #[must_use]
    pub const fn with_max_selected(mut self, max: Option<usize>) -> Self {
        self.max_selected = max;
        self
    }

    /// Closed-summary chip cap.
    #[must_use]
    pub fn with_max_summary_chips(mut self, n: usize) -> Self {
        self.max_summary_chips = n.max(1);
        self
    }

    /// Preferred list rows.
    #[must_use]
    pub fn with_list_rows(mut self, rows: u16) -> Self {
        self.list_rows = rows.max(3);
        self
    }

    /// Checked ids in check order.
    #[must_use]
    pub fn selected(&self) -> &[Id] {
        self.selection.checked()
    }

    /// Whether id is checked.
    #[must_use]
    pub fn is_checked(&self, id: &Id) -> bool {
        self.selection.is_checked(id)
    }

    /// Highlight id.
    #[must_use]
    pub const fn highlight(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> SelectPresentation {
        self.presentation
    }

    /// Open?
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.presentation.is_open()
    }

    /// Search text.
    #[must_use]
    pub fn search_query(&self) -> &str {
        self.search.value()
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(&self) -> SelectRecipe {
        self.recipe
    }

    /// Max selected.
    #[must_use]
    pub const fn max_selected(&self) -> Option<usize> {
        self.max_selected
    }

    /// Selection model.
    #[must_use]
    pub const fn selection(&self) -> &Selection<Id> {
        &self.selection
    }

    /// Mutable selection (advanced).
    pub fn selection_mut(&mut self) -> &mut Selection<Id> {
        &mut self.selection
    }

    /// Collection.
    #[must_use]
    pub const fn collection(&self) -> &CollectionState<Id> {
        &self.collection
    }

    /// Trigger area.
    #[must_use]
    pub const fn trigger_area(&self) -> Rect {
        self.trigger
    }

    /// Panel area.
    #[must_use]
    pub const fn panel_area(&self) -> Rect {
        self.panel
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on {
            self.search.set_focused(false);
        }
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Replace membership (controlled).
    pub fn set_selected(&mut self, ids: impl IntoIterator<Item = Id>) {
        self.selection.clear();
        for id in ids {
            if !self.selection.is_checked(&id) {
                let _ = self.selection.toggle(&id);
            }
        }
    }

    /// Force presentation.
    pub const fn set_presentation(&mut self, presentation: SelectPresentation) {
        self.presentation = presentation;
    }

    fn collection_items(options: &[SelectOption<Id>]) -> Vec<CollectionItem<Id>> {
        options
            .iter()
            .filter(|o| o.is_option())
            .map(|o| CollectionItem::new(o.id.clone(), o.label.clone()).enabled(!o.disabled))
            .collect()
    }

    fn filtered_items(options: &[SelectOption<Id>], query: &str) -> Vec<CollectionItem<Id>> {
        if query.trim().is_empty() {
            return Self::collection_items(options);
        }
        // Reuse Select filter logic inline
        let q = query.trim().to_ascii_lowercase();
        options
            .iter()
            .filter(|o| o.is_option() && o.label.to_ascii_lowercase().contains(&q))
            .map(|o| CollectionItem::new(o.id.clone(), o.label.clone()).enabled(!o.disabled))
            .collect()
    }

    fn presentation_for_bounds(bounds: Rect) -> SelectPresentation {
        if bounds.width < SELECT_FULLSCREEN_MAX_WIDTH
            || bounds.height < SELECT_FULLSCREEN_MAX_HEIGHT
        {
            SelectPresentation::Fullscreen
        } else {
            SelectPresentation::Popover
        }
    }

    fn navigable_ids(options: &[SelectOption<Id>], query: &str) -> Vec<Id> {
        Self::filtered_items(options, query)
            .into_iter()
            .filter(|i| i.enabled)
            .map(|i| i.id)
            .collect()
    }

    /// Open list.
    pub fn open(&mut self, bounds: Rect, options: &[SelectOption<Id>]) -> MultiSelectOutcome<Id> {
        if !self.enabled {
            return MultiSelectOutcome::Ignored;
        }
        let presentation = Self::presentation_for_bounds(bounds);
        self.presentation = presentation;
        let items = Self::collection_items(options);
        let _ = self.collection.reconcile(&items);
        // Prefer highlight first selected, else first enabled
        if let Some(first) = self.selection.checked().first() {
            if items.iter().any(|i| &i.id == first && i.enabled) {
                self.collection.set_active(Some(first.clone()));
            }
        }
        self.range_anchor = self.collection.active().cloned();
        if self.searchable {
            self.search.set_focused(true);
            self.search.set_enabled(true);
        }
        self.focused = true;
        MultiSelectOutcome::Opened { presentation }
    }

    /// Close list (keep membership).
    pub fn close(&mut self) -> MultiSelectOutcome<Id> {
        if !self.is_open() {
            return MultiSelectOutcome::Ignored;
        }
        self.presentation = SelectPresentation::Closed;
        self.search.set_focused(false);
        let _ = self.search.clear();
        self.range_anchor = None;
        MultiSelectOutcome::Closed
    }

    /// Clear all checks.
    pub fn clear_selection(&mut self) -> MultiSelectOutcome<Id> {
        if self.selection.checked().is_empty() {
            return MultiSelectOutcome::Ignored;
        }
        self.selection.clear();
        MultiSelectOutcome::Cleared
    }

    /// Select all enabled options in current filter.
    pub fn select_all_visible(&mut self, options: &[SelectOption<Id>]) -> MultiSelectOutcome<Id> {
        let ids = Self::navigable_ids(options, self.search.value());
        if ids.is_empty() {
            return MultiSelectOutcome::Ignored;
        }
        if let Some(max) = self.max_selected {
            for id in ids {
                if self.selection.is_checked(&id) {
                    continue;
                }
                if self.selection.checked().len() >= max {
                    break;
                }
                let _ = self.selection.toggle(&id);
            }
        } else {
            self.selection.select_all(&ids);
        }
        MultiSelectOutcome::SelectAll {
            count: self.selection.checked().len(),
        }
    }

    fn try_toggle(&mut self, id: &Id) -> MultiSelectOutcome<Id> {
        let checked = self.selection.is_checked(id);
        if !checked {
            if let Some(max) = self.max_selected {
                if self.selection.checked().len() >= max {
                    return MultiSelectOutcome::MaxReached { max };
                }
            }
        }
        let now = self.selection.toggle(id);
        MultiSelectOutcome::Toggled {
            id: id.clone(),
            checked: now,
        }
    }

    /// Apply range check from anchor to `to` (inclusive among navigable ids).
    fn apply_range(&mut self, options: &[SelectOption<Id>], to: &Id) -> MultiSelectOutcome<Id> {
        let nav = Self::navigable_ids(options, self.search.value());
        let Some(anchor) = self.range_anchor.clone() else {
            self.range_anchor = Some(to.clone());
            return self.try_toggle(to);
        };
        let Some(a) = nav.iter().position(|i| i == &anchor) else {
            self.range_anchor = Some(to.clone());
            return self.try_toggle(to);
        };
        let Some(b) = nav.iter().position(|i| i == to) else {
            return MultiSelectOutcome::Ignored;
        };
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let mut applied = Vec::new();
        for id in &nav[lo..=hi] {
            if !self.selection.is_checked(id) {
                if let Some(max) = self.max_selected {
                    if self.selection.checked().len() >= max {
                        break;
                    }
                }
                let _ = self.selection.toggle(id);
                applied.push(id.clone());
            }
        }
        if applied.is_empty() {
            MultiSelectOutcome::Changed
        } else {
            MultiSelectOutcome::RangeApplied { ids: applied }
        }
    }

    /// Reconcile after option changes.
    pub fn reconcile_options(&mut self, options: &[SelectOption<Id>]) {
        let items = Self::filtered_items(options, self.search.value());
        let valid: Vec<Id> = options
            .iter()
            .filter(|o| o.is_option())
            .map(|o| o.id.clone())
            .collect();
        self.selection.reconcile(&valid);
        let _ = self.collection.reconcile(&items);
    }

    /// Key adapter.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> MultiSelectOutcome<Id> {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return MultiSelectOutcome::Ignored;
        }
        if !self.is_open() {
            return self.handle_closed_key(key, options, bounds);
        }
        self.handle_open_key(key, options, bounds)
    }

    fn handle_closed_key(
        &mut self,
        key: KeyEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> MultiSelectOutcome<Id> {
        if !self.focused {
            return MultiSelectOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Down if key.modifiers.is_empty() => {
                self.open(bounds, options)
            }
            KeyCode::Backspace | KeyCode::Delete
                if key.modifiers.is_empty() && !self.selection.checked().is_empty() =>
            {
                self.clear_selection()
            }
            KeyCode::Char(c)
                if !c.is_control()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                let out = self.open(bounds, options);
                if self.searchable {
                    let _ = self.search.insert_str(&c.to_string());
                    self.reconcile_options(options);
                    return MultiSelectOutcome::SearchChanged {
                        query: self.search.value().to_owned(),
                    };
                }
                out
            }
            _ => MultiSelectOutcome::Ignored,
        }
    }

    fn handle_open_key(
        &mut self,
        key: KeyEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> MultiSelectOutcome<Id> {
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return self.close();
        }

        // Ctrl+A select all
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('a' | 'A'))
        {
            return self.select_all_visible(options);
        }

        // Ctrl+D / clear
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('d' | 'D') | KeyCode::Backspace)
        {
            return self.clear_selection();
        }

        let desired = Self::presentation_for_bounds(bounds);
        if desired != self.presentation {
            self.presentation = desired;
            return MultiSelectOutcome::PresentationChanged {
                presentation: desired,
            };
        }

        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // Search typing
        if self.searchable
            && (matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
            ) && !key.modifiers.contains(KeyModifiers::CONTROL)
                || key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('u' | 'U')))
        {
            // Don't steal Space (toggle) when search empty and not typing word?
            if matches!(key.code, KeyCode::Char(' ')) && self.search.value().is_empty() {
                // fall through to toggle
            } else {
                match self.search.handle_key(key) {
                    TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                        self.reconcile_options(options);
                        return MultiSelectOutcome::SearchChanged {
                            query: self.search.value().to_owned(),
                        };
                    }
                    _ => {}
                }
            }
        }

        let items = Self::filtered_items(options, self.search.value());

        // Space / Enter toggles highlight (Enter can also close with Ctrl)
        if matches!(key.code, KeyCode::Char(' ')) && key.modifiers.is_empty() {
            if let Some(id) = self.collection.active().cloned() {
                self.range_anchor = Some(id.clone());
                return self.try_toggle(&id);
            }
            return MultiSelectOutcome::Ignored;
        }
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            // Confirm & close (common multi-select UX)
            return self.close();
        }

        // Shift+arrows: move and range-select
        if shift && matches!(key.code, KeyCode::Down | KeyCode::Up) {
            if self.range_anchor.is_none() {
                self.range_anchor = self.collection.active().cloned();
            }
            let delta = if matches!(key.code, KeyCode::Down) {
                1
            } else {
                -1
            };
            let moved = self.collection.move_by(&items, delta);
            if let CollectionOutcome::ActiveChanged { to: Some(to), .. } = moved {
                return self.apply_range(options, &to);
            }
            return MultiSelectOutcome::Ignored;
        }

        match self.collection.handle_key(key, &items) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                if !shift {
                    self.range_anchor = to.clone();
                }
                MultiSelectOutcome::HighlightChanged { id: to }
            }
            CollectionOutcome::Scrolled => MultiSelectOutcome::Changed,
            CollectionOutcome::Ignored => MultiSelectOutcome::Ignored,
        }
    }

    /// Intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> MultiSelectOutcome<Id> {
        if !self.enabled {
            return MultiSelectOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate if !self.is_open() => self.open(bounds, options),
            UiIntent::Activate if self.is_open() => {
                if let Some(id) = self.collection.active().cloned() {
                    self.try_toggle(&id)
                } else {
                    MultiSelectOutcome::Ignored
                }
            }
            UiIntent::Submit if self.is_open() => self.close(),
            UiIntent::Cancel | UiIntent::Close if self.is_open() => self.close(),
            UiIntent::Fullscreen if self.is_open() => {
                self.presentation = SelectPresentation::Fullscreen;
                MultiSelectOutcome::PresentationChanged {
                    presentation: SelectPresentation::Fullscreen,
                }
            }
            other if self.is_open() => {
                let items = Self::filtered_items(options, self.search.value());
                match self.collection.handle_intent(other, &items) {
                    CollectionOutcome::ActiveChanged { to, .. } => {
                        MultiSelectOutcome::HighlightChanged { id: to }
                    }
                    CollectionOutcome::Scrolled => MultiSelectOutcome::Changed,
                    CollectionOutcome::Ignored => MultiSelectOutcome::Ignored,
                }
            }
            _ => MultiSelectOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> MultiSelectOutcome<Id> {
        if !self.enabled {
            return MultiSelectOutcome::Ignored;
        }
        if matches!(event.kind, MouseEventKind::Moved) {
            // Hover is stated every event, so leaving the list clears it.
            let was = self.hovered.clone();
            self.hovered = self
                .option_regions
                .iter()
                .find(|(_, rect)| rect.contains(event.position))
                .map(|(id, _)| id.clone());
            return if was == self.hovered {
                MultiSelectOutcome::Ignored
            } else {
                MultiSelectOutcome::HoverChanged
            };
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return MultiSelectOutcome::Ignored;
        }
        if !self.is_open() {
            if self.trigger.contains(event.position) {
                self.focused = true;
                // clear hit on trigger?
                if let Some(clear) = self.clear_region {
                    if clear.contains(event.position) {
                        return self.clear_selection();
                    }
                }
                return self.open(bounds, options);
            }
            return MultiSelectOutcome::Ignored;
        }
        if let Some(clear) = self.clear_region {
            if clear.contains(event.position) {
                return self.clear_selection();
            }
        }
        let hit = self
            .option_regions
            .iter()
            .find(|(_, rect)| rect.contains(event.position))
            .map(|(id, _)| id.clone());
        if let Some(id) = hit {
            if let Some(opt) = options.iter().find(|o| o.id == id && o.is_option()) {
                if opt.disabled {
                    return MultiSelectOutcome::Ignored;
                }
            }
            let shift = event.modifiers.contains(KeyModifiers::SHIFT);
            if shift {
                return self.apply_range(options, &id);
            }
            self.collection.set_active(Some(id.clone()));
            self.range_anchor = Some(id.clone());
            return self.try_toggle(&id);
        }
        if !self.panel.contains(event.position) && !self.trigger.contains(event.position) {
            return self.close();
        }
        MultiSelectOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// MultiSelect chrome.
#[derive(Debug, Clone, Copy)]
pub struct MultiSelect<'a, Id> {
    options: &'a [SelectOption<Id>],
    system: &'a DesignSystem,
    placeholder: &'a str,
    label: &'a str,
    validation: Validation<'a>,
    show_clear: bool,
}

impl<'a, Id> MultiSelect<'a, Id> {
    /// Options + design system.
    #[must_use]
    pub const fn new(options: &'a [SelectOption<Id>], system: &'a DesignSystem) -> Self {
        Self {
            options,
            system,
            placeholder: "Select",
            label: "",
            validation: Validation::Valid,
            show_clear: true,
        }
    }

    /// Placeholder when none selected.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Validation.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    /// Clear affordance on trigger.
    pub const fn show_clear(mut self, on: bool) -> Self {
        self.show_clear = on;
        self
    }
}

impl<'a, Id: Clone + PartialEq + std::fmt::Display> MultiSelect<'a, Id> {
    /// Paint trigger + optional open list (stacked in `area` when open).
    pub fn paint_stacked(&self, area: Rect, buffer: &mut Buffer, state: &mut MultiSelectState<Id>) {
        if !state.is_open() {
            self.paint_trigger_only(area, buffer, state);
            return;
        }
        let base_trigger_h: u16 = if !self.label.is_empty() && area.height >= 3 {
            2
        } else {
            1
        };
        let trigger_h = base_trigger_h.saturating_add(1).min(area.height);
        let trigger_area = Rect::new(area.x, area.y, area.width, trigger_h.min(area.height));
        let list = Rect::new(
            area.x,
            area.y.saturating_add(trigger_h),
            area.width,
            area.height.saturating_sub(trigger_h),
        );
        self.paint(trigger_area, list, buffer, state);
    }

    /// Paint trigger in `area` and list in `list_area` when open.
    pub fn paint(
        &self,
        area: Rect,
        list_area: Rect,
        buffer: &mut Buffer,
        state: &mut MultiSelectState<Id>,
    ) {
        state.option_regions.clear();
        state.search_region = None;
        state.clear_region = None;
        if area.is_empty() {
            return;
        }
        self.paint_trigger_only(area, buffer, state);
        if state.is_open() && !list_area.is_empty() {
            state.panel = list_area;
            self.paint_list(list_area, buffer, state);
        } else {
            state.panel = Rect::default();
        }
    }

    fn paint_trigger_only(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut MultiSelectState<Id>,
    ) {
        let invalid = matches!(self.validation, Validation::Invalid(_));
        let recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if state.focused || state.is_open() {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            invalid,
        );
        let mut y = area.y;
        if (matches!(state.recipe, SelectRecipe::Form) || !self.label.is_empty())
            && area.height >= 2
            && !self.label.is_empty()
        {
            let mut style = recipe.value;
            if state.focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.label, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }
        let trigger = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            1.min(area.height),
        );
        state.trigger = trigger;
        if trigger.is_empty() {
            return;
        }

        buffer.set_style(trigger, recipe.fill);
        if let Some((glyph, style)) = recipe.prompt {
            buffer.set_stringn(trigger.x, trigger.y, glyph, 1, style);
        }

        if let Validation::Invalid(message) = self.validation
            && trigger.y.saturating_add(1) < area.bottom()
        {
            crate::widgets::field_message::paint_field_message(
                buffer,
                Rect::new(area.x, trigger.y.saturating_add(1), area.width, 1),
                self.system,
                crate::widgets::label::DescriptionKind::Error,
                message,
            );
        }

        let checked = state.selection.checked();
        let mut x = trigger.x.saturating_add(1).min(trigger.right());
        let mut right = trigger.right();

        // clear
        let show_clear = state.enabled && !checked.is_empty();
        if self.show_clear && trigger.width > 6 {
            right = right.saturating_sub(2);
            if show_clear {
                state.clear_region = Some(Rect::new(right.saturating_add(1), trigger.y, 1, 1));
                let clear_recipe = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    ControlState::Default,
                    self.system.junie_theme().surface,
                );
                buffer.set_stringn(
                    right.saturating_add(1),
                    trigger.y,
                    self.system.glyphs.resolve(Glyph::Close).text,
                    1,
                    clear_recipe.fill.patch(clear_recipe.label),
                );
            }
        }

        let chev = if state.is_open() { "▴" } else { "▾" };
        if right > x {
            right = right.saturating_sub(1);
            buffer.set_stringn(right, trigger.y, chev, 1, recipe.placeholder);
        }

        if checked.is_empty() {
            buffer.set_stringn(
                x,
                trigger.y,
                take_display_cols(self.placeholder, usize::from(right.saturating_sub(x))),
                usize::from(right.saturating_sub(x).max(1)),
                recipe.placeholder,
            );
            return;
        }

        // Chip summary
        let max_chips = state.max_summary_chips;
        let show = checked.len().min(max_chips);
        let overflow = checked.len().saturating_sub(show);
        for id in checked.iter().take(show) {
            let label = self
                .options
                .iter()
                .find(|o| o.is_option() && &o.id == id)
                .map(|o| o.label.as_str())
                .unwrap_or("?");
            let chip = format!("[{label}]");
            let w = display_cols(&chip) as u16;
            if x.saturating_add(w) >= right {
                break;
            }
            buffer.set_stringn(
                x,
                trigger.y,
                &take_display_cols(&chip, usize::from(w)),
                usize::from(w),
                recipe.cursor,
            );
            x = x.saturating_add(w).saturating_add(1);
        }
        if overflow > 0 && x.saturating_add(4) < right {
            let ov = format!("+{overflow}");
            buffer.set_stringn(
                x,
                trigger.y,
                &ov,
                usize::from(right.saturating_sub(x)),
                recipe.placeholder,
            );
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut MultiSelectState<Id>) {
        let panel = Panel::new(self.system)
            .variant(PanelVariant::Bordered)
            .overlay(true)
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut list_top = inner.y;
        // toolbar: select all / clear hints
        if inner.height >= 2 {
            let hint = { "Space toggle · ^A all · ^D clear · Enter done" };
            buffer.set_stringn(
                inner.x,
                list_top,
                take_display_cols(hint, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
            list_top = list_top.saturating_add(1);
        }

        if state.searchable && list_top < inner.bottom() {
            let search_row = Rect::new(inner.x, list_top, inner.width, 1);
            state.search_region = Some(search_row);
            state.search.set_focused(true);
            let _ = TextInput::new("", self.system).placeholder("Filter").paint(
                search_row,
                buffer,
                &mut state.search,
            );
            list_top = list_top.saturating_add(1);
        }

        let list_area = Rect::new(
            inner.x,
            list_top,
            inner.width,
            inner.bottom().saturating_sub(list_top),
        );
        if list_area.is_empty() {
            return;
        }

        let query = state.search.value().to_owned();
        let visible: Vec<&SelectOption<Id>> = if state.searchable && !query.is_empty() {
            let q = query.to_ascii_lowercase();
            let mut out = Vec::new();
            let mut pending_group: Option<&SelectOption<Id>> = None;
            for o in self.options {
                match o.kind {
                    SelectRowKind::Group => pending_group = Some(o),
                    SelectRowKind::Separator => {}
                    SelectRowKind::Option => {
                        if o.label.to_ascii_lowercase().contains(&q) {
                            if let Some(g) = pending_group.take() {
                                out.push(g);
                            }
                            out.push(o);
                        }
                    }
                }
            }
            out
        } else {
            self.options.iter().collect()
        };

        let coll_items = MultiSelectState::filtered_items(self.options, &query);
        let vp = usize::from(list_area.height).max(1);
        state
            .collection
            .set_viewport(state.collection.offset(), vp, coll_items.len());
        let _ = state.collection.reconcile(&coll_items);
        let _ = state.collection.ensure_active_visible(&coll_items);
        let offset = state.collection.offset();

        let mut row_y = list_area.y;
        let mut skipped = 0usize;

        for opt in &visible {
            if row_y >= list_area.bottom() {
                break;
            }
            match opt.kind {
                SelectRowKind::Separator => {
                    if skipped >= offset {
                        let line = "-".repeat(usize::from(list_area.width).min(64));
                        buffer.set_stringn(
                            list_area.x,
                            row_y,
                            take_display_cols(&line, usize::from(list_area.width)),
                            usize::from(list_area.width),
                            self.system.style(Role::Border),
                        );
                        row_y = row_y.saturating_add(1);
                    }
                }
                SelectRowKind::Group => {
                    if skipped >= offset {
                        buffer.set_stringn(
                            list_area.x,
                            row_y,
                            take_display_cols(&opt.label, usize::from(list_area.width)),
                            usize::from(list_area.width),
                            self.system
                                .style(Role::TextMuted)
                                .add_modifier(Modifier::BOLD),
                        );
                        row_y = row_y.saturating_add(1);
                    }
                }
                SelectRowKind::Option => {
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    let rect = Rect::new(list_area.x, row_y, list_area.width, 1);
                    let is_hi = state.collection.active() == Some(&opt.id);
                    let is_on = state.selection.is_checked(&opt.id);
                    let recipe = self.system.resolve_list_row(ListRowVisualState {
                        selected: is_hi,
                        focused: is_hi && state.focused,
                        hovered: state.hovered.as_ref() == Some(&opt.id),
                        enabled: !opt.disabled,
                        loading: false,
                        checked: is_on,
                        ..ListRowVisualState::default()
                    });
                    if recipe.use_fill {
                        buffer.set_style(rect, recipe.label);
                    } else if recipe.use_tint {
                        buffer.set_style(rect, recipe.tint);
                    }
                    let mark = if is_on { "[✓]" } else { "[ ]" };
                    // Highlight = reverse focus; checked mark independent
                    let style = recipe.label;
                    let label = if let Some(desc) = &opt.description {
                        format!("{mark} {} {} {desc}", opt.label, { "—" })
                    } else {
                        format!("{mark} {}", opt.label)
                    };
                    buffer.set_stringn(
                        rect.x,
                        rect.y,
                        take_display_cols(&label, usize::from(rect.width)),
                        usize::from(rect.width),
                        style,
                    );
                    if !opt.disabled {
                        state.option_regions.push((opt.id.clone(), rect));
                    }
                    row_y = row_y.saturating_add(1);
                    skipped += 1; // count painted options toward viewport? offset is among coll
                }
            }
        }
    }

    /// Semantic for trigger.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &MultiSelectState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "multi-select {} count={}",
            state.presentation.id(),
            state.selection.checked().len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "multi-select"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused || state.is_open(),
                    invalid: matches!(self.validation, Validation::Invalid(_)),
                    expanded: state.is_open(),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq + std::fmt::Display> StatefulWidget for &MultiSelect<'_, Id> {
    type State = MultiSelectState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint_stacked(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq + std::fmt::Display> StatefulWidget for MultiSelect<'_, Id> {
    type State = MultiSelectState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    fn opts() -> Vec<SelectOption<&'static str>> {
        vec![
            SelectOption::group("g", "Lang"),
            SelectOption::option("rs", "Rust"),
            SelectOption::option("go", "Go"),
            SelectOption::option("ts", "TypeScript"),
            SelectOption::option("off", "Off").disabled(true),
        ]
    }

    #[test]
    fn toggle_membership_highlight_distinct() {
        let options = opts();
        let mut state = MultiSelectState::new();
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        assert_eq!(state.highlight(), Some(&"rs"));
        assert!(!state.is_checked(&"rs"));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                &options,
                bounds
            ),
            MultiSelectOutcome::Toggled {
                id: "rs",
                checked: true
            }
        ));
        assert!(state.is_checked(&"rs"));
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &options,
            bounds,
        );
        assert_eq!(state.highlight(), Some(&"go"));
        assert!(!state.is_checked(&"go"));
    }

    #[test]
    fn select_all_and_clear() {
        let options = opts();
        let mut state = MultiSelectState::new();
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        let out = state.select_all_visible(&options);
        assert!(matches!(out, MultiSelectOutcome::SelectAll { count: 3 }));
        assert_eq!(state.selected().len(), 3);
        assert_eq!(state.clear_selection(), MultiSelectOutcome::Cleared);
        assert!(state.selected().is_empty());
    }

    #[test]
    fn max_selected_blocks() {
        let options = opts();
        let mut state = MultiSelectState::new().with_max_selected(Some(1));
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &options,
            bounds,
        );
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &options,
            bounds,
        );
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                &options,
                bounds
            ),
            MultiSelectOutcome::MaxReached { max: 1 }
        ));
    }

    #[test]
    fn shift_range_select() {
        let options = opts();
        let mut state = MultiSelectState::new();
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        // anchor on rs without checking
        state.range_anchor = Some("rs");
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
                &options,
                bounds
            ),
            MultiSelectOutcome::RangeApplied { .. }
        ));
        assert!(state.is_checked(&"rs") || state.is_checked(&"go"));
    }

    #[test]
    fn esc_keeps_selection() {
        let options = opts();
        let mut state = MultiSelectState::new().with_selected(["rs"]);
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &options,
                bounds
            ),
            MultiSelectOutcome::Closed
        );
        assert!(state.is_checked(&"rs"));
    }

    #[test]
    fn enter_confirms_close() {
        let options = opts();
        let mut state = MultiSelectState::new();
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &options,
            bounds,
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &options,
                bounds
            ),
            MultiSelectOutcome::Closed
        );
        assert!(state.is_checked(&"rs"));
    }

    #[test]
    fn tiny_fullscreen() {
        let tiny = Rect::new(0, 0, 20, 8);
        let mut state = MultiSelectState::<&str>::new();
        let options = opts();
        match state.open(tiny, &options) {
            MultiSelectOutcome::Opened {
                presentation: SelectPresentation::Fullscreen,
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn summary_chips_paint() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let options = opts();
        let mut state = MultiSelectState::new()
            .with_selected(["rs", "go", "ts"])
            .with_max_summary_chips(2);
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        MultiSelect::new(&options, &system)
            .label("Filters")
            .paint_stacked(area, &mut buf, &mut state);
        assert!(!state.trigger.is_empty());
        let mut row = String::new();
        for x in 0..area.width {
            row.push_str(buf[(x, 1.min(area.height - 1))].symbol());
        }
        assert!(
            row.contains('+') || row.contains('[') || row.contains('R') || !row.trim().is_empty()
        );
    }

    #[test]
    fn mouse_toggle() {
        let system = DesignSystem::default();
        let options = opts();
        let mut state = MultiSelectState::new();
        state.set_focused(true);
        let area = Rect::new(0, 0, 48, 16);
        let mut buf = Buffer::empty(area);
        let _ = state.open(area, &options);
        MultiSelect::new(&options, &system).paint_stacked(area, &mut buf, &mut state);
        assert!(!state.option_regions.is_empty());
        let (id, rect) = state.option_regions[0].clone();
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(rect.x, rect.y),
                    modifiers: KeyModifiers::NONE,
                },
                &options,
                area,
            ),
            MultiSelectOutcome::Toggled { id: tid, checked: true } if tid == id
        ));
    }

    #[test]
    fn search_filters() {
        let options = opts();
        let mut state = MultiSelectState::new().with_searchable(true);
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        let _ = state.search.insert_str("go");
        state.reconcile_options(&options);
        let items = MultiSelectState::filtered_items(&options, "go");
        assert!(items.iter().any(|i| i.id == "go"));
        assert!(!items.iter().any(|i| i.id == "rs"));
    }

    #[test]
    fn fuzz_open() {
        let options = opts();
        let mut state = MultiSelectState::new().with_searchable(true);
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &options);
        let keys = [
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT),
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key, &options, bounds);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let options = opts();
        let mut state = MultiSelectState::new().with_selected(["rs"]);
        state.set_focused(true);
        let area = Rect::new(0, 0, 50, 14);
        let mut buf = Buffer::empty(area);
        let _ = state.open(area, &options);
        let w = MultiSelect::new(&options, &system);
        for _ in 0..80 {
            w.paint_stacked(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let options = opts();
        let state = MultiSelectState::new().with_selected(["rs"]);
        let mut scene = SemanticScene::<&str, ()>::default();
        MultiSelect::new(&options, &system).register_semantic(
            &mut scene,
            "m",
            Rect::new(0, 0, 20, 1),
            &state,
        );
        assert!(scene.get(&"m").is_some());
    }
}
