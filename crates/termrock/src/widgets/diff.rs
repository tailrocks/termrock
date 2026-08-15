// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **DiffView** — high-quality read-only unified / side-by-side diff renderer.
//!
//! **Mission.** Files, hunks, line numbers, syntax spans, additions / deletions /
//! context, whitespace markers, word-level changes, folding, search, and
//! navigation. Narrow terminals force unified mode. Large diffs virtualize with
//! stable file/hunk anchors. Semantic roles + no-color prefixes (`+`/`-`/` `).
//!
//! **Ownership.** Host projects a window of [`DiffLine`] (+ optional [`DiffHunk`]
//! / file labels). Scroll, mode, search, fold, cursor live in [`DiffViewState`].
//! Scene owns surface focus (`focused` + `accepts_input`).
//!
//! **vs [`super::review::DiffReview`].** DiffReview is the interactive hunk
//! review veneer (activate/stage) over the same projection model. Prefer
//! DiffView for pure viewing (delta/GitUI-class paint); DiffReview when Enter
//! should fire product effects.
//!
//! Research: delta, lazygit, GitUI, review tools, TermRock DiffView/DiffReview.

#![allow(unused_variables, unused_mut)] // unit-test fixtures
use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{row_chrome::RowChrome, scroll_area::ScrollAreaState},
};

/// Semantic kind of a projected diff line (or word span).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DiffKind {
    /// Unchanged context.
    #[default]
    Context,
    /// Addition.
    Added,
    /// Deletion.
    Removed,
    /// File header (`diff --git`, `--- a/…`, `+++ b/…`).
    FileHeader,
    /// Hunk header (`@@ … @@`).
    HunkHeader,
    /// Meta / binary / note.
    Meta,
}

impl DiffKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Context => "context",
            Self::Added => "added",
            Self::Removed => "removed",
            Self::FileHeader => "file_header",
            Self::HunkHeader => "hunk_header",
            Self::Meta => "meta",
        }
    }

    /// No-color / monochrome line prefix (delta/git style).
    #[must_use]
    pub const fn prefix(self) -> char {
        match self {
            Self::Context => ' ',
            Self::Added => '+',
            Self::Removed => '-',
            Self::FileHeader | Self::HunkHeader | Self::Meta => ' ',
        }
    }

    /// Semantic role for themed paint.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Context => Role::Text,
            Self::Added => Role::DiffAdded,
            Self::Removed => Role::DiffRemoved,
            Self::FileHeader => Role::TextStrong,
            Self::HunkHeader | Self::Meta => Role::TextMuted,
        }
    }

    /// Whether this kind is a structural header (fold target).
    #[must_use]
    pub const fn is_header(self) -> bool {
        matches!(self, Self::FileHeader | Self::HunkHeader)
    }
}

/// Word-level change within a line (host-computed Myers/diff spans).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiffWordKind {
    /// Unchanged word/span.
    Equal,
    /// Inserted span.
    Insert,
    /// Deleted span.
    Delete,
}

impl DiffWordKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Equal => "equal",
            Self::Insert => "insert",
            Self::Delete => "delete",
        }
    }
}

/// One word-level span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffWordSpan<'a> {
    /// Change kind.
    pub kind: DiffWordKind,
    /// Span text.
    pub text: &'a str,
}

impl<'a> DiffWordSpan<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(kind: DiffWordKind, text: &'a str) -> Self {
        Self { kind, text }
    }
}

/// Optional syntax highlight span (host lexer; TermRock paints styles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffSyntaxSpan<'a> {
    /// Byte/char range start in line text (char index).
    pub start: usize,
    /// Exclusive end char index.
    pub end: usize,
    /// Semantic role override (e.g. keyword, string).
    pub role: Role,
    /// Borrowed label for diagnostics (unused in paint).
    pub _tag: Option<&'a str>,
}

impl<'a> DiffSyntaxSpan<'a> {
    /// Construct span.
    #[must_use]
    pub const fn new(start: usize, end: usize, role: Role) -> Self {
        Self {
            start,
            end,
            role,
            _tag: None,
        }
    }
}

/// Layout mode for side-by-side vs unified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum DiffMode {
    /// Split when wide enough; force unified when narrow (default).
    #[default]
    Auto,
    /// Always unified.
    Unified,
    /// Prefer side-by-side (still collapses under narrow width).
    Split,
}

impl DiffMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Unified => "unified",
            Self::Split => "split",
        }
    }
}

/// Effective paint mode after responsive resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiffEffectiveMode {
    /// Single column with prefixes.
    Unified,
    /// Two columns (old | new).
    Split,
}

/// Hunk index for navigation and folding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// Stable id (anchors / fold keys). Prefer path+header.
    pub id: String,
    /// Start line index in projected lines.
    pub start: usize,
    /// Length in lines (≥ 1).
    pub len: usize,
    /// Header label (`@@ -1,3 +1,4 @@`).
    pub header: String,
    /// Optional owning file id.
    pub file_id: Option<String>,
}

impl DiffHunk {
    /// Construct with start/len/header (id defaults to header).
    #[must_use]
    pub fn new(start: usize, len: usize, header: impl Into<String>) -> Self {
        let header = header.into();
        Self {
            id: header.clone(),
            start,
            len,
            header,
            file_id: None,
        }
    }

    /// Stable id override.
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// File association.
    #[must_use]
    pub fn file_id(mut self, file_id: impl Into<String>) -> Self {
        self.file_id = Some(file_id.into());
        self
    }

    /// Exclusive end line index.
    #[must_use]
    pub fn end(&self) -> usize {
        self.start.saturating_add(self.len.max(1))
    }

    /// Whether projected line `i` belongs to this hunk.
    #[must_use]
    pub fn contains_line(&self, i: usize) -> bool {
        i >= self.start && i < self.end()
    }
}

/// File band metadata (optional projection chrome).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffFile<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Display path (new side preferred).
    pub path: &'a str,
    /// Old path when renamed.
    pub old_path: Option<&'a str>,
    /// Language hint for host syntax (paint-neutral).
    pub language: Option<&'a str>,
    /// Start line index in projection.
    pub start: usize,
    /// Length in lines.
    pub len: usize,
}

impl<'a> DiffFile<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, path: &'a str, start: usize, len: usize) -> Self {
        Self {
            id,
            path,
            old_path: None,
            language: None,
            start,
            len,
        }
    }

    /// Old path (rename).
    #[must_use]
    pub const fn old_path(mut self, old: &'a str) -> Self {
        self.old_path = Some(old);
        self
    }

    /// Language.
    #[must_use]
    pub const fn language(mut self, lang: &'a str) -> Self {
        self.language = Some(lang);
        self
    }

    /// Exclusive end.
    #[must_use]
    pub fn end(&self) -> usize {
        self.start.saturating_add(self.len.max(1))
    }

    /// Contains line.
    #[must_use]
    pub fn contains_line(&self, i: usize) -> bool {
        i >= self.start && i < self.end()
    }
}

/// One projected diff line.
///
/// Prefer builders — new fields are additive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine<'a> {
    /// Stable identity (cursor, anchors).
    pub id: &'a str,
    /// Semantic kind.
    pub kind: DiffKind,
    /// Body text **without** leading `+`/`-`/` ` when TermRock supplies prefix.
    /// Hosts may still include prefixes; paint de-dupes when present.
    pub text: &'a str,
    /// Old side line number.
    pub old_no: Option<u32>,
    /// New side line number.
    pub new_no: Option<u32>,
    /// Word-level spans (when host provides).
    pub words: Option<&'a [DiffWordSpan<'a>]>,
    /// Syntax spans (when host provides).
    pub syntax: Option<&'a [DiffSyntaxSpan<'a>]>,
    /// Trailing whitespace present (marker when enabled).
    pub trailing_ws: bool,
    /// Owning file id.
    pub file_id: Option<&'a str>,
    /// Owning hunk id.
    pub hunk_id: Option<&'a str>,
    /// Paired old text for split mode (removed half when kind is Added context pair).
    pub pair_text: Option<&'a str>,
}

impl<'a> DiffLine<'a> {
    /// Minimal line.
    #[must_use]
    pub const fn new(id: &'a str, kind: DiffKind, text: &'a str) -> Self {
        Self {
            id,
            kind,
            text,
            old_no: None,
            new_no: None,
            words: None,
            syntax: None,
            trailing_ws: false,
            file_id: None,
            hunk_id: None,
            pair_text: None,
        }
    }

    /// Convenience context/added/removed.
    #[must_use]
    pub const fn context(id: &'a str, text: &'a str) -> Self {
        Self::new(id, DiffKind::Context, text)
    }

    /// Added line.
    #[must_use]
    pub const fn added(id: &'a str, text: &'a str) -> Self {
        Self::new(id, DiffKind::Added, text)
    }

    /// Removed line.
    #[must_use]
    pub const fn removed(id: &'a str, text: &'a str) -> Self {
        Self::new(id, DiffKind::Removed, text)
    }

    /// Hunk header line.
    #[must_use]
    pub const fn hunk_header(id: &'a str, text: &'a str) -> Self {
        Self::new(id, DiffKind::HunkHeader, text)
    }

    /// File header line.
    #[must_use]
    pub const fn file_header(id: &'a str, text: &'a str) -> Self {
        Self::new(id, DiffKind::FileHeader, text)
    }

    /// Old line number.
    #[must_use]
    pub const fn old_no(mut self, n: u32) -> Self {
        self.old_no = Some(n);
        self
    }

    /// New line number.
    #[must_use]
    pub const fn new_no(mut self, n: u32) -> Self {
        self.new_no = Some(n);
        self
    }

    /// Word spans.
    #[must_use]
    pub const fn words(mut self, words: &'a [DiffWordSpan<'a>]) -> Self {
        self.words = Some(words);
        self
    }

    /// Syntax spans.
    #[must_use]
    pub const fn syntax(mut self, spans: &'a [DiffSyntaxSpan<'a>]) -> Self {
        self.syntax = Some(spans);
        self
    }

    /// Trailing whitespace flag.
    #[must_use]
    pub const fn trailing_ws(mut self, on: bool) -> Self {
        self.trailing_ws = on;
        self
    }

    /// File id.
    #[must_use]
    pub const fn file_id(mut self, id: &'a str) -> Self {
        self.file_id = Some(id);
        self
    }

    /// Hunk id.
    #[must_use]
    pub const fn hunk_id(mut self, id: &'a str) -> Self {
        self.hunk_id = Some(id);
        self
    }

    /// Pair text for split row.
    #[must_use]
    pub const fn pair_text(mut self, text: &'a str) -> Self {
        self.pair_text = Some(text);
        self
    }
}

/// Hit region for a painted line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRegion {
    /// Line id.
    pub id: String,
    /// Index in filtered projection.
    pub index: usize,
    /// Area.
    pub area: Rect,
}

/// Outcomes (host owns stage/copy I/O).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiffViewOutcome {
    /// No change.
    Ignored,
    /// Viewport scrolled.
    Scrolled {
        /// Offset.
        offset: u16,
    },
    /// Cursor line changed.
    CursorMoved {
        /// Line index in filtered view.
        index: usize,
    },
    /// Hunk cursor moved.
    HunkCursorMoved {
        /// Hunk index.
        index: usize,
    },
    /// Hunk activated (stage/open — consumer effect).
    HunkActivated {
        /// Hunk index.
        index: usize,
    },
    /// File band focused.
    FileNavigated {
        /// File id.
        id: String,
    },
    /// Mode preference changed.
    ModeChanged(DiffMode),
    /// Search query changed.
    SearchChanged(String),
    /// Fold toggled.
    FoldToggled {
        /// Hunk or file id.
        id: String,
        /// Folded after toggle.
        folded: bool,
    },
    /// Cancel / clear search.
    Cancelled,
}

/// Runtime state (scroll sole authority for offset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffViewState {
    scroll: ScrollAreaState,
    accepts_input: bool,
    origin: (u16, u16),
    body_rows: u16,
    line_count: u16,
    /// Preferred mode (Auto/Unified/Split).
    pub mode: DiffMode,
    /// Cursor line in filtered projection.
    pub cursor: usize,
    /// Active hunk index.
    pub hunk_cursor: usize,
    /// Search query.
    pub search: Option<String>,
    /// Folded hunk ids.
    folded_hunks: BTreeSet<String>,
    /// Folded file ids.
    folded_files: BTreeSet<String>,
    /// Show old/new line numbers.
    pub show_line_numbers: bool,
    /// Show trailing-whitespace markers.
    pub show_whitespace: bool,
    /// Prefer word-level paint when spans present.
    pub word_diff: bool,
    /// Anchor line id.
    anchor_id: Option<String>,
    /// Anchor hunk id.
    anchor_hunk: Option<String>,
    /// Hit regions.
    pub regions: Vec<DiffRegion>,
    /// ASCII glyphs.
    pub ascii: bool,
    /// Colorless paint preference (also on widget).
    pub colorless: bool,
    content_width: u16,
    /// Horizontal content offset (unified long lines).
    pub h_offset: u16,
}

impl Default for DiffViewState {
    fn default() -> Self {
        Self::new()
    }
}

/// Backward-compatible alias for [`DiffViewState`].
pub type DiffState = DiffViewState;

impl DiffViewState {
    /// Fresh viewer (Auto mode, line numbers on, word diff on).
    #[must_use]
    pub fn new() -> Self {
        Self {
            scroll: ScrollAreaState::new().axes(true, false),
            accepts_input: true,
            origin: (0, 0),
            body_rows: 0,
            line_count: 0,
            mode: DiffMode::Auto,
            cursor: 0,
            hunk_cursor: 0,
            search: None,
            folded_hunks: BTreeSet::new(),
            folded_files: BTreeSet::new(),
            show_line_numbers: true,
            show_whitespace: true,
            word_diff: true,
            anchor_id: None,
            anchor_hunk: None,
            regions: Vec::new(),
            ascii: false,
            colorless: false,
            content_width: 0,
            h_offset: 0,
        }
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Whether host granted input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Vertical offset.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Scroll state.
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Split preferred (may still paint unified when narrow).
    #[must_use]
    pub const fn prefers_split(&self) -> bool {
        matches!(self.mode, DiffMode::Split | DiffMode::Auto)
    }

    /// Whether a hunk is folded.
    #[must_use]
    pub fn is_hunk_folded(&self, id: &str) -> bool {
        self.folded_hunks.contains(id)
    }

    /// Whether a file is folded.
    #[must_use]
    pub fn is_file_folded(&self, id: &str) -> bool {
        self.folded_files.contains(id)
    }

    /// Folded hunk set.
    #[must_use]
    pub fn folded_hunks(&self) -> &BTreeSet<String> {
        &self.folded_hunks
    }

    /// Capture line + hunk anchors.
    pub fn capture_anchor(&mut self, lines: &[DiffLine<'_>], hunks: &[DiffHunk]) {
        if let Some(l) = lines.get(self.cursor) {
            self.anchor_id = Some(l.id.to_string());
            if let Some(hid) = l.hunk_id {
                self.anchor_hunk = Some(hid.to_string());
            }
        }
        if let Some(h) = hunks.get(self.hunk_cursor) {
            self.anchor_hunk = Some(h.id.clone());
        }
    }

    /// Restore cursor from anchors.
    pub fn restore_anchor(&mut self, lines: &[DiffLine<'_>], hunks: &[DiffHunk]) {
        if let Some(aid) = self.anchor_id.as_ref() {
            if let Some(i) = lines.iter().position(|l| l.id == aid) {
                self.cursor = i;
            }
        }
        if let Some(hid) = self.anchor_hunk.as_ref() {
            if let Some(i) = hunks.iter().position(|h| h.id == *hid) {
                self.hunk_cursor = i;
            }
        }
        self.ensure_cursor_visible(lines.len());
    }

    fn sync_metrics(&mut self, total: u16, viewport: u16) {
        self.line_count = total;
        self.body_rows = viewport;
        self.scroll.set_content_size(1, total);
        self.scroll.set_viewport(1, viewport);
        self.scroll.clamp();
    }

    fn ensure_cursor_visible(&mut self, len: usize) {
        if len == 0 || self.body_rows == 0 {
            return;
        }
        let vh = usize::from(self.body_rows);
        let start = usize::from(self.scroll.offset_y());
        let end = start.saturating_add(vh);
        if self.cursor < start {
            self.scroll.set_offset_y_quiet(self.cursor as u16);
        } else if self.cursor >= end {
            let next = self.cursor.saturating_add(1).saturating_sub(vh);
            self.scroll.set_offset_y_quiet(next as u16);
        }
        self.scroll.clamp();
    }

    fn ensure_hunk_visible(&mut self, hunks: &[DiffHunk]) {
        if hunks.is_empty() || self.body_rows == 0 {
            return;
        }
        self.hunk_cursor = self.hunk_cursor.min(hunks.len() - 1);
        let Some(hunk) = hunks.get(self.hunk_cursor) else {
            return;
        };
        let vh = usize::from(self.body_rows);
        let start = usize::from(self.scroll.offset_y());
        let end = start.saturating_add(vh);
        if hunk.start < start {
            self.scroll
                .set_offset_y_quiet(hunk.start.min(u16::MAX as usize) as u16);
        } else if hunk.start >= end {
            let next = hunk.start.saturating_add(1).saturating_sub(vh);
            self.scroll
                .set_offset_y_quiet(next.min(u16::MAX as usize) as u16);
        }
        self.cursor = hunk.start.min(self.line_count.saturating_sub(1) as usize);
    }

    fn scroll_by_lines(&mut self, delta: i32) -> bool {
        self.scroll.scroll_by(delta as isize, 0).is_scrolled()
    }

    /// Resolve effective mode for a width.
    #[must_use]
    pub fn effective_mode(&self, width: u16) -> DiffEffectiveMode {
        // Side-by-side needs room for two gutters + numbers.
        const SPLIT_MIN: u16 = 56;
        match self.mode {
            DiffMode::Unified => DiffEffectiveMode::Unified,
            DiffMode::Split if width >= SPLIT_MIN => DiffEffectiveMode::Split,
            DiffMode::Split => DiffEffectiveMode::Unified,
            DiffMode::Auto if width >= SPLIT_MIN => DiffEffectiveMode::Split,
            DiffMode::Auto => DiffEffectiveMode::Unified,
        }
    }

    /// Keys (needs lines + hunks for nav).
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffViewOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return DiffViewOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        let view = filter_diff_lines(lines, self.search.as_deref().unwrap_or(""), self);

        // Search
        if is_press && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if self.search.is_none() {
                self.search = Some(String::new());
            }
            return DiffViewOutcome::SearchChanged(self.search.clone().unwrap_or_default());
        }
        if let Some(q) = self.search.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.search = None;
                    return DiffViewOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.search = None;
                    }
                    return DiffViewOutcome::SearchChanged(self.search.clone().unwrap_or_default());
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return DiffViewOutcome::SearchChanged(q.clone());
                }
                _ => {}
            }
        }

        if is_press {
            match key.code {
                KeyCode::Char('s' | 'S') => {
                    self.mode = match self.mode {
                        DiffMode::Auto => DiffMode::Split,
                        DiffMode::Split => DiffMode::Unified,
                        DiffMode::Unified => DiffMode::Auto,
                    };
                    return DiffViewOutcome::ModeChanged(self.mode);
                }
                KeyCode::Char('n' | 'N') if !hunks.is_empty() => {
                    let next = (self.hunk_cursor + 1).min(hunks.len() - 1);
                    if next == self.hunk_cursor {
                        return DiffViewOutcome::Ignored;
                    }
                    self.hunk_cursor = next;
                    self.ensure_hunk_visible(hunks);
                    return DiffViewOutcome::HunkCursorMoved {
                        index: self.hunk_cursor,
                    };
                }
                KeyCode::Char('p' | 'P') if !hunks.is_empty() => {
                    let next = self.hunk_cursor.saturating_sub(1);
                    if next == self.hunk_cursor {
                        return DiffViewOutcome::Ignored;
                    }
                    self.hunk_cursor = next;
                    self.ensure_hunk_visible(hunks);
                    return DiffViewOutcome::HunkCursorMoved {
                        index: self.hunk_cursor,
                    };
                }
                KeyCode::Char('z' | 'Z') => {
                    // Toggle fold on current hunk
                    if let Some(h) = hunks.get(self.hunk_cursor) {
                        let id = h.id.clone();
                        let folded = if !self.folded_hunks.remove(&id) {
                            self.folded_hunks.insert(id.clone());
                            true
                        } else {
                            false
                        };
                        return DiffViewOutcome::FoldToggled { id, folded };
                    }
                }
                KeyCode::Char('l' | 'L') if key.modifiers.is_empty() => {
                    self.show_line_numbers = !self.show_line_numbers;
                    return DiffViewOutcome::Ignored;
                }
                KeyCode::Char('w' | 'W') => {
                    self.word_diff = !self.word_diff;
                    return DiffViewOutcome::Ignored;
                }
                KeyCode::Char('.') => {
                    self.show_whitespace = !self.show_whitespace;
                    return DiffViewOutcome::Ignored;
                }
                KeyCode::Left | KeyCode::Char('h' | 'H') => {
                    if self.h_offset > 0 {
                        self.h_offset = self.h_offset.saturating_sub(4);
                        return DiffViewOutcome::Scrolled {
                            offset: self.offset(),
                        };
                    }
                }
                KeyCode::Right => {
                    let max = self.content_width.saturating_sub(40);
                    if self.h_offset < max {
                        self.h_offset = self.h_offset.saturating_add(4).min(max);
                        return DiffViewOutcome::Scrolled {
                            offset: self.offset(),
                        };
                    }
                }
                _ => {}
            }
        }

        if let Some(intent) = crate::interaction::default_diff_review_intent(key)
            .or_else(|| crate::interaction::default_list_intent(key))
        {
            return self.handle_intent(intent, &view, hunks);
        }
        DiffViewOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        lines: &[&DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffViewOutcome {
        if !self.accepts_input {
            return DiffViewOutcome::Ignored;
        }
        let len = lines.len();
        if len > 0 {
            self.cursor = self.cursor.min(len - 1);
        }
        match intent {
            UiIntent::Move(NavigationMove::Next) => {
                if len > 0 && self.cursor + 1 < len {
                    self.cursor += 1;
                    self.ensure_cursor_visible(len);
                    sync_hunk_from_cursor(self, lines, hunks);
                    return DiffViewOutcome::CursorMoved { index: self.cursor };
                }
                if !self.scroll_by_lines(1) {
                    return DiffViewOutcome::Ignored;
                }
                DiffViewOutcome::Scrolled {
                    offset: self.offset(),
                }
            }
            UiIntent::Move(NavigationMove::Previous) => {
                if len > 0 && self.cursor > 0 {
                    self.cursor -= 1;
                    self.ensure_cursor_visible(len);
                    sync_hunk_from_cursor(self, lines, hunks);
                    return DiffViewOutcome::CursorMoved { index: self.cursor };
                }
                if !self.scroll_by_lines(-1) {
                    return DiffViewOutcome::Ignored;
                }
                DiffViewOutcome::Scrolled {
                    offset: self.offset(),
                }
            }
            UiIntent::Move(NavigationMove::First) => {
                if !hunks.is_empty() {
                    self.hunk_cursor = 0;
                    self.ensure_hunk_visible(hunks);
                    return DiffViewOutcome::HunkCursorMoved { index: 0 };
                }
                self.cursor = 0;
                let before = self.offset();
                self.scroll.set_offset_y(0);
                if self.offset() != before {
                    DiffViewOutcome::Scrolled {
                        offset: self.offset(),
                    }
                } else {
                    DiffViewOutcome::CursorMoved { index: 0 }
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                if !hunks.is_empty() {
                    self.hunk_cursor = hunks.len() - 1;
                    self.ensure_hunk_visible(hunks);
                    return DiffViewOutcome::HunkCursorMoved {
                        index: self.hunk_cursor,
                    };
                }
                if len > 0 {
                    self.cursor = len - 1;
                    self.ensure_cursor_visible(len);
                }
                DiffViewOutcome::CursorMoved { index: self.cursor }
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = i32::from(self.body_rows.max(1));
                if !self.scroll_by_lines(step) {
                    return DiffViewOutcome::Ignored;
                }
                DiffViewOutcome::Scrolled {
                    offset: self.offset(),
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = i32::from(self.body_rows.max(1));
                if !self.scroll_by_lines(-step) {
                    return DiffViewOutcome::Ignored;
                }
                DiffViewOutcome::Scrolled {
                    offset: self.offset(),
                }
            }
            UiIntent::Activate | UiIntent::Submit if !hunks.is_empty() => {
                DiffViewOutcome::HunkActivated {
                    index: self.hunk_cursor,
                }
            }
            UiIntent::Toggle => {
                self.mode = match self.mode {
                    DiffMode::Auto => DiffMode::Split,
                    DiffMode::Split => DiffMode::Unified,
                    DiffMode::Unified => DiffMode::Auto,
                };
                DiffViewOutcome::ModeChanged(self.mode)
            }
            UiIntent::Cancel => {
                if self.search.is_some() {
                    self.search = None;
                    return DiffViewOutcome::Cancelled;
                }
                DiffViewOutcome::Ignored
            }
            _ => DiffViewOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        lines: &[DiffLine<'_>],
        hunks: &[DiffHunk],
    ) -> DiffViewOutcome {
        if !self.accepts_input {
            return DiffViewOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let hit = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.body_rows.max(1),
        };
        if !hit.contains(event.position) {
            return DiffViewOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => self.handle_intent(
                UiIntent::Move(NavigationMove::Next),
                &filter_diff_lines(lines, self.search.as_deref().unwrap_or(""), self),
                hunks,
            ),
            MouseEventKind::ScrollUp => self.handle_intent(
                UiIntent::Move(NavigationMove::Previous),
                &filter_diff_lines(lines, self.search.as_deref().unwrap_or(""), self),
                hunks,
            ),
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    self.cursor = r.index;
                    let view = filter_diff_lines(lines, self.search.as_deref().unwrap_or(""), self);
                    sync_hunk_from_cursor(self, &view, hunks);
                    if event.modifiers.contains(KeyModifiers::CONTROL) && !hunks.is_empty() {
                        return DiffViewOutcome::HunkActivated {
                            index: self.hunk_cursor,
                        };
                    }
                    return DiffViewOutcome::CursorMoved { index: self.cursor };
                }
                DiffViewOutcome::Ignored
            }
            _ => DiffViewOutcome::Ignored,
        }
    }
}

fn sync_hunk_from_cursor(state: &mut DiffViewState, lines: &[&DiffLine<'_>], hunks: &[DiffHunk]) {
    let Some(line) = lines.get(state.cursor) else {
        return;
    };
    if let Some(hid) = line.hunk_id {
        if let Some(i) = hunks.iter().position(|h| h.id == hid) {
            state.hunk_cursor = i;
            return;
        }
    }
    // Fallback: index into raw projection order
    if let Some(i) = hunks.iter().position(|h| h.contains_line(state.cursor)) {
        state.hunk_cursor = i;
    }
}

/// Filter by search + fold.
#[must_use]
pub fn filter_diff_lines<'a>(
    lines: &'a [DiffLine<'a>],
    query: &str,
    state: &DiffViewState,
) -> Vec<&'a DiffLine<'a>> {
    let q = query.trim().to_ascii_lowercase();
    let mut out = Vec::with_capacity(lines.len());
    let mut skip_until: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(end) = skip_until {
            if i < end {
                // Keep headers visible when folded
                if line.kind.is_header() {
                    out.push(line);
                }
                continue;
            }
            skip_until = None;
        }
        if let Some(fid) = line.file_id {
            if state.folded_files.contains(fid) && !matches!(line.kind, DiffKind::FileHeader) {
                continue;
            }
        }
        if let Some(hid) = line.hunk_id {
            if state.folded_hunks.contains(hid) && !matches!(line.kind, DiffKind::HunkHeader) {
                continue;
            }
        }
        if !q.is_empty() {
            let hay = line.text.to_ascii_lowercase();
            if !hay.contains(&q) && !line.kind.is_header() {
                continue;
            }
        }
        out.push(line);
    }
    // Second pass: apply hunk fold by start/len when ids match DiffHunk stored folds
    if !state.folded_hunks.is_empty() {
        // already handled via hunk_id on lines
    }
    let _ = skip_until;
    out
}

/// Strip a single leading git prefix if present.
fn strip_diff_prefix(text: &str, kind: DiffKind) -> &str {
    let b = text.as_bytes();
    if b.is_empty() {
        return text;
    }
    match kind {
        DiffKind::Added if b[0] == b'+' => &text[1..],
        DiffKind::Removed if b[0] == b'-' => &text[1..],
        DiffKind::Context if b[0] == b' ' => &text[1..],
        _ => text,
    }
}

/// Escape control characters for safe paint.
#[must_use]
pub fn escape_diff_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push('\t'),
            '\0' => out.push_str("\\0"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Visible whitespace marker for trailing spaces/tabs.
fn ws_marker(ascii: bool) -> &'static str {
    if ascii { "~" } else { "·" }
}

/// High-quality read-only diff paint.
#[derive(Debug, Clone)]
pub struct DiffView<'a> {
    lines: &'a [DiffLine<'a>],
    hunks: &'a [DiffHunk],
    files: &'a [DiffFile<'a>],
    system: &'a DesignSystem,
    focused: bool,
    ascii: bool,
    colorless: bool,
    title: Option<&'a str>,
}

impl<'a> DiffView<'a> {
    /// Lines + design system.
    #[must_use]
    pub const fn new(lines: &'a [DiffLine<'a>], system: &'a DesignSystem) -> Self {
        Self {
            lines,
            hunks: &[],
            files: &[],
            system,
            focused: true,
            ascii: false,
            colorless: false,
            title: None,
        }
    }

    /// Hunk index model.
    #[must_use]
    pub const fn hunks(mut self, hunks: &'a [DiffHunk]) -> Self {
        self.hunks = hunks;
        self
    }

    /// File bands.
    #[must_use]
    pub const fn files(mut self, files: &'a [DiffFile<'a>]) -> Self {
        self.files = files;
        self
    }

    /// Optional title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Surface focus chrome.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII prefixes / empty marks.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint O(visible).
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut DiffViewState) {
        state.regions.clear();
        if area.is_empty() {
            state.body_rows = 0;
            return;
        }
        let ascii = self.ascii || state.ascii;
        let colorless = self.colorless || state.colorless;
        state.origin = (area.x, area.y);

        let view = filter_diff_lines(self.lines, state.search.as_deref().unwrap_or(""), state);
        // Also fold hunk ranges by DiffHunk id when lines lack hunk_id
        let view = apply_hunk_fold_fallback(view, self.lines, self.hunks, state);

        state.content_width = view
            .iter()
            .map(|l| u16::try_from(display_cols(l.text)).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0)
            .saturating_add(24);

        let effective = state.effective_mode(area.width);
        let tiny = area.width < 16;
        let narrow = area.width < 36;
        let title_h = u16::from(self.title.is_some() && area.height >= 2);
        let chip_h = u16::from(area.height >= 3);
        let body_h = area.height.saturating_sub(title_h + chip_h).max(1);

        let total = view.len().min(u16::MAX as usize) as u16;
        state.sync_metrics(total, body_h);
        if total > 0 {
            state.cursor = state.cursor.min(usize::from(total) - 1);
        }
        if !self.hunks.is_empty() {
            state.hunk_cursor = state.hunk_cursor.min(self.hunks.len() - 1);
        }

        let surface = self.focused && state.accepts_input;
        let mut y = area.y;

        if let Some(title) = self.title {
            if y < area.bottom() {
                buffer.set_stringn(
                    area.x,
                    y,
                    take_display_cols(title, usize::from(area.width)),
                    usize::from(area.width),
                    self.system.style(Role::TextStrong),
                );
                y = y.saturating_add(1);
            }
        }

        let body_top = y;
        let bottom = area
            .y
            .saturating_add(title_h)
            .saturating_add(body_h)
            .min(area.bottom().saturating_sub(chip_h));

        if view.is_empty() {
            let mark = if ascii { "[ ] " } else { "∅ " };
            let msg = if tiny {
                format!("{mark}empty")
            } else {
                format!("{mark}(empty diff)")
            };
            buffer.set_stringn(
                area.x,
                body_top,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let start = state.offset() as usize;
            let mut py = body_top;
            for (i, line) in view.iter().enumerate().skip(start) {
                if py >= bottom {
                    break;
                }
                let in_hunk = self.hunks.get(state.hunk_cursor).is_some_and(|h| {
                    if let Some(hid) = line.hunk_id {
                        h.id == hid
                    } else {
                        // map filtered index poorly — use id match on header only
                        h.contains_line(i) || line.text.contains(&h.header)
                    }
                });
                let cursor = i == state.cursor;

                match effective {
                    DiffEffectiveMode::Unified => {
                        paint_unified_line(
                            buffer,
                            Rect::new(area.x, py, area.width, 1),
                            line,
                            state,
                            self.system,
                            surface,
                            ascii,
                            colorless,
                            tiny,
                            narrow,
                            cursor,
                            in_hunk,
                        );
                    }
                    DiffEffectiveMode::Split => {
                        paint_split_line(
                            buffer,
                            Rect::new(area.x, py, area.width, 1),
                            line,
                            state,
                            self.system,
                            surface,
                            ascii,
                            colorless,
                            cursor,
                            in_hunk,
                        );
                    }
                }

                state.regions.push(DiffRegion {
                    id: line.id.to_string(),
                    index: i,
                    area: Rect::new(area.x, py, area.width, 1),
                });
                py = py.saturating_add(1);
            }
        }

        if chip_h > 0 {
            let chip_y = area.bottom().saturating_sub(1);
            let mode = match effective {
                DiffEffectiveMode::Unified => {
                    if ascii {
                        "unified"
                    } else {
                        "unified"
                    }
                }
                DiffEffectiveMode::Split => "split",
            };
            let mut chip = format!(
                "{mode} · hunk {}/{}",
                if self.hunks.is_empty() {
                    0
                } else {
                    state.hunk_cursor + 1
                },
                self.hunks.len()
            );
            if let Some(q) = &state.search {
                chip.push_str(&format!(" · /{q}"));
            }
            if !state.folded_hunks.is_empty() {
                chip.push_str(&format!(" · fold {}", state.folded_hunks.len()));
            }
            if state.word_diff {
                chip.push_str(" · words");
            }
            if !state.show_line_numbers {
                chip.push_str(" · no#");
            }
            let style = if surface {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                area.x,
                chip_y,
                take_display_cols(&chip, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
        }
    }
}

fn apply_hunk_fold_fallback<'a>(
    view: Vec<&'a DiffLine<'a>>,
    _all: &'a [DiffLine<'a>],
    hunks: &[DiffHunk],
    state: &DiffViewState,
) -> Vec<&'a DiffLine<'a>> {
    if state.folded_hunks.is_empty() || hunks.is_empty() {
        return view;
    }
    // If lines carry hunk_id, filter_diff_lines already handled it.
    if view.iter().any(|l| l.hunk_id.is_some()) {
        return view;
    }
    // Fold projection incomplete: identity until header/body drop is implemented.
    view
}

fn kind_style(
    system: &DesignSystem,
    kind: DiffKind,
    colorless: bool,
    surface: bool,
    emphasize: bool,
) -> Style {
    if colorless {
        match kind {
            DiffKind::Added | DiffKind::Removed if surface || emphasize => {
                system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
            }
            DiffKind::HunkHeader | DiffKind::Meta | DiffKind::FileHeader => {
                system.style(Role::TextMuted)
            }
            _ if emphasize => system.style(Role::TextStrong),
            _ => system.style(Role::Text),
        }
    } else {
        // A cursored line is still an added / removed / context line: the
        // selection speaks through the row chrome, not by repainting the tone.
        system.style(kind.role())
    }
}

fn paint_unified_line(
    buffer: &mut Buffer,
    area: Rect,
    line: &DiffLine<'_>,
    state: &DiffViewState,
    system: &DesignSystem,
    surface: bool,
    ascii: bool,
    colorless: bool,
    tiny: bool,
    narrow: bool,
    cursor: bool,
    in_hunk: bool,
) {
    if area.is_empty() {
        return;
    }
    let chrome = RowChrome::resolve(
        system,
        ListRowVisualState {
            selected: cursor,
            focused: surface,
            enabled: true,
            ..Default::default()
        },
    );
    let style = chrome.label_style(kind_style(
        system,
        line.kind,
        colorless,
        surface,
        cursor || in_hunk,
    ));
    if !colorless && matches!(line.kind, DiffKind::Added | DiffKind::Removed) {
        buffer.set_style(area, system.style(line.kind.role()));
    }
    // The cursor's own column is stamped by the shared chrome below.
    let gutter = if cursor && surface {
        " "
    } else if in_hunk {
        if ascii { "." } else { "·" }
    } else {
        " "
    };

    let body = strip_diff_prefix(line.text, line.kind);
    let body = escape_diff_text(body);
    let prefix = line.kind.prefix();

    let nums = if state.show_line_numbers && !tiny && !narrow {
        format!(
            "{:>4} {:>4} ",
            line.old_no
                .map(|n| n.to_string())
                .unwrap_or_else(|| "    ".into()),
            line.new_no
                .map(|n| n.to_string())
                .unwrap_or_else(|| "    ".into()),
        )
    } else if state.show_line_numbers && !tiny {
        format!("{:>3} ", line.new_no.or(line.old_no).unwrap_or(0))
    } else {
        String::new()
    };

    let mut composed = format!("{gutter}{nums}{prefix}{body}");
    if state.show_whitespace && line.trailing_ws {
        composed.push_str(ws_marker(ascii));
    }

    // Word-level: paint base then overlay is complex without multi-span set_string;
    // paint flattened with kind style; host words used for emphasis markers.
    if state.word_diff {
        if let Some(words) = line.words {
            if !words.is_empty() && !tiny {
                composed = format!("{gutter}{nums}{prefix}");
                for w in words {
                    composed.push_str(&escape_diff_text(w.text));
                }
                if state.show_whitespace && line.trailing_ws {
                    composed.push_str(ws_marker(ascii));
                }
            }
        }
    }

    let skip = usize::from(state.h_offset);
    let visible: String = composed.chars().skip(skip).collect();
    let painted = take_display_cols(&visible, usize::from(area.width));

    if state.word_diff {
        if let Some(words) = line.words {
            if !words.is_empty() && !tiny && state.h_offset == 0 {
                paint_word_line(
                    buffer,
                    area,
                    &format!("{gutter}{nums}{prefix}"),
                    words,
                    system,
                    colorless,
                    surface,
                    style,
                    line.trailing_ws && state.show_whitespace,
                    ascii,
                );
                chrome.paint(buffer, area);
                return;
            }
        }
        buffer.set_stringn(area.x, area.y, &painted, usize::from(area.width), style);
    } else {
        buffer.set_stringn(area.x, area.y, &painted, usize::from(area.width), style);
    }
    chrome.paint(buffer, area);
}

fn paint_word_line(
    buffer: &mut Buffer,
    area: Rect,
    lead: &str,
    words: &[DiffWordSpan<'_>],
    system: &DesignSystem,
    colorless: bool,
    surface: bool,
    base: Style,
    trailing_ws: bool,
    ascii: bool,
) {
    let mut x = area.x;
    let max_x = area.x.saturating_add(area.width);
    let lead_t = take_display_cols(lead, usize::from(area.width));
    buffer.set_stringn(x, area.y, &lead_t, lead_t.chars().count().max(1), base);
    x = x.saturating_add(display_cols(&lead_t) as u16);

    for w in words {
        if x >= max_x {
            break;
        }
        let st = if colorless {
            match w.kind {
                DiffWordKind::Equal => base,
                DiffWordKind::Insert | DiffWordKind::Delete => {
                    system.style(Role::TextStrong).add_modifier(Modifier::BOLD)
                }
            }
        } else {
            match w.kind {
                DiffWordKind::Equal => base,
                // The word tints already carry their own ground; weight marks
                // which words inside the line actually moved.
                DiffWordKind::Insert => system.style(Role::DiffAdded).add_modifier(Modifier::BOLD),
                DiffWordKind::Delete => {
                    system.style(Role::DiffRemoved).add_modifier(Modifier::BOLD)
                }
            }
        };
        let remain = max_x.saturating_sub(x);
        let t = take_display_cols(&escape_diff_text(w.text), usize::from(remain));
        let wcols = display_cols(&t) as u16;
        buffer.set_stringn(x, area.y, &t, usize::from(remain), st);
        x = x.saturating_add(wcols);
        let _ = surface;
    }
    if trailing_ws && x < max_x {
        let m = ws_marker(ascii);
        buffer.set_stringn(x, area.y, m, 1, system.style(Role::Warning));
    }
}

fn paint_split_line(
    buffer: &mut Buffer,
    area: Rect,
    line: &DiffLine<'_>,
    state: &DiffViewState,
    system: &DesignSystem,
    surface: bool,
    ascii: bool,
    colorless: bool,
    cursor: bool,
    in_hunk: bool,
) {
    if area.is_empty() {
        return;
    }
    if !colorless && matches!(line.kind, DiffKind::Added | DiffKind::Removed) {
        buffer.set_style(area, system.style(line.kind.role()));
    }
    let mid = area.width / 2;
    let left = Rect::new(area.x, area.y, mid.saturating_sub(1).max(1), 1);
    let right = Rect::new(
        area.x.saturating_add(mid),
        area.y,
        area.width.saturating_sub(mid),
        1,
    );

    let gutter = if cursor && surface {
        if ascii { ">" } else { "›" }
    } else if in_hunk {
        if ascii { "." } else { "·" }
    } else {
        " "
    };

    let (left_text, left_kind, right_text, right_kind) = match line.kind {
        DiffKind::Removed => (
            strip_diff_prefix(line.text, DiffKind::Removed),
            DiffKind::Removed,
            "",
            DiffKind::Context,
        ),
        DiffKind::Added => (
            line.pair_text.unwrap_or(""),
            DiffKind::Context,
            strip_diff_prefix(line.text, DiffKind::Added),
            DiffKind::Added,
        ),
        DiffKind::Context => {
            let t = strip_diff_prefix(line.text, DiffKind::Context);
            (t, DiffKind::Context, t, DiffKind::Context)
        }
        DiffKind::FileHeader | DiffKind::HunkHeader | DiffKind::Meta => {
            // Span full width in split for headers
            paint_unified_line(
                buffer, area, line, state, system, surface, ascii, colorless, false, false, cursor,
                in_hunk,
            );
            return;
        }
    };

    let ln = if state.show_line_numbers {
        format!(
            "{gutter}{:>4} ",
            line.old_no.map(|n| n.to_string()).unwrap_or_default()
        )
    } else {
        gutter.to_string()
    };
    let rn = if state.show_line_numbers {
        format!(
            "{:>4} ",
            line.new_no.map(|n| n.to_string()).unwrap_or_default()
        )
    } else {
        String::new()
    };

    let ls = kind_style(system, left_kind, colorless, surface, cursor || in_hunk);
    let rs = kind_style(system, right_kind, colorless, surface, cursor || in_hunk);

    let left_s = format!(
        "{ln}{}{}",
        if left_kind == DiffKind::Removed {
            '-'
        } else {
            ' '
        },
        escape_diff_text(left_text)
    );
    let right_s = format!(
        "{rn}{}{}",
        if right_kind == DiffKind::Added {
            '+'
        } else {
            ' '
        },
        escape_diff_text(right_text)
    );

    buffer.set_stringn(
        left.x,
        left.y,
        take_display_cols(&left_s, usize::from(left.width)),
        usize::from(left.width),
        ls,
    );
    // Column divider
    if mid > 0 {
        buffer.set_stringn(
            area.x.saturating_add(mid.saturating_sub(1)),
            area.y,
            if ascii { "|" } else { "│" },
            1,
            system.style(Role::Border),
        );
    }
    buffer.set_stringn(
        right.x,
        right.y,
        take_display_cols(&right_s, usize::from(right.width)),
        usize::from(right.width),
        rs,
    );
}

impl StatefulWidget for &DiffView<'_> {
    type State = DiffViewState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DiffView::render(self, area, buffer, state);
    }
}

impl StatefulWidget for DiffView<'_> {
    type State = DiffViewState;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        DiffView::render(&self, area, buffer, state);
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Sustained-rate paint targets.
pub mod bench {
    /// Lines in a large projected window host should virtualize to.
    pub const VIEWPORT: u16 = 40;
    /// Typical large patch line count.
    pub const LARGE_DIFF_LINES: usize = 50_000;
    /// Max paint cells per frame.
    pub const MAX_PAINT_CELLS: u32 = 40 * 120;
    /// Minimum width for split mode.
    pub const SPLIT_MIN_WIDTH: u16 = 56;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    fn sample_lines() -> Vec<DiffLine<'static>> {
        vec![
            DiffLine::file_header("f0", "diff --git a/a.rs b/a.rs").file_id("a.rs"),
            DiffLine::hunk_header("h0", "@@ -1,3 +1,4 @@")
                .file_id("a.rs")
                .hunk_id("h0"),
            DiffLine::context("c1", "fn main() {")
                .old_no(1)
                .new_no(1)
                .file_id("a.rs")
                .hunk_id("h0"),
            DiffLine::removed("r1", "    let x = 1;")
                .old_no(2)
                .file_id("a.rs")
                .hunk_id("h0")
                .trailing_ws(true),
            DiffLine::added("a1", "    let x = 2;")
                .new_no(2)
                .file_id("a.rs")
                .hunk_id("h0"),
            DiffLine::context("c2", "}")
                .old_no(3)
                .new_no(3)
                .file_id("a.rs")
                .hunk_id("h0"),
            DiffLine::hunk_header("h1", "@@ -10,2 +11,2 @@")
                .file_id("a.rs")
                .hunk_id("h1"),
            DiffLine::removed("r2", "gone")
                .old_no(10)
                .file_id("a.rs")
                .hunk_id("h1"),
            DiffLine::added("a2", "ready 東京 🧪")
                .new_no(11)
                .file_id("a.rs")
                .hunk_id("h1"),
        ]
    }

    fn sample_hunks() -> [DiffHunk; 2] {
        [
            DiffHunk::new(1, 5, "@@ -1,3 +1,4 @@")
                .id("h0")
                .file_id("a.rs"),
            DiffHunk::new(6, 3, "@@ -10,2 +11,2 @@")
                .id("h1")
                .file_id("a.rs"),
        ]
    }

    fn row_text(buffer: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.right())
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect::<String>()
            .trim_end()
            .to_string()
    }

    #[test]
    fn word_kind_ids() {
        assert_eq!(DiffWordKind::Equal.id(), "equal");
        assert_eq!(DiffWordKind::Insert.id(), "insert");
        assert_eq!(DiffWordKind::Delete.id(), "delete");
    }

    #[test]
    fn renders_kind_styles_and_prefixes() {
        let lines = sample_lines();
        let system = DesignSystem::default();
        let hunks = sample_hunks();
        let view = DiffView::new(&lines, &system).hunks(&hunks);
        let area = Rect::new(0, 0, 48, 12);
        let mut buffer = Buffer::empty(area);
        let mut state = DiffViewState::new();
        (&view).render(area, &mut buffer, &mut state);
        let text: String = buffer
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("main") || text.contains("ready") || text.contains('+'),
            "{text}"
        );
        assert!(!state.regions.is_empty());
    }

    #[test]
    fn narrow_forces_unified() {
        let mut state = DiffViewState::new();
        state.mode = DiffMode::Split;
        assert_eq!(state.effective_mode(22), DiffEffectiveMode::Unified);
        assert_eq!(state.effective_mode(80), DiffEffectiveMode::Split);
    }

    #[test]
    fn clamps_over_scroll() {
        let lines = sample_lines();
        let system = DesignSystem::default();
        let view = DiffView::new(&lines, &system);
        let area = Rect::new(0, 0, 40, 3);
        let mut buffer = Buffer::empty(area);
        let mut state = DiffViewState::new();
        state.scroll.set_content_size(1, 100);
        state.scroll.set_viewport(1, 3);
        // force large offset via render path
        for _ in 0..50 {
            let _ = state.scroll_by_lines(1);
        }
        (&view).render(area, &mut buffer, &mut state);
        assert!(state.offset() < 100);
    }

    #[test]
    fn tiny_and_control_text_safe() {
        let lines = [DiffLine::context("1", "a\u{7}b")];
        let system = DesignSystem::default();
        let view = DiffView::new(&lines, &system);
        let mut state = DiffViewState::new();
        (&view).render(
            Rect::new(0, 0, 0, 0),
            &mut Buffer::empty(Rect::new(0, 0, 0, 0)),
            &mut state,
        );
        let area = Rect::new(0, 0, 12, 2);
        let mut buffer = Buffer::empty(area);
        (&view).render(area, &mut buffer, &mut state);
        let t = row_text(&buffer, area, 0);
        assert!(!t.is_empty());
    }

    #[test]
    fn hunk_nav_and_activate() {
        let lines = sample_lines();
        let hunks = sample_hunks();
        let mut state = DiffViewState::new();
        state.sync_metrics(9, 5);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &lines,
                &hunks
            ),
            DiffViewOutcome::HunkCursorMoved { index: 1 }
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &lines,
                &hunks
            ),
            DiffViewOutcome::HunkActivated { index: 1 }
        ));
    }

    #[test]
    fn mode_toggle_and_search() {
        let lines = sample_lines();
        let hunks = sample_hunks();
        let mut state = DiffViewState::new();
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
                &lines,
                &hunks
            ),
            DiffViewOutcome::ModeChanged(DiffMode::Split)
        ));
        let _ = state.handle_key(
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
            &lines,
            &hunks,
        );
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
            &lines,
            &hunks,
        );
        assert!(matches!(out, DiffViewOutcome::SearchChanged(q) if q == "x"));
    }

    #[test]
    fn fold_hunk() {
        let lines = sample_lines();
        let hunks = sample_hunks();
        let mut state = DiffViewState::new();
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            &lines,
            &hunks,
        );
        assert!(matches!(
            out,
            DiffViewOutcome::FoldToggled { folded: true, .. }
        ));
        assert!(state.is_hunk_folded("h0"));
        let system = DesignSystem::default();
        let view = DiffView::new(&lines, &system).hunks(&hunks);
        let area = Rect::new(0, 0, 60, 10);
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf, &mut state);
        // Folded body should reduce regions vs full
        assert!(state.regions.len() < lines.len());
    }

    #[test]
    fn word_diff_paint() {
        let words = [
            DiffWordSpan::new(DiffWordKind::Equal, "let x = "),
            DiffWordSpan::new(DiffWordKind::Delete, "1"),
            DiffWordSpan::new(DiffWordKind::Insert, "2"),
            DiffWordSpan::new(DiffWordKind::Equal, ";"),
        ];
        // static words need static - use separate
        let words = [
            DiffWordSpan::new(DiffWordKind::Equal, "let x = "),
            DiffWordSpan::new(DiffWordKind::Insert, "2"),
        ];
        let lines = [DiffLine::added("a", "let x = 2;").words(&words).new_no(2)];
        let system = DesignSystem::default();
        let mut state = DiffViewState::new();
        state.word_diff = true;
        let view = DiffView::new(&lines, &system);
        let area = Rect::new(0, 0, 40, 3);
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf, &mut state);
        let t = row_text(&buf, area, 0);
        assert!(t.contains('2') || t.contains('+'), "{t}");
    }

    #[test]
    fn split_paint_wide() {
        let lines = sample_lines();
        let system = DesignSystem::default();
        let mut state = DiffViewState::new();
        state.mode = DiffMode::Split;
        let hunks = sample_hunks();
        let view = DiffView::new(&lines, &system).hunks(&hunks);
        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains('│') || text.contains('|') || text.contains('+'),
            "{text}"
        );
    }

    #[test]
    fn anchor_restore() {
        let lines = sample_lines();
        let hunks = sample_hunks();
        let mut state = DiffViewState::new();
        state.cursor = 4;
        state.hunk_cursor = 0;
        state.capture_anchor(&lines, &hunks);
        state.cursor = 0;
        state.restore_anchor(&lines, &hunks);
        assert_eq!(state.cursor, 4);
    }

    #[test]
    fn mouse_click_region() {
        let lines = sample_lines();
        let hunks = sample_hunks();
        let system = DesignSystem::default();
        let mut state = DiffViewState::new();
        let view = DiffView::new(&lines, &system).hunks(&hunks);
        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        (&view).render(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        let r = &state.regions[0];
        let click = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(r.area.x, r.area.y),
            modifiers: KeyModifiers::NONE,
        };
        assert!(matches!(
            state.handle_mouse(click, &lines, &hunks),
            DiffViewOutcome::CursorMoved { .. }
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let lines = sample_lines();
        let hunks = sample_hunks();
        let mut state = DiffViewState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
                &lines,
                &hunks
            ),
            DiffViewOutcome::Ignored
        ));
    }

    #[test]
    fn sustained_viewport_paint() {
        let owned: Vec<(String, String)> = (0..80)
            .map(|i| (i.to_string(), format!("line {i} body")))
            .collect();
        let lines: Vec<DiffLine<'_>> = owned
            .iter()
            .map(|(id, t)| {
                if id.parse::<usize>().unwrap_or(0) % 3 == 0 {
                    DiffLine::added(id.as_str(), t.as_str())
                } else if id.parse::<usize>().unwrap_or(0) % 3 == 1 {
                    DiffLine::removed(id.as_str(), t.as_str())
                } else {
                    DiffLine::context(id.as_str(), t.as_str())
                }
            })
            .collect();
        let system = DesignSystem::default();
        let mut state = DiffViewState::new();
        let view = DiffView::new(&lines, &system);
        let area = Rect::new(0, 0, 72, 22);
        let mut buf = Buffer::empty(area);
        for _ in 0..40 {
            (&view).render(area, &mut buf, &mut state);
        }
        assert!(state.regions.len() <= 25);
    }

    #[test]
    fn fuzz_kinds_and_modes() {
        for kind in [
            DiffKind::Context,
            DiffKind::Added,
            DiffKind::Removed,
            DiffKind::FileHeader,
            DiffKind::HunkHeader,
            DiffKind::Meta,
        ] {
            assert!(!kind.id().is_empty());
            let _ = kind.prefix();
        }
        for mode in [DiffMode::Auto, DiffMode::Unified, DiffMode::Split] {
            assert!(!mode.id().is_empty());
        }
        assert_eq!(bench::SPLIT_MIN_WIDTH, 56);
    }

    #[test]
    fn escape_controls() {
        assert!(escape_diff_text("a\nb").contains("\\n"));
    }
}
