// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **AgentStatusHeader** — compact top-level status for the current agent/session.
//!
//! **Mission.** Project/session, branch, agent/mode/model, connection,
//! working/waiting status, context usage, cost/time when provided, and
//! action-required state. **Actionable state outranks decorative metadata.**
//! Narrow layouts contract into [`super::StatusBar`] slots. Quick actions:
//! sessions, model, tasks, help. Provider-neutral — no vendor APIs.
//!
//! **vs [`super::StatusBar`].** StatusBar is a general L/C/R slot bar;
//! AgentStatusHeader is the agent workbench header that *projects into*
//! StatusBar when contracted.
//! **vs [`super::ModeRibbon`].** Mode strip only; header is multi-field chrome.
//! **vs [`super::ContextMeter`].** Budget specialist; header shows a compact cue.
//!
//! Research: Grok Build headers, OpenCode, Amp, IDE workspace headers.
//!
//! Teaches: how to compose compact top-level status for the current
//! agent/session.
//!
//! Composes: [`crate::widgets::AccentRail`], [`crate::widgets::StatefulWidget`],
//! [`crate::widgets::StatusBar`], [`crate::widgets::StatusBarRecipe`],
//! [`crate::widgets::StatusBarState`], [`crate::widgets::StatusKind`],
//! [`crate::widgets::StatusRegion`], [`crate::widgets::StatusSegment`], and 3
//! more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::AccentRail,
    widgets::SemanticStatus,
    widgets::StatusBar,
    widgets::StatusBarRecipe,
    widgets::StatusBarState,
    widgets::StatusKind,
    widgets::StatusRegion,
    widgets::StatusSegment,
    widgets::StatusSlot,
    widgets::StatusStrip,
};

/// Overlay / focus id for header chrome.
pub const AGENT_STATUS_HEADER_ID: &str = "termrock.agent_status_header";
/// Width below which presentation contracts to StatusBar projection.
pub const AGENT_STATUS_NARROW_WIDTH: u16 = 56;
/// Max action chips painted.
pub const AGENT_STATUS_ACTION_CAP: usize = 6;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Agent run / attention state (provider-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentWorkStatus {
    /// Idle, ready for input.
    #[default]
    Idle,
    /// Actively working (tools / stream).
    Working,
    /// Streaming model tokens.
    Streaming,
    /// Waiting on user (question / plan).
    WaitingUser,
    /// Waiting on permission / trust.
    WaitingPermission,
    /// Soft error / failed last turn (still actionable).
    Error,
    /// Generic action required (host sets label).
    ActionRequired,
}

impl AgentWorkStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Streaming => "streaming",
            Self::WaitingUser => "waiting_user",
            Self::WaitingPermission => "waiting_permission",
            Self::Error => "error",
            Self::ActionRequired => "action_required",
        }
    }

    /// Default short label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Working => "working",
            Self::Streaming => "streaming",
            Self::WaitingUser => "waiting on you",
            Self::WaitingPermission => "permission",
            Self::Error => "error",
            Self::ActionRequired => "action required",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Idle => ".",
                Self::Working | Self::Streaming => "*",
                Self::WaitingUser | Self::WaitingPermission | Self::ActionRequired => "!",
                Self::Error => "x",
            };
        }
        match self {
            Self::Idle => "○",
            Self::Working => "●",
            Self::Streaming => "◎",
            Self::WaitingUser | Self::ActionRequired => "⚠",
            // One column, not two (plans/013 Step 2).
            Self::WaitingPermission => "⚿",
            Self::Error => "✗",
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Idle => SemanticStatus::Idle,
            Self::Working | Self::Streaming => SemanticStatus::Running,
            Self::WaitingUser | Self::WaitingPermission => SemanticStatus::Waiting,
            Self::Error => SemanticStatus::Failed,
            Self::ActionRequired => SemanticStatus::Warning,
        }
    }

    /// Whether this outranks decorative meta (always show when true).
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        matches!(
            self,
            Self::WaitingUser
                | Self::WaitingPermission
                | Self::ActionRequired
                | Self::Error
                | Self::Working
                | Self::Streaming
        )
    }

    /// Priority for drop order (higher = keep longer).
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::WaitingPermission | Self::ActionRequired => 100,
            Self::WaitingUser | Self::Error => 95,
            Self::Working | Self::Streaming => 80,
            Self::Idle => 20,
        }
    }
}

/// Connection chrome (provider-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentConnectionStatus {
    /// Online / ready.
    #[default]
    Ready,
    /// Connecting / reconnecting.
    Connecting,
    /// Offline / disconnected.
    Disconnected,
    /// Connected but degraded.
    Degraded,
}

impl AgentConnectionStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Degraded => "degraded",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ready => "online",
            Self::Connecting => "connecting",
            Self::Disconnected => "offline",
            Self::Degraded => "degraded",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Ready => "+",
                Self::Connecting => "~",
                Self::Disconnected => "x",
                Self::Degraded => "!",
            };
        }
        match self {
            Self::Ready => "●",
            Self::Connecting => "◌",
            Self::Disconnected => "○",
            Self::Degraded => "⚠",
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Ready => SemanticStatus::Online,
            Self::Connecting => SemanticStatus::Running,
            Self::Disconnected => SemanticStatus::Offline,
            Self::Degraded => SemanticStatus::Warning,
        }
    }

    /// Actionable connection problems.
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        !matches!(self, Self::Ready)
    }
}

/// Quick action ids (outcomes; host handles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AgentStatusAction {
    /// Open session picker.
    Sessions,
    /// Open model selector.
    Model,
    /// Open mode selector.
    Mode,
    /// Open task rail / tasks.
    Tasks,
    /// Open help / keymap.
    Help,
    /// Focus context meter / budget.
    Context,
    /// Focus project/branch (optional host jump).
    Project,
    /// Dismiss / acknowledge action-required (host).
    Acknowledge,
}

impl AgentStatusAction {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Model => "model",
            Self::Mode => "mode",
            Self::Tasks => "tasks",
            Self::Help => "help",
            Self::Context => "context",
            Self::Project => "project",
            Self::Acknowledge => "acknowledge",
        }
    }

    /// Chip label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sessions => "Sessions",
            Self::Model => "Model",
            Self::Mode => "Mode",
            Self::Tasks => "Tasks",
            Self::Help => "Help",
            Self::Context => "Context",
            Self::Project => "Project",
            Self::Acknowledge => "Ack",
        }
    }

    /// Default action strip (sessions · model · tasks · help).
    #[must_use]
    pub fn default_strip() -> &'static [AgentStatusAction] {
        &[Self::Sessions, Self::Model, Self::Tasks, Self::Help]
    }
}

/// Host-projected snapshot for one frame (no provider I/O).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentStatusSnapshot {
    /// Project name / path short form.
    pub project: Option<String>,
    /// Session title or id label.
    pub session: Option<String>,
    /// Branch / workspace branch.
    pub branch: Option<String>,
    /// Agent name / role label.
    pub agent: Option<String>,
    /// Safety / work mode (plan/edit/…).
    pub mode: Option<String>,
    /// Model label (provider-neutral string).
    pub model: Option<String>,
    /// Connection.
    pub connection: AgentConnectionStatus,
    /// Work status.
    pub work: AgentWorkStatus,
    /// Optional host override for work label.
    pub work_label: Option<String>,
    /// Context tokens used (optional).
    pub context_used: Option<u64>,
    /// Context limit (optional).
    pub context_limit: Option<u64>,
    /// Cost label (`$0.12`, `12¢`) — host formatted.
    pub cost: Option<String>,
    /// Elapsed / wall time label (`2m`, `1:04:12`).
    pub elapsed: Option<String>,
    /// Explicit action-required (or derived from work).
    pub action_required: bool,
    /// Action detail (`permission: shell`, `3 questions`).
    pub action_detail: Option<String>,
    /// Prompt queue depth (optional).
    pub queue_depth: Option<u32>,
    /// Task needs-input count (optional).
    pub tasks_needing_input: Option<u32>,
}

impl AgentStatusSnapshot {
    /// Empty idle snapshot.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Project.
    #[must_use]
    pub fn project(mut self, p: impl Into<String>) -> Self {
        self.project = Some(p.into());
        self
    }

    /// Session.
    #[must_use]
    pub fn session(mut self, s: impl Into<String>) -> Self {
        self.session = Some(s.into());
        self
    }

    /// Branch.
    #[must_use]
    pub fn branch(mut self, b: impl Into<String>) -> Self {
        self.branch = Some(b.into());
        self
    }

    /// Agent.
    #[must_use]
    pub fn agent(mut self, a: impl Into<String>) -> Self {
        self.agent = Some(a.into());
        self
    }

    /// Mode.
    #[must_use]
    pub fn mode(mut self, m: impl Into<String>) -> Self {
        self.mode = Some(m.into());
        self
    }

    /// Model.
    #[must_use]
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = Some(m.into());
        self
    }

    /// Connection.
    #[must_use]
    pub const fn connection(mut self, c: AgentConnectionStatus) -> Self {
        self.connection = c;
        self
    }

    /// Work.
    #[must_use]
    pub const fn work(mut self, w: AgentWorkStatus) -> Self {
        self.work = w;
        self
    }

    /// Work label override.
    #[must_use]
    pub fn work_label(mut self, l: impl Into<String>) -> Self {
        self.work_label = Some(l.into());
        self
    }

    /// Context budget.
    #[must_use]
    pub const fn context(mut self, used: u64, limit: u64) -> Self {
        self.context_used = Some(used);
        self.context_limit = Some(limit);
        self
    }

    /// Cost.
    #[must_use]
    pub fn cost(mut self, c: impl Into<String>) -> Self {
        self.cost = Some(c.into());
        self
    }

    /// Elapsed.
    #[must_use]
    pub fn elapsed(mut self, e: impl Into<String>) -> Self {
        self.elapsed = Some(e.into());
        self
    }

    /// Action required with detail.
    #[must_use]
    pub fn action_required(mut self, detail: impl Into<String>) -> Self {
        self.action_required = true;
        self.action_detail = Some(detail.into());
        if matches!(self.work, AgentWorkStatus::Idle) {
            self.work = AgentWorkStatus::ActionRequired;
        }
        self
    }

    /// Queue depth.
    #[must_use]
    pub const fn queue_depth(mut self, n: u32) -> Self {
        self.queue_depth = Some(n);
        self
    }

    /// Tasks needing input.
    #[must_use]
    pub const fn tasks_needing_input(mut self, n: u32) -> Self {
        self.tasks_needing_input = Some(n);
        self
    }

    /// Effective action-required.
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.action_required
            || self.work.is_actionable()
            || self.connection.is_actionable()
            || self.tasks_needing_input.is_some_and(|n| n > 0)
    }

    /// Work display string.
    #[must_use]
    pub fn work_text(&self) -> String {
        self.work_label
            .clone()
            .unwrap_or_else(|| self.work.label().into())
    }

    /// Context compact `12k/128k` or percent.
    #[must_use]
    pub fn context_text(&self) -> Option<String> {
        match (self.context_used, self.context_limit) {
            (Some(u), Some(l)) if l > 0 => {
                let pct = (u.saturating_mul(100)) / l;
                Some(format!("{pct}% ctx"))
            }
            (Some(u), None) => Some(format!("{u} tok")),
            _ => None,
        }
    }
}

// ── Presentation / outcomes ─────────────────────────────────────────────────

/// Layout form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AgentStatusPresentation {
    /// 1–2 row header (default when wide).
    #[default]
    Header,
    /// Contracted StatusBar projection.
    StatusBar,
}

impl AgentStatusPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::StatusBar => "status_bar",
        }
    }

    /// Auto-select from width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < AGENT_STATUS_NARROW_WIDTH {
            Self::StatusBar
        } else {
            Self::Header
        }
    }
}

/// Outcomes — requests only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AgentStatusHeaderOutcome {
    /// Ignored.
    Ignored,
    /// Quick action activated.
    Action(AgentStatusAction),
    /// Action cursor moved (does not fire).
    ActionFocused(AgentStatusAction),
    /// Presentation changed.
    PresentationChanged(AgentStatusPresentation),
    /// Header focused.
    Focused,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive header state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatusHeaderState {
    /// Snapshot (host updates each frame / tick).
    pub snapshot: AgentStatusSnapshot,
    /// Presentation (auto or forced).
    pub presentation: AgentStatusPresentation,
    /// When true, `for_width` updates presentation on paint.
    pub auto_contract: bool,
    /// Quick actions strip.
    pub actions: Vec<AgentStatusAction>,
    /// Action cursor.
    pub action_cursor: usize,
    /// Focused.
    pub focused: bool,
    /// Header shows the full segment sheet instead of its actionable core.
    pub segments_expanded: bool,
    accepts_input: bool,
    /// Action hit regions.
    pub action_hits: Vec<(AgentStatusAction, Rect)>,
    /// Owned strings for StatusBar projection (stable for paint frame).
    slot_strings: Vec<String>,
}

impl Default for AgentStatusHeaderState {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentStatusHeaderState {
    /// Empty with default action strip.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshot: AgentStatusSnapshot::new(),
            presentation: AgentStatusPresentation::Header,
            auto_contract: true,
            actions: AgentStatusAction::default_strip().to_vec(),
            action_cursor: 0,
            focused: true,
            segments_expanded: false,
            accepts_input: true,
            action_hits: Vec::new(),
            slot_strings: Vec::new(),
        }
    }

    /// Set snapshot.
    pub fn set_snapshot(&mut self, snap: AgentStatusSnapshot) {
        self.snapshot = snap;
        // Prefer Acknowledge when attention needed
        if self.snapshot.needs_attention()
            && !self.actions.contains(&AgentStatusAction::Acknowledge)
        {
            self.actions.insert(0, AgentStatusAction::Acknowledge);
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Force presentation.
    pub const fn set_presentation(&mut self, p: AgentStatusPresentation) {
        self.presentation = p;
        self.auto_contract = false;
    }

    /// Re-enable auto contract.
    pub const fn set_auto_contract(&mut self, on: bool) {
        self.auto_contract = on;
    }

    /// Custom actions.
    pub fn set_actions(&mut self, actions: Vec<AgentStatusAction>) {
        self.actions = actions;
        self.action_cursor = self.action_cursor.min(self.actions.len().saturating_sub(1));
    }

    fn current_action(&self) -> Option<AgentStatusAction> {
        self.actions.get(self.action_cursor).copied()
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> AgentStatusHeaderOutcome {
        if !self.focused || !self.accepts_input || key.kind != KeyEventKind::Press {
            return AgentStatusHeaderOutcome::Ignored;
        }
        if self.actions.is_empty() {
            return AgentStatusHeaderOutcome::Ignored;
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.action_cursor = self.action_cursor.saturating_sub(1);
                AgentStatusHeaderOutcome::ActionFocused(
                    self.current_action().unwrap_or(AgentStatusAction::Help),
                )
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.action_cursor + 1 < self.actions.len() {
                    self.action_cursor += 1;
                }
                AgentStatusHeaderOutcome::ActionFocused(
                    self.current_action().unwrap_or(AgentStatusAction::Help),
                )
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(a) = self.current_action() {
                    AgentStatusHeaderOutcome::Action(a)
                } else {
                    AgentStatusHeaderOutcome::Ignored
                }
            }
            // Chord shortcuts (focus not required to move)
            KeyCode::Char('i') if key.modifiers.is_empty() => {
                self.segments_expanded = !self.segments_expanded;
                AgentStatusHeaderOutcome::Ignored
            }
            KeyCode::Char('s') if key.modifiers.is_empty() => {
                AgentStatusHeaderOutcome::Action(AgentStatusAction::Sessions)
            }
            KeyCode::Char('m') if key.modifiers.is_empty() => {
                AgentStatusHeaderOutcome::Action(AgentStatusAction::Model)
            }
            KeyCode::Char('t') if key.modifiers.is_empty() => {
                AgentStatusHeaderOutcome::Action(AgentStatusAction::Tasks)
            }
            KeyCode::Char('?') => AgentStatusHeaderOutcome::Action(AgentStatusAction::Help),
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                AgentStatusHeaderOutcome::Action(AgentStatusAction::Context)
            }
            KeyCode::Char('y') => AgentStatusHeaderOutcome::Ignored,
            _ => AgentStatusHeaderOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> AgentStatusHeaderOutcome {
        if !self.focused || !self.accepts_input {
            return AgentStatusHeaderOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return AgentStatusHeaderOutcome::Ignored;
        }
        let pos = ev.position;
        for (action, r) in &self.action_hits {
            if r.contains(pos) {
                return AgentStatusHeaderOutcome::Action(*action);
            }
        }
        AgentStatusHeaderOutcome::Ignored
    }

    /// Build owned slot content strings + `StatusSlot` list for StatusBar paint.
    ///
    /// Callers should paint with the returned slots in the same frame; strings
    /// live in `self.slot_strings`.
    pub fn project_status_slots(&mut self) -> Vec<StatusSlot<'_, &str>> {
        self.slot_strings.clear();
        let snap = &self.snapshot;
        let mut specs: Vec<(
            StatusRegion,
            StatusKind,
            u8,
            String,
            Option<&'static str>,
            Option<SemanticStatus>,
        )> = Vec::new();

        // Priority: action > connection issues > work > project/session > mode/model > context > cost/time
        if snap.needs_attention() {
            let mut text = snap.work_text();
            if let Some(d) = snap.action_detail.as_ref() {
                text = format!("{text}: {d}");
            }
            if let Some(n) = snap.tasks_needing_input {
                if n > 0 {
                    text = format!("{text} · {n} tasks");
                }
            }
            specs.push((
                StatusRegion::Left,
                StatusKind::Transient,
                100,
                text,
                Some(snap.work.glyph(false)),
                Some(snap.work.semantic()),
            ));
        } else {
            specs.push((
                StatusRegion::Left,
                StatusKind::Mode,
                70,
                snap.work_text(),
                Some(snap.work.glyph(false)),
                Some(snap.work.semantic()),
            ));
        }

        if snap.connection.is_actionable() {
            specs.push((
                StatusRegion::Right,
                StatusKind::Connection,
                98,
                snap.connection.label().into(),
                Some(snap.connection.glyph(false)),
                Some(snap.connection.semantic()),
            ));
        }

        if let Some(p) = snap.project.as_ref() {
            let mut s = p.clone();
            if let Some(b) = snap.branch.as_ref() {
                s = format!("{s} · {b}");
            }
            specs.push((StatusRegion::Center, StatusKind::Context, 60, s, None, None));
        } else if let Some(s) = snap.session.as_ref() {
            specs.push((
                StatusRegion::Center,
                StatusKind::Context,
                55,
                s.clone(),
                None,
                None,
            ));
        }

        if let Some(m) = snap.mode.as_ref() {
            specs.push((
                StatusRegion::Left,
                StatusKind::Mode,
                50,
                m.clone(),
                None,
                None,
            ));
        }
        if let Some(m) = snap.model.as_ref() {
            specs.push((
                StatusRegion::Right,
                StatusKind::Text,
                45,
                m.clone(),
                None,
                None,
            ));
        }
        if let Some(ctx) = snap.context_text() {
            specs.push((
                StatusRegion::Right,
                StatusKind::Context,
                35,
                ctx,
                None,
                None,
            ));
        }
        if let Some(c) = snap.cost.as_ref() {
            specs.push((
                StatusRegion::Right,
                StatusKind::Text,
                25,
                c.clone(),
                None,
                None,
            ));
        }
        if let Some(e) = snap.elapsed.as_ref() {
            specs.push((
                StatusRegion::Right,
                StatusKind::Text,
                20,
                e.clone(),
                None,
                None,
            ));
        }
        if let Some(q) = snap.queue_depth {
            if q > 0 {
                specs.push((
                    StatusRegion::Right,
                    StatusKind::Text,
                    65,
                    format!("q:{q}"),
                    None,
                    None,
                ));
            }
        }

        // Sort by priority desc for stable fill, but StatusBar uses priority itself
        for (_, _, _, content, _, _) in &specs {
            self.slot_strings.push(content.clone());
        }

        specs
            .into_iter()
            .enumerate()
            .map(|(i, (region, kind, priority, _, glyph, semantic))| {
                let content: &str = self.slot_strings[i].as_str();
                let id: &str = match kind {
                    StatusKind::Mode => {
                        if priority >= 100 {
                            "action"
                        } else if priority >= 70 {
                            "work"
                        } else {
                            "mode"
                        }
                    }
                    StatusKind::Connection => "connection",
                    StatusKind::Context => {
                        if region == StatusRegion::Center {
                            "project"
                        } else {
                            "context"
                        }
                    }
                    StatusKind::Transient => "action",
                    StatusKind::Text => {
                        if content.starts_with('q') {
                            "queue"
                        } else if self.snapshot.model.as_deref() == Some(content) {
                            "model"
                        } else if self.snapshot.cost.as_deref() == Some(content) {
                            "cost"
                        } else {
                            "meta"
                        }
                    }
                    _ => "slot",
                };
                let mut slot = StatusSlot::new(id, content)
                    .region(region)
                    .kind(kind)
                    .priority(priority)
                    .min_width(4);
                if let Some(g) = glyph {
                    slot = slot.glyph(g);
                }
                if let Some(status) = semantic {
                    slot = slot.semantic(status);
                }
                slot
            })
            .collect()
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Agent status header painter.
#[derive(Debug, Clone, Copy)]
pub struct AgentStatusHeader<'a> {
    system: &'a DesignSystem,
    colorless: bool,
    show_actions: bool,
}

impl<'a> AgentStatusHeader<'a> {
    /// System only — snapshot in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
            show_actions: true,
        }
    }

    /// ASCII glyphs.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Hide quick action chips.
    #[must_use]
    pub const fn actions(mut self, on: bool) -> Self {
        self.show_actions = on;
        self
    }

    /// Paint (auto-contracts when `auto_contract` and narrow).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut AgentStatusHeaderState) {
        state.action_hits.clear();
        if area.is_empty() {
            return;
        }
        if state.auto_contract {
            state.presentation = AgentStatusPresentation::for_width(area.width);
        }
        match state.presentation {
            AgentStatusPresentation::Header => self.paint_header(area, buffer, state),
            AgentStatusPresentation::StatusBar => self.paint_as_status_bar(area, buffer, state),
        }
    }

    fn paint_as_status_bar(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut AgentStatusHeaderState,
    ) {
        let slots = state.project_status_slots();
        let left: Vec<StatusSlot<'_, &str>> = slots
            .iter()
            .filter(|s| s.region == StatusRegion::Left)
            .cloned()
            .collect();
        let center: Vec<StatusSlot<'_, &str>> = slots
            .iter()
            .filter(|s| s.region == StatusRegion::Center)
            .cloned()
            .collect();
        let right: Vec<StatusSlot<'_, &str>> = slots
            .iter()
            .filter(|s| s.region == StatusRegion::Right)
            .cloned()
            .collect();
        let bar = StatusBar::with_center(&left, &center, &right, self.system)
            .recipe(StatusBarRecipe::Compact);
        let mut sb_state = StatusBarState::<&str>::new();
        StatefulWidget::render(&bar, area, buffer, &mut sb_state);
    }

    fn paint_header(&self, area: Rect, buffer: &mut Buffer, state: &mut AgentStatusHeaderState) {
        let snap = &state.snapshot;
        let title = {
            let mut parts = Vec::new();
            if let Some(p) = snap.project.as_ref() {
                parts.push(p.as_str());
            }
            if let Some(s) = snap.session.as_ref() {
                parts.push(s.as_str());
            }
            if parts.is_empty() {
                "Agent".into()
            } else {
                // Identity hierarchy is not footer metadata. Keep it visually
                // distinct from the meta separator that marks key-hint rows.
                parts.join(" / ")
            }
        };
        let rail_role = if self.colorless {
            Role::TextStrong
        } else {
            snap.work.semantic().role()
        };
        let inner = AccentRail::new(self.system, rail_role).paint(area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let _w = usize::from(inner.width);
        let max_y = inner.bottom();

        // A wide header starts with identity, then status. At the two-row
        // minimum the status keeps the first row and actions keep the second.
        if area.height >= 3 && y < max_y {
            let identity = match snap.agent.as_deref() {
                Some(agent) if false => format!("{title} / {agent}"),
                Some(agent) => format!("{title} — {agent}"),
                None => title,
            };
            let identity = take_display_cols(&identity, usize::from(inner.width));
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                &identity,
                self.system.style(Role::TextStrong),
            );
            y = y.saturating_add(1);
        }

        // Status is always rail + glyph + verb; metadata never displaces it.
        if y < max_y {
            let work = snap.work_text();
            let mut work_s = work.to_string();
            if let Some(d) = snap.action_detail.as_ref() {
                work_s = format!("{work_s}: {d}");
            }
            let connection = snap.connection.label().to_string();
            let queue = snap
                .queue_depth
                .filter(|q| *q > 0)
                .map(|q| format!("q:{q}"));
            let branch = snap.branch.as_ref().map(|b| format!("⌥ {b}"));

            // A permanent row of eight segments in five hues is not a status
            // line, it is a dashboard. The default frame keeps what is
            // actionable — work, a connection that is not ready, the model,
            // and a non-empty queue — and `i` opens the rest in place
            // (information budget, plans/017 Part B). StatusStrip owns the
            // colour budget and the drop order (plans/016 Step 1).
            let expanded = state.segments_expanded;
            let ready = matches!(snap.connection, AgentConnectionStatus::Ready);
            let mut segments = vec![
                StatusSegment::new(&work_s)
                    .semantic(snap.work.semantic())
                    .priority(100),
            ];
            if expanded || !ready {
                segments.push(
                    StatusSegment::new(&connection)
                        .semantic(snap.connection.semantic())
                        .priority(90),
                );
            }
            if let Some(model) = snap.model.as_ref() {
                segments.push(StatusSegment::new(model).priority(70));
            }
            if let Some(queue) = queue.as_ref() {
                segments.push(
                    StatusSegment::new(queue)
                        .semantic(SemanticStatus::Warning)
                        .priority(80),
                );
            }
            let context = snap.context_text();
            if expanded {
                for (text, priority) in [
                    (branch.as_deref(), 40u8),
                    (snap.mode.as_deref(), 35),
                    (context.as_deref(), 30),
                    (snap.cost.as_deref(), 20),
                    (snap.elapsed.as_deref(), 10),
                ] {
                    if let Some(text) = text {
                        segments.push(StatusSegment::new(text).priority(priority));
                    }
                }
            }

            let hidden = [
                (!expanded && ready).then_some(()),
                (!expanded).then_some(()).filter(|()| branch.is_some()),
                (!expanded).then_some(()).filter(|()| snap.mode.is_some()),
                (!expanded).then_some(()).filter(|()| context.is_some()),
                (!expanded).then_some(()).filter(|()| snap.cost.is_some()),
                (!expanded)
                    .then_some(())
                    .filter(|()| snap.elapsed.is_some()),
            ]
            .into_iter()
            .flatten()
            .count();
            let hint = if expanded {
                "i less".to_string()
            } else {
                format!("i +{hidden}")
            };
            if expanded || hidden > 0 {
                segments.push(StatusSegment::new(&hint).quiet().priority(5));
            }

            StatusStrip::new(&segments, self.system)
                .colorless(self.colorless)
                .paint(Rect::new(inner.x, y, inner.width, 1), buffer);
            y = y.saturating_add(1);
        }

        // Row 2: quick actions
        if self.show_actions && y < max_y && !state.actions.is_empty() {
            let mut x = inner.x;
            let end = inner.x.saturating_add(inner.width);
            for (i, action) in state
                .actions
                .iter()
                .take(AGENT_STATUS_ACTION_CAP)
                .enumerate()
            {
                let focused = state.focused && i == state.action_cursor;
                let label = action.label();
                let text = if focused {
                    format!("[{label}]")
                } else {
                    format!(" {label} ")
                };
                let tw = display_cols(&text) as u16;
                if x.saturating_add(tw) > end {
                    break;
                }
                let style = if focused {
                    self.system.style(Role::Accent).add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(Role::TextMuted)
                };
                self.system
                    .paint_row(buffer, Rect::new(x, y, tw, 1), &text, style);
                state.action_hits.push((
                    *action,
                    Rect {
                        x,
                        y,
                        width: tw,
                        height: 1,
                    },
                ));
                x = x.saturating_add(tw.saturating_add(1));
            }
        }
    }
}

impl StatefulWidget for &AgentStatusHeader<'_> {
    type State = AgentStatusHeaderState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for AgentStatusHeader<'_> {
    type State = AgentStatusHeaderState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo snapshot with action-required attention.
#[must_use]
pub fn example_agent_status() -> AgentStatusSnapshot {
    AgentStatusSnapshot::new()
        .project("termrock")
        .session("Auth refactor")
        .branch("feat/auth")
        .agent("main")
        .mode("edit")
        .model("grok")
        .connection(AgentConnectionStatus::Ready)
        .work(AgentWorkStatus::WaitingPermission)
        .action_required("shell: cargo test")
        .context(48_000, 128_000)
        .cost("$0.04")
        .elapsed("12m")
        .queue_depth(2)
        .tasks_needing_input(1)
}

/// Idle connected snapshot.
#[must_use]
pub fn example_agent_status_idle() -> AgentStatusSnapshot {
    AgentStatusSnapshot::new()
        .project("termrock")
        .session("Docs")
        .branch("main")
        .mode("ask")
        .model("grok")
        .connection(AgentConnectionStatus::Ready)
        .work(AgentWorkStatus::Idle)
        .context(12_000, 128_000)
        .elapsed("3m")
}

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 30;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open_action() -> AgentStatusHeaderState {
        let mut st = AgentStatusHeaderState::new();
        st.set_snapshot(example_agent_status());
        st.presentation = AgentStatusPresentation::Header;
        st.auto_contract = false;
        st
    }

    #[test]
    fn actionable_outranks_idle() {
        let snap = example_agent_status();
        assert!(snap.needs_attention());
        assert!(snap.work.priority() > AgentWorkStatus::Idle.priority());
    }

    #[test]
    fn auto_contract_on_narrow() {
        assert_eq!(
            AgentStatusPresentation::for_width(40),
            AgentStatusPresentation::StatusBar
        );
        assert_eq!(
            AgentStatusPresentation::for_width(80),
            AgentStatusPresentation::Header
        );
    }

    #[test]
    fn action_chords() {
        let mut st = open_action();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('s'))),
            AgentStatusHeaderOutcome::Action(AgentStatusAction::Sessions)
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('m'))),
            AgentStatusHeaderOutcome::Action(AgentStatusAction::Model)
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('t'))),
            AgentStatusHeaderOutcome::Action(AgentStatusAction::Tasks)
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('?'))),
            AgentStatusHeaderOutcome::Action(AgentStatusAction::Help)
        ));
    }

    #[test]
    fn enter_activates_focused_action() {
        let mut st = open_action();
        assert_eq!(st.actions[0], AgentStatusAction::Acknowledge);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            AgentStatusHeaderOutcome::Action(AgentStatusAction::Acknowledge)
        ));
    }

    #[test]
    fn arrow_moves_action_focus() {
        let mut st = open_action();
        let out = st.handle_key(press(KeyCode::Right));
        assert!(matches!(out, AgentStatusHeaderOutcome::ActionFocused(_)));
        assert_eq!(st.action_cursor, 1);
    }

    #[test]
    fn y_unbound() {
        let mut st = open_action();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            AgentStatusHeaderOutcome::Ignored
        ));
    }

    #[test]
    fn project_status_slots_prioritize_action() {
        let mut st = open_action();
        let slots = st.project_status_slots();
        assert!(!slots.is_empty());
        let top = slots.iter().max_by_key(|s| s.priority).unwrap();
        assert!(top.priority >= 95, "{}", top.priority);
    }

    #[test]
    fn paint_header_and_statusbar() {
        let system = DesignSystem::default();
        let mut st = open_action();
        let area = Rect::new(0, 0, 72, 3);
        let mut buf = Buffer::empty(area);
        st.presentation = AgentStatusPresentation::Header;
        AgentStatusHeader::new(&system).paint(area, &mut buf, &mut st);

        let narrow = Rect::new(0, 0, 40, 1);
        let mut buf2 = Buffer::empty(narrow);
        st.auto_contract = true;
        AgentStatusHeader::new(&system).paint(narrow, &mut buf2, &mut st);
        assert_eq!(st.presentation, AgentStatusPresentation::StatusBar);
    }

    #[test]
    fn paint_ascii_colorless() {
        let system = DesignSystem::default();
        let mut st = open_action();
        st.presentation = AgentStatusPresentation::Header;
        st.auto_contract = false;
        let area = Rect::new(0, 0, 64, 3);
        let mut buf = Buffer::empty(area);
        AgentStatusHeader::new(&system)
            .colorless(true)
            .actions(false)
            .paint(area, &mut buf, &mut st);
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open_action();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            AgentStatusHeaderOutcome::Ignored
        ));
    }

    #[test]
    fn no_provider_io() {
        let src = include_str!("agent_status_header.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "openai", "anthropic"] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = open_action();
        st.presentation = AgentStatusPresentation::Header;
        st.auto_contract = false;
        let area = Rect::new(0, 0, 80, 3);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            AgentStatusHeader::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_work_connection() {
        for w in [
            AgentWorkStatus::Idle,
            AgentWorkStatus::Working,
            AgentWorkStatus::Streaming,
            AgentWorkStatus::WaitingUser,
            AgentWorkStatus::WaitingPermission,
            AgentWorkStatus::Error,
            AgentWorkStatus::ActionRequired,
        ] {
            assert!(!w.id().is_empty());
            let _ = w.glyph(true);
        }
        for c in [
            AgentConnectionStatus::Ready,
            AgentConnectionStatus::Connecting,
            AgentConnectionStatus::Disconnected,
            AgentConnectionStatus::Degraded,
        ] {
            assert!(!c.id().is_empty());
        }
    }

    #[test]
    fn mouse_action() {
        let system = DesignSystem::default();
        let mut st = open_action();
        st.presentation = AgentStatusPresentation::Header;
        st.auto_contract = false;
        let area = Rect::new(0, 0, 72, 3);
        let mut buf = Buffer::empty(area);
        AgentStatusHeader::new(&system).paint(area, &mut buf, &mut st);
        if let Some((action, r)) = st.action_hits.first().copied() {
            let out = st.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position { x: r.x, y: r.y },
                modifiers: KeyModifiers::NONE,
            });
            assert!(matches!(
                out,
                AgentStatusHeaderOutcome::Action(a) if a == action
            ));
        }
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let system = DesignSystem::default();
        for _ in [false, true] {
            for (width, height) in [(48, 3), (20, 1), (1, 1), (0, 0)] {
                let mut st = AgentStatusHeaderState::new();
                st.set_snapshot(
                    AgentStatusSnapshot::new()
                        .project("プロジェクト Cafe\u{301}")
                        .session("検査 🔍")
                        .branch("機能")
                        .mode("編集")
                        .model("モデル")
                        .work(AgentWorkStatus::Working),
                );
                st.presentation = AgentStatusPresentation::Header;
                st.auto_contract = false;
                let area = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(area);
                AgentStatusHeader::new(&system).paint(area, &mut buf, &mut st);
                if width == 48 {
                    let text: String = buf.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains('プ'), "{text:?}");
                    assert!(text.contains("Cafe\u{301}"), "{text:?}");
                }
            }
        }
    }

    #[test]
    fn idle_snapshot_no_ack_action() {
        let mut st = AgentStatusHeaderState::new();
        st.set_snapshot(example_agent_status_idle());
        assert!(!st.actions.contains(&AgentStatusAction::Acknowledge));
    }

    #[test]
    fn context_text_percent() {
        let s = AgentStatusSnapshot::new().context(50, 100);
        assert_eq!(s.context_text().as_deref(), Some("50% ctx"));
    }
}
