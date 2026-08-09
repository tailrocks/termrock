// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral agent composition blocks: modes, questions, plan review,
//! task rail, session picker. Domain wording and effects stay consumer-owned.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    text::Line,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode,
        KeyEvent,
        KeyEventKind,
    },
    interaction::Outcome,
    style::{
        DesignTokens,
        Role,
    },
    text::take_display_cols,
    widgets::{
        List,
        ListRow,
        ListState,
        Panel,
        PanelEmphasis,
    },
};

// ── Mode ribbon ─────────────────────────────────────────────────────────────

/// One caller-defined mode (plan/build/ask/… — labels are projections).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkbenchMode<'a, Id> {
    /// Stable mode identity.
    pub id: Id,
    /// Visible label.
    pub label: &'a str,
    /// Whether this mode is currently active.
    pub active: bool,
    /// Whether the mode may be selected.
    pub enabled: bool,
}

/// Horizontal mode strip (product-neutral ribbon).
#[derive(Debug, Clone, Copy)]
pub struct ModeRibbon<'a, Id> {
    modes: &'a [WorkbenchMode<'a, Id>],
    tokens: &'a DesignTokens,
}

impl<'a, Id> ModeRibbon<'a, Id> {
    /// Creates a mode ribbon from borrowed modes.
    #[must_use]
    pub const fn new(modes: &'a [WorkbenchMode<'a, Id>], tokens: &'a DesignTokens) -> Self {
        Self { modes, tokens }
    }
}

/// Outcomes from mode ribbon interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModeRibbonOutcome<Id> {
    /// No change.
    Ignored,
    /// Consumer should switch mode (no effect here).
    ModeRequested(Id),
}

/// Runtime state for mode ribbon focus/selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeRibbonState<Id> {
    selected: Option<Id>,
    focused: bool,
}

impl<Id> Default for ModeRibbonState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            focused: false,
        }
    }
}

impl<Id: Clone + PartialEq> ModeRibbonState<Id> {
    /// Creates state with an optional selected mode.
    #[must_use]
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            focused: true,
        }
    }

    /// Selected mode id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Sets focus for keyboard routing.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Routes left/right/enter.
    pub fn handle_key(
        &mut self,
        modes: &[WorkbenchMode<'_, Id>],
        key: KeyEvent,
    ) -> ModeRibbonOutcome<Id> {
        if !self.focused || key.kind != KeyEventKind::Press {
            return ModeRibbonOutcome::Ignored;
        }
        let enabled: Vec<usize> = modes
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.enabled.then_some(i))
            .collect();
        if enabled.is_empty() {
            return ModeRibbonOutcome::Ignored;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|id| modes.iter().position(|m| &m.id == id))
            .and_then(|i| enabled.iter().position(|&e| e == i))
            .unwrap_or(0);
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                let next = if cur == 0 { enabled.len() - 1 } else { cur - 1 };
                self.selected = Some(modes[enabled[next]].id.clone());
                ModeRibbonOutcome::ModeRequested(modes[enabled[next]].id.clone())
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let next = (cur + 1) % enabled.len();
                self.selected = Some(modes[enabled[next]].id.clone());
                ModeRibbonOutcome::ModeRequested(modes[enabled[next]].id.clone())
            }
            KeyCode::Enter => {
                if let Some(id) = self.selected.clone() {
                    ModeRibbonOutcome::ModeRequested(id)
                } else {
                    let id = modes[enabled[0]].id.clone();
                    self.selected = Some(id.clone());
                    ModeRibbonOutcome::ModeRequested(id)
                }
            }
            _ => ModeRibbonOutcome::Ignored,
        }
    }
}

impl<Id: Clone + PartialEq> Widget for &ModeRibbon<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let mut x = area.x;
        for mode in self.modes {
            if x >= area.right() {
                break;
            }
            let label = if mode.active {
                format!("[{}]", mode.label)
            } else {
                format!(" {} ", mode.label)
            };
            let style = if !mode.enabled {
                self.tokens.theme.style(Role::TextDisabled)
            } else if mode.active {
                self.tokens.theme.style(Role::Accent)
            } else {
                self.tokens.theme.style(Role::TextMuted)
            };
            let clipped = take_display_cols(&label, usize::from(area.right().saturating_sub(x)));
            let w = u16::try_from(clipped.chars().count().max(clipped.len().min(12))).unwrap_or(12);
            let w = w.min(area.right().saturating_sub(x));
            buffer.set_stringn(x, area.y, &clipped, usize::from(w), style);
            x = x.saturating_add(w).saturating_add(1);
        }
    }
}

impl<Id: Clone + PartialEq> Widget for ModeRibbon<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

// ── Question flow ───────────────────────────────────────────────────────────

/// One interview option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionOption<'a, Id> {
    /// Stable option id.
    pub id: Id,
    /// Visible label.
    pub label: &'a str,
}

/// One step in a multi-step question flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestionStep<'a, Id> {
    /// Step identity.
    pub id: Id,
    /// Question text.
    pub prompt: &'a str,
    /// Options for this step.
    pub options: &'a [QuestionOption<'a, Id>],
    /// Whether an answer is required before next.
    pub required: bool,
}

/// Multi-step agent interview (product-neutral).
///
/// Step ids and option ids share the same type parameter for simplicity;
/// consumers can use distinct string namespaces (`"step:…"` / `"opt:…"`).
#[derive(Debug, Clone, Copy)]
pub struct QuestionFlow<'a, Id> {
    steps: &'a [QuestionStep<'a, Id>],
    tokens: &'a DesignTokens,
}

impl<'a, Id> QuestionFlow<'a, Id> {
    /// Creates a question flow from borrowed steps.
    #[must_use]
    pub const fn new(steps: &'a [QuestionStep<'a, Id>], tokens: &'a DesignTokens) -> Self {
        Self { steps, tokens }
    }
}

/// Question flow outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestionFlowOutcome<Id> {
    /// Ignored.
    Ignored,
    /// Option selected on current step.
    Answered {
        /// Step identity.
        step: Id,
        /// Chosen option identity.
        option: Id,
    },
    /// Moved back.
    Back,
    /// Skipped current (if allowed).
    Skip,
    /// All steps complete.
    Finished,
}

/// Runtime state for question flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionFlowState<Id> {
    step_index: usize,
    option_index: usize,
    /// Answers by step index (option id).
    answers: Vec<Option<Id>>,
    focused: bool,
}

impl<Id> Default for QuestionFlowState<Id> {
    fn default() -> Self {
        Self {
            step_index: 0,
            option_index: 0,
            answers: Vec::new(),
            focused: true,
        }
    }
}

impl<Id: Clone> QuestionFlowState<Id> {
    /// Creates empty flow state sized for `step_count`.
    #[must_use]
    pub fn new(step_count: usize) -> Self {
        Self {
            step_index: 0,
            option_index: 0,
            answers: vec![None; step_count],
            focused: true,
        }
    }

    /// Current step index.
    #[must_use]
    pub const fn step_index(&self) -> usize {
        self.step_index
    }

    /// Answer for step if any.
    #[must_use]
    pub fn answer(&self, step: usize) -> Option<&Id> {
        self.answers.get(step).and_then(|a| a.as_ref())
    }

    /// Focus flag.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Keyboard routing.
    pub fn handle_key(
        &mut self,
        steps: &[QuestionStep<'_, Id>],
        key: KeyEvent,
    ) -> QuestionFlowOutcome<Id>
    where
        Id: PartialEq,
    {
        if !self.focused || key.kind != KeyEventKind::Press || steps.is_empty() {
            return QuestionFlowOutcome::Ignored;
        }
        let step = &steps[self.step_index.min(steps.len() - 1)];
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if !step.options.is_empty() {
                    self.option_index = self.option_index.saturating_sub(1);
                }
                QuestionFlowOutcome::Ignored
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !step.options.is_empty() {
                    self.option_index = (self.option_index + 1).min(step.options.len() - 1);
                }
                QuestionFlowOutcome::Ignored
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if step.options.is_empty() {
                    return QuestionFlowOutcome::Ignored;
                }
                let opt = step.options[self.option_index.min(step.options.len() - 1)]
                    .id
                    .clone();
                if self.step_index < self.answers.len() {
                    self.answers[self.step_index] = Some(opt.clone());
                }
                QuestionFlowOutcome::Answered {
                    step: step.id.clone(),
                    option: opt,
                }
            }
            KeyCode::Char('[') | KeyCode::Left => {
                if self.step_index > 0 {
                    self.step_index -= 1;
                    self.option_index = 0;
                    QuestionFlowOutcome::Back
                } else {
                    QuestionFlowOutcome::Ignored
                }
            }
            KeyCode::Char(']') | KeyCode::Right => {
                let answered = self
                    .answers
                    .get(self.step_index)
                    .and_then(|a| a.as_ref())
                    .is_some();
                if step.required && !answered {
                    return QuestionFlowOutcome::Ignored;
                }
                if self.step_index + 1 >= steps.len() {
                    return QuestionFlowOutcome::Finished;
                }
                self.step_index += 1;
                self.option_index = 0;
                QuestionFlowOutcome::Skip
            }
            _ => QuestionFlowOutcome::Ignored,
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &QuestionFlow<'_, Id> {
    type State = QuestionFlowState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() || self.steps.is_empty() {
            return;
        }
        let step = &self.steps[state.step_index.min(self.steps.len() - 1)];
        let panel = Panel::new(self.tokens)
            .title("Question")
            .emphasis(if state.focused {
                PanelEmphasis::Focused
            } else {
                PanelEmphasis::Normal
            });
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }
        let progress = format!("{}/{}", state.step_index + 1, self.steps.len());
        buffer.set_stringn(
            inner.x,
            inner.y,
            &progress,
            usize::from(inner.width),
            self.tokens.theme.style(Role::TextMuted),
        );
        if inner.height > 1 {
            let prompt = take_display_cols(step.prompt, usize::from(inner.width));
            buffer.set_stringn(
                inner.x,
                inner.y + 1,
                &prompt,
                usize::from(inner.width),
                self.tokens.theme.style(Role::Text),
            );
        }
        let mut y = inner.y.saturating_add(2);
        for (i, opt) in step.options.iter().enumerate() {
            if y >= inner.bottom() {
                break;
            }
            let marker = if i == state.option_index {
                "› "
            } else {
                "  "
            };
            let line = format!("{marker}{}", opt.label);
            let style = if i == state.option_index {
                self.tokens.theme.style(Role::Accent)
            } else {
                self.tokens.theme.style(Role::Text)
            };
            let clipped = take_display_cols(&line, usize::from(inner.width));
            buffer.set_stringn(inner.x, y, &clipped, usize::from(inner.width), style);
            y = y.saturating_add(1);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for QuestionFlow<'_, Id> {
    type State = QuestionFlowState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

// ── Plan review ─────────────────────────────────────────────────────────────

/// One plan step projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanStep<'a, Id> {
    /// Stable id.
    pub id: Id,
    /// Title.
    pub title: &'a str,
    /// Optional detail.
    pub detail: Option<&'a str>,
    /// Accepted mark.
    pub accepted: bool,
}

/// Plan review surface.
#[derive(Debug, Clone, Copy)]
pub struct PlanReview<'a, Id> {
    steps: &'a [PlanStep<'a, Id>],
    tokens: &'a DesignTokens,
}

impl<'a, Id> PlanReview<'a, Id> {
    /// Creates plan review from steps.
    #[must_use]
    pub const fn new(steps: &'a [PlanStep<'a, Id>], tokens: &'a DesignTokens) -> Self {
        Self { steps, tokens }
    }
}

/// Plan review outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PlanReviewOutcome<Id> {
    /// Ignored.
    Ignored,
    /// Step selected.
    StepSelected(Id),
    /// Accept whole plan.
    Accepted,
    /// Reject whole plan.
    Rejected,
    /// Edit requested for step.
    EditRequested(Id),
}

/// Plan review state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanReviewState<Id> {
    selected: Option<Id>,
    focused: bool,
}

impl<Id> Default for PlanReviewState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            focused: true,
        }
    }
}

impl<Id: Clone + PartialEq> PlanReviewState<Id> {
    /// New state.
    #[must_use]
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            focused: true,
        }
    }

    /// Selected step.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Focus.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Keys: up/down, a accept, r reject, e edit.
    pub fn handle_key(
        &mut self,
        steps: &[PlanStep<'_, Id>],
        key: KeyEvent,
    ) -> PlanReviewOutcome<Id> {
        if !self.focused || key.kind != KeyEventKind::Press || steps.is_empty() {
            return PlanReviewOutcome::Ignored;
        }
        let idx = self
            .selected
            .as_ref()
            .and_then(|id| steps.iter().position(|s| &s.id == id))
            .unwrap_or(0);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                let next = idx.saturating_sub(1);
                self.selected = Some(steps[next].id.clone());
                PlanReviewOutcome::StepSelected(steps[next].id.clone())
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = (idx + 1).min(steps.len() - 1);
                self.selected = Some(steps[next].id.clone());
                PlanReviewOutcome::StepSelected(steps[next].id.clone())
            }
            KeyCode::Char('a') => PlanReviewOutcome::Accepted,
            KeyCode::Char('r') => PlanReviewOutcome::Rejected,
            KeyCode::Char('e') => {
                if let Some(id) = self.selected.clone() {
                    PlanReviewOutcome::EditRequested(id)
                } else {
                    PlanReviewOutcome::EditRequested(steps[idx].id.clone())
                }
            }
            _ => PlanReviewOutcome::Ignored,
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &PlanReview<'_, Id> {
    type State = PlanReviewState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let panel = Panel::new(self.tokens)
            .title("Plan")
            .emphasis(if state.focused {
                PanelEmphasis::Focused
            } else {
                PanelEmphasis::Normal
            });
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        let mut y = inner.y;
        for step in self.steps {
            if y >= inner.bottom() {
                break;
            }
            let selected = state.selected.as_ref() == Some(&step.id);
            let mark = if step.accepted {
                "✓"
            } else if selected {
                "›"
            } else {
                " "
            };
            let line = format!("{mark} {}", step.title);
            let style = if selected {
                self.tokens.theme.style(Role::Accent)
            } else {
                self.tokens.theme.style(Role::Text)
            };
            let clipped = take_display_cols(&line, usize::from(inner.width));
            buffer.set_stringn(inner.x, y, &clipped, usize::from(inner.width), style);
            y = y.saturating_add(1);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for PlanReview<'_, Id> {
    type State = PlanReviewState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

// ── Session picker ──────────────────────────────────────────────────────────

/// One session row projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionItem<'a, Id> {
    /// Stable session id.
    pub id: Id,
    /// Title.
    pub title: &'a str,
    /// Optional preview/time.
    pub meta: Option<&'a str>,
}

/// Session picker over a list projection.
#[derive(Debug, Clone, Copy)]
pub struct SessionPicker<'a, Id> {
    sessions: &'a [SessionItem<'a, Id>],
    tokens: &'a DesignTokens,
}

impl<'a, Id> SessionPicker<'a, Id> {
    /// Creates a session picker.
    #[must_use]
    pub const fn new(sessions: &'a [SessionItem<'a, Id>], tokens: &'a DesignTokens) -> Self {
        Self { sessions, tokens }
    }

    /// Projects sessions into list rows (caller may filter first).
    #[must_use]
    pub fn rows(&self) -> Vec<ListRow<'a, Id>>
    where
        Id: Clone,
    {
        self.sessions
            .iter()
            .map(|s| {
                let mut row = ListRow::item(s.id.clone(), Line::from(s.title));
                if let Some(meta) = s.meta {
                    row.trailing = Some(Line::from(meta));
                }
                row
            })
            .collect()
    }
}

/// Session picker outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionPickerOutcome<Id> {
    /// Ignored.
    Ignored,
    /// Session picked.
    Picked(Id),
    /// Cancelled.
    Cancelled,
    /// Selection moved.
    SelectionChanged,
}

/// Routes keys through list state.
pub fn session_picker_handle_key<Id: Clone + PartialEq>(
    state: &mut ListState<Id>,
    rows: &[ListRow<'_, Id>],
    key: KeyEvent,
) -> SessionPickerOutcome<Id> {
    match state.handle_key(rows, key) {
        Outcome::Activated(id) => SessionPickerOutcome::Picked(id),
        Outcome::Cancelled => SessionPickerOutcome::Cancelled,
        Outcome::Changed => SessionPickerOutcome::SelectionChanged,
        Outcome::Ignored | Outcome::CheckToggled(_) => SessionPickerOutcome::Ignored,
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &SessionPicker<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let rows = self.rows();
        let panel = Panel::new(self.tokens)
            .title("Sessions")
            .emphasis(PanelEmphasis::Focused);
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if !inner.is_empty() {
            StatefulWidget::render(&List::new(&rows, self.tokens), inner, buffer, state);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for SessionPicker<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

// ── Task rail (thin List façade) ────────────────────────────────────────────

/// Task rail is a titled list with composed rows — use [`List`] + panel chrome.
#[derive(Debug, Clone, Copy)]
pub struct TaskRail<'a, Id> {
    rows: &'a [ListRow<'a, Id>],
    tokens: &'a DesignTokens,
    title: &'a str,
}

impl<'a, Id> TaskRail<'a, Id> {
    /// Creates a task rail.
    #[must_use]
    pub const fn new(
        rows: &'a [ListRow<'a, Id>],
        tokens: &'a DesignTokens,
        title: &'a str,
    ) -> Self {
        Self {
            rows,
            tokens,
            title,
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &TaskRail<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let panel = Panel::new(self.tokens)
            .title(self.title)
            .emphasis(if state.is_focused() {
                PanelEmphasis::Focused
            } else {
                PanelEmphasis::Normal
            });
        let inner = panel.inner(area);
        Widget::render(&panel, area, buffer);
        if !inner.is_empty() {
            StatefulWidget::render(&List::new(self.rows, self.tokens), inner, buffer, state);
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for TaskRail<'_, Id> {
    type State = ListState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn mode_ribbon_requests_mode_on_arrows() {
        let tokens = DesignTokens::default();
        let modes = [
            WorkbenchMode {
                id: "plan",
                label: "Plan",
                active: true,
                enabled: true,
            },
            WorkbenchMode {
                id: "build",
                label: "Build",
                active: false,
                enabled: true,
            },
        ];
        let mut state = ModeRibbonState::new(Some("plan"));
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(
            state.handle_key(&modes, key),
            ModeRibbonOutcome::ModeRequested("build")
        );
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        Widget::render(ModeRibbon::new(&modes, &tokens), area, &mut buf);
        let text: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(text.contains("Plan") || text.contains("Build"), "{text:?}");
    }

    #[test]
    fn question_flow_required_blocks_next() {
        let tokens = DesignTokens::default();
        let opts = [
            QuestionOption {
                id: "a",
                label: "Yes",
            },
            QuestionOption {
                id: "b",
                label: "No",
            },
        ];
        let steps = [
            QuestionStep {
                id: "s1",
                prompt: "Continue?",
                options: &opts,
                required: true,
            },
            QuestionStep {
                id: "s2",
                prompt: "Sure?",
                options: &opts,
                required: false,
            },
        ];
        let mut state = QuestionFlowState::<&str>::new(2);
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(
            state.handle_key(&steps, right),
            QuestionFlowOutcome::Ignored
        );
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert!(matches!(
            state.handle_key(&steps, enter),
            QuestionFlowOutcome::Answered { step: "s1", .. }
        ));
        assert_eq!(state.handle_key(&steps, right), QuestionFlowOutcome::Skip);
        let area = Rect::new(0, 0, 30, 8);
        let mut buf = Buffer::empty(area);
        StatefulWidget::render(
            &QuestionFlow::new(&steps, &tokens),
            area,
            &mut buf,
            &mut state,
        );
    }

    #[test]
    fn plan_review_accept_reject() {
        let steps = [
            PlanStep {
                id: "1",
                title: "Read files",
                detail: None,
                accepted: false,
            },
            PlanStep {
                id: "2",
                title: "Edit",
                detail: Some("src/main.rs"),
                accepted: false,
            },
        ];
        let mut state = PlanReviewState::new(Some("1"));
        let a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(state.handle_key(&steps, a), PlanReviewOutcome::Accepted);
        let r = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE);
        assert_eq!(state.handle_key(&steps, r), PlanReviewOutcome::Rejected);
    }
}
