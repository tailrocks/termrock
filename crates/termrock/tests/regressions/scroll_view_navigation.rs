use super::common::*;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Rect, Size};
use termrock::core::event::{Key, Outcome};
use termrock::core::id::WidgetId;
use termrock::core::keymap::KeyBinding;
use termrock::ui::scroll_view::{Axes, ScrollCmd, ScrollView};

const ID: WidgetId = WidgetId::of("t.scroll");
const AREA: Rect = Rect::new(0, 0, 40, 4);

fn render(view: &mut ScrollView, content: Size) {
    draw(None, |buf, ctx| {
        view.render(AREA, content, buf, ctx, |_, _, _| {});
    });
}

fn press(view: &mut ScrollView, code: KeyCode) -> Option<Outcome> {
    view.on_key(&Key {
        code,
        mods: KeyModifiers::NONE,
    })
}

#[test]
fn scrollview_home_resets_horizontal_only() {
    let mut view = ScrollView::new(ID).axes(Axes::HORIZONTAL);
    render(&mut view, Size::new(100, 1));
    assert!(view.scroll_to(40, 0));
    render(&mut view, Size::new(100, 1));

    assert_eq!(press(&mut view, KeyCode::Home), Some(Outcome::Changed));
    assert_eq!(view.offset(), (0, 0));
}

#[test]
fn scrollview_home_resets_both_axes() {
    let mut view = ScrollView::new(ID).axes(Axes::BOTH);
    render(&mut view, Size::new(100, 10));
    assert!(view.scroll_to(40, 5));
    render(&mut view, Size::new(100, 10));

    assert_eq!(press(&mut view, KeyCode::Home), Some(Outcome::Changed));
    assert_eq!(view.offset(), (0, 0));
}

#[test]
fn scrollview_home_at_scrollable_origin_is_consumed() {
    let mut view = ScrollView::new(ID).axes(Axes::HORIZONTAL);
    render(&mut view, Size::new(100, 1));

    assert_eq!(press(&mut view, KeyCode::Home), Some(Outcome::Consumed));
    assert_eq!(view.offset(), (0, 0));
}

#[test]
fn scrollview_home_without_overflow_bubbles() {
    let mut view = ScrollView::new(ID).axes(Axes::BOTH);
    render(&mut view, Size::new(20, 2));

    assert_eq!(press(&mut view, KeyCode::Home), None);
}

#[test]
fn scrollview_top_remap_and_vertical_commands_keep_policy() {
    let mut view = ScrollView::new(ID).axes(Axes::HORIZONTAL).keys(vec![(
        KeyBinding::new(KeyCode::Char('g'), KeyModifiers::NONE),
        ScrollCmd::Top,
    )]);
    render(&mut view, Size::new(100, 1));
    assert!(view.scroll_to(40, 0));
    render(&mut view, Size::new(100, 1));

    assert_eq!(press(&mut view, KeyCode::Home), None);
    assert_eq!(press(&mut view, KeyCode::PageDown), None);
    assert_eq!(press(&mut view, KeyCode::End), None);
    assert_eq!(view.offset(), (40, 0));

    assert_eq!(press(&mut view, KeyCode::Char('g')), Some(Outcome::Changed));
    assert_eq!(view.offset(), (0, 0));
}
