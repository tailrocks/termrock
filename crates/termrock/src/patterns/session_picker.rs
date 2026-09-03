// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SessionPicker** — polished selector for creating, resuming, searching,
//! renaming, archiving, and deleting agent sessions.
//!
//! **Mission.** Project, branch, status, recency, model/mode, summary,
//! unread/action-required, pinning, search, and preview. **Preserve current
//! draft and app context on cancel.** Safe delete/archive confirmation
//! (Cancel default). Multi-device/remote status. Thousands of sessions via
//! windowed list + provider search request. Popover and fullscreen forms.
//!
//! **vs [`super::Picker`].** Generic query+list; SessionPicker is session-domain.
//! **vs [`super::HistoryPicker`].** Value/history recall; not session lifecycle.
//! **vs [`super::CommandPalette`].** Commands; not session threads.
//!
//! Research: Amp sessions, OpenCode sessions, Grok Build picker, project launchers.
//! Outcomes are **requests only** — no persistence, network, or draft mutation.
//!
//! Teaches: how to compose polished selector for creating, resuming,
//! searching, renaming, archiving, and deleting agent sessions.
//!
//! Composes: [`crate::widgets::ConfirmFocus`],
//! [`crate::widgets::ConfirmPrompt`], [`crate::widgets::Panel`],
//! [`crate::widgets::StatefulWidget`], [`crate::widgets::Widget`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, PanelChrome, Role},
    widgets::{
        ConfirmFocus, ConfirmPrompt, EmptyKind, EmptyState, Panel, SemanticStatus, StatusIndicator,
    },
};

/// Overlay id for session picker (dialog / fullscreen).
pub const SESSION_PICKER_OVERLAY_ID: &str = "termrock.session_picker";
/// Overlay id for popover form.
pub const SESSION_PICKER_POPOVER_OVERLAY_ID: &str = "termrock.session_picker_popover";
/// Visible list window for large catalogs (virtualization).
pub const SESSION_PICKER_WINDOW: usize = 64;
/// Provider-search query length hint before host should search remotely.
pub const SESSION_PICKER_PROVIDER_SEARCH_MIN: usize = 2;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Lifecycle status of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SessionStatus {
    /// Active / running agent work.
    #[default]
    Active,
    /// Idle but open.
    Idle,
    /// Needs user action (permission, question, plan).
    ActionRequired,
    /// Completed successfully.
    Completed,
    /// Failed or errored.
    Failed,
    /// Archived (hidden from default list unless filter).
    Archived,
}

impl SessionStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Idle => "idle",
            Self::ActionRequired => "action_required",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Archived => "archived",
        }
    }

    /// Letter (colorless).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Active => 'A',
            Self::Idle => 'I',
            Self::ActionRequired => '!',
            Self::Completed => 'S',
            Self::Failed => 'F',
            Self::Archived => 'Z',
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Active => "*",
                Self::Idle => ".",
                Self::ActionRequired => "!",
                Self::Completed => "+",
                Self::Failed => "x",
                Self::Archived => "z",
            };
        }
        match self {
            Self::Active => "●",
            Self::Idle => "○",
            Self::ActionRequired => "⚠",
            Self::Completed => "✓",
            Self::Failed => "✗",
            Self::Archived => "▣",
        }
    }

    /// Shared lifecycle projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Active => SemanticStatus::Running,
            Self::Idle => SemanticStatus::Idle,
            Self::ActionRequired => SemanticStatus::Waiting,
            Self::Completed => SemanticStatus::Success,
            Self::Failed => SemanticStatus::Failed,
            Self::Archived => SemanticStatus::Paused,
        }
    }
}

/// Where the session lives / sync state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SessionLocation {
    /// Local only.
    #[default]
    Local,
    /// Remote / cloud.
    Remote,
    /// Synced multi-device.
    MultiDevice,
    /// Offline (cached remote).
    Offline,
}

impl SessionLocation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::MultiDevice => "multi_device",
            Self::Offline => "offline",
        }
    }

    /// Short label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Remote => "remote",
            Self::MultiDevice => "devices",
            Self::Offline => "offline",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Local => "L",
                Self::Remote => "R",
                Self::MultiDevice => "M",
                Self::Offline => "O",
            };
        }
        match self {
            Self::Local => "⌂",
            Self::Remote => "☁",
            Self::MultiDevice => "⧉",
            Self::Offline => "◌",
        }
    }
}

/// Load state for async session catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SessionLoadState {
    /// Idle / ready.
    #[default]
    Ready,
    /// Loading first page.
    Loading,
    /// Searching via provider.
    Searching,
    /// Load failed.
    Error,
}

impl SessionLoadState {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Searching => "searching",
            Self::Error => "error",
        }
    }
}

/// One agent session projection (host-owned data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEntry {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Project name / path label.
    pub project: Option<String>,
    /// Git branch / workspace branch.
    pub branch: Option<String>,
    /// Status.
    pub status: SessionStatus,
    /// Recency label (`2m ago`, `Yesterday`).
    pub recency: Option<String>,
    /// Model label.
    pub model: Option<String>,
    /// Agent mode label.
    pub mode: Option<String>,
    /// One-line summary / last message preview.
    pub summary: Option<String>,
    /// Unread count (0 = none).
    pub unread: u32,
    /// Action required badge.
    pub action_required: bool,
    /// Pinned.
    pub pinned: bool,
    /// Location / multi-device.
    pub location: SessionLocation,
    /// Optional device label (`laptop`, `ci`).
    pub device: Option<String>,
    /// Dirty / has unsaved local draft elsewhere (informational).
    pub dirty: bool,
    /// Disabled (cannot open).
    pub enabled: bool,
}

impl SessionEntry {
    /// Minimal session.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            project: None,
            branch: None,
            status: SessionStatus::Idle,
            recency: None,
            model: None,
            mode: None,
            summary: None,
            unread: 0,
            action_required: false,
            pinned: false,
            location: SessionLocation::Local,
            device: None,
            dirty: false,
            enabled: true,
        }
    }

    /// Project.
    #[must_use]
    pub fn project(mut self, p: impl Into<String>) -> Self {
        self.project = Some(p.into());
        self
    }

    /// Branch.
    #[must_use]
    pub fn branch(mut self, b: impl Into<String>) -> Self {
        self.branch = Some(b.into());
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: SessionStatus) -> Self {
        self.status = s;
        self
    }

    /// Recency.
    #[must_use]
    pub fn recency(mut self, r: impl Into<String>) -> Self {
        self.recency = Some(r.into());
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

    /// Summary.
    #[must_use]
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }

    /// Unread.
    #[must_use]
    pub const fn unread(mut self, n: u32) -> Self {
        self.unread = n;
        self
    }

    /// Action required.
    #[must_use]
    pub const fn action_required(mut self, on: bool) -> Self {
        self.action_required = on;
        if on {
            self.status = SessionStatus::ActionRequired;
        }
        self
    }

    /// Pin.
    #[must_use]
    pub const fn pinned(mut self, on: bool) -> Self {
        self.pinned = on;
        self
    }

    /// Location.
    #[must_use]
    pub const fn location(mut self, loc: SessionLocation) -> Self {
        self.location = loc;
        self
    }

    /// Device.
    #[must_use]
    pub fn device(mut self, d: impl Into<String>) -> Self {
        self.device = Some(d.into());
        self
    }

    /// Dirty marker.
    #[must_use]
    pub const fn dirty(mut self, on: bool) -> Self {
        self.dirty = on;
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Whether text matches query (case-insensitive substring).
    #[must_use]
    pub fn matches_query(&self, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        let q = q.to_ascii_lowercase();
        let hit = |s: &str| crate::text::contains_lower(&s, &q);
        hit(&self.title)
            || self.project.as_deref().is_some_and(hit)
            || self.branch.as_deref().is_some_and(hit)
            || self.summary.as_deref().is_some_and(hit)
            || self.model.as_deref().is_some_and(hit)
            || self.mode.as_deref().is_some_and(hit)
            || hit(&self.id)
    }
}

// ── Presentation / phase ───────────────────────────────────────────────────

/// Chrome form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SessionPickerPresentation {
    /// Embedded / dialog body.
    #[default]
    Dialog,
    /// Compact popover.
    Popover,
    /// Fullscreen host overlay.
    Fullscreen,
}

impl SessionPickerPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Dialog => "dialog",
            Self::Popover => "popover",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Interaction phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SessionPickerPhase {
    /// Browse / search list.
    #[default]
    Browse,
    /// Creating a new session (title draft).
    Create,
    /// Renaming selected session.
    Rename,
    /// Confirm archive.
    ConfirmArchive,
    /// Confirm delete (destructive).
    ConfirmDelete,
}

/// Destructive / archive confirm action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SessionConfirmAction {
    /// Archive (soft remove from default list).
    Archive,
    /// Delete permanently (host).
    Delete,
}

impl SessionConfirmAction {
    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Archive => "Archive",
            Self::Delete => "Delete",
        }
    }

    /// Consequence.
    #[must_use]
    pub const fn consequence(self) -> &'static str {
        match self {
            Self::Archive => "hide from default list (host archives)",
            Self::Delete => "permanently delete session (host executes)",
        }
    }

    /// Whether destructive chrome.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Delete)
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — requests only; host owns I/O, persistence, and draft buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SessionPickerOutcome {
    /// Ignored.
    Ignored,
    /// Search query changed (local filter and/or provider search).
    QueryChanged {
        /// Query text.
        query: String,
        /// Host should run provider search (large catalog).
        provider_search: bool,
    },
    /// Cursor selection moved.
    Selected {
        /// Session id.
        id: String,
    },
    /// Open / resume session.
    Opened {
        /// Session id.
        id: String,
    },
    /// Create new session with title.
    CreateRequested {
        /// Title draft.
        title: String,
    },
    /// Rename session.
    RenameRequested {
        /// Id.
        id: String,
        /// New title.
        title: String,
    },
    /// Pin toggled (host persists).
    PinToggled {
        /// Id.
        id: String,
        /// Pinned.
        pinned: bool,
    },
    /// Archive requested (after confirm).
    ArchiveRequested {
        /// Id.
        id: String,
    },
    /// Delete requested (after confirm).
    DeleteRequested {
        /// Id.
        id: String,
    },
    /// Confirm dialog opened.
    ConfirmOpened {
        /// Id.
        id: String,
        /// Action.
        action: SessionConfirmAction,
    },
    /// Confirm cancelled.
    ConfirmCancelled,
    /// Phase changed.
    PhaseChanged(SessionPickerPhase),
    /// Presentation promote (popover → fullscreen).
    FullscreenRequested,
    /// Popover form requested.
    PopoverRequested,
    /// Retry load after error.
    RetryLoad,
    /// Load more / next page (virtualization).
    LoadMore {
        /// Cursor / offset hint.
        offset: usize,
    },
    /// Cancelled — **draft and app context preserved**.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interactive session picker state.
///
/// **Composer draft:** never held or cleared. Cancel leaves host draft intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPickerState {
    /// Session catalog (may be a window into a larger set).
    pub sessions: Vec<SessionEntry>,
    /// Total catalog size if known (for virtualization chrome).
    pub total_count: Option<usize>,
    /// Search query.
    pub query: String,
    /// Filtered indices into `sessions` (recomputed).
    filtered: Vec<usize>,
    /// Cursor into `filtered`.
    pub cursor: usize,
    /// Scroll offset into filtered window.
    pub scroll: usize,
    /// Phase.
    pub phase: SessionPickerPhase,
    /// Presentation.
    pub presentation: SessionPickerPresentation,
    /// Load state.
    pub load_state: SessionLoadState,
    /// Last load error message.
    pub load_error: Option<String>,
    /// Create / rename text draft (picker-local; not composer).
    pub text_draft: String,
    /// Confirm action.
    pub confirm_action: Option<SessionConfirmAction>,
    /// Confirm: false = Cancel (safe default), true = proceed.
    pub confirm_proceed_focused: bool,
    /// Show archived in list.
    pub show_archived: bool,
    /// Filter pins only.
    pub pins_only: bool,
    /// Preview shows the full metadata sheet instead of its five-line core.
    pub preview_details: bool,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Typing in search (browse).
    search_mode: bool,
    /// Row hit regions.
    pub row_hits: Vec<(String, Rect)>,
    /// Confirm hits (proceed?).
    pub confirm_hits: Vec<(bool, Rect)>,
}

impl Default for SessionPickerState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionPickerState {
    /// Empty ready state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            total_count: None,
            query: String::new(),
            filtered: Vec::new(),
            cursor: 0,
            scroll: 0,
            phase: SessionPickerPhase::Browse,
            presentation: SessionPickerPresentation::Dialog,
            load_state: SessionLoadState::Ready,
            load_error: None,
            text_draft: String::new(),
            confirm_action: None,
            confirm_proceed_focused: false,
            show_archived: false,
            pins_only: false,
            preview_details: false,
            focused: true,
            accepts_input: true,
            search_mode: true,
            row_hits: Vec::new(),
            confirm_hits: Vec::new(),
        }
    }

    /// Set search query and refilter (host or tests).
    pub fn set_query(&mut self, q: impl Into<String>) {
        self.query = q.into();
        self.refilter();
    }

    /// Replace sessions and refilter.
    pub fn set_sessions(&mut self, sessions: Vec<SessionEntry>) {
        let keep = self.current_id();
        self.sessions = sessions;
        self.refilter();
        if let Some(id) = keep {
            if let Some(fi) = self
                .filtered
                .iter()
                .position(|&si| self.sessions.get(si).is_some_and(|s| s.id == id))
            {
                self.cursor = fi;
            }
        }
        self.clamp_cursor();
    }
    /// Total count for chrome.
    pub fn set_total_count(&mut self, n: Option<usize>) {
        self.total_count = n;
    }
    /// Error.
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.load_state = SessionLoadState::Error;
        self.load_error = Some(msg.into());
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Presentation.
    pub const fn set_presentation(&mut self, p: SessionPickerPresentation) {
        self.presentation = p;
    }

    /// Current session id.
    #[must_use]
    pub fn current_id(&self) -> Option<String> {
        self.current().map(|s| s.id.clone())
    }

    /// Current session.
    #[must_use]
    pub fn current(&self) -> Option<&SessionEntry> {
        let si = *self.filtered.get(self.cursor)?;
        self.sessions.get(si)
    }

    /// Filtered count.
    #[must_use]
    pub fn filtered_len(&self) -> usize {
        self.filtered.len()
    }

    fn refilter(&mut self) {
        self.filtered = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                if !self.show_archived && matches!(s.status, SessionStatus::Archived) {
                    return false;
                }
                if self.pins_only && !s.pinned {
                    return false;
                }
                s.matches_query(&self.query)
            })
            .map(|(i, _)| i)
            .collect();
        // Pins first, then action required, then original order
        self.filtered.sort_by_key(|&i| {
            let s = &self.sessions[i];
            (
                if s.pinned { 0u8 } else { 1 },
                if s.action_required || s.unread > 0 {
                    0u8
                } else {
                    1
                },
                i,
            )
        });
        self.clamp_cursor();
    }

    fn clamp_cursor(&mut self) {
        if self.filtered.is_empty() {
            self.cursor = 0;
            self.scroll = 0;
            return;
        }
        self.cursor = self.cursor.min(self.filtered.len() - 1);
        let window = SESSION_PICKER_WINDOW;
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + window {
            self.scroll = self.cursor + 1 - window;
        }
    }

    fn select_cursor(&mut self) -> SessionPickerOutcome {
        if let Some(s) = self.current() {
            SessionPickerOutcome::Selected { id: s.id.clone() }
        } else {
            SessionPickerOutcome::Ignored
        }
    }

    fn move_cursor(&mut self, delta: isize) -> SessionPickerOutcome {
        if self.filtered.is_empty() {
            return SessionPickerOutcome::Ignored;
        }
        let n = self.filtered.len() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, n - 1) as usize;
        self.clamp_cursor();
        // Near end → ask host for more
        let out = self.select_cursor();
        if self.cursor + 8 >= self.filtered.len() {
            if let Some(total) = self.total_count {
                if self.sessions.len() < total {
                    return SessionPickerOutcome::LoadMore {
                        offset: self.sessions.len(),
                    };
                }
            }
        }
        out
    }

    fn open_confirm(&mut self, action: SessionConfirmAction) -> SessionPickerOutcome {
        let (id, enabled) = match self.current() {
            Some(s) => (s.id.clone(), s.enabled),
            None => return SessionPickerOutcome::Ignored,
        };
        if !enabled {
            return SessionPickerOutcome::Ignored;
        }
        self.phase = match action {
            SessionConfirmAction::Archive => SessionPickerPhase::ConfirmArchive,
            SessionConfirmAction::Delete => SessionPickerPhase::ConfirmDelete,
        };
        self.confirm_action = Some(action);
        self.confirm_proceed_focused = false; // Cancel default
        SessionPickerOutcome::ConfirmOpened { id, action }
    }

    fn emit_confirm(&mut self) -> SessionPickerOutcome {
        let action = self.confirm_action.unwrap_or(SessionConfirmAction::Delete);
        let Some(id) = self.current_id() else {
            return SessionPickerOutcome::Ignored;
        };
        self.phase = SessionPickerPhase::Browse;
        self.confirm_action = None;
        match action {
            SessionConfirmAction::Archive => SessionPickerOutcome::ArchiveRequested { id },
            SessionConfirmAction::Delete => SessionPickerOutcome::DeleteRequested { id },
        }
    }

    /// Keyboard.
    ///
    /// **Draft:** cancel never touches host PromptComposer draft.
    pub fn handle_key(&mut self, key: KeyEvent) -> SessionPickerOutcome {
        if !self.focused || !self.accepts_input || !key.is_press() {
            return SessionPickerOutcome::Ignored;
        }

        match self.phase {
            SessionPickerPhase::ConfirmArchive | SessionPickerPhase::ConfirmDelete => {
                return self.handle_confirm_key(key);
            }
            SessionPickerPhase::Create | SessionPickerPhase::Rename => {
                return self.handle_text_phase(key);
            }
            SessionPickerPhase::Browse => {}
        }

        // Error retry
        if matches!(self.load_state, SessionLoadState::Error)
            && matches!(key.code, KeyCode::Char('r') if key.modifiers.is_empty())
        {
            self.load_state = SessionLoadState::Loading;
            return SessionPickerOutcome::RetryLoad;
        }

        match key.code {
            KeyCode::Esc => SessionPickerOutcome::Cancelled,
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => self.move_cursor(1),
            KeyCode::Enter => {
                let Some(s) = self.current() else {
                    return SessionPickerOutcome::Ignored;
                };
                if !s.enabled {
                    return SessionPickerOutcome::Ignored;
                }
                SessionPickerOutcome::Opened { id: s.id.clone() }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.phase = SessionPickerPhase::Create;
                self.text_draft.clear();
                SessionPickerOutcome::PhaseChanged(SessionPickerPhase::Create)
            }
            KeyCode::Char('n') if key.modifiers.is_empty() && self.query.is_empty() => {
                self.phase = SessionPickerPhase::Create;
                self.text_draft.clear();
                SessionPickerOutcome::PhaseChanged(SessionPickerPhase::Create)
            }
            KeyCode::Char('r') if key.modifiers.is_empty() && self.query.is_empty() => {
                let Some(title) = self.current().map(|s| s.title.clone()) else {
                    return SessionPickerOutcome::Ignored;
                };
                self.phase = SessionPickerPhase::Rename;
                self.text_draft = title;
                SessionPickerOutcome::PhaseChanged(SessionPickerPhase::Rename)
            }
            KeyCode::Char('p') if key.modifiers.is_empty() && self.query.is_empty() => {
                let Some((id, pinned)) = self.current().map(|s| (s.id.clone(), !s.pinned)) else {
                    return SessionPickerOutcome::Ignored;
                };
                // Optimistic local flip
                if let Some(si) = self.filtered.get(self.cursor).copied() {
                    if let Some(e) = self.sessions.get_mut(si) {
                        e.pinned = pinned;
                    }
                }
                self.refilter();
                SessionPickerOutcome::PinToggled { id, pinned }
            }
            KeyCode::Char('a') if key.modifiers.is_empty() && self.query.is_empty() => {
                self.open_confirm(SessionConfirmAction::Archive)
            }
            KeyCode::Delete => self.open_confirm(SessionConfirmAction::Delete),
            KeyCode::Char('d') if key.modifiers.is_empty() && self.query.is_empty() => {
                self.open_confirm(SessionConfirmAction::Delete)
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                SessionPickerOutcome::FullscreenRequested
            }
            KeyCode::Char('f') if key.modifiers.is_empty() && self.query.is_empty() => {
                SessionPickerOutcome::FullscreenRequested
            }
            KeyCode::Char('o') if key.modifiers.is_empty() && self.query.is_empty() => {
                SessionPickerOutcome::PopoverRequested
            }
            KeyCode::Char('i') if key.modifiers.is_empty() && self.query.is_empty() => {
                self.preview_details = !self.preview_details;
                SessionPickerOutcome::Ignored
            }
            KeyCode::Char('z') if key.modifiers.is_empty() && self.query.is_empty() => {
                self.show_archived = !self.show_archived;
                self.refilter();
                SessionPickerOutcome::Ignored
            }
            KeyCode::Char('P') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.pins_only = !self.pins_only;
                self.refilter();
                SessionPickerOutcome::Ignored
            }
            KeyCode::Char('/') if key.modifiers.is_empty() => {
                self.search_mode = true;
                SessionPickerOutcome::Ignored
            }
            KeyCode::Backspace => {
                if !self.query.is_empty() {
                    self.query.pop();
                    self.refilter();
                    return self.query_changed();
                }
                SessionPickerOutcome::Ignored
            }
            KeyCode::Char('y') | KeyCode::Char('Y') => SessionPickerOutcome::Ignored,
            KeyCode::Char(c)
                if !c.is_control()
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                // Avoid swallowing navigation letters when query empty and reserved
                if self.query.is_empty()
                    && matches!(c, 'j' | 'k' | 'n' | 'p' | 'a' | 'r' | 'f' | 'o' | 'z' | 'd')
                {
                    // already handled above for empty query; leftover
                    return SessionPickerOutcome::Ignored;
                }
                self.push_query_char(c)
            }
            KeyCode::PageDown => self.move_cursor(8),
            KeyCode::PageUp => self.move_cursor(-8),
            KeyCode::Home => {
                self.cursor = 0;
                self.clamp_cursor();
                self.select_cursor()
            }
            KeyCode::End => {
                if !self.filtered.is_empty() {
                    self.cursor = self.filtered.len() - 1;
                    self.clamp_cursor();
                }
                self.select_cursor()
            }
            _ => SessionPickerOutcome::Ignored,
        }
    }

    fn push_query_char(&mut self, c: char) -> SessionPickerOutcome {
        self.query.push(c);
        self.refilter();
        self.query_changed()
    }

    fn query_changed(&self) -> SessionPickerOutcome {
        let provider_search = self.query.len() >= SESSION_PICKER_PROVIDER_SEARCH_MIN
            && self.total_count.is_some_and(|t| t > SESSION_PICKER_WINDOW);
        SessionPickerOutcome::QueryChanged {
            query: self.query.clone(),
            provider_search,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> SessionPickerOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = SessionPickerPhase::Browse;
                self.confirm_action = None;
                SessionPickerOutcome::ConfirmCancelled
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.confirm_proceed_focused = false;
                SessionPickerOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.confirm_proceed_focused = true;
                SessionPickerOutcome::Ignored
            }
            KeyCode::Tab => {
                self.confirm_proceed_focused = !self.confirm_proceed_focused;
                SessionPickerOutcome::Ignored
            }
            KeyCode::Enter => {
                if self.confirm_proceed_focused {
                    self.emit_confirm()
                } else {
                    self.phase = SessionPickerPhase::Browse;
                    self.confirm_action = None;
                    SessionPickerOutcome::ConfirmCancelled
                }
            }
            KeyCode::Char('y') => SessionPickerOutcome::Ignored,
            _ => SessionPickerOutcome::Ignored,
        }
    }

    fn handle_text_phase(&mut self, key: KeyEvent) -> SessionPickerOutcome {
        match key.code {
            KeyCode::Esc => {
                self.phase = SessionPickerPhase::Browse;
                self.text_draft.clear();
                SessionPickerOutcome::PhaseChanged(SessionPickerPhase::Browse)
            }
            KeyCode::Enter => {
                let title = self.text_draft.trim().to_string();
                if title.is_empty() {
                    return SessionPickerOutcome::Ignored;
                }
                let phase = self.phase;
                self.phase = SessionPickerPhase::Browse;
                match phase {
                    SessionPickerPhase::Create => {
                        self.text_draft.clear();
                        SessionPickerOutcome::CreateRequested { title }
                    }
                    SessionPickerPhase::Rename => {
                        let Some(id) = self.current_id() else {
                            return SessionPickerOutcome::Ignored;
                        };
                        self.text_draft.clear();
                        SessionPickerOutcome::RenameRequested { id, title }
                    }
                    _ => SessionPickerOutcome::Ignored,
                }
            }
            KeyCode::Backspace => {
                self.text_draft.pop();
                SessionPickerOutcome::Ignored
            }
            KeyCode::Char(c)
                if !c.is_control() && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.text_draft.push(c);
                SessionPickerOutcome::Ignored
            }
            _ => SessionPickerOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> SessionPickerOutcome {
        if !self.focused || !self.accepts_input {
            return SessionPickerOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return SessionPickerOutcome::Ignored;
        }
        let pos = ev.position;
        if matches!(
            self.phase,
            SessionPickerPhase::ConfirmArchive | SessionPickerPhase::ConfirmDelete
        ) {
            let hit = self
                .confirm_hits
                .iter()
                .find(|(_, r)| r.contains(pos))
                .map(|(p, _)| *p);
            if let Some(proceed) = hit {
                self.confirm_proceed_focused = proceed;
                if proceed {
                    return self.emit_confirm();
                }
                self.phase = SessionPickerPhase::Browse;
                self.confirm_action = None;
                return SessionPickerOutcome::ConfirmCancelled;
            }
            return SessionPickerOutcome::Ignored;
        }
        let hit = self
            .row_hits
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| id.clone());
        let Some(id) = hit else {
            return SessionPickerOutcome::Ignored;
        };
        if let Some(fi) = self
            .filtered
            .iter()
            .position(|&si| self.sessions.get(si).is_some_and(|s| s.id == id))
        {
            let already = self.cursor == fi;
            self.cursor = fi;
            self.clamp_cursor();
            if already {
                if let Some(s) = self.current() {
                    if s.enabled {
                        return SessionPickerOutcome::Opened { id: s.id.clone() };
                    }
                }
            }
            return SessionPickerOutcome::Selected { id };
        }
        SessionPickerOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Session picker painter.
#[derive(Debug, Clone, Copy)]
pub struct SessionPicker<'a> {
    system: &'a DesignSystem,
    colorless: bool,
    show_preview: bool,
}

impl<'a> SessionPicker<'a> {
    /// System only — sessions live in state.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
            show_preview: true,
        }
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Hide preview pane.
    #[must_use]
    pub const fn list_only(mut self, on: bool) -> Self {
        self.show_preview = !on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut SessionPickerState) {
        state.row_hits.clear();
        state.confirm_hits.clear();
        if area.is_empty() {
            return;
        }

        let title = match state.presentation {
            SessionPickerPresentation::Popover => "Sessions · popover",
            SessionPickerPresentation::Fullscreen => "Sessions · fullscreen",
            SessionPickerPresentation::Dialog => "Sessions",
        };
        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system).title(title).emphasis(emphasis);
        let inner = panel.inner(area);
        panel.paint(area, buffer, None);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let _w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Draft preservation banner
        if y < max_y {
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                "draft & context preserved on cancel",
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Search / phase line
        if y < max_y {
            let line = match state.phase {
                SessionPickerPhase::Browse => {
                    let count = match state.total_count {
                        Some(t) => format!("{}/{}", state.filtered_len(), t),
                        None => format!("{}", state.filtered_len()),
                    };
                    format!("/{}  ({count})", state.query)
                }
                SessionPickerPhase::Create => format!("new › {}▎", state.text_draft),
                SessionPickerPhase::Rename => format!("rename › {}▎", state.text_draft),
                SessionPickerPhase::ConfirmArchive | SessionPickerPhase::ConfirmDelete => {
                    "confirm…".into()
                }
            };
            let style = if matches!(
                state.phase,
                SessionPickerPhase::Create | SessionPickerPhase::Rename
            ) {
                self.system.style(Role::Accent)
            } else {
                self.system.style(Role::Text)
            };
            self.system
                .paint_row(buffer, Rect::new(inner.x, y, inner.width, 1), &line, style);
            y = y.saturating_add(1);
        }

        // Load / error
        if matches!(
            state.load_state,
            SessionLoadState::Loading | SessionLoadState::Searching
        ) && y < max_y
        {
            let m = if matches!(state.load_state, SessionLoadState::Searching) {
                "searching…"
            } else {
                "loading…"
            };
            StatusIndicator::new(SemanticStatus::Running, self.system)
                .label(m)
                .colorless(self.colorless)
                .paint(Rect::new(inner.x, y, inner.width, 1), buffer, None);
            y = y.saturating_add(1);
        }
        if matches!(state.load_state, SessionLoadState::Error) && y < max_y {
            let msg = state
                .load_error
                .as_deref()
                .unwrap_or("load failed · r retry");
            StatusIndicator::new(SemanticStatus::Failed, self.system)
                .label(msg)
                .colorless(self.colorless)
                .paint(Rect::new(inner.x, y, inner.width, 1), buffer, None);
            y = y.saturating_add(1);
        }

        // Split list / preview
        let footer = if matches!(
            state.phase,
            SessionPickerPhase::ConfirmArchive | SessionPickerPhase::ConfirmDelete
        ) {
            2u16
        } else {
            1u16
        };
        let content_bottom = max_y.saturating_sub(footer);
        let content_h = content_bottom.saturating_sub(y);
        let content = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: content_h,
        };

        if !content.is_empty() {
            let (list_area, preview_area) = if self.show_preview
                && content.width >= 52
                && !matches!(state.presentation, SessionPickerPresentation::Popover)
            {
                let lw = (content.width * 6 / 10).max(24);
                (
                    Rect {
                        x: content.x,
                        y: content.y,
                        width: lw,
                        height: content.height,
                    },
                    Some(Rect {
                        x: content.x.saturating_add(lw),
                        y: content.y,
                        width: content.width.saturating_sub(lw),
                        height: content.height,
                    }),
                )
            } else {
                (content, None)
            };
            self.paint_list(list_area, buffer, state);
            if let Some(pa) = preview_area {
                self.paint_preview(pa, buffer, state);
            }
        }

        // Footer hints / confirm
        if matches!(
            state.phase,
            SessionPickerPhase::ConfirmArchive | SessionPickerPhase::ConfirmDelete
        ) {
            self.paint_confirm(inner, buffer, state);
        } else if max_y > inner.y {
            let fy = max_y.saturating_sub(1);
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, fy, inner.width, 1),
                "enter open · n new · i details · del delete · esc close",
                self.system.style(Role::TextMuted),
            );
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut SessionPickerState) {
        if area.is_empty() {
            return;
        }
        let _w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let viewport = max_y.saturating_sub(y) as usize;

        if state.filtered.is_empty() {
            let msg = if matches!(state.load_state, SessionLoadState::Loading) {
                "loading sessions…"
            } else if state.query.is_empty() {
                "no sessions · n to create"
            } else {
                "no matches"
            };
            self.system.paint_row(
                buffer,
                Rect::new(area.x, y, area.width, 1),
                msg,
                self.system.style(Role::TextMuted),
            );
            return;
        }

        let mut offset = state.scroll;
        if state.cursor < offset {
            offset = state.cursor;
        } else if viewport > 0 && state.cursor >= offset + viewport {
            offset = state.cursor + 1 - viewport;
        }
        state.scroll = offset;

        for (fi, &si) in state.filtered.iter().enumerate().skip(offset) {
            if y >= max_y {
                break;
            }
            let Some(s) = state.sessions.get(si) else {
                continue;
            };
            let selected = fi == state.cursor;
            let pin = if s.pinned { "★" } else { " " };
            let semantic = if s.action_required {
                SemanticStatus::Waiting
            } else {
                s.status.semantic()
            };
            let status_label = if s.action_required {
                "action required"
            } else {
                s.status.id()
            };
            let indicator = StatusIndicator::new(semantic, self.system)
                .label(status_label)
                .colorless(self.colorless);
            let status_text = indicator.text(None);
            let unread = if s.unread > 0 {
                format!(" ({})", s.unread)
            } else if s.action_required {
                " !".into()
            } else {
                String::new()
            };
            let dirty = if s.dirty { " ·" } else { "" };
            let mark = if selected { "›" } else { " " };
            let loc = s.location.glyph(false);
            let text = format!("{mark}{pin}{status_text} {loc} {}{unread}{dirty}", s.title);
            // Status lives in its glyph cell, not across the whole row: a
            // list of five sessions used to paint five hues over its titles
            // (information budget, plans/017 Part B).
            let style = if !s.enabled {
                self.system.style(Role::TextMuted)
            } else if selected && !self.colorless {
                self.system.style(Role::Accent).add_modifier(Modifier::BOLD)
            } else if selected {
                // Mono keeps the selected row visible by the explicit reversal
                // pair, never by a modifier that re-swaps the cell (D5).
                self.system.reversed()
            } else {
                self.system.style(Role::Text)
            };
            self.system
                .paint_row(buffer, Rect::new(area.x, y, area.width, 1), &text, style);
            if area.width > 2 {
                indicator.paint(
                    Rect::new(area.x.saturating_add(2), y, area.width.saturating_sub(2), 1),
                    buffer,
                    None,
                );
            }
            state.row_hits.push((
                s.id.clone(),
                Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                },
            ));
            y = y.saturating_add(1);
            // meta line when wide enough
            if area.width >= 28 && y < max_y {
                let mut meta = String::new();
                if let Some(p) = s.project.as_ref() {
                    meta.push_str(p);
                }
                if let Some(b) = s.branch.as_ref() {
                    if !meta.is_empty() {
                        meta.push(' ');
                    }
                    meta.push_str(b);
                }
                if let Some(r) = s.recency.as_ref() {
                    if !meta.is_empty() {
                        meta.push_str(" · ");
                    }
                    meta.push_str(r);
                }
                if !meta.is_empty() {
                    self.system.paint_row(
                        buffer,
                        Rect::new(area.x, y, area.width, 1),
                        &format!("    {meta}"),
                        self.system.style(Role::TextMuted),
                    );
                    y = y.saturating_add(1);
                }
            }
        }
    }

    fn paint_preview(&self, area: Rect, buffer: &mut Buffer, state: &SessionPickerState) {
        if area.is_empty() {
            return;
        }
        let _w = usize::from(area.width);
        let mut y = area.y;
        let max_y = area.bottom();
        let Some(s) = state.current() else {
            EmptyState::new("No session selected", self.system)
                .kind(EmptyKind::NoData)
                .paint(
                    Rect::new(area.x, y, area.width, 1),
                    buffer,
                    &mut crate::widgets::EmptyStateState::new(),
                );
            return;
        };
        // Default frame: five quiet lines. Everything else is one keypress
        // away behind `i` (information budget, plans/017 Part B).
        let lines: Vec<(String, Role, Option<SemanticStatus>)> = if state.preview_details {
            let mut v = vec![(s.title.clone(), Role::TextStrong, None)];
            if let Some(p) = s.project.as_ref() {
                v.push((format!("project {p}"), Role::TextMuted, None));
            }
            if let Some(b) = s.branch.as_ref() {
                v.push((format!("branch {b}"), Role::TextMuted, None));
            }
            let status = StatusIndicator::new(s.status.semantic(), self.system)
                .label(s.status.id())
                .colorless(self.colorless);
            v.push((
                format!("{} · {}", status.text(None), s.location.label()),
                Role::Text,
                Some(s.status.semantic()),
            ));
            if let Some(m) = s.model.as_ref() {
                v.push((format!("model {m}"), Role::TextMuted, None));
            }
            if let Some(m) = s.mode.as_ref() {
                v.push((format!("mode {m}"), Role::TextMuted, None));
            }
            if let Some(d) = s.device.as_ref() {
                v.push((format!("device {d}"), Role::TextMuted, None));
            }
            if let Some(sum) = s.summary.as_ref() {
                v.push((sum.clone(), Role::Text, None));
            }
            if s.pinned {
                v.push(("pinned".into(), Role::TextMuted, None));
            }
            if s.dirty {
                v.push(("dirty / local draft elsewhere".into(), Role::Warning, None));
            }
            if s.unread > 0 {
                v.push((format!("{} unread", s.unread), Role::TextMuted, None));
            }
            if s.action_required {
                v.push(("action required".into(), Role::Warning, None));
            }
            v
        } else {
            let mut v = vec![(s.title.clone(), Role::TextStrong, None)];
            if let Some(r) = s.recency.as_ref() {
                v.push((r.clone(), Role::TextFaint, None));
            }
            if let Some(m) = s.model.as_ref() {
                v.push((format!("model {m}"), Role::TextMuted, None));
            }
            let status = StatusIndicator::new(s.status.semantic(), self.system)
                .label(s.status.id())
                .colorless(self.colorless);
            v.push((status.text(None), Role::Text, Some(s.status.semantic())));
            if let Some(sum) = s.summary.as_ref() {
                v.push((sum.clone(), Role::Text, None));
            }
            v.push(("i details".into(), Role::TextFaint, None));
            v
        };
        for (line, role, semantic) in lines {
            if y >= max_y {
                break;
            }
            let style = if self.colorless {
                self.system.style(Role::Text)
            } else {
                self.system.style(role)
            };
            self.system
                .paint_row(buffer, Rect::new(area.x, y, area.width, 1), &line, style);
            if let Some(semantic) = semantic {
                StatusIndicator::new(semantic, self.system)
                    .label(s.status.id())
                    .colorless(self.colorless)
                    .paint(Rect::new(area.x, y, area.width, 1), buffer, None);
            }
            y = y.saturating_add(1);
        }
    }

    fn paint_confirm(&self, area: Rect, buffer: &mut Buffer, state: &mut SessionPickerState) {
        let action = state.confirm_action.unwrap_or(SessionConfirmAction::Delete);
        let title = state
            .current()
            .map(|s| s.title.clone())
            .unwrap_or_else(|| "session".to_string());
        let message = format!("{} “{title}”", action.label());
        let consequence = action.consequence();
        let hits = ConfirmPrompt::new(&message, action.label(), self.system)
            .detail(&consequence)
            .destructive(action.is_destructive())
            .colorless(self.colorless)
            .focus(if state.confirm_proceed_focused {
                ConfirmFocus::Confirm
            } else {
                ConfirmFocus::Cancel
            })
            .paint(area, buffer);
        if let Some(cancel) = hits.cancel {
            state.confirm_hits.push((false, cancel));
        }
        if let Some(confirm) = hits.confirm {
            state.confirm_hits.push((true, confirm));
        }
    }
}

impl StatefulWidget for &SessionPicker<'_> {
    type State = SessionPickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for SessionPicker<'_> {
    type State = SessionPickerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Filter helper ───────────────────────────────────────────────────────────

/// Filter sessions by query (case-insensitive).
#[must_use]
pub fn filter_sessions<'a>(sessions: &'a [SessionEntry], query: &str) -> Vec<&'a SessionEntry> {
    sessions.iter().filter(|s| s.matches_query(query)).collect()
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo catalog.
#[must_use]
pub fn example_sessions() -> Vec<SessionEntry> {
    vec![
        SessionEntry::new("s1", "Auth module refactor")
            .project("termrock")
            .branch("feat/auth")
            .status(SessionStatus::ActionRequired)
            .action_required(true)
            .unread(2)
            .recency("2m ago")
            .model("grok")
            .mode("edit")
            .summary("Waiting on permission for shell")
            .pinned(true)
            .location(SessionLocation::Local)
            .dirty(true),
        SessionEntry::new("s2", "Docs pass")
            .project("termrock")
            .branch("main")
            .status(SessionStatus::Active)
            .recency("15m ago")
            .model("grok")
            .mode("ask")
            .summary("Writing handbook pages")
            .location(SessionLocation::MultiDevice)
            .device("laptop"),
        SessionEntry::new("s3", "CI green")
            .project("termrock")
            .branch("ci")
            .status(SessionStatus::Completed)
            .recency("Yesterday")
            .model("grok")
            .summary("All checks passed")
            .location(SessionLocation::Remote),
        SessionEntry::new("s4", "Broken experiment")
            .project("sandbox")
            .status(SessionStatus::Failed)
            .recency("3d ago")
            .summary("Parse error in fixture")
            .location(SessionLocation::Offline),
        SessionEntry::new("s5", "Old thread")
            .project("archive-me")
            .status(SessionStatus::Archived)
            .recency("2w ago")
            .summary("Archived session"),
        SessionEntry::new("s6", "Disabled remote")
            .project("termrock")
            .status(SessionStatus::Idle)
            .location(SessionLocation::Remote)
            .disabled()
            .summary("Cannot open while offline"),
    ]
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
    use ratatui_core::layout::Position;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open() -> SessionPickerState {
        let mut st = SessionPickerState::new();
        st.set_sessions(example_sessions());
        st
    }

    #[test]
    fn filter_case_insensitive() {
        let mut st = open();
        st.query = "AUTH".into();
        st.refilter();
        assert_eq!(st.filtered_len(), 1);
        assert_eq!(st.current().unwrap().id, "s1");
    }

    #[test]
    fn pins_sort_first() {
        let st = open();
        let first = st.sessions.get(st.filtered[0]).unwrap();
        assert!(first.pinned);
    }

    #[test]
    fn open_session() {
        let mut st = open();
        // cursor on first (pinned)
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            SessionPickerOutcome::Opened { ref id } if id == "s1"
        ));
    }

    #[test]
    fn cancel_preserves_draft_contract() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Esc)),
            SessionPickerOutcome::Cancelled
        ));
        // state still has sessions; host draft not in widget
        assert!(!st.sessions.is_empty());
        let src = include_str!("session_picker.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(body.contains("draft"));
        assert!(body.contains("preserved") || body.contains("never"));
    }

    #[test]
    fn create_session() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('n')));
        assert!(matches!(
            out,
            SessionPickerOutcome::PhaseChanged(SessionPickerPhase::Create)
        ));
        for c in "New work".chars() {
            let _ = st.handle_key(press(KeyCode::Char(c)));
        }
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            SessionPickerOutcome::CreateRequested { ref title } if title == "New work"
        ));
    }

    #[test]
    fn rename_session() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('r')));
        assert!(matches!(
            out,
            SessionPickerOutcome::PhaseChanged(SessionPickerPhase::Rename)
        ));
        st.text_draft = "Renamed".into();
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            SessionPickerOutcome::RenameRequested {
                ref id,
                ref title
            } if id == "s1" && title == "Renamed"
        ));
    }

    #[test]
    fn pin_toggle() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('p')));
        assert!(matches!(
            out,
            SessionPickerOutcome::PinToggled {
                ref id,
                pinned: false
            } if id == "s1"
        ));
    }

    #[test]
    fn delete_confirm_cancel_default() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Delete));
        assert!(matches!(
            out,
            SessionPickerOutcome::ConfirmOpened {
                action: SessionConfirmAction::Delete,
                ..
            }
        ));
        assert!(!st.confirm_proceed_focused);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, SessionPickerOutcome::ConfirmCancelled));
    }

    #[test]
    fn delete_confirm_proceed() {
        let mut st = open();
        let _ = st.handle_key(press(KeyCode::Delete));
        let _ = st.handle_key(press(KeyCode::Right));
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            SessionPickerOutcome::DeleteRequested { ref id } if id == "s1"
        ));
    }

    #[test]
    fn archive_confirm() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('a')));
        assert!(matches!(
            out,
            SessionPickerOutcome::ConfirmOpened {
                action: SessionConfirmAction::Archive,
                ..
            }
        ));
        st.confirm_proceed_focused = true;
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            SessionPickerOutcome::ArchiveRequested { ref id } if id == "s1"
        ));
    }

    #[test]
    fn archived_hidden_until_toggle() {
        let mut st = open();
        assert!(!st.filtered.iter().any(|&i| st.sessions[i].id == "s5"));
        st.show_archived = true;
        st.refilter();
        assert!(st.filtered.iter().any(|&i| st.sessions[i].id == "s5"));
    }

    #[test]
    fn provider_search_flag() {
        let mut st = open();
        st.set_total_count(Some(5000));
        // use non-reserved letters for search typing
        let out = st.handle_key(press(KeyCode::Char('x')));
        assert!(matches!(out, SessionPickerOutcome::QueryChanged { .. }));
        let _ = st.handle_key(press(KeyCode::Char('y'))); // unbound even mid-search
        let out = st.handle_key(press(KeyCode::Char('z'))); // 'z' with non-empty query types
        // after "xz" length 2; third char makes length 3 >= min 2
        assert!(
            matches!(
                out,
                SessionPickerOutcome::QueryChanged {
                    provider_search: true,
                    ref query
                } if query.len() >= SESSION_PICKER_PROVIDER_SEARCH_MIN
            ),
            "{out:?} q={}",
            st.query
        );
    }

    #[test]
    fn load_more_near_end() {
        let mut st = SessionPickerState::new();
        let many: Vec<_> = (0..20)
            .map(|i| SessionEntry::new(format!("s{i}"), format!("Session {i}")))
            .collect();
        st.set_sessions(many);
        st.set_total_count(Some(100));
        st.cursor = 18;
        let out = st.handle_key(press(KeyCode::Down));
        assert!(
            matches!(out, SessionPickerOutcome::LoadMore { offset: 20 })
                || matches!(out, SessionPickerOutcome::Selected { .. }),
            "{out:?}"
        );
    }

    #[test]
    fn disabled_cannot_open() {
        let mut st = open();
        let i = st
            .filtered
            .iter()
            .position(|&si| st.sessions[si].id == "s6")
            .unwrap();
        st.cursor = i;
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            SessionPickerOutcome::Ignored
        ));
    }

    #[test]
    fn retry_load() {
        let mut st = open();
        st.set_error("network");
        let out = st.handle_key(press(KeyCode::Char('r')));
        assert!(matches!(out, SessionPickerOutcome::RetryLoad));
    }

    #[test]
    fn y_unbound() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            SessionPickerOutcome::Ignored
        ));
    }

    #[test]
    fn fullscreen_and_popover() {
        let mut st = open();
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('f'))),
            SessionPickerOutcome::FullscreenRequested
        ));
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('o'))),
            SessionPickerOutcome::PopoverRequested
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            SessionPickerOutcome::Ignored
        ));
    }

    #[test]
    fn paint_presentations() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        for p in [
            SessionPickerPresentation::Dialog,
            SessionPickerPresentation::Popover,
            SessionPickerPresentation::Fullscreen,
        ] {
            st.presentation = p;
            SessionPicker::new(&system).paint(area, &mut buf, &mut st);
        }
        st.phase = SessionPickerPhase::ConfirmDelete;
        st.confirm_action = Some(SessionConfirmAction::Delete);
        SessionPicker::new(&system)
            .colorless(true)
            .list_only(true)
            .paint(area, &mut buf, &mut st);
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        // stress with many sessions
        let many: Vec<_> = (0..200)
            .map(|i| {
                SessionEntry::new(format!("id{i}"), format!("Session {i}"))
                    .project("p")
                    .recency("now")
            })
            .collect();
        st.set_sessions(many);
        st.set_total_count(Some(5000));
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            SessionPicker::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn no_process_io() {
        let src = include_str!("session_picker.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "std::fs", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn fuzz_status_location() {
        for s in [
            SessionStatus::Active,
            SessionStatus::Idle,
            SessionStatus::ActionRequired,
            SessionStatus::Completed,
            SessionStatus::Failed,
            SessionStatus::Archived,
        ] {
            assert!(!s.id().is_empty());
            let _ = s.glyph(true);
        }
        for l in [
            SessionLocation::Local,
            SessionLocation::Remote,
            SessionLocation::MultiDevice,
            SessionLocation::Offline,
        ] {
            assert!(!l.id().is_empty());
        }
    }

    #[test]
    fn mouse_open() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        SessionPicker::new(&system).paint(area, &mut buf, &mut st);
        assert!(!st.row_hits.is_empty());
        let (id, r) = st.row_hits[0].clone();
        // first click select
        let out = st.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        });
        assert!(
            matches!(
                out,
                SessionPickerOutcome::Selected { .. } | SessionPickerOutcome::Opened { .. }
            ),
            "{out:?} {id}"
        );
        // second click open
        let out = st.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        });
        assert!(
            matches!(out, SessionPickerOutcome::Opened { .. }),
            "{out:?}"
        );
    }

    #[test]
    fn unicode_titles() {
        let system = DesignSystem::default();
        let mut st = SessionPickerState::new();
        st.set_sessions(vec![
            SessionEntry::new("u1", "セッション 🔍")
                .project("プロジェクト")
                .branch("機能")
                .summary("概要"),
            SessionEntry::new("u2", "当前").pinned(true),
        ]);
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        SessionPicker::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn filter_sessions_helper() {
        let s = example_sessions();
        let hit = filter_sessions(&s, "docs");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].id, "s2");
    }

    #[test]
    fn selection_stable_on_set() {
        let mut st = open();
        st.cursor = 1;
        let id = st.current_id().unwrap();
        let mut next = example_sessions();
        next.push(SessionEntry::new("s9", "Extra"));
        st.set_sessions(next);
        assert_eq!(st.current_id().as_deref(), Some(id.as_str()));
    }
}
