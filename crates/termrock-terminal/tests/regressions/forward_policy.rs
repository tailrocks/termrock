#![cfg(feature = "terminal")]

use junie_tui::terminal::{ForwardPolicy, Paste};

const OPEN: &[u8] = b"\x1b[200~";
const CLOSE: &[u8] = b"\x1b[201~";

fn bracketed(body: &[u8]) -> Paste {
    Paste::Send([OPEN, body, CLOSE].concat())
}

#[test]
fn ctrl_d_and_ctrl_q_never_reach_the_program() {
    let policy = ForwardPolicy::default();
    assert_eq!(policy.key(vec![0x04]), None, "Ctrl+D");
    assert_eq!(policy.key(vec![0x11]), None, "Ctrl+Q");
    assert_eq!(policy.key(vec![0x1b, 0x04]), None, "Alt+Ctrl+D");
    assert_eq!(policy.key(vec![0x10]), Some(vec![0x10]), "Ctrl+P is typed");
    assert_eq!(policy.key(b"a".to_vec()), Some(b"a".to_vec()));
    assert_eq!(policy.key(b"\x1b[A".to_vec()), Some(b"\x1b[A".to_vec()));
}

#[test]
fn a_paste_drops_control_bytes_but_keeps_tab_and_newline() {
    let policy = ForwardPolicy::default();
    let text = "a\u{1}b\u{1b}c\u{4}d\te\r\nf";
    assert_eq!(
        policy.paste(text, true),
        bracketed(b"abcd\te\nf"),
        "wrapped in the markers when the program asked for them"
    );
    assert_eq!(
        policy.paste(text, false),
        Paste::NeedsConfirm { lines: 2 },
        "two lines without protection need a confirm"
    );
    assert_eq!(policy.paste("\u{1}\u{7f}", false), Paste::Send(vec![]));
}

#[test]
fn a_paste_cannot_close_the_bracket_early() {
    let policy = ForwardPolicy::default();
    let Paste::Send(bytes) = policy.paste("x\x1b[201~y; rm -rf /", true) else {
        panic!("a bracketed paste is sent");
    };
    assert_eq!(bytes, b"\x1b[200~x[201~y; rm -rf /\x1b[201~");
    let closes = bytes.windows(CLOSE.len()).filter(|w| *w == CLOSE).count();
    assert_eq!(closes, 1, "only the real end marker is left");
}

#[test]
fn one_line_is_sent_and_many_lines_ask_first() {
    let policy = ForwardPolicy::default();
    assert_eq!(policy.paste("hello", false), Paste::Send(b"hello".to_vec()));
    assert_eq!(
        policy.paste("a\nb\nc", false),
        Paste::NeedsConfirm { lines: 3 }
    );
    assert_eq!(
        policy.paste("a\nb\n", false),
        Paste::NeedsConfirm { lines: 2 },
        "a trailing newline does not add a line"
    );
    assert_eq!(
        policy.paste("a\n", false),
        Paste::NeedsConfirm { lines: 1 },
        "one line that ends in a newline would run at once"
    );
    assert_eq!(policy.paste("a\nb\nc", true), bracketed(b"a\nb\nc"));
}

#[test]
fn a_paste_over_the_limit_is_refused_by_its_byte_count() {
    let policy = ForwardPolicy::new(8);
    assert_eq!(
        policy.paste("12345678", false),
        Paste::Send(b"12345678".to_vec())
    );
    assert_eq!(
        policy.paste("123456789", false),
        Paste::TooLarge { bytes: 9 }
    );
    assert_eq!(
        policy.paste("éééééé", true),
        Paste::TooLarge { bytes: 12 },
        "bytes, not characters"
    );

    let default = ForwardPolicy::default();
    assert!(matches!(
        default.paste(&"x".repeat(64 * 1024), true),
        Paste::Send(_)
    ));
    assert_eq!(
        default.paste(&"x".repeat(64 * 1024 + 1), true),
        Paste::TooLarge {
            bytes: 64 * 1024 + 1
        }
    );
}

#[test]
fn a_confirmed_paste_is_sent_as_it_is_without_brackets() {
    let policy = ForwardPolicy::new(8);
    assert_eq!(
        policy.confirmed("a\x01\nb\x1bc"),
        Paste::Send(b"a\nbc".to_vec()),
        "the operator said yes, the control bytes still go"
    );
    assert_eq!(policy.confirmed("123456789"), Paste::TooLarge { bytes: 9 });
}

#[test]
fn every_line_break_is_one_newline() {
    let policy = ForwardPolicy::default();
    assert_eq!(policy.paste("one\rtwo", true), bracketed(b"one\ntwo"));
    assert_eq!(policy.paste("a\r\nb\rc\nd", true), bracketed(b"a\nb\nc\nd"));
    assert_eq!(
        policy.paste("one\rtwo\rthree", false),
        Paste::NeedsConfirm { lines: 3 },
        "lines that end in a bare carriage return are still lines"
    );
    assert_eq!(
        policy.paste("a\r\n", false),
        Paste::NeedsConfirm { lines: 1 },
        "CR LF is one break, not two"
    );
    assert_eq!(
        policy.confirmed("one\rtwo"),
        Paste::Send(b"one\ntwo".to_vec())
    );
}
