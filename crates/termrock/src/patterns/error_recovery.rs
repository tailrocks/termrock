// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ErrorRecovery** / **CrashReport** — graceful recovery for serious failures
//! from **public** TermRock widgets (ErrorState, diagnostics list, StatusBar,
//! doctor/capability projection).
//!
//! **Mission.** Human summary, preserved-work cue, recovery options (restart /
//! restore session, copy diagnostics, logs, environment/capabilities, report
//! issue, safe quit), and redacted crash-report projection. Full recovery
//! surface and **inline fallback** when full-screen paint is compromised.
//! **Host owns** panic hooks, process restart, session persistence, log tails,
//! issue trackers, and terminal restore — outcomes/requests only.
//!
//! **vs standalone [`ErrorState`].** Elevated composition with crash-report
//! redaction, multi-option action list, and inline fallback mode — not a second
//! paint fork of ErrorState.
//!
//! Research: crash reporters, terminal panic hooks, session restoration,
//! resilient CLI design.
//!
//! Teaches: how to compose a graceful recovery surface for serious failures:
//! what broke, what was preserved, and what to do next.
//!
//! Composes: [`crate::widgets::ErrorKind`], [`crate::widgets::ErrorRecipe`],
//! [`crate::widgets::ErrorState`], [`crate::widgets::ErrorStateOutcome`],
//! [`crate::widgets::ErrorStateState`], [`crate::widgets::HistoryRedaction`],
//! [`crate::widgets::List`], [`crate::widgets::ListRow`], and 9 more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{
    capability::DoctorReport,
    input::{KeyCode, KeyEvent, KeyModifiers},
    interaction::Outcome,
    layout::{
        PaneConstraint, PaneGeom, PaneId, Workspace, WorkspaceAxis, WorkspaceNode, WorkspaceState,
    },
    style::{DesignSystem, PanelChrome, Role},
    widgets::{
        ErrorKind, ErrorRecipe, ErrorState, ErrorStateOutcome, ErrorStateState, List, ListRow,
        ListState, Panel, Recovery, RecoveryAction, RetrySafety, StatusBar, StatusBarState,
        StatusSlot, history_redaction_secret, redact_history_text,
    },
};

// ── Panes, mode, density ────────────────────────────────────────────────────

/// Named panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorRecoveryPane {
    /// Primary ErrorState summary / recovery chrome.
    Summary,
    /// Preserved work note.
    Preserved,
    /// Action list (restart, restore, copy, logs, env, report, quit).
    Actions,
    /// Diagnostics / crash report detail (redacted).
    Diagnostics,
    /// Status strip.
    Status,
}

impl ErrorRecoveryPane {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Preserved => "preserved",
            Self::Actions => "actions",
            Self::Diagnostics => "diagnostics",
            Self::Status => "status",
        }
    }

    /// Tab focus order (status chrome-only).
    #[must_use]
    pub fn focus_order() -> &'static [ErrorRecoveryPane] {
        &[
            Self::Summary,
            Self::Actions,
            Self::Diagnostics,
            Self::Preserved,
        ]
    }
}

/// Presentation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ErrorRecoveryMode {
    /// Full multi-pane recovery / crash report.
    #[default]
    Full,
    /// Inline fallback when full-screen rendering is compromised.
    InlineFallback,
}

impl ErrorRecoveryMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::InlineFallback => "inline-fallback",
        }
    }
}

/// Density for full mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ErrorRecoveryDensity {
    /// Summary + actions + diagnostics.
    #[default]
    Normal,
    /// Collapse diagnostics detail first.
    Narrow,
    /// Summary + actions only.
    Tiny,
}

impl ErrorRecoveryDensity {
    /// From width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < 48 {
            Self::Tiny
        } else if width < 80 {
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

// ── Domain ──────────────────────────────────────────────────────────────────

/// Host-projected failure class (maps to ErrorKind for paint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum FailureClass {
    /// Unexpected panic / crash.
    #[default]
    Crash,
    /// Terminal restore failed after panic/exit.
    TerminalRestoreFailed,
    /// Partial init (capabilities / backend incomplete).
    PartialInit,
    /// Network / transport.
    Network,
    /// Generic recoverable.
    Generic,
}

impl FailureClass {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Crash => "crash",
            Self::TerminalRestoreFailed => "terminal-restore-failed",
            Self::PartialInit => "partial-init",
            Self::Network => "network",
            Self::Generic => "generic",
        }
    }

    /// Map to ErrorKind for ErrorState paint.
    #[must_use]
    pub const fn error_kind(self) -> ErrorKind {
        match self {
            Self::Crash | Self::TerminalRestoreFailed => ErrorKind::Crash,
            Self::PartialInit => ErrorKind::UnsupportedCapability,
            Self::Network => ErrorKind::Network,
            Self::Generic => ErrorKind::Generic,
        }
    }
}

/// Recovery action id (host maps to effects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RecoveryActionId {
    /// Restart view / process (host).
    Restart,
    /// Restore last session snapshot.
    RestoreSession,
    /// Copy redacted diagnostics.
    CopyDiagnostics,
    /// Open logs (host tailer).
    OpenLogs,
    /// Show environment / capabilities (doctor).
    ShowCapabilities,
    /// Report issue (host tracker).
    ReportIssue,
    /// Safe quit (restore terminal then exit).
    SafeQuit,
}

impl RecoveryActionId {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Restart => "restart",
            Self::RestoreSession => "restore-session",
            Self::CopyDiagnostics => "copy-diagnostics",
            Self::OpenLogs => "open-logs",
            Self::ShowCapabilities => "show-capabilities",
            Self::ReportIssue => "report-issue",
            Self::SafeQuit => "safe-quit",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Restart => "Restart view",
            Self::RestoreSession => "Restore session",
            Self::CopyDiagnostics => "Copy diagnostics",
            Self::OpenLogs => "View logs",
            Self::ShowCapabilities => "Environment / capabilities",
            Self::ReportIssue => "Report issue",
            Self::SafeQuit => "Safe quit",
        }
    }

    /// Chord hint.
    #[must_use]
    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::Restart => "r",
            Self::RestoreSession => "s",
            Self::CopyDiagnostics => "c",
            Self::OpenLogs => "l",
            Self::ShowCapabilities => "e",
            Self::ReportIssue => "i",
            Self::SafeQuit => "q",
        }
    }

    /// Default full-mode action set.
    #[must_use]
    pub fn default_set() -> &'static [RecoveryActionId] {
        &[
            Self::Restart,
            Self::RestoreSession,
            Self::CopyDiagnostics,
            Self::OpenLogs,
            Self::ShowCapabilities,
            Self::ReportIssue,
            Self::SafeQuit,
        ]
    }

    /// Inline fallback: minimal keyboard-reachable set.
    #[must_use]
    pub fn inline_set() -> &'static [RecoveryActionId] {
        &[
            Self::Restart,
            Self::RestoreSession,
            Self::CopyDiagnostics,
            Self::SafeQuit,
        ]
    }
}

/// Host-projected crash/diagnostic snapshot (may contain secrets — redact before paint/copy).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrashReportSnapshot {
    /// Human summary.
    pub summary: String,
    /// Technical / panic message.
    pub technical: String,
    /// Source component / crate.
    pub source: String,
    /// Preserved work note.
    pub preserved_note: String,
    /// Whether work was preserved.
    pub work_preserved: bool,
    /// Raw env lines (may include secrets).
    pub env_lines: Vec<String>,
    /// Raw log snippet lines.
    pub log_lines: Vec<String>,
    /// Optional capability/doctor sample text.
    pub capabilities_text: String,
    /// Failure class.
    pub class: FailureClass,
}

impl CrashReportSnapshot {
    /// Minimal crash snapshot.
    #[must_use]
    pub fn crash(summary: impl Into<String>, technical: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            technical: technical.into(),
            source: "termrock".into(),
            preserved_note: "Session draft retained".into(),
            work_preserved: true,
            env_lines: Vec::new(),
            log_lines: Vec::new(),
            capabilities_text: String::new(),
            class: FailureClass::Crash,
        }
    }
}

// ── Secret redaction (pure; used by copy + diagnostics paint) ───────────────

/// Redact common secret patterns from crash-report text.
///
/// Covers Authorization headers, bearer tokens, API keys, password assignments,
/// and long opaque tokens. Non-secret structure is retained. Uses
/// [`redact_history_text`] / middle-mask for residual token-shaped values.
#[must_use]
pub fn redact_crash_report_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        out.push_str(&redact_crash_report_line(line));
        out.push('\n');
    }
    // preserve trailing newline only if input had one
    if !raw.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    out
}

fn redact_crash_report_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();

    // 1) Assignment keys only (word-boundary before key, optional WS then =/:).
    //    Longer / more specific keys first so `api_key` wins before bare `secret`
    //    and value substrings like `sk-secret-…` never match as keys.
    const ASSIGN_KEYS: &[&str] = &[
        "authorization",
        "client_secret",
        "access_token",
        "refresh_token",
        "private_key",
        "auth_token",
        "api_key",
        "apikey",
        "password",
        "passwd",
        "secret",
        "bearer",
    ];
    for key in ASSIGN_KEYS {
        if let Some(pos) = find_assignment_key(&lower, key) {
            return redact_assignment_at(line, pos, key.len());
        }
    }

    // 2) Token-shaped prefixes anywhere (sk-, ghp_, …)
    let mut s = line.to_string();
    s = mask_token_like(&s, "sk-");
    s = mask_token_like(&s, "ghp_");
    s = mask_token_like(&s, "github_pat_");
    s = mask_token_like(&s, "xoxb-");
    s = mask_token_like(&s, "AKIA");

    // 3) Long opaque blobs (no spaces, mostly token alphabet)
    if looks_like_opaque_secret(&s) {
        return redact_history_text(&s, history_redaction_secret());
    }
    s
}

/// True if `c` is not part of an env/header key identifier.
fn is_key_boundary(c: char) -> bool {
    !c.is_ascii_alphanumeric() && c != '_'
}

/// Find `key` only when it is an assignment key: boundary before, then optional
/// whitespace and `=` or `:`. Does **not** match key text inside values.
fn find_assignment_key(lower_line: &str, key: &str) -> Option<usize> {
    let mut search = 0;
    while search < lower_line.len() {
        let Some(rel) = lower_line[search..].find(key) else {
            return None;
        };
        let pos = search + rel;
        let before_ok = pos == 0
            || lower_line[..pos]
                .chars()
                .next_back()
                .is_some_and(is_key_boundary);
        let after = pos + key.len();
        let rest = lower_line.get(after..).unwrap_or("");
        // no more identifier chars glued to key (e.g. "secret" in "secrets")
        let after_char = rest.chars().next();
        let glued = after_char.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if before_ok && !glued {
            let trimmed = rest.trim_start_matches(|c: char| c == ' ' || c == '\t');
            if trimmed.starts_with('=') || trimmed.starts_with(':') {
                return Some(pos);
            }
        }
        search = pos.saturating_add(1);
        if search <= pos {
            search = pos + 1;
        }
    }
    None
}

/// Redact value after assignment at `key_pos` (byte index of key in `line`).
fn redact_assignment_at(line: &str, key_pos: usize, key_len: usize) -> String {
    let after_key = key_pos + key_len;
    let rest = line.get(after_key..).unwrap_or("");
    let ws_len = rest.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    let after_ws = after_key
        + rest
            .chars()
            .take(ws_len)
            .map(|c| c.len_utf8())
            .sum::<usize>();
    let sep_and_value = line.get(after_ws..).unwrap_or("");
    let sep_len = sep_and_value
        .chars()
        .next()
        .map(|c| c.len_utf8())
        .unwrap_or(0);
    let value_start = after_ws + sep_len;
    let prefix = &line[..value_start.min(line.len())];
    let value = line.get(value_start..).unwrap_or("").trim_start();
    // Full mask for assignment values — no residual token prefixes (sk-…).
    let masked = if value.is_empty() {
        String::new()
    } else {
        redact_history_text(value, crate::widgets::HistoryRedaction::MaskAll)
    };
    format!("{prefix}{masked}")
}

fn mask_token_like(s: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(prefix) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail
            .char_indices()
            .skip(prefix.chars().count())
            .find(|(_, c)| c.is_whitespace() || *c == '"' || *c == '\'' || *c == ',')
            .map(|(idx, _)| idx)
            .unwrap_or(tail.len());
        let token = &tail[..end];
        out.push_str(&redact_history_text(token, history_redaction_secret()));
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn looks_like_opaque_secret(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 24 {
        return false;
    }
    // no spaces and mostly alnum/+/=
    if t.contains(' ') {
        return false;
    }
    let ok = t
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
        .count();
    ok * 10 >= t.chars().count() * 9
}

/// Build redacted diagnostic report text for copy / diagnostics pane.
///
/// **This is the shipped redaction path** used by CopyDiagnostics outcomes.
#[must_use]
pub fn build_redacted_crash_report(snap: &CrashReportSnapshot) -> String {
    let mut raw = String::new();
    raw.push_str("=== termrock crash report ===\n");
    raw.push_str(&format!("class: {}\n", snap.class.id()));
    raw.push_str(&format!("summary: {}\n", snap.summary));
    raw.push_str(&format!("source: {}\n", snap.source));
    raw.push_str(&format!("technical: {}\n", snap.technical));
    raw.push_str(&format!(
        "work_preserved: {} ({})\n",
        snap.work_preserved, snap.preserved_note
    ));
    if !snap.env_lines.is_empty() {
        raw.push_str("\n--- environment ---\n");
        for e in &snap.env_lines {
            raw.push_str(e);
            raw.push('\n');
        }
    }
    if !snap.log_lines.is_empty() {
        raw.push_str("\n--- logs ---\n");
        for l in &snap.log_lines {
            raw.push_str(l);
            raw.push('\n');
        }
    }
    if !snap.capabilities_text.is_empty() {
        raw.push_str("\n--- capabilities ---\n");
        raw.push_str(&snap.capabilities_text);
        raw.push('\n');
    }
    redact_crash_report_text(&raw)
}

/// Action list rows for recovery options.
#[must_use]
pub fn recovery_action_rows(actions: &[RecoveryActionId]) -> Vec<ListRow<'static, String>> {
    actions
        .iter()
        .map(|a| {
            let label = format!("[{}] {}", a.shortcut(), a.label());
            ListRow::item(a.id().to_string(), Line::from(label))
        })
        .collect()
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Recovery outcomes — host owns effects.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorRecoveryOutcome {
    /// Ignored.
    Ignored,
    /// Focus changed.
    FocusChanged(&'static str),
    /// Mode changed.
    ModeChanged(ErrorRecoveryMode),
    /// Restart view / process.
    RestartRequested,
    /// Restore session snapshot.
    RestoreSessionRequested,
    /// Copy **redacted** diagnostics text (payload attached).
    CopyDiagnostics {
        /// Redacted report body.
        text: String,
    },
    /// Open logs.
    OpenLogs,
    /// Show capabilities / doctor.
    ShowCapabilities,
    /// Report issue (host tracker; text is redacted).
    ReportIssue {
        /// Redacted report body.
        text: String,
    },
    /// Safe quit.
    SafeQuit,
    /// ErrorState child residual.
    ErrorChild {
        /// Kind.
        kind: String,
    },
    /// Esc / cancel.
    Cancelled,
}

// ── Surfaces ────────────────────────────────────────────────────────────────

/// Borrowed surfaces for one paint frame.
pub struct ErrorRecoverySurfaces<'a> {
    /// Design system.
    pub system: &'a DesignSystem,
    /// State.
    pub state: &'a mut ErrorRecoveryState,
    /// Host crash snapshot (secrets redacted on copy/report path).
    pub snapshot: &'a CrashReportSnapshot,
    /// Optional doctor report for capabilities pane cue.
    pub doctor: Option<&'a DoctorReport>,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Persistent recovery surface state.
#[derive(Debug)]
pub struct ErrorRecoveryState {
    /// Workspace.
    pub workspace: WorkspaceState,
    /// Embedded ErrorState interaction.
    pub error: ErrorStateState,
    /// Action list.
    pub actions: ListState<String>,
    /// Status.
    pub status: StatusBarState<&'static str>,
    /// Mode.
    pub mode: ErrorRecoveryMode,
    /// Density override.
    pub density: Option<ErrorRecoveryDensity>,
    /// Focus pane.
    pub focus: &'static str,
    /// Host: terminal restore failed flag (chrome).
    pub terminal_restore_failed: bool,
    /// Host: partial init flag.
    pub partial_init: bool,
    /// Cached redacted report (invalidated when host rebuilds snapshot).
    redacted_cache: Option<String>,
    /// Last panes.
    last_panes: Vec<PaneGeom>,
    last_area_width: Option<u16>,
}

impl Default for ErrorRecoveryState {
    fn default() -> Self {
        Self::new()
    }
}

impl ErrorRecoveryState {
    /// Full recovery factory.
    #[must_use]
    pub fn new() -> Self {
        let mut error = ErrorStateState::new();
        error.focus_retry();
        Self {
            workspace: WorkspaceState::new(),
            error,
            actions: ListState::new(Some(RecoveryActionId::Restart.id().to_string())),
            status: StatusBarState::new(),
            mode: ErrorRecoveryMode::Full,
            density: None,
            focus: ErrorRecoveryPane::Summary.id(),
            terminal_restore_failed: false,
            partial_init: false,
            redacted_cache: None,
            last_panes: Vec::new(),
            last_area_width: None,
        }
    }
    /// Last panes.
    #[must_use]
    pub fn last_panes(&self) -> &[PaneGeom] {
        &self.last_panes
    }

    /// Effective density.
    #[must_use]
    pub fn effective_density(&self) -> ErrorRecoveryDensity {
        self.density
            .unwrap_or_else(|| ErrorRecoveryDensity::for_width(self.last_area_width.unwrap_or(100)))
    }

    /// Active action set for mode.
    #[must_use]
    pub fn action_set(&self) -> &'static [RecoveryActionId] {
        match self.mode {
            ErrorRecoveryMode::Full => RecoveryActionId::default_set(),
            ErrorRecoveryMode::InlineFallback => RecoveryActionId::inline_set(),
        }
    }

    /// Visible focus panes.
    #[must_use]
    pub fn visible_focus_panes(&self, density: ErrorRecoveryDensity) -> Vec<ErrorRecoveryPane> {
        match self.mode {
            ErrorRecoveryMode::InlineFallback => {
                vec![ErrorRecoveryPane::Summary, ErrorRecoveryPane::Actions]
            }
            ErrorRecoveryMode::Full => match density {
                ErrorRecoveryDensity::Tiny => {
                    vec![ErrorRecoveryPane::Summary, ErrorRecoveryPane::Actions]
                }
                ErrorRecoveryDensity::Narrow => vec![
                    ErrorRecoveryPane::Summary,
                    ErrorRecoveryPane::Actions,
                    ErrorRecoveryPane::Preserved,
                ],
                ErrorRecoveryDensity::Normal => vec![
                    ErrorRecoveryPane::Summary,
                    ErrorRecoveryPane::Actions,
                    ErrorRecoveryPane::Diagnostics,
                    ErrorRecoveryPane::Preserved,
                ],
            },
        }
    }

    /// Clamp focus.
    pub fn clamp_focus_to_density(&mut self, density: ErrorRecoveryDensity) {
        let visible = self.visible_focus_panes(density);
        if !visible.iter().any(|p| p.id() == self.focus) {
            self.focus = visible
                .first()
                .map(|p| p.id())
                .unwrap_or(ErrorRecoveryPane::Summary.id());
        }
    }

    /// Invalidate redacted cache (call when host updates snapshot).
    pub fn invalidate_report_cache(&mut self) {
        self.redacted_cache = None;
    }

    /// Redacted report for this snapshot (cached).
    #[must_use]
    pub fn redacted_report(&mut self, snap: &CrashReportSnapshot) -> &str {
        if self.redacted_cache.is_none() {
            self.redacted_cache = Some(build_redacted_crash_report(snap));
        }
        self.redacted_cache.as_deref().unwrap_or("")
    }

    /// Cycle Tab.
    pub fn cycle_focus(&mut self, reverse: bool) -> ErrorRecoveryOutcome {
        let density = self.effective_density();
        let visible = self.visible_focus_panes(density);
        if visible.is_empty() {
            return ErrorRecoveryOutcome::Ignored;
        }
        let cur = visible
            .iter()
            .position(|p| p.id() == self.focus)
            .unwrap_or(0);
        let next = if reverse {
            if cur == 0 { visible.len() - 1 } else { cur - 1 }
        } else {
            (cur + 1) % visible.len()
        };
        self.focus = visible[next].id();
        ErrorRecoveryOutcome::FocusChanged(self.focus)
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: ErrorRecoveryMode) -> ErrorRecoveryOutcome {
        if self.mode == mode {
            return ErrorRecoveryOutcome::Ignored;
        }
        self.mode = mode;
        let density = self.effective_density();
        self.clamp_focus_to_density(density);
        // Reset action selection to first available
        if let Some(a) = self.action_set().first() {
            self.actions = ListState::new(Some(a.id().into()));
        }
        ErrorRecoveryOutcome::ModeChanged(mode)
    }

    /// Status slots.
    #[must_use]
    pub fn status_slots(&self) -> Vec<StatusSlot<'static, &'static str>> {
        let mut slots = vec![
            StatusSlot::new("failure", "recovery required")
                .semantic(crate::widgets::SemanticStatus::Failed)
                .priority(100),
            StatusSlot::context("mode", self.mode.id()).priority(50),
            StatusSlot::focus_zone("focus", self.focus).priority(70),
            StatusSlot::shortcut("keys", "r restart · s restore · l logs · i report · q quit")
                .priority(10),
        ];
        if self.terminal_restore_failed {
            slots.push(
                StatusSlot::new("tty", "tty restore failed")
                    .semantic(crate::widgets::SemanticStatus::Failed)
                    .priority(95),
            );
        }
        if self.partial_init {
            slots.push(
                StatusSlot::new("init", "partial init")
                    .semantic(crate::widgets::SemanticStatus::Warning)
                    .priority(90),
            );
        }
        slots
    }

    /// Map action id to outcome (uses redacted report for copy/report).
    pub fn outcome_for_action(
        &mut self,
        id: RecoveryActionId,
        snap: &CrashReportSnapshot,
    ) -> ErrorRecoveryOutcome {
        match id {
            RecoveryActionId::Restart => ErrorRecoveryOutcome::RestartRequested,
            RecoveryActionId::RestoreSession => ErrorRecoveryOutcome::RestoreSessionRequested,
            RecoveryActionId::CopyDiagnostics => {
                let text = self.redacted_report(snap).to_string();
                ErrorRecoveryOutcome::CopyDiagnostics { text }
            }
            RecoveryActionId::OpenLogs => ErrorRecoveryOutcome::OpenLogs,
            RecoveryActionId::ShowCapabilities => ErrorRecoveryOutcome::ShowCapabilities,
            RecoveryActionId::ReportIssue => {
                let text = self.redacted_report(snap).to_string();
                ErrorRecoveryOutcome::ReportIssue { text }
            }
            RecoveryActionId::SafeQuit => ErrorRecoveryOutcome::SafeQuit,
        }
    }

    /// Keys — real path.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        snap: &CrashReportSnapshot,
    ) -> ErrorRecoveryOutcome {
        if key.is_release() {
            return ErrorRecoveryOutcome::Ignored;
        }
        let is_press = key.is_press();

        if is_press {
            match key.code {
                KeyCode::Tab if key.modifiers.is_empty() => {
                    return self.cycle_focus(false);
                }
                KeyCode::BackTab | KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.cycle_focus(true);
                }
                KeyCode::Esc => {
                    return ErrorRecoveryOutcome::Cancelled;
                }
                // Global recovery chords (always available — no hover-only)
                KeyCode::Char('r') if key.modifiers.is_empty() && self.focus != "summary" => {
                    // When on summary, ErrorState may own 'r' for retry — map to Restart
                    return self.outcome_for_action(RecoveryActionId::Restart, snap);
                }
                KeyCode::Char('s') if key.modifiers.is_empty() => {
                    return self.outcome_for_action(RecoveryActionId::RestoreSession, snap);
                }
                KeyCode::Char('c') if key.modifiers.is_empty() => {
                    return self.outcome_for_action(RecoveryActionId::CopyDiagnostics, snap);
                }
                KeyCode::Char('l') if key.modifiers.is_empty() => {
                    return self.outcome_for_action(RecoveryActionId::OpenLogs, snap);
                }
                KeyCode::Char('e') if key.modifiers.is_empty() => {
                    return self.outcome_for_action(RecoveryActionId::ShowCapabilities, snap);
                }
                KeyCode::Char('i') if key.modifiers.is_empty() => {
                    return self.outcome_for_action(RecoveryActionId::ReportIssue, snap);
                }
                KeyCode::Char('q') if key.modifiers.is_empty() => {
                    return self.outcome_for_action(RecoveryActionId::SafeQuit, snap);
                }
                KeyCode::Char('m') if key.modifiers.is_empty() => {
                    // Toggle inline / full
                    let next = match self.mode {
                        ErrorRecoveryMode::Full => ErrorRecoveryMode::InlineFallback,
                        ErrorRecoveryMode::InlineFallback => ErrorRecoveryMode::Full,
                    };
                    return self.set_mode(next);
                }
                _ => {}
            }
        }

        match self.focus {
            "summary" => self.handle_summary_key(key, snap),
            "actions" => self.handle_actions_key(key, snap),
            "diagnostics" | "preserved" => {
                // arrows noop; chords handled globally
                ErrorRecoveryOutcome::Ignored
            }
            _ => ErrorRecoveryOutcome::Ignored,
        }
    }

    fn handle_summary_key(
        &mut self,
        key: KeyEvent,
        snap: &CrashReportSnapshot,
    ) -> ErrorRecoveryOutcome {
        // Build ErrorState view for key routing
        let system = DesignSystem::default();
        let view = build_error_state_view(snap, self, &system);
        let out = view.handle_key(key, &mut self.error);
        match out {
            ErrorStateOutcome::Ignored => {
                // r on summary with retry focus → restart
                if key.is_press() && matches!(key.code, KeyCode::Char('r')) {
                    return self.outcome_for_action(RecoveryActionId::Restart, snap);
                }
                ErrorRecoveryOutcome::Ignored
            }
            ErrorStateOutcome::Retry => self.outcome_for_action(RecoveryActionId::Restart, snap),
            ErrorStateOutcome::CopyDiagnostics => {
                self.outcome_for_action(RecoveryActionId::CopyDiagnostics, snap)
            }
            ErrorStateOutcome::ReportIssue => {
                self.outcome_for_action(RecoveryActionId::ReportIssue, snap)
            }
            ErrorStateOutcome::Alternative => {
                self.outcome_for_action(RecoveryActionId::RestoreSession, snap)
            }
            ErrorStateOutcome::ToggleDetails => ErrorRecoveryOutcome::ErrorChild {
                kind: "ToggleDetails".into(),
            },
        }
    }

    fn handle_actions_key(
        &mut self,
        key: KeyEvent,
        snap: &CrashReportSnapshot,
    ) -> ErrorRecoveryOutcome {
        let set = self.action_set();
        let rows = recovery_action_rows(set);
        if key.is_press() && key.code == KeyCode::Enter {
            if let Some(id) = self.actions.selected().cloned() {
                if let Some(action) = set.iter().find(|a| a.id() == id) {
                    return self.outcome_for_action(*action, snap);
                }
            }
            if let Some(action) = set.first() {
                return self.outcome_for_action(*action, snap);
            }
        }
        let out = self.actions.handle_key(&rows, key);
        match out {
            Outcome::Ignored => ErrorRecoveryOutcome::Ignored,
            Outcome::Activated(id) => {
                if let Some(action) = set.iter().find(|a| a.id() == id) {
                    self.outcome_for_action(*action, snap)
                } else {
                    ErrorRecoveryOutcome::Ignored
                }
            }
            Outcome::Changed | Outcome::CheckToggled(_) => ErrorRecoveryOutcome::Ignored,
            Outcome::Cancelled => ErrorRecoveryOutcome::Cancelled,
        }
    }
}

fn build_error_state_view<'a>(
    snap: &'a CrashReportSnapshot,
    state: &ErrorRecoveryState,
    system: &'a DesignSystem,
) -> ErrorState<'a> {
    let recipe = match state.mode {
        ErrorRecoveryMode::InlineFallback => ErrorRecipe::Inline,
        ErrorRecoveryMode::Full => ErrorRecipe::FullScreen,
    };

    let mut summary = snap.summary.as_str();
    if summary.is_empty() {
        summary = snap.class.error_kind().default_summary();
    }

    // Technical shown is already host-owned; redaction applied for diagnostics pane / copy
    ErrorState::new(summary, system)
        .kind(snap.class.error_kind())
        .explanation(if state.terminal_restore_failed {
            "Terminal restore may have failed. Prefer Safe quit after copying diagnostics."
        } else if state.partial_init {
            "Initialization was incomplete. Capabilities may be limited."
        } else {
            "Something went wrong. Your work was preserved when possible."
        })
        .technical(snap.technical.as_str())
        .source(snap.source.as_str())
        .recipe(recipe)
        .recovery(
            Recovery::none()
                .with_retry(RecoveryAction::with_shortcut("Restart view", "r"))
                .with_alternative(RecoveryAction::with_shortcut("Restore session", "s"))
                .with_copy_diagnostics(true)
                .with_report_issue(RecoveryAction::with_shortcut("Report issue", "i"))
                .with_retry_safety(RetrySafety::Unknown)
                .with_work_preserved(
                    snap.work_preserved,
                    if snap.preserved_note.is_empty() {
                        None
                    } else {
                        Some(snap.preserved_note.as_str())
                    },
                ),
        )
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Width-derived layout.
#[must_use]
pub fn error_recovery_layout(area: Rect, state: &WorkspaceState) -> Vec<PaneGeom> {
    error_recovery_layout_density(
        area,
        state,
        ErrorRecoveryDensity::for_width(area.width),
        ErrorRecoveryMode::Full,
    )
}

/// Explicit density + mode.
#[must_use]
pub fn error_recovery_layout_density(
    area: Rect,
    state: &WorkspaceState,
    density: ErrorRecoveryDensity,
    mode: ErrorRecoveryMode,
) -> Vec<PaneGeom> {
    let root = match mode {
        ErrorRecoveryMode::InlineFallback => WorkspaceNode::Split {
            axis: WorkspaceAxis::Vertical,
            ratio_percent: 55,
            first: Box::new(WorkspaceNode::Leaf {
                id: PaneId::from_static(ErrorRecoveryPane::Summary.id()),
                constraint: PaneConstraint::Min(2),
                collapse_priority: 1,
            }),
            second: Box::new(WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 85,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ErrorRecoveryPane::Actions.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
                second: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ErrorRecoveryPane::Status.id()),
                    constraint: PaneConstraint::Fixed(1),
                    collapse_priority: 3,
                }),
            }),
        },
        ErrorRecoveryMode::Full => match density {
            ErrorRecoveryDensity::Tiny => WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 50,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ErrorRecoveryPane::Summary.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 85,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ErrorRecoveryPane::Actions.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ErrorRecoveryPane::Status.id()),
                        constraint: PaneConstraint::Fixed(1),
                        collapse_priority: 3,
                    }),
                }),
            },
            ErrorRecoveryDensity::Narrow => WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 40,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ErrorRecoveryPane::Summary.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Vertical,
                    ratio_percent: 55,
                    first: Box::new(WorkspaceNode::Leaf {
                        id: PaneId::from_static(ErrorRecoveryPane::Actions.id()),
                        constraint: PaneConstraint::Weight(1),
                        collapse_priority: 1,
                    }),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 70,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ErrorRecoveryPane::Preserved.id()),
                            constraint: PaneConstraint::Min(1),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ErrorRecoveryPane::Status.id()),
                            constraint: PaneConstraint::Fixed(1),
                            collapse_priority: 3,
                        }),
                    }),
                }),
            },
            ErrorRecoveryDensity::Normal => WorkspaceNode::Split {
                axis: WorkspaceAxis::Vertical,
                ratio_percent: 35,
                first: Box::new(WorkspaceNode::Leaf {
                    id: PaneId::from_static(ErrorRecoveryPane::Summary.id()),
                    constraint: PaneConstraint::Weight(1),
                    collapse_priority: 1,
                }),
                second: Box::new(WorkspaceNode::Split {
                    axis: WorkspaceAxis::Horizontal,
                    ratio_percent: 40,
                    first: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 70,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ErrorRecoveryPane::Actions.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 1,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ErrorRecoveryPane::Preserved.id()),
                            constraint: PaneConstraint::Min(2),
                            collapse_priority: 0,
                        }),
                    }),
                    second: Box::new(WorkspaceNode::Split {
                        axis: WorkspaceAxis::Vertical,
                        ratio_percent: 90,
                        first: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ErrorRecoveryPane::Diagnostics.id()),
                            constraint: PaneConstraint::Weight(1),
                            collapse_priority: 0,
                        }),
                        second: Box::new(WorkspaceNode::Leaf {
                            id: PaneId::from_static(ErrorRecoveryPane::Status.id()),
                            constraint: PaneConstraint::Fixed(1),
                            collapse_priority: 3,
                        }),
                    }),
                }),
            },
        },
    };
    Workspace::new(root).layout(area, state)
}

fn pane_area(panes: &[PaneGeom], id: &str) -> Option<Rect> {
    panes.iter().find_map(|p| {
        if p.id.0.as_str() == id && !p.collapsed && p.area.width > 0 && p.area.height > 0 {
            Some(p.area)
        } else {
            None
        }
    })
}

// ── Render ──────────────────────────────────────────────────────────────────

/// Paint error recovery / crash report surface.
pub fn paint_error_recovery(buffer: &mut Buffer, area: Rect, surfaces: ErrorRecoverySurfaces<'_>) {
    let ErrorRecoverySurfaces {
        system,
        state,
        snapshot,
        doctor,
    } = surfaces;

    if area.is_empty() {
        return;
    }

    state.last_area_width = Some(area.width);
    let density = state.effective_density();
    let panes = error_recovery_layout_density(area, &state.workspace, density, state.mode);
    state.last_panes = panes.clone();
    state.clamp_focus_to_density(density);

    // Summary — ErrorState
    if let Some(r) = pane_area(&panes, "summary") {
        let view = build_error_state_view(snapshot, state, system);
        view.paint(r, buffer, &mut state.error);
    }

    // Preserved work strip
    if let Some(r) = pane_area(&panes, "preserved") {
        let inner = Panel::new(system)
            .title("Preserved work")
            .emphasis(PanelChrome::for_focus(state.focus == "preserved"))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            let msg = if snapshot.work_preserved {
                if snapshot.preserved_note.is_empty() {
                    "Your work was preserved.".to_string()
                } else {
                    snapshot.preserved_note.clone()
                }
            } else {
                "No preserved work snapshot.".into()
            };
            system.paint_row(
                buffer,
                Rect::new(inner.x, inner.y, inner.width, 1),
                &msg,
                system.style(if snapshot.work_preserved {
                    Role::TextStrong
                } else {
                    Role::TextMuted
                }),
            );
        }
    }

    // Actions list
    if let Some(r) = pane_area(&panes, "actions") {
        let focused = state.focus == "actions";
        let inner = Panel::new(system)
            .title("Recovery options")
            .emphasis(PanelChrome::for_focus(focused))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            let rows = recovery_action_rows(state.action_set());
            let list = List::new(&rows, system).focused(focused);
            StatefulWidget::render(&list, inner, buffer, &mut state.actions);
        }
    }

    // Diagnostics — redacted report text
    if let Some(r) = pane_area(&panes, "diagnostics") {
        let focused = state.focus == "diagnostics";
        let inner = Panel::new(system)
            .title("Diagnostics (redacted)")
            .emphasis(PanelChrome::for_focus(focused))
            .paint(r, buffer, None);
        if !inner.is_empty() {
            let report = state.redacted_report(snapshot);
            let mut y = inner.y;
            let max_y = inner.y.saturating_add(inner.height);
            for line in report.lines().take(inner.height as usize) {
                if y >= max_y {
                    break;
                }
                system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    line,
                    system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
            // doctor cue
            if let Some(d) = doctor {
                if y < max_y {
                    let cue = format!("doctor findings: {}", d.findings.len());
                    system.paint_row(
                        buffer,
                        Rect::new(inner.x, y, inner.width, 1),
                        &cue,
                        system.style(Role::TextSecondary),
                    );
                }
            }
        }
    }

    // Status
    if let Some(r) = pane_area(&panes, "status") {
        if state.terminal_restore_failed {
            state.status.transient =
                Some("terminal restore failed · prefer safe quit after copy".into());
        } else if state.partial_init {
            state.status.transient = Some("partial init · capabilities may be limited".into());
        } else {
            state.status.transient = Some("secrets redacted in copy/report".into());
        }
        let slots = state.status_slots();
        StatefulWidget::render(
            &StatusBar::new(&slots, &[], system),
            r,
            buffer,
            &mut state.status,
        );
    }
}

// ── Fixtures ────────────────────────────────────────────────────────────────

/// Example crash snapshot with secrets that must be redacted.
#[must_use]
pub fn example_crash_snapshot_with_secrets() -> CrashReportSnapshot {
    CrashReportSnapshot {
        summary: "Unexpected panic in table paint".into(),
        technical: "panic: index out of bounds at widgets/table.rs:412".into(),
        source: "termrock".into(),
        preserved_note: "Composer draft retained".into(),
        work_preserved: true,
        env_lines: vec![
            "TERM=xterm-256color".into(),
            "API_KEY=sk-secret-should-not-leak-1234567890abcdef".into(),
            "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.sig".into(),
            "PASSWORD=hunter2supersecret".into(),
            "USER=alex".into(),
        ],
        log_lines: vec![
            "INFO ready".into(),
            "DEBUG token=ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345".into(),
            "ERROR panic recovered".into(),
        ],
        capabilities_text: "color=truecolor glyphs=unicode".into(),
        class: FailureClass::Crash,
    }
}

/// Clean network-style recovery snapshot.
#[must_use]
pub fn example_recovery_snapshot() -> CrashReportSnapshot {
    CrashReportSnapshot {
        summary: "Request failed".into(),
        technical: "timeout after 30s: GET /v1/jobs".into(),
        source: "jobs-service".into(),
        preserved_note: "Draft retained in editor".into(),
        work_preserved: true,
        env_lines: vec!["TERM=xterm-256color".into()],
        log_lines: vec!["WARN timeout".into()],
        capabilities_text: String::new(),
        class: FailureClass::Network,
    }
}

/// Terminal restore failure fixture.
#[must_use]
pub fn example_terminal_restore_failed_snapshot() -> CrashReportSnapshot {
    let mut s = example_crash_snapshot_with_secrets();
    s.class = FailureClass::TerminalRestoreFailed;
    s.summary = "Terminal state may be inconsistent".into();
    s
}

/// Seed terminal restore failed chrome.
pub fn seed_terminal_restore_failed(state: &mut ErrorRecoveryState) {
    state.terminal_restore_failed = true;
    state.mode = ErrorRecoveryMode::Full;
}

/// Seed partial init.
pub fn seed_partial_init(state: &mut ErrorRecoveryState) {
    state.partial_init = true;
}

/// Seed inline fallback.
pub fn seed_inline_fallback(state: &mut ErrorRecoveryState) {
    let _ = state.set_mode(ErrorRecoveryMode::InlineFallback);
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Large diagnostic blob lines.
    pub const DIAG_LINES: usize = 400;
    /// Paint frames.
    pub const PAINT_FRAMES: usize = 8;
    /// Viewport.
    pub const VIEWPORT: (u16, u16) = (100, 36);
}

/// Large snapshot for paint stress.
#[must_use]
pub fn burst_crash_snapshot(n_log: usize) -> CrashReportSnapshot {
    let mut s = example_crash_snapshot_with_secrets();
    s.log_lines = (0..n_log)
        .map(|i| format!("LOG line {i} payload={}", i * 3))
        .collect();
    s
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open() -> ErrorRecoveryState {
        let mut st = ErrorRecoveryState::new();
        st.density = Some(ErrorRecoveryDensity::Normal);
        st
    }

    #[test]
    fn recovery_actions_typed_outcomes() {
        let mut st = open();
        let snap = example_crash_snapshot_with_secrets();
        st.focus = "actions";

        let out = st.handle_key(press(KeyCode::Char('r')), &snap);
        assert!(
            matches!(out, ErrorRecoveryOutcome::RestartRequested),
            "{out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('s')), &snap);
        assert!(
            matches!(out, ErrorRecoveryOutcome::RestoreSessionRequested),
            "{out:?}"
        );

        let out = st.handle_key(press(KeyCode::Char('c')), &snap);
        match out {
            ErrorRecoveryOutcome::CopyDiagnostics { text } => {
                assert!(text.contains("crash report") || text.contains("summary"));
                assert!(!text.contains("sk-secret-should-not-leak"));
                assert!(!text.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
                assert!(!text.contains("hunter2supersecret"));
                assert!(!text.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
            }
            other => panic!("expected CopyDiagnostics, got {other:?}"),
        }

        let out = st.handle_key(press(KeyCode::Char('i')), &snap);
        match out {
            ErrorRecoveryOutcome::ReportIssue { text } => {
                assert!(!text.contains("sk-secret-should-not-leak"));
                assert!(!text.contains("Bearer eyJ"));
            }
            other => panic!("expected ReportIssue, got {other:?}"),
        }

        let out = st.handle_key(press(KeyCode::Char('q')), &snap);
        assert!(matches!(out, ErrorRecoveryOutcome::SafeQuit), "{out:?}");

        let out = st.handle_key(press(KeyCode::Char('l')), &snap);
        assert!(matches!(out, ErrorRecoveryOutcome::OpenLogs), "{out:?}");

        let out = st.handle_key(press(KeyCode::Char('e')), &snap);
        assert!(
            matches!(out, ErrorRecoveryOutcome::ShowCapabilities),
            "{out:?}"
        );
    }

    #[test]
    fn secret_redaction_on_real_path() {
        let snap = example_crash_snapshot_with_secrets();
        let redacted = build_redacted_crash_report(&snap);
        // Secrets absent (full strings)
        for secret in [
            "sk-secret-should-not-leak-1234567890abcdef",
            "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.payload.sig",
            "hunter2supersecret",
            "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345",
        ] {
            assert!(
                !redacted.contains(secret),
                "secret still present: {secret}\n{redacted}"
            );
        }
        // Residual token bodies must not survive (assignment-key match, not value contains)
        for residual in ["sk-secret", "sk-", "ghp_", "hunter2", "eyJhbGci"] {
            assert!(
                !redacted.contains(residual),
                "residual token body {residual:?} still present:\n{redacted}"
            );
        }
        // API_KEY line specifically: key retained, value fully masked
        let api_line = redacted
            .lines()
            .find(|l| l.to_ascii_lowercase().contains("api_key"))
            .expect("API_KEY line must remain in report structure");
        assert!(
            !api_line.contains("sk-secret")
                && !api_line.contains("sk-")
                && !api_line.contains("should-not-leak"),
            "API_KEY value must be fully redacted, got {api_line:?}"
        );
        assert!(
            api_line.contains('●') || api_line.contains("****") || api_line.contains('…'),
            "API_KEY line should show mask glyphs: {api_line:?}"
        );
        // Non-secret context retained
        assert!(
            redacted.contains("TERM=xterm-256color") || redacted.contains("xterm"),
            "non-secret env should remain: {redacted}"
        );
        assert!(
            redacted.contains("USER=alex") || redacted.contains("alex"),
            "user context retained: {redacted}"
        );
        assert!(
            redacted.contains("panic") || redacted.contains("table"),
            "technical context retained"
        );

        // Copy path uses same function
        let mut st = open();
        let out = st.outcome_for_action(RecoveryActionId::CopyDiagnostics, &snap);
        match out {
            ErrorRecoveryOutcome::CopyDiagnostics { text } => {
                assert_eq!(text, redacted);
                let api = text
                    .lines()
                    .find(|l| l.to_ascii_lowercase().contains("api_key"))
                    .expect("copy path API_KEY line");
                assert!(!api.contains("sk-secret") && !api.contains("sk-"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn assignment_key_not_value_substring() {
        // Value contains "secret" but key is API_KEY — must not match bare "secret" first.
        let line = "API_KEY=sk-secret-should-not-leak-1234567890abcdef";
        let out = redact_crash_report_line(line);
        assert!(
            out.to_ascii_lowercase().starts_with("api_key"),
            "key name retained: {out}"
        );
        assert!(
            !out.contains("sk-secret") && !out.contains("sk-") && !out.contains("should-not-leak"),
            "value fully redacted: {out}"
        );
        // Control: bare secret assignment still redacts
        let out2 = redact_crash_report_line("SECRET=hunter2supersecret");
        assert!(!out2.contains("hunter2supersecret"), "{out2}");
    }

    #[test]
    fn inline_fallback_differs_from_full() {
        let ws = WorkspaceState::new();
        let full = error_recovery_layout_density(
            Rect::new(0, 0, 100, 36),
            &ws,
            ErrorRecoveryDensity::Normal,
            ErrorRecoveryMode::Full,
        );
        let inline = error_recovery_layout_density(
            Rect::new(0, 0, 100, 36),
            &ws,
            ErrorRecoveryDensity::Normal,
            ErrorRecoveryMode::InlineFallback,
        );
        let full_ids: Vec<_> = full
            .iter()
            .filter(|p| !p.collapsed && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        let inline_ids: Vec<_> = inline
            .iter()
            .filter(|p| !p.collapsed && p.area.width > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        assert!(full_ids.contains(&"diagnostics"));
        assert!(!inline_ids.contains(&"diagnostics"));
        assert!(inline_ids.contains(&"summary"));
        assert!(inline_ids.contains(&"actions"));

        let mut st = open();
        let out = st.set_mode(ErrorRecoveryMode::InlineFallback);
        assert!(matches!(
            out,
            ErrorRecoveryOutcome::ModeChanged(ErrorRecoveryMode::InlineFallback)
        ));
        let vis = st.visible_focus_panes(ErrorRecoveryDensity::Normal);
        assert!(!vis.contains(&ErrorRecoveryPane::Diagnostics));
        assert_eq!(st.action_set().len(), RecoveryActionId::inline_set().len());
    }

    #[test]
    fn focus_cycle_and_enter_action() {
        let mut st = open();
        let snap = example_recovery_snapshot();
        st.focus = "summary";
        let mut seen = vec![st.focus];
        for _ in 0..8 {
            let out = st.handle_key(press(KeyCode::Tab), &snap);
            assert!(matches!(out, ErrorRecoveryOutcome::FocusChanged(_)));
            seen.push(st.focus);
        }
        assert!(seen.contains(&"summary"));
        assert!(seen.contains(&"actions"));
        assert!(!seen.contains(&"status"));

        st.focus = "actions";
        st.actions = ListState::new(Some(RecoveryActionId::SafeQuit.id().into()));
        let out = st.handle_key(press(KeyCode::Enter), &snap);
        assert!(matches!(out, ErrorRecoveryOutcome::SafeQuit), "{out:?}");
    }

    #[test]
    fn no_process_or_network_in_composition() {
        let body = include_str!("error_recovery.rs");
        let code = body
            .split("fn no_process_or_network_in_composition")
            .next()
            .unwrap_or(body);
        for forbidden in [
            "std::process::",
            "Command::new",
            "std::panic::",
            "set_hook",
            "TcpStream",
            "reqwest",
            "ureq",
            "std::fs::",
        ] {
            let hits: Vec<_> = code
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    !t.starts_with("//")
                        && !t.starts_with("//!")
                        && !t.starts_with('*')
                        && l.contains(forbidden)
                })
                .collect();
            assert!(hits.is_empty(), "forbidden {forbidden}: {hits:?}");
        }
    }

    #[test]
    fn paint_smoke_full_and_inline() {
        let system = DesignSystem::default();
        let snap = example_crash_snapshot_with_secrets();
        let mut st = open();
        let area = Rect::new(0, 0, 100, 36);
        let mut buf = Buffer::empty(area);
        paint_error_recovery(
            &mut buf,
            area,
            ErrorRecoverySurfaces {
                system: &system,
                state: &mut st,
                snapshot: &snap,
                doctor: None,
            },
        );
        assert!(st.last_panes().iter().any(|p| p.id.0.as_str() == "summary"));
        // redacted cache filled for diagnostics
        assert!(st.redacted_cache.is_some());
        let report = st.redacted_cache.as_deref().unwrap_or("");
        assert!(!report.contains("sk-secret-should-not-leak"));

        seed_inline_fallback(&mut st);
        let mut buf2 = Buffer::empty(area);
        paint_error_recovery(
            &mut buf2,
            area,
            ErrorRecoverySurfaces {
                system: &system,
                state: &mut st,
                snapshot: &snap,
                doctor: None,
            },
        );
        assert!(
            !st.last_panes()
                .iter()
                .any(|p| p.id.0.as_str() == "diagnostics" && !p.collapsed && p.area.height > 0)
        );
    }

    #[test]
    fn burst_paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        st.density = Some(ErrorRecoveryDensity::Normal);
        let snap = burst_crash_snapshot(bench::DIAG_LINES);
        let area = Rect::new(0, 0, bench::VIEWPORT.0, bench::VIEWPORT.1);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            st.invalidate_report_cache();
            paint_error_recovery(
                &mut buf,
                area,
                ErrorRecoverySurfaces {
                    system: &system,
                    state: &mut st,
                    snapshot: &snap,
                    doctor: None,
                },
            );
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 5, "paint too slow: {elapsed:?}");
    }

    #[test]
    fn layout_full_normal_has_diagnostics() {
        let ws = WorkspaceState::new();
        let panes = error_recovery_layout_density(
            Rect::new(0, 0, 100, 36),
            &ws,
            ErrorRecoveryDensity::Normal,
            ErrorRecoveryMode::Full,
        );
        let ids: Vec<_> = panes
            .iter()
            .filter(|p| !p.collapsed && p.area.width > 0 && p.area.height > 0)
            .map(|p| p.id.0.as_str())
            .collect();
        for need in ["summary", "actions", "diagnostics", "status"] {
            assert!(ids.contains(&need), "missing {need} in {ids:?}");
        }
    }
}
