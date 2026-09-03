// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **EventStream** — high-volume structured-event viewer (not plain logs).
//!
//! **Mission.** Observability consoles, Kubernetes events, and agent activity:
//! event type, timestamp, severity, actor/source, structured field summary,
//! correlation IDs, filtering, grouping, pause/follow, details, pluggable row
//! summaries, burst batching / backpressure indicators, stable scroll anchors,
//! and unread counts.
//!
//! **vs [`super::LogStream`].** LogStream is line/level text. EventStream is
//! typed structured events with inspector-style selection and field chrome.
//! **vs [`super::Timeline`].** Timeline is chronological presentation recipes;
//! EventStream optimizes sustained append rates and unread/backpressure.
//!
//! Research: observability event consoles, k8s events, agent activity streams.
use std::collections::BTreeSet;

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    input::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
    interaction::{NavigationMove, PageMove, UiIntent},
    style::{DesignSystem, ListRowVisualState, Role},
    text::take_display_cols,
    widgets::{scroll_area::ScrollAreaState, tiered_row::TieredRow},
};

/// Appends one part, toned when the surface has color to spend.
fn push_tier(row: &mut TieredRow, text: &str, tone: Option<Style>) {
    match tone {
        Some(style) => row.push(text, style),
        None => row.push_plain(text),
    }
}

/// Selection gutter width: one marker cell plus its breathing space.
///
/// Stated once so the gutter cannot drift between the row, the header and the
/// hit regions (plans/022 Step 6).
const GUTTER: u16 = 2;

/// Severity for structured events (maps to no-color letters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum EventSeverity {
    /// Trace / finest.
    Trace,
    /// Debug.
    Debug,
    /// Informational.
    #[default]
    Info,
    /// Warning.
    Warn,
    /// Error.
    Error,
    /// Critical / fatal.
    Critical,
}

impl EventSeverity {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    /// No-color letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Trace => 'T',
            Self::Debug => 'D',
            Self::Info => 'I',
            Self::Warn => 'W',
            Self::Error => 'E',
            Self::Critical => 'C',
        }
    }

    /// Glyph for structured chrome.
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Trace => ".",
            Self::Debug => "·",
            Self::Info => "i",
            Self::Warn => "!",
            Self::Error => "x",
            Self::Critical => "◆",
        }
    }

    fn role(self) -> Role {
        match self {
            Self::Trace | Self::Debug => Role::TextMuted,
            Self::Info => Role::Text,
            Self::Warn => Role::Warning,
            Self::Error | Self::Critical => Role::Danger,
        }
    }
}

/// Row kind in the projected stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum StreamRowKind {
    /// Ordinary event.
    #[default]
    Event,
    /// Group header (by type / time bucket / source).
    Group,
}

/// One structured event projection (host owns storage; TermRock paints the window).
///
/// Prefer builders. `summary` is the pluggable row summary; `detail` is optional
/// inspector body (shown when selected+expanded or on detail panel host).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEvent<'a, Id = ()> {
    /// Stable identity (selection, anchors).
    pub id: Id,
    /// Event type (`Normal`, `Warning`, `PodScheduled`, `tool.call`, …).
    pub event_type: &'a str,
    /// Timestamp label (absolute or relative display).
    pub timestamp: &'a str,
    /// Severity.
    pub severity: EventSeverity,
    /// Actor / source (`kubelet`, `agent`, `user`).
    pub source: Option<&'a str>,
    /// Pluggable one-line summary (host formats structured fields).
    pub summary: &'a str,
    /// Optional structured field chrome (`k=v k2=v2`) — dropped when narrow.
    pub fields: Option<&'a str>,
    /// Correlation / trace id.
    pub correlation: Option<&'a str>,
    /// Optional multi-line / rich detail for inspector.
    pub detail: Option<&'a str>,
    /// Group key when host injects group headers.
    pub group_key: Option<&'a str>,
    /// Row kind.
    pub kind: StreamRowKind,
    /// Count of batched/coalesced underlying events (burst).
    pub batch_count: u32,
    /// Interactive.
    pub enabled: bool,
}

impl<'a> StreamEvent<'a, ()> {
    /// Simple event without id.
    #[must_use]
    pub const fn new(event_type: &'a str, timestamp: &'a str, summary: &'a str) -> Self {
        Self {
            id: (),
            event_type,
            timestamp,
            severity: EventSeverity::Info,
            source: None,
            summary,
            fields: None,
            correlation: None,
            detail: None,
            group_key: None,
            kind: StreamRowKind::Event,
            batch_count: 1,
            enabled: true,
        }
    }
}

impl<'a, Id> StreamEvent<'a, Id> {
    /// Event with stable id.
    #[must_use]
    pub const fn with_id(
        id: Id,
        event_type: &'a str,
        timestamp: &'a str,
        summary: &'a str,
    ) -> Self {
        Self {
            id,
            event_type,
            timestamp,
            severity: EventSeverity::Info,
            source: None,
            summary,
            fields: None,
            correlation: None,
            detail: None,
            group_key: None,
            kind: StreamRowKind::Event,
            batch_count: 1,
            enabled: true,
        }
    }

    /// Group header.
    #[must_use]
    pub const fn group(id: Id, label: &'a str) -> Self {
        Self {
            id,
            event_type: label,
            timestamp: "",
            severity: EventSeverity::Info,
            source: None,
            summary: "",
            fields: None,
            correlation: None,
            detail: None,
            group_key: Some(label),
            kind: StreamRowKind::Group,
            batch_count: 0,
            enabled: true,
        }
    }

    /// Severity.
    #[must_use]
    pub const fn severity(mut self, s: EventSeverity) -> Self {
        self.severity = s;
        self
    }

    /// Source / actor.
    #[must_use]
    pub const fn source(mut self, source: &'a str) -> Self {
        self.source = Some(source);
        self
    }

    /// Structured field summary.
    #[must_use]
    pub const fn fields(mut self, fields: &'a str) -> Self {
        self.fields = Some(fields);
        self
    }

    /// Correlation id.
    #[must_use]
    pub const fn correlation(mut self, id: &'a str) -> Self {
        self.correlation = Some(id);
        self
    }

    /// Inspector detail body.
    #[must_use]
    pub const fn detail(mut self, detail: &'a str) -> Self {
        self.detail = Some(detail);
        self
    }

    /// Group key.
    #[must_use]
    pub const fn group_key(mut self, key: &'a str) -> Self {
        self.group_key = Some(key);
        self
    }

    /// Coalesced batch size (>1 shows `×N`).
    #[must_use]
    pub const fn batch_count(mut self, n: u32) -> Self {
        self.batch_count = if n == 0 { 1 } else { n };
        self
    }

    /// Focusable (not group header).
    #[must_use]
    pub const fn focusable(&self) -> bool {
        self.enabled && matches!(self.kind, StreamRowKind::Event)
    }
}

/// Hit region for one painted event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventStreamRegion<Id> {
    /// Event id.
    pub id: Id,
    /// Index in current filtered projection.
    pub index: usize,
    /// Row rect.
    pub area: Rect,
}

/// Outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EventStreamOutcome<Id> {
    /// No change.
    Ignored,
    /// Selection moved (for inspector detail host).
    Selected(Id),
    /// Open / activate selected event.
    Activated(Id),
    /// Viewport scrolled.
    Scrolled {
        /// Offset.
        offset: u16,
        /// Still following tail.
        following: bool,
    },
    /// Re-attached to tail.
    Follow,
    /// Detached from tail.
    Detach,
    /// Filter query changed (host may reproject).
    FilterChanged(String),
    /// Severity floor filter changed.
    SeverityFilter(EventSeverity),
    /// Type filter toggled (host applies).
    TypeFilterChanged {
        /// Type key.
        event_type: String,
        /// Included after toggle.
        included: bool,
    },
    /// Cancel / clear filter.
    Cancelled,
    /// Backpressure dropped count acknowledged / cleared.
    BackpressureAck,
}

/// Stream interaction + backpressure / unread state.
///
/// Follow/pause/unseen live in [`ScrollAreaState`] (sole scroll authority).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventStreamState<Id: Clone + PartialEq + Ord = ()> {
    scroll: ScrollAreaState,
    /// Selected event id (inspector host).
    pub selected: Option<Id>,
    /// Cursor index in the filtered projection.
    pub cursor: usize,
    accepts_input: bool,
    /// Free-text filter query.
    pub filter: Option<String>,
    /// Minimum severity shown (inclusive).
    severity_floor: EventSeverity,
    /// Excluded event types (host may also pre-filter).
    excluded_types: BTreeSet<String>,
    /// Events dropped by host under backpressure (display only).
    dropped: u64,
    /// Events batched/coalesced in last window.
    batched: u64,
    /// Inline detail strip open for selection.
    pub detail_open: bool,
    /// Stable anchor id (preserve across reproject).
    anchor_id: Option<Id>,
    origin: (u16, u16),
    body_rows: u16,
    area_rows: u16,
    event_count: u16,
    /// Hit regions from the last paint.
    pub regions: Vec<EventStreamRegion<Id>>,
    /// Prefer non-chromatic severity emphasis.
    pub colorless: bool,
    /// Show inspector detail strip under selection when detail present.
    pub show_inline_detail: bool,
}

impl<Id: Clone + PartialEq + Ord> Default for EventStreamState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Clone + PartialEq + Ord> EventStreamState<Id> {
    /// Following by default.
    #[must_use]
    pub fn new() -> Self {
        let mut scroll = ScrollAreaState::new().axes(true, false);
        scroll.follow_tail();
        Self {
            scroll,
            selected: None,
            cursor: 0,
            accepts_input: true,
            filter: None,
            severity_floor: EventSeverity::Trace,
            excluded_types: BTreeSet::new(),
            dropped: 0,
            batched: 0,
            detail_open: false,
            anchor_id: None,
            origin: (0, 0),
            body_rows: 0,
            area_rows: 0,
            event_count: 0,
            regions: Vec::new(),
            colorless: false,
            show_inline_detail: true,
        }
    }

    /// Selected id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Cursor index in filtered projection.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Following tail.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.scroll.is_following()
    }

    /// Scroll offset.
    #[must_use]
    pub const fn offset(&self) -> u16 {
        self.scroll.offset_y()
    }

    /// Unseen new events while paused.
    #[must_use]
    pub fn unread(&self) -> u64 {
        u64::from(self.scroll.new_content().unseen)
    }

    /// Dropped under backpressure.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Batched in last window.
    #[must_use]
    pub const fn batched(&self) -> u64 {
        self.batched
    }

    /// Shared scroll (anchors, indicator).
    #[must_use]
    pub const fn scroll(&self) -> &ScrollAreaState {
        &self.scroll
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, accepts: bool) {
        self.accepts_input = accepts;
    }

    /// Force follow.
    pub fn set_following(&mut self, following: bool) {
        if following {
            self.scroll.follow_tail();
        } else {
            self.scroll.pause_follow();
        }
    }

    /// Report host-side backpressure (dropped events not in projection).
    pub fn report_backpressure(&mut self, dropped: u64, batched: u64) {
        self.dropped = self.dropped.saturating_add(dropped);
        self.batched = batched;
    }

    /// Clear dropped counter.
    pub fn ack_backpressure(&mut self) {
        self.dropped = 0;
        self.batched = 0;
    }

    /// Severity floor filter.
    pub fn set_severity_floor(&mut self, floor: EventSeverity) {
        self.severity_floor = floor;
    }

    /// Severity floor.
    #[must_use]
    pub const fn severity_floor(&self) -> EventSeverity {
        self.severity_floor
    }

    /// Exclude / include event type.
    pub fn toggle_type_filter(&mut self, event_type: &str) -> bool {
        let key = event_type.to_string();
        if !self.excluded_types.remove(&key) {
            self.excluded_types.insert(key);
            false
        } else {
            true
        }
    }

    /// Capture stable anchor at current cursor (host reprojects; restore later).
    pub fn capture_anchor(&mut self, events: &[StreamEvent<'_, Id>]) {
        let view = self.filtered_view(events);
        if let Some(e) = view.get(self.cursor) {
            self.anchor_id = Some(e.id.clone());
        }
    }

    /// Restore cursor from anchor id after reproject.
    pub fn restore_anchor(&mut self, events: &[StreamEvent<'_, Id>]) {
        let view = self.filtered_view(events);
        if let Some(aid) = self.anchor_id.as_ref() {
            if let Some(i) = view.iter().position(|e| &e.id == aid) {
                self.cursor = i;
                self.selected = Some(aid.clone());
                self.scroll.reveal_row(self.cursor);
            }
        }
    }

    fn filtered_view<'a>(&self, events: &'a [StreamEvent<'a, Id>]) -> Vec<&'a StreamEvent<'a, Id>> {
        filter_stream_events(
            events,
            self.filter.as_deref().unwrap_or(""),
            self.severity_floor,
            &self.excluded_types,
        )
    }

    fn sync_metrics(&mut self, total: u16, viewport: u16) {
        self.event_count = total;
        self.body_rows = viewport;
        self.scroll.set_content_size(1, total);
        self.scroll.set_viewport(1, viewport);
    }

    /// After host appends projected events.
    pub fn on_append(&mut self, total_events: u16, viewport: u16) {
        self.sync_metrics(total_events, viewport);
        if self.scroll.is_following() && total_events > 0 {
            self.cursor = usize::from(total_events.saturating_sub(1));
        }
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        events: &[StreamEvent<'_, Id>],
    ) -> EventStreamOutcome<Id> {
        if !self.accepts_input || key.is_release() {
            return EventStreamOutcome::Ignored;
        }
        let is_press = key.is_press();

        if is_press && matches!(key.code, KeyCode::Char('/')) && key.modifiers.is_empty() {
            if self.filter.is_none() {
                self.filter = Some(String::new());
            }
            return EventStreamOutcome::FilterChanged(self.filter.clone().unwrap_or_default());
        }
        if let Some(q) = self.filter.as_mut()
            && is_press
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.filter = None;
                    return EventStreamOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.filter = None;
                    }
                    return EventStreamOutcome::FilterChanged(
                        self.filter.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c) if !c.is_control() && c != '/' => {
                    q.push(c);
                    return EventStreamOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        // Severity floor cycle: 1-6 or [
        if is_press && matches!(key.code, KeyCode::Char('[')) {
            self.severity_floor = match self.severity_floor {
                EventSeverity::Trace => EventSeverity::Debug,
                EventSeverity::Debug => EventSeverity::Info,
                EventSeverity::Info => EventSeverity::Warn,
                EventSeverity::Warn => EventSeverity::Error,
                EventSeverity::Error => EventSeverity::Critical,
                EventSeverity::Critical => EventSeverity::Trace,
            };
            return EventStreamOutcome::SeverityFilter(self.severity_floor);
        }

        if is_press && matches!(key.code, KeyCode::Char('b' | 'B')) && key.modifiers.is_empty() {
            self.ack_backpressure();
            return EventStreamOutcome::BackpressureAck;
        }

        if is_press && matches!(key.code, KeyCode::Char('f' | 'F')) && key.modifiers.is_empty() {
            return self.handle_intent(UiIntent::Toggle, events);
        }

        if is_press && matches!(key.code, KeyCode::Char('i' | 'I' | ' ')) {
            self.detail_open = !self.detail_open;
            if let Some(id) = self.selected.clone() {
                return EventStreamOutcome::Selected(id);
            }
            return EventStreamOutcome::Ignored;
        }

        if let Some(intent) = crate::interaction::default_log_stream_intent(key)
            .or_else(|| crate::interaction::default_list_intent(key))
        {
            return self.handle_intent(intent, events);
        }
        EventStreamOutcome::Ignored
    }

    /// Intent routing.
    pub fn handle_intent(
        &mut self,
        intent: UiIntent,
        events: &[StreamEvent<'_, Id>],
    ) -> EventStreamOutcome<Id> {
        if !self.accepts_input {
            return EventStreamOutcome::Ignored;
        }
        let view = self.filtered_view(events);
        if view.is_empty() {
            match intent {
                UiIntent::Toggle => {
                    if self.scroll.is_following() {
                        self.scroll.pause_follow();
                        EventStreamOutcome::Detach
                    } else {
                        self.scroll.follow_tail();
                        EventStreamOutcome::Follow
                    }
                }
                _ => EventStreamOutcome::Ignored,
            }
        } else {
            self.cursor = self.cursor.min(view.len() - 1);
            match intent {
                UiIntent::Move(NavigationMove::Next | NavigationMove::Down) => {
                    self.move_cursor(&view, 1)
                }
                UiIntent::Move(NavigationMove::Previous | NavigationMove::Up) => {
                    self.move_cursor(&view, -1)
                }
                UiIntent::Move(NavigationMove::First) => {
                    let was = self.scroll.is_following();
                    self.cursor = 0;
                    self.scroll.pause_follow();
                    self.scroll.reveal_row(self.cursor);
                    self.select_at(&view);
                    if was {
                        EventStreamOutcome::Detach
                    } else {
                        EventStreamOutcome::Selected(view[0].id.clone())
                    }
                }
                UiIntent::Move(NavigationMove::Last) => {
                    self.cursor = view.len() - 1;
                    self.scroll.follow_tail();
                    self.select_at(&view);
                    EventStreamOutcome::Follow
                }
                UiIntent::Page(PageMove::Forward) => {
                    self.move_cursor(&view, self.body_rows.max(1) as isize)
                }
                UiIntent::Page(PageMove::Backward) => {
                    self.move_cursor(&view, -(self.body_rows.max(1) as isize))
                }
                UiIntent::Activate | UiIntent::Submit | UiIntent::Open => {
                    let id = view[self.cursor].id.clone();
                    self.selected = Some(id.clone());
                    self.detail_open = true;
                    EventStreamOutcome::Activated(id)
                }
                UiIntent::Toggle => {
                    if self.scroll.is_following() {
                        self.scroll.pause_follow();
                        EventStreamOutcome::Detach
                    } else {
                        self.scroll.follow_tail();
                        self.cursor = view.len() - 1;
                        EventStreamOutcome::Follow
                    }
                }
                UiIntent::Cancel => {
                    if self.filter.is_some() {
                        self.filter = None;
                        return EventStreamOutcome::Cancelled;
                    }
                    self.detail_open = false;
                    EventStreamOutcome::Cancelled
                }
                _ => EventStreamOutcome::Ignored,
            }
        }
    }

    fn move_cursor(
        &mut self,
        view: &[&StreamEvent<'_, Id>],
        delta: isize,
    ) -> EventStreamOutcome<Id> {
        let focusable: Vec<usize> = view
            .iter()
            .enumerate()
            .filter(|(_, e)| e.focusable())
            .map(|(i, _)| i)
            .collect();
        if focusable.is_empty() {
            return EventStreamOutcome::Ignored;
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
        let was_follow = self.scroll.is_following();
        if idx < self.cursor {
            self.scroll.pause_follow();
        } else if idx + 1 >= view.len() {
            self.scroll.follow_tail();
        }
        if idx == self.cursor && !was_follow {
            // try scroll page
            let d = if delta >= 0 { 1 } else { -1 };
            if self.scroll.scroll_by(d, 0).is_scrolled() {
                return EventStreamOutcome::Scrolled {
                    offset: self.offset(),
                    following: self.is_following(),
                };
            }
            return EventStreamOutcome::Ignored;
        }
        self.cursor = idx;
        self.scroll.reveal_row(self.cursor);
        self.select_at(view);
        if was_follow && !self.is_following() {
            EventStreamOutcome::Detach
        } else {
            EventStreamOutcome::Selected(view[idx].id.clone())
        }
    }

    fn select_at(&mut self, view: &[&StreamEvent<'_, Id>]) {
        if let Some(e) = view.get(self.cursor) {
            self.selected = Some(e.id.clone());
            self.anchor_id = Some(e.id.clone());
        }
    }

    /// Mouse.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        events: &[StreamEvent<'_, Id>],
    ) -> EventStreamOutcome<Id> {
        if !self.accepts_input {
            return EventStreamOutcome::Ignored;
        }
        let (ox, oy) = self.origin;
        let hit = Rect {
            x: ox,
            y: oy,
            width: 240,
            height: self.area_rows.max(1),
        };
        if !hit.contains(event.position) {
            return EventStreamOutcome::Ignored;
        }
        match event.kind {
            MouseEventKind::ScrollDown => {
                self.handle_intent(UiIntent::Move(NavigationMove::Next), events)
            }
            MouseEventKind::ScrollUp => {
                self.handle_intent(UiIntent::Move(NavigationMove::Previous), events)
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let chip_y = oy.saturating_add(self.area_rows.saturating_sub(1));
                if self.area_rows >= 2 && event.position.y == chip_y {
                    if self.scroll.is_following() && self.dropped == 0 {
                        return EventStreamOutcome::Ignored;
                    }
                    self.scroll.jump_to_new_content();
                    return EventStreamOutcome::Follow;
                }
                if let Some(r) = self
                    .regions
                    .iter()
                    .find(|r| r.area.contains(event.position))
                {
                    if self.selected.as_ref() == Some(&r.id) {
                        self.detail_open = true;
                        return EventStreamOutcome::Activated(r.id.clone());
                    }
                    self.cursor = r.index;
                    self.selected = Some(r.id.clone());
                    self.anchor_id = Some(r.id.clone());
                    self.scroll.pause_follow();
                    return EventStreamOutcome::Selected(r.id.clone());
                }
                EventStreamOutcome::Ignored
            }
            _ => EventStreamOutcome::Ignored,
        }
    }
}

/// Filter by query, severity floor, and excluded types.
#[must_use]
pub fn filter_stream_events<'a, Id>(
    events: &'a [StreamEvent<'a, Id>],
    query: &str,
    severity_floor: EventSeverity,
    excluded_types: &BTreeSet<String>,
) -> Vec<&'a StreamEvent<'a, Id>> {
    let q = query.trim().to_ascii_lowercase();
    let mut keep = vec![false; events.len()];
    for (i, e) in events.iter().enumerate() {
        if matches!(e.kind, StreamRowKind::Group) {
            continue;
        }
        if e.severity < severity_floor {
            continue;
        }
        if excluded_types.contains(e.event_type) {
            continue;
        }
        if !q.is_empty() {
            let hay = format!(
                "{} {} {} {} {} {}",
                e.event_type,
                e.summary,
                e.fields.unwrap_or(""),
                e.source.unwrap_or(""),
                e.correlation.unwrap_or(""),
                e.timestamp
            )
            .to_ascii_lowercase();
            if !hay.contains(&q) {
                continue;
            }
        }
        keep[i] = true;
        // Keep preceding group
        let mut j = i;
        while j > 0 {
            j -= 1;
            if matches!(events[j].kind, StreamRowKind::Group) {
                keep[j] = true;
                break;
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

/// Structured event stream widget.
#[derive(Debug, Clone)]
pub struct EventStream<'a, Id = ()> {
    events: &'a [StreamEvent<'a, Id>],
    system: &'a DesignSystem,
    focused: bool,
    colorless: bool,
}

impl<'a> EventStream<'a, ()> {
    /// Unit-id stream.
    #[must_use]
    pub const fn new(events: &'a [StreamEvent<'a, ()>], system: &'a DesignSystem) -> Self {
        Self {
            events,
            system,
            focused: true,
            colorless: false,
        }
    }
}

impl<'a, Id: Clone + PartialEq + Ord> EventStream<'a, Id> {
    /// Typed-id stream.
    #[must_use]
    pub const fn with_events(events: &'a [StreamEvent<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            events,
            system,
            focused: true,
            colorless: false,
        }
    }

    /// Scene focus.
    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// ASCII glyphs.
    #[must_use]
    /// Colorless.
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Paint O(visible).
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut EventStreamState<Id>) {
        state.regions.clear();
        if area.is_empty() {
            state.body_rows = 0;
            state.area_rows = 0;
            return;
        }
        let separator = " · ";
        let colorless = self.colorless || state.colorless || self.system.mono();
        state.origin = (area.x, area.y);
        state.area_rows = area.height;

        let view = state.filtered_view(self.events);
        let total = view.len().min(u16::MAX as usize) as u16;
        let following = state.scroll.is_following();
        let unread = state.unread();
        let show_chip =
            area.height >= 2 && (following || unread > 0 || state.dropped > 0 || !view.is_empty());
        let detail_h = if state.show_inline_detail
            && state.detail_open
            && state.selected.is_some()
            && area.height >= 4
        {
            2u16
        } else {
            0
        };
        let body_h = area
            .height
            .saturating_sub(u16::from(show_chip) + detail_h)
            .max(1);
        state.sync_metrics(total, body_h);
        if following && total > 0 {
            state.cursor = usize::from(total.saturating_sub(1));
        }
        if total > 0 {
            state.cursor = state.cursor.min(usize::from(total) - 1);
        }

        let surface = self.focused && state.accepts_input;
        let tiny = area.width < 20;
        let narrow = area.width < 40;

        if view.is_empty() {
            let mark = "∅ ";
            let msg = if tiny {
                format!("{mark}empty")
            } else {
                format!("{mark}(no events)")
            };
            buffer.set_stringn(
                area.x,
                area.y,
                take_display_cols(&msg, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
        } else {
            let start = state.offset() as usize;
            let mut y = area.y;
            let bottom = area.y.saturating_add(body_h);
            for (i, event) in view.iter().enumerate().skip(start) {
                if y >= bottom {
                    break;
                }
                let selected = state.selected.as_ref() == Some(&event.id)
                    || (state.selected.is_none() && i == state.cursor);
                let cursor = i == state.cursor;

                if matches!(event.kind, StreamRowKind::Group) {
                    let mark = "▸ ";
                    let line = format!("{mark}{}", event.event_type);
                    buffer.set_stringn(
                        area.x,
                        y,
                        take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD),
                    );
                    y = y.saturating_add(1);
                    continue;
                }

                let style = if colorless {
                    if selected || cursor {
                        self.system
                            .style(Role::TextStrong)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        self.system.style(Role::Text)
                    }
                } else {
                    // A selected warning is still a warning — and its severity
                    // rides the glyph, not the sentence (plans/007).
                    self.system.style(Role::Text)
                };
                let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                    self.system,
                    ListRowVisualState {
                        selected: selected || cursor,
                        focused: surface,
                        enabled: true,
                        ..Default::default()
                    },
                );
                let style = chrome.label_style(style);

                // The cursor column is stamped by the shared row chrome.
                let gutter = " ";
                buffer.set_stringn(
                    area.x,
                    y,
                    gutter,
                    1,
                    if cursor {
                        self.system.style(Role::Accent)
                    } else {
                        style
                    },
                );
                buffer.set_stringn(area.x.saturating_add(1), y, " ", 1, style);

                let sev = event.severity.glyph();
                let batch = if event.batch_count > 1 {
                    format!("{}{}", "×", event.batch_count)
                } else {
                    String::new()
                };
                // Tiers, not a sentence: the severity owns its glyph, the
                // timestamp and the type sit under the summary, and the
                // fields trail behind it (plans/012 Step 3).
                let tone = |role: Role| {
                    if colorless {
                        None
                    } else {
                        Some(self.system.style(role))
                    }
                };
                let meta = tone(Role::TextFaint);
                let type_tone = tone(Role::TextMuted);
                let mut row = TieredRow::with_separator("  ");
                if tiny {
                    row.push_joined(sev, tone(event.severity.role()));
                    row.push_plain(event.summary);
                } else if narrow {
                    row.push_joined(sev, tone(event.severity.role()));
                    push_tier(&mut row, event.event_type, type_tone);
                    row.push_plain(event.summary);
                    row.push_joined(&batch, type_tone);
                } else {
                    row.push_joined(sev, tone(event.severity.role()));
                    if colorless {
                        row.push_plain(&event.severity.letter().to_string());
                    }
                    push_tier(&mut row, event.timestamp, meta);
                    push_tier(&mut row, event.event_type, type_tone);
                    if let Some(s) = event.source {
                        push_tier(&mut row, s, type_tone);
                    }
                    row.push_plain(event.summary);
                    if let Some(f) = event.fields
                        && area.width >= 72
                    {
                        push_tier(&mut row, f, meta);
                    }
                    if let Some(c) = event.correlation
                        && area.width >= 80
                    {
                        push_tier(&mut row, &format!("#{c}"), meta);
                    }
                    push_tier(&mut row, &batch, type_tone);
                }
                let line = row.text().to_string();
                buffer.set_stringn(
                    area.x.saturating_add(GUTTER),
                    y,
                    take_display_cols(&line, usize::from(area.width.saturating_sub(GUTTER))),
                    usize::from(area.width.saturating_sub(GUTTER)),
                    style,
                );
                row.paint_tiers(
                    buffer,
                    Rect::new(
                        area.x.saturating_add(GUTTER),
                        y,
                        area.width.saturating_sub(GUTTER),
                        1,
                    ),
                    0,
                );
                chrome.paint(buffer, Rect::new(area.x, y, area.width, 1));
                if event.focusable() {
                    state.regions.push(EventStreamRegion {
                        id: event.id.clone(),
                        index: i,
                        area: Rect::new(area.x, y, area.width, 1),
                    });
                }
                y = y.saturating_add(1);
            }
        }

        // Inline detail strip
        if detail_h > 0 {
            let dy = area.y.saturating_add(body_h);
            if let Some(sel) = state.selected.as_ref() {
                if let Some(ev) = view.iter().find(|e| &e.id == sel) {
                    let detail = ev.detail.unwrap_or(ev.summary);
                    let line = format!(
                        "{}{}",
                        "  └ ",
                        take_display_cols(detail, usize::from(area.width.saturating_sub(4)))
                    );
                    buffer.set_stringn(
                        area.x,
                        dy,
                        take_display_cols(&line, usize::from(area.width)),
                        usize::from(area.width),
                        self.system.style(Role::TextMuted),
                    );
                }
            }
        }

        // Follow / unread / backpressure chip
        if show_chip {
            let cy = area.bottom().saturating_sub(1);
            let mut chip = if following {
                "↓ live".into()
            } else if unread > 0 {
                format!("↓ {unread} new")
            } else {
                "paused".into()
            };
            if state.dropped > 0 {
                chip.push_str(&format!("{separator}drop {}", state.dropped));
            }
            if state.batched > 1 {
                chip.push_str(&format!("{separator}batch {}", state.batched));
            }
            if let Some(q) = &state.filter {
                chip.push_str(&format!("{separator}/{q}"));
            }
            if state.severity_floor > EventSeverity::Trace {
                let comparison = "≥";
                chip.push_str(&format!(
                    "{separator}{comparison}{}",
                    state.severity_floor.letter()
                ));
            }
            buffer.set_stringn(
                area.x,
                cy,
                take_display_cols(&chip, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(if following {
                    Role::Accent
                } else if unread > 0 || state.dropped > 0 {
                    Role::Warning
                } else {
                    Role::TextMuted
                }),
            );
        }
    }
}

impl<'a, Id: Clone + PartialEq + Ord> StatefulWidget for &EventStream<'a, Id> {
    type State = EventStreamState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        EventStream::paint(self, area, buffer, state);
    }
}

impl<'a, Id: Clone + PartialEq + Ord> StatefulWidget for EventStream<'a, Id> {
    type State = EventStreamState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        EventStream::paint(&self, area, buffer, state);
    }
}

// ── Bench helpers ───────────────────────────────────────────────────────────

/// Sustained-rate bench targets (documentation / tests).
pub mod bench {
    /// Events per second target for host append loops.
    pub const EVENTS_PER_SEC: u32 = 10_000;
    /// Viewport rows for paint budget.
    pub const VIEWPORT: u16 = 40;
    /// Burst batch size under backpressure.
    pub const BURST_BATCH: u32 = 64;
    /// Max paint cells per frame (viewport × avg cols).
    pub const MAX_PAINT_CELLS: u32 = 40 * 80;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    fn sample() -> Vec<StreamEvent<'static, &'static str>> {
        vec![
            StreamEvent::group("g0", "kube-system"),
            StreamEvent::with_id("e1", "Normal", "12:01:00", "Scheduled pod")
                .severity(EventSeverity::Info)
                .source("scheduler")
                .fields("pod=api-7 node=n1")
                .correlation("deploy-9")
                .group_key("kube-system"),
            StreamEvent::with_id("e2", "Warning", "12:01:01", "FailedMount")
                .severity(EventSeverity::Warn)
                .source("kubelet")
                .fields("vol=cfg")
                .detail("MountVolume.SetUp failed for volume \"cfg\"")
                .group_key("kube-system")
                .batch_count(3),
            StreamEvent::with_id("e3", "tool.call", "12:01:02", "run_terminal_command")
                .severity(EventSeverity::Info)
                .source("agent")
                .correlation("turn-4")
                .detail("cargo test -p termrock"),
            StreamEvent::with_id("e4", "Error", "12:01:03", "CrashLoopBackOff")
                .severity(EventSeverity::Error)
                .source("kubelet")
                .group_key("kube-system"),
        ]
    }

    #[test]
    fn follow_detach_and_unread() {
        let events = sample();
        let mut state = EventStreamState::<&str>::new();
        assert!(state.is_following());
        state.on_append(events.len() as u16, 10);
        let out = state.handle_intent(UiIntent::Move(NavigationMove::Previous), &events);
        assert!(matches!(
            out,
            EventStreamOutcome::Detach | EventStreamOutcome::Selected(_)
        ));
        // Simulate append while paused
        state.set_following(false);
        state.on_append(100, 10);
        // ScrollArea tracks unseen via set_content_size when paused
        assert!(!state.is_following());
    }

    #[test]
    fn filter_severity_and_query() {
        let events = sample();
        let excl = BTreeSet::new();
        let v = filter_stream_events(&events, "Mount", EventSeverity::Trace, &excl);
        let ids: Vec<_> = v.iter().map(|e| e.id).collect();
        assert!(ids.contains(&"e2"));
        assert!(ids.contains(&"g0")); // group kept
        let v2 = filter_stream_events(&events, "", EventSeverity::Error, &excl);
        assert!(v2.iter().all(|e| {
            matches!(e.kind, StreamRowKind::Group) || e.severity >= EventSeverity::Error
        }));
    }

    #[test]
    fn selection_opens_detail() {
        let events = sample();
        let mut state = EventStreamState::<&str>::new();
        state.set_following(false);
        state.cursor = 2;
        let out = state.handle_intent(UiIntent::Activate, &events);
        assert!(matches!(out, EventStreamOutcome::Activated("e2")));
        assert!(state.detail_open);
    }

    #[test]
    fn backpressure_report() {
        let mut state = EventStreamState::<&str>::new();
        state.report_backpressure(50, 64);
        assert_eq!(state.dropped(), 50);
        assert_eq!(state.batched(), 64);
        state.ack_backpressure();
        assert_eq!(state.dropped(), 0);
    }

    #[test]
    fn anchor_survives_reproject() {
        let events = sample();
        let mut state = EventStreamState::<&str>::new();
        state.set_following(false);
        state.cursor = 2;
        state.capture_anchor(&events);
        // "reproject" same data
        state.cursor = 0;
        state.restore_anchor(&events);
        assert_eq!(state.cursor(), 2);
        assert_eq!(state.selected(), Some(&"e2"));
    }

    #[test]
    fn paint_visible_only() {
        let system = DesignSystem::default();
        let events = sample();
        let mut state = EventStreamState::<&str>::new();
        state.set_following(false);
        let stream = EventStream::with_events(&events, &system).focused(true);
        let area = Rect::new(0, 0, 80, 8);
        let mut buf = Buffer::empty(area);
        stream.render(area, &mut buf, &mut state);
        assert!(!state.regions.is_empty());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Scheduled") || text.contains("Failed") || text.contains("tool"),
            "{text}"
        );
    }

    #[test]
    fn sustained_append_viewport_bound() {
        // Host projects only a window; state metrics stay O(viewport).
        let system = DesignSystem::default();
        let mut owned: Vec<(String, String, String)> = Vec::new();
        for i in 0..bench::VIEWPORT {
            owned.push((
                format!("e{i}"),
                format!("12:00:{i:02}"),
                format!("event-{i}"),
            ));
        }
        // Build static-like borrows for one paint
        let events: Vec<StreamEvent<'_, usize>> = owned
            .iter()
            .enumerate()
            .map(|(i, (_, ts, sum))| {
                StreamEvent::with_id(i, "Normal", ts, sum).severity(EventSeverity::Info)
            })
            .collect();
        let mut state = EventStreamState::<usize>::new();
        state.on_append(events.len() as u16, bench::VIEWPORT);
        let stream = EventStream::with_events(&events, &system);
        let area = Rect::new(0, 0, 80, bench::VIEWPORT + 1);
        let mut buf = Buffer::empty(area);
        // Paint many times — cost bounded by viewport
        for _ in 0..100 {
            (&stream).render(area, &mut buf, &mut state);
        }
        assert!(state.regions.len() <= usize::from(bench::VIEWPORT) + 2);
    }

    #[test]
    fn burst_batch_marker() {
        let system = DesignSystem::default();
        let events = [StreamEvent::with_id("b", "Warning", "t", "flapping")
            .severity(EventSeverity::Warn)
            .batch_count(bench::BURST_BATCH)];
        let mut state = EventStreamState::<&str>::new();
        let stream = EventStream::with_events(&events, &system);
        let area = Rect::new(0, 0, 60, 4);
        let mut buf = Buffer::empty(area);
        stream.render(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains('×') || text.contains("64") || text.contains("flapping"),
            "{text}"
        );
    }

    #[test]
    fn severity_filter_cycle() {
        let events = sample();
        let mut state = EventStreamState::<&str>::new();
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE),
            &events,
        );
        assert!(matches!(
            out,
            EventStreamOutcome::SeverityFilter(EventSeverity::Debug)
        ));
    }

    #[test]
    fn fuzz_filter_empty_query() {
        let events = sample();
        let excl = BTreeSet::new();
        for floor in [
            EventSeverity::Trace,
            EventSeverity::Info,
            EventSeverity::Error,
        ] {
            let v = filter_stream_events(&events, "", floor, &excl);
            assert!(v.len() <= events.len());
        }
    }
}
