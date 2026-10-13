//! Stable-ID focus, hover, hit regions, and logical outcomes.

mod focus_owner;
mod hover_tracker;
mod modal;

pub use focus_owner::{ButtonFocus, FocusOwner};
pub use hover_tracker::HoverTracker;
pub use modal::{ModalClickResult, ModalStack, classify_click, render_backdrop};

use ratatui_core::layout::{Position, Rect};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusState<Id> {
    focused: Option<Id>,
}

impl<Id> FocusState<Id> {
    #[must_use]
    pub const fn new(focused: Option<Id>) -> Self {
        Self { focused }
    }

    #[must_use]
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    pub fn set(&mut self, focused: Option<Id>) {
        self.focused = focused;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitRegion<Id> {
    pub id: Id,
    pub area: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HoverState<Id> {
    hovered: Option<Id>,
}

impl<Id: Clone> HoverState<Id> {
    pub fn update(&mut self, position: Position, regions: &[HitRegion<Id>]) -> Option<&Id> {
        self.hovered = regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    pub const fn hovered(&self) -> Option<&Id> {
        self.hovered.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Shared result vocabulary for widget interaction handlers.
pub enum Outcome<T> {
    /// The event was not actionable.
    Ignored,
    /// State changed without activating an identity.
    Changed,
    /// The identified item was activated.
    Activated(T),
    /// The interaction was cancelled.
    Cancelled,
}
