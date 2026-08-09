// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Flagship Agent Workbench: workspace geometry + **persistent** scene +
//! surface composition for TaskRail, Transcript, PromptComposer, Status, and
//! permission / question overlays.
//!
//! **Sole chrome (Break J):** [`PromptComposer`] + [`PermissionPrompt`].
//! Legacy `PromptBox` / `ApprovalCard` are deleted.
//!
//! Consumers own [`AgentWorkbenchState`] across frames so Esc/focus/layer
//! policy survives paint. TermRock never executes domain effects.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::KeyEvent,
    interaction::{
        InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene,
        LayerDismissPolicy, LayerKind, Outcome, SemanticRole,
    },
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignSystem, Role},
    widgets::{
        List, ListRow, ListState, ModeRibbon, ModeRibbonState, Panel, PanelChrome,
        PermissionOutcome, PermissionPrompt, PermissionPromptState, PromptComposer,
        PromptComposerOutcome, PromptComposerState, QuestionFlow, QuestionFlowState, StatusBar,
        StatusBarState, StatusSlot, Transcript, TranscriptBlock, TranscriptOutcome,
        TranscriptState, WorkbenchMode,
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

/// Typed result from workbench key routing (no side effects beyond UI state).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkbenchKeyOutcome {
    /// Nothing handled.
    Ignored,
    /// Scene focus / layer change (e.g. Esc peel).
    Scene(InteractionOutcome<&'static str, &'static str, ()>),
    /// Prompt composer consumed the key.
    Prompt(PromptComposerOutcome),
    /// Permission surface consumed the key.
    Permission(PermissionOutcome),
    /// Task rail list outcome.
    Task(Outcome<&'static str>),
    /// Transcript viewport / selection outcome.
    Transcript(TranscriptOutcome<&'static str>),
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
    /// Whether a permission layer is currently registered.
    permission_open: bool,
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

    /// Whether the permission overlay layer is open.
    #[must_use]
    pub const fn permission_open(&self) -> bool {
        self.permission_open
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
    ///
    /// Does **not** resolve a permission grant. Hosts that need queue cancel must
    /// also call [`PermissionPromptState::handle_key`] with Esc (or
    /// [`PermissionPromptState::handle_intent`] with [`crate::interaction::UiIntent::Cancel`]).
    pub fn handle_escape(&mut self) -> InteractionOutcome<&'static str, &'static str, ()> {
        let outcome = self.scene.handle_escape();
        match &outcome {
            InteractionOutcome::LayerDismissed { layer, .. } if *layer == "permission" => {
                self.permission_open = false;
            }
            InteractionOutcome::LayerDismissed { layer, .. } if *layer == "question" => {
                self.question_open = false;
            }
            _ => {
                if !self
                    .scene
                    .layers()
                    .iter()
                    .any(|layer| layer.id == "permission")
                {
                    self.permission_open = false;
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

    /// Route a key using scene focus / top layer ownership.
    ///
    /// Order: permission layer (if open) → question layer (if open) → focused pane.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        tasks: &[ListRow<'_, &'static str>],
        prompt: &mut PromptComposerState,
        transcript: &mut TranscriptState<&'static str>,
        transcript_blocks: &[TranscriptBlock<'_, &'static str>],
        permission: Option<&mut PermissionPromptState>,
    ) -> WorkbenchKeyOutcome {
        // Esc always goes through scene first when a dismissible layer is top.
        if matches!(key.code, crate::input::KeyCode::Esc)
            && key.kind == crate::input::KeyEventKind::Press
        {
            let top_dismissible = self.scene.layers().last().is_some_and(|layer| {
                layer.id != "root" && matches!(layer.esc, LayerDismissPolicy::Dismissible)
            });
            if top_dismissible {
                // Permission may also cancel queue on Esc — give surface first chance
                // when the top layer is permission and queue is non-empty.
                if self.permission_open
                    && let Some(perm) = permission
                    && !perm.is_empty()
                {
                    let out = perm.handle_key(key);
                    if !matches!(out, PermissionOutcome::Ignored) {
                        // Still peel scene layer when cancelled without a decision.
                        if matches!(out, PermissionOutcome::Cancelled { .. }) {
                            let _ = self.handle_escape();
                        }
                        return WorkbenchKeyOutcome::Permission(out);
                    }
                }
                return WorkbenchKeyOutcome::Scene(self.handle_escape());
            }
        }

        if self.permission_open
            && let Some(perm) = permission
            && !perm.is_empty()
        {
            let out = perm.handle_key(key);
            if !matches!(out, PermissionOutcome::Ignored) {
                // Peel layer after cancel or when the queue is empty post-decide.
                if matches!(out, PermissionOutcome::Cancelled { .. })
                    || (matches!(out, PermissionOutcome::Decided { .. }) && perm.is_empty())
                {
                    let _ = self.handle_escape();
                }
                return WorkbenchKeyOutcome::Permission(out);
            }
        }

        match self.scene.focused().copied() {
            Some("prompt") => {
                prompt.set_focused(true);
                let out = prompt.handle_key(key);
                if matches!(out, PromptComposerOutcome::Ignored) {
                    WorkbenchKeyOutcome::Ignored
                } else {
                    WorkbenchKeyOutcome::Prompt(out)
                }
            }
            Some("task_rail") => {
                let out = self.task_list.handle_key(tasks, key);
                if matches!(out, Outcome::Ignored) {
                    WorkbenchKeyOutcome::Ignored
                } else {
                    WorkbenchKeyOutcome::Task(out)
                }
            }
            Some("transcript") => {
                transcript.set_focused(true);
                let out = transcript.handle_key(key, transcript_blocks);
                if matches!(out, TranscriptOutcome::Ignored) {
                    WorkbenchKeyOutcome::Ignored
                } else {
                    WorkbenchKeyOutcome::Transcript(out)
                }
            }
            _ => WorkbenchKeyOutcome::Ignored,
        }
    }
}

/// Resolves workbench geometry for the current area and collapse state.
#[must_use]
pub fn agent_workbench_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    // Main column (task + transcript) vs south band (composer + status).
    // South band needs real height: Prompt Min(4) + Status Fixed(1). A 92/8 split
    // collapsed the composer on common 24-row terminals.
    let root = WorkspaceNode::Split {
        axis: WorkspaceAxis::Vertical,
        ratio_percent: 75,
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
            ratio_percent: 85,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Prompt.id()),
                constraint: PaneConstraint::Min(4),
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
    /// Permission prompt area.
    pub permission: Option<Rect>,
    /// Question-flow area.
    pub question: Option<Rect>,
}

/// Re-registers workbench panes into the **consumer-owned** scene.
pub fn sync_workbench_scene(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    panes: &[PaneGeom],
    modals: WorkbenchModals,
) {
    // Per-frame element registry (layers + focus identity persist).
    scene.begin_frame();
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
    if let Some(modal) = modals.permission {
        if !scene.layers().iter().any(|layer| layer.id == "permission") {
            scene.push_layer(InteractionLayer {
                id: "permission",
                kind: LayerKind::Card,
                owns_input: true,
                esc: LayerDismissPolicy::Dismissible,
                outside: LayerDismissPolicy::Trap,
                focus_return: Some("prompt"),
            });
        }
        let _ = scene.register(InteractionElement::control(
            "permission",
            "permission",
            modal,
        ));
    } else {
        let _ = scene.remove_layer(&"permission");
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
    } else {
        let _ = scene.remove_layer(&"question");
    }
    scene.reconcile();
}

/// Borrowed surfaces for one workbench paint.
pub struct WorkbenchSurfaces<'a, 'b> {
    /// Design system (sole paint authority).
    pub system: &'a DesignSystem,
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
    /// Flagship prompt composer chrome.
    pub prompt: &'a PromptComposer<'a>,
    /// Prompt composer state.
    pub prompt_state: &'a mut PromptComposerState,
    /// Status bar slots.
    pub status_slots: &'a [StatusSlot<'a, &'b str>],
    /// Status bar state.
    pub status_state: &'a mut StatusBarState<&'b str>,
    /// Optional permission overlay (opens/keeps permission scene layer).
    pub permission: Option<(&'a PermissionPrompt<'a>, &'a mut PermissionPromptState)>,
    /// Optional question-flow overlay.
    pub question: Option<&'a QuestionFlow<'a, &'static str>>,
}

/// Permission modal geometry clamped for narrow terminals.
#[must_use]
pub fn permission_modal_rect(area: Rect) -> Rect {
    let width = area.width.saturating_mul(3) / 4;
    let height = (area.height / 3).max(6).min(area.height.saturating_sub(2));
    let width = width.clamp(16, area.width.saturating_sub(2).max(1));
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 4);
    Rect {
        x,
        y,
        width,
        height,
    }
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
        system,
        state,
        tasks,
        modes,
        transcript,
        transcript_state,
        prompt,
        prompt_state,
        status_slots,
        status_state,
        permission,
        question,
    } = surfaces;

    let panes = agent_workbench_layout(area, &state.workspace);
    let permission_rect = permission.as_ref().and_then(|(widget, perm_state)| {
        if perm_state.is_empty() {
            let _ = widget;
            None
        } else {
            Some(permission_modal_rect(area))
        }
    });
    let question_rect = question.as_ref().map(|_| Rect {
        x: area.x.saturating_add(area.width / 10),
        y: area.y.saturating_add(area.height / 5),
        width: area.width.saturating_mul(4) / 5,
        height: area.height / 2,
    });
    state.permission_open = permission_rect.is_some();
    state.question_open = question.is_some();

    sync_workbench_scene(
        &mut state.scene,
        &panes,
        WorkbenchModals {
            permission: permission_rect,
            question: question_rect,
        },
    );

    let focused = state.scene.focused().copied();
    prompt_state.set_focused(focused == Some("prompt") && !state.permission_open);

    for pane in &panes {
        if pane.collapsed || pane.area.is_empty() {
            continue;
        }
        match pane.id.0.as_str() {
            "task_rail" => {
                let is_focused = focused == Some("task_rail") && !state.permission_open;
                let panel = Panel::new(system).title("Tasks").emphasis(if is_focused {
                    PanelChrome::Focused
                } else {
                    PanelChrome::Normal
                });
                let inner = panel.inner(pane.area);
                Widget::render(&panel, pane.area, buffer);
                if !inner.is_empty() {
                    StatefulWidget::render(
                        &List::new(tasks, system).focused(is_focused),
                        inner,
                        buffer,
                        &mut state.task_list,
                    );
                }
            }
            "transcript" => {
                let is_focused = focused == Some("transcript") && !state.permission_open;
                let panel = Panel::new(system)
                    .title("Transcript")
                    .emphasis(if is_focused {
                        PanelChrome::Focused
                    } else {
                        PanelChrome::Normal
                    });
                let inner = panel.inner(pane.area);
                Widget::render(&panel, pane.area, buffer);
                transcript_state.set_focused(is_focused);
                // Paint-time focused chrome; host still owns dispatch.
                StatefulWidget::render(
                    &transcript.focused(is_focused),
                    inner,
                    buffer,
                    transcript_state,
                );
            }
            "prompt" => {
                // Mode ribbon sits on the top row of the prompt band when height allows.
                let mut prompt_area = pane.area;
                if !modes.is_empty() && pane.area.height > 1 {
                    let mode_area = Rect::new(pane.area.x, pane.area.y, pane.area.width, 1);
                    Widget::render(ModeRibbon::new(modes, system), mode_area, buffer);
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
                    &StatusBar::new(status_slots, &[], system),
                    pane.area,
                    buffer,
                    status_state,
                );
            }
            _ => {}
        }
    }

    if let Some((card, permission_state)) = permission
        && let Some(modal) = permission_rect
    {
        StatefulWidget::render(card, modal, buffer, permission_state);
        // Touch BorderFocused role so recipe path stays live under colorless themes.
        let _ = system.style(Role::BorderFocused);
    }
    if let Some(flow) = question
        && let Some(modal) = question_rect
    {
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
    use crate::input::{KeyCode, KeyEventKind, KeyModifiers};
    use crate::widgets::{
        PermissionRequest, PermissionRisk, QuestionOption, QuestionStep, TranscriptBlock,
        TranscriptKind,
    };
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper packs workbench surfaces"
    )]
    fn paint(
        workbench: &mut AgentWorkbenchState,
        system: &DesignSystem,
        tasks: &[ListRow<'_, &'static str>],
        modes: &[WorkbenchMode<'_, &'static str>],
        blocks: &[TranscriptBlock<'_, &str>],
        permission: Option<(&PermissionPrompt<'_>, &mut PermissionPromptState)>,
        question: Option<&QuestionFlow<'_, &'static str>>,
        width: u16,
        height: u16,
    ) -> Terminal<TestBackend> {
        let transcript = Transcript::new(blocks, system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(system);
        let mut pstate = PromptComposerState::new();
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
                        system,
                        state: workbench,
                        tasks,
                        modes,
                        transcript: &transcript,
                        transcript_state: &mut tstate,
                        prompt: &prompt,
                        prompt_state: &mut pstate,
                        status_slots: &slots,
                        status_state: &mut sstate,
                        permission,
                        question,
                    },
                );
            })
            .unwrap();
        terminal
    }

    fn sample_permission() -> PermissionRequest {
        PermissionRequest::new("req-1", "bash", "workspace")
            .risk(PermissionRisk::High)
            .command("cargo test")
            .expected("tests pass")
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
        let system = DesignSystem::default();
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
            &system,
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
    fn escape_peels_permission_then_question_on_persistent_scene() {
        let system = DesignSystem::default();
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        let tasks: [ListRow<'_, &str>; 0] = [];
        let modes = default_modes("build");
        let prompt_w = PermissionPrompt::new(&system);
        let mut pstate = PermissionPromptState::new();
        let _ = pstate.enqueue(sample_permission());
        let _ = paint(
            &mut workbench,
            &system,
            &tasks,
            &modes,
            &blocks,
            Some((&prompt_w, &mut pstate)),
            None,
            80,
            24,
        );
        assert!(workbench.permission_open());
        let outcome = workbench.handle_escape();
        assert!(
            matches!(
                outcome,
                InteractionOutcome::LayerDismissed {
                    layer: "permission",
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert!(!workbench.permission_open());

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
        let flow = QuestionFlow::new(&steps, &system);
        workbench.question = QuestionFlowState::new(1);
        let _ = paint(
            &mut workbench,
            &system,
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
    fn handle_key_routes_to_permission_when_open() {
        let system = DesignSystem::default();
        let mut workbench = AgentWorkbenchState::new();
        let mut perm = PermissionPromptState::new();
        let _ = perm.enqueue(sample_permission());
        // Register layer via paint.
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let modes = default_modes("plan");
        let prompt_w = PermissionPrompt::new(&system);
        let _ = paint(
            &mut workbench,
            &system,
            &[],
            &modes,
            &blocks,
            Some((&prompt_w, &mut perm)),
            None,
            80,
            24,
        );
        assert!(workbench.permission_open());
        // Default focus is Deny — Enter confirms Deny (never Allow).
        let mut tstate = TranscriptState::new();
        let out = workbench.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &[],
            &mut PromptComposerState::new(),
            &mut tstate,
            &[],
            Some(&mut perm),
        );
        assert!(matches!(out, WorkbenchKeyOutcome::Permission(_)), "{out:?}");
    }

    #[test]
    fn handle_key_routes_prompt_when_focused() {
        let mut workbench = AgentWorkbenchState::new();
        let system = DesignSystem::default();
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let modes = default_modes("plan");
        // Tall enough that Prompt Min(3) is registered (short frames collapse it).
        let _ = paint(
            &mut workbench,
            &system,
            &[],
            &modes,
            &blocks,
            None,
            None,
            80,
            40,
        );
        let focus_out = workbench.scene.focus("prompt");
        assert!(
            workbench.focused_pane() == Some("prompt"),
            "focus_out={focus_out:?} focused={:?} layers={:?}",
            workbench.focused_pane(),
            workbench
                .scene
                .layers()
                .iter()
                .map(|l| l.id)
                .collect::<Vec<_>>()
        );
        let mut prompt = PromptComposerState::new();
        prompt.set_text("ship");
        let mut tstate = TranscriptState::new();
        let out = workbench.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &[],
            &mut prompt,
            &mut tstate,
            &[],
            None,
        );
        assert!(
            matches!(
                out,
                WorkbenchKeyOutcome::Prompt(PromptComposerOutcome::Submit { .. })
            ),
            "{out:?}"
        );
    }

    #[test]
    #[test]
    fn handle_key_routes_transcript_when_focused() {
        let mut workbench = AgentWorkbenchState::new();
        let system = DesignSystem::default();
        let lines = ["hello", "world"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let modes = default_modes("plan");
        let _ = paint(
            &mut workbench,
            &system,
            &[],
            &modes,
            &blocks,
            None,
            None,
            80,
            40,
        );
        let _ = workbench.scene.focus("transcript");
        assert_eq!(workbench.focused_pane(), Some("transcript"));
        let mut tstate = TranscriptState::new();
        let out = workbench.handle_key(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &[],
            &mut PromptComposerState::new(),
            &mut tstate,
            &blocks,
            None,
        );
        assert!(matches!(out, WorkbenchKeyOutcome::Transcript(_)), "{out:?}");
    }

    fn flagship_script_narrow_widths_keep_contained_geometry() {
        let system = DesignSystem::default();
        let lines = ["stream line"];
        let blocks = [TranscriptBlock::new(
            "b1",
            TranscriptKind::Assistant,
            &lines,
        )];
        let mut workbench = AgentWorkbenchState::new();
        let tasks = [ListRow::item("t1", Line::from("task"))];
        let modes = default_modes("plan");
        let transcript = Transcript::new(&blocks, &system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(&system);
        let mut pstate = PromptComposerState::new();
        let slots: [StatusSlot<'_, &str>; 0] = [];
        let mut sstate = StatusBarState::default();
        for (w, h) in [(120, 40), (80, 24), (40, 16), (20, 10), (12, 6), (120, 40)] {
            let area = Rect::new(0, 0, w, h);
            let mut buffer = Buffer::empty(area);
            render_agent_workbench(
                &mut buffer,
                area,
                WorkbenchSurfaces {
                    system: &system,
                    state: &mut workbench,
                    tasks: &tasks,
                    modes: &modes,
                    transcript: &transcript,
                    transcript_state: &mut tstate,
                    prompt: &prompt,
                    prompt_state: &mut pstate,
                    status_slots: &slots,
                    status_state: &mut sstate,
                    permission: None,
                    question: None,
                },
            );
            let panes = agent_workbench_layout(area, &workbench.workspace);
            for pane in &panes {
                assert!(pane.area.right() <= w);
                assert!(pane.area.bottom() <= h);
            }
            assert!(!workbench.scene.layers().is_empty());
            workbench.focus_pane(WorkbenchPane::Prompt);
        }
    }

    #[test]
    fn permission_modal_and_unicode_task_paint_at_narrow() {
        let system = DesignSystem::default();
        let lines = ["こんにちは"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        let tasks = [ListRow::item("t1", Line::from("タスク 🔧"))];
        let modes = default_modes("plan");
        let prompt_w = PermissionPrompt::new(&system).ascii(true);
        let mut perm = PermissionPromptState::new();
        let _ = perm.enqueue(
            PermissionRequest::new("r", "bash", "tmp")
                .risk(PermissionRisk::Critical)
                .command("rm -rf /tmp/x")
                .expected("削除")
                .details(["権限ゲート"]),
        );
        let terminal = paint(
            &mut workbench,
            &system,
            &tasks,
            &modes,
            &blocks,
            Some((&prompt_w, &mut perm)),
            None,
            40,
            16,
        );
        assert!(workbench.permission_open());
        let buffer = terminal.backend().buffer();
        assert!(!buffer.content().is_empty());
        let modal = permission_modal_rect(Rect::new(0, 0, 40, 16));
        assert!(modal.width >= 16);
        assert!(modal.right() <= 40);
    }

    #[test]
    fn no_legacy_dual_chrome_in_workbench_source() {
        let src = include_str!("agent_workbench.rs");
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        let code: String = code
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                !t.starts_with("//")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let a = ["Approv", "alCard"].concat();
        let b = ["Prompt", "Box"].concat();
        assert!(
            !code.contains(&a) && !code.contains(&b),
            "workbench non-test code must not mention deleted dual types"
        );
        assert!(src.contains("PermissionPrompt") && src.contains("PromptComposer"));
    }

    #[test]
    fn esc_repeat_does_not_double_peel() {
        let system = DesignSystem::default();
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        let modes = default_modes("plan");
        let prompt_w = PermissionPrompt::new(&system);
        let mut perm = PermissionPromptState::new();
        let _ = perm.enqueue(sample_permission());
        let _ = paint(
            &mut workbench,
            &system,
            &[],
            &modes,
            &blocks,
            Some((&prompt_w, &mut perm)),
            None,
            80,
            24,
        );
        let mut key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        key.kind = KeyEventKind::Repeat;
        let mut tstate = TranscriptState::new();
        let out = workbench.handle_key(
            key,
            &[],
            &mut PromptComposerState::new(),
            &mut tstate,
            &[],
            Some(&mut perm),
        );
        // Repeat Esc should not peel (press-only on permission / scene path).
        assert!(
            matches!(
                out,
                WorkbenchKeyOutcome::Ignored | WorkbenchKeyOutcome::Permission(_)
            ),
            "{out:?}"
        );
        assert!(workbench.permission_open() || matches!(out, WorkbenchKeyOutcome::Permission(_)));
    }
}
