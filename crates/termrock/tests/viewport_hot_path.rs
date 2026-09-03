//! Integration coverage for the viewport rendering hot path.

use std::{alloc::System, hint::black_box};

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    text::Line,
};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use termrock::{
    input::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind},
    interaction::Outcome,
    style::{DesignSystem, Role, RolePalette},
    widgets::{Viewport, ViewportState},
};

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
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let viewport = Viewport::new(&lines, &system).content_revision(1);
    let area = Rect::new(0, 0, 120, VIEWPORT_HEIGHT);
    let mut buffer = Buffer::empty(area);
    let mut state = ViewportState::default();
    state.scroll.scroll_y = 5_000;

    viewport.paint(area, &mut buffer, &mut state);

    let allocations = Region::new(GLOBAL);
    for _ in 0..SAMPLES {
        viewport.paint(area, black_box(&mut buffer), black_box(&mut state));
    }
    let change = allocations.change();

    assert_eq!(state.scroll.scroll_y, 5_000);
    assert!(
        change.allocations < MAX_ALLOCATIONS_PER_RENDER * SAMPLES,
        "viewport allocations must scale with {VISIBLE_ROWS} visible rows, not {LINE_COUNT} lines: {change:?}"
    );
    eprintln!(
        "viewport hot path: {SAMPLES} renders, {LINE_COUNT} lines, {VISIBLE_ROWS} visible, {change:?}"
    );
}

fn viewport_fixture() -> (Viewport<'static>, DesignSystem, ViewportState) {
    let lines = Box::leak(
        vec![
            Line::from("alpha beta"),
            Line::from("second line"),
            Line::from("third line"),
        ]
        .into_boxed_slice(),
    );
    let system = DesignSystem::default();
    let mut state = ViewportState::default();
    let viewport = Viewport::new(lines, Box::leak(Box::new(system.clone())));
    viewport.paint(
        Rect::new(0, 0, 24, 6),
        &mut Buffer::empty(Rect::new(0, 0, 24, 6)),
        &mut state,
    );
    (viewport, system, state)
}

#[test]
fn mouse_drag_selects_multiline_text_and_paints_popover_selection() {
    let (viewport, system, mut state) = viewport_fixture();
    let area = Rect::new(0, 0, 24, 6);
    let mut buffer = Buffer::empty(area);
    viewport.paint(area, &mut buffer, &mut state);

    assert_eq!(
        viewport.on_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(1, 1),
                modifiers: KeyModifiers::NONE,
            }
        ),
        Outcome::Changed
    );
    assert_eq!(
        viewport.on_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Drag(MouseButton::Left),
                position: Position::new(7, 2),
                modifiers: KeyModifiers::NONE,
            }
        ),
        Outcome::Changed
    );
    assert_eq!(
        viewport.selected_text(&state).as_deref(),
        Some("alpha beta\nsecond")
    );

    viewport.paint(area, &mut buffer, &mut state);
    let selection_bg = system
        .style(Role::Selection)
        .bg
        .expect("selection background");
    assert_eq!(buffer[(1, 1)].bg, selection_bg);
    assert_eq!(buffer[(6, 2)].bg, selection_bg);
}

#[test]
fn double_click_word_copy_and_escape_are_typed_outcomes() {
    let (viewport, _system, mut state) = viewport_fixture();
    let area = Rect::new(0, 0, 24, 6);
    viewport.paint(area, &mut Buffer::empty(area), &mut state);

    assert_eq!(
        viewport.select_word_at(&mut state, Position::new(7, 1)),
        Outcome::Changed
    );
    assert_eq!(viewport.selected_text(&state).as_deref(), Some("beta"));
    assert_eq!(viewport.copy_selection(&state).as_deref(), Some("beta"));

    let (outcome, event) = viewport.on_key(
        &mut state,
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
    );
    assert_eq!(outcome, Outcome::Changed);
    assert!(event.is_some(), "y emits the host clipboard event");

    let (outcome, event) =
        viewport.on_key(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(outcome, Outcome::Changed);
    assert!(event.is_some(), "Esc emits the selection-changed event");
    assert!(!viewport.has_selection(&state));
}

#[test]
fn drag_auto_scroll_is_applied_to_dialog_scroll_on_render() {
    let lines = Box::leak(
        (0..12)
            .map(|index| Line::from(format!("line {index}")))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let system = DesignSystem::default();
    let viewport = Viewport::new(lines, Box::leak(Box::new(system)));
    let area = Rect::new(0, 0, 24, 5);
    let mut state = ViewportState::default();
    viewport.paint(area, &mut Buffer::empty(area), &mut state);
    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(1, 8));
    viewport.paint(area, &mut Buffer::empty(area), &mut state);
    assert_eq!(state.scroll.scroll_y, 1);
}

#[test]
fn selection_state_survives_repaint_and_rejects_outside_pointer_events() {
    let lines =
        Box::leak(vec![Line::from("alpha beta"), Line::from("second line")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let mut state = ViewportState::default();
    let area = Rect::new(0, 0, 24, 6);

    viewport.paint(area, &mut Buffer::empty(area), &mut state);
    assert_eq!(
        viewport.on_mouse(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                position: Position::new(0, 0),
                modifiers: KeyModifiers::NONE,
            },
        ),
        Outcome::Ignored
    );
    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(7, 2));
    assert_eq!(
        viewport.selected_text(&state).as_deref(),
        Some("alpha beta\nsecond")
    );

    let fresh_viewport = Viewport::new(lines, system);
    fresh_viewport.paint(area, &mut Buffer::empty(area), &mut state);

    assert_eq!(
        fresh_viewport.selected_text(&state).as_deref(),
        Some("alpha beta\nsecond")
    );
}

#[test]
fn scrollbar_pointer_changes_persistent_scroll_state() {
    let lines = Box::leak(
        (0..20)
            .map(|index| Line::from(format!("line {index}")))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let mut state = ViewportState::default();
    let area = Rect::new(0, 0, 24, 5);
    viewport.paint(area, &mut Buffer::empty(area), &mut state);
    // Surface leaves the trailing border column as the scrollbar gutter.
    let scrollbar_x = area.right().saturating_sub(1);
    let before = state.scroll.scroll_y;

    let outcome = viewport.on_mouse(
        &mut state,
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            position: Position::new(scrollbar_x, area.bottom().saturating_sub(2)),
            modifiers: KeyModifiers::NONE,
        },
    );

    assert_eq!(outcome, Outcome::Changed);
    assert!(state.scroll.scroll_y > before);
}

#[test]
fn selectable_cells_preserve_tabs_wide_graphemes_and_trailing_spaces() {
    let lines = Box::leak(vec![Line::from("a\t界  ")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let mut state = ViewportState::default();
    let area = Rect::new(0, 0, 20, 4);
    viewport.paint(area, &mut Buffer::empty(area), &mut state);

    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(10, 1));
    assert_eq!(viewport.selected_text(&state).as_deref(), Some("a    界  "));

    viewport.on_click(&mut state, Position::new(6, 1));
    viewport.on_drag(&mut state, Position::new(7, 1));
    assert_eq!(viewport.selected_text(&state).as_deref(), Some("界"));

    viewport.on_click(&mut state, Position::new(1, 1));
    let (outcome, event) = viewport.on_key(
        &mut state,
        KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
    );
    assert_eq!(outcome, Outcome::Ignored);
    assert!(event.is_none());
}
