//! The Capsule around terminal panes: how it builds them, sizes them, tells
//! them about focus and closes them. The panes run on the test links of
//! `pane_support`; no `demo` feature is needed.

mod capsule_support;
mod pane_support;
mod product_support;

use capsule_support::*;
use jackin_console::domain::account::{Account, CredentialSource, Lifecycle};
use jackin_console::domain::agent::{Agent, Provider};
use jackin_console::domain::instance::{
    AgentState, DaemonSnapshot, InstanceEntry, InstanceId, InstanceStatus, SessionRecord,
    SessionStatus,
};
use jackin_console::domain::workspace::Workspace;
use jackin_console::panes::PanePolicy;
use jackin_console::panes::{EndKind, Lease, LinkEvent, LinkFault, PaneCommand, PaneRequest};
use jackin_console::screens::Go;
use jackin_console::world::{Msg, Snapshot, World};
use pane_support::*;

const FOCUS_OUT: &[u8] = b"\x1b[O";
const FOCUS_IN: &[u8] = b"\x1b[I";

fn lines(n: usize) -> Vec<u8> {
    (0..n)
        .flat_map(|i| format!("l{i}\r\n").into_bytes())
        .collect()
}

/// Opens a shell pane beside the focused one and runs it past its replay.
fn split_shell(rig: &mut Rig) {
    rig.press("ctrl+b %");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(80);
    let n = rig.bench.opened() - 1;
    rig.bench.push(n, LinkEvent::Attached);
    rig.bench.push(n, LinkEvent::Lease(Lease::Owner));
    rig.advance(900);
}

fn tab_strip(rig: &mut Rig) -> String {
    rig.screen()
        .lines()
        .find(|l| l.contains("Codex") && l.contains("Shell"))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn daemon_is_built_by_attach() {
    let mut rig = Rig::new();
    let daemon = rig.daemon();
    assert_eq!(daemon.workspace, "dev");
    assert_eq!(daemon.tabs.len(), 1);
    assert_eq!(daemon.panes.len(), 1);
    assert_eq!(
        *rig.backend.requests.lock().expect("requests"),
        [PaneRequest::new(&instance_id(), Some(Agent::Codex), None)],
        "the pane comes from what the backend asks for"
    );
    let first = rig.squashed();
    assert!(first.contains("Waiting for room"), "{first}");
    assert_eq!(
        rig.bench.opened(),
        0,
        "no link before a tick has given it a size"
    );

    rig.advance(80);
    assert_eq!(rig.bench.opened(), 1);
    assert!(rig.squashed().contains("Attaching to Codex…"));
    let first_size = rig.bench.sizes(0)[0];
    assert!(
        first_size.cols > 100 && first_size.rows > 20,
        "{first_size:?}"
    );
}

#[test]
fn a_pane_outlives_its_instance_record() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"still here");
    rig.advance(80);
    assert!(rig.screen().contains("still here"));

    assert!(
        rig.screen().contains("the-architect"),
        "the identity row names the role"
    );

    rig.app.world.instances.clear();
    rig.advance(80);
    rig.press("ctrl+b i");
    rig.press("ctrl+b u");
    rig.press("esc");
    let screen = rig.screen();
    assert!(
        screen.contains("still here"),
        "the pane still draws:\n{screen}"
    );
    assert!(
        !screen.contains("the-architect"),
        "the identity row is empty without the record:\n{screen}"
    );
}

#[test]
fn new_tab_asks_the_backend_for_a_shell_pane() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    let requests = rig.backend.requests.lock().expect("requests").clone();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1], PaneRequest::new(&instance_id(), None, None));
    assert_eq!(rig.daemon().tabs.len(), 2);

    rig.advance(80);
    assert_eq!(
        rig.bench.opened(),
        2,
        "the new tab's pane starts on its own tick"
    );
    assert_eq!(rig.bench.sizes(1), [rig.bench.sizes(0)[0]]);
}

#[test]
fn a_split_and_a_zoom_send_the_pane_its_new_size() {
    let mut rig = Rig::new();
    rig.attach();
    let full = rig.bench.sizes(0)[0];
    split_shell(&mut rig);
    let sizes = rig.bench.sizes(0);
    assert_eq!(sizes.len(), 2, "one new size for the split: {sizes:?}");
    assert_eq!(
        sizes[1].rows,
        full.rows - 2,
        "the frame takes a row above and below"
    );
    assert!(sizes[1].cols < full.cols / 2 + 2, "{sizes:?}");
    assert_eq!(rig.bench.opened(), 2, "the new pane started");

    rig.press("ctrl+b z");
    rig.advance(80);
    assert_eq!(
        rig.bench.sizes(1).last().copied(),
        Some(size(full.rows - 2, full.cols - 2)),
        "the zoomed pane has the body, framed"
    );
    assert_eq!(
        rig.bench.sizes(0).len(),
        2,
        "a pane that is hidden is not resized"
    );
}

#[test]
fn redraw_asks_the_program_and_the_menu_clear_only_clears_locally() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, &lines(100));
    rig.advance(80);
    rig.press("ctrl+b r");
    assert_eq!(rig.bench.commands(0), [PaneCommand::Redraw]);
    assert_eq!(rig.status(), "Redrawn");

    rig.press("f10 right down down down enter");
    assert_eq!(rig.status(), "Scrollback cleared");
    assert_eq!(
        rig.bench.commands(0),
        [PaneCommand::Redraw],
        "clearing sends the program nothing"
    );
}

#[test]
fn leaving_and_entering_a_tab_tells_the_program() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"\x1b[?1004h");
    rig.advance(80);
    assert_eq!(
        rig.bench.commands(0),
        [],
        "the pane has the focus: nothing to say"
    );
    let out = PaneCommand::Input(FOCUS_OUT.to_vec());
    let back = PaneCommand::Input(FOCUS_IN.to_vec());

    rig.press("ctrl+b c");
    assert_eq!(
        rig.bench.commands(0),
        std::slice::from_ref(&out),
        "a picker over the pane counts as leaving"
    );
    rig.typed("Shell");
    rig.press("enter");
    assert_eq!(
        rig.bench.commands(0),
        [out.clone(), back.clone(), out.clone()],
        "closing it was returning, and the new tab took the focus away"
    );

    rig.press("ctrl+b p");
    assert_eq!(rig.bench.commands(0).last(), Some(&back));

    rig.press("ctrl+\\");
    assert_eq!(
        rig.bench.commands(0).last(),
        Some(&out),
        "so does a palette"
    );
    rig.press("esc");
    assert_eq!(rig.bench.commands(0).last(), Some(&back));
    assert_eq!(rig.bench.commands(0).len(), 6);
}

#[test]
fn leaving_the_capsule_tells_the_program_it_lost_the_focus() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"\x1b[?1004h");
    rig.advance(80);
    assert_eq!(rig.bench.commands(0), []);
    rig.press("ctrl+b d");
    assert_eq!(
        rig.bench.commands(0),
        [PaneCommand::Input(FOCUS_OUT.to_vec())],
        "the manager is not the pane"
    );
}

#[test]
fn a_bell_in_a_background_tab_marks_the_tab() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(80);
    assert!(!tab_strip(&mut rig).contains('●'));
    rig.bench.output(0, b"\x07");
    rig.advance(80);
    assert!(tab_strip(&mut rig).contains('●'), "{}", rig.screen());

    rig.press("ctrl+b p");
    assert!(
        !tab_strip(&mut rig).contains('●'),
        "focusing the pane clears it"
    );
}

#[test]
fn closing_a_live_pane_asks_by_its_kind() {
    let mut rig = Rig::new();
    rig.attach();
    split_shell(&mut rig);

    rig.press("ctrl+b x");
    let shell = rig.flat();
    assert!(
        shell.contains("Close pane? The shell ends. Unsaved state in it is lost."),
        "{shell}"
    );
    rig.press("esc");

    rig.press("ctrl+b h ctrl+b x");
    let codex = rig.flat();
    assert!(
        codex.contains("Close pane? Codex keeps running in the sandbox. This pane detaches."),
        "{codex}"
    );
    rig.press("esc");

    rig.press("ctrl+b &");
    let tab = rig.flat();
    assert!(
        tab.contains(
            "Close tab? Closes every pane in this tab. Codex panes keep running; shells end."
        ),
        "{tab}"
    );
    rig.press("esc");
    assert_eq!(rig.daemon().panes.len(), 2, "nothing closed yet");
}

#[test]
fn a_pane_that_is_not_live_closes_without_a_dialog() {
    for end in [
        LinkEvent::Ended(EndKind::Detached),
        LinkEvent::Exited(Some(0)),
        LinkEvent::Ended(EndKind::Refused(LinkFault::new("no"))),
    ] {
        let mut rig = Rig::new();
        rig.attach();
        split_shell(&mut rig);
        rig.bench.push(1, end);
        rig.advance(80);
        rig.press("ctrl+b x");
        assert_eq!(
            rig.daemon().panes.len(),
            1,
            "closed at once:\n{}",
            rig.screen()
        );
        assert_eq!(rig.status(), "Pane closed");
    }

    let mut rig = Rig::new();
    rig.advance(80);
    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(80);
    rig.press("ctrl+b x");
    assert_eq!(
        rig.daemon().tabs.len(),
        1,
        "an attach in progress is cancelled"
    );
}

#[test]
fn enter_reattaches_a_detached_pane_and_the_replay_shows_once() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"first screen");
    rig.bench.push(0, LinkEvent::Ended(EndKind::Detached));
    rig.advance(80);
    assert!(
        rig.squashed()
            .contains("Detached from Codex · press Enter to reattach")
    );
    assert_eq!(
        rig.status(),
        "Detached from Codex · press Enter to reattach"
    );

    rig.press("enter");
    assert_eq!(rig.bench.opened(), 2);
    rig.bench.push(1, LinkEvent::Attached);
    rig.bench.push(1, LinkEvent::Lease(Lease::Owner));
    rig.bench.output(1, b"first screen");
    rig.advance(900);
    let screen = rig.screen();
    assert_eq!(screen.matches("first screen").count(), 1, "{screen}");
    assert_eq!(rig.bench.commands(1), [], "Enter was not forwarded");
}

#[test]
fn the_info_dialog_shows_the_title_the_program_set() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b i");
    assert!(!rig.squashed().contains("Title"), "no title, no row");
    rig.press("esc");

    rig.bench
        .output(0, "\x1b]2;build\u{202e}   done\x07".as_bytes());
    rig.advance(80);
    rig.press("ctrl+b i");
    let info = rig.flat();
    assert!(info.contains("Title build done"), "{info}");
}

#[test]
fn a_clipboard_write_by_the_program_follows_the_policy() {
    let mut rig = Rig::new();
    rig.attach();
    rig.bench.output(0, b"\x1b]52;c;SGVsbG8=\x07");
    rig.advance(80);
    assert_eq!(
        rig.status(),
        "Clipboard write from Codex blocked · select text to copy it",
        "nobody typed yet"
    );
    assert!(rig.copied.0.lock().expect("sink").is_empty());

    rig.press("a");
    rig.bench.output(0, b"\x1b]52;c;SGVsbG8=\x07");
    rig.advance(80);
    assert_eq!(rig.status(), "Copied from Codex · 5 characters");
    assert_eq!(*rig.copied.0.lock().expect("sink"), ["Hello"]);
}

#[test]
fn a_split_whose_focused_pane_vanished_starts_nothing() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b %");
    let gone = rig.daemon().next_id + 100;
    rig.app
        .world
        .daemons
        .get_mut(&instance_id())
        .expect("the daemon")
        .tabs[0]
        .focused = gone;
    rig.typed("Shell");
    rig.press("enter");

    let status = rig.status();
    assert!(status.contains("focused pane not found"), "{status}");
    assert!(!status.contains("Started"), "{status}");
    assert_eq!(rig.daemon().panes.len(), 1, "no pane was left behind");
    assert_eq!(rig.daemon().tabs[0].leaves().len(), 1);
    let sessions = rig
        .app
        .world
        .instance(&instance_id())
        .expect("instance")
        .sessions
        .clone();
    assert_eq!(sessions, Ok(Vec::new()), "no session was recorded");
}

#[test]
fn the_manager_shows_the_state_a_pane_reached_while_detached() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b d");
    let state = |rig: &Rig| match &rig
        .app
        .world
        .instance(&instance_id())
        .expect("instance")
        .daemon
    {
        DaemonSnapshot::Tabs(tabs) => tabs[0].panes[0].state,
        other => panic!("{other:?}"),
    };
    assert_eq!(state(&rig), AgentState::Unknown);
    rig.bench.push(0, LinkEvent::Exited(Some(0)));
    rig.advance(400);
    assert_eq!(state(&rig), AgentState::Done);
}

fn with_session(rig: &Rig, id: &str) -> Snapshot {
    let mut snapshot = snapshot_of(&rig.app.world);
    for entry in snapshot
        .instances
        .iter_mut()
        .filter(|entry| entry.id() == &instance_id())
    {
        if let Some(i) = entry.as_available_mut() {
            i.sessions = Ok(vec![SessionRecord {
                id: id.into(),
                agent: None,
                label: "Shell".into(),
                status: SessionStatus::Active,
                started_secs: 1_788_401_700,
            }]);
        }
    }
    snapshot
}

fn session_ids(rig: &Rig) -> Vec<String> {
    rig.app
        .world
        .instance(&instance_id())
        .expect("instance")
        .sessions
        .clone()
        .expect("sessions")
        .into_iter()
        .map(|s| s.id)
        .collect()
}

#[test]
fn a_new_pane_asks_the_backend_for_the_session_list_at_once() {
    let mut rig = Rig::new();
    rig.attach();
    assert!(rig.backend.refreshes().is_empty());

    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(160);

    let [ticket] = rig.backend.refreshes()[..] else {
        panic!("one refresh expected, got {:?}", rig.backend.refreshes());
    };
    let snapshot = with_session(&rig, "s-new");
    rig.app.deliver(&Msg::Refreshed {
        ticket,
        result: Ok(Box::new(snapshot)),
    });
    assert_eq!(session_ids(&rig), ["s-new"]);
}

#[test]
fn a_new_session_asks_the_backend_for_the_session_list_at_once() {
    let mut rig = Rig::new();
    rig.attach();

    rig.app.go(Go::NewSession {
        instance: instance_id(),
        agent: Some(Agent::Codex),
        account: None,
    });

    let [ticket] = rig.backend.refreshes()[..] else {
        panic!("one refresh expected, got {:?}", rig.backend.refreshes());
    };
    let snapshot = with_session(&rig, "s-new");
    rig.app.deliver(&Msg::Refreshed {
        ticket,
        result: Ok(Box::new(snapshot)),
    });
    assert_eq!(session_ids(&rig), ["s-new"]);
}

#[test]
fn a_refresh_asked_for_before_a_new_pane_cannot_hide_the_pane_session() {
    let mut rig = Rig::new();
    rig.attach();
    assert!(rig.app.world.refresh());

    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(160);

    let [before, after] = rig.backend.refreshes()[..] else {
        panic!("two refreshes expected, got {:?}", rig.backend.refreshes());
    };
    let older = snapshot_of(&rig.app.world);
    let newer = with_session(&rig, "s-new");
    rig.app.deliver(&Msg::Refreshed {
        ticket: after,
        result: Ok(Box::new(newer)),
    });
    rig.app.deliver(&Msg::Refreshed {
        ticket: before,
        result: Ok(Box::new(older)),
    });
    assert_eq!(session_ids(&rig), ["s-new"]);
}

#[test]
fn a_tick_that_changes_one_daemon_leaves_the_snapshots_of_the_others() {
    const OTHER: &str = "jk-zzz";
    let mut rig = Rig::with(PanePolicy::default(), |w| {
        let mut other = instance();
        other.id = InstanceId::simulated(OTHER);
        w.instances.push(InstanceEntry::available(other));
        w.open_first_session(&InstanceId::simulated(OTHER));
    });
    rig.advance(80);
    assert_eq!(rig.bench.opened(), 1);
    rig.bench.push(0, LinkEvent::Attached);
    rig.bench.push(0, LinkEvent::Lease(Lease::Owner));
    rig.advance(900);

    rig.app
        .world
        .instance_mut(&InstanceId::simulated(OTHER))
        .expect("instance")
        .daemon = DaemonSnapshot::Unavailable;
    rig.bench.push(0, LinkEvent::Exited(Some(0)));
    rig.advance(400);

    let snapshot = |rig: &Rig, id: &str| {
        rig.app
            .world
            .instance(&InstanceId::simulated(id))
            .expect("instance")
            .daemon
            .clone()
    };
    assert!(
        matches!(snapshot(&rig, INSTANCE), DaemonSnapshot::Tabs(_)),
        "the daemon that changed is read again"
    );
    assert_eq!(
        snapshot(&rig, OTHER),
        DaemonSnapshot::Unavailable,
        "a daemon that did not change is not read"
    );
}

#[test]
fn a_new_session_in_a_stopped_instance_is_refused() {
    let mut rig = Rig::new();
    rig.attach();
    let panes = rig.daemon().panes.len();
    rig.app
        .world
        .instance_mut(&instance_id())
        .expect("instance")
        .status = InstanceStatus::RestoreAvailable;

    rig.app.go(Go::NewSession {
        instance: instance_id(),
        agent: Some(Agent::Codex),
        account: None,
    });

    assert_eq!(
        rig.daemon().panes.len(),
        panes,
        "no pane for a stopped instance"
    );
    assert!(rig.status().contains("Cannot attach"), "{}", rig.status());
}

fn codex_account(id: &str, lifecycle: Lifecycle) -> Account {
    let mut account = Account::registered(
        id,
        id,
        Provider::OpenAi,
        CredentialSource::PlainApiKey {
            fingerprint: format!("fp-{id}"),
            tail: "abcd".into(),
        },
    );
    account.lifecycle = lifecycle;
    account
}

fn snapshot_of(world: &World) -> Snapshot {
    Snapshot {
        workspaces: world.workspaces.clone(),
        roles: world.roles.clone(),
        instances: world.instances.clone(),
        global: world.global.clone(),
        accounts: world.accounts.clone(),
        surfaces: world.surfaces.clone(),
        github: world.github.clone(),
        index_error: None,
        daemon_health: world.daemon_health,
    }
}

#[test]
fn a_new_pane_is_refused_when_the_chosen_account_became_unusable() {
    let mut rig = Rig::with(PanePolicy::default(), |w| {
        w.accounts
            .accounts
            .push(codex_account("acct-a", Lifecycle::Available));
        let mut ws = Workspace::new(1, "dev", "/sandbox/project");
        ws.accounts.enabled.insert("acct-a".into());
        ws.accounts
            .preferred
            .insert(Provider::OpenAi, "acct-a".into());
        w.workspaces.push(ws);
        w.instance_mut(&instance_id()).expect("instance").workspace = Some(1);
    });
    rig.attach();
    let panes = rig.daemon().panes.len();
    rig.press("ctrl+b c");
    let picker = rig.flat();
    assert!(picker.contains("New tab"), "{picker}");

    let mut snapshot = snapshot_of(&rig.app.world);
    snapshot
        .accounts
        .accounts
        .push(codex_account("acct-b", Lifecycle::Available));
    snapshot.workspaces[0]
        .accounts
        .enabled
        .insert("acct-b".into());
    snapshot
        .accounts
        .get_mut("acct-a")
        .expect("account")
        .lifecycle = Lifecycle::NeedsLogin;
    rig.app.world.refresh_from = Some(rig.app.world.authoritative_revision());
    rig.app.deliver(&Msg::Refreshed {
        ticket: rig.app.world.refresh_ticket,
        result: Ok(Box::new(snapshot)),
    });
    rig.press("enter");

    let shown = rig.flat();
    assert!(shown.contains("Codex could not start"), "{shown}");
    rig.press("enter");
    rig.advance(160);
    assert_eq!(
        rig.daemon().panes.len(),
        panes,
        "no pane for the other account"
    );
}

#[test]
fn a_new_pane_leaves_the_session_list_to_the_backend() {
    let mut rig = Rig::new();
    rig.attach();
    rig.press("ctrl+b c");
    rig.typed("Shell");
    rig.press("enter");
    rig.advance(160);
    assert_eq!(rig.daemon().tabs.len(), 2);

    let record = rig.app.world.instance(&instance_id()).expect("instance");
    assert_eq!(
        record.sessions,
        Ok(Vec::new()),
        "the Capsule wrote no session record into the world"
    );
}

/// Draws the rig's screen on a terminal of the given size.
fn draw_at(rig: &mut Rig, cols: u16, rows: u16) {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use termrock::runtime::Application;
    let mut terminal = Terminal::new(TestBackend::new(cols, rows)).expect("test terminal");
    terminal
        .draw(|f| Application::render(&mut rig.app, f))
        .expect("draw");
}

#[test]
fn a_split_of_a_pane_a_shrunk_terminal_left_three_rows_high_is_refused() {
    use termrock::core::event::Input;
    use termrock::runtime::Application;

    let mut rig = Rig::new();
    rig.attach();
    for _ in 0..2 {
        rig.press("ctrl+b \"");
        rig.typed("Shell");
        rig.press("enter");
        rig.advance(80);
        let n = rig.bench.opened() - 1;
        rig.bench.push(n, LinkEvent::Attached);
        rig.bench.push(n, LinkEvent::Lease(Lease::Owner));
        rig.advance(900);
    }
    assert_eq!(rig.daemon().panes.len(), 3);
    draw_at(&mut rig, 72, 20);
    for spec in ["ctrl+b", "\""] {
        Application::handle(&mut rig.app, Input::Key(key(spec)));
        draw_at(&mut rig, 72, 20);
    }
    for c in "Shell".chars() {
        Application::handle(&mut rig.app, Input::Key(key(&c.to_string())));
        draw_at(&mut rig, 72, 20);
    }
    Application::handle(&mut rig.app, Input::Key(key("enter")));
    draw_at(&mut rig, 72, 20);
    assert_eq!(
        rig.daemon().panes.len(),
        3,
        "no room to split a pane three rows high"
    );
    assert!(rig.status().starts_with("Cannot split"), "{}", rig.status());
}
