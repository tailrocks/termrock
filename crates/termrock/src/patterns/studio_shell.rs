// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook / Studio shell geometry: preview + multi-panel design inspector.
//!
//! Product-neutral layout only. Story content and knobs stay caller-owned.
//! Built on AppShell Workbench (knobs = inspector rail) with a bottom
//! inspector band subdivided from main.

use ratatui_core::layout::Rect;

use crate::layout::{RegionId, RegionSize, SurfaceAxis, WorkSurface};
use crate::style::Density;

use super::app_shell::{AppShellConfig, AppShellRecipe, layout_app_shell};

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
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Workbench,
            density: config.density,
            header_height: 0,
            sidebar_width: 0,
            inspector_width: config.knobs_width,
            footer_height: config.status_height.max(1),
            command_height: 0,
            metrics_height: 0,
            log_height: 0,
            lifecycle: Default::default(),
            inline: false,
        },
    );

    let status = shell.footer.unwrap_or(Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1.min(area.height),
    });
    let knobs = shell.inspector;
    let body = shell.main;

    let inspector_h = config
        .inspector_height
        .min(body.height.saturating_sub(3))
        .max(1);
    let rows = WorkSurface::new()
        .axis(SurfaceAxis::Vertical)
        .density(config.density)
        .regions([
            crate::layout::RegionSpec {
                id: RegionId::from_static("preview"),
                size: RegionSize::Weight(1),
            },
            crate::layout::RegionSpec {
                id: RegionId::from_static("inspector"),
                size: RegionSize::Fixed(inspector_h),
            },
        ])
        .layout(body);

    StudioShellSlots {
        preview: rows[0].area,
        inspector: rows[1].area,
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
