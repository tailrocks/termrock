//! The terminal pane over a link it does not own: it starts when there is room,
//! filters what reaches the program, and rebuilds its screen on every attach.
//! No `demo` feature is needed.

mod pane_support;

#[path = "terminal_pane/session_adapter.rs"]
mod session_adapter;

use std::sync::{Arc, Mutex};

use jackin_console::panes::{
    Lease, LinkEvent, LinkFault, LinkSource, LinkStatus, PaneBody, PaneCommand, PaneKey, PaneLink,
    PanePolicy, PaneRequest, PaneSize, PaneTiming, PasteOutcome, Sent, TerminalPane,
};
use jackin_console::sim::link::{SimLink, Timed};
use junie_tui::core::hit::Axis;
use pane_support::*;
use ratatui::layout::Rect;

fn numbered(n: u32) -> Vec<u8> {
    (1..=n)
        .flat_map(|i| format!("line {i:03}\r\n").into_bytes())
        .collect()
}

#[test]
fn pane_starts_at_the_first_valid_size() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    assert_eq!(pane.status(), &LinkStatus::Pending);
    assert_eq!(bench.opened(), 0, "a new pane holds no link");

    pane.sync_size(inner(80, 24));
    assert_eq!(bench.opened(), 1);
    assert_eq!(
        bench.sizes(0),
        [size(24, 80)],
        "the track column is not given to the program"
    );
    assert_eq!(pane.status(), &LinkStatus::Attaching);

    pane.sync_size(inner(80, 24));
    assert_eq!(bench.opened(), 1);
    assert_eq!(bench.sizes(0), [size(24, 80)], "the same size adds nothing");

    pane.sync_size(inner(90, 24));
    assert_eq!(bench.sizes(0), [size(24, 80), size(24, 90)]);
    assert_eq!(pane.grid().state().size(), (24, 90));
}

#[test]
fn a_pane_below_the_floor_stays_pending() {
    let bench = Bench::new();
    let mut narrow = pane(&bench);
    narrow.sync_size(Rect::new(0, 0, 20, 24));
    assert_eq!(narrow.status(), &LinkStatus::Pending, "19 columns");
    let mut short = pane(&bench);
    short.sync_size(Rect::new(0, 0, 81, 3));
    assert_eq!(short.status(), &LinkStatus::Pending, "3 rows");
    assert_eq!(bench.opened(), 0);

    let mut floor = pane(&bench);
    floor.sync_size(inner(20, 4));
    assert_eq!(bench.opened(), 1, "20 columns by 4 rows is enough");
    assert_eq!(bench.sizes(0), [size(4, 20)]);
}

#[test]
fn a_refused_open_is_a_failed_pane_and_enter_retries() {
    let bench = Bench::new();
    bench.refuse_next("the gateway is not reachable");
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    assert_eq!(
        pane.status(),
        &LinkStatus::Failed(LinkFault::new("the gateway is not reachable"))
    );
    assert_eq!(bench.opened(), 0);

    assert_eq!(pane.key(&key("a"), 10), PaneKey::Consumed);
    assert_eq!(bench.opened(), 0, "only Enter retries");

    assert_eq!(pane.key(&key("enter"), 20), PaneKey::Consumed);
    assert_eq!(pane.status(), &LinkStatus::Attaching);
    assert_eq!(bench.sizes(0), [size(24, 80)]);
}

#[test]
fn keys_and_pastes_pass_the_forward_policy() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    assert_eq!(pane.key(&key("ctrl+d"), 1_000), PaneKey::Consumed);
    assert_eq!(pane.key(&key("ctrl+q"), 1_000), PaneKey::Consumed);
    assert_eq!(bench.commands(0), [], "0x04 and 0x11 reach no pane");

    assert_eq!(pane.key(&key("ctrl+p"), 1_000), PaneKey::Forwarded);
    assert_eq!(pane.key(&key("a"), 1_000), PaneKey::Forwarded);
    assert_eq!(bench.commands(0), [input(&[0x10]), input(b"a")]);

    assert_eq!(pane.paste("x\u{1}y\u{4}z", 1_000), PasteOutcome::Sent);
    assert_eq!(bench.commands(0)[2], input(b"xyz"));

    bench.output(0, b"\x1b[?2004h");
    pane.tick(1_100);
    assert_eq!(pane.paste("one\ntwo", 1_200), PasteOutcome::Sent);
    assert_eq!(
        bench.commands(0)[3],
        input(b"\x1b[200~one\ntwo\x1b[201~"),
        "wrapped only when the grid has the mode"
    );

    let big = "x".repeat(64 * 1024 + 1);
    assert_eq!(pane.paste(&big, 1_300), PasteOutcome::TooLarge);
    assert_eq!(
        bench.commands(0).len(),
        4,
        "a paste over the limit sends nothing"
    );
}

#[test]
fn a_paste_without_protection_asks_and_a_confirmed_one_runs() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    assert_eq!(
        pane.paste("a\nb\nc", 1_000),
        PasteOutcome::NeedsConfirm { lines: 3 }
    );
    assert_eq!(bench.commands(0), [], "nothing is sent before the answer");
    assert_eq!(pane.paste_confirmed("a\nb\nc", 1_100), PasteOutcome::Sent);
    assert_eq!(bench.commands(0), [input(b"a\nb\nc")]);

    let mut idle = pane_support::pane(&Bench::new());
    assert_eq!(
        idle.paste("hello", 0),
        PasteOutcome::Dropped,
        "a pane without a link takes nothing"
    );
}

#[test]
fn reattach_builds_a_fresh_grid() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[3");
    bench.push(
        0,
        LinkEvent::Ended(jackin_console::panes::EndKind::Detached),
    );
    pane.tick(900);
    assert_eq!(
        pane.status(),
        &LinkStatus::Detached(jackin_console::panes::DetachCause::Operator)
    );

    assert_eq!(pane.key(&key("enter"), 1_000), PaneKey::Consumed);
    assert_eq!(bench.opened(), 2);
    assert!(bench.dropped(0), "the old link is dropped, not reused");
    bench.push(1, LinkEvent::Attached);
    bench.push(1, LinkEvent::Lease(Lease::Owner));
    bench.output(1, b"hello");
    pane.tick(1_000);
    pane.tick(1_800);
    assert_eq!(
        row(&pane, 0),
        "hello",
        "a sequence cut off in the old grid does not eat the new text"
    );

    bench.output(0, b"late");
    pane.tick(1_900);
    assert_eq!(
        row(&pane, 0),
        "hello",
        "a late event of the old link is not shown"
    );
}

#[test]
fn reattach_resets_the_view_and_takes_the_size_the_pane_has_now() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, &numbered(100));
    pane.tick(1_000);
    pane.viewport_mut().wheel(Axis::Vertical, -3);
    assert!(!pane.viewport().follow());
    bench.push(
        0,
        LinkEvent::Ended(jackin_console::panes::EndKind::Detached),
    );
    pane.tick(1_100);
    pane.sync_size(inner(99, 30));
    assert_eq!(
        bench.sizes(0),
        [size(24, 80)],
        "a detached link gets no size"
    );

    pane.key(&key("enter"), 1_200);
    assert!(pane.viewport().follow(), "the offset is 0 again");
    assert!(!pane.viewport().has_selection());
    assert_eq!(bench.sizes(1), [size(30, 99)]);
    assert_eq!(pane.grid().state().size(), (30, 99));
    assert_eq!(pane.grid().state().scrollback_len(), 0);
}

#[test]
fn scrollback_cleared_resets_the_viewport() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, &numbered(100));
    pane.tick(1_000);
    pane.viewport_mut().wheel(Axis::Vertical, -3);
    assert!(!pane.viewport().follow());
    bench.output(0, b"\x1b[3J");
    pane.tick(1_100);
    assert!(pane.viewport().follow());
    assert!(!pane.viewport().has_selection());
    assert_eq!(pane.grid().state().scrollback_len(), 0);
}

#[test]
fn redraw_asks_the_program_and_clear_only_clears_locally() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, &numbered(100));
    pane.tick(1_000);
    let before = row(&pane, 23);

    pane.redraw();
    assert_eq!(bench.commands(0), [PaneCommand::Redraw]);

    pane.viewport_mut().wheel(Axis::Vertical, -3);
    pane.clear();
    assert_eq!(
        bench.commands(0),
        [PaneCommand::Redraw],
        "clear sends no byte"
    );
    assert_eq!(pane.grid().state().scrollback_len(), 0);
    assert!(pane.viewport().follow());
    assert_eq!(row(&pane, 23), before, "the screen itself is kept");
}

#[test]
fn replies_go_back_to_the_program_and_a_full_queue_drops_them() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"\x1b[3;5H\x1b[6n");
    pane.tick(1_000);
    assert_eq!(
        bench.commands(0),
        [PaneCommand::Reply(b"\x1b[3;5R".to_vec())]
    );
    assert_eq!(pane.dropped_replies(), 0);

    bench.fill(0, true);
    bench.output(0, b"\x1b[6n");
    pane.tick(1_100);
    assert_eq!(
        pane.dropped_replies(),
        1,
        "counted, and the pane did not block"
    );
    assert_eq!(bench.commands(0).len(), 1);

    let tight = PanePolicy {
        paste_max_bytes: 4,
        ..PanePolicy::default()
    };
    let mut small = pane_for(codex(), &bench, tight);
    small.sync_size(inner(80, 24));
    assert_eq!(small.paste("12345", 0), PasteOutcome::TooLarge);
}

#[test]
fn a_reattach_below_the_floor_waits_for_room() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.push(
        0,
        LinkEvent::Ended(jackin_console::panes::EndKind::Detached),
    );
    pane.tick(1_100);
    pane.sync_size(inner(10, 2));

    pane.key(&key("enter"), 1_200);
    assert_eq!(bench.opened(), 1, "no link opens at the old 80x24");
    assert_eq!(pane.status(), &LinkStatus::Pending);
    assert_eq!(
        pane.note(&words()).map(|n| n.text).as_deref(),
        Some("Pane too small · zoom with Ctrl+B z")
    );

    pane.sync_size(inner(60, 20));
    assert_eq!(bench.opened(), 2, "the next size that fits opens it");
    assert_eq!(bench.sizes(1), [size(20, 60)]);
    assert_eq!(pane.status(), &LinkStatus::Attaching);
}

#[test]
fn a_paste_the_link_did_not_take_is_not_reported_sent() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.fill(0, true);
    assert_eq!(pane.paste("hello", 1_000), PasteOutcome::Full);
    assert_eq!(bench.commands(0), []);
    assert_eq!(
        pane.paste_confirmed("a\nb", 1_100),
        PasteOutcome::Full,
        "a confirmed paste says so too"
    );

    bench.fill(0, false);
    assert_eq!(pane.paste("hello", 1_200), PasteOutcome::Sent);
    assert_eq!(bench.commands(0), [input(b"hello")]);
}

#[test]
fn a_pane_feeds_its_grid_no_more_output_per_tick_than_its_policy_allows() {
    let bench = Bench::new();
    let mut pane = pane_for(
        codex(),
        &bench,
        PanePolicy {
            feed_budget_bytes: 100,
            ..PanePolicy::default()
        },
    );
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    pane.tick(800);

    for part in ["a", "b", "c"] {
        bench.output(0, part.repeat(100).as_bytes());
    }
    pane.tick(900);
    assert_eq!(row(&pane, 0), "a".repeat(80), "one chunk fits the budget");
    assert_eq!(row(&pane, 1), "aaaaaaaaaaaaaaaaaaaa");
    assert_eq!(
        row(&pane, 2),
        "",
        "the second chunk waits for the next tick"
    );

    pane.tick(1_000);
    assert!(row(&pane, 2).starts_with('b'), "{:?}", row(&pane, 2));
    pane.tick(1_100);
    assert!(row(&pane, 3).contains('c'));
}

#[test]
fn a_link_that_has_not_sent_its_lease_is_not_taken_for_the_owner() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    pane.tick(0);
    pane.tick(800);
    assert_eq!(pane.status(), &LinkStatus::Attaching, "no lease yet");

    bench.output(0, b"\x1b[5n");
    pane.tick(900);
    assert_eq!(
        bench.commands(0),
        [],
        "no reply before the lease says owner"
    );

    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(1_000);
    assert_eq!(pane.status(), &LinkStatus::Attached(Lease::Owner));
    bench.output(0, b"\x1b[5n");
    pane.tick(1_100);
    assert_eq!(bench.commands(0), [PaneCommand::Reply(b"\x1b[0n".to_vec())]);
}

/// Records the output bytes each drain hands the pane, in order. The pane feeds
/// every byte of one drain within the same tick, so each entry is what one tick
/// feeds into the grid.
#[derive(Debug)]
struct Metered {
    inner: SimLink,
    fed: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl PaneLink for Metered {
    fn send(&self, command: PaneCommand) -> Sent {
        self.inner.send(command)
    }

    fn set_size(&self, size: PaneSize) {
        self.inner.set_size(size);
    }

    fn drain(&self, now_ms: i64, output_budget: usize) -> Vec<LinkEvent> {
        let events = self.inner.drain(now_ms, output_budget);
        let bytes = events
            .iter()
            .filter_map(|event| match event {
                LinkEvent::Output(chunk) => Some(chunk.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .collect();
        self.fed.lock().expect("fed").push(bytes);
        events
    }
}

/// Hands one prepared link to the pane, which opens it once.
#[derive(Debug)]
struct Once(Mutex<Option<Box<dyn PaneLink>>>);

impl LinkSource for Once {
    fn open(&self, _: &PaneRequest, _: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
        Ok(self.0.lock().expect("link").take().expect("one open"))
    }

    fn restart(&self, _: &PaneRequest, _: PaneSize) -> Result<Box<dyn PaneLink>, LinkFault> {
        Err(LinkFault::new("this script does not restart"))
    }
}

/// A pane over a scripted attach that shows `stream`, and the per-drain byte
/// counts that the pane's ticks are fed.
fn scripted(stream: Vec<u8>, policy: PanePolicy) -> (TerminalPane, Arc<Mutex<Vec<Vec<u8>>>>) {
    let fed = Arc::new(Mutex::new(Vec::new()));
    let link = Metered {
        inner: SimLink::new(vec![
            Timed {
                after_ms: 0,
                event: LinkEvent::Attached,
            },
            Timed {
                after_ms: 0,
                event: LinkEvent::Lease(Lease::Owner),
            },
            Timed {
                after_ms: 0,
                event: LinkEvent::Output(stream),
            },
        ]),
        fed: Arc::clone(&fed),
    };
    let boxed: Box<dyn PaneLink> = Box::new(link);
    let mut pane = TerminalPane::new(
        1,
        shell(),
        Arc::new(Once(Mutex::new(Some(boxed)))),
        PaneTiming::default(),
        0,
    );
    pane.set_policy(&Arc::new(policy));
    pane.sync_size(inner(80, 24));
    (pane, fed)
}

/// What the user sees: the text of each row, the history above the screen and
/// the title.
fn screen(pane: &TerminalPane) -> (Vec<String>, usize, Option<String>) {
    let (rows, _) = pane.grid().state().size();
    (
        (0..rows).map(|r| row(pane, r)).collect(),
        pane.grid().state().scrollback_len(),
        pane.title().map(str::to_owned),
    )
}

fn script(len: usize) -> Vec<u8> {
    (1..)
        .flat_map(|i| format!("line {i:05}\r\n").into_bytes())
        .take(len)
        .collect()
}

#[test]
fn terminal_pane_script_feed_budget_limits_each_tick() {
    const BUDGET: usize = 257;
    const STREAM: usize = 16 * 1024;
    let stream = script(STREAM);

    let (mut pane, fed) = scripted(
        stream.clone(),
        PanePolicy {
            feed_budget_bytes: BUDGET,
            ..PanePolicy::default()
        },
    );
    for now in 0..100 {
        pane.tick(now);
    }
    let ticks = fed.lock().expect("fed").clone();
    let sizes: Vec<usize> = ticks.iter().map(Vec::len).collect();
    assert!(
        sizes.iter().all(|&bytes| bytes <= BUDGET),
        "a tick feeds at most the budget: {sizes:?}"
    );
    assert_eq!(sizes.iter().copied().max(), Some(BUDGET));
    assert_eq!(
        ticks.concat(),
        stream,
        "every byte arrives within 100 ticks, in order"
    );

    let (mut reference, _) = scripted(stream, PanePolicy::default());
    reference.tick(0);
    assert_eq!(
        screen(&pane),
        screen(&reference),
        "the bounded feed shows the same screen as one feed"
    );
}
