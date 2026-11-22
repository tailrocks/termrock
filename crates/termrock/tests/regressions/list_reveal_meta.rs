use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use termrock::core::event::Outcome;
use termrock::core::id::WidgetId;
use termrock::widgets::list::{ListBox, ListItem, SelectMode};

use crate::common::{key, paint_in};

const ID: WidgetId = WidgetId::of("test.list.reveal_meta");
const TITLE: &str = "OpenRouter · Synthetic standard key · Free requests";
const META: &str = "27 requests remaining";

fn list() -> ListBox {
    ListBox::new(
        ID,
        vec![ListItem::new(TITLE).meta(META)],
        SelectMode::Single,
    )
}

fn paint(list: &mut ListBox, cols: u16, hover: bool) -> Buffer {
    let screen = Rect::new(0, 0, cols, 24);
    paint_in(screen, (!hover).then_some(ID), |buf, ctx| {
        if hover {
            ctx.interaction.hover = Some(list.row_id(0));
            ctx.interaction.hover_ms = Some(500);
        }
        list.render(Rect::new(2, 3, cols - 6, 6), buf, ctx, ctx.theme.canvas);
    })
    .buf
}

fn row(buf: &Buffer) -> String {
    (2..buf.area.width - 4)
        .map(|x| buf[(x, 3)].symbol())
        .collect()
}

fn assert_revealed(buf: &Buffer) {
    let shown = row(buf);
    let (_, after) = shown.split_once(TITLE).expect("full title is visible");
    assert!(
        after.trim().is_empty(),
        "a tooltip must cover intersecting metadata completely: {shown:?}"
    );
}

#[test]
fn keyboard_reveal_never_leaves_a_metadata_fragment_and_close_restores_the_value() {
    for close in [KeyCode::Left, KeyCode::Esc] {
        let mut list = list();
        let before = row(&paint(&mut list, 80, false));
        assert!(before.contains(META), "{before}");
        assert_eq!(list.on_key(&key(KeyCode::Right)), Outcome::Changed);
        assert_revealed(&paint(&mut list, 80, false));
        assert_eq!(list.on_key(&key(close)), Outcome::Changed);
        assert_eq!(row(&paint(&mut list, 80, false)), before);
    }
}

#[test]
fn hover_reveal_never_leaves_a_metadata_fragment() {
    let mut list = list();
    assert_revealed(&paint(&mut list, 80, true));
}

#[test]
fn a_wide_row_keeps_its_complete_title_and_metadata() {
    let mut list = list();
    let before = row(&paint(&mut list, 140, false));
    assert!(before.contains(TITLE) && before.contains(META), "{before}");
    assert_eq!(list.on_key(&key(KeyCode::Right)), Outcome::Ignored);
    let hovered = row(&paint(&mut list, 140, true));
    assert!(
        hovered.contains(TITLE) && hovered.contains(META),
        "{hovered}"
    );
}
