// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **DropdownMenu** and **ContextMenu** — command menus with nesting, state, shortcuts.
//!
//! **Mission.** Anchored or pointer-origin command menus with nested items,
//! checkbox/radio, separators, labels, disabled reasons, destructive rows,
//! loading / custom-preview placeholders, roving focus, typeahead, semantic
//! command keys, and generated shortcut hints. Geometry and nested dismiss
//! live on [`OverlayStack`].
//!
//! **vs MenuBar.** MenuBar owns a horizontal top strip; these widgets own a
//! single trigger- or pointer-opened cascade.
//! One canonical model serves flat, nested, and context-menu presentations.
//! **vs CommandPalette.** Deep or oversized cascades promote via
//! [`DropdownMenuOutcome::PreferCommandPalette`] + [`flatten_menu_nodes`].
//!
//! Research: Radix menus, desktop context menus, Textual, lazygit, file managers.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Modifier},
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionState, NavigationMove, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, RovingOrientation, SemanticNode,
        SemanticRole, SemanticScene, SemanticState, UiIntent, place_overlay,
    },
    style::{DesignSystem, Glyph, Role, VisualState},
    text::{display_cols, take_display_cols},
};

use super::menu_bar::{MenuCommandRef, MenuNode, MenuRowKind};

/// Default overlay id for dropdown root panels.
pub const DROPDOWN_MENU_OVERLAY_ID: &str = "termrock.dropdown-menu";
/// Nested submenu id prefix (`termrock.dropdown-menu.sub.N`).
pub const DROPDOWN_MENU_SUBMENU_PREFIX: &str = "termrock.dropdown-menu.sub";
/// Default overlay id for context-menu root panels.
pub const CONTEXT_MENU_OVERLAY_ID: &str = "termrock.context-menu";
/// Nested context submenu prefix.
pub const CONTEXT_MENU_SUBMENU_PREFIX: &str = "termrock.context-menu.sub";
/// Depth at or above which hosts should prefer palette pages.
pub const MENU_PROMOTE_MIN_DEPTH: usize = 4;
/// Item count that triggers palette promotion.
pub const MENU_PROMOTE_MAX_ITEMS: usize = 24;
/// Bounds width that forces palette promotion.
pub const MENU_PROMOTE_MAX_WIDTH: u16 = 36;
/// Bounds height that forces palette promotion.
pub const MENU_PROMOTE_MAX_HEIGHT: u16 = 14;

// ── Placement / open kind ───────────────────────────────────────────────────

/// How the root panel was / should be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MenuOpenTrigger {
    /// Keyboard from a focusable trigger (dropdown).
    #[default]
    Keyboard,
    /// Pointer primary click on trigger.
    Pointer,
    /// Right-click / secondary button.
    ContextPointer,
    /// Keyboard context key (host maps Menu / Shift+F10 → this).
    ContextKey,
}

impl MenuOpenTrigger {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Keyboard => "keyboard",
            Self::Pointer => "pointer",
            Self::ContextPointer => "context-pointer",
            Self::ContextKey => "context-key",
        }
    }

    /// Whether this trigger uses context-menu placement (AtOrigin).
    #[must_use]
    pub const fn is_context(self) -> bool {
        matches!(self, Self::ContextPointer | Self::ContextKey)
    }
}

/// Cascading panels vs command-palette promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DropdownMenuPresentation {
    /// Nested overlay panels (default).
    #[default]
    Cascading,
    /// Host should open CommandPalette with flattened commands.
    CommandPalette,
}

impl DropdownMenuPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Cascading => "cascading",
            Self::CommandPalette => "command-palette",
        }
    }
}

/// Choose presentation from bounds, tree size, and max depth.
#[must_use]
pub fn dropdown_menu_presentation_for(
    bounds: Rect,
    root: &[MenuNode<impl Sized>],
) -> DropdownMenuPresentation {
    if bounds.is_empty() {
        return DropdownMenuPresentation::Cascading;
    }
    if bounds.width <= MENU_PROMOTE_MAX_WIDTH || bounds.height <= MENU_PROMOTE_MAX_HEIGHT {
        return DropdownMenuPresentation::CommandPalette;
    }
    let (count, depth) = count_nodes(root, 1);
    if count > MENU_PROMOTE_MAX_ITEMS || depth >= MENU_PROMOTE_MIN_DEPTH {
        return DropdownMenuPresentation::CommandPalette;
    }
    DropdownMenuPresentation::Cascading
}

fn count_nodes<Id>(nodes: &[MenuNode<Id>], depth: usize) -> (usize, usize) {
    let mut count = 0usize;
    let mut max_d = depth;
    for n in nodes {
        match &n.kind {
            MenuRowKind::Separator
            | MenuRowKind::Section
            | MenuRowKind::Loading
            | MenuRowKind::CustomPreview => {}
            MenuRowKind::Submenu => {
                let (c, d) = count_nodes(&n.children, depth.saturating_add(1));
                count = count.saturating_add(c);
                max_d = max_d.max(d);
            }
            MenuRowKind::Command | MenuRowKind::Checkbox { .. } | MenuRowKind::Radio { .. } => {
                count = count.saturating_add(1);
            }
        }
    }
    (count, max_d)
}

/// Flatten activatable leaves for CommandPalette projection.
#[must_use]
pub fn flatten_menu_nodes<Id: Clone>(nodes: &[MenuNode<Id>]) -> Vec<MenuCommandRef<Id>> {
    let mut out = Vec::new();
    flatten_menu_nodes_into(nodes, "", &mut out);
    out
}

fn flatten_menu_nodes_into<Id: Clone>(
    nodes: &[MenuNode<Id>],
    prefix: &str,
    out: &mut Vec<MenuCommandRef<Id>>,
) {
    for n in nodes {
        match &n.kind {
            MenuRowKind::Separator
            | MenuRowKind::Section
            | MenuRowKind::Loading
            | MenuRowKind::CustomPreview => {}
            MenuRowKind::Submenu => {
                let next = if prefix.is_empty() {
                    n.label.clone()
                } else {
                    format!("{prefix} › {}", n.label)
                };
                flatten_menu_nodes_into(&n.children, &next, out);
            }
            MenuRowKind::Command | MenuRowKind::Checkbox { .. } | MenuRowKind::Radio { .. } => {
                let path_label = if prefix.is_empty() {
                    n.label.clone()
                } else {
                    format!("{prefix} › {}", n.label)
                };
                out.push(MenuCommandRef {
                    id: n.id.clone(),
                    path_label,
                    label: n.label.clone(),
                    command: n.command.clone(),
                    shortcut: n.shortcut.clone(),
                    enabled: n.enabled,
                    disabled_reason: n.disabled_reason.clone(),
                });
            }
        }
    }
}

/// Measure preferred overlay size for a panel.
#[must_use]
pub fn measure_menu_panel<Id>(items: &[MenuNode<Id>], ascii: bool) -> OverlaySize {
    let mut max_w = 14u16;
    let mut h = 2u16; // borders
    for item in items {
        h = h.saturating_add(1);
        let mark = 4u16;
        let label_w = display_cols(&item.label) as u16;
        let sc = item
            .shortcut
            .as_ref()
            .map(|s| display_cols(s) as u16 + 2)
            .unwrap_or(0);
        let sub = if matches!(item.kind, MenuRowKind::Submenu) {
            2
        } else {
            0
        };
        let reason = item
            .disabled_reason
            .as_ref()
            .map(|r| display_cols(r) as u16 + 3)
            .unwrap_or(0);
        let w = mark
            .saturating_add(label_w)
            .saturating_add(sc)
            .saturating_add(sub)
            .saturating_add(reason)
            .saturating_add(2);
        max_w = max_w.max(w);
        let _ = ascii;
    }
    OverlaySize {
        width: max_w.min(64),
        height: h.min(40),
        min_width: 12,
        min_height: 3,
        max_width: 72,
        max_height: 48,
    }
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Place dropdown below/start-aligned to anchor.
#[must_use]
pub fn place_dropdown_menu(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Menu),
    )
}

/// Place context menu at origin (pointer / context key).
#[must_use]
pub fn place_context_menu(bounds: Rect, origin: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(origin),
        size,
        OverlayPolicy::for_kind(OverlayKind::ContextMenu),
    )
}

/// Open root dropdown panel.
pub fn open_dropdown_menu_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let spec = OverlaySpec::menu(DROPDOWN_MENU_OVERLAY_ID, anchor, size, opener_focus);
    stack.open(bounds, spec)
}

/// Open root context-menu panel at origin.
pub fn open_context_menu_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    origin: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let spec = OverlaySpec::context_menu(CONTEXT_MENU_OVERLAY_ID, origin, size, opener_focus);
    stack.open(bounds, spec)
}

/// Open nested submenu under dropdown or context root.
pub fn open_menu_submenu_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    depth: usize,
    context: bool,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let (prefix, root_id) = if context {
        (CONTEXT_MENU_SUBMENU_PREFIX, CONTEXT_MENU_OVERLAY_ID)
    } else {
        (DROPDOWN_MENU_SUBMENU_PREFIX, DROPDOWN_MENU_OVERLAY_ID)
    };
    let id = format!("{prefix}.{depth}");
    let parent = if depth <= 1 {
        OverlayId::from_static(root_id)
    } else {
        OverlayId(format!("{prefix}.{}", depth.saturating_sub(1)))
    };
    let mut spec = OverlaySpec::menu(id, anchor, size, opener_focus).with_parent(parent);
    if context {
        // Keep AtOrigin-friendly flip for nested? Nested still Menu kind below parent row.
        let _ = &mut spec;
    }
    stack.open(bounds, spec)
}

/// Dismiss entire dropdown cascade.
pub fn dismiss_dropdown_menu_overlays<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(DROPDOWN_MENU_OVERLAY_ID))
}

/// Dismiss entire context-menu cascade.
pub fn dismiss_context_menu_overlays<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(CONTEXT_MENU_OVERLAY_ID))
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Typed outcomes (host runs commands / stack dismiss).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DropdownMenuOutcome<Id> {
    /// No change.
    Ignored,
    /// Cursor moved in a panel.
    CursorMoved,
    /// The pointer moved onto (or off) a row.
    HoverChanged,
    /// Root panel opened.
    Opened {
        /// Trigger used.
        trigger: MenuOpenTrigger,
    },
    /// Nested submenu opened.
    SubmenuOpened {
        /// Submenu node id.
        id: Id,
    },
    /// Leaf command activated.
    Activated {
        /// Node id.
        id: Id,
        /// Optional host command key.
        command: Option<String>,
    },
    /// Checkbox toggled (host flips model).
    CheckToggled {
        /// Node id.
        id: Id,
        /// Suggested new value.
        checked: bool,
    },
    /// Radio selected.
    RadioSelected {
        /// Node id.
        id: Id,
        /// Group key.
        group: String,
    },
    /// Typeahead jumped cursor.
    TypeaheadMatched,
    /// One cascade layer closed (Esc / Left).
    LayerClosed,
    /// Fully closed; restore opener focus via stack.
    Closed,
    /// Deep / oversized — host should open CommandPalette.
    PreferCommandPalette,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CascadeFrame {
    collection: CollectionState<usize>,
}

impl CascadeFrame {
    fn new() -> Self {
        Self {
            collection: CollectionState::new().orientation(RovingOrientation::Vertical),
        }
    }

    fn cursor(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    fn set_cursor(&mut self, idx: usize) {
        self.collection.set_active(Some(idx));
    }
}

/// Shared cascade state for dropdown and context menus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownMenuState {
    focused: bool,
    enabled: bool,
    accepts_input: bool,
    /// Open cascade (empty = closed).
    cascade: Vec<CascadeFrame>,
    /// Indices that opened nested frames after root.
    open_path: Vec<usize>,
    /// Context vs dropdown placement mode for stack helpers.
    context_mode: bool,
    /// Last open trigger.
    trigger: MenuOpenTrigger,
    presentation: DropdownMenuPresentation,
    presentation_override: Option<DropdownMenuPresentation>,
    /// Panel hits: (depth, item_index, rect).
    panel_hits: Vec<(usize, usize, Rect)>,
    /// (depth, item) the pointer is over. Hover washes; it never commits.
    hovered: Option<(usize, usize)>,
    /// Custom-preview hit rects for host paint.
    preview_hits: Vec<(usize, usize, Rect)>,
    /// Root panel origin for mouse.
    origin: (u16, u16),
    /// Typeahead buffer (also on collection; mirrored for diagnostics).
    typeahead: String,
}

impl Default for DropdownMenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl DropdownMenuState {
    /// Closed dropdown state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            focused: true,
            enabled: true,
            accepts_input: true,
            cascade: Vec::new(),
            open_path: Vec::new(),
            context_mode: false,
            trigger: MenuOpenTrigger::Keyboard,
            presentation: DropdownMenuPresentation::Cascading,
            presentation_override: None,
            panel_hits: Vec::new(),
            hovered: None,
            preview_hits: Vec::new(),
            origin: (0, 0),
            typeahead: String::new(),
        }
    }

    /// Context-menu factory (AtOrigin open helpers).
    #[must_use]
    pub fn context() -> Self {
        let mut s = Self::new();
        s.context_mode = true;
        s
    }

    /// Whether any panel is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.cascade.is_empty()
    }

    /// Cascade depth (0 = closed).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.cascade.len()
    }

    /// Context mode.
    #[must_use]
    pub const fn is_context_mode(&self) -> bool {
        self.context_mode
    }

    /// Last trigger.
    #[must_use]
    pub const fn trigger(&self) -> MenuOpenTrigger {
        self.trigger
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> DropdownMenuPresentation {
        self.presentation
    }

    /// Cursor at depth.
    #[must_use]
    pub fn panel_cursor(&self, depth: usize) -> Option<usize> {
        self.cascade.get(depth).map(CascadeFrame::cursor)
    }

    /// Root panel cursor.
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.panel_cursor(0).unwrap_or(0)
    }

    /// Typeahead buffer.
    #[must_use]
    pub fn typeahead_buffer(&self) -> &str {
        &self.typeahead
    }

    /// Custom-preview hits after paint: (depth, item_index, rect).
    #[must_use]
    pub fn preview_hits(&self) -> &[(usize, usize, Rect)] {
        &self.preview_hits
    }

    /// Panel hits.
    #[must_use]
    pub fn panel_hits(&self) -> &[(usize, usize, Rect)] {
        &self.panel_hits
    }

    /// Scene focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Enable.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Force context mode.
    pub fn set_context_mode(&mut self, on: bool) {
        self.context_mode = on;
    }

    /// Force presentation.
    pub fn set_presentation_override(&mut self, p: Option<DropdownMenuPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    fn live(&self) -> bool {
        self.enabled && self.accepts_input && self.focused
    }

    fn panel_entries<Id>(items: &[MenuNode<Id>]) -> Vec<CollectionItem<usize>> {
        items
            .iter()
            .enumerate()
            .map(|(i, n)| CollectionItem {
                id: i,
                enabled: n.is_activatable(),
                label: n.label.clone(),
                parent: None,
            })
            .collect()
    }

    fn items_at_path<'a, Id>(
        root: &'a [MenuNode<Id>],
        path: &[usize],
    ) -> Option<&'a [MenuNode<Id>]> {
        let mut items = root;
        for &idx in path {
            let node = items.get(idx)?;
            if !matches!(node.kind, MenuRowKind::Submenu) {
                return None;
            }
            items = node.children.as_slice();
        }
        Some(items)
    }

    fn current_items<'a, Id>(&self, root: &'a [MenuNode<Id>]) -> Option<&'a [MenuNode<Id>]> {
        if self.cascade.is_empty() {
            return None;
        }
        Self::items_at_path(root, &self.open_path)
    }

    fn ensure_top_frame<Id>(
        &mut self,
        root: &[MenuNode<Id>],
    ) -> Option<Vec<CollectionItem<usize>>> {
        let items = self.current_items(root)?;
        let entries = Self::panel_entries(items);
        if let Some(frame) = self.cascade.last_mut() {
            let _ = frame.collection.reconcile(&entries);
        }
        Some(entries)
    }

    /// Close without outcome.
    pub fn close_all(&mut self) {
        self.cascade.clear();
        self.open_path.clear();
        self.typeahead.clear();
    }

    /// Open root from a trigger; may emit PreferCommandPalette.
    pub fn open_with_trigger<Id: Clone>(
        &mut self,
        root: &[MenuNode<Id>],
        bounds: Rect,
        trigger: MenuOpenTrigger,
    ) -> DropdownMenuOutcome<Id> {
        if !self.enabled || root.is_empty() {
            return DropdownMenuOutcome::Ignored;
        }
        self.trigger = trigger;
        if trigger.is_context() {
            self.context_mode = true;
        }
        let presentation = self
            .presentation_override
            .unwrap_or_else(|| dropdown_menu_presentation_for(bounds, root));
        self.presentation = presentation;
        if matches!(presentation, DropdownMenuPresentation::CommandPalette) {
            return DropdownMenuOutcome::PreferCommandPalette;
        }
        let mut frame = CascadeFrame::new();
        let entries = Self::panel_entries(root);
        let _ = frame.collection.reconcile(&entries);
        self.cascade = vec![frame];
        self.open_path.clear();
        self.typeahead.clear();
        DropdownMenuOutcome::Opened { trigger }
    }

    /// Keyboard open (dropdown).
    pub fn open_from_keyboard<Id: Clone>(
        &mut self,
        root: &[MenuNode<Id>],
        bounds: Rect,
    ) -> DropdownMenuOutcome<Id> {
        self.context_mode = false;
        self.open_with_trigger(root, bounds, MenuOpenTrigger::Keyboard)
    }

    /// Pointer open on trigger.
    pub fn open_from_pointer<Id: Clone>(
        &mut self,
        root: &[MenuNode<Id>],
        bounds: Rect,
    ) -> DropdownMenuOutcome<Id> {
        self.context_mode = false;
        self.open_with_trigger(root, bounds, MenuOpenTrigger::Pointer)
    }

    /// Right-click open.
    pub fn open_from_context_pointer<Id: Clone>(
        &mut self,
        root: &[MenuNode<Id>],
        bounds: Rect,
    ) -> DropdownMenuOutcome<Id> {
        self.open_with_trigger(root, bounds, MenuOpenTrigger::ContextPointer)
    }

    /// Context-key open (host maps Menu key).
    pub fn open_from_context_key<Id: Clone>(
        &mut self,
        root: &[MenuNode<Id>],
        bounds: Rect,
    ) -> DropdownMenuOutcome<Id> {
        self.open_with_trigger(root, bounds, MenuOpenTrigger::ContextKey)
    }

    /// Open on OverlayStack after successful local open.
    pub fn open_on_stack<F: Clone, Id>(
        &self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        anchor_or_origin: Rect,
        root: &[MenuNode<Id>],
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        let size = measure_menu_panel(root, false);
        if self.context_mode {
            open_context_menu_overlay(stack, bounds, anchor_or_origin, size, opener_focus)
        } else {
            open_dropdown_menu_overlay(stack, bounds, anchor_or_origin, size, opener_focus)
        }
    }

    /// Close on stack (restores opener).
    pub fn close_on_stack<F: Clone>(&mut self, stack: &mut OverlayStack<F>) -> OverlayOutcome<F> {
        self.close_all();
        if self.context_mode {
            dismiss_context_menu_overlays(stack)
        } else {
            dismiss_dropdown_menu_overlays(stack)
        }
    }

    fn open_submenu_under_cursor<Id: Clone>(
        &mut self,
        root: &[MenuNode<Id>],
    ) -> DropdownMenuOutcome<Id> {
        let items = match self.current_items(root) {
            Some(i) => i,
            None => return DropdownMenuOutcome::Ignored,
        };
        let frame = match self.cascade.last() {
            Some(f) => f,
            None => return DropdownMenuOutcome::Ignored,
        };
        let idx = frame.cursor();
        let node = match items.get(idx) {
            Some(n) if n.is_activatable() && matches!(n.kind, MenuRowKind::Submenu) => n,
            _ => return DropdownMenuOutcome::Ignored,
        };
        if node.children.is_empty() {
            return DropdownMenuOutcome::Ignored;
        }
        // Promote if nested would exceed depth budget.
        let next_depth = self.cascade.len().saturating_add(1);
        if next_depth >= MENU_PROMOTE_MIN_DEPTH {
            return DropdownMenuOutcome::PreferCommandPalette;
        }
        let id = node.id.clone();
        let children = node.children.clone();
        self.open_path.push(idx);
        let mut child = CascadeFrame::new();
        let entries = Self::panel_entries(&children);
        let _ = child.collection.reconcile(&entries);
        self.cascade.push(child);
        self.typeahead.clear();
        DropdownMenuOutcome::SubmenuOpened { id }
    }

    fn close_one_layer<Id: Clone>(&mut self) -> DropdownMenuOutcome<Id> {
        if self.cascade.is_empty() {
            return DropdownMenuOutcome::Ignored;
        }
        self.cascade.pop();
        if !self.open_path.is_empty() {
            self.open_path.pop();
        }
        self.typeahead.clear();
        if self.cascade.is_empty() {
            self.open_path.clear();
            DropdownMenuOutcome::Closed
        } else {
            DropdownMenuOutcome::LayerClosed
        }
    }

    fn activate_cursor<Id: Clone>(&mut self, root: &[MenuNode<Id>]) -> DropdownMenuOutcome<Id> {
        let items = match self.current_items(root) {
            Some(i) => i,
            None => return DropdownMenuOutcome::Ignored,
        };
        let idx = match self.cascade.last() {
            Some(f) => f.cursor(),
            None => return DropdownMenuOutcome::Ignored,
        };
        let node = match items.get(idx) {
            Some(n) if n.is_activatable() => n,
            _ => return DropdownMenuOutcome::Ignored,
        };
        match &node.kind {
            MenuRowKind::Submenu => self.open_submenu_under_cursor(root),
            MenuRowKind::Checkbox { checked } => {
                let id = node.id.clone();
                let next = !*checked;
                self.close_all();
                DropdownMenuOutcome::CheckToggled { id, checked: next }
            }
            MenuRowKind::Radio { group, .. } => {
                let id = node.id.clone();
                let group = group.clone();
                self.close_all();
                DropdownMenuOutcome::RadioSelected { id, group }
            }
            MenuRowKind::Command => {
                let id = node.id.clone();
                let command = node.command.clone();
                self.close_all();
                DropdownMenuOutcome::Activated { id, command }
            }
            MenuRowKind::Separator
            | MenuRowKind::Section
            | MenuRowKind::Loading
            | MenuRowKind::CustomPreview => DropdownMenuOutcome::Ignored,
        }
    }

    /// Keyboard.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        root: &[MenuNode<Id>],
    ) -> DropdownMenuOutcome<Id> {
        if !self.live() || root.is_empty() || key.kind == KeyEventKind::Release {
            return DropdownMenuOutcome::Ignored;
        }
        if !self.is_open() {
            return DropdownMenuOutcome::Ignored;
        }
        let Some(entries) = self.ensure_top_frame(root) else {
            return DropdownMenuOutcome::Ignored;
        };

        // Left/Right for cascade (beyond default_menu_intent).
        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::NONE {
            match key.code {
                KeyCode::Right | KeyCode::Char('l' | 'L') => {
                    let out = self.open_submenu_under_cursor(root);
                    if !matches!(out, DropdownMenuOutcome::Ignored) {
                        return out;
                    }
                }
                KeyCode::Left | KeyCode::Char('h' | 'H') => {
                    if self.cascade.len() > 1 {
                        return self.close_one_layer();
                    }
                }
                _ => {}
            }
        }

        if let Some(intent) = crate::interaction::default_menu_intent(key) {
            let out = self.handle_intent_after_ensure(intent, root, &entries);
            if !matches!(out, DropdownMenuOutcome::Ignored) {
                return out;
            }
        }

        // Typeahead printable (no modifiers).
        if key.modifiers.is_empty() {
            if let KeyCode::Char(c) = key.code {
                if !c.is_control() {
                    return self.typeahead_char(c, &entries);
                }
            }
        }
        DropdownMenuOutcome::Ignored
    }

    fn typeahead_char<Id: Clone>(
        &mut self,
        ch: char,
        entries: &[CollectionItem<usize>],
    ) -> DropdownMenuOutcome<Id> {
        let frame = match self.cascade.last_mut() {
            Some(f) => f,
            None => return DropdownMenuOutcome::Ignored,
        };
        let before = frame.cursor();
        // CollectionState/roving typeahead via handle_key on synthetic char? Use roving API.
        let out = frame.collection.handle_key(
            KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
            entries,
        );
        self.typeahead.push(ch);
        if out.active_changed() || frame.cursor() != before {
            DropdownMenuOutcome::TypeaheadMatched
        } else {
            // Reset buffer if no match (roving may clear internally).
            DropdownMenuOutcome::Ignored
        }
    }

    /// Intent routing.
    pub fn handle_intent<Id: Clone>(
        &mut self,
        intent: UiIntent,
        root: &[MenuNode<Id>],
    ) -> DropdownMenuOutcome<Id> {
        if !self.live() || !self.is_open() || root.is_empty() {
            return DropdownMenuOutcome::Ignored;
        }
        let Some(entries) = self.ensure_top_frame(root) else {
            return DropdownMenuOutcome::Ignored;
        };
        self.handle_intent_after_ensure(intent, root, &entries)
    }

    fn handle_intent_after_ensure<Id: Clone>(
        &mut self,
        intent: UiIntent,
        root: &[MenuNode<Id>],
        entries: &[CollectionItem<usize>],
    ) -> DropdownMenuOutcome<Id> {
        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Previous
                | NavigationMove::First
                | NavigationMove::Last
                | NavigationMove::Up
                | NavigationMove::Down,
            ) => {
                let frame = match self.cascade.last_mut() {
                    Some(f) => f,
                    None => return DropdownMenuOutcome::Ignored,
                };
                let out = frame.collection.handle_intent(intent, entries);
                self.typeahead.clear();
                if out.active_changed() {
                    DropdownMenuOutcome::CursorMoved
                } else {
                    DropdownMenuOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Right) => self.open_submenu_under_cursor(root),
            UiIntent::Move(NavigationMove::Left) => {
                if self.cascade.len() > 1 {
                    self.close_one_layer()
                } else {
                    DropdownMenuOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                self.typeahead.clear();
                self.activate_cursor(root)
            }
            UiIntent::Cancel | UiIntent::Close => self.close_one_layer(),
            _ => DropdownMenuOutcome::Ignored,
        }
    }

    /// Mouse on painted panels.
    pub fn handle_mouse<Id: Clone>(
        &mut self,
        event: MouseEvent,
        root: &[MenuNode<Id>],
    ) -> DropdownMenuOutcome<Id> {
        if !self.live() || !self.is_open() || root.is_empty() {
            return DropdownMenuOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Moved => {
                // Hover is stated every event, so leaving a panel clears it.
                let was = self.hovered;
                self.hovered = self
                    .panel_hits
                    .iter()
                    .rev()
                    .find(|(_, _, rect)| rect.contains(event.position))
                    .map(|(depth, idx, _)| (*depth, *idx));
                if was == self.hovered {
                    DropdownMenuOutcome::Ignored
                } else {
                    DropdownMenuOutcome::HoverChanged
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = event.position;
                // Find deepest hit first.
                let mut hit: Option<(usize, usize)> = None;
                for &(depth, idx, rect) in self.panel_hits.iter().rev() {
                    if rect.contains(pos) {
                        hit = Some((depth, idx));
                        break;
                    }
                }
                let Some((depth, idx)) = hit else {
                    return DropdownMenuOutcome::Ignored;
                };
                // Peel cascade to depth.
                while self.cascade.len() > depth.saturating_add(1) {
                    let _ = self.close_one_layer::<Id>();
                }
                while self.open_path.len() > depth {
                    self.open_path.pop();
                }
                let _ = self.ensure_top_frame(root);
                if let Some(frame) = self.cascade.get_mut(depth) {
                    frame.set_cursor(idx);
                }
                // Resolve node at path + idx
                let path: Vec<usize> = self.open_path.iter().copied().take(depth).collect();
                let items = match Self::items_at_path(root, &path) {
                    Some(i) => i,
                    None => return DropdownMenuOutcome::CursorMoved,
                };
                let node = match items.get(idx) {
                    Some(n) => n,
                    None => return DropdownMenuOutcome::CursorMoved,
                };
                if !node.is_activatable() {
                    return DropdownMenuOutcome::CursorMoved;
                }
                if matches!(node.kind, MenuRowKind::Submenu) {
                    return self.open_submenu_under_cursor(root);
                }
                // Second click semantics: activate immediately on left down.
                self.activate_cursor(root)
            }
            MouseEventKind::Down(MouseButton::Right) => {
                // Nested right-click ignored while open; host re-opens root.
                DropdownMenuOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), root)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), root)
            }
            _ => DropdownMenuOutcome::Ignored,
        }
    }

    /// Sync open flag with stack presence for root id.
    pub fn sync_with_stack<F>(&mut self, stack: &OverlayStack<F>) {
        let root = if self.context_mode {
            CONTEXT_MENU_OVERLAY_ID
        } else {
            DROPDOWN_MENU_OVERLAY_ID
        };
        let id = OverlayId::from_static(root);
        if !stack.contains(&id) {
            self.close_all();
        } else {
            self.accepts_input = stack.top_owns_input()
                && stack
                    .top()
                    .is_some_and(|t| t.id.0.starts_with(root) || t.id == id);
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Dropdown / context menu panel paint (single panel or host paints cascade).
#[derive(Debug, Clone, Copy)]
pub struct DropdownMenu<'a, Id> {
    items: &'a [MenuNode<Id>],
    system: &'a DesignSystem,
    colorless: bool,
    /// When painting nested cascade, which depth this call targets.
    depth: usize,
}

impl<'a, Id> DropdownMenu<'a, Id> {
    /// Root items + design system.
    #[must_use]
    pub const fn new(items: &'a [MenuNode<Id>], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            colorless: false,
            depth: 0,
        }
    }

    /// Reduced-color roles.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint cascade depth (0 = root panel of open path).
    #[must_use]
    pub const fn depth(mut self, d: usize) -> Self {
        self.depth = d;
        self
    }

    /// Paint one panel for `state`'s items at `self.depth` into `area`.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut DropdownMenuState)
    where
        Id: Clone,
    {
        if self.depth == 0 {
            state.panel_hits.clear();
            state.preview_hits.clear();
            state.origin = (area.x, area.y);
        }
        if area.is_empty() || !state.is_open() {
            return;
        }
        let path: Vec<usize> = state.open_path.iter().copied().take(self.depth).collect();
        let items = match DropdownMenuState::items_at_path(self.items, &path) {
            Some(i) => i,
            None => return,
        };
        self.paint_items(area, buffer, state, items, self.depth);
    }

    /// Paint all open cascade panels stacked to the right of `root_area`.
    pub fn paint_cascade(
        &self,
        root_area: Rect,
        bounds: Rect,
        buffer: &mut Buffer,
        state: &mut DropdownMenuState,
    ) where
        Id: Clone,
    {
        state.panel_hits.clear();
        state.preview_hits.clear();
        if !state.is_open() || root_area.is_empty() {
            return;
        }
        let mut area = root_area;
        for depth in 0..state.depth() {
            let path: Vec<usize> = state.open_path.iter().copied().take(depth).collect();
            let items = match DropdownMenuState::items_at_path(self.items, &path) {
                Some(i) => i,
                None => break,
            };
            let size = measure_menu_panel(items, false);
            if depth == 0 {
                // Host usually places root; clamp to area.
                let placed = if state.context_mode {
                    place_context_menu(bounds, area, size)
                } else {
                    place_dropdown_menu(bounds, area, size)
                };
                area = if placed.is_empty() { root_area } else { placed };
            } else {
                // Anchor to previous cursor row hit if available.
                let anchor = state
                    .panel_hits
                    .iter()
                    .rev()
                    .find(|(d, idx, _)| {
                        *d == depth.saturating_sub(1)
                            && state.panel_cursor(depth.saturating_sub(1)) == Some(*idx)
                    })
                    .map(|(_, _, r)| *r)
                    .unwrap_or(area);
                let placed = place_dropdown_menu(bounds, anchor, size);
                area = if placed.is_empty() {
                    // Fall right of previous panel.
                    Rect::new(
                        area.right()
                            .saturating_add(1)
                            .min(bounds.right().saturating_sub(size.width)),
                        area.y,
                        size.width.min(bounds.width),
                        size.height.min(bounds.height),
                    )
                } else {
                    placed
                };
            }
            self.paint_items(area, buffer, state, items, depth);
        }
    }

    fn paint_items(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut DropdownMenuState,
        items: &[MenuNode<Id>],
        depth: usize,
    ) {
        if area.is_empty() {
            return;
        }
        let recipe = super::SurfaceRecipe::MenuPopover;
        let colorless_system;
        let surface_system = if self.colorless {
            colorless_system = self
                .system
                .clone()
                .capability(crate::style::ColorCapability::Monochrome);
            &colorless_system
        } else {
            self.system
        };
        let inner = super::Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .content_inset()
            .paint(area, buffer);
        if inner.is_empty() {
            return;
        }

        let cursor = state.panel_cursor(depth).unwrap_or(0);
        let surface_focus = state.live();

        // A menu longer than its panel used to paint until it ran out of rows
        // and drop the rest in silence — including the row the cursor was on,
        // which could sit below the fold with nothing on screen moving. The
        // frame already owns a `CollectionState`; it just was never asked
        // (plans/022 Step 5).
        let viewport = usize::from(inner.height);
        let stored = state
            .cascade
            .get(depth)
            .map_or(0, |frame| frame.collection.offset());
        let offset = crate::scroll::cursor_follow_offset(cursor, items.len(), viewport, stored);
        if let Some(frame) = state.cascade.get_mut(depth) {
            frame.collection.set_viewport(offset, viewport, items.len());
        }
        let gutter = Rect::new(inner.right().saturating_sub(1), inner.y, 1, inner.height);

        let mut y = inner.y;
        for (i, item) in items.iter().enumerate().skip(offset) {
            if y >= inner.bottom() {
                break;
            }
            let hit = Rect::new(inner.x, y, inner.width, 1);
            state.panel_hits.push((depth, i, hit));

            if matches!(item.kind, MenuRowKind::Separator) {
                // A separator that stops one cell short of the panel border
                // leaves a visible notch; it meets the border with a tee
                // instead (plans/022 Step 2).
                let glyphs = self.system.glyphs;
                let rule = { glyphs.resolve(Glyph::RuleH).text };
                let line: String = std::iter::repeat_n(rule, usize::from(inner.width)).collect();
                buffer.set_stringn(
                    inner.x,
                    y,
                    &line,
                    usize::from(inner.width),
                    self.system.style(Role::Border),
                );
                y = y.saturating_add(1);
                continue;
            }
            if matches!(item.kind, MenuRowKind::Section) {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(&item.label, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
                continue;
            }
            if matches!(item.kind, MenuRowKind::Loading) {
                let prefix = { "… " };
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(
                        &format!("{prefix}{}", item.label),
                        usize::from(inner.width),
                    ),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
                continue;
            }
            if matches!(item.kind, MenuRowKind::CustomPreview) {
                state.preview_hits.push((depth, i, hit));
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(&item.label, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
                continue;
            }

            let active = cursor == i && surface_focus;
            let style = if self.colorless {
                if !item.enabled {
                    self.system.style(Role::TextMuted)
                } else if active {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                }
            } else {
                self.system.menu_row(
                    VisualState {
                        selected: active,
                        hovered: state.hovered == Some((depth, i)),
                        disabled: !item.enabled,
                        ..VisualState::default()
                    },
                    item.destructive,
                    surface_system
                        .style(Role::Popover)
                        .bg
                        .unwrap_or(Color::Reset),
                )
            };
            if !self.colorless {
                buffer.set_style(hit, style);
            }

            let mark = match &item.kind {
                MenuRowKind::Checkbox { checked: true } if false => "[x] ",
                MenuRowKind::Checkbox { checked: true } => "✓ ",
                MenuRowKind::Checkbox { checked: false } if false => "[ ] ",
                MenuRowKind::Checkbox { checked: false } => "  ",
                MenuRowKind::Radio { selected: true, .. } if false => "(*) ",
                MenuRowKind::Radio { selected: true, .. } => "● ",
                MenuRowKind::Radio {
                    selected: false, ..
                } if false => "( ) ",
                MenuRowKind::Radio {
                    selected: false, ..
                } => "○ ",
                _ if active && false => "> ",
                _ if active => "› ",
                _ => "  ",
            };
            let label = format_mnemonic_label(&item.label, item.mnemonic, false);
            let mut line = format!("{mark}{label}");
            if matches!(item.kind, MenuRowKind::Submenu) {
                line.push('›');
            }
            if !item.enabled {
                if let Some(reason) = &item.disabled_reason {
                    line.push(' ');
                    line.push('(');
                    line.push_str(reason);
                    line.push(')');
                } else {
                    line.push_str(" ⊘");
                }
            }
            if let Some(sc) = &item.shortcut {
                let used = display_cols(&line);
                let sc_w = display_cols(sc);
                let pad = usize::from(inner.width)
                    .saturating_sub(used)
                    .saturating_sub(sc_w);
                if pad > 1 {
                    line.push_str(&" ".repeat(pad));
                    line.push_str(sc);
                }
            }
            buffer.set_stringn(
                inner.x,
                y,
                &take_display_cols(&line, usize::from(inner.width)),
                usize::from(inner.width),
                style,
            );
            y = y.saturating_add(1);
        }

        // The gutter says the same thing every other scrolled surface in the
        // library says.
        crate::scroll::paint_scrolled_region(
            buffer,
            gutter,
            items.len(),
            viewport,
            u16::try_from(offset).unwrap_or(u16::MAX),
            self.system,
        );
    }

    /// Semantic registration for open menu.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &DropdownMenuState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() || !state.is_open() {
            return;
        }
        let desc = format!(
            "menu depth={} context={} trigger={} presentation={}",
            state.depth(),
            state.context_mode,
            state.trigger.id(),
            state.presentation.id(),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Menu)
                .label("dropdown-menu")
                .description(desc)
                .focusable(state.enabled && state.accepts_input)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.is_open(),
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone> StatefulWidget for DropdownMenu<'_, Id> {
    type State = DropdownMenuState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone> StatefulWidget for &DropdownMenu<'_, Id> {
    type State = DropdownMenuState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

fn format_mnemonic_label(label: &str, mnemonic: Option<char>, _ascii: bool) -> String {
    let Some(m) = mnemonic else {
        return label.to_string();
    };
    let lower = m.to_ascii_lowercase();
    if let Some(pos) = label
        .char_indices()
        .find(|(_, c)| c.to_ascii_lowercase() == lower)
    {
        let (i, ch) = pos;
        let before = &label[..i];
        let after = &label[i + ch.len_utf8()..];
        format!("{before}({ch}){after}")
    } else {
        format!("{label} ({m})")
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn sample_tree() -> Vec<MenuNode<&'static str>> {
        vec![
            MenuNode::command("open", "Open")
                .shortcut("C-o")
                .mnemonic('O'),
            MenuNode::command("save", "Save").shortcut("C-s"),
            MenuNode::separator("sep1"),
            MenuNode::checkbox("wrap", "Word wrap", true),
            MenuNode::radio("theme-dark", "Dark", "theme", true),
            MenuNode::radio("theme-light", "Light", "theme", false),
            MenuNode::section("sec", "Recent"),
            MenuNode::submenu(
                "export",
                "Export",
                vec![
                    MenuNode::command("export-pdf", "PDF"),
                    MenuNode::submenu(
                        "export-img",
                        "Image",
                        vec![
                            MenuNode::command("png", "PNG"),
                            MenuNode::command("svg", "SVG"),
                        ],
                    ),
                ],
            ),
            MenuNode::loading("load", "Fetching…"),
            MenuNode::custom_preview("preview", "Thumbnail"),
            MenuNode::command("delete", "Delete")
                .destructive(true)
                .enabled(false)
                .disabled_reason("locked"),
        ]
    }

    #[test]
    fn open_close_and_activate() {
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        assert!(matches!(
            state.open_from_keyboard(&root, bounds),
            DropdownMenuOutcome::Opened {
                trigger: MenuOpenTrigger::Keyboard
            }
        ));
        assert!(state.is_open());
        // Move to Save (index 1) — skip none, open is 0
        let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &root);
        assert_eq!(state.cursor_index(), 1);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &root),
            DropdownMenuOutcome::Activated {
                id: "save",
                command: None
            }
        ));
        assert!(!state.is_open());
    }

    #[test]
    fn nested_submenu_and_layer_dismiss() {
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open_from_keyboard(&root, bounds);
        // Jump to Export via typeahead 'e' might hit export — move manually
        // indices: 0 open, 1 save, 2 sep, 3 wrap, 4 dark, 5 light, 6 sec, 7 export
        if let Some(frame) = state.cascade.last_mut() {
            frame.set_cursor(7);
        }
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &root),
            DropdownMenuOutcome::SubmenuOpened { id: "export" }
        ));
        assert_eq!(state.depth(), 2);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &root),
            DropdownMenuOutcome::LayerClosed
        ));
        assert_eq!(state.depth(), 1);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &root),
            DropdownMenuOutcome::Closed
        ));
    }

    #[test]
    fn exhaustive_nested_overlay_stack() {
        let root = sample_tree();
        let bounds = Rect::new(0, 0, 100, 30);
        let anchor = Rect::new(5, 2, 10, 1);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = DropdownMenuState::new();
        let out = state.open_from_keyboard(&root, bounds);
        assert!(matches!(out, DropdownMenuOutcome::Opened { .. }));
        let size = measure_menu_panel(&root, false);
        let o = open_dropdown_menu_overlay(&mut stack, bounds, anchor, size, Some("trigger"));
        assert!(matches!(o, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Menu);

        // Open first submenu on stack
        if let Some(frame) = state.cascade.last_mut() {
            frame.set_cursor(7);
        }
        assert!(matches!(
            state.open_submenu_under_cursor(&root),
            DropdownMenuOutcome::SubmenuOpened { id: "export" }
        ));
        let sub_items = state.current_items(&root).unwrap();
        let sub_size = measure_menu_panel(sub_items, false);
        let sub_anchor = Rect::new(20, 8, 12, 1);
        let s1 = open_menu_submenu_overlay(
            &mut stack,
            bounds,
            sub_anchor,
            sub_size,
            1,
            false,
            Some("trigger"),
        );
        assert!(matches!(s1, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.entries().len(), 2);

        // Deeper submenu
        if let Some(frame) = state.cascade.last_mut() {
            // export-pdf=0, export-img=1
            frame.set_cursor(1);
        }
        assert!(matches!(
            state.open_submenu_under_cursor(&root),
            DropdownMenuOutcome::SubmenuOpened { id: "export-img" }
        ));
        let deep = state.current_items(&root).unwrap();
        let dsize = measure_menu_panel(deep, false);
        let s2 = open_menu_submenu_overlay(
            &mut stack,
            bounds,
            Rect::new(40, 10, 8, 1),
            dsize,
            2,
            false,
            Some("trigger"),
        );
        assert!(matches!(s2, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.entries().len(), 3);

        // Dismiss root peels all children
        let d = dismiss_dropdown_menu_overlays(&mut stack);
        assert!(matches!(
            d,
            OverlayOutcome::Dismissed {
                focus: Some("trigger"),
                ..
            }
        ));
        assert!(stack.is_empty());
        state.sync_with_stack(&stack);
        assert!(!state.is_open());
    }

    #[test]
    fn context_menu_at_origin_and_outside_dismiss() {
        let root = vec![MenuNode::command("a", "A"), MenuNode::command("b", "B")];
        let bounds = Rect::new(0, 0, 80, 24);
        let origin = Rect::new(30, 12, 1, 1);
        let mut stack = OverlayStack::<()>::new();
        let mut state = DropdownMenuState::context();
        let _ = state.open_from_context_pointer(&root, bounds);
        let size = measure_menu_panel(&root, false);
        let _ = open_context_menu_overlay(&mut stack, bounds, origin, size, None);
        assert_eq!(stack.top().unwrap().kind, OverlayKind::ContextMenu);
        let placed = place_context_menu(bounds, origin, size);
        assert_eq!(stack.top().unwrap().rect, placed);
        assert!(matches!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Dismissed { .. }
        ));
    }

    #[test]
    fn checkbox_radio_destructive_disabled() {
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open_from_keyboard(&root, bounds);
        if let Some(frame) = state.cascade.last_mut() {
            frame.set_cursor(3); // wrap checkbox
        }
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &root),
            DropdownMenuOutcome::CheckToggled {
                id: "wrap",
                checked: false
            }
        ));
        let _ = state.open_from_keyboard(&root, bounds);
        if let Some(frame) = state.cascade.last_mut() {
            frame.set_cursor(5); // light radio
        }
        assert!(matches!(
            state.handle_intent(UiIntent::Activate, &root),
            DropdownMenuOutcome::RadioSelected {
                id: "theme-light",
                group
            } if group == "theme"
        ));
        // disabled destructive is not activatable; direct activate at that index ignores.
        let _ = state.open_from_keyboard(&root, bounds);
        let delete_idx = root.iter().position(|n| n.id == "delete").unwrap();
        assert!(!root[delete_idx].is_activatable());
        // Roving never lands on disabled: move from start through End.
        let _ = state.handle_intent(UiIntent::Move(NavigationMove::Last), &root);
        let cur = state.cursor_index();
        assert!(
            root[cur].is_activatable(),
            "Last should land on activatable, got {}",
            root[cur].id
        );
        assert_ne!(root[cur].id, "delete");
    }

    #[test]
    fn disabled_menu_does_not_paint_active_cursor() {
        let system = DesignSystem::junie();
        let root = vec![MenuNode::command("off", "Unavailable")];
        let mut state = DropdownMenuState::new();
        let _ = state.open_from_keyboard(&root, Rect::new(0, 0, 80, 24));
        state.set_enabled(false);

        let area = Rect::new(0, 0, 32, 8);
        let mut buffer = Buffer::empty(area);
        DropdownMenu::new(&root, &system).paint(area, &mut buffer, &mut state);

        let row = state
            .panel_hits()
            .iter()
            .find(|(_, index, _)| *index == 0)
            .map(|(_, _, rect)| *rect)
            .expect("disabled row hit");
        let text: String = (row.x..row.right())
            .map(|x| buffer[(x, row.y)].symbol().to_string())
            .collect();
        assert!(
            !text.contains('›'),
            "disabled menu cannot paint cursor: {text:?}"
        );
    }

    #[test]
    fn typeahead_jumps() {
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open_from_keyboard(&root, bounds);
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), &root);
        assert!(
            matches!(
                out,
                DropdownMenuOutcome::TypeaheadMatched | DropdownMenuOutcome::CursorMoved
            ) || state.cursor_index() == 1,
            "typeahead s → save, out={out:?} cursor={}",
            state.cursor_index()
        );
    }

    #[test]
    fn palette_promotion_narrow_and_deep() {
        let root = sample_tree();
        assert_eq!(
            dropdown_menu_presentation_for(Rect::new(0, 0, 30, 24), &root),
            DropdownMenuPresentation::CommandPalette
        );
        let mut state = DropdownMenuState::new();
        assert!(matches!(
            state.open_from_keyboard(&root, Rect::new(0, 0, 30, 24)),
            DropdownMenuOutcome::PreferCommandPalette
        ));
        let flat = flatten_menu_nodes(&root);
        assert!(flat.iter().any(|c| c.id == "png"));
        assert!(flat.iter().any(|c| c.path_label.contains("Export")));
    }

    #[test]
    fn a_menu_taller_than_its_panel_scrolls_to_its_cursor() {
        let system = DesignSystem::default();
        // Below MENU_PROMOTE_MAX_ITEMS, so this stays a menu rather than being
        // promoted to the command palette.
        let root: Vec<MenuNode<&'static str>> = (0..20)
            .map(|i| {
                let label: &'static str = Box::leak(format!("Item {i}").into_boxed_str());
                let id: &'static str = Box::leak(format!("item-{i}").into_boxed_str());
                MenuNode::command(id, label)
            })
            .collect();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        // Production defaults correctly promote 30 commands to a palette. This
        // test exercises the explicit cascading override available to hosts
        // whose menu must remain anchored.
        state.set_presentation_override(Some(DropdownMenuPresentation::Cascading));
        assert!(matches!(
            state.open_from_keyboard(&root, bounds),
            DropdownMenuOutcome::Opened { .. }
        ));
        // A short panel: the menu cannot show all thirty rows.
        let area = Rect::new(0, 0, 24, 8);

        // Walk the cursor past the fold.
        for _ in 0..15 {
            let _ = state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &root);
        }
        let mut buf = Buffer::empty(area);
        DropdownMenu::new(&root, &system).paint(area, &mut buf, &mut state);

        let painted: String = (0..area.height)
            .flat_map(|y| (0..area.width).map(move |x| (x, y)))
            .map(|(x, y)| buf[(x, y)].symbol().to_string())
            .collect();
        assert!(
            painted.contains("Item 15"),
            "the cursor row must be on screen: {painted}"
        );
        assert!(
            !painted.contains("Item 0 "),
            "rows above the fold must scroll away: {painted}"
        );
        // Every hit region reported belongs to a row that was actually painted.
        for (_, index, rect) in state.panel_hits() {
            assert!(
                rect.y >= area.y && rect.y < area.bottom(),
                "{index} {rect:?}"
            );
        }
    }

    #[test]
    fn paint_slots_and_semantics() {
        let system = DesignSystem::default();
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open_from_keyboard(&root, bounds);
        let area = Rect::new(0, 0, 36, 16);
        let mut buf = Buffer::empty(area);
        DropdownMenu::new(&root, &system).paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // Mnemonic paints as "(O)pen".
        assert!(
            text.contains("Open") || text.contains("(O)pen") || text.contains("pen"),
            "{text}"
        );
        assert!(text.contains("C-o") || text.contains("Save"), "{text}");
        assert!(
            !state.preview_hits().is_empty() || text.contains("Thumbnail"),
            "{text}"
        );
        let mut scene = SemanticScene::<&str, ()>::default();
        DropdownMenu::new(&root, &system).register_semantic(&mut scene, "m", area, &state);
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("dropdown-menu"))
        );
    }

    #[test]
    fn render_uses_menu_highlights_and_soft_destructive_label() {
        let system = DesignSystem::junie();
        let root = vec![
            MenuNode::command("open", "Open"),
            MenuNode::command("delete", "Delete").destructive(true),
        ];
        let mut state = DropdownMenuState::new();
        let _ = state.open_from_keyboard(&root, Rect::new(0, 0, 80, 24));
        let area = Rect::new(0, 0, 32, 8);
        let mut buffer = Buffer::empty(area);
        DropdownMenu::new(&root, &system).paint(area, &mut buffer, &mut state);

        let active = state
            .panel_hits()
            .iter()
            .find(|(_, index, _)| *index == 0)
            .map(|(_, _, rect)| *rect)
            .expect("active menu row hit");
        let destructive = state
            .panel_hits()
            .iter()
            .find(|(_, index, _)| *index == 1)
            .map(|(_, _, rect)| *rect)
            .expect("destructive menu row hit");
        let active_style = buffer[(active.x + 2, active.y)].style();
        let highlight = system.style(Role::Highlight);
        assert_eq!(active_style.fg, highlight.fg);
        assert_eq!(active_style.bg, Some(Color::Rgb(0x2f, 0x5a, 0xa8)));
        assert_eq!(active_style.add_modifier, highlight.add_modifier);
        assert!(
            (0..active.width).all(|offset| buffer[(active.x + offset, active.y)].symbol() != "▎"),
            "menu selection has no focus gutter"
        );
        assert_eq!(
            buffer[(destructive.x + 2, destructive.y)].style().fg,
            Some(Color::Rgb(0xd9, 0x8a, 0x8a))
        );
    }

    #[test]
    fn fuzz_keys() {
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open_from_keyboard(&root, bounds);
        let keys = [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Right,
            KeyCode::Left,
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char('e'),
            KeyCode::Home,
            KeyCode::End,
        ];
        let mut seed = 7u64;
        for _ in 0..200 {
            if !state.is_open() {
                let _ = state.open_from_keyboard(&root, bounds);
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE), &root);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let _ = state.open_from_keyboard(&root, Rect::new(0, 0, 80, 24));
        let mut terminal = Terminal::new(TestBackend::new(40, 18)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..200 {
            terminal
                .draw(|f| {
                    DropdownMenu::new(&root, &system).paint(f.area(), f.buffer_mut(), &mut state);
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let root = vec![
            MenuNode::command("a", "Alpha").shortcut("A"),
            MenuNode::command("b", "Beta"),
        ];
        let mut state = DropdownMenuState::new();
        let _ = state.open_from_keyboard(&root, Rect::new(0, 0, 80, 24));
        let mut t1 = Terminal::new(TestBackend::new(28, 8)).unwrap();
        t1.draw(|f| {
            DropdownMenu::new(&root, &system).paint(f.area(), f.buffer_mut(), &mut state);
        })
        .unwrap();
        let s1: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut state2 = DropdownMenuState::new();
        let _ = state2.open_from_keyboard(&root, Rect::new(0, 0, 80, 24));
        let mut t2 = Terminal::new(TestBackend::new(28, 8)).unwrap();
        t2.draw(|f| {
            DropdownMenu::new(&root, &system).paint(f.area(), f.buffer_mut(), &mut state2);
        })
        .unwrap();
        let s2: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(s1, s2);
        assert!(s1.contains("Alpha"));
    }

    #[test]
    fn context_key_and_pointer_triggers() {
        let root = vec![MenuNode::command("x", "X")];
        let bounds = Rect::new(0, 0, 80, 24);
        let mut state = DropdownMenuState::new();
        assert!(matches!(
            state.open_from_context_key(&root, bounds),
            DropdownMenuOutcome::Opened {
                trigger: MenuOpenTrigger::ContextKey
            }
        ));
        assert!(state.is_context_mode());
        state.close_all();
        assert!(matches!(
            state.open_from_pointer(&root, bounds),
            DropdownMenuOutcome::Opened {
                trigger: MenuOpenTrigger::Pointer
            }
        ));
    }

    #[test]
    fn mouse_activates_only_enabled_painted_menu_rows() {
        let root = sample_tree();
        let mut state = DropdownMenuState::new();
        let _ = state.open_from_keyboard(&root, Rect::new(0, 0, 80, 24));
        state.panel_hits = vec![(0, 0, Rect::new(4, 5, 20, 1))];
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(4, 5),
                    modifiers: KeyModifiers::NONE,
                },
                &root,
            ),
            DropdownMenuOutcome::Activated { .. }
                | DropdownMenuOutcome::CheckToggled { .. }
                | DropdownMenuOutcome::RadioSelected { .. }
        ));

        let disabled = vec![MenuNode::command("off", "Unavailable").enabled(false)];
        let mut disabled_state = DropdownMenuState::new();
        let _ = disabled_state.open_from_keyboard(&disabled, Rect::new(0, 0, 80, 24));
        disabled_state.panel_hits = vec![(0, 0, Rect::new(4, 5, 20, 1))];
        assert_eq!(
            disabled_state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(4, 5),
                    modifiers: KeyModifiers::NONE,
                },
                &disabled,
            ),
            DropdownMenuOutcome::CursorMoved
        );
    }
}
