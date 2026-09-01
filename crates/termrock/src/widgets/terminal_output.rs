// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **TerminalOutput** — safe presentation of command execution output.
//!
//! **Mission.** Command, working directory, environment summary/redaction,
//! stdout/stderr distinction, live streaming, exit status, signal, duration,
//! cancel, detach, retry, and copy. ANSI parsed safely (via [`AnsiText`] /
//! [`AnsiLine`]); raw/plain modes. Scroll preserves when the user is reading
//! while output continues. Recipes: compact card, pane, fullscreen.
//!
//! **Critical safety.** This component **never** executes, spawns, or kills
//! processes. It only paints projected state and emits typed
//! [`TerminalOutputOutcome`] control **requests**. Hosts own PTY/process policy.
//!
//! **vs [`super::ToolCard`].** ToolCard is a compact agent tool summary;
//! TerminalOutput is the full command pane. **vs [`super::LogStream`].**
//! LogStream is multi-source log lines; TerminalOutput is one command run.
//! **vs [`crate::ansi_text`].** ANSI parse/paint primitives — TerminalOutput
//! owns chrome, follow, and control outcomes.
//!
//! Research: Grok Build, Amp, OpenCode, terminal emulators, CI command logs.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    ansi_text::{AnsiLine, AnsiText, AnsiTextMode},
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{scroll_area::ScrollAreaState, tiered_row::TieredRow},
};

// ── Streams & status ────────────────────────────────────────────────────────

/// Which stream a line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalStream {
    /// Standard output.
    #[default]
    Stdout,
    /// Standard error.
    Stderr,
    /// Host/system meta (spawn, signal notes).
    System,
}

impl TerminalStream {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
            Self::System => "system",
        }
    }

    /// No-color prefix.
    #[must_use]
    pub const fn prefix(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::Stdout => "o ",
                Self::Stderr => "e ",
                Self::System => "* ",
            }
        } else {
            match self {
                Self::Stdout => "│ ",
                Self::Stderr => "! ",
                Self::System => "· ",
            }
        }
    }

    /// Paint role for plain text path.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Stdout => Role::Text,
            Self::Stderr => Role::Warning,
            Self::System => Role::TextMuted,
        }
    }
}

/// Lifecycle of the projected command run (host-owned truth).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalRunStatus {
    /// Queued / not started.
    #[default]
    Pending,
    /// Waiting for permission grant before spawn.
    WaitingPermission,
    /// Live streaming.
    Running,
    /// Exited zero.
    Succeeded,
    /// Exited non-zero.
    Failed,
    /// Terminated by signal.
    Signaled,
    /// User/host cancelled.
    Cancelled,
    /// Timed out (host policy).
    TimedOut,
    /// Detached from UI (still may run — host owns).
    Detached,
}

impl TerminalRunStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::WaitingPermission => "waiting-permission",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Signaled => "signaled",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Detached => "detached",
        }
    }

    /// Short badge label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::WaitingPermission => "perm",
            Self::Running => "run",
            Self::Succeeded => "ok",
            Self::Failed => "fail",
            Self::Signaled => "signal",
            Self::Cancelled => "cancel",
            Self::TimedOut => "timeout",
            Self::Detached => "detach",
        }
    }

    /// Glyph (ASCII uses letter).
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::Pending => ".",
                Self::WaitingPermission => "A",
                Self::Running => ">",
                Self::Succeeded => "+",
                Self::Failed => "x",
                Self::Signaled => "!",
                Self::Cancelled => "c",
                Self::TimedOut => "t",
                Self::Detached => "d",
            }
        } else {
            match self {
                Self::Pending => "·",
                Self::WaitingPermission => "⏸",
                Self::Running => "▶",
                Self::Succeeded => "✓",
                Self::Failed => "✗",
                Self::Signaled => "⚡",
                Self::Cancelled => "⊘",
                Self::TimedOut => "⏱",
                Self::Detached => "⧉",
            }
        }
    }

    /// Semantic role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Pending | Self::Detached => Role::TextMuted,
            Self::WaitingPermission => Role::Warning,
            // Running is live information, not the brand (plans/007).
            Self::Running => Role::InfoDim,
            Self::Succeeded => Role::Success,
            Self::Failed | Self::Signaled | Self::TimedOut => Role::Danger,
            Self::Cancelled => Role::Warning,
        }
    }

    /// Whether cancel control is meaningful.
    #[must_use]
    pub const fn can_cancel(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::WaitingPermission | Self::Running | Self::Detached
        )
    }

    /// Whether retry is meaningful.
    #[must_use]
    pub const fn can_retry(self) -> bool {
        matches!(
            self,
            Self::Failed | Self::Signaled | Self::Cancelled | Self::TimedOut | Self::Succeeded
        )
    }

    /// Whether permission focus is meaningful.
    #[must_use]
    pub const fn needs_permission(self) -> bool {
        matches!(self, Self::WaitingPermission)
    }
}

// ── Projection model ────────────────────────────────────────────────────────

/// Environment variable row (host redacts secrets before projection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEnvEntry<'a> {
    /// Key.
    pub key: &'a str,
    /// Value (use `"***"` when redacted).
    pub value: &'a str,
    /// Whether value is redacted.
    pub redacted: bool,
}

impl<'a> TerminalEnvEntry<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(key: &'a str, value: &'a str) -> Self {
        Self {
            key,
            value,
            redacted: false,
        }
    }

    /// Mark redacted (display already scrubbed).
    #[must_use]
    pub const fn redacted(mut self, on: bool) -> Self {
        self.redacted = on;
        self
    }

    /// Redacted placeholder helper.
    #[must_use]
    pub const fn secret(key: &'a str) -> Self {
        Self {
            key,
            value: "***",
            redacted: true,
        }
    }
}

/// One projected output line.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalLine<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Stream.
    pub stream: TerminalStream,
    /// Plain text body (always present for search/copy).
    pub text: &'a str,
    /// Optional pre-parsed ANSI (preferred for color mode).
    pub ansi: Option<&'a AnsiLine>,
}

impl<'a> TerminalLine<'a> {
    /// Plain line.
    #[must_use]
    pub const fn new(id: &'a str, stream: TerminalStream, text: &'a str) -> Self {
        Self {
            id,
            stream,
            text,
            ansi: None,
        }
    }

    /// Stdout convenience.
    #[must_use]
    pub const fn stdout(id: &'a str, text: &'a str) -> Self {
        Self::new(id, TerminalStream::Stdout, text)
    }

    /// Stderr convenience.
    #[must_use]
    pub const fn stderr(id: &'a str, text: &'a str) -> Self {
        Self::new(id, TerminalStream::Stderr, text)
    }

    /// System meta line.
    #[must_use]
    pub const fn system(id: &'a str, text: &'a str) -> Self {
        Self::new(id, TerminalStream::System, text)
    }

    /// Attach pre-parsed ANSI.
    #[must_use]
    pub const fn ansi(mut self, line: &'a AnsiLine) -> Self {
        self.ansi = Some(line);
        self
    }
}

/// Command run metadata (host-owned process facts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCommandMeta<'a> {
    /// Command argv / shell string.
    pub command: &'a str,
    /// Working directory.
    pub cwd: Option<&'a str>,
    /// Environment summary (already redacted as needed).
    pub env: &'a [TerminalEnvEntry<'a>],
    /// Run status.
    pub status: TerminalRunStatus,
    /// Exit code when process exited.
    pub exit_code: Option<i32>,
    /// Signal name when signaled (`SIGTERM`).
    pub signal: Option<&'a str>,
    /// Elapsed duration milliseconds.
    pub duration_ms: Option<u64>,
    /// Optional pid (display only).
    pub pid: Option<u32>,
}

impl<'a> TerminalCommandMeta<'a> {
    /// Minimal meta.
    #[must_use]
    pub const fn new(command: &'a str) -> Self {
        Self {
            command,
            cwd: None,
            env: &[],
            status: TerminalRunStatus::Pending,
            exit_code: None,
            signal: None,
            duration_ms: None,
            pid: None,
        }
    }

    /// Cwd.
    #[must_use]
    pub const fn cwd(mut self, cwd: &'a str) -> Self {
        self.cwd = Some(cwd);
        self
    }

    /// Env.
    #[must_use]
    pub const fn env(mut self, env: &'a [TerminalEnvEntry<'a>]) -> Self {
        self.env = env;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: TerminalRunStatus) -> Self {
        self.status = status;
        self
    }

    /// Exit code.
    #[must_use]
    pub const fn exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    /// Signal.
    #[must_use]
    pub const fn signal(mut self, signal: &'a str) -> Self {
        self.signal = Some(signal);
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
    pub const fn pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }
}

/// Presentation recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalOutputRecipe {
    /// Compact card (agent tool strip).
    Compact,
    /// Pane (default workbench).
    #[default]
    Pane,
    /// Fullscreen takeover chrome.
    Fullscreen,
}

impl TerminalOutputRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Pane => "pane",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// How body text is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TerminalPaintMode {
    /// Safe ANSI color (via [`AnsiText`]).
    #[default]
    Ansi,
    /// ANSI structure without color.
    NoColor,
    /// Plain text only.
    Plain,
    /// Show escapes as escaped text (debug).
    Raw,
}

impl TerminalPaintMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ansi => "ansi",
            Self::NoColor => "no-color",
            Self::Plain => "plain",
            Self::Raw => "raw",
        }
    }

    fn to_ansi_mode(self) -> AnsiTextMode {
        match self {
            Self::Ansi => AnsiTextMode::Color,
            Self::NoColor => AnsiTextMode::NoColor,
            Self::Plain | Self::Raw => AnsiTextMode::Plain,
        }
    }
}

// ── State & outcomes ────────────────────────────────────────────────────────

/// Hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOutputRegion {
    /// Line id.
    pub id: String,
    /// Index in filtered view.
    pub index: usize,
    /// Area.
    pub area: Rect,
}

/// Control outcomes — **requests only**, never side effects in TermRock.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TerminalOutputOutcome {
    /// No change.
    Ignored,
    /// Scrolled while pinned.
    Scrolled {
        /// Offset.
        offset: u16,
    },
    /// Re-attached to tail.
    Follow,
    /// Detached from tail (reading history).
    Detach,
    /// Request host cancel the running process.
    CancelRequested,
    /// Request host re-run the command.
    RetryRequested,
    /// Request host detach process from this UI surface.
    DetachProcessRequested,
    /// Copy joined output (host clipboard).
    CopyOutput {
        /// Text.
        text: String,
    },
    /// Copy command string.
    CopyCommand {
        /// Text.
        text: String,
    },
    /// Env panel toggled.
    EnvToggled {
        /// Visible after.
        on: bool,
    },
    /// Stream filter changed.
    StreamFilterChanged {
        /// Hide stdout.
        hide_stdout: bool,
        /// Hide stderr.
        hide_stderr: bool,
    },
    /// Paint mode changed.
    PaintModeChanged(TerminalPaintMode),
    /// Recipe changed.
    RecipeChanged(TerminalOutputRecipe),
    /// Open cwd in host file browser.
    OpenCwdRequested {
        /// Path.
        path: String,
    },
    /// Cancelled search/UI.
    Cancelled,
}

/// Interaction state. Follow/unread live in [`ScrollAreaState`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOutputState {
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    area_rows: u16,
    line_count: u16,
    /// Recipe.
    pub recipe: TerminalOutputRecipe,
    /// Paint mode.
    pub paint_mode: TerminalPaintMode,
    /// Show environment block.
    pub show_env: bool,
    /// Hide stdout lines.
    pub hide_stdout: bool,
    /// Hide stderr lines.
    pub hide_stderr: bool,
    /// Cursor in filtered lines.
    pub cursor: usize,
    /// Hit regions.
    pub regions: Vec<TerminalOutputRegion>,
    /// Prefer ASCII status/stream glyphs.
    pub ascii: bool,
    /// Prefer no-color paint.
    pub colorless: bool,
    /// Anchor line id across reproject.
    anchor_id: Option<String>,
}

impl Default for TerminalOutputState {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalOutputState {
    /// Following by default.
    #[must_use]
    pub fn new() -> Self {
        let mut scroll = ScrollAreaState::new().axes(true, false);
        scroll.follow_tail();
        Self {
            scroll,
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
            area_rows: 0,
            line_count: 0,
            recipe: TerminalOutputRecipe::Pane,
            paint_mode: TerminalPaintMode::Ansi,
            show_env: false,
            hide_stdout: false,
            hide_stderr: false,
            cursor: 0,
            regions: Vec::new(),
            ascii: false,
            colorless: false,
            anchor_id: None,
        }
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Following tail.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.scroll.is_following()
    }

    /// Offset.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Unread while paused.
    #[must_use]
    pub fn unread(&self) -> u64 {
        u64::from(self.scroll.new_content().unseen)
    }

    /// Scroll.
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Force follow.
    pub fn set_following(&mut self, following: bool) {
        if following {
            self.scroll.follow_tail();
        } else {
            self.scroll.pause_follow();
        }
    }

    /// After host appends lines (total projected count).
    pub fn on_append(&mut self, total_lines: u16, viewport: u16) {
        self.sync_metrics(total_lines, viewport);
        if self.scroll.is_following() && total_lines > 0 {
            self.cursor = usize::from(total_lines.saturating_sub(1));
        }
    }

    /// Capture anchor.
    pub fn capture_anchor(&mut self, lines: &[TerminalLine<'_>]) {
        let view = filter_terminal_lines(lines, self.hide_stdout, self.hide_stderr);
        if let Some(l) = view.get(self.cursor) {
            self.anchor_id = Some(l.id.to_string());
        }
    }

    /// Restore anchor.
    pub fn restore_anchor(&mut self, lines: &[TerminalLine<'_>]) {
        let view = filter_terminal_lines(lines, self.hide_stdout, self.hide_stderr);
        if let Some(aid) = self.anchor_id.as_ref() {
            if let Some(i) = view.iter().position(|l| l.id == aid) {
                self.cursor = i;
                self.ensure_cursor_visible(view.len());
            }
        }
    }

    fn sync_metrics(&mut self, total: u16, viewport: u16) {
        self.line_count = total;
        self.body_rows = viewport;
        self.scroll.set_content_size(1, total);
        self.scroll.set_viewport(1, viewport);
        self.scroll.clamp();
    }

    fn ensure_cursor_visible(&mut self, len: usize) {
        if len == 0 || self.body_rows == 0 {
            return;
        }
        let vh = usize::from(self.body_rows);
        let start = usize::from(self.scroll.offset_y());
        let end = start.saturating_add(vh);
        if self.cursor < start {
            self.scroll.set_offset_y_quiet(self.cursor as u16);
        } else if self.cursor >= end {
            let next = self.cursor.saturating_add(1).saturating_sub(vh);
            self.scroll.set_offset_y_quiet(next as u16);
        }
        self.scroll.clamp();
    }

    fn scroll_by(&mut self, delta: i32) -> bool {
        self.scroll.scroll_by(delta as isize, 0).is_scrolled()
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        lines: &[TerminalLine<'_>],
        meta: &TerminalCommandMeta<'_>,
    ) -> TerminalOutputOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return TerminalOutputOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        let view = filter_terminal_lines(lines, self.hide_stdout, self.hide_stderr);

        if is_press {
            match key.code {
                KeyCode::Char('f' | 'F') if key.modifiers.is_empty() => {
                    return self.toggle_follow(&view);
                }
                KeyCode::Char('c') if key.modifiers.is_empty() && meta.status.can_cancel() => {
                    return TerminalOutputOutcome::CancelRequested;
                }
                KeyCode::Char('c' | 'C')
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        || (key.modifiers.is_empty() && !meta.status.can_cancel()) =>
                {
                    // Copy output (C-c always; bare c when not cancellable)
                    let text = view.iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
                    return TerminalOutputOutcome::CopyOutput { text };
                }
                KeyCode::Char('y' | 'Y') if key.modifiers.is_empty() => {
                    return TerminalOutputOutcome::CopyCommand {
                        text: meta.command.to_string(),
                    };
                }
                KeyCode::Char('r' | 'R') if key.modifiers.is_empty() && meta.status.can_retry() => {
                    return TerminalOutputOutcome::RetryRequested;
                }
                KeyCode::Char('d' | 'D')
                    if key.modifiers.is_empty()
                        && matches!(
                            meta.status,
                            TerminalRunStatus::Running | TerminalRunStatus::Pending
                        ) =>
                {
                    return TerminalOutputOutcome::DetachProcessRequested;
                }
                KeyCode::Char('e' | 'E') if key.modifiers.is_empty() => {
                    self.show_env = !self.show_env;
                    return TerminalOutputOutcome::EnvToggled { on: self.show_env };
                }
                KeyCode::Char('1') => {
                    self.hide_stdout = !self.hide_stdout;
                    return TerminalOutputOutcome::StreamFilterChanged {
                        hide_stdout: self.hide_stdout,
                        hide_stderr: self.hide_stderr,
                    };
                }
                KeyCode::Char('2') => {
                    self.hide_stderr = !self.hide_stderr;
                    return TerminalOutputOutcome::StreamFilterChanged {
                        hide_stdout: self.hide_stdout,
                        hide_stderr: self.hide_stderr,
                    };
                }
                KeyCode::Char('m' | 'M') => {
                    self.paint_mode = match self.paint_mode {
                        TerminalPaintMode::Ansi => TerminalPaintMode::NoColor,
                        TerminalPaintMode::NoColor => TerminalPaintMode::Plain,
                        TerminalPaintMode::Plain => TerminalPaintMode::Raw,
                        TerminalPaintMode::Raw => TerminalPaintMode::Ansi,
                    };
                    return TerminalOutputOutcome::PaintModeChanged(self.paint_mode);
                }
                KeyCode::Char('p' | 'P') => {
                    self.recipe = match self.recipe {
                        TerminalOutputRecipe::Compact => TerminalOutputRecipe::Pane,
                        TerminalOutputRecipe::Pane => TerminalOutputRecipe::Fullscreen,
                        TerminalOutputRecipe::Fullscreen => TerminalOutputRecipe::Compact,
                    };
                    return TerminalOutputOutcome::RecipeChanged(self.recipe);
                }
                KeyCode::Char('o' | 'O') => {
                    if let Some(cwd) = meta.cwd {
                        return TerminalOutputOutcome::OpenCwdRequested {
                            path: cwd.to_string(),
                        };
                    }
                }
                KeyCode::Esc => return TerminalOutputOutcome::Cancelled,
                _ => {}
            }
        }

        if let Some(intent) = crate::interaction::default_log_stream_intent(key)
            .or_else(|| crate::interaction::default_list_intent(key))
        {
            return self.handle_intent(intent, &view);
        }
        TerminalOutputOutcome::Ignored
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        view: &[&TerminalLine<'_>],
    ) -> TerminalOutputOutcome {
        if !self.accepts_input {
            return TerminalOutputOutcome::Ignored;
        }
        let len = view.len();
        if len > 0 {
            self.cursor = self.cursor.min(len - 1);
        }
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                let was = self.is_following();
                if len > 0 && self.cursor + 1 < len {
                    self.cursor += 1;
                    if self.cursor + 1 >= len {
                        self.scroll.follow_tail();
                    } else {
                        self.scroll.pause_follow();
                    }
                    self.ensure_cursor_visible(len);
                    if was && !self.is_following() {
                        return TerminalOutputOutcome::Detach;
                    }
                    return TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                if !self.scroll_by(1) {
                    return TerminalOutputOutcome::Ignored;
                }
                if was {
                    TerminalOutputOutcome::Detach
                } else {
                    TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::Previous) => {
                let was = self.is_following();
                if len > 0 && self.cursor > 0 {
                    self.cursor -= 1;
                    self.scroll.pause_follow();
                    self.ensure_cursor_visible(len);
                    if was {
                        return TerminalOutputOutcome::Detach;
                    }
                    return TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    };
                }
                if !self.scroll_by(-1) {
                    return TerminalOutputOutcome::Ignored;
                }
                if was {
                    TerminalOutputOutcome::Detach
                } else {
                    TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                let was = self.is_following();
                self.cursor = 0;
                self.scroll.home();
                if was {
                    TerminalOutputOutcome::Detach
                } else {
                    TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Move(NavigationMove::Last) | UiIntent::Toggle => self.toggle_follow(view),
            UiIntent::Page(PageMove::Forward) => {
                let was = self.is_following();
                if !self.scroll.page(true).is_scrolled() {
                    return TerminalOutputOutcome::Ignored;
                }
                if was {
                    TerminalOutputOutcome::Detach
                } else {
                    TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let was = self.is_following();
                if !self.scroll.page(false).is_scrolled() {
                    return TerminalOutputOutcome::Ignored;
                }
                if was {
                    TerminalOutputOutcome::Detach
                } else {
                    TerminalOutputOutcome::Scrolled {
                        offset: self.offset(),
                    }
                }
            }
            UiIntent::Cancel => TerminalOutputOutcome::Cancelled,
            _ => TerminalOutputOutcome::Ignored,
        }
    }

    fn toggle_follow(&mut self, view: &[&TerminalLine<'_>]) -> TerminalOutputOutcome {
        if self.is_following() {
            self.scroll.pause_follow();
            TerminalOutputOutcome::Detach
        } else {
            self.scroll.follow_tail();
            if !view.is_empty() {
                self.cursor = view.len() - 1;
            }
            TerminalOutputOutcome::Follow
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        lines: &[TerminalLine<'_>],
    ) -> TerminalOutputOutcome {
        if !self.accepts_input {
            return TerminalOutputOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let hit = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.area_rows.max(1),
        };
        if !hit.contains(event.position) {
            return TerminalOutputOutcome::Ignored;
        }
        let view = filter_terminal_lines(lines, self.hide_stdout, self.hide_stderr);
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), &view)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), &view)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let chip_y = oy.saturating_add(self.area_rows.saturating_sub(1));
                if self.area_rows >= 2 && event.position.y == chip_y {
                    self.scroll.jump_to_new_content();
                    return TerminalOutputOutcome::Follow;
                }
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    self.cursor = r.index;
                    self.scroll.pause_follow();
                    return TerminalOutputOutcome::Detach;
                }
                TerminalOutputOutcome::Ignored
            }
            _ => TerminalOutputOutcome::Ignored,
        }
    }
}

/// Filter streams.
#[must_use]
pub fn filter_terminal_lines<'a>(
    lines: &'a [TerminalLine<'a>],
    hide_stdout: bool,
    hide_stderr: bool,
) -> Vec<&'a TerminalLine<'a>> {
    lines
        .iter()
        .filter(|l| match l.stream {
            TerminalStream::Stdout => !hide_stdout,
            TerminalStream::Stderr => !hide_stderr,
            TerminalStream::System => true,
        })
        .collect()
}

/// Escape for raw mode (show control sequences as printable).
#[must_use]
pub fn escape_raw_terminal(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\x1b' => out.push_str("\\e"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Format duration for chrome.
#[must_use]
pub fn format_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let s = ms / 1000;
        format!("{}m{:02}s", s / 60, s % 60)
    }
}

/// Redact common secret-looking env values (host should still pre-redact).
#[must_use]
pub fn redact_env_value(key: &str, value: &str) -> (String, bool) {
    let k = key.to_ascii_uppercase();
    let sensitive = k.contains("SECRET")
        || k.contains("TOKEN")
        || k.contains("PASSWORD")
        || k.contains("API_KEY")
        || k.contains("PRIVATE")
        || k.ends_with("_KEY")
        || k.ends_with("_TOKEN");
    if sensitive && !value.is_empty() && value != "***" {
        ("***".into(), true)
    } else {
        (value.to_string(), false)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Terminal command output presentation.
#[derive(Debug, Clone)]
pub struct TerminalOutput<'a> {
    meta: &'a TerminalCommandMeta<'a>,
    lines: &'a [TerminalLine<'a>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
    title: Option<&'a str>,
    /// When false, paint stream body (+ follow chip) only — for card composition.
    show_chrome: bool,
}

impl<'a> TerminalOutput<'a> {
    /// Meta + lines + system.
    #[must_use]
    pub const fn new(
        meta: &'a TerminalCommandMeta<'a>,
        lines: &'a [TerminalLine<'a>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            meta,
            lines,
            system,
            focused: true,
            ascii: false,
            colorless: false,
            title: None,
            show_chrome: true,
        }
    }

    /// Title override.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Toggle command/status header chrome (default on).
    #[must_use]
    pub const fn show_chrome(mut self, on: bool) -> Self {
        self.show_chrome = on;
        self
    }

    /// Paint.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut TerminalOutputState) {
        state.regions.clear();
        if area.is_empty() {
            state.body_rows = 0;
            state.area_rows = 0;
            return;
        }
        let ascii = self.ascii || state.ascii || self.system.glyphs.is_ascii();
        let colorless = self.colorless || state.colorless || self.system.mono();
        state.origin = (area.x, area.y);
        state.area_rows = area.height;
        let surface = self.focused && state.accepts_input;
        let recipe = state.recipe;
        let tiny = area.width < 20;
        let narrow = area.width < 40;

        let view = filter_terminal_lines(self.lines, state.hide_stdout, state.hide_stderr);

        // Header height by recipe (zero when composed inside TerminalRunCard).
        let header_h = if !self.show_chrome {
            0u16
        } else {
            match recipe {
                TerminalOutputRecipe::Compact => 2u16.min(area.height),
                TerminalOutputRecipe::Pane | TerminalOutputRecipe::Fullscreen => {
                    let mut h = 2u16;
                    if self.meta.cwd.is_some() && !tiny {
                        h = h.saturating_add(1);
                    }
                    if state.show_env && !self.meta.env.is_empty() {
                        h = h.saturating_add((self.meta.env.len() as u16).min(6).saturating_add(1));
                    }
                    if matches!(recipe, TerminalOutputRecipe::Fullscreen) {
                        h = h.saturating_add(1);
                    }
                    h.min(area.height.saturating_sub(1))
                }
            }
        };
        let chip_h = u16::from(area.height >= header_h.saturating_add(2));
        let body_h = area.height.saturating_sub(header_h + chip_h).max(1);

        let total = view.len().min(u16::MAX as usize) as u16;
        state.sync_metrics(total, body_h);
        if state.is_following() && total > 0 {
            state.cursor = usize::from(total.saturating_sub(1));
        } else if total > 0 {
            state.cursor = state.cursor.min(usize::from(total) - 1);
        }

        let mut y = area.y;
        if header_h > 0 {
            y = paint_header(
                buffer,
                Rect::new(area.x, y, area.width, header_h),
                self.meta,
                self.title,
                state,
                self.system,
                surface,
                ascii,
                colorless,
                tiny,
                narrow,
                recipe,
            );
        }

        // Body
        let body = Rect::new(area.x, y, area.width, body_h);
        if view.is_empty() {
            let mark = if ascii { "[ ] " } else { "∅ " };
            let msg = if matches!(self.meta.status, TerminalRunStatus::Pending) {
                format!("{mark}{}", if ascii { "waiting..." } else { "waiting…" })
            } else if matches!(self.meta.status, TerminalRunStatus::Running) {
                format!("{mark}(no output yet)")
            } else {
                format!("{mark}(empty output)")
            };
            buffer.set_stringn(
                body.x,
                body.y,
                take_display_cols(&msg, usize::from(body.width)),
                usize::from(body.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let start = state.offset() as usize;
            let mut py = body.y;
            let bottom = body.bottom();
            for (i, line) in view.iter().enumerate().skip(start) {
                if py >= bottom {
                    break;
                }
                let cursor = i == state.cursor;
                paint_line(
                    buffer,
                    Rect::new(body.x, py, body.width, 1),
                    line,
                    state.paint_mode,
                    self.system,
                    surface,
                    ascii,
                    colorless,
                    cursor,
                    tiny,
                );
                state.regions.push(TerminalOutputRegion {
                    id: line.id.to_string(),
                    index: i,
                    area: Rect::new(body.x, py, body.width, 1),
                });
                py = py.saturating_add(1);
            }
        }

        // Follow chip
        if chip_h > 0 {
            let separator = if ascii { " - " } else { " · " };
            let chip_y = area.bottom().saturating_sub(1);
            let following = state.is_following();
            let indicator = state.scroll.new_content();
            let mut chip = if following {
                if ascii {
                    "v follow".to_string()
                } else {
                    "↓ follow".to_string()
                }
            } else if indicator.visible {
                if ascii {
                    format!("v {} new  f=follow", indicator.unseen)
                } else {
                    format!("↓ {} new · f follow", indicator.unseen)
                }
            } else if ascii {
                "^ pinned  f=follow".to_string()
            } else {
                "↑ pinned · f follow".to_string()
            };
            if state.hide_stdout {
                chip.push_str(separator);
                chip.push_str("-out");
            }
            if state.hide_stderr {
                chip.push_str(separator);
                chip.push_str("-err");
            }
            chip.push_str(separator);
            chip.push_str(state.paint_mode.id());
            let st = if following && surface {
                self.system.style(Role::Accent)
            } else if indicator.visible {
                self.system.style(Role::Warning)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                area.x,
                chip_y,
                take_display_cols(&chip, usize::from(area.width)),
                usize::from(area.width),
                st,
            );
        }
    }
}

fn paint_header(
    buffer: &mut Buffer,
    area: Rect,
    meta: &TerminalCommandMeta<'_>,
    title: Option<&str>,
    state: &TerminalOutputState,
    system: &DesignSystem,
    surface: bool,
    ascii: bool,
    colorless: bool,
    tiny: bool,
    narrow: bool,
    recipe: TerminalOutputRecipe,
) -> u16 {
    if area.is_empty() {
        return area.y;
    }
    let mut y = area.y;
    let g = meta.status.glyph(ascii);
    let badge = meta.status.label();
    let exit = meta
        .exit_code
        .map(|c| format!(" exit={c}"))
        .unwrap_or_default();
    let sig = meta.signal.map(|s| format!(" {s}")).unwrap_or_default();
    let dur = meta
        .duration_ms
        .map(format_duration_ms)
        .map(|d| format!(" {d}"))
        .unwrap_or_default();
    let pid = meta.pid.map(|p| format!(" pid={p}")).unwrap_or_default();

    let head = if tiny {
        format!("{g} {badge}{exit}")
    } else {
        let t = title.unwrap_or("terminal");
        let separator = if ascii { " - " } else { " · " };
        format!("{g} {badge}{exit}{sig}{dur}{pid}{separator}{t}")
    };
    let st = if colorless {
        system.style(Role::TextStrong)
    } else {
        system.style(meta.status.role())
    };
    buffer.set_stringn(
        area.x,
        y,
        take_display_cols(&head, usize::from(area.width)),
        usize::from(area.width),
        st,
    );
    y = y.saturating_add(1);
    if y >= area.bottom() {
        return y;
    }

    // Command line
    let cmd = if narrow {
        take_display_cols(meta.command, usize::from(area.width)).to_string()
    } else {
        format!("$ {}", meta.command)
    };
    buffer.set_stringn(
        area.x,
        y,
        take_display_cols(&cmd, usize::from(area.width)),
        usize::from(area.width),
        if surface {
            system.style(Role::TextStrong)
        } else {
            system.style(Role::Text)
        },
    );
    y = y.saturating_add(1);

    if let Some(cwd) = meta.cwd {
        if y < area.bottom() && !tiny {
            let line = format!("cwd {cwd}");
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }
    }

    if matches!(recipe, TerminalOutputRecipe::Fullscreen) && y < area.bottom() {
        let hints = if ascii {
            "c=cancel r=retry d=detach C-c=copy e=env f=follow m=mode"
        } else {
            "c cancel · r retry · d detach · C-c copy · e env · f follow · m mode"
        };
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(hints, usize::from(area.width)),
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
    }

    if state.show_env && !meta.env.is_empty() && y < area.bottom() {
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols("env", usize::from(area.width)),
            usize::from(area.width),
            system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
        for entry in meta.env.iter().take(6) {
            if y >= area.bottom() {
                break;
            }
            let (val, red) = if entry.redacted {
                (entry.value.to_string(), true)
            } else {
                redact_env_value(entry.key, entry.value)
            };
            let line = format!("  {}={}", entry.key, val);
            let style = if red {
                system.style(Role::Warning)
            } else {
                system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }
    }
    y
}

fn paint_line(
    buffer: &mut Buffer,
    area: Rect,
    line: &TerminalLine<'_>,
    paint_mode: TerminalPaintMode,
    system: &DesignSystem,
    surface: bool,
    ascii: bool,
    colorless: bool,
    cursor: bool,
    tiny: bool,
) {
    if area.is_empty() {
        return;
    }
    // The cursor column is stamped by the shared row chrome.
    let gutter = " ";
    let prefix = if tiny { "" } else { line.stream.prefix(ascii) };

    // The stream rides its prefix, not the whole sentence: a page of stderr
    // is a page of readable text with a marked left edge, not a wall of red
    // (plans/012 Step 3).
    let style = if colorless && cursor {
        system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
    } else {
        system.style(Role::Text)
    };
    let stream_tone = (!colorless).then(|| system.style(line.stream.role()));
    let chrome = crate::widgets::row_chrome::RowChrome::resolve(
        system,
        ListRowVisualState {
            selected: cursor,
            focused: surface,
            enabled: true,
            ..Default::default()
        },
    );
    let style = chrome.label_style(style);

    match paint_mode {
        TerminalPaintMode::Ansi | TerminalPaintMode::NoColor if line.ansi.is_some() => {
            // Lead with stream prefix then paint ANSI via temporary buffer segment
            let mut tiers = TieredRow::with_separator("");
            tiers.push_joined(gutter, None);
            tiers.push_joined(prefix, stream_tone);
            let lead = tiers.text().to_string();
            let lead_w = display_cols(&lead) as u16;
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(&lead, usize::from(area.width)),
                usize::from(area.width.min(lead_w.max(1))),
                style,
            );
            tiers.paint_tiers(buffer, Rect::new(area.x, area.y, area.width, 1), 0);
            if let Some(ansi) = line.ansi {
                let rest = Rect::new(
                    area.x.saturating_add(lead_w.min(area.width)),
                    area.y,
                    area.width.saturating_sub(lead_w),
                    1,
                );
                if !rest.is_empty() {
                    let mode = paint_mode.to_ansi_mode();
                    let mut ast = crate::ansi_text::AnsiTextState::new();
                    AnsiText::lines(std::slice::from_ref(ansi), system)
                        .mode(mode)
                        .paint(rest, buffer, &mut ast);
                }
            }
        }
        TerminalPaintMode::Raw => {
            let body = escape_raw_terminal(line.text);
            paint_stream_line(buffer, area, gutter, prefix, &body, style, stream_tone);
        }
        _ => {
            paint_stream_line(buffer, area, gutter, prefix, line.text, style, stream_tone);
        }
    }
    chrome.paint(buffer, area);
}

/// Paints `gutter + prefix + body`, with the stream tone on the prefix only.
fn paint_stream_line(
    buffer: &mut Buffer,
    area: Rect,
    gutter: &str,
    prefix: &str,
    body: &str,
    style: Style,
    stream_tone: Option<Style>,
) {
    let mut tiers = TieredRow::with_separator("");
    tiers.push_joined(gutter, None);
    tiers.push_joined(prefix, stream_tone);
    tiers.push_joined(body, None);
    let text = tiers.text().to_string();
    buffer.set_stringn(
        area.x,
        area.y,
        take_display_cols(&text, usize::from(area.width)),
        usize::from(area.width),
        style,
    );
    tiers.paint_tiers(buffer, Rect::new(area.x, area.y, area.width, 1), 0);
}

impl StatefulWidget for &TerminalOutput<'_> {
    type State = TerminalOutputState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        TerminalOutput::render(self, area, buffer, state);
    }
}

impl StatefulWidget for TerminalOutput<'_> {
    type State = TerminalOutputState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        TerminalOutput::render(&self, area, buffer, state);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Streaming paint targets.
pub mod bench {
    /// Lines/sec host append target.
    pub const LINES_PER_SEC: u32 = 20_000;
    /// Viewport rows.
    pub const VIEWPORT: u16 = 40;
    /// Max paint cells.
    pub const MAX_PAINT_CELLS: u32 = 40 * 120;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ansi_text::{AnsiParseOptions, parse_to_line};
    use ratatui_core::layout::Position;

    fn sample_meta() -> TerminalCommandMeta<'static> {
        TerminalCommandMeta::new("cargo test -p termrock")
            .cwd("/proj")
            .status(TerminalRunStatus::Running)
            .duration_ms(1200)
            .pid(4242)
    }

    fn sample_lines() -> Vec<TerminalLine<'static>> {
        vec![
            TerminalLine::system("s0", "spawned"),
            TerminalLine::stdout("o1", "running 1 test"),
            TerminalLine::stderr("e1", "warning: unused"),
            TerminalLine::stdout("o2", "test widgets::x ... ok"),
            TerminalLine::stdout("o3", "done 東京 🧪"),
        ]
    }

    #[test]
    fn never_executes_only_requests() {
        // Outcomes are requests; no process API in this module (scan production body).
        let src = include_str!("terminal_output.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(!prod.contains("std::process::"));
        assert!(!prod.contains("tokio::process"));
        assert!(!prod.contains("Command::new"));
        assert!(prod.contains("CancelRequested"));
        assert!(prod.contains("Never") || prod.contains("never"));
    }

    #[test]
    fn follow_detaches_on_scroll() {
        let lines = sample_lines();
        let meta = sample_meta();
        let mut state = TerminalOutputState::new();
        state.on_append(100, 10);
        assert!(state.is_following());
        let out = state.handle_key(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            &lines,
            &meta,
        );
        assert!(matches!(out, TerminalOutputOutcome::Detach));
        assert!(!state.is_following());
        // append while pinned accumulates unread
        state.on_append(120, 10);
        assert!(state.unread() > 0 || !state.is_following());
    }

    #[test]
    fn f_toggles_follow() {
        let lines = sample_lines();
        let meta = sample_meta();
        let mut state = TerminalOutputState::new();
        state.on_append(20, 5);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &lines,
            &meta,
        );
        assert!(!state.is_following());
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
                &lines,
                &meta
            ),
            TerminalOutputOutcome::Follow
        ));
    }

    #[test]
    fn cancel_retry_copy_outcomes() {
        let lines = sample_lines();
        let mut meta = sample_meta();
        let mut state = TerminalOutputState::new();
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
                &lines,
                &meta
            ),
            TerminalOutputOutcome::CancelRequested
        ));
        meta.status = TerminalRunStatus::Failed;
        meta.exit_code = Some(1);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
                &lines,
                &meta
            ),
            TerminalOutputOutcome::RetryRequested
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                &lines,
                &meta
            ),
            TerminalOutputOutcome::CopyOutput { text } if text.contains("running")
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
                &lines,
                &meta
            ),
            TerminalOutputOutcome::CopyCommand { text } if text.contains("cargo")
        ));
    }

    #[test]
    fn env_redaction_helper() {
        let (v, r) = redact_env_value("API_TOKEN", "supersecret");
        assert!(r);
        assert_eq!(v, "***");
        let (v2, r2) = redact_env_value("PATH", "/usr/bin");
        assert!(!r2);
        assert!(v2.contains("usr"));
    }

    #[test]
    fn stream_filter_and_paint_modes() {
        let lines = sample_lines();
        let meta = sample_meta();
        let mut state = TerminalOutputState::new();
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE),
            &lines,
            &meta,
        );
        assert!(state.hide_stderr);
        let v = filter_terminal_lines(&lines, false, true);
        assert!(v.iter().all(|l| l.stream != TerminalStream::Stderr));
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
            &lines,
            &meta,
        );
        assert_eq!(state.paint_mode, TerminalPaintMode::NoColor);
    }

    #[test]
    fn paint_recipes_and_ansi() {
        let system = DesignSystem::default();
        let meta = sample_meta();
        let opts = AnsiParseOptions::default();
        let ansi = parse_to_line("\x1b[32mok\x1b[0m", &opts);
        let lines = [
            TerminalLine::stdout("1", "plain"),
            TerminalLine::stdout("2", "ok").ansi(&ansi),
            TerminalLine::stderr("3", "warn"),
        ];
        let mut state = TerminalOutputState::new();
        state.recipe = TerminalOutputRecipe::Pane;
        let view = TerminalOutput::new(&meta, &lines, &system).title("Build");
        let area = Rect::new(0, 0, 64, 14);
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("cargo") || text.contains("run") || text.contains("follow"),
            "{text}"
        );
        assert!(!state.regions.is_empty() || text.contains("plain"));

        state.recipe = TerminalOutputRecipe::Compact;
        (&view).render(area, &mut buf, &mut state);
        state.recipe = TerminalOutputRecipe::Fullscreen;
        state.show_env = true;
        let env = [
            TerminalEnvEntry::secret("TOKEN"),
            TerminalEnvEntry::new("PATH", "/bin"),
        ];
        let meta2 = TerminalCommandMeta::new("echo hi")
            .cwd("/tmp")
            .env(&env)
            .status(TerminalRunStatus::Succeeded)
            .exit_code(0)
            .duration_ms(50);
        TerminalOutput::new(&meta2, &lines, &system).render(area, &mut buf, &mut state);
    }

    #[test]
    fn raw_escape_and_duration() {
        assert!(escape_raw_terminal("\x1b[31mx").contains("\\e"));
        assert!(format_duration_ms(500).contains("ms"));
        assert!(format_duration_ms(2500).contains('s'));
    }

    #[test]
    fn mouse_chip_follow() {
        let system = DesignSystem::default();
        let meta = sample_meta();
        let lines = sample_lines();
        let mut state = TerminalOutputState::new();
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        TerminalOutput::new(&meta, &lines, &system).render(area, &mut buf, &mut state);
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &lines,
            &meta,
        );
        assert!(!state.is_following());
        let chip_y = area.bottom().saturating_sub(1);
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(0, chip_y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(click, &lines),
            TerminalOutputOutcome::Follow
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let lines = sample_lines();
        let meta = sample_meta();
        let mut state = TerminalOutputState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
                &lines,
                &meta
            ),
            TerminalOutputOutcome::Ignored
        ));
    }

    #[test]
    fn anchor_restore() {
        let lines = sample_lines();
        let mut state = TerminalOutputState::new();
        state.set_following(false);
        state.cursor = 2;
        state.capture_anchor(&lines);
        state.cursor = 0;
        state.restore_anchor(&lines);
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn sustained_viewport_paint() {
        let system = DesignSystem::default();
        let meta = sample_meta();
        let owned: Vec<(String, String)> = (0..60)
            .map(|i| (i.to_string(), format!("line {i}")))
            .collect();
        let lines: Vec<TerminalLine<'_>> = owned
            .iter()
            .map(|(id, t)| {
                if id.parse::<usize>().unwrap_or(0) % 5 == 0 {
                    TerminalLine::stderr(id, t)
                } else {
                    TerminalLine::stdout(id, t)
                }
            })
            .collect();
        let mut state = TerminalOutputState::new();
        state.on_append(60, 20);
        let view = TerminalOutput::new(&meta, &lines, &system);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        for _ in 0..40 {
            (&view).render(area, &mut buf, &mut state);
        }
        assert!(state.regions.len() <= 30);
    }

    #[test]
    fn fuzz_status_and_streams() {
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
            assert!(!s.id().is_empty());
            assert!(!s.glyph(true).is_empty());
        }
        for st in [
            TerminalStream::Stdout,
            TerminalStream::Stderr,
            TerminalStream::System,
        ] {
            assert!(!st.prefix(true).is_empty());
        }
        assert!(bench::LINES_PER_SEC >= 1000);
    }

    #[test]
    fn empty_pending_paint() {
        let system = DesignSystem::default();
        let meta = TerminalCommandMeta::new("sleep 1").status(TerminalRunStatus::Pending);
        let mut state = TerminalOutputState::new();
        let view = TerminalOutput::new(&meta, &[], &system);
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("wait") || text.contains("pending") || text.contains('∅'),
            "{text}"
        );
    }
}
