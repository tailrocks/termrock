// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Flagship **CommandPalette** — universal command surface for TermRock apps.
//!
//! **Mission.** Every serious TUI needs a fast, searchable command surface:
//! fuzzy filter, groups, recent + contextual actions, shortcuts, nested pages,
//! optional arguments, previews, async host results with generation gates, and
//! fullscreen promotion on small terminals — without the widget owning I/O or
//! execution side effects.
//!
//! **vs [`super::Picker`].** Picker is a general query+list popup (selects).
//! CommandPalette is the product command surface: richer entry model, history,
//! pages, loading/empty/no-result chrome, keymap/scene projection helpers.
//!
//! Research: VS Code palette, Textual, Posting, Zellij, television, agent TUIs.
use std::collections::VecDeque;

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        CollectionItem, CollectionState, NavigationMove, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PageMove, RovingOrientation,
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_palette_intent,
        place_overlay,
    },
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        HighlightVisual, HighlightedText, Hint, HintBar, MatchKind, MatchRange, MatchRanges,
        MatchTruncate, Surface, SurfaceRecipe, TextInput, TextInputOutcome, TextInputState,
    },
};

/// Default overlay id for a command palette on an [`OverlayStack`].
pub const COMMAND_PALETTE_OVERLAY_ID: &str = "termrock.command_palette";
/// Width at or below which the host should prefer fullscreen placement.
pub const COMMAND_PALETTE_FULLSCREEN_MAX_WIDTH: u16 = 48;
/// Height at or below which the host should prefer fullscreen placement.
pub const COMMAND_PALETTE_FULLSCREEN_MAX_HEIGHT: u16 = 14;
/// Max remembered query history entries.
pub const COMMAND_PALETTE_HISTORY_CAP: usize = 32;

/// Default "still fetching" copy, and its ASCII twin.
///
/// Two constants rather than one gated literal so host-supplied copy survives
/// the ASCII profile: only the *default* is swapped.
const COMMAND_PALETTE_LOADING: &str = "Loading…";
const COMMAND_PALETTE_LOADING_ASCII: &str = "Loading...";

// ── Size / placement ────────────────────────────────────────────────────────

/// Preferred palette size (width × height in cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandPaletteSize {
    /// Preferred width.
    pub width: u16,
    /// Preferred height.
    pub height: u16,
}

impl Default for CommandPaletteSize {
    fn default() -> Self {
        Self {
            width: 56,
            height: 16,
        }
    }
}

impl From<CommandPaletteSize> for OverlaySize {
    fn from(value: CommandPaletteSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 24,
            min_height: 6,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Placement presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CommandPalettePresentation {
    /// Centered floating panel (default).
    #[default]
    Centered,
    /// Near-fullscreen (narrow / tiny terminals).
    Fullscreen,
}

impl CommandPalettePresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Centered => "centered",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Derive presentation from terminal bounds.
#[must_use]
pub fn command_palette_presentation_for_bounds(bounds: Rect) -> CommandPalettePresentation {
    if bounds.width <= COMMAND_PALETTE_FULLSCREEN_MAX_WIDTH
        || bounds.height <= COMMAND_PALETTE_FULLSCREEN_MAX_HEIGHT
    {
        CommandPalettePresentation::Fullscreen
    } else {
        CommandPalettePresentation::Centered
    }
}

/// Centered command-palette rectangle inside `bounds` (upper third).
#[must_use]
pub fn place_command_palette(bounds: Rect, preferred: CommandPaletteSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    if bounds.width <= COMMAND_PALETTE_FULLSCREEN_MAX_WIDTH
        || bounds.height <= COMMAND_PALETTE_FULLSCREEN_MAX_HEIGHT
    {
        return place_overlay(
            bounds,
            None,
            OverlaySize::from(preferred),
            OverlayPolicy::for_kind(OverlayKind::CommandPalette),
        );
    }
    let width = preferred.width.min(bounds.width.saturating_sub(4)).max(24);
    let height = preferred.height.min(bounds.height.saturating_sub(2)).max(6);
    let x = bounds
        .x
        .saturating_add(bounds.width.saturating_sub(width) / 2);
    let y = bounds
        .y
        .saturating_add((bounds.height.saturating_sub(height) / 3).max(1));
    Rect::new(
        x,
        y.min(bounds.bottom().saturating_sub(height)),
        width,
        height,
    )
}

/// Opens (or replaces) the command palette overlay and returns its outcome.
pub fn open_command_palette_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: CommandPaletteSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::command_palette(
            COMMAND_PALETTE_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Dismisses the default command-palette overlay when present.
pub fn dismiss_command_palette_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(COMMAND_PALETTE_OVERLAY_ID))
}

// ── Entry model ─────────────────────────────────────────────────────────────

/// One command row (host-owned projection; owned strings for async rebuilds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandEntry<Id> {
    /// Stable identity.
    pub id: Id,
    /// Primary label.
    pub label: String,
    /// Extra search keywords (not always painted).
    pub keywords: Vec<String>,
    /// Group header key (rows with same group cluster; first emits header).
    pub group: Option<String>,
    /// Shortcut hint (display only).
    pub shortcut: Option<String>,
    /// Whether activatable.
    pub enabled: bool,
    /// Why disabled (hint / empty-state polish).
    pub disabled_reason: Option<String>,
    /// Recent-commands section membership.
    pub recent: bool,
    /// Contextual (current focus) action.
    pub contextual: bool,
    /// Nested page this entry belongs to (`None` = root).
    pub page: Option<String>,
    /// Activating opens this nested page id.
    pub opens_page: Option<String>,
    /// Optional argument prompt; activation enters argument phase.
    pub argument_prompt: Option<String>,
    /// Host command / keymap key.
    pub command: Option<String>,
    /// Optional one-line preview (detail pane / footer).
    pub preview: Option<String>,
    /// Precomputed fuzzy ranges into [`Self::label`] (byte offsets).
    pub match_ranges: Option<MatchRanges>,
    /// Sort score (lower is better); set by [`filter_command_entries`].
    pub score: u32,
}

impl<Id> CommandEntry<Id> {
    /// Enabled leaf command.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            keywords: Vec::new(),
            group: None,
            shortcut: None,
            enabled: true,
            disabled_reason: None,
            recent: false,
            contextual: false,
            page: None,
            opens_page: None,
            argument_prompt: None,
            command: None,
            preview: None,
            match_ranges: None,
            score: 0,
        }
    }

    /// Keywords for search.
    #[must_use]
    pub fn keywords<I, S>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.keywords = iter.into_iter().map(Into::into).collect();
        self
    }

    /// Group name.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    /// Shortcut hint.
    #[must_use]
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// Enabled flag.
    #[must_use]
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// Disabled reason.
    #[must_use]
    pub fn disabled_reason(mut self, r: impl Into<String>) -> Self {
        self.disabled_reason = Some(r.into());
        self
    }

    /// Recent flag.
    #[must_use]
    pub fn recent(mut self, on: bool) -> Self {
        self.recent = on;
        self
    }

    /// Contextual flag.
    #[must_use]
    pub fn contextual(mut self, on: bool) -> Self {
        self.contextual = on;
        self
    }

    /// Page membership.
    #[must_use]
    pub fn page(mut self, p: impl Into<String>) -> Self {
        self.page = Some(p.into());
        self
    }

    /// Opens nested page.
    #[must_use]
    pub fn opens_page(mut self, p: impl Into<String>) -> Self {
        self.opens_page = Some(p.into());
        self
    }

    /// Argument prompt.
    #[must_use]
    pub fn argument_prompt(mut self, p: impl Into<String>) -> Self {
        self.argument_prompt = Some(p.into());
        self
    }

    /// Command key.
    #[must_use]
    pub fn command_key(mut self, k: impl Into<String>) -> Self {
        self.command = Some(k.into());
        self
    }

    /// Preview text.
    #[must_use]
    pub fn preview(mut self, p: impl Into<String>) -> Self {
        self.preview = Some(p.into());
        self
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed palette outcomes (host executes / fetches).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandPaletteOutcome<Id> {
    /// No change.
    Ignored,
    /// Query text changed — host should refilter / async fetch.
    QueryChanged {
        /// Current query.
        query: String,
        /// Generation for stale-result cancellation.
        generation: u64,
    },
    /// Result cursor moved.
    CursorMoved,
    /// Leaf command activated (no args required).
    Activated {
        /// Entry id.
        id: Id,
        /// Optional host command key.
        command: Option<String>,
        /// Argument text when phase was Argument.
        argument: Option<String>,
    },
    /// Command needs arguments — palette entered argument phase.
    NeedArguments {
        /// Entry id.
        id: Id,
        /// Prompt label.
        prompt: String,
    },
    /// Nested page opened.
    PageOpened {
        /// Page id.
        page_id: String,
    },
    /// Nested page closed (back to parent).
    PageClosed,
    /// Left argument phase without executing.
    ArgumentCancelled,
    /// History entry applied to query.
    HistoryApplied {
        /// Restored query.
        query: String,
    },
    /// Loading flag changed.
    LoadingChanged {
        /// Whether loading.
        loading: bool,
    },
    /// Palette dismissed; restore opener focus via OverlayStack.
    Cancelled,
    /// Presentation changed (host may reflow overlay).
    PresentationChanged {
        /// New presentation.
        presentation: CommandPalettePresentation,
    },
}

// ── Filter helpers ──────────────────────────────────────────────────────────

/// Case-insensitive subsequence fuzzy match; returns score + ranges into `haystack`.
///
/// Lower score is better. `None` = no match.
#[must_use]
pub fn fuzzy_match_label(query: &str, haystack: &str) -> Option<(u32, MatchRanges)> {
    if query.is_empty() {
        return Some((0, MatchRanges::default()));
    }
    // Stream both sides: this runs per row on every query change, so neither
    // the query nor the haystack is materialized.
    let mut q = query.chars().map(|c| c.to_ascii_lowercase());
    let mut want = q.next();
    let mut ranges = MatchRanges::default();
    let mut score = 0u32;
    let mut last_match_idx = None::<usize>;
    let mut run_start: Option<(usize, usize)> = None; // byte start, end

    for (byte_i, ch) in haystack.char_indices() {
        let lower = ch.to_ascii_lowercase();
        if want == Some(lower) {
            let end = byte_i + ch.len_utf8();
            match run_start {
                Some((s, _)) => run_start = Some((s, end)),
                None => run_start = Some((byte_i, end)),
            }
            if let Some(prev) = last_match_idx {
                score = score.saturating_add((byte_i as u32).saturating_sub(prev as u32));
            } else {
                score = score.saturating_add(byte_i as u32); // prefer early match
            }
            last_match_idx = Some(byte_i);
            want = q.next();
            if want.is_none() {
                if let Some((s, e)) = run_start.take() {
                    ranges.push(MatchRange::with_kind(s, e, MatchKind::Match));
                }
                break;
            }
        } else if let Some((s, e)) = run_start.take() {
            ranges.push(MatchRange::with_kind(s, e, MatchKind::Match));
        }
    }
    if want.is_some() {
        return None;
    }
    if let Some((s, e)) = run_start {
        ranges.push(MatchRange::with_kind(s, e, MatchKind::Match));
    }
    Some((score, ranges))
}

/// One scored filter match: the borrowed entry plus computed match metadata.
///
/// Filtering used to clone every matching [`CommandEntry`] per frame; the
/// projection now borrows the entry and carries only the score and the
/// highlight ranges computed for the current query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMatch<'a, Id> {
    /// The matched entry.
    pub entry: &'a CommandEntry<Id>,
    /// Sort score (lower is better).
    pub score: u32,
    /// Highlight ranges into [`CommandEntry::label`] (byte offsets).
    pub match_ranges: Option<MatchRanges>,
}

impl<'a, Id> CommandMatch<'a, Id> {
    /// Wrap an entry with match metadata.
    #[must_use]
    pub const fn new(
        entry: &'a CommandEntry<Id>,
        score: u32,
        match_ranges: Option<MatchRanges>,
    ) -> Self {
        Self {
            entry,
            score,
            match_ranges,
        }
    }
}

impl<Id> std::ops::Deref for CommandMatch<'_, Id> {
    type Target = CommandEntry<Id>;

    fn deref(&self) -> &Self::Target {
        self.entry
    }
}

/// Filter + score entries for `query` on the active page.
///
/// Host may replace this with its own scorer; this is the built-in default.
#[must_use]
pub fn filter_command_entries<'a, Id>(
    entries: &'a [CommandEntry<Id>],
    query: &str,
    page: Option<&str>,
) -> Vec<CommandMatch<'a, Id>> {
    let q = query.trim();
    let mut out: Vec<CommandMatch<'a, Id>> = entries
        .iter()
        .filter(|e| e.page.as_deref() == page)
        .filter_map(|e| {
            if q.is_empty() {
                let score = if e.recent {
                    0
                } else if e.contextual {
                    1
                } else {
                    10
                };
                return Some(CommandMatch::new(e, score, None));
            }
            let mut best =
                fuzzy_match_label(q, &e.label).map(|(score, ranges)| (score, Some(ranges)));
            for kw in &e.keywords {
                if let Some((score, _)) = fuzzy_match_label(q, kw) {
                    let score = score.saturating_add(5); // keyword slightly worse
                    best = Some(match best {
                        Some((best_score, best_ranges)) if best_score <= score => {
                            (best_score, best_ranges)
                        }
                        _ => (score, None),
                    });
                }
            }
            best.map(|(score, ranges)| {
                let match_ranges = ranges.filter(|ranges| !ranges.as_slice().is_empty());
                CommandMatch::new(e, score, match_ranges)
            })
        })
        .collect();

    out.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| b.recent.cmp(&a.recent))
            .then_with(|| b.contextual.cmp(&a.contextual))
            .then_with(|| a.label.cmp(&b.label))
    });
    out
}

/// Project keymap bindings into command entries (host supplies id + label).
///
/// `map` receives the action and returns `(id, label, optional group)`.
#[must_use]
pub fn entries_from_keymap<A, Id, F>(
    keymap: &crate::keymap::Keymap<A>,
    mut map: F,
) -> Vec<CommandEntry<Id>>
where
    A: Clone + Copy + PartialEq + 'static,
    F: FnMut(&A, &crate::keymap::KeyBinding<A>) -> (Id, String, Option<String>),
{
    let mut out = Vec::new();
    for binding in keymap.bindings() {
        // Skip internal widget keys; include Shown and HiddenAlias.
        if matches!(binding.visibility(), crate::keymap::Visibility::Internal) {
            continue;
        }
        let (id, label, group) = map(binding.action(), binding);
        let mut e = CommandEntry::new(id, label);
        if let Some(g) = group {
            e = e.group(g);
        }
        if let Some(hint) = binding.hint() {
            e = e.preview(hint.to_string());
        }
        let glyph = binding.glyph().map(|s| s.to_string()).or_else(|| {
            binding
                .chords()
                .first()
                .map(|c| crate::keymap::chord_glyph(Some(*c)).to_string())
        });
        if let Some(sc) = glyph {
            e = e.shortcut(sc);
        }
        out.push(e);
    }
    out
}

// ── State ───────────────────────────────────────────────────────────────────

/// Interaction phase.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CommandPalettePhase {
    /// Browsing / filtering results.
    #[default]
    Browse,
    /// Collecting argument for a pending command.
    Argument {
        /// Pending entry id as display key (host matches).
        entry_key: String,
        /// Prompt.
        prompt: String,
    },
}

/// Command palette state (query, cursor, async generation, pages, history).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteState<Id> {
    query: TextInputState,
    /// Argument draft when in Argument phase.
    argument: TextInputState,
    collection: CollectionState<usize>,
    /// Generation bumped on every query change; host stamps async replies.
    generation: u64,
    /// Last applied result generation (for diagnostics).
    applied_generation: u64,
    accepts_input: bool,
    focused: bool,
    loading: bool,
    phase: CommandPalettePhase,
    /// Nested page stack (top is current).
    page_stack: Vec<(String, String)>, // (id, title)
    /// Query history (newest at back).
    history: VecDeque<String>,
    history_cursor: Option<usize>,
    presentation: CommandPalettePresentation,
    presentation_override: Option<CommandPalettePresentation>,
    /// Hit regions for mouse: (flat result index, rect).
    hits: Vec<(usize, Rect)>,
    /// Row the pointer is over. Hover washes; it never commits.
    hovered: Option<usize>,
    origin: (u16, u16),
    /// Pending activation id while in argument phase.
    pending_id: Option<Id>,
    pending_command: Option<String>,
    /// Scroll offset into visible results (virtual window).
    scroll: usize,
    painted_rows: u16,
}

impl<Id: Clone + PartialEq> Default for CommandPaletteState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id: Clone + PartialEq> CommandPaletteState<Id> {
    /// Empty query; optional initial selection id is host-reconciled after first filter.
    #[must_use]
    pub fn new(_selected: Option<Id>) -> Self {
        Self {
            query: TextInputState::new("")
                .with_allow_empty(true)
                .with_editing(),
            argument: TextInputState::new("")
                .with_allow_empty(true)
                .with_editing(),
            collection: CollectionState::new().orientation(RovingOrientation::Vertical),
            generation: 0,
            applied_generation: 0,
            accepts_input: true,
            focused: true,
            loading: false,
            phase: CommandPalettePhase::Browse,
            page_stack: Vec::new(),
            history: VecDeque::new(),
            history_cursor: None,
            presentation: CommandPalettePresentation::Centered,
            presentation_override: None,
            hits: Vec::new(),
            hovered: None,
            origin: (0, 0),
            pending_id: None,
            pending_command: None,
            scroll: 0,
            painted_rows: 0,
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

    /// Surface focus chrome.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Loading flag (async).
    pub fn set_loading(&mut self, loading: bool) -> CommandPaletteOutcome<Id> {
        if self.loading == loading {
            return CommandPaletteOutcome::Ignored;
        }
        self.loading = loading;
        CommandPaletteOutcome::LoadingChanged { loading }
    }

    /// Whether loading.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Current generation (for async request tags).
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Last applied generation.
    #[must_use]
    pub const fn applied_generation(&self) -> u64 {
        self.applied_generation
    }

    /// Query text.
    #[must_use]
    pub fn query_text(&self) -> &str {
        self.query.value()
    }

    /// Query state.
    #[must_use]
    pub const fn query(&self) -> &TextInputState {
        &self.query
    }

    /// Mutable query.
    pub const fn query_mut(&mut self) -> &mut TextInputState {
        &mut self.query
    }

    /// Argument draft.
    #[must_use]
    pub fn argument_text(&self) -> &str {
        self.argument.value()
    }

    /// Phase.
    #[must_use]
    pub fn phase(&self) -> &CommandPalettePhase {
        &self.phase
    }

    /// Current page id.
    #[must_use]
    pub fn current_page(&self) -> Option<&str> {
        self.page_stack.last().map(|(id, _)| id.as_str())
    }

    /// Current page title.
    #[must_use]
    pub fn current_page_title(&self) -> Option<&str> {
        self.page_stack.last().map(|(_, t)| t.as_str())
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> CommandPalettePresentation {
        self.presentation
    }

    /// Force presentation.
    pub fn set_presentation_override(&mut self, p: Option<CommandPalettePresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Cursor index into visible flat list.
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    /// History snapshot.
    #[must_use]
    pub fn history(&self) -> impl Iterator<Item = &str> {
        self.history.iter().map(String::as_str)
    }

    /// Push query into history (dedup consecutive).
    pub fn push_history(&mut self, query: impl Into<String>) {
        let q = query.into();
        if q.trim().is_empty() {
            return;
        }
        if self.history.back().map(|s| s == &q).unwrap_or(false) {
            return;
        }
        self.history.push_back(q);
        while self.history.len() > COMMAND_PALETTE_HISTORY_CAP {
            self.history.pop_front();
        }
        self.history_cursor = None;
    }

    fn live(&self) -> bool {
        self.accepts_input && self.focused
    }

    fn bump_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }

    fn entries_collection<'a>(
        visible: &'a [CommandMatch<'_, Id>],
    ) -> Vec<CollectionItem<'a, usize>> {
        visible
            .iter()
            .enumerate()
            .map(|(i, e)| CollectionItem {
                id: i,
                enabled: e.enabled,
                label: &e.label,
                parent: None,
            })
            .collect()
    }

    /// Reconcile cursor after host rebuilds the visible projection.
    ///
    /// Call after filtering / async apply. `generation` must match
    /// [`Self::generation`] for async replies — otherwise the update is ignored
    /// (stale-result cancellation).
    pub fn apply_results(&mut self, generation: u64, visible: &[CommandMatch<'_, Id>]) -> bool {
        if generation != self.generation {
            return false;
        }
        self.applied_generation = generation;
        let entries = Self::entries_collection(visible);
        let _ = self.collection.reconcile(&entries);
        self.scroll = self.scroll.min(visible.len().saturating_sub(1));
        true
    }

    /// Convenience: filter locally and apply (sync path).
    pub fn refilter<'a>(&mut self, catalog: &'a [CommandEntry<Id>]) -> Vec<CommandMatch<'a, Id>> {
        let page = self.current_page().map(str::to_string);
        let visible = filter_command_entries(catalog, self.query_text(), page.as_deref());
        let generation = self.generation;
        let _ = self.apply_results(generation, &visible);
        visible
    }

    /// Open nested page.
    pub fn open_page(
        &mut self,
        page_id: impl Into<String>,
        title: impl Into<String>,
    ) -> CommandPaletteOutcome<Id> {
        let page_id = page_id.into();
        let title = title.into();
        self.page_stack.push((page_id.clone(), title));
        self.query = TextInputState::new("")
            .with_allow_empty(true)
            .with_editing();
        self.bump_generation();
        CommandPaletteOutcome::PageOpened { page_id }
    }

    /// Close one page (or ignored at root).
    pub fn close_page(&mut self) -> CommandPaletteOutcome<Id> {
        if self.page_stack.pop().is_some() {
            self.query = TextInputState::new("")
                .with_allow_empty(true)
                .with_editing();
            let _ = self.bump_generation();
            CommandPaletteOutcome::PageClosed
        } else {
            CommandPaletteOutcome::Ignored
        }
    }

    fn activate_at(
        &mut self,
        visible: &[CommandMatch<'_, Id>],
        idx: usize,
    ) -> CommandPaletteOutcome<Id> {
        let entry = match visible.get(idx) {
            Some(e) if e.enabled => e,
            _ => return CommandPaletteOutcome::Ignored,
        };
        if let Some(page) = &entry.opens_page {
            let title = entry.label.clone();
            return self.open_page(page.clone(), title);
        }
        if let Some(prompt) = &entry.argument_prompt {
            self.pending_id = Some(entry.id.clone());
            self.pending_command = entry.command.clone();
            self.argument = TextInputState::new("")
                .with_allow_empty(true)
                .with_editing();
            self.phase = CommandPalettePhase::Argument {
                entry_key: format!("{:?}", ()), // placeholder overwritten below
                prompt: prompt.clone(),
            };
            // Store prompt properly without Debug on Id
            self.phase = CommandPalettePhase::Argument {
                entry_key: entry.command.clone().unwrap_or_else(|| entry.label.clone()),
                prompt: prompt.clone(),
            };
            return CommandPaletteOutcome::NeedArguments {
                id: entry.id.clone(),
                prompt: prompt.clone(),
            };
        }
        let id = entry.id.clone();
        let command = entry.command.clone();
        let q = self.query_text().to_string();
        self.push_history(q);
        CommandPaletteOutcome::Activated {
            id,
            command,
            argument: None,
        }
    }

    fn submit_argument(&mut self) -> CommandPaletteOutcome<Id> {
        let id = match self.pending_id.take() {
            Some(id) => id,
            None => return CommandPaletteOutcome::Ignored,
        };
        let command = self.pending_command.take();
        let argument = self.argument.value().to_string();
        self.phase = CommandPalettePhase::Browse;
        self.argument = TextInputState::new("")
            .with_allow_empty(true)
            .with_editing();
        let q = self.query_text().to_string();
        self.push_history(q);
        CommandPaletteOutcome::Activated {
            id,
            command,
            argument: if argument.is_empty() {
                None
            } else {
                Some(argument)
            },
        }
    }

    /// Keyboard.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        visible: &[CommandMatch<'_, Id>],
    ) -> CommandPaletteOutcome<Id> {
        if !self.live() || key.is_release() {
            return CommandPaletteOutcome::Ignored;
        }
        let _ = self.apply_results(self.generation, visible);

        // Argument phase: edit draft or Esc back.
        if matches!(self.phase, CommandPalettePhase::Argument { .. }) {
            return self.handle_key_argument(key);
        }

        // Ctrl+P / Ctrl+N history when query empty (or always with Ctrl).
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('p' | 'P' | 'n' | 'N'))
        {
            return self.history_step(matches!(key.code, KeyCode::Char('p' | 'P')));
        }

        // Esc: clear query → close page → cancel.
        if key.code == KeyCode::Esc {
            if !self.query_text().is_empty() {
                self.query = TextInputState::new("")
                    .with_allow_empty(true)
                    .with_editing();
                let generation = self.bump_generation();
                return CommandPaletteOutcome::QueryChanged {
                    query: String::new(),
                    generation,
                };
            }
            if !self.page_stack.is_empty() {
                return self.close_page();
            }
            return CommandPaletteOutcome::Cancelled;
        }

        // Results navigation when list non-empty and not typing specials.
        if matches!(
            key.code,
            KeyCode::Down
                | KeyCode::Up
                | KeyCode::PageDown
                | KeyCode::PageUp
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Enter
        ) || (matches!(key.code, KeyCode::Char('j' | 'k' | 'J' | 'K'))
            && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            if let Some(intent) = default_palette_intent(key) {
                let out = self.handle_intent(intent, visible);
                if !matches!(out, CommandPaletteOutcome::Ignored) {
                    return out;
                }
            }
        }

        // Tab: next result.
        if key.code == KeyCode::Tab {
            return self.handle_intent(UiIntent::Move(NavigationMove::Next), visible);
        }
        if key.code == KeyCode::BackTab {
            return self.handle_intent(UiIntent::Move(NavigationMove::Previous), visible);
        }

        // Query editing via TextInput.
        match self.query.handle_key(key) {
            TextInputOutcome::Changed => {
                let generation = self.bump_generation();
                self.history_cursor = None;
                CommandPaletteOutcome::QueryChanged {
                    query: self.query_text().to_string(),
                    generation,
                }
            }
            TextInputOutcome::Submitted(_) => {
                if !visible.is_empty() {
                    return self.activate_at(visible, self.cursor_index());
                }
                CommandPaletteOutcome::Ignored
            }
            TextInputOutcome::Cancelled => {
                // TextInput may map Esc — already handled above.
                CommandPaletteOutcome::Ignored
            }
            TextInputOutcome::Ignored => {
                if let Some(intent) = default_palette_intent(key) {
                    self.handle_intent(intent, visible)
                } else {
                    CommandPaletteOutcome::Ignored
                }
            }
            _ => CommandPaletteOutcome::Ignored,
        }
    }

    fn handle_key_argument(&mut self, key: KeyEvent) -> CommandPaletteOutcome<Id> {
        if key.code == KeyCode::Esc {
            self.phase = CommandPalettePhase::Browse;
            self.pending_id = None;
            self.pending_command = None;
            self.argument = TextInputState::new("")
                .with_allow_empty(true)
                .with_editing();
            return CommandPaletteOutcome::ArgumentCancelled;
        }
        if key.code == KeyCode::Enter {
            return self.submit_argument();
        }
        match self.argument.handle_key(key) {
            TextInputOutcome::Changed => CommandPaletteOutcome::QueryChanged {
                query: self.argument_text().to_string(),
                generation: self.generation,
            },
            TextInputOutcome::Submitted(_) => self.submit_argument(),
            _ => CommandPaletteOutcome::Ignored,
        }
    }

    fn history_step(&mut self, older: bool) -> CommandPaletteOutcome<Id> {
        if self.history.is_empty() {
            return CommandPaletteOutcome::Ignored;
        }
        let len = self.history.len();
        let idx = match self.history_cursor {
            None if older => len - 1,
            None => return CommandPaletteOutcome::Ignored,
            Some(i) if older => i.saturating_sub(1),
            Some(i) => (i + 1).min(len - 1),
        };
        self.history_cursor = Some(idx);
        let q = self.history[idx].clone();
        self.query = TextInputState::new(&q)
            .with_allow_empty(true)
            .with_editing();
        self.bump_generation();
        CommandPaletteOutcome::HistoryApplied { query: q }
    }

    /// Intent routing (results list + activate).
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        visible: &[CommandMatch<'_, Id>],
    ) -> CommandPaletteOutcome<Id> {
        if !self.live() {
            return CommandPaletteOutcome::Ignored;
        }
        if matches!(self.phase, CommandPalettePhase::Argument { .. }) {
            return match intent {
                UiIntent::Cancel | UiIntent::Close => {
                    self.phase = CommandPalettePhase::Browse;
                    self.pending_id = None;
                    self.pending_command = None;
                    self.argument = TextInputState::new("")
                        .with_allow_empty(true)
                        .with_editing();
                    CommandPaletteOutcome::ArgumentCancelled
                }
                UiIntent::Activate | UiIntent::Submit => self.submit_argument(),
                _ => CommandPaletteOutcome::Ignored,
            };
        }
        let _ = self.apply_results(self.generation, visible);
        let entries = Self::entries_collection(visible);
        match intent {
            UiIntent::Move(
                NavigationMove::Next
                | NavigationMove::Previous
                | NavigationMove::First
                | NavigationMove::Last
                | NavigationMove::Up
                | NavigationMove::Down,
            )
            | UiIntent::Page(PageMove::Forward | PageMove::Backward) => {
                if visible.is_empty() {
                    return CommandPaletteOutcome::Ignored;
                }
                let out = self.collection.handle_intent(intent, &entries);
                if out.active_changed() {
                    // Keep cursor visible.
                    let cur = self.cursor_index();
                    let vis = usize::from(self.painted_rows.max(1));
                    if cur < self.scroll {
                        self.scroll = cur;
                    } else if cur >= self.scroll.saturating_add(vis) {
                        self.scroll = cur.saturating_sub(vis.saturating_sub(1));
                    }
                    CommandPaletteOutcome::CursorMoved
                } else {
                    CommandPaletteOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                if visible.is_empty() {
                    return CommandPaletteOutcome::Ignored;
                }
                self.activate_at(visible, self.cursor_index())
            }
            UiIntent::Cancel | UiIntent::Close => {
                if !self.query_text().is_empty() {
                    self.query = TextInputState::new("")
                        .with_allow_empty(true)
                        .with_editing();
                    let generation = self.bump_generation();
                    CommandPaletteOutcome::QueryChanged {
                        query: String::new(),
                        generation,
                    }
                } else if !self.page_stack.is_empty() {
                    self.close_page()
                } else {
                    CommandPaletteOutcome::Cancelled
                }
            }
            _ => CommandPaletteOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        visible: &[CommandMatch<'_, Id>],
    ) -> CommandPaletteOutcome<Id> {
        if !self.live() {
            return CommandPaletteOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                for (idx, rect) in &self.hits {
                    if rect_contains(*rect, event.position) {
                        self.collection.set_active(Some(*idx));
                        return self.activate_at(visible, *idx);
                    }
                }
                CommandPaletteOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), visible)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), visible)
            }
            MouseEventKind::Moved => {
                // Hover is stated every event, so leaving the list clears it.
                self.hovered = self
                    .hits
                    .iter()
                    .find(|(_, rect)| rect_contains(*rect, event.position))
                    .map(|(idx, _)| *idx);
                for (idx, rect) in &self.hits {
                    if rect_contains(*rect, event.position) {
                        if self.cursor_index() != *idx {
                            self.collection.set_active(Some(*idx));
                            return CommandPaletteOutcome::CursorMoved;
                        }
                        break;
                    }
                }
                CommandPaletteOutcome::Ignored
            }
            _ => CommandPaletteOutcome::Ignored,
        }
    }

    /// Update presentation from bounds (call on open/resize).
    pub fn sync_presentation_from_bounds(&mut self, bounds: Rect) -> CommandPaletteOutcome<Id> {
        if self.presentation_override.is_some() {
            return CommandPaletteOutcome::Ignored;
        }
        let next = command_palette_presentation_for_bounds(bounds);
        if next != self.presentation {
            self.presentation = next;
            CommandPaletteOutcome::PresentationChanged { presentation: next }
        } else {
            CommandPaletteOutcome::Ignored
        }
    }
}

fn rect_contains(rect: Rect, pos: Position) -> bool {
    pos.x >= rect.x
        && pos.y >= rect.y
        && pos.x < rect.x.saturating_add(rect.width)
        && pos.y < rect.y.saturating_sub(0).saturating_add(rect.height)
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Floating command palette chrome.
#[derive(Debug, Clone, Copy)]
pub struct CommandPalette<'a, Id> {
    title: &'a str,
    entries: &'a [CommandMatch<'a, Id>],
    system: &'a DesignSystem,
    focused: bool,
    colorless: bool,
    footer_hint: Option<&'a str>,
    empty_message: &'a str,
    no_result_message: &'a str,
    loading_message: &'a str,
    show_preview: bool,
}

/// Footer chords for the command palette, painted through [`HintBar`].
const COMMAND_PALETTE_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "↑↓",
        label: "move",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "run",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "C-p",
        label: "history",
        priority: 40,
        visible: true,
    },
    Hint {
        chord: "esc",
        label: "close",
        priority: 50,
        visible: true,
    },
];

impl<'a, Id> CommandPalette<'a, Id> {
    /// Title + visible (already filtered) entries + design system.
    #[must_use]
    pub const fn new(
        title: &'a str,
        entries: &'a [CommandMatch<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            title,
            entries,
            system,
            focused: true,
            colorless: false,
            footer_hint: None,
            empty_message: "Type to search commands",
            no_result_message: "No matching commands",
            loading_message: COMMAND_PALETTE_LOADING,
            show_preview: true,
        }
    }

    /// Surface focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    /// Reduced color.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Footer hint.
    #[must_use]
    pub const fn footer_hint(mut self, hint: Option<&'a str>) -> Self {
        self.footer_hint = hint;
        self
    }

    /// Empty catalog message (no entries at all).
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    /// Query with zero matches.
    #[must_use]
    pub const fn no_result_message(mut self, message: &'a str) -> Self {
        self.no_result_message = message;
        self
    }

    /// Loading message.
    #[must_use]
    pub const fn loading_message(mut self, message: &'a str) -> Self {
        self.loading_message = message;
        self
    }

    /// Show preview line for cursor entry.
    #[must_use]
    pub const fn show_preview(mut self, on: bool) -> Self {
        self.show_preview = on;
        self
    }

    /// Static handle_key (migration-friendly).
    pub fn handle_key(
        state: &mut CommandPaletteState<Id>,
        key: KeyEvent,
        entries: &[CommandMatch<'_, Id>],
    ) -> CommandPaletteOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        state.handle_key(key, entries)
    }

    /// Static handle_intent.
    pub fn handle_intent(
        state: &mut CommandPaletteState<Id>,
        intent: UiIntent,
        entries: &[CommandMatch<'_, Id>],
    ) -> CommandPaletteOutcome<Id>
    where
        Id: Clone + PartialEq,
    {
        state.handle_intent(intent, entries)
    }

    /// Query accessor.
    #[must_use]
    pub fn query(state: &CommandPaletteState<Id>) -> &TextInputState
    where
        Id: Clone + PartialEq,
    {
        state.query()
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut CommandPaletteState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.hits.clear();
        state.origin = (area.x, area.y);
        if area.is_empty() {
            return;
        }

        if state.presentation_override.is_none() {
            let _ = state.sync_presentation_from_bounds(area);
        }

        let surface = self.focused && state.accepts_input();
        let recipe = if surface {
            SurfaceRecipe::OverlayFocused
        } else {
            SurfaceRecipe::Overlay
        };

        let title = if state.current_page_title().is_some() {
            // "Commands › Page"
            // Panel title is &'a str — use base title only; page drawn in body.
            self.title
        } else {
            self.title
        };
        let _ = pt_title_marker(state, title);

        let colorless_system;
        let surface_system = if self.colorless {
            colorless_system = self
                .system
                .clone()
                .capability(crate::style::ColorCapability::Monochrome);
            &colorless_system
        } else {
            self.system
        };
        let inner = Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .padding(1, 0)
            .paint(area, buffer);
        if area.width > 4 {
            buffer.set_stringn(
                area.x.saturating_add(2),
                area.y,
                take_display_cols(self.title, usize::from(area.width.saturating_sub(4))).as_ref(),
                usize::from(area.width.saturating_sub(4)),
                self.system.style(Role::TextStrong),
            );
        }
        if inner.is_empty() {
            return;
        }

        let narrow = area.width < 28;
        let tiny = area.height < 6;
        let show_footer = !tiny && area.height >= 8 && !narrow;
        let show_preview = self.show_preview
            && !tiny
            && area.height >= 10
            && !narrow
            && !matches!(state.phase, CommandPalettePhase::Argument { .. });

        let mut body_h = inner.height;
        if show_footer {
            body_h = body_h.saturating_sub(1);
        }
        if show_preview {
            body_h = body_h.saturating_sub(1);
        }
        let body = Rect::new(inner.x, inner.y, inner.width, body_h);

        // Page breadcrumb line when nested.
        let mut content = body;
        if let Some(pt) = state.current_page_title() {
            if content.height > 0 {
                let crumb = { format!("← {pt}") };
                buffer.set_stringn(
                    content.x,
                    content.y,
                    take_display_cols(&crumb, usize::from(content.width)).as_ref(),
                    usize::from(content.width),
                    self.system.style(Role::TextMuted),
                );
                content.y = content.y.saturating_add(1);
                content.height = content.height.saturating_sub(1);
            }
        }

        // Query or argument field.
        if content.height > 0 {
            let field_area = Rect::new(content.x, content.y, content.width, 1);
            buffer.set_style(field_area, self.system.style(Role::Sunken));
            match &state.phase {
                CommandPalettePhase::Argument { prompt, .. } => {
                    let prefix = { format!("{prompt} › ") };
                    let pw = display_cols(&prefix) as u16;
                    buffer.set_stringn(
                        field_area.x,
                        field_area.y,
                        take_display_cols(&prefix, usize::from(field_area.width)).as_ref(),
                        usize::from(field_area.width),
                        self.system.style(Role::TextMuted),
                    );
                    let input_area = Rect::new(
                        field_area.x.saturating_add(pw.min(field_area.width)),
                        field_area.y,
                        field_area.width.saturating_sub(pw.min(field_area.width)),
                        1,
                    );
                    if input_area.width > 0 {
                        state.argument.set_focused(surface);
                        let _ = TextInput::new("", self.system)
                            .placeholder("argument")
                            .paint(input_area, buffer, &mut state.argument);
                    }
                }
                CommandPalettePhase::Browse => {
                    let ph = if narrow {
                        "Filter…"
                    } else {
                        "Type a command"
                    };
                    state.query.set_focused(surface);
                    let _ = TextInput::new("", self.system).placeholder(ph).paint(
                        field_area,
                        buffer,
                        &mut state.query,
                    );
                }
            }
            content.y = content.y.saturating_add(1);
            content.height = content.height.saturating_sub(1);
        }

        // Separator under query.
        if content.height > 0 && content.width > 0 {
            let line = { "─".repeat(usize::from(content.width)) };
            buffer.set_stringn(
                content.x,
                content.y,
                &line,
                usize::from(content.width),
                self.system.style(Role::Border),
            );
            content.y = content.y.saturating_add(1);
            content.height = content.height.saturating_sub(1);
        }

        // Results / empty / loading.
        let _ = state.apply_results(state.generation, self.entries);
        self.paint_results(content, buffer, state);

        // Preview line.
        if show_preview {
            let y = if show_footer {
                inner.bottom().saturating_sub(2)
            } else {
                inner.bottom().saturating_sub(1)
            };
            let preview = self
                .entries
                .get(state.cursor_index())
                .and_then(|e| e.preview.as_deref().or(e.disabled_reason.as_deref()))
                .unwrap_or("");
            if !preview.is_empty() {
                buffer.set_stringn(
                    inner.x,
                    y,
                    take_display_cols(preview, usize::from(inner.width)).as_ref(),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
        }

        // Footer.
        if show_footer {
            let footer = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
            if let Some(hint) = self.footer_hint {
                buffer.set_stringn(
                    footer.x,
                    footer.y,
                    take_display_cols(hint, usize::from(footer.width)).as_ref(),
                    usize::from(footer.width),
                    self.system.style(Role::TextMuted),
                );
            } else {
                ratatui_core::widgets::Widget::render(
                    &HintBar::new(COMMAND_PALETTE_HINTS, self.system),
                    footer,
                    buffer,
                );
            }
        }
    }

    fn paint_results(&self, area: Rect, buffer: &mut Buffer, state: &mut CommandPaletteState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            state.painted_rows = 0;
            return;
        }

        if state.loading && self.entries.is_empty() {
            let msg = if false && self.loading_message == COMMAND_PALETTE_LOADING {
                COMMAND_PALETTE_LOADING_ASCII
            } else {
                self.loading_message
            };
            let style = self.system.style(Role::TextMuted);
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(msg, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                style,
            );
            state.painted_rows = 1;
            return;
        }

        if self.entries.is_empty() {
            let (glyph, msg) = if state.query_text().is_empty() {
                ("∅", self.empty_message)
            } else {
                ("∅", self.no_result_message)
            };
            let line = format!("{glyph} {msg}");
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(&line, usize::from(area.width)).as_ref(),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            // Optional history hints when empty query.
            if state.query_text().is_empty() && !state.history.is_empty() && area.height > 2 {
                let mut y = area.y.saturating_add(2);
                buffer.set_stringn(
                    area.x,
                    area.y.saturating_add(1),
                    take_display_cols("Recent queries", usize::from(area.width)).as_ref(),
                    usize::from(area.width),
                    self.system.style(Role::TextMuted),
                );
                for h in state
                    .history
                    .iter()
                    .rev()
                    .take(usize::from(area.height.saturating_sub(2)))
                {
                    if y >= area.bottom() {
                        break;
                    }
                    buffer.set_stringn(
                        area.x,
                        y,
                        take_display_cols(h, usize::from(area.width)).as_ref(),
                        usize::from(area.width),
                        self.system.style(Role::Text),
                    );
                    y = y.saturating_add(1);
                }
            }
            state.painted_rows = 1;
            return;
        }

        let cursor = state.cursor_index();
        let surface = self.focused && state.accepts_input();
        let capacity = usize::from(area.height);
        if cursor < state.scroll {
            state.scroll = cursor;
        } else if cursor >= state.scroll.saturating_add(capacity) {
            state.scroll = cursor.saturating_sub(capacity.saturating_sub(1));
        }

        let mut y = area.y;
        let mut painted = 0u16;
        let mut last_group: Option<&str> = None;

        for (i, entry) in self.entries.iter().enumerate().skip(state.scroll) {
            if y >= area.bottom() {
                break;
            }
            // Group header.
            if let Some(g) = entry.group.as_deref() {
                if last_group != Some(g) {
                    last_group = Some(g);
                    let style = self.system.style(Role::TextMuted);
                    buffer.set_stringn(
                        area.x,
                        y,
                        take_display_cols(g, usize::from(area.width)).as_ref(),
                        usize::from(area.width),
                        style,
                    );
                    y = y.saturating_add(1);
                    painted = painted.saturating_add(1);
                    if y >= area.bottom() {
                        break;
                    }
                }
            }

            let active = i == cursor && surface;
            let row_rect = Rect::new(area.x, y, area.width, 1);
            state.hits.push((i, row_rect));
            let recipe = self.system.resolve_list_row(ListRowVisualState {
                selected: active,
                focused: active,
                hovered: state.hovered == Some(i),
                enabled: entry.enabled,
                loading: false,
                checked: false,
                ..ListRowVisualState::default()
            });
            if recipe.use_tint {
                buffer.set_style(row_rect, recipe.tint);
            }

            let chrome = super::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected: active,
                    focused: active,
                    hovered: state.hovered == Some(i),
                    enabled: entry.enabled,
                    loading: false,
                    checked: false,
                    ..ListRowVisualState::default()
                },
            );
            chrome.paint(buffer, row_rect);
            let mut x = area.x.saturating_add(3);

            // Leading badges: recent / contextual / disabled.
            let mut leading = String::new();
            if entry.recent {
                leading.push_str("↻ ");
            }
            if entry.contextual {
                leading.push_str("◎ ");
            }
            if !entry.enabled {
                leading.push_str("⊘ ");
            }
            if entry.opens_page.is_some() {
                leading.push_str("▸ ");
            }
            if !leading.is_empty() {
                let lw = display_cols(&leading) as u16;
                let style = if !entry.enabled {
                    self.system.style(Role::TextDisabled)
                } else if active {
                    self.system
                        .style(Role::TextStrong)
                        .patch(self.system.style(Role::SelectionTint))
                } else {
                    self.system.style(Role::TextMuted)
                };
                buffer.set_stringn(x, y, &leading, usize::from(lw), style);
                x = x.saturating_add(lw);
            }

            let remain = area.right().saturating_sub(x);
            // Shortcut reserved.
            let sc_w = entry
                .shortcut
                .as_ref()
                .map(|s| display_cols(s) as u16 + 1)
                .unwrap_or(0)
                .min(remain / 3);
            let label_w = remain.saturating_sub(sc_w);

            // Label with fuzzy highlights.
            if label_w > 0 {
                if let Some(ranges) = &entry.match_ranges {
                    let visual = HighlightVisual::Normal;
                    let _ = HighlightedText::new(&entry.label, ranges.as_slice(), self.system)
                        .visual(visual)
                        .truncate(MatchTruncate::End)
                        .paint(Rect::new(x, y, label_w, 1), buffer);
                } else {
                    let style = if self.colorless {
                        if !entry.enabled {
                            self.system.style(Role::TextMuted)
                        } else if active {
                            // Bold carries the cursor row without colour and
                            // without a reversal slab.
                            self.system
                                .style(Role::TextStrong)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            self.system.style(Role::Text)
                        }
                    } else if !entry.enabled {
                        self.system.style(Role::TextDisabled)
                    } else if active {
                        recipe.label
                    } else {
                        self.system.style(Role::Text)
                    };
                    buffer.set_stringn(
                        x,
                        y,
                        take_display_cols(&entry.label, usize::from(label_w)).as_ref(),
                        usize::from(label_w),
                        style,
                    );
                }
            }

            if sc_w > 0 {
                if let Some(sc) = &entry.shortcut {
                    let sx = area.right().saturating_sub(sc_w);
                    buffer.set_stringn(
                        sx,
                        y,
                        take_display_cols(sc, usize::from(sc_w)).as_ref(),
                        usize::from(sc_w),
                        self.system.style(Role::TextMuted),
                    );
                }
            }

            y = y.saturating_add(1);
            painted = painted.saturating_add(1);
        }
        state.painted_rows = painted;
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &CommandPaletteState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "command-palette q={:?} results={} loading={} page={:?} gen={}",
            state.query_text(),
            self.entries.len(),
            state.is_loading(),
            state.current_page(),
            state.generation()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Control)
                .label("command-palette")
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    selected: state.focused,
                    busy: state.loading,
                    expanded: true,
                    ..Default::default()
                }),
        );
    }
}

fn pt_title_marker<Id: Clone + PartialEq>(
    state: &CommandPaletteState<Id>,
    _title: &str,
) -> Option<()> {
    state.current_page_title().map(|_| ())
}

impl<Id: Clone + PartialEq> StatefulWidget for &CommandPalette<'_, Id> {
    type State = CommandPaletteState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for CommandPalette<'_, Id> {
    type State = CommandPaletteState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Sample catalog ──────────────────────────────────────────────────────────

/// Demo / story catalog.
#[must_use]
pub fn example_command_catalog() -> Vec<CommandEntry<&'static str>> {
    vec![
        CommandEntry::new("theme", "Toggle theme")
            .group("Appearance")
            .shortcut("C-t")
            .command_key("view.theme")
            .preview("Cycle theme / high-contrast")
            .keywords(["appearance", "color"]),
        CommandEntry::new("status", "Toggle status bar")
            .group("Appearance")
            .command_key("view.status")
            .recent(true),
        CommandEntry::new("palette-keys", "Keyboard shortcuts")
            .group("Help")
            .opens_page("keys")
            .preview("Browse keybindings"),
        CommandEntry::new("goto-line", "Go to line…")
            .group("Navigation")
            .argument_prompt("Line")
            .shortcut("C-g")
            .command_key("nav.goto")
            .contextual(true),
        CommandEntry::new("quit", "Quit")
            .group("App")
            .shortcut("C-q")
            .command_key("app.quit"),
        CommandEntry::new("disabled-demo", "Deploy (unavailable)")
            .group("App")
            .enabled(false)
            .disabled_reason("No deploy target configured"),
        // Nested page entries
        CommandEntry::new("key-save", "Save file")
            .page("keys")
            .group("File")
            .shortcut("C-s"),
        CommandEntry::new("key-find", "Find in file")
            .page("keys")
            .group("Edit")
            .shortcut("C-f"),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn catalog() -> Vec<CommandEntry<&'static str>> {
        example_command_catalog()
    }

    fn focused() -> CommandPaletteState<&'static str> {
        let mut s = CommandPaletteState::new(None);
        s.set_focused(true);
        s.set_accepts_input(true);
        s
    }

    #[test]
    fn fuzzy_subsequence() {
        let (score, ranges) = fuzzy_match_label("thm", "Toggle theme").unwrap();
        assert!(score < 100);
        assert!(!ranges.as_slice().is_empty());
        assert!(fuzzy_match_label("zzz", "Toggle theme").is_none());
    }

    #[test]
    fn keyword_match_does_not_highlight_primary_label() {
        let entries = [CommandEntry::new("alpha", "Alpha").keywords(["beta"])];

        let matches = filter_command_entries(&entries, "beta", None);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "alpha");
        assert_eq!(matches[0].match_ranges, None);
    }

    #[test]
    fn filter_groups_and_pages() {
        let cat = catalog();
        let root = filter_command_entries(&cat, "", None);
        assert!(root.iter().all(|e| e.page.is_none()));
        assert!(root.iter().any(|e| e.id == "theme"));
        assert!(!root.iter().any(|e| e.id == "key-save"));

        let keys = filter_command_entries(&cat, "", Some("keys"));
        assert!(keys.iter().all(|e| e.page.as_deref() == Some("keys")));
        assert!(keys.iter().any(|e| e.id == "key-save"));
    }

    #[test]
    fn query_changed_bumps_generation() {
        let mut s = focused();
        let cat = catalog();
        let vis = s.refilter(&cat);
        let g0 = s.generation();
        let out = s.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE), &vis);
        assert!(matches!(
            out,
            CommandPaletteOutcome::QueryChanged { generation, .. } if generation == g0 + 1
        ));
    }

    #[test]
    fn stale_results_rejected() {
        let mut s = focused();
        let cat = catalog();
        let vis = s.refilter(&cat);
        let stale_gen = s.generation();
        // bump
        let _ = s.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE), &vis);
        let filtered = filter_command_entries(&cat, "q", None);
        assert!(!s.apply_results(stale_gen, &filtered));
        assert!(s.apply_results(s.generation(), &filtered));
    }

    #[test]
    fn activate_command() {
        let mut s = focused();
        let cat = catalog();
        let vis = s.refilter(&cat);
        // cursor on first enabled
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            CommandPaletteOutcome::Activated { id: "status", .. }
                | CommandPaletteOutcome::Activated { id: "theme", .. }
                | CommandPaletteOutcome::Activated { .. }
        ));
    }

    #[test]
    fn nested_page_and_esc_layers() {
        let mut s = focused();
        let cat = catalog();
        let mut vis = s.refilter(&cat);
        // find palette-keys
        let idx = vis.iter().position(|e| e.id == "palette-keys").unwrap();
        s.collection.set_active(Some(idx));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            CommandPaletteOutcome::PageOpened { page_id } if page_id == "keys"
        ));
        vis = s.refilter(&cat);
        assert!(vis.iter().any(|e| e.id == "key-save"));
        // Esc with empty query closes page
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &vis),
            CommandPaletteOutcome::PageClosed
        ));
    }

    #[test]
    fn argument_phase() {
        let mut s = focused();
        let cat = catalog();
        let vis = s.refilter(&cat);
        let idx = vis.iter().position(|e| e.id == "goto-line").unwrap();
        s.collection.set_active(Some(idx));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            CommandPaletteOutcome::NeedArguments {
                id: "goto-line",
                ..
            }
        ));
        let _ = s.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE), &vis);
        let _ = s.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE), &vis);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            CommandPaletteOutcome::Activated {
                id: "goto-line",
                argument: Some(a),
                ..
            } if a == "42"
        ));
        let _ = vis;
    }

    #[test]
    fn esc_clears_query_then_cancels() {
        let mut s = focused();
        let cat = catalog();
        let vis = s.refilter(&cat);
        let _ = s.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), &vis);
        assert!(!s.query_text().is_empty());
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &[]),
            CommandPaletteOutcome::QueryChanged { query, .. } if query.is_empty()
        ));
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &[]),
            CommandPaletteOutcome::Cancelled
        ));
    }

    #[test]
    fn loading_and_empty_paint() {
        let system = DesignSystem::default();
        let mut s = focused();
        let _ = s.set_loading(true);
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        CommandPalette::new("Commands", &[], &system)
            .loading_message("Loading…")
            .paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Loading") || text.contains("loading") || text.contains("..."),
            "{text}"
        );

        s.set_loading(false);
        let mut buf2 = Buffer::empty(area);
        // no results with query
        s.query = TextInputState::new("zzz")
            .with_allow_empty(true)
            .with_editing();
        CommandPalette::new("Commands", &[], &system).paint(area, &mut buf2, &mut s);
        let t2: String = buf2
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            t2.contains("No matching") || t2.contains("∅") || t2.contains("No"),
            "{t2}"
        );
    }

    #[test]
    fn overlay_focus_restore_and_fullscreen() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let out = open_command_palette_overlay(
            &mut stack,
            bounds,
            CommandPaletteSize::default(),
            Some("editor"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::CommandPalette);
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("editor"),
                ..
            }
        ));

        let narrow = Rect::new(0, 0, 40, 12);
        let mut stack2 = OverlayStack::<()>::new();
        let _ =
            open_command_palette_overlay(&mut stack2, narrow, CommandPaletteSize::default(), None);
        assert!(stack2.top().unwrap().fullscreen_promoted);
        assert_eq!(stack2.top().unwrap().rect, narrow);
    }

    #[test]
    fn accepts_input_gate() {
        let mut s = focused();
        s.set_accepts_input(false);
        let cat = catalog();
        let vis = filter_command_entries(&cat, "", None);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            CommandPaletteOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_hit_activates_same_enabled_command_as_keyboard() {
        let run = CommandEntry::new("run", "Run");
        let visible = vec![CommandMatch::new(&run, 10, None)];
        let mut state = focused();
        state.hits = vec![(0, Rect::new(4, 3, 8, 1))];
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(4, 3),
                modifiers: KeyModifiers::NONE,
            },
            &visible,
        );
        assert!(matches!(
            out,
            CommandPaletteOutcome::Activated { id: "run", .. }
        ));

        let dis = CommandEntry::new("run", "Run").enabled(false);
        let disabled = vec![CommandMatch::new(&dis, 10, None)];
        assert_eq!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(4, 3),
                    modifiers: KeyModifiers::NONE,
                },
                &disabled,
            ),
            CommandPaletteOutcome::Ignored
        );
    }

    #[test]
    fn history_ctrl_p() {
        let mut s = focused();
        s.push_history("theme");
        s.push_history("quit");
        let out = s.handle_key(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            &[],
        );
        assert!(matches!(
            out,
            CommandPaletteOutcome::HistoryApplied { query } if query == "quit"
        ));
        assert_eq!(s.query_text(), "quit");
    }

    #[test]
    fn presentation_for_bounds() {
        assert_eq!(
            command_palette_presentation_for_bounds(Rect::new(0, 0, 40, 20)),
            CommandPalettePresentation::Fullscreen
        );
        assert_eq!(
            command_palette_presentation_for_bounds(Rect::new(0, 0, 80, 24)),
            CommandPalettePresentation::Centered
        );
    }

    #[test]
    fn paint_with_matches() {
        let system = DesignSystem::default();
        let mut s = focused();
        let cat = catalog();
        let vis = filter_command_entries(&cat, "thm", None);
        let _ = s.apply_results(s.generation(), &vis);
        let area = Rect::new(0, 0, 48, 14);
        let mut buf = Buffer::empty(area);
        CommandPalette::new("Commands", &vis, &system).paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Toggle") || text.contains("theme") || text.contains("Theme"),
            "{text}"
        );
        assert!(!s.hits.is_empty(), "result rows must paint hit geometry");
        let (idx, row) = s.hits[0];
        let _ = idx;
        let gutter = buf[(row.x, row.y)].symbol();
        assert_ne!(
            gutter, "›",
            "› is membership at col1, not a gutter replacement"
        );
        assert!(
            gutter == system.glyphs.selection_gutter(),
            "col0 is the focus bar or reserved slot, got {gutter:?}"
        );
    }

    #[test]
    fn fuzz_keys_no_panic() {
        let mut s = focused();
        let cat = catalog();
        let keys = [
            KeyCode::Char('a'),
            KeyCode::Char('t'),
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Backspace,
            KeyCode::Tab,
            KeyCode::Char('p'),
        ];
        let mut seed = 7u64;
        for _ in 0..300 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let mods = if k == KeyCode::Char('p') && seed % 3 == 0 {
                KeyModifiers::CONTROL
            } else {
                KeyModifiers::NONE
            };
            let vis = s.refilter(&cat);
            let _ = s.handle_key(KeyEvent::new(k, mods), &vis);
        }
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let s = focused();
        let mut scene = SemanticScene::<&str, ()>::default();
        CommandPalette::new("Commands", &[], &system).register_semantic(
            &mut scene,
            "cp",
            Rect::new(0, 0, 40, 12),
            &s,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("command-palette"))
        );
    }

    #[test]
    fn disabled_not_activated() {
        let mut s = focused();
        let cat = catalog();
        let vis = s.refilter(&cat);
        let idx = vis.iter().position(|e| e.id == "disabled-demo").unwrap();
        s.collection.set_active(Some(idx));
        // disabled rows skipped by collection usually — force and try
        assert!(matches!(
            s.activate_at(&vis, idx),
            CommandPaletteOutcome::Ignored
        ));
    }
}
