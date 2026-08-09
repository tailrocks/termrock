// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Letter-badge jump navigation over registered focus rectangles.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::Widget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::{HitRegion, OverlayId, OverlayOutcome, OverlaySpec, OverlayStack},
    style::{DesignSystem, Role, RolePalette},
};

/// Default overlay id for jump mode (fullscreen-class, owns input).
pub const JUMP_OVERLAY_ID: &str = "termrock.jump";

/// Opens jump mode as a fullscreen overlay layer (owns input; Esc dismissible).
pub fn open_jump_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::fullscreen(JUMP_OVERLAY_ID, opener_focus).with_policy(
            crate::interaction::OverlayPolicy {
                // Jump mode: Esc dismisses jump only (one layer).
                esc: crate::interaction::LayerDismissPolicy::Dismissible,
                outside: crate::interaction::LayerDismissPolicy::Dismissible,
                owns_input: true,
                focus_trap: true,
                wheel_captures: true,
                backdrop: crate::interaction::BackdropPolicy::None,
                prefer: crate::interaction::PlacementPrefer::Fullscreen,
                cover_anchor: true,
                narrow_fallback: crate::interaction::NarrowFallback::Fullscreen,
                narrow_cols: 0,
            },
        ),
    )
}

/// Dismisses the default jump overlay when present.
pub fn dismiss_jump_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(JUMP_OVERLAY_ID))
}

/// One jump target with a letter badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpTarget<Id> {
    /// Stable identity activated when the badge key is pressed.
    pub id: Id,
    /// Painted geometry for the badge anchor (top-left of the region).
    pub area: Rect,
    /// Single-character badge (caller-assigned).
    pub badge: char,
}

/// Outcome of jump-mode input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JumpOutcome<Id> {
    /// Event not applicable.
    Ignored,
    /// Jump mode dismissed without activation.
    Dismissed,
    /// Target activated by badge key or click.
    Activated(Id),
}

/// Jump-mode state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpOverlayState {
    open: bool,
}

impl JumpOverlayState {
    /// Creates closed jump state.
    #[must_use]
    pub const fn new() -> Self {
        Self { open: false }
    }

    /// Opens jump mode (local flag; prefer [`open_jump_overlay`] with a stack).
    pub const fn open(&mut self) {
        self.open = true;
    }

    /// Closes jump mode.
    pub const fn close(&mut self) {
        self.open = false;
    }

    /// Opens jump mode and registers a fullscreen-class layer on the overlay stack.
    ///
    /// Esc is dismissible on this layer (one conceptual peel). Pair paint of
    /// [`JumpOverlay`] with `stack.top()` rect when open.
    pub fn open_on_stack<FocusId: Clone>(
        &mut self,
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        opener_focus: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        self.open = true;
        open_jump_overlay(stack, bounds, opener_focus)
    }

    /// Closes jump mode and dismisses the stack entry when present.
    pub fn close_on_stack<FocusId: Clone>(
        &mut self,
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        self.open = false;
        dismiss_jump_overlay(stack)
    }

    /// Whether jump mode is active.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Handles a key while jump mode is open.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open || key.kind != KeyEventKind::Press {
            return JumpOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                self.open = false;
                JumpOutcome::Dismissed
            }
            KeyCode::Char(ch) => {
                let needle = ch.to_ascii_lowercase();
                if let Some(target) = targets
                    .iter()
                    .find(|t| t.badge.to_ascii_lowercase() == needle)
                {
                    self.open = false;
                    JumpOutcome::Activated(target.id.clone())
                } else {
                    JumpOutcome::Ignored
                }
            }
            _ => JumpOutcome::Ignored,
        }
    }

    /// Handles a click against target regions.
    pub fn handle_click<Id: Clone>(
        &mut self,
        position: Position,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open {
            return JumpOutcome::Ignored;
        }
        if let Some(target) = targets.iter().find(|t| t.area.contains(position)) {
            self.open = false;
            JumpOutcome::Activated(target.id.clone())
        } else {
            JumpOutcome::Ignored
        }
    }
}

/// Assigns sequential a–z badges to hit regions (skips non-letter when exhausted).
#[must_use]
pub fn assign_jump_badges<Id: Clone>(regions: &[HitRegion<Id>]) -> Vec<JumpTarget<Id>> {
    const BADGES: &[char] = &[
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    ];
    regions
        .iter()
        .zip(BADGES.iter().copied())
        .map(|(region, badge)| JumpTarget {
            id: region.id.clone(),
            area: region.area,
            badge,
        })
        .collect()
}

/// Builds jump targets from a frame-local [`crate::interaction::SemanticScene`].
///
/// Only focusable, enabled, visible nodes with non-empty geometry participate.
#[must_use]
pub fn assign_jump_badges_from_semantics<Id, Action>(
    scene: &crate::interaction::SemanticScene<Id, Action>,
) -> Vec<JumpTarget<Id>>
where
    Id: Clone,
{
    assign_jump_badges(&scene.jump_regions())
}

/// Renders letter badges for open jump mode.
#[derive(Debug, Clone, Copy)]
pub struct JumpOverlay<'a, Id> {
    targets: &'a [JumpTarget<Id>],
    system: &'a DesignSystem,
}

impl<'a, Id> JumpOverlay<'a, Id> {
    /// Creates a jump overlay over borrowed targets.
    #[must_use]
    pub const fn new(targets: &'a [JumpTarget<Id>], system: &'a DesignSystem) -> Self {
        Self { targets, system }
    }
}

impl<Id> Widget for &JumpOverlay<'_, Id> {
    fn render(self, _area: Rect, buffer: &mut Buffer) {
        let style = self
            .system
            .style(Role::ActionFocused)
            .add_modifier(ratatui_core::style::Modifier::BOLD);
        for target in self.targets {
            if target.area.width == 0 || target.area.height == 0 {
                continue;
            }
            let label = format!("[{}]", target.badge);
            let max = usize::from(target.area.width);
            buffer.set_stringn(target.area.x, target.area.y, &label, max, style);
        }
    }
}

impl<Id> Widget for JumpOverlay<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn badge_key_activates_and_closes() {
        let mut state = JumpOverlayState::new();
        state.open();
        let targets = [JumpTarget {
            id: "files",
            area: Rect::new(0, 0, 10, 3),
            badge: 'f',
        }];
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        assert_eq!(
            state.handle_key(key, &targets),
            JumpOutcome::Activated("files")
        );
        assert!(!state.is_open());
    }

    #[test]
    fn assign_badges_is_stable_order() {
        let regions = [
            HitRegion {
                id: "a",
                area: Rect::new(0, 0, 2, 1),
            },
            HitRegion {
                id: "b",
                area: Rect::new(3, 0, 2, 1),
            },
        ];
        let badges = assign_jump_badges(&regions);
        assert_eq!(badges[0].badge, 'a');
        assert_eq!(badges[1].badge, 'b');
    }

    #[test]
    fn jump_opens_fullscreen_layer_and_esc_restores_opener() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let mut state = JumpOverlayState::new();
        let out = state.open_on_stack(&mut stack, bounds, Some("main.list"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert!(state.is_open());
        assert!(stack.top_owns_input());
        assert_eq!(stack.top().unwrap().rect, bounds);
        assert_eq!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                id: OverlayId::from_static(JUMP_OVERLAY_ID),
                focus: Some("main.list"),
            }
        );
        // App mirrors stack dismiss into local jump state.
        state.close();
        assert!(!state.is_open());
        assert!(stack.is_empty());
    }

    #[test]
    fn jump_targets_from_semantic_scene() {
        use crate::interaction::{SemanticNode, SemanticRole, SemanticScene};
        let mut scene = SemanticScene::<&str>::new();
        scene
            .register(
                SemanticNode::control("a", Rect::new(0, 0, 2, 1)).role(SemanticRole::Button),
            )
            .unwrap();
        scene
            .register(
                SemanticNode::control("b", Rect::new(3, 0, 2, 1))
                    .role(SemanticRole::Button)
                    .disabled(true),
            )
            .unwrap();
        let targets = assign_jump_badges_from_semantics(&scene);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id, "a");
        assert_eq!(targets[0].badge, 'a');
    }
}
