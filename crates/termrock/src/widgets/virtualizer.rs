// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Canonical 1D / 2D virtualizer for large collections and grids.
//!
//! [`Virtualizer`] owns **window math only**: logical length, viewport extent,
//! item offset, overscan, sticky counts, sparse variable-extent measures, and
//! anchors. Hosts project resident items for `visible_slice` / `measure_slice`
//! and must not allocate O(logical_len) structures for paint or semantics.
//!
//! [`data_view::VirtualWindow`] remains the fixed unit-slot facade used by
//! DataTable; it is equivalent to `Virtualizer::fixed(1)`.
use std::collections::BTreeMap;

use crate::perf::{ScrollAnchor, ScrollAnchorKind};

/// Half-open `[start, end)` — same contract as scroll visible ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VirtRange {
    /// First unit.
    pub start: u64,
    /// One past last unit.
    pub end: u64,
}

impl VirtRange {
    /// Length.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Empty?
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// How each logical item maps to display extent (rows or columns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExtentPolicy {
    /// Every item occupies the same extent (usual list/table row = 1).
    Fixed(u16),
    /// Default estimate until measured; sparse overrides in state.
    Variable {
        /// Assumed extent before host calls [`Virtualizer::note_measured`].
        estimated: u16,
    },
}

impl Default for ExtentPolicy {
    fn default() -> Self {
        Self::Fixed(1)
    }
}

impl ExtentPolicy {
    /// Positive extent used for estimates.
    #[must_use]
    pub const fn unit(self) -> u16 {
        match self {
            Self::Fixed(e) | Self::Variable { estimated: e } => {
                if e == 0 {
                    1
                } else {
                    e
                }
            }
        }
    }
}

/// Sticky leading/trailing item counts (always in the semantic set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StickyRegion {
    /// Items pinned at the start (e.g. header rows that stay in the set).
    pub leading: u64,
    /// Items pinned at the end.
    pub trailing: u64,
}

/// Half-open window with optional overscan for measure/prefetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct VirtSlice {
    /// First visible (or sticky-adjusted body) index inclusive.
    pub start: u64,
    /// One past last visible index.
    pub end: u64,
    /// Prefetch/measure start (may be before `start`).
    pub measure_start: u64,
    /// Prefetch/measure end exclusive (may be after `end`).
    pub measure_end: u64,
}

impl VirtSlice {
    /// Visible length.
    #[must_use]
    pub const fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Whether the visible window is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Measure/prefetch length (includes overscan).
    #[must_use]
    pub const fn measure_len(self) -> u64 {
        self.measure_end.saturating_sub(self.measure_start)
    }

    /// Visible half-open range only.
    #[must_use]
    pub const fn visible(self) -> VirtRange {
        VirtRange {
            start: self.start,
            end: self.end,
        }
    }
}

/// One-dimensional virtualizer (rows **or** columns).
///
/// **Offset** is always a **logical item index** (terminal-native). Variable
/// extents affect how many items fit in `viewport_extent` and total size
/// estimates; they never force a dense extent array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Virtualizer {
    logical_len: u64,
    /// Viewport size in the same units as item extents (usually terminal rows).
    viewport_extent: u16,
    /// First non-sticky body item index at the top of the scrollable viewport.
    offset: u64,
    overscan: u16,
    policy: ExtentPolicy,
    sticky: StickyRegion,
    /// Sparse measured extents (variable policy only). Never size O(logical_len).
    measured: BTreeMap<u64, u16>,
    anchor: Option<ScrollAnchor>,
}

impl Default for Virtualizer {
    fn default() -> Self {
        Self::fixed(1)
    }
}

impl Virtualizer {
    /// Fixed extent per item (default list/table = 1).
    #[must_use]
    pub fn fixed(extent: u16) -> Self {
        Self {
            logical_len: 0,
            viewport_extent: 1,
            offset: 0,
            overscan: 0,
            policy: ExtentPolicy::Fixed(extent.max(1)),
            sticky: StickyRegion::default(),
            measured: BTreeMap::new(),
            anchor: None,
        }
    }

    /// Variable extents with a positive estimate before measure.
    #[must_use]
    pub fn variable(estimated: u16) -> Self {
        Self {
            logical_len: 0,
            viewport_extent: 1,
            offset: 0,
            overscan: 0,
            policy: ExtentPolicy::Variable {
                estimated: estimated.max(1),
            },
            sticky: StickyRegion::default(),
            measured: BTreeMap::new(),
            anchor: None,
        }
    }

    /// Logical item count (may be 1_000_000; no allocation).
    #[must_use]
    pub const fn with_len(mut self, logical_len: u64) -> Self {
        self.logical_len = logical_len;
        self
    }

    /// Viewport extent in display units.
    #[must_use]
    pub const fn with_viewport(mut self, viewport_extent: u16) -> Self {
        self.viewport_extent = if viewport_extent == 0 {
            1
        } else {
            viewport_extent
        };
        self
    }

    /// Extra items measured before/after the visible window.
    #[must_use]
    pub const fn with_overscan(mut self, overscan: u16) -> Self {
        self.overscan = overscan;
        self
    }

    /// Sticky leading/trailing counts.
    #[must_use]
    pub const fn with_sticky(mut self, sticky: StickyRegion) -> Self {
        self.sticky = sticky;
        self
    }

    /// Logical length.
    #[must_use]
    pub const fn logical_len(&self) -> u64 {
        self.logical_len
    }

    /// Viewport extent.
    #[must_use]
    pub const fn viewport_extent(&self) -> u16 {
        self.viewport_extent
    }

    /// First body item index.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }

    /// Overscan items.
    #[must_use]
    pub const fn overscan(&self) -> u16 {
        self.overscan
    }

    /// Extent policy.
    #[must_use]
    pub const fn policy(&self) -> ExtentPolicy {
        self.policy
    }

    /// Sticky region.
    #[must_use]
    pub const fn sticky(&self) -> StickyRegion {
        self.sticky
    }

    /// Current anchor, if any.
    #[must_use]
    pub const fn anchor(&self) -> Option<&ScrollAnchor> {
        self.anchor.as_ref()
    }

    /// Number of sparse measure entries (debug / tests).
    #[must_use]
    pub fn measured_count(&self) -> usize {
        self.measured.len()
    }

    /// Set logical universe size without allocating items.
    pub fn set_len(&mut self, logical_len: u64) {
        self.logical_len = logical_len;
        self.clamp();
    }

    /// Viewport resize (display units).
    pub fn set_viewport_extent(&mut self, viewport_extent: u16) {
        self.viewport_extent = viewport_extent.max(1);
        self.restore_or_clamp();
    }

    /// Overscan for measure/prefetch.
    pub fn set_overscan(&mut self, overscan: u16) {
        self.overscan = overscan;
    }

    /// Sticky leading/trailing.
    pub fn set_sticky(&mut self, sticky: StickyRegion) {
        self.sticky = sticky;
        self.clamp();
    }

    /// Programmatic body offset (clamped).
    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
        self.clamp();
        self.capture_index_anchor();
    }

    /// Body item count available to scroll (excludes sticky leading/trailing when possible).
    #[must_use]
    pub fn scrollable_len(&self) -> u64 {
        let sticky = self.sticky.leading.saturating_add(self.sticky.trailing);
        self.logical_len
            .saturating_sub(sticky.min(self.logical_len))
    }

    /// First scrollable item index (after leading sticky).
    #[must_use]
    pub const fn body_start(&self) -> u64 {
        self.sticky.leading
    }

    /// One past last scrollable item (before trailing sticky).
    #[must_use]
    pub fn body_end(&self) -> u64 {
        self.logical_len
            .saturating_sub(self.sticky.trailing.min(self.logical_len))
    }

    /// Extent of one item (measured override or policy unit).
    #[must_use]
    pub fn extent_of(&self, index: u64) -> u16 {
        if let Some(e) = self.measured.get(&index) {
            return (*e).max(1);
        }
        self.policy.unit()
    }

    /// Record a measured extent for a painted/measured item (variable path).
    ///
    /// Fixed policy ignores measures (always uses fixed unit). Cap: hosts should
    /// call [`Self::forget_measured_outside`] so the map stays O(viewport).
    pub fn note_measured(&mut self, index: u64, extent: u16) {
        if matches!(self.policy, ExtentPolicy::Fixed(_)) {
            return;
        }
        if index >= self.logical_len {
            return;
        }
        self.measured.insert(index, extent.max(1));
    }

    /// Drop measures outside `[measure_start - pad, measure_end + pad)`.
    pub fn forget_measured_outside(&mut self, pad: u64) {
        let slice = self.visible_slice();
        let lo = slice.measure_start.saturating_sub(pad);
        let hi = slice.measure_end.saturating_add(pad);
        self.measured.retain(|&k, _| k >= lo && k < hi);
    }

    /// How many body items fit in the viewport (fixed: viewport/unit; variable: walk estimates).
    #[must_use]
    pub fn body_capacity_items(&self) -> u64 {
        let unit = u64::from(self.policy.unit().max(1));
        let vp = u64::from(self.viewport_extent.max(1));
        match self.policy {
            ExtentPolicy::Fixed(_) => vp / unit.max(1),
            ExtentPolicy::Variable { .. } => {
                // Walk from offset using estimates until viewport filled.
                let mut used = 0u64;
                let mut count = 0u64;
                let mut i = self.offset.max(self.body_start());
                let end = self.body_end();
                while i < end && used < vp {
                    used = used.saturating_add(u64::from(self.extent_of(i)));
                    count = count.saturating_add(1);
                    i = i.saturating_add(1);
                    // Hard cap: never walk more than viewport items * small factor for safety.
                    if count > vp.saturating_mul(4).max(64) {
                        break;
                    }
                }
                count.max(1)
            }
        }
    }

    /// Max legal body offset.
    #[must_use]
    pub fn max_offset(&self) -> u64 {
        let body_len = self.scrollable_len();
        if body_len == 0 {
            return self.body_start();
        }
        let cap = self.body_capacity_items().max(1);
        if body_len <= cap {
            self.body_start()
        } else {
            self.body_start()
                .saturating_add(body_len.saturating_sub(cap))
        }
    }

    /// Clamp offset into the scrollable body.
    pub fn clamp(&mut self) {
        if self.viewport_extent == 0 {
            self.viewport_extent = 1;
        }
        let min = self.body_start();
        let max = self.max_offset();
        if self.offset < min {
            self.offset = min;
        }
        if self.offset > max {
            self.offset = max;
        }
        // Drop measures for indices past len.
        if !self.measured.is_empty() {
            let len = self.logical_len;
            self.measured.retain(|&k, _| k < len);
        }
    }

    fn restore_or_clamp(&mut self) {
        if let Some(anchor) = self.anchor.clone() {
            self.apply_anchor(&anchor, |_| None);
        } else {
            self.clamp();
        }
    }

    /// Scroll body by signed item delta.
    pub fn scroll_by(&mut self, delta: i64) -> bool {
        let before = self.offset;
        if delta >= 0 {
            self.offset = self.offset.saturating_add(delta as u64);
        } else {
            self.offset = self.offset.saturating_sub((-delta) as u64);
        }
        self.clamp();
        self.capture_index_anchor();
        before != self.offset
    }

    /// Ensure logical `index` is inside the body viewport (sticky always "visible").
    pub fn reveal(&mut self, index: u64) -> bool {
        if self.logical_len == 0 {
            return false;
        }
        let index = index.min(self.logical_len.saturating_sub(1));
        // Sticky items: no body scroll needed.
        if index < self.body_start() || index >= self.body_end() {
            return false;
        }
        let before = self.offset;
        let cap = self.body_capacity_items().max(1);
        if index < self.offset {
            self.offset = index;
        } else if index >= self.offset.saturating_add(cap) {
            self.offset = index.saturating_add(1).saturating_sub(cap);
        }
        self.clamp();
        self.capture_index_anchor();
        before != self.offset
    }

    /// Snapshot index anchor at current offset.
    pub fn capture_index_anchor(&mut self) {
        self.anchor = Some(ScrollAnchor::at_index(self.offset));
    }

    /// Snapshot from-end anchor (distance from max offset).
    pub fn capture_from_end_anchor(&mut self) {
        let max = self.max_offset();
        let dist = max.saturating_sub(self.offset);
        self.anchor = Some(ScrollAnchor::from_end(dist));
    }

    /// Apply stable anchor. `resolve_id` maps content ids → logical index.
    pub fn apply_anchor(
        &mut self,
        anchor: &ScrollAnchor,
        resolve_id: impl FnOnce(&str) -> Option<u64>,
    ) {
        match anchor.kind {
            ScrollAnchorKind::Index => {
                self.offset = anchor.index;
            }
            ScrollAnchorKind::FromEnd => {
                let max = self.max_offset();
                self.offset = max.saturating_sub(anchor.index);
            }
            ScrollAnchorKind::ContentId => {
                if let Some(id) = anchor.content_id.as_deref()
                    && let Some(idx) = resolve_id(id)
                {
                    self.offset = idx;
                    let _ = self.reveal(idx);
                }
            }
        }
        self.anchor = Some(anchor.clone());
        self.clamp();
    }

    /// After insert/delete/filter changed the logical universe.
    pub fn on_items_changed(&mut self, new_len: u64) {
        self.logical_len = new_len;
        // Drop measures past new len; keep sparse map otherwise.
        self.measured.retain(|&k, _| k < new_len);
        self.restore_or_clamp();
    }

    /// Visible body slice (no overscan) as half-open indices.
    ///
    /// Sticky items are **not** included here — use [`Self::sticky_indices`] and
    /// [`Self::semantic_count`]. Body starts at [`Self::offset`].
    #[must_use]
    pub fn visible_slice(&self) -> VirtSlice {
        if self.logical_len == 0 {
            return VirtSlice::default();
        }
        let start = self.offset.min(self.body_end());
        let cap = self.body_capacity_items().max(1);
        let end = start
            .saturating_add(cap)
            .min(self.body_end())
            .min(self.logical_len);
        let over = u64::from(self.overscan);
        let measure_start = start.saturating_sub(over).max(self.body_start());
        let measure_end = end
            .saturating_add(over)
            .min(self.body_end())
            .min(self.logical_len);
        VirtSlice {
            start,
            end,
            measure_start,
            measure_end,
        }
    }

    /// Sticky indices that must stay in the semantic set (leading then trailing).
    ///
    /// Returns at most `leading + trailing` indices — never the full dataset.
    pub fn sticky_indices(&self, out: &mut Vec<u64>) {
        out.clear();
        if self.logical_len == 0 {
            return;
        }
        let lead = self.sticky.leading.min(self.logical_len);
        for i in 0..lead {
            out.push(i);
        }
        let trail = self
            .sticky
            .trailing
            .min(self.logical_len.saturating_sub(lead));
        let start = self.logical_len.saturating_sub(trail);
        for i in start..self.logical_len {
            if i >= lead {
                out.push(i);
            }
        }
    }

    /// Items a semantic scene may register this frame: sticky + visible body.
    ///
    /// **Never** returns `logical_len` for large universes.
    #[must_use]
    pub fn semantic_count(&self) -> u64 {
        let body = self.visible_slice().len();
        let sticky = self
            .sticky
            .leading
            .saturating_add(self.sticky.trailing)
            .min(self.logical_len);
        // Overlap when sticky consumes the whole list.
        if self.scrollable_len() == 0 {
            sticky
        } else {
            body.saturating_add(sticky)
        }
    }

    /// Fill `out` with semantic indices (sticky + visible body), de-duplicated, sorted.
    pub fn semantic_indices(&self, out: &mut Vec<u64>) {
        out.clear();
        self.sticky_indices(out);
        let slice = self.visible_slice();
        for i in slice.start..slice.end {
            if !out.contains(&i) {
                out.push(i);
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// Estimated total extent (display units) without walking all items.
    #[must_use]
    pub fn total_extent_estimate(&self) -> u64 {
        let unit = u64::from(self.policy.unit().max(1));
        let base = self.logical_len.saturating_mul(unit);
        if matches!(self.policy, ExtentPolicy::Fixed(_)) || self.measured.is_empty() {
            return base;
        }
        // Adjust by measured deltas: sum(measured - unit) for each override.
        let mut adj: i64 = 0;
        for &e in self.measured.values() {
            adj += i64::from(e) - i64::from(self.policy.unit());
        }
        if adj >= 0 {
            base.saturating_add(adj as u64)
        } else {
            base.saturating_sub((-adj) as u64)
        }
    }

    /// Snapshot as fixed-slot triple `(offset, viewport_items, logical_len)`.
    ///
    /// Used by [`crate::widgets::VirtualWindow`] facades without a module cycle.
    #[must_use]
    pub fn to_fixed_slots(&self) -> (u64, u16, u64) {
        (
            self.offset.saturating_sub(self.body_start()),
            self.body_capacity_items().min(u64::from(u16::MAX)) as u16,
            self.scrollable_len(),
        )
    }

    /// Build from fixed-slot window numbers (extent = 1).
    #[must_use]
    pub fn from_fixed_slots(offset: u64, viewport: u16, logical_len: u64) -> Self {
        let mut v = Self::fixed(1)
            .with_len(logical_len)
            .with_viewport(viewport.max(1));
        v.offset = offset;
        v.clamp();
        v
    }
}

/// Two-axis virtualizer for grids (independent row/column windows).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Virtualizer2D {
    /// Vertical (rows).
    pub rows: Virtualizer,
    /// Horizontal (columns).
    pub cols: Virtualizer,
}

impl Virtualizer2D {
    /// Fixed 1×1 cell grid.
    #[must_use]
    pub fn fixed_cells() -> Self {
        Self {
            rows: Virtualizer::fixed(1),
            cols: Virtualizer::fixed(1),
        }
    }

    /// Logical dimensions without allocating cells.
    pub fn set_shape(&mut self, row_count: u64, col_count: u64) {
        self.rows.set_len(row_count);
        self.cols.set_len(col_count);
    }

    /// Viewport in cell rows/cols (fixed extent 1) or display units when variable.
    pub fn set_viewport(&mut self, row_extent: u16, col_extent: u16) {
        self.rows.set_viewport_extent(row_extent);
        self.cols.set_viewport_extent(col_extent);
    }

    /// Visible body cell budget (rows × cols) for paint caps.
    #[must_use]
    pub fn visible_cells_budget(&self) -> u64 {
        self.rows
            .visible_slice()
            .len()
            .saturating_mul(self.cols.visible_slice().len())
    }

    /// Semantic node budget (no millions).
    #[must_use]
    pub fn semantic_count(&self) -> u64 {
        // Grid semantics: typically one node per visible cell or per visible row;
        // report cell budget as upper bound for registration.
        self.rows
            .semantic_count()
            .saturating_mul(self.cols.semantic_count().max(1))
    }
}

/// Project absolute indices for a fixed-extent window (shared with VirtualWindow).
#[must_use]
pub const fn fixed_visible_range(offset: u64, viewport: u16, logical_len: u64) -> (u64, u64) {
    let start = offset;
    let vp = viewport as u64;
    let end = if logical_len == 0 {
        start.saturating_add(vp)
    } else {
        let e = start.saturating_add(vp);
        if e > logical_len { logical_len } else { e }
    };
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_view::bench;

    #[test]
    fn fixed_million_row_o1_window() {
        let mut v = Virtualizer::fixed(1)
            .with_len(bench::ROWS_1M)
            .with_viewport(bench::VIEWPORT_ROWS)
            .with_overscan(5);
        assert_eq!(
            v.max_offset(),
            bench::ROWS_1M - u64::from(bench::VIEWPORT_ROWS)
        );
        assert!(v.scroll_by(500_000));
        let slice = v.visible_slice();
        assert_eq!(slice.len(), u64::from(bench::VIEWPORT_ROWS));
        assert_eq!(slice.measure_len(), u64::from(bench::VIEWPORT_ROWS) + 10);
        assert!(slice.overscan_ok());
        // semantic << logical
        assert!(v.semantic_count() < 100);
        assert!(v.semantic_count() >= u64::from(bench::VIEWPORT_ROWS));
        assert_eq!(v.measured_count(), 0);
    }

    #[test]
    fn sticky_leading_in_semantic_not_body() {
        let mut v = Virtualizer::fixed(1)
            .with_len(1000)
            .with_viewport(10)
            .with_sticky(StickyRegion {
                leading: 2,
                trailing: 1,
            });
        v.set_offset(50);
        let body = v.visible_slice();
        assert!(body.start >= 2);
        let mut idx = Vec::new();
        v.semantic_indices(&mut idx);
        assert!(idx.contains(&0) && idx.contains(&1));
        assert!(idx.contains(&999));
        assert!(idx.contains(&50));
        assert!(idx.len() < 30);
    }

    #[test]
    fn variable_sparse_measure_never_allocates_million() {
        let mut v = Virtualizer::variable(2)
            .with_len(bench::ROWS_1M)
            .with_viewport(40)
            .with_overscan(2);
        v.set_offset(10_000);
        let m = v.visible_slice();
        for i in m.measure_start..m.measure_end {
            // Simulate host measuring only the prefetch window.
            v.note_measured(i, if i % 2 == 0 { 1 } else { 3 });
        }
        assert!(v.measured_count() < 100);
        v.forget_measured_outside(0);
        assert!(v.measured_count() <= m.measure_len() as usize + 2);
        let est = v.total_extent_estimate();
        assert!(est > 0);
        assert!(est < u64::MAX / 2);
    }

    #[test]
    fn reveal_and_anchor_content_id() {
        let mut v = Virtualizer::fixed(1).with_len(10_000).with_viewport(20);
        assert!(v.reveal(500));
        assert!(v.offset() <= 500);
        assert!(v.offset() + 20 > 500);
        let a = ScrollAnchor::content_id("row-900");
        v.apply_anchor(&a, |id| if id == "row-900" { Some(900) } else { None });
        assert!(v.offset() <= 900);
        assert!(v.offset() + 20 > 900 || v.offset() == 900);
    }

    #[test]
    fn on_items_changed_clamps() {
        let mut v = Virtualizer::fixed(1).with_len(1000).with_viewport(10);
        v.set_offset(900);
        v.on_items_changed(50);
        assert!(v.offset() <= v.max_offset());
        assert_eq!(v.logical_len(), 50);
    }

    #[test]
    fn virtualizer2d_budget() {
        let mut g = Virtualizer2D::fixed_cells();
        g.set_shape(bench::ROWS_1M, 64);
        g.set_viewport(40, 16);
        g.rows.set_offset(1000);
        g.cols.set_offset(8);
        let budget = g.visible_cells_budget();
        assert_eq!(budget, 40 * 16);
        assert!(g.semantic_count() < 10_000);
    }

    #[test]
    fn fixed_slots_roundtrip() {
        let v = Virtualizer::from_fixed_slots(100, 25, 1000);
        assert_eq!(v.offset(), 100);
        assert_eq!(v.logical_len(), 1000);
        let (off, vp, len) = v.to_fixed_slots();
        assert_eq!(off, 100);
        assert_eq!(vp, 25);
        assert_eq!(len, 1000);
    }

    #[test]
    fn million_ops_perf_budget() {
        let mut v = Virtualizer::fixed(1)
            .with_len(bench::ROWS_1M)
            .with_viewport(40)
            .with_overscan(8);
        let start = std::time::Instant::now();
        for i in 0..50_000 {
            let _ = v.scroll_by(1);
            if i % 100 == 0 {
                let _ = v.visible_slice();
                let _ = v.semantic_count();
                let _ = v.total_extent_estimate();
            }
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 2_000,
            "virtualizer ops too slow: {elapsed:?}"
        );
    }
}

/// Test helper: overscan relationship.
#[cfg(test)]
trait SliceOverscan {
    fn overscan_ok(self) -> bool;
}

#[cfg(test)]
impl SliceOverscan for VirtSlice {
    fn overscan_ok(self) -> bool {
        self.measure_start <= self.start && self.measure_end >= self.end
    }
}
