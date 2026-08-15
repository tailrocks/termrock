// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! The showcase host: state, one paint, one update.
//!
//! Everything here goes through `termrock`'s public API. Where a capability is
//! missing the answer is to ship it in the library and consume it — never to
//! reach into a private module or fork a widget (SKD-3). The gap log at
//! `docs/design/showcase-api-gaps.md` is where a missing capability goes.

use ratatui_core::layout::Rect;
use termrock::input::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use termrock::patterns::{ActivityItem, ActivityModel};
use termrock::patterns::{
    AgentWorkbenchState, PlanReview, SessionPicker, WorkbenchDensity, WorkbenchKeyOutcome,
    WorkbenchSurfaces, WorkingStateCard, default_modes, example_plan_document,
    render_agent_workbench,
};
use termrock::runtime::FrameTick;
use termrock::style::{DesignSystem, RolePalette};
use termrock::widgets::{
    DiffLine, DiffReview, ListRow, PermissionAction, PermissionOutcome, PermissionPrompt,
    PermissionPromptState, PermissionRequest, PermissionRisk, PromptComposer,
    PromptComposerOutcome, PromptComposerState, StatusBarState, StatusSlot, Transcript,
    TranscriptBlock, TranscriptKind, TranscriptState,
};

use crate::demo_runtime::{DemoEvent, DemoRuntime, Scenario};
use crate::model::{Message, Role, Session, Subagent, ToolRun, demo_files};

/// Wrap column for the model's own line store.
const MODEL_WRAP: usize = 72;

/// The whole application.
pub struct App {
    /// Paint authority.
    pub system: DesignSystem,
    /// Workbench chrome state (focus, panes, overlays).
    pub workbench: AgentWorkbenchState,
    /// Conversation and runs.
    pub session: Session,
    /// The scripted agent.
    pub demo: DemoRuntime,
    /// Composer state — the draft survives every overlay.
    pub prompt: PromptComposerState,
    /// Transcript scroll/selection.
    pub transcript: TranscriptState<&'static str>,
    /// Status bar hit regions.
    pub status: StatusBarState<&'static str>,
    /// Trust queue.
    pub permission: PermissionPromptState,
    /// Whether a plan is awaiting review.
    pub plan_open: bool,
    /// Whether a diff is awaiting review.
    pub diff_open: bool,
    /// The scenario the next submit will run.
    pub scenario: Scenario,
    /// Last decision, shown in the status bar.
    pub last_decision: Option<String>,
    /// Whether the app should exit.
    pub quit: bool,
    /// File tree selection.
    pub files: termrock::widgets::TreeState<&'static str>,
    /// Streaming block counter, for stable ids.
    next_block: u32,
}

impl App {
    /// A booted app on the phosphor preset.
    #[must_use]
    pub fn new() -> Self {
        let mut session = Session::new("showcase");
        session.messages.push(Message::new(
            "m0",
            Role::System,
            "TermRock showcase — every surface here is a public widget.",
        ));
        Self {
            system: DesignSystem::from_palette(RolePalette::tailrocks_phosphor()),
            workbench: AgentWorkbenchState::new(),
            session,
            demo: DemoRuntime::new(),
            prompt: PromptComposerState::new(),
            transcript: TranscriptState::new(),
            status: StatusBarState::default(),
            permission: PermissionPromptState::new(),
            plan_open: false,
            diff_open: false,
            scenario: Scenario::HelloStream,
            last_decision: None,
            quit: false,
            files: termrock::widgets::TreeState::new(Some("list.rs")),
            next_block: 0,
        }
    }

    /// Whether a trust gate is open — the agent must not stream behind one.
    #[must_use]
    pub fn trust_gate_open(&self) -> bool {
        !self.permission.is_empty()
    }

    /// Submits the composer's draft and starts the selected scenario.
    pub fn submit(&mut self, text: String, now_ms: u64) {
        if text.trim().is_empty() {
            return;
        }
        let id = self.block_id("u");
        self.session
            .messages
            .push(Message::new(id, Role::User, text.clone()));
        self.session.finish_streaming();
        let id = self.block_id("a");
        self.session.messages.push(Message::streaming(id));
        self.demo.start(self.scenario, &text, now_ms);
        self.prompt.set_busy(true);
    }

    /// Applies one scripted event to the model.
    pub fn apply(&mut self, event: DemoEvent) {
        match event {
            DemoEvent::TextDelta { text } => {
                if self.session.streaming_mut().is_none() {
                    let id = self.block_id("a");
                    self.session.messages.push(Message::streaming(id));
                }
                if let Some(block) = self.session.streaming_mut() {
                    block.push_delta(&text, MODEL_WRAP);
                }
            }
            DemoEvent::ToolStart { id, name, detail } => {
                self.session.runs.push(ToolRun {
                    id,
                    name,
                    detail,
                    output: Vec::new(),
                    ok: None,
                });
            }
            DemoEvent::ToolStdout { id, line } => {
                if let Some(run) = self.session.run_mut(&id) {
                    run.output.push(line);
                }
            }
            DemoEvent::ToolEnd { id, ok } => {
                if let Some(run) = self.session.run_mut(&id) {
                    run.ok = Some(ok);
                }
            }
            DemoEvent::PermissionRequired {
                id,
                tool,
                scope,
                command,
                high_risk,
            } => {
                let request = PermissionRequest::new(id, tool, scope)
                    .risk(if high_risk {
                        PermissionRisk::High
                    } else {
                        PermissionRisk::Medium
                    })
                    .command(command)
                    .expected("nothing runs until you decide");
                let _ = self.permission.enqueue(request);
                // The agent waits behind its own gate.
                self.demo.set_paused(true);
            }
            DemoEvent::PlanReady => {
                self.workbench.plan.open(example_plan_document());
                self.plan_open = true;
            }
            DemoEvent::DiffReady => {
                self.diff_open = true;
            }
            DemoEvent::Question => {
                // The question flow is a review surface like the others; the
                // showcase opens it through the workbench's own state.
                self.plan_open = false;
            }
            DemoEvent::SubagentSpawn { id, title } => {
                self.session.subagents.push(Subagent {
                    id,
                    title,
                    done: false,
                });
            }
            DemoEvent::Done => {
                self.session.finish_streaming();
                self.prompt.set_busy(false);
                for subagent in &mut self.session.subagents {
                    subagent.done = true;
                }
            }
        }
    }

    /// Advances the scripted agent to `tick`.
    pub fn pump(&mut self, tick: FrameTick) {
        let now = tick.elapsed_ms();
        self.demo.set_paused(self.trust_gate_open());
        for event in self.demo.drain_due(now) {
            self.apply(event);
        }
    }

    /// When the host should wake next, in runner milliseconds.
    #[must_use]
    pub fn next_due_ms(&self) -> Option<u64> {
        self.demo.next_due_ms()
    }

    fn block_id(&mut self, prefix: &str) -> String {
        self.next_block = self.next_block.saturating_add(1);
        format!("{prefix}{}", self.next_block)
    }

    /// Routes one key.
    pub fn handle_key(&mut self, key: KeyEvent, tick: FrameTick) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        // Quit is the host's, not the library's: Ctrl+Q, never a bare Esc,
        // which belongs to the overlay peel.
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.cycle_scenario();
            return;
        }

        // Field borrows, not a `&self` method: the projection borrows the
        // session while the workbench borrows the chrome, and those are
        // disjoint.
        let lines = project_lines(&self.session);
        let blocks = project_blocks(&self.session, &lines);
        let outcome = self.workbench.handle_key(
            key,
            &mut self.prompt,
            &mut self.transcript,
            &blocks,
            Some(&mut self.permission),
            None,
            None,
            None,
            None,
        );
        drop(blocks);
        drop(lines);

        match outcome {
            WorkbenchKeyOutcome::Prompt(PromptComposerOutcome::Submit { text, .. }) => {
                self.prompt.clear_draft();
                self.submit(text, tick.elapsed_ms());
            }
            WorkbenchKeyOutcome::Permission(outcome) => self.apply_permission(outcome),
            _ => {}
        }
    }

    /// Applies a trust decision. Nothing here ever grants by default.
    fn apply_permission(&mut self, outcome: PermissionOutcome) {
        match outcome {
            PermissionOutcome::Decided { action, .. } => {
                let allowed = matches!(
                    action,
                    PermissionAction::Allow
                        | PermissionAction::AllowEdited
                        | PermissionAction::AllowRestricted
                );
                self.last_decision = Some(
                    if allowed {
                        "permission granted"
                    } else {
                        "permission denied"
                    }
                    .to_string(),
                );
                let id = self.block_id("p");
                self.session.messages.push(Message::new(
                    id,
                    Role::System,
                    if allowed {
                        "You allowed the command. The agent continues."
                    } else {
                        "You denied the command. Nothing ran."
                    },
                ));
                if !allowed {
                    self.demo.cancel();
                    self.prompt.set_busy(false);
                }
                self.session.finish_streaming();
            }
            PermissionOutcome::Cancelled { .. } => {
                // Dismissal is never a grant.
                self.last_decision = Some("permission dismissed — nothing ran".to_string());
                self.demo.cancel();
                self.prompt.set_busy(false);
            }
            _ => {}
        }
        self.demo.set_paused(self.trust_gate_open());
    }

    /// Moves to the next demo scenario.
    pub fn cycle_scenario(&mut self) {
        let index = Scenario::ALL
            .iter()
            .position(|s| *s == self.scenario)
            .unwrap_or(0);
        self.scenario = Scenario::ALL[(index + 1) % Scenario::ALL.len()];
    }

    /// Paints one frame.
    pub fn render(&mut self, buffer: &mut ratatui_core::buffer::Buffer, area: Rect) {
        // The files pane is a showcase-owned `layout::Workspace` split, which
        // §2.2.1 of the SoT sanctions until the pattern grows a files slot
        // (GAP-WB-2). Public layout, not a private fork.
        let (files_area, area) = split_files(area);
        if let Some(files) = files_area {
            self.render_files(buffer, files);
        }
        let lines = project_lines(&self.session);
        let blocks = project_blocks(&self.session, &lines);
        let transcript = Transcript::new(&blocks, &self.system);
        let prompt = PromptComposer::new(&self.system);
        let modes = default_modes(match self.scenario {
            Scenario::PlanBuild => "plan",
            _ => "build",
        });
        let scenario_label = self.scenario.label();
        let status_line = self
            .last_decision
            .clone()
            .unwrap_or_else(|| format!("{scenario_label} · ^n next scenario · ^q quit"));
        // Every status string comes from a bounded set — seven scenario ids,
        // seven labels, three decision sentences, three densities — so
        // interning them cannot grow.
        let density = match Self::density_for(area.width) {
            WorkbenchDensity::Normal => "wide",
            WorkbenchDensity::Narrow => "narrow",
            WorkbenchDensity::Tiny => "tiny",
            // The enum is `#[non_exhaustive]`: a new tier reads as its widest
            // neighbour rather than breaking the build of every consumer.
            _ => "wide",
        };
        let slots = [
            StatusSlot::mode("mode", if self.demo.is_busy() { "busy" } else { "ready" }),
            StatusSlot::new("scenario", self.scenario.id()),
            StatusSlot::new("density", density),
            StatusSlot::new("hint", intern(&status_line)),
        ];
        let tasks: Vec<ActivityModel> = Vec::new();
        let activities: Vec<ActivityItem> = Vec::new();
        let legacy: [ListRow<'_, &'static str>; 0] = [];

        let permission_widget = PermissionPrompt::new(&self.system);
        let plan_widget = PlanReview::new(&self.system);
        let diff_lines = demo_diff_lines();
        let diff_widget = DiffReview::new(&diff_lines, &self.system);
        let session_widget = SessionPicker::new(&self.system);
        let working_widget = WorkingStateCard::new(&self.system);

        let show_permission = !self.permission.is_empty();
        let show_plan = self.plan_open;
        let show_diff = self.diff_open;

        let mut workbench = std::mem::replace(&mut self.workbench, AgentWorkbenchState::new());
        render_agent_workbench(
            buffer,
            area,
            WorkbenchSurfaces {
                system: &self.system,
                state: &mut workbench,
                task_models: if tasks.is_empty() {
                    None
                } else {
                    Some(tasks.as_slice())
                },
                tasks: &legacy,
                modes: &modes,
                transcript: &transcript,
                transcript_state: &mut self.transcript,
                activities: if activities.is_empty() {
                    None
                } else {
                    Some(activities.as_slice())
                },
                prompt: &prompt,
                prompt_state: &mut self.prompt,
                status_slots: &slots,
                status_state: &mut self.status,
                permission: show_permission.then_some((&permission_widget, &mut self.permission)),
                question: None,
                plan: show_plan.then_some(&plan_widget),
                diff: show_diff.then_some(&diff_widget),
                session: None,
                working: (!self.session.subagents.is_empty()).then_some(&working_widget),
            },
        );
        let _ = session_widget;
        self.workbench = workbench;
    }

    /// Paints the fake workspace tree.
    fn render_files(&mut self, buffer: &mut ratatui_core::buffer::Buffer, area: Rect) {
        use ratatui_core::text::Line;
        use ratatui_core::widgets::StatefulWidget;
        use termrock::widgets::{Panel, Tree, TreeNode};

        let inner = Panel::new(&self.system)
            .title(intern(&self.session.title))
            .paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }
        let entries = demo_files();
        let nodes: Vec<TreeNode<'_, &'static str>> = entries
            .iter()
            .map(|entry| {
                let mut node = TreeNode::new(entry.path, Line::from(entry.path), entry.depth);
                if entry.directory {
                    node = node.branch().expanded();
                }
                node
            })
            .collect();
        Tree::new(&nodes, &self.system)
            .focused(false)
            .render(inner, buffer, &mut self.files);
    }

    /// Applies one host event.
    pub fn update(&mut self, event: Event, tick: FrameTick) {
        match event {
            Event::Key(key) => self.handle_key(key, tick),
            Event::Resize { .. } => {}
            _ => {}
        }
        self.pump(tick);
    }

    /// Density the current width resolves to (narrow proof).
    #[must_use]
    pub fn density_for(width: u16) -> WorkbenchDensity {
        WorkbenchDensity::for_width(width)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Splits the files rail off a wide frame.
///
/// Below 100 columns there is no rail: the workbench's own contraction is the
/// answer at that width, and a second column would take room the transcript
/// needs.
fn split_files(area: Rect) -> (Option<Rect>, Rect) {
    const RAIL: u16 = 26;
    if area.width < 100 {
        return (None, area);
    }
    let files = Rect::new(area.x, area.y, RAIL, area.height);
    let rest = Rect::new(
        area.x.saturating_add(RAIL),
        area.y,
        area.width.saturating_sub(RAIL),
        area.height,
    );
    (Some(files), rest)
}

/// The diff the review scenario shows.
fn demo_diff_lines() -> Vec<DiffLine<'static>> {
    vec![
        DiffLine::context("1", " fn paint(&self, area: Rect, buffer: &mut Buffer) {"),
        DiffLine::removed(
            "2",
            "-    buffer.set_stringn(area.x, area.y, text, w, style);",
        ),
        DiffLine::added(
            "3",
            "+    self.system.paint_row(buffer, area, text, style);",
        ),
        DiffLine::context("4", " }"),
    ]
}

/// The per-frame line projection: owned text borrowed as `&str`.
///
/// The model owns its strings so a recording of it is legible without a
/// terminal; the widget borrows them for one frame. Nothing is copied and
/// nothing leaks per frame.
fn project_lines(session: &Session) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = session
        .messages
        .iter()
        .map(|message| message.lines.iter().map(String::as_str).collect())
        .collect();
    for run in &session.runs {
        let mut lines = vec![run.name.as_str(), run.detail.as_str()];
        lines.extend(run.output.iter().map(String::as_str));
        out.push(lines);
    }
    for subagent in &session.subagents {
        out.push(vec![subagent.id.as_str(), subagent.title.as_str()]);
    }
    out
}

/// Blocks for this frame, borrowing [`project_lines`]'s slices.
fn project_blocks<'a>(
    session: &'a Session,
    lines: &'a [Vec<&'a str>],
) -> Vec<TranscriptBlock<'a, &'static str>> {
    let mut blocks = Vec::with_capacity(lines.len());
    for (index, message) in session.messages.iter().enumerate() {
        let kind = match message.role {
            Role::User => TranscriptKind::User,
            Role::Assistant => TranscriptKind::Assistant,
            Role::System => TranscriptKind::System,
        };
        blocks.push(TranscriptBlock::new(
            intern(&message.id),
            kind,
            lines[index].as_slice(),
        ));
    }
    let offset = session.messages.len();
    for (index, run) in session.runs.iter().enumerate() {
        blocks.push(TranscriptBlock::new(
            intern(&run.id),
            TranscriptKind::Tool,
            lines[offset + index].as_slice(),
        ));
    }
    let offset = offset + session.runs.len();
    for (index, subagent) in session.subagents.iter().enumerate() {
        blocks.push(TranscriptBlock::new(
            intern(&subagent.id),
            TranscriptKind::System,
            lines[offset + index].as_slice(),
        ));
    }
    blocks
}

/// A stable `&'static str` for an owned id.
///
/// Transcript ids are `&'static str`; the model's ids are owned and bounded by
/// the conversation, so each distinct one is interned exactly once. Frame text
/// never comes through here — only identities.
fn intern(id: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static IDS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let map = IDS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().expect("intern table");
    if let Some(existing) = guard.get(id) {
        return existing;
    }
    let leaked: &'static str = Box::leak(id.to_owned().into_boxed_str());
    guard.insert(id.to_owned(), leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::buffer::Buffer;
    use termrock::runtime::{FrameTick, Instant};

    fn tick(ms: u64) -> FrameTick {
        FrameTick::manual(
            Instant::now(),
            std::time::Duration::from_millis(ms),
            std::time::Duration::from_millis(16),
        )
    }

    #[test]
    fn a_submitted_prompt_streams_a_reply() {
        let mut app = App::new();
        app.submit("hello".into(), 0);
        assert!(app.demo.is_busy());
        for step in 0..400 {
            app.pump(tick(step * 25));
        }
        assert!(!app.demo.is_busy(), "the turn ends");
        let assistant = app
            .session
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .count();
        assert!(assistant >= 1, "the reply is in the thread");
    }

    #[test]
    fn a_high_risk_permission_stops_the_agent_and_never_self_grants() {
        let mut app = App::new();
        app.scenario = Scenario::PermissionHigh;
        app.submit("delete the incremental cache".into(), 0);
        for step in 0..40 {
            app.pump(tick(step * 25));
        }
        assert!(app.trust_gate_open(), "the gate is up");
        assert!(
            app.demo.next_due_ms().is_none(),
            "the agent does not stream behind its own gate"
        );

        // Dismissing is not granting.
        app.apply_permission(PermissionOutcome::Cancelled {
            request_id: Some("p1".into()),
            generation: 0,
        });
        assert_eq!(
            app.last_decision.as_deref(),
            Some("permission dismissed — nothing ran")
        );
    }

    #[test]
    fn the_frame_paints_at_every_size_the_law_names() {
        for (w, h) in [(120, 32), (80, 24), (40, 16), (20, 5)] {
            let mut app = App::new();
            app.submit("hello".into(), 0);
            app.pump(tick(200));
            let area = Rect::new(0, 0, w, h);
            let mut buffer = Buffer::empty(area);
            app.render(&mut buffer, area);
            let painted = buffer
                .content()
                .iter()
                .any(|cell| !cell.symbol().trim().is_empty());
            assert!(painted, "{w}x{h} painted nothing");
        }
    }

    #[test]
    fn the_draft_survives_a_permission() {
        let mut app = App::new();
        app.prompt.set_text("half-written thought");
        app.scenario = Scenario::PermissionHigh;
        app.submit("run it".into(), 0);
        for step in 0..40 {
            app.pump(tick(step * 25));
        }
        assert!(app.trust_gate_open());
        assert_eq!(app.prompt.text(), "half-written thought");
    }
}
