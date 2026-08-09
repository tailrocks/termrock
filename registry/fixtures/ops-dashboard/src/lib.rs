//! Source-owned OpsDashboard block (Plan 053).
//!
//! Owns chrome focus routing only. Domain metrics/logs/rows are projected by the app.
//! Depends on the `termrock` kernel crate — not copied into this file.

use termrock::input::KeyEvent;
use termrock::style::DesignSystem;
use termrock::widgets::{
    BlockChrome, ColumnModel, OpsDashboardOutcome, OpsDashboardState, OpsRegion,
};

/// Layout hint constants (consumer maps to `layout_ops_dashboard` or local geometry).
pub mod slots {
    /// Metrics strip preference.
    pub const METRICS_H: u16 = 3;
    /// Log pane preference.
    pub const LOG_H: u16 = 8;
}

/// Drive the block: Tab cycles regions; table/log keys when focused.
pub fn handle_key<RowId, ColId>(
    state: &mut OpsDashboardState<RowId, ColId>,
    key: KeyEvent,
    visible_rows: &[RowId],
    columns: &ColumnModel<ColId>,
) -> OpsDashboardOutcome<RowId, ColId>
where
    RowId: Clone + Ord,
    ColId: Clone + PartialEq,
{
    state.handle_key(key, visible_rows, columns)
}

/// Paint-time chrome handle (tokens only).
#[must_use]
pub fn chrome<'a>(system: &'a DesignSystem) -> BlockChrome<'a> {
    BlockChrome::new(system)
}

/// Default region for stories.
#[must_use]
pub fn default_region() -> OpsRegion {
    OpsRegion::Main
}
