// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! SelectionModel — single / multi / range selection with stable IDs.
//!
//! Separates **selection membership** from focus (`FocusGraph`) and active
//! cursor (`CollectionState`).
//!
//! Virtualization / filters: only pass **currently addressable** ids into
//! `select_all` / `invert` / `extend_range`. Membership of ids outside the
//! window is retained until [`SelectionModel::reconcile`] is told the universe
//! of still-valid ids (or `reconcile_retain` keeps unknown ids for lazy hosts).
/// How selection membership behaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SelectionKind {
    /// Selection disabled (no membership).
    #[default]
    None,
    /// At most one selected id.
    Single,
    /// Ordered multi-select (toggle / add / remove).
    Multiple,
    /// Multi with shift-style range from [`SelectionModel::anchor`].
    Range,
}

/// Typed selection mutation for hosts and tests.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectionDelta<Id> {
    /// Membership emptied.
    Cleared,
    /// Full replacement of the ordered set.
    Replaced {
        /// New ordered selection.
        selected: Vec<Id>,
    },
    /// Id added.
    Added {
        /// Added identity.
        id: Id,
    },
    /// Id removed.
    Removed {
        /// Removed identity.
        id: Id,
    },
    /// Toggle result.
    Toggled {
        /// Identity.
        id: Id,
        /// Selected after toggle.
        selected: bool,
    },
    /// Contiguous range applied (ordered list of members after op).
    RangeApplied {
        /// Inclusive range endpoints in visible order (not necessarily sorted ids).
        from: Id,
        /// End id.
        to: Id,
        /// Full membership after the range op.
        selected: Vec<Id>,
    },
}

impl<Id> SelectionDelta<Id> {}

/// Visual recipe requirement — never color alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SelectionVisual {
    /// Leading gutter glyph (`▎`).
    Gutter,
    /// Soft tint (`Role::SelectionTint`).
    Tint,
    /// Multi-select check mark.
    Check,
}

impl SelectionVisual {
    /// Whether this recipe requires a non-color glyph cue.
    #[must_use]
    pub const fn requires_glyph(self) -> bool {
        matches!(self, Self::Gutter | Self::Check)
    }
}

/// Ordered selection membership for stable identities.
///
/// # Focus vs active vs selected
///
/// - **Focus** — keyboard surface (`FocusGraph`).
/// - **Active** — collection cursor (`CollectionState`).
/// - **Selected / checked** — this type’s membership set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionModel<Id> {
    kind: SelectionKind,
    /// Ordered membership (check / select order).
    selected: Vec<Id>,
    /// Range anchor (first click / start of shift-range).
    anchor: Option<Id>,
}

impl<Id> Default for SelectionModel<Id> {
    fn default() -> Self {
        Self::new(SelectionKind::None)
    }
}

impl<Id> SelectionModel<Id> {
    /// Empty model with the given kind.
    #[must_use]
    pub const fn new(kind: SelectionKind) -> Self {
        Self {
            kind,
            selected: Vec::new(),
            anchor: None,
        }
    }

    /// Single-select model.
    #[must_use]
    pub const fn single() -> Self {
        Self::new(SelectionKind::Single)
    }

    /// Multi-select (ordered).
    #[must_use]
    pub const fn multiple() -> Self {
        Self::new(SelectionKind::Multiple)
    }

    /// Multi-select with range operations.
    #[must_use]
    pub const fn range() -> Self {
        Self::new(SelectionKind::Range)
    }

    /// Current kind.
    #[must_use]
    pub const fn kind(&self) -> SelectionKind {
        self.kind
    }

    /// Ordered selected identities.
    #[must_use]
    pub fn selected(&self) -> &[Id] {
        &self.selected
    }

    /// Range anchor, if any.
    #[must_use]
    pub const fn anchor(&self) -> Option<&Id> {
        self.anchor.as_ref()
    }

    /// Number of selected ids.
    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Clears membership and anchor without changing kind.
    pub fn clear(&mut self) -> SelectionDelta<Id> {
        self.selected.clear();
        self.anchor = None;
        SelectionDelta::Cleared
    }
}

impl<Id: Clone + PartialEq> SelectionModel<Id> {
    /// Whether `id` is in the selection set.
    #[must_use]
    pub fn is_selected(&self, id: &Id) -> bool {
        self.selected.iter().any(|s| s == id)
    }

    /// Alias of [`Self::is_selected`] (list “checked” vocabulary).
    #[must_use]
    pub fn is_checked(&self, id: &Id) -> bool {
        self.is_selected(id)
    }

    /// Ordered checked ids (alias of [`Self::selected`]).
    #[must_use]
    pub fn checked(&self) -> &[Id] {
        self.selected()
    }
    /// Application-controlled replace of the entire set.
    pub fn replace(&mut self, ids: impl IntoIterator<Item = Id>) -> SelectionDelta<Id> {
        if matches!(self.kind, SelectionKind::None) {
            return SelectionDelta::Cleared;
        }
        self.selected.clear();
        for id in ids {
            if matches!(self.kind, SelectionKind::Single) {
                self.selected.clear();
                self.selected.push(id);
                break;
            }
            if !self.selected.iter().any(|s| s == &id) {
                self.selected.push(id);
            }
        }
        if self.selected.is_empty() {
            self.anchor = None;
            SelectionDelta::Cleared
        } else {
            if self.anchor.is_none() {
                self.anchor = self.selected.first().cloned();
            }
            SelectionDelta::Replaced {
                selected: self.selected.clone(),
            }
        }
    }

    /// Selects one id (single replaces; multi/range adds).
    pub fn select(&mut self, id: Id) -> SelectionDelta<Id> {
        match self.kind {
            SelectionKind::None => SelectionDelta::Cleared,
            SelectionKind::Single => {
                let changed = self.selected.first() != Some(&id);
                self.selected.clear();
                self.selected.push(id.clone());
                self.anchor = Some(id.clone());
                if changed {
                    SelectionDelta::Replaced {
                        selected: self.selected.clone(),
                    }
                } else {
                    SelectionDelta::Replaced {
                        selected: self.selected.clone(),
                    }
                }
            }
            SelectionKind::Multiple | SelectionKind::Range => {
                if self.is_selected(&id) {
                    return SelectionDelta::Replaced {
                        selected: self.selected.clone(),
                    };
                }
                self.selected.push(id.clone());
                if self.anchor.is_none() {
                    self.anchor = Some(id.clone());
                }
                SelectionDelta::Added { id }
            }
        }
    }

    /// Removes one id if present.
    pub fn deselect(&mut self, id: &Id) -> SelectionDelta<Id> {
        if matches!(self.kind, SelectionKind::None) {
            return SelectionDelta::Cleared;
        }
        if let Some(index) = self.selected.iter().position(|s| s == id) {
            self.selected.remove(index);
            if self.anchor.as_ref() == Some(id) {
                self.anchor = self.selected.first().cloned();
            }
            SelectionDelta::Removed { id: id.clone() }
        } else {
            SelectionDelta::Replaced {
                selected: self.selected.clone(),
            }
        }
    }

    /// Toggles membership (no-op when kind is None).
    pub fn toggle(&mut self, id: &Id) -> SelectionDelta<Id> {
        match self.kind {
            SelectionKind::None => SelectionDelta::Cleared,
            SelectionKind::Single => {
                if self.is_selected(id) {
                    self.selected.clear();
                    self.anchor = None;
                    SelectionDelta::Toggled {
                        id: id.clone(),
                        selected: false,
                    }
                } else {
                    self.selected.clear();
                    self.selected.push(id.clone());
                    self.anchor = Some(id.clone());
                    SelectionDelta::Toggled {
                        id: id.clone(),
                        selected: true,
                    }
                }
            }
            SelectionKind::Multiple | SelectionKind::Range => {
                let selected = if let Some(index) = self.selected.iter().position(|s| s == id) {
                    self.selected.remove(index);
                    if self.anchor.as_ref() == Some(id) {
                        self.anchor = self.selected.first().cloned();
                    }
                    false
                } else {
                    self.selected.push(id.clone());
                    if self.anchor.is_none() {
                        self.anchor = Some(id.clone());
                    }
                    true
                };
                SelectionDelta::Toggled {
                    id: id.clone(),
                    selected,
                }
            }
        }
    }

    /// Sets the range anchor (start of shift-range).
    pub fn set_anchor(&mut self, id: Option<Id>) {
        self.anchor = id;
    }

    /// Selects every id in `visible` that is enabled (host filters disabled).
    ///
    /// Does not remove ids already selected that are outside `visible`
    /// (filtered views). Use [`Self::reconcile`] to drop vanished ids.
    pub fn select_all(&mut self, visible: &[Id]) -> SelectionDelta<Id> {
        if matches!(self.kind, SelectionKind::None) {
            return SelectionDelta::Cleared;
        }
        if matches!(self.kind, SelectionKind::Single) {
            if let Some(id) = visible.first() {
                return self.select(id.clone());
            }
            return self.clear();
        }
        for id in visible {
            if !self.is_selected(id) {
                self.selected.push(id.clone());
            }
        }
        if self.anchor.is_none() {
            self.anchor = visible.first().cloned();
        }
        SelectionDelta::Replaced {
            selected: self.selected.clone(),
        }
    }

    /// Inverts membership for each id in `visible` (multi/range only).
    pub fn invert_visible(&mut self, visible: &[Id]) -> SelectionDelta<Id> {
        if !matches!(self.kind, SelectionKind::Multiple | SelectionKind::Range) {
            return SelectionDelta::Cleared;
        }
        for id in visible {
            let _ = self.toggle(id);
        }
        SelectionDelta::Replaced {
            selected: self.selected.clone(),
        }
    }

    /// Replaces selection with exactly the range `[anchor, to]` along `order`.
    pub fn set_range(&mut self, order: &[Id], to: &Id) -> SelectionDelta<Id> {
        if !matches!(self.kind, SelectionKind::Range | SelectionKind::Multiple) {
            return self.select(to.clone());
        }
        let anchor = self.anchor.clone().unwrap_or_else(|| to.clone());
        let Some(ai) = order.iter().position(|id| id == &anchor) else {
            self.selected.clear();
            return self.select(to.clone());
        };
        let Some(ti) = order.iter().position(|id| id == to) else {
            return SelectionDelta::Replaced {
                selected: self.selected.clone(),
            };
        };
        let (lo, hi) = if ai <= ti { (ai, ti) } else { (ti, ai) };
        self.selected = order[lo..=hi].to_vec();
        self.anchor = Some(anchor.clone());
        SelectionDelta::RangeApplied {
            from: anchor,
            to: to.clone(),
            selected: self.selected.clone(),
        }
    }

    /// Drops selected ids not present in `still_valid` (e.g. deleted rows).
    ///
    /// Ids only missing from a **filter window** should not use this — use
    /// [`Self::reconcile_retain`] when off-window selection must survive.
    pub fn reconcile(&mut self, still_valid: &[Id]) -> SelectionDelta<Id> {
        let before = self.selected.len();
        self.selected
            .retain(|id| still_valid.iter().any(|v| v == id));
        if let Some(a) = &self.anchor
            && !still_valid.iter().any(|v| v == a)
        {
            self.anchor = self.selected.first().cloned();
        }
        if self.selected.len() != before {
            if self.selected.is_empty() {
                SelectionDelta::Cleared
            } else {
                SelectionDelta::Replaced {
                    selected: self.selected.clone(),
                }
            }
        } else {
            SelectionDelta::Replaced {
                selected: self.selected.clone(),
            }
        }
    }
}

/// Hierarchical helper: select `id` and every id listed as descendant in `order`.
///
/// Host supplies flattened visible order with descendants contiguous or not —
/// only ids in `subtree` are selected (application projects the tree).
pub fn select_subtree<Id: Clone + PartialEq>(
    model: &mut SelectionModel<Id>,
    subtree: &[Id],
) -> SelectionDelta<Id> {
    model.select_all(subtree)
}

/// Deselect every id in `subtree` (collapse-deselect pattern).
pub fn deselect_subtree<Id: Clone + PartialEq>(
    model: &mut SelectionModel<Id>,
    subtree: &[Id],
) -> SelectionDelta<Id> {
    for id in subtree {
        let _ = model.deselect(id);
    }
    if model.is_empty() {
        SelectionDelta::Cleared
    } else {
        SelectionDelta::Replaced {
            selected: model.selected().to_vec(),
        }
    }
}

// ── Cell / rectangular selection (tables, grids) ─────────────────────────────

/// Logical cell coordinate (row index in projection + column ordinal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct CellCoord {
    /// Logical row index in the current projection / window.
    pub row: u64,
    /// Column ordinal among the host column model.
    pub col: usize,
}

impl CellCoord {
    /// Creates a cell coordinate.
    #[must_use]
    pub const fn new(row: u64, col: usize) -> Self {
        Self { row, col }
    }
}

/// Cell selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CellSelectionMode {
    /// No cell membership.
    #[default]
    None,
    /// Single active cell.
    Single,
    /// Inclusive rectangular range from anchor to extent.
    Range,
}

/// Rectangular / single cell selection (stable as long as projection indices hold).
///
/// Hosts map `CellCoord` ↔ domain (row id, column id) for clipboard and edits.
/// Focus/cursor may share `active` while membership lives in `selected` for multi.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CellSelectionModel {
    mode: CellSelectionMode,
    /// Keyboard / active cell.
    active: Option<CellCoord>,
    /// Range anchor (primary click).
    anchor: Option<CellCoord>,
    /// Range extent (shift-click / drag end).
    extent: Option<CellCoord>,
}

impl CellSelectionModel {
    /// Empty, no cell selection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            mode: CellSelectionMode::None,
            active: None,
            anchor: None,
            extent: None,
        }
    }

    /// Single-cell mode.
    #[must_use]
    pub const fn single() -> Self {
        Self {
            mode: CellSelectionMode::Single,
            active: None,
            anchor: None,
            extent: None,
        }
    }

    /// Rectangular range mode.
    #[must_use]
    pub const fn range() -> Self {
        Self {
            mode: CellSelectionMode::Range,
            active: None,
            anchor: None,
            extent: None,
        }
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> CellSelectionMode {
        self.mode
    }

    /// Active cell (cursor).
    #[must_use]
    pub const fn active(&self) -> Option<CellCoord> {
        self.active
    }

    /// Range anchor.
    #[must_use]
    pub const fn anchor(&self) -> Option<CellCoord> {
        self.anchor
    }

    /// Range extent.
    #[must_use]
    pub const fn extent(&self) -> Option<CellCoord> {
        self.extent
    }

    /// Clears cell membership (keeps mode).
    pub fn clear(&mut self) {
        self.active = None;
        self.anchor = None;
        self.extent = None;
    }

    /// Focus/select one cell (single) or set active + collapse range.
    pub fn select_cell(&mut self, cell: CellCoord) {
        self.active = Some(cell);
        self.anchor = Some(cell);
        self.extent = Some(cell);
    }

    /// Extends rectangular range from anchor to `cell` (creates anchor if missing).
    pub fn extend_to(&mut self, cell: CellCoord) {
        if self.anchor.is_none() {
            self.anchor = Some(cell);
        }
        self.extent = Some(cell);
        self.active = Some(cell);
        if matches!(self.mode, CellSelectionMode::None) {
            self.mode = CellSelectionMode::Range;
        }
    }

    /// Inclusive normalized rectangle (min/max row/col).
    #[must_use]
    pub fn rect(&self) -> Option<(CellCoord, CellCoord)> {
        let a = self.anchor?;
        let b = self.extent.or(self.active)?;
        let min = CellCoord {
            row: a.row.min(b.row),
            col: a.col.min(b.col),
        };
        let max = CellCoord {
            row: a.row.max(b.row),
            col: a.col.max(b.col),
        };
        Some((min, max))
    }

    /// Whether `cell` lies in the current rect (or equals active in single mode).
    #[must_use]
    pub fn contains(&self, cell: CellCoord) -> bool {
        match self.mode {
            CellSelectionMode::None => false,
            CellSelectionMode::Single => self.active == Some(cell),
            CellSelectionMode::Range => {
                let Some((min, max)) = self.rect() else {
                    return self.active == Some(cell);
                };
                cell.row >= min.row
                    && cell.row <= max.row
                    && cell.col >= min.col
                    && cell.col <= max.col
            }
        }
    }

    /// Enumerates cells in the rect (row-major). Empty if none.
    ///
    /// Caps at `max_cells` to avoid huge allocations on 1M-row universes.
    #[must_use]
    pub fn cells(&self, max_cells: usize) -> Vec<CellCoord> {
        let Some((min, max)) = self.rect() else {
            return self.active.into_iter().collect();
        };
        let mut out = Vec::new();
        let mut n = 0usize;
        for row in min.row..=max.row {
            for col in min.col..=max.col {
                if n >= max_cells {
                    return out;
                }
                out.push(CellCoord { row, col });
                n += 1;
            }
        }
        out
    }

    /// Move active cell by delta within bounds; optionally extends range when `extend`.
    pub fn move_active(
        &mut self,
        d_row: i64,
        d_col: i32,
        max_row: u64,
        max_col: usize,
        extend: bool,
    ) -> bool {
        let mut cell = self.active.unwrap_or(CellCoord::new(0, 0));
        let before = cell;
        if d_row >= 0 {
            cell.row = cell.row.saturating_add(d_row as u64).min(max_row);
        } else {
            cell.row = cell.row.saturating_sub((-d_row) as u64);
        }
        if max_col == 0 {
            cell.col = 0;
        } else if d_col >= 0 {
            cell.col = cell.col.saturating_add(d_col as usize).min(max_col - 1);
        } else {
            cell.col = cell.col.saturating_sub((-d_col) as usize);
        }
        if before == cell {
            return false;
        }
        self.active = Some(cell);
        if extend {
            self.extend_to(cell);
        } else {
            self.anchor = Some(cell);
            self.extent = Some(cell);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_replaces() {
        let mut m = SelectionModel::single();
        let _ = m.select("a");
        let _ = m.select("b");
        assert_eq!(m.selected(), &["b"]);
        assert!(m.is_selected(&"b"));
        assert!(!m.is_selected(&"a"));
    }

    #[test]
    fn multiple_toggle_order() {
        let mut m = SelectionModel::multiple();
        assert!(matches!(
            m.toggle(&"beta"),
            SelectionDelta::Toggled { selected: true, .. }
        ));
        assert!(matches!(
            m.toggle(&"alpha"),
            SelectionDelta::Toggled { selected: true, .. }
        ));
        assert_eq!(m.checked(), ["beta", "alpha"]);
        assert!(matches!(
            m.toggle(&"beta"),
            SelectionDelta::Toggled {
                selected: false,
                ..
            }
        ));
        assert_eq!(m.checked(), ["alpha"]);
    }

    #[test]
    fn range_set_along_order() {
        let mut m = SelectionModel::range();
        m.set_anchor(Some("b"));
        let order = ["a", "b", "c", "d", "e"];
        let d = m.set_range(&order, &"d");
        assert!(matches!(d, SelectionDelta::RangeApplied { .. }));
        assert_eq!(m.selected(), ["b", "c", "d"]);
    }

    #[test]
    fn select_all_and_invert_visible() {
        let mut m = SelectionModel::multiple();
        let visible = ["a", "b", "c"];
        let _ = m.select_all(&visible);
        assert_eq!(m.len(), 3);
        let _ = m.invert_visible(&["b"]);
        assert!(!m.is_selected(&"b"));
        assert!(m.is_selected(&"a"));
    }

    #[test]
    fn filtered_view_keeps_offwindow_until_reconcile() {
        let mut m = SelectionModel::multiple();
        let _ = m.select("hidden");
        let _ = m.select("a");
        // Filter shows only a,b — do not drop hidden
        let _ = m.select_all(&["a", "b"]);
        assert!(m.is_selected(&"hidden"));
        // Delete hidden from data
        let _ = m.reconcile(&["a", "b", "c"]);
        assert!(!m.is_selected(&"hidden"));
        assert!(m.is_selected(&"a"));
    }

    #[test]
    fn disabled_skipped_by_host_order() {
        // Host omits disabled from order; range never selects them.
        let mut m = SelectionModel::range();
        m.set_anchor(Some("a"));
        let order = ["a", "c"]; // b disabled omitted
        let _ = m.set_range(&order, &"c");
        assert_eq!(m.selected(), ["a", "c"]);
    }

    #[test]
    fn visual_recipe_requires_glyph_for_gutter_and_check() {
        assert!(SelectionVisual::Gutter.requires_glyph());
        assert!(SelectionVisual::Check.requires_glyph());
        assert!(!SelectionVisual::Tint.requires_glyph());
    }

    #[test]
    fn property_random_toggle_consistency() {
        let ids = ["a", "b", "c", "d", "e"];
        let mut m = SelectionModel::multiple();
        for seed in 0..80u32 {
            let id = ids[(seed as usize) % ids.len()];
            let before = m.is_selected(&id);
            let d = m.toggle(&id);
            match d {
                SelectionDelta::Toggled { selected, .. } => {
                    assert_eq!(selected, !before);
                    assert_eq!(m.is_selected(&id), selected);
                }
                _ => panic!("expected toggle"),
            }
            // occasional clear / select_all on prefix
            if seed % 11 == 0 {
                let _ = m.clear();
            }
            if seed % 13 == 0 {
                let _ = m.select_all(&ids[..((seed as usize) % 5 + 1)]);
            }
            // membership unique
            let mut seen = Vec::new();
            for s in m.selected() {
                assert!(!seen.contains(s));
                seen.push(*s);
            }
        }
    }

    #[test]
    fn select_subtree_helper() {
        let mut m = SelectionModel::multiple();
        let sub = ["folder", "file1", "file2"];
        let _ = select_subtree(&mut m, &sub);
        assert_eq!(m.len(), 3);
        let _ = deselect_subtree(&mut m, &["file1"]);
        assert!(!m.is_selected(&"file1"));
        assert!(m.is_selected(&"folder"));
    }

    #[test]
    fn cell_range_contains_and_enumerate() {
        let mut c = CellSelectionModel::range();
        c.select_cell(CellCoord::new(1, 1));
        c.extend_to(CellCoord::new(3, 2));
        assert!(c.contains(CellCoord::new(2, 1)));
        assert!(!c.contains(CellCoord::new(0, 0)));
        let cells = c.cells(100);
        assert_eq!(cells.len(), 3 * 2); // rows 1..=3, cols 1..=2
    }

    #[test]
    fn cell_move_extend_keeps_anchor() {
        let mut c = CellSelectionModel::range();
        c.select_cell(CellCoord::new(0, 0));
        assert!(c.move_active(2, 1, 10, 5, true));
        assert_eq!(c.anchor(), Some(CellCoord::new(0, 0)));
        assert_eq!(c.active(), Some(CellCoord::new(2, 1)));
        assert!(c.contains(CellCoord::new(1, 0)));
    }
}
