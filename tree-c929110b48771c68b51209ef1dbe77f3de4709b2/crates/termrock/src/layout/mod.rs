//! Responsive layout specifications and caller-defined bottom slots.

mod dialog;

use ratatui_core::layout::Rect;

pub use crate::interaction::HitRegion;
pub use dialog::{render_dialog_shell, render_scrollable_dialog_body};

/// Center a fixed-size rectangle inside `area`, leaving a one-cell margin
/// where the terminal has room for it.
#[must_use]
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + area.width.saturating_sub(w) / 2,
        y: area.y + area.height.saturating_sub(h) / 2,
        width: w,
        height: h,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Vertical placement policies for bounded dialog geometry.
pub enum Placement {
    /// Places content at the centered.
    Centered,
    /// Places content at the top.
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Size constraints, margins, and placement for a dialog rectangle.
pub struct DialogSpec {
    /// Min width in terminal cells.
    pub min_width: u16,
    /// Preferred width in terminal cells.
    pub preferred_width: u16,
    /// Max width in terminal cells.
    pub max_width: u16,
    /// Min height in terminal rows.
    pub min_height: u16,
    /// Preferred height in terminal rows.
    pub preferred_height: u16,
    /// Max height in terminal rows.
    pub max_height: u16,
    /// Horizontal margin in terminal cells.
    pub horizontal_margin: u16,
    /// Vertical margin in terminal cells.
    pub vertical_margin: u16,
    /// Placement policy inside the margin-constrained area.
    pub placement: Placement,
}

/// Resolve a dialog specification inside `outer` without assuming a rendering backend.
#[must_use]
pub fn resolve_dialog(outer: Rect, spec: DialogSpec) -> Rect {
    let available_width = outer.width.saturating_sub(spec.horizontal_margin);
    let available_height = outer.height.saturating_sub(spec.vertical_margin);
    let width = spec
        .preferred_width
        .clamp(spec.min_width, spec.max_width.max(spec.min_width))
        .min(available_width.max(spec.min_width));
    let height = spec
        .preferred_height
        .clamp(spec.min_height, spec.max_height.max(spec.min_height))
        .min(available_height.max(spec.min_height));
    let x = outer
        .x
        .saturating_add(outer.width.saturating_sub(width) / 2);
    let y = match spec.placement {
        Placement::Centered => outer
            .y
            .saturating_add(outer.height.saturating_sub(height) / 2),
        Placement::Top => outer.y,
    };
    Rect::new(x, y, width, height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Resolved body and bottom-chrome rectangles.
pub struct Slots {
    /// Remaining rectangle available to primary content.
    pub body: Rect,
    /// Rectangle reserved for bottom chrome.
    pub bottom: Rect,
}

impl Slots {
    #[must_use]
    /// Reserves bottom chrome and returns the resulting body and bottom rectangles.
    pub const fn bottom(area: Rect, rows: u16) -> Self {
        let bottom_height = if area.height < rows {
            area.height
        } else {
            rows
        };
        Self {
            body: Rect::new(area.x, area.y, area.width, area.height - bottom_height),
            bottom: Rect::new(
                area.x,
                area.y.saturating_add(area.height - bottom_height),
                area.width,
                bottom_height,
            ),
        }
    }
}

/// Split fixed rows from the bottom of an area in top-to-bottom order.
///
/// The body receives all remaining height. Rows that do not fit collapse to
/// zero height at the bottom edge, so consumers share one tiny-terminal
/// contract instead of reimplementing `row_from_bottom` arithmetic.
#[must_use]
pub fn bottom_rows<const N: usize>(area: Rect, heights: [u16; N]) -> (Rect, [Rect; N]) {
    let mut allocated = [0_u16; N];
    let mut remaining = area.height;
    for index in (0..N).rev() {
        allocated[index] = heights[index].min(remaining);
        remaining = remaining.saturating_sub(allocated[index]);
    }
    let rows_height = area.height.saturating_sub(remaining);
    let body = Rect::new(area.x, area.y, area.width, area.height - rows_height);
    let mut y = body.bottom();
    let rows = std::array::from_fn(|index| {
        let height = allocated[index];
        let row = Rect::new(area.x, y, area.width, height);
        y = y.saturating_add(height);
        row
    });
    (body, rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_resolution_centers_and_honors_margins() {
        let rect = resolve_dialog(
            Rect::new(10, 5, 100, 30),
            DialogSpec {
                min_width: 40,
                preferred_width: 60,
                max_width: 80,
                min_height: 8,
                preferred_height: 20,
                max_height: 24,
                horizontal_margin: 4,
                vertical_margin: 4,
                placement: Placement::Centered,
            },
        );
        assert_eq!(rect, Rect::new(30, 10, 60, 20));
    }

    #[test]
    fn top_dialog_keeps_the_outer_origin() {
        let rect = resolve_dialog(
            Rect::new(7, 3, 20, 10),
            DialogSpec {
                min_width: 8,
                preferred_width: 12,
                max_width: 16,
                min_height: 4,
                preferred_height: 6,
                max_height: 8,
                horizontal_margin: 0,
                vertical_margin: 0,
                placement: Placement::Top,
            },
        );
        assert_eq!(rect, Rect::new(11, 3, 12, 6));
    }

    #[test]
    fn dialog_margins_are_axis_independent() {
        let rect = resolve_dialog(
            Rect::new(0, 0, 20, 10),
            DialogSpec {
                min_width: 0,
                preferred_width: 20,
                max_width: 20,
                min_height: 0,
                preferred_height: 10,
                max_height: 10,
                horizontal_margin: 4,
                vertical_margin: 0,
                placement: Placement::Centered,
            },
        );
        assert_eq!(rect, Rect::new(2, 0, 16, 10));
    }

    #[test]
    fn bottom_rows_share_tiny_area_collapse_geometry() {
        let (body, rows) = bottom_rows(Rect::new(4, 2, 20, 2), [1, 1, 1]);
        assert_eq!(body, Rect::new(4, 2, 20, 0));
        assert_eq!(rows[0], Rect::new(4, 2, 20, 0));
        assert_eq!(rows[1], Rect::new(4, 2, 20, 1));
        assert_eq!(rows[2], Rect::new(4, 3, 20, 1));
    }

    #[test]
    fn centered_rect_stays_inside_tiny_and_large_inputs() {
        let outer = Rect::new(7, 11, 20, 10);
        assert_eq!(centered_rect(8, 4, outer), Rect::new(13, 14, 8, 4));
        for (width, height) in [(0, 0), (1, 1), (2, 2), (u16::MAX, u16::MAX)] {
            let rect = centered_rect(width, height, outer);
            assert!(rect.x >= outer.x && rect.y >= outer.y);
            assert!(rect.right() <= outer.right());
            assert!(rect.bottom() <= outer.bottom());
        }
    }
}
