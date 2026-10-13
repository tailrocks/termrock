// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Toast** — transient notifications with priority, actions, lifecycle, and
//! non-disruptive placement.
//!
//! **Mission.** Sonner-class toast stack for informational, success, warning,
//! error, progress, undo, persistent, and grouped notifications. Queueing,
//! deduplication, replacement, timeout pause, and announcement semantics without
//! covering critical content or stealing keyboard focus.
//!
//! **Focus law.** Toasts are **never focusable**. Actions are activated via
//! host hotkeys / pointer hits only; keyboard focus stays on the primary UI.
//! Missed or expired items archive to [`ToastQueue::drain_missed`] for
//! [`super::NotificationCenter`] (`ToastArchive`).
//!
//! **vs Alert/Callout.** Inline layout feedback. Toast is transient overlay.
//! **vs AlertDialog.** Modal risk. Toast never traps.
//!
//! Research: shadcn/Sonner, desktop notifications, Textual notifications, agent
//! task updates.

use std::collections::VecDeque;
use std::time::Duration;
use web_time::Instant;

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

use super::{Surface, SurfaceRecipe};
use crate::{
    runtime::{FrameTick, Presence},
    style::{DesignSystem, Motion, Role},
    text::{display_cols, take_display_cols},
};

// ── Constants (migrated timing defaults) ────────────────────────────────────

/// Default auto-dismiss TTL (Sonner-ish; hosts may override).
pub const TOAST_DEFAULT_TTL: Duration = Duration::from_secs(4);
/// Maximum simultaneous visible toasts in a stack.
pub const TOAST_DEFAULT_MAX_VISIBLE: usize = 5;
/// Vertical gap between stacked toasts (cells).
pub const TOAST_STACK_GAP: u16 = 0;
/// Default horizontal margin from outer edge.
pub const TOAST_DEFAULT_H_MARGIN: u16 = 2;
/// Default vertical margin from outer edge.
pub const TOAST_DEFAULT_V_MARGIN: u16 = 1;

// ── Severity / Anchor / Lifetime (preserved public API) ─────────────────────

/// Semantic status severities used by toasts, banners, and status slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Severity {
    /// Informational content with no success or failure implication.
    #[default]
    Info,
    /// Successful completion or a healthy state.
    Success,
    /// A condition requiring attention but not an error.
    Warning,
    /// A failed operation or invalid state.
    Error,
}

impl Severity {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    /// Semantic paint role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Info => Role::Info,
            Self::Success => Role::Success,
            Self::Warning => Role::Warning,
            Self::Error => Role::Danger,
        }
    }

    /// Non-color glyph (ASCII).
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Success => "+",
            Self::Warning => "!",
            Self::Error => "x",
        }
    }

    /// Unicode glyph.
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Success => "✓",
            Self::Warning => "!",
            Self::Error => "✗",
        }
    }
}

/// Corners used to anchor a toast within its containing rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Anchor {
    /// Places content at the top left.
    TopLeft,
    /// Places content at the top right.
    #[default]
    TopRight,
    /// Places content at the bottom left.
    BottomLeft,
    /// Places content at the bottom right.
    BottomRight,
}

impl Anchor {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::TopLeft => "top-left",
            Self::TopRight => "top-right",
            Self::BottomLeft => "bottom-left",
            Self::BottomRight => "bottom-right",
        }
    }

    /// Stack grows downward from top anchors; upward from bottom.
    #[must_use]
    pub const fn stacks_down(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }
}

/// Lifetime policy for state-managed toast visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ToastLifetime {
    /// Remains visible until explicitly dismissed.
    #[default]
    Persistent,
    /// Expires after the given duration from the latest `show` call.
    ExpiresAfter(Duration),
}

impl ToastLifetime {
    /// Default transient TTL.
    #[must_use]
    pub const fn default_ttl() -> Self {
        Self::ExpiresAfter(TOAST_DEFAULT_TTL)
    }
}

// ── Kind / priority / outcomes ──────────────────────────────────────────────

/// Toast content kind (extends severity with progress / undo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ToastKind {
    /// Informational.
    #[default]
    Info,
    /// Success.
    Success,
    /// Warning.
    Warning,
    /// Error.
    Error,
    /// In-progress operation (optional percent).
    Progress,
    /// Undo-capable action result.
    Undo,
}

impl ToastKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Progress => "progress",
            Self::Undo => "undo",
        }
    }

    /// From classic severity.
    #[must_use]
    pub const fn from_severity(s: Severity) -> Self {
        match s {
            Severity::Info => Self::Info,
            Severity::Success => Self::Success,
            Severity::Warning => Self::Warning,
            Severity::Error => Self::Error,
        }
    }

    /// Border / glyph role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Info | Self::Progress => Role::Info,
            Self::Success | Self::Undo => Role::Success,
            Self::Warning => Role::Warning,
            Self::Error => Role::Danger,
        }
    }

    /// ASCII glyph.
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        match self {
            Self::Info => "i",
            Self::Success => "+",
            Self::Warning => "!",
            Self::Error => "x",
            Self::Progress => "~",
            Self::Undo => "<",
        }
    }

    /// Unicode glyph.
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Success => "✓",
            Self::Warning => "!",
            Self::Error => "✗",
            Self::Progress => "…",
            Self::Undo => "↶",
        }
    }
}

/// Stacking / replacement priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ToastPriority {
    /// Background.
    Low,
    /// Default.
    #[default]
    Normal,
    /// Prefer keep when queue is full.
    High,
    /// Always keep; may archive lower when full.
    Critical,
}

impl ToastPriority {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Host coordination for a single toast or queue.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ToastOutcome {
    /// No change.
    Ignored,
    /// Became visible / pushed.
    Shown {
        /// Entry id.
        id: String,
    },
    /// Expired via TTL.
    Expired {
        /// Entry id.
        id: String,
    },
    /// Explicitly dismissed.
    Dismissed {
        /// Entry id.
        id: String,
    },
    /// Action (e.g. undo) activated.
    ActionActivated {
        /// Toast id.
        id: String,
        /// Action id.
        action: String,
    },
    /// Replaced an existing entry with the same replace/dedup key.
    Replaced {
        /// Previous id.
        previous_id: String,
        /// New id.
        id: String,
    },
    /// Archived to missed list (NotificationCenter route).
    Archived {
        /// Entry id.
        id: String,
    },
    /// Queue paused (hover / host).
    Paused,
    /// Queue resumed.
    Resumed,
    /// Dedup dropped a push (still live).
    Deduplicated {
        /// Existing id.
        id: String,
    },
}

// ── Single ToastState (preserved) ───────────────────────────────────────────

/// Visibility and expiry state for a **single** transient notification.
///
/// Backed by [`Presence`] so TTL, deadlines, and focus rules share one motion
/// primitive (toasts are never focusable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastState {
    presence: Presence,
    lifetime: ToastLifetime,
    /// Timeout pause (hover over toast region).
    paused: bool,
}

impl Default for ToastState {
    fn default() -> Self {
        Self::new(ToastLifetime::Persistent)
    }
}

impl ToastState {
    /// Creates hidden toast state with an explicit lifetime policy.
    pub const fn new(lifetime: ToastLifetime) -> Self {
        let presence = match lifetime {
            ToastLifetime::Persistent => Presence::persistent(),
            ToastLifetime::ExpiresAfter(ttl) => Presence::toast(ttl),
        };
        Self {
            presence,
            lifetime,
            paused: false,
        }
    }

    /// Lifetime policy.
    #[must_use]
    pub const fn lifetime(self) -> ToastLifetime {
        self.lifetime
    }

    /// Makes the toast visible starting at this frame.
    pub fn show(&mut self, tick: FrameTick) {
        self.presence.request_show(tick);
    }

    /// Hides the toast immediately.
    pub const fn dismiss(&mut self) {
        self.presence.force_hide();
        self.paused = false;
    }

    /// Pause TTL (e.g. pointer over toast).
    pub fn set_paused(&mut self, on: bool) {
        self.paused = on;
    }

    /// Paused?
    #[must_use]
    pub const fn is_paused(self) -> bool {
        self.paused
    }

    /// Advance TTL (call once per frame when shown). No-op while paused.
    pub fn advance(&mut self, tick: FrameTick) {
        if self.paused {
            return;
        }
        let _ = self.presence.advance(tick, Motion::Off);
    }

    /// Returns whether the toast is visible at this frame.
    pub fn is_visible(&self, tick: FrameTick) -> bool {
        // Lazily apply TTL without requiring host to call advance (unless paused).
        if self.paused {
            return self.presence.is_visible();
        }
        let mut copy = *self;
        copy.advance(tick);
        copy.presence.is_visible()
    }

    /// Returns the expiration deadline, or `None` when hidden or persistent.
    pub fn next_deadline(&self) -> Option<Instant> {
        if self.paused {
            return None;
        }
        self.presence.next_deadline()
    }

    /// Toasts never take focus.
    #[must_use]
    pub const fn is_focusable(self) -> bool {
        false
    }
}

// ── Spec / archive / queue ──────────────────────────────────────────────────

/// Host push specification (owned strings for queue lifetime).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastSpec {
    /// Stable identity (replace / dismiss).
    pub id: String,
    /// Optional dedup key — drop push if same key still visible.
    pub dedup_key: Option<String>,
    /// Kind.
    pub kind: ToastKind,
    /// Priority for eviction.
    pub priority: ToastPriority,
    /// Optional title (shown above body when set).
    pub title: Option<String>,
    /// Primary message.
    pub message: String,
    /// Lifetime.
    pub lifetime: ToastLifetime,
    /// Progress percent 0–100 when kind is Progress.
    pub progress: Option<u8>,
    /// Group id for stacking related updates.
    pub group_id: Option<String>,
    /// Replace any live entry with this id.
    pub replace_id: Option<String>,
    /// Undo action label (kind often Undo).
    pub undo_label: Option<String>,
    /// Screen-reader / announcement copy (defaults to message).
    pub announcement: Option<String>,
    /// Archive to missed list on expire (default true).
    pub archive_on_expire: bool,
}

impl ToastSpec {
    /// Minimal success-style toast.
    #[must_use]
    pub fn message(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            dedup_key: None,
            kind: ToastKind::Success,
            priority: ToastPriority::Normal,
            title: None,
            message: message.into(),
            lifetime: ToastLifetime::default_ttl(),
            progress: None,
            group_id: None,
            replace_id: None,
            undo_label: None,
            announcement: None,
            archive_on_expire: true,
        }
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        self
    }

    /// Severity helper.
    #[must_use]
    pub const fn severity(mut self, s: Severity) -> Self {
        self.kind = ToastKind::from_severity(s);
        self
    }

    /// Priority.
    #[must_use]
    pub const fn priority(mut self, p: ToastPriority) -> Self {
        self.priority = p;
        self
    }

    /// Title.
    #[must_use]
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Lifetime.
    #[must_use]
    pub const fn lifetime(mut self, life: ToastLifetime) -> Self {
        self.lifetime = life;
        self
    }

    /// Persistent until dismiss.
    #[must_use]
    pub const fn persistent(mut self) -> Self {
        self.lifetime = ToastLifetime::Persistent;
        self
    }

    /// Progress percent.
    #[must_use]
    pub const fn progress(mut self, pct: u8) -> Self {
        self.kind = ToastKind::Progress;
        self.progress = Some(if pct > 100 { 100 } else { pct });
        self
    }

    /// Dedup key.
    #[must_use]
    pub fn dedup_key(mut self, key: impl Into<String>) -> Self {
        self.dedup_key = Some(key.into());
        self
    }

    /// Replace id.
    #[must_use]
    pub fn replace(mut self, id: impl Into<String>) -> Self {
        self.replace_id = Some(id.into());
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, id: impl Into<String>) -> Self {
        self.group_id = Some(id.into());
        self
    }

    /// Undo label (sets kind Undo when not Progress).
    #[must_use]
    pub fn undo(mut self, label: impl Into<String>) -> Self {
        self.undo_label = Some(label.into());
        if !matches!(self.kind, ToastKind::Progress) {
            self.kind = ToastKind::Undo;
        }
        self
    }

    /// Announcement text.
    #[must_use]
    pub fn announcement(mut self, text: impl Into<String>) -> Self {
        self.announcement = Some(text.into());
        self
    }

    /// Announcement string for a11y hosts.
    #[must_use]
    pub fn announce_text(&self) -> &str {
        self.announcement
            .as_deref()
            .unwrap_or(self.message.as_str())
    }
}

/// Archived toast for NotificationCenter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastArchive {
    /// Original id.
    pub id: String,
    /// Kind.
    pub kind: ToastKind,
    /// Title.
    pub title: Option<String>,
    /// Message.
    pub message: String,
    /// Why archived.
    pub reason: ToastArchiveReason,
    /// Announcement snapshot.
    pub announcement: String,
}

/// Why an item was archived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ToastArchiveReason {
    /// TTL expired without interaction.
    Expired,
    /// Evicted from full queue.
    Evicted,
    /// Explicit archive / host move.
    HostArchived,
    /// Dismissed (optional history).
    Dismissed,
}

/// Live queue entry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveToast {
    id: String,
    dedup_key: Option<String>,
    kind: ToastKind,
    priority: ToastPriority,
    title: Option<String>,
    message: String,
    state: ToastState,
    progress: Option<u8>,
    group_id: Option<String>,
    undo_label: Option<String>,
    announcement: String,
    archive_on_expire: bool,
    /// Last painted rect (hit testing).
    region: Option<Rect>,
}

/// Multi-toast host: queue, dedup, replace, pause, archive for NotificationCenter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToastQueue {
    live: VecDeque<LiveToast>,
    missed: Vec<ToastArchive>,
    max_visible: usize,
    max_missed: usize,
    paused: bool,
    anchor: Anchor,
    h_margin: u16,
    v_margin: u16,
    /// Generation for hosts tracking pushes.
    generation: u64,
}

impl Default for ToastQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastQueue {
    /// Empty queue with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            live: VecDeque::new(),
            missed: Vec::new(),
            max_visible: TOAST_DEFAULT_MAX_VISIBLE,
            max_missed: 50,
            paused: false,
            anchor: Anchor::TopRight,
            h_margin: TOAST_DEFAULT_H_MARGIN,
            v_margin: TOAST_DEFAULT_V_MARGIN,
            generation: 0,
        }
    }

    /// Max simultaneous visible.
    pub fn set_max_visible(&mut self, n: usize) {
        self.max_visible = n.max(1);
    }

    /// Placement anchor for the stack.
    pub fn set_anchor(&mut self, anchor: Anchor) {
        self.anchor = anchor;
    }

    /// Margins.
    pub fn set_margins(&mut self, horizontal: u16, vertical: u16) {
        self.h_margin = horizontal;
        self.v_margin = vertical;
    }

    /// Anchor.
    #[must_use]
    pub const fn anchor(&self) -> Anchor {
        self.anchor
    }

    /// Live count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Empty?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// Global pause?
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Missed / archived count (NotificationCenter inbox size).
    #[must_use]
    pub fn missed_len(&self) -> usize {
        self.missed.len()
    }

    /// Borrow missed list.
    #[must_use]
    pub fn missed(&self) -> &[ToastArchive] {
        &self.missed
    }

    /// Drain missed into caller (NotificationCenter open).
    pub fn drain_missed(&mut self) -> Vec<ToastArchive> {
        std::mem::take(&mut self.missed)
    }

    /// Pause all TTL clocks (pointer over stack).
    pub fn set_paused(&mut self, on: bool) -> ToastOutcome {
        if self.paused == on {
            return ToastOutcome::Ignored;
        }
        self.paused = on;
        for t in &mut self.live {
            t.state.set_paused(on);
        }
        if on {
            ToastOutcome::Paused
        } else {
            ToastOutcome::Resumed
        }
    }

    /// Push a toast; handles replace, dedup, eviction.
    pub fn push(&mut self, tick: FrameTick, spec: ToastSpec) -> ToastOutcome {
        self.generation = self.generation.saturating_add(1);

        // Replace by explicit id
        if let Some(ref rid) = spec.replace_id {
            if let Some(pos) = self.live.iter().position(|t| t.id == *rid) {
                let prev = self.live.remove(pos).expect("pos");
                let id = spec.id.clone();
                self.insert_live(tick, spec);
                return ToastOutcome::Replaced {
                    previous_id: prev.id,
                    id,
                };
            }
        }
        // Replace same id
        if let Some(pos) = self.live.iter().position(|t| t.id == spec.id) {
            let prev = self.live.remove(pos).expect("pos");
            let id = spec.id.clone();
            self.insert_live(tick, spec);
            return ToastOutcome::Replaced {
                previous_id: prev.id,
                id,
            };
        }
        // Dedup
        if let Some(ref key) = spec.dedup_key {
            if let Some(existing) = self.live.iter().find(|t| t.dedup_key.as_ref() == Some(key)) {
                return ToastOutcome::Deduplicated {
                    id: existing.id.clone(),
                };
            }
        }
        // Group replace: latest progress in group replaces prior
        if let Some(ref gid) = spec.group_id {
            if matches!(spec.kind, ToastKind::Progress) {
                if let Some(pos) = self
                    .live
                    .iter()
                    .position(|t| t.group_id.as_ref() == Some(gid))
                {
                    let prev = self.live.remove(pos).expect("pos");
                    let id = spec.id.clone();
                    self.insert_live(tick, spec);
                    return ToastOutcome::Replaced {
                        previous_id: prev.id,
                        id,
                    };
                }
            }
        }

        let id = spec.id.clone();
        self.insert_live(tick, spec);
        self.evict_if_needed();
        ToastOutcome::Shown { id }
    }

    fn insert_live(&mut self, tick: FrameTick, spec: ToastSpec) {
        let mut state = ToastState::new(spec.lifetime);
        state.show(tick);
        state.set_paused(self.paused);
        self.live.push_front(LiveToast {
            id: spec.id,
            dedup_key: spec.dedup_key,
            kind: spec.kind,
            priority: spec.priority,
            title: spec.title,
            message: spec.message,
            state,
            progress: spec.progress,
            group_id: spec.group_id,
            undo_label: spec.undo_label,
            announcement: spec.announcement.unwrap_or_else(|| String::new()),
            archive_on_expire: spec.archive_on_expire,
            region: None,
        });
        // Fix empty announcement
        if let Some(front) = self.live.front_mut() {
            if front.announcement.is_empty() {
                front.announcement = front.message.clone();
            }
        }
    }

    fn evict_if_needed(&mut self) {
        while self.live.len() > self.max_visible {
            // Drop lowest priority from back (oldest low)
            let mut worst = self.live.len() - 1;
            for (i, t) in self.live.iter().enumerate().rev() {
                if t.priority < self.live[worst].priority {
                    worst = i;
                }
            }
            if let Some(t) = self.live.remove(worst) {
                if t.priority == ToastPriority::Critical && self.live.len() >= self.max_visible {
                    // Prefer archiving non-critical — if only critical left, still archive oldest
                }
                self.archive(t, ToastArchiveReason::Evicted);
            } else {
                break;
            }
        }
    }

    fn archive(&mut self, t: LiveToast, reason: ToastArchiveReason) {
        if !t.archive_on_expire && matches!(reason, ToastArchiveReason::Expired) {
            return;
        }
        self.missed.push(ToastArchive {
            id: t.id,
            kind: t.kind,
            title: t.title,
            message: t.message,
            reason,
            announcement: t.announcement,
        });
        while self.missed.len() > self.max_missed {
            self.missed.remove(0);
        }
    }

    /// Dismiss by id.
    pub fn dismiss(&mut self, id: &str) -> ToastOutcome {
        if let Some(pos) = self.live.iter().position(|t| t.id == id) {
            if let Some(t) = self.live.remove(pos) {
                self.archive(t, ToastArchiveReason::Dismissed);
                return ToastOutcome::Dismissed { id: id.to_string() };
            }
        }
        ToastOutcome::Ignored
    }

    /// Activate undo / action for top matching id (host maps hotkey).
    pub fn activate_action(&mut self, id: &str, action: impl Into<String>) -> ToastOutcome {
        if self.live.iter().any(|t| t.id == id) {
            ToastOutcome::ActionActivated {
                id: id.to_string(),
                action: action.into(),
            }
        } else {
            ToastOutcome::Ignored
        }
    }

    /// Advance all live toasts; expire and archive.
    pub fn advance(&mut self, tick: FrameTick) -> Vec<ToastOutcome> {
        let mut outs = Vec::new();
        if self.paused {
            return outs;
        }
        let mut i = 0;
        while i < self.live.len() {
            self.live[i].state.advance(tick);
            if !self.live[i].state.is_visible(tick) {
                if let Some(t) = self.live.remove(i) {
                    let id = t.id.clone();
                    self.archive(t, ToastArchiveReason::Expired);
                    outs.push(ToastOutcome::Expired { id });
                }
            } else {
                i += 1;
            }
        }
        outs
    }

    /// Earliest deadline among live (for host wake).
    pub fn next_deadline(&self) -> Option<Instant> {
        if self.paused {
            return None;
        }
        self.live
            .iter()
            .filter_map(|t| t.state.next_deadline())
            .min()
    }

    /// Latest announcement for a11y (most recent push still live).
    #[must_use]
    pub fn latest_announcement(&self) -> Option<&str> {
        self.live.front().map(|t| t.announcement.as_str())
    }

    /// Live ids in paint order (front = newest).
    pub fn live_ids(&self) -> impl Iterator<Item = &str> {
        self.live.iter().map(|t| t.id.as_str())
    }

    /// Hit-test stacked regions for pause / click (call after paint).
    pub fn region_at(&self, x: u16, y: u16) -> Option<&str> {
        let pos = ratatui_core::layout::Position { x, y };
        for t in &self.live {
            if t.region.is_some_and(|r| r.contains(pos)) {
                return Some(t.id.as_str());
            }
        }
        None
    }
}

// ── Paint helpers ───────────────────────────────────────────────────────────

fn place_toast(
    outer: Rect,
    width: u16,
    height: u16,
    anchor: Anchor,
    h_margin: u16,
    v_margin: u16,
    stack_index: u16,
    stacks_down: bool,
) -> Option<Rect> {
    if outer.is_empty() || width == 0 || height == 0 {
        return None;
    }
    let width = width.min(outer.width);
    let height = height.min(outer.height);
    let x = match anchor {
        Anchor::TopLeft | Anchor::BottomLeft => outer
            .left()
            .saturating_add(h_margin)
            .min(outer.right().saturating_sub(width)),
        Anchor::TopRight | Anchor::BottomRight => outer
            .right()
            .saturating_sub(h_margin)
            .saturating_sub(width)
            .max(outer.left()),
    };
    let step = height.saturating_add(TOAST_STACK_GAP);
    let y = if stacks_down {
        let base = outer
            .top()
            .saturating_add(v_margin)
            .min(outer.bottom().saturating_sub(height));
        base.saturating_add(stack_index.saturating_mul(step))
            .min(outer.bottom().saturating_sub(height))
    } else {
        let base = outer
            .bottom()
            .saturating_sub(v_margin)
            .saturating_sub(height)
            .max(outer.top());
        base.saturating_sub(stack_index.saturating_mul(step))
            .max(outer.top())
    };
    Some(Rect::new(x, y, width, height))
}

fn measure_toast_size(
    title: Option<&str>,
    message: &str,
    has_undo: bool,
    progress: Option<u8>,
) -> (u16, u16) {
    let mut w = display_cols(message) as u16 + 6; // glyph + pad + border
    if let Some(t) = title {
        w = w.max(display_cols(t) as u16 + 6);
    }
    if has_undo {
        w = w.saturating_add(8);
    }
    if progress.is_some() {
        w = w.max(16);
    }
    let mut h = 3u16; // border + line
    if title.is_some() {
        h = h.saturating_add(1);
    }
    if progress.is_some() {
        h = h.saturating_add(1);
    }
    (w.clamp(10, 60), h.min(6))
}

fn paint_one_toast(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    kind: ToastKind,
    title: Option<&str>,
    message: &str,
    progress: Option<u8>,
    undo_label: Option<&str>,
    ascii: bool,
    style_override: Option<Style>,
) {
    if area.is_empty() {
        return;
    }
    let inner = Surface::new(system)
        .recipe(SurfaceRecipe::Overlay)
        .bordered(true)
        .padding(0, 0)
        .paint(area, buffer);
    if inner.is_empty() {
        return;
    }
    let rail = system.style(kind.role());
    for y in inner.y..inner.bottom() {
        buffer[(inner.x, y)].set_style(rail);
        buffer[(inner.x, y)].set_symbol("│");
    }
    let content = Rect::new(
        inner.x.saturating_add(2),
        inner.y,
        inner.width.saturating_sub(2),
        inner.height,
    );
    if content.is_empty() {
        return;
    }
    let glyph = if ascii {
        kind.glyph_ascii()
    } else {
        kind.glyph_unicode()
    };
    let text_style = style_override.unwrap_or(system.style(Role::Text));
    let mut y = content.y;
    if let Some(title) = title {
        buffer.set_stringn(content.x, y, glyph, 1, system.style(kind.role()));
        buffer.set_stringn(
            content.x.saturating_add(2),
            y,
            &take_display_cols(title, usize::from(content.width.saturating_sub(2))),
            usize::from(content.width.saturating_sub(2)),
            system.style(Role::TextStrong).add_modifier(Modifier::BOLD),
        );
        y = y.saturating_add(1);
        if y < content.bottom() {
            buffer.set_stringn(
                content.x,
                y,
                &take_display_cols(message, usize::from(content.width)),
                usize::from(content.width),
                text_style,
            );
            y = y.saturating_add(1);
        }
    } else {
        buffer.set_stringn(content.x, y, glyph, 1, system.style(kind.role()));
        let mut line = message.to_string();
        if let Some(ul) = undo_label {
            line = format!("{line}  [{ul}]");
        }
        buffer.set_stringn(
            content.x.saturating_add(2),
            y,
            &take_display_cols(&line, usize::from(content.width.saturating_sub(2))),
            usize::from(content.width.saturating_sub(2)),
            text_style,
        );
        y = y.saturating_add(1);
    }
    if let Some(pct) = progress {
        if y < content.bottom() {
            let bar_w = usize::from(content.width).saturating_sub(5).max(1);
            buffer.set_stringn(
                content.x,
                y,
                &format!("{pct:3}% "),
                5.min(usize::from(content.width)),
                system.style(Role::Info),
            );
            let scaled = f64::from(pct) * bar_w as f64 / 100.0;
            let filled = scaled.floor() as usize;
            let partial = ((scaled.fract() * 8.0).floor() as usize).min(7);
            let partial_glyph = crate::style::BLOCK_RAMP[partial].to_string();
            let track_x = content.x.saturating_add(5);
            for column in 0..bar_w {
                let on = column < filled || (!ascii && column == filled && partial > 0);
                let symbol = if column < filled {
                    if ascii { "#" } else { "█" }
                } else if !ascii && column == filled && partial > 0 {
                    partial_glyph.as_str()
                } else if ascii {
                    "-"
                } else {
                    " "
                };
                buffer.set_stringn(
                    track_x.saturating_add(column as u16),
                    y,
                    symbol,
                    1,
                    system.style(if on { Role::Info } else { Role::Sunken }),
                );
            }
        }
    } else if undo_label.is_some() && title.is_some() && y < content.bottom() {
        if let Some(ul) = undo_label {
            buffer.set_stringn(
                content.x,
                y,
                &take_display_cols(&format!("[{ul}]"), usize::from(content.width)),
                usize::from(content.width),
                system.style(Role::TextMuted),
            );
        }
    }
}

// ── Single Toast widget (preserved shape) ───────────────────────────────────

/// Transient notification overlay with caller-owned lifetime and placement.
///
/// See lookbook `toast/*` stories for semantic variants and timing.
///
/// # Examples
///
/// ```
/// use ratatui_core::layout::Rect;
/// use termrock::style::DesignSystem;
/// use termrock::widgets::{Anchor, Severity, Toast};
///
/// let system = DesignSystem::phosphor();
/// let toast = Toast::new(&system, "Saved", Severity::Success)
///     .anchor(Anchor::BottomRight)
///     .margins(1, 1);
/// assert!(toast.rect(Rect::new(0, 0, 40, 8)).is_some());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Toast<'a> {
    message: &'a str,
    severity: Severity,
    kind: Option<ToastKind>,
    title: Option<&'a str>,
    anchor: Anchor,
    style: Option<Style>,
    horizontal_margin: u16,
    vertical_margin: u16,
    system: &'a DesignSystem,
    progress: Option<u8>,
    undo_label: Option<&'a str>,
    ascii: bool,
}

impl<'a> Toast<'a> {
    /// Creates a toast with default top-right anchoring and margins.
    #[must_use]
    pub const fn new(system: &'a DesignSystem, message: &'a str, severity: Severity) -> Self {
        Self {
            message,
            severity,
            kind: None,
            title: None,
            anchor: Anchor::TopRight,
            style: None,
            horizontal_margin: TOAST_DEFAULT_H_MARGIN,
            vertical_margin: TOAST_DEFAULT_V_MARGIN,
            system,
            progress: None,
            undo_label: None,
            ascii: false,
        }
    }

    /// Sets the corner used to anchor this content.
    #[must_use]
    pub const fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Sets horizontal and vertical margins in terminal cells.
    #[must_use]
    pub const fn margins(mut self, horizontal: u16, vertical: u16) -> Self {
        self.horizontal_margin = horizontal;
        self.vertical_margin = vertical;
        self
    }

    /// Overrides the theme-derived toast text style.
    #[must_use]
    pub const fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Optional title line.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Override kind (default maps from severity).
    #[must_use]
    pub const fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Progress percent.
    #[must_use]
    pub const fn progress(mut self, pct: u8) -> Self {
        self.progress = Some(if pct > 100 { 100 } else { pct });
        self.kind = Some(ToastKind::Progress);
        self
    }

    /// Undo label.
    #[must_use]
    pub const fn undo(mut self, label: &'a str) -> Self {
        self.undo_label = Some(label);
        self.kind = Some(ToastKind::Undo);
        self
    }

    /// ASCII chrome.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    fn resolved_kind(&self) -> ToastKind {
        self.kind
            .unwrap_or_else(|| ToastKind::from_severity(self.severity))
    }

    /// Returns the resolved outer toast rectangle.
    #[must_use]
    pub fn rect(&self, area: Rect) -> Option<Rect> {
        if area.is_empty() || self.message.is_empty() {
            return None;
        }
        // Classic single-line toast geometry (preserved for stories/docs).
        let (width, height) =
            if self.title.is_none() && self.progress.is_none() && self.undo_label.is_none() {
                let width = u16::try_from(display_cols(self.message).saturating_add(4))
                    .unwrap_or(u16::MAX)
                    .min(area.width);
                (width, 3.min(area.height))
            } else {
                let (w, h) = measure_toast_size(
                    self.title,
                    self.message,
                    self.undo_label.is_some(),
                    self.progress,
                );
                (w.min(area.width), h.min(area.height))
            };
        place_toast(
            area,
            width,
            height,
            self.anchor,
            self.horizontal_margin,
            self.vertical_margin,
            0,
            self.anchor.stacks_down(),
        )
    }

    /// Paint when host has already decided visibility.
    pub fn paint(&self, outer: Rect, buffer: &mut Buffer) {
        let Some(area) = self.rect(outer) else {
            return;
        };
        paint_one_toast(
            area,
            buffer,
            self.system,
            self.resolved_kind(),
            self.title,
            self.message,
            self.progress,
            self.undo_label,
            self.ascii,
            self.style,
        );
    }
}

impl Widget for &Toast<'_> {
    fn render(self, outer: Rect, buffer: &mut Buffer) {
        self.paint(outer, buffer);
    }
}

impl Widget for Toast<'_> {
    #[expect(
        clippy::needless_borrows_for_generic_args,
        reason = "explicitly delegate the owned contract to the borrowed renderer"
    )]
    fn render(self, area: Rect, buffer: &mut Buffer) {
        <&Self as Widget>::render(&self, area, buffer);
    }
}

// ── Toast stack widget ──────────────────────────────────────────────────────

/// Paints a [`ToastQueue`] as a non-focus-stealing stack.
#[derive(Debug, Clone, Copy)]
pub struct ToastStack<'a> {
    system: &'a DesignSystem,
    ascii: bool,
}

impl<'a> ToastStack<'a> {
    /// System.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self {
            system,
            ascii: false,
        }
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Paint queue; updates entry regions for hit-testing.
    pub fn paint(&self, outer: Rect, buffer: &mut Buffer, queue: &mut ToastQueue) {
        if outer.is_empty() {
            return;
        }
        let down = queue.anchor.stacks_down();
        for (i, entry) in queue.live.iter_mut().enumerate() {
            let (w, h) = measure_toast_size(
                entry.title.as_deref(),
                &entry.message,
                entry.undo_label.is_some(),
                entry.progress,
            );
            let h = if entry.title.is_none() && entry.progress.is_none() {
                3
            } else {
                h
            };
            let Some(area) = place_toast(
                outer,
                w.min(outer.width),
                h.min(outer.height),
                queue.anchor,
                queue.h_margin,
                queue.v_margin,
                i as u16,
                down,
            ) else {
                entry.region = None;
                continue;
            };
            // Skip if stacked out of bounds
            if area.bottom() > outer.bottom() || area.y < outer.y {
                entry.region = None;
                continue;
            }
            paint_one_toast(
                area,
                buffer,
                self.system,
                entry.kind,
                entry.title.as_deref(),
                &entry.message,
                entry.progress,
                entry.undo_label.as_deref(),
                self.ascii,
                None,
            );
            entry.region = Some(area);
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod state_tests {
    use super::*;

    fn tick(start: Instant, elapsed: Duration) -> FrameTick {
        FrameTick::manual(start + elapsed, elapsed, elapsed)
    }

    #[test]
    fn ttl_is_visible_before_deadline_and_expires_at_boundary() {
        let start = Instant::now();
        let mut state = ToastState::new(ToastLifetime::ExpiresAfter(Duration::from_secs(2)));
        state.show(tick(start, Duration::ZERO));

        assert!(state.is_visible(tick(start, Duration::from_millis(1_999))));
        assert!(!state.is_visible(tick(start, Duration::from_secs(2))));
        assert_eq!(
            state.next_deadline(),
            start.checked_add(Duration::from_secs(2))
        );
    }

    #[test]
    fn persistent_toast_stays_visible_until_dismissed() {
        let start = Instant::now();
        let mut state = ToastState::new(ToastLifetime::Persistent);
        state.show(tick(start, Duration::ZERO));

        assert!(state.is_visible(tick(start, Duration::from_secs(86_400))));
        assert_eq!(state.next_deadline(), None);
        state.dismiss();
        assert!(!state.is_visible(tick(start, Duration::ZERO)));
    }

    #[test]
    fn pause_freezes_ttl() {
        let start = Instant::now();
        let mut state = ToastState::new(ToastLifetime::ExpiresAfter(Duration::from_secs(2)));
        state.show(tick(start, Duration::ZERO));
        state.set_paused(true);
        assert!(state.is_visible(tick(start, Duration::from_secs(10))));
        state.set_paused(false);
        // After unpause, presence still advances from original show time
        assert!(!state.is_visible(tick(start, Duration::from_secs(10))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(start: Instant, elapsed: Duration) -> FrameTick {
        FrameTick::manual(start + elapsed, elapsed, Duration::ZERO)
    }

    #[test]
    fn anchors_and_margins_resolve_inside_the_outer_area() {
        let theme = crate::style::RolePalette::default();
        let system = crate::style::DesignSystem::from_palette(theme.clone());
        let outer = Rect::new(10, 5, 30, 12);
        let top_right = Toast::new(&system, "Saved", Severity::Success)
            .anchor(Anchor::TopRight)
            .margins(2, 1)
            .rect(outer)
            .expect("visible toast");
        let bottom_left = Toast::new(&system, "Saved", Severity::Success)
            .anchor(Anchor::BottomLeft)
            .margins(2, 1)
            .rect(outer)
            .expect("visible toast");

        assert_eq!(top_right, Rect::new(29, 6, 9, 3));
        assert_eq!(bottom_left, Rect::new(12, 13, 9, 3));
    }

    #[test]
    fn never_focusable() {
        assert!(!ToastState::new(ToastLifetime::Persistent).is_focusable());
    }

    #[test]
    fn queue_push_dedup_replace_archive() {
        let start = Instant::now();
        let t0 = tick(start, Duration::ZERO);
        let mut q = ToastQueue::new();
        q.set_max_visible(3);
        assert!(matches!(
            q.push(t0, ToastSpec::message("a", "one")),
            ToastOutcome::Shown { .. }
        ));
        assert!(matches!(
            q.push(t0, ToastSpec::message("b", "two").dedup_key("same")),
            ToastOutcome::Shown { .. }
        ));
        assert!(matches!(
            q.push(
                t0,
                ToastSpec::message("c", "dup").dedup_key("same")
            ),
            ToastOutcome::Deduplicated { id } if id == "b"
        ));
        assert!(matches!(
            q.push(t0, ToastSpec::message("a", "replaced").replace("a")),
            ToastOutcome::Replaced { .. }
        ));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn queue_ttl_archives_for_notification_center() {
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("x", "bye")
                .lifetime(ToastLifetime::ExpiresAfter(Duration::from_secs(1))),
        );
        let outs = q.advance(tick(start, Duration::from_secs(2)));
        assert!(
            outs.iter()
                .any(|o| matches!(o, ToastOutcome::Expired { .. }))
        );
        assert_eq!(q.missed_len(), 1);
        let drained = q.drain_missed();
        assert_eq!(drained[0].id, "x");
        assert_eq!(drained[0].reason, ToastArchiveReason::Expired);
        assert_eq!(q.missed_len(), 0);
    }

    #[test]
    fn queue_pause_and_progress_group() {
        let start = Instant::now();
        let t0 = tick(start, Duration::ZERO);
        let mut q = ToastQueue::new();
        let _ = q.push(
            t0,
            ToastSpec::message("p1", "10%").progress(10).group("job"),
        );
        assert!(matches!(
            q.push(
                t0,
                ToastSpec::message("p2", "50%").progress(50).group("job")
            ),
            ToastOutcome::Replaced { .. }
        ));
        assert_eq!(q.len(), 1);
        assert!(matches!(q.set_paused(true), ToastOutcome::Paused));
        assert!(q.is_paused());
        assert!(q.advance(tick(start, Duration::from_secs(9))).is_empty());
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn queue_evicts_low_priority_archives() {
        let start = Instant::now();
        let t0 = tick(start, Duration::ZERO);
        let mut q = ToastQueue::new();
        q.set_max_visible(2);
        let _ = q.push(
            t0,
            ToastSpec::message("low", "l").priority(ToastPriority::Low),
        );
        let _ = q.push(
            t0,
            ToastSpec::message("hi", "h").priority(ToastPriority::High),
        );
        let _ = q.push(
            t0,
            ToastSpec::message("n", "n").priority(ToastPriority::Normal),
        );
        assert!(q.len() <= 2);
        assert!(q.missed_len() >= 1);
    }

    #[test]
    fn stack_paint_and_announcement() {
        let system = DesignSystem::default();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("1", "Saved")
                .severity(Severity::Success)
                .announcement("Saved to disk"),
        );
        assert_eq!(q.latest_announcement(), Some("Saved to disk"));
        let area = Rect::new(0, 0, 40, 12);
        let mut buf = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buf, &mut q);
        assert!(q.live_ids().next().is_some());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Saved") || text.contains("+") || text.contains("✓"),
            "{text}"
        );
    }

    #[test]
    fn single_toast_kinds_paint() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 10);
        for (sev, kind) in [
            (Severity::Info, ToastKind::Info),
            (Severity::Success, ToastKind::Success),
            (Severity::Warning, ToastKind::Warning),
            (Severity::Error, ToastKind::Error),
        ] {
            let mut buf = Buffer::empty(area);
            Toast::new(&system, "msg", sev)
                .kind(kind)
                .paint(area, &mut buf);
        }
        let mut buf = Buffer::empty(area);
        Toast::new(&system, "working", Severity::Info)
            .progress(40)
            .paint(area, &mut buf);
        let mut buf = Buffer::empty(area);
        Toast::new(&system, "deleted", Severity::Success)
            .undo("Undo")
            .paint(area, &mut buf);
    }

    #[test]
    fn toast_border_is_muted_severity_on_icon_and_rail() {
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 24, 4);
        let mut buffer = Buffer::empty(area);
        paint_one_toast(
            area,
            &mut buffer,
            &system,
            ToastKind::Warning,
            Some("Warning"),
            "Check input",
            None,
            None,
            false,
            None,
        );
        assert_eq!(buffer[(0, 0)].fg, system.style(Role::Border).fg.unwrap());
        assert_eq!(buffer[(1, 1)].fg, system.style(Role::Warning).fg.unwrap());
    }

    #[test]
    fn default_ttl_constant() {
        assert_eq!(TOAST_DEFAULT_TTL, Duration::from_secs(4));
        assert!(matches!(
            ToastLifetime::default_ttl(),
            ToastLifetime::ExpiresAfter(d) if d == TOAST_DEFAULT_TTL
        ));
    }

    #[test]
    fn fuzz_queue_ops() {
        let start = Instant::now();
        let mut q = ToastQueue::new();
        q.set_max_visible(3);
        let mut seed = 11u64;
        for i in 0..100u64 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let t = tick(start, Duration::from_millis(i * 50));
            match seed % 5 {
                0 => {
                    let _ = q.push(
                        t,
                        ToastSpec::message(format!("id-{i}"), format!("m{i}")).priority(
                            if seed % 2 == 0 {
                                ToastPriority::Low
                            } else {
                                ToastPriority::High
                            },
                        ),
                    );
                }
                1 => {
                    let _ = q.push(
                        t,
                        ToastSpec::message(format!("p-{i}"), "prog")
                            .progress((seed % 100) as u8)
                            .group("g"),
                    );
                }
                2 => {
                    let _ = q.set_paused(seed % 2 == 0);
                }
                3 => {
                    let _ = q.advance(t);
                }
                _ => {
                    let id = q.live_ids().next().map(str::to_string);
                    if let Some(id) = id {
                        let _ = q.dismiss(&id);
                    }
                }
            }
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        for i in 0..4 {
            let _ = q.push(
                tick(start, Duration::ZERO),
                ToastSpec::message(format!("{i}"), format!("toast {i}")),
            );
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let t0 = std::time::Instant::now();
        for _ in 0..150 {
            terminal
                .draw(|f| {
                    ToastStack::new(&system).paint(f.area(), f.buffer_mut(), &mut q);
                })
                .unwrap();
        }
        assert!(t0.elapsed().as_millis() < 5_000);
    }

    #[test]
    fn pty_snapshot_stable() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let paint = || {
            let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
            terminal
                .draw(|f| {
                    Toast::new(&system, "Updated", Severity::Success)
                        .anchor(Anchor::TopRight)
                        .paint(f.area(), f.buffer_mut());
                })
                .unwrap();
            terminal
                .backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        };
        assert_eq!(paint(), paint());
    }
}
