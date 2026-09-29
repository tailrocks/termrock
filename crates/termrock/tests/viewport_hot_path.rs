//! Integration coverage for the viewport rendering hot path.

use std::{alloc::System, hint::black_box};

use ratatui_core::{
    buffer::Buffer,
    layout::{Alignment, Position, Rect},
    style::{Color, Style},
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

    viewport.render(area, &mut buffer, &mut state);

    let allocations = Region::new(GLOBAL);
    for _ in 0..SAMPLES {
        viewport.render(area, black_box(&mut buffer), black_box(&mut state));
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
    viewport.render(
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
    viewport.render(area, &mut buffer, &mut state);

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

    viewport.render(area, &mut buffer, &mut state);
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
    viewport.render(area, &mut Buffer::empty(area), &mut state);

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
    viewport.render(area, &mut Buffer::empty(area), &mut state);
    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(1, 8));
    viewport.render(area, &mut Buffer::empty(area), &mut state);
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

    viewport.render(area, &mut Buffer::empty(area), &mut state);
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
    fresh_viewport.render(area, &mut Buffer::empty(area), &mut state);

    assert_eq!(
        fresh_viewport.selected_text(&state).as_deref(),
        Some("alpha beta\nsecond")
    );
}

#[test]
fn horizontal_scroll_does_not_paint_tail_on_offscreen_short_line() {
    let lines = Box::leak(
        vec![
            Line::from("0123456789abcdef"),
            Line::from("x"),
            Line::from("tail"),
        ]
        .into_boxed_slice(),
    );
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let area = Rect::new(0, 0, 12, 5);
    let mut state = ViewportState::default();
    viewport.render(area, &mut Buffer::empty(area), &mut state);
    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(1, 3));
    assert_eq!(
        viewport.selected_text(&state).as_deref(),
        Some("0123456789abcdef\nx\n")
    );

    state.scroll.scroll_x = 4;
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer, &mut state);

    let selection_bg = system
        .style(Role::Selection)
        .bg
        .expect("selection background");
    assert_ne!(buffer[(1, 2)].bg, selection_bg);
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
    viewport.render(area, &mut Buffer::empty(area), &mut state);
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
    viewport.render(area, &mut Buffer::empty(area), &mut state);

    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(10, 1));
    assert_eq!(viewport.selected_text(&state).as_deref(), Some("a   界  "));

    viewport.on_click(&mut state, Position::new(5, 1));
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

#[test]
fn tabs_expand_to_four_column_stops() {
    let lines = Box::leak(
        vec![
            Line::from("a\tb"),
            Line::from("ab\tc"),
            Line::from("abcde\tf"),
        ]
        .into_boxed_slice(),
    );
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let mut state = ViewportState::default();
    let area = Rect::new(0, 0, 32, 6);
    viewport.render(area, &mut Buffer::empty(area), &mut state);

    for (row, (end_x, expected)) in [(6, "a   b"), (6, "ab  c"), (10, "abcde   f")]
        .into_iter()
        .enumerate()
    {
        viewport.on_click(&mut state, Position::new(1, row as u16 + 1));
        viewport.on_drag(&mut state, Position::new(end_x, row as u16 + 1));
        assert_eq!(viewport.selected_text(&state).as_deref(), Some(expected));
    }
}

#[test]
fn aligned_lines_keep_their_paragraph_positions() {
    let lines = Box::leak(
        vec![
            Line::from("left"),
            Line::from("odd").alignment(Alignment::Center),
            Line::from("right").alignment(Alignment::Right),
        ]
        .into_boxed_slice(),
    );
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let area = Rect::new(0, 0, 12, 6);
    let mut buffer = Buffer::empty(area);
    let mut state = ViewportState::default();
    viewport.render(area, &mut buffer, &mut state);

    // The body is ten cells wide: x=1..11. Ratatui's center offset is
    // (10 - 3) / 2 = 3, so the three-cell line starts at x=4.
    assert_eq!(buffer[(1, 1)].symbol(), "l");
    assert_eq!(buffer[(4, 2)].symbol(), "o");
    assert_eq!(buffer[(6, 3)].symbol(), "r");
    assert_eq!(
        viewport
            .pos_at(&state, Position::new(4, 2))
            .map(|p| (p.line, p.col)),
        Some((1, 0))
    );
    assert_eq!(
        viewport
            .pos_at(&state, Position::new(6, 3))
            .map(|p| (p.line, p.col)),
        Some((2, 0))
    );
    assert!(viewport.pos_at(&state, Position::new(3, 2)).is_none());
    assert!(viewport.pos_at(&state, Position::new(7, 2)).is_none());
    assert!(viewport.pos_at(&state, Position::new(5, 3)).is_none());
}

#[test]
fn overflowing_centered_and_right_aligned_lines_use_horizontal_scroll() {
    let lines = Box::leak(
        vec![
            Line::from("abcdefghij").alignment(Alignment::Center),
            Line::from("klmnopqrst").alignment(Alignment::Right),
        ]
        .into_boxed_slice(),
    );
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let mut state = ViewportState::default();
    state.scroll.scroll_x = 2;
    let area = Rect::new(0, 0, 8, 5);
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer, &mut state);

    assert_eq!(buffer[(1, 1)].symbol(), "c");
    assert_eq!(buffer[(1, 2)].symbol(), "m");
    assert_eq!(
        viewport
            .pos_at(&state, Position::new(1, 1))
            .map(|p| (p.line, p.col)),
        Some((0, 2))
    );
    assert_eq!(
        viewport
            .pos_at(&state, Position::new(1, 2))
            .map(|p| (p.line, p.col)),
        Some((1, 2))
    );
}

#[test]
fn horizontal_scroll_does_not_draw_or_hit_test_a_partial_wide_grapheme() {
    let lines = Box::leak(vec![Line::from("界abc")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let mut state = ViewportState::default();
    state.scroll.scroll_x = 1;
    let area = Rect::new(0, 0, 6, 4);
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer, &mut state);

    assert_eq!(buffer[(1, 1)].symbol(), " ");
    assert_eq!(buffer[(2, 1)].symbol(), "a");
    assert_eq!(viewport.pos_at(&state, Position::new(1, 1)), None);
}

#[test]
fn render_clips_partial_nonzero_origin_buffers() {
    let lines = Box::leak(vec![Line::from("visible")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let buffer_area = Rect::new(5, 5, 8, 4);
    let mut buffer = Buffer::empty(buffer_area);

    viewport.render(
        Rect::new(2, 2, 20, 20),
        &mut buffer,
        &mut ViewportState::default(),
    );
}

#[test]
fn explicit_content_revision_bump_invalidates_stale_selection_coordinates() {
    let mut lines = vec![Line::from("abcdef")];
    let system = Box::leak(Box::new(DesignSystem::default()));
    let area = Rect::new(0, 0, 16, 4);
    let mut state = ViewportState::default();
    {
        let viewport = Viewport::new(&lines, system).content_revision(1);
        viewport.render(area, &mut Buffer::empty(area), &mut state);
        viewport.on_click(&mut state, Position::new(1, 1));
        viewport.on_drag(&mut state, Position::new(7, 1));
        assert_eq!(viewport.selected_text(&state).as_deref(), Some("abcdef"));
    }

    lines[0] = Line::from("uvwxyz");
    let viewport = Viewport::new(&lines, system).content_revision(2);
    viewport.render(area, &mut Buffer::empty(area), &mut state);

    assert!(!viewport.has_selection(&state));
    assert!(!viewport.has_anchor(&state));
}

#[test]
fn style_only_revision_preserves_selection_and_copy() {
    let mut lines = vec![Line::from("abcdef")];
    let system = Box::leak(Box::new(DesignSystem::default()));
    let area = Rect::new(0, 0, 16, 4);
    let mut state = ViewportState::default();
    let viewport = Viewport::new(&lines, system).content_revision(1);
    viewport.render(area, &mut Buffer::empty(area), &mut state);
    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(7, 1));
    assert_eq!(viewport.copy_selection(&state).as_deref(), Some("abcdef"));

    lines[0] = Line::from("abcdef").style(Style::default().fg(Color::Red));
    let styled = Viewport::new(&lines, system).content_revision(2);
    styled.render(area, &mut Buffer::empty(area), &mut state);

    assert_eq!(styled.copy_selection(&state).as_deref(), Some("abcdef"));
    assert!(styled.has_selection(&state));
}

#[test]
fn drag_after_empty_resize_clears_anchor_without_panicking() {
    let lines = Box::leak(vec![Line::from("abcdef")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let area = Rect::new(0, 0, 16, 4);
    let mut state = ViewportState::default();
    viewport.render(area, &mut Buffer::empty(area), &mut state);
    viewport.on_click(&mut state, Position::new(1, 1));
    assert!(viewport.has_anchor(&state));

    let empty = Rect::new(8, 8, 0, 0);
    viewport.render(empty, &mut Buffer::empty(empty), &mut state);
    assert!(!viewport.has_anchor(&state));
    assert_eq!(
        viewport.on_drag(&mut state, Position::new(8, 8)),
        Outcome::Ignored
    );
}

#[test]
fn uncached_revision_rebuilds_reused_storage_for_both_width_changes() {
    let mut lines = vec![Line::from("x")];
    let system = Box::leak(Box::new(DesignSystem::default()));
    let area = Rect::new(0, 0, 16, 4);
    let mut state = ViewportState::default();
    let viewport = Viewport::new(&lines, system);
    let mut first = Buffer::empty(area);
    viewport.render(area, &mut first, &mut state);
    assert_eq!(first[(1, 1)].symbol(), "x");
    assert_eq!(
        viewport.select_word_at(&mut state, Position::new(1, 1)),
        Outcome::Changed
    );
    assert_eq!(viewport.copy_selection(&state).as_deref(), Some("x"));

    let source = lines.as_ptr();
    lines[0] = Line::from("界");
    assert_eq!(
        lines.as_ptr(),
        source,
        "the Vec storage is intentionally reused"
    );

    let changed = Viewport::new(&lines, system);
    let mut fresh = Buffer::empty(area);
    changed.render(area, &mut fresh, &mut state);
    assert_eq!(fresh[(1, 1)].symbol(), "界");
    assert!(!changed.has_selection(&state));
    assert_eq!(
        changed.pos_at(&state, Position::new(2, 1)),
        Some(termrock::widgets::CellPos { line: 0, col: 2 })
    );
    assert_eq!(
        changed.select_word_at(&mut state, Position::new(2, 1)),
        Outcome::Changed
    );
    assert_eq!(changed.copy_selection(&state).as_deref(), Some("界"));

    lines[0] = Line::from("x");
    let narrowed = Viewport::new(&lines, system);
    narrowed.render(area, &mut Buffer::empty(area), &mut state);
    assert!(!narrowed.has_selection(&state));
    assert_eq!(
        narrowed.pos_at(&state, Position::new(1, 1)),
        Some(termrock::widgets::CellPos { line: 0, col: 0 })
    );
    assert_eq!(
        narrowed.select_word_at(&mut state, Position::new(1, 1)),
        Outcome::Changed
    );
    assert_eq!(narrowed.copy_selection(&state).as_deref(), Some("x"));
}

#[test]
fn halfwidth_sound_marks_use_ratatui_cell_width() {
    let lines = Box::leak(vec![Line::from("ﾞ"), Line::from("ｶﾞ")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let area = Rect::new(0, 0, 16, 4);
    let mut state = ViewportState::default();
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer, &mut state);

    assert_eq!(buffer[(1, 1)].symbol(), "ﾞ");
    assert_eq!(buffer[(1, 2)].symbol(), "ｶﾞ");
    assert_eq!(
        viewport.pos_at(&state, Position::new(2, 2)),
        Some(termrock::widgets::CellPos { line: 1, col: 2 })
    );

    viewport.on_click(&mut state, Position::new(1, 1));
    viewport.on_drag(&mut state, Position::new(2, 1));
    assert_eq!(viewport.copy_selection(&state).as_deref(), Some("ﾞ"));

    viewport.on_click(&mut state, Position::new(1, 2));
    viewport.on_drag(&mut state, Position::new(3, 2));
    assert_eq!(viewport.copy_selection(&state).as_deref(), Some("ｶﾞ"));
}

#[test]
fn wide_grapheme_word_selection_works_on_both_terminal_cells() {
    let lines = Box::leak(vec![Line::from("界 ")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let viewport = Viewport::new(lines, system);
    let area = Rect::new(0, 0, 20, 4);
    let mut state = ViewportState::default();
    viewport.render(area, &mut Buffer::empty(area), &mut state);

    assert_eq!(
        viewport.select_word_at(&mut state, Position::new(1, 1)),
        Outcome::Changed
    );
    assert_eq!(viewport.selected_text(&state).as_deref(), Some("界"));
    assert_eq!(
        viewport.select_word_at(&mut state, Position::new(2, 1)),
        Outcome::Changed
    );
    assert_eq!(viewport.selected_text(&state).as_deref(), Some("界"));
}

#[test]
fn tab_stops_and_content_style_cover_blank_cells() {
    let lines = Box::leak(vec![Line::from("a\tb")].into_boxed_slice());
    let system = Box::leak(Box::new(DesignSystem::default()));
    let style = Style::default().bg(Color::Blue);
    let viewport = Viewport::new(lines, system).content_style(style);
    let area = Rect::new(0, 0, 16, 4);
    let mut buffer = Buffer::empty(area);
    viewport.render(area, &mut buffer, &mut ViewportState::default());

    assert_eq!(buffer[(1, 1)].symbol(), "a");
    assert_eq!(buffer[(2, 1)].symbol(), " ");
    assert_eq!(buffer[(3, 1)].symbol(), " ");
    assert_eq!(buffer[(4, 1)].symbol(), " ");
    assert_eq!(buffer[(5, 1)].symbol(), "b");
    assert_eq!(buffer[(6, 1)].bg, Color::Blue);
}
