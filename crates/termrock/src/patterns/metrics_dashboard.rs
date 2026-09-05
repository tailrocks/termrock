// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! **MetricsDashboard** — reusable observability dashboard block.
//!
//! **Mission.** Compose metric cards, sparklines/gauges, alerts, and time
//! controls from **public** TermRock APIs only. Time range, refresh, comparison,
//! thresholds, drill-down, loading, partial failure, and responsive grid.
//! Prioritize trend and exception readability. Keyboard spatial navigation and
//! command-palette action ids. Narrow terminals collapse to a vertical summary.
//!
//! **vs [`super::blocks::OpsDashboardState`].** OpsDashboard is a thin region
//! router over DataTable + LogStream. MetricsDashboard owns metric-card grid
//! chrome, thresholds, comparison deltas, and layout contraction.
//!
//! Research: btop, Grafana concepts, observability TUIs, operating dashboards.
//!
//! Teaches: how to compose reusable observability dashboard block.
//!
//! Composes: [`crate::widgets::CommandEntry`], [`crate::widgets::LoadState`],
//! [`crate::widgets::MetricTile`], [`crate::widgets::MetricTileHealth`],
//! [`crate::widgets::MetricTilePresentation`], [`crate::widgets::MetricViz`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::{buffer::Buffer, layout::Rect};

use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    style::{DesignSystem, Role},
    widgets::{
        CommandEntry, LoadState, MetricTile, MetricTileHealth, MetricTilePresentation,
        SemanticStatus, StatusIndicator,
    },
};

/// Width at or below which layout becomes a vertical summary stack.
pub const METRICS_DASHBOARD_NARROW_MAX_WIDTH: u16 = 48;
/// Default refresh interval display (host timer).
pub const METRICS_DASHBOARD_DEFAULT_REFRESH_MS: u32 = 5_000;

// ── Time / comparison ───────────────────────────────────────────────────────

/// Dashboard time window (host resolves absolute times).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricsTimeRange {
    /// Last 5 minutes.
    M5,
    /// Last 15 minutes.
    #[default]
    M15,
    /// Last 1 hour.
    H1,
    /// Last 6 hours.
    H6,
    /// Last 24 hours.
    D1,
    /// Last 7 days.
    D7,
    /// Custom (host).
    Custom,
}

/// Alerts listed before the dashboard defers to `+N more`.
const ALERTS_SHOWN: usize = 3;

impl MetricsTimeRange {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H6 => "6h",
            Self::D1 => "24h",
            Self::D7 => "7d",
            Self::Custom => "custom",
        }
    }

    /// Human label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H6 => "6h",
            Self::D1 => "24h",
            Self::D7 => "7d",
            Self::Custom => "custom",
        }
    }

    /// Cycle forward.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::M5 => Self::M15,
            Self::M15 => Self::H1,
            Self::H1 => Self::H6,
            Self::H6 => Self::D1,
            Self::D1 => Self::D7,
            Self::D7 => Self::Custom,
            Self::Custom => Self::M5,
        }
    }

    /// Cycle backward.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::M5 => Self::Custom,
            Self::M15 => Self::M5,
            Self::H1 => Self::M15,
            Self::H6 => Self::H1,
            Self::D1 => Self::H6,
            Self::D7 => Self::D1,
            Self::Custom => Self::D7,
        }
    }
}

/// Comparison baseline for delta chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricsComparison {
    /// No comparison.
    #[default]
    None,
    /// Versus previous window of same length.
    PreviousPeriod,
    /// Versus same period yesterday.
    DayOverDay,
    /// Versus previous week.
    WeekOverWeek,
}

impl MetricsComparison {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PreviousPeriod => "prev",
            Self::DayOverDay => "dod",
            Self::WeekOverWeek => "wow",
        }
    }

    /// Short label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "—",
            Self::PreviousPeriod => "vs prev",
            Self::DayOverDay => "DoD",
            Self::WeekOverWeek => "WoW",
        }
    }

    /// Cycle.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::None => Self::PreviousPeriod,
            Self::PreviousPeriod => Self::DayOverDay,
            Self::DayOverDay => Self::WeekOverWeek,
            Self::WeekOverWeek => Self::None,
        }
    }
}

// ── Metric / alert model ────────────────────────────────────────────────────

/// Severity for dashboard alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[non_exhaustive]
pub enum MetricAlertSeverity {
    /// Info.
    #[default]
    Info,
    /// Warning.
    Warning,
    /// Critical.
    Critical,
}

impl MetricAlertSeverity {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    /// Letter.
    #[must_use]
    pub const fn letter(self) -> char {
        match self {
            Self::Info => 'i',
            Self::Warning => 'w',
            Self::Critical => 'c',
        }
    }

    /// Shared severity projection for recipe-owned status paint.
    #[must_use]
    pub const fn semantic(self) -> SemanticStatus {
        match self {
            Self::Info => SemanticStatus::Idle,
            Self::Warning => SemanticStatus::Warning,
            Self::Critical => SemanticStatus::Failed,
        }
    }
}

/// One alert row (host projection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricAlert<'a> {
    /// Id.
    pub id: &'a str,
    /// Severity.
    pub severity: MetricAlertSeverity,
    /// Message.
    pub message: &'a str,
    /// Optional related metric id.
    pub metric_id: Option<&'a str>,
}

impl<'a> MetricAlert<'a> {
    /// Construct.
    #[must_use]
    pub const fn new(id: &'a str, severity: MetricAlertSeverity, message: &'a str) -> Self {
        Self {
            id,
            severity,
            message,
            metric_id: None,
        }
    }

    /// Link to metric.
    #[must_use]
    pub const fn metric(mut self, id: &'a str) -> Self {
        self.metric_id = Some(id);
        self
    }
}

// ── Layout ──────────────────────────────────────────────────────────────────

/// Presentation mode from width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricsDashboardLayoutMode {
    /// Multi-column card grid.
    #[default]
    Grid,
    /// Single-column vertical summary.
    Summary,
}

impl MetricsDashboardLayoutMode {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Grid => "grid",
            Self::Summary => "summary",
        }
    }

    /// From width.
    #[must_use]
    pub const fn for_width(width: u16) -> Self {
        if width <= METRICS_DASHBOARD_NARROW_MAX_WIDTH {
            Self::Summary
        } else {
            Self::Grid
        }
    }
}

/// Slot rects after layout.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetricsDashboardSlots {
    /// Outer.
    pub root: Rect,
    /// Time / refresh toolbar.
    pub toolbar: Rect,
    /// Metric tiles area (grid or summary).
    pub metrics: Rect,
    /// Alerts strip.
    pub alerts: Rect,
    /// Footer / status.
    pub footer: Rect,
    /// Per-tile rects (grid order matching tiles slice).
    pub tiles: Vec<Rect>,
}

/// Layout dashboard chrome.
#[must_use]
pub fn layout_metrics_dashboard(
    area: Rect,
    tile_count: usize,
    alert_count: usize,
    mode: MetricsDashboardLayoutMode,
) -> MetricsDashboardSlots {
    if area.is_empty() {
        return MetricsDashboardSlots::default();
    }
    let mut y = area.y;
    let mut h = area.height;
    let toolbar = Rect {
        x: area.x,
        y,
        width: area.width,
        height: 1.min(h),
    };
    y = y.saturating_add(toolbar.height);
    h = h.saturating_sub(toolbar.height);

    let footer_h = 1u16.min(h);
    let alert_h = if alert_count == 0 || h < 4 {
        0
    } else {
        (1 + alert_count.min(3) as u16).min(h.saturating_sub(footer_h + 2))
    };
    let metrics_h = h.saturating_sub(footer_h).saturating_sub(alert_h).max(1);

    let metrics = Rect {
        x: area.x,
        y,
        width: area.width,
        height: metrics_h,
    };
    y = y.saturating_add(metrics_h);

    let alerts = Rect {
        x: area.x,
        y,
        width: area.width,
        height: alert_h,
    };
    y = y.saturating_add(alert_h);

    let footer = Rect {
        x: area.x,
        y,
        width: area.width,
        height: footer_h,
    };

    let tiles = match mode {
        MetricsDashboardLayoutMode::Summary => layout_summary_tiles(metrics, tile_count),
        MetricsDashboardLayoutMode::Grid => layout_grid_tiles(metrics, tile_count),
    };

    MetricsDashboardSlots {
        root: area,
        toolbar,
        metrics,
        alerts,
        footer,
        tiles,
    }
}

fn layout_summary_tiles(area: Rect, n: usize) -> Vec<Rect> {
    if n == 0 || area.is_empty() {
        return Vec::new();
    }
    let row_h = 1u16;
    let mut out = Vec::with_capacity(n);
    let mut y = area.y;
    for _ in 0..n {
        if y >= area.y.saturating_add(area.height) {
            break;
        }
        out.push(Rect {
            x: area.x,
            y,
            width: area.width,
            height: row_h,
        });
        y = y.saturating_add(row_h);
    }
    out
}

fn layout_grid_tiles(area: Rect, n: usize) -> Vec<Rect> {
    if n == 0 || area.is_empty() {
        return Vec::new();
    }
    // Prefer 2–4 columns by width
    let cols = if area.width >= 100 {
        4usize
    } else if area.width >= 72 {
        3
    } else {
        2
    };
    let cols = cols.min(n.max(1));
    let rows = n.div_ceil(cols);
    let cell_w = (area.width / cols as u16).max(1);
    let cell_h = (area.height / rows as u16).max(3);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let r = i / cols;
        let c = i % cols;
        let x = area.x.saturating_add(c as u16 * cell_w);
        let y = area.y.saturating_add(r as u16 * cell_h);
        let w = if c + 1 == cols {
            area.right().saturating_sub(x)
        } else {
            cell_w.saturating_sub(1).max(1)
        };
        let h = if r + 1 == rows {
            area.bottom().saturating_sub(y)
        } else {
            cell_h.saturating_sub(1).max(2)
        };
        out.push(Rect {
            x,
            y,
            width: w,
            height: h,
        });
    }
    out
}

// ── Outcomes / commands ─────────────────────────────────────────────────────

/// Typed outcomes — host owns scrape/query/refresh effects.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetricsDashboardOutcome {
    /// No change.
    Ignored,
    /// Focused tile changed.
    TileFocused {
        /// Metric id.
        id: String,
    },
    /// Drill-down / open detail for metric.
    DrillDownRequested {
        /// Metric id.
        id: String,
    },
    /// Alert activated.
    AlertActivated {
        /// Alert id.
        id: String,
    },
    /// Time range changed (state already updated).
    TimeRangeChanged(MetricsTimeRange),
    /// Comparison mode changed.
    ComparisonChanged(MetricsComparison),
    /// Host should refresh all metrics.
    RefreshRequested,
    /// Retry failed tiles only.
    RetryFailedRequested,
    /// Pause auto-refresh.
    PauseToggled {
        /// Paused after.
        paused: bool,
    },
    /// Open command palette with dashboard actions.
    CommandPaletteRequested,
    /// Layout mode changed (grid ↔ summary force).
    LayoutModeChanged(MetricsDashboardLayoutMode),
    /// Focus moved to alerts strip.
    AlertsFocused,
    /// Focus moved to toolbar.
    ToolbarFocused,
}

/// Stable command ids for CommandPalette integration.
pub mod commands {
    /// Refresh.
    pub const REFRESH: &str = "metrics.refresh";
    /// Retry failed.
    pub const RETRY_FAILED: &str = "metrics.retry_failed";
    /// Cycle time range.
    pub const TIME_NEXT: &str = "metrics.time_next";
    /// Previous time range.
    pub const TIME_PREV: &str = "metrics.time_prev";
    /// Cycle comparison.
    pub const COMPARE_CYCLE: &str = "metrics.compare_cycle";
    /// Pause auto-refresh.
    pub const PAUSE: &str = "metrics.pause";
    /// Force summary layout.
    pub const LAYOUT_SUMMARY: &str = "metrics.layout_summary";
    /// Force grid layout.
    pub const LAYOUT_GRID: &str = "metrics.layout_grid";
    /// Drill focused tile.
    pub const DRILL: &str = "metrics.drill";
}

/// Build command palette entries for the dashboard (host wires handlers).
#[must_use]
pub fn metrics_dashboard_commands() -> Vec<CommandEntry<&'static str>> {
    vec![
        CommandEntry::new(commands::REFRESH, "Refresh metrics").group("metrics"),
        CommandEntry::new(commands::RETRY_FAILED, "Retry failed tiles").group("metrics"),
        CommandEntry::new(commands::TIME_NEXT, "Next time range").group("metrics"),
        CommandEntry::new(commands::TIME_PREV, "Previous time range").group("metrics"),
        CommandEntry::new(commands::COMPARE_CYCLE, "Cycle comparison").group("metrics"),
        CommandEntry::new(commands::PAUSE, "Toggle pause refresh").group("metrics"),
        CommandEntry::new(commands::LAYOUT_SUMMARY, "Summary layout").group("metrics"),
        CommandEntry::new(commands::LAYOUT_GRID, "Grid layout").group("metrics"),
        CommandEntry::new(commands::DRILL, "Drill into focused metric").group("metrics"),
    ]
}

/// Apply a command id (from palette) to state.
pub fn apply_metrics_command(
    state: &mut MetricsDashboardState,
    command_id: &str,
    tiles: &[MetricTile<'_>],
) -> MetricsDashboardOutcome {
    match command_id {
        commands::REFRESH => MetricsDashboardOutcome::RefreshRequested,
        commands::RETRY_FAILED => MetricsDashboardOutcome::RetryFailedRequested,
        commands::TIME_NEXT => {
            state.time_range = state.time_range.next();
            MetricsDashboardOutcome::TimeRangeChanged(state.time_range)
        }
        commands::TIME_PREV => {
            state.time_range = state.time_range.prev();
            MetricsDashboardOutcome::TimeRangeChanged(state.time_range)
        }
        commands::COMPARE_CYCLE => {
            state.comparison = state.comparison.next();
            MetricsDashboardOutcome::ComparisonChanged(state.comparison)
        }
        commands::PAUSE => {
            state.paused = !state.paused;
            MetricsDashboardOutcome::PauseToggled {
                paused: state.paused,
            }
        }
        commands::LAYOUT_SUMMARY => {
            state.layout_override = Some(MetricsDashboardLayoutMode::Summary);
            MetricsDashboardOutcome::LayoutModeChanged(MetricsDashboardLayoutMode::Summary)
        }
        commands::LAYOUT_GRID => {
            state.layout_override = Some(MetricsDashboardLayoutMode::Grid);
            MetricsDashboardOutcome::LayoutModeChanged(MetricsDashboardLayoutMode::Grid)
        }
        commands::DRILL => {
            if let Some(t) = tiles.get(state.focus_tile) {
                MetricsDashboardOutcome::DrillDownRequested {
                    id: t.id.to_string(),
                }
            } else {
                MetricsDashboardOutcome::Ignored
            }
        }
        _ => MetricsDashboardOutcome::Ignored,
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Focus zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum MetricsFocus {
    /// Metric grid.
    #[default]
    Tiles,
    /// Alerts strip.
    Alerts,
    /// Toolbar (time/refresh).
    Toolbar,
}

/// Dashboard interaction state.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricsDashboardState {
    /// Time range.
    pub time_range: MetricsTimeRange,
    /// Comparison.
    pub comparison: MetricsComparison,
    /// Refresh cadence ms (chrome).
    pub refresh_ms: u32,
    /// Auto-refresh paused.
    pub paused: bool,
    /// Global load (all tiles).
    pub load: LoadState,
    /// Focus zone.
    pub focus: MetricsFocus,
    /// Focused tile index.
    pub focus_tile: usize,
    /// Focused alert index.
    pub focus_alert: usize,
    /// Layout override (None = auto from width).
    pub layout_override: Option<MetricsDashboardLayoutMode>,
    /// Grid column count cache for spatial nav (set on paint).
    pub grid_cols: usize,
    /// Last slots.
    pub slots: MetricsDashboardSlots,
    accepts_input: bool,
}

impl Default for MetricsDashboardState {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsDashboardState {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            time_range: MetricsTimeRange::M15,
            comparison: MetricsComparison::None,
            refresh_ms: METRICS_DASHBOARD_DEFAULT_REFRESH_MS,
            paused: false,
            load: LoadState::Ready { count: 0 },
            focus: MetricsFocus::Tiles,
            focus_tile: 0,
            focus_alert: 0,
            layout_override: None,
            grid_cols: 2,
            slots: MetricsDashboardSlots::default(),
            accepts_input: true,
        }
    }

    /// Host input gate.
    pub fn set_accepts_input(&mut self, on: bool) {
        self.accepts_input = on;
    }

    /// Accepts input.
    #[must_use]
    pub const fn accepts_input(&self) -> bool {
        self.accepts_input
    }

    /// Effective layout mode.
    #[must_use]
    pub fn layout_mode(&self, width: u16) -> MetricsDashboardLayoutMode {
        self.layout_override
            .unwrap_or_else(|| MetricsDashboardLayoutMode::for_width(width))
    }

    /// Focused metric id.
    #[must_use]
    pub fn focused_metric_id<'a>(&self, tiles: &[MetricTile<'a>]) -> Option<&'a str> {
        tiles.get(self.focus_tile).map(|t| t.id)
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        tiles: &[MetricTile<'_>],
        alerts: &[MetricAlert<'_>],
    ) -> MetricsDashboardOutcome {
        if !self.accepts_input || !key.is_press() {
            return MetricsDashboardOutcome::Ignored;
        }

        // Global chords
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && !key.modifiers.contains(KeyModifiers::ALT)
        {
            match key.code {
                KeyCode::Char('r' | 'R') => {
                    return MetricsDashboardOutcome::RefreshRequested;
                }
                KeyCode::Char('p' | 'P') => {
                    self.paused = !self.paused;
                    return MetricsDashboardOutcome::PauseToggled {
                        paused: self.paused,
                    };
                }
                KeyCode::Char('k' | 'K') => {
                    return MetricsDashboardOutcome::CommandPaletteRequested;
                }
                KeyCode::Char('t' | 'T') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.time_range = self.time_range.prev();
                    return MetricsDashboardOutcome::TimeRangeChanged(self.time_range);
                }
                KeyCode::Char('t' | 'T') => {
                    self.time_range = self.time_range.next();
                    return MetricsDashboardOutcome::TimeRangeChanged(self.time_range);
                }
                KeyCode::Char('d' | 'D') => {
                    self.comparison = self.comparison.next();
                    return MetricsDashboardOutcome::ComparisonChanged(self.comparison);
                }
                KeyCode::Char('f' | 'F') => {
                    return MetricsDashboardOutcome::RetryFailedRequested;
                }
                _ => {}
            }
        }

        // Tab zones
        if key.code == KeyCode::Tab {
            self.focus = if key.modifiers.contains(KeyModifiers::SHIFT) {
                match self.focus {
                    MetricsFocus::Tiles => MetricsFocus::Toolbar,
                    MetricsFocus::Alerts => MetricsFocus::Tiles,
                    MetricsFocus::Toolbar => MetricsFocus::Alerts,
                }
            } else {
                match self.focus {
                    MetricsFocus::Tiles => MetricsFocus::Alerts,
                    MetricsFocus::Alerts => MetricsFocus::Toolbar,
                    MetricsFocus::Toolbar => MetricsFocus::Tiles,
                }
            };
            return match self.focus {
                MetricsFocus::Alerts => MetricsDashboardOutcome::AlertsFocused,
                MetricsFocus::Toolbar => MetricsDashboardOutcome::ToolbarFocused,
                MetricsFocus::Tiles => {
                    if let Some(t) = tiles.get(self.focus_tile) {
                        MetricsDashboardOutcome::TileFocused {
                            id: t.id.to_string(),
                        }
                    } else {
                        MetricsDashboardOutcome::Ignored
                    }
                }
            };
        }

        match self.focus {
            MetricsFocus::Toolbar => self.handle_toolbar_key(key),
            MetricsFocus::Alerts => self.handle_alerts_key(key, alerts),
            MetricsFocus::Tiles => self.handle_tiles_key(key, tiles),
        }
    }

    fn handle_toolbar_key(&mut self, key: KeyEvent) -> MetricsDashboardOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                self.time_range = self.time_range.prev();
                MetricsDashboardOutcome::TimeRangeChanged(self.time_range)
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.time_range = self.time_range.next();
                MetricsDashboardOutcome::TimeRangeChanged(self.time_range)
            }
            KeyCode::Char('c') if key.modifiers.is_empty() => {
                self.comparison = self.comparison.next();
                MetricsDashboardOutcome::ComparisonChanged(self.comparison)
            }
            KeyCode::Enter | KeyCode::Char('r') => MetricsDashboardOutcome::RefreshRequested,
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                MetricsDashboardOutcome::PauseToggled {
                    paused: self.paused,
                }
            }
            _ => MetricsDashboardOutcome::Ignored,
        }
    }

    fn handle_alerts_key(
        &mut self,
        key: KeyEvent,
        alerts: &[MetricAlert<'_>],
    ) -> MetricsDashboardOutcome {
        if alerts.is_empty() {
            return MetricsDashboardOutcome::Ignored;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.focus_alert = (self.focus_alert + 1).min(alerts.len() - 1);
                MetricsDashboardOutcome::Ignored
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.focus_alert = self.focus_alert.saturating_sub(1);
                MetricsDashboardOutcome::Ignored
            }
            KeyCode::Enter => {
                let a = &alerts[self.focus_alert.min(alerts.len() - 1)];
                MetricsDashboardOutcome::AlertActivated {
                    id: a.id.to_string(),
                }
            }
            _ => MetricsDashboardOutcome::Ignored,
        }
    }

    fn handle_tiles_key(
        &mut self,
        key: KeyEvent,
        tiles: &[MetricTile<'_>],
    ) -> MetricsDashboardOutcome {
        if tiles.is_empty() {
            return MetricsDashboardOutcome::Ignored;
        }
        self.focus_tile = self.focus_tile.min(tiles.len() - 1);
        let cols = self.grid_cols.max(1);

        match key.code {
            KeyCode::Right | KeyCode::Char('l') if key.modifiers.is_empty() => {
                let next = (self.focus_tile + 1).min(tiles.len() - 1);
                self.focus_tile = next;
                MetricsDashboardOutcome::TileFocused {
                    id: tiles[next].id.to_string(),
                }
            }
            KeyCode::Left | KeyCode::Char('h') if key.modifiers.is_empty() => {
                let next = self.focus_tile.saturating_sub(1);
                self.focus_tile = next;
                MetricsDashboardOutcome::TileFocused {
                    id: tiles[next].id.to_string(),
                }
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers.is_empty() => {
                let next = (self.focus_tile + cols).min(tiles.len() - 1);
                self.focus_tile = next;
                MetricsDashboardOutcome::TileFocused {
                    id: tiles[next].id.to_string(),
                }
            }
            KeyCode::Up | KeyCode::Char('k') if key.modifiers.is_empty() => {
                let next = self.focus_tile.saturating_sub(cols);
                self.focus_tile = next;
                MetricsDashboardOutcome::TileFocused {
                    id: tiles[next].id.to_string(),
                }
            }
            KeyCode::Home => {
                self.focus_tile = 0;
                MetricsDashboardOutcome::TileFocused {
                    id: tiles[0].id.to_string(),
                }
            }
            KeyCode::End => {
                self.focus_tile = tiles.len() - 1;
                MetricsDashboardOutcome::TileFocused {
                    id: tiles[self.focus_tile].id.to_string(),
                }
            }
            KeyCode::Enter => MetricsDashboardOutcome::DrillDownRequested {
                id: tiles[self.focus_tile].id.to_string(),
            },
            _ => MetricsDashboardOutcome::Ignored,
        }
    }

    /// Mouse click tiles/alerts.
    pub fn handle_mouse(
        &mut self,
        event: MouseEvent,
        tiles: &[MetricTile<'_>],
        alerts: &[MetricAlert<'_>],
    ) -> MetricsDashboardOutcome {
        if !self.accepts_input {
            return MetricsDashboardOutcome::Ignored;
        }
        if !matches!(event.kind, MouseEventKind::Down(MouseButton::Left)) {
            return MetricsDashboardOutcome::Ignored;
        }
        let pos = event.position;
        if !self.slots.toolbar.is_empty() && self.slots.toolbar.contains(pos) {
            self.focus = MetricsFocus::Toolbar;
            return MetricsDashboardOutcome::ToolbarFocused;
        }
        if !self.slots.alerts.is_empty() && self.slots.alerts.contains(pos) {
            self.focus = MetricsFocus::Alerts;
            // row by y
            let rel = pos.y.saturating_sub(self.slots.alerts.y) as usize;
            if rel < alerts.len() {
                self.focus_alert = rel;
            }
            return MetricsDashboardOutcome::AlertsFocused;
        }
        for (i, rect) in self.slots.tiles.iter().enumerate() {
            if rect.contains(pos) {
                self.focus = MetricsFocus::Tiles;
                self.focus_tile = i;
                if let Some(t) = tiles.get(i) {
                    return MetricsDashboardOutcome::TileFocused {
                        id: t.id.to_string(),
                    };
                }
            }
        }
        MetricsDashboardOutcome::Ignored
    }
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// Metrics dashboard paint.
#[derive(Debug, Clone, Copy)]
pub struct MetricsDashboard<'a> {
    tiles: &'a [MetricTile<'a>],
    alerts: &'a [MetricAlert<'a>],
    system: &'a DesignSystem,
    focused: bool,
    title: Option<&'a str>,
    hints: bool,
}

impl<'a> MetricsDashboard<'a> {
    /// Tiles + alerts + system.
    #[must_use]
    pub const fn new(
        tiles: &'a [MetricTile<'a>],
        alerts: &'a [MetricAlert<'a>],
        system: &'a DesignSystem,
    ) -> Self {
        Self {
            tiles,
            alerts,
            system,
            focused: true,
            hints: true,
            title: None,
        }
    }

    /// Title.
    #[must_use]
    pub const fn title(mut self, t: &'a str) -> Self {
        self.title = Some(t);
        self
    }

    /// Whether this surface owns the frame's hint row.
    ///
    /// A dashboard embedded in a shell does not: the shell already has a
    /// status bar, and two footers on one frame is two answers to the same
    /// question (plans/017 §B2 — one hint row per default frame).
    #[must_use]
    pub const fn hints(mut self, on: bool) -> Self {
        self.hints = on;
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
    /// Paint using public Sparkline/Gauge APIs only.
    pub fn render(&self, area: Rect, buffer: &mut Buffer, state: &mut MetricsDashboardState) {
        if area.is_empty() {
            return;
        }
        let mode = state.layout_mode(area.width);
        let slots = layout_metrics_dashboard(area, self.tiles.len(), self.alerts.len(), mode);
        // grid cols for nav
        state.grid_cols = match mode {
            MetricsDashboardLayoutMode::Summary => 1,
            MetricsDashboardLayoutMode::Grid => {
                if area.width >= 100 {
                    4
                } else if area.width >= 72 {
                    3
                } else {
                    2
                }
            }
        };
        if !self.tiles.is_empty() {
            state.focus_tile = state.focus_tile.min(self.tiles.len() - 1);
        }
        if !self.alerts.is_empty() {
            state.focus_alert = state.focus_alert.min(self.alerts.len() - 1);
        }

        // Toolbar
        if !slots.toolbar.is_empty() {
            let title = self.title.unwrap_or("metrics");
            let pause = if state.paused { "paused" } else { "live" };
            let failed = self
                .tiles
                .iter()
                .filter(|t| matches!(t.health, MetricTileHealth::Failed))
                .count();
            // Title band carries what the operator changed, not the whole
            // control state: refresh cadence and layout mode are visible in
            // the frame itself, and a default comparison says nothing
            // (information budget, plans/017 Part B).
            let mut line = format!("{title} · {} · {pause}", state.time_range.label());
            if !matches!(state.comparison, MetricsComparison::None) {
                line.push_str(" · ");
                line.push_str(state.comparison.label());
            }
            if failed > 0 {
                line.push_str(&format!(" · {failed} failed"));
            }
            let style = if matches!(state.focus, MetricsFocus::Toolbar) && self.focused {
                self.system.style(Role::Focus)
            } else {
                self.system.style(Role::TextStrong)
            };
            self.system.paint_row(
                buffer,
                Rect::new(slots.toolbar.x, slots.toolbar.y, slots.toolbar.width, 1),
                &line,
                style,
            );
        }

        // Tiles
        for (i, tile) in self.tiles.iter().enumerate() {
            let Some(rect) = slots.tiles.get(i).copied() else {
                break;
            };
            if rect.is_empty() {
                continue;
            }
            let focused =
                matches!(state.focus, MetricsFocus::Tiles) && i == state.focus_tile && self.focused;
            let presentation = match mode {
                MetricsDashboardLayoutMode::Summary => MetricTilePresentation::Row,
                MetricsDashboardLayoutMode::Grid => MetricTilePresentation::Card,
            };
            tile.view(self.system)
                .presentation(presentation)
                .focused(focused)
                .paint(rect, buffer);
        }

        // Alerts
        if !slots.alerts.is_empty() && !self.alerts.is_empty() {
            let mut y = slots.alerts.y;
            let max_y = slots.alerts.bottom();
            for (i, a) in self.alerts.iter().enumerate().take(ALERTS_SHOWN) {
                if y >= max_y {
                    break;
                }
                let focused = matches!(state.focus, MetricsFocus::Alerts)
                    && i == state.focus_alert
                    && self.focused;
                let mark = if focused { "›" } else { " " };
                let indicator =
                    StatusIndicator::new(a.severity.semantic(), self.system).label(a.severity.id());
                let status_text = indicator.text(None);
                let line = format!("{mark} {status_text} · {}", a.message);
                self.system.paint_row(
                    buffer,
                    Rect::new(slots.alerts.x, y, slots.alerts.width, 1),
                    &line,
                    self.system.style(Role::Text),
                );
                self.system.paint_row(
                    buffer,
                    Rect::new(slots.alerts.x, y, slots.alerts.width.min(1), 1),
                    mark,
                    self.system.style(if focused {
                        Role::Focus
                    } else {
                        Role::TextMuted
                    }),
                );
                if slots.alerts.width > 2 {
                    indicator.paint(
                        Rect::new(
                            slots.alerts.x.saturating_add(2),
                            y,
                            slots.alerts.width.saturating_sub(2),
                            1,
                        ),
                        buffer,
                    );
                }
                y = y.saturating_add(1);
            }
            // An alert list that stops at three says so.
            let hidden = self.alerts.len().saturating_sub(ALERTS_SHOWN);
            if let Some(note) = crate::text::more_note(hidden)
                && y < max_y
            {
                self.system.paint_row(
                    buffer,
                    Rect::new(slots.alerts.x, y, slots.alerts.width, 1),
                    &note,
                    self.system.style(Role::TextMuted),
                );
            }
        }

        // Footer
        if self.hints && !slots.footer.is_empty() {
            let failed = self
                .tiles
                .iter()
                .filter(|t| matches!(t.health, MetricTileHealth::Failed))
                .count();
            let footer = if failed > 0 {
                "Tab zones · hjkl tiles · Enter drill · C-r refresh · C-k cmds"
            } else {
                "Tab zones · hjkl tiles · Enter drill · C-t range · C-k cmds"
            };
            self.system.paint_row(
                buffer,
                Rect::new(slots.footer.x, slots.footer.y, slots.footer.width, 1),
                footer,
                self.system.style(Role::TextMuted),
            );
        }

        state.slots = slots;
    }
}

// ── Bench ───────────────────────────────────────────────────────────────────

/// Dashboard scale targets.
pub mod bench {
    /// Metric tiles.
    pub const TILE_COUNT: usize = 24;
    /// Samples per sparkline.
    pub const SAMPLE_LEN: usize = 64;
    /// Paint frames.
    pub const PAINT_FRAMES: u32 = 40;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::DesignSystem;
    use crate::widgets::MetricViz;

    fn samples() -> &'static [f64] {
        &[1.0, 2.0, 3.0, 2.5, 4.0, 3.5, 5.0, 4.2]
    }

    fn tiles() -> Vec<MetricTile<'static>> {
        static THR: &[f64] = &[70.0, 90.0];
        vec![
            MetricTile::new("cpu", "CPU", "42%")
                .unit("util")
                .delta("+2.1%", true)
                .samples(samples())
                .thresholds(THR)
                .health(MetricTileHealth::Ok),
            MetricTile::new("mem", "Memory", "71%")
                .gauge(71.0)
                .thresholds(THR)
                .health(MetricTileHealth::Warning),
            MetricTile::new("rps", "RPS", "1.2k")
                .samples(samples())
                .delta("−3%", false)
                .health(MetricTileHealth::Ok),
            MetricTile::new("err", "Errors", "—")
                .failed("scrape timeout")
                .viz(MetricViz::ValueOnly),
        ]
    }

    fn alerts() -> Vec<MetricAlert<'static>> {
        vec![
            MetricAlert::new("a1", MetricAlertSeverity::Warning, "mem > 70%").metric("mem"),
            MetricAlert::new("a2", MetricAlertSeverity::Critical, "error scrape failed")
                .metric("err"),
        ]
    }

    #[test]
    fn layout_grid_and_summary() {
        let wide = layout_metrics_dashboard(
            Rect::new(0, 0, 100, 24),
            4,
            2,
            MetricsDashboardLayoutMode::Grid,
        );
        assert_eq!(wide.tiles.len(), 4);
        assert!(wide.metrics.height > 0);
        let narrow = layout_metrics_dashboard(
            Rect::new(0, 0, 40, 16),
            4,
            1,
            MetricsDashboardLayoutMode::Summary,
        );
        assert!(narrow.tiles.iter().all(|t| t.height == 1));
    }

    #[test]
    fn spatial_nav() {
        let tiles = tiles();
        let mut state = MetricsDashboardState::new();
        state.grid_cols = 2;
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &tiles,
            &[],
        );
        assert!(matches!(
            out,
            MetricsDashboardOutcome::TileFocused { id } if id == "mem"
        ));
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &tiles,
            &[],
        );
        assert!(matches!(out, MetricsDashboardOutcome::TileFocused { .. }));
    }

    #[test]
    fn time_range_and_refresh() {
        let tiles = tiles();
        let mut state = MetricsDashboardState::new();
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
                &tiles,
                &[]
            ),
            MetricsDashboardOutcome::TimeRangeChanged(MetricsTimeRange::H1)
        ));
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL),
                &tiles,
                &[]
            ),
            MetricsDashboardOutcome::RefreshRequested
        ));
    }

    #[test]
    fn command_list_and_apply() {
        let cmds = metrics_dashboard_commands();
        assert!(cmds.iter().any(|c| c.id == commands::REFRESH));
        let tiles = tiles();
        let mut state = MetricsDashboardState::new();
        assert!(matches!(
            apply_metrics_command(&mut state, commands::COMPARE_CYCLE, &tiles),
            MetricsDashboardOutcome::ComparisonChanged(MetricsComparison::PreviousPeriod)
        ));
    }

    #[test]
    fn paint_grid_and_narrow() {
        let system = DesignSystem::default();
        let tiles = tiles();
        let alerts = alerts();
        let mut state = MetricsDashboardState::new();
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        let _ = MetricsDashboard::new(&tiles, &alerts, &system)
            .title("Ops")
            .render(area, &mut buf, &mut state);
        assert!(!state.slots.tiles.is_empty());
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("CPU") || text.contains("ops") || text.contains("42"),
            "{text}"
        );

        let area_n = Rect::new(0, 0, 40, 12);
        let mut buf_n = Buffer::empty(area_n);
        let _ =
            MetricsDashboard::new(&tiles, &alerts, &system).render(area_n, &mut buf_n, &mut state);
        assert_eq!(state.layout_mode(40), MetricsDashboardLayoutMode::Summary);
    }

    #[test]
    fn drill_and_alert() {
        let tiles = tiles();
        let alerts = alerts();
        let mut state = MetricsDashboardState::new();
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &tiles,
                &alerts
            ),
            MetricsDashboardOutcome::DrillDownRequested { id } if id == "cpu"
        ));
        state.focus = MetricsFocus::Alerts;
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                &tiles,
                &alerts
            ),
            MetricsDashboardOutcome::AlertActivated { .. }
        ));
    }

    #[test]
    fn accepts_input_gate() {
        let mut state = MetricsDashboardState::new();
        state.set_accepts_input(false);
        assert!(matches!(
            state.handle_key(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                &tiles(),
                &[]
            ),
            MetricsDashboardOutcome::Ignored
        ));
    }

    #[test]
    fn large_dashboard_paint() {
        let system = DesignSystem::default();
        let sample_store: Vec<Vec<f64>> = (0..bench::TILE_COUNT)
            .map(|i| {
                (0..bench::SAMPLE_LEN)
                    .map(|j| (i + j) as f64 * 0.1)
                    .collect()
            })
            .collect();
        let titles: Vec<String> = (0..bench::TILE_COUNT).map(|i| format!("m{i}")).collect();
        let values: Vec<String> = (0..bench::TILE_COUNT).map(|i| format!("{i}")).collect();
        let ids: Vec<String> = (0..bench::TILE_COUNT).map(|i| format!("id{i}")).collect();
        let tiles: Vec<MetricTile<'_>> = (0..bench::TILE_COUNT)
            .map(|i| {
                MetricTile::new(&ids[i], &titles[i], &values[i])
                    .samples(&sample_store[i])
                    .health(if i % 7 == 0 {
                        MetricTileHealth::Warning
                    } else {
                        MetricTileHealth::Ok
                    })
            })
            .collect();
        let mut state = MetricsDashboardState::new();
        let area = Rect::new(0, 0, 120, 36);
        let mut buf = Buffer::empty(area);
        for _ in 0..6 {
            let _ = MetricsDashboard::new(&tiles, &[], &system).render(area, &mut buf, &mut state);
            let _ = state.handle_key(
                KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
                &tiles,
                &[],
            );
        }
    }

    #[test]
    fn uses_only_public_viz() {
        // Guard: the dashboard composes the public tile widget and neither
        // reimplements chart raster nor re-rolls tile chrome (plans/016).
        let src = include_str!("metrics_dashboard.rs");
        let body = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(body.contains("tile.view("));
        assert!(!body.contains("braille_plot"));
        assert!(
            !body.contains("Sparkline::"),
            "raster belongs to MetricTile"
        );
        assert!(!body.contains("Gauge::"), "raster belongs to MetricTile");
    }
}
