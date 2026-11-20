use ratatui::layout::Rect;
use termrock::core::id::WidgetId;
use termrock::widgets::nav_list::{NavItem, NavList};

use crate::common::paint_in;

#[test]
fn a_nav_list_too_narrow_to_paint_a_row_is_not_a_focus_stop() {
    let id = WidgetId::of("t.nav");
    let mut nav = NavList::new(id, vec![NavItem::new("one"), NavItem::new("two")]);
    let screen = Rect::new(0, 0, 20, 6);
    for (width, stop) in [(0, false), (1, false), (2, true), (8, true)] {
        let p = paint_in(screen, None, |buf, ctx| {
            nav.render(Rect::new(0, 0, width, 4), buf, ctx, ctx.theme.canvas)
        });
        assert_eq!(p.ring.contains(id), stop, "width {width}");
    }
}
