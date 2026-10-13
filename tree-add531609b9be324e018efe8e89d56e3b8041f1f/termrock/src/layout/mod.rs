//! Responsive layout specifications and caller-defined bottom slots.

use ratatui::layout::Rect;

pub use crate::interaction::HitRegion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Centered,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogSpec {
    pub min_width: u16,
    pub preferred_width: u16,
    pub max_width: u16,
    pub min_height: u16,
    pub preferred_height: u16,
    pub max_height: u16,
    pub margin: u16,
    pub placement: Placement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slots {
    pub body: Rect,
    pub bottom: Rect,
}

impl Slots {
    #[must_use]
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
