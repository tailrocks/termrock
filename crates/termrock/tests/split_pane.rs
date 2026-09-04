//! Integration coverage for split-pane geometry and interaction.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use termrock::{
    input::{KeyCode, KeyEvent, KeyModifiers},
    style::{DesignSystem, Role, RolePalette},
    widgets::{SplitDirection, SplitPane, SplitPaneOutcome, SplitPaneState, SplitRatio, SplitSide},
};

#[test]
fn ratio_constructors_and_defaults_keep_expanded_bounds() {
    assert_eq!(SplitRatio::from_basis_points(0).basis_points(), 500);
    assert_eq!(SplitRatio::from_basis_points(499).basis_points(), 500);
    assert_eq!(SplitRatio::from_basis_points(500).basis_points(), 500);
    assert_eq!(SplitRatio::from_basis_points(9_500).basis_points(), 9_500);
    assert_eq!(SplitRatio::from_basis_points(9_501).basis_points(), 9_500);
    assert_eq!(
        SplitRatio::from_basis_points(u16::MAX).basis_points(),
        9_500
    );

    assert_eq!(SplitRatio::from_percent(0).basis_points(), 500);
    assert_eq!(SplitRatio::from_percent(4).basis_points(), 500);
    assert_eq!(SplitRatio::from_percent(5).basis_points(), 500);
    assert_eq!(SplitRatio::from_percent(95).basis_points(), 9_500);
    assert_eq!(SplitRatio::from_percent(96).basis_points(), 9_500);
    assert_eq!(SplitRatio::default().basis_points(), 5_000);
    assert_eq!(SplitPaneState::default().ratio().basis_points(), 5_000);
    assert_eq!(
        SplitPaneState::new(SplitRatio::from_basis_points(0))
            .ratio()
            .basis_points(),
        500
    );
}

#[cfg(feature = "serde")]
#[test]
fn serde_deserialization_normalizes_ratio_endpoints_and_out_of_range_values() {
    for (encoded, expected) in [
        ("0", 500),
        ("499", 500),
        ("500", 500),
        ("9500", 9_500),
        ("9501", 9_500),
        ("10000", 9_500),
        ("65535", 9_500),
    ] {
        let ratio: SplitRatio =
            serde_json::from_str(encoded).expect("ratio JSON should deserialize");
        assert_eq!(ratio.basis_points(), expected, "encoded ratio: {encoded}");
    }
}

#[test]
fn horizontal_layout_honors_ratio_and_minimums() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 10, 15, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(40));
    state.set_focused(true);

    let layout = split.layout(Rect::new(2, 3, 51, 8), &mut state);

    assert_eq!(layout.first, Rect::new(2, 3, 20, 8));
    assert_eq!(layout.divider, Rect::new(22, 3, 1, 8));
    assert_eq!(layout.second, Rect::new(23, 3, 30, 8));
    assert_eq!(state.ratio().basis_points(), 4_000);

    state.set_ratio(SplitRatio::from_percent(5));
    assert_eq!(
        split.layout(Rect::new(2, 3, 51, 8), &mut state).first.width,
        10
    );
    state.set_ratio(SplitRatio::from_percent(95));
    assert_eq!(
        split.layout(Rect::new(2, 3, 51, 8), &mut state).first.width,
        35
    );
}

#[test]
fn layout_floors_ratio_allocation_before_the_divider() {
    let system = DesignSystem::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));

    let layout = split.layout(Rect::new(0, 0, 4, 1), &mut state);

    assert_eq!(layout.first.width, 1);
    assert_eq!(layout.divider.width, 1);
    assert_eq!(layout.second.width, 2);
}

#[test]
fn vertical_layout_and_tiny_areas_never_escape_the_input_rectangle() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Vertical, 8, 8, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    state.set_focused(true);

    let regular = split.layout(Rect::new(4, 6, 12, 21), &mut state);
    assert_eq!(regular.first, Rect::new(4, 6, 12, 10));
    assert_eq!(regular.divider, Rect::new(4, 16, 12, 1));
    assert_eq!(regular.second, Rect::new(4, 17, 12, 10));

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let tiny = SplitPane::new(direction, 8, 8, &system);
        for area in [
            Rect::new(0, 0, 0, 0),
            Rect::new(0, 0, 0, 5),
            Rect::new(0, 0, 5, 0),
            Rect::new(7, 9, 1, 1),
            Rect::new(u16::MAX - 1, u16::MAX - 1, 1, 1),
        ] {
            let layout = tiny.layout(area, &mut state);
            assert!(area.contains(layout.first.as_position()) || layout.first.is_empty());
            assert!(area.contains(layout.second.as_position()) || layout.second.is_empty());
            assert!(area.contains(layout.divider.as_position()) || layout.divider.is_empty());
            assert!(layout.first.right() <= area.right());
            assert!(layout.second.right() <= area.right());
            assert!(layout.divider.right() <= area.right());
            assert!(layout.first.bottom() <= area.bottom());
            assert!(layout.second.bottom() <= area.bottom());
            assert!(layout.divider.bottom() <= area.bottom());
        }
    }
}

#[test]
fn impossible_minimums_use_canonical_full_pane_fallbacks() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());

    let horizontal_area = Rect::new(2, 3, 51, 2);
    let horizontal = SplitPane::new(SplitDirection::Horizontal, 90, 10, &system);
    let mut horizontal_state = SplitPaneState::new(SplitRatio::from_percent(5));
    let horizontal_layout = horizontal.layout(horizontal_area, &mut horizontal_state);
    assert_eq!(horizontal_layout.first, Rect::ZERO);
    assert_eq!(horizontal_layout.divider, Rect::ZERO);
    assert_eq!(horizontal_layout.second, horizontal_area);

    let vertical_area = Rect::new(2, 3, 4, 51);
    let vertical = SplitPane::new(SplitDirection::Vertical, 90, 10, &system);
    let mut vertical_state = SplitPaneState::new(SplitRatio::from_percent(95));
    let vertical_layout = vertical.layout(vertical_area, &mut vertical_state);
    assert_eq!(vertical_layout.first, vertical_area);
    assert_eq!(vertical_layout.divider, Rect::ZERO);
    assert_eq!(vertical_layout.second, Rect::ZERO);
}

#[test]
fn zero_usable_axis_with_zero_minima_has_empty_panes_and_no_divider() {
    let system = DesignSystem::default();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 8));

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let split = SplitPane::new(direction, 0, 0, &system);
        let live_area = match direction {
            SplitDirection::Horizontal => Rect::new(2, 3, 3, 4),
            SplitDirection::Vertical => Rect::new(2, 3, 4, 3),
        };
        let zero_usable_area = match direction {
            SplitDirection::Horizontal => Rect::new(2, 3, 1, 4),
            SplitDirection::Vertical => Rect::new(2, 3, 4, 1),
        };
        let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
        state.set_focused(true);

        split.render(live_area, &mut buffer, &mut state);
        assert_eq!(
            state.drag_start(&split, state.layout().divider.as_position()),
            SplitPaneOutcome::Focused
        );
        let before_ratio = state.ratio();

        split.render(zero_usable_area, &mut buffer, &mut state);
        let layout = state.layout();
        assert!(layout.first.is_empty());
        assert!(layout.second.is_empty());
        assert!(layout.divider.is_empty());
        assert_eq!(
            state.handle_key(
                &split,
                KeyEvent::new(
                    match direction {
                        SplitDirection::Horizontal => KeyCode::Right,
                        SplitDirection::Vertical => KeyCode::Down,
                    },
                    KeyModifiers::NONE,
                ),
            ),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(
            state.drag_move(&split, zero_usable_area.as_position()),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.ratio(), before_ratio);
    }
}

#[test]
fn empty_pane_layout_has_no_divider_or_pointer_hit_target() {
    let system = DesignSystem::default();

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let area = match direction {
            SplitDirection::Horizontal => Rect::new(2, 3, 2, 4),
            SplitDirection::Vertical => Rect::new(2, 3, 4, 2),
        };
        let split = SplitPane::new(direction, 0, 0, &system);
        let mut state = SplitPaneState::default();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 8));

        split.render(area, &mut buffer, &mut state);

        let layout = state.layout();
        assert!(layout.first.is_empty());
        assert!(!layout.second.is_empty());
        assert_eq!(layout.divider, Rect::ZERO);
        let would_be_divider = area.as_position();
        assert_eq!(buffer[would_be_divider].symbol(), " ");
        assert_eq!(
            state.drag_start(&split, would_be_divider),
            SplitPaneOutcome::Ignored
        );
        assert!(!state.hover(&split, would_be_divider));
        assert!(!state.is_hovered());
    }
}

#[test]
fn zero_axis_matches_canonical_empty_and_impossible_layouts() {
    let system = DesignSystem::default();

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let area = match direction {
            SplitDirection::Horizontal => Rect::new(7, 9, 0, 4),
            SplitDirection::Vertical => Rect::new(7, 9, 4, 0),
        };
        let split = SplitPane::new(direction, 0, 0, &system);
        let mut state = SplitPaneState::default();
        let layout = split.layout(area, &mut state);
        let expected_first = match direction {
            SplitDirection::Horizontal => Rect::new(area.x, area.y, 0, area.height),
            SplitDirection::Vertical => Rect::new(area.x, area.y, area.width, 0),
        };
        let expected_second = match direction {
            SplitDirection::Horizontal => Rect::new(area.x + 1, area.y, 0, area.height),
            SplitDirection::Vertical => Rect::new(area.x, area.y + 1, area.width, 0),
        };
        assert_eq!(layout.first, expected_first);
        assert_eq!(layout.divider, Rect::ZERO);
        assert_eq!(layout.second, expected_second);

        let impossible = SplitPane::new(direction, 1, 0, &system);
        let impossible_layout = impossible.layout(area, &mut state);
        match direction {
            SplitDirection::Horizontal => {
                assert_eq!(impossible_layout.first, Rect::ZERO);
                assert_eq!(impossible_layout.second, area);
            }
            SplitDirection::Vertical => {
                assert_eq!(impossible_layout.first, area);
                assert_eq!(impossible_layout.second, Rect::ZERO);
            }
        }
        assert_eq!(impossible_layout.divider, Rect::ZERO);
    }
}

#[test]
fn focused_keyboard_resize_is_axis_specific_and_bounded() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    split.layout(Rect::new(0, 0, 41, 4), &mut state);
    state.set_focused(false);
    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        SplitPaneOutcome::Ignored
    );
    state.set_focused(true);
    assert!(matches!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        SplitPaneOutcome::RatioChanged(_)
    ));
    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        SplitPaneOutcome::Ignored
    );
    for _ in 0..100 {
        let _ = state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }
    assert_eq!(state.ratio().basis_points(), 9_500);
}

#[test]
fn focused_keyboard_resize_clamps_both_directions_to_expanded_bounds() {
    let system = DesignSystem::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    split.layout(Rect::new(0, 0, 41, 4), &mut state);
    state.set_focused(true);

    for _ in 0..100 {
        let _ = state.handle_key(&split, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
    }
    assert_eq!(state.ratio().basis_points(), 500);

    for _ in 0..100 {
        let _ = state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
    }
    assert_eq!(state.ratio().basis_points(), 9_500);
}

#[test]
fn saturated_keyboard_is_ignored_without_mutating_collapsed_state() {
    let system = DesignSystem::default();
    let area = Rect::new(2, 3, 31, 4);

    for (side, ratio, key) in [
        (SplitSide::First, SplitRatio::from_percent(5), KeyCode::Left),
        (
            SplitSide::Second,
            SplitRatio::from_percent(95),
            KeyCode::Right,
        ),
    ] {
        let split = SplitPane::new(SplitDirection::Horizontal, 0, 0, &system);
        let mut state = SplitPaneState::new(ratio);
        state.set_focused(true);
        split.layout(area, &mut state);

        let before_ratio = state.ratio();
        let before_layout = state.layout();
        assert_eq!(
            state.handle_key(&split, KeyEvent::new(key, KeyModifiers::NONE)),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.ratio(), before_ratio);
        assert_eq!(state.layout(), before_layout);

        assert_eq!(state.collapse(side), SplitPaneOutcome::Collapsed(side));
        split.layout(area, &mut state);

        let before_ratio = state.ratio();
        let before_layout = state.layout();
        let before_hovered = state.is_hovered();
        let before_dragging = state.is_dragging();
        assert_eq!(
            state.handle_key(&split, KeyEvent::new(key, KeyModifiers::NONE)),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.ratio(), before_ratio);
        assert_eq!(state.layout(), before_layout);
        assert_eq!(state.is_hovered(), before_hovered);
        assert_eq!(state.is_dragging(), before_dragging);
    }
}

#[test]
fn no_op_drag_is_ignored_without_mutating_ratio_or_interaction_state() {
    let system = DesignSystem::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 0, 0, &system);
    let area = Rect::new(2, 3, 31, 4);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(5));
    assert_eq!(
        state.collapse(SplitSide::First),
        SplitPaneOutcome::Collapsed(SplitSide::First)
    );
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 12));
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;
    assert!(divider.is_empty());
    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Ignored
    );

    let before_ratio = state.ratio();
    let before_layout = state.layout();
    let before_hovered = state.is_hovered();
    let before_dragging = state.is_dragging();
    assert_eq!(
        state.drag_move(&split, divider.as_position()),
        SplitPaneOutcome::Ignored
    );
    assert_eq!(state.ratio(), before_ratio);
    assert_eq!(state.layout(), before_layout);
    assert_eq!(state.is_hovered(), before_hovered);
    assert_eq!(state.is_dragging(), before_dragging);
}

#[test]
fn drag_at_minimum_clamped_seam_is_ignored_without_mutating_state() {
    let system = DesignSystem::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 99, 1, &system);
    let area = Rect::new(2, 3, 101, 4);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    let mut buffer = Buffer::empty(Rect::new(0, 0, 110, 12));

    split.render(area, &mut buffer, &mut state);
    assert_eq!(state.layout().first.width, 99);
    let divider = state.layout().divider;
    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Focused
    );

    let before_ratio = state.ratio();
    let before_layout = state.layout();
    assert_eq!(
        state.drag_move(&split, divider.as_position()),
        SplitPaneOutcome::Ignored
    );
    assert_eq!(state.ratio(), before_ratio);
    assert_eq!(state.layout(), before_layout);
}

#[test]
fn pointer_drag_stores_canonical_percent_and_round_trips_the_cell_seam() {
    let system = DesignSystem::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 0, 0, &system);
    let area = Rect::new(2, 3, 8, 4);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    let mut buffer = Buffer::empty(Rect::new(0, 0, 16, 12));
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;
    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Focused
    );

    assert!(matches!(
        state.drag_move(&split, Position::new(area.x + 2, area.y)),
        SplitPaneOutcome::RatioChanged(ratio) if ratio.basis_points() == 2_900
    ));
    assert_eq!(state.layout().first.width, 2);
}

#[test]
fn keyboard_resize_stores_canonical_percent_and_round_trips_the_cell_seam() {
    let system = DesignSystem::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 0, 0, &system);
    let area = Rect::new(2, 3, 8, 4);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    state.set_focused(true);
    split.layout(area, &mut state);

    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        SplitPaneOutcome::RatioChanged(SplitRatio::from_percent(29))
    );
    assert_eq!(state.ratio().basis_points(), 2_900);
    assert_eq!(split.layout(area, &mut state).first.width, 2);
}

#[test]
fn keyboard_resize_ignores_ratio_change_without_physical_seam_move() {
    let system = DesignSystem::default();

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let split = SplitPane::new(direction, 0, 0, &system);
        let area = match direction {
            SplitDirection::Horizontal => Rect::new(2, 3, 3, 4),
            SplitDirection::Vertical => Rect::new(2, 3, 4, 3),
        };
        let mut state = SplitPaneState::new(SplitRatio::from_percent(94));
        state.set_focused(true);
        split.layout(area, &mut state);
        let before_ratio = state.ratio();
        let before_layout = state.layout();

        let key = match direction {
            SplitDirection::Horizontal => KeyCode::Right,
            SplitDirection::Vertical => KeyCode::Down,
        };
        assert_eq!(
            state.handle_key(&split, KeyEvent::new(key, KeyModifiers::NONE)),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.ratio(), before_ratio);
        assert_eq!(state.layout(), before_layout);
    }
}

#[test]
fn focused_keyboard_resize_clamps_to_physical_minima_on_both_axes() {
    let system = DesignSystem::default();

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let split = SplitPane::new(direction, 8, 9, &system);
        let area = match direction {
            SplitDirection::Horizontal => Rect::new(3, 4, 31, 8),
            SplitDirection::Vertical => Rect::new(3, 4, 8, 31),
        };
        let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
        state.set_focused(true);
        split.layout(area, &mut state);

        let negative_key = match direction {
            SplitDirection::Horizontal => KeyCode::Left,
            SplitDirection::Vertical => KeyCode::Up,
        };
        let mut reached_first_minimum = false;
        for _ in 0..20 {
            let outcome = state.handle_key(&split, KeyEvent::new(negative_key, KeyModifiers::NONE));
            if reached_first_minimum {
                assert_eq!(outcome, SplitPaneOutcome::Ignored);
            } else {
                assert!(matches!(outcome, SplitPaneOutcome::RatioChanged(_)));
            }
            let layout = split.layout(area, &mut state);
            let (first, second) = match direction {
                SplitDirection::Horizontal => (layout.first.width, layout.second.width),
                SplitDirection::Vertical => (layout.first.height, layout.second.height),
            };
            assert!(first >= 8);
            assert!(second >= 9);
            reached_first_minimum = first == 8;
        }
        assert!(reached_first_minimum);
        let layout = split.layout(area, &mut state);
        let first = match direction {
            SplitDirection::Horizontal => layout.first.width,
            SplitDirection::Vertical => layout.first.height,
        };
        assert_eq!(first, 8);

        let positive_key = match direction {
            SplitDirection::Horizontal => KeyCode::Right,
            SplitDirection::Vertical => KeyCode::Down,
        };
        let mut reached_second_minimum = false;
        for _ in 0..40 {
            let outcome = state.handle_key(&split, KeyEvent::new(positive_key, KeyModifiers::NONE));
            if reached_second_minimum {
                assert_eq!(outcome, SplitPaneOutcome::Ignored);
            } else {
                assert!(matches!(outcome, SplitPaneOutcome::RatioChanged(_)));
            }
            let layout = split.layout(area, &mut state);
            let (first, second) = match direction {
                SplitDirection::Horizontal => (layout.first.width, layout.second.width),
                SplitDirection::Vertical => (layout.first.height, layout.second.height),
            };
            assert!(first >= 8);
            assert!(second >= 9);
            reached_second_minimum = second == 9;
        }
        assert!(reached_second_minimum);
        let layout = split.layout(area, &mut state);
        let (first, second) = match direction {
            SplitDirection::Horizontal => (layout.first.width, layout.second.width),
            SplitDirection::Vertical => (layout.first.height, layout.second.height),
        };
        assert_eq!(first, 21);
        assert_eq!(second, 9);
    }
}

#[test]
fn impossible_minimum_interactions_are_ignored_without_mutation_on_both_axes() {
    let system = DesignSystem::default();

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let split = SplitPane::new(direction, 6, 6, &system);
        let feasible_area = match direction {
            SplitDirection::Horizontal => Rect::new(3, 4, 31, 8),
            SplitDirection::Vertical => Rect::new(3, 4, 8, 31),
        };
        let impossible_area = match direction {
            SplitDirection::Horizontal => Rect::new(3, 4, 10, 8),
            SplitDirection::Vertical => Rect::new(3, 4, 8, 10),
        };
        let mut state = SplitPaneState::new(SplitRatio::from_percent(40));
        state.set_focused(true);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 48, 48));
        split.render(feasible_area, &mut buffer, &mut state);
        let divider = state.layout().divider;
        assert_eq!(
            state.drag_start(&split, divider.as_position()),
            SplitPaneOutcome::Focused
        );
        let before = state.ratio();

        split.render(impossible_area, &mut buffer, &mut state);
        let key = match direction {
            SplitDirection::Horizontal => KeyCode::Right,
            SplitDirection::Vertical => KeyCode::Down,
        };
        assert_eq!(
            state.handle_key(&split, KeyEvent::new(key, KeyModifiers::NONE)),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.ratio(), before);
        assert_eq!(
            state.drag_move(&split, impossible_area.as_position()),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.ratio(), before);
    }
}

#[test]
fn pointer_drag_clamps_horizontal_and_vertical_edges_to_expanded_bounds() {
    let system = DesignSystem::default();

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let split = SplitPane::new(direction, 1, 1, &system);
        let area = Rect::new(5, 7, 31, 31);
        let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 48, 48));
        split.render(area, &mut buffer, &mut state);
        let divider = state.layout().divider;

        assert_eq!(
            state.drag_start(&split, divider.as_position()),
            SplitPaneOutcome::Focused
        );
        let before_start = match direction {
            SplitDirection::Horizontal => Position::new(area.x - 1, area.y),
            SplitDirection::Vertical => Position::new(area.x, area.y - 1),
        };
        assert!(matches!(
            state.drag_move(&split, before_start),
            SplitPaneOutcome::RatioChanged(ratio) if ratio.basis_points() == 500
        ));

        let after_end = match direction {
            SplitDirection::Horizontal => Position::new(area.x + area.width, area.y),
            SplitDirection::Vertical => Position::new(area.x, area.y + area.height),
        };
        assert!(matches!(
            state.drag_move(&split, after_end),
            SplitPaneOutcome::RatioChanged(ratio) if ratio.basis_points() == 9_500
        ));
    }
}

#[test]
fn collapse_preserves_ratio_and_each_side_can_expand() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 3, 3, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(35));
    state.set_focused(true);
    let area = Rect::new(0, 0, 21, 4);

    assert_eq!(
        state.collapse(SplitSide::First),
        SplitPaneOutcome::Collapsed(SplitSide::First)
    );
    let first_hidden = split.layout(area, &mut state);
    assert!(first_hidden.first.is_empty());
    assert_eq!(first_hidden.divider, Rect::ZERO);
    assert_eq!(first_hidden.second, area);
    assert_eq!(state.expand(), SplitPaneOutcome::Expanded);
    assert_eq!(state.ratio(), SplitRatio::from_percent(35));

    assert_eq!(
        state.collapse(SplitSide::Second),
        SplitPaneOutcome::Collapsed(SplitSide::Second)
    );
    let second_hidden = split.layout(area, &mut state);
    assert!(second_hidden.second.is_empty());
    assert_eq!(second_hidden.divider, Rect::ZERO);
    assert_eq!(second_hidden.first, area);
}

#[test]
fn collapsed_layout_maximizes_remaining_pane_without_a_handle() {
    let system = DesignSystem::default();
    let area = Rect::new(3, 4, 21, 7);

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let area = match direction {
            SplitDirection::Horizontal => area,
            SplitDirection::Vertical => Rect::new(area.x, area.y, area.height, area.width),
        };
        let split = SplitPane::new(direction, 4, 4, &system);
        let mut state = SplitPaneState::default();
        state.set_focused(true);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 32, 32));

        assert_eq!(
            state.collapse(SplitSide::First),
            SplitPaneOutcome::Collapsed(SplitSide::First)
        );
        split.render(area, &mut buffer, &mut state);
        assert_eq!(state.layout().first, Rect::ZERO);
        assert_eq!(state.layout().divider, Rect::ZERO);
        assert_eq!(state.layout().second, area);
        assert_eq!(
            state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(
            state.drag_start(&split, area.as_position()),
            SplitPaneOutcome::Ignored
        );
        assert_eq!(state.expand(), SplitPaneOutcome::Expanded);

        assert_eq!(
            state.collapse(SplitSide::Second),
            SplitPaneOutcome::Collapsed(SplitSide::Second)
        );
        split.render(area, &mut buffer, &mut state);
        assert_eq!(state.layout().first, area);
        assert_eq!(state.layout().divider, Rect::ZERO);
        assert_eq!(state.layout().second, Rect::ZERO);
    }
}

#[test]
fn collapse_and_expand_preserve_the_normalized_expanded_ratio() {
    let mut state = SplitPaneState::new(SplitRatio::from_basis_points(u16::MAX));

    assert_eq!(state.ratio().basis_points(), 9_500);
    assert_eq!(
        state.collapse(SplitSide::First),
        SplitPaneOutcome::Collapsed(SplitSide::First)
    );
    assert_eq!(state.expand(), SplitPaneOutcome::Expanded);
    assert_eq!(state.ratio().basis_points(), 9_500);

    assert_eq!(
        state.collapse(SplitSide::Second),
        SplitPaneOutcome::Collapsed(SplitSide::Second)
    );
    assert_eq!(state.expand(), SplitPaneOutcome::Expanded);
    assert_eq!(state.ratio().basis_points(), 9_500);
}

#[test]
fn painted_divider_supports_focus_drag_and_release() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 2, 2, &system);
    let area = Rect::new(5, 7, 31, 5);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    // One divider glyph in every state; the role says focused vs hovered.
    state.set_focused(false);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 16));
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;

    assert!(state.hover(&split, divider.as_position()));
    assert!(state.is_hovered());
    split.render(area, &mut buffer, &mut state);
    assert_eq!(
        buffer[divider.as_position()].symbol(),
        system.glyphs.rule_v()
    );
    assert_eq!(
        buffer[divider.as_position()].fg,
        theme.style(Role::Focus).fg.unwrap()
    );
    assert!(state.hover(&split, Position::new(0, 0)));
    assert!(!state.is_hovered());

    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Focused
    );
    assert!(state.is_dragging());
    assert!(matches!(
        state.drag_move(&split, Position::new(area.x + 23, area.y)),
        SplitPaneOutcome::RatioChanged(_)
    ));
    state.drag_end();
    assert!(!state.is_dragging());
    let moved = split.layout(area, &mut state);
    assert_eq!(moved.first.width, 23);
}

#[test]
fn only_same_direction_rendered_geometry_authorizes_pointer_input() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let horizontal = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
    let vertical = SplitPane::new(SplitDirection::Vertical, 1, 1, &system);
    let area = Rect::new(2, 3, 15, 7);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    state.set_focused(true);
    let computed = horizontal.layout(area, &mut state);

    assert_eq!(
        state.drag_start(&horizontal, computed.divider.as_position()),
        SplitPaneOutcome::Ignored,
        "computed-only geometry is not a hit target"
    );

    let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 12));
    horizontal.render(area, &mut buffer, &mut state);
    let painted = state.layout().divider;
    assert_eq!(
        state.drag_start(&vertical, painted.as_position()),
        SplitPaneOutcome::Ignored,
        "stale geometry cannot cross directions"
    );

    let mut zero = Buffer::empty(Rect::ZERO);
    horizontal.render(Rect::ZERO, &mut zero, &mut state);
    assert_eq!(
        state.drag_start(&horizontal, painted.as_position()),
        SplitPaneOutcome::Ignored,
        "zero repaint invalidates the old divider"
    );
}

#[test]
fn vertical_keyboard_pointer_and_collapsed_rendering_match_horizontal_behavior() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Vertical, 2, 2, &system);
    let area = Rect::new(3, 4, 10, 31);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    state.set_focused(true);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 16, 40));
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;

    assert!(state.hover(&split, divider.as_position()));
    assert_eq!(
        state.drag_start(&split, Position::new(0, 0)),
        SplitPaneOutcome::Ignored
    );
    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Focused
    );
    assert!(matches!(
        state.drag_move(&split, Position::new(area.x, area.y + 23)),
        SplitPaneOutcome::RatioChanged(_)
    ));
    state.drag_end();
    assert_eq!(split.layout(area, &mut state).first.height, 23);
    assert!(matches!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        SplitPaneOutcome::RatioChanged(_)
    ));
    assert!(matches!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        SplitPaneOutcome::RatioChanged(_)
    ));

    state.collapse(SplitSide::Second);
    split.render(area, &mut buffer, &mut state);
    assert_eq!(state.layout().first, area);
    assert_eq!(state.layout().divider, Rect::ZERO);
    assert_eq!(state.layout().second, Rect::ZERO);
}

#[test]
fn focused_and_collapsed_dividers_have_non_color_glyphs() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
    let area = Rect::new(0, 0, 9, 3);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    state.set_focused(true);
    let mut buffer = Buffer::empty(area);

    split.render(area, &mut buffer, &mut state);
    // Focus swaps the border role; it never thickens the glyph.
    assert_eq!(
        buffer[state.layout().divider.as_position()].symbol(),
        system.glyphs.rule_v()
    );
    assert_eq!(
        buffer[state.layout().divider.as_position()].fg,
        theme.style(Role::BorderFocused).fg.unwrap()
    );

    state.collapse(SplitSide::First);
    split.render(area, &mut buffer, &mut state);
    assert_eq!(state.layout().first, Rect::ZERO);
    assert_eq!(state.layout().divider, Rect::ZERO);
    assert_eq!(state.layout().second, area);
}
