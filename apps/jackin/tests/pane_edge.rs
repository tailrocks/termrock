//! The right edge, the cursor and the size of a terminal pane.

mod pane_support;

use jackin_console::panes::{LinkStatus, PaneBody, PaneNote};
use pane_support::*;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use termrock::core::focus::FocusRing;
use termrock::core::hit::HitRegistry;
use termrock::theme::{ColorLevel, Theme, Tone};
use termrock::ui::ctx::{Interaction, RenderCtx};

fn paint(pane: &impl PaneBody, area: Rect, caret_allowed: bool) -> (Buffer, Option<Position>) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 100, 40));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let cursor = pane
        .viewport()
        .draw(area, &mut buf, &mut ctx, theme.canvas, caret_allowed);
    (buf, cursor)
}

#[test]
fn track_column_is_always_reserved() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    let area = inner(80, 24);
    let (buf, _) = paint(&pane, area, true);
    let glyph = Theme::for_level(ColorLevel::TrueColor).glyphs.scroll_track;
    for y in 0..24 {
        assert_eq!(
            buf[(80, y)].symbol(),
            glyph,
            "row {y} of the last inner column is the track, with no history yet"
        );
    }

    bench.output(
        0,
        &(0..100)
            .flat_map(|n| format!("l{n}\r\n").into_bytes())
            .collect::<Vec<_>>(),
    );
    pane.tick(900);
    let (buf, _) = paint(&pane, area, true);
    assert!(
        (0..24).all(|y| buf[(80, y)].symbol() != "l"),
        "the track never draws over program text"
    );
}

#[test]
fn grid_width_is_constant() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    let lines: Vec<u8> = (0..100)
        .flat_map(|n| format!("l{n}\r\n").into_bytes())
        .collect();
    bench.output(0, &lines);
    pane.tick(900);
    pane.sync_size(inner(80, 24));
    assert_eq!(pane.grid().state().size(), (24, 80));
    assert_eq!(
        bench.sizes(0),
        [size(24, 80)],
        "the first history adds no size to the link"
    );
}

#[test]
fn no_bottom_bar_row() {
    let bench = Bench::new();
    let pane = live(&bench);
    assert_eq!(pane.viewport().hbar(), Rect::ZERO);
    assert_eq!(
        pane.grid().state().size().0,
        24,
        "the program gets every row"
    );
}

#[test]
fn cursor_visibility_rules() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    let area = inner(80, 24);
    bench.output(0, b"ab");
    pane.tick(900);
    assert_eq!(paint(&pane, area, true).1, Some(Position::new(2, 0)));
    assert_eq!(
        paint(&pane, area, false).1,
        None,
        "a dialog or another focus"
    );

    bench.output(0, b"\x1b[?25l");
    pane.tick(1_000);
    assert_eq!(paint(&pane, area, true).1, None, "the program hid it");
    bench.output(0, b"\x1b[?25h");
    pane.tick(1_100);

    let lines: Vec<u8> = (0..100)
        .flat_map(|n| format!("l{n}\r\n").into_bytes())
        .collect();
    bench.output(0, &lines);
    pane.tick(1_200);
    pane.viewport_mut()
        .wheel(termrock::core::hit::Axis::Vertical, -3);
    assert_eq!(paint(&pane, area, true).1, None, "scrolled back");
}

#[test]
fn below_floor_note() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    pane.sync_size(Rect::new(0, 0, 19, 24));
    assert_eq!(
        pane.note(&words()),
        Some(PaneNote {
            text: "Pane too small · zoom with Ctrl+B z".into(),
            tone: Tone::Warning
        })
    );
    assert_eq!(
        bench.sizes(0),
        [size(24, 80)],
        "the program keeps its last size"
    );
    assert!(matches!(pane.status(), LinkStatus::Attached(_)));
    assert_eq!(pane.grid().state().size(), (24, 80));

    pane.sync_size(inner(80, 24));
    assert_eq!(
        pane.note(&words()),
        None,
        "room again, and the same size: no resize"
    );
    assert_eq!(bench.sizes(0).len(), 1);
}
