//! Source-owned OpsDashboard block (Plan 053).
//!
//! Owns chrome focus routing only. Domain metrics/logs/rows are projected by the app.
//! Depends on the `termrock` kernel crate — not copied into this file.

use termrock::input::KeyEvent;
use termrock::patterns::{OpsDashboardOutcome, OpsDashboardState};
use termrock::widgets::{ColumnModel, LogLine};

/// Layout hint constants (consumer maps to `layout_ops_dashboard` or local geometry).
pub mod slots {
    /// Metrics strip preference.
    pub const METRICS_H: u16 = 3;
    /// Log pane preference.
    pub const LOG_H: u16 = 8;
}

/// Drive the block: Tab cycles regions; table/log keys when focused.
pub fn handle_key<RowId: Clone + Ord, ColId: Clone + PartialEq>(
    state: &mut OpsDashboardState<RowId, ColId>,
    key: KeyEvent,
    visible_rows: &[RowId],
    columns: &ColumnModel<ColId>,
    logs: &[LogLine<'_>],
) -> OpsDashboardOutcome<RowId, ColId> {
    state.handle_key(key, visible_rows, columns, logs)
}
