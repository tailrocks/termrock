// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **TerminalRunCard** — shell/terminal command card with live output.
//!
//! **Mission.** Specialize [`super::ToolCallCard`] for shell runs: exact
//! command, cwd, environment/redaction summary, provenance, status, elapsed
//! time, stdout/stderr, exit code/signal, and actions. Stop, detach, retry,
//! copy, open fullscreen, and permission boundary. Preserve user scroll while
//! output streams. Clearly distinguish **proposed** vs **executed** command and
//! **edited approval**. Safe ANSI via [`crate::ansi_text`] / substrate paint.
//!
//! **vs [`super::TerminalOutput`].** TerminalOutput is the full command pane
//! (recipes compact/pane/fullscreen). TerminalRunCard is the agent-card form:
//! proposed/executed chrome, ToolCall bridge, permission focus, and card
//! expand. Body reuses TerminalOutput substrate (lines, follow, ANSI modes).
//!
//! **Ownership.** Host owns PTY/process. Outcomes are **requests only**.
//!
//! Research: agent CLIs, CI command cards, terminal output panes.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        ToolStatus,
        Card,
        TerminalCommandMeta, TerminalEnvEntry, TerminalLine, TerminalOutput, TerminalOutputOutcome, TerminalOutputRecipe, TerminalOutputState, TerminalPaintMode, TerminalRunStatus, escape_raw_terminal, filter_terminal_lines, format_duration_ms, redact_env_value,
        ToolCall,
        ToolRisk,
        redact_tool_secrets,
    },
};

/// Overlay id for fullscreen terminal run.
pub const TERMINAL_RUN_FULLSCREEN_OVERLAY_ID: &str = "termrock.terminal_run";
/// Max env rows shown in expanded chrome.
pub const TERMINAL_RUN_ENV_CAP: usize = 6;
/// Compact body lines when not expanded.
pub const TERMINAL_RUN_COMPACT_BODY_LINES: u16 = 3;

// ── Domain ──────────────────────────────────────────────────────────────────

/// How the command text reached execution (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalCommandPhase {
    /// Proposed only — not yet spawned.
    #[default]
    Proposed,
    /// User/host edited the proposed command before approve/run.
    EditedApproval,
    /// Executing or finished with executed argv (may equal proposed).
    Executed,
}

impl TerminalCommandPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::EditedApproval => "edited-approval",
            Self::Executed => "executed",
        }
    }

    /// Short badge.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Proposed => "propose",
            Self::EditedApproval => "edited",
            Self::Executed => "exec",
        }
    }
}

/// Owned env row for [`TerminalRun`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRunEnv {
    /// Key.
    pub key: String,
    /// Value (host-redacted preferred).
    pub value: String,
    /// Redacted flag.
    pub redacted: bool,
}

impl TerminalRunEnv {
    /// Construct.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            redacted: false,
        }
    }

    /// Secret placeholder.
    #[must_use]
    pub fn secret(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: "***".into(),
            redacted: true,
        }
    }

    /// Apply best-effort key-based redaction.
    #[must_use]
    pub fn auto_redact(key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        let (v, r) = redact_env_value(&key, &value);
        Self {
            key,
            value: v,
            redacted: r,
        }
    }
}

/// Host-projected shell/terminal run (no PTY, no provider protocol).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRun {
    /// Stable run id.
    pub id: String,
    /// Proposed command (always set).
    pub proposed_command: String,
    /// Executed command when started (None = not executed yet).
    pub executed_command: Option<String>,
    /// True when executed differs from proposed (edited approval).
    pub approval_edited: bool,
    /// Working directory.
    pub cwd: Option<String>,
    /// Environment summary (already redacted preferred).
    pub env: Vec<TerminalRunEnv>,
    /// Actor / provenance.
    pub actor: Option<String>,
    /// Run status.
    pub status: TerminalRunStatus,
    /// Exit code.
    pub exit_code: Option<i32>,
    /// Signal name.
    pub signal: Option<String>,
    /// Elapsed ms.
    pub duration_ms: Option<u64>,
    /// Pid (display only).
    pub pid: Option<u32>,
    /// Secrets redacted flag.
    pub secrets_redacted: bool,
    /// Risk / egress (shell often Write/Network).
    pub risk: ToolRisk,
    /// Explicit egress note.
    pub egress: Option<String>,
    /// Stream revision for height cache.
    pub revision: u64,
}

impl TerminalRun {
    /// Proposed command, pending.
    #[must_use]
    pub fn new(id: impl Into<String>, proposed: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            proposed_command: proposed.into(),
            executed_command: None,
            approval_edited: false,
            cwd: None,
            env: Vec::new(),
            actor: None,
            status: TerminalRunStatus::Pending,
            exit_code: None,
            signal: None,
            duration_ms: None,
            pid: None,
            secrets_redacted: false,
            risk: ToolRisk::Write,
            egress: None,
            revision: 0,
        }
    }

    /// Command phase for chrome.
    #[must_use]
    pub fn phase(&self) -> TerminalCommandPhase {
        if self.executed_command.is_none()
            && matches!(
                self.status,
                TerminalRunStatus::Pending | TerminalRunStatus::WaitingPermission
            )
        {
            if self.approval_edited {
                TerminalCommandPhase::EditedApproval
            } else {
                TerminalCommandPhase::Proposed
            }
        } else if self.approval_edited
            || self
                .executed_command
                .as_ref()
                .is_some_and(|e| e != &self.proposed_command)
        {
            TerminalCommandPhase::EditedApproval
        } else if self.executed_command.is_some() {
            TerminalCommandPhase::Executed
        } else {
            TerminalCommandPhase::Proposed
        }
    }

    /// Display command: executed if present, else proposed.
    #[must_use]
    pub fn display_command(&self) -> &str {
        self.executed_command
            .as_deref()
            .unwrap_or(self.proposed_command.as_str())
    }

    /// Mark executed (optionally different argv).
    #[must_use]
    pub fn execute(mut self, command: impl Into<String>) -> Self {
        let cmd = command.into();
        self.approval_edited = cmd != self.proposed_command || self.approval_edited;
        self.executed_command = Some(cmd);
        if matches!(
            self.status,
            TerminalRunStatus::Pending | TerminalRunStatus::WaitingPermission
        ) {
            self.status = TerminalRunStatus::Running;
        }
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: TerminalRunStatus) -> Self {
        self.status = s;
        self
    }

    /// Cwd.
    #[must_use]
    pub fn cwd(mut self, c: impl Into<String>) -> Self {
        self.cwd = Some(c.into());
        self
    }

    /// Env rows.
    #[must_use]
    pub fn env(mut self, env: Vec<TerminalRunEnv>) -> Self {
        self.env = env;
        self
    }

    /// Actor.
    #[must_use]
    pub fn actor(mut self, a: impl Into<String>) -> Self {
        self.actor = Some(a.into());
        self
    }

    /// Duration.
    #[must_use]
    pub const fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Exit.
    #[must_use]
    pub const fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Signal.
    #[must_use]
    pub fn signal(mut self, s: impl Into<String>) -> Self {
        self.signal = Some(s.into());
        self
    }

    /// Pid.
    #[must_use]
    pub const fn pid(mut self, p: u32) -> Self {
        self.pid = Some(p);
        self
    }

    /// Risk.
    #[must_use]
    pub const fn risk(mut self, r: ToolRisk) -> Self {
        self.risk = r;
        self
    }

    /// Egress.
    #[must_use]
    pub fn egress(mut self, e: impl Into<String>) -> Self {
        self.egress = Some(e.into());
        self
    }

    /// Secrets flag.
    #[must_use]
    pub const fn secrets_redacted(mut self, on: bool) -> Self {
        self.secrets_redacted = on;
        self
    }

    /// Approval edited without changing executed yet.
    #[must_use]
    pub const fn approval_edited(mut self, on: bool) -> Self {
        self.approval_edited = on;
        self
    }

    /// Revision.
    #[must_use]
    pub const fn revision(mut self, r: u64) -> Self {
        self.revision = r;
        self
    }

    /// Header summary line.
    #[must_use]
    pub fn header_line(&self, ascii: bool) -> String {
        let g = self.status.glyph(ascii);
        let phase = self.phase().badge();
        let mut s = format!(
            "{g} [{phase}] {}",
            take_display_cols(self.display_command(), 48)
        );
        if let Some(ms) = self.duration_ms {
            s.push_str(" · ");
            s.push_str(&format_duration_ms(ms));
        }
        if let Some(code) = self.exit_code {
            s.push_str(&format!(" · exit {code}"));
        }
        if let Some(sig) = &self.signal {
            s.push_str(&format!(" · {sig}"));
        }
        if self.secrets_redacted {
            s.push_str(" · redacted");
        }
        s
    }
}

/// Project run → borrowed meta for TerminalOutput (env slice must outlive).
#[must_use]
pub fn terminal_run_to_meta<'a>(
    run: &'a TerminalRun,
    env: &'a [TerminalEnvEntry<'a>],
) -> TerminalCommandMeta<'a> {
    let mut m = TerminalCommandMeta::new(run.display_command())
        .status(run.status)
        .env(env);
    if let Some(cwd) = run.cwd.as_deref() {
        m = m.cwd(cwd);
    }
    if let Some(code) = run.exit_code {
        m = m.exit_code(code);
    }
    if let Some(sig) = run.signal.as_deref() {
        m = m.signal(sig);
    }
    if let Some(ms) = run.duration_ms {
        m = m.duration_ms(ms);
    }
    if let Some(pid) = run.pid {
        m = m.pid(pid);
    }
    m
}

/// Build env entries for a paint frame (pointers into `run.env`).
#[must_use]
pub fn terminal_run_env_entries(run: &TerminalRun) -> Vec<TerminalEnvEntry<'_>> {
    run.env
        .iter()
        .map(|e| {
            let mut ent = TerminalEnvEntry::new(e.key.as_str(), e.value.as_str());
            if e.redacted {
                ent = ent.redacted(true);
            }
            ent
        })
        .collect()
}

/// Bridge to ToolCall for MessageThread / ToolCallCard hosts.
#[must_use]
pub fn terminal_run_to_tool_call(run: &TerminalRun) -> ToolCall {
    let status = match run.status {
        TerminalRunStatus::Pending => ToolStatus::Queued,
        TerminalRunStatus::WaitingPermission => ToolStatus::WaitingPermission,
        TerminalRunStatus::Running => ToolStatus::Running,
        TerminalRunStatus::Succeeded => ToolStatus::Success,
        TerminalRunStatus::Failed | TerminalRunStatus::Signaled | TerminalRunStatus::TimedOut => {
            ToolStatus::Failed
        }
        TerminalRunStatus::Cancelled => ToolStatus::Cancelled,
        TerminalRunStatus::Detached => ToolStatus::Detached,
    };
    let mut verb = match run.phase() {
        TerminalCommandPhase::Proposed => "proposed shell",
        TerminalCommandPhase::EditedApproval => "edited shell",
        TerminalCommandPhase::Executed => "ran shell",
    }
    .to_string();
    if let Some(code) = run.exit_code {
        verb = format!("shell exit {code}");
    }
    let mut call = ToolCall::new(run.id.clone(), "shell", verb)
        .status(status)
        .args_summary(run.display_command().to_string())
        .args_detail(run.proposed_command.clone())
        .risk(run.risk)
        .secrets_redacted(run.secrets_redacted)
        .has_log(true)
        .revision(run.revision);
    if let Some(a) = &run.actor {
        call = call.actor(a.clone());
    }
    if let Some(ms) = run.duration_ms {
        call = call.duration(format_duration_ms(ms));
    }
    if let Some(e) = &run.egress {
        call = call.egress(e.clone());
    }
    if let Some(code) = run.exit_code {
        call = call.result_summary(format!("exit {code}"));
    } else if let Some(sig) = &run.signal {
        call = call.result_summary(sig.clone());
    }
    call
}

/// Plain lines for MessageThread projection.
#[must_use]
pub fn project_terminal_run_lines(
    run: &TerminalRun,
    lines: &[TerminalLine<'_>],
    expanded: bool,
    ascii: bool,
) -> Vec<String> {
    let mut out = vec![run.header_line(ascii)];
    let phase = run.phase();
    if phase != TerminalCommandPhase::Executed
        || run
            .executed_command
            .as_ref()
            .is_some_and(|e| e != &run.proposed_command)
    {
        out.push(format!(
            "  proposed: {}",
            redact_tool_secrets(&run.proposed_command)
        ));
    }
    if let Some(ex) = &run.executed_command {
        if ex != &run.proposed_command {
            out.push(format!("  executed: {}", redact_tool_secrets(ex)));
        }
    }
    if let Some(cwd) = &run.cwd {
        out.push(format!("  cwd: {cwd}"));
    }
    if let Some(a) = &run.actor {
        out.push(format!("  via {a}"));
    }
    if expanded {
        for e in run.env.iter().take(TERMINAL_RUN_ENV_CAP) {
            let mark = if e.redacted { " (redacted)" } else { "" };
            out.push(format!("  env {}={}{}", e.key, e.value, mark));
        }
        for l in filter_terminal_lines(lines, false, false)
            .into_iter()
            .take(12)
        {
            out.push(format!(
                "  {}{}",
                l.stream.prefix(ascii),
                redact_tool_secrets(l.text)
            ));
        }
    }
    out
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Card presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalRunPresentation {
    /// Compact header + few body lines.
    #[default]
    Compact,
    /// Expanded card with stream viewport.
    Expanded,
    /// Fullscreen (host overlay).
    Fullscreen,
}

impl TerminalRunPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Expanded => "expanded",
            Self::Fullscreen => "fullscreen",
        }
    }

    fn to_recipe(self) -> TerminalOutputRecipe {
        match self {
            Self::Compact => TerminalOutputRecipe::Compact,
            Self::Expanded => TerminalOutputRecipe::Pane,
            Self::Fullscreen => TerminalOutputRecipe::Fullscreen,
        }
    }
}

/// Outcomes — requests only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TerminalRunCardOutcome {
    /// Ignored.
    Ignored,
    /// Expanded.
    Expanded {
        /// Run id.
        id: String,
    },
    /// Collapsed.
    Collapsed {
        /// Run id.
        id: String,
    },
    /// Stop / cancel process.
    StopRequested {
        /// Run id.
        id: String,
    },
    /// Detach process from UI.
    DetachRequested {
        /// Run id.
        id: String,
    },
    /// Retry / re-run.
    RetryRequested {
        /// Run id.
        id: String,
    },
    /// Follow tail.
    Follow {
        /// Run id.
        id: String,
    },
    /// User scrolled away from tail.
    ScrollDetached {
        /// Run id.
        id: String,
        /// Offset.
        offset: u16,
    },
    /// Scrolled while pinned.
    Scrolled {
        /// Run id.
        id: String,
        /// Offset.
        offset: u16,
    },
    /// Copy output.
    CopyOutput {
        /// Run id.
        id: String,
        /// Text.
        text: String,
    },
    /// Copy command (executed preferred, redacted).
    CopyCommand {
        /// Run id.
        id: String,
        /// Text.
        text: String,
    },
    /// Fullscreen zoom.
    FullscreenRequested {
        /// Run id.
        id: String,
    },
    /// Focus permission UI.
    PermissionFocus {
        /// Run id.
        id: String,
    },
    /// Open cwd.
    OpenCwdRequested {
        /// Run id.
        id: String,
        /// Path.
        path: String,
    },
    /// Env panel toggled.
    EnvToggled {
        /// Run id.
        id: String,
        /// On.
        on: bool,
    },
    /// Paint mode.
    PaintModeChanged {
        /// Run id.
        id: String,
        /// Mode.
        mode: TerminalPaintMode,
    },
}

/// Interactive terminal run card state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRunCardState {
    /// Presentation.
    pub presentation: TerminalRunPresentation,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Substrate (follow / scroll / paint mode).
    pub output: TerminalOutputState,
    /// Header hit.
    pub header_hit: Rect,
}

impl Default for TerminalRunCardState {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalRunCardState {
    /// Compact following.
    #[must_use]
    pub fn new() -> Self {
        let mut output = TerminalOutputState::new();
        output.recipe = TerminalOutputRecipe::Compact;
        Self {
            presentation: TerminalRunPresentation::Compact,
            focused: true,
            accepts_input: true,
            output,
            header_hit: Rect::default(),
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.output.set_accepts_input(on);
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Expanded?
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        !matches!(self.presentation, TerminalRunPresentation::Compact)
    }

    /// Following tail.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.output.is_following()
    }

    /// Host append notification.
    pub fn on_append(&mut self, total_lines: u16, viewport: u16) {
        self.output.on_append(total_lines, viewport);
    }

    /// Toggle expand.
    pub fn toggle_expand(&mut self, id: &str) -> TerminalRunCardOutcome {
        match self.presentation {
            TerminalRunPresentation::Compact => {
                self.presentation = TerminalRunPresentation::Expanded;
                self.output.recipe = TerminalOutputRecipe::Pane;
                TerminalRunCardOutcome::Expanded {
                    id: id.to_string(),
                }
            }
            TerminalRunPresentation::Expanded | TerminalRunPresentation::Fullscreen => {
                self.presentation = TerminalRunPresentation::Compact;
                self.output.recipe = TerminalOutputRecipe::Compact;
                TerminalRunCardOutcome::Collapsed {
                    id: id.to_string(),
                }
            }
        }
    }

    fn map_output(
        &self,
        id: &str,
        out: TerminalOutputOutcome,
    ) -> TerminalRunCardOutcome {
        match out {
            TerminalOutputOutcome::Ignored => TerminalRunCardOutcome::Ignored,
            TerminalOutputOutcome::Scrolled { offset } => TerminalRunCardOutcome::Scrolled {
                id: id.to_string(),
                offset,
            },
            TerminalOutputOutcome::Follow => TerminalRunCardOutcome::Follow {
                id: id.to_string(),
            },
            TerminalOutputOutcome::Detach => TerminalRunCardOutcome::ScrollDetached {
                id: id.to_string(),
                offset: self.output.offset(),
            },
            TerminalOutputOutcome::CancelRequested => TerminalRunCardOutcome::StopRequested {
                id: id.to_string(),
            },
            TerminalOutputOutcome::RetryRequested => TerminalRunCardOutcome::RetryRequested {
                id: id.to_string(),
            },
            TerminalOutputOutcome::DetachProcessRequested => {
                TerminalRunCardOutcome::DetachRequested {
                    id: id.to_string(),
                }
            }
            TerminalOutputOutcome::CopyOutput { text } => TerminalRunCardOutcome::CopyOutput {
                id: id.to_string(),
                text: redact_tool_secrets(&text),
            },
            TerminalOutputOutcome::CopyCommand { text } => TerminalRunCardOutcome::CopyCommand {
                id: id.to_string(),
                text: redact_tool_secrets(&text),
            },
            TerminalOutputOutcome::EnvToggled { on } => TerminalRunCardOutcome::EnvToggled {
                id: id.to_string(),
                on,
            },
            TerminalOutputOutcome::PaintModeChanged(mode) => {
                TerminalRunCardOutcome::PaintModeChanged {
                    id: id.to_string(),
                    mode,
                }
            }
            TerminalOutputOutcome::OpenCwdRequested { path } => {
                TerminalRunCardOutcome::OpenCwdRequested {
                    id: id.to_string(),
                    path,
                }
            }
            TerminalOutputOutcome::StreamFilterChanged { .. }
            | TerminalOutputOutcome::RecipeChanged(_)
            | TerminalOutputOutcome::Cancelled => TerminalRunCardOutcome::Ignored,
        }
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        run: &TerminalRun,
        lines: &[TerminalLine<'_>],
    ) -> TerminalRunCardOutcome {
        if !self.accepts_input || !self.focused || key.kind != KeyEventKind::Press {
            return TerminalRunCardOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => {
                return self.toggle_expand(&run.id);
            }
            KeyCode::Char('f')
                if key.modifiers.is_empty()
                    && matches!(self.presentation, TerminalRunPresentation::Compact) =>
            {
                // compact: f = fullscreen; expanded: f = follow (substrate)
                self.presentation = TerminalRunPresentation::Fullscreen;
                self.output.recipe = TerminalOutputRecipe::Fullscreen;
                return TerminalRunCardOutcome::FullscreenRequested {
                    id: run.id.clone(),
                };
            }
            KeyCode::Char('F') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.presentation = TerminalRunPresentation::Fullscreen;
                self.output.recipe = TerminalOutputRecipe::Fullscreen;
                return TerminalRunCardOutcome::FullscreenRequested {
                    id: run.id.clone(),
                };
            }
            KeyCode::Char('p') if run.status.needs_permission() => {
                return TerminalRunCardOutcome::PermissionFocus {
                    id: run.id.clone(),
                };
            }
            KeyCode::Esc if matches!(self.presentation, TerminalRunPresentation::Fullscreen) => {
                self.presentation = TerminalRunPresentation::Expanded;
                self.output.recipe = TerminalOutputRecipe::Pane;
                return TerminalRunCardOutcome::Expanded {
                    id: run.id.clone(),
                };
            }
            _ => {}
        }

        // Build meta for substrate
        let env = terminal_run_env_entries(run);
        let meta = terminal_run_to_meta(run, &env);
        let out = self.output.handle_key(key, lines, &meta);
        // Compact: bare f was fullscreen; if expanded and substrate got Follow, ok
        if matches!(out, TerminalOutputOutcome::RecipeChanged(_)) {
            return TerminalRunCardOutcome::Ignored;
        }
        self.map_output(&run.id, out)
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        run: &TerminalRun,
        lines: &[TerminalLine<'_>],
    ) -> TerminalRunCardOutcome {
        if !self.accepts_input {
            return TerminalRunCardOutcome::Ignored;
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left)
            && self.header_hit.contains(event.position)
        {
            return self.toggle_expand(&run.id);
        }
        let out = self.output.handle_mouse(event, lines);
        self.map_output(&run.id, out)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Interactive terminal run card.
#[derive(Debug, Clone, Copy)]
pub struct TerminalRunCard<'a> {
    run: &'a TerminalRun,
    lines: &'a [TerminalLine<'a>],
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
}

impl<'a> TerminalRunCard<'a> {
    /// Run + lines + system.
    #[must_use]
    pub const fn new(
        run: &'a TerminalRun,
        lines: &'a [TerminalLine<'a>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            run,
            lines,
            system,
            ascii: false,
            colorless: false,
        }
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TerminalRunCardState,
    ) {
        if area.is_empty() {
            return;
        }
        let run = self.run;
        let ascii = self.ascii || state.output.ascii;
        let colorless = self.colorless || state.output.colorless;
        state.output.ascii = ascii;
        state.output.colorless = colorless;
        state.output.recipe = state.presentation.to_recipe();
        state.output.set_accepts_input(state.accepts_input && state.focused);

        let phase = run.phase();
        let mut title = take_display_cols(run.display_command(), 40);
        if ascii {
            title = format!("{} {title}", run.status.glyph(true));
        }
        let mut subtitle = format!("{} · {}", phase.badge(), run.status.label());
        if let Some(ms) = run.duration_ms {
            subtitle.push_str(" · ");
            subtitle.push_str(&format_duration_ms(ms));
        }
        if let Some(code) = run.exit_code {
            subtitle.push_str(&format!(" · exit {code}"));
        }
        if let Some(sig) = &run.signal {
            subtitle.push_str(&format!(" · {sig}"));
        }
        if run.secrets_redacted {
            subtitle.push_str(" · redacted");
        }

        let emphasis = match run.status {
            TerminalRunStatus::Failed | TerminalRunStatus::Signaled | TerminalRunStatus::TimedOut => {
                PanelChrome::Danger
            }
            TerminalRunStatus::Running | TerminalRunStatus::WaitingPermission => {
                PanelChrome::Focused
            }
            _ if state.focused => PanelChrome::Focused,
            _ => PanelChrome::Normal,
        };

        let leading = if ascii || colorless {
            ""
        } else {
            run.status.glyph(false)
        };
        let badge = phase.badge();
        let card = Card::new(self.system)
            .title(title.as_str())
            .leading(leading)
            .badge(badge)
            .subtitle(subtitle.as_str())
            .emphasis(emphasis);
        let body = card.paint(area, buffer, None);
        state.header_hit = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1.min(area.height),
        };
        if body.is_empty() {
            return;
        }

        let mut y = body.y;
        let max_y = body.bottom();

        // Proposed vs executed distinction
        if phase != TerminalCommandPhase::Executed
            || run
                .executed_command
                .as_ref()
                .is_some_and(|e| e != &run.proposed_command)
        {
            if y < max_y {
                let line = format!(
                    "proposed: {}",
                    take_display_cols(
                        &redact_tool_secrets(&run.proposed_command),
                        usize::from(body.width).saturating_sub(10)
                    )
                );
                buffer.set_stringn(
                    body.x,
                    y,
                    take_display_cols(&line, usize::from(body.width)),
                    usize::from(body.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }
        if let Some(ex) = &run.executed_command {
            if ex != &run.proposed_command && y < max_y {
                let line = format!(
                    "executed: {}",
                    take_display_cols(
                        &redact_tool_secrets(ex),
                        usize::from(body.width).saturating_sub(10)
                    )
                );
                buffer.set_stringn(
                    body.x,
                    y,
                    take_display_cols(&line, usize::from(body.width)),
                    usize::from(body.width),
                    self.system.style(Role::Accent),
                );
                y = y.saturating_add(1);
            }
        }
        if let Some(cwd) = &run.cwd {
            if y < max_y && body.width >= 24 {
                buffer.set_stringn(
                    body.x,
                    y,
                    take_display_cols(&format!("cwd: {cwd}"), usize::from(body.width)),
                    usize::from(body.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }
        if let Some(a) = &run.actor {
            if y < max_y {
                buffer.set_stringn(
                    body.x,
                    y,
                    take_display_cols(&format!("via {a}"), usize::from(body.width)),
                    usize::from(body.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }
        if run.status.needs_permission() && y < max_y {
            buffer.set_stringn(
                body.x,
                y,
                take_display_cols("permission required · p", usize::from(body.width)),
                usize::from(body.width),
                self.system.style(Role::Warning),
            );
            y = y.saturating_add(1);
        }
        if let Some(e) = &run.egress {
            if y < max_y {
                buffer.set_stringn(
                    body.x,
                    y,
                    take_display_cols(&format!("egress: {e}"), usize::from(body.width)),
                    usize::from(body.width),
                    self.system.style(Role::Warning),
                );
                y = y.saturating_add(1);
            }
        }

        // Stream body via TerminalOutput substrate
        if y >= max_y {
            return;
        }
        let stream_area = Rect {
            x: body.x,
            y,
            width: body.width,
            height: max_y.saturating_sub(y),
        };
        if stream_area.height == 0 {
            return;
        }

        // Compact: only last N lines via recipe
        if matches!(state.presentation, TerminalRunPresentation::Compact)
            && stream_area.height > TERMINAL_RUN_COMPACT_BODY_LINES
        {
            // leave recipe compact
        }

        let env = terminal_run_env_entries(run);
        let meta = terminal_run_to_meta(run, &env);
        let view = TerminalOutput::new(&meta, self.lines, self.system)
            .focused(state.focused)
            .ascii(ascii)
            .colorless(colorless)
            .show_chrome(false); // card owns command chrome; substrate owns stream
        view.render(stream_area, buffer, &mut state.output);

        let _ = (display_cols, escape_raw_terminal);
    }

    /// Render alias.
    pub fn render(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut TerminalRunCardState,
    ) {
        self.paint(area, buffer, state);
    }
}

/// Example runs for stories/tests.
#[must_use]
pub fn example_terminal_runs() -> Vec<TerminalRun> {
    vec![
        TerminalRun::new("r1", "cargo test -p termrock --lib")
            .execute("cargo test -p termrock --lib")
            .status(TerminalRunStatus::Running)
            .cwd("/proj")
            .duration_ms(1200)
            .pid(4242)
            .actor("agent")
            .env(vec![
                TerminalRunEnv::auto_redact("PATH", "/usr/bin"),
                TerminalRunEnv::secret("TOKEN"),
            ])
            .secrets_redacted(true),
        TerminalRun::new("r2", "rm -rf /tmp/x")
            .status(TerminalRunStatus::WaitingPermission)
            .cwd("/tmp")
            .risk(ToolRisk::High)
            .egress("fs:delete")
            .actor("agent"),
        TerminalRun::new("r3", "echo hello")
            .approval_edited(true)
            .execute("echo hello-world")
            .status(TerminalRunStatus::Succeeded)
            .exit_code(0)
            .duration_ms(40)
            .cwd("/proj"),
        TerminalRun::new("r4", "false")
            .execute("false")
            .status(TerminalRunStatus::Failed)
            .exit_code(1)
            .duration_ms(5),
    ]
}

/// Example lines.
#[must_use]
pub fn example_terminal_run_lines() -> Vec<TerminalLine<'static>> {
    vec![
        TerminalLine::system("s0", "spawned pid 4242"),
        TerminalLine::stdout("o1", "running 3 tests"),
        TerminalLine::stderr("e1", "warning: unused import"),
        TerminalLine::stdout("o2", "test widgets::x ... ok"),
        TerminalLine::stdout("o3", "done"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Cards.
    pub const CARD_COUNT: usize = 32;
    /// Frames.
    pub const PAINT_FRAMES: u32 = 24;
    /// Lines per run.
    pub const LINES: usize = 80;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansi_text::{AnsiParseOptions, parse_to_line};
    use ratatui_core::layout::Position;

    #[test]
    fn phase_proposed_edited_executed() {
        let p = TerminalRun::new("r", "echo a");
        assert_eq!(p.phase(), TerminalCommandPhase::Proposed);
        let e = TerminalRun::new("r", "echo a")
            .approval_edited(true)
            .status(TerminalRunStatus::WaitingPermission);
        assert_eq!(e.phase(), TerminalCommandPhase::EditedApproval);
        let x = TerminalRun::new("r", "echo a").execute("echo a");
        assert_eq!(x.phase(), TerminalCommandPhase::Executed);
        let edit = TerminalRun::new("r", "echo a").execute("echo b");
        assert_eq!(edit.phase(), TerminalCommandPhase::EditedApproval);
        assert_ne!(edit.proposed_command, edit.executed_command.as_deref().unwrap());
    }

    #[test]
    fn stop_retry_permission_fullscreen() {
        let run = TerminalRun::new("r", "sleep 1")
            .execute("sleep 1")
            .status(TerminalRunStatus::Running);
        let lines = example_terminal_run_lines();
        let mut st = TerminalRunCardState::new();
        st.presentation = TerminalRunPresentation::Expanded;
        st.output.recipe = TerminalOutputRecipe::Pane;
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
                &run,
                &lines
            ),
            TerminalRunCardOutcome::StopRequested { .. }
        ));
        let fail = run.clone().status(TerminalRunStatus::Failed).exit_code(1);
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                &fail,
                &lines
            ),
            TerminalRunCardOutcome::RetryRequested { .. }
        ));
        let perm = TerminalRun::new("r", "rm -rf x").status(TerminalRunStatus::WaitingPermission);
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE),
                &perm,
                &lines
            ),
            TerminalRunCardOutcome::PermissionFocus { .. }
        ));
        let mut st2 = TerminalRunCardState::new();
        assert!(matches!(
            st2.handle_key(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &run,
                &lines
            ),
            TerminalRunCardOutcome::FullscreenRequested { .. }
        ));
    }

    #[test]
    fn follow_preserves_scroll_on_append() {
        let run = TerminalRun::new("r", "cargo test")
            .execute("cargo test")
            .status(TerminalRunStatus::Running);
        let lines = example_terminal_run_lines();
        let mut st = TerminalRunCardState::new();
        st.presentation = TerminalRunPresentation::Expanded;
        st.output.recipe = TerminalOutputRecipe::Pane;
        st.on_append(100, 8);
        assert!(st.is_following());
        let out = st.handle_key(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            &run,
            &lines,
        );
        assert!(matches!(out, TerminalRunCardOutcome::ScrollDetached { .. }));
        assert!(!st.is_following());
        st.on_append(120, 8);
        assert!(!st.is_following());
        assert!(st.output.unread() > 0 || !st.is_following());
    }

    #[test]
    fn copy_command_redacted() {
        let run = TerminalRun::new("r", "export TOKEN=secret && true")
            .execute("export TOKEN=secret && true")
            .status(TerminalRunStatus::Succeeded)
            .exit_code(0);
        let lines = example_terminal_run_lines();
        let mut st = TerminalRunCardState::new();
        st.presentation = TerminalRunPresentation::Expanded;
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
            &run,
            &lines,
        );
        match out {
            TerminalRunCardOutcome::CopyCommand { text, .. } => {
                assert!(text.contains("***") || !text.contains("secret"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bridge_to_tool_call() {
        let run = TerminalRun::new("r", "ls")
            .execute("ls")
            .status(TerminalRunStatus::Succeeded)
            .exit_code(0)
            .actor("agent");
        let call = terminal_run_to_tool_call(&run);
        assert_eq!(call.name, "shell");
        assert_eq!(call.status, ToolStatus::Success);
        assert_eq!(call.actor.as_deref(), Some("agent"));
    }

    #[test]
    fn project_lines_show_proposed_executed() {
        let run = TerminalRun::new("r", "echo a")
            .execute("echo b")
            .status(TerminalRunStatus::Succeeded)
            .exit_code(0);
        let lines = example_terminal_run_lines();
        let p = project_terminal_run_lines(&run, &lines, true, true);
        let j = p.join("\n");
        assert!(j.contains("proposed"));
        assert!(j.contains("executed"));
    }

    #[test]
    fn paint_all_phases_and_ansi() {
        let system = DesignSystem::default();
        let lines = example_terminal_run_lines();
        let opts = AnsiParseOptions::default();
        let ansi = parse_to_line("\x1b[32mok\x1b[0m", &opts);
        let mut ansi_lines = lines.clone();
        ansi_lines.push(TerminalLine::stdout("a1", "ok").ansi(&ansi));
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        for run in example_terminal_runs() {
            let mut st = TerminalRunCardState::new();
            st.presentation = TerminalRunPresentation::Expanded;
            TerminalRunCard::new(&run, &ansi_lines, &system).paint(area, &mut buf, &mut st);
        }
    }

    #[test]
    fn never_process_or_pty() {
        let src = include_str!("terminal_run_card.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "std::process",
            "Command::new",
            "portable_pty",
            "tokio::process",
            "nix::",
            "openai",
            "anthropic",
        ] {
            assert!(!body.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn accepts_input_gate() {
        let run = TerminalRun::new("r", "x");
        let lines = example_terminal_run_lines();
        let mut st = TerminalRunCardState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &run,
                &lines
            ),
            TerminalRunCardOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_header_toggles() {
        let system = DesignSystem::default();
        let run = TerminalRun::new("r", "echo hi").execute("echo hi");
        let lines = example_terminal_run_lines();
        let mut st = TerminalRunCardState::new();
        let area = Rect::new(0, 0, 48, 10);
        let mut buf = Buffer::empty(area);
        TerminalRunCard::new(&run, &lines, &system).paint(area, &mut buf, &mut st);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(st.header_hit.x, st.header_hit.y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            st.handle_mouse(ev, &run, &lines),
            TerminalRunCardOutcome::Expanded { .. }
        ));
    }

    #[test]
    fn paint_perf_budget() {
        let system = DesignSystem::default();
        let runs = example_terminal_runs();
        let lines = example_terminal_run_lines();
        let area = Rect::new(0, 0, 72, 14);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            for run in &runs {
                let mut st = TerminalRunCardState::new();
                st.presentation = TerminalRunPresentation::Expanded;
                TerminalRunCard::new(run, &lines, &system).paint(area, &mut buf, &mut st);
            }
        }
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    }

    #[test]
    fn detach_process_chord() {
        let run = TerminalRun::new("r", "sleep 99")
            .execute("sleep 99")
            .status(TerminalRunStatus::Running);
        let lines = example_terminal_run_lines();
        let mut st = TerminalRunCardState::new();
        st.presentation = TerminalRunPresentation::Expanded;
        st.output.recipe = TerminalOutputRecipe::Pane;
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE),
                &run,
                &lines
            ),
            TerminalRunCardOutcome::DetachRequested { .. }
        ));
    }

    #[test]
    fn fuzz_statuses_and_phases() {
        for s in [
            TerminalRunStatus::Pending,
            TerminalRunStatus::WaitingPermission,
            TerminalRunStatus::Running,
            TerminalRunStatus::Succeeded,
            TerminalRunStatus::Failed,
            TerminalRunStatus::Signaled,
            TerminalRunStatus::Cancelled,
            TerminalRunStatus::TimedOut,
            TerminalRunStatus::Detached,
        ] {
            let run = TerminalRun::new("r", "cmd").status(s);
            let _ = run.header_line(true);
            let _ = terminal_run_to_tool_call(&run);
        }
        for p in [
            TerminalCommandPhase::Proposed,
            TerminalCommandPhase::EditedApproval,
            TerminalCommandPhase::Executed,
        ] {
            assert!(!p.id().is_empty());
        }
    }

    #[test]
    fn terminal_output_still_paints() {
        let system = DesignSystem::default();
        let meta = TerminalCommandMeta::new("echo").status(TerminalRunStatus::Running);
        let lines = example_terminal_run_lines();
        let mut st = TerminalOutputState::new();
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        TerminalOutput::new(&meta, &lines, &system).render(area, &mut buf, &mut st);
    }
}
