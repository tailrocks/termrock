#![cfg(feature = "demo")]

use jackin_console::app::{App, Remote, Route};
use jackin_console::domain::instance::{InstanceEntry, InstanceId, InstanceStatus};
use jackin_console::domain::workspace::Workspace;
use jackin_console::fixtures::{Scenario, app_for, world_for};
use jackin_console::screens::Go;
use jackin_console::sim;
use jackin_console::world::{Msg, Snapshot};
use junie_tui::core::event::{External, Input, Key, Outcome};
use junie_tui::motion::Motion;
use junie_tui::runtime::{
    Application, ExternalPolicy, ExternalReceiver, ExternalSendError, ExternalSender,
    bounded_external_channel, external_channel,
};
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

const STOPPED: &str = "jk-7f3a";

fn instance_id(id: &str) -> InstanceId {
    InstanceId::simulated(id)
}

fn deliver_roster(app: &mut App, snapshot: Snapshot) -> Outcome {
    app.world.refresh_from = Some(app.world.authoritative_revision());
    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(Box::new(snapshot)),
    })
}

fn app() -> App {
    app_for(
        Scenario::Returning,
        Motion::Paused,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn linked() -> (App, ExternalSender) {
    let (tx, rx) = external_channel();
    (app().with_external(rx), tx)
}

fn pump(app: &mut App) -> Vec<Outcome> {
    let mut outcomes = Vec::new();
    while let Some(message) = Application::external(app).and_then(ExternalReceiver::try_recv) {
        outcomes.push(Application::handle(app, Input::External(message)));
    }
    outcomes
}

/// The reply to a stop: the roster with the instance no longer running.
fn stopped(app: &App, id: &str) -> Msg {
    let mut snapshot = sim::snapshot(&app.world);
    for i in snapshot
        .instances
        .iter_mut()
        .filter_map(InstanceEntry::as_available_mut)
        .filter(|i| i.id == instance_id(id))
    {
        i.status = InstanceStatus::RestoreAvailable;
    }
    Msg::Stopped {
        instance: instance_id(id),
        result: Ok(Box::new(snapshot)),
    }
}

fn status_of(app: &App, id: &str) -> InstanceStatus {
    app.world
        .instance(&instance_id(id))
        .expect("fixture instance")
        .status
}

fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_application_exposes_the_receiver_it_was_given() {
    let (with, _tx) = linked();
    assert!(Application::external(&with).is_some());
    assert!(Application::external(&app()).is_none());
}

#[test]
fn a_remote_message_reaches_the_screens_like_a_scheduled_one() {
    let (mut app, tx) = linked();
    assert_eq!(status_of(&app, STOPPED), InstanceStatus::Running);
    let running = app.world.running_count();
    assert!(tx.send(Remote::Msg(stopped(&app, STOPPED))));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);
    assert_ne!(status_of(&app, STOPPED), InstanceStatus::Running);
    assert_eq!(app.world.running_count(), running - 1);
    assert_eq!(app.world.arbiter.running(), Ok(running - 1));
}

fn bounded(capacity: usize) -> (App, ExternalSender) {
    let (tx, rx) = bounded_external_channel(
        std::num::NonZeroUsize::new(capacity).expect("nonzero"),
        ExternalPolicy::default(),
    );
    (app().with_external(rx), tx)
}

#[test]
fn a_full_bounded_channel_hands_the_message_back_and_the_retry_is_applied() {
    let (mut app, tx) = bounded(1);
    let first = External::new(Remote::Msg(stopped(&app, STOPPED)));
    let second = External::new(Remote::Msg(stopped(&app, "jk-9b02")));

    assert_eq!(tx.try_send(first), Ok(()));
    let Err(ExternalSendError::Full(retained)) = tx.try_send(second.clone()) else {
        panic!("capacity 1 is full after one message");
    };
    assert_eq!(retained, second, "the producer keeps the payload to retry");
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);
    assert_ne!(status_of(&app, STOPPED), InstanceStatus::Running);
    assert_eq!(status_of(&app, "jk-9b02"), InstanceStatus::Running);

    assert_eq!(tx.try_send(retained), Ok(()));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);
    assert_ne!(status_of(&app, "jk-9b02"), InstanceStatus::Running);
}

#[test]
fn the_application_reports_the_policy_of_the_receiver_it_was_given() {
    let (with, _tx) = linked();
    let rx = Application::external(&with).expect("receiver");
    assert_eq!(rx.policy(), ExternalPolicy::default());
}

#[test]
fn deliver_runs_a_message_without_the_channel() {
    let mut app = app();
    let msg = stopped(&app, STOPPED);
    assert_eq!(app.deliver(&msg), Outcome::Changed);
    assert_ne!(status_of(&app, STOPPED), InstanceStatus::Running);
}

#[test]
fn a_payload_of_another_type_is_ignored() {
    let (mut app, tx) = linked();
    let before = app.world.running_count();
    assert!(tx.send(42_u32));
    assert_eq!(pump(&mut app), vec![Outcome::Ignored]);
    assert_eq!(
        Application::handle(&mut app, Input::External(External::new("text"))),
        Outcome::Ignored
    );
    assert_eq!(app.world.running_count(), before);
}

#[test]
fn a_reloaded_snapshot_shows_on_the_next_frame() {
    let (mut app, tx) = linked();
    let name = "reloaded-by-snapshot";
    assert!(!screen(&mut app).contains(name));
    let mut workspace: Workspace = app.world.workspaces[0].clone();
    workspace.id = 900;
    workspace.name = name.into();
    let w = &app.world;
    let mut workspaces = w.workspaces.clone();
    workspaces.push(workspace);
    let snapshot = Snapshot {
        workspaces,
        roles: w.roles.clone(),
        instances: w.instances.clone(),
        global: w.global.clone(),
        accounts: w.accounts.clone(),
        surfaces: w.surfaces.clone(),
        github: w.github.clone(),
        index_error: None,
        daemon_health: w.daemon_health,
    };
    assert!(tx.send(Remote::Reloaded(Box::new(snapshot))));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);
    assert!(app.world.workspaces.iter().any(|w| w.name == name));
    assert!(screen(&mut app).contains(name));
}

fn without(app: &App, id: &str) -> Snapshot {
    let mut snapshot = sim::snapshot(&app.world);
    snapshot.instances.retain(|i| i.id() != &instance_id(id));
    snapshot
}

#[test]
fn a_reload_that_removes_the_open_instance_leaves_its_capsule() {
    let (mut app, tx) = linked();
    app.go(Go::Attach {
        instance: instance_id(STOPPED),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);
    assert!(app.world.daemons.contains_key(&instance_id(STOPPED)));

    assert!(tx.send(Remote::Reloaded(Box::new(without(&app, STOPPED)))));
    assert_eq!(pump(&mut app), vec![Outcome::Changed]);

    assert_ne!(app.route(), Route::Capsule);
    assert!(app.world.instance(&instance_id(STOPPED)).is_none());
    assert!(!app.world.daemons.contains_key(&instance_id(STOPPED)));
}

#[test]
fn a_reload_that_removes_an_unopened_instance_drops_its_daemon() {
    let (mut app, tx) = linked();
    let other = "jk-9b02";
    assert!(app.world.daemons.contains_key(&instance_id(other)));

    assert!(tx.send(Remote::Reloaded(Box::new(without(&app, other)))));
    pump(&mut app);

    assert!(!app.world.daemons.contains_key(&instance_id(other)));
    assert!(app.world.daemons.contains_key(&instance_id(STOPPED)));
}

#[test]
fn a_periodic_refresh_that_removes_the_open_instance_leaves_its_capsule() {
    let mut app = app();
    app.go(Go::Attach {
        instance: instance_id(STOPPED),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);

    let gone = without(&app, STOPPED);
    assert_eq!(deliver_roster(&mut app, gone), Outcome::Changed);

    assert_ne!(app.route(), Route::Capsule);
    assert!(app.world.instance(&instance_id(STOPPED)).is_none());
    assert!(!app.world.daemons.contains_key(&instance_id(STOPPED)));
    let shown = screen(&mut app);
    assert!(
        shown.contains(&format!("Instance {STOPPED} was removed elsewhere")),
        "{shown}"
    );
}

#[test]
fn a_refresh_that_removes_the_open_instance_closes_the_capsule_dialog_over_it() {
    let mut app = app();
    app.go(Go::Attach {
        instance: instance_id(STOPPED),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);
    for (code, mods) in [
        (KeyCode::Char('b'), KeyModifiers::CONTROL),
        (KeyCode::Char('i'), KeyModifiers::NONE),
    ] {
        Application::handle(&mut app, Input::Key(Key { code, mods }));
        screen(&mut app);
    }
    assert!(
        screen(&mut app).contains("Debug info"),
        "the dialog is open"
    );

    let gone = without(&app, STOPPED);
    deliver_roster(&mut app, gone);

    assert_ne!(app.route(), Route::Capsule);
    let shown = screen(&mut app);
    assert!(
        !shown.contains("Debug info"),
        "the capsule's dialog does not outlive the capsule:\n{shown}"
    );
}

#[test]
fn a_periodic_refresh_that_removes_an_unopened_instance_drops_its_daemon() {
    let mut app = app();
    let other = "jk-9b02";
    assert!(app.world.daemons.contains_key(&instance_id(other)));

    let gone = without(&app, other);
    deliver_roster(&mut app, gone);

    assert!(!app.world.daemons.contains_key(&instance_id(other)));
    assert!(app.world.daemons.contains_key(&instance_id(STOPPED)));
}

fn restorable(app: &App, id: &str) -> Snapshot {
    let mut snapshot = sim::snapshot(&app.world);
    for i in snapshot
        .instances
        .iter_mut()
        .filter_map(InstanceEntry::as_available_mut)
        .filter(|i| i.id == instance_id(id))
    {
        i.status = InstanceStatus::RestoreAvailable;
    }
    snapshot
}

#[test]
fn a_reload_that_stops_the_open_instance_leaves_its_capsule() {
    let (mut app, tx) = linked();
    app.go(Go::Attach {
        instance: instance_id(STOPPED),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);

    assert!(tx.send(Remote::Reloaded(Box::new(restorable(&app, STOPPED)))));
    pump(&mut app);

    assert_ne!(app.route(), Route::Capsule);
    assert_eq!(
        status_of(&app, STOPPED),
        InstanceStatus::RestoreAvailable,
        "the backend's status stands"
    );
    assert!(!app.world.daemons.contains_key(&instance_id(STOPPED)));
    let shown = screen(&mut app);
    assert!(
        shown.contains(&format!("Instance {STOPPED} stopped elsewhere")),
        "{shown}"
    );
}

#[test]
fn a_periodic_refresh_that_stops_an_unopened_instance_drops_its_daemon() {
    let mut app = app();
    let other = "jk-9b02";
    assert!(app.world.daemons.contains_key(&instance_id(other)));

    let stopped = restorable(&app, other);
    deliver_roster(&mut app, stopped);

    assert!(!app.world.daemons.contains_key(&instance_id(other)));
    assert!(app.world.daemons.contains_key(&instance_id(STOPPED)));
}

fn with_roles(app: &App, roles: bool) -> Snapshot {
    let mut snapshot = sim::snapshot(&app.world);
    snapshot.surfaces.roles = roles
        .then(|| world_for(Scenario::RoleBinding).surfaces.roles)
        .flatten();
    snapshot
}

fn key(app: &mut App, c: char) {
    Application::handle(
        app,
        Input::Key(Key {
            code: KeyCode::Char(c),
            mods: KeyModifiers::NONE,
        }),
    );
    screen(app);
}

#[test]
fn a_reload_that_adds_the_roles_surface_opens_the_roles_tab() {
    let (mut app, tx) = linked();
    assert!(app.world.surfaces.roles.is_none());
    key(&mut app, 'R');
    assert_eq!(app.route(), Route::Manager, "no Roles tab yet");

    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, true)))));
    pump(&mut app);
    assert_eq!(app.route(), Route::Manager, "the tab in use stays");
    key(&mut app, 'R');
    assert_eq!(app.route(), Route::Roles);
}

#[test]
fn a_reload_that_removes_the_roles_surface_closes_the_roles_tab() {
    let (mut app, tx) = linked();
    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, true)))));
    pump(&mut app);
    key(&mut app, 'R');
    assert_eq!(app.route(), Route::Roles);

    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, false)))));
    pump(&mut app);
    assert_eq!(app.route(), Route::Manager);
    key(&mut app, 'R');
    assert_eq!(app.route(), Route::Manager, "the key is gone with the tab");
    let shown = screen(&mut app);
    assert!(!shown.contains("R Roles"), "{shown}");
}

#[test]
fn a_reload_that_adds_the_doctor_surface_binds_its_key() {
    let (mut app, tx) = linked();
    assert!(app.world.surfaces.doctor.is_none());
    key(&mut app, 'D');
    assert_ne!(app.route(), Route::Doctor, "no Doctor yet");

    let mut snapshot = sim::snapshot(&app.world);
    snapshot.surfaces.doctor = world_for(Scenario::Doctor).surfaces.doctor;
    assert!(tx.send(Remote::Reloaded(Box::new(snapshot))));
    pump(&mut app);
    key(&mut app, 'D');
    assert_eq!(app.route(), Route::Doctor);
}

fn down(app: &mut App) {
    Application::handle(
        app,
        Input::Key(Key {
            code: KeyCode::Down,
            mods: KeyModifiers::NONE,
        }),
    );
    screen(app);
}

#[test]
fn a_reload_that_removes_the_roles_surface_closes_the_modals_the_roles_page_opened() {
    let (mut app, tx) = linked();
    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, true)))));
    pump(&mut app);
    key(&mut app, 'R');
    key(&mut app, 'a');
    let asked = screen(&mut app);
    assert!(asked.contains("register a role"), "{asked}");

    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, false)))));
    pump(&mut app);

    assert_eq!(app.route(), Route::Manager);
    let shown = screen(&mut app);
    assert!(!shown.contains("register a role"), "{shown}");
    down(&mut app);
    let moved = screen(&mut app);
    assert!(
        moved.contains("▎ ▸ payments-platform"),
        "the manager takes the keys:\n{moved}"
    );
}

#[test]
fn a_reload_that_removes_the_roles_surface_leaves_a_role_editor_in_use() {
    let (mut app, tx) = linked();
    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, true)))));
    pump(&mut app);
    key(&mut app, 'R');
    key(&mut app, 'e');
    assert_eq!(app.route(), Route::RoleEditor);

    assert!(tx.send(Remote::Reloaded(Box::new(with_roles(&app, false)))));
    pump(&mut app);

    assert_eq!(app.route(), Route::RoleEditor);
    let shown = screen(&mut app);
    assert!(shown.contains("Roles › sentinel › edit"), "{shown}");
}
