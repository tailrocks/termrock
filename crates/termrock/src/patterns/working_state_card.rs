// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **WorkingStateCard** — transparent but non-invasive summary of what the
//! agent is doing **now**.
//!
//! **Mission.** Current phase, concise **application-supplied** rationale
//! (never private chain-of-thought), relevant files/resources, elapsed time,
//! next expected action, cancel/inspect controls. Phases: planning, searching,
//! editing, running, waiting, reviewing. Collapses into
//! [`super::ActivityShelf`] when not expanded. Semantic description + no-color
//! state for accessibility.
//!
//! **Privacy law.** Host must only pass user-safe summaries. This component
//! never labels content as “thinking”/CoT and never implies hidden reasoning
//! is shown. Prefer “status” / “summary” wording in chrome.
//!
//! **vs [`super::ActivityShelf`].** Multi-activity strip; this is the primary
//! “current work” card that *projects into* a shelf chip when collapsed.
//! **vs [`super::ThinkingBlock`].** Optional raw thinking chrome (host policy);
//! WorkingStateCard is status-only and privacy-preserving by design.
//! **vs [`super::AgentStatusHeader`].** Session chrome; this is turn work.
//!
//! Research: agent status surfaces with privacy-preserving reasoning summaries.

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
    patterns::activity_shelf::{ActivityItem, ActivityKind},
    style::{DesignSystem, MotionPolicy, PanelChrome, Role, SPINNER_DOT_PULSE_FRAMES},
    text::{display_cols, take_display_cols},
    widgets::SemanticStatus,
    widgets::{AccentRail, Panel},
};

/// Overlay id when host promotes expanded work card.
pub const WORKING_STATE_OVERLAY_ID: &str = "termrock.working_state";
/// Max files painted in expanded card.
pub const WORKING_STATE_FILE_WINDOW: usize = 6;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Coarse agent work phase (product-neutral, not CoT).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WorkingPhase {
    /// Planning next steps (no private plan text implied).
    Planning,
    /// Searching / reading.
    Searching,
    /// Editing files.
    Editing,
    /// Running tools / shell / builds.
    #[default]
    Running,
    /// Waiting on user, permission, or external.
    Waiting,
    /// Reviewing results / diffs.
    Reviewing,
}

impl WorkingPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Searching => "searching",
            Self::Editing => "editing",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Reviewing => "reviewing",
        }
    }

    /// Short chrome label (never “thinking”).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::Searching => "searching",
            Self::Editing => "editing",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Reviewing => "reviewing",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Planning => "P",
                Self::Searching => "/",
                Self::Editing => "E",
                Self::Running => "*",
                Self::Waiting => "!",
                Self::Reviewing => "R",
            };
        }
        match self {
            Self::Planning => "◇",
            Self::Searching => "⌕",
            Self::Editing => "✎",
            Self::Running => "●",
            Self::Waiting => "⏸",
            Self::Reviewing => "◎",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Planning | Self::Searching | Self::Reviewing => Role::Info,
            // Live work reads as information, not as the brand (plans/007).
            Self::Editing | Self::Running => Role::InfoDim,
            Self::Waiting => Role::Warning,
        }
    }

    /// Map to ActivityShelf kind for collapse.
    #[must_use]
    pub const fn to_activity_kind(self) -> ActivityKind {
        match self {
            Self::Planning | Self::Reviewing => ActivityKind::Generic,
            Self::Searching => ActivityKind::Search,
            Self::Editing => ActivityKind::Tool,
            Self::Running => ActivityKind::Shell,
            Self::Waiting => ActivityKind::Generic,
        }
    }

    /// Semantic status for shelf projection.
    #[must_use]
    pub const fn to_semantic_status(self) -> SemanticStatus {
        match self {
            Self::Waiting => SemanticStatus::Waiting,
            Self::Planning | Self::Searching | Self::Editing | Self::Running | Self::Reviewing => {
                SemanticStatus::Running
            }
        }
    }
}

/// One file or resource touch (host-projected path/label).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingResource {
    /// Stable id.
    pub id: String,
    /// Display path or name.
    pub label: String,
    /// Optional role (`read`, `write`, `ref`).
    pub role: Option<String>,
}

impl WorkingResource {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role: None,
        }
    }

    /// Role.
    #[must_use]
    pub fn role(mut self, r: impl Into<String>) -> Self {
        self.role = Some(r.into());
        self
    }
}

/// Host-projected “what is happening now” (privacy-safe).
///
/// **Rationale** must be an application-authored public summary — never raw
/// model chain-of-thought. Field name is `summary` on purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingState {
    /// Stable work id (turn / job).
    pub id: String,
    /// Phase.
    pub phase: WorkingPhase,
    /// Public status summary (host-authored; not private CoT).
    pub summary: String,
    /// Optional next expected action (user-visible).
    pub next_action: Option<String>,
    /// Elapsed label (`12s`, `2m`).
    pub elapsed: Option<String>,
    /// Relevant files / resources.
    pub resources: Vec<WorkingResource>,
    /// Progress 0–100 when known.
    pub progress: Option<u8>,
    /// Cancel available.
    pub can_cancel: bool,
    /// Inspect / open details available.
    pub can_inspect: bool,
    /// Actor label (`agent`, `subagent:x`).
    pub actor: Option<String>,
    /// Optional waiting reason (permission, question) — not CoT.
    pub waiting_reason: Option<String>,
}

impl WorkingState {
    /// Running work with public summary.
    #[must_use]
    pub fn new(id: impl Into<String>, phase: WorkingPhase, summary: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            phase,
            summary: summary.into(),
            next_action: None,
            elapsed: None,
            resources: Vec::new(),
            progress: None,
            can_cancel: true,
            can_inspect: true,
            actor: None,
            waiting_reason: None,
        }
    }

    /// Next action.
    #[must_use]
    pub fn next_action(mut self, a: impl Into<String>) -> Self {
        self.next_action = Some(a.into());
        self
    }

    /// Elapsed.
    #[must_use]
    pub fn elapsed(mut self, e: impl Into<String>) -> Self {
        self.elapsed = Some(e.into());
        self
    }

    /// Resources.
    #[must_use]
    pub fn resources(mut self, r: Vec<WorkingResource>) -> Self {
        self.resources = r;
        self
    }

    /// Progress.
    #[must_use]
    pub fn progress(mut self, p: u8) -> Self {
        self.progress = Some(p.min(100));
        self
    }

    /// Cancel allowed.
    #[must_use]
    pub const fn can_cancel(mut self, on: bool) -> Self {
        self.can_cancel = on;
        self
    }

    /// Inspect allowed.
    #[must_use]
    pub const fn can_inspect(mut self, on: bool) -> Self {
        self.can_inspect = on;
        self
    }

    /// Actor.
    #[must_use]
    pub fn actor(mut self, a: impl Into<String>) -> Self {
        self.actor = Some(a.into());
        self
    }

    /// Waiting reason (public).
    #[must_use]
    pub fn waiting_reason(mut self, r: impl Into<String>) -> Self {
        self.waiting_reason = Some(r.into());
        self.phase = WorkingPhase::Waiting;
        self
    }

    /// Screen-reader / semantic description (no CoT language).
    #[must_use]
    pub fn semantic_description(&self) -> String {
        let mut parts = vec![
            format!("Agent status: {}", self.phase.label()),
            self.summary.clone(),
        ];
        if let Some(n) = &self.next_action {
            parts.push(format!("Next: {n}"));
        }
        if let Some(e) = &self.elapsed {
            parts.push(format!("Elapsed {e}"));
        }
        if let Some(w) = &self.waiting_reason {
            parts.push(format!("Waiting: {w}"));
        }
        if !self.resources.is_empty() {
            parts.push(format!("{} resources", self.resources.len()));
        }
        parts.join(". ")
    }

    /// Compact one-line for collapsed chrome.
    #[must_use]
    pub fn compact_line(&self, ascii: bool, max_cols: usize) -> String {
        let g = self.phase.glyph(ascii);
        let el = self
            .elapsed
            .as_ref()
            .map(|e| format!(" · {e}"))
            .unwrap_or_default();
        let mut s = format!("{g} {} · {}{el}", self.phase.label(), self.summary);
        if display_cols(&s) > max_cols {
            s = take_display_cols(&s, max_cols);
        }
        s
    }

    /// Project into [`ActivityItem`] for ActivityShelf collapse.
    #[must_use]
    pub fn to_activity_item(&self) -> ActivityItem {
        let mut item = ActivityItem::new(self.id.clone(), self.summary.clone())
            .status(self.phase.to_semantic_status())
            .kind(self.phase.to_activity_kind());
        if let Some(a) = &self.actor {
            item = item.actor(a.clone());
        }
        if let Some(e) = &self.elapsed {
            item = item.elapsed(e.clone());
        }
        if let Some(p) = self.progress {
            item = item.progress(p);
        }
        if let Some(w) = &self.waiting_reason {
            item = item.waiting_reason(w.clone());
        }
        if matches!(self.phase, WorkingPhase::Waiting) {
            item.blocked = true;
            item.action_required = self.waiting_reason.is_some();
        }
        item
    }
}

// ── Presentation / outcomes ─────────────────────────────────────────────────

/// Expanded card vs collapsed shelf projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum WorkingStatePresentation {
    /// Full card (phase, summary, files, controls).
    #[default]
    Expanded,
    /// Collapsed — host should paint ActivityShelf with projected item.
    Collapsed,
}

impl WorkingStatePresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Collapsed => "collapsed",
        }
    }
}

/// Outcomes — requests only.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WorkingStateOutcome {
    /// Ignored.
    Ignored,
    /// Expand card.
    Expanded,
    /// Collapse to shelf.
    Collapsed,
    /// Cancel current work (host maps to stop).
    CancelRequested {
        /// Work id.
        id: String,
    },
    /// Inspect / open details (tool card, log, etc.).
    InspectRequested {
        /// Work id.
        id: String,
    },
    /// Open a resource.
    ResourceActivated {
        /// Work id.
        work_id: String,
        /// Resource id.
        resource_id: String,
    },
    /// Resource cursor moved.
    ResourceSelected {
        /// Resource id.
        resource_id: String,
    },
    /// Presentation changed.
    PresentationChanged(WorkingStatePresentation),
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive working-state card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingStateCardState {
    /// Current work (None = idle / empty).
    pub work: Option<WorkingState>,
    /// Presentation.
    pub presentation: WorkingStatePresentation,
    /// Resource list cursor.
    pub resource_cursor: usize,
    /// Focused action: 0 = Inspect, 1 = Cancel (safe default Inspect when both).
    pub action_cursor: usize,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Action strip hit regions.
    pub action_hits: Vec<(WorkingAction, Rect)>,
    /// Resource row hit regions.
    pub resource_hits: Vec<(String, Rect)>,
    /// Header hit (toggle expand/collapse).
    pub header_hit: Option<Rect>,
}

/// Action strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WorkingAction {
    /// Inspect.
    Inspect,
    /// Cancel.
    Cancel,
    /// Expand / collapse toggle.
    ToggleExpand,
}

impl WorkingAction {
    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Inspect => "Inspect",
            Self::Cancel => "Cancel",
            Self::ToggleExpand => "Toggle",
        }
    }
}

impl Default for WorkingStateCardState {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkingStateCardState {
    /// Empty idle.
    #[must_use]
    pub fn new() -> Self {
        Self {
            work: None,
            presentation: WorkingStatePresentation::Expanded,
            resource_cursor: 0,
            action_cursor: 0, // Inspect default (safer than Cancel)
            focused: true,
            accepts_input: true,
            action_hits: Vec::new(),
            resource_hits: Vec::new(),
            header_hit: None,
        }
    }

    /// Set work snapshot.
    pub fn set_work(&mut self, work: Option<WorkingState>) {
        self.work = work;
        self.resource_cursor = 0;
        self.action_cursor = 0;
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Force collapsed (host paints shelf).
    pub fn collapse(&mut self) {
        self.presentation = WorkingStatePresentation::Collapsed;
    }

    /// Expand card.
    pub fn expand(&mut self) {
        self.presentation = WorkingStatePresentation::Expanded;
    }

    /// Whether collapsed.
    #[must_use]
    pub const fn is_collapsed(&self) -> bool {
        matches!(self.presentation, WorkingStatePresentation::Collapsed)
    }

    /// Project for ActivityShelf when collapsed (or always for multi-activity).
    #[must_use]
    pub fn to_activity_item(&self) -> Option<ActivityItem> {
        self.work.as_ref().map(WorkingState::to_activity_item)
    }

    /// Semantic description for a11y.
    #[must_use]
    pub fn semantic_description(&self) -> String {
        self.work
            .as_ref()
            .map(WorkingState::semantic_description)
            .unwrap_or_else(|| "Agent idle".into())
    }

    fn available_actions(&self) -> Vec<WorkingAction> {
        let Some(w) = &self.work else {
            return Vec::new();
        };
        let mut a = vec![WorkingAction::ToggleExpand];
        if w.can_inspect {
            a.push(WorkingAction::Inspect);
        }
        if w.can_cancel {
            a.push(WorkingAction::Cancel);
        }
        a
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> WorkingStateOutcome {
        if !self.focused || !self.accepts_input || key.kind != KeyEventKind::Press {
            return WorkingStateOutcome::Ignored;
        }
        let Some(work) = self.work.as_ref() else {
            return WorkingStateOutcome::Ignored;
        };
        let work_id = work.id.clone();

        if self.is_collapsed() {
            return match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('e') => {
                    self.expand();
                    WorkingStateOutcome::Expanded
                }
                KeyCode::Char('c') if work.can_cancel => {
                    WorkingStateOutcome::CancelRequested { id: work_id }
                }
                KeyCode::Char('i') if work.can_inspect => {
                    WorkingStateOutcome::InspectRequested { id: work_id }
                }
                KeyCode::Char('y') => WorkingStateOutcome::Ignored,
                _ => WorkingStateOutcome::Ignored,
            };
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('c')
                if matches!(key.code, KeyCode::Esc)
                    || (key.modifiers.contains(KeyModifiers::CONTROL)) =>
            {
                // Ctrl+C cancel when expanded; Esc collapses first
                if matches!(key.code, KeyCode::Esc) {
                    self.collapse();
                    return WorkingStateOutcome::Collapsed;
                }
                if work.can_cancel {
                    WorkingStateOutcome::CancelRequested { id: work_id }
                } else {
                    WorkingStateOutcome::Ignored
                }
            }
            KeyCode::Char('c') if work.can_cancel && key.modifiers.is_empty() => {
                // Prefer action strip: 'c' moves focus to Cancel then… fire cancel
                WorkingStateOutcome::CancelRequested { id: work_id }
            }
            KeyCode::Char('i') if work.can_inspect => {
                WorkingStateOutcome::InspectRequested { id: work_id }
            }
            KeyCode::Char('e') | KeyCode::Char(' ') => {
                // toggle
                self.collapse();
                WorkingStateOutcome::Collapsed
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.resource_cursor > 0 {
                    self.resource_cursor -= 1;
                    if let Some(r) = work.resources.get(self.resource_cursor) {
                        return WorkingStateOutcome::ResourceSelected {
                            resource_id: r.id.clone(),
                        };
                    }
                }
                WorkingStateOutcome::Ignored
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !work.resources.is_empty() && self.resource_cursor + 1 < work.resources.len() {
                    self.resource_cursor += 1;
                    if let Some(r) = work.resources.get(self.resource_cursor) {
                        return WorkingStateOutcome::ResourceSelected {
                            resource_id: r.id.clone(),
                        };
                    }
                }
                WorkingStateOutcome::Ignored
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.action_cursor = self.action_cursor.saturating_sub(1);
                WorkingStateOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let n = self.available_actions().len();
                if n > 0 && self.action_cursor + 1 < n {
                    self.action_cursor += 1;
                }
                WorkingStateOutcome::Ignored
            }
            KeyCode::Enter => {
                let actions = self.available_actions();
                let Some(a) = actions.get(self.action_cursor).copied() else {
                    // activate resource
                    if let Some(r) = work.resources.get(self.resource_cursor) {
                        return WorkingStateOutcome::ResourceActivated {
                            work_id,
                            resource_id: r.id.clone(),
                        };
                    }
                    return WorkingStateOutcome::Ignored;
                };
                match a {
                    WorkingAction::Inspect if work.can_inspect => {
                        WorkingStateOutcome::InspectRequested { id: work_id }
                    }
                    WorkingAction::Cancel if work.can_cancel => {
                        WorkingStateOutcome::CancelRequested { id: work_id }
                    }
                    WorkingAction::ToggleExpand => {
                        self.collapse();
                        WorkingStateOutcome::Collapsed
                    }
                    _ => WorkingStateOutcome::Ignored,
                }
            }
            KeyCode::Char('y') => WorkingStateOutcome::Ignored,
            _ => WorkingStateOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> WorkingStateOutcome {
        if !self.focused || !self.accepts_input {
            return WorkingStateOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return WorkingStateOutcome::Ignored;
        }
        let pos = ev.position;
        if let Some(h) = self.header_hit {
            if h.contains(pos) {
                if self.is_collapsed() {
                    self.expand();
                    return WorkingStateOutcome::Expanded;
                }
                self.collapse();
                return WorkingStateOutcome::Collapsed;
            }
        }
        for (action, r) in &self.action_hits {
            if r.contains(pos) {
                let id = self.work.as_ref().map(|w| w.id.clone()).unwrap_or_default();
                return match action {
                    WorkingAction::Inspect => WorkingStateOutcome::InspectRequested { id },
                    WorkingAction::Cancel => WorkingStateOutcome::CancelRequested { id },
                    WorkingAction::ToggleExpand => {
                        if self.is_collapsed() {
                            self.expand();
                            WorkingStateOutcome::Expanded
                        } else {
                            self.collapse();
                            WorkingStateOutcome::Collapsed
                        }
                    }
                };
            }
        }
        for (rid, r) in &self.resource_hits {
            if r.contains(pos) {
                let work_id = self.work.as_ref().map(|w| w.id.clone()).unwrap_or_default();
                return WorkingStateOutcome::ResourceActivated {
                    work_id,
                    resource_id: rid.clone(),
                };
            }
        }
        WorkingStateOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Working state card painter.
#[derive(Debug, Clone, Copy)]
pub struct WorkingStateCard<'a> {
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
    tick: u64,
}

impl<'a> WorkingStateCard<'a> {
    /// System only — work lives in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            ascii: false,
            colorless: false,
            tick: 0,
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

    /// Deterministic paint tick for active presence.
    #[must_use]
    pub const fn tick(mut self, tick: u64) -> Self {
        self.tick = tick;
        self
    }

    /// Paint.
    ///
    /// When collapsed, paints a single non-invasive line; host should also
    /// feed [`WorkingStateCardState::to_activity_item`] into ActivityShelf.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut WorkingStateCardState) {
        state.action_hits.clear();
        state.resource_hits.clear();
        state.header_hit = None;
        if area.is_empty() {
            return;
        }
        let Some(work) = state.work.clone() else {
            if !area.is_empty() {
                buffer.set_stringn(
                    area.x,
                    area.y,
                    take_display_cols("idle", usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
            }
            return;
        };

        if state.is_collapsed() {
            self.paint_collapsed(area, buffer, state, &work);
            return;
        }
        self.paint_expanded(area, buffer, state, &work);
    }

    fn paint_collapsed(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut WorkingStateCardState,
        work: &WorkingState,
    ) {
        let w = usize::from(area.width);
        let line = work.compact_line(self.ascii, w.saturating_sub(2));
        let style = if self.colorless {
            self.system.style(Role::Text)
        } else {
            self.system.style(work.phase.role())
        };
        let mark = if matches!(work.phase, WorkingPhase::Waiting) {
            if self.ascii { "!" } else { "●" }
        } else if self.ascii || !matches!(self.system.motion, MotionPolicy::Full) {
            if self.ascii { "." } else { "●" }
        } else {
            SPINNER_DOT_PULSE_FRAMES[self.tick as usize % SPINNER_DOT_PULSE_FRAMES.len()]
        };
        let text = format!("{mark} {line}");
        buffer.set_stringn(area.x, area.y, take_display_cols(&text, w), w, style);
        state.header_hit = Some(Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        });
    }

    fn paint_expanded(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut WorkingStateCardState,
        work: &WorkingState,
    ) {
        // Title avoids “thinking” — status chrome only
        let title = format!("Working · {}", work.phase.label());
        let rail = AccentRail::new(self.system, Role::ActorAssistant)
            .active(!matches!(work.phase, WorkingPhase::Waiting))
            .tick(self.tick);
        let content_area = rail.paint(area, buffer);
        let panel = Panel::new(self.system)
            .title(title.as_str())
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(content_area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, content_area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Phase + elapsed + progress
        if y < max_y {
            let g = work.phase.glyph(self.ascii);
            let el = work
                .elapsed
                .as_ref()
                .map(|e| format!(" · {e}"))
                .unwrap_or_default();
            let prog = work
                .progress
                .map(|p| format!(" · {p}%"))
                .unwrap_or_default();
            let actor = work
                .actor
                .as_ref()
                .map(|a| format!(" · {a}"))
                .unwrap_or_default();
            let line = format!("{g} {}{el}{prog}{actor}", work.phase.label());
            let style = if self.colorless {
                self.system.style(Role::Text)
            } else {
                self.system.style(work.phase.role())
            };
            buffer.set_stringn(inner.x, y, take_display_cols(&line, w), w, style);
            state.header_hit = Some(Rect {
                x: inner.x,
                y,
                width: inner.width,
                height: 1,
            });
            y = y.saturating_add(1);
        }

        // Public summary (never labeled as private reasoning)
        if y < max_y {
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&format!("summary: {}", work.summary), w),
                w,
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }

        if let Some(next) = &work.next_action {
            if y < max_y {
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(&format!("next: {next}"), w),
                    w,
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }

        if let Some(wr) = &work.waiting_reason {
            if y < max_y {
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(&format!("waiting: {wr}"), w),
                    w,
                    self.system.style(Role::Warning),
                );
                y = y.saturating_add(1);
            }
        }

        // Resources
        if !work.resources.is_empty() && y < max_y {
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols("resources", w),
                w,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
            for (i, r) in work
                .resources
                .iter()
                .enumerate()
                .take(WORKING_STATE_FILE_WINDOW)
            {
                if y >= max_y.saturating_sub(1) {
                    break;
                }
                let sel = i == state.resource_cursor;
                let mark = if sel {
                    if self.ascii { ">" } else { "›" }
                } else {
                    " "
                };
                let role = r
                    .role
                    .as_ref()
                    .map(|x| format!(" ({x})"))
                    .unwrap_or_default();
                let line = format!("{mark}{}{role}", r.label);
                let style = if sel {
                    self.system.style(Role::Accent)
                } else {
                    self.system.style(Role::Text)
                };
                buffer.set_stringn(inner.x, y, take_display_cols(&line, w), w, style);
                state.resource_hits.push((
                    r.id.clone(),
                    Rect {
                        x: inner.x,
                        y,
                        width: inner.width,
                        height: 1,
                    },
                ));
                y = y.saturating_add(1);
            }
        }

        // Actions (Inspect default focus; Cancel never sole default when both exist)
        let fy = max_y.saturating_sub(1);
        if fy >= inner.y {
            let actions = {
                let mut a = Vec::new();
                a.push(WorkingAction::ToggleExpand);
                if work.can_inspect {
                    a.push(WorkingAction::Inspect);
                }
                if work.can_cancel {
                    a.push(WorkingAction::Cancel);
                }
                a
            };
            // Prefer Inspect as default when present
            if state.action_cursor >= actions.len() {
                state.action_cursor = actions
                    .iter()
                    .position(|x| *x == WorkingAction::Inspect)
                    .unwrap_or(0);
            }
            let mut col = inner.x;
            let end = inner.x.saturating_add(inner.width);
            for (i, action) in actions.iter().enumerate() {
                let focused = state.focused && i == state.action_cursor;
                let label = match action {
                    WorkingAction::ToggleExpand => "Collapse",
                    other => other.label(),
                };
                let text = if focused {
                    format!("[{label}]")
                } else {
                    format!(" {label} ")
                };
                let tw = display_cols(&text) as u16;
                if col.saturating_add(tw) > end {
                    break;
                }
                let style = if focused {
                    if matches!(action, WorkingAction::Cancel) {
                        self.system.style(Role::Danger).add_modifier(Modifier::BOLD)
                    } else {
                        self.system.style(Role::Accent).add_modifier(Modifier::BOLD)
                    }
                } else {
                    self.system.style(Role::TextMuted)
                };
                buffer.set_stringn(col, fy, &text, usize::from(tw), style);
                state.action_hits.push((
                    *action,
                    Rect {
                        x: col,
                        y: fy,
                        width: tw,
                        height: 1,
                    },
                ));
                col = col.saturating_add(tw.saturating_add(1));
            }
        }
    }
}

impl StatefulWidget for &WorkingStateCard<'_> {
    type State = WorkingStateCardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for WorkingStateCard<'_> {
    type State = WorkingStateCardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Bridges ─────────────────────────────────────────────────────────────────

/// Project working state into a one-item shelf list for collapse composition.
#[must_use]
pub fn working_state_to_shelf_items(work: &WorkingState) -> Vec<ActivityItem> {
    vec![work.to_activity_item()]
}

/// Merge working state into existing shelf items (replace same id or push).
pub fn merge_working_into_shelf(items: &mut Vec<ActivityItem>, work: &WorkingState) {
    let next = work.to_activity_item();
    if let Some(slot) = items.iter_mut().find(|i| i.id == next.id) {
        *slot = next;
    } else {
        items.insert(0, next);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo: editing with public summary (not CoT).
#[must_use]
pub fn example_working_state() -> WorkingState {
    WorkingState::new("w1", WorkingPhase::Editing, "Updating auth module exports")
        .elapsed("24s")
        .progress(40)
        .next_action("run tests")
        .actor("agent")
        .resources(vec![
            WorkingResource::new("f1", "src/auth/mod.rs").role("write"),
            WorkingResource::new("f2", "src/auth/token.rs").role("read"),
        ])
}

/// Waiting on user (permission) — public reason only.
#[must_use]
pub fn example_working_waiting() -> WorkingState {
    WorkingState::new("w2", WorkingPhase::Waiting, "Need approval to run tests")
        .elapsed("1m")
        .waiting_reason("permission: shell")
        .next_action("respond to permission prompt")
        .can_cancel(true)
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 30;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open() -> WorkingStateCardState {
        let mut st = WorkingStateCardState::new();
        st.set_work(Some(example_working_state()));
        st.presentation = WorkingStatePresentation::Expanded;
        st
    }

    #[test]
    fn privacy_no_cot_wording_in_source() {
        let src = include_str!("working_state_card.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        // chrome must not present CoT as product feature
        assert!(body.contains("summary"));
        assert!(body.contains("Privacy") || body.contains("privacy"));
        assert!(body.contains("chain-of-thought") || body.contains("CoT"));
        // field is summary not thinking_body
        assert!(body.contains("pub summary"));
        for f in ["std::process", "Command::new", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn semantic_description_uses_status_not_thinking() {
        let w = example_working_state();
        let d = w.semantic_description();
        assert!(d.contains("status") || d.contains("editing"));
        assert!(!d.to_ascii_lowercase().contains("chain of thought"));
        assert!(!d.to_ascii_lowercase().contains("private"));
    }

    #[test]
    fn collapse_and_expand() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Esc));
        assert!(matches!(out, WorkingStateOutcome::Collapsed));
        assert!(st.is_collapsed());
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, WorkingStateOutcome::Expanded));
    }

    #[test]
    fn inspect_default_before_cancel() {
        let mut st = open();
        // action_cursor 0 is ToggleExpand; move to Inspect
        let actions = st.available_actions();
        assert!(actions.contains(&WorkingAction::Inspect));
        let inspect_i = actions
            .iter()
            .position(|a| *a == WorkingAction::Inspect)
            .unwrap();
        st.action_cursor = inspect_i;
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            WorkingStateOutcome::InspectRequested { ref id } if id == "w1"
        ));
    }

    #[test]
    fn cancel_request() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('c')));
        assert!(matches!(
            out,
            WorkingStateOutcome::CancelRequested { ref id } if id == "w1"
        ));
    }

    #[test]
    fn to_activity_item_for_shelf() {
        let w = example_working_state();
        let item = w.to_activity_item();
        assert_eq!(item.id, "w1");
        assert_eq!(item.kind, ActivityKind::Tool); // Editing
        assert_eq!(item.status, SemanticStatus::Running);
    }

    #[test]
    fn waiting_maps_blocked_shelf() {
        let w = example_working_waiting();
        let item = w.to_activity_item();
        assert!(item.blocked || item.action_required);
        assert_eq!(item.status, SemanticStatus::Waiting);
    }

    #[test]
    fn merge_into_shelf() {
        let mut items = vec![ActivityItem::new("other", "other work")];
        merge_working_into_shelf(&mut items, &example_working_state());
        assert_eq!(items[0].id, "w1");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn resource_nav() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Down));
        assert!(matches!(
            out,
            WorkingStateOutcome::ResourceSelected { ref resource_id } if resource_id == "f2"
        ));
    }

    #[test]
    fn y_unbound() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            WorkingStateOutcome::Ignored
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            WorkingStateOutcome::Ignored
        ));
    }

    #[test]
    fn paint_expanded_and_collapsed() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 12);
        let mut buf = Buffer::empty(area);
        WorkingStateCard::new(&system).paint(area, &mut buf, &mut st);
        st.collapse();
        WorkingStateCard::new(&system)
            .ascii(true)
            .colorless(true)
            .paint(area, &mut buf, &mut st);
        // title must not say thinking
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(!text.to_ascii_lowercase().contains("thinking"), "{text}");
    }

    #[test]
    fn reduced_motion_running_presence_is_tick_static() {
        let system = DesignSystem::default().motion(MotionPolicy::Basic);
        let render = |tick| {
            let area = Rect::new(0, 0, 48, 10);
            let mut buffer = Buffer::empty(area);
            let mut state = open();
            WorkingStateCard::new(&system)
                .tick(tick)
                .paint(area, &mut buffer, &mut state);
            buffer
        };
        assert_eq!(render(0), render(19));
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 60, 14);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            WorkingStateCard::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_phases() {
        for p in [
            WorkingPhase::Planning,
            WorkingPhase::Searching,
            WorkingPhase::Editing,
            WorkingPhase::Running,
            WorkingPhase::Waiting,
            WorkingPhase::Reviewing,
        ] {
            assert!(!p.id().is_empty());
            assert_ne!(p.label(), "thinking");
            let _ = p.to_activity_kind();
        }
    }

    #[test]
    fn mouse_header_toggles() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 10);
        let mut buf = Buffer::empty(area);
        WorkingStateCard::new(&system).paint(area, &mut buf, &mut st);
        if let Some(h) = st.header_hit {
            let out = st.handle_mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position { x: h.x, y: h.y },
                modifiers: KeyModifiers::NONE,
            });
            assert!(matches!(out, WorkingStateOutcome::Collapsed));
        }
    }

    #[test]
    fn unicode_summary() {
        let system = DesignSystem::default();
        let mut st = WorkingStateCardState::new();
        st.set_work(Some(
            WorkingState::new("u", WorkingPhase::Searching, "ファイルを検索 🔍")
                .resources(vec![WorkingResource::new("f", "日本語.rs")]),
        ));
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        WorkingStateCard::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn idle_paints() {
        let system = DesignSystem::default();
        let mut st = WorkingStateCardState::new();
        let area = Rect::new(0, 0, 20, 1);
        let mut buf = Buffer::empty(area);
        WorkingStateCard::new(&system).paint(area, &mut buf, &mut st);
    }
}
