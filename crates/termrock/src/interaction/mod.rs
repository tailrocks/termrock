//! Stable-ID focus, hover, hit regions, and logical outcomes.

mod focus;
mod intent;
mod keymap_bridge;
mod modal;
mod overlay_stack;
mod scene;

// FocusRing is crate-private (pre-1.0 M3 / Break C0). Hosts use InteractionScene.
// Overlay authority is OverlayStack only (Break D / M4). ModalStack is crate-private.
pub(crate) use focus::{FocusOutcome, FocusRing, FocusTarget};
pub use intent::{
    NavigationMove, PageMove, UiIntent, default_button_intent, default_choice_dialog_intent,
    default_data_table_intent, default_diff_review_intent, default_form_intent,
    default_inspector_intent, default_list_intent, default_log_stream_intent, default_menu_intent,
    default_permission_intent, default_prompt_composer_intent, default_table_intent,
    default_transcript_intent, default_tree_intent,
};
pub use keymap_bridge::dispatch_keymap_action;
pub(crate) use modal::ModalStack;
/// Paint a dim/occlude wash when [`OverlayStack::backdrop_policy`] requests it.
pub use modal::render_backdrop;
pub use overlay_stack::{
    BackdropPolicy, NarrowFallback, OverlayEntry, OverlayId, OverlayKind, OverlayOutcome,
    OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PlacementPrefer, place_overlay,
};
pub(crate) use scene::SemanticScene;
pub use scene::{
    InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene, LayerDismissPolicy,
    LayerKind, SceneError, SemanticElement, SemanticRole,
};

use ratatui_core::layout::{Position, Rect};

#[derive(Debug, Clone, PartialEq, Eq)]
/// A stable identity paired with its painted terminal rectangle.
pub struct HitRegion<Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Painted terminal rectangle used for hit testing.
    pub area: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Cached stable-ID hover state driven from painted [`HitRegion`]s.
///
/// Widgets may expose their painted regions for a consumer-owned hover state,
/// or own equivalent stateful `hover` methods when input belongs to the widget.
pub struct HoverState<Id> {
    hovered: Option<Id>,
}

#[cfg(test)]
mod hover_tests {
    use super::*;

    #[test]
    fn hover_state_caches_hit_and_clears_on_miss() {
        let regions = [HitRegion {
            id: "action",
            area: Rect::new(2, 3, 4, 2),
        }];
        let mut hover = HoverState::default();

        assert_eq!(hover.update(Position::new(3, 3), &regions), Some(&"action"));
        assert_eq!(hover.hovered(), Some(&"action"));
        assert_eq!(hover.update(Position::new(0, 0), &regions), None);
        assert_eq!(hover.hovered(), None);
    }
}

impl<Id: Clone> HoverState<Id> {
    /// Updates cached hover identity from the current pointer position and hit regions.
    pub fn update(&mut self, position: Position, regions: &[HitRegion<Id>]) -> Option<&Id> {
        self.hovered = regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Returns the stable identity currently under the pointer.
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Shared result vocabulary for widget interaction handlers.
#[non_exhaustive]
pub enum Outcome<T> {
    /// The event was not actionable.
    Ignored,
    /// State changed without activating an identity.
    Changed,
    /// A check gesture toggled the item with this stable identity.
    CheckToggled(T),
    /// The identified item was activated.
    Activated(T),
    /// The interaction was cancelled.
    Cancelled,
}
