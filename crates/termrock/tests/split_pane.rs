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
fn impossible_minimums_degrade_proportionally_without_overflow() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 90, 10, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(5));
    state.set_focused(true);
    let layout = split.layout(Rect::new(0, 0, 51, 2), &mut state);
    assert_eq!(layout.first.width, 45);
    assert_eq!(layout.divider.width, 1);
    assert_eq!(layout.second.width, 5);

    let maximums = SplitPane::new(SplitDirection::Horizontal, u16::MAX, u16::MAX, &system);
    let layout = maximums.layout(Rect::new(0, 0, u16::MAX, 1), &mut state);
    assert_eq!(layout.first.width, 32_767);
    assert_eq!(layout.divider.width, 1);
    assert_eq!(layout.second.width, 32_767);

    let vertical = SplitPane::new(SplitDirection::Vertical, 90, 10, &system);
    let layout = vertical.layout(Rect::new(0, 0, 2, 51), &mut state);
    assert_eq!(layout.first.height, 45);
    assert_eq!(layout.divider.height, 1);
    assert_eq!(layout.second.height, 5);

    let horizontal_one = SplitPane::new(SplitDirection::Horizontal, 1, 1, &system);
    let horizontal_one_layout = horizontal_one.layout(Rect::new(4, 5, 1, 3), &mut state);
    assert!(horizontal_one_layout.first.is_empty());
    assert!(horizontal_one_layout.divider.is_empty());
    assert!(horizontal_one_layout.second.is_empty());

    let vertical_one = SplitPane::new(SplitDirection::Vertical, 1, 1, &system);
    let vertical_one_layout = vertical_one.layout(Rect::new(4, 5, 3, 1), &mut state);
    assert!(vertical_one_layout.first.is_empty());
    assert!(vertical_one_layout.divider.is_empty());
    assert!(vertical_one_layout.second.is_empty());
}

#[test]
fn focused_keyboard_resize_is_axis_specific_and_bounded() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 2, 2, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    let area = Rect::new(0, 0, 31, 3);
    split.layout(area, &mut state);
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
    assert_eq!(split.layout(area, &mut state).first.width, 28);
    let bounded = state.ratio();
    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        SplitPaneOutcome::Ignored
    );
    assert_eq!(state.ratio(), bounded);

    state.collapse(SplitSide::First);
    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        SplitPaneOutcome::Ignored
    );
}

#[test]
fn keyboard_resize_before_layout_preserves_basis_point_setter_behavior() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme);
    let split = SplitPane::new(SplitDirection::Horizontal, 2, 2, &system);
    let mut state = SplitPaneState::default();
    state.set_focused(true);

    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
        SplitPaneOutcome::RatioChanged(SplitRatio::from_basis_points(5_250))
    );
    assert_eq!(
        state.handle_key(&split, KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)),
        SplitPaneOutcome::RatioChanged(SplitRatio::from_basis_points(5_000))
    );
}

#[test]
fn keyboard_geometry_is_direction_tagged() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme);
    let horizontal = SplitPane::new(SplitDirection::Horizontal, 2, 2, &system);
    let vertical = SplitPane::new(SplitDirection::Vertical, 2, 2, &system);
    let mut state = SplitPaneState::default();
    let area = Rect::new(2, 3, 21, 11);
    horizontal.layout(area, &mut state);
    state.set_focused(true);
    let ratio = state.ratio();

    assert_eq!(
        state.handle_key(&vertical, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        SplitPaneOutcome::Ignored
    );
    assert_eq!(state.ratio(), ratio);
    assert!(matches!(
        state.handle_key(
            &horizontal,
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)
        ),
        SplitPaneOutcome::RatioChanged(_)
    ));
}

#[test]
fn collapse_preserves_ratio_and_each_side_can_expand() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme.clone());
    let split = SplitPane::new(SplitDirection::Horizontal, 3, 3, &system);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(35));
    state.set_focused(true);
    let area = Rect::new(0, 0, 21, 4);
    let mut buffer = Buffer::empty(area);
    split.render(area, &mut buffer, &mut state);
    let old_divider = state.layout().divider;

    assert_eq!(
        state.collapse(SplitSide::First),
        SplitPaneOutcome::Collapsed(SplitSide::First)
    );
    assert_eq!(
        state.drag_start(&split, old_divider.as_position()),
        SplitPaneOutcome::Ignored
    );
    let first_hidden = split.layout(area, &mut state);
    assert!(first_hidden.first.is_empty());
    assert!(first_hidden.divider.is_empty());
    assert_eq!(first_hidden.second, area);
    assert_eq!(state.expand(), SplitPaneOutcome::Expanded);
    assert_eq!(state.ratio(), SplitRatio::from_percent(35));

    assert_eq!(
        state.collapse(SplitSide::Second),
        SplitPaneOutcome::Collapsed(SplitSide::Second)
    );
    let second_hidden = split.layout(area, &mut state);
    assert!(second_hidden.second.is_empty());
    assert!(second_hidden.divider.is_empty());
    assert_eq!(second_hidden.first, area);
}

#[test]
fn zero_and_one_cell_axes_have_no_panes_or_interaction_divider() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme);
    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let split = SplitPane::new(direction, 0, 0, &system);
        let mut state = SplitPaneState::default();
        let areas = match direction {
            SplitDirection::Horizontal => [Rect::ZERO, Rect::new(3, 4, 1, 5)],
            SplitDirection::Vertical => [Rect::ZERO, Rect::new(3, 4, 5, 1)],
        };
        for area in areas {
            let layout = split.layout(area, &mut state);
            assert!(layout.first.is_empty());
            assert!(layout.divider.is_empty());
            assert!(layout.second.is_empty());
            state.set_focused(true);
            assert_eq!(
                state.handle_key(&split, KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)),
                SplitPaneOutcome::Ignored
            );
            let mut buffer = Buffer::empty(area);
            split.render(area, &mut buffer, &mut state);
            assert_eq!(
                state.drag_start(&split, Position::new(area.x, area.y)),
                SplitPaneOutcome::Ignored
            );
            assert!(!state.hover(&split, Position::new(area.x, area.y)));
        }
    }
}

#[test]
fn empty_pane_layout_has_no_divider_or_pointer_hit_target() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme);
    let split = SplitPane::new(SplitDirection::Horizontal, 0, 0, &system);
    let area = Rect::new(2, 3, 7, 2);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 8));
    for ratio in [
        SplitRatio::from_basis_points(0),
        SplitRatio::from_basis_points(10_000),
    ] {
        let mut state = SplitPaneState::new(ratio);
        split.render(area, &mut buffer, &mut state);
        let layout = state.layout();
        assert!(layout.divider.is_empty());
        assert_eq!(
            state.drag_start(&split, Position::new(area.x, area.y)),
            SplitPaneOutcome::Ignored
        );
        assert!(!state.hover(&split, Position::new(area.x, area.y)));
        if ratio.basis_points() == 0 {
            assert!(layout.first.is_empty());
            assert_eq!(layout.second, area);
        } else {
            assert_eq!(layout.first, area);
            assert!(layout.second.is_empty());
        }
    }
}

#[test]
fn ratio_endpoints_keep_the_documented_zero_to_hundred_contract() {
    assert_eq!(SplitRatio::from_percent(0).basis_points(), 0);
    assert_eq!(SplitRatio::from_percent(100).basis_points(), 10_000);
    assert_eq!(SplitRatio::from_percent(101).basis_points(), 10_000);
    assert_eq!(
        SplitRatio::from_basis_points(u16::MAX).basis_points(),
        10_000
    );

    let theme = RolePalette::default();
    let system = DesignSystem::new(theme);
    let split = SplitPane::new(SplitDirection::Horizontal, 0, 0, &system);
    let area = Rect::new(2, 3, 7, 2);
    let mut zero = SplitPaneState::new(SplitRatio::from_percent(0));
    let zero_layout = split.layout(area, &mut zero);
    assert!(zero_layout.first.is_empty());
    assert!(zero_layout.divider.is_empty());
    assert_eq!(zero_layout.second, area);

    let mut hundred = SplitPaneState::new(SplitRatio::from_percent(100));
    let hundred_layout = split.layout(area, &mut hundred);
    assert_eq!(hundred_layout.first, area);
    assert!(hundred_layout.divider.is_empty());
    assert!(hundred_layout.second.is_empty());
}

#[cfg(feature = "serde")]
#[test]
fn serde_deserialization_clamps_out_of_range_basis_points() {
    let ratio: SplitRatio = serde_json::from_str("65535").expect("valid ratio integer");
    assert_eq!(ratio, SplitRatio::from_basis_points(10_000));
    let ratio: SplitRatio = serde_json::from_str("0").expect("valid ratio integer");
    assert_eq!(ratio, SplitRatio::from_basis_points(0));
}

#[test]
fn drag_at_same_seam_is_ignored_and_minimums_bound_the_move() {
    let theme = RolePalette::default();
    let system = DesignSystem::new(theme);
    let split = SplitPane::new(SplitDirection::Horizontal, 2, 2, &system);
    let area = Rect::new(5, 7, 31, 4);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 16));
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;
    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Focused
    );
    assert_eq!(
        state.drag_move(&split, divider.as_position()),
        SplitPaneOutcome::Ignored
    );
    assert_eq!(
        state.drag_move(&split, Position::new(area.x, area.y)),
        SplitPaneOutcome::RatioChanged(SplitRatio::from_basis_points(667))
    );
    state.drag_end();
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;
    assert_eq!(
        state.drag_start(&split, divider.as_position()),
        SplitPaneOutcome::Focused
    );
    assert_eq!(
        state.drag_move(&split, Position::new(area.right(), area.y)),
        SplitPaneOutcome::RatioChanged(SplitRatio::from_basis_points(9333))
    );
    assert_eq!(split.layout(area, &mut state).first.width, 28);
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
    assert!(state.layout().divider.is_empty());
    assert_eq!(state.layout().first, area);
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
    buffer = Buffer::empty(area);
    split.render(area, &mut buffer, &mut state);
    assert!(state.layout().divider.is_empty());
    assert_eq!(state.layout().second, area);
}
