//! Leaving a Capsule through the public `App::go` takes the focus away from
//! its panes at once: a program that is no longer on screen neither counts
//! as watched nor may write to the clipboard.

use crate::capsule_support::*;
use crate::pane_support::*;
use jackin_console::app::Remote;
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{AgentState, InstanceEntry, InstanceId, InstanceStatus};
use jackin_console::panes::{Lease, LinkEvent, PaneCommand, PaneEffect, PanePolicy};
use jackin_console::screens::Go;
use jackin_console::world::Msg;
use junie_tui::core::event::{External, Input};
use junie_tui::runtime::Application;

const OTHER: &str = "jk-zzz";
const FOCUS_OUT: &[u8] = b"\x1b[O";
const REPORTING: &[u8] = b"\x1b[?1004h";
const HELLO: &[u8] = b"\x1b]52;c;SGVsbG8=\x07";

fn out() -> PaneCommand {
    PaneCommand::Input(FOCUS_OUT.to_vec())
}

fn other_id() -> InstanceId {
    InstanceId::simulated(OTHER)
}

/// Two running instances. Only the first has a daemon: attaching builds the
/// second one.
fn two() -> Rig {
    Rig::with(PanePolicy::default(), |w| {
        w.instances
            .push(InstanceEntry::available(instance_named(OTHER)));
    })
}

/// The first instance attached and owned, its program asking for focus
/// reports.
fn reporting() -> Rig {
    let mut rig = two();
    rig.attach();
    rig.bench.output(0, REPORTING);
    rig.advance(80);
    assert_eq!(rig.bench.commands(0), [], "the pane has the focus");
    rig
}

fn attach_other(rig: &mut Rig) {
    let before = rig.bench.opened();
    rig.app.go(Go::Attach {
        instance: other_id(),
        pane: None,
    });
    rig.advance(160);
    assert_eq!(
        rig.bench.opened(),
        before + 1,
        "the new pane opened its link"
    );
    rig.bench.push(before, LinkEvent::Attached);
    rig.bench.push(before, LinkEvent::Lease(Lease::Owner));
    rig.advance(900);
}

fn wake(rig: &mut Rig, instance: &InstanceId) {
    let pane = rig.app.world.daemons[instance].panes[0].id;
    Application::handle(
        &mut rig.app,
        Input::External(External::new(Remote::Msg(Msg::PaneWake {
            instance: instance.clone(),
            pane,
        }))),
    );
}

fn state(rig: &Rig, instance: &InstanceId) -> AgentState {
    rig.app.world.daemons[instance].panes[0].body.state()
}

fn focus_outs(rig: &Rig, links: &[usize]) -> usize {
    links
        .iter()
        .flat_map(|n| rig.bench.commands(*n))
        .filter(|c| *c == out())
        .count()
}

#[test]
fn switching_capsules_blurs_only_old_daemon() {
    let mut rig = reporting();
    attach_other(&mut rig);

    assert_eq!(rig.bench.commands(0), [out()], "the old program lost focus");
    assert_eq!(
        rig.backend.left(),
        [INSTANCE],
        "one callback, for the old one"
    );
    assert_eq!(
        rig.backend.left_seen(),
        [vec![vec![out()]]],
        "the backend heard after the old pane was blurred"
    );

    rig.bench.output(1, REPORTING);
    rig.advance(80);
    assert_eq!(rig.bench.commands(1), [], "the new pane kept the focus");

    let first = instance_id();
    let second = other_id();
    rig.bench.output(0, b"\x07");
    wake(&mut rig, &first);
    assert_eq!(state(&rig, &first), AgentState::Blocked);
    rig.bench.output(1, b"\x07");
    wake(&mut rig, &second);
    assert_eq!(
        state(&rig, &second),
        AgentState::Unknown,
        "a bell in the pane on screen asks for nothing"
    );

    rig.app.go(Go::Manager);
    assert_eq!(
        rig.backend.left(),
        [INSTANCE.to_owned(), other_id().short_label()]
    );
    assert_eq!(rig.bench.commands(1), [out()]);
    assert_eq!(
        rig.bench.commands(0),
        [out()],
        "the old one was not told twice"
    );
}

#[test]
fn same_or_refused_attach_keeps_capsule_focus() {
    let mut rig = reporting();

    rig.app.go(Go::Attach {
        instance: instance_id(),
        pane: None,
    });
    rig.advance(80);
    assert_eq!(
        rig.bench.commands(0),
        [],
        "the same instance is not leaving"
    );
    assert_eq!(rig.backend.left(), Vec::<String>::new());

    rig.app
        .world
        .instance_mut(&other_id())
        .expect("instance")
        .status = InstanceStatus::RestoreAvailable;
    rig.app.go(Go::Attach {
        instance: other_id(),
        pane: None,
    });
    rig.app.go(Go::Attach {
        instance: InstanceId::simulated("jk-nobody"),
        pane: None,
    });
    rig.advance(80);
    assert!(
        rig.status().starts_with("Cannot attach"),
        "{}",
        rig.status()
    );
    assert_eq!(rig.bench.commands(0), [], "a refusal is not leaving");
    assert_eq!(rig.backend.left(), Vec::<String>::new());

    let first = instance_id();
    rig.bench.output(0, b"\x07");
    wake(&mut rig, &first);
    assert_eq!(state(&rig, &first), AgentState::Unknown, "still watched");
}

#[test]
fn new_session_in_other_instance_blurs_and_notifies_once() {
    let mut rig = reporting();
    rig.app.go(Go::NewSession {
        instance: other_id(),
        agent: Some(Agent::Codex),
        account: None,
    });

    assert_eq!(
        rig.bench.commands(0),
        [out()],
        "one focus-out for the old pane"
    );
    assert_eq!(rig.backend.left(), [INSTANCE], "one callback");
    assert_eq!(
        rig.backend.left_seen(),
        [vec![vec![out()]]],
        "after the blur, not before"
    );
    assert_eq!(
        rig.app.world.daemons[&other_id()].panes.len(),
        2,
        "the first pane and the one the session asked for"
    );

    rig.advance(160);
    let links: Vec<usize> = (1..rig.bench.opened()).collect();
    assert!(!links.is_empty(), "the new Capsule opened its links");
    for link in &links {
        rig.bench.push(*link, LinkEvent::Attached);
        rig.bench.push(*link, LinkEvent::Lease(Lease::Owner));
        rig.bench.output(*link, REPORTING);
    }
    rig.advance(900);
    assert_eq!(focus_outs(&rig, &links), 0, "the new Capsule is watched");
    rig.app.go(Go::Manager);
    assert_eq!(
        focus_outs(&rig, &links),
        1,
        "and it was the only focused pane"
    );
    assert_eq!(rig.bench.commands(0), [out()]);
    assert_eq!(
        rig.backend.left(),
        [INSTANCE.to_owned(), other_id().short_label()]
    );
}

#[test]
fn new_session_in_the_open_instance_is_not_leaving() {
    let mut rig = reporting();
    rig.app.go(Go::NewSession {
        instance: instance_id(),
        agent: Some(Agent::Codex),
        account: None,
    });
    rig.advance(80);
    assert_eq!(rig.backend.left(), Vec::<String>::new());
    assert_eq!(
        rig.bench.commands(0),
        [out()],
        "the old pane lost the focus to the new tab, through the Capsule"
    );
}

#[test]
fn offscreen_capsule_bell_marks_attention() {
    let mut rig = two();
    rig.attach();
    let first = instance_id();
    assert_eq!(state(&rig, &first), AgentState::Unknown);

    rig.app.go(Go::Manager);
    rig.bench.output(0, b"\x07");
    wake(&mut rig, &first);
    assert_eq!(
        state(&rig, &first),
        AgentState::Blocked,
        "the wake alone, before any tick, saw an unwatched pane"
    );
}

#[test]
fn offscreen_capsule_cannot_write_clipboard_after_gesture() {
    let mut rig = two();
    rig.attach();
    rig.press("a");
    rig.bench.output(0, HELLO);
    rig.advance(80);
    assert_eq!(
        *rig.copied.0.lock().expect("sink"),
        ["Hello"],
        "on screen it may"
    );

    rig.press("a");
    rig.app.go(Go::Manager);
    rig.bench.output(0, HELLO);
    wake(&mut rig, &instance_id());

    let effects = rig
        .app
        .world
        .daemons
        .get_mut(&instance_id())
        .expect("daemon")
        .panes[0]
        .body
        .take_effects(&words());
    assert!(
        !effects.iter().any(|e| matches!(e, PaneEffect::Copy { .. })),
        "{effects:?}"
    );
    assert!(
        effects.iter().any(|e| matches!(
            e,
            PaneEffect::Status { text, .. } if text.contains("blocked")
        )),
        "the refusal is worded: {effects:?}"
    );
    assert_eq!(*rig.copied.0.lock().expect("sink"), ["Hello"]);
}
