use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::StatefulWidget,
};
use termrock::{
    Theme,
    input::{KeyCode, KeyEvent, KeyModifiers},
    widgets::{SplitDirection, SplitPane, SplitPaneOutcome, SplitPaneState, SplitRatio, SplitSide},
};

#[test]
fn horizontal_layout_honors_ratio_and_minimums() {
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 10, 15, &theme);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(40));

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
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Vertical, 8, 8, &theme);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));

    let regular = split.layout(Rect::new(4, 6, 12, 21), &mut state);
    assert_eq!(regular.first, Rect::new(4, 6, 12, 10));
    assert_eq!(regular.divider, Rect::new(4, 16, 12, 1));
    assert_eq!(regular.second, Rect::new(4, 17, 12, 10));

    for direction in [SplitDirection::Horizontal, SplitDirection::Vertical] {
        let tiny = SplitPane::new(direction, 8, 8, &theme);
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
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 90, 10, &theme);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(5));
    let layout = split.layout(Rect::new(0, 0, 51, 2), &mut state);
    assert_eq!(layout.first.width, 45);
    assert_eq!(layout.second.width, 5);

    let maximums = SplitPane::new(SplitDirection::Horizontal, u16::MAX, u16::MAX, &theme);
    let layout = maximums.layout(Rect::new(0, 0, u16::MAX, 1), &mut state);
    assert_eq!(layout.first.width, 32_767);
    assert_eq!(layout.second.width, 32_767);
}

#[test]
fn focused_keyboard_resize_is_axis_specific_and_bounded() {
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 2, 2, &theme);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));

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
    assert_eq!(state.ratio().basis_points(), 10_000);
}

#[test]
fn collapse_preserves_ratio_and_each_side_can_expand() {
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 3, 3, &theme);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(35));
    let area = Rect::new(0, 0, 21, 4);

    assert_eq!(
        state.collapse(SplitSide::First),
        SplitPaneOutcome::Collapsed(SplitSide::First)
    );
    let first_hidden = split.layout(area, &mut state);
    assert!(first_hidden.first.is_empty());
    assert_eq!(first_hidden.second.width, 20);
    assert_eq!(state.expand(), SplitPaneOutcome::Expanded);
    assert_eq!(state.ratio(), SplitRatio::from_percent(35));

    assert_eq!(
        state.collapse(SplitSide::Second),
        SplitPaneOutcome::Collapsed(SplitSide::Second)
    );
    let second_hidden = split.layout(area, &mut state);
    assert!(second_hidden.second.is_empty());
    assert_eq!(second_hidden.first.width, 20);
}

#[test]
fn painted_divider_supports_focus_drag_and_release() {
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 2, 2, &theme);
    let area = Rect::new(5, 7, 31, 5);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 16));
    split.render(area, &mut buffer, &mut state);
    let divider = state.layout().divider;

    assert!(state.hover(&split, divider.as_position()));
    assert!(state.is_hovered());
    split.render(area, &mut buffer, &mut state);
    assert_eq!(buffer[divider.as_position()].symbol(), "┋");
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
    let theme = Theme::default();
    let horizontal = SplitPane::new(SplitDirection::Horizontal, 1, 1, &theme);
    let vertical = SplitPane::new(SplitDirection::Vertical, 1, 1, &theme);
    let area = Rect::new(2, 3, 15, 7);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
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
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Vertical, 2, 2, &theme);
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
    assert_eq!(buffer[state.layout().divider.as_position()].symbol(), "⌃");
}

#[test]
fn focused_and_collapsed_dividers_have_non_color_glyphs() {
    let theme = Theme::default();
    let split = SplitPane::new(SplitDirection::Horizontal, 1, 1, &theme);
    let area = Rect::new(0, 0, 9, 3);
    let mut state = SplitPaneState::new(SplitRatio::from_percent(50));
    state.set_focused(true);
    let mut buffer = Buffer::empty(area);

    split.render(area, &mut buffer, &mut state);
    assert_eq!(buffer[state.layout().divider.as_position()].symbol(), "┃");

    state.collapse(SplitSide::First);
    split.render(area, &mut buffer, &mut state);
    assert_eq!(buffer[state.layout().divider.as_position()].symbol(), "›");
}
