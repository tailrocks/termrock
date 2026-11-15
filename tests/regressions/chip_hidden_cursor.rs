use crate::common::*;
use junie_tui::core::event::Outcome;
use junie_tui::core::id::WidgetId;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::chips::{Chip, ChipAction, ChipBar};
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

fn add_bar(marker: bool, add: bool) -> ChipBar {
    let mut cb = bar().overflow_marker(marker);
    cb.set_add_label(add.then(|| "+ Add".to_owned()));
    cb
}

#[test]
fn with_the_marker_off_a_resize_that_hides_the_cursor_chip_moves_it_to_the_last_shown_chip() {
    let mut cb = add_bar(false, false);
    render(&mut cb, Some(CHIPS));
    cb.cursor = 4;

    let moved = render(&mut cb, Some(CHIPS));

    assert!(
        cb.cursor < 4,
        "the cursor left the hidden chip: {}",
        cb.cursor
    );
    assert_eq!(
        focus_bars(&moved),
        1,
        "the cursor is on a drawn chip: {:?}",
        row(&moved, 0)
    );
    let bar = Theme::for_level(ColorLevel::TrueColor).glyphs.focus_bar;
    let label = ["alpha", "bravo", "charlie", "delta", "echo"][cb.cursor];
    let at = row(&moved, 0)
        .find(label)
        .expect("the cursor chip is drawn");
    let x = row(&moved, 0)[..at].chars().count() as u16;
    assert_eq!(moved[(x - 1, 0)].symbol(), bar, "the mark sits on {label}");
}

#[test]
fn with_the_marker_off_a_visible_cursor_stays_put() {
    let mut cb = add_bar(false, false);
    cb.cursor = 1;
    render(&mut cb, Some(CHIPS));

    assert_eq!(cb.cursor, 1);
}

#[test]
fn with_the_marker_off_a_resize_that_hides_the_cursor_chip_moves_it_to_a_drawn_add_button() {
    let mut cb = add_bar(false, true);
    cb.cursor = 4;
    let buf = draw(Some(CHIPS), |buf, ctx| {
        cb.render(Rect::new(0, 0, 20, 1), buf, ctx, ctx.theme.canvas);
    })
    .0;

    assert_eq!(cb.cursor, 5, "the add button is the nearest drawn stop");
    assert_eq!(focus_bars(&buf), 1, "{:?}", row(&buf, 0));
    let bar = Theme::for_level(ColorLevel::TrueColor).glyphs.focus_bar;
    let add = row(&buf, 0).find("+ Add").expect("the add button is drawn");
    let x = row(&buf, 0)[..add].chars().count() as u16;
    assert_eq!(
        buf[(x - 1, 0)].symbol(),
        bar,
        "the mark sits on the add button"
    );
}

#[test]
fn a_focused_bar_always_marks_its_cursor_at_every_width_with_the_marker_off() {
    // Below these widths the bar draws no stop at all, so none can be marked.
    for (add, narrowest) in [(false, 13), (true, 10)] {
        for w in narrowest..=WIDTH {
            for start in 0..5 {
                let mut cb = add_bar(false, add);
                cb.cursor = start;
                let buf = draw(Some(CHIPS), |buf, ctx| {
                    cb.render(Rect::new(0, 0, w, 1), buf, ctx, ctx.theme.canvas);
                })
                .0;
                let bar = Theme::for_level(ColorLevel::TrueColor).glyphs.focus_bar;
                let marks = (0..w).filter(|&x| buf[(x, 0)].symbol() == bar).count();
                assert_eq!(
                    marks,
                    1,
                    "add {add}, width {w}, cursor {start}: {:?}",
                    row(&buf, 0)
                );
            }
        }
    }
}

const NAMES: [&str; 5] = ["alpha", "bravo", "charlie", "delta", "echo"];

fn render_at(cb: &mut ChipBar, w: u16) -> Buffer {
    draw(Some(CHIPS), |buf, ctx| {
        cb.render(Rect::new(0, 0, w, 1), buf, ctx, ctx.theme.canvas);
    })
    .0
}

/// How many chips the bar drew, and whether it drew the add button.
fn drawn(buf: &Buffer) -> (usize, bool) {
    let r = row(buf, 0);
    (
        NAMES.iter().filter(|n| r.contains(**n)).count(),
        r.contains("+ Add"),
    )
}

#[test]
fn left_from_a_drawn_add_button_reaches_the_last_shown_chip() {
    let mut reached = 0;
    for w in 14..=WIDTH + 20 {
        let mut cb = add_bar(false, true);
        let (shown, add) = drawn(&render_at(&mut cb, w));
        if !add || NAMES.len() - shown < 2 {
            continue;
        }
        reached += 1;
        cb.cursor = NAMES.len();
        cb.on_key(&key(KeyCode::Left));
        let buf = render_at(&mut cb, w);
        assert_eq!(cb.cursor, shown - 1, "width {w}: {:?}", row(&buf, 0));
        assert_eq!(focus_bars(&buf), 1, "width {w}: {:?}", row(&buf, 0));
        cb.on_key(&key(KeyCode::Right));
        assert_eq!(cb.cursor, NAMES.len(), "width {w}: Right returns to add");
    }
    assert!(
        reached > 0,
        "no width drew the add button with 2+ hidden chips"
    );
}

#[test]
fn end_with_an_add_button_that_does_not_fit_lands_on_the_last_drawn_chip() {
    let mut reached = 0;
    for w in 13..=WIDTH + 20 {
        let mut cb = add_bar(false, true);
        let (shown, add) = drawn(&render_at(&mut cb, w));
        if add || shown == 0 || shown == NAMES.len() {
            continue;
        }
        reached += 1;
        cb.on_key(&key(KeyCode::End));
        assert_eq!(cb.cursor, shown - 1, "width {w}");
        let buf = render_at(&mut cb, w);
        assert_eq!(cb.cursor, shown - 1, "width {w}: render keeps it");
        assert_eq!(focus_bars(&buf), 1, "width {w}: {:?}", row(&buf, 0));
        let (_, ev) = cb.on_key(&key(KeyCode::Enter));
        assert_eq!(ev, Some(ChipAction::Activate(shown - 1)), "width {w}");
    }
    assert!(reached > 0, "no width left the add button out");
}

#[test]
fn batched_rights_with_no_render_between_never_activate_a_hidden_chip() {
    let mut cb = add_bar(false, false);
    let (shown, _) = drawn(&render(&mut cb, Some(CHIPS)));
    assert!(shown < NAMES.len(), "the bar overflows");

    for _ in 0..8 {
        cb.on_key(&key(KeyCode::Right));
    }
    let (_, ev) = cb.on_key(&key(KeyCode::Enter));

    assert_eq!(ev, Some(ChipAction::Activate(shown - 1)));
}

#[test]
fn keys_before_the_first_render_do_not_reach_a_hidden_chip() {
    let mut cb = add_bar(false, false);
    for _ in 0..8 {
        let (outcome, _) = cb.on_key(&key(KeyCode::Right));
        assert_eq!(
            outcome,
            Outcome::Ignored,
            "nothing is drawn, so Right is unhandled"
        );
    }
    assert_eq!(cb.cursor, 0, "nothing is known to be drawn yet");
    let (_, ev) = cb.on_key(&key(KeyCode::Enter));
    assert_eq!(ev, Some(ChipAction::Activate(0)));

    let (shown, _) = drawn(&render(&mut cb, Some(CHIPS)));
    assert!(shown < NAMES.len(), "the bar overflows");
    for _ in 0..8 {
        cb.on_key(&key(KeyCode::Right));
    }
    let (_, ev) = cb.on_key(&key(KeyCode::Enter));
    assert_eq!(ev, Some(ChipAction::Activate(shown - 1)));
}

#[test]
fn a_lead_set_after_a_render_keeps_navigation_off_the_hidden_chips() {
    let mut cb = add_bar(false, false);
    let (shown, _) = drawn(&render(&mut cb, Some(CHIPS)));
    assert!(shown < NAMES.len(), "the bar overflows");

    assert!(cb.set_lead(Some("Lead".to_owned())));
    for _ in 0..8 {
        let (outcome, _) = cb.on_key(&key(KeyCode::Right));
        assert_eq!(
            outcome,
            Outcome::Ignored,
            "no render has laid out the new lead"
        );
    }
    assert_eq!(cb.cursor, 0, "the cursor stays on a chip that is drawn");
    let (_, ev) = cb.on_key(&key(KeyCode::Enter));
    assert_eq!(ev, Some(ChipAction::Activate(0)));
}
