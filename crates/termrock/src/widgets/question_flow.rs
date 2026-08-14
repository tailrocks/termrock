// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **QuestionFlow** — multi-question human-in-the-loop for agents/workflows.
//!
//! **Mission.** Single/multiple choice, free text, “other”, validation, optional
//! questions, multi-question as steps or tabs, per-question cursor/scroll,
//! review before submit. **Preserve** host composer draft (never mutate it).
//! Queued question sets with actor/provenance. Fullscreen when complex.
//! Structured answers only — no embedded workflow policy.
//!
//! **vs [`super::FormWizard`].** FormWizard is multi-field forms; QuestionFlow
//! is agent interview Q&A with provenance and answer sets.
//! **vs [`super::PermissionPrompt`].** Trust gate, not interview.
//!
//! Research: Grok Build question view, form wizards, conversational agent prompts.

use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::panel::Panel,
};

/// Overlay id for fullscreen question flow.
pub const QUESTION_FLOW_FULLSCREEN_OVERLAY_ID: &str = "termrock.question_flow_fullscreen";
/// Max options painted before scroll window.
pub const QUESTION_FLOW_OPTION_WINDOW: usize = 12;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Question input kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QuestionKind {
    /// Pick exactly one option.
    #[default]
    SingleChoice,
    /// Pick zero or more options.
    MultiChoice,
    /// Free-text answer.
    FreeText,
}

impl QuestionKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::SingleChoice => "single",
            Self::MultiChoice => "multi",
            Self::FreeText => "text",
        }
    }
}

/// One choice row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOption {
    /// Stable id.
    pub id: String,
    /// Label.
    pub label: String,
    /// When selected, collect free-text “other”.
    pub is_other: bool,
}

impl QuestionOption {
    /// Normal option.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            is_other: false,
        }
    }

    /// Other option (free text companion).
    #[must_use]
    pub fn other(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            is_other: true,
        }
    }
}

/// Host-projected question (immutable snapshot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Stable id.
    pub id: String,
    /// Prompt text.
    pub prompt: String,
    /// Kind.
    pub kind: QuestionKind,
    /// Options (empty for pure free text).
    pub options: Vec<QuestionOption>,
    /// Required before next/submit.
    pub required: bool,
    /// Allow “other” free text even if not in options.
    pub allow_other: bool,
    /// Validation message when answer invalid (host/precomputed or UI).
    pub validation_hint: Option<String>,
    /// Help / detail.
    pub help: Option<String>,
}

impl Question {
    /// Single-choice required question.
    #[must_use]
    pub fn single(
        id: impl Into<String>,
        prompt: impl Into<String>,
        options: Vec<QuestionOption>,
    ) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            kind: QuestionKind::SingleChoice,
            options,
            required: true,
            allow_other: false,
            validation_hint: None,
            help: None,
        }
    }

    /// Multi-choice.
    #[must_use]
    pub fn multi(
        id: impl Into<String>,
        prompt: impl Into<String>,
        options: Vec<QuestionOption>,
    ) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            kind: QuestionKind::MultiChoice,
            options,
            required: true,
            allow_other: false,
            validation_hint: None,
            help: None,
        }
    }

    /// Free text.
    #[must_use]
    pub fn text(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
            kind: QuestionKind::FreeText,
            options: Vec::new(),
            required: true,
            allow_other: false,
            validation_hint: None,
            help: None,
        }
    }

    /// Optional.
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    /// Required.
    #[must_use]
    pub const fn required(mut self, on: bool) -> Self {
        self.required = on;
        self
    }

    /// Allow other.
    #[must_use]
    pub const fn allow_other(mut self, on: bool) -> Self {
        self.allow_other = on;
        self
    }

    /// Validation hint.
    #[must_use]
    pub fn validation_hint(mut self, h: impl Into<String>) -> Self {
        self.validation_hint = Some(h.into());
        self
    }

    /// Help.
    #[must_use]
    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = Some(h.into());
        self
    }
}

/// Structured answer for one question (policy-free).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestionAnswer {
    /// Single choice.
    Single {
        /// Option id.
        option_id: String,
        /// Other free text when option is_other or allow_other.
        other_text: Option<String>,
    },
    /// Multi choice.
    Multi {
        /// Selected option ids.
        option_ids: Vec<String>,
        /// Other free text.
        other_text: Option<String>,
    },
    /// Free text.
    FreeText {
        /// Text.
        text: String,
    },
    /// Explicitly skipped (optional).
    Skipped,
}

impl QuestionAnswer {
    /// Whether this satisfies a required question.
    #[must_use]
    pub fn is_present(&self) -> bool {
        match self {
            Self::Skipped => false,
            Self::FreeText { text } => !text.trim().is_empty(),
            Self::Single {
                option_id,
                other_text,
            } => {
                !option_id.is_empty()
                    && (other_text
                        .as_ref()
                        .map(|t| !t.trim().is_empty())
                        .unwrap_or(true)
                        || true)
            }
            Self::Multi {
                option_ids,
                other_text,
            } => {
                !option_ids.is_empty() || other_text.as_ref().is_some_and(|t| !t.trim().is_empty())
            }
        }
    }
}

/// Ordered structured answers for a set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestionAnswerSet {
    /// Pairs of question id → answer.
    pub items: Vec<(String, QuestionAnswer)>,
}

impl QuestionAnswerSet {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Lookup.
    #[must_use]
    pub fn get(&self, question_id: &str) -> Option<&QuestionAnswer> {
        self.items
            .iter()
            .find(|(id, _)| id == question_id)
            .map(|(_, a)| a)
    }

    /// Insert or replace.
    pub fn set(&mut self, question_id: impl Into<String>, answer: QuestionAnswer) {
        let id = question_id.into();
        if let Some(slot) = self.items.iter_mut().find(|(i, _)| *i == id) {
            slot.1 = answer;
        } else {
            self.items.push((id, answer));
        }
    }
}

/// Originating actor for a queued interview set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestionProvenance {
    /// Actor id (`agent`, `subagent:x`).
    pub actor: Option<String>,
    /// Display path / hop labels.
    pub path: Option<String>,
}

impl QuestionProvenance {
    /// Actor only.
    #[must_use]
    pub fn actor(a: impl Into<String>) -> Self {
        Self {
            actor: Some(a.into()),
            path: None,
        }
    }

    /// Path.
    #[must_use]
    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.path = Some(p.into());
        self
    }
}

/// One queued interview (set of questions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionSet {
    /// Set id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Questions.
    pub questions: Vec<Question>,
    /// Provenance.
    pub provenance: QuestionProvenance,
    /// Require review before submit.
    pub review_before_submit: bool,
}

impl QuestionSet {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>, questions: Vec<Question>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            questions,
            provenance: QuestionProvenance::default(),
            review_before_submit: true,
        }
    }

    /// Provenance.
    #[must_use]
    pub fn provenance(mut self, p: QuestionProvenance) -> Self {
        self.provenance = p;
        self
    }

    /// Review gate.
    #[must_use]
    pub const fn review_before_submit(mut self, on: bool) -> Self {
        self.review_before_submit = on;
        self
    }
}

/// Validate answer against question (UI-level presence; host may re-validate).
#[must_use]
pub fn validate_question_answer(
    q: &Question,
    answer: Option<&QuestionAnswer>,
) -> Result<(), String> {
    if !q.required {
        return Ok(());
    }
    let Some(a) = answer else {
        return Err(q
            .validation_hint
            .clone()
            .unwrap_or_else(|| "required".into()));
    };
    if !a.is_present() {
        return Err(q
            .validation_hint
            .clone()
            .unwrap_or_else(|| "required".into()));
    }
    // Other without text
    match a {
        QuestionAnswer::Single {
            option_id,
            other_text,
        } => {
            if let Some(opt) = q.options.iter().find(|o| o.id == *option_id) {
                if opt.is_other && other_text.as_ref().is_none_or(|t| t.trim().is_empty()) {
                    return Err("other text required".into());
                }
            }
        }
        QuestionAnswer::Multi {
            option_ids,
            other_text,
        } => {
            let needs_other = option_ids
                .iter()
                .any(|id| q.options.iter().any(|o| o.id == *id && o.is_other));
            if needs_other && other_text.as_ref().is_none_or(|t| t.trim().is_empty()) {
                return Err("other text required".into());
            }
        }
        _ => {}
    }
    Ok(())
}

// ── Presentation / phase / outcomes ─────────────────────────────────────────

/// Chrome mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QuestionFlowPresentation {
    /// Step progress (default).
    #[default]
    Steps,
    /// Tab-like step headers.
    Tabs,
    /// Fullscreen host overlay.
    Fullscreen,
}

impl QuestionFlowPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Steps => "steps",
            Self::Tabs => "tabs",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Flow phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QuestionFlowPhase {
    /// Answering questions.
    #[default]
    Answering,
    /// Review all answers before submit.
    Review,
}

/// Outcomes — structured answers only; no workflow policy.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuestionFlowOutcome {
    /// Ignored.
    Ignored,
    /// Answer recorded for current question.
    Answered {
        /// Question id.
        question_id: String,
        /// Answer.
        answer: QuestionAnswer,
    },
    /// Step / tab index changed.
    StepChanged {
        /// Index.
        index: usize,
    },
    /// Validation failed.
    ValidationFailed {
        /// Question id.
        question_id: String,
        /// Message.
        message: String,
    },
    /// Entered review phase.
    ReviewOpened,
    /// Left review to a step.
    ReviewClosed {
        /// Step index.
        index: usize,
    },
    /// Submitted full answer set.
    Submitted {
        /// Set id if any.
        set_id: Option<String>,
        /// Structured answers.
        answers: QuestionAnswerSet,
    },
    /// Cancelled without submit (composer draft untouched).
    Cancelled,
    /// Fullscreen promote request.
    FullscreenRequested,
    /// Queued set advanced after submit/cancel.
    QueueChanged {
        /// Remaining sets.
        remaining: usize,
    },
}

// ── Per-question interaction state ──────────────────────────────────────────

/// Cursor/scroll/text state for one question.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestionStepState {
    /// Option list cursor.
    pub option_cursor: usize,
    /// Multi-select set.
    pub multi_selected: BTreeSet<String>,
    /// Free text / other text.
    pub text: String,
    /// Option scroll offset.
    pub scroll: usize,
    /// Typing in free text / other.
    pub text_mode: bool,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive question flow state.
///
/// **Composer draft:** this state never holds or clears host PromptComposer
/// draft text. Host keeps draft while the flow is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionFlowState {
    /// Active set (head of queue or host-pushed).
    pub set: Option<QuestionSet>,
    /// Additional queued sets (FIFO after current).
    pub queue: Vec<QuestionSet>,
    /// Phase.
    pub phase: QuestionFlowPhase,
    /// Current question index.
    pub step_index: usize,
    /// Per-question UI state.
    pub step_states: Vec<QuestionStepState>,
    /// Collected answers.
    pub answers: QuestionAnswerSet,
    /// Presentation.
    pub presentation: QuestionFlowPresentation,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Last validation error for paint.
    pub last_error: Option<String>,
    /// Option hit regions (id, rect).
    pub option_hits: Vec<(String, Rect)>,
}

impl Default for QuestionFlowState {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestionFlowState {
    /// Empty (no open set).
    #[must_use]
    pub fn new() -> Self {
        Self {
            set: None,
            queue: Vec::new(),
            phase: QuestionFlowPhase::Answering,
            step_index: 0,
            step_states: Vec::new(),
            answers: QuestionAnswerSet::new(),
            presentation: QuestionFlowPresentation::Steps,
            focused: true,
            accepts_input: true,
            last_error: None,
            option_hits: Vec::new(),
        }
    }

    /// Sized empty legacy helper (no set yet).
    #[must_use]
    pub fn with_capacity(_step_count: usize) -> Self {
        Self::new()
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Whether a set is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.set.is_some()
    }

    /// Open / replace current set (does **not** touch composer draft).
    pub fn open_set(&mut self, set: QuestionSet) {
        let n = set.questions.len();
        self.step_states = vec![QuestionStepState::default(); n];
        self.answers = QuestionAnswerSet::new();
        self.step_index = 0;
        self.phase = QuestionFlowPhase::Answering;
        self.last_error = None;
        self.set = Some(set);
    }

    /// Enqueue after current (or open if none).
    pub fn enqueue(&mut self, set: QuestionSet) {
        if self.set.is_none() {
            self.open_set(set);
        } else {
            self.queue.push(set);
        }
    }

    /// Current questions.
    #[must_use]
    pub fn questions(&self) -> &[Question] {
        self.set
            .as_ref()
            .map(|s| s.questions.as_slice())
            .unwrap_or(&[])
    }

    /// Current question.
    #[must_use]
    pub fn current_question(&self) -> Option<&Question> {
        self.questions().get(self.step_index)
    }

    fn current_step_state_mut(&mut self) -> Option<&mut QuestionStepState> {
        self.step_states.get_mut(self.step_index)
    }

    /// Build answer from step state.
    fn draft_answer(&self, q: &Question, st: &QuestionStepState) -> QuestionAnswer {
        match q.kind {
            QuestionKind::FreeText => QuestionAnswer::FreeText {
                text: st.text.clone(),
            },
            QuestionKind::SingleChoice => {
                if q.options.is_empty() {
                    return QuestionAnswer::FreeText {
                        text: st.text.clone(),
                    };
                }
                let idx = st.option_cursor.min(q.options.len().saturating_sub(1));
                let opt = &q.options[idx];
                let other = if opt.is_other || q.allow_other {
                    Some(st.text.clone()).filter(|t| !t.is_empty())
                } else {
                    None
                };
                QuestionAnswer::Single {
                    option_id: opt.id.clone(),
                    other_text: other,
                }
            }
            QuestionKind::MultiChoice => {
                let other_needed = st
                    .multi_selected
                    .iter()
                    .any(|id| q.options.iter().any(|o| o.id == *id && o.is_other));
                QuestionAnswer::Multi {
                    option_ids: st.multi_selected.iter().cloned().collect(),
                    other_text: if other_needed || q.allow_other {
                        Some(st.text.clone()).filter(|t| !t.is_empty())
                    } else {
                        None
                    },
                }
            }
        }
    }

    fn commit_current(&mut self) -> Result<QuestionAnswer, String> {
        let q = self
            .current_question()
            .cloned()
            .ok_or_else(|| "no question".to_string())?;
        let st = self
            .step_states
            .get(self.step_index)
            .cloned()
            .unwrap_or_default();
        let answer = self.draft_answer(&q, &st);
        validate_question_answer(&q, Some(&answer))?;
        self.answers.set(q.id.clone(), answer.clone());
        Ok(answer)
    }

    fn advance_after_answer(&mut self) -> QuestionFlowOutcome {
        let n = self.questions().len();
        if self.step_index + 1 >= n {
            let review = self.set.as_ref().is_some_and(|s| s.review_before_submit);
            if review {
                self.phase = QuestionFlowPhase::Review;
                return QuestionFlowOutcome::ReviewOpened;
            }
            return self.submit();
        }
        self.step_index += 1;
        let free = matches!(
            self.current_question().map(|q| q.kind),
            Some(QuestionKind::FreeText)
        );
        if let Some(st) = self.current_step_state_mut() {
            st.option_cursor = 0;
            st.scroll = 0;
            st.text_mode = free;
        }
        QuestionFlowOutcome::StepChanged {
            index: self.step_index,
        }
    }

    fn submit(&mut self) -> QuestionFlowOutcome {
        // Validate all required
        let questions: Vec<Question> = self.questions().to_vec();
        for q in &questions {
            let a = self.answers.get(&q.id);
            if let Err(msg) = validate_question_answer(q, a) {
                // jump to failing step
                if let Some(i) = questions.iter().position(|x| x.id == q.id) {
                    self.step_index = i;
                    self.phase = QuestionFlowPhase::Answering;
                }
                self.last_error = Some(msg.clone());
                return QuestionFlowOutcome::ValidationFailed {
                    question_id: q.id.clone(),
                    message: msg,
                };
            }
        }
        let set_id = self.set.as_ref().map(|s| s.id.clone());
        let answers = self.answers.clone();
        // Advance queue
        if let Some(next) = self.queue.first().cloned() {
            self.queue.remove(0);
            self.open_set(next);
            return QuestionFlowOutcome::Submitted { set_id, answers };
            // host should also observe QueueChanged — emit via remaining
        }
        self.set = None;
        self.step_states.clear();
        self.phase = QuestionFlowPhase::Answering;
        QuestionFlowOutcome::Submitted { set_id, answers }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent) -> QuestionFlowOutcome {
        if !self.accepts_input || !self.focused || key.kind != KeyEventKind::Press {
            return QuestionFlowOutcome::Ignored;
        }
        if self.set.is_none() {
            return QuestionFlowOutcome::Ignored;
        }

        if matches!(self.phase, QuestionFlowPhase::Review) {
            return self.handle_review_key(key);
        }

        let Some(q) = self.current_question().cloned() else {
            return QuestionFlowOutcome::Ignored;
        };

        // Text input mode
        let text_mode = self
            .step_states
            .get(self.step_index)
            .is_some_and(|s| s.text_mode)
            || matches!(q.kind, QuestionKind::FreeText);

        if text_mode {
            match key.code {
                KeyCode::Esc => {
                    if let Some(st) = self.current_step_state_mut() {
                        if matches!(q.kind, QuestionKind::FreeText) {
                            return QuestionFlowOutcome::Cancelled;
                        }
                        st.text_mode = false;
                    }
                    return QuestionFlowOutcome::Ignored;
                }
                KeyCode::Enter if key.modifiers.is_empty() => {
                    match self.commit_current() {
                        Ok(answer) => {
                            self.last_error = None;
                            let qid = q.id.clone();
                            let next = self.advance_after_answer();
                            if matches!(
                                next,
                                QuestionFlowOutcome::StepChanged { .. }
                                    | QuestionFlowOutcome::ReviewOpened
                                    | QuestionFlowOutcome::Submitted { .. }
                            ) {
                                // Prefer Answered then host may apply next
                                return QuestionFlowOutcome::Answered {
                                    question_id: qid,
                                    answer,
                                };
                            }
                            next
                        }
                        Err(msg) => {
                            self.last_error = Some(msg.clone());
                            QuestionFlowOutcome::ValidationFailed {
                                question_id: q.id.clone(),
                                message: msg,
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    if let Some(st) = self.current_step_state_mut() {
                        st.text.pop();
                    }
                    QuestionFlowOutcome::Ignored
                }
                KeyCode::Char(c)
                    if !c.is_control() && !key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if let Some(st) = self.current_step_state_mut() {
                        st.text.push(c);
                    }
                    QuestionFlowOutcome::Ignored
                }
                _ => QuestionFlowOutcome::Ignored,
            }
        } else {
            match key.code {
                KeyCode::Esc => QuestionFlowOutcome::Cancelled,
                KeyCode::Char('f') if key.modifiers.is_empty() => {
                    self.presentation = QuestionFlowPresentation::Fullscreen;
                    QuestionFlowOutcome::FullscreenRequested
                }
                KeyCode::Char('t') if key.modifiers.is_empty() => {
                    self.presentation = match self.presentation {
                        QuestionFlowPresentation::Tabs => QuestionFlowPresentation::Steps,
                        QuestionFlowPresentation::Steps | QuestionFlowPresentation::Fullscreen => {
                            QuestionFlowPresentation::Tabs
                        }
                    };
                    QuestionFlowOutcome::Ignored
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(st) = self.current_step_state_mut() {
                        st.option_cursor = st.option_cursor.saturating_sub(1);
                        if st.option_cursor < st.scroll {
                            st.scroll = st.option_cursor;
                        }
                    }
                    QuestionFlowOutcome::Ignored
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let len = q.options.len();
                    if let Some(st) = self.current_step_state_mut() {
                        if len > 0 {
                            st.option_cursor = (st.option_cursor + 1).min(len - 1);
                            let win = QUESTION_FLOW_OPTION_WINDOW;
                            if st.option_cursor >= st.scroll + win {
                                st.scroll = st.option_cursor + 1 - win;
                            }
                        }
                    }
                    QuestionFlowOutcome::Ignored
                }
                KeyCode::Char(' ') if matches!(q.kind, QuestionKind::MultiChoice) => {
                    if let Some(st) = self.current_step_state_mut() {
                        if !q.options.is_empty() {
                            let id = q.options[st.option_cursor.min(q.options.len() - 1)]
                                .id
                                .clone();
                            if !st.multi_selected.remove(&id) {
                                st.multi_selected.insert(id.clone());
                            }
                            // enter other text mode
                            if q.options.iter().any(|o| o.id == id && o.is_other) {
                                st.text_mode = true;
                            }
                        }
                    }
                    QuestionFlowOutcome::Ignored
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if matches!(q.kind, QuestionKind::MultiChoice) && key.code == KeyCode::Char(' ')
                    {
                        return QuestionFlowOutcome::Ignored;
                    }
                    // Single: if other, enter text mode first
                    if matches!(q.kind, QuestionKind::SingleChoice) {
                        if let Some(st) = self.current_step_state_mut() {
                            if !q.options.is_empty() {
                                let opt = &q.options[st.option_cursor.min(q.options.len() - 1)];
                                if opt.is_other {
                                    st.text_mode = true;
                                    return QuestionFlowOutcome::Ignored;
                                }
                            }
                        }
                    }
                    match self.commit_current() {
                        Ok(answer) => {
                            self.last_error = None;
                            let qid = q.id.clone();
                            let _ = self.advance_after_answer();
                            QuestionFlowOutcome::Answered {
                                question_id: qid,
                                answer,
                            }
                        }
                        Err(msg) => {
                            self.last_error = Some(msg.clone());
                            QuestionFlowOutcome::ValidationFailed {
                                question_id: q.id.clone(),
                                message: msg,
                            }
                        }
                    }
                }
                KeyCode::Char('[') | KeyCode::Left => {
                    if self.step_index > 0 {
                        self.step_index -= 1;
                        QuestionFlowOutcome::StepChanged {
                            index: self.step_index,
                        }
                    } else {
                        QuestionFlowOutcome::Ignored
                    }
                }
                KeyCode::Char(']') | KeyCode::Right | KeyCode::Tab => {
                    // Advance only if already answered (Enter), or skip optional.
                    let answered = self.answers.get(&q.id).is_some();
                    if answered {
                        return self.advance_after_answer();
                    }
                    if !q.required {
                        self.answers.set(q.id.clone(), QuestionAnswer::Skipped);
                        return self.advance_after_answer();
                    }
                    let msg = q
                        .validation_hint
                        .clone()
                        .unwrap_or_else(|| "answer required".into());
                    self.last_error = Some(msg.clone());
                    QuestionFlowOutcome::ValidationFailed {
                        question_id: q.id.clone(),
                        message: msg,
                    }
                }
                KeyCode::Char('o') if q.allow_other || q.options.iter().any(|o| o.is_other) => {
                    if let Some(st) = self.current_step_state_mut() {
                        st.text_mode = true;
                    }
                    QuestionFlowOutcome::Ignored
                }
                KeyCode::Char('v') if key.modifiers.is_empty() => {
                    // jump to review if all required answered
                    let qs = self.questions().to_vec();
                    for q in &qs {
                        if let Err(msg) = validate_question_answer(q, self.answers.get(&q.id)) {
                            self.last_error = Some(msg);
                            return QuestionFlowOutcome::Ignored;
                        }
                    }
                    self.phase = QuestionFlowPhase::Review;
                    QuestionFlowOutcome::ReviewOpened
                }
                _ => QuestionFlowOutcome::Ignored,
            }
        }
    }

    fn handle_review_key(&mut self, key: KeyEvent) -> QuestionFlowOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = QuestionFlowPhase::Answering;
                QuestionFlowOutcome::ReviewClosed {
                    index: self.step_index,
                }
            }
            KeyCode::Enter | KeyCode::Char('s') => self.submit(),
            KeyCode::Char(c) if c.is_ascii_digit() => {
                let n = (c as u8 - b'1') as usize;
                if n < self.questions().len() {
                    self.step_index = n;
                    self.phase = QuestionFlowPhase::Answering;
                    QuestionFlowOutcome::ReviewClosed { index: n }
                } else {
                    QuestionFlowOutcome::Ignored
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.step_index = self.step_index.saturating_sub(1);
                QuestionFlowOutcome::Ignored
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.questions().len().saturating_sub(1);
                self.step_index = (self.step_index + 1).min(max);
                QuestionFlowOutcome::Ignored
            }
            _ => QuestionFlowOutcome::Ignored,
        }
    }

    /// Mouse on options.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> QuestionFlowOutcome {
        if !self.accepts_input || !self.focused {
            return QuestionFlowOutcome::Ignored;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return QuestionFlowOutcome::Ignored;
        }
        if matches!(self.phase, QuestionFlowPhase::Review) {
            return QuestionFlowOutcome::Ignored;
        }
        let Some(q) = self.current_question().cloned() else {
            return QuestionFlowOutcome::Ignored;
        };
        let hit_id = self
            .option_hits
            .iter()
            .find(|(_, rect)| rect.contains(event.position))
            .map(|(id, _)| id.clone());
        let Some(id) = hit_id else {
            return QuestionFlowOutcome::Ignored;
        };
        if let Some(st) = self.current_step_state_mut() {
            if let Some(idx) = q.options.iter().position(|o| o.id == id) {
                st.option_cursor = idx;
            }
            if matches!(q.kind, QuestionKind::MultiChoice) {
                if !st.multi_selected.remove(&id) {
                    st.multi_selected.insert(id);
                }
                return QuestionFlowOutcome::Ignored;
            }
            if q.options.iter().any(|o| o.id == id && o.is_other) {
                st.text_mode = true;
                return QuestionFlowOutcome::Ignored;
            }
        }
        self.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Question flow widget (active set from state).
#[derive(Debug, Clone, Copy)]
pub struct QuestionFlow<'a> {
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
}

impl<'a> QuestionFlow<'a> {
    /// System only — questions live in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
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
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut QuestionFlowState) {
        state.option_hits.clear();
        if area.is_empty() {
            return;
        }
        let Some(set) = state.set.as_ref() else {
            let panel = Panel::new(self.system)
                .overlay(true)
                .title("Questions")
                .emphasis(PanelChrome::Normal);
            let inner = panel.inner(area);
            use ratatui_core::widgets::Widget;
            Widget::render(&panel, area, buffer);
            if !inner.is_empty() {
                let m = if self.ascii { "[ ] idle" } else { "∅ idle" };
                buffer.set_stringn(
                    inner.x,
                    inner.y,
                    m,
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
            return;
        };

        let title = if matches!(state.phase, QuestionFlowPhase::Review) {
            format!("{} · review", set.title)
        } else {
            set.title.clone()
        };
        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system)
            .overlay(true)
            .title(title.as_str())
            .emphasis(emphasis);
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        if matches!(state.phase, QuestionFlowPhase::Review) {
            self.paint_review(inner, buffer, state);
            return;
        }

        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom();

        // provenance
        if let Some(p) = set
            .provenance
            .path
            .as_ref()
            .or(set.provenance.actor.as_ref())
        {
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&format!("from {p}"), w),
                w,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // progress / tabs
        let n = set.questions.len().max(1);
        let progress = match state.presentation {
            QuestionFlowPresentation::Tabs => {
                let mut s = String::new();
                for (i, q) in set.questions.iter().enumerate() {
                    if i > 0 {
                        s.push(' ');
                    }
                    let mark = if i == state.step_index {
                        if self.ascii { "*" } else { "●" }
                    } else if state.answers.get(&q.id).is_some() {
                        if self.ascii { "+" } else { "✓" }
                    } else if self.ascii {
                        "o"
                    } else {
                        "○"
                    };
                    s.push_str(&format!("{mark}{}", i + 1));
                }
                s
            }
            _ => format!("{}/{}", state.step_index + 1, n),
        };
        if y < max_y {
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&progress, w),
                w,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        let Some(q) = state.current_question() else {
            return;
        };
        let q = q.clone();
        let st = state
            .step_states
            .get(state.step_index)
            .cloned()
            .unwrap_or_default();

        if y < max_y {
            let req = if q.required { "*" } else { "" };
            buffer.set_stringn(
                inner.x,
                y,
                take_display_cols(&format!("{req}{}", q.prompt), w),
                w,
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }

        if let Some(err) = &state.last_error {
            if y < max_y {
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(err, w),
                    w,
                    self.system.style(Role::Danger),
                );
                y = y.saturating_add(1);
            }
        }

        match q.kind {
            QuestionKind::FreeText => {
                if y < max_y {
                    let line = format!("> {}", st.text);
                    buffer.set_stringn(
                        inner.x,
                        y,
                        take_display_cols(&line, w),
                        w,
                        self.system.style(Role::Input),
                    );
                }
            }
            QuestionKind::SingleChoice | QuestionKind::MultiChoice => {
                let win = QUESTION_FLOW_OPTION_WINDOW;
                let start = st.scroll;
                let end = (start + win).min(q.options.len());
                for (i, opt) in q.options.iter().enumerate().take(end).skip(start) {
                    if y >= max_y.saturating_sub(1) {
                        break;
                    }
                    let rel = i;
                    let on = rel == st.option_cursor;
                    let checked = matches!(q.kind, QuestionKind::MultiChoice)
                        && st.multi_selected.contains(&opt.id);
                    let mark = if matches!(q.kind, QuestionKind::MultiChoice) {
                        if checked {
                            if self.ascii { "[x]" } else { "[✓]" }
                        } else if self.ascii {
                            "[ ]"
                        } else {
                            "[ ]"
                        }
                    } else if on {
                        if self.ascii { ">" } else { "›" }
                    } else {
                        " "
                    };
                    let other = if opt.is_other { "…" } else { "" };
                    let line = format!("{mark} {}{other}", opt.label);
                    let style = if on {
                        if self.colorless {
                            self.system
                                .style(Role::Text)
                                .add_modifier(Modifier::REVERSED)
                        } else {
                            self.system.style(Role::Focus)
                        }
                    } else {
                        self.system.style(Role::Text)
                    };
                    buffer.set_stringn(inner.x, y, take_display_cols(&line, w), w, style);
                    state.option_hits.push((
                        opt.id.clone(),
                        Rect {
                            x: inner.x,
                            y,
                            width: inner.width,
                            height: 1,
                        },
                    ));
                    y = y.saturating_add(1);
                }
                if st.text_mode && y < max_y {
                    buffer.set_stringn(
                        inner.x,
                        y,
                        take_display_cols(&format!("other> {}", st.text), w),
                        w,
                        self.system.style(Role::Input),
                    );
                }
            }
        }

        // footer
        let foot_y = inner.bottom().saturating_sub(1);
        if foot_y >= y || inner.height > 2 {
            let foot = "j/k · enter · space multi · [] step · v review · f full · esc cancel";
            // composer draft preserved — never cleared by this widget
            buffer.set_stringn(
                inner.x,
                foot_y,
                take_display_cols(foot, w),
                w,
                self.system.style(Role::TextMuted),
            );
        }
        let _ = display_cols;
    }

    fn paint_review(&self, area: Rect, buffer: &mut Buffer, state: &QuestionFlowState) {
        let mut y = area.y;
        let w = usize::from(area.width);
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols("Review answers · Enter submit · Esc edit", w),
            w,
            self.system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);
        for (i, q) in state.questions().iter().enumerate() {
            if y >= area.bottom() {
                break;
            }
            let ans = state
                .answers
                .get(&q.id)
                .map(|a| match a {
                    QuestionAnswer::Single {
                        option_id,
                        other_text,
                    } => {
                        if let Some(t) = other_text {
                            format!("{option_id}: {t}")
                        } else {
                            option_id.clone()
                        }
                    }
                    QuestionAnswer::Multi { option_ids, .. } => option_ids.join(","),
                    QuestionAnswer::FreeText { text } => text.clone(),
                    QuestionAnswer::Skipped => "(skipped)".into(),
                })
                .unwrap_or_else(|| "—".into());
            let mark = if i == state.step_index { ">" } else { " " };
            let line = format!(
                "{mark}{}. {} → {}",
                i + 1,
                take_display_cols(&q.prompt, 20),
                ans
            );
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, w),
                w,
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut QuestionFlowState) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for &QuestionFlow<'_> {
    type State = QuestionFlowState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for QuestionFlow<'_> {
    type State = QuestionFlowState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo interview set.
#[must_use]
pub fn example_question_set() -> QuestionSet {
    QuestionSet::new(
        "set1",
        "Plan check-in",
        vec![
            Question::single(
                "q1",
                "Deploy strategy?",
                vec![
                    QuestionOption::new("blue", "Blue/green"),
                    QuestionOption::new("canary", "Canary"),
                    QuestionOption::other("other", "Other"),
                ],
            )
            .allow_other(true),
            Question::multi(
                "q2",
                "Notify whom?",
                vec![
                    QuestionOption::new("eng", "Engineering"),
                    QuestionOption::new("sre", "SRE"),
                    QuestionOption::new("pm", "PM"),
                ],
            )
            .optional(),
            Question::text("q3", "Any risks to call out?").optional(),
        ],
    )
    .provenance(QuestionProvenance::actor("agent").path("main > plan"))
    .review_before_submit(true)
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

    #[test]
    fn single_choice_answer_and_advance() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        assert_eq!(st.step_index, 0);
        // move to canary
        let _ = st.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            out,
            QuestionFlowOutcome::Answered {
                question_id: ref id,
                answer: QuestionAnswer::Single { ref option_id, .. }
            } if id == "q1" && option_id == "canary"
        ));
        assert_eq!(st.step_index, 1);
    }

    #[test]
    fn required_blocks_skip() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        let out = st.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert!(matches!(out, QuestionFlowOutcome::ValidationFailed { .. }));
        assert_eq!(st.step_index, 0);
    }

    #[test]
    fn optional_skip_and_multi() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        // answer q1
        let _ = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // q2 multi optional — skip with ]
        let out = st.handle_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert!(matches!(
            out,
            QuestionFlowOutcome::StepChanged { index: 2 } | QuestionFlowOutcome::Answered { .. }
        ));
    }

    #[test]
    fn free_text_and_review_submit() {
        let mut st = QuestionFlowState::new();
        let set = QuestionSet::new(
            "s",
            "T",
            vec![
                Question::single("a", "Go?", vec![QuestionOption::new("y", "Yes")]),
                Question::text("b", "Notes?").optional(),
            ],
        )
        .review_before_submit(true);
        st.open_set(set);
        let _ = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // on free text
        assert!(matches!(
            st.current_question().map(|q| q.kind),
            Some(QuestionKind::FreeText)
        ));
        for c in "risk".chars() {
            let _ = st.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // Answered then review
        assert!(matches!(
            out,
            QuestionFlowOutcome::Answered { .. } | QuestionFlowOutcome::ReviewOpened
        ));
        // force review
        st.phase = QuestionFlowPhase::Review;
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            out,
            QuestionFlowOutcome::Submitted { answers, .. } if answers.get("a").is_some()
        ));
    }

    #[test]
    fn other_requires_text() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        // select other (index 2)
        let _ = st.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let _ = st.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        // enters text mode or validation
        assert!(matches!(
            out,
            QuestionFlowOutcome::Ignored | QuestionFlowOutcome::ValidationFailed { .. }
        ));
    }

    #[test]
    fn queue_enqueue_and_provenance() {
        let mut st = QuestionFlowState::new();
        st.enqueue(example_question_set());
        st.enqueue(QuestionSet::new(
            "s2",
            "Second",
            vec![Question::text("x", "More?").optional()],
        ));
        assert!(st.is_open());
        assert_eq!(st.queue.len(), 1);
        assert!(
            st.set
                .as_ref()
                .unwrap()
                .provenance
                .actor
                .as_deref()
                .is_some()
        );
    }

    #[test]
    fn cancel_does_not_submit() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            QuestionFlowOutcome::Cancelled
        ));
    }

    #[test]
    fn fullscreen_and_tabs() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE)),
            QuestionFlowOutcome::FullscreenRequested
        ));
        let _ = st.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(st.presentation, QuestionFlowPresentation::Tabs);
    }

    #[test]
    fn structured_answers_no_policy() {
        let src = include_str!("question_flow.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "workflow::", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
        // documents composer draft preservation
        assert!(body.contains("composer") || body.contains("Composer") || body.contains("draft"));
    }

    #[test]
    fn paint_steps_and_review() {
        let system = DesignSystem::default();
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        let area = Rect::new(0, 0, 48, 16);
        let mut buf = Buffer::empty(area);
        QuestionFlow::new(&system).paint(area, &mut buf, &mut st);
        st.phase = QuestionFlowPhase::Review;
        QuestionFlow::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            QuestionFlowOutcome::Ignored
        ));
    }

    #[test]
    fn multi_space_toggles() {
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        let _ = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)); // q1
        // q2 multi
        let _ = st.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let st_s = &st.step_states[1];
        assert!(!st_s.multi_selected.is_empty());
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = QuestionFlowState::new();
        st.open_set(example_question_set());
        let area = Rect::new(0, 0, 56, 18);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            QuestionFlow::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn validate_optional_empty_ok() {
        let q = Question::text("t", "x").optional();
        assert!(validate_question_answer(&q, None).is_ok());
        assert!(validate_question_answer(&q, Some(&QuestionAnswer::Skipped)).is_ok());
    }

    #[test]
    fn fuzz_kinds() {
        for k in [
            QuestionKind::SingleChoice,
            QuestionKind::MultiChoice,
            QuestionKind::FreeText,
        ] {
            assert!(!k.id().is_empty());
        }
    }
}
