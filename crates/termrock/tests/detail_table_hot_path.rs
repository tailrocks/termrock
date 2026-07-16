//! Integration coverage for the detail-table rendering hot path.

use std::{alloc::System, hint::black_box, time::Duration, time::Instant};

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::{
    Theme,
    widgets::{DetailCapability, DetailRow, DetailTable, DetailTableState},
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn unchanged_large_table_reuses_measurement_and_paints_visible_rows() {
    const ROW_COUNT: usize = 10_000;
    const SAMPLES: usize = 100;
    const MAX_ALLOCATIONS_PER_RENDER: usize = 100;

    let rows = (0..ROW_COUNT)
        .map(|id| DetailRow {
            id,
            label: "resident label",
            value: "resident value",
            href: None,
            capability: DetailCapability::Copy,
            emphasis: false,
            style: None,
        })
        .collect::<Vec<_>>();
    let theme = Theme::default();
    let table = DetailTable::new(&rows, &theme).content_revision(1);
    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    let mut state = DetailTableState::default();
    state.selected = Some(5_000);

    table.render(area, &mut buffer, &mut state);
    assert_eq!(state.regions.len(), usize::from(area.height));

    let allocations = Region::new(GLOBAL);
    let started = Instant::now();
    for _ in 0..SAMPLES {
        table.render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let elapsed = started.elapsed();
    let change = allocations.change();

    assert!(
        change.allocations < MAX_ALLOCATIONS_PER_RENDER * SAMPLES,
        "detail-table allocations must scale with the visible window: {change:?}"
    );
    assert!(
        elapsed <= Duration::from_millis(250),
        "100 cached renders exceeded the 250 ms debug-profile budget: {elapsed:?}"
    );
    eprintln!(
        "detail-table hot path: {SAMPLES} renders, {ROW_COUNT} rows, {} visible, {elapsed:?}, {change:?}",
        area.height
    );
}
