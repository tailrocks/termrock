// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Source-ownable application block state machines (Plan 053).
//!
//! Blocks compose public TermRock APIs only. Domain data, I/O, and effects stay
//! consumer-owned and surface as typed outcomes.

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    style::DesignSystem,
    widgets::{
        data_table::{DataTableOutcome, DataTableState},
        sidebar::{SidebarOutcome, SidebarState},
        object_inspector::ObjectInspectorState,
        log_stream::{LogStreamOutcome, LogStreamState},
        scroll_area::ScrollAreaState,
    },
};

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
        columns: &crate::widgets::data_view::ColumnModel<ColId>,
    ) -> OpsDashboardOutcome<RowId, ColId> {
        if key.kind != KeyEventKind::Press {
            return OpsDashboardOutcome::Ignored;
        }
        if key.code == KeyCode::Tab && !key.modifiers.contains(KeyModifiers::SHIFT) {
            self.region = match self.region {
                OpsRegion::Metrics => OpsRegion::Main,
                OpsRegion::Main => OpsRegion::Log,
                OpsRegion::Log => OpsRegion::Status,
                OpsRegion::Status => OpsRegion::Metrics,
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
            // Scroll/follow without a projected window (host may re-route with lines).
            OpsRegion::Log => OpsDashboardOutcome::Log(self.log.handle_key_scroll(key)),
            _ => OpsDashboardOutcome::Ignored,
        }
    }
}

// ── ResourceBrowser ─────────────────────────────────────────────────────────

/// Resource browser outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResourceBrowserOutcome<Id> {
    /// No change.
    Ignored,
    /// Sidebar selection.
    Sidebar(SidebarOutcome<Id>),
    /// Request load of selection (consumer).
    LoadRequested(Id),
    /// Open preview.
    PreviewRequested(Id),
}

/// Resource browser state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceBrowserState<Id: Clone + PartialEq> {
    /// Sidebar.
    pub sidebar: SidebarState<Id>,
    /// List scroll.
    pub list_scroll: ScrollAreaState,
    /// Generation for stale preview guard.
    pub selection_generation: u64,
}

impl<Id: Clone + PartialEq> ResourceBrowserState<Id> {
    /// Fresh.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidebar: SidebarState::new(None),
            list_scroll: ScrollAreaState::new(),
            selection_generation: 0,
        }
    }

    /// Keys.
    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        items: &[crate::widgets::SidebarItem<Id>],
    ) -> ResourceBrowserOutcome<Id> {
        let out = self.sidebar.handle_key(key, items);
        match out {
            SidebarOutcome::Selected(id) => {
                self.selection_generation = self.selection_generation.saturating_add(1);
                ResourceBrowserOutcome::LoadRequested(id)
            }
            other => ResourceBrowserOutcome::Sidebar(other),
        }
    }
}

impl<Id: Clone + PartialEq> Default for ResourceBrowserState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

// SettingsShell elevated to `patterns::settings_screen` (migration 0237).

/// Marker type for block chrome that needs tokens (paint lives in consumer/story).
#[derive(Debug, Clone, Copy)]
pub struct BlockChrome<'a> {
    /// Design tokens.
    pub tokens: &'a DesignSystem,
}

impl<'a> BlockChrome<'a> {
    /// Tokens.
    #[must_use]
    pub const fn new(tokens: &'a DesignSystem) -> Self {
        Self { tokens }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_view::{ColumnModel, DataColumn, DataColumnWidth};
    use crate::widgets::SidebarItem;

    #[test]
    fn ops_tab_cycles_region() {
        let mut state = OpsDashboardState::<u64, &str>::new();
        let cols = ColumnModel::new(vec![DataColumn::new("c", "C", DataColumnWidth::Min(4))]);
        let rows = [1u64];
        let out = state.handle_key(
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            &rows,
            &cols,
        );
        assert!(matches!(
            out,
            OpsDashboardOutcome::FocusRegion(OpsRegion::Log)
        ));
    }

    #[test]
    fn resource_load_on_select() {
        let mut state = ResourceBrowserState::new();
        let items = [SidebarItem::new("a", "A")];
        let out = state.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &items);
        assert!(matches!(out, ResourceBrowserOutcome::LoadRequested("a")));
        assert_eq!(state.selection_generation, 1);
    }

}
