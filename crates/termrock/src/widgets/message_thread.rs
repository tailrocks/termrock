// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **MessageThread** — virtualized conversation / activity transcript for
//! long-running agent sessions.
//!
//! **Mission.** User, assistant, system, tool, status, compact event, and error
//! entries; stable anchors; grouping; timestamps; actors; actions; selection;
//! copy; and search. Preserve reading position while streaming; show a
//! new-content indicator when not following tail. Collapse tool/activity
//! entries and support semantic zoom. Virtualize long sessions and compact old
//! content without losing checkpoints. **Avoid chat-bubble web imitation** —
//! editorial lines with kind prefixes (project-to-lines over
//! [`Transcript`](crate::widgets::Transcript)).
//!
//! Research: Amp, OpenCode, Grok Build, Claude Code, editorial Markdown TUIs.
//!
//! **v1 law (AD-1):** nested StatefulWidgets inside the viewport are out of
//! scope. Host expands tools / opens overlays on [`MessageThreadOutcome::Activated`].

use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent},
    style::DesignSystem,
    text::{display_cols, take_display_cols},
    widgets::{
        transcript::{
            Transcript, TranscriptBlock, TranscriptKind, TranscriptOutcome, TranscriptState,
        },
    },
};

/// Max body lines projected when expanded (host may open fullscreen for more).
pub const MESSAGE_THREAD_EXPAND_LINE_CAP: usize = 12;
/// Default compact zoom keeps this many body lines for non-compact kinds.
pub const MESSAGE_THREAD_COMPACT_BODY_LINES: usize = 2;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Semantic entry kind (maps to [`TranscriptKind`] chrome).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MessageKind {
    /// User turn.
    #[default]
    User,
    /// Assistant turn.
    Assistant,
    /// System notice.
    System,
    /// Tool call / result.
    Tool,
    /// Lightweight status line (run state, queue).
    Status,
    /// Compact activity event (collapsed by default at Summary zoom).
    Event,
    /// Error / failed step.
    Error,
    /// Thinking / reasoning fold.
    Thinking,
}

impl MessageKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
            Self::Status => "status",
            Self::Event => "event",
            Self::Error => "error",
            Self::Thinking => "thinking",
        }
    }

    /// Map to transcript kind chrome.
    #[must_use]
    pub const fn transcript_kind(self) -> TranscriptKind {
        match self {
            Self::User => TranscriptKind::User,
            Self::Assistant => TranscriptKind::Assistant,
            Self::System | Self::Status | Self::Event => TranscriptKind::System,
            Self::Tool => TranscriptKind::Tool,
            Self::Error => TranscriptKind::Approval,
            Self::Thinking => TranscriptKind::Thinking,
        }
    }

    /// Default collapsed at Summary zoom.
    #[must_use]
    pub const fn default_collapsed_at_summary(self) -> bool {
        matches!(self, Self::Tool | Self::Event | Self::Thinking | Self::Status)
    }
}

/// Semantic zoom level (how much body is projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum MessageZoom {
    /// One-line summaries; tools/events folded.
    Compact,
    /// Short bodies; tools folded unless expanded by host.
    #[default]
    Summary,
    /// Full projected bodies (capped).
    Full,
}

impl MessageZoom {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Summary => "summary",
            Self::Full => "full",
        }
    }

    /// Cycle Compact → Summary → Full → Compact.
    #[must_use]
    pub const fn cycle(self) -> Self {
        match self {
            Self::Compact => Self::Summary,
            Self::Summary => Self::Full,
            Self::Full => Self::Compact,
        }
    }
}

/// Actor label for a turn (host-projected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageActor {
    /// Stable actor id.
    pub id: String,
    /// Display name.
    pub label: String,
}

impl MessageActor {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Action available when an entry is selected (host keymap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageAction {
    /// Action id (`copy`, `retry`, `cancel`, …).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional chord hint.
    pub chord: Option<String>,
}

impl MessageAction {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            chord: None,
        }
    }

    /// Chord hint.
    #[must_use]
    pub fn chord(mut self, c: impl Into<String>) -> Self {
        self.chord = Some(c.into());
        self
    }
}

/// One conversation / activity entry (host owns payload; lines are projected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageEntry {
    /// Stable identity across stream patches.
    pub id: String,
    /// Kind.
    pub kind: MessageKind,
    /// Content revision (bump on stream patch to remeasure).
    pub revision: u64,
    /// Actor (user name, agent, tool name).
    pub actor: Option<MessageActor>,
    /// Timestamp display string (host formats).
    pub timestamp: Option<String>,
    /// Group key for consecutive headers (session day, tool batch).
    pub group: Option<String>,
    /// Full body lines (host wraps for width).
    pub lines: Vec<String>,
    /// One-line summary when collapsed / compact zoom.
    pub summary: Option<String>,
    /// Host wants collapsed (tool/activity).
    pub collapsed: bool,
    /// Checkpoint — survives compaction of older content.
    pub checkpoint: bool,
    /// Searchable haystack (defaults to join of lines + actor).
    pub search_text: Option<String>,
    /// Status letter for tools/errors (`R` running, `E` error, …).
    pub status_letter: Option<char>,
    /// Actions when selected.
    pub actions: Vec<MessageAction>,
    /// Whether selectable / activatable.
    pub enabled: bool,
}

impl MessageEntry {
    /// Simple entry with body lines.
    #[must_use]
    pub fn new(id: impl Into<String>, kind: MessageKind, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            kind,
            revision: 0,
            actor: None,
            timestamp: None,
            group: None,
            lines: lines.into_iter().map(Into::into).collect(),
            summary: None,
            collapsed: false,
            checkpoint: false,
            search_text: None,
            status_letter: None,
            actions: Vec::new(),
            enabled: true,
        }
    }

    /// User message.
    #[must_use]
    pub fn user(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(id, MessageKind::User, [text.into()])
    }

    /// Assistant message.
    #[must_use]
    pub fn assistant(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(id, MessageKind::Assistant, [text.into()])
    }

    /// Tool entry (default collapsed at summary zoom).
    #[must_use]
    pub fn tool(id: impl Into<String>, summary: impl Into<String>) -> Self {
        let summary = summary.into();
        Self::new(id, MessageKind::Tool, [summary.clone()])
            .summary(summary)
            .collapsed(true)
            .status_letter('R')
    }

    /// Error entry.
    #[must_use]
    pub fn error(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(id, MessageKind::Error, [text.into()]).status_letter('E')
    }

    /// Compact event.
    #[must_use]
    pub fn event(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(id, MessageKind::Event, [text.into()]).collapsed(true)
    }

    /// Status line.
    #[must_use]
    pub fn status(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::new(id, MessageKind::Status, [text.into()])
    }

    /// Revision.
    #[must_use]
    pub const fn revision(mut self, r: u64) -> Self {
        self.revision = r;
        self
    }

    /// Actor.
    #[must_use]
    pub fn actor(mut self, a: MessageActor) -> Self {
        self.actor = Some(a);
        self
    }

    /// Timestamp.
    #[must_use]
    pub fn timestamp(mut self, t: impl Into<String>) -> Self {
        self.timestamp = Some(t.into());
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    /// Summary.
    #[must_use]
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }

    /// Collapsed.
    #[must_use]
    pub const fn collapsed(mut self, on: bool) -> Self {
        self.collapsed = on;
        self
    }

    /// Checkpoint.
    #[must_use]
    pub const fn checkpoint(mut self, on: bool) -> Self {
        self.checkpoint = on;
        self
    }

    /// Status letter.
    #[must_use]
    pub const fn status_letter(mut self, c: char) -> Self {
        self.status_letter = Some(c);
        self
    }

    /// Actions.
    #[must_use]
    pub fn actions(mut self, a: Vec<MessageAction>) -> Self {
        self.actions = a;
        self
    }

    /// Enabled.
    #[must_use]
    pub const fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Haystack for search.
    #[must_use]
    pub fn haystack(&self) -> String {
        if let Some(s) = &self.search_text {
            return s.clone();
        }
        let mut h = self.lines.join("\n");
        if let Some(a) = &self.actor {
            h.push(' ');
            h.push_str(&a.label);
        }
        if let Some(s) = &self.summary {
            h.push(' ');
            h.push_str(s);
        }
        h
    }
}

// ── Projection ──────────────────────────────────────────────────────────────

/// Owned line buffers + block metadata for one paint (host retains across frame).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedThread {
    /// Owned lines per entry (parallel to blocks).
    pub line_bufs: Vec<Vec<String>>,
    /// Pointer tables into `line_bufs` for TranscriptBlock::lines.
    pub line_refs: Vec<Vec<&'static str>>,
    /// Block shells (ids/kinds); lines filled after pinning refs — see
    /// [`build_transcript_blocks`].
    pub meta: Vec<ProjectedEntryMeta>,
}

/// Metadata for a projected entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedEntryMeta {
    /// Id.
    pub id: String,
    /// Kind.
    pub kind: MessageKind,
    /// Revision.
    pub revision: u64,
    /// Folded.
    pub folded: bool,
    /// Summary when folded.
    pub summary: Option<String>,
    /// Enabled.
    pub enabled: bool,
    /// Index into line_bufs.
    pub line_buf_index: usize,
}

/// Filter + project entries for the current zoom / search / collapse overrides.
#[must_use]
pub fn project_message_thread(
    entries: &[MessageEntry],
    zoom: MessageZoom,
    search: Option<&str>,
    expanded_ids: &BTreeSet<String>,
    force_collapsed: &BTreeSet<String>,
) -> (Vec<ProjectedEntryMeta>, Vec<Vec<String>>) {
    let q = search.map(|s| s.trim().to_ascii_lowercase()).filter(|s| !s.is_empty());
    let mut meta = Vec::new();
    let mut bufs = Vec::new();
    let mut last_group: Option<&str> = None;

    for e in entries {
        if let Some(ref qq) = q {
            if !e.haystack().to_ascii_lowercase().contains(qq) {
                continue;
            }
        }
        // group header as synthetic status line when group changes
        if let Some(g) = e.group.as_deref() {
            if last_group != Some(g) {
                let header = format!("── {g} ──");
                bufs.push(vec![header]);
                meta.push(ProjectedEntryMeta {
                    id: format!("grp:{}:{}", e.id, g),
                    kind: MessageKind::Status,
                    revision: 0,
                    folded: false,
                    summary: None,
                    enabled: false,
                    line_buf_index: bufs.len() - 1,
                });
                last_group = Some(g);
            }
        }

        let host_collapsed = e.collapsed || force_collapsed.contains(&e.id);
        let force_exp = expanded_ids.contains(&e.id);
        let folded = match zoom {
            MessageZoom::Compact => true,
            MessageZoom::Summary => {
                if force_exp {
                    false
                } else {
                    host_collapsed || e.kind.default_collapsed_at_summary()
                }
            }
            MessageZoom::Full => {
                if force_exp {
                    false
                } else {
                    host_collapsed && !e.checkpoint
                }
            }
        };

        let lines = project_entry_lines(e, zoom, folded);
        bufs.push(lines);
        meta.push(ProjectedEntryMeta {
            id: e.id.clone(),
            kind: e.kind,
            revision: e.revision,
            folded,
            summary: e.summary.clone().or_else(|| e.lines.first().cloned()),
            enabled: e.enabled,
            line_buf_index: bufs.len() - 1,
        });
    }
    (meta, bufs)
}

fn project_entry_lines(e: &MessageEntry, zoom: MessageZoom, folded: bool) -> Vec<String> {
    if folded {
        let mut s = String::new();
        if let Some(t) = &e.timestamp {
            s.push_str(t);
            s.push(' ');
        }
        if let Some(a) = &e.actor {
            s.push_str(&a.label);
            s.push(' ');
        }
        if let Some(c) = e.status_letter {
            s.push('[');
            s.push(c);
            s.push(']');
            s.push(' ');
        }
        let body = e
            .summary
            .as_deref()
            .or_else(|| e.lines.first().map(String::as_str))
            .unwrap_or("…");
        s.push_str(body);
        if e.checkpoint {
            s.push_str(" ◆");
        }
        return vec![s];
    }

    let mut out = Vec::new();
    // header line: actor · time · status
    let mut head = String::new();
    if let Some(a) = &e.actor {
        head.push_str(&a.label);
    }
    if let Some(t) = &e.timestamp {
        if !head.is_empty() {
            head.push_str(" · ");
        }
        head.push_str(t);
    }
    if let Some(c) = e.status_letter {
        if !head.is_empty() {
            head.push(' ');
        }
        head.push('[');
        head.push(c);
        head.push(']');
    }
    if e.checkpoint {
        if !head.is_empty() {
            head.push(' ');
        }
        head.push('◆');
    }
    let had_head = !head.is_empty();
    if had_head {
        out.push(head);
    }

    let cap = match zoom {
        MessageZoom::Compact => 1,
        MessageZoom::Summary => MESSAGE_THREAD_COMPACT_BODY_LINES,
        MessageZoom::Full => MESSAGE_THREAD_EXPAND_LINE_CAP,
    };
    let body_take = if had_head { cap } else { cap.max(1) };
    for line in e.lines.iter().take(body_take) {
        out.push(line.clone());
    }
    if e.lines.len() > body_take {
        out.push(format!("… +{} lines · open detail", e.lines.len() - body_take));
    }
    if out.is_empty() {
        out.push(e.summary.clone().unwrap_or_else(|| "…".into()));
    }
    out
}

/// Build transcript blocks borrowing from pinned line buffers.
///
/// # Safety
///
/// Callers must keep `bufs` alive for the lifetime of returned blocks. The
/// `'static` transmute is scoped to paint/frame only (same pattern as other
/// projection hosts in this crate). Prefer [`ThreadProjection`] which owns the
/// buffers.
pub fn build_transcript_blocks<'a>(
    meta: &'a [ProjectedEntryMeta],
    bufs: &'a [Vec<String>],
    line_ptrs: &'a mut Vec<Vec<&'a str>>,
) -> Vec<TranscriptBlock<'a, String>> {
    line_ptrs.clear();
    let mut blocks = Vec::with_capacity(meta.len());
    for m in meta {
        let lines = bufs.get(m.line_buf_index).map(Vec::as_slice).unwrap_or(&[]);
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        line_ptrs.push(refs);
    }
    for (i, m) in meta.iter().enumerate() {
        let refs = &line_ptrs[i];
        // reborrow as slice of refs
        let line_slice: &[&str] = refs.as_slice();
        let mut b = TranscriptBlock::new(m.id.clone(), m.kind.transcript_kind(), line_slice)
            .revision(m.revision)
            .folded(m.folded)
            .enabled(m.enabled);
        if let Some(s) = &m.summary {
            // summary lives in meta — need 'a — use first line of bufs as summary if folded
            let _ = s;
        }
        if m.folded {
            if let Some(first) = bufs.get(m.line_buf_index).and_then(|v| v.first()) {
                b = b.summary(first.as_str());
            }
        }
        blocks.push(b);
    }
    blocks
}

/// Owns projection for a frame (safe API).
#[derive(Debug, Clone)]
pub struct ThreadProjection {
    /// Meta rows.
    pub meta: Vec<ProjectedEntryMeta>,
    /// Line buffers.
    pub bufs: Vec<Vec<String>>,
}

impl ThreadProjection {
    /// Project entries.
    #[must_use]
    pub fn project(
        entries: &[MessageEntry],
        zoom: MessageZoom,
        search: Option<&str>,
        expanded_ids: &BTreeSet<String>,
        force_collapsed: &BTreeSet<String>,
    ) -> Self {
        let (meta, bufs) = project_message_thread(entries, zoom, search, expanded_ids, force_collapsed);
        Self { meta, bufs }
    }

    /// Build blocks for Transcript (borrows self).
    pub fn blocks<'a>(&'a self, line_ptrs: &'a mut Vec<Vec<&'a str>>) -> Vec<TranscriptBlock<'a, String>> {
        build_transcript_blocks(&self.meta, &self.bufs, line_ptrs)
    }

    /// Entry count (including group headers).
    #[must_use]
    pub fn len(&self) -> usize {
        self.meta.len()
    }

    /// Empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.meta.is_empty()
    }
}

/// Compact old entries while keeping checkpoints (returns kept entries).
#[must_use]
pub fn compact_entries(entries: &[MessageEntry], keep_recent: usize) -> Vec<MessageEntry> {
    if entries.len() <= keep_recent {
        return entries.to_vec();
    }
    let cut = entries.len().saturating_sub(keep_recent);
    let mut out: Vec<MessageEntry> = entries[..cut]
        .iter()
        .filter(|e| e.checkpoint)
        .cloned()
        .collect();
    out.extend(entries[cut..].iter().cloned());
    out
}

/// Filter entries by search query.
#[must_use]
pub fn filter_entries<'a>(entries: &'a [MessageEntry], query: &str) -> Vec<&'a MessageEntry> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return entries.iter().collect();
    }
    entries
        .iter()
        .filter(|e| e.haystack().to_ascii_lowercase().contains(&q))
        .collect()
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Message thread outcomes (host owns side effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MessageThreadOutcome {
    /// Ignored.
    Ignored,
    /// Viewport / selection changed.
    Changed,
    /// Block activated (expand or open detail).
    Activated {
        /// Entry id.
        id: String,
    },
    /// Fold request (host toggles collapsed / expanded set).
    FoldToggled {
        /// Id.
        id: String,
        /// Requested folded.
        folded: bool,
    },
    /// Follow mode changed.
    FollowChanged {
        /// Following tail.
        follow: bool,
    },
    /// Jump to latest (clear unread).
    JumpLatest,
    /// Copy requested for selected entry body.
    CopyRequested {
        /// Id.
        id: String,
    },
    /// Search draft changed.
    SearchChanged {
        /// Query.
        query: String,
    },
    /// Semantic zoom changed.
    ZoomChanged {
        /// Zoom.
        zoom: MessageZoom,
    },
    /// Host action chord on selected entry.
    ActionRequested {
        /// Entry id.
        id: String,
        /// Action id.
        action: String,
    },
    /// Esc / cancel.
    Cancelled,
}

/// Message thread interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageThreadState {
    /// Substrate viewport / selection / follow.
    pub transcript: TranscriptState<String>,
    /// Semantic zoom.
    pub zoom: MessageZoom,
    /// Search filter (None = off).
    pub search: Option<String>,
    /// Search mode typing.
    pub search_active: bool,
    /// Host expanded ids (override collapse).
    pub expanded: BTreeSet<String>,
    /// Host force-collapsed ids.
    pub force_collapsed: BTreeSet<String>,
    /// Unread entries below viewport while not following.
    pub unread_below: u32,
    /// Last known entry count for unread accounting.
    last_len: usize,
    /// Accepts input gate.
    accepts_input: bool,
}

impl Default for MessageThreadState {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageThreadState {
    /// Follow on by default.
    #[must_use]
    pub fn new() -> Self {
        let mut transcript = TranscriptState::new();
        transcript.set_follow(true);
        Self {
            transcript,
            zoom: MessageZoom::Summary,
            search: None,
            search_active: false,
            expanded: BTreeSet::new(),
            force_collapsed: BTreeSet::new(),
            unread_below: 0,
            last_len: 0,
            accepts_input: true,
        }
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Gate (does not clear selection / unread).
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.transcript.set_focused(on);
    }

    /// Focus chrome.
    pub fn set_focused(&mut self, on: bool) {
        self.transcript.set_focused(on);
    }

    /// Selected id.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.transcript.selected().map(String::as_str)
    }

    /// Follow.
    #[must_use]
    pub const fn follow(&self) -> bool {
        self.transcript.follow()
    }

    /// Whether to show new-content chip.
    #[must_use]
    pub const fn show_new_content(&self) -> bool {
        !self.transcript.follow() && self.unread_below > 0
    }

    /// Notify stream append of `new_len` entries (call after host append).
    pub fn on_entries_len(&mut self, new_len: usize) {
        if new_len > self.last_len {
            let delta = (new_len - self.last_len) as u32;
            if !self.transcript.follow() {
                self.unread_below = self.unread_below.saturating_add(delta);
            }
        }
        self.last_len = new_len;
    }

    /// Jump to latest / re-enable follow.
    pub fn jump_latest(&mut self) -> MessageThreadOutcome {
        self.transcript.set_follow(true);
        self.unread_below = 0;
        MessageThreadOutcome::JumpLatest
    }

    /// Toggle expand for id.
    pub fn set_expanded(&mut self, id: impl Into<String>, expanded: bool) {
        let id = id.into();
        if expanded {
            self.expanded.insert(id.clone());
            self.force_collapsed.remove(&id);
        } else {
            self.expanded.remove(&id);
            self.force_collapsed.insert(id);
        }
    }

    /// Cycle zoom.
    pub fn cycle_zoom(&mut self) -> MessageThreadOutcome {
        self.zoom = self.zoom.cycle();
        MessageThreadOutcome::ZoomChanged { zoom: self.zoom }
    }

    /// Project current view.
    #[must_use]
    pub fn projection(&self, entries: &[MessageEntry]) -> ThreadProjection {
        ThreadProjection::project(
            entries,
            self.zoom,
            self.search.as_deref(),
            &self.expanded,
            &self.force_collapsed,
        )
    }

    /// Map transcript outcome + local chords.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        entries: &[MessageEntry],
        blocks: &[TranscriptBlock<'_, String>],
    ) -> MessageThreadOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return MessageThreadOutcome::Ignored;
        }
        // Search mode
        if self.search_active {
            return self.handle_search_key(key);
        }
        if key.code == KeyCode::Char('/') && key.modifiers.is_empty() {
            self.search_active = true;
            self.search = Some(String::new());
            return MessageThreadOutcome::SearchChanged {
                query: String::new(),
            };
        }
        if key.code == KeyCode::Char('z') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.cycle_zoom();
        }
        if key.code == KeyCode::Char('n')
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && self.show_new_content()
        {
            return self.jump_latest();
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Some(id) = self.selected().map(str::to_string) {
                return MessageThreadOutcome::CopyRequested { id };
            }
        }
        // action chords when selected tool/error
        if key.modifiers.is_empty() {
            if let Some(id) = self.selected() {
                if let Some(e) = entries.iter().find(|e| e.id == id) {
                    match key.code {
                        KeyCode::Char('r') if matches!(e.kind, MessageKind::Tool | MessageKind::Error) => {
                            return MessageThreadOutcome::ActionRequested {
                                id: id.to_string(),
                                action: "retry".into(),
                            };
                        }
                        KeyCode::Char('x') if e.kind == MessageKind::Tool => {
                            return MessageThreadOutcome::ActionRequested {
                                id: id.to_string(),
                                action: "cancel".into(),
                            };
                        }
                        _ => {}
                    }
                }
            }
        }

        let out = self.transcript.handle_key(key, blocks);
        self.map_transcript_outcome(out)
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> MessageThreadOutcome {
        match key.code {
            KeyCode::Esc => {
                self.search_active = false;
                self.search = None;
                MessageThreadOutcome::SearchChanged {
                    query: String::new(),
                }
            }
            KeyCode::Enter => {
                self.search_active = false;
                MessageThreadOutcome::SearchChanged {
                    query: self.search.clone().unwrap_or_default(),
                }
            }
            KeyCode::Backspace => {
                if let Some(s) = &mut self.search {
                    s.pop();
                    let q = s.clone();
                    return MessageThreadOutcome::SearchChanged { query: q };
                }
                MessageThreadOutcome::Ignored
            }
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL) && !c.is_control() =>
            {
                self.search.get_or_insert_with(String::new).push(c);
                MessageThreadOutcome::SearchChanged {
                    query: self.search.clone().unwrap_or_default(),
                }
            }
            _ => MessageThreadOutcome::Ignored,
        }
    }

    fn map_transcript_outcome(&mut self, out: TranscriptOutcome<String>) -> MessageThreadOutcome {
        match out {
            TranscriptOutcome::Ignored => MessageThreadOutcome::Ignored,
            TranscriptOutcome::Changed => {
                if self.transcript.follow() {
                    self.unread_below = 0;
                }
                MessageThreadOutcome::Changed
            }
            TranscriptOutcome::Activated(id) => MessageThreadOutcome::Activated { id },
            TranscriptOutcome::FoldToggled { id, folded } => {
                self.set_expanded(&id, !folded);
                MessageThreadOutcome::FoldToggled { id, folded }
            }
            TranscriptOutcome::FollowChanged(follow) => {
                if follow {
                    self.unread_below = 0;
                }
                MessageThreadOutcome::FollowChanged { follow }
            }
            TranscriptOutcome::Cancelled => MessageThreadOutcome::Cancelled,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        blocks: &[TranscriptBlock<'_, String>],
    ) -> MessageThreadOutcome {
        if !self.accepts_input {
            return MessageThreadOutcome::Ignored;
        }
        let out = self.transcript.handle_mouse(event, blocks);
        self.map_transcript_outcome(out)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Message thread paint (Transcript + new-content / search chrome).
#[derive(Debug, Clone, Copy)]
pub struct MessageThread<'a> {
    entries: &'a [MessageEntry],
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
    focused: bool,
}

impl<'a> MessageThread<'a> {
    /// Entries + system.
    #[must_use]
    pub const fn new(entries: &'a [MessageEntry], system: &'a DesignSystem) -> Self {
        Self {
            entries,
            system,
            ascii: false,
            colorless: false,
            focused: true,
        }
    }

    /// ASCII prefixes.
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

    /// Focused chrome.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// Paint thread. Host should call [`MessageThreadState::on_entries_len`] after appends.
    pub fn paint(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut MessageThreadState,
    ) {
        if area.is_empty() {
            return;
        }
        let mut body = area;
        let mut footer_h = 0u16;
        if state.show_new_content() || state.search_active || state.search.is_some() {
            footer_h = 1;
        }
        if footer_h > 0 && area.height > 1 {
            body.height = area.height.saturating_sub(footer_h);
        }

        let proj = state.projection(self.entries);
        let mut line_ptrs: Vec<Vec<&str>> = Vec::new();
        let blocks = proj.blocks(&mut line_ptrs);

        let transcript = Transcript::new(&blocks, self.system)
            .focused(self.focused)
            .ascii(self.ascii)
            .colorless(self.colorless)
            .empty_label("(empty thread)");
        StatefulWidget::render(&transcript, body, buffer, &mut state.transcript);

        if footer_h > 0 {
            let y = area.y.saturating_add(area.height.saturating_sub(1));
            let style = self.system.style(crate::style::Role::Accent);
            let msg = if state.search_active {
                format!("/{}", state.search.as_deref().unwrap_or(""))
            } else if let Some(q) = &state.search {
                if !q.is_empty() {
                    format!("filter:{q}")
                } else {
                    String::new()
                }
            } else if state.show_new_content() {
                format!(
                    "↓ {} new  Ctrl+N jump · zoom:{}",
                    state.unread_below,
                    state.zoom.id()
                )
            } else {
                String::new()
            };
            if !msg.is_empty() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&msg, usize::from(area.width)),
                    usize::from(area.width),
                    style,
                );
            }
        }
        let _ = display_cols;
    }

    /// Render alias.
    pub fn render(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut MessageThreadState,
    ) {
        self.paint(area, buffer, state);
    }
}

/// Example session for stories / tests.
#[must_use]
pub fn example_message_session() -> Vec<MessageEntry> {
    vec![
        MessageEntry::user("u1", "Ship the MessageThread redesign")
            .actor(MessageActor::new("user", "you"))
            .timestamp("12:01")
            .group("today")
            .checkpoint(true),
        MessageEntry::assistant("a1", "Planning virtualized project-to-lines over Transcript.")
            .actor(MessageActor::new("agent", "agent"))
            .timestamp("12:01"),
        MessageEntry::tool("t1", "cargo test -p termrock")
            .actor(MessageActor::new("tool", "bash"))
            .timestamp("12:02")
            .status_letter('✓')
            .actions(vec![
                MessageAction::new("retry", "Retry").chord("r"),
                MessageAction::new("copy", "Copy").chord("Ctrl+C"),
            ]),
        MessageEntry::event("e1", "stream coalesced 12 chunks")
            .timestamp("12:02"),
        MessageEntry::error("err1", "preview paint failed once — retrying")
            .timestamp("12:03")
            .status_letter('E')
            .actions(vec![MessageAction::new("retry", "Retry").chord("r")]),
        MessageEntry::status("s1", "ready · follow")
            .timestamp("12:03"),
        MessageEntry::new(
            "a2",
            MessageKind::Assistant,
            [
                "Done. Follow preserves anchors; Ctrl+N jumps when unread.",
                "Zoom: Compact · Summary · Full via Ctrl+Z.",
                "Tools stay collapsed at Summary until activated.",
            ],
        )
        .actor(MessageActor::new("agent", "agent"))
        .timestamp("12:04"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Long-session sizes.
pub mod bench {
    /// Blocks in stress projection.
    pub const ENTRY_COUNT: usize = 2_000;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 40;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn project_folds_tools_at_summary() {
        let entries = example_message_session();
        let exp = BTreeSet::new();
        let col = BTreeSet::new();
        let (meta, _) = project_message_thread(&entries, MessageZoom::Summary, None, &exp, &col);
        let tool = meta.iter().find(|m| m.id == "t1").unwrap();
        assert!(tool.folded);
        let user = meta.iter().find(|m| m.id == "u1").unwrap();
        assert!(!user.folded || user.kind == MessageKind::User);
    }

    #[test]
    fn expand_override_unfolds_tool() {
        let entries = example_message_session();
        let mut exp = BTreeSet::new();
        exp.insert("t1".into());
        let col = BTreeSet::new();
        let (meta, bufs) =
            project_message_thread(&entries, MessageZoom::Summary, None, &exp, &col);
        let tool = meta.iter().find(|m| m.id == "t1").unwrap();
        assert!(!tool.folded);
        assert!(!bufs[tool.line_buf_index].is_empty());
    }

    #[test]
    fn search_filters_entries() {
        let entries = example_message_session();
        let hits = filter_entries(&entries, "virtualized");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "a1");
    }

    #[test]
    fn compact_keeps_checkpoints() {
        let mut entries = Vec::new();
        entries.push(MessageEntry::user("c0", "checkpoint").checkpoint(true));
        for i in 1..20 {
            entries.push(MessageEntry::status(format!("s{i}"), format!("line {i}")));
        }
        let kept = compact_entries(&entries, 5);
        assert!(kept.iter().any(|e| e.id == "c0" && e.checkpoint));
        assert!(kept.len() < entries.len());
        assert!(kept.len() >= 6); // checkpoint + 5 recent
    }

    #[test]
    fn unread_while_not_following() {
        let mut st = MessageThreadState::new();
        st.on_entries_len(3);
        st.transcript.set_follow(false);
        st.on_entries_len(5);
        assert_eq!(st.unread_below, 2);
        assert!(st.show_new_content());
        let _ = st.jump_latest();
        assert!(!st.show_new_content());
        assert!(st.follow());
    }

    #[test]
    fn follow_preserves_anchor_on_paint() {
        let system = DesignSystem::default();
        let mut entries = Vec::new();
        for i in 0..30 {
            entries.push(MessageEntry::assistant(
                format!("a{i}"),
                format!("message body line {i}"),
            ));
        }
        let mut st = MessageThreadState::new();
        st.transcript.set_follow(false);
        st.transcript.set_focused(true);
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        MessageThread::new(&entries, &system).paint(area, &mut buf, &mut st);
        let first = st.transcript.first_display_row();
        // append more while not following
        entries.push(MessageEntry::status("new", "appended"));
        st.on_entries_len(entries.len());
        MessageThread::new(&entries, &system).paint(area, &mut buf, &mut st);
        // anchor restore should keep near previous first (not jump to end)
        assert!(!st.follow());
        assert!(st.transcript.first_display_row() <= first.saturating_add(5));
    }

    #[test]
    fn copy_and_retry_actions() {
        let entries = example_message_session();
        let mut st = MessageThreadState::new();
        st.set_accepts_input(true);
        st.transcript.select(Some("err1".into()));
        let proj = st.projection(&entries);
        let mut ptrs = Vec::new();
        let blocks = proj.blocks(&mut ptrs);
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
            &entries,
            &blocks,
        );
        assert!(matches!(
            out,
            MessageThreadOutcome::ActionRequested { ref action, .. } if action == "retry"
        ));
        st.transcript.select(Some("a1".into()));
        let out = st.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &entries,
            &blocks,
        );
        assert!(matches!(
            out,
            MessageThreadOutcome::CopyRequested { ref id } if id == "a1"
        ));
    }

    #[test]
    fn zoom_cycle() {
        let mut st = MessageThreadState::new();
        assert_eq!(st.zoom, MessageZoom::Summary);
        assert!(matches!(
            st.cycle_zoom(),
            MessageThreadOutcome::ZoomChanged {
                zoom: MessageZoom::Full
            }
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let entries = example_message_session();
        let mut st = MessageThreadState::new();
        st.set_accepts_input(false);
        let proj = st.projection(&entries);
        let mut ptrs = Vec::new();
        let blocks = proj.blocks(&mut ptrs);
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &entries,
                &blocks
            ),
            MessageThreadOutcome::Ignored
        ));
    }

    #[test]
    fn moderate_session_bench() {
        let system = DesignSystem::default();
        let mut entries = Vec::with_capacity(bench::ENTRY_COUNT);
        for i in 0..bench::ENTRY_COUNT {
            let kind = match i % 5 {
                0 => MessageKind::User,
                1 => MessageKind::Assistant,
                2 => MessageKind::Tool,
                3 => MessageKind::Event,
                _ => MessageKind::Status,
            };
            let mut e = MessageEntry::new(format!("e{i}"), kind, [format!("body {i} line")]);
            if i % 50 == 0 {
                e = e.checkpoint(true);
            }
            if kind == MessageKind::Tool {
                e = e.collapsed(true).summary(format!("tool {i}"));
            }
            entries.push(e);
        }
        let mut st = MessageThreadState::new();
        st.on_entries_len(entries.len());
        let area = Rect::new(0, 0, 60, 24);
        let mut buf = Buffer::empty(area);
        for _ in 0..8 {
            MessageThread::new(&entries, &system).paint(area, &mut buf, &mut st);
            let proj = st.projection(&entries);
            let mut ptrs = Vec::new();
            let blocks = proj.blocks(&mut ptrs);
            let _ = st.handle_key(
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
                &entries,
                &blocks,
            );
        }
        assert!(st.projection(&entries).len() > 0);
    }

    #[test]
    fn no_chat_bubble_imitation() {
        let src = include_str!("message_thread.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        // strip mission docs that forbid bubbles
        let code = body
            .lines()
            .filter(|l| !l.trim_start().starts_with("//!") && !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
            .to_ascii_lowercase();
        for banned in ["rounded-lg", "chat-bubble", "speech_bubble"] {
            assert!(!code.contains(banned), "must not contain {banned}");
        }
        // no bubble paint helpers
        assert!(!code.contains("fn paint_bubble") && !code.contains("struct bubble"));
    }

    #[test]
    fn never_network() {
        let src = include_str!("message_thread.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in ["reqwest::", "tokio::net", "std::net::"] {
            assert!(!body.contains(forbidden));
        }
    }
}
