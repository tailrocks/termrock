//! The seven link states of a terminal pane, one look and one way out each.
//! The scenario `capsule-terminal` shows them as baselines; here the pane
//! itself is held to the texts and the recoveries.

mod pane_support;

use jackin_console::domain::instance::AgentState;
use jackin_console::panes::{
    DetachCause, EndKind, FailNext, Lease, LinkEvent, LinkFault, LinkStatus, PaneBody, PaneEffect,
    PaneFace, PaneKey, PaneNote,
};
use pane_support::*;
use ratatui::layout::Rect;
use termrock::theme::Tone;

fn note(pane: &impl PaneBody) -> Option<String> {
    pane.note(&words()).map(|n| n.text)
}

fn effects(pane: &mut impl PaneBody) -> Vec<PaneEffect> {
    pane.take_effects(&words())
}

#[test]
fn pending_note_and_zoom() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    assert_eq!(
        note(&pane),
        None,
        "before the first size there is nothing to say"
    );
    assert_eq!(
        pane.face(&words()),
        PaneFace::Empty {
            title: "Waiting for room".into(),
            hint: Some("Codex starts when the pane is big enough".into()),
        }
    );

    pane.sync_size(Rect::new(0, 0, 12, 10));
    assert_eq!(
        pane.note(&words()),
        Some(PaneNote {
            text: "Pane too small · zoom with Ctrl+B z".into(),
            tone: Tone::Warning
        })
    );
    assert_eq!(bench.opened(), 0);

    pane.sync_size(inner(80, 24));
    assert_eq!(bench.opened(), 1, "zoom gives it room and it starts");
    assert_eq!(note(&pane), None);
}

#[test]
fn attaching_placeholder() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    assert_eq!(
        pane.face(&words()),
        PaneFace::Empty {
            title: "Attaching to Codex…".into(),
            hint: Some("Ctrl+B x cancels".into()),
        }
    );
    assert_eq!(note(&pane), None);
    assert!(
        !pane.close_needs_confirm(),
        "closing an attach in progress asks nothing"
    );
    assert_eq!(pane.state(), AgentState::Unknown);
}

#[test]
fn viewing_note_and_lease_handoff() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    bench.push(
        0,
        LinkEvent::Notice("another client\u{1b}[31m is attached\u{7}".into()),
    );
    bench.output(0, b"screen");
    pane.tick(0);
    pane.tick(800);

    assert_eq!(pane.status(), &LinkStatus::Attached(Lease::ReadOnly));
    assert_eq!(pane.suffix(&words()).as_deref(), Some("(viewing)"));
    assert_eq!(
        pane.note(&words()),
        Some(PaneNote {
            text: "Read-only · another client controls this pane".into(),
            tone: Tone::Muted
        })
    );
    assert_eq!(
        effects(&mut pane),
        [PaneEffect::Status {
            text: "another client[31m is attached".into(),
            tone: Tone::Secondary
        }],
        "the server notice, without its control characters"
    );
    assert_eq!(pane.face(&words()), PaneFace::Grid);

    assert_eq!(pane.key(&key("a"), 900), PaneKey::Forwarded);
    assert_eq!(
        bench.commands(0),
        [input(b"a")],
        "typing is how the lease is taken"
    );

    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(1_000);
    assert_eq!(pane.status(), &LinkStatus::Attached(Lease::Owner));
    assert_eq!(pane.suffix(&words()), None);
    assert_eq!(note(&pane), None);
}

#[test]
fn detached_note_and_enter() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"last screen");
    bench.push(0, LinkEvent::Ended(EndKind::Detached));
    pane.tick(1_000);
    assert_eq!(pane.status(), &LinkStatus::Detached(DetachCause::Operator));
    assert_eq!(
        pane.note(&words()),
        Some(PaneNote {
            text: "Detached from Codex · press Enter to reattach".into(),
            tone: Tone::Warning
        })
    );
    assert_eq!(pane.face(&words()), PaneFace::Dimmed);
    assert_eq!(row(&pane, 0), "last screen", "the last grid is kept");
    assert_eq!(
        effects(&mut pane),
        [PaneEffect::Status {
            text: "Detached from Codex · press Enter to reattach".into(),
            tone: Tone::Warning
        }]
    );
    assert!(!pane.close_needs_confirm());

    assert_eq!(pane.key(&key("enter"), 1_100), PaneKey::Consumed);
    assert_eq!(bench.opened(), 2);
    assert_eq!(bench.commands(1), [], "Enter is not forwarded");

    let lost = Bench::new();
    let mut gone = live(&lost);
    lost.push(0, LinkEvent::Ended(EndKind::StreamClosed));
    gone.tick(1_000);
    assert_eq!(
        gone.status(),
        &LinkStatus::Detached(DetachCause::GatewayLost)
    );
    assert_eq!(
        note(&gone).as_deref(),
        Some("Gateway connection lost · press Enter to reattach")
    );
}

#[test]
fn a_stream_that_closes_before_the_attach_is_a_failure() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Ended(EndKind::StreamClosed));
    pane.tick(100);
    assert_eq!(pane.status(), &LinkStatus::Failed(LinkFault::new("")));
    assert_eq!(
        note(&pane).as_deref(),
        Some(
            "Press Enter to retry · could not attach to Codex: the connection closed before the pane attached"
        )
    );
}

#[test]
fn exited_note_and_close() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"bye");
    bench.push(0, LinkEvent::Exited(Some(0)));
    pane.tick(1_000);
    assert_eq!(pane.status(), &LinkStatus::Exited(Some(0)));
    assert_eq!(
        note(&pane).as_deref(),
        Some("Codex exited (code 0) · press Enter to restart · the sandbox keeps running")
    );
    assert_eq!(pane.state(), AgentState::Done);
    assert!(!pane.close_needs_confirm(), "Ctrl+B x closes it at once");
    assert_eq!(row(&pane, 0), "bye", "the last output stays");
    assert_eq!(pane.key(&key("a"), 1_100), PaneKey::Consumed);
    assert_eq!(bench.opened(), 1, "a letter does not restart it");

    let failed = Bench::new();
    let mut crashed = live(&failed);
    failed.push(0, LinkEvent::Exited(Some(3)));
    crashed.tick(1_000);
    assert_eq!(
        note(&crashed).as_deref(),
        Some("Codex exited (code 3) · press Enter to restart · the sandbox keeps running")
    );
    assert_eq!(crashed.state(), AgentState::Blocked);

    let unknown = Bench::new();
    let mut lost = live(&unknown);
    unknown.push(0, LinkEvent::Exited(None));
    lost.tick(1_000);
    assert_eq!(
        note(&lost).as_deref(),
        Some("Codex exited (code unknown) · press Enter to restart · the sandbox keeps running")
    );
}

#[test]
fn enter_restarts_an_exited_program() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    bench.output(0, b"bye");
    bench.push(0, LinkEvent::Exited(Some(0)));
    pane.tick(1_000);
    assert_eq!(pane.key(&key("enter"), 1_100), PaneKey::Consumed);
    assert_eq!(bench.opened(), 2, "Enter starts the program again");
    assert_eq!(bench.restarted(), [1], "the second link is a restart");
    assert_eq!(pane.status(), &LinkStatus::Attaching);
    assert_eq!(bench.commands(1), [], "Enter is not forwarded");
    assert_eq!(row(&pane, 0), "", "the new attach has a fresh screen");
}

#[test]
fn failed_note_and_retry() {
    let bench = Bench::new();
    bench.refuse_with(LinkFault::new("the gateway is unreachable").then(FailNext::RunDoctor));
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    assert_eq!(
        pane.note(&words()),
        Some(PaneNote {
            text: "Press Enter to retry · could not attach to Codex: the gateway is unreachable · run jackin doctor".into(),
            tone: Tone::Error
        })
    );
    assert_eq!(
        pane.face(&words()),
        PaneFace::Empty {
            title: "Codex is not attached".into(),
            hint: None
        },
        "there is no grid to keep"
    );
    assert_eq!(pane.state(), AgentState::Blocked);
    assert_eq!(effects(&mut pane).len(), 1);
    assert!(!pane.close_needs_confirm());

    assert_eq!(pane.key(&key("enter"), 100), PaneKey::Consumed);
    assert_eq!(pane.status(), &LinkStatus::Attaching, "Enter retries");

    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    bench.output(0, b"half");
    bench.push(
        0,
        LinkEvent::Ended(EndKind::Refused(LinkFault::new("not the owner"))),
    );
    pane.tick(200);
    assert_eq!(
        pane.status(),
        &LinkStatus::Failed(LinkFault::new("not the owner"))
    );
    assert_eq!(pane.face(&words()), PaneFace::Grid, "the last grid is kept");
}

#[test]
fn enter_forwarding_rules() {
    let bench = Bench::new();
    let mut owner = live(&bench);
    assert_eq!(owner.key(&key("enter"), 1_000), PaneKey::Forwarded);
    assert_eq!(bench.commands(0), [input(b"\r")], "state 3 forwards Enter");

    let down = Bench::new();
    let mut detached = live(&down);
    down.push(0, LinkEvent::Ended(EndKind::Detached));
    detached.tick(1_000);
    detached.key(&key("enter"), 1_100);
    assert_eq!(down.commands(0), [], "state 5 consumes it");
    assert_eq!(down.commands(1), []);

    let bad = Bench::new();
    bad.refuse_next("no");
    let mut failed = pane(&bad);
    failed.sync_size(inner(80, 24));
    failed.key(&key("enter"), 100);
    assert_eq!(bad.opened(), 1, "state 7 consumes it and tries again");
    assert_eq!(bad.commands(0), []);
    assert_eq!(
        bad.restarted(),
        Vec::<usize>::new(),
        "a retry is not a restart"
    );
}

#[test]
fn only_a_pane_with_a_program_takes_the_cursor() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    assert!(!pane.accepts_input(), "pending");
    pane.sync_size(inner(80, 24));
    assert!(pane.accepts_input(), "attaching: the link is up");
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    pane.tick(800);
    assert!(pane.accepts_input());
    bench.push(0, LinkEvent::Ended(EndKind::Detached));
    pane.tick(900);
    assert!(!pane.accepts_input(), "detached");

    let gone = Bench::new();
    let mut exited = live(&gone);
    gone.push(0, LinkEvent::Exited(Some(0)));
    exited.tick(1_000);
    assert!(!exited.accepts_input(), "exited");
}

#[test]
fn a_live_pane_asks_before_it_closes() {
    let bench = Bench::new();
    let pane = live(&bench);
    assert!(pane.close_needs_confirm());
    assert_eq!(
        pane.kind(),
        jackin_console::panes::PaneKindTag::Agent,
        "the dialog picks its text by the kind"
    );
    let shell_bench = Bench::new();
    let shell = pane_for(shell(), &shell_bench, Default::default());
    assert_eq!(shell.kind(), jackin_console::panes::PaneKindTag::Shell);
}

#[test]
fn a_server_notice_loses_its_bidirectional_controls() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    bench.push(
        0,
        LinkEvent::Notice("txt\u{202e}gpj.exe \u{2066}\u{200f}\u{61c}ok".into()),
    );
    pane.tick(0);
    pane.tick(800);

    assert_eq!(
        effects(&mut pane),
        [PaneEffect::Status {
            text: "txtgpj.exe ok".into(),
            tone: Tone::Secondary
        }]
    );
}
