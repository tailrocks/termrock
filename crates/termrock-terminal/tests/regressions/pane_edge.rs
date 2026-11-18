#![cfg(feature = "terminal")]

use junie_tui::core::focus::FocusRing;
use junie_tui::core::hit::HitRegistry;
use junie_tui::core::id::WidgetId;
use junie_tui::terminal::{TerminalState, TerminalViewport};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::ui::ctx::{Interaction, RenderCtx};
use junie_tui::widgets::scrollbar::{THUMB, TRACK};
use junie_tui::widgets::viewport_ops::ViewportOps;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

const ID: WidgetId = WidgetId::of("t.pane.edge");
const AREA: Rect = Rect::new(1, 1, 11, 10);

fn viewport(state: TerminalState) -> TerminalViewport {
    let mut v = TerminalViewport::new(ID, state);
    v.set_area(AREA);
    v
}

fn fed(bytes: &[u8]) -> TerminalViewport {
    let mut v = viewport(TerminalState::new(10, 10));
    v.feed(bytes);
    v
}

fn history() -> TerminalViewport {
    let lines: Vec<String> = (1..=100).map(|n| format!("line {n:03}")).collect();
    fed(lines.join("\r\n").as_bytes())
}

fn paint(v: &TerminalViewport, caret_allowed: bool) -> (Buffer, Option<Position>) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 14));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let cursor = v.draw(AREA, &mut buf, &mut ctx, theme.canvas, caret_allowed);
    (buf, cursor)
}

fn track(buf: &Buffer) -> Vec<String> {
    (AREA.y..AREA.bottom())
        .map(|y| buf[(AREA.right() - 1, y)].symbol().to_owned())
        .collect()
}

fn grid_row(buf: &Buffer, row: u16) -> String {
    (AREA.x..AREA.right() - 1)
        .map(|x| buf[(x, AREA.y + row)].symbol().to_owned())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn track_column_is_always_reserved() {
    let empty = fed(b"");
    let rest = track(&paint(&empty, true).0);
    assert_eq!(
        rest,
        vec![TRACK.to_owned(); 10],
        "with no history the track column is drawn at rest, without a thumb"
    );

    let long = history();
    let at_tail = track(&paint(&long, true).0);
    assert_eq!(at_tail.len(), 10);
    assert!(
        at_tail.iter().any(|s| s == THUMB) && at_tail.iter().any(|s| s == TRACK),
        "history shows a thumb on the track: {at_tail:?}"
    );
    assert_eq!(
        at_tail.last().map(String::as_str),
        Some(THUMB),
        "at the live tail the thumb sits at the bottom"
    );

    let mut scrolled = history();
    scrolled.jump_start();
    let top = track(&paint(&scrolled, true).0);
    assert_eq!(top.first().map(String::as_str), Some(THUMB));
    assert_eq!(top.last().map(String::as_str), Some(TRACK));

    let mut full = history();
    full.feed(b"\x1b[?1049h");
    assert_eq!(
        track(&paint(&full, true).0),
        vec![TRACK.to_owned(); 10],
        "the alternate screen has no history to show"
    );
}

#[test]
fn grid_width_is_constant() {
    assert_eq!(TerminalViewport::grid_size(AREA), (10, 10));
    assert_eq!(
        TerminalViewport::grid_size(Rect::new(0, 0, 81, 24)),
        (24, 80),
        "the grid is the inner width minus the track column"
    );
    assert_eq!(
        TerminalViewport::grid_size(Rect::new(0, 0, 1, 0)),
        (1, 1),
        "a degenerate area still gives a valid grid"
    );

    let mut v = viewport(TerminalState::new(10, 10));
    let size = v.state().size();
    v.feed(b"0123456789ABC");
    assert_eq!(
        grid_row(&paint(&v, true).0, 0),
        "0123456789",
        "a full row fills the grid and stops before the track"
    );
    assert_eq!(
        grid_row(&paint(&v, true).0, 1),
        "ABC",
        "the 11th column wraps"
    );

    let before = TerminalViewport::grid_size(AREA);
    for n in 0..100 {
        v.feed(format!("\r\nrow {n}").as_bytes());
    }
    assert!(v.state().scrollback_len() > 0, "history appeared");
    assert_eq!(v.state().size(), size, "the grid did not change size");
    assert_eq!(TerminalViewport::grid_size(AREA), before);
    assert_eq!(
        paint(&v, true).0[(AREA.right() - 1, AREA.y)].symbol(),
        TRACK,
        "the first history never draws over the last column of the grid"
    );
}

#[test]
fn no_bottom_bar_row() {
    let mut wide = viewport(TerminalState::new(10, 40));
    wide.feed(&b"x".repeat(40));
    assert!(wide.hbar().is_empty(), "no bottom bar for a terminal pane");
    assert_eq!(
        TerminalViewport::grid_size(AREA).0,
        AREA.height,
        "the program gets every row"
    );

    let v = history();
    let buf = paint(&v, true).0;
    assert_eq!(
        grid_row(&buf, 9),
        "line 100",
        "the last row is program text"
    );
    assert!(v.hbar().is_empty());
    let (buf, _) = paint(&wide, true);
    assert_eq!(grid_row(&buf, 0), "x".repeat(10), "wider rows are clipped");
    assert!(wide.hbar().is_empty());
}

#[test]
fn cursor_visibility_rules() {
    let v = fed(b"ab");
    assert_eq!(
        paint(&v, true).1,
        Some(Position::new(AREA.x + 2, AREA.y)),
        "the hardware cursor is the grid cursor"
    );
    assert_eq!(
        paint(&v, false).1,
        None,
        "a dialog, a menu, a drag or another focus withholds it"
    );

    let mut hidden = fed(b"ab");
    hidden.feed(b"\x1b[?25l");
    assert!(!hidden.caret_visible());
    assert_eq!(paint(&hidden, true).1, None, "the program hid its cursor");
    hidden.feed(b"\x1b[?25h");
    assert!(hidden.caret_visible());
    assert_eq!(
        paint(&hidden, true).1,
        Some(Position::new(AREA.x + 2, AREA.y))
    );

    let mut scrolled = history();
    assert_eq!(
        paint(&scrolled, true).1,
        Some(Position::new(AREA.x + 8, AREA.y + 9))
    );
    scrolled.wheel(junie_tui::core::hit::Axis::Vertical, -3);
    assert_eq!(
        paint(&scrolled, true).1,
        None,
        "a scrolled-back pane has no live cursor"
    );
    scrolled.set_follow(true);
    assert!(paint(&scrolled, true).1.is_some());

    let pending = fed(b"0123456789");
    assert_eq!(
        paint(&pending, true).1,
        Some(Position::new(AREA.x + 9, AREA.y)),
        "a cursor after a full row rests on the last column"
    );
}
