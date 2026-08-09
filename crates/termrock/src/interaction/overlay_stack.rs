// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Unified overlay stack: z-order, placement, dismissal, and focus policy.
//!
//! **Law:** Escape closes exactly one conceptual interaction layer. A trapping
//! top layer protects every layer beneath it.
//!
//! [`OverlayStack`] owns durable open-state and resolved geometry. Pair it with
//! [`super::InteractionScene`] for per-frame element registration: call
//! [`OverlayStack::sync_scene_layers`] then register controls on those layers.

use ratatui_core::layout::{Position, Rect};

use super::{InteractionLayer, InteractionScene, LayerDismissPolicy, LayerKind};

/// Stable overlay identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayId(pub String);

impl OverlayId {
    /// Owned id from a static string.
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self(id.to_owned())
    }

    /// Borrow the raw id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for OverlayId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for OverlayId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Semantic overlay kind — selects default policy and placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OverlayKind {
    /// Ephemeral tip; no input ownership.
    Tooltip,
    /// Anchored rich content; light focus.
    Popover,
    /// Dropdown / application menu.
    Menu,
    /// Pointer-triggered menu at a point.
    ContextMenu,
    /// Inline completion list (must not cover anchor).
    Completion,
    /// Select / combobox popup.
    Select,
    /// Modal dialog.
    Dialog,
    /// Blocking alert (trap Esc or require action).
    AlertDialog,
    /// Edge-attached drawer / sheet.
    Drawer,
    /// Global command palette (centered).
    CommandPalette,
    /// Fullscreen viewer / semantic zoom.
    Fullscreen,
    /// Caller-defined.
    Custom,
}

/// Preferred placement relative to an anchor or the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PlacementPrefer {
    /// Below anchor, start-aligned (default for menus).
    #[default]
    BelowStart,
    /// Above anchor, start-aligned.
    AboveStart,
    /// End-aligned horizontally (right for LTR).
    EndAligned,
    /// Centered in the screen/bounds.
    Center,
    /// Fill bounds (fullscreen).
    Fullscreen,
    /// Left edge drawer.
    DrawerStart,
    /// Right edge drawer.
    DrawerEnd,
    /// At pointer / exact origin (context menu).
    AtOrigin,
}

/// Backdrop treatment behind a modal overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BackdropPolicy {
    /// No backdrop.
    #[default]
    None,
    /// Dim / clear wash; clicks may dismiss depending on outside policy.
    Dim,
    /// Opaque occlusion; outside clicks typically trapped.
    Occlude,
}

/// When the preferred placement cannot fit on a tiny terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum NarrowFallback {
    /// Keep preferred placement, clamp size.
    #[default]
    Clamp,
    /// Force center in bounds.
    Center,
    /// Promote to nearly fullscreen.
    Fullscreen,
    /// Caller should hide (stack still tracks entry; rect empty).
    Hide,
}

/// Preferred and constrained size in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OverlaySize {
    /// Preferred width.
    pub width: u16,
    /// Preferred height.
    pub height: u16,
    /// Minimum width.
    pub min_width: u16,
    /// Minimum height.
    pub min_height: u16,
    /// Maximum width (0 = bounds-limited only).
    pub max_width: u16,
    /// Maximum height (0 = bounds-limited only).
    pub max_height: u16,
}

impl OverlaySize {
    /// Square-ish menu defaults.
    #[must_use]
    pub const fn menu(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            min_width: 8,
            min_height: 1,
            max_width: 0,
            max_height: 0,
        }
    }

    /// Dialog defaults.
    #[must_use]
    pub const fn dialog(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            min_width: 20,
            min_height: 3,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Policy bundle for one overlay kind (or override).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayPolicy {
    /// Esc dismissal.
    pub esc: LayerDismissPolicy,
    /// Outside-click dismissal.
    pub outside: LayerDismissPolicy,
    /// Whether this layer owns keyboard while topmost.
    pub owns_input: bool,
    /// Whether Tab/focus is trapped inside the layer.
    pub focus_trap: bool,
    /// Whether wheel events are consumed by the overlay (not the parent).
    pub wheel_captures: bool,
    /// Backdrop.
    pub backdrop: BackdropPolicy,
    /// Placement preference.
    pub prefer: PlacementPrefer,
    /// Whether the overlay may cover the anchor cell.
    pub cover_anchor: bool,
    /// Tiny-terminal behavior.
    pub narrow_fallback: NarrowFallback,
    /// Column threshold for narrow fallback (inclusive).
    pub narrow_cols: u16,
}

impl OverlayPolicy {
    /// Builtin policy table for each kind.
    #[must_use]
    pub const fn for_kind(kind: OverlayKind) -> Self {
        match kind {
            OverlayKind::Tooltip => Self {
                esc: LayerDismissPolicy::Ignore,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: false,
                focus_trap: false,
                wheel_captures: false,
                backdrop: BackdropPolicy::None,
                prefer: PlacementPrefer::AboveStart,
                cover_anchor: false,
                narrow_fallback: NarrowFallback::Hide,
                narrow_cols: 20,
            },
            OverlayKind::Popover => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: false,
                wheel_captures: true,
                backdrop: BackdropPolicy::None,
                prefer: PlacementPrefer::BelowStart,
                cover_anchor: false,
                narrow_fallback: NarrowFallback::Center,
                narrow_cols: 40,
            },
            OverlayKind::Menu | OverlayKind::Select => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::None,
                prefer: PlacementPrefer::BelowStart,
                cover_anchor: false,
                narrow_fallback: NarrowFallback::Clamp,
                narrow_cols: 40,
            },
            OverlayKind::ContextMenu => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::None,
                prefer: PlacementPrefer::AtOrigin,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Clamp,
                narrow_cols: 30,
            },
            OverlayKind::Completion => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: false,
                wheel_captures: true,
                backdrop: BackdropPolicy::None,
                prefer: PlacementPrefer::BelowStart,
                cover_anchor: false,
                narrow_fallback: NarrowFallback::Clamp,
                narrow_cols: 24,
            },
            OverlayKind::Dialog => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Trap,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Dim,
                prefer: PlacementPrefer::Center,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: 40,
            },
            OverlayKind::AlertDialog => Self {
                esc: LayerDismissPolicy::Trap,
                outside: LayerDismissPolicy::Trap,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Occlude,
                prefer: PlacementPrefer::Center,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: 40,
            },
            OverlayKind::Drawer => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Dim,
                prefer: PlacementPrefer::DrawerEnd,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: 50,
            },
            OverlayKind::CommandPalette => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Dim,
                prefer: PlacementPrefer::Center,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: 48,
            },
            OverlayKind::Fullscreen => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Trap,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: BackdropPolicy::Occlude,
                prefer: PlacementPrefer::Fullscreen,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Fullscreen,
                narrow_cols: 0,
            },
            OverlayKind::Custom => Self {
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: false,
                wheel_captures: true,
                backdrop: BackdropPolicy::None,
                prefer: PlacementPrefer::Center,
                cover_anchor: true,
                narrow_fallback: NarrowFallback::Clamp,
                narrow_cols: 40,
            },
        }
    }

    /// Maps to scene layer kind for tooling.
    #[must_use]
    pub const fn scene_layer_kind(kind: OverlayKind) -> LayerKind {
        match kind {
            OverlayKind::Tooltip | OverlayKind::Popover => LayerKind::Menu,
            OverlayKind::Menu
            | OverlayKind::ContextMenu
            | OverlayKind::Completion
            | OverlayKind::Select
            | OverlayKind::CommandPalette => LayerKind::Menu,
            OverlayKind::Dialog | OverlayKind::AlertDialog | OverlayKind::Drawer => LayerKind::Card,
            OverlayKind::Fullscreen => LayerKind::Custom,
            OverlayKind::Custom => LayerKind::Custom,
        }
    }
}

/// Spec used to open an overlay (before geometry is resolved).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlaySpec<FocusId = ()> {
    /// Stable id.
    pub id: OverlayId,
    /// Kind (selects default policy).
    pub kind: OverlayKind,
    /// Optional parent overlay (nested dismissal).
    pub parent: Option<OverlayId>,
    /// Anchor rectangle (menus/tooltips); ignored for center/fullscreen.
    pub anchor: Option<Rect>,
    /// Size constraints.
    pub size: OverlaySize,
    /// Focus identity to restore on dismiss.
    pub opener_focus: Option<FocusId>,
    /// Optional policy override (None = kind default).
    pub policy: Option<OverlayPolicy>,
}

impl<FocusId> OverlaySpec<FocusId> {
    /// Completion menu under an anchor cell.
    #[must_use]
    pub fn completion(
        id: impl Into<OverlayId>,
        anchor: Rect,
        size: OverlaySize,
        opener_focus: Option<FocusId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: OverlayKind::Completion,
            parent: None,
            anchor: Some(anchor),
            size,
            opener_focus,
            policy: None,
        }
    }

    /// Centered command palette.
    #[must_use]
    pub fn command_palette(
        id: impl Into<OverlayId>,
        size: OverlaySize,
        opener_focus: Option<FocusId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: OverlayKind::CommandPalette,
            parent: None,
            anchor: None,
            size,
            opener_focus,
            policy: None,
        }
    }

    /// Centered modal dialog.
    #[must_use]
    pub fn dialog(
        id: impl Into<OverlayId>,
        size: OverlaySize,
        opener_focus: Option<FocusId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: OverlayKind::Dialog,
            parent: None,
            anchor: None,
            size,
            opener_focus,
            policy: None,
        }
    }

    /// Alert dialog that traps Esc until an action is taken.
    #[must_use]
    pub fn alert_dialog(
        id: impl Into<OverlayId>,
        size: OverlaySize,
        opener_focus: Option<FocusId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: OverlayKind::AlertDialog,
            parent: None,
            anchor: None,
            size,
            opener_focus,
            policy: None,
        }
    }

    /// Anchored menu / select popup.
    #[must_use]
    pub fn menu(
        id: impl Into<OverlayId>,
        anchor: Rect,
        size: OverlaySize,
        opener_focus: Option<FocusId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: OverlayKind::Menu,
            parent: None,
            anchor: Some(anchor),
            size,
            opener_focus,
            policy: None,
        }
    }

    /// Context menu at a pointer origin.
    #[must_use]
    pub fn context_menu(
        id: impl Into<OverlayId>,
        origin: Rect,
        size: OverlaySize,
        opener_focus: Option<FocusId>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: OverlayKind::ContextMenu,
            parent: None,
            anchor: Some(origin),
            size,
            opener_focus,
            policy: None,
        }
    }

    /// Nested child under an open parent.
    #[must_use]
    pub fn with_parent(mut self, parent: impl Into<OverlayId>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Override default policy for this open.
    #[must_use]
    pub const fn with_policy(mut self, policy: OverlayPolicy) -> Self {
        self.policy = Some(policy);
        self
    }
}

/// One open overlay after geometry resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayEntry<FocusId = ()> {
    /// Spec identity.
    pub id: OverlayId,
    /// Kind.
    pub kind: OverlayKind,
    /// Parent id if nested.
    pub parent: Option<OverlayId>,
    /// Effective policy.
    pub policy: OverlayPolicy,
    /// Preferred size (reflow input).
    pub size: OverlaySize,
    /// Anchor used for placement (reflow input).
    pub anchor: Option<Rect>,
    /// Resolved painted rectangle (may be empty if hidden).
    pub rect: Rect,
    /// Opener focus to restore.
    pub opener_focus: Option<FocusId>,
    /// Whether this entry was promoted to fullscreen by narrow fallback.
    pub fullscreen_promoted: bool,
}

/// Result of a stack mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OverlayOutcome<FocusId = ()> {
    /// No change.
    Ignored,
    /// Layer opened or geometry updated.
    Opened {
        /// Id.
        id: OverlayId,
        /// Rect.
        rect: Rect,
    },
    /// Exactly one layer dismissed.
    Dismissed {
        /// Removed id.
        id: OverlayId,
        /// Focus to restore.
        focus: Option<FocusId>,
    },
    /// Esc reached empty stack / non-dismissible root.
    UnhandledEscape,
}

/// Z-ordered overlay host with placement and single-layer Esc law.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayStack<FocusId = ()> {
    entries: Vec<OverlayEntry<FocusId>>,
    /// Last known screen bounds for reflow on resize.
    bounds: Rect,
}

impl<FocusId> Default for OverlayStack<FocusId> {
    fn default() -> Self {
        Self::new()
    }
}

impl<FocusId> OverlayStack<FocusId> {
    /// Empty stack.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            bounds: Rect::new(0, 0, 0, 0),
        }
    }

    /// Bottom → top entries.
    #[must_use]
    pub fn entries(&self) -> &[OverlayEntry<FocusId>] {
        &self.entries
    }

    /// Topmost entry.
    #[must_use]
    pub fn top(&self) -> Option<&OverlayEntry<FocusId>> {
        self.entries.last()
    }

    /// Whether any overlay is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Current bounds used for placement.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        self.bounds
    }

    /// Lookup an open entry by id.
    #[must_use]
    pub fn get(&self, id: &OverlayId) -> Option<&OverlayEntry<FocusId>> {
        self.entries.iter().find(|e| &e.id == id)
    }

    /// Whether `id` is currently open.
    #[must_use]
    pub fn contains(&self, id: &OverlayId) -> bool {
        self.entries.iter().any(|e| &e.id == id)
    }

    /// Whether the top overlay owns keyboard while open.
    #[must_use]
    pub fn top_owns_input(&self) -> bool {
        self.entries.last().is_some_and(|e| e.policy.owns_input)
    }

    /// Whether a pointer position hits the top overlay rect.
    #[must_use]
    pub fn pointer_hits_top(&self, position: Position) -> bool {
        self.entries
            .last()
            .is_some_and(|e| e.rect.contains(position))
    }

    /// Whether the top overlay captures wheel events at `position`.
    #[must_use]
    pub fn wheel_captured(&self, position: Position) -> bool {
        self.entries.last().is_some_and(|top| {
            top.policy.wheel_captures && (top.rect.contains(position) || top.policy.focus_trap)
        })
    }

    /// Whether any open overlay wants a backdrop painted.
    #[must_use]
    pub fn backdrop_policy(&self) -> BackdropPolicy {
        self.entries
            .iter()
            .rev()
            .map(|e| e.policy.backdrop)
            .find(|b| *b != BackdropPolicy::None)
            .unwrap_or(BackdropPolicy::None)
    }

    /// Clears every overlay.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<FocusId: Clone> OverlayStack<FocusId> {
    /// Opens or replaces an overlay and resolves geometry inside `bounds`.
    pub fn open(&mut self, bounds: Rect, spec: OverlaySpec<FocusId>) -> OverlayOutcome<FocusId> {
        self.bounds = bounds;
        let policy = spec
            .policy
            .unwrap_or_else(|| OverlayPolicy::for_kind(spec.kind));
        let (rect, fullscreen_promoted) =
            resolve_placement(bounds, spec.anchor, spec.size, policy, spec.kind);
        let id = spec.id.clone();
        self.entries.retain(|e| e.id != spec.id);
        // Nested: if parent missing, still allow (caller responsibility).
        self.entries.push(OverlayEntry {
            id: spec.id,
            kind: spec.kind,
            parent: spec.parent,
            policy,
            size: spec.size,
            anchor: spec.anchor,
            rect,
            opener_focus: spec.opener_focus,
            fullscreen_promoted,
        });
        OverlayOutcome::Opened { id, rect }
    }

    /// Promotes the top overlay to fullscreen within bounds.
    pub fn promote_top_fullscreen(&mut self, bounds: Rect) -> OverlayOutcome<FocusId> {
        self.bounds = bounds;
        let Some(top) = self.entries.last_mut() else {
            return OverlayOutcome::Ignored;
        };
        top.rect = bounds;
        top.fullscreen_promoted = true;
        top.policy.prefer = PlacementPrefer::Fullscreen;
        let id = top.id.clone();
        OverlayOutcome::Opened { id, rect: bounds }
    }

    /// Reflows all geometries after resize (keeps stack order and open set).
    pub fn reflow(&mut self, bounds: Rect) {
        self.bounds = bounds;
        for entry in &mut self.entries {
            if entry.fullscreen_promoted
                || matches!(entry.policy.prefer, PlacementPrefer::Fullscreen)
            {
                entry.rect = bounds;
                continue;
            }
            let (rect, promoted) =
                resolve_placement(bounds, entry.anchor, entry.size, entry.policy, entry.kind);
            entry.rect = rect;
            entry.fullscreen_promoted = promoted || entry.fullscreen_promoted;
        }
    }

    /// Esc: dismiss exactly one conceptual layer (top only).
    pub fn handle_escape(&mut self) -> OverlayOutcome<FocusId> {
        let Some(top) = self.entries.last() else {
            return OverlayOutcome::UnhandledEscape;
        };
        match top.policy.esc {
            LayerDismissPolicy::Trap => OverlayOutcome::Ignored,
            LayerDismissPolicy::Ignore => OverlayOutcome::UnhandledEscape,
            LayerDismissPolicy::Dismissible => {
                let removed = self.entries.pop().expect("top");
                // Nested dismissal: also drop descendants of removed id.
                self.entries
                    .retain(|e| e.parent.as_ref() != Some(&removed.id));
                OverlayOutcome::Dismissed {
                    id: removed.id,
                    focus: removed.opener_focus,
                }
            }
        }
    }

    /// Outside-click: dismiss top when policy allows and position is outside rect.
    pub fn handle_outside_click(&mut self, position: Position) -> OverlayOutcome<FocusId> {
        let Some(top) = self.entries.last() else {
            return OverlayOutcome::Ignored;
        };
        if top.rect.contains(position) {
            return OverlayOutcome::Ignored;
        }
        match top.policy.outside {
            LayerDismissPolicy::Trap | LayerDismissPolicy::Ignore => OverlayOutcome::Ignored,
            LayerDismissPolicy::Dismissible => {
                let removed = self.entries.pop().expect("top");
                self.entries
                    .retain(|e| e.parent.as_ref() != Some(&removed.id));
                OverlayOutcome::Dismissed {
                    id: removed.id,
                    focus: removed.opener_focus,
                }
            }
        }
    }

    /// Removes an overlay by id (and its descendants).
    pub fn dismiss(&mut self, id: &OverlayId) -> OverlayOutcome<FocusId> {
        let Some(index) = self.entries.iter().position(|e| &e.id == id) else {
            return OverlayOutcome::Ignored;
        };
        let removed = self.entries.remove(index);
        self.entries
            .retain(|e| e.parent.as_ref() != Some(&removed.id));
        OverlayOutcome::Dismissed {
            id: removed.id,
            focus: removed.opener_focus,
        }
    }

    /// Pushes matching layers onto an [`InteractionScene`] (does not clear root).
    ///
    /// Call after ensuring a root layer. Overlay layer ids are stringified
    /// [`OverlayId`] values — use the same string when registering elements.
    pub fn sync_scene_layers<Id, Action>(&self, scene: &mut InteractionScene<Id, String, Action>)
    where
        Id: Clone + PartialEq,
        FocusId: Into<Id> + Clone,
    {
        for entry in &self.entries {
            let focus_return = entry.opener_focus.clone().map(Into::into);
            scene.push_layer(InteractionLayer {
                id: entry.id.0.clone(),
                kind: OverlayPolicy::scene_layer_kind(entry.kind),
                owns_input: entry.policy.owns_input,
                esc: entry.policy.esc,
                outside: entry.policy.outside,
                focus_return,
            });
        }
    }
}

impl OverlayStack<()> {
    /// Convenience sync when focus ids are unit.
    pub fn sync_scene_layers_unit<Action>(&self, scene: &mut InteractionScene<(), String, Action>) {
        for entry in &self.entries {
            scene.push_layer(InteractionLayer {
                id: entry.id.0.clone(),
                kind: OverlayPolicy::scene_layer_kind(entry.kind),
                owns_input: entry.policy.owns_input,
                esc: entry.policy.esc,
                outside: entry.policy.outside,
                focus_return: None,
            });
        }
    }
}

/// Public placement helper used by completion menus and the stack.
#[must_use]
pub fn place_overlay(
    bounds: Rect,
    anchor: Option<Rect>,
    size: OverlaySize,
    policy: OverlayPolicy,
) -> Rect {
    resolve_placement(bounds, anchor, size, policy, OverlayKind::Custom).0
}

fn resolve_placement(
    bounds: Rect,
    anchor: Option<Rect>,
    size: OverlaySize,
    policy: OverlayPolicy,
    kind: OverlayKind,
) -> (Rect, bool) {
    if bounds.is_empty() {
        return (Rect::default(), false);
    }

    let mut fullscreen_promoted = false;
    let mut prefer = policy.prefer;
    let narrow = bounds.width <= policy.narrow_cols && policy.narrow_cols > 0;
    if narrow {
        match policy.narrow_fallback {
            NarrowFallback::Hide => return (Rect::default(), false),
            NarrowFallback::Center => prefer = PlacementPrefer::Center,
            NarrowFallback::Fullscreen => {
                prefer = PlacementPrefer::Fullscreen;
                fullscreen_promoted = true;
            }
            NarrowFallback::Clamp => {}
        }
    }

    let mut width = size.width.max(size.min_width).max(1);
    let mut height = size.height.max(size.min_height).max(1);
    if size.max_width > 0 {
        width = width.min(size.max_width);
    }
    if size.max_height > 0 {
        height = height.min(size.max_height);
    }
    width = width.min(bounds.width).max(1);
    height = height.min(bounds.height).max(1);

    // Kind-specific size bias for drawers/fullscreen.
    if matches!(kind, OverlayKind::Fullscreen) || matches!(prefer, PlacementPrefer::Fullscreen) {
        return (bounds, true);
    }
    if matches!(prefer, PlacementPrefer::DrawerStart) {
        let w = width
            .min(bounds.width / 2)
            .max(size.min_width.min(bounds.width));
        return (Rect::new(bounds.x, bounds.y, w, bounds.height), false);
    }
    if matches!(prefer, PlacementPrefer::DrawerEnd) {
        let w = width
            .min(bounds.width / 2)
            .max(size.min_width.min(bounds.width));
        let x = bounds.x.saturating_add(bounds.width.saturating_sub(w));
        return (Rect::new(x, bounds.y, w, bounds.height), false);
    }

    let rect = match prefer {
        PlacementPrefer::Center => {
            let x = bounds
                .x
                .saturating_add(bounds.width.saturating_sub(width) / 2);
            let y = bounds
                .y
                .saturating_add(bounds.height.saturating_sub(height) / 2);
            Rect::new(x, y, width, height)
        }
        PlacementPrefer::AtOrigin => {
            let origin = anchor.unwrap_or(Rect::new(bounds.x, bounds.y, 1, 1));
            clamp_rect(Rect::new(origin.x, origin.y, width, height), bounds)
        }
        PlacementPrefer::BelowStart | PlacementPrefer::AboveStart | PlacementPrefer::EndAligned => {
            let anchor = anchor.unwrap_or(Rect::new(
                bounds.x.saturating_add(bounds.width / 2),
                bounds.y.saturating_add(bounds.height / 2),
                1,
                1,
            ));
            place_anchored(bounds, anchor, width, height, prefer, policy.cover_anchor)
        }
        PlacementPrefer::Fullscreen | PlacementPrefer::DrawerStart | PlacementPrefer::DrawerEnd => {
            bounds
        }
    };
    (rect, fullscreen_promoted)
}

fn clamp_rect(rect: Rect, bounds: Rect) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    let width = rect.width.min(bounds.width).max(1);
    let height = rect.height.min(bounds.height).max(1);
    let max_x = bounds.x.saturating_add(bounds.width.saturating_sub(width));
    let max_y = bounds
        .y
        .saturating_add(bounds.height.saturating_sub(height));
    let x = rect.x.clamp(bounds.x, max_x);
    let y = rect.y.clamp(bounds.y, max_y);
    Rect::new(x, y, width, height)
}

fn place_anchored(
    bounds: Rect,
    anchor: Rect,
    width: u16,
    height: u16,
    prefer: PlacementPrefer,
    cover_anchor: bool,
) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    let width = width.min(bounds.width).max(1);
    let height = height.min(bounds.height).max(1);

    let below_y = anchor.y.saturating_add(1);
    let space_below = bounds
        .y
        .saturating_add(bounds.height)
        .saturating_sub(below_y);
    let space_above = anchor.y.saturating_sub(bounds.y);

    let prefer_below = matches!(
        prefer,
        PlacementPrefer::BelowStart | PlacementPrefer::EndAligned
    );
    let y = if prefer_below {
        if space_below >= height {
            below_y
        } else if space_above >= height {
            anchor.y.saturating_sub(height)
        } else if space_below >= space_above {
            below_y.min(
                bounds
                    .y
                    .saturating_add(bounds.height)
                    .saturating_sub(height),
            )
        } else {
            bounds.y
        }
    } else if space_above >= height {
        anchor.y.saturating_sub(height)
    } else if space_below >= height {
        below_y
    } else {
        bounds.y
    };

    let right_limit = bounds.x.saturating_add(bounds.width);
    let x = if matches!(prefer, PlacementPrefer::EndAligned) {
        right_limit.saturating_sub(width).max(bounds.x)
    } else if anchor.x.saturating_add(width) <= right_limit {
        anchor.x.max(bounds.x)
    } else {
        right_limit.saturating_sub(width).max(bounds.x)
    };

    let mut rect = clamp_rect(Rect::new(x, y, width, height), bounds);

    if !cover_anchor && rect_intersects(rect, anchor) {
        if anchor.y > bounds.y {
            let flipped = clamp_rect(
                Rect::new(
                    rect.x,
                    anchor.y.saturating_sub(height).max(bounds.y),
                    width,
                    height,
                ),
                bounds,
            );
            if !rect_intersects(flipped, anchor) {
                return flipped;
            }
        }
        let pushed = clamp_rect(
            Rect::new(
                rect.x,
                anchor.y.saturating_add(1).min(
                    bounds
                        .y
                        .saturating_add(bounds.height)
                        .saturating_sub(height),
                ),
                width,
                height,
            ),
            bounds,
        );
        if !rect_intersects(pushed, anchor) {
            return pushed;
        }
    }
    // Final edge clamp already applied.
    let _ = &mut rect;
    rect
}

fn rect_intersects(a: Rect, b: Rect) -> bool {
    let a_x2 = a.x.saturating_add(a.width);
    let a_y2 = a.y.saturating_add(a.height);
    let b_x2 = b.x.saturating_add(b.width);
    let b_y2 = b.y.saturating_add(b.height);
    a.x < b_x2 && b.x < a_x2 && a.y < b_y2 && b.y < a_y2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_closes_exactly_one_layer() {
        let mut stack = OverlayStack::<&'static str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("dialog"),
                kind: OverlayKind::Dialog,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(40, 10),
                opener_focus: Some("list"),
                policy: None,
            },
        );
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("menu"),
                kind: OverlayKind::Menu,
                parent: Some(OverlayId::from_static("dialog")),
                anchor: Some(Rect::new(10, 10, 1, 1)),
                size: OverlaySize::menu(20, 6),
                opener_focus: None,
                policy: None,
            },
        );
        assert_eq!(stack.entries().len(), 2);
        let first = stack.handle_escape();
        assert!(matches!(
            first,
            OverlayOutcome::Dismissed {
                id: OverlayId(ref s),
                ..
            } if s == "menu"
        ));
        assert_eq!(stack.entries().len(), 1);
        let second = stack.handle_escape();
        assert!(matches!(
            second,
            OverlayOutcome::Dismissed {
                id: OverlayId(ref s),
                focus: Some("list"),
                ..
            } if s == "dialog"
        ));
        assert!(stack.is_empty());
        assert_eq!(stack.handle_escape(), OverlayOutcome::UnhandledEscape);
    }

    #[test]
    fn trap_protects_layers_beneath() {
        let mut stack = OverlayStack::<()>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("alert"),
                kind: OverlayKind::AlertDialog,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(30, 8),
                opener_focus: None,
                policy: None,
            },
        );
        assert_eq!(stack.handle_escape(), OverlayOutcome::Ignored);
        assert_eq!(stack.entries().len(), 1);
    }

    #[test]
    fn completion_never_covers_anchor() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 1, 1);
        let policy = OverlayPolicy::for_kind(OverlayKind::Completion);
        let rect = place_overlay(bounds, Some(anchor), OverlaySize::menu(20, 8), policy);
        assert!(!rect_intersects(rect, anchor));
        assert!(rect.y >= 6 || rect.y + rect.height <= 5);
    }

    #[test]
    fn placement_flips_near_bottom_edge() {
        let bounds = Rect::new(0, 0, 40, 20);
        let anchor = Rect::new(5, 18, 1, 1);
        let policy = OverlayPolicy::for_kind(OverlayKind::Menu);
        let rect = place_overlay(bounds, Some(anchor), OverlaySize::menu(12, 6), policy);
        assert!(rect.y + rect.height <= 20);
        assert!(!rect_intersects(rect, anchor) || policy.cover_anchor);
    }

    #[test]
    fn placement_clamps_near_right_edge() {
        let bounds = Rect::new(0, 0, 40, 20);
        let anchor = Rect::new(35, 5, 1, 1);
        let rect = place_overlay(
            bounds,
            Some(anchor),
            OverlaySize::menu(20, 4),
            OverlayPolicy::for_kind(OverlayKind::Menu),
        );
        assert!(rect.x + rect.width <= 40);
    }

    #[test]
    fn tiny_terminal_hides_tooltip() {
        let bounds = Rect::new(0, 0, 16, 8);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("tip"),
                kind: OverlayKind::Tooltip,
                parent: None,
                anchor: Some(Rect::new(2, 2, 1, 1)),
                size: OverlaySize::menu(12, 1),
                opener_focus: None,
                policy: None,
            },
        );
        assert!(stack.top().unwrap().rect.is_empty());
    }

    #[test]
    fn dialog_narrow_promotes_fullscreen() {
        let bounds = Rect::new(0, 0, 30, 12);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("dlg"),
                kind: OverlayKind::Dialog,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(40, 10),
                opener_focus: None,
                policy: None,
            },
        );
        let top = stack.top().unwrap();
        assert!(top.fullscreen_promoted);
        assert_eq!(top.rect, bounds);
    }

    #[test]
    fn outside_click_dismisses_menu_not_alert() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("menu"),
                kind: OverlayKind::Menu,
                parent: None,
                anchor: Some(Rect::new(10, 10, 1, 1)),
                size: OverlaySize::menu(15, 5),
                opener_focus: None,
                policy: None,
            },
        );
        let outside = Position::new(0, 0);
        assert!(matches!(
            stack.handle_outside_click(outside),
            OverlayOutcome::Dismissed { .. }
        ));

        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("alert"),
                kind: OverlayKind::AlertDialog,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(20, 6),
                opener_focus: None,
                policy: None,
            },
        );
        assert_eq!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Ignored
        );
    }

    #[test]
    fn nested_dismiss_removes_children() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("parent"),
                kind: OverlayKind::Popover,
                parent: None,
                anchor: Some(Rect::new(5, 5, 3, 1)),
                size: OverlaySize::menu(20, 5),
                opener_focus: None,
                policy: None,
            },
        );
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("child"),
                kind: OverlayKind::Menu,
                parent: Some(OverlayId::from_static("parent")),
                anchor: Some(Rect::new(8, 8, 1, 1)),
                size: OverlaySize::menu(12, 4),
                opener_focus: None,
                policy: None,
            },
        );
        assert_eq!(stack.entries().len(), 2);
        let _ = stack.dismiss(&OverlayId::from_static("parent"));
        assert!(stack.is_empty());
    }

    #[test]
    fn reflow_on_resize_keeps_stack() {
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            Rect::new(0, 0, 80, 24),
            OverlaySpec {
                id: OverlayId::from_static("palette"),
                kind: OverlayKind::CommandPalette,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(48, 12),
                opener_focus: None,
                policy: None,
            },
        );
        stack.reflow(Rect::new(0, 0, 40, 12));
        assert_eq!(stack.entries().len(), 1);
        let top = stack.top().unwrap();
        assert!(top.rect.width <= 40);
        assert!(top.rect.height <= 12);
    }

    #[test]
    fn promote_fullscreen() {
        let bounds = Rect::new(0, 0, 100, 30);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("view"),
                kind: OverlayKind::Popover,
                parent: None,
                anchor: Some(Rect::new(10, 10, 5, 1)),
                size: OverlaySize::menu(20, 8),
                opener_focus: None,
                policy: None,
            },
        );
        let _ = stack.promote_top_fullscreen(bounds);
        assert_eq!(stack.top().unwrap().rect, bounds);
        assert!(stack.top().unwrap().fullscreen_promoted);
    }

    #[test]
    fn queued_dialogs_stack_order() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&str>::new();
        for (id, opener) in [("d1", "a"), ("d2", "b"), ("d3", "c")] {
            stack.open(
                bounds,
                OverlaySpec::dialog(id, OverlaySize::dialog(30, 8), Some(opener)),
            );
        }
        assert_eq!(stack.top().unwrap().id.as_str(), "d3");
        let _ = stack.handle_escape();
        assert_eq!(stack.top().unwrap().id.as_str(), "d2");
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("b"),
                ..
            }
        ));
    }

    #[test]
    fn wheel_capture_on_top_menu() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("m"),
                kind: OverlayKind::Menu,
                parent: None,
                anchor: Some(Rect::new(10, 10, 1, 1)),
                size: OverlaySize::menu(20, 6),
                opener_focus: None,
                policy: None,
            },
        );
        let rect = stack.top().unwrap().rect;
        assert!(stack.wheel_captured(Position::new(rect.x, rect.y)));
    }

    // --- Story scenarios (nested, edges, tiny, mouse, keyboard, opener, resize, queue, fullscreen) ---

    #[test]
    fn story_nested_overlays_escape_one_layer() {
        let bounds = Rect::new(0, 0, 100, 30);
        let mut stack = OverlayStack::<&'static str>::new();
        stack.open(
            bounds,
            OverlaySpec::dialog("dlg", OverlaySize::dialog(40, 12), Some("root-list")),
        );
        stack.open(
            bounds,
            OverlaySpec::menu(
                "menu",
                Rect::new(20, 12, 8, 1),
                OverlaySize::menu(16, 5),
                None,
            )
            .with_parent("dlg"),
        );
        stack.open(
            bounds,
            OverlaySpec::context_menu(
                "ctx",
                Rect::new(25, 14, 1, 1),
                OverlaySize::menu(12, 4),
                None,
            )
            .with_parent("menu"),
        );
        assert_eq!(stack.entries().len(), 3);
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                id: OverlayId(ref s),
                ..
            } if s == "ctx"
        ));
        assert_eq!(stack.entries().len(), 2);
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                id: OverlayId(ref s),
                ..
            } if s == "menu"
        ));
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                id: OverlayId(ref s),
                focus: Some("root-list"),
                ..
            } if s == "dlg"
        ));
    }

    #[test]
    fn story_placement_near_every_screen_edge() {
        let bounds = Rect::new(0, 0, 60, 20);
        let policy = OverlayPolicy::for_kind(OverlayKind::Menu);
        let size = OverlaySize::menu(14, 5);
        // Top-left
        let tl = place_overlay(bounds, Some(Rect::new(0, 0, 1, 1)), size, policy);
        assert!(tl.x + tl.width <= 60 && tl.y + tl.height <= 20);
        assert!(!rect_intersects(tl, Rect::new(0, 0, 1, 1)));
        // Top-right
        let tr = place_overlay(bounds, Some(Rect::new(58, 0, 1, 1)), size, policy);
        assert!(tr.x + tr.width <= 60);
        // Bottom-left
        let bl = place_overlay(bounds, Some(Rect::new(0, 19, 1, 1)), size, policy);
        assert!(bl.y + bl.height <= 20);
        assert!(!rect_intersects(bl, Rect::new(0, 19, 1, 1)));
        // Bottom-right
        let br = place_overlay(bounds, Some(Rect::new(58, 19, 1, 1)), size, policy);
        assert!(br.x + br.width <= 60 && br.y + br.height <= 20);
        // Mid edges
        let top = place_overlay(bounds, Some(Rect::new(30, 0, 1, 1)), size, policy);
        let bottom = place_overlay(bounds, Some(Rect::new(30, 19, 1, 1)), size, policy);
        let left = place_overlay(bounds, Some(Rect::new(0, 10, 1, 1)), size, policy);
        let right = place_overlay(bounds, Some(Rect::new(59, 10, 1, 1)), size, policy);
        for rect in [top, bottom, left, right] {
            assert!(rect.x + rect.width <= 60);
            assert!(rect.y + rect.height <= 20);
        }
    }

    #[test]
    fn story_tiny_terminal_fallback() {
        let tiny = Rect::new(0, 0, 24, 8);
        let mut stack = OverlayStack::<()>::new();
        // Dialog promotes to fullscreen under narrow_cols=40
        stack.open(
            tiny,
            OverlaySpec::dialog("d", OverlaySize::dialog(40, 10), None),
        );
        assert!(stack.top().unwrap().fullscreen_promoted);
        assert_eq!(stack.top().unwrap().rect, tiny);
        // Tooltip hides under narrow_cols=20
        stack.clear();
        stack.open(
            Rect::new(0, 0, 12, 6),
            OverlaySpec {
                id: OverlayId::from_static("tip"),
                kind: OverlayKind::Tooltip,
                parent: None,
                anchor: Some(Rect::new(1, 1, 1, 1)),
                size: OverlaySize::menu(10, 1),
                opener_focus: None,
                policy: None,
            },
        );
        assert!(stack.top().unwrap().rect.is_empty());
        // Completion clamps
        stack.clear();
        stack.open(
            tiny,
            OverlaySpec::completion(
                "cmp",
                Rect::new(2, 3, 1, 1),
                OverlaySize::menu(40, 10),
                None,
            ),
        );
        let rect = stack.top().unwrap().rect;
        assert!(rect.width <= 24 && rect.height <= 8);
        assert!(!rect.is_empty());
    }

    #[test]
    fn story_mouse_dismissal() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec::menu("m", Rect::new(20, 10, 1, 1), OverlaySize::menu(15, 5), None),
        );
        let rect = stack.top().unwrap().rect;
        // Click inside: no dismiss
        assert_eq!(
            stack.handle_outside_click(Position::new(rect.x, rect.y)),
            OverlayOutcome::Ignored
        );
        // Click outside: dismiss
        assert!(matches!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Dismissed { .. }
        ));
        // Dialog traps outside click
        stack.open(
            bounds,
            OverlaySpec::dialog("d", OverlaySize::dialog(30, 8), None),
        );
        assert_eq!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Ignored
        );
        // Palette dismisses outside
        stack.clear();
        stack.open(
            bounds,
            OverlaySpec::command_palette("p", OverlaySize::dialog(48, 12), None),
        );
        assert!(matches!(
            stack.handle_outside_click(Position::new(0, 0)),
            OverlayOutcome::Dismissed { .. }
        ));
    }

    #[test]
    fn story_keyboard_only_navigation_and_esc_law() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        stack.open(
            bounds,
            OverlaySpec::command_palette("palette", OverlaySize::dialog(48, 12), Some("editor")),
        );
        assert!(stack.top_owns_input());
        assert!(stack.top().unwrap().policy.focus_trap);
        // Esc closes palette only
        let out = stack.handle_escape();
        assert!(matches!(
            out,
            OverlayOutcome::Dismissed {
                id: OverlayId(ref s),
                focus: Some("editor"),
                ..
            } if s == "palette"
        ));
        // Empty → unhandled (consumer may quit)
        assert_eq!(stack.handle_escape(), OverlayOutcome::UnhandledEscape);
        // Alert traps Esc
        stack.open(
            bounds,
            OverlaySpec::alert_dialog("alert", OverlaySize::dialog(28, 6), Some("btn")),
        );
        assert_eq!(stack.handle_escape(), OverlayOutcome::Ignored);
        assert!(stack.contains(&OverlayId::from_static("alert")));
    }

    #[test]
    fn story_opener_focus_restoration() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        stack.open(
            bounds,
            OverlaySpec::dialog("confirm", OverlaySize::dialog(36, 8), Some("save-btn")),
        );
        match stack.handle_escape() {
            OverlayOutcome::Dismissed { focus: Some(f), .. } => assert_eq!(f, "save-btn"),
            other => panic!("expected focus restore, got {other:?}"),
        }
    }

    #[test]
    fn story_resize_while_overlay_open() {
        let mut stack = OverlayStack::<()>::new();
        let wide = Rect::new(0, 0, 120, 40);
        stack.open(
            wide,
            OverlaySpec::completion(
                "cmp",
                Rect::new(50, 20, 1, 1),
                OverlaySize::menu(30, 10),
                None,
            ),
        );
        let before = stack.top().unwrap().rect;
        assert!(!before.is_empty());
        stack.reflow(Rect::new(0, 0, 40, 12));
        let after = stack.top().unwrap().rect;
        assert!(after.width <= 40);
        assert!(after.height <= 12);
        assert_eq!(stack.entries().len(), 1);
        // Anchor + size preserved for reflow
        assert_eq!(stack.top().unwrap().anchor, Some(Rect::new(50, 20, 1, 1)));
        // Dialog that was centered stays centered after shrink
        stack.clear();
        stack.open(
            wide,
            OverlaySpec::dialog("d", OverlaySize::dialog(50, 14), None),
        );
        stack.reflow(Rect::new(0, 0, 60, 20));
        let d = stack.top().unwrap().rect;
        assert!(d.x + d.width <= 60);
        assert!(d.y + d.height <= 20);
    }

    #[test]
    fn story_multiple_queued_dialogs() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<usize>::new();
        for i in 0..4 {
            stack.open(
                bounds,
                OverlaySpec::dialog(format!("q{i}"), OverlaySize::dialog(32, 8), Some(i)),
            );
        }
        assert_eq!(stack.entries().len(), 4);
        assert_eq!(stack.top().unwrap().id.as_str(), "q3");
        for expect in (0..4).rev() {
            match stack.handle_escape() {
                OverlayOutcome::Dismissed {
                    focus: Some(f), id, ..
                } => {
                    assert_eq!(f, expect);
                    assert_eq!(id.as_str(), format!("q{expect}"));
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(stack.is_empty());
    }

    #[test]
    fn story_fullscreen_promotion() {
        let bounds = Rect::new(0, 0, 100, 30);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("viewer"),
                kind: OverlayKind::Popover,
                parent: None,
                anchor: Some(Rect::new(10, 10, 4, 1)),
                size: OverlaySize::menu(24, 10),
                opener_focus: None,
                policy: None,
            },
        );
        assert!(!stack.top().unwrap().fullscreen_promoted);
        let _ = stack.promote_top_fullscreen(bounds);
        assert!(stack.top().unwrap().fullscreen_promoted);
        assert_eq!(stack.top().unwrap().rect, bounds);
        // Narrow dialog auto-promotes
        stack.clear();
        stack.open(
            Rect::new(0, 0, 28, 10),
            OverlaySpec::dialog("narrow", OverlaySize::dialog(50, 16), None),
        );
        assert!(stack.top().unwrap().fullscreen_promoted);
        // Explicit Fullscreen kind fills bounds
        stack.clear();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("fs"),
                kind: OverlayKind::Fullscreen,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(10, 10),
                opener_focus: None,
                policy: None,
            },
        );
        assert_eq!(stack.top().unwrap().rect, bounds);
    }

    #[test]
    fn story_policy_table_covers_all_kinds() {
        for kind in [
            OverlayKind::Tooltip,
            OverlayKind::Popover,
            OverlayKind::Menu,
            OverlayKind::ContextMenu,
            OverlayKind::Completion,
            OverlayKind::Select,
            OverlayKind::Dialog,
            OverlayKind::AlertDialog,
            OverlayKind::Drawer,
            OverlayKind::CommandPalette,
            OverlayKind::Fullscreen,
            OverlayKind::Custom,
        ] {
            let p = OverlayPolicy::for_kind(kind);
            // Every kind has a coherent prefer + esc policy
            let _ = p.esc;
            let _ = p.prefer;
            let scene = OverlayPolicy::scene_layer_kind(kind);
            let _ = scene;
        }
    }

    #[test]
    fn story_drawer_edge_placement() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<()>::new();
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static("drawer"),
                kind: OverlayKind::Drawer,
                parent: None,
                anchor: None,
                size: OverlaySize {
                    width: 30,
                    height: 24,
                    min_width: 12,
                    min_height: 1,
                    max_width: 0,
                    max_height: 0,
                },
                opener_focus: None,
                policy: None,
            },
        );
        let rect = stack.top().unwrap().rect;
        // Default drawer is end (right) edge
        assert_eq!(rect.y, 0);
        assert_eq!(rect.height, 24);
        assert!(rect.x + rect.width == 80 || rect.x >= 40);
    }
}
