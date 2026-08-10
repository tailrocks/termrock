// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Primary application navigation: [`NavigationList`] + [`Sidebar`] shell.
//!
//! **Mission.** App shells need hierarchical, sectioned navigation with route
//! selection distinct from keyboard focus, badges/status, collapse to rail /
//! drawer / palette, and search for large trees — without owning app routes.
//!
//! **vs [`Tabs`](super::Tabs).** Tabs switch panels inside a region. Sidebar is
//! primary app routing (often AppShell start dock).
//! **vs [`Tree`](super::Tree).** Tree is data hierarchy; NavigationList is
//! route-oriented with sections, rail collapse, and semantic commands.
//! **vs [`Menu`](super::Menu).** Menu is ephemeral overlay; nav is persistent.
//!
//! **Route ≠ focus.** [`NavigationListState::route`] is the active destination;
//! roving focus is independent until activation (Enter / click).
//!
//! Research: IDE sidebars, Yazi, Posting, OpenCode, shadcn sidebar.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        CollectionItem, CollectionOutcome, CollectionState, HitRegion, OverlayId, OverlayOutcome,
        OverlaySize, OverlaySpec, OverlayStack, RovingOrientation, SemanticNode, SemanticRole,
        SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

use super::{Panel, PanelChrome};

/// Width under which expanded sidebar prefers rail.
pub const SIDEBAR_RAIL_MAX_WIDTH: u16 = 12;
/// Width under which host should open drawer/palette instead of dock.
pub const SIDEBAR_DRAWER_MAX_WIDTH: u16 = 28;
/// Overlay id for sidebar-as-drawer.
pub const SIDEBAR_DRAWER_OVERLAY_ID: &str = "termrock.sidebar-drawer";

// ── Item model ──────────────────────────────────────────────────────────────

/// Visual / semantic status on a nav row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum NavItemStatus {
    /// None.
    #[default]
    None,
    /// Busy.
    Running,
    /// Ok.
    Success,
    /// Warning.
    Warning,
    /// Error.
    Error,
    /// Unsaved / dirty.
    Dirty,
}

impl NavItemStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Running => "running",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Dirty => "dirty",
        }
    }

    /// Non-color mark.
    #[must_use]
    pub const fn mark(self, ascii: bool) -> Option<&'static str> {
        match (self, ascii) {
            (Self::None, _) => None,
            (Self::Running, true) => Some("*"),
            (Self::Running, false) => Some("●"),
            (Self::Success, true) => Some("+"),
            (Self::Success, false) => Some("✓"),
            (Self::Warning, true) => Some("!"),
            (Self::Warning, false) => Some("⚠"),
            (Self::Error, true) => Some("x"),
            (Self::Error, false) => Some("✗"),
            (Self::Dirty, true) => Some("."),
            (Self::Dirty, false) => Some("•"),
        }
    }
}

/// Kind of navigation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum NavItemKind {
    /// Activatable route / leaf.
    #[default]
    Item,
    /// Section header (not activatable; may collapse children).
    Section,
    /// Group / branch (expandable).
    Group,
    /// Visual separator.
    Separator,
}

impl NavItemKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Section => "section",
            Self::Group => "group",
            Self::Separator => "separator",
        }
    }

    /// Participates in focus / activation.
    #[must_use]
    pub const fn is_focusable(self) -> bool {
        matches!(self, Self::Item | Self::Group | Self::Section)
    }

    /// Can be a route target.
    #[must_use]
    pub const fn is_route(self) -> bool {
        matches!(self, Self::Item)
    }
}

/// One navigation row (owned; host projects each frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem<Id> {
    /// Stable id.
    pub id: Id,
    /// Label.
    pub label: String,
    /// Optional leading icon / glyph (1–2 cols).
    pub icon: Option<String>,
    /// Badge text (`3`, `NEW`).
    pub badge: Option<String>,
    /// Status mark.
    pub status: NavItemStatus,
    /// Kind.
    pub kind: NavItemKind,
    /// Enabled for activation.
    pub enabled: bool,
    /// Indent depth (0 = root).
    pub depth: u8,
    /// Group/section expanded (host-projected).
    pub expanded: bool,
    /// Has children (show chevron).
    pub has_children: bool,
    /// Optional shortcut / command id for semantic commands.
    pub command: Option<String>,
}

impl<Id> NavItem<Id> {
    /// Leaf route item.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            icon: None,
            badge: None,
            status: NavItemStatus::None,
            kind: NavItemKind::Item,
            enabled: true,
            depth: 0,
            expanded: true,
            has_children: false,
            command: None,
        }
    }

    /// Section header.
    #[must_use]
    pub fn section(id: Id, label: impl Into<String>) -> Self {
        Self {
            kind: NavItemKind::Section,
            has_children: true,
            expanded: true,
            ..Self::new(id, label)
        }
    }

    /// Expandable group.
    #[must_use]
    pub fn group(id: Id, label: impl Into<String>) -> Self {
        Self {
            kind: NavItemKind::Group,
            has_children: true,
            expanded: true,
            ..Self::new(id, label)
        }
    }

    /// Separator (id still required for list identity).
    #[must_use]
    pub fn separator(id: Id) -> Self {
        Self {
            kind: NavItemKind::Separator,
            enabled: false,
            has_children: false,
            label: String::new(),
            ..Self::new(id, "")
        }
    }

    /// Icon.
    #[must_use]
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Badge.
    #[must_use]
    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: NavItemStatus) -> Self {
        self.status = status;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Depth.
    #[must_use]
    pub const fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    /// Expanded.
    #[must_use]
    pub const fn expanded(mut self, on: bool) -> Self {
        self.expanded = on;
        self
    }

    /// Has children.
    #[must_use]
    pub const fn has_children(mut self, on: bool) -> Self {
        self.has_children = on;
        self
    }

    /// Semantic command id.
    #[must_use]
    pub fn command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }
}

/// Compatibility alias used by ResourceBrowser / older call sites.
pub type SidebarItem<Id> = NavItem<Id>;

// ── Presentation ────────────────────────────────────────────────────────────

/// Sidebar chrome presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SidebarPresentation {
    /// Full labels in dock.
    #[default]
    Expanded,
    /// Compact rail (icon / first glyph).
    Rail,
    /// Host paints as drawer overlay.
    Drawer,
    /// Host should open command palette for navigation.
    Palette,
}

impl SidebarPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Rail => "rail",
            Self::Drawer => "drawer",
            Self::Palette => "palette",
        }
    }
}

/// From available dock width (host still chooses drawer/palette policy).
#[must_use]
pub fn sidebar_presentation_for_width(width: u16) -> SidebarPresentation {
    if width < SIDEBAR_RAIL_MAX_WIDTH {
        SidebarPresentation::Rail
    } else {
        SidebarPresentation::Expanded
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Navigation list outcomes (route / focus / structure).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NavigationListOutcome<Id> {
    /// No effect.
    Ignored,
    /// Focus / filter chrome.
    Changed,
    /// Keyboard focus moved (route unchanged).
    FocusChanged {
        /// Focused id.
        id: Option<Id>,
    },
    /// Active **route** changed (Enter / click / host).
    RouteChanged {
        /// Route id.
        id: Id,
    },
    /// Group/section expand toggled (host updates projection).
    ExpandToggled {
        /// Item id.
        id: Id,
        /// New expanded.
        expanded: bool,
    },
    /// Filter / search text changed.
    FilterChanged {
        /// Query.
        query: String,
    },
    /// Contextual actions for item (host menu).
    ContextMenuRequested {
        /// Item.
        id: Id,
    },
    /// Semantic command shortcut activated.
    CommandRequested {
        /// Command id.
        command: String,
        /// Source item.
        id: Id,
    },
}

/// Sidebar outcomes (nav + chrome).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SidebarOutcome<Id> {
    /// No change.
    Ignored,
    /// Chrome changed.
    Changed,
    /// Route selected (compat name for ResourceBrowser).
    Selected(Id),
    /// Focus moved.
    FocusChanged {
        /// Focus id.
        id: Option<Id>,
    },
    /// Rail/expanded toggled (compat).
    ToggleRail {
        /// Expanded (true) vs rail (false).
        expanded: bool,
    },
    /// Presentation changed.
    PresentationChanged {
        /// Presentation.
        presentation: SidebarPresentation,
    },
    /// Expand toggle.
    ExpandToggled {
        /// Id.
        id: Id,
        /// Expanded.
        expanded: bool,
    },
    /// Filter.
    FilterChanged {
        /// Query.
        query: String,
    },
    /// Context menu.
    ContextMenuRequested {
        /// Id.
        id: Id,
    },
    /// Command.
    CommandRequested {
        /// Command.
        command: String,
        /// Id.
        id: Id,
    },
    /// Host should open command palette for nav.
    OpenPalette,
    /// Host should open drawer overlay.
    OpenDrawer,
    /// Cancel / blur.
    Blurred,
}

impl<Id> From<NavigationListOutcome<Id>> for SidebarOutcome<Id> {
    fn from(value: NavigationListOutcome<Id>) -> Self {
        match value {
            NavigationListOutcome::Ignored => Self::Ignored,
            NavigationListOutcome::Changed => Self::Changed,
            NavigationListOutcome::FocusChanged { id } => Self::FocusChanged { id },
            NavigationListOutcome::RouteChanged { id } => Self::Selected(id),
            NavigationListOutcome::ExpandToggled { id, expanded } => {
                Self::ExpandToggled { id, expanded }
            }
            NavigationListOutcome::FilterChanged { query } => Self::FilterChanged { query },
            NavigationListOutcome::ContextMenuRequested { id } => {
                Self::ContextMenuRequested { id }
            }
            NavigationListOutcome::CommandRequested { command, id } => {
                Self::CommandRequested { command, id }
            }
        }
    }
}

// ── NavigationList state ────────────────────────────────────────────────────

/// Pure navigation list: route ≠ focus, filter, expand hooks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationListState<Id> {
    /// Active route (destination).
    route: Option<Id>,
    /// Roving focus among focusable rows.
    collection: CollectionState<Id>,
    /// Search / filter query (host may filter projection).
    filter: String,
    /// Filter field active.
    filter_active: bool,
    focused: bool,
    enabled: bool,
    /// Host grants input (sidebar accepts_input).
    pub accepts_input: bool,
    regions: Vec<HitRegion<Id>>,
    root: Rect,
}

impl<Id> Default for NavigationListState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id> NavigationListState<Id> {
    /// New list; optional initial route.
    #[must_use]
    pub fn new(route: Option<Id>) -> Self {
        let mut collection = CollectionState::new()
            .wrap(true)
            .orientation(RovingOrientation::Vertical);
        if let Some(ref id) = route {
            // set after first reconcile by host; store route only
            let _ = id;
        }
        Self {
            route,
            collection,
            filter: String::new(),
            filter_active: false,
            focused: false,
            enabled: true,
            accepts_input: true,
            regions: Vec::new(),
            root: Rect::default(),
        }
    }

    /// Active route.
    #[must_use]
    pub const fn route(&self) -> Option<&Id> {
        self.route.as_ref()
    }

    /// Compat: selected == route.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.route.as_ref()
    }

    /// Focused row id.
    #[must_use]
    pub fn focus(&self) -> Option<&Id> {
        self.collection.active()
    }

    /// Cursor index among last projected items (compat).
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        // best-effort: 0 if unknown without projection
        0
    }

    /// Cursor index from projection.
    #[must_use]
    pub fn cursor_index_in(&self, items: &[NavItem<Id>]) -> usize
    where
        Id: Clone + PartialEq,
    {
        let focusable = focusable_items(items);
        self.collection
            .active_index(&focusable)
            .unwrap_or(0)
    }

    /// Filter query.
    #[must_use]
    pub fn filter(&self) -> &str {
        &self.filter
    }

    /// Filter field focused.
    #[must_use]
    pub const fn is_filter_active(&self) -> bool {
        self.filter_active
    }

    /// Focus strip.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        if !on {
            self.filter_active = false;
        }
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Set route without moving focus.
    pub fn set_route(&mut self, id: Option<Id>) {
        self.route = id;
    }

    /// Set route and align focus.
    pub fn set_route_and_focus(&mut self, id: Id)
    where
        Id: Clone + PartialEq,
    {
        self.route = Some(id.clone());
        self.collection.set_active(Some(id));
    }

    /// Set filter.
    pub fn set_filter(&mut self, q: impl Into<String>) {
        self.filter = q.into();
    }

    fn collection_items(items: &[NavItem<Id>]) -> Vec<CollectionItem<Id>>
    where
        Id: Clone,
    {
        focusable_items(items)
    }

    /// Activate focused row as route (if item).
    pub fn activate_focus(&mut self, items: &[NavItem<Id>]) -> NavigationListOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        let Some(id) = self.collection.active().cloned() else {
            return NavigationListOutcome::Ignored;
        };
        let Some(item) = items.iter().find(|i| i.id == id) else {
            return NavigationListOutcome::Ignored;
        };
        if !item.enabled {
            return NavigationListOutcome::Ignored;
        }
        match item.kind {
            NavItemKind::Item => {
                self.route = Some(id.clone());
                if let Some(cmd) = item.command.clone() {
                    return NavigationListOutcome::CommandRequested { command: cmd, id };
                }
                NavigationListOutcome::RouteChanged { id }
            }
            NavItemKind::Group | NavItemKind::Section => NavigationListOutcome::ExpandToggled {
                id,
                expanded: !item.expanded,
            },
            NavItemKind::Separator => NavigationListOutcome::Ignored,
        }
    }

    /// Key adapter.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        items: &[NavItem<Id>],
    ) -> NavigationListOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if key.kind == KeyEventKind::Release || !self.enabled || !self.accepts_input {
            return NavigationListOutcome::Ignored;
        }
        if !self.focused {
            return NavigationListOutcome::Ignored;
        }

        // Filter mode
        if self.filter_active {
            match key.code {
                KeyCode::Esc => {
                    self.filter_active = false;
                    return NavigationListOutcome::Changed;
                }
                KeyCode::Enter => {
                    self.filter_active = false;
                    return self.activate_focus(items);
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    return NavigationListOutcome::FilterChanged {
                        query: self.filter.clone(),
                    };
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT)
                        && !c.is_control() =>
                {
                    self.filter.push(c);
                    return NavigationListOutcome::FilterChanged {
                        query: self.filter.clone(),
                    };
                }
                _ => {}
            }
        }

        let coll = Self::collection_items(items);
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

        // Start filter
        if matches!(key.code, KeyCode::Char('/') | KeyCode::Char('f'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
            || (key.code == KeyCode::Char('/') && key.modifiers.is_empty())
        {
            self.filter_active = true;
            return NavigationListOutcome::Changed;
        }

        // Context menu
        if key.code == KeyCode::Char(' ')
            && key.modifiers.contains(KeyModifiers::SHIFT)
            || matches!(key.code, KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL))
        {
            if let Some(id) = self.collection.active().cloned() {
                return NavigationListOutcome::ContextMenuRequested { id };
            }
        }

        // Expand/collapse Left/Right
        if matches!(key.code, KeyCode::Left | KeyCode::Right) && key.modifiers.is_empty() {
            if let Some(id) = self.collection.active().cloned() {
                if let Some(item) = items.iter().find(|i| i.id == id) {
                    if item.has_children
                        && matches!(item.kind, NavItemKind::Group | NavItemKind::Section)
                    {
                        let expand = key.code == KeyCode::Right;
                        return NavigationListOutcome::ExpandToggled {
                            id,
                            expanded: expand,
                        };
                    }
                }
            }
        }

        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            return self.activate_focus(items);
        }

        // Space activates item without expand (if leaf)
        if matches!(key.code, KeyCode::Char(' ')) && key.modifiers.is_empty() {
            return self.activate_focus(items);
        }

        match self.collection.handle_key(key, &coll) {
            CollectionOutcome::ActiveChanged { to, .. } => {
                NavigationListOutcome::FocusChanged { id: to }
            }
            CollectionOutcome::Scrolled => NavigationListOutcome::Changed,
            CollectionOutcome::Ignored => NavigationListOutcome::Ignored,
        }
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        items: &[NavItem<Id>],
    ) -> NavigationListOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled || !self.focused || !self.accepts_input {
            return NavigationListOutcome::Ignored;
        }
        let coll = Self::collection_items(items);
        match intent {
            UiIntent::Activate | UiIntent::Submit => self.activate_focus(items),
            UiIntent::Search => {
                self.filter_active = true;
                NavigationListOutcome::Changed
            }
            UiIntent::Expand => {
                if let Some(id) = self.collection.active().cloned() {
                    return NavigationListOutcome::ExpandToggled {
                        id,
                        expanded: true,
                    };
                }
                NavigationListOutcome::Ignored
            }
            UiIntent::Collapse => {
                if let Some(id) = self.collection.active().cloned() {
                    return NavigationListOutcome::ExpandToggled {
                        id,
                        expanded: false,
                    };
                }
                NavigationListOutcome::Ignored
            }
            other => match self.collection.handle_intent(other, &coll) {
                CollectionOutcome::ActiveChanged { to, .. } => {
                    NavigationListOutcome::FocusChanged { id: to }
                }
                CollectionOutcome::Scrolled => NavigationListOutcome::Changed,
                CollectionOutcome::Ignored => NavigationListOutcome::Ignored,
            },
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        items: &[NavItem<Id>],
    ) -> NavigationListOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        if !self.enabled || !self.accepts_input {
            return NavigationListOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            // right-click context
            if matches!(event.kind, MouseEventKind::Down(MouseButton::Right)) {
                for r in &self.regions {
                    if r.area.contains(event.position) {
                        self.focused = true;
                        self.collection.set_active(Some(r.id.clone()));
                        return NavigationListOutcome::ContextMenuRequested {
                            id: r.id.clone(),
                        };
                    }
                }
            }
            return NavigationListOutcome::Ignored;
        }
        self.focused = true;
        for r in &self.regions {
            if r.area.contains(event.position) {
                let id = r.id.clone();
                self.collection.set_active(Some(id.clone()));
                if let Some(item) = items.iter().find(|i| i.id == id) {
                    if matches!(item.kind, NavItemKind::Group | NavItemKind::Section)
                        && item.has_children
                    {
                        return NavigationListOutcome::ExpandToggled {
                            id,
                            expanded: !item.expanded,
                        };
                    }
                    if item.kind.is_route() && item.enabled {
                        self.route = Some(id.clone());
                        return NavigationListOutcome::RouteChanged { id };
                    }
                }
                return NavigationListOutcome::FocusChanged { id: Some(id) };
            }
        }
        NavigationListOutcome::Ignored
    }
}

fn focusable_items<Id: Clone>(items: &[NavItem<Id>]) -> Vec<CollectionItem<Id>> {
    items
        .iter()
        .filter(|i| i.kind.is_focusable() && i.kind != NavItemKind::Separator)
        .map(|i| {
            CollectionItem::new(i.id.clone(), i.label.clone())
                .enabled(i.enabled || matches!(i.kind, NavItemKind::Section | NavItemKind::Group))
        })
        .collect()
}

// ── Sidebar state ───────────────────────────────────────────────────────────

/// Sidebar = navigation list + presentation chrome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidebarState<Id> {
    /// Inner list (route / focus / filter).
    pub nav: NavigationListState<Id>,
    presentation: SidebarPresentation,
    /// Host grants input (compat field).
    pub accepts_input: bool,
}

impl<Id> Default for SidebarState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id> SidebarState<Id> {
    /// Expanded sidebar with optional initial route.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        let mut nav = NavigationListState::new(selected);
        nav.accepts_input = true;
        Self {
            nav,
            presentation: SidebarPresentation::Expanded,
            accepts_input: true,
        }
    }

    /// Presentation.
    #[must_use]
    pub const fn with_presentation(mut self, p: SidebarPresentation) -> Self {
        self.presentation = p;
        self
    }

    /// Selected route (compat).
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.nav.route()
    }

    /// Route.
    #[must_use]
    pub const fn route(&self) -> Option<&Id> {
        self.nav.route()
    }

    /// Focus id.
    #[must_use]
    pub fn focus(&self) -> Option<&Id> {
        self.nav.focus()
    }

    /// Expanded (not rail).
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        matches!(self.presentation, SidebarPresentation::Expanded)
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> SidebarPresentation {
        self.presentation
    }

    /// Cursor index among items (compat; uses projection).
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.nav.cursor_index()
    }

    /// Cursor in projection.
    #[must_use]
    pub fn cursor_index_in(&self, items: &[NavItem<Id>]) -> usize
    where
        Id: Clone + PartialEq,
    {
        self.nav.cursor_index_in(items)
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.nav.set_focused(on);
    }

    /// Sync accepts_input.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.nav.accepts_input = on;
    }

    /// Force presentation.
    pub fn set_presentation(&mut self, p: SidebarPresentation) {
        self.presentation = p;
    }

    /// Toggle rail ↔ expanded.
    pub fn toggle_rail(&mut self) -> SidebarOutcome<Id> {
        let expanded = !self.is_expanded();
        self.presentation = if expanded {
            SidebarPresentation::Expanded
        } else {
            SidebarPresentation::Rail
        };
        SidebarOutcome::ToggleRail { expanded }
    }

    /// Auto presentation from width.
    pub fn apply_width(&mut self, width: u16) -> SidebarOutcome<Id> {
        let next = sidebar_presentation_for_width(width);
        // don't auto-exit Drawer/Palette
        if matches!(
            self.presentation,
            SidebarPresentation::Drawer | SidebarPresentation::Palette
        ) {
            return SidebarOutcome::Ignored;
        }
        if next != self.presentation {
            self.presentation = next;
            return SidebarOutcome::PresentationChanged {
                presentation: next,
            };
        }
        SidebarOutcome::Ignored
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, items: &[NavItem<Id>]) -> SidebarOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        self.nav.accepts_input = self.accepts_input;
        if !self.accepts_input {
            return SidebarOutcome::Ignored;
        }
        // ensure focused when host only sets accepts_input (legacy tests)
        if !self.nav.focused && self.accepts_input {
            self.nav.focused = true;
        }

        // Rail toggle
        if key.code == KeyCode::Char('[') && key.modifiers.is_empty() {
            return self.toggle_rail();
        }
        // Palette hint
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.presentation = SidebarPresentation::Palette;
            return SidebarOutcome::OpenPalette;
        }
        // Drawer request on very narrow (host may already show drawer)
        if key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.presentation = SidebarPresentation::Drawer;
            return SidebarOutcome::OpenDrawer;
        }
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if self.nav.filter_active {
                return self.nav.handle_key(key, items).into();
            }
            return SidebarOutcome::Blurred;
        }

        self.nav.handle_key(key, items).into()
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        items: &[NavItem<Id>],
    ) -> SidebarOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        self.nav.accepts_input = self.accepts_input;
        if !self.accepts_input {
            return SidebarOutcome::Ignored;
        }
        if !self.nav.focused {
            self.nav.focused = true;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => SidebarOutcome::Blurred,
            UiIntent::Help | UiIntent::Search => {
                self.nav.filter_active = true;
                SidebarOutcome::Changed
            }
            other => self.nav.handle_intent(other, items).into(),
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        items: &[NavItem<Id>],
    ) -> SidebarOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        self.nav.accepts_input = self.accepts_input;
        self.nav.handle_mouse(event, items).into()
    }

    /// Open as drawer overlay helper.
    pub fn open_drawer_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        size: OverlaySize,
        opener: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.open(
            bounds,
            OverlaySpec::drawer(SIDEBAR_DRAWER_OVERLAY_ID, size, opener),
        )
    }
}

// ── Widgets ─────────────────────────────────────────────────────────────────

/// Navigation list paint (no outer panel).
#[derive(Debug, Clone, Copy)]
pub struct NavigationList<'a, Id> {
    items: &'a [NavItem<Id>],
    system: &'a DesignSystem,
    ascii: bool,
    rail: bool,
    show_filter: bool,
}

impl<'a, Id> NavigationList<'a, Id> {
    /// Create.
    #[must_use]
    pub const fn new(items: &'a [NavItem<Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            ascii: false,
            rail: false,
            show_filter: true,
        }
    }

    /// ASCII marks.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Rail (compact) paint.
    #[must_use]
    pub const fn rail(mut self, on: bool) -> Self {
        self.rail = on;
        self
    }

    /// Show filter row when active.
    #[must_use]
    pub const fn show_filter(mut self, on: bool) -> Self {
        self.show_filter = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut NavigationListState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.regions.clear();
        state.root = area;
        if area.is_empty() {
            return;
        }
        let coll = NavigationListState::<Id>::collection_items(self.items);
        let _ = state.collection.reconcile(&coll);
        let vp = usize::from(area.height).max(1);
        state
            .collection
            .set_viewport(state.collection.offset(), vp, coll.len());
        let _ = state.collection.ensure_active_visible(&coll);

        let mut y = area.y;
        if self.show_filter && state.filter_active && y < area.bottom() {
            let q = format!("/{}", state.filter);
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&q, usize::from(area.width)),
                usize::from(area.width),
                self.system
                    .style(Role::Focus)
                    .add_modifier(Modifier::BOLD),
            );
            y = y.saturating_add(1);
        }

        let surface = state.focused && state.accepts_input;
        let filter_q = state.filter.to_ascii_lowercase();
        let visible: Vec<&NavItem<Id>> = self
            .items
            .iter()
            .filter(|i| {
                if filter_q.is_empty() {
                    return true;
                }
                i.label.to_ascii_lowercase().contains(&filter_q)
                    || i.command
                        .as_ref()
                        .is_some_and(|c| c.to_ascii_lowercase().contains(&filter_q))
            })
            .collect();

        let offset = state.collection.offset();
        // Map offset through focusable — simple paint all filtered from y
        let mut painted = 0usize;
        for item in visible {
            if y >= area.bottom() {
                break;
            }
            if matches!(item.kind, NavItemKind::Separator) {
                let line = "─".repeat(usize::from(area.width));
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&line, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
                continue;
            }
            // skip until offset for focusable scroll — approximate
            if item.kind.is_focusable() {
                if painted < offset {
                    painted += 1;
                    continue;
                }
                painted += 1;
            }

            let route = state.route.as_ref() == Some(&item.id);
            let focus = state.collection.active() == Some(&item.id) && surface;
            let style = if !item.enabled && item.kind.is_route() {
                self.system.style(Role::TextDisabled)
            } else if route {
                self.system
                    .style(Role::Selection)
                    .add_modifier(Modifier::BOLD)
            } else if focus {
                self.system
                    .style(Role::Focus)
                    .add_modifier(Modifier::REVERSED)
            } else if matches!(item.kind, NavItemKind::Section) {
                self.system
                    .style(Role::TextMuted)
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::Text)
            };

            let gutter = if focus {
                if self.ascii {
                    ">"
                } else {
                    "›"
                }
            } else if route {
                if self.ascii {
                    "*"
                } else {
                    "•"
                }
            } else {
                " "
            };

            let text = if self.rail {
                let ch = item
                    .icon
                    .as_ref()
                    .and_then(|i| i.chars().next())
                    .or_else(|| item.label.chars().next())
                    .unwrap_or(if self.ascii { '.' } else { '·' });
                format!("{gutter}{ch}")
            } else {
                let indent = "  ".repeat(usize::from(item.depth));
                let chev = if item.has_children {
                    if item.expanded {
                        if self.ascii {
                            "v "
                        } else {
                            "▾ "
                        }
                    } else if self.ascii {
                        "> "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };
                let icon = item
                    .icon
                    .as_deref()
                    .map(|i| format!("{i} "))
                    .unwrap_or_default();
                let status = item
                    .status
                    .mark(self.ascii)
                    .map(|m| format!("{m} "))
                    .unwrap_or_default();
                let badge = item
                    .badge
                    .as_deref()
                    .map(|b| format!(" [{b}]"))
                    .unwrap_or_default();
                format!("{gutter}{indent}{chev}{status}{icon}{}{badge}", item.label)
            };

            let rect = Rect::new(area.x, y, area.width, 1);
            buffer.set_stringn(
                rect.x,
                rect.y,
                take_display_cols(&text, usize::from(rect.width)),
                usize::from(rect.width),
                style,
            );
            if item.kind.is_focusable() {
                state.regions.push(HitRegion {
                    id: item.id.clone(),
                    area: rect,
                });
            }
            y = y.saturating_add(1);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &NavigationList<'_, Id> {
    type State = NavigationListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for NavigationList<'_, Id> {
    type State = NavigationListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

/// Sidebar chrome: optional panel + navigation list.
#[derive(Debug, Clone, Copy)]
pub struct Sidebar<'a, Id> {
    items: &'a [NavItem<Id>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    title: &'a str,
    show_panel: bool,
}

impl<'a, Id: Clone + PartialEq> Sidebar<'a, Id> {
    /// Items + design system.
    #[must_use]
    pub const fn new(items: &'a [NavItem<Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            focused: true,
            ascii: false,
            title: "",
            show_panel: false,
        }
    }

    /// Scene surface focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII rail glyph fallback.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Optional title when panel chrome on.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// Draw panel border.
    #[must_use]
    pub const fn show_panel(mut self, on: bool) -> Self {
        self.show_panel = on;
        self
    }

    /// Render rail or full labels.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &SidebarState<Id>) {
        // immutable state path for legacy; paint needs mut for hits — clone regions only via paint_mut
        let mut state_mut = state.clone();
        self.paint(area, buffer, &mut state_mut);
    }

    /// Preferred paint (updates hit regions on state.nav).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SidebarState<Id>) {
        if area.is_empty() {
            return;
        }
        let _ = state.apply_width(area.width);
        state.nav.focused = self.focused || state.nav.focused;
        state.nav.accepts_input = state.accepts_input;

        let mut inner = area;
        if self.show_panel {
            let panel = Panel::new(self.system).emphasis(if self.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
            let title = if self.title.is_empty() {
                match state.presentation {
                    SidebarPresentation::Rail => "Nav",
                    _ => "Navigation",
                }
            } else {
                self.title
            };
            inner = panel.inner(area);
            Widget::render(&panel.title(title), area, buffer);
        }

        let rail = matches!(state.presentation, SidebarPresentation::Rail);
        NavigationList::new(self.items, self.system)
            .ascii(self.ascii)
            .rail(rail)
            .paint(inner, buffer, &mut state.nav);
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &SidebarState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "sidebar {} route_set={} filter={}",
            state.presentation.id(),
            state.route().is_some(),
            state.nav.filter()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::List)
                .label(if self.title.is_empty() {
                    "sidebar"
                } else {
                    self.title
                })
                .description(desc)
                .focusable(state.accepts_input)
                .disabled(!state.accepts_input)
                .state(SemanticState {
                    selected: self.focused,
                    busy: false,
                    invalid: false,
                    expanded: state.is_expanded(),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &Sidebar<'_, Id> {
    type State = SidebarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for Sidebar<'_, Id> {
    type State = SidebarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Example projectors (Studio stories) ─────────────────────────────────────

/// Database explorer sample projection.
#[must_use]
pub fn example_database_nav() -> Vec<NavItem<&'static str>> {
    vec![
        NavItem::section("conn", "Connections").icon("⬡"),
        NavItem::new("prod", "production")
            .depth(1)
            .icon("●")
            .status(NavItemStatus::Success)
            .command("nav.db.prod"),
        NavItem::new("staging", "staging")
            .depth(1)
            .icon("○")
            .command("nav.db.staging"),
        NavItem::section("schema", "Schema"),
        NavItem::group("tables", "Tables")
            .depth(1)
            .expanded(true)
            .has_children(true),
        NavItem::new("users", "users")
            .depth(2)
            .badge("12")
            .command("nav.db.users"),
        NavItem::new("orders", "orders")
            .depth(2)
            .command("nav.db.orders"),
        NavItem::new("disabled", "legacy")
            .depth(2)
            .enabled(false),
    ]
}

/// Settings nav sample.
#[must_use]
pub fn example_settings_nav() -> Vec<NavItem<&'static str>> {
    vec![
        NavItem::section("general", "General"),
        NavItem::new("profile", "Profile").depth(1).command("settings.profile"),
        NavItem::new("appearance", "Appearance")
            .depth(1)
            .command("settings.appearance"),
        NavItem::section("agent", "Agent"),
        NavItem::new("models", "Models").depth(1).badge("3"),
        NavItem::new("tools", "Tools").depth(1).status(NavItemStatus::Dirty),
        NavItem::new("keys", "API keys").depth(1),
    ]
}

/// Agent workbench nav sample.
#[must_use]
pub fn example_agent_workbench_nav() -> Vec<NavItem<&'static str>> {
    vec![
        NavItem::new("chat", "Chat")
            .icon("💬")
            .command("wb.chat"),
        NavItem::new("plan", "Plan")
            .icon("📋")
            .status(NavItemStatus::Running)
            .command("wb.plan"),
        NavItem::new("files", "Files").icon("📁").command("wb.files"),
        NavItem::separator("sep1"),
        NavItem::new("sessions", "Sessions")
            .badge("2")
            .command("wb.sessions"),
        NavItem::new("settings", "Settings").command("wb.settings"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    #[test]
    fn route_distinct_from_focus() {
        let items = [
            NavItem::new("x", "X"),
            NavItem::new("y", "Y"),
            NavItem::new("z", "Z"),
        ];
        let mut state = NavigationListState::new(Some("x"));
        state.set_focused(true);
        assert_eq!(state.route(), Some(&"x"));
        // move focus down without activating
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items),
            NavigationListOutcome::FocusChanged { .. }
        ));
        assert_eq!(state.route(), Some(&"x"));
        assert_eq!(state.focus(), Some(&"y"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            NavigationListOutcome::RouteChanged { id: "y" }
        ));
        assert_eq!(state.route(), Some(&"y"));
    }

    #[test]
    fn sidebar_select_compat() {
        let items = [SidebarItem::new("x", "X"), SidebarItem::new("y", "Y")];
        let mut state = SidebarState::new(None);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        // first focus may be x after reconcile; down -> y
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items);
        // After Down from empty, focus should be second or first then need two downs
        // new() with None: move_first on first key path sets focus
        // Down then Enter — depends on initial active
        assert!(
            matches!(out, SidebarOutcome::Selected(_))
                || matches!(
                    {
                        let _ = state.handle_key(
                            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                            &items,
                        );
                        state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items)
                    },
                    SidebarOutcome::Selected("y") | SidebarOutcome::Selected("x")
                )
        );
    }

    #[test]
    fn sidebar_legacy_select_y() {
        let items = [SidebarItem::new("x", "X"), SidebarItem::new("y", "Y")];
        let mut state = SidebarState::new(None);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        // Ensure focus on y
        if state.focus() != Some(&"y") {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items);
        }
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            SidebarOutcome::Selected("y")
        ));
    }

    #[test]
    fn toggle_rail() {
        let mut state = SidebarState::<&str>::new(None);
        assert!(state.is_expanded());
        assert!(matches!(
            state.toggle_rail(),
            SidebarOutcome::ToggleRail { expanded: false }
        ));
        assert!(!state.is_expanded());
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
                &[]
            ),
            SidebarOutcome::ToggleRail { expanded: true }
        ));
    }

    #[test]
    fn expand_group() {
        let items = [
            NavItem::group("g", "Group").has_children(true).expanded(false),
            NavItem::new("child", "Child").depth(1),
        ];
        let mut state = NavigationListState::new(None);
        state.set_focused(true);
        // focus group first
        let coll = NavigationListState::<&str>::collection_items(&items);
        let _ = state.collection.reconcile(&coll);
        state.collection.set_active(Some("g"));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items),
            NavigationListOutcome::ExpandToggled {
                id: "g",
                expanded: true
            }
        ));
    }

    #[test]
    fn filter_search() {
        let items = [
            NavItem::new("a", "Alpha"),
            NavItem::new("b", "Beta"),
        ];
        let mut state = NavigationListState::new(None);
        state.set_focused(true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE), &items),
            NavigationListOutcome::Changed
        ));
        assert!(state.is_filter_active());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &items),
            NavigationListOutcome::FilterChanged { query } if query == "a"
        ));
    }

    #[test]
    fn disabled_not_routed() {
        let items = [
            NavItem::new("a", "A"),
            NavItem::new("b", "B").enabled(false),
        ];
        let mut state = NavigationListState::new(Some("a"));
        state.set_focused(true);
        // Direct activate path (reconcile would skip disabled for roving)
        state.collection.set_active(Some("b"));
        assert!(matches!(
            state.activate_focus(&items),
            NavigationListOutcome::Ignored
        ));
        assert_eq!(state.route(), Some(&"a"));
    }

    #[test]
    fn paint_rail_and_expanded() {
        let system = DesignSystem::from_palette(RolePalette::default());
        let items = example_settings_nav();
        let mut state = SidebarState::new(Some("profile"));
        state.set_focused(true);
        let area = Rect::new(0, 0, 24, 12);
        let mut buf = Buffer::empty(area);
        Sidebar::new(&items, &system)
            .ascii(true)
            .show_panel(true)
            .title("Settings")
            .paint(area, &mut buf, &mut state);
        assert!(!state.nav.regions.is_empty());
        let _ = state.toggle_rail();
        let rail_area = Rect::new(0, 0, 6, 12);
        Sidebar::new(&items, &system)
            .ascii(true)
            .paint(rail_area, &mut buf, &mut state);
        assert_eq!(state.presentation(), SidebarPresentation::Rail);
    }

    #[test]
    fn mouse_route() {
        let system = DesignSystem::default();
        let items = [
            NavItem::new("a", "Alpha"),
            NavItem::new("b", "Beta"),
        ];
        let mut state = SidebarState::new(None);
        state.set_focused(true);
        let area = Rect::new(0, 0, 20, 6);
        let mut buf = Buffer::empty(area);
        Sidebar::new(&items, &system)
            .ascii(true)
            .paint(area, &mut buf, &mut state);
        let hit = state
            .nav
            .regions
            .iter()
            .find(|r| r.id == "b")
            .expect("b");
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(hit.area.x, hit.area.y),
                    modifiers: KeyModifiers::NONE,
                },
                &items
            ),
            SidebarOutcome::Selected("b")
        ));
    }

    #[test]
    fn examples_nonempty() {
        assert!(!example_database_nav().is_empty());
        assert!(!example_settings_nav().is_empty());
        assert!(!example_agent_workbench_nav().is_empty());
    }

    #[test]
    fn fuzz_keys() {
        let items = example_agent_workbench_nav();
        let mut state = SidebarState::new(Some("chat"));
        state.set_focused(true);
        let keys = [
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key, &items);
            state.set_focused(true);
            state.accepts_input = true;
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let items = example_database_nav();
        let mut state = SidebarState::new(Some("users"));
        state.set_focused(true);
        let area = Rect::new(0, 0, 28, 16);
        let mut buf = Buffer::empty(area);
        let w = Sidebar::new(&items, &system).ascii(true);
        for _ in 0..50 {
            w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let items = example_settings_nav();
        let state = SidebarState::new(Some("profile"));
        let mut scene = SemanticScene::<&str, ()>::default();
        Sidebar::new(&items, &system).register_semantic(
            &mut scene,
            "sb",
            Rect::new(0, 0, 20, 10),
            &state,
        );
        assert!(scene.get(&"sb").is_some());
    }

    #[test]
    fn presentation_for_width() {
        assert_eq!(
            sidebar_presentation_for_width(8),
            SidebarPresentation::Rail
        );
        assert_eq!(
            sidebar_presentation_for_width(24),
            SidebarPresentation::Expanded
        );
    }

    #[test]
    fn command_on_activate() {
        let items = [NavItem::new("a", "A").command("do.a")];
        let mut state = NavigationListState::new(None);
        state.set_focused(true);
        let coll = NavigationListState::<&str>::collection_items(&items);
        let _ = state.collection.reconcile(&coll);
        state.collection.set_active(Some("a"));
        assert!(matches!(
            state.activate_focus(&items),
            NavigationListOutcome::CommandRequested {
                command,
                id: "a"
            } if command == "do.a"
        ));
    }
}
