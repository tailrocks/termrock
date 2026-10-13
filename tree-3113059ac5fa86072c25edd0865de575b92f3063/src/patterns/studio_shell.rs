// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook / Studio shell geometry: preview + multi-panel design inspector.
//!
//! Product-neutral layout only. Story content and knobs stay caller-owned.

use ratatui_core::layout::Rect;

use crate::layout::{RegionId, RegionSize, RegionSpec, SurfaceAxis, WorkSurface};
use crate::style::Density;

/// Studio shell regions for one frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioShellSlots {
    /// Main component preview.
    pub preview: Rect,
    /// Design inspector (focus/layers/tokens/recipes).
    pub inspector: Rect,
    /// Optional knobs column (None when width too narrow).
    pub knobs: Option<Rect>,
    /// Hint / status strip.
    pub status: Rect,
}

/// Layout knobs for [`layout_studio_shell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StudioShellLayout {
    /// Density.
    pub density: Density,
    /// Inspector height (bottom band).
    pub inspector_height: u16,
    /// Knobs width (right rail); 0 hides knobs.
    pub knobs_width: u16,
    /// Status height.
    pub status_height: u16,
}

impl Default for StudioShellLayout {
    fn default() -> Self {
        Self {
            density: Density::Compact,
            inspector_height: 4,
            knobs_width: 24,
            status_height: 1,
        }
    }
}

/// Resolves studio shell rectangles (preview + inspector + optional knobs).
#[must_use]
pub fn layout_studio_shell(area: Rect, config: StudioShellLayout) -> StudioShellSlots {
    let (body, status) = {
        let surface = WorkSurface::new()
            .axis(SurfaceAxis::Vertical)
            .density(Density::Dashboard)
            .regions([
                RegionSpec {
                    id: RegionId::from_static("body"),
                    size: RegionSize::Weight(1),
                },
                RegionSpec {
                    id: RegionId::from_static("status"),
                    size: RegionSize::Fixed(config.status_height.max(1)),
                },
            ]);
        let regions = surface.layout(area);
        (regions[0].area, regions[1].area)
    };

    let inspector_h = config
        .inspector_height
        .min(body.height.saturating_sub(3))
        .max(1);
    let (main, inspector) = {
        let surface = WorkSurface::new()
            .axis(SurfaceAxis::Vertical)
            .density(config.density)
            .regions([
                RegionSpec {
                    id: RegionId::from_static("main"),
                    size: RegionSize::Weight(1),
                },
                RegionSpec {
                    id: RegionId::from_static("inspector"),
                    size: RegionSize::Fixed(inspector_h),
                },
            ]);
        let regions = surface.layout(body);
        (regions[0].area, regions[1].area)
    };

    let show_knobs = config.knobs_width > 0 && main.width > config.knobs_width.saturating_add(28);
    let (preview, knobs) = if show_knobs {
        let surface = WorkSurface::new()
            .axis(SurfaceAxis::Horizontal)
            .density(config.density)
            .regions([
                RegionSpec {
                    id: RegionId::from_static("preview"),
                    size: RegionSize::Weight(1),
                },
                RegionSpec {
                    id: RegionId::from_static("knobs"),
                    size: RegionSize::Fixed(config.knobs_width),
                },
            ]);
        let regions = surface.layout(main);
        (regions[0].area, Some(regions[1].area))
    } else {
        (main, None)
    };

    StudioShellSlots {
        preview,
        inspector,
        knobs,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_shell_hides_knobs_when_narrow() {
        let wide = layout_studio_shell(Rect::new(0, 0, 120, 40), StudioShellLayout::default());
        assert!(wide.knobs.is_some());
        assert!(wide.inspector.height >= 1);
        let narrow = layout_studio_shell(Rect::new(0, 0, 40, 20), StudioShellLayout::default());
        assert!(narrow.knobs.is_none());
        assert!(narrow.preview.width > 0);
    }
}
