// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **HistoryPicker** — reusable recent-history selector.
//!
//! **Mission.** Commands, prompts, searches, sessions, and values need a shared
//! history surface: recency + pinning, search, delete, metadata, groups, preview,
//! privacy redaction, and draft preservation on open/cancel — without the widget
//! owning persistence or secrets.
//!
//! **vs [`super::Picker`].** Picker is generic query+list. HistoryPicker adds
//! pin/delete, redaction hooks, draft stash, grouping, and history-specific
//! chrome.
//! **vs [`crate::patterns::SessionPicker`].** SessionPicker is agent
//! session lifecycle (create/resume/archive/delete); HistoryPicker is value recall.
//! **vs CommandPalette.** Palette executes commands; HistoryPicker recalls past
//! values into a draft.
//!
//! Research: shell history search, prompt histories, session pickers, palettes.
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
    style::{DesignSystem, Glyph, ListRowVisualState, MASK_CELLS, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        HighlightVisual, HighlightedText, Hint, HintBar, MatchRanges, MatchTruncate, Panel,
        PanelChrome, PanelTitleSpec, PanelVariant, TextInput, TextInputOutcome, TextInputState,
        fuzzy_match_label,
    },
};

/// Default overlay id.
pub const HISTORY_PICKER_OVERLAY_ID: &str = "termrock.history_picker";
/// Width under which popover prefers fullscreen presentation.
pub const HISTORY_PICKER_FULLSCREEN_MAX_WIDTH: u16 = 48;
/// Height under which fullscreen is preferred.
pub const HISTORY_PICKER_FULLSCREEN_MAX_HEIGHT: u16 = 14;

// ── Placement ───────────────────────────────────────────────────────────────

/// Preferred size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryPickerSize {
    /// Width.
    pub width: u16,
    /// Height.
    pub height: u16,
}

impl Default for HistoryPickerSize {
    fn default() -> Self {
        Self {
            width: 56,
            height: 14,
        }
    }
}

impl From<HistoryPickerSize> for OverlaySize {
    fn from(value: HistoryPickerSize) -> Self {
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

/// Presentation variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HistoryPickerPresentation {
    /// Anchored compact popover (default).
    #[default]
    Popover,
    /// Full bounds ([`crate::widgets::FullscreenViewer`]-class promotion).
    Fullscreen,
}

impl HistoryPickerPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Popover => "popover",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Derive presentation from bounds.
#[must_use]
pub fn history_picker_presentation_for_bounds(bounds: Rect) -> HistoryPickerPresentation {
    if bounds.width <= HISTORY_PICKER_FULLSCREEN_MAX_WIDTH
        || bounds.height <= HISTORY_PICKER_FULLSCREEN_MAX_HEIGHT
    {
        HistoryPickerPresentation::Fullscreen
    } else {
        HistoryPickerPresentation::Popover
    }
}

/// Place as centered palette-class overlay (upper third).
#[must_use]
pub fn place_history_picker(bounds: Rect, preferred: HistoryPickerSize) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    if bounds.width <= HISTORY_PICKER_FULLSCREEN_MAX_WIDTH
        || bounds.height <= HISTORY_PICKER_FULLSCREEN_MAX_HEIGHT
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

/// Place as anchored popover under `anchor`.
#[must_use]
pub fn place_history_picker_popover(
    bounds: Rect,
    anchor: Rect,
    preferred: HistoryPickerSize,
) -> Rect {
    if bounds.is_empty() {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        OverlaySize::from(preferred),
        OverlayPolicy::for_kind(OverlayKind::Popover),
    )
}

/// Open centered / fullscreen-promoting overlay.
pub fn open_history_picker_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    preferred: HistoryPickerSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::command_palette(
            HISTORY_PICKER_OVERLAY_ID,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Open fullscreen layer.
pub fn open_history_picker_fullscreen<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::fullscreen(HISTORY_PICKER_OVERLAY_ID, opener_focus),
    )
}

/// Open anchored popover overlay.
pub fn open_history_picker_popover_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    preferred: HistoryPickerSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::popover(
            HISTORY_PICKER_OVERLAY_ID,
            anchor,
            OverlaySize::from(preferred),
            opener_focus,
        ),
    )
}

/// Dismiss default overlay.
pub fn dismiss_history_picker_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(HISTORY_PICKER_OVERLAY_ID))
}

// ── Privacy / redaction ─────────────────────────────────────────────────────

/// How sensitive history text is shown in the list (value for apply stays host-owned).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HistoryRedaction {
    /// Show full text.
    #[default]
    None,
    /// Replace with fixed mask.
    MaskAll,
    /// Keep first/last grapheme clusters; mask middle.
    MaskMiddle {
        /// Keep this many leading chars.
        keep_start: usize,
        /// Keep this many trailing chars.
        keep_end: usize,
    },
    /// Host already set [`HistoryEntry::display`]; do not alter.
    HostProvided,
}

impl HistoryRedaction {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MaskAll => "mask-all",
            Self::MaskMiddle { .. } => "mask-middle",
            Self::HostProvided => "host-provided",
        }
    }
}

/// Default middle mask for secrets.
#[must_use]
pub const fn history_redaction_secret() -> HistoryRedaction {
    HistoryRedaction::MaskMiddle {
        keep_start: 2,
        keep_end: 2,
    }
}

/// Apply redaction policy to a display string (never use for crypto).
#[must_use]
pub fn redact_history_text(text: &str, policy: HistoryRedaction) -> String {
    match policy {
        HistoryRedaction::None | HistoryRedaction::HostProvided => text.to_string(),
        HistoryRedaction::MaskAll => {
            if text.is_empty() {
                String::new()
            } else {
                Glyph::Mask.resolve().text.repeat(MASK_CELLS)
            }
        }
        HistoryRedaction::MaskMiddle {
            keep_start,
            keep_end,
        } => {
            // Grapheme clusters, not chars: masking by code point splits a
            // family emoji or a combining accent and leaks half of it
            // (plans/022 Step 3).
            let clusters: Vec<&str> =
                unicode_segmentation::UnicodeSegmentation::graphemes(text, true).collect();
            let n = clusters.len();
            if n == 0 {
                return String::new();
            }
            if keep_start + keep_end >= n {
                return Glyph::Mask.resolve().text.repeat(n.min(MASK_CELLS).max(1));
            }
            let mut out: String = clusters[..keep_start].concat();
            out.push_str(Glyph::Ellipsis.resolve().text);
            out.push_str(&clusters[n - keep_end..].concat());
            out
        }
    }
}

// ── Entry model ─────────────────────────────────────────────────────────────

/// Kind of history value (product-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum HistoryKind {
    /// Shell / app command.
    #[default]
    Command,
    /// Multi-line prompt / message.
    Prompt,
    /// Search query.
    Search,
    /// Session / conversation.
    Session,
    /// Generic value.
    Value,
    /// Host-defined.
    Custom,
}

impl HistoryKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Prompt => "prompt",
            Self::Search => "search",
            Self::Session => "session",
            Self::Value => "value",
            Self::Custom => "custom",
        }
    }

    /// Short badge.
    #[must_use]
    pub const fn badge(self, ascii: bool) -> &'static str {
        match (self, ascii) {
            (Self::Command, true) => "cmd",
            (Self::Command, false) => "⌘",
            (Self::Prompt, true) => "prm",
            (Self::Prompt, false) => "✎",
            (Self::Search, true) => "src",
            (Self::Search, false) => "⌕",
            (Self::Session, true) => "ses",
            (Self::Session, false) => "◎",
            (Self::Value, true) => "val",
            (Self::Value, false) => "·",
            (Self::Custom, _) => "…",
        }
    }
}

/// One history row (host-projected; persistence is host-owned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry<Id> {
    /// Stable id for delete/pin.
    pub id: Id,
    /// Full value to apply on select (host may keep secrets only in memory).
    pub value: String,
    /// Painted label (after redaction if host used hooks).
    pub display: String,
    /// Pinned to top.
    pub pinned: bool,
    /// Group header key.
    pub group: Option<String>,
    /// Metadata (time, source).
    pub meta: Option<String>,
    /// Preview pane text.
    pub preview: Option<String>,
    /// Mark sensitive (default redaction applies when policy set on paint).
    pub sensitive: bool,
    /// Kind badge.
    pub kind: HistoryKind,
    /// Recency rank (lower = newer); host sets; used for sort.
    pub recency: u64,
    /// Fuzzy ranges into display.
    pub match_ranges: Option<MatchRanges>,
}

impl<Id> HistoryEntry<Id> {
    /// Construct with value used as display.
    #[must_use]
    pub fn new(id: Id, value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            id,
            display: value.clone(),
            value,
            pinned: false,
            group: None,
            meta: None,
            preview: None,
            sensitive: false,
            kind: HistoryKind::Value,
            recency: 0,
            match_ranges: None,
        }
    }

    /// Display override.
    #[must_use]
    pub fn display(mut self, d: impl Into<String>) -> Self {
        self.display = d.into();
        self
    }

    /// Pin.
    #[must_use]
    pub const fn pinned(mut self, on: bool) -> Self {
        self.pinned = on;
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = Some(g.into());
        self
    }

    /// Meta.
    #[must_use]
    pub fn meta(mut self, m: impl Into<String>) -> Self {
        self.meta = Some(m.into());
        self
    }

    /// Preview.
    #[must_use]
    pub fn preview(mut self, p: impl Into<String>) -> Self {
        self.preview = Some(p.into());
        self
    }

    /// Sensitive.
    #[must_use]
    pub const fn sensitive(mut self, on: bool) -> Self {
        self.sensitive = on;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: HistoryKind) -> Self {
        self.kind = k;
        self
    }

    /// Recency rank.
    #[must_use]
    pub const fn recency(mut self, r: u64) -> Self {
        self.recency = r;
        self
    }

    /// Apply redaction to display when sensitive.
    #[must_use]
    pub fn with_redaction(mut self, policy: HistoryRedaction) -> Self {
        if self.sensitive
            && !matches!(
                policy,
                HistoryRedaction::HostProvided | HistoryRedaction::None
            )
        {
            self.display = redact_history_text(&self.value, policy);
        } else if matches!(policy, HistoryRedaction::HostProvided) {
            // leave display
        } else if !self.sensitive && matches!(policy, HistoryRedaction::None) {
            // leave
        }
        self
    }
}

/// Filter + sort for host catalogs (sync path).
#[must_use]
pub fn filter_history_entries<Id: Clone>(
    entries: &[HistoryEntry<Id>],
    query: &str,
) -> Vec<HistoryEntry<Id>> {
    let q = query.trim();
    let mut out: Vec<HistoryEntry<Id>> = entries
        .iter()
        .filter_map(|e| {
            if q.is_empty() {
                let mut c = e.clone();
                c.match_ranges = None;
                return Some(c);
            }
            let hay = format!(
                "{} {} {}",
                e.display,
                e.value,
                e.meta.as_deref().unwrap_or("")
            );
            fuzzy_match_label(q, &hay).map(|(score, ranges)| {
                let mut c = e.clone();
                // Prefer ranges on display if match there
                c.match_ranges = fuzzy_match_label(q, &e.display)
                    .map(|(_, r)| r)
                    .or(Some(ranges));
                let _ = score;
                c
            })
        })
        .collect();
    out.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| a.recency.cmp(&b.recency))
            .then_with(|| a.display.cmp(&b.display))
    });
    out
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes (host persists / applies draft).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HistoryPickerOutcome<Id> {
    /// No change.
    Ignored,
    /// Search query changed.
    QueryChanged {
        /// Filter text.
        query: String,
    },
    /// Cursor moved.
    CursorMoved,
    /// Entry selected — apply `value` to draft; history closes.
    Selected {
        /// Id.
        id: Id,
        /// Full value (not redacted display).
        value: String,
    },
    /// Host should delete entry from store.
    Deleted {
        /// Id.
        id: Id,
    },
    /// Host should persist pin flip.
    PinToggled {
        /// Id.
        id: Id,
        /// New pin state.
        pinned: bool,
    },
    /// Cancelled — restore stashed draft via [`HistoryPickerState::take_draft`].
    Cancelled,
    /// Opened and draft was captured.
    Opened {
        /// Whether a draft was stashed.
        draft_stashed: bool,
    },
    /// Presentation changed.
    PresentationChanged {
        /// New presentation.
        presentation: HistoryPickerPresentation,
    },
}

// ── State ───────────────────────────────────────────────────────────────────

/// History picker state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPickerState<Id> {
    query: TextInputState,
    collection: CollectionState<usize>,
    /// Draft captured at open (restored on cancel).
    draft: Option<String>,
    open: bool,
    focused: bool,
    accepts_input: bool,
    presentation: HistoryPickerPresentation,
    presentation_override: Option<HistoryPickerPresentation>,
    redaction: HistoryRedaction,
    show_preview: bool,
    hits: Vec<(usize, Rect)>,
    /// Row the pointer is over. Hover washes; it never commits.
    hovered: Option<usize>,
    scroll: usize,
    painted_rows: u16,
    _id: std::marker::PhantomData<Id>,
}

impl<Id: Clone + PartialEq> Default for HistoryPickerState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq> HistoryPickerState<Id> {
    /// Closed empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: TextInputState::new("").with_allow_empty(true),
            collection: CollectionState::new().orientation(RovingOrientation::Vertical),
            draft: None,
            open: false,
            focused: true,
            accepts_input: true,
            presentation: HistoryPickerPresentation::Popover,
            presentation_override: None,
            redaction: HistoryRedaction::None,
            show_preview: true,
            hits: Vec::new(),
            hovered: None,
            scroll: 0,
            painted_rows: 0,
            _id: std::marker::PhantomData,
        }
    }

    /// Open picker, optionally stashing the current draft.
    pub fn open(&mut self, current_draft: Option<String>) -> HistoryPickerOutcome<Id> {
        self.open = true;
        self.focused = true;
        self.accepts_input = true;
        self.query = TextInputState::new("").with_allow_empty(true);
        let draft_stashed = current_draft.is_some();
        self.draft = current_draft;
        HistoryPickerOutcome::Opened { draft_stashed }
    }

    /// Close without consuming draft (host may still take_draft).
    pub fn close(&mut self) {
        self.open = false;
        self.query = TextInputState::new("").with_allow_empty(true);
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Peek stashed draft.
    #[must_use]
    pub fn draft(&self) -> Option<&str> {
        self.draft.as_deref()
    }

    /// Take stashed draft (after Cancelled).
    pub fn take_draft(&mut self) -> Option<String> {
        self.draft.take()
    }

    /// Clear draft without restore.
    pub fn discard_draft(&mut self) {
        self.draft = None;
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Redaction policy for sensitive rows.
    pub fn set_redaction(&mut self, policy: HistoryRedaction) {
        self.redaction = policy;
    }

    /// Redaction.
    #[must_use]
    pub const fn redaction(&self) -> HistoryRedaction {
        self.redaction
    }

    /// Preview pane.
    pub fn set_show_preview(&mut self, on: bool) {
        self.show_preview = on;
    }

    /// Presentation override.
    pub fn set_presentation_override(&mut self, p: Option<HistoryPickerPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> HistoryPickerPresentation {
        self.presentation
    }

    /// Query.
    #[must_use]
    pub fn query_text(&self) -> &str {
        self.query.value()
    }

    /// Query mut.
    pub const fn query_mut(&mut self) -> &mut TextInputState {
        &mut self.query
    }

    /// Cursor.
    #[must_use]
    pub fn cursor_index(&self) -> usize {
        self.collection.active().copied().unwrap_or(0)
    }

    /// Hits after paint.
    #[must_use]
    pub fn hits(&self) -> &[(usize, Rect)] {
        &self.hits
    }

    fn live(&self) -> bool {
        self.open && self.enabled_gate()
    }

    fn enabled_gate(&self) -> bool {
        self.accepts_input && self.focused
    }

    fn entries(visible: &[HistoryEntry<Id>]) -> Vec<CollectionItem<usize>> {
        visible
            .iter()
            .enumerate()
            .map(|(i, e)| CollectionItem {
                id: i,
                enabled: true,
                label: e.display.clone(),
                parent: None,
            })
            .collect()
    }

    /// Reconcile cursor after host rebuilds visible list.
    pub fn reconcile(&mut self, visible: &[HistoryEntry<Id>]) {
        let entries = Self::entries(visible);
        let _ = self.collection.reconcile(&entries);
        self.scroll = self.scroll.min(visible.len().saturating_sub(1));
    }

    /// Select cursor entry.
    pub fn select_cursor(&mut self, visible: &[HistoryEntry<Id>]) -> HistoryPickerOutcome<Id> {
        let idx = self.cursor_index();
        let Some(e) = visible.get(idx) else {
            return HistoryPickerOutcome::Ignored;
        };
        let id = e.id.clone();
        let value = e.value.clone();
        self.discard_draft();
        self.close();
        HistoryPickerOutcome::Selected { id, value }
    }

    /// Delete cursor (host removes from store then refreshes).
    pub fn delete_cursor(&mut self, visible: &[HistoryEntry<Id>]) -> HistoryPickerOutcome<Id> {
        let idx = self.cursor_index();
        let Some(e) = visible.get(idx) else {
            return HistoryPickerOutcome::Ignored;
        };
        HistoryPickerOutcome::Deleted { id: e.id.clone() }
    }

    /// Toggle pin on cursor.
    pub fn toggle_pin_cursor(&mut self, visible: &[HistoryEntry<Id>]) -> HistoryPickerOutcome<Id> {
        let idx = self.cursor_index();
        let Some(e) = visible.get(idx) else {
            return HistoryPickerOutcome::Ignored;
        };
        HistoryPickerOutcome::PinToggled {
            id: e.id.clone(),
            pinned: !e.pinned,
        }
    }

    /// Keyboard.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        visible: &[HistoryEntry<Id>],
    ) -> HistoryPickerOutcome<Id> {
        if !self.live() || key.kind == KeyEventKind::Release {
            return HistoryPickerOutcome::Ignored;
        }
        self.reconcile(visible);

        // Ctrl+D delete, Ctrl+P pin
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('d' | 'D'))
        {
            return self.delete_cursor(visible);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('p' | 'P'))
        {
            return self.toggle_pin_cursor(visible);
        }

        if key.code == KeyCode::Esc {
            self.close();
            return HistoryPickerOutcome::Cancelled;
        }

        if matches!(
            key.code,
            KeyCode::Down
                | KeyCode::Up
                | KeyCode::PageDown
                | KeyCode::PageUp
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::Enter
        ) || (key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('j' | 'k' | 'J' | 'K')))
        {
            if let Some(intent) = default_history_picker_intent(key) {
                let out = self.handle_intent(intent, visible);
                if !matches!(out, HistoryPickerOutcome::Ignored) {
                    return out;
                }
            }
        }

        match self.query.handle_key(key) {
            TextInputOutcome::Changed => HistoryPickerOutcome::QueryChanged {
                query: self.query_text().to_string(),
            },
            TextInputOutcome::Submitted(_) => self.select_cursor(visible),
            TextInputOutcome::Ignored => {
                if let Some(intent) = default_history_picker_intent(key) {
                    self.handle_intent(intent, visible)
                } else {
                    HistoryPickerOutcome::Ignored
                }
            }
            TextInputOutcome::Cancelled => {
                self.close();
                HistoryPickerOutcome::Cancelled
            }
            _ => HistoryPickerOutcome::Ignored,
        }
    }

    /// Intent.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        visible: &[HistoryEntry<Id>],
    ) -> HistoryPickerOutcome<Id> {
        if !self.live() {
            return HistoryPickerOutcome::Ignored;
        }
        self.reconcile(visible);
        let entries = Self::entries(visible);
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
                    return HistoryPickerOutcome::Ignored;
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
                    HistoryPickerOutcome::CursorMoved
                } else {
                    HistoryPickerOutcome::Ignored
                }
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Toggle => self.select_cursor(visible),
            UiIntent::Cancel | UiIntent::Close => {
                self.close();
                HistoryPickerOutcome::Cancelled
            }
            _ => HistoryPickerOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        visible: &[HistoryEntry<Id>],
    ) -> HistoryPickerOutcome<Id> {
        if !self.live() {
            return HistoryPickerOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                for (idx, rect) in &self.hits {
                    if rect_contains(*rect, event.position) {
                        self.collection.set_active(Some(*idx));
                        return self.select_cursor(visible);
                    }
                }
                HistoryPickerOutcome::Ignored
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
                    if rect_contains(*rect, event.position) && self.cursor_index() != *idx {
                        self.collection.set_active(Some(*idx));
                        return HistoryPickerOutcome::CursorMoved;
                    }
                }
                HistoryPickerOutcome::Ignored
            }
            _ => HistoryPickerOutcome::Ignored,
        }
    }

    /// Sync presentation.
    pub fn sync_presentation_from_bounds(&mut self, bounds: Rect) -> HistoryPickerOutcome<Id> {
        if self.presentation_override.is_some() {
            return HistoryPickerOutcome::Ignored;
        }
        let next = history_picker_presentation_for_bounds(bounds);
        if next != self.presentation {
            self.presentation = next;
            HistoryPickerOutcome::PresentationChanged { presentation: next }
        } else {
            HistoryPickerOutcome::Ignored
        }
    }
}

fn rect_contains(rect: Rect, pos: Position) -> bool {
    pos.x >= rect.x
        && pos.y >= rect.y
        && pos.x < rect.x.saturating_add(rect.width)
        && pos.y < rect.y.saturating_add(rect.height)
}

/// Default intents.
#[must_use]
pub fn default_history_picker_intent(key: KeyEvent) -> Option<UiIntent> {
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

// ── Widget ──────────────────────────────────────────────────────────────────

/// History picker paint.
#[derive(Debug, Clone, Copy)]
pub struct HistoryPicker<'a, Id> {
    entries: &'a [HistoryEntry<Id>],
    system: &'a DesignSystem,
    title: &'a str,
    colorless: bool,
    footer_hint: Option<&'a str>,
    empty_message: &'a str,
}

/// Footer chords for the history picker, painted through [`HintBar`].
///
/// One separator and one alignment rule for every overlay footer: the flat
/// sentence these replaced joined its chords by hand (plans/009 Step 1).
const HISTORY_PICKER_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "↑↓",
        label: "move",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "apply",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "C-p",
        label: "pin",
        priority: 40,
        visible: true,
    },
    Hint {
        chord: "C-d",
        label: "delete",
        priority: 50,
        visible: true,
    },
    Hint {
        chord: "esc",
        label: "close",
        priority: 60,
        visible: true,
    },
];

impl<'a, Id> HistoryPicker<'a, Id> {
    /// Visible entries + design system.
    #[must_use]
    pub const fn new(entries: &'a [HistoryEntry<Id>], system: &'a DesignSystem) -> Self {
        Self {
            entries,
            system,
            title: "History",
            colorless: false,
            footer_hint: None,
            empty_message: "No history yet",
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = t;
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

    /// Empty message.
    #[must_use]
    pub const fn empty_message(mut self, m: &'a str) -> Self {
        self.empty_message = m;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut HistoryPickerState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.hits.clear();
        if area.is_empty() {
            return;
        }
        if state.presentation_override.is_none() {
            let _ = state.sync_presentation_from_bounds(area);
        }
        state.reconcile(self.entries);

        let surface = state.focused && state.accepts_input;
        let emphasis = if surface {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        // The title states how much the picker holds and what is filtering it,
        // through the one title grammar every panel uses (plans/009, 017 §B2).
        let query = state.query_text();
        let mut spec = PanelTitleSpec::new(self.title).count(self.entries.len());
        if !query.is_empty() {
            spec = spec.filter(query);
        }
        let panel = Panel::new(self.system)
            .variant(PanelVariant::Bordered)
            .overlay(true)
            .title_spec(spec)
            .emphasis(emphasis);
        let inner = panel.inner(area);
        ratatui_core::widgets::Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let narrow = area.width < 36;
        let tiny = area.height < 8;
        let show_footer = !tiny && area.height >= 8 && !narrow;
        let show_preview = state.show_preview && !tiny && area.width >= 52 && area.height >= 10;

        let mut y = inner.y;
        let bottom = if show_footer {
            inner.bottom().saturating_sub(1)
        } else {
            inner.bottom()
        };

        // Draft indicator
        if state.draft.is_some() && y < bottom {
            let msg = { "⊙ draft preserved · esc restores" };
            buffer.set_stringn(
                inner.x,
                y,
                &take_display_cols(msg, usize::from(inner.width)),
                usize::from(inner.width),
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        // Query
        if y < bottom {
            state.query.set_focused(surface);
            let _ = TextInput::new("", self.system)
                .placeholder(if narrow {
                    "Filter…"
                } else {
                    "Filter history"
                })
                .paint(
                    Rect::new(inner.x, y, inner.width, 1),
                    buffer,
                    &mut state.query,
                );
            y = y.saturating_add(1);
        }

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

        let (list_area, preview_area) = if show_preview {
            let pw = (inner.width / 3).clamp(14, 24);
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

        if let Some(pa) = preview_area {
            let vx = pa.x.saturating_sub(1);
            for row in y..bottom {
                buffer.set_stringn(vx, row, "│", 1, self.system.style(Role::Border));
            }
            self.paint_preview(pa, buffer, state);
        }

        self.paint_list(list_area, buffer, state);

        if show_footer {
            let footer = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
            if let Some(hint) = self.footer_hint {
                buffer.set_stringn(
                    footer.x,
                    footer.y,
                    &take_display_cols(hint, usize::from(footer.width)),
                    usize::from(footer.width),
                    self.system.style(Role::TextMuted),
                );
            } else {
                ratatui_core::widgets::Widget::render(
                    &HintBar::new(HISTORY_PICKER_HINTS, self.system),
                    footer,
                    buffer,
                );
            }
        }
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut HistoryPickerState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            state.painted_rows = 0;
            return;
        }
        if self.entries.is_empty() {
            buffer.set_stringn(
                area.x,
                area.y,
                &take_display_cols(self.empty_message, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            state.painted_rows = 1;
            return;
        }

        let cursor = state.cursor_index();
        let surface = state.focused && state.accepts_input;
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
            if let Some(g) = entry.group.as_deref() {
                if last_group != Some(g) {
                    last_group = Some(g);
                    buffer.set_stringn(
                        area.x,
                        y,
                        &take_display_cols(g, usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::TextMuted),
                    );
                    y = y.saturating_add(1);
                    painted = painted.saturating_add(1);
                    if y >= area.bottom() {
                        break;
                    }
                }
            }

            let active = i == cursor && surface;
            let rect = Rect::new(area.x, y, area.width, 1);
            state.hits.push((i, rect));
            let recipe = self.system.resolve_list_row(ListRowVisualState {
                selected: active,
                focused: active,
                hovered: state.hovered == Some(i),
                enabled: true,
                loading: false,
                checked: entry.pinned,
                ..ListRowVisualState::default()
            });
            if recipe.use_tint {
                buffer.set_style(rect, recipe.tint);
            }

            // Apply redaction for display if sensitive
            let display = if entry.sensitive
                && !matches!(
                    state.redaction,
                    HistoryRedaction::None | HistoryRedaction::HostProvided
                ) {
                redact_history_text(&entry.value, state.redaction)
            } else {
                entry.display.clone()
            };

            // The pin slot is reserved on every row: a column that only exists
            // when a row is pinned shifts every other column beside it, so a
            // pinned list read as a ragged one (plans/009 Step 6).
            let pin = if entry.pinned { "★ " } else { "  " };
            let kind = entry.kind.badge(false);
            let mut x = area.x;
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

            let chrome = super::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected: active,
                    focused: active,
                    hovered: state.hovered == Some(i),
                    enabled: true,
                    loading: false,
                    checked: entry.pinned,
                    ..ListRowVisualState::default()
                },
            );
            chrome.paint(buffer, rect);
            x = x.saturating_add(3);
            let pw = display_cols(pin) as u16;
            buffer.set_stringn(
                x,
                y,
                pin,
                usize::from(pw),
                self.system.style(Role::TextMuted),
            );
            x = x.saturating_add(pw);
            let kb = format!("{kind} ");
            let kw = display_cols(&kb) as u16;
            buffer.set_stringn(
                x,
                y,
                &kb,
                usize::from(kw),
                self.system.style(Role::TextMuted),
            );
            x = x.saturating_add(kw);

            let meta_w = entry
                .meta
                .as_ref()
                .map(|m| (display_cols(m) as u16 + 1).min(area.right().saturating_sub(x) / 3))
                .unwrap_or(0);
            let label_w = area.right().saturating_sub(x).saturating_sub(meta_w);

            if label_w > 0 {
                if let Some(ranges) = &entry.match_ranges {
                    let visual = if active {
                        HighlightVisual::Selected
                    } else {
                        HighlightVisual::Normal
                    };
                    let _ = HighlightedText::new(&display, ranges.as_slice(), self.system)
                        .visual(visual)
                        .truncate(MatchTruncate::End)
                        .paint(Rect::new(x, y, label_w, 1), buffer);
                } else {
                    buffer.set_stringn(
                        x,
                        y,
                        &take_display_cols(&display, usize::from(label_w)),
                        usize::from(label_w),
                        base,
                    );
                }
            }
            if meta_w > 0 {
                if let Some(m) = &entry.meta {
                    let mx = area.right().saturating_sub(meta_w);
                    buffer.set_stringn(
                        mx,
                        y,
                        &take_display_cols(m, usize::from(meta_w)),
                        usize::from(meta_w),
                        self.system.style(Role::TextMuted),
                    );
                }
            }

            y = y.saturating_add(1);
            painted = painted.saturating_add(1);
        }
        state.painted_rows = painted;
    }

    fn paint_preview(&self, area: Rect, buffer: &mut Buffer, state: &HistoryPickerState<Id>)
    where
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        buffer.set_stringn(
            area.x,
            area.y,
            &take_display_cols("Preview", usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::TextMuted),
        );
        let Some(entry) = self.entries.get(state.cursor_index()) else {
            return;
        };
        let text = entry.preview.as_deref().unwrap_or(entry.value.as_str());
        let text = if entry.sensitive
            && !matches!(
                state.redaction,
                HistoryRedaction::None | HistoryRedaction::HostProvided
            ) {
            redact_history_text(text, state.redaction)
        } else {
            text.to_string()
        };
        let mut y = area.y.saturating_add(1);
        for line in text
            .lines()
            .take(usize::from(area.height.saturating_sub(1)))
        {
            if y >= area.bottom() {
                break;
            }
            buffer.set_stringn(
                area.x,
                y,
                &take_display_cols(line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::Text),
            );
            y = y.saturating_add(1);
        }
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &HistoryPickerState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: Clone + PartialEq,
    {
        if area.is_empty() {
            return;
        }
        let desc = format!(
            "history-picker open={} q={:?} n={} redaction={}",
            state.is_open(),
            state.query_text(),
            self.entries.len(),
            state.redaction().id()
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Menu)
                .label("history-picker")
                .description(desc)
                .focusable(true)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &HistoryPicker<'_, Id> {
    type State = HistoryPickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for HistoryPicker<'_, Id> {
    type State = HistoryPickerState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

// ── Sample data ─────────────────────────────────────────────────────────────

/// Demo history catalog.
#[must_use]
pub fn example_history_entries() -> Vec<HistoryEntry<&'static str>> {
    vec![
        HistoryEntry::new("1", "cargo test -p termrock")
            .kind(HistoryKind::Command)
            .meta("2m ago")
            .pinned(true)
            .group("Pinned")
            .recency(0)
            .preview("Run crate tests"),
        HistoryEntry::new("2", "git status")
            .kind(HistoryKind::Command)
            .meta("10m ago")
            .group("Today")
            .recency(1),
        HistoryEntry::new("3", "explain the OverlayStack Esc law")
            .kind(HistoryKind::Prompt)
            .meta("1h ago")
            .group("Today")
            .recency(2)
            .preview("Prompt → agent"),
        HistoryEntry::new("4", "stepper")
            .kind(HistoryKind::Search)
            .meta("yesterday")
            .group("Earlier")
            .recency(3),
        HistoryEntry::new("5", "session: onboarding-wip")
            .kind(HistoryKind::Session)
            .meta("2d ago")
            .group("Earlier")
            .recency(4),
        HistoryEntry::new("6", "sk-live-secret-example-value")
            .kind(HistoryKind::Value)
            .sensitive(true)
            .meta("3d ago")
            .group("Earlier")
            .recency(5)
            .preview("API key (redacted in list)"),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn catalog() -> Vec<HistoryEntry<&'static str>> {
        example_history_entries()
    }

    fn open_state() -> HistoryPickerState<&'static str> {
        let mut s = HistoryPickerState::new();
        let _ = s.open(Some("draft text".into()));
        s
    }

    #[test]
    fn pinned_and_unpinned_rows_start_their_text_at_one_column() {
        use ratatui_core::buffer::Buffer;
        let system = DesignSystem::default();
        let entries = catalog();
        let mut state = open_state();
        let area = Rect::new(0, 0, 60, 16);
        let mut buffer = Buffer::empty(area);
        HistoryPicker::new(&entries, &system).paint(area, &mut buffer, &mut state);

        // A pinned row and an unpinned row must agree on where their kind
        // badge starts: the pin slot is reserved either way.
        let row_text = |y: u16| -> String {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        };
        // Unicode kind badges: command, prompt, search. Position by cell, not
        // by byte — a `★` is three bytes and one column.
        let badge_col = |line: &str| line.chars().position(|ch| matches!(ch, '⌘' | '✎' | '⌕'));
        let mut columns: Vec<usize> = Vec::new();
        for y in 0..area.height {
            let line = row_text(y);
            if let Some(col) = badge_col(&line) {
                columns.push(col);
            }
        }
        assert!(columns.len() >= 2, "expected several rows with kind badges");
        assert!(
            columns.windows(2).all(|pair| pair[0] == pair[1]),
            "kind badges start at different columns: {columns:?}"
        );
    }

    #[test]
    fn redact_mask_middle() {
        let s = redact_history_text(
            "sk-live-secret-example",
            HistoryRedaction::MaskMiddle {
                keep_start: 2,
                keep_end: 2,
            },
        );
        assert!(s.starts_with("sk"));
        assert!(s.contains('…'));
        assert!(s.ends_with("le"));
    }

    #[test]
    fn filter_prefers_pinned() {
        let c = catalog();
        let f = filter_history_entries(&c, "");
        assert!(f[0].pinned);
    }

    #[test]
    fn draft_preserved_on_cancel() {
        let mut s = open_state();
        assert_eq!(s.draft(), Some("draft text"));
        let vis = filter_history_entries(&catalog(), "");
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &vis),
            HistoryPickerOutcome::Cancelled
        ));
        assert_eq!(s.take_draft().as_deref(), Some("draft text"));
        assert!(!s.is_open());
    }

    #[test]
    fn select_applies_value_and_discards_draft() {
        let mut s = open_state();
        let vis = filter_history_entries(&catalog(), "");
        s.reconcile(&vis);
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            HistoryPickerOutcome::Selected { value, .. } if value.contains("cargo")
        ));
        assert!(s.draft().is_none());
        assert!(!s.is_open());
    }

    #[test]
    fn delete_and_pin_outcomes() {
        let mut s = open_state();
        let vis = filter_history_entries(&catalog(), "");
        s.reconcile(&vis);
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
                &vis
            ),
            HistoryPickerOutcome::Deleted { id: "1" }
        ));
        assert!(matches!(
            s.handle_key(
                KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
                &vis
            ),
            HistoryPickerOutcome::PinToggled {
                id: "1",
                pinned: false
            }
        ));
    }

    #[test]
    fn query_filters() {
        let c = catalog();
        let hit = filter_history_entries(&c, "git");
        assert!(hit.iter().any(|e| e.id == "2"));
        assert!(!hit.iter().any(|e| e.id == "6"));
    }

    #[test]
    fn sensitive_redacted_on_paint() {
        let system = DesignSystem::default();
        let mut entries = catalog();
        // apply policy via state
        let mut s = open_state();
        s.set_redaction(history_redaction_secret());
        let vis = filter_history_entries(&entries, "");
        let area = Rect::new(0, 0, 60, 14);
        let mut buf = Buffer::empty(area);
        HistoryPicker::new(&vis, &system).paint(area, &mut buf, &mut s);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            !text.contains("sk-live-secret-example-value"),
            "secret leaked: {text}"
        );
        let _ = &mut entries;
    }

    #[test]
    fn overlay_restore_focus() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&'static str>::new();
        let out = open_history_picker_overlay(
            &mut stack,
            bounds,
            HistoryPickerSize::default(),
            Some("composer"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert!(matches!(
            stack.handle_escape(),
            OverlayOutcome::Dismissed {
                focus: Some("composer"),
                ..
            }
        ));
    }

    #[test]
    fn presentation_bounds() {
        assert_eq!(
            history_picker_presentation_for_bounds(Rect::new(0, 0, 40, 20)),
            HistoryPickerPresentation::Fullscreen
        );
        assert_eq!(
            history_picker_presentation_for_bounds(Rect::new(0, 0, 80, 24)),
            HistoryPickerPresentation::Popover
        );
    }

    #[test]
    fn fuzz_keys() {
        let mut s = open_state();
        let vis = filter_history_entries(&catalog(), "");
        s.reconcile(&vis);
        let keys = [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Char('a'),
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char('d'),
            KeyCode::Char('p'),
        ];
        let mut seed = 11u64;
        for _ in 0..200 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let mods = if matches!(k, KeyCode::Char('d' | 'p')) {
                KeyModifiers::CONTROL
            } else {
                KeyModifiers::NONE
            };
            if !s.is_open() {
                let _ = s.open(Some("x".into()));
            }
            let _ = s.handle_key(KeyEvent::new(k, mods), &vis);
        }
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let s = open_state();
        let mut scene = SemanticScene::<&str, ()>::default();
        HistoryPicker::new(&[], &system).register_semantic(
            &mut scene,
            "hp",
            Rect::new(0, 0, 40, 10),
            &s,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("history-picker"))
        );
    }

    #[test]
    fn accepts_input_gate() {
        let mut s = open_state();
        s.set_accepts_input(false);
        let vis = filter_history_entries(&catalog(), "");
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &vis),
            HistoryPickerOutcome::Ignored
        ));
    }

    #[test]
    fn mouse_hit_selects_the_painted_history_entry() {
        let visible = catalog();
        let mut state = open_state();
        state.hits = vec![(0, Rect::new(3, 4, 20, 1))];
        let out = state.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(3, 4),
                modifiers: KeyModifiers::NONE,
            },
            &visible,
        );
        assert!(matches!(
            out,
            HistoryPickerOutcome::Selected { ref id, .. } if id == &visible[0].id
        ));
    }
}
