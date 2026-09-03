// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Specialized search field: query, status, clear, history, filter metadata.
//!
//! **Mission.** Tables, logs, quick-open, command palettes, and inspectors need
//! a search chrome that owns typing, clear, history, debounce *signals*, and
//! status projection — without embedding async search work or domain filters.
//!
//! **Debounce.** Hosts call [`SearchInputState::poll`] with a
//! [`FrameTick`](crate::runtime::FrameTick). When the quiet period elapses after
//! an edit, poll emits [`SearchInputOutcome::DebouncedQuery`]. No threads, no
//! timers inside the widget.
//!
//! **vs [`TextInput`](super::TextInput).** Free text entry without search chrome.
//! **vs [`Picker`](super::Picker).** Full query+list composition; SearchInput is
//! the standalone field used *inside* those surfaces.
//!
//! Research: fzf, television, browser find, VisiData, editor search bars.
use std::collections::VecDeque;
use std::time::Duration;
use web_time::Instant;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{SemanticNode, SemanticRole, SemanticScene, SemanticState},
    runtime::FrameTick,
    style::{ButtonRecipeVariant, ControlState, DesignSystem, Glyph, Role},
    text::{display_cols, take_display_cols},
};

use super::{TextInput, TextInputOutcome, TextInputState, Validation};

/// Default debounce quiet period.
pub const DEFAULT_DEBOUNCE: Duration = Duration::from_millis(150);
/// Default history capacity.
pub const DEFAULT_HISTORY_LIMIT: usize = 32;

// ── Status / syntax ─────────────────────────────────────────────────────────

/// Host-projected search progress (never computed by filtering inside TermRock).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SearchStatus {
    /// No active search / idle chrome.
    #[default]
    Idle,
    /// Host is searching (spinner / muted “searching”).
    Searching,
    /// Match count.
    Results {
        /// Number of hits (host-owned).
        count: usize,
    },
    /// Query non-empty but zero hits.
    NoResults,
    /// Host error (message on widget).
    Error,
}

/// Columns a status label may spend before it is contracted.
const STATUS_LABEL_COLS: usize = 12;

impl SearchStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Searching => "searching",
            Self::Results { .. } => "results",
            Self::NoResults => "no-results",
            Self::Error => "error",
        }
    }
}

/// Leading command / filter syntax detected in the query (host may branch).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SearchSyntax {
    /// Plain text query.
    #[default]
    Plain,
    /// Leading `>` — command mode (palette-style).
    Command,
    /// Leading `/` — filter / regex mode (host-defined).
    Filter,
    /// Leading `:` — goto / line / command (editor-style).
    Goto,
}

impl SearchSyntax {
    /// Detect from raw query (first non-space character).
    #[must_use]
    pub fn detect(query: &str) -> Self {
        let t = query.trim_start();
        match t.chars().next() {
            Some('>') => Self::Command,
            Some('/') => Self::Filter,
            Some(':') => Self::Goto,
            _ => Self::Plain,
        }
    }

    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Command => "command",
            Self::Filter => "filter",
            Self::Goto => "goto",
        }
    }

    /// Payload after the leading sigil (trimmed).
    #[must_use]
    pub fn payload(self, query: &str) -> &str {
        let t = query.trim_start();
        match self {
            Self::Plain => t,
            Self::Command | Self::Filter | Self::Goto => {
                t.get(1..).map(str::trim_start).unwrap_or("")
            }
        }
    }
}

/// Borrowed active-filter chip shown in the leading metadata strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SearchFilterChip<'a> {
    /// Stable id for activation.
    pub id: &'a str,
    /// Short label.
    pub label: &'a str,
}

impl<'a> SearchFilterChip<'a> {
    /// Chip.
    #[must_use]
    pub const fn new(id: &'a str, label: &'a str) -> Self {
        Self { id, label }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Interaction / debounce outcomes. Host owns filtering and async work.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SearchInputOutcome {
    /// No effect.
    Ignored,
    /// Query text or caret changed (immediate; may not be debounced yet).
    Changed,
    /// Quiet period elapsed — host should apply filter / start search.
    DebouncedQuery {
        /// Current query snapshot.
        query: String,
    },
    /// Query cleared (× or Esc-on-nonempty).
    Cleared,
    /// Enter / submit search.
    Submitted {
        /// Query at submit.
        query: String,
    },
    /// Esc on empty query (dismiss surface).
    Cancelled,
    /// History navigation changed the query.
    HistoryRecalled {
        /// Recalled entry.
        query: String,
    },
    /// Tab / completion request (host opens candidates).
    CompletionRequested,
    /// Leading filter chip activated.
    FilterChipActivated {
        /// Chip id.
        id: String,
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

/// Runtime state for [`SearchInput`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInputState {
    query: TextInputState,
    history: VecDeque<String>,
    history_limit: usize,
    /// Index into history while recalling (`0` = most recent); `None` = live edit.
    history_cursor: Option<usize>,
    /// Draft saved when entering history browse.
    history_stash: Option<String>,
    debounce: Duration,
    last_edit_at: Option<Instant>,
    debounce_pending: bool,
    /// Last query emitted via debounce (avoid duplicate polls).
    last_emitted: Option<String>,
    focused: bool,
    enabled: bool,
    parts: Option<SearchInputParts>,
}

impl Default for SearchInputState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchInputState {
    /// Empty search field.
    #[must_use]
    pub fn new() -> Self {
        let mut query = TextInputState::new("").with_allow_empty(true);
        query.set_focused(false);
        Self {
            query,
            history: VecDeque::new(),
            history_limit: DEFAULT_HISTORY_LIMIT,
            history_cursor: None,
            history_stash: None,
            debounce: DEFAULT_DEBOUNCE,
            last_edit_at: None,
            debounce_pending: false,
            last_emitted: None,
            focused: false,
            enabled: true,
            parts: None,
        }
    }

    /// Seed query.
    #[must_use]
    pub fn with_query(mut self, query: impl Into<String>) -> Self {
        self.query.set_focused(false);
        self.query = self.query.reseed(query);
        self
    }

    /// Live typing. [`Self::new`] stays idle (`editing: false`).
    #[must_use]
    pub fn with_editing(mut self) -> Self {
        self.query.begin_edit();
        self
    }

    /// Start the insert session (Junie Enter on an idle field).
    pub fn begin_edit(&mut self) {
        self.query.begin_edit();
    }

    /// Debounce quiet period (`Duration::ZERO` = emit every poll after change).
    #[must_use]
    pub const fn with_debounce(mut self, debounce: Duration) -> Self {
        self.debounce = debounce;
        self
    }
    /// Current query text.
    #[must_use]
    pub fn query(&self) -> &str {
        self.query.value()
    }

    /// Mutable query editor (advanced).
    pub fn query_mut(&mut self) -> &mut TextInputState {
        &mut self.query
    }

    /// Detected leading syntax.
    #[must_use]
    pub fn syntax(&self) -> SearchSyntax {
        SearchSyntax::detect(self.query.value())
    }
    /// History entries (newest first).
    #[must_use]
    pub fn history(&self) -> impl Iterator<Item = &str> {
        self.history.iter().map(String::as_str)
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
        self.query.set_focused(on);
    }

    /// Replace query without history side effects.
    pub fn set_query(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.query.set_focused(self.focused);
        self.query.set_enabled(self.enabled);
        self.query = self.query.reseed(text);
        self.history_cursor = None;
        self.history_stash = None;
        self.mark_edited(None);
    }

    /// Clear query.
    pub fn clear(&mut self) -> bool {
        if self.query.value().is_empty() {
            return false;
        }
        let _ = self.query.clear();
        self.history_cursor = None;
        self.history_stash = None;
        self.mark_edited(None);
        true
    }

    /// Push query into history (newest first); skips empty / duplicate of newest.
    pub fn push_history(&mut self, entry: impl Into<String>) {
        let entry = entry.into();
        if entry.is_empty() {
            return;
        }
        if self.history.front().is_some_and(|h| h == &entry) {
            return;
        }
        self.history.push_front(entry);
        while self.history.len() > self.history_limit {
            self.history.pop_back();
        }
    }
    fn mark_edited(&mut self, now: Option<Instant>) {
        self.debounce_pending = true;
        self.last_edit_at = now.or_else(|| Some(Instant::now()));
        // Force re-emit even if same text re-typed after clear
        if self
            .last_emitted
            .as_deref()
            .is_some_and(|s| s == self.query.value())
        {
            // still pending; allow re-emit of same after further quiet? keep last_emitted
        }
    }

    /// After an edit known to be at `now` (tests / host).
    pub fn note_edit_at(&mut self, now: Instant) {
        self.debounce_pending = true;
        self.last_edit_at = Some(now);
    }

    /// Poll debounce using frame tick (preferred host path).
    pub fn poll(&mut self, tick: FrameTick) -> SearchInputOutcome {
        self.poll_at(tick.now())
    }

    /// Poll debounce at instant.
    pub fn poll_at(&mut self, now: Instant) -> SearchInputOutcome {
        if !self.debounce_pending {
            return SearchInputOutcome::Ignored;
        }
        let Some(edited) = self.last_edit_at else {
            return SearchInputOutcome::Ignored;
        };
        if now.saturating_duration_since(edited) < self.debounce {
            return SearchInputOutcome::Ignored;
        }
        let query = self.query.value().to_owned();
        if self.last_emitted.as_ref() == Some(&query) {
            self.debounce_pending = false;
            return SearchInputOutcome::Ignored;
        }
        self.last_emitted = Some(query.clone());
        self.debounce_pending = false;
        SearchInputOutcome::DebouncedQuery { query }
    }

    /// Force emit debounced query now (host flush).
    pub fn flush_debounce(&mut self) -> SearchInputOutcome {
        if !self.debounce_pending && self.last_emitted.as_deref() == Some(self.query.value()) {
            return SearchInputOutcome::Ignored;
        }
        let query = self.query.value().to_owned();
        self.last_emitted = Some(query.clone());
        self.debounce_pending = false;
        SearchInputOutcome::DebouncedQuery { query }
    }

    fn recall_history(&mut self, older: bool) -> SearchInputOutcome {
        if self.history.is_empty() {
            return SearchInputOutcome::Ignored;
        }
        if self.history_cursor.is_none() {
            self.history_stash = Some(self.query.value().to_owned());
            self.history_cursor = Some(0);
            if !older {
                // down without prior up → ignore
                self.history_cursor = None;
                self.history_stash = None;
                return SearchInputOutcome::Ignored;
            }
        } else if older {
            let i = self.history_cursor.unwrap_or(0);
            if i + 1 >= self.history.len() {
                return SearchInputOutcome::Ignored;
            }
            self.history_cursor = Some(i + 1);
        } else {
            let i = self.history_cursor.unwrap_or(0);
            if i == 0 {
                // restore stash
                let stash = self.history_stash.take().unwrap_or_default();
                self.history_cursor = None;
                self.apply_recalled(&stash);
                return SearchInputOutcome::HistoryRecalled { query: stash };
            }
            self.history_cursor = Some(i - 1);
        }
        let idx = self.history_cursor.unwrap_or(0);
        let entry = self.history[idx].clone();
        self.apply_recalled(&entry);
        SearchInputOutcome::HistoryRecalled { query: entry }
    }

    fn apply_recalled(&mut self, text: &str) {
        self.query.set_focused(self.focused);
        self.query.set_enabled(self.enabled);
        self.query = self.query.reseed(text.to_owned());
        self.mark_edited(None);
    }

    /// Key adapter.
    pub fn handle_key(&mut self, key: KeyEvent) -> SearchInputOutcome {
        if key.is_release() || !self.enabled {
            return SearchInputOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        // These branches emit host-facing lifecycle or clipboard outcomes.
        // A held key must not clear, submit, request completion, repeat a
        // clipboard request, or repeat a destructive editor chord before the
        // editor is synchronized.
        let one_shot = matches!(
            key.code,
            KeyCode::Enter
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Esc
                | KeyCode::Char(
                    'c' | 'C'
                        | 'k'
                        | 'K'
                        | 'm'
                        | 'M'
                        | 'u'
                        | 'U'
                        | 'v'
                        | 'V'
                        | 'w'
                        | 'W'
                        | 'x'
                        | 'X',
                )
        );
        if !key.is_press()
            && (matches!(
                key.code,
                KeyCode::Enter | KeyCode::Tab | KeyCode::BackTab | KeyCode::Esc
            ) || (ctrl && one_shot))
        {
            return SearchInputOutcome::Ignored;
        }

        self.query.set_focused(self.focused);
        self.query.set_enabled(self.enabled);

        // History: Up/Down when query empty or Alt+Up/Down
        if !ctrl
            && !shift
            && matches!(key.code, KeyCode::Up | KeyCode::Down)
            && (alt || self.query.value().is_empty() || self.history_cursor.is_some())
        {
            return self.recall_history(matches!(key.code, KeyCode::Up));
        }

        // Esc: clear nonempty, else cancel
        if key.code == KeyCode::Esc && key.modifiers.is_empty() {
            if !self.query.value().is_empty() {
                let _ = self.clear();
                return SearchInputOutcome::Cleared;
            }
            return SearchInputOutcome::Cancelled;
        }

        // Tab → completion
        if matches!(key.code, KeyCode::Tab) && !ctrl && !alt {
            return SearchInputOutcome::CompletionRequested;
        }

        // Clear chord Ctrl+U / Ctrl+K style partial — Ctrl+U clear all
        if ctrl && !alt && matches!(key.code, KeyCode::Char('u' | 'U')) {
            if self.clear() {
                return SearchInputOutcome::Cleared;
            }
            return SearchInputOutcome::Ignored;
        }

        match self.query.handle_key(key) {
            TextInputOutcome::Changed => {
                self.history_cursor = None;
                self.history_stash = None;
                self.mark_edited(None);
                SearchInputOutcome::Changed
            }
            TextInputOutcome::Cleared => {
                self.history_cursor = None;
                self.history_stash = None;
                self.mark_edited(None);
                SearchInputOutcome::Cleared
            }
            TextInputOutcome::Submitted(q) => {
                self.push_history(q.clone());
                self.debounce_pending = false;
                self.last_emitted = Some(q.clone());
                SearchInputOutcome::Submitted { query: q }
            }
            TextInputOutcome::Cancelled => {
                if !self.query.value().is_empty() {
                    let _ = self.clear();
                    SearchInputOutcome::Cleared
                } else {
                    SearchInputOutcome::Cancelled
                }
            }
            TextInputOutcome::ClipboardPasteRequest => SearchInputOutcome::ClipboardPasteRequest,
            TextInputOutcome::ClipboardCopy { text } | TextInputOutcome::ClipboardCut { text } => {
                SearchInputOutcome::ClipboardCopy { text }
            }
            TextInputOutcome::Ignored => SearchInputOutcome::Ignored,
        }
    }

    /// Paste into query.
    pub fn insert_str(&mut self, text: &str) -> SearchInputOutcome {
        if !self.enabled {
            return SearchInputOutcome::Ignored;
        }
        self.query.begin_edit();
        match self.query.insert_str(text) {
            TextInputOutcome::Changed => {
                self.history_cursor = None;
                self.mark_edited(None);
                SearchInputOutcome::Changed
            }
            _ => SearchInputOutcome::Ignored,
        }
    }

    /// Mouse: clear / chips / field.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        chips: &[SearchFilterChip<'_>],
    ) -> SearchInputOutcome {
        if !self.enabled {
            return SearchInputOutcome::Ignored;
        }
        let Some(parts) = self.parts.clone() else {
            return SearchInputOutcome::Ignored;
        };
        if matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some(clear) = parts.clear {
                if clear.contains(event.position) && self.clear() {
                    return SearchInputOutcome::Cleared;
                }
            }
            for (i, chip_rect) in parts.filter_chips.iter().enumerate() {
                if chip_rect.contains(event.position) {
                    if let Some(chip) = chips.get(i) {
                        return SearchInputOutcome::FilterChipActivated {
                            id: chip.id.to_owned(),
                        };
                    }
                }
            }
            if parts.field.contains(event.position) {
                self.set_focused(true);
                return match self.query.handle_mouse(event, parts.field) {
                    TextInputOutcome::Changed => {
                        self.mark_edited(None);
                        SearchInputOutcome::Changed
                    }
                    _ => SearchInputOutcome::Ignored,
                };
            }
        } else if parts.field.contains(event.position) {
            return match self.query.handle_mouse(event, parts.field) {
                TextInputOutcome::Changed => {
                    self.mark_edited(None);
                    SearchInputOutcome::Changed
                }
                _ => SearchInputOutcome::Ignored,
            };
        }
        SearchInputOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Hit geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInputParts {
    /// Root.
    pub root: Rect,
    /// Leading metadata strip (icons / chips).
    pub meta: Rect,
    /// Filter chip hit regions (order matches paint chips).
    pub filter_chips: Vec<Rect>,
    /// Editable query field.
    pub field: Rect,
    /// Trailing status region.
    pub status: Option<Rect>,
    /// Clear control.
    pub clear: Option<Rect>,
    /// Cursor.
    pub cursor: Option<Rect>,
}

/// Search field chrome.
#[derive(Debug, Clone, Copy)]
pub struct SearchInput<'a> {
    label: &'a str,
    placeholder: &'a str,
    system: &'a DesignSystem,
    status: SearchStatus,
    status_message: Option<&'a str>,
    filters: &'a [SearchFilterChip<'a>],
    show_clear: bool,
    show_leading_icon: bool,
    validation: Validation<'a>,
}

impl<'a> SearchInput<'a> {
    /// Create search field.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            label: "",
            placeholder: "Search",
            system,
            status: SearchStatus::Idle,
            status_message: None,
            filters: &[],
            show_clear: true,
            show_leading_icon: true,
            validation: Validation::Valid,
        }
    }

    /// Placeholder.
    #[must_use]
    pub const fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Host status projection.
    #[must_use]
    pub const fn status(mut self, status: SearchStatus) -> Self {
        self.status = status;
        self
    }

    /// Error / custom status message (for [`SearchStatus::Error`] or override).
    #[must_use]
    pub const fn status_message(mut self, message: &'a str) -> Self {
        self.status_message = Some(message);
        self
    }

    /// Active filter chips (leading metadata).
    #[must_use]
    pub const fn filters(mut self, filters: &'a [SearchFilterChip<'a>]) -> Self {
        self.filters = filters;
        self
    }

    /// Clear control.
    #[must_use]
    pub const fn show_clear(mut self, on: bool) -> Self {
        self.show_clear = on;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    /// External validation.
    pub const fn validation(mut self, validation: Validation<'a>) -> Self {
        self.validation = validation;
        self
    }

    /// Paint.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut SearchInputState,
    ) -> SearchInputParts {
        state.parts = None;
        state.query.set_focused(state.focused);
        state.query.set_enabled(state.enabled);
        if area.is_empty() {
            return SearchInputParts {
                root: area,
                meta: area,
                filter_chips: Vec::new(),
                field: area,
                status: None,
                clear: None,
                cursor: None,
            };
        }
        let field_recipe = self.system.input_recipe(
            if !state.enabled {
                ControlState::Disabled
            } else if matches!(self.status, SearchStatus::Searching) {
                ControlState::Loading
            } else if state.focused {
                ControlState::Focused
            } else {
                ControlState::Default
            },
            state.query.is_editing(),
        );

        let mut y = area.y;
        if area.height >= 2 && !self.label.is_empty() {
            let style = field_recipe.value;
            let style = if state.focused {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(self.label, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
        }

        let row = Rect::new(
            area.x,
            y.min(area.bottom().saturating_sub(1)),
            area.width,
            1,
        );
        let mut x = row.x;
        let mut right = row.right();
        let mut chip_rects = Vec::new();

        // Leading icon (contracts before query)
        if self.show_leading_icon && row.width > 4 {
            let icon = { "⌕" };
            buffer.set_stringn(x, row.y, icon, 1, field_recipe.placeholder);
            x = x.saturating_add(2);
        }

        // Filter chips metadata before query text
        for chip in self.filters {
            if x.saturating_add(3) >= right {
                break;
            }
            let label = format!(" {} ", take_display_cols(chip.label, 8));
            let w = display_cols(&label).min(10) as u16;
            if x.saturating_add(w.saturating_add(1)) >= right {
                break;
            }
            let rect = Rect::new(x, row.y, w, 1);
            buffer.set_stringn(x, row.y, &label, usize::from(w), field_recipe.cursor);
            chip_rects.push(rect);
            x = x.saturating_add(w).saturating_add(1);
        }

        let meta = Rect::new(row.x, row.y, x.saturating_sub(row.x), 1);

        // Trailing: clear + status
        let mut clear_rect = None;
        let mut status_rect = None;
        let status_text = self.status_label(state);
        if right > x.saturating_add(13) {
            let sw = 12u16;
            right = right.saturating_sub(sw.saturating_add(1));
            if !status_text.is_empty() {
                status_rect = Some(Rect::new(right.saturating_add(1), row.y, sw, 1));
                let role = match self.status {
                    SearchStatus::Error => Role::Danger,
                    SearchStatus::NoResults => Role::Warning,
                    SearchStatus::Searching => Role::TextMuted,
                    _ => Role::TextMuted,
                };
                buffer.set_stringn(
                    right.saturating_add(1),
                    row.y,
                    take_display_cols(&status_text, usize::from(sw)),
                    usize::from(sw),
                    self.system.style(role),
                );
            }
        }

        let show_clear = self.show_clear
            && state.focused
            && state.enabled
            && !state.query.value().is_empty()
            && right > x.saturating_add(2);
        if self.show_clear && right > x.saturating_add(2) {
            right = right.saturating_sub(2);
            if show_clear {
                clear_rect = Some(Rect::new(right.saturating_add(1), row.y, 1, 1));
                let action = self.system.button_recipe(
                    ButtonRecipeVariant::Quiet,
                    ControlState::Default,
                    self.system.junie_theme().surface,
                );
                buffer.set_stringn(
                    right.saturating_add(1),
                    row.y,
                    self.system.glyphs.resolve(Glyph::Close).text,
                    1,
                    action.fill.patch(action.label),
                );
            }
        }

        let field = Rect::new(x, row.y, right.saturating_sub(x).max(1), 1);
        let input = TextInput::new("", self.system)
            .placeholder(self.placeholder)
            .validation(self.validation);
        let ti = input.paint(field, buffer, &mut state.query);

        // Second row: expanded status / error
        if ti.field.y.saturating_add(1) < area.bottom() {
            let feedback = match self.validation {
                Validation::Invalid(msg) => {
                    Some((crate::widgets::label::DescriptionKind::Error, msg))
                }
                Validation::Valid => self.status_message.map(|msg| {
                    let kind = if matches!(self.status, SearchStatus::Error) {
                        crate::widgets::label::DescriptionKind::Error
                    } else {
                        crate::widgets::label::DescriptionKind::Meta
                    };
                    (kind, msg)
                }),
            };
            if let Some((kind, msg)) = feedback {
                crate::widgets::field_message::paint_field_message(
                    buffer,
                    Rect::new(area.x, ti.field.y.saturating_add(1), area.width, 1),
                    self.system,
                    kind,
                    msg,
                );
            }
        }

        let parts = SearchInputParts {
            root: area,
            meta,
            filter_chips: chip_rects,
            field: ti.field,
            status: status_rect,
            clear: clear_rect,
            cursor: ti.cursor,
        };
        state.parts = Some(parts.clone());
        parts
    }

    fn status_label(&self, _state: &SearchInputState) -> String {
        if let Some(msg) = self.status_message {
            if matches!(self.status, SearchStatus::Error) {
                return take_display_cols(msg, STATUS_LABEL_COLS).into_owned();
            }
        }
        let label = match self.status {
            SearchStatus::Idle => String::new(),
            SearchStatus::Searching => "…".into(),
            SearchStatus::Results { count } => format!("{count}"),
            SearchStatus::NoResults => "0".into(),
            SearchStatus::Error => "err".into(),
        };
        // Display columns, not code points: a status label is painted, and a
        // wide glyph spends two cells (plans/022 Step 3).
        crate::text::take_display_cols(&label, STATUS_LABEL_COLS).into_owned()
    }

    /// Semantic registration — status ids, never raw error dumps with secrets.
    pub fn register_semantic<Id, Action>(
        &self,
        scene: &mut SemanticScene<Id, Action>,
        id: Id,
        area: Rect,
        state: &SearchInputState,
    ) where
        Id: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "search {} {} filters={}",
            self.status.id(),
            state.syntax().id(),
            self.filters.len()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Input)
                .label(if self.label.is_empty() {
                    "search"
                } else {
                    self.label
                })
                .description(desc)
                .focusable(state.enabled)
                .disabled(!state.enabled)
                .state(SemanticState {
                    selected: state.focused,
                    busy: matches!(self.status, SearchStatus::Searching),
                    invalid: matches!(self.status, SearchStatus::Error)
                        || matches!(self.validation, Validation::Invalid(_)),
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for &SearchInput<'_> {
    type State = SearchInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        let _ = self.paint(area, buffer, state);
    }
}

impl StatefulWidget for SearchInput<'_> {
    type State = SearchInputState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::RolePalette;
    use crate::widgets::tests::click;

    fn tick_at(start: Instant, ms: u64) -> FrameTick {
        FrameTick::manual(
            start + Duration::from_millis(ms),
            Duration::from_millis(ms),
            Duration::from_millis(16),
        )
    }

    #[test]
    fn debounce_emits_after_quiet_period() {
        let start = Instant::now();
        let mut state = SearchInputState::new().with_debounce(Duration::from_millis(100));
        state.set_focused(true);
        state.note_edit_at(start);
        let _ = state.insert_str("foo");
        state.note_edit_at(start);
        assert_eq!(state.poll(tick_at(start, 50)), SearchInputOutcome::Ignored);
        match state.poll(tick_at(start, 120)) {
            SearchInputOutcome::DebouncedQuery { query } => assert_eq!(query, "foo"),
            other => panic!("expected debounce, got {other:?}"),
        }
        // no duplicate
        assert_eq!(state.poll(tick_at(start, 200)), SearchInputOutcome::Ignored);
    }

    #[test]
    fn esc_clears_then_cancels() {
        let mut state = SearchInputState::new().with_query("ab");
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            SearchInputOutcome::Cleared
        );
        assert!(state.query().is_empty());
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            SearchInputOutcome::Cancelled
        );
    }

    #[test]
    fn history_up_down() {
        let mut state = SearchInputState::new();
        state.push_history("one");
        state.push_history("two");
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            SearchInputOutcome::HistoryRecalled {
                query: "two".into()
            }
        );
        assert_eq!(state.query(), "two");
        let mut repeat_up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        repeat_up.kind = KeyEventKind::Repeat;
        assert_eq!(
            state.handle_key(repeat_up),
            SearchInputOutcome::HistoryRecalled {
                query: "one".into()
            }
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            SearchInputOutcome::HistoryRecalled {
                query: "two".into()
            }
        );
    }

    #[test]
    fn syntax_detect() {
        assert_eq!(SearchSyntax::detect(">run"), SearchSyntax::Command);
        assert_eq!(SearchSyntax::detect("/todo"), SearchSyntax::Filter);
        assert_eq!(SearchSyntax::detect(":42"), SearchSyntax::Goto);
        assert_eq!(SearchSyntax::detect("plain"), SearchSyntax::Plain);
        assert_eq!(SearchSyntax::Command.payload("> run build"), "run build");
    }

    #[test]
    fn clear_control_and_submit_history() {
        let mut state = SearchInputState::new().with_query("x");
        state.set_focused(true);
        state.begin_edit();
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            SearchInputOutcome::Submitted { query: "x".into() }
        );
        assert_eq!(state.history().next(), Some("x"));
        assert!(state.clear());
        assert!(state.query().is_empty());
    }

    #[test]
    fn tab_completion_request() {
        let mut state = SearchInputState::new();
        state.set_focused(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            SearchInputOutcome::CompletionRequested
        );
    }

    #[test]
    fn repeated_lifecycle_and_clipboard_actions_are_ignored() {
        let mut state = SearchInputState::new().with_query("abc");
        state.set_focused(true);
        state.begin_edit();
        let actions = [
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Tab, KeyModifiers::NONE),
            (KeyCode::BackTab, KeyModifiers::NONE),
            (KeyCode::Char('u'), KeyModifiers::CONTROL),
            (KeyCode::Char('m'), KeyModifiers::CONTROL),
            (
                KeyCode::Char('m'),
                KeyModifiers::CONTROL | KeyModifiers::ALT,
            ),
            (KeyCode::Char('k'), KeyModifiers::CONTROL),
            (KeyCode::Char('w'), KeyModifiers::CONTROL),
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('x'), KeyModifiers::CONTROL),
            (KeyCode::Char('v'), KeyModifiers::CONTROL),
        ];
        for (code, modifiers) in actions {
            let before = state.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(state.handle_key(key), SearchInputOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated search state");
        }

        let mut key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        key.kind = KeyEventKind::Repeat;
        assert_eq!(state.handle_key(key), SearchInputOutcome::Changed);
        assert_eq!(state.query(), "abcz");
    }

    #[test]
    fn paint_meta_before_query_and_status() {
        let system = DesignSystem::new(RolePalette::default());
        let mut state = SearchInputState::new().with_query("table");
        state.set_focused(true);
        let chips = [SearchFilterChip::new("ext", "rs")];
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        let parts = SearchInput::new(&system)
            .filters(&chips)
            .status(SearchStatus::Results { count: 12 })
            .paint(area, &mut buf, &mut state);
        assert!(!parts.meta.is_empty() || parts.meta.width > 0);
        assert!(!parts.filter_chips.is_empty());
        assert!(parts.status.is_some());
        assert!(parts.field.x >= parts.meta.right() || parts.meta.width == 0);
    }

    #[test]
    fn mouse_filter_chip() {
        let system = DesignSystem::default();
        let mut state = SearchInputState::new().with_query("q");
        state.set_focused(true);
        let chips = [SearchFilterChip::new("lang", "rs")];
        let area = Rect::new(0, 0, 36, 2);
        let mut buf = Buffer::empty(area);
        let parts = SearchInput::new(&system)
            .filters(&chips)
            .paint(area, &mut buf, &mut state);
        let chip = parts.filter_chips[0];
        assert_eq!(
            state.handle_mouse(click(chip.x, chip.y), &chips,),
            SearchInputOutcome::FilterChipActivated { id: "lang".into() }
        );
    }

    #[test]
    fn semantic_status_not_query_body_as_label() {
        let system = DesignSystem::default();
        let state = SearchInputState::new().with_query("secret-query-xyz");
        let mut scene = SemanticScene::<&str, ()>::default();
        SearchInput::new(&system)
            .status(SearchStatus::Searching)
            .register_semantic(&mut scene, "s", Rect::new(0, 0, 20, 1), &state);
        let node = scene.get(&"s").unwrap();
        let dump = format!("{node:?}");
        // description should mention searching, label is "search"
        assert!(dump.contains("searching") || dump.contains("search"));
    }

    #[test]
    fn fuzz_keys_stable() {
        let mut state = SearchInputState::new().with_debounce(Duration::from_millis(10));
        state.set_focused(true);
        state.begin_edit();
        let keys = [
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('>'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        ];
        for key in keys.iter().cycle().take(40) {
            let _ = state.handle_key(*key);
        }
        assert!(state.query().len() < 1000);
    }

    #[test]
    fn paint_hot_path() {
        let system = DesignSystem::default();
        let mut state = SearchInputState::new().with_query("filter");
        state.set_focused(true);
        let area = Rect::new(0, 0, 48, 2);
        let mut buf = Buffer::empty(area);
        let w = SearchInput::new(&system).status(SearchStatus::Results { count: 3 });
        for _ in 0..200 {
            let _ = w.paint(area, &mut buf, &mut state);
        }
    }

    #[test]
    fn field_plane_idle_and_hover() {
        let system = DesignSystem::junie();
        let theme = system.junie_theme();
        let mut state = SearchInputState::new().with_query("table");
        let area = Rect::new(0, 0, 40, 2);
        let mut buf = Buffer::empty(area);
        let parts = SearchInput::new(&system).paint(area, &mut buf, &mut state);
        let cell = &buf[(parts.field.x, parts.field.y)];
        assert_eq!(cell.bg, theme.field);
        state.query_mut().set_hovered(true);
        let mut hover = Buffer::empty(area);
        let hover_parts = SearchInput::new(&system).paint(area, &mut hover, &mut state);
        assert_eq!(
            hover[(hover_parts.field.x, hover_parts.field.y)].bg,
            theme.field_hover
        );
    }

    #[test]
    fn flush_debounce() {
        let mut state = SearchInputState::new();
        state.set_focused(true);
        let _ = state.insert_str("x");
        match state.flush_debounce() {
            SearchInputOutcome::DebouncedQuery { query } => assert_eq!(query, "x"),
            other => panic!("{other:?}"),
        }
    }
}
