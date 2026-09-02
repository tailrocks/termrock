// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Terminal grid layout for dashboards, forms, settings, and card collections.
//!
//! **Stateless.** Track templates + placements → rectangles. No retained DOM.
//! Complements [`crate::layout::Stack`] (1D) and [`crate::layout::WorkSurface`]
//! (named app panes). Studio can print [`GridLayout::debug_summary`] for
//! transparent track sizes and overflow flags.
//!
//! ## Tracks
//!
//! - [`TrackSize::Fixed`] / [`TrackSize::content`] — exact / host-measured cells
//! - [`TrackSize::Weight`] / [`TrackSize::fr`] — fractional share of residual
//! - [`TrackSize::MinMax`] — preferred clamped to [min, max]; may grow to max
//!
//! ## Overflow
//!
//! When fixed/minmax minima exceed the axis: active [`OverflowPolicy`] (default
//! shrink-from-end). Spanned cells clip to grid bounds.
//!
//! ## Navigation
//!
//! Optional spatial neighbor helpers for hosts that wire keyboard Move intents.
//! The grid itself is not interactive.
use ratatui_core::layout::Rect;

use crate::interaction::NavigationMove;
use crate::style::DesignSystem;

use super::stack::OverflowPolicy;

/// Soft cap for stack-allocated track scratch (above → heap).
const TRACK_SCRATCH: usize = 64;

/// How one row or column track sizes along its axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TrackSize {
    /// Exact track size in cells.
    Fixed(u16),
    /// Fractional share of residual after fixed/minmax mins (weight ≥ 1).
    Weight(u16),
    /// Preferred size clamped to [min, max].
    MinMax {
        /// Minimum cells.
        min: u16,
        /// Ideal cells.
        preferred: u16,
        /// Maximum cells.
        max: u16,
    },
}

impl TrackSize {
    /// Fixed track.
    #[must_use]
    pub const fn fixed(n: u16) -> Self {
        Self::Fixed(n)
    }

    /// Host-measured intrinsic size (alias of [`Self::Fixed`]; documents intent).
    #[must_use]
    pub const fn content(n: u16) -> Self {
        Self::Fixed(n)
    }

    /// Fractional weight track (`fr` unit).
    #[must_use]
    pub const fn fr(w: u16) -> Self {
        Self::Weight(if w == 0 { 1 } else { w })
    }

    /// Minmax track.
    #[must_use]
    pub const fn minmax(min: u16, preferred: u16, max: u16) -> Self {
        Self::MinMax {
            min,
            preferred,
            max,
        }
    }

    /// Minimum claim before free-space distribution.
    #[must_use]
    pub const fn min_size(self) -> u16 {
        match self {
            Self::Fixed(n) => n,
            Self::Weight(_) => 0,
            Self::MinMax { min, .. } => min,
        }
    }

    /// Ideal claim (fixed / preferred / weight 0).
    #[must_use]
    pub const fn ideal_size(self) -> u16 {
        match self {
            Self::Fixed(n) => n,
            Self::Weight(_) => 0,
            Self::MinMax {
                min,
                preferred,
                max,
            } => {
                let hi = if max < min { min } else { max };
                if preferred < min {
                    min
                } else if preferred > hi {
                    hi
                } else {
                    preferred
                }
            }
        }
    }

    /// Maximum claim when growing (None = unbounded weight).
    #[must_use]
    pub const fn max_size(self) -> Option<u16> {
        match self {
            Self::Fixed(n) => Some(n),
            Self::Weight(_) => None,
            Self::MinMax { min, max, .. } => Some(if max < min { min } else { max }),
        }
    }
}

/// One placed cell (0-based grid coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridItem {
    /// Column start (0-based).
    pub col: u16,
    /// Row start (0-based).
    pub row: u16,
    /// Column span (≥ 1).
    pub col_span: u16,
    /// Row span (≥ 1).
    pub row_span: u16,
}

impl GridItem {
    /// Single cell at (col, row).
    #[must_use]
    pub const fn cell(col: u16, row: u16) -> Self {
        Self {
            col,
            row,
            col_span: 1,
            row_span: 1,
        }
    }

    /// Cell with spans.
    #[must_use]
    pub const fn span(col: u16, row: u16, col_span: u16, row_span: u16) -> Self {
        Self {
            col,
            row,
            col_span: if col_span == 0 { 1 } else { col_span },
            row_span: if row_span == 0 { 1 } else { row_span },
        }
    }
}

/// Auto-flow order when placing a sequence of items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum GridAutoFlow {
    /// Fill columns left→right, then next row (default).
    #[default]
    Row,
    /// Fill rows top→bottom, then next column.
    Column,
}

/// Grid template and gaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridSpec {
    /// Column tracks (left → right).
    pub columns: Vec<TrackSize>,
    /// Row tracks (top → bottom). Empty → auto rows of `auto_row` size.
    pub rows: Vec<TrackSize>,
    /// Gap between columns.
    pub column_gap: u16,
    /// Gap between rows.
    pub row_gap: u16,
    /// Default row track when `rows` is empty and items need more rows.
    pub auto_row: TrackSize,
    /// Padding inside the grid area.
    pub pad_x: u16,
    /// Padding inside the grid area.
    pub pad_y: u16,
    /// Track overflow policy when fixed/minmax exceed the axis.
    pub overflow: OverflowPolicy,
}

impl Default for GridSpec {
    fn default() -> Self {
        Self {
            columns: vec![TrackSize::fr(1)],
            rows: vec![],
            column_gap: 0,
            row_gap: 0,
            auto_row: TrackSize::Fixed(1),
            pad_x: 0,
            pad_y: 0,
            overflow: OverflowPolicy::ShrinkFromEnd,
        }
    }
}

impl GridSpec {
    /// Resolves gaps and padding from the frame design system.
    #[must_use]
    pub fn from_system(system: &DesignSystem) -> Self {
        Self {
            column_gap: system.spacing.gap,
            row_gap: system.spacing.gap,
            pad_x: system.spacing.card_inset,
            pad_y: 1,
            ..Self::default()
        }
    }

    /// N equal fractional columns.
    #[must_use]
    pub fn columns_fr(n: u16) -> Self {
        let n = n.max(1);
        Self {
            columns: (0..n).map(|_| TrackSize::fr(1)).collect(),
            ..Self::default()
        }
    }

    /// Explicit gaps.
    #[must_use]
    pub const fn gaps(mut self, column_gap: u16, row_gap: u16) -> Self {
        self.column_gap = column_gap;
        self.row_gap = row_gap;
        self
    }

    /// Explicit column tracks.
    #[must_use]
    pub fn columns(mut self, columns: impl IntoIterator<Item = TrackSize>) -> Self {
        self.columns = columns.into_iter().collect();
        if self.columns.is_empty() {
            self.columns.push(TrackSize::fr(1));
        }
        self
    }

    /// Explicit row tracks.
    #[must_use]
    pub fn rows(mut self, rows: impl IntoIterator<Item = TrackSize>) -> Self {
        self.rows = rows.into_iter().collect();
        self
    }

    /// Padding.
    #[must_use]
    pub const fn padding(mut self, pad_x: u16, pad_y: u16) -> Self {
        self.pad_x = pad_x;
        self.pad_y = pad_y;
        self
    }

    /// Auto row size when rows are generated.
    #[must_use]
    pub const fn auto_row(mut self, track: TrackSize) -> Self {
        self.auto_row = track;
        self
    }

    /// Overflow policy for both axes.
    #[must_use]
    pub const fn overflow(mut self, policy: OverflowPolicy) -> Self {
        self.overflow = policy;
        self
    }

    /// Column count.
    #[must_use]
    pub fn col_count(&self) -> u16 {
        self.columns.len() as u16
    }
}

/// Resolved grid geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridLayout {
    /// One rect per input item (same order). Zero-sized when clipped out.
    pub cells: Vec<Rect>,
    /// Column track rectangles (full height of content).
    pub column_tracks: Vec<Rect>,
    /// Row track rectangles (full width of content).
    pub row_tracks: Vec<Rect>,
    /// Content area after padding.
    pub content: Rect,
    /// True if any track was shrunk below its preferred/fixed claim.
    pub overflowed: bool,
    /// Column count used.
    pub col_count: u16,
    /// Row count used.
    pub row_count: u16,
    /// Resolved column sizes (cells) for Studio debug.
    pub column_sizes: Vec<u16>,
    /// Resolved row sizes (cells) for Studio debug.
    pub row_sizes: Vec<u16>,
}

impl GridLayout {
    /// Cell count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Empty placement list.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Cell by item index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Rect> {
        self.cells.get(index).copied()
    }

    /// Iterate cells with stable indices.
    pub fn iter(&self) -> impl Iterator<Item = (usize, Rect)> + '_ {
        self.cells.iter().copied().enumerate()
    }

    /// Hit-test: first cell containing the point.
    #[must_use]
    pub fn hit_cell(&self, col: u16, row: u16) -> Option<usize> {
        let pos = ratatui_core::layout::Position { x: col, y: row };
        self.cells
            .iter()
            .position(|r| r.width > 0 && r.height > 0 && r.contains(pos))
    }

    /// Non-empty cell hit regions (id = item index).
    #[must_use]
    pub fn hit_regions(&self) -> Vec<crate::interaction::HitRegion<usize>> {
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, r)| r.width > 0 && r.height > 0)
            .map(|(id, area)| crate::interaction::HitRegion { id, area: *area })
            .collect()
    }

    /// One-line Studio summary: sizes + overflow flag.
    #[must_use]
    pub fn debug_summary(&self) -> String {
        format!(
            "grid {}x{} cols={:?} rows={:?} overflowed={} cells={}",
            self.col_count,
            self.row_count,
            self.column_sizes,
            self.row_sizes,
            self.overflowed,
            self.cells.len()
        )
    }

    /// Register group + non-empty cells into a semantic scene (non-focusable).
    pub fn register_semantic_group<Id, Action, F>(
        &self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        group_id: Id,
        label: &str,
        mut child_id: F,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        F: FnMut(usize) -> Id,
    {
        use crate::interaction::{SemanticNode, SemanticRole};
        let _ = scene.register(
            SemanticNode::content(group_id.clone(), self.content)
                .role(SemanticRole::Content)
                .label(label),
        );
        for (i, rect) in self.cells.iter().enumerate() {
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            let id = child_id(i);
            let _ = scene.register_child(
                group_id.clone(),
                SemanticNode::content(id, *rect)
                    .role(SemanticRole::Content)
                    .label(format!("{label}[{i}]")),
            );
        }
    }
}

/// Builder for common grid layouts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grid {
    spec: GridSpec,
}

impl Grid {
    /// Single full-width column.
    #[must_use]
    pub fn new() -> Self {
        Self {
            spec: GridSpec::default(),
        }
    }

    /// Equal fractional columns.
    #[must_use]
    pub fn columns(n: u16) -> Self {
        Self {
            spec: GridSpec::columns_fr(n),
        }
    }

    /// From a full spec.
    #[must_use]
    pub fn from_spec(spec: GridSpec) -> Self {
        Self { spec }
    }

    /// Gaps.
    #[must_use]
    pub fn gaps(mut self, column_gap: u16, row_gap: u16) -> Self {
        self.spec = self.spec.gaps(column_gap, row_gap);
        self
    }

    /// Column tracks.
    #[must_use]
    pub fn tracks(mut self, columns: impl IntoIterator<Item = TrackSize>) -> Self {
        self.spec = self.spec.columns(columns);
        self
    }

    /// Row tracks.
    #[must_use]
    pub fn row_tracks(mut self, rows: impl IntoIterator<Item = TrackSize>) -> Self {
        self.spec = self.spec.rows(rows);
        self
    }

    /// Auto row height for auto-generated rows.
    #[must_use]
    pub fn auto_row(mut self, track: TrackSize) -> Self {
        self.spec = self.spec.auto_row(track);
        self
    }

    /// Padding.
    #[must_use]
    pub fn padding(mut self, pad_x: u16, pad_y: u16) -> Self {
        self.spec = self.spec.padding(pad_x, pad_y);
        self
    }

    /// Overflow policy.
    #[must_use]
    pub fn overflow(mut self, policy: OverflowPolicy) -> Self {
        self.spec = self.spec.overflow(policy);
        self
    }

    /// Borrow the underlying template.
    #[must_use]
    pub fn spec(&self) -> &GridSpec {
        &self.spec
    }

    /// Layout explicit placements.
    #[must_use]
    pub fn layout(&self, area: Rect, items: &[GridItem]) -> GridLayout {
        layout_grid(area, &self.spec, items)
    }

    /// Auto-flow `count` cells into the grid (row-major by default).
    #[must_use]
    pub fn layout_flow(&self, area: Rect, count: usize, flow: GridAutoFlow) -> GridLayout {
        let items = auto_flow_items(self.spec.col_count(), count, flow);
        layout_grid(area, &self.spec, &items)
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

/// Responsive form/dashboard column template.
///
/// - width ≥ `two_col_min` and ≥ 2× min column + gap → 2 equal columns
/// - else → 1 column
#[must_use]
pub fn responsive_columns(width: u16, two_col_min: u16, min_col: u16, gap: u16) -> GridSpec {
    let fits_two = width >= two_col_min && width >= min_col.saturating_mul(2).saturating_add(gap);
    if fits_two {
        GridSpec::columns_fr(2).gaps(gap, gap)
    } else {
        GridSpec::columns_fr(1).gaps(0, gap)
    }
}

/// Form-oriented template (matches historical Form column policy).
#[must_use]
pub fn form_grid_template(width: u16) -> GridSpec {
    const MIN_COL: u16 = 30;
    const GAP: u16 = 2;
    const TWO_MIN: u16 = 64;
    responsive_columns(width, TWO_MIN, MIN_COL, GAP).auto_row(TrackSize::Fixed(4))
}

/// Dashboard card grid: up to `max_cols` equal columns when width allows.
#[must_use]
pub fn dashboard_grid_template(width: u16, max_cols: u16, min_card: u16, gap: u16) -> GridSpec {
    let max_cols = max_cols.max(1);
    let mut cols = 1u16;
    for c in (2..=max_cols).rev() {
        let need = min_card
            .saturating_mul(c)
            .saturating_add(gap.saturating_mul(c - 1));
        if width >= need {
            cols = c;
            break;
        }
    }
    GridSpec::columns_fr(cols)
        .gaps(gap, gap)
        .auto_row(TrackSize::minmax(3, 6, 12))
}

/// Settings list template: label column (content/fixed) + fractional value column.
///
/// Narrow terminals collapse to a single column of stacked rows.
#[must_use]
pub fn settings_grid_template(width: u16, label_width: u16, gap: u16) -> GridSpec {
    let label_width = label_width.max(8);
    let need = label_width.saturating_add(gap).saturating_add(12);
    if width >= need {
        GridSpec::default()
            .columns([TrackSize::content(label_width), TrackSize::fr(1)])
            .gaps(gap, 0)
            .auto_row(TrackSize::Fixed(1))
    } else {
        GridSpec::columns_fr(1)
            .gaps(0, 0)
            .auto_row(TrackSize::Fixed(2))
    }
}

/// Layout items into `area` using `spec`.
#[must_use]
pub fn layout_grid(area: Rect, spec: &GridSpec, items: &[GridItem]) -> GridLayout {
    let mut scratch = Vec::with_capacity(items.len());
    layout_grid_into(area, spec, items, &mut scratch)
}

/// Layout into a caller-owned scratch buffer (capacity reused; cells live on returned layout).
pub fn layout_grid_into(
    area: Rect,
    spec: &GridSpec,
    items: &[GridItem],
    cells_out: &mut Vec<Rect>,
) -> GridLayout {
    let content = pad_rect(area, spec.pad_x, spec.pad_y);
    let cap = cells_out.capacity().max(items.len());
    cells_out.clear();
    if content.width == 0 || content.height == 0 || items.is_empty() {
        cells_out.extend(std::iter::repeat_n(
            Rect {
                x: content.x,
                y: content.y,
                width: 0,
                height: 0,
            },
            items.len(),
        ));
        let cells = std::mem::take(cells_out);
        *cells_out = Vec::with_capacity(cap);
        return GridLayout {
            cells,
            column_tracks: vec![],
            row_tracks: vec![],
            content,
            overflowed: false,
            col_count: spec.col_count(),
            row_count: 0,
            column_sizes: vec![],
            row_sizes: vec![],
        };
    }

    let col_count = spec.col_count().max(1);
    let max_row = items
        .iter()
        .map(|i| i.row.saturating_add(i.row_span.saturating_sub(1)))
        .max()
        .unwrap_or(0);
    let row_count = {
        let explicit = spec.rows.len() as u16;
        explicit.max(max_row.saturating_add(1))
    };

    let mut row_tracks_spec = spec.rows.clone();
    while (row_tracks_spec.len() as u16) < row_count {
        row_tracks_spec.push(spec.auto_row);
    }

    let (col_sizes, col_overflow) = resolve_tracks(
        main_available(content.width, spec.column_gap, col_count),
        &spec.columns,
        spec.overflow,
    );
    let (row_sizes, row_overflow) = resolve_tracks(
        main_available(content.height, spec.row_gap, row_count),
        &row_tracks_spec,
        spec.overflow,
    );

    let col_offsets = prefix_offsets(&col_sizes, spec.column_gap, content.x);
    let row_offsets = prefix_offsets(&row_sizes, spec.row_gap, content.y);

    let column_tracks: Vec<Rect> = col_sizes
        .iter()
        .enumerate()
        .map(|(i, &w)| Rect {
            x: col_offsets[i],
            y: content.y,
            width: w,
            height: content.height,
        })
        .collect();
    let row_tracks: Vec<Rect> = row_sizes
        .iter()
        .enumerate()
        .map(|(i, &h)| Rect {
            x: content.x,
            y: row_offsets[i],
            width: content.width,
            height: h,
        })
        .collect();

    for item in items {
        cells_out.push(cell_rect(
            item,
            &col_offsets,
            &row_offsets,
            &col_sizes,
            &row_sizes,
            col_count,
            row_count,
            content,
        ));
    }

    let cells = std::mem::take(cells_out);
    *cells_out = Vec::with_capacity(cap);
    GridLayout {
        cells,
        column_tracks,
        row_tracks,
        content,
        overflowed: col_overflow || row_overflow,
        col_count,
        row_count,
        column_sizes: col_sizes,
        row_sizes,
    }
}

/// Generate row-major or column-major placements for `count` items.
#[must_use]
pub fn auto_flow_items(col_count: u16, count: usize, flow: GridAutoFlow) -> Vec<GridItem> {
    let cols = col_count.max(1) as usize;
    (0..count)
        .map(|i| match flow {
            GridAutoFlow::Row => {
                let row = (i / cols) as u16;
                let col = (i % cols) as u16;
                GridItem::cell(col, row)
            }
            GridAutoFlow::Column => {
                let rows = count.div_ceil(cols).max(1);
                GridItem::cell((i / rows) as u16, (i % rows) as u16)
            }
        })
        .collect()
}

/// Spatial neighbor among placed cells (Manhattan on grid coords).
///
/// Returns the index of the nearest item in the move direction, or `None`.
/// Hosts map [`NavigationMove`] from intents; grid does not consume keys.
#[must_use]
pub fn grid_neighbor(items: &[GridItem], focus: usize, direction: NavigationMove) -> Option<usize> {
    match direction {
        NavigationMove::First => items.iter().enumerate().map(|(i, _)| i).min(),
        NavigationMove::Last => items.iter().enumerate().map(|(i, _)| i).max(),
        NavigationMove::Next | NavigationMove::Right => grid_neighbor_2d(items, focus, 1, 0)
            .or_else(|| grid_reading_neighbor(items, focus, true)),
        NavigationMove::Previous | NavigationMove::Left => grid_neighbor_2d(items, focus, -1, 0)
            .or_else(|| grid_reading_neighbor(items, focus, false)),
        NavigationMove::Down => grid_neighbor_2d(items, focus, 0, 1),
        NavigationMove::Up => grid_neighbor_2d(items, focus, 0, -1),
    }
}

/// 2D spatial neighbor: `(dx, dy)` in grid cell steps (e.g. (0,-1) = up).
#[must_use]
pub fn grid_neighbor_2d(items: &[GridItem], focus: usize, dx: i32, dy: i32) -> Option<usize> {
    if dx == 0 && dy == 0 {
        return Some(focus);
    }
    let cur = items.get(focus)?;
    let cx = i32::from(cur.col);
    let cy = i32::from(cur.row);
    let mut best: Option<(usize, i32, i32)> = None;
    for (i, item) in items.iter().enumerate() {
        if i == focus {
            continue;
        }
        let ix = i32::from(item.col);
        let iy = i32::from(item.row);
        let ddx = ix - cx;
        let ddy = iy - cy;
        if dx != 0 && ddx.signum() != dx.signum() {
            continue;
        }
        if dy != 0 && ddy.signum() != dy.signum() {
            continue;
        }
        if dx != 0 && ddx == 0 {
            continue;
        }
        if dy != 0 && ddy == 0 {
            continue;
        }
        let primary = if dx != 0 { ddx.abs() } else { ddy.abs() };
        let secondary = if dx != 0 { ddy.abs() } else { ddx.abs() };
        let better = match best {
            None => true,
            Some((_, bp, bs)) => primary < bp || (primary == bp && secondary < bs),
        };
        if better {
            best = Some((i, primary, secondary));
        }
    }
    best.map(|(i, _, _)| i)
}

/// Reading-order next/previous (row-major by (row, col)).
#[must_use]
pub fn grid_reading_neighbor(items: &[GridItem], focus: usize, forward: bool) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (items[i].row, items[i].col));
    let pos = order.iter().position(|&i| i == focus)?;
    if forward {
        order.get(pos + 1).copied()
    } else if pos > 0 {
        order.get(pos - 1).copied()
    } else {
        None
    }
}

// ── internals ──────────────────────────────────────────────────────────────

fn pad_rect(area: Rect, pad_x: u16, pad_y: u16) -> Rect {
    let x = area.x.saturating_add(pad_x);
    let y = area.y.saturating_add(pad_y);
    let width = area.width.saturating_sub(pad_x.saturating_mul(2));
    let height = area.height.saturating_sub(pad_y.saturating_mul(2));
    if width == 0 || height == 0 {
        Rect {
            x,
            y,
            width: 0,
            height: 0,
        }
    } else {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

fn main_available(total: u16, gap: u16, count: u16) -> u16 {
    let gaps = gap.saturating_mul(count.saturating_sub(1));
    total.saturating_sub(gaps)
}

/// Resolve track sizes along one axis. Returns (sizes, overflowed).
fn resolve_tracks(
    available: u16,
    tracks: &[TrackSize],
    overflow: OverflowPolicy,
) -> (Vec<u16>, bool) {
    let n = tracks.len();
    if n == 0 {
        return (vec![], false);
    }

    let mut scratch = [0u16; TRACK_SCRATCH];
    let mut heap = Vec::new();
    let sizes: &mut [u16] = if n <= TRACK_SCRATCH {
        &mut scratch[..n]
    } else {
        heap.resize(n, 0);
        heap.as_mut_slice()
    };

    let mut weight_sum = 0u32;
    let mut fixed_sum = 0u16;

    for (i, track) in tracks.iter().enumerate() {
        match *track {
            TrackSize::Fixed(f) => {
                sizes[i] = f;
                fixed_sum = fixed_sum.saturating_add(f);
            }
            TrackSize::MinMax {
                min,
                preferred,
                max,
            } => {
                let p = preferred.clamp(min, max.max(min));
                sizes[i] = p;
                fixed_sum = fixed_sum.saturating_add(p);
            }
            TrackSize::Weight(w) => {
                weight_sum = weight_sum.saturating_add(u32::from(w.max(1)));
            }
        }
    }

    let mut overflowed = false;
    if fixed_sum > available {
        overflowed = true;
        apply_track_overflow(overflow, tracks, sizes, available);
    } else {
        let mut residual = available.saturating_sub(fixed_sum);
        if weight_sum > 0 {
            let mut rem_w = weight_sum;
            let mut rem_flex = residual;
            for i in 0..n {
                if let TrackSize::Weight(w) = tracks[i] {
                    let w = u32::from(w.max(1));
                    let is_last = tracks[i + 1..]
                        .iter()
                        .all(|t| !matches!(t, TrackSize::Weight(_)));
                    let share = if rem_w == 0 {
                        0
                    } else {
                        (u32::from(rem_flex) * w / rem_w) as u16
                    };
                    let size = if is_last { rem_flex } else { share };
                    sizes[i] = size;
                    rem_w = rem_w.saturating_sub(w);
                    rem_flex = rem_flex.saturating_sub(size);
                }
            }
            residual = rem_flex;
        }
        if residual > 0 {
            grow_minmax(tracks, sizes, &mut residual);
        }
    }

    (sizes.to_vec(), overflowed)
}

fn apply_track_overflow(
    policy: OverflowPolicy,
    tracks: &[TrackSize],
    sizes: &mut [u16],
    available: u16,
) {
    let n = tracks.len();
    match policy {
        OverflowPolicy::ShrinkFromEnd => {
            let mut remaining = available;
            for i in (0..n).rev() {
                if matches!(tracks[i], TrackSize::Weight(_)) {
                    sizes[i] = 0;
                    continue;
                }
                let take = sizes[i].min(remaining);
                sizes[i] = take;
                remaining = remaining.saturating_sub(take);
            }
        }
        OverflowPolicy::ClipTail => {
            let mut remaining = available;
            for i in 0..n {
                if matches!(tracks[i], TrackSize::Weight(_)) {
                    sizes[i] = 0;
                    continue;
                }
                let take = sizes[i].min(remaining);
                sizes[i] = take;
                remaining = remaining.saturating_sub(take);
            }
        }
        OverflowPolicy::EqualShare => {
            for i in 0..n {
                if matches!(tracks[i], TrackSize::Weight(_)) {
                    sizes[i] = 0;
                }
            }
            loop {
                let sum: u16 = sizes.iter().copied().sum();
                if sum <= available {
                    break;
                }
                let mut peeled = false;
                for i in (0..n).rev() {
                    if sizes[i] > 0 {
                        sizes[i] -= 1;
                        peeled = true;
                        break;
                    }
                }
                if !peeled {
                    break;
                }
            }
        }
    }
}

fn grow_minmax(tracks: &[TrackSize], sizes: &mut [u16], residual: &mut u16) {
    while *residual > 0 {
        let mut grew = false;
        for (i, track) in tracks.iter().enumerate() {
            if *residual == 0 {
                break;
            }
            if let TrackSize::MinMax { min, max, .. } = *track {
                let hi = max.max(min);
                if sizes[i] < hi {
                    sizes[i] += 1;
                    *residual -= 1;
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }
}

fn prefix_offsets(sizes: &[u16], gap: u16, origin: u16) -> Vec<u16> {
    let mut out = Vec::with_capacity(sizes.len());
    let mut cursor = origin;
    for (i, &s) in sizes.iter().enumerate() {
        out.push(cursor);
        cursor = cursor.saturating_add(s);
        if i + 1 < sizes.len() {
            cursor = cursor.saturating_add(gap);
        }
    }
    out
}

#[expect(clippy::too_many_arguments, reason = "pure geometry helper")]
fn cell_rect(
    item: &GridItem,
    col_offsets: &[u16],
    row_offsets: &[u16],
    col_sizes: &[u16],
    row_sizes: &[u16],
    col_count: u16,
    row_count: u16,
    content: Rect,
) -> Rect {
    let col = item.col;
    let row = item.row;
    if col >= col_count || row >= row_count {
        return Rect {
            x: content.x,
            y: content.y,
            width: 0,
            height: 0,
        };
    }
    let col_end = (col.saturating_add(item.col_span)).min(col_count);
    let row_end = (row.saturating_add(item.row_span)).min(row_count);
    let x = col_offsets[col as usize];
    let y = row_offsets[row as usize];
    let right = if col_end == 0 {
        x
    } else {
        let last = (col_end - 1) as usize;
        col_offsets[last].saturating_add(col_sizes[last])
    };
    let bottom = if row_end == 0 {
        y
    } else {
        let last = (row_end - 1) as usize;
        row_offsets[last].saturating_add(row_sizes[last])
    };
    let width = right.saturating_sub(x);
    let height = bottom.saturating_sub(y);
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_column_equal_fr() {
        let grid = Grid::columns(2).gaps(2, 0);
        let layout = grid.layout_flow(Rect::new(0, 0, 42, 10), 2, GridAutoFlow::Row);
        assert_eq!(layout.col_count, 2);
        assert_eq!(layout.cells[0].width, 20);
        assert_eq!(layout.cells[1].width, 20);
        assert_eq!(layout.cells[1].x, 22);
        assert_eq!(layout.column_sizes, vec![20, 20]);
    }

    #[test]
    fn span_merges_columns() {
        let spec = GridSpec::columns_fr(3)
            .gaps(1, 1)
            .rows([TrackSize::Fixed(3), TrackSize::Fixed(3)]);
        let items = [
            GridItem::span(0, 0, 2, 1),
            GridItem::cell(2, 0),
            GridItem::span(0, 1, 3, 1),
        ];
        let layout = layout_grid(Rect::new(0, 0, 32, 10), &spec, &items);
        assert!(layout.cells[0].width > layout.cells[1].width);
        assert_eq!(layout.cells[2].width, layout.content.width);
    }

    #[test]
    fn overflow_shrinks_fixed_tracks() {
        let spec = GridSpec::default().columns([
            TrackSize::Fixed(10),
            TrackSize::Fixed(10),
            TrackSize::Fixed(10),
        ]);
        let items = [
            GridItem::cell(0, 0),
            GridItem::cell(1, 0),
            GridItem::cell(2, 0),
        ];
        let layout = layout_grid(Rect::new(0, 0, 15, 5), &spec, &items);
        assert!(layout.overflowed);
        let sum: u16 = layout.column_sizes.iter().sum();
        assert_eq!(sum, 15);
        // shrink-from-end: last keeps 10, middle 5, first 0
        assert_eq!(layout.column_sizes[2], 10);
        assert_eq!(layout.column_sizes[1], 5);
        assert_eq!(layout.column_sizes[0], 0);
    }

    #[test]
    fn overflow_clip_tail_keeps_head() {
        let spec = GridSpec::default()
            .overflow(OverflowPolicy::ClipTail)
            .columns([
                TrackSize::Fixed(10),
                TrackSize::Fixed(10),
                TrackSize::Fixed(10),
            ]);
        let items = [
            GridItem::cell(0, 0),
            GridItem::cell(1, 0),
            GridItem::cell(2, 0),
        ];
        let layout = layout_grid(Rect::new(0, 0, 15, 5), &spec, &items);
        assert!(layout.overflowed);
        assert_eq!(layout.column_sizes[0], 10);
        assert_eq!(layout.column_sizes[1], 5);
        assert_eq!(layout.column_sizes[2], 0);
    }

    #[test]
    fn auto_flow_row_major() {
        let items = auto_flow_items(2, 4, GridAutoFlow::Row);
        assert_eq!(items[0], GridItem::cell(0, 0));
        assert_eq!(items[1], GridItem::cell(1, 0));
        assert_eq!(items[2], GridItem::cell(0, 1));
        assert_eq!(items[3], GridItem::cell(1, 1));
    }

    #[test]
    fn form_template_collapses_narrow() {
        let wide = form_grid_template(80);
        assert_eq!(wide.col_count(), 2);
        let narrow = form_grid_template(40);
        assert_eq!(narrow.col_count(), 1);
    }

    #[test]
    fn dashboard_template_max_cols() {
        let g = dashboard_grid_template(100, 3, 20, 2);
        assert_eq!(g.col_count(), 3);
        let tiny = dashboard_grid_template(25, 3, 20, 2);
        assert_eq!(tiny.col_count(), 1);
    }

    #[test]
    fn settings_template_two_or_one() {
        let wide = settings_grid_template(80, 18, 2);
        assert_eq!(wide.col_count(), 2);
        assert!(matches!(wide.columns[0], TrackSize::Fixed(18)));
        let narrow = settings_grid_template(20, 18, 2);
        assert_eq!(narrow.col_count(), 1);
    }

    #[test]
    fn neighbor_right() {
        let items = auto_flow_items(3, 6, GridAutoFlow::Row);
        assert_eq!(grid_neighbor_2d(&items, 0, 1, 0), Some(1));
        assert_eq!(grid_neighbor_2d(&items, 0, 0, 1), Some(3));
        assert_eq!(grid_neighbor(&items, 0, NavigationMove::Right), Some(1));
    }

    #[test]
    fn reading_order() {
        let items = [
            GridItem::cell(1, 0),
            GridItem::cell(0, 0),
            GridItem::cell(0, 1),
        ];
        assert_eq!(grid_reading_neighbor(&items, 1, true), Some(0));
    }

    #[test]
    fn hit_cell_and_regions() {
        let layout = Grid::columns(2)
            .gaps(0, 0)
            .auto_row(TrackSize::Fixed(4))
            .layout_flow(Rect::new(0, 0, 20, 4), 2, GridAutoFlow::Row);
        assert_eq!(layout.hit_cell(15, 1), Some(1));
        assert_eq!(layout.hit_cell(2, 1), Some(0));
        assert_eq!(layout.hit_regions().len(), 2);
    }

    #[test]
    fn debug_summary_lists_sizes() {
        let layout =
            Grid::columns(2)
                .gaps(0, 0)
                .layout_flow(Rect::new(0, 0, 20, 4), 2, GridAutoFlow::Row);
        let s = layout.debug_summary();
        assert!(s.contains("cols="));
        assert!(s.contains("overflowed=false"));
    }

    #[test]
    fn layout_is_cheap() {
        let grid = Grid::columns(4).gaps(1, 1).auto_row(TrackSize::Fixed(3));
        let area = Rect::new(0, 0, 120, 40);
        let mut buf = Vec::with_capacity(48);
        for _ in 0..10_000 {
            let _ = layout_grid_into(
                area,
                grid.spec(),
                &auto_flow_items(4, 32, GridAutoFlow::Row),
                &mut buf,
            );
        }
    }

    #[test]
    fn large_realistic_dashboard_bench() {
        // 6×8 card wall — realistic dense dashboard.
        let grid = Grid::columns(6)
            .gaps(1, 1)
            .auto_row(TrackSize::minmax(3, 5, 8));
        let area = Rect::new(0, 0, 160, 48);
        let items = auto_flow_items(6, 48, GridAutoFlow::Row);
        let mut buf = Vec::with_capacity(48);
        for _ in 0..5_000 {
            let layout = layout_grid_into(area, grid.spec(), &items, &mut buf);
            assert_eq!(layout.cells.len(), 48);
        }
    }

    #[test]
    fn minmax_clamps_and_grows() {
        let spec = GridSpec::default()
            .columns([TrackSize::minmax(5, 100, 10), TrackSize::fr(1)])
            .rows([TrackSize::Fixed(4)]);
        let layout = layout_grid(
            Rect::new(0, 0, 40, 4),
            &spec,
            &[GridItem::cell(0, 0), GridItem::cell(1, 0)],
        );
        assert_eq!(layout.column_tracks[0].width, 10);
        // Only minmax: grows to max with residual
        let grow = layout_grid(
            Rect::new(0, 0, 20, 4),
            &GridSpec::default()
                .columns([TrackSize::minmax(2, 2, 12)])
                .rows([TrackSize::Fixed(4)]),
            &[GridItem::cell(0, 0)],
        );
        assert_eq!(grow.column_sizes[0], 12);
    }

    #[test]
    fn content_track_is_fixed() {
        assert_eq!(TrackSize::content(14), TrackSize::Fixed(14));
    }

    #[test]
    fn semantic_group_registers() {
        use crate::interaction::SemanticScene;
        let layout = Grid::columns(2).auto_row(TrackSize::Fixed(2)).layout_flow(
            Rect::new(0, 0, 20, 4),
            2,
            GridAutoFlow::Row,
        );
        let mut scene = SemanticScene::<String, ()>::new();
        scene.begin_frame();
        layout.register_semantic_group(&mut scene, "g".into(), "grid", |i| format!("c{i}"));
        assert!(scene.len() >= 2);
    }
}
