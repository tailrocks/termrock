// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Ops dashboard recipe: metrics strip + main + log + status.
//!
//! Thin wrapper over [`crate::patterns::layout_app_shell`]
//! ([`AppShellRecipe::Dashboard`]).
//!
//! Teaches: how to compose an ops dashboard's geometry — metric strip, main
//! pane, log and status bar — as slots a host paints into.
//!
//! Composes: [`crate::widgets::ColumnModel`], [`crate::widgets::DataColumn`],
//! [`crate::widgets::DataColumnWidth`], [`crate::widgets::DataTableOutcome`],
//! [`crate::widgets::DataTableState`], [`crate::widgets::LogStreamOutcome`],
//! [`crate::widgets::LogStreamState`],
//! [`crate::widgets::ObjectInspectorState`].
//!
//! Copy-adapt: keep the widget composition and the focus routing;
//! replace the domain types, the wording, and the effects with your own.
use ratatui_core::layout::Rect;

use super::app_shell::{AppShellConfig, AppShellRecipe, layout_app_shell};

/// Slots for an ops-style dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsDashboardSlots {
    /// Top metrics / sparkline strip.
    pub metrics: Rect,
    /// Primary content (table, resource list).
    pub main: Rect,
    /// Log / event stream.
    pub log: Rect,
    /// Bottom status / hints.
    pub status: Rect,
}

/// Layout knobs for [`layout_ops_dashboard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpsDashboardLayout {
    /// Metrics strip height.
    pub metrics_height: u16,
    /// Log pane height.
    pub log_height: u16,
    /// Status height.
    pub status_height: u16,
}

impl Default for OpsDashboardLayout {
    fn default() -> Self {
        Self {
            metrics_height: 3,
            log_height: 8,
            status_height: 1,
        }
    }
}

/// Resolves ops dashboard rectangles.
#[must_use]
pub fn layout_ops_dashboard(area: Rect, config: OpsDashboardLayout) -> OpsDashboardSlots {
    let shell = layout_app_shell(
        area,
        AppShellConfig {
            recipe: AppShellRecipe::Dashboard,
            header_height: 0,
            sidebar_width: 0,
            inspector_width: 0,
            footer_height: config.status_height.max(1),
            command_height: 0,
            metrics_height: config.metrics_height.max(1),
            log_height: config.log_height.max(1),
            lifecycle: Default::default(),
            inline: false,
        },
    );

    // Dashboard recipe may collapse log on narrow viewports — keep zero-height
    // placeholders so callers always get four rects.
    OpsDashboardSlots {
        metrics: shell.metrics.unwrap_or(Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 0,
        }),
        main: shell.main,
        log: shell.log.unwrap_or(Rect {
            x: area.x,
            y: shell.main.y.saturating_add(shell.main.height),
            width: area.width,
            height: 0,
        }),
        status: shell.footer.unwrap_or(Rect {
            x: area.x,
            y: area.y.saturating_add(area.height.saturating_sub(1)),
            width: area.width,
            height: 1.min(area.height),
        }),
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn reference_paint_fills_every_slot_within_budget() {
        use crate::style::DesignSystem;
        use crate::widgets::{
            ColumnModel, DataColumn, DataColumnWidth, LogLevel, LogLine, MetricTile, StatusSlot,
        };
        use ratatui_core::buffer::Buffer;

        let system = DesignSystem::default();
        let tiles = [
            MetricTile::new("cpu", "CPU", "42%"),
            MetricTile::new("mem", "Memory", "7.1G"),
        ];
        let columns = ColumnModel::new(vec![
            DataColumn::new("pod", "Pod", DataColumnWidth::Min(10)),
            DataColumn::new("age", "Age", DataColumnWidth::Fixed(6)),
        ]);
        let row0: &[&str] = &["api-7", "3h"];
        let rows = [(1u64, row0)];
        let logs = [LogLine::new("1", LogLevel::Info, "started").timestamp("12:00:00")];
        let hints = [StatusSlot::new("tab", "tab pane")];
        let view = OpsDashboardView {
            tiles: &tiles,
            columns: &columns,
            rows: &rows,
            logs: &logs,
            hints: &hints,
        };
        let mut state = OpsDashboardState::<u64, &str>::new();
        let area = Rect::new(0, 0, 80, 24);
        let mut buffer = Buffer::empty(area);
        let slots = paint_ops_dashboard(
            area,
            &mut buffer,
            &system,
            OpsDashboardLayout::default(),
            view,
            &mut state,
        );

        for (name, rect) in [
            ("metrics", slots.metrics),
            ("main", slots.main),
            ("log", slots.log),
            ("status", slots.status),
        ] {
            if rect.height == 0 {
                continue;
            }
            let painted = (rect.x..rect.right()).any(|x| {
                (rect.y..rect.bottom()).any(|y| !buffer[(x, y)].symbol().trim().is_empty())
            });
            assert!(painted, "{name} slot painted nothing");
        }

        // Focus is visible: moving it changes the frame.
        let mut moved = OpsDashboardState::<u64, &str>::new();
        moved.region = OpsRegion::Log;
        let mut other = Buffer::empty(area);
        paint_ops_dashboard(
            area,
            &mut other,
            &system,
            OpsDashboardLayout::default(),
            view,
            &mut moved,
        );
        assert_ne!(
            buffer.content(),
            other.content(),
            "which pane owns focus must be visible"
        );
    }
    use super::*;

    #[test]
    fn ops_dashboard_fills_height() {
        let slots = layout_ops_dashboard(Rect::new(0, 0, 80, 30), OpsDashboardLayout::default());
        let sum = slots.metrics.height + slots.main.height + slots.log.height + slots.status.height;
        assert_eq!(sum, 24);
        assert!(slots.main.height >= slots.log.height);
    }

    #[test]
    fn ops_dashboard_narrow_keeps_main() {
        let slots = layout_ops_dashboard(Rect::new(0, 0, 40, 20), OpsDashboardLayout::default());
        assert!(slots.main.height > 0);
        assert_eq!(
            slots.metrics.height + slots.main.height + slots.log.height + slots.status.height,
            16
        );
    }
}

// ── Ops dashboard state machine (example composite) ──────────────────────────

use crate::style::DesignSystem;
use crate::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    widgets::{
        ColumnModel, DataTable, DataTableOutcome, DataTableState, LogLine, LogStream,
        LogStreamOutcome, LogStreamState, MetricTile, MetricTilePresentation, ObjectInspectorState,
        StatusBar, StatusBarState, StatusSlot,
    },
};
use ratatui_core::{buffer::Buffer, widgets::StatefulWidget};

// ── OpsDashboard ────────────────────────────────────────────────────────────

/// Ops dashboard outcomes (never execute domain effects).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpsDashboardOutcome<RowId, ColId> {
    /// No change.
    Ignored,
    /// Focus region changed.
    FocusRegion(OpsRegion),
    /// Table interaction bubbled.
    Table(DataTableOutcome<RowId, ColId>),
    /// Log interaction.
    Log(LogStreamOutcome),
    /// Request time-range change (consumer applies).
    TimeRangeRequested,
    /// Retry failed load (consumer).
    RetryRequested,
}

/// Focusable regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum OpsRegion {
    /// Metrics strip.
    Metrics,
    /// Main table.
    #[default]
    Main,
    /// Log stream.
    Log,
    /// Status.
    Status,
}

/// Controlled ops dashboard chrome state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsDashboardState<RowId: Clone + Ord, ColId: Clone + PartialEq> {
    /// Focused region.
    pub region: OpsRegion,
    /// Table state.
    pub table: DataTableState<RowId, ColId>,
    /// Log state.
    pub log: LogStreamState,
    /// Inspector optional.
    pub inspector: ObjectInspectorState,
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> Default for OpsDashboardState<RowId, ColId> {
    fn default() -> Self {
        Self::new()
    }
}

impl<RowId: Clone + Ord, ColId: Clone + PartialEq> OpsDashboardState<RowId, ColId> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            region: OpsRegion::Main,
            table: DataTableState::new(),
            log: LogStreamState::new(),
            inspector: ObjectInspectorState::new(),
        }
    }

    /// Tab cycles regions; region keys route to child.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        visible_rows: &[RowId],
        columns: &ColumnModel<ColId>,
        logs: &[LogLine<'_>],
    ) -> OpsDashboardOutcome<RowId, ColId> {
        if !key.is_press() {
            return OpsDashboardOutcome::Ignored;
        }
        if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
            // Tab visits regions that own interaction. The metrics strip and
            // the status bar are read-only, so stopping there was a focus
            // ring that did nothing (plans/016 Step 2).
            self.region = match self.region {
                OpsRegion::Main => OpsRegion::Log,
                OpsRegion::Metrics | OpsRegion::Log | OpsRegion::Status => OpsRegion::Main,
            };
            return OpsDashboardOutcome::FocusRegion(self.region);
        }
        if key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return OpsDashboardOutcome::RetryRequested;
        }
        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return OpsDashboardOutcome::TimeRangeRequested;
        }
        match self.region {
            OpsRegion::Main => {
                OpsDashboardOutcome::Table(self.table.handle_key(key, visible_rows, columns))
            }
            // Log pane owns scroll/follow/cursor over the same lines the host paints.
            OpsRegion::Log => OpsDashboardOutcome::Log(self.log.handle_key(key, logs)),
            _ => OpsDashboardOutcome::Ignored,
        }
    }
}

// ── Reference paint ─────────────────────────────────────────────────────────

/// Host-owned content for one ops dashboard frame.
///
/// Everything here belongs to the host: what the metrics are, which rows the
/// table shows, what the log holds. The recipe owns only the assembly.
#[derive(Debug, Clone, Copy)]
pub struct OpsDashboardView<'a, RowId, ColId> {
    /// Metric tiles for the top strip.
    pub tiles: &'a [MetricTile<'a>],
    /// Table columns.
    pub columns: &'a ColumnModel<ColId>,
    /// Visible projected rows.
    pub rows: &'a [(RowId, &'a [&'a str])],
    /// Log lines for the stream pane.
    pub logs: &'a [LogLine<'a>],
    /// Footer hints (`tab pane`, `^r retry`, …).
    pub hints: &'a [StatusSlot<'a, &'a str>],
}

/// Paints a reference ops dashboard over [`layout_ops_dashboard`]'s slots.
///
/// This is the example: a host that wants a different assembly copies it and
/// changes the widgets, and a host that only wants the geometry keeps calling
/// [`layout_ops_dashboard`] and paints its own panes.
pub fn paint_ops_dashboard<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    area: Rect,
    buffer: &mut Buffer,
    system: &DesignSystem,
    config: OpsDashboardLayout,
    view: OpsDashboardView<'_, RowId, ColId>,
    state: &mut OpsDashboardState<RowId, ColId>,
) -> OpsDashboardSlots {
    let slots = layout_ops_dashboard(area, config);

    if slots.metrics.height > 0 && !view.tiles.is_empty() {
        let width = slots.metrics.width / u16::try_from(view.tiles.len()).unwrap_or(1).max(1);
        for (i, tile) in view.tiles.iter().enumerate() {
            let x = slots
                .metrics
                .x
                .saturating_add(width.saturating_mul(u16::try_from(i).unwrap_or(0)));
            let rect = Rect::new(x, slots.metrics.y, width, slots.metrics.height);
            if rect.right() > slots.metrics.right() || rect.width == 0 {
                break;
            }
            tile.view(system)
                .presentation(if slots.metrics.height > 2 {
                    MetricTilePresentation::Card
                } else {
                    MetricTilePresentation::Row
                })
                .paint(rect, buffer);
        }
    }

    if slots.main.height > 0 {
        DataTable::new(system, view.columns, view.rows)
            .focused(matches!(state.region, OpsRegion::Main))
            .render(slots.main, buffer, &mut state.table);
    }

    if slots.log.height > 0 {
        LogStream::new(view.logs, system)
            .focused(matches!(state.region, OpsRegion::Log))
            .render(slots.log, buffer, &mut state.log);
    }

    if slots.status.height > 0 {
        let mut status = StatusBarState::new();
        StatusBar::new(view.hints, &[], system).render(slots.status, buffer, &mut status);
    }

    slots
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use crate::input::{KeyCode, KeyEvent, KeyModifiers};
    use crate::widgets::{ColumnModel, DataColumn, DataColumnWidth};

    #[test]
    fn ops_tab_cycles_region() {
        let mut state = OpsDashboardState::<u64, &str>::new();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(4))]);
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &rows,
            &cols,
            &[],
        );
        assert!(matches!(
            out,
            OpsDashboardOutcome::FocusRegion(OpsRegion::Log)
        ));
    }
}
