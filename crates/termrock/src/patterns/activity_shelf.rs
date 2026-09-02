// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ActivityShelf** — compact persistent summary of active/blocked operations.
//!
//! **Mission.** Multiple concurrent tasks with status, elapsed time, actor,
//! progress, waiting reason, and jump/open actions. Prioritize blocked and
//! user-action-required items. Contract to one-line summary or badge in narrow
//! layouts. Do **not** duplicate full [`super::TaskRail`] (vertical task list).
//! Integrate with [`super::StatusBar`] and notifications via projection helpers.
//!
//! Research: agent activity indicators, build queues, IDE background tasks.
//!
//! Teaches: how to compose compact persistent summary of active/blocked
//! operations.
//!
//! Composes: [`crate::widgets::NotificationItem`],
//! [`crate::widgets::SemanticStatus`], [`crate::widgets::StatusKind`],
//! [`crate::widgets::StatusRegion`], [`crate::widgets::StatusSlot`],
//! [`crate::widgets::ToastKind`], [`crate::widgets::ToastPriority`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    style::{DesignSystem, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        NotificationItem, SemanticStatus, StatusIndicator, StatusKind, StatusRegion, StatusSlot,
        ToastKind, ToastPriority,
    },
};

/// Max chips painted before overflow (host may lower via layout).
pub const ACTIVITY_SHELF_CHIP_CAP: usize = 8;
/// Narrow width → summary/badge.
pub const ACTIVITY_SHELF_NARROW_WIDTH: u16 = 36;
/// Tiny width → badge only.
pub const ACTIVITY_SHELF_TINY_WIDTH: u16 = 18;
/// Default chip min columns.
pub const ACTIVITY_SHELF_CHIP_MIN_COLS: u16 = 6;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Coarse activity kind (product-neutral).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActivityKind {
    /// Generic / unknown.
    #[default]
    Generic,
    /// Tool invocation.
    Tool,
    /// Shell / terminal run.
    Shell,
    /// Search / index.
    Search,
    /// Build / compile / test.
    Build,
    /// Network / fetch.
    Network,
    /// Nested agent / subagent.
    Subagent,
}

impl ActivityKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Tool => "tool",
            Self::Shell => "shell",
            Self::Search => "search",
            Self::Build => "build",
            Self::Network => "network",
            Self::Subagent => "subagent",
        }
    }

    /// Compact letter (colorless / icons-only).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Generic => '·',
            Self::Tool => 'T',
            Self::Shell => '$',
            Self::Search => '/',
            Self::Build => 'B',
            Self::Network => 'N',
            Self::Subagent => 'A',
        }
    }
}

/// One concurrent activity (host-projected).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityItem {
    /// Stable id.
    pub id: String,
    /// Short title.
    pub title: String,
    /// Lifecycle status (shared vocabulary).
    pub status: SemanticStatus,
    /// Kind.
    pub kind: ActivityKind,
    /// Actor / provenance.
    pub actor: Option<String>,
    /// Elapsed display (host formats).
    pub elapsed: Option<String>,
    /// Progress 0–100 when known.
    pub progress: Option<u8>,
    /// Why waiting / blocked (input, lock, permission).
    pub waiting_reason: Option<String>,
    /// Blocked on external condition.
    pub blocked: bool,
    /// Needs user action (permission, input, conflict).
    pub action_required: bool,
    /// UI dismiss allowed (does not cancel process unless host maps it).
    pub dismissible: bool,
}

impl ActivityItem {
    /// Running generic activity.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: SemanticStatus::Running,
            kind: ActivityKind::Generic,
            actor: None,
            elapsed: None,
            progress: None,
            waiting_reason: None,
            blocked: false,
            action_required: false,
            dismissible: true,
        }
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: SemanticStatus) -> Self {
        self.status = s;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: ActivityKind) -> Self {
        self.kind = k;
        self
    }

    /// Actor.
    #[must_use]
    pub fn actor(mut self, a: impl Into<String>) -> Self {
        self.actor = Some(a.into());
        self
    }

    /// Elapsed.
    #[must_use]
    pub fn elapsed(mut self, e: impl Into<String>) -> Self {
        self.elapsed = Some(e.into());
        self
    }

    /// Progress.
    #[must_use]
    pub fn progress(mut self, p: u8) -> Self {
        self.progress = Some(p.min(100));
        self
    }

    /// Waiting reason.
    #[must_use]
    pub fn waiting_reason(mut self, r: impl Into<String>) -> Self {
        self.waiting_reason = Some(r.into());
        self
    }

    /// Blocked.
    #[must_use]
    pub const fn blocked(mut self, on: bool) -> Self {
        self.blocked = on;
        if on {
            self.status = SemanticStatus::Waiting;
        }
        self
    }

    /// Action required.
    #[must_use]
    pub const fn action_required(mut self, on: bool) -> Self {
        self.action_required = on;
        if on {
            self.status = SemanticStatus::Waiting;
        }
        self
    }

    /// Dismissible.
    #[must_use]
    pub const fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Sort rank (lower = more urgent / painted first).
    #[must_use]
    pub fn priority_rank(&self) -> u32 {
        let mut r = 1000u32;
        if self.action_required {
            r = r.saturating_sub(500);
        }
        if self.blocked {
            r = r.saturating_sub(300);
        }
        r = r.saturating_sub(match self.status {
            SemanticStatus::Failed => 200,
            SemanticStatus::Warning | SemanticStatus::Waiting => 150,
            SemanticStatus::Running => 100,
            SemanticStatus::Queued => 50,
            SemanticStatus::Paused => 40,
            SemanticStatus::Success => 10,
            _ => 0,
        });
        r
    }

    /// Chip label for current density.
    #[must_use]
    pub fn chip_label(&self, _ascii: bool, icons_only: bool, max_cols: usize) -> String {
        let semantic = activity_item_semantic(self);
        let g = semantic.glyph();
        let verb = activity_item_verb(self);
        if icons_only {
            return format!("| {g} {verb}");
        }
        let mut s = format!("| {g} {verb} · {}", self.title);
        if let Some(p) = self.progress {
            s.push_str(&format!(" {p}%"));
        } else if let Some(e) = &self.elapsed {
            s.push(' ');
            s.push_str(e);
        }
        take_display_cols(&s, max_cols)
    }

    /// One-line detail for summary.
    #[must_use]
    pub fn summary_fragment(&self) -> String {
        let mut s = self.title.clone();
        if self.action_required {
            s.push_str(" (action)");
        } else if self.blocked {
            s.push_str(" (blocked)");
        }
        if let Some(w) = &self.waiting_reason {
            s.push_str(": ");
            s.push_str(w);
        }
        s
    }
}

const fn activity_item_semantic(item: &ActivityItem) -> SemanticStatus {
    if matches!(item.status, SemanticStatus::Failed) {
        SemanticStatus::Failed
    } else if item.action_required {
        SemanticStatus::Warning
    } else if item.blocked {
        SemanticStatus::Waiting
    } else {
        item.status
    }
}

const fn activity_item_verb(item: &ActivityItem) -> &'static str {
    if matches!(item.status, SemanticStatus::Failed) {
        "failed"
    } else if item.action_required {
        "action required"
    } else if item.blocked {
        "blocked"
    } else {
        item.status.default_label()
    }
}

/// Sort items: action-required / blocked first, then status urgency, stable id.
#[must_use]
pub fn sort_activity_items(items: &[ActivityItem]) -> Vec<&ActivityItem> {
    let mut v: Vec<&ActivityItem> = items.iter().collect();
    v.sort_by(|a, b| {
        a.priority_rank()
            .cmp(&b.priority_rank())
            .then_with(|| a.id.cmp(&b.id))
    });
    v
}

/// Counts for summary/badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActivityCounts {
    /// Total items.
    pub total: usize,
    /// Running.
    pub running: usize,
    /// Blocked or action-required.
    pub blocked: usize,
    /// Action required only.
    pub action_required: usize,
    /// Failed.
    pub failed: usize,
    /// Queued.
    pub queued: usize,
}

/// Aggregate counts.
#[must_use]
pub fn activity_counts(items: &[ActivityItem]) -> ActivityCounts {
    let mut c = ActivityCounts {
        total: items.len(),
        ..ActivityCounts::default()
    };
    for i in items {
        if i.action_required {
            c.action_required += 1;
            c.blocked += 1;
        } else if i.blocked {
            c.blocked += 1;
        }
        match i.status {
            SemanticStatus::Running => c.running += 1,
            SemanticStatus::Failed => c.failed += 1,
            SemanticStatus::Queued => c.queued += 1,
            SemanticStatus::Waiting if !i.blocked && !i.action_required => c.blocked += 1,
            _ => {}
        }
    }
    c
}

/// One-line summary for narrow StatusBar / shelf.
#[must_use]
pub fn activity_status_summary(items: &[ActivityItem], _ascii: bool) -> String {
    let c = activity_counts(items);
    if c.total == 0 {
        return "∅ idle".into();
    }
    let text = activity_status_text(items);
    let g = if c.action_required > 0 || c.blocked > 0 {
        "⚠"
    } else {
        "◉"
    };
    format!("{g} {text}")
}

fn activity_status_text(items: &[ActivityItem]) -> String {
    let c = activity_counts(items);
    if c.total == 0 {
        return "idle".into();
    }
    let mut parts = Vec::new();
    if c.action_required > 0 {
        parts.push(format!("{} action", c.action_required));
    }
    if c.blocked > c.action_required {
        parts.push(format!(
            "{} blocked",
            c.blocked.saturating_sub(c.action_required)
        ));
    }
    if c.running > 0 {
        parts.push(format!("{} run", c.running));
    }
    if c.failed > 0 {
        parts.push(format!("{} fail", c.failed));
    }
    if c.queued > 0 {
        parts.push(format!("{} queue", c.queued));
    }
    if parts.is_empty() {
        parts.push(format!("{} active", c.total));
    }
    parts.join(" · ")
}

const fn activity_semantic(counts: ActivityCounts) -> SemanticStatus {
    if counts.action_required > 0 || counts.failed > 0 {
        SemanticStatus::Failed
    } else if counts.blocked > 0 {
        SemanticStatus::Waiting
    } else if counts.running > 0 {
        SemanticStatus::Running
    } else if counts.queued > 0 {
        SemanticStatus::Queued
    } else if counts.total == 0 {
        SemanticStatus::Idle
    } else {
        SemanticStatus::Success
    }
}

/// Tiny badge text (`!3` / `●3`).
#[must_use]
pub fn activity_badge_label(items: &[ActivityItem], _ascii: bool) -> String {
    let c = activity_counts(items);
    if c.total == 0 {
        return "·".into();
    }
    let g = if c.action_required > 0 || c.blocked > 0 {
        "!"
    } else {
        "●"
    };
    format!("{g}{}", c.total.min(99))
}

/// Layout plan: how many chips fit + overflow count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityShelfPlan {
    /// Presentation chosen for width.
    pub presentation: ActivityShelfPresentation,
    /// Indices into **sorted** item list that paint as chips.
    pub visible: Vec<usize>,
    /// Hidden count (overflow).
    pub overflow: usize,
    /// Chip column budget used.
    pub used_cols: u16,
}

/// Presentation density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActivityShelfPresentation {
    /// Glyph + title chips.
    #[default]
    Chips,
    /// Glyph + kind letter only.
    IconsOnly,
    /// One-line aggregate summary.
    Summary,
    /// Count badge only.
    Badge,
}

impl ActivityShelfPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Chips => "chips",
            Self::IconsOnly => "icons",
            Self::Summary => "summary",
            Self::Badge => "badge",
        }
    }

    /// Auto from width when host does not force.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < ACTIVITY_SHELF_TINY_WIDTH {
            Self::Badge
        } else if width < ACTIVITY_SHELF_NARROW_WIDTH {
            Self::Summary
        } else if width < 56 {
            Self::IconsOnly
        } else {
            Self::Chips
        }
    }
}

/// Strip orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActivityShelfOrientation {
    /// Horizontal chip row (default).
    #[default]
    Horizontal,
    /// Vertical thin column (east/west dock).
    Vertical,
}

impl ActivityShelfOrientation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

/// Plan visible chips for width (sorted items).
#[must_use]
pub fn plan_activity_shelf(
    sorted: &[&ActivityItem],
    width: u16,
    presentation: ActivityShelfPresentation,
    _ascii: bool,
) -> ActivityShelfPlan {
    match presentation {
        ActivityShelfPresentation::Badge | ActivityShelfPresentation::Summary => {
            ActivityShelfPlan {
                presentation,
                visible: Vec::new(),
                overflow: sorted.len(),
                used_cols: width,
            }
        }
        ActivityShelfPresentation::Chips | ActivityShelfPresentation::IconsOnly => {
            let icons = matches!(presentation, ActivityShelfPresentation::IconsOnly);
            let mut visible = Vec::new();
            let mut used = 0u16;
            let gap = 1u16;
            let overflow_reserve = 4u16; // "+N"
            let budget = width.saturating_sub(overflow_reserve);
            for (i, item) in sorted.iter().enumerate().take(ACTIVITY_SHELF_CHIP_CAP) {
                let max_c = if icons { 4 } else { 18 };
                let label = item.chip_label(false, icons, max_c);
                let w = (display_cols(&label) as u16).saturating_add(2); // padding
                let next = used
                    .saturating_add(if visible.is_empty() { 0 } else { gap })
                    .saturating_add(w);
                if next > budget && !visible.is_empty() {
                    break;
                }
                if w > budget && visible.is_empty() {
                    // force at least one truncated chip
                    visible.push(i);
                    used = budget.min(w);
                    break;
                }
                visible.push(i);
                used = next;
            }
            let overflow = sorted.len().saturating_sub(visible.len());
            ActivityShelfPlan {
                presentation,
                visible,
                overflow,
                used_cols: used,
            }
        }
    }
}

// ── StatusBar / Notification projection ─────────────────────────────────────

/// Owned summary strings for StatusBar slots (host paints via [`StatusSlot`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityStatusProjection {
    /// Compact summary content.
    pub summary: String,
    /// Summary words without a caller-painted glyph (for typed StatusSlot).
    pub summary_text: String,
    /// Badge content.
    pub badge: String,
    /// Badge count without a caller-painted glyph (for typed StatusSlot).
    pub badge_text: String,
    /// Priority (higher = keep under pressure).
    pub priority: u8,
    /// Suggested region.
    pub region: StatusRegion,
    /// Kind.
    pub kind: StatusKind,
    /// Aggregate lifecycle state driving the canonical glyph and tone.
    pub semantic: SemanticStatus,
}

/// Project activities → StatusBar slot content.
#[must_use]
pub fn project_activities_for_status_bar(
    items: &[ActivityItem],
    _ascii: bool,
) -> ActivityStatusProjection {
    let c = activity_counts(items);
    let priority = if c.action_required > 0 {
        95
    } else if c.blocked > 0 {
        90
    } else if c.running > 0 {
        75
    } else {
        55
    };
    ActivityStatusProjection {
        summary: activity_status_summary(items, false),
        summary_text: activity_status_text(items),
        badge: activity_badge_label(items, false),
        badge_text: c.total.to_string(),
        priority,
        region: StatusRegion::Right,
        kind: StatusKind::Transient,
        semantic: activity_semantic(c),
    }
}

/// Build a [`StatusSlot`] borrowing projection summary (id host-owned).
#[must_use]
pub fn activity_status_slot<'a, Id>(
    id: Id,
    projection: &'a ActivityStatusProjection,
    use_badge: bool,
) -> StatusSlot<'a, Id> {
    let content = if use_badge {
        projection.badge_text.as_str()
    } else {
        projection.summary_text.as_str()
    };
    StatusSlot::new(id, content)
        .kind(projection.kind)
        .semantic(projection.semantic)
        .priority(projection.priority)
        .region(projection.region)
        .min_width(if use_badge { 2 } else { 8 })
}

/// Map activity → notification history row.
#[must_use]
pub fn activity_to_notification(item: &ActivityItem, now_secs: u64) -> NotificationItem {
    let kind = if item.action_required || item.status == SemanticStatus::Failed {
        ToastKind::Error
    } else if item.blocked || item.status == SemanticStatus::Warning {
        ToastKind::Warning
    } else if item.status == SemanticStatus::Success {
        ToastKind::Success
    } else if item.progress.is_some() || item.status == SemanticStatus::Running {
        ToastKind::Progress
    } else {
        ToastKind::Info
    };
    let mut n = NotificationItem::new(item.id.clone(), item.summary_fragment(), kind)
        .title(item.title.clone());
    n.priority = if item.action_required {
        ToastPriority::High
    } else {
        ToastPriority::Normal
    };
    n.source = item.actor.clone();
    n.progress = item.progress;
    n.created_at_secs = now_secs;
    n.group_id = Some(format!("activity:{}", item.kind.id()));
    n.announcement = item.summary_fragment();
    if item.action_required {
        n.actions.push(("open".into(), "Open".into()));
    }
    n
}

/// Project all action-required / blocked items into notifications.
#[must_use]
pub fn activities_to_notifications(items: &[ActivityItem], now_secs: u64) -> Vec<NotificationItem> {
    sort_activity_items(items)
        .into_iter()
        .filter(|i| i.action_required || i.blocked || i.status == SemanticStatus::Failed)
        .map(|i| activity_to_notification(i, now_secs))
        .collect()
}

// ── Outcomes / state ────────────────────────────────────────────────────────

/// Shelf outcomes (requests / selection only).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActivityShelfOutcome {
    /// Ignored.
    Ignored,
    /// Selection moved.
    Selected {
        /// Item id.
        id: String,
    },
    /// Jump/open (host focuses task / tool card / thread).
    Activated {
        /// Item id.
        id: String,
    },
    /// Dismiss chip (UI-only unless host maps to cancel).
    Dismissed {
        /// Item id.
        id: String,
    },
    /// Overflow menu opened.
    OverflowOpen,
    /// Overflow closed.
    OverflowClosed,
    /// Presentation auto-contracted.
    PresentationChanged {
        /// Mode.
        presentation: ActivityShelfPresentation,
    },
}

/// Interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityShelfState {
    /// Selected index into **sorted** list.
    pub selected: usize,
    /// Overflow popover open (host may paint menu).
    pub overflow_open: bool,
    /// Forced presentation; None = auto from width.
    pub force_presentation: Option<ActivityShelfPresentation>,
    /// Orientation.
    pub orientation: ActivityShelfOrientation,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Chip hits (id, rect).
    pub hits: Vec<(String, Rect)>,
    /// Overflow control hit.
    pub overflow_hit: Rect,
    /// Last plan (for tests / host).
    pub last_plan: Option<ActivityShelfPlan>,
}

impl Default for ActivityShelfState {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivityShelfState {
    /// Default horizontal.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: 0,
            overflow_open: false,
            force_presentation: None,
            orientation: ActivityShelfOrientation::Horizontal,
            focused: true,
            accepts_input: true,
            hits: Vec::new(),
            overflow_hit: Rect::default(),
            last_plan: None,
        }
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    fn clamp_selected(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(len - 1);
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, items: &[ActivityItem]) -> ActivityShelfOutcome {
        if !self.accepts_input || !self.focused || key.kind != KeyEventKind::Press {
            return ActivityShelfOutcome::Ignored;
        }
        let sorted = sort_activity_items(items);
        self.clamp_selected(sorted.len());
        if sorted.is_empty() {
            return ActivityShelfOutcome::Ignored;
        }
        let horizontal = matches!(self.orientation, ActivityShelfOrientation::Horizontal);
        match key.code {
            KeyCode::Right | KeyCode::Char('l') if horizontal || key.code == KeyCode::Right => {
                self.selected = (self.selected + 1) % sorted.len();
                ActivityShelfOutcome::Selected {
                    id: sorted[self.selected].id.clone(),
                }
            }
            KeyCode::Left | KeyCode::Char('h') if horizontal || key.code == KeyCode::Left => {
                self.selected = if self.selected == 0 {
                    sorted.len() - 1
                } else {
                    self.selected - 1
                };
                ActivityShelfOutcome::Selected {
                    id: sorted[self.selected].id.clone(),
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !horizontal || key.code == KeyCode::Down => {
                self.selected = (self.selected + 1) % sorted.len();
                ActivityShelfOutcome::Selected {
                    id: sorted[self.selected].id.clone(),
                }
            }
            KeyCode::Up | KeyCode::Char('k') if !horizontal || key.code == KeyCode::Up => {
                self.selected = if self.selected == 0 {
                    sorted.len() - 1
                } else {
                    self.selected - 1
                };
                ActivityShelfOutcome::Selected {
                    id: sorted[self.selected].id.clone(),
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => ActivityShelfOutcome::Activated {
                id: sorted[self.selected].id.clone(),
            },
            KeyCode::Char('x') | KeyCode::Delete | KeyCode::Backspace => {
                let item = sorted[self.selected];
                if item.dismissible {
                    ActivityShelfOutcome::Dismissed {
                        id: item.id.clone(),
                    }
                } else {
                    ActivityShelfOutcome::Ignored
                }
            }
            KeyCode::Char('o') => {
                if self.overflow_open {
                    self.overflow_open = false;
                    ActivityShelfOutcome::OverflowClosed
                } else {
                    self.overflow_open = true;
                    ActivityShelfOutcome::OverflowOpen
                }
            }
            KeyCode::Esc if self.overflow_open => {
                self.overflow_open = false;
                ActivityShelfOutcome::OverflowClosed
            }
            _ => ActivityShelfOutcome::Ignored,
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        items: &[ActivityItem],
    ) -> ActivityShelfOutcome {
        if !self.accepts_input || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return ActivityShelfOutcome::Ignored;
        }
        if self.overflow_hit.width > 0 && self.overflow_hit.contains(event.position) {
            self.overflow_open = !self.overflow_open;
            return if self.overflow_open {
                ActivityShelfOutcome::OverflowOpen
            } else {
                ActivityShelfOutcome::OverflowClosed
            };
        }
        for (i, (id, rect)) in self.hits.iter().enumerate() {
            if rect.contains(event.position) {
                // map hit order to selection in sorted list by id
                let sorted = sort_activity_items(items);
                if let Some(idx) = sorted.iter().position(|it| it.id == *id) {
                    self.selected = idx;
                } else {
                    self.selected = i;
                }
                return ActivityShelfOutcome::Activated { id: id.clone() };
            }
        }
        ActivityShelfOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Compact activity shelf.
#[derive(Debug, Clone, Copy)]
pub struct ActivityShelf<'a> {
    items: &'a [ActivityItem],
    system: &'a DesignSystem,
    colorless: bool,
}

impl<'a> ActivityShelf<'a> {
    /// Items + system.
    #[must_use]
    pub const fn new(items: &'a [ActivityItem], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            colorless: false,
        }
    }

    /// ASCII glyphs.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ActivityShelfState) {
        state.hits.clear();
        state.overflow_hit = Rect::default();
        if area.is_empty() {
            state.last_plan = None;
            return;
        }
        let sorted = sort_activity_items(self.items);
        state.clamp_selected(sorted.len());

        let presentation = state
            .force_presentation
            .unwrap_or_else(|| ActivityShelfPresentation::for_width(area.width));
        let plan = plan_activity_shelf(&sorted, area.width, presentation, false);
        state.last_plan = Some(plan.clone());

        // fill muted bar background line
        let bar_style = if self.colorless {
            self.system.style(Role::TextMuted)
        } else {
            self.system.style(Role::StatusBar)
        };
        buffer.set_style(area, bar_style);

        match plan.presentation {
            ActivityShelfPresentation::Badge => {
                let counts = activity_counts(self.items);
                let label = if counts.total == 0 {
                    "idle".to_string()
                } else {
                    format!("{} activities", counts.total)
                };
                StatusIndicator::new(activity_semantic(counts), self.system)
                    .label(&label)
                    .colorless(self.colorless)
                    .strong(state.focused)
                    .paint(Rect::new(area.x, area.y, area.width, 1), buffer);
            }
            ActivityShelfPresentation::Summary => {
                let counts = activity_counts(self.items);
                let label = activity_status_text(self.items);
                StatusIndicator::new(activity_semantic(counts), self.system)
                    .label(&label)
                    .colorless(self.colorless)
                    .strong(state.focused)
                    .paint(Rect::new(area.x, area.y, area.width, 1), buffer);
            }
            ActivityShelfPresentation::Chips | ActivityShelfPresentation::IconsOnly => {
                self.paint_chips(area, buffer, state, &sorted, &plan, false);
            }
        }
    }

    fn paint_chips(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &mut ActivityShelfState,
        sorted: &[&ActivityItem],
        plan: &ActivityShelfPlan,
        _ascii: bool,
    ) {
        let icons = matches!(plan.presentation, ActivityShelfPresentation::IconsOnly);
        let vertical = matches!(state.orientation, ActivityShelfOrientation::Vertical);
        let mut x = area.x;
        let mut y = area.y;
        let max_c = if icons { 4 } else { 18 };

        for &vi in &plan.visible {
            let Some(item) = sorted.get(vi).copied() else {
                continue;
            };
            let label = item.chip_label(false, icons, max_c);
            let w = (display_cols(&label) as u16)
                .saturating_add(2)
                .max(ACTIVITY_SHELF_CHIP_MIN_COLS);
            if vertical {
                if y >= area.bottom() {
                    break;
                }
                let rect = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: 1,
                };
                let sel = state.focused && vi == state.selected;
                let style = chip_style(self.system, self.colorless, sel);
                self.system.paint_row(
                    buffer,
                    Rect::new(rect.x, rect.y, rect.width, 1),
                    &label,
                    style,
                );
                StatusIndicator::new(activity_item_semantic(item), self.system)
                    .label(activity_item_verb(item))
                    .colorless(self.colorless)
                    .paint(rect, buffer);
                state.hits.push((item.id.clone(), rect));
                y = y.saturating_add(1);
            } else {
                if x.saturating_add(w) > area.right() {
                    break;
                }
                let rect = Rect {
                    x,
                    y: area.y,
                    width: w.min(area.right().saturating_sub(x)),
                    height: 1,
                };
                let sel = state.focused && vi == state.selected;
                let style = chip_style(self.system, self.colorless, sel);
                let text = format!(" {label} ");
                self.system.paint_row(
                    buffer,
                    Rect::new(rect.x, rect.y, rect.width, 1),
                    &text,
                    style,
                );
                if rect.width > 1 {
                    StatusIndicator::new(activity_item_semantic(item), self.system)
                        .label(activity_item_verb(item))
                        .colorless(self.colorless)
                        .paint(
                            Rect::new(
                                rect.x.saturating_add(1),
                                rect.y,
                                rect.width.saturating_sub(1),
                                1,
                            ),
                            buffer,
                        );
                }
                state.hits.push((item.id.clone(), rect));
                x = x.saturating_add(rect.width).saturating_add(1);
            }
        }

        if plan.overflow > 0 {
            let ov = format!("+{}", plan.overflow);
            if vertical {
                if y < area.bottom() {
                    let rect = Rect {
                        x: area.x,
                        y,
                        width: area.width,
                        height: 1,
                    };
                    self.system.paint_row(
                        buffer,
                        Rect::new(rect.x, rect.y, rect.width, 1),
                        &ov,
                        self.system.style(Role::TextMuted),
                    );
                    state.overflow_hit = rect;
                }
            } else {
                let ow = (display_cols(&ov) as u16).saturating_add(1);
                if x.saturating_add(ow) <= area.right() {
                    let rect = Rect {
                        x,
                        y: area.y,
                        width: ow,
                        height: 1,
                    };
                    let st = if state.overflow_open {
                        self.system.style(Role::Focus)
                    } else {
                        self.system.style(Role::TextMuted)
                    };
                    self.system.paint_row(
                        buffer,
                        Rect::new(rect.x, rect.y, rect.width, 1),
                        &ov,
                        st,
                    );
                    state.overflow_hit = rect;
                }
            }
        }
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut ActivityShelfState) {
        self.paint(area, buffer, state);
    }
}

fn chip_style(
    system: &DesignSystem,
    colorless: bool,
    selected: bool,
) -> ratatui_core::style::Style {
    if colorless {
        if selected {
            // Mono selection is the explicit reversal pair (D5), not a swap
            // modifier over the idle face.
            return system.reversed();
        }
        return system.style(Role::Text);
    }
    let mut s = system.style(Role::Text);
    if selected {
        s = system.style(Role::Focus).add_modifier(Modifier::BOLD);
    }
    s
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Example concurrent activities.
#[must_use]
pub fn example_activities() -> Vec<ActivityItem> {
    vec![
        ActivityItem::new("a1", "permission: rm")
            .kind(ActivityKind::Shell)
            .action_required(true)
            .waiting_reason("awaiting allow")
            .actor("agent")
            .elapsed("12s"),
        ActivityItem::new("a2", "cargo test")
            .kind(ActivityKind::Build)
            .status(SemanticStatus::Running)
            .progress(64)
            .elapsed("1.2s")
            .actor("ci"),
        ActivityItem::new("a3", "fetch docs")
            .kind(ActivityKind::Network)
            .blocked(true)
            .waiting_reason("rate limit")
            .elapsed("30s"),
        ActivityItem::new("a4", "search index")
            .kind(ActivityKind::Search)
            .status(SemanticStatus::Queued)
            .actor("agent"),
        ActivityItem::new("a5", "subagent:review")
            .kind(ActivityKind::Subagent)
            .status(SemanticStatus::Running)
            .elapsed("4s"),
        ActivityItem::new("a6", "lint")
            .kind(ActivityKind::Tool)
            .status(SemanticStatus::Failed)
            .elapsed("0.4s"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Items.
    pub const ITEM_COUNT: usize = 48;
    /// Frames.
    pub const PAINT_FRAMES: u32 = 30;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    #[test]
    fn sort_prioritizes_action_and_blocked() {
        let items = example_activities();
        let s = sort_activity_items(&items);
        assert_eq!(s[0].id, "a1"); // action_required
        assert!(
            s.iter().position(|i| i.id == "a3").unwrap()
                < s.iter().position(|i| i.id == "a4").unwrap()
        );
    }

    #[test]
    fn narrow_contracts_to_summary_or_badge() {
        assert_eq!(
            ActivityShelfPresentation::for_width(10),
            ActivityShelfPresentation::Badge
        );
        assert_eq!(
            ActivityShelfPresentation::for_width(30),
            ActivityShelfPresentation::Summary
        );
        let items = example_activities();
        let sorted = sort_activity_items(&items);
        let p = plan_activity_shelf(&sorted, 20, ActivityShelfPresentation::Summary, true);
        assert!(p.visible.is_empty());
        assert_eq!(p.overflow, items.len());
        let badge = activity_badge_label(&items, true);
        assert!(badge.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn keyboard_nav_activate_dismiss() {
        let items = example_activities();
        let mut st = ActivityShelfState::new();
        let out = st.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items);
        assert!(matches!(out, ActivityShelfOutcome::Selected { .. }));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            ActivityShelfOutcome::Activated { .. }
        ));
        // select dismissible
        while !sort_activity_items(&items)[st.selected].dismissible {
            let _ = st.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &items);
        }
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
                &items
            ),
            ActivityShelfOutcome::Dismissed { .. }
        ));
    }

    #[test]
    fn overflow_open_chord() {
        let items = example_activities();
        let mut st = ActivityShelfState::new();
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
                &items
            ),
            ActivityShelfOutcome::OverflowOpen
        ));
        assert!(st.overflow_open);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &items),
            ActivityShelfOutcome::OverflowClosed
        ));
    }

    #[test]
    fn status_bar_projection() {
        let items = example_activities();
        let p = project_activities_for_status_bar(&items, true);
        assert!(p.summary.contains("action") || p.summary.contains("run"));
        assert!(p.priority >= 90);
        let slot = activity_status_slot("act", &p, false);
        assert_eq!(slot.region, StatusRegion::Right);
        assert!(!slot.content.is_empty());
    }

    #[test]
    fn notification_projection() {
        let items = example_activities();
        let n = activity_to_notification(&items[0], 1000);
        assert_eq!(n.kind, ToastKind::Error); // action_required
        assert!(!n.actions.is_empty());
        let many = activities_to_notifications(&items, 1000);
        assert!(!many.is_empty());
        assert!(many.iter().all(|m| m.kind == ToastKind::Error
            || m.kind == ToastKind::Warning
            || m.kind == ToastKind::Error));
    }

    #[test]
    fn paint_presentations() {
        let system = DesignSystem::default();
        let items = example_activities();
        let area = Rect::new(0, 0, 72, 1);
        let mut buf = Buffer::empty(area);
        for pres in [
            ActivityShelfPresentation::Chips,
            ActivityShelfPresentation::IconsOnly,
            ActivityShelfPresentation::Summary,
            ActivityShelfPresentation::Badge,
        ] {
            let mut st = ActivityShelfState::new();
            st.force_presentation = Some(pres);
            st.focused = true;
            ActivityShelf::new(&items, &system).paint(area, &mut buf, &mut st);
            assert!(st.last_plan.is_some());
        }
        // narrow auto
        let narrow = Rect::new(0, 0, 16, 1);
        let mut st = ActivityShelfState::new();
        ActivityShelf::new(&items, &system).paint(narrow, &mut buf, &mut st);
        assert_eq!(
            st.last_plan.as_ref().map(|p| p.presentation),
            Some(ActivityShelfPresentation::Badge)
        );
    }

    #[test]
    fn resize_cjk_combining_and_ascii_safe() {
        let system = DesignSystem::default();
        let label = "e\u{301} 本";
        let items = [ActivityItem::new("unicode", label).status(SemanticStatus::Running)];
        for _ in [false, true] {
            for width in [72, 30, 12, 1, 0] {
                let area = Rect::new(0, 0, width, 1);
                let mut buffer = Buffer::empty(area);
                let mut state = ActivityShelfState::new();
                if width == 72 {
                    state.force_presentation = Some(ActivityShelfPresentation::Chips);
                }
                ActivityShelf::new(&items, &system).paint(area, &mut buffer, &mut state);
                if width == 72 {
                    let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
                    assert!(text.contains("e\u{301}"), "{text:?}");
                    assert!(text.contains('本'), "{text:?}");
                }
            }
        }
    }

    #[test]
    fn mouse_activates_chip() {
        let system = DesignSystem::default();
        let items = example_activities();
        let mut st = ActivityShelfState::new();
        st.force_presentation = Some(ActivityShelfPresentation::Chips);
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        ActivityShelf::new(&items, &system).paint(area, &mut buf, &mut st);
        assert!(!st.hits.is_empty());
        let id = st.hits[0].0.clone();
        let rect = st.hits[0].1;
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(rect.x, rect.y),
            modifiers: KeyModifiers::NONE,
        };
        let out = st.handle_mouse(ev, &items);
        match out {
            ActivityShelfOutcome::Activated { id: aid } => assert_eq!(aid, id),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn not_task_rail_model() {
        // ActivityShelf must not own ListState / ListRow task-tree model.
        let src = include_str!("activity_shelf.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(!body.contains("ListState"));
        // `ListRowVisualState` is the shared row *recipe* input, not the list
        // model: the shelf resolves selection chrome from it (plans/010).
        assert!(!body.contains("ListRow<"));
        assert!(!body.contains("ListRow {"));
        assert!(!body.contains("std::process"));
        assert!(!body.contains("Command::new"));
        // Mentions of TaskRail in docs are OK; no List façade re-export.
        assert!(!body.contains("pub use") || !body.contains("List::"));
    }

    #[test]
    fn accepts_input_gate() {
        let items = example_activities();
        let mut st = ActivityShelfState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            ActivityShelfOutcome::Ignored
        ));
    }

    #[test]
    fn paint_perf_budget() {
        let system = DesignSystem::default();
        let mut items = example_activities();
        for i in 0..bench::ITEM_COUNT {
            items.push(
                ActivityItem::new(format!("x{i}"), format!("job {i}"))
                    .status(SemanticStatus::Running)
                    .progress((i % 100) as u8),
            );
        }
        let area = Rect::new(0, 0, 100, 1);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            let mut st = ActivityShelfState::new();
            ActivityShelf::new(&items, &system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_kinds_and_statuses() {
        for k in [
            ActivityKind::Generic,
            ActivityKind::Tool,
            ActivityKind::Shell,
            ActivityKind::Search,
            ActivityKind::Build,
            ActivityKind::Network,
            ActivityKind::Subagent,
        ] {
            assert!(!k.id().is_empty());
            let _ = k.letter();
        }
        for s in [
            SemanticStatus::Running,
            SemanticStatus::Waiting,
            SemanticStatus::Failed,
            SemanticStatus::Queued,
            SemanticStatus::Success,
        ] {
            let item = ActivityItem::new("i", "t").status(s);
            let _ = item.chip_label(true, false, 12);
            let _ = item.priority_rank();
        }
        assert!(bench::ITEM_COUNT >= 16);
    }

    #[test]
    fn empty_idle_summary() {
        let s = activity_status_summary(&[], true);
        assert!(s.contains("idle"));
        let b = activity_badge_label(&[], false);
        assert!(!b.is_empty());
    }

    #[test]
    fn vertical_orientation_paint() {
        let system = DesignSystem::default();
        let items = example_activities();
        let mut st = ActivityShelfState::new();
        st.orientation = ActivityShelfOrientation::Vertical;
        st.force_presentation = Some(ActivityShelfPresentation::Chips);
        let area = Rect::new(0, 0, 16, 8);
        let mut buf = Buffer::empty(area);
        ActivityShelf::new(&items, &system).paint(area, &mut buf, &mut st);
        assert!(!st.hits.is_empty());
    }
}
