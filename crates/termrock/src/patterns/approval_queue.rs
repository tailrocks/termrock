// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **ApprovalQueue** — unified surface for pending permissions, questions,
//! plans, diffs, and other human decisions.
//!
//! **Mission.** Priority, type, source actor, age, blocking status, summary,
//! preview; open, approve **where safe**, defer, dismiss/cancel. **Never**
//! reduce high-risk approvals to one-click bulk actions. Preserve protocol
//! order when required. Compact badge, drawer, and full view. Projects into
//! [`super::NotificationCenter`] and [`super::TaskRail`] / ActivityModel.
//!
//! **vs [`super::PermissionPrompt`] / `PermissionQueue`.** Those own the full
//! trust-gate interaction for one permission; ApprovalQueue is the multi-type
//! inbox that *opens* the right surface. High-risk items only emit `Open` —
//! never silent bulk grant.
//! **vs [`super::QuestionFlow`] / [`super::PlanReview`] / DiffReview.** Full
//! HITL UIs; queue holds pointers + summaries until opened.
//!
//! Research: agent approval flows, code review queues, security request
//! dashboards.
//!
//! Teaches: how to compose unified surface for pending permissions,
//! questions, plans, diffs, and other human decisions.
//!
//! Composes: [`crate::widgets::NotificationItem`], [`crate::widgets::Panel`],
//! [`crate::widgets::PermissionRisk`], [`crate::widgets::SemanticStatus`],
//! [`crate::widgets::StatefulWidget`], [`crate::widgets::ToastKind`],
//! [`crate::widgets::ToastPriority`], [`crate::widgets::Widget`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect, style::Modifier, widgets::StatefulWidget};

use crate::{
    input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    patterns::ActivityKind,
    patterns::task_rail::{ActivityModel, ActivityScope},
    style::{DesignSystem, ListRowVisualState, PanelChrome, Role},
    text::display_cols,
    widgets::NotificationItem,
    widgets::Panel,
    widgets::PermissionRisk,
    widgets::SemanticStatus,
    widgets::StatusIndicator,
    widgets::ToastKind,
    widgets::ToastPriority,
    widgets::{EmptyKind, EmptyState},
};

/// Overlay / drawer id.
pub const APPROVAL_QUEUE_OVERLAY_ID: &str = "termrock.approval_queue";
/// Drawer overlay helper id.
pub const APPROVAL_QUEUE_DRAWER_OVERLAY_ID: &str = "termrock.approval_queue_drawer";
/// Max rows in full view window.
pub const APPROVAL_QUEUE_WINDOW: usize = 48;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Kind of human decision pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ApprovalKind {
    /// Permission / trust gate.
    #[default]
    Permission,
    /// Question flow / interview.
    Question,
    /// Plan review.
    Plan,
    /// Diff / patch review.
    Diff,
    /// Other product-neutral decision.
    Other,
}

impl ApprovalKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Plan => "plan",
            Self::Diff => "diff",
            Self::Other => "other",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Plan => "plan",
            Self::Diff => "diff",
            Self::Other => "decision",
        }
    }

    /// Glyph.
    #[must_use]
    pub const fn glyph(self, ascii: bool) -> &'static str {
        if ascii {
            return match self {
                Self::Permission => "!",
                Self::Question => "?",
                Self::Plan => "P",
                Self::Diff => "D",
                Self::Other => "*",
            };
        }
        match self {
            // One column, not two: an emoji here jittered every column to
            // its right (plans/013 Step 2).
            Self::Permission => "⚿",
            Self::Question => "?",
            Self::Plan => "☰",
            Self::Diff => "±",
            Self::Other => "·",
        }
    }

    /// Whether this kind is protocol-FIFO by default (order preserved).
    #[must_use]
    pub const fn protocol_ordered_default(self) -> bool {
        matches!(self, Self::Permission | Self::Question)
    }
}

/// Whether item blocks agent progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ApprovalBlocking {
    /// Does not block other work.
    NonBlocking,
    /// Blocks the issuing agent turn.
    #[default]
    Blocking,
    /// Hard gate — no progress until resolved (permissions often).
    HardGate,
}

impl ApprovalBlocking {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NonBlocking => "non_blocking",
            Self::Blocking => "blocking",
            Self::HardGate => "hard_gate",
        }
    }

    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NonBlocking => "non-blocking",
            Self::Blocking => "blocking",
            Self::HardGate => "hard gate",
        }
    }

    /// Priority boost for sort.
    #[must_use]
    pub const fn boost(self) -> u8 {
        match self {
            Self::HardGate => 40,
            Self::Blocking => 20,
            Self::NonBlocking => 0,
        }
    }
}

/// One pending decision (host-projected; no policy execution).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalItem {
    /// Stable id.
    pub id: String,
    /// Kind.
    pub kind: ApprovalKind,
    /// Risk (drives safe-approve gates).
    pub risk: PermissionRisk,
    /// Base priority 0–100 (higher first when not protocol-locked).
    pub priority: u8,
    /// Source actor (`agent`, `subagent:x`, `mcp:y`).
    pub actor: Option<String>,
    /// Age label (`2m`, `just now`).
    pub age: Option<String>,
    /// Blocking class.
    pub blocking: ApprovalBlocking,
    /// Short summary.
    pub summary: String,
    /// Optional preview line.
    pub preview: Option<String>,
    /// Generation for stale protection (host/protocol).
    pub generation: u64,
    /// Must preserve relative FIFO among protocol_ordered peers.
    pub protocol_ordered: bool,
    /// Host allows single-item quick approve (still blocked if risk high).
    pub host_allows_quick_approve: bool,
    /// Deferred by user (still pending).
    pub deferred: bool,
}

impl ApprovalItem {
    /// Construct (protocol_ordered defaults from kind).
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        kind: ApprovalKind,
        summary: impl Into<String>,
        risk: PermissionRisk,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            risk,
            priority: 50,
            actor: None,
            age: None,
            blocking: ApprovalBlocking::Blocking,
            summary: summary.into(),
            preview: None,
            generation: 0,
            protocol_ordered: kind.protocol_ordered_default(),
            host_allows_quick_approve: true,
            deferred: false,
        }
    }

    /// Priority.
    #[must_use]
    pub fn priority(mut self, p: u8) -> Self {
        self.priority = p.min(100);
        self
    }

    /// Actor.
    #[must_use]
    pub fn actor(mut self, a: impl Into<String>) -> Self {
        self.actor = Some(a.into());
        self
    }

    /// Age.
    #[must_use]
    pub fn age(mut self, a: impl Into<String>) -> Self {
        self.age = Some(a.into());
        self
    }

    /// Blocking.
    #[must_use]
    pub const fn blocking(mut self, b: ApprovalBlocking) -> Self {
        self.blocking = b;
        self
    }

    /// Preview.
    #[must_use]
    pub fn preview(mut self, p: impl Into<String>) -> Self {
        self.preview = Some(p.into());
        self
    }

    /// Generation.
    #[must_use]
    pub const fn generation(mut self, g: u64) -> Self {
        self.generation = g;
        self
    }

    /// Force protocol order on/off.
    #[must_use]
    pub const fn protocol_ordered(mut self, on: bool) -> Self {
        self.protocol_ordered = on;
        self
    }

    /// Host quick-approve opt-in (still gated by risk).
    #[must_use]
    pub const fn host_allows_quick_approve(mut self, on: bool) -> Self {
        self.host_allows_quick_approve = on;
        self
    }

    /// Effective sort score (higher first) for non-protocol items.
    #[must_use]
    pub fn sort_score(&self) -> u16 {
        u16::from(self.priority)
            + u16::from(self.blocking.boost())
            + if self.deferred { 0 } else { 5 }
    }

    /// Whether **single** quick approve is allowed (never high/critical).
    #[must_use]
    pub const fn allows_quick_approve(&self) -> bool {
        if !self.host_allows_quick_approve {
            return false;
        }
        // Hard rule: high/critical never one-click from queue
        matches!(self.risk, PermissionRisk::Low | PermissionRisk::Medium)
            && !matches!(self.kind, ApprovalKind::Permission if self.risk.is_destructive())
            && !self.risk.is_destructive()
    }

    /// Whether eligible for **bulk** approve (stricter: Low only).
    #[must_use]
    pub const fn allows_bulk_approve(&self) -> bool {
        self.allows_quick_approve() && matches!(self.risk, PermissionRisk::Low)
    }
}

// ── Presentation ────────────────────────────────────────────────────────────

/// Badge · drawer · full inbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ApprovalQueuePresentation {
    /// Compact status badge (`3 pending`).
    Badge,
    /// Drawer-height list.
    Drawer,
    /// Full management view.
    #[default]
    Full,
}

impl ApprovalQueuePresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Badge => "badge",
            Self::Drawer => "drawer",
            Self::Full => "full",
        }
    }
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Outcomes — requests only; host opens PermissionPrompt / QuestionFlow / etc.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApprovalQueueOutcome {
    /// Ignored.
    Ignored,
    /// Selection moved.
    Selected {
        /// Item id.
        id: String,
    },
    /// Open the full decision UI for this item (always safe path).
    Open {
        /// Id.
        id: String,
        /// Kind (host routes).
        kind: ApprovalKind,
        /// Generation for stale checks.
        generation: u64,
    },
    /// Quick-approve **one** safe item (Low/Medium only; host still validates).
    ApproveRequested {
        /// Id.
        id: String,
        /// Generation.
        generation: u64,
    },
    /// Bulk approve of **only** bulk-safe Low items (explicit multi-select).
    BulkApproveRequested {
        /// Ids that passed the Low-only filter.
        ids: Vec<String>,
    },
    /// Attempted bulk/quick approve that included unsafe items — **denied**.
    BulkApproveDenied {
        /// Reason for chrome/tests.
        reason: String,
        /// Count of blocked high-risk ids.
        blocked: usize,
    },
    /// Defer item (still pending).
    DeferRequested {
        /// Id.
        id: String,
    },
    /// Dismiss / cancel without approve.
    DismissRequested {
        /// Id.
        id: String,
        /// Generation.
        generation: u64,
    },
    /// Presentation changed.
    PresentationChanged(ApprovalQueuePresentation),
    /// Multi-select toggle.
    SelectionToggled {
        /// Id.
        id: String,
        /// Selected.
        selected: bool,
    },
    /// Fullscreen / overlay promote.
    FullscreenRequested,
    /// Drawer open request.
    DrawerRequested,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Approval queue interaction state.
///
/// Protocol-ordered items keep relative FIFO among themselves. High-risk never
/// bulk-approves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalQueueState {
    /// Pending items (host order may include priority; we re-view).
    pub items: Vec<ApprovalItem>,
    /// View order indices into `items`.
    view: Vec<usize>,
    /// Cursor into `view`.
    pub cursor: usize,
    /// Scroll.
    pub scroll: usize,
    /// Multi-select set of item ids (for bulk — Low only).
    pub multi: Vec<String>,
    /// Presentation.
    pub presentation: ApprovalQueuePresentation,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Action strip cursor: 0 Open, 1 Approve?, 2 Defer, 3 Dismiss.
    pub action_cursor: usize,
    /// Row hits.
    pub row_hits: Vec<(String, Rect)>,
    /// Action hits.
    pub action_hits: Vec<(ApprovalAction, Rect)>,
}

/// Local action strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ApprovalAction {
    /// Open full UI.
    Open,
    /// Quick approve (may be disabled).
    Approve,
    /// Defer.
    Defer,
    /// Dismiss.
    Dismiss,
}

impl ApprovalAction {
    /// Label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Approve => "Approve",
            Self::Defer => "Defer",
            Self::Dismiss => "Dismiss",
        }
    }
}

impl Default for ApprovalQueueState {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalQueueState {
    /// Empty.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            view: Vec::new(),
            cursor: 0,
            scroll: 0,
            multi: Vec::new(),
            presentation: ApprovalQueuePresentation::Full,
            focused: true,
            accepts_input: true,
            action_cursor: 0, // Open default — never Approve default
            row_hits: Vec::new(),
            action_hits: Vec::new(),
        }
    }

    /// Replace items and rebuild view (preserves protocol order).
    pub fn set_items(&mut self, items: Vec<ApprovalItem>) {
        let keep = self.current_id();
        self.items = items;
        self.rebuild_view();
        if let Some(id) = keep {
            if let Some(vi) = self
                .view
                .iter()
                .position(|&i| self.items.get(i).is_some_and(|x| x.id == id))
            {
                self.cursor = vi;
            }
        }
        self.clamp();
    }

    /// Push item (assign generation if 0).
    pub fn push(&mut self, mut item: ApprovalItem) -> u64 {
        if item.generation == 0 {
            let g = self
                .items
                .iter()
                .map(|i| i.generation)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            item.generation = g;
        }
        let g = item.generation;
        self.items.push(item);
        self.rebuild_view();
        self.clamp();
        g
    }

    /// Remove by id.
    pub fn remove(&mut self, id: &str) -> Option<ApprovalItem> {
        let i = self.items.iter().position(|x| x.id == id)?;
        let item = self.items.remove(i);
        self.multi.retain(|m| m != id);
        self.rebuild_view();
        self.clamp();
        Some(item)
    }

    /// Gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Focus.
    pub const fn set_focused(&mut self, on: bool) {
        self.focused = on;
    }

    /// Pending count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Blocking count.
    #[must_use]
    pub fn blocking_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| !matches!(i.blocking, ApprovalBlocking::NonBlocking))
            .count()
    }

    /// High-risk count.
    #[must_use]
    pub fn high_risk_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.risk.is_destructive())
            .count()
    }

    /// Badge text.
    #[must_use]
    pub fn badge_label(&self) -> String {
        let n = self.len();
        if n == 0 {
            return "no pending".into();
        }
        let hi = self.high_risk_count();
        if hi > 0 {
            format!("{n} pending · {hi} high-risk")
        } else {
            format!("{n} pending")
        }
    }

    /// Current item.
    #[must_use]
    pub fn current(&self) -> Option<&ApprovalItem> {
        let i = *self.view.get(self.cursor)?;
        self.items.get(i)
    }

    /// Current id.
    #[must_use]
    pub fn current_id(&self) -> Option<String> {
        self.current().map(|i| i.id.clone())
    }

    /// Rebuild view: protocol_ordered items keep relative FIFO; others by score.
    fn rebuild_view(&mut self) {
        let mut protocol: Vec<usize> = Vec::new();
        let mut free: Vec<usize> = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            if item.protocol_ordered {
                protocol.push(i);
            } else {
                free.push(i);
            }
        }
        // protocol: stable insertion order (FIFO)
        free.sort_by(|&a, &b| {
            self.items[b]
                .sort_score()
                .cmp(&self.items[a].sort_score())
                .then_with(|| a.cmp(&b))
        });
        // Protocol head first if any hard gates, else free high priority, but
        // protocol relative order never shuffled.
        // Display: hard-gate protocol first, then free by score, then rest protocol?
        // Spec: "Preserve request order where protocol requires it" — keep all
        // protocol items in FIFO block at front for permission/question chain.
        self.view = protocol;
        self.view.extend(free);
    }

    fn clamp(&mut self) {
        if self.view.is_empty() {
            self.cursor = 0;
            self.scroll = 0;
            self.action_cursor = 0;
            return;
        }
        self.cursor = self.cursor.min(self.view.len() - 1);
        let window = APPROVAL_QUEUE_WINDOW;
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + window {
            self.scroll = self.cursor + 1 - window;
        }
        // Always default action to Open (index 0)
        self.action_cursor = 0;
    }

    fn actions_for_current(&self) -> Vec<ApprovalAction> {
        let Some(item) = self.current() else {
            return Vec::new();
        };
        let mut a = vec![ApprovalAction::Open];
        if item.allows_quick_approve() {
            a.push(ApprovalAction::Approve);
        }
        a.push(ApprovalAction::Defer);
        a.push(ApprovalAction::Dismiss);
        a
    }

    fn move_cursor(&mut self, delta: isize) -> ApprovalQueueOutcome {
        if self.view.is_empty() {
            return ApprovalQueueOutcome::Ignored;
        }
        let n = self.view.len() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, n - 1) as usize;
        self.action_cursor = 0;
        self.clamp();
        ApprovalQueueOutcome::Selected {
            id: self.current_id().unwrap_or_default(),
        }
    }

    /// Keyboard.
    pub fn handle_key(&mut self, key: KeyEvent) -> ApprovalQueueOutcome {
        if !self.focused || !self.accepts_input || !key.is_press() {
            return ApprovalQueueOutcome::Ignored;
        }

        if self.presentation == ApprovalQueuePresentation::Badge {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('o') | KeyCode::Char('d') => {
                    self.presentation = ApprovalQueuePresentation::Drawer;
                    ApprovalQueueOutcome::DrawerRequested
                }
                KeyCode::Char('f') => {
                    self.presentation = ApprovalQueuePresentation::Full;
                    ApprovalQueueOutcome::PresentationChanged(ApprovalQueuePresentation::Full)
                }
                _ => ApprovalQueueOutcome::Ignored,
            };
        }

        if self.items.is_empty() {
            return ApprovalQueueOutcome::Ignored;
        }

        match key.code {
            KeyCode::Esc => {
                if self.presentation == ApprovalQueuePresentation::Full {
                    self.presentation = ApprovalQueuePresentation::Drawer;
                    return ApprovalQueueOutcome::PresentationChanged(
                        ApprovalQueuePresentation::Drawer,
                    );
                }
                if self.presentation == ApprovalQueuePresentation::Drawer {
                    self.presentation = ApprovalQueuePresentation::Badge;
                    return ApprovalQueueOutcome::PresentationChanged(
                        ApprovalQueuePresentation::Badge,
                    );
                }
                ApprovalQueueOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Left | KeyCode::Char('h') => {
                self.action_cursor = self.action_cursor.saturating_sub(1);
                ApprovalQueueOutcome::Ignored
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let n = self.actions_for_current().len();
                if n > 0 && self.action_cursor + 1 < n {
                    self.action_cursor += 1;
                }
                ApprovalQueueOutcome::Ignored
            }
            KeyCode::Enter | KeyCode::Char('o') => self.fire_action_at_cursor(),
            KeyCode::Char('a') => self.try_quick_approve_current(),
            KeyCode::Char('A') => self.try_bulk_approve(),
            KeyCode::Char('d') => self.defer_current(),
            KeyCode::Char('x') | KeyCode::Delete => self.dismiss_current(),
            KeyCode::Char(' ') => self.toggle_multi(),
            KeyCode::Char('b') => {
                self.presentation = ApprovalQueuePresentation::Badge;
                ApprovalQueueOutcome::PresentationChanged(ApprovalQueuePresentation::Badge)
            }
            KeyCode::Char('w') => {
                self.presentation = ApprovalQueuePresentation::Drawer;
                ApprovalQueueOutcome::DrawerRequested
            }
            KeyCode::Char('f') => {
                self.presentation = ApprovalQueuePresentation::Full;
                ApprovalQueueOutcome::FullscreenRequested
            }
            // y never grants
            KeyCode::Char('y') => ApprovalQueueOutcome::Ignored,
            _ => ApprovalQueueOutcome::Ignored,
        }
    }

    fn fire_action_at_cursor(&mut self) -> ApprovalQueueOutcome {
        let actions = self.actions_for_current();
        let Some(action) = actions.get(self.action_cursor).copied() else {
            return self.open_current();
        };
        match action {
            ApprovalAction::Open => self.open_current(),
            ApprovalAction::Approve => self.try_quick_approve_current(),
            ApprovalAction::Defer => self.defer_current(),
            ApprovalAction::Dismiss => self.dismiss_current(),
        }
    }

    fn open_current(&self) -> ApprovalQueueOutcome {
        let Some(item) = self.current() else {
            return ApprovalQueueOutcome::Ignored;
        };
        ApprovalQueueOutcome::Open {
            id: item.id.clone(),
            kind: item.kind,
            generation: item.generation,
        }
    }

    fn try_quick_approve_current(&self) -> ApprovalQueueOutcome {
        let Some(item) = self.current() else {
            return ApprovalQueueOutcome::Ignored;
        };
        if !item.allows_quick_approve() {
            return ApprovalQueueOutcome::BulkApproveDenied {
                reason: "high-risk or permission items require Open — not one-click approve".into(),
                blocked: 1,
            };
        }
        ApprovalQueueOutcome::ApproveRequested {
            id: item.id.clone(),
            generation: item.generation,
        }
    }

    fn try_bulk_approve(&self) -> ApprovalQueueOutcome {
        if self.multi.is_empty() {
            return ApprovalQueueOutcome::Ignored;
        }
        let mut ok = Vec::new();
        let mut blocked = 0usize;
        for id in &self.multi {
            if let Some(item) = self.items.iter().find(|i| i.id == *id) {
                if item.allows_bulk_approve() {
                    ok.push(id.clone());
                } else {
                    blocked += 1;
                }
            }
        }
        if blocked > 0 && ok.is_empty() {
            return ApprovalQueueOutcome::BulkApproveDenied {
                reason: "bulk approve only allowed for low-risk items; open high-risk individually"
                    .into(),
                blocked,
            };
        }
        if blocked > 0 {
            // Partial: still deny bulk when any unsafe selected (safer)
            return ApprovalQueueOutcome::BulkApproveDenied {
                reason: format!(
                    "{blocked} high-risk selected — remove them or Open each; no mixed bulk"
                ),
                blocked,
            };
        }
        if ok.is_empty() {
            return ApprovalQueueOutcome::Ignored;
        }
        ApprovalQueueOutcome::BulkApproveRequested { ids: ok }
    }

    fn defer_current(&mut self) -> ApprovalQueueOutcome {
        let Some(item) = self.current() else {
            return ApprovalQueueOutcome::Ignored;
        };
        let id = item.id.clone();
        if let Some(i) = self.items.iter_mut().find(|x| x.id == id) {
            i.deferred = true;
        }
        self.rebuild_view();
        ApprovalQueueOutcome::DeferRequested { id }
    }

    fn dismiss_current(&self) -> ApprovalQueueOutcome {
        let Some(item) = self.current() else {
            return ApprovalQueueOutcome::Ignored;
        };
        ApprovalQueueOutcome::DismissRequested {
            id: item.id.clone(),
            generation: item.generation,
        }
    }

    fn toggle_multi(&mut self) -> ApprovalQueueOutcome {
        let Some(id) = self.current_id() else {
            return ApprovalQueueOutcome::Ignored;
        };
        if let Some(pos) = self.multi.iter().position(|m| *m == id) {
            self.multi.remove(pos);
            ApprovalQueueOutcome::SelectionToggled {
                id,
                selected: false,
            }
        } else {
            self.multi.push(id.clone());
            ApprovalQueueOutcome::SelectionToggled { id, selected: true }
        }
    }

    /// Mouse.
    pub fn handle_mouse(&mut self, ev: MouseEvent) -> ApprovalQueueOutcome {
        if !self.focused || !self.accepts_input {
            return ApprovalQueueOutcome::Ignored;
        }
        if !matches!(ev.kind, MouseEventKind::Down(MouseButton::Left)) {
            return ApprovalQueueOutcome::Ignored;
        }
        let pos = ev.position;
        for (action, r) in &self.action_hits {
            if r.contains(pos) {
                // Map action without moving action_cursor permanently
                return match action {
                    ApprovalAction::Open => self.open_current(),
                    ApprovalAction::Approve => self.try_quick_approve_current(),
                    ApprovalAction::Defer => self.defer_current(),
                    ApprovalAction::Dismiss => self.dismiss_current(),
                };
            }
        }
        let hit = self
            .row_hits
            .iter()
            .find(|(_, r)| r.contains(pos))
            .map(|(id, _)| id.clone());
        if let Some(id) = hit {
            if let Some(vi) = self
                .view
                .iter()
                .position(|&i| self.items.get(i).is_some_and(|x| x.id == id))
            {
                self.cursor = vi;
                self.action_cursor = 0;
                return ApprovalQueueOutcome::Selected { id };
            }
        }
        ApprovalQueueOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Approval queue painter.
#[derive(Debug, Clone, Copy)]
pub struct ApprovalQueue<'a> {
    system: &'a DesignSystem,
    colorless: bool,
}

impl<'a> ApprovalQueue<'a> {
    /// System only.
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

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut ApprovalQueueState) {
        state.row_hits.clear();
        state.action_hits.clear();
        if area.is_empty() {
            return;
        }
        match state.presentation {
            ApprovalQueuePresentation::Badge => self.paint_badge(area, buffer, state),
            ApprovalQueuePresentation::Drawer | ApprovalQueuePresentation::Full => {
                self.paint_list(area, buffer, state)
            }
        }
    }

    fn paint_badge(&self, area: Rect, buffer: &mut Buffer, state: &ApprovalQueueState) {
        let label = state.badge_label();
        let (semantic, verb) = if state.high_risk_count() > 0 {
            (SemanticStatus::Warning, "warning")
        } else if state.blocking_count() > 0 {
            (SemanticStatus::Waiting, "waiting")
        } else {
            (SemanticStatus::Idle, "idle")
        };
        let text = format!("{verb}: {label}");
        StatusIndicator::new(semantic, self.system)
            .label(&text)
            .colorless(self.colorless)
            .paint(Rect::new(area.x, area.y, area.width, 1), buffer);
    }

    fn paint_list(&self, area: Rect, buffer: &mut Buffer, state: &mut ApprovalQueueState) {
        let title = match state.presentation {
            ApprovalQueuePresentation::Drawer => "Approvals · drawer",
            _ => "Approvals",
        };
        let panel = Panel::new(self.system)
            .title(title)
            .emphasis(if state.focused {
                PanelChrome::Focused
            } else {
                PanelChrome::Normal
            });
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);
        if inner.is_empty() {
            return;
        }

        let mut y = inner.y;
        let w = usize::from(inner.width);
        let max_y = inner.bottom();

        // Safety banner
        if y < max_y {
            let banner = if state.high_risk_count() > 0 {
                "high-risk: Open only — no bulk approve"
            } else {
                "Space multi-select Low only · A bulk · a approve · never y"
            };
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, y, inner.width, 1),
                banner,
                self.system.style(Role::TextMuted),
            );
            y = y.saturating_add(1);
        }

        if state.view.is_empty() {
            EmptyState::new("Nothing to decide", self.system)
                .kind(EmptyKind::NoData)
                .paint(Rect::new(inner.x, y, inner.width, 1), buffer);
            return;
        }

        let list_bottom = max_y.saturating_sub(2);
        let viewport = list_bottom.saturating_sub(y) as usize;
        let mut offset = state.scroll;
        if state.cursor < offset {
            offset = state.cursor;
        } else if viewport > 0 && state.cursor >= offset + viewport {
            offset = state.cursor + 1 - viewport;
        }
        state.scroll = offset;

        for (vi, &ii) in state.view.iter().enumerate().skip(offset) {
            if y >= list_bottom {
                break;
            }
            let Some(item) = state.items.get(ii) else {
                continue;
            };
            let selected = vi == state.cursor;
            let multi = state.multi.iter().any(|m| m == &item.id);
            let mark = if selected {
                crate::style::Glyph::SelectionMarker.resolve().text
            } else {
                " "
            };
            let boxm = if multi {
                crate::style::Glyph::Success.resolve().text
            } else {
                " "
            };
            let kg = item.kind.glyph(false);
            let risk = format!("{} {}", item.risk.glyph(), item.risk.label());
            let proto = if item.protocol_ordered { " fifo" } else { "" };
            let def = if item.deferred { " def" } else { "" };
            let text = format!(
                "{mark}{boxm}{kg}[{risk}] {} · {}{proto}{def}",
                item.kind.label(),
                item.summary
            );
            // Selection speaks through the shared row recipe (tint + weight via
            // the cursor marker), never by painting the whole label accent.
            let style = if selected {
                self.system
                    .resolve_list_row(ListRowVisualState {
                        selected: true,
                        focused: selected,
                        enabled: true,
                        ..ListRowVisualState::default()
                    })
                    .label
            } else {
                self.system.style(Role::Text)
            };
            self.system
                .paint_row(buffer, Rect::new(inner.x, y, inner.width, 1), &text, style);
            state.row_hits.push((
                item.id.clone(),
                Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: 1,
                },
            ));
            y = y.saturating_add(1);
            // preview when selected / full
            if selected || matches!(state.presentation, ApprovalQueuePresentation::Full) {
                if let Some(p) = &item.preview {
                    if y < list_bottom {
                        let actor = item
                            .actor
                            .as_ref()
                            .map(|a| format!(" · {a}"))
                            .unwrap_or_default();
                        let age = item
                            .age
                            .as_ref()
                            .map(|a| format!(" · {a}"))
                            .unwrap_or_default();
                        let line = format!("    {p}{actor}{age}");
                        self.system.paint_row(
                            buffer,
                            Rect::new(inner.x, y, inner.width, 1),
                            &line,
                            self.system.style(Role::TextMuted),
                        );
                        y = y.saturating_add(1);
                    }
                }
            }
        }

        // Preview detail line + actions
        if let Some(item) = state.current() {
            let py = max_y.saturating_sub(2);
            if py >= inner.y {
                let safe = if item.allows_quick_approve() {
                    "quick-approve ok"
                } else {
                    "must Open — not bulk-safe"
                };
                self.system.paint_row(
                    buffer,
                    Rect::new(inner.x, py, inner.width, 1),
                    &format!("{} · {} · {safe}", item.blocking.label(), item.risk.label()),
                    self.system.style(if item.allows_quick_approve() {
                        Role::TextMuted
                    } else {
                        Role::Warning
                    }),
                );
            }
        }
        let ay = max_y.saturating_sub(1);
        self.paint_actions(inner.x, ay, w, buffer, state);
    }

    fn paint_actions(
        &self,
        x: u16,
        y: u16,
        w: usize,
        buffer: &mut Buffer,
        state: &mut ApprovalQueueState,
    ) {
        let actions = state.actions_for_current();
        if actions.is_empty() {
            return;
        }
        let mut col = x;
        let end = x.saturating_add(w as u16);
        for (i, action) in actions.iter().enumerate() {
            let focused = i == state.action_cursor;
            let label = action.label();
            let disabled_look = matches!(action, ApprovalAction::Approve)
                && !state.current().is_some_and(|c| c.allows_quick_approve());
            let text = if focused {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            };
            let tw = display_cols(&text) as u16;
            if col.saturating_add(tw) > end {
                break;
            }
            let style = if focused && !self.colorless {
                self.system.style(Role::Accent).add_modifier(Modifier::BOLD)
            } else if focused {
                // Mono focus is the explicit reversal pair (D5), not a swap
                // modifier over the idle face.
                self.system.reversed()
            } else if disabled_look {
                self.system.style(Role::TextMuted)
            } else {
                self.system.style(Role::TextMuted)
            };
            self.system
                .paint_row(buffer, Rect::new(col, y, tw, 1), &text, style);
            state.action_hits.push((
                *action,
                Rect {
                    x: col,
                    y,
                    width: tw,
                    height: 1,
                },
            ));
            col = col.saturating_add(tw.saturating_add(1));
        }
    }
}

impl StatefulWidget for &ApprovalQueue<'_> {
    type State = ApprovalQueueState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for ApprovalQueue<'_> {
    type State = ApprovalQueueState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Integrations ────────────────────────────────────────────────────────────

/// Project pending approvals into NotificationCenter items.
#[must_use]
pub fn approval_items_to_notifications(
    items: &[ApprovalItem],
    now_secs: u64,
) -> Vec<NotificationItem> {
    items
        .iter()
        .map(|i| {
            let kind = if i.risk.is_destructive() {
                ToastKind::Error
            } else if matches!(
                i.blocking,
                ApprovalBlocking::HardGate | ApprovalBlocking::Blocking
            ) {
                ToastKind::Warning
            } else {
                ToastKind::Info
            };
            let priority = if i.risk.is_destructive() {
                ToastPriority::High
            } else {
                ToastPriority::Normal
            };
            let mut n =
                NotificationItem::new(format!("approval:{}", i.id), i.summary.clone(), kind)
                    .title(format!("{} · {}", i.kind.label(), i.risk.label()));
            n.priority = priority;
            n.source = i.actor.clone();
            n.group_id = Some("approvals".into());
            n.created_at_secs = now_secs;
            n.unread = true;
            n.actions = vec![
                ("open".into(), "Open".into()),
                ("defer".into(), "Defer".into()),
            ];
            if i.allows_quick_approve() {
                n.actions.push(("approve".into(), "Approve".into()));
            }
            n.announcement = format!("Pending {}: {}", i.kind.label(), i.summary);
            n
        })
        .collect()
}

/// Project into TaskRail ActivityModel rows (needs_input).
#[must_use]
pub fn approval_items_to_activity_models(items: &[ApprovalItem]) -> Vec<ActivityModel> {
    items
        .iter()
        .map(|i| {
            let mut m = ActivityModel::new(
                format!("approval:{}", i.id),
                format!("{}: {}", i.kind.label(), i.summary),
            )
            .status(SemanticStatus::Waiting)
            .kind(ActivityKind::Generic);
            m.scope = ActivityScope::Foreground;
            m.needs_input = true;
            m.blocked = !matches!(i.blocking, ApprovalBlocking::NonBlocking);
            m.actor = i.actor.clone();
            m.elapsed = i.age.clone();
            m.waiting_reason = Some(i.blocking.label().into());
            m.detail = i.preview.clone();
            m.group_key = Some("approvals".into());
            m
        })
        .collect()
}

/// Count badge for StatusBar / header.
#[must_use]
pub fn approval_queue_badge(items: &[ApprovalItem]) -> String {
    let n = items.len();
    if n == 0 {
        return String::new();
    }
    let hi = items.iter().filter(|i| i.risk.is_destructive()).count();
    if hi > 0 {
        format!("approvals:{n} high:{hi}")
    } else {
        format!("approvals:{n}")
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo mixed decision queue (protocol perms first).
#[must_use]
pub fn example_approval_queue() -> Vec<ApprovalItem> {
    vec![
        ApprovalItem::new(
            "p1",
            ApprovalKind::Permission,
            "shell: cargo test",
            PermissionRisk::High,
        )
        .actor("agent > sub")
        .age("30s")
        .blocking(ApprovalBlocking::HardGate)
        .generation(1)
        .preview("cwd: workspace · DESTRUCTIVE"),
        ApprovalItem::new(
            "p2",
            ApprovalKind::Permission,
            "read Cargo.toml",
            PermissionRisk::Low,
        )
        .actor("agent")
        .age("1m")
        .blocking(ApprovalBlocking::Blocking)
        .generation(2)
        .preview("file read"),
        ApprovalItem::new(
            "q1",
            ApprovalKind::Question,
            "Deploy strategy?",
            PermissionRisk::Medium,
        )
        .actor("agent")
        .age("2m")
        .generation(3)
        .preview("blue/green vs canary"),
        ApprovalItem::new(
            "plan1",
            ApprovalKind::Plan,
            "Migrate auth module",
            PermissionRisk::Medium,
        )
        .actor("agent")
        .age("5m")
        .blocking(ApprovalBlocking::NonBlocking)
        .protocol_ordered(false)
        .priority(80)
        .preview("3 tasks · 4 files"),
        ApprovalItem::new(
            "diff1",
            ApprovalKind::Diff,
            "Review token.rs hunks",
            PermissionRisk::Low,
        )
        .actor("agent")
        .age("6m")
        .blocking(ApprovalBlocking::NonBlocking)
        .protocol_ordered(false)
        .priority(40)
        .preview("+42 −8"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Frames.
    pub const PAINT_FRAMES: u32 = 30;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;
    use ratatui_core::layout::Position;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn open() -> ApprovalQueueState {
        let mut st = ApprovalQueueState::new();
        st.set_items(example_approval_queue());
        st.presentation = ApprovalQueuePresentation::Full;
        st
    }

    #[test]
    fn protocol_order_preserved_for_permissions() {
        let st = open();
        // first two view entries should be protocol permissions in push order
        let ids: Vec<_> = st
            .view
            .iter()
            .filter_map(|&i| st.items.get(i).map(|x| x.id.as_str()))
            .collect();
        let p1 = ids.iter().position(|id| *id == "p1").unwrap();
        let p2 = ids.iter().position(|id| *id == "p2").unwrap();
        let q1 = ids.iter().position(|id| *id == "q1").unwrap();
        assert!(p1 < p2 && p2 < q1, "{ids:?}");
    }

    #[test]
    fn high_risk_no_quick_approve() {
        let st = open();
        let p1 = st.items.iter().find(|i| i.id == "p1").unwrap();
        assert!(!p1.allows_quick_approve());
        assert!(!p1.allows_bulk_approve());
    }

    #[test]
    fn low_risk_allows_quick_and_bulk() {
        let st = open();
        let p2 = st.items.iter().find(|i| i.id == "p2").unwrap();
        assert!(p2.allows_quick_approve());
        assert!(p2.allows_bulk_approve());
    }

    #[test]
    fn open_default_action_not_approve() {
        let mut st = open();
        // high risk head
        assert_eq!(st.action_cursor, 0);
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::Open {
                kind: ApprovalKind::Permission,
                ref id,
                ..
            } if id == "p1"
        ));
    }

    #[test]
    fn a_on_high_risk_denied() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('a')));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::BulkApproveDenied { blocked: 1, .. }
        ));
    }

    #[test]
    fn bulk_rejects_mixed_high_risk() {
        let mut st = open();
        st.multi = vec!["p1".into(), "p2".into()];
        let out = st.handle_key(press(KeyCode::Char('A')));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::BulkApproveDenied { blocked: 1, .. }
        ));
    }

    #[test]
    fn bulk_low_only_ok() {
        let mut st = open();
        st.multi = vec!["p2".into(), "diff1".into()];
        let out = st.handle_key(press(KeyCode::Char('A')));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::BulkApproveRequested { ref ids }
                if ids.len() == 2 && ids.contains(&"p2".into())
        ));
    }

    #[test]
    fn y_unbound() {
        let mut st = open();
        // even on low risk
        let i = st
            .view
            .iter()
            .position(|&ii| st.items[ii].id == "p2")
            .unwrap();
        st.cursor = i;
        assert!(matches!(
            st.handle_key(press(KeyCode::Char('y'))),
            ApprovalQueueOutcome::Ignored
        ));
    }

    #[test]
    fn defer_and_dismiss() {
        let mut st = open();
        let out = st.handle_key(press(KeyCode::Char('d')));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::DeferRequested { ref id } if id == "p1"
        ));
        let out = st.handle_key(press(KeyCode::Char('x')));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::DismissRequested { ref id, .. } if id == "p1"
        ));
    }

    #[test]
    fn notification_projection() {
        let items = example_approval_queue();
        let notes = approval_items_to_notifications(&items, 1000);
        assert_eq!(notes.len(), items.len());
        let hi = notes.iter().find(|n| n.id.contains("p1")).unwrap();
        assert_eq!(hi.kind, ToastKind::Error);
        // high risk should not get approve action
        assert!(!hi.actions.iter().any(|(id, _)| id == "approve"));
        let lo = notes.iter().find(|n| n.id.contains("p2")).unwrap();
        assert!(lo.actions.iter().any(|(id, _)| id == "approve"));
    }

    #[test]
    fn task_rail_projection() {
        let models = approval_items_to_activity_models(&example_approval_queue());
        assert!(models.iter().all(|m| m.needs_input));
        assert!(models.iter().any(|m| m.blocked));
    }

    #[test]
    fn badge_and_presentations() {
        let mut st = open();
        assert!(st.badge_label().contains("high-risk"));
        st.presentation = ApprovalQueuePresentation::Badge;
        let out = st.handle_key(press(KeyCode::Enter));
        assert!(matches!(out, ApprovalQueueOutcome::DrawerRequested));
    }

    #[test]
    fn accepts_input_gate() {
        let mut st = open();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(press(KeyCode::Enter)),
            ApprovalQueueOutcome::Ignored
        ));
    }

    #[test]
    fn no_process_policy() {
        let src = include_str!("approval_queue.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in ["std::process", "Command::new", "openai"] {
            assert!(!body.contains(f), "{f}");
        }
        assert!(body.contains("bulk") || body.contains("high-risk"));
        assert!(body.contains("never") || body.contains("Never"));
    }

    #[test]
    fn paint_all() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 64, 16);
        let mut buf = Buffer::empty(area);
        for p in [
            ApprovalQueuePresentation::Badge,
            ApprovalQueuePresentation::Drawer,
            ApprovalQueuePresentation::Full,
        ] {
            st.presentation = p;
            ApprovalQueue::new(&system)
                .colorless(true)
                .paint(area, &mut buf, &mut st);
        }
    }

    #[test]
    fn paint_perf() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 60, 18);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            ApprovalQueue::new(&system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 3, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_kinds() {
        for k in [
            ApprovalKind::Permission,
            ApprovalKind::Question,
            ApprovalKind::Plan,
            ApprovalKind::Diff,
            ApprovalKind::Other,
        ] {
            assert!(!k.id().is_empty());
        }
    }

    #[test]
    fn mouse_select() {
        let system = DesignSystem::default();
        let mut st = open();
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        ApprovalQueue::new(&system).paint(area, &mut buf, &mut st);
        assert!(!st.row_hits.is_empty());
        let (id, r) = st.row_hits[0].clone();
        let out = st.handle_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position { x: r.x, y: r.y },
            modifiers: KeyModifiers::NONE,
        });
        assert!(
            matches!(out, ApprovalQueueOutcome::Selected { .. }),
            "{out:?} {id}"
        );
    }

    #[test]
    fn unicode() {
        let system = DesignSystem::default();
        let mut st = ApprovalQueueState::new();
        st.set_items(vec![
            ApprovalItem::new(
                "u",
                ApprovalKind::Question,
                "続行しますか？",
                PermissionRisk::Low,
            )
            .actor("エージェント"),
        ]);
        let area = Rect::new(0, 0, 40, 8);
        let mut buf = Buffer::empty(area);
        ApprovalQueue::new(&system).paint(area, &mut buf, &mut st);
    }

    #[test]
    fn space_toggles_multi() {
        let mut st = open();
        let i = st
            .view
            .iter()
            .position(|&ii| st.items[ii].id == "p2")
            .unwrap();
        st.cursor = i;
        let out = st.handle_key(press(KeyCode::Char(' ')));
        assert!(matches!(
            out,
            ApprovalQueueOutcome::SelectionToggled { selected: true, .. }
        ));
        assert!(st.multi.contains(&"p2".into()));
    }

    #[test]
    fn multi_membership_is_check_not_checkbox_well() {
        let system = DesignSystem::junie();
        let mut st = open();
        let i = st
            .view
            .iter()
            .position(|&ii| st.items[ii].id == "p2")
            .unwrap();
        st.cursor = i;
        let _ = st.handle_key(press(KeyCode::Char(' ')));
        let area = Rect::new(0, 0, 56, 14);
        let mut buf = Buffer::empty(area);
        ApprovalQueue::new(&system).paint(area, &mut buf, &mut st);
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buf[(x, y)].symbol());
            }
        }
        assert!(
            !text.contains("[✓]") && !text.contains("[ ]"),
            "checkbox wells leaked: {text:?}"
        );
        assert!(text.contains('✓'), "list membership ✓ missing: {text:?}");
    }
}
