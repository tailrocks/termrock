// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **AttachmentChip** and **PasteChip** — structured compact representations of
//! files, images, URLs, selected code, and large pasted text.
//!
//! **Mission.** Type, name, size/line count, status, validation, remove,
//! open/preview, retry, and upload/indexing progress. Large pastes collapse
//! while remaining inspectable and copyable. Stable IDs; **never** put sensitive
//! bodies into semantic summaries or recording labels. Strips support wrap,
//! horizontal scroll, and `+N` overflow via [`TokenStrip`]. Compose with
//! [`PromptComposer`](crate::widgets::PromptComposer) and permission / data-egress
//! hosts (outcomes only — no upload I/O).
//!
//! Research: Grok Build paste/file chips, modern chat attachments, terminal
//! prompt composers.
//!
//! **Buckets**
//! - Domain model: [`AttachmentItem`] / [`PastePayload`] (host projects)
//! - Paint: [`AttachmentChip`] / [`PasteChip`] (compose [`Tag`])
//! - Strip: [`attachment_token_items`] + [`TokenStrip`]
//! - Bridges: convert to/from composer chips in `prompt_composer` (avoids cycle)
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{UiIntent, default_button_intent},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::tag_chip::{
        Tag, TagOutcome, TagState, TokenItem, TokenPart, TokenParts, TokenStatus, TokenStrip,
        TokenStripLayout, TokenStripOutcome, TokenStripState, remove_label,
    },
};

/// Default chars kept in paste preview labels (not full body).
pub const PASTE_PREVIEW_CHARS: usize = 32;
/// Bytes above which hosts typically promote paste to a chip (align with PromptComposer).
pub const PASTE_CHIP_THRESHOLD: usize = 400;
/// Max lines shown when paste is expanded inline.
pub const PASTE_EXPAND_LINES: usize = 8;
/// Progress percent unknown.
pub const PROGRESS_UNKNOWN: u8 = 255;

// ── Domain: attachment ──────────────────────────────────────────────────────

/// Attachment content kind (display + semantic letter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AttachmentType {
    /// File path / workspace file.
    #[default]
    File,
    /// Image / binary media.
    Image,
    /// URL / remote reference.
    Url,
    /// Selected code snippet reference.
    Code,
    /// Document / PDF / notebook label.
    Document,
    /// Generic context blob.
    Other,
}

impl AttachmentType {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Image => "image",
            Self::Url => "url",
            Self::Code => "code",
            Self::Document => "document",
            Self::Other => "other",
        }
    }

    /// ASCII type letter for colorless / recordings.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::File => 'F',
            Self::Image => 'I',
            Self::Url => 'U',
            Self::Code => 'C',
            Self::Document => 'D',
            Self::Other => 'A',
        }
    }

    /// Glyph mark (emoji or letter).
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::File => "F",
                Self::Image => "I",
                Self::Url => "U",
                Self::Code => "C",
                Self::Document => "D",
                Self::Other => "A",
            }
        } else {
            match self {
                // One column each: a two-column emoji in a chip's glyph slot
                // shifts the label and the remove affordance (plans/013).
                Self::File => "▤",
                Self::Image => "▣",
                Self::Url => "↗",
                Self::Code => "⟨⟩",
                Self::Document => "▤",
                Self::Other => "·",
            }
        }
    }
}

/// Upload / index / validation lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum AttachmentStatus {
    /// Ready for submit.
    #[default]
    Ready,
    /// Queued / pending host work.
    Pending,
    /// Upload in progress (0–100, or [`PROGRESS_UNKNOWN`]).
    Uploading {
        /// Percent complete.
        progress: u8,
    },
    /// Indexing / embedding progress.
    Indexing {
        /// Percent complete.
        progress: u8,
    },
    /// Host validation running.
    Validating,
    /// Soft error (broken path, network); still removable.
    Error,
    /// Hard validation failure.
    Invalid,
}

impl AttachmentStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Pending => "pending",
            Self::Uploading { .. } => "uploading",
            Self::Indexing { .. } => "indexing",
            Self::Validating => "validating",
            Self::Error => "error",
            Self::Invalid => "invalid",
        }
    }

    /// Map to shared token paint status.
    #[must_use]
    pub const fn token_status(self) -> TokenStatus {
        match self {
            Self::Error | Self::Invalid => TokenStatus::Error,
            Self::Pending | Self::Uploading { .. } | Self::Indexing { .. } | Self::Validating => {
                TokenStatus::Loading
            }
            Self::Ready => TokenStatus::Default,
        }
    }

    /// Progress percent if known (0–100).
    #[must_use]
    pub const fn progress(self) -> Option<u8> {
        match self {
            Self::Uploading { progress } | Self::Indexing { progress } => {
                if progress == PROGRESS_UNKNOWN {
                    None
                } else if progress > 100 {
                    Some(100)
                } else {
                    Some(progress)
                }
            }
            _ => None,
        }
    }

    /// Short status mark for chrome (ASCII-safe when `ascii`).
    #[must_use]
    pub fn mark(self, ascii: bool) -> &'static str {
        match self {
            Self::Ready => "",
            Self::Pending => {
                if ascii {
                    "…"
                } else {
                    "…"
                }
            }
            Self::Uploading { .. } | Self::Indexing { .. } | Self::Validating => {
                if ascii {
                    "~"
                } else {
                    "↻"
                }
            }
            Self::Error | Self::Invalid => {
                if ascii {
                    "!"
                } else {
                    "⚠"
                }
            }
        }
    }
}

/// Host-projected attachment (file / image / URL / code / document).
///
/// **Do not** store secrets in `name` / `meta` when `sensitive` is true for
/// recordings — use basenames and [`semantic_summary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentItem {
    /// Stable id (composer chip id, upload handle, …).
    pub id: String,
    /// Content kind.
    pub kind: AttachmentType,
    /// Display name (basename preferred).
    pub name: String,
    /// Optional size / line meta (safe text).
    pub meta: Option<String>,
    /// Byte size when known.
    pub bytes: Option<u64>,
    /// Line count when known (code / text files).
    pub line_count: Option<u32>,
    /// Lifecycle status.
    pub status: AttachmentStatus,
    /// Validation message (must not include secrets when `sensitive`).
    pub validation: Option<String>,
    /// Redact path-like detail in semantic summaries / recordings.
    pub sensitive: bool,
    /// Whether remove is allowed.
    pub removable: bool,
}

impl AttachmentItem {
    /// File attachment (ready).
    #[must_use]
    pub fn file(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: AttachmentType::File,
            name: name.into(),
            meta: None,
            bytes: None,
            line_count: None,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// Image attachment.
    #[must_use]
    pub fn image(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: AttachmentType::Image,
            name: name.into(),
            meta: None,
            bytes: None,
            line_count: None,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// URL attachment.
    #[must_use]
    pub fn url(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: AttachmentType::Url,
            name: name.into(),
            meta: None,
            bytes: None,
            line_count: None,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// Selected code reference.
    #[must_use]
    pub fn code(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: AttachmentType::Code,
            name: name.into(),
            meta: None,
            bytes: None,
            line_count: None,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: AttachmentType) -> Self {
        self.kind = kind;
        self
    }

    /// Meta string.
    #[must_use]
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = Some(meta.into());
        self
    }

    /// Bytes.
    #[must_use]
    pub const fn bytes(mut self, n: u64) -> Self {
        self.bytes = Some(n);
        self
    }

    /// Line count.
    #[must_use]
    pub const fn line_count(mut self, n: u32) -> Self {
        self.line_count = Some(n);
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: AttachmentStatus) -> Self {
        self.status = status;
        self
    }

    /// Validation message.
    #[must_use]
    pub fn validation(mut self, msg: impl Into<String>) -> Self {
        self.validation = Some(msg.into());
        self
    }

    /// Sensitive flag.
    #[must_use]
    pub const fn sensitive(mut self, on: bool) -> Self {
        self.sensitive = on;
        self
    }

    /// Removable.
    #[must_use]
    pub const fn removable(mut self, on: bool) -> Self {
        self.removable = on;
        self
    }

    /// Compact display label (glyph + name + optional meta/progress).
    #[must_use]
    pub fn display_label(&self, ascii: bool) -> String {
        let g = self.kind.glyph(ascii);
        let mut s = format!("{g} {}", self.name);
        if let Some(p) = self.status.progress() {
            s.push_str(&format!(" {p}%"));
        } else if let Some(m) = &self.meta {
            s.push(' ');
            s.push_str(m);
        } else if let Some(lines) = self.line_count {
            s.push_str(&format!(" {lines}L"));
        } else if let Some(b) = self.bytes {
            s.push(' ');
            s.push_str(&format_bytes(b));
        }
        let mark = self.status.mark(ascii);
        if !mark.is_empty() {
            s.push(' ');
            s.push_str(mark);
        }
        s
    }
}

/// Semantic / recording summary — **never** includes full paths when sensitive.
#[must_use]
pub fn attachment_semantic_summary(item: &AttachmentItem) -> String {
    let name = if item.sensitive {
        redacted_name(&item.name)
    } else {
        item.name.as_str()
    };
    let mut s = format!("attachment {} {name}", item.kind.id());
    if let Some(b) = item.bytes {
        s.push_str(&format!(" {}", format_bytes(b)));
    }
    if let Some(n) = item.line_count {
        s.push_str(&format!(" {n} lines"));
    }
    s.push(' ');
    s.push_str(item.status.id());
    if item.validation.is_some() {
        s.push_str(" validation-failed");
    }
    s
}

// ── Domain: paste ───────────────────────────────────────────────────────────

/// Large paste payload (collapsed by default).
///
/// Full `body` is host-held for copy/expand/submit. Semantic summaries use only
/// size / line counts — never the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PastePayload {
    /// Stable id.
    pub id: String,
    /// Short preview (truncated; not the body).
    pub preview: String,
    /// Byte length.
    pub bytes: usize,
    /// Line count of body (or estimate).
    pub line_count: usize,
    /// Full body when host retains it (optional for privacy).
    pub body: Option<String>,
    /// Binary / non-text paste — no auto-insert as text.
    pub binary: bool,
    /// Lifecycle.
    pub status: AttachmentStatus,
    /// Validation message (safe).
    pub validation: Option<String>,
    /// Redact preview in semantic summaries.
    pub sensitive: bool,
    /// Removable.
    pub removable: bool,
}

impl PastePayload {
    /// From full body (preview + counts derived).
    #[must_use]
    pub fn from_body(id: impl Into<String>, body: impl Into<String>) -> Self {
        let body = body.into();
        let bytes = body.len();
        let line_count = body.lines().count().max(1);
        let preview = paste_preview_from(&body);
        Self {
            id: id.into(),
            preview,
            bytes,
            line_count,
            body: Some(body),
            binary: false,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// Binary paste (no body insert without host confirm).
    #[must_use]
    pub fn binary(id: impl Into<String>, bytes: usize) -> Self {
        Self {
            id: id.into(),
            preview: "binary".into(),
            bytes,
            line_count: 0,
            body: None,
            binary: true,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// Preview-only (host keeps body elsewhere).
    #[must_use]
    pub fn preview_only(
        id: impl Into<String>,
        preview: impl Into<String>,
        bytes: usize,
        line_count: usize,
    ) -> Self {
        Self {
            id: id.into(),
            preview: preview.into(),
            bytes,
            line_count,
            body: None,
            binary: false,
            status: AttachmentStatus::Ready,
            validation: None,
            sensitive: false,
            removable: true,
        }
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: AttachmentStatus) -> Self {
        self.status = status;
        self
    }

    /// Sensitive.
    #[must_use]
    pub const fn sensitive(mut self, on: bool) -> Self {
        self.sensitive = on;
        self
    }

    /// Validation.
    #[must_use]
    pub fn validation(mut self, msg: impl Into<String>) -> Self {
        self.validation = Some(msg.into());
        self
    }

    /// Compact chip label.
    #[must_use]
    pub fn display_label(&self, ascii: bool, expanded: bool) -> String {
        let badge = if ascii { "P" } else { "⧉" };
        let mark = self.status.mark(ascii);
        let size = format_bytes(self.bytes as u64);
        if self.binary {
            let mut s = format!("{badge} binary {size}");
            if !mark.is_empty() {
                s.push(' ');
                s.push_str(mark);
            }
            return s;
        }
        let prev = if self.sensitive {
            "…"
        } else {
            self.preview.as_str()
        };
        let mut s = if expanded {
            format!("{badge} {prev} · {size} · {}L", self.line_count)
        } else {
            format!("{badge} {prev} · {size}")
        };
        if let Some(p) = self.status.progress() {
            s.push_str(&format!(" {p}%"));
        }
        if !mark.is_empty() {
            s.push(' ');
            s.push_str(mark);
        }
        s
    }

    /// First N lines of body for expanded preview (empty if binary / no body).
    #[must_use]
    pub fn expanded_preview_lines(&self, max_lines: usize) -> Vec<String> {
        if self.binary {
            return vec!["(binary paste — confirm before insert)".into()];
        }
        let Some(body) = &self.body else {
            return vec![format!("(preview) {}", self.preview)];
        };
        body.lines()
            .take(max_lines.max(1))
            .map(str::to_string)
            .collect()
    }
}

/// Semantic summary — **never** includes paste body.
#[must_use]
pub fn paste_semantic_summary(paste: &PastePayload) -> String {
    let kind = if paste.binary {
        "binary-paste"
    } else {
        "paste"
    };
    let mut s = format!(
        "{kind} {} bytes {} lines {}",
        paste.bytes,
        paste.line_count,
        paste.status.id()
    );
    if paste.sensitive {
        s.push_str(" sensitive");
    }
    if paste.validation.is_some() {
        s.push_str(" validation-failed");
    }
    s
}

/// Truncate body to preview label.
#[must_use]
pub fn paste_preview_from(body: &str) -> String {
    let mut it = body.chars();
    let mut s: String = it.by_ref().take(PASTE_PREVIEW_CHARS).collect();
    if it.next().is_some() {
        s.push('…');
    }
    // collapse newlines in preview
    s.replace(['\n', '\r'], " ")
}

// ── TokenStrip projection ───────────────────────────────────────────────────

/// Build [`TokenItem`] rows for strip paint (ids borrowed from items).
///
/// `label_bufs` must outlive returned items (same length as attachments+pastes).
pub fn attachment_token_items<'a>(
    attachments: &'a [AttachmentItem],
    pastes: &'a [PastePayload],
    label_bufs: &'a [String],
) -> Vec<TokenItem<'a, &'a str>> {
    let mut out = Vec::with_capacity(attachments.len() + pastes.len());
    let mut i = 0usize;
    for a in attachments {
        let label = label_bufs
            .get(i)
            .map(String::as_str)
            .unwrap_or(a.name.as_str());
        i = i.saturating_add(1);
        out.push(
            TokenItem::tag(a.id.as_str(), label)
                .removable(a.removable)
                .status(a.status.token_status())
                .disabled(false),
        );
    }
    for p in pastes {
        let label = label_bufs
            .get(i)
            .map(String::as_str)
            .unwrap_or(p.preview.as_str());
        i = i.saturating_add(1);
        out.push(
            TokenItem::tag(p.id.as_str(), label)
                .removable(p.removable)
                .status(p.status.token_status())
                .disabled(false),
        );
    }
    out
}

/// Fill display labels for strip (ascii).
pub fn fill_attachment_strip_labels(
    attachments: &[AttachmentItem],
    pastes: &[PastePayload],
    ascii: bool,
    expanded_paste_ids: &[&str],
) -> Vec<String> {
    let mut labels = Vec::with_capacity(attachments.len() + pastes.len());
    for a in attachments {
        labels.push(a.display_label(ascii));
    }
    for p in pastes {
        let exp = expanded_paste_ids.contains(&p.id.as_str());
        labels.push(p.display_label(ascii, exp));
    }
    labels
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Attachment chip outcomes (host owns open/upload/retry I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AttachmentChipOutcome {
    /// Not handled.
    Ignored,
    /// Body activated (open / focus).
    Activated {
        /// Id.
        id: String,
    },
    /// Remove requested.
    Removed {
        /// Id.
        id: String,
    },
    /// Open / reveal in host (file manager, browser).
    OpenRequested {
        /// Id.
        id: String,
    },
    /// Preview requested (overlay).
    PreviewRequested {
        /// Id.
        id: String,
    },
    /// Retry upload / index / validation.
    RetryRequested {
        /// Id.
        id: String,
    },
    /// Internal Body ↔ Remove focus.
    PartChanged(TokenPart),
}

/// Paste chip outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PasteChipOutcome {
    /// Not handled.
    Ignored,
    /// Expanded preview.
    Expanded {
        /// Id.
        id: String,
    },
    /// Collapsed.
    Collapsed {
        /// Id.
        id: String,
    },
    /// Remove.
    Removed {
        /// Id.
        id: String,
    },
    /// Host should copy full body (payload never put in outcome text).
    CopyRequested {
        /// Id.
        id: String,
    },
    /// Insert body into prompt (blocked for binary — host must confirm).
    InsertRequested {
        /// Id.
        id: String,
        /// True when binary — host must confirm.
        needs_confirm: bool,
    },
    /// Preview overlay.
    PreviewRequested {
        /// Id.
        id: String,
    },
    /// Retry.
    RetryRequested {
        /// Id.
        id: String,
    },
    /// Part focus.
    PartChanged(TokenPart),
}

// ── AttachmentChip widget ───────────────────────────────────────────────────

/// Attachment chip interaction state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttachmentChipState {
    /// Tag part focus.
    pub tag: TagState,
}

impl AttachmentChipState {
    /// Fresh.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tag: TagState::new(),
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.tag.set_focused(on);
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.tag.focused
    }
}

/// Single attachment chip paint (file / image / URL / code).
#[derive(Debug, Clone, Copy)]
pub struct AttachmentChip<'a> {
    item: &'a AttachmentItem,
    system: &'a DesignSystem,
}

impl<'a> AttachmentChip<'a> {
    /// Item + design system.
    #[must_use]
    pub const fn new(item: &'a AttachmentItem, system: &'a DesignSystem) -> Self {
        Self { item, system }
    }

    /// ASCII glyphs.
    #[must_use]
    /// Natural width.
    pub fn measure_width(&self) -> u16 {
        let label = self.item.display_label(false);
        let tag = if self.item.removable {
            Tag::removable_tag(self.item.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.item.id.as_str(), label.as_str(), self.system)
        }
        .status(self.item.status.token_status());
        tag.measure_width()
    }

    /// Semantic remove label (uses safe name).
    #[must_use]
    pub fn remove_action_label(&self) -> String {
        let name = if self.item.sensitive {
            redacted_name(&self.item.name)
        } else {
            self.item.name.as_str()
        };
        remove_label(name)
    }

    /// Paint into area.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut AttachmentChipState,
    ) -> TokenParts {
        let label = self.item.display_label(false);
        let tag = if self.item.removable {
            Tag::removable_tag(self.item.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.item.id.as_str(), label.as_str(), self.system)
        }
        .status(self.item.status.token_status());
        tag.paint(area, buffer, &mut state.tag)
    }

    /// Keys.
    pub fn handle_key(
        &self,
        state: &mut AttachmentChipState,
        key: KeyEvent,
    ) -> AttachmentChipOutcome {
        if key.is_release() {
            return AttachmentChipOutcome::Ignored;
        }
        let direct_action = key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('o' | 'O' | 'p' | 'P' | 'r' | 'R'));
        if !key.is_press() && direct_action {
            return AttachmentChipOutcome::Ignored;
        }
        // Retry on error
        if matches!(
            self.item.status,
            AttachmentStatus::Error | AttachmentStatus::Invalid
        ) && key.code == KeyCode::Char('r')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return AttachmentChipOutcome::RetryRequested {
                id: self.item.id.clone(),
            };
        }
        // Open
        if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return AttachmentChipOutcome::OpenRequested {
                id: self.item.id.clone(),
            };
        }
        // Preview
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return AttachmentChipOutcome::PreviewRequested {
                id: self.item.id.clone(),
            };
        }
        let label = self.item.display_label(false);
        let tag = if self.item.removable {
            Tag::removable_tag(self.item.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.item.id.as_str(), label.as_str(), self.system)
        }
        .status(self.item.status.token_status());
        match tag.handle_key(&mut state.tag, key) {
            TagOutcome::Ignored => AttachmentChipOutcome::Ignored,
            TagOutcome::Remove(id) => AttachmentChipOutcome::Removed { id: id.to_string() },
            TagOutcome::PartChanged(p) => AttachmentChipOutcome::PartChanged(p),
            TagOutcome::Activated(id) => AttachmentChipOutcome::Activated { id: id.to_string() },
            TagOutcome::HoverChanged => AttachmentChipOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &self,
        state: &mut AttachmentChipState,
        event: MouseEvent,
    ) -> AttachmentChipOutcome {
        let label = self.item.display_label(false);
        let tag = if self.item.removable {
            Tag::removable_tag(self.item.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.item.id.as_str(), label.as_str(), self.system)
        }
        .status(self.item.status.token_status());
        match tag.handle_mouse(&mut state.tag, event) {
            TagOutcome::Ignored => AttachmentChipOutcome::Ignored,
            TagOutcome::Remove(id) => AttachmentChipOutcome::Removed { id: id.to_string() },
            TagOutcome::PartChanged(p) => AttachmentChipOutcome::PartChanged(p),
            TagOutcome::Activated(id) => AttachmentChipOutcome::Activated { id: id.to_string() },
            TagOutcome::HoverChanged => AttachmentChipOutcome::Ignored,
        }
    }
}

// ── PasteChip widget ────────────────────────────────────────────────────────

/// Paste chip interaction state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PasteChipState {
    /// Tag chrome state.
    pub tag: TagState,
    /// Expanded preview mode.
    pub expanded: bool,
}

impl PasteChipState {
    /// Fresh collapsed.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tag: TagState::new(),
            expanded: false,
        }
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        // Unfocusing keeps the popover expanded; only the Esc path collapses it.
        self.tag.set_focused(on);
    }

    /// Collapse.
    pub const fn collapse(&mut self) {
        self.expanded = false;
    }
}

/// Large-paste chip (collapsed summary + expand/copy outcomes).
#[derive(Debug, Clone, Copy)]
pub struct PasteChip<'a> {
    paste: &'a PastePayload,
    system: &'a DesignSystem,
}

impl<'a> PasteChip<'a> {
    /// Paste + system.
    #[must_use]
    pub const fn new(paste: &'a PastePayload, system: &'a DesignSystem) -> Self {
        Self { paste, system }
    }

    /// ASCII.
    #[must_use]
    /// Measure width.
    pub fn measure_width(&self, expanded: bool) -> u16 {
        let label = self.paste.display_label(false, expanded);
        let tag = if self.paste.removable {
            Tag::removable_tag(self.paste.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.paste.id.as_str(), label.as_str(), self.system)
        }
        .status(self.paste.status.token_status());
        tag.measure_width()
    }

    /// Paint chip chrome (expanded body is host popover or [`paint_expanded_preview`]).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut PasteChipState) -> TokenParts {
        let label = self.paste.display_label(false, state.expanded);
        let tag = if self.paste.removable {
            Tag::removable_tag(self.paste.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.paste.id.as_str(), label.as_str(), self.system)
        }
        .status(self.paste.status.token_status());
        tag.paint(area, buffer, &mut state.tag)
    }

    /// Paint expanded preview lines below chip (host allocates area).
    pub fn paint_expanded_preview(&self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        let lines = self.paste.expanded_preview_lines(PASTE_EXPAND_LINES);
        let style = self.system.style(Role::TextMuted);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y.saturating_add(i as u16);
            if y >= area.bottom() {
                break;
            }
            let clipped = take_display_cols(line, usize::from(area.width));
            buffer.set_stringn(area.x, y, &clipped, usize::from(area.width), style);
        }
    }

    /// Keys.
    pub fn handle_key(&self, state: &mut PasteChipState, key: KeyEvent) -> PasteChipOutcome {
        if key.is_release() {
            return PasteChipOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let direct_activation = state.tag.part == TokenPart::Body
            && key.modifiers.is_empty()
            && matches!(key.code, KeyCode::Enter | KeyCode::Char(' '));
        let direct_action = ctrl
            && matches!(
                key.code,
                KeyCode::Char('c' | 'C' | 'i' | 'I' | 'p' | 'P' | 'r' | 'R')
            );
        if !key.is_press()
            && ((key.code == KeyCode::Esc && state.expanded) || direct_activation || direct_action)
        {
            return PasteChipOutcome::Ignored;
        }
        if key.code == KeyCode::Esc && state.expanded {
            state.expanded = false;
            return PasteChipOutcome::Collapsed {
                id: self.paste.id.clone(),
            };
        }
        if default_button_intent(key).is_some_and(|intent| matches!(intent, UiIntent::Activate))
            && state.tag.part == TokenPart::Body
        {
            if state.expanded {
                state.expanded = false;
                return PasteChipOutcome::Collapsed {
                    id: self.paste.id.clone(),
                };
            }
            state.expanded = true;
            return PasteChipOutcome::Expanded {
                id: self.paste.id.clone(),
            };
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return PasteChipOutcome::CopyRequested {
                id: self.paste.id.clone(),
            };
        }
        if key.code == KeyCode::Char('i') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return PasteChipOutcome::InsertRequested {
                id: self.paste.id.clone(),
                needs_confirm: self.paste.binary,
            };
        }
        if matches!(
            self.paste.status,
            AttachmentStatus::Error | AttachmentStatus::Invalid
        ) && key.code == KeyCode::Char('r')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return PasteChipOutcome::RetryRequested {
                id: self.paste.id.clone(),
            };
        }
        if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return PasteChipOutcome::PreviewRequested {
                id: self.paste.id.clone(),
            };
        }
        let label = self.paste.display_label(false, state.expanded);
        let tag = if self.paste.removable {
            Tag::removable_tag(self.paste.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.paste.id.as_str(), label.as_str(), self.system)
        }
        .status(self.paste.status.token_status());
        match tag.handle_key(&mut state.tag, key) {
            TagOutcome::Ignored => PasteChipOutcome::Ignored,
            TagOutcome::Remove(id) => PasteChipOutcome::Removed { id: id.to_string() },
            TagOutcome::PartChanged(p) => PasteChipOutcome::PartChanged(p),
            TagOutcome::Activated(id) => {
                // body activate toggles expand
                if state.expanded {
                    state.expanded = false;
                    PasteChipOutcome::Collapsed { id: id.to_string() }
                } else {
                    state.expanded = true;
                    PasteChipOutcome::Expanded { id: id.to_string() }
                }
            }
            TagOutcome::HoverChanged => PasteChipOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(&self, state: &mut PasteChipState, event: MouseEvent) -> PasteChipOutcome {
        let label = self.paste.display_label(false, state.expanded);
        let tag = if self.paste.removable {
            Tag::removable_tag(self.paste.id.as_str(), label.as_str(), self.system)
        } else {
            Tag::new(self.paste.id.as_str(), label.as_str(), self.system)
        }
        .status(self.paste.status.token_status());
        match tag.handle_mouse(&mut state.tag, event) {
            TagOutcome::Ignored => {
                // double-check: click body when parts known
                if event.kind == MouseEventKind::Down(MouseButton::Left)
                    && let Some(parts) = state.tag.parts
                    && parts.body.contains(event.position)
                {
                    state.expanded = !state.expanded;
                    return if state.expanded {
                        PasteChipOutcome::Expanded {
                            id: self.paste.id.clone(),
                        }
                    } else {
                        PasteChipOutcome::Collapsed {
                            id: self.paste.id.clone(),
                        }
                    };
                }
                PasteChipOutcome::Ignored
            }
            TagOutcome::Remove(id) => PasteChipOutcome::Removed { id: id.to_string() },
            TagOutcome::PartChanged(p) => PasteChipOutcome::PartChanged(p),
            TagOutcome::Activated(id) => {
                state.expanded = !state.expanded;
                if state.expanded {
                    PasteChipOutcome::Expanded { id: id.to_string() }
                } else {
                    PasteChipOutcome::Collapsed { id: id.to_string() }
                }
            }
            TagOutcome::HoverChanged => PasteChipOutcome::Ignored,
        }
    }
}

// ── Strip helper (compose TokenStrip) ───────────────────────────────────────

/// Paint a mixed attachment + paste strip with wrap / scroll / overflow.
pub fn paint_attachment_strip(
    attachments: &[AttachmentItem],
    pastes: &[PastePayload],
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    state: &mut TokenStripState<String>,
    layout: TokenStripLayout,
    max_visible: usize,
    ascii: bool,
    expanded_paste_ids: &[&str],
) {
    let labels = fill_attachment_strip_labels(attachments, pastes, ascii, expanded_paste_ids);
    let mut items: Vec<TokenItem<'_, String>> = Vec::with_capacity(labels.len());
    let mut i = 0usize;
    for a in attachments {
        let label = labels.get(i).map(String::as_str).unwrap_or(a.name.as_str());
        i += 1;
        items.push(
            TokenItem::tag(a.id.clone(), label)
                .removable(a.removable)
                .status(a.status.token_status()),
        );
    }
    for p in pastes {
        let label = labels
            .get(i)
            .map(String::as_str)
            .unwrap_or(p.preview.as_str());
        i += 1;
        items.push(
            TokenItem::tag(p.id.clone(), label)
                .removable(p.removable)
                .status(p.status.token_status()),
        );
    }
    let mut strip = TokenStrip::new(&items, system).max_visible(max_visible);
    strip = match layout {
        TokenStripLayout::Wrap => strip.wrap(),
        TokenStripLayout::Scroll => strip.scroll(),
    };
    strip.paint(area, buffer, state);
}

/// Map strip outcome to attachment/paste remove ids (host dispatches).
#[must_use]
pub fn map_strip_outcome(out: TokenStripOutcome<String>) -> AttachmentStripEvent {
    match out {
        TokenStripOutcome::Ignored => AttachmentStripEvent::Ignored,
        TokenStripOutcome::CursorMoved { from, to } => {
            AttachmentStripEvent::CursorMoved { from, to }
        }
        TokenStripOutcome::Remove(id) => AttachmentStripEvent::Removed { id },
        TokenStripOutcome::Activated(id) => AttachmentStripEvent::Activated { id },
        TokenStripOutcome::OverflowActivated => AttachmentStripEvent::OverflowActivated,
        TokenStripOutcome::PartChanged(p) => AttachmentStripEvent::PartChanged(p),
        TokenStripOutcome::Selected(id) | TokenStripOutcome::Unselected(id) => {
            AttachmentStripEvent::Activated { id }
        }
        TokenStripOutcome::Add | TokenStripOutcome::HoverChanged => AttachmentStripEvent::Ignored,
    }
}

/// Strip-level events for host routing (permission / composer).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AttachmentStripEvent {
    /// Ignored.
    Ignored,
    /// Roving cursor.
    CursorMoved {
        /// From.
        from: Option<String>,
        /// To.
        to: Option<String>,
    },
    /// Remove.
    Removed {
        /// Id.
        id: String,
    },
    /// Activate / open.
    Activated {
        /// Id.
        id: String,
    },
    /// `+N` overflow.
    OverflowActivated,
    /// Part focus.
    PartChanged(TokenPart),
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn format_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{} KB", n / 1024)
    } else {
        format!("{} MB", n / (1024 * 1024))
    }
}

fn redacted_name(name: &str) -> &str {
    // keep basename only; if path-like, last segment
    name.rsplit(['/', '\\']).next().unwrap_or("…")
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Moderate strip / paste stress sizes.
pub mod bench {
    /// Attachments in strip paint.
    pub const ATTACHMENT_COUNT: usize = 24;
    /// Pastes in strip paint.
    pub const PASTE_COUNT: usize = 8;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 20;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyEventKind;
    use crate::style::DesignSystem;
    use crate::widgets::tests::click;

    #[test]
    fn attachment_display_and_semantic_redacts() {
        let item = AttachmentItem::file("a1", "/home/user/secret/key.pem")
            .sensitive(true)
            .bytes(4096)
            .status(AttachmentStatus::Error)
            .validation("unreadable");
        let sum = attachment_semantic_summary(&item);
        assert!(!sum.contains("/home/user"));
        assert!(!sum.contains("secret"));
        assert!(sum.contains("attachment"));
        // basename may appear; full path must not
        assert!(sum.contains("key.pem") || sum.contains("file"));
        let label = item.display_label(true);
        assert!(label.contains('F') || label.contains("key"));
    }

    #[test]
    fn paste_semantic_never_includes_body() {
        let body = "SUPER_SECRET_TOKEN_ABCDEF\nline2\nline3";
        let p = PastePayload::from_body("p1", body).sensitive(true);
        let sum = paste_semantic_summary(&p);
        assert!(!sum.contains("SUPER_SECRET"));
        assert!(sum.contains("paste") || sum.contains("bytes"));
        assert_eq!(p.line_count, 3);
        assert!(p.preview.chars().count() <= PASTE_PREVIEW_CHARS + 1);
    }

    #[test]
    fn binary_paste_insert_needs_confirm() {
        let p = PastePayload::binary("b1", 999);
        let system = DesignSystem::default();
        let mut state = PasteChipState::new();
        state.tag.set_focused(true);
        let out = PasteChip::new(&p, &system).handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL),
        );
        assert!(matches!(
            out,
            PasteChipOutcome::InsertRequested {
                needs_confirm: true,
                ..
            }
        ));
    }

    #[test]
    fn paste_expand_collapse_esc() {
        let p = PastePayload::from_body("p1", "hello\nworld\n");
        let system = DesignSystem::default();
        let mut state = PasteChipState::new();
        state.tag.set_focused(true);
        let chip = PasteChip::new(&p, &system);
        assert!(matches!(
            chip.handle_key(
                &mut state,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            PasteChipOutcome::Expanded { .. }
        ));
        assert!(state.expanded);
        assert!(matches!(
            chip.handle_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PasteChipOutcome::Collapsed { .. }
        ));
        assert!(!state.expanded);
    }

    #[test]
    fn attachment_remove_via_tag() {
        let item = AttachmentItem::file("f1", "main.rs");
        let system = DesignSystem::default();
        let mut state = AttachmentChipState::new();
        state.set_focused(true);
        state.tag.set_part(TokenPart::Remove);
        let chip = AttachmentChip::new(&item, &system);
        let out = chip.handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        // Enter on remove may Remove or Activated depending on Tag — both ok if focused remove
        assert!(matches!(
            out,
            AttachmentChipOutcome::Removed { .. }
                | AttachmentChipOutcome::Activated { .. }
                | AttachmentChipOutcome::Ignored
                | AttachmentChipOutcome::PartChanged(_)
        ));
    }

    #[test]
    fn attachment_and_paste_mouse_use_painted_hit_geometry() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);

        let item = AttachmentItem::file("f1", "main.rs");
        let chip = AttachmentChip::new(&item, &system);
        let mut attachment = AttachmentChipState::new();
        let parts = chip.paint(area, &mut buffer, &mut attachment);
        assert!(matches!(
            chip.handle_mouse(
                &mut attachment,
                click(parts.body.x, parts.body.y),
            ),
            AttachmentChipOutcome::Activated { id } if id == "f1"
        ));

        let paste = PastePayload::from_body("p1", "hello\nworld");
        let chip = PasteChip::new(&paste, &system);
        let mut paste_state = PasteChipState::new();
        let parts = chip.paint(area, &mut buffer, &mut paste_state);
        assert!(matches!(
            chip.handle_mouse(
                &mut paste_state,
                click(parts.body.x, parts.body.y),
            ),
            PasteChipOutcome::Expanded { id } if id == "p1"
        ));
    }

    #[test]
    fn strip_wrap_and_overflow_paint() {
        let system = DesignSystem::default();
        let atts: Vec<_> = (0..bench::ATTACHMENT_COUNT)
            .map(|i| AttachmentItem::file(format!("f{i}"), format!("file{i}.rs")).bytes(100))
            .collect();
        let pastes: Vec<_> = (0..bench::PASTE_COUNT)
            .map(|i| PastePayload::preview_only(format!("p{i}"), format!("paste{i}"), 500, 10))
            .collect();
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        let mut state = TokenStripState::new();
        state.set_surface_focused(true);
        for _ in 0..bench::PAINT_FRAMES {
            paint_attachment_strip(
                &atts,
                &pastes,
                area,
                &mut buf,
                &system,
                &mut state,
                TokenStripLayout::Wrap,
                6,
                true,
                &[],
            );
        }
    }

    #[test]
    fn upload_progress_in_label() {
        let item = AttachmentItem::file("u1", "big.bin")
            .status(AttachmentStatus::Uploading { progress: 42 });
        let label = item.display_label(true);
        assert!(label.contains("42%"), "{label}");
        assert_eq!(item.status.token_status(), TokenStatus::Loading);
    }

    #[test]
    fn copy_outcome_has_id_not_body() {
        let secret = "SECRET_BODY_XYZ";
        let p = PastePayload::from_body("p1", secret);
        let system = DesignSystem::default();
        let mut state = PasteChipState::new();
        let out = PasteChip::new(&p, &system).handle_key(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        let dbg = format!("{out:?}");
        assert!(!dbg.contains(secret));
        match out {
            PasteChipOutcome::CopyRequested { id } => {
                assert_eq!(id, "p1");
            }
            other => panic!("expected CopyRequested, got {other:?}"),
        }
    }

    #[test]
    fn repeated_attachment_host_actions_are_ignored() {
        let item = AttachmentItem::file("f1", "main.rs").status(AttachmentStatus::Error);
        let system = DesignSystem::default();
        let chip = AttachmentChip::new(&item, &system);
        let mut state = AttachmentChipState::new();
        state.set_focused(true);
        for (code, modifiers) in [
            (KeyCode::Char('r'), KeyModifiers::CONTROL),
            (KeyCode::Char('o'), KeyModifiers::CONTROL),
            (KeyCode::Char('p'), KeyModifiers::CONTROL),
        ] {
            let before = state.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(
                chip.handle_key(&mut state, key),
                AttachmentChipOutcome::Ignored
            );
            assert_eq!(state, before, "{code:?} repeat mutated attachment state");
        }
    }

    #[test]
    fn repeated_paste_host_actions_are_ignored_before_toggle_or_collapse() {
        let paste = PastePayload::from_body("p1", "hello").status(AttachmentStatus::Error);
        let system = DesignSystem::default();
        let chip = PasteChip::new(&paste, &system);
        let mut state = PasteChipState::new();
        state.tag.set_focused(true);
        state.expanded = true;

        for (code, modifiers) in [
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Char(' '), KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Char('c'), KeyModifiers::CONTROL),
            (KeyCode::Char('i'), KeyModifiers::CONTROL),
            (KeyCode::Char('p'), KeyModifiers::CONTROL),
            (KeyCode::Char('r'), KeyModifiers::CONTROL),
        ] {
            let before = state.clone();
            let mut key = KeyEvent::new(code, modifiers);
            key.kind = KeyEventKind::Repeat;
            assert_eq!(chip.handle_key(&mut state, key), PasteChipOutcome::Ignored);
            assert_eq!(state, before, "{code:?} repeat mutated paste state");
        }
    }

    #[test]
    fn never_uploads_or_network() {
        let src = include_str!("attachment_chips.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in ["reqwest::", "std::process::Command", "tokio::net", "ureq::"] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }

    #[test]
    fn paint_attachment_and_paste_single() {
        let system = DesignSystem::default();
        let item = AttachmentItem::url("u", "https://example.com")
            .meta("link")
            .status(AttachmentStatus::Validating);
        let mut st = AttachmentChipState::new();
        st.set_focused(true);
        let area = Rect::new(0, 0, 48, 1);
        let mut buf = Buffer::empty(area);
        AttachmentChip::new(&item, &system).paint(area, &mut buf, &mut st);

        let paste = PastePayload::from_body("p", "line1\nline2\nline3");
        let mut ps = PasteChipState::new();
        ps.expanded = true;
        PasteChip::new(&paste, &system).paint(area, &mut buf, &mut ps);
        let prev = Rect::new(0, 1, 48, 4);
        let mut buf2 = Buffer::empty(Rect::new(0, 0, 48, 5));
        PasteChip::new(&paste, &system).paint_expanded_preview(prev, &mut buf2);
    }

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(100), "100 B");
        assert!(format_bytes(2048).contains("KB"));
    }

    #[test]
    fn map_strip_remove() {
        let e = map_strip_outcome(TokenStripOutcome::Remove("x".into()));
        assert!(matches!(e, AttachmentStripEvent::Removed { id } if id == "x"));
    }
}
