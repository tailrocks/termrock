use crate::common::*;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use termrock::core::hit::HitRegistry;
use termrock::core::id::WidgetId;
use termrock::core::intent::Phase;
use termrock::core::pointer::{MouseTarget, PointerEvent};
use termrock::theme::{ColorLevel, Theme};
use termrock::widgets::nav_list::{NavItem, NavList};
use termrock::widgets::rail::SidebarRail;
use termrock::widgets::scrollbar;

const ID: WidgetId = WidgetId::of("t.nav");
const AREA: Rect = Rect::new(0, 0, 30, 8);

fn nav() -> NavList {
    NavList::new(
        ID,
        (0..12)
            .map(|n| NavItem::new(&format!("item {n:02}")))
            .collect(),
    )
}

fn render(nav: &mut NavList) -> (Buffer, HitRegistry) {
    let mut hits = HitRegistry::default();
    let (buf, _) = draw(None, |buf, ctx| {
        nav.render(AREA, buf, ctx, ctx.theme.canvas);
        hits = ctx.hits.clone();
    });
    (buf, hits)
}

fn thumb_start(nav: &mut NavList, x: u16) -> u16 {
    let (buf, _) = render(nav);
    (0..AREA.height)
        .find(|&y| buf[(x, y)].symbol() == scrollbar::THUMB)
        .expect("a thumb")
}

fn mouse(nav: &mut NavList, phase: Phase, pos: Position) {
    let (_, hits) = render(nav);
    let id = scrollbar::id_for(ID);
    let ev = PointerEvent::new(&hits, phase, id, pos, 1, KeyModifiers::NONE);
    nav.route(&ev);
}

#[test]
fn a_press_off_centre_on_the_nav_thumb_drags_it_cell_for_cell() {
    let mut nav = nav();
    let x = AREA.right() - 1;
    let start = thumb_start(&mut nav, x);
    let (buf, _) = render(&mut nav);
    let len = (0..AREA.height)
        .filter(|&y| buf[(x, y)].symbol() == scrollbar::THUMB)
        .count();
    assert!(len >= 3, "the thumb is long enough to grab off-centre");
    let grab = start + 1;

    mouse(&mut nav, Phase::Press, Position::new(x, grab));
    assert_eq!(
        thumb_start(&mut nav, x),
        start,
        "a press on the thumb does not move it"
    );

    mouse(&mut nav, Phase::Drag, Position::new(x, grab + 1));
    assert_eq!(
        thumb_start(&mut nav, x),
        start + 1,
        "one cell of drag moves the thumb one cell"
    );
    mouse(&mut nav, Phase::DragEnd, Position::new(x, grab + 1));
}

const SCREEN: Rect = Rect::new(0, 0, 40, 10);

fn long_nav(collapsed: bool) -> NavList {
    let items = vec![
        NavItem::new("1234567890")
            .section("A long section heading")
            .icon("界")
            .badge("99+ unread"),
        NavItem::new("e\u{301}e\u{301}e\u{301}e\u{301}界界界").icon("●"),
        NavItem::new("plain").badge("7").disabled(true),
        NavItem::new("zz").section("Another heading"),
    ];
    NavList::new(ID, items).collapsed(collapsed)
}

#[test]
fn navlist_text_never_overwrites_neighbor() {
    for collapsed in [false, true] {
        for w in 0..=12u16 {
            for (x, h) in [(5u16, 3u16), (0, 8), (35, 4)] {
                let owner = Rect::new(x, 2, w, h);
                let mut nav = long_nav(collapsed);
                let p = paint_in(SCREEN, Some(ID), |buf, ctx| {
                    nav.render(owner, buf, ctx, Color::Reset);
                });
                let what = format!("collapsed {collapsed} owner {owner:?}");
                assert_outside_untouched(&p.buf, owner, &what);
                assert_hits_inside(&p, owner.intersection(SCREEN), &what);
            }
        }
    }
}

#[test]
fn navlist_zero_surface_has_no_divider_or_focus() {
    let mut nav = long_nav(false);
    let wide = Rect::new(5, 2, 12, 2);
    paint_in(SCREEN, Some(ID), |buf, ctx| {
        nav.render(wide, buf, ctx, Color::Reset)
    });
    assert!(
        nav.scroll.viewport_len > 0,
        "the wide render sized the viewport"
    );
    for owner in [
        Rect::new(5, 2, 0, 3),
        Rect::new(5, 2, 12, 0),
        Rect::new(60, 2, 12, 3),
    ] {
        let p = paint_in(SCREEN, Some(ID), |buf, ctx| {
            nav.render(owner, buf, ctx, Color::Reset)
        });
        assert_outside_untouched(&p.buf, Rect::ZERO, &format!("{owner:?}"));
        assert!(p.hits.regions().is_empty(), "{owner:?}: no hit region");
        assert!(p.hits.bars().is_empty(), "{owner:?}: no scrollbar");
        assert!(p.ring.is_empty(), "{owner:?}: no focus stop");
        assert_eq!(
            nav.scroll.viewport_len, 0,
            "{owner:?}: the old viewport is gone"
        );
    }
}

#[test]
fn navlist_badge_stays_after_row_origin() {
    let items = vec![NavItem::new("name").badge("0123456789abcdef")];
    for w in 3..=10u16 {
        let owner = Rect::new(10, 2, w, 1);
        let mut nav = NavList::new(ID, items.clone());
        let p = paint_in(SCREEN, None, |buf, ctx| {
            nav.render(owner, buf, ctx, Color::Reset)
        });
        assert_outside_untouched(&p.buf, owner, &format!("badge width {w}"));
        let text = (owner.x..owner.right())
            .map(|x| p.buf[(x, 2)].symbol().to_owned())
            .collect::<String>();
        if w >= 8 {
            assert!(text.contains('0') || text.contains('…'), "{w}: {text:?}");
        }
    }
}

#[test]
fn navigation_wide_ascii_layout_is_compatible() {
    let owner = Rect::new(2, 1, 24, 6);
    let mut nav = NavList::new(
        ID,
        vec![
            NavItem::new("Inbox").section("Mail").icon("i").badge("12"),
            NavItem::new("Sent").icon("s"),
        ],
    );
    let p = paint_in(SCREEN, Some(ID), |buf, ctx| {
        nav.render(owner, buf, ctx, Color::Reset)
    });
    let at = |x: u16, y: u16| p.buf[(x, y)].symbol().to_owned();
    assert_eq!(at(owner.x + 3, owner.y), "M", "section heading column");
    assert_eq!(at(owner.x + 3, owner.y + 1), "i", "icon column");
    assert_eq!(at(owner.x + 5, owner.y + 1), "I", "label column");
    assert_eq!(
        at(owner.right() - 4, owner.y + 1),
        "1",
        "badge leaves a pad before the divider"
    );
    assert_eq!(at(owner.right() - 3, owner.y + 1), "2");
    assert_eq!(at(owner.x + 1, owner.y + 1), "›", "current marker");
    let row = Rect::new(owner.x, owner.y + 1, owner.width - 1, 1);
    assert!(
        p.hits.regions().iter().any(|r| r.area == row),
        "the item hit covers the whole row"
    );
}

fn rail_label(buf: &Buffer, x: u16, from: u16, to: u16) -> Vec<String> {
    (from..to)
        .map(|y| buf[(x, y)].symbol().to_owned())
        .collect()
}

#[test]
fn rail_combining_cluster_stays_on_one_row() {
    let rail = SidebarRail::new(WidgetId::of("t.rail"), "e\u{301}x");
    let col = Rect::new(5, 2, 1, 8);
    let p = paint_in(SCREEN, None, |buf, ctx| {
        rail.render_rail(col, buf, ctx, Color::Reset)
    });
    assert_outside_untouched(&p.buf, col, "rail");
    assert_eq!(rail_label(&p.buf, 5, 4, 6), ["e\u{301}", "x"]);
}

#[test]
fn rail_wide_cluster_uses_one_cell_fallback() {
    for wide in ["界", "👨\u{200d}👩\u{200d}👧"] {
        let rail = SidebarRail::new(WidgetId::of("t.rail"), &format!("{wide}a{wide}b"));
        let col = Rect::new(5, 2, 1, 8);
        let p = paint_in(SCREEN, None, |buf, ctx| {
            rail.render_rail(col, buf, ctx, Color::Reset)
        });
        assert_outside_untouched(&p.buf, col, wide);
        assert_eq!(rail_label(&p.buf, 5, 4, 8), ["…", "a", "…", "b"], "{wide}");
    }
}

#[test]
fn rail_collapse_and_custom_glyphs_are_bounded() {
    let mut theme = Theme::for_level(ColorLevel::TrueColor);
    theme.glyphs.overflow_left = "界";
    theme.glyphs.overflow_right = "界";
    let rail = SidebarRail::new(WidgetId::of("t.rail"), "ab");
    let panel = Rect::new(20, 2, 14, 3);
    let plate = Rect::new(panel.right() - 5, panel.y, 3, 1);
    let p = paint_with(&theme, SCREEN, None, |buf, ctx| {
        rail.render_collapse(panel, buf, ctx, Color::Reset);
    });
    assert_outside_untouched(&p.buf, plate, "collapse plate");
    assert_hits_inside(&p, plate, "collapse plate");
    assert_eq!(p.hits.regions().len(), 1);

    for clipped in [Rect::new(30, 2, 14, 3), Rect::new(100, 2, 20, 3)] {
        let p = paint_with(&theme, SCREEN, None, |buf, ctx| {
            rail.render_collapse(clipped, buf, ctx, Color::Reset);
        });
        assert_outside_untouched(&p.buf, Rect::ZERO, &format!("{clipped:?}"));
        assert!(p.hits.regions().is_empty(), "{clipped:?}");
    }

    let col = Rect::new(5, 2, 1, 8);
    let p = paint_with(&theme, SCREEN, None, |buf, ctx| {
        rail.render_rail(col, buf, ctx, Color::Reset);
    });
    assert_outside_untouched(&p.buf, col, "custom top glyph in a one-cell rail");
}

/// Paints a rail whose labels outrun `col` and checks the labels stop at the
/// column's bottom edge: only `expected` lands in the column, nothing paints
/// or registers a hit outside it.
fn assert_rail_stops_at_bottom(screen: Rect, col: Rect, expected: &[&str]) {
    let rail = SidebarRail::new(WidgetId::of("t.rail"), "abcdefghijkl");
    let p = paint_in(screen, None, |buf, ctx| {
        rail.render_rail(col, buf, ctx, Color::Reset)
    });
    let what = format!("rail {col:?} in {screen:?}");
    assert_outside_untouched(&p.buf, col, &what);
    assert_hits_inside(&p, col, &what);
    assert_eq!(
        rail_label(&p.buf, col.x, col.y.saturating_add(2), col.bottom()),
        expected,
        "{what}: the labels stop at the bottom edge"
    );
}

#[test]
fn rail_labels_stop_at_allocation_bottom_above_screen_edge() {
    assert_rail_stops_at_bottom(SCREEN, Rect::new(5, 5, 1, 3), &["a"]);
}

#[test]
fn rail_labels_stop_at_allocation_bottom_on_screen_edge() {
    assert_rail_stops_at_bottom(SCREEN, Rect::new(5, 7, 1, 3), &["a"]);
}

#[test]
fn rail_labels_stop_at_allocation_bottom_near_u16_max() {
    let screen = Rect::new(0, u16::MAX - 5, 4, 5);
    assert_rail_stops_at_bottom(screen, Rect::new(0, u16::MAX - 5, 1, 3), &["a"]);
}

#[test]
fn rail_row_at_u16_max_paints_nothing_and_does_not_overflow() {
    let screen = Rect::new(0, u16::MAX, 4, 1);
    assert_rail_stops_at_bottom(screen, Rect::new(0, u16::MAX, 1, 1), &[]);
}
