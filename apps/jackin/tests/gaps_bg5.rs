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
use jackin_console::domain::instance::{AgentState, DaemonSnapshot, InstanceEntry, InstanceId};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::panes::{
    EndKind, FailNext, Lease, LinkEvent, LinkFault, LinkStatus, PaneBody, PaneEffect, PaneId,
};
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

fn projected(rig: &Rig) -> DaemonSnapshot {
    rig.app
        .world
        .instance(&InstanceId::simulated(INSTANCE))
        .expect("record")
        .daemon
        .clone()
}

fn projected_states(rig: &Rig) -> Vec<AgentState> {
    match projected(rig) {
        DaemonSnapshot::Tabs(tabs) => tabs
            .iter()
            .flat_map(|t| t.panes.iter().map(|p| p.state))
            .collect(),
        other => panic!("{other:?}"),
    }
}

fn body_states(rig: &Rig) -> Vec<AgentState> {
    rig.daemon().panes.iter().map(|p| p.body.state()).collect()
}

/// Wipes the projection, so a later rebuild is visible.
fn blank_projection(rig: &mut Rig) {
    rig.app
        .world
        .instance_mut(&InstanceId::simulated(INSTANCE))
        .expect("record")
        .daemon = DaemonSnapshot::Unavailable;
}

fn open_second_tab(rig: &mut Rig) {
    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(80);
    let n = rig.bench.opened() - 1;
    rig.bench.push(n, LinkEvent::Attached);
    rig.bench.push(n, LinkEvent::Lease(Lease::Owner));
    rig.advance(900);
}

#[test]
fn wake_exit_updates_daemon_projection_before_next_tick() {
    let mut rig = Rig::new();
    rig.attach();
    let pane = first_pane(&rig);
    assert_eq!(projected_states(&rig), [AgentState::Unknown]);

    rig.bench.push(0, LinkEvent::Exited(Some(0)));
    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
    assert_eq!(body_states(&rig), [AgentState::Done]);
    assert_eq!(
        projected_states(&rig),
        [AgentState::Done],
        "the roster agrees before any tick"
    );

    rig.advance(400);
    assert_eq!(body_states(&rig), [AgentState::Done]);
    assert_eq!(
        projected_states(&rig),
        [AgentState::Done],
        "empty ticks do not undo it"
    );
}

#[test]
fn wake_background_attention_updates_manager_row() {
    let mut rig = Rig::new();
    rig.attach();
    open_second_tab(&mut rig);
    let pane = first_pane(&rig);
    assert_eq!(
        projected_states(&rig),
        [AgentState::Unknown, AgentState::Unknown]
    );

    rig.bench.output(0, b"\x07");
    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
    assert_eq!(
        body_states(&rig),
        [AgentState::Blocked, AgentState::Unknown]
    );
    assert_eq!(
        projected_states(&rig),
        [AgentState::Blocked, AgentState::Unknown],
        "a wake carries the bell to the roster"
    );

    rig.press("ctrl+b d");
    let manager = rig.flat();
    let mut ticked = Rig::new();
    ticked.attach();
    open_second_tab(&mut ticked);
    ticked.bench.output(0, b"\x07");
    ticked.advance(80);
    ticked.press("ctrl+b d");
    assert_eq!(projected_states(&ticked), projected_states(&rig));
    assert_eq!(
        manager,
        ticked.flat(),
        "the Manager reads the same after a wake as after a tick"
    );
}

#[test]
fn wake_failure_matches_tick_projection() {
    let refused = |rig: &mut Rig| {
        rig.advance(80);
        let fault = LinkFault::new("the gateway is unreachable").then(FailNext::RunDoctor);
        rig.bench.push(0, LinkEvent::Ended(EndKind::Refused(fault)));
    };
    let mut woken = Rig::new();
    refused(&mut woken);
    let pane = first_pane(&woken);
    assert_eq!(wake(&mut woken, pane), Outcome::Changed);

    let mut ticked = Rig::new();
    refused(&mut ticked);
    ticked.advance(80);

    assert_eq!(body_states(&woken), [AgentState::Blocked]);
    assert_eq!(body_states(&woken), body_states(&ticked));
    assert_eq!(projected(&woken), projected(&ticked));
    assert_eq!(
        woken.daemon().panes[0].body.note(&words()),
        ticked.daemon().panes[0].body.note(&words())
    );
}

#[test]
fn empty_wake_does_not_rebuild_projection() {
    let mut rig = Rig::new();
    rig.attach();
    let pane = first_pane(&rig);
    rig.bench.push(0, LinkEvent::Exited(Some(0)));
    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
    assert_eq!(projected_states(&rig), [AgentState::Done]);

    blank_projection(&mut rig);
    for _ in 0..100 {
        assert_eq!(wake(&mut rig, pane), Outcome::Ignored);
    }
    assert_eq!(
        projected(&rig),
        DaemonSnapshot::Unavailable,
        "an empty wake rebuilt nothing"
    );
}

#[test]
fn wake_missing_target_is_ignored() {
    let mut rig = Rig::new();
    rig.attach();
    let pane = first_pane(&rig);
    rig.bench.push(0, LinkEvent::Exited(Some(0)));
    blank_projection(&mut rig);

    assert_eq!(wake_of(&mut rig, "jk-nobody", pane), Outcome::Ignored);
    assert_eq!(wake(&mut rig, pane + 99), Outcome::Ignored);
    assert_eq!(body_states(&rig), [AgentState::Unknown]);
    assert_eq!(projected(&rig), DaemonSnapshot::Unavailable);

    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
    assert_eq!(projected_states(&rig), [AgentState::Done]);
}

#[test]
fn a_wake_pumps_only_its_pane() {
    let mut rig = Rig::new();
    rig.attach();
    open_second_tab(&mut rig);
    let panes: Vec<PaneId> = rig.daemon().panes.iter().map(|p| p.id).collect();
    let drains = |rig: &Rig, n: usize| rig.bench.log(n).lock().expect("log").drains.len();
    let (first, second) = (drains(&rig, 0), drains(&rig, 1));

    rig.bench.push(0, LinkEvent::Exited(Some(1)));
    assert_eq!(wake(&mut rig, panes[0]), Outcome::Changed);
    assert_eq!(
        drains(&rig, 0),
        first + 1,
        "the addressed pane drained once"
    );
    assert_eq!(drains(&rig, 1), second, "its neighbour did not");
    assert_eq!(
        projected_states(&rig),
        [AgentState::Blocked, AgentState::Unknown]
    );
}

#[test]
fn wake_replay_expiry_preserves_changed_without_summary_change() {
    let mut rig = Rig::new();
    rig.advance(80);
    rig.bench.push(0, LinkEvent::Attached);
    rig.bench.push(0, LinkEvent::Lease(Lease::Owner));
    rig.advance(20);
    rig.bench.output(0, b"replayed canary");
    rig.advance(20);
    let pane = first_pane(&rig);
    assert!(!rig.screen().contains("replayed canary"));
    let before = projected(&rig);

    rig.app.world.clock.advance(900);
    assert_eq!(wake(&mut rig, pane), Outcome::Changed);
    assert!(rig.screen().contains("replayed canary"), "{}", rig.screen());
    assert_eq!(projected(&rig), before, "the summary did not change");
    assert_eq!(wake(&mut rig, pane), Outcome::Ignored);
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
