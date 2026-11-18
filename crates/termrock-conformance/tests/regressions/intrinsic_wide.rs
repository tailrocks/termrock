//! Pure checks of the wide intrinsic helpers. They read the true totals that a
//! public measurement saturates, without laying out a huge collection.

use termrock_controls::button::{Button, wrap_lines};
use termrock_core::id::WidgetId;
use termrock_feedback::statusbar::{StatusBar, StatusItem};
use termrock_fields::chips::{Chip, ChipBar, overflow_run_cells};
use termrock_navigation::breadcrumb::Breadcrumb;
use termrock_navigation::toolbar::{ToolItem, Toolbar};
use termrock_theme::Tone;

const ID: WidgetId = WidgetId::of("intrinsic.wide");

#[test]
fn intrinsic_totals_match_arithmetic_beyond_u16() {
    let bar = Toolbar::new(ID, (0..20_000).map(|_| ToolItem::new("x")).collect());
    assert_eq!(bar.intrinsic_width(), 20_000 * 3 + 19_999);

    let sep = 3;
    let crumbs = Breadcrumb::new(ID, vec!["x".to_owned(); 20_000]);
    assert_eq!(crumbs.intrinsic_width(sep), 20_000 + sep * 19_999);

    let mut status = StatusBar::new();
    status.left = (0..20_000)
        .map(|_| StatusItem::new("x", Tone::Normal))
        .collect();
    assert_eq!(status.intrinsic_width(), 20_000 + 3 * 19_999 + 2);

    let mut chips = ChipBar::new(ID);
    chips.set_add_label(None);
    chips.set_chips((0..10_000).map(|_| Chip::new("x")).collect());
    assert_eq!(chips.intrinsic_width(), 10_000 * 7 - 1);
}

#[test]
fn intrinsic_sum_overflow_never_becomes_small_after_gap_subtraction() {
    let wide = [usize::MAX, usize::MAX, 3];
    for gap in [0, 1, usize::MAX] {
        let (shown, marker) = overflow_run_cells(&wide, 80, gap);
        assert_eq!(shown, 0, "gap {gap}");
        assert_eq!(marker.as_deref(), Some("+3"), "gap {gap}");
    }
    let lines = wrap_lines(80, &[usize::MAX, 3, usize::MAX], usize::MAX, usize::MAX);
    assert_eq!(lines, vec![vec![0], vec![1], vec![2]]);
    let fits = wrap_lines(80, &[10, 10, 10], 1, usize::MAX);
    assert_eq!(fits, vec![vec![0, 1, 2]]);
}

#[test]
fn intrinsic_button_width_is_wide_before_it_is_saturated() {
    let b = Button::primary(ID, &"x".repeat(70_000));
    assert_eq!(b.intrinsic_width(), 70_002);
    assert_eq!(b.width(), u16::MAX);
}

#[test]
fn intrinsic_props_row_count_saturates() {
    use termrock_collections::props::{Prop, PropsList, measure};
    let props: Vec<Prop> = (0..70_000).map(|_| Prop::new("k", "v")).collect();
    assert_eq!(measure(&props, 80), u16::MAX);
    let list = PropsList::new(ID, props);
    assert_eq!(list.height_at(80), u16::MAX);
}
