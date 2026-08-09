// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Agent shell geometry recipe: stream + prompt + status + optional side rail.

use ratatui_core::layout::Rect;

use crate::{
    layout::{
        RegionId,
        RegionLayout,
        RegionSize,
        RegionSpec,
        SurfaceAxis,
        WorkSurface,
    },
    style::Density,
};

/// Named slots produced by [`layout_agent_shell`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentShellSlots {
    /// Optional side rail (file tree, sessions).
    pub rail: Option<Rect>,
    /// Main conversation / stream area.
    pub stream: Rect,
    /// Prompt input chrome.
    pub prompt: Rect,
    /// Status / token / hint strip.
    pub status: Rect,
}

/// Configuration for the agent shell recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentShellLayout {
    /// Overall density.
    pub density: Density,
    /// Side rail width; `0` hides the rail.
    pub rail_width: u16,
    /// Prompt height in rows.
    pub prompt_height: u16,
    /// Status strip height.
    pub status_height: u16,
}

impl Default for AgentShellLayout {
    fn default() -> Self {
        Self {
            density: Density::Comfortable,
            rail_width: 24,
            prompt_height: 4,
            status_height: 1,
        }
    }
}

/// Resolves agent shell rectangles inside `area`.
#[must_use]
pub fn layout_agent_shell(area: Rect, config: AgentShellLayout) -> AgentShellSlots {
    let (main, rail) = if config.rail_width == 0 || area.width <= config.rail_width {
        (area, None)
    } else {
        let surface = WorkSurface::new()
            .axis(SurfaceAxis::Horizontal)
            .density(Density::Dashboard)
            .regions([
                RegionSpec {
                    id: RegionId::from_static("rail"),
                    size: RegionSize::Fixed(config.rail_width),
                },
                RegionSpec {
                    id: RegionId::from_static("main"),
                    size: RegionSize::Weight(1),
                },
            ]);
        let layout = surface.layout(area);
        (layout[1].area, Some(layout[0].area))
    };

    let vertical = WorkSurface::new()
        .axis(SurfaceAxis::Vertical)
        .density(config.density)
        .regions([
            RegionSpec {
                id: RegionId::from_static("stream"),
                size: RegionSize::Weight(1),
            },
            RegionSpec {
                id: RegionId::from_static("prompt"),
                size: RegionSize::Fixed(config.prompt_height.max(1)),
            },
            RegionSpec {
                id: RegionId::from_static("status"),
                size: RegionSize::Fixed(config.status_height.max(1)),
            },
        ]);
    let regions: Vec<RegionLayout> = vertical.layout(main);
    AgentShellSlots {
        rail,
        stream: regions[0].area,
        prompt: regions[1].area,
        status: regions[2].area,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_shell_reserves_prompt_and_status() {
        let slots = layout_agent_shell(
            Rect::new(0, 0, 80, 24),
            AgentShellLayout {
                density: Density::Dashboard,
                rail_width: 20,
                prompt_height: 3,
                status_height: 1,
            },
        );
        assert_eq!(slots.rail.map(|r| r.width), Some(20));
        assert_eq!(slots.prompt.height, 3);
        assert_eq!(slots.status.height, 1);
        assert!(slots.stream.height >= 10);
        assert_eq!(
            slots.stream.height + slots.prompt.height + slots.status.height,
            24
        );
    }
}
