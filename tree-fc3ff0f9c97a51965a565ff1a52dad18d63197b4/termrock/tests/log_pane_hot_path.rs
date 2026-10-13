//! Integration coverage for the log-pane rendering hot path.

use std::{
    alloc::System,
    hint::black_box,
    sync::Mutex,
    time::{Duration, Instant},
};

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::{
    Theme,
    widgets::{LogPane, LogPaneState},
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;
static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn warmed_tail_render_allocations_scale_with_visible_rows() {
    let _guard = TEST_LOCK.lock().expect("hot-path test lock poisoned");
    const LINE_COUNT: usize = 10_000;
    const VIEWPORT_HEIGHT: u16 = 42;
    const VISIBLE_ROWS: usize = 40;
    const SAMPLES: usize = 100;
    // TIGHTENED after plan 016: observed baseline is 46 allocations/render;
    // 64 leaves Ratatui headroom while rejecting full-history work.
    const MAX_ALLOCATIONS_PER_RENDER: usize = 64;

    let mut state = LogPaneState::new();
    for _ in 0..LINE_COUNT {
        state.append("resident log line");
    }
    let theme = Theme::default();
    let pane = LogPane::new(&theme).title("Build log");
    let area = Rect::new(0, 0, 120, VIEWPORT_HEIGHT);
    let mut buffer = Buffer::empty(area);

    pane.render(area, &mut buffer, &mut state);
    let allocations = Region::new(GLOBAL);
    for _ in 0..SAMPLES {
        pane.render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let change = allocations.change();

    assert!(state.is_following());
    assert_eq!(state.len(), LINE_COUNT);
    assert!(
        change.allocations < MAX_ALLOCATIONS_PER_RENDER * SAMPLES,
        "log-pane allocations must scale with {VISIBLE_ROWS} visible rows, not {LINE_COUNT} buffered lines: {change:?}"
    );
    eprintln!(
        "log-pane hot path: {SAMPLES} renders, {LINE_COUNT} lines, {VISIBLE_ROWS} visible, {change:?}"
    );
}

#[test]
fn sustained_bounded_ingestion_is_amortized() {
    let _guard = TEST_LOCK.lock().expect("hot-path test lock poisoned");
    const INITIAL_LINES: usize = 10_000;
    const APPENDS: usize = 10_000;

    let mut state = LogPaneState::new();
    for _ in 0..INITIAL_LINES {
        state.append("resident log line");
    }

    let allocations = Region::new(GLOBAL);
    let started = Instant::now();
    for _ in 0..APPENDS {
        state.append(black_box("streamed log line"));
    }
    let elapsed = started.elapsed();
    let change = allocations.change();

    assert_eq!(state.len(), INITIAL_LINES);
    assert!(
        change.allocations < APPENDS * 3,
        "bounded ingestion allocated excessively: {change:?}"
    );
    assert!(
        elapsed < Duration::from_secs(1),
        "bounded ingestion must not shift the full history per append: {elapsed:?}"
    );
    eprintln!(
        "log-pane ingest hot path: {APPENDS} appends into {INITIAL_LINES}-line history, {elapsed:?}, {change:?}"
    );
}
