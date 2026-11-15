#![cfg(feature = "demo")]

mod common;

use common::{app_with, click, click_text, deliver_roster, press, screen, screen_at};
use jackin_console::app::{App, Route};
use jackin_console::arbiter::DiscoveryError;
use jackin_console::backend::Effect;
use jackin_console::domain::instance::{
    DaemonSnapshot, InstanceEntry, InstanceId, SandboxPhase, UnavailableInstance,
};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::screens::Go;
use jackin_console::world::{Absorbed, Msg, SaveSuccess, SaveWarning, Snapshot, World};
use junie_tui::core::event::Outcome;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn cell_symbols(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| Application::render(app, frame))
        .expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect()
}

fn is_terminal_control(ch: char) -> bool {
    ch.is_control()
        || matches!(
            ch,
            '\u{00ad}'
                | '\u{061c}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
                | '\u{feff}'
        )
}

fn snapshot_of(w: &World) -> Snapshot {
    Snapshot {
        workspaces: w.workspaces.clone(),
        roles: w.roles.clone(),
        instances: w.instances.clone(),
        global: w.global.clone(),
        accounts: w.accounts.clone(),
        surfaces: w.surfaces.clone(),
        github: w.github.clone(),
        index_error: None,
        daemon_health: w.daemon_health,
    }
}

#[test]
fn apply_replaces_every_domain_collection() {
    let mut source = world_for(Scenario::RoleEditor);
    let mut target = world_for(Scenario::FirstUse);
    assert!(target.workspaces.is_empty() && target.surfaces.roles.is_none());
    assert!(target.roles.len() > 1 && target.github.len() > 1);
    source.roles.truncate(1);
    source.github.truncate(1);
    target.apply(snapshot_of(&source));
    assert_eq!(target.workspaces, source.workspaces);
    assert_eq!(target.roles, source.roles);
    assert_eq!(target.instances.len(), source.instances.len());
    assert_eq!(target.global, source.global);
    assert_eq!(target.accounts, source.accounts);
    assert_eq!(target.github, source.github);
    assert_eq!(
        format!("{:?}", target.surfaces),
        format!("{:?}", source.surfaces)
    );
}

#[test]
fn apply_leaves_the_console_state_alone() {
    let source = world_for(Scenario::Returning);
    let mut target = world_for(Scenario::CapsuleMulti);
    let daemons: Vec<_> = target.daemons.keys().cloned().collect();
    assert!(!daemons.is_empty());
    let (home, cwd, fs, now_ms) = (
        target.home.clone(),
        target.cwd.clone(),
        target.fs.clone(),
        target.clock.now_ms,
    );
    target.apply(snapshot_of(&source));
    assert_eq!(target.daemons.keys().cloned().collect::<Vec<_>>(), daemons);
    assert_eq!((target.home, target.cwd), (home, cwd));
    assert_eq!((target.fs, target.clock.now_ms), (fs, now_ms));
}

#[test]
fn apply_shows_the_console_tabs_on_the_new_instances() {
    let mut target = world_for(Scenario::CapsuleMulti);
    let mut snapshot = snapshot_of(&target);
    for instance in snapshot
        .instances
        .iter_mut()
        .filter_map(InstanceEntry::as_available_mut)
    {
        instance.daemon = DaemonSnapshot::Unavailable;
    }
    let id = target.daemons.keys().next().cloned().expect("a daemon");
    target.apply(snapshot);
    let shown = &target.instance(&id).expect("instance").daemon;
    assert!(matches!(shown, DaemonSnapshot::Tabs(_)), "{shown:?}");
}

#[test]
fn refresh_replaces_verified_and_unavailable_entries_without_stale_metadata() {
    let mut world = world_for(Scenario::Returning);
    let mut verified = world.instances[0]
        .as_available()
        .expect("returning scenario has verified instances")
        .clone();
    let id = verified.id.clone();
    let workspace = verified.workspace;
    let workspace_name = workspace
        .and_then(|id| world.workspace(id))
        .map(|workspace| workspace.name.clone());
    world.instances = vec![InstanceEntry::available(verified.clone())];
    world.daemons.clear();
    world.sync_arbiter();
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "* down right");
    let verified_screen = screen(&mut app);
    assert!(
        verified_screen.contains(&id.short_suffix()),
        "{verified_screen}"
    );

    let unavailable = UnavailableInstance::new(
        id.clone(),
        workspace,
        workspace_name,
        None,
        SandboxPhase::Ready,
        Some(verified.created_secs),
        "lead metadata could not be verified",
    );
    let mut unavailable_snapshot = snapshot_of(&app.world);
    unavailable_snapshot.instances[0] = InstanceEntry::Unavailable(unavailable);
    assert_eq!(
        deliver_roster(&mut app, unavailable_snapshot),
        Outcome::Changed
    );
    assert_eq!(app.world.instances.len(), 1);
    assert!(app.world.instance(&id).is_none());
    assert_eq!(app.world.running_count(), 1);
    assert_eq!(app.world.running_count_of(workspace), 1);
    assert_eq!(app.world.arbiter.running(), Ok(1));
    let unavailable_screen = screen(&mut app);
    assert!(
        unavailable_screen.contains(&id.sandbox_name),
        "{unavailable_screen}"
    );
    assert!(
        unavailable_screen.contains("lead metadata could not be verified"),
        "{unavailable_screen}"
    );
    assert!(
        unavailable_screen.contains("Agent metadata"),
        "{unavailable_screen}"
    );
    assert!(
        !unavailable_screen.contains("Run id"),
        "{unavailable_screen}"
    );

    verified.role = Some("freshly-verified-role".into());
    let mut verified_snapshot = snapshot_of(&app.world);
    verified_snapshot.instances[0] = InstanceEntry::available(verified.clone());
    assert_eq!(
        deliver_roster(&mut app, verified_snapshot),
        Outcome::Changed
    );
    assert_eq!(app.world.instances.len(), 1);
    assert_eq!(
        app.world.instance(&id).and_then(|i| i.role.as_deref()),
        Some("freshly-verified-role")
    );
    assert_eq!(app.world.running_count(), 1);
    assert_eq!(app.world.arbiter.running(), Ok(1));
    let verified_screen = screen(&mut app);
    assert!(
        verified_screen.contains("freshly-verified-role"),
        "{verified_screen}"
    );
    assert!(
        !verified_screen.contains("lead metadata could not be verified"),
        "{verified_screen}"
    );
}

#[test]
fn refresh_to_unavailable_keeps_the_attached_capsule_and_reports_the_degradation() {
    let mut world = world_for(Scenario::CapsuleMulti);
    let id = InstanceId::simulated("jk-7f3a");
    let verified = world
        .instance(&id)
        .expect("capsule fixture instance")
        .clone();
    let workspace = verified.workspace;
    let workspace_name = workspace
        .and_then(|workspace| world.workspace(workspace))
        .map(|workspace| workspace.name.clone());
    world.instances = vec![InstanceEntry::available(verified.clone())];
    world.daemons.retain(|instance, _| instance == &id);
    let mut app = app_with(Scenario::CapsuleMulti, world);
    assert_eq!(app.route(), Route::Capsule);

    let unavailable = UnavailableInstance::new(
        id.clone(),
        workspace,
        workspace_name,
        Some("the-architect".into()),
        SandboxPhase::Ready,
        Some(verified.created_secs),
        "required agent metadata is malformed",
    );
    let mut unavailable_snapshot = snapshot_of(&app.world);
    unavailable_snapshot.instances = vec![InstanceEntry::Unavailable(unavailable)];
    assert_eq!(
        deliver_roster(&mut app, unavailable_snapshot),
        Outcome::Changed
    );
    assert_eq!(app.route(), Route::Capsule);
    assert!(
        app.world.daemons.contains_key(&id),
        "a sandbox that is still ready keeps its daemon when the metadata degrades"
    );
    assert_eq!(app.world.instances.len(), 1);
    assert!(app.world.instance(&id).is_none());
    assert_eq!(app.world.running_count(), 1);
    let Some(InstanceEntry::Unavailable(row)) = app.world.entry(&id) else {
        panic!("the observed sandbox remains unavailable and visible");
    };
    assert_eq!(row.phase, SandboxPhase::Ready);
    let status = app.shell.status.text().unwrap_or_default();
    assert!(status.contains("metadata unavailable"), "{status}");
    assert!(
        status.contains("required agent metadata is malformed"),
        "{status}"
    );
    assert!(!status.contains("removed"), "{status}");

    let mut restored_snapshot = snapshot_of(&app.world);
    restored_snapshot.instances = vec![InstanceEntry::available(verified)];
    assert_eq!(
        deliver_roster(&mut app, restored_snapshot),
        Outcome::Changed
    );
    app.go(Go::Attach {
        instance: id.clone(),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);

    let mut absent_snapshot = snapshot_of(&app.world);
    absent_snapshot.instances.clear();
    assert_eq!(deliver_roster(&mut app, absent_snapshot), Outcome::Changed);
    assert_ne!(app.route(), Route::Capsule);
    let status = app.shell.status.text().unwrap_or_default();
    assert!(status.contains("removed elsewhere"), "{status}");
}

#[test]
fn admission_replaces_a_pair_key_without_reordering_rows() {
    let mut world = world_for(Scenario::Returning);
    let first = world.instances[0]
        .as_available()
        .expect("returning scenario has verified instances")
        .clone();
    let second = world.instances[1]
        .as_available()
        .expect("returning scenario has a second verified instance")
        .clone();
    assert_ne!(first.id, second.id);
    let first_id = first.id.clone();

    world.instances = vec![
        InstanceEntry::available(first.clone()),
        InstanceEntry::available(second.clone()),
    ];
    let workspace_name = first
        .workspace
        .and_then(|id| world.workspace(id))
        .map(|workspace| workspace.name.clone());
    let unavailable = UnavailableInstance::new(
        first.id.clone(),
        first.workspace,
        workspace_name,
        None,
        SandboxPhase::Ready,
        Some(first.created_secs),
        "lead metadata could not be verified",
    );
    world.admit_entry(InstanceEntry::Unavailable(unavailable));

    assert_eq!(world.instances.len(), 2);
    assert_eq!(world.instances[0].id(), &first.id);
    assert_eq!(world.instances[1].id(), &second.id);
    assert!(matches!(world.instances[0], InstanceEntry::Unavailable(_)));

    world.admit_entry(InstanceEntry::available(first));
    assert_eq!(world.instances.len(), 2);
    assert_eq!(world.instances[0].id(), &first_id);
    assert_eq!(world.instances[1].id(), &second.id);
    assert!(world.instances[0].as_available().is_some());
}

#[test]
fn unavailable_context_is_bounded_and_safe_without_changing_its_locator() {
    let oversized = "界".repeat(1_500);
    let workspace_text = format!("billing\nplatform\u{001b}[31m\u{2066}{oversized}");
    let role_text = format!("developer\u{0007}\u{0085}\u{202e}{oversized}");
    let reason_text =
        format!("metadata error\n\t\u{0085}\u{202e}sk-REDACTED-TEST-FIXTURE{oversized}");
    let first_name = "sandbox-main-8f3a";
    let first_gateway = "gateway-west";
    let mut world = world_for(Scenario::Returning);
    world.instances.clear();
    world.daemons.clear();
    let first = UnavailableInstance::new(
        InstanceId::new(first_gateway, first_name),
        Some(1),
        Some(workspace_text.clone()),
        Some(role_text.clone()),
        SandboxPhase::Ready,
        None,
        reason_text.clone(),
    );
    let first_id = first.id.clone();
    let second = UnavailableInstance::new(
        InstanceId::new("gateway-east", "sandbox-control-8f3a"),
        Some(1),
        Some("payments-platform".into()),
        Some("reviewer".into()),
        SandboxPhase::Ready,
        Some(1_788_401_000),
        "agent metadata is unavailable",
    );
    let second_id = second.id.clone();
    let third = UnavailableInstance::new(
        InstanceId::new("gateway-east", first_name),
        Some(1),
        Some("payments-platform".into()),
        Some("reviewer".into()),
        SandboxPhase::Ready,
        Some(1_788_401_000),
        "agent metadata is unavailable",
    );
    let third_id = third.id.clone();
    world.admit_entry(InstanceEntry::Unavailable(second));
    world.admit_entry(InstanceEntry::Unavailable(third));
    world.admit_entry(InstanceEntry::Unavailable(first));
    let mut app = app_with_world(
        Scenario::Returning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "*");
    let rows = screen(&mut app)
        .lines()
        .enumerate()
        .filter_map(|(y, line)| {
            let x = line.chars().position(|ch| ch == '?')?;
            Some((x as u16, y as u16))
        })
        .collect::<Vec<_>>();
    for (x, y) in rows {
        click(&mut app, x, y);
        let selected = screen(&mut app);
        if selected.contains(first_name) && selected.contains(first_gateway) {
            break;
        }
    }
    let selected = screen(&mut app);
    assert!(selected.contains(first_name), "{selected}");
    assert!(selected.contains(first_gateway), "{selected}");

    for (width, height) in [(140, 40), (80, 24)] {
        let shown = screen_at(&mut app, width, height);
        let cells = cell_symbols(&mut app, width, height);
        assert_eq!(cells.len(), usize::from(width) * usize::from(height));
        assert!(
            cells
                .iter()
                .flat_map(|cell| cell.chars())
                .all(|ch| !is_terminal_control(ch)),
            "rendered cells contain terminal controls at {width}x{height}"
        );
        assert!(shown.contains(first_name), "{shown}");
        assert!(shown.contains("gateway-west"), "{shown}");
        assert!(shown.contains("ready"), "{shown}");
        assert!(!shown.contains("sk-REDACTED-TEST-FIXTURE"), "{shown}");
    }

    let Some(InstanceEntry::Unavailable(source)) = app.world.entry(&first_id) else {
        panic!("the raw source entry remains in the world");
    };
    assert_eq!(source.id.gateway, first_gateway);
    assert_eq!(source.id.sandbox_name, first_name);
    assert_eq!(source.id.gateway, first_gateway);
    assert_eq!(source.id.sandbox_name, first_name);
    assert_eq!(
        source.workspace_name.as_deref(),
        Some(workspace_text.as_str())
    );
    assert_eq!(source.role.as_deref(), Some(role_text.as_str()));
    assert_eq!(source.reason, reason_text);
    assert_eq!(first_id.row_key(), source.id.row_key());
    let display = source.display();
    assert!(junie_tui::ui::text::width(&display.sandbox_name) <= 64);
    assert!(junie_tui::ui::text::width(&display.gateway) <= 40);
    assert!(junie_tui::ui::text::width(&display.workspace_name) <= 48);
    assert!(junie_tui::ui::text::width(&display.role) <= 40);
    assert!(junie_tui::ui::text::width(&display.reason) <= 112);

    press(&mut app, "i");
    let inspected = screen(&mut app);
    assert!(
        inspected.contains("agent metadata could not be verified"),
        "{inspected}"
    );
    assert!(
        inspected.contains("creation time unavailable"),
        "{inspected}"
    );
    assert!(inspected.contains("developer"), "{inspected}");
    assert!(inspected.contains("billing"), "{inspected}");
    assert!(
        !inspected.contains("sk-REDACTED-TEST-FIXTURE"),
        "{inspected}"
    );
    assert!(!inspected.contains("FailedSetup"), "{inspected}");
    assert!(!inspected.contains("Run id"), "{inspected}");
    for (width, height) in [(140, 40), (80, 24)] {
        let inspect_cells = cell_symbols(&mut app, width, height);
        assert_eq!(
            inspect_cells.len(),
            usize::from(width) * usize::from(height)
        );
        assert!(
            inspect_cells
                .iter()
                .flat_map(|cell| cell.chars())
                .all(|ch| !is_terminal_control(ch)),
            "inspect cells contain terminal controls at {width}x{height}"
        );
    }
    press(&mut app, "esc");
    press(&mut app, "t");
    for _ in 0..40 {
        Application::handle(&mut app, junie_tui::core::event::Input::Tick);
    }
    assert!(matches!(
        app.world.entry(&first_id),
        Some(InstanceEntry::Unavailable(row))
            if row.phase == SandboxPhase::Stopped
                && row.id.gateway == first_gateway
                && row.id.sandbox_name == first_name
    ));
    assert!(matches!(
        app.world.entry(&second_id),
        Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
    ));
    assert!(matches!(
        app.world.entry(&third_id),
        Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
    ));
}

#[test]
fn unavailable_empty_reason_renders_a_safe_fallback() {
    let mut world = world_for(Scenario::Returning);
    world.instances.clear();
    world.daemons.clear();
    let unavailable = UnavailableInstance::new(
        InstanceId::new("gateway-empty-reason", "sandbox-empty-reason"),
        Some(1),
        Some("payments-platform".into()),
        Some("the-architect".into()),
        SandboxPhase::Stopped,
        None,
        " \n\t ",
    );
    let id = unavailable.id.clone();
    world.admit_entry(InstanceEntry::Unavailable(unavailable));
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "*");
    click_text(&mut app, "sandbox-empty-reason");
    press(&mut app, "i");
    let inspected = screen(&mut app);
    assert!(
        inspected.contains("agent metadata could not be verified"),
        "{inspected}"
    );
    assert!(
        inspected.contains("creation time unavailable"),
        "{inspected}"
    );
    assert!(matches!(
        app.world.entry(&id),
        Some(InstanceEntry::Unavailable(row)) if row.reason == " \n\t "
    ));
    assert!(
        cell_symbols(&mut app, 80, 24)
            .iter()
            .flat_map(|cell| cell.chars())
            .all(|ch| !is_terminal_control(ch))
    );
}

#[test]
fn apply_counts_the_running_instances_in_the_arbiter() {
    let source = world_for(Scenario::Returning);
    let mut target = world_for(Scenario::FirstUse);
    assert_eq!(target.arbiter.running(), Ok(0));
    target.apply(snapshot_of(&source));
    assert!(source.running_count() > 0);
    assert_eq!(target.arbiter.running(), Ok(source.running_count()));
}

#[test]
fn apply_reports_an_unreadable_index_and_recovers() {
    let source = world_for(Scenario::Returning);
    let mut target = world_for(Scenario::Returning);
    target.apply(Snapshot {
        index_error: Some(DiscoveryError::IndexUnreadable),
        ..snapshot_of(&source)
    });
    assert_eq!(
        target.arbiter.running(),
        Err(DiscoveryError::IndexUnreadable)
    );
    target.apply(snapshot_of(&source));
    assert_eq!(target.arbiter.running(), Ok(source.running_count()));
}

#[test]
fn the_sim_never_hands_out_a_workspace_id_in_use() {
    let mut world = world_for(Scenario::Returning);
    let highest = 500;
    world.workspaces[0].id = highest;
    let fresh = world.workspaces[0].clone();
    let mut ids = vec![];
    for _ in 0..2 {
        world.perform(Effect::SaveWorkspace {
            id: None,
            base: None,
            workspace: Box::new(fresh.clone()),
        });
        world.clock.advance(5_000);
        let msg = world.take_due();
        let Some(Msg::WorkspaceSaved { id, result }) = &msg else {
            panic!("one reply expected, got {msg:?}");
        };
        ids.push(id.expect("an assigned id"));
        world.apply(*result.clone().expect("saved").snapshot);
    }
    assert_eq!(ids, vec![highest + 1, highest + 2]);
}

#[test]
fn a_stale_account_save_does_not_report_uncertain_durability() {
    let mut world = world_for(Scenario::Returning);
    let older = world.next_ticket();
    let newer = world.next_ticket();
    let snapshot = snapshot_of(&world);
    let saved = |warning| SaveSuccess {
        snapshot: Box::new(snapshot.clone()),
        warning,
    };
    assert_eq!(
        world.absorb(&Msg::AccountsSaved {
            ticket: newer,
            added: None,
            result: Ok(saved(None)),
        }),
        Absorbed::Current,
        "the newest account save is the current one"
    );
    assert_eq!(
        world.absorb(&Msg::AccountsSaved {
            ticket: older,
            added: None,
            result: Ok(saved(Some(SaveWarning::DirectorySync))),
        }),
        Absorbed::Stale,
        "the older account save is stale"
    );
    assert_eq!(
        world.committed_save_warning, None,
        "a stale reply must not report uncertain durability for an ignored snapshot"
    );
}
