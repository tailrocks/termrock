// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **TaskRail** — unified task and agent activity side panel.
//!
//! **Mission.** Group workflows, subagents, foreground/background tasks,
//! watchers, and completed history. Collapse, filter, search, selection,
//! semantic zoom, status, elapsed, progress, dependencies, and contextual
//! actions. Prioritize requests needing user input. Collapse into Drawer or
//! StatusBar summary responsively. Application-neutral [`ActivityModel`].
//!
//! **vs [`super::ActivityShelf`].** Shelf = glanceable strip of concurrent
//! actives. Rail = full vertical inventory with groups + search + deps.
//! **vs thin List façade (removed).** Hosts project [`ActivityModel`]; optional
//! [`project_task_rail_list_rows`] still feeds raw [`List`] if needed.
//!
//! Research: Grok Build tasks pane, Amp sessions, OpenCode agents, CI lists,
//! Zellij panes.
//!
//! Teaches: how to compose unified task and agent activity side panel.
//!
//! Composes: [`crate::widgets::List`], [`crate::widgets::ListRow`],
//! [`crate::widgets::ListState`], [`crate::widgets::Panel`],
//! [`crate::widgets::RowRole`], [`crate::widgets::SemanticStatus`],
//! [`crate::widgets::StatefulWidget`], [`crate::widgets::StatusKind`], and 3
//! more.
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer, layout::Rect, style::Modifier, text::Line, widgets::StatefulWidget,
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    patterns::{
        ActivityItem, ActivityKind, ActivityStatusProjection, activity_status_summary,
        project_activities_for_status_bar,
    },
    style::{DesignSystem, PanelChrome, Role},
    text::{display_cols, take_display_cols},
    widgets::{
        List, ListRow, ListState, Panel, RowRole, SemanticStatus, StatusKind, StatusRegion,
        StatusSlot,
    },
};

/// Overlay / drawer id for collapsed rail.
pub const TASK_RAIL_DRAWER_OVERLAY_ID: &str = "termrock.task_rail_drawer";
/// Narrow width → host should prefer drawer.
pub const TASK_RAIL_DRAWER_WIDTH: u16 = 48;
/// Compact detail threshold.
pub const TASK_RAIL_COMPACT_WIDTH: u16 = 22;
/// Max dependency labels painted per row.
pub const TASK_RAIL_DEP_CAP: usize = 3;

// ── ActivityModel ───────────────────────────────────────────────────────────

/// Application-neutral activity scope / group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[non_exhaustive]
pub enum ActivityScope {
    /// Multi-step workflow / plan.
    Workflow,
    /// Nested agent / subagent.
    Subagent,
    /// Foreground interactive work.
    #[default]
    Foreground,
    /// Background long-running job.
    Background,
    /// Watcher / file / CI watch.
    Watcher,
    /// Completed history (success/fail/cancel).
    Completed,
}

impl ActivityScope {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Workflow => "workflow",
            Self::Subagent => "subagent",
            Self::Foreground => "foreground",
            Self::Background => "background",
            Self::Watcher => "watcher",
            Self::Completed => "completed",
        }
    }

    /// Group header title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Workflow => "Workflows",
            Self::Subagent => "Subagents",
            Self::Foreground => "Foreground",
            Self::Background => "Background",
            Self::Watcher => "Watchers",
            Self::Completed => "Completed",
        }
    }

    /// Sort order in rail (workflows first, completed last).
    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::Workflow => 0,
            Self::Subagent => 1,
            Self::Foreground => 2,
            Self::Background => 3,
            Self::Watcher => 4,
            Self::Completed => 5,
        }
    }
}

/// Edge to another activity (depends-on).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityDependency {
    /// Target activity id.
    pub id: String,
    /// Short label.
    pub label: String,
}

impl ActivityDependency {
    /// Construct.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Contextual action affordance (host executes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ActivityActionKind {
    /// Open / jump.
    Open,
    /// Cancel.
    Cancel,
    /// Retry.
    Retry,
    /// Focus transcript / log.
    FocusTranscript,
    /// Inspect dependencies.
    InspectDeps,
    /// Promote to drawer/fullscreen host surface.
    Promote,
}

impl ActivityActionKind {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Cancel => "cancel",
            Self::Retry => "retry",
            Self::FocusTranscript => "focus-transcript",
            Self::InspectDeps => "inspect-deps",
            Self::Promote => "promote",
        }
    }

    /// Chord hint.
    #[must_use]
    pub const fn chord(self) -> &'static str {
        match self {
            Self::Open => "Enter",
            Self::Cancel => "x",
            Self::Retry => "r",
            Self::FocusTranscript => "t",
            Self::InspectDeps => "d",
            Self::Promote => "f",
        }
    }
}

/// Application-neutral activity / task node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityModel {
    /// Stable id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Scope group.
    pub scope: ActivityScope,
    /// Kind (tool/shell/…).
    pub kind: ActivityKind,
    /// Status.
    pub status: SemanticStatus,
    /// Actor / provenance.
    pub actor: Option<String>,
    /// Elapsed display.
    pub elapsed: Option<String>,
    /// Progress 0–100.
    pub progress: Option<u8>,
    /// Waiting / blocked reason.
    pub waiting_reason: Option<String>,
    /// Needs user input (permission, question, conflict).
    pub needs_input: bool,
    /// Blocked on dependency or external condition.
    pub blocked: bool,
    /// Parent activity id (tree).
    pub parent_id: Option<String>,
    /// Dependencies.
    pub dependencies: Vec<ActivityDependency>,
    /// Extra detail line.
    pub detail: Option<String>,
    /// Optional free-form group key within scope.
    pub group_key: Option<String>,
    /// Stream / paint revision.
    pub revision: u64,
}

impl ActivityModel {
    /// Foreground running task.
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            scope: ActivityScope::Foreground,
            kind: ActivityKind::Generic,
            status: SemanticStatus::Running,
            actor: None,
            elapsed: None,
            progress: None,
            waiting_reason: None,
            needs_input: false,
            blocked: false,
            parent_id: None,
            dependencies: Vec::new(),
            detail: None,
            group_key: None,
            revision: 0,
        }
    }

    /// Scope.
    #[must_use]
    pub const fn scope(mut self, s: ActivityScope) -> Self {
        self.scope = s;
        self
    }

    /// Kind.
    #[must_use]
    pub const fn kind(mut self, k: ActivityKind) -> Self {
        self.kind = k;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: SemanticStatus) -> Self {
        self.status = s;
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

    /// Needs input.
    #[must_use]
    pub const fn needs_input(mut self, on: bool) -> Self {
        self.needs_input = on;
        if on {
            self.status = SemanticStatus::Waiting;
        }
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

    /// Parent.
    #[must_use]
    pub fn parent(mut self, id: impl Into<String>) -> Self {
        self.parent_id = Some(id.into());
        self
    }

    /// Dependency.
    #[must_use]
    pub fn depend(mut self, dep: ActivityDependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Detail.
    #[must_use]
    pub fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }

    /// Group key.
    #[must_use]
    pub fn group_key(mut self, k: impl Into<String>) -> Self {
        self.group_key = Some(k.into());
        self
    }

    /// Priority rank (lower first). Needs-input wins.
    #[must_use]
    pub fn priority_rank(&self) -> u32 {
        let mut r = 1000u32;
        if self.needs_input {
            r = r.saturating_sub(600);
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
        // completed scope sorts later unless needs_input
        r = r.saturating_add(u32::from(self.scope.order()) * 5);
        r
    }

    /// Default actions for status.
    #[must_use]
    pub fn default_actions(&self) -> Vec<ActivityActionKind> {
        let mut a = vec![ActivityActionKind::Open];
        if matches!(
            self.status,
            SemanticStatus::Running | SemanticStatus::Queued | SemanticStatus::Waiting
        ) {
            a.push(ActivityActionKind::Cancel);
        }
        if matches!(
            self.status,
            SemanticStatus::Failed | SemanticStatus::Paused | SemanticStatus::Success
        ) {
            a.push(ActivityActionKind::Retry);
        }
        a.push(ActivityActionKind::FocusTranscript);
        if !self.dependencies.is_empty() {
            a.push(ActivityActionKind::InspectDeps);
        }
        a.push(ActivityActionKind::Promote);
        a
    }

    /// Bridge to shelf item (active strip).
    #[must_use]
    pub fn to_shelf_item(&self) -> ActivityItem {
        ActivityItem::new(self.id.clone(), self.title.clone())
            .status(self.status)
            .kind(self.kind)
            .blocked(self.blocked)
            .action_required(self.needs_input)
            .dismissible(!matches!(self.scope, ActivityScope::Watcher))
            .then_actor(self.actor.clone())
            .then_elapsed(self.elapsed.clone())
            .then_progress(self.progress)
            .then_waiting(self.waiting_reason.clone())
    }
}

/// Builder helpers avoiding Option-chain noise on ActivityItem (local).
trait ActivityItemBridge {
    fn then_actor(self, a: Option<String>) -> Self;
    fn then_elapsed(self, e: Option<String>) -> Self;
    fn then_progress(self, p: Option<u8>) -> Self;
    fn then_waiting(self, w: Option<String>) -> Self;
}

impl ActivityItemBridge for ActivityItem {
    fn then_actor(self, a: Option<String>) -> Self {
        match a {
            Some(v) => self.actor(v),
            None => self,
        }
    }
    fn then_elapsed(self, e: Option<String>) -> Self {
        match e {
            Some(v) => self.elapsed(v),
            None => self,
        }
    }
    fn then_progress(self, p: Option<u8>) -> Self {
        match p {
            Some(v) => self.progress(v),
            None => self,
        }
    }
    fn then_waiting(self, w: Option<String>) -> Self {
        match w {
            Some(v) => self.waiting_reason(v),
            None => self,
        }
    }
}

/// Shelf → model (lossy for scope).
#[must_use]
pub fn activity_model_from_shelf(item: &ActivityItem) -> ActivityModel {
    let mut m = ActivityModel::new(item.id.clone(), item.title.clone())
        .status(item.status)
        .kind(item.kind)
        .blocked(item.blocked)
        .needs_input(item.action_required)
        .scope(match item.kind {
            ActivityKind::Subagent => ActivityScope::Subagent,
            ActivityKind::Build | ActivityKind::Network | ActivityKind::Search => {
                ActivityScope::Background
            }
            ActivityKind::Shell | ActivityKind::Tool | ActivityKind::Generic => {
                ActivityScope::Foreground
            }
        });
    if let Some(a) = &item.actor {
        m = m.actor(a.clone());
    }
    if let Some(e) = &item.elapsed {
        m = m.elapsed(e.clone());
    }
    if let Some(p) = item.progress {
        m = m.progress(p);
    }
    if let Some(w) = &item.waiting_reason {
        m = m.waiting_reason(w.clone());
    }
    m
}

/// Active (non-completed) models as shelf items, priority sorted.
#[must_use]
pub fn activity_models_to_shelf(items: &[ActivityModel]) -> Vec<ActivityItem> {
    let mut v: Vec<ActivityItem> = items
        .iter()
        .filter(|i| !matches!(i.scope, ActivityScope::Completed) || i.needs_input)
        .map(ActivityModel::to_shelf_item)
        .collect();
    v.sort_by_key(|i| i.priority_rank());
    v
}

/// Sort models: needs_input first, then scope order / status.
#[must_use]
pub fn sort_activity_models(items: &[ActivityModel]) -> Vec<&ActivityModel> {
    let mut v: Vec<&ActivityModel> = items.iter().collect();
    v.sort_by(|a, b| {
        a.priority_rank()
            .cmp(&b.priority_rank())
            .then_with(|| a.scope.order().cmp(&b.scope.order()))
            .then_with(|| a.id.cmp(&b.id))
    });
    v
}

/// Filter by free-text query (title, actor, detail, waiting, deps).
#[must_use]
pub fn filter_activity_models<'a>(
    items: &'a [ActivityModel],
    query: &str,
) -> Vec<&'a ActivityModel> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return items.iter().collect();
    }
    items
        .iter()
        .filter(|i| {
            i.title.to_ascii_lowercase().contains(&q)
                || i.actor
                    .as_ref()
                    .is_some_and(|a| a.to_ascii_lowercase().contains(&q))
                || i.detail
                    .as_ref()
                    .is_some_and(|d| d.to_ascii_lowercase().contains(&q))
                || i.waiting_reason
                    .as_ref()
                    .is_some_and(|w| w.to_ascii_lowercase().contains(&q))
                || i.scope.id().contains(q.as_str())
                || i.kind.id().contains(q.as_str())
                || i.dependencies
                    .iter()
                    .any(|d| d.label.to_ascii_lowercase().contains(&q) || d.id.contains(&q))
        })
        .collect()
}

/// Counts for footer / summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskRailCounts {
    /// Total visible.
    pub total: usize,
    /// Needs input.
    pub needs_input: usize,
    /// Running.
    pub running: usize,
    /// Blocked.
    pub blocked: usize,
    /// Failed.
    pub failed: usize,
    /// Completed scope.
    pub completed: usize,
}

/// Aggregate counts.
#[must_use]
pub fn task_rail_counts(items: &[ActivityModel]) -> TaskRailCounts {
    let mut c = TaskRailCounts {
        total: items.len(),
        ..TaskRailCounts::default()
    };
    for i in items {
        if i.needs_input {
            c.needs_input += 1;
        }
        if i.blocked {
            c.blocked += 1;
        }
        if matches!(i.scope, ActivityScope::Completed) {
            c.completed += 1;
        }
        match i.status {
            SemanticStatus::Running => c.running += 1,
            SemanticStatus::Failed => c.failed += 1,
            _ => {}
        }
    }
    c
}

/// One-line StatusBar / drawer chip summary.
#[must_use]
pub fn task_rail_status_summary(items: &[ActivityModel], ascii: bool) -> String {
    let shelf = activity_models_to_shelf(items);
    if shelf.is_empty() {
        let c = task_rail_counts(items);
        if c.total == 0 {
            return if ascii {
                "tasks: idle".into()
            } else {
                "tasks · idle".into()
            };
        }
    }
    // Prefer shelf summary for active; append completed count
    let base = activity_status_summary(&shelf, ascii);
    let c = task_rail_counts(items);
    if c.completed > 0 {
        format!("{base} · {} done", c.completed)
    } else {
        base
    }
}

/// StatusBar projection for collapsed rail.
#[must_use]
pub fn project_task_rail_for_status_bar(
    items: &[ActivityModel],
    ascii: bool,
) -> ActivityStatusProjection {
    let shelf = activity_models_to_shelf(items);
    let mut p = project_activities_for_status_bar(&shelf, ascii);
    // Keep high priority when needs_input
    if items.iter().any(|i| i.needs_input) {
        p.priority = 98;
        p.kind = StatusKind::Transient;
        p.region = StatusRegion::Right;
        p.summary = task_rail_status_summary(items, ascii);
    } else {
        p.summary = task_rail_status_summary(items, ascii);
    }
    p
}

/// Status slot helper.
#[must_use]
pub fn task_rail_status_slot<'a, Id>(
    id: Id,
    projection: &'a ActivityStatusProjection,
    use_badge: bool,
) -> StatusSlot<'a, Id> {
    let content = if use_badge {
        projection.badge.as_str()
    } else {
        projection.summary.as_str()
    };
    StatusSlot::new(id, content)
        .kind(projection.kind)
        .priority(projection.priority)
        .region(projection.region)
        .min_width(if use_badge { 2 } else { 10 })
}

// ── Flat row model for paint / list projection ──────────────────────────────

/// Flattened paint row (group header or activity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRailRow {
    /// Collapsible group header.
    Group {
        /// Scope.
        scope: ActivityScope,
        /// Count in group (visible).
        count: usize,
        /// Collapsed.
        collapsed: bool,
    },
    /// Activity leaf.
    Item {
        /// Model id.
        id: String,
        /// Indent depth (parent chain).
        depth: u8,
    },
}

/// Build ordered rows from filtered models + collapsed scopes.
#[must_use]
pub fn build_task_rail_rows(
    items: &[ActivityModel],
    collapsed: &BTreeSet<ActivityScope>,
    hide_completed: bool,
) -> Vec<TaskRailRow> {
    let mut by_scope: Vec<(ActivityScope, Vec<&ActivityModel>)> = Vec::new();
    for scope in [
        ActivityScope::Workflow,
        ActivityScope::Subagent,
        ActivityScope::Foreground,
        ActivityScope::Background,
        ActivityScope::Watcher,
        ActivityScope::Completed,
    ] {
        let mut group: Vec<&ActivityModel> = items
            .iter()
            .filter(|i| i.scope == scope)
            .filter(|i| !(hide_completed && matches!(i.scope, ActivityScope::Completed)))
            .collect();
        if group.is_empty() {
            continue;
        }
        group.sort_by(|a, b| {
            a.priority_rank()
                .cmp(&b.priority_rank())
                .then_with(|| a.id.cmp(&b.id))
        });
        by_scope.push((scope, group));
    }

    let mut rows = Vec::new();
    for (scope, group) in by_scope {
        let collapsed = collapsed.contains(&scope);
        rows.push(TaskRailRow::Group {
            scope,
            count: group.len(),
            collapsed,
        });
        if collapsed {
            continue;
        }
        // Roots first, then children under parent if present in group
        let ids: BTreeSet<&str> = group.iter().map(|g| g.id.as_str()).collect();
        let mut emitted = BTreeSet::new();
        // needs_input always emit early regardless of parent order
        for item in group.iter().filter(|i| i.needs_input) {
            if emitted.insert(item.id.as_str()) {
                rows.push(TaskRailRow::Item {
                    id: item.id.clone(),
                    depth: 0,
                });
            }
        }
        for item in &group {
            if emitted.contains(item.id.as_str()) {
                continue;
            }
            if item
                .parent_id
                .as_ref()
                .is_some_and(|p| ids.contains(p.as_str()))
            {
                continue; // emit under parent
            }
            emit_tree(&mut rows, &mut emitted, item, &group, 0);
        }
    }
    rows
}

fn emit_tree<'a>(
    rows: &mut Vec<TaskRailRow>,
    emitted: &mut BTreeSet<&'a str>,
    item: &'a ActivityModel,
    group: &[&'a ActivityModel],
    depth: u8,
) {
    if !emitted.insert(item.id.as_str()) {
        return;
    }
    rows.push(TaskRailRow::Item {
        id: item.id.clone(),
        depth,
    });
    for child in group
        .iter()
        .filter(|c| c.parent_id.as_deref() == Some(item.id.as_str()))
    {
        emit_tree(rows, emitted, child, group, depth.saturating_add(1));
    }
}

/// Project to List rows (migration / workbench List path).
#[must_use]
pub fn project_task_rail_list_rows(
    items: &[ActivityModel],
    rows: &[TaskRailRow],
    ascii: bool,
    detail: bool,
) -> Vec<ListRow<'static, String>> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match row {
            TaskRailRow::Group {
                scope,
                count,
                collapsed,
            } => {
                let mark = if *collapsed {
                    if ascii { ">" } else { "▸" }
                } else if ascii {
                    "v"
                } else {
                    "▾"
                };
                let label = format!("{mark} {} ({count})", scope.title());
                out.push(ListRow::group_header(
                    format!("g:{}", scope.id()),
                    Line::from(label),
                ));
            }
            TaskRailRow::Item { id, depth } => {
                let Some(item) = items.iter().find(|i| i.id == *id) else {
                    continue;
                };
                let g = if ascii {
                    item.status.glyph_ascii()
                } else {
                    item.status.glyph_unicode()
                };
                let indent = "  ".repeat(usize::from(*depth));
                let mut title = format!("{indent}{g} {}", item.title);
                if item.needs_input {
                    title.push_str(" !");
                }
                let mut list_row = ListRow::item(item.id.clone(), Line::from(title));
                if detail {
                    let mut sec = String::new();
                    if let Some(a) = &item.actor {
                        sec.push_str(a);
                    }
                    if let Some(w) = &item.waiting_reason {
                        if !sec.is_empty() {
                            sec.push_str(" · ");
                        }
                        sec.push_str(w);
                    } else if let Some(d) = &item.detail {
                        if !sec.is_empty() {
                            sec.push_str(" · ");
                        }
                        sec.push_str(d);
                    }
                    if !item.dependencies.is_empty() {
                        if !sec.is_empty() {
                            sec.push_str(" · ");
                        }
                        sec.push_str("deps:");
                        for (i, dep) in item.dependencies.iter().take(TASK_RAIL_DEP_CAP).enumerate()
                        {
                            if i > 0 {
                                sec.push(',');
                            }
                            sec.push_str(&dep.label);
                        }
                    }
                    if !sec.is_empty() {
                        list_row.secondary = Some(Line::from(sec));
                    }
                }
                let mut trail = String::new();
                if let Some(p) = item.progress {
                    trail.push_str(&format!("{p}%"));
                } else if let Some(e) = &item.elapsed {
                    trail.push_str(e);
                }
                if !trail.is_empty() {
                    list_row.trailing = Some(Line::from(trail));
                }
                if item.needs_input {
                    list_row.badge = Some(Line::from(if ascii { "IN" } else { "input" }));
                }
                list_row.enabled = !matches!(item.status, SemanticStatus::Unknown);
                out.push(list_row);
            }
        }
    }
    out
}

// ── Presentation / state / outcomes ─────────────────────────────────────────

/// Semantic zoom for rail density.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TaskRailZoom {
    /// Status + title.
    Compact,
    /// + detail / deps / progress.
    #[default]
    Detail,
}

impl TaskRailZoom {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Detail => "detail",
        }
    }

    /// Auto from width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < TASK_RAIL_COMPACT_WIDTH {
            Self::Compact
        } else {
            Self::Detail
        }
    }
}

/// Host presentation hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TaskRailPresentation {
    /// Docked side panel.
    #[default]
    Panel,
    /// Host should host in Drawer.
    Drawer,
    /// Collapsed to StatusBar summary only.
    StatusSummary,
}

impl TaskRailPresentation {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Drawer => "drawer",
            Self::StatusSummary => "status-summary",
        }
    }

    /// Responsive recommendation.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width < TASK_RAIL_DRAWER_WIDTH {
            Self::Drawer
        } else {
            Self::Panel
        }
    }
}

/// Outcomes (requests only).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskRailOutcome {
    /// Ignored.
    Ignored,
    /// Selection moved.
    Selected {
        /// Id.
        id: String,
    },
    /// Open / jump.
    Activated {
        /// Id.
        id: String,
    },
    /// Cancel task request.
    CancelTask {
        /// Id.
        id: String,
    },
    /// Retry.
    RetryTask {
        /// Id.
        id: String,
    },
    /// Focus transcript for id.
    FocusTranscript {
        /// Id.
        id: String,
    },
    /// Inspect dependencies.
    InspectDeps {
        /// Id.
        id: String,
    },
    /// Promote / drawer / fullscreen host.
    Promote {
        /// Id.
        id: String,
    },
    /// Group collapse toggled.
    GroupToggled {
        /// Scope.
        scope: ActivityScope,
        /// Collapsed after.
        collapsed: bool,
    },
    /// Filter query changed.
    FilterChanged {
        /// Query.
        query: String,
    },
    /// Zoom changed.
    ZoomChanged {
        /// Zoom.
        zoom: TaskRailZoom,
    },
    /// Prefer drawer (responsive hint).
    PreferDrawer,
    /// Prefer status summary.
    PreferStatusSummary,
    /// Hide completed toggled.
    HideCompletedChanged {
        /// On.
        on: bool,
    },
}

/// Interactive state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRailState {
    /// Underlying list cursor (row ids are activity or group ids).
    pub list: ListState<String>,
    /// Filter / search query.
    pub filter: String,
    /// Collapsed scopes.
    pub collapsed: BTreeSet<ActivityScope>,
    /// Zoom.
    pub zoom: TaskRailZoom,
    /// Force zoom; None = auto width.
    pub force_zoom: Option<TaskRailZoom>,
    /// Hide completed group contents.
    pub hide_completed: bool,
    /// Focused.
    pub focused: bool,
    accepts_input: bool,
    /// Last recommended presentation.
    pub recommended: TaskRailPresentation,
    /// Flat rows last paint.
    pub last_rows: Vec<TaskRailRow>,
    /// Typing into filter (`/` mode).
    pub filter_mode: bool,
}

impl Default for TaskRailState {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRailState {
    /// Default: completed collapsed, detail zoom.
    #[must_use]
    pub fn new() -> Self {
        let mut collapsed = BTreeSet::new();
        collapsed.insert(ActivityScope::Completed);
        Self {
            list: ListState::default(),
            filter: String::new(),
            collapsed,
            zoom: TaskRailZoom::Detail,
            force_zoom: None,
            hide_completed: false,
            focused: true,
            accepts_input: true,
            recommended: TaskRailPresentation::Panel,
            last_rows: Vec::new(),
            filter_mode: false,
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

    /// Selected activity id (skips group headers).
    #[must_use]
    pub fn selected_activity_id(&self) -> Option<&str> {
        self.list
            .selected()
            .map(String::as_str)
            .filter(|id| !id.starts_with("g:"))
    }

    fn rebuild_rows(&mut self, items: &[ActivityModel]) -> Vec<ListRow<'static, String>> {
        let filtered: Vec<ActivityModel> = filter_activity_models(items, &self.filter)
            .into_iter()
            .cloned()
            .collect();
        let rows = build_task_rail_rows(&filtered, &self.collapsed, self.hide_completed);
        self.last_rows = rows.clone();
        let detail = matches!(self.zoom, TaskRailZoom::Detail);
        project_task_rail_list_rows(&filtered, &rows, false, detail)
    }

    /// Keys.
    pub fn handle_key(&mut self, key: KeyEvent, items: &[ActivityModel]) -> TaskRailOutcome {
        if !self.accepts_input || key.kind != KeyEventKind::Press {
            return TaskRailOutcome::Ignored;
        }

        if self.filter_mode {
            match key.code {
                KeyCode::Esc => {
                    self.filter_mode = false;
                    return TaskRailOutcome::FilterChanged {
                        query: self.filter.clone(),
                    };
                }
                KeyCode::Enter => {
                    self.filter_mode = false;
                    return TaskRailOutcome::FilterChanged {
                        query: self.filter.clone(),
                    };
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    return TaskRailOutcome::FilterChanged {
                        query: self.filter.clone(),
                    };
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.filter.push(c);
                    return TaskRailOutcome::FilterChanged {
                        query: self.filter.clone(),
                    };
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Char('/') if key.modifiers.is_empty() => {
                self.filter_mode = true;
                return TaskRailOutcome::FilterChanged {
                    query: self.filter.clone(),
                };
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.clear();
                self.filter_mode = false;
                return TaskRailOutcome::FilterChanged {
                    query: String::new(),
                };
            }
            KeyCode::Char('z') if key.modifiers.is_empty() => {
                self.zoom = match self.zoom {
                    TaskRailZoom::Compact => TaskRailZoom::Detail,
                    TaskRailZoom::Detail => TaskRailZoom::Compact,
                };
                self.force_zoom = Some(self.zoom);
                return TaskRailOutcome::ZoomChanged { zoom: self.zoom };
            }
            KeyCode::Char('H') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.hide_completed = !self.hide_completed;
                return TaskRailOutcome::HideCompletedChanged {
                    on: self.hide_completed,
                };
            }
            KeyCode::Char('h') if key.modifiers.is_empty() => {
                // collapse selected group or selected item's scope
                if let Some(id) = self.list.selected() {
                    if let Some(scope_id) = id.strip_prefix("g:") {
                        if let Some(scope) = scope_from_id(scope_id) {
                            return self.toggle_scope(scope);
                        }
                    } else if let Some(item) = items.iter().find(|i| i.id == *id) {
                        return self.toggle_scope(item.scope);
                    }
                }
            }
            KeyCode::Char('l') if key.modifiers.is_empty() => {
                if let Some(id) = self.list.selected() {
                    if let Some(scope_id) = id.strip_prefix("g:") {
                        if let Some(scope) = scope_from_id(scope_id) {
                            if self.collapsed.remove(&scope) {
                                return TaskRailOutcome::GroupToggled {
                                    scope,
                                    collapsed: false,
                                };
                            }
                        }
                    }
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(id) = self.selected_activity_id() {
                    return TaskRailOutcome::CancelTask { id: id.to_string() };
                }
            }
            KeyCode::Char('r') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_activity_id() {
                    return TaskRailOutcome::RetryTask { id: id.to_string() };
                }
            }
            KeyCode::Char('t') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_activity_id() {
                    return TaskRailOutcome::FocusTranscript { id: id.to_string() };
                }
            }
            KeyCode::Char('d') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_activity_id() {
                    return TaskRailOutcome::InspectDeps { id: id.to_string() };
                }
            }
            KeyCode::Char('f') if key.modifiers.is_empty() => {
                if let Some(id) = self.selected_activity_id() {
                    return TaskRailOutcome::Promote { id: id.to_string() };
                }
                return TaskRailOutcome::PreferDrawer;
            }
            KeyCode::Tab if key.modifiers.is_empty() => {
                // jump to first needs_input
                if let Some(item) = sort_activity_models(items)
                    .into_iter()
                    .find(|i| i.needs_input)
                {
                    self.list.select(Some(item.id.clone()));
                    return TaskRailOutcome::Selected {
                        id: item.id.clone(),
                    };
                }
            }
            _ => {}
        }

        let list_rows = self.rebuild_rows(items);
        // Space on group header toggles
        if matches!(key.code, KeyCode::Char(' ') | KeyCode::Enter) {
            if let Some(id) = self.list.selected() {
                if let Some(scope_id) = id.strip_prefix("g:") {
                    if let Some(scope) = scope_from_id(scope_id) {
                        return self.toggle_scope(scope);
                    }
                }
            }
        }

        use crate::interaction::Outcome;
        match self.list.handle_key(&list_rows, key) {
            Outcome::Activated(id) => {
                if let Some(scope_id) = id.strip_prefix("g:") {
                    if let Some(scope) = scope_from_id(scope_id) {
                        return self.toggle_scope(scope);
                    }
                    TaskRailOutcome::Ignored
                } else {
                    TaskRailOutcome::Activated { id }
                }
            }
            Outcome::Changed => {
                if let Some(id) = self.list.selected() {
                    if id.starts_with("g:") {
                        TaskRailOutcome::Ignored
                    } else {
                        TaskRailOutcome::Selected { id: id.clone() }
                    }
                } else {
                    TaskRailOutcome::Ignored
                }
            }
            Outcome::Cancelled => TaskRailOutcome::Ignored,
            Outcome::Ignored | Outcome::CheckToggled(_) => TaskRailOutcome::Ignored,
        }
    }

    fn toggle_scope(&mut self, scope: ActivityScope) -> TaskRailOutcome {
        let collapsed = if self.collapsed.contains(&scope) {
            self.collapsed.remove(&scope);
            false
        } else {
            self.collapsed.insert(scope);
            true
        };
        TaskRailOutcome::GroupToggled { scope, collapsed }
    }

    /// Mouse via list hits after paint (call after `paint` so regions exist).
    pub fn handle_mouse(&mut self, event: MouseEvent, _items: &[ActivityModel]) -> TaskRailOutcome {
        if !self.accepts_input || event.kind != MouseEventKind::Down(MouseButton::Left) {
            return TaskRailOutcome::Ignored;
        }
        use crate::interaction::Outcome;
        match self.list.click(event.position) {
            Outcome::Activated(id) => {
                if let Some(scope_id) = id.strip_prefix("g:") {
                    if let Some(scope) = scope_from_id(scope_id) {
                        return self.toggle_scope(scope);
                    }
                    return TaskRailOutcome::Ignored;
                }
                TaskRailOutcome::Activated { id }
            }
            Outcome::Changed => {
                if let Some(id) = self.list.selected() {
                    if id.starts_with("g:") {
                        TaskRailOutcome::Ignored
                    } else {
                        TaskRailOutcome::Selected { id: id.clone() }
                    }
                } else {
                    TaskRailOutcome::Ignored
                }
            }
            _ => TaskRailOutcome::Ignored,
        }
    }
}

fn scope_from_id(id: &str) -> Option<ActivityScope> {
    match id {
        "workflow" => Some(ActivityScope::Workflow),
        "subagent" => Some(ActivityScope::Subagent),
        "foreground" => Some(ActivityScope::Foreground),
        "background" => Some(ActivityScope::Background),
        "watcher" => Some(ActivityScope::Watcher),
        "completed" => Some(ActivityScope::Completed),
        _ => None,
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Unified task / activity rail.
#[derive(Debug, Clone, Copy)]
pub struct TaskRail<'a> {
    items: &'a [ActivityModel],
    system: &'a DesignSystem,
    title: &'a str,
    ascii: bool,
    colorless: bool,
}

impl<'a> TaskRail<'a> {
    /// Models + system.
    #[must_use]
    pub const fn new(items: &'a [ActivityModel], system: &'a DesignSystem) -> Self {
        Self {
            items,
            system,
            title: "Tasks",
            ascii: false,
            colorless: false,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    /// ASCII.
    #[must_use]
    pub const fn ascii(mut self, on: bool) -> Self {
        self.ascii = on;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, on: bool) -> Self {
        self.colorless = on;
        self
    }

    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut TaskRailState) {
        if area.is_empty() {
            return;
        }
        state.recommended = TaskRailPresentation::for_width(area.width);
        if state.force_zoom.is_none() {
            state.zoom = TaskRailZoom::for_width(area.width);
        } else if let Some(z) = state.force_zoom {
            state.zoom = z;
        }

        // Status-summary only: one line
        if matches!(state.recommended, TaskRailPresentation::StatusSummary) && area.height <= 1 {
            let s = task_rail_status_summary(self.items, self.ascii);
            self.system.paint_row(
                buffer,
                Rect::new(area.x, area.y, area.width, 1),
                &s,
                self.system.style(Role::TextMuted),
            );
            return;
        }

        let emphasis = if state.focused {
            PanelChrome::Focused
        } else {
            PanelChrome::Normal
        };
        let counts = task_rail_counts(self.items);
        let mut title = self.title.to_string();
        if counts.needs_input > 0 {
            title = format!("{} · {} input", self.title, counts.needs_input);
        }
        if state.filter_mode || !state.filter.is_empty() {
            title = format!("{title} /{}", state.filter);
        }
        let panel = Panel::new(self.system)
            .title(title.as_str())
            .emphasis(emphasis);
        let inner = panel.inner(area);
        use ratatui_core::widgets::Widget;
        Widget::render(&panel, area, buffer);

        if inner.is_empty() {
            return;
        }

        // footer counts row
        let body_h = inner.height.saturating_sub(1);
        let list_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: body_h.max(1),
        };
        let footer_y = inner.bottom().saturating_sub(1);

        let filtered: Vec<ActivityModel> = filter_activity_models(self.items, &state.filter)
            .into_iter()
            .cloned()
            .collect();
        let rows = build_task_rail_rows(&filtered, &state.collapsed, state.hide_completed);
        state.last_rows = rows.clone();
        let detail = matches!(state.zoom, TaskRailZoom::Detail);
        let list_rows =
            project_task_rail_list_rows(&filtered, &rows, self.ascii || self.colorless, detail);

        StatefulWidget::render(
            &List::new(&list_rows, self.system).focused(state.focused && state.accepts_input),
            list_area,
            buffer,
            &mut state.list,
        );

        if inner.height >= 2 {
            let foot = format!(
                "{} · {} run · {} block · {} fail",
                counts.total, counts.running, counts.blocked, counts.failed
            );
            let mut style = self.system.style(Role::TextMuted);
            if counts.needs_input > 0 && !self.colorless {
                style = self.system.style(Role::Warning);
            }
            if self.colorless && state.focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
            self.system.paint_row(
                buffer,
                Rect::new(inner.x, footer_y, inner.width, 1),
                &foot,
                style,
            );
        }
        let _ = display_cols;
    }

    /// Render alias.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut TaskRailState) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for &TaskRail<'_> {
    type State = TaskRailState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

impl StatefulWidget for TaskRail<'_> {
    type State = TaskRailState;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        self.paint(area, buffer, state);
    }
}

// ── Examples ────────────────────────────────────────────────────────────────

/// Demo activity inventory.
#[must_use]
pub fn example_activity_models() -> Vec<ActivityModel> {
    vec![
        ActivityModel::new("w1", "ship feature")
            .scope(ActivityScope::Workflow)
            .status(SemanticStatus::Running)
            .progress(40)
            .actor("agent")
            .elapsed("2m"),
        ActivityModel::new("p1", "allow: cargo publish")
            .scope(ActivityScope::Foreground)
            .kind(ActivityKind::Shell)
            .needs_input(true)
            .waiting_reason("permission")
            .actor("agent")
            .elapsed("15s"),
        ActivityModel::new("s1", "subagent:review")
            .scope(ActivityScope::Subagent)
            .kind(ActivityKind::Subagent)
            .status(SemanticStatus::Running)
            .parent("w1")
            .elapsed("40s")
            .progress(70),
        ActivityModel::new("b1", "cargo test")
            .scope(ActivityScope::Background)
            .kind(ActivityKind::Build)
            .status(SemanticStatus::Running)
            .progress(55)
            .elapsed("1.1s")
            .depend(ActivityDependency::new("s1", "review")),
        ActivityModel::new("bg2", "index workspace")
            .scope(ActivityScope::Background)
            .kind(ActivityKind::Search)
            .blocked(true)
            .waiting_reason("lock held")
            .elapsed("30s"),
        ActivityModel::new("wt1", "watch src/**")
            .scope(ActivityScope::Watcher)
            .kind(ActivityKind::Tool)
            .status(SemanticStatus::Running)
            .actor("host"),
        ActivityModel::new("c1", "lint")
            .scope(ActivityScope::Completed)
            .kind(ActivityKind::Tool)
            .status(SemanticStatus::Failed)
            .elapsed("0.3s")
            .detail("exit 1"),
        ActivityModel::new("c2", "fmt")
            .scope(ActivityScope::Completed)
            .status(SemanticStatus::Success)
            .elapsed("0.1s"),
    ]
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Paint stress.
pub mod bench {
    /// Items.
    pub const ITEM_COUNT: usize = 64;
    /// Frames.
    pub const PAINT_FRAMES: u32 = 24;
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_input_sorted_first_in_priority() {
        let items = example_activity_models();
        let s = sort_activity_models(&items);
        assert_eq!(s[0].id, "p1");
        assert!(s[0].needs_input);
    }

    #[test]
    fn groups_and_collapse() {
        let items = example_activity_models();
        let mut collapsed = BTreeSet::new();
        collapsed.insert(ActivityScope::Completed);
        let rows = build_task_rail_rows(&items, &collapsed, false);
        assert!(rows.iter().any(|r| matches!(
            r,
            TaskRailRow::Group {
                scope: ActivityScope::Completed,
                collapsed: true,
                ..
            }
        )));
        assert!(!rows.iter().any(|r| matches!(
            r,
            TaskRailRow::Item { id, .. } if id == "c1" || id == "c2"
        )));
    }

    #[test]
    fn filter_search() {
        let items = example_activity_models();
        let f = filter_activity_models(&items, "review");
        assert!(f.iter().any(|i| i.id == "s1"));
        let f2 = filter_activity_models(&items, "permission");
        assert!(f2.iter().any(|i| i.id == "p1"));
    }

    #[test]
    fn tree_parent_indent() {
        let items = example_activity_models();
        let rows = build_task_rail_rows(&items, &BTreeSet::new(), true);
        let s1 = rows.iter().find_map(|r| match r {
            TaskRailRow::Item { id, depth } if id == "s1" => Some(*depth),
            _ => None,
        });
        // s1 parent w1 is Workflow scope — different group, so depth 0 in Subagent group
        assert_eq!(s1, Some(0));
    }

    #[test]
    fn keyboard_cancel_retry_tab_input() {
        let items = example_activity_models();
        let mut st = TaskRailState::new();
        st.set_focused(true);
        // paint builds list state
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 36, 20);
        let mut buf = Buffer::empty(area);
        TaskRail::new(&items, &system).paint(area, &mut buf, &mut st);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE), &items),
            TaskRailOutcome::Selected { id } if id == "p1"
        ));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), &items),
            TaskRailOutcome::CancelTask { id } if id == "p1"
        ));
        st.list.select(Some("c1".into()));
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE), &items),
            TaskRailOutcome::RetryTask { id } if id == "c1"
        ));
    }

    #[test]
    fn filter_mode_and_zoom() {
        let items = example_activity_models();
        let mut st = TaskRailState::new();
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
                &items
            ),
            TaskRailOutcome::FilterChanged { .. }
        ));
        assert!(st.filter_mode);
        let _ = st.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            &items,
        );
        let _ = st.handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &items,
        );
        assert!(st.filter.contains('c') || st.filter.contains('a'));
        st.filter_mode = false;
        assert!(matches!(
            st.handle_key(
                KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
                &items
            ),
            TaskRailOutcome::ZoomChanged { .. }
        ));
    }

    #[test]
    fn group_toggle() {
        let items = example_activity_models();
        let mut st = TaskRailState::new();
        let system = DesignSystem::default();
        let area = Rect::new(0, 0, 40, 18);
        let mut buf = Buffer::empty(area);
        TaskRail::new(&items, &system).paint(area, &mut buf, &mut st);
        st.list.select(Some("g:background".into()));
        let out = st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items);
        assert!(matches!(
            out,
            TaskRailOutcome::GroupToggled {
                scope: ActivityScope::Background,
                ..
            }
        ));
    }

    #[test]
    fn shelf_and_statusbar_bridge() {
        let items = example_activity_models();
        let shelf = activity_models_to_shelf(&items);
        assert!(shelf.iter().any(|i| i.action_required));
        let p = project_task_rail_for_status_bar(&items, true);
        assert!(p.priority >= 90);
        assert!(
            p.summary.contains("action")
                || p.summary.contains("input")
                || p.summary.contains("run")
        );
        let slot = task_rail_status_slot("tasks", &p, false);
        assert!(!slot.content.is_empty());
    }

    #[test]
    fn list_projection_has_groups() {
        let items = example_activity_models();
        let rows = build_task_rail_rows(&items, &BTreeSet::new(), false);
        let list = project_task_rail_list_rows(&items, &rows, true, true);
        assert!(list.iter().any(|r| r.role == RowRole::GroupHeader));
        assert!(list.iter().any(|r| r.id == "p1"));
    }

    #[test]
    fn responsive_presentation() {
        assert_eq!(
            TaskRailPresentation::for_width(40),
            TaskRailPresentation::Drawer
        );
        assert_eq!(
            TaskRailPresentation::for_width(80),
            TaskRailPresentation::Panel
        );
        assert_eq!(TaskRailZoom::for_width(10), TaskRailZoom::Compact);
    }

    #[test]
    fn paint_all_and_narrow() {
        let system = DesignSystem::default();
        let items = example_activity_models();
        let mut st = TaskRailState::new();
        let area = Rect::new(0, 0, 40, 16);
        let mut buf = Buffer::empty(area);
        TaskRail::new(&items, &system)
            .title("Tasks")
            .paint(area, &mut buf, &mut st);
        assert!(!st.last_rows.is_empty());
        let narrow = Rect::new(0, 0, 14, 12);
        TaskRail::new(&items, &system)
            .ascii(true)
            .paint(narrow, &mut buf, &mut st);
        assert_eq!(st.recommended, TaskRailPresentation::Drawer);
    }

    #[test]
    fn never_process_pty() {
        let src = include_str!("task_rail.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for f in [
            "std::process",
            "Command::new",
            "portable_pty",
            "openai",
            "anthropic",
        ] {
            assert!(!body.contains(f), "{f}");
        }
    }

    #[test]
    fn accepts_input_gate() {
        let items = example_activity_models();
        let mut st = TaskRailState::new();
        st.set_accepts_input(false);
        assert!(matches!(
            st.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items),
            TaskRailOutcome::Ignored
        ));
    }

    #[test]
    fn paint_perf_budget() {
        let system = DesignSystem::default();
        let mut items = example_activity_models();
        for i in 0..bench::ITEM_COUNT {
            items.push(
                ActivityModel::new(format!("x{i}"), format!("job {i}"))
                    .scope(if i % 2 == 0 {
                        ActivityScope::Background
                    } else {
                        ActivityScope::Foreground
                    })
                    .status(SemanticStatus::Running)
                    .progress((i % 100) as u8),
            );
        }
        let area = Rect::new(0, 0, 36, 24);
        let mut buf = Buffer::empty(area);
        let start = std::time::Instant::now();
        for _ in 0..bench::PAINT_FRAMES {
            let mut st = TaskRailState::new();
            TaskRail::new(&items, &system).paint(area, &mut buf, &mut st);
        }
        assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    }

    #[test]
    fn fuzz_scopes_and_actions() {
        for s in [
            ActivityScope::Workflow,
            ActivityScope::Subagent,
            ActivityScope::Foreground,
            ActivityScope::Background,
            ActivityScope::Watcher,
            ActivityScope::Completed,
        ] {
            assert!(!s.id().is_empty());
            assert!(!s.title().is_empty());
        }
        let m = ActivityModel::new("a", "t")
            .needs_input(true)
            .depend(ActivityDependency::new("b", "dep"));
        assert!(!m.default_actions().is_empty());
        let _ = activity_model_from_shelf(&m.to_shelf_item());
    }

    #[test]
    fn thin_list_facade_removed_from_agent_blocks() {
        let src = include_str!("../widgets/agent_blocks.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(
            !body.contains("pub struct TaskRail"),
            "TaskRail moved to task_rail.rs"
        );
    }
}
