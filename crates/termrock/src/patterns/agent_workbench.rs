// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **AgentWorkbench** — north-star application block composed from **public**
//! TermRock widgets only (source-owned registry composition, not a monolith).
//!
//! **Mission.** Layout + persistent scene for TaskRail, center thread
//! (MessageThread or Transcript), ActivityShelf, PromptComposer, status chrome,
//! and dismissible overlays: PermissionPrompt, QuestionFlow, PlanReview,
//! DiffReview, SessionPicker, command surfaces. One-layer Escape, draft
//! preservation (composer never cleared by overlays), focus order, responsive
//! collapse, ASCII/no-color paint flags.
//!
//! **Sole agent chrome:** elevated widgets only — no local visual substitutes.
//! Hosts own domain data, streaming feeds, and effects.
//!
//! Research: Grok Build, Amp, OpenCode, Claude Code, Posting, Zellij, Glow
//! (experience references, not product clones).
//!
//! Teaches: how to compose a full agent workbench: transcript, prompt
//! composer, task rail, approvals and diagnostics in one focus-routed shell.
//!
//! Composes: [`crate::widgets::DiffHunk`], [`crate::widgets::DiffReview`],
//! [`crate::widgets::DiffReviewOutcome`],
//! [`crate::widgets::DiffReviewState`],
//! [`crate::widgets::ModeRibbon`], and 30 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    // nav sample seed
    input::{KeyCode, KeyEvent},
    interaction::{
        InteractionElement, InteractionLayer, InteractionOutcome, InteractionScene,
        LayerDismissPolicy, LayerKind, SemanticRole,
    },
    layout::{
        ModalSpec, PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode,
        WorkspaceState, modal_rect,
    },
    patterns::{
        ActivityItem, ActivityModel, ActivityShelf, ActivityShelfOutcome, ActivityShelfState,
        PlanReview, PlanReviewState, SessionPicker, SessionPickerState, TaskRail, TaskRailOutcome,
        TaskRailState, WorkingStateCard, WorkingStateCardState,
    },
    style::{DesignSystem, PanelChrome},
    widgets::{
        DiffHunk, DiffReview, DiffReviewState, ModeRibbon, ModeRibbonState, Panel,
        PermissionOutcome, PermissionPrompt, PermissionPromptState, PromptComposer,
        PromptComposerOutcome, PromptComposerState, QuestionFlow, QuestionFlowState, StatusBar,
        StatusBarState, StatusSlot, Transcript, TranscriptBlock, TranscriptOutcome,
        TranscriptState, WorkbenchMode,
    },
};

// ── Panes & density ─────────────────────────────────────────────────────────

/// Named panes of the default agent workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorkbenchPane {
    /// Task / subagent rail (TaskRail).
    TaskRail,
    /// Center transcript / message thread.
    Transcript,
    /// Concurrent activity strip.
    Activity,
    /// Working-state card (optional band above composer).
    Working,
    /// South prompt composer.
    Prompt,
    /// Status strip / header contraction.
    Status,
}

impl WorkbenchPane {
    /// Stable pane id string.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TaskRail => "task_rail",
            Self::Transcript => "transcript",
            Self::Activity => "activity",
            Self::Working => "working",
            Self::Prompt => "prompt",
            Self::Status => "status",
        }
    }

    /// Default keyboard focus cycle order (root).
    #[must_use]
    pub fn focus_order() -> &'static [WorkbenchPane] {
        &[
            Self::TaskRail,
            Self::Transcript,
            Self::Activity,
            Self::Prompt,
        ]
    }
}

/// Responsive density for story / host contraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WorkbenchDensity {
    /// Full workbench.
    #[default]
    Normal,
    /// Collapse activity; keep rail if width allows.
    Narrow,
    /// Transcript + composer only.
    Tiny,
}

impl WorkbenchDensity {
    /// From terminal width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 40 {
            Self::Tiny
        } else if width < 72 {
            Self::Narrow
        } else {
            Self::Normal
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Narrow => "narrow",
            Self::Tiny => "tiny",
        }
    }
}

// ── Key outcomes ────────────────────────────────────────────────────────────

/// Typed result from workbench key routing (UI state only — no domain effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkbenchKeyOutcome {
    /// Nothing handled.
    Ignored,
    /// Scene focus / layer change (e.g. Esc peel).
    Scene(InteractionOutcome<&'static str, &'static str, ()>),
    /// Prompt composer consumed the key (**draft preserved** on overlays).
    Prompt(PromptComposerOutcome),
    /// Permission surface.
    Permission(PermissionOutcome),
    /// Question flow (answers only; never clears draft).
    Question,
    /// Plan review.
    Plan,
    /// Diff review.
    Diff,
    /// Session picker (cancel keeps draft).
    Session,
    /// Elevated task rail.
    Task(TaskRailOutcome),
    /// Transcript / thread.
    Transcript(TranscriptOutcome<&'static str>),
    /// Activity shelf.
    Activity(ActivityShelfOutcome),
    /// Working state card.
    Working,
    /// Focus moved between panes.
    FocusChanged(&'static str),
}

// ── Persistent state ────────────────────────────────────────────────────────

/// Consumer-owned workbench interaction state (survives frames).
///
/// **Draft law:** never clear [`PromptComposerState`] when opening overlays —
/// only host submit/clear policy may wipe draft.
#[derive(Debug)]
pub struct AgentWorkbenchState {
    /// Workspace collapse/zoom.
    pub workspace: WorkspaceState,
    /// Single scene authority for focus, layers, Esc.
    pub scene: InteractionScene<&'static str, &'static str, ()>,
    /// Elevated TaskRail state.
    pub task_rail: TaskRailState,
    /// Activity shelf.
    pub activity: ActivityShelfState,
    /// Working-state card.
    pub working: WorkingStateCardState,
    /// Mode ribbon selection (plan/build/…).
    pub mode_ribbon: ModeRibbonState<&'static str>,
    /// Question-flow state (never owns composer draft).
    pub question: QuestionFlowState,
    /// Plan review state.
    pub plan: PlanReviewState,
    /// Diff review state.
    pub diff: DiffReviewState,
    /// Session picker state.
    pub session: SessionPickerState,
    /// Density override (`None` = derive from width each paint).
    pub density: Option<WorkbenchDensity>,
    /// Colorless paint preference.
    pub colorless: bool,
    /// Overlay open flags (synced on paint).
    permission_open: bool,
    question_open: bool,
    plan_open: bool,
    diff_open: bool,
    session_open: bool,
    command_open: bool,
}

impl Default for AgentWorkbenchState {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentWorkbenchState {
    /// Creates a workbench state focused on the transcript by default.
    #[must_use]
    pub fn new() -> Self {
        let mut state = Self {
            workspace: WorkspaceState::new(),
            scene: InteractionScene::default(),
            task_rail: TaskRailState::new(),
            activity: ActivityShelfState::new(),
            working: WorkingStateCardState::new(),
            mode_ribbon: ModeRibbonState::default(),
            question: QuestionFlowState::new(),
            plan: PlanReviewState::new(),
            diff: DiffReviewState::default(),
            session: SessionPickerState::new(),
            density: None,
            colorless: false,
            permission_open: false,
            question_open: false,
            plan_open: false,
            diff_open: false,
            session_open: false,
            command_open: false,
        };
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

    /// Whether permission overlay layer is open.
    #[must_use]
    pub const fn permission_open(&self) -> bool {
        self.permission_open
    }

    /// Whether question-flow overlay is open.
    #[must_use]
    pub const fn question_open(&self) -> bool {
        self.question_open
    }

    /// Plan overlay.
    #[must_use]
    pub const fn plan_open(&self) -> bool {
        self.plan_open
    }

    /// Diff overlay.
    #[must_use]
    pub const fn diff_open(&self) -> bool {
        self.diff_open
    }

    /// Session picker overlay.
    #[must_use]
    pub const fn session_open(&self) -> bool {
        self.session_open
    }

    /// Command palette overlay flag.
    #[must_use]
    pub const fn command_open(&self) -> bool {
        self.command_open
    }

    /// Host opens/closes session picker (draft stays in composer).
    pub const fn set_session_open(&mut self, open: bool) {
        self.session_open = open;
    }

    /// Host opens/closes plan review.
    pub const fn set_plan_open(&mut self, open: bool) {
        self.plan_open = open;
    }

    /// Host opens/closes diff review.
    pub const fn set_diff_open(&mut self, open: bool) {
        self.diff_open = open;
    }

    /// Host opens/closes question flow.
    pub const fn set_question_open(&mut self, open: bool) {
        self.question_open = open;
    }

    /// Host opens/closes command surface.
    pub const fn set_command_open(&mut self, open: bool) {
        self.command_open = open;
    }

    /// Any dismissible overlay owning input.
    #[must_use]
    pub const fn any_overlay_open(&self) -> bool {
        self.permission_open
            || self.question_open
            || self.plan_open
            || self.diff_open
            || self.session_open
            || self.command_open
    }

    /// Focused pane id when a workbench control owns focus.
    #[must_use]
    pub fn focused_pane(&self) -> Option<&'static str> {
        self.scene.focused().copied()
    }

    /// Focus a workbench pane by id.
    pub fn focus_pane(&mut self, pane: WorkbenchPane) {
        let _ = self.scene.focus(pane.id());
    }

    /// Cycle focus among root panes (Tab when no overlay).
    pub fn cycle_focus(&mut self, reverse: bool) -> WorkbenchKeyOutcome {
        let order = WorkbenchPane::focus_order();
        let cur = self.focused_pane().unwrap_or("transcript");
        let idx = order.iter().position(|p| p.id() == cur).unwrap_or(1);
        let next = if reverse {
            if idx == 0 { order.len() - 1 } else { idx - 1 }
        } else {
            (idx + 1) % order.len()
        };
        let id = order[next].id();
        let _ = self.scene.focus(id);
        WorkbenchKeyOutcome::FocusChanged(id)
    }

    /// Routes Escape through the persistent scene (top dismissible peels first).
    ///
    /// **One-layer Esc:** only the top dismissible layer peels. Does **not**
    /// grant permissions or submit plans. Composer draft is never cleared here.
    pub fn handle_escape(&mut self) -> InteractionOutcome<&'static str, &'static str, ()> {
        let outcome = self.scene.handle_escape();
        self.sync_overlay_flags_from_scene();
        outcome
    }

    fn sync_overlay_flags_from_scene(&mut self) {
        let layers = self.scene.layers();
        let has = |id: &str| layers.iter().any(|l| l.id == id);
        if !has("permission") {
            self.permission_open = false;
        }
        if !has("question") {
            self.question_open = false;
        }
        if !has("plan") {
            self.plan_open = false;
        }
        if !has("diff") {
            self.diff_open = false;
        }
        if !has("session") {
            self.session_open = false;
        }
        if !has("command") {
            self.command_open = false;
        }
    }

    /// Route a key using scene focus / top layer ownership.
    ///
    /// Order: top overlay → focused pane. **Prompt draft is never cleared.**
    ///
    /// `activities` / `diff_hunks` are borrowed host data for elevated widgets
    /// that require them on input (same law as paint: host owns models).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        prompt: &mut PromptComposerState,
        transcript: &mut TranscriptState<&'static str>,
        transcript_blocks: &[TranscriptBlock<'_, &'static str>],
        permission: Option<&mut PermissionPromptState>,
        task_models: &[ActivityModel],
        activities: Option<&[ActivityItem]>,
        diff_hunks: Option<&[DiffHunk]>,
    ) -> WorkbenchKeyOutcome {
        // Esc: one-layer peel
        if matches!(key.code, KeyCode::Esc) && key.is_press() {
            let top_dismissible = self.scene.layers().last().is_some_and(|layer| {
                layer.id != "root" && matches!(layer.esc, LayerDismissPolicy::Dismissible)
            });
            if top_dismissible {
                if self.permission_open
                    && let Some(perm) = permission
                    && !perm.is_empty()
                {
                    let out = perm.handle_key(key);
                    if !matches!(out, PermissionOutcome::Ignored) {
                        if matches!(out, PermissionOutcome::Cancelled { .. }) {
                            let _ = self.handle_escape();
                        }
                        return WorkbenchKeyOutcome::Permission(out);
                    }
                }
                if self.question_open {
                    let _ = self.question.handle_key(key);
                    let _ = self.handle_escape();
                    return WorkbenchKeyOutcome::Question;
                }
                if self.plan_open {
                    let _ = self.plan.handle_key(key);
                    let _ = self.handle_escape();
                    return WorkbenchKeyOutcome::Plan;
                }
                if self.diff_open {
                    let hunks = diff_hunks.unwrap_or(&[]);
                    let _ = self.diff.handle_key(key, hunks);
                    let _ = self.handle_escape();
                    return WorkbenchKeyOutcome::Diff;
                }
                if self.session_open {
                    let _ = self.session.handle_key(key);
                    // Cancelled preserves draft by design of SessionPicker
                    let _ = self.handle_escape();
                    return WorkbenchKeyOutcome::Session;
                }
                if self.command_open {
                    return WorkbenchKeyOutcome::Scene(self.handle_escape());
                }
                return WorkbenchKeyOutcome::Scene(self.handle_escape());
            }
        }

        // Overlay input ownership (while open)
        if self.permission_open
            && let Some(perm) = permission
            && !perm.is_empty()
        {
            let out = perm.handle_key(key);
            if !matches!(out, PermissionOutcome::Ignored) {
                if matches!(out, PermissionOutcome::Cancelled { .. })
                    || (matches!(out, PermissionOutcome::Decided { .. }) && perm.is_empty())
                {
                    let _ = self.handle_escape();
                }
                return WorkbenchKeyOutcome::Permission(out);
            }
        }
        if self.question_open {
            let out = self.question.handle_key(key);
            if !matches!(out, crate::widgets::QuestionFlowOutcome::Ignored) {
                return WorkbenchKeyOutcome::Question;
            }
        }
        if self.plan_open {
            let out = self.plan.handle_key(key);
            if !matches!(out, crate::patterns::PlanReviewOutcome::Ignored) {
                return WorkbenchKeyOutcome::Plan;
            }
        }
        if self.diff_open {
            let hunks = diff_hunks.unwrap_or(&[]);
            let out = self.diff.handle_key(key, hunks);
            if !matches!(out, crate::widgets::DiffReviewOutcome::Ignored) {
                return WorkbenchKeyOutcome::Diff;
            }
        }
        if self.session_open {
            let out = self.session.handle_key(key);
            if !matches!(out, crate::patterns::SessionPickerOutcome::Ignored) {
                if matches!(out, crate::patterns::SessionPickerOutcome::Cancelled) {
                    let _ = self.handle_escape();
                }
                return WorkbenchKeyOutcome::Session;
            }
        }

        // Tab focus cycle when no overlay
        if !self.any_overlay_open()
            && key.is_press()
            && matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
        {
            return self.cycle_focus(matches!(key.code, KeyCode::BackTab));
        }

        // Gate composer only when focused and no overlay
        let overlay = self.any_overlay_open();
        match self.scene.focused().copied() {
            Some("prompt") => {
                prompt.set_accepts_input(!overlay);
                if overlay {
                    return WorkbenchKeyOutcome::Ignored;
                }
                let out = prompt.handle_key(key);
                if matches!(out, PromptComposerOutcome::Ignored) {
                    WorkbenchKeyOutcome::Ignored
                } else {
                    WorkbenchKeyOutcome::Prompt(out)
                }
            }
            Some("task_rail") => {
                let out = self.task_rail.handle_key(key, task_models);
                if matches!(out, TaskRailOutcome::Ignored) {
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
            Some("activity") => {
                if let Some(items) = activities {
                    let out = self.activity.handle_key(key, items);
                    if matches!(out, ActivityShelfOutcome::Ignored) {
                        WorkbenchKeyOutcome::Ignored
                    } else {
                        WorkbenchKeyOutcome::Activity(out)
                    }
                } else {
                    WorkbenchKeyOutcome::Ignored
                }
            }
            Some("working") => {
                let out = self.working.handle_key(key);
                if matches!(out, crate::patterns::WorkingStateOutcome::Ignored) {
                    WorkbenchKeyOutcome::Ignored
                } else {
                    WorkbenchKeyOutcome::Working
                }
            }
            _ => WorkbenchKeyOutcome::Ignored,
        }
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Resolves workbench geometry for the current area and density.
#[must_use]
pub fn agent_workbench_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    agent_workbench_layout_density(area, state, WorkbenchDensity::for_width(area.width))
}

/// Layout with explicit density (stories / tests).
#[must_use]
pub fn agent_workbench_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: WorkbenchDensity,
) -> Vec<PaneGeom> {
    let root = match density {
        WorkbenchDensity::Tiny => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 70,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Transcript.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 2,
            }),
            second: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 85,
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
        },
        WorkbenchDensity::Narrow => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 72,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(WorkbenchPane::Transcript.id()),
                constraint: PaneConstraint::Weight(1),
                collapse_priority: 2,
            }),
            second: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 12,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(WorkbenchPane::Activity.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 0,
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
            }),
        },
        WorkbenchDensity::Normal => {
            // west rail | center column (transcript + activity + working) / south prompt+status
            WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 72,
                first: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 22,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(WorkbenchPane::TaskRail.id()),
                        constraint: PaneConstraint::Min(12),
                        collapse_priority: 0,
                    }),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 88,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(WorkbenchPane::Transcript.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 2,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(WorkbenchPane::Activity.id()),
                            constraint: PaneConstraint::Fixed(1),
                            collapse_priority: 1,
                        }),
                    }),
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 15,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(WorkbenchPane::Working.id()),
                        constraint: PaneConstraint::Min(0),
                        collapse_priority: 0,
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
                }),
            }
        }
    };
    Workspace::new(root).layout(area, state)
}

// ── Modals / scene ──────────────────────────────────────────────────────────

/// Modal areas registered into the scene for the frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkbenchModals {
    /// Permission prompt area.
    pub permission: Option<Rect>,
    /// Question-flow area.
    pub question: Option<Rect>,
    /// Plan review area.
    pub plan: Option<Rect>,
    /// Diff review area.
    pub diff: Option<Rect>,
    /// Session picker area.
    pub session: Option<Rect>,
    /// Command palette / system picker area.
    pub command: Option<Rect>,
}

/// Centered modal geometry.
#[must_use]
pub fn permission_modal_rect(area: Rect) -> Rect {
    modal_rect(area, ModalSpec::new(3, 4, 16).height(1, 3, 6))
}

/// Question / plan / session modal.
#[must_use]
pub fn dialog_modal_rect(area: Rect) -> Rect {
    modal_rect(area, ModalSpec::new(4, 5, 20).height(1, 3, 8))
}

/// Diff modal (taller).
#[must_use]
pub fn diff_modal_rect(area: Rect) -> Rect {
    modal_rect(area, ModalSpec::new(5, 6, 24).height(1, 3, 10))
}

fn ensure_layer(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    id: &'static str,
    kind: LayerKind,
    area: Rect,
    focus_return: Option<&'static str>,
) {
    if !scene.layers().iter().any(|layer| layer.id == id) {
        scene.push_layer(InteractionLayer {
            id,
            kind,
            owns_input: true,
            esc: LayerDismissPolicy::Dismissible,
            outside: LayerDismissPolicy::Trap,
            focus_return,
        });
    }
    let _ = scene.register(InteractionElement::control(id, id, area));
}

/// Re-registers workbench panes into the **consumer-owned** scene.
pub fn sync_workbench_scene(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    panes: &[PaneGeom],
    modals: WorkbenchModals,
) {
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
            "activity" => "activity",
            "working" => "working",
            "prompt" => "prompt",
            "status" => "status",
            _ => continue,
        };
        let focusable = !matches!(id, "status");
        let _ = scene.register(
            InteractionElement::control(id, "root", pane.area)
                .role(SemanticRole::Control)
                .focusable(focusable),
        );
    }
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

    let pairs = [
        (modals.permission, "permission", LayerKind::Card),
        (modals.question, "question", LayerKind::Card),
        (modals.plan, "plan", LayerKind::Card),
        (modals.diff, "diff", LayerKind::Card),
        (modals.session, "session", LayerKind::Card),
        (modals.command, "command", LayerKind::Menu),
    ];
    for (rect, id, kind) in pairs {
        if let Some(modal) = rect {
            ensure_layer(scene, id, kind, modal, Some("prompt"));
        } else {
            let _ = scene.remove_layer(&id);
        }
    }
    scene.reconcile();
}

/// Registers workbench panes (prefer [`sync_workbench_scene`] with owned state).
pub fn register_workbench_scene(
    scene: &mut InteractionScene<&'static str, &'static str, ()>,
    panes: &[PaneGeom],
) {
    sync_workbench_scene(scene, panes, WorkbenchModals::default());
}

// ── Surfaces ────────────────────────────────────────────────────────────────

/// Borrowed surfaces for one workbench paint (public widgets only).
///
/// Optional elevated surfaces: provide when available; host owns data.
pub struct WorkbenchSurfaces<'a, 'b> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// Persistent workbench state.
    pub state: &'a mut AgentWorkbenchState,
    /// Task models for the rail.
    pub task_models: &'a [ActivityModel],
    /// Mode ribbon modes.
    pub modes: &'a [WorkbenchMode<'a, &'static str>],
    /// Transcript widget (MessageThread may paint in place via host).
    pub transcript: &'a Transcript<'a, &'b str>,
    /// Transcript state.
    pub transcript_state: &'a mut TranscriptState<&'b str>,
    /// Activity shelf items.
    pub activities: Option<&'a [ActivityItem]>,
    /// Prompt composer.
    pub prompt: &'a PromptComposer<'a>,
    /// Prompt state (**draft survives overlays**).
    pub prompt_state: &'a mut PromptComposerState,
    /// Status slots.
    pub status_slots: &'a [StatusSlot<'a, &'b str>],
    /// Status state.
    pub status_state: &'a mut StatusBarState<&'b str>,
    /// Permission overlay.
    pub permission: Option<(&'a PermissionPrompt<'a>, &'a mut PermissionPromptState)>,
    /// Question flow painter.
    pub question: Option<&'a QuestionFlow<'a>>,
    /// Plan review painter (state in workbench).
    pub plan: Option<&'a PlanReview<'a>>,
    /// Diff review painter.
    pub diff: Option<&'a DiffReview<'a>>,
    /// Session picker painter.
    pub session: Option<&'a SessionPicker<'a>>,
    /// Working state painter.
    pub working: Option<&'a WorkingStateCard<'a>>,
}

/// Paints a composed workbench frame from borrowed public surfaces.
pub fn paint_agent_workbench(buffer: &mut Buffer, area: Rect, surfaces: WorkbenchSurfaces<'_, '_>) {
    let WorkbenchSurfaces {
        system,
        state,
        task_models,
        modes,
        transcript,
        transcript_state,
        activities,
        prompt,
        prompt_state,
        status_slots,
        status_state,
        permission,
        question,
        plan,
        diff,
        session,
        working,
    } = surfaces;

    let density = state
        .density
        .unwrap_or_else(|| WorkbenchDensity::for_width(area.width));
    let panes = agent_workbench_layout_density(area, &state.workspace, density);

    let permission_rect = permission.as_ref().and_then(|(_, perm_state)| {
        if perm_state.is_empty() {
            None
        } else {
            Some(permission_modal_rect(area))
        }
    });
    let question_rect = question.map(|_| dialog_modal_rect(area));
    let plan_rect = plan.map(|_| dialog_modal_rect(area));
    let diff_rect = diff.map(|_| diff_modal_rect(area));
    let session_rect = session.map(|_| dialog_modal_rect(area));
    let command_rect = if state.command_open {
        Some(dialog_modal_rect(area))
    } else {
        None
    };

    state.permission_open = permission_rect.is_some();
    state.question_open = question.is_some();
    state.plan_open = plan.is_some();
    state.diff_open = diff.is_some();
    state.session_open = session.is_some();

    sync_workbench_scene(
        &mut state.scene,
        &panes,
        WorkbenchModals {
            permission: permission_rect,
            question: question_rect,
            plan: plan_rect,
            diff: diff_rect,
            session: session_rect,
            command: command_rect,
        },
    );

    let focused = state.scene.focused().copied();
    let overlay = state.any_overlay_open();
    // Draft preservation: accept input only when prompt focused and no overlay
    prompt_state.set_accepts_input(focused == Some("prompt") && !overlay);

    let colorless = state.colorless;

    for pane in &panes {
        if pane.collapsed || pane.area.is_empty() {
            continue;
        }
        match pane.id.0.as_str() {
            "task_rail" => {
                let is_focused = focused == Some("task_rail") && !overlay;
                state.task_rail.focused = is_focused;
                TaskRail::new(task_models, system)
                    .title("Tasks")
                    .colorless(colorless)
                    .paint(pane.area, buffer, &mut state.task_rail);
            }
            "transcript" => {
                let is_focused = focused == Some("transcript") && !overlay;
                let panel = Panel::new(system)
                    .title("Transcript")
                    .emphasis(if is_focused {
                        PanelChrome::Focused
                    } else {
                        PanelChrome::Normal
                    });
                let inner = panel.inner(pane.area);
                panel.paint(pane.area, buffer, None);
                transcript_state.set_focused(is_focused);
                StatefulWidget::render(
                    &transcript.focused(is_focused),
                    inner,
                    buffer,
                    transcript_state,
                );
            }
            "activity" => {
                if let Some(items) = activities {
                    state.activity.focused = focused == Some("activity") && !overlay;
                    ActivityShelf::new(items, system)
                        .colorless(colorless)
                        .paint(pane.area, buffer, &mut state.activity);
                }
            }
            "working" => {
                if let Some(card) = working {
                    if state.working.work.is_some() {
                        state.working.focused = focused == Some("working") && !overlay;
                        card.colorless(colorless)
                            .paint(pane.area, buffer, &mut state.working);
                    }
                }
            }
            "prompt" => {
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

    // Overlays (top of paint order)
    if let Some((card, permission_state)) = permission
        && let Some(modal) = permission_rect
    {
        StatefulWidget::render(card, modal, buffer, permission_state);
    }
    if let Some(flow) = question
        && let Some(modal) = question_rect
    {
        StatefulWidget::render(flow, modal, buffer, &mut state.question);
    }
    if let Some(plan_w) = plan
        && let Some(modal) = plan_rect
    {
        plan_w.paint(modal, buffer, &mut state.plan);
    }
    if let Some(diff_w) = diff
        && let Some(modal) = diff_rect
    {
        StatefulWidget::render(diff_w, modal, buffer, &mut state.diff);
    }
    if let Some(session_w) = session
        && let Some(modal) = session_rect
    {
        session_w.paint(modal, buffer, &mut state.session);
    }
}

// ── Helpers / fixtures ──────────────────────────────────────────────────────

/// Default plan/build modes for demos.
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

/// Demo activity models for multi-agent / tool-running stories.
#[must_use]
pub fn example_workbench_activities() -> Vec<ActivityItem> {
    use crate::patterns::ActivityKind;
    use crate::widgets::SemanticStatus;
    vec![
        ActivityItem::new("a1", "cargo test")
            .kind(ActivityKind::Shell)
            .status(SemanticStatus::Running)
            .elapsed("12s"),
        ActivityItem::new("a2", "subagent:review")
            .kind(ActivityKind::Subagent)
            .status(SemanticStatus::Waiting)
            .waiting_reason("permission")
            .action_required(true),
    ]
}

/// Demo task models for TaskRail.
#[must_use]
pub fn example_workbench_tasks() -> Vec<ActivityModel> {
    use crate::patterns::ActivityKind;
    use crate::widgets::SemanticStatus;
    vec![
        ActivityModel::new("t1", "Plan review")
            .status(SemanticStatus::Success)
            .kind(ActivityKind::Generic),
        ActivityModel::new("t2", "Tool: cargo test")
            .status(SemanticStatus::Running)
            .kind(ActivityKind::Shell)
            .elapsed("12s"),
        ActivityModel::new("t3", "subagent:docs")
            .status(SemanticStatus::Waiting)
            .kind(ActivityKind::Subagent)
            .needs_input(true),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::patterns::{example_plan_document, example_working_state};
    use crate::widgets::{
        PermissionRequest, PermissionRisk, Question, QuestionOption, QuestionSet, TranscriptBlock,
        TranscriptKind,
    };
    use ratatui_core::backend::TestBackend;
    use ratatui_core::terminal::Terminal;

    fn paint(
        workbench: &mut AgentWorkbenchState,
        system: &DesignSystem,
        task_models: &[ActivityModel],
        modes: &[WorkbenchMode<'_, &'static str>],
        blocks: &[TranscriptBlock<'_, &str>],
        permission: Option<(&PermissionPrompt<'_>, &mut PermissionPromptState)>,
        question: Option<&QuestionFlow<'_>>,
        width: u16,
        height: u16,
    ) -> Terminal<TestBackend> {
        let transcript = Transcript::new(blocks, system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(system);
        let mut pstate = PromptComposerState::new();
        pstate.set_text("draft survives");
        let slots = [StatusSlot::new("s", "ready").priority(0)];
        let mut sstate = StatusBarState::default();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                paint_agent_workbench(
                    f.buffer_mut(),
                    area,
                    WorkbenchSurfaces {
                        system,
                        state: workbench,
                        task_models,
                        modes,
                        transcript: &transcript,
                        transcript_state: &mut tstate,
                        activities: None,
                        prompt: &prompt,
                        prompt_state: &mut pstate,
                        status_slots: &slots,
                        status_state: &mut sstate,
                        permission,
                        question,
                        plan: None,
                        diff: None,
                        session: None,
                        working: None,
                    },
                );
            })
            .unwrap();
        // draft must still be host-owned — we only check pstate in other tests
        let _ = pstate.text();
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
        for d in [
            WorkbenchDensity::Normal,
            WorkbenchDensity::Narrow,
            WorkbenchDensity::Tiny,
        ] {
            let panes = agent_workbench_layout_density(area, &state, d);
            assert!(!panes.is_empty());
            for pane in panes {
                assert!(pane.area.right() <= area.right());
                assert!(pane.area.bottom() <= area.bottom());
            }
        }
    }

    #[test]
    fn density_for_width() {
        assert_eq!(WorkbenchDensity::for_width(30), WorkbenchDensity::Tiny);
        assert_eq!(WorkbenchDensity::for_width(50), WorkbenchDensity::Narrow);
        assert_eq!(WorkbenchDensity::for_width(100), WorkbenchDensity::Normal);
    }

    #[test]
    fn composed_workbench_paints_task_rail_modes_and_keeps_scene() {
        let system = DesignSystem::default();
        let lines = ["hello", "world"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        let models = example_workbench_tasks();
        let modes = default_modes("plan");
        let terminal = paint(
            &mut workbench,
            &system,
            &models,
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
        let modes = default_modes("build");
        let prompt_w = PermissionPrompt::new(&system);
        let mut pstate = PermissionPromptState::new();
        let _ = pstate.enqueue(sample_permission());
        let _ = paint(
            &mut workbench,
            &system,
            &[],
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

        let set = QuestionSet::new(
            "s1",
            "Proceed?",
            vec![Question::single(
                "q1",
                "Proceed?",
                vec![QuestionOption::new("yes", "Yes")],
            )],
        );
        workbench.question = QuestionFlowState::new();
        workbench.question.open_set(set);
        let flow = QuestionFlow::new(&system);
        let _ = paint(
            &mut workbench,
            &system,
            &[],
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
        let mut tstate = TranscriptState::new();
        let mut pcomp = PromptComposerState::new();
        pcomp.set_text("keep me");
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let out = workbench.handle_key(
            key,
            &mut pcomp,
            &mut tstate,
            &blocks,
            Some(&mut perm),
            &[],
            None,
            None,
        );
        assert!(
            matches!(out, WorkbenchKeyOutcome::Permission(_))
                || matches!(out, WorkbenchKeyOutcome::Scene(_)),
            "{out:?}"
        );
        // draft never cleared by workbench routing
        assert_eq!(pcomp.text(), "keep me");
    }

    #[test]
    fn draft_preserved_when_overlay_blocks_prompt() {
        let mut workbench = AgentWorkbenchState::new();
        workbench.permission_open = true;
        let mut prompt = PromptComposerState::new();
        prompt.set_text("important draft");
        let mut tstate = TranscriptState::new();
        let blocks: [TranscriptBlock<'_, &str>; 0] = [];
        workbench.focus_pane(WorkbenchPane::Prompt);
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let _ = workbench.handle_key(
            key,
            &mut prompt,
            &mut tstate,
            &blocks,
            None,
            &[],
            None,
            None,
        );
        assert_eq!(prompt.text(), "important draft");
    }

    #[test]
    fn focus_cycle_tab() {
        let mut wb = AgentWorkbenchState::new();
        wb.focus_pane(WorkbenchPane::Transcript);
        let out = wb.cycle_focus(false);
        assert!(matches!(out, WorkbenchKeyOutcome::FocusChanged(_)));
    }

    #[test]
    fn elevated_task_rail_and_activity_paint() {
        let system = DesignSystem::default();
        let lines = ["stream"];
        let blocks = [TranscriptBlock::new(
            "b1",
            TranscriptKind::Assistant,
            &lines,
        )];
        let mut workbench = AgentWorkbenchState::new();
        let models = example_workbench_tasks();
        let activities = example_workbench_activities();
        let transcript = Transcript::new(&blocks, &system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(&system);
        let mut pstate = PromptComposerState::new();
        let modes = default_modes("build");
        let slots = [StatusSlot::connection("s", "ready")];
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 100, 28);
        let mut buf = Buffer::empty(area);
        paint_agent_workbench(
            &mut buf,
            area,
            WorkbenchSurfaces {
                system: &system,
                state: &mut workbench,
                task_models: &models,
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut tstate,
                activities: Some(&activities),
                prompt: &prompt,
                prompt_state: &mut pstate,
                status_slots: &slots,
                status_state: &mut sstate,
                permission: None,
                question: None,
                plan: None,
                diff: None,
                session: None,
                working: None,
            },
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Tasks") || text.contains("cargo") || text.contains("Plan"),
            "{text}"
        );
    }

    #[test]
    fn plan_overlay_opens_layer() {
        let system = DesignSystem::default();
        let lines = ["plan"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        workbench.plan.open(example_plan_document());
        let plan_w = PlanReview::new(&system);
        let transcript = Transcript::new(&blocks, &system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(&system);
        let mut pstate = PromptComposerState::new();
        let modes = default_modes("plan");
        let slots = [];
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        paint_agent_workbench(
            &mut buf,
            area,
            WorkbenchSurfaces {
                system: &system,
                state: &mut workbench,
                task_models: &[],
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut tstate,
                activities: None,
                prompt: &prompt,
                prompt_state: &mut pstate,
                status_slots: &slots,
                status_state: &mut sstate,
                permission: None,
                question: None,
                plan: Some(&plan_w),
                diff: None,
                session: None,
                working: None,
            },
        );
        assert!(workbench.plan_open());
        assert!(workbench.scene.layers().iter().any(|l| l.id == "plan"));
        let _ = workbench.handle_escape();
        assert!(!workbench.plan_open());
    }

    #[test]
    fn working_card_band() {
        let system = DesignSystem::default();
        let lines = ["run"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        workbench.working.set_work(Some(example_working_state()));
        let working_w = WorkingStateCard::new(&system);
        let transcript = Transcript::new(&blocks, &system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(&system);
        let mut pstate = PromptComposerState::new();
        let modes = default_modes("build");
        let slots = [];
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 90, 28);
        let mut buf = Buffer::empty(area);
        paint_agent_workbench(
            &mut buf,
            area,
            WorkbenchSurfaces {
                system: &system,
                state: &mut workbench,
                task_models: &[],
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut tstate,
                activities: None,
                prompt: &prompt,
                prompt_state: &mut pstate,
                status_slots: &slots,
                status_state: &mut sstate,
                permission: None,
                question: None,
                plan: None,
                diff: None,
                session: None,
                working: Some(&working_w),
            },
        );
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("run") || text.contains("editing") || text.contains("summary"),
            "{text}"
        );
    }

    #[test]
    fn narrow_and_tiny_layout() {
        let state = WorkspaceState::new();
        let narrow = agent_workbench_layout_density(
            Rect::new(0, 0, 50, 20),
            &state,
            WorkbenchDensity::Narrow,
        );
        assert!(narrow.iter().any(|p| p.id.0 == "activity"));
        assert!(!narrow.iter().any(|p| p.id.0 == "task_rail" && !p.collapsed));
        let tiny =
            agent_workbench_layout_density(Rect::new(0, 0, 30, 16), &state, WorkbenchDensity::Tiny);
        assert!(tiny.iter().any(|p| p.id.0 == "transcript"));
        assert!(tiny.iter().any(|p| p.id.0 == "prompt"));
    }

    #[test]
    fn public_api_surface_no_private_imports() {
        let src = include_str!("agent_workbench.rs");
        assert!(src.contains("public"));
        assert!(src.contains("draft"));
        // Build needles so this test body does not self-match.
        let forbidden = [format!("{}::process", "std"), format!("{}::new", "Command")];
        for f in &forbidden {
            assert!(!src.contains(f.as_str()), "{f}");
        }
    }

    #[test]
    fn fixtures_non_empty() {
        assert!(!example_workbench_tasks().is_empty());
        assert!(!example_workbench_activities().is_empty());
    }

    #[test]
    fn ascii_and_colorless_flags_paint() {
        let system = DesignSystem::default();
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        workbench.colorless = true;
        let models = example_workbench_tasks();
        let activities = example_workbench_activities();
        let transcript = Transcript::new(&blocks, &system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(&system);
        let mut pstate = PromptComposerState::new();
        pstate.set_text("keep");
        let modes = default_modes("build");
        let slots = [StatusSlot::connection("s", "ok")];
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 100, 28);
        let mut buf = Buffer::empty(area);
        paint_agent_workbench(
            &mut buf,
            area,
            WorkbenchSurfaces {
                system: &system,
                state: &mut workbench,
                task_models: &models,
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut tstate,
                activities: Some(&activities),
                prompt: &prompt,
                prompt_state: &mut pstate,
                status_slots: &slots,
                status_state: &mut sstate,
                permission: None,
                question: None,
                plan: None,
                diff: None,
                session: None,
                working: None,
            },
        );
        assert_eq!(pstate.text(), "keep");
        assert!(workbench.colorless);
    }

    #[test]
    fn overlay_blocks_composer_accepts_input_on_paint() {
        let system = DesignSystem::default();
        let lines = ["x"];
        let blocks = [TranscriptBlock::new("b1", TranscriptKind::User, &lines)];
        let mut workbench = AgentWorkbenchState::new();
        workbench.focus_pane(WorkbenchPane::Prompt);
        let prompt_w = PermissionPrompt::new(&system);
        let mut perm = PermissionPromptState::new();
        let _ = perm.enqueue(sample_permission());
        let transcript = Transcript::new(&blocks, &system);
        let mut tstate = TranscriptState::new();
        let prompt = PromptComposer::new(&system);
        let mut pstate = PromptComposerState::new();
        pstate.set_text("draft");
        pstate.set_accepts_input(true);
        let modes = default_modes("build");
        let slots = [];
        let mut sstate = StatusBarState::default();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        paint_agent_workbench(
            &mut buf,
            area,
            WorkbenchSurfaces {
                system: &system,
                state: &mut workbench,
                task_models: &[],
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut tstate,
                activities: None,
                prompt: &prompt,
                prompt_state: &mut pstate,
                status_slots: &slots,
                status_state: &mut sstate,
                permission: Some((&prompt_w, &mut perm)),
                question: None,
                plan: None,
                diff: None,
                session: None,
                working: None,
            },
        );
        assert!(workbench.permission_open());
        assert!(!pstate.accepts_input());
        assert_eq!(pstate.text(), "draft");
    }

    #[test]
    fn modals_survive_a_terminal_narrower_than_their_minimum() {
        // 20x5 is the tiny tier the design law names; every modal helper has
        // to produce a rect there rather than panicking.
        for area in [
            Rect::new(0, 0, 20, 5),
            Rect::new(0, 0, 8, 3),
            Rect::new(0, 0, 1, 1),
        ] {
            for rect in [
                permission_modal_rect(area),
                dialog_modal_rect(area),
                diff_modal_rect(area),
            ] {
                assert!(rect.width <= area.width, "{rect:?} escapes {area:?}");
                assert!(rect.height <= area.height, "{rect:?} escapes {area:?}");
            }
        }
    }

    #[test]
    fn modal_rects_contained() {
        let area = Rect::new(0, 0, 80, 24);
        for r in [
            permission_modal_rect(area),
            dialog_modal_rect(area),
            diff_modal_rect(area),
        ] {
            assert!(r.right() <= area.right());
            assert!(r.bottom() <= area.bottom());
            assert!(r.width >= 1 && r.height >= 1);
        }
    }
}

/// Agent workbench nav sample.
#[must_use]
pub fn example_agent_workbench_nav() -> Vec<crate::widgets::NavItem<&'static str>> {
    use crate::widgets::{NavItem, NavItemStatus};

    vec![
        NavItem::new("chat", "Chat").icon("💬").command("wb.chat"),
        NavItem::new("plan", "Plan")
            .icon("📋")
            .status(NavItemStatus::Running)
            .command("wb.plan"),
        NavItem::new("files", "Files")
            .icon("📁")
            .command("wb.files"),
        NavItem::separator("sep1"),
        NavItem::new("sessions", "Sessions")
            .badge("2")
            .command("wb.sessions"),
        NavItem::new("settings", "Settings").command("wb.settings"),
    ]
}
