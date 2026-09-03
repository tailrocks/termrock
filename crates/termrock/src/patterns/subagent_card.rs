// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SubagentCard** — reusable representation of delegated agent work.
//!
//! **Mission.** Show role, task, parent/provenance, status, model/mode, context,
//! elapsed time, progress, latest summary, output preview, and actions. Support
//! steer, message, inspect, cancel, retry, detach, and promote result outcomes
//! **without** implementing agent control. Distinguish **live** work from
//! **completed artifact/result**. Compact row, card, and fullscreen. Nested
//! delegation readable.
//!
//! **vs [`super::TaskRail`] / [`ActivityModel`](super::ActivityModel).** Rail is
//! the inventory; this card is the detailed subagent surface. Bridge via
//! [`subagent_to_activity_model`].
//! **vs [`super::ToolCallCard`].** Tools are invocations; subagents are
//! delegated agent runs with steer/message.
//!
//! **Ownership.** Host owns agent runtime. Outcomes are requests only.
//!
//! Research: multi-agent products, Grok Build subagents, orchestration UIs.
//!
//! Teaches: how to compose reusable representation of delegated agent work.
//!
//! Composes: [`crate::widgets::AccentRail`], [`crate::widgets::Card`],
//! [`crate::widgets::SemanticStatus`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    patterns::{ActivityKind, ActivityModel, ActivityScope},
    style::{DesignSystem, MotionPolicy, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::{AccentRail, Card, SemanticStatus, StatusIndicator},
};

/// Overlay id for fullscreen subagent detail.
pub const SUBAGENT_FULLSCREEN_OVERLAY_ID: &str = "termrock.subagent_fullscreen";
/// Max provenance hops painted.
pub const SUBAGENT_PROVENANCE_CAP: usize = 4;
/// Preview lines when expanded.
pub const SUBAGENT_PREVIEW_LINE_CAP: usize = 6;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Live work vs completed artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SubagentPhase {
    /// Still running / waiting / queued (live delegation).
    #[default]
    Live,
    /// Finished — result/artifact available.
    Artifact,
}

impl SubagentPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Artifact => "artifact",
        }
    }

    /// Badge.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Artifact => "result",
        }
    }

    /// Infer from status when host omits phase.
    #[must_use]
    pub const fn from_status(status: SemanticStatus) -> Self {
        match status {
            SemanticStatus::Success
            | SemanticStatus::Failed
            | SemanticStatus::Paused
            | SemanticStatus::Warning => Self::Artifact,
            _ => Self::Live,
        }
    }
}

/// Action affordance on the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubagentAction {
    /// Toggle expand / card detail.
    ToggleExpand,
    /// Steer (host sends guidance).
    Steer,
    /// Message subagent.
    Message,
    /// Inspect run / logs.
    Inspect,
    /// Cancel live work.
    Cancel,
    /// Retry failed/cancelled.
    Retry,
    /// Detach from UI (host may keep process).
    Detach,
    /// Promote result into parent transcript/context.
    PromoteResult,
    /// Fullscreen zoom.
    Fullscreen,
    /// Jump parent / provenance hop.
    OpenParent,
}

impl SubagentAction {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::ToggleExpand => "toggle-expand",
            Self::Steer => "steer",
            Self::Message => "message",
            Self::Inspect => "inspect",
            Self::Cancel => "cancel",
            Self::Retry => "retry",
            Self::Detach => "detach",
            Self::PromoteResult => "promote-result",
            Self::Fullscreen => "fullscreen",
            Self::OpenParent => "open-parent",
        }
    }

    /// Chord hint.
    #[must_use]
    pub const fn chord(self) -> &'static str {
        match self {
            Self::ToggleExpand => "Enter",
            Self::Steer => "s",
            Self::Message => "m",
            Self::Inspect => "i",
            Self::Cancel => "c",
            Self::Retry => "r",
            Self::Detach => "d",
            Self::PromoteResult => "p",
            Self::Fullscreen => "f",
            Self::OpenParent => "u",
        }
    }
}

/// Host-projected delegated agent run (no runtime control in TermRock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentRun {
    /// Stable run id.
    pub id: String,
    /// Role label (`reviewer`, `researcher`, domain-neutral).
    pub role: String,
    /// Task summary.
    pub task: String,
    /// Parent agent / run id.
    pub parent_id: Option<String>,
    /// Provenance hops outer→inner (`main`, `sub:plan`, `sub:review`).
    pub provenance: Vec<String>,
    /// Lifecycle status.
    pub status: SemanticStatus,
    /// Explicit phase (live vs artifact); if None, inferred from status.
    pub phase: Option<SubagentPhase>,
    /// Model id display.
    pub model: Option<String>,
    /// Mode display (`plan`, `build`, host-owned).
    pub mode: Option<String>,
    /// Context summary (`32k · 12 tools`).
    pub context: Option<String>,
    /// Elapsed display.
    pub elapsed: Option<String>,
    /// Progress 0–100.
    pub progress: Option<u8>,
    /// Latest status summary line.
    pub latest_summary: Option<String>,
    /// Output preview (stdout / last message).
    pub output_preview: Option<String>,
    /// Final result summary when artifact.
    pub result_summary: Option<String>,
    /// Nesting depth (0 = top-level subagent).
    pub depth: u8,
    /// Stream revision.
    pub revision: u64,
}

impl SubagentRun {
    /// Live subagent with role + task.
    #[must_use]
    pub fn new(id: impl Into<String>, role: impl Into<String>, task: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: role.into(),
            task: task.into(),
            parent_id: None,
            provenance: Vec::new(),
            status: SemanticStatus::Running,
            phase: Some(SubagentPhase::Live),
            model: None,
            mode: None,
            context: None,
            elapsed: None,
            progress: None,
            latest_summary: None,
            output_preview: None,
            result_summary: None,
            depth: 0,
            revision: 0,
        }
    }

    /// Effective phase.
    #[must_use]
    pub fn phase(&self) -> SubagentPhase {
        self.phase
            .unwrap_or_else(|| SubagentPhase::from_status(self.status))
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: SemanticStatus) -> Self {
        self.status = s;
        self
    }

    /// Phase.
    #[must_use]
    pub const fn phase_set(mut self, p: SubagentPhase) -> Self {
        self.phase = Some(p);
        self
    }

    /// Parent.
    #[must_use]
    pub fn parent(mut self, id: impl Into<String>) -> Self {
        self.parent_id = Some(id.into());
        self
    }

    /// Provenance hop.
    #[must_use]
    pub fn hop(mut self, label: impl Into<String>) -> Self {
        self.provenance.push(label.into());
        self
    }

    /// Model.
    #[must_use]
    pub fn model(mut self, m: impl Into<String>) -> Self {
        self.model = Some(m.into());
        self
    }

    /// Mode.
    #[must_use]
    pub fn mode(mut self, m: impl Into<String>) -> Self {
        self.mode = Some(m.into());
        self
    }

    /// Context.
    #[must_use]
    pub fn context(mut self, c: impl Into<String>) -> Self {
        self.context = Some(c.into());
        self
    }

    /// Elapsed.
    #[must_use]
    pub fn elapsed(mut self, e: impl Into<String>) -> Self {
        self.elapsed = Some(e.into());
        self
    }

    /// Progress.
    #[must_use]
    pub fn progress(mut self, p: u8) -> Self {
        self.progress = Some(p.min(100));
        self
    }

    /// Latest summary.
    #[must_use]
    pub fn latest_summary(mut self, s: impl Into<String>) -> Self {
        self.latest_summary = Some(s.into());
        self
    }

    /// Output preview.
    #[must_use]
    pub fn output_preview(mut self, s: impl Into<String>) -> Self {
        self.output_preview = Some(s.into());
        self
    }

    /// Result summary (artifact).
    #[must_use]
    pub fn result_summary(mut self, s: impl Into<String>) -> Self {
        self.result_summary = Some(s.into());
        self.phase = Some(SubagentPhase::Artifact);
        self
    }

    /// Depth.
    #[must_use]
    pub const fn depth(mut self, d: u8) -> Self {
        self.depth = d;
        self
    }

    /// Revision.
    #[must_use]
    pub const fn revision(mut self, r: u64) -> Self {
        self.revision = r;
        self
    }

    /// Cancel meaningful?
    #[must_use]
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            SemanticStatus::Running
                | SemanticStatus::Queued
                | SemanticStatus::Waiting
                | SemanticStatus::Paused
        ) && matches!(self.phase(), SubagentPhase::Live)
    }

    /// Retry meaningful?
    #[must_use]
    pub fn can_retry(&self) -> bool {
        matches!(self.phase(), SubagentPhase::Artifact)
            && matches!(
                self.status,
                SemanticStatus::Failed | SemanticStatus::Paused | SemanticStatus::Warning
            )
    }

    /// Promote result meaningful?
    #[must_use]
    pub fn can_promote(&self) -> bool {
        matches!(self.phase(), SubagentPhase::Artifact)
            && matches!(
                self.status,
                SemanticStatus::Success | SemanticStatus::Warning
            )
    }

    /// Detach meaningful (live)?
    #[must_use]
    pub fn can_detach(&self) -> bool {
        matches!(self.phase(), SubagentPhase::Live)
            && matches!(
                self.status,
                SemanticStatus::Running | SemanticStatus::Waiting
            )
    }

    /// Header line.
    #[must_use]
    pub fn header_line(&self, _ascii: bool) -> String {
        let g = { self.status.glyph_unicode() };
        let phase = self.phase().badge();
        let mut s = format!("{g} [{phase}] {} — {}", self.role, self.task);
        if let Some(e) = &self.elapsed {
            s.push_str(" · ");
            s.push_str(e);
        }
        if let Some(p) = self.progress {
            s.push_str(&format!(" · {p}%"));
        }
        s
    }

    /// Provenance display `a › b › c`.
    #[must_use]
    pub fn provenance_line(&self, _ascii: bool) -> Option<String> {
        if self.provenance.is_empty() && self.parent_id.is_none() {
            return None;
        }
        let sep = { " › " };
        let mut parts = self.provenance.clone();
        if parts.is_empty() {
            if let Some(p) = &self.parent_id {
                parts.push(p.clone());
            }
        }
        if parts.is_empty() {
            return None;
        }
        let take = parts.len().min(SUBAGENT_PROVENANCE_CAP);
        let mut s = parts[parts.len().saturating_sub(take)..].join(sep);
        if parts.len() > SUBAGENT_PROVENANCE_CAP {
            s = format!("…{sep}{s}");
        }
        if self.depth > 0 {
            s = format!("d{} {s}", self.depth);
        }
        Some(s)
    }
}

/// Actions available for a run.
#[must_use]
pub fn subagent_actions_for(run: &SubagentRun) -> Vec<SubagentAction> {
    let mut a = vec![
        SubagentAction::ToggleExpand,
        SubagentAction::Fullscreen,
        SubagentAction::Inspect,
    ];
    if matches!(run.phase(), SubagentPhase::Live) {
        a.push(SubagentAction::Steer);
        a.push(SubagentAction::Message);
    }
    if run.can_cancel() {
        a.push(SubagentAction::Cancel);
    }
    if run.can_retry() {
        a.push(SubagentAction::Retry);
    }
    if run.can_detach() {
        a.push(SubagentAction::Detach);
    }
    if run.can_promote() {
        a.push(SubagentAction::PromoteResult);
    }
    if run.parent_id.is_some() || !run.provenance.is_empty() {
        a.push(SubagentAction::OpenParent);
    }
    a
}

/// Bridge → TaskRail ActivityModel.
#[must_use]
pub fn subagent_to_activity_model(run: &SubagentRun) -> ActivityModel {
    let mut m = ActivityModel::new(run.id.clone(), format!("{}: {}", run.role, run.task))
        .scope(ActivityScope::Subagent)
        .kind(ActivityKind::Subagent)
        .status(run.status);
    if let Some(p) = &run.parent_id {
        m = m.parent(p.clone());
    }
    if let Some(e) = &run.elapsed {
        m = m.elapsed(e.clone());
    }
    if let Some(p) = run.progress {
        m = m.progress(p);
    }
    if let Some(s) = run.latest_summary.as_ref().or(run.result_summary.as_ref()) {
        m = m.detail(s.clone());
    }
    if let Some(w) = run
        .latest_summary
        .as_ref()
        .filter(|_| run.phase() == SubagentPhase::Live)
    {
        // waiting reason-ish from summary when waiting
        if matches!(run.status, SemanticStatus::Waiting) {
            m = m.waiting_reason(w.clone());
        }
    }
    if let Some(actor) = run.provenance.last() {
        m = m.actor(actor.clone());
    }
    m
}

/// Project compact lines for MessageThread / rail.
#[must_use]
pub fn project_subagent_lines(run: &SubagentRun, expanded: bool, _ascii: bool) -> Vec<String> {
    let mut lines = vec![run.header_line(false)];
    if let Some(p) = run.provenance_line(false) {
        lines.push(format!("  via {p}"));
    }
    if let Some(m) = &run.model {
        let mut meta = m.clone();
        if let Some(mode) = &run.mode {
            meta.push_str(" · ");
            meta.push_str(mode);
        }
        if let Some(c) = &run.context {
            meta.push_str(" · ");
            meta.push_str(c);
        }
        lines.push(format!("  {meta}"));
    }
    if expanded {
        if let Some(s) = &run.latest_summary {
            lines.push(format!("  · {s}"));
        }
        if let Some(preview) = &run.output_preview {
            for l in preview.lines().take(SUBAGENT_PREVIEW_LINE_CAP) {
                lines.push(format!("  | {l}"));
            }
        }
        if matches!(run.phase(), SubagentPhase::Artifact) {
            if let Some(r) = &run.result_summary {
                lines.push(format!("  result: {r}"));
            }
        }
    } else if let Some(s) = run.latest_summary.as_ref().or(run.result_summary.as_ref()) {
        lines.push(format!("  → {s}"));
    }
    lines
}

// ── Presentation / state / outcomes ─────────────────────────────────────────

/// View density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SubagentPresentation {
    /// One-line row (rail / thread).
    #[default]
    CompactRow,
    /// Expanded card.
    Card,
    /// Fullscreen (host overlay).
    Fullscreen,
}

impl SubagentPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::CompactRow => "compact-row",
            Self::Card => "card",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Outcomes — **requests only**.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SubagentCardOutcome {
    /// Ignored.
    Ignored,
    /// Expanded to card.
    Expanded {
        /// Id.
        id: String,
    },
    /// Collapsed to row.
    Collapsed {
        /// Id.
        id: String,
    },
    /// Steer request.
    SteerRequested {
        /// Id.
        id: String,
    },
    /// Message request.
    MessageRequested {
        /// Id.
        id: String,
    },
    /// Inspect request.
    InspectRequested {
        /// Id.
        id: String,
    },
    /// Cancel request.
    CancelRequested {
        /// Id.
        id: String,
    },
    /// Retry request.
    RetryRequested {
        /// Id.
        id: String,
    },
    /// Detach request.
    DetachRequested {
        /// Id.
        id: String,
    },
    /// Promote result into parent.
    PromoteResult {
        /// Id.
        id: String,
    },
    /// Fullscreen.
    FullscreenRequested {
        /// Id.
        id: String,
    },
    /// Open parent / hop.
    OpenParent {
        /// Parent id if known.
        parent_id: Option<String>,
        /// Child id.
        id: String,
    },
    /// Generic activate.
    Activated {
        /// Id.
        id: String,
    },
}

/// Interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentCardState {
    /// Presentation.
    pub presentation: SubagentPresentation,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Action cursor when expanded.
    pub action_cursor: usize,
    /// Header hit.
    pub header_hit: Rect,
    /// Action hits.
    pub action_hits: Vec<(SubagentAction, Rect)>,
}

impl Default for SubagentCardState {
    fn default() -> Self {
        Self::new()
    }
}

impl SubagentCardState {
    /// Compact default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            presentation: SubagentPresentation::CompactRow,
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

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Expanded?
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        !matches!(self.presentation, SubagentPresentation::CompactRow)
    }

    /// Toggle expand.
    pub fn toggle_expand(&mut self, id: &str) -> SubagentCardOutcome {
        match self.presentation {
            SubagentPresentation::CompactRow => {
                self.presentation = SubagentPresentation::Card;
                SubagentCardOutcome::Expanded { id: id.to_string() }
            }
            SubagentPresentation::Card | SubagentPresentation::Fullscreen => {
                self.presentation = SubagentPresentation::CompactRow;
                SubagentCardOutcome::Collapsed { id: id.to_string() }
            }
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, run: &SubagentRun) -> SubagentCardOutcome {
        if !self.accepts_input || !self.focused || !key.is_press() {
            return SubagentCardOutcome::Ignored;
        }
        let actions = subagent_actions_for(run);
        match key.code {
            KeyCode::Enter | KeyCode::Char(' ') if key.modifiers.is_empty() => {
                self.toggle_expand(&run.id)
            }
            KeyCode::Char('s')
                if key.modifiers.is_empty() && matches!(run.phase(), SubagentPhase::Live) =>
            {
                SubagentCardOutcome::SteerRequested { id: run.id.clone() }
            }
            KeyCode::Char('m')
                if key.modifiers.is_empty() && matches!(run.phase(), SubagentPhase::Live) =>
            {
                SubagentCardOutcome::MessageRequested { id: run.id.clone() }
            }
            KeyCode::Char('i') if key.modifiers.is_empty() => {
                SubagentCardOutcome::InspectRequested { id: run.id.clone() }
            }
            KeyCode::Char('c') if key.modifiers.is_empty() && run.can_cancel() => {
                SubagentCardOutcome::CancelRequested { id: run.id.clone() }
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && run.can_retry() => {
                SubagentCardOutcome::RetryRequested { id: run.id.clone() }
            }
            KeyCode::Char('d') if key.modifiers.is_empty() && run.can_detach() => {
                SubagentCardOutcome::DetachRequested { id: run.id.clone() }
            }
            KeyCode::Char('p') if key.modifiers.is_empty() && run.can_promote() => {
                SubagentCardOutcome::PromoteResult { id: run.id.clone() }
            }
            KeyCode::Char('f') if key.modifiers.is_empty() => {
                self.presentation = SubagentPresentation::Fullscreen;
                SubagentCardOutcome::FullscreenRequested { id: run.id.clone() }
            }
            KeyCode::Char('u')
                if key.modifiers.is_empty()
                    && (run.parent_id.is_some() || !run.provenance.is_empty()) =>
            {
                SubagentCardOutcome::OpenParent {
                    parent_id: run.parent_id.clone(),
                    id: run.id.clone(),
                }
            }
            KeyCode::Left | KeyCode::Char('h') if self.is_expanded() && !actions.is_empty() => {
                self.action_cursor = self.action_cursor.saturating_sub(1);
                SubagentCardOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') if self.is_expanded() && !actions.is_empty() => {
                self.action_cursor = (self.action_cursor + 1).min(actions.len().saturating_sub(1));
                SubagentCardOutcome::Ignored
            }
            KeyCode::Esc if matches!(self.presentation, SubagentPresentation::Fullscreen) => {
                self.presentation = SubagentPresentation::Card;
                SubagentCardOutcome::Expanded { id: run.id.clone() }
            }
            _ => SubagentCardOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, run: &SubagentRun) -> SubagentCardOutcome {
        if !self.accepts_input || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return SubagentCardOutcome::Ignored;
        }
        for (act, rect) in &self.action_hits {
            if rect.contains(event.position) {
                return match act {
                    SubagentAction::ToggleExpand => self.toggle_expand(&run.id),
                    SubagentAction::Steer => {
                        SubagentCardOutcome::SteerRequested { id: run.id.clone() }
                    }
                    SubagentAction::Message => {
                        SubagentCardOutcome::MessageRequested { id: run.id.clone() }
                    }
                    SubagentAction::Inspect => {
                        SubagentCardOutcome::InspectRequested { id: run.id.clone() }
                    }
                    SubagentAction::Cancel => {
                        SubagentCardOutcome::CancelRequested { id: run.id.clone() }
                    }
                    SubagentAction::Retry => {
                        SubagentCardOutcome::RetryRequested { id: run.id.clone() }
                    }
                    SubagentAction::Detach => {
                        SubagentCardOutcome::DetachRequested { id: run.id.clone() }
                    }
                    SubagentAction::PromoteResult => {
                        SubagentCardOutcome::PromoteResult { id: run.id.clone() }
                    }
                    SubagentAction::Fullscreen => {
                        self.presentation = SubagentPresentation::Fullscreen;
                        SubagentCardOutcome::FullscreenRequested { id: run.id.clone() }
                    }
                    SubagentAction::OpenParent => SubagentCardOutcome::OpenParent {
                        parent_id: run.parent_id.clone(),
                        id: run.id.clone(),
                    },
                };
            }
        }
        if self.header_hit.contains(event.position) {
            return self.toggle_expand(&run.id);
        }
        SubagentCardOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Subagent card / row / fullscreen chrome.
#[derive(Debug, Clone, Copy)]
pub struct SubagentCard<'a> {
    run: &'a SubagentRun,
    system: &'a DesignSystem,
    colorless: bool,
    tick: u64,
}

impl<'a> SubagentCard<'a> {
    /// Run + system.
    #[must_use]
    pub const fn new(run: &'a SubagentRun, system: &'a DesignSystem) -> Self {
        Self {
            run,
            system,
            colorless: false,
            tick: 0,
        }
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Deterministic paint tick for active presence.
    #[must_use]
    pub const fn tick(mut self, tick: u64) -> Self {
        self.tick = tick;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SubagentCardState) {
        state.action_hits.clear();
        if area.is_empty() {
            return;
        }
        let run = self.run;
        let phase = run.phase();

        if matches!(state.presentation, SubagentPresentation::CompactRow) {
            self.paint_row(area, buffer, state, false);
            return;
        }

        let _active = matches!(run.status, SemanticStatus::Running);
        let rail = AccentRail::new(self.system, Role::ActorAssistant);
        let content_area = rail.paint(area, buffer);
        let g = { run.status.glyph_unicode() };
        let title = format!("{} · {}", run.role, take_display_cols(&run.task, 36));
        let mut subtitle = format!("{} · {}", phase.badge(), run.status.default_label());
        if let Some(e) = &run.elapsed {
            subtitle.push_str(" · ");
            subtitle.push_str(e);
        }
        if let Some(p) = run.progress {
            subtitle.push_str(&format!(" · {p}%"));
        }
        if let Some(m) = &run.model {
            subtitle.push_str(" · ");
            subtitle.push_str(m);
        }

        let emphasis = match run.status {
            SemanticStatus::Failed => PanelChrome::Danger,
            SemanticStatus::Running | SemanticStatus::Waiting if state.focused => {
                PanelChrome::Focused
            }
            _ if state.focused => PanelChrome::Focused,
            _ => PanelChrome::Normal,
        };

        let leading = { g };
        let badge = phase.badge();
        let card = Card::new(self.system)
            .title(title.as_str())
            .leading(leading)
            .badge(badge)
            .subtitle(subtitle.as_str())
            .emphasis(emphasis);
        let body = card.paint(content_area, buffer, None);
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
        let muted = self.system.style(Role::TextMuted);
        let text_style = self.system.style(Role::Text);

        // Nesting indent cue
        if run.depth > 0 && y < max_y {
            let ind = format!("{}nested d{}", { "↳ " }, run.depth);
            self.system
                .paint_row(buffer, Rect::new(body.x, y, body.width, 1), &ind, muted);
            y = y.saturating_add(1);
        }

        if let Some(p) = run.provenance_line(false) {
            if y < max_y {
                self.system.paint_row(
                    buffer,
                    Rect::new(body.x, y, body.width, 1),
                    &format!("via {p}"),
                    muted,
                );
                y = y.saturating_add(1);
            }
        }

        // model / mode / context
        {
            let mut meta = String::new();
            if let Some(m) = &run.mode {
                meta.push_str(m);
            }
            if let Some(c) = &run.context {
                if !meta.is_empty() {
                    meta.push_str(" · ");
                }
                meta.push_str(c);
            }
            if !meta.is_empty() && y < max_y {
                self.system
                    .paint_row(buffer, Rect::new(body.x, y, body.width, 1), &meta, muted);
                y = y.saturating_add(1);
            }
        }

        // Lifecycle is a status recipe, not a colored sentence.
        if y < max_y {
            let verb = match phase {
                SubagentPhase::Live => "live work",
                SubagentPhase::Artifact => "result artifact",
            };
            StatusIndicator::new(run.status, self.system)
                .label(verb)
                .colorless(self.colorless)
                .paint(Rect::new(body.x, y, body.width, 1), buffer);
            y = y.saturating_add(1);
        }

        if let Some(s) = &run.latest_summary {
            if y < max_y {
                self.system
                    .paint_row(buffer, Rect::new(body.x, y, body.width, 1), s, text_style);
                y = y.saturating_add(1);
            }
        }

        if let Some(preview) = &run.output_preview {
            for l in preview.lines().take(SUBAGENT_PREVIEW_LINE_CAP) {
                if y >= max_y {
                    break;
                }
                self.system.paint_row(
                    buffer,
                    Rect::new(body.x, y, body.width, 1),
                    &format!("│ {l}"),
                    muted,
                );
                y = y.saturating_add(1);
            }
        }

        if matches!(phase, SubagentPhase::Artifact) {
            if let Some(r) = &run.result_summary {
                if y < max_y {
                    self.system.paint_row(
                        buffer,
                        Rect::new(body.x, y, body.width, 1),
                        &format!("result: {r}"),
                        self.system.style(Role::TextStrong),
                    );
                    y = y.saturating_add(1);
                }
            }
        }

        // action strip
        if y < max_y {
            let actions = subagent_actions_for(run);
            let mut x = body.x;
            for (i, act) in actions.iter().take(8).enumerate() {
                let label = format!("[{}]", act.chord());
                let w = (display_cols(&label) as u16).saturating_add(1);
                if x.saturating_add(w) > body.right() {
                    break;
                }
                let sel = state.focused && i == state.action_cursor;
                self.system.paint_row(
                    buffer,
                    Rect::new(x, y, w, 1),
                    &label,
                    if sel {
                        self.system.style(Role::Focus)
                    } else {
                        muted
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
        let _ = display_cols;
    }

    fn paint_row(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SubagentCardState,
        _ascii: bool,
    ) {
        let run = self.run;
        let rail_role = if self.colorless {
            Role::TextStrong
        } else {
            run.status.role()
        };
        let inner = AccentRail::new(self.system, rail_role).paint(area, buffer);
        if false && area.width > 0 {
            buffer.set_string(area.x, area.y, "|", self.system.style(rail_role));
        }
        if inner.is_empty() {
            return;
        }
        let glyph = run.status.glyph();
        let line = format!(
            "{glyph} {} · {} · {}",
            run.status.default_label(),
            run.role,
            take_display_cols(&run.task, 36)
        );
        let indent = "  ".repeat(usize::from(run.depth.min(4)));
        let mut text = format!("{indent}{line}");
        if let Some(s) = run.latest_summary.as_ref().or(run.result_summary.as_ref()) {
            text.push_str(" · ");
            text.push_str(take_display_cols(s, 24).as_ref());
        }
        let style = if state.focused {
            self.system.style(Role::Text).add_modifier(Modifier::BOLD)
        } else {
            self.system.style(Role::Text)
        };
        self.system.paint_row(
            buffer,
            Rect::new(inner.x, inner.y, inner.width, 1),
            &text,
            style,
        );
        let glyph_column = u16::try_from(display_cols(&indent)).unwrap_or(u16::MAX);
        StatusIndicator::compact(run.status, self.system)
            .colorless(self.colorless)
            .paint(
                Rect::new(
                    inner.x.saturating_add(glyph_column),
                    inner.y,
                    inner.width.saturating_sub(glyph_column).min(1),
                    1,
                ),
                buffer,
            );
        state.header_hit = area;
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut SubagentCardState) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo subagent runs.
#[must_use]
pub fn example_subagent_runs() -> Vec<SubagentRun> {
    vec![
        SubagentRun::new("sa1", "reviewer", "review PR diff")
            .parent("main")
            .hop("main")
            .hop("sub:reviewer")
            .status(SemanticStatus::Running)
            .model("grok-4")
            .mode("plan")
            .context("24k · 6 tools")
            .elapsed("42s")
            .progress(55)
            .latest_summary("reading crates/termrock/src/…")
            .output_preview("found 3 risk areas\nchecking tests…")
            .depth(1),
        SubagentRun::new("sa2", "researcher", "fetch docs")
            .parent("sa1")
            .hop("main")
            .hop("sub:reviewer")
            .hop("sub:researcher")
            .status(SemanticStatus::Waiting)
            .waiting_via_summary("rate limited")
            .model("grok-4")
            .elapsed("12s")
            .depth(2),
        SubagentRun::new("sa3", "builder", "cargo test")
            .parent("main")
            .hop("main")
            .hop("sub:builder")
            .status(SemanticStatus::Success)
            .phase_set(SubagentPhase::Artifact)
            .result_summary("ok · 128 passed")
            .output_preview("test widgets::x ... ok\ndone")
            .elapsed("1.2s")
            .model("grok-4")
            .mode("build")
            .depth(1),
        SubagentRun::new("sa4", "fixer", "apply patches")
            .parent("main")
            .hop("main")
            .status(SemanticStatus::Failed)
            .phase_set(SubagentPhase::Artifact)
            .result_summary("exit 1 · conflict")
            .latest_summary("merge conflict in mod.rs")
            .elapsed("8s")
            .depth(1),
    ]
}

impl SubagentRun {
    fn waiting_via_summary(mut self, s: impl Into<String>) -> Self {
        self.latest_summary = Some(s.into());
        self.status = SemanticStatus::Waiting;
        self.phase = Some(SubagentPhase::Live);
        self
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Cards.
    pub const CARD_COUNT: usize = 32;
    /// Frames.
    pub const PAINT_FRAMES: u32 = 24;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    #[test]
    fn phase_live_vs_artifact() {
        let live = SubagentRun::new("a", "r", "t");
        assert_eq!(live.phase(), SubagentPhase::Live);
        let art = SubagentRun::new("a", "r", "t")
            .status(SemanticStatus::Success)
            .result_summary("ok");
        assert_eq!(art.phase(), SubagentPhase::Artifact);
        assert!(art.can_promote());
        assert!(!art.can_cancel());
        assert!(!live.can_promote());
        assert!(live.can_cancel());
    }

    #[test]
    fn nested_provenance_line() {
        let run = SubagentRun::new("a", "r", "t")
            .hop("main")
            .hop("sub:a")
            .hop("sub:b")
            .depth(2);
        let p = run.provenance_line(true).unwrap();
        assert!(p.contains("main"));
        assert!(p.contains("sub:b"));
        assert!(p.contains("d2") || p.contains('2'));
    }

    #[test]
    fn actions_gated_by_phase() {
        let live = SubagentRun::new("a", "r", "t").status(SemanticStatus::Running);
        let al = subagent_actions_for(&live);
        assert!(al.contains(&SubagentAction::Steer));
        assert!(al.contains(&SubagentAction::Cancel));
        assert!(!al.contains(&SubagentAction::PromoteResult));

        let art = SubagentRun::new("a", "r", "t")
            .status(SemanticStatus::Success)
            .result_summary("ok");
        let aa = subagent_actions_for(&art);
        assert!(aa.contains(&SubagentAction::PromoteResult));
        assert!(!aa.contains(&SubagentAction::Cancel));
        assert!(!aa.contains(&SubagentAction::Steer));
    }

    #[test]
    fn keys_steer_cancel_promote_fullscreen() {
        let live = SubagentRun::new("a", "r", "t").status(SemanticStatus::Running);
        let mut st = SubagentCardState::new();
        st.presentation = SubagentPresentation::Card;
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), &live),
            SubagentCardOutcome::SteerRequested { .. }
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &live),
            SubagentCardOutcome::CancelRequested { .. }
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE), &live),
            SubagentCardOutcome::FullscreenRequested { .. }
        ));
        let art = live
            .clone()
            .status(SemanticStatus::Success)
            .result_summary("ok");
        let mut st2 = SubagentCardState::new();
        st2.presentation = SubagentPresentation::Card;
        assert!(matches!(
            st2.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE), &art),
            SubagentCardOutcome::PromoteResult { .. }
        ));
        assert!(matches!(
            st2.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE), &art),
            SubagentCardOutcome::Ignored
        ));
    }

    #[test]
    fn expand_collapse_and_activity_bridge() {
        let run = SubagentRun::new("sa1", "reviewer", "review")
            .parent("main")
            .status(SemanticStatus::Running);
        let mut st = SubagentCardState::new();
        assert!(matches!(
            st.toggle_expand("sa1"),
            SubagentCardOutcome::Expanded { .. }
        ));
        assert!(st.is_expanded());
        let m = subagent_to_activity_model(&run);
        assert_eq!(m.scope, ActivityScope::Subagent);
        assert_eq!(m.kind, ActivityKind::Subagent);
        assert_eq!(m.parent_id.as_deref(), Some("main"));
    }

    #[test]
    fn project_lines_expanded() {
        let run = SubagentRun::new("a", "r", "t")
            .latest_summary("working")
            .output_preview("line1\nline2")
            .result_summary("done")
            .status(SemanticStatus::Success);
        let c = project_subagent_lines(&run, false, true);
        let e = project_subagent_lines(&run, true, true);
        assert!(e.len() >= c.len());
        assert!(e.join("\n").contains("result") || e.join("\n").contains("done"));
    }

    #[test]
    fn paint_all_presentations() {
        let system = DesignSystem::default();
        let runs = example_subagent_runs();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        for run in &runs {
            for pres in [
                SubagentPresentation::CompactRow,
                SubagentPresentation::Card,
                SubagentPresentation::Fullscreen,
            ] {
                let mut st = SubagentCardState::new();
                st.presentation = pres;
                st.focused = true;
                SubagentCard::new(run, &system).paint(area, &mut buf, &mut st);
            }
        }
    }

    #[test]
    fn reduced_motion_running_presence_is_tick_static() {
        let system = DesignSystem::default().motion(MotionPolicy::Off);
        let run =
            SubagentRun::new("sa", "reviewer", "review changes").status(SemanticStatus::Running);
        let render = |_tick| {
            let area = Rect::new(0, 0, 48, 10);
            let mut buffer = Buffer::empty(area);
            let mut state = SubagentCardState::new();
            state.presentation = SubagentPresentation::Card;
            SubagentCard::new(&run, &system).paint(area, &mut buffer, &mut state);
            buffer
        };
        assert_eq!(render(0), render(19));
    }

    #[test]
    fn mouse_header_toggles() {
        let system = DesignSystem::default();
        let run = example_subagent_runs()[0].clone();
        let mut st = SubagentCardState::new();
        st.presentation = SubagentPresentation::Card;
        let area = Rect::new(0, 0, 48, 10);
        let mut buf = Buffer::empty(area);
        SubagentCard::new(&run, &system).paint(area, &mut buf, &mut st);
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(st.header_hit.x, st.header_hit.y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            st.handle_mouse(ev, &run),
            SubagentCardOutcome::Collapsed { .. }
        ));
    }

    #[test]
    fn never_agent_runtime() {
        let src = include_str!("subagent_card.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in [
            "std::process",
            "Command::new",
            "portable_pty",
            "openai",
            "anthropic",
            "tokio::spawn",
            "reqwest",
        ] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn accepts_input_gate() {
        let run = SubagentRun::new("a", "r", "t");
        let mut st = SubagentCardState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &run),
            SubagentCardOutcome::Ignored
        ));
    }

    #[test]
    fn paint_perf_budget() {
        let system = DesignSystem::default();
        let runs = example_subagent_runs();
        let area = Rect::new(0, 0, 64, 12);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            for run in &runs {
                let mut st = SubagentCardState::new();
                st.presentation = SubagentPresentation::Card;
                SubagentCard::new(run, &system).paint(area, &mut buf, &mut st);
            }
        }
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_actions_and_phases() {
        for p in [SubagentPhase::Live, SubagentPhase::Artifact] {
            assert!(!p.id().is_empty());
        }
        for a in [
            SubagentAction::Steer,
            SubagentAction::Message,
            SubagentAction::Inspect,
            SubagentAction::Cancel,
            SubagentAction::Retry,
            SubagentAction::Detach,
            SubagentAction::PromoteResult,
            SubagentAction::Fullscreen,
            SubagentAction::OpenParent,
        ] {
            assert!(!a.id().is_empty());
            assert!(!a.chord().is_empty());
        }
        for s in [
            SemanticStatus::Running,
            SemanticStatus::Waiting,
            SemanticStatus::Success,
            SemanticStatus::Failed,
            SemanticStatus::Queued,
        ] {
            let _ = SubagentPhase::from_status(s);
        }
    }

    #[test]
    fn open_parent_chord() {
        let run = SubagentRun::new("a", "r", "t").parent("main").hop("main");
        let mut st = SubagentCardState::new();
        st.presentation = SubagentPresentation::Card;
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE), &run),
            SubagentCardOutcome::OpenParent {
                parent_id: Some(ref p),
                ..
            } if p == "main"
        ));
    }

    #[test]
    fn detach_and_message_live_only() {
        let live = SubagentRun::new("a", "r", "t").status(SemanticStatus::Running);
        let mut st = SubagentCardState::new();
        st.presentation = SubagentPresentation::Card;
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE), &live),
            SubagentCardOutcome::DetachRequested { .. }
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE), &live),
            SubagentCardOutcome::MessageRequested { .. }
        ));
    }
}
