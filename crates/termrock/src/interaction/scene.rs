// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Per-frame interaction scene: one authority for focus, hits, layers, actions.
//!
//! Immediate-mode registration each frame. Stable focus and open layers persist
//! across frames. No callbacks, effects, or domain policy.

use ratatui_core::layout::{Position, Rect};

use crate::input::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// Semantic role of a registered element for discovery and tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SemanticRole {
    /// Ordinary content.
    #[default]
    Content,
    /// Focusable control.
    Control,
    /// Overlay / modal chrome.
    Overlay,
    /// Status or chrome strip.
    Chrome,
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
    /// Duplicate layer id.
    DuplicateLayer,
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
                && self.layer_accepts_focus(&element.layer)
        })
    }

    fn layer_accepts_focus(&self, layer_id: &LayerId) -> bool
    where
        LayerId: PartialEq,
    {
        // Only elements on the top input-owning layer (or root if none) may
        // receive focus while a modal stack is open.
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
                    && self.layer_accepts_focus(&element.layer)
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
    pub fn hit_test(&self, position: Position) -> Option<&InteractionElement<Id, LayerId, Action>> {
        self.elements
            .iter()
            .rev()
            .find(|element| element.enabled && !element.hidden && element.area.contains(position))
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
        if key.kind == KeyEventKind::Release {
            return InteractionOutcome::Ignored;
        }
        match key.code {
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.focus_move(true),
            KeyCode::Tab => self.focus_move(false),
            KeyCode::BackTab => self.focus_move(true),
            KeyCode::Esc if key.kind == KeyEventKind::Press => self.handle_escape(),
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
                if let Some(id) = layer.focus_return.clone() {
                    self.focused = Some(id.clone());
                    InteractionOutcome::LayerDismissed {
                        layer: layer.id,
                        focus: Some(id),
                    }
                } else {
                    self.reconcile();
                    InteractionOutcome::LayerDismissed {
                        layer: layer.id,
                        focus: self.focused.clone(),
                    }
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
        if let Some(id) = layer.focus_return.clone() {
            self.focused = Some(id.clone());
            return InteractionOutcome::LayerDismissed {
                layer: layer.id,
                focus: Some(id),
            };
        }
        self.reconcile();
        InteractionOutcome::LayerDismissed {
            layer: layer.id,
            focus: self.focused.clone(),
        }
    }

    /// Dispatches `action` if available on the focused element (or any active).
    pub fn dispatch_action(&self, action: Action) -> InteractionOutcome<Id, LayerId, Action>
    where
        Id: Clone + PartialEq,
        LayerId: PartialEq,
        Action: Clone + PartialEq,
    {
        if let Some(focus_id) = &self.focused
            && let Some(element) = self.get(focus_id)
            && element.enabled
            && !element.hidden
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

// ── Compatibility thin wrappers (semantic scene shape) ─────────────────────

/// Legacy element without layers/actions (prefer [`InteractionElement`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticElement<Id> {
    /// Stable identity across frames.
    pub id: Id,
    /// Painted rectangle.
    pub area: Rect,
    /// Whether the element may receive focus.
    pub focusable: bool,
    /// Whether the element is enabled.
    pub enabled: bool,
    /// Semantic classification.
    pub role: SemanticRole,
}

/// Legacy per-frame registry (prefer [`InteractionScene`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticScene<Id> {
    elements: Vec<SemanticElement<Id>>,
}

impl<Id> Default for SemanticScene<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> SemanticScene<Id> {
    /// Creates an empty scene.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }

    /// Clears registrations for a new frame.
    pub fn begin_frame(&mut self) {
        self.elements.clear();
    }

    /// Registers one element (later duplicates with same id are ignored).
    pub fn register(&mut self, element: SemanticElement<Id>)
    where
        Id: PartialEq,
    {
        if self.elements.iter().any(|item| item.id == element.id) {
            return;
        }
        self.elements.push(element);
    }

    /// All registered elements in registration order.
    #[must_use]
    pub fn elements(&self) -> &[SemanticElement<Id>] {
        &self.elements
    }

    /// First enabled focusable element containing `position`.
    #[must_use]
    pub fn hit_test(&self, position: Position) -> Option<&SemanticElement<Id>> {
        self.elements
            .iter()
            .rev()
            .find(|element| element.enabled && element.focusable && element.area.contains(position))
    }

    /// Focusable enabled ids in registration order.
    #[must_use]
    pub fn focus_order(&self) -> Vec<&Id> {
        self.elements
            .iter()
            .filter(|element| element.focusable && element.enabled)
            .map(|element| &element.id)
            .collect()
    }

    /// Looks up an element by id.
    #[must_use]
    pub fn get(&self, id: &Id) -> Option<&SemanticElement<Id>>
    where
        Id: PartialEq,
    {
        self.elements.iter().find(|element| &element.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
