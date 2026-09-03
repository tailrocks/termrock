// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **NotificationCenter** — persistent history and action surface for
//! application notifications.
//!
//! **Mission.** Desktop-style notification center for unread state, grouping,
//! filtering, timestamps, actions, progress, source, dismissal, and clear-all.
//! **Persistence is application-owned** — this module holds the in-memory view
//! model the host loads/saves. Integrates with [`super::Toast`] via
//! [`ToastArchive`] / [`ToastQueue::drain_missed`] without duplicating kind or
//! archive models.
//!
//! **Recipes.** [`NotificationRecipe::Drawer`] (edge panel) and
//! [`NotificationRecipe::FullPage`] (fills host area).
//!
//! **Focus.** When open and focused, list navigation is local; Esc closes.
//! Does not steal focus while closed. High-volume ingest uses dedup keys.
//!
//! Research: desktop notification centers, CI dashboards, task histories.

use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::{
        NavigationMove, OverlayId, OverlayOutcome, OverlaySize, OverlaySpec, OverlayStack,
        SemanticNode, SemanticRole, SemanticScene, SemanticState, UiIntent, default_list_intent,
    },
    style::{DesignSystem, Role},
    text::take_display_cols,
    widgets::{Hint, HintBar},
};

use super::drawer::DRAWER_DEFAULT_WIDTH;
use super::toast::{ToastArchive, ToastKind, ToastPriority, ToastQueue};

/// Overlay id for notification center drawer presentation.
pub const NOTIFICATION_CENTER_OVERLAY_ID: &str = "termrock.notification-center";
/// Default max retained items in memory (host may trim further for disk).
pub const NOTIFICATION_CENTER_DEFAULT_CAPACITY: usize = 500;
/// Formats an age in seconds the way a person reads it.
fn format_age_secs(secs: u64) -> String {
    match secs {
        0..=44 => "just now".to_string(),
        45..=5399 => format!("{}m ago", secs.div_ceil(60)),
        5400..=86_399 => format!("{}h ago", secs.div_ceil(3600)),
        _ => format!("{}d ago", secs.div_ceil(86_400)),
    }
}

/// Footer chords, painted through [`HintBar`].
///
/// One separator and one alignment rule for every overlay footer; the flat
/// sentence this replaced joined its chords by hand and picked its own
/// spacing under ASCII (plans/009 Step 1).
pub const NOTIFICATION_CENTER_HINTS: &[Hint<'static>] = &[
    Hint {
        chord: "j/k",
        label: "move",
        priority: 10,
        visible: true,
    },
    Hint {
        chord: "enter",
        label: "open",
        priority: 20,
        visible: true,
    },
    Hint {
        chord: "x",
        label: "dismiss",
        priority: 40,
        visible: true,
    },
    Hint {
        chord: "/",
        label: "filter",
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

// ── Models (shared kinds from Toast) ────────────────────────────────────────

/// One history row. Host owns durable storage; TermRock holds the live view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationItem {
    /// Stable id (toast id or app id).
    pub id: String,
    /// Kind (same enum as Toast).
    pub kind: ToastKind,
    /// Priority.
    pub priority: ToastPriority,
    /// Optional title.
    pub title: Option<String>,
    /// Body.
    pub message: String,
    /// Provenance (agent, pipeline, host).
    pub source: Option<String>,
    /// Group for collapse / filter.
    pub group_id: Option<String>,
    /// Progress 0–100 when kind is Progress.
    pub progress: Option<u8>,
    /// Unread flag.
    pub unread: bool,
    /// Host clock seconds (epoch or session offset — app-defined).
    pub created_at_secs: u64,
    /// Dedup key for high-volume collapse.
    pub dedup_key: Option<String>,
    /// Count of collapsed duplicates under this row.
    pub coalesce_count: u32,
    /// Action labels (id, label).
    pub actions: Vec<(String, String)>,
    /// Accessibility announcement snapshot.
    pub announcement: String,
}

impl NotificationItem {
    /// From a toast archive (missed/expired).
    #[must_use]
    pub fn from_archive(archive: ToastArchive, created_at_secs: u64) -> Self {
        let announcement = if archive.announcement.is_empty() {
            archive.message.clone()
        } else {
            archive.announcement
        };
        Self {
            id: archive.id,
            kind: archive.kind,
            priority: ToastPriority::Normal,
            title: archive.title,
            message: archive.message,
            source: None,
            group_id: None,
            progress: None,
            unread: true,
            created_at_secs,
            dedup_key: None,
            coalesce_count: 1,
            actions: Vec::new(),
            announcement,
        }
    }
    /// Minimal constructor.
    #[must_use]
    pub fn new(id: impl Into<String>, message: impl Into<String>, kind: ToastKind) -> Self {
        let message = message.into();
        Self {
            id: id.into(),
            kind,
            priority: ToastPriority::Normal,
            title: None,
            message: message.clone(),
            source: None,
            group_id: None,
            progress: None,
            unread: true,
            created_at_secs: 0,
            dedup_key: None,
            coalesce_count: 1,
            actions: Vec::new(),
            announcement: message,
        }
    }

    /// Source.
    #[must_use]
    pub fn source(mut self, s: impl Into<String>) -> Self {
        self.source = Some(s.into());
        self
    }

    /// Group.
    #[must_use]
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group_id = Some(g.into());
        self
    }

    /// Timestamp.
    #[must_use]
    pub const fn at(mut self, secs: u64) -> Self {
        self.created_at_secs = secs;
        self
    }

    /// Dedup key.
    #[must_use]
    pub fn dedup_key(mut self, k: impl Into<String>) -> Self {
        self.dedup_key = Some(k.into());
        self
    }

    /// Title.
    #[must_use]
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = Some(t.into());
        self
    }

    /// Progress.
    #[must_use]
    pub const fn progress(mut self, pct: u8) -> Self {
        self.kind = ToastKind::Progress;
        self.progress = Some(if pct > 100 { 100 } else { pct });
        self
    }

    /// Mark read/unread.
    #[must_use]
    pub const fn unread(mut self, on: bool) -> Self {
        self.unread = on;
        self
    }

    /// Action.
    #[must_use]
    pub fn action(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.actions.push((id.into(), label.into()));
        self
    }
}

/// Filter applied to the list (host may also pre-filter persistence).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum NotificationFilter {
    /// All items.
    #[default]
    All,
    /// Unread only.
    Unread,
    /// Kind filter.
    Kind(ToastKind),
    /// Group id.
    Group(String),
    /// Source substring match (case-sensitive host data).
    Source(String),
    /// Free-text query over title/message/source.
    Query(String),
}

impl NotificationFilter {
    /// Stable id.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Unread => "unread",
            Self::Kind(_) => "kind",
            Self::Group(_) => "group",
            Self::Source(_) => "source",
            Self::Query(_) => "query",
        }
    }

    fn matches(&self, item: &NotificationItem) -> bool {
        match self {
            Self::All => true,
            Self::Unread => item.unread,
            Self::Kind(k) => item.kind == *k,
            Self::Group(g) => item.group_id.as_ref() == Some(g),
            Self::Source(s) => item
                .source
                .as_ref()
                .is_some_and(|src| src.contains(s.as_str())),
            Self::Query(q) if q.is_empty() => true,
            Self::Query(q) => {
                let q = q.as_str();
                item.message.contains(q)
                    || item.title.as_ref().is_some_and(|t| t.contains(q))
                    || item.source.as_ref().is_some_and(|s| s.contains(q))
                    || item.group_id.as_ref().is_some_and(|g| g.contains(q))
            }
        }
    }
}

/// Presentation recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum NotificationRecipe {
    /// Edge drawer (default right).
    #[default]
    Drawer,
    /// Full host area (page / main pane).
    FullPage,
}

impl NotificationRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Drawer => "drawer",
            Self::FullPage => "full-page",
        }
    }
}

/// Slots after paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NotificationCenterSlots {
    /// Root.
    pub root: Rect,
    /// Title / unread badge.
    pub header: Rect,
    /// Filter strip.
    pub filter: Rect,
    /// List body.
    pub list: Rect,
    /// Footer / hints.
    pub footer: Rect,
}

impl NotificationCenterSlots {
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
            filter: Rect {
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
            footer: Rect {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
            },
        }
    }
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotificationCenterOutcome {
    /// No change.
    Ignored,
    /// Opened.
    Opened,
    /// Closed.
    Closed,
    /// Cursor moved.
    SelectionChanged {
        /// Selected id.
        id: Option<String>,
    },
    /// Item marked read.
    MarkedRead {
        /// Id.
        id: String,
    },
    /// All visible marked read.
    MarkedAllRead {
        /// Count.
        count: usize,
    },
    /// Item dismissed (removed from live view; host should delete from store).
    Dismissed {
        /// Id.
        id: String,
    },
    /// Clear all matching filter (or all).
    ClearAll {
        /// Removed count.
        count: usize,
    },
    /// Action activated.
    ActionActivated {
        /// Notification id.
        id: String,
        /// Action id.
        action: String,
    },
    /// Filter changed.
    FilterChanged,
    /// Items ingested.
    Ingested {
        /// New or updated count.
        count: usize,
    },
    /// Dedup coalesced into existing.
    Deduplicated {
        /// Surviving id.
        id: String,
    },
    /// Enter / open detail on selection.
    OpenItem {
        /// Id.
        id: String,
    },
}

// ── Overlay helpers ─────────────────────────────────────────────────────────

/// Open as right drawer on OverlayStack.
pub fn open_notification_center_drawer<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    depth: u16,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    let depth = depth.max(20).min(bounds.width.saturating_sub(2).max(20));
    let size = OverlaySize {
        width: depth,
        height: bounds.height.max(4),
        min_width: 18,
        min_height: 4,
        max_width: bounds.width,
        max_height: bounds.height,
    };
    stack.open(
        bounds,
        OverlaySpec::drawer(NOTIFICATION_CENTER_OVERLAY_ID, size, opener_focus),
    )
}

/// Open with dedicated overlay id (drawer or fullscreen recipe).
pub fn open_notification_center_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
    bounds: Rect,
    recipe: NotificationRecipe,
    opener_focus: Option<FocusId>,
) -> OverlayOutcome<FocusId> {
    match recipe {
        NotificationRecipe::Drawer => {
            let depth = DRAWER_DEFAULT_WIDTH
                .min(bounds.width.saturating_sub(2))
                .max(24.min(bounds.width));
            open_notification_center_drawer(stack, bounds, depth, opener_focus)
        }
        NotificationRecipe::FullPage => stack.open(
            bounds,
            OverlaySpec::fullscreen(NOTIFICATION_CENTER_OVERLAY_ID, opener_focus),
        ),
    }
}

/// Dismiss center overlay.
pub fn dismiss_notification_center_overlay<FocusId: Clone>(
    stack: &mut OverlayStack<FocusId>,
) -> OverlayOutcome<FocusId> {
    stack.dismiss(&OverlayId::from_static(NOTIFICATION_CENTER_OVERLAY_ID))
}

// ── State ───────────────────────────────────────────────────────────────────

/// Live notification center state (host loads/saves [`Self::items`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationCenterState {
    /// Host clock, when the host keeps it current.
    now_secs: Option<u64>,
    open: bool,
    focused: bool,
    accepts_input: bool,
    enabled: bool,
    recipe: NotificationRecipe,
    filter: NotificationFilter,
    /// Host-owned durable list (newest first recommended).
    items: Vec<NotificationItem>,
    /// Cursor into **filtered** view by item id.
    cursor: Option<String>,
    scroll: usize,
    /// Visible list rows, written by paint so event-time reveal matches.
    viewport_rows: usize,
    capacity: usize,
    slots: NotificationCenterSlots,
    /// Filter chrome: cycling presets.
    filter_cycle: usize,
    /// When true, clear-all / dismiss only apply to filtered view.
    scope_to_filter: bool,
}

impl Default for NotificationCenterState {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationCenterState {
    /// Closed empty center.
    #[must_use]
    pub fn new() -> Self {
        Self {
            now_secs: None,
            open: false,
            focused: false,
            accepts_input: true,
            enabled: true,
            recipe: NotificationRecipe::Drawer,
            filter: NotificationFilter::All,
            items: Vec::new(),
            cursor: None,
            scroll: 0,
            viewport_rows: 0,
            capacity: NOTIFICATION_CENTER_DEFAULT_CAPACITY,
            slots: NotificationCenterSlots::empty(),
            filter_cycle: 0,
            scope_to_filter: true,
        }
    }

    /// Whether open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Focused.
    #[must_use]
    pub const fn is_focused(&self) -> bool {
        self.focused
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(&self) -> NotificationRecipe {
        self.recipe
    }

    /// Filter.
    #[must_use]
    pub fn filter(&self) -> &NotificationFilter {
        &self.filter
    }

    /// All items (for host persistence).
    #[must_use]
    pub fn items(&self) -> &[NotificationItem] {
        &self.items
    }

    /// Replace entire list from host store (load).
    pub fn replace_items(&mut self, items: Vec<NotificationItem>) {
        self.items = items;
        self.trim_capacity();
        self.ensure_cursor();
    }
    /// Unread count (all items).
    #[must_use]
    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|i| i.unread).count()
    }

    /// Cursor id.
    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Slots.
    #[must_use]
    pub const fn slots(&self) -> NotificationCenterSlots {
        self.slots
    }

    /// Capacity.
    pub fn set_capacity(&mut self, n: usize) {
        self.capacity = n.max(1);
        self.trim_capacity();
    }

    /// Recipe.
    pub fn set_recipe(&mut self, recipe: NotificationRecipe) {
        self.recipe = recipe;
    }

    /// Focus.
    pub fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// ASCII.
    /// Open center.
    pub fn open(&mut self) -> NotificationCenterOutcome {
        if !self.enabled {
            return NotificationCenterOutcome::Ignored;
        }
        self.open = true;
        self.focused = true;
        self.ensure_cursor();
        NotificationCenterOutcome::Opened
    }

    /// Close.
    pub fn close(&mut self) -> NotificationCenterOutcome {
        if !self.open {
            return NotificationCenterOutcome::Ignored;
        }
        self.open = false;
        self.focused = false;
        NotificationCenterOutcome::Closed
    }

    /// Toggle.
    pub fn toggle(&mut self) -> NotificationCenterOutcome {
        if self.open { self.close() } else { self.open() }
    }

    /// Set filter.
    pub fn set_filter(&mut self, filter: NotificationFilter) -> NotificationCenterOutcome {
        self.filter = filter;
        self.scroll = 0;
        self.ensure_cursor();
        NotificationCenterOutcome::FilterChanged
    }

    /// Filtered indices into `items`.
    #[must_use]
    pub fn filtered_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, i)| self.filter.matches(i))
            .map(|(i, _)| i)
            .collect()
    }

    /// Filtered items borrow.
    pub fn filtered(&self) -> Vec<&NotificationItem> {
        self.filtered_indices()
            .into_iter()
            .filter_map(|i| self.items.get(i))
            .collect()
    }

    /// Tells the list what time it is, so rows can say "3m ago".
    ///
    /// Without it a row can only state the raw age it was given. TermRock has
    /// no clock of its own — the host owns time (plans/009 Step 6).
    pub const fn set_now_secs(&mut self, now_secs: u64) {
        self.now_secs = Some(now_secs);
    }

    /// Ingest archives from toast queue (NotificationCenter route).
    pub fn ingest_archives(
        &mut self,
        archives: impl IntoIterator<Item = ToastArchive>,
        now_secs: u64,
    ) -> NotificationCenterOutcome {
        let mut count = 0usize;
        let mut last_dedup = None;
        for a in archives {
            match self.ingest_item(NotificationItem::from_archive(a, now_secs)) {
                NotificationCenterOutcome::Ingested { count: c } => count += c,
                NotificationCenterOutcome::Deduplicated { id } => last_dedup = Some(id),
                _ => {}
            }
        }
        if count > 0 {
            NotificationCenterOutcome::Ingested { count }
        } else if let Some(id) = last_dedup {
            NotificationCenterOutcome::Deduplicated { id }
        } else {
            NotificationCenterOutcome::Ignored
        }
    }

    /// Drain toast queue missed into center.
    pub fn ingest_from_toast_queue(
        &mut self,
        queue: &mut ToastQueue,
        now_secs: u64,
    ) -> NotificationCenterOutcome {
        let missed = queue.drain_missed();
        self.ingest_archives(missed, now_secs)
    }

    /// Ingest one item with high-volume dedup.
    pub fn ingest_item(&mut self, item: NotificationItem) -> NotificationCenterOutcome {
        if let Some(ref key) = item.dedup_key {
            if let Some(existing) = self
                .items
                .iter_mut()
                .find(|i| i.dedup_key.as_ref() == Some(key))
            {
                existing.message = item.message;
                existing.title = item.title.or(existing.title.clone());
                existing.kind = item.kind;
                existing.progress = item.progress;
                existing.unread = true;
                existing.created_at_secs = item.created_at_secs;
                existing.coalesce_count = existing.coalesce_count.saturating_add(1);
                existing.announcement = item.announcement;
                let id = existing.id.clone();
                // Move to front
                if let Some(pos) = self.items.iter().position(|i| i.id == id) {
                    let row = self.items.remove(pos);
                    self.items.insert(0, row);
                }
                return NotificationCenterOutcome::Deduplicated { id };
            }
        }
        // Replace same id
        if let Some(pos) = self.items.iter().position(|i| i.id == item.id) {
            self.items.remove(pos);
        }
        let id = item.id.clone();
        self.items.insert(0, item);
        self.trim_capacity();
        if self.cursor.is_none() {
            self.cursor = Some(id);
        }
        NotificationCenterOutcome::Ingested { count: 1 }
    }

    fn trim_capacity(&mut self) {
        while self.items.len() > self.capacity {
            self.items.pop();
        }
    }

    fn ensure_cursor(&mut self) {
        let ids: Vec<String> = self
            .filtered_indices()
            .into_iter()
            .filter_map(|i| self.items.get(i).map(|it| it.id.clone()))
            .collect();
        if ids.is_empty() {
            self.cursor = None;
            self.scroll = 0;
            return;
        }
        if self
            .cursor
            .as_ref()
            .is_none_or(|c| !ids.iter().any(|id| id == c))
        {
            self.cursor = ids.first().cloned();
        }
        self.reveal_cursor();
    }

    fn reveal_cursor(&mut self) {
        let Some(ref c) = self.cursor else {
            return;
        };
        let ids: Vec<_> = self
            .filtered_indices()
            .into_iter()
            .filter_map(|i| self.items.get(i).map(|it| it.id.as_str()))
            .collect();
        let Some(idx) = ids.iter().position(|id| *id == c.as_str()) else {
            return;
        };
        // Paint writes the real viewport height; before the first paint the
        // reveal is a no-op and paint performs it with the real page size.
        self.reveal_index(idx, self.viewport_rows);
    }

    /// One reveal policy: scroll the minimal amount so row `idx` is visible
    /// within `page` rows. Shared by the event path and paint.
    fn reveal_index(&mut self, idx: usize, page: usize) {
        if page == 0 {
            return;
        }
        self.scroll = crate::scroll::cursor_follow_offset(idx, self.items.len(), page, self.scroll);
    }

    /// Mark one read.
    pub fn mark_read(&mut self, id: &str) -> NotificationCenterOutcome {
        if let Some(it) = self.items.iter_mut().find(|i| i.id == id) {
            if it.unread {
                it.unread = false;
                return NotificationCenterOutcome::MarkedRead { id: id.to_string() };
            }
        }
        NotificationCenterOutcome::Ignored
    }

    /// Mark all matching filter as read.
    pub fn mark_all_read(&mut self) -> NotificationCenterOutcome {
        let ids: Vec<String> = self
            .filtered_indices()
            .into_iter()
            .filter_map(|i| self.items.get(i).map(|it| it.id.clone()))
            .collect();
        let mut count = 0usize;
        for id in ids {
            if let Some(it) = self.items.iter_mut().find(|i| i.id == id) {
                if it.unread {
                    it.unread = false;
                    count += 1;
                }
            }
        }
        if count == 0 {
            NotificationCenterOutcome::Ignored
        } else {
            NotificationCenterOutcome::MarkedAllRead { count }
        }
    }

    /// Dismiss (remove) one.
    pub fn dismiss(&mut self, id: &str) -> NotificationCenterOutcome {
        let Some(pos) = self.items.iter().position(|i| i.id == id) else {
            return NotificationCenterOutcome::Ignored;
        };
        self.items.remove(pos);
        if self.cursor.as_deref() == Some(id) {
            self.cursor = None;
            self.ensure_cursor();
        }
        NotificationCenterOutcome::Dismissed { id: id.to_string() }
    }

    /// Clear matching filter (or all when filter is All / scope false).
    pub fn clear_all(&mut self) -> NotificationCenterOutcome {
        if !self.scope_to_filter || matches!(self.filter, NotificationFilter::All) {
            let count = self.items.len();
            self.items.clear();
            self.cursor = None;
            self.scroll = 0;
            return if count == 0 {
                NotificationCenterOutcome::Ignored
            } else {
                NotificationCenterOutcome::ClearAll { count }
            };
        }
        let remove: Vec<String> = self
            .filtered_indices()
            .into_iter()
            .filter_map(|i| self.items.get(i).map(|it| it.id.clone()))
            .collect();
        let count = remove.len();
        self.items.retain(|i| !remove.iter().any(|id| id == &i.id));
        self.cursor = None;
        self.ensure_cursor();
        if count == 0 {
            NotificationCenterOutcome::Ignored
        } else {
            NotificationCenterOutcome::ClearAll { count }
        }
    }

    /// Cycle filter presets: All → Unread → Error → Warning → All.
    pub fn cycle_filter(&mut self) -> NotificationCenterOutcome {
        self.filter_cycle = (self.filter_cycle + 1) % 4;
        let filter = match self.filter_cycle {
            0 => NotificationFilter::All,
            1 => NotificationFilter::Unread,
            2 => NotificationFilter::Kind(ToastKind::Error),
            _ => NotificationFilter::Kind(ToastKind::Warning),
        };
        self.set_filter(filter)
    }

    /// Move cursor.
    pub fn move_cursor(&mut self, delta: isize) -> NotificationCenterOutcome {
        let ids: Vec<String> = self
            .filtered_indices()
            .into_iter()
            .filter_map(|i| self.items.get(i).map(|it| it.id.clone()))
            .collect();
        if ids.is_empty() {
            self.cursor = None;
            return NotificationCenterOutcome::Ignored;
        }
        let cur = self
            .cursor
            .as_ref()
            .and_then(|c| ids.iter().position(|id| id == c))
            .unwrap_or(0);
        let next = if delta < 0 {
            cur.saturating_sub((-delta) as usize)
        } else {
            (cur + delta as usize).min(ids.len() - 1)
        };
        self.cursor = Some(ids[next].clone());
        self.reveal_cursor();
        NotificationCenterOutcome::SelectionChanged {
            id: self.cursor.clone(),
        }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> NotificationCenterOutcome {
        if !self.open || !self.enabled || !self.accepts_input || !self.focused {
            return NotificationCenterOutcome::Ignored;
        }
        if key.is_release() {
            return NotificationCenterOutcome::Ignored;
        }
        if !key.is_insert() {
            return NotificationCenterOutcome::Ignored;
        }
        let is_press = key.is_press();

        if matches!(key.code, KeyCode::Esc) && is_press && key.modifiers.is_empty() {
            return self.close();
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j' | 'J') => self.move_cursor(1),
            KeyCode::Up | KeyCode::Char('k' | 'K') => self.move_cursor(-1),
            KeyCode::Home => {
                let ids = self.filtered_indices();
                if let Some(i) = ids.first().and_then(|i| self.items.get(*i)) {
                    self.cursor = Some(i.id.clone());
                    self.scroll = 0;
                    NotificationCenterOutcome::SelectionChanged {
                        id: self.cursor.clone(),
                    }
                } else {
                    NotificationCenterOutcome::Ignored
                }
            }
            KeyCode::End => {
                let ids = self.filtered_indices();
                if let Some(i) = ids.last().and_then(|i| self.items.get(*i)) {
                    self.cursor = Some(i.id.clone());
                    self.reveal_cursor();
                    NotificationCenterOutcome::SelectionChanged {
                        id: self.cursor.clone(),
                    }
                } else {
                    NotificationCenterOutcome::Ignored
                }
            }
            KeyCode::Enter if is_press => {
                if let Some(id) = self.cursor.clone() {
                    let _ = self.mark_read(&id);
                    NotificationCenterOutcome::OpenItem { id }
                } else {
                    NotificationCenterOutcome::Ignored
                }
            }
            KeyCode::Char('u' | 'U') if is_press && key.modifiers.is_empty() => {
                if let Some(id) = self.cursor.clone() {
                    self.mark_read(&id)
                } else {
                    NotificationCenterOutcome::Ignored
                }
            }
            KeyCode::Char('U') if is_press && key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.mark_all_read()
            }
            KeyCode::Char('x' | 'X' | 'd' | 'D') if is_press && key.modifiers.is_empty() => {
                if let Some(id) = self.cursor.clone() {
                    self.dismiss(&id)
                } else {
                    NotificationCenterOutcome::Ignored
                }
            }
            KeyCode::Char('c' | 'C') if is_press && key.modifiers.is_empty() => self.clear_all(),
            KeyCode::Char('/' | 'f' | 'F') if is_press && key.modifiers.is_empty() => {
                self.cycle_filter()
            }
            KeyCode::Char('1') if is_press => {
                if let Some(id) = self.cursor.clone() {
                    if let Some(item) = self.items.iter().find(|i| i.id == id) {
                        if let Some((aid, _)) = item.actions.first() {
                            return NotificationCenterOutcome::ActionActivated {
                                id,
                                action: aid.clone(),
                            };
                        }
                    }
                }
                NotificationCenterOutcome::Ignored
            }
            _ => {
                if let Some(intent) = default_list_intent(key) {
                    return self.handle_intent(intent);
                }
                NotificationCenterOutcome::Ignored
            }
        }
    }

    /// Intent routing.
    pub fn handle_intent(&mut self, intent: UiIntent) -> NotificationCenterOutcome {
        if !self.open || !self.enabled || !self.accepts_input || !self.focused {
            return NotificationCenterOutcome::Ignored;
        }
        match intent {
            UiIntent::Cancel | UiIntent::Close => self.close(),
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down | NavigationMove::Right) => {
                self.move_cursor(1)
            }
            UiIntent::Move(
                NavigationMove::Previous | NavigationMove::Up | NavigationMove::Left,
            ) => self.move_cursor(-1),
            UiIntent::Move(NavigationMove::First) => {
                self.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE))
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE))
            }
            UiIntent::Activate | UiIntent::Submit => {
                if let Some(id) = self.cursor.clone() {
                    let _ = self.mark_read(&id);
                    NotificationCenterOutcome::OpenItem { id }
                } else {
                    NotificationCenterOutcome::Ignored
                }
            }
            _ => NotificationCenterOutcome::Ignored,
        }
    }

    /// Mouse select / dismiss.
    pub fn handle_mouse(&mut self, event: MouseEvent) -> NotificationCenterOutcome {
        if !self.open || !self.enabled || !self.accepts_input {
            return NotificationCenterOutcome::Ignored;
        }
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return NotificationCenterOutcome::Ignored;
        }
        let list = self.slots.list;
        if list.is_empty() || !list.contains(event.position) {
            return NotificationCenterOutcome::Ignored;
        }
        let row = event.position.y.saturating_sub(list.y) as usize;
        let ids: Vec<String> = self
            .filtered_indices()
            .into_iter()
            .skip(self.scroll)
            .filter_map(|i| self.items.get(i).map(|it| it.id.clone()))
            .collect();
        if let Some(id) = ids.get(row).cloned() {
            self.cursor = Some(id.clone());
            let _ = self.mark_read(&id);
            return NotificationCenterOutcome::SelectionChanged { id: Some(id) };
        }
        NotificationCenterOutcome::Ignored
    }

    /// Open on stack.
    pub fn open_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
        bounds: Rect,
        opener_focus: Option<F>,
    ) -> OverlayOutcome<F> {
        let _ = self.open();
        open_notification_center_overlay(stack, bounds, self.recipe, opener_focus)
    }

    /// Close + dismiss overlay.
    pub fn close_on_stack<F: Clone>(
        &mut self,
        stack: &mut OverlayStack<F>,
    ) -> (NotificationCenterOutcome, OverlayOutcome<F>) {
        let out = self.close();
        let stack_out = dismiss_notification_center_overlay(stack);
        (out, stack_out)
    }

    /// Sync open flag with stack.
    pub fn sync_with_stack<F>(&mut self, stack: &OverlayStack<F>) {
        let id = OverlayId::from_static(NOTIFICATION_CENTER_OVERLAY_ID);
        self.open = stack.contains(&id);
        if !self.open {
            self.focused = false;
        }
    }

    /// Accessibility: unread badge text.
    #[must_use]
    pub fn status_line(&self) -> String {
        let n = self.unread_count();
        if n == 0 {
            "notifications".into()
        } else {
            format!("notifications ({n} unread)")
        }
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Notification center paint.
#[derive(Debug, Clone, Copy)]
pub struct NotificationCenter<'a> {
    system: &'a DesignSystem,
    colorless: bool,
}

impl<'a> NotificationCenter<'a> {
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

    /// Paint when open.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut NotificationCenterState) {
        state.slots = NotificationCenterSlots::empty();
        if area.is_empty() || !state.open {
            return;
        }

        let panel = match state.recipe {
            NotificationRecipe::FullPage => area,
            NotificationRecipe::Drawer => {
                // Right strip within area
                let w = DRAWER_DEFAULT_WIDTH.min(area.width).max(20).min(area.width);
                Rect::new(area.right().saturating_sub(w), area.y, w, area.height)
            }
        };
        if panel.is_empty() {
            return;
        }
        state.slots.root = panel;

        let recipe = if state.focused {
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
        let inner = super::Surface::new(surface_system)
            .recipe(recipe)
            .bordered(true)
            .content_inset()
            .paint(panel, buffer);
        if inner.is_empty() {
            return;
        }

        let header_h = 1u16;
        let filter_h = 1u16;
        let footer_h = 1u16;
        let list_h = inner
            .height
            .saturating_sub(header_h + filter_h + footer_h)
            .max(1);

        let mut y = inner.y;
        // Header
        state.slots.header = Rect::new(inner.x, y, inner.width, header_h);
        let unread = state.unread_count();
        let title = if unread > 0 {
            format!("Notifications · {unread} unread")
        } else {
            "Notifications".into()
        };
        buffer.set_stringn(
            inner.x,
            y,
            take_display_cols(&title, usize::from(inner.width)).as_ref(),
            usize::from(inner.width),
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD),
        );
        y = y.saturating_add(1);

        // Filter
        state.slots.filter = Rect::new(inner.x, y, inner.width, filter_h);
        let filter_label = match &state.filter {
            NotificationFilter::All => "filter: all",
            NotificationFilter::Unread => "filter: unread",
            NotificationFilter::Kind(k) => match k {
                ToastKind::Error => "filter: error",
                ToastKind::Warning => "filter: warning",
                ToastKind::Info => "filter: info",
                ToastKind::Success => "filter: success",
                ToastKind::Progress => "filter: progress",
                ToastKind::Undo => "filter: undo",
            },
            NotificationFilter::Group(_) => "filter: group",
            NotificationFilter::Source(_) => "filter: source",
            NotificationFilter::Query(_) => "filter: query",
        };
        buffer.set_stringn(
            inner.x,
            y,
            take_display_cols(filter_label, usize::from(inner.width)).as_ref(),
            usize::from(inner.width),
            self.system.style(Role::TextMuted),
        );
        y = y.saturating_add(1);

        // List
        state.slots.list = Rect::new(inner.x, y, inner.width, list_h);
        let indices = state.filtered_indices();
        let page = list_h as usize;
        state.viewport_rows = page;
        // Clamp scroll
        if state.scroll >= indices.len() && !indices.is_empty() {
            state.scroll = indices.len().saturating_sub(1);
        }
        // Reveal cursor with the real page size
        if let Some(ref c) = state.cursor {
            if let Some(idx) = indices
                .iter()
                .position(|&i| state.items.get(i).is_some_and(|it| it.id == *c))
            {
                state.reveal_index(idx, page);
            }
        }

        if indices.is_empty() {
            super::EmptyState::new("No notifications", self.system).paint(
                Rect::new(inner.x, y, inner.width, 1),
                buffer,
                &mut super::EmptyStateState::new(),
            );
        } else {
            for (row, &item_idx) in indices.iter().skip(state.scroll).take(page).enumerate() {
                let Some(item) = state.items.get(item_idx) else {
                    continue;
                };
                let row_y = y.saturating_add(row as u16);
                if row_y >= y.saturating_add(list_h) {
                    break;
                }
                let selected = state.cursor.as_ref() == Some(&item.id);
                let glyph = { item.kind.glyph_unicode() };
                let unread_mark = if item.unread { "●" } else { " " };
                let coalesce = if item.coalesce_count > 1 {
                    format!(" ×{}", item.coalesce_count)
                } else {
                    String::new()
                };
                let primary = item.title.as_deref().unwrap_or(item.message.as_str());
                let mut line = format!("{unread_mark}{glyph} {primary}{coalesce}");
                if let Some(src) = &item.source {
                    line = format!("{line} · {src}");
                }
                if let Some(pct) = item.progress {
                    line = format!("{line} {pct}%");
                }
                // Relative when the host keeps a clock; otherwise the raw age
                // it was given, which is still a duration and not an epoch.
                let when = match state.now_secs {
                    Some(now) => format_age_secs(now.saturating_sub(item.created_at_secs)),
                    None => format!("{}s", item.created_at_secs),
                };
                line = format!("{line}  {when}");

                let style = if selected {
                    self.system
                        .style(Role::TextStrong)
                        .patch(self.system.style(Role::SelectionTint))
                } else if item.unread {
                    self.system.style(Role::Text)
                } else {
                    self.system.style(Role::TextMuted)
                };
                let tone = if self.colorless {
                    style
                } else if selected {
                    style
                } else {
                    // glyph uses kind color via prefix only
                    style
                };
                buffer.set_stringn(
                    inner.x,
                    row_y,
                    take_display_cols(&line, usize::from(inner.width)).as_ref(),
                    usize::from(inner.width),
                    if selected {
                        tone
                    } else {
                        // paint kind-colored first cell separately is hard; use text style
                        let _ = item.kind.role();
                        tone
                    },
                );
            }
        }
        y = y.saturating_add(list_h);

        // Footer
        state.slots.footer = Rect::new(inner.x, y, inner.width, footer_h);
        ratatui_core::widgets::Widget::render(
            &HintBar::new(NOTIFICATION_CENTER_HINTS, self.system),
            Rect::new(inner.x, y, inner.width, 1),
            buffer,
        );
    }

    /// Semantic registration.
    pub fn register_semantic<Sid, Act>(
        &self,
        scene: &mut SemanticScene<Sid, Act>,
        id: Sid,
        area: Rect,
        state: &NotificationCenterState,
    ) where
        Sid: Clone + PartialEq + std::fmt::Display,
        Act: Clone,
    {
        if area.is_empty() || !state.open {
            return;
        }
        let desc = format!(
            "notification-center recipe={} filter={} unread={} items={} cursor={}",
            state.recipe.id(),
            state.filter.id(),
            state.unread_count(),
            state.items.len(),
            state.cursor().unwrap_or("-"),
        );
        let _ = scene.register(
            SemanticNode::control(id, area)
                .role(SemanticRole::List)
                .label("notification-center")
                .description(desc)
                .focusable(state.focused && state.open)
                .state(SemanticState {
                    selected: state.focused,
                    expanded: state.open,
                    ..Default::default()
                }),
        );
    }
}

impl StatefulWidget for NotificationCenter<'_> {
    type State = NotificationCenterState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for &NotificationCenter<'_> {
    type State = NotificationCenterState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Example data ────────────────────────────────────────────────────────────

/// Demo notifications for stories.
#[must_use]
pub fn example_notifications(now_secs: u64) -> Vec<NotificationItem> {
    vec![
        NotificationItem::new("1", "Deploy failed on prod", ToastKind::Error)
            .title("Deploy")
            .source("pipeline #42")
            .at(now_secs.saturating_sub(30))
            .action("retry", "Retry")
            .unread(true),
        NotificationItem::new("2", "Agent finished step 3", ToastKind::Success)
            .source("agent")
            .group("agent-run")
            .at(now_secs.saturating_sub(120))
            .unread(true),
        NotificationItem::new("3", "Uploading artifacts", ToastKind::Progress)
            .progress(62)
            .group("upload")
            .at(now_secs.saturating_sub(10))
            .unread(true),
        NotificationItem::new("4", "Disk free space low", ToastKind::Warning)
            .source("host-a")
            .at(now_secs.saturating_sub(600))
            .unread(false),
        NotificationItem::new("5", "Draft deleted", ToastKind::Undo)
            .action("undo", "Undo")
            .at(now_secs.saturating_sub(5))
            .unread(true),
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyEventKind, KeyModifiers};
    use crate::runtime::FrameTick;
    use crate::widgets::tests::click;
    use crate::widgets::toast::ToastArchiveReason;
    use crate::widgets::toast::{ToastLifetime, ToastSpec};
    use std::time::{Duration, Instant};

    #[test]
    fn from_archive_reuses_toast_model() {
        let a = ToastArchive {
            id: "t1".into(),
            kind: ToastKind::Success,
            title: Some("OK".into()),
            message: "done".into(),
            reason: ToastArchiveReason::Expired,
            announcement: "done".into(),
        };
        let n = NotificationItem::from_archive(a, 100);
        assert_eq!(n.id, "t1");
        assert_eq!(n.kind, ToastKind::Success);
        assert!(n.unread);
        assert_eq!(n.created_at_secs, 100);
    }

    #[test]
    fn ingest_from_toast_queue() {
        let start = Instant::now();
        let tick = FrameTick::manual(start, Duration::ZERO, Duration::ZERO);
        let mut q = ToastQueue::new();
        let _ = q.push(
            tick,
            ToastSpec::message("x", "bye")
                .lifetime(ToastLifetime::ExpiresAfter(Duration::from_secs(1))),
        );
        let _ = q.advance(
            FrameTick::manual(
                start + Duration::from_secs(2),
                Duration::from_secs(2),
                Duration::ZERO,
            ),
            crate::style::MotionPolicy::Off,
        );
        assert_eq!(q.missed_len(), 1);
        let mut center = NotificationCenterState::new();
        let out = center.ingest_from_toast_queue(&mut q, 50);
        assert!(matches!(
            out,
            NotificationCenterOutcome::Ingested { count: 1 }
        ));
        assert_eq!(q.missed_len(), 0);
        assert_eq!(center.items().len(), 1);
        assert_eq!(center.unread_count(), 1);
    }

    #[test]
    fn high_volume_dedup_coalesces() {
        let mut s = NotificationCenterState::new();
        let _ = s.ingest_item(
            NotificationItem::new("a", "log line 1", ToastKind::Info).dedup_key("job-log"),
        );
        let out = s.ingest_item(
            NotificationItem::new("b", "log line 2", ToastKind::Info).dedup_key("job-log"),
        );
        assert!(matches!(
            out,
            NotificationCenterOutcome::Deduplicated { .. }
        ));
        assert_eq!(s.items().len(), 1);
        assert_eq!(s.items()[0].coalesce_count, 2);
        assert_eq!(s.items()[0].message, "log line 2");
    }

    #[test]
    fn filter_unread_and_clear() {
        let mut s = NotificationCenterState::new();
        s.replace_items(example_notifications(1000));
        assert!(s.unread_count() >= 1);
        let _ = s.set_filter(NotificationFilter::Unread);
        let filtered = s.filtered();
        assert!(filtered.iter().all(|i| i.unread));
        let _ = s.mark_all_read();
        assert_eq!(s.unread_count(), 0);
    }

    #[test]
    fn dismiss_and_clear_all() {
        let mut s = NotificationCenterState::new();
        s.replace_items(example_notifications(1));
        let id = s.items()[0].id.clone();
        assert!(matches!(
            s.dismiss(&id),
            NotificationCenterOutcome::Dismissed { .. }
        ));
        let n = s.items().len();
        let _ = s.set_filter(NotificationFilter::All);
        assert!(matches!(
            s.clear_all(),
            NotificationCenterOutcome::ClearAll { count } if count == n
        ));
        assert!(s.items().is_empty());
    }

    #[test]
    fn keyboard_nav_and_esc_close() {
        let mut s = NotificationCenterState::new();
        s.replace_items(example_notifications(1));
        let _ = s.open();
        assert!(s.is_open());
        let _ = s.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(s.cursor().is_some());
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            NotificationCenterOutcome::Closed
        ));
        assert!(!s.is_open());
    }

    #[test]
    fn repeated_one_shot_actions_are_ignored_but_navigation_repeats() {
        let mut s = NotificationCenterState::new();
        s.replace_items(vec![
            NotificationItem::new("1", "first", ToastKind::Info)
                .action("open", "Open")
                .unread(true),
            NotificationItem::new("2", "second", ToastKind::Warning).unread(true),
        ]);
        let _ = s.open();
        s.cursor = Some("1".into());

        for (code, modifiers) in [
            (KeyCode::Enter, KeyModifiers::NONE),
            (KeyCode::Esc, KeyModifiers::NONE),
            (KeyCode::Char('u'), KeyModifiers::NONE),
            (KeyCode::Char('U'), KeyModifiers::SHIFT),
            (KeyCode::Char('x'), KeyModifiers::NONE),
            (KeyCode::Char('c'), KeyModifiers::NONE),
            (KeyCode::Char('/'), KeyModifiers::NONE),
            (KeyCode::Char('1'), KeyModifiers::NONE),
        ] {
            let mut repeat = KeyEvent::new(code, modifiers);
            repeat.kind = KeyEventKind::Repeat;
            let before = s.clone();
            assert_eq!(s.handle_key(repeat), NotificationCenterOutcome::Ignored);
            assert_eq!(s, before, "{code:?} repeat mutated notification center");
        }

        let mut repeat_down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        repeat_down.kind = KeyEventKind::Repeat;
        assert_eq!(
            s.handle_key(repeat_down),
            NotificationCenterOutcome::SelectionChanged {
                id: Some("2".into())
            }
        );
        assert_eq!(s.cursor(), Some("2"));
    }

    #[test]
    fn recipes_paint_drawer_and_full() {
        let system = DesignSystem::default();
        let mut s = NotificationCenterState::new();
        s.replace_items(example_notifications(99));
        let _ = s.open();
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        s.set_recipe(NotificationRecipe::Drawer);
        NotificationCenter::new(&system).paint(area, &mut buf, &mut s);
        assert!(!s.slots.root.is_empty());
        assert!(s.slots.root.width < area.width || area.width < 30);

        s.set_recipe(NotificationRecipe::FullPage);
        let mut buf = Buffer::empty(area);
        NotificationCenter::new(&system).paint(area, &mut buf, &mut s);
        assert_eq!(s.slots.root, area);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Notifications") || text.contains("unread"),
            "{text}"
        );
    }

    #[test]
    fn persistence_is_host_replace() {
        let mut s = NotificationCenterState::new();
        s.replace_items(vec![NotificationItem::new("x", "m", ToastKind::Info)]);
        let snapshot = s.items().to_vec();
        let mut s2 = NotificationCenterState::new();
        s2.replace_items(snapshot);
        assert_eq!(s2.items().len(), 1);
        assert_eq!(s2.items()[0].id, "x");
    }

    #[test]
    fn capacity_trims_oldest() {
        let mut s = NotificationCenterState::new();
        s.set_capacity(3);
        for i in 0..5 {
            let _ = s.ingest_item(NotificationItem::new(format!("{i}"), "m", ToastKind::Info));
        }
        assert!(s.items().len() <= 3);
    }

    #[test]
    fn semantic_registers() {
        let system = DesignSystem::default();
        let mut s = NotificationCenterState::new();
        let _ = s.open();
        let mut scene = SemanticScene::<&str, ()>::default();
        NotificationCenter::new(&system).register_semantic(
            &mut scene,
            "nc",
            Rect::new(0, 0, 40, 20),
            &s,
        );
        assert!(
            scene
                .nodes()
                .iter()
                .any(|n| n.label.as_deref() == Some("notification-center"))
        );
    }

    #[test]
    fn overlay_open_close() {
        let bounds = Rect::new(0, 0, 80, 24);
        let mut stack = OverlayStack::<&str>::new();
        let mut s = NotificationCenterState::new();
        let _ = s.open_on_stack(&mut stack, bounds, Some("main"));
        assert!(stack.contains(&OverlayId::from_static(NOTIFICATION_CENTER_OVERLAY_ID)));
        let (out, _) = s.close_on_stack(&mut stack);
        assert!(matches!(out, NotificationCenterOutcome::Closed));
    }

    #[test]
    fn action_activation() {
        let mut s = NotificationCenterState::new();
        s.replace_items(vec![
            NotificationItem::new("1", "deleted", ToastKind::Undo)
                .action("undo", "Undo")
                .unread(true),
        ]);
        let _ = s.open();
        s.cursor = Some("1".into());
        assert!(matches!(
            s.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)),
            NotificationCenterOutcome::ActionActivated {
                id,
                action
            } if id == "1" && action == "undo"
        ));
    }

    #[test]
    fn mouse_row_hit_selects_and_marks_read() {
        let mut state = NotificationCenterState::new();
        state.replace_items(vec![
            NotificationItem::new("n1", "new", ToastKind::Info).unread(true),
        ]);
        let _ = state.open();
        state.slots.list = Rect::new(2, 3, 24, 4);
        assert_eq!(
            state.handle_mouse(click(2, 3)),
            NotificationCenterOutcome::SelectionChanged {
                id: Some("n1".into())
            }
        );
        assert!(!state.items()[0].unread);
    }

    #[test]
    fn fuzz_keys() {
        let mut s = NotificationCenterState::new();
        s.replace_items(example_notifications(1));
        let _ = s.open();
        let keys = [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Enter,
            KeyCode::Char('u'),
            KeyCode::Char('x'),
            KeyCode::Char('/'),
            KeyCode::Char('c'),
            KeyCode::Esc,
            KeyCode::Home,
            KeyCode::End,
        ];
        let mut seed = 5u64;
        for _ in 0..200 {
            if !s.is_open() {
                let _ = s.open();
                if s.items().is_empty() {
                    s.replace_items(example_notifications(1));
                }
            }
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let k = keys[(seed as usize) % keys.len()];
            let _ = s.handle_key(KeyEvent::new(k, KeyModifiers::NONE));
        }
    }

    #[test]
    fn paint_perf_smoke() {
        use ratatui_core::backend::TestBackend;
        use ratatui_core::terminal::Terminal;
        let system = DesignSystem::default();
        let mut s = NotificationCenterState::new();
        let mut items = Vec::new();
        for i in 0..80 {
            items.push(
                NotificationItem::new(format!("id-{i}"), format!("message {i}"), ToastKind::Info)
                    .at(i)
                    .unread(i % 3 == 0),
            );
        }
        s.replace_items(items);
        let _ = s.open();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            terminal
                .draw(|f| {
                    NotificationCenter::new(&system).paint(f.area(), f.buffer_mut(), &mut s);
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
        let paint = || {
            let mut terminal = Terminal::new(TestBackend::new(48, 16)).unwrap();
            let mut s = NotificationCenterState::new();
            s.replace_items(example_notifications(100));
            let _ = s.open();
            s.set_recipe(NotificationRecipe::FullPage);
            terminal
                .draw(|f| {
                    NotificationCenter::new(&system).paint(f.area(), f.buffer_mut(), &mut s);
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
