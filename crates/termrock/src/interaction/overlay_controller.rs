// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Coordinates overlay stack + Esc peel + focus scope restore.

use super::{
    esc_cascade::{EscCascade, EscLayer, EscOutcome},
    focus::{FocusOutcome, FocusRing},
    overlay::{OverlayHost, OverlayId, OverlayLayer},
};

/// Result of handling Escape against overlays and the cascade.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OverlayEscResult<Id> {
    /// Nothing to peel.
    Empty,
    /// Top Esc-dismissible overlay dismissed; focus may have restored.
    OverlayDismissed {
        /// Removed overlay id.
        id: OverlayId,
        /// Focus outcome after optional scope pop.
        focus: FocusOutcome<Id>,
    },
    /// Cascade peeled a non-overlay layer (draft/work/quit).
    Cascade(EscLayer),
}

/// Unified overlay + Esc controller for one application shell.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverlayController {
    /// Z-order overlay host.
    pub host: OverlayHost,
    /// Escape priority cascade (optional layers below overlays).
    pub cascade: EscCascade,
}

impl OverlayController {
    /// Creates an empty controller.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes an overlay and ensures `EscLayer::Overlay` is on the cascade.
    pub fn push_overlay(&mut self, layer: OverlayLayer) {
        self.host.push(layer);
        if self.host.layers().iter().any(|l| l.dismiss_on_esc) {
            self.cascade.push(EscLayer::Overlay);
        }
    }

    /// Peels exactly one conceptual Esc target:
    /// 1. Top Esc-dismissible overlay (and matching focus scope if provided).
    /// 2. Else cascade peel (draft/work/quit).
    pub fn handle_esc<Id: Clone + Eq, ScopeId: Clone + Eq>(
        &mut self,
        focus: Option<&mut FocusRing<Id, ScopeId>>,
        pop_focus_scope_on_overlay: bool,
    ) -> OverlayEscResult<Id> {
        if let Some(layer) = self.host.dismiss_top_esc() {
            // Drop Overlay markers from the cascade when no Esc-dismissible
            // overlay remains (they may sit under Draft/Work layers).
            if !self.host.layers().iter().any(|l| l.dismiss_on_esc) {
                let rest: Vec<_> = self
                    .cascade
                    .layers()
                    .iter()
                    .copied()
                    .filter(|layer| *layer != EscLayer::Overlay)
                    .collect();
                self.cascade.set_layers(rest);
            }
            let focus_outcome = if let Some(ring) = focus {
                if pop_focus_scope_on_overlay && ring.scope_depth() > 1 {
                    ring.pop_scope()
                } else {
                    FocusOutcome::Unchanged
                }
            } else {
                FocusOutcome::Unchanged
            };
            return OverlayEscResult::OverlayDismissed {
                id: layer.id,
                focus: focus_outcome,
            };
        }
        match self.cascade.peel() {
            EscOutcome::Empty => OverlayEscResult::Empty,
            EscOutcome::Peeled(layer) => OverlayEscResult::Cascade(layer),
        }
    }
}

// FocusRing needs scope_depth + pop_scope public — check if they exist
// We'll add thin wrappers if needed.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::overlay::OverlayKind;
    use crate::interaction::{FocusRing, FocusTarget};
    use ratatui_core::layout::Rect;

    #[test]
    fn esc_peels_one_overlay_then_cascade() {
        let mut ctl = OverlayController::new();
        ctl.push_overlay(OverlayLayer {
            id: OverlayId::from_static("menu"),
            kind: OverlayKind::Menu,
            dismiss_on_esc: true,
            dismiss_on_outside: true,
        });
        ctl.cascade.push(EscLayer::Draft);
        let first = ctl.handle_esc::<&str, &str>(None, false);
        assert!(matches!(
            first,
            OverlayEscResult::OverlayDismissed { id, .. } if id.0 == "menu"
        ));
        assert!(ctl.host.is_empty());
        let second = ctl.handle_esc::<&str, &str>(None, false);
        assert_eq!(second, OverlayEscResult::Cascade(EscLayer::Draft));
        let third = ctl.handle_esc::<&str, &str>(None, false);
        assert_eq!(third, OverlayEscResult::Empty);
    }

    #[test]
    fn esc_restores_focus_when_scope_popped() {
        let mut ring = FocusRing::new("root", Some("main"));
        ring.begin_frame();
        ring.register(FocusTarget {
            id: "main",
            scope: "root",
            area: Some(Rect::new(0, 0, 10, 1)),
            enabled: true,
        });
        let _ = ring.reconcile();
        ring.push_scope("modal");
        ring.begin_frame();
        ring.register(FocusTarget {
            id: "main",
            scope: "root",
            area: Some(Rect::new(0, 0, 10, 1)),
            enabled: true,
        });
        ring.register(FocusTarget {
            id: "ok",
            scope: "modal",
            area: Some(Rect::new(0, 2, 10, 1)),
            enabled: true,
        });
        let _ = ring.reconcile();
        assert_eq!(ring.focused(), Some(&"ok"));

        let mut ctl = OverlayController::new();
        ctl.push_overlay(OverlayLayer {
            id: OverlayId::from_static("dialog"),
            kind: OverlayKind::Card,
            dismiss_on_esc: true,
            dismiss_on_outside: true,
        });
        let result = ctl.handle_esc(Some(&mut ring), true);
        assert!(matches!(result, OverlayEscResult::OverlayDismissed { .. }));
        assert_eq!(ring.focused(), Some(&"main"));
    }
}
