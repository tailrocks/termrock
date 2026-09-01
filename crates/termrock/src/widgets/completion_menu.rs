// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **CompletionMenu** — reusable anchored suggestion surface for editors and inputs.
//!
//! **Mission.** Host-ranked candidates with groups, fuzzy ranges, kind glyphs,
//! details, documentation preview, async generation gates, loading / empty /
//! stale chrome, and commit characters. The menu uses **active descendant**
//! semantics: editor focus stays on the field; selection is navigated without
//! a focus trap ([`OverlayKind::Completion`] policy).
//!
//! **Geometry.** Placement is [`OverlayStack`]-owned via helpers. The widget
//! clamps and flips relative to the anchor (never covers it) and **promotes to
//! fullscreen** on small terminals.
//!
//! **Host owns.** Ranking, filtering, language / LSP, token replacement ranges,
//! insert text, and process I/O.
//! **TermRock owns.** Selection, scroll, paint, intents (Tab/Enter/Esc), hits,
//! presentation contraction, generation race gates.
//!
//! Research: LSP completion UIs, prompt-toolkit, terminal editors, Grok Build
//! prompt completion.

#![allow(unused_variables, unused_mut)] // unit-test fixtures
use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Style,
    widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{
        NavigationMove, OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy, OverlaySize,
        OverlaySpec, OverlayStack, PageMove, SemanticNode, SemanticRole, SemanticScene,
        SemanticState, UiIntent, place_overlay,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
};

/// Default stable overlay id.
pub const COMPLETION_OVERLAY_ID: &str = "termrock.completion";
/// Width at or below which presentation promotes toward fullscreen.
pub const COMPLETION_FULLSCREEN_MAX_WIDTH: u16 = 28;
/// Height at or below which presentation promotes toward fullscreen.
pub const COMPLETION_FULLSCREEN_MAX_HEIGHT: u16 = 10;
/// Default documentation side-panel width when details are shown.
pub const COMPLETION_DOCS_DEFAULT_WIDTH: u16 = 28;

/// Default "still fetching" copy, and its ASCII twin.
///
/// Two constants rather than one gated literal so a host-supplied message
/// survives the ASCII profile: only the *default* is swapped.
const LOADING_MESSAGE: &str = "Loading…";
const LOADING_MESSAGE_ASCII: &str = "Loading...";

// ── Model ───────────────────────────────────────────────────────────────────

/// Async / empty / stale chrome status (host drives; paint reflects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CompletionStatus {
    /// Results ready (may still be empty → empty message).
    #[default]
    Ready,
    /// Waiting on host fetch (generation in flight).
    Loading,
    /// Explicit empty (optional distinct copy from Ready+[]).
    Empty,
    /// Results belong to an older generation (show stale cue; host should refresh).
    Stale,
}

impl CompletionStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Stale => "stale",
        }
    }
}

/// Anchored popup vs fullscreen promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum CompletionPresentation {
    /// Below/above anchor (default).
    #[default]
    Anchored,
    /// Nearly full bounds (small terminals).
    Fullscreen,
}

impl CompletionPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Anchored => "anchored",
            Self::Fullscreen => "fullscreen",
        }
    }
}

/// Choose presentation from terminal bounds.
#[must_use]
pub fn completion_presentation_for(bounds: Rect) -> CompletionPresentation {
    if bounds.is_empty() {
        return CompletionPresentation::Anchored;
    }
    if bounds.width <= COMPLETION_FULLSCREEN_MAX_WIDTH
        || bounds.height <= COMPLETION_FULLSCREEN_MAX_HEIGHT
    {
        CompletionPresentation::Fullscreen
    } else {
        CompletionPresentation::Anchored
    }
}

/// One borrowed completion candidate (host-ranked).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionCandidate<'a, Id> {
    /// Stable identity (caller-owned; selection + commit).
    pub id: Id,
    /// Primary label (Unicode display-width measured).
    pub label: &'a str,
    /// Optional trailing kind annotation text (e.g. "fn", "keyword").
    pub kind: Option<&'a str>,
    /// Optional kind glyph / icon (single-cell preferred; host-owned).
    pub kind_glyph: Option<&'a str>,
    /// Optional secondary detail (signature, module path) — right/muted column.
    pub detail: Option<&'a str>,
    /// Optional documentation body for the side preview.
    pub documentation: Option<&'a str>,
    /// Optional group label; consecutive same-group rows share a header.
    pub group: Option<&'a str>,
    /// Whether selectable / committable.
    pub enabled: bool,
    /// Optional precomputed match ranges into [`Self::label`] (fuzzy/search).
    pub match_ranges: Option<&'a [crate::widgets::MatchRange]>,
}

impl<'a, Id> CompletionCandidate<'a, Id> {
    /// Enabled candidate without annotations.
    #[must_use]
    pub const fn new(id: Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            kind: None,
            kind_glyph: None,
            detail: None,
            documentation: None,
            group: None,
            enabled: true,
            match_ranges: None,
        }
    }

    /// Trailing kind annotation.
    #[must_use]
    pub const fn kind(mut self, kind: &'a str) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Kind glyph (icon column).
    #[must_use]
    pub const fn kind_glyph(mut self, glyph: &'a str) -> Self {
        self.kind_glyph = Some(glyph);
        self
    }

    /// Secondary detail line/column.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Documentation for side preview.
    #[must_use]
    pub const fn documentation(mut self, docs: &'a str) -> Self {
        self.documentation = Some(docs);
        self
    }

    /// Group header key (consecutive equal groups collapse to one header).
    #[must_use]
    pub const fn group(mut self, group: &'a str) -> Self {
        self.group = Some(group);
        self
    }

    /// Enabled flag.
    #[must_use]
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Fuzzy / search match ranges on the label.
    #[must_use]
    pub const fn matches(mut self, ranges: &'a [crate::widgets::MatchRange]) -> Self {
        self.match_ranges = Some(ranges);
        self
    }
}

/// Slot geometry after paint (list + optional docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompletionSlots {
    /// Outer menu rect.
    pub root: Rect,
    /// Candidate list body.
    pub list: Rect,
    /// Optional documentation panel.
    pub docs: Rect,
    /// Status / footer strip (loading, stale).
    pub status: Rect,
}

impl CompletionSlots {
    /// Empty.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            root: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            list: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            docs: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            status: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

/// Semantic outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompletionMenuOutcome<Id> {
    /// Event not applicable.
    Ignored,
    /// Selected identity changed (keyboard, hover, reconcile).
    SelectionChanged,
    /// Caller should commit the given candidate id (Enter / Tab / click).
    Committed(Id),
    /// Commit via a commit-character (e.g. `(` after selection).
    CommitWithChar {
        /// Candidate id.
        id: Id,
        /// Character that triggered commit.
        ch: char,
    },
    /// Caller should dismiss (Escape / outside click).
    Dismissed,
    /// Status / generation changed (loading, stale, ready).
    StatusChanged {
        /// New status.
        status: CompletionStatus,
    },
    /// Presentation should reflow (anchored ↔ fullscreen).
    PresentationChanged {
        /// New presentation.
        presentation: CompletionPresentation,
    },
    /// Incoming async results ignored (stale generation).
    GenerationStale {
        /// Generation that was applied / rejected.
        generation: u64,
    },
}

/// Preferred popup size before clamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionMenuSize {
    /// Preferred width in cells (list; docs extra when shown).
    pub width: u16,
    /// Preferred height in rows.
    pub height: u16,
}

impl Default for CompletionMenuSize {
    fn default() -> Self {
        Self {
            width: 32,
            height: 8,
        }
    }
}

impl From<CompletionMenuSize> for OverlaySize {
    fn from(value: CompletionMenuSize) -> Self {
        Self {
            width: value.width,
            height: value.height,
            min_width: 8,
            min_height: 1,
            max_width: 0,
            max_height: 0,
        }
    }
}

// ── Intents ─────────────────────────────────────────────────────────────────

/// Default intent map for completion: list navigation + Tab/Enter activate + Esc cancel.
///
/// Does **not** map printable characters (those may be commit chars or editor typing).
#[must_use]
pub fn default_completion_intent(key: KeyEvent) -> Option<UiIntent> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let is_press = key.kind == KeyEventKind::Press;
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }
    match key.code {
        KeyCode::Tab if is_press && key.modifiers.is_empty() => Some(UiIntent::Activate),
        KeyCode::Enter if is_press => Some(UiIntent::Activate),
        KeyCode::Esc if is_press => Some(UiIntent::Cancel),
        KeyCode::Up | KeyCode::Char('k' | 'K') => Some(UiIntent::Move(NavigationMove::Previous)),
        KeyCode::Down | KeyCode::Char('j' | 'J') => Some(UiIntent::Move(NavigationMove::Next)),
        KeyCode::Home => Some(UiIntent::Move(NavigationMove::First)),
        KeyCode::End => Some(UiIntent::Move(NavigationMove::Last)),
        KeyCode::PageUp => Some(UiIntent::Page(PageMove::Backward)),
        KeyCode::PageDown => Some(UiIntent::Page(PageMove::Forward)),
        _ => None,
    }
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Compute anchored menu rect (never covers anchor; clamp/flip).
#[must_use]
pub fn place_completion_menu(bounds: Rect, anchor: Rect, preferred: CompletionMenuSize) -> Rect {
    if bounds.is_empty() || preferred.width == 0 || preferred.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        OverlaySize::from(preferred),
        OverlayPolicy::for_kind(OverlayKind::Completion),
    )
}

/// Place using presentation (fullscreen fills bounds).
#[must_use]
pub fn place_completion_with_presentation(
    bounds: Rect,
    anchor: Rect,
    preferred: CompletionMenuSize,
    presentation: CompletionPresentation,
) -> Rect {
    match presentation {
        CompletionPresentation::Fullscreen => {
            if bounds.is_empty() {
                Rect::default()
            } else {
                bounds
            }
        }
        CompletionPresentation::Anchored => place_completion_menu(bounds, anchor, preferred),
    }
}

/// Open completion overlay; auto-promotes to fullscreen on small bounds.
pub fn open_completion_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    preferred: CompletionMenuSize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    open_completion_configured(stack, bounds, anchor, preferred, opener_focus, None, None)
}

/// Full open with optional presentation / id override.
pub fn open_completion_configured<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    preferred: CompletionMenuSize,
    opener_focus: Option<FocusId>,
    force_presentation: Option<CompletionPresentation>,
    id_override: Option<String>,
) -> OverlayOutcome<FocusId> {
    let presentation = force_presentation.unwrap_or_else(|| completion_presentation_for(bounds));
    let id = OverlayId(id_override.unwrap_or_else(|| COMPLETION_OVERLAY_ID.to_string()));
    let size = OverlaySize::from(preferred);
    let spec = match presentation {
        CompletionPresentation::Fullscreen => {
            // Fullscreen kind but keep Completion policy (no focus trap).
            let policy = OverlayPolicy {
                narrow_fallback: crate::interaction::NarrowFallback::Fullscreen,
                ..OverlayPolicy::for_kind(OverlayKind::Completion)
            };
            OverlaySpec::fullscreen(id, opener_focus).with_policy(policy)
        }
        CompletionPresentation::Anchored => OverlaySpec::completion(id, anchor, size, opener_focus),
    };
    stack.open(bounds, spec)
}

/// Dismiss default completion overlay.
pub fn dismiss_completion_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(COMPLETION_OVERLAY_ID))
}

// ── State ───────────────────────────────────────────────────────────────────

/// Runtime state — **active descendant** of the editor (does not steal focus).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionMenuState<Id> {
    selected: Option<Id>,
    hovered: Option<Id>,
    offset: usize,
    viewport_height: usize,
    painted: Rect,
    hits: Vec<(Id, Rect)>,
    open: bool,
    /// Host grants routing of keys into the menu (editor still focused).
    accepts_input: bool,
    status: CompletionStatus,
    /// Async generation counter (apply only matching gen).
    generation: u64,
    /// Expected generation for in-flight request (`None` = idle).
    pending_generation: Option<u64>,
    presentation: CompletionPresentation,
    presentation_override: Option<CompletionPresentation>,
    /// Characters that commit the current selection (e.g. `().[]`).
    commit_characters: String,
    /// Show documentation side panel when selected candidate has docs.
    show_docs: bool,
    docs_scroll: u16,
    slots: CompletionSlots,
    /// Loading / empty / stale copy overrides.
    loading_message: String,
    empty_message: String,
    stale_message: String,
}

impl<Id> Default for CompletionMenuState<Id> {
    fn default() -> Self {
        Self::new(None)
    }
}

impl<Id> CompletionMenuState<Id> {
    /// State with optional initial selection.
    #[must_use]
    pub fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            hovered: None,
            offset: 0,
            viewport_height: 0,
            painted: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            hits: Vec::new(),
            open: true,
            accepts_input: true,
            status: CompletionStatus::Ready,
            generation: 0,
            pending_generation: None,
            presentation: CompletionPresentation::Anchored,
            presentation_override: None,
            commit_characters: String::new(),
            show_docs: true,
            docs_scroll: 0,
            slots: CompletionSlots::empty(),
            loading_message: LOADING_MESSAGE.into(),
            empty_message: "No matches".into(),
            stale_message: "Stale results".into(),
        }
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Open / close.
    pub fn set_open(&mut self, open: bool) {
        self.open = open;
        if !open {
            self.hovered = None;
            self.hits.clear();
            self.pending_generation = None;
        }
    }

    /// Selected id (active descendant).
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Replace selection.
    pub fn select(&mut self, selected: Option<Id>) {
        self.selected = selected;
    }

    /// Painted geometry.
    #[must_use]
    pub const fn painted(&self) -> Rect {
        self.painted
    }

    /// Scroll offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Status.
    #[must_use]
    pub const fn status(&self) -> CompletionStatus {
        self.status
    }

    /// Current generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Presentation.
    #[must_use]
    pub const fn presentation(&self) -> CompletionPresentation {
        self.presentation
    }

    /// Slots after paint.
    #[must_use]
    pub const fn slots(&self) -> CompletionSlots {
        self.slots
    }

    /// Commit characters string.
    #[must_use]
    pub fn commit_characters(&self) -> &str {
        &self.commit_characters
    }

    /// Input gate (menu routes keys while editor keeps focus).
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Force presentation.
    pub fn set_presentation_override(&mut self, p: Option<CompletionPresentation>) {
        self.presentation_override = p;
        if let Some(p) = p {
            self.presentation = p;
        }
    }

    /// Characters that auto-commit selection (empty = disabled).
    pub fn set_commit_characters(&mut self, chars: impl Into<String>) {
        self.commit_characters = chars.into();
    }

    /// Show documentation side panel when available.
    pub fn set_show_docs(&mut self, on: bool) {
        self.show_docs = on;
    }

    /// Status message overrides.
    pub fn set_status_messages(
        &mut self,
        loading: impl Into<String>,
        empty: impl Into<String>,
        stale: impl Into<String>,
    ) {
        self.loading_message = loading.into();
        self.empty_message = empty.into();
        self.stale_message = stale.into();
    }

    /// Begin async fetch; returns generation token for [`Self::apply_results`].
    pub fn begin_async(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        let g = self.generation;
        self.pending_generation = Some(g);
        self.status = CompletionStatus::Loading;
        g
    }

    /// Mark empty without candidates.
    pub fn set_status(&mut self, status: CompletionStatus) {
        self.status = status;
    }

    /// Sync presentation from bounds.
    pub fn sync_presentation(&mut self, bounds: Rect) -> CompletionMenuOutcome<Id> {
        if self.presentation_override.is_some() {
            return CompletionMenuOutcome::Ignored;
        }
        let next = completion_presentation_for(bounds);
        if next != self.presentation {
            self.presentation = next;
            CompletionMenuOutcome::PresentationChanged { presentation: next }
        } else {
            CompletionMenuOutcome::Ignored
        }
    }
}

impl<Id: Clone + PartialEq> CompletionMenuState<Id> {
    /// Apply async results only if `generation` matches pending or current.
    pub fn apply_results(
        &mut self,
        generation: u64,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        if let Some(pending) = self.pending_generation {
            if generation != pending {
                return CompletionMenuOutcome::GenerationStale { generation };
            }
        } else if generation < self.generation {
            return CompletionMenuOutcome::GenerationStale { generation };
        }
        self.generation = generation;
        self.pending_generation = None;
        self.status = if candidates.is_empty() {
            CompletionStatus::Empty
        } else {
            CompletionStatus::Ready
        };
        self.reconcile(candidates);
        CompletionMenuOutcome::StatusChanged {
            status: self.status,
        }
    }

    /// Mark results stale (newer host query supersedes without apply).
    pub fn mark_stale(&mut self) -> CompletionMenuOutcome<Id> {
        self.status = CompletionStatus::Stale;
        self.pending_generation = None;
        CompletionMenuOutcome::StatusChanged {
            status: CompletionStatus::Stale,
        }
    }

    /// Reconcile selection after candidate list rebuild.
    pub fn reconcile(&mut self, candidates: &[CompletionCandidate<'_, Id>]) {
        if let Some(selected) = self.selected.clone()
            && candidates.iter().any(|c| c.id == selected && c.enabled)
        {
            self.ensure_visible(candidates);
            return;
        }
        self.selected = candidates.iter().find(|c| c.enabled).map(|c| c.id.clone());
        self.offset = 0;
        self.ensure_visible(candidates);
        if candidates.is_empty() && matches!(self.status, CompletionStatus::Ready) {
            self.status = CompletionStatus::Empty;
        }
    }

    fn ensure_visible(&mut self, candidates: &[CompletionCandidate<'_, Id>]) {
        let Some(selected) = self.selected.as_ref() else {
            return;
        };
        let Some(index) = candidates.iter().position(|c| &c.id == selected) else {
            return;
        };
        let height = self.viewport_height.max(1);
        if index < self.offset {
            self.offset = index;
        } else if index >= self.offset.saturating_add(height) {
            self.offset = index.saturating_add(1).saturating_sub(height);
        }
        let max_offset = candidates.len().saturating_sub(height);
        if self.offset > max_offset {
            self.offset = max_offset;
        }
    }

    /// Move selection by `delta` enabled candidates.
    pub fn move_by(
        &mut self,
        candidates: &[CompletionCandidate<'_, Id>],
        delta: isize,
    ) -> CompletionMenuOutcome<Id> {
        if candidates.is_empty() || delta == 0 {
            return CompletionMenuOutcome::Ignored;
        }
        let enabled: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.enabled.then_some(i))
            .collect();
        if enabled.is_empty() {
            return CompletionMenuOutcome::Ignored;
        }
        let current = self
            .selected
            .as_ref()
            .and_then(|id| candidates.iter().position(|c| &c.id == id))
            .and_then(|idx| enabled.iter().position(|&i| i == idx))
            .unwrap_or(0);
        let len = enabled.len() as isize;
        let next = (current as isize + delta).rem_euclid(len) as usize;
        let new_id = candidates[enabled[next]].id.clone();
        if self.selected.as_ref() == Some(&new_id) {
            return CompletionMenuOutcome::Ignored;
        }
        self.selected = Some(new_id);
        self.docs_scroll = 0;
        self.ensure_visible(candidates);
        CompletionMenuOutcome::SelectionChanged
    }

    /// Commit current selection.
    pub fn commit(
        &mut self,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        let Some(id) = self.selected.clone() else {
            return CompletionMenuOutcome::Ignored;
        };
        if !candidates.iter().any(|c| c.id == id && c.enabled) {
            return CompletionMenuOutcome::Ignored;
        }
        CompletionMenuOutcome::Committed(id)
    }

    /// Keyboard via [`default_completion_intent`] + commit characters.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        if !self.open || !self.accepts_input || key.kind == KeyEventKind::Release {
            return CompletionMenuOutcome::Ignored;
        }
        // Commit characters (Press only).
        if key.kind == KeyEventKind::Press
            && key.modifiers.is_empty()
            && let KeyCode::Char(ch) = key.code
            && self.commit_characters.contains(ch)
        {
            if let Some(id) = self.selected.clone() {
                if candidates.iter().any(|c| c.id == id && c.enabled) {
                    return CompletionMenuOutcome::CommitWithChar { id, ch };
                }
            }
            return CompletionMenuOutcome::Ignored;
        }
        let Some(intent) = default_completion_intent(key) else {
            return CompletionMenuOutcome::Ignored;
        };
        if matches!(intent, UiIntent::Activate) && key.kind != KeyEventKind::Press {
            return CompletionMenuOutcome::Ignored;
        }
        self.handle_intent(candidates, intent)
    }

    /// Semantic intent routing (Tab/Enter → Activate, Esc → Cancel).
    pub fn handle_intent(
        &mut self,
        candidates: &[CompletionCandidate<'_, Id>],
        intent: UiIntent,
    ) -> CompletionMenuOutcome<Id> {
        if !self.open || !self.accepts_input {
            return CompletionMenuOutcome::Ignored;
        }
        match intent {
            UiIntent::Move(NavigationMove::Previous) => self.move_by(candidates, -1),
            UiIntent::Move(NavigationMove::Next) => self.move_by(candidates, 1),
            UiIntent::Move(NavigationMove::First) => {
                let first = candidates.iter().find(|c| c.enabled).map(|c| c.id.clone());
                if first.is_some() && first != self.selected {
                    self.selected = first;
                    self.offset = 0;
                    self.docs_scroll = 0;
                    self.ensure_visible(candidates);
                    CompletionMenuOutcome::SelectionChanged
                } else {
                    CompletionMenuOutcome::Ignored
                }
            }
            UiIntent::Move(NavigationMove::Last) => {
                let last = candidates
                    .iter()
                    .rev()
                    .find(|c| c.enabled)
                    .map(|c| c.id.clone());
                if last.is_some() && last != self.selected {
                    self.selected = last;
                    self.docs_scroll = 0;
                    self.ensure_visible(candidates);
                    CompletionMenuOutcome::SelectionChanged
                } else {
                    CompletionMenuOutcome::Ignored
                }
            }
            UiIntent::Page(PageMove::Backward) => {
                let step = isize::try_from(self.viewport_height.max(1)).unwrap_or(1);
                self.move_by(candidates, -step)
            }
            UiIntent::Page(PageMove::Forward) => {
                let step = isize::try_from(self.viewport_height.max(1)).unwrap_or(1);
                self.move_by(candidates, step)
            }
            UiIntent::Activate | UiIntent::Open | UiIntent::Submit => self.commit(candidates),
            UiIntent::Cancel | UiIntent::Close => {
                self.open = false;
                CompletionMenuOutcome::Dismissed
            }
            _ => CompletionMenuOutcome::Ignored,
        }
    }

    /// Mouse against painted geometry.
    pub fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        candidates: &[CompletionCandidate<'_, Id>],
    ) -> CompletionMenuOutcome<Id> {
        if !self.open || !self.accepts_input {
            return CompletionMenuOutcome::Ignored;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp if self.painted.contains(mouse.position) => {
                if self.slots.docs.contains(mouse.position) {
                    self.docs_scroll = self.docs_scroll.saturating_sub(1);
                    return CompletionMenuOutcome::Ignored;
                }
                self.move_by(candidates, -1)
            }
            MouseEventKind::ScrollDown if self.painted.contains(mouse.position) => {
                if self.slots.docs.contains(mouse.position) {
                    self.docs_scroll = self.docs_scroll.saturating_add(1);
                    return CompletionMenuOutcome::Ignored;
                }
                self.move_by(candidates, 1)
            }
            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(id) = self.hit_at(mouse.position)
                    && candidates.iter().any(|c| c.id == id && c.enabled)
                    && self.hovered.as_ref() != Some(&id)
                {
                    self.hovered = Some(id.clone());
                    if self.selected.as_ref() != Some(&id) {
                        self.selected = Some(id);
                        self.docs_scroll = 0;
                        self.ensure_visible(candidates);
                        return CompletionMenuOutcome::SelectionChanged;
                    }
                }
                CompletionMenuOutcome::Ignored
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(id) = self.hit_at(mouse.position)
                    && candidates.iter().any(|c| c.id == id && c.enabled)
                {
                    self.selected = Some(id.clone());
                    return CompletionMenuOutcome::Committed(id);
                }
                if !self.painted.contains(mouse.position) && !self.painted.is_empty() {
                    self.open = false;
                    return CompletionMenuOutcome::Dismissed;
                }
                CompletionMenuOutcome::Ignored
            }
            MouseEventKind::Up(MouseButton::Left) => CompletionMenuOutcome::Ignored,
            _ => CompletionMenuOutcome::Ignored,
        }
    }

    fn hit_at(&self, position: Position) -> Option<Id> {
        self.hits
            .iter()
            .find(|(_, rect)| rect.contains(position))
            .map(|(id, _)| id.clone())
    }

    /// Open on stack helper.
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        anchor: Rect,
        preferred: CompletionMenuSize,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        self.open = true;
        let _ = self.sync_presentation(bounds);
        open_completion_configured(
            stack,
            bounds,
            anchor,
            preferred,
            opener_focus,
            self.presentation_override.or(Some(self.presentation)),
            None,
        )
    }

    /// Close on stack.
    pub fn close_on_stack<F: Clone>(&mut self, stack: &mut OverlayStack<F>) -> OverlayOutcome<F> {
        self.set_open(false);
        dismiss_completion_overlay(stack)
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Popup completion list (active descendant of editor).
pub struct CompletionMenu<'a, Id> {
    candidates: &'a [CompletionCandidate<'a, Id>],
    system: &'a DesignSystem,
    empty_message: &'a str,
    bounds: Rect,
    anchor: Rect,
    preferred: CompletionMenuSize,
    ascii: bool,
    colorless: bool,
    focused: bool,
    /// When set, paint into this rect instead of re-placing (stack geometry).
    force_area: Option<Rect>,
}

impl<'a, Id> CompletionMenu<'a, Id> {
    /// Menu over borrowed candidates.
    #[must_use]
    pub const fn new(
        candidates: &'a [CompletionCandidate<'a, Id>],
        system: &'a DesignSystem,
        bounds: Rect,
        anchor: Rect,
    ) -> Self {
        Self {
            candidates,
            system,
            empty_message: "No matches",
            focused: false,
            bounds,
            anchor,
            preferred: CompletionMenuSize {
                width: 32,
                height: 8,
            },
            ascii: false,
            colorless: false,
            force_area: None,
        }
    }

    /// Preferred size before clamp.
    #[must_use]
    pub const fn preferred_size(mut self, size: CompletionMenuSize) -> Self {
        self.preferred = size;
        self
    }

    /// Empty-list cue (Ready empty).
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Whether the menu itself owns focus.
    ///
    /// Defaults to `false`: a completion menu floats under an editor that
    /// keeps the keyboard, and only the interaction owner wears the focused
    /// border.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Reduced-color roles.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint into a pre-placed rect (from OverlayStack).
    #[must_use]
    pub const fn force_area(mut self, area: Rect) -> Self {
        self.force_area = Some(area);
        self
    }

    /// Paint entry (also used by StatefulWidget).
    pub fn paint(&self, _area: Rect, buffer: &mut Buffer, state: &mut CompletionMenuState<Id>)
    where
        Id: Clone + PartialEq,
    {
        state.hits.clear();
        if !state.open {
            state.painted = Rect::default();
            state.slots = CompletionSlots::empty();
            return;
        }

        let presentation = state
            .presentation_override
            .unwrap_or_else(|| completion_presentation_for(self.bounds));
        state.presentation = presentation;

        let mut preferred = self.preferred;
        // Docs panel expands preferred width when enabled and selection has docs.
        let docs_w = if state.show_docs
            && self
                .candidates
                .iter()
                .any(|c| c.documentation.is_some() && state.selected.as_ref() == Some(&c.id))
        {
            COMPLETION_DOCS_DEFAULT_WIDTH
        } else {
            0
        };

        if self.candidates.is_empty()
            || matches!(
                state.status,
                CompletionStatus::Loading | CompletionStatus::Empty
            )
        {
            preferred.height = preferred.height.min(3).max(1);
        } else {
            preferred.height = preferred.height.min(
                u16::try_from(self.candidates.len())
                    .unwrap_or(u16::MAX)
                    .max(1),
            );
        }

        let content_width = self
            .candidates
            .iter()
            .map(|c| {
                let kind = c.kind.map(display_cols).unwrap_or(0);
                let glyph = c.kind_glyph.map(display_cols).unwrap_or(0);
                let detail = c.detail.map(display_cols).unwrap_or(0);
                display_cols(c.label)
                    .saturating_add(if kind == 0 { 0 } else { kind + 2 })
                    .saturating_add(if glyph == 0 { 0 } else { glyph + 1 })
                    .saturating_add(if detail == 0 { 0 } else { detail + 2 })
            })
            .max()
            .unwrap_or(display_cols(self.empty_message))
            .saturating_add(2);
        preferred.width = preferred
            .width
            .max(
                u16::try_from(content_width)
                    .unwrap_or(u16::MAX)
                    .min(preferred.width.max(12)),
            )
            .saturating_add(docs_w);

        let menu = if let Some(forced) = self.force_area {
            forced
        } else {
            place_completion_with_presentation(self.bounds, self.anchor, preferred, presentation)
        };
        state.painted = menu;
        state.slots.root = menu;
        if menu.is_empty() {
            state.slots = CompletionSlots::empty();
            return;
        }

        // Split list | docs
        let (list_area, docs_area) = if docs_w > 0 && menu.width > docs_w.saturating_add(10) {
            let dw = docs_w.min(menu.width / 2);
            let list_w = menu.width.saturating_sub(dw);
            (
                Rect::new(menu.x, menu.y, list_w, menu.height),
                Rect::new(menu.x.saturating_add(list_w), menu.y, dw, menu.height),
            )
        } else {
            (menu, Rect::default())
        };
        state.slots.list = list_area;
        state.slots.docs = docs_area;

        // Status row: last line when loading/stale
        let status_h = if matches!(
            state.status,
            CompletionStatus::Loading | CompletionStatus::Stale
        ) && list_area.height > 1
        {
            1
        } else {
            0
        };
        let list_body = Rect {
            x: list_area.x,
            y: list_area.y,
            width: list_area.width,
            height: list_area.height.saturating_sub(status_h),
        };
        state.slots.status = if status_h > 0 {
            Rect::new(
                list_area.x,
                list_area.bottom().saturating_sub(1),
                list_area.width,
                1,
            )
        } else {
            Rect::default()
        };

        state.viewport_height = usize::from(list_body.height.max(1));
        state.reconcile(self.candidates);

        // The menu declares itself non-focusable — the editor keeps focus — so
        // it must not wear the focused border. A host that gives the menu its
        // own focus says so with `focused(true)` (plans/009 Step 4).
        let recipe = if self.focused {
            super::SurfaceRecipe::OverlayFocused
        } else {
            super::SurfaceRecipe::Overlay
        };
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
        super::Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .padding(0, 0)
            .paint(menu, buffer);

        // Loading / empty full-body messages
        if matches!(state.status, CompletionStatus::Loading) && self.candidates.is_empty() {
            let msg = loading_copy(self.ascii, state);
            paint_centered_msg(buffer, list_body, msg, self.system.style(Role::TextMuted));
            paint_status_line(self, buffer, state);
            return;
        }
        if self.candidates.is_empty() || matches!(state.status, CompletionStatus::Empty) {
            let msg = if state.empty_message.is_empty() {
                self.empty_message
            } else {
                state.empty_message.as_str()
            };
            let text = take_display_cols(msg, usize::from(list_body.width.saturating_sub(2)));
            buffer.set_stringn(
                list_body.x.saturating_add(1),
                list_body.y,
                text,
                usize::from(list_body.width.saturating_sub(2)),
                self.system.style(Role::TextMuted),
            );
            paint_status_line(self, buffer, state);
            return;
        }

        // Group-aware paint: reserve header rows by walking from offset.
        let max_offset = self
            .candidates
            .len()
            .saturating_sub(usize::from(list_body.height.max(1)));
        if state.offset > max_offset {
            state.offset = max_offset;
        }

        let mut y = list_body.y;
        let mut i = state.offset;
        let mut last_group: Option<&str> = if state.offset > 0 {
            self.candidates
                .get(state.offset.saturating_sub(1))
                .and_then(|c| c.group)
        } else {
            None
        };

        while y < list_body.bottom() && i < self.candidates.len() {
            let candidate = &self.candidates[i];
            // Group header when group changes
            if let Some(g) = candidate.group {
                if last_group != Some(g) {
                    let header =
                        take_display_cols(g, usize::from(list_body.width.saturating_sub(2)));
                    buffer.set_stringn(
                        list_body.x.saturating_add(1),
                        y,
                        header,
                        usize::from(list_body.width.saturating_sub(2)),
                        self.system.style(Role::TextMuted),
                    );
                    y = y.saturating_add(1);
                    last_group = Some(g);
                    if y >= list_body.bottom() {
                        break;
                    }
                }
            } else {
                last_group = None;
            }

            let row_rect = Rect::new(list_body.x, y, list_body.width, 1);
            state.hits.push((candidate.id.clone(), row_rect));

            let selected = state.selected.as_ref() == Some(&candidate.id);
            let hovered = state.hovered.as_ref() == Some(&candidate.id);
            let style = row_style(
                self.system,
                candidate.enabled,
                selected,
                hovered,
                self.colorless,
            );

            let mut x = list_body.x.saturating_add(1);
            let right = list_body.right().saturating_sub(1);

            // Selection gutter
            if selected {
                let mark = self.system.glyphs.selection_gutter();
                if let Some(cell) = buffer.cell_mut((list_body.x, y)) {
                    cell.set_symbol(mark);
                    cell.set_style(style);
                }
            }

            // Kind glyph
            if let Some(glyph) = candidate.kind_glyph {
                let g = take_display_cols(glyph, 2);
                let gw = display_cols(&g) as u16;
                buffer.set_stringn(x, y, &g, 2, self.system.style(Role::TextMuted));
                x = x.saturating_add(gw.saturating_add(1));
            }

            // Kind + detail budgets from right
            let kind_cols = candidate.kind.map(display_cols).unwrap_or(0);
            let detail_cols = candidate.detail.map(display_cols).unwrap_or(0);
            let right_budget = kind_cols
                .saturating_add(if kind_cols == 0 { 0 } else { 1 })
                .saturating_add(detail_cols)
                .saturating_add(if detail_cols == 0 { 0 } else { 1 });
            let label_budget = usize::from(right.saturating_sub(x)).saturating_sub(right_budget);
            let label_area = Rect::new(x, y, u16::try_from(label_budget).unwrap_or(0), 1);

            if let Some(ranges) = candidate.match_ranges {
                use crate::widgets::{HighlightVisual, HighlightedText, MatchTruncate};
                let visual = if selected {
                    HighlightVisual::Selected
                } else if !candidate.enabled {
                    HighlightVisual::Inactive
                } else {
                    HighlightVisual::Normal
                };
                let _ = HighlightedText::new(candidate.label, ranges, self.system)
                    .visual(visual)
                    .truncate(MatchTruncate::KeepFirstMatch)
                    .paint(label_area, buffer);
            } else {
                let label = take_display_cols(candidate.label, label_budget);
                buffer.set_stringn(x, y, label, label_budget, style);
            }

            // Detail then kind from right
            let mut rx = right;
            if let Some(kind) = candidate.kind {
                let kind_text = take_display_cols(kind, kind_cols.min(12));
                let kw = display_cols(&kind_text) as u16;
                rx = rx.saturating_sub(kw);
                buffer.set_stringn(
                    rx,
                    y,
                    &kind_text,
                    usize::from(kw),
                    if candidate.enabled {
                        self.system.style(Role::TextMuted)
                    } else {
                        style
                    },
                );
                rx = rx.saturating_sub(1);
            }
            if let Some(detail) = candidate.detail {
                let dtext = take_display_cols(detail, detail_cols.min(16));
                let dw = display_cols(&dtext) as u16;
                rx = rx.saturating_sub(dw);
                if rx > x {
                    buffer.set_stringn(
                        rx,
                        y,
                        &dtext,
                        usize::from(dw),
                        self.system.style(Role::TextMuted),
                    );
                }
            }

            y = y.saturating_add(1);
            i = i.saturating_add(1);
        }

        // Docs panel
        if !docs_area.is_empty() {
            if docs_area.width >= 1 {
                let sep = if self.ascii { "|" } else { "│" };
                for yy in docs_area.y..docs_area.bottom() {
                    buffer.set_stringn(docs_area.x, yy, sep, 1, self.system.style(Role::Border));
                }
            }
            let docs_inner = Rect {
                x: docs_area.x.saturating_add(1),
                y: docs_area.y,
                width: docs_area.width.saturating_sub(1),
                height: docs_area.height,
            };
            if let Some(sel) = state.selected.as_ref() {
                if let Some(c) = self.candidates.iter().find(|c| &c.id == sel) {
                    if let Some(docs) = c.documentation {
                        paint_docs(buffer, docs_inner, docs, state.docs_scroll, self.system);
                    }
                }
            }
        }

        // The right margin the rows already reserve doubles as the scroll
        // gutter: a menu that scrolls says so (plans/022 Step 2).
        crate::scroll::paint_scrolled_region(
            buffer,
            list_body,
            Rect::new(
                list_body.right().saturating_sub(1),
                list_body.y,
                1,
                list_body.height,
            ),
            self.candidates.len(),
            state.viewport_height,
            u16::try_from(state.offset).unwrap_or(u16::MAX),
            self.system,
        );

        paint_status_line(self, buffer, state);
    }

    /// Semantic registration (menu is not a focus trap — active descendant).
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &CompletionMenuState<Id>,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
        Id: std::fmt::Display,
    {
        if area.is_empty() || !state.open {
            return;
        }
        let sel = state
            .selected
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let desc = format!(
            "completion status={} presentation={} activedescendant={} gen={}",
            state.status.id(),
            state.presentation.id(),
            sel,
            state.generation,
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Menu)
                .label("completion-menu")
                .description(desc)
                .focusable(false) // editor keeps focus
                .state(SemanticState {
                    selected: state.selected.is_some(),
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &CompletionMenu<'_, Id> {
    type State = CompletionMenuState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for CompletionMenu<'_, Id> {
    type State = CompletionMenuState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

/// Loading copy for the active glyph profile.
///
/// A host-supplied message is painted as written; only the default carries an
/// ASCII twin, so overriding the copy never loses it on a degraded terminal.
fn loading_copy<'a, Id>(ascii: bool, state: &'a CompletionMenuState<Id>) -> &'a str {
    if ascii && state.loading_message == LOADING_MESSAGE {
        LOADING_MESSAGE_ASCII
    } else {
        state.loading_message.as_str()
    }
}

fn paint_status_line<Id>(
    menu: &CompletionMenu<'_, Id>,
    buffer: &mut Buffer,
    state: &CompletionMenuState<Id>,
) {
    let strip = state.slots.status;
    if strip.is_empty() {
        return;
    }
    let msg = match state.status {
        CompletionStatus::Loading => loading_copy(menu.ascii, state),
        CompletionStatus::Stale => state.stale_message.as_str(),
        _ => return,
    };
    buffer.set_stringn(
        strip.x.saturating_add(1),
        strip.y,
        &take_display_cols(msg, usize::from(strip.width.saturating_sub(2))),
        usize::from(strip.width.saturating_sub(2)),
        menu.system.style(Role::TextMuted),
    );
}

fn paint_centered_msg(buffer: &mut Buffer, area: Rect, msg: &str, style: Style) {
    if area.is_empty() {
        return;
    }
    let text = take_display_cols(msg, usize::from(area.width.saturating_sub(2)));
    buffer.set_stringn(
        area.x.saturating_add(1),
        area.y,
        text,
        usize::from(area.width.saturating_sub(2)),
        style,
    );
}

fn paint_docs(buffer: &mut Buffer, area: Rect, docs: &str, scroll: u16, system: &DesignSystem) {
    if area.is_empty() {
        return;
    }
    let width = usize::from(area.width.max(1));
    let lines: Vec<&str> = docs.lines().collect();
    let start = usize::from(scroll).min(lines.len().saturating_sub(1));
    let mut y = area.y;
    for line in lines.iter().skip(start) {
        if y >= area.bottom() {
            break;
        }
        // naive wrap: single line take
        buffer.set_stringn(
            area.x,
            y,
            &take_display_cols(line, width),
            width,
            system.style(Role::Text),
        );
        y = y.saturating_add(1);
    }
}

fn row_style(
    system: &DesignSystem,
    enabled: bool,
    selected: bool,
    hovered: bool,
    colorless: bool,
) -> Style {
    if colorless {
        if !enabled {
            system.style(Role::TextMuted)
        } else if selected {
            system.style(Role::TextStrong)
        } else {
            system.style(Role::Text)
        }
    } else if !enabled {
        system.style(Role::TextDisabled)
    } else if selected {
        system
            .style(Role::TextStrong)
            .patch(system.style(Role::SelectionTint))
    } else if hovered {
        system.style(Role::TextStrong)
    } else {
        system.style(Role::Text)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn rect_intersects(a: Rect, b: Rect) -> bool {
        let a_x2 = a.x.saturating_add(a.width);
        let a_y2 = a.y.saturating_add(a.height);
        let b_x2 = b.x.saturating_add(b.width);
        let b_y2 = b.y.saturating_add(b.height);
        a.x < b_x2 && b.x < a_x2 && a.y < b_y2 && b.y < a_y2
    }

    fn candidates(ids: &[&'static str]) -> Vec<CompletionCandidate<'static, &'static str>> {
        ids.iter()
            .map(|id| CompletionCandidate::new(*id, id))
            .collect()
    }

    #[test]
    fn open_on_overlay_stack_and_dismiss() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 1, 1);
        let mut stack = OverlayStack::<&'static str>::new();
        let out = open_completion_overlay(
            &mut stack,
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 6,
            },
            Some("editor"),
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Completion);
        let rect = stack.top().unwrap().rect;
        assert!(!rect_intersects(rect, anchor));
        assert!(matches!(
            dismiss_completion_overlay(&mut stack),
            OverlayOutcome::Dismissed {
                focus: Some("editor"),
                ..
            }
        ));
    }

    #[test]
    fn place_prefers_below_anchor_without_covering() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 5, 1, 1);
        let menu = place_completion_menu(
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 6,
            },
        );
        assert_eq!(menu.y, 6, "below anchor");
        assert!(!rect_intersects(menu, anchor));
        assert!(menu.x >= bounds.x);
        assert!(menu.x + menu.width <= bounds.x + bounds.width);
    }

    #[test]
    fn place_flips_above_when_bottom_edge() {
        let bounds = Rect::new(0, 0, 80, 20);
        let anchor = Rect::new(10, 18, 1, 1);
        let menu = place_completion_menu(
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 6,
            },
        );
        assert!(menu.y + menu.height <= anchor.y, "above anchor: {menu:?}");
        assert!(!rect_intersects(menu, anchor));
    }

    #[test]
    fn place_clamps_right_edge() {
        let bounds = Rect::new(0, 0, 40, 20);
        let anchor = Rect::new(35, 2, 1, 1);
        let menu = place_completion_menu(
            bounds,
            anchor,
            CompletionMenuSize {
                width: 20,
                height: 4,
            },
        );
        assert!(menu.x + menu.width <= 40);
        assert!(!rect_intersects(menu, anchor));
    }

    #[test]
    fn presentation_fullscreen_on_narrow() {
        assert_eq!(
            completion_presentation_for(Rect::new(0, 0, 20, 24)),
            CompletionPresentation::Fullscreen
        );
        assert_eq!(
            completion_presentation_for(Rect::new(0, 0, 80, 24)),
            CompletionPresentation::Anchored
        );
        let placed = place_completion_with_presentation(
            Rect::new(0, 0, 20, 12),
            Rect::new(2, 2, 1, 1),
            CompletionMenuSize::default(),
            CompletionPresentation::Fullscreen,
        );
        assert_eq!(placed, Rect::new(0, 0, 20, 12));
    }

    #[test]
    fn keyboard_tab_enter_esc_via_intents() {
        let items = candidates(&["alpha", "beta", "gamma"]);
        let mut state = CompletionMenuState::new(Some("alpha"));
        state.viewport_height = 3;
        assert_eq!(
            default_completion_intent(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Some(UiIntent::Activate)
        );
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &items),
            CompletionMenuOutcome::SelectionChanged
        );
        assert_eq!(state.selected().copied(), Some("beta"));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &items),
            CompletionMenuOutcome::Committed("beta")
        );
        state.set_open(true);
        state.select(Some("gamma"));
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            CompletionMenuOutcome::Committed("gamma")
        );
        state.set_open(true);
        assert_eq!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &items),
            CompletionMenuOutcome::Dismissed
        );
        assert!(!state.is_open());
    }

    #[test]
    fn commit_characters() {
        let items = candidates(&["foo", "bar"]);
        let mut state = CompletionMenuState::new(Some("foo"));
        state.set_commit_characters("().");
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('('), KeyModifiers::NONE),
                &items
            ),
            CompletionMenuOutcome::CommitWithChar { id: "foo", ch: '(' }
        ));
    }

    #[test]
    fn async_generation_gate() {
        let mut state = CompletionMenuState::<&str>::new(None);
        let g1 = state.begin_async();
        assert_eq!(state.status(), CompletionStatus::Loading);
        let g2 = state.begin_async();
        assert_ne!(g1, g2);
        // stale apply
        assert!(matches!(
            state.apply_results(g1, &candidates(&["a"])),
            CompletionMenuOutcome::GenerationStale { generation } if generation == g1
        ));
        assert!(matches!(
            state.apply_results(g2, &candidates(&["a", "b"])),
            CompletionMenuOutcome::StatusChanged {
                status: CompletionStatus::Ready
            }
        ));
        assert_eq!(state.selected().copied(), Some("a"));
        let _ = state.mark_stale();
        assert_eq!(state.status(), CompletionStatus::Stale);
    }

    #[test]
    fn reconcile_keeps_id_then_falls_back() {
        let mut state = CompletionMenuState::new(Some("beta"));
        state.reconcile(&candidates(&["alpha", "beta", "gamma"]));
        assert_eq!(state.selected().copied(), Some("beta"));
        state.reconcile(&candidates(&["alpha", "gamma"]));
        assert_eq!(state.selected().copied(), Some("alpha"));
        state.reconcile(&[]);
        assert_eq!(state.selected().copied(), None);
    }

    #[test]
    fn mouse_click_commits_selected_hit() {
        let items = candidates(&["one", "two"]);
        let mut state = CompletionMenuState::new(Some("one"));
        state.open = true;
        state.painted = Rect::new(0, 0, 20, 2);
        state.hits = vec![
            ("one", Rect::new(0, 0, 20, 1)),
            ("two", Rect::new(0, 1, 20, 1)),
        ];
        let event = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: 2, y: 1 },
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(
            state.handle_mouse(event, &items),
            CompletionMenuOutcome::Committed("two")
        );
    }

    #[test]
    fn paint_groups_glyphs_docs() {
        let system = DesignSystem::default();
        let items = [
            CompletionCandidate::new("s", "SELECT")
                .kind("kw")
                .kind_glyph("⌘")
                .group("Keywords")
                .documentation("Select rows from a table."),
            CompletionCandidate::new("f", "FROM")
                .kind("kw")
                .group("Keywords"),
            CompletionCandidate::new("u", "users")
                .kind("table")
                .detail("public")
                .group("Tables"),
        ];
        let mut state = CompletionMenuState::new(Some("s"));
        state.set_show_docs(true);
        let area = Rect::new(0, 0, 60, 12);
        let anchor = Rect::new(2, 1, 1, 1);
        let mut buf = Buffer::empty(area);
        CompletionMenu::new(&items, &system, area, anchor)
            .preferred_size(CompletionMenuSize {
                width: 40,
                height: 8,
            })
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("SELECT") || text.contains("Keywords"),
            "{text}"
        );
        assert!(!state.slots().list.is_empty());
    }

    #[test]
    fn active_descendant_not_focus_trap() {
        let policy = OverlayPolicy::for_kind(OverlayKind::Completion);
        assert!(!policy.focus_trap);
        assert!(policy.owns_input); // can route keys when host grants
        let system = DesignSystem::default();
        let items = candidates(&["a"]);
        let mut state = CompletionMenuState::new(Some("a"));
        let mut scene = SemanticScene::<&str, ()>::default();
        CompletionMenu::new(
            &items,
            &system,
            Rect::new(0, 0, 40, 10),
            Rect::new(1, 1, 1, 1),
        )
        .register_semantic(&mut scene, "cm", Rect::new(0, 0, 20, 5), &state);
        let node = scene
            .nodes()
            .iter()
            .find(|n| n.label.as_deref() == Some("completion-menu"));
        assert!(node.is_some());
        assert!(!node.unwrap().focusable);
    }

    #[test]
    fn fuzz_keys() {
        let items = candidates(&["a", "b", "c", "d", "e"]);
        let mut state = CompletionMenuState::new(Some("a"));
        state.set_commit_characters(".");
        state.viewport_height = 3;
        let keys = [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Tab,
            KeyCode::Enter,
            KeyCode::Esc,
            KeyCode::Char('.'),
            KeyCode::PageDown,
            KeyCode::Home,
        ];
        let mut seed = 11u64;
        for _ in 0..200 {
            if !state.is_open() {
                state.set_open(true);
                state.select(Some("a"));
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE), &items);
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let labels: Vec<String> = (0..40).map(|i| format!("item-{i}")).collect();
        let items: Vec<CompletionCandidate<'_, usize>> = labels
            .iter()
            .enumerate()
            .map(|(i, l)| CompletionCandidate::new(i, l.as_str()).kind("fn"))
            .collect();
        let mut state = CompletionMenuState::new(Some(0));
        let mut terminal = Terminal::new(TestBackend::new(48, 16)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..150 {
            terminal
                .draw(|f| {
                    CompletionMenu::new(&items, &system, f.area(), Rect::new(2, 2, 1, 1)).paint(
                        f.area(),
                        f.buffer_mut(),
                        &mut state,
                    );
                })
                .unwrap();
        }
        assert!(start.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let items = [
            CompletionCandidate::new("a", "alpha").kind("kw"),
            CompletionCandidate::new("b", "beta").kind("fn"),
        ];
        let mut s1 = CompletionMenuState::new(Some("a"));
        let mut t1 = Terminal::new(TestBackend::new(32, 8)).unwrap();
        t1.draw(|f| {
            CompletionMenu::new(&items, &system, f.area(), Rect::new(1, 1, 1, 1)).paint(
                f.area(),
                f.buffer_mut(),
                &mut s1,
            );
        })
        .unwrap();
        let text1: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut s2 = CompletionMenuState::new(Some("a"));
        let mut t2 = Terminal::new(TestBackend::new(32, 8)).unwrap();
        t2.draw(|f| {
            CompletionMenu::new(&items, &system, f.area(), Rect::new(1, 1, 1, 1)).paint(
                f.area(),
                f.buffer_mut(),
                &mut s2,
            );
        })
        .unwrap();
        let text2: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(text1, text2);
        assert!(text1.contains("alpha"));
    }

    #[test]
    fn fullscreen_open_on_stack() {
        let bounds = Rect::new(0, 0, 24, 8);
        let anchor = Rect::new(2, 2, 1, 1);
        let mut stack = OverlayStack::<()>::new();
        let out = open_completion_configured(
            &mut stack,
            bounds,
            anchor,
            CompletionMenuSize::default(),
            None,
            Some(CompletionPresentation::Fullscreen),
            None,
        );
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Fullscreen);
        assert!(!stack.top().unwrap().policy.focus_trap);
    }
}
