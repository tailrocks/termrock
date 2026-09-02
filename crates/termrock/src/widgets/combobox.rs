// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Combobox and Autocomplete — editable input + suggestion collection.
//!
//! **Mission.** Forms need free-form or constrained values with async
//! suggestions, without private popup geometry. TermRock owns draft typing,
//! menu navigation via [`CompletionMenu`], and generation-gated suggestion
//! application. Hosts own ranking, fetch, and OverlayStack placement.
//!
//! **Three values (kept separate):**
//! 1. **Draft text** — what the user is typing (`draft()`).
//! 2. **Active suggestion** — keyboard highlight in the menu (`active_suggestion()`).
//! 3. **Committed value** — accepted id/label after Enter/click (`value()`).
//!
//! **vs [`Select`](super::Select).** Select is closed-by-default single choice
//! without free typing as the primary path.
//! **vs [`CompletionMenu`](super::CompletionMenu).** Menu-only; Combobox owns
//! the field + policy and embeds menu state.
//!
//! Research: Radix Combobox, prompt-toolkit completion, editor menus, palettes.
use std::collections::VecDeque;

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
    interaction::{
        OverlayStack, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
};

use super::{
    COMPLETION_OVERLAY_ID, CompletionCandidate, CompletionMenu, CompletionMenuOutcome,
    CompletionMenuSize, CompletionMenuState, TextInput, TextInputOutcome, TextInputState,
    Validation, dismiss_completion_overlay, open_completion_overlay, place_completion_menu,
};

/// Default recent-values capacity.
pub const DEFAULT_COMBO_RECENT_LIMIT: usize = 8;

// ── Mode / status ───────────────────────────────────────────────────────────

/// Combobox vs Autocomplete defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ComboMode {
    /// Filter/select field; creatable optional; often exact-match validation.
    #[default]
    Combobox,
    /// Continuous free text with optional suggestions (creatable default).
    Autocomplete,
}

impl ComboMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Combobox => "combobox",
            Self::Autocomplete => "autocomplete",
        }
    }
}

/// Host-projected suggestion fetch status (no async inside the widget).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SuggestionStatus {
    /// No request / idle closed.
    #[default]
    Idle,
    /// Host is loading suggestions for [`ComboboxState::suggestion_generation`].
    Loading,
    /// Non-empty list applied for current generation.
    Ready,
    /// Applied empty list for current generation.
    Empty,
    /// Host error for current generation.
    Error,
}

impl SuggestionStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Combobox / Autocomplete outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComboboxOutcome<Id> {
    /// No effect.
    Ignored,
    /// Draft text or caret changed; host should fetch for `generation`.
    DraftChanged {
        /// Current draft.
        text: String,
        /// Monotonic generation for race-safe apply.
        generation: u64,
    },
    /// Suggestion menu opened (host should open completion overlay).
    MenuOpened {
        /// Generation that should be loaded.
        generation: u64,
    },
    /// Menu closed without commit.
    MenuClosed,
    /// Active suggestion highlight moved.
    HighlightChanged {
        /// Active candidate id.
        id: Option<Id>,
    },
    /// Committed a candidate from the menu.
    Committed {
        /// Candidate id.
        id: Id,
        /// Label at commit (host may re-resolve).
        label: String,
    },
    /// Committed free text / creatable value (no id).
    Created {
        /// Draft text committed.
        text: String,
    },
    /// Esc / cancel without value change (menu already closed).
    Dismissed,
    /// Focus left the field.
    Blurred,
    /// Exact-value validation failed.
    ValidationFailed {
        /// Human reason.
        reason: &'static str,
    },
    /// Host paste.
    ClipboardPasteRequest,
    /// Host copy.
    ClipboardCopy {
        /// Text.
        text: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state for [`Combobox`] in select or autocomplete mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboboxState<Id: Clone + PartialEq> {
    mode: ComboMode,
    /// Typed text (separate from committed value).
    draft: TextInputState,
    /// Committed id (constrained path).
    value: Option<Id>,
    /// Committed display label.
    value_label: Option<String>,
    /// Whether free-text / creatable commit is allowed.
    creatable: bool,
    /// When true, commit requires exact match against last applied candidates
    /// or an existing value (Combobox default).
    exact_required: bool,
    /// Menu state (CompletionMenu).
    menu: CompletionMenuState<Id>,
    /// Suggestion pipeline status.
    status: SuggestionStatus,
    /// Bumped on each draft change that should trigger fetch.
    generation: u64,
    /// Generation of last successfully applied suggestion batch.
    applied_generation: u64,
    /// Recent commits (newest first).
    recent: VecDeque<(Id, String)>,
    recent_limit: usize,
    focused: bool,
    enabled: bool,
    read_only: bool,
    /// Last known field geometry (anchor for overlay).
    field: Rect,
    /// Host error message projection.
    error_message: Option<String>,
}

impl<Id: Clone + PartialEq> Default for ComboboxState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> ComboboxState<Id> {
    /// Empty combobox (constrained defaults: not creatable, exact preferred).
    #[must_use]
    pub fn new() -> Self {
        let mut draft = TextInputState::new("").with_allow_empty(true);
        draft.set_focused(false);
        Self {
            mode: ComboMode::Combobox,
            draft,
            value: None,
            value_label: None,
            creatable: false,
            exact_required: true,
            menu: CompletionMenuState::new(None),
            status: SuggestionStatus::Idle,
            generation: 0,
            applied_generation: 0,
            recent: VecDeque::new(),
            recent_limit: DEFAULT_COMBO_RECENT_LIMIT,
            focused: false,
            enabled: true,
            read_only: false,
            field: Rect::default(),
            error_message: None,
        }
    }

    /// Autocomplete defaults (creatable, exact not required).
    #[must_use]
    pub fn autocomplete() -> Self {
        Self::new()
            .with_mode(ComboMode::Autocomplete)
            .with_creatable(true)
            .with_exact_required(false)
    }

    /// Mode.
    #[must_use]
    pub const fn with_mode(mut self, mode: ComboMode) -> Self {
        self.mode = mode;
        self
    }

    /// Creatable free-text commit.
    #[must_use]
    pub const fn with_creatable(mut self, on: bool) -> Self {
        self.creatable = on;
        self
    }

    /// Require exact candidate match when not creatable.
    #[must_use]
    pub const fn with_exact_required(mut self, on: bool) -> Self {
        self.exact_required = on;
        self
    }

    /// Recent capacity.
    #[must_use]
    pub fn with_recent_limit(mut self, limit: usize) -> Self {
        self.recent_limit = limit.max(1);
        while self.recent.len() > self.recent_limit {
            self.recent.pop_back();
        }
        self
    }

    /// Seed draft text.
    #[must_use]
    pub fn with_draft(mut self, text: impl Into<String>) -> Self {
        self.set_draft(text);
        self
    }

    /// Live typing. [`Self::new`] stays idle (`editing: false`).
    #[must_use]
    pub fn with_editing(mut self) -> Self {
        self.draft.begin_edit();
        self
    }

    /// Start the insert session (Junie Enter on an idle field).
    pub fn begin_edit(&mut self) {
        self.draft.begin_edit();
    }

    /// Draft text.
    #[must_use]
    pub fn draft(&self) -> &str {
        self.draft.value()
    }

    /// Committed value id.
    #[must_use]
    pub const fn value(&self) -> Option<&Id> {
        self.value.as_ref()
    }

    /// Committed label.
    #[must_use]
    pub fn value_label(&self) -> Option<&str> {
        self.value_label.as_deref()
    }

    /// Active suggestion id.
    #[must_use]
    pub const fn active_suggestion(&self) -> Option<&Id> {
        self.menu.selected()
    }

    /// Menu open?
    #[must_use]
    pub const fn is_menu_open(&self) -> bool {
        self.menu.is_open()
    }

    /// Suggestion status.
    #[must_use]
    pub const fn suggestion_status(&self) -> SuggestionStatus {
        self.status
    }

    /// Current fetch generation (pass back into [`Self::apply_suggestions`]).
    #[must_use]
    pub const fn suggestion_generation(&self) -> u64 {
        self.generation
    }

    /// Generation of last applied batch.
    #[must_use]
    pub const fn applied_generation(&self) -> u64 {
        self.applied_generation
    }

    /// Mode.
    #[must_use]
    pub const fn mode(&self) -> ComboMode {
        self.mode
    }

    /// Creatable.
    #[must_use]
    pub const fn is_creatable(&self) -> bool {
        self.creatable
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Enabled.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Field rect (anchor).
    #[must_use]
    pub const fn field_area(&self) -> Rect {
        self.field
    }

    /// Menu state.
    #[must_use]
    pub const fn menu(&self) -> &CompletionMenuState<Id> {
        &self.menu
    }

    /// Mutable menu (advanced paint).
    pub fn menu_mut(&mut self) -> &mut CompletionMenuState<Id> {
        &mut self.menu
    }

    /// Recent values (newest first).
    #[must_use]
    pub fn recent(&self) -> impl Iterator<Item = (&Id, &str)> {
        self.recent.iter().map(|(i, l)| (i, l.as_str()))
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.draft.set_focused(on);
        if !on {
            self.menu.set_open(false);
        }
    }

    /// Enabled.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        self.draft.set_enabled(on);
    }

    /// Read-only.
    pub fn set_read_only(&mut self, on: bool) {
        self.read_only = on;
        self.draft.set_read_only(on);
    }

    /// Controlled draft (does not bump generation unless `notify`).
    pub fn set_draft(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.draft.set_focused(self.focused);
        self.draft.set_enabled(self.enabled);
        self.draft.set_read_only(self.read_only);
        self.draft = self.draft.reseed(text);
    }

    /// Set committed value without changing draft.
    pub fn set_value(&mut self, id: Option<Id>, label: Option<String>) {
        self.value = id;
        self.value_label = label;
    }

    /// Host error message for status Error.
    pub fn set_error_message(&mut self, msg: Option<String>) {
        self.error_message = msg;
        if self.error_message.is_some() {
            self.status = SuggestionStatus::Error;
        }
    }

    fn bump_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.status = SuggestionStatus::Loading;
        self.error_message = None;
        self.generation
    }

    fn open_menu(&mut self) -> ComboboxOutcome<Id> {
        if !self.menu.is_open() {
            self.menu.set_open(true);
            let request_gen = if self.status == SuggestionStatus::Idle {
                self.bump_generation()
            } else {
                self.generation
            };
            return ComboboxOutcome::MenuOpened {
                generation: request_gen,
            };
        }
        ComboboxOutcome::Ignored
    }

    fn close_menu(&mut self) -> ComboboxOutcome<Id> {
        if !self.menu.is_open() {
            return ComboboxOutcome::Ignored;
        }
        self.menu.set_open(false);
        ComboboxOutcome::MenuClosed
    }

    /// Apply async suggestion results for `generation`.
    ///
    /// **Stale results** (`generation != self.generation`) are ignored — this
    /// is the race-condition gate for out-of-order host responses.
    pub fn apply_suggestions(
        &mut self,
        generation: u64,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.applied_generation = generation;
        self.menu.reconcile(candidates);
        self.status = if candidates.is_empty() {
            SuggestionStatus::Empty
        } else {
            SuggestionStatus::Ready
        };
        if !candidates.is_empty() {
            self.menu.set_open(true);
        }
        true
    }

    /// Mark loading for current generation (host starts fetch).
    pub fn mark_loading(&mut self) {
        self.status = SuggestionStatus::Loading;
    }

    /// Mark error for current generation.
    pub fn mark_error(&mut self, message: impl Into<String>) {
        self.status = SuggestionStatus::Error;
        self.error_message = Some(message.into());
    }

    fn push_recent(&mut self, id: Id, label: String) {
        self.recent.retain(|(i, _)| i != &id);
        self.recent.push_front((id, label));
        while self.recent.len() > self.recent_limit {
            self.recent.pop_back();
        }
    }

    /// Commit active menu candidate (label from `candidates`).
    pub fn commit_active(
        &mut self,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> ComboboxOutcome<Id> {
        let Some(id) = self.menu.selected().cloned() else {
            return ComboboxOutcome::Ignored;
        };
        let label = candidates
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.label.to_owned())
            .unwrap_or_default();
        self.value = Some(id.clone());
        self.value_label = Some(label.clone());
        self.set_draft(label.clone());
        self.menu.set_open(false);
        self.status = SuggestionStatus::Idle;
        self.push_recent(id.clone(), label.clone());
        ComboboxOutcome::Committed { id, label }
    }

    /// Commit draft as free text.
    pub fn commit_created(&mut self) -> ComboboxOutcome<Id> {
        let text = self.draft.value().trim().to_owned();
        if text.is_empty() {
            return ComboboxOutcome::Ignored;
        }
        if !self.creatable {
            return ComboboxOutcome::ValidationFailed {
                reason: "value must match a suggestion",
            };
        }
        self.value = None;
        self.value_label = Some(text.clone());
        self.menu.set_open(false);
        self.status = SuggestionStatus::Idle;
        ComboboxOutcome::Created { text }
    }

    /// Whether draft exactly matches a candidate label (case-sensitive).
    #[must_use]
    pub fn draft_matches_candidate(
        &self,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> Option<Id> {
        let d = self.draft.value().trim();
        candidates
            .iter()
            .find(|c| c.enabled && c.label == d)
            .map(|c| c.id.clone())
    }

    /// Key adapter. Pass current candidate projection (may be empty while loading).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> ComboboxOutcome<Id> {
        if key.kind == KeyEventKind::Release || !self.enabled {
            return ComboboxOutcome::Ignored;
        }
        self.draft.set_focused(self.focused);
        self.draft.set_enabled(self.enabled);
        self.draft.set_read_only(self.read_only);

        if !self.focused {
            return ComboboxOutcome::Ignored;
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Esc semantics
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if self.menu.is_open() {
                return self.close_menu();
            }
            if !self.draft.value().is_empty() {
                // restore committed label if any
                if let Some(label) = self.value_label.clone() {
                    self.set_draft(label);
                } else {
                    let _ = self.draft.clear();
                }
                return ComboboxOutcome::Dismissed;
            }
            return ComboboxOutcome::Dismissed;
        }

        // Arrows / menu navigation when open or Down opens
        if matches!(
            key.code,
            KeyCode::Down | KeyCode::Up | KeyCode::PageDown | KeyCode::PageUp
        ) && !ctrl
            && !alt
        {
            if !self.menu.is_open() && matches!(key.code, KeyCode::Down) {
                let opened = self.open_menu();
                if !candidates.is_empty() {
                    self.menu.reconcile(candidates);
                }
                // still try move
                let _ = self.menu.handle_key(key, candidates);
                return if matches!(opened, ComboboxOutcome::Ignored) {
                    ComboboxOutcome::HighlightChanged {
                        id: self.menu.selected().cloned(),
                    }
                } else {
                    opened
                };
            }
            if self.menu.is_open() {
                return match self.menu.handle_key(key, candidates) {
                    CompletionMenuOutcome::SelectionChanged => ComboboxOutcome::HighlightChanged {
                        id: self.menu.selected().cloned(),
                    },
                    CompletionMenuOutcome::Committed(id)
                    | CompletionMenuOutcome::CommitWithChar { id, .. } => {
                        let label = candidates
                            .iter()
                            .find(|c| c.id == id)
                            .map(|c| c.label.to_owned())
                            .unwrap_or_default();
                        self.value = Some(id.clone());
                        self.value_label = Some(label.clone());
                        self.set_draft(label.clone());
                        self.menu.set_open(false);
                        self.status = SuggestionStatus::Idle;
                        self.push_recent(id.clone(), label.clone());
                        ComboboxOutcome::Committed { id, label }
                    }
                    CompletionMenuOutcome::Dismissed => self.close_menu(),
                    CompletionMenuOutcome::Ignored
                    | CompletionMenuOutcome::StatusChanged { .. }
                    | CompletionMenuOutcome::PresentationChanged { .. }
                    | CompletionMenuOutcome::GenerationStale { .. } => ComboboxOutcome::Ignored,
                };
            }
        }

        // Tab: commit highlight if menu open, else host Tab (Ignored)
        if matches!(key.code, KeyCode::Tab) && !ctrl && !alt {
            if self.menu.is_open() && self.menu.selected().is_some() && !candidates.is_empty() {
                return self.commit_active(candidates);
            }
            return ComboboxOutcome::Ignored;
        }

        // Enter
        if key.code == KeyCode::Enter && key.modifiers.is_empty() {
            if !self.draft.is_editing() {
                self.draft.begin_edit();
                return ComboboxOutcome::DraftChanged {
                    text: self.draft.value().to_owned(),
                    generation: self.generation,
                };
            }
            if self.menu.is_open() && self.menu.selected().is_some() && !candidates.is_empty() {
                return self.commit_active(candidates);
            }
            if let Some(id) = self.draft_matches_candidate(candidates) {
                let label = self.draft.value().trim().to_owned();
                self.value = Some(id.clone());
                self.value_label = Some(label.clone());
                self.menu.set_open(false);
                self.status = SuggestionStatus::Idle;
                self.push_recent(id.clone(), label.clone());
                return ComboboxOutcome::Committed { id, label };
            }
            if self.creatable {
                return self.commit_created();
            }
            if self.exact_required {
                return ComboboxOutcome::ValidationFailed {
                    reason: "value must match a suggestion",
                };
            }
            return ComboboxOutcome::Ignored;
        }

        // Typing → draft + open menu + bump generation
        if self.read_only {
            return ComboboxOutcome::Ignored;
        }

        match self.draft.handle_key(key) {
            TextInputOutcome::Changed | TextInputOutcome::Cleared => {
                let request_gen = self.bump_generation();
                self.menu.set_open(true);
                // clear selection until new list arrives
                self.menu.select(None);
                ComboboxOutcome::DraftChanged {
                    text: self.draft.value().to_owned(),
                    generation: request_gen,
                }
            }
            TextInputOutcome::Submitted(_) => {
                // Enter already handled
                ComboboxOutcome::Ignored
            }
            TextInputOutcome::Cancelled => {
                if self.menu.is_open() {
                    self.close_menu()
                } else {
                    ComboboxOutcome::Dismissed
                }
            }
            TextInputOutcome::ClipboardPasteRequest => ComboboxOutcome::ClipboardPasteRequest,
            TextInputOutcome::ClipboardCopy { text } | TextInputOutcome::ClipboardCut { text } => {
                ComboboxOutcome::ClipboardCopy { text }
            }
            TextInputOutcome::Ignored => ComboboxOutcome::Ignored,
        }
    }

    /// Intent path.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> ComboboxOutcome<Id> {
        if !self.enabled || !self.focused {
            return ComboboxOutcome::Ignored;
        }
        match intent {
            UiIntent::Activate | UiIntent::Submit => {
                if !self.draft.is_editing() {
                    self.draft.begin_edit();
                    ComboboxOutcome::DraftChanged {
                        text: self.draft.value().to_owned(),
                        generation: self.generation,
                    }
                } else if self.menu.is_open() && self.menu.selected().is_some() {
                    self.commit_active(candidates)
                } else if self.creatable {
                    self.commit_created()
                } else {
                    ComboboxOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => {
                if self.menu.is_open() {
                    self.close_menu()
                } else {
                    ComboboxOutcome::Dismissed
                }
            }
            other => match self.menu.handle_intent(candidates, other) {
                CompletionMenuOutcome::SelectionChanged => ComboboxOutcome::HighlightChanged {
                    id: self.menu.selected().cloned(),
                },
                CompletionMenuOutcome::Committed(id)
                | CompletionMenuOutcome::CommitWithChar { id, .. } => {
                    let label = candidates
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.label.to_owned())
                        .unwrap_or_default();
                    self.value = Some(id.clone());
                    self.value_label = Some(label.clone());
                    self.set_draft(label.clone());
                    self.menu.set_open(false);
                    ComboboxOutcome::Committed { id, label }
                }
                CompletionMenuOutcome::Dismissed => self.close_menu(),
                CompletionMenuOutcome::Ignored
                | CompletionMenuOutcome::StatusChanged { .. }
                | CompletionMenuOutcome::PresentationChanged { .. }
                | CompletionMenuOutcome::GenerationStale { .. } => ComboboxOutcome::Ignored,
            },
        }
    }

    /// Blur: close menu, keep draft; emit Blurred.
    pub fn blur(&mut self) -> ComboboxOutcome<Id> {
        self.focused = false;
        self.draft.set_focused(false);
        self.menu.set_open(false);
        ComboboxOutcome::Blurred
    }

    /// Paste into draft.
    pub fn insert_str(&mut self, text: &str) -> ComboboxOutcome<Id> {
        if !self.enabled || self.read_only {
            return ComboboxOutcome::Ignored;
        }
        self.draft.begin_edit();
        match self.draft.insert_str(text) {
            TextInputOutcome::Changed => {
                let request_gen = self.bump_generation();
                self.menu.set_open(true);
                ComboboxOutcome::DraftChanged {
                    text: self.draft.value().to_owned(),
                    generation: request_gen,
                }
            }
            _ => ComboboxOutcome::Ignored,
        }
    }

    /// Mouse: field focus / menu commit / outside close.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        candidates: &[CompletionCandidate<'_, Id>],
        menu_area: Rect,
    ) -> ComboboxOutcome<Id> {
        if !self.enabled {
            return ComboboxOutcome::Ignored;
        }
        // Menu first when open
        if self.menu.is_open() && !menu_area.is_empty() {
            match self.menu.handle_mouse(event, candidates) {
                CompletionMenuOutcome::Committed(id)
                | CompletionMenuOutcome::CommitWithChar { id, .. } => {
                    let label = candidates
                        .iter()
                        .find(|c| c.id == id)
                        .map(|c| c.label.to_owned())
                        .unwrap_or_default();
                    self.value = Some(id.clone());
                    self.value_label = Some(label.clone());
                    self.set_draft(label.clone());
                    self.menu.set_open(false);
                    self.status = SuggestionStatus::Idle;
                    self.push_recent(id.clone(), label.clone());
                    return ComboboxOutcome::Committed { id, label };
                }
                CompletionMenuOutcome::SelectionChanged => {
                    return ComboboxOutcome::HighlightChanged {
                        id: self.menu.selected().cloned(),
                    };
                }
                CompletionMenuOutcome::Dismissed => return self.close_menu(),
                CompletionMenuOutcome::Ignored
                | CompletionMenuOutcome::StatusChanged { .. }
                | CompletionMenuOutcome::PresentationChanged { .. }
                | CompletionMenuOutcome::GenerationStale { .. } => {}
            }
            // click outside menu and field → close menu
            if matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
                && !menu_area.contains(event.position)
                && !self.field.contains(event.position)
            {
                return self.close_menu();
            }
        }
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left))
            && self.field.contains(event.position)
        {
            self.set_focused(true);
            let _ = self.open_menu();
            return match self.draft.handle_mouse(event, self.field) {
                TextInputOutcome::Changed => {
                    let request_gen = self.bump_generation();
                    ComboboxOutcome::DraftChanged {
                        text: self.draft.value().to_owned(),
                        generation: request_gen,
                    }
                }
                _ => ComboboxOutcome::MenuOpened {
                    generation: self.generation,
                },
            };
        }
        ComboboxOutcome::Ignored
    }

    /// Open completion overlay helper (host OverlayStack).
    pub fn open_overlay<FocusId: Clone>(
        &self,
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        preferred: CompletionMenuSize,
        opener: Option<FocusId>,
    ) -> crate::interaction::OverlayOutcome<FocusId> {
        open_completion_overlay(stack, bounds, self.field, preferred, opener)
    }

    /// Dismiss completion overlay.
    pub fn dismiss_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
    ) -> crate::interaction::OverlayOutcome<FocusId> {
        dismiss_completion_overlay(stack)
    }

    /// Place menu rect for paint.
    #[must_use]
    pub fn place_menu(&self, bounds: Rect, preferred: CompletionMenuSize) -> Rect {
        place_completion_menu(bounds, self.field, preferred)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Combobox / Autocomplete field chrome (menu painted via [`CompletionMenu`]).
#[derive(Debug, Clone, Copy)]
pub struct Combobox<'a> {
    label: &'a str,
    placeholder: &'a str,
    system: &'a DesignSystem,
    validation: Validation<'a>,
}

impl<'a> Combobox<'a> {
    /// Field.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            label: "",
            placeholder: "Type",
            system,
            validation: Validation::Valid,
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(mut self, label: &'a str) -> Self {
        self.label = label;
        self
    }

    /// Placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Validation.
    #[must_use]
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// ASCII.
    #[must_use]
    /// Paint field only; host paints [`CompletionMenu`] in placed rect.
    pub fn paint<Id: Clone + PartialEq>(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ComboboxState<Id>,
    ) -> Rect {
        state.field = Rect::default();
        if area.is_empty() {
            return area;
        }
        let invalid = matches!(self.validation, Validation::Invalid(_))
            || matches!(state.status, SuggestionStatus::Error);
        let recipe = self.system.input_recipe(
            if !state.enabled {
                crate::style::ControlState::Disabled
            } else if matches!(state.status, SuggestionStatus::Loading) {
                crate::style::ControlState::Loading
            } else if state.focused {
                crate::style::ControlState::Focused
            } else {
                crate::style::ControlState::Default
            },
            invalid,
            state.draft.is_editing(),
        );
        let mut y = area.y;
        if area.height >= 2 && !self.label.is_empty() {
            let mut style = recipe.value;
            if state.focused {
                style = style.add_modifier(Modifier::BOLD);
            }
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.label, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }

        let mut right = area.right();
        // status cue
        let status = match state.status {
            SuggestionStatus::Loading => "…",
            SuggestionStatus::Error => "!",
            SuggestionStatus::Empty if state.menu.is_open() => "0",
            _ => "",
        };
        if area.width > 4 {
            right = right.saturating_sub(2);
            if !status.is_empty() {
                buffer.set_stringn(
                    right.saturating_add(1),
                    y.min(area.bottom().saturating_sub(1)),
                    status,
                    1,
                    if matches!(state.status, SuggestionStatus::Error) {
                        recipe.placeholder.patch(self.system.style(Role::Danger))
                    } else {
                        recipe.placeholder
                    },
                );
            }
        }

        let field = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            right.saturating_sub(area.x).max(1),
            1,
        );
        state.field = field;
        state.draft.set_focused(state.focused);
        // A suggestion-source error is a validation failure: it reaches the
        // field instead of being computed and dropped. Owning the message
        // locally keeps it alive past the borrow of `state.draft`
        // (plans/009 Step 3).
        let escalated: Option<String> = (invalid && matches!(self.validation, Validation::Valid))
            .then(|| {
                state
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "invalid".to_string())
            });
        let validation = match escalated.as_deref() {
            Some(msg) => Validation::Invalid(msg),
            None => self.validation,
        };
        let input = TextInput::new("", self.system)
            .placeholder(self.placeholder)
            .validation(validation);
        let _ = input.paint(field, buffer, &mut state.draft);

        // The chevron says whether the menu is open; it goes in the cell the
        // status did not take.
        if status.is_empty() && area.width > 4 {
            let chevron = self.system.glyphs.resolve(if state.menu.is_open() {
                crate::style::Glyph::ChevronUp
            } else {
                crate::style::Glyph::ChevronDown
            });
            buffer.set_stringn(
                right.saturating_add(1),
                field.y,
                chevron.text,
                1,
                recipe.placeholder,
            );
        }

        if field.y.saturating_add(1) < area.bottom() {
            if let Validation::Invalid(msg) = self.validation {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, field.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    msg,
                );
            } else if let Some(msg) = &state.error_message {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, field.y.saturating_add(1), area.width, 1),
                    self.system,
                    crate::widgets::label::DescriptionKind::Error,
                    msg,
                );
            }
        }
        field
    }

    /// Paint field + completion menu in one area (menu below field).
    pub fn paint_with_menu<Id: Clone + PartialEq + std::fmt::Display>(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ComboboxState<Id>,
        candidates: &[CompletionCandidate<'_, Id>],
    ) {
        let base_field_h: u16 = if !self.label.is_empty() && area.height >= 3 {
            2
        } else {
            1
        };
        // Reserve the message row in every state. Validation appearing must
        // not move the menu or steal a candidate row only after an error.
        let field_h = base_field_h.saturating_add(1).min(area.height);
        let field_area = Rect::new(area.x, area.y, area.width, field_h.min(area.height));
        let _ = self.paint(field_area, buffer, state);
        if state.menu.is_open() {
            let menu_area = Rect::new(
                area.x,
                area.y.saturating_add(field_h),
                area.width,
                area.height.saturating_sub(field_h),
            );
            if !menu_area.is_empty() {
                let empty = match state.status {
                    SuggestionStatus::Loading => "Loading…",
                    SuggestionStatus::Error => "Error",
                    SuggestionStatus::Empty => "No matches",
                    _ => "No matches",
                };
                let menu = CompletionMenu::new(candidates, self.system, area, state.field)
                    .preferred_size(CompletionMenuSize {
                        width: menu_area.width.max(8),
                        height: menu_area.height.max(1),
                    })
                    .empty_message(empty);
                // CompletionMenu places itself; force paint into menu_area by
                // temporarily using bounds/anchor such that place stays in area.
                menu.render(menu_area, buffer, &mut state.menu);
            }
        }
    }

    /// Semantic.
    pub fn register_semantic<Id, Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &ComboboxState<Id>,
    ) where
        Id: Clone + PartialEq,
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "{} {} gen={}",
            state.mode.id(),
            state.status.id(),
            state.generation
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    state.mode.id()
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    invalid: matches!(self.validation, Validation::Invalid(_))
                        || matches!(state.status, SuggestionStatus::Error),
                    busy: matches!(state.status, SuggestionStatus::Loading),
                    expanded: state.menu.is_open(),
                    ..Default::default()
                }),
        );
    }
}

const _: fn(u16, u16) -> Position = Position::new;
const _: &str = COMPLETION_OVERLAY_ID;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::RolePalette;

    fn cands() -> Vec<CompletionCandidate<'static, &'static str>> {
        vec![
            CompletionCandidate::new("rs", "Rust").kind("lang"),
            CompletionCandidate::new("go", "Go").kind("lang"),
            CompletionCandidate::new("ts", "TypeScript").kind("lang"),
        ]
    }

    #[test]
    fn focused_without_editing_is_not_underlined() {
        let system = DesignSystem::junie();
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        state.set_focused(true);
        assert!(
            !state.draft.is_editing(),
            "ComboboxState::new is idle like TextInput"
        );
        let area = Rect::new(0, 0, 24, 3);
        let mut buffer = Buffer::empty(area);
        let _ = Combobox::new(&system)
            .label("Lang")
            .paint(area, &mut buffer, &mut state);
        let field_y = area.y.saturating_add(1);
        let underlined = |buffer: &Buffer| {
            (0..area.width).any(|x| {
                buffer[(x, field_y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::UNDERLINED)
            })
        };
        assert!(
            !underlined(&buffer),
            "nav-focus combobox is gutter, not an editing underline"
        );
        state.draft.set_editing(true);
        let mut buffer = Buffer::empty(area);
        let _ = Combobox::new(&system)
            .label("Lang")
            .paint(area, &mut buffer, &mut state);
        assert!(underlined(&buffer), "editing combobox underlines the field");
    }

    #[test]
    fn draft_active_value_separate() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new()
            .with_creatable(true)
            .with_exact_required(false);
        state.set_focused(true);
        state.begin_edit();
        let c = cands();
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE), &c);
        match out {
            ComboboxOutcome::DraftChanged { text, generation } => {
                assert_eq!(text, "R");
                assert_eq!(generation, 1);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(state.draft(), "R");
        assert!(state.value().is_none());
        assert!(state.apply_suggestions(1, &c));
        assert_eq!(state.active_suggestion(), Some(&"rs"));
        // still no committed value until Enter
        assert!(state.value().is_none());
        assert!(matches!(
            state.commit_active(&c),
            ComboboxOutcome::Committed { id: "rs", .. }
        ));
        assert_eq!(state.value(), Some(&"rs"));
        assert_eq!(state.draft(), "Rust");
    }

    #[test]
    fn stale_generation_ignored() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        state.set_focused(true);
        state.begin_edit();
        let c = cands();
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &[]);
        assert_eq!(state.suggestion_generation(), 1);
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), &[]);
        assert_eq!(state.suggestion_generation(), 2);
        // stale gen 1
        assert!(!state.apply_suggestions(1, &c));
        assert_eq!(state.applied_generation(), 0);
        // current gen 2
        assert!(state.apply_suggestions(2, &c));
        assert_eq!(state.applied_generation(), 2);
        assert_eq!(state.suggestion_status(), SuggestionStatus::Ready);
    }

    #[test]
    fn enter_creatable() {
        let mut state: ComboboxState<&'static str> = ComboboxState::autocomplete();
        state.set_focused(true);
        let _ = state.insert_str("custom");
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &[]),
            ComboboxOutcome::Created { text } if text == "custom"
        ));
    }

    #[test]
    fn enter_exact_required_fails() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new()
            .with_creatable(false)
            .with_exact_required(true);
        state.set_focused(true);
        let _ = state.insert_str("nope");
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &[]),
            ComboboxOutcome::ValidationFailed { .. }
        ));
    }

    #[test]
    fn esc_closes_menu_then_dismisses() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        state.set_focused(true);
        let c = cands();
        let _ = state.open_menu();
        state.menu.reconcile(&c);
        assert!(state.is_menu_open());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &c),
            ComboboxOutcome::MenuClosed
        );
        assert!(!state.is_menu_open());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &c),
            ComboboxOutcome::Dismissed
        );
    }

    #[test]
    fn tab_commits_highlight() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new().with_creatable(false);
        state.set_focused(true);
        let c = cands();
        let _ = state.open_menu();
        assert!(
            state.apply_suggestions(0, &c) || state.apply_suggestions(state.generation, &c) || {
                // generation may be 0 still
                state.generation = 1;
                state.apply_suggestions(1, &c)
            }
        );
        // force ready menu
        state.generation = 1;
        assert!(state.apply_suggestions(1, &c));
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &c),
            ComboboxOutcome::Committed { id: "rs", .. }
        ));
    }

    #[test]
    fn blur_closes_menu() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        state.set_focused(true);
        let _ = state.open_menu();
        assert_eq!(state.blur(), ComboboxOutcome::Blurred);
        assert!(!state.is_menu_open());
        assert!(!state.is_focused());
    }

    #[test]
    fn recent_on_commit() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        state.set_focused(true);
        let c = cands();
        state.generation = 1;
        assert!(state.apply_suggestions(1, &c));
        let _ = state.commit_active(&c);
        assert_eq!(state.recent().next().map(|(i, _)| *i), Some("rs"));
    }

    #[test]
    fn overlay_helpers_compile() {
        let mut stack = OverlayStack::<&str>::default();
        let state = ComboboxState::<&str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let _ = state.open_overlay(
            &mut stack,
            bounds,
            CompletionMenuSize::default(),
            Some("field"),
        );
        let _ = ComboboxState::<&str>::dismiss_overlay(&mut stack);
    }

    #[test]
    fn paint_field_and_menu() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state: ComboboxState<&'static str> = ComboboxState::new().with_draft("Ru");
        state.set_focused(true);
        let c = cands();
        state.generation = 1;
        assert!(state.apply_suggestions(1, &c));
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        Combobox::new(&system)
            .label("Lang")
            .paint_with_menu(area, &mut buf, &mut state, &c);
        assert!(!state.field.is_empty());
    }

    #[test]
    fn mouse_focuses_field_and_opens_menu_from_painted_geometry() {
        let system = DesignSystem::default();
        let candidates = cands();
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        let area = Rect::new(0, 0, 40, 8);
        let mut buffer = Buffer::empty(area);
        Combobox::new(&system).paint_with_menu(area, &mut buffer, &mut state, &candidates);

        let outcome = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(state.field.x, state.field.y),
                modifiers: KeyModifiers::NONE,
            },
            &candidates,
            Rect::default(),
        );

        assert!(state.is_focused());
        assert!(state.is_menu_open());
        assert!(matches!(
            outcome,
            ComboboxOutcome::MenuOpened { .. } | ComboboxOutcome::DraftChanged { .. }
        ));
    }

    #[test]
    fn race_sequence_out_of_order() {
        let mut state: ComboboxState<&'static str> = ComboboxState::new();
        state.set_focused(true);
        state.begin_edit();
        // type a
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE), &[]);
        let g1 = state.suggestion_generation();
        // type b
        let _ = state.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), &[]);
        let g2 = state.suggestion_generation();
        assert!(g2 > g1);
        let late = cands();
        let early = vec![CompletionCandidate::new("old", "Old")];
        // g2 response first
        assert!(state.apply_suggestions(g2, &late));
        assert_eq!(state.active_suggestion(), Some(&"rs"));
        // g1 arrives late — ignored
        assert!(!state.apply_suggestions(g1, &early));
        assert_eq!(state.active_suggestion(), Some(&"rs"));
        assert_ne!(state.active_suggestion(), Some(&"old"));
    }

    #[test]
    fn fuzz_keys() {
        let mut state: ComboboxState<&'static str> = ComboboxState::autocomplete();
        state.set_focused(true);
        state.begin_edit();
        let c = cands();
        let keys = [
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        ];
        for (i, key) in keys.iter().cycle().take(30).enumerate() {
            let _ = state.handle_key(*key, &c);
            if i % 3 == 0 {
                let g = state.suggestion_generation();
                let _ = state.apply_suggestions(g, &c);
            }
        }
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state: ComboboxState<&'static str> = ComboboxState::new().with_draft("Go");
        state.set_focused(true);
        let c = cands();
        state.generation = 1;
        let _ = state.apply_suggestions(1, &c);
        let area = Rect::new(0, 0, 36, 8);
        let mut buf = Buffer::empty(area);
        let w = Combobox::new(&system);
        for _ in 0..100 {
            w.paint_with_menu(area, &mut buf, &mut state, &c);
        }
    }

    #[test]
    fn semantic() {
        let system = DesignSystem::default();
        let state = ComboboxState::<&str>::autocomplete();
        let mut scene = SemanticScene::<&str, ()>::default();
        Combobox::new(&system).register_semantic(&mut scene, "c", Rect::new(0, 0, 20, 1), &state);
        assert!(scene.get(&"c").is_some());
    }
}
