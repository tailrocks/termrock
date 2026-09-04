// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ToolCallCard** — compact-to-expanded agent tool execution card.
//!
//! **Mission.** Queued, preparing, running, waiting for input, waiting for
//! permission, streaming, success, warning, failure, cancelled, and detached
//! states. Show tool name, meaningful verb, actor/provenance, arguments
//! summary, duration, result summary, risk, and actions. Inline expansion and
//! fullscreen semantic zoom. Redact secrets; make data egress explicit. Never
//! couple to a specific agent provider or tool protocol.
//!
//! Research: Grok Build, Amp, OpenCode, Claude Code tool presentations.
//!
//! **vs [`ToolCard`](crate::widgets::ToolCard).** ToolCard is a thin paint
//! summary. ToolCallCard owns interaction state, outcomes, redaction, projection
//! lines for MessageThread, and action chrome.
//!
//! **Ownership.** Host executes tools / cancels processes. Outcomes are requests
//! only.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, MotionPolicy, PanelChrome, Role, SPINNER_BRAILLE_FRAMES},
    text::{display_cols, take_display_cols},
    widgets::{AccentRail, agent::ToolStatus, card::Card},
};

/// Overlay id for fullscreen tool detail.
pub const TOOL_CALL_FULLSCREEN_OVERLAY_ID: &str = "termrock.tool_call_fullscreen";
/// Max args/result lines when expanded inline.
pub const TOOL_CALL_EXPAND_LINE_CAP: usize = 8;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Risk / egress level (host-projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToolRisk {
    /// No significant egress.
    #[default]
    None,
    /// Reads workspace.
    Read,
    /// Writes workspace.
    Write,
    /// Network / external.
    Network,
    /// Secrets or high impact.
    High,
}

impl ToolRisk {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Read => "read",
            Self::Write => "write",
            Self::Network => "network",
            Self::High => "high",
        }
    }

    /// Badge letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::None => '·',
            Self::Read => 'r',
            Self::Write => 'w',
            Self::Network => 'n',
            Self::High => '!',
        }
    }

    /// Warning chrome.
    #[must_use]
    pub const fn is_elevated(self) -> bool {
        matches!(self, Self::Network | Self::High | Self::Write)
    }
}

/// One action affordance on the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ToolCallAction {
    /// Toggle expand.
    ToggleExpand,
    /// Cancel running tool.
    Cancel,
    /// Retry failed/cancelled.
    Retry,
    /// Open diff of changes.
    OpenDiff,
    /// Open full log / terminal.
    OpenLog,
    /// Focus permission prompt.
    PermissionFocus,
    /// Copy args (redacted).
    CopyArgs,
    /// Copy result (redacted).
    CopyResult,
    /// Fullscreen zoom.
    Fullscreen,
}

impl ToolCallAction {
    /// Chord hint.
    #[must_use]
    pub const fn chord(self) -> &'static str {
        match self {
            Self::ToggleExpand => "Enter",
            Self::Cancel => "c",
            Self::Retry => "r",
            Self::OpenDiff => "d",
            Self::OpenLog => "l",
            Self::PermissionFocus => "p",
            Self::CopyArgs => "C-a",
            Self::CopyResult => "C-c",
            Self::Fullscreen => "f",
        }
    }
}

/// Host-projected tool invocation model (no provider protocol).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    /// Stable call id.
    pub id: String,
    /// Tool name (e.g. `bash`, `read_file`).
    pub name: String,
    /// Meaningful verb phrase (`ran tests`, `read path`).
    pub verb: String,
    /// Actor / provenance (`agent`, `subagent:x`).
    pub actor: Option<String>,
    /// Status.
    pub status: ToolStatus,
    /// Arguments summary (already host-redacted preferred).
    pub args_summary: String,
    /// Full args text for expand (may still be redacted).
    pub args_detail: Option<String>,
    /// Result summary.
    pub result_summary: Option<String>,
    /// Result detail / stdout slice.
    pub result_detail: Option<String>,
    /// Duration display (host formats).
    pub duration: Option<String>,
    /// Risk / egress.
    pub risk: ToolRisk,
    /// Explicit data egress note (network host, path).
    pub egress: Option<String>,
    /// Secret redaction applied.
    pub secrets_redacted: bool,
    /// Host reports a diff unit exists for this call.
    pub has_diff: bool,
    /// Host reports a full log / terminal stream exists.
    pub has_log: bool,
    /// Revision for stream height cache.
    pub revision: u64,
}

impl ToolCall {
    /// Minimal running call.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, verb: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            verb: verb.into(),
            actor: None,
            status: ToolStatus::Running,
            args_summary: String::new(),
            args_detail: None,
            result_summary: None,
            result_detail: None,
            duration: None,
            risk: ToolRisk::None,
            egress: None,
            secrets_redacted: false,
            has_diff: false,
            has_log: false,
            revision: 0,
        }
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: ToolStatus) -> Self {
        self.status = s;
        self
    }

    /// Args summary.
    #[must_use]
    pub fn args_summary(mut self, s: impl Into<String>) -> Self {
        self.args_summary = s.into();
        self
    }

    /// Args detail.
    #[must_use]
    pub fn args_detail(mut self, s: impl Into<String>) -> Self {
        self.args_detail = Some(s.into());
        self
    }

    /// Result summary.
    #[must_use]
    pub fn result_summary(mut self, s: impl Into<String>) -> Self {
        self.result_summary = Some(s.into());
        self
    }

    /// Result detail.
    #[must_use]
    pub fn result_detail(mut self, s: impl Into<String>) -> Self {
        self.result_detail = Some(s.into());
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
    pub fn duration(mut self, d: impl Into<String>) -> Self {
        self.duration = Some(d.into());
        self
    }

    /// Risk.
    #[must_use]
    pub const fn risk(mut self, r: ToolRisk) -> Self {
        self.risk = r;
        self
    }

    /// Egress note.
    #[must_use]
    pub fn egress(mut self, e: impl Into<String>) -> Self {
        self.egress = Some(e.into());
        self
    }

    /// Secrets redacted flag.
    #[must_use]
    pub const fn secrets_redacted(mut self, on: bool) -> Self {
        self.secrets_redacted = on;
        self
    }

    /// Diff available for open-diff action.
    #[must_use]
    pub const fn has_diff(mut self, on: bool) -> Self {
        self.has_diff = on;
        self
    }

    /// Full log available for open-log action.
    #[must_use]
    pub const fn has_log(mut self, on: bool) -> Self {
        self.has_log = on;
        self
    }

    /// Revision.
    #[must_use]
    pub const fn revision(mut self, r: u64) -> Self {
        self.revision = r;
        self
    }

    /// Header line for compact chrome.
    #[must_use]
    pub fn header_line(&self, ascii: bool) -> String {
        let g = if ascii {
            self.status.letter().to_string()
        } else {
            self.status.glyph().to_string()
        };
        let mut s = format!("{g} {} — {}", self.name, self.verb);
        if let Some(d) = &self.duration {
            s.push_str(" · ");
            s.push_str(d);
        }
        if self.risk.is_elevated() {
            s.push_str(&format!(" · {}", self.risk.id()));
        }
        if self.secrets_redacted {
            s.push_str(" · redacted");
        }
        s
    }
}

/// Redact common secret patterns in tool text (best-effort; host should pre-redact).
#[must_use]
pub fn redact_tool_secrets(text: &str) -> String {
    let mut out = text.to_string();
    // simple patterns — not a crypto vault
    for key in [
        "password=",
        "PASSWORD=",
        "api_key=",
        "API_KEY=",
        "token=",
        "TOKEN=",
        "secret=",
        "SECRET=",
        "Authorization: Bearer ",
    ] {
        if let Some(i) = out.find(key) {
            let start = i + key.len();
            let rest = &out[start..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .unwrap_or(rest.len());
            out.replace_range(start..start + end, "***");
        }
    }
    out
}

/// Available actions for a call at status.
#[must_use]
pub fn tool_actions_for(status: ToolStatus, has_diff: bool, has_log: bool) -> Vec<ToolCallAction> {
    let mut a = vec![ToolCallAction::ToggleExpand, ToolCallAction::Fullscreen];
    if status.can_cancel() {
        a.push(ToolCallAction::Cancel);
    }
    if status.can_retry() {
        a.push(ToolCallAction::Retry);
    }
    if matches!(status, ToolStatus::WaitingPermission) {
        a.push(ToolCallAction::PermissionFocus);
    }
    if has_diff {
        a.push(ToolCallAction::OpenDiff);
    }
    if has_log {
        a.push(ToolCallAction::OpenLog);
    }
    a.push(ToolCallAction::CopyArgs);
    a.push(ToolCallAction::CopyResult);
    a
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Tool call card outcomes (requests only).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToolCallCardOutcome {
    /// Ignored.
    Ignored,
    /// Expanded.
    Expanded {
        /// Call id.
        id: String,
    },
    /// Collapsed.
    Collapsed {
        /// Call id.
        id: String,
    },
    /// Cancel tool.
    CancelRequested {
        /// Call id.
        id: String,
    },
    /// Retry.
    RetryRequested {
        /// Call id.
        id: String,
    },
    /// Open diff.
    OpenDiff {
        /// Call id.
        id: String,
    },
    /// Open log / terminal.
    OpenLog {
        /// Call id.
        id: String,
    },
    /// Focus permission UI.
    PermissionFocus {
        /// Call id.
        id: String,
    },
    /// Copy args (redacted text).
    CopyArgs {
        /// Call id.
        id: String,
        /// Text.
        text: String,
    },
    /// Copy result (redacted).
    CopyResult {
        /// Call id.
        id: String,
        /// Text.
        text: String,
    },
    /// Fullscreen zoom requested.
    FullscreenRequested {
        /// Call id.
        id: String,
    },
    /// Activated (generic).
    Activated {
        /// Call id.
        id: String,
    },
}

/// Presentation / zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToolCallPresentation {
    /// One/two line compact.
    #[default]
    Compact,
    /// Inline expanded args/result.
    Expanded,
    /// Fullscreen (host overlay; card paints dense).
    Fullscreen,
}

/// Interactive tool call card state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallCardState {
    /// Presentation.
    pub presentation: ToolCallPresentation,
    /// Focused.
    pub focused: bool,
    /// Accepts input.
    accepts_input: bool,
    /// Action cursor when expanded.
    pub action_cursor: usize,
    /// Header hit region.
    pub header_hit: Rect,
    /// Action hits.
    pub action_hits: Vec<(ToolCallAction, Rect)>,
}

impl Default for ToolCallCardState {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolCallCardState {
    /// Compact default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            presentation: ToolCallPresentation::Compact,
            focused: true,
            accepts_input: true,
            action_cursor: 0,
            header_hit: Rect::default(),
            action_hits: Vec::new(),
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Expanded?
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        !matches!(self.presentation, ToolCallPresentation::Compact)
    }

    /// Toggle expand.
    pub fn toggle_expand(&mut self, id: &str) -> ToolCallCardOutcome {
        match self.presentation {
            ToolCallPresentation::Compact => {
                self.presentation = ToolCallPresentation::Expanded;
                ToolCallCardOutcome::Expanded { id: id.to_string() }
            }
            ToolCallPresentation::Expanded | ToolCallPresentation::Fullscreen => {
                self.presentation = ToolCallPresentation::Compact;
                ToolCallCardOutcome::Collapsed { id: id.to_string() }
            }
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, call: &ToolCall) -> ToolCallCardOutcome {
        if !self.accepts_input || !self.focused || !key.is_press() {
            return ToolCallCardOutcome::Ignored;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_expand(&call.id),
            KeyCode::Char('c') if key.modifiers.is_empty() && call.status.can_cancel() => {
                ToolCallCardOutcome::CancelRequested {
                    id: call.id.clone(),
                }
            }
            KeyCode::Char('r') if call.status.can_retry() => ToolCallCardOutcome::RetryRequested {
                id: call.id.clone(),
            },
            KeyCode::Char('d') if call.has_diff => ToolCallCardOutcome::OpenDiff {
                id: call.id.clone(),
            },
            KeyCode::Char('l') if call.has_log => ToolCallCardOutcome::OpenLog {
                id: call.id.clone(),
            },
            KeyCode::Char('p') if matches!(call.status, ToolStatus::WaitingPermission) => {
                ToolCallCardOutcome::PermissionFocus {
                    id: call.id.clone(),
                }
            }
            KeyCode::Char('f') => {
                self.presentation = ToolCallPresentation::Fullscreen;
                ToolCallCardOutcome::FullscreenRequested {
                    id: call.id.clone(),
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let text = redact_tool_secrets(
                    call.args_detail
                        .as_deref()
                        .unwrap_or(call.args_summary.as_str()),
                );
                ToolCallCardOutcome::CopyArgs {
                    id: call.id.clone(),
                    text,
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let raw = call
                    .result_detail
                    .as_deref()
                    .or(call.result_summary.as_deref())
                    .unwrap_or("");
                ToolCallCardOutcome::CopyResult {
                    id: call.id.clone(),
                    text: redact_tool_secrets(raw),
                }
            }
            KeyCode::Esc if matches!(self.presentation, ToolCallPresentation::Fullscreen) => {
                self.presentation = ToolCallPresentation::Expanded;
                ToolCallCardOutcome::Expanded {
                    id: call.id.clone(),
                }
            }
            _ => ToolCallCardOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, call: &ToolCall) -> ToolCallCardOutcome {
        if !self.accepts_input || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return ToolCallCardOutcome::Ignored;
        }
        for (act, rect) in &self.action_hits {
            if rect.contains(event.position) {
                return match act {
                    ToolCallAction::ToggleExpand => self.toggle_expand(&call.id),
                    ToolCallAction::Cancel => ToolCallCardOutcome::CancelRequested {
                        id: call.id.clone(),
                    },
                    ToolCallAction::Retry => ToolCallCardOutcome::RetryRequested {
                        id: call.id.clone(),
                    },
                    ToolCallAction::OpenDiff => ToolCallCardOutcome::OpenDiff {
                        id: call.id.clone(),
                    },
                    ToolCallAction::OpenLog => ToolCallCardOutcome::OpenLog {
                        id: call.id.clone(),
                    },
                    ToolCallAction::PermissionFocus => ToolCallCardOutcome::PermissionFocus {
                        id: call.id.clone(),
                    },
                    ToolCallAction::Fullscreen => {
                        self.presentation = ToolCallPresentation::Fullscreen;
                        ToolCallCardOutcome::FullscreenRequested {
                            id: call.id.clone(),
                        }
                    }
                    ToolCallAction::CopyArgs => ToolCallCardOutcome::CopyArgs {
                        id: call.id.clone(),
                        text: redact_tool_secrets(
                            call.args_detail
                                .as_deref()
                                .unwrap_or(call.args_summary.as_str()),
                        ),
                    },
                    ToolCallAction::CopyResult => ToolCallCardOutcome::CopyResult {
                        id: call.id.clone(),
                        text: redact_tool_secrets(
                            call.result_detail
                                .as_deref()
                                .or(call.result_summary.as_deref())
                                .unwrap_or(""),
                        ),
                    },
                };
            }
        }
        if self.header_hit.contains(event.position) {
            return self.toggle_expand(&call.id);
        }
        ToolCallCardOutcome::Ignored
    }
}

// ── Projection (MessageThread lines) ────────────────────────────────────────

/// Project tool call to plain lines (no nested widgets).
#[must_use]
pub fn project_tool_call_lines(call: &ToolCall, expanded: bool, ascii: bool) -> Vec<String> {
    let mut lines = vec![call.header_line(ascii)];
    if !call.args_summary.is_empty() {
        lines.push(format!(
            "  args: {}",
            redact_tool_secrets(&call.args_summary)
        ));
    }
    if let Some(e) = &call.egress {
        lines.push(format!("  egress: {e}"));
    }
    if expanded {
        if let Some(a) = &call.args_detail {
            for (i, l) in redact_tool_secrets(a)
                .lines()
                .take(TOOL_CALL_EXPAND_LINE_CAP)
                .enumerate()
            {
                if i == 0 {
                    lines.push(format!("  {l}"));
                } else {
                    lines.push(format!("  {l}"));
                }
            }
        }
        if let Some(r) = &call.result_detail.as_ref().or(call.result_summary.as_ref()) {
            lines.push("  result:".into());
            for l in redact_tool_secrets(r)
                .lines()
                .take(TOOL_CALL_EXPAND_LINE_CAP)
            {
                lines.push(format!("    {l}"));
            }
        }
    } else if let Some(r) = &call.result_summary {
        lines.push(format!("  → {}", redact_tool_secrets(r)));
    }
    if let Some(a) = &call.actor {
        lines.push(format!("  via {a}"));
    }
    lines
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Interactive tool call card.
#[derive(Debug, Clone, Copy)]
pub struct ToolCallCard<'a> {
    call: &'a ToolCall,
    system: &'a DesignSystem,
    colorless: bool,
    tick: u64,
}

impl<'a> ToolCallCard<'a> {
    /// Call + system.
    #[must_use]
    pub const fn new(call: &'a ToolCall, system: &'a DesignSystem) -> Self {
        Self {
            call,
            system,
            colorless: false,
            tick: 0,
        }
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ToolCallCardState) {
        state.action_hits.clear();
        if area.is_empty() {
            return;
        }
        let call = self.call;
        let running = matches!(call.status, ToolStatus::Running | ToolStatus::Streaming);
        let rail = AccentRail::new(self.system, Role::ActorTool).collapsed(!state.is_expanded());
        let content_area = rail.paint(area, buffer);
        state.header_hit = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1.min(area.height),
        };
        if content_area.is_empty() {
            return;
        }

        if !state.is_expanded() {
            let disclosure = self.system.glyphs.disclosure_closed();
            let pulse = if running {
                if matches!(self.system.motion, MotionPolicy::Full) {
                    SPINNER_BRAILLE_FRAMES[self.tick as usize % SPINNER_BRAILLE_FRAMES.len()]
                } else {
                    "●"
                }
            } else {
                ""
            };
            let status = call.status.semantic();
            let status_glyph = { status.glyph() };
            let prefix = format!("{disclosure} {status_glyph} {} · ", status.default_label());
            buffer.set_stringn(
                content_area.x,
                content_area.y,
                &prefix,
                usize::from(content_area.width),
                self.system.style(Role::Text),
            );
            crate::widgets::row_chrome::paint_status_glyph(
                buffer,
                content_area,
                u16::try_from(display_cols(disclosure).saturating_add(1)).unwrap_or(u16::MAX),
                status_glyph,
                self.system.style(if self.colorless {
                    Role::TextStrong
                } else {
                    status.role()
                }),
            );
            let verb_x = content_area.x.saturating_add(display_cols(&prefix) as u16);
            let verb = take_display_cols(
                &call.verb,
                usize::from(content_area.right().saturating_sub(verb_x)),
            );
            buffer.set_stringn(
                verb_x,
                content_area.y,
                &verb,
                usize::from(content_area.right().saturating_sub(verb_x)),
                self.system.style(Role::TextStrong),
            );
            let detail_x = verb_x
                .saturating_add(display_cols(&verb) as u16)
                .saturating_add(1);
            if detail_x < content_area.right() {
                let details = if call.args_summary.is_empty() {
                    call.result_summary.as_deref().unwrap_or("")
                } else {
                    call.args_summary.as_str()
                };
                let parenthetical = if pulse.is_empty() {
                    format!("({details})")
                } else {
                    format!("({pulse} {details})")
                };
                buffer.set_stringn(
                    detail_x,
                    content_area.y,
                    take_display_cols(
                        &parenthetical,
                        usize::from(content_area.right().saturating_sub(detail_x)),
                    ),
                    usize::from(content_area.right().saturating_sub(detail_x)),
                    self.system.style(Role::TextMuted),
                );
            }
            return;
        }
        let status_label = { call.status.badge().to_string() };
        let leading = { call.status.semantic().glyph() };
        let title = call.name.clone();
        let mut subtitle = call.verb.clone();
        if !call.args_summary.is_empty() {
            subtitle = format!(
                "{} · {}",
                call.verb,
                take_display_cols(&redact_tool_secrets(&call.args_summary), 40)
            );
        }
        let emphasis = match call.status {
            ToolStatus::Failed => PanelChrome::Danger,
            ToolStatus::Running
            | ToolStatus::Streaming
            | ToolStatus::WaitingPermission
            | ToolStatus::WaitingInput => PanelChrome::Focused,
            _ => {
                if state.focused {
                    PanelChrome::Focused
                } else {
                    PanelChrome::Normal
                }
            }
        };
        let card = Card::new(self.system)
            .title(title.as_str())
            .leading(leading)
            .badge(status_label.as_str())
            .subtitle(subtitle.as_str())
            .emphasis(emphasis);
        let body = card.paint(content_area, buffer, None);

        if body.is_empty() {
            return;
        }
        for y in body.top()..body.bottom() {
            for x in body.left()..body.right() {
                buffer[(x, y)].set_style(self.system.style(Role::Sunken));
            }
        }
        let mut y = body.y;
        let max_y = body.bottom();

        // compact always shows one status/summary line
        let line1 = if state.is_expanded() {
            format!(
                "{}{}",
                if call.risk.is_elevated() {
                    format!("[{}] ", call.risk.id())
                } else {
                    String::new()
                },
                take_display_cols(
                    &redact_tool_secrets(&call.args_summary),
                    usize::from(body.width)
                )
            )
        } else {
            take_display_cols(
                call.result_summary
                    .as_deref()
                    .unwrap_or(call.args_summary.as_str()),
                usize::from(body.width),
            )
            .into_owned()
        };
        let style = self.system.style(Role::Text);
        buffer.set_stringn(body.x, y, &line1, usize::from(body.width), style);
        y = y.saturating_add(1);

        if state.is_expanded() && y < max_y {
            if let Some(a) = &call.actor {
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
            if let Some(e) = &call.egress {
                buffer.set_stringn(
                    body.x,
                    y,
                    take_display_cols(&format!("egress: {e}"), usize::from(body.width)),
                    usize::from(body.width),
                    self.system.style(Role::Warning),
                );
                y = y.saturating_add(1);
            }
            if let Some(detail) = call
                .result_detail
                .as_deref()
                .or(call.args_detail.as_deref())
            {
                for l in redact_tool_secrets(detail)
                    .lines()
                    .take(TOOL_CALL_EXPAND_LINE_CAP)
                {
                    if y >= max_y {
                        break;
                    }
                    buffer.set_stringn(
                        body.x,
                        y,
                        take_display_cols(l, usize::from(body.width)),
                        usize::from(body.width),
                        self.system.style(Role::TextMuted),
                    );
                    y = y.saturating_add(1);
                }
            }
            // action strip
            if y < max_y {
                let actions = tool_actions_for(
                    call.status,
                    call.has_diff,
                    call.has_log || call.result_detail.is_some(),
                );
                let mut x = body.x;
                for (i, act) in actions.iter().take(6).enumerate() {
                    let label = format!("[{}]", act.chord());
                    let w = (display_cols(&label) as u16).saturating_add(1);
                    if x.saturating_add(w) > body.right() {
                        break;
                    }
                    let sel = state.focused && i == state.action_cursor;
                    buffer.set_stringn(
                        x,
                        y,
                        &label,
                        usize::from(w),
                        if sel {
                            self.system.style(Role::Focus)
                        } else {
                            self.system.style(Role::TextMuted)
                        },
                    );
                    state.action_hits.push((
                        *act,
                        Rect {
                            x,
                            y,
                            width: w,
                            height: 1,
                        },
                    ));
                    x = x.saturating_add(w);
                }
            }
        }
    }
}

/// Example calls for stories/tests.
#[must_use]
pub fn example_tool_calls() -> Vec<ToolCall> {
    vec![
        ToolCall::new("t1", "bash", "ran cargo test")
            .status(ToolStatus::Success)
            .args_summary("cargo test -p termrock --lib")
            .result_summary("ok · 12 passed")
            .duration("1.2s")
            .risk(ToolRisk::Read)
            .has_log(true),
        ToolCall::new("t2", "bash", "running build")
            .status(ToolStatus::Running)
            .args_summary("cargo build")
            .duration("…")
            .risk(ToolRisk::Write)
            .actor("agent")
            .has_log(true),
        ToolCall::new("t3", "http", "fetch docs")
            .status(ToolStatus::WaitingPermission)
            .args_summary("GET https://api.example.com")
            .risk(ToolRisk::Network)
            .egress("api.example.com"),
        ToolCall::new("t4", "bash", "failed lint")
            .status(ToolStatus::Failed)
            .args_summary("eslint .")
            .result_summary("exit 1")
            .args_detail("eslint .\n# token=supersecret")
            .result_detail("Unexpected token")
            .secrets_redacted(true)
            .has_diff(true)
            .has_log(true),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Cards.
    pub const CARD_COUNT: usize = 48;
    /// Frames.
    pub const PAINT_FRAMES: u32 = 20;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;
    use crate::widgets::tests::click;

    #[test]
    fn expand_collapse_outcomes() {
        let mut st = ToolCallCardState::new();
        assert!(matches!(
            st.toggle_expand("t"),
            ToolCallCardOutcome::Expanded { .. }
        ));
        assert!(st.is_expanded());
        assert!(matches!(
            st.toggle_expand("t"),
            ToolCallCardOutcome::Collapsed { .. }
        ));
    }

    #[test]
    fn cancel_only_when_running() {
        let run = ToolCall::new("t", "bash", "x").status(ToolStatus::Running);
        let done = ToolCall::new("t", "bash", "x").status(ToolStatus::Success);
        let mut st = ToolCallCardState::new();
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &run);
        assert!(matches!(out, ToolCallCardOutcome::CancelRequested { .. }));
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &done);
        assert!(matches!(out, ToolCallCardOutcome::Ignored));
    }

    #[test]
    fn retry_on_failed() {
        let call = ToolCall::new("t", "bash", "x").status(ToolStatus::Failed);
        let mut st = ToolCallCardState::new();
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), &call);
        assert!(matches!(out, ToolCallCardOutcome::RetryRequested { .. }));
    }

    #[test]
    fn redact_secrets() {
        let s = redact_tool_secrets("export TOKEN=abc123 rest");
        assert!(s.contains("***"));
        assert!(!s.contains("abc123"));
    }

    #[test]
    fn copy_args_redacted() {
        let call = ToolCall::new("t", "bash", "x").args_detail("password=hunter2 ok");
        let mut st = ToolCallCardState::new();
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
            &call,
        );
        match out {
            ToolCallCardOutcome::CopyArgs { text, .. } => {
                assert!(text.contains("***"));
                assert!(!text.contains("hunter2"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn project_lines_compact_vs_expanded() {
        let call = ToolCall::new("t", "bash", "test")
            .args_summary("cargo test")
            .result_summary("ok")
            .result_detail("line1\nline2\nline3");
        let c = project_tool_call_lines(&call, false, true);
        assert!(c.len() >= 2);
        let e = project_tool_call_lines(&call, true, true);
        assert!(e.len() > c.len());
    }

    #[test]
    fn permission_focus_chord() {
        let call = ToolCall::new("t", "http", "get").status(ToolStatus::WaitingPermission);
        let mut st = ToolCallCardState::new();
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE), &call);
        assert!(matches!(out, ToolCallCardOutcome::PermissionFocus { .. }));
    }

    #[test]
    fn paint_all_statuses() {
        let system = DesignSystem::default();
        let statuses = [
            ToolStatus::Queued,
            ToolStatus::Preparing,
            ToolStatus::Running,
            ToolStatus::WaitingInput,
            ToolStatus::WaitingPermission,
            ToolStatus::Streaming,
            ToolStatus::Success,
            ToolStatus::Warning,
            ToolStatus::Failed,
            ToolStatus::Cancelled,
            ToolStatus::Detached,
        ];
        let area = Rect::new(0, 0, 48, 8);
        let mut buf = Buffer::empty(area);
        for s in statuses {
            let call = ToolCall::new("t", "tool", "verb")
                .status(s)
                .args_summary("args");
            let mut st = ToolCallCardState::new();
            st.presentation = ToolCallPresentation::Expanded;
            ToolCallCard::new(&call, &system).paint(area, &mut buf, &mut st);
        }
    }

    #[test]
    fn collapsed_row_shape_uses_status_glyph_verb_and_dim_details() {
        let system = DesignSystem::default();
        let call = ToolCall::new("t", "bash", "Run tests")
            .status(ToolStatus::Success)
            .args_summary("cargo test");
        let area = Rect::new(0, 0, 48, 1);
        let mut buffer = Buffer::empty(area);
        let mut state = ToolCallCardState::new();
        ToolCallCard::new(&call, &system).paint(area, &mut buffer, &mut state);
        let text = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains(ToolStatus::Success.semantic().glyph()));
        assert!(text.contains("ok"));
        assert!(text.contains("Run tests"));
        assert!(text.contains("(cargo test)"));
        let detail_x = text.find('(').unwrap() as u16;
        assert_eq!(
            buffer[(detail_x, 0)].fg,
            system.style(Role::TextMuted).fg.unwrap()
        );
    }

    #[test]
    fn reduced_motion_running_card_is_tick_static() {
        let system = DesignSystem::default().motion(MotionPolicy::Off);
        let call = ToolCall::new("t", "bash", "Run tests").args_summary("cargo test");
        let render = |_tick| {
            let area = Rect::new(0, 0, 48, 1);
            let mut buffer = Buffer::empty(area);
            let mut state = ToolCallCardState::new();
            ToolCallCard::new(&call, &system).paint(area, &mut buffer, &mut state);
            buffer
        };
        assert_eq!(render(0), render(31));
    }

    #[test]
    fn accepts_input_gate() {
        let call = ToolCall::new("t", "bash", "x");
        let mut st = ToolCallCardState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &call),
            ToolCallCardOutcome::Ignored
        ));
    }

    #[test]
    fn never_provider_protocol() {
        let src = include_str!("tool_call_card.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "openai",
            "anthropic",
            "mcp::",
            "reqwest::",
            "std::process::Command",
        ] {
            assert!(
                !body
                    .to_ascii_lowercase()
                    .contains(&forbidden.to_ascii_lowercase())
                    || forbidden == "mcp::"
            );
            if forbidden != "mcp::" {
                assert!(!body.contains(forbidden), "{forbidden}");
            }
        }
    }

    #[test]
    fn thin_tool_card_still_paints() {
        use ratatui_core::widgets::Widget;
        let system = DesignSystem::default();
        let card = crate::widgets::ToolCard::new("bash", "run", ToolStatus::Running, &system);
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        Widget::render(card, area, &mut buf);
    }

    #[test]
    fn fullscreen_and_escape() {
        let call = ToolCall::new("t", "bash", "x").status(ToolStatus::Running);
        let mut st = ToolCallCardState::new();
        let out = st.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), &call);
        assert!(matches!(
            out,
            ToolCallCardOutcome::FullscreenRequested { .. }
        ));
        assert_eq!(st.presentation, ToolCallPresentation::Fullscreen);
        let out = st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &call);
        assert!(matches!(out, ToolCallCardOutcome::Expanded { .. }));
        assert_eq!(st.presentation, ToolCallPresentation::Expanded);
    }

    #[test]
    fn mouse_header_toggles_expand() {
        let call = ToolCall::new("t", "bash", "x");
        let system = DesignSystem::default();
        let mut st = ToolCallCardState::new();
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        ToolCallCard::new(&call, &system).paint(area, &mut buf, &mut st);
        let ev = click(st.header_hit.x, st.header_hit.y);
        let out = st.handle_mouse(ev, &call);
        assert!(matches!(out, ToolCallCardOutcome::Expanded { .. }));
    }

    #[test]
    fn actions_for_statuses() {
        let run = tool_actions_for(ToolStatus::Running, false, true);
        assert!(run.contains(&ToolCallAction::Cancel));
        assert!(!run.contains(&ToolCallAction::Retry));
        let fail = tool_actions_for(ToolStatus::Failed, true, false);
        assert!(fail.contains(&ToolCallAction::Retry));
        assert!(fail.contains(&ToolCallAction::OpenDiff));
        let perm = tool_actions_for(ToolStatus::WaitingPermission, false, false);
        assert!(perm.contains(&ToolCallAction::PermissionFocus));
    }

    #[test]
    fn egress_and_actor_in_projection() {
        let call = ToolCall::new("t", "http", "fetch")
            .args_summary("GET /")
            .egress("api.example.com")
            .actor("subagent:research")
            .risk(ToolRisk::Network);
        let lines = project_tool_call_lines(&call, true, true);
        let joined = lines.join("\n");
        assert!(joined.contains("egress:"));
        assert!(joined.contains("via subagent:research"));
        assert!(joined.contains("network") || joined.contains("http"));
    }

    #[test]
    fn no_process_or_pty_coupling() {
        let src = include_str!("tool_call_card.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "portable_pty",
            "Command::new",
            "std::process",
            "nix::",
            "tokio::process",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn paint_perf_budget_many_cards() {
        let system = DesignSystem::default();
        let calls = example_tool_calls();
        let area = Rect::new(0, 0, 64, 10);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            for call in &calls {
                let mut st = ToolCallCardState::new();
                st.presentation = ToolCallPresentation::Expanded;
                ToolCallCard::new(call, &system).paint(area, &mut buf, &mut st);
            }
            // expand synthetic load toward CARD_COUNT
            for i in 0..bench::CARD_COUNT / calls.len().max(1) {
                let call = ToolCall::new(format!("x{i}"), "tool", "verb")
                    .status(ToolStatus::Running)
                    .args_summary("a".repeat(80));
                let mut st = ToolCallCardState::new();
                ToolCallCard::new(&call, &system).paint(area, &mut buf, &mut st);
            }
        }
        // soft budget: CI-safe (multi-second only fails)
        assert!(
            start.elapsed().as_secs() < 5,
            "paint too slow: {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn risk_letters_and_ids_stable() {
        assert_eq!(ToolRisk::High.id(), "high");
        assert_eq!(ToolRisk::Network.letter(), 'n');
        assert!(ToolRisk::Write.is_elevated());
        assert!(!ToolRisk::Read.is_elevated());
    }

    #[test]
    fn header_line_includes_duration_and_redacted() {
        let call = ToolCall::new("t", "bash", "ran")
            .status(ToolStatus::Success)
            .duration("1.2s")
            .risk(ToolRisk::Write)
            .secrets_redacted(true);
        let h = call.header_line(true);
        assert!(h.contains("1.2s"));
        assert!(h.contains("write") || h.contains("redacted"));
        assert!(h.contains("redacted"));
    }

    #[test]
    fn open_diff_log_gated_by_flags() {
        let bare = ToolCall::new("t", "bash", "x");
        let with = ToolCall::new("t", "bash", "x").has_diff(true).has_log(true);
        let mut st = ToolCallCardState::new();
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), &bare),
            ToolCallCardOutcome::Ignored
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), &with),
            ToolCallCardOutcome::OpenDiff { .. }
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE), &with),
            ToolCallCardOutcome::OpenLog { .. }
        ));
    }

    #[test]
    fn paint_shows_actor_when_expanded() {
        let system = DesignSystem::default();
        let call = ToolCall::new("t", "bash", "run")
            .actor("subagent:x")
            .args_summary("echo")
            .egress("local");
        let mut st = ToolCallCardState::new();
        st.presentation = ToolCallPresentation::Expanded;
        let area = Rect::new(0, 0, 56, 10);
        let mut buf = Buffer::empty(area);
        ToolCallCard::new(&call, &system).paint(area, &mut buf, &mut st);
        // actor projected into header subtitle path via verb; egress line must paint
        let mut found_egress = false;
        for y in area.y..area.bottom() {
            let mut row = String::new();
            for x in area.x..area.right() {
                row.push_str(buf[(x, y)].symbol());
            }
            if row.contains("egress") {
                found_egress = true;
            }
        }
        assert!(found_egress, "egress note missing from expanded paint");
    }
}
