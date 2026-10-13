// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Persistent composite-pattern demos using public pattern state machines.

use ratatui::{Frame, layout::Rect};
use termrock::{
    input::{KeyEvent, MouseEvent},
    patterns::{
        ActivityShelf, ActivityShelfOutcome, ActivityShelfPresentation, ActivityShelfState,
        AgentBusyState, AgentStatusHeader, AgentStatusHeaderOutcome, AgentStatusHeaderState,
        AgentStatusPresentation, ApprovalQueue, ApprovalQueueOutcome, ApprovalQueuePresentation,
        ApprovalQueueState, BackgroundTaskPanel, BackgroundTaskPanelOutcome,
        BackgroundTaskPanelState, CrashReportSnapshot, ErrorRecoveryMode, ErrorRecoveryOutcome,
        ErrorRecoveryState, ErrorRecoverySurfaces, IntegrationStatus, IntegrationStatusOutcome,
        IntegrationStatusPresentation, IntegrationStatusState, PlanReview, PlanReviewOutcome,
        PlanReviewPhase, PlanReviewState, ProcessKey, ProcessRow, ProcessStatus, ProcessTable,
        ProcessTableOutcome, ProcessTableState, PromptQueue, PromptQueueOutcome, PromptQueuePhase,
        PromptQueuePresentation, PromptQueueState, QueryEditor, QueryEditorOutcome,
        QueryEditorState, QueryLanguage, QueryParameter, QueryResultSummary, ResultCell,
        ResultColumn, ResultGrid, ResultGridOutcome, ResultGridState, ResultQueryStatus, ResultRow,
        SessionPicker, SessionPickerOutcome, SessionPickerPhase, SessionPickerState, SubagentCard,
        SubagentCardOutcome, SubagentCardState, SubagentPresentation, TaskRail, TaskRailOutcome,
        TaskRailState, TerminalRunCard, TerminalRunCardOutcome, TerminalRunCardState,
        TerminalRunPresentation, WorkingStateCard, WorkingStateCardState, WorkingStateOutcome,
        WorkingStatePresentation, example_activities, example_activity_models,
        example_agent_status, example_approval_queue, example_background_tasks,
        example_integrations, example_plan_document, example_prompt_queue,
        example_recovery_snapshot, example_sessions, example_subagent_runs,
        example_terminal_run_lines, example_terminal_runs, example_working_state,
        render_error_recovery, result_column_model,
    },
    style::{DesignSystem, RolePalette},
    widgets::DataColumnWidth,
};

use super::StoryInteraction;

pub(crate) fn composite_interactor(id: &str) -> Option<Box<dyn StoryInteraction>> {
    let kind = match id {
        "process-table/basic" => CompositeKind::Process(process_state()),
        "query-editor/basic" => CompositeKind::Query(Box::new(query_state())),
        "result-grid/basic" => CompositeKind::Results(result_state()),
        "approval-queue/basic" => CompositeKind::Approval(approval_state()),
        "working-state-card/basic" => CompositeKind::Working(working_state()),
        "integration-status/list" => CompositeKind::Integration(integration_state()),
        "agent-status-header/basic" => CompositeKind::AgentStatus(agent_status_state()),
        "prompt-queue/compact" => CompositeKind::PromptQueue(prompt_queue_state()),
        "terminal-run-card/running" => CompositeKind::Terminal(terminal_state()),
        "activity-shelf/statuses" => CompositeKind::Activity(activity_state()),
        "error-recovery/basic" => CompositeKind::Recovery(recovery_state()),
        "plan-review/basic" => CompositeKind::Plan(plan_state()),
        "session-picker/basic" => CompositeKind::Session(session_state()),
        "task-rail/basic" => CompositeKind::Tasks(task_state()),
        "subagent-card/running" => CompositeKind::Subagent(SubagentCardState::new()),
        "background-tasks/mixed-statuses" => CompositeKind::Background(background_state()),
        _ => return None,
    };
    Some(Box::new(CompositePatternDemo {
        kind,
        theme: RolePalette::default(),
        outcome: None,
        elapsed_ms: 0,
    }))
}

enum CompositeKind {
    Process(ProcessTableState),
    Query(Box<QueryEditorState>),
    Results(ResultGridState),
    Approval(ApprovalQueueState),
    Working(WorkingStateCardState),
    Integration(IntegrationStatusState),
    AgentStatus(AgentStatusHeaderState),
    PromptQueue(PromptQueueState),
    Terminal(TerminalRunCardState),
    Activity(ActivityShelfState),
    Recovery(ErrorRecoveryState),
    Plan(PlanReviewState),
    Session(SessionPickerState),
    Tasks(TaskRailState),
    Subagent(SubagentCardState),
    Background(BackgroundTaskPanelState),
}

struct CompositePatternDemo {
    kind: CompositeKind,
    theme: RolePalette,
    outcome: Option<String>,
    elapsed_ms: u64,
}

fn process_state() -> ProcessTableState {
    ProcessTableState::with_selected(Some(ProcessKey::new(1902, 500)))
}

fn process_rows() -> Vec<ProcessRow<'static>> {
    vec![
        ProcessRow::new(ProcessKey::new(1, 100), "systemd")
            .cpu(0.1)
            .mem(4_000_000)
            .user("root")
            .branch()
            .expanded(),
        ProcessRow::new(ProcessKey::new(482, 200), "cargo")
            .parent(ProcessKey::new(1, 100))
            .depth(1)
            .cpu(42.0)
            .mem(640_000_000)
            .user("alice")
            .branch()
            .expanded(),
        ProcessRow::new(ProcessKey::new(1902, 500), "rustc")
            .parent(ProcessKey::new(482, 200))
            .depth(2)
            .cpu(88.4)
            .mem(1_100_000_000)
            .user("alice")
            .status(ProcessStatus::Running),
    ]
}

fn query_state() -> QueryEditorState {
    let mut state = QueryEditorState::with_text(
        "select u.id, u.name\nfrom users u\nwhere u.active = true\nlimit 20;",
    );
    state.language = QueryLanguage::sql();
    state.set_results(
        QueryResultSummary::new("ok · 20 rows · 12ms")
            .rows(20)
            .columns(2),
    );
    state.set_parameters(vec![QueryParameter::new("limit", "20").type_hint("int")]);
    state
}

fn result_columns() -> Vec<ResultColumn> {
    vec![
        ResultColumn::new("id", "ID")
            .type_name("int8")
            .not_null()
            .width(DataColumnWidth::Fixed(6))
            .pin_start(),
        ResultColumn::new("name", "Name")
            .type_name("text")
            .width(DataColumnWidth::Min(12))
            .editable(),
        ResultColumn::new("token", "Token")
            .type_name("text")
            .secret(),
    ]
}

fn result_rows() -> Vec<ResultRow<'static>> {
    static FIRST: [ResultCell<'static>; 3] = [
        ResultCell::integer("1"),
        ResultCell::text("alpha"),
        ResultCell::secret_value("secret"),
    ];
    static SECOND: [ResultCell<'static>; 3] = [
        ResultCell::integer("2"),
        ResultCell::text("beta"),
        ResultCell::null(),
    ];
    vec![ResultRow::new(1, 1, &FIRST), ResultRow::new(2, 2, &SECOND)]
}

fn result_state() -> ResultGridState {
    let columns = result_columns();
    let mut state = ResultGridState::with_schema(columns);
    state.set_status(
        ResultQueryStatus::Ready {
            total: Some(2),
            duration_ms: Some(8),
        },
        2,
    );
    state
}

fn approval_state() -> ApprovalQueueState {
    let mut state = ApprovalQueueState::new();
    state.set_items(example_approval_queue());
    state.presentation = ApprovalQueuePresentation::Full;
    state.focused = true;
    state
}

fn working_state() -> WorkingStateCardState {
    let mut state = WorkingStateCardState::new();
    state.set_work(Some(example_working_state()));
    state.presentation = WorkingStatePresentation::Expanded;
    state.focused = true;
    state
}

fn integration_state() -> IntegrationStatusState {
    let mut state = IntegrationStatusState::new();
    state.set_entries(example_integrations());
    state.presentation = IntegrationStatusPresentation::CompactList;
    state.focused = true;
    state
}

fn agent_status_state() -> AgentStatusHeaderState {
    let mut state = AgentStatusHeaderState::new();
    state.set_snapshot(example_agent_status());
    state.presentation = AgentStatusPresentation::Header;
    state.auto_contract = false;
    state.focused = true;
    state
}

fn prompt_queue_state() -> PromptQueueState {
    let mut state = PromptQueueState::new();
    state.set_items(example_prompt_queue());
    state.set_agent(AgentBusyState::Busy);
    state.presentation = PromptQueuePresentation::Compact;
    state.focused = true;
    state
}

fn activity_state() -> ActivityShelfState {
    let mut state = ActivityShelfState::new();
    state.focused = true;
    state.force_presentation = Some(ActivityShelfPresentation::Chips);
    state
}

fn terminal_state() -> TerminalRunCardState {
    let mut state = TerminalRunCardState::new();
    state.presentation = TerminalRunPresentation::Expanded;
    state
}

fn recovery_state() -> ErrorRecoveryState {
    let mut state = ErrorRecoveryState::new();
    state.mode = ErrorRecoveryMode::Full;
    state
}

fn plan_state() -> PlanReviewState {
    let mut state = PlanReviewState::new();
    state.open(example_plan_document());
    state.focused = true;
    state
}

fn session_state() -> SessionPickerState {
    let mut state = SessionPickerState::new();
    state.set_sessions(example_sessions());
    state.focused = true;
    state
}

fn task_state() -> TaskRailState {
    let mut state = TaskRailState::new();
    state.focused = true;
    state.list.select(Some("p1".into()));
    state
}

fn background_state() -> BackgroundTaskPanelState {
    let mut state = BackgroundTaskPanelState::new();
    state.focused = true;
    state.list.select(Some("b1".into()));
    state
}

impl CompositePatternDemo {
    fn note<T: std::fmt::Debug>(&mut self, label: &str, value: T) -> bool {
        self.outcome = Some(format!("{label}: {value:?}"));
        true
    }
}

impl StoryInteraction for CompositePatternDemo {
    fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let system = DesignSystem::from_palette(self.theme.clone());
        let tick = self.elapsed_ms / 400;
        match &mut self.kind {
            CompositeKind::Process(state) => ProcessTable::new(&process_rows(), &system)
                .title("processes")
                .render(area, frame.buffer_mut(), state),
            CompositeKind::Query(state) => {
                QueryEditor::new(&system)
                    .title("SQL")
                    .render(area, frame.buffer_mut(), state)
            }
            CompositeKind::Results(state) => {
                ResultGrid::new(&system, &result_columns(), &result_rows())
                    .title("query results")
                    .render(area, frame.buffer_mut(), state);
            }
            CompositeKind::Approval(state) => {
                frame.render_stateful_widget(&ApprovalQueue::new(&system), area, state);
            }
            CompositeKind::Working(state) => {
                frame.render_stateful_widget(
                    &WorkingStateCard::new(&system).tick(tick),
                    area,
                    state,
                );
            }
            CompositeKind::Integration(state) => {
                frame.render_stateful_widget(&IntegrationStatus::new(&system), area, state);
            }
            CompositeKind::AgentStatus(state) => {
                frame.render_stateful_widget(&AgentStatusHeader::new(&system), area, state);
            }
            CompositeKind::PromptQueue(state) => {
                frame.render_stateful_widget(&PromptQueue::new(&system), area, state);
            }
            CompositeKind::Terminal(state) => {
                let runs = example_terminal_runs();
                let lines = example_terminal_run_lines();
                state.focused = true;
                TerminalRunCard::new(&runs[0], &lines, &system)
                    .tick(tick)
                    .paint(area, frame.buffer_mut(), state);
            }
            CompositeKind::Activity(state) => {
                ActivityShelf::new(&example_activities(), &system).paint(
                    area,
                    frame.buffer_mut(),
                    state,
                );
            }
            CompositeKind::Recovery(state) => {
                let snapshot = example_recovery_snapshot();
                render_error_recovery(
                    frame.buffer_mut(),
                    area,
                    ErrorRecoverySurfaces {
                        system: &system,
                        state,
                        snapshot: &snapshot,
                        doctor: None,
                    },
                );
            }
            CompositeKind::Plan(state) => {
                frame.render_stateful_widget(&PlanReview::new(&system), area, state);
            }
            CompositeKind::Session(state) => {
                frame.render_stateful_widget(&SessionPicker::new(&system), area, state);
            }
            CompositeKind::Tasks(state) => {
                TaskRail::new(&example_activity_models(), &system)
                    .title("Tasks")
                    .paint(area, frame.buffer_mut(), state);
            }
            CompositeKind::Subagent(state) => {
                let runs = example_subagent_runs();
                state.focused = true;
                state.presentation = SubagentPresentation::Card;
                SubagentCard::new(&runs[0], &system).tick(tick).paint(
                    area,
                    frame.buffer_mut(),
                    state,
                );
            }
            CompositeKind::Background(state) => {
                BackgroundTaskPanel::new(&example_background_tasks(), &system).paint(
                    area,
                    frame.buffer_mut(),
                    state,
                );
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match &mut self.kind {
            CompositeKind::Process(state) => {
                let value = state.handle_key(&process_rows(), key);
                if matches!(value, ProcessTableOutcome::Ignored) {
                    false
                } else {
                    self.note("Process table", value)
                }
            }
            CompositeKind::Query(state) => {
                let value = state.handle_key(key, &[]);
                if matches!(value, QueryEditorOutcome::Ignored) {
                    false
                } else {
                    self.note("Query editor", value)
                }
            }
            CompositeKind::Results(state) => {
                let columns = result_columns();
                let model = result_column_model(&columns, true);
                let value = state.handle_key(key, &model, &[1, 2]);
                if matches!(value, ResultGridOutcome::Ignored) {
                    false
                } else {
                    self.note("Result grid", value)
                }
            }
            CompositeKind::Approval(state) => {
                let value = state.handle_key(key);
                if matches!(value, ApprovalQueueOutcome::Ignored) {
                    false
                } else {
                    self.note("Approval queue", value)
                }
            }
            CompositeKind::Working(state) => {
                let value = state.handle_key(key);
                if matches!(value, WorkingStateOutcome::Ignored) {
                    false
                } else {
                    self.note("Working state", value)
                }
            }
            CompositeKind::Integration(state) => {
                let value = state.handle_key(key);
                if matches!(value, IntegrationStatusOutcome::Ignored) {
                    false
                } else {
                    self.note("Integration status", value)
                }
            }
            CompositeKind::AgentStatus(state) => {
                let value = state.handle_key(key);
                if matches!(value, AgentStatusHeaderOutcome::Ignored) {
                    false
                } else {
                    self.note("Agent status", value)
                }
            }
            CompositeKind::PromptQueue(state) => {
                let value = state.handle_key(key);
                if matches!(value, PromptQueueOutcome::Ignored) {
                    false
                } else {
                    self.note("Prompt queue", value)
                }
            }
            CompositeKind::Terminal(state) => {
                let runs = example_terminal_runs();
                let lines = example_terminal_run_lines();
                let value = state.handle_key(key, &runs[0], &lines);
                if matches!(value, TerminalRunCardOutcome::Ignored) {
                    false
                } else {
                    self.note("Terminal run", value)
                }
            }
            CompositeKind::Activity(state) => {
                let value = state.handle_key(key, &example_activities());
                if matches!(value, ActivityShelfOutcome::Ignored) {
                    false
                } else {
                    self.note("Activity shelf", value)
                }
            }
            CompositeKind::Recovery(state) => {
                let snapshot: CrashReportSnapshot = example_recovery_snapshot();
                let value = state.handle_key(key, &snapshot);
                if matches!(value, ErrorRecoveryOutcome::Ignored) {
                    false
                } else {
                    self.note("Error recovery", value)
                }
            }
            CompositeKind::Plan(state) => {
                let value = state.handle_key(key);
                if matches!(value, PlanReviewOutcome::Ignored) {
                    false
                } else {
                    self.note("Plan review", value)
                }
            }
            CompositeKind::Session(state) => {
                let value = state.handle_key(key);
                if matches!(value, SessionPickerOutcome::Ignored) {
                    false
                } else {
                    self.note("Session picker", value)
                }
            }
            CompositeKind::Tasks(state) => {
                let value = state.handle_key(key, &example_activity_models());
                if matches!(value, TaskRailOutcome::Ignored) {
                    false
                } else {
                    self.note("Task rail", value)
                }
            }
            CompositeKind::Subagent(state) => {
                let runs = example_subagent_runs();
                let value = state.handle_key(key, &runs[0]);
                if matches!(value, SubagentCardOutcome::Ignored) {
                    false
                } else {
                    self.note("Subagent", value)
                }
            }
            CompositeKind::Background(state) => {
                let value = state.handle_key(key, &example_background_tasks());
                if matches!(value, BackgroundTaskPanelOutcome::Ignored) {
                    false
                } else {
                    self.note("Background tasks", value)
                }
            }
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, _preview_area: Rect) -> bool {
        match &mut self.kind {
            CompositeKind::Process(state) => {
                let value = state.handle_mouse(&process_rows(), mouse);
                if matches!(value, ProcessTableOutcome::Ignored) {
                    false
                } else {
                    self.note("Process table", value)
                }
            }
            CompositeKind::Query(state) => {
                let value = state.handle_mouse(mouse, &[]);
                if matches!(value, QueryEditorOutcome::Ignored) {
                    false
                } else {
                    self.note("Query editor", value)
                }
            }
            CompositeKind::Results(state) => {
                let columns = result_columns();
                let mut model = result_column_model(&columns, true);
                let value = state.handle_mouse(mouse, &mut model, &[1, 2]);
                if matches!(value, ResultGridOutcome::Ignored) {
                    false
                } else {
                    self.note("Result grid", value)
                }
            }
            CompositeKind::Approval(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, ApprovalQueueOutcome::Ignored) {
                    false
                } else {
                    self.note("Approval queue", value)
                }
            }
            CompositeKind::Working(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, WorkingStateOutcome::Ignored) {
                    false
                } else {
                    self.note("Working state", value)
                }
            }
            CompositeKind::Integration(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, IntegrationStatusOutcome::Ignored) {
                    false
                } else {
                    self.note("Integration status", value)
                }
            }
            CompositeKind::AgentStatus(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, AgentStatusHeaderOutcome::Ignored) {
                    false
                } else {
                    self.note("Agent status", value)
                }
            }
            CompositeKind::PromptQueue(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, PromptQueueOutcome::Ignored) {
                    false
                } else {
                    self.note("Prompt queue", value)
                }
            }
            CompositeKind::Terminal(state) => {
                let runs = example_terminal_runs();
                let lines = example_terminal_run_lines();
                let value = state.handle_mouse(mouse, &runs[0], &lines);
                if matches!(value, TerminalRunCardOutcome::Ignored) {
                    false
                } else {
                    self.note("Terminal run", value)
                }
            }
            CompositeKind::Activity(state) => {
                let value = state.handle_mouse(mouse, &example_activities());
                if matches!(value, ActivityShelfOutcome::Ignored) {
                    false
                } else {
                    self.note("Activity shelf", value)
                }
            }
            CompositeKind::Recovery(_) => false,
            CompositeKind::Plan(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, PlanReviewOutcome::Ignored) {
                    false
                } else {
                    self.note("Plan review", value)
                }
            }
            CompositeKind::Session(state) => {
                let value = state.handle_mouse(mouse);
                if matches!(value, SessionPickerOutcome::Ignored) {
                    false
                } else {
                    self.note("Session picker", value)
                }
            }
            CompositeKind::Tasks(state) => {
                let value = state.handle_mouse(mouse, &example_activity_models());
                if matches!(value, TaskRailOutcome::Ignored) {
                    false
                } else {
                    self.note("Task rail", value)
                }
            }
            CompositeKind::Subagent(state) => {
                let runs = example_subagent_runs();
                let value = state.handle_mouse(mouse, &runs[0]);
                if matches!(value, SubagentCardOutcome::Ignored) {
                    false
                } else {
                    self.note("Subagent", value)
                }
            }
            CompositeKind::Background(state) => {
                let value = state.handle_mouse(mouse, &example_background_tasks());
                if matches!(value, BackgroundTaskPanelOutcome::Ignored) {
                    false
                } else {
                    self.note("Background tasks", value)
                }
            }
        }
    }

    fn set_theme(&mut self, theme: RolePalette) {
        self.theme = theme;
    }

    fn hints(&self) -> Vec<&'static str> {
        match &self.kind {
            CompositeKind::Process(_) => {
                vec!["↑↓ select", "←→ tree", "/ filter", "K signal", "Esc cancel"]
            }
            CompositeKind::Query(_) => {
                vec![
                    "type/paste query",
                    "Ctrl+Enter run",
                    "Tab panes",
                    "Esc cancel",
                ]
            }
            CompositeKind::Results(_) => {
                vec!["arrows select cell", "Enter edit", "C copy", "E export"]
            }
            CompositeKind::Approval(_) => {
                vec![
                    "↑↓ select",
                    "Enter inspect",
                    "A approve",
                    "D deny",
                    "Esc cancel",
                ]
            }
            CompositeKind::Working(_) => {
                vec!["Enter expand/collapse", "S stop request", "L logs"]
            }
            CompositeKind::Integration(_) => {
                vec!["↑↓ select", "Enter inspect", "R reconnect", "Tab details"]
            }
            CompositeKind::AgentStatus(_) => {
                vec!["←→ action", "Enter activate", "click action"]
            }
            CompositeKind::PromptQueue(_) => {
                vec!["↑↓ select", "Enter edit", "D remove", "Esc cancel"]
            }
            CompositeKind::Terminal(_) => vec![
                "Enter expand/collapse",
                "wheel/↑↓ scroll",
                "S stop request",
                "F full view",
            ],
            CompositeKind::Activity(_) => {
                vec!["←→ select", "Enter activate", "wheel navigate"]
            }
            CompositeKind::Recovery(_) => {
                vec!["↑↓ action", "Enter request", "Tab panes", "Esc cancel"]
            }
            CompositeKind::Plan(_) => {
                vec![
                    "Tab panes",
                    "↑↓ select",
                    "Enter inspect",
                    "A approve",
                    "R reject",
                ]
            }
            CompositeKind::Session(_) => vec![
                "↑↓ select",
                "/ search",
                "Enter resume",
                "D delete dialog",
                "Esc cancel",
            ],
            CompositeKind::Tasks(_) => {
                vec![
                    "↑↓ select",
                    "←→ collapse/expand",
                    "/ filter",
                    "Enter activate",
                ]
            }
            CompositeKind::Subagent(_) => {
                vec!["Enter expand/collapse", "S stop request", "F full view"]
            }
            CompositeKind::Background(_) => {
                vec![
                    "↑↓ select",
                    "Enter inspect",
                    "S stop request",
                    "C clear completed",
                ]
            }
        }
    }

    fn take_outcome(&mut self) -> Option<String> {
        self.outcome.take()
    }

    fn handle_tick(&mut self, elapsed_ms: u64) -> bool {
        let timed = matches!(
            self.kind,
            CompositeKind::Working(_) | CompositeKind::Terminal(_) | CompositeKind::Subagent(_)
        );
        let changed = timed && self.elapsed_ms / 400 != elapsed_ms / 400;
        self.elapsed_ms = elapsed_ms;
        changed
    }

    fn next_deadline_ms(&self, elapsed_ms: u64) -> Option<u64> {
        matches!(
            self.kind,
            CompositeKind::Working(_) | CompositeKind::Terminal(_) | CompositeKind::Subagent(_)
        )
        .then_some((elapsed_ms / 400 + 1) * 400)
    }

    fn captures_text_input(&self) -> bool {
        match &self.kind {
            CompositeKind::Query(_) => true,
            CompositeKind::Process(state) => state.filter.is_some(),
            CompositeKind::PromptQueue(state) => {
                matches!(state.phase, PromptQueuePhase::Edit { .. })
            }
            CompositeKind::Plan(state) => state.phase != PlanReviewPhase::Review,
            CompositeKind::Session(state) => matches!(
                state.phase,
                SessionPickerPhase::Browse
                    | SessionPickerPhase::Create
                    | SessionPickerPhase::Rename
            ),
            _ => false,
        }
    }

    fn handle_preview_escape(&mut self, key: KeyEvent) -> bool {
        let owns_escape = match &self.kind {
            CompositeKind::Process(state) => {
                state.filter.is_some() || state.pending_confirm.is_some()
            }
            CompositeKind::PromptQueue(state) => state.phase != PromptQueuePhase::Browse,
            CompositeKind::Plan(state) => state.phase != PlanReviewPhase::Review,
            CompositeKind::Session(state) => state.phase != SessionPickerPhase::Browse,
            _ => false,
        };
        owns_escape && self.handle_key(key)
    }
}
