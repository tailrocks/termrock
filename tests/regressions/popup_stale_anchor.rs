//! A popup whose anchor a resize left off screen must still land on screen.

use junie_tui::core::id::WidgetId;
use junie_tui::ui::popup::{Overflow, Place, Placement, place_with};
use junie_tui::widgets::completion::Completion;
use junie_tui::widgets::menu::{ContextMenu, MenuItem};
use junie_tui::widgets::option_list::Item;
use ratatui::layout::Rect;

use crate::common::paint_in;

#[test]
fn a_placement_stays_inside_the_screen_for_an_anchor_outside_it() {
    let mut bad = Vec::new();
    for (sw, sh) in [(1u16, 1u16), (6, 6), (20, 8), (80, 24)] {
        let screen = Rect::new(0, 0, sw, sh);
        for ax in [0, 3, sw, sw + 10] {
            for ay in [0, 3, sh, sh + 10] {
                for (w, h) in [(1, 1), (5, 3), (30, 12), (200, 100)] {
                    for placement in [
                        Placement::Below,
                        Placement::Above,
                        Placement::Right,
                        Placement::Left,
                    ] {
                        for overflow in [Overflow::Shrink, Overflow::Slide] {
                            let fit = Place {
                                overflow,
                                ..Place::DEFAULT
                            };
                            let anchor = Rect::new(ax, ay, 1, 1);
                            let r = place_with(screen, anchor, w, h, placement, fit);
                            if screen.intersection(r) != r {
                                bad.push((screen, anchor, (w, h), placement, r));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} placements left the screen, first {:?}",
        bad.len(),
        bad.first()
    );
}

/// The row menu opened low in a tall terminal keeps its anchor when the
/// terminal shrinks and the row is no longer drawn. The title row used to be
/// written below a one row popup.
#[test]
fn a_titled_menu_survives_a_resize_that_strands_its_anchor() {
    let items = (0..9)
        .map(|i| MenuItem::new(format!("Action {i}")))
        .collect();
    let mut menu = ContextMenu::new(WidgetId::of("t.menu"), items)
        .title("Row actions")
        .anchor(Rect::new(29, 11, 1, 1), Placement::Below);
    for (w, h) in [(40, 14), (12, 6), (12, 1)] {
        let screen = Rect::new(0, 0, w, h);
        paint_in(screen, None, |buf, ctx| menu.render(screen, buf, ctx));
        assert_eq!(
            screen.intersection(menu.area),
            menu.area,
            "menu area {:?} leaves a {w}x{h} screen",
            menu.area
        );
    }
}

#[test]
fn a_completion_popup_stays_on_screen_after_a_resize() {
    let mut popup = Completion::new(WidgetId::of("t.completion"));
    popup.open(
        vec![Item::new("alpha"), Item::new("beta"), Item::new("gamma")],
        Rect::new(60, 18, 1, 1),
        1,
    );
    for (w, h) in [(80, 24), (20, 6), (12, 3), (12, 1)] {
        let screen = Rect::new(0, 0, w, h);
        paint_in(screen, None, |buf, ctx| popup.render(screen, buf, ctx));
        assert_eq!(
            screen.intersection(popup.area),
            popup.area,
            "completion area {:?} leaves a {w}x{h} screen",
            popup.area
        );
    }
}
