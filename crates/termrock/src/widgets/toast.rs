// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Toast** — transient footer status sentence (junie feedback grammar).
//!
//! **Mission.** Host-usable queue (`push` / `dismiss` / expire) that paints as
//! **one status sentence on the footer's right edge**, never as a stacked
//! overlay card. Quiet, timed, not focusable, does not steal the keyboard.
//!
//! Junie: “Feedback is quiet and timed. A status sentence on the footer's
//! right edge for 4–5 seconds… No toasts, no flashing.”
//!
//! **Paint.** One row. [`Role::TextSecondary`] body. Error: [`Role::Danger`] +
//! `!`. Warning: [`Role::Warning`] + `•`. Success: `✓` + secondary text (no
//! green fill). Info is secondary, never an accent swatch. Several live
//! notices collapse to that one sentence plus `N notices`.
//!
//! **Focus law.** Never focusable. Missed or expired items archive to
//! [`ToastQueue::drain_missed`] for [`super::NotificationCenter`].
use std::collections::VecDeque;
use std::time::Duration;

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    runtime::{FrameTick, Instant, Presence},
    style::{DesignSystem, MotionPolicy, Role},
    text::{display_cols, truncate_cols},
};

// ── Timing / retained host constants ────────────────────────────────────────

/// Default auto-dismiss TTL (junie footer status: ~4–5 seconds).
pub const TOAST_DEFAULT_TTL: Duration = Duration::from_secs(4);
/// Queue cap before eviction into the missed archive. Paint is always one row.
pub const TOAST_DEFAULT_MAX_VISIBLE: usize = 5;
/// No stack: footer status is one row. Always zero.
pub const TOAST_STACK_GAP: u16 = 0;
/// Default inset from the footer right (or left) edge.
pub const TOAST_DEFAULT_H_MARGIN: u16 = 0;
/// Default inset from the footer row (the last row of the host area).
pub const TOAST_DEFAULT_V_MARGIN: u16 = 0;

// ── Severity / Anchor / Lifetime ────────────────────────────────────────────

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

    /// Footer-status paint role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Error => Role::Danger,
            Self::Warning => Role::Warning,
            Self::Info | Self::Success => Role::TextSecondary,
        }
    }

    /// Marker painted before the sentence (`!` / `•` / `✓`; info is quiet).
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        self.glyph()
    }

    /// Marker painted before the sentence (`!` / `•` / `✓`; info is quiet).
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        self.glyph()
    }

    const fn glyph(self) -> &'static str {
        match self {
            Self::Info => "",
            Self::Success => "✓",
            Self::Warning => "•",
            Self::Error => "!",
        }
    }
}

/// Horizontal edge used to place the footer sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Anchor {
    /// Footer left edge.
    TopLeft,
    /// Footer right edge.
    TopRight,
    /// Footer left edge.
    BottomLeft,
    /// Footer right edge (junie).
    #[default]
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

    /// Whether the sentence sits on the right edge of the footer.
    #[must_use]
    pub const fn is_right(self) -> bool {
        matches!(self, Self::TopRight | Self::BottomRight)
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

    /// Footer-status paint role. Success is secondary text, not a green fill.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Error => Role::Danger,
            Self::Warning => Role::Warning,
            Self::Info | Self::Success | Self::Progress | Self::Undo => Role::TextSecondary,
        }
    }

    /// Marker painted before the sentence.
    #[must_use]
    pub const fn glyph_ascii(self) -> &'static str {
        self.glyph()
    }

    /// Marker painted before the sentence.
    #[must_use]
    pub const fn glyph_unicode(self) -> &'static str {
        self.glyph()
    }

    const fn glyph(self) -> &'static str {
        match self {
            Self::Info => "",
            Self::Success | Self::Undo => "✓",
            Self::Warning => "•",
            Self::Error => "!",
            Self::Progress => "…",
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
    /// Queue paused (host / unfocused window).
    Paused,
    /// Queue resumed.
    Resumed,
    /// Dedup dropped a push (still live).
    Deduplicated {
        /// Existing id.
        id: String,
    },
}

// ── Single ToastState ───────────────────────────────────────────────────────

/// Visibility and expiry state for a **single** transient notification.
///
/// Backed by [`Presence`] so TTL and deadlines share one motion primitive
/// (toasts are never focusable). No entrance or exit fade: junie feedback is
/// quiet and timed, not flashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastState {
    presence: Presence,
    lifetime: ToastLifetime,
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
        Self { presence, lifetime }
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
    }

    /// Pause TTL (e.g. terminal focus lost).
    pub fn set_paused(&mut self, tick: FrameTick, on: bool) {
        self.presence.set_paused(tick, on);
    }

    /// Paused?
    #[must_use]
    pub const fn is_paused(self) -> bool {
        self.presence.is_paused()
    }

    /// Advance TTL (call once per frame when shown). No-op while paused.
    pub fn advance(&mut self, tick: FrameTick, motion: MotionPolicy) {
        let _ = self.presence.advance(tick, motion);
    }

    /// When this toast became visible, if it is.
    #[must_use]
    pub const fn shown_at(self) -> Option<Instant> {
        match self.presence.phase() {
            crate::runtime::PresencePhase::Visible { since } => Some(since),
            _ => None,
        }
    }

    /// Returns whether the toast is visible at this frame.
    pub fn is_visible(&self, tick: FrameTick, motion: MotionPolicy) -> bool {
        let mut copy = *self;
        copy.advance(tick, motion);
        copy.presence.is_visible()
    }

    /// Returns the expiration deadline, or `None` when hidden or persistent.
    pub fn next_deadline(&self) -> Option<Instant> {
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
    /// Optional title (folded into the one sentence when set).
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
    announcement: String,
    archive_on_expire: bool,
    /// Last painted rect (hit testing).
    region: Option<Rect>,
    undo_label: Option<String>,
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
            anchor: Anchor::BottomRight,
            h_margin: TOAST_DEFAULT_H_MARGIN,
            v_margin: TOAST_DEFAULT_V_MARGIN,
            generation: 0,
        }
    }

    /// Max simultaneous queued notices (paint is still one sentence).
    pub fn set_max_visible(&mut self, n: usize) {
        self.max_visible = n.max(1);
    }

    /// Placement edge for the footer sentence.
    pub fn set_anchor(&mut self, anchor: Anchor) {
        self.anchor = anchor;
    }

    /// Insets from the footer edge.
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

    /// Pause all TTL clocks (terminal focus lost).
    pub fn set_paused(&mut self, tick: FrameTick, on: bool) -> ToastOutcome {
        if self.paused == on {
            return ToastOutcome::Ignored;
        }
        self.paused = on;
        for t in &mut self.live {
            t.state.set_paused(tick, on);
        }
        if on {
            ToastOutcome::Paused
        } else {
            ToastOutcome::Resumed
        }
    }

    /// Push a toast; handles replace, dedup, eviction. Latest becomes the
    /// visible footer sentence.
    pub fn push(&mut self, tick: FrameTick, spec: ToastSpec) -> ToastOutcome {
        self.generation = self.generation.saturating_add(1);

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
        if let Some(pos) = self.live.iter().position(|t| t.id == spec.id) {
            let prev = self.live.remove(pos).expect("pos");
            let id = spec.id.clone();
            self.insert_live(tick, spec);
            return ToastOutcome::Replaced {
                previous_id: prev.id,
                id,
            };
        }
        if let Some(ref key) = spec.dedup_key {
            if let Some(existing) = self.live.iter().find(|t| t.dedup_key.as_ref() == Some(key)) {
                return ToastOutcome::Deduplicated {
                    id: existing.id.clone(),
                };
            }
        }
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
        state.set_paused(tick, self.paused);
        let announcement = spec.announcement.unwrap_or_else(|| spec.message.clone());
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
            announcement,
            archive_on_expire: spec.archive_on_expire,
            region: None,
            undo_label: spec.undo_label,
        });
    }

    fn evict_if_needed(&mut self) {
        while self.live.len() > self.max_visible {
            let mut worst = self.live.len() - 1;
            for (i, t) in self.live.iter().enumerate().rev() {
                if t.priority < self.live[worst].priority {
                    worst = i;
                }
            }
            if let Some(t) = self.live.remove(worst) {
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

    /// Dismisses the newest live toast.
    pub fn dismiss_top(&mut self) -> ToastOutcome {
        let Some(id) = self.live.front().map(|t| t.id.clone()) else {
            return ToastOutcome::Ignored;
        };
        self.dismiss(&id)
    }

    /// Pauses or resumes every live toast's TTL.
    ///
    /// Hosts wire this to terminal focus (`FocusGained` / `FocusLost`).
    pub fn set_focus_paused(&mut self, tick: FrameTick, paused: bool) -> ToastOutcome {
        self.set_paused(tick, paused)
    }

    /// Activate undo / action for matching id (host maps hotkey).
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
    pub fn advance(&mut self, tick: FrameTick, motion: MotionPolicy) -> Vec<ToastOutcome> {
        let mut outs = Vec::new();
        if self.paused {
            return outs;
        }
        let mut i = 0;
        while i < self.live.len() {
            self.live[i].state.advance(tick, motion);
            if !self.live[i].state.is_visible(tick, motion) {
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

    /// Hit-test the painted footer sentence (call after paint).
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

// ── Sentence composition / paint ────────────────────────────────────────────

fn body_text(title: Option<&str>, message: &str, progress: Option<u8>) -> String {
    let mut body = match title {
        Some(t) if !t.is_empty() && message.is_empty() => t.to_string(),
        Some(t) if !t.is_empty() && t != message => format!("{t} · {message}"),
        _ => message.to_string(),
    };
    if let Some(pct) = progress {
        body = format!("{body} {pct}%");
    }
    body
}

fn status_sentence(
    kind: ToastKind,
    title: Option<&str>,
    message: &str,
    progress: Option<u8>,
    live_count: usize,
    undo_label: Option<&str>,
) -> String {
    let body = body_text(title, message, progress);
    let body = match undo_label {
        Some(label) if !label.is_empty() && body.is_empty() => label.to_string(),
        Some(label) if !label.is_empty() => format!("{label} · {body}"),
        _ => body,
    };
    let marker = kind.glyph();
    let mut sentence = if marker.is_empty() {
        body
    } else if body.is_empty() {
        marker.to_string()
    } else {
        format!("{marker} {body}")
    };
    if live_count > 1 {
        sentence = format!("{sentence} · {live_count} notices");
    }
    sentence
}

fn place_status_sentence(
    area: Rect,
    sentence: &str,
    anchor: Anchor,
    h_margin: u16,
    v_margin: u16,
    ellipsis: &str,
) -> Option<(Rect, String)> {
    if area.is_empty() || sentence.is_empty() {
        return None;
    }
    let y = area
        .bottom()
        .saturating_sub(1)
        .saturating_sub(v_margin)
        .max(area.y);
    if y >= area.bottom() {
        return None;
    }
    let budget = usize::from(area.width.saturating_sub(h_margin));
    if budget == 0 {
        return None;
    }
    let fitted = truncate_cols(sentence, budget, ellipsis).into_owned();
    let cols = u16::try_from(display_cols(&fitted)).unwrap_or(u16::MAX);
    if cols == 0 {
        return None;
    }
    let x = if anchor.is_right() {
        area.right()
            .saturating_sub(h_margin)
            .saturating_sub(cols)
            .max(area.x)
    } else {
        area.x
            .saturating_add(h_margin)
            .min(area.right().saturating_sub(cols).max(area.x))
    };
    let width = cols.min(area.width);
    Some((Rect::new(x, y, width, 1), fitted))
}

fn paint_status_sentence(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    kind: ToastKind,
    title: Option<&str>,
    message: &str,
    progress: Option<u8>,
    live_count: usize,
    undo_label: Option<&str>,
    anchor: Anchor,
    h_margin: u16,
    v_margin: u16,
) -> Option<Rect> {
    let sentence = status_sentence(kind, title, message, progress, live_count, undo_label);
    let (rect, fitted) = place_status_sentence(
        area,
        &sentence,
        anchor,
        h_margin,
        v_margin,
        system.glyphs.ellipsis(),
    )?;
    system.paint_row(buffer, rect, &fitted, system.style(kind.role()));
    Some(rect)
}

// ── Single Toast widget ─────────────────────────────────────────────────────

/// Transient footer status sentence with caller-owned lifetime and placement.
///
/// # Examples
///
/// ```
/// use ratatui_core::layout::Rect;
/// use termrock::style::DesignSystem;
/// use termrock::widgets::{Anchor, Severity, Toast};
///
/// let system = DesignSystem::junie();
/// let toast = Toast::new(&system, "Saved", Severity::Success)
///     .anchor(Anchor::BottomRight)
///     .margins(1, 0);
/// assert!(toast.rect(Rect::new(0, 0, 40, 8)).is_some());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Toast<'a> {
    message: &'a str,
    severity: Severity,
    kind: Option<ToastKind>,
    title: Option<&'a str>,
    anchor: Anchor,
    horizontal_margin: u16,
    vertical_margin: u16,
    system: &'a DesignSystem,
    progress: Option<u8>,
    undo_label: Option<&'a str>,
}

impl<'a> Toast<'a> {
    /// Creates a toast anchored to the footer right edge.
    #[must_use]
    pub const fn new(system: &'a DesignSystem, message: &'a str, severity: Severity) -> Self {
        Self {
            message,
            severity,
            kind: None,
            title: None,
            anchor: Anchor::BottomRight,
            horizontal_margin: TOAST_DEFAULT_H_MARGIN,
            vertical_margin: TOAST_DEFAULT_V_MARGIN,
            system,
            progress: None,
            undo_label: None,
        }
    }

    /// Sets the footer edge used to place this sentence.
    #[must_use]
    pub const fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Sets horizontal and vertical insets in terminal cells.
    #[must_use]
    pub const fn margins(mut self, horizontal: u16, vertical: u16) -> Self {
        self.horizontal_margin = horizontal;
        self.vertical_margin = vertical;
        self
    }

    /// Optional title folded into the one sentence.
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

    /// Undo label (kind Undo). The sentence stays one row; the host owns the
    /// hotkey that fires [`ToastQueue::activate_action`].
    #[must_use]
    pub const fn undo(mut self, label: &'a str) -> Self {
        self.undo_label = Some(label);
        self.kind = Some(ToastKind::Undo);
        self
    }

    fn resolved_kind(&self) -> ToastKind {
        self.kind
            .unwrap_or_else(|| ToastKind::from_severity(self.severity))
    }

    /// Returns the resolved one-row footer rectangle.
    #[must_use]
    pub fn rect(&self, area: Rect) -> Option<Rect> {
        let sentence = status_sentence(
            self.resolved_kind(),
            self.title,
            self.message,
            self.progress,
            1,
            self.undo_label,
        );
        place_status_sentence(
            area,
            &sentence,
            self.anchor,
            self.horizontal_margin,
            self.vertical_margin,
            self.system.glyphs.ellipsis(),
        )
        .map(|(rect, _)| rect)
    }

    /// Paint when host has already decided visibility.
    pub fn paint(&self, outer: Rect, buffer: &mut Buffer) {
        let _ = paint_status_sentence(
            outer,
            buffer,
            self.system,
            self.resolved_kind(),
            self.title,
            self.message,
            self.progress,
            1,
            self.undo_label,
            self.anchor,
            self.horizontal_margin,
            self.vertical_margin,
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

// ── Queue painter (one footer sentence, never a stack) ──────────────────────

/// Paints a [`ToastQueue`] as one footer status sentence. Latest wins; extra
/// live notices are named (`3 notices`) on that same row.
#[derive(Debug, Clone, Copy)]
pub struct ToastStack<'a> {
    system: &'a DesignSystem,
}

impl<'a> ToastStack<'a> {
    /// System.
    #[must_use]
    pub const fn new(system: &'a DesignSystem) -> Self {
        Self { system }
    }

    /// Paint the newest live notice as a footer sentence; records its region.
    pub fn paint(&self, outer: Rect, buffer: &mut Buffer, queue: &mut ToastQueue) {
        for entry in &mut queue.live {
            entry.region = None;
        }
        if outer.is_empty() {
            return;
        }
        let live_count = queue.live.len();
        let Some(entry) = queue.live.front_mut() else {
            return;
        };
        entry.region = paint_status_sentence(
            outer,
            buffer,
            self.system,
            entry.kind,
            entry.title.as_deref(),
            &entry.message,
            entry.progress,
            live_count,
            entry.undo_label.as_deref(),
            queue.anchor,
            queue.h_margin,
            queue.v_margin,
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::style::Color;

    fn tick(start: Instant, elapsed: Duration) -> FrameTick {
        FrameTick::manual(start + elapsed, elapsed, Duration::ZERO)
    }

    fn occupied_rows(buffer: &Buffer, area: Rect) -> Vec<u16> {
        (area.y..area.bottom())
            .filter(|&y| {
                (area.x..area.right()).any(|x| {
                    let sym = buffer[(x, y)].symbol();
                    !sym.is_empty() && sym != " "
                })
            })
            .collect()
    }

    fn row_text(buffer: &Buffer, area: Rect, y: u16) -> String {
        (area.x..area.right())
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect::<String>()
    }

    fn assert_no_rounded_frame(buffer: &Buffer) {
        for cell in buffer.content() {
            let s = cell.symbol();
            assert!(
                !matches!(
                    s,
                    "╭" | "╮"
                        | "╰"
                        | "╯"
                        | "┌"
                        | "┐"
                        | "└"
                        | "┘"
                        | "╔"
                        | "╗"
                        | "╚"
                        | "╝"
                ),
                "footer status must not paint overlay-card chrome, found {s:?}"
            );
        }
    }

    fn painted_cells<'a>(
        buffer: &'a Buffer,
        area: Rect,
    ) -> impl Iterator<Item = (u16, u16, &'a ratatui_core::buffer::Cell)> {
        (area.y..area.bottom()).flat_map(move |y| {
            (area.x..area.right()).filter_map(move |x| {
                let cell = &buffer[(x, y)];
                let sym = cell.symbol();
                if sym.is_empty() || sym == " " {
                    None
                } else {
                    Some((x, y, cell))
                }
            })
        })
    }

    #[test]
    fn never_focusable() {
        assert!(!ToastState::new(ToastLifetime::Persistent).is_focusable());
    }

    #[test]
    fn default_ttl_is_four_seconds() {
        assert_eq!(TOAST_DEFAULT_TTL, Duration::from_secs(4));
        assert!(matches!(
            ToastLifetime::default_ttl(),
            ToastLifetime::ExpiresAfter(d) if d == TOAST_DEFAULT_TTL
        ));
    }

    #[test]
    fn footer_sentence_sits_on_the_last_row_right_edge() {
        let system = DesignSystem::junie();
        let outer = Rect::new(0, 0, 40, 8);
        let toast = Toast::new(&system, "Saved", Severity::Success);
        let rect = toast.rect(outer).expect("visible status");
        assert_eq!(rect.height, 1);
        assert_eq!(rect.y, outer.bottom() - 1);
        assert_eq!(rect.right(), outer.right());
        assert!(rect.width >= 7, "✓ Saved must fit: {rect:?}");
    }

    #[test]
    fn push_error_paints_bang_and_message_in_danger_on_a_single_row() {
        let system = DesignSystem::junie();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("e", "deploy failed").severity(Severity::Error),
        );
        let area = Rect::new(0, 0, 48, 6);
        let mut buffer = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buffer, &mut q);

        let rows = occupied_rows(&buffer, area);
        assert_eq!(
            rows,
            vec![area.bottom() - 1],
            "one footer row, got {rows:?}"
        );
        let text = row_text(&buffer, area, rows[0]);
        assert!(text.contains('!'), "{text:?}");
        assert!(text.contains("deploy failed"), "{text:?}");
        assert_no_rounded_frame(&buffer);

        let danger = system.style(Role::Danger).fg.expect("danger fg");
        for (_, _, cell) in painted_cells(&buffer, area) {
            assert_eq!(cell.fg, danger, "error sentence must use Role::Danger");
            assert!(
                matches!(cell.bg, Color::Reset),
                "error must not fill a second surface, bg={:?}",
                cell.bg
            );
        }
    }

    #[test]
    fn push_success_paints_check_in_secondary_text_without_green_fill() {
        let system = DesignSystem::junie();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("s", "Saved").severity(Severity::Success),
        );
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buffer, &mut q);

        let rows = occupied_rows(&buffer, area);
        assert_eq!(rows.len(), 1);
        let text = row_text(&buffer, area, rows[0]);
        assert!(text.contains('✓'), "{text:?}");
        assert!(text.contains("Saved"), "{text:?}");
        assert_no_rounded_frame(&buffer);

        let secondary = system.style(Role::TextSecondary).fg.expect("secondary fg");
        let success = system.style(Role::Success).fg.expect("success fg");
        for (_, _, cell) in painted_cells(&buffer, area) {
            assert_eq!(
                cell.fg, secondary,
                "success is secondary text, not a green fill"
            );
            assert_ne!(cell.fg, success);
            assert!(
                matches!(cell.bg, Color::Reset),
                "success must not fill, bg={:?}",
                cell.bg
            );
        }
    }

    #[test]
    fn warning_uses_bullet_and_warning_role() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 40, 4);
        let mut buffer = Buffer::empty(area);
        Toast::new(&system, "disk low", Severity::Warning).paint(area, &mut buffer);
        let rows = occupied_rows(&buffer, area);
        assert_eq!(rows.len(), 1);
        let text = row_text(&buffer, area, rows[0]);
        assert!(text.contains('•'), "{text:?}");
        assert!(text.contains("disk low"), "{text:?}");
        let warning = system.style(Role::Warning).fg.expect("warning fg");
        for (_, _, cell) in painted_cells(&buffer, area) {
            assert_eq!(cell.fg, warning);
        }
    }

    #[test]
    fn info_is_text_secondary_never_accent() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 40, 4);
        let mut buffer = Buffer::empty(area);
        Toast::new(&system, "watching files", Severity::Info).paint(area, &mut buffer);
        let rows = occupied_rows(&buffer, area);
        assert_eq!(rows.len(), 1);
        let text = row_text(&buffer, area, rows[0]);
        assert!(text.contains("watching files"), "{text:?}");
        let secondary = system.style(Role::TextSecondary).fg.expect("secondary fg");
        let accent = system.style(Role::Accent).fg.expect("accent fg");
        for (_, _, cell) in painted_cells(&buffer, area) {
            assert_eq!(cell.fg, secondary);
            assert_ne!(cell.fg, accent, "Info is never an accent swatch");
        }
    }

    #[test]
    fn expiry_after_four_seconds_with_frame_tick() {
        let system = DesignSystem::junie();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("e", "boom").severity(Severity::Error),
        );
        let area = Rect::new(0, 0, 32, 4);
        let mut buffer = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buffer, &mut q);
        assert!(
            occupied_rows(&buffer, area)
                .iter()
                .any(|&y| row_text(&buffer, area, y).contains("boom"))
        );

        let still = tick(start, Duration::from_millis(3_999));
        assert!(
            q.advance(still, MotionPolicy::Off).is_empty(),
            "must hold until 4s"
        );
        assert_eq!(q.len(), 1);

        let gone = tick(start, TOAST_DEFAULT_TTL);
        let outs = q.advance(gone, MotionPolicy::Off);
        assert!(
            outs.iter()
                .any(|o| matches!(o, ToastOutcome::Expired { id } if id == "e")),
            "{outs:?}"
        );
        assert!(q.is_empty());

        let mut buffer = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buffer, &mut q);
        assert!(
            occupied_rows(&buffer, area).is_empty(),
            "expired status must not paint"
        );
        assert_no_rounded_frame(&buffer);
    }

    #[test]
    fn empty_paint_clears_stale_hit_regions() {
        let system = DesignSystem::junie();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("a", "Saved"),
        );
        let area = Rect::new(0, 0, 40, 5);
        let mut buffer = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buffer, &mut q);

        assert!((area.x..area.right()).any(|x| { q.region_at(x, area.bottom() - 1).is_some() }));

        ToastStack::new(&system).paint(Rect::default(), &mut buffer, &mut q);

        assert!((area.x..area.right()).all(|x| { q.region_at(x, area.bottom() - 1).is_none() }));
    }

    #[test]
    fn latest_wins_and_names_the_count_on_one_row() {
        let system = DesignSystem::junie();
        let start = Instant::now();
        let mut q = ToastQueue::new();
        let t0 = tick(start, Duration::ZERO);
        let _ = q.push(t0, ToastSpec::message("a", "one"));
        let _ = q.push(t0, ToastSpec::message("b", "two"));
        let _ = q.push(
            t0,
            ToastSpec::message("c", "three").severity(Severity::Error),
        );
        let area = Rect::new(0, 0, 48, 8);
        let mut buffer = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buffer, &mut q);
        let rows = occupied_rows(&buffer, area);
        assert_eq!(rows.len(), 1, "never a stack, got {rows:?}");
        let text = row_text(&buffer, area, rows[0]);
        assert!(text.contains("three"), "{text:?}");
        assert!(text.contains("3 notices"), "{text:?}");
        assert!(!text.contains("one"), "{text:?}");
        assert_no_rounded_frame(&buffer);
    }

    #[test]
    fn never_steals_keyboard() {
        assert!(!ToastState::new(ToastLifetime::Persistent).is_focusable());
        let start = Instant::now();
        let mut queue = ToastQueue::new();
        let _ = queue.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("a", "Saved"),
        );
        assert_eq!(queue.len(), 1);
        assert!(matches!(
            queue.dismiss("a"),
            ToastOutcome::Dismissed { ref id } if id == "a"
        ));
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
        let outs = q.advance(tick(start, Duration::from_secs(2)), MotionPolicy::Off);
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
        assert!(matches!(
            q.set_paused(tick(start, Duration::ZERO), true),
            ToastOutcome::Paused
        ));
        assert!(q.is_paused());
        assert!(
            q.advance(tick(start, Duration::from_secs(9)), MotionPolicy::Off)
                .is_empty()
        );
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
        let system = DesignSystem::junie();
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
        assert!(text.contains("Saved") && text.contains('✓'), "{text}");
        assert_eq!(occupied_rows(&buf, area).len(), 1);
        assert_no_rounded_frame(&buf);
    }

    #[test]
    fn single_toast_kinds_paint() {
        let system = DesignSystem::junie();
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
            assert_eq!(occupied_rows(&buf, area).len(), 1);
            assert_no_rounded_frame(&buf);
        }
        let mut buf = Buffer::empty(area);
        Toast::new(&system, "working", Severity::Info)
            .progress(40)
            .paint(area, &mut buf);
        let text = occupied_rows(&buf, area)
            .into_iter()
            .map(|y| row_text(&buf, area, y))
            .collect::<String>();
        assert!(text.contains("40%"), "{text}");
        let mut buf = Buffer::empty(area);
        Toast::new(&system, "deleted", Severity::Success)
            .undo("Undo")
            .paint(area, &mut buf);
        let text = occupied_rows(&buf, area)
            .into_iter()
            .map(|y| row_text(&buf, area, y))
            .collect::<String>();
        assert!(text.contains('✓'), "{text}");
        assert!(text.contains("Undo · deleted"), "{text}");
    }

    #[test]
    fn undo_label_is_in_the_footer_sentence() {
        let system = DesignSystem::junie();
        let area = Rect::new(0, 0, 48, 4);
        let mut buf = Buffer::empty(area);
        Toast::new(&system, "Deleted draft", Severity::Success)
            .undo("Undo")
            .paint(area, &mut buf);
        let text = occupied_rows(&buf, area)
            .into_iter()
            .map(|y| row_text(&buf, area, y))
            .collect::<String>();
        assert!(text.contains("Undo · Deleted draft"), "{text}");

        let start = Instant::now();
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("u", "Deleted draft").undo("Undo"),
        );
        let mut buf = Buffer::empty(area);
        ToastStack::new(&system).paint(area, &mut buf, &mut q);
        let text = occupied_rows(&buf, area)
            .into_iter()
            .map(|y| row_text(&buf, area, y))
            .collect::<String>();
        assert!(text.contains("Undo · Deleted draft"), "{text}");
    }

    #[test]
    fn ttl_is_visible_before_deadline_and_expires_at_boundary() {
        let start = Instant::now();
        let mut state = ToastState::new(ToastLifetime::ExpiresAfter(Duration::from_secs(2)));
        state.show(tick(start, Duration::ZERO));

        assert!(state.is_visible(tick(start, Duration::from_millis(1_999)), MotionPolicy::Off));
        assert!(!state.is_visible(tick(start, Duration::from_secs(2)), MotionPolicy::Off));
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

        assert!(state.is_visible(tick(start, Duration::from_secs(86_400)), MotionPolicy::Off));
        assert_eq!(state.next_deadline(), None);
        state.dismiss();
        assert!(!state.is_visible(tick(start, Duration::ZERO), MotionPolicy::Off));
    }

    #[test]
    fn pause_freezes_ttl_and_shifts_deadline() {
        let start = Instant::now();
        let mut state = ToastState::new(ToastLifetime::ExpiresAfter(Duration::from_secs(2)));
        state.show(tick(start, Duration::ZERO));
        state.set_paused(tick(start, Duration::from_secs(1)), true);
        assert!(state.is_paused());
        assert!(state.next_deadline().is_none());
        assert!(state.is_visible(tick(start, Duration::from_secs(10)), MotionPolicy::Off));

        state.set_paused(tick(start, Duration::from_secs(10)), false);
        assert!(!state.is_paused());
        assert_eq!(state.shown_at(), start.checked_add(Duration::from_secs(9)));
        assert_eq!(
            state.next_deadline(),
            start.checked_add(Duration::from_secs(11))
        );
        assert!(state.is_visible(tick(start, Duration::from_secs(10)), MotionPolicy::Off));
        state.advance(tick(start, Duration::from_secs(11)), MotionPolicy::Off);
        assert!(!state.is_visible(tick(start, Duration::from_secs(11)), MotionPolicy::Off));
    }

    #[test]
    fn queue_pause_freezes_existing_and_new_toasts() {
        let start = Instant::now();
        let lifetime = ToastLifetime::ExpiresAfter(Duration::from_secs(2));
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick(start, Duration::ZERO),
            ToastSpec::message("old", "old").lifetime(lifetime),
        );
        assert!(matches!(
            q.set_paused(tick(start, Duration::from_secs(1)), true),
            ToastOutcome::Paused
        ));
        let _ = q.push(
            tick(start, Duration::from_secs(9)),
            ToastSpec::message("new", "new").lifetime(lifetime),
        );

        assert!(q.next_deadline().is_none());
        assert!(
            q.advance(tick(start, Duration::from_secs(10)), MotionPolicy::Off)
                .is_empty()
        );
        assert!(matches!(
            q.set_paused(tick(start, Duration::from_secs(10)), false),
            ToastOutcome::Resumed
        ));
        assert_eq!(
            q.next_deadline(),
            start.checked_add(Duration::from_secs(11))
        );
        let old_expired = q.advance(tick(start, Duration::from_secs(11)), MotionPolicy::Off);
        assert!(
            old_expired
                .iter()
                .any(|out| matches!(out, ToastOutcome::Expired { id } if id == "old"))
        );
        assert_eq!(q.len(), 1);
        let new_expired = q.advance(tick(start, Duration::from_secs(12)), MotionPolicy::Off);
        assert!(
            new_expired
                .iter()
                .any(|out| matches!(out, ToastOutcome::Expired { id } if id == "new"))
        );
        assert!(q.is_empty());
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
                    let _ = q.set_paused(t, seed % 2 == 0);
                }
                3 => {
                    let _ = q.advance(t, MotionPolicy::Off);
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
        let system = DesignSystem::junie();
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
        let system = DesignSystem::junie();
        let paint = || {
            let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
            terminal
                .draw(|f| {
                    Toast::new(&system, "Updated", Severity::Success)
                        .anchor(Anchor::BottomRight)
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
