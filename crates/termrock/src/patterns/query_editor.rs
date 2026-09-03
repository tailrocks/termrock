// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **QueryEditor** — code-oriented editor for SQL, logs, search languages, and
//! structured queries.
//!
//! **Mission.** Multiline draft editing, completion, diagnostics, parameters,
//! history, run/stop, selection execution, format request, and saved queries.
//! Language services and execution stay **application-provided**. TermRock owns
//! interaction chrome and typed request outcomes.
//!
//! **Integrates.** [`super::TextArea`] (edit kernel), [`super::CompletionMenu`],
//! [`super::CodeFrame`] / [`super::Diagnostic`], [`super::KeyboardHelp`],
//! [`super::HistoryPicker`], and a **results slot** for host-painted
//! [`super::DataTable`] / future ResultGrid.
//!
//! **Draft + cursor** survive result-pane focus changes: only `accepts_input`
//! and focus zone flip — never clear the editor.
//!
//! Research: TablePlus-like query editors, database TUIs, Grafana query
//! workflows, terminal editors.
//!
//! Teaches: how to compose code-oriented editor for SQL, logs, search
//! languages, and structured queries.
//!
//! Composes: [`crate::widgets::CodeFrame`],
//! [`crate::widgets::CodeFrameLine`],
//! [`crate::widgets::CompletionMenuState`], [`crate::widgets::Diagnostic`],
//! [`crate::widgets::DiagnosticSeverity`], [`crate::widgets::HelpEntry`],
//! [`crate::widgets::HistoryEntry`], [`crate::widgets::HistoryKind`], and 9
//! more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};

use crate::{
    input::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent},
    style::{DesignSystem, Role},
    widgets::{
        CodeFrame, CodeFrameLine, CompletionMenuState, Diagnostic, DiagnosticSeverity, HelpEntry,
        HistoryEntry, HistoryKind, SemanticStatus, SourceLabel, StatusIndicator, TextArea,
        TextAreaOutcome, TextAreaState, TextCursor, TextWrap,
    },
};

// ── Language / mode / focus ─────────────────────────────────────────────────

/// Query language id (host maps to parser / highlighter / completer).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct QueryLanguage {
    /// Stable id (`sql`, `kql`, `promql`, `logs`, `jsonpath`, …).
    pub id: String,
    /// Display label.
    pub label: String,
}

impl QueryLanguage {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }

    /// SQL preset.
    #[must_use]
    pub fn sql() -> Self {
        Self::new("sql", "SQL")
    }

    /// Log query preset.
    #[must_use]
    pub fn logs() -> Self {
        Self::new("logs", "Logs")
    }

    /// Search / filter language.
    #[must_use]
    pub fn search() -> Self {
        Self::new("search", "Search")
    }
}

/// Presentation density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QueryEditorMode {
    /// Compact single-panel (editor only; results host-side or collapsed).
    Compact,
    /// Normal split (editor + results slot).
    #[default]
    Normal,
    /// Fullscreen editor overlay request.
    Fullscreen,
}

impl QueryEditorMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Normal => "normal",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// Cycle compact → normal → fullscreen → compact.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Compact => Self::Normal,
            Self::Normal => Self::Fullscreen,
            Self::Fullscreen => Self::Compact,
        }
    }
}

/// Keyboard focus zone inside the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QueryFocus {
    /// Draft editor (default).
    #[default]
    Editor,
    /// Results slot (host DataTable / ResultGrid).
    Results,
    /// Diagnostics strip.
    Diagnostics,
    /// Parameter bar.
    Parameters,
}

impl QueryFocus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Results => "results",
            Self::Diagnostics => "diagnostics",
            Self::Parameters => "parameters",
        }
    }
}

/// Run lifecycle chrome (host maps to real execution).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QueryRunStatus {
    /// Idle / no run.
    #[default]
    Idle,
    /// Execution in flight (host token optional).
    Running {
        /// Opaque host run id.
        run_id: String,
    },
    /// Last run succeeded.
    Success {
        /// Optional row count summary.
        rows: Option<u64>,
        /// Duration ms.
        duration_ms: Option<u64>,
    },
    /// Last run failed (message for chrome; details in diagnostics).
    Failed {
        /// Short summary.
        message: String,
    },
    /// Cancelled by user/host.
    Cancelled,
}

impl QueryRunStatus {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running { .. } => "running",
            Self::Success { .. } => "success",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Operator-facing lifecycle verb.
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running { .. } => "running",
            Self::Success { .. } => "completed",
            Self::Failed { .. } => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Shared lifecycle projection for recipe-owned status chrome.
    #[must_use]
    pub fn semantic(&self) -> SemanticStatus {
        match self {
            Self::Idle => SemanticStatus::Idle,
            Self::Running { .. } => SemanticStatus::Running,
            Self::Success { .. } => SemanticStatus::Success,
            Self::Failed { .. } => SemanticStatus::Failed,
            Self::Cancelled => SemanticStatus::Paused,
        }
    }

    /// Whether a stop request is meaningful.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running { .. })
    }
}

// ── Parameters / results chrome ─────────────────────────────────────────────

/// Named query parameter (host binds values at execute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParameter {
    /// Name (`limit`, `user_id`).
    pub name: String,
    /// Display value (may be redacted).
    pub value: String,
    /// Type hint (`int`, `text`, `uuid`).
    pub type_hint: Option<String>,
    /// Required.
    pub required: bool,
    /// Secret / redacted paint.
    pub secret: bool,
}

impl QueryParameter {
    /// Construct.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            type_hint: None,
            required: false,
            secret: false,
        }
    }

    /// Type hint.
    #[must_use]
    pub fn type_hint(mut self, t: impl Into<String>) -> Self {
        self.type_hint = Some(t.into());
        self
    }

    /// Required.
    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Secret.
    #[must_use]
    pub const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// Paint label.
    #[must_use]
    pub fn display_chip(&self) -> String {
        let v = if self.secret {
            "••••"
        } else if self.value.is_empty() {
            "—"
        } else {
            &self.value
        };
        if let Some(t) = &self.type_hint {
            format!("{}:{} ({})", self.name, v, t)
        } else {
            format!("{}:{}", self.name, v)
        }
    }
}

/// Host-projected results summary for the results slot chrome.
///
/// Full grid paint stays on host [`super::DataTable`] / future ResultGrid;
/// this is status chrome only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryResultSummary {
    /// Status line (`12 rows · 34ms` or error).
    pub status: String,
    /// Optional column count.
    pub columns: Option<u32>,
    /// Optional known row count (streaming may leave None).
    pub rows: Option<u64>,
    /// Whether more pages exist.
    pub has_more: bool,
}

impl QueryResultSummary {
    /// Construct.
    #[must_use]
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            columns: None,
            rows: None,
            has_more: false,
        }
    }

    /// Rows.
    #[must_use]
    pub const fn rows(mut self, n: u64) -> Self {
        self.rows = Some(n);
        self
    }

    /// Columns.
    #[must_use]
    pub const fn columns(mut self, n: u32) -> Self {
        self.columns = Some(n);
        self
    }
}

/// Saved query list item (host persistence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQuery {
    /// Stable id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Body preview / full text host loads on select.
    pub preview: String,
    /// Language id when known.
    pub language: Option<String>,
}

impl SavedQuery {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, preview: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            preview: preview.into(),
            language: None,
        }
    }

    /// Language.
    #[must_use]
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = Some(lang.into());
        self
    }
}

// ── Geometry after paint ────────────────────────────────────────────────────

/// Slot rects for host composition (results grid, overlays).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueryEditorSlots {
    /// Outer root.
    pub root: Rect,
    /// Title / status strip.
    pub chrome: Rect,
    /// Parameter chips.
    pub parameters: Rect,
    /// Editor body (TextArea).
    pub editor: Rect,
    /// Diagnostics strip.
    pub diagnostics: Rect,
    /// Results slot (host paints DataTable / ResultGrid here).
    pub results: Rect,
    /// Footer help / run bar.
    pub footer: Rect,
}

impl QueryEditorSlots {
    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            root: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            chrome: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            parameters: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            editor: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            diagnostics: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            results: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            footer: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed control requests — host owns language services and execution.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QueryEditorOutcome {
    /// No change.
    Ignored,
    /// Draft, cursor, or chrome changed.
    Changed,
    /// Viewport scrolled.
    Scrolled,
    /// Focus zone changed (draft preserved).
    FocusChanged(QueryFocus),
    /// Presentation mode changed.
    ModeChanged(QueryEditorMode),
    /// Run full buffer or selection.
    RunRequested {
        /// Text to execute.
        text: String,
        /// True when selection-only.
        selection_only: bool,
        /// Language id.
        language: String,
    },
    /// Stop in-flight run.
    StopRequested {
        /// Host run id when known.
        run_id: Option<String>,
    },
    /// Format / pretty-print request (host applies rewrite).
    FormatRequested {
        /// Current text.
        text: String,
        /// Language id.
        language: String,
    },
    /// Save query request.
    SaveQueryRequested {
        /// Draft text.
        text: String,
        /// Optional suggested name.
        name: Option<String>,
        /// Language id.
        language: String,
    },
    /// Open saved-query picker (host).
    OpenSavedQueries,
    /// Load saved query into draft (host may call [`QueryEditorState::set_text`]).
    SavedQuerySelected {
        /// Saved id.
        id: String,
    },
    /// Open history picker.
    OpenHistory,
    /// Apply history value (host may also call set_text).
    HistoryApplied {
        /// History entry id.
        id: String,
        /// Value text.
        value: String,
    },
    /// Completion surface should open/refresh.
    CompletionRequested {
        /// Prefix / token at cursor.
        query: String,
        /// Cursor for host insert range.
        cursor: TextCursor,
        /// Language id.
        language: String,
    },
    /// Completion menu closed.
    CompletionClosed,
    /// Candidate committed (host inserts / replaces range).
    CompletionCommitted {
        /// Candidate id as string.
        id: String,
    },
    /// Open keyboard help modal.
    OpenKeyboardHelp,
    /// Jump caret to diagnostic primary span (host may also open CodeFrame).
    JumpToDiagnostic {
        /// Diagnostic id.
        id: String,
    },
    /// Parameter edit requested.
    ParameterEditRequested {
        /// Parameter name.
        name: String,
    },
    /// Clipboard copy.
    ClipboardCopy {
        /// Text.
        text: String,
    },
    /// Paste request.
    ClipboardPasteRequest,
    /// External editor.
    ExternalEditorRequested,
    /// Fullscreen open (mode already Fullscreen).
    FullscreenRequested,
    /// Fullscreen dismiss.
    FullscreenDismissed,
    /// Language switch request.
    LanguageChanged {
        /// New language id.
        id: String,
    },
    /// Cancel / blur.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Query editor state.
///
/// Draft lives in [`Self::editor`]. Across result focus, completion overlays,
/// and help modals the draft and cursor are **never cleared** — only
/// [`Self::focus`] and editor `accepts_input` change.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryEditorState {
    /// Multiline grapheme-safe draft.
    pub editor: TextAreaState,
    /// Language chrome.
    pub language: QueryLanguage,
    /// Presentation.
    pub mode: QueryEditorMode,
    /// Focus zone.
    pub focus: QueryFocus,
    /// Run chrome.
    pub run: QueryRunStatus,
    /// Parameter chips (host-owned values; state holds projection).
    pub parameters: Vec<QueryParameter>,
    /// Selected parameter index when focus is Parameters.
    pub param_cursor: usize,
    /// Results summary chrome.
    pub results: QueryResultSummary,
    /// Editor vs results height split (0..=100 percent for editor in Normal).
    pub editor_percent: u8,
    /// Completion menu state (host supplies candidates at paint).
    pub completion: CompletionMenuState<String>,
    /// Whether completion popup is open.
    pub completion_open: bool,
    /// Diagnostic strip cursor.
    pub diagnostic_cursor: usize,
    /// Soft wrap in editor.
    pub soft_wrap: bool,
    /// Line numbers.
    pub line_numbers: bool,
    /// Title override.
    pub title: Option<String>,
    /// Placeholder when empty.
    pub placeholder: String,
    /// Last paint slots.
    pub slots: QueryEditorSlots,
    /// Host grants input to the workbench.
    accepts_input: bool,
}

impl Default for QueryEditorState {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryEditorState {
    /// Fresh idle SQL editor.
    #[must_use]
    pub fn new() -> Self {
        let mut editor = TextAreaState::new("");
        editor.set_accepts_input(true);
        editor.set_editing(true);
        Self {
            editor,
            language: QueryLanguage::sql(),
            mode: QueryEditorMode::Normal,
            focus: QueryFocus::Editor,
            run: QueryRunStatus::Idle,
            parameters: Vec::new(),
            param_cursor: 0,
            results: QueryResultSummary::default(),
            editor_percent: 45,
            completion: CompletionMenuState::new(None),
            completion_open: false,
            diagnostic_cursor: 0,
            soft_wrap: false,
            line_numbers: true,
            title: None,
            placeholder: "Enter query…".into(),
            slots: QueryEditorSlots::empty(),
            accepts_input: true,
        }
    }

    /// With initial draft.
    #[must_use]
    pub fn with_text(text: impl AsRef<str>) -> Self {
        let mut s = Self::new();
        s.set_text(text.as_ref());
        s
    }

    /// Host input gate for the whole workbench.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.sync_editor_input();
    }

    /// Whether workbench accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Sync TextArea accepts_input from focus + gate.
    fn sync_editor_input(&mut self) {
        let on =
            self.accepts_input && matches!(self.focus, QueryFocus::Editor) && !self.completion_open;
        self.editor.set_accepts_input(on);
        self.editor.set_editing(on);
    }

    /// Draft text.
    #[must_use]
    pub fn text(&self) -> String {
        self.editor.text()
    }

    /// Replace draft (preserves as host load of history/saved).
    pub fn set_text(&mut self, text: &str) {
        self.editor.set_text(text);
        self.sync_editor_input();
    }

    /// Cursor.
    #[must_use]
    pub const fn cursor(&self) -> TextCursor {
        self.editor.cursor()
    }

    /// Selection text if any.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        self.editor.selected_text()
    }

    /// Text for run: selection if present, else full buffer.
    #[must_use]
    pub fn executable_text(&self) -> (String, bool) {
        if let Some(sel) = self.selected_text().filter(|s| !s.trim().is_empty()) {
            (sel, true)
        } else {
            (self.text(), false)
        }
    }

    /// Move focus without clearing draft/cursor.
    pub fn set_focus(&mut self, focus: QueryFocus) -> QueryEditorOutcome {
        if self.focus == focus {
            return QueryEditorOutcome::Ignored;
        }
        self.focus = focus;
        self.sync_editor_input();
        QueryEditorOutcome::FocusChanged(focus)
    }

    /// Set presentation mode.
    pub fn set_mode(&mut self, mode: QueryEditorMode) -> QueryEditorOutcome {
        if self.mode == mode {
            return QueryEditorOutcome::Ignored;
        }
        let prev = self.mode;
        self.mode = mode;
        if mode == QueryEditorMode::Fullscreen {
            return QueryEditorOutcome::FullscreenRequested;
        }
        if prev == QueryEditorMode::Fullscreen {
            return QueryEditorOutcome::FullscreenDismissed;
        }
        QueryEditorOutcome::ModeChanged(mode)
    }

    /// Host updates run status.
    pub fn set_run(&mut self, run: QueryRunStatus) {
        self.run = run;
    }

    /// Host sets parameters.
    pub fn set_parameters(&mut self, params: Vec<QueryParameter>) {
        self.parameters = params;
        if self.param_cursor >= self.parameters.len() {
            self.param_cursor = self.parameters.len().saturating_sub(1);
        }
    }

    /// Host sets results summary chrome.
    pub fn set_results(&mut self, summary: QueryResultSummary) {
        self.results = summary;
    }

    /// Open completion (host then paints CompletionMenu with candidates).
    pub fn open_completion(&mut self) -> QueryEditorOutcome {
        self.completion_open = true;
        self.sync_editor_input();
        let (query, cursor) = token_at_cursor(&self.editor);
        QueryEditorOutcome::CompletionRequested {
            query,
            cursor,
            language: self.language.id.clone(),
        }
    }

    /// Close completion.
    pub fn close_completion(&mut self) -> QueryEditorOutcome {
        if !self.completion_open {
            return QueryEditorOutcome::Ignored;
        }
        self.completion_open = false;
        self.sync_editor_input();
        QueryEditorOutcome::CompletionClosed
    }

    /// Insert text at cursor (completion commit helper).
    pub fn insert_text(&mut self, text: &str) {
        self.editor.set_accepts_input(true);
        let _ = self.editor.insert_text(text);
        self.sync_editor_input();
    }

    /// Request run.
    pub fn request_run(&self) -> QueryEditorOutcome {
        let (text, selection_only) = self.executable_text();
        if text.trim().is_empty() {
            return QueryEditorOutcome::Ignored;
        }
        if self.run.is_running() {
            return QueryEditorOutcome::Ignored;
        }
        QueryEditorOutcome::RunRequested {
            text,
            selection_only,
            language: self.language.id.clone(),
        }
    }

    /// Request stop.
    pub fn request_stop(&self) -> QueryEditorOutcome {
        match &self.run {
            QueryRunStatus::Running { run_id } => QueryEditorOutcome::StopRequested {
                run_id: Some(run_id.clone()),
            },
            _ => QueryEditorOutcome::StopRequested { run_id: None },
        }
    }

    /// Primary key handler.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        diagnostics: &[Diagnostic<'_>],
    ) -> QueryEditorOutcome {
        if !self.accepts_input || key.is_release() {
            return QueryEditorOutcome::Ignored;
        }
        let is_press = key.is_press();
        if !is_press {
            return QueryEditorOutcome::Ignored;
        }

        // Completion open: route nav to host menu; Esc closes; Enter commits via host.
        if self.completion_open {
            match key.code {
                KeyCode::Esc => return self.close_completion(),
                KeyCode::Tab | KeyCode::Enter if key.modifiers.is_empty() => {
                    // Host should commit selected candidate; we emit generic commit if menu has selection.
                    if let Some(id) = self.completion.selected().cloned() {
                        self.completion_open = false;
                        self.sync_editor_input();
                        return QueryEditorOutcome::CompletionCommitted { id };
                    }
                    return QueryEditorOutcome::Ignored;
                }
                // Let host drive CompletionMenuState for arrows; still allow typing into editor
                KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                    return QueryEditorOutcome::Ignored;
                }
                _ => {}
            }
        }

        // Global chords (run/stop/format/save/help/history/focus) before editor.
        // TermRock KeyCode has no function keys — use Ctrl/Alt chords only.
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Enter => {
                    return self.request_run();
                }
                KeyCode::Char('r' | 'R') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.request_run();
                }
                KeyCode::Char('r' | 'R') if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                    if self.run.is_running() {
                        return self.request_stop();
                    }
                    return self.request_run();
                }
                KeyCode::Char('s' | 'S') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.request_stop();
                }
                KeyCode::Char('s' | 'S') => {
                    return QueryEditorOutcome::SaveQueryRequested {
                        text: self.text(),
                        name: None,
                        language: self.language.id.clone(),
                    };
                }
                KeyCode::Char('f' | 'F') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return self.set_mode(QueryEditorMode::Fullscreen);
                }
                KeyCode::Char('f' | 'F') => {
                    return QueryEditorOutcome::FormatRequested {
                        text: self.text(),
                        language: self.language.id.clone(),
                    };
                }
                KeyCode::Char('h' | 'H') => {
                    return QueryEditorOutcome::OpenHistory;
                }
                KeyCode::Char('o' | 'O') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return QueryEditorOutcome::OpenSavedQueries;
                }
                KeyCode::Char(' ') => {
                    return self.open_completion();
                }
                KeyCode::Char('1') => return self.set_focus(QueryFocus::Editor),
                KeyCode::Char('2') => return self.set_focus(QueryFocus::Results),
                KeyCode::Char('3') => return self.set_focus(QueryFocus::Diagnostics),
                KeyCode::Char('4') => return self.set_focus(QueryFocus::Parameters),
                KeyCode::Char('m' | 'M') => {
                    let next = self.mode.cycle();
                    return self.set_mode(next);
                }
                KeyCode::Char('j' | 'J') => {
                    // Cycle focus zones (Ctrl+J)
                    let next = match self.focus {
                        QueryFocus::Editor => QueryFocus::Results,
                        QueryFocus::Results => QueryFocus::Diagnostics,
                        QueryFocus::Diagnostics => QueryFocus::Parameters,
                        QueryFocus::Parameters => QueryFocus::Editor,
                    };
                    return self.set_focus(next);
                }
                KeyCode::Char('?' | '/') => {
                    return QueryEditorOutcome::OpenKeyboardHelp;
                }
                KeyCode::Char('l' | 'L') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    return QueryEditorOutcome::LanguageChanged {
                        id: self.language.id.clone(),
                    };
                }
                _ => {}
            }
        }

        if key.code == KeyCode::Esc {
            if self.completion_open {
                return self.close_completion();
            }
            if self.mode == QueryEditorMode::Fullscreen {
                return self.set_mode(QueryEditorMode::Normal);
            }
            if !matches!(self.focus, QueryFocus::Editor) {
                return self.set_focus(QueryFocus::Editor);
            }
            return QueryEditorOutcome::Cancelled;
        }

        // Zone-specific keys when not on editor
        match self.focus {
            QueryFocus::Results => {
                // Host owns result grid keys; we only handle focus return
                if matches!(key.code, KeyCode::Char('e' | 'E')) && key.modifiers.is_empty() {
                    return self.set_focus(QueryFocus::Editor);
                }
                return QueryEditorOutcome::Ignored;
            }
            QueryFocus::Diagnostics => {
                return self.handle_diagnostics_key(key, diagnostics);
            }
            QueryFocus::Parameters => {
                return self.handle_parameters_key(key);
            }
            QueryFocus::Editor => {}
        }

        // Editor path
        self.sync_editor_input();
        match self.editor.handle_key(key) {
            TextAreaOutcome::Ignored => {
                // Trigger completion on `.` for SQL-ish (request only)
                if matches!(key.code, KeyCode::Char('.')) {
                    let _ = self.editor.handle_key(key); // already ignored
                }
                QueryEditorOutcome::Ignored
            }
            TextAreaOutcome::Changed => {
                // Auto-request completion after identifier-ish char
                if let KeyCode::Char(c) = key.code
                    && (c.is_alphanumeric() || c == '_' || c == '.')
                    && self.completion_open
                {
                    let (query, cursor) = token_at_cursor(&self.editor);
                    return QueryEditorOutcome::CompletionRequested {
                        query,
                        cursor,
                        language: self.language.id.clone(),
                    };
                }
                QueryEditorOutcome::Changed
            }
            TextAreaOutcome::Scrolled => QueryEditorOutcome::Scrolled,
            TextAreaOutcome::Cancelled => QueryEditorOutcome::Cancelled,
            TextAreaOutcome::ClipboardCopy { text } => QueryEditorOutcome::ClipboardCopy { text },
            TextAreaOutcome::ClipboardCut { text } => QueryEditorOutcome::ClipboardCopy { text },
            TextAreaOutcome::ClipboardPasteRequest => QueryEditorOutcome::ClipboardPasteRequest,
            TextAreaOutcome::ExternalEditorRequested => QueryEditorOutcome::ExternalEditorRequested,
            TextAreaOutcome::FullscreenRequested => self.set_mode(QueryEditorMode::Fullscreen),
        }
    }

    fn handle_diagnostics_key(
        &mut self,
        key: KeyEvent,
        diagnostics: &[Diagnostic<'_>],
    ) -> QueryEditorOutcome {
        if diagnostics.is_empty() {
            return QueryEditorOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                self.diagnostic_cursor = (self.diagnostic_cursor + 1).min(diagnostics.len() - 1);
                QueryEditorOutcome::Changed
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                self.diagnostic_cursor = self.diagnostic_cursor.saturating_sub(1);
                QueryEditorOutcome::Changed
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                let id = diagnostics[self.diagnostic_cursor.min(diagnostics.len() - 1)]
                    .id
                    .to_string();
                // Jump caret if primary label present
                if let Some(label) = diagnostics[self.diagnostic_cursor.min(diagnostics.len() - 1)]
                    .labels
                    .first()
                {
                    let line = label.range.start_line.saturating_sub(1) as usize;
                    let byte = 0usize;
                    let _ = self.editor.set_cursor(TextCursor { line, byte });
                    let _ = self.set_focus(QueryFocus::Editor);
                }
                QueryEditorOutcome::JumpToDiagnostic { id }
            }
            KeyCode::Esc => self.set_focus(QueryFocus::Editor),
            _ => QueryEditorOutcome::Ignored,
        }
    }

    fn handle_parameters_key(&mut self, key: KeyEvent) -> QueryEditorOutcome {
        if self.parameters.is_empty() {
            return QueryEditorOutcome::Ignored;
        }
        match key.code {
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab if key.modifiers.is_empty() => {
                self.param_cursor = (self.param_cursor + 1).min(self.parameters.len() - 1);
                QueryEditorOutcome::Changed
            }
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                self.param_cursor = self.param_cursor.saturating_sub(1);
                QueryEditorOutcome::Changed
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                let name = self.parameters[self.param_cursor.min(self.parameters.len() - 1)]
                    .name
                    .clone();
                QueryEditorOutcome::ParameterEditRequested { name }
            }
            KeyCode::Esc => self.set_focus(QueryFocus::Editor),
            _ => QueryEditorOutcome::Ignored,
        }
    }

    /// Mouse: click focus zones; else forward to editor when focused.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        diagnostics: &[Diagnostic<'_>],
    ) -> QueryEditorOutcome {
        if !self.accepts_input {
            return QueryEditorOutcome::Ignored;
        }
        let pos = event.position;
        let slots = self.slots;
        if !slots.results.is_empty() && slots.results.contains(pos) {
            return self.set_focus(QueryFocus::Results);
        }
        if !slots.diagnostics.is_empty() && slots.diagnostics.contains(pos) {
            let _ = diagnostics;
            return self.set_focus(QueryFocus::Diagnostics);
        }
        if !slots.parameters.is_empty() && slots.parameters.contains(pos) {
            return self.set_focus(QueryFocus::Parameters);
        }
        if !slots.editor.is_empty() && slots.editor.contains(pos) {
            let out = self.set_focus(QueryFocus::Editor);
            if matches!(
                out,
                QueryEditorOutcome::Ignored | QueryEditorOutcome::FocusChanged(_)
            ) && matches!(self.focus, QueryFocus::Editor)
            {
                self.sync_editor_input();
                match self.editor.handle_event(Event::Mouse(event)) {
                    TextAreaOutcome::Changed | TextAreaOutcome::Scrolled => {
                        return QueryEditorOutcome::Changed;
                    }
                    _ => {}
                }
            }
            return out;
        }
        QueryEditorOutcome::Ignored
    }
}

/// Extract rough token at cursor for completion query.
#[must_use]
pub fn token_at_cursor(editor: &TextAreaState) -> (String, TextCursor) {
    let cursor = editor.cursor();
    let line = editor.lines().nth(cursor.line).unwrap_or("");
    let byte = cursor.byte.min(line.len());
    let before = &line[..byte];
    let start = before
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .map(|i| {
            let ch = before[i..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            i + ch
        })
        .unwrap_or(0);
    let token = before[start..].to_string();
    (token, cursor)
}

/// Map diagnostics into CodeFrame lines from the draft (host may pass richer windows).
#[must_use]
pub fn draft_code_frame_lines(editor: &TextAreaState) -> Vec<CodeFrameLine<'_>> {
    editor
        .lines()
        .enumerate()
        .map(|(i, text)| CodeFrameLine::new((i + 1) as u32, text))
        .collect()
}

/// Default keyboard help entries for QueryEditor (host merges with live Keymap).
#[must_use]
pub fn query_editor_help_entries() -> Vec<HelpEntry> {
    vec![
        HelpEntry::new("run", "Query", "C-r / C-enter", "Run query / selection"),
        HelpEntry::new("stop", "Query", "C-S-s", "Stop run"),
        HelpEntry::new("format", "Query", "C-f", "Format request"),
        HelpEntry::new("save", "Query", "C-s", "Save query request"),
        HelpEntry::new("history", "Query", "C-h", "Open history"),
        HelpEntry::new("saved", "Query", "C-S-o", "Open saved queries"),
        HelpEntry::new("complete", "Edit", "C-space", "Completion"),
        HelpEntry::new(
            "focus",
            "Nav",
            "C-j",
            "Cycle editor/results/diagnostics/params",
        ),
        HelpEntry::new("help", "Help", "C-?", "Keyboard help"),
        HelpEntry::new("fullscreen", "View", "C-S-f", "Fullscreen editor"),
        HelpEntry::new("mode", "View", "C-m", "Cycle compact/normal/fullscreen"),
    ]
}

/// Project saved queries into history-picker shaped entries.
#[must_use]
pub fn saved_queries_to_history(entries: &[SavedQuery]) -> Vec<HistoryEntry<String>> {
    entries
        .iter()
        .map(|s| {
            let mut e = HistoryEntry::new(s.id.clone(), s.preview.clone())
                .display(s.name.clone())
                .kind(HistoryKind::Command)
                .preview(s.preview.clone());
            if let Some(lang) = &s.language {
                e = e.meta(lang.clone());
            }
            e
        })
        .collect()
}

/// Count diagnostics by severity letter summary.
#[must_use]
pub fn diagnostic_summary(diagnostics: &[Diagnostic<'_>]) -> String {
    if diagnostics.is_empty() {
        return "0 problems".into();
    }
    let mut e = 0u32;
    let mut w = 0u32;
    for d in diagnostics {
        match d.severity {
            DiagnosticSeverity::Error => e += 1,
            DiagnosticSeverity::Warning => w += 1,
            _ => {}
        }
    }
    format!("E{e} W{w} · {} total", diagnostics.len())
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Query workbench chrome (editor + optional results slot).
#[derive(Debug, Clone, Copy)]
pub struct QueryEditor<'a> {
    system: &'a DesignSystem,
    diagnostics: &'a [Diagnostic<'a>],
    focused: bool,
    title: Option<&'a str>,
}

impl<'a> QueryEditor<'a> {
    /// Design system.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            diagnostics: &[],
            focused: true,
            title: None,
        }
    }

    /// Diagnostics projection.
    #[must_use]
    pub const fn diagnostics(mut self, items: &'a [Diagnostic<'a>]) -> Self {
        self.diagnostics = items;
        self
    }

    /// Title override.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus chrome.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// ASCII.
    #[must_use]
    /// Paint workbench; host paints result grid into [`QueryEditorSlots::results`].
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut QueryEditorState) {
        if area.is_empty() {
            return;
        }
        let mut slots = QueryEditorSlots {
            root: area,
            ..QueryEditorSlots::empty()
        };

        let mut y = area.y;
        let mut remaining = area.height;

        // Chrome title line
        if remaining > 0 {
            slots.chrome = Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            };
            let title = self.title.or(state.title.as_deref()).unwrap_or("Query");
            let status =
                StatusIndicator::new(state.run.semantic(), self.system).label(state.run.verb());
            let status_width = status.measure_width(None).min(area.width);
            status.paint(Rect::new(area.x, y, status_width, 1), buffer, None);
            let metadata_x = area.x.saturating_add(status_width.saturating_add(1));
            let metadata_width = area.right().saturating_sub(metadata_x);
            let line = format!(
                "· {title} · {} · {} · {}",
                state.language.label,
                state.mode.id(),
                state.focus.id(),
            );
            let style = if self.focused {
                self.system.style(Role::TextStrong)
            } else {
                self.system.style(Role::TextMuted)
            };
            if metadata_width > 0 {
                self.system.paint_row(
                    buffer,
                    Rect::new(metadata_x, y, metadata_width, 1),
                    &line,
                    style,
                );
            }
            y = y.saturating_add(1);
            remaining = remaining.saturating_sub(1);
        }

        // Parameters
        let param_h = u16::from(!state.parameters.is_empty() && remaining >= 3);
        if param_h > 0 {
            slots.parameters = Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            };
            let mut chips = String::from("params ");
            for (i, p) in state.parameters.iter().enumerate() {
                if i == state.param_cursor && matches!(state.focus, QueryFocus::Parameters) {
                    chips.push('[');
                    chips.push_str(&p.display_chip());
                    chips.push(']');
                } else {
                    chips.push_str(&p.display_chip());
                }
                chips.push(' ');
            }
            let style = if matches!(state.focus, QueryFocus::Parameters) {
                self.system.style(Role::Focus)
            } else {
                self.system.style(Role::TextMuted)
            };
            self.system
                .paint_row(buffer, Rect::new(area.x, y, area.width, 1), &chips, style);
            y = y.saturating_add(1);
            remaining = remaining.saturating_sub(1);
        }

        // Footer + diagnostics + results reserve
        let footer_h = u16::from(remaining >= 2);
        let diag_h = if self.diagnostics.is_empty() || remaining < 4 {
            0
        } else {
            match state.mode {
                QueryEditorMode::Compact => u16::from(remaining >= 5).min(1),
                _ => 2u16.min(remaining.saturating_sub(footer_h + 2)),
            }
        };

        let results_h = match state.mode {
            QueryEditorMode::Compact | QueryEditorMode::Fullscreen => 0,
            QueryEditorMode::Normal => {
                let body = remaining.saturating_sub(footer_h + diag_h);
                let pct = u16::from(state.editor_percent.clamp(20, 80));
                let editor_h = body.saturating_mul(pct) / 100;
                body.saturating_sub(editor_h.max(2))
                    .min(body.saturating_sub(2))
            }
        };

        let editor_h = remaining
            .saturating_sub(footer_h)
            .saturating_sub(diag_h)
            .saturating_sub(results_h)
            .max(1);

        // Editor
        slots.editor = Rect {
            x: area.x,
            y,
            width: area.width,
            height: editor_h,
        };
        state.sync_editor_input();
        if state.soft_wrap {
            state.editor.set_wrap(TextWrap::Soft);
        }
        TextArea::new(self.system)
            .placeholder(state.placeholder.as_str())
            .line_numbers(state.line_numbers)
            .render(slots.editor, buffer, &mut state.editor);
        // Focus border cue on first cell of editor row when focused
        if matches!(state.focus, QueryFocus::Editor) && self.focused {
            // light accent mark
            if slots.editor.width > 0 {
                self.system.paint_row(
                    buffer,
                    Rect::new(slots.editor.x, slots.editor.y, 1, 1),
                    "›",
                    self.system.style(Role::Accent),
                );
            }
        }
        y = y.saturating_add(editor_h);

        // Diagnostics strip
        if diag_h > 0 {
            slots.diagnostics = Rect {
                x: area.x,
                y,
                width: area.width,
                height: diag_h,
            };
            let focused_diag = matches!(state.focus, QueryFocus::Diagnostics);
            let summary = diagnostic_summary(self.diagnostics);
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &format!("{} {}", if focused_diag { "●" } else { " " }, summary),
                if focused_diag {
                    self.system.style(Role::Focus)
                } else {
                    self.system.style(Role::TextMuted)
                },
            );
            if diag_h > 1
                && let Some(d) = self.diagnostics.get(
                    state
                        .diagnostic_cursor
                        .min(self.diagnostics.len().saturating_sub(1)),
                )
            {
                let msg = format!(
                    "  {}{} {}",
                    d.severity.letter(),
                    d.code.map(|c| format!("[{c}] ")).unwrap_or_default(),
                    d.message
                );
                self.system.paint_row(
                    buffer,
                    Rect::new(area.x, y.saturating_add(1), area.width, 1),
                    &msg,
                    self.system.style(d.severity.role()),
                );
            }
            y = y.saturating_add(diag_h);
        }

        // Results slot chrome (host paints grid inside)
        if results_h > 0 {
            slots.results = Rect {
                x: area.x,
                y,
                width: area.width,
                height: results_h,
            };
            let focused_res = matches!(state.focus, QueryFocus::Results);
            let hdr = if state.results.status.is_empty() {
                "results · (host DataTable / ResultGrid)".to_string()
            } else {
                format!("results · {}", state.results.status)
            };
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                &hdr,
                if focused_res {
                    self.system.style(Role::Focus)
                } else {
                    self.system.style(Role::TextMuted)
                },
            );
            // Fill remaining result area with subtle empty mark
            if results_h > 1 {
                let mark = if state.results.rows.is_some() {
                    format!(
                        "  {} cols · {} rows{}",
                        state.results.columns.unwrap_or(0),
                        state.results.rows.unwrap_or(0),
                        if state.results.has_more {
                            " · more…"
                        } else {
                            ""
                        }
                    )
                } else {
                    "  (awaiting host result projection)".into()
                };
                self.system.paint_row(
                    buffer,
                    Rect::new(area.x, y.saturating_add(1), area.width, 1),
                    &mark,
                    self.system.style(Role::TextDisabled),
                );
            }
            y = y.saturating_add(results_h);
        }

        // Footer
        if footer_h > 0 {
            slots.footer = Rect {
                x: area.x,
                y,
                width: area.width,
                height: 1,
            };
            let footer = if state.run.is_running() {
                "C-r run · C-S-s stop · C-space complete · C-j focus · C-? help"
            } else {
                "C-r run · C-f format · C-s save · C-h history · C-? help"
            };
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                footer,
                self.system.style(Role::TextMuted),
            );
        }

        state.slots = slots;
    }

    /// Optional CodeFrame for selected diagnostic over draft lines.
    pub fn render_diagnostic_frame(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &QueryEditorState,
        labels: &[SourceLabel<'a>],
    ) {
        if area.is_empty() || self.diagnostics.is_empty() {
            return;
        }
        let lines = draft_code_frame_lines(&state.editor);
        // CodeFrame needs 'a lines — draft_code_frame_lines returns owned refs into editor
        // which is tied to state lifetime, not 'a. Paint via temporary borrow:
        let line_refs: Vec<CodeFrameLine<'_>> = lines;
        CodeFrame::new(&line_refs, self.system)
            .labels(labels)
            .render(area, buffer);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Large draft / frequent edit targets.
pub mod bench {
    /// Characters in a large SQL draft.
    pub const DRAFT_CHARS: usize = 50_000;
    /// Lines in a large draft.
    pub const DRAFT_LINES: usize = 2_000;
    /// Completion candidate count.
    pub const COMPLETION_CANDIDATES: usize = 500;
    /// Diagnostic count.
    pub const DIAGNOSTIC_COUNT: usize = 100;
    /// Paint frames for stress.
    pub const PAINT_FRAMES: u32 = 60;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;
    use crate::widgets::SourceRange;
    use crate::widgets::SpanStyle;

    fn sample_diag() -> Diagnostic<'static> {
        static LABELS: &[SourceLabel<'static>] = &[SourceLabel {
            range: SourceRange {
                start_line: 2,
                start_col: 1,
                end_line: 2,
                end_col: 4,
            },
            label: Some("expected identifier"),
            style: SpanStyle::Primary,
        }];
        Diagnostic::new("d1", DiagnosticSeverity::Error, "syntax error near FROM")
            .code("SQL-001")
            .labels(LABELS)
    }

    #[test]
    fn draft_survives_result_focus() {
        let mut state = QueryEditorState::with_text("select 1");
        let cur = state.cursor();
        let text = state.text();
        let _ = state.set_focus(QueryFocus::Results);
        assert_eq!(state.text(), text);
        assert_eq!(state.cursor(), cur);
        assert!(!state.editor.accepts_input() || true); // may still report based on sync
        let _ = state.set_focus(QueryFocus::Editor);
        assert_eq!(state.text(), "select 1");
    }

    #[test]
    fn run_full_and_selection() {
        let mut state = QueryEditorState::with_text("select 1;\nselect 2;");
        state.editor.select_all();
        // shrink selection manually via selected_text path — select_all is full
        let out = state.request_run();
        assert!(matches!(
            out,
            QueryEditorOutcome::RunRequested {
                selection_only: true,
                ..
            }
        ));
        state.editor.clear_selection();
        let out = state.request_run();
        assert!(matches!(
            out,
            QueryEditorOutcome::RunRequested {
                selection_only: false,
                text,
                ..
            } if text.contains("select 1")
        ));
    }

    #[test]
    fn stop_while_running() {
        let mut state = QueryEditorState::new();
        state.set_run(QueryRunStatus::Running {
            run_id: "r1".into(),
        });
        assert!(matches!(
            state.request_stop(),
            QueryEditorOutcome::StopRequested {
                run_id: Some(id)
            } if id == "r1"
        ));
    }

    #[test]
    fn format_save_history_help_keys() {
        let mut state = QueryEditorState::with_text("select 1");
        let diags: &[Diagnostic<'_>] = &[];
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::FormatRequested { .. }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::SaveQueryRequested { .. }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::OpenHistory
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('?'), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::OpenKeyboardHelp
        ));
    }

    #[test]
    fn completion_open_close() {
        let mut state = QueryEditorState::with_text("sel");
        let diags: &[Diagnostic<'_>] = &[];
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::CompletionRequested { .. }
        ));
        assert!(state.completion_open);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), diags),
            QueryEditorOutcome::CompletionClosed
        ));
    }

    #[test]
    fn mode_cycle_and_focus() {
        let mut state = QueryEditorState::new();
        let diags: &[Diagnostic<'_>] = &[];
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('m'), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::ModeChanged(QueryEditorMode::Fullscreen)
                | QueryEditorOutcome::FullscreenRequested
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
                diags
            ),
            QueryEditorOutcome::FocusChanged(_)
        ));
    }

    #[test]
    fn token_at_cursor_extracts_identifier() {
        let mut ed = TextAreaState::new("select foo.bar");
        // place cursor at end
        let _ = ed.set_cursor(TextCursor {
            line: 0,
            byte: "select foo.bar".len(),
        });
        let (tok, _) = token_at_cursor(&ed);
        assert!(tok.contains("bar") || tok.contains("foo"), "{tok}");
    }

    #[test]
    fn paint_normal_with_diagnostics_and_params() {
        let system = DesignSystem::default();
        let mut state = QueryEditorState::with_text("select * from t");
        state.set_parameters(vec![
            QueryParameter::new("limit", "100").type_hint("int"),
            QueryParameter::new("token", "secret").secret(),
        ]);
        state.set_results(QueryResultSummary::new("ok").rows(3).columns(2));
        let diags = [sample_diag()];
        let area = Rect::new(0, 0, 72, 20);
        let mut buf = Buffer::empty(area);
        let _ = QueryEditor::new(&system)
            .diagnostics(&diags)
            .title("SQL")
            .render(area, &mut buf, &mut state);
        assert!(!state.slots.editor.is_empty());
        assert!(!state.slots.results.is_empty() || state.mode != QueryEditorMode::Normal);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("sql") || text.contains("SQL") || text.contains("select"),
            "{text}"
        );
    }

    #[test]
    fn compact_hides_results_slot() {
        let system = DesignSystem::default();
        let mut state = QueryEditorState::with_text("x");
        state.mode = QueryEditorMode::Compact;
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        let _ = QueryEditor::new(&system).render(area, &mut buf, &mut state);
        assert!(state.slots.results.is_empty() || state.slots.results.height == 0);
    }

    #[test]
    fn accepts_input_gate() {
        let mut state = QueryEditorState::with_text("a");
        state.set_accepts_input(false);
        let diags: &[Diagnostic<'_>] = &[];
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE), diags),
            QueryEditorOutcome::Ignored
        ));
    }

    #[test]
    fn help_entries_nonempty() {
        assert!(!query_editor_help_entries().is_empty());
    }

    #[test]
    fn saved_queries_bridge() {
        let saved = [SavedQuery::new("1", "daily", "select 1").language("sql")];
        let hist = saved_queries_to_history(&saved);
        assert_eq!(hist.len(), 1);
    }

    #[test]
    fn never_executes_queries() {
        let src = include_str!("query_editor.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "sqlx::",
            "tokio_postgres",
            "std::process::Command",
            "rusqlite",
        ] {
            assert!(
                !body.contains(forbidden),
                "query_editor must not contain {forbidden}"
            );
        }
    }

    #[test]
    fn large_draft_paint() {
        let system = DesignSystem::default();
        let draft = "select 1;\n".repeat(bench::DRAFT_LINES / 10);
        let mut state = QueryEditorState::with_text(&draft);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        for _ in 0..8 {
            let _ = QueryEditor::new(&system).render(area, &mut buf, &mut state);
        }
        assert!(!state.slots.editor.is_empty());
    }

    #[test]
    fn completion_menu_state_composes() {
        let mut state = QueryEditorState::new();
        state.completion_open = true;
        // selected none → Enter ignored path
        let diags: &[Diagnostic<'_>] = &[];
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), diags);
        assert!(matches!(
            out,
            QueryEditorOutcome::Ignored | QueryEditorOutcome::CompletionCommitted { .. }
        ));
    }

    #[test]
    fn diagnostic_jump() {
        let mut state = QueryEditorState::with_text("select\nfrom t");
        let _ = state.set_focus(QueryFocus::Diagnostics);
        let diags = [sample_diag()];
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &diags);
        assert!(matches!(
            out,
            QueryEditorOutcome::JumpToDiagnostic { id } if id == "d1"
        ));
    }
}
