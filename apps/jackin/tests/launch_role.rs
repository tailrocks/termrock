#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::domain::account::Lifecycle;
use jackin_console::domain::role::{
    DEFAULT_ROLE_UNUSABLE, LaunchRole, LaunchRoleRule, NO_DEFAULT_ROLE, NO_SAVED_WORKSPACE,
};
use jackin_console::domain::workspace::{AllowedRoles, Workspace};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::sim;
use jackin_console::world::{LaunchSource, Msg, Snapshot, World};

fn world(rule: LaunchRoleRule) -> World {
    let mut world = world_for(Scenario::Returning);
    world.surfaces.launch_role = rule;
    world
}

fn workspace(world: &World) -> Workspace {
    let mut ws = world.workspaces[0].clone();
    ws.roles.default = None;
    ws.roles.last = None;
    ws.roles.allowed = AllowedRoles::All;
    ws
}

fn role(name: &str) -> LaunchRole {
    LaunchRole::Role(name.into())
}

fn names(world: &World, n: usize) -> Vec<String> {
    world.roles.iter().take(n).map(|r| r.name.clone()).collect()
}

#[test]
fn the_demo_rule_keeps_the_fixture_roles() {
    let world = world(LaunchRoleRule::Demo);
    let mut ws = workspace(&world);
    assert_eq!(
        world.launch_role(LaunchSource::Row(None)),
        role("the-architect")
    );
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("the-architect")
    );
    ws.roles.last = Some("reviewer".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("reviewer")
    );
    ws.roles.default = Some("data-eng".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role("data-eng")
    );
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(Some(&ws))),
        role("the-architect")
    );
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(None)),
        role("the-architect")
    );
}

#[test]
fn the_configured_rule_takes_the_default_role() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let default = world.roles[0].name.clone();
    ws.roles.default = Some(default.clone());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        role(&default)
    );
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(Some(&ws))),
        role(&default)
    );
}

#[test]
fn the_configured_rule_takes_the_only_allowed_role() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let only = names(&world, 3)[1].clone();
    ws.roles.allowed = AllowedRoles::Custom(vec![only.clone()]);
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), role(&only));
}

#[test]
fn the_configured_rule_asks_for_a_default_when_several_roles_are_allowed() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let unavailable = LaunchRole::Unavailable(NO_DEFAULT_ROLE.into());
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unavailable);
    ws.roles.allowed = AllowedRoles::Custom(names(&world, 2));
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unavailable);
}

#[test]
fn the_configured_rule_asks_when_no_allowed_role_is_configured() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    ws.roles.allowed = AllowedRoles::Custom(vec!["not-configured".into()]);
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        LaunchRole::Unavailable(NO_DEFAULT_ROLE.into())
    );
}

#[test]
fn the_configured_rule_ignores_the_last_role() {
    let world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    ws.roles.last = Some("reviewer".into());
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        LaunchRole::Unavailable(NO_DEFAULT_ROLE.into())
    );
}

#[test]
fn the_configured_rule_for_a_directory_without_a_workspace() {
    let mut world = world(LaunchRoleRule::Configured);
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(None)),
        LaunchRole::Unavailable(NO_SAVED_WORKSPACE.into())
    );
    world.roles.truncate(1);
    let only = world.roles[0].name.clone();
    assert_eq!(
        world.launch_role(LaunchSource::CurrentDir(None)),
        role(&only)
    );
}

#[test]
fn the_picker_disables_every_row_while_no_role_resolves() {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = None;
    world.workspaces[0].roles.last = None;
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter");
    let screen = screen(&mut app);
    assert!(
        screen.contains("payments-platform › no default role"),
        "{screen}"
    );
    let narrow = screen_at(&mut app, 80, 24);
    assert!(
        narrow.contains("no default role · set it in Roles"),
        "{narrow}"
    );
    let rows = screen.matches(NO_DEFAULT_ROLE).count();
    assert!(rows >= 4, "{rows} rows\n{screen}");
    assert_eq!(screen.matches("unavailable").count(), rows);
    press(&mut app, "enter");
    assert_eq!(app.route(), jackin_console::app::Route::Manager);
}

#[test]
fn the_picker_keeps_its_rows_while_a_role_resolves() {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = Some("the-architect".into());
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter");
    let screen = screen(&mut app);
    assert!(
        screen.contains("payments-platform › the-architect"),
        "{screen}"
    );
    assert!(!screen.contains(NO_DEFAULT_ROLE) && !screen.contains("unavailable"));
}

#[test]
fn the_configured_rule_refuses_a_default_role_that_cannot_launch() {
    let mut world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let name = world.roles[0].name.clone();
    ws.roles.default = Some(name.clone());
    let unusable = LaunchRole::Unavailable(DEFAULT_ROLE_UNUSABLE.into());
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), role(&name));

    world.roles[0].trusted = false;
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unusable);
    world.roles[0].trusted = true;

    world.roles[0].in_registry = false;
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unusable);
    world.roles[0].in_registry = true;

    world.roles[0].load_error = Some("manifest does not parse".into());
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unusable);

    world.roles[0].load_error = None;
    ws.roles.default = Some("data-eng".into());
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unusable);

    ws.roles.default = Some("not-registered".into());
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), unusable);
}

#[test]
fn the_configured_rule_skips_a_role_that_cannot_launch_as_the_only_one() {
    let mut world = world(LaunchRoleRule::Configured);
    let mut ws = workspace(&world);
    let only = names(&world, 3)[1].clone();
    ws.roles.allowed = AllowedRoles::Custom(vec![only.clone()]);
    assert_eq!(world.launch_role(LaunchSource::Row(Some(&ws))), role(&only));
    world
        .roles
        .iter_mut()
        .find(|r| r.name == only)
        .expect("role")
        .trusted = false;
    assert_eq!(
        world.launch_role(LaunchSource::Row(Some(&ws))),
        LaunchRole::Unavailable(NO_DEFAULT_ROLE.into())
    );
}

fn snapshot_after(app: &mut App, change: impl FnOnce(&mut Snapshot)) -> Msg {
    let mut snapshot = sim::snapshot(&app.world);
    change(&mut snapshot);
    roster_reply(app, snapshot)
}

fn open_picker() -> App {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = Some("the-architect".into());
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter");
    let shown = screen(&mut app);
    assert!(
        shown.contains("payments-platform › the-architect"),
        "{shown}"
    );
    app
}

#[test]
fn a_launch_picked_after_the_role_became_unusable_is_refused() {
    let mut app = open_picker();
    let reload = snapshot_after(&mut app, |s| {
        s.roles
            .iter_mut()
            .find(|r| r.name == "the-architect")
            .expect("role")
            .trusted = false;
    });
    app.deliver(&reload);
    press(&mut app, "space enter");

    assert_eq!(app.route(), Route::Manager);
    let shown = app.shell.status.text().unwrap_or_default().to_owned();
    assert!(shown.contains("launch options changed"), "{shown}");
}

#[test]
fn a_launch_picked_after_its_account_became_unusable_is_refused() {
    let mut app = open_picker();
    let reload = snapshot_after(&mut app, |s| {
        let preselected = s
            .accounts
            .accounts
            .iter_mut()
            .find(|a| a.id == "acct-claude-personal")
            .expect("account");
        preselected.lifecycle = Lifecycle::NeedsLogin;
    });
    app.deliver(&reload);
    press(&mut app, "space enter");

    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn an_account_invalidated_while_its_picker_is_open_is_refused() {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = Some("the-architect".into());
    for account in &mut world.accounts.accounts {
        account.default_for_provider = false;
    }
    world.workspaces[0]
        .accounts
        .enabled
        .insert("acct-claude-work".into());
    world.workspaces[0]
        .accounts
        .enabled
        .insert("acct-claude-personal".into());
    world.workspaces[0].accounts.role_preferred.clear();
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter space enter");
    assert!(screen(&mut app).contains("Account for Claude Code"));
    let launches_before = app.world.instances.len();
    let reload = snapshot_after(&mut app, |snapshot| {
        snapshot
            .accounts
            .accounts
            .iter_mut()
            .find(|account| account.id == "acct-claude-work")
            .expect("work account")
            .lifecycle = Lifecycle::NeedsLogin;
    });
    app.deliver(&reload);
    press(&mut app, "down enter");

    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances.len(), launches_before);
    assert!(
        app.shell
            .status
            .text()
            .unwrap_or_default()
            .contains("launch options changed")
    );
}

#[test]
fn cancelling_an_account_picker_leaves_no_launch_or_pending_choice() {
    let mut world = world(LaunchRoleRule::Configured);
    world.workspaces[0].roles.default = Some("the-architect".into());
    for account in &mut world.accounts.accounts {
        account.default_for_provider = false;
    }
    world.workspaces[0]
        .accounts
        .enabled
        .insert("acct-claude-work".into());
    world.workspaces[0]
        .accounts
        .enabled
        .insert("acct-claude-personal".into());
    world.workspaces[0].accounts.role_preferred.clear();
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down enter space enter");
    assert!(screen(&mut app).contains("Account for Claude Code"));
    let launches_before = app.world.instances.len();
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances.len(), launches_before);
    let shown = screen(&mut app);
    assert!(shown.contains("Current directory"));
    assert!(!shown.contains("Account for Claude Code"), "{shown}");
}

#[test]
fn a_launch_picked_with_nothing_changed_goes_on() {
    let mut app = open_picker();
    press(&mut app, "space enter");
    assert_eq!(app.route(), Route::Cockpit);
}
