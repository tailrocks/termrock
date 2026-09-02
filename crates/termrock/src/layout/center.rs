// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Center — constrained, axis-aware placement of child content.
//!
//! **Pure geometry.** Does not paint, own focus, or register a semantic node for
//! itself; hosts place a child widget into [`CenterLayout::child`] and may use
//! [`CenterLayout::register_child_semantics`] so only the **child** appears in
//! the semantic scene. Used by empty states, dialogs, onboarding, and failure
//! screens.
//!
//! ## Safety
//!
//! - Never underflows: sizes use saturating arithmetic; child is always
//!   contained in `area` (may be zero-sized when outer is empty).
//! - Tiny terminals: preferred size is clamped to available space after margins.
//! - Optional one-cell safe margin through [`Center::dialog`].
use ratatui_core::layout::Rect;

/// Which axes to center on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CenterAxis {
    /// Center on X only; height fills content (after margin).
    Horizontal,
    /// Center on Y only; width fills content (after margin).
    Vertical,
    /// Center on both axes (default).
    #[default]
    Both,
}

impl CenterAxis {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Both => "both",
        }
    }
}

/// Constraints for resolving the child rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CenterSpec {
    /// Axis policy.
    pub axis: CenterAxis,
    /// Preferred child width (clamped by max/min and available).
    pub width: u16,
    /// Preferred child height.
    pub height: u16,
    /// Hard max width (None → no extra cap beyond area).
    pub max_width: Option<u16>,
    /// Hard max height.
    pub max_height: Option<u16>,
    /// Soft minimum width (honored when area allows).
    pub min_width: u16,
    /// Soft minimum height.
    pub min_height: u16,
    /// Outer horizontal margin (cells reserved on each side when room).
    pub margin_x: u16,
    /// Outer vertical margin.
    pub margin_y: u16,
    /// When true, force at least one cell margin per side if outer ≥ preferred+2.
    pub safe_margin: bool,
}

impl Default for CenterSpec {
    fn default() -> Self {
        Self {
            axis: CenterAxis::Both,
            width: 1,
            height: 1,
            max_width: None,
            max_height: None,
            min_width: 0,
            min_height: 0,
            margin_x: 0,
            margin_y: 0,
            safe_margin: false,
        }
    }
}

impl CenterSpec {
    /// Both-axis center with preferred size.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            axis: CenterAxis::Both,
            width,
            height,
            max_width: None,
            max_height: None,
            min_width: 0,
            min_height: 0,
            margin_x: 0,
            margin_y: 0,
            safe_margin: false,
        }
    }

    /// Legacy dialog helper: both axes + 1-cell safe margin when room.
    #[must_use]
    pub const fn dialog(width: u16, height: u16) -> Self {
        Self {
            axis: CenterAxis::Both,
            width,
            height,
            max_width: None,
            max_height: None,
            min_width: 0,
            min_height: 0,
            margin_x: 0,
            margin_y: 0,
            safe_margin: true,
        }
    }

    /// Onboarding / hero card: safe margin + readable max width/height caps.
    #[must_use]
    pub const fn onboarding(width: u16, height: u16) -> Self {
        Self {
            axis: CenterAxis::Both,
            width,
            height,
            max_width: Some(56),
            max_height: Some(20),
            min_width: 12,
            min_height: 4,
            margin_x: 1,
            margin_y: 1,
            safe_margin: true,
        }
    }

    /// Failure / error panel: safe margin, moderate max width.
    #[must_use]
    pub const fn failure(width: u16, height: u16) -> Self {
        Self {
            axis: CenterAxis::Both,
            width,
            height,
            max_width: Some(48),
            max_height: Some(16),
            min_width: 8,
            min_height: 3,
            margin_x: 1,
            margin_y: 1,
            safe_margin: true,
        }
    }

    /// Axis.
    #[must_use]
    pub const fn axis(mut self, axis: CenterAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Preferred size.
    #[must_use]
    pub const fn size(mut self, width: u16, height: u16) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Max width/height caps.
    #[must_use]
    pub const fn max(mut self, max_width: Option<u16>, max_height: Option<u16>) -> Self {
        self.max_width = max_width;
        self.max_height = max_height;
        self
    }

    /// Minimum child size when outer allows.
    #[must_use]
    pub const fn min(mut self, min_width: u16, min_height: u16) -> Self {
        self.min_width = min_width;
        self.min_height = min_height;
        self
    }

    /// Outer margins.
    #[must_use]
    pub const fn margin(mut self, margin_x: u16, margin_y: u16) -> Self {
        self.margin_x = margin_x;
        self.margin_y = margin_y;
        self
    }

    /// Safe one-cell margin when room.
    #[must_use]
    pub const fn safe_margin(mut self, on: bool) -> Self {
        self.safe_margin = on;
        self
    }
}

/// Resolved placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CenterLayout {
    /// Outer allocation passed to [`layout_center`].
    pub area: Rect,
    /// Child content rectangle (always ⊆ area).
    pub child: Rect,
    /// Available content box after margins (may equal area).
    pub content: Rect,
}

impl CenterLayout {
    /// True when child has positive area.
    #[must_use]
    pub const fn has_child(self) -> bool {
        self.child.width > 0 && self.child.height > 0
    }

    /// Hit-test: point inside child (not a focus target by itself).
    #[must_use]
    pub fn contains_child(self, col: u16, row: u16) -> bool {
        self.child
            .contains(ratatui_core::layout::Position { x: col, y: row })
    }

    /// Studio / debug one-liner.
    #[must_use]
    pub fn debug_summary(self) -> String {
        format!(
            "center outer={}x{} content={}x{} child={}x{}@{},{}",
            self.area.width,
            self.area.height,
            self.content.width,
            self.content.height,
            self.child.width,
            self.child.height,
            self.child.x,
            self.child.y
        )
    }

    /// Register **only** the child as a non-focusable content node.
    ///
    /// Center itself is never registered — avoids fake interactive geometry.
    pub fn register_child_semantics<Id, Action>(
        self,
        scene: &mut crate::interaction::SemanticScene<Id, Action>,
        child_id: Id,
        label: &str,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if !self.has_child() {
            return;
        }
        use crate::interaction::{SemanticNode, SemanticRole};
        let _ = scene.register(
            SemanticNode::content(child_id, self.child)
                .role(SemanticRole::Content)
                .label(label)
                .focusable(false),
        );
    }
}

/// Ergonomic builder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Center {
    spec: CenterSpec,
}

impl Center {
    /// Both-axis center, preferred size.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            spec: CenterSpec::new(width, height),
        }
    }

    /// Dialog-style with safe margin.
    #[must_use]
    pub const fn dialog(width: u16, height: u16) -> Self {
        Self {
            spec: CenterSpec::dialog(width, height),
        }
    }

    /// Onboarding / hero card placement.
    #[must_use]
    pub const fn onboarding(width: u16, height: u16) -> Self {
        Self {
            spec: CenterSpec::onboarding(width, height),
        }
    }

    /// Failure / error panel placement.
    #[must_use]
    pub const fn failure(width: u16, height: u16) -> Self {
        Self {
            spec: CenterSpec::failure(width, height),
        }
    }

    /// From full spec.
    #[must_use]
    pub const fn from_spec(spec: CenterSpec) -> Self {
        Self { spec }
    }

    /// Borrow spec.
    #[must_use]
    pub const fn spec(self) -> CenterSpec {
        self.spec
    }

    /// Axis.
    #[must_use]
    pub const fn axis(mut self, axis: CenterAxis) -> Self {
        self.spec.axis = axis;
        self
    }

    /// Preferred size.
    #[must_use]
    pub const fn size(mut self, width: u16, height: u16) -> Self {
        self.spec.width = width;
        self.spec.height = height;
        self
    }

    /// Max width cap.
    #[must_use]
    pub const fn max_width(mut self, max: u16) -> Self {
        self.spec.max_width = Some(max);
        self
    }

    /// Max height cap.
    #[must_use]
    pub const fn max_height(mut self, max: u16) -> Self {
        self.spec.max_height = Some(max);
        self
    }

    /// Max width and height.
    #[must_use]
    pub const fn max(mut self, max_width: u16, max_height: u16) -> Self {
        self.spec.max_width = Some(max_width);
        self.spec.max_height = Some(max_height);
        self
    }

    /// Mins.
    #[must_use]
    pub const fn min(mut self, min_width: u16, min_height: u16) -> Self {
        self.spec.min_width = min_width;
        self.spec.min_height = min_height;
        self
    }

    /// Margins.
    #[must_use]
    pub const fn margin(mut self, margin_x: u16, margin_y: u16) -> Self {
        self.spec.margin_x = margin_x;
        self.spec.margin_y = margin_y;
        self
    }

    /// Safe margin flag.
    #[must_use]
    pub const fn safe_margin(mut self, on: bool) -> Self {
        self.spec.safe_margin = on;
        self
    }

    /// Resolve child rect inside `area`.
    #[must_use]
    pub fn layout(self, area: Rect) -> CenterLayout {
        layout_center(area, &self.spec)
    }
}

/// Layout child rectangle inside `area` using `spec`.
#[must_use]
pub fn layout_center(area: Rect, spec: &CenterSpec) -> CenterLayout {
    if area.width == 0 || area.height == 0 {
        return CenterLayout {
            area,
            child: Rect {
                x: area.x,
                y: area.y,
                width: 0,
                height: 0,
            },
            content: area,
        };
    }

    let mut margin_x = spec.margin_x;
    let mut margin_y = spec.margin_y;
    if spec.safe_margin {
        if area.width >= spec.width.saturating_add(2) {
            margin_x = margin_x.max(1);
        }
        if area.height >= spec.height.saturating_add(2) {
            margin_y = margin_y.max(1);
        }
    }

    let content = inset(area, margin_x, margin_y);

    let mut w = spec.width;
    let mut h = spec.height;
    if let Some(max_w) = spec.max_width {
        w = w.min(max_w);
    }
    if let Some(max_h) = spec.max_height {
        h = h.min(max_h);
    }
    // Prefer at least min when content allows.
    if content.width >= spec.min_width {
        w = w.max(spec.min_width);
    }
    if content.height >= spec.min_height {
        h = h.max(spec.min_height);
    }
    // Never larger than available content.
    w = w.min(content.width);
    h = h.min(content.height);

    let child = match spec.axis {
        CenterAxis::Both => Rect {
            x: content
                .x
                .saturating_add(content.width.saturating_sub(w) / 2),
            y: content
                .y
                .saturating_add(content.height.saturating_sub(h) / 2),
            width: w,
            height: h,
        },
        CenterAxis::Horizontal => Rect {
            x: content
                .x
                .saturating_add(content.width.saturating_sub(w) / 2),
            y: content.y,
            width: w,
            height: content.height,
        },
        CenterAxis::Vertical => Rect {
            x: content.x,
            y: content
                .y
                .saturating_add(content.height.saturating_sub(h) / 2),
            width: content.width,
            height: h,
        },
    };

    let child = clamp_inside(area, child);

    CenterLayout {
        area,
        child,
        content,
    }
}

/// A modal sized as a share of the terminal it opens in.
///
/// The minimums are *preferences*, not floors. A terminal narrower than the
/// modal's minimum is a real size — an operator with a split pane, a tmux
/// column, a phone SSH client — and the modal contracts into it. Three
/// patterns used to hand-roll this arithmetic with `clamp(min, max)`, which
/// panics the moment the minimum exceeds the maximum, so opening any overlay
/// in a narrow pane took the whole application down (plans/019).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModalSpec {
    /// Numerator of the terminal width the modal asks for.
    pub width_num: u16,
    /// Denominator of the terminal width the modal asks for.
    pub width_den: u16,
    /// Width the modal prefers not to go below, when the terminal allows.
    pub min_width: u16,
    /// Numerator of the terminal height the modal asks for.
    pub height_num: u16,
    /// Denominator of the terminal height the modal asks for.
    pub height_den: u16,
    /// Height the modal prefers not to go below, when the terminal allows.
    pub min_height: u16,
    /// Cells left to the terminal on each axis, so the modal never fills it.
    pub margin: u16,
}

impl ModalSpec {
    /// A modal claiming `width_num/width_den` of the width, a third of the
    /// height, and never less than `min_width` unless the terminal is smaller.
    #[must_use]
    pub const fn new(width_num: u16, width_den: u16, min_width: u16) -> Self {
        Self {
            width_num,
            width_den,
            min_width,
            height_num: 1,
            height_den: 3,
            min_height: 6,
            margin: 2,
        }
    }

    /// Sets the height share and the height the modal prefers not to lose.
    #[must_use]
    pub const fn height(mut self, num: u16, den: u16, min_height: u16) -> Self {
        self.height_num = num;
        self.height_den = den;
        self.min_height = min_height;
        self
    }

    /// Sets the cells left to the terminal on each axis.
    #[must_use]
    pub const fn margin(mut self, margin: u16) -> Self {
        self.margin = margin;
        self
    }
}

/// Places a modal proportionally inside `area`, biased into the upper third.
///
/// The bias is deliberate: a dialog pinned to the exact middle covers the row
/// the operator was reading, while one held a little high leaves the content
/// that prompted it visible underneath.
///
/// Total: never panics, never escapes `area`, and never overflows — the share
/// is computed in `u32` because a 3/5 share of a wide terminal overflows `u16`
/// before the division brings it back.
#[must_use]
pub fn modal_rect(area: Rect, spec: ModalSpec) -> Rect {
    if area.is_empty() {
        return Rect::new(area.x, area.y, 0, 0);
    }
    let width = modal_extent(
        area.width,
        spec.width_num,
        spec.width_den,
        spec.min_width,
        spec.margin,
    );
    let height = modal_extent(
        area.height,
        spec.height_num,
        spec.height_den,
        spec.min_height,
        spec.margin,
    );
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 4),
        width,
        height,
    }
}

/// One axis of [`modal_rect`]: a share, raised to the minimum, capped by room.
///
/// The order matters. Raising to the minimum *before* capping is what lets the
/// cap win on a terminal too small for the minimum, instead of asserting an
/// impossible range.
fn modal_extent(available: u16, num: u16, den: u16, min: u16, margin: u16) -> u16 {
    let room = available.saturating_sub(margin).max(1);
    let share = u32::from(available) * u32::from(num) / u32::from(den.max(1));
    let share = u16::try_from(share).unwrap_or(u16::MAX);
    share.max(min).min(room)
}

/// Horizontally center a line of `display_cols` width on row `y` inside `area`.
/// Returns the starting x (never outside area).
#[must_use]
pub fn center_line_x(area: Rect, display_width: u16) -> u16 {
    let w = display_width.min(area.width);
    area.x.saturating_add(area.width.saturating_sub(w) / 2)
}

/// Vertically center a block of `height` rows inside `area`. Returns top y.
#[must_use]
pub fn center_block_y(area: Rect, height: u16) -> u16 {
    let h = height.min(area.height);
    area.y.saturating_add(area.height.saturating_sub(h) / 2)
}

fn inset(area: Rect, mx: u16, my: u16) -> Rect {
    // Cap margins so we never double-count past the outer size.
    let mx = mx.min(area.width / 2);
    let my = my.min(area.height / 2);
    let width = area.width.saturating_sub(mx.saturating_mul(2));
    let height = area.height.saturating_sub(my.saturating_mul(2));
    if width == 0 || height == 0 {
        // Degenerate: fall back to full area so child can still claim cells.
        area
    } else {
        Rect {
            x: area.x.saturating_add(mx),
            y: area.y.saturating_add(my),
            width,
            height,
        }
    }
}

fn clamp_inside(outer: Rect, inner: Rect) -> Rect {
    if outer.width == 0 || outer.height == 0 {
        return Rect {
            x: outer.x,
            y: outer.y,
            width: 0,
            height: 0,
        };
    }
    let max_x = outer.right().saturating_sub(1).max(outer.x);
    let max_y = outer.bottom().saturating_sub(1).max(outer.y);
    let x = inner.x.clamp(outer.x, max_x);
    let y = inner.y.clamp(outer.y, max_y);
    let right = inner.right().min(outer.right()).max(x);
    let bottom = inner.bottom().min(outer.bottom()).max(y);
    Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

/// Assert child ⊆ outer (used by property tests; always true for valid layout).
#[cfg(test)]
fn child_inside(outer: Rect, child: Rect) -> bool {
    if outer.width == 0 || outer.height == 0 {
        return child.width == 0 && child.height == 0;
    }
    child.x >= outer.x
        && child.y >= outer.y
        && child.right() <= outer.right()
        && child.bottom() <= outer.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_axis_centers() {
        let layout = Center::new(10, 4).layout(Rect::new(0, 0, 40, 20));
        assert_eq!(layout.child, Rect::new(15, 8, 10, 4));
    }

    #[test]
    fn dialog_safe_margin_matches_expected_geometry() {
        let outer = Rect::new(7, 11, 20, 10);
        let legacy = {
            let w = 8u16.min(outer.width.saturating_sub(2));
            let h = 4u16.min(outer.height.saturating_sub(2));
            Rect {
                x: outer.x + outer.width.saturating_sub(w) / 2,
                y: outer.y + outer.height.saturating_sub(h) / 2,
                width: w,
                height: h,
            }
        };
        let child = Center::dialog(8, 4).layout(outer).child;
        assert_eq!(child, legacy);
        assert_eq!(child, Rect::new(13, 14, 8, 4));
    }

    #[test]
    fn horizontal_only_fills_height() {
        let layout = Center::new(6, 2)
            .axis(CenterAxis::Horizontal)
            .layout(Rect::new(0, 0, 20, 10));
        assert_eq!(layout.child.width, 6);
        assert_eq!(layout.child.height, 10);
        assert_eq!(layout.child.x, 7);
        assert_eq!(layout.child.y, 0);
    }

    #[test]
    fn vertical_only_fills_width() {
        let layout = Center::new(6, 2)
            .axis(CenterAxis::Vertical)
            .layout(Rect::new(0, 0, 20, 10));
        assert_eq!(layout.child.width, 20);
        assert_eq!(layout.child.height, 2);
        assert_eq!(layout.child.y, 4);
    }

    #[test]
    fn max_width_caps_child() {
        let layout = Center::new(100, 4)
            .max_width(12)
            .layout(Rect::new(0, 0, 40, 10));
        assert_eq!(layout.child.width, 12);
    }

    #[test]
    fn max_both_builder() {
        let layout = Center::new(100, 50)
            .max(12, 6)
            .layout(Rect::new(0, 0, 80, 40));
        assert_eq!(layout.child.width, 12);
        assert_eq!(layout.child.height, 6);
    }

    #[test]
    fn min_honored_when_room() {
        let layout = Center::new(2, 1).min(8, 3).layout(Rect::new(0, 0, 40, 20));
        assert_eq!(layout.child.width, 8);
        assert_eq!(layout.child.height, 3);
    }

    #[test]
    fn onboarding_caps_and_mins() {
        let layout = Center::onboarding(80, 40).layout(Rect::new(0, 0, 120, 40));
        assert!(layout.child.width <= 56);
        assert!(layout.child.height <= 20);
        assert!(layout.child.width >= 12 || layout.content.width < 12);
    }

    #[test]
    fn failure_recipe() {
        let layout = Center::failure(60, 20).layout(Rect::new(0, 0, 100, 30));
        assert!(layout.child.width <= 48);
        assert!(layout.has_child());
    }

    #[test]
    fn tiny_outer_never_underflows() {
        for w in 0..8u16 {
            for h in 0..8u16 {
                let outer = Rect::new(3, 5, w, h);
                let child = Center::new(20, 20).layout(outer).child;
                assert!(child_inside(outer, child));
            }
        }
    }

    #[test]
    fn property_child_always_inside_all_axes() {
        // Exhaustive-ish over terminal dimensions and axes (property-style).
        let axes = [
            CenterAxis::Both,
            CenterAxis::Horizontal,
            CenterAxis::Vertical,
        ];
        for ow in 0..=40u16 {
            for oh in [0u16, 1, 2, 3, 5, 8, 12, 24, 40] {
                for cw in [0u16, 1, 3, 8, 20, 40, 80] {
                    for ch in [0u16, 1, 3, 8, 20, 40] {
                        for &axis in &axes {
                            for safe in [false, true] {
                                let outer = Rect::new(2, 3, ow, oh);
                                let layout = Center::new(cw, ch)
                                    .axis(axis)
                                    .max_width(cw.max(1))
                                    .max_height(ch.max(1))
                                    .min(0, 0)
                                    .margin(if safe { 1 } else { 0 }, if safe { 1 } else { 0 })
                                    .safe_margin(safe)
                                    .layout(outer);
                                assert!(
                                    child_inside(outer, layout.child),
                                    "axis={:?} outer={:?} child={:?} safe={safe}",
                                    axis,
                                    outer,
                                    layout.child
                                );
                                // content also ⊆ area
                                assert!(
                                    child_inside(outer, layout.content) || layout.content == outer
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn property_full_screen_sizes_dialog() {
        // Common terminal sizes + dialog preferred sizes.
        let screens = [
            (0u16, 0u16),
            (1, 1),
            (20, 5),
            (40, 12),
            (80, 24),
            (100, 30),
            (120, 40),
            (200, 50),
        ];
        let prefs = [(8u16, 4u16), (40, 12), (52, 9), (60, 20), (100, 40)];
        for &(ow, oh) in &screens {
            for &(pw, ph) in &prefs {
                let outer = Rect::new(0, 0, ow, oh);
                let child = Center::dialog(pw, ph).layout(outer).child;
                assert!(child_inside(outer, child));
                let fail = Center::failure(pw, ph).layout(outer).child;
                assert!(child_inside(outer, fail));
                let onb = Center::onboarding(pw, ph).layout(outer).child;
                assert!(child_inside(outer, onb));
            }
        }
    }

    #[test]
    fn center_line_x_clamps() {
        let area = Rect::new(10, 0, 20, 1);
        assert_eq!(center_line_x(area, 4), 18);
        assert_eq!(center_line_x(area, 100), 10);
    }

    #[test]
    fn center_block_y_clamps() {
        let area = Rect::new(0, 10, 5, 20);
        assert_eq!(center_block_y(area, 4), 18);
        assert_eq!(center_block_y(area, 100), 10);
    }

    #[test]
    fn layout_is_cheap() {
        let area = Rect::new(0, 0, 80, 24);
        for _ in 0..100_000 {
            let _ = Center::new(40, 12).safe_margin(true).layout(area);
        }
    }

    #[test]
    fn empty_area_zero_child() {
        let layout = Center::new(5, 5).layout(Rect::new(0, 0, 0, 0));
        assert_eq!(layout.child.width, 0);
        assert!(!layout.has_child());
    }

    #[test]
    fn semantic_registers_child_only() {
        use crate::interaction::SemanticScene;
        let layout = Center::new(10, 4).layout(Rect::new(0, 0, 40, 20));
        let mut scene = SemanticScene::<&str, ()>::new();
        scene.begin_frame();
        layout.register_child_semantics(&mut scene, "panel", "onboarding");
        assert_eq!(scene.len(), 1);
        assert_eq!(scene.nodes()[0].id, "panel");
        assert!(!scene.nodes()[0].focusable);
        // Empty outer → no registration
        let empty = Center::new(10, 4).layout(Rect::new(0, 0, 0, 0));
        empty.register_child_semantics(&mut scene, "ghost", "x");
        assert_eq!(scene.len(), 1);
    }

    #[test]
    fn debug_summary_nonempty() {
        let s = Center::new(4, 2)
            .layout(Rect::new(0, 0, 20, 10))
            .debug_summary();
        assert!(s.contains("center"));
        assert!(s.contains("child="));
    }

    #[test]
    fn contains_child_hit() {
        let layout = Center::new(4, 2).layout(Rect::new(0, 0, 20, 10));
        assert!(layout.contains_child(layout.child.x, layout.child.y));
        assert!(!layout.contains_child(0, 0));
    }
}
