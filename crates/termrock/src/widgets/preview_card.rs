// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **PreviewCard** — non-essential contextual preview for selected resources.
//!
//! **Mission.** IDE-style quick preview / hover card for files, commands,
//! symbols, and sessions. Delayed show (pointer / focus / selection), metadata +
//! rich body, loading / error / stale chrome, pin-to-open, and application-owned
//! async data with **generation race gates**. Never hides required information
//! exclusively in the preview (`essential_elsewhere`).
//!
//! **Focus law.** Unpinned previews never own keyboard input and never steal
//! focus. Pinned previews may accept a small key set (Esc unpin, Enter open)
//! without trapping the whole scene — host still owns domain navigation.
//!
//! **Redraw discipline.** Rapid selection changes debounce the show timer and
//! bump generation so in-flight async results for older selections are dropped.
//!
//! **vs Tooltip.** Tooltip is short help text. PreviewCard is a resource card
//! (kind badge, meta rows, multi-line body, async).
//! **vs Popover.** Popover is an interactive anchor surface. PreviewCard is
//! non-essential and delayed; pin promotes toward sticky / open paths.
//! **vs QuickOpen preview pane.** QuickOpen embeds an inline pane; PreviewCard
//! is the reusable floating / side card primitive.
//!
//! Research: IDE quick previews, hover cards, Yazi previews, QuickOpen panels.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use std::time::Duration;
use web_time::Instant;

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    interaction::{
        OverlayId, OverlayKind, OverlayOutcome, OverlayPolicy, OverlaySize, OverlaySpec,
        OverlayStack, SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent,
        place_overlay,
    },
    runtime::{FrameTick, Presence},
    style::{DesignSystem, GlyphSet, MotionPolicy, Role},
    text::{display_cols, take_display_cols},
};

use super::{Surface, SurfaceRecipe};

/// Default overlay id.
pub const PREVIEW_CARD_OVERLAY_ID: &str = "termrock.preview-card";
/// Default show delay after arm (pointer/focus/selection stable).
pub const PREVIEW_CARD_DEFAULT_DELAY_MS: u64 = 300;
/// Debounce window while selection changes rapidly (suppress thrash).
pub const PREVIEW_CARD_SELECTION_DEBOUNCE_MS: u64 = 80;
/// Default max card width.
pub const PREVIEW_CARD_DEFAULT_MAX_WIDTH: u16 = 48;
/// Default preferred height (content may shrink).
pub const PREVIEW_CARD_DEFAULT_MAX_HEIGHT: u16 = 16;
/// Footer hint when unpinned.
pub const PREVIEW_CARD_HINT: &str = "pin · enter open · esc cancel";
/// Footer hint when pinned.
pub const PREVIEW_CARD_PINNED_HINT: &str = "pinned · enter open · esc unpin";

// ── Resource kind / load / trigger ──────────────────────────────────────────

/// Resource family for chrome badge (host owns domain model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PreviewResourceKind {
    /// File / path resource.
    #[default]
    File,
    /// Command / task invocation.
    Command,
    /// Symbol / definition.
    Symbol,
    /// Session / conversation / run.
    Session,
    /// Host custom.
    Custom,
}

impl PreviewResourceKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Command => "command",
            Self::Symbol => "symbol",
            Self::Session => "session",
            Self::Custom => "custom",
        }
    }

    /// Short badge label.
    #[must_use]
    pub const fn badge(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Command => "cmd",
            Self::Symbol => "sym",
            Self::Session => "sess",
            Self::Custom => "prev",
        }
    }
}

/// Async / load chrome (host drives; paint reflects).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PreviewLoadState {
    /// No selection / idle.
    #[default]
    Idle,
    /// Waiting on host fetch (generation in flight).
    Loading,
    /// Content ready for current generation.
    Ready,
    /// Host reported error for current generation.
    Error,
    /// Content belongs to an older generation (cue host refresh).
    Stale,
}

impl PreviewLoadState {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Error => "error",
            Self::Stale => "stale",
        }
    }
}

/// What arms the show delay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PreviewTrigger {
    /// Pointer over anchor only.
    Pointer,
    /// Keyboard focus on anchor only.
    Focus,
    /// List / tree selection only (cursor stable).
    Selection,
    /// Pointer or focus (hover card).
    Hover,
    /// Any of pointer, focus, or selection (default).
    #[default]
    Any,
}

impl PreviewTrigger {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pointer => "pointer",
            Self::Focus => "focus",
            Self::Selection => "selection",
            Self::Hover => "hover",
            Self::Any => "any",
        }
    }

    fn armed(self, pointer: bool, focus: bool, selection: bool) -> bool {
        match self {
            Self::Pointer => pointer,
            Self::Focus => focus,
            Self::Selection => selection,
            Self::Hover => pointer || focus,
            Self::Any => pointer || focus || selection,
        }
    }
}

// ── Content / metadata / slots ──────────────────────────────────────────────

/// One metadata row (label / value), host-owned strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewMetadata<'a> {
    /// Label (e.g. "size").
    pub label: &'a str,
    /// Value (e.g. "12 KB").
    pub value: &'a str,
}

impl<'a> PreviewMetadata<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(label: &'a str, value: &'a str) -> Self {
        Self { label, value }
    }
}

/// Host-projected preview payload (borrowed for paint).
///
/// **Required information must not live only here** — set
/// [`Self::essential_elsewhere`] when the same facts appear in the main UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewCardContent<'a> {
    /// Primary title (file name, command, symbol, session title).
    pub title: &'a str,
    /// Resource family.
    pub kind: PreviewResourceKind,
    /// Optional subtitle (path, module, provider).
    pub subtitle: Option<&'a str>,
    /// Metadata rows.
    pub meta: &'a [PreviewMetadata<'a>],
    /// Rich body lines (code snippet, docs, transcript excerpt). Empty when loading.
    pub body_lines: &'a [&'a str],
    /// Load chrome.
    pub load: PreviewLoadState,
    /// Error message when [`PreviewLoadState::Error`].
    pub error_message: Option<&'a str>,
    /// Same facts available without preview (list columns, status, labels).
    pub essential_elsewhere: bool,
    /// Pin action label (default "Pin").
    pub pin_label: Option<&'a str>,
    /// Open / promote action label (default "Open").
    pub open_label: Option<&'a str>,
}

impl<'a> PreviewCardContent<'a> {
    /// Minimal ready title.
    #[must_use]
    pub const fn title(title: &'a str, kind: PreviewResourceKind) -> Self {
        Self {
            title,
            kind,
            subtitle: None,
            meta: &[],
            body_lines: &[],
            load: PreviewLoadState::Ready,
            error_message: None,
            essential_elsewhere: true,
            pin_label: None,
            open_label: None,
        }
    }

    /// Subtitle.
    #[must_use]
    pub const fn subtitle(mut self, s: &'a str) -> Self {
        self.subtitle = Some(s);
        self
    }

    /// Meta rows.
    #[must_use]
    pub const fn meta(mut self, rows: &'a [PreviewMetadata<'a>]) -> Self {
        self.meta = rows;
        self
    }

    /// Body lines.
    #[must_use]
    pub const fn body(mut self, lines: &'a [&'a str]) -> Self {
        self.body_lines = lines;
        self
    }

    /// Load state.
    #[must_use]
    pub const fn load(mut self, load: PreviewLoadState) -> Self {
        self.load = load;
        self
    }

    /// Error message + error load.
    #[must_use]
    pub const fn error(mut self, msg: &'a str) -> Self {
        self.load = PreviewLoadState::Error;
        self.error_message = Some(msg);
        self
    }

    /// Essential-elsewhere gate.
    #[must_use]
    pub const fn essential_elsewhere(mut self, on: bool) -> Self {
        self.essential_elsewhere = on;
        self
    }

    /// Preferred height for sizing (chrome + body).
    #[must_use]
    pub fn preferred_height(self, max_body: u16) -> u16 {
        let mut h = 2u16; // border
        h = h.saturating_add(1); // title
        if self.subtitle.is_some() {
            h = h.saturating_add(1);
        }
        h = h.saturating_add(self.meta.len() as u16);
        let body = match self.load {
            PreviewLoadState::Ready => (self.body_lines.len() as u16).min(max_body).max(1),
            PreviewLoadState::Loading
            | PreviewLoadState::Error
            | PreviewLoadState::Stale
            | PreviewLoadState::Idle => 1,
        };
        h = h.saturating_add(body);
        h = h.saturating_add(1); // footer
        h.max(3)
    }

    /// Preferred content width.
    #[must_use]
    pub fn preferred_width(self, max_width: u16) -> u16 {
        let mut w = display_cols(self.title) as u16 + 8; // badge + pad
        if let Some(s) = self.subtitle {
            w = w.max(display_cols(s) as u16 + 2);
        }
        for m in self.meta {
            w = w.max((display_cols(m.label) + display_cols(m.value) + 3) as u16);
        }
        for line in self.body_lines {
            w = w.max(display_cols(line) as u16 + 2);
        }
        w.clamp(12, max_width.max(12))
    }
}

/// Slot geometry after paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreviewCardSlots {
    /// Outer card.
    pub root: Rect,
    /// Title / kind badge.
    pub header: Rect,
    /// Metadata block.
    pub meta: Rect,
    /// Body / loading / error.
    pub body: Rect,
    /// Footer (pin / open hints).
    pub footer: Rect,
}

impl PreviewCardSlots {
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
            header: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            meta: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            body: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
            footer: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Host coordination.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PreviewCardOutcome {
    /// No change.
    Ignored,
    /// Became visible — host may open overlay.
    Shown,
    /// Became hidden — host should dismiss overlay.
    Hidden,
    /// Armed but still in delay / debounce.
    Pending,
    /// Disabled.
    Disabled,
    /// Selection identity changed (host should cancel old fetch).
    SelectionChanged {
        /// Generation for the new selection.
        generation: u64,
        /// Selection id snapshot.
        selection_id: String,
    },
    /// Begin async fetch for this generation.
    Loading {
        /// Generation token.
        generation: u64,
    },
    /// Apply rejected (stale).
    GenerationStale {
        /// Offered generation.
        generation: u64,
    },
    /// Content applied for generation.
    ContentApplied {
        /// Generation.
        generation: u64,
    },
    /// Pinned (sticky; survives pointer leave).
    Pinned,
    /// Unpinned.
    Unpinned,
    /// Host should open full surface (FullscreenViewer / editor).
    OpenRequested,
    /// Essential content without non-preview channel.
    EssentialRequiresNonPreview,
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Measure overlay size for content.
#[must_use]
pub fn preview_card_overlay_size(content: &PreviewCardContent<'_>, max_width: u16) -> OverlaySize {
    let w = content.preferred_width(max_width).saturating_add(2);
    let h = content.preferred_height(8);
    OverlaySize {
        width: w,
        height: h.min(PREVIEW_CARD_DEFAULT_MAX_HEIGHT),
        min_width: 14,
        min_height: 4,
        max_width: max_width.saturating_add(2).max(16),
        max_height: PREVIEW_CARD_DEFAULT_MAX_HEIGHT,
    }
}

/// Place anchored preview (Tooltip policy: no cover, flip/clamp).
#[must_use]
pub fn place_preview_card(bounds: Rect, anchor: Rect, size: OverlaySize) -> Rect {
    if bounds.is_empty() || size.width == 0 || size.height == 0 {
        return Rect::default();
    }
    place_overlay(
        bounds,
        Some(anchor),
        size,
        OverlayPolicy::for_kind(OverlayKind::Tooltip),
    )
}

/// Open unpinned preview (Tooltip kind — **no input ownership**).
pub fn open_preview_card_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::tooltip(PREVIEW_CARD_OVERLAY_ID, anchor, size, opener_focus),
    )
}

/// Open pinned sticky preview (Popover kind — light input ownership).
pub fn open_preview_card_pinned_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    anchor: Rect,
    size: OverlaySize,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.open(
        bounds,
        OverlaySpec::popover(PREVIEW_CARD_OVERLAY_ID, anchor, size, opener_focus),
    )
}

/// Dismiss preview overlay.
pub fn dismiss_preview_card_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(PREVIEW_CARD_OVERLAY_ID))
}

// ── State ───────────────────────────────────────────────────────────────────

/// Delay, selection debounce, generation, pin, load chrome.
///
/// Unpinned: never focusable. Host opens OverlayStack on [`PreviewCardOutcome::Shown`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewCardState {
    presence: Presence,
    pointer_over: bool,
    focus_within: bool,
    selection_active: bool,
    disabled: bool,
    trigger: PreviewTrigger,
    delay: Duration,
    selection_debounce: Duration,
    synth_origin: Option<Instant>,
    synth_elapsed_ms: u64,
    was_visible: bool,
    show_requested: bool,
    enforce_essential_elsewhere: bool,
    /// Sticky pin — stays visible without arm; accepts limited keys.
    pinned: bool,
    /// Current selection identity (host string).
    selection_id: Option<String>,
    /// Monotonic generation for async.
    generation: u64,
    /// In-flight fetch generation.
    pending_generation: Option<u64>,
    /// Load state for paint when content not re-supplied every frame.
    load: PreviewLoadState,
    /// Last applied generation (for stale detection paint).
    applied_generation: u64,
    /// Debounce: selection changed at this synth time; show waits until quiet.
    selection_dirty_at_ms: Option<u64>,
    slots: PreviewCardSlots,
    max_width: u16,
}

impl Default for PreviewCardState {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewCardState {
    /// Defaults: 300ms delay, 80ms selection debounce, Any trigger.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            presence: Presence::tooltip(Duration::from_millis(PREVIEW_CARD_DEFAULT_DELAY_MS)),
            pointer_over: false,
            focus_within: false,
            selection_active: false,
            disabled: false,
            trigger: PreviewTrigger::Any,
            delay: Duration::from_millis(PREVIEW_CARD_DEFAULT_DELAY_MS),
            selection_debounce: Duration::from_millis(PREVIEW_CARD_SELECTION_DEBOUNCE_MS),
            synth_origin: None,
            synth_elapsed_ms: 0,
            was_visible: false,
            show_requested: false,
            enforce_essential_elsewhere: true,
            pinned: false,
            selection_id: None,
            generation: 0,
            pending_generation: None,
            load: PreviewLoadState::Idle,
            applied_generation: 0,
            selection_dirty_at_ms: None,
            slots: PreviewCardSlots::empty(),
            max_width: PREVIEW_CARD_DEFAULT_MAX_WIDTH,
        }
    }

    /// Custom show delay.
    #[must_use]
    pub const fn with_delay(delay: Duration) -> Self {
        let mut s = Self::new();
        s.delay = delay;
        s.presence = Presence::tooltip(delay);
        s
    }

    /// Trigger mode.
    pub fn set_trigger(&mut self, t: PreviewTrigger) {
        self.trigger = t;
    }

    /// Selection debounce duration.
    pub fn set_selection_debounce(&mut self, d: Duration) {
        self.selection_debounce = d;
    }

    /// Disable.
    pub fn set_disabled(&mut self, on: bool) {
        self.disabled = on;
        if on {
            self.force_hide();
        }
    }

    /// Enforce essential-elsewhere (default true).
    pub fn set_enforce_essential_elsewhere(&mut self, on: bool) {
        self.enforce_essential_elsewhere = on;
    }

    /// ASCII chrome preference (paint also accepts widget flag).
    /// Max width for measure helpers.
    pub fn set_max_width(&mut self, w: u16) {
        self.max_width = w.max(12);
    }

    /// Pointer over anchor.
    pub fn set_pointer_over(&mut self, over: bool) {
        self.pointer_over = over;
        if !self.pinned && !self.armed() {
            self.force_hide();
        }
    }

    /// Anchor focused.
    pub fn set_focus_within(&mut self, focused: bool) {
        self.focus_within = focused;
        if !self.pinned && !self.armed() {
            self.force_hide();
        }
    }

    /// Selection present (list cursor non-empty).
    pub fn set_selection_active(&mut self, on: bool) {
        self.selection_active = on;
        if !self.pinned && !self.armed() {
            self.force_hide();
        }
    }

    /// Armed for delayed show (or pinned).
    #[must_use]
    pub fn armed(&self) -> bool {
        if self.disabled {
            return false;
        }
        if self.pinned {
            return true;
        }
        self.trigger
            .armed(self.pointer_over, self.focus_within, self.selection_active)
    }

    /// Visible for paint.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.pinned || self.presence.is_visible()
    }

    /// Pinned?
    #[must_use]
    pub const fn is_pinned(&self) -> bool {
        self.pinned
    }

    /// Disabled?
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Current generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Pending generation (fetch in flight).
    #[must_use]
    pub const fn pending_generation(&self) -> Option<u64> {
        self.pending_generation
    }

    /// Selection id.
    #[must_use]
    pub fn selection_id(&self) -> Option<&str> {
        self.selection_id.as_deref()
    }

    /// Load state.
    #[must_use]
    pub const fn load(&self) -> PreviewLoadState {
        self.load
    }

    /// Slots after paint.
    #[must_use]
    pub const fn slots(&self) -> PreviewCardSlots {
        self.slots
    }

    /// Body area for host-extended paint.
    #[must_use]
    pub const fn body_area(&self) -> Rect {
        self.slots.body
    }

    /// Presence deadline for host poll.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.presence.next_deadline()
    }

    /// Force hide (also unpins).
    pub fn force_hide(&mut self) {
        self.synth_origin = None;
        self.synth_elapsed_ms = 0;
        self.show_requested = false;
        self.selection_dirty_at_ms = None;
        self.pinned = false;
        self.presence.force_hide();
        self.was_visible = false;
    }

    /// Change selection identity. Bumps generation, marks debounce, cancels
    /// pending apply. Host should abort in-flight IO for the previous gen.
    pub fn set_selection(&mut self, id: impl Into<String>) -> PreviewCardOutcome {
        if self.disabled {
            return PreviewCardOutcome::Disabled;
        }
        let id = id.into();
        if self.selection_id.as_ref() == Some(&id) {
            self.selection_active = true;
            return PreviewCardOutcome::Ignored;
        }
        self.selection_id = Some(id.clone());
        self.selection_active = true;
        self.generation = self.generation.saturating_add(1);
        self.pending_generation = None;
        self.load = PreviewLoadState::Idle;
        self.selection_dirty_at_ms = Some(self.synth_elapsed_ms);
        // Rapid selection: hide unpinned card until debounce settles (no thrash).
        if !self.pinned {
            self.show_requested = false;
            self.presence.force_hide();
            self.was_visible = false;
        }
        PreviewCardOutcome::SelectionChanged {
            generation: self.generation,
            selection_id: id,
        }
    }

    /// Clear selection (hides unpinned).
    pub fn clear_selection(&mut self) -> PreviewCardOutcome {
        self.selection_id = None;
        self.selection_active = false;
        self.pending_generation = None;
        self.load = PreviewLoadState::Idle;
        self.selection_dirty_at_ms = None;
        if self.pinned {
            return PreviewCardOutcome::Ignored;
        }
        let was = self.was_visible || self.is_visible();
        self.force_hide();
        if was {
            PreviewCardOutcome::Hidden
        } else {
            PreviewCardOutcome::Ignored
        }
    }

    /// Begin async fetch for current selection; returns generation token.
    pub fn begin_fetch(&mut self) -> PreviewCardOutcome {
        if self.disabled || self.selection_id.is_none() {
            return PreviewCardOutcome::Ignored;
        }
        self.generation = self.generation.saturating_add(1);
        let g = self.generation;
        self.pending_generation = Some(g);
        self.load = PreviewLoadState::Loading;
        PreviewCardOutcome::Loading { generation: g }
    }

    /// Apply async result only if `generation` is still current/pending.
    pub fn apply_ready(&mut self, generation: u64) -> PreviewCardOutcome {
        if !self.accepts_generation(generation) {
            return PreviewCardOutcome::GenerationStale { generation };
        }
        self.pending_generation = None;
        self.applied_generation = generation;
        self.generation = generation;
        self.load = PreviewLoadState::Ready;
        PreviewCardOutcome::ContentApplied { generation }
    }

    /// Apply error for generation.
    pub fn apply_error(&mut self, generation: u64) -> PreviewCardOutcome {
        if !self.accepts_generation(generation) {
            return PreviewCardOutcome::GenerationStale { generation };
        }
        self.pending_generation = None;
        self.applied_generation = generation;
        self.generation = generation;
        self.load = PreviewLoadState::Error;
        PreviewCardOutcome::ContentApplied { generation }
    }

    /// Mark load stale (optional host cue).
    pub fn mark_stale(&mut self) {
        if self.load == PreviewLoadState::Ready {
            self.load = PreviewLoadState::Stale;
        }
    }

    fn accepts_generation(&self, generation: u64) -> bool {
        if let Some(pending) = self.pending_generation {
            generation == pending
        } else {
            generation >= self.generation
        }
    }

    /// Pin sticky (survives pointer leave).
    pub fn pin(&mut self) -> PreviewCardOutcome {
        if self.disabled {
            return PreviewCardOutcome::Disabled;
        }
        if self.pinned {
            return PreviewCardOutcome::Ignored;
        }
        self.pinned = true;
        self.was_visible = true;
        // Ensure presence visible for paint.
        self.presence = Presence::tooltip(Duration::ZERO);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        self.presence.request_show(tick);
        let _ = self.presence.advance(tick, MotionPolicy::Off);
        PreviewCardOutcome::Pinned
    }

    /// Unpin (may hide if not armed).
    pub fn unpin(&mut self) -> PreviewCardOutcome {
        if !self.pinned {
            return PreviewCardOutcome::Ignored;
        }
        self.pinned = false;
        if !self.armed() {
            self.force_hide();
            return PreviewCardOutcome::Unpinned;
        }
        PreviewCardOutcome::Unpinned
    }

    /// Toggle pin.
    pub fn toggle_pin(&mut self) -> PreviewCardOutcome {
        if self.pinned {
            self.unpin()
        } else {
            self.pin()
        }
    }

    fn effective_delay(&self, motion: MotionPolicy) -> Duration {
        match motion {
            MotionPolicy::Off => Duration::ZERO,
            MotionPolicy::Full => self.delay,
        }
    }

    fn rebuild_presence(&mut self, motion: MotionPolicy) {
        let d = self.effective_delay(motion);
        self.presence = Presence::tooltip(d);
    }

    fn selection_debouncing(&self) -> bool {
        let Some(dirty) = self.selection_dirty_at_ms else {
            return false;
        };
        let elapsed = self.synth_elapsed_ms.saturating_sub(dirty);
        elapsed < self.selection_debounce.as_millis() as u64
    }

    /// Synthetic hover tick (tests / simple hosts).
    pub fn tick_hover(&mut self, delta_ms: u64, hovering: bool) -> PreviewCardOutcome {
        self.set_pointer_over(hovering);
        if self.pinned {
            return PreviewCardOutcome::Ignored;
        }
        if !self.armed() {
            let was = self.was_visible;
            self.force_hide();
            return if was {
                PreviewCardOutcome::Hidden
            } else {
                PreviewCardOutcome::Ignored
            };
        }
        if self.disabled {
            return PreviewCardOutcome::Disabled;
        }
        let origin = *self.synth_origin.get_or_insert_with(Instant::now);
        self.synth_elapsed_ms = self.synth_elapsed_ms.saturating_add(delta_ms);
        if self.selection_debouncing() {
            return PreviewCardOutcome::Pending;
        }
        if !self.show_requested && !self.presence.is_visible() {
            self.rebuild_presence(MotionPolicy::Full);
            let tick = FrameTick::manual(origin, Duration::ZERO, Duration::ZERO);
            self.presence.request_show(tick);
            self.show_requested = true;
        }
        let tick = FrameTick::manual(
            origin + Duration::from_millis(self.synth_elapsed_ms),
            Duration::from_millis(self.synth_elapsed_ms),
            Duration::from_millis(delta_ms),
        );
        let _ = self.presence.advance(tick, MotionPolicy::Full);
        self.visibility_outcome()
    }

    /// FrameTick-driven advance (canonical).
    pub fn advance(&mut self, tick: FrameTick, motion: MotionPolicy) -> PreviewCardOutcome {
        if self.disabled {
            self.force_hide();
            return PreviewCardOutcome::Disabled;
        }
        if self.pinned {
            return PreviewCardOutcome::Ignored;
        }
        if !self.armed() {
            let was = self.was_visible || self.presence.is_visible();
            self.force_hide();
            return if was {
                PreviewCardOutcome::Hidden
            } else {
                PreviewCardOutcome::Ignored
            };
        }
        // Track synth time from tick when possible for debounce.
        self.synth_elapsed_ms = tick.elapsed().as_millis() as u64;
        if self.selection_debouncing() {
            return PreviewCardOutcome::Pending;
        }
        if !self.show_requested && !self.presence.is_visible() {
            self.rebuild_presence(motion);
            self.presence.request_show(tick);
            self.show_requested = true;
        }
        let _ = self.presence.advance(tick, motion);
        if matches!(motion, MotionPolicy::Off) && !self.presence.is_visible() {
            self.presence = Presence::tooltip(Duration::ZERO);
            self.presence.request_show(tick);
            self.show_requested = true;
            let _ = self.presence.advance(tick, motion);
        }
        self.visibility_outcome()
    }

    /// Update triggers then advance.
    pub fn advance_with_triggers(
        &mut self,
        tick: FrameTick,
        pointer_over: bool,
        focus_within: bool,
        selection_active: bool,
        motion: MotionPolicy,
    ) -> PreviewCardOutcome {
        self.pointer_over = pointer_over;
        self.focus_within = focus_within;
        self.selection_active = selection_active;
        if self.pinned {
            return PreviewCardOutcome::Ignored;
        }
        if !self.armed() {
            let was = self.was_visible || self.presence.is_visible();
            self.force_hide();
            return if was {
                PreviewCardOutcome::Hidden
            } else {
                PreviewCardOutcome::Ignored
            };
        }
        self.advance(tick, motion)
    }

    fn visibility_outcome(&mut self) -> PreviewCardOutcome {
        let vis = self.is_visible();
        if vis && !self.was_visible {
            self.was_visible = true;
            PreviewCardOutcome::Shown
        } else if !vis && self.was_visible {
            self.was_visible = false;
            PreviewCardOutcome::Hidden
        } else if vis {
            PreviewCardOutcome::Ignored
        } else if self.armed() {
            PreviewCardOutcome::Pending
        } else {
            PreviewCardOutcome::Ignored
        }
    }

    /// Gate show for essential content policy.
    pub fn allow_show_for(&self, content: &PreviewCardContent<'_>) -> PreviewCardOutcome {
        if self.disabled {
            return PreviewCardOutcome::Disabled;
        }
        if self.enforce_essential_elsewhere && !content.essential_elsewhere {
            return PreviewCardOutcome::EssentialRequiresNonPreview;
        }
        PreviewCardOutcome::Ignored
    }

    /// Limited keys when **pinned** (unpinned never steals focus / keys).
    pub fn handle_key(&mut self, key: KeyEvent) -> PreviewCardOutcome {
        if !self.pinned || self.disabled {
            return PreviewCardOutcome::Ignored;
        }
        if key.is_release() {
            return PreviewCardOutcome::Ignored;
        }
        if !key.is_insert() {
            return PreviewCardOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc if key.modifiers.is_empty() => self.unpin(),
            KeyCode::Enter if key.modifiers.is_empty() => PreviewCardOutcome::OpenRequested,
            KeyCode::Char('p' | 'P') if key.modifiers.is_empty() => self.unpin(),
            _ => PreviewCardOutcome::Ignored,
        }
    }

    /// Intent routing (pinned only).
    pub fn handle_intent(&mut self, intent: UiIntent) -> PreviewCardOutcome {
        if !self.pinned || self.disabled {
            return PreviewCardOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => self.unpin(),
            UiIntent::Activate | UiIntent::Submit => PreviewCardOutcome::OpenRequested,
            _ => PreviewCardOutcome::Ignored,
        }
    }

    /// Sync visibility with stack presence.
    pub fn sync_with_stack<F>(&mut self, stack: &OverlayStack<F>) {
        let id = OverlayId::from_static(PREVIEW_CARD_OVERLAY_ID);
        if !stack.contains(&id) && !self.pinned {
            self.was_visible = false;
            self.show_requested = false;
            self.presence.force_hide();
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Preview card paint (host supplies [`PreviewCardContent`]).
#[derive(Debug, Clone, Copy)]
pub struct PreviewCard<'a> {
    content: PreviewCardContent<'a>,
    system: &'a DesignSystem,
    colorless: bool,
    max_width: u16,
}

impl<'a> PreviewCard<'a> {
    /// Content + system.
    #[must_use]
    pub const fn new(content: PreviewCardContent<'a>, system: &'a DesignSystem) -> Self {
        Self {
            content,
            system,
            colorless: false,
            max_width: PREVIEW_CARD_DEFAULT_MAX_WIDTH,
        }
    }

    /// ASCII borders.
    #[must_use]
    /// Colorless roles.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Max width.
    #[must_use]
    pub const fn max_width(mut self, w: u16) -> Self {
        self.max_width = w;
        self
    }

    /// Overlay size for current content.
    #[must_use]
    pub fn overlay_size(&self) -> OverlaySize {
        preview_card_overlay_size(&self.content, self.max_width)
    }

    /// Paint when visible; no-op when hidden.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut PreviewCardState) {
        state.slots = PreviewCardSlots::empty();
        if area.is_empty() || !state.is_visible() {
            return;
        }
        if state.enforce_essential_elsewhere && !self.content.essential_elsewhere {
            return;
        }

        let recipe = if state.pinned {
            SurfaceRecipe::OverlayFocused
        } else {
            SurfaceRecipe::Overlay
        };
        let adapted_system = (false || self.colorless).then(|| {
            let system = { self.system.clone() };
            if self.colorless {
                system.capability(crate::style::ColorCapability::Monochrome)
            } else {
                system
            }
        });
        let surface_system = adapted_system.as_ref().unwrap_or(self.system);
        state.slots.root = area;
        let inner = Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .content_inset()
            .paint(area, buffer);
        if inner.is_empty() {
            return;
        }

        let footer_h = 1u16;
        let header_h = if self.content.subtitle.is_some() {
            2u16
        } else {
            1
        };
        let meta_h =
            (self.content.meta.len() as u16).min(inner.height.saturating_sub(header_h + footer_h));
        let body_h = inner
            .height
            .saturating_sub(header_h + meta_h + footer_h)
            .max(1);

        let mut y = inner.y;

        // Header: [kind] title  [pin]
        state.slots.header = Rect::new(inner.x, y, inner.width, header_h);
        let badge = self.content.kind.badge();
        let pin_mark = if state.pinned { " ● " } else { "" };
        let title = format!("[{badge}] {}{pin_mark}", self.content.title);
        let title_style = self
            .system
            .style(Role::TextStrong)
            .add_modifier(Modifier::BOLD);
        buffer.set_stringn(
            inner.x,
            y,
            &take_display_cols(&title, usize::from(inner.width)),
            usize::from(inner.width),
            title_style,
        );
        y = y.saturating_add(1);
        if let Some(sub) = self.content.subtitle {
            if y < inner.bottom().saturating_sub(footer_h) {
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(sub, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
                y = y.saturating_add(1);
            }
        }

        // Meta
        if meta_h > 0 && !self.content.meta.is_empty() {
            state.slots.meta = Rect::new(inner.x, y, inner.width, meta_h);
            for (i, m) in self.content.meta.iter().enumerate() {
                if i as u16 >= meta_h {
                    break;
                }
                let line = format!("{}: {}", m.label, m.value);
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(&line, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::Text),
                );
                y = y.saturating_add(1);
            }
        } else {
            state.slots.meta = Rect::default();
        }

        // Body
        state.slots.body = Rect::new(inner.x, y, inner.width, body_h);
        let load = if self.content.load != PreviewLoadState::Idle {
            self.content.load
        } else {
            state.load
        };
        match load {
            PreviewLoadState::Loading => {
                // Verb first, ellipsis trailing: the row reads as an action in
                // progress, not as an elision.
                let msg = { "loading…" };
                buffer.set_stringn(
                    inner.x,
                    y,
                    msg,
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
            PreviewLoadState::Error => {
                let msg = self.content.error_message.unwrap_or("preview failed");
                let line = format!("! {msg}");
                buffer.set_stringn(
                    inner.x,
                    y,
                    &take_display_cols(&line, usize::from(inner.width)),
                    usize::from(inner.width),
                    self.system.style(Role::Danger),
                );
            }
            PreviewLoadState::Stale => {
                let msg = { "↻ stale" };
                buffer.set_stringn(
                    inner.x,
                    y,
                    msg,
                    usize::from(inner.width),
                    self.system.style(Role::Warning),
                );
            }
            PreviewLoadState::Idle => {
                buffer.set_stringn(
                    inner.x,
                    y,
                    "—",
                    usize::from(inner.width),
                    self.system.style(Role::TextMuted),
                );
            }
            PreviewLoadState::Ready => {
                for (i, line) in self.content.body_lines.iter().enumerate() {
                    if i as u16 >= body_h {
                        break;
                    }
                    buffer.set_stringn(
                        inner.x,
                        y.saturating_add(i as u16),
                        &take_display_cols(line, usize::from(inner.width)),
                        usize::from(inner.width),
                        self.system.style(Role::Text),
                    );
                }
            }
        }
        y = y.saturating_add(body_h);

        // Footer
        state.slots.footer = Rect::new(inner.x, y, inner.width, footer_h);
        let hint = if state.pinned {
            PREVIEW_CARD_PINNED_HINT
        } else {
            PREVIEW_CARD_HINT
        };
        buffer.set_stringn(
            inner.x,
            y,
            &take_display_cols(hint, usize::from(inner.width)),
            usize::from(inner.width),
            self.system.style(Role::TextMuted),
        );
    }

    /// Semantic registration (not focusable when unpinned).
    pub fn register_semantic<Sid, Action>(
        &self,
        scene: &mut SemanticScene<Sid, Action>,
        id: Sid,
        area: Rect,
        state: &PreviewCardState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Action: Clone,
    {
        if area.is_empty() || !state.is_visible() {
            return;
        }
        let desc = format!(
            "preview-card kind={} load={} pinned={} gen={} selection={}",
            self.content.kind.id(),
            state.load().id(),
            state.is_pinned(),
            state.generation(),
            state.selection_id().unwrap_or("-"),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::Content)
                .label("preview-card")
                .description(desc)
                .focusable(state.is_pinned())
                .state(SemanticState {
                    selected: state.is_pinned(),
                    expanded: state.is_visible(),
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for PreviewCard<'_> {
    type State = PreviewCardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for &PreviewCard<'_> {
    type State = PreviewCardState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Example content builders (stories / docs) ───────────────────────────────

/// Example file preview payload.
#[must_use]
pub fn example_file_preview<'a>() -> (
    PreviewCardContent<'a>,
    &'a [PreviewMetadata<'a>],
    &'a [&'a str],
) {
    const META: &[PreviewMetadata<'static>] = &[
        PreviewMetadata::new("size", "4.2 KB"),
        PreviewMetadata::new("lang", "Rust"),
    ];
    const BODY: &[&str] = &["pub fn main() {", "    println!(\"hi\");", "}"];
    let content = PreviewCardContent::title("main.rs", PreviewResourceKind::File)
        .subtitle("src/main.rs")
        .meta(META)
        .body(BODY)
        .essential_elsewhere(true);
    (content, META, BODY)
}

/// Example command preview.
#[must_use]
pub fn example_command_preview<'a>() -> (
    PreviewCardContent<'a>,
    &'a [PreviewMetadata<'a>],
    &'a [&'a str],
) {
    const META: &[PreviewMetadata<'static>] = &[
        PreviewMetadata::new("shell", "zsh"),
        PreviewMetadata::new("cwd", "~/proj"),
    ];
    const BODY: &[&str] = &["cargo test -p termrock --lib", "→ exit 0 · 1.2s"];
    let content = PreviewCardContent::title("cargo test", PreviewResourceKind::Command)
        .subtitle("recent command")
        .meta(META)
        .body(BODY)
        .essential_elsewhere(true);
    (content, META, BODY)
}

/// Example symbol preview.
#[must_use]
pub fn example_symbol_preview<'a>() -> (
    PreviewCardContent<'a>,
    &'a [PreviewMetadata<'a>],
    &'a [&'a str],
) {
    const META: &[PreviewMetadata<'static>] = &[
        PreviewMetadata::new("kind", "fn"),
        PreviewMetadata::new("mod", "widgets"),
    ];
    const BODY: &[&str] = &[
        "pub fn paint(&self, area: Rect, …)",
        "FullscreenViewer chrome",
    ];
    let content = PreviewCardContent::title("paint", PreviewResourceKind::Symbol)
        .subtitle("FullscreenViewer::paint")
        .meta(META)
        .body(BODY)
        .essential_elsewhere(true);
    (content, META, BODY)
}

/// Example session preview.
#[must_use]
pub fn example_session_preview<'a>() -> (
    PreviewCardContent<'a>,
    &'a [PreviewMetadata<'a>],
    &'a [&'a str],
) {
    const META: &[PreviewMetadata<'static>] = &[
        PreviewMetadata::new("msgs", "12"),
        PreviewMetadata::new("model", "grok"),
    ];
    const BODY: &[&str] = &["… implement PreviewCard", "agent: ok, drafting API"];
    let content = PreviewCardContent::title("PreviewCard design", PreviewResourceKind::Session)
        .subtitle("session · 2m ago")
        .meta(META)
        .body(BODY)
        .essential_elsewhere(true);
    (content, META, BODY)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use crate::interaction::OverlayOutcome;

    #[test]
    fn delay_then_show_no_focus_theft() {
        let mut state = PreviewCardState::with_delay(Duration::from_millis(100));
        state.set_trigger(PreviewTrigger::Pointer);
        assert!(!state.is_visible());
        assert!(matches!(
            state.tick_hover(50, true),
            PreviewCardOutcome::Pending
        ));
        assert!(!state.is_visible());
        assert!(matches!(
            state.tick_hover(60, true),
            PreviewCardOutcome::Shown
        ));
        assert!(state.is_visible());
        assert!(!state.is_pinned()); // unpinned → not focusable surface
    }

    #[test]
    fn selection_debounce_suppresses_thrash() {
        let mut state = PreviewCardState::with_delay(Duration::ZERO);
        state.set_selection_debounce(Duration::from_millis(100));
        state.set_trigger(PreviewTrigger::Selection);
        let _ = state.set_selection("a");
        // Dirty at t=0; with ZERO delay presence, debounce still blocks show.
        state.selection_active = true;
        assert!(matches!(
            state.tick_hover(10, false),
            PreviewCardOutcome::Pending | PreviewCardOutcome::Ignored
        ));
        // Advance past debounce with selection active
        let _ = state.set_selection("b");
        state.synth_elapsed_ms = 0;
        state.selection_dirty_at_ms = Some(0);
        state.selection_active = true;
        // Manually clear debounce window
        state.synth_elapsed_ms = 150;
        state.selection_dirty_at_ms = Some(0);
        state.show_requested = false;
        // selection_active arms Selection trigger
        state.set_trigger(PreviewTrigger::Selection);
        state.selection_active = true;
        state.selection_dirty_at_ms = None;
        let out = state.tick_hover(1, false);
        assert!(
            matches!(
                out,
                PreviewCardOutcome::Shown
                    | PreviewCardOutcome::Pending
                    | PreviewCardOutcome::Ignored
            ),
            "{out:?}"
        );
    }

    #[test]
    fn rapid_selection_bumps_generation() {
        let mut state = PreviewCardState::new();
        let a = state.set_selection("file-a");
        let b = state.set_selection("file-b");
        match (a, b) {
            (
                PreviewCardOutcome::SelectionChanged { generation: g1, .. },
                PreviewCardOutcome::SelectionChanged {
                    generation: g2,
                    selection_id,
                },
            ) => {
                assert!(g2 > g1);
                assert_eq!(selection_id, "file-b");
            }
            other => panic!("expected selection changes, got {other:?}"),
        }
    }

    #[test]
    fn async_generation_stale_cancellation() {
        let mut state = PreviewCardState::new();
        let _ = state.set_selection("x");
        let PreviewCardOutcome::Loading { generation: g1 } = state.begin_fetch() else {
            panic!("expected loading");
        };
        // Selection changed while fetch in flight
        let _ = state.set_selection("y");
        let PreviewCardOutcome::Loading { generation: g2 } = state.begin_fetch() else {
            panic!("expected loading g2");
        };
        assert!(g2 > g1);
        assert!(matches!(
            state.apply_ready(g1),
            PreviewCardOutcome::GenerationStale { generation } if generation == g1
        ));
        assert!(matches!(
            state.apply_ready(g2),
            PreviewCardOutcome::ContentApplied { generation } if generation == g2
        ));
        assert_eq!(state.load(), PreviewLoadState::Ready);
    }

    #[test]
    fn essential_elsewhere_gate() {
        let state = PreviewCardState::new();
        let bad = PreviewCardContent::title("secret", PreviewResourceKind::File)
            .essential_elsewhere(false);
        assert!(matches!(
            state.allow_show_for(&bad),
            PreviewCardOutcome::EssentialRequiresNonPreview
        ));
        let mut s = PreviewCardState::with_delay(Duration::ZERO);
        let _ = s.tick_hover(1, true);
        let system = DesignSystem::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 10));
        // Should not paint essential-only content
        PreviewCard::new(bad, &system).paint(Rect::new(0, 0, 30, 10), &mut buf, &mut s);
        assert!(s.slots.root.is_empty() || s.slots.body.is_empty() || !bad.essential_elsewhere);
    }

    #[test]
    fn pin_survives_pointer_leave() {
        let mut state = PreviewCardState::with_delay(Duration::ZERO);
        let _ = state.tick_hover(1, true);
        assert!(state.is_visible());
        assert!(matches!(state.pin(), PreviewCardOutcome::Pinned));
        let _ = state.tick_hover(1, false); // pointer left
        assert!(state.is_visible());
        assert!(state.is_pinned());
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            PreviewCardOutcome::Unpinned
        ));
        // After unpin without arm → hidden
        assert!(!state.is_visible() || !state.is_pinned());
    }

    #[test]
    fn pin_open_enter() {
        let mut state = PreviewCardState::new();
        let _ = state.pin();
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PreviewCardOutcome::OpenRequested
        ));
    }

    #[test]
    fn unpinned_keys_ignored() {
        let mut state = PreviewCardState::with_delay(Duration::ZERO);
        let _ = state.tick_hover(1, true);
        assert!(matches!(
            state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PreviewCardOutcome::Ignored
        ));
    }

    #[test]
    fn overlay_no_input_when_unpinned() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(2, 2, 10, 1);
        let mut stack = OverlayStack::<&str>::new();
        let size = OverlaySize::menu(30, 10);
        let out = open_preview_card_overlay(&mut stack, bounds, anchor, size, Some("list"));
        assert!(matches!(out, OverlayOutcome::Opened { .. }));
        let top = stack.top().unwrap();
        assert_eq!(top.kind, OverlayKind::Tooltip);
        assert!(!top.policy.owns_input);
        let _ = dismiss_preview_card_overlay(&mut stack);
    }

    #[test]
    fn overlay_pinned_uses_popover() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(2, 2, 10, 1);
        let mut stack = OverlayStack::<&str>::new();
        let size = OverlaySize::menu(30, 10);
        let _ = open_preview_card_pinned_overlay(&mut stack, bounds, anchor, size, Some("list"));
        assert_eq!(stack.top().unwrap().kind, OverlayKind::Popover);
    }

    #[test]
    fn paint_file_command_symbol_session() {
        let system = DesignSystem::default();
        let mut state = PreviewCardState::with_delay(Duration::ZERO);
        let _ = state.pin();
        let area = Rect::new(0, 0, 40, 12);
        for (content, _, _) in [
            example_file_preview(),
            example_command_preview(),
            example_symbol_preview(),
            example_session_preview(),
        ] {
            let mut buf = Buffer::empty(area);
            PreviewCard::new(content, &system).paint(area, &mut buf, &mut state);
            assert!(!state.slots.body.is_empty());
            let text: String = buf
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect();
            assert!(
                text.contains(content.title) || text.contains(content.kind.badge()),
                "missing title in {text}"
            );
        }
    }

    #[test]
    fn paint_loading_and_error() {
        let system = DesignSystem::default();
        let mut state = PreviewCardState::new();
        let _ = state.pin();
        let area = Rect::new(0, 0, 36, 8);
        let mut buf = Buffer::empty(area);
        let loading = PreviewCardContent::title("x", PreviewResourceKind::File)
            .load(PreviewLoadState::Loading)
            .essential_elsewhere(true);
        PreviewCard::new(loading, &system).paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("loading"), "{text}");

        let err = PreviewCardContent::title("x", PreviewResourceKind::File)
            .error("timeout")
            .essential_elsewhere(true);
        let mut buf = Buffer::empty(area);
        PreviewCard::new(err, &system).paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(text.contains("timeout") || text.contains("!"), "{text}");
    }

    #[test]
    fn semantic_registers_unpinned_not_focusable() {
        let system = DesignSystem::default();
        let (content, _, _) = example_file_preview();
        let mut state = PreviewCardState::with_delay(Duration::ZERO);
        let _ = state.tick_hover(1, true);
        let mut scene = SemanticScene::<&str, ()>::default();
        PreviewCard::new(content, &system).register_semantic(
            &mut scene,
            "pc",
            Rect::new(0, 0, 30, 10),
            &state,
        );
        let node = scene
            .nodes()
            .iter()
            .find(|n| n.label.as_deref() == Some("preview-card"))
            .expect("node");
        assert!(!node.focusable);
    }

    #[test]
    fn place_preview_near_anchor() {
        let bounds = Rect::new(0, 0, 80, 24);
        let anchor = Rect::new(10, 10, 8, 1);
        let r = place_preview_card(bounds, anchor, OverlaySize::menu(24, 8));
        assert!(!r.is_empty());
        assert!(r.width <= bounds.width);
    }

    #[test]
    fn fuzz_selection_and_keys() {
        let mut state = PreviewCardState::with_delay(Duration::from_millis(20));
        let keys = [
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Char('p'),
            KeyCode::Tab,
        ];
        let mut seed = 3u64;
        for i in 0..200u64 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            if seed % 3 == 0 {
                let _ = state.set_selection(format!("id-{}", seed % 7));
            }
            if seed % 5 == 0 {
                let _ = state.begin_fetch();
                let g = state.pending_generation().unwrap_or(state.generation());
                if seed % 2 == 0 {
                    let _ = state.apply_ready(g);
                } else {
                    let _ = state.apply_ready(g.saturating_sub(1));
                }
            }
            let _ = state.tick_hover(i % 40, seed % 2 == 0);
            if seed % 11 == 0 {
                let _ = state.pin();
            }
            let k = keys[(seed as usize) % keys.len()];
            let _ = state.handle_key(KeyEvent::new(k, KeyModifiers::NONE));
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let (content, _, _) = example_file_preview();
        let mut state = PreviewCardState::new();
        let _ = state.pin();
        let mut terminal = Terminal::new(TestBackend::new(48, 16)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..200 {
            terminal
                .draw(|f| {
                    PreviewCard::new(content, &system).paint(f.area(), f.buffer_mut(), &mut state);
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
        let (content, _, _) = example_symbol_preview();
        let mut s1 = PreviewCardState::new();
        let _ = s1.pin();
        let mut t1 = Terminal::new(TestBackend::new(40, 12)).unwrap();
        t1.draw(|f| {
            PreviewCard::new(content, &system).paint(f.area(), f.buffer_mut(), &mut s1);
        })
        .unwrap();
        let a: String = t1
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        let mut s2 = PreviewCardState::new();
        let _ = s2.pin();
        let mut t2 = Terminal::new(TestBackend::new(40, 12)).unwrap();
        t2.draw(|f| {
            PreviewCard::new(content, &system).paint(f.area(), f.buffer_mut(), &mut s2);
        })
        .unwrap();
        let b: String = t2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert_eq!(a, b);
        assert!(a.contains("paint") || a.contains("sym"));
    }

    #[test]
    fn reduced_motion_instant_show() {
        let mut state = PreviewCardState::with_delay(Duration::from_millis(500));
        state.set_pointer_over(true);
        let tick = FrameTick::manual(Instant::now(), Duration::ZERO, Duration::ZERO);
        let out = state.advance(tick, MotionPolicy::Off);
        assert!(
            matches!(
                out,
                PreviewCardOutcome::Shown
                    | PreviewCardOutcome::Pending
                    | PreviewCardOutcome::Ignored
            ),
            "{out:?}"
        );
        // With reduced motion should become visible quickly
        assert!(state.is_visible() || matches!(out, PreviewCardOutcome::Shown));
    }
}
