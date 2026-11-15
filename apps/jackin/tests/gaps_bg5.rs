#![cfg(feature = "demo")]
//! The Capsule pane gaps the jackin pane step (J5) found. The rule behind
//! every row is the one of BG to BG4: jackin writes no screen text, so every
//! sentence comes from `PaneWords` and every live fact arrives through a
//! typed value.

mod capsule_support;
mod common;
mod pane_support;
mod product_support;

use capsule_support::*;
use common::app_with;
use jackin_console::app::Remote;
use jackin_console::domain::instance::{DaemonSnapshot, InstanceEntry, InstanceId};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::panes::{
    EndKind, FailNext, LinkEvent, LinkFault, LinkStatus, PaneBody, PaneCommand, PaneEffect, PaneId,
};
use jackin_console::screens::Go;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, World};
use junie_tui::core::event::{External, Input, Outcome};
use junie_tui::runtime::Application;
use junie_tui::theme::Tone;
use pane_support::*;

fn wake(rig: &mut Rig, pane: PaneId) -> Outcome {
    wake_of(rig, INSTANCE, pane)
}

fn wake_of(rig: &mut Rig, instance: &str, pane: PaneId) -> Outcome {
    let msg = Msg::PaneWake {
        instance: InstanceId::simulated(instance),
        pane,
    };
    Application::handle(
        &mut rig.app,
        Input::External(External::new(Remote::Msg(msg))),
    )
}

fn first_pane(rig: &Rig) -> PaneId {
    rig.daemon().panes[0].id
}

#[test]
fn a_wake_shows_output_before_the_tick() {
    let mut rig = Rig::new();
    rig.attach();
    let pane = first_pane(&rig);
    let at = rig.app.world.now_ms();

    rig.bench.output(0, b"wake canary");
    assert!(
        !rig.screen().contains("wake canary"),
        "without a wake the output waits for the tick"
    );

    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
    assert!(rig.screen().contains("wake canary"), "{}", rig.screen());
    assert_eq!(rig.app.world.now_ms(), at, "no tick ran");
}

#[test]
fn wake_is_coalesced() {
    let mut rig = Rig::new();
    rig.attach();
    let pane = first_pane(&rig);

    rig.bench.output(0, b"burst one ");
    rig.bench.output(0, b"burst two");
    let outcomes: Vec<Outcome> = (0..5).map(|_| wake(&mut rig, pane)).collect();
    assert_eq!(
        outcomes,
        [
            Outcome::Changed,
            Outcome::Ignored,
            Outcome::Ignored,
            Outcome::Ignored,
            Outcome::Ignored
        ],
        "five wakes before the next frame ask for one redraw"
    );
    assert!(rig.squashed().contains("burst one burst two"));

    rig.bench.output(0, b" burst three");
    assert_eq!(
        wake(&mut rig, pane),
        Outcome::Changed,
        "output that arrived after the redraw asks for the next one"
    );
    assert!(rig.squashed().contains("burst one burst two burst three"));
}

#[test]
fn a_wake_for_nothing_changes_nothing() {
    let mut rig = Rig::new();
    rig.attach();
    let pane = first_pane(&rig);
    rig.bench.output(0, b"unseen");

    assert_eq!(wake_of(&mut rig, "jk-nobody", pane), Outcome::Ignored);
    assert_eq!(wake(&mut rig, pane + 99), Outcome::Ignored);
    assert!(
        !rig.screen().contains("unseen"),
        "a stray wake drains no pane"
    );
    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
}

#[test]
fn the_backend_hears_when_the_operator_leaves_the_capsule() {
    let mut rig = Rig::new();
    rig.attach();
    assert_eq!(rig.backend.left(), Vec::<String>::new());

    rig.press("ctrl+b c");
    rig.press("esc");
    rig.press("ctrl+b z");
    rig.press("ctrl+b z");
    rig.advance(400);
    assert_eq!(
        rig.backend.left(),
        Vec::<String>::new(),
        "a picker, a zoom and the clock are not leaving"
    );

    rig.press("ctrl+b d");
    assert_eq!(rig.backend.left(), [INSTANCE], "Ctrl+B d leaves");
    rig.advance(400);
    assert_eq!(rig.backend.left(), [INSTANCE], "once, not on every tick");
}

const FOCUS_OUT: &[u8] = b"\x1b[O";

/// An attached, owned Codex pane whose program asked for focus reports.
fn reporting_rig() -> Rig {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"\x1b[?1004h");
    rig.advance(80);
    assert_eq!(rig.bench.commands(0), [], "the pane has the focus");
    rig
}

#[test]
fn public_navigation_blurs_departed_capsule_before_callback() {
    type Destination = (&'static str, fn() -> Go);
    let destinations: [Destination; 4] = [
        ("manager", || Go::Manager),
        ("accounts", || Go::Accounts { select: None }),
        ("usage", || Go::Usage { select: None }),
        ("settings", || Go::Settings),
    ];
    for (name, destination) in destinations {
        let mut rig = reporting_rig();
        rig.app.go(destination());
        assert_eq!(rig.backend.left(), [INSTANCE], "{name}: one departure");
        assert_eq!(
            rig.backend.left_seen(),
            [vec![vec![PaneCommand::Input(FOCUS_OUT.to_vec())]]],
            "{name}: the program was told it lost the focus before the backend heard"
        );
    }
}

#[test]
fn detach_emits_one_focus_out() {
    let mut rig = reporting_rig();
    rig.press("ctrl+b d");
    let out = PaneCommand::Input(FOCUS_OUT.to_vec());
    assert_eq!(rig.bench.commands(0), std::slice::from_ref(&out));
    assert_eq!(rig.backend.left(), [INSTANCE]);
    assert_eq!(rig.backend.left_seen(), [vec![vec![out]]]);

    rig.app.go(Go::Accounts { select: None });
    rig.app.go(Go::Manager);
    rig.advance(400);
    assert_eq!(
        rig.bench.commands(0),
        [PaneCommand::Input(FOCUS_OUT.to_vec())],
        "navigating again says nothing"
    );
    assert_eq!(rig.backend.left(), [INSTANCE], "and leaves nothing again");
}

#[test]
fn ending_the_instance_leaves_the_capsule() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+q");
    rig.press("tab enter");
    assert_eq!(rig.backend.left(), [INSTANCE], "{}", rig.flat());
}

#[test]
fn a_failed_link_words_its_typed_next_step() {
    let bench = Bench::new();
    bench.refuse_with(LinkFault::new("the gateway is unreachable").then(FailNext::RunDoctor));
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));

    let said = "Press Enter to retry · could not attach to Codex: the gateway is unreachable · run jackin doctor";
    let note = pane.note(&words()).expect("a note");
    assert_eq!(note.text, said, "the key comes first, the next step last");
    assert_eq!(note.tone, Tone::Error);
    assert_eq!(
        pane.take_effects(&words()),
        [PaneEffect::Status {
            text: said.into(),
            tone: Tone::Error
        }]
    );
    assert_eq!(
        pane.status(),
        &LinkStatus::Failed(LinkFault::new("the gateway is unreachable").then(FailNext::RunDoctor))
    );
}

#[test]
fn a_link_that_ends_carries_its_next_step() {
    let bench = Bench::new();
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    bench.push(
        0,
        LinkEvent::Ended(EndKind::Refused(
            LinkFault::new("not the owner").then(FailNext::RunDoctor),
        )),
    );
    pane.tick(100);
    assert_eq!(
        pane.note(&words()).map(|n| n.text).as_deref(),
        Some("Press Enter to retry · could not attach to Codex: not the owner · run jackin doctor")
    );
}

#[test]
fn no_next_step_adds_nothing_to_the_note() {
    let bench = Bench::new();
    bench.refuse_with(LinkFault::new("the gateway is unreachable"));
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    assert_eq!(
        pane.note(&words()).map(|n| n.text).as_deref(),
        Some("Press Enter to retry · could not attach to Codex: the gateway is unreachable")
    );

    let closed = Bench::new();
    let mut silent = self::pane(&closed);
    silent.sync_size(inner(80, 24));
    closed.push(0, LinkEvent::Ended(EndKind::StreamClosed));
    silent.tick(100);
    assert_eq!(
        silent.note(&words()).map(|n| n.text).as_deref(),
        Some(
            "Press Enter to retry · could not attach to Codex: the connection closed before the pane attached"
        ),
        "a stream that closed says nothing more"
    );
}

#[test]
fn the_next_step_comes_from_the_pane_words() {
    let bench = Bench::new();
    bench.refuse_with(LinkFault::new("no route").then(FailNext::RunDoctor));
    let mut pane = pane(&bench);
    pane.sync_size(inner(80, 24));
    let mut canary = words();
    canary.next_doctor = "CANARY doctor".into();
    canary.failed_next = "{failed} // {next}".into();
    assert_eq!(
        pane.note(&canary).map(|n| n.text).as_deref(),
        Some("Press Enter to retry · could not attach to Codex: no route // CANARY doctor")
    );
    assert_eq!(
        pane.take_effects(&canary),
        [PaneEffect::Status {
            text: "Press Enter to retry · could not attach to Codex: no route // CANARY doctor"
                .into(),
            tone: Tone::Error
        }]
    );
}

/// The simulator, with the instance's daemon wiped from each record the way
/// a snapshot built by jackin carries it.
fn snapshot_without_daemons(world: &World) -> jackin_console::world::Snapshot {
    let mut snapshot = sim::snapshot(world);
    for instance in snapshot
        .instances
        .iter_mut()
        .filter_map(InstanceEntry::as_available_mut)
    {
        instance.daemon = DaemonSnapshot::Unavailable;
    }
    snapshot
}

#[test]
fn refresh_keeps_open_panes() {
    let mut world = world_for(Scenario::CapsuleMulti);
    world.backend = SimBackend::shared();
    let mut app = app_with(Scenario::CapsuleMulti, world);

    let before: Vec<(InstanceId, usize, usize)> = app
        .world
        .daemons
        .iter()
        .map(|(id, d)| (id.clone(), d.tabs.len(), d.panes.len()))
        .collect();
    let ids: Vec<Vec<PaneId>> = app
        .world
        .daemons
        .values()
        .map(|d| d.panes.iter().map(|p| p.id).collect())
        .collect();
    let open: DaemonSnapshot = app
        .world
        .instance(&InstanceId::simulated("jk-7f3a"))
        .expect("record")
        .daemon
        .clone();
    assert!(
        before.iter().any(|(_, tabs, _)| *tabs > 1),
        "the scenario has panes to lose"
    );

    let snapshot = snapshot_without_daemons(&app.world);
    assert!(
        snapshot
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .all(|i| i.daemon == DaemonSnapshot::Unavailable),
        "the snapshot says nothing about daemons"
    );
    let outcome = app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(Box::new(snapshot.clone())),
    });
    assert_ne!(outcome, Outcome::Ignored);

    let after: Vec<(InstanceId, usize, usize)> = app
        .world
        .daemons
        .iter()
        .map(|(id, d)| (id.clone(), d.tabs.len(), d.panes.len()))
        .collect();
    assert_eq!(after, before, "a refresh closes no tab and no pane");
    let after_ids: Vec<Vec<PaneId>> = app
        .world
        .daemons
        .values()
        .map(|d| d.panes.iter().map(|p| p.id).collect())
        .collect();
    assert_eq!(after_ids, ids, "the panes are the same panes");
    assert_eq!(
        app.world
            .instance(&InstanceId::simulated("jk-7f3a"))
            .expect("record")
            .daemon,
        open,
        "the roster shows the open tabs again"
    );

    let outcome = Application::handle(
        &mut app,
        Input::External(External::new(Remote::Reloaded(Box::new(snapshot)))),
    );
    assert_eq!(outcome, Outcome::Changed);
    assert_eq!(
        app.world.daemons[&InstanceId::simulated("jk-7f3a")]
            .panes
            .len(),
        before
            .iter()
            .find(|(id, ..)| id == &InstanceId::simulated("jk-7f3a"))
            .map(|(_, _, panes)| *panes)
            .expect("daemon"),
        "a reloaded snapshot keeps them too"
    );
}
