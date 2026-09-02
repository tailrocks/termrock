// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Multi-region work surface layout (lazygit/k9s class shells).
use ratatui_core::layout::Rect;

/// Named region identity for focus and hit registration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RegionId(pub String);

impl RegionId {
    /// Creates a region id from a static name.
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self(id.to_owned())
    }
}

/// How a region claims space along the primary axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegionSize {
    /// Fixed rows/cols.
    Fixed(u16),
    /// Weighted share of remaining space.
    Weight(u16),
    /// Collapsed to zero (remembered weight preserved by caller).
    Collapsed,
}

/// One region specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionSpec {
    /// Stable identity.
    pub id: RegionId,
    /// Size policy.
    pub size: RegionSize,
}

/// Axis for stacking regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum SurfaceAxis {
    /// Stack top → bottom (default for main/detail shells).
    #[default]
    Vertical,
    /// Stack left → right.
    Horizontal,
}

/// Resolved rectangle for one region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionLayout {
    /// Region identity.
    pub id: RegionId,
    /// Painted rectangle (may be empty when collapsed).
    pub area: Rect,
}

/// Multi-region surface layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkSurface {
    axis: SurfaceAxis,
    regions: Vec<RegionSpec>,
}

impl WorkSurface {
    /// Creates an empty vertical surface.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            axis: SurfaceAxis::Vertical,
            regions: Vec::new(),
        }
    }

    /// Sets stack axis.
    #[must_use]
    pub const fn axis(mut self, axis: SurfaceAxis) -> Self {
        self.axis = axis;
        self
    }

    /// Replaces region specifications (bottom→top / left→right order).
    #[must_use]
    pub fn regions(mut self, regions: impl IntoIterator<Item = RegionSpec>) -> Self {
        self.regions = regions.into_iter().collect();
        self
    }

    /// Resolves region rectangles inside `area`.
    #[must_use]
    pub fn layout(&self, area: Rect) -> Vec<RegionLayout> {
        if area.is_empty() || self.regions.is_empty() {
            return self
                .regions
                .iter()
                .map(|region| RegionLayout {
                    id: region.id.clone(),
                    area: Rect::new(area.x, area.y, 0, 0),
                })
                .collect();
        }

        let gap = crate::style::SpacingScale::junie().gap;
        let primary = match self.axis {
            SurfaceAxis::Vertical => area.height,
            SurfaceAxis::Horizontal => area.width,
        };
        let visible_regions = self
            .regions
            .iter()
            .filter(|region| !matches!(region.size, RegionSize::Collapsed))
            .count();
        let gaps = gap.saturating_mul(visible_regions.saturating_sub(1) as u16);
        let mut fixed = 0u16;
        let mut weight_sum = 0u32;
        for region in &self.regions {
            match region.size {
                RegionSize::Fixed(n) => fixed = fixed.saturating_add(n),
                RegionSize::Weight(w) => {
                    weight_sum = weight_sum.saturating_add(u32::from(w.max(1)))
                }
                RegionSize::Collapsed => {}
            }
        }
        let available = primary.saturating_sub(fixed).saturating_sub(gaps);
        let mut sizes = Vec::with_capacity(self.regions.len());
        let mut remaining_weight = weight_sum;
        let mut remaining_flex = available;
        for (index, region) in self.regions.iter().enumerate() {
            let size = match region.size {
                RegionSize::Fixed(n) => n.min(primary),
                RegionSize::Collapsed => 0,
                RegionSize::Weight(w) => {
                    let w = u32::from(w.max(1));
                    if index + 1 == self.regions.len() || remaining_weight == 0 {
                        remaining_flex
                    } else {
                        let share =
                            (u32::from(remaining_flex) * w / remaining_weight.max(1)) as u16;
                        remaining_weight = remaining_weight.saturating_sub(w);
                        remaining_flex = remaining_flex.saturating_sub(share);
                        share
                    }
                }
            };
            sizes.push(size);
        }

        let mut cursor = match self.axis {
            SurfaceAxis::Vertical => area.y,
            SurfaceAxis::Horizontal => area.x,
        };
        let mut out = Vec::with_capacity(self.regions.len());
        let mut seen_visible = false;
        for (index, region) in self.regions.iter().enumerate() {
            let size = sizes[index];
            let visible = !matches!(region.size, RegionSize::Collapsed);
            if visible && seen_visible {
                cursor = cursor.saturating_add(gap);
            }
            let rect = match self.axis {
                SurfaceAxis::Vertical => Rect::new(area.x, cursor, area.width, size),
                SurfaceAxis::Horizontal => Rect::new(cursor, area.y, size, area.height),
            };
            out.push(RegionLayout {
                id: region.id.clone(),
                area: rect,
            });
            if visible {
                cursor = cursor.saturating_add(size);
                seen_visible = true;
            }
        }
        out
    }
}

impl Default for WorkSurface {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertical_fixed_and_weight_fill_height() {
        let surface = WorkSurface::new().regions([
            RegionSpec {
                id: RegionId::from_static("header"),
                size: RegionSize::Fixed(2),
            },
            RegionSpec {
                id: RegionId::from_static("body"),
                size: RegionSize::Weight(1),
            },
            RegionSpec {
                id: RegionId::from_static("footer"),
                size: RegionSize::Fixed(1),
            },
        ]);
        let layout = surface.layout(Rect::new(0, 0, 40, 20));
        assert_eq!(layout[0].area, Rect::new(0, 0, 40, 2));
        assert_eq!(layout[1].area, Rect::new(0, 4, 40, 13));
        assert_eq!(layout[2].area, Rect::new(0, 19, 40, 1));
    }

    #[test]
    fn collapsed_region_is_zero_sized() {
        let surface = WorkSurface::new().regions([
            RegionSpec {
                id: RegionId::from_static("side"),
                size: RegionSize::Collapsed,
            },
            RegionSpec {
                id: RegionId::from_static("main"),
                size: RegionSize::Weight(1),
            },
        ]);
        let layout = surface.layout(Rect::new(0, 0, 10, 10));
        assert_eq!(layout[0].area.height, 0);
        assert_eq!(layout[1].area, Rect::new(0, 0, 10, 10));
    }
}
