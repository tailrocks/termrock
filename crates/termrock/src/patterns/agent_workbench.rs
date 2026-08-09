// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Flagship Agent Workbench: workspace geometry + scene registration + surface
//! composition for TaskRail, Transcript, Prompt, Status, and modal overlays.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    interaction::{
        InteractionElement, InteractionLayer, InteractionScene, LayerDismissPolicy, LayerKind,
        SemanticRole,
    },
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignTokens, Role},
    widgets::{
        ApprovalCard, ApprovalCardState, Panel, PanelEmphasis, PromptBox, PromptBoxState,
        StatusBar, StatusBarState, StatusSlot, Transcript, TranscriptState,
    },
};

/// Named panes of the default agent workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorkbenchPane {
    /// Task / subagent rail.
    TaskRail,
    /// Center transcript.
    Transcript,
    /// South prompt composer.
    Prompt,
    /// Status strip.
    Status,
}

impl WorkbenchPane {
    /// Stable pane id string.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TaskRail => "task_rail",
            Self::Transcript => "transcript",
            Self::Prompt => "prompt",
            Self::Status => "status",
        }
    }
}

/// Resolves workbench geometry for the current area and collapse state.
#[must_use]
pub fn agent_workbench_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    let root = WorkspaceNode::Split {
        axis: WorkspaceAxis::Vertical,
        ratio_percent: 92,
        first: Box::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Horizontal,
            ratio_percent: 22,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::TaskRail.id()),
                constraint: PaneConstraint::Min(12),
                collapse_priority: 0,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Transcript.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 2,
            }),
        }),
        second: Box::new(WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 70,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Prompt.id()),
                constraint: PaneConstraint::Min(3),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Status.id()),
                constraint: PaneConstraint::Fixed(1),
                collapse_priority: 3,
            }),
        }),
    };
    Workspace::new(root).layout(area, state)
}

/// Registers workbench panes into an interaction scene for the current frame.
pub fn register_workbench_scene(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    panes: &[PaneGeom],
) {
    scene.ensure_root(InteractionLayer {
        id: "root",
        kind: LayerKind::Root,
        owns_input: true,
        esc: LayerDismissPolicy::Ignore,
        outside: LayerDismissPolicy::Ignore,
        focus_return: None,
    });
    for pane in panes {
        if pane.collapsed || pane.area.width == 0 || pane.area.height == 0 {
            continue;
        }
        let id: &'static str = match pane.id.0.as_str() {
            "task_rail" => "task_rail",
            "transcript" => "transcript",
            "prompt" => "prompt",
            "status" => "status",
            _ => continue,
        };
        let _ = scene.register(
            InteractionElement::control(id, "root", pane.area)
                .role(SemanticRole::Control)
                .focusable(id != "status"),
        );
    }
    scene.reconcile();
}

/// Borrowed surfaces for one workbench paint.
pub struct WorkbenchSurfaces<'a, 'b> {
    /// Design tokens (canonical paint input).
    pub tokens: &'a DesignTokens,
    /// Workspace collapse/zoom state.
    pub workspace: &'a WorkspaceState,
    /// Transcript widget.
    pub transcript: &'a Transcript<'a, &'b str>,
    /// Transcript interaction state.
    pub transcript_state: &'a mut TranscriptState<&'b str>,
    /// Prompt composer.
    pub prompt: &'a PromptBox<'a>,
    /// Prompt state.
    pub prompt_state: &'a mut PromptBoxState,
    /// Status bar slots.
    pub status_slots: &'a [StatusSlot<'a, &'b str>],
    /// Status bar state.
    pub status_state: &'a mut StatusBarState<&'b str>,
    /// Optional approval overlay.
    pub approval: Option<(&'a ApprovalCard<'a>, &'a mut ApprovalCardState)>,
}

/// Paints a composed workbench frame from borrowed surfaces.
///
/// Domain wording/content remains consumer-owned; this only composes TermRock
/// chrome (panels, transcript, prompt, status, optional approval overlay).
pub fn render_agent_workbench(
    buffer: &mut Buffer,
    area: Rect,
    surfaces: WorkbenchSurfaces<'_, '_>,
) {
    let WorkbenchSurfaces {
        tokens,
        workspace,
        transcript,
        transcript_state,
        prompt,
        prompt_state,
        status_slots,
        status_state,
        approval,
    } = surfaces;
    let panes = agent_workbench_layout(area, workspace);
    let mut scene = InteractionScene::new();
    register_workbench_scene(&mut scene, &panes);

    for pane in &panes {
        if pane.collapsed || pane.area.is_empty() {
            continue;
        }
        match pane.id.0.as_str() {
            "task_rail" => {
                let panel = Panel::from_tokens(tokens).title("Tasks").emphasis(
                    if scene.focused() == Some(&"task_rail") {
                        PanelEmphasis::Focused
                    } else {
                        PanelEmphasis::Normal
                    },
                );
                let inner = panel.inner(pane.area);
                Widget::render(&panel, pane.area, buffer);
                // Consumer paints task rows into `inner`.
                let _ = inner;
            }
            "transcript" => {
                let panel = Panel::from_tokens(tokens).title("Transcript").emphasis(
                    if scene.focused() == Some(&"transcript") {
                        PanelEmphasis::Focused
                    } else {
                        PanelEmphasis::Normal
                    },
                );
                let inner = panel.inner(pane.area);
                Widget::render(&panel, pane.area, buffer);
                StatefulWidget::render(transcript, inner, buffer, transcript_state);
            }
            "prompt" => {
                StatefulWidget::render(prompt, pane.area, buffer, prompt_state);
            }
            "status" => {
                StatefulWidget::render(
                    &StatusBar::new(status_slots, &[], &tokens.theme),
                    pane.area,
                    buffer,
                    status_state,
                );
            }
            _ => {}
        }
    }

    if let Some((card, state)) = approval {
        // Modal layer: trap Esc under InteractionScene policy when registered.
        scene.push_layer(InteractionLayer {
            id: "approval",
            kind: LayerKind::Card,
            owns_input: true,
            esc: LayerDismissPolicy::Trap,
            outside: LayerDismissPolicy::Trap,
            focus_return: Some("prompt"),
        });
        let modal = Rect {
            x: area.x.saturating_add(area.width / 8),
            y: area.y.saturating_add(area.height / 6),
            width: area.width.saturating_mul(3) / 4,
            height: area.height / 3,
        };
        let _ = scene.register(InteractionElement::control("approval", "approval", modal));
        scene.reconcile();
        StatefulWidget::render(card, modal, buffer, state);
        let _ = tokens.theme.style(Role::BorderFocused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{TranscriptBlock, TranscriptKind};
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    #[test]
    fn workbench_rects_are_contained() {
        let state = WorkspaceState::new();
        let area = Rect::new(0, 0, 80, 24);
        let panes = agent_workbench_layout(area, &state);
        assert!(!panes.is_empty());
        for pane in panes {
            assert!(pane.area.right() <= area.right());
            assert!(pane.area.bottom() <= area.bottom());
        }
    }

    #[test]
    fn composed_workbench_paints_transcript_and_prompt() {
        let tokens = DesignTokens::default();
        let lines = ["hello", "world"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let transcript = Transcript::new(&blocks, &tokens.theme);
        let mut tstate = TranscriptState::new();
        let prompt = PromptBox::new(&tokens.theme);
        let mut pstate = PromptBoxState::new();
        let slots = [StatusSlot {
            id: "s",
            content: "ready",
            priority: 0,
            min_width: 0,
            enabled: true,
            style: ratatui_core::style::Style::default(),
            hover_style: None,
        }];
        let mut sstate = StatusBarState::default();
        let workspace = WorkspaceState::new();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_agent_workbench(
                    f.buffer_mut(),
                    area,
                    WorkbenchSurfaces {
                        tokens: &tokens,
                        workspace: &workspace,
                        transcript: &transcript,
                        transcript_state: &mut tstate,
                        prompt: &prompt,
                        prompt_state: &mut pstate,
                        status_slots: &slots,
                        status_state: &mut sstate,
                        approval: None,
                    },
                );
            })
            .unwrap();
    }
}
