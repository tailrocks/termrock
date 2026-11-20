use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, CellWidth};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;
use termrock_core::event::Key;
use termrock_terminal::{
    Modes, RowWrap, TermEvent, TermView, TerminalState, TerminalView, encode_key,
};

use super::support::draw_state;

fn lines(from: u32, to: u32) -> Vec<u8> {
    (from..=to)
        .flat_map(|n| format!("L{n:02}\r\n").into_bytes())
        .collect()
}

fn view_rows(view: &TermView<'_>) -> Vec<String> {
    (0..view.rows())
        .map(|row| {
            (0..view.cols())
                .filter_map(|col| view.cell(row, col))
                .filter(|cell| !cell.is_wide_continuation())
                .map(|cell| match cell.text() {
                    "" => " ".to_owned(),
                    text => text.to_owned(),
                })
                .collect()
        })
        .collect()
}

fn buf_rows(buf: &Buffer) -> Vec<String> {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect()
        })
        .collect()
}

fn numbered(first: u32, last: u32) -> Vec<String> {
    (first..=last).map(|n| format!("L{n:02}")).collect()
}

fn history_state() -> TerminalState {
    let mut state = TerminalState::new(6, 3);
    state.feed(&lines(1, 29));
    state.feed(b"L30");
    state
}

#[test]
fn scrolled_view_reads_scrollback() {
    let mut state = history_state();
    assert_eq!(state.scrollback_len(), 24);
    assert_eq!(buf_rows(&draw_state(&state, false)), numbered(25, 30));
    for (offset, first) in [(3, 22), (6, 19), (10, 15), (99, 1)] {
        state.set_scrollback(offset);
        assert_eq!(
            buf_rows(&draw_state(&state, false)),
            numbered(first, first + 5),
            "offset {offset}"
        );
        assert_eq!(
            view_rows(&state.view(offset)),
            numbered(first, first + 5),
            "view at offset {offset}"
        );
    }
    assert_eq!(state.scrollback(), 24);
}

#[test]
fn view_forces_offset_zero_on_the_alternate_screen() {
    let mut state = history_state();
    let events = state.feed(b"\x1b[?1049h\x1b[H\x1b[2JA1\r\nA2\r\nA3");
    assert_eq!(events, vec![]);
    let live = vec!["A1 ", "A2 ", "A3 ", "   ", "   ", "   "];
    for offset in [0, 3, 6, 99] {
        assert_eq!(view_rows(&state.view(offset)), live, "view at {offset}");
    }
    state.set_scrollback(6);
    assert_eq!(buf_rows(&draw_state(&state, false)), live);

    state.feed(b"\x1b[?1049l");
    assert_eq!(view_rows(&state.view(3)), numbered(22, 27));
}

#[test]
fn with_scrollback_limit_is_honoured() {
    let mut state = TerminalState::with_scrollback(6, 3, 20);
    for n in 1..=10 {
        state.feed(&lines(n, n));
    }
    assert_eq!(state.scrollback_len(), 5);
    assert_eq!(state.scrolled_off(), 5);

    for n in 11..=60 {
        state.feed(&lines(n, n));
    }
    assert_eq!(state.scrollback_len(), 20);
    assert_eq!(state.scrolled_off(), 55);

    let mut once = TerminalState::with_scrollback(6, 3, 20);
    once.feed(&lines(1, 60));
    assert_eq!(once.scrollback_len(), 20);
    assert_eq!(once.scrolled_off(), 55);

    state.reset();
    assert_eq!(state.size(), (6, 3));
    assert_eq!((state.scrollback_len(), state.scrolled_off()), (0, 0));
    state.feed(&lines(1, 60));
    assert_eq!(state.scrollback_len(), 20);

    let mut none = TerminalState::with_scrollback(6, 3, 0);
    none.feed(&lines(1, 60));
    assert_eq!((none.scrollback_len(), none.scrolled_off()), (0, 55));
}

#[test]
fn scrolled_off_is_exact_after_many_feeds() {
    let mut state = TerminalState::with_scrollback(6, 3, 20);
    for _ in 0..100 {
        state.feed(b"abc\r\n");
    }
    assert_eq!(state.scrollback_len(), 20);
    assert_eq!(state.scrolled_off(), 95);
}

#[test]
fn scrolled_off_counts_only_rows_that_reach_history() {
    let mut state = TerminalState::with_scrollback(6, 3, 50);
    state.feed(b"\x1b[M\x1b[3M");
    assert_eq!((state.scrollback_len(), state.scrolled_off()), (0, 0));

    state.feed(b"\x1b[?1049h\r\n\r\n\r\n\r\n\r\n\r\n\r\n\r\n\x1b[?1049l");
    assert_eq!((state.scrollback_len(), state.scrolled_off()), (0, 0));

    state.feed(b"\x1b[H\x1b[2Jone\r\ntwo\r\nthree\r\nfour\r\nfive\r\nsix\r\nseven\r\neight");
    let pushed = state.scrollback_len();
    assert!(pushed > 0);
    assert_eq!(state.scrolled_off(), pushed as u64);

    state.feed(b"\x1b[2J\x1b[H");
    assert_eq!(state.scrollback_len(), pushed);
    assert_eq!(state.scrolled_off(), pushed as u64);
}

#[test]
fn wide_cell_is_drawn_once() {
    let mut state = TerminalState::new(2, 6);
    state.feed("\x1b[41m日\x1b[0mx".as_bytes());
    let buf = draw_state(&state, false);
    assert_eq!(buf[(0, 0)].symbol(), "日");
    assert_eq!(buf[(0, 0)].bg, Color::Indexed(1));
    assert_eq!(buf[(1, 0)].symbol(), " ");
    assert_eq!(buf[(1, 0)].bg, Color::Reset);
    assert_eq!(buf[(2, 0)].symbol(), "x");
    assert_eq!(buf[(2, 0)].bg, Color::Reset);
}

#[test]
fn phantom_cursor_column_stays_visible() {
    let mut state = TerminalState::new(3, 5);
    state.feed(b"abcde");
    assert_eq!(state.view(0).cursor(), (0, 4));
    let buf = draw_state(&state, true);
    let reversed: Vec<(u16, u16)> = (0..3)
        .flat_map(|y| (0..5).map(move |x| (x, y)))
        .filter(|&at| buf[at].modifier.contains(Modifier::REVERSED))
        .collect();
    assert_eq!(reversed, vec![(4, 0)]);

    let unfocused = draw_state(&state, false);
    assert!(!unfocused[(4, 0)].modifier.contains(Modifier::REVERSED));

    state.feed(b"\x1b[?25l");
    let hidden = draw_state(&state, true);
    assert!(
        (0..5).all(|x| !hidden[(x, 0)].modifier.contains(Modifier::REVERSED)),
        "a hidden cursor draws nothing"
    );
}

#[test]
fn terminal_state_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<TerminalState>();
}

#[test]
fn replies_for_queries() {
    let mut state = TerminalState::new(6, 20);
    let reply = |events: Vec<TermEvent>| match events.as_slice() {
        [TermEvent::Reply(bytes)] => bytes.clone(),
        other => panic!("expected one reply, got {other:?}"),
    };

    let device = reply(state.feed(b"\x1b[c"));
    assert!(
        device.starts_with(b"\x1b[?") && device.ends_with(b"c"),
        "{device:?}"
    );

    assert_eq!(reply(state.feed(b"\x1b[3;5H\x1b[6n")), b"\x1b[3;5R");

    let colour = reply(state.feed(b"\x1b]11;?\x07"));
    assert!(colour.starts_with(b"\x1b]11;"), "{colour:?}");

    assert_eq!(reply(state.feed(b"\x1b[?2004$p")), b"\x1b[?2004;2$y");

    assert_eq!(state.feed(b"plain text"), vec![]);
}

#[test]
fn passthrough_events_map() {
    let mut state = TerminalState::new(6, 20);
    let cases: [(&[u8], TermEvent); 8] = [
        (b"\x1b]0;first\x07", TermEvent::Title("first".into())),
        (b"\x1b]2;second\x07", TermEvent::Title("second".into())),
        (
            b"\x1b]52;c;SGVsbG8=\x07",
            TermEvent::ClipboardWrite("c;SGVsbG8=".into()),
        ),
        (b"\x1b]52;c;?\x07", TermEvent::ClipboardWrite("c;?".into())),
        (b"\x07", TermEvent::Bell),
        (
            b"\x1b]9;build done\x07",
            TermEvent::Notification("build done".into()),
        ),
        (
            b"\x1b]7;file:///tmp\x07",
            TermEvent::Cwd("file:///tmp".into()),
        ),
        (b"\x1b[3J", TermEvent::ScrollbackCleared),
    ];
    for (bytes, event) in cases {
        assert_eq!(state.feed(bytes), vec![event], "{bytes:?}");
        assert_eq!(
            state.feed(b"x"),
            vec![],
            "a second feed returns only new events"
        );
    }

    assert_eq!(
        state.feed(b"\x1b[?2004h\x1b[?1h\x1b[?1004h\x1b]8;;https://example.com\x07"),
        vec![]
    );
    assert_eq!(
        state.feed(b"\x07\x1b]0;a\x07\x07"),
        vec![
            TermEvent::Bell,
            TermEvent::Title("a".into()),
            TermEvent::Bell
        ]
    );
}

#[test]
fn split_utf8_across_feeds() {
    let mut state = TerminalState::new(2, 4);
    state.feed(&[0xE2, 0x94]);
    assert_eq!(buf_rows(&draw_state(&state, false))[0], "    ");
    state.feed(&[0x82, b'x']);
    assert_eq!(buf_rows(&draw_state(&state, false))[0], "│x  ");
}

#[test]
fn resize_contract() {
    let mut state = TerminalState::new(4, 10);
    state.feed(b"\x1b[?1049hhi");
    state.resize(2, 5);
    assert_eq!(state.size(), (2, 5));
    assert!(state.modes().alt_screen);
    let area = Rect::new(3, 1, 5, 2);
    let at = state.cursor(area).expect("cursor inside the area");
    assert!(area.contains(at), "{at:?}");
    state.feed(b"\x1b[1;1HZZ");
    assert_eq!(buf_rows(&draw_state(&state, false))[0], "ZZ   ");

    state.resize(6, 20);
    assert_eq!(state.size(), (6, 20));
    assert!(state.modes().alt_screen);
    state.feed(b"\x1b[6;20Hq");
    let buf = draw_state(&state, false);
    assert_eq!(buf[(19, 5)].symbol(), "q");

    state.resize(0, 0);
    assert_eq!(state.size(), (1, 1));
    state.feed(b"still alive\r\n");
    assert_eq!(TerminalState::new(0, 0).size(), (1, 1));
    TerminalState::new(0, 0).feed(b"no panic\r\n");
}

#[test]
fn view_reports_soft_wraps() {
    let mut state = TerminalState::new(5, 10);
    state.feed(b"ABCDEFGHIJKLMNOPQRSTUVWXY\r\nab\r\ncd");
    let view = state.view(0);
    let wraps: Vec<_> = (0..5).map(|row| view.row_wrap(row)).collect();
    assert_eq!(
        wraps,
        [
            Some(RowWrap::Hard),
            Some(RowWrap::Soft),
            Some(RowWrap::Soft),
            Some(RowWrap::Hard),
            Some(RowWrap::Hard),
        ]
    );
    assert_eq!(view.row_wrap(5), None);
    assert_eq!(view_rows(&view)[2], "UVWXY     ");
}

#[test]
fn the_program_turns_focus_events_and_mouse_reporting_on_and_off() {
    let mut state = TerminalState::new(6, 20);
    assert!(!state.focus_events());
    assert!(!state.mouse_reporting());

    state.feed(b"\x1b[?1004h");
    assert!(state.focus_events());
    assert!(!state.mouse_reporting(), "focus events are not the mouse");
    state.feed(b"\x1b[?1004l");
    assert!(!state.focus_events());

    for mode in ["1000", "1002", "1003"] {
        state.feed(format!("\x1b[?{mode}h").as_bytes());
        assert!(state.mouse_reporting(), "mode {mode}");
        state.feed(format!("\x1b[?{mode}l").as_bytes());
        assert!(!state.mouse_reporting(), "mode {mode} off");
    }
    state.feed(b"\x1b[?1006h");
    assert!(
        !state.mouse_reporting(),
        "an encoding without a protocol mode reports nothing"
    );
}

#[test]
fn clear_scrollback_drops_the_history_and_keeps_the_screen() {
    let mut state = history_state();
    assert_eq!(state.scrollback_len(), 24);
    let pushed = state.scrolled_off();
    state.set_scrollback(6);

    state.clear_scrollback();

    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(state.scrollback(), 0, "the view is back at the live rows");
    assert_eq!(view_rows(&state.view(0)), numbered(25, 30));
    assert_eq!(
        state.scrolled_off(),
        pushed,
        "rows pushed since the start is a count of the past, not of what is held"
    );
    assert_eq!(state.feed(b"\r\nL31"), vec![]);
    assert_eq!(state.scrollback_len(), 1, "history starts again");
}

fn encoded(code: KeyCode, mods: KeyModifiers, app_cursor: bool) -> Option<Vec<u8>> {
    let modes = Modes {
        app_cursor,
        ..Modes::default()
    };
    encode_key(&Key { code, mods }, &modes)
}

#[test]
fn encode_key_writes_the_xterm_modifier_forms() {
    const NONE: KeyModifiers = KeyModifiers::NONE;
    const SHIFT: KeyModifiers = KeyModifiers::SHIFT;
    const ALT: KeyModifiers = KeyModifiers::ALT;
    const CTRL: KeyModifiers = KeyModifiers::CONTROL;
    let both = SHIFT.union(CTRL);
    let all = SHIFT.union(ALT).union(CTRL);
    let table: &[(KeyCode, KeyModifiers, &str)] = &[
        (KeyCode::Up, NONE, "\x1b[A"),
        (KeyCode::Left, SHIFT, "\x1b[1;2D"),
        (KeyCode::Left, CTRL, "\x1b[1;5D"),
        (KeyCode::Right, CTRL, "\x1b[1;5C"),
        (KeyCode::Up, both, "\x1b[1;6A"),
        (KeyCode::Down, all, "\x1b[1;8B"),
        (KeyCode::Home, CTRL, "\x1b[1;5H"),
        (KeyCode::End, SHIFT, "\x1b[1;2F"),
        (KeyCode::Up, ALT, "\x1b\x1b[A"),
        (KeyCode::PageUp, NONE, "\x1b[5~"),
        (KeyCode::PageUp, SHIFT, "\x1b[5;2~"),
        (KeyCode::PageDown, CTRL, "\x1b[6;5~"),
        (KeyCode::Delete, CTRL, "\x1b[3;5~"),
        (KeyCode::Delete, both, "\x1b[3;6~"),
        (KeyCode::Insert, SHIFT, "\x1b[2;2~"),
        (KeyCode::F(1), NONE, "\x1bOP"),
        (KeyCode::F(1), SHIFT, "\x1b[1;2P"),
        (KeyCode::F(4), CTRL, "\x1b[1;5S"),
        (KeyCode::F(5), NONE, "\x1b[15~"),
        (KeyCode::F(5), CTRL, "\x1b[15;5~"),
        (KeyCode::F(12), all, "\x1b[24;8~"),
        (KeyCode::Backspace, NONE, "\x7f"),
        (KeyCode::Backspace, CTRL, "\x08"),
        (KeyCode::Backspace, ALT, "\x1b\x7f"),
        (KeyCode::Enter, SHIFT, "\r"),
        (KeyCode::Char('c'), CTRL, "\x03"),
        (KeyCode::Char('\\'), CTRL, "\x1c"),
        (KeyCode::Char(']'), CTRL, "\x1d"),
        (KeyCode::Char('^'), CTRL, "\x1e"),
        (KeyCode::Char('_'), CTRL, "\x1f"),
        (KeyCode::Char('/'), CTRL, "\x1f"),
        (KeyCode::Char('@'), CTRL, "\x00"),
        (KeyCode::Char(' '), CTRL, "\x00"),
        (KeyCode::Char('\\'), NONE, "\\"),
        (KeyCode::Char('x'), ALT, "\x1bx"),
    ];
    for (code, mods, want) in table {
        assert_eq!(
            encoded(*code, *mods, false).as_deref(),
            Some(want.as_bytes()),
            "{code:?} with {mods:?}"
        );
    }
}

#[test]
fn encode_key_follows_application_cursor_mode_only_without_modifiers() {
    assert_eq!(
        encoded(KeyCode::Up, KeyModifiers::NONE, true).as_deref(),
        Some(&b"\x1bOA"[..])
    );
    assert_eq!(
        encoded(KeyCode::End, KeyModifiers::NONE, true).as_deref(),
        Some(&b"\x1bOF"[..])
    );
    assert_eq!(
        encoded(KeyCode::Left, KeyModifiers::CONTROL, true).as_deref(),
        Some(&b"\x1b[1;5D"[..]),
        "xterm sends the parameter form in application mode too"
    );
}

#[test]
fn a_feed_that_enters_and_leaves_the_alternate_screen_adds_nothing_to_a_full_history() {
    let mut state = TerminalState::with_scrollback(6, 3, 10);
    state.feed(&lines(1, 40));
    assert_eq!(state.scrollback_len(), 10);
    let pushed = state.scrolled_off();

    let mut transient = b"\x1b[?1049h".to_vec();
    transient.extend(lines(1, 40));
    transient.extend(b"\x1b[?1049l");
    state.feed(&transient);

    assert_eq!(state.scrolled_off(), pushed);
    assert_eq!(state.scrollback_len(), 10);
}

#[test]
fn a_screen_switch_split_over_two_feeds_is_still_attributed() {
    let mut state = TerminalState::with_scrollback(6, 3, 10);
    state.feed(&lines(1, 40));
    let pushed = state.scrolled_off();

    state.feed(b"\x1b[?10");
    let mut entered = b"49h".to_vec();
    entered.extend(lines(1, 40));
    state.feed(&entered);
    state.feed(b"\x1b[?1049");
    assert_eq!(
        state.scrolled_off(),
        pushed,
        "still on the alternate screen"
    );
    let mut left = b"l".to_vec();
    left.extend(lines(1, 3));
    state.feed(&left);
    assert_eq!(
        state.scrolled_off(),
        pushed + 3,
        "the main screen counts again"
    );
}

#[test]
fn rows_scrolled_off_the_main_screen_are_counted_after_a_transient_alternate_screen_in_the_same_feed()
 {
    let mut state = TerminalState::with_scrollback(6, 3, 10);
    state.feed(&lines(1, 40));
    let pushed = state.scrolled_off();

    let mut mixed = lines(1, 4);
    mixed.extend(b"\x1b[?1049h");
    mixed.extend(lines(1, 40));
    mixed.extend(b"\x1b[?1049l");
    mixed.extend(lines(1, 2));
    state.feed(&mixed);

    assert_eq!(state.scrolled_off(), pushed + 6);
}

#[test]
fn rows_scroll_off_the_screen_even_when_no_history_is_kept() {
    let mut state = TerminalState::with_scrollback(6, 3, 0);
    state.feed(&lines(1, 60));
    assert_eq!(state.scrollback_len(), 0);
    assert_eq!(
        state.scrolled_off(),
        55,
        "a row that left the screen counts although no history holds it"
    );

    state.feed(b"\x1b[?1049h");
    state.feed(&lines(1, 20));
    state.feed(b"\x1b[?1049l");
    assert_eq!(state.scrolled_off(), 55);
}

#[test]
fn the_cursor_stays_visible_on_a_cell_that_is_already_inverse() {
    let mut state = TerminalState::new(3, 5);
    state.feed(b"\x1b[7mX\x1b[0m\x1b[1D");
    assert_eq!(state.view(0).cursor(), (0, 0));

    let unfocused = draw_state(&state, false);
    assert!(unfocused[(0, 0)].modifier.contains(Modifier::REVERSED));

    let focused = draw_state(&state, true);
    assert!(
        !focused[(0, 0)].modifier.contains(Modifier::REVERSED),
        "the cursor flips the inverse cell, so it shows apart from the unfocused look"
    );
}

#[test]
fn strikethrough_text_is_drawn_crossed_out() {
    let mut state = TerminalState::new(2, 8);
    state.feed(b"\x1b[9mgone\x1b[0m ok");

    let buf = draw_state(&state, false);

    for x in 0..4 {
        assert!(
            buf[(x, 0)].modifier.contains(Modifier::CROSSED_OUT),
            "col {x}"
        );
    }
    assert!(!buf[(5, 0)].modifier.contains(Modifier::CROSSED_OUT));
}

fn sentinel(area: Rect) -> Buffer {
    let mut buf = Buffer::empty(area);
    for cell in &mut buf.content {
        cell.set_symbol("#")
            .set_style(Style::new().fg(Color::Magenta));
    }
    buf
}

fn row_symbols(buf: &Buffer, y: u16, from: u16, to: u16) -> Vec<String> {
    (from..to)
        .map(|x| buf[(x, y)].symbol().to_owned())
        .collect()
}

fn assert_symbols_fit(buf: &Buffer, pane: Rect) {
    for y in pane.top()..pane.bottom() {
        for x in pane.left()..pane.right() {
            let width = buf[(x, y)].symbol().cell_width();
            assert!(
                width <= pane.right() - x,
                "symbol {:?} at ({x}, {y}) is {width} cells wide and leaves the pane",
                buf[(x, y)].symbol()
            );
        }
    }
}

#[test]
fn issue_terminal_wide_edge_is_blank() {
    let mut state = TerminalState::new(1, 6);
    state.feed("ab\u{754c}".as_bytes());
    let pane = Rect::new(2, 1, 3, 1);
    let mut buf = sentinel(Rect::new(0, 0, 9, 3));
    let before = buf.clone();

    TerminalView::new(&state).render(pane, &mut buf);

    assert_eq!(row_symbols(&buf, 1, 2, 5), ["a", "b", " "]);
    assert_symbols_fit(&buf, pane);
    for y in 0..3 {
        for x in 0..9 {
            if !pane.contains((x, y).into()) {
                assert_eq!(
                    buf[(x, y)],
                    before[(x, y)],
                    "({x}, {y}) is outside the pane"
                );
            }
        }
    }
}

#[test]
fn issue_terminal_wide_edge_blank_keeps_the_cell_style() {
    let mut state = TerminalState::new(1, 6);
    state.feed("a\x1b[41m\u{754c}".as_bytes());
    let mut buf = Buffer::empty(Rect::new(0, 0, 6, 1));

    TerminalView::new(&state).render(Rect::new(0, 0, 2, 1), &mut buf);

    assert_eq!(buf[(1, 0)].symbol(), " ");
    assert_eq!(buf[(1, 0)].bg, Color::Indexed(1));
}

#[test]
fn issue_terminal_complete_wide_preserves_existing_style() {
    let mut state = TerminalState::new(1, 6);
    state.feed("\x1b[41m\u{65e5}\x1b[0me\u{301}\u{2764}\u{fe0f}".as_bytes());

    for width in [4, 6] {
        let pane = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(pane);
        TerminalView::new(&state).render(pane, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "\u{65e5}", "width {width}");
        assert_eq!(buf[(0, 0)].bg, Color::Indexed(1), "width {width}");
        assert_eq!(buf[(1, 0)].symbol(), " ", "width {width}");
        assert_eq!(buf[(1, 0)].bg, Color::Reset, "width {width}");
        assert_eq!(buf[(2, 0)].symbol(), "e\u{301}", "width {width}");
        assert_symbols_fit(&buf, pane);
    }
    let pane = Rect::new(0, 0, 6, 1);
    let mut buf = Buffer::empty(pane);
    TerminalView::new(&state).render(pane, &mut buf);
    assert_eq!(buf[(3, 0)].symbol(), "\u{2764}\u{fe0f}");

    let narrow = Rect::new(0, 0, 4, 1);
    let mut buf = Buffer::empty(narrow);
    TerminalView::new(&state).render(narrow, &mut buf);
    assert_eq!(
        buf[(3, 0)].symbol(),
        " ",
        "the emoji starts in the last column"
    );
}

#[test]
fn issue_terminal_clip_preserves_source_origin() {
    let mut state = TerminalState::new(3, 8);
    state.feed("01234567\r\nab\u{754c}cdef\r\nABCDEFGH".as_bytes());
    let area = Rect::new(0, 0, 8, 3);

    let mut buf = Buffer::empty(Rect::new(2, 1, 5, 2));
    TerminalView::new(&state).render(area, &mut buf);
    assert_eq!(row_symbols(&buf, 1, 2, 7), ["\u{754c}", " ", "c", "d", "e"]);
    assert_eq!(row_symbols(&buf, 2, 2, 7), ["C", "D", "E", "F", "G"]);

    let mut buf = Buffer::empty(Rect::new(3, 1, 4, 1));
    TerminalView::new(&state).render(area, &mut buf);
    assert_eq!(
        row_symbols(&buf, 1, 3, 7),
        [" ", "c", "d", "e"],
        "the first visible cell continues a wide glyph that starts offscreen"
    );
}

#[test]
fn issue_terminal_zero_and_disjoint_area() {
    let mut state = TerminalState::new(3, 6);
    state.feed(b"abc");
    let buf = sentinel(Rect::new(0, 0, 6, 4));

    for area in [
        Rect::new(1, 1, 0, 2),
        Rect::new(1, 1, 3, 0),
        Rect::new(20, 20, 3, 3),
        Rect::new(0, 8, 6, 3),
    ] {
        let mut painted = buf.clone();
        TerminalView::new(&state)
            .focused(true)
            .render(area, &mut painted);
        assert_eq!(painted, buf, "{area:?}");
    }

    let mut painted = buf.clone();
    TerminalView::new(&state)
        .focused(true)
        .render(Rect::new(0, 0, 6, 8), &mut painted);
    assert_eq!(row_symbols(&painted, 0, 0, 3), ["a", "b", "c"]);
}

#[test]
fn issue_terminal_cursor_outside_the_buffer_is_not_painted() {
    let mut state = TerminalState::new(2, 8);
    state.feed(b"abcdef");
    let mut buf = Buffer::empty(Rect::new(0, 0, 4, 2));

    TerminalView::new(&state)
        .focused(true)
        .render(Rect::new(0, 0, 8, 2), &mut buf);

    assert_eq!(row_symbols(&buf, 0, 0, 4), ["a", "b", "c", "d"]);
    assert!(
        buf.content
            .iter()
            .all(|cell| !cell.modifier.contains(Modifier::REVERSED))
    );
}

#[test]
fn issue_terminal_narrow_redraw_preserves_neighbor() {
    let mut state = TerminalState::new(1, 6);
    state.feed("ab\u{754c}cd".as_bytes());
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).unwrap();

    let frame = |terminal: &mut Terminal<TestBackend>, pane: Rect, neighbor_x: u16| {
        terminal
            .draw(|f| {
                TerminalView::new(&state).render(pane, f.buffer_mut());
                let neighbor = Rect::new(neighbor_x, 0, 12 - neighbor_x, 1);
                for (i, ch) in "RIGHT....."
                    .chars()
                    .take(neighbor.width as usize)
                    .enumerate()
                {
                    f.buffer_mut()[(neighbor.x + i as u16, 0)].set_char(ch);
                }
            })
            .unwrap();
        terminal.backend().buffer().clone()
    };

    let wide = frame(&mut terminal, Rect::new(0, 0, 6, 1), 6);
    assert_eq!(
        row_symbols(&wide, 0, 0, 6),
        ["a", "b", "\u{754c}", " ", "c", "d"]
    );

    let narrow_pane = Rect::new(0, 0, 3, 1);
    let narrow = frame(&mut terminal, narrow_pane, 3);
    assert_eq!(row_symbols(&narrow, 0, 0, 3), ["a", "b", " "]);
    assert_eq!(row_symbols(&narrow, 0, 3, 8), ["R", "I", "G", "H", "T"]);
    assert_symbols_fit(&narrow, narrow_pane);

    let cleared = frame(&mut terminal, Rect::new(0, 0, 0, 1), 3);
    assert_eq!(row_symbols(&cleared, 0, 0, 3), [" ", " ", " "]);
    assert_eq!(row_symbols(&cleared, 0, 3, 8), ["R", "I", "G", "H", "T"]);
}
