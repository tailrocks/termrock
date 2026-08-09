// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Per-frame semantic UI scene registration (immediate mode, not a retained DOM).

use ratatui_core::layout::{Position, Rect};

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

/// One element registered for the current frame.
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

/// Per-frame registry rebuilt each draw (or each interaction sample).
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

    #[test]
    fn hit_test_prefers_later_overlapping_registration() {
        let mut scene = SemanticScene::new();
        scene.register(SemanticElement {
            id: "a",
            area: Rect::new(0, 0, 10, 5),
            focusable: true,
            enabled: true,
            role: SemanticRole::Content,
        });
        scene.register(SemanticElement {
            id: "b",
            area: Rect::new(2, 1, 4, 2),
            focusable: true,
            enabled: true,
            role: SemanticRole::Control,
        });
        let hit = scene.hit_test(Position::new(3, 2)).expect("hit");
        assert_eq!(hit.id, "b");
    }

    #[test]
    fn focus_order_skips_disabled() {
        let mut scene = SemanticScene::new();
        scene.register(SemanticElement {
            id: "one",
            area: Rect::new(0, 0, 1, 1),
            focusable: true,
            enabled: true,
            role: SemanticRole::Control,
        });
        scene.register(SemanticElement {
            id: "two",
            area: Rect::new(1, 0, 1, 1),
            focusable: true,
            enabled: false,
            role: SemanticRole::Control,
        });
        scene.register(SemanticElement {
            id: "three",
            area: Rect::new(2, 0, 1, 1),
            focusable: false,
            enabled: true,
            role: SemanticRole::Content,
        });
        assert_eq!(scene.focus_order(), vec![&"one"]);
    }

    #[test]
    fn begin_frame_clears() {
        let mut scene = SemanticScene::new();
        scene.register(SemanticElement {
            id: 1u8,
            area: Rect::new(0, 0, 1, 1),
            focusable: true,
            enabled: true,
            role: SemanticRole::Chrome,
        });
        scene.begin_frame();
        assert!(scene.elements().is_empty());
    }
}
