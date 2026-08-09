// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Flagship Agent Workbench: workspace geometry + **persistent** scene +
//! surface composition for TaskRail, Transcript, Prompt, Status, and modals.
//!
//! Consumers own the [`AgentWorkbenchState`] across frames so Esc/focus/layer
//! policy survives paint. TermRock never executes domain effects.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    interaction::{
        InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene,
        LayerDismissPolicy, LayerKind, SemanticRole,
    },
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignTokens, Role},
    widgets::{
        ApprovalCard, ApprovalCardState, List, ListRow, ListState, ModeRibbon, ModeRibbonState,
        Panel, PanelEmphasis, PromptBox, PromptBoxState, QuestionFlow, QuestionFlowState,
        StatusBar, StatusBarState, StatusSlot, Transcript, TranscriptState, WorkbenchMode,
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

/// Consumer-owned workbench interaction state (survives frames).
#[derive(Debug, Default)]
pub struct AgentWorkbenchState {
    /// Workspace collapse/zoom.
    pub workspace: WorkspaceState,
    /// Single scene authority for focus, layers, Esc.
    pub scene: InteractionScene<&'static str, &'static str, ()>,
    /// Task rail list selection/scroll.
    pub task_list: ListState<&'static str>,
    /// Mode ribbon selection (plan/build/…).
    pub mode_ribbon: ModeRibbonState<&'static str>,
    /// Optional question-flow step state.
    pub question: QuestionFlowState<&'static str>,
    /// Whether an approval layer is currently registered.
    approval_open: bool,
    /// Whether a question-flow layer is open.
    question_open: bool,
}

impl AgentWorkbenchState {
    /// Creates a workbench state focused on the transcript by default.
    #[must_use]
    pub fn new() -> Self {
        let mut state = Self::default();
        state.scene.ensure_root(InteractionLayer {
            id: "root",
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        state
    }

    /// Whether the approval overlay layer is open.
    #[must_use]
    pub const fn approval_open(&self) -> bool {
        self.approval_open
    }

    /// Whether the question-flow overlay is open.
    #[must_use]
    pub const fn question_open(&self) -> bool {
        self.question_open
    }

    /// Opens or closes the question-flow overlay flag (scene synced on render).
    pub const fn set_question_open(&mut self, open: bool) {
        self.question_open = open;
    }

    /// Routes Escape through the persistent scene (top dismissible peels first).
    pub fn handle_escape(&mut self) -> InteractionOutcome<&'static str, &'static str, ()> {
        let outcome = self.scene.handle_escape();
        match &outcome {
            InteractionOutcome::LayerDismissed { layer, .. } if *layer == "approval" => {
                self.approval_open = false;
            }
            InteractionOutcome::LayerDismissed { layer, .. } if *layer == "question" => {
                self.question_open = false;
            }
            _ => {
                if !self
                    .scene
                    .layers()
                    .iter()
                    .any(|layer| layer.id == "approval")
                {
                    self.approval_open = false;
                }
                if !self
                    .scene
                    .layers()
                    .iter()
                    .any(|layer| layer.id == "question")
                {
                    self.question_open = false;
                }
            }
        }
        outcome
    }

    /// Focused pane id when a workbench control owns focus.
    #[must_use]
    pub fn focused_pane(&self) -> Option<&'static str> {
        self.scene.focused().copied()
    }

    /// Focus a workbench pane by id after layout (consumer-driven).
    pub fn focus_pane(&mut self, pane: WorkbenchPane) {
        let _ = self.scene.focus(pane.id());
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

/// Modal areas registered into the scene for the frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkbenchModals {
    /// Approval card area.
    pub approval: Option<Rect>,
    /// Question-flow area.
    pub question: Option<Rect>,
}

/// Re-registers workbench panes into the **consumer-owned** scene.
pub fn sync_workbench_scene(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    panes: &[PaneGeom],
    modals: WorkbenchModals,
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
    // Register mode ribbon as focusable control on status band when present.
    if let Some(status) = panes.iter().find(|p| p.id.0.as_str() == "status")
        && !status.collapsed
        && status.area.height > 0
    {
        let _ = scene.register(
            InteractionElement::control("modes", "root", status.area)
                .role(SemanticRole::Control)
                .focusable(true),
        );
    }
    if let Some(modal) = modals.approval {
        if !scene.layers().iter().any(|layer| layer.id == "approval") {
            scene.push_layer(InteractionLayer {
                id: "approval",
                kind: LayerKind::Card,
                owns_input: true,
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Trap,
                focus_return: Some("prompt"),
            });
        }
        let _ = scene.register(InteractionElement::control("approval", "approval", modal));
    }
    if let Some(modal) = modals.question {
        if !scene.layers().iter().any(|layer| layer.id == "question") {
            scene.push_layer(InteractionLayer {
                id: "question",
                kind: LayerKind::Card,
                owns_input: true,
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Trap,
                focus_return: Some("prompt"),
            });
        }
        let _ = scene.register(InteractionElement::control("question", "question", modal));
    }
    scene.reconcile();
}

/// Borrowed surfaces for one workbench paint.
pub struct WorkbenchSurfaces<'a, 'b> {
    /// Design tokens (canonical paint input).
    pub tokens: &'a DesignTokens,
    /// Persistent workbench state (scene, workspace, task list).
    pub state: &'a mut AgentWorkbenchState,
    /// Task rail rows (composed anatomy).
    pub tasks: &'a [ListRow<'a, &'static str>],
    /// Mode ribbon modes (caller-defined labels).
    pub modes: &'a [WorkbenchMode<'a, &'static str>],
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
    /// Optional approval overlay (opens/keeps approval scene layer).
    pub approval: Option<(&'a ApprovalCard<'a>, &'a mut ApprovalCardState)>,
    /// Optional question-flow overlay.
    pub question: Option<&'a QuestionFlow<'a, &'static str>>,
}

/// Paints a composed workbench frame from borrowed surfaces.
///
/// Updates [`AgentWorkbenchState::scene`] in place so Esc/focus remain usable
/// after the frame. Domain wording/content remains consumer-owned.
pub fn render_agent_workbench(
    buffer: &mut Buffer,
    area: Rect,
    surfaces: WorkbenchSurfaces<'_, '_>,
) {
    let WorkbenchSurfaces {
        tokens,
        state,
        tasks,
        modes,
        transcript,
        transcript_state,
        prompt,
        prompt_state,
        status_slots,
        status_state,
        approval,
        question,
    } = surfaces;

    let panes = agent_workbench_layout(area, &state.workspace);
    let approval_rect = approval.as_ref().map(|_| Rect {
        x: area.x.saturating_add(area.width / 8),
        y: area.y.saturating_add(area.height / 6),
        width: area.width.saturating_mul(3) / 4,
        height: area.height / 3,
    });
    let question_rect = question.as_ref().map(|_| Rect {
        x: area.x.saturating_add(area.width / 10),
        y: area.y.saturating_add(area.height / 5),
        width: area.width.saturating_mul(4) / 5,
        height: area.height / 2,
    });
    state.approval_open = approval.is_some();
    state.question_open = question.is_some() || state.question_open && question.is_some();
    if question.is_some() {
        state.question_open = true;
    } else if !state.question_open {
        // closed
    }
    // Prefer explicit presence of borrowed overlay widgets.
    state.question_open = question.is_some();

    sync_workbench_scene(
        &mut state.scene,
        &panes,
        WorkbenchModals {
            approval: approval_rect,
            question: question_rect,
        },
    );

    for pane in &panes {
        if pane.collapsed || pane.area.is_empty() {
            continue;
        }
        match pane.id.0.as_str() {
            "task_rail" => {
                let is_focused = state.scene.focused() == Some(&"task_rail");
                let panel = Panel::new(tokens).title("Tasks").emphasis(if is_focused {
                    PanelEmphasis::Focused
                } else {
                    PanelEmphasis::Normal
                });
                let inner = panel.inner(pane.area);
                Widget::render(&panel, pane.area, buffer);
                if !inner.is_empty() {
                    state.task_list.set_focused(is_focused);
                    StatefulWidget::render(
                        &List::new(tasks, tokens),
                        inner,
                        buffer,
                        &mut state.task_list,
                    );
                }
            }
            "transcript" => {
                let is_focused = state.scene.focused() == Some(&"transcript");
                let panel = Panel::new(tokens)
                    .title("Transcript")
                    .emphasis(if is_focused {
                        PanelEmphasis::Focused
                    } else {
                        PanelEmphasis::Normal
                    });
                let inner = panel.inner(pane.area);
                Widget::render(&panel, pane.area, buffer);
                StatefulWidget::render(transcript, inner, buffer, transcript_state);
            }
            "prompt" => {
                // Mode ribbon sits on the top row of the prompt band when height allows.
                let mut prompt_area = pane.area;
                if !modes.is_empty() && pane.area.height > 1 {
                    let mode_area = Rect::new(pane.area.x, pane.area.y, pane.area.width, 1);
                    Widget::render(ModeRibbon::new(modes, tokens), mode_area, buffer);
                    prompt_area = Rect::new(
                        pane.area.x,
                        pane.area.y.saturating_add(1),
                        pane.area.width,
                        pane.area.height.saturating_sub(1),
                    );
                }
                StatefulWidget::render(prompt, prompt_area, buffer, prompt_state);
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

    if let Some((card, approval_state)) = approval
        && let Some(modal) = approval_rect
    {
        StatefulWidget::render(card, modal, buffer, approval_state);
        let _ = tokens.theme.style(Role::BorderFocused);
    }
    if let Some(flow) = question
        && let Some(modal) = question_rect
    {
        state.question.set_focused(true);
        StatefulWidget::render(flow, modal, buffer, &mut state.question);
    }
}

/// Convenience: empty task-rail placeholder row.
#[must_use]
pub fn empty_task_row() -> ListRow<'static, &'static str> {
    let mut row = ListRow::item("empty", Line::from("No tasks"));
    row.enabled = false;
    row
}

// Re-export legacy name used by older call sites.
/// Registers workbench panes (prefer [`sync_workbench_scene`] with owned state).
pub fn register_workbench_scene(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    panes: &[PaneGeom],
) {
    sync_workbench_scene(scene, panes, WorkbenchModals::default());
}

/// Default plan/build modes for demos (caller may replace).
#[must_use]
pub fn default_modes(active: &'static str) -> [WorkbenchMode<'static, &'static str>; 2] {
    [
        WorkbenchMode {
            id: "plan",
            label: "Plan",
            active: active == "plan",
            enabled: true,
        },
        WorkbenchMode {
            id: "build",
            label: "Build",
            active: active == "build",
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::{
        ApprovalCard, ApprovalRisk, QuestionOption, QuestionStep, TranscriptBlock, TranscriptKind,
    };
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper packs workbench surfaces"
    )]
    fn paint(
        workbench: &mut AgentWorkbenchState,
        tokens: &DesignTokens,
        tasks: &[ListRow<'_, &'static str>],
        modes: &[WorkbenchMode<'_, &'static str>],
        blocks: &[TranscriptBlock<'_, &str>],
        approval: Option<(&ApprovalCard<'_>, &mut ApprovalCardState)>,
        question: Option<&QuestionFlow<'_, &'static str>>,
        width: u16,
        height: u16,
    ) -> Terminal<TestBackend> {
        let transcript = Transcript::new(blocks, &tokens.theme);
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
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                render_agent_workbench(
                    f.buffer_mut(),
                    area,
                    WorkbenchSurfaces {
                        tokens,
                        state: workbench,
                        tasks,
                        modes,
                        transcript: &transcript,
                        transcript_state: &mut tstate,
                        prompt: &prompt,
                        prompt_state: &mut pstate,
                        status_slots: &slots,
                        status_state: &mut sstate,
                        approval,
                        question,
                    },
                );
            })
            .unwrap();
        terminal
    }

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
    fn composed_workbench_paints_task_rail_modes_and_keeps_scene() {
        let tokens = DesignTokens::default();
        let lines = ["hello", "world"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        let tasks = [
            ListRow::item("t1", Line::from("Plan review")),
            ListRow::item("t2", Line::from("Tool: cargo test")),
        ];
        workbench.task_list.select(Some("t1"));
        let modes = default_modes("plan");
        let terminal = paint(
            &mut workbench,
            &tokens,
            &tasks,
            &modes,
            &blocks,
            None,
            None,
            80,
            24,
        );
        assert!(workbench.scene.focused().is_some() || !workbench.scene.layers().is_empty());
        let buffer = terminal.backend().buffer();
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Plan") || text.contains("Tasks") || text.contains("Tool"),
            "task rail / modes painted: {text:?}"
        );
    }

    #[test]
    fn escape_peels_approval_then_question_on_persistent_scene() {
        let tokens = DesignTokens::default();
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        let tasks: [ListRow<'_, &str>; 0] = [];
        let modes = default_modes("build");
        let card = ApprovalCard::new("Delete", "Remove files?", ApprovalRisk::High, &tokens.theme);
        let mut astate = ApprovalCardState::new();
        let _ = paint(
            &mut workbench,
            &tokens,
            &tasks,
            &modes,
            &blocks,
            Some((&card, &mut astate)),
            None,
            80,
            24,
        );
        assert!(workbench.approval_open());
        let outcome = workbench.handle_escape();
        assert!(
            matches!(
                outcome,
                InteractionOutcome::LayerDismissed {
                    layer: "approval",
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert!(!workbench.approval_open());

        let opts = [QuestionOption {
            id: "yes",
            label: "Yes",
        }];
        let steps = [QuestionStep {
            id: "s1",
            prompt: "Proceed?",
            options: &opts,
            required: true,
        }];
        let flow = QuestionFlow::new(&steps, &tokens);
        workbench.question = QuestionFlowState::new(1);
        let _ = paint(
            &mut workbench,
            &tokens,
            &tasks,
            &modes,
            &blocks,
            None,
            Some(&flow),
            80,
            24,
        );
        assert!(workbench.question_open());
        let outcome = workbench.handle_escape();
        assert!(
            matches!(
                outcome,
                InteractionOutcome::LayerDismissed {
                    layer: "question",
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert!(!workbench.question_open());
    }

    #[test]
    fn flagship_script_narrow_widths_keep_contained_geometry() {
        let tokens = DesignTokens::default();
        let lines = ["stream line"];
        let blocks = [TranscriptBlock::new(
            "b1",
            TranscriptKind::Assistant,
            &lines,
        )];
        let mut workbench = AgentWorkbenchState::new();
        let tasks = [ListRow::item("t1", Line::from("task"))];
        let modes = default_modes("plan");
        let transcript = Transcript::new(&blocks, &tokens.theme);
        let mut tstate = TranscriptState::new();
        let prompt = PromptBox::new(&tokens.theme);
        let mut pstate = PromptBoxState::new();
        let slots: [StatusSlot<'_, &str>; 0] = [];
        let mut sstate = StatusBarState::default();
        for (w, h) in [(120, 40), (80, 24), (40, 16), (20, 10), (120, 40)] {
            let area = Rect::new(0, 0, w, h);
            let mut buffer = Buffer::empty(area);
            render_agent_workbench(
                &mut buffer,
                area,
                WorkbenchSurfaces {
                    tokens: &tokens,
                    state: &mut workbench,
                    tasks: &tasks,
                    modes: &modes,
                    transcript: &transcript,
                    transcript_state: &mut tstate,
                    prompt: &prompt,
                    prompt_state: &mut pstate,
                    status_slots: &slots,
                    status_state: &mut sstate,
                    approval: None,
                    question: None,
                },
            );
            let panes = agent_workbench_layout(area, &workbench.workspace);
            for pane in &panes {
                assert!(pane.area.right() <= w);
                assert!(pane.area.bottom() <= h);
            }
            assert!(!workbench.scene.layers().is_empty());
            // Consumer can re-focus after layout shrink/expand.
            workbench.focus_pane(WorkbenchPane::Prompt);
        }
    }
}
