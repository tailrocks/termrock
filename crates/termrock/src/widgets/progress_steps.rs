// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ProgressSteps** — pipeline and phase progress for builds, imports,
//! migrations, and agent plans.
//!
//! **Mission.** CI-style step lists: queued → running → waiting → complete, with
//! skipped, warning, failed, retrying, and cancelled. Durations, current verb,
//! optional details, and retry actions. Interactive navigation is explicit and
//! distinct from passive progress. Narrow terminals contract to a compact
//! summary. Compose with [`Timeline`](super::Timeline) and
//! [`TaskRail`](super::TaskRail) via projection helpers.
//!
//! **vs [`super::Stepper`].** Stepper is **navigation chrome** for multi-step
//! forms (Future/Current/Complete). ProgressSteps is **execution progress** for
//! pipelines (Queued/Running/Failed/Retrying…).
//! **vs [`super::ProgressBar`].** ProgressBar is one bar; ProgressSteps is many
//! ordered phases.
//! **vs [`super::PlanReview`].** PlanReview is pre-execution plan approval; this is
//! live run progress.
//!
//! Research: CI pipelines, installers, agent task plans.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        NavigationMove, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
        default_list_intent,
    },
    style::{DesignSystem, Glyph, Role},
    text::take_display_cols,
    widgets::{Hint, HintBar},
};

use super::list::ListRow;
use super::stepper::StepStatus;
use super::timeline::{Timeline, TimelineEvent};

/// Width under which expanded list becomes compact summary.
pub const PROGRESS_STEPS_COMPACT_MAX_WIDTH: u16 = 36;
/// Width under which only `n/total · verb` is shown.
pub const PROGRESS_STEPS_SUMMARY_MAX_WIDTH: u16 = 22;
/// Default hint for interactive mode.
pub const PROGRESS_STEPS_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "j/k",
        label: "move",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "retry",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "esc",
        label: "blur",
        priority: 30,
        visible: true,
    },
];

// ── Status ──────────────────────────────────────────────────────────────────

/// Execution status of one pipeline step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressStepStatus {
    /// Not started / queued.
    #[default]
    Queued,
    /// Currently executing.
    Running,
    /// Blocked waiting (input, lock, dependency).
    Waiting,
    /// Finished successfully.
    Complete,
    /// Intentionally skipped.
    Skipped,
    /// Completed with warning.
    Warning,
    /// Failed.
    Failed,
    /// Failed and retrying / in retry.
    Retrying,
    /// Cancelled by user/host.
    Cancelled,
}

impl ProgressStepStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Complete => "complete",
            Self::Skipped => "skipped",
            Self::Warning => "warning",
            Self::Failed => "failed",
            Self::Retrying => "retrying",
            Self::Cancelled => "cancelled",
        }
    }

    /// Non-color mark. One junie vocabulary; no ASCII profile.
    ///
    /// Progress marks, not checkbox wells: `[✓]` / `[ ]` belong to Checkbox.
    #[must_use]
    pub const fn mark(self) -> &'static str {
        match self {
            Self::Queued => " ",
            Self::Running | Self::Retrying => Glyph::SelectionMarker.resolve().text,
            Self::Waiting => Glyph::Ellipsis.resolve().text,
            Self::Complete => Glyph::Success.resolve().text,
            Self::Skipped | Self::Cancelled => Glyph::Remove.resolve().text,
            Self::Warning | Self::Failed => Glyph::Error.resolve().text,
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> super::SemanticStatus {
        super::SemanticStatus::from_progress_step_status(self)
    }
    /// Active work (not terminal).
    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Waiting | Self::Retrying)
    }

    /// Terminal success-ish.
    #[must_use]
    pub const fn is_done(self) -> bool {
        matches!(self, Self::Complete | Self::Skipped | Self::Warning)
    }

    /// Map loosely to form [`StepStatus`] for hosts that share chrome.
    #[must_use]
    pub const fn to_step_status(self) -> StepStatus {
        match self {
            Self::Queued => StepStatus::Future,
            Self::Running | Self::Waiting | Self::Retrying => StepStatus::Current,
            Self::Complete | Self::Warning => StepStatus::Complete,
            Self::Failed => StepStatus::Error,
            Self::Skipped => StepStatus::Skipped,
            Self::Cancelled => StepStatus::Disabled,
        }
    }

    /// Default verb when host omits one.
    #[must_use]
    pub const fn default_verb(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Complete => "done",
            Self::Skipped => "skipped",
            Self::Warning => "warn",
            Self::Failed => "failed",
            Self::Retrying => "retrying",
            Self::Cancelled => "cancelled",
        }
    }
}

// ── Model ───────────────────────────────────────────────────────────────────

/// One pipeline step (host-owned strings via owned model for simplicity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressStep {
    /// Stable id.
    pub id: String,
    /// Title (phase name).
    pub title: String,
    /// Current verb (e.g. "compiling", "uploading").
    pub verb: Option<String>,
    /// Optional detail line.
    pub detail: Option<String>,
    /// Status.
    pub status: ProgressStepStatus,
    /// Duration so far or final (ms); `None` if not tracked.
    pub duration_ms: Option<u64>,
    /// Host may offer retry for failed/cancelled.
    pub retryable: bool,
    /// Source (agent, job, crate).
    pub source: Option<String>,
}

impl ProgressStep {
    /// Queued step with title.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            verb: None,
            detail: None,
            status: ProgressStepStatus::Queued,
            duration_ms: None,
            retryable: false,
            source: None,
        }
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: ProgressStepStatus) -> Self {
        self.status = s;
        self
    }

    /// Verb.
    #[must_use]
    pub fn verb(mut self, v: impl Into<String>) -> Self {
        self.verb = Some(v.into());
        self
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Duration ms.
    #[must_use]
    pub const fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Retryable.
    #[must_use]
    pub const fn retryable(mut self, on: bool) -> Self {
        self.retryable = on;
        self
    }

    /// Source.
    #[must_use]
    pub fn source(mut self, s: impl Into<String>) -> Self {
        self.source = Some(s.into());
        self
    }

    /// Effective verb.
    #[must_use]
    pub fn effective_verb(&self) -> &str {
        self.verb
            .as_deref()
            .unwrap_or_else(|| self.status.default_verb())
    }
}

/// Interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressStepsMode {
    /// Paint only — no keyboard cursor (default for live pipelines).
    #[default]
    Passive,
    /// Cursor + retry activation (plan inspection / recovery).
    Interactive,
}

impl ProgressStepsMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Passive => "passive",
            Self::Interactive => "interactive",
        }
    }
}

/// Responsive presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ProgressStepsPresentation {
    /// Full list with marks, titles, verbs, details.
    #[default]
    Expanded,
    /// One line per step: mark + title + duration.
    Compact,
    /// Single summary line `3/7 · compiling`.
    Summary,
}

impl ProgressStepsPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Expanded => "expanded",
            Self::Compact => "compact",
            Self::Summary => "summary",
        }
    }

    /// Choose from width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width <= PROGRESS_STEPS_SUMMARY_MAX_WIDTH {
            Self::Summary
        } else if width <= PROGRESS_STEPS_COMPACT_MAX_WIDTH {
            Self::Compact
        } else {
            Self::Expanded
        }
    }
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProgressStepsOutcome {
    /// No change.
    Ignored,
    /// Cursor moved (interactive).
    SelectionChanged {
        /// Step id.
        id: Option<String>,
    },
    /// Retry requested for a step.
    RetryRequested {
        /// Step id.
        id: String,
    },
    /// Step activated (enter without retry).
    StepActivated {
        /// Step id.
        id: String,
    },
    /// Blur / leave interactive focus.
    Blurred,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Progress steps interaction / presentation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressStepsState {
    mode: ProgressStepsMode,
    presentation: Option<ProgressStepsPresentation>,
    cursor: Option<String>,
    scroll: usize,
    focused: bool,
    accepts_input: bool,
    enabled: bool,
    /// Show footer hint when interactive.
    show_hint: bool,
}

impl Default for ProgressStepsState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressStepsState {
    /// Passive default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: ProgressStepsMode::Passive,
            presentation: None,
            cursor: None,
            scroll: 0,
            focused: false,
            accepts_input: true,
            enabled: true,
            show_hint: true,
        }
    }

    /// Interactive factory.
    #[must_use]
    pub fn interactive() -> Self {
        let mut s = Self::new();
        s.mode = ProgressStepsMode::Interactive;
        s.focused = true;
        s
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> ProgressStepsMode {
        self.mode
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: ProgressStepsMode) {
        self.mode = mode;
        if matches!(mode, ProgressStepsMode::Passive) {
            self.focused = false;
        }
    }

    /// Focused (interactive).
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Forced presentation (else width-derived).
    pub fn set_presentation(&mut self, p: Option<ProgressStepsPresentation>) {
        self.presentation = p;
    }

    fn can_interact(&self) -> bool {
        matches!(self.mode, ProgressStepsMode::Interactive) && self.enabled && self.accepts_input
    }

    /// ASCII marks.
    /// Cursor.
    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Set cursor by step id.
    pub fn set_cursor(&mut self, id: Option<String>) {
        self.cursor = id;
    }

    /// Set cursor to first active or first step.
    pub fn ensure_cursor(&mut self, steps: &[ProgressStep]) {
        if steps.is_empty() {
            self.cursor = None;
            return;
        }
        if self
            .cursor
            .as_ref()
            .is_some_and(|c| steps.iter().any(|s| &s.id == c))
        {
            return;
        }
        let id = steps
            .iter()
            .find(|s| s.status.is_active())
            .or_else(|| steps.first())
            .map(|s| s.id.clone());
        self.cursor = id;
    }

    /// Summary counts.
    #[must_use]
    pub fn counts(steps: &[ProgressStep]) -> (usize, usize, usize) {
        let total = steps.len();
        let done = steps
            .iter()
            .filter(|s| s.status.is_done() || matches!(s.status, ProgressStepStatus::Cancelled))
            .count();
        let failed = steps
            .iter()
            .filter(|s| matches!(s.status, ProgressStepStatus::Failed))
            .count();
        (done, total, failed)
    }

    /// Current active step.
    #[must_use]
    pub fn active_step(steps: &[ProgressStep]) -> Option<&ProgressStep> {
        steps.iter().find(|s| s.status.is_active())
    }

    /// Summary line for narrow terminals.
    #[must_use]
    pub fn summary_line(steps: &[ProgressStep]) -> String {
        let (done, total, failed) = Self::counts(steps);
        let verb = Self::active_step(steps)
            .map(|s| s.effective_verb().to_string())
            .unwrap_or_else(|| {
                if failed > 0 {
                    "failed".into()
                } else if done >= total && total > 0 {
                    "complete".into()
                } else {
                    "idle".into()
                }
            });
        if failed > 0 {
            format!("{done}/{total} · {verb} · {failed} failed")
        } else {
            format!("{done}/{total} · {verb}")
        }
    }

    /// Keyboard (interactive only).
    pub fn handle_key(&mut self, steps: &[ProgressStep], key: KeyEvent) -> ProgressStepsOutcome {
        if !self.can_interact() || !self.focused {
            return ProgressStepsOutcome::Ignored;
        }
        if key.is_release() || steps.is_empty() {
            return ProgressStepsOutcome::Ignored;
        }
        if !key.is_insert() {
            return ProgressStepsOutcome::Ignored;
        }

        if matches!(key.code, KeyCode::Esc) && key.modifiers.is_empty() {
            self.focused = false;
            return ProgressStepsOutcome::Blurred;
        }

        self.ensure_cursor(steps);
        let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        let cur = self
            .cursor
            .as_deref()
            .and_then(|c| ids.iter().position(|id| *id == c))
            .unwrap_or(0);

        match key.code {
            KeyCode::Down | KeyCode::Char('j' | 'J') => {
                let next = (cur + 1).min(ids.len() - 1);
                self.cursor = Some(ids[next].to_string());
                ProgressStepsOutcome::SelectionChanged {
                    id: self.cursor.clone(),
                }
            }
            KeyCode::Up | KeyCode::Char('k' | 'K') => {
                let next = cur.saturating_sub(1);
                self.cursor = Some(ids[next].to_string());
                ProgressStepsOutcome::SelectionChanged {
                    id: self.cursor.clone(),
                }
            }
            KeyCode::Enter | KeyCode::Char('r' | 'R') => {
                let Some(id) = self.cursor.clone() else {
                    return ProgressStepsOutcome::Ignored;
                };
                let step = steps.iter().find(|s| s.id == id);
                if let Some(s) = step {
                    if s.retryable
                        && matches!(
                            s.status,
                            ProgressStepStatus::Failed
                                | ProgressStepStatus::Cancelled
                                | ProgressStepStatus::Retrying
                        )
                    {
                        return ProgressStepsOutcome::RetryRequested { id };
                    }
                }
                ProgressStepsOutcome::StepActivated { id }
            }
            _ => {
                if let Some(intent) = default_list_intent(key) {
                    return self.handle_intent(steps, intent);
                }
                ProgressStepsOutcome::Ignored
            }
        }
    }

    /// Intent path.
    pub fn handle_intent(
        &mut self,
        steps: &[ProgressStep],
        intent: UiIntent,
    ) -> ProgressStepsOutcome {
        if !self.can_interact() || !self.focused {
            return ProgressStepsOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => {
                self.focused = false;
                ProgressStepsOutcome::Blurred
            }
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                self.handle_key(steps, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            }
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                self.handle_key(steps, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            }
            UiIntent::Activate | UiIntent::Submit => {
                self.handle_key(steps, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            _ => ProgressStepsOutcome::Ignored,
        }
    }

    /// Mouse select (interactive).
    pub fn handle_mouse(
        &mut self,
        steps: &[ProgressStep],
        event: MouseEvent,
        list_area: Rect,
        row_height: u16,
    ) -> ProgressStepsOutcome {
        if !self.can_interact()
            || event.kind != MouseEventKind::Down(MouseButton::Left)
            || list_area.is_empty()
        {
            return ProgressStepsOutcome::Ignored;
        }
        if !list_area.contains(event.position) {
            return ProgressStepsOutcome::Ignored;
        }
        let row = event.position.y.saturating_sub(list_area.y) / row_height.max(1);
        let idx = self.scroll.saturating_add(row as usize);
        if let Some(s) = steps.get(idx) {
            self.cursor = Some(s.id.clone());
            self.focused = true;
            return ProgressStepsOutcome::SelectionChanged {
                id: self.cursor.clone(),
            };
        }
        ProgressStepsOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Pipeline steps paint widget.
#[derive(Debug, Clone, Copy)]
pub struct ProgressSteps<'a> {
    steps: &'a [ProgressStep],
    system: &'a DesignSystem,
    title: Option<&'a str>,
}

impl<'a> ProgressSteps<'a> {
    /// Steps + system.
    #[must_use]
    pub const fn new(steps: &'a [ProgressStep], system: &'a DesignSystem) -> Self {
        Self {
            steps,
            system,
            title: None,
        }
    }

    /// Optional header title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// ASCII marks.
    #[must_use]
    /// Resolve presentation.
    pub fn presentation(
        &self,
        state: &ProgressStepsState,
        width: u16,
    ) -> ProgressStepsPresentation {
        state
            .presentation
            .unwrap_or_else(|| ProgressStepsPresentation::for_width(width))
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ProgressStepsState) {
        if area.is_empty() {
            return;
        }
        let pres = self.presentation(state, area.width);

        if matches!(pres, ProgressStepsPresentation::Summary) {
            let line = ProgressStepsState::summary_line(self.steps);
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(&line, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                self.system.style(Role::Text),
            );
            return;
        }

        let mut y = area.y;
        if let Some(title) = self.title {
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(title, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                self.system
                    .style(Role::TextStrong)
                    .add_modifier(Modifier::BOLD),
            );
            y = y.saturating_add(1);
        }

        let footer = u16::from(
            matches!(state.mode, ProgressStepsMode::Interactive)
                && state.show_hint
                && state.focused
                && area.height > 2,
        );
        let list_bottom = area.bottom().saturating_sub(footer);
        let expanded = matches!(pres, ProgressStepsPresentation::Expanded);
        let row_h = if expanded { 2u16 } else { 1u16 };

        if matches!(state.mode, ProgressStepsMode::Interactive) {
            state.ensure_cursor(self.steps);
        }

        // Scroll so cursor visible
        if let Some(ref c) = state.cursor {
            if let Some(idx) = self.steps.iter().position(|s| &s.id == c) {
                let page = ((list_bottom.saturating_sub(y)) / row_h.max(1)) as usize;
                let page = page.max(1);
                if idx < state.scroll {
                    state.scroll = idx;
                } else if idx >= state.scroll.saturating_add(page) {
                    state.scroll = idx.saturating_sub(page.saturating_sub(1));
                }
            }
        }

        let mut idx = state.scroll;
        while y + row_h.saturating_sub(1) < list_bottom && idx < self.steps.len() {
            let step = &self.steps[idx];
            let selected = state.cursor.as_ref() == Some(&step.id)
                && matches!(state.mode, ProgressStepsMode::Interactive)
                && state.focused;
            let mark = step.status.mark();
            let dur = step.duration_ms.map(format_duration_ms).unwrap_or_default();
            let verb = step.effective_verb();
            let line = if expanded {
                format!("{mark} {} · {verb}", step.title)
            } else {
                let mut l = format!("{mark} {}", step.title);
                if !dur.is_empty() {
                    l = format!("{l}  {dur}");
                }
                l
            };
            // A selected step keeps its status tone; selection is a wash.
            let style = if selected {
                self.system
                    .style(step.status.semantic().role())
                    .patch(self.system.style(Role::SelectionTint))
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(step.status.semantic().role())
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                style,
            );
            if expanded && row_h > 1 {
                let mut detail = step.detail.clone().unwrap_or_default();
                if detail.is_empty() {
                    if let Some(src) = &step.source {
                        detail = src.clone();
                    }
                }
                if !dur.is_empty() {
                    if detail.is_empty() {
                        detail = dur;
                    } else {
                        detail = format!("{detail} · {dur}");
                    }
                }
                if step.retryable
                    && matches!(
                        step.status,
                        ProgressStepStatus::Failed | ProgressStepStatus::Cancelled
                    )
                {
                    let retry = { " ↻ retry" };
                    detail = format!("{detail}{retry}");
                }
                if !detail.is_empty() {
                    buffer.set_stringn(
                        area.x.saturating_add(4),
                        y + 1,
                        take_display_cols(&detail, usize::from(area.width.saturating_sub(4)))
                            .as_ref(),
                        usize::from(area.width.saturating_sub(4)),
                        self.system.style(Role::TextMuted),
                    );
                }
            }
            y = y.saturating_add(row_h);
            idx += 1;
        }

        if footer > 0 && y < area.bottom() {
            ratatui_core::widgets::Widget::render(
                &HintBar::new(PROGRESS_STEPS_HINTS, self.system),
                Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
                buffer,
            );
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &ProgressStepsState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() {
            return;
        }
        let (done, total, failed) = ProgressStepsState::counts(self.steps);
        let desc = format!(
            "progress-steps mode={} done={done}/{total} failed={failed} cursor={}",
            state.mode.id(),
            state.cursor().unwrap_or("-"),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::List)
                .label("progress-steps")
                .description(desc)
                .focusable(matches!(state.mode, ProgressStepsMode::Interactive) && state.focused)
                .state(SemanticState {
                    busy: ProgressStepsState::active_step(self.steps).is_some(),
                    selected: state.focused,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for ProgressSteps<'_> {
    type State = ProgressStepsState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for &ProgressSteps<'_> {
    type State = ProgressStepsState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

fn format_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let s = ms / 1000;
        format!("{}m{:02}s", s / 60, s % 60)
    }
}

// ── Composition helpers ─────────────────────────────────────────────────────

/// Project steps into [`TimelineEvent`] borrows (host must keep steps alive).
#[must_use]
pub fn progress_steps_as_timeline_events(steps: &[ProgressStep]) -> Vec<TimelineEvent<'_>> {
    steps
        .iter()
        .map(|s| {
            let mut ev = TimelineEvent::new(s.status.id(), s.title.as_str());
            if s.status.is_active() {
                ev = ev.active();
            }
            ev
        })
        .collect()
}

/// Paint steps through [`Timeline`] (passive composition).
pub fn paint_progress_steps_as_timeline(
    steps: &[ProgressStep],
    area: Rect,
    buffer: &mut Buffer,
    state: &mut crate::widgets::TimelineState<()>,
    system: &DesignSystem,
) {
    let events = progress_steps_as_timeline_events(steps);
    Timeline::new(&events, system).paint(area, buffer, state);
}

/// Project to list rows for List / legacy rail hosts (prefer [`ActivityModel`](super::ActivityModel) + TaskRail).
///
/// **Note:** `label`/`trailing` use owned strings via `Line::from`; callers
/// typically rebuild rows each frame from live steps.
#[must_use]
pub fn progress_steps_as_list_rows(steps: &[ProgressStep]) -> Vec<ListRow<'static, String>> {
    steps
        .iter()
        .map(|s| {
            let mut row = ListRow::item(
                s.id.clone(),
                ratatui_core::text::Line::from(s.title.clone()),
            );
            row.status = Some(ratatui_core::text::Line::from(format!(
                "| {} {}",
                s.status.semantic().glyph_unicode(),
                s.status.default_verb()
            )));
            if let Some(d) = &s.detail {
                row.secondary = Some(ratatui_core::text::Line::from(d.clone()));
            }
            if let Some(ms) = s.duration_ms {
                row.badge = Some(ratatui_core::text::Line::from(format_duration_ms(ms)));
            }
            row.enabled = !matches!(s.status, ProgressStepStatus::Cancelled);
            row
        })
        .collect()
}

// ── Example data ────────────────────────────────────────────────────────────

/// Demo CI-style pipeline.
#[must_use]
pub fn example_build_pipeline() -> Vec<ProgressStep> {
    vec![
        ProgressStep::new("fetch", "Fetch")
            .status(ProgressStepStatus::Complete)
            .verb("fetched")
            .duration_ms(420),
        ProgressStep::new("deps", "Resolve deps")
            .status(ProgressStepStatus::Complete)
            .duration_ms(1_200),
        ProgressStep::new("compile", "Compile")
            .status(ProgressStepStatus::Running)
            .verb("compiling")
            .detail("crates/termrock")
            .duration_ms(8_400),
        ProgressStep::new("test", "Test")
            .status(ProgressStepStatus::Queued)
            .detail("waiting on compile"),
        ProgressStep::new("package", "Package").status(ProgressStepStatus::Queued),
    ]
}

/// Demo agent plan with failure + retry.
#[must_use]
pub fn example_agent_plan_steps() -> Vec<ProgressStep> {
    vec![
        ProgressStep::new("plan", "Plan")
            .status(ProgressStepStatus::Complete)
            .duration_ms(900),
        ProgressStep::new("edit", "Edit files")
            .status(ProgressStepStatus::Warning)
            .detail("3 files · 1 low-confidence")
            .duration_ms(12_000),
        ProgressStep::new("build", "Build")
            .status(ProgressStepStatus::Failed)
            .verb("failed")
            .detail("error[E0308]")
            .retryable(true)
            .duration_ms(3_100),
        ProgressStep::new("verify", "Verify").status(ProgressStepStatus::Cancelled),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    #[test]
    fn status_marks_are_non_color() {
        for s in [
            ProgressStepStatus::Queued,
            ProgressStepStatus::Running,
            ProgressStepStatus::Waiting,
            ProgressStepStatus::Complete,
            ProgressStepStatus::Skipped,
            ProgressStepStatus::Warning,
            ProgressStepStatus::Failed,
            ProgressStepStatus::Retrying,
            ProgressStepStatus::Cancelled,
        ] {
            assert!(!s.mark().is_empty());
            assert!(!s.id().is_empty());
            assert!(
                !s.mark().contains('['),
                "progress marks are glyph catalog, not checkbox wells: {:?}",
                s.mark()
            );
        }
        assert_eq!(
            ProgressStepStatus::Complete.mark(),
            Glyph::Success.resolve().text
        );
        assert_eq!(
            ProgressStepStatus::Running.mark(),
            Glyph::SelectionMarker.resolve().text
        );
        assert_eq!(
            ProgressStepStatus::Failed.mark(),
            Glyph::Error.resolve().text
        );
        assert_eq!(
            ProgressStepStatus::Skipped.mark(),
            Glyph::Remove.resolve().text
        );
        assert_eq!(
            ProgressStepStatus::Waiting.mark(),
            Glyph::Ellipsis.resolve().text
        );
    }

    #[test]
    fn summary_contracts_on_narrow() {
        let steps = example_build_pipeline();
        let line = ProgressStepsState::summary_line(&steps);
        assert!(line.contains('/'), "{line}");
        assert!(
            line.contains("compil") || line.contains("running"),
            "{line}"
        );
        assert_eq!(
            ProgressStepsPresentation::for_width(20),
            ProgressStepsPresentation::Summary
        );
        assert_eq!(
            ProgressStepsPresentation::for_width(30),
            ProgressStepsPresentation::Compact
        );
        assert_eq!(
            ProgressStepsPresentation::for_width(50),
            ProgressStepsPresentation::Expanded
        );
    }

    #[test]
    fn passive_ignores_keys() {
        let steps = example_build_pipeline();
        let mut state = ProgressStepsState::new();
        assert!(matches!(
            state.handle_key(&steps, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ProgressStepsOutcome::Ignored
        ));
    }

    #[test]
    fn interactive_nav_and_retry() {
        let steps = example_agent_plan_steps();
        let mut state = ProgressStepsState::interactive();
        state.set_cursor(Some("build".into()));
        assert!(matches!(
            state.handle_key(
                &steps,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            ProgressStepsOutcome::RetryRequested { id } if id == "build"
        ));
        let _ = state.handle_key(&steps, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert!(state.cursor().is_some());
        assert!(matches!(
            state.handle_key(&steps, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            ProgressStepsOutcome::Blurred
        ));
    }

    #[test]
    fn disabled_state_ignores_key_intent_and_mouse() {
        let steps = example_build_pipeline();
        let mut state = ProgressStepsState::interactive();
        state.set_cursor(Some("compile".into()));
        state.enabled = false;

        assert_eq!(
            state.handle_key(&steps, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(
            state.handle_intent(&steps, UiIntent::Move(NavigationMove::Next)),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(
            state.handle_intent(&steps, UiIntent::Activate),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(
            state.handle_intent(&steps, UiIntent::Cancel),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(
            state.handle_mouse(
                &steps,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(0, 1),
                    modifiers: KeyModifiers::NONE,
                },
                Rect::new(0, 0, 20, 4),
                1,
            ),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(state.cursor(), Some("compile"));
        assert!(state.focused);
    }

    #[test]
    fn accepts_input_false_ignores_mouse_and_intent() {
        let steps = example_build_pipeline();
        let mut state = ProgressStepsState::interactive();
        state.set_cursor(Some("compile".into()));
        state.accepts_input = false;

        assert_eq!(
            state.handle_mouse(
                &steps,
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(0, 1),
                    modifiers: KeyModifiers::NONE,
                },
                Rect::new(0, 0, 20, 4),
                1,
            ),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(
            state.handle_intent(&steps, UiIntent::Cancel),
            ProgressStepsOutcome::Ignored
        );
        assert_eq!(state.cursor(), Some("compile"));
        assert!(state.focused);
    }

    #[test]
    fn paint_expanded_and_summary() {
        let system = DesignSystem::default();
        let steps = example_build_pipeline();
        let mut state = ProgressStepsState::new();
        let area = Rect::new(0, 0, 48, 12);
        let mut buf = Buffer::empty(area);
        ProgressSteps::new(&steps, &system)
            .title("Build")
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("Compile") || text.contains("Fetch"), "{text}");

        let mut state = ProgressStepsState::new();
        state.set_presentation(Some(ProgressStepsPresentation::Summary));
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        ProgressSteps::new(&steps, &system).paint(Rect::new(0, 0, 20, 1), &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains('/'), "{text}");
    }

    #[test]
    fn maps_to_step_status_for_stepper_hosts() {
        assert_eq!(
            ProgressStepStatus::Running.to_step_status(),
            StepStatus::Current
        );
        assert_eq!(
            ProgressStepStatus::Complete.to_step_status(),
            StepStatus::Complete
        );
        assert_eq!(
            ProgressStepStatus::Failed.to_step_status(),
            StepStatus::Error
        );
    }

    #[test]
    fn timeline_composition_paints() {
        let system = DesignSystem::default();
        let steps = example_build_pipeline();
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        let mut state = crate::widgets::TimelineState::new();
        paint_progress_steps_as_timeline(&steps, area, &mut buf, &mut state, &system);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Compile") || text.contains("●") || text.contains("○"),
            "{text}"
        );
    }

    #[test]
    fn list_rows_projection() {
        let steps = example_build_pipeline();
        let rows = progress_steps_as_list_rows(&steps);
        assert_eq!(rows.len(), steps.len());
        assert_eq!(rows[0].id, "fetch");
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let steps = example_build_pipeline();
        let state = ProgressStepsState::new();
        let mut scene = SemanticScene::<&str, ()>::default();
        ProgressSteps::new(&steps, &system).register_semantic(
            &mut scene,
            "ps",
            Rect::new(0, 0, 40, 10),
            &state,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("progress-steps"))
        );
    }

    #[test]
    fn counts_active() {
        let steps = example_build_pipeline();
        let (done, total, failed) = ProgressStepsState::counts(&steps);
        assert_eq!(total, 5);
        assert!(done >= 2);
        assert_eq!(failed, 0);
        assert!(ProgressStepsState::active_step(&steps).is_some());
    }

    #[test]
    fn fuzz_keys_interactive() {
        let steps = example_agent_plan_steps();
        let mut state = ProgressStepsState::interactive();
        let keys = [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Enter,
            KeyCode::Char('r'),
            KeyCode::Esc,
            KeyCode::Char('j'),
        ];
        let mut seed = 11u64;
        for _ in 0..120 {
            if !state.focused {
                state.set_focused(true);
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(&steps, KeyEvent::new(k, KeyModifiers::NONE));
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut steps = Vec::new();
        for i in 0..40 {
            steps.push(
                ProgressStep::new(format!("s{i}"), format!("Step {i}")).status(if i < 10 {
                    ProgressStepStatus::Complete
                } else if i == 10 {
                    ProgressStepStatus::Running
                } else {
                    ProgressStepStatus::Queued
                }),
            );
        }
        let mut state = ProgressStepsState::new();
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            terminal
                .draw(|f| {
                    ProgressSteps::new(&steps, &system).paint(f.area(), f.buffer_mut(), &mut state);
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let paint = || {
            let mut t = Terminal::new(TestBackend::new(40, 10)).unwrap();
            let steps = example_build_pipeline();
            let mut state = ProgressStepsState::new();
            t.draw(|f| {
                ProgressSteps::new(&steps, &system).title("CI").paint(
                    f.area(),
                    f.buffer_mut(),
                    &mut state,
                );
            })
            .unwrap();
            t.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }
}
