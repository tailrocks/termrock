// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Z-ordered overlay host (private legacy). Prefer [`super::InteractionScene`].
#![allow(dead_code)]
//!
//! TermRock owns stack order and identity. Callers own paint payloads and
//! decide when to push/pop layers. This is intentionally paint-agnostic so
//! any widget family can participate.

use std::fmt;

/// Stable identity for one overlay layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayId(pub String);

impl OverlayId {
    /// Borrows a static identity into an owned overlay id.
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self(id.to_owned())
    }
}

impl fmt::Display for OverlayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kind of overlay chrome (for Esc and focus policy hints).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OverlayKind {
    /// Command palette / completion / picker popup.
    Menu,
    /// Jump-mode letter badges.
    Jump,
    /// Transient toast or banner (usually non-modal).
    Toast,
    /// Blocking card or dialog-like floating surface.
    Card,
    /// Caller-defined layer.
    Custom,
}

/// One registered overlay layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayer {
    /// Stable identity.
    pub id: OverlayId,
    /// Semantic kind.
    pub kind: OverlayKind,
    /// Whether Esc should dismiss this layer before lower layers.
    pub dismiss_on_esc: bool,
    /// Whether pointer events outside should dismiss (caller enforces geometry).
    pub dismiss_on_outside: bool,
}

/// Stack of overlays. The last entry is topmost (highest z).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverlayHost {
    layers: Vec<OverlayLayer>,
}

impl OverlayHost {
    /// Creates an empty host.
    #[must_use]
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Returns layers from bottom to top.
    #[must_use]
    pub fn layers(&self) -> &[OverlayLayer] {
        &self.layers
    }

    /// Returns the topmost layer.
    #[must_use]
    pub fn top(&self) -> Option<&OverlayLayer> {
        self.layers.last()
    }

    /// Pushes a layer (or replaces an existing id, moving it to the top).
    pub fn push(&mut self, layer: OverlayLayer) {
        self.layers.retain(|item| item.id != layer.id);
        self.layers.push(layer);
    }

    /// Removes a layer by id.
    pub fn remove(&mut self, id: &OverlayId) -> bool {
        let before = self.layers.len();
        self.layers.retain(|item| &item.id != id);
        self.layers.len() != before
    }

    /// Pops the topmost layer when present.
    pub fn pop(&mut self) -> Option<OverlayLayer> {
        self.layers.pop()
    }

    /// Dismisses the topmost layer only when it is Esc-dismissible.
    ///
    /// A non-dismissible top layer protects every lower layer: Esc must not
    /// peel a menu under a trapping dialog.
    pub fn dismiss_top_esc(&mut self) -> Option<OverlayLayer> {
        let top = self.layers.last()?;
        if !top.dismiss_on_esc {
            return None;
        }
        self.layers.pop()
    }

    /// Returns whether any layer is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Clears every layer.
    pub fn clear(&mut self) {
        self.layers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_replace_moves_to_top() {
        let mut host = OverlayHost::new();
        host.push(OverlayLayer {
            id: OverlayId::from_static("a"),
            kind: OverlayKind::Menu,
            dismiss_on_esc: true,
            dismiss_on_outside: true,
        });
        host.push(OverlayLayer {
            id: OverlayId::from_static("b"),
            kind: OverlayKind::Jump,
            dismiss_on_esc: true,
            dismiss_on_outside: false,
        });
        host.push(OverlayLayer {
            id: OverlayId::from_static("a"),
            kind: OverlayKind::Card,
            dismiss_on_esc: true,
            dismiss_on_outside: true,
        });
        assert_eq!(host.layers().len(), 2);
        assert_eq!(host.top().map(|l| l.id.0.as_str()), Some("a"));
        assert_eq!(host.top().map(|l| l.kind), Some(OverlayKind::Card));
    }

    #[test]
    fn dismiss_top_esc_only_when_top_is_dismissible() {
        let mut host = OverlayHost::new();
        host.push(OverlayLayer {
            id: OverlayId::from_static("menu"),
            kind: OverlayKind::Menu,
            dismiss_on_esc: true,
            dismiss_on_outside: true,
        });
        host.push(OverlayLayer {
            id: OverlayId::from_static("dialog"),
            kind: OverlayKind::Card,
            dismiss_on_esc: false,
            dismiss_on_outside: false,
        });
        // Non-dismissible top must not peel the menu beneath.
        assert!(host.dismiss_top_esc().is_none());
        assert_eq!(host.layers().len(), 2);
        host.pop();
        let dismissed = host.dismiss_top_esc().expect("menu");
        assert_eq!(dismissed.id.0, "menu");
    }
}
