// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Accordion — single- or multi-open disclosure groups.
//!
//! **Anatomy:** `root` · item[] (`trigger` · optional `content`).
//!
//! Built on [`super::Collapsible`] for per-item paint and
//! [`crate::interaction::RovingFocusGroup`] for active-descendant navigation.
//! **Navigation focus** (`roving` cursor) is independent of **expanded** state
//! (`open` set). Host paints large bodies into content rects (often via
//! [`super::ScrollArea`]).
//!
//! **Controlled vs uncontrolled.** Pass [`Accordion::open_ids`] each frame for
//! controlled expansion; omit it and [`AccordionState`] owns the open set.
//!
//! Recipes: section, settings, logs, FAQ.
//!
//! References: Radix Accordion, mutual collapsibles, settings/help TUIs.
#![allow(unused_variables, unused_mut)] // unit-test fixtures
use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use crate::interaction::{
    EventResult, HitRegion, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome,
    UiIntent, default_button_intent, default_list_intent,
};
use crate::style::DesignSystem;
use crate::widgets::collapsible::{
    CollapsedContentPolicy, Collapsible, CollapsibleState, CollapsibleVariant,
};

/// How many items may be expanded at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AccordionMode {
    /// At most one open (opening another closes the rest).
    #[default]
    Single,
    /// Any subset may be open.
    Multiple,
}

impl AccordionMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }
}

/// Visual / product recipes for common surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AccordionRecipe {
    /// Editorial section groups (multi-open, section trigger).
    #[default]
    Section,
    /// Settings pages (single-open, section trigger).
    Settings,
    /// Log / tool streams (multi-open, keep-mounted, inline).
    Logs,
    /// FAQ / help (single-open, section trigger).
    Faq,
}

impl AccordionRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Section => "section",
            Self::Settings => "settings",
            Self::Logs => "logs",
            Self::Faq => "faq",
        }
    }

    /// Default mode for this recipe.
    #[must_use]
    pub const fn mode(self) -> AccordionMode {
        match self {
            Self::Section | Self::Logs => AccordionMode::Multiple,
            Self::Settings | Self::Faq => AccordionMode::Single,
        }
    }

    /// Collapsible paint variant.
    #[must_use]
    pub const fn variant(self) -> CollapsibleVariant {
        match self {
            Self::Logs => CollapsibleVariant::Inline,
            Self::Section | Self::Settings | Self::Faq => CollapsibleVariant::Section,
        }
    }

    /// Child state policy while closed.
    #[must_use]
    pub const fn content_policy(self) -> CollapsedContentPolicy {
        match self {
            Self::Logs => CollapsedContentPolicy::KeepMounted,
            _ => CollapsedContentPolicy::Unmount,
        }
    }
}

/// One accordion item (content body is host-owned).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccordionItem<'a, Id> {
    /// Stable identity (survives insert/remove/reorder).
    pub id: Id,
    /// Trigger label.
    pub trigger: &'a str,
    /// Preferred content height when open (0 = flexible share of remaining).
    pub content_height: u16,
    /// Disabled: not activatable; skipped by roving / typeahead.
    pub disabled: bool,
}

impl<'a, Id> AccordionItem<'a, Id> {
    /// Enabled item with flexible content height.
    #[must_use]
    pub const fn new(id: Id, trigger: &'a str) -> Self {
        Self {
            id,
            trigger,
            content_height: 0,
            disabled: false,
        }
    }

    /// Preferred body rows when open.
    #[must_use]
    pub const fn content_height(mut self, rows: u16) -> Self {
        self.content_height = rows;
        self
    }

    /// Disabled item.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Geometry for one item after layout/paint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionItemParts<Id> {
    /// Item id.
    pub id: Id,
    /// Full item band (trigger + content).
    pub root: Rect,
    /// Trigger hit target.
    pub trigger: Rect,
    /// Content body (zero height when closed). Host may nest [`super::ScrollArea`].
    pub content: Rect,
    /// Whether expanded this frame.
    pub open: bool,
    /// Whether disabled.
    pub disabled: bool,
}

/// Full accordion geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionParts<Id> {
    /// Outer allocation.
    pub root: Rect,
    /// Per-item bands in visual order.
    pub items: Vec<AccordionItemParts<Id>>,
}

impl<Id> AccordionParts<Id> {
    /// Content rect for id (if any).
    #[must_use]
    pub fn content_of(&self, id: &Id) -> Option<Rect>
    where
        Id: PartialEq,
    {
        self.items.iter().find(|i| &i.id == id).map(|i| i.content)
    }
}

/// Typed outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccordionOutcome<Id> {
    /// No change.
    Ignored,
    /// Roving cursor moved (navigation focus only).
    CursorMoved {
        /// Previous active.
        from: Option<Id>,
        /// New active.
        to: Option<Id>,
    },
    /// Item opened (expanded).
    Opened {
        /// Item id.
        id: Id,
    },
    /// Item closed (collapsed).
    Closed {
        /// Item id.
        id: Id,
    },
    /// Single-mode open that closed others; reports final open id.
    ExclusiveOpened {
        /// Newly open id.
        id: Id,
        /// Ids closed as a side effect.
        closed: Vec<Id>,
    },
}

impl<Id> AccordionOutcome<Id> {
    /// Whether navigation or expansion changed.
    #[must_use]
    pub const fn changed(&self) -> bool {
        !matches!(self, Self::Ignored)
    }
}

/// Interaction + uncontrolled open set + roving cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccordionState<Id> {
    /// Host set true when the accordion surface owns keyboard input.
    pub surface_focused: bool,
    /// Active-descendant among items (not expansion).
    pub roving: RovingFocusGroup<Id>,
    /// Uncontrolled open ids (ignored when paint is controlled).
    open: Vec<Id>,
    /// Cached parts from last paint.
    pub parts: Option<AccordionParts<Id>>,
    /// Trigger hit regions from last paint.
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for AccordionState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> AccordionState<Id> {
    /// Unfocused, empty open set, vertical wrapping roving.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surface_focused: false,
            roving: RovingFocusGroup::new()
                .orientation(RovingOrientation::Vertical)
                .wrap(true),
            open: Vec::new(),
            parts: None,
            regions: Vec::new(),
        }
    }

    /// Surface keyboard ownership.
    pub const fn set_surface_focused(&mut self, focused: bool) {
        self.surface_focused = focused;
    }

    /// Active descendant (navigation focus).
    #[must_use]
    pub const fn cursor(&self) -> Option<&Id> {
        self.roving.active()
    }

    /// Uncontrolled open ids (borrowed).
    #[must_use]
    pub fn open_ids(&self) -> &[Id] {
        &self.open
    }
}

impl<Id: Clone + PartialEq> AccordionState<Id> {
    /// Sets roving cursor.
    pub fn set_cursor(&mut self, id: Option<Id>) {
        self.roving.set_active(id);
    }

    /// Whether id is open in the uncontrolled store.
    #[must_use]
    pub fn is_open(&self, id: &Id) -> bool {
        self.open.iter().any(|x| x == id)
    }

    /// Replace uncontrolled open set.
    pub fn set_open_ids(&mut self, ids: impl IntoIterator<Item = Id>) {
        self.open = ids.into_iter().collect();
    }

    /// Seed initially open ids (builder-style).
    #[must_use]
    pub fn initially_open(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.open = ids.into_iter().collect();
        self
    }

    /// Drop open ids not present in the current item list (dynamic remove).
    pub fn reconcile_open(&mut self, items: &[AccordionItem<'_, Id>]) {
        self.open.retain(|id| items.iter().any(|i| &i.id == id));
    }

    fn roving_entries(items: &[AccordionItem<'_, Id>]) -> Vec<RovingEntry<Id>> {
        items
            .iter()
            .map(|i| RovingEntry::new(i.id.clone(), i.trigger.to_string()).enabled(!i.disabled))
            .collect()
    }

    fn resolved_open(&self, id: &Id, controlled: Option<&[Id]>) -> bool {
        controlled
            .map(|ids| ids.iter().any(|x| x == id))
            .unwrap_or_else(|| self.is_open(id))
    }

    fn apply_open_set(
        &mut self,
        id: Id,
        open: bool,
        mode: AccordionMode,
        controlled: Option<&[Id]>,
    ) -> AccordionOutcome<Id> {
        let currently = self.resolved_open(&id, controlled);
        if controlled.is_some() {
            // Host applies; emit desired change only (single mode lists peers to close).
            if open {
                if currently && matches!(mode, AccordionMode::Single) {
                    // already exclusive
                    if controlled
                        .map(|ids| ids.len() == 1 && ids.iter().any(|x| x == &id))
                        .unwrap_or(false)
                    {
                        return AccordionOutcome::Ignored;
                    }
                } else if currently {
                    return AccordionOutcome::Ignored;
                }
                if matches!(mode, AccordionMode::Single) {
                    let closed: Vec<Id> = controlled
                        .unwrap_or(&[])
                        .iter()
                        .filter(|x| *x != &id)
                        .cloned()
                        .collect();
                    if closed.is_empty() {
                        return AccordionOutcome::Opened { id };
                    }
                    return AccordionOutcome::ExclusiveOpened { id, closed };
                }
                return AccordionOutcome::Opened { id };
            }
            if !currently {
                return AccordionOutcome::Ignored;
            }
            return AccordionOutcome::Closed { id };
        }
        if open {
            match mode {
                AccordionMode::Multiple => {
                    if currently {
                        return AccordionOutcome::Ignored;
                    }
                    self.open.push(id.clone());
                    AccordionOutcome::Opened { id }
                }
                AccordionMode::Single => {
                    if currently && self.open.len() == 1 {
                        return AccordionOutcome::Ignored;
                    }
                    let closed: Vec<Id> = self.open.iter().filter(|x| *x != &id).cloned().collect();
                    self.open.clear();
                    self.open.push(id.clone());
                    if closed.is_empty() {
                        AccordionOutcome::Opened { id }
                    } else {
                        AccordionOutcome::ExclusiveOpened { id, closed }
                    }
                }
            }
        } else if currently {
            self.open.retain(|x| x != &id);
            AccordionOutcome::Closed { id }
        } else {
            AccordionOutcome::Ignored
        }
    }

    fn toggle_id(
        &mut self,
        id: Id,
        mode: AccordionMode,
        controlled: Option<&[Id]>,
    ) -> AccordionOutcome<Id> {
        let open = !self.resolved_open(&id, controlled);
        self.apply_open_set(id, open, mode, controlled)
    }
}

/// Accordion group widget.
#[derive(Debug, Clone)]
pub struct Accordion<'a, Id> {
    items: &'a [AccordionItem<'a, Id>],
    system: &'a DesignSystem,
    mode: AccordionMode,
    recipe: AccordionRecipe,
    /// Controlled open ids for this frame; `None` → state owns open set.
    controlled_open: Option<&'a [Id]>,
    content_policy: CollapsedContentPolicy,
    variant: CollapsibleVariant,
    /// Cap each open body height (0 = no extra cap beyond preferred/share).
    max_content_height: u16,
}

impl<'a, Id> Accordion<'a, Id> {
    /// Accordion over borrowed items (section recipe defaults).
    #[must_use]
    pub fn new(items: &'a [AccordionItem<'a, Id>], system: &'a DesignSystem) -> Self {
        let recipe = AccordionRecipe::Section;
        Self {
            items,
            system,
            mode: recipe.mode(),
            recipe,
            controlled_open: None,
            content_policy: recipe.content_policy(),
            variant: recipe.variant(),
            max_content_height: 0,
        }
    }

    /// Section recipe (multi, section triggers).
    #[must_use]
    pub fn section(items: &'a [AccordionItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(items, system).recipe(AccordionRecipe::Section)
    }

    /// Settings recipe (single, section triggers).
    #[must_use]
    pub fn settings(items: &'a [AccordionItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(items, system).recipe(AccordionRecipe::Settings)
    }

    /// Logs recipe (multi, keep-mounted, inline).
    #[must_use]
    pub fn logs(items: &'a [AccordionItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(items, system).recipe(AccordionRecipe::Logs)
    }

    /// FAQ recipe (single, section triggers).
    #[must_use]
    pub fn faq(items: &'a [AccordionItem<'a, Id>], system: &'a DesignSystem) -> Self {
        Self::new(items, system).recipe(AccordionRecipe::Faq)
    }

    /// Recipe presets (mode/variant/policy); explicit setters still override.
    #[must_use]
    pub fn recipe(mut self, recipe: AccordionRecipe) -> Self {
        self.recipe = recipe;
        self.mode = recipe.mode();
        self.variant = recipe.variant();
        self.content_policy = recipe.content_policy();
        self
    }

    /// Open mode.
    #[must_use]
    pub const fn mode(mut self, mode: AccordionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Single-open.
    #[must_use]
    pub const fn single(mut self) -> Self {
        self.mode = AccordionMode::Single;
        self
    }

    /// Multi-open.
    #[must_use]
    pub const fn multiple(mut self) -> Self {
        self.mode = AccordionMode::Multiple;
        self
    }

    /// Controlled open ids this frame.
    #[must_use]
    pub const fn open_ids(mut self, ids: &'a [Id]) -> Self {
        self.controlled_open = Some(ids);
        self
    }

    /// Content policy while closed.
    #[must_use]
    pub const fn content_policy(mut self, policy: CollapsedContentPolicy) -> Self {
        self.content_policy = policy;
        self
    }

    /// Keep child domain state while closed.
    #[must_use]
    pub const fn keep_mounted(mut self) -> Self {
        self.content_policy = CollapsedContentPolicy::KeepMounted;
        self
    }

    /// Trigger paint variant.
    #[must_use]
    pub const fn variant(mut self, variant: CollapsibleVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Cap open body height (scroll host for overflow).
    #[must_use]
    pub const fn max_content_height(mut self, rows: u16) -> Self {
        self.max_content_height = rows;
        self
    }

    /// Mode.
    #[must_use]
    pub const fn open_mode(&self) -> AccordionMode {
        self.mode
    }

    /// Recipe id.
    #[must_use]
    pub const fn recipe_kind(&self) -> AccordionRecipe {
        self.recipe
    }
}

impl<'a, Id: Clone + PartialEq> Accordion<'a, Id> {
    /// Layout without painting.
    #[must_use]
    pub fn layout(&self, area: Rect, state: &AccordionState<Id>) -> AccordionParts<Id> {
        if area.is_empty() || self.items.is_empty() {
            return AccordionParts {
                root: area,
                items: Vec::new(),
            };
        }

        let n = self.items.len();
        let trigger_rows = u16::try_from(n).unwrap_or(u16::MAX).min(area.height);
        // How many triggers fit (one row each, top-down).
        let visible_triggers = usize::from(trigger_rows);
        let open_flags: Vec<bool> = self
            .items
            .iter()
            .map(|i| state.resolved_open(&i.id, self.controlled_open))
            .collect();

        // Content budget after all fitted triggers.
        let mut remaining = area.height.saturating_sub(trigger_rows);
        // Preferred heights for open items that fit.
        let mut preferred: Vec<u16> = vec![0; n];
        let mut flexible = 0u16;
        for (i, item) in self.items.iter().enumerate().take(visible_triggers) {
            if !open_flags[i] {
                continue;
            }
            let mut h = item.content_height;
            if self.max_content_height > 0 {
                h = if h == 0 {
                    self.max_content_height
                } else {
                    h.min(self.max_content_height)
                };
            }
            preferred[i] = h;
            if h == 0 {
                flexible = flexible.saturating_add(1);
            }
        }

        // Assign fixed preferred first.
        let mut assigned: Vec<u16> = vec![0; n];
        for i in 0..visible_triggers {
            if open_flags[i] && preferred[i] > 0 {
                let take = preferred[i].min(remaining);
                assigned[i] = take;
                remaining = remaining.saturating_sub(take);
            }
        }
        // Share rest among flexible open items.
        if flexible > 0 && remaining > 0 {
            let each = remaining / flexible;
            let mut extra = remaining % flexible;
            for i in 0..visible_triggers {
                if open_flags[i] && preferred[i] == 0 {
                    let mut h = each;
                    if extra > 0 {
                        h = h.saturating_add(1);
                        extra = extra.saturating_sub(1);
                    }
                    if self.max_content_height > 0 {
                        h = h.min(self.max_content_height);
                    }
                    assigned[i] = h;
                }
            }
        }

        let mut items = Vec::with_capacity(n);
        let mut y = area.y;
        let bottom = area.bottom();
        for (i, item) in self.items.iter().enumerate() {
            if y >= bottom {
                // No room: zero-height stubs for remaining items.
                items.push(AccordionItemParts {
                    id: item.id.clone(),
                    root: Rect {
                        x: area.x,
                        y: bottom,
                        width: area.width,
                        height: 0,
                    },
                    trigger: Rect {
                        x: area.x,
                        y: bottom,
                        width: area.width,
                        height: 0,
                    },
                    content: Rect {
                        x: area.x,
                        y: bottom,
                        width: area.width,
                        height: 0,
                    },
                    open: open_flags[i],
                    disabled: item.disabled,
                });
                continue;
            }
            let trigger_h = 1u16.min(bottom.saturating_sub(y));
            let trigger = Rect {
                x: area.x,
                y,
                width: area.width,
                height: trigger_h,
            };
            y = y.saturating_add(trigger_h);
            let content_h = if open_flags[i] && i < visible_triggers {
                assigned[i].min(bottom.saturating_sub(y))
            } else {
                0
            };
            let content = Rect {
                x: area.x,
                y,
                width: area.width,
                height: content_h,
            };
            y = y.saturating_add(content_h);
            let root = Rect {
                x: area.x,
                y: trigger.y,
                width: area.width,
                height: trigger.height.saturating_add(content.height),
            };
            items.push(AccordionItemParts {
                id: item.id.clone(),
                root,
                trigger,
                content,
                open: open_flags[i],
                disabled: item.disabled,
            });
        }

        AccordionParts { root: area, items }
    }

    /// Paint triggers; returns parts (host paints open bodies into content rects).
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut AccordionState<Id>,
    ) -> AccordionParts<Id> {
        state.reconcile_open(self.items);
        let parts = self.layout(area, state);
        state.regions.clear();

        let entries = AccordionState::roving_entries(self.items);
        let _ = state.roving.reconcile(&entries);

        for (item, band) in self.items.iter().zip(parts.items.iter()) {
            if band.trigger.is_empty() {
                continue;
            }
            let focused =
                state.surface_focused && state.roving.active().is_some_and(|a| a == &item.id);
            let mut col_state = CollapsibleState::new().initially_open(band.open);
            col_state.set_focused(focused);

            let col = Collapsible::new(item.trigger, self.system)
                .open(band.open)
                .disabled(item.disabled)
                .content_policy(self.content_policy)
                .variant(self.variant)
                .preferred_content_height(band.content.height);
            // Paint only the trigger band (content painted by host).
            let _ = col.paint(band.trigger, buffer, &mut col_state);

            if !item.disabled && band.trigger.height > 0 {
                state.regions.push(HitRegion {
                    id: item.id.clone(),
                    area: band.trigger,
                });
            }
        }

        state.parts = Some(parts.clone());
        parts
    }

    /// Key path: requires surface focus. Roving + expand/collapse of active item.
    pub fn handle_key(
        &self,
        state: &mut AccordionState<Id>,
        key: KeyEvent,
    ) -> AccordionOutcome<Id> {
        if !state.surface_focused || !key.is_press() {
            return AccordionOutcome::Ignored;
        }
        state.reconcile_open(self.items);
        let entries = AccordionState::roving_entries(self.items);
        let _ = state.roving.reconcile(&entries);

        // Activate on Enter/Space (button map).
        if let Some(intent) = default_button_intent(key) {
            if matches!(intent, UiIntent::Activate | UiIntent::Submit) {
                return self.activate_cursor(state);
            }
        }
        // Expand / Collapse: arrow keys only (leave h/l free for typeahead).
        match key.code {
            KeyCode::Right => return self.set_cursor_open(state, true),
            KeyCode::Left => return self.set_cursor_open(state, false),
            _ => {}
        }
        // j/k / list Toggle when not already handled.
        if let Some(intent) = default_list_intent(key) {
            match intent {
                UiIntent::Toggle => return self.activate_cursor(state),
                UiIntent::Move(_) => {
                    return match state.roving.handle_intent(intent, &entries) {
                        RovingOutcome::Ignored => AccordionOutcome::Ignored,
                        RovingOutcome::ActiveChanged { from, to } => {
                            AccordionOutcome::CursorMoved { from, to }
                        }
                    };
                }
                _ => {}
            }
        }

        match state.roving.handle_key(key, &entries) {
            RovingOutcome::Ignored => AccordionOutcome::Ignored,
            RovingOutcome::ActiveChanged { from, to } => AccordionOutcome::CursorMoved { from, to },
        }
    }

    /// Intent path when host already mapped keys.
    pub fn handle_intent(
        &self,
        state: &mut AccordionState<Id>,
        intent: UiIntent,
    ) -> AccordionOutcome<Id> {
        if !state.surface_focused {
            return AccordionOutcome::Ignored;
        }
        state.reconcile_open(self.items);
        let entries = AccordionState::roving_entries(self.items);
        let _ = state.roving.reconcile(&entries);
        match intent {
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => self.activate_cursor(state),
            UiIntent::Expand => self.set_cursor_open(state, true),
            UiIntent::Collapse => self.set_cursor_open(state, false),
            other => match state.roving.handle_intent(other, &entries) {
                RovingOutcome::Ignored => AccordionOutcome::Ignored,
                RovingOutcome::ActiveChanged { from, to } => {
                    AccordionOutcome::CursorMoved { from, to }
                }
            },
        }
    }

    /// Key with EventResult.
    pub fn handle_key_result(
        &self,
        state: &mut AccordionState<Id>,
        key: KeyEvent,
    ) -> EventResult<AccordionOutcome<Id>> {
        match self.handle_key(state, key) {
            AccordionOutcome::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Mouse down on trigger: focus surface cursor + toggle.
    pub fn handle_mouse(
        &self,
        state: &mut AccordionState<Id>,
        event: MouseEvent,
    ) -> AccordionOutcome<Id> {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return AccordionOutcome::Ignored;
        }
        for region in &state.regions {
            if region.area.contains(event.position) {
                let id = region.id.clone();
                // Find disabled?
                if self
                    .items
                    .iter()
                    .find(|i| i.id == id)
                    .is_some_and(|i| i.disabled)
                {
                    return AccordionOutcome::Ignored;
                }
                state.surface_focused = true;
                state.roving.set_active(Some(id.clone()));
                return state.toggle_id(id, self.mode, self.controlled_open);
            }
        }
        AccordionOutcome::Ignored
    }

    fn activate_cursor(&self, state: &mut AccordionState<Id>) -> AccordionOutcome<Id> {
        let Some(id) = state.roving.active().cloned() else {
            return AccordionOutcome::Ignored;
        };
        if self
            .items
            .iter()
            .find(|i| i.id == id)
            .is_some_and(|i| i.disabled)
        {
            return AccordionOutcome::Ignored;
        }
        state.toggle_id(id, self.mode, self.controlled_open)
    }

    fn set_cursor_open(&self, state: &mut AccordionState<Id>, open: bool) -> AccordionOutcome<Id> {
        let Some(id) = state.roving.active().cloned() else {
            return AccordionOutcome::Ignored;
        };
        if self
            .items
            .iter()
            .find(|i| i.id == id)
            .is_some_and(|i| i.disabled)
        {
            return AccordionOutcome::Ignored;
        }
        let currently = state.resolved_open(&id, self.controlled_open);
        if currently == open {
            return AccordionOutcome::Ignored;
        }
        state.apply_open_set(id, open, self.mode, self.controlled_open)
    }

    /// Register each trigger as a focusable control (expanded on semantic state).
    pub fn register_semantic<Action>(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        area: Rect,
        state: &AccordionState<Id>,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        use crate::interaction::{SemanticNode, SemanticRole, SemanticState};
        let parts = self.layout(area, state);
        for (item, band) in self.items.iter().zip(parts.items.iter()) {
            if band.trigger.is_empty() {
                continue;
            }
            let on_cursor =
                state.surface_focused && state.roving.active().is_some_and(|a| a == &item.id);
            let _ = scene.register(
                SemanticNode::control(item.id.clone(), band.trigger)
                    .role(SemanticRole::Button)
                    .label(item.trigger)
                    .focusable(!item.disabled)
                    .state(SemanticState {
                        expanded: band.open,
                        // selected ≈ navigation focus (distinct from expanded)
                        selected: on_cursor,
                        ..Default::default()
                    }),
            );
        }
    }
}

impl<Id: Clone + PartialEq> Widget for &Accordion<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state = AccordionState::new();
        if let Some(ids) = self.controlled_open {
            state.set_open_ids(ids.iter().cloned());
        }
        let _ = self.paint(area, buffer, &mut state);
    }
}

impl<Id: Clone + PartialEq> Widget for Accordion<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::style::DesignSystem;

    fn items() -> Vec<AccordionItem<'static, &'static str>> {
        vec![
            AccordionItem::new("a", "Alpha").content_height(2),
            AccordionItem::new("b", "Beta").content_height(2),
            AccordionItem::new("c", "Gamma").content_height(2),
            AccordionItem::new("d", "Disabled").disabled(true),
        ]
    }

    #[test]
    fn single_mode_exclusive_open() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::settings(&items, &system);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        let out = acc.handle_intent(&mut state, UiIntent::Activate);
        assert!(matches!(out, AccordionOutcome::Opened { id: "a" }));
        assert!(state.is_open(&"a"));
        state.set_cursor(Some("b"));
        let out = acc.handle_intent(&mut state, UiIntent::Activate);
        match out {
            AccordionOutcome::ExclusiveOpened { id: "b", closed } => {
                assert!(closed.contains(&"a"));
            }
            AccordionOutcome::Opened { id: "b" } => {}
            other => panic!("unexpected {other:?}"),
        }
        assert!(state.is_open(&"b"));
        assert!(!state.is_open(&"a"));
    }

    #[test]
    fn multi_mode_allows_several() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system).multiple();
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        let _ = acc.handle_intent(&mut state, UiIntent::Activate);
        state.set_cursor(Some("b"));
        let _ = acc.handle_intent(&mut state, UiIntent::Activate);
        assert!(state.is_open(&"a") && state.is_open(&"b"));
    }

    #[test]
    fn focus_independent_of_open() {
        let mut state = AccordionState::new().initially_open(["a"]);
        state.set_cursor(Some("b"));
        assert!(state.is_open(&"a"));
        assert_eq!(state.cursor(), Some(&"b"));
        assert!(!state.is_open(&"b"));
    }

    #[test]
    fn home_end_roving() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::faq(&items, &system);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("b"));
        let out = acc.handle_key(&mut state, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert!(matches!(
            out,
            AccordionOutcome::CursorMoved { to: Some("a"), .. }
        ));
        let out = acc.handle_key(&mut state, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        // End skips disabled → last enabled is "c"
        assert!(matches!(
            out,
            AccordionOutcome::CursorMoved { to: Some("c"), .. }
        ));
    }

    #[test]
    fn typeahead_jumps() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        let out = acc.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE),
        );
        assert!(matches!(
            out,
            AccordionOutcome::CursorMoved { to: Some("b"), .. }
        ));
    }

    #[test]
    fn disabled_not_activated() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("d"));
        // reconcile should move off disabled
        let entries = AccordionState::roving_entries(&items);
        let _ = state.roving.reconcile(&entries);
        assert_ne!(state.cursor(), Some(&"d"));
    }

    #[test]
    fn controlled_does_not_mutate_store() {
        let system = DesignSystem::default();
        let items = items();
        let open: &[&str] = &[];
        let acc = Accordion::section(&items, &system).open_ids(open);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        let out = acc.handle_intent(&mut state, UiIntent::Activate);
        assert!(matches!(out, AccordionOutcome::Opened { id: "a" }));
        assert!(!state.is_open(&"a"));
    }

    #[test]
    fn controlled_single_lists_peers_to_close() {
        let system = DesignSystem::default();
        let items = items();
        let open: &[&str] = &["a"];
        let acc = Accordion::settings(&items, &system).open_ids(open);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("b"));
        let out = acc.handle_intent(&mut state, UiIntent::Activate);
        match out {
            AccordionOutcome::ExclusiveOpened { id: "b", closed } => {
                assert_eq!(closed, vec!["a"]);
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(state.open_ids().is_empty()); // still uncontrolled store empty
    }

    #[test]
    fn dynamic_remove_reconciles_open() {
        let mut state = AccordionState::new().initially_open(["a", "gone"]);
        let items = items();
        state.reconcile_open(&items);
        assert!(state.is_open(&"a"));
        assert!(!state.is_open(&"gone"));
    }

    #[test]
    fn dynamic_insert_keeps_cursor_when_valid() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system);
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("b"));
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        let _ = acc.paint(Rect::new(0, 0, 40, 12), &mut buf, &mut state);
        assert_eq!(state.cursor(), Some(&"b"));
        // insert new list with extra item — cursor still valid
        let more = vec![
            AccordionItem::new("a", "Alpha"),
            AccordionItem::new("z", "Zeta"),
            AccordionItem::new("b", "Beta"),
        ];
        let _ =
            Accordion::section(&more, &system).paint(Rect::new(0, 0, 40, 12), &mut buf, &mut state);
        assert_eq!(state.cursor(), Some(&"b"));
    }

    #[test]
    fn layout_open_content_and_closed_zero() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::logs(&items, &system);
        let state = AccordionState::new().initially_open(["a", "b"]);
        let parts = acc.layout(Rect::new(0, 0, 40, 20), &state);
        let a = parts.items.iter().find(|i| i.id == "a").unwrap();
        let c = parts.items.iter().find(|i| i.id == "c").unwrap();
        assert!(a.open && a.content.height == 2);
        assert!(!c.open && c.content.height == 0);
    }

    #[test]
    fn max_content_height_caps_body() {
        let system = DesignSystem::default();
        let items = vec![AccordionItem::new("a", "A").content_height(20)];
        let acc = Accordion::section(&items, &system).max_content_height(3);
        let state = AccordionState::new().initially_open(["a"]);
        let parts = acc.layout(Rect::new(0, 0, 30, 30), &state);
        assert_eq!(parts.items[0].content.height, 3);
    }

    #[test]
    fn narrow_layout_safe() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::faq(&items, &system);
        let mut state = AccordionState::new().initially_open(["a"]);
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 4));
        let parts = acc.paint(Rect::new(0, 0, 12, 4), &mut buf, &mut state);
        assert!(!parts.items.is_empty());
        // triggers still painted in tiny width
        assert!(buf[(0, 0)].symbol() != " ");
    }

    #[test]
    fn mouse_toggles_and_focuses() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system).multiple();
        let mut state = AccordionState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 12));
        let parts = acc.paint(Rect::new(0, 0, 40, 12), &mut buf, &mut state);
        let trigger = parts.items[1].trigger;
        let out = acc.handle_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: ratatui_core::layout::Position {
                    x: trigger.x,
                    y: trigger.y,
                },
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(matches!(out, AccordionOutcome::Opened { id: "b" }));
        assert_eq!(state.cursor(), Some(&"b"));
        assert!(state.surface_focused);
    }

    #[test]
    fn expand_collapse_intents() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system).multiple();
        let mut state = AccordionState::new();
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        assert!(matches!(
            acc.handle_intent(&mut state, UiIntent::Expand),
            AccordionOutcome::Opened { id: "a" }
        ));
        assert!(matches!(
            acc.handle_intent(&mut state, UiIntent::Collapse),
            AccordionOutcome::Closed { id: "a" }
        ));
    }

    #[test]
    fn recipe_ids_stable() {
        assert_eq!(AccordionRecipe::Faq.id(), "faq");
        assert_eq!(AccordionRecipe::Logs.mode(), AccordionMode::Multiple);
        assert_eq!(
            AccordionRecipe::Logs.content_policy(),
            CollapsedContentPolicy::KeepMounted
        );
    }

    #[test]
    fn semantic_registers_expanded_and_focus() {
        use crate::interaction::SemanticScene;
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system);
        let mut state = AccordionState::new().initially_open(["a"]);
        state.set_surface_focused(true);
        state.set_cursor(Some("a"));
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        acc.register_semantic(&mut scene, Rect::new(0, 0, 40, 12), &state);
        assert!(scene.len() >= 3);
        let a = scene.nodes().iter().find(|n| n.id == "a").unwrap();
        assert!(a.state.expanded);
        assert!(a.state.selected); // navigation focus ≠ expanded-only
    }

    #[test]
    fn layout_is_cheap() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::logs(&items, &system).max_content_height(5);
        let state = AccordionState::new().initially_open(["a", "b"]);
        let area = Rect::new(0, 0, 60, 40);
        for _ in 0..20_000 {
            let _ = acc.layout(area, &state);
        }
    }

    #[test]
    fn empty_area_safe() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system);
        let mut state = AccordionState::new();
        let mut buf = Buffer::empty(Rect::new(0, 0, 1, 1));
        let parts = acc.paint(Rect::new(0, 0, 0, 0), &mut buf, &mut state);
        assert!(parts.items.is_empty() || parts.root.is_empty());
    }

    #[test]
    fn content_of_helper() {
        let system = DesignSystem::default();
        let items = items();
        let acc = Accordion::section(&items, &system);
        let state = AccordionState::new().initially_open(["b"]);
        let parts = acc.layout(Rect::new(0, 0, 40, 16), &state);
        assert!(parts.content_of(&"b").is_some_and(|r| r.height > 0));
        assert!(parts.content_of(&"c").is_some_and(|r| r.height == 0));
    }
}
