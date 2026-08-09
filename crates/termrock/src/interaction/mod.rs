//! Stable-ID focus, hover, hit regions, and logical outcomes.

mod collection;
mod event_result;
mod focus;
mod focus_graph;
mod intent;
mod keymap_bridge;
mod modal;
mod overlay_stack;
mod roving;
mod scene;

pub use collection::{CollectionItem, CollectionOutcome, CollectionState};
pub use event_result::{
    EventResult, FocusRequest, OverlayRequest, Propagation, Redraw, compose_bubble, compose_capture,
    from_consumed_flag,
};
pub use focus_graph::{
    FocusDebugSnapshot, FocusGraph, FocusLens, FocusNavMode, FocusNode, FocusOutcome,
};
pub use roving::{
    RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome, roving_hint_keymap,
    roving_hint_keymap_horizontal, roving_hint_keymap_vertical,
};

// FocusRing remains crate-private (pre-1.0 M3). Public focus graph is FocusGraph.
// Overlay authority is OverlayStack only (Break D / M4). ModalStack is crate-private.
pub(crate) use focus::{FocusRing, FocusTarget};
// Legacy name: FocusRing used FocusOutcome from focus.rs — re-export graph's outcome privately for ring tests via focus module.
pub use intent::{
    NavigationMove, PageMove, UiIntent, default_button_intent, default_choice_dialog_intent,
    default_data_table_intent, default_diff_review_intent, default_form_intent,
    default_inspector_intent, default_list_intent, default_log_stream_intent, default_menu_intent,
    default_permission_intent, default_prompt_composer_intent, default_table_intent,
    default_text_area_intent, default_transcript_intent, default_tree_intent,
};
pub use keymap_bridge::dispatch_keymap_action;
pub(crate) use modal::ModalStack;
/// Paint a dim/occlude wash when [`OverlayStack::backdrop_policy`] requests it.
pub use modal::render_backdrop;
pub use overlay_stack::{
    BackdropPolicy, NarrowFallback, OverlayEntry, OverlayId, OverlayKind, OverlayOutcome,
    OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PlacementPrefer, place_overlay,
};
pub use scene::{
    InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene, LayerDismissPolicy,
    LayerKind, SceneError, SemanticDiagnostic, SemanticElement, SemanticError, SemanticNode,
    SemanticRole, SemanticScene, SemanticSnapshot, SemanticSnapshotNode, SemanticState,
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
///
/// Prefer wrapping non-ignored values in [`EventResult`] at host boundaries:
/// `outcome.into_event_result()`.
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

impl<T> Outcome<T> {
    /// Converts to the standard [`EventResult`] envelope.
    ///
    /// `Ignored` → bubble / no paint. All other variants → stop + redraw + message.
    #[must_use]
    pub fn into_event_result(self) -> EventResult<Self> {
        match self {
            Self::Ignored => EventResult::ignored(),
            other => EventResult::emit(other),
        }
    }

    /// Whether this outcome means the input was not handled.
    #[must_use]
    pub const fn is_ignored(&self) -> bool {
        matches!(self, Self::Ignored)
    }
}
