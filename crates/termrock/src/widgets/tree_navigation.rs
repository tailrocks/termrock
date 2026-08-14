// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Hierarchical **route** navigation — distinct from data [`Tree`](super::Tree).
//!
//! **Mission.** Project explorers, schema browsers, settings trees, and docs
//! nav need expansion, lazy children, typeahead, and a stable **route** that
//! survives filter/reload — without multi-select checkboxes or generic data
//! tree APIs.
//!
//! **vs [`Tree`](super::Tree).** Tree is a data hierarchy (optional multi-check,
//! composed rows). TreeNavigation is **route-oriented**: `route` ≠ focus,
//! active ancestors, lazy load generation, context actions.
//! **vs [`NavigationList`](super::NavigationList) / [`Sidebar`](super::Sidebar).**
//! Those are primary app rails; TreeNavigation is hierarchical content/nav
//! (often AppShell main or inspector).
//!
//! ## Arrow semantics (explorer style)
//!
//! | Key | Behavior |
//! |-----|----------|
//! | ↑/↓ | Move focus among visible rows |
//! | ← | Collapse expanded branch, else jump to parent |
//! | → | Expand collapsed branch / request lazy load, else first child |
//! | Enter | Set **route** if routeable; else toggle expand |
//! | Typeahead | Jump focus to label prefix match |
//!
//! Research: VS Code trees, file explorers, Yazi, broot, DB navigators.

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
        CollectionItem, CollectionOutcome, CollectionState, HitRegion, RovingOrientation,
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Indent columns per depth level (compact).
pub const TREE_NAV_INDENT: u16 = 2;
/// Depth at which indent clamps for narrow terminals.
pub const TREE_NAV_MAX_INDENT_DEPTH: u16 = 6;
/// Width under which labels truncate aggressively / chevrons only.
pub const TREE_NAV_NARROW_MAX_WIDTH: u16 = 16;

// ── Node model ──────────────────────────────────────────────────────────────

/// Status for a navigation tree node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TreeNavStatus {
    /// Ready.
    #[default]
    Ready,
    /// Children loading (lazy).
    Loading,
    /// Load / access error.
    Error,
    /// Unsaved / dirty leaf.
    Dirty,
    /// Soft warning.
    Warning,
}

impl TreeNavStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Error => "error",
            Self::Dirty => "dirty",
            Self::Warning => "warning",
        }
    }

    /// Non-color mark.
    #[must_use]
    pub const fn mark(self, ascii: bool) -> Option<&'static str> {
        match (self, ascii) {
            (Self::Ready, _) => None,
            (Self::Loading, true) => Some("..."),
            (Self::Loading, false) => Some("…"),
            (Self::Error, true) => Some("!"),
            (Self::Error, false) => Some("✗"),
            (Self::Dirty, true) => Some("."),
            (Self::Dirty, false) => Some("•"),
            (Self::Warning, true) => Some("?"),
            (Self::Warning, false) => Some("⚠"),
        }
    }
}

/// One visible row in a flattened tree projection (host owns hierarchy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNavNode<Id> {
    /// Stable id.
    pub id: Id,
    /// Parent id when known (enables parent jump + ancestors).
    pub parent: Option<Id>,
    /// Label.
    pub label: String,
    /// Depth (0 = root).
    pub depth: u16,
    /// Can expand/collapse.
    pub branch: bool,
    /// Currently expanded (host).
    pub expanded: bool,
    /// Enabled for focus/activate.
    pub enabled: bool,
    /// Can become the active **route** (files, pages; folders often false).
    pub routeable: bool,
    /// Children not loaded yet — Right/Enter may request load.
    pub lazy: bool,
    /// Badge text.
    pub badge: Option<String>,
    /// Status.
    pub status: TreeNavStatus,
    /// Optional icon (1–2 cols).
    pub icon: Option<String>,
}

impl<Id> TreeNavNode<Id> {
    /// Leaf routeable node.
    #[must_use]
    pub fn leaf(id: Id, label: impl Into<String>, depth: u16) -> Self {
        Self {
            id,
            parent: None,
            label: label.into(),
            depth,
            branch: false,
            expanded: false,
            enabled: true,
            routeable: true,
            lazy: false,
            badge: None,
            status: TreeNavStatus::Ready,
            icon: None,
        }
    }

    /// Branch (folder / group).
    #[must_use]
    pub fn branch(id: Id, label: impl Into<String>, depth: u16) -> Self {
        Self {
            id,
            parent: None,
            label: label.into(),
            depth,
            branch: true,
            expanded: false,
            enabled: true,
            routeable: false,
            lazy: false,
            badge: None,
            status: TreeNavStatus::Ready,
            icon: None,
        }
    }

    /// Parent.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Expanded.
    #[must_use]
    pub const fn expanded(mut self, on: bool) -> Self {
        self.expanded = on;
        self
    }

    /// Routeable leaf/branch.
    #[must_use]
    pub const fn routeable(mut self, on: bool) -> Self {
        self.routeable = on;
        self
    }

    /// Lazy children.
    #[must_use]
    pub const fn lazy(mut self, on: bool) -> Self {
        self.lazy = on;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Badge.
    #[must_use]
    pub fn badge(mut self, b: impl Into<String>) -> Self {
        self.badge = Some(b.into());
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: TreeNavStatus) -> Self {
        self.status = s;
        self
    }

    /// Icon.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// TreeNavigation outcomes. Host owns expansion lists and lazy I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TreeNavigationOutcome<Id> {
    /// No effect.
    Ignored,
    /// Chrome / scroll / filter.
    Changed,
    /// Focus moved (route unchanged).
    FocusChanged {
        /// Focused node.
        id: Option<Id>,
    },
    /// Active **route** changed.
    RouteChanged {
        /// Route id.
        id: Id,
    },
    /// Expand or collapse requested (host updates projection).
    ExpandToggled {
        /// Node.
        id: Id,
        /// Desired expanded state.
        expanded: bool,
    },
    /// Lazy children needed (`generation` race gate).
    LazyLoadRequested {
        /// Branch id.
        id: Id,
        /// Generation.
        generation: u64,
    },
    /// Context menu for node.
    ContextMenuRequested {
        /// Node.
        id: Id,
    },
    /// Typeahead jumped focus.
    TypeaheadMatched {
        /// Node.
        id: Id,
    },
    /// Esc / cancel.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state: route ≠ focus; filter; lazy generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNavigationState<Id> {
    /// Active route (destination).
    route: Option<Id>,
    /// Roving focus.
    collection: CollectionState<Id>,
    /// Filter query (host may filter; client also filters paint labels).
    filter: String,
    filter_active: bool,
    /// Lazy load generation.
    generation: u64,
    /// Last known ancestor ids of route (for paint).
    route_ancestors: Vec<Id>,
    focused: bool,
    enabled: bool,
    /// Typeahead buffer (cleared on non-type keys by collection/roving).
    typeahead: String,
    regions: Vec<HitRegion<Id>>,
    disclosure_regions: Vec<HitRegion<Id>>,
    root: Rect,
    narrow: bool,
}

impl<Id> Default for TreeNavigationState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id> TreeNavigationState<Id> {
    /// New tree nav with optional initial route.
    #[must_use]
    pub fn new(route: Option<Id>) -> Self {
        Self {
            route,
            collection: CollectionState::new()
                .wrap(false)
                .orientation(RovingOrientation::Vertical),
            filter: String::new(),
            filter_active: false,
            generation: 0,
            route_ancestors: Vec::new(),
            focused: false,
            enabled: true,
            typeahead: String::new(),
            regions: Vec::new(),
            disclosure_regions: Vec::new(),
            root: Rect::default(),
            narrow: false,
        }
    }

    /// Active route.
    #[must_use]
    pub const fn route(&self) -> Option<&Id> {
        self.route.as_ref()
    }

    /// Focused node.
    #[must_use]
    pub fn focus(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Filter.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Lazy generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Ancestors of current route (ids).
    #[must_use]
    pub fn route_ancestors(&self) -> &[Id] {
        &self.route_ancestors
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on {
            self.filter_active = false;
            self.typeahead.clear();
        }
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Set route; recomputes ancestors from projection when provided later.
    pub fn set_route(&mut self, id: Option<Id>) {
        self.route = id;
        self.route_ancestors.clear();
    }

    /// Set route and recompute ancestors from full projection.
    pub fn set_route_in(&mut self, id: Option<Id>, nodes: &[TreeNavNode<Id>])
    where
        Id: Clone + PartialEq,
    {
        self.route = id;
        self.recompute_ancestors(nodes);
    }

    /// Align focus to route if present in projection.
    pub fn focus_route(&mut self, nodes: &[TreeNavNode<Id>])
    where
        Id: Clone + PartialEq,
    {
        if let Some(r) = self.route.clone() {
            if nodes.iter().any(|n| n.id == r && n.enabled) {
                self.collection.set_active(Some(r));
            }
        }
    }

    fn recompute_ancestors(&mut self, nodes: &[TreeNavNode<Id>])
    where
        Id: Clone + PartialEq,
    {
        self.route_ancestors.clear();
        let Some(route) = self.route.clone() else {
            return;
        };
        let mut current = nodes
            .iter()
            .find(|n| n.id == route)
            .and_then(|n| n.parent.clone());
        let mut guard = 0u32;
        while let Some(pid) = current {
            if guard > 256 {
                break;
            }
            self.route_ancestors.push(pid.clone());
            current = nodes
                .iter()
                .find(|n| n.id == pid)
                .and_then(|n| n.parent.clone());
            guard += 1;
        }
        self.route_ancestors.reverse();
    }

    /// Whether id is an ancestor of the route.
    #[must_use]
    pub fn is_route_ancestor(&self, id: &Id) -> bool
    where
        Id: PartialEq,
    {
        self.route_ancestors.iter().any(|a| a == id)
    }

    /// Whether id is the active route.
    #[must_use]
    pub fn is_route(&self, id: &Id) -> bool
    where
        Id: PartialEq,
    {
        self.route.as_ref() == Some(id)
    }

    fn collection_items(nodes: &[TreeNavNode<Id>]) -> Vec<CollectionItem<Id>>
    where
        Id: Clone,
    {
        nodes
            .iter()
            .filter(|n| n.enabled || n.branch)
            .map(|n| {
                CollectionItem::new(n.id.clone(), n.label.clone()).enabled(n.enabled || n.branch)
            })
            .collect()
    }

    fn visible_filtered<'a>(&self, nodes: &'a [TreeNavNode<Id>]) -> Vec<&'a TreeNavNode<Id>> {
        let q = self.filter.to_ascii_lowercase();
        if q.is_empty() {
            return nodes.iter().collect();
        }
        nodes
            .iter()
            .filter(|n| n.label.to_ascii_lowercase().contains(&q))
            .collect()
    }

    /// Activate focused node as route (if routeable).
    pub fn activate_focus(&mut self, nodes: &[TreeNavNode<Id>]) -> TreeNavigationOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        let Some(id) = self.collection.active().cloned() else {
            return TreeNavigationOutcome::Ignored;
        };
        let Some(node) = nodes.iter().find(|n| n.id == id) else {
            return TreeNavigationOutcome::Ignored;
        };
        if !node.enabled {
            return TreeNavigationOutcome::Ignored;
        }
        if node.routeable {
            self.route = Some(id.clone());
            self.recompute_ancestors(nodes);
            return TreeNavigationOutcome::RouteChanged { id };
        }
        if node.branch {
            if node.lazy && !node.expanded {
                return self.request_lazy(id);
            }
            return TreeNavigationOutcome::ExpandToggled {
                id,
                expanded: !node.expanded,
            };
        }
        TreeNavigationOutcome::Ignored
    }

    fn request_lazy(&mut self, id: Id) -> TreeNavigationOutcome<Id> {
        self.generation = self.generation.saturating_add(1);
        TreeNavigationOutcome::LazyLoadRequested {
            id,
            generation: self.generation,
        }
    }

    /// Apply lazy result (race-safe). Host still updates nodes projection.
    pub fn apply_lazy_result(&mut self, generation: u64) -> bool {
        generation == self.generation
    }

    /// Preserve route across a new projection (filter/reload).
    ///
    /// If route still exists, keep it and recompute ancestors; if missing, keep
    /// route id (ghost) until host clears — focus moves to first visible.
    pub fn reconcile_route(&mut self, nodes: &[TreeNavNode<Id>])
    where
        Id: Clone + PartialEq,
    {
        self.recompute_ancestors(nodes);
        let coll = Self::collection_items(nodes);
        let _ = self.collection.reconcile(&coll);
        if let Some(r) = self.route.clone() {
            if nodes.iter().any(|n| n.id == r) {
                // keep focus near route if focus missing
                if self.collection.active().is_none() {
                    self.collection.set_active(Some(r));
                }
            }
        }
    }

    /// Key adapter.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        nodes: &[TreeNavNode<Id>],
    ) -> TreeNavigationOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return TreeNavigationOutcome::Ignored;
        }
        if !self.focused {
            return TreeNavigationOutcome::Ignored;
        }

        if self.filter_active {
            match key.code {
                KeyCode::Esc => {
                    self.filter_active = false;
                    return TreeNavigationOutcome::Changed;
                }
                KeyCode::Enter => {
                    self.filter_active = false;
                    return self.activate_focus(nodes);
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    return TreeNavigationOutcome::Changed;
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.filter.push(c);
                    return TreeNavigationOutcome::Changed;
                }
                _ => {}
            }
        }

        let coll = Self::collection_items(nodes);
        let _ = self.collection.reconcile(&coll);
        if self.collection.active().is_none() {
            if let Some(r) = self.route.clone() {
                if coll.iter().any(|c| c.id == r) {
                    self.collection.set_active(Some(r));
                }
            }
            if self.collection.active().is_none() {
                let _ = self.collection.move_first(&coll);
            }
        }

        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if !self.typeahead.is_empty() {
                self.typeahead.clear();
                return TreeNavigationOutcome::Changed;
            }
            return TreeNavigationOutcome::Cancelled;
        }

        // Filter
        if key.code == KeyCode::Char('/') && key.modifiers.is_empty() {
            self.filter_active = true;
            return TreeNavigationOutcome::Changed;
        }

        // Context menu
        if (key.code == KeyCode::Char(' ') && key.modifiers.contains(KeyModifiers::SHIFT))
            || (matches!(key.code, KeyCode::Char('m') | KeyCode::Char('M'))
                && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            if let Some(id) = self.collection.active().cloned() {
                return TreeNavigationOutcome::ContextMenuRequested { id };
            }
        }

        // Enter activate / toggle
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            self.typeahead.clear();
            return self.activate_focus(nodes);
        }

        // Left: collapse or parent
        if key.code == KeyCode::Left && key.modifiers.is_empty() {
            self.typeahead.clear();
            return self.collapse_or_parent(nodes);
        }

        // Right: expand / lazy / first child
        if key.code == KeyCode::Right && key.modifiers.is_empty() {
            self.typeahead.clear();
            return self.expand_or_child(nodes);
        }

        // Home / End
        if key.code == KeyCode::Home {
            self.typeahead.clear();
            let out = self.collection.move_first(&coll);
            return Self::map_focus(out);
        }
        if key.code == KeyCode::End {
            self.typeahead.clear();
            let out = self.collection.move_last(&coll);
            return Self::map_focus(out);
        }

        // Typeahead printable
        if let KeyCode::Char(c) = key.code {
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT)
                && !c.is_control()
                && c != '/'
            {
                self.typeahead.push(c);
                let needle = self.typeahead.to_ascii_lowercase();
                if let Some(node) = nodes.iter().find(|n| {
                    n.enabled && n.label.to_ascii_lowercase().starts_with(needle.as_str())
                }) {
                    self.collection.set_active(Some(node.id.clone()));
                    return TreeNavigationOutcome::TypeaheadMatched {
                        id: node.id.clone(),
                    };
                }
                return TreeNavigationOutcome::Changed;
            }
        }

        match self.collection.handle_key(key, &coll) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                self.typeahead.clear();
                TreeNavigationOutcome::FocusChanged { id: to }
            }
            CollectionOutcome::Scrolled => TreeNavigationOutcome::Changed,
            CollectionOutcome::Ignored => TreeNavigationOutcome::Ignored,
        }
    }

    fn map_focus(out: CollectionOutcome<Id>) -> TreeNavigationOutcome<Id> {
        match out {
            CollectionOutcome::ActiveChanged { to, .. } => {
                TreeNavigationOutcome::FocusChanged { id: to }
            }
            CollectionOutcome::Scrolled => TreeNavigationOutcome::Changed,
            CollectionOutcome::Ignored => TreeNavigationOutcome::Ignored,
        }
    }

    fn collapse_or_parent(&mut self, nodes: &[TreeNavNode<Id>]) -> TreeNavigationOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        let Some(id) = self.collection.active().cloned() else {
            return TreeNavigationOutcome::Ignored;
        };
        let Some(idx) = nodes.iter().position(|n| n.id == id) else {
            return TreeNavigationOutcome::Ignored;
        };
        let node = &nodes[idx];
        if node.branch && node.expanded {
            return TreeNavigationOutcome::ExpandToggled {
                id,
                expanded: false,
            };
        }
        // jump to parent
        if let Some(pid) = node.parent.clone() {
            if nodes.iter().any(|n| n.id == pid) {
                self.collection.set_active(Some(pid.clone()));
                return TreeNavigationOutcome::FocusChanged { id: Some(pid) };
            }
        }
        // depth-based parent if parent field missing
        let parent_idx = nodes[..idx]
            .iter()
            .rposition(|n| n.enabled && n.depth < node.depth);
        if let Some(pi) = parent_idx {
            let pid = nodes[pi].id.clone();
            self.collection.set_active(Some(pid.clone()));
            return TreeNavigationOutcome::FocusChanged { id: Some(pid) };
        }
        TreeNavigationOutcome::Ignored
    }

    fn expand_or_child(&mut self, nodes: &[TreeNavNode<Id>]) -> TreeNavigationOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        let Some(id) = self.collection.active().cloned() else {
            return TreeNavigationOutcome::Ignored;
        };
        let Some(idx) = nodes.iter().position(|n| n.id == id) else {
            return TreeNavigationOutcome::Ignored;
        };
        let node = &nodes[idx];
        if node.branch && !node.expanded {
            if node.lazy {
                return self.request_lazy(id);
            }
            return TreeNavigationOutcome::ExpandToggled { id, expanded: true };
        }
        // first child: next row with greater depth
        if let Some(child) = nodes.iter().skip(idx + 1).find(|n| n.depth > node.depth) {
            if child.enabled || child.branch {
                self.collection.set_active(Some(child.id.clone()));
                return TreeNavigationOutcome::FocusChanged {
                    id: Some(child.id.clone()),
                };
            }
        }
        TreeNavigationOutcome::Ignored
    }

    /// Intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        nodes: &[TreeNavNode<Id>],
    ) -> TreeNavigationOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled || !self.focused {
            return TreeNavigationOutcome::Ignored;
        }
        let coll = Self::collection_items(nodes);
        match intent {
            UiIntent::Activate | UiIntent::Submit => self.activate_focus(nodes),
            UiIntent::Expand => self.expand_or_child(nodes),
            UiIntent::Collapse => self.collapse_or_parent(nodes),
            UiIntent::Cancel | UiIntent::Close => TreeNavigationOutcome::Cancelled,
            UiIntent::Search => {
                self.filter_active = true;
                TreeNavigationOutcome::Changed
            }
            other => {
                let out = self.collection.handle_intent(other, &coll);
                Self::map_focus(out)
            }
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        nodes: &[TreeNavNode<Id>],
    ) -> TreeNavigationOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled {
            return TreeNavigationOutcome::Ignored;
        }
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Right)) {
            for r in &self.regions {
                if r.area.contains(event.position) {
                    self.focused = true;
                    self.collection.set_active(Some(r.id.clone()));
                    return TreeNavigationOutcome::ContextMenuRequested { id: r.id.clone() };
                }
            }
            return TreeNavigationOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return TreeNavigationOutcome::Ignored;
        }
        self.focused = true;
        for r in &self.disclosure_regions {
            if r.area.contains(event.position) {
                let id = r.id.clone();
                if let Some(n) = nodes.iter().find(|n| n.id == id) {
                    if n.lazy && !n.expanded {
                        return self.request_lazy(id);
                    }
                    return TreeNavigationOutcome::ExpandToggled {
                        id,
                        expanded: !n.expanded,
                    };
                }
            }
        }
        for r in &self.regions {
            if r.area.contains(event.position) {
                let id = r.id.clone();
                self.collection.set_active(Some(id.clone()));
                if let Some(n) = nodes.iter().find(|n| n.id == id) {
                    if n.routeable && n.enabled {
                        self.route = Some(id.clone());
                        self.recompute_ancestors(nodes);
                        return TreeNavigationOutcome::RouteChanged { id };
                    }
                    if n.branch {
                        if n.lazy && !n.expanded {
                            return self.request_lazy(id);
                        }
                        return TreeNavigationOutcome::ExpandToggled {
                            id,
                            expanded: !n.expanded,
                        };
                    }
                }
                return TreeNavigationOutcome::FocusChanged { id: Some(id) };
            }
        }
        TreeNavigationOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Hierarchical route navigation chrome.
#[derive(Debug, Clone, Copy)]
pub struct TreeNavigation<'a, Id> {
    nodes: &'a [TreeNavNode<Id>],
    system: &'a DesignSystem,
    ascii: bool,
    empty_message: &'a str,
}

impl<'a, Id> TreeNavigation<'a, Id> {
    /// Create.
    #[must_use]
    pub const fn new(nodes: &'a [TreeNavNode<Id>], system: &'a DesignSystem) -> Self {
        Self {
            nodes,
            system,
            ascii: false,
            empty_message: "(empty)",
        }
    }

    /// ASCII glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Empty message.
    #[must_use]
    pub const fn empty_message(mut self, msg: &'a str) -> Self {
        self.empty_message = msg;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut TreeNavigationState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.regions.clear();
        state.disclosure_regions.clear();
        state.root = area;
        state.narrow = area.width < TREE_NAV_NARROW_MAX_WIDTH;
        if area.is_empty() {
            return;
        }

        state.reconcile_route(self.nodes);
        let coll = TreeNavigationState::<Id>::collection_items(self.nodes);
        let vp = usize::from(area.height).saturating_sub(if state.filter_active { 1 } else { 0 });
        state
            .collection
            .set_viewport(state.collection.offset(), vp.max(1), coll.len());
        let _ = state.collection.ensure_active_visible(&coll);

        let mut y = area.y;
        if state.filter_active && y < area.bottom() {
            let q = format!("/{}", state.filter);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&q, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Focus).add_modifier(Modifier::BOLD),
            );
            y = y.saturating_add(1);
        }

        let filtered = state.visible_filtered(self.nodes);
        if filtered.is_empty() {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.empty_message, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }

        let offset = state.collection.offset();
        let mut row = 0usize;
        for node in filtered {
            if !node.enabled && !node.branch {
                // still paint disabled route leaves
            }
            // Map collection offset to paint rows for focusable set
            let focusable = node.enabled || node.branch;
            if focusable {
                if row < offset {
                    row += 1;
                    continue;
                }
                row += 1;
            }
            if y >= area.bottom() {
                break;
            }

            let is_route = state.is_route(&node.id);
            let is_ancestor = state.is_route_ancestor(&node.id);
            let is_focus = state.collection.active() == Some(&node.id) && state.focused;

            let style = if !node.enabled && !node.branch {
                self.system.style(Role::TextDisabled)
            } else if is_route {
                self.system
                    .style(Role::TextStrong)
                    .patch(self.system.style(Role::SelectionTint))
                    .add_modifier(Modifier::BOLD)
            } else if is_focus {
                self.system
                    .style(Role::Focus)
                    .add_modifier(Modifier::REVERSED)
            } else if is_ancestor {
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::Text)
            };

            let depth = node.depth.min(TREE_NAV_MAX_INDENT_DEPTH);
            let indent_cols = if state.narrow {
                depth.min(2) * 1
            } else {
                depth * TREE_NAV_INDENT
            };
            let indent = " ".repeat(usize::from(indent_cols));

            let chev = if node.branch {
                if node.expanded {
                    if self.ascii { "v" } else { "▾" }
                } else if self.ascii {
                    ">"
                } else {
                    "▸"
                }
            } else if self.ascii {
                " "
            } else {
                "·"
            };

            let status = node
                .status
                .mark(self.ascii)
                .map(|m| format!("{m} "))
                .unwrap_or_default();
            let icon = node
                .icon
                .as_deref()
                .map(|i| format!("{i} "))
                .unwrap_or_default();
            let badge = node
                .badge
                .as_deref()
                .map(|b| format!(" [{b}]"))
                .unwrap_or_default();
            let lazy = if node.lazy && !node.expanded {
                if self.ascii { " ?" } else { " …" }
            } else {
                ""
            };

            let label = if state.narrow {
                take_display_cols(&node.label, 8)
            } else {
                node.label.clone()
            };

            let line = format!("{indent}{chev} {status}{icon}{label}{badge}{lazy}");
            let rect = Rect::new(area.x, y, area.width, 1);
            buffer.set_stringn(
                rect.x,
                rect.y,
                take_display_cols(&line, usize::from(rect.width)),
                usize::from(rect.width),
                style,
            );

            // disclosure hit: first few cols after indent
            if node.branch {
                let dw = 2u16;
                let dx = area.x.saturating_add(indent_cols);
                state.disclosure_regions.push(HitRegion {
                    id: node.id.clone(),
                    area: Rect::new(dx, y, dw.min(area.width), 1),
                });
            }
            if node.enabled || node.branch {
                state.regions.push(HitRegion {
                    id: node.id.clone(),
                    area: rect,
                });
            }
            y = y.saturating_add(1);
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &TreeNavigationState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "tree-navigation nodes={} route_set={} filter={}",
            self.nodes.len(),
            state.route().is_some(),
            !state.filter().is_empty()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Tree)
                .label("tree-navigation")
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: false,
                    invalid: false,
                    expanded: true,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &TreeNavigation<'_, Id> {
    type State = TreeNavigationState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for TreeNavigation<'_, Id> {
    type State = TreeNavigationState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Example projections ─────────────────────────────────────────────────────

/// Project / file explorer sample.
#[must_use]
pub fn example_project_tree() -> Vec<TreeNavNode<&'static str>> {
    vec![
        TreeNavNode::branch("root", "termrock", 0)
            .expanded(true)
            .icon("📁"),
        TreeNavNode::branch("src", "src", 1)
            .parent("root")
            .expanded(true)
            .icon("📁"),
        TreeNavNode::leaf("lib", "lib.rs", 2)
            .parent("src")
            .badge("rs")
            .icon("📄"),
        TreeNavNode::leaf("main", "main.rs", 2)
            .parent("src")
            .badge("rs")
            .status(TreeNavStatus::Dirty),
        TreeNavNode::branch("widgets", "widgets", 2)
            .parent("src")
            .expanded(false)
            .lazy(true),
        TreeNavNode::branch("docs", "docs", 1)
            .parent("root")
            .expanded(false),
        TreeNavNode::leaf("readme", "README.md", 1)
            .parent("root")
            .badge("md"),
    ]
}

/// Schema / database tree sample.
#[must_use]
pub fn example_schema_tree() -> Vec<TreeNavNode<&'static str>> {
    vec![
        TreeNavNode::branch("db", "analytics", 0).expanded(true),
        TreeNavNode::branch("public", "public", 1)
            .parent("db")
            .expanded(true),
        TreeNavNode::leaf("users", "users", 2)
            .parent("public")
            .badge("table")
            .routeable(true),
        TreeNavNode::leaf("events", "events", 2)
            .parent("public")
            .badge("table"),
        TreeNavNode::branch("views", "views", 1)
            .parent("db")
            .lazy(true)
            .status(TreeNavStatus::Loading),
    ]
}

/// Settings tree sample.
#[must_use]
pub fn example_settings_tree() -> Vec<TreeNavNode<&'static str>> {
    vec![
        TreeNavNode::branch("app", "Application", 0).expanded(true),
        TreeNavNode::leaf("general", "General", 1).parent("app"),
        TreeNavNode::leaf("appearance", "Appearance", 1).parent("app"),
        TreeNavNode::branch("agent", "Agent", 0).expanded(true),
        TreeNavNode::leaf("models", "Models", 1)
            .parent("agent")
            .badge("3"),
        TreeNavNode::leaf("tools", "Tools", 1)
            .parent("agent")
            .status(TreeNavStatus::Dirty),
        TreeNavNode::leaf("keys", "Secrets", 1)
            .parent("agent")
            .status(TreeNavStatus::Warning),
    ]
}

/// Documentation nav sample.
#[must_use]
pub fn example_docs_tree() -> Vec<TreeNavNode<&'static str>> {
    vec![
        TreeNavNode::branch("guide", "Guide", 0).expanded(true),
        TreeNavNode::leaf("intro", "Introduction", 1).parent("guide"),
        TreeNavNode::leaf("install", "Install", 1).parent("guide"),
        TreeNavNode::branch("api", "API", 0).expanded(true),
        TreeNavNode::leaf("widgets", "Widgets", 1).parent("api"),
        TreeNavNode::leaf("interaction", "Interaction", 1).parent("api"),
        TreeNavNode::leaf("broken", "Missing page", 1)
            .parent("api")
            .status(TreeNavStatus::Error)
            .enabled(false),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn route_distinct_from_focus() {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(Some("lib"));
        state.set_focused(true);
        state.reconcile_route(&nodes);
        state.focus_route(&nodes);
        assert_eq!(state.route(), Some(&"lib"));
        // move down
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &nodes),
            TreeNavigationOutcome::FocusChanged { .. }
        ));
        assert_eq!(state.route(), Some(&"lib"));
        assert_ne!(state.focus(), Some(&"lib"));
    }

    #[test]
    fn enter_sets_route_on_leaf() {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(None);
        state.set_focused(true);
        let coll = TreeNavigationState::<&str>::collection_items(&nodes);
        let _ = state.collection.reconcile(&coll);
        state.collection.set_active(Some("main"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &nodes),
            TreeNavigationOutcome::RouteChanged { id: "main" }
        ));
        assert_eq!(state.route(), Some(&"main"));
        assert!(state.route_ancestors().contains(&"src"));
    }

    #[test]
    fn left_collapses_or_parent() {
        let mut nodes = example_project_tree();
        // src is expanded
        let mut state = TreeNavigationState::new(None);
        state.set_focused(true);
        state.collection.set_active(Some("src"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &nodes),
            TreeNavigationOutcome::ExpandToggled {
                id: "src",
                expanded: false
            }
        ));
        // after collapse request, simulate collapsed + focus on lib then left -> parent
        if let Some(n) = nodes.iter_mut().find(|n| n.id == "src") {
            n.expanded = false;
        }
        state.collection.set_active(Some("lib"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &nodes),
            TreeNavigationOutcome::FocusChanged { id: Some("src") }
        ));
    }

    #[test]
    fn right_lazy_load() {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(None);
        state.set_focused(true);
        state.collection.set_active(Some("widgets"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &nodes),
            TreeNavigationOutcome::LazyLoadRequested {
                id: "widgets",
                generation: 1
            }
        ));
        assert!(state.apply_lazy_result(1));
        assert!(!state.apply_lazy_result(0));
    }

    #[test]
    fn typeahead_jumps() {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(None);
        state.set_focused(true);
        let coll = TreeNavigationState::<&str>::collection_items(&nodes);
        let _ = state.collection.reconcile(&coll);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
                &nodes
            ),
            TreeNavigationOutcome::TypeaheadMatched { id: "main" }
        ));
    }

    #[test]
    fn route_preserved_when_still_visible() {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(Some("main"));
        state.reconcile_route(&nodes);
        assert_eq!(state.route(), Some(&"main"));
        assert!(
            state.is_route_ancestor(&"src")
                || state
                    .route_ancestors()
                    .iter()
                    .any(|a| *a == "src" || *a == "root")
        );
    }

    #[test]
    fn filter_does_not_clear_route() {
        let nodes = example_docs_tree();
        let mut state = TreeNavigationState::new(Some("install"));
        state.set_focused(true);
        state.reconcile_route(&nodes);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            &nodes,
        );
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &nodes,
        );
        assert_eq!(state.route(), Some(&"install"));
    }

    #[test]
    fn context_menu() {
        let nodes = example_settings_tree();
        let mut state = TreeNavigationState::new(Some("general"));
        state.set_focused(true);
        state.collection.set_active(Some("models"));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
                &nodes
            ),
            TreeNavigationOutcome::ContextMenuRequested { id: "models" }
        ));
    }

    #[test]
    fn paint_and_mouse() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let nodes = example_schema_tree();
        let mut state = TreeNavigationState::new(Some("users"));
        state.set_focused(true);
        let area = Rect::new(0, 0, 32, 12);
        let mut buf = Buffer::empty(area);
        TreeNavigation::new(&nodes, &system)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        if let Some(hit) = state.regions.iter().find(|r| r.id == "events") {
            assert!(matches!(
                state.handle_mouse(
                    MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        position: Position::new(hit.area.x, hit.area.y),
                        modifiers: KeyModifiers::NONE,
                    },
                    &nodes
                ),
                TreeNavigationOutcome::RouteChanged { id: "events" }
            ));
        }
    }

    #[test]
    fn narrow_paint() {
        let system = DesignSystem::default();
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(Some("lib"));
        state.set_focused(true);
        let area = Rect::new(0, 0, 12, 10);
        let mut buf = Buffer::empty(area);
        TreeNavigation::new(&nodes, &system)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        assert!(state.narrow);
    }

    #[test]
    fn examples_nonempty() {
        assert!(!example_project_tree().is_empty());
        assert!(!example_schema_tree().is_empty());
        assert!(!example_settings_tree().is_empty());
        assert!(!example_docs_tree().is_empty());
    }

    #[test]
    fn fuzz_keys() {
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(Some("lib"));
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key, &nodes);
            state.set_focused(true);
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let nodes = example_project_tree();
        let mut state = TreeNavigationState::new(Some("main"));
        state.set_focused(true);
        let area = Rect::new(0, 0, 40, 14);
        let mut buf = Buffer::empty(area);
        let w = TreeNavigation::new(&nodes, &system).ascii(true);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let nodes = example_docs_tree();
        let state = TreeNavigationState::new(Some("intro"));
        let mut scene = SemanticScene::<&str, ()>::default();
        TreeNavigation::new(&nodes, &system).register_semantic(
            &mut scene,
            "tn",
            Rect::new(0, 0, 30, 10),
            &state,
        );
        assert!(scene.get(&"tn").is_some());
    }
}
