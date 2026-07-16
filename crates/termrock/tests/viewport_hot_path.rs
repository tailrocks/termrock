//! Integration coverage for the viewport rendering hot path.

use std::{alloc::System, hint::black_box};

use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::{Theme, scroll::DialogScroll, widgets::Viewport};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn large_viewport_allocations_scale_with_visible_rows() {
    const LINE_COUNT: usize = 10_000;
    const VIEWPORT_HEIGHT: u16 = 42;
    const VISIBLE_ROWS: usize = 40;
    const SAMPLES: usize = 100;
    const MAX_ALLOCATIONS_PER_RENDER: usize = 200;

    let lines = (0..LINE_COUNT)
        .map(|_| Line::from("resident line"))
        .collect::<Vec<_>>();
    let theme = Theme::default();
    let viewport = Viewport::new(&lines, &theme);
    let area = Rect::new(0, 0, 120, VIEWPORT_HEIGHT);
    let mut buffer = Buffer::empty(area);
    let mut state = DialogScroll {
        scroll_y: 5_000,
        ..DialogScroll::default()
    };

    viewport.render(area, &mut buffer, &mut state);

    let allocations = Region::new(GLOBAL);
    for _ in 0..SAMPLES {
        viewport.render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let change = allocations.change();

    assert_eq!(state.scroll_y, 5_000);
    assert!(
        change.allocations < MAX_ALLOCATIONS_PER_RENDER * SAMPLES,
        "viewport allocations must scale with {VISIBLE_ROWS} visible rows, not {LINE_COUNT} lines: {change:?}"
    );
    eprintln!(
        "viewport hot path: {SAMPLES} renders, {LINE_COUNT} lines, {VISIBLE_ROWS} visible, {change:?}"
    );
}
