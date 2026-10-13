// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ops dashboard recipe: metrics strip + main + log + status.

use ratatui_core::layout::Rect;

use crate::{
    layout::{RegionId, RegionSize, RegionSpec, SurfaceAxis, WorkSurface},
    style::Density,
};

/// Slots for an ops-style dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsDashboardSlots {
    /// Top metrics / sparkline strip.
    pub metrics: Rect,
    /// Primary content (table, resource list).
    pub main: Rect,
    /// Log / event stream.
    pub log: Rect,
    /// Bottom status / hints.
    pub status: Rect,
}

/// Layout knobs for [`layout_ops_dashboard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpsDashboardLayout {
    /// Density.
    pub density: Density,
    /// Metrics strip height.
    pub metrics_height: u16,
    /// Log pane height.
    pub log_height: u16,
    /// Status height.
    pub status_height: u16,
}

impl Default for OpsDashboardLayout {
    fn default() -> Self {
        Self {
            density: Density::Dashboard,
            metrics_height: 3,
            log_height: 8,
            status_height: 1,
        }
    }
}

/// Resolves ops dashboard rectangles.
#[must_use]
pub fn layout_ops_dashboard(area: Rect, config: OpsDashboardLayout) -> OpsDashboardSlots {
    let surface = WorkSurface::new()
        .axis(SurfaceAxis::Vertical)
        .density(config.density)
        .regions([
            RegionSpec {
                id: RegionId::from_static("metrics"),
                size: RegionSize::Fixed(config.metrics_height.max(1)),
            },
            RegionSpec {
                id: RegionId::from_static("main"),
                size: RegionSize::Weight(2),
            },
            RegionSpec {
                id: RegionId::from_static("log"),
                size: RegionSize::Fixed(config.log_height.max(1)),
            },
            RegionSpec {
                id: RegionId::from_static("status"),
                size: RegionSize::Fixed(config.status_height.max(1)),
            },
        ]);
    let regions = surface.layout(area);
    OpsDashboardSlots {
        metrics: regions[0].area,
        main: regions[1].area,
        log: regions[2].area,
        status: regions[3].area,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_dashboard_fills_height() {
        let slots = layout_ops_dashboard(Rect::new(0, 0, 80, 30), OpsDashboardLayout::default());
        let sum = slots.metrics.height + slots.main.height + slots.log.height + slots.status.height;
        assert_eq!(sum, 30);
        assert!(slots.main.height >= slots.log.height);
    }
}
