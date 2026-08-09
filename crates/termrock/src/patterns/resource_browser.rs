// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Resource browser recipe: tree/list rail + detail + optional preview.

use ratatui_core::layout::Rect;

use crate::{
    layout::{RegionId, RegionSize, RegionSpec, SurfaceAxis, WorkSurface},
    style::Density,
};

/// Slots for a resource browser (file manager / k8s / DB class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBrowserSlots {
    /// Navigation rail (tree or list).
    pub rail: Rect,
    /// Primary detail / table.
    pub detail: Rect,
    /// Optional preview pane (None when `preview_width == 0`).
    pub preview: Option<Rect>,
    /// Status / hints.
    pub status: Rect,
}

/// Layout knobs for [`layout_resource_browser`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBrowserLayout {
    /// Density.
    pub density: Density,
    /// Left rail width.
    pub rail_width: u16,
    /// Right preview width; 0 hides preview.
    pub preview_width: u16,
    /// Status height.
    pub status_height: u16,
}

impl Default for ResourceBrowserLayout {
    fn default() -> Self {
        Self {
            density: Density::Compact,
            rail_width: 28,
            preview_width: 32,
            status_height: 1,
        }
    }
}

/// Resolves resource browser rectangles.
#[must_use]
pub fn layout_resource_browser(area: Rect, config: ResourceBrowserLayout) -> ResourceBrowserSlots {
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

    let mut specs = vec![RegionSpec {
        id: RegionId::from_static("rail"),
        size: RegionSize::Fixed(config.rail_width.max(1).min(body.width.saturating_sub(4))),
    }];
    specs.push(RegionSpec {
        id: RegionId::from_static("detail"),
        size: RegionSize::Weight(1),
    });
    if config.preview_width > 0
        && body.width
            > config
                .rail_width
                .saturating_add(config.preview_width)
                .saturating_add(4)
    {
        specs.push(RegionSpec {
            id: RegionId::from_static("preview"),
            size: RegionSize::Fixed(config.preview_width),
        });
    }

    let surface = WorkSurface::new()
        .axis(SurfaceAxis::Horizontal)
        .density(config.density)
        .regions(specs);
    let regions = surface.layout(body);
    let preview = regions.get(2).map(|r| r.area);
    ResourceBrowserSlots {
        rail: regions[0].area,
        detail: regions[1].area,
        preview,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_browser_optional_preview() {
        let with_preview =
            layout_resource_browser(Rect::new(0, 0, 120, 40), ResourceBrowserLayout::default());
        assert!(with_preview.preview.is_some());
        let no_preview = layout_resource_browser(
            Rect::new(0, 0, 120, 40),
            ResourceBrowserLayout {
                preview_width: 0,
                ..ResourceBrowserLayout::default()
            },
        );
        assert!(no_preview.preview.is_none());
    }
}
