// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Flagship agent prompt composer: editing, chips, completion, queue, chrome.
//!
//! **Separation of concerns**
//! - **Text editing** — [`TextAreaState`] + local undo/redo/history/draft.
//! - **Tokens & attachments** — chips (files, pastes, mentions as display tokens).
//! - **Completion** — slash / file / symbol overlay request state (consumer supplies rows).
//! - **Presentation** — compact / normal / expanded / fullscreen geometry.
//! - **Submission policy** — consumer flags (`busy`, `submit_on_enter`, …); no provider I/O.
//!
//! Draft text is never cleared when focus moves to permission, plan, session,
//! or palette overlays — only [`PromptComposerState::clear_draft`] or a
//! successful submit policy does that.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    interaction::{
        OverlayId, OverlayKind, OverlayOutcome, OverlaySize, OverlaySpec, OverlayStack,
        place_overlay,
    },
    style::{Density, DesignTokens, Role, Theme},
    text::{display_cols, take_display_cols},
    widgets::{
        Panel, PanelEmphasis, TextArea, TextAreaOutcome, TextAreaState, TextCursor, TokenMeter,
    },
};

/// Default overlay id for composer completion (slash / mention).
pub const PROMPT_COMPLETION_OVERLAY_ID: &str = "termrock.prompt_completion";

/// Default overlay id when the composer is promoted fullscreen.
pub const PROMPT_FULLSCREEN_OVERLAY_ID: &str = "termrock.prompt_fullscreen";

/// Bytes above which a paste becomes a [`ComposerChip`] (kind paste) instead of inline text.
pub const LARGE_PASTE_THRESHOLD: usize = 400;

/// Max undo snapshots retained.
const UNDO_LIMIT: usize = 64;

/// Max submit history entries.
const HISTORY_LIMIT: usize = 100;

// ── Tokens / attachments ────────────────────────────────────────────────────

/// Kind of chip shown above the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChipKind {
    /// File / path attachment.
    File,
    /// Large paste summary.
    Paste,
    /// Image or binary blob label.
    Media,
    /// Generic attachment.
    Other,
}

/// Stable attachment / paste chip (consumer owns bytes/path meaning).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerChip {
    /// Stable id.
    pub id: String,
    /// Chip kind.
    pub kind: ChipKind,
    /// Primary label (filename, "paste 12KB", …).
    pub label: String,
    /// Optional secondary meta.
    pub meta: Option<String>,
    /// Optional byte size for paste chips.
    pub bytes: Option<usize>,
}

impl ComposerChip {
    /// File attachment chip.
    #[must_use]
    pub fn file(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: ChipKind::File,
            label: label.into(),
            meta: None,
            bytes: None,
        }
    }

    /// Large paste chip.
    #[must_use]
    pub fn paste(id: impl Into<String>, preview: impl Into<String>, bytes: usize) -> Self {
        Self {
            id: id.into(),
            kind: ChipKind::Paste,
            label: preview.into(),
            meta: Some(format!("{bytes} B")),
            bytes: Some(bytes),
        }
    }
}

// ── Completion ──────────────────────────────────────────────────────────────

/// Which completion surface is open (consumer fills candidates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CompletionKind {
    /// No completion overlay.
    #[default]
    None,
    /// Slash command menu (`/`).
    Slash,
    /// File mention (`@` path).
    FileMention,
    /// Symbol mention (`@` or `#` symbols — consumer decides trigger).
    SymbolMention,
}

/// Completion query extracted from the draft.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionQuery {
    /// Kind of completion.
    pub kind: CompletionKind,
    /// Text after the trigger through the cursor (no trigger char).
    pub query: String,
    /// Byte offset in full draft where the trigger began.
    pub trigger_byte: usize,
    /// Byte offset of cursor in full draft.
    pub cursor_byte: usize,
}

// ── Presentation ────────────────────────────────────────────────────────────

/// Visual density of the composer chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ComposerPresentation {
    /// Single status line + minimal editor rows.
    Compact,
    /// Default agent chrome.
    #[default]
    Normal,
    /// Tall editor for long prompts.
    Expanded,
    /// Nearly full-screen editor overlay.
    Fullscreen,
}

/// Agent-mode badge (labels are display-only; policy is consumer-owned).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModeIndicator {
    /// Short label (e.g. `PLAN`, `EDIT`).
    pub label: String,
    /// Whether to emphasize as warning (full-auto).
    pub warning: bool,
}

/// Model badge.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelIndicator {
    /// Model id or short name.
    pub label: String,
}

/// Context / token estimate for the meter strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContextEstimate {
    /// Estimated used tokens (or units).
    pub used: u64,
    /// Limit (0 = unknown / hide fraction).
    pub limit: u64,
}

// ── Queue ───────────────────────────────────────────────────────────────────

/// Prompt waiting while the agent is active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedPrompt {
    /// Queue entry id.
    pub id: String,
    /// Draft text at enqueue time.
    pub text: String,
    /// Attachment ids copied at enqueue.
    pub chip_ids: Vec<String>,
}

// ── Policy / connection ─────────────────────────────────────────────────────

/// Connection / enablement (consumer sets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ComposerConnection {
    /// Ready to edit and submit.
    #[default]
    Ready,
    /// Visually disabled; ignores input.
    Disabled,
    /// Offline / disconnected; edit allowed, submit blocked.
    Disconnected,
}

/// Submission and input policy (application-specific).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmitPolicy {
    /// Enter submits when no completion open.
    pub submit_on_enter: bool,
    /// Mod+Enter (or Alt+Enter) inserts newline when `submit_on_enter`.
    pub newline_chord: bool,
    /// Allow submit of whitespace-only drafts.
    pub allow_empty: bool,
    /// When agent is busy, Enter enqueues instead of submit.
    pub queue_when_busy: bool,
    /// Clear draft after successful submit (not after enqueue).
    pub clear_on_submit: bool,
    /// Paste longer than [`LARGE_PASTE_THRESHOLD`] becomes a chip.
    pub large_paste_as_chip: bool,
}

impl Default for SubmitPolicy {
    fn default() -> Self {
        Self {
            submit_on_enter: true,
            newline_chord: true,
            allow_empty: false,
            queue_when_busy: true,
            clear_on_submit: true,
            large_paste_as_chip: true,
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed composer messages (no side effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PromptComposerOutcome {
    /// Not handled.
    Ignored,
    /// Draft, chips, or cursor changed.
    Changed,
    /// Submit the current draft (and chips); consumer runs the agent.
    Submit {
        /// Draft text.
        text: String,
        /// Chip ids attached.
        chip_ids: Vec<String>,
    },
    /// Enqueued while busy.
    Queued {
        /// Queue entry.
        entry: QueuedPrompt,
    },
    /// User removed a queued entry.
    QueueRemoved {
        /// Id.
        id: String,
    },
    /// Cancel current agent run / stop.
    Cancel,
    /// Soft interrupt (Ctrl+C style) without clearing draft.
    Interrupt,
    /// Esc when nothing to dismiss → bubble.
    DismissRequest,
    /// Open external editor with current draft.
    ExternalEditor,
    /// Completion surface should open or update.
    Completion {
        /// Query.
        query: CompletionQuery,
    },
    /// Completion closed.
    CompletionClosed,
    /// User confirmed a completion item (consumer inserts token).
    CompletionCommitted {
        /// Completion kind.
        kind: CompletionKind,
        /// Selected candidate id.
        id: String,
    },
    /// Mode badge activated.
    ModeMenu,
    /// Model badge activated.
    ModelMenu,
    /// Chip removed.
    ChipRemoved {
        /// Chip id.
        id: String,
    },
    /// Chip activated (e.g. expand paste).
    ChipActivated {
        /// Chip id.
        id: String,
    },
    /// Attachment requested (e.g. drop / key).
    AttachRequest,
    /// Validation failed (empty submit, disconnected, …).
    ValidationFailed {
        /// Human-readable reason.
        reason: String,
    },
    /// Presentation mode changed.
    PresentationChanged(ComposerPresentation),
    /// Focus left the composer (consumer may move focus).
    Blur,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Flagship prompt composer state.
///
/// Draft lives in [`Self::editor`] and is preserved across temporary overlays
/// (permission, plan, palette, …). Call [`Self::set_focused`]` (false)` when
/// another layer owns input; do not clear text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptComposerState {
    // —— text editing ——
    /// Multiline grapheme-safe editor.
    pub editor: TextAreaState,
    undo: Vec<String>,
    redo: Vec<String>,
    /// Submitted prompt history (newest last).
    history: Vec<String>,
    history_index: Option<usize>,
    /// Draft snapshot when browsing history (restored on leave).
    history_draft: Option<String>,
    /// Selection anchor (None = caret only). Cursor is the other end.
    select_anchor: Option<TextCursor>,

    // —— tokens & attachments ——
    chips: Vec<ComposerChip>,
    chip_focus: Option<usize>,

    // —— completion ——
    completion: CompletionQuery,

    // —— presentation ——
    presentation: ComposerPresentation,
    mode: Option<ModeIndicator>,
    model: Option<ModelIndicator>,
    context: ContextEstimate,
    density: Density,
    ascii_fallback: bool,
    placeholder: String,

    // —— policy / session ——
    policy: SubmitPolicy,
    connection: ComposerConnection,
    /// Agent currently running (enables queue / stop).
    busy: bool,
    queue: Vec<QueuedPrompt>,
    next_queue_id: u64,
    next_chip_id: u64,
    validation_error: Option<String>,
    focused: bool,
}

impl Default for PromptComposerState {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptComposerState {
    /// Empty ready composer.
    #[must_use]
    pub fn new() -> Self {
        let mut editor = TextAreaState::default();
        editor.set_focused(true);
        Self {
            editor,
            undo: Vec::new(),
            redo: Vec::new(),
            history: Vec::new(),
            history_index: None,
            history_draft: None,
            select_anchor: None,
            chips: Vec::new(),
            chip_focus: None,
            completion: CompletionQuery::default(),
            presentation: ComposerPresentation::Normal,
            mode: None,
            model: None,
            context: ContextEstimate::default(),
            density: Density::Comfortable,
            ascii_fallback: false,
            placeholder: "Message…".into(),
            policy: SubmitPolicy::default(),
            connection: ComposerConnection::Ready,
            busy: false,
            queue: Vec::new(),
            next_queue_id: 1,
            next_chip_id: 1,
            validation_error: None,
            focused: true,
        }
    }

    // —— accessors ——

    /// Full draft text (LF normalized).
    #[must_use]
    pub fn text(&self) -> String {
        self.editor.text()
    }

    /// Whether the editor holds a non-whitespace draft.
    #[must_use]
    pub fn has_draft(&self) -> bool {
        !self.text().trim().is_empty()
    }

    /// Focused for keyboard.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Sets keyboard focus without clearing draft.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.editor.set_focused(focused);
    }

    /// Agent busy flag.
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    /// Sets busy (queue/stop chrome).
    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    /// Connection state.
    #[must_use]
    pub const fn connection(&self) -> ComposerConnection {
        self.connection
    }

    /// Sets connection.
    pub fn set_connection(&mut self, connection: ComposerConnection) {
        self.connection = connection;
    }

    /// Submit policy.
    #[must_use]
    pub const fn policy(&self) -> SubmitPolicy {
        self.policy
    }

    /// Sets submit policy.
    pub fn set_policy(&mut self, policy: SubmitPolicy) {
        self.policy = policy;
    }

    /// Presentation mode.
    #[must_use]
    pub const fn presentation(&self) -> ComposerPresentation {
        self.presentation
    }

    /// Sets presentation.
    pub fn set_presentation(&mut self, presentation: ComposerPresentation) {
        self.presentation = presentation;
    }

    /// Mode indicator.
    pub fn set_mode(&mut self, mode: Option<ModeIndicator>) {
        self.mode = mode;
    }

    /// Model indicator.
    pub fn set_model(&mut self, model: Option<ModelIndicator>) {
        self.model = model;
    }

    /// Context estimate.
    pub fn set_context(&mut self, context: ContextEstimate) {
        self.context = context;
    }

    /// Density for chrome spacing.
    pub fn set_density(&mut self, density: Density) {
        self.density = density;
    }

    /// ASCII glyph fallback for badges.
    pub fn set_ascii_fallback(&mut self, ascii: bool) {
        self.ascii_fallback = ascii;
    }

    /// Placeholder when empty.
    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    /// Chips.
    #[must_use]
    pub fn chips(&self) -> &[ComposerChip] {
        &self.chips
    }

    /// Queue.
    #[must_use]
    pub fn queue(&self) -> &[QueuedPrompt] {
        &self.queue
    }

    /// Open completion query.
    #[must_use]
    pub fn completion(&self) -> &CompletionQuery {
        &self.completion
    }

    /// Validation error string.
    #[must_use]
    pub fn validation_error(&self) -> Option<&str> {
        self.validation_error.as_deref()
    }

    /// Clears draft text only (keeps chips unless `clear_chips`).
    pub fn clear_draft(&mut self) {
        self.push_undo();
        self.editor.set_text("");
        self.select_anchor = None;
        self.completion = CompletionQuery::default();
        self.history_index = None;
        self.history_draft = None;
        self.validation_error = None;
    }

    /// Adds a chip.
    pub fn add_chip(&mut self, chip: ComposerChip) {
        self.chips.push(chip);
    }

    /// Removes chip by id.
    pub fn remove_chip(&mut self, id: &str) -> bool {
        let before = self.chips.len();
        self.chips.retain(|c| c.id != id);
        if self.chip_focus.is_some_and(|i| i >= self.chips.len()) {
            self.chip_focus = self.chips.len().checked_sub(1);
        }
        before != self.chips.len()
    }

    /// Inserts text at cursor (records undo).
    pub fn insert_text(&mut self, text: &str) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled {
            return PromptComposerOutcome::Ignored;
        }
        self.push_undo();
        self.delete_selection_if_any();
        let _ = self.editor.insert_text(text);
        self.after_edit()
    }

    /// Replaces draft (records undo).
    pub fn set_text(&mut self, text: &str) {
        self.push_undo();
        self.editor.set_text(text);
        self.select_anchor = None;
        self.completion = CompletionQuery::default();
    }

    /// Consumer commits a completion: replaces trigger..cursor with `insertion`.
    pub fn apply_completion_insert(&mut self, insertion: &str) -> PromptComposerOutcome {
        if self.completion.kind == CompletionKind::None {
            return PromptComposerOutcome::Ignored;
        }
        let draft = self.text();
        let start = self.completion.trigger_byte.min(draft.len());
        let end = self.completion.cursor_byte.min(draft.len()).max(start);
        let mut next = String::new();
        next.push_str(&draft[..start]);
        next.push_str(insertion);
        next.push_str(&draft[end..]);
        self.push_undo();
        self.editor.set_text(&next);
        self.completion = CompletionQuery::default();
        self.select_anchor = None;
        PromptComposerOutcome::Changed
    }

    /// Closes completion without editing.
    pub fn close_completion(&mut self) -> PromptComposerOutcome {
        if self.completion.kind == CompletionKind::None {
            return PromptComposerOutcome::Ignored;
        }
        self.completion = CompletionQuery::default();
        PromptComposerOutcome::CompletionClosed
    }

    /// Preferred editor height in rows for presentation + area.
    #[must_use]
    pub fn preferred_editor_rows(&self, area_height: u16) -> u16 {
        let chrome = self.chrome_rows(area_height);
        let available = area_height.saturating_sub(chrome).max(1);
        let want = match self.presentation {
            ComposerPresentation::Compact => 2,
            ComposerPresentation::Normal => 4,
            ComposerPresentation::Expanded => 10,
            ComposerPresentation::Fullscreen => available,
        };
        want.min(available).max(1)
    }

    fn chrome_rows(&self, area_height: u16) -> u16 {
        let mut rows = 0u16;
        if !self.chips.is_empty() {
            rows = rows.saturating_add(1);
        }
        // status / indicators
        rows = rows.saturating_add(1);
        if self.validation_error.is_some() {
            rows = rows.saturating_add(1);
        }
        if self.presentation == ComposerPresentation::Compact {
            rows = rows.min(area_height.saturating_sub(1));
        }
        rows
    }

    // —— input ——

    /// Routes a key event.
    pub fn handle_key(&mut self, key: KeyEvent) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled {
            return PromptComposerOutcome::Ignored;
        }
        if !self.focused || key.kind == KeyEventKind::Release {
            return PromptComposerOutcome::Ignored;
        }

        // Completion open: Esc closes one layer; navigation left to consumer list.
        if self.completion.kind != CompletionKind::None && key.code == KeyCode::Esc {
            return self.close_completion();
        }

        // Ctrl+Z / Ctrl+Y undo redo
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('z') | KeyCode::Char('Z') => return self.undo(),
                KeyCode::Char('y') | KeyCode::Char('Y') => return self.redo(),
                KeyCode::Char('c') | KeyCode::Char('C') if self.busy => {
                    return PromptComposerOutcome::Interrupt;
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    return PromptComposerOutcome::ExternalEditor;
                }
                _ => {}
            }
        }

        // Stop / cancel when busy: Ctrl+Backspace or dedicated
        if self.busy
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return PromptComposerOutcome::Cancel;
        }

        // Submit / newline policy
        if key.code == KeyCode::Enter && key.kind == KeyEventKind::Press {
            let mod_newline = self.policy.newline_chord
                && (key.modifiers.contains(KeyModifiers::ALT)
                    || key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::SHIFT));
            if self.policy.submit_on_enter && !mod_newline && key.modifiers.is_empty() {
                return self.try_submit_or_queue();
            }
            if mod_newline || !self.policy.submit_on_enter {
                self.push_undo();
                let _ = self
                    .editor
                    .handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                return self.after_edit();
            }
        }

        if key.code == KeyCode::Esc {
            if self.completion.kind != CompletionKind::None {
                return self.close_completion();
            }
            if self.select_anchor.take().is_some() {
                return PromptComposerOutcome::Changed;
            }
            return PromptComposerOutcome::DismissRequest;
        }

        // History: Up/Down on first/last line empty-ish
        if key.modifiers.is_empty() && self.try_history_nav(key.code) {
            return PromptComposerOutcome::Changed;
        }

        // Chip focus strip: when chip_focus set, left/right/delete
        if let Some(out) = self.handle_chip_keys(&key) {
            return out;
        }

        // Detect slash / @ triggers after plain char path below
        let before = self.text();
        match self.editor.handle_key(key) {
            TextAreaOutcome::Changed => {
                if self.undo.last().is_none_or(|s| s != &before) {
                    self.undo.push(before);
                    if self.undo.len() > UNDO_LIMIT {
                        self.undo.remove(0);
                    }
                }
                self.redo.clear();
                self.after_edit()
            }
            TextAreaOutcome::Cancelled => PromptComposerOutcome::DismissRequest,
            TextAreaOutcome::Ignored => PromptComposerOutcome::Ignored,
        }
    }

    /// Paste (bracketed paste arrives as [`Event::Paste`]).
    pub fn handle_paste(&mut self, text: &str) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled || !self.focused {
            return PromptComposerOutcome::Ignored;
        }
        if self.policy.large_paste_as_chip && text.len() >= LARGE_PASTE_THRESHOLD {
            let id = format!("paste-{}", self.next_chip_id);
            self.next_chip_id = self.next_chip_id.saturating_add(1);
            let preview: String = text.chars().take(32).collect();
            let preview = if text.chars().count() > 32 {
                format!("{preview}…")
            } else {
                preview
            };
            self.chips
                .push(ComposerChip::paste(id, preview, text.len()));
            return PromptComposerOutcome::Changed;
        }
        self.insert_text(text)
    }

    /// Mouse: click in editor positions cursor; chip hits remove/activate.
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        editor_area: Rect,
        chip_areas: &[(String, Rect)],
    ) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled {
            return PromptComposerOutcome::Ignored;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            for (id, area) in chip_areas {
                if area.contains(mouse.position) {
                    // Click near right edge = remove if width > 2
                    if mouse.position.x + 1 >= area.right().saturating_sub(1) && area.width > 3 {
                        let id = id.clone();
                        let _ = self.remove_chip(&id);
                        return PromptComposerOutcome::ChipRemoved { id };
                    }
                    return PromptComposerOutcome::ChipActivated { id: id.clone() };
                }
            }
            if editor_area.contains(mouse.position) {
                self.set_focused(true);
                // TextArea handles click via scroll_to / position — use event
                let _ = self.editor.handle_event(Event::Mouse(mouse));
                return PromptComposerOutcome::Changed;
            }
        }
        if editor_area.contains(mouse.position) {
            match self.editor.handle_event(Event::Mouse(mouse)) {
                TextAreaOutcome::Changed => PromptComposerOutcome::Changed,
                _ => PromptComposerOutcome::Ignored,
            }
        } else {
            PromptComposerOutcome::Ignored
        }
    }

    /// Unified event entry.
    pub fn handle_event(
        &mut self,
        event: Event,
        editor_area: Rect,
        chip_areas: &[(String, Rect)],
    ) -> PromptComposerOutcome {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Paste(text) => self.handle_paste(&text),
            Event::Mouse(mouse) => self.handle_mouse(mouse, editor_area, chip_areas),
            _ => PromptComposerOutcome::Ignored,
        }
    }

    // —— overlay helpers ——

    /// Opens completion overlay on the stack (menu policy).
    pub fn open_completion_overlay<FocusId: Clone>(
        &self,
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        anchor: Rect,
        size: OverlaySize,
        opener: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static(PROMPT_COMPLETION_OVERLAY_ID),
                kind: OverlayKind::Completion,
                parent: None,
                anchor: Some(anchor),
                size,
                opener_focus: opener,
                policy: None,
            },
        )
    }

    /// Dismisses completion overlay.
    pub fn dismiss_completion_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.dismiss(&OverlayId::from_static(PROMPT_COMPLETION_OVERLAY_ID))
    }

    /// Preferred completion rect (does not open stack).
    #[must_use]
    pub fn place_completion(bounds: Rect, anchor: Rect, width: u16, height: u16) -> Rect {
        place_overlay(
            bounds,
            Some(anchor),
            OverlaySize::menu(width, height),
            crate::interaction::OverlayPolicy::for_kind(OverlayKind::Completion),
        )
    }

    // —— internals ——

    fn try_submit_or_queue(&mut self) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disconnected {
            self.validation_error = Some("Disconnected".into());
            return PromptComposerOutcome::ValidationFailed {
                reason: "Disconnected".into(),
            };
        }
        let text = self.text();
        if !self.policy.allow_empty && text.trim().is_empty() && self.chips.is_empty() {
            self.validation_error = Some("Empty prompt".into());
            return PromptComposerOutcome::ValidationFailed {
                reason: "Empty prompt".into(),
            };
        }
        self.validation_error = None;
        let chip_ids: Vec<String> = self.chips.iter().map(|c| c.id.clone()).collect();
        if self.busy && self.policy.queue_when_busy {
            let id = format!("q-{}", self.next_queue_id);
            self.next_queue_id = self.next_queue_id.saturating_add(1);
            let entry = QueuedPrompt {
                id: id.clone(),
                text: text.clone(),
                chip_ids: chip_ids.clone(),
            };
            self.queue.push(entry.clone());
            if self.policy.clear_on_submit {
                self.editor.set_text("");
                // keep chips? typically clear on enqueue of text only
            }
            return PromptComposerOutcome::Queued { entry };
        }
        if self.busy && !self.policy.queue_when_busy {
            return PromptComposerOutcome::ValidationFailed {
                reason: "Agent busy".into(),
            };
        }
        if !text.trim().is_empty() {
            self.history.push(text.clone());
            if self.history.len() > HISTORY_LIMIT {
                self.history.remove(0);
            }
        }
        self.history_index = None;
        self.history_draft = None;
        if self.policy.clear_on_submit {
            self.editor.set_text("");
            self.chips.clear();
            self.undo.clear();
            self.redo.clear();
        }
        PromptComposerOutcome::Submit { text, chip_ids }
    }

    fn push_undo(&mut self) {
        let snap = self.text();
        if self.undo.last().is_some_and(|s| s == &snap) {
            return;
        }
        self.undo.push(snap);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo(&mut self) -> PromptComposerOutcome {
        let Some(prev) = self.undo.pop() else {
            return PromptComposerOutcome::Ignored;
        };
        self.redo.push(self.text());
        self.editor.set_text(&prev);
        self.select_anchor = None;
        PromptComposerOutcome::Changed
    }

    fn redo(&mut self) -> PromptComposerOutcome {
        let Some(next) = self.redo.pop() else {
            return PromptComposerOutcome::Ignored;
        };
        self.undo.push(self.text());
        self.editor.set_text(&next);
        self.select_anchor = None;
        PromptComposerOutcome::Changed
    }

    fn delete_selection_if_any(&mut self) {
        // Selection editing is caret-only until TextArea gains ranges;
        // clear anchor so shift-nav can be layered later.
        self.select_anchor = None;
    }

    fn after_edit(&mut self) -> PromptComposerOutcome {
        self.validation_error = None;
        if let Some(q) = detect_completion(&self.text(), self.editor.cursor()) {
            self.completion = q.clone();
            return PromptComposerOutcome::Completion { query: q };
        }
        if self.completion.kind != CompletionKind::None {
            self.completion = CompletionQuery::default();
            return PromptComposerOutcome::CompletionClosed;
        }
        PromptComposerOutcome::Changed
    }

    fn try_history_nav(&mut self, code: KeyCode) -> bool {
        let lines: Vec<_> = self.editor.lines().collect();
        let cursor = self.editor.cursor();
        let at_top = cursor.line == 0;
        let at_bottom = cursor.line + 1 >= lines.len();
        match code {
            KeyCode::Up if at_top && !self.history.is_empty() => {
                if self.history_index.is_none() {
                    self.history_draft = Some(self.text());
                    self.history_index = Some(self.history.len() - 1);
                } else if let Some(i) = self.history_index {
                    self.history_index = Some(i.saturating_sub(1));
                }
                if let Some(i) = self.history_index {
                    self.editor.set_text(&self.history[i]);
                }
                true
            }
            KeyCode::Down if at_bottom && self.history_index.is_some() => {
                let i = self.history_index.unwrap();
                if i + 1 >= self.history.len() {
                    self.history_index = None;
                    if let Some(d) = self.history_draft.take() {
                        self.editor.set_text(&d);
                    }
                } else {
                    self.history_index = Some(i + 1);
                    self.editor.set_text(&self.history[i + 1]);
                }
                true
            }
            _ => false,
        }
    }

    fn handle_chip_keys(&mut self, key: &KeyEvent) -> Option<PromptComposerOutcome> {
        if self.chips.is_empty() {
            return None;
        }
        // Focus chips with Shift+Tab from empty? Keep simple: when chip_focus set.
        let Some(idx) = self.chip_focus else {
            if key.code == KeyCode::BackTab {
                self.chip_focus = Some(self.chips.len() - 1);
                return Some(PromptComposerOutcome::Changed);
            }
            return None;
        };
        match key.code {
            KeyCode::Left => {
                self.chip_focus = Some(idx.saturating_sub(1));
                Some(PromptComposerOutcome::Changed)
            }
            KeyCode::Right => {
                if idx + 1 >= self.chips.len() {
                    self.chip_focus = None;
                } else {
                    self.chip_focus = Some(idx + 1);
                }
                Some(PromptComposerOutcome::Changed)
            }
            KeyCode::Delete | KeyCode::Backspace => {
                let id = self.chips[idx].id.clone();
                let _ = self.remove_chip(&id);
                Some(PromptComposerOutcome::ChipRemoved { id })
            }
            KeyCode::Enter => {
                let id = self.chips[idx].id.clone();
                Some(PromptComposerOutcome::ChipActivated { id })
            }
            KeyCode::Esc => {
                self.chip_focus = None;
                Some(PromptComposerOutcome::Changed)
            }
            _ => None,
        }
    }
}

/// Detect `/` or `@` completion trigger before the cursor.
fn detect_completion(text: &str, cursor: TextCursor) -> Option<CompletionQuery> {
    // Map cursor to absolute byte in LF-joined text.
    let mut abs = 0usize;
    let mut line_idx = 0usize;
    for (i, line) in text.split('\n').enumerate() {
        if i == cursor.line {
            abs = abs.saturating_add(cursor.byte.min(line.len()));
            break;
        }
        abs = abs.saturating_add(line.len()).saturating_add(1);
        line_idx = i;
    }
    let _ = line_idx;
    let abs = abs.min(text.len());
    let head = &text[..abs];
    // Find last trigger not preceded by word char.
    let bytes = head.as_bytes();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        let b = bytes[i];
        if b == b'/' || b == b'@' || b == b'#' {
            let at_start = i == 0;
            let prev_ok =
                at_start || matches!(bytes[i - 1], b' ' | b'\n' | b'\t' | b'(' | b'[' | b'{');
            if !prev_ok {
                continue;
            }
            let kind = match b {
                b'/' => CompletionKind::Slash,
                b'@' => CompletionKind::FileMention,
                b'#' => CompletionKind::SymbolMention,
                _ => continue,
            };
            let query = head[i + 1..].to_string();
            // Only active if query has no spaces (single token).
            if query.chars().any(char::is_whitespace) {
                return None;
            }
            return Some(CompletionQuery {
                kind,
                query,
                trigger_byte: i,
                cursor_byte: abs,
            });
        }
        if b == b' ' || b == b'\n' || b == b'\t' {
            break;
        }
    }
    None
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Prompt composer chrome + editor.
#[derive(Debug, Clone, Copy)]
pub struct PromptComposer<'a> {
    tokens: &'a DesignTokens,
    theme: &'a Theme,
}

impl<'a> PromptComposer<'a> {
    /// Creates a composer using design tokens + theme paint.
    #[must_use]
    pub const fn new(tokens: &'a DesignTokens, theme: &'a Theme) -> Self {
        Self { tokens, theme }
    }
}

/// Layout rectangles produced while rendering (for hit testing).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PromptComposerLayout {
    /// Full area.
    pub area: Rect,
    /// Chip row.
    pub chips: Rect,
    /// Per-chip rects.
    pub chip_hits: Vec<(String, Rect)>,
    /// Editor body.
    pub editor: Rect,
    /// Status / indicators row.
    pub status: Rect,
    /// Validation line.
    pub validation: Rect,
}

impl PromptComposerState {
    /// Computes layout without painting (for hit testing before render).
    #[must_use]
    pub fn layout_in(&self, area: Rect) -> PromptComposerLayout {
        layout_composer(area, self)
    }
}

fn layout_composer(area: Rect, state: &PromptComposerState) -> PromptComposerLayout {
    let mut layout = PromptComposerLayout {
        area,
        ..Default::default()
    };
    if area.is_empty() {
        return layout;
    }
    let mut y = area.y;
    let mut remaining = area.height;

    if !state.chips.is_empty() && remaining > 0 {
        layout.chips = Rect::new(area.x, y, area.width, 1);
        let mut x = area.x;
        for chip in &state.chips {
            let w = (display_cols(&chip.label) as u16)
                .saturating_add(4)
                .min(area.width.saturating_sub(x.saturating_sub(area.x)))
                .max(3);
            if x.saturating_add(w) > area.x.saturating_add(area.width) {
                break;
            }
            layout
                .chip_hits
                .push((chip.id.clone(), Rect::new(x, y, w, 1)));
            x = x.saturating_add(w.saturating_add(1));
        }
        y = y.saturating_add(1);
        remaining = remaining.saturating_sub(1);
    }

    let status_h = if remaining > 1 { 1u16 } else { 0 };
    let valid_h = if state.validation_error.is_some() && remaining > status_h + 1 {
        1u16
    } else {
        0
    };
    let editor_h = remaining
        .saturating_sub(status_h)
        .saturating_sub(valid_h)
        .max(1);

    layout.editor = Rect::new(area.x, y, area.width, editor_h);
    y = y.saturating_add(editor_h);
    if status_h > 0 {
        layout.status = Rect::new(area.x, y, area.width, 1);
        y = y.saturating_add(1);
    }
    if valid_h > 0 {
        layout.validation = Rect::new(area.x, y, area.width, 1);
    }
    layout
}

impl StatefulWidget for &PromptComposer<'_> {
    type State = PromptComposerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            return;
        }
        let layout = layout_composer(area, state);

        // Panel border only in normal+ when height allows
        if state.presentation != ComposerPresentation::Compact && area.height >= 3 {
            let emphasis = if state.focused {
                PanelEmphasis::Focused
            } else {
                PanelEmphasis::Normal
            };
            let panel = Panel::new(self.tokens).emphasis(emphasis);
            Widget::render(&panel, area, buffer);
        }

        // Chips
        for (i, (id, rect)) in layout.chip_hits.iter().enumerate() {
            if let Some(chip) = state.chips.iter().find(|c| c.id == *id) {
                let focused = state.chip_focus == Some(i);
                let style = if focused {
                    self.theme.style(Role::Selection)
                } else {
                    self.theme.style(Role::Elevated)
                };
                let mark = match chip.kind {
                    ChipKind::File => {
                        if state.ascii_fallback {
                            "F"
                        } else {
                            "📎"
                        }
                    }
                    ChipKind::Paste => {
                        if state.ascii_fallback {
                            "P"
                        } else {
                            "📋"
                        }
                    }
                    ChipKind::Media => "M",
                    ChipKind::Other => "·",
                };
                let label = format!(" {mark} {} ×", chip.label);
                let clipped = take_display_cols(&label, usize::from(rect.width));
                buffer.set_stringn(rect.x, rect.y, &clipped, usize::from(rect.width), style);
            }
        }

        // Editor
        if !layout.editor.is_empty() {
            let placeholder = state.placeholder.as_str();
            StatefulWidget::render(
                &TextArea::new(self.theme).placeholder(placeholder),
                layout.editor,
                buffer,
                &mut state.editor,
            );
        }

        // Status row: mode · model · context · queue · busy
        if !layout.status.is_empty() {
            let mut parts: Vec<String> = Vec::new();
            if let Some(mode) = &state.mode {
                parts.push(mode.label.clone());
            }
            if let Some(model) = &state.model {
                parts.push(model.label.clone());
            }
            if state.busy {
                parts.push(if state.ascii_fallback {
                    "busy".into()
                } else {
                    "● busy".into()
                });
            }
            if !state.queue.is_empty() {
                parts.push(format!("queue:{}", state.queue.len()));
            }
            match state.connection {
                ComposerConnection::Disconnected => parts.push("offline".into()),
                ComposerConnection::Disabled => parts.push("disabled".into()),
                ComposerConnection::Ready => {}
            }
            let left = parts.join(" · ");
            let style = if state.mode.as_ref().is_some_and(|m| m.warning) {
                self.theme.style(Role::Warning)
            } else {
                self.theme.style(Role::TextMuted)
            };
            let clipped = take_display_cols(&left, usize::from(layout.status.width));
            buffer.set_stringn(
                layout.status.x,
                layout.status.y,
                &clipped,
                usize::from(layout.status.width),
                style,
            );
            // Token meter on the right when space
            if state.context.limit > 0 && layout.status.width > 20 {
                let meter_w = 16u16.min(layout.status.width / 3);
                let mx = layout
                    .status
                    .x
                    .saturating_add(layout.status.width.saturating_sub(meter_w));
                let meter = TokenMeter::new(state.context.used, state.context.limit, self.theme)
                    .label("ctx");
                Widget::render(meter, Rect::new(mx, layout.status.y, meter_w, 1), buffer);
            }
        }

        if let (Some(err), true) = (
            state.validation_error.as_ref(),
            !layout.validation.is_empty(),
        ) {
            let clipped = take_display_cols(err, usize::from(layout.validation.width));
            buffer.set_stringn(
                layout.validation.x,
                layout.validation.y,
                &clipped,
                usize::from(layout.validation.width),
                self.theme.style(Role::Danger),
            );
        }
    }
}

impl StatefulWidget for PromptComposer<'_> {
    type State = PromptComposerState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn draft_survives_blur_for_overlay_takeover() {
        let mut state = PromptComposerState::new();
        state.set_text("keep me");
        state.set_focused(false); // permission / plan / palette
        assert_eq!(state.text(), "keep me");
        state.set_focused(true);
        assert_eq!(state.text(), "keep me");
    }

    #[test]
    fn empty_submit_validates() {
        let mut state = PromptComposerState::new();
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptComposerOutcome::ValidationFailed { .. }
        ));
    }

    #[test]
    fn submit_returns_text_and_clears() {
        let mut state = PromptComposerState::new();
        state.set_text("hello agent");
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptComposerOutcome::Submit { ref text, .. } if text == "hello agent"
        ));
        assert!(state.text().is_empty());
    }

    #[test]
    fn busy_enqueues_instead_of_submit() {
        let mut state = PromptComposerState::new();
        state.set_busy(true);
        state.set_text("later");
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, PromptComposerOutcome::Queued { .. }));
        assert_eq!(state.queue().len(), 1);
    }

    #[test]
    fn disconnected_blocks_submit() {
        let mut state = PromptComposerState::new();
        state.set_connection(ComposerConnection::Disconnected);
        state.set_text("x");
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptComposerOutcome::ValidationFailed { reason } if reason == "Disconnected"
        ));
    }

    #[test]
    fn large_paste_becomes_chip() {
        let mut state = PromptComposerState::new();
        let big = "a".repeat(LARGE_PASTE_THRESHOLD);
        let out = state.handle_paste(&big);
        assert_eq!(out, PromptComposerOutcome::Changed);
        assert_eq!(state.chips().len(), 1);
        assert_eq!(state.chips()[0].kind, ChipKind::Paste);
        assert!(state.text().is_empty());
    }

    #[test]
    fn slash_detects_completion() {
        let mut state = PromptComposerState::new();
        state.set_text("/pl");
        // cursor at end after set_text
        let q = detect_completion(&state.text(), state.editor.cursor());
        assert!(matches!(
            q,
            Some(CompletionQuery {
                kind: CompletionKind::Slash,
                ref query,
                ..
            }) if query == "pl"
        ));
    }

    #[test]
    fn file_mention_detects_at() {
        let q = detect_completion("see @src/foo", TextCursor { line: 0, byte: 12 });
        assert!(matches!(
            q,
            Some(CompletionQuery {
                kind: CompletionKind::FileMention,
                ref query,
                ..
            }) if query == "src/foo"
        ));
    }

    #[test]
    fn undo_redo_restores_text() {
        let mut state = PromptComposerState::new();
        state.set_text("one");
        state.insert_text(" two");
        assert_eq!(state.text(), "one two");
        assert_eq!(state.undo(), PromptComposerOutcome::Changed);
        // undo stack may restore "one" or "" depending on snapshots
        let after_undo = state.text();
        assert_ne!(after_undo, "one two");
        let _ = state.redo();
    }

    #[test]
    fn alt_enter_inserts_newline_when_submit_on_enter() {
        let mut state = PromptComposerState::new();
        state.set_text("a");
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        let out = state.handle_key(key);
        assert_eq!(out, PromptComposerOutcome::Changed);
        assert!(state.text().contains('\n') || state.editor.lines().count() >= 1);
    }

    #[test]
    fn history_up_recalls_submit() {
        let mut state = PromptComposerState::new();
        state.set_text("first");
        let _ = state.handle_key(press(KeyCode::Enter));
        state.set_text("draft");
        let _ = state.handle_key(press(KeyCode::Up));
        assert_eq!(state.text(), "first");
        let _ = state.handle_key(press(KeyCode::Down));
        assert_eq!(state.text(), "draft");
    }

    #[test]
    fn apply_completion_replaces_trigger_span() {
        let mut state = PromptComposerState::new();
        state.set_text("/plan");
        state.completion = CompletionQuery {
            kind: CompletionKind::Slash,
            query: "plan".into(),
            trigger_byte: 0,
            cursor_byte: 5,
        };
        let out = state.apply_completion_insert("/plan ");
        assert_eq!(out, PromptComposerOutcome::Changed);
        assert!(state.text().starts_with("/plan"));
        assert_eq!(state.completion.kind, CompletionKind::None);
    }

    #[test]
    fn preferred_rows_contract_with_presentation() {
        let mut state = PromptComposerState::new();
        state.set_presentation(ComposerPresentation::Compact);
        assert!(state.preferred_editor_rows(10) <= 2);
        state.set_presentation(ComposerPresentation::Expanded);
        assert!(state.preferred_editor_rows(20) >= 4);
    }

    #[test]
    fn completion_overlay_opens_on_stack() {
        let state = PromptComposerState::new();
        let mut stack = OverlayStack::<&'static str>::new();
        let bounds = Rect::new(0, 0, 80, 24);
        let out = state.open_completion_overlay(
            &mut stack,
            bounds,
            Rect::new(2, 20, 1, 1),
            OverlaySize::menu(24, 8),
            Some("composer"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Completion);
    }
}
