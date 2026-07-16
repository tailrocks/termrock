//! Stable-ID focus, hover, hit regions, and logical outcomes.

mod focus_owner;
mod hover_tracker;
mod modal;

pub use focus_owner::{ButtonFocus, FocusOwner};
pub use hover_tracker::HoverTracker;
pub use modal::{ModalClickResult, ModalStack, classify_click, render_backdrop};

use ratatui_core::layout::{Position, Rect};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Runtime state for `Focus`.
pub struct FocusState<Id> {
    focused: Option<Id>,
}

impl<Id> FocusState<Id> {
    #[must_use]
    /// Creates a new value with canonical defaults.
    pub const fn new(focused: Option<Id>) -> Self {
        Self { focused }
    }

    #[must_use]
    /// Performs the `focused` operation.
    pub const fn focused(&self) -> Option<&Id> {
        self.focused.as_ref()
    }

    /// Performs the `set` operation.
    pub fn set(&mut self, focused: Option<Id>) {
        self.focused = focused;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data carried by `HitRegion`.
pub struct HitRegion<Id> {
    /// Documentation for `item`.
    pub id: Id,
    /// Documentation for `item`.
    pub area: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
/// Runtime state for `Hover`.
pub struct HoverState<Id> {
    hovered: Option<Id>,
}

impl<Id: Clone> HoverState<Id> {
    /// Performs the `update` operation.
    pub fn update(&mut self, position: Position, regions: &[HitRegion<Id>]) -> Option<&Id> {
        self.hovered = regions
            .iter()
            .find(|region| region.area.contains(position))
            .map(|region| region.id.clone());
        self.hovered.as_ref()
    }

    #[must_use]
    /// Performs the `hovered` operation.
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
    /// The identified item was activated.
    Activated(T),
    /// The interaction was cancelled.
    Cancelled,
}
