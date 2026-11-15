use crate::common::*;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::toolbar::{ToolItem, Toolbar};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

const BAR: WidgetId = WidgetId::of("hidden_cursor.toolbar");
const LABELS: [&str; 5] = ["Run", "Stop", "Format", "Explain", "Share"];

fn bar(overflow: bool) -> Toolbar {
    let items = LABELS.iter().map(|label| ToolItem::new(label)).collect();
    Toolbar::new(BAR, items).overflow(overflow)
}

fn render(tb: &mut Toolbar, width: u16) -> Buffer {
    draw(Some(BAR), |buf, ctx| {
        tb.render(Rect::new(0, 0, width, 1), buf, ctx, ctx.theme.canvas);
    })
    .0
}

fn focus_bars(buf: &Buffer, width: u16) -> usize {
    let bar = Theme::for_level(ColorLevel::TrueColor).glyphs.focus_bar;
    (0..width).filter(|&x| buf[(x, 0)].symbol() == bar).count()
}

#[test]
fn with_overflow_off_a_cursor_on_a_narrowed_away_item_moves_to_the_last_drawn_one() {
    let mut tb = bar(false);
    let wide = render(&mut tb, 60);
    tb.cursor = 4;
    assert_eq!(focus_bars(&wide, 60), 1, "the wide bar marks its cursor");
    render(&mut tb, 60);
    assert_eq!(tb.cursor, 4, "a drawn cursor does not move");

    let narrow = render(&mut tb, 30);

    assert!(
        tb.cursor < 4,
        "the cursor left the hidden item: {}",
        tb.cursor
    );
    assert!(
        !tb.areas[tb.cursor].is_empty(),
        "the cursor sits on a drawn item: {:?}",
        row(&narrow, 0)
    );
    assert_eq!(
        focus_bars(&narrow, 30),
        1,
        "the cursor item carries the focus mark: {:?}",
        row(&narrow, 0)
    );
}

#[test]
fn with_overflow_off_the_cursor_skips_a_disabled_item_when_it_moves() {
    let items = vec![
        ToolItem::new("Run"),
        ToolItem::new("Stop").disabled(true),
        ToolItem::new("Format"),
        ToolItem::new("Explain"),
    ];
    let mut tb = Toolbar::new(BAR, items).overflow(false);
    tb.cursor = 3;

    render(&mut tb, 20);

    assert!(
        tb.areas[1].width > 0 && tb.areas[2].width == 0,
        "Stop is the last drawn"
    );
    assert_eq!(tb.cursor, 0, "the nearest enabled drawn item");
}

#[test]
fn with_overflow_on_the_more_button_keeps_standing_in_for_the_cursor() {
    let mut tb = bar(true);
    tb.cursor = 4;
    let narrow = render(&mut tb, 30);

    assert_eq!(tb.cursor, 5, "the cursor rests on the More stop");
    assert_eq!(focus_bars(&narrow, 30), 1);
}

#[test]
fn a_focused_bar_with_overflow_off_marks_its_cursor_at_every_width() {
    // Below the first item's width the bar draws nothing to mark.
    for width in 8..=60 {
        for start in 0..LABELS.len() {
            let mut tb = bar(false);
            tb.cursor = start;
            let buf = render(&mut tb, width);
            assert_eq!(
                focus_bars(&buf, width),
                1,
                "width {width}, cursor {start}: {:?}",
                row(&buf, 0)
            );
        }
    }
}
