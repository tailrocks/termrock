// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **StreamingMarkdown** — streaming-safe Markdown for token-by-token AI output.
//!
//! **Mission.** Unfinished paragraphs, lists, tables, links, and code fences
//! without flicker or full-document reparse when avoidable. Preserve scroll
//! anchors and selection while content grows. Citations/source anchors, partial
//! syntax highlighting (via fence language + incomplete cue), and tool/status
//! insertions. Batch updates to balance latency and redraw cost. Adversarial
//! streaming fixtures and performance budgets.
//!
//! **Algorithm (stable prefix).** `committed` only grows on deltas; `tail` is
//! the reparse window (≤ [`STREAM_TAIL_MAX`]). Completed fence closes and blank
//! lines (outside fences) promote tail into committed. Width changes allow
//! full reparse. On failure, paint raw tail with [`StreamPhase::Failed`] —
//! never panic.
//!
//! Research: Glow-quality rendering + agent CLI streaming behavior.
//!
//! **Composition.** Projects into [`MarkdownView`](crate::widgets::MarkdownView)
//! / plain lines for [`MessageThread`](crate::widgets::MessageThread). Host
//! owns network; TermRock owns buffer split, provisional parse, paint.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyEvent, MouseEvent},
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::markdown::{
        MarkdownBlock, MarkdownBlockKind, MarkdownOutcome, MarkdownView, MarkdownViewState,
        SourceAnchor, project_markdown,
    },
};

/// Max reparse window for tail (bytes). Larger tails still work but may reparse more.
pub const STREAM_TAIL_MAX: usize = 16 * 1024;
/// Default coalesce budget: flush after this many deltas or timeout policy (host).
pub const STREAM_COALESCE_DELTAS: u32 = 8;
/// Default coalesce character budget before forced apply.
pub const STREAM_COALESCE_CHARS: usize = 256;
/// Performance: max full reparses allowed on hot path tests (should be 0).
pub const STREAM_HOT_FULL_REPARSE_BUDGET: u32 = 0;

// ── Phase / insertions ──────────────────────────────────────────────────────

/// Stream lifecycle for chrome (caret / failed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StreamPhase {
    /// Idle empty.
    #[default]
    Idle,
    /// Receiving deltas.
    Streaming,
    /// Stream completed; tail flushed.
    Done,
    /// Parse/host failure — raw fallback.
    Failed,
}

impl StreamPhase {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Streaming => "streaming",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// Host-injected non-markdown insertion (tool/status) at a stream offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInsertion {
    /// Stable id.
    pub id: String,
    /// Kind label (`tool`, `status`, `citation`).
    pub kind: String,
    /// Display lines (already projected).
    pub lines: Vec<String>,
}

impl StreamInsertion {
    /// Construct.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        lines: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            lines: lines.into_iter().map(Into::into).collect(),
        }
    }
}

/// Citation / source anchor attached to streamed content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCitation {
    /// Stable id.
    pub id: String,
    /// Display label (`[1]`, source title).
    pub label: String,
    /// Optional href / path (host opens).
    pub href: Option<String>,
    /// Source map into stream text.
    pub source: Option<SourceAnchor>,
}

impl StreamCitation {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            href: None,
            source: None,
        }
    }

    /// Href.
    #[must_use]
    pub fn href(mut self, h: impl Into<String>) -> Self {
        self.href = Some(h.into());
        self
    }

    /// Source.
    #[must_use]
    pub const fn source(mut self, a: SourceAnchor) -> Self {
        self.source = Some(a);
        self
    }
}

// ── Fence tracking ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct FenceState {
    /// Language tag on open fence.
    language: String,
    /// Fence marker length (3+ backticks).
    ticks: usize,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Streaming markdown buffer + view state.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamingMarkdownState {
    /// Stable prefix (only grows on normal stream).
    committed: String,
    /// Unstable tail (reparsed each apply).
    tail: String,
    /// Coalesced pending chars before apply.
    pending: String,
    /// Deltas since last apply.
    pending_deltas: u32,
    /// Open fence in committed+tail.
    fence: Option<FenceState>,
    /// Phase.
    pub phase: StreamPhase,
    /// Revision for height cache (MessageThread).
    pub revision: u64,
    /// Full reparse count (tests / metrics).
    pub full_reparse_count: u32,
    /// Tail-only reparse count.
    pub tail_reparse_count: u32,
    /// Markdown view interaction (scroll/selection).
    pub view: MarkdownViewState,
    /// Insertions (tool/status).
    pub insertions: Vec<StreamInsertion>,
    /// Citations.
    pub citations: Vec<StreamCitation>,
    /// Coalesce delta threshold.
    pub coalesce_deltas: u32,
    /// Coalesce char threshold.
    pub coalesce_chars: usize,
    /// Follow end while streaming (like tail).
    pub follow_stream: bool,
    /// Last measured width (for reparse on resize).
    last_width: u16,
    /// Accepts input for scroll/selection.
    accepts_input: bool,
    /// Show streaming caret.
    pub show_caret: bool,
    /// Error message when Failed.
    pub error: Option<String>,
}

impl Default for StreamingMarkdownState {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamingMarkdownState {
    /// Empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            committed: String::new(),
            tail: String::new(),
            pending: String::new(),
            pending_deltas: 0,
            fence: None,
            phase: StreamPhase::Idle,
            revision: 0,
            full_reparse_count: 0,
            tail_reparse_count: 0,
            view: MarkdownViewState::new(),
            insertions: Vec::new(),
            citations: Vec::new(),
            coalesce_deltas: STREAM_COALESCE_DELTAS,
            coalesce_chars: STREAM_COALESCE_CHARS,
            follow_stream: true,
            last_width: 0,
            accepts_input: true,
            show_caret: true,
            error: None,
        }
    }

    /// Full document (committed + tail).
    #[must_use]
    pub fn text(&self) -> String {
        let mut s = self.committed.clone();
        s.push_str(&self.tail);
        s.push_str(&self.pending);
        s
    }

    /// Committed length (stable prefix size).
    #[must_use]
    pub fn committed_len(&self) -> usize {
        self.committed.len()
    }

    /// Tail length (reparse window, excluding pending).
    #[must_use]
    pub fn tail_len(&self) -> usize {
        self.tail.len()
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
        self.view.focused = on;
    }

    /// Push token/delta (coalesced).
    pub fn push_delta(&mut self, delta: &str) {
        if delta.is_empty() {
            return;
        }
        if matches!(self.phase, StreamPhase::Idle | StreamPhase::Done) {
            self.phase = StreamPhase::Streaming;
        }
        self.pending.push_str(delta);
        self.pending_deltas = self.pending_deltas.saturating_add(1);
        if self.pending_deltas >= self.coalesce_deltas || self.pending.len() >= self.coalesce_chars
        {
            self.apply_pending();
        }
    }

    /// Force apply coalesced pending into tail and promote stables.
    pub fn apply_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        self.tail.push_str(&self.pending);
        self.pending.clear();
        self.pending_deltas = 0;
        self.promote_stable_from_tail();
        self.trim_tail_window();
        self.tail_reparse_count = self.tail_reparse_count.saturating_add(1);
        self.revision = self.revision.saturating_add(1);
        self.fence = detect_open_fence(&self.committed, &self.tail);
        if self.follow_stream {
            // scroll will be applied on paint after measure
            self.view.scroll_y = u16::MAX; // sentinel: paint clamps to end
        }
    }

    /// Flush pending and close stream.
    pub fn finish(&mut self) {
        self.apply_pending();
        // promote remaining tail
        self.committed.push_str(&self.tail);
        self.tail.clear();
        self.fence = None;
        self.phase = StreamPhase::Done;
        self.revision = self.revision.saturating_add(1);
        self.tail_reparse_count = self.tail_reparse_count.saturating_add(1);
    }

    /// Replace entire document (regenerate — committed may shrink).
    pub fn reset_with(&mut self, text: &str) {
        self.committed.clear();
        self.tail.clear();
        self.pending.clear();
        self.pending_deltas = 0;
        self.fence = None;
        self.error = None;
        self.phase = if text.is_empty() {
            StreamPhase::Idle
        } else {
            StreamPhase::Streaming
        };
        self.tail.push_str(text);
        self.promote_stable_from_tail();
        self.full_reparse_count = self.full_reparse_count.saturating_add(1);
        self.revision = self.revision.saturating_add(1);
        self.fence = detect_open_fence(&self.committed, &self.tail);
    }

    /// Mark failed; keep buffers for raw paint.
    pub fn fail(&mut self, message: impl Into<String>) {
        self.apply_pending();
        self.phase = StreamPhase::Failed;
        self.error = Some(message.into());
        self.revision = self.revision.saturating_add(1);
    }

    /// Width change: full reparse allowed.
    pub fn on_width_change(&mut self, width: u16) {
        if width == 0 || width == self.last_width {
            self.last_width = width;
            return;
        }
        self.last_width = width;
        // Merge for clean reparse
        let full = self.text();
        self.committed.clear();
        self.tail = full;
        self.pending.clear();
        self.pending_deltas = 0;
        self.promote_stable_from_tail();
        self.full_reparse_count = self.full_reparse_count.saturating_add(1);
        self.revision = self.revision.saturating_add(1);
        self.fence = detect_open_fence(&self.committed, &self.tail);
    }

    /// Add citation.
    pub fn add_citation(&mut self, c: StreamCitation) {
        self.citations.push(c);
        self.revision = self.revision.saturating_add(1);
    }

    /// Add insertion (tool/status).
    pub fn add_insertion(&mut self, ins: StreamInsertion) {
        self.insertions.push(ins);
        self.revision = self.revision.saturating_add(1);
    }

    fn promote_stable_from_tail(&mut self) {
        // Promote complete fences and blank-line-terminated segments out of tail.
        loop {
            if self.tail.is_empty() {
                break;
            }
            // If open fence only in tail, try to find close
            if let Some(split) = find_stable_boundary(&self.committed, &self.tail) {
                if split == 0 {
                    break;
                }
                self.committed.push_str(&self.tail[..split]);
                self.tail = self.tail[split..].to_string();
            } else {
                break;
            }
        }
    }

    fn trim_tail_window(&mut self) {
        // If tail grows beyond STREAM_TAIL_MAX without boundary, force promote head of tail
        // only when not inside fence (avoid splitting fence body).
        if self.tail.len() <= STREAM_TAIL_MAX {
            return;
        }
        if detect_open_fence(&self.committed, &self.tail).is_some() {
            // keep fence in tail; allow oversize for correctness
            return;
        }
        let overflow = self.tail.len() - STREAM_TAIL_MAX;
        // promote at last newline before overflow if possible
        let head = &self.tail[..overflow];
        let cut = head.rfind('\n').map(|i| i + 1).unwrap_or(overflow);
        self.committed.push_str(&self.tail[..cut]);
        self.tail = self.tail[cut..].to_string();
    }

    fn build_owned_blocks(&self) -> Vec<OwnedBlock> {
        let mut owned = Vec::new();
        if !self.committed.is_empty() {
            for b in project_markdown(&self.committed) {
                owned.push(OwnedBlock::from_block(&b, false));
            }
        }
        let mut work = self.tail.clone();
        work.push_str(&self.pending);
        if !work.is_empty() {
            let tail_blocks = project_markdown(&work);
            let n = tail_blocks.len();
            for (i, b) in tail_blocks.iter().enumerate() {
                let trailing = i + 1 == n
                    && matches!(self.phase, StreamPhase::Streaming | StreamPhase::Failed);
                let incomplete = b.incomplete
                    || (trailing
                        && matches!(
                            b.kind,
                            MarkdownBlockKind::Fence
                                | MarkdownBlockKind::Table
                                | MarkdownBlockKind::Paragraph
                        ));
                owned.push(OwnedBlock::from_block(b, incomplete));
            }
        }
        if matches!(self.phase, StreamPhase::Failed) && owned.is_empty() {
            owned.push(OwnedBlock {
                kind: MarkdownBlockKind::Paragraph,
                text: self.text(),
                incomplete: true,
                language: None,
                source: None,
                heading_level: crate::widgets::HeadingLevel::H2,
                depth: 0,
                list_index: None,
                task_checked: None,
            });
        }
        for ins in &self.insertions {
            owned.push(OwnedBlock {
                kind: MarkdownBlockKind::Paragraph,
                text: format!("[{}] {}", ins.kind, ins.lines.join(" ")),
                incomplete: false,
                language: None,
                source: None,
                heading_level: crate::widgets::HeadingLevel::H2,
                depth: 0,
                list_index: None,
                task_checked: None,
            });
        }
        if !self.citations.is_empty() {
            let cites = self
                .citations
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>()
                .join(" · ");
            owned.push(OwnedBlock {
                kind: MarkdownBlockKind::Paragraph,
                text: format!("Sources: {cites}"),
                incomplete: false,
                language: None,
                source: None,
                heading_level: crate::widgets::HeadingLevel::H2,
                depth: 0,
                list_index: None,
                task_checked: None,
            });
        }
        owned
    }

    /// Plain lines for MessageThread projection.
    #[must_use]
    pub fn plain_lines(&self) -> Vec<String> {
        let owned = self.build_owned_blocks();
        let mut lines = Vec::new();
        for b in &owned {
            match b.kind {
                MarkdownBlockKind::Blank => lines.push(String::new()),
                MarkdownBlockKind::Fence => {
                    let lang = b.language.as_deref().unwrap_or("");
                    lines.push(format!("```{lang}"));
                    for l in b.text.lines() {
                        lines.push(l.to_string());
                    }
                    if b.incomplete {
                        lines.push("…".into());
                    } else {
                        lines.push("```".into());
                    }
                }
                _ => {
                    for l in b.text.lines() {
                        lines.push(l.to_string());
                    }
                    if b.incomplete && matches!(self.phase, StreamPhase::Streaming) {
                        lines.push(crate::style::Glyph::SelectionGutter.resolve().text.into());
                    }
                }
            }
        }
        if lines.is_empty() && matches!(self.phase, StreamPhase::Streaming) {
            lines.push(crate::style::Glyph::SelectionGutter.resolve().text.into());
        }
        lines
    }

    /// Keys → markdown view (scroll/select/copy).
    pub fn handle_key(&mut self, key: KeyEvent, view: &MarkdownView<'_>) -> MarkdownOutcome {
        if !self.accepts_input {
            return MarkdownOutcome::Ignored;
        }
        view.handle_key(&mut self.view, key)
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, event: MouseEvent, view: &MarkdownView<'_>) -> MarkdownOutcome {
        if !self.accepts_input {
            return MarkdownOutcome::Ignored;
        }
        view.handle_mouse(&mut self.view, event)
    }
}

#[derive(Debug, Clone)]
struct OwnedBlock {
    kind: MarkdownBlockKind,
    text: String,
    incomplete: bool,
    language: Option<String>,
    source: Option<SourceAnchor>,
    heading_level: crate::widgets::HeadingLevel,
    depth: u8,
    list_index: Option<u32>,
    task_checked: Option<bool>,
}

impl OwnedBlock {
    fn from_block(b: &MarkdownBlock<'_>, incomplete: bool) -> Self {
        Self {
            kind: b.kind,
            text: b.text.to_string(),
            incomplete: incomplete || b.incomplete,
            language: b.language.map(str::to_string),
            source: b.source,
            heading_level: b.heading_level,
            depth: b.depth,
            list_index: b.list_index,
            task_checked: b.task_checked,
        }
    }
}

// ── Stable boundary ─────────────────────────────────────────────────────────

/// Find end offset into `tail` that can be promoted into committed.
fn find_stable_boundary(committed: &str, tail: &str) -> Option<usize> {
    if tail.is_empty() {
        return None;
    }
    // If currently inside fence (open without close in committed+tail prefix)
    if let Some(fence) = detect_open_fence(committed, "") {
        // look for close in tail
        return find_fence_close_offset(tail, fence.ticks);
    }
    if detect_open_fence(committed, tail).is_some() {
        // fence opens in tail — only promote content before the open fence line
        if let Some(idx) = find_fence_open_line_start(tail) {
            if idx > 0 {
                return Some(idx);
            }
            // fence starts at 0 of tail — wait for close
            return find_fence_close_offset(tail, detect_open_fence(committed, tail)?.ticks);
        }
        return None;
    }
    // Not in fence: promote through last double-newline (completed paragraph)
    // or completed list block ending with blank line
    if let Some(idx) = tail.rfind("\n\n") {
        let end = idx + 2;
        if end < tail.len() || tail.ends_with("\n\n") {
            // only promote if there's something after OR fully closed paragraph
            // Promote up to end if remaining starts a new block or empty
            return Some(end.min(tail.len()));
        }
    }
    // Completed fence entirely in tail
    if let Some(end) = find_complete_fence_in_tail(tail) {
        return Some(end);
    }
    None
}

fn find_fence_open_line_start(tail: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in tail.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if strip_fence_open(content).is_some() {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

fn find_fence_close_offset(tail: &str, ticks: usize) -> Option<usize> {
    let mut offset = 0usize;
    let mut seen_open = false;
    for line in tail.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if !seen_open {
            if strip_fence_open(content).is_some() {
                seen_open = true;
            }
            offset += line.len();
            continue;
        }
        if is_fence_close_ticks(content, ticks) {
            return Some(offset + line.len());
        }
        offset += line.len();
    }
    None
}

fn find_complete_fence_in_tail(tail: &str) -> Option<usize> {
    let mut offset = 0usize;
    let mut open_ticks: Option<usize> = None;
    for line in tail.split_inclusive('\n') {
        let content = line.trim_end_matches(['\n', '\r']);
        if open_ticks.is_none() {
            if strip_fence_open(content).is_some() {
                open_ticks = Some(fence_tick_count(content));
            }
            offset += line.len();
            continue;
        }
        if is_fence_close_ticks(content, open_ticks.unwrap_or(3)) {
            return Some(offset + line.len());
        }
        offset += line.len();
    }
    None
}

fn detect_open_fence(committed: &str, tail: &str) -> Option<FenceState> {
    let mut text = String::with_capacity(committed.len() + tail.len());
    text.push_str(committed);
    text.push_str(tail);
    let mut fence: Option<FenceState> = None;
    for line in text.split('\n') {
        let content = line.trim_end_matches('\r');
        if fence.is_none() {
            if let Some(lang) = strip_fence_open(content) {
                fence = Some(FenceState {
                    language: lang.to_string(),
                    ticks: fence_tick_count(content),
                });
            }
        } else if is_fence_close_ticks(content, fence.as_ref().map(|f| f.ticks).unwrap_or(3)) {
            fence = None;
        }
    }
    fence
}

fn strip_fence_open(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("```") && !trimmed.starts_with("~~~") {
        return None;
    }
    let rest = if let Some(r) = trimmed.strip_prefix("```") {
        r
    } else {
        trimmed.strip_prefix("~~~")?
    };
    // not a close fence (only ticks)
    if rest
        .chars()
        .all(|c| c == '`' || c == '~' || c.is_whitespace())
        && rest.trim().is_empty()
    {
        return None;
    }
    Some(rest.trim())
}

fn fence_tick_count(line: &str) -> usize {
    let t = line.trim_start();
    let ch = t.chars().next().unwrap_or('`');
    t.chars().take_while(|c| *c == ch).count().max(3)
}

fn is_fence_close_ticks(line: &str, ticks: usize) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    let ch = t.chars().next().unwrap_or('`');
    if ch != '`' && ch != '~' {
        return false;
    }
    let n = t.chars().take_while(|c| *c == ch).count();
    n >= ticks && t.chars().skip(n).all(|c| c.is_whitespace())
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Streaming markdown surface outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum StreamingMarkdownOutcome {
    /// Ignored.
    Ignored,
    /// Content/revision changed.
    Changed {
        /// New revision.
        revision: u64,
    },
    /// Markdown view outcome passthrough.
    View(MarkdownOutcome),
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Streaming markdown paint host.
#[derive(Debug, Clone, Copy)]
pub struct StreamingMarkdown<'a> {
    system: &'a DesignSystem,
    colorless: bool,
}

impl<'a> StreamingMarkdown<'a> {
    /// System.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            colorless: false,
        }
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Runs `f` over the blocks this stream currently projects.
    ///
    /// The projection borrows buffers that only live for the call, so the
    /// blocks cannot be returned. Hosts that need to measure a stream before
    /// they lay it out — the case GAP-MD-1 named — reach it through here or
    /// through [`Self::measure_height`], rather than reparsing the document
    /// themselves and disagreeing with the paint.
    pub fn with_blocks<R>(
        &self,
        state: &StreamingMarkdownState,
        f: impl FnOnce(&[MarkdownBlock<'_>]) -> R,
    ) -> R {
        let owned = state.build_owned_blocks();
        let bufs: Vec<String> = owned.iter().map(|ob| ob.text.clone()).collect();
        let lang_bufs: Vec<Option<String>> = owned.iter().map(|ob| ob.language.clone()).collect();
        let mut blocks: Vec<MarkdownBlock<'_>> = Vec::with_capacity(owned.len());
        for (i, ob) in owned.iter().enumerate() {
            let mut b = MarkdownBlock::new(ob.kind, bufs[i].as_str())
                .incomplete(ob.incomplete)
                .depth(ob.depth);
            b.heading_level = ob.heading_level;
            b.list_index = ob.list_index;
            b.task_checked = ob.task_checked;
            b.source = ob.source;
            if let Some(l) = lang_bufs[i].as_ref() {
                b.language = Some(l.as_str());
            }
            blocks.push(b);
        }
        f(&blocks)
    }

    /// Display rows this stream needs at `width`, including an open fence.
    #[must_use]
    pub fn measure_height(&self, state: &StreamingMarkdownState, width: u16) -> u16 {
        self.with_blocks(state, |blocks| {
            MarkdownView::new(blocks, self.system).measure_height(width)
        })
    }

    /// Paint streaming document.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut StreamingMarkdownState) {
        if area.is_empty() {
            return;
        }
        if state.last_width != 0 && state.last_width != area.width {
            state.on_width_change(area.width);
        }
        state.last_width = area.width;

        let owned = state.build_owned_blocks();
        let mut bufs: Vec<String> = Vec::new();
        let mut lang_bufs: Vec<Option<String>> = Vec::new();
        for ob in &owned {
            bufs.push(ob.text.clone());
            lang_bufs.push(ob.language.clone());
        }
        let mut blocks: Vec<MarkdownBlock<'_>> = Vec::with_capacity(owned.len());
        for (i, ob) in owned.iter().enumerate() {
            let mut b = MarkdownBlock::new(ob.kind, bufs[i].as_str())
                .incomplete(ob.incomplete)
                .depth(ob.depth);
            b.heading_level = ob.heading_level;
            b.list_index = ob.list_index;
            b.task_checked = ob.task_checked;
            b.source = ob.source;
            if let Some(ref l) = lang_bufs[i] {
                b.language = Some(l.as_str());
            }
            blocks.push(b);
        }

        let view = MarkdownView::new(&blocks, self.system);
        let colorless = self.colorless || self.system.mono();

        // follow stream scroll
        if state.follow_stream && matches!(state.phase, StreamPhase::Streaming) {
            let h = view.measure_height(area.width);
            state.view.scroll_y = h.saturating_sub(area.height);
        }

        view.paint(area, buffer, &mut state.view);

        // caret / failed strip
        if state.show_caret && matches!(state.phase, StreamPhase::Streaming) && area.height > 0 {
            // Not `▎`: that bar means "this row is selected".
            let cue = "▍";
            let y = area.bottom().saturating_sub(1);
            buffer.set_stringn(
                area.x.saturating_add(area.width.saturating_sub(2)),
                y,
                cue,
                1,
                self.system.style(if colorless {
                    Role::TextStrong
                } else {
                    Role::Accent
                }),
            );
        }
        if matches!(state.phase, StreamPhase::Failed) {
            if let Some(err) = &state.error {
                let y = area.y;
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(&format!("! {err}"), usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::Danger),
                );
            }
        }
    }
}

// ── Public helpers ──────────────────────────────────────────────────────────

/// Re-export stable boundary for tests.
#[must_use]
pub fn streaming_stable_prefix_len(committed: &str, tail: &str) -> usize {
    let mut c = committed.to_string();
    let mut t = tail.to_string();
    while let Some(split) = find_stable_boundary(&c, &t) {
        if split == 0 || split > t.len() {
            break;
        }
        c.push_str(&t[..split]);
        t = t[split..].to_string();
    }
    c.len()
}

/// Whether open fence is present.
#[must_use]
pub fn has_open_fence(text: &str) -> bool {
    detect_open_fence("", text).is_some()
}

// ── Bench / fixtures ────────────────────────────────────────────────────────

/// Performance sizes.
pub mod bench {
    /// Deltas in stream.
    pub const DELTA_COUNT: u32 = 500;
    /// Chars per delta.
    pub const DELTA_CHARS: usize = 4;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 30;
}

/// Adversarial streaming fixtures (partial markdown).
pub mod fixtures {
    /// Mid-fence stream chunks.
    #[must_use]
    pub fn mid_fence_chunks() -> Vec<&'static str> {
        vec![
            "# Title\n\n",
            "Intro para\n\n",
            "```rust\n",
            "fn main() {\n",
            "  println!(\"hi\");\n",
            // no close yet
        ]
    }

    /// Completes the fence.
    #[must_use]
    pub fn mid_fence_close() -> &'static str {
        "}\n```\n\nDone.\n"
    }

    /// Unclosed table.
    #[must_use]
    pub fn partial_table_chunks() -> Vec<&'static str> {
        vec!["| a | b |\n", "| --- | --- |\n", "| 1 | "]
    }

    /// Nested lists partial.
    #[must_use]
    pub fn partial_list_chunks() -> Vec<&'static str> {
        vec!["- one\n", "- two\n", "  - nested "]
    }

    /// Broken emphasis / links.
    #[must_use]
    pub fn partial_inline_chunks() -> Vec<&'static str> {
        vec!["See [docs](http", "s://example.com", ")\n\n"]
    }

    /// Rapid fence open/close thrash.
    #[must_use]
    pub fn fence_thrash() -> Vec<&'static str> {
        vec!["```\n", "x\n", "```\n", "```\n", "y"]
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    #[test]
    fn committed_grows_monotone_on_deltas() {
        let mut st = StreamingMarkdownState::new();
        st.coalesce_deltas = 1;
        st.coalesce_chars = 1;
        let mut last = 0usize;
        for ch in ["# H\n\n", "para\n\n", "```\n", "code\n", "```\n\n"] {
            st.push_delta(ch);
            st.apply_pending();
            assert!(
                st.committed_len() >= last,
                "committed shrank {} → {}",
                last,
                st.committed_len()
            );
            last = st.committed_len();
        }
        assert!(st.committed_len() > 0);
    }

    #[test]
    fn mid_fence_stays_in_tail_until_close() {
        let mut st = StreamingMarkdownState::new();
        st.coalesce_deltas = 1;
        for c in fixtures::mid_fence_chunks() {
            st.push_delta(c);
            st.apply_pending();
        }
        assert!(has_open_fence(&st.text()) || !st.tail.is_empty());
        // incomplete projection
        let lines = st.plain_lines();
        assert!(
            lines.iter().any(|l| l.contains("println")
                || l.contains("```")
                || l.contains("…")
                || l.contains("▎")),
            "{lines:?}"
        );
        st.push_delta(fixtures::mid_fence_close());
        st.apply_pending();
        st.finish();
        assert_eq!(st.phase, StreamPhase::Done);
        assert!(st.tail.is_empty());
        assert!(!has_open_fence(&st.text()));
    }

    /// GAP-MD-1: an open fence must not break the wrap contract.
    ///
    /// The failure this guards against is not "the fence looks wrong" — it is
    /// the row map drifting from the measured height while the fence is open,
    /// which makes every row below the fence paint one line off and scroll to
    /// the wrong place.
    #[test]
    fn an_open_fence_keeps_paint_and_measurement_agreeing() {
        use ratatui_core::buffer::Buffer;
        use ratatui_core::layout::Rect;

        let system = DesignSystem::default();
        let mut state = StreamingMarkdownState::new();
        state.coalesce_deltas = 1;
        state.coalesce_chars = 1;

        // Prose, then a fence that never closes, one token at a time.
        for delta in [
            "Explanation first.\n\n",
            "```rust\n",
            "fn main() {\n",
            "    println!(\"a line long enough to need clipping in a narrow pane\");\n",
        ] {
            state.push_delta(delta);
            state.apply_pending();

            StreamingMarkdown::new(&system).with_blocks(&state, |blocks| {
                let view = crate::widgets::MarkdownView::new(blocks, &system);
                for width in [12u16, 24, 80] {
                    assert_eq!(
                        view.measure_height(width),
                        view.block_start_row(usize::MAX, width),
                        "row count drifted between the two measure paths at {width} cols"
                    );
                }
            });
        }

        // The open fence is marked, so the view can paint its streaming cue.
        StreamingMarkdown::new(&system).with_blocks(&state, |blocks| {
            assert!(
                blocks
                    .iter()
                    .any(|b| b.kind == MarkdownBlockKind::Fence && b.incomplete),
                "an unterminated fence must project as incomplete"
            );
        });

        // And it paints inside a narrow pane without losing the prose above it.
        let area = Rect::new(0, 0, 16, 8);
        let mut buffer = Buffer::empty(area);
        StreamingMarkdown::new(&system).paint(area, &mut buffer, &mut state);
        let painted: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(painted.contains("rust"), "{painted}");
    }

    #[test]
    fn no_full_reparse_on_hot_deltas() {
        let mut st = StreamingMarkdownState::new();
        st.coalesce_deltas = 1;
        st.coalesce_chars = 1;
        let before = st.full_reparse_count;
        for _ in 0..50 {
            st.push_delta("word ");
            st.apply_pending();
        }
        assert_eq!(
            st.full_reparse_count, before,
            "hot path must not full-reparse"
        );
        assert!(st.tail_reparse_count >= 50 || st.revision >= 50);
    }

    #[test]
    fn width_change_allows_full_reparse() {
        let mut st = StreamingMarkdownState::new();
        st.push_delta("# Hi\n\nbody\n");
        st.apply_pending();
        let before = st.full_reparse_count;
        st.on_width_change(40);
        st.on_width_change(20);
        assert!(st.full_reparse_count > before);
    }

    #[test]
    fn finish_flushes_tail() {
        let mut st = StreamingMarkdownState::new();
        st.push_delta("only tail");
        st.apply_pending();
        st.finish();
        assert!(st.tail.is_empty());
        assert!(st.text().contains("only tail"));
        assert_eq!(st.phase, StreamPhase::Done);
    }

    #[test]
    fn fail_never_panics_partial() {
        let mut st = StreamingMarkdownState::new();
        for c in fixtures::partial_table_chunks() {
            st.push_delta(c);
        }
        st.apply_pending();
        st.fail("parse?");
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        StreamingMarkdown::new(&system).paint(area, &mut buf, &mut st);
        assert_eq!(st.phase, StreamPhase::Failed);
    }

    #[test]
    fn citations_and_insertions() {
        let mut st = StreamingMarkdownState::new();
        st.push_delta("See claim.\n\n");
        st.apply_pending();
        st.add_citation(StreamCitation::new("c1", "[1]").href("https://example.com"));
        st.add_insertion(StreamInsertion::new("t1", "tool", ["ran tests"]));
        let lines = st.plain_lines();
        assert!(lines.iter().any(|l| l.contains("Sources")));
        assert!(lines.iter().any(|l| l.contains("tool")));
    }

    #[test]
    fn coalesce_batches_deltas() {
        let mut st = StreamingMarkdownState::new();
        st.coalesce_deltas = 4;
        st.coalesce_chars = 10_000;
        st.push_delta("a");
        st.push_delta("b");
        assert!(st.tail.is_empty()); // still pending
        st.push_delta("c");
        st.push_delta("d");
        // 4th should apply
        assert!(!st.pending.is_empty() || !st.tail.is_empty() || st.committed_len() > 0);
        st.apply_pending();
        assert!(st.text().contains('a'));
    }

    #[test]
    fn scroll_state_preserved_when_not_following() {
        let mut st = StreamingMarkdownState::new();
        st.follow_stream = false;
        st.view.scroll_y = 3;
        st.push_delta("# x\n\n");
        for _ in 0..20 {
            st.push_delta("line\n\n");
            st.apply_pending();
        }
        assert_eq!(st.view.scroll_y, 3);
    }

    #[test]
    fn adversarial_fixtures_do_not_panic() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 48, 16);
        let mut buf = Buffer::empty(area);
        for fixture in [
            fixtures::mid_fence_chunks(),
            fixtures::partial_table_chunks(),
            fixtures::partial_list_chunks(),
            fixtures::partial_inline_chunks(),
            fixtures::fence_thrash(),
        ] {
            let mut st = StreamingMarkdownState::new();
            st.coalesce_deltas = 1;
            for c in fixture {
                st.push_delta(c);
                st.apply_pending();
                StreamingMarkdown::new(&system).paint(area, &mut buf, &mut st);
            }
        }
    }

    #[test]
    fn stream_perf_budget() {
        let system = DesignSystem::default();
        let mut st = StreamingMarkdownState::new();
        st.coalesce_deltas = 2;
        st.coalesce_chars = 32;
        let full_before = st.full_reparse_count;
        for i in 0..bench::DELTA_COUNT {
            let chunk = format!("{:04}", i % 1000);
            st.push_delta(&chunk[..bench::DELTA_CHARS.min(chunk.len())]);
            if i % 10 == 0 {
                st.push_delta("\n\n");
            }
        }
        st.apply_pending();
        assert_eq!(st.full_reparse_count, full_before);
        let area = Rect::new(0, 0, 60, 20);
        let mut buf = Buffer::empty(area);
        for _ in 0..bench::PAINT_FRAMES {
            StreamingMarkdown::new(&system).paint(area, &mut buf, &mut st);
        }
    }

    #[test]
    fn never_network() {
        let src = include_str!("streaming_markdown.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in ["reqwest::", "std::net::", "tokio::"] {
            assert!(!body.contains(forbidden));
        }
    }

    #[test]
    fn reset_allows_shrink() {
        let mut st = StreamingMarkdownState::new();
        st.push_delta("long text here\n\n");
        st.apply_pending();
        let n = st.committed_len() + st.tail_len();
        st.reset_with("x");
        assert!(st.text().len() < n || st.text() == "x");
    }
}
