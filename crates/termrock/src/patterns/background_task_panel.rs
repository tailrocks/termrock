// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **BackgroundTaskPanel** — persistent monitoring for detached jobs.
//!
//! **Mission.** Detached commands, watchers, servers, and long jobs: live
//! output, status, restart count, ports/resources, elapsed time, follow/pause,
//! stop, restart, detach, open, and notifications. Reconnect and lost-process
//! states. Bounded output history with dropped-line indicators. Compact rail
//! row and full pane. **Process control is application-owned** — outcomes are
//! requests only.
//!
//! **vs [`super::ProcessTable`].** OS process monitor (signals, tree). This
//! panel is host-managed long jobs with output history.
//! **vs [`super::TerminalOutput`] / [`super::TerminalRunCard`].** Single-run
//! chrome; this is multi-job inventory + selected detail.
//! **vs [`super::TaskRail`].** Agent activity inventory; this is supervisor UI.
//!
//! Research: IDE task terminals, process supervisors, Grok Build watchers,
//! Zellij sessions.
//!
//! Teaches: how to compose persistent monitoring for detached jobs.
//!
//! Composes: [`crate::widgets::List`], [`crate::widgets::ListRow`],
//! [`crate::widgets::ListState`], [`crate::widgets::NotificationItem`],
//! [`crate::widgets::Panel`], [`crate::widgets::RowRole`],
//! [`crate::widgets::SemanticStatus`], [`crate::widgets::StatefulWidget`],
//! and 11 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use std::collections::VecDeque;

use ratatui_core::{
    buffer::Buffer, layout::Rect, style::Modifier, text::Line, widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    patterns::{ActivityKind, ActivityModel, ActivityScope},
    style::{DesignSystem, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        EmptyKind, EmptyState, List, ListRow, ListState, NotificationItem, Panel, RowRole,
        SemanticStatus, StatusIndicator, TerminalLine, TerminalOutput, TerminalOutputState,
        TerminalPaintMode, TerminalRunStatus, TerminalStream, ToastKind, ToastPriority,
        format_duration_ms,
    },
};

/// Overlay / drawer id.
pub const BACKGROUND_TASKS_OVERLAY_ID: &str = "termrock.background_tasks";
/// Default max retained output lines per task (host may raise).
pub const BACKGROUND_TASK_DEFAULT_HISTORY: usize = 500;
/// Compact rail min width.
pub const BACKGROUND_TASK_RAIL_WIDTH: u16 = 28;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Kind of long-running job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BackgroundTaskKind {
    /// Shell / command job.
    #[default]
    Command,
    /// File / CI watcher.
    Watcher,
    /// Server / listener.
    Server,
    /// Generic long job.
    Job,
}

impl BackgroundTaskKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Watcher => "watcher",
            Self::Server => "server",
            Self::Job => "job",
        }
    }

    /// Letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Command => '$',
            Self::Watcher => 'w',
            Self::Server => 'S',
            Self::Job => 'J',
        }
    }
}

/// Lifecycle including reconnect / lost process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BackgroundTaskStatus {
    /// Not started / queued.
    #[default]
    Pending,
    /// Live.
    Running,
    /// Host reconnecting to process.
    Reconnecting,
    /// Process lost / unknown (PID gone).
    Lost,
    /// Exited ok.
    Succeeded,
    /// Exited fail.
    Failed,
    /// Stopped by host/user.
    Stopped,
    /// Detached from UI (may still run).
    Detached,
}

impl BackgroundTaskStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Reconnecting => "reconnecting",
            Self::Lost => "lost",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Detached => "detached",
        }
    }

    /// Badge.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pend",
            Self::Running => "run",
            Self::Reconnecting => "reconn",
            Self::Lost => "lost",
            Self::Succeeded => "ok",
            Self::Failed => "fail",
            Self::Stopped => "stop",
            Self::Detached => "detach",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        {
            match self {
                Self::Pending => "·",
                Self::Running => "▶",
                Self::Reconnecting => "↻",
                Self::Lost => "⚠",
                Self::Succeeded => "✓",
                Self::Failed => "✗",
                Self::Stopped => "■",
                Self::Detached => "⧉",
            }
        }
    }

    /// Semantic mapping.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Pending => SemanticStatus::Queued,
            Self::Running | Self::Detached => SemanticStatus::Running,
            Self::Reconnecting => SemanticStatus::Waiting,
            Self::Lost | Self::Failed => SemanticStatus::Failed,
            Self::Succeeded => SemanticStatus::Success,
            Self::Stopped => SemanticStatus::Paused,
        }
    }

    /// Stop meaningful?
    #[must_use]
    pub const fn can_stop(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Running | Self::Reconnecting | Self::Detached
        )
    }

    /// Restart meaningful?
    #[must_use]
    pub const fn can_restart(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Stopped | Self::Lost | Self::Running
        )
    }

    /// Completed for clear-completed?
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Stopped)
    }

    /// Map toward TerminalRunStatus for detail pane.
    #[must_use]
    pub const fn to_terminal_status(self) -> TerminalRunStatus {
        match self {
            Self::Pending => TerminalRunStatus::Pending,
            Self::Running => TerminalRunStatus::Running,
            Self::Reconnecting => TerminalRunStatus::Running,
            Self::Lost => TerminalRunStatus::Failed,
            Self::Succeeded => TerminalRunStatus::Succeeded,
            Self::Failed => TerminalRunStatus::Failed,
            Self::Stopped => TerminalRunStatus::Cancelled,
            Self::Detached => TerminalRunStatus::Detached,
        }
    }
}

/// One projected output line in the bounded buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundOutputLine {
    /// Stable id within task.
    pub id: String,
    /// Stream.
    pub stream: TerminalStream,
    /// Plain text.
    pub text: String,
}

impl BackgroundOutputLine {
    /// Stdout.
    #[must_use]
    pub fn stdout(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            stream: TerminalStream::Stdout,
            text: text.into(),
        }
    }

    /// System.
    #[must_use]
    pub fn system(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            stream: TerminalStream::System,
            text: text.into(),
        }
    }
}

/// Bounded output history with drop accounting (host may own; helper available).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundOutputBuffer {
    lines: VecDeque<BackgroundOutputLine>,
    max_lines: usize,
    /// Total lines dropped from the head.
    pub dropped: u64,
    /// Total lines ever appended.
    pub total_appended: u64,
}

impl BackgroundOutputBuffer {
    /// Bounded.
    #[must_use]
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_lines: max_lines.max(1),
            dropped: 0,
            total_appended: 0,
        }
    }

    /// Default history cap.
    #[must_use]
    pub fn default_history() -> Self {
        Self::new(BACKGROUND_TASK_DEFAULT_HISTORY)
    }

    /// Current retained lines.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Slice.
    #[must_use]
    pub fn lines(&self) -> &VecDeque<BackgroundOutputLine> {
        &self.lines
    }

    /// Append one line; evict oldest when full.
    pub fn append(&mut self, line: BackgroundOutputLine) {
        self.total_appended = self.total_appended.saturating_add(1);
        if self.lines.len() >= self.max_lines {
            let _ = self.lines.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.lines.push_back(line);
    }
    /// Dropped-line indicator text.
    #[must_use]
    pub fn dropped_banner(&self) -> Option<String> {
        if self.dropped == 0 {
            return None;
        }
        Some({
            format!(
                "⚠ {} lines dropped (history cap {})",
                self.dropped, self.max_lines
            )
        })
    }

    /// Project to TerminalLine borrows for one paint frame (ids/text owned here).
    /// Host should prefer keeping TerminalLine lifetimes in their store.
    #[must_use]
    pub fn as_terminal_lines(&self) -> Vec<(String, TerminalStream, String)> {
        self.lines
            .iter()
            .map(|l| (l.id.clone(), l.stream, l.text.clone()))
            .collect()
    }
}

/// Host-projected background task (no process control in TermRock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundTask {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: BackgroundTaskKind,
    /// Status.
    pub status: BackgroundTaskStatus,
    /// Command / watch pattern.
    pub command: Option<String>,
    /// Cwd.
    pub cwd: Option<String>,
    /// Restart count.
    pub restart_count: u32,
    /// Listening ports / endpoints.
    pub ports: Vec<String>,
    /// Resource summary (`cpu 2% · rss 40M`).
    pub resources: Option<String>,
    /// Elapsed ms (host clock).
    pub duration_ms: Option<u64>,
    /// Pid display.
    pub pid: Option<u32>,
    /// Latest one-line status note.
    pub status_note: Option<String>,
    /// Bounded output (optional; host may paint separately).
    pub output: BackgroundOutputBuffer,
    /// Notify on complete/fail.
    pub notify_on_finish: bool,
    /// Revision.
    pub revision: u64,
}

impl BackgroundTask {
    /// Running command job.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            kind: BackgroundTaskKind::Command,
            status: BackgroundTaskStatus::Running,
            command: None,
            cwd: None,
            restart_count: 0,
            ports: Vec::new(),
            resources: None,
            duration_ms: None,
            pid: None,
            status_note: None,
            output: BackgroundOutputBuffer::default_history(),
            notify_on_finish: false,
            revision: 0,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: BackgroundTaskKind) -> Self {
        self.kind = k;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: BackgroundTaskStatus) -> Self {
        self.status = s;
        self
    }

    /// Command.
    #[must_use]
    pub fn command(mut self, c: impl Into<String>) -> Self {
        self.command = Some(c.into());
        self
    }

    /// Restarts.
    #[must_use]
    pub const fn restart_count(mut self, n: u32) -> Self {
        self.restart_count = n;
        self
    }

    /// Port.
    #[must_use]
    pub fn port(mut self, p: impl Into<String>) -> Self {
        self.ports.push(p.into());
        self
    }

    /// Resources.
    #[must_use]
    pub fn resources(mut self, r: impl Into<String>) -> Self {
        self.resources = Some(r.into());
        self
    }

    /// Duration.
    #[must_use]
    pub const fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Pid.
    #[must_use]
    pub const fn pid(mut self, p: u32) -> Self {
        self.pid = Some(p);
        self
    }

    /// Status note.
    #[must_use]
    pub fn status_note(mut self, n: impl Into<String>) -> Self {
        self.status_note = Some(n.into());
        self
    }

    /// Notify.
    #[must_use]
    pub const fn notify_on_finish(mut self, on: bool) -> Self {
        self.notify_on_finish = on;
        self
    }

    /// Output buffer.
    #[must_use]
    pub fn with_output(mut self, buf: BackgroundOutputBuffer) -> Self {
        self.output = buf;
        self
    }
    /// Meta line.
    #[must_use]
    pub fn meta_line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(c) = &self.command {
            parts.push(take_display_cols(c, 40).into_owned());
        }
        if let Some(r) = &self.resources {
            parts.push(r.clone());
        }
        if let Some(n) = &self.status_note {
            parts.push(n.clone());
        }
        if self.output.dropped > 0 {
            parts.push(format!("drop {}", self.output.dropped));
        }
        parts.join(" · ")
    }
}

/// Bridge → ActivityModel.
#[must_use]
pub fn background_task_to_activity(task: &BackgroundTask) -> ActivityModel {
    let scope = match task.kind {
        BackgroundTaskKind::Watcher => ActivityScope::Watcher,
        _ => ActivityScope::Background,
    };
    let mut m = ActivityModel::new(task.id.clone(), task.title.clone())
        .scope(scope)
        .kind(match task.kind {
            BackgroundTaskKind::Command => ActivityKind::Shell,
            BackgroundTaskKind::Watcher | BackgroundTaskKind::Job => ActivityKind::Tool,
            BackgroundTaskKind::Server => ActivityKind::Network,
        })
        .status(task.status.semantic());
    if let Some(ms) = task.duration_ms {
        m = m.elapsed(format_duration_ms(ms));
    }
    if let Some(c) = &task.command {
        m = m.detail(c.clone());
    }
    if matches!(
        task.status,
        BackgroundTaskStatus::Lost | BackgroundTaskStatus::Reconnecting
    ) {
        m = m.blocked(true).waiting_reason(task.status.label());
    }
    m
}

/// Finish notification when host policy says so.
#[must_use]
pub fn background_task_to_notification(
    task: &BackgroundTask,
    now_secs: u64,
) -> Option<NotificationItem> {
    if !task.notify_on_finish && !matches!(task.status, BackgroundTaskStatus::Lost) {
        if !task.status.is_terminal() {
            return None;
        }
        if !task.notify_on_finish {
            return None;
        }
    }
    if !task.status.is_terminal() && !matches!(task.status, BackgroundTaskStatus::Lost) {
        return None;
    }
    let kind = match task.status {
        BackgroundTaskStatus::Succeeded => ToastKind::Success,
        BackgroundTaskStatus::Failed | BackgroundTaskStatus::Lost => ToastKind::Error,
        BackgroundTaskStatus::Stopped => ToastKind::Warning,
        _ => ToastKind::Info,
    };
    let mut n = NotificationItem::new(
        task.id.clone(),
        format!("{} · {}", task.title, task.status.label()),
        kind,
    )
    .title(task.title.clone());
    n.priority = if matches!(task.status, BackgroundTaskStatus::Lost) {
        ToastPriority::High
    } else {
        ToastPriority::Normal
    };
    n.source = Some(task.kind.id().into());
    n.created_at_secs = now_secs;
    n.group_id = Some("background-task".into());
    Some(n)
}

// ── Presentation / state / outcomes ─────────────────────────────────────────

/// Panel density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum BackgroundTaskPresentation {
    /// Compact list row (rail).
    CompactRail,
    /// Full panel: list + detail output.
    #[default]
    Pane,
    /// Fullscreen (host overlay).
    Fullscreen,
}

impl BackgroundTaskPresentation {
    /// Auto.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < BACKGROUND_TASK_RAIL_WIDTH {
            Self::CompactRail
        } else {
            Self::Pane
        }
    }
}

/// Outcomes — **requests only**.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackgroundTaskPanelOutcome {
    /// Ignored.
    Ignored,
    /// Panel closed (Esc).
    Closed,
    /// Selection changed.
    Selected {
        /// Id.
        id: String,
    },
    /// Open task detail / focus.
    Opened {
        /// Id.
        id: String,
    },
    /// Stop process request.
    StopRequested {
        /// Id.
        id: String,
    },
    /// Restart request.
    RestartRequested {
        /// Id.
        id: String,
    },
    /// Detach UI from process.
    DetachRequested {
        /// Id.
        id: String,
    },
    /// Clear completed tasks (ids host should drop).
    ClearCompleted {
        /// Ids.
        ids: Vec<String>,
    },
    /// Follow tail toggled for detail.
    FollowChanged {
        /// Following.
        following: bool,
    },
    /// Notify preference toggled for selection.
    NotifyToggled {
        /// Id.
        id: String,
        /// On.
        on: bool,
    },
    /// Presentation changed.
    PresentationChanged {
        /// Mode.
        presentation: BackgroundTaskPresentation,
    },
    /// Output scroll detached.
    ScrollDetached,
}

/// Interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundTaskPanelState {
    /// List cursor.
    pub list: ListState<String>,
    /// Presentation.
    pub presentation: BackgroundTaskPresentation,
    /// Force presentation.
    pub force_presentation: Option<BackgroundTaskPresentation>,
    /// Detail output state (follow).
    pub output: TerminalOutputState,
    /// Hide terminal successes/stops.
    pub hide_completed: bool,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Panel open (overlay hosts).
    pub open: bool,
}

impl Default for BackgroundTaskPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundTaskPanelState {
    /// Defaults: pane, follow, open.
    #[must_use]
    pub fn new() -> Self {
        let mut output = TerminalOutputState::new();
        output.follow_tail_default();
        Self {
            list: ListState::default(),
            presentation: BackgroundTaskPresentation::Pane,
            force_presentation: None,
            output,
            hide_completed: false,
            focused: true,
            accepts_input: true,
            open: true,
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.output.set_accepts_input(on);
    }

    /// Close panel.
    pub fn close(&mut self) -> BackgroundTaskPanelOutcome {
        self.open = false;
        BackgroundTaskPanelOutcome::Closed
    }
    /// Selected id.
    #[must_use]
    pub fn selected_id(&self) -> Option<&str> {
        self.list.selected().map(String::as_str)
    }

    /// Visible tasks after filter.
    #[must_use]
    pub fn visible_tasks<'a>(&self, tasks: &'a [BackgroundTask]) -> Vec<&'a BackgroundTask> {
        tasks
            .iter()
            .filter(|t| !(self.hide_completed && t.status.is_terminal()))
            .collect()
    }

    fn list_rows(&self, tasks: &[BackgroundTask]) -> Vec<ListRow<'static, String>> {
        let vis = self.visible_tasks(tasks);
        vis.into_iter()
            .map(|t| {
                let label = format!("{} {}", t.kind.letter(), t.title);
                let mut row = ListRow::item(t.id.clone(), Line::from(label));
                row.status = Some(Line::from(format!(
                    "| {} {}",
                    t.status.semantic().glyph(),
                    t.status.id()
                )));
                let meta = t.meta_line();
                if !meta.is_empty() {
                    row.secondary = Some(Line::from(meta));
                }
                if t.restart_count > 0 {
                    row.badge = Some(Line::from(format!("r{}", t.restart_count)));
                }
                if matches!(
                    t.status,
                    BackgroundTaskStatus::Lost | BackgroundTaskStatus::Reconnecting
                ) {
                    row.badge = Some(Line::from(t.status.label()));
                } else if let Some(ms) = t.duration_ms {
                    row.badge = Some(Line::from(format_duration_ms(ms)));
                }
                row.role = RowRole::Item;
                row
            })
            .collect()
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        tasks: &[BackgroundTask],
    ) -> BackgroundTaskPanelOutcome {
        if !self.accepts_input || !key.is_press() {
            return BackgroundTaskPanelOutcome::Ignored;
        }
        if !self.open {
            return BackgroundTaskPanelOutcome::Ignored;
        }

        match key.code {
            KeyCode::Esc => return self.close(),
            KeyCode::Char('H') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.hide_completed = !self.hide_completed;
                return BackgroundTaskPanelOutcome::Ignored;
            }
            KeyCode::Char('c')
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || (key.modifiers.is_empty()
                        && key.code == KeyCode::Char('C')
                        && key.modifiers.contains(KeyModifiers::SHIFT)) =>
            {
                let ids: Vec<String> = tasks
                    .iter()
                    .filter(|t| t.status.is_terminal())
                    .map(|t| t.id.clone())
                    .collect();
                return BackgroundTaskPanelOutcome::ClearCompleted { ids };
            }
            KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let ids: Vec<String> = tasks
                    .iter()
                    .filter(|t| t.status.is_terminal())
                    .map(|t| t.id.clone())
                    .collect();
                return BackgroundTaskPanelOutcome::ClearCompleted { ids };
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(id) = self.selected_id() {
                    return BackgroundTaskPanelOutcome::StopRequested { id: id.to_string() };
                }
            }
            KeyCode::Char('r') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_id() {
                    return BackgroundTaskPanelOutcome::RestartRequested { id: id.to_string() };
                }
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_id() {
                    return BackgroundTaskPanelOutcome::DetachRequested { id: id.to_string() };
                }
            }
            KeyCode::Char('f') if key.modifiers.is_empty() => {
                let on = !self.output.is_following();
                self.output.set_following(on);
                return BackgroundTaskPanelOutcome::FollowChanged { following: on };
            }
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_id() {
                    // host flips notify; we only request toggle
                    return BackgroundTaskPanelOutcome::NotifyToggled {
                        id: id.to_string(),
                        on: true,
                    };
                }
            }
            KeyCode::Char('p') if key.modifiers.is_empty() => {
                self.presentation = match self.presentation {
                    BackgroundTaskPresentation::CompactRail => BackgroundTaskPresentation::Pane,
                    BackgroundTaskPresentation::Pane => BackgroundTaskPresentation::Fullscreen,
                    BackgroundTaskPresentation::Fullscreen => {
                        BackgroundTaskPresentation::CompactRail
                    }
                };
                self.force_presentation = Some(self.presentation);
                return BackgroundTaskPanelOutcome::PresentationChanged {
                    presentation: self.presentation,
                };
            }
            _ => {}
        }

        let rows = self.list_rows(tasks);
        use crate::interaction::Outcome;
        match self.list.handle_key(&rows, key) {
            Outcome::Activated(id) => BackgroundTaskPanelOutcome::Opened { id },
            Outcome::Changed => {
                if let Some(id) = self.list.selected() {
                    BackgroundTaskPanelOutcome::Selected { id: id.clone() }
                } else {
                    BackgroundTaskPanelOutcome::Ignored
                }
            }
            Outcome::Cancelled => self.close(),
            _ => {
                // Page keys on detail: try output when list ignores
                if let Some(task) = self
                    .selected_id()
                    .and_then(|id| tasks.iter().find(|t| t.id == id))
                {
                    let term_owned = task.output.as_terminal_lines();
                    let term_lines: Vec<TerminalLine<'_>> = term_owned
                        .iter()
                        .map(|(id, stream, text)| TerminalLine {
                            id: id.as_str(),
                            stream: *stream,
                            text: text.as_str(),
                            ansi: None,
                        })
                        .collect();
                    // Only scroll intents — avoid stealing letter keys
                    if matches!(
                        key.code,
                        KeyCode::PageUp
                            | KeyCode::PageDown
                            | KeyCode::Home
                            | KeyCode::End
                            | KeyCode::Up
                            | KeyCode::Down
                    ) && matches!(
                        self.presentation,
                        BackgroundTaskPresentation::Pane | BackgroundTaskPresentation::Fullscreen
                    ) {
                        // Prefer list navigation already handled; if list empty use output
                        if rows.is_empty() {
                            let meta = crate::widgets::TerminalCommandMeta::new(
                                task.command.as_deref().unwrap_or(task.title.as_str()),
                            )
                            .status(task.status.to_terminal_status());
                            let out = self.output.handle_key(key, &term_lines, &meta);
                            use crate::widgets::TerminalOutputOutcome;
                            return match out {
                                TerminalOutputOutcome::Detach => {
                                    BackgroundTaskPanelOutcome::ScrollDetached
                                }
                                TerminalOutputOutcome::Follow => {
                                    BackgroundTaskPanelOutcome::FollowChanged { following: true }
                                }
                                _ => BackgroundTaskPanelOutcome::Ignored,
                            };
                        }
                    }
                }
                BackgroundTaskPanelOutcome::Ignored
            }
        }
    }
}

/// Extend TerminalOutputState with follow default helper if needed.
trait FollowDefault {
    fn follow_tail_default(&mut self);
}

impl FollowDefault for TerminalOutputState {
    fn follow_tail_default(&mut self) {
        self.set_following(true);
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Background task panel / rail.
#[derive(Debug, Clone, Copy)]
pub struct BackgroundTaskPanel<'a> {
    tasks: &'a [BackgroundTask],
    system: &'a DesignSystem,
    title: &'a str,
    colorless: bool,
}

impl<'a> BackgroundTaskPanel<'a> {
    /// Tasks + system.
    #[must_use]
    pub const fn new(tasks: &'a [BackgroundTask], system: &'a DesignSystem) -> Self {
        Self {
            tasks,
            system,
            title: "Background",
            colorless: false,
        }
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut BackgroundTaskPanelState) {
        if area.is_empty() || !state.open {
            return;
        }
        if state.force_presentation.is_none() {
            state.presentation = BackgroundTaskPresentation::for_width(area.width);
        } else if let Some(p) = state.force_presentation {
            state.presentation = p;
        }
        // Glyph vocabulary and color capability are independent host choices.
        // A monochrome terminal may still render the Unicode status set.

        if matches!(state.presentation, BackgroundTaskPresentation::CompactRail) || area.height <= 3
        {
            self.paint_rail(area, buffer, state);
            return;
        }

        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let running = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Running))
            .count();
        let lost = self
            .tasks
            .iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Lost))
            .count();
        let mut title = self.title.to_string();
        if running > 0 {
            title = format!("{} · {running} run", self.title);
        }
        if lost > 0 {
            title = format!("{title} · {lost} lost");
        }
        let panel = Panel::new(self.system)
            .title(title.as_str())
            .emphasis(emphasis);
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }

        // split list | detail
        let list_w = (inner.width / 3).clamp(12, 28);
        let list_area = Rect {
            x: inner.x,
            y: inner.y,
            width: list_w.min(inner.width),
            height: inner.height.saturating_sub(1),
        };
        let detail_area = Rect {
            x: inner.x.saturating_add(list_area.width),
            y: inner.y,
            width: inner.width.saturating_sub(list_area.width),
            height: inner.height.saturating_sub(1),
        };
        let foot_y = inner.bottom().saturating_sub(1);

        let rows = state.list_rows(self.tasks);
        StatefulWidget::render(
            &List::new(&rows, self.system).focused(state.focused && state.accepts_input),
            list_area,
            buffer,
            &mut state.list,
        );

        // detail
        if !detail_area.is_empty() {
            if let Some(task) = state
                .selected_id()
                .and_then(|id| self.tasks.iter().find(|t| t.id == id))
            {
                self.paint_detail(detail_area, buffer, state, task);
            } else {
                EmptyState::new("Pick a task", self.system)
                    .kind(EmptyKind::NoData)
                    .paint(
                        detail_area,
                        buffer,
                        &mut crate::widgets::EmptyStateState::new(),
                    );
            }
        }

        let foot = "enter open · x stop · r restart · f follow · esc close";
        self.system.paint_row(
            buffer,
            Rect::new(inner.x, foot_y, inner.width, 1),
            foot,
            self.system.style(Role::TextMuted),
        );
    }

    fn paint_rail(&self, area: Rect, buffer: &mut Buffer, state: &mut BackgroundTaskPanelState) {
        let rows = state.list_rows(self.tasks);
        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system).title(self.title).emphasis(emphasis);
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
        if !inner.is_empty() {
            StatefulWidget::render(
                &List::new(&rows, self.system).focused(state.focused),
                inner,
                buffer,
                &mut state.list,
            );
        }
    }

    fn paint_detail(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut BackgroundTaskPanelState,
        task: &BackgroundTask,
    ) {
        let mut y = area.y;
        let max_y = area.bottom();
        // header meta
        let status = StatusIndicator::new(task.status.semantic(), self.system)
            .label(task.status.id())
            .colorless(self.colorless);
        let status_width = status.measure_width(None).min(area.width);
        status.paint(Rect::new(area.x, y, status_width, 1), buffer, None);
        let command_x = area
            .x
            .saturating_add(status_width)
            .saturating_add(u16::from(status_width < area.width));
        let command_width = area.right().saturating_sub(command_x);
        if command_width > 0 {
            self.system.paint_row(
                buffer,
                Rect::new(command_x, y, command_width, 1),
                take_display_cols(
                    task.command.as_deref().unwrap_or(&task.title),
                    usize::from(command_width),
                )
                .as_ref(),
                self.system.style(Role::Text),
            );
        }
        y = y.saturating_add(1);

        let mut meta = String::new();
        if task.restart_count > 0 {
            meta.push_str(&format!("restarts {} · ", task.restart_count));
        }
        if !task.ports.is_empty() {
            meta.push_str("ports ");
            meta.push_str(&task.ports.join(","));
            meta.push_str(" · ");
        }
        if let Some(r) = &task.resources {
            meta.push_str(r);
            meta.push_str(" · ");
        }
        if let Some(ms) = task.duration_ms {
            meta.push_str(&format_duration_ms(ms));
        }
        if !meta.is_empty() && y < max_y {
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &meta,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        if let Some(banner) = task.output.dropped_banner() {
            if y < max_y {
                StatusIndicator::new(SemanticStatus::Warning, self.system)
                    .label(&banner)
                    .colorless(self.colorless)
                    .paint(Rect::new(area.x, y, area.width, 1), buffer, None);
                y = y.saturating_add(1);
            }
        }

        if matches!(
            task.status,
            BackgroundTaskStatus::Lost | BackgroundTaskStatus::Reconnecting
        ) && y < max_y
        {
            let note = task.status_note.as_deref().unwrap_or(
                if matches!(task.status, BackgroundTaskStatus::Lost) {
                    "process lost — restart or clear"
                } else {
                    "reconnecting…"
                },
            );
            StatusIndicator::new(task.status.semantic(), self.system)
                .label(note)
                .colorless(self.colorless)
                .paint(Rect::new(area.x, y, area.width, 1), buffer, None);
            y = y.saturating_add(1);
        }

        if y >= max_y {
            return;
        }
        let out_area = Rect {
            x: area.x,
            y,
            width: area.width,
            height: max_y.saturating_sub(y),
        };
        let term_owned = task.output.as_terminal_lines();
        let term_lines: Vec<TerminalLine<'_>> = term_owned
            .iter()
            .map(|(id, stream, text)| TerminalLine {
                id: id.as_str(),
                stream: *stream,
                text: text.as_str(),
                ansi: None,
            })
            .collect();
        let meta = crate::widgets::TerminalCommandMeta::new(
            task.command.as_deref().unwrap_or(task.title.as_str()),
        )
        .status(task.status.to_terminal_status());
        state.output.colorless = self.colorless;
        if state.output.is_following() {
            state
                .output
                .on_append(term_lines.len() as u16, out_area.height.max(1));
        }
        TerminalOutput::new(&meta, &term_lines, self.system)
            // List navigation owns keyboard focus in the split view. Output
            // remains scrollable by pointer, but must not advertise a second
            // active focus target beside the selected task.
            .focused(false)
            .colorless(self.colorless)
            .show_chrome(false)
            .render(out_area, buffer, &mut state.output);
        let _ = (display_cols, TerminalPaintMode::Ansi, Modifier::BOLD);
    }
}

impl StatefulWidget for &BackgroundTaskPanel<'_> {
    type State = BackgroundTaskPanelState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for BackgroundTaskPanel<'_> {
    type State = BackgroundTaskPanelState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo tasks.
#[must_use]
pub fn example_background_tasks() -> Vec<BackgroundTask> {
    let mut out1 = BackgroundOutputBuffer::new(8);
    for i in 0..12 {
        out1.append(BackgroundOutputLine::stdout(
            format!("o{i}"),
            format!("build line {i}"),
        ));
    }
    let mut out2 = BackgroundOutputBuffer::default_history();
    out2.append(BackgroundOutputLine::system("s0", "watching src/**"));
    out2.append(BackgroundOutputLine::stdout("o0", "changed: main.rs"));

    vec![
        BackgroundTask::new("b1", "cargo watch")
            .kind(BackgroundTaskKind::Watcher)
            .command("cargo watch -x test")
            .status(BackgroundTaskStatus::Running)
            .restart_count(2)
            .duration_ms(120_000)
            .pid(4242)
            .resources("cpu 3% · rss 80M")
            .with_output(out2)
            .notify_on_finish(true),
        BackgroundTask::new("b2", "vite dev")
            .kind(BackgroundTaskKind::Server)
            .command("npm run dev")
            .status(BackgroundTaskStatus::Running)
            .port("5173")
            .duration_ms(45_000)
            .pid(4300)
            .resources("cpu 1%"),
        BackgroundTask::new("b3", "nightly build")
            .kind(BackgroundTaskKind::Command)
            .command("cargo build --release")
            .status(BackgroundTaskStatus::Failed)
            .duration_ms(90_000)
            .status_note("exit 101")
            .with_output(out1)
            .notify_on_finish(true),
        BackgroundTask::new("b4", "orphan job")
            .kind(BackgroundTaskKind::Job)
            .status(BackgroundTaskStatus::Lost)
            .status_note("pid 99 gone")
            .restart_count(1)
            .duration_ms(10_000),
        BackgroundTask::new("b5", "reconnect ssh")
            .kind(BackgroundTaskKind::Server)
            .status(BackgroundTaskStatus::Reconnecting)
            .status_note("backoff 2s")
            .port("22"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Tasks.
    pub const TASK_COUNT: usize = 48;
    /// Frames.
    pub const PAINT_FRAMES: u32 = 20;
    /// Lines per task.
    pub const LINES: usize = 64;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_output_drops_oldest() {
        let mut buf = BackgroundOutputBuffer::new(3);
        for i in 0..5 {
            buf.append(BackgroundOutputLine::stdout(
                format!("{i}"),
                format!("L{i}"),
            ));
        }
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.dropped, 2);
        assert!(buf.dropped_banner().unwrap().contains("2"));
        let texts: Vec<_> = buf.lines().iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["L2", "L3", "L4"]);
    }

    #[test]
    fn stop_restart_clear_outcomes() {
        let tasks = example_background_tasks();
        let mut st = BackgroundTaskPanelState::new();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        BackgroundTaskPanel::new(&tasks, &system).paint(area, &mut buf, &mut st);
        st.list.select(Some("b1".into()));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), &tasks),
            BackgroundTaskPanelOutcome::StopRequested { id } if id == "b1"
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), &tasks),
            BackgroundTaskPanelOutcome::RestartRequested { id } if id == "b1"
        ));
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('C'), KeyModifiers::SHIFT),
            &tasks,
        );
        match out {
            BackgroundTaskPanelOutcome::ClearCompleted { ids } => {
                assert!(ids.iter().any(|i| i == "b3"));
                assert!(!ids.iter().any(|i| i == "b1"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn esc_closes() {
        let tasks = example_background_tasks();
        let mut st = BackgroundTaskPanelState::new();
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &tasks),
            BackgroundTaskPanelOutcome::Closed
        ));
        assert!(!st.open);
    }

    #[test]
    fn lost_and_reconnect_status() {
        assert_eq!(
            BackgroundTaskStatus::Lost.semantic(),
            SemanticStatus::Failed
        );
        assert!(BackgroundTaskStatus::Reconnecting.can_stop());
        assert!(BackgroundTaskStatus::Lost.can_restart());
        let t = BackgroundTask::new("x", "y").status(BackgroundTaskStatus::Lost);
        let m = background_task_to_activity(&t);
        assert!(m.blocked);
    }

    #[test]
    fn notification_on_finish() {
        let t = BackgroundTask::new("b", "job")
            .status(BackgroundTaskStatus::Succeeded)
            .notify_on_finish(true);
        let n = background_task_to_notification(&t, 1).unwrap();
        assert_eq!(n.kind, ToastKind::Success);
        let silent = BackgroundTask::new("b", "job").status(BackgroundTaskStatus::Succeeded);
        assert!(background_task_to_notification(&silent, 1).is_none());
        let lost = BackgroundTask::new("b", "job").status(BackgroundTaskStatus::Lost);
        assert!(background_task_to_notification(&lost, 1).is_some());
    }

    #[test]
    fn activity_bridge() {
        let t = example_background_tasks()[0].clone();
        let m = background_task_to_activity(&t);
        assert_eq!(m.scope, ActivityScope::Watcher);
    }

    #[test]
    fn paint_pane_and_rail() {
        let system = DesignSystem::default();
        let tasks = example_background_tasks();
        let mut st = BackgroundTaskPanelState::new();
        st.list.select(Some("b3".into()));
        let area = Rect::new(0, 0, 90, 22);
        let mut buf = Buffer::empty(area);
        BackgroundTaskPanel::new(&tasks, &system).paint(area, &mut buf, &mut st);
        st.force_presentation = Some(BackgroundTaskPresentation::CompactRail);
        BackgroundTaskPanel::new(&tasks, &system).paint(Rect::new(0, 0, 24, 12), &mut buf, &mut st);

        let compact = Rect::new(0, 0, 24, 12);
        let mut monochrome = Buffer::empty(compact);
        BackgroundTaskPanel::new(&tasks, &system)
            .colorless(true)
            .paint(compact, &mut monochrome, &mut st);
        let text: String = monochrome
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            text.contains("◉ running") && text.contains("✗ failed"),
            "no-color must preserve semantic Unicode glyphs and verbs: {text}"
        );
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let system = DesignSystem::default();
        let label = "監視 Cafe\u{301}";
        let tasks = [BackgroundTask::new("unicode", label)
            .status(BackgroundTaskStatus::Failed)
            .status_note("失敗 Cafe\u{301}")];
        for _ in [false, true] {
            for (width, height) in [(64, 14), (24, 5), (1, 1), (0, 0)] {
                let area = Rect::new(0, 0, width, height);
                let mut buffer = Buffer::empty(area);
                let mut state = BackgroundTaskPanelState::new();
                state.list.select(Some("unicode".into()));
                BackgroundTaskPanel::new(&tasks, &system).paint(area, &mut buffer, &mut state);
                if width == 64 {
                    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains('監'), "{text:?}");
                    assert!(text.contains("Cafe\u{301}"), "{text:?}");
                    assert!(text.contains("failed"), "{text:?}");
                }
            }
        }
    }

    #[test]
    fn selected_output_copy_stays_visible_without_claiming_second_focus() {
        let system = DesignSystem::junie();
        let tasks = example_background_tasks();
        let mut state = BackgroundTaskPanelState::new();
        state.list.select(Some("b1".into()));
        state.focused = true;
        let area = Rect::new(0, 0, 88, 20);
        let mut buffer = Buffer::empty(area);

        BackgroundTaskPanel::new(&tasks, &system).paint(area, &mut buffer, &mut state);

        let selected_output_y = (area.top()..area.bottom())
            .find(|&y| {
                (area.left()..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .contains("changed: main.rs")
            })
            .expect("selected output line remains painted");
        let copy_x = (area.left()..area.right())
            .find(|&x| {
                (x..area.right())
                    .map(|tail_x| buffer[(tail_x, selected_output_y)].symbol())
                    .collect::<String>()
                    .starts_with("changed: main.rs")
            })
            .expect("selected output copy has a stable start cell");
        let copy = &buffer[(copy_x, selected_output_y)];
        // The tint rides the keyboard: the task list owns focus, so the
        // parked output selection is marked, not tinted, and its copy reads
        // as secondary metadata.
        assert_eq!(
            copy.fg,
            system.style(Role::Text).fg.unwrap(),
            "parked selection copy reads as ordinary body copy"
        );
        assert_ne!(
            copy.bg,
            system.style(Role::SelectionTint).bg.unwrap(),
            "the parked row never wears the focused tint"
        );

        let title = "Background · 2 run · 1 lost";
        let inner = Panel::new(&system)
            .title(title)
            .emphasis(PanelChrome::Focused)
            .inner(area);
        let detail_x = inner.x.saturating_add((inner.width / 3).clamp(12, 28));
        let parked = &buffer[(detail_x, selected_output_y)];
        assert_ne!(
            parked.bg,
            system.style(Role::SelectionTint).bg.unwrap(),
            "parked output cursor is muted while the task list owns focus"
        );
    }

    #[test]
    fn follow_toggle() {
        let tasks = example_background_tasks();
        let mut st = BackgroundTaskPanelState::new();
        assert!(st.output.is_following());
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &tasks
            ),
            BackgroundTaskPanelOutcome::FollowChanged { following: false }
        ));
    }

    #[test]
    fn never_process_control() {
        let src = include_str!("background_task_panel.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in [
            "std::process",
            "Command::new",
            "portable_pty",
            "kill(",
            "nix::",
            "tokio::process",
        ] {
            assert!(!body.contains(f), "{f}");
        }
        assert!(body.contains("StopRequested") || body.contains("requests only"));
    }

    #[test]
    fn accepts_input_gate() {
        let tasks = example_background_tasks();
        let mut st = BackgroundTaskPanelState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &tasks),
            BackgroundTaskPanelOutcome::Ignored
        ));
    }

    #[test]
    fn paint_perf_budget() {
        let system = DesignSystem::default();
        let mut tasks = example_background_tasks();
        for i in 0..bench::TASK_COUNT {
            let mut buf = BackgroundOutputBuffer::new(32);
            for j in 0..bench::LINES {
                buf.append(BackgroundOutputLine::stdout(
                    format!("{i}-{j}"),
                    format!("line {j}"),
                ));
            }
            tasks.push(
                BackgroundTask::new(format!("t{i}"), format!("job {i}"))
                    .status(BackgroundTaskStatus::Running)
                    .with_output(buf),
            );
        }
        let area = Rect::new(0, 0, 100, 24);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            let mut st = BackgroundTaskPanelState::new();
            st.list.select(Some("t0".into()));
            BackgroundTaskPanel::new(&tasks, &system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 8, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_kinds_statuses() {
        for k in [
            BackgroundTaskKind::Command,
            BackgroundTaskKind::Watcher,
            BackgroundTaskKind::Server,
            BackgroundTaskKind::Job,
        ] {
            assert!(!k.id().is_empty());
        }
        for s in [
            BackgroundTaskStatus::Pending,
            BackgroundTaskStatus::Running,
            BackgroundTaskStatus::Reconnecting,
            BackgroundTaskStatus::Lost,
            BackgroundTaskStatus::Succeeded,
            BackgroundTaskStatus::Failed,
            BackgroundTaskStatus::Stopped,
            BackgroundTaskStatus::Detached,
        ] {
            assert!(!s.id().is_empty());
            let _ = s.to_terminal_status();
            let _ = s.glyph();
        }
    }

    #[test]
    fn hide_completed_filter() {
        let tasks = example_background_tasks();
        let mut st = BackgroundTaskPanelState::new();
        st.hide_completed = true;
        let v = st.visible_tasks(&tasks);
        assert!(v.iter().all(|t| !t.status.is_terminal()));
    }
}
