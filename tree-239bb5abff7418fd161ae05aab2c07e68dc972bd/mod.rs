//! Stable-ID focus, hover, hit regions, and logical outcomes.

mod focus_owner;
mod modal;

pub use focus_owner::{ButtonFocus, FocusOwner};
pub use modal::{ModalClickResult, ModalStack, classify_click, render_backdrop};

use ratatui_core::layout::{Position, Rect};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Runtime state for `Focus`.
pub struct FocusState<Id> {
    focused: Option<Id>,
}

impl<Id> FocusState<Id> {
    #[must_use]
    /// Creates an unfocused state cell.
    pub const fn new(focused: Option<Id>) -> Self {
        Self { focused }
    }

    #[must_use]
    /// Returns whether this focus state currently owns focus.
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// Updates whether this focus state owns focus.
    pub fn set(&mut self, focused: Option<Id>) {
        self.focused = focused;
    }
}

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
