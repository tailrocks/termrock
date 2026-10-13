// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ordered Escape semantics (private legacy). Prefer [`super::InteractionScene`].
#![allow(dead_code)]
//!
//! Consumers register the active layers each frame (or after state changes).
//! Esc peels the topmost layer; when the stack is empty, Esc is ignored so
//! quit policy stays application-owned.

/// One cancel/dismiss layer in Escape priority order (top = last pushed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EscLayer {
    /// Completion menu, jump overlay, context menu, or similar transient chrome.
    Overlay,
    /// Modal dialog or blocking card.
    Modal,
    /// Non-empty draft text in a prompt or editor.
    Draft,
    /// In-flight work that can be cancelled (agent turn, background task).
    Work,
    /// Application quit (only if the consumer chooses to register it).
    Quit,
}

/// Result of handling Escape against the cascade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EscOutcome {
    /// No layer was registered; caller may apply a default (often quit).
    Empty,
    /// The topmost layer was peeled and should be acted on by the caller.
    Peeled(EscLayer),
}

/// Stack of Escape targets. Top of stack is the first Esc destination.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EscCascade {
    layers: Vec<EscLayer>,
}

impl EscCascade {
    /// Creates an empty cascade.
    #[must_use]
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Rebuilds the cascade from an ordered bottom→top slice.
    pub fn set_layers(&mut self, layers: impl IntoIterator<Item = EscLayer>) {
        self.layers.clear();
        self.layers.extend(layers);
    }

    /// Pushes a layer if it is not already the top layer (idempotent top).
    pub fn push(&mut self, layer: EscLayer) {
        if self.layers.last() != Some(&layer) {
            self.layers.push(layer);
        }
    }

    /// Removes the top layer when it matches `layer`.
    pub fn pop_if(&mut self, layer: EscLayer) -> bool {
        if self.layers.last() == Some(&layer) {
            self.layers.pop();
            true
        } else {
            false
        }
    }

    /// Clears every registered layer.
    pub fn clear(&mut self) {
        self.layers.clear();
    }

    /// Returns the ordered layers from bottom to top.
    #[must_use]
    pub fn layers(&self) -> &[EscLayer] {
        &self.layers
    }

    /// Peels the topmost layer, if any.
    pub fn peel(&mut self) -> EscOutcome {
        match self.layers.pop() {
            Some(layer) => EscOutcome::Peeled(layer),
            None => EscOutcome::Empty,
        }
    }

    /// Peeks at the topmost layer without removing it.
    #[must_use]
    pub fn top(&self) -> Option<EscLayer> {
        self.layers.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peels_top_first_in_agent_order() {
        let mut cascade = EscCascade::new();
        cascade.set_layers([
            EscLayer::Quit,
            EscLayer::Work,
            EscLayer::Draft,
            EscLayer::Overlay,
        ]);
        assert_eq!(cascade.peel(), EscOutcome::Peeled(EscLayer::Overlay));
        assert_eq!(cascade.peel(), EscOutcome::Peeled(EscLayer::Draft));
        assert_eq!(cascade.peel(), EscOutcome::Peeled(EscLayer::Work));
        assert_eq!(cascade.peel(), EscOutcome::Peeled(EscLayer::Quit));
        assert_eq!(cascade.peel(), EscOutcome::Empty);
    }

    #[test]
    fn push_is_idempotent_at_top() {
        let mut cascade = EscCascade::new();
        cascade.push(EscLayer::Overlay);
        cascade.push(EscLayer::Overlay);
        assert_eq!(cascade.layers().len(), 1);
        cascade.push(EscLayer::Draft);
        assert_eq!(cascade.layers(), &[EscLayer::Overlay, EscLayer::Draft]);
    }
}
