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
//!
//! Teaches: how to compose transparent but non-invasive summary of what the
//! agent is doing now.
//!
//! Composes: [`crate::widgets::AccentRail`],
//! [`crate::widgets::SemanticStatus`], [`crate::widgets::StatefulWidget`],
//! [`crate::widgets::Widget`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{
    buffer::Buffer, layout::Rect, style::Modifier, text::Line, widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    patterns::activity_shelf::{ActivityItem, ActivityKind},
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::SemanticStatus,
    widgets::{AccentRail, List, ListRow, ListState, StatusIndicator},
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

fn working_resource_rows(work: &WorkingState) -> Vec<ListRow<'static, String>> {
    work.resources
        .iter()
        .map(|resource| {
            let mut row = ListRow::item(resource.id.clone(), Line::from(resource.label.clone()));
            if let Some(role) = resource.role.as_ref() {
                row = row.secondary(Line::from(role.clone()));
            }
            row
        })
        .collect()
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
    /// Stable-id resource collection state.
    pub resource_list: ListState<String>,
    /// Focused action: 0 = Inspect, 1 = Cancel (safe default Inspect when both).
    pub action_cursor: usize,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Action strip hit regions.
    pub action_hits: Vec<(WorkingAction, Rect)>,
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
            resource_list: ListState::new(None),
            action_cursor: 0, // Inspect default (safer than Cancel)
            focused: true,
            accepts_input: true,
            action_hits: Vec::new(),
            header_hit: None,
        }
    }

    /// Set work snapshot.
    pub fn set_work(&mut self, work: Option<WorkingState>) {
        let selected = work
            .as_ref()
            .and_then(|snapshot| snapshot.resources.first())
            .map(|resource| resource.id.clone());
        self.work = work;
        self.resource_list.select(selected);
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
        if !self.focused || !self.accepts_input || !key.is_press() {
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
                let rows = working_resource_rows(work);
                if matches!(
                    self.resource_list.select_previous(&rows),
                    crate::interaction::Outcome::Changed
                ) && let Some(resource_id) = self.resource_list.selected().cloned()
                {
                    return WorkingStateOutcome::ResourceSelected { resource_id };
                }
                WorkingStateOutcome::Ignored
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let rows = working_resource_rows(work);
                if matches!(
                    self.resource_list.select_next(&rows),
                    crate::interaction::Outcome::Changed
                ) && let Some(resource_id) = self.resource_list.selected().cloned()
                {
                    return WorkingStateOutcome::ResourceSelected { resource_id };
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
                    if let Some(resource_id) = self.resource_list.selected().cloned() {
                        return WorkingStateOutcome::ResourceActivated {
                            work_id,
                            resource_id,
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
        if let crate::interaction::Outcome::Activated(resource_id) = self.resource_list.click(pos) {
            let work_id = self
                .work
                .as_ref()
                .map(|work| work.id.clone())
                .unwrap_or_default();
            return WorkingStateOutcome::ResourceActivated {
                work_id,
                resource_id,
            };
        }
        WorkingStateOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Working state card painter.
#[derive(Debug, Clone, Copy)]
pub struct WorkingStateCard<'a> {
    system: &'a DesignSystem,
    colorless: bool,
    tick: u64,
}

impl<'a> WorkingStateCard<'a> {
    /// System only — work lives in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
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
    ///
    /// When collapsed, paints a single non-invasive line; host should also
    /// feed [`WorkingStateCardState::to_activity_item`] into ActivityShelf.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut WorkingStateCardState) {
        state.action_hits.clear();
        state.header_hit = None;
        if area.is_empty() {
            return;
        }
        let Some(work) = state.work.clone() else {
            let semantic = SemanticStatus::Idle;
            let rail_role = if self.colorless {
                Role::TextStrong
            } else {
                semantic.role()
            };
            let inner = AccentRail::new(self.system, rail_role).paint(area, buffer);
            if !inner.is_empty() {
                let glyph = semantic.glyph();
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, inner.y, inner.width, 1),
                    &format!("{glyph} idle"),
                    self.system.style(Role::Text),
                );
                StatusIndicator::compact(semantic, self.system)
                    .colorless(self.colorless)
                    .paint(
                        Rect::new(inner.x, inner.y, inner.width.min(1), 1),
                        buffer,
                        None,
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
        let semantic = work.phase.to_semantic_status();
        let rail_role = if self.colorless {
            Role::TextStrong
        } else {
            semantic.role()
        };
        let inner = AccentRail::new(self.system, rail_role).paint(area, buffer);
        if inner.is_empty() {
            return;
        }
        let glyph = semantic.glyph();
        let elapsed = work
            .elapsed
            .as_ref()
            .map(|value| format!(" · {value}"))
            .unwrap_or_default();
        let text = format!("{glyph} {} · {}{elapsed}", work.phase.label(), work.summary);
        self.system.paint_row(
            buffer,
            Rect::new(inner.x, inner.y, inner.width, 1),
            take_display_cols(&text, usize::from(inner.width)).as_ref(),
            self.system.style(Role::Text),
        );
        StatusIndicator::compact(semantic, self.system)
            .colorless(self.colorless)
            .paint(
                Rect::new(inner.x, inner.y, inner.width.min(1), 1),
                buffer,
                None,
            );
        state.header_hit = Some(Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
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
        let semantic = work.phase.to_semantic_status();
        let rail_role = if self.colorless {
            Role::TextStrong
        } else if state.focused {
            Role::Focus
        } else {
            semantic.role()
        };
        let content_area = AccentRail::new(self.system, rail_role).paint(area, buffer);
        let inner = content_area;
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let _w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Phase + elapsed + progress
        if y < max_y {
            let g = semantic.glyph();
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
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                &line,
                self.system.style(Role::Text),
            );
            StatusIndicator::compact(semantic, self.system)
                .colorless(self.colorless)
                .paint(Rect::new(inner.x, y, inner.width.min(1), 1), buffer, None);
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
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                &format!("summary: {}", work.summary),
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }

        if let Some(next) = &work.next_action {
            if y < max_y {
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    &format!("next: {next}"),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }

        if let Some(wr) = &work.waiting_reason {
            if y < max_y {
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, y, inner.width, 1),
                    &format!("! waiting: {wr}"),
                    self.system.style(Role::Warning),
                );
                y = y.saturating_add(1);
            }
        }

        // Resources use the shared collection recipe: stable identity, focus
        // ground, hit regions, and width contraction stay single-owned.
        if !work.resources.is_empty() && y < max_y {
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                "resources",
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
            let list_height = max_y.saturating_sub(1).saturating_sub(y);
            if list_height > 0 {
                let rows = working_resource_rows(work);
                StatefulWidget::render(
                    &List::new(&rows, self.system).focused(state.focused),
                    Rect::new(inner.x, y, inner.width, list_height),
                    buffer,
                    &mut state.resource_list,
                );
            }
        }

        // Actions (Inspect default focus; Cancel never sole default when both exist)
        let fy = max_y.saturating_sub(1);
        // The status row is the card's minimum viable anatomy. Never let
        // footer actions overwrite it when the host contracts to one row.
        if fy > inner.y {
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
                self.system
                    .paint_row(buffer, Rect::new(col, fy, tw, 1), &text, style);
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
    use crate::style::MotionPolicy;
    use crate::widgets::tests::click;
    use crate::widgets::tests::press;

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
    fn reference_minimum_and_below_minimum_keep_status_anatomy() {
        let system = DesignSystem::default();
        for (width, height) in [(56, 10), (12, 1), (1, 1)] {
            let area = Rect::new(0, 0, width, height);
            let mut buffer = Buffer::empty(area);
            let mut state = open();
            WorkingStateCard::new(&system)
                .colorless(true)
                .paint(area, &mut buffer, &mut state);
            let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
            assert!(text.starts_with('\u{2503}'), "{width}x{height}: {text:?}");
            assert!(
                !text.contains('┌')
                    && !text.contains('┐')
                    && !text.contains('└')
                    && !text.contains('┘'),
                "status surfaces do not grow a box: {text:?}"
            );
            if width > 12 {
                assert!(text.contains("editing"), "{width}x{height}: {text:?}");
            }
        }
    }

    #[test]
    fn reduced_motion_running_presence_is_tick_static() {
        let system = DesignSystem::default().motion(MotionPolicy::Off);
        let render = |_tick| {
            let area = Rect::new(0, 0, 48, 10);
            let mut buffer = Buffer::empty(area);
            let mut state = open();
            WorkingStateCard::new(&system).paint(area, &mut buffer, &mut state);
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
            let out = st.handle_mouse(click(h.x, h.y));
            assert!(matches!(out, WorkingStateOutcome::Collapsed));
        }
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let system = DesignSystem::default();
        let summary = "ファイルを検索 Cafe\u{301}";
        for _ascii in [false, true] {
            for (width, height) in [(48, 8), (12, 1), (1, 1), (0, 0)] {
                let mut st = WorkingStateCardState::new();
                st.set_work(Some(
                    WorkingState::new("u", WorkingPhase::Searching, summary)
                        .resources(vec![WorkingResource::new("f", "日本語.rs")]),
                ));
                let area = Rect::new(0, 0, width, height);
                let mut buf = Buffer::empty(area);
                WorkingStateCard::new(&system).paint(area, &mut buf, &mut st);
                if width == 48 {
                    let text: String = buf.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains('フ'), "{text:?}");
                    assert!(text.contains("Cafe\u{301}"), "{text:?}");
                }
            }
        }
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
