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
use ratatui_core::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::{block::Block, borders::Borders};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent,
    },
    style::{ControlState, DesignSystem, Glyph, ListRowVisualState, Role, VisualState},
    text::{display_cols, take_display_cols, truncate_cols},
};

use super::{Surface, SurfaceRecipe, TextInput, TextInputOutcome, TextInputState, Validation};

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
            collection: CollectionState::new().wrap(false),
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
    pub fn collection_items<'a>(options: &'a [SelectOption<Id>]) -> Vec<CollectionItem<'a, Id>> {
        options
            .iter()
            .filter(|o| o.is_option())
            .map(|o| CollectionItem::new(o.id.clone(), &o.label).enabled(!o.disabled))
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

    fn filtered_collection_items<'a>(
        options: &'a [SelectOption<Id>],
        query: &str,
    ) -> Vec<CollectionItem<'a, Id>> {
        if query.trim().is_empty() {
            return Self::collection_items(options);
        }
        Self::filter_options(options, query)
            .into_iter()
            .filter(|o| o.is_option())
            .map(|o| CollectionItem::new(o.id.clone(), &o.label).enabled(!o.disabled))
            .collect()
    }

    fn current_collection_items<'a>(
        &self,
        options: &'a [SelectOption<Id>],
    ) -> Vec<CollectionItem<'a, Id>> {
        if self.searchable {
            Self::filtered_collection_items(options, self.search.value())
        } else {
            Self::collection_items(options)
        }
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

    /// Cycle the committed value while closed (junie Up/Left Down/Right).
    fn cycle_closed_value(&mut self, options: &[SelectOption<Id>], delta: i16) -> SelectOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled {
            return SelectOutcome::Ignored;
        }
        let enabled: Vec<Id> = options
            .iter()
            .filter(|option| option.is_option() && !option.disabled)
            .map(|option| option.id.clone())
            .collect();
        if enabled.is_empty() {
            return SelectOutcome::Ignored;
        }
        let current = self
            .value
            .as_ref()
            .and_then(|id| enabled.iter().position(|candidate| candidate == id));
        let next = match current {
            Some(index) if delta < 0 => index.saturating_sub(1),
            Some(index) => (index + 1).min(enabled.len() - 1),
            None if delta < 0 => enabled.len() - 1,
            None => 0,
        };
        let id = enabled[next].clone();
        if self.value.as_ref() == Some(&id) {
            return SelectOutcome::Ignored;
        }
        self.value = Some(id.clone());
        self.collection.set_active(Some(id.clone()));
        SelectOutcome::ValueChanged { id }
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
            self.search.set_editing(true);
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
        self.search.set_editing(false);
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
        self.search.set_editing(false);
        let _ = self.search.clear();
        SelectOutcome::ValueChanged { id }
    }

    /// Reconcile after option list changes while open.
    pub fn reconcile_options(&mut self, options: &[SelectOption<Id>]) {
        let items = self.current_collection_items(options);
        let _ = self.collection.reconcile(&items);
    }

    /// Key adapter. Pass full option set; filtering applied when searchable.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        options: &[SelectOption<Id>],
        bounds: Rect,
    ) -> SelectOutcome<Id> {
        if key.is_release() || !self.enabled {
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
            KeyCode::Down | KeyCode::Right if key.modifiers.is_empty() => {
                self.cycle_closed_value(options, 1)
            }
            KeyCode::Up | KeyCode::Left if key.modifiers.is_empty() => {
                self.cycle_closed_value(options, -1)
            }
            KeyCode::Esc => SelectOutcome::Ignored,
            // junie closed select: j/k ignored (arrows cycle; Enter/Space open).
            KeyCode::Char('j' | 'k' | 'J' | 'K') => SelectOutcome::Ignored,
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

        let items = self.current_collection_items(options);

        // junie open select: j/k move the cursor, not typeahead.
        if !self.searchable && matches!(key.code, KeyCode::Char('j' | 'J' | 'k' | 'K')) {
            let dir = if matches!(key.code, KeyCode::Char('j' | 'J')) {
                1
            } else {
                -1
            };
            return match self.collection.move_by(&items, dir) {
                CollectionOutcome::ActiveChanged { to, .. } => {
                    SelectOutcome::HighlightChanged { id: to }
                }
                CollectionOutcome::Scrolled => SelectOutcome::Changed,
                CollectionOutcome::Ignored => SelectOutcome::Ignored,
            };
        }

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
                let items = self.current_collection_items(options);
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
    help: &'a str,
    validation: Validation<'a>,
}

impl<'a, Id> Select<'a, Id> {
    /// Select over options.
    #[must_use]
    pub const fn new(options: &'a [SelectOption<Id>], system: &'a DesignSystem) -> Self {
        Self {
            options,
            system,
            placeholder: "Select",
            label: "",
            help: "",
            validation: Validation::Valid,
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

    /// Muted help under the trigger (source Select `help`, origin `area.x + 2`).
    #[must_use]
    pub const fn help(mut self, help: &'a str) -> Self {
        self.help = help;
        self
    }

    /// Validation.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
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
        let formish = !matches!(state.recipe, SelectRecipe::Compact)
            && (matches!(state.recipe, SelectRecipe::Form) || !self.label.is_empty());
        if formish && area.height >= 2 && !self.label.is_empty() {
            // Source `Theme::label(focused)`: secondary idle, title when focused.
            let theme = self.system.junie_theme();
            let mut style = if !state.enabled {
                Style::new().fg(theme.text_faint)
            } else if state.focused || state.is_open() {
                theme.title()
            } else {
                theme.secondary()
            };
            if state.focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            // Source Select: label at `area.x + 2` (gutter column stays empty).
            buffer.set_stringn(
                area.x.saturating_add(2),
                y,
                take_display_cols(self.label, usize::from(area.width.saturating_sub(2))),
                usize::from(area.width.saturating_sub(2)),
                style,
            );
            y = y.saturating_add(1);
        }

        let trigger = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            1.min(area.height.saturating_sub(y.saturating_sub(area.y))),
        );
        state.trigger = trigger;
        self.paint_trigger(trigger, buffer, state);

        // Help sits under the field. Source then paints the popup on top of
        // the rest of the screen (`place(Below)`), covering the help row.
        if trigger.y.saturating_add(1) < area.bottom() && !state.is_open() {
            match self.validation {
                Validation::Invalid(msg) => {
                    crate::widgets::field_message::paint_field_message(
                        buffer,
                        Rect::new(area.x, trigger.y.saturating_add(1), area.width, 1),
                        self.system,
                        crate::widgets::label::DescriptionKind::Error,
                        msg,
                    );
                }
                _ if !self.help.is_empty() => {
                    let help_x = area.x.saturating_add(2);
                    let help_w = area.width.saturating_sub(2);
                    let help = truncate_cols(
                        self.help,
                        usize::from(help_w),
                        self.system.glyphs.ellipsis(),
                    );
                    buffer.set_stringn(
                        help_x,
                        trigger.y.saturating_add(1),
                        help.as_ref(),
                        usize::from(help_w),
                        self.system.style(Role::TextMuted),
                    );
                }
                _ => {}
            }
        }

        if state.is_open() {
            let n = self.options.iter().filter(|o| o.is_option()).count() as u16;
            let h = n.saturating_add(2).min(10);
            let w = trigger.width.clamp(12, 40);
            let screen = *buffer.area();
            let pa = if matches!(state.presentation, SelectPresentation::Fullscreen)
                && !list_area.is_empty()
            {
                list_area
            } else {
                place_below(screen, trigger, w, h)
            };
            if !pa.is_empty() {
                state.panel = pa;
                self.paint_list(pa, buffer, state);
            }
        } else {
            state.panel = Rect::default();
        }
    }

    /// Convenience: paint trigger; if open, list fills remainder below trigger in `area`.
    pub fn paint_stacked(&self, area: Rect, buffer: &mut Buffer, state: &mut SelectState<Id>) {
        if !state.is_open() {
            self.paint(area, Rect::default(), buffer, state);
            return;
        }
        // Closed select is three rows (label, field, help) unless compact.
        let trigger_h = closed_select_height(state.recipe, !self.label.is_empty(), area.height);
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
            ControlState::Disabled
        } else if state.focused || state.is_open() {
            ControlState::Focused
        } else {
            ControlState::Default
        };
        let recipe = self.system.input_recipe(control_state, invalid, false);
        buffer.set_style(area, recipe.fill);
        // Prompt column is reserved in every state so the value does not shift
        // when focus arrives. Idle paints ▎ with fg=bg; focus makes it visible.
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

        let chev = self
            .system
            .glyphs
            .resolve(if state.is_open() {
                Glyph::ChevronUp
            } else {
                Glyph::ChevronDown
            })
            .text;
        // Source Select: gutter, pad, value, chevron at `right-2`.
        let text_x = area.x.saturating_add(2).min(area.right());
        let text_w = area.width.saturating_sub(5);
        let muted = state.value.is_none();
        buffer.set_stringn(
            text_x,
            area.y,
            take_display_cols(value_label, usize::from(text_w)),
            usize::from(text_w),
            if muted {
                recipe.placeholder
            } else {
                recipe.value
            },
        );
        if area.width > 1 {
            let theme = self.system.junie_theme();
            let chev_fg = if !state.enabled {
                theme.disabled
            } else {
                theme.text_secondary
            };
            let chev_bg = recipe.fill.bg.unwrap_or(theme.field);
            buffer.set_stringn(
                area.right().saturating_sub(2),
                area.y,
                chev,
                1,
                Style::new().fg(chev_fg).bg(chev_bg),
            );
        }
        apply_field_underline(buffer, area, &recipe);
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut SelectState<Id>) {
        let popover =
            matches!(state.presentation, SelectPresentation::Popover) && !state.searchable;
        let inner = if popover {
            paint_junie_popup_surface(buffer, area, self.system)
        } else {
            let recipe = if state.focused {
                SurfaceRecipe::OverlayFocused
            } else {
                SurfaceRecipe::Overlay
            };
            Surface::new(self.system)
                .recipe(recipe)
                .bordered(true)
                .content_inset()
                .paint(area, buffer)
        };
        if inner.is_empty() {
            return;
        }

        let mut list_top = inner.y;
        if state.searchable {
            let search_row = Rect::new(inner.x, inner.y, inner.width, 1);
            state.search_region = Some(search_row);
            state.search.set_focused(true);
            let _ = TextInput::new("", self.system).placeholder("Filter").paint(
                search_row,
                buffer,
                &mut state.search,
            );
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
        // Source Select popup has no scroll gutter. Reserve one only for
        // searchable/fullscreen lists.
        let gutter = Rect::new(
            full_list.right().saturating_sub(1),
            full_list.y,
            1,
            full_list.height,
        );
        let list_area = if popover {
            full_list
        } else {
            Rect::new(
                full_list.x,
                full_list.y,
                full_list.width.saturating_sub(1),
                full_list.height,
            )
        };
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
        let coll_items = state.current_collection_items(self.options);
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
                    let label = if let Some(desc) = &opt.description {
                        format!("{} {} {desc}", opt.label, { "—" })
                    } else {
                        opt.label.clone()
                    };
                    if popover {
                        paint_junie_select_row(
                            buffer,
                            rect,
                            self.system,
                            is_hi && state.focused,
                            is_val,
                            !opt.disabled,
                            &label,
                        );
                    } else {
                        let visual = ListRowVisualState {
                            selected: is_hi,
                            focused: is_hi && state.focused,
                            hovered: state.hovered.as_ref() == Some(&opt.id),
                            enabled: !opt.disabled,
                            loading: false,
                            checked: is_val,
                            ..ListRowVisualState::default()
                        };
                        paint_list_anatomy_row(buffer, rect, self.system, visual, is_val, &label);
                    }
                    if !opt.disabled {
                        state.option_regions.push((opt.id.clone(), rect));
                    }
                    row_y = row_y.saturating_add(1);
                    option_idx += 1;
                }
            }
        }

        if !popover {
            crate::scroll::paint_overflow_scrollbar(
                buffer,
                gutter,
                coll_items.len(),
                vp,
                u16::try_from(state.collection.offset()).unwrap_or(u16::MAX),
                state.focused,
                self.system,
            );
        }
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

/// Closed select occupies three rows (label, field, help) except compact.
fn closed_select_height(recipe: SelectRecipe, has_label: bool, available: u16) -> u16 {
    if matches!(recipe, SelectRecipe::Compact) {
        return 1.min(available);
    }
    if has_label && available >= 3 {
        3
    } else if available >= 3 {
        2
    } else {
        1.min(available)
    }
}

/// Source `ui/popup.rs` `surface()`: elevated fill, focused colour, no bold.
fn paint_junie_popup_surface(buffer: &mut Buffer, area: Rect, system: &DesignSystem) -> Rect {
    if area.is_empty() {
        return area;
    }
    let theme = system.junie_theme();
    buffer.set_style(area, Style::new().bg(theme.surface_elevated));
    Block::default()
        .borders(Borders::ALL)
        .border_style(system.style(Role::BorderFocused).bg(theme.surface_elevated))
        .border_set(system.border_set())
        .render(area, buffer);
    area.inner(Margin::new(1, 1))
}

/// Source Select popup row: `▎› label` (gutter + selected marker + text).
fn paint_junie_select_row(
    buffer: &mut Buffer,
    row: Rect,
    system: &DesignSystem,
    focused: bool,
    selected: bool,
    enabled: bool,
    label: &str,
) {
    if row.is_empty() {
        return;
    }
    let theme = system.junie_theme();
    let vis = VisualState {
        focused,
        selected,
        disabled: !enabled,
        ..VisualState::default()
    };
    let st = system.row(vis, theme.surface_elevated);
    buffer.set_style(row, st);
    buffer.set_stringn(
        row.x,
        row.y,
        system.glyphs.selection_gutter(),
        1,
        system.gutter(vis, st.bg.unwrap_or(theme.surface_elevated), false),
    );
    if selected && row.width > 1 {
        buffer.set_stringn(row.x.saturating_add(1), row.y, "›", 1, st.fg(theme.accent));
    }
    let text_x = row.x.saturating_add(3).min(row.right());
    let text_w = row.right().saturating_sub(text_x);
    if text_w > 0 {
        buffer.set_stringn(
            text_x,
            row.y,
            take_display_cols(label, usize::from(text_w)),
            usize::from(text_w),
            st,
        );
    }
}

/// Source `ui/popup.rs` `place(..., Placement::Below)`.
fn place_below(screen: Rect, anchor: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(screen.width).max(1);
    let height = height.min(screen.height).max(1);
    let below = anchor.bottom();
    let room_below = screen.bottom().saturating_sub(below);
    let y = if room_below >= height {
        below
    } else if anchor.y >= screen.y.saturating_add(height) {
        anchor.y.saturating_sub(height)
    } else {
        screen.bottom().saturating_sub(height)
    };
    let x = anchor
        .x
        .min(screen.right().saturating_sub(width))
        .max(screen.x);
    Rect::new(x, y, width, height)
}

fn apply_field_underline(buffer: &mut Buffer, field: Rect, recipe: &crate::style::InputRecipe) {
    if field.is_empty() {
        return;
    }
    let mut underline = Style::new().add_modifier(recipe.border.add_modifier);
    if let Some(color) = recipe.border.underline_color {
        underline = underline.underline_color(color);
    }
    buffer.set_style(field, underline);
}

/// List anatomy: `▎` in col0 (keyboard), `›` in col1 (chosen). Never `› ` as a gutter.
fn paint_list_anatomy_row(
    buffer: &mut Buffer,
    row: Rect,
    system: &DesignSystem,
    visual: ListRowVisualState,
    chosen: bool,
    label: &str,
) {
    if row.is_empty() {
        return;
    }
    let chrome = super::row_chrome::RowChrome::resolve(system, visual);
    let recipe = system.resolve_list_row(visual);
    let style = chrome.label_style(recipe.label);
    chrome.paint(buffer, row);
    let _ = chosen;
    let text_x = row.x.saturating_add(3).min(row.right());
    let text_w = row.right().saturating_sub(text_x);
    if text_w > 0 {
        buffer.set_stringn(
            text_x,
            row.y,
            take_display_cols(label, usize::from(text_w)),
            usize::from(text_w),
            style,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;
    use ratatui_core::layout::Position;

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
    fn closed_non_searchable_jk_are_ignored_plain_and_modified() {
        let opts = sample_options();
        let bounds = Rect::new(0, 0, 80, 24);
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);

        for (code, modifiers) in [
            (KeyCode::Char('j'), KeyModifiers::NONE),
            (KeyCode::Char('k'), KeyModifiers::SHIFT),
            (KeyCode::Char('J'), KeyModifiers::CONTROL),
            (KeyCode::Char('K'), KeyModifiers::ALT),
        ] {
            assert_eq!(
                state.handle_key(KeyEvent::new(code, modifiers), &opts, bounds),
                SelectOutcome::Ignored
            );
        }

        assert!(!state.is_open());
        assert_eq!(state.value(), Some(&"apple"));
        assert_eq!(state.highlight(), Some(&"apple"));
    }

    #[test]
    fn open_non_searchable_jk_move_highlight_with_bounds() {
        let opts = sample_options();
        let bounds = Rect::new(0, 0, 80, 24);
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let _ = state.open(bounds, &opts);

        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                &opts,
                bounds,
            ),
            SelectOutcome::HighlightChanged { id: Some("banana") }
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                &opts,
                bounds,
            ),
            SelectOutcome::HighlightChanged { id: Some("carrot") }
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
                &opts,
                bounds,
            ),
            SelectOutcome::Ignored
        );
        assert_eq!(state.highlight(), Some(&"carrot"));

        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                &opts,
                bounds,
            ),
            SelectOutcome::HighlightChanged { id: Some("banana") }
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                &opts,
                bounds,
            ),
            SelectOutcome::HighlightChanged { id: Some("apple") }
        );
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
                &opts,
                bounds,
            ),
            SelectOutcome::Ignored
        );
        assert_eq!(state.highlight(), Some(&"apple"));
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
        let items = SelectState::filtered_collection_items(&opts, "car");
        assert!(items.iter().any(|i| i.id == "carrot"));
        assert!(!items.iter().any(|i| i.id == "apple"));
    }

    #[test]
    fn semantic_navigation_stays_inside_filtered_options() {
        let opts = sample_options();
        let bounds = Rect::new(0, 0, 80, 24);
        let mut state = SelectState::new().with_searchable(true);
        state.set_focused(true);
        let _ = state.open(bounds, &opts);
        let _ = state.search.insert_str("car");
        state.reconcile_options(&opts);

        assert_eq!(state.highlight(), Some(&"carrot"));
        assert_eq!(
            state.handle_intent(
                UiIntent::Move(crate::interaction::NavigationMove::Next),
                &opts,
                bounds,
            ),
            SelectOutcome::Ignored
        );
        assert_eq!(state.highlight(), Some(&"carrot"));
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
        let system = DesignSystem::new(RolePalette::default());
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
    fn focused_trigger_reserves_prompt_before_value() {
        let system = DesignSystem::default();
        let opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let area = Rect::new(0, 0, 16, 1);
        let mut buffer = Buffer::empty(area);

        Select::new(&opts, &system).paint(area, Rect::default(), &mut buffer, &mut state);

        assert_ne!(buffer[(area.x, area.y)].symbol(), "A");
        assert_eq!(buffer[(area.x + 1, area.y)].symbol(), " ");
        assert_eq!(buffer[(area.x + 2, area.y)].symbol(), "A");
    }

    #[test]
    fn form_label_and_value_use_source_inset() {
        let system = DesignSystem::junie();
        let opts = sample_options();
        let mut state = SelectState::new()
            .with_recipe(SelectRecipe::Form)
            .with_value("apple");
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        Select::new(&opts, &system)
            .label("Fruit")
            .help("Applies to the next query")
            .paint(area, Rect::default(), &mut buffer, &mut state);
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(2, 0)].symbol(), "F");
        assert_eq!(buffer[(2, 1)].symbol(), "A");
        assert_eq!(buffer[(area.right() - 2, 1)].symbol(), "▾");
        assert_eq!(buffer[(2, 2)].symbol(), "A");
        assert_eq!(buffer[(3, 2)].symbol(), "p");
    }

    #[test]
    fn validation_does_not_move_the_open_menu() {
        let system = DesignSystem::default();
        let opts = sample_options();
        let area = Rect::new(0, 0, 24, 10);

        let mut valid = SelectState::new();
        valid.set_focused(true);
        let _ = valid.open(area, &opts);
        let mut valid_buffer = Buffer::empty(area);
        Select::new(&opts, &system).paint_stacked(area, &mut valid_buffer, &mut valid);

        let mut invalid = SelectState::new();
        invalid.set_focused(true);
        let _ = invalid.open(area, &opts);
        let mut invalid_buffer = Buffer::empty(area);
        Select::new(&opts, &system)
            .validation(Validation::Invalid("required"))
            .paint_stacked(area, &mut invalid_buffer, &mut invalid);

        assert_eq!(valid.panel.y, invalid.panel.y);
        assert_eq!(valid.panel.height, invalid.panel.height);
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

    fn row_text(buffer: &Buffer, y: u16, width: u16) -> String {
        (0..width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn closed_select_is_three_rows_with_disclosure() {
        let system = DesignSystem::default();
        let opts = sample_options();
        let mut state = SelectState::new()
            .with_recipe(SelectRecipe::Form)
            .with_value("apple");
        state.set_focused(true);
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        Select::new(&opts, &system)
            .label("Fruit")
            .paint_stacked(area, &mut buffer, &mut state);
        assert_eq!(state.trigger.height, 1);
        assert_eq!(state.trigger.y, 1);
        let field = row_text(&buffer, 1, area.width);
        assert!(
            field.contains('▾') || field.contains(system.glyphs.resolve(Glyph::ChevronDown).text),
            "closed disclosure: {field:?}"
        );
        assert_eq!(
            buffer[(state.trigger.x, state.trigger.y)].symbol(),
            system.glyphs.selection_gutter()
        );
        assert!(
            !buffer[(state.trigger.x + 1, state.trigger.y)]
                .style()
                .add_modifier
                .contains(Modifier::UNDERLINED),
            "closed focused select is not editing"
        );
    }

    #[test]
    fn closed_select_down_cycles_value_without_opening() {
        let opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let bounds = Rect::new(0, 0, 80, 24);
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &opts,
                bounds
            ),
            SelectOutcome::ValueChanged { id: "banana" }
        );
        assert!(!state.is_open());
        assert_eq!(state.value(), Some(&"banana"));
        assert_eq!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &opts,
                bounds
            ),
            SelectOutcome::Opened {
                presentation: SelectPresentation::Popover
            }
        );
        assert!(state.is_open());
    }

    #[test]
    fn overflowing_select_uses_overflow_thumb() {
        let system = DesignSystem::default();
        let opts: Vec<SelectOption<usize>> = (0..24)
            .map(|i| SelectOption::option(i, format!("opt-{i:02}")))
            .collect();
        let mut state = SelectState::new().with_value(0);
        state.set_focused(true);
        let area = Rect::new(0, 0, 32, 14);
        let _ = state.open(area, &opts);
        // Source popover has no scroll gutter. Thumb lives on fullscreen lists.
        state.set_presentation(SelectPresentation::Fullscreen);
        let mut buffer = Buffer::empty(area);
        Select::new(&opts, &system).paint_stacked(area, &mut buffer, &mut state);
        let thumb = crate::scroll::ScrollbarStyle::Line.vertical_thumb();
        let mut sb_x = None;
        for y in 0..area.height {
            for x in 0..area.width {
                if buffer[(x, y)].symbol() == thumb {
                    sb_x = Some(x);
                }
            }
        }
        let sb_x = sb_x.expect("overflowing select paints a thumb");
        let track = crate::scroll::SCROLLBAR_TRACK;
        let track_ys: Vec<u16> = (1..area.height.saturating_sub(1))
            .filter(|y| {
                let symbol = buffer[(sb_x, *y)].symbol();
                symbol == thumb || symbol == track
            })
            .collect();
        let viewport = track_ys.len();
        let (start, len) = crate::scroll::overflow_thumb(24, viewport, viewport, 0)
            .expect("24 options overflow the list viewport");
        let thumbs: Vec<u16> = track_ys
            .iter()
            .copied()
            .filter(|y| buffer[(sb_x, *y)].symbol() == thumb)
            .collect();
        assert_eq!(thumbs.len(), len);
        assert_eq!(thumbs[0], track_ys[start]);
    }

    #[test]
    fn open_popup_uses_list_anatomy_and_closes_outside() {
        let system = DesignSystem::default();
        let opts = sample_options();
        let mut state = SelectState::new().with_value("apple");
        state.set_focused(true);
        let area = Rect::new(0, 0, 32, 14);
        let mut buffer = Buffer::empty(area);
        let _ = state.open(area, &opts);
        Select::new(&opts, &system).paint_stacked(area, &mut buffer, &mut state);
        assert!(!state.option_regions.is_empty());
        let (_, rect) = state
            .option_regions
            .iter()
            .find(|(id, _)| *id == "apple")
            .cloned()
            .expect("chosen option painted");
        assert_eq!(
            buffer[(rect.x, rect.y)].symbol(),
            system.glyphs.selection_gutter(),
            "col0 is the focus bar"
        );
        assert_eq!(
            buffer[(rect.x + 1, rect.y)].symbol(),
            "›",
            "committed value is selected marker ›"
        );
        let field = row_text(&buffer, state.trigger.y, area.width);
        assert!(
            field.contains('▴') || field.contains(system.glyphs.resolve(Glyph::ChevronUp).text),
            "open disclosure: {field:?}"
        );
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(80, 40),
                    modifiers: KeyModifiers::NONE,
                },
                &opts,
                area,
            ),
            SelectOutcome::Closed
        );
        assert!(!state.is_open());
    }
}
