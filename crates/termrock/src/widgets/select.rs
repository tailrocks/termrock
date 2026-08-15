// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Single-choice Select: opener + list built on [`CollectionState`] and popover chrome.
//!
//! **Mission.** Forms and toolbars need a compact trigger that opens a
//! navigable option list with placeholder, groups, separators, typeahead /
//! search, disabled options, and a **value** distinct from the **highlight**.
//!
//! **vs [`RadioGroup`](super::RadioGroup).** Always-visible exclusive options.
//! **vs [`Picker`](super::Picker).** Full query+list composition; Select is the
//! closed-by-default form control.
//! **vs [`ThemePicker`](super::ThemePicker).** Domain-specific preset browser.
//!
//! **Overlay.** Host opens [`open_popover_overlay`](super::open_popover_overlay)
//! or fullscreen via outcomes; Select tracks open state and list geometry.
//! Tiny terminals set [`SelectPresentation::Fullscreen`].
//!
//! Research: Radix Select, Huh select, Textual Select, terminal pickers.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
};

use super::{Panel, PanelChrome, TextInput, TextInputOutcome, TextInputState, Validation};

/// Width under which open list prefers fullscreen.
pub const SELECT_FULLSCREEN_MAX_WIDTH: u16 = 28;
/// Height under which open list prefers fullscreen.
pub const SELECT_FULLSCREEN_MAX_HEIGHT: u16 = 10;

// ── Recipe / presentation ───────────────────────────────────────────────────

/// Visual recipe for the closed trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectRecipe {
    /// Default field-like trigger.
    #[default]
    Inline,
    /// Form field with label emphasis.
    Form,
    /// Compact toolbar control.
    Compact,
}

impl SelectRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Form => "form",
            Self::Compact => "compact",
        }
    }
}

/// How the open list is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectPresentation {
    /// Closed.
    #[default]
    Closed,
    /// Anchored popover list.
    Popover,
    /// Nearly fullscreen list (tiny terminal / many options).
    Fullscreen,
}

impl SelectPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Popover => "popover",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Whether the list is open.
    #[must_use]
    pub const fn is_open(self) -> bool {
        !matches!(self, Self::Closed)
    }
}

/// Kind of row in the option projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectRowKind {
    /// Selectable option.
    #[default]
    Option,
    /// Non-selectable group header.
    Group,
    /// Visual separator.
    Separator,
}

// ── Option model ────────────────────────────────────────────────────────────

/// One projected row for paint/navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption<Id> {
    /// Stable id (only meaningful for [`SelectRowKind::Option`]).
    pub id: Id,
    /// Primary label.
    pub label: String,
    /// Secondary description.
    pub description: Option<String>,
    /// Disabled (options only).
    pub disabled: bool,
    /// Row kind.
    pub kind: SelectRowKind,
}

impl<Id> SelectOption<Id> {
    /// Selectable option.
    #[must_use]
    pub fn option(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            description: None,
            disabled: false,
            kind: SelectRowKind::Option,
        }
    }

    /// Description.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Disabled option.
    #[must_use]
    pub const fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }

    /// Group header (id is a sentinel; not selectable).
    #[must_use]
    pub fn group(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            description: None,
            disabled: true,
            kind: SelectRowKind::Group,
        }
    }

    /// Separator (id is a sentinel).
    #[must_use]
    pub fn separator(id: Id) -> Self {
        Self {
            id,
            label: String::new(),
            description: None,
            disabled: true,
            kind: SelectRowKind::Separator,
        }
    }

    /// Whether this row is a navigable option.
    #[must_use]
    pub const fn is_option(&self) -> bool {
        matches!(self.kind, SelectRowKind::Option)
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Select interaction outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectOutcome<Id> {
    /// No effect.
    Ignored,
    /// Opener focus / chrome changed.
    Changed,
    /// List opened (host should place popover/fullscreen overlay).
    Opened {
        /// Presentation chosen.
        presentation: SelectPresentation,
    },
    /// List closed without commit (Esc / outside).
    Closed,
    /// The pointer moved onto (or off) an option.
    HoverChanged,
    /// Highlight moved in the open list.
    HighlightChanged {
        /// New highlight id.
        id: Option<Id>,
    },
    /// Value committed (Enter / click option).
    ValueChanged {
        /// New value.
        id: Id,
    },
    /// Search draft changed (filter host projection).
    SearchChanged {
        /// Search text.
        query: String,
    },
    /// Host should promote / demote presentation (tiny terminal).
    PresentationChanged {
        /// New presentation.
        presentation: SelectPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`Select`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectState<Id> {
    /// Controlled committed value.
    value: Option<Id>,
    /// Open presentation.
    presentation: SelectPresentation,
    /// List cursor (highlight) — distinct from value.
    collection: CollectionState<Id>,
    /// Optional search field when open + searchable.
    search: TextInputState,
    searchable: bool,
    recipe: SelectRecipe,
    focused: bool,
    enabled: bool,
    /// Preferred list height in rows.
    list_rows: u16,
    trigger: Rect,
    panel: Rect,
    option_regions: Vec<(Id, Rect)>,
    /// Option the pointer is over (hover wash; never a commit).
    hovered: Option<Id>,
    search_region: Option<Rect>,
}

impl<Id> Default for SelectState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> SelectState<Id> {
    /// Closed select with no value.
    #[must_use]
    pub fn new() -> Self {
        let mut search = TextInputState::new("").with_allow_empty(true);
        search.set_focused(false);
        Self {
            value: None,
            presentation: SelectPresentation::Closed,
            collection: CollectionState::new().wrap(true),
            search,
            searchable: false,
            recipe: SelectRecipe::Inline,
            focused: false,
            enabled: true,
            list_rows: 8,
            trigger: Rect::default(),
            panel: Rect::default(),
            option_regions: Vec::new(),
            hovered: None,
            search_region: None,
        }
    }

    /// Initial value.
    #[must_use]
    pub fn with_value(mut self, id: Id) -> Self
    where
        Id: Clone + PartialEq,
    {
        self.value = Some(id.clone());
        self.collection.set_active(Some(id));
        self
    }

    /// Enable search field in open list.
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

    /// Preferred list rows.
    #[must_use]
    pub fn with_list_rows(mut self, rows: u16) -> Self {
        self.list_rows = rows.max(3);
        self
    }

    /// Committed value.
    #[must_use]
    pub const fn value(&self) -> Option<&Id> {
        self.value.as_ref()
    }

    /// Highlight (open list cursor).
    #[must_use]
    pub const fn highlight(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> SelectPresentation {
        self.presentation
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.presentation.is_open()
    }

    /// Search query.
    #[must_use]
    pub fn search_query(&self) -> &str {
        self.search.value()
    }

    /// Focused (opener when closed; list when open).
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

    /// Collection model.
    #[must_use]
    pub const fn collection(&self) -> &CollectionState<Id> {
        &self.collection
    }

    /// Trigger geometry.
    #[must_use]
    pub const fn trigger_area(&self) -> Rect {
        self.trigger
    }

    /// Open list geometry.
    #[must_use]
    pub const fn panel_area(&self) -> Rect {
        self.panel
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on && self.is_open() {
            // keep open unless host closes; only unfocus search
            self.search.set_focused(false);
        }
    }

    /// Enabled.
    pub const fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Controlled value.
    pub fn set_value(&mut self, id: Option<Id>)
    where
        Id: Clone + PartialEq,
    {
        self.value = id.clone();
        if !self.is_open() {
            self.collection.set_active(id);
        }
    }

    /// Force presentation (host overlay policy).
    pub const fn set_presentation(&mut self, presentation: SelectPresentation) {
        self.presentation = presentation;
    }
}

impl<Id: Clone + PartialEq> SelectState<Id> {
    /// Build collection items from options (only selectable options).
    #[must_use]
    pub fn collection_items(options: &[SelectOption<Id>]) -> Vec<CollectionItem<Id>> {
        options
            .iter()
            .filter(|o| o.is_option())
            .map(|o| CollectionItem::new(o.id.clone(), o.label.clone()).enabled(!o.disabled))
            .collect()
    }

    /// Filter options by search query (case-insensitive substring on label).
    #[must_use]
    pub fn filter_options<'a>(
        options: &'a [SelectOption<Id>],
        query: &str,
    ) -> Vec<&'a SelectOption<Id>> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return options.iter().collect();
        }
        // Keep group headers that precede a matching option; separators if neighbors match.
        let mut out = Vec::new();
        let mut pending_group: Option<&SelectOption<Id>> = None;
        for o in options {
            match o.kind {
                SelectRowKind::Group => {
                    pending_group = Some(o);
                }
                SelectRowKind::Separator => {
                    // drop unless we already emitted something
                }
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
    }

    /// Choose presentation for terminal size.
    #[must_use]
    pub fn presentation_for_bounds(bounds: Rect, force_fullscreen: bool) -> SelectPresentation {
        if force_fullscreen
            || bounds.width < SELECT_FULLSCREEN_MAX_WIDTH
            || bounds.height < SELECT_FULLSCREEN_MAX_HEIGHT
        {
            SelectPresentation::Fullscreen
        } else {
            SelectPresentation::Popover
        }
    }

    /// Open list.
    pub fn open(&mut self, bounds: Rect, options: &[SelectOption<Id>]) -> SelectOutcome<Id> {
        if !self.enabled {
            return SelectOutcome::Ignored;
        }
        let presentation = Self::presentation_for_bounds(bounds, false);
        self.presentation = presentation;
        let items = Self::collection_items(options);
        let _ = self.collection.reconcile(&items);
        // Highlight value if present, else first enabled
        if let Some(v) = self.value.clone() {
            if items.iter().any(|i| i.id == v && i.enabled) {
                self.collection.set_active(Some(v));
            }
        }
        if self.searchable {
            self.search.set_focused(true);
            self.search.set_enabled(true);
        }
        self.focused = true;
        SelectOutcome::Opened { presentation }
    }

    /// Close without changing value.
    pub fn close(&mut self) -> SelectOutcome<Id> {
        if !self.is_open() {
            return SelectOutcome::Ignored;
        }
        self.presentation = SelectPresentation::Closed;
        self.search.set_focused(false);
        let _ = self.search.clear();
        // Restore collection active to value for next open
        self.collection.set_active(self.value.clone());
        SelectOutcome::Closed
    }

    /// Commit current highlight as value and close.
    pub fn commit_highlight(&mut self) -> SelectOutcome<Id> {
        let Some(id) = self.collection.active().cloned() else {
            return SelectOutcome::Ignored;
        };
        self.value = Some(id.clone());
        self.presentation = SelectPresentation::Closed;
        self.search.set_focused(false);
        let _ = self.search.clear();
        SelectOutcome::ValueChanged { id }
    }

    /// Reconcile after option list changes while open.
    pub fn reconcile_options(&mut self, options: &[SelectOption<Id>]) {
        let items = if self.searchable && !self.search.value().is_empty() {
            let filtered = Self::filter_options(options, self.search.value());
            filtered
                .into_iter()
                .filter(|o| o.is_option())
                .map(|o| CollectionItem::new(o.id.clone(), o.label.clone()).enabled(!o.disabled))
                .collect()
        } else {
            Self::collection_items(options)
        };
        let _ = self.collection.reconcile(&items);
    }

    /// Key adapter. Pass full option set; filtering applied when searchable.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> SelectOutcome<Id> {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return SelectOutcome::Ignored;
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
    ) -> SelectOutcome<Id> {
        if !self.focused {
            return SelectOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ')
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::NONE =>
            {
                self.open(bounds, options)
            }
            KeyCode::Down if key.modifiers.is_empty() => self.open(bounds, options),
            KeyCode::Esc => SelectOutcome::Ignored,
            // typeahead open + first char
            KeyCode::Char(c)
                if !c.is_control()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                let out = self.open(bounds, options);
                if self.searchable {
                    let _ = self.search.insert_str(&c.to_string());
                    self.reconcile_options(options);
                    return SelectOutcome::SearchChanged {
                        query: self.search.value().to_owned(),
                    };
                }
                // non-searchable: let collection typeahead handle after open
                let items = Self::collection_items(options);
                let _ = self.collection.handle_key(key, &items);
                out
            }
            _ => SelectOutcome::Ignored,
        }
    }

    fn handle_open_key(
        &mut self,
        key: KeyEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> SelectOutcome<Id> {
        // Esc close
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            return self.close();
        }

        // Enter commit
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            return self.commit_highlight();
        }

        // Promote presentation if bounds shrink
        let desired = Self::presentation_for_bounds(bounds, false);
        if desired != self.presentation && desired != SelectPresentation::Closed {
            self.presentation = desired;
            return SelectOutcome::PresentationChanged {
                presentation: desired,
            };
        }

        // Search field takes printable when searchable and focused on search
        if self.searchable {
            // Tab toggles search focus vs list — keep simple: printable goes to search
            if matches!(
                key.code,
                KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete
            ) || key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('a' | 'A' | 'u' | 'U'))
            {
                match self.search.handle_key(key) {
                    TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                        self.reconcile_options(options);
                        return SelectOutcome::SearchChanged {
                            query: self.search.value().to_owned(),
                        };
                    }
                    TextInputOutcome::Ignored => {}
                    _ => {}
                }
            }
        }

        let items = if self.searchable && !self.search.value().is_empty() {
            Self::filter_options(options, self.search.value())
                .into_iter()
                .filter(|o| o.is_option())
                .map(|o| CollectionItem::new(o.id.clone(), o.label.clone()).enabled(!o.disabled))
                .collect::<Vec<_>>()
        } else {
            Self::collection_items(options)
        };

        // Page / arrows via collection
        match self.collection.handle_key(key, &items) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                SelectOutcome::HighlightChanged { id: to }
            }
            CollectionOutcome::Scrolled => SelectOutcome::Changed,
            CollectionOutcome::Ignored => {
                // Space commits like Enter when open
                if matches!(key.code, KeyCode::Char(' ')) && key.modifiers.is_empty() {
                    return self.commit_highlight();
                }
                SelectOutcome::Ignored
            }
        }
    }

    /// Intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> SelectOutcome<Id> {
        if !self.enabled {
            return SelectOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Submit if !self.is_open() => self.open(bounds, options),
            UiIntent::Activate | UiIntent::Submit if self.is_open() => self.commit_highlight(),
            UiIntent::Cancel | UiIntent::Close if self.is_open() => self.close(),
            UiIntent::Fullscreen if self.is_open() => {
                self.presentation = SelectPresentation::Fullscreen;
                SelectOutcome::PresentationChanged {
                    presentation: SelectPresentation::Fullscreen,
                }
            }
            other if self.is_open() => {
                let items = Self::collection_items(options);
                match self.collection.handle_intent(other, &items) {
                    CollectionOutcome::ActiveChanged { to, .. } => {
                        SelectOutcome::HighlightChanged { id: to }
                    }
                    CollectionOutcome::Scrolled => SelectOutcome::Changed,
                    CollectionOutcome::Ignored => SelectOutcome::Ignored,
                }
            }
            _ => SelectOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> SelectOutcome<Id> {
        if !self.enabled {
            return SelectOutcome::Ignored;
        }
        if matches!(event.kind, MouseEventKind::Moved) {
            // Hover is stated per event, unconditionally: a pointer that
            // leaves the list must clear it (plans/021 Step 1).
            let was = self.hovered.clone();
            self.hovered = self
                .option_regions
                .iter()
                .find(|(_, rect)| rect.contains(event.position))
                .map(|(id, _)| id.clone());
            return if was == self.hovered {
                SelectOutcome::Ignored
            } else {
                SelectOutcome::HoverChanged
            };
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return SelectOutcome::Ignored;
        }
        if !self.is_open() {
            if self.trigger.contains(event.position) {
                self.focused = true;
                return self.open(bounds, options);
            }
            return SelectOutcome::Ignored;
        }
        // click option
        for (id, rect) in &self.option_regions {
            if rect.contains(event.position) {
                if let Some(opt) = options.iter().find(|o| &o.id == id && o.is_option()) {
                    if opt.disabled {
                        return SelectOutcome::Ignored;
                    }
                }
                self.value = Some(id.clone());
                self.presentation = SelectPresentation::Closed;
                self.search.set_focused(false);
                let _ = self.search.clear();
                return SelectOutcome::ValueChanged { id: id.clone() };
            }
        }
        // click outside panel → close
        if !self.panel.contains(event.position) && !self.trigger.contains(event.position) {
            return self.close();
        }
        SelectOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Select chrome.
#[derive(Debug, Clone, Copy)]
pub struct Select<'a, Id> {
    options: &'a [SelectOption<Id>],
    system: &'a DesignSystem,
    placeholder: &'a str,
    label: &'a str,
    validation: Validation<'a>,
    ascii: bool,
}

impl<'a, Id> Select<'a, Id> {
    /// Select over options.
    #[must_use]
    pub const fn new(options: &'a [SelectOption<Id>], system: &'a DesignSystem) -> Self {
        Self {
            options,
            system,
            placeholder: "Select…",
            label: "",
            validation: Validation::Valid,
            ascii: false,
        }
    }

    /// Placeholder when no value.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Label (form recipe).
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

    /// ASCII chevrons.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }
}

impl<'a, Id: Clone + PartialEq + std::fmt::Display> Select<'a, Id> {
    /// Paint trigger (+ open panel when state is open).
    ///
    /// `area` is the **trigger** region for closed/popover; when fullscreen,
    /// host should pass a larger panel area and call
    /// [`SelectState::set_presentation`] first. This method paints trigger in
    /// `area` and, if open, paints list into `list_area` (may equal `area` for
    /// fullscreen).
    pub fn paint(
        &self,
        area: Rect,
        list_area: Rect,
        buffer: &mut Buffer,
        state: &mut SelectState<Id>,
    ) {
        state.option_regions.clear();
        state.search_region = None;
        if area.is_empty() {
            return;
        }

        let mut y = area.y;
        let formish = matches!(state.recipe, SelectRecipe::Form) || !self.label.is_empty();
        if formish && area.height >= 2 && !self.label.is_empty() {
            let mut style = self.system.style(if state.focused {
                Role::Focus
            } else {
                Role::Text
            });
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

        let trigger_h = if matches!(state.recipe, SelectRecipe::Compact) {
            1
        } else {
            1
        };
        let trigger = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            trigger_h.min(area.height.saturating_sub(y.saturating_sub(area.y))),
        );
        state.trigger = trigger;
        self.paint_trigger(trigger, buffer, state);

        if state.is_open() && !list_area.is_empty() {
            state.panel = list_area;
            self.paint_list(list_area, buffer, state);
        } else {
            state.panel = Rect::default();
        }

        // Validation directly under the trigger — not pinned to the bottom
        // edge, where it drifted away from the field it describes.
        if area.height >= 3
            && let Validation::Invalid(msg) = self.validation
        {
            crate::widgets::field_message::paint_field_message(
                buffer,
                Rect::new(area.x, area.y.saturating_add(2), area.width, 1),
                self.system,
                crate::widgets::label::DescriptionKind::Error,
                msg,
            );
        }
    }

    /// Convenience: paint trigger; if open, list fills remainder below trigger in `area`.
    pub fn paint_stacked(&self, area: Rect, buffer: &mut Buffer, state: &mut SelectState<Id>) {
        if !state.is_open() {
            self.paint(area, Rect::default(), buffer, state);
            return;
        }
        let trigger_h = if !self.label.is_empty() && area.height >= 3 {
            2
        } else {
            1
        };
        let trigger_area = Rect::new(area.x, area.y, area.width, trigger_h.min(area.height));
        let list = Rect::new(
            area.x,
            area.y.saturating_add(trigger_h),
            area.width,
            area.height.saturating_sub(trigger_h),
        );
        self.paint(trigger_area, list, buffer, state);
    }

    fn paint_trigger(&self, area: Rect, buffer: &mut Buffer, state: &SelectState<Id>) {
        if area.is_empty() {
            return;
        }
        let invalid = matches!(self.validation, Validation::Invalid(_));
        // The trigger is a field, so it wears the field's chrome. Swapping the
        // whole style to `Role::Focus` on focus threw away the well underneath
        // it — the box stopped looking like something you type into at the one
        // moment it mattered.
        let control_state = if !state.enabled {
            crate::style::ControlState::Disabled
        } else if state.focused || state.is_open() {
            crate::style::ControlState::Focused
        } else {
            crate::style::ControlState::Default
        };
        let recipe = self.system.input_recipe(control_state, invalid);
        buffer.set_style(area, recipe.fill);
        if let Some((glyph, style)) = recipe.prompt
            && area.width > 0
        {
            buffer.set_stringn(area.x, area.y, glyph, 1, style);
        }

        let value_label = state
            .value
            .as_ref()
            .and_then(|id| {
                self.options
                    .iter()
                    .find(|o| o.is_option() && &o.id == id)
                    .map(|o| o.label.as_str())
            })
            .unwrap_or(self.placeholder);

        let chev = if self.ascii {
            if state.is_open() { "^" } else { "v" }
        } else if state.is_open() {
            "▴"
        } else {
            "▾"
        };
        let text_w = area.width.saturating_sub(2);
        let muted = state.value.is_none();
        buffer.set_stringn(
            area.x,
            area.y,
            take_display_cols(value_label, usize::from(text_w)),
            usize::from(text_w),
            self.system.style(if muted {
                Role::TextMuted
            } else if state.focused {
                Role::TextStrong
            } else {
                Role::Text
            }),
        );
        if area.width > 0 {
            buffer.set_stringn(
                area.right().saturating_sub(1),
                area.y,
                chev,
                1,
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut SelectState<Id>) {
        let panel = Panel::new(self.system).emphasis(if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        });
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut list_top = inner.y;
        if state.searchable {
            let search_row = Rect::new(inner.x, inner.y, inner.width, 1);
            state.search_region = Some(search_row);
            state.search.set_focused(true);
            let _ = TextInput::new("", self.system)
                .placeholder("Filter…")
                .paint(search_row, buffer, &mut state.search);
            list_top = list_top.saturating_add(1);
        }

        let full_list = Rect::new(
            inner.x,
            list_top,
            inner.width,
            inner.bottom().saturating_sub(list_top),
        );
        if full_list.is_empty() {
            return;
        }
        // Reserve the scroll gutter whether or not it is painted, so rows do
        // not reflow the moment the list grows past its viewport
        // (plans/022 Step 2).
        let gutter = Rect::new(
            full_list.right().saturating_sub(1),
            full_list.y,
            1,
            full_list.height,
        );
        let list_area = Rect::new(
            full_list.x,
            full_list.y,
            full_list.width.saturating_sub(1),
            full_list.height,
        );
        if list_area.is_empty() {
            return;
        }

        let visible_opts: Vec<&SelectOption<Id>> =
            if state.searchable && !state.search.value().is_empty() {
                SelectState::filter_options(self.options, state.search.value())
            } else {
                self.options.iter().collect()
            };

        // Flatten for collection viewport among options only
        let coll_items: Vec<CollectionItem<Id>> = visible_opts
            .iter()
            .filter(|o| o.is_option())
            .map(|o| CollectionItem::new(o.id.clone(), o.label.clone()).enabled(!o.disabled))
            .collect();
        let vp = usize::from(list_area.height).max(1);
        state
            .collection
            .set_viewport(state.collection.offset(), vp, coll_items.len());
        let _ = state.collection.reconcile(&coll_items);
        let _ = state.collection.ensure_active_visible(&coll_items);

        // Paint all visible_opts with scroll only on options — simple: paint from start
        // with offset applied to option rows by skipping until offset options painted
        let mut option_idx = 0usize;
        let mut row_y = list_area.y;
        let offset = state.collection.offset();
        let mut skipped = 0usize;

        for opt in &visible_opts {
            if row_y >= list_area.bottom() {
                break;
            }
            match opt.kind {
                SelectRowKind::Separator => {
                    if skipped >= offset || option_idx == 0 {
                        let line = self
                            .system
                            .glyphs
                            .rule()
                            .repeat(usize::from(list_area.width));
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
                        option_idx += 1;
                        continue;
                    }
                    let rect = Rect::new(list_area.x, row_y, list_area.width, 1);
                    let is_hi = state.collection.active() == Some(&opt.id);
                    let is_val = state.value.as_ref() == Some(&opt.id);
                    let recipe = self.system.resolve_list_row(ListRowVisualState {
                        selected: is_hi,
                        focused: is_hi && state.focused,
                        hovered: state.hovered.as_ref() == Some(&opt.id),
                        enabled: !opt.disabled,
                        loading: false,
                        checked: is_val,
                    });
                    if recipe.use_fill {
                        buffer.set_style(rect, recipe.label);
                    } else if recipe.use_tint {
                        buffer.set_style(rect, recipe.tint);
                    }
                    let mut style = if opt.disabled {
                        self.system.style(Role::TextDisabled)
                    } else if is_hi {
                        recipe.label
                    } else {
                        self.system.style(Role::Text)
                    };
                    if is_val && !is_hi {
                        style = self.system.style(Role::TextStrong);
                    }
                    let mark = if is_val {
                        if self.ascii { "*" } else { "✓" }
                    } else {
                        " "
                    };
                    let label = if let Some(desc) = &opt.description {
                        format!("{mark} {} — {desc}", opt.label)
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
                    option_idx += 1;
                }
            }
        }

        crate::scroll::paint_scrolled_region(
            buffer,
            list_area,
            gutter,
            coll_items.len(),
            vp,
            u16::try_from(state.collection.offset()).unwrap_or(u16::MAX),
            self.system,
        );
    }

    /// Semantic registration for trigger.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &SelectState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!("select {} {}", state.presentation.id(), state.recipe.id());
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "select"
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

impl<Id: Clone + PartialEq + std::fmt::Display> StatefulWidget for &Select<'_, Id> {
    type State = SelectState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint_stacked(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq + std::fmt::Display> StatefulWidget for Select<'_, Id> {
    type State = SelectState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// Touch display_cols for measure helpers
const _: fn(&str) -> usize = display_cols;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    fn sample_options() -> Vec<SelectOption<&'static str>> {
        vec![
            SelectOption::group("g-fruits", "Fruits"),
            SelectOption::option("apple", "Apple").description("red"),
            SelectOption::option("banana", "Banana"),
            SelectOption::separator("sep1"),
            SelectOption::group("g-veg", "Veggies"),
            SelectOption::option("carrot", "Carrot"),
            SelectOption::option("disabled", "Nope").disabled(true),
        ]
    }

    #[test]
    fn open_highlight_distinct_from_value() {
        let opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        assert!(matches!(
            state.open(bounds, &opts),
            SelectOutcome::Opened { .. }
        ));
        assert_eq!(state.value(), Some(&"apple"));
        assert_eq!(state.highlight(), Some(&"apple"));
        // move highlight
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &opts,
            bounds,
        );
        assert_eq!(state.value(), Some(&"apple"));
        assert_eq!(state.highlight(), Some(&"banana"));
    }

    #[test]
    fn esc_closes_without_commit() {
        let opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &opts);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &opts,
            bounds,
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                &opts,
                bounds
            ),
            SelectOutcome::Closed
        );
        assert_eq!(state.value(), Some(&"apple"));
        assert!(!state.is_open());
    }

    #[test]
    fn enter_commits_highlight() {
        let opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &opts);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &opts,
            bounds,
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &opts,
                bounds
            ),
            SelectOutcome::ValueChanged { id: "banana" }
        );
        assert_eq!(state.value(), Some(&"banana"));
    }

    #[test]
    fn search_filters_options() {
        let opts = sample_options();
        let mut state = SelectState::new().with_searchable(true);
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &opts);
        let _ = state.search.insert_str("car");
        state.reconcile_options(&opts);
        let items = SelectState::collection_items(
            &SelectState::filter_options(&opts, "car")
                .into_iter()
                .cloned()
                .collect::<Vec<_>>(),
        );
        assert!(items.iter().any(|i| i.id == "carrot"));
        assert!(!items.iter().any(|i| i.id == "apple"));
    }

    #[test]
    fn tiny_bounds_fullscreen() {
        let tiny = Rect::new(0, 0, 20, 8);
        assert_eq!(
            SelectState::<&str>::presentation_for_bounds(tiny, false),
            SelectPresentation::Fullscreen
        );
        let big = Rect::new(0, 0, 80, 24);
        assert_eq!(
            SelectState::<&str>::presentation_for_bounds(big, false),
            SelectPresentation::Popover
        );
    }

    #[test]
    fn dynamic_options_reconcile() {
        let mut opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &opts);
        opts.push(SelectOption::option("date", "Date"));
        state.reconcile_options(&opts);
        // still valid highlight
        assert!(state.highlight().is_some());
    }

    #[test]
    fn disabled_skipped_in_collection() {
        let opts = sample_options();
        let items = SelectState::collection_items(&opts);
        assert!(items.iter().any(|i| i.id == "disabled" && !i.enabled));
        let mut state = SelectState::new();
        let _ = state.collection.reconcile(&items);
        // move through — should not land on disabled as active if starting first
        let _ = state.collection.move_last(&items);
        assert_ne!(state.collection.active(), Some(&"disabled"));
    }

    #[test]
    fn mouse_select_option() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let opts = sample_options();
        let mut state = SelectState::new();
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 16);
        let mut buf = Buffer::empty(area);
        let _ = state.open(area, &opts);
        Select::new(&opts, &system).paint_stacked(area, &mut buf, &mut state);
        assert!(!state.option_regions.is_empty());
        let (id, rect) = state.option_regions[0].clone();
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(rect.x, rect.y),
                    modifiers: KeyModifiers::NONE,
                },
                &opts,
                area,
            ),
            SelectOutcome::ValueChanged { id }
        );
    }

    #[test]
    fn recipes_paint() {
        let system = DesignSystem::default();
        let opts = sample_options();
        for recipe in [
            SelectRecipe::Inline,
            SelectRecipe::Form,
            SelectRecipe::Compact,
        ] {
            let mut state = SelectState::new().with_recipe(recipe).with_value("apple");
            state.set_focused(true);
            let area = Rect::new(0, 0, 32, 4);
            let mut buf = Buffer::empty(area);
            Select::new(&opts, &system)
                .label("Fruit")
                .paint_stacked(area, &mut buf, &mut state);
            assert!(!state.trigger.is_empty());
        }
    }

    #[test]
    fn nested_open_close_cycle() {
        let opts = sample_options();
        let mut state = SelectState::new();
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        for _ in 0..5 {
            let _ = state.open(bounds, &opts);
            assert!(state.is_open());
            let _ = state.close();
            assert!(!state.is_open());
        }
    }

    #[test]
    fn fuzz_keys_open() {
        let opts = sample_options();
        let mut state = SelectState::new().with_searchable(true);
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open(bounds, &opts);
        let keys = [
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key, &opts, bounds);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let opts = sample_options();
        let mut state = SelectState::new().with_value("banana");
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        let _ = state.open(area, &opts);
        let w = Select::new(&opts, &system);
        for _ in 0..100 {
            w.paint_stacked(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic_register() {
        let system = DesignSystem::default();
        let opts = sample_options();
        let state = SelectState::new().with_value("apple");
        let mut scene = SemanticScene::<&str, ()>::default();
        Select::new(&opts, &system).register_semantic(
            &mut scene,
            "s",
            Rect::new(0, 0, 20, 1),
            &state,
        );
        assert!(scene.get(&"s").is_some());
    }
}
