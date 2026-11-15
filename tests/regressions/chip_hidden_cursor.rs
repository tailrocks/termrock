use crate::common::*;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::chips::{Chip, ChipBar};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;

const CHIPS: WidgetId = WidgetId::of("hidden_cursor.chips");
const WIDTH: u16 = 24;

fn bar() -> ChipBar {
    let mut cb = ChipBar::new(CHIPS);
    cb.set_chips(
        ["alpha", "bravo", "charlie", "delta", "echo"]
            .iter()
            .map(|label| Chip::new(label))
            .collect(),
    );
    cb.set_add_label(None);
    cb
}

fn render(cb: &mut ChipBar, focus: Option<WidgetId>) -> Buffer {
    draw(focus, |buf, ctx| {
        cb.render(Rect::new(0, 0, WIDTH, 1), buf, ctx, ctx.theme.canvas);
    })
    .0
}

fn focus_bars(buf: &Buffer) -> usize {
    let bar = Theme::for_level(ColorLevel::TrueColor).glyphs.focus_bar;
    (0..WIDTH).filter(|&x| buf[(x, 0)].symbol() == bar).count()
}

#[test]
fn a_cursor_on_a_hidden_chip_marks_the_overflow_marker() {
    let mut cb = bar();
    let rest = render(&mut cb, Some(CHIPS));
    let row0 = row(&rest, 0);
    assert!(row0.contains('+'), "the bar overflows: {row0:?}");
    assert_eq!(focus_bars(&rest), 1, "the first chip carries the cursor");

    for _ in 0..4 {
        cb.on_key(&key(KeyCode::Right));
    }
    assert_eq!(cb.cursor, 4, "the cursor reached the last, hidden chip");
    let hidden = render(&mut cb, Some(CHIPS));

    assert_eq!(
        focus_bars(&hidden),
        1,
        "a hidden cursor chip must still show a focus mark: {:?}",
        row(&hidden, 0)
    );
    let marker = (0..WIDTH)
        .find(|&x| hidden[(x, 0)].symbol() == "+")
        .unwrap();
    let bar = Theme::for_level(ColorLevel::TrueColor).glyphs.focus_bar;
    assert_eq!(
        hidden[(marker - 1, 0)].symbol(),
        bar,
        "the focus mark sits on the overflow marker"
    );
}

#[test]
fn an_unfocused_bar_draws_no_mark_for_a_hidden_cursor() {
    let mut cb = bar();
    render(&mut cb, Some(CHIPS));
    for _ in 0..4 {
        cb.on_key(&key(KeyCode::Right));
    }
    let hidden = render(&mut cb, None);

    assert_eq!(focus_bars(&hidden), 0, "no focus, no mark");
}
