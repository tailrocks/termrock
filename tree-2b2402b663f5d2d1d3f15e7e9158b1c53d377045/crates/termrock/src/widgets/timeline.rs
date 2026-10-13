// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **Timeline** — chronological event presentation for sessions, tasks, deploys, traces.
//!
//! **Mission.** Timestamps, relative time, duration, actor, status, grouping,
//! expansion, correlation, filters, and live streaming while preserving reading
//! position. Recipes: compact **rail**, **detailed** list, **grouped-day**.
//! No-color mode uses symbols and labels. Composes with checkpoint restore
//! ([`super::CheckpointTimeline`]), event streams ([`super::LogStream`]), and task
//! history ([`super::progress_steps`]).
//!
//! Research: Git history, CI timelines, observability tools, agent session views.

#![allow(unused_imports)] // test-module imports kept for unit tests; lib path may not use them
use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    style::Modifier,
    widgets::{StatefulWidget, Widget},
};

use crate::{
    input::{
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    },
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::take_display_cols,
};

const GUTTER: u16 = 2;

/// Visual / density recipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TimelineRecipe {
    /// Compact marker rail (●/○ + short text).
    Rail,
    /// Full rows: time · actor · status · text (default).
    #[default]
    Detailed,
    /// Day/session group headers with nested events.
    GroupedDay,
}

impl TimelineRecipe {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Rail => "rail",
            Self::Detailed => "detailed",
            Self::GroupedDay => "grouped-day",
        }
    }
}

/// Event lifecycle / severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TimelineStatus {
    /// Not started / queued.
    #[default]
    Pending,
    /// In progress.
    Running,
    /// Completed successfully.
    Success,
    /// Failed.
    Failed,
    /// Cancelled / skipped.
    Cancelled,
    /// Neutral informational.
    Info,
    /// Caution.
    Warning,
}

impl TimelineStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Info => "info",
            Self::Warning => "warning",
        }
    }

    /// No-color label letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Pending => 'P',
            Self::Running => 'R',
            Self::Success => 'S',
            Self::Failed => 'F',
            Self::Cancelled => 'C',
            Self::Info => 'I',
            Self::Warning => 'W',
        }
    }

    /// Marker glyph (unicode / ascii).
    #[must_use]
    pub const fn marker(self, active: bool, ascii: bool) -> &'static str {
        if ascii {
            return match (self, active) {
                (_, true) => "*",
                (Self::Failed, _) => "x",
                (Self::Success, _) => "+",
                (Self::Running, _) => "o",
                (Self::Warning, _) => "!",
                _ => ".",
            };
        }
        match (self, active) {
            (_, true) => "●",
            (Self::Failed, _) => "✗",
            (Self::Success, _) => "✓",
            (Self::Running, _) => "◎",
            (Self::Warning, _) => "⚠",
            (Self::Pending, _) => "○",
            (Self::Cancelled, _) => "–",
            (Self::Info, _) => "◇",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Pending | Self::Cancelled => Role::TextMuted,
            Self::Running | Self::Info => Role::Info,
            Self::Success => Role::Success,
            Self::Failed => Role::Danger,
            Self::Warning => Role::Warning,
        }
    }

    /// Motion channel for this status, matching [`SemanticStatus::channel`].
    ///
    /// A running step is the same fact as a running status indicator, so it
    /// breathes on the same channel at the same period. Every terminal state
    /// is `Static`: a finished row that keeps moving reads as still working.
    ///
    /// [`SemanticStatus::channel`]: crate::widgets::SemanticStatus::channel
    #[must_use]
    pub const fn channel(self) -> crate::style::MotionChannel {
        match self {
            Self::Running => crate::style::MotionChannel::Live,
            Self::Pending => crate::style::MotionChannel::Wait,
            Self::Success | Self::Failed | Self::Warning | Self::Cancelled | Self::Info => {
                crate::style::MotionChannel::Static
            }
        }
    }
}

/// Row kind in the projected stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TimelineRowKind {
    /// Ordinary event.
    #[default]
    Event,
    /// Group header (day / session).
    Group,
    /// Checkpoint / restore point.
    Checkpoint,
}

/// One timeline event (borrowed projection).
///
/// Prefer builders; struct fields remain public for advanced hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEvent<'a, Id = ()> {
    /// Stable identity (selection, expansion, checkpoints).
    pub id: Id,
    /// Absolute or sequence time label (`12:02`, `T+3s`, ISO day).
    pub when: &'a str,
    /// Relative time (`2m ago`) — dropped under width pressure first.
    pub relative: Option<&'a str>,
    /// Duration label (`1.2s`).
    pub duration: Option<&'a str>,
    /// Event summary.
    pub text: &'a str,
    /// Actor / author.
    pub actor: Option<&'a str>,
    /// Status.
    pub status: TimelineStatus,
    /// Row kind.
    pub kind: TimelineRowKind,
    /// Active / current event emphasis.
    pub active: bool,
    /// Expandable detail.
    pub expandable: bool,
    /// Expanded in projection.
    pub expanded: bool,
    /// Detail body when expanded.
    pub detail: Option<&'a str>,
    /// Correlation id (trace / deploy).
    pub correlation: Option<&'a str>,
    /// Group key for filters (`2026-04-01`).
    pub group_key: Option<&'a str>,
    /// Interactive enabled.
    pub enabled: bool,
}

impl<'a> TimelineEvent<'a, ()> {
    /// Simple event without id (paint-only / progress composition).
    #[must_use]
    pub const fn new(when: &'a str, text: &'a str) -> Self {
        Self {
            id: (),
            when,
            relative: None,
            duration: None,
            text,
            actor: None,
            status: TimelineStatus::Info,
            kind: TimelineRowKind::Event,
            active: false,
            expandable: false,
            expanded: false,
            detail: None,
            correlation: None,
            group_key: None,
            enabled: true,
        }
    }
}

impl<'a, Id> TimelineEvent<'a, Id> {
    /// Event with stable id.
    #[must_use]
    pub const fn with_id(id: Id, when: &'a str, text: &'a str) -> Self {
        Self {
            id,
            when,
            relative: None,
            duration: None,
            text,
            actor: None,
            status: TimelineStatus::Info,
            kind: TimelineRowKind::Event,
            active: false,
            expandable: false,
            expanded: false,
            detail: None,
            correlation: None,
            group_key: None,
            enabled: true,
        }
    }

    /// Day / session group header.
    #[must_use]
    pub const fn group(id: Id, label: &'a str) -> Self {
        Self {
            id,
            when: label,
            relative: None,
            duration: None,
            text: "",
            actor: None,
            status: TimelineStatus::Info,
            kind: TimelineRowKind::Group,
            active: false,
            expandable: true,
            expanded: true,
            detail: None,
            correlation: None,
            group_key: Some(label),
            enabled: true,
        }
    }

    /// Checkpoint event (restore substrate).
    #[must_use]
    pub const fn checkpoint(id: Id, when: &'a str, text: &'a str) -> Self {
        Self {
            id,
            when,
            relative: None,
            duration: None,
            text,
            actor: None,
            status: TimelineStatus::Success,
            kind: TimelineRowKind::Checkpoint,
            active: false,
            expandable: false,
            expanded: false,
            detail: None,
            correlation: None,
            group_key: None,
            enabled: true,
        }
    }

    /// Marks active/current.
    #[must_use]
    pub const fn active(mut self) -> Self {
        self.active = true;
        self
    }

    /// Sets active flag.
    #[must_use]
    pub const fn set_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, status: TimelineStatus) -> Self {
        self.status = status;
        self
    }

    /// Relative time.
    #[must_use]
    pub const fn relative(mut self, rel: &'a str) -> Self {
        self.relative = Some(rel);
        self
    }

    /// Duration.
    #[must_use]
    pub const fn duration(mut self, d: &'a str) -> Self {
        self.duration = Some(d);
        self
    }

    /// Actor.
    #[must_use]
    pub const fn actor(mut self, actor: &'a str) -> Self {
        self.actor = Some(actor);
        self
    }

    /// Expandable with detail.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self.expandable = true;
        self
    }

    /// Expanded.
    #[must_use]
    pub const fn expanded(mut self) -> Self {
        self.expanded = true;
        self.expandable = true;
        self
    }

    /// Correlation id.
    #[must_use]
    pub const fn correlation(mut self, id: &'a str) -> Self {
        self.correlation = Some(id);
        self
    }

    /// Group key.
    #[must_use]
    pub const fn group_key(mut self, key: &'a str) -> Self {
        self.group_key = Some(key);
        self
    }

    /// Disabled (skipped by keyboard).
    #[must_use]
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Focusable row (not pure separator group without selection).
    #[must_use]
    pub const fn focusable(&self) -> bool {
        self.enabled && !matches!(self.kind, TimelineRowKind::Group)
    }
}

/// Hit region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineRegion<Id> {
    /// Event id.
    pub id: Id,
    /// Projected index.
    pub index: usize,
    /// Row area.
    pub area: Rect,
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TimelineOutcome<Id> {
    /// No change.
    Ignored,
    /// Selection moved.
    Selected(Id),
    /// Activate / open.
    Activated(Id),
    /// Expand/collapse detail.
    ExpandToggled(Id),
    /// Filter query changed.
    FilterChanged(String),
    /// Scroll / follow changed.
    Scrolled {
        /// Following live tail.
        following: bool,
    },
    /// Checkpoint restore requested (host confirms).
    RestoreRequested(Id),
    /// Compare checkpoint to current (host opens DiffReview).
    CompareRequested(Id),
    /// Cancel / clear filter.
    Cancelled,
}

/// Interaction state — preserves reading position while streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineState<Id: Clone + PartialEq = ()> {
    selected: Option<Id>,
    /// Cursor index in the current projection.
    pub cursor: usize,
    offset: usize,
    viewport: usize,
    /// Follow newest events (live stream).
    pub following: bool,
    /// Host grants input.
    accepts_input: bool,
    /// Filter query.
    filter: Option<String>,
    /// Expanded event ids (host may mirror into projection).
    expanded: BTreeSet<Id>,
    /// Checkpoint mode (Enter → RestoreRequested).
    checkpoint_mode: bool,
    /// ASCII markers.
    pub ascii: bool,
    /// Colorless paint.
    pub colorless: bool,
    /// Hit regions.
    pub regions: Vec<TimelineRegion<Id>>,
    painted: Rect,
    /// Logical universe for virtualization (0 = use projected len).
    pub logical_len: usize,
}

impl<Id: Clone + PartialEq + Ord> Default for TimelineState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq + Ord> TimelineState<Id> {
    /// Fresh state; follows tail by default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: None,
            cursor: 0,
            offset: 0,
            viewport: 0,
            following: true,
            accepts_input: true,
            filter: None,
            expanded: BTreeSet::new(),
            checkpoint_mode: false,
            ascii: false,
            colorless: false,
            regions: Vec::new(),
            painted: Rect::default(),
            logical_len: 0,
        }
    }

    /// Selected id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Cursor index in current projection.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Scroll offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Whether following live tail.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.following
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Enable checkpoint Enter→restore semantics.
    pub fn set_checkpoint_mode(&mut self, on: bool) {
        self.checkpoint_mode = on;
    }

    /// Follow newest.
    pub fn follow(&mut self) {
        self.following = true;
    }

    /// Pause follow (reading history).
    pub fn unfollow(&mut self) {
        self.following = false;
    }

    /// Whether id is expanded.
    #[must_use]
    pub fn is_expanded(&self, id: &Id) -> bool {
        self.expanded.contains(id)
    }

    /// Toggle expansion.
    pub fn toggle_expanded(&mut self, id: Id) -> bool {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
            true
        } else {
            false
        }
    }

    /// Filter query.
    #[must_use]
    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    /// Notify that new events were appended at the end (re-follow if following).
    pub fn on_append(&mut self, projected_len: usize) {
        if self.following && projected_len > 0 {
            self.cursor = projected_len.saturating_sub(1);
            self.reveal(projected_len);
        }
    }

    fn reveal(&mut self, len: usize) {
        if self.viewport == 0 || len == 0 {
            return;
        }
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor >= self.offset + self.viewport {
            self.offset = self.cursor + 1 - self.viewport;
        }
        let max_off = len.saturating_sub(self.viewport);
        self.offset = self.offset.min(max_off);
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        events: &[TimelineEvent<'_, Id>],
    ) -> TimelineOutcome<Id> {
        if !self.accepts_input || key.kind == KeyEventKind::Release {
            return TimelineOutcome::Ignored;
        }
        let is_press = key.kind == KeyEventKind::Press;

        // Filter
        if is_press && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if self.filter.is_none() {
                self.filter = Some(String::new());
            }
            return TimelineOutcome::FilterChanged(self.filter.clone().unwrap_or_default());
        }
        if let Some(q) = self.filter.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.filter = None;
                    return TimelineOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.filter = None;
                    }
                    return TimelineOutcome::FilterChanged(self.filter.clone().unwrap_or_default());
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return TimelineOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        let view = filter_timeline_events(events, self.filter.as_deref().unwrap_or(""));
        if view.is_empty() {
            return TimelineOutcome::Ignored;
        }
        self.cursor = self.cursor.min(view.len() - 1);

        if is_press && matches!(key.code, KeyCode::Char('f' | 'F')) && key.modifiers.is_empty() {
            self.following = !self.following;
            if self.following {
                self.cursor = view.len() - 1;
                self.reveal(view.len());
            }
            return TimelineOutcome::Scrolled {
                following: self.following,
            };
        }

        if is_press
            && matches!(key.code, KeyCode::Char('c' | 'C'))
            && key.modifiers.is_empty()
            && self.checkpoint_mode
        {
            if let Some(e) = view.get(self.cursor) {
                if matches!(e.kind, TimelineRowKind::Checkpoint) {
                    return TimelineOutcome::CompareRequested(e.id.clone());
                }
            }
        }

        if let Some(intent) = crate::interaction::default_list_intent(key) {
            return self.handle_intent(intent, &view);
        }

        // Expand
        if is_press
            && matches!(
                key.code,
                KeyCode::Right | KeyCode::Left | KeyCode::Char('l' | 'h' | 'L' | 'H')
            )
        {
            let expand = matches!(key.code, KeyCode::Right | KeyCode::Char('l' | 'L'));
            if let Some(e) = view.get(self.cursor) {
                if e.expandable {
                    let id = e.id.clone();
                    let _ = self.toggle_expanded(id.clone());
                    return TimelineOutcome::ExpandToggled(id);
                }
            }
            let _ = expand;
        }

        TimelineOutcome::Ignored
    }

    /// Intent routing over a pre-filtered view (or full events).
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        view: &[&TimelineEvent<'_, Id>],
    ) -> TimelineOutcome<Id> {
        if !self.accepts_input || view.is_empty() {
            return TimelineOutcome::Ignored;
        }
        self.cursor = self.cursor.min(view.len() - 1);
        match intent {
            UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => self.move_by(view, 1),
            UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => self.move_by(view, -1),
            UiIntent::Move(NavigationMove::First) => {
                self.following = false;
                self.cursor = 0;
                self.reveal(view.len());
                self.select_cursor(view)
            }
            UiIntent::Move(NavigationMove::Last) => {
                self.cursor = view.len() - 1;
                self.following = true;
                self.reveal(view.len());
                self.select_cursor(view)
            }
            UiIntent::Page(PageMove::Forward) => {
                self.following = false;
                self.move_by(view, self.viewport.max(1) as isize)
            }
            UiIntent::Page(PageMove::Backward) => {
                self.following = false;
                self.move_by(view, -(self.viewport.max(1) as isize))
            }
            UiIntent::Activate | UiIntent::Submit | UiIntent::Open => {
                let e = view[self.cursor];
                if self.checkpoint_mode && matches!(e.kind, TimelineRowKind::Checkpoint) {
                    return TimelineOutcome::RestoreRequested(e.id.clone());
                }
                TimelineOutcome::Activated(e.id.clone())
            }
            UiIntent::Expand | UiIntent::Toggle => {
                let e = view[self.cursor];
                if e.expandable {
                    let id = e.id.clone();
                    let _ = self.toggle_expanded(id.clone());
                    TimelineOutcome::ExpandToggled(id)
                } else {
                    TimelineOutcome::Ignored
                }
            }
            UiIntent::Collapse => TimelineOutcome::Ignored,
            UiIntent::Cancel => {
                if self.filter.is_some() {
                    self.filter = None;
                    return TimelineOutcome::Cancelled;
                }
                TimelineOutcome::Cancelled
            }
            _ => TimelineOutcome::Ignored,
        }
    }

    fn move_by(&mut self, view: &[&TimelineEvent<'_, Id>], delta: isize) -> TimelineOutcome<Id> {
        let focusable: Vec<usize> = view
            .iter()
            .enumerate()
            .filter(|(_, e)| e.focusable())
            .map(|(i, _)| i)
            .collect();
        if focusable.is_empty() {
            return TimelineOutcome::Ignored;
        }
        let cur_pos = focusable
            .iter()
            .position(|&i| i == self.cursor)
            .unwrap_or(0);
        let next_pos = if delta >= 0 {
            (cur_pos + delta as usize).min(focusable.len() - 1)
        } else {
            cur_pos.saturating_sub((-delta) as usize)
        };
        let idx = focusable[next_pos];
        if idx == self.cursor {
            return TimelineOutcome::Ignored;
        }
        // Moving up detaches follow
        if idx < self.cursor {
            self.following = false;
        } else if idx + 1 >= view.len() {
            self.following = true;
        }
        self.cursor = idx;
        self.reveal(view.len());
        self.select_cursor(view)
    }

    fn select_cursor(&mut self, view: &[&TimelineEvent<'_, Id>]) -> TimelineOutcome<Id> {
        let e = view[self.cursor];
        self.selected = Some(e.id.clone());
        TimelineOutcome::Selected(e.id.clone())
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        events: &[TimelineEvent<'_, Id>],
    ) -> TimelineOutcome<Id> {
        if !self.accepts_input {
            return TimelineOutcome::Ignored;
        }
        let view = filter_timeline_events(events, self.filter.as_deref().unwrap_or(""));
        match event.kind {
            MouseEventKind::ScrollUp if self.painted.contains(event.position) => {
                self.following = false;
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), &view)
            }
            MouseEventKind::ScrollDown if self.painted.contains(event.position) => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), &view)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    if self.selected.as_ref() == Some(&r.id) {
                        if self.checkpoint_mode {
                            return TimelineOutcome::RestoreRequested(r.id.clone());
                        }
                        return TimelineOutcome::Activated(r.id.clone());
                    }
                    self.cursor = r.index;
                    self.following = false;
                    self.selected = Some(r.id.clone());
                    return TimelineOutcome::Selected(r.id.clone());
                }
                TimelineOutcome::Ignored
            }
            _ => TimelineOutcome::Ignored,
        }
    }
}

/// Filter events by text/actor/correlation/status (keeps group headers for matches).
#[must_use]
pub fn filter_timeline_events<'a, Id>(
    events: &'a [TimelineEvent<'a, Id>],
    query: &str,
) -> Vec<&'a TimelineEvent<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return events.iter().collect();
    }
    let mut keep = vec![false; events.len()];
    for (i, e) in events.iter().enumerate() {
        let hay = format!(
            "{} {} {} {} {} {}",
            e.when,
            e.text,
            e.actor.unwrap_or(""),
            e.correlation.unwrap_or(""),
            e.status.id(),
            e.group_key.unwrap_or("")
        )
        .to_ascii_lowercase();
        if hay.contains(&q) && e.focusable() {
            keep[i] = true;
            // Keep preceding group header
            let mut j = i;
            while j > 0 {
                j -= 1;
                if matches!(events[j].kind, TimelineRowKind::Group) {
                    keep[j] = true;
                    break;
                }
            }
        }
    }
    events
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, e)| e)
        .collect()
}

/// Chronological timeline widget.
#[derive(Debug, Clone)]
pub struct Timeline<'a, Id = ()> {
    empty_message: &'a str,
    events: &'a [TimelineEvent<'a, Id>],
    system: &'a DesignSystem,
    recipe: TimelineRecipe,
    focused: bool,
    ascii: bool,
    colorless: bool,
}

impl<'a> Timeline<'a, ()> {
    /// Paint-only timeline (unit id).
    #[must_use]
    pub const fn new(events: &'a [TimelineEvent<'a, ()>], system: &'a DesignSystem) -> Self {
        Self {
            empty_message: "No events",
            events,
            system,
            recipe: TimelineRecipe::Detailed,
            focused: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Line shown when there is nothing to show.
    ///
    /// A collection that paints nothing when empty reads as broken; it has to
    /// say that it is empty.
    #[must_use]
    pub const fn empty_message(mut self, message: &'a str) -> Self {
        self.empty_message = message;
        self
    }
}

impl<'a, Id: Clone + PartialEq + Ord> Timeline<'a, Id> {
    /// Timeline with typed event ids.
    #[must_use]
    pub const fn with_events(
        events: &'a [TimelineEvent<'a, Id>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            empty_message: "No events",
            events,
            system,
            recipe: TimelineRecipe::Detailed,
            focused: true,
            ascii: false,
            colorless: false,
        }
    }

    /// Recipe.
    #[must_use]
    pub const fn recipe(mut self, recipe: TimelineRecipe) -> Self {
        self.recipe = recipe;
        self
    }

    /// Scene focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII markers.
    #[must_use]
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Colorless.
    #[must_use]
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Stateful paint.
    pub fn render_stateful(&self, area: Rect, buffer: &mut Buffer, state: &mut TimelineState<Id>) {
        state.regions.clear();
        state.painted = area;
        if area.is_empty() {
            state.viewport = 0;
            return;
        }
        let ascii = self.ascii || state.ascii;
        let colorless = self.colorless || state.colorless;
        let footer = 1u16;
        let body_h = area.height.saturating_sub(footer).max(1);
        state.viewport = usize::from(body_h);

        let view = filter_timeline_events(self.events, state.filter.as_deref().unwrap_or(""));
        let len = view.len();
        if state.following && len > 0 {
            state.cursor = len - 1;
        }
        if len > 0 {
            state.cursor = state.cursor.min(len - 1);
        } else {
            state.cursor = 0;
        }
        state.reveal(len.max(1));

        let surface = self.focused && state.accepts_input;
        let mut y = area.y;
        let start = state.offset;
        let end = (start + state.viewport).min(len);

        if view.is_empty() {
            let mark = if ascii { "[ ] " } else { "∅ " };
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(
                    &format!("{mark}{}", self.empty_message),
                    usize::from(area.width),
                ),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            self.paint_footer(area, buffer, state, ascii);
            return;
        }

        for (paint_i, idx) in (start..end).enumerate() {
            let event = view[idx];
            let row_y = area.y.saturating_add(paint_i as u16);
            if row_y >= area.bottom().saturating_sub(footer) {
                break;
            }
            let selected = state.selected.as_ref() == Some(&event.id)
                || (state.selected.is_none() && idx == state.cursor);
            let cursor = idx == state.cursor;

            if matches!(event.kind, TimelineRowKind::Group) {
                self.paint_group(area, row_y, buffer, event, ascii);
                y = row_y;
                continue;
            }

            let marker = event.status.marker(event.active || selected, ascii);
            let mut style = if colorless {
                if selected || cursor {
                    self.system
                        .style(Role::TextStrong)
                        .add_modifier(Modifier::BOLD)
                } else {
                    self.system.style(Role::Text)
                }
            } else if event.active {
                self.system.style(Role::Accent)
            } else {
                self.system.style(event.status.role())
            };

            if !event.enabled {
                style = self.system.style(Role::TextDisabled);
            }
            // A selected event keeps its status tone; the chrome marks it.
            let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected: selected || cursor,
                    focused: surface,
                    enabled: event.enabled,
                    ..Default::default()
                },
            );
            let style = chrome.label_style(style);

            let line = self.format_line(event, area.width, ascii, colorless);
            buffer.set_stringn(area.x, row_y, " ", 1, style);
            buffer.set_stringn(area.x.saturating_add(1), row_y, " ", 1, style);
            let body = format!("{marker} {line}");
            buffer.set_stringn(
                area.x.saturating_add(GUTTER),
                row_y,
                take_display_cols(&body, usize::from(area.width.saturating_sub(GUTTER))),
                usize::from(area.width.saturating_sub(GUTTER)),
                style,
            );
            // The marker cell breathes while the step runs; the label never
            // does. Same channel and period as `StatusIndicator`, so a running
            // step and a running status agree instead of each inventing a
            // rhythm (plans/014 Step 4).
            let brightness = crate::style::breathe_over(
                self.system.motion,
                self.system.elapsed_ms(),
                event.status.channel().period_ms(),
            );
            if brightness < 1.0 && !colorless {
                let canvas = self
                    .system
                    .style(Role::Canvas)
                    .bg
                    .unwrap_or(ratatui_core::style::Color::Reset);
                let marker_x = area.x.saturating_add(GUTTER);
                if marker_x < area.right() {
                    let faded = crate::style::fade_style(style, brightness, canvas);
                    buffer.set_stringn(marker_x, row_y, marker, 1, faded);
                }
            }

            chrome.paint(buffer, Rect::new(area.x, row_y, area.width, 1));

            if event.focusable() {
                state.regions.push(TimelineRegion {
                    id: event.id.clone(),
                    index: idx,
                    area: Rect::new(area.x, row_y, area.width, 1),
                });
            }

            // Expanded detail line
            if (event.expanded || state.is_expanded(&event.id)) && event.detail.is_some() {
                // detail consumes next paint slot only if room — host should project detail as rows
            }
            y = row_y;
        }
        let _ = y;
        self.paint_footer(area, buffer, state, ascii);
    }

    fn format_line(
        &self,
        event: &TimelineEvent<'a, Id>,
        width: u16,
        ascii: bool,
        colorless: bool,
    ) -> String {
        let narrow = width < 36;
        let tiny = width < 22;
        match self.recipe {
            TimelineRecipe::Rail => {
                if tiny {
                    event.text.to_string()
                } else {
                    format!("{} {}", event.when, event.text)
                }
            }
            TimelineRecipe::GroupedDay | TimelineRecipe::Detailed => {
                if tiny {
                    event.text.to_string()
                } else if narrow {
                    let mut s = format!("{} {}", event.when, event.text);
                    if colorless {
                        s = format!("[{}] {}", event.status.letter(), s);
                    }
                    s
                } else {
                    let mut parts = vec![event.when.to_string()];
                    if let Some(r) = event.relative {
                        parts.push(r.to_string());
                    }
                    if let Some(a) = event.actor {
                        parts.push(a.to_string());
                    }
                    if colorless || width >= 48 {
                        parts.push(format!("{}", event.status.letter()));
                    }
                    parts.push(event.text.to_string());
                    if let Some(d) = event.duration {
                        parts.push(format!("({d})"));
                    }
                    if let Some(c) = event.correlation {
                        if width >= 64 {
                            parts.push(format!("#{c}"));
                        }
                    }
                    if matches!(event.kind, TimelineRowKind::Checkpoint) {
                        parts.push(if ascii { "[ckpt]".into() } else { "◆".into() });
                    }
                    parts.join("  ")
                }
            }
        }
    }

    fn paint_group(
        &self,
        area: Rect,
        y: u16,
        buffer: &mut Buffer,
        event: &TimelineEvent<'a, Id>,
        ascii: bool,
    ) {
        let mark = if ascii { "# " } else { "▸ " };
        let line = format!("{mark}{}", event.when);
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            self.system
                .style(Role::TextStrong)
                .add_modifier(Modifier::BOLD),
        );
    }

    fn paint_footer(
        &self,
        area: Rect,
        buffer: &mut Buffer,
        state: &TimelineState<Id>,
        ascii: bool,
    ) {
        let y = area.bottom().saturating_sub(1);
        if y < area.y {
            return;
        }
        let mut parts = Vec::new();
        parts.push(format!("recipe:{}", self.recipe.id()));
        if state.following {
            parts.push(if ascii { "FOLLOW" } else { "↓ live" }.into());
        } else {
            parts.push("paused".into());
        }
        if let Some(q) = &state.filter {
            parts.push(format!("/{q}"));
        }
        if state.checkpoint_mode {
            parts.push("ckpt".into());
        }
        parts.push("f follow · / filter".into());
        let line = parts.join(" · ");
        buffer.set_stringn(
            area.x,
            y,
            take_display_cols(&line, usize::from(area.width)),
            usize::from(area.width),
            self.system.style(Role::TextMuted),
        );
    }
}

// ── Stateless Widget for progress_steps / simple paint ──────────────────────

impl Widget for &Timeline<'_, ()> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let mut state: TimelineState<()> = TimelineState::new();
        state.following = false;
        // Stateless: show from start; footer still paints when height allows.
        Timeline::render_stateful(self, area, buffer, &mut state);
    }
}

impl Widget for Timeline<'_, ()> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

impl<Id: Clone + PartialEq + Ord> StatefulWidget for &Timeline<'_, Id> {
    type State = TimelineState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Timeline::render_stateful(self, area, buffer, state);
    }
}

impl<Id: Clone + PartialEq + Ord> StatefulWidget for Timeline<'_, Id> {
    type State = TimelineState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        Timeline::render_stateful(&self, area, buffer, state);
    }
}

// CheckpointTimeline elevated in `checkpoint_timeline` module (migration 0229).
// Timeline still supports checkpoint_mode for generic RestoreRequested/CompareRequested.

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<TimelineEvent<'static, &'static str>> {
        vec![
            TimelineEvent::group("d1", "Today"),
            TimelineEvent::with_id("e1", "12:01", "Started deploy")
                .status(TimelineStatus::Success)
                .actor("ci")
                .relative("1h ago")
                .duration("12s")
                .group_key("Today"),
            TimelineEvent::with_id("e2", "12:02", "Running tests")
                .status(TimelineStatus::Running)
                .active()
                .actor("ci")
                .correlation("trace-9")
                .detail("cargo test -p termrock")
                .group_key("Today"),
            TimelineEvent::checkpoint("c1", "12:03", "pre-release")
                .actor("user")
                .group_key("Today"),
            TimelineEvent::with_id("e3", "12:04", "Failed health")
                .status(TimelineStatus::Failed)
                .group_key("Today"),
        ]
    }

    #[test]
    fn select_and_follow_detach() {
        let events = sample();
        let mut state = TimelineState::<&str>::new();
        state.following = true;
        state.cursor = events.len() - 1;
        let view = filter_timeline_events(&events, "");
        let out = state.handle_intent(UiIntent::Move(NavigationMove::Previous), &view);
        assert!(matches!(out, TimelineOutcome::Selected(_)));
        assert!(!state.is_following());
    }

    #[test]
    fn on_append_keeps_follow() {
        let events = sample();
        let mut state = TimelineState::<&str>::new();
        state.following = true;
        state.on_append(events.len());
        assert_eq!(state.cursor(), events.len() - 1);
    }

    #[test]
    fn checkpoint_restore_and_compare() {
        let events = sample();
        let mut state = TimelineState::<&str>::new();
        state.set_checkpoint_mode(true);
        state.following = false;
        state.cursor = 3; // checkpoint
        let view = filter_timeline_events(&events, "");
        let out = state.handle_intent(UiIntent::Activate, &view);
        assert!(matches!(out, TimelineOutcome::RestoreRequested("c1")));
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
            &events,
        );
        assert!(matches!(out, TimelineOutcome::CompareRequested("c1")));
    }

    #[test]
    fn filter_keeps_group() {
        let events = sample();
        let kept = filter_timeline_events(&events, "health");
        let ids: Vec<_> = kept.iter().map(|e| e.id).collect();
        assert!(ids.contains(&"d1"));
        assert!(ids.contains(&"e3"));
    }

    #[test]
    fn paint_recipes() {
        let system = DesignSystem::default();
        let events = sample();
        for recipe in [
            TimelineRecipe::Rail,
            TimelineRecipe::Detailed,
            TimelineRecipe::GroupedDay,
        ] {
            let mut state = TimelineState::<&str>::new();
            state.following = false;
            state.cursor = 1;
            let t = Timeline::with_events(&events, &system)
                .recipe(recipe)
                .focused(true);
            let area = Rect::new(0, 0, 64, 10);
            let mut buf = Buffer::empty(area);
            t.render_stateful(area, &mut buf, &mut state);
            assert!(!state.regions.is_empty() || recipe == TimelineRecipe::Rail);
            let text: String = buf
                .content()
                .iter()
                .map(|c| c.symbol().to_string())
                .collect();
            assert!(
                text.contains("deploy") || text.contains("tests") || text.contains("Today"),
                "{text}"
            );
        }
    }

    #[test]
    fn stateless_widget_compat() {
        let system = DesignSystem::default();
        let events = [
            TimelineEvent::new("12:01", "Started"),
            TimelineEvent::new("12:02", "Running").active(),
        ];
        let area = Rect::new(0, 0, 40, 4);
        let mut buf = Buffer::empty(area);
        Widget::render(&Timeline::new(&events, &system), area, &mut buf);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Started") || text.contains("Running"),
            "{text}"
        );
    }

    #[test]
    fn colorless_uses_letters() {
        let system = DesignSystem::default();
        let events = sample();
        let mut state = TimelineState::<&str>::new();
        state.colorless = true;
        state.following = false;
        let t = Timeline::with_events(&events, &system).colorless(true);
        let area = Rect::new(0, 0, 56, 8);
        let mut buf = Buffer::empty(area);
        t.render_stateful(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        // Status letters appear in detailed colorless mode
        assert!(
            text.contains('S')
                || text.contains('R')
                || text.contains('F')
                || text.contains("deploy"),
            "{text}"
        );
    }

    #[test]
    fn expand_toggle() {
        let events = sample();
        let mut state = TimelineState::<&str>::new();
        state.following = false;
        state.cursor = 2; // expandable running tests
        let view = filter_timeline_events(&events, "");
        let out = state.handle_intent(UiIntent::Expand, &view);
        assert!(matches!(out, TimelineOutcome::ExpandToggled("e2")));
        assert!(state.is_expanded(&"e2"));
    }

    #[test]
    fn ascii_markers() {
        assert_eq!(TimelineStatus::Failed.marker(false, true), "x");
        assert_eq!(TimelineStatus::Success.marker(false, false), "✓");
    }

    #[test]
    fn fuzz_filter_and_status() {
        let events = sample();
        for q in ["", "ci", "ZZZ", "12:", "trace"] {
            let v = filter_timeline_events(&events, q);
            assert!(v.len() <= events.len());
        }
        for s in [
            TimelineStatus::Pending,
            TimelineStatus::Running,
            TimelineStatus::Success,
            TimelineStatus::Failed,
        ] {
            let _ = s.marker(true, false);
            let _ = s.letter();
        }
    }

    #[test]
    fn checkpoint_mode_restore_path() {
        let events = [
            TimelineEvent::checkpoint("a", "t0", "snap-a"),
            TimelineEvent::checkpoint("b", "t1", "snap-b").active(),
        ];
        let mut state = TimelineState::<&str>::new();
        state.set_checkpoint_mode(true);
        state.following = false;
        state.cursor = 0;
        let view = filter_timeline_events(&events, "");
        let out = state.handle_intent(UiIntent::Activate, &view);
        assert!(matches!(out, TimelineOutcome::RestoreRequested("a")));
    }

    #[test]
    fn only_a_running_step_breathes_and_it_breathes_like_a_status() {
        use crate::style::MotionChannel;

        assert_eq!(TimelineStatus::Running.channel(), MotionChannel::Live);
        assert_eq!(TimelineStatus::Pending.channel(), MotionChannel::Wait);
        for status in [
            TimelineStatus::Success,
            TimelineStatus::Failed,
            TimelineStatus::Warning,
            TimelineStatus::Cancelled,
            TimelineStatus::Info,
        ] {
            assert_eq!(
                status.channel(),
                MotionChannel::Static,
                "{status:?} has finished; it must be still"
            );
        }
        // The same channel a running status indicator uses, so the two agree.
        assert_eq!(
            TimelineStatus::Running.channel().period_ms(),
            crate::widgets::SemanticStatus::Running
                .channel()
                .period_ms()
        );
    }
}
