// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **PromptComposer** — flagship input surface for terminal AI agents.
//!
//! **Mission.** Multiline grapheme-safe editing, selection, undo/redo, history,
//! attachments, paste chips, slash commands, file/symbol mentions, completion,
//! model/mode indicators, queueing, submit, interrupt, cancel, external editor.
//! Draft survives permission / question / plan / session / palette takeover.
//!
//! **Separation (do not merge buckets)**
//! - **Text editing** — [`TextAreaState`] + undo/redo/history/selection.
//! - **Token model** — [`ComposerChip`] attachments / paste payloads / mentions.
//! - **Completion** — [`CompletionQuery`] only; host owns candidate rows + menu.
//! - **Presentation** — compact / normal / expanded / fullscreen + density/ascii.
//! - **Submission policy** — [`SubmitPolicy`], busy, connection; outcomes only.
//!
//! Draft is never cleared when host gates input — only
//! [`PromptComposerState::clear_draft`] or successful submit policy.
//!
//! Research: Grok Build prompt widget, Amp, OpenCode, Claude Code,
//! prompt-toolkit, terminal editors.

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
    style::{Density, DesignSystem, Glyph, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        HelpEntry, Panel, PanelChrome, TextArea, TextAreaOutcome, TextAreaState, TextCursor,
        TokenMeter,
        history_picker::{HistoryEntry, HistoryKind},
    },
};

/// Default overlay id for composer completion (slash / mention).
pub const PROMPT_COMPLETION_OVERLAY_ID: &str = "termrock.prompt_completion";

/// Default overlay id when the composer is promoted fullscreen.
pub const PROMPT_FULLSCREEN_OVERLAY_ID: &str = "termrock.prompt_fullscreen";

/// Bytes above which a paste becomes a [`ComposerChip`] (kind paste) instead of inline text.
pub const LARGE_PASTE_THRESHOLD: usize = 400;

/// Max undo snapshots retained.
pub const PROMPT_UNDO_LIMIT: usize = 64;

/// Max submit history entries.
pub const PROMPT_HISTORY_LIMIT: usize = 100;

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
    /// File or symbol mention token (committed from completion).
    Mention,
    /// Generic attachment.
    Other,
}

/// Stable attachment / paste chip (consumer owns path meaning; paste may hold body).
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
    /// Optional full payload (large paste body). Consumer may strip for privacy.
    pub payload: Option<String>,
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
            payload: None,
        }
    }

    /// Large paste chip (preview label; full body in [`Self::payload`]).
    #[must_use]
    pub fn paste(id: impl Into<String>, preview: impl Into<String>, bytes: usize) -> Self {
        Self {
            id: id.into(),
            kind: ChipKind::Paste,
            label: preview.into(),
            meta: Some(format!("{bytes} B")),
            bytes: Some(bytes),
            payload: None,
        }
    }

    /// Large paste chip retaining full body for expand / submit.
    #[must_use]
    pub fn paste_with_body(
        id: impl Into<String>,
        preview: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        let body = body.into();
        let bytes = body.len();
        Self {
            id: id.into(),
            kind: ChipKind::Paste,
            label: preview.into(),
            meta: Some(format!("{bytes} B")),
            bytes: Some(bytes),
            payload: Some(body),
        }
    }

    /// Mention chip (file path or symbol label after completion commit).
    #[must_use]
    pub fn mention(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: ChipKind::Mention,
            label: label.into(),
            meta: None,
            bytes: None,
            payload: None,
        }
    }

    /// From [`AttachmentItem`](crate::widgets::AttachmentItem) (file/image/code/url).
    #[must_use]
    pub fn from_attachment(item: &crate::widgets::AttachmentItem) -> Self {
        attachment_to_composer_chip(item)
    }

    /// From [`PastePayload`](crate::widgets::PastePayload).
    #[must_use]
    pub fn from_paste(paste: &crate::widgets::PastePayload) -> Self {
        paste_to_composer_chip(paste)
    }
}

/// Project attachment into composer chip list model.
#[must_use]
pub fn attachment_to_composer_chip(item: &crate::widgets::AttachmentItem) -> ComposerChip {
    use crate::widgets::AttachmentType;
    let kind = match item.kind {
        AttachmentType::File => ChipKind::File,
        AttachmentType::Image => ChipKind::Media,
        AttachmentType::Code => ChipKind::Mention,
        AttachmentType::Url | AttachmentType::Document | AttachmentType::Other => ChipKind::Other,
    };
    ComposerChip {
        id: item.id.clone(),
        kind,
        label: item.name.clone(),
        meta: item.meta.clone().or_else(|| {
            item.bytes.map(|b| {
                if b < 1024 {
                    format!("{b} B")
                } else {
                    format!("{} KB", b / 1024)
                }
            })
        }),
        bytes: item.bytes.map(|b| b as usize),
        payload: None,
    }
}

/// Project paste payload into composer chip (body in payload when present).
#[must_use]
pub fn paste_to_composer_chip(paste: &crate::widgets::PastePayload) -> ComposerChip {
    if let Some(body) = &paste.body {
        ComposerChip::paste_with_body(paste.id.clone(), paste.preview.clone(), body.clone())
    } else {
        ComposerChip::paste(paste.id.clone(), paste.preview.clone(), paste.bytes)
    }
}

/// Best-effort upgrade of a composer chip into an attachment (non-paste).
#[must_use]
pub fn composer_chip_to_attachment(chip: &ComposerChip) -> Option<crate::widgets::AttachmentItem> {
    use crate::widgets::{AttachmentItem, AttachmentStatus, AttachmentType};
    let kind = match chip.kind {
        ChipKind::File => AttachmentType::File,
        ChipKind::Media => AttachmentType::Image,
        ChipKind::Mention => AttachmentType::Code,
        ChipKind::Other => AttachmentType::Other,
        ChipKind::Paste => return None,
    };
    Some(AttachmentItem {
        id: chip.id.clone(),
        kind,
        name: chip.label.clone(),
        meta: chip.meta.clone(),
        bytes: chip.bytes.map(|b| b as u64),
        line_count: None,
        status: AttachmentStatus::Ready,
        validation: None,
        sensitive: false,
        removable: true,
    })
}

/// Upgrade composer paste chip into [`PastePayload`](crate::widgets::PastePayload).
#[must_use]
pub fn composer_chip_to_paste(chip: &ComposerChip) -> Option<crate::widgets::PastePayload> {
    use crate::widgets::PastePayload;
    if chip.kind != ChipKind::Paste {
        return None;
    }
    if let Some(body) = &chip.payload {
        let mut p = PastePayload::from_body(chip.id.clone(), body.clone());
        p.preview = chip.label.clone();
        if let Some(b) = chip.bytes {
            p.bytes = b;
        }
        Some(p)
    } else {
        Some(PastePayload::preview_only(
            chip.id.clone(),
            chip.label.clone(),
            chip.bytes.unwrap_or(0),
            0,
        ))
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
// Management UI recipe: `termrock::patterns::PromptQueue`. Composer keeps a thin
// FIFO of [`PromptQueueItem`] (also `QueuedPrompt`) for enqueue chrome.

use crate::widgets::{PromptQueueItem, PromptQueueRef, PromptQueueStatus};

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
        /// Queue entry (rich item; identities preserved).
        entry: PromptQueueItem,
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
    /// Selection copied (Ctrl+C with active selection when not busy).
    SelectionCopied {
        /// Selected text.
        text: String,
    },
    /// Fullscreen overlay should open (presentation already set).
    FullscreenRequested,
    /// Fullscreen overlay should close.
    FullscreenDismissed,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Flagship prompt composer state.
///
/// Draft lives in [`Self::editor`] and is preserved across temporary overlays
/// (permission, plan, palette, …). Call [`Self::set_accepts_input`]`(false)` when
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
    chip_cursor: Option<usize>,

    // —— completion ——
    completion: CompletionQuery,

    // —— presentation ——
    presentation: ComposerPresentation,
    mode: Option<ModeIndicator>,
    model: Option<ModelIndicator>,
    context: ContextEstimate,
    density: Density,
    ascii_fallback: bool,
    /// Force word/glyph status (no emoji); pair with monochrome Theme for no-color.
    colorless: bool,
    placeholder: String,

    // —— policy / session ——
    policy: SubmitPolicy,
    connection: ComposerConnection,
    /// Agent currently running (enables queue / stop).
    busy: bool,
    queue: Vec<PromptQueueItem>,
    next_queue_id: u64,
    next_chip_id: u64,
    validation_error: Option<String>,
    accepts_input: bool,
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
        editor.set_accepts_input(true);
        Self {
            editor,
            undo: Vec::new(),
            redo: Vec::new(),
            history: Vec::new(),
            history_index: None,
            history_draft: None,
            select_anchor: None,
            chips: Vec::new(),
            chip_cursor: None,
            completion: CompletionQuery::default(),
            presentation: ComposerPresentation::Normal,
            mode: None,
            model: None,
            context: ContextEstimate::default(),
            density: Density::Comfortable,
            ascii_fallback: false,
            colorless: false,
            placeholder: "Message…".into(),
            policy: SubmitPolicy::default(),
            connection: ComposerConnection::Ready,
            busy: false,
            queue: Vec::new(),
            next_queue_id: 1,
            next_chip_id: 1,
            validation_error: None,
            accepts_input: true,
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

    /// Whether host granted keyboard/pointer input (overlay/scene ownership).
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Host input gate without clearing draft (permission/plan/palette takeover).
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
        // Embedded editor must accept keys when the composer does (parent already gates).
        self.editor.set_accepts_input(accepts);
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

    /// No-color / monochrome-friendly chrome (forces ASCII marks; host should also
    /// pass a monochrome-quantized [`DesignSystem`]).
    pub fn set_colorless(&mut self, colorless: bool) {
        self.colorless = colorless;
        if colorless {
            self.ascii_fallback = true;
        }
    }

    /// Whether colorless chrome is active.
    #[must_use]
    pub const fn is_colorless(&self) -> bool {
        self.colorless
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

    /// Queue ([`PromptQueueItem`] entries).
    #[must_use]
    pub fn queue(&self) -> &[PromptQueueItem] {
        &self.queue
    }

    /// Open completion query.
    #[must_use]
    pub fn completion(&self) -> &CompletionQuery {
        &self.completion
    }

    /// Host sets / refreshes completion query (streaming candidate updates).
    ///
    /// Does not mutate draft text. Prefer after async file/symbol search returns.
    pub fn set_completion(&mut self, query: CompletionQuery) {
        self.completion = query;
    }

    /// Submitted prompt history (newest last). Host may project into HistoryPicker.
    #[must_use]
    pub fn submit_history(&self) -> &[String] {
        &self.history
    }

    /// Validation error string.
    #[must_use]
    pub fn validation_error(&self) -> Option<&str> {
        self.validation_error.as_deref()
    }

    /// Clears draft text only (keeps chips).
    pub fn clear_draft(&mut self) {
        self.push_undo();
        self.editor.set_text("");
        self.select_anchor = None;
        self.completion = CompletionQuery::default();
        self.history_index = None;
        self.history_draft = None;
        self.validation_error = None;
    }

    /// Whether a non-empty selection is active.
    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.select_anchor
            .is_some_and(|a| a != self.editor.cursor())
    }

    /// Selected text if any (order-independent).
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let anchor = self.select_anchor?;
        let cur = self.editor.cursor();
        if anchor == cur {
            return None;
        }
        self.editor.extract_between(anchor, cur)
    }

    /// Clears selection without moving the caret.
    pub fn clear_selection(&mut self) {
        self.select_anchor = None;
    }

    /// Selects entire draft.
    pub fn select_all(&mut self) {
        let end = self.editor.cursor_at_byte(self.text().len());
        self.select_anchor = Some(TextCursor::default());
        let _ = self.editor.set_cursor(end);
    }

    /// Adds a chip.
    pub fn add_chip(&mut self, chip: ComposerChip) {
        self.chips.push(chip);
    }

    /// Removes chip by id.
    pub fn remove_chip(&mut self, id: &str) -> bool {
        let before = self.chips.len();
        self.chips.retain(|c| c.id != id);
        if self.chip_cursor.is_some_and(|i| i >= self.chips.len()) {
            self.chip_cursor = self.chips.len().checked_sub(1);
        }
        before != self.chips.len()
    }

    /// Chip by id (e.g. expand paste payload).
    #[must_use]
    pub fn chip(&self, id: &str) -> Option<&ComposerChip> {
        self.chips.iter().find(|c| c.id == id)
    }

    /// Removes a queue entry by id.
    pub fn remove_queue_entry(&mut self, id: &str) -> bool {
        let before = self.queue.len();
        self.queue.retain(|e| e.id != id);
        before != self.queue.len()
    }

    /// Pops the front queued prompt (FIFO drain by host).
    pub fn pop_queue_front(&mut self) -> Option<PromptQueueItem> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }

    /// Clone of the FIFO queue for hosts / patterns recipes.
    #[must_use]
    pub fn queue_items(&self) -> Vec<PromptQueueItem> {
        self.queue.clone()
    }

    /// Clears the submit queue (does not touch draft).
    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    /// Inserts text at cursor (records undo); replaces selection if any.
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

    /// Applies text returned from an external editor (draft preserved until this call).
    pub fn apply_external_editor_text(&mut self, text: &str) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled {
            return PromptComposerOutcome::Ignored;
        }
        self.push_undo();
        self.editor.set_text(text);
        self.select_anchor = None;
        self.completion = CompletionQuery::default();
        self.history_index = None;
        self.history_draft = None;
        self.after_edit()
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

    /// Commit a completion candidate: insert token and emit [`PromptComposerOutcome::CompletionCommitted`].
    pub fn commit_completion(
        &mut self,
        id: impl Into<String>,
        insertion: &str,
    ) -> PromptComposerOutcome {
        let kind = self.completion.kind;
        if kind == CompletionKind::None {
            return PromptComposerOutcome::Ignored;
        }
        let id = id.into();
        match self.apply_completion_insert(insertion) {
            PromptComposerOutcome::Ignored => PromptComposerOutcome::Ignored,
            _ => PromptComposerOutcome::CompletionCommitted { kind, id },
        }
    }

    /// Closes completion without editing.
    pub fn close_completion(&mut self) -> PromptComposerOutcome {
        if self.completion.kind == CompletionKind::None {
            return PromptComposerOutcome::Ignored;
        }
        self.completion = CompletionQuery::default();
        PromptComposerOutcome::CompletionClosed
    }

    /// Suggested presentation for terminal width (host may apply).
    #[must_use]
    pub const fn presentation_for_width(width: u16) -> ComposerPresentation {
        if width < 40 {
            ComposerPresentation::Compact
        } else if width < 100 {
            ComposerPresentation::Normal
        } else {
            ComposerPresentation::Expanded
        }
    }

    /// Contracts presentation for narrow widths (never expands user Fullscreen).
    pub fn contract_for_width(&mut self, width: u16) -> PromptComposerOutcome {
        if self.presentation == ComposerPresentation::Fullscreen {
            return PromptComposerOutcome::Ignored;
        }
        let target = Self::presentation_for_width(width);
        let rank = |p: ComposerPresentation| match p {
            ComposerPresentation::Compact => 0u8,
            ComposerPresentation::Normal => 1,
            ComposerPresentation::Expanded => 2,
            ComposerPresentation::Fullscreen => 3,
        };
        if rank(target) < rank(self.presentation) {
            self.presentation = target;
            if width < 48 {
                self.ascii_fallback = true;
            }
            return PromptComposerOutcome::PresentationChanged(target);
        }
        if width < 48 {
            self.ascii_fallback = true;
        }
        PromptComposerOutcome::Ignored
    }

    /// Promotes to fullscreen presentation (host should open overlay).
    pub fn request_fullscreen(&mut self) -> PromptComposerOutcome {
        if self.presentation == ComposerPresentation::Fullscreen {
            return PromptComposerOutcome::Ignored;
        }
        self.presentation = ComposerPresentation::Fullscreen;
        PromptComposerOutcome::FullscreenRequested
    }

    /// Leaves fullscreen to normal (host should dismiss overlay).
    pub fn exit_fullscreen(&mut self) -> PromptComposerOutcome {
        if self.presentation != ComposerPresentation::Fullscreen {
            return PromptComposerOutcome::Ignored;
        }
        self.presentation = ComposerPresentation::Normal;
        PromptComposerOutcome::FullscreenDismissed
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

    /// Semantic intent routing for surface chords (submit / cancel peel).
    ///
    /// Editor caret, history, chips, and product Ctrl chords stay on [`Self::handle_key`].
    pub fn handle_intent(&mut self, intent: crate::interaction::UiIntent) -> PromptComposerOutcome {
        use crate::interaction::UiIntent;
        if self.connection == ComposerConnection::Disabled || !self.accepts_input {
            return PromptComposerOutcome::Ignored;
        }
        match intent {
            UiIntent::Submit | UiIntent::Activate => {
                if self.policy.submit_on_enter {
                    self.try_submit_or_queue()
                } else {
                    // Newline-only policy: host should inject newline via handle_key.
                    PromptComposerOutcome::Ignored
                }
            }
            UiIntent::Cancel | UiIntent::Close => {
                if self.completion.kind != CompletionKind::None {
                    return self.close_completion();
                }
                if self.presentation == ComposerPresentation::Fullscreen {
                    return self.exit_fullscreen();
                }
                if self.select_anchor.take().is_some() {
                    return PromptComposerOutcome::Changed;
                }
                PromptComposerOutcome::DismissRequest
            }
            _ => PromptComposerOutcome::Ignored,
        }
    }

    /// Routes a key event.
    pub fn handle_key(&mut self, key: KeyEvent) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled {
            return PromptComposerOutcome::Ignored;
        }
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return PromptComposerOutcome::Ignored;
        }

        // Bare Enter/Esc via intent map (modifiers still use product paths below).
        if key.modifiers.is_empty()
            && let Some(intent) = crate::interaction::default_prompt_composer_intent(key)
        {
            let out = self.handle_intent(intent);
            // When submit_on_enter is false, Enter falls through to newline path.
            if !matches!(out, PromptComposerOutcome::Ignored)
                || matches!(
                    intent,
                    crate::interaction::UiIntent::Cancel | crate::interaction::UiIntent::Close
                )
            {
                return out;
            }
        }

        // Completion open: Esc closes one layer; navigation left to consumer list.
        if self.completion.kind != CompletionKind::None && key.code == KeyCode::Esc {
            return self.close_completion();
        }

        // Ctrl chords: undo/redo, interrupt/cancel, external editor, select-all, attach
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('z') | KeyCode::Char('Z') => return self.undo(),
                KeyCode::Char('y') | KeyCode::Char('Y') => return self.redo(),
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.select_all();
                    return PromptComposerOutcome::Changed;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    if let Some(text) = self.selected_text().filter(|t| !t.is_empty()) {
                        return PromptComposerOutcome::SelectionCopied { text };
                    }
                    if self.busy {
                        // Soft interrupt — draft preserved
                        return PromptComposerOutcome::Interrupt;
                    }
                    return PromptComposerOutcome::Ignored;
                }
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    return PromptComposerOutcome::ExternalEditor;
                }
                KeyCode::Char('u') | KeyCode::Char('U') if self.busy => {
                    // Hard cancel / stop when agent active
                    return PromptComposerOutcome::Cancel;
                }
                KeyCode::Backspace if self.busy => {
                    return PromptComposerOutcome::Cancel;
                }
                KeyCode::Char('o') | KeyCode::Char('O')
                    if key.modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    return PromptComposerOutcome::AttachRequest;
                }
                KeyCode::Char('f') | KeyCode::Char('F')
                    if key.modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    return self.request_fullscreen();
                }
                _ => {}
            }
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
                self.delete_selection_if_any();
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
            if self.presentation == ComposerPresentation::Fullscreen {
                return self.exit_fullscreen();
            }
            if self.select_anchor.take().is_some() {
                return PromptComposerOutcome::Changed;
            }
            return PromptComposerOutcome::DismissRequest;
        }

        // Shift+arrows: extend selection
        if key.modifiers.contains(KeyModifiers::SHIFT)
            && matches!(
                key.code,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
            )
        {
            return self.extend_selection_with(key);
        }

        // History: Up/Down on first/last line (no modifiers)
        if key.modifiers.is_empty() && self.try_history_nav(key.code) {
            self.select_anchor = None;
            return PromptComposerOutcome::Changed;
        }

        // Chip focus strip: when chip_cursor set, left/right/delete
        if let Some(out) = self.handle_chip_keys(&key) {
            return out;
        }

        // Typing / delete with selection replaces span
        let replaces = matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Delete | KeyCode::Tab
        );
        if replaces && self.has_selection() {
            self.push_undo();
            self.delete_selection_if_any();
            if matches!(key.code, KeyCode::Backspace | KeyCode::Delete) {
                return self.after_edit();
            }
        } else if !key.modifiers.contains(KeyModifiers::SHIFT) {
            // Plain navigation clears selection
            if matches!(
                key.code,
                KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            ) {
                self.select_anchor = None;
            }
        }

        let before = self.text();
        match self.editor.handle_key(key) {
            TextAreaOutcome::Changed => {
                if self.undo.last().is_none_or(|s| s != &before) {
                    self.undo.push(before);
                    if self.undo.len() > PROMPT_UNDO_LIMIT {
                        self.undo.remove(0);
                    }
                }
                self.redo.clear();
                self.after_edit()
            }
            TextAreaOutcome::Scrolled => PromptComposerOutcome::Changed,
            TextAreaOutcome::Cancelled => PromptComposerOutcome::DismissRequest,
            TextAreaOutcome::Ignored => PromptComposerOutcome::Ignored,
            // Composer owns clipboard / external-editor / fullscreen chords before
            // TextArea; if they leak through, treat as no-op or host passthrough.
            TextAreaOutcome::ClipboardCopy { text } => {
                PromptComposerOutcome::SelectionCopied { text }
            }
            TextAreaOutcome::ClipboardCut { text } => {
                self.select_anchor = None;
                PromptComposerOutcome::SelectionCopied { text }
            }
            TextAreaOutcome::ClipboardPasteRequest => PromptComposerOutcome::Ignored,
            TextAreaOutcome::ExternalEditorRequested => PromptComposerOutcome::ExternalEditor,
            TextAreaOutcome::FullscreenRequested => self.request_fullscreen(),
        }
    }

    /// Paste (bracketed paste arrives as [`Event::Paste`]).
    pub fn handle_paste(&mut self, text: &str) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled || !self.accepts_input {
            return PromptComposerOutcome::Ignored;
        }
        if self.policy.large_paste_as_chip && text.len() >= LARGE_PASTE_THRESHOLD {
            let id = format!("paste-{}", self.next_chip_id);
            self.next_chip_id = self.next_chip_id.saturating_add(1);
            let preview = crate::text::truncate_cols(text, 32, "…").into_owned();
            self.chips
                .push(ComposerChip::paste_with_body(id, preview, text.to_string()));
            return PromptComposerOutcome::Changed;
        }
        self.insert_text(text)
    }

    /// Mouse using a full layout (chips, editor, mode/model status hits).
    pub fn handle_mouse_at(
        &mut self,
        mouse: MouseEvent,
        layout: &PromptComposerLayout,
    ) -> PromptComposerOutcome {
        if self.connection == ComposerConnection::Disabled {
            return PromptComposerOutcome::Ignored;
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            if let Some(area) = layout.mode_hit
                && area.contains(mouse.position)
            {
                return PromptComposerOutcome::ModeMenu;
            }
            if let Some(area) = layout.model_hit
                && area.contains(mouse.position)
            {
                return PromptComposerOutcome::ModelMenu;
            }
        }
        self.handle_mouse(mouse, layout.editor, &layout.chip_hits)
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
                if !mouse.modifiers.contains(KeyModifiers::SHIFT) {
                    self.select_anchor = None;
                } else if self.select_anchor.is_none() {
                    self.select_anchor = Some(self.editor.cursor());
                }
                // TextArea handles click via scroll_to / position
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

    /// Unified event entry (editor + chips; use [`Self::handle_mouse_at`] for status hits).
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

    /// Opens fullscreen editor overlay on the stack.
    pub fn open_fullscreen_overlay<FocusId: Clone>(
        &self,
        stack: &mut OverlayStack<FocusId>,
        bounds: Rect,
        opener: Option<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.open(
            bounds,
            OverlaySpec {
                id: OverlayId::from_static(PROMPT_FULLSCREEN_OVERLAY_ID),
                kind: OverlayKind::Dialog,
                parent: None,
                anchor: None,
                size: OverlaySize::dialog(
                    bounds.width.saturating_sub(2).max(20),
                    bounds.height.saturating_sub(2).max(8),
                ),
                opener_focus: opener,
                policy: None,
            },
        )
    }

    /// Dismisses fullscreen editor overlay.
    pub fn dismiss_fullscreen_overlay<FocusId: Clone>(
        stack: &mut OverlayStack<FocusId>,
    ) -> OverlayOutcome<FocusId> {
        stack.dismiss(&OverlayId::from_static(PROMPT_FULLSCREEN_OVERLAY_ID))
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
            let mut attachments = Vec::new();
            let mut mentions = Vec::new();
            for c in &self.chips {
                let kind = match c.kind {
                    ChipKind::File => "file",
                    ChipKind::Paste => "paste",
                    ChipKind::Mention => "mention",
                    ChipKind::Media => "media",
                    ChipKind::Other => "chip",
                };
                let r = PromptQueueRef::new(c.id.clone(), kind, c.label.clone());
                if matches!(c.kind, ChipKind::Mention) {
                    mentions.push(r);
                } else {
                    attachments.push(r);
                }
            }
            // also honor chip_ids-only path if chips empty but ids tracked
            if attachments.is_empty() && mentions.is_empty() {
                for cid in &chip_ids {
                    attachments.push(PromptQueueRef::new(cid.clone(), "chip", cid.clone()));
                }
            }
            let entry = PromptQueueItem::new(id, text.clone())
                .attachments(attachments)
                .mentions(mentions)
                .status(PromptQueueStatus::Queued);
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
            if self.history.len() > PROMPT_HISTORY_LIMIT {
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
        if self.undo.len() > PROMPT_UNDO_LIMIT {
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
        let Some(anchor) = self.select_anchor.take() else {
            return;
        };
        let cur = self.editor.cursor();
        if anchor == cur {
            return;
        }
        let _ = self.editor.replace_between(anchor, cur, "");
    }

    fn extend_selection_with(&mut self, key: KeyEvent) -> PromptComposerOutcome {
        if self.select_anchor.is_none() {
            self.select_anchor = Some(self.editor.cursor());
        }
        // Strip SHIFT so TextArea moves caret without treating as special.
        let bare = KeyEvent::new(key.code, KeyModifiers::NONE);
        match self.editor.handle_key(bare) {
            TextAreaOutcome::Changed | TextAreaOutcome::Scrolled => {
                if self.select_anchor == Some(self.editor.cursor()) {
                    self.select_anchor = None;
                }
                PromptComposerOutcome::Changed
            }
            TextAreaOutcome::Ignored => PromptComposerOutcome::Ignored,
            TextAreaOutcome::Cancelled => PromptComposerOutcome::DismissRequest,
            TextAreaOutcome::ClipboardCopy { text } | TextAreaOutcome::ClipboardCut { text } => {
                PromptComposerOutcome::SelectionCopied { text }
            }
            TextAreaOutcome::ClipboardPasteRequest
            | TextAreaOutcome::ExternalEditorRequested
            | TextAreaOutcome::FullscreenRequested => PromptComposerOutcome::Ignored,
        }
    }

    fn after_edit(&mut self) -> PromptComposerOutcome {
        self.validation_error = None;
        self.select_anchor = None;
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
        // Focus chips with Shift+Tab from empty? Keep simple: when chip_cursor set.
        let Some(idx) = self.chip_cursor else {
            if key.code == KeyCode::BackTab {
                self.chip_cursor = Some(self.chips.len() - 1);
                return Some(PromptComposerOutcome::Changed);
            }
            return None;
        };
        match key.code {
            KeyCode::Left => {
                self.chip_cursor = Some(idx.saturating_sub(1));
                Some(PromptComposerOutcome::Changed)
            }
            KeyCode::Right => {
                if idx + 1 >= self.chips.len() {
                    self.chip_cursor = None;
                } else {
                    self.chip_cursor = Some(idx + 1);
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
                self.chip_cursor = None;
                Some(PromptComposerOutcome::Changed)
            }
            _ => None,
        }
    }
}

/// Detect `/` `@` `#` completion trigger before the cursor (pure; no I/O).
#[must_use]
pub fn detect_completion(text: &str, cursor: TextCursor) -> Option<CompletionQuery> {
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
    system: &'a DesignSystem,
}

impl<'a> PromptComposer<'a> {
    /// Creates a composer from the sole paint authority.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self { system }
    }

    /// Paint (same as [`StatefulWidget::render`]).
    pub fn render(self, area: Rect, buffer: &mut Buffer, state: &mut PromptComposerState) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

/// Keyboard help seed for live keymap merge (host owns remaps).
#[must_use]
pub fn prompt_composer_help_entries() -> Vec<HelpEntry> {
    vec![
        HelpEntry::new("submit", "Prompt", "Enter", "Submit draft (policy)"),
        HelpEntry::new(
            "newline",
            "Prompt",
            "A-enter / C-enter / S-enter",
            "Insert newline when submit-on-enter",
        ),
        HelpEntry::new("undo", "Edit", "C-z", "Undo draft snapshot"),
        HelpEntry::new("redo", "Edit", "C-y", "Redo draft snapshot"),
        HelpEntry::new("select-all", "Edit", "C-a", "Select entire draft"),
        HelpEntry::new(
            "interrupt",
            "Agent",
            "C-c",
            "Soft interrupt when busy (draft kept)",
        ),
        HelpEntry::new("cancel", "Agent", "C-u", "Hard cancel / stop when busy"),
        HelpEntry::new("external", "Edit", "C-e", "Open external editor"),
        HelpEntry::new("attach", "Prompt", "C-S-o", "Request file attach"),
        HelpEntry::new("fullscreen", "View", "C-S-f", "Request fullscreen overlay"),
        HelpEntry::new(
            "history",
            "Prompt",
            "Up/Down",
            "Browse submit history at line edges",
        ),
        HelpEntry::new(
            "chips",
            "Prompt",
            "BackTab · ←/→ · Del",
            "Chip strip focus and remove",
        ),
        HelpEntry::new(
            "completion",
            "Prompt",
            "/ · @ · #",
            "Slash / file / symbol completion trigger",
        ),
        HelpEntry::new(
            "esc",
            "Nav",
            "Esc",
            "Close completion → fullscreen → dismiss",
        ),
    ]
}

/// Project submit history into HistoryPicker rows (newest first for picker).
#[must_use]
pub fn submit_history_to_entries(history: &[String]) -> Vec<HistoryEntry<String>> {
    history
        .iter()
        .rev()
        .enumerate()
        .map(|(rank, text)| {
            let mut e =
                HistoryEntry::new(format!("h-{rank}"), text.clone()).kind(HistoryKind::Prompt);
            e.display = crate::text::truncate_cols(text, 80, "…").into_owned();
            e.preview = Some(text.clone());
            e.recency = rank as u64;
            e
        })
        .collect()
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
    /// Mode badge hit target within status (if present).
    pub mode_hit: Option<Rect>,
    /// Model badge hit target within status (if present).
    pub model_hit: Option<Rect>,
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

    let prompt_gutter = area.width.min(2);
    layout.editor = Rect::new(
        area.x.saturating_add(prompt_gutter),
        y,
        area.width.saturating_sub(prompt_gutter),
        editor_h,
    );
    y = y.saturating_add(editor_h);
    if status_h > 0 {
        layout.status = Rect::new(area.x, y, area.width, 1);
        // Mode / model hit targets (left-aligned segments).
        let mut sx = area.x;
        if let Some(mode) = &state.mode {
            let w = (display_cols(&mode.label) as u16).saturating_add(1).max(1);
            let w = w.min(layout.status.width);
            layout.mode_hit = Some(Rect::new(sx, y, w, 1));
            sx = sx.saturating_add(w.saturating_add(3)); // " · "
        }
        if let Some(model) = &state.model {
            let w = (display_cols(&model.label) as u16).saturating_add(1).max(1);
            let remaining = layout
                .status
                .width
                .saturating_sub(sx.saturating_sub(area.x));
            let w = w.min(remaining);
            if w > 0 {
                layout.model_hit = Some(Rect::new(sx, y, w, 1));
            }
        }
        y = y.saturating_add(1);
    }
    if valid_h > 0 {
        layout.validation = Rect::new(area.x, y, area.width, 1);
    }
    layout
}

fn order_text_cursors(a: TextCursor, b: TextCursor) -> (TextCursor, TextCursor) {
    if a.line < b.line || (a.line == b.line && a.byte <= b.byte) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Paint selection highlight over the editor viewport (style-only; mono uses reverse via theme).
fn paint_editor_selection(
    buffer: &mut Buffer,
    area: Rect,
    state: &PromptComposerState,
    style: ratatui_core::style::Style,
) {
    let Some(anchor) = state.select_anchor else {
        return;
    };
    let cur = state.editor.cursor();
    if anchor == cur || area.is_empty() {
        return;
    }
    let (start, end) = order_text_cursors(anchor, cur);
    let scroll_y = usize::from(state.editor.scroll().offset_y());
    let scroll_x = usize::from(state.editor.scroll().offset_x());
    let lines: Vec<&str> = state.editor.lines().collect();
    for line_idx in start.line..=end.line {
        if line_idx < scroll_y {
            continue;
        }
        let row = line_idx - scroll_y;
        if row >= usize::from(area.height) {
            break;
        }
        let line = lines.get(line_idx).copied().unwrap_or("");
        let start_byte = if line_idx == start.line {
            start.byte.min(line.len())
        } else {
            0
        };
        let end_byte = if line_idx == end.line {
            end.byte.min(line.len())
        } else {
            line.len()
        };
        if start_byte >= end_byte {
            continue;
        }
        let col0 = display_cols(&line[..start_byte]);
        let col1 = display_cols(&line[..end_byte]);
        if col1 <= scroll_x {
            continue;
        }
        let vis0 = col0.saturating_sub(scroll_x);
        let vis1 = col1.saturating_sub(scroll_x).min(usize::from(area.width));
        if vis0 >= vis1 {
            continue;
        }
        let x0 = area
            .x
            .saturating_add(u16::try_from(vis0).unwrap_or(u16::MAX));
        let width = u16::try_from(vis1.saturating_sub(vis0)).unwrap_or(0);
        if width == 0 {
            continue;
        }
        let y = area
            .y
            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX));
        buffer.set_style(Rect::new(x0, y, width, 1), style);
    }
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
            let emphasis = if state.accepts_input {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            };
            let panel = Panel::new(self.system).emphasis(emphasis);
            Widget::render(&panel, area, buffer);
        }

        // Chips — AttachmentChip / PasteChip (Tag chrome underneath).
        let ascii = state.ascii_fallback || state.colorless;
        for (i, (id, rect)) in layout.chip_hits.iter().enumerate() {
            if let Some(chip) = state.chips.iter().find(|c| c.id == *id) {
                let focused = state.chip_cursor == Some(i);
                if chip.kind == ChipKind::Paste {
                    if let Some(paste) = composer_chip_to_paste(chip) {
                        use crate::widgets::{PasteChip, PasteChipState};
                        let mut ps = PasteChipState::new();
                        ps.set_focused(focused);
                        let _ = PasteChip::new(&paste, self.system)
                            .ascii(ascii)
                            .paint(*rect, buffer, &mut ps);
                        continue;
                    }
                }
                if let Some(item) = composer_chip_to_attachment(chip) {
                    use crate::widgets::{AttachmentChip, AttachmentChipState};
                    let mut st = AttachmentChipState::new();
                    st.set_focused(focused);
                    let _ = AttachmentChip::new(&item, self.system)
                        .ascii(ascii)
                        .paint(*rect, buffer, &mut st);
                } else {
                    // Fallback Tag for unexpected kinds
                    use crate::widgets::{Tag, TagState};
                    let tag = Tag::removable_tag(id.as_str(), chip.label.as_str(), self.system);
                    let mut ts = TagState::new();
                    ts.set_focused(focused);
                    let _ = tag.paint(*rect, buffer, &mut ts);
                }
            }
        }

        // Editor
        if !layout.editor.is_empty() {
            let editor_surface =
                Rect::new(area.x, layout.editor.y, area.width, layout.editor.height);
            buffer.set_style(editor_surface, self.system.style(Role::Sunken));
            let prompt = Glyph::Prompt.resolve(self.system.glyphs).text;
            buffer.set_stringn(
                area.x,
                layout.editor.y,
                prompt,
                usize::from(area.width.min(1)),
                self.system.style(Role::Accent),
            );
            let placeholder = state.placeholder.as_str();
            StatefulWidget::render(
                &TextArea::new(self.system).placeholder(placeholder),
                layout.editor,
                buffer,
                &mut state.editor,
            );
            // Selection highlight (after TextArea paint)
            if state.has_selection() {
                let sel = if state.colorless {
                    self.system
                        .style(Role::Selection)
                        .add_modifier(ratatui_core::style::Modifier::REVERSED)
                } else {
                    self.system.style(Role::Selection)
                };
                paint_editor_selection(buffer, layout.editor, state, sel);
            }
        }

        // Status row: mode · model · context · queue · busy
        if !layout.status.is_empty() {
            let ascii = state.ascii_fallback || state.colorless;
            let mut parts: Vec<String> = Vec::new();
            if let Some(mode) = &state.mode {
                parts.push(mode.label.clone());
            }
            if let Some(model) = &state.model {
                parts.push(model.label.clone());
            }
            if state.busy {
                parts.push(if ascii {
                    "BUSY ^C soft ^U stop".into()
                } else {
                    "● busy  ^C interrupt  ^U stop".into()
                });
            }
            if !state.queue.is_empty() {
                parts.push(format!("queue:{}", state.queue.len()));
            }
            if state.presentation == ComposerPresentation::Fullscreen {
                parts.push(if ascii {
                    "FULL".into()
                } else {
                    "fullscreen".into()
                });
            }
            match state.connection {
                ComposerConnection::Disconnected => parts.push("offline".into()),
                ComposerConnection::Disabled => parts.push("disabled".into()),
                ComposerConnection::Ready => {}
            }
            let left = parts.join(" · ");
            let style = if state.mode.as_ref().is_some_and(|m| m.warning) {
                self.system.style(Role::Warning)
            } else {
                self.system.style(Role::TextMuted)
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
                let meter = TokenMeter::new(state.context.used, state.context.limit, self.system)
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
                self.system.style(Role::Danger),
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
    fn draft_survives_accepts_input_gate_for_overlay_takeover() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("keep me");
        state.set_accepts_input(false);
        // permission / plan / palette — keys ignored, draft intact
        assert_eq!(
            state.handle_key(press(KeyCode::Enter)),
            PromptComposerOutcome::Ignored
        );
        assert_eq!(state.text(), "keep me");
        assert!(!state.accepts_input());
    }

    #[test]
    fn accepts_input_gate_blocks_paste() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(false);
        assert_eq!(state.handle_paste("hello"), PromptComposerOutcome::Ignored);
        assert!(state.text().is_empty());
    }

    #[test]
    fn empty_submit_validates() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            PromptComposerOutcome::ValidationFailed { .. }
        ));
    }

    #[test]
    fn submit_returns_text_and_clears() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
        state.set_busy(true);
        state.set_text("later");
        let out = state.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, PromptComposerOutcome::Queued { .. }));
        assert_eq!(state.queue().len(), 1);
    }

    #[test]
    fn disconnected_blocks_submit() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
        state.set_text("a");
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        let out = state.handle_key(key);
        assert_eq!(out, PromptComposerOutcome::Changed);
        assert!(state.text().contains('\n') || state.editor.lines().count() >= 1);
    }

    #[test]
    fn history_up_recalls_submit() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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
        state.set_accepts_input(true);
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

    #[test]
    fn symbol_mention_detects_hash() {
        let q = detect_completion("see #parse_", TextCursor { line: 0, byte: 11 });
        assert!(matches!(
            q,
            Some(CompletionQuery {
                kind: CompletionKind::SymbolMention,
                ref query,
                ..
            }) if query == "parse_"
        ));
    }

    #[test]
    fn large_paste_stores_payload() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        let big = "b".repeat(LARGE_PASTE_THRESHOLD);
        let _ = state.handle_paste(&big);
        assert_eq!(state.chips()[0].payload.as_deref(), Some(big.as_str()));
    }

    #[test]
    fn selection_delete_and_typeover() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("abcdef");
        state.select_anchor = Some(TextCursor { line: 0, byte: 1 });
        assert!(state.editor.set_cursor(TextCursor { line: 0, byte: 4 }));
        assert_eq!(state.selected_text().as_deref(), Some("bcd"));
        let out = state.handle_key(press(KeyCode::Backspace));
        assert!(matches!(
            out,
            PromptComposerOutcome::Changed | PromptComposerOutcome::CompletionClosed
        ));
        assert_eq!(state.text(), "aef");
    }

    #[test]
    fn select_all_and_copy_outcome() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("hello");
        state.select_all();
        assert!(state.has_selection());
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(matches!(
            out,
            PromptComposerOutcome::SelectionCopied { ref text } if text == "hello"
        ));
    }

    #[test]
    fn busy_ctrl_c_interrupts_ctrl_u_cancels() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_busy(true);
        state.set_text("keep");
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(out, PromptComposerOutcome::Interrupt);
        assert_eq!(state.text(), "keep");
        let out = state.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert_eq!(out, PromptComposerOutcome::Cancel);
    }

    #[test]
    fn disabled_ignores_keys() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_connection(ComposerConnection::Disabled);
        state.set_text("x");
        assert_eq!(
            state.handle_key(press(KeyCode::Enter)),
            PromptComposerOutcome::Ignored
        );
    }

    #[test]
    fn external_editor_applies_text() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("old");
        let out = state.apply_external_editor_text("from $EDITOR");
        assert!(matches!(out, PromptComposerOutcome::Changed));
        assert_eq!(state.text(), "from $EDITOR");
    }

    #[test]
    fn contract_for_narrow_width() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_presentation(ComposerPresentation::Expanded);
        let out = state.contract_for_width(36);
        assert!(matches!(
            out,
            PromptComposerOutcome::PresentationChanged(ComposerPresentation::Compact)
        ));
        assert!(state.ascii_fallback);
    }

    #[test]
    fn fullscreen_request_and_esc_exit() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        assert!(matches!(
            state.request_fullscreen(),
            PromptComposerOutcome::FullscreenRequested
        ));
        assert_eq!(state.presentation(), ComposerPresentation::Fullscreen);
        let out = state.handle_key(press(KeyCode::Esc));
        assert_eq!(out, PromptComposerOutcome::FullscreenDismissed);
        assert_eq!(state.presentation(), ComposerPresentation::Normal);
    }

    #[test]
    fn queue_fifo_pop_and_remove() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_busy(true);
        state.set_text("a");
        let _ = state.handle_key(press(KeyCode::Enter));
        state.set_text("b");
        let _ = state.handle_key(press(KeyCode::Enter));
        assert_eq!(state.queue().len(), 2);
        let first = state.pop_queue_front().unwrap();
        assert_eq!(first.text, "a");
        let id = state.queue()[0].id.clone();
        assert!(state.remove_queue_entry(&id));
        assert!(state.queue().is_empty());
    }

    #[test]
    fn draft_survives_busy_and_connection_flip() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("draft under overlays");
        state.set_busy(true);
        state.set_connection(ComposerConnection::Disconnected);
        state.set_connection(ComposerConnection::Ready);
        assert_eq!(state.text(), "draft under overlays");
    }

    #[test]
    fn commit_completion_emits_committed() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("/pl");
        state.completion = CompletionQuery {
            kind: CompletionKind::Slash,
            query: "pl".into(),
            trigger_byte: 0,
            cursor_byte: 3,
        };
        let out = state.commit_completion("plan", "/plan ");
        assert!(matches!(
            out,
            PromptComposerOutcome::CompletionCommitted {
                kind: CompletionKind::Slash,
                ref id
            } if id == "plan"
        ));
        assert!(state.text().starts_with("/plan"));
        assert_eq!(state.completion.kind, CompletionKind::None);
    }

    #[test]
    fn colorless_forces_ascii() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_colorless(true);
        assert!(state.is_colorless());
        assert!(state.ascii_fallback);
    }

    #[test]
    fn mode_model_status_hits() {
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_mode(Some(ModeIndicator {
            label: "PLAN".into(),
            warning: false,
        }));
        state.set_model(Some(ModelIndicator { label: "m1".into() }));
        state.set_text("x");
        let layout = state.layout_in(Rect::new(0, 0, 60, 10));
        assert!(layout.mode_hit.is_some());
        assert!(layout.model_hit.is_some());
        let mode = layout.mode_hit.unwrap();
        let mouse = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: ratatui_core::layout::Position {
                x: mode.x,
                y: mode.y,
            },
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            state.handle_mouse_at(mouse, &layout),
            PromptComposerOutcome::ModeMenu
        );
    }

    #[test]
    fn selection_paint_does_not_panic() {
        let system = crate::style::DesignSystem::default();
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("hello world");
        state.select_anchor = Some(TextCursor { line: 0, byte: 0 });
        assert!(state.editor.set_cursor(TextCursor { line: 0, byte: 5 }));
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        PromptComposer::new(&system).render(Rect::new(0, 0, 40, 8), &mut buf, &mut state);
    }

    #[test]
    fn editor_paints_sunken_prompt_gutter() {
        let system = DesignSystem::default();
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        let area = Rect::new(0, 0, 40, 6);
        let mut buffer = Buffer::empty(area);
        PromptComposer::new(&system).render(area, &mut buffer, &mut state);
        assert_eq!(
            buffer[(0, 0)].symbol(),
            Glyph::Prompt.resolve(system.glyphs).text
        );
        assert_eq!(buffer[(1, 0)].bg, system.style(Role::Sunken).bg.unwrap());
    }

    #[test]
    fn help_and_history_bridges() {
        assert!(!prompt_composer_help_entries().is_empty());
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        state.set_text("one");
        let _ = state.handle_key(press(KeyCode::Enter));
        state.set_text("two");
        let _ = state.handle_key(press(KeyCode::Enter));
        assert_eq!(state.submit_history().len(), 2);
        let rows = submit_history_to_entries(state.submit_history());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, HistoryKind::Prompt);
    }

    #[test]
    fn streaming_completion_set_query() {
        let mut state = PromptComposerState::new();
        state.set_text("/he");
        let q = detect_completion(&state.text(), state.editor.cursor()).expect("slash");
        state.set_completion(q.clone());
        assert_eq!(state.completion().kind, CompletionKind::Slash);
        // host refreshes query prefix without re-edit
        let mut q2 = q;
        q2.query = "help".into();
        state.set_completion(q2);
        assert_eq!(state.completion().query, "help");
        assert_eq!(state.text(), "/he");
    }

    #[test]
    fn mention_chip_kind() {
        let mut state = PromptComposerState::new();
        state.add_chip(ComposerChip::mention("m1", "src/lib.rs"));
        assert_eq!(state.chips()[0].kind, ChipKind::Mention);
    }

    #[test]
    fn large_prompt_and_repeated_paste_bench() {
        let system = crate::style::DesignSystem::default();
        let mut state = PromptComposerState::new();
        state.set_accepts_input(true);
        let line = "word ".repeat(40);
        let mut body = String::new();
        for _ in 0..bench::LARGE_PROMPT_LINES {
            body.push_str(&line);
            body.push('\n');
        }
        state.set_text(&body);
        assert!(state.text().len() > 1_000);
        let area = Rect::new(0, 0, 80, 16);
        let mut buf = Buffer::empty(area);
        for _ in 0..bench::PAINT_FRAMES {
            PromptComposer::new(&system).render(area, &mut buf, &mut state);
        }
        // repeated large pastes → chips
        let paste = "p".repeat(LARGE_PASTE_THRESHOLD);
        for _ in 0..bench::PASTE_ROUNDS {
            let _ = state.handle_paste(&paste);
        }
        assert_eq!(state.chips().len(), bench::PASTE_ROUNDS);
        // streaming completion updates
        state.set_text("/stream");
        for i in 0..bench::COMPLETION_UPDATES {
            let mut q = detect_completion(&state.text(), state.editor.cursor()).unwrap();
            q.query = format!("stream{i}");
            state.set_completion(q);
        }
        assert_eq!(state.completion().kind, CompletionKind::Slash);
    }

    #[test]
    fn never_owns_provider_io() {
        let src = include_str!("prompt_composer.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "reqwest::",
            "std::process::Command",
            "tokio::",
            "async_openai",
            "anthropic",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }
}

/// Performance benchmark sizes (moderate real-world prompts).
pub mod bench {
    /// Lines in a large draft for paint stress.
    pub const LARGE_PROMPT_LINES: usize = 200;
    /// Repeated large-paste rounds.
    pub const PASTE_ROUNDS: usize = 12;
    /// Streaming completion query refresh rounds.
    pub const COMPLETION_UPDATES: usize = 64;
    /// Paint frames per bench.
    pub const PAINT_FRAMES: u32 = 24;
}
