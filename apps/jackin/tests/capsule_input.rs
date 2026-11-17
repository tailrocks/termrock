//! What the Capsule lets through to a terminal pane: the keys, the wheel and
//! the paste, and what it keeps for itself.

mod capsule_support;
mod pane_support;
mod product_support;

use capsule_support::*;
use jackin_console::panes::PaneCommand;
use pane_support::*;
use termrock::core::event::MouseKind;

fn pane_cell(rig: &mut Rig) -> (u16, u16) {
    let at = rig.locate("line one");
    (at.0 + 2, at.1)
}

#[test]
fn ctrl_b_ctrl_b_sends_the_prefix_key() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b");
    let hints = rig.squashed();
    assert!(hints.contains("Ctrl+B Ctrl+B Send Ctrl+B"), "{hints}");
    rig.press("ctrl+b");
    assert_eq!(rig.bench.commands(0), [input(&[0x02])], "exactly 0x02");
    rig.press("ctrl+d");
    assert_eq!(rig.bench.commands(0).len(), 1, "Ctrl+D still sends nothing");
    assert!(
        !rig.squashed().contains("Send Ctrl+B"),
        "the prefix wait is over"
    );
}

#[test]
fn y_after_a_drag_reaches_the_program() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one\r\nline two");
    rig.advance(80);
    let from = pane_cell(&mut rig);
    rig.mouse(MouseKind::Down, from);
    rig.mouse(MouseKind::Drag, (from.0 + 5, from.1));
    rig.mouse(MouseKind::Up, (from.0 + 5, from.1));
    assert_eq!(
        *rig.copied.0.lock().expect("sink"),
        ["ne on"],
        "release copies"
    );
    assert!(
        rig.app.world.daemons[&instance_id()].panes[0]
            .body
            .viewport()
            .has_selection()
    );

    rig.press("y");
    assert_eq!(rig.bench.commands(0), [input(b"y")], "y is the program's");
    assert!(
        !rig.app.world.daemons[&instance_id()].panes[0]
            .body
            .viewport()
            .has_selection()
    );
    assert_eq!(
        rig.copied.0.lock().expect("sink").len(),
        1,
        "no second copy"
    );
}

#[test]
fn esc_clears_a_selection_and_otherwise_reaches_the_program() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one");
    rig.advance(80);
    rig.press("esc");
    assert_eq!(
        rig.bench.commands(0),
        [input(&[0x1b])],
        "no selection: forwarded"
    );

    let from = pane_cell(&mut rig);
    rig.mouse(MouseKind::Down, from);
    rig.mouse(MouseKind::Drag, (from.0 + 3, from.1));
    rig.mouse(MouseKind::Up, (from.0 + 3, from.1));
    rig.press("esc");
    assert_eq!(
        rig.bench.commands(0).len(),
        1,
        "Esc cleared the selection and stopped"
    );
    assert!(
        !rig.app.world.daemons[&instance_id()].panes[0]
            .body
            .viewport()
            .has_selection()
    );
}

#[test]
fn copy_selection_shows_no_y_for_a_terminal_pane() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one");
    rig.advance(80);
    rig.press("f10 right");
    let menu = rig.screen();
    let row = menu
        .lines()
        .find(|l| l.contains("Copy selection"))
        .expect("row");
    let at = row.find("Copy selection").expect("the label");
    let after: String = row[at + "Copy selection".len()..]
        .chars()
        .take(20)
        .collect();
    assert!(!after.contains('y'), "no shortcut after the label: {row}");
}

#[test]
fn the_pane_menu_shows_no_y_for_a_terminal_pane() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one");
    rig.advance(80);
    let at = pane_cell(&mut rig);
    rig.mouse(MouseKind::Secondary, at);
    let menu = rig.screen();
    let row = menu
        .lines()
        .find(|l| l.contains("Copy selection"))
        .expect("the pane menu is open");
    let start = row.find("Copy selection").expect("the label");
    let after: String = row[start + "Copy selection".len()..]
        .chars()
        .take(20)
        .collect();
    assert!(!after.contains('y'), "no shortcut after the label: {row}");
}

#[test]
fn alt_screen_wheel_sends_arrows() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"line one");
    rig.advance(80);
    let at = pane_cell(&mut rig);
    rig.mouse(MouseKind::WheelUp, at);
    assert_eq!(
        rig.bench.commands(0),
        [],
        "the primary screen scrolls history"
    );

    rig.bench.output(0, b"\x1b[?1049h");
    rig.advance(80);
    rig.mouse(MouseKind::WheelUp, at);
    assert_eq!(rig.bench.commands(0), vec![input(b"\x1b[A"); 3]);
    rig.bench.output(0, b"\x1b[?1000h");
    rig.advance(80);
    rig.mouse(MouseKind::WheelUp, at);
    assert_eq!(
        rig.bench.commands(0).len(),
        3,
        "with mouse reporting on: nothing"
    );
}

#[test]
fn a_multi_line_paste_without_protection_asks_first() {
    let mut rig = Rig::new();
    rig.attach();
    rig.paste("one\ntwo\nthree");
    let dialog = rig.flat();
    assert!(
        dialog.contains(
            "Paste 3 lines? This pane has no paste protection, so each line runs as you paste it."
        ),
        "{dialog}"
    );
    assert_eq!(rig.bench.commands(0), [], "nothing before the answer");

    rig.press("esc");
    assert_eq!(rig.bench.commands(0), [], "Cancel sends nothing");

    rig.paste("one\ntwo\nthree");
    rig.press("enter");
    assert_eq!(
        rig.bench.commands(0),
        [],
        "Enter is on Cancel: a stray key pastes nothing"
    );

    rig.paste("one\ntwo\nthree");
    rig.press("tab enter");
    assert_eq!(rig.bench.commands(0), [input(b"one\ntwo\nthree")]);
}

#[test]
fn a_paste_into_a_protected_pane_goes_straight_in() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"\x1b[?2004h");
    rig.advance(80);
    rig.paste("one\ntwo");
    assert_eq!(
        rig.bench.commands(0),
        [input(b"\x1b[200~one\ntwo\x1b[201~")]
    );
    rig.paste("single");
    assert_eq!(rig.bench.commands(0).len(), 2);
}

#[test]
fn a_paste_the_link_did_not_take_says_so() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.fill(0, true);
    rig.paste("hello");
    assert_eq!(rig.status(), "Paste not sent · the pane is busy, try again");
    assert_eq!(rig.bench.commands(0), []);
}

#[test]
fn a_paste_over_the_limit_is_refused_and_says_so() {
    let mut rig = Rig::new();
    rig.attach();
    rig.paste(&"x".repeat(64 * 1024 + 1));
    assert_eq!(rig.status(), "Paste too large · 65537 bytes");
    assert_eq!(rig.bench.commands(0), []);
}

#[test]
fn paste_last_copy_takes_the_same_path() {
    let mut rig = Rig::new();
    rig.attach();
    rig.app.world.clipboard = Some("a\nb".into());
    rig.press("f10 right down enter");
    let dialog = rig.flat();
    assert!(dialog.contains("Paste 2 lines?"), "{dialog}");
    rig.press("tab enter");
    assert_eq!(
        rig.bench.commands(0),
        [PaneCommand::Input(b"a\nb".to_vec())]
    );
    assert_eq!(rig.status(), "Pasted into the focused pane");
}

#[test]
fn a_drag_selects_in_a_pane_whose_id_is_past_sixty_four() {
    use jackin_console::panes::{EndKind, Lease, LinkEvent};

    let mut rig = Rig::new();
    rig.attach();
    for _ in 0..70 {
        rig.press("ctrl+b %");
        rig.typed("Shell");
        rig.press("enter");
        rig.advance(80);
        let n = rig.bench.opened() - 1;
        rig.bench.push(n, LinkEvent::Ended(EndKind::Detached));
        rig.advance(80);
        rig.press("ctrl+b x");
        assert_eq!(rig.daemon().panes.len(), 1, "closed at once");
    }

    rig.press("ctrl+b %");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(80);
    let n = rig.bench.opened() - 1;
    rig.bench.push(n, LinkEvent::Attached);
    rig.bench.push(n, LinkEvent::Lease(Lease::Owner));
    rig.advance(900);
    let pid = rig
        .daemon()
        .focused_pane()
        .expect("the new pane is focused");
    assert!(pid > 64, "the pane id outgrew the old search range: {pid}");

    rig.bench.output(n, b"line one\r\nline two");
    rig.advance(80);
    let from = pane_cell(&mut rig);
    rig.mouse(MouseKind::Down, from);
    rig.mouse(MouseKind::Drag, (from.0 + 5, from.1));
    rig.mouse(MouseKind::Up, (from.0 + 5, from.1));
    assert_eq!(
        *rig.copied.0.lock().expect("sink"),
        ["ne on"],
        "release copies the dragged text"
    );
}
