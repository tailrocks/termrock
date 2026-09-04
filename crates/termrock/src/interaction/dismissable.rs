// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Reusable dismissal behavior for transient interactive surfaces.
//!
//! Conceptual cousin of Radix **DismissableLayer**, translated to terminal
//! event semantics: Escape, outside pointer press/release, focus leave, parent
//! closure, explicit dismiss, and non-dismissable critical traps.
//!
//! [`OverlayStack`] and [`super::InteractionScene`] remain the **hosts** that
//! own z-order and geometry; this module owns **policy evaluation**, nested
//! capture/bubble, pointer gesture safety, and single-event double-dismiss
//! prevention.
use ratatui_core::layout::{Position, Rect};

use super::scene::LayerDismissPolicy;

/// Why a layer is being asked to dismiss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DismissReason {
    /// Escape / Cancel intent on the top layer.
    Escape,
    /// Pointer interaction outside the layer rect (completed gesture).
    OutsidePointer,
    /// Focus left the layer's focus scope (host-detected).
    FocusLeave,
    /// Parent overlay/layer closed (cascade).
    ParentClosed,
    /// Caller requested dismiss (`dismiss` / action).
    Explicit,
}

/// Per-trigger action when a dismiss attempt reaches a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DismissAction {
    /// Allow dismissal of this layer.
    #[default]
    Dismiss,
    /// Consume the event; do not dismiss; do not bubble (critical / alert).
    Trap,
    /// Do not dismiss; bubble to parent / host (transparent layer).
    Bubble,
}

impl DismissAction {
    /// Map the scene layer policy into its concrete dismiss action.
    #[must_use]
    pub const fn from_layer(policy: LayerDismissPolicy) -> Self {
        match policy {
            LayerDismissPolicy::Dismissible => Self::Dismiss,
            LayerDismissPolicy::Trap => Self::Trap,
            LayerDismissPolicy::Ignore => Self::Bubble,
        }
    }
}

/// Full policy bundle (Radix-like knobs, terminal-shaped).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DismissPolicy {
    /// Escape key / Cancel intent.
    pub escape: DismissAction,
    /// Completed outside pointer gesture.
    pub outside: DismissAction,
    /// Focus left this surface.
    pub focus_leave: DismissAction,
    /// Parent closed (usually always dismiss children).
    pub parent_closed: DismissAction,
    /// Explicit `dismiss()` API.
    pub explicit: DismissAction,
}

impl Default for DismissPolicy {
    fn default() -> Self {
        Self::dismissible()
    }
}

impl DismissPolicy {
    /// Menu / popover defaults: Esc + outside dismiss; focus leave dismisses.
    #[must_use]
    pub const fn dismissible() -> Self {
        Self {
            escape: DismissAction::Dismiss,
            outside: DismissAction::Dismiss,
            focus_leave: DismissAction::Dismiss,
            parent_closed: DismissAction::Dismiss,
            explicit: DismissAction::Dismiss,
        }
    }

    /// Alert / permission critical: trap Esc + outside; parent still cascades.
    #[must_use]
    pub const fn critical() -> Self {
        Self {
            escape: DismissAction::Trap,
            outside: DismissAction::Trap,
            focus_leave: DismissAction::Trap,
            parent_closed: DismissAction::Dismiss,
            explicit: DismissAction::Dismiss,
        }
    }

    /// Tooltip-like: Esc bubbles; outside dismisses; no focus ownership.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            escape: DismissAction::Bubble,
            outside: DismissAction::Dismiss,
            focus_leave: DismissAction::Bubble,
            parent_closed: DismissAction::Dismiss,
            explicit: DismissAction::Dismiss,
        }
    }

    /// From overlay esc/outside pair (other triggers default).
    #[must_use]
    pub const fn from_layer_pair(esc: LayerDismissPolicy, outside: LayerDismissPolicy) -> Self {
        Self {
            escape: DismissAction::from_layer(esc),
            outside: DismissAction::from_layer(outside),
            focus_leave: DismissAction::from_layer(esc),
            parent_closed: DismissAction::Dismiss,
            explicit: DismissAction::Dismiss,
        }
    }

    /// Action for a reason.
    #[must_use]
    pub const fn action_for(self, reason: DismissReason) -> DismissAction {
        match reason {
            DismissReason::Escape => self.escape,
            DismissReason::OutsidePointer => self.outside,
            DismissReason::FocusLeave => self.focus_leave,
            DismissReason::ParentClosed => self.parent_closed,
            DismissReason::Explicit => self.explicit,
        }
    }
}

/// Capture vs bubble phase for nested evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DismissPhase {
    /// Topmost layer first (terminal Esc / outside default).
    #[default]
    CaptureTopFirst,
    /// Root-first then children (rare; host opt-in).
    BubbleBottomFirst,
}

/// Result of evaluating one dismiss attempt on one layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DismissDecision {
    /// No effect (idle / wrong gesture / already handled this event).
    None,
    /// Dismiss this layer for `reason`.
    Dismiss {
        /// Why.
        reason: DismissReason,
    },
    /// Event consumed; layer stays; do not peel lower layers.
    Consumed,
    /// Pass to the next outer host / parent layer.
    Bubble,
}

impl DismissDecision {
    /// Whether the layer should close.
    #[must_use]
    pub const fn should_dismiss(self) -> bool {
        matches!(self, Self::Dismiss { .. })
    }

    /// Whether the event stops at this layer.
    #[must_use]
    pub const fn stops_propagation(self) -> bool {
        matches!(self, Self::Dismiss { .. } | Self::Consumed)
    }
}

/// Monotonic event id so one input cannot dismiss twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DismissEventId(pub u64);

/// Prevents double dismissal from a single logical input event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DismissGuard {
    last_event: u64,
    dismissed: bool,
}

impl DismissGuard {
    /// Fresh guard.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_event: 0,
            dismissed: false,
        }
    }

    /// Begin handling `event`. Returns false if a dismiss already fired for it.
    pub fn begin(&mut self, event: DismissEventId) -> bool {
        if event.0 == self.last_event && self.dismissed {
            return false;
        }
        if event.0 != self.last_event {
            self.last_event = event.0;
            self.dismissed = false;
        }
        true
    }

    /// Record that a dismiss occurred for the current event.
    pub fn mark_dismissed(&mut self) {
        self.dismissed = true;
    }
}

/// Pointer press/release tracking for outside-dismiss safety (Radix drag cancel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PointerGesture {
    /// No active button.
    #[default]
    Idle,
    /// Press started outside the layer (candidate outside dismiss).
    PressOutside {
        /// Press position.
        at: Position,
    },
    /// Press started inside (outside release must not dismiss).
    PressInside {
        /// Press position.
        at: Position,
    },
}

/// Stateful dismiss controller for one surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DismissableLayer {
    policy: DismissPolicy,
    /// Hit geometry for outside tests (host updates each paint).
    rect: Rect,
    gesture: PointerGesture,
    /// When true, empty rect is never "outside" (hidden layer).
    ignore_empty_rect: bool,
}

impl Default for DismissableLayer {
    fn default() -> Self {
        Self::new(DismissPolicy::dismissible())
    }
}

impl DismissableLayer {
    /// Layer with policy.
    #[must_use]
    pub const fn new(policy: DismissPolicy) -> Self {
        Self {
            policy,
            rect: Rect::new(0, 0, 0, 0),
            gesture: PointerGesture::Idle,
            ignore_empty_rect: true,
        }
    }
    /// Replace policy.
    pub fn set_policy(&mut self, policy: DismissPolicy) {
        self.policy = policy;
    }

    /// Update geometry after layout.
    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    /// Reset gesture (layer became non-top or dismissed).
    pub fn reset_gesture(&mut self) {
        self.gesture = PointerGesture::Idle;
    }

    /// Whether `pos` is outside the layer body.
    #[must_use]
    pub fn is_outside(&self, pos: Position) -> bool {
        if self.rect.width == 0 || self.rect.height == 0 {
            // Hidden / zero area: treat as outside only when not ignored.
            return !self.ignore_empty_rect;
        }
        !self.rect.contains(pos)
    }

    fn decide(&self, reason: DismissReason) -> DismissDecision {
        match self.policy.action_for(reason) {
            DismissAction::Dismiss => DismissDecision::Dismiss { reason },
            DismissAction::Trap => DismissDecision::Consumed,
            DismissAction::Bubble => DismissDecision::Bubble,
        }
    }

    /// Escape / Cancel.
    pub fn on_escape(
        &mut self,
        guard: &mut DismissGuard,
        event: DismissEventId,
    ) -> DismissDecision {
        if !guard.begin(event) {
            return DismissDecision::None;
        }
        let d = self.decide(DismissReason::Escape);
        if d.should_dismiss() {
            guard.mark_dismissed();
            self.reset_gesture();
        }
        d
    }
    /// Parent overlay closed — children always evaluate parent_closed policy.
    pub fn on_parent_closed(
        &mut self,
        guard: &mut DismissGuard,
        event: DismissEventId,
    ) -> DismissDecision {
        if !guard.begin(event) {
            return DismissDecision::None;
        }
        let d = self.decide(DismissReason::ParentClosed);
        if d.should_dismiss() {
            guard.mark_dismissed();
            self.reset_gesture();
        }
        d
    }

    /// Explicit dismiss API.
    pub fn on_explicit(
        &mut self,
        guard: &mut DismissGuard,
        event: DismissEventId,
    ) -> DismissDecision {
        if !guard.begin(event) {
            return DismissDecision::None;
        }
        let d = self.decide(DismissReason::Explicit);
        if d.should_dismiss() {
            guard.mark_dismissed();
            self.reset_gesture();
        }
        d
    }

    /// Pointer button down (start outside-dismiss gesture).
    pub fn on_pointer_down(
        &mut self,
        pos: Position,
        guard: &mut DismissGuard,
        event: DismissEventId,
    ) -> DismissDecision {
        if !guard.begin(event) {
            return DismissDecision::None;
        }
        if self.is_outside(pos) {
            self.gesture = PointerGesture::PressOutside { at: pos };
            // Press alone does not dismiss — wait for release (drag cancel).
            // Trap still consumes press so lower layers don't see it.
            match self.policy.outside {
                DismissAction::Trap => DismissDecision::Consumed,
                DismissAction::Bubble => DismissDecision::Bubble,
                DismissAction::Dismiss => DismissDecision::None,
            }
        } else {
            self.gesture = PointerGesture::PressInside { at: pos };
            DismissDecision::None
        }
    }

    /// Pointer button up — complete outside dismiss only if press was outside
    /// and release is still outside (Radix-style interact-outside).
    pub fn on_pointer_up(
        &mut self,
        pos: Position,
        guard: &mut DismissGuard,
        event: DismissEventId,
    ) -> DismissDecision {
        if !guard.begin(event) {
            self.gesture = PointerGesture::Idle;
            return DismissDecision::None;
        }
        let gesture = self.gesture;
        self.gesture = PointerGesture::Idle;
        match gesture {
            PointerGesture::PressOutside { .. } if self.is_outside(pos) => {
                let d = self.decide(DismissReason::OutsidePointer);
                if d.should_dismiss() {
                    guard.mark_dismissed();
                }
                d
            }
            PointerGesture::PressOutside { .. } => {
                // Released inside after outside press: cancel dismiss.
                DismissDecision::None
            }
            PointerGesture::PressInside { .. } => {
                // Drag from inside to outside: do not dismiss (accidental).
                DismissDecision::None
            }
            PointerGesture::Idle => DismissDecision::None,
        }
    }

    /// Convenience: treat a single outside click as press+release (tests / simple hosts).
    pub fn on_outside_click(
        &mut self,
        pos: Position,
        guard: &mut DismissGuard,
        event: DismissEventId,
    ) -> DismissDecision {
        if !self.is_outside(pos) {
            return DismissDecision::None;
        }
        let _ = self.on_pointer_down(pos, guard, event);
        // Same event id: begin() allows continue until mark_dismissed.
        self.on_pointer_up(pos, guard, event)
    }
}

/// Nested stack evaluation: which layer index reacts to Esc (top-first capture).
///
/// Returns `(index_from_bottom, decision)` for the layer that stops the event.
pub fn evaluate_escape_stack(
    layers: &mut [DismissableLayer],
    guard: &mut DismissGuard,
    event: DismissEventId,
    phase: DismissPhase,
) -> Option<(usize, DismissDecision)> {
    if layers.is_empty() {
        return None;
    }
    match phase {
        DismissPhase::CaptureTopFirst => {
            for index in (0..layers.len()).rev() {
                let decision = layers[index].on_escape(guard, event);
                if decision.stops_propagation() || decision.should_dismiss() {
                    return Some((index, decision));
                }
                if matches!(decision, DismissDecision::None) {
                    // Transparent — continue outer.
                    continue;
                }
            }
            None
        }
        DismissPhase::BubbleBottomFirst => {
            for index in 0..layers.len() {
                let decision = layers[index].on_escape(guard, event);
                if decision.stops_propagation() || decision.should_dismiss() {
                    return Some((index, decision));
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu_layer(rect: Rect) -> DismissableLayer {
        let mut l = DismissableLayer::new(DismissPolicy::dismissible());
        l.set_rect(rect);
        l
    }

    #[test]
    fn escape_dismisses_dismissible_traps_critical() {
        let mut g = DismissGuard::new();
        let mut menu = menu_layer(Rect::new(0, 0, 10, 5));
        let e = DismissEventId(1);
        assert!(matches!(
            menu.on_escape(&mut g, e),
            DismissDecision::Dismiss {
                reason: DismissReason::Escape
            }
        ));
        // Double dismiss blocked on same event.
        assert_eq!(menu.on_escape(&mut g, e), DismissDecision::None);

        let mut alert = DismissableLayer::new(DismissPolicy::critical());
        alert.set_rect(Rect::new(0, 0, 10, 5));
        let mut g2 = DismissGuard::new();
        assert_eq!(
            alert.on_escape(&mut g2, DismissEventId(2)),
            DismissDecision::Consumed
        );
    }

    #[test]
    fn outside_requires_press_and_release_outside() {
        let mut layer = menu_layer(Rect::new(10, 10, 20, 10));
        let mut g = DismissGuard::new();
        let e = DismissEventId(1);
        // press outside
        assert_eq!(
            layer.on_pointer_down(Position::new(0, 0), &mut g, e),
            DismissDecision::None
        );
        // release inside → cancel
        assert_eq!(
            layer.on_pointer_up(Position::new(15, 12), &mut g, e),
            DismissDecision::None
        );
        // full outside gesture
        let e2 = DismissEventId(2);
        let mut g2 = DismissGuard::new();
        let _ = layer.on_pointer_down(Position::new(0, 0), &mut g2, e2);
        assert!(matches!(
            layer.on_pointer_up(Position::new(0, 0), &mut g2, e2),
            DismissDecision::Dismiss {
                reason: DismissReason::OutsidePointer
            }
        ));
    }

    #[test]
    fn drag_from_inside_to_outside_does_not_dismiss() {
        let mut layer = menu_layer(Rect::new(10, 10, 20, 10));
        let mut g = DismissGuard::new();
        let e = DismissEventId(1);
        let _ = layer.on_pointer_down(Position::new(15, 12), &mut g, e);
        assert_eq!(
            layer.on_pointer_up(Position::new(0, 0), &mut g, e),
            DismissDecision::None
        );
    }

    #[test]
    fn nested_escape_top_first_one_layer_only() {
        let mut root = menu_layer(Rect::new(0, 0, 80, 24));
        let mut child = menu_layer(Rect::new(10, 10, 20, 8));
        let mut layers = [root.clone(), child.clone()];
        let mut g = DismissGuard::new();
        let e = DismissEventId(1);
        let hit = evaluate_escape_stack(&mut layers, &mut g, e, DismissPhase::CaptureTopFirst);
        assert!(matches!(
            hit,
            Some((
                1,
                DismissDecision::Dismiss {
                    reason: DismissReason::Escape
                }
            ))
        ));
        // Same event cannot dismiss root.
        let hit2 = evaluate_escape_stack(&mut layers, &mut g, e, DismissPhase::CaptureTopFirst);
        assert!(hit2.is_none() || matches!(hit2, Some((_, DismissDecision::None))));
        let _ = (&mut root, &mut child);
    }

    #[test]
    fn trap_on_top_protects_layers_beneath() {
        let mut stack = [menu_layer(Rect::new(0, 0, 80, 24)), {
            let mut a = DismissableLayer::new(DismissPolicy::critical());
            a.set_rect(Rect::new(20, 5, 40, 10));
            a
        }];
        let mut g = DismissGuard::new();
        let hit = evaluate_escape_stack(
            &mut stack,
            &mut g,
            DismissEventId(1),
            DismissPhase::CaptureTopFirst,
        );
        assert_eq!(
            hit,
            Some((1, DismissDecision::Consumed)),
            "alert traps; menu stays"
        );
    }

    #[test]
    fn parent_closed_cascades_even_when_critical() {
        let mut child = DismissableLayer::new(DismissPolicy::critical());
        child.set_rect(Rect::new(0, 0, 10, 5));
        let mut g = DismissGuard::new();
        assert!(matches!(
            child.on_parent_closed(&mut g, DismissEventId(1)),
            DismissDecision::Dismiss {
                reason: DismissReason::ParentClosed
            }
        ));
    }

    #[test]
    fn light_escape_bubbles() {
        let mut tip = DismissableLayer::new(DismissPolicy::light());
        tip.set_rect(Rect::new(0, 0, 10, 1));
        let mut g = DismissGuard::new();
        assert_eq!(
            tip.on_escape(&mut g, DismissEventId(1)),
            DismissDecision::Bubble
        );
    }

    #[test]
    fn layer_policy_maps_to_actions() {
        assert_eq!(
            DismissAction::from_layer(LayerDismissPolicy::Trap),
            DismissAction::Trap
        );
        let p = DismissPolicy::from_layer_pair(
            LayerDismissPolicy::Dismissible,
            LayerDismissPolicy::Trap,
        );
        assert_eq!(p.escape, DismissAction::Dismiss);
        assert_eq!(p.outside, DismissAction::Trap);
    }

    #[test]
    fn explicit_always_dismisses_on_default_policy() {
        let mut l = menu_layer(Rect::new(0, 0, 5, 5));
        let mut g = DismissGuard::new();
        assert!(l.on_explicit(&mut g, DismissEventId(9)).should_dismiss());
    }
}
