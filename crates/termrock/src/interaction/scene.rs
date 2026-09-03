// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Per-frame interaction scene: one authority for focus, hits, layers, actions.
//!
//! Immediate-mode registration each frame. Stable focus and open layers persist
//! across frames. No callbacks, effects, or domain policy.
use ratatui_core::layout::{Position, Rect};

use crate::input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// Semantic role of a registered element for discovery and tooling.
///
/// Roles are terminal-native accessibility / inspection labels — not a retained DOM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SemanticRole {
    /// Ordinary content.
    #[default]
    Content,
    /// Focusable control (generic).
    Control,
    /// Overlay / modal chrome.
    Overlay,
    /// Status or chrome strip.
    Chrome,
    /// Button / primary action.
    Button,
    /// Text field or composer.
    Input,
    /// List container.
    List,
    /// List / menu row.
    ListItem,
    /// Tree container or node.
    Tree,
    /// Table / grid surface.
    Table,
    /// Tab strip or tab.
    Tab,
    /// Dialog / card body.
    Dialog,
    /// Menu / palette / completion.
    Menu,
    /// Status bar / strip.
    Status,
    /// Heading / section title.
    Heading,
    /// Image / media surface.
    Image,
    /// Progress / meter.
    Progress,
    /// Caller-defined role.
    Custom,
}

impl SemanticRole {
    /// Short stable token for snapshots and Studio lines.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Control => "control",
            Self::Overlay => "overlay",
            Self::Chrome => "chrome",
            Self::Button => "button",
            Self::Input => "input",
            Self::List => "list",
            Self::ListItem => "list_item",
            Self::Tree => "tree",
            Self::Table => "table",
            Self::Tab => "tab",
            Self::Dialog => "dialog",
            Self::Menu => "menu",
            Self::Status => "status",
            Self::Heading => "heading",
            Self::Image => "image",
            Self::Progress => "progress",
            Self::Custom => "custom",
        }
    }
}

/// Escape / outside-click policy for one interaction layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LayerDismissPolicy {
    /// Esc and outside-click may dismiss this layer when it is topmost.
    #[default]
    Dismissible,
    /// Topmost layer absorbs Esc/outside without dismissing (protects lower).
    Trap,
    /// Layer is transparent to Esc (should not sit as top input owner).
    Ignore,
}

/// Kind of layer for tooling and default policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum LayerKind {
    /// Root application surface.
    #[default]
    Root,
    /// Menu / palette / completion.
    Menu,
    /// Jump mode.
    Jump,
    /// Transient toast (usually non-modal).
    Toast,
    /// Blocking card/dialog.
    Card,
    /// Caller-defined.
    Custom,
}

/// One open layer that may own input and Esc policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionLayer<LayerId, Id = ()> {
    /// Stable layer identity.
    pub id: LayerId,
    /// Kind for tooling.
    pub kind: LayerKind,
    /// Whether this layer owns keyboard routing while topmost.
    pub owns_input: bool,
    /// Esc policy when this layer is topmost.
    pub esc: LayerDismissPolicy,
    /// Outside-click policy when this layer is topmost.
    pub outside: LayerDismissPolicy,
    /// Focus identity to restore when this layer is dismissed (optional).
    pub focus_return: Option<Id>,
}

/// One element registered for the current frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionElement<Id, LayerId, Action> {
    /// Stable identity across frames.
    pub id: Id,
    /// Painted rectangle.
    pub area: Rect,
    /// Layer that owns this element.
    pub layer: LayerId,
    /// Whether the element may receive focus.
    pub focusable: bool,
    /// Whether the element is enabled.
    pub enabled: bool,
    /// Whether the element is hidden (registered for geometry only).
    pub hidden: bool,
    /// Semantic classification.
    pub role: SemanticRole,
    /// Actions currently available on this element (borrowed projection).
    pub actions: Vec<Action>,
}

impl<Id, LayerId, Action> InteractionElement<Id, LayerId, Action> {
    /// Creates a visible enabled control with no actions.
    #[must_use]
    pub fn control(id: Id, layer: LayerId, area: Rect) -> Self
    where
        Action: Clone,
    {
        Self {
            id,
            area,
            layer,
            focusable: true,
            enabled: true,
            hidden: false,
            role: SemanticRole::Control,
            actions: Vec::new(),
        }
    }

    /// Attaches available actions for this frame.
    #[must_use]
    pub fn actions(mut self, actions: Vec<Action>) -> Self {
        self.actions = actions;
        self
    }

    /// Marks the element disabled.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Marks the element non-focusable.
    #[must_use]
    pub const fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Marks the element hidden.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Sets semantic role.
    #[must_use]
    pub const fn role(mut self, role: SemanticRole) -> Self {
        self.role = role;
        self
    }
}

/// Result of routing input through the scene.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InteractionOutcome<Id, LayerId, Action> {
    /// Input did not apply.
    Ignored,
    /// Focus moved.
    FocusChanged {
        /// Previous focus.
        from: Option<Id>,
        /// New focus.
        to: Option<Id>,
    },
    /// A typed action was dispatched for the focused or hit target.
    Action {
        /// Target element identity.
        target: Id,
        /// Action payload (caller-owned type).
        action: Action,
    },
    /// Top layer was dismissed by Esc or outside policy.
    LayerDismissed {
        /// Removed layer.
        layer: LayerId,
        /// Restored focus if known.
        focus: Option<Id>,
    },
    /// Esc reached the bottom of the stack; consumer owns quit policy.
    UnhandledEscape,
}

/// Registration error for invalid frame graphs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SceneError {
    /// Duplicate element id in one frame.
    DuplicateElement,
    /// Element references an unknown layer.
    UnknownLayer,
}

/// Per-frame interaction scene (immediate mode) with cross-frame focus/layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionScene<Id, LayerId, Action> {
    elements: Vec<InteractionElement<Id, LayerId, Action>>,
    /// Open layers bottom → top. Root is optional first entry.
    layers: Vec<InteractionLayer<LayerId, Id>>,
    focused: Option<Id>,
    root_layer: Option<LayerId>,
}

impl<Id, LayerId, Action> Default for InteractionScene<Id, LayerId, Action> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id, LayerId, Action> InteractionScene<Id, LayerId, Action> {
    /// Creates an empty scene.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            elements: Vec::new(),
            layers: Vec::new(),
            focused: None,
            root_layer: None,
        }
    }

    /// Clears per-frame registrations; keeps focus and open layers.
    pub fn begin_frame(&mut self) {
        self.elements.clear();
    }

    /// Ensures a root layer exists (idempotent by id).
    pub fn ensure_root(&mut self, layer: InteractionLayer<LayerId, Id>)
    where
        LayerId: Clone + PartialEq,
    {
        if self.root_layer.as_ref() == Some(&layer.id) {
            if self.layers.is_empty() {
                self.layers.push(layer);
            }
            return;
        }
        self.root_layer = Some(layer.id.clone());
        if self.layers.first().is_none_or(|item| item.id != layer.id) {
            self.layers.retain(|item| item.id != layer.id);
            self.layers.insert(0, layer);
        }
    }

    /// Pushes or replaces a layer, moving it to the top.
    pub fn push_layer(&mut self, layer: InteractionLayer<LayerId, Id>)
    where
        LayerId: PartialEq,
    {
        self.layers.retain(|item| item.id != layer.id);
        self.layers.push(layer);
    }

    /// Removes a layer by id.
    pub fn remove_layer(&mut self, id: &LayerId) -> bool
    where
        LayerId: PartialEq,
    {
        let before = self.layers.len();
        self.layers.retain(|item| &item.id != id);
        self.layers.len() != before
    }

    /// Open layers bottom → top.
    #[must_use]
    pub fn layers(&self) -> &[InteractionLayer<LayerId, Id>] {
        &self.layers
    }

    /// Topmost layer.
    #[must_use]
    pub fn top_layer(&self) -> Option<&InteractionLayer<LayerId, Id>> {
        self.layers.last()
    }

    /// Currently focused id.
    #[must_use]
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// Registered elements this frame.
    #[must_use]
    pub fn elements(&self) -> &[InteractionElement<Id, LayerId, Action>] {
        &self.elements
    }

    /// Registers one element. Returns error on duplicate id or unknown layer.
    pub fn register(
        &mut self,
        element: InteractionElement<Id, LayerId, Action>,
    ) -> Result<(), SceneError>
    where
        Id: PartialEq,
        LayerId: PartialEq,
    {
        if self.elements.iter().any(|item| item.id == element.id) {
            return Err(SceneError::DuplicateElement);
        }
        if !self.layers.iter().any(|layer| layer.id == element.layer) {
            return Err(SceneError::UnknownLayer);
        }
        self.elements.push(element);
        Ok(())
    }

    /// After registrations, reconcile focus to a valid enabled target.
    pub fn reconcile(&mut self)
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
    {
        if let Some(id) = self.focused.clone()
            && self.is_focusable(&id)
        {
            return;
        }
        self.focused = self
            .focus_candidates()
            .into_iter()
            .next()
            .map(|element| element.id.clone());
    }

    fn is_focusable(&self, id: &Id) -> bool
    where
        Id: PartialEq,
        LayerId: PartialEq,
    {
        self.elements.iter().any(|element| {
            &element.id == id
                && element.focusable
                && element.enabled
                && !element.hidden
                && self.layer_is_registered(&element.layer)
                && self.layer_accepts_input(&element.layer)
        })
    }

    fn layer_is_registered(&self, layer_id: &LayerId) -> bool
    where
        LayerId: PartialEq,
    {
        self.layers.iter().any(|layer| &layer.id == layer_id)
    }

    fn layer_accepts_input(&self, layer_id: &LayerId) -> bool
    where
        LayerId: PartialEq,
    {
        // Only elements on the top input-owning layer (or root if none) may
        // receive input while a modal stack is open.
        let Some(top) = self.layers.iter().rev().find(|layer| layer.owns_input) else {
            return true;
        };
        &top.id == layer_id
    }

    fn focus_candidates(&self) -> Vec<&InteractionElement<Id, LayerId, Action>>
    where
        LayerId: PartialEq,
    {
        self.elements
            .iter()
            .filter(|element| {
                element.focusable
                    && element.enabled
                    && !element.hidden
                    && self.layer_is_registered(&element.layer)
                    && self.layer_accepts_input(&element.layer)
            })
            .collect()
    }

    /// Focusable enabled ids in registration order (active input layer only).
    #[must_use]
    pub fn focus_order(&self) -> Vec<&Id>
    where
        LayerId: PartialEq,
    {
        self.focus_candidates()
            .into_iter()
            .map(|element| &element.id)
            .collect()
    }

    /// Topmost enabled, non-hidden element containing `position`.
    #[must_use]
    pub fn hit_test(&self, position: Position) -> Option<&InteractionElement<Id, LayerId, Action>>
    where
        LayerId: PartialEq,
    {
        self.elements.iter().rev().find(|element| {
            element.enabled
                && !element.hidden
                && element.area.contains(position)
                && self.layer_is_registered(&element.layer)
                && self.layer_accepts_input(&element.layer)
        })
    }

    /// Looks up an element by id.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&InteractionElement<Id, LayerId, Action>>
    where
        Id: PartialEq,
    {
        self.elements.iter().find(|element| &element.id == id)
    }

    /// Actions available in the active input layer (union of element actions).
    #[must_use]
    pub fn available_actions(&self) -> Vec<Action>
    where
        Id: PartialEq,
        LayerId: PartialEq,
        Action: Clone + PartialEq,
    {
        let mut out = Vec::new();
        for element in self.focus_candidates() {
            for action in &element.actions {
                if !out.iter().any(|item| item == action) {
                    out.push(action.clone());
                }
            }
        }
        out
    }

    /// Whether `action` is available on any active-layer element.
    #[must_use]
    pub fn action_available(&self, action: &Action) -> bool
    where
        LayerId: PartialEq,
        Action: PartialEq,
    {
        self.focus_candidates()
            .into_iter()
            .any(|element| element.actions.iter().any(|item| item == action))
    }

    /// Moves focus forward/backward among candidates. Wraps.
    pub fn focus_move(&mut self, reverse: bool) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
    {
        let order: Vec<Id> = self
            .focus_candidates()
            .into_iter()
            .map(|element| element.id.clone())
            .collect();
        if order.is_empty() {
            let from = self.focused.take();
            return if from.is_some() {
                InteractionOutcome::FocusChanged { from, to: None }
            } else {
                InteractionOutcome::Ignored
            };
        }
        let from = self.focused.clone();
        let index = from
            .as_ref()
            .and_then(|id| order.iter().position(|item| item == id))
            .unwrap_or(if reverse { 0 } else { order.len() - 1 });
        let next = if reverse {
            if index == 0 {
                order.len() - 1
            } else {
                index - 1
            }
        } else {
            (index + 1) % order.len()
        };
        let to = Some(order[next].clone());
        if from == to {
            return InteractionOutcome::Ignored;
        }
        self.focused = to.clone();
        InteractionOutcome::FocusChanged { from, to }
    }

    /// Sets focus if the id is currently focusable.
    pub fn focus(&mut self, id: Id) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
    {
        if !self.is_focusable(&id) {
            return InteractionOutcome::Ignored;
        }
        let from = self.focused.clone();
        if from.as_ref() == Some(&id) {
            return InteractionOutcome::Ignored;
        }
        self.focused = Some(id.clone());
        InteractionOutcome::FocusChanged { from, to: Some(id) }
    }

    /// Routes a key event: Tab focus, Esc layer peel, or action via callback map.
    pub fn handle_key_tab_esc(&mut self, key: KeyEvent) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: Clone + PartialEq,
    {
        if key.is_release() {
            return InteractionOutcome::Ignored;
        }
        match key.code {
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.focus_move(true),
            KeyCode::Tab => self.focus_move(false),
            KeyCode::BackTab => self.focus_move(true),
            KeyCode::Esc if key.is_press() => self.handle_escape(),
            _ => InteractionOutcome::Ignored,
        }
    }

    /// Peels Esc according to top-layer policy only.
    pub fn handle_escape(&mut self) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: Clone + PartialEq,
    {
        let Some(top) = self.layers.last().cloned() else {
            return InteractionOutcome::UnhandledEscape;
        };
        match top.esc {
            LayerDismissPolicy::Trap => {
                // Non-dismissible top protects every lower layer.
                InteractionOutcome::Ignored
            }
            LayerDismissPolicy::Ignore => {
                // Transparent / root: consumer owns quit policy.
                InteractionOutcome::UnhandledEscape
            }
            LayerDismissPolicy::Dismissible => {
                // Only the top layer may dismiss — never rposition under a trap.
                let layer = self.layers.pop().expect("top exists");
                let focus = self.restore_focus(layer.focus_return);
                InteractionOutcome::LayerDismissed {
                    layer: layer.id,
                    focus,
                }
            }
        }
    }

    /// Routes pointer input: hit-test topmost, outside-click dismiss top only.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: Clone + PartialEq,
        Action: Clone,
    {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(hit) = self.hit_test(event.position) {
                    let id = hit.id.clone();
                    if hit.focusable && hit.enabled && !hit.hidden {
                        let from = self.focused.clone();
                        self.focused = Some(id.clone());
                        if from.as_ref() != Some(&id) {
                            return InteractionOutcome::FocusChanged { from, to: Some(id) };
                        }
                    }
                    return InteractionOutcome::Ignored;
                }
                // Outside click: only top layer policy.
                let Some(top) = self.layers.last().cloned() else {
                    return InteractionOutcome::Ignored;
                };
                match top.outside {
                    LayerDismissPolicy::Dismissible => self.dismiss_top(),
                    LayerDismissPolicy::Trap | LayerDismissPolicy::Ignore => {
                        InteractionOutcome::Ignored
                    }
                }
            }
            _ => InteractionOutcome::Ignored,
        }
    }

    fn dismiss_top(&mut self) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: Clone + PartialEq,
    {
        let Some(layer) = self.layers.pop() else {
            return InteractionOutcome::Ignored;
        };
        let focus = self.restore_focus(layer.focus_return);
        InteractionOutcome::LayerDismissed {
            layer: layer.id,
            focus,
        }
    }

    fn restore_focus(&mut self, focus_return: Option<Id>) -> Option<Id>
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
    {
        if let Some(id) = focus_return
            && self.is_focusable(&id)
        {
            self.focused = Some(id);
        } else {
            self.reconcile();
        }
        self.focused.clone()
    }

    /// Dispatches `action` if available on the focused element (or any active).
    pub fn dispatch_action(&self, action: Action) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
        Action: Clone + PartialEq,
    {
        if let Some(focus_id) = &self.focused
            && self.is_focusable(focus_id)
            && let Some(element) = self.get(focus_id)
            && element.actions.iter().any(|item| item == &action)
        {
            return InteractionOutcome::Action {
                target: focus_id.clone(),
                action,
            };
        }
        // Fallback: first active element advertising the action.
        for element in self.focus_candidates() {
            if element.actions.iter().any(|item| item == &action) {
                return InteractionOutcome::Action {
                    target: element.id.clone(),
                    action,
                };
            }
        }
        InteractionOutcome::Ignored
    }
}

// ── SemanticScene: frame-local semantic tree (not InteractionScene) ────────

/// Registration / graph error for [`SemanticScene`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticError {
    /// Duplicate node id in one frame.
    DuplicateId,
    /// Parent id was not registered this frame.
    UnknownParent,
    /// Node parent equals its own id.
    SelfParent,
}

/// Collision / integrity diagnostic recorded during registration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticDiagnostic {
    /// Two nodes share an id (second rejected).
    DuplicateId {
        /// Display form of the colliding id.
        id: String,
    },
    /// Parent reference missing at register time.
    UnknownParent {
        /// Child id display.
        id: String,
        /// Missing parent display.
        parent: String,
    },
    /// Zero-area node (registered but not hittable).
    EmptyArea {
        /// Node id display.
        id: String,
    },
}

/// Interactive / visual state flags for one semantic node (frame projection).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SemanticState {
    /// Cursor / selection membership.
    pub selected: bool,
    /// Expanded disclosure (tree/list groups).
    pub expanded: bool,
    /// Multi-select checked.
    pub checked: bool,
    /// Busy / loading.
    pub busy: bool,
    /// Validation failed.
    pub invalid: bool,
    /// Pressed / active pointer state.
    pub pressed: bool,
}

/// One node in the frame-local semantic tree.
///
/// Prefer [`SemanticNode::control`] / builders. Labels are owned `String` so
/// virtualized rows can project without long-lived borrows into the scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticNode<Id, Action = ()> {
    /// Stable identity across frames (host-owned).
    pub id: Id,
    /// Parent identity when nested; `None` = root-level.
    pub parent: Option<Id>,
    /// Semantic role for tooling and snapshots.
    pub role: SemanticRole,
    /// Human-readable name (help, jump, AI).
    pub label: Option<String>,
    /// Longer description / help text.
    pub description: Option<String>,
    /// Painted rectangle this frame.
    pub area: Rect,
    /// Whether the node may receive focus (aid; focus authority is InteractionScene).
    pub focusable: bool,
    /// Whether the node is disabled.
    pub disabled: bool,
    /// Whether the node is hidden (geometry may still exist).
    pub hidden: bool,
    /// Part / widget state flags.
    pub state: SemanticState,
    /// Actions advertised this frame (caller vocabulary).
    pub actions: Vec<Action>,
}

impl<Id, Action> SemanticNode<Id, Action> {
    /// Visible enabled control with no parent or actions.
    #[must_use]
    pub fn control(id: Id, area: Rect) -> Self
    where
        Action: Clone,
    {
        Self {
            id,
            parent: None,
            role: SemanticRole::Control,
            label: None,
            description: None,
            area,
            focusable: true,
            disabled: false,
            hidden: false,
            state: SemanticState::default(),
            actions: Vec::new(),
        }
    }

    /// Content / non-control node.
    #[must_use]
    pub fn content(id: Id, area: Rect) -> Self
    where
        Action: Clone,
    {
        Self {
            id,
            parent: None,
            role: SemanticRole::Content,
            label: None,
            description: None,
            area,
            focusable: false,
            disabled: false,
            hidden: false,
            state: SemanticState::default(),
            actions: Vec::new(),
        }
    }

    /// Sets parent id.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Sets role.
    #[must_use]
    pub const fn role(mut self, role: SemanticRole) -> Self {
        self.role = role;
        self
    }

    /// Sets label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets description.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Marks focusable.
    #[must_use]
    pub const fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Marks disabled.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks hidden.
    #[must_use]
    pub const fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Replaces state flags.
    #[must_use]
    pub const fn state(mut self, state: SemanticState) -> Self {
        self.state = state;
        self
    }

    /// Attaches actions.
    #[must_use]
    pub fn actions(mut self, actions: Vec<Action>) -> Self {
        self.actions = actions;
        self
    }
}

/// One node in a portable semantic snapshot (string identities).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticSnapshotNode {
    /// Stable id display.
    pub id: String,
    /// Parent id display when nested.
    pub parent: Option<String>,
    /// Role token.
    pub role: &'static str,
    /// Label when present.
    pub label: Option<String>,
    /// Description when present.
    pub description: Option<String>,
    /// Geometry x.
    pub x: u16,
    /// Geometry y.
    pub y: u16,
    /// Geometry width.
    pub width: u16,
    /// Geometry height.
    pub height: u16,
    /// Focusable this frame.
    pub focusable: bool,
    /// Disabled this frame.
    pub disabled: bool,
    /// Hidden this frame.
    pub hidden: bool,
    /// Selected flag.
    pub selected: bool,
    /// Expanded flag.
    pub expanded: bool,
    /// Checked flag.
    pub checked: bool,
    /// Busy flag.
    pub busy: bool,
    /// Invalid flag.
    pub invalid: bool,
    /// Pressed / active pointer flag.
    pub pressed: bool,
    /// Action names (Display form).
    pub actions: Vec<String>,
}

/// Portable semantic tree for Studio, remote clients, and AI-readable UI state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SemanticSnapshot {
    /// Nodes in registration order.
    pub nodes: Vec<SemanticSnapshotNode>,
    /// Collision / integrity messages (human-readable).
    pub diagnostics: Vec<String>,
}

impl SemanticSnapshot {
    /// Compact Studio / log lines (`id@role label`).
    #[must_use]
    pub fn summary_lines(&self, max: usize) -> Vec<String> {
        let mut lines: Vec<String> = self
            .nodes
            .iter()
            .take(max.saturating_sub(self.diagnostics.len().min(max)))
            .map(|n| {
                let label = n.label.as_deref().unwrap_or("—");
                let flags = [
                    n.focusable.then_some("f"),
                    n.disabled.then_some("d"),
                    n.selected.then_some("s"),
                    n.busy.then_some("b"),
                    n.pressed.then_some("p"),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join("");
                format!("{}@{} [{}] {}", n.id, n.role, flags, label)
            })
            .collect();
        for d in self
            .diagnostics
            .iter()
            .take(max.saturating_sub(lines.len()))
        {
            lines.push(format!("! {d}"));
        }
        lines
    }

    /// Focusable node ids in snapshot order.
    #[must_use]
    pub fn focusable_ids(&self) -> Vec<&str> {
        self.nodes
            .iter()
            .filter(|n| n.focusable && !n.disabled && !n.hidden)
            .map(|n| n.id.as_str())
            .collect()
    }

    /// Serializes to a stable newline text format (one node per line).
    ///
    /// Format is line-oriented TSV-ish fields for remote clients and AI tools.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for n in &self.nodes {
            let parent = n.parent.as_deref().unwrap_or("-");
            let label = n.label.as_deref().unwrap_or("").replace(['\t', '\n'], " ");
            let desc = n
                .description
                .as_deref()
                .unwrap_or("")
                .replace(['\t', '\n'], " ");
            let actions = n.actions.join(",").replace('\t', " ");
            out.push_str(&format!(
                "node\tid={}\tparent={}\trole={}\tx={}\ty={}\tw={}\th={}\tlabel={}\tfocusable={}\tdisabled={}\thidden={}\tselected={}\texpanded={}\tchecked={}\tbusy={}\tinvalid={}\tpressed={}\tactions={}\tdesc={}\n",
                n.id,
                parent,
                n.role,
                n.x,
                n.y,
                n.width,
                n.height,
                label,
                u8::from(n.focusable),
                u8::from(n.disabled),
                u8::from(n.hidden),
                u8::from(n.selected),
                u8::from(n.expanded),
                u8::from(n.checked),
                u8::from(n.busy),
                u8::from(n.invalid),
                u8::from(n.pressed),
                actions,
                desc,
            ));
        }
        for d in &self.diagnostics {
            out.push_str(&format!("diag\t{}\n", d.replace(['\t', '\n'], " ")));
        }
        out
    }

    /// Parses [`Self::to_text`] output (best-effort; unknown fields ignored).
    #[must_use]
    pub fn from_text(text: &str) -> Self {
        let mut nodes = Vec::new();
        let mut diagnostics = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("diag\t") {
                diagnostics.push(rest.to_string());
                continue;
            }
            if !line.starts_with("node\t") {
                continue;
            }
            let mut map = std::collections::BTreeMap::<&str, &str>::new();
            for field in line.split('\t').skip(1) {
                if let Some((k, v)) = field.split_once('=') {
                    map.insert(k, v);
                }
            }
            let flag = |k: &str| map.get(k).copied().unwrap_or("0") == "1";
            let parse_u16 = |k: &str| map.get(k).and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
            let parent = map
                .get("parent")
                .copied()
                .filter(|p| *p != "-" && !p.is_empty());
            let label = map
                .get("label")
                .copied()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let description = map
                .get("desc")
                .copied()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let actions = map
                .get("actions")
                .copied()
                .filter(|s| !s.is_empty())
                .map(|s| s.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            nodes.push(SemanticSnapshotNode {
                id: map.get("id").unwrap_or(&"").to_string(),
                parent: parent.map(str::to_string),
                role: match map.get("role").copied().unwrap_or("content") {
                    // snapshot stores static role tokens; keep as leaked-free via as_str match
                    "control" => "control",
                    "overlay" => "overlay",
                    "chrome" => "chrome",
                    "button" => "button",
                    "input" => "input",
                    "list" => "list",
                    "list_item" => "list_item",
                    "tree" => "tree",
                    "table" => "table",
                    "tab" => "tab",
                    "dialog" => "dialog",
                    "menu" => "menu",
                    "status" => "status",
                    "heading" => "heading",
                    "image" => "image",
                    "progress" => "progress",
                    "custom" => "custom",
                    _ => "content",
                },
                label,
                description,
                x: parse_u16("x"),
                y: parse_u16("y"),
                width: parse_u16("w"),
                height: parse_u16("h"),
                focusable: flag("focusable"),
                disabled: flag("disabled"),
                hidden: flag("hidden"),
                selected: flag("selected"),
                expanded: flag("expanded"),
                checked: flag("checked"),
                busy: flag("busy"),
                invalid: flag("invalid"),
                pressed: flag("pressed"),
                actions,
            });
        }
        Self { nodes, diagnostics }
    }
}

/// Frame-local semantic tree rebuilt alongside rendering.
///
/// Does **not** own focus or Esc policy — that is [`InteractionScene`].
/// Register only painted / virtualized-visible nodes for large collections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticScene<Id, Action = ()> {
    nodes: Vec<SemanticNode<Id, Action>>,
    diagnostics: Vec<SemanticDiagnostic>,
}

impl<Id, Action> Default for SemanticScene<Id, Action> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id, Action> SemanticScene<Id, Action> {
    /// Creates an empty scene.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nodes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Clears nodes and diagnostics for a new frame (retains capacity).
    pub fn begin_frame(&mut self) {
        self.nodes.clear();
        self.diagnostics.clear();
    }

    /// Reserves capacity for bulk registration (virtualized windows).
    pub fn reserve(&mut self, additional: usize) {
        self.nodes.reserve(additional);
    }

    /// Registered nodes in order.
    #[must_use]
    pub fn nodes(&self) -> &[SemanticNode<Id, Action>] {
        &self.nodes
    }

    /// Diagnostics collected this frame.
    #[must_use]
    pub fn diagnostics(&self) -> &[SemanticDiagnostic] {
        &self.diagnostics
    }

    /// Registers one node. Parent must already be registered when set.
    pub fn register(&mut self, node: SemanticNode<Id, Action>) -> Result<(), SemanticError>
    where
        Id: Clone + PartialEq + std::fmt::Display,
    {
        if node.parent.as_ref() == Some(&node.id) {
            return Err(SemanticError::SelfParent);
        }
        if self.nodes.iter().any(|item| item.id == node.id) {
            self.diagnostics.push(SemanticDiagnostic::DuplicateId {
                id: node.id.to_string(),
            });
            return Err(SemanticError::DuplicateId);
        }
        if let Some(parent) = &node.parent
            && !self.nodes.iter().any(|item| &item.id == parent)
        {
            self.diagnostics.push(SemanticDiagnostic::UnknownParent {
                id: node.id.to_string(),
                parent: parent.to_string(),
            });
            return Err(SemanticError::UnknownParent);
        }
        if node.area.width == 0 || node.area.height == 0 {
            self.diagnostics.push(SemanticDiagnostic::EmptyArea {
                id: node.id.to_string(),
            });
        }
        self.nodes.push(node);
        Ok(())
    }

    /// Registers a child under `parent` (sets parent field).
    pub fn register_child(
        &mut self,
        parent: Id,
        mut node: SemanticNode<Id, Action>,
    ) -> Result<(), SemanticError>
    where
        Id: Clone + PartialEq + std::fmt::Display,
    {
        node.parent = Some(parent);
        self.register(node)
    }

    /// Looks up a node by id.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&SemanticNode<Id, Action>>
    where
        Id: PartialEq,
    {
        self.nodes.iter().find(|node| &node.id == id)
    }

    /// Direct children of `id` in registration order.
    #[must_use]
    pub fn children_of(&self, id: &Id) -> Vec<&SemanticNode<Id, Action>>
    where
        Id: PartialEq,
    {
        self.nodes
            .iter()
            .filter(|node| node.parent.as_ref() == Some(id))
            .collect()
    }

    /// Ancestor chain root → node (including `id` when present).
    #[must_use]
    pub fn path_to(&self, id: &Id) -> Vec<&Id>
    where
        Id: PartialEq,
    {
        let mut chain = Vec::new();
        let mut current = self.get(id);
        while let Some(node) = current {
            chain.push(&node.id);
            current = node.parent.as_ref().and_then(|p| self.get(p));
        }
        chain.reverse();
        chain
    }

    /// Topmost visible node containing `position` (later registration wins).
    ///
    /// Includes disabled nodes so Studio/inspection can still name geometry.
    /// Prefer [`Self::hit_test_interactive`] for activation routing.
    #[must_use]
    pub fn hit_test(&self, position: Position) -> Option<&SemanticNode<Id, Action>> {
        self.nodes.iter().rev().find(|node| {
            !node.hidden
                && node.area.width > 0
                && node.area.height > 0
                && node.area.contains(position)
        })
    }

    /// Topmost interactive target: visible, enabled, non-empty area.
    #[must_use]
    pub fn hit_test_interactive(&self, position: Position) -> Option<&SemanticNode<Id, Action>> {
        self.nodes.iter().rev().find(|node| {
            !node.hidden
                && !node.disabled
                && node.area.width > 0
                && node.area.height > 0
                && node.area.contains(position)
        })
    }

    /// Focusable, enabled, visible ids in registration order (navigation aid).
    #[must_use]
    pub fn focus_order(&self) -> Vec<&Id> {
        self.nodes
            .iter()
            .filter(|node| node.focusable && !node.disabled && !node.hidden)
            .map(|node| &node.id)
            .collect()
    }

    /// Hit regions for jump mode (focusable, visible, non-empty).
    #[must_use]
    pub fn jump_regions(&self) -> Vec<super::HitRegion<Id>>
    where
        Id: Clone,
    {
        self.nodes
            .iter()
            .filter(|node| {
                node.focusable
                    && !node.disabled
                    && !node.hidden
                    && node.area.width > 0
                    && node.area.height > 0
            })
            .map(|node| super::HitRegion {
                id: node.id.clone(),
                area: node.area,
            })
            .collect()
    }

    /// Help lines: `label — description` or id fallback for focusable nodes.
    #[must_use]
    pub fn help_lines(&self) -> Vec<String>
    where
        Id: std::fmt::Display,
    {
        self.nodes
            .iter()
            .filter(|node| node.focusable && !node.hidden)
            .map(|node| {
                let name = node.label.clone().unwrap_or_else(|| node.id.to_string());
                match &node.description {
                    Some(desc) if !desc.is_empty() => format!("{name} — {desc}"),
                    _ => name,
                }
            })
            .collect()
    }

    /// Builds a portable snapshot; action names via `action_name`.
    #[must_use]
    pub fn snapshot_with<F>(&self, mut action_name: F) -> SemanticSnapshot
    where
        Id: std::fmt::Display,
        F: FnMut(&Action) -> String,
    {
        let nodes = self
            .nodes
            .iter()
            .map(|n| SemanticSnapshotNode {
                id: n.id.to_string(),
                parent: n.parent.as_ref().map(ToString::to_string),
                role: n.role.as_str(),
                label: n.label.clone(),
                description: n.description.clone(),
                x: n.area.x,
                y: n.area.y,
                width: n.area.width,
                height: n.area.height,
                focusable: n.focusable,
                disabled: n.disabled,
                hidden: n.hidden,
                selected: n.state.selected,
                expanded: n.state.expanded,
                checked: n.state.checked,
                busy: n.state.busy,
                invalid: n.state.invalid,
                pressed: n.state.pressed,
                actions: n.actions.iter().map(&mut action_name).collect(),
            })
            .collect();
        let diagnostics = self
            .diagnostics
            .iter()
            .map(|d| match d {
                SemanticDiagnostic::DuplicateId { id } => format!("duplicate id: {id}"),
                SemanticDiagnostic::UnknownParent { id, parent } => {
                    format!("unknown parent {parent} for {id}")
                }
                SemanticDiagnostic::EmptyArea { id } => format!("empty area: {id}"),
            })
            .collect();
        SemanticSnapshot { nodes, diagnostics }
    }

    /// Snapshot with empty action names (structure-only).
    #[must_use]
    pub fn snapshot(&self) -> SemanticSnapshot
    where
        Id: std::fmt::Display,
    {
        self.snapshot_with(|_| String::new())
    }

    /// Topmost focusable interactive node at `position`.
    #[must_use]
    pub fn hit_test_focusable(&self, position: Position) -> Option<&SemanticNode<Id, Action>> {
        self.nodes.iter().rev().find(|node| {
            node.focusable
                && !node.hidden
                && !node.disabled
                && node.area.width > 0
                && node.area.height > 0
                && node.area.contains(position)
        })
    }

    /// Nodes with a given role (registration order).
    #[must_use]
    pub fn by_role(&self, role: SemanticRole) -> Vec<&SemanticNode<Id, Action>> {
        self.nodes.iter().filter(|n| n.role == role).collect()
    }

    /// Next/previous focusable id in registration order (wraps optional).
    #[must_use]
    pub fn focus_neighbor(&self, current: Option<&Id>, forward: bool, wrap: bool) -> Option<&Id>
    where
        Id: PartialEq,
    {
        let order = self.focus_order();
        if order.is_empty() {
            return None;
        }
        let Some(cur) = current else {
            return if forward {
                order.first().copied()
            } else {
                order.last().copied()
            };
        };
        let pos = order.iter().position(|id| *id == cur)?;
        if forward {
            if pos + 1 < order.len() {
                Some(order[pos + 1])
            } else if wrap {
                order.first().copied()
            } else {
                None
            }
        } else if pos > 0 {
            Some(order[pos - 1])
        } else if wrap {
            order.last().copied()
        } else {
            None
        }
    }

    /// Bulk-register visible virtualized rows (skips errors; records diagnostics).
    ///
    /// Prefer this for large windows: call [`Self::reserve`] first with the
    /// visible count (not logical length).
    pub fn register_many<I>(&mut self, nodes: I)
    where
        I: IntoIterator<Item = SemanticNode<Id, Action>>,
        Id: Clone + PartialEq + std::fmt::Display,
    {
        for node in nodes {
            let _ = self.register(node);
        }
    }

    /// Project focusable nodes into [`super::FocusNode`] leaves for FocusGraph.
    #[must_use]
    pub fn to_focus_nodes(&self) -> Vec<super::FocusNode<Id>>
    where
        Id: Clone,
    {
        self.nodes
            .iter()
            .filter(|n| n.focusable && !n.hidden)
            .map(|n| {
                let mut leaf = super::FocusNode::leaf(n.id.clone(), n.area);
                leaf.enabled = !n.disabled;
                leaf
            })
            .collect()
    }

    /// Flatten advertised actions with owner id (help / command palette seed).
    #[must_use]
    pub fn action_catalog(&self) -> Vec<(&Id, &Action)> {
        let mut out = Vec::new();
        for n in &self.nodes {
            if n.hidden || n.disabled {
                continue;
            }
            for a in &n.actions {
                out.push((&n.id, a));
            }
        }
        out
    }

    /// Count of registered nodes (Studio / diagnostics).
    #[must_use]
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the scene has no nodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Projects an [`InteractionScene`] into a flat semantic scene (no parents).
    #[must_use]
    pub fn from_interaction<LayerId>(scene: &InteractionScene<Id, LayerId, Action>) -> Self
    where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        let mut out = Self::new();
        out.reserve(scene.elements().len());
        for el in scene.elements() {
            let node = SemanticNode {
                id: el.id.clone(),
                parent: None,
                role: el.role,
                label: None,
                description: None,
                area: el.area,
                focusable: el.focusable,
                disabled: !el.enabled,
                hidden: el.hidden,
                state: SemanticState::default(),
                actions: el.actions.clone(),
            };
            let _ = out.register(node);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::input::KeyModifiers;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Layer {
        Root,
        Menu,
        Dialog,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum Act {
        Open,
        Confirm,
        Cancel,
    }

    fn root_layer() -> InteractionLayer<Layer, &'static str> {
        InteractionLayer {
            id: Layer::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        }
    }

    fn menu_layer() -> InteractionLayer<Layer, &'static str> {
        InteractionLayer {
            id: Layer::Menu,
            kind: LayerKind::Menu,
            owns_input: true,
            esc: LayerDismissPolicy::Dismissible,
            outside: LayerDismissPolicy::Dismissible,
            focus_return: Some("main"),
        }
    }

    fn dialog_trap() -> InteractionLayer<Layer, &'static str> {
        InteractionLayer {
            id: Layer::Dialog,
            kind: LayerKind::Card,
            owns_input: true,
            esc: LayerDismissPolicy::Trap,
            outside: LayerDismissPolicy::Trap,
            focus_return: Some("main"),
        }
    }

    #[test]
    fn duplicate_element_ids_reject() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        assert!(
            scene
                .register(InteractionElement::control(
                    "a",
                    Layer::Root,
                    Rect::new(0, 0, 1, 1)
                ))
                .is_ok()
        );
        assert_eq!(
            scene.register(InteractionElement::control(
                "a",
                Layer::Root,
                Rect::new(1, 0, 1, 1)
            )),
            Err(SceneError::DuplicateElement)
        );
    }

    #[test]
    fn disabled_and_hidden_never_focus_or_hit_action() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(
                InteractionElement::control("ok", Layer::Root, Rect::new(0, 0, 2, 1))
                    .actions(vec![Act::Confirm]),
            )
            .unwrap();
        scene
            .register(
                InteractionElement::control("no", Layer::Root, Rect::new(2, 0, 2, 1))
                    .enabled(false)
                    .actions(vec![Act::Open]),
            )
            .unwrap();
        scene
            .register(
                InteractionElement::control("hid", Layer::Root, Rect::new(4, 0, 2, 1))
                    .hidden(true)
                    .actions(vec![Act::Cancel]),
            )
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focus_order(), vec![&"ok"]);
        assert!(scene.hit_test(Position::new(2, 0)).is_none()); // disabled
        assert!(scene.hit_test(Position::new(4, 0)).is_none()); // hidden
        assert!(!scene.action_available(&Act::Open));
        assert!(scene.action_available(&Act::Confirm));
    }

    #[test]
    fn later_registration_wins_hit_test() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "a",
                Layer::Root,
                Rect::new(0, 0, 10, 5),
            ))
            .unwrap();
        scene
            .register(InteractionElement::control(
                "b",
                Layer::Root,
                Rect::new(2, 1, 4, 2),
            ))
            .unwrap();
        assert_eq!(scene.hit_test(Position::new(3, 2)).map(|e| e.id), Some("b"));
    }

    #[test]
    fn trapping_layer_blocks_lower_pointer_hits_but_accepts_own_hits() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "main",
                Layer::Root,
                Rect::new(5, 0, 2, 1),
            ))
            .unwrap();
        scene
            .register(InteractionElement::control(
                "other",
                Layer::Root,
                Rect::new(0, 0, 2, 1),
            ))
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focused(), Some(&"main"));

        scene.push_layer(dialog_trap());
        scene
            .register(InteractionElement::control(
                "dialog",
                Layer::Dialog,
                Rect::new(10, 0, 2, 1),
            ))
            .unwrap();

        assert!(scene.hit_test(Position::new(0, 0)).is_none());
        assert_eq!(
            scene.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(0, 0),
                modifiers: KeyModifiers::NONE,
            }),
            InteractionOutcome::Ignored
        );
        assert_eq!(scene.focused(), Some(&"main"));
        assert_eq!(
            scene.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(10, 0),
                modifiers: KeyModifiers::NONE,
            }),
            InteractionOutcome::FocusChanged {
                from: Some("main"),
                to: Some("dialog"),
            }
        );
    }

    #[test]
    fn non_dismissible_top_blocks_escape_for_lower_layers() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene.push_layer(menu_layer());
        scene.push_layer(dialog_trap());
        // Old OverlayHost bug would peel Menu under Dialog. Scene must not.
        assert_eq!(scene.handle_escape(), InteractionOutcome::Ignored);
        assert_eq!(scene.layers().len(), 3);
        assert_eq!(scene.top_layer().map(|l| l.id), Some(Layer::Dialog));
    }

    #[test]
    fn dismiss_top_restores_focus_return() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "main",
                Layer::Root,
                Rect::new(0, 0, 4, 1),
            ))
            .unwrap();
        scene.push_layer(menu_layer());
        scene
            .register(InteractionElement::control(
                "item",
                Layer::Menu,
                Rect::new(0, 2, 4, 1),
            ))
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focused(), Some(&"item"));
        let outcome = scene.handle_escape();
        assert!(matches!(
            outcome,
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                focus: Some("main"),
            }
        ));
        assert_eq!(scene.focused(), Some(&"main"));
    }

    #[test]
    fn invalid_focus_return_reconciles_after_escape() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "main",
                Layer::Root,
                Rect::new(0, 0, 4, 1),
            ))
            .unwrap();
        let mut layer = menu_layer();
        layer.focus_return = Some("missing");
        scene.push_layer(layer);
        scene
            .register(InteractionElement::control(
                "item",
                Layer::Menu,
                Rect::new(0, 2, 4, 1),
            ))
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focused(), Some(&"item"));

        assert!(matches!(
            scene.handle_escape(),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                focus: Some("main"),
            }
        ));
        assert_eq!(scene.focused(), Some(&"main"));
    }

    #[test]
    fn disabled_focus_return_reconciles_after_outside_click() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "main",
                Layer::Root,
                Rect::new(0, 0, 4, 1),
            ))
            .unwrap();
        scene
            .register(
                InteractionElement::control("disabled", Layer::Root, Rect::new(5, 0, 4, 1))
                    .enabled(false),
            )
            .unwrap();
        let mut layer = menu_layer();
        layer.focus_return = Some("disabled");
        scene.push_layer(layer);
        scene
            .register(InteractionElement::control(
                "item",
                Layer::Menu,
                Rect::new(0, 2, 4, 1),
            ))
            .unwrap();
        scene.reconcile();

        let outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(9, 9),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            scene.handle_mouse(outside),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                focus: Some("main"),
            }
        ));
        assert_eq!(scene.focused(), Some(&"main"));
    }

    #[test]
    fn hidden_focus_return_reconciles_after_escape() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "main",
                Layer::Root,
                Rect::new(0, 0, 4, 1),
            ))
            .unwrap();
        scene
            .register(
                InteractionElement::control("hidden", Layer::Root, Rect::new(5, 0, 4, 1))
                    .hidden(true),
            )
            .unwrap();
        let mut layer = menu_layer();
        layer.focus_return = Some("hidden");
        scene.push_layer(layer);
        scene
            .register(InteractionElement::control(
                "item",
                Layer::Menu,
                Rect::new(0, 2, 4, 1),
            ))
            .unwrap();
        scene.reconcile();

        assert!(matches!(
            scene.handle_escape(),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                focus: Some("main"),
            }
        ));
        assert_eq!(scene.focused(), Some(&"main"));
    }

    #[test]
    fn popped_layer_elements_are_not_focusable_after_last_layer_closes() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.push_layer(menu_layer());
        scene
            .register(
                InteractionElement::control("item", Layer::Menu, Rect::new(0, 0, 4, 1))
                    .actions(vec![Act::Confirm]),
            )
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focused(), Some(&"item"));

        assert_eq!(
            scene.handle_escape(),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                focus: None,
            }
        );
        assert_eq!(scene.focused(), None);
        assert!(scene.hit_test(Position::new(1, 0)).is_none());
        scene.focused = Some("item");
        assert_eq!(
            scene.dispatch_action(Act::Confirm),
            InteractionOutcome::Ignored
        );
    }

    #[test]
    fn inactive_layer_focus_return_reconciles_to_active_layer() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "main",
                Layer::Root,
                Rect::new(0, 0, 4, 1),
            ))
            .unwrap();
        scene.push_layer(menu_layer());
        scene
            .register(InteractionElement::control(
                "menu-item",
                Layer::Menu,
                Rect::new(0, 2, 4, 1),
            ))
            .unwrap();
        let mut dialog = menu_layer();
        dialog.id = Layer::Dialog;
        dialog.focus_return = Some("main");
        scene.push_layer(dialog);
        scene
            .register(InteractionElement::control(
                "dialog-item",
                Layer::Dialog,
                Rect::new(0, 4, 4, 1),
            ))
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focused(), Some(&"dialog-item"));

        assert!(matches!(
            scene.handle_escape(),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Dialog,
                focus: Some("menu-item"),
            }
        ));
        assert_eq!(scene.focused(), Some(&"menu-item"));
    }

    #[test]
    fn nested_menus_peel_one_layer_per_escape() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene.push_layer(menu_layer());
        scene.push_layer(InteractionLayer {
            id: Layer::Dialog, // reuse as second menu-like dismissible
            kind: LayerKind::Menu,
            owns_input: true,
            esc: LayerDismissPolicy::Dismissible,
            outside: LayerDismissPolicy::Dismissible,
            focus_return: None,
        });
        assert!(matches!(
            scene.handle_escape(),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Dialog,
                ..
            }
        ));
        assert_eq!(scene.top_layer().map(|l| l.id), Some(Layer::Menu));
        assert!(matches!(
            scene.handle_escape(),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                ..
            }
        ));
        assert_eq!(scene.handle_escape(), InteractionOutcome::UnhandledEscape);
    }

    #[test]
    fn outside_click_only_applies_top_policy() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene.push_layer(menu_layer());
        scene
            .register(InteractionElement::control(
                "item",
                Layer::Menu,
                Rect::new(0, 0, 2, 1),
            ))
            .unwrap();
        let outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(9, 9),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            scene.handle_mouse(outside),
            InteractionOutcome::LayerDismissed {
                layer: Layer::Menu,
                ..
            }
        ));
    }

    #[test]
    fn focus_preserved_by_stable_id_across_reorder() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "a",
                Layer::Root,
                Rect::new(0, 0, 1, 1),
            ))
            .unwrap();
        scene
            .register(InteractionElement::control(
                "b",
                Layer::Root,
                Rect::new(1, 0, 1, 1),
            ))
            .unwrap();
        scene.reconcile();
        let _ = scene.focus("b");
        scene.begin_frame();
        // Reorder: b first, a second.
        scene
            .register(InteractionElement::control(
                "b",
                Layer::Root,
                Rect::new(5, 0, 1, 1),
            ))
            .unwrap();
        scene
            .register(InteractionElement::control(
                "a",
                Layer::Root,
                Rect::new(0, 0, 1, 1),
            ))
            .unwrap();
        scene.reconcile();
        assert_eq!(scene.focused(), Some(&"b"));
    }

    #[test]
    fn available_actions_match_dispatch_surface() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(
                InteractionElement::control("a", Layer::Root, Rect::new(0, 0, 1, 1))
                    .actions(vec![Act::Open, Act::Confirm]),
            )
            .unwrap();
        scene
            .register(
                InteractionElement::control("b", Layer::Root, Rect::new(1, 0, 1, 1))
                    .enabled(false)
                    .actions(vec![Act::Cancel]),
            )
            .unwrap();
        scene.reconcile();
        let available = scene.available_actions();
        assert!(available.contains(&Act::Open));
        assert!(available.contains(&Act::Confirm));
        assert!(!available.contains(&Act::Cancel));
        assert!(matches!(
            scene.dispatch_action(Act::Open),
            InteractionOutcome::Action {
                target: "a",
                action: Act::Open
            }
        ));
        assert_eq!(
            scene.dispatch_action(Act::Cancel),
            InteractionOutcome::Ignored
        );
    }

    #[test]
    fn unhandled_escape_on_root_only() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        assert_eq!(scene.handle_escape(), InteractionOutcome::UnhandledEscape);
    }

    #[test]
    fn release_keys_ignored_for_tab() {
        let mut scene = InteractionScene::<&str, Layer, Act>::new();
        scene.ensure_root(root_layer());
        scene
            .register(InteractionElement::control(
                "a",
                Layer::Root,
                Rect::new(0, 0, 1, 1),
            ))
            .unwrap();
        scene.reconcile();
        let mut key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        key.kind = KeyEventKind::Release;
        assert_eq!(scene.handle_key_tab_esc(key), InteractionOutcome::Ignored);
    }

    // ── SemanticScene ─────────────────────────────────────────────────────

    #[test]
    fn semantic_tree_parent_path_and_children() {
        let mut scene = SemanticScene::<&str, Act>::new();
        scene
            .register(
                SemanticNode::content("list", Rect::new(0, 0, 20, 10))
                    .role(SemanticRole::List)
                    .label("Files"),
            )
            .unwrap();
        scene
            .register_child(
                "list",
                SemanticNode::control("row0", Rect::new(0, 1, 20, 1))
                    .role(SemanticRole::ListItem)
                    .label("a.rs")
                    .state(SemanticState {
                        selected: true,
                        ..SemanticState::default()
                    })
                    .actions(vec![Act::Open]),
            )
            .unwrap();
        scene
            .register_child(
                "list",
                SemanticNode::control("row1", Rect::new(0, 2, 20, 1))
                    .role(SemanticRole::ListItem)
                    .label("b.rs")
                    .disabled(true),
            )
            .unwrap();

        assert_eq!(scene.children_of(&"list").len(), 2);
        assert_eq!(scene.path_to(&"row0"), vec![&"list", &"row0"]);
        assert_eq!(scene.focus_order(), vec![&"row0"]); // row1 disabled
        assert_eq!(
            scene.hit_test(Position::new(2, 1)).map(|n| n.id),
            Some("row0")
        );
        // Disabled row still visible to inspection hit_test…
        assert_eq!(
            scene.hit_test(Position::new(2, 2)).map(|n| n.id),
            Some("row1")
        );
        // …but interactive routing skips it and falls through to parent list.
        assert_eq!(
            scene
                .hit_test_interactive(Position::new(2, 2))
                .map(|n| n.id),
            Some("list")
        );
    }

    #[test]
    fn semantic_duplicate_and_unknown_parent_diagnostics() {
        let mut scene = SemanticScene::<&str>::new();
        scene
            .register(SemanticNode::<&str>::control("a", Rect::new(0, 0, 1, 1)))
            .unwrap();
        assert_eq!(
            scene.register(SemanticNode::<&str>::control("a", Rect::new(1, 0, 1, 1))),
            Err(SemanticError::DuplicateId)
        );
        assert_eq!(
            scene.register(
                SemanticNode::<&str>::control("b", Rect::new(0, 0, 1, 1)).parent("missing")
            ),
            Err(SemanticError::UnknownParent)
        );
        assert_eq!(
            scene.register(SemanticNode::<&str>::control("c", Rect::new(0, 0, 1, 1)).parent("c")),
            Err(SemanticError::SelfParent)
        );
        assert!(!scene.diagnostics().is_empty());
    }

    #[test]
    fn semantic_snapshot_text_and_help() {
        let mut scene = SemanticScene::<&str, Act>::new();
        scene
            .register(
                SemanticNode::control("ok", Rect::new(0, 0, 4, 1))
                    .role(SemanticRole::Button)
                    .label("Confirm")
                    .description("Accept the change")
                    .actions(vec![Act::Confirm]),
            )
            .unwrap();
        let snap = scene.snapshot_with(|a| format!("{a:?}"));
        assert_eq!(snap.nodes.len(), 1);
        assert_eq!(snap.nodes[0].role, "button");
        assert!(snap.to_text().contains("id=ok"));
        assert!(snap.summary_lines(4)[0].contains("ok@button"));
        let help = scene.help_lines();
        assert_eq!(help, vec!["Confirm — Accept the change".to_string()]);
        assert_eq!(scene.jump_regions().len(), 1);
    }

    #[test]
    fn semantic_from_interaction_adapter() {
        let mut iscene = InteractionScene::<&str, Layer, Act>::new();
        iscene.ensure_root(root_layer());
        iscene
            .register(
                InteractionElement::control("main", Layer::Root, Rect::new(0, 0, 5, 1))
                    .actions(vec![Act::Open]),
            )
            .unwrap();
        let semantic = SemanticScene::from_interaction(&iscene);
        assert_eq!(semantic.nodes().len(), 1);
        assert!(!semantic.nodes()[0].disabled);
        assert_eq!(semantic.nodes()[0].actions, vec![Act::Open]);
    }

    #[test]
    fn semantic_begin_frame_clears_and_reserve_is_cheap() {
        let mut scene = SemanticScene::<usize>::new();
        scene.reserve(10_000);
        for i in 0..500 {
            scene
                .register(SemanticNode::control(i, Rect::new(0, i as u16 % 50, 10, 1)))
                .unwrap();
        }
        assert_eq!(scene.nodes().len(), 500);
        scene.begin_frame();
        assert!(scene.nodes().is_empty());
        assert!(scene.diagnostics().is_empty());
    }

    #[test]
    fn semantic_empty_area_records_diagnostic_but_registers() {
        let mut scene = SemanticScene::<&str>::new();
        scene
            .register(SemanticNode::<&str>::control(
                "ghost",
                Rect::new(0, 0, 0, 0),
            ))
            .unwrap();
        assert_eq!(scene.nodes().len(), 1);
        assert!(matches!(
            scene.diagnostics(),
            [SemanticDiagnostic::EmptyArea { .. }]
        ));
        assert!(scene.hit_test(Position::new(0, 0)).is_none());
    }

    #[test]
    fn semantic_focus_neighbor_and_by_role() {
        let mut scene = SemanticScene::<&str>::new();
        scene
            .register(
                SemanticNode::control("a", Rect::new(0, 0, 2, 1))
                    .role(SemanticRole::Button)
                    .label("A"),
            )
            .unwrap();
        scene
            .register(
                SemanticNode::control("b", Rect::new(2, 0, 2, 1))
                    .role(SemanticRole::Button)
                    .label("B"),
            )
            .unwrap();
        scene
            .register(
                SemanticNode::content("note", Rect::new(0, 1, 4, 1)).role(SemanticRole::Heading),
            )
            .unwrap();
        assert_eq!(scene.by_role(SemanticRole::Button).len(), 2);
        assert_eq!(scene.focus_neighbor(Some(&"a"), true, false), Some(&"b"));
        assert_eq!(scene.focus_neighbor(Some(&"b"), true, true), Some(&"a"));
        assert_eq!(
            scene.hit_test_focusable(Position::new(3, 0)).map(|n| n.id),
            Some("b")
        );
    }

    #[test]
    fn semantic_snapshot_roundtrip_text() {
        let mut scene = SemanticScene::<&str, Act>::new();
        scene
            .register(
                SemanticNode::control("ok", Rect::new(1, 2, 8, 1))
                    .role(SemanticRole::Button)
                    .label("Confirm")
                    .state(SemanticState {
                        pressed: true,
                        ..Default::default()
                    })
                    .actions(vec![Act::Confirm]),
            )
            .unwrap();
        let snap = scene.snapshot_with(|a| format!("{a:?}"));
        let text = snap.to_text();
        let parsed = SemanticSnapshot::from_text(&text);
        assert_eq!(parsed.nodes.len(), 1);
        assert_eq!(parsed.nodes[0].id, "ok");
        assert_eq!(parsed.nodes[0].role, "button");
        assert!(parsed.nodes[0].pressed);
        assert_eq!(parsed.nodes[0].x, 1);
        assert_eq!(parsed.focusable_ids(), vec!["ok"]);
        let again = SemanticSnapshot::from_text(&parsed.to_text());
        assert_eq!(again.nodes[0].id, snap.nodes[0].id);
    }

    #[test]
    fn semantic_register_many_and_focus_nodes() {
        let mut scene = SemanticScene::<usize>::new();
        scene.reserve(200);
        scene.register_many((0..100).map(|i| {
            SemanticNode::control(i, Rect::new(0, (i % 20) as u16, 12, 1)).label(format!("r{i}"))
        }));
        assert_eq!(scene.len(), 100);
        let focus_nodes = scene.to_focus_nodes();
        assert_eq!(focus_nodes.len(), 100);
        assert!(focus_nodes[0].focusable);
    }

    #[test]
    fn semantic_action_catalog() {
        let mut scene = SemanticScene::<&str, Act>::new();
        scene
            .register(
                SemanticNode::control("ok", Rect::new(0, 0, 2, 1)).actions(vec![Act::Confirm]),
            )
            .unwrap();
        scene
            .register(
                SemanticNode::control("x", Rect::new(2, 0, 2, 1))
                    .disabled(true)
                    .actions(vec![Act::Cancel]),
            )
            .unwrap();
        let catalog = scene.action_catalog();
        assert_eq!(catalog.len(), 1);
        assert_eq!(*catalog[0].0, "ok");
    }

    #[test]
    fn semantic_virtualizer_window_not_full_len() {
        // Only register the visible window (contract for large lists).
        let logical = 1_000_000usize;
        let visible = 40usize;
        let offset = 250_000usize;
        let mut scene = SemanticScene::<usize>::new();
        scene.reserve(visible);
        scene.register_many((0..visible).map(|i| {
            let id = offset + i;
            SemanticNode::control(id, Rect::new(0, i as u16, 20, 1)).label(format!("row-{id}"))
        }));
        assert_eq!(scene.len(), visible);
        assert!(scene.len() << 10 < logical); // << logical
        assert!(scene.get(&(offset + 5)).is_some());
        assert!(scene.get(&0).is_none());
    }
}
