// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! List multi-select membership — thin facade over [`crate::interaction::SelectionModel`].
//!
//! Prefer `interaction::SelectionModel` for new code (single/multi/range + deltas).
use crate::interaction::{SelectionDelta, SelectionKind, SelectionModel as Model};

/// An ordered set of checked stable identities (list multi-select).
///
/// Implemented as [`SelectionKind::Multiple`] [`crate::interaction::SelectionModel`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection<Id> {
    inner: Model<Id>,
}

impl<Id> Default for Selection<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> Selection<Id> {
    /// Creates an empty ordered multi-selection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            inner: Model::multiple(),
        }
    }

    /// Clears the checked-identity set.
    pub fn clear(&mut self) {
        let _ = self.inner.clear();
    }

    /// Borrow the full selection model.
    #[must_use]
    pub const fn model(&self) -> &Model<Id> {
        &self.inner
    }

    /// Mutable selection model.
    pub const fn model_mut(&mut self) -> &mut Model<Id> {
        &mut self.inner
    }
}

impl<Id: Clone + PartialEq> Selection<Id> {
    /// Returns checked identities in their check order.
    #[must_use]
    pub fn checked(&self) -> &[Id] {
        self.inner.selected()
    }

    /// Toggle a stable identity, preserving check order.
    ///
    /// Returns whether the identity is checked after the toggle.
    pub fn toggle(&mut self, id: &Id) -> bool {
        match self.inner.toggle(id) {
            SelectionDelta::Toggled { selected, .. } => selected,
            _ => self.inner.is_checked(id),
        }
    }

    /// Returns whether the stable identity is currently checked.
    #[must_use]
    pub fn is_checked(&self, id: &Id) -> bool {
        self.inner.is_checked(id)
    }

    /// Select-all visible ids (does not drop off-window checks).
    pub fn select_all(&mut self, visible: &[Id]) {
        let _ = self.inner.select_all(visible);
    }

    /// Invert membership for visible ids.
    pub fn invert_visible(&mut self, visible: &[Id]) {
        let _ = self.inner.invert_visible(visible);
    }

    /// Drop checks not in `still_valid` (deleted rows).
    pub fn reconcile(&mut self, still_valid: &[Id]) {
        let _ = self.inner.reconcile(still_valid);
    }
}

impl<Id: Clone + PartialEq> From<Model<Id>> for Selection<Id> {
    fn from(mut inner: Model<Id>) -> Self {
        if !matches!(inner.kind(), SelectionKind::Multiple | SelectionKind::Range) {
            inner.set_kind(SelectionKind::Multiple);
        }
        Self { inner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_preserves_check_order_and_clear_resets() {
        let mut selection = Selection::new();

        assert!(selection.toggle(&"beta"));
        assert!(selection.toggle(&"alpha"));
        assert_eq!(selection.checked(), ["beta", "alpha"]);
        assert!(!selection.toggle(&"beta"));
        assert_eq!(selection.checked(), ["alpha"]);
        assert!(selection.is_checked(&"alpha"));

        selection.clear();
        assert!(selection.checked().is_empty());
    }

    #[test]
    fn select_all_and_reconcile() {
        let mut selection = Selection::new();
        selection.select_all(&["a", "b"]);
        assert_eq!(selection.checked().len(), 2);
        selection.reconcile(&["b"]);
        assert_eq!(selection.checked(), ["b"]);
    }
}
