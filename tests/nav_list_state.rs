mod common;

use common::*;
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::widgets::nav_list::{NavAction, NavItem, NavList};
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

const ID: WidgetId = WidgetId::of("t.nav.state");
const AREA: Rect = Rect::new(0, 0, 30, 8);

fn nav(n: usize) -> NavList {
    NavList::new(
        ID,
        (0..n)
            .map(|i| NavItem::new(&format!("item {i:02}")))
            .collect(),
    )
}

fn render(nav: &mut NavList) {
    draw(None, |buf, ctx| {
        nav.render(AREA, buf, ctx, ctx.theme.canvas);
    });
}

#[test]
fn navlist_shrink_before_enter_is_safe() {
    let mut nav = nav(3);
    nav.on_key(&key(KeyCode::Down));
    nav.on_key(&key(KeyCode::Down));
    assert_eq!(nav.cursor, 2);

    nav.items.truncate(1);
    let (_, action) = nav.on_key(&key(KeyCode::Enter));

    assert_eq!(action, Some(NavAction::Activated(0)));
    assert_eq!((nav.cursor, nav.current), (0, 0));
}

#[test]
fn navlist_shrink_before_up_is_safe_and_repairs_the_cursor() {
    let mut nav = nav(3);
    nav.on_key(&key(KeyCode::Down));
    nav.on_key(&key(KeyCode::Down));

    nav.items.truncate(1);
    let (outcome, action) = nav.on_key(&key(KeyCode::Up));

    assert_eq!(action, None);
    assert_eq!(nav.cursor, 0);
    assert_eq!(
        outcome,
        Outcome::Changed,
        "a repaired index is a state change"
    );
}

#[test]
fn navlist_render_reconciles_before_keyboard() {
    let mut nav = nav(3);
    nav.on_key(&key(KeyCode::Down));
    nav.on_key(&key(KeyCode::Down));
    render(&mut nav);

    nav.items.truncate(1);
    render(&mut nav);
    let (_, action) = nav.on_key(&key(KeyCode::Enter));

    assert_eq!(action, Some(NavAction::Activated(0)));
}

#[test]
fn navlist_public_max_cursor_is_safe() {
    let mut nav = nav(3);
    nav.cursor = usize::MAX;
    nav.current = usize::MAX;

    let (outcome, _) = nav.on_key(&key(KeyCode::Down));
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(nav.cursor, 2);

    let (_, action) = nav.on_key(&key(KeyCode::Enter));
    assert_eq!(action, Some(NavAction::Activated(2)));
    assert_eq!(nav.current, 2);
}

#[test]
fn navlist_empty_source_repairs_stale_indices_and_emits_nothing() {
    let mut nav = NavList::new(ID, vec![]);
    nav.cursor = 3;
    nav.current = 3;

    let (outcome, action) = nav.on_key(&key(KeyCode::Enter));

    assert_eq!(action, None);
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!((nav.cursor, nav.current), (0, 0));
}

#[test]
fn navlist_all_disabled_source_emits_nothing() {
    let mut nav = NavList::new(
        ID,
        vec![
            NavItem::new("a").disabled(true),
            NavItem::new("b").disabled(true),
        ],
    );

    assert_eq!(nav.on_key(&key(KeyCode::Enter)).1, None);
    assert_eq!(nav.on_key(&key(KeyCode::Down)).1, None);
    render(&mut nav);
    assert_eq!(nav.on_key(&key(KeyCode::Enter)).1, None);
    assert_eq!(nav.current, 0);
}

#[test]
fn navlist_boundary_navigation_is_a_consumed_noop() {
    let mut nav = nav(3);
    nav.on_key(&key(KeyCode::End));
    render(&mut nav);

    let (outcome, action) = nav.on_key(&key(KeyCode::Down));

    assert_eq!(action, None);
    assert_eq!(nav.cursor, 2);
    assert_eq!(outcome, Outcome::Consumed);
}

#[test]
fn navlist_boundary_navigation_reveals_a_scrolled_away_cursor() {
    let mut nav = nav(12);
    nav.on_key(&key(KeyCode::End));
    render(&mut nav);
    nav.scroll.scroll_to(0);
    render(&mut nav);
    assert_eq!(nav.scroll.offset, 0);

    let (outcome, _) = nav.on_key(&key(KeyCode::End));

    assert_eq!(nav.cursor, 11);
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(nav.scroll.offset, nav.scroll.max_offset());
}

#[test]
fn navlist_set_items_repairs_indices_and_ignores_equal_replacement() {
    let mut nav = nav(3);
    nav.on_key(&key(KeyCode::Down));
    nav.on_key(&key(KeyCode::Down));
    render(&mut nav);

    assert!(nav.set_items(vec![NavItem::new("only")]));
    assert_eq!((nav.cursor, nav.current), (0, 0));

    let offset = nav.scroll.offset;
    assert!(!nav.set_items(vec![NavItem::new("only")]));
    assert_eq!(nav.scroll.offset, offset);

    let (_, action) = nav.on_key(&key(KeyCode::Enter));
    assert_eq!(action, Some(NavAction::Activated(0)));
}
