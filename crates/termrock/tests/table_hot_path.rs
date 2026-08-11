//! Integration coverage for visible-window Table rendering.

use std::{alloc::System, hint::black_box, num::NonZeroU16, time::Instant};

use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::perf::{check_batch_budget, check_zero_alloc_steady};
use termrock::style::DesignSystem;
use termrock::widgets::{CellAlignment, Column, ColumnWidth, Table, TableRow, TableState};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn warmed_large_table_paints_only_the_viewport_without_allocating() {
    let tokens = DesignSystem::default();
    const ROW_COUNT: usize = 10_000;
    const HEIGHT: u16 = 40;
    const SAMPLES: usize = 100;
    let columns = [
        Column::new(0, "ID", ColumnWidth::Fixed(8))
            .alignment(CellAlignment::Right)
            .sortable(None),
        Column::new(1, "Name", ColumnWidth::Fill(NonZeroU16::new(2).unwrap())).sortable(None),
        Column::new(2, "State", ColumnWidth::Fill(NonZeroU16::new(1).unwrap()))
            .alignment(CellAlignment::Center),
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
        .map(|(id, cells)| TableRow::new(id, cells))
        .collect::<Vec<_>>();
    let table = Table::new(&columns, &rows, &tokens);
    let area = Rect::new(0, 0, 100, HEIGHT);
    let mut buffer = Buffer::empty(area);
    let mut state = TableState::new(Some(ROW_COUNT - 1));
    state.reconcile(&rows);
    (&table).render(area, &mut buffer, &mut state);
    assert_eq!(state.row_regions.len(), usize::from(HEIGHT - 1));

    let allocations = Region::new(GLOBAL);
    let started = Instant::now();
    for _ in 0..SAMPLES {
        (&table).render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let elapsed = started.elapsed();
    let change = allocations.change();
    check_zero_alloc_steady(
        "table_viewport_10k_alloc",
        change.allocations,
        change.reallocations,
    )
    .unwrap_or_else(|e| panic!("{e}; stats={change:?}"));
    assert_eq!(state.row_regions.len(), usize::from(HEIGHT - 1));
    check_batch_budget("table_viewport_10k", SAMPLES as u32, elapsed)
        .unwrap_or_else(|e| panic!("{e}"));
}
