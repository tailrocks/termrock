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

#[test]
fn expanding_a_background_never_moves_the_revealed_text_origin() {
    use termrock::ui::reveal::{self, Reveal, RevealState};
    let title = "A synthetic clipped title";
    let mut state = RevealState::new(Some(Reveal::default()));
    let mut frame = || {
        paint_in(Rect::new(0, 0, 80, 24), None, |_, ctx| {
            state.begin(ctx);
            reveal::occlude(Rect::new(4, 3, 15, 4));
            state.cell(ctx, ID, Rect::new(12, 5, 6, 1), title, true);
        })
        .buf
    };
    frame();
    state.on_key(&key(KeyCode::Right));
    let shown = paint_in(Rect::new(0, 0, 80, 24), None, |_, ctx| {
        state.begin(ctx);
        reveal::occlude(Rect::new(4, 3, 15, 4));
        state.cell(ctx, ID, Rect::new(12, 5, 6, 1), title, true);
    })
    .buf;
    let anchored: String = (12..12 + title.len() as u16)
        .map(|x| shown[(x, 5)].symbol())
        .collect();
    assert_eq!(anchored, title);
}

#[test]
fn hidden_metadata_never_erases_ui_outside_a_horizontal_viewport() {
    use ratatui::layout::Size;
    use termrock::ui::scroll_view::{Axes, ScrollView};
    for meta in [META, "27 requests remaining until the next daily reset"] {
        for clipped in [false, true] {
            let mut list = ListBox::new(
                ID,
                vec![ListItem::new(TITLE).meta(meta)],
                SelectMode::Single,
            );
            let mut view = ScrollView::new(ID.sub("view"))
                .axes(Axes::HORIZONTAL)
                .follow_focus(false);
            let mut frame = |list: &mut ListBox| {
                paint_in(Rect::new(0, 0, 80, 24), Some(ID), |buf, ctx| {
                    buf[(70, 3)].set_char('Z');
                    if clipped {
                        ctx.clip(buf, Rect::new(2, 3, 35, 8), |buf, ctx| {
                            list.render(Rect::new(2, 3, 74, 6), buf, ctx, ctx.theme.canvas);
                        });
                    } else {
                        view.render(
                            Rect::new(2, 3, 35, 8),
                            Size::new(74, 6),
                            buf,
                            ctx,
                            |canvas, buf, ctx| list.render(canvas, buf, ctx, ctx.theme.canvas),
                        );
                    }
                })
                .buf
            };
            assert_eq!(frame(&mut list)[(70, 3)].symbol(), "Z");
            assert_eq!(list.on_key(&key(KeyCode::Right)), Outcome::Changed);
            assert_eq!(frame(&mut list)[(70, 3)].symbol(), "Z");
        }
    }
}

#[test]
fn wrapped_reveal_covers_metadata_on_every_crossed_row() {
    let title = format!("{} {}", "a".repeat(40), "b".repeat(40));
    let meta = "27 requests remaining until next reset";
    let mut list = ListBox::new(
        ID,
        vec![ListItem::new(&title), ListItem::new("next row").meta(meta)],
        SelectMode::Single,
    );
    let before = paint(&mut list, 80, false);
    let line = |buf: &Buffer| -> String { (2..76).map(|x| buf[(x, 4)].symbol()).collect() };
    assert!(line(&before).contains(meta));
    assert_eq!(list.on_key(&key(KeyCode::Right)), Outcome::Changed);
    let opened = line(&paint(&mut list, 80, false));
    let (_, tail) = opened
        .split_once(&"b".repeat(40))
        .expect("second title line");
    assert!(tail.trim().is_empty(), "{opened}");
    list.on_key(&key(KeyCode::Left));
    assert!(line(&paint(&mut list, 80, false)).contains(meta));
}

#[test]
fn metadata_registered_inside_an_overlay_batch_is_covered() {
    let mut list = list();
    paint(&mut list, 80, false);
    list.on_key(&key(KeyCode::Right));
    let shown = paint_in(Rect::new(0, 0, 80, 24), Some(ID), move |_, ctx| {
        ctx.overlay(move |buf, ctx| {
            list.render(Rect::new(2, 3, 74, 6), buf, ctx, ctx.theme.canvas);
        });
    })
    .buf;
    assert_revealed(&shown);
}

#[test]
fn protected_metadata_never_leaks_into_a_new_layer_or_frame() {
    use termrock::ui::reveal::{Reveal, RevealState};
    let mut list = list();
    paint(&mut list, 80, false);
    list.on_key(&key(KeyCode::Right));
    let next = "m".repeat(55);
    let shown = paint_in(Rect::new(0, 0, 80, 24), Some(ID), move |buf, ctx| {
        ctx.overlay(move |buf, ctx| {
            list.render(Rect::new(2, 3, 74, 6), buf, ctx, ctx.theme.canvas);
        });
        ctx.begin_modal(buf);
        buf[(70, 3)].set_char('Z');
        let mut state = RevealState::new(Some(Reveal::default()));
        state.open = true;
        state.begin(ctx);
        state.cell(ctx, ID.sub("next"), Rect::new(5, 3, 6, 1), &next, true);
    })
    .buf;
    assert_eq!(shown[(70, 3)].symbol(), "Z");

    let shown = paint_in(Rect::new(0, 0, 80, 24), None, |buf, ctx| {
        buf[(70, 3)].set_char('Z');
        let mut state = RevealState::new(Some(Reveal::default()));
        state.open = true;
        state.begin(ctx);
        state.cell(
            ctx,
            ID.sub("frame"),
            Rect::new(5, 3, 6, 1),
            &"n".repeat(55),
            true,
        );
    })
    .buf;
    assert_eq!(shown[(70, 3)].symbol(), "Z");
}

#[test]
fn clipping_marks_survive_a_repainted_cut_label() {
    use termrock::ui::reveal;
    let mut list = list();
    paint(&mut list, 80, false);
    list.on_key(&key(KeyCode::Right));
    let shown = paint_in(Rect::new(0, 0, 80, 24), Some(ID), |buf, ctx| {
        let cut = Rect::new(0, 0, 1, 1);
        buf.set_string(
            0,
            0,
            reveal::cut(cut, "outer text"),
            ratatui::style::Style::default(),
        );
        buf[(70, 3)].set_char('Z');
        ctx.clip(buf, Rect::new(2, 3, 35, 8), |buf, ctx| {
            list.render(Rect::new(2, 3, 74, 6), buf, ctx, ctx.theme.canvas);
            buf.set_string(
                0,
                0,
                reveal::cut(cut, "inner text"),
                ratatui::style::Style::default(),
            );
        });
    })
    .buf;
    assert_eq!(shown[(70, 3)].symbol(), "Z");
}
