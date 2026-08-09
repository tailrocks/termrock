// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ops dashboard recipe: metrics strip + main + log + status.
//!
//! Thin wrapper over [`crate::patterns::layout_app_shell`]
//! ([`AppShellRecipe::Dashboard`]).

use ratatui_core::layout::Rect;

use crate::style::Density;

use super::app_shell::{layout_app_shell, AppShellConfig, AppShellRecipe};

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
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Dashboard,
            density: config.density,
            header_height: 0,
            sidebar_width: 0,
            inspector_width: 0,
            footer_height: config.status_height.max(1),
            command_height: 0,
            metrics_height: config.metrics_height.max(1),
            log_height: config.log_height.max(1),
            lifecycle: Default::default(),
            inline: false,
        },
    );

    // Dashboard recipe may collapse log on narrow viewports — keep zero-height
    // placeholders so callers always get four rects.
    OpsDashboardSlots {
        metrics: shell.metrics.unwrap_or(Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 0,
        }),
        main: shell.main,
        log: shell.log.unwrap_or(Rect {
            x: area.x,
            y: shell.main.y.saturating_add(shell.main.height),
            width: area.width,
            height: 0,
        }),
        status: shell.footer.unwrap_or(Rect {
            x: area.x,
            y: area.y.saturating_add(area.height.saturating_sub(1)),
            width: area.width,
            height: 1.min(area.height),
        }),
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

    #[test]
    fn ops_dashboard_narrow_keeps_main() {
        let slots = layout_ops_dashboard(Rect::new(0, 0, 40, 20), OpsDashboardLayout::default());
        assert!(slots.main.height > 0);
        assert_eq!(
            slots.metrics.height + slots.main.height + slots.log.height + slots.status.height,
            20
        );
    }
}
