// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Agent shell geometry recipe: stream + prompt + status + optional side rail.
//!
//! Thin wrapper over [`crate::patterns::layout_app_shell`] (Workbench recipe
//! with command strip = prompt). Prefer AppShell directly for new hosts.
//!
//! Teaches: how to compose the agent shell's geometry — stream, prompt,
//! status, and an optional side rail — as slots a host paints into.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::layout::Rect;

use ratatui_core::{buffer::Buffer, widgets::StatefulWidget};

use crate::style::DesignSystem;
use crate::widgets::{
    PromptComposer, PromptComposerState, StatusBar, StatusBarState, StatusSlot, Transcript,
    TranscriptBlock, TranscriptState, Tree, TreeNode, TreeState,
};

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

// ── Reference paint ─────────────────────────────────────────────────────────

/// Which pane of the agent shell owns interaction this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentShellFocus {
    /// The side rail.
    Rail,
    /// The prompt composer.
    #[default]
    Prompt,
}

/// Host-owned content for one agent shell frame.
#[derive(Debug, Clone, Copy)]
pub struct AgentShellView<'a, BlockId, NodeId> {
    /// Conversation blocks.
    pub blocks: &'a [TranscriptBlock<'a, BlockId>],
    /// Optional rail nodes (files, sessions).
    pub rail: &'a [TreeNode<'a, NodeId>],
    /// Footer hints.
    pub hints: &'a [StatusSlot<'a, &'a str>],
}

/// Paints a reference agent shell over [`layout_agent_shell`]'s slots.
///
/// Rail is a [`Tree`], stream a [`Transcript`], prompt a
/// [`PromptComposer`], footer a [`StatusBar`]. Copy and swap.
pub fn paint_agent_shell<BlockId: Clone + Eq, NodeId: Clone + Eq>(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    config: AgentShellLayout,
    view: AgentShellView<'_, BlockId, NodeId>,
    focus: AgentShellFocus,
    rail_state: &mut TreeState<NodeId>,
    transcript_state: &mut TranscriptState<BlockId>,
    prompt_state: &mut PromptComposerState,
) -> AgentShellSlots {
    let slots = layout_agent_shell(area, config);

    if let Some(rail) = slots.rail
        && rail.height > 0
    {
        Tree::new(view.rail, system)
            .focused(matches!(focus, AgentShellFocus::Rail))
            .render(rail, buffer, rail_state);
    }

    if slots.stream.height > 0 {
        Transcript::new(view.blocks, system).render(slots.stream, buffer, transcript_state);
    }

    if slots.prompt.height > 0 {
        PromptComposer::new(system).paint(slots.prompt, buffer, prompt_state);
    }

    if slots.status.height > 0 {
        let mut status = StatusBarState::new();
        StatusBar::new(view.hints, &[], system).render(slots.status, buffer, &mut status);
    }

    slots
}

#[cfg(test)]
mod tests {

    #[test]
    fn reference_paint_fills_stream_prompt_and_status() {
        use crate::style::DesignSystem;
        use crate::widgets::{
            PromptComposerState, StatusSlot, TranscriptBlock, TranscriptKind, TranscriptState,
            TreeNode, TreeState,
        };
        use ratatui_core::buffer::Buffer;
        use ratatui_core::text::Line;

        let system = DesignSystem::default();
        let lines = ["how do I run the tests?"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let rail = [TreeNode::new("src", Line::from("src"), 0)];
        let hints = [StatusSlot::new("tab", "tab pane")];
        let view = AgentShellView {
            blocks: &blocks,
            rail: &rail,
            hints: &hints,
        };
        let mut rail_state = TreeState::new(Some("src"));
        let mut transcript_state = TranscriptState::new();
        let mut prompt_state = PromptComposerState::new();
        let area = Rect::new(0, 0, 80, 24);
        let mut buffer = Buffer::empty(area);
        let slots = paint_agent_shell(
            area,
            &mut buffer,
            &system,
            AgentShellLayout::default(),
            view,
            AgentShellFocus::Prompt,
            &mut rail_state,
            &mut transcript_state,
            &mut prompt_state,
        );

        let painted = |rect: Rect| {
            (rect.x..rect.right()).any(|x| {
                (rect.y..rect.bottom()).any(|y| !buffer[(x, y)].symbol().trim().is_empty())
            })
        };
        assert!(painted(slots.stream), "stream painted nothing");
        assert!(painted(slots.prompt), "prompt painted nothing");
        assert!(painted(slots.status), "status painted nothing");
    }
    use super::*;

    #[test]
    fn agent_shell_reserves_prompt_and_status() {
        let slots = layout_agent_shell(
            Rect::new(0, 0, 80, 24),
            AgentShellLayout {
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
            20
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
