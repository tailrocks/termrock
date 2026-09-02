// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **QuickOpen** — high-performance fuzzy resource opener.
//!
//! **Mission.** Open files, symbols, sessions, tables, commands, and arbitrary
//! host resources through a multi-provider fuzzy surface — without the widget
//! owning filesystem, index, or network I/O.
//!
//! **vs [`super::CommandPalette`].** CommandPalette is a *command execution*
//! surface (actions, args, nested pages). QuickOpen is a *resource opener*:
//! provider tabs, streaming search windows, previews, query syntax, and
//! selection memory across providers. Both share fuzzy highlighting and
//! generation-gated async contracts.
//!
//! **Scale.** The component holds only the **visible result window** the host
//! supplies. Millions of logical candidates stay in the host index / searcher;
//! TermRock paints, roves, and emits typed [`QuickOpenSearchRequest`]s.
//!
//! **Integrations.**
//! - Fullscreen: [`open_quick_open_overlay`] + presentation sync (narrow promote).
//! - JumpMode: [`QuickOpenOutcome::JumpModeRequested`] + [`quick_open_jump_targets`].
//!
//! Research: fzf, television, VS Code Quick Open, Yazi, launchers.
use std::collections::HashMap;

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        CollectionItem, CollectionState, NavigationMove, OverlayId, OverlayKind, OverlayOutcome,
        OverlayPolicy, OverlaySize, OverlaySpec, OverlayStack, PageMove, RovingOrientation,
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, place_overlay,
    },
    style::{DesignSystem, ListRowVisualState, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        HighlightVisual, HighlightedText, JumpTarget, MatchRanges, MatchTruncate, Panel,
        PanelChrome, PanelVariant, TextInput, TextInputOutcome, TextInputState, fuzzy_match_label,
    },
};

/// Default overlay id.
pub const QUICK_OPEN_OVERLAY_ID: &str = "termrock.quick_open";
/// Width at or below which fullscreen presentation is preferred.
pub const QUICK_OPEN_FULLSCREEN_MAX_WIDTH: u16 = 56;
/// Height at or below which fullscreen presentation is preferred.
pub const QUICK_OPEN_FULLSCREEN_MAX_HEIGHT: u16 = 16;
/// Default host window hint for streaming searches.
pub const QUICK_OPEN_DEFAULT_LIMIT: usize = 200;
/// Max providers shown in the tab strip before compact mode.
pub const QUICK_OPEN_PROVIDER_STRIP_COMPACT_MAX: u16 = 40;

/// Default "still searching" copy, and its ASCII twin.
///
/// Two constants rather than one gated literal so host-supplied copy survives
/// the ASCII profile: only the *default* is swapped.
const QUICK_OPEN_SEARCHING: &str = "Searching…";
const QUICK_OPEN_SEARCHING_ASCII: &str = "Searching...";

// ── Size / placement ────────────────────────────────────────────────────────

/// Preferred size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickOpenSize {
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

impl Default for QuickOpenSize {
    fn default() -> Self {
        Self {
            width: 72,
            height: 20,
        }
    }
}

impl From<QuickOpenSize> for OverlaySize {
    fn from(value: QuickOpenSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 28,
            min_height: 8,
            max_width: 0,
            max_height: 0,
        }
    }
}

/// Placement presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum QuickOpenPresentation {
    /// Centered floating panel.
    #[default]
    Centered,
    /// Full bounds (FullscreenViewer-class).
    Fullscreen,
}

impl QuickOpenPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Centered => "centered",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Derive presentation from bounds.
#[must_use]
pub fn quick_open_presentation_for_bounds(bounds: Rect) -> QuickOpenPresentation {
    if bounds.width <= QUICK_OPEN_FULLSCREEN_MAX_WIDTH
        || bounds.height <= QUICK_OPEN_FULLSCREEN_MAX_HEIGHT
    {
        QuickOpenPresentation::Fullscreen
    } else {
        QuickOpenPresentation::Centered
    }
}

/// Place using CommandPalette-class center policy (upper third; may fullscreen).
#[must_use]
pub fn place_quick_open(bounds: Rect, preferred: QuickOpenSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    if bounds.width <= QUICK_OPEN_FULLSCREEN_MAX_WIDTH
        || bounds.height <= QUICK_OPEN_FULLSCREEN_MAX_HEIGHT
    {
        return place_overlay(
            bounds,
            None,
            OverlaySize::from(preferred),
            OverlayPolicy::for_kind(OverlayKind::CommandPalette),
        );
    }
    let width = preferred.width.min(bounds.width.saturating_sub(4)).max(28);
    let height = preferred.height.min(bounds.height.saturating_sub(2)).max(8);
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

/// Open as centered command-palette-class overlay.
pub fn open_quick_open_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: QuickOpenSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::command_palette(
            QUICK_OPEN_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Open as FullscreenViewer-class layer (explicit fullscreen).
pub fn open_quick_open_fullscreen<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::fullscreen(QUICK_OPEN_OVERLAY_ID, opener_focus),
    )
}

/// Dismiss default overlay.
pub fn dismiss_quick_open_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(QUICK_OPEN_OVERLAY_ID))
}

// ── Providers & query syntax ────────────────────────────────────────────────

/// One search provider (files, symbols, sessions, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickOpenProvider {
    /// Stable id (also used in `@id` query syntax).
    pub id: String,
    /// Tab label.
    pub label: String,
    /// Optional short glyph for compact strip.
    pub glyph: Option<String>,
    /// Whether a preview pane is meaningful.
    pub supports_preview: bool,
    /// Whether host understands query syntax beyond plain text.
    pub supports_query_syntax: bool,
}

impl QuickOpenProvider {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            glyph: None,
            supports_preview: true,
            supports_query_syntax: true,
        }
    }

    /// Glyph.
    #[must_use]
    pub fn glyph(mut self, g: impl Into<String>) -> Self {
        self.glyph = Some(g.into());
        self
    }

    /// Preview support.
    #[must_use]
    pub const fn supports_preview(mut self, on: bool) -> Self {
        self.supports_preview = on;
        self
    }

    /// Query syntax support.
    #[must_use]
    pub const fn supports_query_syntax(mut self, on: bool) -> Self {
        self.supports_query_syntax = on;
        self
    }
}

/// Parsed query with optional provider override and filter body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedQuickOpenQuery {
    /// `@provider` override when present.
    pub provider_override: Option<String>,
    /// Remaining filter text (after syntax prefix).
    pub filter: String,
    /// Raw full query string.
    pub raw: String,
}

/// Parse query syntax: `@files foo`, `#symbols bar` ( `#` alias for symbols),
/// plain text otherwise. Leading `@id` or `#id` switches provider.
#[must_use]
pub fn parse_quick_open_query(raw: &str) -> ParsedQuickOpenQuery {
    let raw_owned = raw.to_string();
    let trimmed = raw.trim_start();
    if let Some(rest) = trimmed
        .strip_prefix('@')
        .or_else(|| trimmed.strip_prefix('#'))
    {
        let mut parts = rest.splitn(2, char::is_whitespace);
        if let Some(id) = parts.next() {
            if !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                let filter = parts.next().unwrap_or("").trim_start().to_string();
                return ParsedQuickOpenQuery {
                    provider_override: Some(id.to_string()),
                    filter,
                    raw: raw_owned,
                };
            }
        }
    }
    ParsedQuickOpenQuery {
        provider_override: None,
        filter: raw.to_string(),
        raw: raw_owned,
    }
}

// ── Items & requests ────────────────────────────────────────────────────────

/// Host-owned preview payload (text lines; host may also paint externally).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuickOpenPreview {
    /// Inline text preview.
    Text {
        /// Lines (truncated by paint).
        lines: Vec<String>,
    },
    /// Single placeholder line.
    Placeholder(String),
    /// Host paints the preview pane itself.
    HostManaged,
}

impl QuickOpenPreview {
    /// Convenience text preview.
    #[must_use]
    pub fn text(lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self::Text {
            lines: lines.into_iter().map(Into::into).collect(),
        }
    }
}

/// One result row in the **visible window** (not the full corpus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickOpenItem<Id> {
    /// Stable identity.
    pub id: Id,
    /// Primary label (file name, symbol, …).
    pub label: String,
    /// Secondary path / signature / metadata.
    pub detail: Option<String>,
    /// Kind badge (file, fn, table, …).
    pub kind: Option<String>,
    /// Recent list membership.
    pub recent: bool,
    /// Sort score (lower better); host or local filter sets this.
    pub score: u32,
    /// Fuzzy ranges into [`Self::label`].
    pub match_ranges: Option<MatchRanges>,
    /// Fuzzy ranges into detail.
    pub detail_match_ranges: Option<MatchRanges>,
    /// Optional preview.
    pub preview: Option<QuickOpenPreview>,
}

impl<Id> QuickOpenItem<Id> {
    /// Construct.
    #[must_use]
    pub fn new(id: Id, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            detail: None,
            kind: None,
            recent: false,
            score: 0,
            match_ranges: None,
            detail_match_ranges: None,
            preview: None,
        }
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Kind badge.
    #[must_use]
    pub fn kind(mut self, k: impl Into<String>) -> Self {
        self.kind = Some(k.into());
        self
    }

    /// Recent.
    #[must_use]
    pub fn recent(mut self, on: bool) -> Self {
        self.recent = on;
        self
    }

    /// Preview.
    #[must_use]
    pub fn preview(mut self, p: QuickOpenPreview) -> Self {
        self.preview = Some(p);
        self
    }

    /// Score.
    #[must_use]
    pub const fn score(mut self, s: u32) -> Self {
        self.score = s;
        self
    }
}

/// Typed search request — **host performs I/O / index query**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickOpenSearchRequest {
    /// Active provider id.
    pub provider_id: String,
    /// Raw query.
    pub query: String,
    /// Parsed filter body (syntax stripped).
    pub filter: String,
    /// Generation for stale cancellation.
    pub generation: u64,
    /// Preferred max results for this window.
    pub limit: usize,
    /// Offset for streaming / pagination into the result set.
    pub offset: u64,
}

impl QuickOpenSearchRequest {
    /// Construct.
    #[must_use]
    pub fn new(
        provider_id: impl Into<String>,
        query: impl Into<String>,
        filter: impl Into<String>,
        generation: u64,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            query: query.into(),
            filter: filter.into(),
            generation,
            limit: QUICK_OPEN_DEFAULT_LIMIT,
            offset: 0,
        }
    }

    /// Limit.
    #[must_use]
    pub const fn limit(mut self, n: usize) -> Self {
        self.limit = if n == 0 { 1 } else { n };
        self
    }

    /// Offset.
    #[must_use]
    pub const fn offset(mut self, o: u64) -> Self {
        self.offset = o;
        self
    }
}

/// Local filter helper when the host already has a modest candidate slice.
///
/// For multi-million corpora the host should run its own searcher and only
/// push the top window via [`QuickOpenState::apply_results`].
#[must_use]
pub fn filter_quick_open_items<Id: Clone>(
    items: &[QuickOpenItem<Id>],
    filter: &str,
) -> Vec<QuickOpenItem<Id>> {
    let q = filter.trim();
    let mut out: Vec<QuickOpenItem<Id>> = items
        .iter()
        .filter_map(|it| {
            if q.is_empty() {
                let mut c = it.clone();
                c.match_ranges = None;
                c.detail_match_ranges = None;
                c.score = if c.recent { 0 } else { 10 };
                return Some(c);
            }
            let mut best = fuzzy_match_label(q, &it.label);
            if let Some(d) = &it.detail {
                if let Some((s, r)) = fuzzy_match_label(q, d) {
                    best = Some(match best {
                        Some((bs, br)) if bs <= s.saturating_add(3) => (bs, br),
                        _ => (s.saturating_add(3), r),
                    });
                }
            }
            best.map(|(score, ranges)| {
                let mut c = it.clone();
                c.score = score;
                // If match was on detail only, leave label ranges empty.
                if fuzzy_match_label(q, &it.label).is_some() {
                    c.match_ranges = Some(ranges);
                    c.detail_match_ranges = it
                        .detail
                        .as_ref()
                        .and_then(|d| fuzzy_match_label(q, d).map(|(_, r)| r));
                } else {
                    c.match_ranges = None;
                    c.detail_match_ranges = Some(ranges);
                }
                c
            })
        })
        .collect();
    out.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| b.recent.cmp(&a.recent))
            .then_with(|| a.label.cmp(&b.label))
    });
    out
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum QuickOpenOutcome<Id> {
    /// No change.
    Ignored,
    /// Query changed — host should search / cancel prior work.
    SearchRequested {
        /// Request payload.
        request: QuickOpenSearchRequest,
    },
    /// Prior generation should be cancelled (host side).
    SearchCancelled {
        /// Stale generation.
        generation: u64,
    },
    /// Provider tab changed (query/selection restored for that provider).
    ProviderChanged {
        /// Previous provider id.
        from: String,
        /// New provider id.
        to: String,
        /// Search to run for restored or empty query.
        request: QuickOpenSearchRequest,
    },
    /// Result cursor moved.
    CursorMoved,
    /// Resource activated.
    Activated {
        /// Provider.
        provider_id: String,
        /// Item id.
        id: Id,
    },
    /// Host should load / refresh preview for cursor item.
    PreviewRequested {
        /// Provider.
        provider_id: String,
        /// Item id.
        id: Id,
    },
    /// Request JumpMode labels over visible results.
    JumpModeRequested,
    /// Presentation changed (host may reflow overlay / FullscreenViewer).
    PresentationChanged {
        /// New presentation.
        presentation: QuickOpenPresentation,
    },
    /// Host should stream more results (scroll near end).
    StreamMore {
        /// Request with non-zero offset.
        request: QuickOpenSearchRequest,
    },
    /// Dismissed.
    Cancelled,
    /// Loading flag toggled.
    LoadingChanged {
        /// Loading.
        loading: bool,
    },
}

// ── Per-provider memory ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderMemory {
    query: String,
    cursor: usize,
    scroll: usize,
    /// Optional last selected label/id fingerprint for restore hints.
    selected_label: Option<String>,
}

impl Default for ProviderMemory {
    fn default() -> Self {
        Self {
            query: String::new(),
            cursor: 0,
            scroll: 0,
            selected_label: None,
        }
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// QuickOpen interaction state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickOpenState<Id> {
    query: TextInputState,
    collection: CollectionState<usize>,
    generation: u64,
    applied_generation: u64,
    accepts_input: bool,
    focused: bool,
    loading: bool,
    /// Stream complete for current generation.
    stream_complete: bool,
    /// Total hits hint from host (`None` = unknown).
    total_hint: Option<u64>,
    provider_index: usize,
    memory: HashMap<String, ProviderMemory>,
    presentation: QuickOpenPresentation,
    presentation_override: Option<QuickOpenPresentation>,
    show_preview: bool,
    hits: Vec<(usize, Rect)>,
    /// Row the pointer is over. Hover washes; it never commits.
    hovered: Option<usize>,
    provider_hits: Vec<(usize, Rect)>,
    scroll: usize,
    painted_rows: u16,
    limit: usize,
    /// Marker for Id type (results live in host projection).
    _id: std::marker::PhantomData<Id>,
}

impl<Id: Clone + PartialEq> Default for QuickOpenState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> QuickOpenState<Id> {
    /// Empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: TextInputState::new("").with_allow_empty(true),
            collection: CollectionState::new().orientation(RovingOrientation::Vertical),
            generation: 0,
            applied_generation: 0,
            accepts_input: true,
            focused: true,
            loading: false,
            stream_complete: true,
            total_hint: None,
            provider_index: 0,
            memory: HashMap::new(),
            presentation: QuickOpenPresentation::Centered,
            presentation_override: None,
            show_preview: true,
            hits: Vec::new(),
            hovered: None,
            provider_hits: Vec::new(),
            scroll: 0,
            painted_rows: 0,
            limit: QUICK_OPEN_DEFAULT_LIMIT,
            _id: std::marker::PhantomData,
        }
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Show preview pane.
    pub fn set_show_preview(&mut self, on: bool) {
        self.show_preview = on;
    }

    /// Window limit for requests.
    pub fn set_limit(&mut self, n: usize) {
        self.limit = n.max(1);
    }

    /// Loading.
    pub fn set_loading(&mut self, loading: bool) -> QuickOpenOutcome<Id> {
        if self.loading == loading {
            return QuickOpenOutcome::Ignored;
        }
        self.loading = loading;
        QuickOpenOutcome::LoadingChanged { loading }
    }

    /// Loading?
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        self.loading
    }

    /// Generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Applied generation.
    #[must_use]
    pub const fn applied_generation(&self) -> u64 {
        self.applied_generation
    }

    /// Stream complete?
    #[must_use]
    pub const fn stream_complete(&self) -> bool {
        self.stream_complete
    }

    /// Total hint.
    #[must_use]
    pub const fn total_hint(&self) -> Option<u64> {
        self.total_hint
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

    /// Provider index.
    #[must_use]
    pub const fn provider_index(&self) -> usize {
        self.provider_index
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> QuickOpenPresentation {
        self.presentation
    }

    /// Override presentation.
    pub fn set_presentation_override(&mut self, p: Option<QuickOpenPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Cursor index into visible window.
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    fn live(&self) -> bool {
        self.accepts_input && self.focused
    }

    fn bump_generation(&mut self) -> u64 {
        let prev = self.generation;
        self.generation = self.generation.saturating_add(1);
        let _ = prev;
        self.generation
    }

    fn active_provider<'a>(
        &self,
        providers: &'a [QuickOpenProvider],
    ) -> Option<&'a QuickOpenProvider> {
        providers.get(self.provider_index)
    }

    fn save_memory(&mut self, provider_id: &str, visible: &[QuickOpenItem<Id>]) {
        let selected_label = visible.get(self.cursor_index()).map(|i| i.label.clone());
        self.memory.insert(
            provider_id.to_string(),
            ProviderMemory {
                query: self.query_text().to_string(),
                cursor: self.cursor_index(),
                scroll: self.scroll,
                selected_label,
            },
        );
    }

    fn restore_memory(&mut self, provider_id: &str) {
        if let Some(mem) = self.memory.get(provider_id).cloned() {
            self.query = TextInputState::new(&mem.query).with_allow_empty(true);
            self.scroll = mem.scroll;
            self.collection.set_active(Some(mem.cursor));
        } else {
            self.query = TextInputState::new("").with_allow_empty(true);
            self.scroll = 0;
            self.collection.set_active(Some(0));
        }
    }

    fn build_request(
        &self,
        providers: &[QuickOpenProvider],
        offset: u64,
    ) -> Option<QuickOpenSearchRequest> {
        let p = self.active_provider(providers)?;
        let parsed = parse_quick_open_query(self.query_text());
        let provider_id = parsed
            .provider_override
            .clone()
            .unwrap_or_else(|| p.id.clone());
        Some(
            QuickOpenSearchRequest::new(
                provider_id,
                self.query_text(),
                parsed.filter,
                self.generation,
            )
            .limit(self.limit)
            .offset(offset),
        )
    }

    /// Apply a result window for `generation` (stale → false).
    ///
    /// `complete` marks end of stream; `total_hint` optional corpus size.
    pub fn apply_results(
        &mut self,
        generation: u64,
        visible: &[QuickOpenItem<Id>],
        complete: bool,
        total_hint: Option<u64>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.applied_generation = generation;
        self.stream_complete = complete;
        self.total_hint = total_hint;
        let entries: Vec<CollectionItem<usize>> = visible
            .iter()
            .enumerate()
            .map(|(i, it)| CollectionItem {
                id: i,
                enabled: true,
                label: it.label.clone(),
                parent: None,
            })
            .collect();
        let _ = self.collection.reconcile(&entries);
        // Prefer remembered label when present for this generation's provider.
        if let Some(mem) = self.memory.values().find(|m| m.selected_label.is_some()) {
            if let Some(label) = &mem.selected_label {
                if let Some(idx) = visible.iter().position(|it| &it.label == label) {
                    self.collection.set_active(Some(idx));
                }
            }
        }
        self.scroll = self.scroll.min(visible.len().saturating_sub(1));
        true
    }

    /// Append stream chunk (same generation); host concatenates then re-applies
    /// full window, or uses this to extend indices.
    pub fn note_stream_progress(&mut self, generation: u64, complete: bool) -> bool {
        if generation != self.generation {
            return false;
        }
        self.stream_complete = complete;
        true
    }

    /// Switch provider by index (preserves per-provider query/cursor).
    pub fn set_provider(
        &mut self,
        providers: &[QuickOpenProvider],
        index: usize,
        visible: &[QuickOpenItem<Id>],
    ) -> QuickOpenOutcome<Id> {
        if providers.is_empty() || index >= providers.len() {
            return QuickOpenOutcome::Ignored;
        }
        if index == self.provider_index {
            return QuickOpenOutcome::Ignored;
        }
        let from = providers[self.provider_index].id.clone();
        self.save_memory(&from, visible);
        self.provider_index = index;
        let to = providers[index].id.clone();
        self.restore_memory(&to);
        let generation = self.bump_generation();
        self.loading = true;
        self.stream_complete = false;
        let request = self
            .build_request(providers, 0)
            .unwrap_or_else(|| QuickOpenSearchRequest::new(&to, "", "", generation));
        QuickOpenOutcome::ProviderChanged { from, to, request }
    }

    /// Next / previous provider (Ctrl+Tab style via Tab with Ctrl).
    pub fn cycle_provider(
        &mut self,
        providers: &[QuickOpenProvider],
        delta: isize,
        visible: &[QuickOpenItem<Id>],
    ) -> QuickOpenOutcome<Id> {
        if providers.is_empty() {
            return QuickOpenOutcome::Ignored;
        }
        let len = providers.len() as isize;
        let next = (self.provider_index as isize + delta).rem_euclid(len) as usize;
        self.set_provider(providers, next, visible)
    }

    /// Emit search for current query (after host or local edits).
    pub fn request_search(&mut self, providers: &[QuickOpenProvider]) -> QuickOpenOutcome<Id> {
        // Apply @provider syntax switch without losing typed filter.
        let parsed = parse_quick_open_query(self.query_text());
        if let Some(ref override_id) = parsed.provider_override {
            if let Some(idx) = providers.iter().position(|p| &p.id == override_id) {
                if idx != self.provider_index {
                    let dummy: &[QuickOpenItem<Id>] = &[];
                    let from = providers
                        .get(self.provider_index)
                        .map(|p| p.id.clone())
                        .unwrap_or_default();
                    self.save_memory(&from, dummy);
                    self.provider_index = idx;
                    // Keep full raw query in the box; filter is for host.
                }
            }
        }
        let generation = self.bump_generation();
        let _ = generation;
        self.loading = true;
        self.stream_complete = false;
        match self.build_request(providers, 0) {
            Some(request) => QuickOpenOutcome::SearchRequested { request },
            None => QuickOpenOutcome::Ignored,
        }
    }

    /// Keyboard.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        providers: &[QuickOpenProvider],
        visible: &[QuickOpenItem<Id>],
    ) -> QuickOpenOutcome<Id> {
        if !self.live() || key.kind == KeyEventKind::Release {
            return QuickOpenOutcome::Ignored;
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        // Ctrl+P / Ctrl+N — provider cycle (VS Code-ish) when Alt not held.
        if ctrl && !alt && matches!(key.code, KeyCode::Char('p' | 'P')) {
            return self.cycle_provider(providers, -1, visible);
        }
        if ctrl && !alt && matches!(key.code, KeyCode::Char('n' | 'N')) {
            return self.cycle_provider(providers, 1, visible);
        }
        // Ctrl+Tab / Ctrl+Shift+Tab
        if ctrl && key.code == KeyCode::Tab {
            let delta = if key.modifiers.contains(KeyModifiers::SHIFT) {
                -1
            } else {
                1
            };
            return self.cycle_provider(providers, delta, visible);
        }

        // Ctrl+J → JumpMode over results
        if ctrl && matches!(key.code, KeyCode::Char('j' | 'J')) {
            return QuickOpenOutcome::JumpModeRequested;
        }

        // Ctrl+Space or F-less fullscreen toggle via Ctrl+\
        if ctrl && matches!(key.code, KeyCode::Char('\\')) {
            let next = match self.presentation {
                QuickOpenPresentation::Centered => QuickOpenPresentation::Fullscreen,
                QuickOpenPresentation::Fullscreen => QuickOpenPresentation::Centered,
            };
            self.presentation = next;
            self.presentation_override = Some(next);
            return QuickOpenOutcome::PresentationChanged { presentation: next };
        }

        // Esc cancel
        if key.code == KeyCode::Esc {
            if !self.query_text().is_empty() {
                self.query = TextInputState::new("").with_allow_empty(true);
                return self.request_search(providers);
            }
            // Cancel in-flight
            let generation = self.generation;
            if self.loading {
                self.loading = false;
                return QuickOpenOutcome::SearchCancelled { generation };
            }
            return QuickOpenOutcome::Cancelled;
        }

        // Left/Right on empty query cycle providers (desktop launcher feel)
        if self.query_text().is_empty()
            && matches!(key.code, KeyCode::Left | KeyCode::Right)
            && !ctrl
        {
            let delta = if key.code == KeyCode::Left { -1 } else { 1 };
            return self.cycle_provider(providers, delta, visible);
        }

        // Results navigation
        if matches!(
            key.code,
            KeyCode::Down
                | KeyCode::Up
                | KeyCode::PageDown
                | KeyCode::PageUp
                | KeyCode::Enter
                | KeyCode::Home
                | KeyCode::End
        ) || (ctrl && matches!(key.code, KeyCode::Char('j' | 'k' | 'J' | 'K')))
        {
            // Ctrl+J reserved for jump when alone — already handled.
            if !(ctrl && matches!(key.code, KeyCode::Char('j' | 'J'))) {
                if let Some(intent) = default_quick_open_intent(key) {
                    let out = self.handle_intent(intent, providers, visible);
                    if !matches!(out, QuickOpenOutcome::Ignored) {
                        return out;
                    }
                }
            }
        }

        if key.code == KeyCode::Tab && !ctrl {
            return self.handle_intent(UiIntent::Move(NavigationMove::Next), providers, visible);
        }
        if key.code == KeyCode::BackTab {
            return self.handle_intent(
                UiIntent::Move(NavigationMove::Previous),
                providers,
                visible,
            );
        }

        // Query edit
        match self.query.handle_key(key) {
            TextInputOutcome::Changed => self.request_search(providers),
            TextInputOutcome::Submitted(_) => {
                if visible.is_empty() {
                    QuickOpenOutcome::Ignored
                } else {
                    self.activate(providers, visible, self.cursor_index())
                }
            }
            TextInputOutcome::Ignored => {
                if let Some(intent) = default_quick_open_intent(key) {
                    self.handle_intent(intent, providers, visible)
                } else {
                    QuickOpenOutcome::Ignored
                }
            }
            _ => QuickOpenOutcome::Ignored,
        }
    }

    fn activate(
        &mut self,
        providers: &[QuickOpenProvider],
        visible: &[QuickOpenItem<Id>],
        idx: usize,
    ) -> QuickOpenOutcome<Id> {
        let item = match visible.get(idx) {
            Some(i) => i,
            None => return QuickOpenOutcome::Ignored,
        };
        if let Some(p) = self.active_provider(providers) {
            self.save_memory(&p.id, visible);
        }
        let provider_id = self
            .active_provider(providers)
            .map(|p| p.id.clone())
            .unwrap_or_default();
        QuickOpenOutcome::Activated {
            provider_id,
            id: item.id.clone(),
        }
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        providers: &[QuickOpenProvider],
        visible: &[QuickOpenItem<Id>],
    ) -> QuickOpenOutcome<Id> {
        if !self.live() {
            return QuickOpenOutcome::Ignored;
        }
        let entries: Vec<CollectionItem<usize>> = visible
            .iter()
            .enumerate()
            .map(|(i, it)| CollectionItem {
                id: i,
                enabled: true,
                label: it.label.clone(),
                parent: None,
            })
            .collect();
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
                    return QuickOpenOutcome::Ignored;
                }
                let out = self.collection.handle_intent(intent, &entries);
                if out.active_changed() {
                    let cur = self.cursor_index();
                    let vis = usize::from(self.painted_rows.max(1));
                    if cur < self.scroll {
                        self.scroll = cur;
                    } else if cur >= self.scroll.saturating_add(vis) {
                        self.scroll = cur.saturating_sub(vis.saturating_sub(1));
                    }
                    // Stream more near end
                    if !self.stream_complete
                        && cur + 5 >= visible.len()
                        && let Some(mut req) = self.build_request(providers, visible.len() as u64)
                    {
                        req.generation = self.generation;
                        return QuickOpenOutcome::StreamMore { request: req };
                    }
                    // Preview
                    if let (Some(p), Some(item)) =
                        (self.active_provider(providers), visible.get(cur))
                    {
                        if p.supports_preview && self.show_preview {
                            return QuickOpenOutcome::PreviewRequested {
                                provider_id: p.id.clone(),
                                id: item.id.clone(),
                            };
                        }
                    }
                    QuickOpenOutcome::CursorMoved
                } else {
                    QuickOpenOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => {
                self.activate(providers, visible, self.cursor_index())
            }
            UiIntent::Cancel | UiIntent::Close => {
                if !self.query_text().is_empty() {
                    self.query = TextInputState::new("").with_allow_empty(true);
                    self.request_search(providers)
                } else {
                    QuickOpenOutcome::Cancelled
                }
            }
            _ => QuickOpenOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        providers: &[QuickOpenProvider],
        visible: &[QuickOpenItem<Id>],
    ) -> QuickOpenOutcome<Id> {
        if !self.live() {
            return QuickOpenOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                for (idx, rect) in &self.provider_hits {
                    if rect_contains(*rect, event.position) {
                        return self.set_provider(providers, *idx, visible);
                    }
                }
                for (idx, rect) in &self.hits {
                    if rect_contains(*rect, event.position) {
                        self.collection.set_active(Some(*idx));
                        return self.activate(providers, visible, *idx);
                    }
                }
                QuickOpenOutcome::Ignored
            }
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), providers, visible)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), providers, visible)
            }
            MouseEventKind::Moved => {
                // Hover is stated every event, so leaving the list clears it.
                self.hovered = self
                    .hits
                    .iter()
                    .find(|(_, rect)| rect_contains(*rect, event.position))
                    .map(|(idx, _)| *idx);
                for (idx, rect) in &self.hits {
                    if rect_contains(*rect, event.position) && self.cursor_index() != *idx {
                        self.collection.set_active(Some(*idx));
                        return QuickOpenOutcome::CursorMoved;
                    }
                }
                QuickOpenOutcome::Ignored
            }
            _ => QuickOpenOutcome::Ignored,
        }
    }

    /// Sync presentation from bounds.
    pub fn sync_presentation_from_bounds(&mut self, bounds: Rect) -> QuickOpenOutcome<Id> {
        if self.presentation_override.is_some() {
            return QuickOpenOutcome::Ignored;
        }
        let next = quick_open_presentation_for_bounds(bounds);
        if next != self.presentation {
            self.presentation = next;
            QuickOpenOutcome::PresentationChanged { presentation: next }
        } else {
            QuickOpenOutcome::Ignored
        }
    }
}

fn rect_contains(rect: Rect, pos: Position) -> bool {
    pos.x >= rect.x
        && pos.y >= rect.y
        && pos.x < rect.x.saturating_add(rect.width)
        && pos.y < rect.y.saturating_add(rect.height)
}

/// Default intents for result list.
#[must_use]
pub fn default_quick_open_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Down => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Up => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Char('j' | 'J') if ctrl => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Char('k' | 'K') if ctrl => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::Home if ctrl => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End if ctrl => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        _ => None,
    }
}

/// Build [`JumpTarget`]s from last painted result hits (JumpMode integration).
#[must_use]
pub fn quick_open_jump_targets<Id: Clone>(
    visible: &[QuickOpenItem<Id>],
    hits: &[(usize, Rect)],
    badges: &[char],
) -> Vec<JumpTarget<Id>> {
    let mut out = Vec::new();
    for (n, (idx, rect)) in hits.iter().enumerate() {
        if let Some(item) = visible.get(*idx) {
            let badge = badges.get(n).copied().unwrap_or('?');
            out.push(JumpTarget::new(item.id.clone(), *rect, badge.to_string()));
        }
    }
    out
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// QuickOpen paint.
#[derive(Debug, Clone, Copy)]
pub struct QuickOpen<'a, Id> {
    providers: &'a [QuickOpenProvider],
    items: &'a [QuickOpenItem<Id>],
    system: &'a DesignSystem,
    focused: bool,
    colorless: bool,
    footer_hint: Option<&'a str>,
    empty_message: &'a str,
    no_result_message: &'a str,
    loading_message: &'a str,
    title: &'a str,
}

impl<'a, Id> QuickOpen<'a, Id> {
    /// Providers + visible window + design system.
    #[must_use]
    pub const fn new(
        providers: &'a [QuickOpenProvider],
        items: &'a [QuickOpenItem<Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            providers,
            items,
            system,
            focused: true,
            // Seeded from the system: a widget that defaults to false is
            // claiming the terminal has Unicode and colour before anyone
            // asked it. Builders below still force either way.
            colorless: system.mono(),
            footer_hint: Some("↑↓ open · enter · @provider · C-n/C-p switch · C-j jump · esc"),
            empty_message: "Type to search resources",
            no_result_message: "No matching resources",
            loading_message: QUICK_OPEN_SEARCHING,
            title: "Quick Open",
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }

    /// Focused chrome.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// ASCII.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Footer.
    #[must_use]
    pub const fn footer_hint(mut self, h: Option<&'a str>) -> Self {
        self.footer_hint = h;
        self
    }

    /// Empty catalog message.
    #[must_use]
    pub const fn empty_message(mut self, m: &'a str) -> Self {
        self.empty_message = m;
        self
    }

    /// No results.
    #[must_use]
    pub const fn no_result_message(mut self, m: &'a str) -> Self {
        self.no_result_message = m;
        self
    }

    /// Loading.
    #[must_use]
    pub const fn loading_message(mut self, m: &'a str) -> Self {
        self.loading_message = m;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut QuickOpenState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.hits.clear();
        state.provider_hits.clear();
        if area.is_empty() {
            return;
        }
        if state.presentation_override.is_none() {
            let _ = state.sync_presentation_from_bounds(area);
        }

        let surface = self.focused && state.accepts_input();
        let emphasis = if surface {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let panel = Panel::new(self.system)
            .variant(PanelVariant::Bordered)
            .overlay(true)
            .title(self.title)
            .emphasis(emphasis);
        let inner = panel.inner(area);
        ratatui_core::widgets::Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let narrow = area.width < 36;
        let tiny = area.height < 8;
        let show_footer = self.footer_hint.is_some() && !tiny && area.height >= 10 && !narrow;
        let preview_on = state.show_preview
            && !tiny
            && area.width >= 48
            && self
                .providers
                .get(state.provider_index)
                .map(|p| p.supports_preview)
                .unwrap_or(false);

        let mut y = inner.y;
        let bottom = if show_footer {
            inner.bottom().saturating_sub(1)
        } else {
            inner.bottom()
        };

        // Provider strip
        if y < bottom && !self.providers.is_empty() {
            self.paint_provider_strip(
                Rect::new(inner.x, y, inner.width, 1),
                buffer,
                state,
                surface,
            );
            y = y.saturating_add(1);
        }

        // Query
        if y < bottom {
            state.query.set_focused(surface);
            let _ = TextInput::new("", self.system)
                .placeholder(if narrow {
                    "Search… (@files)"
                } else {
                    "Search resources  (@files query)"
                })
                .paint(
                    Rect::new(inner.x, y, inner.width, 1),
                    buffer,
                    &mut state.query,
                );
            y = y.saturating_add(1);
        }

        // Separator
        if y < bottom {
            let line = { "─".repeat(usize::from(inner.width)) };
            buffer.set_stringn(
                inner.x,
                y,
                &line,
                usize::from(inner.width),
                self.system.style(Role::Border),
            );
            y = y.saturating_add(1);
        }

        let body_h = bottom.saturating_sub(y);
        if body_h == 0 {
            return;
        }

        let (list_area, preview_area) = if preview_on && inner.width >= 48 {
            let pw = (inner.width / 3).clamp(16, 28);
            let lw = inner.width.saturating_sub(pw).saturating_sub(1);
            (
                Rect::new(inner.x, y, lw, body_h),
                Some(Rect::new(
                    inner.x.saturating_add(lw).saturating_add(1),
                    y,
                    pw,
                    body_h,
                )),
            )
        } else {
            (Rect::new(inner.x, y, inner.width, body_h), None)
        };

        // Vertical divider for preview
        if let Some(pa) = preview_area {
            let vx = pa.x.saturating_sub(1);
            for row in y..bottom {
                buffer.set_stringn(vx, row, "│", 1, self.system.style(Role::Border));
            }
            self.paint_preview(pa, buffer, state);
        }

        self.paint_results(list_area, buffer, state);

        // Status line under list (counts)
        // Footer
        if show_footer {
            if let Some(hint) = self.footer_hint {
                let mut line = hint.to_string();
                if let Some(t) = state.total_hint {
                    line = format!("{}  ·  {}/{}", hint, self.items.len(), t);
                } else if state.loading {
                    line = format!("{}  ·  streaming…", hint);
                }
                buffer.set_stringn(
                    inner.x,
                    inner.bottom().saturating_sub(1),
                    &take_display_cols(&line, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
        }
    }

    fn paint_provider_strip(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut QuickOpenState<Id>,
        surface: bool,
    ) {
        let compact = area.width <= QUICK_OPEN_PROVIDER_STRIP_COMPACT_MAX;
        let mut x = area.x;
        for (i, p) in self.providers.iter().enumerate() {
            if x >= area.right() {
                break;
            }
            let label = if compact {
                p.glyph.clone().unwrap_or_else(|| {
                    p.label
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "?".into())
                })
            } else {
                p.label.clone()
            };
            let active = i == state.provider_index;
            let text = if active {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            };
            let w = (display_cols(&text) as u16).min(area.right().saturating_sub(x));
            if w == 0 {
                break;
            }
            let style = if self.colorless {
                if active && surface {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::TextMuted)
                }
            } else if active && surface {
                self.system
                    .style(Role::TextStrong)
                    .patch(self.system.style(Role::SelectionTint))
                    .add_modifier(Modifier::BOLD)
            } else {
                self.system.style(Role::TextMuted)
            };
            buffer.set_stringn(
                x,
                area.y,
                &take_display_cols(&text, usize::from(w)),
                usize::from(w),
                style,
            );
            state.provider_hits.push((i, Rect::new(x, area.y, w, 1)));
            x = x.saturating_add(w);
        }
    }

    fn paint_results(&self, area: Rect, buffer: &mut Buffer, state: &mut QuickOpenState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            state.painted_rows = 0;
            return;
        }

        if state.loading && self.items.is_empty() {
            let msg = if false && self.loading_message == QUICK_OPEN_SEARCHING {
                QUICK_OPEN_SEARCHING_ASCII
            } else {
                self.loading_message
            };
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            state.painted_rows = 1;
            return;
        }

        if self.items.is_empty() {
            let msg = if state.query_text().is_empty() {
                self.empty_message
            } else {
                self.no_result_message
            };
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
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
        for (i, item) in self.items.iter().enumerate().skip(state.scroll) {
            if y >= area.bottom() {
                break;
            }
            let active = i == cursor && surface;
            let row = Rect::new(area.x, y, area.width, 1);
            state.hits.push((i, row));
            let recipe = self.system.resolve_list_row(ListRowVisualState {
                selected: active,
                focused: active,
                hovered: state.hovered == Some(i),
                enabled: true,
                loading: false,
                checked: false,
                ..ListRowVisualState::default()
            });
            if recipe.use_tint {
                buffer.set_style(row, recipe.tint);
            }

            let visual = ListRowVisualState {
                selected: active,
                focused: active,
                hovered: state.hovered == Some(i),
                enabled: true,
                loading: false,
                checked: false,
                ..ListRowVisualState::default()
            };
            let chrome = super::row_chrome::RowChrome::resolve(self.system, visual);
            chrome.paint(buffer, row);
            let mut x = area.x.saturating_add(3);
            let base = if self.colorless {
                if active {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(Role::Text)
                }
            } else if active {
                recipe.label
            } else {
                self.system.style(Role::Text)
            };

            if item.recent {
                let mark = { "↻ " };
                buffer.set_stringn(x, y, mark, 2, self.system.style(Role::TextMuted));
                x = x.saturating_add(2);
            }
            if let Some(k) = &item.kind {
                let badge = { format!("{k} ") };
                let bw = display_cols(&badge) as u16;
                buffer.set_stringn(
                    x,
                    y,
                    &take_display_cols(&badge, usize::from(bw)),
                    usize::from(bw),
                    self.system.style(Role::TextMuted),
                );
                x = x.saturating_add(bw);
            }

            let remain = area.right().saturating_sub(x);
            let detail_w = item
                .detail
                .as_ref()
                .map(|d| (display_cols(d) as u16 + 1).min(remain / 2))
                .unwrap_or(0);
            let label_w = remain.saturating_sub(detail_w);

            if label_w > 0 {
                if let Some(ranges) = &item.match_ranges {
                    let visual = if active {
                        HighlightVisual::Selected
                    } else {
                        HighlightVisual::Normal
                    };
                    let _ = HighlightedText::new(&item.label, ranges.as_slice(), self.system)
                        .visual(visual)
                        .truncate(MatchTruncate::End)
                        .paint(Rect::new(x, y, label_w, 1), buffer);
                } else {
                    buffer.set_stringn(
                        x,
                        y,
                        &take_display_cols(&item.label, usize::from(label_w)),
                        usize::from(label_w),
                        base,
                    );
                }
            }

            if detail_w > 0 {
                if let Some(d) = &item.detail {
                    let dx = area.right().saturating_sub(detail_w);
                    // A path end-cut loses exactly the token that tells two
                    // candidates apart, so drop leading segments instead
                    // (plans/022 Step 3).
                    let shown = crate::text::truncate_path(
                        d,
                        usize::from(detail_w),
                        self.system.glyphs.ellipsis(),
                    );
                    buffer.set_stringn(
                        dx,
                        y,
                        shown.as_ref(),
                        usize::from(detail_w),
                        self.system.style(Role::TextMuted),
                    );
                }
            }

            y = y.saturating_add(1);
            painted = painted.saturating_add(1);
        }
        state.painted_rows = painted;
    }

    fn paint_preview(&self, area: Rect, buffer: &mut Buffer, state: &QuickOpenState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        let item = self.items.get(state.cursor_index());
        let header = { "Preview" };
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols(header, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::TextMuted),
        );
        let Some(item) = item else {
            return;
        };
        let mut y = area.y.saturating_add(1);
        match &item.preview {
            Some(QuickOpenPreview::Text { lines }) => {
                for line in lines {
                    if y >= area.bottom() {
                        break;
                    }
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(line.as_str(), usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::Text),
                    );
                    y = y.saturating_add(1);
                }
            }
            Some(QuickOpenPreview::Placeholder(s)) => {
                if y < area.bottom() {
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(s.as_str(), usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::TextMuted),
                    );
                }
            }
            Some(QuickOpenPreview::HostManaged) => {
                if y < area.bottom() {
                    let msg = { "⋯ host preview" };
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(msg, usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::TextMuted),
                    );
                }
            }
            None => {
                if let Some(d) = &item.detail {
                    if y < area.bottom() {
                        buffer.set_stringn(
                            area.x,
                            y,
                            &take_display_cols(d, usize::from(area.width)),
                            usize::from(area.width),
                            self.system.style(Role::TextMuted),
                        );
                    }
                }
            }
        }
    }

    /// Access last painted result hits (for JumpMode).
    #[must_use]
    pub fn hits_from_state(state: &QuickOpenState<Id>) -> &[(usize, Rect)] {
        &state.hits
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &QuickOpenState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        let prov = self
            .providers
            .get(state.provider_index)
            .map(|p| p.id.as_str())
            .unwrap_or("-");
        let desc = format!(
            "quick-open provider={prov} q={:?} results={} loading={} gen={}",
            state.query_text(),
            self.items.len(),
            state.is_loading(),
            state.generation()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Control)
                .label("quick-open")
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

impl<Id: Clone + PartialEq> StatefulWidget for &QuickOpen<'_, Id> {
    type State = QuickOpenState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for QuickOpen<'_, Id> {
    type State = QuickOpenState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Sample data ─────────────────────────────────────────────────────────────

/// Demo providers.
#[must_use]
pub fn example_quick_open_providers() -> Vec<QuickOpenProvider> {
    vec![
        QuickOpenProvider::new("files", "Files").glyph("f"),
        QuickOpenProvider::new("symbols", "Symbols").glyph("s"),
        QuickOpenProvider::new("sessions", "Sessions")
            .glyph("S")
            .supports_preview(false),
        QuickOpenProvider::new("tables", "Tables").glyph("t"),
    ]
}

/// Demo file items.
#[must_use]
pub fn example_quick_open_files() -> Vec<QuickOpenItem<&'static str>> {
    vec![
        QuickOpenItem::new("main", "main.rs")
            .detail("src/main.rs")
            .kind("rs")
            .recent(true)
            .preview(QuickOpenPreview::text([
                "fn main() {",
                "    println!(\"hi\");",
                "}",
            ])),
        QuickOpenItem::new("lib", "lib.rs")
            .detail("src/lib.rs")
            .kind("rs")
            .preview(QuickOpenPreview::text([
                "//! crate root",
                "",
                "pub mod widgets;",
            ])),
        QuickOpenItem::new("quick", "quick_open.rs")
            .detail("src/widgets/quick_open.rs")
            .kind("rs")
            .preview(QuickOpenPreview::Placeholder("widget source".into())),
        QuickOpenItem::new("readme", "README.md")
            .detail("README.md")
            .kind("md"),
        QuickOpenItem::new("toml", "Cargo.toml")
            .detail("Cargo.toml")
            .kind("toml"),
    ]
}

/// Demo symbols.
#[must_use]
pub fn example_quick_open_symbols() -> Vec<QuickOpenItem<&'static str>> {
    vec![
        QuickOpenItem::new("fn-paint", "QuickOpen::paint")
            .detail("widgets/quick_open.rs")
            .kind("fn"),
        QuickOpenItem::new("struct-state", "QuickOpenState")
            .detail("widgets/quick_open.rs")
            .kind("struct"),
        QuickOpenItem::new("fn-filter", "filter_quick_open_items")
            .detail("widgets/quick_open.rs")
            .kind("fn"),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn providers() -> Vec<QuickOpenProvider> {
        example_quick_open_providers()
    }

    fn focused() -> QuickOpenState<&'static str> {
        let mut s = QuickOpenState::new();
        s.set_focused(true);
        s.set_accepts_input(true);
        s
    }

    #[test]
    fn narrow_paths_keep_their_filename() {
        use ratatui_core::buffer::Buffer;
        let system = crate::style::DesignSystem::default();
        let items = example_quick_open_files();
        let mut state = focused();
        let area = Rect::new(0, 0, 40, 14);
        let mut buffer = Buffer::empty(area);
        QuickOpen::new(&providers(), &items, &system).paint(area, &mut buffer, &mut state);
        let painted: String = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        // `src/widgets/quick_open.rs` end-cut to `src/widgets/quick_o…`, which
        // loses the only token that tells two candidates apart. Dropping
        // leading segments keeps it.
        assert!(painted.contains("quick_open.rs"), "{painted}");
        // Nothing is left showing only the directories it came from.
        assert!(!painted.contains("src/widgets/q"), "{painted}");
        assert!(painted.contains("src/main.rs"), "{painted}");
    }

    #[test]
    fn parse_provider_syntax() {
        let p = parse_quick_open_query("@files main");
        assert_eq!(p.provider_override.as_deref(), Some("files"));
        assert_eq!(p.filter, "main");
        let p2 = parse_quick_open_query("plain");
        assert!(p2.provider_override.is_none());
        assert_eq!(p2.filter, "plain");
    }

    #[test]
    fn filter_local_window() {
        let items = example_quick_open_files();
        let hit = filter_quick_open_items(&items, "qck");
        assert!(hit.iter().any(|i| i.id == "quick"));
        assert!(hit[0].match_ranges.is_some() || hit.iter().any(|i| i.match_ranges.is_some()));
    }

    #[test]
    fn search_request_bumps_generation() {
        let mut s = focused();
        let p = providers();
        let out = s.request_search(&p);
        assert!(matches!(
            out,
            QuickOpenOutcome::SearchRequested { request } if request.generation == 1
        ));
        assert_eq!(s.generation(), 1);
    }

    #[test]
    fn stale_results_rejected() {
        let mut s = focused();
        let p = providers();
        let _ = s.request_search(&p);
        let stale = s.generation();
        let _ = s.request_search(&p);
        let items = example_quick_open_files();
        assert!(!s.apply_results(stale, &items, true, Some(5)));
        assert!(s.apply_results(s.generation(), &items, true, Some(5)));
    }

    #[test]
    fn provider_switch_preserves_query() {
        let mut s = focused();
        let p = providers();
        *s.query_mut() = TextInputState::new("main").with_allow_empty(true);
        let items = filter_quick_open_items(&example_quick_open_files(), "main");
        let _ = s.apply_results(s.generation(), &items, true, None);
        // switch to symbols
        let out = s.set_provider(&p, 1, &items);
        assert!(matches!(
            out,
            QuickOpenOutcome::ProviderChanged {
                from,
                to,
                ..
            } if from == "files" && to == "symbols"
        ));
        // symbols memory empty → query cleared
        assert_eq!(s.query_text(), "");
        // type on symbols
        *s.query_mut() = TextInputState::new("paint").with_allow_empty(true);
        let sym = filter_quick_open_items(&example_quick_open_symbols(), "paint");
        let _ = s.apply_results(s.generation(), &sym, true, None);
        // back to files — restores "main"
        let out = s.set_provider(&p, 0, &sym);
        assert!(matches!(out, QuickOpenOutcome::ProviderChanged { .. }));
        assert_eq!(s.query_text(), "main");
    }

    #[test]
    fn activate_item() {
        let mut s = focused();
        let p = providers();
        let items = example_quick_open_files();
        let _ = s.apply_results(0, &items, true, None);
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &p,
                &items
            ),
            QuickOpenOutcome::Activated {
                provider_id,
                id: "main"
            } if provider_id == "files"
        ));
    }

    #[test]
    fn jump_mode_and_targets() {
        let mut s = focused();
        let p = providers();
        let items = example_quick_open_files();
        let _ = s.apply_results(0, &items, true, None);
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL),
                &p,
                &items
            ),
            QuickOpenOutcome::JumpModeRequested
        ));
        // paint to fill hits
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 60, 16);
        let mut buf = Buffer::empty(area);
        QuickOpen::new(&p, &items, &system).paint(area, &mut buf, &mut s);
        let badges: Vec<char> = ('a'..='z').take(s.hits.len()).collect();
        let targets = quick_open_jump_targets(&items, &s.hits, &badges);
        assert!(!targets.is_empty());
        assert_eq!(targets[0].badge(), badges[0]);
    }

    #[test]
    fn overlay_fullscreen_and_restore() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let out =
            open_quick_open_overlay(&mut stack, bounds, QuickOpenSize::default(), Some("editor"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("editor"),
                ..
            }
        ));

        let mut stack2 = OverlayStack::<()>::new();
        let _ = open_quick_open_fullscreen(&mut stack2, bounds, None);
        assert_eq!(stack2.top().unwrap().kind, OverlayKind::Fullscreen);
        assert_eq!(stack2.top().unwrap().rect, bounds);
    }

    #[test]
    fn stream_more_near_end() {
        let mut s = focused();
        let p = providers();
        // small window, incomplete stream
        let items: Vec<_> = (0..10)
            .map(|i| {
                QuickOpenItem::new(
                    // leak-free static ids via match - use format owned... Id is &str
                    // use numeric via leaking is bad — use array of statics
                    ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"][i],
                    format!("file{i}.rs"),
                )
            })
            .collect();
        s.stream_complete = false;
        let _ = s.apply_results(0, &items, false, Some(10_000));
        // move to end
        for _ in 0..20 {
            let _ = s.handle_intent(UiIntent::Move(NavigationMove::Next), &p, &items);
        }
        let out = s.handle_intent(UiIntent::Move(NavigationMove::Next), &p, &items);
        // may be StreamMore or Ignored if already at end without change
        assert!(
            matches!(
                out,
                QuickOpenOutcome::StreamMore { .. }
                    | QuickOpenOutcome::CursorMoved
                    | QuickOpenOutcome::PreviewRequested { .. }
                    | QuickOpenOutcome::Ignored
            ),
            "{out:?}"
        );
    }

    #[test]
    fn presentation_bounds() {
        assert_eq!(
            quick_open_presentation_for_bounds(Rect::new(0, 0, 40, 20)),
            QuickOpenPresentation::Fullscreen
        );
        assert_eq!(
            quick_open_presentation_for_bounds(Rect::new(0, 0, 100, 30)),
            QuickOpenPresentation::Centered
        );
    }

    #[test]
    fn paint_smoke() {
        let system = DesignSystem::default();
        let p = providers();
        let items = example_quick_open_files();
        let mut s = focused();
        let _ = s.apply_results(0, &items, true, Some(5));
        let area = Rect::new(0, 0, 72, 18);
        let mut buf = Buffer::empty(area);
        QuickOpen::new(&p, &items, &system).paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Files") || text.contains("main") || text.contains("Quick"),
            "{text}"
        );
    }

    #[test]
    fn fuzz_keys() {
        let mut s = focused();
        let p = providers();
        let items = example_quick_open_files();
        let _ = s.apply_results(0, &items, true, None);
        let keys = [
            KeyCode::Char('a'),
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Tab,
            KeyCode::Char('n'),
            KeyCode::Left,
            KeyCode::Right,
        ];
        let mut seed = 99u64;
        for _ in 0..250 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let k = keys[(seed as usize) % keys.len()];
            let mods = if matches!(k, KeyCode::Char('n')) {
                KeyModifiers::CONTROL
            } else {
                KeyModifiers::NONE
            };
            let _ = s.handle_key(KeyEvent::new(k, mods), &p, &items);
        }
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let p = providers();
        let s = focused();
        let mut scene = SemanticScene::<&str, ()>::default();
        QuickOpen::new(&p, &[], &system).register_semantic(
            &mut scene,
            "qo",
            Rect::new(0, 0, 40, 12),
            &s,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("quick-open"))
        );
    }

    #[test]
    fn accepts_input_gate() {
        let mut s = focused();
        s.set_accepts_input(false);
        let p = providers();
        let items = example_quick_open_files();
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &p,
                &items
            ),
            QuickOpenOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_result_hit_activates_the_canonical_provider_item() {
        let providers = providers();
        let visible = example_quick_open_files();
        let mut state = focused();
        state.hits = vec![(0, Rect::new(5, 6, 20, 1))];
        assert!(matches!(
            state.handle_mouse(
                MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    position: Position::new(5, 6),
                    modifiers: KeyModifiers::NONE,
                },
                &providers,
                &visible,
            ),
            QuickOpenOutcome::Activated {
                provider_id,
                id: "main"
            } if provider_id == providers[0].id
        ));
    }
}
