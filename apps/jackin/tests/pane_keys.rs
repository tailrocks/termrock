//! The keys, the wheel and the paste of a terminal pane, held against the
//! hazards of a real program on the other end.

mod pane_support;

use jackin_console::panes::{Lease, LinkEvent, PaneBody, PaneKey};
use pane_support::*;
use termrock::core::event::Outcome;
use termrock::core::hit::Axis;

#[test]
fn ctrl_b_ctrl_b_sends_the_prefix_key() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    assert!(pane.sends_prefix());
    assert_eq!(pane.key(&key("ctrl+b"), 1_000), PaneKey::Forwarded);
    assert_eq!(bench.commands(0), [input(&[0x02])], "exactly 0x02");
    assert_eq!(pane.key(&key("ctrl+d"), 1_100), PaneKey::Consumed);
    assert_eq!(
        bench.commands(0).len(),
        1,
        "the next Ctrl+D still sends nothing"
    );
}

#[test]
fn any_key_after_a_selection_clears_it_and_is_forwarded() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"select me");
    pane.tick(900);
    pane.viewport_mut()
        .select_word_at(ratatui::layout::Position::new(1, 0));
    assert!(pane.viewport().has_selection());
    assert_eq!(pane.key(&key("y"), 1_000), PaneKey::Forwarded);
    assert!(!pane.viewport().has_selection());
    assert_eq!(bench.commands(0), [input(b"y")], "y reaches the program");
    assert!(!pane.viewport().keyboard_copy(), "there is no y shortcut");
}

#[test]
fn a_key_while_scrolled_back_returns_to_the_tail_and_is_forwarded() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    let lines: Vec<u8> = (0..100)
        .flat_map(|n| format!("l{n}\r\n").into_bytes())
        .collect();
    bench.output(0, &lines);
    pane.tick(900);
    pane.viewport_mut().wheel(Axis::Vertical, -3);
    assert!(!pane.viewport().follow());
    assert_eq!(pane.key(&key("x"), 1_000), PaneKey::Forwarded);
    assert!(pane.viewport().follow());
    assert_eq!(bench.commands(0), [input(b"x")]);
}

fn arrows(n: usize, bytes: &[u8]) -> Vec<jackin_console::panes::PaneCommand> {
    (0..n).map(|_| input(bytes)).collect()
}

#[test]
fn a_viewing_pane_scrolls_nothing_into_the_program_but_a_key_still_types() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[?1049h");
    bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    pane.tick(900);
    assert_eq!(pane.wheel(Axis::Vertical, -3), Some(Outcome::Consumed));
    assert_eq!(
        bench.commands(0),
        [],
        "a wheel on a viewer is not a keystroke"
    );
    assert_eq!(pane.key(&key("x"), 1_000), PaneKey::Forwarded);
    assert_eq!(bench.commands(0), [input(b"x")], "typing takes the lease");
}

#[test]
fn alt_screen_wheel_sends_arrows() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    assert_eq!(
        pane.wheel(Axis::Vertical, -3),
        None,
        "the primary screen scrolls its history"
    );
    assert_eq!(bench.commands(0), []);

    bench.output(0, b"\x1b[?1049h");
    pane.tick(900);
    assert_eq!(pane.wheel(Axis::Vertical, -3), Some(Outcome::Changed));
    assert_eq!(bench.commands(0), arrows(3, b"\x1b[A"));
    pane.wheel(Axis::Vertical, 3);
    assert_eq!(bench.commands(0)[3..], arrows(3, b"\x1b[B")[..]);

    bench.output(0, b"\x1b[?1h");
    pane.tick(1_000);
    pane.wheel(Axis::Vertical, -3);
    assert_eq!(
        bench.commands(0)[6..],
        arrows(3, b"\x1bOA")[..],
        "application cursor"
    );

    assert_eq!(
        pane.wheel(Axis::Horizontal, 3),
        Some(Outcome::Consumed),
        "no side scroll on the alternate screen"
    );
    assert_eq!(bench.commands(0).len(), 9);

    bench.output(0, b"\x1b[?1000h");
    pane.tick(1_100);
    assert_eq!(pane.wheel(Axis::Vertical, -3), Some(Outcome::Consumed));
    assert_eq!(
        bench.commands(0).len(),
        9,
        "with mouse reporting on it does nothing"
    );
}

#[test]
fn mouse_mode_on_the_primary_screen_still_scrolls_history() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[?1000h");
    pane.tick(900);
    assert_eq!(pane.wheel(Axis::Vertical, -3), None);
    assert_eq!(bench.commands(0), [], "no mouse bytes reach the program");
}

#[test]
fn kitty_query_answer_is_legacy() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[?u");
    pane.tick(900);
    assert_eq!(
        bench.commands(0),
        [jackin_console::panes::PaneCommand::Reply(
            b"\x1b[?0u".to_vec()
        )]
    );
    let enter = jackin_console::panes::PaneBody::key(
        &mut pane,
        &termrock::core::event::Key {
            code: ratatui::crossterm::event::KeyCode::Enter,
            mods: ratatui::crossterm::event::KeyModifiers::SHIFT,
        },
        1_000,
    );
    assert_eq!(enter, PaneKey::Forwarded);
    assert_eq!(bench.commands(0)[1], input(b"\r"), "Shift+Enter is Enter");
}

fn attaching(bench: &std::sync::Arc<Bench>) -> jackin_console::panes::TerminalPane {
    let mut pane = pane(bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.output(0, b"\x1b[?1049h\x1b[?1004h");
    pane.tick(0);
    pane.tick(900);
    assert_eq!(
        pane.status(),
        &jackin_console::panes::LinkStatus::Attaching,
        "no lease has arrived"
    );
    pane
}

#[test]
fn a_wheel_and_a_focus_report_wait_for_the_lease() {
    let bench = Bench::new();
    let mut pane = attaching(&bench);
    assert_eq!(pane.wheel(Axis::Vertical, -3), Some(Outcome::Consumed));
    pane.focus_changed(true, 1_000);
    assert_eq!(bench.commands(0), [], "not the owner yet");

    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(1_100);
    assert_eq!(pane.wheel(Axis::Vertical, -3), Some(Outcome::Changed));
    pane.focus_changed(false, 1_200);
    let mut sent = arrows(3, b"\x1b[A");
    sent.push(input(b"\x1b[O"));
    assert_eq!(bench.commands(0), sent);
}

#[test]
fn a_key_and_a_paste_type_before_the_lease_is_known() {
    let bench = Bench::new();
    let mut pane = attaching(&bench);
    assert_eq!(pane.key(&key("x"), 1_000), PaneKey::Forwarded);
    assert_eq!(
        pane.paste("hi", 1_100),
        jackin_console::panes::PasteOutcome::Sent
    );
    assert_eq!(
        bench.commands(0).len(),
        2,
        "typing is how a viewer takes over"
    );
}

#[test]
fn a_new_attach_forgets_the_lease_of_the_one_before() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[?1049h");
    pane.tick(900);
    bench.push(0, LinkEvent::Attached);
    pane.tick(1_000);
    pane.tick(2_000);
    bench.output(0, b"\x1b[6n");
    pane.tick(2_100);
    assert_eq!(pane.wheel(Axis::Vertical, -3), Some(Outcome::Consumed));
    assert_eq!(
        bench.commands(0),
        [],
        "the old lease says nothing about this attach"
    );

    bench.push(0, LinkEvent::Lease(Lease::Owner));
    bench.output(0, b"\x1b[6n");
    pane.tick(2_200);
    assert_eq!(bench.commands(0).len(), 1, "{:?}", bench.commands(0));
}
