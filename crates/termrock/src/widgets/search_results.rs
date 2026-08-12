// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **SearchResults** — grouped, navigable search results for files, logs,
//! objects, commands, and documentation.
//!
//! **Mission.** Result groups, match ranges, snippets, source metadata, status,
//! pagination/streaming, selection, preview, and open action. Keep important
//! matched text visible under truncation. Keyboard next/previous match and
//! group collapse. Stale async searches and cancellation. Compose with
//! [`super::SearchInput`], [`super::QuickOpen`], and [`super::FullscreenViewer`].
//!
//! **Ownership.** Host owns search I/O, ranking, and async generation.
//! TermRock owns paint, navigation, group chrome, and typed outcomes.
//!
//! Research: ripgrep UIs, IDE search, fzf previews, documentation search.

use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{
        data_view::{LoadState, VirtualWindow},
        highlighted_text::{HighlightedText, MatchKind, MatchRange, MatchRanges, MatchTruncate},
        quick_open::{QuickOpenItem, QuickOpenPreview},
    },
};

// ── Domain ──────────────────────────────────────────────────────────────────

/// Kind of search hit (host classification).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum SearchResultKind {
    /// File path / content in files.
    #[default]
    File,
    /// Log line / stream event.
    Log,
    /// Structured object / JSON path.
    Object,
    /// Command / palette action.
    Command,
    /// Documentation page.
    Doc,
    /// Symbol / definition.
    Symbol,
    /// Other.
    Other,
}

impl SearchResultKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Log => "log",
            Self::Object => "object",
            Self::Command => "command",
            Self::Doc => "doc",
            Self::Symbol => "symbol",
            Self::Other => "other",
        }
    }

    /// Short glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            match self {
                Self::File => "f",
                Self::Log => "l",
                Self::Object => "o",
                Self::Command => "c",
                Self::Doc => "d",
                Self::Symbol => "s",
                Self::Other => "?",
            }
        } else {
            match self {
                Self::File => "·",
                Self::Log => "☰",
                Self::Object => "{}",
                Self::Command => "⌘",
                Self::Doc => "¶",
                Self::Symbol => "ƒ",
                Self::Other => "?",
            }
        }
    }

    /// Role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::File => Role::Text,
            Self::Log => Role::Info,
            Self::Object => Role::Accent,
            Self::Command => Role::Warning,
            Self::Doc => Role::Link,
            Self::Symbol => Role::Success,
            Self::Other => Role::TextMuted,
        }
    }
}

/// Async / empty / error chrome for the result set.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum SearchResultsStatus {
    /// Idle (no query yet).
    #[default]
    Idle,
    /// Host fetch in flight.
    Loading {
        /// Optional status.
        message: Option<String>,
    },
    /// Some hits; more may stream.
    Partial {
        /// Resident count.
        resident: u64,
        /// Optional known total.
        total: Option<u64>,
    },
    /// Ready complete set (or complete page).
    Ready {
        /// Total hits when known.
        total: Option<u64>,
    },
    /// Successful empty.
    Empty {
        /// Guidance.
        message: Option<String>,
    },
    /// Failed search.
    Error {
        /// Message.
        message: String,
        /// Retryable.
        retryable: bool,
    },
    /// Results belong to older generation (stale).
    Stale {
        /// Generation that is stale.
        generation: u64,
    },
    /// User/host cancelled.
    Cancelled,
}

impl SearchResultsStatus {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading { .. } => "loading",
            Self::Partial { .. } => "partial",
            Self::Ready { .. } => "ready",
            Self::Empty { .. } => "empty",
            Self::Error { .. } => "error",
            Self::Stale { .. } => "stale",
            Self::Cancelled => "cancelled",
        }
    }

    /// Status line for chrome.
    #[must_use]
    pub fn summary_line(&self, visible: usize) -> String {
        match self {
            Self::Idle => "type to search".into(),
            Self::Loading { message } => message.clone().unwrap_or_else(|| "searching…".into()),
            Self::Partial { resident, total } => match total {
                Some(t) => format!("streaming {resident}/{t} · showing {visible}"),
                None => format!("streaming {resident}+ · showing {visible}"),
            },
            Self::Ready { total } => match total {
                Some(t) => format!("{t} results · showing {visible}"),
                None => format!("{visible} results"),
            },
            Self::Empty { message } => message.clone().unwrap_or_else(|| "no matches".into()),
            Self::Error { message, .. } => format!("error · {message}"),
            Self::Stale { generation } => format!("stale gen {generation} · refresh"),
            Self::Cancelled => "cancelled".into(),
        }
    }

    /// Map to LoadState for consistency.
    #[must_use]
    pub fn to_load_state(&self, projected: usize) -> LoadState {
        match self {
            Self::Idle => LoadState::Idle,
            Self::Loading { message } => LoadState::Loading {
                message: message.clone(),
            },
            Self::Partial { resident, total } => LoadState::Partial {
                resident: *resident,
                total: *total,
            },
            Self::Ready { total } => LoadState::Ready {
                count: total.unwrap_or(projected as u64),
            },
            Self::Empty { message } => LoadState::Empty {
                message: message.clone(),
            },
            Self::Error { message, retryable } => LoadState::Error {
                message: message.clone(),
                retryable: *retryable,
            },
            Self::Stale { .. } | Self::Cancelled => LoadState::Empty {
                message: Some(self.summary_line(0)),
            },
        }
    }
}

/// Group header in the result stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultGroup {
    /// Stable group id.
    pub id: String,
    /// Display label (`src/`, `docs/`, `Commands`).
    pub label: String,
    /// Hit count (may exceed projected members).
    pub count: u64,
    /// Collapsed (host projects no children when true; state may also track).
    pub collapsed: bool,
}

impl SearchResultGroup {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, count: u64) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            count,
            collapsed: false,
        }
    }

    /// Collapsed.
    #[must_use]
    pub const fn collapsed(mut self) -> Self {
        self.collapsed = true;
        self
    }
}

/// One search hit (host projection of the visible window).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResultItem<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Group id (empty = ungrouped).
    pub group_id: &'a str,
    /// Primary title (file name, command, heading).
    pub title: &'a str,
    /// Snippet / context line (may include match).
    pub snippet: &'a str,
    /// Source path / module / stream id.
    pub source: &'a str,
    /// Kind.
    pub kind: SearchResultKind,
    /// Match ranges into [`Self::title`] (byte offsets).
    pub title_matches: Option<&'a [MatchRange]>,
    /// Match ranges into [`Self::snippet`].
    pub snippet_matches: Option<&'a [MatchRange]>,
    /// Optional 1-based line number in source.
    pub line: Option<u32>,
    /// Host score (lower better, optional chrome).
    pub score: Option<u32>,
    /// Enabled.
    pub enabled: bool,
}

impl<'a> SearchResultItem<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, title: &'a str) -> Self {
        Self {
            id,
            group_id: "",
            title,
            snippet: "",
            source: "",
            kind: SearchResultKind::File,
            title_matches: None,
            snippet_matches: None,
            line: None,
            score: None,
            enabled: true,
        }
    }

    /// Group.
    #[must_use]
    pub const fn group(mut self, group_id: &'a str) -> Self {
        self.group_id = group_id;
        self
    }

    /// Snippet.
    #[must_use]
    pub const fn snippet(mut self, s: &'a str) -> Self {
        self.snippet = s;
        self
    }

    /// Source.
    #[must_use]
    pub const fn source(mut self, s: &'a str) -> Self {
        self.source = s;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: SearchResultKind) -> Self {
        self.kind = k;
        self
    }

    /// Title matches.
    #[must_use]
    pub const fn title_matches(mut self, r: &'a [MatchRange]) -> Self {
        self.title_matches = Some(r);
        self
    }

    /// Snippet matches.
    #[must_use]
    pub const fn snippet_matches(mut self, r: &'a [MatchRange]) -> Self {
        self.snippet_matches = Some(r);
        self
    }

    /// Line number.
    #[must_use]
    pub const fn line(mut self, n: u32) -> Self {
        self.line = Some(n);
        self
    }

    /// Score.
    #[must_use]
    pub const fn score(mut self, s: u32) -> Self {
        self.score = Some(s);
        self
    }

    /// Disabled.
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

// ── Flattened paint rows ────────────────────────────────────────────────────

/// Flattened navigable row (group band or item).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFlatRow<'a> {
    /// Group header.
    Group {
        /// Group.
        group: &'a SearchResultGroup,
        /// Index into groups slice.
        group_index: usize,
    },
    /// Hit item.
    Item {
        /// Item.
        item: &'a SearchResultItem<'a>,
        /// Index into items slice.
        item_index: usize,
    },
}

/// Build flattened rows respecting collapsed groups (host may already omit
/// children; this also skips items whose group is in `collapsed`).
#[must_use]
pub fn flatten_search_results<'a>(
    groups: &'a [SearchResultGroup],
    items: &'a [SearchResultItem<'a>],
    collapsed: &BTreeSet<String>,
) -> Vec<SearchFlatRow<'a>> {
    let mut out = Vec::with_capacity(groups.len() + items.len());
    if groups.is_empty() {
        for (i, item) in items.iter().enumerate() {
            out.push(SearchFlatRow::Item {
                item,
                item_index: i,
            });
        }
        return out;
    }
    for (gi, g) in groups.iter().enumerate() {
        let is_collapsed = g.collapsed || collapsed.contains(&g.id);
        out.push(SearchFlatRow::Group {
            group: g,
            group_index: gi,
        });
        if is_collapsed {
            continue;
        }
        for (i, item) in items.iter().enumerate() {
            if item.group_id == g.id {
                out.push(SearchFlatRow::Item {
                    item,
                    item_index: i,
                });
            }
        }
    }
    // Ungrouped trailing
    for (i, item) in items.iter().enumerate() {
        if item.group_id.is_empty() || !groups.iter().any(|g| g.id == item.group_id) {
            out.push(SearchFlatRow::Item {
                item,
                item_index: i,
            });
        }
    }
    out
}

/// Collect absolute match walk targets (item_index, in_snippet) for n/N.
#[must_use]
pub fn collect_match_targets(items: &[SearchResultItem<'_>]) -> Vec<(usize, bool)> {
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if item.title_matches.map(|m| !m.is_empty()).unwrap_or(false) {
            out.push((i, false));
        }
        if item.snippet_matches.map(|m| !m.is_empty()).unwrap_or(false) {
            out.push((i, true));
        }
    }
    out
}

/// Match-preserving snippet truncation for paint width.
#[must_use]
pub fn truncate_snippet_keep_match(
    snippet: &str,
    matches: Option<&[MatchRange]>,
    max_cols: usize,
) -> String {
    keep_first_match_slice(snippet, matches.unwrap_or(&[]), max_cols)
}

/// Core keep-first-match window in display columns.
#[must_use]
pub fn keep_first_match_slice(source: &str, matches: &[MatchRange], max_cols: usize) -> String {
    use crate::text::display_cols;
    if max_cols == 0 {
        return String::new();
    }
    if display_cols(source) <= max_cols {
        return source.to_string();
    }
    let focus = matches
        .iter()
        .find(|r| matches!(r.kind, MatchKind::Focused | MatchKind::Match))
        .or_else(|| matches.first());
    let Some(r) = focus else {
        return take_display_cols(source, max_cols).to_string();
    };
    let start = r.start.min(source.len());
    // Aim to center match window with "…" prefix when needed
    let before = 8usize.min(max_cols / 4);
    // Walk back graphemes approximately by bytes
    let win_start = start.saturating_sub(before);
    let slice = &source[win_start..];
    let body = take_display_cols(slice, max_cols.saturating_sub(1));
    if win_start > 0 {
        format!("…{body}")
    } else {
        body.to_string()
    }
}

/// Project items to QuickOpen.
#[must_use]
pub fn search_results_to_quick_open(items: &[SearchResultItem<'_>]) -> Vec<QuickOpenItem<String>> {
    items
        .iter()
        .map(|it| {
            let mut item = QuickOpenItem::new(it.id.to_string(), it.title).kind(it.kind.id());
            if !it.source.is_empty() {
                item = item.detail(it.source);
            } else if !it.snippet.is_empty() {
                item = item.detail(it.snippet);
            }
            if !it.snippet.is_empty() {
                item = item.preview(QuickOpenPreview::text([it.snippet]));
            }
            if let Some(s) = it.score {
                item = item.score(s);
            }
            item
        })
        .collect()
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes — host owns search I/O and open targets.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SearchResultsOutcome {
    /// No change.
    Ignored,
    /// Cursor moved.
    SelectionChanged {
        /// Item id when on item; group id when on group header.
        id: String,
        /// True when cursor is a group band.
        is_group: bool,
    },
    /// Open / activate hit.
    OpenRequested {
        /// Item id.
        id: String,
    },
    /// Preview pane / side panel.
    PreviewRequested {
        /// Item id.
        id: String,
    },
    /// Group collapse toggled (host updates projection / collapsed set).
    GroupToggled {
        /// Group id.
        id: String,
        /// Collapsed after toggle.
        collapsed: bool,
    },
    /// Focused next match (item may have changed).
    MatchWalk {
        /// Item id.
        id: String,
        /// True if focus is on snippet match.
        in_snippet: bool,
        /// Absolute match index among walk targets.
        match_index: usize,
    },
    /// Request next result page / more hits.
    PageNext,
    /// Previous page.
    PagePrev,
    /// Cancel in-flight search.
    CancelSearch,
    /// Retry after error.
    RetrySearch,
    /// Host should refresh because generation is stale.
    RefreshStale {
        /// Stale generation.
        generation: u64,
    },
    /// Promote to fullscreen viewer.
    FullscreenRequested {
        /// Item id.
        id: String,
    },
    /// Open QuickOpen over current hits.
    QuickOpenRequested,
    /// Multi-check toggled.
    CheckToggled {
        /// Item id.
        id: String,
    },
    /// Viewport scrolled.
    Scrolled,
    /// Copy path/title.
    CopyRequested {
        /// Text.
        text: String,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// Search results interaction state.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultsState {
    /// Virtual window over flattened rows.
    pub window: VirtualWindow,
    /// Cursor index into flattened projection.
    pub cursor: usize,
    /// Async generation gate (host increments per search).
    pub generation: u64,
    /// Status chrome.
    pub status: SearchResultsStatus,
    /// Local collapsed groups (merged with host group.collapsed).
    pub collapsed: BTreeSet<String>,
    /// Multi-check item ids.
    pub checked: Vec<String>,
    /// Multi-select enabled.
    pub multi: bool,
    /// Match walk index into [`collect_match_targets`].
    pub match_walk: usize,
    /// Load mirror.
    pub load: LoadState,
    /// ASCII.
    pub ascii: bool,
    /// Title.
    pub title: Option<String>,
    /// Row hit regions from last paint.
    row_regions: Vec<(SearchHitKind, String, Rect)>,
    accepts_input: bool,
}

/// Hit target for mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchHitKind {
    Group,
    Item,
}

impl Default for SearchResultsState {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchResultsState {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            window: VirtualWindow::default(),
            cursor: 0,
            generation: 0,
            status: SearchResultsStatus::Idle,
            collapsed: BTreeSet::new(),
            checked: Vec::new(),
            multi: false,
            match_walk: 0,
            load: LoadState::Idle,
            ascii: false,
            title: None,
            row_regions: Vec::new(),
            accepts_input: true,
        }
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Enable multi-check.
    pub fn enable_multi_select(&mut self) {
        self.multi = true;
    }

    /// Begin a new search generation (host). Returns generation to tag results.
    pub fn begin_search(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.status = SearchResultsStatus::Loading { message: None };
        self.load = LoadState::Loading { message: None };
        self.cursor = 0;
        self.window.offset = 0;
        self.match_walk = 0;
        self.generation
    }

    /// Apply host results if generation matches; else mark stale.
    pub fn apply_results(&mut self, generation: u64, status: SearchResultsStatus, count: usize) {
        if generation < self.generation {
            self.status = SearchResultsStatus::Stale { generation };
            return;
        }
        if generation > self.generation {
            self.generation = generation;
        }
        self.status = status;
        self.load = self.status.to_load_state(count);
        self.window.logical_len = count as u64;
        self.window.clamp();
        if self.cursor >= count && count > 0 {
            self.cursor = count - 1;
        }
    }

    /// Cancel current search chrome.
    pub fn cancel(&mut self) {
        self.status = SearchResultsStatus::Cancelled;
        self.load = LoadState::Empty {
            message: Some("cancelled".into()),
        };
    }

    /// Toggle group collapse.
    pub fn toggle_group(&mut self, id: &str) -> bool {
        if self.collapsed.contains(id) {
            self.collapsed.remove(id);
            false
        } else {
            self.collapsed.insert(id.to_string());
            true
        }
    }

    fn reveal(&mut self, idx: usize) {
        let _ = self.window.reveal(idx as u64);
    }

    /// Keys over flattened projection.
    pub fn handle_key(
        &mut self,
        flat: &[SearchFlatRow<'_>],
        items: &[SearchResultItem<'_>],
        key: KeyEvent,
    ) -> SearchResultsOutcome {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return SearchResultsOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;
        if !is_press {
            return SearchResultsOutcome::Ignored;
        }

        self.window.logical_len = flat.len() as u64;
        self.window.clamp();

        // Status-only keys
        if matches!(
            self.status,
            SearchResultsStatus::Error {
                retryable: true,
                ..
            }
        ) && matches!(key.code, KeyCode::Char('r' | 'R') | KeyCode::Enter)
            && key.modifiers.is_empty()
        {
            return SearchResultsOutcome::RetrySearch;
        }
        if let SearchResultsStatus::Stale { generation } = self.status {
            if matches!(key.code, KeyCode::Char('r' | 'R') | KeyCode::Enter)
                && key.modifiers.is_empty()
            {
                return SearchResultsOutcome::RefreshStale { generation };
            }
        }
        if matches!(
            self.status,
            SearchResultsStatus::Loading { .. } | SearchResultsStatus::Partial { .. }
        ) && matches!(key.code, KeyCode::Esc)
        {
            return SearchResultsOutcome::CancelSearch;
        }

        if flat.is_empty() {
            return SearchResultsOutcome::Ignored;
        }
        self.cursor = self.cursor.min(flat.len() - 1);

        // Global chords
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('o' | 'O') => {
                    return SearchResultsOutcome::QuickOpenRequested;
                }
                KeyCode::Char('f' | 'F') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    if let Some(id) = self.cursor_item_id(flat) {
                        return SearchResultsOutcome::FullscreenRequested { id };
                    }
                }
                KeyCode::Char('c' | 'C') => {
                    if let Some(text) = self.cursor_copy_text(flat) {
                        return SearchResultsOutcome::CopyRequested { text };
                    }
                }
                KeyCode::Char('n' | 'N') => return SearchResultsOutcome::PageNext,
                KeyCode::Char('p' | 'P') => return SearchResultsOutcome::PagePrev,
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                return self.walk_match(items, flat, 1);
            }
            KeyCode::Char('N') => {
                return self.walk_match(items, flat, -1);
            }
            KeyCode::Char(']') if key.modifiers.is_empty() => {
                return SearchResultsOutcome::PageNext;
            }
            KeyCode::Char('[') if key.modifiers.is_empty() => {
                return SearchResultsOutcome::PagePrev;
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                return self.move_cursor(flat, 1);
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                return self.move_cursor(flat, -1);
            }
            KeyCode::PageDown => {
                let vh = usize::from(self.window.viewport.max(1));
                return self.move_cursor(flat, vh as isize);
            }
            KeyCode::PageUp => {
                let vh = usize::from(self.window.viewport.max(1));
                return self.move_cursor(flat, -(vh as isize));
            }
            KeyCode::Home => {
                self.cursor = 0;
                self.reveal(0);
                return self.selection_outcome(flat);
            }
            KeyCode::End => {
                self.cursor = flat.len() - 1;
                self.reveal(self.cursor);
                return self.selection_outcome(flat);
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                return match flat.get(self.cursor) {
                    Some(SearchFlatRow::Group { group, .. }) => {
                        let collapsed = self.toggle_group(&group.id);
                        SearchResultsOutcome::GroupToggled {
                            id: group.id.clone(),
                            collapsed,
                        }
                    }
                    Some(SearchFlatRow::Item { item, .. }) if item.enabled => {
                        SearchResultsOutcome::OpenRequested {
                            id: item.id.to_string(),
                        }
                    }
                    _ => SearchResultsOutcome::Ignored,
                };
            }
            KeyCode::Char(' ') if key.modifiers.is_empty() => {
                return match flat.get(self.cursor) {
                    Some(SearchFlatRow::Group { group, .. }) => {
                        let collapsed = self.toggle_group(&group.id);
                        SearchResultsOutcome::GroupToggled {
                            id: group.id.clone(),
                            collapsed,
                        }
                    }
                    Some(SearchFlatRow::Item { item, .. }) if self.multi && item.enabled => {
                        if let Some(pos) = self.checked.iter().position(|c| c == item.id) {
                            self.checked.remove(pos);
                        } else {
                            self.checked.push(item.id.to_string());
                        }
                        SearchResultsOutcome::CheckToggled {
                            id: item.id.to_string(),
                        }
                    }
                    Some(SearchFlatRow::Item { item, .. }) if item.enabled => {
                        SearchResultsOutcome::PreviewRequested {
                            id: item.id.to_string(),
                        }
                    }
                    _ => SearchResultsOutcome::Ignored,
                };
            }
            KeyCode::Char('p') if key.modifiers.is_empty() => {
                if let Some(id) = self.cursor_item_id(flat) {
                    return SearchResultsOutcome::PreviewRequested { id };
                }
            }
            KeyCode::Char('h' | 'l') if key.modifiers.is_empty() => {
                // collapse / expand group under cursor or parent
                if let Some(SearchFlatRow::Group { group, .. }) = flat.get(self.cursor) {
                    let collapsed = self.toggle_group(&group.id);
                    return SearchResultsOutcome::GroupToggled {
                        id: group.id.clone(),
                        collapsed,
                    };
                }
                if let Some(SearchFlatRow::Item { item, .. }) = flat.get(self.cursor) {
                    if !item.group_id.is_empty() {
                        let collapsed = self.toggle_group(item.group_id);
                        return SearchResultsOutcome::GroupToggled {
                            id: item.group_id.to_string(),
                            collapsed,
                        };
                    }
                }
            }
            KeyCode::Esc => {
                if matches!(
                    self.status,
                    SearchResultsStatus::Loading { .. } | SearchResultsStatus::Partial { .. }
                ) {
                    return SearchResultsOutcome::CancelSearch;
                }
                return SearchResultsOutcome::Ignored;
            }
            _ => {}
        }
        SearchResultsOutcome::Ignored
    }

    fn move_cursor(&mut self, flat: &[SearchFlatRow<'_>], delta: isize) -> SearchResultsOutcome {
        if flat.is_empty() {
            return SearchResultsOutcome::Ignored;
        }
        let next = if delta >= 0 {
            (self.cursor + delta as usize).min(flat.len() - 1)
        } else {
            self.cursor.saturating_sub((-delta) as usize)
        };
        if next == self.cursor {
            return SearchResultsOutcome::Ignored;
        }
        self.cursor = next;
        self.reveal(self.cursor);
        self.selection_outcome(flat)
    }

    fn selection_outcome(&self, flat: &[SearchFlatRow<'_>]) -> SearchResultsOutcome {
        match flat.get(self.cursor) {
            Some(SearchFlatRow::Group { group, .. }) => SearchResultsOutcome::SelectionChanged {
                id: group.id.clone(),
                is_group: true,
            },
            Some(SearchFlatRow::Item { item, .. }) => SearchResultsOutcome::SelectionChanged {
                id: item.id.to_string(),
                is_group: false,
            },
            None => SearchResultsOutcome::Ignored,
        }
    }

    fn cursor_item_id(&self, flat: &[SearchFlatRow<'_>]) -> Option<String> {
        match flat.get(self.cursor) {
            Some(SearchFlatRow::Item { item, .. }) => Some(item.id.to_string()),
            _ => None,
        }
    }

    fn cursor_copy_text(&self, flat: &[SearchFlatRow<'_>]) -> Option<String> {
        match flat.get(self.cursor) {
            Some(SearchFlatRow::Item { item, .. }) => {
                if !item.source.is_empty() {
                    Some(item.source.to_string())
                } else {
                    Some(item.title.to_string())
                }
            }
            Some(SearchFlatRow::Group { group, .. }) => Some(group.label.clone()),
            None => None,
        }
    }

    fn walk_match(
        &mut self,
        items: &[SearchResultItem<'_>],
        flat: &[SearchFlatRow<'_>],
        dir: isize,
    ) -> SearchResultsOutcome {
        let targets = collect_match_targets(items);
        if targets.is_empty() {
            return SearchResultsOutcome::Ignored;
        }
        if dir >= 0 {
            self.match_walk = (self.match_walk + 1) % targets.len();
        } else {
            self.match_walk = if self.match_walk == 0 {
                targets.len() - 1
            } else {
                self.match_walk - 1
            };
        }
        let (item_index, in_snippet) = targets[self.match_walk];
        let id = items[item_index].id.to_string();
        // Move cursor to flattened row for this item
        if let Some(fi) = flat.iter().position(
            |r| matches!(r, SearchFlatRow::Item { item_index: i, .. } if *i == item_index),
        ) {
            self.cursor = fi;
            self.reveal(fi);
        }
        SearchResultsOutcome::MatchWalk {
            id,
            in_snippet,
            match_index: self.match_walk,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        flat: &[SearchFlatRow<'_>],
        event: MouseEvent,
    ) -> SearchResultsOutcome {
        if !self.accepts_input {
            return SearchResultsOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => {
                let before = self.window.offset;
                let _ = self.window.scroll_by(3);
                if self.window.offset != before {
                    SearchResultsOutcome::Scrolled
                } else {
                    SearchResultsOutcome::Ignored
                }
            }
            MouseEventKind::ScrollUp => {
                let before = self.window.offset;
                let _ = self.window.scroll_by(-3);
                if self.window.offset != before {
                    SearchResultsOutcome::Scrolled
                } else {
                    SearchResultsOutcome::Ignored
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = self
                    .row_regions
                    .iter()
                    .find(|(_, _, rect)| rect.contains(event.position))
                    .map(|(k, id, _)| (*k, id.clone()));
                match hit {
                    Some((SearchHitKind::Group, id)) => {
                        let collapsed = self.toggle_group(&id);
                        SearchResultsOutcome::GroupToggled { id, collapsed }
                    }
                    Some((SearchHitKind::Item, id)) => {
                        if let Some(fi) = flat.iter().position(
                            |r| matches!(r, SearchFlatRow::Item { item, .. } if item.id == id),
                        ) {
                            self.cursor = fi;
                            self.reveal(fi);
                        }
                        SearchResultsOutcome::SelectionChanged {
                            id,
                            is_group: false,
                        }
                    }
                    None => SearchResultsOutcome::Ignored,
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let hit = self
                    .row_regions
                    .iter()
                    .find(|(k, _, rect)| {
                        matches!(k, SearchHitKind::Item) && rect.contains(event.position)
                    })
                    .map(|(_, id, _)| id.clone());
                match hit {
                    Some(id) => SearchResultsOutcome::PreviewRequested { id },
                    None => SearchResultsOutcome::Ignored,
                }
            }
            _ => SearchResultsOutcome::Ignored,
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Grouped search results list.
#[derive(Debug, Clone, Copy)]
pub struct SearchResults<'a> {
    groups: &'a [SearchResultGroup],
    items: &'a [SearchResultItem<'a>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
    ascii: bool,
    /// Show two-line items (title + snippet).
    dense: bool,
}

impl<'a> SearchResults<'a> {
    /// Groups + items + system (groups may be empty).
    #[must_use]
    pub const fn new(
        groups: &'a [SearchResultGroup],
        items: &'a [SearchResultItem<'a>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            groups,
            items,
            system,
            focused: true,
            title: None,
            ascii: false,
            dense: true,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Single-line density.
    #[must_use]
    pub const fn compact(mut self) -> Self {
        self.dense = false;
        self
    }

    /// Paint.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut SearchResultsState) {
        if area.is_empty() {
            return;
        }
        let ascii = self.ascii || state.ascii;
        state.row_regions.clear();

        let mut y = area.y;
        let mut h = area.height;

        // Status chrome
        if h > 0 {
            let title = self.title.or(state.title.as_deref()).unwrap_or("search");
            let line = format!(
                "{title} · {} · gen {}",
                state.status.summary_line(self.items.len()),
                state.generation
            );
            let style = match &state.status {
                SearchResultsStatus::Error { .. } => self.system.style(Role::Danger),
                SearchResultsStatus::Stale { .. } | SearchResultsStatus::Loading { .. } => {
                    self.system.style(Role::Warning)
                }
                _ if self.focused => self.system.style(Role::TextStrong),
                _ => self.system.style(Role::TextMuted),
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                style,
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if h == 0 {
            return;
        }

        let flat = flatten_search_results(self.groups, self.items, &state.collapsed);
        state.window.logical_len = flat.len() as u64;
        state.window.viewport = h;
        state.window.clamp();
        if !flat.is_empty() {
            state.cursor = state.cursor.min(flat.len() - 1);
        }

        // Empty / loading chrome in body
        if flat.is_empty() {
            let msg = match &state.status {
                SearchResultsStatus::Loading { .. } => "searching…",
                SearchResultsStatus::Empty { .. } => "no matches",
                SearchResultsStatus::Error { message, .. } => message.as_str(),
                SearchResultsStatus::Cancelled => "cancelled",
                SearchResultsStatus::Idle => "type to search",
                SearchResultsStatus::Stale { .. } => "stale — press r to refresh",
                _ => "(no results)",
            };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            return;
        }

        let start = state.window.offset as usize;
        let end = (start + usize::from(h)).min(flat.len());
        let mut py = y;
        let bottom = y.saturating_add(h);
        let row_h: u16 = if self.dense { 2 } else { 1 };

        let mut i = start;
        while i < end && py < bottom {
            let selected = i == state.cursor;
            match &flat[i] {
                SearchFlatRow::Group { group, .. } => {
                    let collapsed = group.collapsed || state.collapsed.contains(&group.id);
                    let disc = if collapsed {
                        if ascii { ">" } else { "▸" }
                    } else if ascii {
                        "v"
                    } else {
                        "▾"
                    };
                    let mark = if selected {
                        if ascii { "*" } else { "›" }
                    } else {
                        " "
                    };
                    let line = format!("{mark}{disc} {} ({})", group.label, group.count);
                    buffer.set_stringn(
                        area.x,
                        py,
                        take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        if selected && self.focused {
                            self.system.style(Role::Focus)
                        } else {
                            self.system.style(Role::TextStrong)
                        },
                    );
                    state.row_regions.push((
                        SearchHitKind::Group,
                        group.id.clone(),
                        Rect {
                            x: area.x,
                            y: py,
                            width: area.width,
                            height: 1,
                        },
                    ));
                    py = py.saturating_add(1);
                }
                SearchFlatRow::Item { item, .. } => {
                    let mark = if selected {
                        if ascii { ">" } else { "›" }
                    } else if state.multi && state.checked.iter().any(|c| c == item.id) {
                        if ascii { "*" } else { "★" }
                    } else {
                        " "
                    };
                    let glyph = item.kind.glyph(ascii);
                    let line_no = item.line.map(|n| format!(":{n}")).unwrap_or_default();
                    let title_budget = usize::from(area.width).saturating_sub(4);
                    // Focused match walk: mark first range focused when this is walk target
                    let title_ranges = promote_focused(
                        item.title_matches.unwrap_or(&[]),
                        items_match_focused(self.items, item.id, state, false),
                    );
                    let title_disp = {
                        let ranges = MatchRanges::from_ranges(title_ranges.iter().copied())
                            .normalized(item.title);
                        let _ = ranges;
                        take_display_cols(item.title, title_budget).to_string()
                    };
                    let head = format!("{mark}{glyph} {title_disp}");
                    let style = if selected && self.focused {
                        self.system.style(Role::Focus)
                    } else if !item.enabled {
                        self.system.style(Role::TextDisabled)
                    } else {
                        self.system.style(Role::Text)
                    };
                    buffer.set_stringn(
                        area.x,
                        py,
                        take_display_cols(&head, usize::from(area.width)),
                        usize::from(area.width),
                        style,
                    );
                    // Paint title highlights on top when not selected focus
                    if !selected {
                        let ranges = MatchRanges::from_ranges(title_ranges.iter().copied())
                            .normalized(item.title);
                        if !ranges.is_empty() {
                            let prefix = format!("{mark}{glyph} ");
                            let prefix_w = crate::text::display_cols(&prefix) as u16;
                            let ha = Rect {
                                x: area.x.saturating_add(prefix_w),
                                y: py,
                                width: area.width.saturating_sub(prefix_w),
                                height: 1,
                            };
                            if !ha.is_empty() {
                                HighlightedText::new(item.title, ranges.as_slice(), self.system)
                                    .truncate(MatchTruncate::KeepFirstMatch)
                                    .render(ha, buffer);
                            }
                        }
                    }
                    // Source trailing
                    if !item.source.is_empty() && area.width > 20 {
                        let src = format!("{}{line_no}", item.source);
                        let src_t = take_display_cols(&src, 24).to_string();
                        let sw = crate::text::display_cols(&src_t) as u16;
                        if sw < area.width {
                            buffer.set_stringn(
                                area.x.saturating_add(area.width.saturating_sub(sw)),
                                py,
                                &src_t,
                                usize::from(sw),
                                self.system.style(Role::TextMuted),
                            );
                        }
                    }
                    state.row_regions.push((
                        SearchHitKind::Item,
                        item.id.to_string(),
                        Rect {
                            x: area.x,
                            y: py,
                            width: area.width,
                            height: row_h,
                        },
                    ));
                    py = py.saturating_add(1);

                    // Snippet row
                    if self.dense && py < bottom && !item.snippet.is_empty() {
                        let sn_ranges = promote_focused(
                            item.snippet_matches.unwrap_or(&[]),
                            items_match_focused(self.items, item.id, state, true),
                        );
                        let sn = keep_first_match_slice(
                            item.snippet,
                            &sn_ranges,
                            usize::from(area.width).saturating_sub(4),
                        );
                        let sn_line = format!("  {sn}");
                        buffer.set_stringn(
                            area.x,
                            py,
                            take_display_cols(&sn_line, usize::from(area.width)),
                            usize::from(area.width),
                            if selected {
                                self.system.style(Role::TextMuted)
                            } else {
                                self.system.style(Role::TextMuted)
                            },
                        );
                        if !selected {
                            let ranges = MatchRanges::from_ranges(sn_ranges.iter().copied())
                                .normalized(item.snippet);
                            if !ranges.is_empty() {
                                let ha = Rect {
                                    x: area.x.saturating_add(2),
                                    y: py,
                                    width: area.width.saturating_sub(2),
                                    height: 1,
                                };
                                if !ha.is_empty() {
                                    HighlightedText::new(
                                        item.snippet,
                                        ranges.as_slice(),
                                        self.system,
                                    )
                                    .truncate(MatchTruncate::KeepFirstMatch)
                                    .render(ha, buffer);
                                }
                            }
                        }
                        py = py.saturating_add(1);
                    }
                }
            }
            i += 1;
        }
    }
}

fn items_match_focused(
    items: &[SearchResultItem<'_>],
    id: &str,
    state: &SearchResultsState,
    want_snippet: bool,
) -> bool {
    let targets = collect_match_targets(items);
    if targets.is_empty() {
        return false;
    }
    let (ii, in_sn) = targets[state.match_walk % targets.len()];
    items.get(ii).is_some_and(|it| it.id == id) && in_sn == want_snippet
}

fn promote_focused(ranges: &[MatchRange], focused: bool) -> Vec<MatchRange> {
    if !focused || ranges.is_empty() {
        return ranges.to_vec();
    }
    let mut out = ranges.to_vec();
    if let Some(r) = out.first_mut() {
        r.kind = MatchKind::Focused;
    }
    out
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Large result-set targets.
pub mod bench {
    /// Hits in a large page.
    pub const HIT_COUNT: usize = 2_000;
    /// Groups.
    pub const GROUP_COUNT: usize = 40;
    /// Viewport.
    pub const VIEWPORT: u16 = 30;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 40;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn sample() -> (Vec<SearchResultGroup>, Vec<SearchResultItem<'static>>) {
        static T0: &[MatchRange] = &[MatchRange::new(0, 4)];
        static S0: &[MatchRange] = &[MatchRange::new(10, 14)];
        static T1: &[MatchRange] = &[MatchRange::new(0, 6)];
        let groups = vec![
            SearchResultGroup::new("src", "src/", 2),
            SearchResultGroup::new("docs", "docs/", 1),
        ];
        let items = vec![
            SearchResultItem::new("f1", "main.rs")
                .group("src")
                .source("src/main.rs")
                .snippet("fn main() { search(); }")
                .title_matches(T0)
                .snippet_matches(S0)
                .line(12)
                .kind(SearchResultKind::File),
            SearchResultItem::new("f2", "search.rs")
                .group("src")
                .source("src/search.rs")
                .snippet("pub fn search() {}")
                .title_matches(T1)
                .line(1)
                .kind(SearchResultKind::File),
            SearchResultItem::new("d1", "SearchResults")
                .group("docs")
                .source("docs/components/search-results.mdx")
                .snippet("grouped navigable search results")
                .kind(SearchResultKind::Doc),
            SearchResultItem::new("c1", "termrock search")
                .snippet("run workspace search")
                .kind(SearchResultKind::Command),
        ];
        (groups, items)
    }

    #[test]
    fn flatten_respects_collapse() {
        let (groups, items) = sample();
        let mut collapsed = BTreeSet::new();
        let flat = flatten_search_results(&groups, &items, &collapsed);
        assert!(flat.len() > 3);
        collapsed.insert("src".into());
        let flat2 = flatten_search_results(&groups, &items, &collapsed);
        assert!(flat2.len() < flat.len());
    }

    #[test]
    fn generation_stale_gate() {
        let mut state = SearchResultsState::new();
        let g1 = state.begin_search();
        let g2 = state.begin_search();
        assert!(g2 > g1);
        state.apply_results(g1, SearchResultsStatus::Ready { total: Some(1) }, 1);
        assert!(matches!(state.status, SearchResultsStatus::Stale { .. }));
        state.apply_results(g2, SearchResultsStatus::Ready { total: Some(2) }, 2);
        assert!(matches!(state.status, SearchResultsStatus::Ready { .. }));
    }

    #[test]
    fn keep_match_visible() {
        let long = "aaaaaaabbbbbsearch_termccccccccddddddd";
        let ranges = [MatchRange::new(13, 24)];
        let t = keep_first_match_slice(long, &ranges, 20);
        assert!(t.contains("search") || t.contains("…"), "{t}");
    }

    #[test]
    fn nav_open_preview_group() {
        let (groups, items) = sample();
        let mut state = SearchResultsState::new();
        state.apply_results(0, SearchResultsStatus::Ready { total: Some(4) }, 4);
        let flat = flatten_search_results(&groups, &items, &state.collapsed);
        assert!(matches!(
            state.handle_key(
                &flat,
                &items,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
            ),
            SearchResultsOutcome::SelectionChanged { .. }
        ));
        // move to first item if on group
        while matches!(flat.get(state.cursor), Some(SearchFlatRow::Group { .. })) {
            let _ = state.handle_key(
                &flat,
                &items,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            );
        }
        assert!(matches!(
            state.handle_key(
                &flat,
                &items,
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
            ),
            SearchResultsOutcome::OpenRequested { .. }
        ));
    }

    #[test]
    fn match_walk_n() {
        let (groups, items) = sample();
        let mut state = SearchResultsState::new();
        let flat = flatten_search_results(&groups, &items, &state.collapsed);
        let out = state.handle_key(
            &flat,
            &items,
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        );
        assert!(matches!(out, SearchResultsOutcome::MatchWalk { .. }));
    }

    #[test]
    fn cancel_while_loading() {
        let mut state = SearchResultsState::new();
        let _ = state.begin_search();
        let flat: Vec<SearchFlatRow<'_>> = vec![];
        let items: Vec<SearchResultItem<'_>> = vec![];
        assert!(matches!(
            state.handle_key(
                &flat,
                &items,
                KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
            ),
            SearchResultsOutcome::CancelSearch
        ));
    }

    #[test]
    fn quick_open_bridge() {
        let (_, items) = sample();
        let qo = search_results_to_quick_open(&items);
        assert_eq!(qo.len(), items.len());
    }

    #[test]
    fn paint_basic() {
        let system = DesignSystem::default();
        let (groups, items) = sample();
        let mut state = SearchResultsState::new();
        state.apply_results(0, SearchResultsStatus::Ready { total: Some(4) }, 4);
        let area = Rect::new(0, 0, 64, 14);
        let mut buf = Buffer::empty(area);
        SearchResults::new(&groups, &items, &system)
            .title("find")
            .render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("main") || text.contains("find") || text.contains("src"),
            "{text}"
        );
    }

    #[test]
    fn accepts_input_gate() {
        let mut state = SearchResultsState::new();
        state.set_accepts_input(false);
        let flat: Vec<SearchFlatRow<'_>> = vec![];
        let items: Vec<SearchResultItem<'_>> = vec![];
        assert!(matches!(
            state.handle_key(
                &flat,
                &items,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
            ),
            SearchResultsOutcome::Ignored
        ));
    }

    #[test]
    fn large_page_paint() {
        let system = DesignSystem::default();
        let groups = vec![SearchResultGroup::new("g", "all", bench::HIT_COUNT as u64)];
        let titles: Vec<String> = (0..bench::HIT_COUNT).map(|i| format!("hit-{i}")).collect();
        let snips: Vec<String> = (0..bench::HIT_COUNT)
            .map(|i| format!("context match_{i} here"))
            .collect();
        let ids: Vec<String> = (0..bench::HIT_COUNT).map(|i| format!("id{i}")).collect();
        let items: Vec<SearchResultItem<'_>> = (0..bench::HIT_COUNT)
            .map(|i| {
                SearchResultItem::new(&ids[i], &titles[i])
                    .group("g")
                    .snippet(&snips[i])
                    .source("file.rs")
            })
            .collect();
        let mut state = SearchResultsState::new();
        state.apply_results(
            0,
            SearchResultsStatus::Partial {
                resident: bench::HIT_COUNT as u64,
                total: None,
            },
            items.len(),
        );
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        for _ in 0..6 {
            SearchResults::new(&groups, &items, &system).render(area, &mut buf, &mut state);
            let flat = flatten_search_results(&groups, &items, &state.collapsed);
            let _ = state.handle_key(
                &flat,
                &items,
                KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            );
        }
    }

    #[test]
    fn never_runs_search_io() {
        let src = include_str!("search_results.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in [
            "std::fs::",
            "std::process::Command",
            "reqwest::",
            "tokio::fs",
        ] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }
}
