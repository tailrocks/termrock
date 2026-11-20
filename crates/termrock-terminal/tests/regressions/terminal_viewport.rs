use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::{Position, Rect};
use termrock_controls::scrollbar;
use termrock_core::event::Outcome;
use termrock_core::focus::FocusRing;
use termrock_core::hit::{Axis, Bar, HitRegistry};
use termrock_core::id::WidgetId;
use termrock_core::scroll::TrackClick;
use termrock_runtime::ctx::{Interaction, RenderCtx};
use termrock_terminal::{TermEvent, TerminalState, TerminalViewport};
use termrock_theme::{ColorLevel, Theme};
use termrock_viewport::viewport::CellPos;
use termrock_viewport::viewport_ops::ViewportOps;

const ID: WidgetId = WidgetId::of("t.terminal.viewport");
const AREA: Rect = Rect::new(0, 0, 11, 10);

fn joined(first: u32, last: u32) -> Vec<u8> {
    (first..=last)
        .map(|n| format!("line {n:03}"))
        .collect::<Vec<_>>()
        .join("\r\n")
        .into_bytes()
}

fn with_state(state: TerminalState) -> TerminalViewport {
    let mut v = TerminalViewport::new(ID, state);
    v.set_area(AREA);
    v
}

fn blank() -> TerminalViewport {
    with_state(TerminalState::new(10, 10))
}

fn viewport(last: u32) -> TerminalViewport {
    let mut v = blank();
    v.feed(&joined(1, last));
    v
}

fn paint(v: &dyn ViewportOps, caret_allowed: bool) -> (Buffer, Option<Position>) {
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 12));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    let cursor = v.draw(AREA, &mut buf, &mut ctx, theme.canvas, caret_allowed);
    (buf, cursor)
}

fn grid_rows(buf: &Buffer) -> Vec<String> {
    (0..AREA.height)
        .map(|y| {
            (0..AREA.width - 1)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

fn screen(v: &dyn ViewportOps) -> Vec<String> {
    grid_rows(&paint(v, true).0)
}

fn lines(first: u32, last: u32) -> Vec<String> {
    (first..=last).map(|n| format!("line {n:03}")).collect()
}

fn drag(v: &mut TerminalViewport, from: (u16, u16), to: (u16, u16)) {
    v.on_press_with(Position::new(from.0, from.1), 1, KeyModifiers::NONE);
    v.on_drag(Position::new(to.0, to.1));
}

fn wheel_up(v: &mut TerminalViewport, notches: u32) {
    for _ in 0..notches {
        v.wheel(Axis::Vertical, -3);
    }
}

#[test]
fn wheel_scrolls_history() {
    let mut v = viewport(100);
    assert!(v.follow());
    assert_eq!(v.scroll().content_len, 100, "history 90 + 10 live rows");
    assert_eq!(v.scroll().visible_range(), 90..100);
    assert_eq!(screen(&v), lines(91, 100));
    assert_eq!(v.position_label(), "", "the live tail shows no position");

    assert_eq!(v.wheel(Axis::Vertical, -3), Outcome::Changed);
    assert!(!v.follow());
    assert_eq!(v.offset(), 3);
    assert_eq!(screen(&v), lines(88, 97), "one notch is 3 rows");
    assert_eq!(v.scroll().visible_range(), 87..97);
    assert_eq!(v.position_label(), "88–97 of 100");

    wheel_up(&mut v, 2);
    assert_eq!(screen(&v), lines(82, 91));
    assert_eq!(v.position_label(), "82–91 of 100");

    assert_eq!(v.wheel(Axis::Vertical, 3), Outcome::Changed);
    assert_eq!(screen(&v), lines(85, 94));
    assert_eq!(
        v.wheel(Axis::Horizontal, 3),
        Outcome::Ignored,
        "a terminal grid has no side scroll"
    );

    v.jump_start();
    assert_eq!(screen(&v), lines(1, 10));
    assert_eq!(v.position_label(), "1–10 of 100");
    assert_eq!(
        v.wheel(Axis::Vertical, -3),
        Outcome::Consumed,
        "the oldest row stops the wheel"
    );
    v.page_down();
    assert_eq!(screen(&v), lines(11, 20));
    v.page_up();
    assert_eq!(screen(&v), lines(1, 10));
}

#[test]
fn scroll_is_anchored_against_new_output() {
    let mut v = viewport(100);
    wheel_up(&mut v, 2);
    let before = screen(&v);
    assert_eq!(before, lines(85, 94));
    assert_eq!(v.offset(), 6);

    v.feed(b"\r\nline 101\r\nline 102");
    assert_eq!(screen(&v), before, "new output does not move what is read");
    assert_eq!(v.offset(), 8);
    assert_eq!(v.position_label(), "85–94 of 102");

    v.set_follow(true);
    assert_eq!(screen(&v), lines(93, 102));
    v.feed(b"\r\nline 103");
    assert_eq!(
        screen(&v),
        lines(94, 103),
        "at the live tail the view follows the output"
    );
    assert_eq!(v.offset(), 0);
}

#[test]
fn anchor_at_the_scrollback_limit() {
    let mut v = with_state(TerminalState::with_scrollback(10, 10, 20));
    v.feed(&joined(1, 40));
    assert_eq!(v.state().scrollback_len(), 20);
    assert_eq!(v.scroll().content_len, 30);
    wheel_up(&mut v, 1);
    v.wheel(Axis::Vertical, -2);
    assert_eq!(v.offset(), 5);
    assert_eq!(screen(&v), lines(26, 35));

    v.feed(b"\r\nline 041\r\nline 042");
    assert_eq!(v.state().scrollback_len(), 20, "the limit holds");
    assert_eq!(v.offset(), 7);
    assert_eq!(screen(&v), lines(26, 35), "below the limit nothing slides");

    v.jump_start();
    assert_eq!(v.offset(), 20);
    assert_eq!(screen(&v), lines(13, 22));
    v.feed(b"\r\nline 043");
    assert_eq!(v.offset(), 20, "the offset clamps to the history");
    assert_eq!(
        screen(&v),
        lines(14, 23),
        "the view slides by the one row that was trimmed"
    );
    assert_eq!(v.scroll().content_len, 30);
    assert_eq!(v.scroll().visible_range(), 0..10);
}

#[test]
fn copy_at_the_live_tail() {
    let mut v = viewport(100);
    drag(&mut v, (0, 3), (8, 5));
    assert!(v.has_selection());
    assert_eq!(
        v.selected_text().as_deref(),
        Some("line 094\nline 095\nline 096")
    );
    drag(&mut v, (5, 9), (8, 9));
    assert_eq!(v.selected_text().as_deref(), Some("100"));
}

#[test]
fn copy_from_history() {
    let mut v = viewport(100);
    wheel_up(&mut v, 2);
    drag(&mut v, (0, 0), (8, 2));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("line 085\nline 086\nline 087")
    );
}

#[test]
fn copy_across_history_and_live() {
    let mut v = viewport(100);
    wheel_up(&mut v, 1);
    assert_eq!(screen(&v), lines(88, 97));
    drag(&mut v, (5, 2), (4, 4));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("090\nline 091\nline"),
        "line 090 is the last history row and line 091 the first live row"
    );
}

#[test]
fn copy_survives_scrolling_away() {
    let mut v = viewport(100);
    drag(&mut v, (0, 3), (8, 5));
    let text = v.selected_text();
    wheel_up(&mut v, 4);
    assert_eq!(v.selected_text(), text, "the rows are tracked by number");
    v.feed(b"\r\nmore");
    assert_eq!(v.selected_text(), text, "and so is new output");
}

#[test]
fn a_selection_follows_its_text_when_no_history_is_kept() {
    let mut v = with_state(TerminalState::with_scrollback(10, 10, 0));
    v.feed(&joined(1, 10));
    drag(&mut v, (0, 7), (8, 7));
    assert_eq!(v.selected_text().as_deref(), Some("line 008"));

    v.feed(b"\r\nline 011");
    assert_eq!(
        v.selected_text().as_deref(),
        Some("line 008"),
        "the selected text moved up a row and the selection moved with it"
    );
    assert_eq!(screen(&v)[6], "line 008");
}

#[test]
fn a_one_column_area_still_shows_the_program_its_column() {
    let area = Rect::new(0, 0, 1, 4);
    let (rows, cols) = TerminalViewport::grid_size(area);
    assert_eq!((rows, cols), (4, 1));
    let mut v = TerminalViewport::new(ID, TerminalState::new(rows, cols));
    v.set_area(area);
    v.feed(b"x");

    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(area);
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    ViewportOps::draw(&v, area, &mut buf, &mut ctx, theme.canvas, false);

    assert_eq!(buf[(0, 0)].symbol(), "x");
}

fn wrapped(bytes: &[u8]) -> TerminalViewport {
    let mut v = blank();
    v.feed(bytes);
    v
}

fn roomy(bytes: &[u8]) -> TerminalViewport {
    let mut v = TerminalViewport::new(ID, TerminalState::new(5, 40));
    v.set_area(Rect::new(0, 0, 41, 5));
    v.feed(bytes);
    v
}

#[test]
fn soft_wraps_join() {
    let mut v = wrapped(b"ABCDEFGHIJKLMNOPQRSTUVWXY");
    assert_eq!(screen(&v)[..3], ["ABCDEFGHIJ", "KLMNOPQRST", "UVWXY"]);
    drag(&mut v, (0, 0), (10, 2));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("ABCDEFGHIJKLMNOPQRSTUVWXY"),
        "no newline at a soft boundary"
    );

    let mut hard = wrapped(b"ab\r\ncd");
    drag(&mut hard, (0, 0), (10, 1));
    assert_eq!(
        hard.selected_text().as_deref(),
        Some("ab\ncd"),
        "a hard line break stays one"
    );

    let mut gap = wrapped(b"ABCDEFGH  XYZ");
    assert_eq!(screen(&gap)[..2], ["ABCDEFGH", "XYZ"]);
    drag(&mut gap, (0, 0), (10, 1));
    assert_eq!(
        gap.selected_text().as_deref(),
        Some("ABCDEFGH  XYZ"),
        "blanks inside a logical line are kept"
    );
}

#[test]
fn trailing_blanks_trim_only_at_the_logical_line_end() {
    let mut v = wrapped(b"ab   \r\ncd  ");
    drag(&mut v, (0, 0), (10, 1));
    assert_eq!(v.selected_text().as_deref(), Some("ab\ncd"));

    let mut tail = wrapped(b"short");
    drag(&mut tail, (0, 0), (10, 0));
    assert_eq!(
        tail.selected_text().as_deref(),
        Some("short"),
        "dragging past the text copies no padding"
    );
}

#[test]
fn partial_wrapped_selection() {
    let mut v = wrapped(b"ABCDEFGHIJKLMNOPQRSTUVWXY");
    drag(&mut v, (5, 0), (3, 1));
    assert_eq!(v.selected_text().as_deref(), Some("FGHIJKLM"));

    let mut inside = wrapped(b"ABCDEFGHIJKLMNOPQRSTUVWXY");
    drag(&mut inside, (2, 1), (6, 1));
    assert_eq!(inside.selected_text().as_deref(), Some("MNOP"));

    let mut backwards = wrapped(b"ABCDEFGHIJKLMNOPQRSTUVWXY");
    drag(&mut backwards, (3, 1), (5, 0));
    assert_eq!(
        backwards.selected_text().as_deref(),
        Some("FGHIJKLM"),
        "the direction of the drag does not matter"
    );
}

#[test]
fn wide_character_is_copied_whole() {
    let mut v = wrapped("日本語 ok".as_bytes());
    drag(&mut v, (0, 0), (2, 0));
    assert_eq!(v.selected_text().as_deref(), Some("日"));
    drag(&mut v, (0, 0), (3, 0));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("日"),
        "a head inside the second wide character stops before it"
    );
    drag(&mut v, (1, 0), (4, 0));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("日本"),
        "a press on the second half of a wide character starts at the character"
    );
    drag(&mut v, (0, 0), (9, 0));
    assert_eq!(v.selected_text().as_deref(), Some("日本語 ok"));
    drag(&mut v, (0, 0), (1, 0));
    assert_eq!(
        v.selected_text(),
        None,
        "pointing inside one wide character selects nothing"
    );
}

#[test]
fn word_select_and_release_copy() {
    let mut v = roomy(b"run the build-all script\r\nnext one");
    assert_eq!(v.select_word_at(Position::new(5, 0)), Outcome::Changed);
    assert_eq!(v.selected_text().as_deref(), Some("the"));
    v.select_word_at(Position::new(12, 0));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("build-all"),
        "a word keeps its hyphen"
    );
    assert_eq!(
        v.select_word_at(Position::new(3, 0)),
        Outcome::Consumed,
        "a blank is no word"
    );

    v.on_press_with(Position::new(1, 1), 2, KeyModifiers::NONE);
    v.on_click_with(Position::new(1, 1), 2);
    assert_eq!(v.selected_text().as_deref(), Some("next"));

    v.on_press_with(Position::new(1, 0), 3, KeyModifiers::NONE);
    v.on_click_with(Position::new(1, 0), 3);
    assert_eq!(
        v.selected_text().as_deref(),
        Some("run the build-all script"),
        "three clicks take the logical line"
    );

    let mut long = wrapped(b"ABCDEFGHIJKLMNOPQRSTUVWXY\r\nz");
    long.on_press_with(Position::new(1, 1), 3, KeyModifiers::NONE);
    long.on_click_with(Position::new(1, 1), 3);
    assert_eq!(
        long.selected_text().as_deref(),
        Some("ABCDEFGHIJKLMNOPQRSTUVWXY"),
        "a wrapped line is one line"
    );

    let mut release = viewport(100);
    drag(&mut release, (0, 0), (8, 0));
    assert_eq!(
        release.selected_text().as_deref(),
        Some("line 091"),
        "what a drag release hands to the clipboard"
    );
}

#[test]
fn selection_is_painted_per_physical_row() {
    let mut v = wrapped(b"ABCDEFGHIJKLMNOPQRSTUVWXY");
    drag(&mut v, (5, 0), (3, 1));
    let (buf, _) = paint(&v, true);
    let want = Theme::for_level(ColorLevel::TrueColor)
        .selection()
        .bg
        .expect("the selection has a background");
    let marked = |x: u16, y: u16| buf[(x, y)].bg == want;
    assert!((5..10).all(|x| marked(x, 0)), "FGHIJ");
    assert!((0..3).all(|x| marked(x, 1)), "KLM");
    assert!(!marked(4, 0) && !marked(3, 1), "outside the range");
    assert!(!(0..10).any(|x| marked(x, 2)), "the next row");
}

#[test]
fn alt_screen_no_scroll_and_copy() {
    let mut v = viewport(100);
    v.feed(b"\x1b[?1049h\x1b[HA1\r\nA2\r\nA3");
    assert!(v.state().modes().alt_screen);
    assert_eq!(
        v.wheel(Axis::Vertical, -3),
        Outcome::Ignored,
        "the wheel never reaches the primary history"
    );
    assert!(v.follow());
    assert_eq!(screen(&v)[..3], ["A1", "A2", "A3"]);
    assert_eq!(v.scroll().content_len, 10);
    drag(&mut v, (0, 0), (2, 2));
    assert_eq!(
        v.selected_text().as_deref(),
        Some("A1\nA2\nA3"),
        "copy reads the alternate rows"
    );
    v.page_up();
    v.jump_start();
    assert_eq!(v.offset(), 0);
}

#[test]
fn alt_screen_toggle_resets() {
    let mut v = viewport(100);
    wheel_up(&mut v, 2);
    drag(&mut v, (0, 0), (8, 1));
    assert!(v.has_selection() && !v.follow());

    v.feed(b"\x1b[?1049h");
    assert!(v.follow(), "entering resets the offset");
    assert!(
        !v.has_selection(),
        "and drops a selection of the other buffer"
    );

    v.feed(b"\x1b[?1049l");
    assert!(v.follow(), "leaving does not restore the old offset");
    assert!(!v.has_selection());
    assert_eq!(screen(&v), lines(91, 100));

    let mut live = viewport(100);
    drag(&mut live, (0, 3), (8, 5));
    live.feed(b"\x1b[?1049h");
    assert!(
        !live.has_selection(),
        "the live rows of the primary screen are not the alternate rows"
    );
    live.feed(b"\x1b[HA1\r\nA2");
    drag(&mut live, (0, 0), (2, 1));
    assert_eq!(live.selected_text().as_deref(), Some("A1\nA2"));
    live.feed(b"\x1b[?1049l");
    assert!(
        !live.has_selection(),
        "nor are the alternate rows the primary ones"
    );
}

#[test]
fn mouse_mode_does_not_change_scrolling() {
    let mut plain = viewport(100);
    let mut reporting = blank();
    reporting.feed(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");
    reporting.feed(&joined(1, 100));
    for v in [&mut plain, &mut reporting] {
        wheel_up(v, 2);
        drag(v, (0, 0), (8, 2));
    }
    assert_eq!(screen(&reporting), screen(&plain));
    assert_eq!(reporting.offset(), plain.offset());
    assert_eq!(reporting.selected_text(), plain.selected_text());
    assert_eq!(
        reporting.selected_text().as_deref(),
        Some("line 085\nline 086\nline 087")
    );
}

#[test]
fn resize_clamps_and_clears() {
    let mut v = viewport(100);
    v.jump_start();
    drag(&mut v, (0, 0), (8, 2));
    assert!(v.has_selection());
    assert_eq!(v.offset(), 90);

    v.resize(6, 10);
    assert!(!v.has_selection(), "a resize drops the selection");
    assert!(
        v.offset() <= v.state().scrollback_len(),
        "the offset stays inside the history"
    );
    assert_eq!(v.state().size(), (6, 10));
    assert_eq!(
        v.scroll().content_len,
        v.state().scrollback_len() + 6,
        "the scroll model follows the new height"
    );
    assert_eq!(v.scroll().viewport_len, 6);

    v.set_follow(true);
    v.feed(b"\r\nx");
    assert_eq!(v.state().size(), (6, 10));
}

#[test]
fn scrollback_cleared_resets() {
    let mut v = viewport(100);
    wheel_up(&mut v, 2);
    drag(&mut v, (0, 0), (8, 2));
    let events = v.feed(b"\x1b[3J");
    assert!(events.contains(&TermEvent::ScrollbackCleared));
    assert!(v.follow());
    assert!(!v.has_selection());
    assert_eq!(v.state().scrollback_len(), 0);
    assert_eq!(v.scroll().content_len, 10);

    let mut live = viewport(100);
    drag(&mut live, (0, 3), (8, 5));
    assert!(live.has_selection());
    live.feed(b"\x1b[3J");
    assert!(
        !live.has_selection(),
        "the rows under a selection of the live screen are gone too"
    );

    let mut other = viewport(100);
    wheel_up(&mut other, 2);
    drag(&mut other, (0, 0), (8, 2));
    other.reset();
    assert!(other.follow() && !other.has_selection());
    assert_eq!(other.state().size(), (10, 10));
    assert_eq!(other.state().scrollback_len(), 0);
    assert_eq!(screen(&other), vec![String::new(); 10]);
}

#[test]
fn scrolled_back_edges_fade_and_the_live_tail_does_not() {
    let mut v = viewport(100);
    let tail = paint(&v, true).0;
    v.wheel(Axis::Vertical, -3);
    let back = paint(&v, true).0;
    let fg = |buf: &Buffer, y: u16| buf[(0, y)].fg;
    assert_eq!(grid_rows(&tail)[0], "line 091");
    assert_eq!(grid_rows(&back)[0], "line 088");
    assert_eq!(
        fg(&tail, 0),
        fg(&tail, 4),
        "at the live tail the program's rows keep their colour"
    );
    assert_ne!(
        fg(&back, 0),
        fg(&back, 4),
        "scrolled back, the first row fades to show there is more above"
    );
    assert_ne!(fg(&back, 9), fg(&back, 4), "and the last row, more below");
}

#[test]
fn a_drag_after_the_area_collapsed_is_ignored() {
    let mut v = viewport(100);
    v.on_press_with(Position::new(1, 0), 1, KeyModifiers::NONE);
    v.set_area(Rect::new(3, 2, 0, 0));
    assert_eq!(v.on_drag(Position::new(2, 2)), Outcome::Ignored);
    v.set_area(AREA);
    assert_eq!(
        v.on_drag(Position::new(4, 2)),
        Outcome::Ignored,
        "the press ended with the area: a later drag selects nothing"
    );
    assert!(!v.has_selection());
}

#[test]
fn no_selection_is_none() {
    let mut v = viewport(100);
    assert!(!v.has_selection());
    assert_eq!(v.selected_text(), None);
    v.on_click(Position::new(2, 2));
    assert!(!v.has_selection(), "a press alone selects nothing");
    assert_eq!(v.selected_text(), None);
    v.on_drag(Position::new(2, 2));
    assert_eq!(
        v.selected_text(),
        None,
        "a drag inside one cell selects nothing"
    );
    assert_eq!(
        blank().clear_selection(),
        Outcome::Ignored,
        "nothing to clear"
    );
}

#[test]
fn escape_and_click_clear_the_selection() {
    let mut v = viewport(100);
    drag(&mut v, (0, 3), (8, 5));
    assert_eq!(v.clear_selection(), Outcome::Changed);
    assert!(!v.has_selection());
    assert_eq!(v.selected_text(), None);

    drag(&mut v, (0, 3), (8, 5));
    assert!(v.has_selection());
    assert_eq!(
        v.on_click(Position::new(4, 8)),
        Outcome::Changed,
        "a click elsewhere starts over"
    );
    assert!(!v.has_selection());
}

#[test]
fn end_returns_to_the_tail() {
    let mut v = viewport(100);
    wheel_up(&mut v, 5);
    assert!(!v.follow());
    v.set_follow(true);
    assert!(v.follow());
    assert_eq!(screen(&v), lines(91, 100));
    assert_eq!(v.scroll().offset, v.scroll().max_offset());

    wheel_up(&mut v, 3);
    for _ in 0..3 {
        v.page_down();
    }
    assert!(v.follow(), "paging down to the end follows the tail again");
    assert_eq!(screen(&v), lines(91, 100));
}

#[test]
fn keyboard_copy_is_off() {
    let v = blank();
    assert!(
        !v.keyboard_copy(),
        "a terminal pane copies on drag release; y belongs to the program"
    );
}

#[test]
fn the_terminal_viewport_draws_its_own_track() {
    let v = blank();
    assert!(
        v.draws_track(),
        "a host that draws a second bar over the track column would double it"
    );
}

#[test]
fn the_ops_drive_the_terminal_viewport_through_the_trait() {
    let mut v = viewport(100);
    let ops: &mut dyn ViewportOps = &mut v;
    assert_eq!(ops.id(), ID);
    assert!(ops.follow());
    assert_eq!(ops.scroll().content_len, 100);
    assert!(ops.hbar().is_empty(), "a terminal pane has no bottom bar");

    assert_eq!(
        ops.caret(),
        Some(CellPos { line: 99, col: 8 }),
        "the caret is the grid cursor, by absolute row"
    );
    assert!(ops.caret_visible());
    assert_eq!(
        ops.screen_pos(CellPos { line: 99, col: 8 }),
        Some(Position::new(8, 9))
    );
    assert_eq!(
        ops.screen_pos(CellPos { line: 80, col: 0 }),
        None,
        "a row in the history is off screen at the tail"
    );
    assert_eq!(ops.context_point(), Position::new(8, 9));

    ops.wheel(Axis::Vertical, -3);
    assert_eq!(
        ops.screen_pos(CellPos { line: 94, col: 2 }),
        Some(Position::new(2, 7)),
        "line 95 is the eighth row when three rows back"
    );
    assert_eq!(ops.screen_pos(CellPos { line: 99, col: 8 }), None);
    assert_eq!(ops.context_point(), Position::new(0, 0));

    ops.set_follow(true);
    ops.on_click(Position::new(0, 1));
    ops.on_drag(Position::new(4, 1));
    assert!(ops.has_selection());
    assert_eq!(ops.selected_text().as_deref(), Some("line"));
    assert_eq!(ops.context_point(), Position::new(4, 1));
    ops.clear_selection();
    ops.select_word_at(Position::new(6, 2));
    assert_eq!(ops.selected_text().as_deref(), Some("093"));

    let area = Rect::new(2, 1, 9, 6);
    ops.set_area(area);
    assert_eq!(ops.scroll().viewport_len, 10, "the grid size is the host's");
}

fn paint_at(v: &TerminalViewport, level: ColorLevel) -> (Buffer, Theme) {
    let theme = Theme::for_level(level);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 12));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    v.draw(AREA, &mut buf, &mut ctx, theme.canvas, false);
    (buf, theme)
}

#[test]
fn default_colours_follow_the_theme_and_program_colours_stay() {
    let mut v = blank();
    v.feed(b"a \x1b[31mr\x1b[0m \x1b[48;2;1;2;3mb\x1b[0m");
    let (buf, theme) = paint_at(&v, ColorLevel::TrueColor);
    assert_eq!(buf[(0, 0)].fg, theme.text_primary, "a default foreground");
    assert_eq!(buf[(0, 0)].bg, theme.canvas, "a default background");
    assert_eq!(
        buf[(2, 0)].fg,
        ratatui::style::Color::Indexed(1),
        "the program's own colour is kept"
    );
    assert_eq!(
        buf[(4, 0)].bg,
        ratatui::style::Color::Rgb(1, 2, 3),
        "so is its own background"
    );

    let (mono, _) = paint_at(&v, ColorLevel::Mono);
    assert_eq!(
        mono[(2, 0)].fg,
        ratatui::style::Color::Reset,
        "without colour the program's colours are dropped"
    );
}

#[test]
fn terminal_viewport_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TerminalViewport>();
}

#[test]
fn clear_scrollback_returns_to_the_tail_and_drops_the_selection() {
    let mut v = viewport(100);
    wheel_up(&mut v, 2);
    drag(&mut v, (0, 0), (8, 2));
    assert!(!v.follow() && v.has_selection());

    v.clear_scrollback();

    assert!(v.follow());
    assert!(!v.has_selection());
    assert_eq!(v.offset(), 0);
    assert_eq!(v.state().scrollback_len(), 0);
    assert_eq!(v.scroll().content_len, 10, "only the live rows are left");
    assert_eq!(screen(&v), lines(91, 100), "the screen itself is kept");

    let mut live = viewport(100);
    drag(&mut live, (0, 3), (8, 5));
    live.clear_scrollback();
    assert!(
        live.has_selection(),
        "a selection of live rows does not depend on the history"
    );
    assert_eq!(
        live.selected_text().as_deref(),
        Some("line 094\nline 095\nline 096")
    );
}

fn press_below_the_thumb(app: TrackClick) -> bool {
    let mut v = viewport(300);
    v.jump_start();
    let theme = Theme::for_level(ColorLevel::TrueColor);
    let mut buf = Buffer::empty(Rect::new(0, 0, 30, 12));
    let mut hits = HitRegistry::default();
    let mut ring = FocusRing::default();
    let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
    ctx.track_click = app;
    ViewportOps::draw(&v, AREA, &mut buf, &mut ctx, theme.canvas, false);

    let bar = Bar {
        id: scrollbar::id_for(ID),
        owner: ID,
        axis: Axis::Vertical,
        track: Rect::new(AREA.right() - 1, AREA.y, 1, AREA.height),
    };
    v.bar_input(
        &bar,
        Position::new(AREA.right() - 1, AREA.bottom() - 1),
        true,
    );
    v.follow()
}

#[test]
fn the_app_track_click_of_a_drawn_frame_decides_a_press_on_the_track() {
    assert!(
        press_below_the_thumb(TrackClick::Jump),
        "the app asked for jump: a press at the bottom of the track lands at the live tail"
    );
    assert!(
        !press_below_the_thumb(TrackClick::Page),
        "the app kept paging: the same press moves one page"
    );
}

#[test]
fn the_track_click_each_terminal_was_drawn_with_decides_its_own_press() {
    let mut first = viewport(300);
    let mut second = viewport(300);
    first.jump_start();
    second.jump_start();
    let theme = Theme::for_level(ColorLevel::TrueColor);
    for (v, click) in [(&first, TrackClick::Jump), (&second, TrackClick::Page)] {
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 12));
        let mut hits = HitRegistry::default();
        let mut ring = FocusRing::default();
        let mut ctx = RenderCtx::new(&theme, Interaction::default(), &mut hits, &mut ring);
        ctx.track_click = click;
        ViewportOps::draw(v, AREA, &mut buf, &mut ctx, theme.canvas, false);
    }

    let bar = Bar {
        id: scrollbar::id_for(ID),
        owner: ID,
        axis: Axis::Vertical,
        track: Rect::new(AREA.right() - 1, AREA.y, 1, AREA.height),
    };
    let press = Position::new(AREA.right() - 1, AREA.bottom() - 1);
    first.bar_input(&bar, press, true);
    second.bar_input(&bar, press, true);

    assert!(
        first.follow(),
        "the first terminal was drawn with jump: a press at the bottom of the track lands at the live tail"
    );
    assert!(
        !second.follow(),
        "the second terminal was drawn with page: the same press moves one page"
    );
}

#[test]
fn issue_terminal_screen_pos_excludes_track() {
    for origin in [Position::new(0, 0), Position::new(3, 2)] {
        let area = Rect::new(origin.x, origin.y, 6, 4);
        let mut state = TerminalState::new(4, 20);
        state.feed(b"0123456789012345678");
        let mut v = TerminalViewport::new(ID, state);
        v.set_area(area);
        let ops: &dyn ViewportOps = &v;
        let at = |col| ops.screen_pos(CellPos { line: 0, col });

        assert_eq!(at(4), Some(Position::new(origin.x + 4, origin.y)));
        assert_eq!(at(5), None, "column 5 is the scroll track");
        assert_eq!(at(19), None, "column 19 is past the pane");
        assert_eq!(at(25), None, "column 25 is past the emulated grid");
        assert_eq!(
            ops.caret(),
            Some(CellPos { line: 0, col: 19 }),
            "the program's caret stays where the emulator put it"
        );
        let anchor = ops.context_point();
        assert!(
            anchor.x < area.right() - 1 && area.contains(anchor),
            "the context point {anchor:?} falls back inside the content of {area:?}"
        );
    }
}

#[test]
fn issue_terminal_viewport_draw_clips_wide_glyph_before_the_track() {
    let mut v = with_state(TerminalState::new(10, 12));
    v.feed("abcdefghi\u{754c}j".as_bytes());

    let (buf, _) = paint(&v, true);

    assert_eq!(buf[(8, 0)].symbol(), "i");
    assert_eq!(
        buf[(9, 0)].symbol(),
        " ",
        "the wide glyph would cover the track"
    );
    assert_ne!(buf[(10, 0)].symbol(), "j");
    assert_ne!(buf[(10, 0)].symbol(), "\u{754c}");
}
