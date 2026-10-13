// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Agent shell geometry recipe: stream + prompt + status + optional side rail.
//!
//! Thin wrapper over [`crate::patterns::layout_app_shell`] (Workbench recipe
//! with command strip = prompt). Prefer AppShell directly for new hosts.

use ratatui_core::layout::Rect;

use crate::style::Density;

use super::app_shell::{AppShellConfig, AppShellRecipe, layout_app_shell};

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
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Workbench,
            density: config.density,
            header_height: 0,
            sidebar_width: config.rail_width,
            inspector_width: 0,
            footer_height: config.status_height.max(1),
            command_height: config.prompt_height.max(1),
            metrics_height: 0,
            log_height: 0,
            lifecycle: Default::default(),
            inline: false,
        },
    );

    let prompt = shell.command.unwrap_or_else(|| {
        // Degenerate: reclaim last rows of main if command collapsed.
        let h = config.prompt_height.max(1).min(shell.main.height);
        Rect {
            x: shell.main.x,
            y: shell
                .main
                .y
                .saturating_add(shell.main.height.saturating_sub(h)),
            width: shell.main.width,
            height: h,
        }
    });
    let status = shell.footer.unwrap_or(Rect {
        x: area.x,
        y: area.y.saturating_add(area.height.saturating_sub(1)),
        width: area.width,
        height: 1.min(area.height),
    });
    let stream = if shell.command.is_some() {
        shell.main
    } else {
        Rect {
            x: shell.main.x,
            y: shell.main.y,
            width: shell.main.width,
            height: shell.main.height.saturating_sub(prompt.height),
        }
    };

    AgentShellSlots {
        rail: shell.sidebar,
        stream,
        prompt,
        status,
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

    #[test]
    fn agent_shell_hides_rail_when_zero() {
        let slots = layout_agent_shell(
            Rect::new(0, 0, 80, 24),
            AgentShellLayout {
                rail_width: 0,
                ..Default::default()
            },
        );
        assert!(slots.rail.is_none());
        assert_eq!(slots.stream.width, 80);
    }
}
