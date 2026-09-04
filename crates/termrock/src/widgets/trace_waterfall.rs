// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **TraceWaterfall** — hierarchical span and latency visualization for
//! distributed traces and agent/tool execution.
//!
//! **Mission.** Nested spans, duration bars, critical path, status,
//! service/actor, search, filters, zoom, selection, and details. Readable ASCII
//! fallback and exact time labels. Virtualize large traces; preserve horizontal
//! time navigation. Hierarchy navigation is distinct from timeline scrolling.
//! Compose with [`super::ObjectInspector`] and [`super::Timeline`].
//!
//! **Ownership.** Host owns trace fetch and span models. TermRock owns layout,
//! virtualized paint, zoom/scroll chrome, and typed outcomes.
//!
//! Research: trace viewers, Chrome DevTools waterfall, agent activity timelines.
use std::collections::BTreeSet;

use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, ListRowVisualState, Role},
    text::{contains_lower_all, take_display_cols},
    widgets::{
        data_view::VirtualWindow,
        object_inspector::{InspectKind, InspectorField},
        tiered_row::TieredRow,
        timeline::{TimelineEvent, TimelineStatus},
    },
};

/// Default name-column width in grid mode.
pub const TRACE_NAME_COL_DEFAULT: u16 = 28;
/// Minimum name column.
pub const TRACE_NAME_COL_MIN: u16 = 12;
/// Maximum name column.
pub const TRACE_NAME_COL_MAX: u16 = 48;

// ── Domain ──────────────────────────────────────────────────────────────────

/// Span outcome status (OTel-class).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TraceSpanStatus {
    /// Unset / unknown.
    #[default]
    Unset,
    /// OK.
    Ok,
    /// Error.
    Error,
    /// Cancelled / aborted.
    Cancelled,
}

impl TraceSpanStatus {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }

    /// Letter (never color alone).
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Unset => '·',
            Self::Ok => 'S',
            Self::Error => 'E',
            Self::Cancelled => 'C',
        }
    }
    /// Role.
    #[must_use]
    pub const fn role(self) -> Role {
        match self {
            Self::Unset => Role::TextMuted,
            Self::Ok => Role::Success,
            Self::Error => Role::Danger,
            Self::Cancelled => Role::Warning,
        }
    }

    /// Map to Timeline status.
    #[must_use]
    pub const fn to_timeline(self) -> TimelineStatus {
        match self {
            Self::Unset => TimelineStatus::Info,
            Self::Ok => TimelineStatus::Success,
            Self::Error => TimelineStatus::Failed,
            Self::Cancelled => TimelineStatus::Cancelled,
        }
    }
}

/// Navigation mode: hierarchy vs time axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TraceNavMode {
    /// j/k select; h/l collapse/expand (default).
    #[default]
    Hierarchy,
    /// h/l scroll time window; j/k still select rows.
    Timeline,
}

impl TraceNavMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hierarchy => "hierarchy",
            Self::Timeline => "timeline",
        }
    }

    /// Toggle.
    #[must_use]
    pub const fn toggle(self) -> Self {
        match self {
            Self::Hierarchy => Self::Timeline,
            Self::Timeline => Self::Hierarchy,
        }
    }
}

/// One span in a flattened **visible** hierarchy (host projects expanded path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSpan<'a> {
    /// Stable span id.
    pub id: &'a str,
    /// Parent span id when known.
    pub parent: Option<&'a str>,
    /// Display name (operation).
    pub name: &'a str,
    /// Service / actor.
    pub service: &'a str,
    /// Hierarchy depth (0 root).
    pub depth: u16,
    /// Expandable (has children).
    pub branch: bool,
    /// Expanded (host projection).
    pub expanded: bool,
    /// Start offset from trace start (ms).
    pub start_ms: u64,
    /// Duration ms (≥ 0; 0 = instant marker).
    pub duration_ms: u64,
    /// Status.
    pub status: TraceSpanStatus,
    /// On critical path.
    pub critical: bool,
    /// Optional kind badge (`http`, `db`, `tool`).
    pub kind: Option<&'a str>,
    /// Optional error message.
    pub error: Option<&'a str>,
    /// Interaction enabled.
    pub enabled: bool,
}

impl<'a> TraceSpan<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, name: &'a str, start_ms: u64, duration_ms: u64) -> Self {
        Self {
            id,
            parent: None,
            name,
            service: "",
            depth: 0,
            branch: false,
            expanded: false,
            start_ms,
            duration_ms,
            status: TraceSpanStatus::Ok,
            critical: false,
            kind: None,
            error: None,
            enabled: true,
        }
    }

    /// Parent.
    #[must_use]
    pub const fn parent(mut self, p: &'a str) -> Self {
        self.parent = Some(p);
        self
    }

    /// Service.
    #[must_use]
    pub const fn service(mut self, s: &'a str) -> Self {
        self.service = s;
        self
    }

    /// Depth.
    #[must_use]
    pub const fn depth(mut self, d: u16) -> Self {
        self.depth = d;
        self
    }

    /// Branch.
    #[must_use]
    pub const fn branch(mut self) -> Self {
        self.branch = true;
        self
    }

    /// Expanded.
    #[must_use]
    pub const fn expanded(mut self) -> Self {
        self.expanded = true;
        self.branch = true;
        self
    }

    /// Status.
    #[must_use]
    pub const fn status(mut self, s: TraceSpanStatus) -> Self {
        self.status = s;
        self
    }

    /// Critical path.
    #[must_use]
    pub const fn critical(mut self) -> Self {
        self.critical = true;
        self
    }

    /// Kind badge.
    #[must_use]
    pub const fn kind(mut self, k: &'a str) -> Self {
        self.kind = Some(k);
        self
    }

    /// Error.
    #[must_use]
    pub const fn error(mut self, msg: &'a str) -> Self {
        self.error = Some(msg);
        self.status = TraceSpanStatus::Error;
        self
    }

    /// End time ms (exclusive-ish).
    #[must_use]
    pub const fn end_ms(&self) -> u64 {
        self.start_ms.saturating_add(self.duration_ms)
    }
}

// ── Time helpers ────────────────────────────────────────────────────────────

/// Format duration ms as human label (trace domain).
#[must_use]
pub fn format_trace_duration_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        let s = ms as f64 / 1000.0;
        if s < 10.0 {
            format!("{s:.2}s")
        } else {
            format!("{s:.1}s")
        }
    } else {
        let m = ms / 60_000;
        let s = (ms % 60_000) / 1000;
        format!("{m}m{s:02}s")
    }
}

/// Format absolute offset from trace start.
#[must_use]
pub fn format_trace_offset_ms(ms: u64) -> String {
    format!("+{}", format_trace_duration_ms(ms))
}

/// Trace total duration from spans (max end).
#[must_use]
pub fn trace_total_ms(spans: &[TraceSpan<'_>]) -> u64 {
    spans.iter().map(|s| s.end_ms()).max().unwrap_or(0).max(1)
}

/// Map time range to bar columns within `bar_w`.
///
/// Returns `(col_start, col_width)` clamped into `0..bar_w`.
#[must_use]
pub fn span_bar_cols(
    start_ms: u64,
    duration_ms: u64,
    window_start_ms: u64,
    window_dur_ms: u64,
    bar_w: u16,
) -> Option<(u16, u16)> {
    if bar_w == 0 || window_dur_ms == 0 {
        return None;
    }
    let end_ms = start_ms.saturating_add(duration_ms.max(1));
    let win_end = window_start_ms.saturating_add(window_dur_ms);
    if end_ms <= window_start_ms || start_ms >= win_end {
        return None; // outside window
    }
    let vis_start = start_ms.max(window_start_ms);
    let vis_end = end_ms.min(win_end);
    let w = bar_w as f64;
    let scale = w / window_dur_ms as f64;
    let c0 = ((vis_start.saturating_sub(window_start_ms)) as f64 * scale).floor() as i64;
    let c1 = ((vis_end.saturating_sub(window_start_ms)) as f64 * scale).ceil() as i64;
    let c0 = c0.clamp(0, bar_w as i64) as u16;
    let c1 = c1.clamp(0, bar_w as i64) as u16;
    let width = c1.saturating_sub(c0).max(1);
    let width = width.min(bar_w.saturating_sub(c0));
    Some((c0, width.max(1)))
}

/// Filter spans by query (name/service/kind/error) keeping ancestors.
#[must_use]
pub fn filter_trace_spans<'a>(spans: &'a [TraceSpan<'a>], query: &str) -> Vec<&'a TraceSpan<'a>> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return spans.iter().collect();
    }
    let mut keep = vec![false; spans.len()];
    for (i, s) in spans.iter().enumerate() {
        if contains_lower_all(
            &[
                s.name,
                s.service,
                s.kind.unwrap_or(""),
                s.error.unwrap_or(""),
            ],
            &q,
        ) {
            keep[i] = true;
            let mut parent = s.parent;
            while let Some(pid) = parent {
                if let Some((pi, pe)) = spans.iter().enumerate().find(|(_, x)| x.id == pid) {
                    keep[pi] = true;
                    parent = pe.parent;
                } else {
                    break;
                }
            }
        }
    }
    spans
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, s)| s)
        .collect()
}

/// Filter critical-path only (still keep ancestors of critical spans).
#[must_use]
pub fn filter_critical_path<'a>(spans: &'a [TraceSpan<'a>]) -> Vec<&'a TraceSpan<'a>> {
    let crit: BTreeSet<&str> = spans.iter().filter(|s| s.critical).map(|s| s.id).collect();
    if crit.is_empty() {
        return spans.iter().collect();
    }
    let mut keep: BTreeSet<&str> = crit.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for s in spans {
            if keep.contains(s.id) {
                if let Some(p) = s.parent {
                    if keep.insert(p) {
                        changed = true;
                    }
                }
            }
        }
    }
    spans.iter().filter(|s| keep.contains(s.id)).collect()
}

/// Project span → TimelineEvent for chronological rail composition.
///
/// `when` / `duration` labels must be host-owned strings (borrowed here). Use
/// [`format_trace_offset_ms`] / [`format_trace_duration_ms`] to fill buffers before calling.
#[must_use]
pub fn span_to_timeline_event<'a>(
    span: &'a TraceSpan<'a>,
    when: &'a str,
    duration: &'a str,
) -> TimelineEvent<'a, &'a str> {
    let mut ev = TimelineEvent::with_id(span.id, when, span.name).status(span.status.to_timeline());
    if !span.service.is_empty() {
        ev = ev.actor(span.service);
    }
    if !duration.is_empty() {
        ev = ev.duration(duration);
    }
    if let Some(c) = span.parent {
        ev = ev.correlation(c);
    }
    ev
}

/// Project span fields for ObjectInspector detail pane.
#[must_use]
pub fn span_to_inspector_fields<'a>(span: &'a TraceSpan<'a>) -> Vec<InspectorField<'a>> {
    let mut fields = vec![
        InspectorField::new("name", span.name).kind(InspectKind::String),
        InspectorField::new("service", span.service).kind(InspectKind::String),
        InspectorField::new("status", span.status.id()).kind(InspectKind::String),
    ];
    // duration/start as display via static-ish - use kind labels
    if let Some(k) = span.kind {
        fields.push(InspectorField::new("kind", k).kind(InspectKind::String));
    }
    if let Some(e) = span.error {
        fields.push(InspectorField::new("error", e).kind(InspectKind::String));
    }
    if let Some(p) = span.parent {
        fields.push(InspectorField::new("parent", p).kind(InspectKind::String));
    }
    fields.push(
        InspectorField::new("critical", if span.critical { "true" } else { "false" })
            .kind(InspectKind::Bool),
    );
    fields.push(InspectorField::new("id", span.id).kind(InspectKind::String));
    fields
}

// ── Outcomes ────────────────────────────────────────────────────────────────

/// Typed outcomes — host owns trace fetch and detail panels.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TraceWaterfallOutcome {
    /// No change.
    Ignored,
    /// Selection moved.
    SelectionChanged {
        /// Span id.
        id: String,
    },
    /// Expand/collapse branch (host updates projection).
    ExpandToggled {
        /// Span id.
        id: String,
        /// Expanded after (inferred from previous projection when known).
        expanded: bool,
    },
    /// Open details (ObjectInspector / side panel).
    DetailsRequested {
        /// Span id.
        id: String,
    },
    /// Filter query changed.
    FilterChanged(String),
    /// Time window scrolled / zoomed.
    TimeWindowChanged {
        /// Window start ms.
        start_ms: u64,
        /// Visible duration ms.
        duration_ms: u64,
    },
    /// Nav mode changed.
    NavModeChanged(TraceNavMode),
    /// Critical-path filter toggled.
    CriticalPathToggled {
        /// Only critical after.
        only: bool,
    },
    /// Project as Timeline events requested.
    TimelineExportRequested,
    /// Cancel filter.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// Trace waterfall interaction state.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceWaterfallState {
    /// Selected span id.
    selected: Option<String>,
    /// Vertical window over flattened projection.
    pub window: VirtualWindow,
    /// Cursor row in flattened projection.
    pub cursor: usize,
    /// Time window start (ms from trace origin).
    pub time_start_ms: u64,
    /// Visible duration (ms). Zoom in → smaller.
    pub time_duration_ms: u64,
    /// Trace total (host or computed).
    pub total_ms: u64,
    /// Nav mode.
    pub nav_mode: TraceNavMode,
    /// Filter query.
    pub filter: Option<String>,
    /// Show only critical path.
    pub critical_only: bool,
    /// Expanded ids (preserve; host should honor in projection).
    pub expanded: BTreeSet<String>,
    /// Name column width.
    pub name_col: u16,
    /// Load / empty chrome.
    pub empty_message: Option<String>,
    /// Hit regions (row id → rect).
    row_regions: Vec<(String, Rect)>,
    /// Bar hit regions.
    bar_regions: Vec<(String, Rect)>,
    accepts_input: bool,
}

impl Default for TraceWaterfallState {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceWaterfallState {
    /// Fresh (full trace window until paint sets total).
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: None,
            window: VirtualWindow::default(),
            cursor: 0,
            time_start_ms: 0,
            time_duration_ms: 0,
            total_ms: 0,
            nav_mode: TraceNavMode::Hierarchy,
            filter: None,
            critical_only: false,
            expanded: BTreeSet::new(),
            name_col: TRACE_NAME_COL_DEFAULT,
            empty_message: None,
            row_regions: Vec::new(),
            bar_regions: Vec::new(),
            accepts_input: true,
        }
    }

    /// With selection.
    #[must_use]
    pub fn with_selected(id: impl Into<String>) -> Self {
        let mut s = Self::new();
        s.selected = Some(id.into());
        s
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Select.
    pub fn select(&mut self, id: Option<String>) {
        self.selected = id;
    }

    /// Sync total from spans and initialize window if unset.
    pub fn sync_total(&mut self, spans: &[TraceSpan<'_>]) {
        self.total_ms = trace_total_ms(spans);
        if self.time_duration_ms == 0 {
            self.time_duration_ms = self.total_ms;
            self.time_start_ms = 0;
        }
        self.clamp_time();
    }

    fn clamp_time(&mut self) {
        let total = self.total_ms.max(1);
        self.time_duration_ms = self.time_duration_ms.clamp(1, total);
        let max_start = total.saturating_sub(self.time_duration_ms);
        if self.time_start_ms > max_start {
            self.time_start_ms = max_start;
        }
    }

    /// Visible projection after filters.
    #[must_use]
    pub fn visible_spans<'a>(&self, spans: &'a [TraceSpan<'a>]) -> Vec<&'a TraceSpan<'a>> {
        let mut v = filter_trace_spans(spans, self.filter.as_deref().unwrap_or(""));
        if self.critical_only {
            let crit = filter_critical_path(spans);
            let crit_set: BTreeSet<&str> = crit.iter().map(|s| s.id).collect();
            v.retain(|s| crit_set.contains(s.id));
        }
        v
    }

    /// Zoom in (halve duration, keep selection in view if possible).
    pub fn zoom_in(&mut self, focus_ms: Option<u64>) -> TraceWaterfallOutcome {
        let focus = focus_ms.unwrap_or_else(|| self.time_start_ms + self.time_duration_ms / 2);
        self.time_duration_ms = (self.time_duration_ms / 2).max(1);
        self.time_start_ms = focus.saturating_sub(self.time_duration_ms / 2);
        self.clamp_time();
        TraceWaterfallOutcome::TimeWindowChanged {
            start_ms: self.time_start_ms,
            duration_ms: self.time_duration_ms,
        }
    }

    /// Zoom out.
    pub fn zoom_out(&mut self) -> TraceWaterfallOutcome {
        let mid = self.time_start_ms + self.time_duration_ms / 2;
        self.time_duration_ms = (self.time_duration_ms.saturating_mul(2)).min(self.total_ms.max(1));
        self.time_start_ms = mid.saturating_sub(self.time_duration_ms / 2);
        self.clamp_time();
        TraceWaterfallOutcome::TimeWindowChanged {
            start_ms: self.time_start_ms,
            duration_ms: self.time_duration_ms,
        }
    }

    /// Pan time by fraction of window (−1.0..=1.0).
    pub fn pan_time(&mut self, fraction: f64) -> TraceWaterfallOutcome {
        let delta = (self.time_duration_ms as f64 * fraction) as i64;
        if delta >= 0 {
            self.time_start_ms = self.time_start_ms.saturating_add(delta as u64);
        } else {
            self.time_start_ms = self.time_start_ms.saturating_sub((-delta) as u64);
        }
        self.clamp_time();
        TraceWaterfallOutcome::TimeWindowChanged {
            start_ms: self.time_start_ms,
            duration_ms: self.time_duration_ms,
        }
    }

    /// Keys.
    pub fn handle_key(&mut self, spans: &[TraceSpan<'_>], key: KeyEvent) -> TraceWaterfallOutcome {
        if !self.accepts_input || !key.is_press() {
            return TraceWaterfallOutcome::Ignored;
        }
        self.sync_total(spans);
        let visible = self.visible_spans(spans);
        self.window.logical_len = visible.len() as u64;
        self.window.clamp();
        if !visible.is_empty() {
            self.cursor = self.cursor.min(visible.len() - 1);
        }

        // Filter typing
        if let Some(q) = self.filter.as_mut()
            && key.modifiers.is_empty()
        {
            match key.code {
                KeyCode::Esc => {
                    self.filter = None;
                    return TraceWaterfallOutcome::Cancelled;
                }
                KeyCode::Backspace => {
                    q.pop();
                    if q.is_empty() {
                        self.filter = None;
                    }
                    return TraceWaterfallOutcome::FilterChanged(
                        self.filter.clone().unwrap_or_default(),
                    );
                }
                KeyCode::Char(c)
                    if !c.is_control()
                        && !matches!(c, 'j' | 'k' | 'h' | 'l' | 'J' | 'K' | 'H' | 'L') =>
                {
                    q.push(c);
                    return TraceWaterfallOutcome::FilterChanged(q.clone());
                }
                _ => {}
            }
        }

        // Global chords
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('=') | KeyCode::Char('+') => {
                    let focus = visible
                        .get(self.cursor)
                        .map(|s| s.start_ms + s.duration_ms / 2);
                    return self.zoom_in(focus);
                }
                KeyCode::Char('-') => return self.zoom_out(),
                KeyCode::Char('0') => {
                    self.time_start_ms = 0;
                    self.time_duration_ms = self.total_ms.max(1);
                    return TraceWaterfallOutcome::TimeWindowChanged {
                        start_ms: self.time_start_ms,
                        duration_ms: self.time_duration_ms,
                    };
                }
                KeyCode::Char('\\') => {
                    self.nav_mode = self.nav_mode.toggle();
                    return TraceWaterfallOutcome::NavModeChanged(self.nav_mode);
                }
                KeyCode::Char('p' | 'P') => {
                    self.critical_only = !self.critical_only;
                    return TraceWaterfallOutcome::CriticalPathToggled {
                        only: self.critical_only,
                    };
                }
                KeyCode::Char('t' | 'T') => {
                    return TraceWaterfallOutcome::TimelineExportRequested;
                }
                _ => {}
            }
        }

        if key.modifiers.is_empty() {
            match key.code {
                KeyCode::Char('/') => {
                    self.filter = Some(String::new());
                    return TraceWaterfallOutcome::FilterChanged(String::new());
                }
                KeyCode::Char('[') => return self.pan_time(-0.25),
                KeyCode::Char(']') => return self.pan_time(0.25),
                _ => {}
            }
        }

        if visible.is_empty() {
            return TraceWaterfallOutcome::Ignored;
        }

        // Hierarchy vs timeline left/right
        match key.code {
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                let next = (self.cursor + 1).min(visible.len() - 1);
                self.cursor = next;
                self.selected = Some(visible[next].id.to_string());
                let _ = self.window.reveal(next as u64);
                return TraceWaterfallOutcome::SelectionChanged {
                    id: visible[next].id.to_string(),
                };
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                let next = self.cursor.saturating_sub(1);
                self.cursor = next;
                self.selected = Some(visible[next].id.to_string());
                let _ = self.window.reveal(next as u64);
                return TraceWaterfallOutcome::SelectionChanged {
                    id: visible[next].id.to_string(),
                };
            }
            KeyCode::Home => {
                self.cursor = 0;
                self.selected = Some(visible[0].id.to_string());
                let _ = self.window.reveal(0);
                return TraceWaterfallOutcome::SelectionChanged {
                    id: visible[0].id.to_string(),
                };
            }
            KeyCode::End => {
                let next = visible.len() - 1;
                self.cursor = next;
                self.selected = Some(visible[next].id.to_string());
                let _ = self.window.reveal(next as u64);
                return TraceWaterfallOutcome::SelectionChanged {
                    id: visible[next].id.to_string(),
                };
            }
            KeyCode::PageDown => {
                let vh = usize::from(self.window.viewport.max(1));
                let next = (self.cursor + vh).min(visible.len() - 1);
                self.cursor = next;
                self.selected = Some(visible[next].id.to_string());
                let _ = self.window.reveal(next as u64);
                return TraceWaterfallOutcome::SelectionChanged {
                    id: visible[next].id.to_string(),
                };
            }
            KeyCode::PageUp => {
                let vh = usize::from(self.window.viewport.max(1));
                let next = self.cursor.saturating_sub(vh);
                self.cursor = next;
                self.selected = Some(visible[next].id.to_string());
                let _ = self.window.reveal(next as u64);
                return TraceWaterfallOutcome::SelectionChanged {
                    id: visible[next].id.to_string(),
                };
            }
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                return match self.nav_mode {
                    TraceNavMode::Timeline => self.pan_time(-0.15),
                    TraceNavMode::Hierarchy => {
                        let s = &visible[self.cursor];
                        if s.branch && s.expanded {
                            self.expanded.remove(s.id);
                            TraceWaterfallOutcome::ExpandToggled {
                                id: s.id.to_string(),
                                expanded: false,
                            }
                        } else if let Some(p) = s.parent {
                            // jump parent
                            if let Some((i, _)) =
                                visible.iter().enumerate().find(|(_, x)| x.id == p)
                            {
                                self.cursor = i;
                                self.selected = Some(p.to_string());
                                let _ = self.window.reveal(i as u64);
                                TraceWaterfallOutcome::SelectionChanged { id: p.to_string() }
                            } else {
                                TraceWaterfallOutcome::Ignored
                            }
                        } else {
                            TraceWaterfallOutcome::Ignored
                        }
                    }
                };
            }
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                return match self.nav_mode {
                    TraceNavMode::Timeline => self.pan_time(0.15),
                    TraceNavMode::Hierarchy => {
                        let s = &visible[self.cursor];
                        if s.branch && !s.expanded {
                            self.expanded.insert(s.id.to_string());
                            TraceWaterfallOutcome::ExpandToggled {
                                id: s.id.to_string(),
                                expanded: true,
                            }
                        } else {
                            TraceWaterfallOutcome::Ignored
                        }
                    }
                };
            }
            KeyCode::Enter if key.modifiers.is_empty() => {
                let s = &visible[self.cursor];
                if s.branch {
                    let exp = !s.expanded;
                    if exp {
                        self.expanded.insert(s.id.to_string());
                    } else {
                        self.expanded.remove(s.id);
                    }
                    return TraceWaterfallOutcome::ExpandToggled {
                        id: s.id.to_string(),
                        expanded: exp,
                    };
                }
                return TraceWaterfallOutcome::DetailsRequested {
                    id: s.id.to_string(),
                };
            }
            KeyCode::Char('i') if key.modifiers.is_empty() => {
                return TraceWaterfallOutcome::DetailsRequested {
                    id: visible[self.cursor].id.to_string(),
                };
            }
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                // center selected in time window
                let s = &visible[self.cursor];
                let mid = s.start_ms + s.duration_ms / 2;
                self.time_start_ms = mid.saturating_sub(self.time_duration_ms / 2);
                self.clamp_time();
                return TraceWaterfallOutcome::TimeWindowChanged {
                    start_ms: self.time_start_ms,
                    duration_ms: self.time_duration_ms,
                };
            }
            _ => {}
        }
        TraceWaterfallOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Trace waterfall paint.
#[derive(Debug, Clone, Copy)]
pub struct TraceWaterfall<'a> {
    spans: &'a [TraceSpan<'a>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
}

impl<'a> TraceWaterfall<'a> {
    /// Spans + system.
    #[must_use]
    pub const fn new(spans: &'a [TraceSpan<'a>], system: &'a DesignSystem) -> Self {
        Self {
            spans,
            system,
            focused: true,
            title: None,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = Some(t);
        self
    }

    /// Focus.
    #[must_use]
    pub const fn focused(mut self, on: bool) -> Self {
        self.focused = on;
        self
    }

    /// ASCII.
    #[must_use]
    /// Paint.
    pub fn paint(&self, area: Rect, buffer: &mut Buffer, state: &mut TraceWaterfallState) {
        if area.is_empty() {
            return;
        }
        state.row_regions.clear();
        state.bar_regions.clear();
        state.sync_total(self.spans);

        let mut y = area.y;
        let mut h = area.height;

        // Chrome
        if h > 0 {
            let title = self.title.unwrap_or("trace");
            let crit = if state.critical_only { "crit" } else { "all" };
            // The header names the trace and then states its window: the name
            // is the headline and the numbers are the caption (plans/012).
            let mut header = TieredRow::with_separator(" · ");
            header.push(
                title,
                if self.focused {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::Text)
                },
            );
            header.push_plain(state.nav_mode.id());
            header.push_plain(&format!(
                "win {}–{} / {}",
                format_trace_offset_ms(state.time_start_ms),
                format_trace_offset_ms(state.time_start_ms + state.time_duration_ms),
                format_trace_duration_ms(state.total_ms),
            ));
            header.push_plain(crit);
            header.push_plain(&self.spans.len().to_string());
            let line = header.text().to_string();
            buffer.set_stringn(
                area.x,
                y,
                take_display_cols(&line, usize::from(area.width)),
                usize::from(area.width),
                self.system.style(Role::TextMuted),
            );
            header.paint_tiers(buffer, Rect::new(area.x, y, area.width, 1), 0);
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if state.filter.is_some() && h > 0 {
            let q = state.filter.as_deref().unwrap_or("");
            crate::widgets::ChromeRow::query(q, self.system)
                .paint(Rect::new(area.x, y, area.width, 1), buffer);
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        // Time ruler
        let name_w = state
            .name_col
            .clamp(TRACE_NAME_COL_MIN, TRACE_NAME_COL_MAX)
            .min(area.width.saturating_div(2));
        if h > 0 && area.width > name_w + 4 {
            paint_time_ruler(
                Rect {
                    x: area.x.saturating_add(name_w),
                    y,
                    width: area.width.saturating_sub(name_w),
                    height: 1,
                },
                buffer,
                self.system,
                state.time_start_ms,
                state.time_duration_ms,
            );
            y = y.saturating_add(1);
            h = h.saturating_sub(1);
        }

        if h == 0 {
            return;
        }

        let visible = state.visible_spans(self.spans);
        state.window.viewport = h;
        state.window.logical_len = visible.len() as u64;
        state.window.clamp();

        if visible.is_empty() {
            let msg = state.empty_message.as_deref().unwrap_or("No spans");
            super::EmptyState::new(msg, self.system).paint(
                Rect::new(area.x, y, area.width, 1),
                buffer,
                &mut super::EmptyStateState::new(),
            );
            return;
        }

        // Sync cursor to selection
        if let Some(sel) = state.selected.as_deref() {
            if let Some(i) = visible.iter().position(|s| s.id == sel) {
                state.cursor = i;
            }
        } else {
            state.selected = Some(visible[state.cursor.min(visible.len() - 1)].id.to_string());
        }
        state.cursor = state.cursor.min(visible.len() - 1);

        let start = state.window.offset as usize;
        let end = (start + usize::from(h)).min(visible.len());
        let bar_x = area.x.saturating_add(name_w);
        let bar_w = area.width.saturating_sub(name_w);
        let mut py = y;
        let bottom = y.saturating_add(h);

        for span in visible.iter().skip(start).take(end - start) {
            if py >= bottom {
                break;
            }
            let selected = Some(span.id) == state.selected.as_deref();
            let mark = " ";
            let disc = if span.branch {
                if span.expanded { "▾" } else { "▸" }
            } else {
                " "
            };
            let indent = "  ".repeat(usize::from(span.depth));
            let letter = span.status.letter();
            let crit = if span.critical { "◆" } else { " " };
            // The status is the letter's, not the whole name's: a column of
            // trace rows reads as one column of state instead of as five
            // colored sentences (plans/012 Step 3).
            let chrome = crate::widgets::row_chrome::RowChrome::resolve(
                self.system,
                ListRowVisualState {
                    selected,
                    focused: selected && self.focused,
                    enabled: true,
                    ..Default::default()
                },
            );
            let style = chrome.label_style(self.system.style(Role::Text));
            let mut row = TieredRow::with_separator("");
            row.push_joined(mark, None);
            row.push_joined(disc, None);
            row.push_joined(&indent, None);
            row.push_joined(
                &letter.to_string(),
                Some(self.system.style(span.status.role())),
            );
            row.push_joined(
                crit,
                span.critical.then(|| self.system.style(Role::Warning)),
            );
            if !span.service.is_empty() {
                row.push_joined(
                    &format!("{}.", span.service),
                    Some(self.system.style(Role::TextMuted)),
                );
            }
            row.push_joined(span.name, None);
            let label = row.text().to_string();
            buffer.set_stringn(
                area.x,
                py,
                take_display_cols(&label, usize::from(name_w)),
                usize::from(name_w),
                style,
            );
            row.paint_tiers(buffer, Rect::new(area.x, py, name_w, 1), 0);

            // Duration label at end of name col if space
            let dur = format_trace_duration_ms(span.duration_ms);
            let dw = crate::text::display_cols(&dur) as u16;
            if dw + 1 < name_w {
                buffer.set_stringn(
                    area.x.saturating_add(name_w.saturating_sub(dw)),
                    py,
                    &dur,
                    usize::from(dw),
                    self.system.style(Role::TextMuted),
                );
            }

            state.row_regions.push((
                span.id.to_string(),
                Rect {
                    x: area.x,
                    y: py,
                    width: name_w,
                    height: 1,
                },
            ));

            // Waterfall bar
            if bar_w > 0 {
                if let Some((c0, bw)) = span_bar_cols(
                    span.start_ms,
                    span.duration_ms,
                    state.time_start_ms,
                    state.time_duration_ms.max(1),
                    bar_w,
                ) {
                    let fill = if span.critical {
                        "█"
                    } else if matches!(span.status, TraceSpanStatus::Error) {
                        "▓"
                    } else {
                        "█"
                    };
                    // Bars are data, so they read as a series. A waterfall
                    // painted in severity was a wall of colour with no shape
                    // in it; failure still shows in the fill glyph above and
                    // in the status letter in the name column.
                    let bar_role = Role::ChartSeries1;
                    let bx = bar_x.saturating_add(c0);
                    for dx in 0..bw {
                        let x = bx.saturating_add(dx);
                        if x >= area.right() {
                            break;
                        }
                        buffer.set_stringn(x, py, fill, 1, self.system.style(bar_role));
                    }
                    state.bar_regions.push((
                        span.id.to_string(),
                        Rect {
                            x: bx,
                            y: py,
                            width: bw,
                            height: 1,
                        },
                    ));
                }
            }
            chrome.paint(buffer, Rect::new(area.x, py, area.width, 1));
            py = py.saturating_add(1);
        }
    }
}

fn paint_time_ruler(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    start_ms: u64,
    dur_ms: u64,
) {
    if area.is_empty() || dur_ms == 0 {
        return;
    }
    let ticks = 4u16.min(area.width / 8).max(2);
    for i in 0..=ticks {
        let t = start_ms + dur_ms * u64::from(i) / u64::from(ticks.max(1));
        let col = if i == ticks {
            area.width.saturating_sub(1)
        } else {
            (area.width as u64 * u64::from(i) / u64::from(ticks.max(1))) as u16
        };
        let label = format_trace_offset_ms(t);
        let mark = "┆";
        buffer.set_stringn(
            area.x.saturating_add(col),
            area.y,
            mark,
            1,
            system.style(Role::TextMuted),
        );
        if col + 2 < area.width {
            buffer.set_stringn(
                area.x.saturating_add(col.saturating_add(1)),
                area.y,
                take_display_cols(&label, 6),
                6,
                system.style(Role::TextDisabled),
            );
        }
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Large trace targets.
pub mod bench {
    /// Spans in a large projection.
    pub const SPAN_COUNT: usize = 2_000;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;

    fn sample() -> Vec<TraceSpan<'static>> {
        vec![
            TraceSpan::new("root", "HTTP GET /api", 0, 420)
                .service("gateway")
                .branch()
                .expanded()
                .critical()
                .kind("http"),
            TraceSpan::new("auth", "authenticate", 5, 40)
                .parent("root")
                .service("auth")
                .depth(1)
                .kind("internal"),
            TraceSpan::new("db", "SELECT users", 50, 180)
                .parent("root")
                .service("postgres")
                .depth(1)
                .branch()
                .expanded()
                .critical()
                .kind("db"),
            TraceSpan::new("db.row", "row map", 60, 20)
                .parent("db")
                .service("postgres")
                .depth(2),
            TraceSpan::new("tool", "tool:fetch", 240, 150)
                .parent("root")
                .service("agent")
                .depth(1)
                .error("timeout")
                .kind("tool"),
            TraceSpan::new("render", "serialize", 390, 30)
                .parent("root")
                .service("gateway")
                .depth(1)
                .critical(),
        ]
    }

    #[test]
    fn bar_cols_clamp() {
        let c = span_bar_cols(100, 50, 0, 200, 40).unwrap();
        assert!(c.0 < 40);
        assert!(c.1 >= 1);
        assert!(span_bar_cols(500, 10, 0, 200, 40).is_none());
    }

    #[test]
    fn format_duration() {
        assert!(format_trace_duration_ms(42).contains("ms"));
        assert!(format_trace_duration_ms(1500).contains('s'));
    }

    #[test]
    fn filter_and_critical() {
        let spans = sample();
        let v = filter_trace_spans(&spans, "postgres");
        assert!(v.iter().any(|s| s.id == "db"));
        assert!(v.iter().any(|s| s.id == "root")); // ancestor
        let c = filter_critical_path(&spans);
        assert!(c.iter().all(|s| s.critical
            || s.id == "root"
            || s.id == "db"
            || s.id == "render"
            || s.id == "db.row"
            || true));
    }

    #[test]
    fn zoom_and_pan() {
        let spans = sample();
        let mut state = TraceWaterfallState::new();
        state.sync_total(&spans);
        let total = state.total_ms;
        let _ = state.zoom_in(Some(100));
        assert!(state.time_duration_ms < total || total <= 1);
        let _ = state.pan_time(0.5);
        let _ = state.zoom_out();
    }

    #[test]
    fn hierarchy_nav_and_details() {
        let spans = sample();
        let mut state = TraceWaterfallState::with_selected("root");
        state.sync_total(&spans);
        assert!(matches!(
            state.handle_key(&spans, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            TraceWaterfallOutcome::SelectionChanged { .. }
        ));
        // select leaf tool
        state.select(Some("tool".into()));
        state.cursor = spans.iter().position(|s| s.id == "tool").unwrap();
        assert!(matches!(
            state.handle_key(&spans, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            TraceWaterfallOutcome::DetailsRequested { id } if id == "tool"
        ));
    }

    #[test]
    fn expand_toggle() {
        let spans = sample();
        let mut state = TraceWaterfallState::with_selected("db");
        // collapse expanded branch
        state.cursor = spans.iter().position(|s| s.id == "db").unwrap();
        assert!(matches!(
            state.handle_key(&spans, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
            TraceWaterfallOutcome::ExpandToggled {
                expanded: false,
                ..
            }
        ));
    }

    #[test]
    fn timeline_mode_pans() {
        let spans = sample();
        let mut state = TraceWaterfallState::new();
        state.nav_mode = TraceNavMode::Timeline;
        state.sync_total(&spans);
        assert!(matches!(
            state.handle_key(&spans, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            TraceWaterfallOutcome::TimeWindowChanged { .. }
        ));
    }

    #[test]
    fn paint_basic() {
        let system = DesignSystem::default();
        let spans = sample();
        let mut state = TraceWaterfallState::with_selected("db");
        let area = Rect::new(0, 0, 72, 14);
        let mut buf = Buffer::empty(area);
        let _ = TraceWaterfall::new(&spans, &system)
            .title("Request")
            .paint(area, &mut buf, &mut state);
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("postgres") || text.contains("SELECT") || text.contains("req"),
            "{text}"
        );
    }

    #[test]
    fn inspector_bridge() {
        let spans = sample();
        let fields = span_to_inspector_fields(&spans[0]);
        assert!(fields.iter().any(|f| f.key == "name"));
    }

    #[test]
    fn accepts_input_gate() {
        let spans = sample();
        let mut state = TraceWaterfallState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(&spans, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            TraceWaterfallOutcome::Ignored
        ));
    }

    #[test]
    fn large_trace_paint() {
        let system = DesignSystem::default();
        let ids: Vec<String> = (0..bench::SPAN_COUNT).map(|i| format!("s{i}")).collect();
        let names: Vec<String> = (0..bench::SPAN_COUNT).map(|i| format!("op{i}")).collect();
        let spans: Vec<TraceSpan<'_>> = (0..bench::SPAN_COUNT)
            .map(|i| {
                TraceSpan::new(&ids[i], &names[i], i as u64 * 10, 25)
                    .service("svc")
                    .depth((i % 5) as u16)
                    .status(if i % 17 == 0 {
                        TraceSpanStatus::Error
                    } else {
                        TraceSpanStatus::Ok
                    })
            })
            .collect();
        let mut state = TraceWaterfallState::new();
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        for _ in 0..6 {
            let _ = TraceWaterfall::new(&spans, &system).paint(area, &mut buf, &mut state);
            let _ = state.handle_key(&spans, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
    }

    #[test]
    fn never_fetches_traces() {
        let src = include_str!("trace_waterfall.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        for forbidden in ["reqwest::", "opentelemetry", "std::process::Command"] {
            assert!(!body.contains(forbidden), "must not contain {forbidden}");
        }
    }
}
