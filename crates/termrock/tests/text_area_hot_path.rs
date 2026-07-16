//! Allocation and visible-window proof for TextArea.

use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use std::{
    alloc::System,
    hint::black_box,
    time::{Duration, Instant},
};
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    widgets::{TextArea, TextAreaState},
};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn warmed_large_document_render_is_allocation_free() {
    let text = (0..10_000)
        .map(|_| "resident line with 東京 and emoji 🧪")
        .collect::<Vec<_>>()
        .join("\n");
    let theme = Theme::default();
    let widget = TextArea::new(&theme);
    let mut state = TextAreaState::new(text);
    let area = Rect::new(0, 0, 80, 40);
    let mut buffer = Buffer::empty(area);
    widget.render(area, &mut buffer, &mut state);
    let region = Region::new(GLOBAL);
    let started = Instant::now();
    for _ in 0..100 {
        widget.render(area, black_box(&mut buffer), black_box(&mut state));
    }
    let elapsed = started.elapsed();
    let change = region.change();
    assert_eq!(change.allocations, 0, "warm render allocated: {change:?}");
    assert_eq!(
        change.reallocations, 0,
        "warm render reallocated: {change:?}"
    );
    assert!(
        elapsed < Duration::from_millis(250),
        "batch too slow: {elapsed:?}"
    );
}

#[test]
fn ordinary_inline_insert_reuses_existing_line_capacity() {
    let mut state = TextAreaState::new("");
    state.set_focused(true);
    let reserve = "x".repeat(256);
    let _ = state.insert_text(&reserve);
    for _ in 0..256 {
        let _ = state.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
    let region = Region::new(GLOBAL);
    let _ = black_box(&mut state).insert_text(black_box("ordinary"));
    let change = region.change();
    assert_eq!(
        change.allocations, 0,
        "inline insertion allocated: {change:?}"
    );
    assert_eq!(
        change.reallocations, 0,
        "inline insertion reallocated: {change:?}"
    );
}
