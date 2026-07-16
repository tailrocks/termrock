//! Integration coverage for visible-window Table rendering.

use std::{
    alloc::System,
    hint::black_box,
    num::NonZeroU16,
    time::{Duration, Instant},
};

use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::{
    Theme,
    widgets::{CellAlignment, Column, ColumnWidth, Table, TableRow, TableState},
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn warmed_large_table_paints_only_the_viewport_without_allocating() {
    const ROW_COUNT: usize = 10_000;
    const HEIGHT: u16 = 40;
    const SAMPLES: usize = 100;
    let columns = [
        Column {
            id: 0,
            title: Line::from("ID"),
            width: ColumnWidth::Fixed(8),
            alignment: CellAlignment::Right,
            sortable: true,
            sort: None,
        },
        Column {
            id: 1,
            title: Line::from("Name"),
            width: ColumnWidth::Fill(NonZeroU16::new(2).unwrap()),
            alignment: CellAlignment::Left,
            sortable: true,
            sort: None,
        },
        Column {
            id: 2,
            title: Line::from("State"),
            width: ColumnWidth::Fill(NonZeroU16::new(1).unwrap()),
            alignment: CellAlignment::Center,
            sortable: false,
            sort: None,
        },
    ];
    let cells = (0..ROW_COUNT)
        .map(|_| {
            [
                Line::from("42"),
                Line::from("resident process"),
                Line::from("ready"),
            ]
        })
        .collect::<Vec<_>>();
    let rows = cells
        .iter()
        .enumerate()
        .map(|(id, cells)| TableRow {
            id,
            cells,
            enabled: true,
            emphasis: false,
            style: None,
        })
        .collect::<Vec<_>>();
    let theme = Theme::default();
    let table = Table::new(&columns, &rows, &theme);
    let area = Rect::new(0, 0, 100, HEIGHT);
    let mut buffer = Buffer::empty(area);
    let mut state = TableState::new(Some(ROW_COUNT - 1));
    state.reconcile(&rows);
    table.render(area, &mut buffer, &mut state);
    assert_eq!(state.row_regions.len(), usize::from(HEIGHT - 1));

    let allocations = Region::new(GLOBAL);
    let started = Instant::now();
    for _ in 0..SAMPLES {
        table.render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let elapsed = started.elapsed();
    let change = allocations.change();
    assert_eq!(
        change.allocations, 0,
        "warmed renders allocated: {change:?}"
    );
    assert_eq!(
        change.reallocations, 0,
        "warmed renders reallocated: {change:?}"
    );
    assert_eq!(state.row_regions.len(), usize::from(HEIGHT - 1));
    assert!(
        elapsed <= Duration::from_millis(250),
        "table hot path exceeded budget: {elapsed:?}"
    );
}
