//! Integration coverage for unchanged picker-projection reconciliation.

use std::{alloc::System, hint::black_box};

use ratatui_core::text::Line;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::widgets::{ListRow, PickerState, RowRole};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn warmed_owned_projection_reconciliation_is_allocation_free() {
    const ROW_COUNT: usize = 1_000;
    const SAMPLES: usize = 100;

    let rows = (0..ROW_COUNT)
        .map(|index| ListRow {
            id: format!("command-{index}"),
            label: Line::from("resident command"),
            trailing: None,
            role: RowRole::Item,
            enabled: true,
        })
        .collect::<Vec<_>>();
    let mut state = PickerState::new(Some("command-500".to_owned()));
    state.reconcile(&rows);

    let allocations = Region::new(GLOBAL);
    for _ in 0..SAMPLES {
        black_box(&mut state).reconcile(black_box(&rows));
    }
    let change = allocations.change();

    assert_eq!(
        change.allocations, 0,
        "warmed reconcile allocated: {change:?}"
    );
    assert_eq!(
        change.reallocations, 0,
        "warmed reconcile reallocated: {change:?}"
    );
}
