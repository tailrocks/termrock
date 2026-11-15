//! The pane over the library's session controller: every word, state and
//! effect the console reads still comes out of `TerminalPane`, and its cached
//! `LinkStatus` never lags the controller behind it.

use std::sync::Arc;

use jackin_console::domain::instance::AgentState;
use jackin_console::domain::shown::shown;
use jackin_console::domain::vocabulary::fill_in;
use jackin_console::panes::{
    ClipboardWrite, DetachCause, EndKind, FailNext, Lease, LinkEvent, LinkFault, LinkStatus,
    PaneBody, PaneCommand, PaneEffect, PaneFace, PanePolicy, PaneTiming, TerminalPane,
    sanitize_title,
};
use junie_tui::core::hit::Axis;
use junie_tui::theme::Tone;
use ratatui::layout::Rect;

use super::pane_support::*;

fn status_effect(text: impl Into<String>, tone: Tone) -> PaneEffect {
    PaneEffect::Status {
        text: text.into(),
        tone,
    }
}

fn read_only(bench: &Arc<Bench>) -> TerminalPane {
    let mut pane = pane(bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    pane.tick(0);
    pane.tick(800);
    pane
}

fn ended(bench: &Arc<Bench>, kind: EndKind) -> TerminalPane {
    let mut pane = live(bench);
    bench.push(0, LinkEvent::Ended(kind));
    pane.tick(900);
    pane
}

#[test]
fn jackin_adapter_preserves_all_pane_words_and_effects() {
    let words = words();
    let doctor = LinkFault::new("sandbox missing").then(FailNext::RunDoctor);

    let bench = Bench::new();
    let mut pending = pane(&bench);
    assert!(matches!(
        pending.face(&words),
        PaneFace::Empty { ref title, .. } if *title == words.waiting_title
    ));
    assert_eq!(pending.state(), AgentState::Unknown);
    assert_eq!(pending.note(&words), None);
    pending.sync_size(Rect::new(0, 0, 10, 2));
    assert_eq!(
        pending.note(&words).map(|n| (n.text, n.tone)),
        Some((words.too_small.clone(), Tone::Warning)),
        "a pane below the floor says it needs room"
    );

    let bench = Bench::new();
    let viewer = read_only(&bench);
    assert_eq!(viewer.suffix(&words), Some(words.viewing_suffix.clone()));
    assert_eq!(
        viewer.note(&words).map(|n| (n.text, n.tone)),
        Some((words.viewing.clone(), Tone::Muted))
    );
    assert_eq!(viewer.face(&words), PaneFace::Grid);

    let bench = Bench::new();
    let mut shell_pane = pane_for(shell(), &bench, PanePolicy::default());
    shell_pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    shell_pane.tick(0);
    shell_pane.tick(800);
    bench.output(0, b"\x1b]52;c;SGVsbG8=\x07\x07");
    shell_pane.tick(900);
    assert_eq!(
        shell_pane.state(),
        AgentState::Blocked,
        "a bell is attention"
    );
    assert_eq!(
        shell_pane.take_effects(&words),
        [status_effect(
            fill_in(&words.clip_write_blocked, &[("source", &words.shell_label)]),
            Tone::Warning
        )],
        "a shell never writes the clipboard, in the shell's own words"
    );

    let bench = Bench::new();
    let mut exited = live(&bench);
    bench.push(0, LinkEvent::Exited(Some(0)));
    exited.tick(900);
    assert_eq!(exited.state(), AgentState::Done);
    assert_eq!(
        exited.note(&words).map(|n| (n.text, n.tone)),
        Some((
            fill_in(&words.exited, &[("agent", "Codex"), ("code", "0")]),
            Tone::Muted
        ))
    );
    assert_eq!(exited.face(&words), PaneFace::Grid);
    assert_eq!(
        exited.take_effects(&words),
        [],
        "an exit says nothing twice"
    );

    let bench = Bench::new();
    let mut crashed = live(&bench);
    bench.push(0, LinkEvent::Exited(None));
    crashed.tick(900);
    assert_eq!(crashed.state(), AgentState::Unknown);
    assert_eq!(
        crashed.note(&words).map(|n| (n.text, n.tone)),
        Some((
            fill_in(
                &words.exited,
                &[("agent", "Codex"), ("code", &words.exited_unknown_code)]
            ),
            Tone::Warning
        ))
    );

    let bench = Bench::new();
    let mut failed = pane(&bench);
    bench.refuse_with(doctor.clone());
    failed.sync_size(inner(80, 24));
    assert_eq!(failed.state(), AgentState::Blocked);
    let expected = words.failure("Codex", &doctor);
    assert_ne!(
        expected,
        words.failure("Codex", &LinkFault::new("sandbox missing")),
        "the next step is part of the words"
    );
    assert_eq!(
        failed.note(&words).map(|n| (n.text, n.tone)),
        Some((expected.clone(), Tone::Error))
    );
    assert_eq!(
        failed.take_effects(&words),
        [status_effect(expected, Tone::Error)]
    );
    assert!(matches!(
        failed.face(&words),
        PaneFace::Empty { ref title, hint: None }
            if *title == fill_in(&words.unattached_title, &[("agent", "Codex")])
    ));

    let bench = Bench::new();
    let mut operator = ended(&bench, EndKind::Detached);
    assert_eq!(operator.face(&words), PaneFace::Dimmed);
    let detached = fill_in(&words.detached, &[("agent", "Codex")]);
    assert_eq!(
        operator.note(&words).map(|n| (n.text, n.tone)),
        Some((detached.clone(), Tone::Warning))
    );
    assert_eq!(
        operator.take_effects(&words),
        [status_effect(detached, Tone::Warning)]
    );

    let bench = Bench::new();
    let mut lost = ended(&bench, EndKind::StreamClosed);
    assert_eq!(
        lost.take_effects(&words),
        [status_effect(words.connection_lost.clone(), Tone::Warning)]
    );
    assert_eq!(
        lost.note(&words).map(|n| n.text),
        Some(words.connection_lost.clone())
    );
}

#[test]
fn jackin_adapter_keeps_the_host_text_hygiene() {
    let words = words();
    let bench = Bench::new();
    let mut pane = live(&bench);

    let long_notice = format!("{}\u{1b}[31m", "n".repeat(500));
    bench.push(0, LinkEvent::Notice(long_notice.clone()));
    bench.push(0, LinkEvent::Notice("\u{1}\u{2}".to_owned()));
    bench.output(0, b"\x1b]0;a\x01b\x07");
    pane.tick(900);
    let effects = pane.take_effects(&words);
    assert_eq!(
        effects,
        [status_effect(shown(&long_notice, 240), Tone::Secondary)],
        "the control-only notice shows nothing and is discarded"
    );
    let PaneEffect::Status { text, .. } = &effects[0] else {
        panic!("a notice is a status");
    };
    assert!(text.chars().count() <= 240, "the notice is cut to 240");
    assert_eq!(pane.title(), Some(sanitize_title("a\u{1}b").as_str()));
}

#[test]
fn jackin_adapter_forwards_what_the_transport_sees() {
    let bench = Bench::new();
    let mut pane = live(&bench);
    pane.focus_changed(true, 900);
    pane.key(&key("a"), 1_000);
    pane.redraw();
    assert_eq!(bench.commands(0), [input(b"a"), PaneCommand::Redraw]);
    assert_eq!(bench.sizes(0), [size(24, 80)]);
    pane.sync_size(inner(100, 30));
    assert_eq!(bench.sizes(0), [size(24, 80), size(30, 100)]);

    bench.push(
        0,
        LinkEvent::Ended(EndKind::Failed(
            LinkFault::new("busy").then(FailNext::RunDoctor),
        )),
    );
    pane.tick(2_000);
    assert_eq!(
        pane.status(),
        &LinkStatus::Detached(DetachCause::GatewayLost),
        "a failure after the attach is a lost gateway"
    );

    let bench = Bench::new();
    let mut fresh = pane_with_refusal(
        &bench,
        EndKind::Refused(LinkFault::new("busy").then(FailNext::RunDoctor)),
    );
    fresh.tick(10);
    assert_eq!(
        fresh.status(),
        &LinkStatus::Failed(LinkFault::new("busy").then(FailNext::RunDoctor)),
        "the host fault, next step included, survives the round trip"
    );
}

fn pane_with_refusal(bench: &Arc<Bench>, kind: EndKind) -> TerminalPane {
    let mut pane = pane(bench);
    pane.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Ended(kind));
    pane
}

fn reach(status: &LinkStatus) -> (Arc<Bench>, TerminalPane) {
    let bench = Bench::new();
    let pane = match status {
        LinkStatus::Pending => pane(&bench),
        LinkStatus::Attaching => {
            let mut pane = pane(&bench);
            pane.sync_size(inner(80, 24));
            pane
        }
        LinkStatus::Attached(Lease::Owner) => live(&bench),
        LinkStatus::Attached(Lease::ReadOnly) => read_only(&bench),
        LinkStatus::Detached(DetachCause::Operator) => ended(&bench, EndKind::Detached),
        LinkStatus::Detached(DetachCause::GatewayLost) => ended(&bench, EndKind::StreamClosed),
        LinkStatus::Exited(code) => {
            let mut pane = live(&bench);
            bench.push(0, LinkEvent::Exited(*code));
            pane.tick(900);
            pane
        }
        LinkStatus::Failed(fault) if fault.reason.is_empty() => {
            let mut pane = pane_with_refusal(&bench, EndKind::StreamClosed);
            pane.tick(0);
            pane
        }
        LinkStatus::Failed(fault) => {
            let mut pane = pane(&bench);
            bench.refuse_with(fault.clone());
            pane.sync_size(inner(80, 24));
            pane
        }
    };
    assert_eq!(pane.status(), status, "the scenario reaches {status:?}");
    (bench, pane)
}

type Mutator = fn(&mut TerminalPane);

fn every_status() -> Vec<LinkStatus> {
    vec![
        LinkStatus::Pending,
        LinkStatus::Attaching,
        LinkStatus::Attached(Lease::Owner),
        LinkStatus::Attached(Lease::ReadOnly),
        LinkStatus::Detached(DetachCause::Operator),
        LinkStatus::Detached(DetachCause::GatewayLost),
        LinkStatus::Exited(Some(0)),
        LinkStatus::Exited(Some(2)),
        LinkStatus::Exited(None),
        LinkStatus::Failed(LinkFault::new("")),
        LinkStatus::Failed(LinkFault::new("no sandbox").then(FailNext::RunDoctor)),
    ]
}

#[test]
fn jackin_session_status_cache_matches_controller() {
    let mutators: [(&str, Mutator); 13] = [
        ("focus in", |p| p.focus_changed(true, 5_000)),
        ("focus out", |p| p.focus_changed(false, 5_000)),
        ("gesture", |p| p.gesture(5_000)),
        ("wheel", |p| {
            p.wheel(Axis::Vertical, 1);
        }),
        ("paste", |p| {
            p.paste("x", 5_000);
        }),
        ("paste confirmed", |p| {
            p.paste_confirmed("a\nb", 5_000);
        }),
        ("redraw", PaneBody::redraw),
        ("clear", PaneBody::clear),
        ("tick, nothing queued", |p| {
            p.tick(9_000);
        }),
        ("take effects", |p| {
            let _ = p.take_effects(&super::pane_support::words());
        }),
        ("set policy", |p| {
            p.set_policy(&Arc::new(PanePolicy {
                clipboard_write: ClipboardWrite::Deny,
                ..PanePolicy::default()
            }));
        }),
        ("viewport access", |p| {
            let _ = p.viewport_mut();
        }),
        ("key", |p| {
            p.key(&key("a"), 5_000);
        }),
    ];
    for status in every_status() {
        for (name, mutate) in mutators {
            let (_bench, mut pane) = reach(&status);
            mutate(&mut pane);
            assert_eq!(
                pane.status(),
                &status,
                "{name} on {status:?} leaves the status as it was"
            );
        }
    }
}

#[test]
fn jackin_session_status_cache_follows_every_lifecycle_mutator() {
    let (bench, mut pane) = reach(&LinkStatus::Pending);
    pane.sync_size(inner(10, 2));
    assert_eq!(pane.status(), &LinkStatus::Pending, "below the floor");
    pane.sync_size(inner(80, 24));
    assert_eq!(pane.status(), &LinkStatus::Attaching, "sync_size opens");

    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    pane.tick(0);
    assert_eq!(pane.status(), &LinkStatus::Attaching, "the replay is open");
    pane.tick(600);
    assert_eq!(
        pane.status(),
        &LinkStatus::Attached(Lease::Owner),
        "tick closes it"
    );
    bench.push(0, LinkEvent::Lease(Lease::ReadOnly));
    pane.tick(700);
    assert_eq!(
        pane.status(),
        &LinkStatus::Attached(Lease::ReadOnly),
        "a lease change"
    );

    bench.push(0, LinkEvent::Ended(EndKind::Detached));
    pane.tick(800);
    assert_eq!(pane.status(), &LinkStatus::Detached(DetachCause::Operator));
    pane.key(&key("enter"), 900);
    assert_eq!(pane.status(), &LinkStatus::Attaching, "Enter retries");
    bench.push(1, LinkEvent::Exited(Some(1)));
    pane.tick(1_000);
    assert_eq!(pane.status(), &LinkStatus::Exited(Some(1)));
    pane.sync_size(inner(5, 2));
    pane.key(&key("enter"), 1_100);
    assert_eq!(
        pane.status(),
        &LinkStatus::Pending,
        "a restart waits for room"
    );
    pane.sync_size(inner(80, 24));
    assert_eq!(pane.status(), &LinkStatus::Attaching);
    assert_eq!(bench.restarted(), [2], "and then restarts");

    bench.refuse_with(LinkFault::new("again"));
    bench.push(
        2,
        LinkEvent::Ended(EndKind::Failed(LinkFault::new("closed"))),
    );
    pane.tick(1_200);
    assert_eq!(pane.status(), &LinkStatus::Failed(LinkFault::new("closed")));
    pane.key(&key("enter"), 1_300);
    assert_eq!(
        pane.status(),
        &LinkStatus::Failed(LinkFault::new("again")),
        "a refused retry is the new status"
    );
}

#[test]
fn jackin_policy_replacement_reaches_the_controller() {
    let words = words();
    let bench = Bench::new();
    let mut pane = live(&bench);
    pane.focus_changed(true, 900);
    pane.key(&key("a"), 1_000);
    pane.set_policy(&Arc::new(PanePolicy {
        clipboard_write: ClipboardWrite::Deny,
        paste_max_bytes: 4,
        refusal_status_every_ms: 1,
        ..PanePolicy::default()
    }));
    bench.output(0, b"\x1b]52;c;SGVsbG8=\x07");
    pane.tick(1_100);
    assert_eq!(
        pane.take_effects(&words),
        [status_effect(
            fill_in(&words.clip_write_blocked, &[("source", "Codex")]),
            Tone::Warning
        )],
        "the replaced policy denies a write the default allowed"
    );
    assert_eq!(
        pane.paste("12345", 1_200),
        jackin_console::panes::PasteOutcome::TooLarge
    );

    let timing = PaneTiming {
        replay_cap_ms: 100,
        nudge_ms: 50,
        quiet_ms: 10,
    };
    let bench = Bench::new();
    let mut quick = TerminalPane::new(1, codex(), bench.clone(), timing, 0);
    quick.set_policy(&Arc::new(PanePolicy::default()));
    quick.sync_size(inner(80, 24));
    bench.push(0, LinkEvent::Attached);
    bench.push(0, LinkEvent::Lease(Lease::Owner));
    quick.tick(0);
    quick.tick(50);
    assert_eq!(
        quick.status(),
        &LinkStatus::Attached(Lease::Owner),
        "a policy replacement keeps the pane's own replay timing"
    );
}
