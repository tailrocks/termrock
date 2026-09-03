// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Stack and Inline: cell-accurate flex-ish packing for component anatomy.
//!
//! **Stateless.** Pure functions over [`Rect`] + child size specs — no retained
//! layout tree. Output is one rect per child (stable index). Prefer
//! [`layout_stack_into`] to reuse a buffer and avoid per-frame allocation of
//! the result `Vec` (internal size scratch uses a stack buffer for ≤64 children).
//!
//! - [`Stack`] — vertical main axis (default)
//! - [`Inline`] — horizontal main axis (wrap optional)
//!
//! Cross-axis alignment, main-axis justify, density gaps, overflow policy, and
//! responsive direction are explicit. When children exceed the main axis the
//! active [`OverflowPolicy`] decides shrink/clip behavior; `StackLayout::overflowed`
//! is set for Studio and hosts.
use ratatui_core::layout::Rect;

/// Soft cap for stack-allocated main-size scratch (above → heap `Vec`).
const INLINE_SCRATCH: usize = 64;

/// Main axis direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StackDirection {
    /// Top → bottom (Stack).
    #[default]
    Vertical,
    /// Left → right (Inline).
    Horizontal,
}

/// Cross-axis alignment (perpendicular to main).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Align {
    /// Pack toward start of cross axis.
    #[default]
    Start,
    /// Center on cross axis.
    Center,
    /// Pack toward end of cross axis.
    End,
    /// Stretch to full cross-axis size (default for most chrome).
    Stretch,
}

/// Main-axis distribution of free space after fixed/preferred children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Justify {
    /// Pack toward start; free space at end.
    #[default]
    Start,
    /// Center the group.
    Center,
    /// Pack toward end; free space at start.
    End,
    /// Equal free space between children (not before/after).
    SpaceBetween,
    /// Equal free space around each child (half-gap at ends).
    SpaceAround,
    /// Equal free space before, between, and after children.
    SpaceEvenly,
}

/// How one child claims main-axis cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FlexSize {
    /// Exact main-axis size (clamped under overflow).
    Fixed(u16),
    /// Share of residual after fixed/preferred mins (weight ≥ 1).
    Weight(u16),
    /// Preferred size clamped to [min, max]; may grow toward max when free residual.
    Preferred {
        /// Minimum cells.
        min: u16,
        /// Ideal cells.
        preferred: u16,
        /// Maximum cells.
        max: u16,
    },
    /// Collapsed (zero main-axis; still present for stable indexing).
    Collapsed,
}

impl FlexSize {
    /// Fixed helper.
    #[must_use]
    pub const fn fixed(n: u16) -> Self {
        Self::Fixed(n)
    }

    /// Fill residual equally (weight 1).
    #[must_use]
    pub const fn fill() -> Self {
        Self::Weight(1)
    }

    /// Preferred helper.
    #[must_use]
    pub const fn preferred(min: u16, preferred: u16, max: u16) -> Self {
        Self::Preferred {
            min,
            preferred,
            max,
        }
    }

    /// Minimum main-axis claim before free-space distribution.
    #[must_use]
    pub const fn min_main(self) -> u16 {
        match self {
            Self::Fixed(n) => n,
            Self::Weight(_) => 0,
            Self::Preferred { min, .. } => min,
            Self::Collapsed => 0,
        }
    }
}

/// Behavior when fixed/preferred children exceed the main axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OverflowPolicy {
    /// Shrink fixed/preferred from the **end** until they fit (default).
    #[default]
    ShrinkFromEnd,
    /// Keep earlier children at claim; later children collapse to zero.
    ClipTail,
    /// Reduce fixed/preferred toward min, from the end, one cell at a time.
    EqualShare,
}

/// Layout knobs for [`layout_stack`] / builders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackSpec {
    /// Main axis.
    pub direction: StackDirection,
    /// Gap between children (cells).
    pub gap: u16,
    /// Cross-axis alignment.
    pub align: Align,
    /// Main-axis free-space distribution.
    pub justify: Justify,
    /// When true and direction is horizontal, wrap to next row.
    pub wrap: bool,
    /// Inner padding before packing children.
    pub pad_x: u16,
    /// Inner padding before packing children.
    pub pad_y: u16,
    /// Overflow policy when fixed/preferred exceed main axis.
    pub overflow: OverflowPolicy,
}

impl Default for StackSpec {
    fn default() -> Self {
        Self {
            direction: StackDirection::Vertical,
            gap: 0,
            align: Align::Stretch,
            justify: Justify::Start,
            wrap: false,
            pad_x: 0,
            pad_y: 0,
            overflow: OverflowPolicy::ShrinkFromEnd,
        }
    }
}

impl StackSpec {
    /// Vertical stack defaults.
    #[must_use]
    pub const fn vertical() -> Self {
        Self {
            direction: StackDirection::Vertical,
            gap: 0,
            align: Align::Stretch,
            justify: Justify::Start,
            wrap: false,
            pad_x: 0,
            pad_y: 0,
            overflow: OverflowPolicy::ShrinkFromEnd,
        }
    }

    /// Horizontal inline defaults.
    #[must_use]
    pub const fn horizontal() -> Self {
        Self {
            direction: StackDirection::Horizontal,
            gap: 0,
            align: Align::Stretch,
            justify: Justify::Start,
            wrap: false,
            pad_x: 0,
            pad_y: 0,
            overflow: OverflowPolicy::ShrinkFromEnd,
        }
    }
    /// Responsive direction from outer width.
    #[must_use]
    pub const fn responsive(mut self, width: u16, inline_min_width: u16) -> Self {
        self.direction = direction_for_width(width, inline_min_width);
        self
    }
}

/// Resolved layout: one rect per child (same order), plus hit/overflow metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackLayout {
    /// Child rectangles (stable index = input index). Zero-sized when collapsed/overflowed.
    pub children: Vec<Rect>,
    /// Content area after padding (group hit target).
    pub content: Rect,
    /// True when at least one child was reduced below its preferred/fixed claim.
    pub overflowed: bool,
    /// Sum of main-axis sizes + gaps before padding (for scroll hosts).
    pub content_main: u16,
    /// Direction used.
    pub direction: StackDirection,
}

impl StackLayout {
    /// Child count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.children.len()
    }

    /// Child rect by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Rect> {
        self.children.get(index).copied()
    }

    /// Iterate child rects with stable indices.
    pub fn iter(&self) -> impl Iterator<Item = (usize, Rect)> + '_ {
        self.children.iter().copied().enumerate()
    }

    /// Hit-test: first child containing the cell, if any.
    #[must_use]
    pub fn hit_child(&self, col: u16, row: u16) -> Option<usize> {
        let pos = ratatui_core::layout::Position { x: col, y: row };
        self.children
            .iter()
            .position(|r| r.width > 0 && r.height > 0 && r.contains(pos))
    }
    /// Project non-empty children into hit regions (id = stable index).
    #[must_use]
    pub fn hit_regions(&self) -> Vec<crate::interaction::HitRegion<usize>> {
        self.children
            .iter()
            .enumerate()
            .filter(|(_, r)| r.width > 0 && r.height > 0)
            .map(|(id, area)| crate::interaction::HitRegion { id, area: *area })
            .collect()
    }

    /// Register this group + non-empty children into a semantic scene.
    ///
    /// Group is non-focusable content; children are non-focusable content nodes
    /// (hosts re-register interactive descendants with real ids).
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
        for (i, rect) in self.children.iter().enumerate() {
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

/// Vertical stack builder (ergonomic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    spec: StackSpec,
}

impl Stack {
    /// Vertical stack, density gap only (no pad).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            spec: StackSpec::vertical(),
        }
    }

    /// Main-axis direction (for responsive flip without rebuilding).
    #[must_use]
    pub const fn direction(mut self, direction: StackDirection) -> Self {
        self.spec.direction = direction;
        self
    }

    /// Responsive direction from outer width.
    #[must_use]
    pub const fn responsive(mut self, width: u16, inline_min_width: u16) -> Self {
        self.spec = self.spec.responsive(width, inline_min_width);
        self
    }

    /// Gap override.
    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.spec.gap = gap;
        self
    }

    /// Cross-axis align.
    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.spec.align = align;
        self
    }

    /// Main-axis justify.
    #[must_use]
    pub const fn justify(mut self, justify: Justify) -> Self {
        self.spec.justify = justify;
        self
    }

    /// Overflow policy.
    #[must_use]
    pub const fn overflow(mut self, policy: OverflowPolicy) -> Self {
        self.spec.overflow = policy;
        self
    }

    /// Padding.
    #[must_use]
    pub const fn padding(mut self, pad_x: u16, pad_y: u16) -> Self {
        self.spec.pad_x = pad_x;
        self.spec.pad_y = pad_y;
        self
    }

    /// Layout children in `area` (stretch cross-axis).
    #[must_use]
    pub fn layout(self, area: Rect, children: &[FlexSize]) -> StackLayout {
        layout_stack(area, &self.spec, children)
    }

    /// Layout with per-child cross-axis preferred sizes (ignored under [`Align::Stretch`]).
    #[must_use]
    pub fn layout_with_cross(
        self,
        area: Rect,
        children: &[FlexSize],
        cross: &[u16],
    ) -> StackLayout {
        layout_stack_with_cross(area, &self.spec, children, Some(cross))
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

/// Horizontal inline builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inline {
    spec: StackSpec,
}

impl Inline {
    /// Horizontal, no wrap.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            spec: StackSpec::horizontal(),
        }
    }

    /// Main-axis direction.
    #[must_use]
    pub const fn direction(mut self, direction: StackDirection) -> Self {
        self.spec.direction = direction;
        self
    }

    /// Responsive direction from outer width.
    #[must_use]
    pub const fn responsive(mut self, width: u16, inline_min_width: u16) -> Self {
        self.spec = self.spec.responsive(width, inline_min_width);
        self
    }

    /// Enable wrapping (fixed/preferred sizes only; weights treated as preferred=1).
    #[must_use]
    pub const fn wrap(mut self, wrap: bool) -> Self {
        self.spec.wrap = wrap;
        self
    }

    /// Gap.
    #[must_use]
    pub const fn gap(mut self, gap: u16) -> Self {
        self.spec.gap = gap;
        self
    }

    /// Cross-axis align (vertical alignment in a row).
    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.spec.align = align;
        self
    }

    /// Main-axis justify.
    #[must_use]
    pub const fn justify(mut self, justify: Justify) -> Self {
        self.spec.justify = justify;
        self
    }

    /// Overflow policy.
    #[must_use]
    pub const fn overflow(mut self, policy: OverflowPolicy) -> Self {
        self.spec.overflow = policy;
        self
    }

    /// Padding.
    #[must_use]
    pub const fn padding(mut self, pad_x: u16, pad_y: u16) -> Self {
        self.spec.pad_x = pad_x;
        self.spec.pad_y = pad_y;
        self
    }

    /// Layout children in `area`.
    #[must_use]
    pub fn layout(self, area: Rect, children: &[FlexSize]) -> StackLayout {
        layout_stack(area, &self.spec, children)
    }

    /// Layout with per-child cross-axis preferred sizes.
    #[must_use]
    pub fn layout_with_cross(
        self,
        area: Rect,
        children: &[FlexSize],
        cross: &[u16],
    ) -> StackLayout {
        layout_stack_with_cross(area, &self.spec, children, Some(cross))
    }
}

impl Default for Inline {
    fn default() -> Self {
        Self::new()
    }
}

/// Responsive direction helper: stack when narrow, inline when wide.
#[must_use]
pub const fn direction_for_width(width: u16, inline_min_width: u16) -> StackDirection {
    if width >= inline_min_width {
        StackDirection::Horizontal
    } else {
        StackDirection::Vertical
    }
}

/// Layout children into `area` using `spec`.
#[must_use]
pub fn layout_stack(area: Rect, spec: &StackSpec, children: &[FlexSize]) -> StackLayout {
    layout_stack_with_cross(area, spec, children, None)
}

/// Layout with optional per-child cross-axis sizes (cells).
#[must_use]
pub fn layout_stack_with_cross(
    area: Rect,
    spec: &StackSpec,
    children: &[FlexSize],
    cross: Option<&[u16]>,
) -> StackLayout {
    let mut out = Vec::with_capacity(children.len());
    layout_stack_into_cross(area, spec, children, cross, &mut out);
    let content = padded_content(area, spec);
    let content_main = estimate_content_main(spec, children, &out);
    let overflowed = out.iter().zip(children.iter()).any(|(r, c)| {
        let main = match spec.direction {
            StackDirection::Vertical => r.height,
            StackDirection::Horizontal => r.width,
        };
        main < c.min_main() && !matches!(c, FlexSize::Collapsed | FlexSize::Weight(_))
    });
    StackLayout {
        children: out,
        content,
        overflowed,
        content_main,
        direction: spec.direction,
    }
}

/// Layout into a caller-owned buffer (avoids alloc when reusing capacity).
pub fn layout_stack_into(area: Rect, spec: &StackSpec, children: &[FlexSize], out: &mut Vec<Rect>) {
    layout_stack_into_cross(area, spec, children, None, out);
}

/// Layout into caller buffer with optional cross-axis sizes.
pub fn layout_stack_into_cross(
    area: Rect,
    spec: &StackSpec,
    children: &[FlexSize],
    cross: Option<&[u16]>,
    out: &mut Vec<Rect>,
) {
    out.clear();
    if children.is_empty() {
        return;
    }
    let content = padded_content(area, spec);
    if content.width == 0 || content.height == 0 {
        out.extend(std::iter::repeat_n(
            Rect {
                x: content.x,
                y: content.y,
                width: 0,
                height: 0,
            },
            children.len(),
        ));
        return;
    }

    if spec.wrap && matches!(spec.direction, StackDirection::Horizontal) {
        layout_wrap_horizontal(content, spec, children, out);
        return;
    }

    layout_single_line(content, spec, children, cross, out);
}

fn padded_content(area: Rect, spec: &StackSpec) -> Rect {
    let x = area.x.saturating_add(spec.pad_x);
    let y = area.y.saturating_add(spec.pad_y);
    let width = area.width.saturating_sub(spec.pad_x.saturating_mul(2));
    let height = area.height.saturating_sub(spec.pad_y.saturating_mul(2));
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

fn main_len(area: Rect, direction: StackDirection) -> u16 {
    match direction {
        StackDirection::Vertical => area.height,
        StackDirection::Horizontal => area.width,
    }
}

fn cross_len(area: Rect, direction: StackDirection) -> u16 {
    match direction {
        StackDirection::Vertical => area.width,
        StackDirection::Horizontal => area.height,
    }
}

fn layout_single_line(
    content: Rect,
    spec: &StackSpec,
    children: &[FlexSize],
    cross: Option<&[u16]>,
    out: &mut Vec<Rect>,
) {
    let main_total = main_len(content, spec.direction);
    let cross_total = cross_len(content, spec.direction);
    let n = children.len();
    let gaps = spec.gap.saturating_mul(n.saturating_sub(1) as u16);

    let mut scratch = [0u16; INLINE_SCRATCH];
    let mut heap: Vec<u16> = Vec::new();
    let main_sizes: &mut [u16] = if n <= INLINE_SCRATCH {
        &mut scratch[..n]
    } else {
        heap.resize(n, 0);
        heap.as_mut_slice()
    };

    let mut weight_sum = 0u32;
    let mut fixed_sum = 0u16;
    for (i, child) in children.iter().enumerate() {
        match *child {
            FlexSize::Collapsed => main_sizes[i] = 0,
            FlexSize::Fixed(f) => {
                main_sizes[i] = f;
                fixed_sum = fixed_sum.saturating_add(f);
            }
            FlexSize::Preferred {
                min,
                preferred,
                max,
            } => {
                let hi = max.max(min);
                let p = preferred.clamp(min, hi);
                main_sizes[i] = p;
                fixed_sum = fixed_sum.saturating_add(p);
            }
            FlexSize::Weight(w) => {
                weight_sum = weight_sum.saturating_add(u32::from(w.max(1)));
            }
        }
    }

    let available = main_total.saturating_sub(gaps);
    let mut overflowed = false;

    if fixed_sum > available {
        overflowed = true;
        apply_overflow(spec.overflow, children, main_sizes, available);
    } else {
        let mut residual = available.saturating_sub(fixed_sum);
        if weight_sum > 0 {
            let mut rem_w = weight_sum;
            let mut rem_flex = residual;
            for i in 0..n {
                if let FlexSize::Weight(w) = children[i] {
                    let w = u32::from(w.max(1));
                    let share = if rem_w == 0 {
                        0
                    } else {
                        (u32::from(rem_flex) * w / rem_w) as u16
                    };
                    let is_last_weight = children[i + 1..]
                        .iter()
                        .all(|c| !matches!(c, FlexSize::Weight(_)));
                    let size = if is_last_weight { rem_flex } else { share };
                    main_sizes[i] = size;
                    rem_w = rem_w.saturating_sub(w);
                    rem_flex = rem_flex.saturating_sub(size);
                }
            }
            residual = rem_flex;
        }
        // Grow preferred toward max with leftover residual (after weights).
        if residual > 0 {
            grow_preferred(children, main_sizes, &mut residual);
        }
    }

    let used_main: u16 = main_sizes.iter().copied().sum::<u16>().saturating_add(gaps);
    let free = main_total.saturating_sub(used_main);
    let (leading, between_extra) = justify_offsets(spec.justify, free, n, overflowed);

    let mut cursor = match spec.direction {
        StackDirection::Vertical => content.y.saturating_add(leading),
        StackDirection::Horizontal => content.x.saturating_add(leading),
    };

    out.reserve(n);
    for i in 0..n {
        let main = main_sizes[i];
        let child_cross = cross.and_then(|c| c.get(i).copied()).unwrap_or(cross_total);
        let (cross_off, cross_size) = cross_place(spec.align, cross_total, child_cross);
        let rect = match spec.direction {
            StackDirection::Vertical => Rect {
                x: content.x.saturating_add(cross_off),
                y: cursor,
                width: cross_size,
                height: main,
            },
            StackDirection::Horizontal => Rect {
                x: cursor,
                y: content.y.saturating_add(cross_off),
                width: main,
                height: cross_size,
            },
        };
        out.push(rect);
        cursor = cursor.saturating_add(main);
        if i + 1 < n {
            cursor = cursor
                .saturating_add(spec.gap)
                .saturating_add(between_extra);
        }
    }
}

fn apply_overflow(
    policy: OverflowPolicy,
    children: &[FlexSize],
    main_sizes: &mut [u16],
    available: u16,
) {
    let n = children.len();
    match policy {
        OverflowPolicy::ShrinkFromEnd => {
            let mut remaining = available;
            for i in (0..n).rev() {
                if matches!(children[i], FlexSize::Weight(_)) {
                    main_sizes[i] = 0;
                    continue;
                }
                let take = main_sizes[i].min(remaining);
                main_sizes[i] = take;
                remaining = remaining.saturating_sub(take);
            }
        }
        OverflowPolicy::ClipTail => {
            let mut remaining = available;
            for i in 0..n {
                if matches!(children[i], FlexSize::Weight(_) | FlexSize::Collapsed) {
                    main_sizes[i] = 0;
                    continue;
                }
                let take = main_sizes[i].min(remaining);
                main_sizes[i] = take;
                remaining = remaining.saturating_sub(take);
            }
        }
        OverflowPolicy::EqualShare => {
            // Zero weights first; then peel cells from the end while sum > available.
            for i in 0..n {
                if matches!(children[i], FlexSize::Weight(_)) {
                    main_sizes[i] = 0;
                }
            }
            loop {
                let sum: u16 = main_sizes.iter().copied().sum();
                if sum <= available {
                    break;
                }
                let mut peeled = false;
                for i in (0..n).rev() {
                    let floor = children[i].min_main().min(main_sizes[i]);
                    // Under hard overflow, min may also shrink: floor to 0.
                    let floor = if sum.saturating_sub(1) < available {
                        floor
                    } else {
                        0
                    };
                    if main_sizes[i] > floor {
                        main_sizes[i] -= 1;
                        peeled = true;
                        break;
                    }
                }
                if !peeled {
                    // Force zero from end.
                    for i in (0..n).rev() {
                        if main_sizes[i] > 0 {
                            main_sizes[i] = 0;
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn grow_preferred(children: &[FlexSize], main_sizes: &mut [u16], residual: &mut u16) {
    // Round-robin grow Preferred up to max.
    while *residual > 0 {
        let mut grew = false;
        for (i, child) in children.iter().enumerate() {
            if *residual == 0 {
                break;
            }
            if let FlexSize::Preferred { min, max, .. } = *child {
                let hi = max.max(min);
                if main_sizes[i] < hi {
                    main_sizes[i] += 1;
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

fn justify_offsets(justify: Justify, free: u16, n: usize, overflowed: bool) -> (u16, u16) {
    if free == 0 || n == 0 || overflowed {
        return (0, 0);
    }
    match justify {
        Justify::Start => (0, 0),
        Justify::End => (free, 0),
        Justify::Center => (free / 2, 0),
        Justify::SpaceBetween if n > 1 => (0, free / (n as u16 - 1)),
        Justify::SpaceBetween => (0, 0),
        Justify::SpaceAround if n > 0 => {
            // CSS-like: free / n around each; leading = half unit.
            let unit = free / n as u16;
            let leading = unit / 2;
            let between = if n > 1 {
                free.saturating_sub(leading.saturating_mul(2)) / (n as u16 - 1)
            } else {
                0
            };
            (leading, between)
        }
        Justify::SpaceAround => (0, 0),
        Justify::SpaceEvenly => {
            let slots = n as u16 + 1;
            let each = free / slots;
            (each, each)
        }
    }
}

fn cross_place(align: Align, cross_total: u16, child_cross: u16) -> (u16, u16) {
    let size = match align {
        Align::Stretch => cross_total,
        _ => child_cross.min(cross_total).max(1).min(cross_total),
    };
    // If child_cross is 0 under non-stretch, still paint 0 height/width.
    let size = if child_cross == 0 && !matches!(align, Align::Stretch) {
        0
    } else {
        size
    };
    let off = match align {
        Align::Start | Align::Stretch => 0,
        Align::Center => cross_total.saturating_sub(size) / 2,
        Align::End => cross_total.saturating_sub(size),
    };
    (off, size)
}

fn layout_wrap_horizontal(
    content: Rect,
    spec: &StackSpec,
    children: &[FlexSize],
    out: &mut Vec<Rect>,
) {
    let sizes: Vec<u16> = children
        .iter()
        .map(|c| match *c {
            FlexSize::Fixed(n) => n.max(1),
            FlexSize::Preferred {
                min,
                preferred,
                max,
            } => preferred.clamp(min.max(1), max.max(1)),
            FlexSize::Weight(_) => 1,
            FlexSize::Collapsed => 0,
        })
        .collect();

    out.resize(
        children.len(),
        Rect {
            x: content.x,
            y: content.y,
            width: 0,
            height: 0,
        },
    );

    let row_h: u16 = 1;
    let mut x = content.x;
    let mut y = content.y;

    for (i, &w) in sizes.iter().enumerate() {
        if w == 0 {
            out[i] = Rect {
                x,
                y,
                width: 0,
                height: 0,
            };
            continue;
        }
        if x > content.x && x.saturating_add(w) > content.right() {
            y = y.saturating_add(row_h).saturating_add(spec.gap);
            x = content.x;
            if y >= content.bottom() {
                for j in i..children.len() {
                    out[j] = Rect {
                        x: content.x,
                        y: content.bottom(),
                        width: 0,
                        height: 0,
                    };
                }
                return;
            }
        }
        let place_w = w.min(content.right().saturating_sub(x));
        let h = row_h.min(content.bottom().saturating_sub(y));
        out[i] = Rect {
            x,
            y,
            width: place_w,
            height: h,
        };
        x = x.saturating_add(place_w).saturating_add(spec.gap);
    }
}

fn estimate_content_main(spec: &StackSpec, _children: &[FlexSize], rects: &[Rect]) -> u16 {
    if rects.is_empty() {
        return 0;
    }
    match spec.direction {
        StackDirection::Vertical => {
            let top = rects.iter().map(|r| r.y).min().unwrap_or(0);
            let bottom = rects.iter().map(|r| r.bottom()).max().unwrap_or(0);
            bottom.saturating_sub(top)
        }
        StackDirection::Horizontal if !spec.wrap => {
            let left = rects.iter().map(|r| r.x).min().unwrap_or(0);
            let right = rects.iter().map(|r| r.right()).max().unwrap_or(0);
            right.saturating_sub(left)
        }
        StackDirection::Horizontal => {
            let top = rects.iter().map(|r| r.y).min().unwrap_or(0);
            let bottom = rects.iter().map(|r| r.bottom()).max().unwrap_or(0);
            bottom.saturating_sub(top)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_fixed_and_weight_fill() {
        let area = Rect::new(0, 0, 20, 10);
        let layout = Stack::new().gap(0).layout(
            area,
            &[FlexSize::Fixed(2), FlexSize::fill(), FlexSize::Fixed(1)],
        );
        assert_eq!(layout.children[0].height, 2);
        assert_eq!(layout.children[2].height, 1);
        assert_eq!(layout.children[1].height, 7);
    }

    #[test]
    fn horizontal_inline_equal_weights() {
        let layout = Inline::new().layout(
            Rect::new(0, 0, 30, 5),
            &[
                FlexSize::Weight(1),
                FlexSize::Weight(1),
                FlexSize::Weight(1),
            ],
        );
        assert_eq!(layout.children[0].width, 10);
        assert_eq!(layout.children[1].width, 10);
        assert_eq!(layout.children[2].width, 10);
    }

    #[test]
    fn gap_consumes_main_axis() {
        let layout = Stack::new().gap(1).layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::Weight(1), FlexSize::Weight(1)],
        );
        assert_eq!(layout.children[0].height + layout.children[1].height, 9);
        assert_eq!(
            layout.children[1].y,
            layout.children[0].bottom().saturating_add(1)
        );
    }

    #[test]
    fn overflow_shrinks_from_end() {
        let layout = Stack::new().layout(
            Rect::new(0, 0, 10, 5),
            &[FlexSize::Fixed(3), FlexSize::Fixed(3), FlexSize::Fixed(3)],
        );
        assert!(layout.overflowed);
        let sum: u16 = layout.children.iter().map(|r| r.height).sum();
        assert_eq!(sum, 5);
        // End child takes remainder first under shrink-from-end reverse walk
        // (last keeps up to claim while remaining lasts from end).
        assert_eq!(layout.children[2].height, 3);
        assert_eq!(layout.children[1].height, 2);
        assert_eq!(layout.children[0].height, 0);
    }

    #[test]
    fn overflow_clip_tail_keeps_head() {
        let layout = Stack::new().overflow(OverflowPolicy::ClipTail).layout(
            Rect::new(0, 0, 10, 5),
            &[FlexSize::Fixed(3), FlexSize::Fixed(3), FlexSize::Fixed(3)],
        );
        assert!(layout.overflowed);
        assert_eq!(layout.children[0].height, 3);
        assert_eq!(layout.children[1].height, 2);
        assert_eq!(layout.children[2].height, 0);
    }

    #[test]
    fn justify_end_pushes_block() {
        let layout = Stack::new()
            .justify(Justify::End)
            .layout(Rect::new(0, 0, 10, 10), &[FlexSize::Fixed(2)]);
        assert_eq!(layout.children[0].y, 8);
    }

    #[test]
    fn justify_space_around() {
        let layout = Inline::new().justify(Justify::SpaceAround).layout(
            Rect::new(0, 0, 20, 3),
            &[FlexSize::Fixed(4), FlexSize::Fixed(4)],
        );
        // free = 12, unit = 6, leading = 3
        assert_eq!(layout.children[0].x, 3);
    }

    #[test]
    fn align_center_with_cross_hint() {
        let layout = Inline::new().align(Align::Center).layout_with_cross(
            Rect::new(0, 0, 20, 6),
            &[FlexSize::Fixed(4)],
            &[2],
        );
        assert_eq!(layout.children[0].height, 2);
        assert_eq!(layout.children[0].y, 2);
    }

    #[test]
    fn wrap_moves_to_next_row() {
        let layout = Inline::new().wrap(true).gap(0).layout(
            Rect::new(0, 0, 10, 4),
            &[FlexSize::Fixed(6), FlexSize::Fixed(6), FlexSize::Fixed(6)],
        );
        assert_eq!(layout.children[0].y, 0);
        assert_eq!(layout.children[1].y, 1);
        assert_eq!(layout.children[2].y, 2);
    }

    #[test]
    fn direction_for_width_responsive() {
        assert_eq!(direction_for_width(40, 60), StackDirection::Vertical);
        assert_eq!(direction_for_width(80, 60), StackDirection::Horizontal);
        let layout = Stack::new().responsive(80, 60).gap(0).layout(
            Rect::new(0, 0, 80, 4),
            &[FlexSize::Weight(1), FlexSize::Weight(1)],
        );
        assert_eq!(layout.direction, StackDirection::Horizontal);
        assert_eq!(layout.children[0].width, 40);
    }

    #[test]
    fn hit_child_and_regions() {
        let layout = Inline::new().layout(
            Rect::new(0, 0, 30, 3),
            &[
                FlexSize::Fixed(10),
                FlexSize::Fixed(10),
                FlexSize::Fixed(10),
            ],
        );
        assert_eq!(layout.hit_child(15, 1), Some(1));
        assert_eq!(layout.hit_child(5, 1), Some(0));
        let hits = layout.hit_regions();
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[1].id, 1);
    }

    #[test]
    fn padding_shrinks_content() {
        let layout = Stack::new()
            .padding(1, 1)
            .layout(Rect::new(0, 0, 20, 10), &[FlexSize::Weight(1)]);
        assert_eq!(layout.children[0], Rect::new(1, 1, 18, 8));
        assert_eq!(layout.content, Rect::new(1, 1, 18, 8));
    }

    #[test]
    fn collapsed_child_zero_main() {
        let layout = Stack::new().layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::Fixed(2), FlexSize::Collapsed, FlexSize::Weight(1)],
        );
        assert_eq!(layout.children[1].height, 0);
        assert_eq!(layout.children[2].height, 8);
    }

    #[test]
    fn preferred_clamped_and_grows() {
        let layout = Stack::new().layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::preferred(2, 100, 4), FlexSize::Weight(1)],
        );
        assert_eq!(layout.children[0].height, 4);
        // No weight residual to preferred-only: preferred 2..6 with free space grows
        let grow = Stack::new().layout(Rect::new(0, 0, 10, 10), &[FlexSize::preferred(2, 2, 6)]);
        assert_eq!(grow.children[0].height, 6);
    }

    #[test]
    fn layout_is_cheap() {
        let children = [
            FlexSize::Fixed(1),
            FlexSize::Weight(1),
            FlexSize::Fixed(1),
            FlexSize::Weight(2),
        ];
        let area = Rect::new(0, 0, 80, 40);
        let mut buf = Vec::with_capacity(8);
        let spec = StackSpec::vertical();
        for _ in 0..50_000 {
            layout_stack_into(area, &spec, &children, &mut buf);
        }
    }

    #[test]
    fn gap_only_without_pad() {
        let layout = Stack::new().gap(1).layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::Weight(1), FlexSize::Weight(1)],
        );
        assert_eq!(layout.children[0].height + layout.children[1].height, 9);
    }

    #[test]
    fn semantic_group_registers() {
        use crate::interaction::SemanticScene;
        let layout = Stack::new().layout(
            Rect::new(0, 0, 10, 6),
            &[FlexSize::Fixed(2), FlexSize::fill()],
        );
        let mut scene = SemanticScene::<String, ()>::new();
        scene.begin_frame();
        layout.register_semantic_group(&mut scene, "g".into(), "stack", |i| format!("c{i}"));
        assert!(scene.len() >= 2);
    }

    #[test]
    fn builder_direction_flip() {
        let layout = Inline::new().direction(StackDirection::Vertical).layout(
            Rect::new(0, 0, 10, 6),
            &[FlexSize::Fixed(2), FlexSize::fill()],
        );
        assert_eq!(layout.direction, StackDirection::Vertical);
        assert_eq!(layout.children[0].height, 2);
    }
}
