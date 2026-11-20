//! What a program may put on the clipboard (OSC 52) and what it may show in
//! the pane info dialog (its title).

mod pane_support;

use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use jackin_console::panes::{
    ClipboardWrite, EndKind, Lease, LinkEvent, LinkStatus, PaneBody, PaneEffect, PanePolicy,
    TerminalPane, sanitize_title,
};
use pane_support::*;
use termrock::theme::Tone;

const HELLO: &[u8] = b"\x1b]52;c;SGVsbG8=\x07";

fn osc52(selection: &str, data: &str) -> Vec<u8> {
    format!("\x1b]52;{selection};{data}\x07").into_bytes()
}

struct Rig {
    bench: Arc<Bench>,
    pane: TerminalPane,
}

/// An attached Codex pane that has the focus and was typed into at 1,000 ms.
fn rig(request: jackin_console::panes::PaneRequest, policy: PanePolicy) -> Rig {
    let bench = Bench::new();
    let mut pane = pane_for(request, &bench, policy);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    pane.tick(800);
    pane.focus_changed(true, 800);
    pane.key(&key("a"), 1_000);
    Rig { bench, pane }
}

impl Rig {
    fn codex() -> Self {
        rig(codex(), PanePolicy::default())
    }

    fn write(&mut self, at: i64, bytes: &[u8]) -> Vec<PaneEffect> {
        self.bench.output(0, bytes);
        self.pane.tick(at);
        self.pane.take_effects(&words())
    }
}

fn copied(text: &str, what: &str) -> PaneEffect {
    PaneEffect::Copy {
        text: text.to_owned(),
        what: what.to_owned(),
    }
}

fn blocked(text: &str) -> PaneEffect {
    PaneEffect::Status {
        text: text.to_owned(),
        tone: Tone::Warning,
    }
}

const WRITE_BLOCKED: &str = "Clipboard write from Codex blocked · select text to copy it";

#[test]
fn osc52_allowed_cases() {
    let mut rig = Rig::codex();
    assert_eq!(
        rig.write(2_000, HELLO),
        [copied("Hello", "from Codex · 5 characters")],
        "the status is the copy's own, every time"
    );
    assert_eq!(
        rig.write(4_000, HELLO),
        [copied("Hello", "from Codex · 5 characters")],
        "a second write 2 s later is allowed again"
    );
    assert_eq!(
        rig.write(4_100, &osc52("p", "SGVsbG8=")),
        [copied("Hello", "from Codex · 5 characters")],
        "the selection field is ignored"
    );
    assert_eq!(
        rig.write(4_200, &osc52("c", &STANDARD.encode("日本".as_bytes()))),
        [copied("日本", "from Codex · 2 characters")]
    );

    let exact = "x".repeat(100_000);
    let effects = rig.write(4_300, &osc52("c", &STANDARD.encode(exact.as_bytes())));
    assert_eq!(
        effects.len(),
        1,
        "100,000 decoded bytes (133,336 encoded) is at the cap, not over it"
    );
    assert!(matches!(&effects[0], PaneEffect::Copy { text, .. } if text.len() == 100_000));
}

#[test]
fn the_gesture_window_is_five_seconds_from_the_last_forwarded_key() {
    let mut rig = Rig::codex();
    assert_eq!(rig.write(5_999, HELLO).len(), 1, "4,999 ms after the key");
    let mut late = Rig::codex();
    assert_eq!(
        late.write(6_001, HELLO),
        [blocked(WRITE_BLOCKED)],
        "5,001 ms after the key"
    );

    let mut never = rig_without_gesture();
    assert_eq!(
        never.write(1_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "a pane never typed into is refused at a small clock"
    );
}

fn rig_without_gesture() -> Rig {
    let bench = Bench::new();
    let mut pane = live(&bench);
    pane.focus_changed(true, 800);
    Rig { bench, pane }
}

#[test]
fn only_a_key_or_a_click_is_a_gesture() {
    let mut wheel = rig_without_gesture();
    wheel.pane.wheel(termrock::core::hit::Axis::Vertical, -3);
    assert_eq!(
        wheel.write(1_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "the wheel"
    );

    let mut chord = rig_without_gesture();
    chord.pane.key(&key("ctrl+d"), 900);
    assert_eq!(
        chord.write(1_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "a key the filter drops was not forwarded"
    );

    let mut click = rig_without_gesture();
    click.pane.gesture(900);
    assert!(
        matches!(click.write(1_000, HELLO)[..], [PaneEffect::Copy { .. }]),
        "a click in the pane"
    );
}

#[test]
fn osc52_refusal_matrix() {
    let mut shell_pane = rig(shell(), PanePolicy::default());
    assert_eq!(
        shell_pane.write(2_000, HELLO),
        [blocked(
            "Clipboard write from Shell blocked · select text to copy it"
        )],
        "a shell pane never writes"
    );
    let mut shell_read = rig(shell(), PanePolicy::default());
    assert_eq!(
        shell_read.write(2_000, &osc52("c", "?")),
        [blocked("Clipboard read from Shell blocked")]
    );

    let mut unfocused = Rig::codex();
    unfocused.pane.focus_changed(false, 1_500);
    assert_eq!(
        unfocused.write(2_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "an unfocused pane or a pane in a background tab"
    );

    let mut read = Rig::codex();
    assert_eq!(
        read.write(2_000, &osc52("c", "?")),
        [blocked("Clipboard read from Codex blocked")]
    );
    assert!(
        read.bench
            .commands(0)
            .iter()
            .all(|c| !matches!(c, jackin_console::panes::PaneCommand::Reply(_))),
        "a read is never answered"
    );

    let mut garbage = Rig::codex();
    assert_eq!(
        garbage.write(2_000, &osc52("c", "!!!!")),
        [],
        "bad base64 is dropped"
    );
    assert_eq!(
        garbage.write(2_100, &osc52("c", &STANDARD.encode([0xff_u8, 0xfe, 0xfd]))),
        [],
        "bytes that are not UTF-8 are dropped"
    );

    let mut big = Rig::codex();
    let over = "x".repeat(100_001);
    assert_eq!(
        big.write(2_000, &osc52("c", &STANDARD.encode(over.as_bytes()))),
        [blocked("Clipboard write from Codex blocked · too large")]
    );

    let deny = PanePolicy {
        clipboard_write: ClipboardWrite::Deny,
        ..PanePolicy::default()
    };
    let mut denied = rig(codex(), deny);
    assert_eq!(
        denied.write(2_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "the demo policy refuses everything"
    );
}

#[test]
fn a_refusal_shows_its_status_at_most_once_every_ten_seconds() {
    let mut rig = Rig::codex();
    rig.pane.focus_changed(false, 1_000);
    assert_eq!(rig.write(1_000, HELLO), [blocked(WRITE_BLOCKED)]);
    assert_eq!(rig.write(5_000, HELLO), [], "4 s later shows nothing new");
    assert_eq!(rig.write(9_999, HELLO), [], "just inside the interval");
    assert_eq!(
        rig.write(11_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "11 s after the first shows the status again"
    );
}

#[test]
fn title_is_sanitised_and_shown() {
    assert_eq!(sanitize_title("a\u{9b}b\u{202e}c  \t d"), "abc d");
    assert_eq!(sanitize_title("\u{2066}x\u{2069}\n\n y\u{7}"), "x y");
    assert_eq!(sanitize_title("   "), "");
    let wide = "日".repeat(35);
    let cut = sanitize_title(&wide);
    assert_eq!(cut, "日".repeat(32), "70 columns are cut at 64");
    assert_eq!(sanitize_title(&"ab".repeat(40)), "ab".repeat(32));

    let mut rig = Rig::codex();
    assert_eq!(rig.pane.title(), None);
    rig.write(1_100, "\x1b]2;  build\u{202e}  done \x07".as_bytes());
    assert_eq!(rig.pane.title(), Some("build done"));
    rig.write(1_200, b"\x1b]0;second\x07");
    assert_eq!(rig.pane.title(), Some("second"));
}

#[test]
fn title_drops_marks_that_reorder_or_hide_text() {
    let spoofed = "a\u{200e}b\u{200f}c\u{61c}d\u{e0041}\u{e0042}e\u{e007f}";
    assert_eq!(sanitize_title(spoofed), "abcde");

    let mut rig = Rig::codex();
    rig.write(1_100, format!("\x1b]2;{spoofed}\x07").as_bytes());
    assert_eq!(rig.pane.title(), Some("abcde"));
}

/// A fresh Codex pane with the focus, attached at 0 ms, still in its replay
/// window: the link is `Attaching`.
fn replaying() -> Rig {
    let bench = Bench::new();
    let mut pane = pane_for(codex(), &bench, PanePolicy::default());
    pane.sync_size(inner(80, 24));
    pane.focus_changed(true, 0);
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    Rig { bench, pane }
}

#[test]
fn a_clipboard_write_in_the_replay_is_not_copied_after_a_key() {
    let mut rig = replaying();
    rig.pane.key(&key("a"), 100);
    assert_eq!(
        rig.bench.commands(0),
        [input(b"a")],
        "the key reached the link while it was attaching"
    );
    assert_eq!(
        rig.write(200, HELLO),
        [],
        "output replayed from before the key is not the program answering it"
    );
}

/// A Codex pane typed into at 1,000 ms, detached, and reattached by Enter at
/// 1,500 ms. The new link attached at 1,600 ms.
fn reattached() -> Rig {
    let mut rig = Rig::codex();
    rig.bench.push(0, LinkEvent::Ended(EndKind::Detached));
    rig.pane.tick(1_100);
    rig.pane.take_effects(&words());
    rig.pane.key(&key("enter"), 1_500);
    assert_eq!(rig.bench.opened(), 2, "Enter reattaches");
    rig.bench.push(1, LinkEvent::Attached);
    rig.bench.push(1, LinkEvent::Lease(Lease::Owner));
    rig.pane.tick(1_600);
    rig
}

#[test]
fn the_replay_of_a_reattach_is_not_copied() {
    let mut rig = reattached();
    rig.bench.output(1, HELLO);
    rig.pane.tick(2_000);
    assert_eq!(
        rig.pane.take_effects(&words()),
        [],
        "the key at 1,000 ms is 1 s old, and the replay is not the program answering it"
    );
}

#[test]
fn a_reattach_forgets_the_key_before_it() {
    let mut rig = reattached();
    rig.pane.tick(2_400);
    assert!(matches!(rig.pane.status(), LinkStatus::Attached(_)));
    rig.bench.output(1, HELLO);
    rig.pane.tick(2_500);
    assert_eq!(
        rig.pane.take_effects(&words()),
        [blocked(
            "Clipboard write from Codex blocked · select text to copy it"
        )],
        "the key at 1,000 ms was for the old link, not a gesture of the new one"
    );

    rig.pane.key(&key("b"), 3_000);
    rig.bench.output(1, HELLO);
    rig.pane.tick(3_100);
    assert_eq!(
        rig.pane.take_effects(&words()),
        [copied("Hello", "from Codex · 5 characters")],
        "a key on the new link opens the window again"
    );
}

#[test]
fn a_lease_change_forgets_the_key_before_it() {
    let mut rig = Rig::codex();
    rig.bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    rig.bench.push(0, LinkEvent::Lease(Lease::Owner));
    rig.pane.tick(1_500);
    assert_eq!(
        rig.write(2_000, HELLO),
        [blocked(WRITE_BLOCKED)],
        "the key at 1,000 ms was typed under a lease this client no longer holds"
    );

    rig.pane.key(&key("b"), 2_100);
    assert_eq!(
        rig.write(2_200, HELLO),
        [copied("Hello", "from Codex · 5 characters")],
        "a key under the new lease opens the window again"
    );
}

#[test]
fn a_pane_this_client_only_views_never_copies() {
    let mut rig = Rig::codex();
    rig.bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    rig.pane.tick(1_100);
    rig.pane.gesture(1_200);
    assert_eq!(
        rig.write(1_300, HELLO),
        [blocked(WRITE_BLOCKED)],
        "a click in a viewing pane is not a gesture to write with"
    );
}

#[test]
fn a_title_loses_every_bidirectional_control() {
    for c in [
        '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
        '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
    ] {
        assert_eq!(
            sanitize_title(&format!("a{c}b")),
            "ab",
            "U+{:04X} survived in the title",
            c as u32
        );
    }
}
