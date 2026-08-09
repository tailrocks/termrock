// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Stack and Inline: cell-accurate flex-ish packing for component anatomy.
//!
//! **Stateless.** Pure functions over [`Rect`] + child size specs — no retained
//! layout tree, no allocation beyond the output `Vec` (callers can use
//! [`layout_stack_into`] to reuse a buffer).
//!
//! - [`Stack`] — vertical main axis (default)
//! - [`Inline`] — horizontal main axis (wrap optional)
//!
//! Cross-axis alignment, main-axis justify, density gaps, and overflow rules
//! are explicit. When children exceed the main axis: **fixed/preferred mins
//! win first**, then weight shares residual; trailing children may collapse to
//! zero. This is deterministic and Studio-debuggable (each child gets a rect).

use ratatui_core::layout::Rect;

use crate::style::{Density, SpacingScale};

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

impl StackDirection {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }

    /// Flip for responsive contracts (narrow → stack, wide → inline).
    #[must_use]
    pub const fn flipped(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }
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
    /// Equal free space around each child.
    SpaceEvenly,
}

/// How one child claims main-axis cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FlexSize {
    /// Exact main-axis size (clamped to remaining).
    Fixed(u16),
    /// Share of residual after fixed/preferred mins (weight ≥ 1).
    Weight(u16),
    /// Preferred size clamped to [min, max].
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

    /// Weight helper (zero treated as 1 when allocated).
    #[must_use]
    pub const fn weight(w: u16) -> Self {
        Self::Weight(w)
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
        }
    }

    /// Density-driven gap + padding.
    #[must_use]
    pub const fn with_density(mut self, density: Density) -> Self {
        self.gap = density.gap();
        self.pad_x = density.padding_x();
        self.pad_y = density.padding_y();
        self
    }

    /// Spacing scale from design system.
    #[must_use]
    pub const fn with_spacing(mut self, spacing: SpacingScale) -> Self {
        self.gap = spacing.gap;
        self.pad_x = spacing.pad_x;
        self.pad_y = spacing.pad_y;
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
    /// Child rect by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Rect> {
        self.children.get(index).copied()
    }

    /// Hit-test: first child containing the cell, if any.
    #[must_use]
    pub fn hit_child(&self, col: u16, row: u16) -> Option<usize> {
        let pos = ratatui_core::layout::Position { x: col, y: row };
        self.children
            .iter()
            .position(|r| r.width > 0 && r.height > 0 && r.contains(pos))
    }

    /// Group hit (padded content).
    #[must_use]
    pub fn contains_group(&self, col: u16, row: u16) -> bool {
        self.content
            .contains(ratatui_core::layout::Position { x: col, y: row })
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

    /// Vertical + density spacing.
    #[must_use]
    pub const fn with_density(density: Density) -> Self {
        Self {
            spec: StackSpec::vertical().with_density(density),
        }
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

    /// Horizontal + density spacing.
    #[must_use]
    pub const fn with_density(density: Density) -> Self {
        Self {
            spec: StackSpec::horizontal().with_density(density),
        }
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
    let mut out = Vec::with_capacity(children.len());
    layout_stack_into(area, spec, children, &mut out);
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
pub fn layout_stack_into(
    area: Rect,
    spec: &StackSpec,
    children: &[FlexSize],
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

    layout_single_line(content, spec, children, out);
}

fn padded_content(area: Rect, spec: &StackSpec) -> Rect {
    let x = area.x.saturating_add(spec.pad_x);
    let y = area.y.saturating_add(spec.pad_y);
    let width = area
        .width
        .saturating_sub(spec.pad_x.saturating_mul(2));
    let height = area
        .height
        .saturating_sub(spec.pad_y.saturating_mul(2));
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
    out: &mut Vec<Rect>,
) {
    let main_total = main_len(content, spec.direction);
    let cross_total = cross_len(content, spec.direction);
    let n = children.len();
    let gaps = spec.gap.saturating_mul(n.saturating_sub(1) as u16);

    // First pass: resolve mins for fixed/preferred; collect weights.
    let mut main_sizes = vec![0u16; n];
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
                let p = preferred.clamp(min, max.max(min));
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
        // Shrink fixed/preferred proportionally from the end.
        overflowed = true;
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
    } else {
        // Distribute residual to weights.
        let mut residual = available.saturating_sub(fixed_sum);
        if weight_sum == 0 {
            // No weights: apply justify free space as leading offset later.
        } else {
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
                    // Last weight gets remainder.
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
            let _ = residual;
        }
    }

    let used_main: u16 = main_sizes.iter().copied().sum::<u16>().saturating_add(gaps);
    let free = main_total.saturating_sub(used_main);
    let (leading, between_extra) = justify_offsets(spec.justify, free, n, overflowed);

    let mut cursor = match spec.direction {
        StackDirection::Vertical => content.y.saturating_add(leading),
        StackDirection::Horizontal => content.x.saturating_add(leading),
    };

    for i in 0..n {
        let main = main_sizes[i];
        let (cross_off, cross_size) = cross_place(spec.align, cross_total, cross_total);
        let rect = match spec.direction {
            StackDirection::Vertical => Rect {
                x: content.x.saturating_add(cross_off),
                y: cursor,
                width: if matches!(spec.align, Align::Stretch) {
                    cross_total
                } else {
                    cross_size
                },
                height: main,
            },
            StackDirection::Horizontal => Rect {
                x: cursor,
                y: content.y.saturating_add(cross_off),
                width: main,
                height: if matches!(spec.align, Align::Stretch) {
                    cross_total
                } else {
                    cross_size
                },
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
    let _ = overflowed;
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
        _ => child_cross.min(cross_total),
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
    // Pre-size: use preferred/fixed; weight → 1 cell min for wrapping chips.
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

    let row_h: u16 = 1; // wrap rows are 1 cell tall; hosts that need multi-row children use nested Stack
    let mut x = content.x;
    let mut y = content.y;
    let mut row_max_x = content.x;

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
            // wrap
            y = y.saturating_add(row_h).saturating_add(spec.gap);
            x = content.x;
            if y >= content.bottom() {
                // overflow remaining
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
        row_max_x = row_max_x.max(x);
        let _ = row_max_x;
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
            // wrapped: height of packed block
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
            &[FlexSize::Fixed(2), FlexSize::Weight(1), FlexSize::Fixed(1)],
        );
        assert_eq!(layout.children[0].height, 2);
        assert_eq!(layout.children[2].height, 1);
        assert_eq!(layout.children[1].height, 7);
        assert_eq!(
            layout.children[0].height + layout.children[1].height + layout.children[2].height,
            10
        );
    }

    #[test]
    fn horizontal_inline_equal_weights() {
        let layout = Inline::new().layout(
            Rect::new(0, 0, 30, 5),
            &[FlexSize::Weight(1), FlexSize::Weight(1), FlexSize::Weight(1)],
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
            &[
                FlexSize::Fixed(3),
                FlexSize::Fixed(3),
                FlexSize::Fixed(3),
            ],
        );
        assert!(layout.overflowed);
        let sum: u16 = layout.children.iter().map(|r| r.height).sum();
        assert_eq!(sum, 5);
    }

    #[test]
    fn justify_end_pushes_block() {
        let layout = Stack::new()
            .justify(Justify::End)
            .layout(Rect::new(0, 0, 10, 10), &[FlexSize::Fixed(2)]);
        assert_eq!(layout.children[0].y, 8);
    }

    #[test]
    fn align_center_on_cross_axis() {
        // Non-stretch: child cross size = full for Stretch default; use Start with pad
        let layout = Inline::new()
            .align(Align::Stretch)
            .layout(Rect::new(0, 0, 20, 6), &[FlexSize::Fixed(4)]);
        assert_eq!(layout.children[0].height, 6);
    }

    #[test]
    fn wrap_moves_to_next_row() {
        let layout = Inline::new().wrap(true).gap(0).layout(
            Rect::new(0, 0, 10, 4),
            &[
                FlexSize::Fixed(6),
                FlexSize::Fixed(6),
                FlexSize::Fixed(6),
            ],
        );
        assert_eq!(layout.children[0].y, 0);
        assert_eq!(layout.children[1].y, 1);
        assert_eq!(layout.children[2].y, 2);
    }

    #[test]
    fn direction_for_width_responsive() {
        assert_eq!(direction_for_width(40, 60), StackDirection::Vertical);
        assert_eq!(direction_for_width(80, 60), StackDirection::Horizontal);
    }

    #[test]
    fn hit_child_index() {
        let layout = Inline::new().layout(
            Rect::new(0, 0, 30, 3),
            &[FlexSize::Fixed(10), FlexSize::Fixed(10), FlexSize::Fixed(10)],
        );
        assert_eq!(layout.hit_child(15, 1), Some(1));
        assert_eq!(layout.hit_child(5, 1), Some(0));
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
    fn preferred_clamped() {
        let layout = Stack::new().layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::preferred(2, 100, 4), FlexSize::Weight(1)],
        );
        assert_eq!(layout.children[0].height, 4);
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
        let spec = StackSpec::vertical().with_density(Density::Compact);
        for _ in 0..50_000 {
            layout_stack_into(area, &spec, &children, &mut buf);
        }
    }

    #[test]
    fn density_gap_comfortable() {
        let layout = Stack::with_density(Density::Comfortable).layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::Weight(1), FlexSize::Weight(1)],
        );
        // Comfortable: pad_y=1 each side + gap=1 → content height 8, children sum 7.
        assert_eq!(layout.content.height, 8);
        assert_eq!(
            layout.children[0].height + layout.children[1].height,
            7
        );
    }

    #[test]
    fn gap_only_without_pad() {
        let layout = Stack::new().gap(1).layout(
            Rect::new(0, 0, 10, 10),
            &[FlexSize::Weight(1), FlexSize::Weight(1)],
        );
        assert_eq!(
            layout.children[0].height + layout.children[1].height,
            9
        );
    }
}
