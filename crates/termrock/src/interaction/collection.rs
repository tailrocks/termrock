// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Headless collection model for lists, menus, pickers, tables, and trees.
//!
//! Long-lived state holds only **owned ids** and virtualization numbers — never
//! borrowed display text. Frame projections pass [`CollectionItem`] (with
//! ephemeral labels for typeahead) into reconcile / move / key handlers.
//!
//! Hierarchy is optional (`parent` on items). Flat collections leave it `None`.
//! Active-descendant movement reuses [`super::RovingFocusGroup`].
use crate::{
    input::KeyEvent,
    interaction::{
        NavigationMove, RovingEntry, RovingFocusGroup, RovingOrientation, RovingOutcome, UiIntent,
    },
};

/// One item in a frame projection (not stored long-term in [`CollectionState`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionItem<Id> {
    /// Stable identity across frames.
    pub id: Id,
    /// Whether the item participates in movement / activation.
    pub enabled: bool,
    /// Typeahead / a11y label for this call only (empty disables typeahead match).
    pub label: String,
    /// Optional parent id (trees); ignored by flat movement.
    pub parent: Option<Id>,
}

impl<Id> CollectionItem<Id> {
    /// Enabled flat item with label.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            enabled: true,
            label: label.into(),
            parent: None,
        }
    }

    /// Disabled flag.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Optional parent for hierarchical projections.
    #[must_use]
    pub fn parent(mut self, parent: Id) -> Self {
        self.parent = Some(parent);
        self
    }
}

/// Outcome of collection navigation / scroll (not domain Activate).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CollectionOutcome<Id> {
    /// No change.
    Ignored,
    /// Active descendant changed.
    ActiveChanged {
        /// Previous active.
        from: Option<Id>,
        /// New active.
        to: Option<Id>,
    },
    /// Viewport offset changed (active may be unchanged).
    Scrolled,
}

impl<Id: PartialEq> CollectionOutcome<Id> {
    /// Whether active id or scroll changed.
    #[must_use]
    pub fn changed(&self) -> bool {
        !matches!(self, Self::Ignored)
    }

    /// Whether the active id changed.
    #[must_use]
    pub fn active_changed(&self) -> bool {
        matches!(self, Self::ActiveChanged { .. })
    }
}

impl<Id> From<RovingOutcome<Id>> for CollectionOutcome<Id> {
    fn from(value: RovingOutcome<Id>) -> Self {
        match value {
            RovingOutcome::Ignored => Self::Ignored,
            RovingOutcome::ActiveChanged { from, to } => Self::ActiveChanged { from, to },
        }
    }
}

/// Headless collection cursor + virtualization metadata.
///
/// # Current vs active
///
/// For single-cursor collections, **current** == **active** (the keyboard
/// descendant). Multi-select membership stays on widget `Selection` — not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionState<Id> {
    roving: RovingFocusGroup<Id>,
    offset: usize,
    viewport_len: usize,
    total_len: usize,
    /// Absolute start of the projected window, when `items` is not the full list.
    window_start: Option<usize>,
}

impl<Id> Default for CollectionState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> CollectionState<Id> {
    /// Empty collection, vertical wrapping roving, zero viewport.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            roving: RovingFocusGroup::new(),
            offset: 0,
            viewport_len: 0,
            total_len: 0,
            window_start: None,
        }
    }

    /// Sets roving orientation.
    #[must_use]
    pub fn orientation(mut self, orientation: RovingOrientation) -> Self {
        self.roving = self.roving.orientation(orientation);
        self
    }

    /// Sets wrap policy for active movement.
    #[must_use]
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.roving = self.roving.wrap(wrap);
        self
    }

    /// Active descendant (keyboard cursor).
    #[must_use]
    pub const fn active(&self) -> Option<&Id> {
        self.roving.active()
    }

    /// Alias of [`Self::active`] for “current item” vocabulary.
    #[must_use]
    pub const fn current(&self) -> Option<&Id> {
        self.active()
    }

    /// First visible index in the host’s full ordering.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Visible row capacity.
    #[must_use]
    pub const fn viewport_len(&self) -> usize {
        self.viewport_len
    }

    /// Host-reported total item count (may exceed projected window).
    #[must_use]
    pub const fn total_len(&self) -> usize {
        self.total_len
    }

    /// Borrow the embedded roving group.
    #[must_use]
    pub const fn roving(&self) -> &RovingFocusGroup<Id> {
        &self.roving
    }

    /// Clears typeahead buffer.
    pub fn clear_typeahead(&mut self) {
        self.roving.clear_typeahead();
    }
}

impl<Id: Clone + PartialEq> CollectionState<Id> {
    /// Sets active / current id (clears typeahead).
    pub fn set_active(&mut self, id: Option<Id>) {
        self.roving.set_active(id);
    }

    /// Alias of [`Self::set_active`].
    pub fn set_current(&mut self, id: Option<Id>) {
        self.set_active(id);
    }

    /// Updates virtualization numbers (clamps offset).
    pub fn set_viewport(&mut self, offset: usize, viewport_len: usize, total_len: usize) {
        self.viewport_len = viewport_len;
        self.total_len = total_len;
        let max = total_len.saturating_sub(viewport_len.min(total_len));
        self.offset = offset.min(max);
    }

    /// Converts frame items to roving entries (owned labels for this call).
    #[must_use]
    pub fn to_roving_entries(items: &[CollectionItem<Id>]) -> Vec<RovingEntry<Id>> {
        items
            .iter()
            .map(|i| RovingEntry {
                id: i.id.clone(),
                enabled: i.enabled,
                label: i.label.clone(),
            })
            .collect()
    }

    /// Reconciles active id against a full (or filtered) projection.
    ///
    /// Sets `total_len` to `items.len()` when not using a virtual window.
    pub fn reconcile(&mut self, items: &[CollectionItem<Id>]) -> CollectionOutcome<Id> {
        self.window_start = None;
        self.total_len = items.len();
        if self.viewport_len == 0 {
            self.viewport_len = items.len();
        }
        let max = self
            .total_len
            .saturating_sub(self.viewport_len.min(self.total_len));
        self.offset = self.offset.min(max);
        let entries = Self::to_roving_entries(items);
        self.roving.reconcile(&entries).into()
    }

    /// Reconciles against a **window** of items starting at `window_start` in the full list.
    ///
    /// Host owns filtering/sorting; pass only the painted/virtual slice.
    pub fn reconcile_window(
        &mut self,
        window: &[CollectionItem<Id>],
        window_start: usize,
        total_len: usize,
        viewport_len: usize,
    ) -> CollectionOutcome<Id> {
        self.window_start = Some(window_start);
        self.total_len = total_len;
        self.viewport_len = viewport_len;
        self.offset = window_start.min(total_len.saturating_sub(viewport_len.min(total_len)));
        let entries = Self::to_roving_entries(window);
        self.roving.reconcile(&entries).into()
    }

    /// Moves active by `steps` among enabled items in the projection.
    pub fn move_by(&mut self, items: &[CollectionItem<Id>], steps: isize) -> CollectionOutcome<Id> {
        let entries = Self::to_roving_entries(items);
        let out: CollectionOutcome<Id> = self.roving.move_by(&entries, steps).into();
        if out.active_changed() {
            let _ = self.ensure_active_visible(items);
        }
        out
    }

    /// Next enabled item.
    pub fn move_next(&mut self, items: &[CollectionItem<Id>]) -> CollectionOutcome<Id> {
        self.move_by(items, 1)
    }

    /// Previous enabled item.
    pub fn move_previous(&mut self, items: &[CollectionItem<Id>]) -> CollectionOutcome<Id> {
        self.move_by(items, -1)
    }

    /// First enabled item (scrolls to top).
    pub fn move_first(&mut self, items: &[CollectionItem<Id>]) -> CollectionOutcome<Id> {
        let entries = Self::to_roving_entries(items);
        let out: CollectionOutcome<Id> = self.roving.move_first(&entries).into();
        if out.active_changed() {
            self.offset = self.window_start.unwrap_or(0);
        }
        out
    }

    /// Last enabled item.
    pub fn move_last(&mut self, items: &[CollectionItem<Id>]) -> CollectionOutcome<Id> {
        let entries = Self::to_roving_entries(items);
        let out: CollectionOutcome<Id> = self.roving.move_last(&entries).into();
        if out.active_changed() {
            let _ = self.ensure_active_visible(items);
        }
        out
    }

    /// Page-scale move of the active item among the projection.
    pub fn move_page(
        &mut self,
        items: &[CollectionItem<Id>],
        direction: isize,
    ) -> CollectionOutcome<Id> {
        let page = self.viewport_len.max(1) as isize;
        self.move_by(items, page * direction.signum())
    }

    /// Scrolls the viewport without necessarily changing active.
    pub fn scroll_by(&mut self, delta: isize) -> CollectionOutcome<Id> {
        if self.viewport_len == 0 || self.total_len == 0 {
            return CollectionOutcome::Ignored;
        }
        let max = self
            .total_len
            .saturating_sub(self.viewport_len.min(self.total_len));
        let before = self.offset;
        if delta.is_negative() {
            self.offset = self.offset.saturating_sub(delta.unsigned_abs());
        } else {
            self.offset = self.offset.saturating_add(delta.unsigned_abs()).min(max);
        }
        if before == self.offset {
            CollectionOutcome::Ignored
        } else {
            CollectionOutcome::Scrolled
        }
    }

    /// Ensures active lies within `[offset, offset+viewport)` when possible.
    pub fn ensure_active_visible(&mut self, items: &[CollectionItem<Id>]) -> CollectionOutcome<Id> {
        let Some(active) = self.roving.active() else {
            return CollectionOutcome::Ignored;
        };
        let Some(local_idx) = items.iter().position(|i| &i.id == active) else {
            return CollectionOutcome::Ignored;
        };
        let idx = self.window_start.map_or(local_idx, |window_start| {
            window_start.saturating_add(local_idx)
        });
        let vp = self.viewport_len.max(1);
        let before = self.offset;
        if idx < self.offset {
            self.offset = idx;
        } else if idx >= self.offset.saturating_add(vp) {
            self.offset = idx.saturating_add(1).saturating_sub(vp);
        }
        let max = self.total_len.saturating_sub(vp.min(self.total_len));
        self.offset = self.offset.min(max);
        if before == self.offset {
            CollectionOutcome::Ignored
        } else {
            CollectionOutcome::Scrolled
        }
    }

    /// Semantic intents for move / page.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        items: &[CollectionItem<Id>],
    ) -> CollectionOutcome<Id> {
        match intent {
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down | NavigationMove::Right) => {
                self.move_next(items)
            }
            UiIntent::Move(
                NavigationMove::Previous | NavigationMove::Up | NavigationMove::Left,
            ) => self.move_previous(items),
            UiIntent::Move(NavigationMove::First) => self.move_first(items),
            UiIntent::Move(NavigationMove::Last) => self.move_last(items),
            UiIntent::Page(crate::interaction::PageMove::Forward) => self.move_page(items, 1),
            UiIntent::Page(crate::interaction::PageMove::Backward) => self.move_page(items, -1),
            _ => {
                let entries = Self::to_roving_entries(items);
                self.roving.handle_intent(intent, &entries).into()
            }
        }
    }

    /// Key routing via roving (Home/End/arrows/typeahead) + ensures visibility.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        items: &[CollectionItem<Id>],
    ) -> CollectionOutcome<Id> {
        let entries = Self::to_roving_entries(items);
        let out: CollectionOutcome<Id> = self.roving.handle_key(key, &entries).into();
        if out.active_changed() {
            let _ = self.ensure_active_visible(items);
        }
        out
    }

    /// Index of active within `items`, if any.
    #[must_use]
    pub fn active_index(&self, items: &[CollectionItem<Id>]) -> Option<usize> {
        self.roving
            .active()
            .and_then(|id| items.iter().position(|i| &i.id == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::PageMove;

    fn items(specs: &[(&'static str, bool)]) -> Vec<CollectionItem<&'static str>> {
        specs
            .iter()
            .map(|(id, en)| CollectionItem::new(*id, *id).enabled(*en))
            .collect()
    }

    #[test]
    fn reconcile_and_skip_disabled() {
        let mut c = CollectionState::new();
        let list = items(&[("a", true), ("b", false), ("c", true)]);
        let _ = c.reconcile(&list);
        assert_eq!(c.active(), Some(&"a"));
        assert!(c.move_next(&list).active_changed());
        assert_eq!(c.active(), Some(&"c"));
        assert_eq!(c.current(), Some(&"c"));
    }

    #[test]
    fn virtual_window_metadata() {
        let mut c = CollectionState::new();
        let window = items(&[("c", true), ("d", true)]);
        let _ = c.reconcile_window(&window, 2, 10, 2);
        assert_eq!(c.offset(), 2);
        assert_eq!(c.total_len(), 10);
        assert_eq!(c.viewport_len(), 2);
        assert_eq!(c.active(), Some(&"c"));
    }

    #[test]
    fn virtual_window_movement_keeps_absolute_offset() {
        let mut c = CollectionState::new();
        let window = items(&[("c", true), ("d", true)]);
        let _ = c.reconcile_window(&window, 50, 200, 2);

        assert!(c.move_next(&window).active_changed());
        assert_eq!(c.active(), Some(&"d"));
        assert_eq!(c.offset(), 50);
        assert_eq!(c.total_len(), 200);

        let _ = c.move_first(&window);
        assert_eq!(c.active(), Some(&"c"));
        assert_eq!(c.offset(), 50);
    }

    #[test]
    fn ensure_visible_scrolls() {
        let mut c = CollectionState::new();
        c.set_viewport(0, 2, 5);
        let list = items(&[
            ("a", true),
            ("b", true),
            ("c", true),
            ("d", true),
            ("e", true),
        ]);
        let _ = c.reconcile(&list);
        c.set_active(Some("d"));
        assert!(matches!(
            c.ensure_active_visible(&list),
            CollectionOutcome::Scrolled
        ));
        assert!(c.offset() <= 3);
        assert!(c.offset() + c.viewport_len() > 3);
    }

    #[test]
    fn page_and_intent() {
        let mut c = CollectionState::new();
        c.set_viewport(0, 2, 6);
        let list = items(&[
            ("a", true),
            ("b", true),
            ("c", true),
            ("d", true),
            ("e", true),
            ("f", true),
        ]);
        let _ = c.reconcile(&list);
        let _ = c.handle_intent(UiIntent::Page(PageMove::Forward), &list);
        assert_ne!(c.active(), Some(&"a"));
    }

    #[test]
    fn no_borrowed_labels_in_state() {
        let mut c = CollectionState::new();
        {
            let list = vec![CollectionItem::new("x", "Temporary Label")];
            let _ = c.reconcile(&list);
        }
        // State only retains id, not the label string.
        assert_eq!(c.active(), Some(&"x"));
        assert!(c.roving().typeahead_buffer().is_empty());
    }

    #[test]
    fn property_reorder_and_disable() {
        let labels = ["a", "b", "c", "d", "e"];
        let mut c = CollectionState::new();
        for seed in 0..40u32 {
            let mut specs: Vec<(&str, bool)> = labels
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    let en = ((seed as usize) + i * 3) % 4 != 0;
                    (*l, en)
                })
                .collect();
            if specs.iter().all(|(_, e)| !e) {
                specs[0].1 = true;
            }
            // reorder
            if seed % 2 == 0 {
                specs.reverse();
            }
            let list = items(&specs);
            let _ = c.reconcile(&list);
            let _ = c.move_next(&list);
            let _ = c.move_previous(&list);
            if let Some(id) = c.active() {
                let row = list.iter().find(|i| &i.id == id).unwrap();
                assert!(row.enabled);
            }
        }
    }

    #[test]
    fn optional_parent_does_not_break_flat_move() {
        let list = vec![
            CollectionItem::new("root", "Root"),
            CollectionItem::new("child", "Child").parent("root"),
        ];
        let mut c = CollectionState::new();
        let _ = c.reconcile(&list);
        assert!(c.move_next(&list).active_changed());
        assert_eq!(c.active(), Some(&"child"));
    }
}
