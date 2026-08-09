// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Center — constrained, axis-aware placement of child content.
//!
//! **Pure geometry.** Does not paint, own focus, or register a semantic node;
//! hosts place a child widget into [`CenterLayout::child`]. Used by empty
//! states, dialogs, onboarding, and failure screens.
//!
//! ## Safety
//!
//! - Never underflows: sizes use saturating arithmetic; child is always
//!   contained in `area` (may be zero-sized when outer is empty).
//! - Tiny terminals: preferred size is clamped to available space after margins.
//! - Optional one-cell safe margin (legacy [`crate::layout::centered_rect`]).

use ratatui_core::layout::Rect;

/// Which axes to center on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CenterAxis {
    /// Center on X only; Y stays at content origin (after margin).
    Horizontal,
    /// Center on Y only; X stays at content origin (after margin).
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
    /// When true, force at least one cell margin per side if outer ≥ preferred+2
    /// (matches historical `centered_rect` dialog chrome room).
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

    /// Axis.
    #[must_use]
    pub const fn axis(mut self, axis: CenterAxis) -> Self {
        self.axis = axis;
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

    /// From full spec.
    #[must_use]
    pub const fn from_spec(spec: CenterSpec) -> Self {
        Self { spec }
    }

    /// Axis.
    #[must_use]
    pub const fn axis(mut self, axis: CenterAxis) -> Self {
        self.spec.axis = axis;
        self
    }

    /// Max caps.
    #[must_use]
    pub const fn max_width(mut self, max: u16) -> Self {
        self.spec.max_width = Some(max);
        self
    }

    /// Max height.
    #[must_use]
    pub const fn max_height(mut self, max: u16) -> Self {
        self.spec.max_height = Some(max);
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

    // Margins: explicit + optional safe 1-cell when room for preferred size.
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

    // Content box after margins (may collapse to full area if margins too large).
    let content = inset(area, margin_x, margin_y);

    // Resolve desired size.
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
    // Clamp to content (never larger than available).
    w = w.min(content.width);
    h = h.min(content.height);

    // Axis placement.
    let (x, child_w) = match spec.axis {
        CenterAxis::Vertical => (content.x, content.width), // stretch X when only vertical center
        CenterAxis::Horizontal | CenterAxis::Both => {
            let x = content
                .x
                .saturating_add(content.width.saturating_sub(w) / 2);
            (x, w)
        }
    };
    let (y, child_h) = match spec.axis {
        CenterAxis::Horizontal => (content.y, content.height), // stretch Y when only horizontal center
        CenterAxis::Vertical | CenterAxis::Both => {
            let y = content
                .y
                .saturating_add(content.height.saturating_sub(h) / 2);
            (y, h)
        }
    };

    // For Horizontal-only: width is centered size; height fills content.
    // For Vertical-only: height is centered size; width fills content.
    // Re-apply carefully:
    let child = match spec.axis {
        CenterAxis::Both => Rect {
            x,
            y,
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

    // Final safety: child ⊆ area
    let child = clamp_inside(area, child);

    CenterLayout {
        area,
        child,
        content,
    }
}

/// Center a fixed-size block (legacy API). Equivalent to
/// `Center::dialog(width, height).layout(area).child` with safe margin.
#[must_use]
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Center::dialog(width, height).layout(area).child
}

/// Horizontally center a line of `display_cols` width on row `y` inside `area`.
/// Returns the starting x (never outside area).
#[must_use]
pub fn center_line_x(area: Rect, display_width: u16) -> u16 {
    let w = display_width.min(area.width);
    area.x
        .saturating_add(area.width.saturating_sub(w) / 2)
}

/// Vertically center a block of `height` rows inside `area`. Returns top y.
#[must_use]
pub fn center_block_y(area: Rect, height: u16) -> u16 {
    let h = height.min(area.height);
    area.y
        .saturating_add(area.height.saturating_sub(h) / 2)
}

fn inset(area: Rect, mx: u16, my: u16) -> Rect {
    let x = area.x.saturating_add(mx.min(area.width / 2));
    let y = area.y.saturating_add(my.min(area.height / 2));
    let width = area.width.saturating_sub(mx.saturating_mul(2).min(area.width));
    let height = area
        .height
        .saturating_sub(my.saturating_mul(2).min(area.height));
    if width == 0 || height == 0 {
        // Degenerate: fall back to full area so child can still claim cells.
        area
    } else {
        Rect {
            x,
            y,
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
    let x = inner.x.clamp(outer.x, outer.right().saturating_sub(1).max(outer.x));
    let y = inner.y.clamp(outer.y, outer.bottom().saturating_sub(1).max(outer.y));
    let right = inner.right().min(outer.right()).max(x);
    let bottom = inner.bottom().min(outer.bottom()).max(y);
    Rect {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
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
    fn dialog_safe_margin_matches_legacy() {
        let outer = Rect::new(7, 11, 20, 10);
        let legacy = {
            // old formula: min with area-2
            let w = 8u16.min(outer.width.saturating_sub(2));
            let h = 4u16.min(outer.height.saturating_sub(2));
            Rect {
                x: outer.x + outer.width.saturating_sub(w) / 2,
                y: outer.y + outer.height.saturating_sub(h) / 2,
                width: w,
                height: h,
            }
        };
        assert_eq!(centered_rect(8, 4, outer), legacy);
        assert_eq!(centered_rect(8, 4, outer), Rect::new(13, 14, 8, 4));
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
    fn min_honored_when_room() {
        let layout = Center::new(2, 1)
            .min(8, 3)
            .layout(Rect::new(0, 0, 40, 20));
        assert_eq!(layout.child.width, 8);
        assert_eq!(layout.child.height, 3);
    }

    #[test]
    fn tiny_outer_never_underflows() {
        for w in 0..8u16 {
            for h in 0..8u16 {
                let outer = Rect::new(3, 5, w, h);
                let child = Center::new(20, 20).layout(outer).child;
                assert!(child.x >= outer.x || w == 0);
                assert!(child.y >= outer.y || h == 0);
                if w > 0 && h > 0 {
                    assert!(child.right() <= outer.right());
                    assert!(child.bottom() <= outer.bottom());
                }
            }
        }
    }

    #[test]
    fn property_child_always_inside() {
        for ow in [0u16, 1, 2, 5, 10, 40, 80, 200] {
            for oh in [0u16, 1, 2, 5, 10, 24, 60] {
                for cw in [0u16, 1, 5, 40, 100] {
                    for ch in [0u16, 1, 5, 30, 100] {
                        let outer = Rect::new(2, 3, ow, oh);
                        let layout = Center::new(cw, ch)
                            .max_width(cw)
                            .max_height(ch)
                            .margin(1, 1)
                            .safe_margin(true)
                            .layout(outer);
                        let c = layout.child;
                        if ow == 0 || oh == 0 {
                            assert_eq!(c.width, 0);
                            continue;
                        }
                        assert!(c.x >= outer.x);
                        assert!(c.y >= outer.y);
                        assert!(c.right() <= outer.right());
                        assert!(c.bottom() <= outer.bottom());
                    }
                }
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
}
