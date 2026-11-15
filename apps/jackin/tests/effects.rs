#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Remote, Route};
use jackin_console::backend::{Backend, Completion, Effect, FileSource, Refresh};
use jackin_console::domain::account::{
    AccountRegistry, CredentialSource, DetectedKind, IssueCode, ValidationState,
};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{Instance, InstanceEntry, InstanceId, InstanceStatus};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::reason::{IssueReason, RefreshOutcome};
use jackin_console::domain::role::{FindingArea, RoleAdmin, RoleCheckState, RoleFinding};
use jackin_console::domain::usage::{Freshness, FreshnessInfo};
use jackin_console::domain::workspace::RoleSource;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::screens::Go;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{
    DaemonHealth, GlobalConfig, Msg, SaveFailure, SaveSuccess, SaveWarning, Snapshot, World,
};
use junie_tui::core::event::{External, Input, Outcome};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::file_browser::FileEntry;

fn iid(label: &str) -> InstanceId {
    InstanceId::simulated(label)
}

type Answer = fn(&Effect, &World) -> Option<Msg>;

/// A backend that records every effect it receives and answers with its own
/// function, the way a real backend would. Effects it does not answer go to
/// the simulator.
#[derive(Debug)]
struct TestBackend {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
    answer: Answer,
    unanswered: fn(&Effect) -> bool,
    files: Option<fn() -> Box<dyn FileSource>>,
}

impl TestBackend {
    fn new(answer: Answer) -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
            answer,
            unanswered: |_| false,
            files: None,
        })
    }

    /// Every effect received except the manager's periodic refresh.
    fn seen(&self) -> Vec<Effect> {
        let mut all = self.seen.lock().expect("seen").clone();
        all.retain(|e| !matches!(e, Effect::Refresh { .. }));
        all
    }

    fn refresh_tickets(&self) -> Vec<u64> {
        let all = self.seen.lock().expect("seen");
        all.iter()
            .filter_map(|e| match e {
                Effect::Refresh { ticket } => Some(*ticket),
                _ => None,
            })
            .collect()
    }

    fn refreshes(&self) -> usize {
        let all = self.seen.lock().expect("seen");
        all.iter()
            .filter(|e| matches!(e, Effect::Refresh { .. }))
            .count()
    }
}

impl Backend for TestBackend {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        if (self.unanswered)(effect) {
            return None;
        }
        match (self.answer)(effect, world) {
            Some(msg) => Some(Completion::ready(0, msg)),
            None => self.inner.perform(effect, world),
        }
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        match self.files {
            Some(files) => files(),
            None => self.inner.file_source(world),
        }
    }
}

fn backed(scenario: Scenario, answer: Answer) -> (App, Arc<TestBackend>) {
    let backend = TestBackend::new(answer);
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
}

fn timed(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn settle(app: &mut App) {
    for _ in 0..40 {
        Application::handle(app, Input::Tick);
    }
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn save_success(snapshot: Snapshot, warning: Option<SaveWarning>) -> SaveSuccess {
    SaveSuccess {
        snapshot: Box::new(snapshot),
        warning,
    }
}

fn unsynced(reply: Option<Msg>) -> Option<Msg> {
    match reply? {
        Msg::WorkspaceDeleted { id, result } => Some(Msg::WorkspaceDeleted {
            id,
            result: result
                .map(|saved| save_success(*saved.snapshot, Some(SaveWarning::DirectorySync))),
        }),
        Msg::RoleBound {
            role,
            agent,
            result,
        } => Some(Msg::RoleBound {
            role,
            agent,
            result: result
                .map(|saved| save_success(*saved.snapshot, Some(SaveWarning::DirectorySync))),
        }),
        Msg::RoleAdded { location, result } => Some(Msg::RoleAdded {
            location,
            result: result.map(|(name, saved)| {
                (
                    name,
                    save_success(*saved.snapshot, Some(SaveWarning::DirectorySync)),
                )
            }),
        }),
        Msg::RoleRemoved { role, result } => Some(Msg::RoleRemoved {
            role,
            result: result
                .map(|saved| save_success(*saved.snapshot, Some(SaveWarning::DirectorySync))),
        }),
        other => Some(other),
    }
}

const CANONICAL: &str = "/workspace/backend-canonical";

/// Stores the payload the way a backend would: the record keeps the id it was
/// given or 42, and the backend normalises the working directory.
fn store_workspace(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveWorkspace { id, workspace, .. } = effect else {
        return None;
    };
    let mut stored = (**workspace).clone();
    stored.id = id.unwrap_or(42);
    stored.workdir = CANONICAL.into();
    let mut snapshot = sim::snapshot(world);
    match snapshot.workspaces.iter_mut().find(|w| w.id == stored.id) {
        Some(slot) => *slot = stored.clone(),
        None => snapshot.workspaces.push(stored.clone()),
    }
    Some(Msg::WorkspaceSaved {
        id: Some(stored.id),
        result: Ok(save_success(snapshot, None)),
    })
}

fn store_workspace_unsynced(effect: &Effect, world: &World) -> Option<Msg> {
    match store_workspace(effect, world)? {
        Msg::WorkspaceSaved {
            id,
            result: Ok(mut saved),
        } => {
            saved.warning = Some(SaveWarning::DirectorySync);
            Some(Msg::WorkspaceSaved {
                id,
                result: Ok(saved),
            })
        }
        message => Some(message),
    }
}

fn fail_workspace(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::SaveWorkspace { id, .. } = effect else {
        return None;
    };
    Some(Msg::WorkspaceSaved {
        id: *id,
        result: Err(SaveFailure {
            error: "disk full".into(),
            next: None,
        }),
    })
}

#[test]
fn save_workspace_stores_the_payload() {
    let (mut app, backend) = backed(Scenario::Returning, store_workspace);
    let original = app.world.workspaces[0].clone();
    press(&mut app, "down e enter x ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let [
        Effect::SaveWorkspace {
            id,
            base,
            workspace,
        },
    ] = seen.as_slice()
    else {
        panic!("one SaveWorkspace expected, got {seen:?}");
    };
    assert_eq!(*id, Some(original.id));
    assert_eq!(workspace.name, "x");
    assert_eq!(
        base.as_deref().map(|b| b.name.as_str()),
        Some(original.name.as_str())
    );

    assert_eq!(status(&app), "Workspace x saved");
    let roster: Vec<(&str, &str)> = app
        .world
        .workspaces
        .iter()
        .map(|w| (w.name.as_str(), w.workdir.as_str()))
        .collect();
    assert!(roster.contains(&("x", CANONICAL)), "{roster:?}");
    assert!(
        !roster.contains(&("x", original.workdir.as_str())),
        "the roster must be the reply snapshot, not the pending copy: {roster:?}"
    );
}

#[test]
fn committed_unsynced_workspace_save_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::Returning, store_workspace_unsynced);
    press(&mut app, "down e enter x ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(
        status(&app),
        "Saved, durability uncertain · Workspace x saved"
    );
    let roster: Vec<(&str, &str)> = app
        .world
        .workspaces
        .iter()
        .map(|workspace| (workspace.name.as_str(), workspace.workdir.as_str()))
        .collect();
    assert!(roster.contains(&("x", CANONICAL)), "{roster:?}");
    assert!(!screen(&mut app).contains("Save failed"));
}

#[test]
fn failed_save_keeps_the_input() {
    let (mut app, _) = backed(Scenario::Returning, fail_workspace);
    let before = app.world.workspaces.clone();
    press(&mut app, "down e enter x ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("Save failed"), "{shown}");
    assert!(shown.contains("disk full"), "{shown}");
    assert!(
        !shown.contains("EACCES") && !shown.contains("not writable"),
        "the dialog must show the reply text, not the canned one:\n{shown}"
    );
    assert!(shown.contains("your edits are intact"), "{shown}");
    assert_eq!(app.world.workspaces, before);
    press(&mut app, "esc");
    let editor = screen(&mut app);
    assert!(
        editor.contains("Name *") && editor.contains(" x "),
        "{editor}"
    );
}

#[test]
fn new_workspace_id_comes_from_the_backend() {
    let (mut app, backend) = backed(Scenario::Returning, store_workspace);
    press(&mut app, "n space enter enter - t w o enter");
    press(&mut app, "ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let [
        Effect::SaveWorkspace {
            id,
            base,
            workspace,
        },
    ] = seen.as_slice()
    else {
        panic!("one SaveWorkspace expected, got {seen:?}");
    };
    assert_eq!(*id, None, "a new workspace has no id yet");
    assert!(base.is_none(), "a new workspace has no base");
    let created: Vec<_> = app
        .world
        .workspaces
        .iter()
        .filter(|w| w.name == workspace.name)
        .collect();
    assert_eq!(created.len(), 1, "{:?}", app.world.workspaces);
    assert_eq!(created[0].id, 42, "the id is the backend's");
    assert!(
        app.world.workspaces.iter().all(|w| w.id != 100),
        "no screen may allocate an id: {:?}",
        app.world
            .workspaces
            .iter()
            .map(|w| w.id)
            .collect::<Vec<_>>()
    );
}

/// Stores the payload and normalises one flag the editor never changed, so a
/// screen that writes its pending copy is told from one that applies the
/// snapshot.
fn store_global(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveGlobal { global, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.global = (**global).clone();
    snapshot.global.dco_signoff = false;
    Some(Msg::GlobalSaved {
        result: Ok(save_success(snapshot, None)),
    })
}

fn store_global_unsynced(effect: &Effect, world: &World) -> Option<Msg> {
    match store_global(effect, world)? {
        Msg::GlobalSaved {
            result: Ok(mut saved),
        } => {
            saved.warning = Some(SaveWarning::DirectorySync);
            Some(Msg::GlobalSaved { result: Ok(saved) })
        }
        message => Some(message),
    }
}

fn fail_global(effect: &Effect, _: &World) -> Option<Msg> {
    matches!(effect, Effect::SaveGlobal { .. }).then(|| Msg::GlobalSaved {
        result: Err(SaveFailure {
            error: "disk full".into(),
            next: None,
        }),
    })
}

fn only_global(seen: &[Effect]) -> (&GlobalConfig, &GlobalConfig) {
    let [Effect::SaveGlobal { base, global }] = seen else {
        panic!("one SaveGlobal expected, got {seen:?}");
    };
    (base, global)
}

#[test]
fn save_global_stores_the_payload() {
    let (mut app, backend) = backed(Scenario::Returning, store_global);
    assert!(app.world.global.coauthor_trailer && app.world.global.dco_signoff);
    press(&mut app, "s enter space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let (base, global) = only_global(&seen);
    assert!(base.coauthor_trailer && !global.coauthor_trailer);
    assert!(global.dco_signoff, "the editor did not touch the DCO flag");

    assert_eq!(status(&app), "Settings saved");
    assert!(!app.world.global.coauthor_trailer);
    assert!(
        !app.world.global.dco_signoff,
        "the world must be the reply snapshot, not the pending copy"
    );
}

#[test]
fn committed_unsynced_global_save_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::Returning, store_global_unsynced);
    press(&mut app, "s enter space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(status(&app), "Saved, durability uncertain · Settings saved");
    assert!(!app.world.global.dco_signoff);
    assert!(!screen(&mut app).contains("Settings error"));
}

#[test]
fn trust_rows_reach_the_roles_only_through_the_snapshot() {
    let (mut app, backend) = backed(Scenario::Returning, store_global);
    // A source written with a scheme still names the trust row's repository.
    let role = app
        .world
        .roles
        .iter_mut()
        .find(|r| r.name == "data-eng")
        .expect("the untrusted role");
    role.source = RoleSource::Git {
        url: "https://github.com/acme-labs/roles-experimental".into(),
        branch: Some("next".into()),
    };
    let untrusted = |w: &World| {
        w.roles
            .iter()
            .filter(|r| !r.trusted)
            .map(|r| r.name.clone())
            .collect::<Vec<_>>()
    };
    let before = untrusted(&app.world);
    assert_eq!(before, ["data-eng"]);
    press(&mut app, "s ] ] ] ] enter down space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let seen = backend.seen();
    let (base, global) = only_global(&seen);
    assert_eq!(base.change_count(global), 1);
    assert!(global.trust[1].trusted && !base.trust[1].trusted);
    assert_eq!(
        untrusted(&app.world),
        before,
        "a screen must not copy trust rows onto the roles"
    );
}

#[test]
fn failed_global_save_keeps_the_input() {
    let (mut app, _) = backed(Scenario::Returning, fail_global);
    let before = app.world.global.clone();
    press(&mut app, "s enter space ctrl+s");
    press(&mut app, "right enter");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("Settings error"), "{shown}");
    assert!(shown.contains("disk full"), "{shown}");
    assert!(
        !shown.contains("EACCES") && !shown.contains("not writable"),
        "the dialog must show the reply text, not the canned one:\n{shown}"
    );
    assert!(shown.contains("your edits are intact"), "{shown}");
    assert_eq!(app.world.global, before);
}

const NEW_ACCOUNT: &str = "c a enter S p a r e tab tab tab down down tab enter s k - a n t - v a l i d - a b c d e f 1 2 3 4 tab";

fn store_accounts(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveAccounts {
        ticket, registry, ..
    } = effect
    else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.accounts = (**registry).clone();
    for account in &mut snapshot.accounts.accounts {
        if account.display_name == "Spare" {
            account.purpose = Some("kept by the backend".into());
        }
    }
    Some(Msg::AccountsSaved {
        ticket: *ticket,
        added: None,
        result: Ok(save_success(snapshot, None)),
    })
}

fn store_accounts_unsynced(effect: &Effect, world: &World) -> Option<Msg> {
    match store_accounts(effect, world)? {
        Msg::AccountsSaved {
            ticket,
            added,
            result: Ok(mut saved),
        } => {
            saved.warning = Some(SaveWarning::DirectorySync);
            Some(Msg::AccountsSaved {
                ticket,
                added,
                result: Ok(saved),
            })
        }
        message => Some(message),
    }
}

fn fail_accounts(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::SaveAccounts { ticket, .. } = effect else {
        return None;
    };
    Some(Msg::AccountsSaved {
        ticket: *ticket,
        added: None,
        result: Err("disk full".into()),
    })
}

#[test]
fn save_accounts_stores_the_payload() {
    let (mut app, backend) = backed(Scenario::AccountsMixed, store_accounts);
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);

    let seen = backend.seen();
    let [
        Effect::CheckSource { .. },
        Effect::SaveAccounts { base, registry, .. },
    ] = seen.as_slice()
    else {
        panic!("one CheckSource, then one SaveAccounts expected, got {seen:?}");
    };
    let named = |r: &AccountRegistry| r.accounts.iter().any(|a| a.display_name == "Spare");
    assert!(named(registry) && !named(base));

    assert_eq!(status(&app), "Saved Claude · Spare");
    let spare = app
        .world
        .accounts
        .accounts
        .iter()
        .find(|a| a.display_name == "Spare")
        .expect("the saved account");
    assert_eq!(
        spare.purpose.as_deref(),
        Some("kept by the backend"),
        "the registry must be the reply snapshot, not the pending copy"
    );
    assert!(!screen(&mut app).contains("New account"));
}

#[test]
fn committed_unsynced_accounts_save_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::AccountsMixed, store_accounts_unsynced);
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);

    assert_eq!(
        status(&app),
        "Saved, durability uncertain · Saved Claude · Spare"
    );
    let spare = app
        .world
        .accounts
        .accounts
        .iter()
        .find(|account| account.display_name == "Spare")
        .expect("the saved account");
    assert_eq!(spare.purpose.as_deref(), Some("kept by the backend"));
    let shown = screen(&mut app);
    assert!(!shown.contains("New account"), "{shown}");
    assert!(!shown.contains("disk full"), "{shown}");
}

#[test]
fn failed_accounts_save_keeps_the_form() {
    let (mut app, _) = backed(Scenario::AccountsMixed, fail_accounts);
    let before = app.world.accounts.clone();
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("New account"), "{shown}");
    assert!(shown.contains("disk full"), "{shown}");
    assert!(shown.contains("Spare"), "{shown}");
    assert_eq!(app.world.accounts, before);
}

/// Removes the workspace and, unlike the simulator, keeps the instances'
/// link to it, so the roster can only have come from this snapshot.
fn delete_workspace(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::DeleteWorkspace { id } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot.workspaces.retain(|w| w.id != *id);
    Some(Msg::WorkspaceDeleted {
        id: *id,
        result: Ok(save_success(snapshot, None)),
    })
}

fn refuse_delete(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::DeleteWorkspace { id } = effect else {
        return None;
    };
    Some(Msg::WorkspaceDeleted {
        id: *id,
        result: Err("the workspace is open in another session".into()),
    })
}

#[test]
fn delete_workspace_is_an_effect() {
    let (mut app, backend) = backed(Scenario::Returning, delete_workspace);
    let doomed = app.world.workspaces[0].clone();
    let linked: Vec<_> = app
        .world
        .instances
        .iter()
        .filter(|entry| entry.workspace_id() == Some(doomed.id))
        .map(|entry| entry.id().clone())
        .collect();
    assert!(!linked.is_empty());
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(backend.seen(), [Effect::DeleteWorkspace { id: doomed.id }]);
    assert!(app.world.workspaces.iter().all(|w| w.id != doomed.id));
    assert_eq!(
        status(&app),
        "Deleted workspace payments-platform · instances and files kept"
    );
    for id in linked {
        assert_eq!(
            app.world.instance(&id).map(|i| i.workspace),
            Some(Some(doomed.id)),
            "the instances are the reply snapshot's, not the screen's"
        );
    }
}

#[test]
fn committed_unsynced_workspace_delete_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::Returning, |effect, world| {
        unsynced(delete_workspace(effect, world))
    });
    let doomed = app.world.workspaces[0].clone();
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);

    assert!(
        app.world
            .workspaces
            .iter()
            .all(|workspace| workspace.id != doomed.id)
    );
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(
        status(&app),
        "Saved, durability uncertain · Deleted workspace payments-platform · instances and files kept"
    );
    assert!(!screen(&mut app).contains("Nothing changed"));
}

#[test]
fn refused_delete_leaves_the_row() {
    let (mut app, _) = backed(Scenario::Returning, refuse_delete);
    let ids = |w: &World| w.workspaces.iter().map(|w| w.id).collect::<Vec<_>>();
    let before = ids(&app.world);
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(ids(&app.world), before);
    assert_eq!(status(&app), "the workspace is open in another session");
    assert!(screen(&mut app).contains("payments-platform"));
}

const RESTORED_AT: i64 = 12_345;

fn restore(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::Restore { instance } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    for i in snapshot
        .instances
        .iter_mut()
        .filter_map(InstanceEntry::as_available_mut)
        .filter(|i| i.id == *instance)
    {
        i.status = InstanceStatus::Running;
        i.last_seen_secs = RESTORED_AT;
    }
    Some(Msg::Restored {
        instance: instance.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_restore(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::Restore { instance } = effect else {
        return None;
    };
    Some(Msg::Restored {
        instance: instance.clone(),
        result: Err("the sandbox was removed".into()),
    })
}

#[test]
fn restore_is_an_effect() {
    let (mut app, backend) = backed(Scenario::Returning, restore);
    press(&mut app, "down right down down enter");

    assert_eq!(
        backend.seen(),
        [Effect::Restore {
            instance: iid("jk-c41e")
        }]
    );
    let restored = app.world.instance(&iid("jk-c41e")).expect("the instance");
    assert_eq!(restored.status, InstanceStatus::Running);
    assert_eq!(
        restored.last_seen_secs, RESTORED_AT,
        "the instance is the reply snapshot's, not the screen's"
    );
    assert_eq!(app.route(), Route::Capsule);
    assert!(app.world.daemons.contains_key(&iid("jk-c41e")));
}

#[test]
fn refused_restore_stays_on_the_row() {
    let (mut app, _) = backed(Scenario::Returning, refuse_restore);
    press(&mut app, "down right down down enter");
    settle(&mut app);

    let kept = app.world.instance(&iid("jk-c41e")).expect("the instance");
    assert_eq!(kept.status, InstanceStatus::PreservedDirty);
    assert_eq!(app.route(), Route::Manager);
    assert!(!app.world.daemons.contains_key(&iid("jk-c41e")));
    assert_eq!(status(&app), "the sandbox was removed");
}

const LOADED: &str = "loaded by the backend";

fn load_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::LoadRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    for r in snapshot.roles.iter_mut().filter(|r| r.name == *role) {
        r.in_registry = true;
        r.trusted = true;
        r.load_error = None;
        r.description = LOADED.into();
    }
    Some(Msg::RoleLoaded {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_role_load(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::LoadRole { role } = effect else {
        return None;
    };
    Some(Msg::RoleLoaded {
        role: role.clone(),
        result: Err("manifest not found".into()),
    })
}

fn role<'a>(w: &'a World, name: &str) -> &'a jackin_console::domain::workspace::RoleEntry {
    w.roles.iter().find(|r| r.name == name).expect("the role")
}

#[test]
fn role_load_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::Returning, load_role);
    assert!(!role(&app.world, "writer").in_registry);
    press(&mut app, "down e ] ] down a w r i t e r enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::LoadRole {
            role: "writer".into()
        }]
    );
    assert_eq!(
        role(&app.world, "writer").description,
        LOADED,
        "the role is the reply snapshot's, not the screen's"
    );
    assert!(role(&app.world, "writer").in_registry);
    assert_eq!(status(&app), "Role writer loaded and allowed");
}

#[test]
fn failed_role_load_names_the_reply_error() {
    let (mut app, _) = backed(Scenario::Returning, fail_role_load);
    press(&mut app, "down e ] ] down a w r i t e r enter");
    settle(&mut app);

    let shown = screen(&mut app);
    assert!(shown.contains("Role writer failed to load"), "{shown}");
    assert!(shown.contains("manifest not found"), "{shown}");
    assert!(!role(&app.world, "writer").in_registry);
    assert_eq!(
        role(&app.world, "writer").load_error.as_deref(),
        Some("manifest not found"),
        "the world keeps the error of a failed load"
    );
}

/// Trusts the role in its own way: the role, and nothing in the global trust
/// rows. A screen that finds a trust row itself would flip that row too.
fn trust_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::TrustRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    for r in snapshot.roles.iter_mut().filter(|r| r.name == *role) {
        r.trusted = true;
    }
    Some(Msg::RoleTrusted {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn refuse_trust(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::TrustRole { role } = effect else {
        return None;
    };
    Some(Msg::RoleTrusted {
        role: role.clone(),
        result: Err("the trust store is read-only".into()),
    })
}

const TRUST_DIALOG: &str = "down e ] ] down down down down down enter";

#[test]
fn trust_dialog_sends_trust_role() {
    let (mut app, backend) = backed(Scenario::Returning, trust_role);
    let rows = app.world.global.trust.clone();
    press(&mut app, TRUST_DIALOG);
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [
            Effect::TrustRole {
                role: "data-eng".into()
            },
            Effect::LoadRole {
                role: "data-eng".into()
            },
        ]
    );
    assert!(role(&app.world, "data-eng").trusted);
    assert_eq!(
        app.world.global.trust, rows,
        "the screen must not find a trust row by the name of a source"
    );
}

#[test]
fn refused_trust_leaves_the_role_untrusted() {
    let (mut app, backend) = backed(Scenario::Returning, refuse_trust);
    press(&mut app, TRUST_DIALOG);
    press(&mut app, "right enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::TrustRole {
            role: "data-eng".into()
        }],
        "an untrusted role is not loaded"
    );
    assert!(!role(&app.world, "data-eng").trusted);
    assert_eq!(status(&app), "the trust store is read-only");
}

const MOUNT_FORM: &str = "down e ] enter a";

#[test]
fn mount_form_keeps_its_error() {
    let mut app = app(Scenario::Returning);
    press(&mut app, MOUNT_FORM);
    press(&mut app, "tab tab r e l a t i v e enter");
    click_text(&mut app, "Save   │");

    let shown = screen(&mut app);
    assert!(shown.contains("New mount"), "the form closed:\n{shown}");
    assert!(
        shown.contains("Destination must be an absolute path"),
        "{shown}"
    );
    assert!(shown.contains("relative"), "the input was lost:\n{shown}");
    assert!(
        !shown.contains("/workspace/relative"),
        "nothing is added on an error:\n{shown}"
    );
}

#[test]
fn mount_form_saves_a_valid_mount_once() {
    let mut app = app(Scenario::Returning);
    press(&mut app, MOUNT_FORM);
    press(
        &mut app,
        "down tab g i t h u b . c o m / o / r tab / w o r k s p a c e / o k",
    );
    click_text(&mut app, "Save   │");

    let shown = screen(&mut app);
    assert!(!shown.contains("New mount"), "{shown}");
    let rows = shown
        .lines()
        .filter(|l| l.contains("/workspace/ok") && l.contains("github.com/o/r"))
        .count();
    assert_eq!(rows, 1, "{shown}");
    assert!(
        shown.contains("Mounts •") && shown.contains("1 change"),
        "{shown}"
    );
    assert!(
        shown.contains("Workspaces › payments-platform › edit"),
        "{shown}"
    );
}

const PERSONAL: &str = "acct-claude-personal";

fn first_window_pct(w: &World, id: &str) -> Option<u8> {
    w.accounts.get(id)?.usage.windows.first()?.used_pct
}

/// Refreshes the account the way a backend that read the provider would:
/// the first window reads 40% used, whatever it read before.
fn refresh_account(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::RefreshAccount {
        ticket,
        account,
        source,
        ..
    } = effect
    else {
        return None;
    };
    let now = world.now_secs();
    let mut usage = world.accounts.get(account)?.usage.clone();
    usage.freshness = FreshnessInfo::current(now);
    usage.windows[0].used_pct = Some(40);
    Some(Msg::AccountRefreshed {
        ticket: *ticket,
        account: account.clone(),
        source: Box::new(source.clone()),
        result: Ok(Box::new(Refresh {
            usage,
            issue: None,
            last_refresh_secs: now,
            outcome: RefreshOutcome::NeedsAttention,
        })),
    })
}

fn refresh_personal(app: &mut App) {
    press(app, "c down down r");
    for _ in 0..40 {
        Application::handle(app, Input::Tick);
    }
}

#[test]
fn refresh_result_comes_from_the_backend() {
    let (mut app, backend) = backed(Scenario::AccountsMixed, refresh_account);
    refresh_personal(&mut app);

    assert!(matches!(
        backend.seen().as_slice(),
        [Effect::RefreshAccount { account, .. }] if account == PERSONAL
    ));
    assert_eq!(first_window_pct(&app.world, PERSONAL), Some(40));
    let status = status(&app);
    assert!(
        status.starts_with("Refreshed Claude · Personal · needs attention · "),
        "{status}"
    );
}

#[test]
fn the_sim_refreshes_as_it_did() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    assert_eq!(first_window_pct(&app.world, PERSONAL), Some(38));
    refresh_personal(&mut app);

    assert_eq!(first_window_pct(&app.world, PERSONAL), Some(39));
    assert_eq!(
        status(&app),
        "Refreshed Claude · Personal · current · 4 windows"
    );
}

#[test]
fn hard_cases_refresh_keeps_its_per_account_outcomes() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    press(&mut app, "u r");
    settle(&mut app);

    let work = app
        .world
        .accounts
        .get("acct-claude-work")
        .expect("work account");
    assert_eq!(work.usage.freshness.phase, Freshness::Current);
    assert_eq!(work.usage.windows[0].used_pct, Some(77));
    assert_eq!(work.issue, None);

    let go = app
        .world
        .accounts
        .get("acct-opencode-go")
        .expect("go account");
    assert_eq!(go.usage.freshness.phase, Freshness::Failed);
    assert_eq!(
        go.issue.as_ref().map(|issue| issue.code),
        Some(IssueCode::RateLimited)
    );

    let minimax = app
        .world
        .accounts
        .get("disc-minimax")
        .expect("minimax account");
    assert_eq!(minimax.usage.freshness.phase, Freshness::Failed);
    assert_eq!(
        minimax.issue.as_ref().map(|issue| issue.code),
        Some(IssueCode::ProviderUnavailable)
    );
}

#[test]
fn refreshing_usage_updates_the_stale_work_account() {
    const WORK: &str = "acct-claude-work";
    let (mut app, backend) = backed(Scenario::AccountsMixed, unchanged_roster);
    let original_source = app
        .world
        .accounts
        .get(WORK)
        .expect("work account")
        .source
        .clone();

    press(&mut app, "u r");
    settle(&mut app);

    let work_effects: Vec<_> = backend
        .seen()
        .into_iter()
        .filter_map(|effect| match effect {
            Effect::RefreshAccount {
                account, source, ..
            } if account == WORK => Some((source, account)),
            _ => None,
        })
        .collect();
    assert_eq!(work_effects.len(), 1, "{work_effects:?}");
    assert_eq!(work_effects[0].0, original_source);

    let work = app.world.accounts.get(WORK).expect("work account");
    assert_eq!(work.usage.freshness.phase, Freshness::Current);
    assert_eq!(work.issue, None);
}

#[test]
fn a_validation_reply_for_a_replaced_source_does_not_touch_the_edited_account() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    let old = app
        .world
        .accounts
        .get(PERSONAL)
        .expect("account")
        .source
        .clone();
    press(&mut app, "c down down v");
    assert!(matches!(
        app.world
            .accounts
            .get(PERSONAL)
            .expect("account")
            .validation,
        ValidationState::Validating { .. }
    ));

    let mut snapshot = sim::snapshot(&app.world);
    let edited = snapshot.accounts.get_mut(PERSONAL).expect("account");
    edited.source = CredentialSource::HostEnv {
        var: "REPLACEMENT_KEY".into(),
        detected: DetectedKind::ClaudeApiKeyEnv,
    };
    edited.validation = ValidationState::NeverValidated;
    let edited = edited.clone();
    Application::handle(
        &mut app,
        Input::External(External::new(Remote::Reloaded(Box::new(snapshot)))),
    );
    settle(&mut app);

    let kept = app.world.accounts.get(PERSONAL).expect("account");
    assert_ne!(kept.source, old);
    assert_eq!(kept.validation, ValidationState::NeverValidated);
    assert_eq!(*kept, edited);
    let shown = status(&app);
    assert!(!shown.contains(&kept.title()), "{shown}");
    assert_eq!(shown, "Validation discarded · the account changed");
}

#[test]
fn a_refresh_reply_for_a_replaced_source_does_not_touch_the_edited_account() {
    let mut app = timed(Scenario::AccountsMixed, world_for(Scenario::AccountsMixed));
    let old = app
        .world
        .accounts
        .get(PERSONAL)
        .expect("account")
        .source
        .clone();
    press(&mut app, "c down down r");
    assert_eq!(
        app.world
            .accounts
            .get(PERSONAL)
            .expect("account")
            .usage
            .freshness
            .phase,
        Freshness::Refreshing
    );

    let mut snapshot = sim::snapshot(&app.world);
    let edited = snapshot.accounts.get_mut(PERSONAL).expect("account");
    edited.source = CredentialSource::HostEnv {
        var: "REPLACEMENT_KEY".into(),
        detected: DetectedKind::ClaudeApiKeyEnv,
    };
    let edited = edited.clone();
    Application::handle(
        &mut app,
        Input::External(External::new(Remote::Reloaded(Box::new(snapshot)))),
    );
    settle(&mut app);

    let kept = app.world.accounts.get(PERSONAL).expect("account");
    assert_ne!(kept.source, old);
    assert_eq!(*kept, edited);
    let shown = status(&app);
    assert!(!shown.starts_with("Refreshed"), "{shown}");
}

fn failure_app() -> App {
    let mut app = timed(Scenario::Returning, world_for(Scenario::Returning));
    screen(&mut app);
    app
}

#[test]
fn a_failed_stop_shows_the_reply_error() {
    let mut app = failure_app();
    app.deliver(&Msg::Stopped {
        instance: iid("jk-7f3a"),
        result: Err("docker is not answering".into()),
    });
    assert_eq!(status(&app), "docker is not answering");
    let kept = app.world.instance(&iid("jk-7f3a")).expect("the instance");
    assert_eq!(kept.status, InstanceStatus::Running);
    assert!(app.world.daemons.contains_key(&iid("jk-7f3a")));
}

#[test]
fn a_refused_purge_reports_its_reason() {
    let mut app = failure_app();
    let reason = "purge refused: 2 unpushed commits would be lost";
    app.deliver(&Msg::Purged {
        instance: iid("jk-c41e"),
        result: Err(reason.into()),
    });
    assert_eq!(status(&app), reason);
    let kept = app.world.instance(&iid("jk-c41e")).expect("the instance");
    assert_ne!(kept.status, InstanceStatus::Purged);
}

#[test]
fn a_failed_prewarm_shows_the_reply_error() {
    let mut app = failure_app();
    app.deliver(&Msg::Prewarmed {
        workspace: 1,
        result: Err("the image build failed".into()),
    });
    assert_eq!(status(&app), "the image build failed");
}

#[test]
fn a_failed_validation_shows_the_reply_error() {
    let mut app = failure_app();
    let source = app
        .world
        .accounts
        .get(PERSONAL)
        .expect("account")
        .source
        .clone();
    app.deliver(&Msg::AccountValidated {
        account: PERSONAL.into(),
        source: Box::new(source),
        result: Err("the provider did not answer".into()),
    });
    assert!(
        status(&app).contains("the provider did not answer"),
        "{}",
        status(&app)
    );
}

#[test]
fn the_sim_keeps_the_last_good_values_when_the_broker_is_down() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    refresh_personal(&mut app);

    assert_eq!(
        status(&app),
        "Refreshed Claude · Personal · broker unreachable · last good kept · 4 windows"
    );
    let a = app.world.accounts.get(PERSONAL).expect("the account");
    assert_eq!(a.usage.freshness.phase, Freshness::Stale);
    assert_eq!(
        a.issue.as_ref().map(|i| &i.reason),
        Some(&IssueReason::BrokerDown)
    );
}

/// Answers a refresh with the roster it already has: a backend that keeps
/// no heartbeat of its own and never fails the index.
fn unchanged_roster(effect: &Effect, world: &World) -> Option<Msg> {
    match effect {
        Effect::Refresh { ticket } => Some(Msg::Refreshed {
            ticket: *ticket,
            result: Ok(Box::new(sim::snapshot(world))),
        }),
        _ => None,
    }
}

/// Ticks the app for `ms` of world time and returns the status line after
/// each tick.
fn run_for(app: &mut App, ms: i64) -> Vec<String> {
    let end = app.world.now_ms() + ms;
    let mut seen = vec![];
    while app.world.now_ms() < end {
        Application::handle(app, Input::Tick);
        seen.push(status(app));
    }
    seen
}

fn last_seen(w: &World) -> Vec<(InstanceId, i64)> {
    w.instances
        .iter()
        .filter_map(InstanceEntry::as_available)
        .map(|i| (i.id.clone(), i.last_seen_secs))
        .collect()
}

#[test]
fn a_backend_owns_last_seen() {
    let (mut app, backend) = backed(Scenario::HardCases, unchanged_roster);
    let before = last_seen(&app.world);
    run_for(&mut app, 60_000);

    assert!(
        backend.refreshes() >= 10,
        "the manager asks the backend for the roster"
    );
    assert_eq!(
        last_seen(&app.world),
        before,
        "the screen kept no heartbeat"
    );
}

#[test]
fn the_sim_fails_the_first_refresh_then_bumps_last_seen() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    let before = last_seen(&app.world);
    let start = app.world.now_secs();
    let live = |w: &World| {
        w.instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .filter(|i| i.status.is_live())
            .map(|i| (i.id.clone(), i.last_seen_secs))
            .collect::<Vec<_>>()
    };

    run_for(&mut app, 4_000);
    assert_eq!(
        last_seen(&app.world),
        before,
        "the first refresh fails and bumps nothing"
    );

    run_for(&mut app, 8_000);
    let bumped = live(&app.world);
    assert!(!bumped.is_empty());
    for (id, seen) in bumped {
        assert!(seen >= start + 4, "{id} last seen {seen}, started {start}");
    }
}

const TAKEOVER: &str = "tty004 · MacBook attached to this instance";

fn attached_for(app: &mut App, ms: i64) -> Vec<String> {
    app.go(Go::Attach {
        instance: iid("jk-7f3a"),
        pane: None,
    });
    run_for(app, ms)
}

#[test]
fn a_backend_opens_no_takeover() {
    let (mut app, _) = backed(Scenario::HardCases, unchanged_roster);
    let seen = attached_for(&mut app, 60_000);
    assert!(
        seen.iter()
            .all(|s| !s.contains("attached to this instance")),
        "{seen:?}"
    );
}

#[test]
fn the_sim_opens_the_second_client_at_45_seconds() {
    let mut app = timed(Scenario::HardCases, world_for(Scenario::HardCases));
    let seen = attached_for(&mut app, 60_000);
    assert!(seen.iter().any(|s| s == TAKEOVER), "{seen:?}");
    let first = seen.iter().position(|s| s == TAKEOVER).expect("a takeover");
    assert!(first > 30, "not before its time: tick {first}");
}

struct ListedSource;

impl FileSource for ListedSource {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        match dir {
            "/Users/alexey/src/shared-libs" => Ok(vec![
                FileEntry::dir("/Users/alexey/src/shared-libs/zeta").meta("zeta meta"),
                FileEntry::dir("/Users/alexey/src/shared-libs/alpha").meta("alpha meta"),
                FileEntry::file("/Users/alexey/src/shared-libs/notes.md"),
            ]),
            _ => Ok(vec![]),
        }
    }

    fn is_dir(&self, _: &str) -> bool {
        true
    }
}

fn workdir_picker(world: World, files: Option<fn() -> Box<dyn FileSource>>) -> String {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer: |_, _| None,
        unanswered: |_| false,
        files,
    });
    let mut world = world;
    world.backend = backend;
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down e");
    click_text(&mut app, "Choose…");
    screen(&mut app)
}

#[test]
fn the_working_directory_picker_lists_the_source() {
    let mut world = world_for(Scenario::Returning);
    world.fs.clear();
    let shown = workdir_picker(world, Some(|| Box::new(ListedSource)));
    let line = |dest: &str, meta: &str| shown.lines().any(|l| l.contains(dest) && l.contains(meta));
    assert!(line("/workspace/libs/zeta", "zeta meta"), "{shown}");
    assert!(line("/workspace/libs/alpha", "alpha meta"), "{shown}");
    assert!(
        !shown.contains("notes.md"),
        "a file is not a directory: {shown}"
    );
}

#[test]
fn the_working_directory_picker_lists_what_the_demo_listed() {
    let shown = workdir_picker(world_for(Scenario::Returning), None);
    let at = |meta: &str| {
        shown
            .find(meta)
            .unwrap_or_else(|| panic!("{meta}\n{shown}"))
    };
    assert!(
        at("6 items") < at("adr") && at("adr") < at("3 items"),
        "crates, docs and scripts in the order of the demo files: {shown}"
    );
    assert!(!shown.contains("rust"), "only direct children: {shown}");
}

#[test]
fn two_stops_due_in_one_tick_both_land() {
    let mut app = failure_app();
    let running: Vec<_> = ["jk-7f3a", "jk-9b02"]
        .iter()
        .map(|id| {
            let i = app.world.instance(&iid(id)).expect("the instance");
            assert_eq!(i.status, InstanceStatus::Running, "{id}");
            i.id.clone()
        })
        .collect();
    for instance in &running {
        app.world.perform(Effect::Stop {
            instance: instance.clone(),
        });
    }
    settle(&mut app);

    for id in &running {
        let stopped = app.world.instance(id).expect("the instance");
        assert_ne!(
            stopped.status,
            InstanceStatus::Running,
            "the second reply must not put {id} back"
        );
        assert!(!app.world.daemons.contains_key(id), "{id}");
    }
}

fn admin(w: &World) -> &RoleAdmin {
    w.surfaces.roles.as_ref().expect("the roles admin")
}

const MARKER: &str = "acct-claude-work";

/// Binds what was asked and, on its own, a second agent the screen never
/// touches, so a screen that writes its pending copy is told from one that
/// applies the snapshot.
fn bind_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::BindRole {
        role,
        agent,
        account,
        ..
    } = effect
    else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot
        .surfaces
        .roles
        .as_mut()
        .and_then(|a| a.role_mut(role))?;
    rec.bindings.insert(*agent, account.clone()?);
    rec.bindings.insert(Agent::ClaudeCode, MARKER.into());
    Some(Msg::RoleBound {
        role: role.clone(),
        agent: *agent,
        result: Ok(save_success(snapshot, None)),
    })
}

fn refuse_bind(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::BindRole { role, agent, .. } = effect else {
        return None;
    };
    Some(Msg::RoleBound {
        role: role.clone(),
        agent: *agent,
        result: Err("config.toml is locked by another jackin".into()),
    })
}

const PICK_EXPERIMENTS: &str = "tab enter down enter";

#[test]
fn role_bind_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, bind_role);
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::BindRole {
            role: "sentinel".into(),
            agent: Agent::Codex,
            base: Some("acct-codex-primary".into()),
            account: Some("acct-codex-experiments".into()),
        }]
    );
    let bindings = &admin(&app.world)
        .role("sentinel")
        .expect("sentinel")
        .bindings;
    assert_eq!(
        bindings.get(&Agent::Codex).map(String::as_str),
        Some("acct-codex-experiments")
    );
    assert_eq!(
        bindings.get(&Agent::ClaudeCode).map(String::as_str),
        Some(MARKER),
        "the role is the reply snapshot's, not the screen's"
    );
}

#[test]
fn committed_unsynced_role_bind_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::RoleBinding, |effect, world| {
        unsynced(bind_role(effect, world))
    });
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);

    let bindings = &admin(&app.world)
        .role("sentinel")
        .expect("sentinel")
        .bindings;
    assert_eq!(
        bindings.get(&Agent::Codex).map(String::as_str),
        Some("acct-codex-experiments")
    );
    assert_eq!(
        bindings.get(&Agent::ClaudeCode).map(String::as_str),
        Some(MARKER)
    );
    assert_eq!(
        status(&app),
        "Saved, durability uncertain · Bound Codex · Experiments to sentinel for Codex"
    );
    assert!(!screen(&mut app).contains("Could not save the binding"));
}

#[test]
fn failed_role_bind_keeps_the_binding() {
    let (mut app, _) = backed(Scenario::RoleBinding, refuse_bind);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    let shown = status(&app);
    assert!(
        shown.contains("config.toml is locked by another jackin")
            && shown.contains("Nothing changed"),
        "{shown}"
    );
}

/// Names the role the way the backend does, not the way the location reads.
fn add_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::AddRole { location } = effect else {
        return None;
    };
    let record = sim::surfaces::add_role("~/roles/registered-by-backend", |_| false).ok()?;
    let name = record.name.clone();
    let mut snapshot = sim::snapshot(world);
    snapshot.surfaces.roles.as_mut()?.roles.push(record);
    Some(Msg::RoleAdded {
        location: location.clone(),
        result: Ok((name, save_success(snapshot, None))),
    })
}

fn refuse_add(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::AddRole { location } = effect else {
        return None;
    };
    Some(Msg::RoleAdded {
        location: location.clone(),
        result: Err("the registry is read-only".into()),
    })
}

const ADD_NEW_ROLE: &str = "a ~ / r o l e s / n e w - r o l e enter tab tab enter";

#[test]
fn role_add_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, add_role);
    press(&mut app, ADD_NEW_ROLE);
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::AddRole {
            location: "~/roles/new-role".into()
        }]
    );
    let names: Vec<_> = admin(&app.world)
        .roles
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert!(names.contains(&"registered-by-backend"), "{names:?}");
    assert!(!names.contains(&"new-role"), "{names:?}");
    assert_eq!(status(&app), "Role registered-by-backend added");
}

#[test]
fn committed_unsynced_role_add_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::RoleBinding, |effect, world| {
        unsynced(add_role(effect, world))
    });
    press(&mut app, ADD_NEW_ROLE);
    settle(&mut app);

    let names: Vec<_> = admin(&app.world)
        .roles
        .iter()
        .map(|role| role.name.as_str())
        .collect();
    assert!(names.contains(&"registered-by-backend"), "{names:?}");
    assert!(!names.contains(&"new-role"), "{names:?}");
    assert_eq!(
        status(&app),
        "Saved, durability uncertain · Role registered-by-backend added"
    );
    assert!(!screen(&mut app).contains("Could not add role"));
}

#[test]
fn failed_role_add_keeps_the_location() {
    let (mut app, _) = backed(Scenario::RoleBinding, refuse_add);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, ADD_NEW_ROLE);
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    let shown = screen(&mut app);
    assert!(shown.contains("Could not add role"), "{shown}");
    assert!(shown.contains("the registry is read-only"), "{shown}");
    press(&mut app, "esc");
    let form = screen(&mut app);
    assert!(form.contains("register a role"), "{form}");
    assert!(form.contains("~/roles/new-role"), "{form}");
}

fn finding() -> RoleFinding {
    RoleFinding {
        area: FindingArea::Policy,
        key: Some("role.policy".into()),
        message: "found by the backend".into(),
        fix: Some("nothing to fix".into()),
    }
}

/// Reports a finding the simulator never raises for this manifest, and a
/// binding the screen never writes.
fn check_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::CheckRole { ticket, role, .. } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot.surfaces.roles.as_mut()?.role_mut(role)?;
    rec.check = RoleCheckState::Done(vec![finding()]);
    rec.bindings.insert(Agent::ClaudeCode, MARKER.into());
    Some(Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_check(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::CheckRole { ticket, role, .. } = effect else {
        return None;
    };
    Some(Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Err("the checker did not start".into()),
    })
}

#[test]
fn role_check_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, check_role);
    press(&mut app, "v");
    settle(&mut app);

    let seen = backend.seen();
    let [Effect::CheckRole { role, .. }] = seen.as_slice() else {
        panic!("one CheckRole expected, got {seen:?}");
    };
    assert_eq!(role, "sentinel");
    assert_eq!(
        admin(&app.world).role("sentinel").expect("sentinel").check,
        RoleCheckState::Done(vec![finding()]),
        "the check is the reply snapshot's"
    );
    assert_eq!(
        admin(&app.world).binding("sentinel", Agent::ClaudeCode),
        None,
        "a check reply changes the check only, never the bindings"
    );
    assert_eq!(status(&app), "Role sentinel: 1 error");
}

#[test]
fn failed_role_check_leaves_the_record() {
    let (mut app, _) = backed(Scenario::RoleBinding, fail_check);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, "v");
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    assert_eq!(status(&app), "Role sentinel: the checker did not start");
    let shown = screen(&mut app);
    assert!(!shown.contains("checking…"), "{shown}");
}

/// Removes the role asked for and, on its own, a second one.
fn remove_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::RemoveRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    snapshot
        .surfaces
        .roles
        .as_mut()?
        .roles
        .retain(|r| r.name != *role && r.name != "data-eng");
    Some(Msg::RoleRemoved {
        role: role.clone(),
        result: Ok(save_success(snapshot, None)),
    })
}

fn refuse_remove(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::RemoveRole { role } = effect else {
        return None;
    };
    Some(Msg::RoleRemoved {
        role: role.clone(),
        result: Err("config.toml is read-only".into()),
    })
}

const REMOVE_WRITER: &str = "down down down down d right enter";

#[test]
fn role_remove_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleBinding, remove_role);
    press(&mut app, REMOVE_WRITER);
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [Effect::RemoveRole {
            role: "writer".into()
        }]
    );
    let names: Vec<_> = admin(&app.world)
        .roles
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["sentinel", "the-architect", "reviewer"],
        "the roster is the reply snapshot's, not the screen's"
    );
    assert_eq!(status(&app), "Removed role writer");
}

#[test]
fn committed_unsynced_role_remove_applies_snapshot_and_warns() {
    let (mut app, _) = backed(Scenario::RoleBinding, |effect, world| {
        unsynced(remove_role(effect, world))
    });
    press(&mut app, REMOVE_WRITER);
    settle(&mut app);

    let names: Vec<_> = admin(&app.world)
        .roles
        .iter()
        .map(|role| role.name.as_str())
        .collect();
    assert_eq!(names, ["sentinel", "the-architect", "reviewer"]);
    assert_eq!(
        status(&app),
        "Saved, durability uncertain · Removed role writer"
    );
    assert!(!screen(&mut app).contains("Nothing changed"));
}

#[test]
fn failed_role_remove_keeps_the_role() {
    let (mut app, _) = backed(Scenario::RoleBinding, refuse_remove);
    let before = app.world.surfaces.roles.clone();
    press(&mut app, REMOVE_WRITER);
    settle(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    let shown = status(&app);
    assert!(
        shown.contains("config.toml is read-only") && shown.contains("Nothing changed"),
        "{shown}"
    );
}

const PINNED: &str = "backend-pinned";

/// Stores the manifest and pins the model, which the editor did not set.
fn save_role(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::SaveRole { role, doc, .. } = effect else {
        return None;
    };
    let mut stored = (**doc).clone();
    stored.codex_model = Some(PINNED.into());
    let mut snapshot = sim::snapshot(world);
    let rec = snapshot.surfaces.roles.as_mut()?.role_mut(role)?;
    rec.manifest = Ok(stored);
    Some(Msg::RoleSaved {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn fail_role_save(effect: &Effect, _: &World) -> Option<Msg> {
    let Effect::SaveRole { role, .. } = effect else {
        return None;
    };
    Some(Msg::RoleSaved {
        role: role.clone(),
        result: Err("jackin.role.toml is on a read-only disk".into()),
    })
}

const EDIT_AND_SAVE: &str = "enter tab tab tab tab tab p i n enter ctrl+s";

fn save_in_the_editor(app: &mut App) {
    press(app, EDIT_AND_SAVE);
    settle(app);
    press(app, "right enter");
    settle(app);
}

#[test]
fn role_save_comes_from_the_snapshot() {
    let (mut app, backend) = backed(Scenario::RoleEditor, save_role);
    let original = admin(&app.world)
        .role("sentinel")
        .expect("sentinel")
        .manifest
        .clone();
    save_in_the_editor(&mut app);

    let seen = backend.seen();
    let [
        Effect::CheckRole { .. },
        Effect::SaveRole { role, base, doc },
    ] = seen.as_slice()
    else {
        panic!("a check and a SaveRole expected, got {seen:?}");
    };
    assert_eq!(role, "sentinel");
    assert_eq!(
        Ok(&**base),
        original.as_ref(),
        "the base is the opened manifest"
    );
    assert_eq!(doc.codex_model.as_deref(), Some("pin"));
    let saved = admin(&app.world).role("sentinel").expect("sentinel");
    assert_eq!(
        saved.manifest.as_ref().map(|m| m.codex_model.as_deref()),
        Ok(Some(PINNED)),
        "the manifest is the reply snapshot's, not the screen's"
    );
    assert_eq!(app.route(), Route::Roles);
    assert_eq!(status(&app), "Role sentinel saved");
}

#[test]
fn failed_role_save_keeps_the_edits() {
    let (mut app, _) = backed(Scenario::RoleEditor, fail_role_save);
    let before = app.world.surfaces.roles.clone();
    save_in_the_editor(&mut app);

    assert_eq!(app.world.surfaces.roles, before);
    assert_eq!(app.route(), Route::RoleEditor);
    let shown = screen(&mut app);
    assert!(shown.contains("Save failed"), "{shown}");
    assert!(
        shown.contains("jackin.role.toml is on a read-only disk"),
        "{shown}"
    );
    press(&mut app, "esc");
    let editor = screen(&mut app);
    assert!(
        editor.contains("pin") && editor.contains("1 change"),
        "{editor}"
    );
}

#[test]
fn failed_editor_check_unlocks_the_editor() {
    let (mut app, backend) = backed(Scenario::RoleEditor, fail_check);
    press(&mut app, EDIT_AND_SAVE);
    settle(&mut app);

    assert_eq!(status(&app), "the checker did not start");
    let shown = screen(&mut app);
    assert!(
        !shown.contains("checking") && !shown.contains("Save role"),
        "{shown}"
    );
    press(&mut app, "ctrl+s");
    settle(&mut app);
    let checks = backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::CheckRole { .. }))
        .count();
    assert_eq!(checks, 2, "the editor takes a second check");
}

#[test]
fn a_restored_instance_gets_the_host_pane_policy_when_it_is_created() {
    use jackin_console::pages::Pages;
    use jackin_console::panes::ClipboardWrite;

    let (mut app, _) = backed(Scenario::Returning, restore);
    let id = "jk-c41e";
    let msg = Msg::Restored {
        instance: iid(id),
        result: Ok(Box::new(sim::snapshot(&app.world))),
    };
    let route = app.route();
    let App {
        shell,
        router,
        world,
        ..
    } = &mut app;
    let (_, requests) = shell.deliver(true, |cx| match router.screen_mut(route) {
        Some(screen) => screen.on_msg(&msg, world, cx),
        None => junie_tui::core::event::Outcome::Ignored,
    });
    assert!(!requests.is_empty(), "the screen asks to attach");

    let daemon = app
        .world
        .daemons
        .get(&iid(id))
        .expect("the restored daemon");
    assert_eq!(
        daemon.policy.clipboard_write,
        ClipboardWrite::Deny,
        "the host policy is on the daemon before the attach request runs"
    );
    assert!(!daemon.panes.is_empty());
}

fn held(unanswered: fn(&Effect) -> bool) -> (App, Arc<TestBackend>) {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer: restore,
        unanswered,
        files: None,
    });
    let mut world = world_for(Scenario::Returning);
    world.backend = backend.clone();
    (timed(Scenario::Returning, world), backend)
}

fn restores(backend: &TestBackend) -> usize {
    backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::Restore { .. }))
        .count()
}

#[test]
fn a_second_enter_on_a_restoring_instance_does_not_restore_it_again() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Restore { .. }));
    press(&mut app, "down right down down enter");
    press(&mut app, "enter");

    assert_eq!(restores(&backend), 1, "one restore for two Enters");
    assert_eq!(status(&app), "Restore already running");

    let reply = restore(
        &Effect::Restore {
            instance: iid("jk-c41e"),
        },
        &app.world,
    )
    .expect("the reply");
    app.deliver(&reply);
    assert_eq!(app.route(), Route::Capsule);
    assert!(app.world.daemons.contains_key(&iid("jk-c41e")));
}

#[test]
fn a_refresh_waits_for_the_one_in_flight() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Refresh { .. }));
    run_for(&mut app, 30_000);
    assert_eq!(backend.refreshes(), 1, "no second refresh while one is out");

    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(Box::new(sim::snapshot(&app.world))),
    });
    run_for(&mut app, 6_000);
    assert_eq!(backend.refreshes(), 2, "the next one goes after the reply");
}

#[test]
fn a_refresh_that_lands_after_a_newer_snapshot_is_dropped() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Refresh { .. }));
    run_for(&mut app, 6_000);
    assert_eq!(backend.refreshes(), 1);

    let old = sim::snapshot(&app.world);
    let mut newer = old.clone();
    newer.instances.retain(|i| i.id() != &iid("jk-c41e"));
    app.deliver(&Msg::Purged {
        instance: iid("jk-c41e"),
        result: Ok(Box::new(newer)),
    });
    assert!(app.world.instance(&iid("jk-c41e")).is_none());

    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(Box::new(old)),
    });
    assert!(
        app.world.instance(&iid("jk-c41e")).is_none(),
        "the older refresh did not bring the instance back"
    );
}

fn held_in(
    scenario: Scenario,
    unanswered: fn(&Effect) -> bool,
    answer: Answer,
) -> (App, Arc<TestBackend>) {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer,
        unanswered,
        files: None,
    });
    let mut world = world_for(scenario);
    world.backend = backend.clone();
    (timed(scenario, world), backend)
}

fn held_roles(unanswered: fn(&Effect) -> bool, answer: Answer) -> (App, Arc<TestBackend>) {
    held_in(Scenario::RoleBinding, unanswered, answer)
}

fn count(backend: &TestBackend, wanted: fn(&Effect) -> bool) -> usize {
    backend.seen().iter().filter(|e| wanted(e)).count()
}

static BINDS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn bind_once_then_refuse(effect: &Effect, world: &World) -> Option<Msg> {
    if !matches!(effect, Effect::BindRole { .. }) {
        return None;
    }
    match BINDS.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
        0 => bind_role(effect, world),
        _ => refuse_bind(effect, world),
    }
}

#[test]
fn an_undo_that_failed_can_be_tried_again() {
    let (mut app, backend) = backed(Scenario::RoleBinding, bind_once_then_refuse);
    press(&mut app, PICK_EXPERIMENTS);
    settle(&mut app);
    press(&mut app, "u");
    settle(&mut app);
    assert!(
        status(&app).contains("Could not save the binding"),
        "{}",
        status(&app)
    );

    press(&mut app, "u");
    settle(&mut app);
    assert_ne!(status(&app), "Nothing to undo");
    assert_eq!(count(&backend, |e| matches!(e, Effect::BindRole { .. })), 3);
}

#[test]
fn a_second_add_waits_for_the_first_reply() {
    let (mut app, backend) = held_roles(|e| matches!(e, Effect::AddRole { .. }), add_role);
    press(&mut app, ADD_NEW_ROLE);
    press(&mut app, "a");

    assert_eq!(status(&app), "Adding role…");
    let shown = screen(&mut app);
    assert!(!shown.contains("register a role"), "{shown}");
    assert_eq!(count(&backend, |e| matches!(e, Effect::AddRole { .. })), 1);
}

#[test]
fn an_add_that_fails_after_the_operator_left_the_roles_says_so() {
    let (mut app, _) = held_roles(|e| matches!(e, Effect::AddRole { .. }), add_role);
    press(&mut app, ADD_NEW_ROLE);
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);

    app.deliver(&Msg::RoleAdded {
        location: "~/roles/new-role".into(),
        result: Err("the registry is read-only".into()),
    });
    let shown = status(&app);
    assert!(
        shown.contains("Could not add role") && shown.contains("the registry is read-only"),
        "{shown}"
    );
}

#[test]
fn a_role_an_allowed_list_names_cannot_be_removed() {
    let mut world = world_for(Scenario::RoleBinding);
    for ws in &mut world.workspaces {
        ws.roles.default = None;
    }
    world.workspaces[0].roles.allowed =
        jackin_console::domain::workspace::AllowedRoles::Custom(vec!["sentinel".into()]);
    let name = world.workspaces[0].name.clone();
    let mut app = timed(Scenario::RoleBinding, world);
    press(&mut app, "d");

    let shown = screen(&mut app);
    assert!(shown.contains("Cannot remove role sentinel"), "{shown}");
    assert!(shown.contains(&name), "{shown}");
    assert!(!shown.contains("Remove role sentinel?"), "{shown}");
}

const WRITER: &str = "down down down down";

fn reread_manifest(effect: &Effect, world: &World) -> Option<Msg> {
    let Effect::LoadRole { role } = effect else {
        return None;
    };
    let mut snapshot = sim::snapshot(world);
    let admin = snapshot.surfaces.roles.as_mut()?;
    let doc = admin.role("sentinel")?.manifest.clone();
    admin.role_mut(role)?.manifest = doc;
    Some(Msg::RoleLoaded {
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    })
}

fn is_check(effect: &Effect) -> bool {
    matches!(effect, Effect::CheckRole { .. })
}

#[test]
fn a_role_whose_manifest_did_not_parse_is_read_again_before_the_check() {
    let (mut app, backend) = backed(Scenario::RoleBinding, reread_manifest);
    press(&mut app, WRITER);
    press(&mut app, "v");
    settle(&mut app);

    let seen = backend.seen();
    assert_eq!(
        seen.first(),
        Some(&Effect::LoadRole {
            role: "writer".into()
        }),
        "{seen:?}"
    );
    assert!(
        matches!(seen.get(1), Some(Effect::CheckRole { role, .. }) if role == "writer"),
        "{seen:?}"
    );
}

#[test]
fn a_manifest_that_still_does_not_parse_is_not_checked() {
    let (mut app, backend) = backed(Scenario::RoleBinding, |_, _| None);
    press(&mut app, WRITER);
    press(&mut app, "v");
    settle(&mut app);

    assert_eq!(count(&backend, |e| matches!(e, Effect::LoadRole { .. })), 1);
    assert_eq!(count(&backend, is_check), 0);
    assert_eq!(
        status(&app),
        "writer: jackin.role.toml still does not parse"
    );
}

#[test]
fn a_check_reply_of_another_ticket_does_not_finish_the_roles_check() {
    let (mut app, backend) = held_roles(is_check, check_role);
    press(&mut app, "v");
    let seen = backend.seen();
    let [Effect::CheckRole { ticket, role, .. }] = seen.as_slice() else {
        panic!("one CheckRole expected, got {seen:?}");
    };
    let mut snapshot = sim::snapshot(&app.world);
    snapshot
        .surfaces
        .roles
        .as_mut()
        .expect("roles")
        .role_mut(role)
        .expect("role")
        .check = RoleCheckState::Done(vec![finding()]);
    app.deliver(&Msg::RoleChecked {
        ticket: ticket + 1_000,
        role: role.clone(),
        result: Ok(Box::new(snapshot.clone())),
    });
    assert!(
        screen(&mut app).contains("checking…"),
        "the screen took the reply of another check"
    );
    assert_eq!(
        status(&app),
        format!("Checking {role}…"),
        "the reply of another check ended this one"
    );

    app.deliver(&Msg::RoleChecked {
        ticket: *ticket,
        role: role.clone(),
        result: Ok(Box::new(snapshot)),
    });
    assert_eq!(status(&app), format!("Role {role}: 1 error"));
    assert!(!screen(&mut app).contains("checking…"));
}

fn without_instances(app: &App) -> Box<jackin_console::world::Snapshot> {
    let mut snapshot = sim::snapshot(&app.world);
    snapshot.instances.clear();
    Box::new(snapshot)
}

#[test]
fn a_stop_confirmed_after_the_instance_left_is_refused() {
    let (mut app, backend) = backed(Scenario::Returning, |_, _| None);
    press(&mut app, "down right down t");
    let asked = screen(&mut app);
    assert!(asked.contains("Stop instance"), "{asked}");

    let gone = without_instances(&app);
    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(gone),
    });
    press(&mut app, "right enter");

    assert_eq!(count(&backend, |e| matches!(e, Effect::Stop { .. })), 0);
    assert!(status(&app).contains("nothing to stop"), "{}", status(&app));
}

#[test]
fn a_purge_confirmed_after_the_instance_left_is_refused() {
    let (mut app, backend) = backed(Scenario::Returning, |_, _| None);
    press(&mut app, "down right down down p");
    let asked = screen(&mut app);
    assert!(asked.contains("Purge instance"), "{asked}");

    let gone = without_instances(&app);
    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(gone),
    });
    press(&mut app, "c 4 1 e enter");

    assert_eq!(count(&backend, |e| matches!(e, Effect::Purge { .. })), 0);
    assert!(
        status(&app).contains("nothing to purge"),
        "{}",
        status(&app)
    );
}

#[test]
fn a_second_delete_waits_for_the_first_reply() {
    let (mut app, backend) = held_in(
        Scenario::Returning,
        |e| matches!(e, Effect::DeleteWorkspace { .. }),
        delete_workspace,
    );
    press(&mut app, "down d");
    press(&mut app, "right enter");
    press(&mut app, "d");
    press(&mut app, "right enter");

    assert_eq!(
        count(&backend, |e| matches!(e, Effect::DeleteWorkspace { .. })),
        1,
        "one delete for two confirmations"
    );
    assert_eq!(status(&app), "Workspace is deleting…");
}

#[test]
fn a_delete_error_for_a_workspace_that_is_gone_is_dropped() {
    let (mut app, _) = backed(Scenario::Returning, delete_workspace);
    let doomed = app.world.workspaces[0].id;
    press(&mut app, "down d");
    press(&mut app, "right enter");
    settle(&mut app);
    assert!(app.world.workspace(doomed).is_none());

    app.deliver(&Msg::WorkspaceDeleted {
        id: doomed,
        result: Err("no workspace with that id".into()),
    });
    assert!(
        !status(&app).contains("no workspace with that id"),
        "{}",
        status(&app)
    );
}

#[test]
fn inspect_names_the_account_the_launch_used() {
    let mut world = world_for(Scenario::Returning);
    let (agent, provider, ws, role) = {
        let i = world.instance(&iid("jk-7f3a")).expect("instance");
        (i.agent, i.agent.provider(), i.workspace, i.role.clone())
    };
    let resolved = world
        .account_for(
            jackin_console::domain::tuple::TupleIdentity::new(agent, provider),
            ws.and_then(|id| world.workspace(id)),
            role.as_deref(),
            None,
        )
        .account
        .expect("an account");
    let other = world
        .accounts
        .by_provider(provider)
        .find(|a| a.id != resolved)
        .expect("a second account")
        .clone();
    world
        .instance_mut(&iid("jk-7f3a"))
        .expect("instance")
        .account = Some(other.id.clone());
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, "down right down i");

    let shown = screen(&mut app);
    let wanted = format!("account {}", other.title());
    assert!(
        shown
            .lines()
            .any(|l| l.contains("Agent") && l.contains(&wanted)),
        "{wanted}\n{shown}"
    );
}

#[test]
fn a_refresh_failure_that_a_newer_reply_overtook_is_not_reported() {
    let (mut app, _) = backed(Scenario::Returning, |_, _| None);
    app.world.refresh_from = Some(app.world.snapshots_applied);
    app.world.apply(sim::snapshot(&app.world));
    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Err("instance index unreadable".into()),
    });
    assert!(!status(&app).contains("Refresh failed"), "{}", status(&app));

    app.world.refresh_from = Some(app.world.snapshots_applied);
    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Err("instance index unreadable".into()),
    });
    assert!(status(&app).contains("Refresh failed"), "{}", status(&app));
}

#[test]
fn a_second_restore_reply_keeps_the_daemon_the_first_built() {
    let (mut app, _) = backed(Scenario::Returning, restore);
    let id = "jk-c41e";
    let reply = |app: &App| {
        let mut snapshot = sim::snapshot(&app.world);
        for i in snapshot
            .instances
            .iter_mut()
            .filter_map(InstanceEntry::as_available_mut)
            .filter(|i| i.id == iid(id))
        {
            i.status = InstanceStatus::Running;
        }
        Msg::Restored {
            instance: iid(id),
            result: Ok(Box::new(snapshot)),
        }
    };
    let first = reply(&app);
    app.deliver(&first);
    let daemon = app
        .world
        .daemons
        .get_mut(&iid(id))
        .expect("the restored daemon");
    daemon.workspace = "kept".into();

    let second = reply(&app);
    app.deliver(&second);
    assert_eq!(app.world.daemons[&iid(id)].workspace, "kept");
}

#[test]
fn a_refresh_nobody_answers_does_not_stop_the_next_one() {
    let (mut app, backend) = held_in(
        Scenario::Returning,
        |e| matches!(e, Effect::Refresh { .. }),
        |_, _| None,
    );
    assert!(app.world.refresh());
    assert!(!app.world.refresh(), "one is out");
    assert_eq!(backend.refreshes(), 1);

    app.world.clock.advance(31_000);
    assert!(app.world.refresh(), "the first one was given up on");
    assert_eq!(backend.refreshes(), 2);
}

#[test]
fn a_backend_owns_last_seen_while_the_capsule_is_attached() {
    let (mut app, _) = backed(Scenario::HardCases, unchanged_roster);
    let before = last_seen(&app.world);
    app.go(Go::Attach {
        instance: iid("jk-7f3a"),
        pane: None,
    });
    assert_eq!(app.route(), Route::Capsule);
    run_for(&mut app, 20_000);

    assert_eq!(app.route(), Route::Capsule);
    assert_eq!(
        last_seen(&app.world),
        before,
        "the capsule kept no heartbeat"
    );
}

#[test]
fn a_saved_account_does_not_close_the_modal_the_operator_opened_after_leaving_the_form() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        store_accounts,
    );
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);
    let saves: Vec<_> = backend
        .seen()
        .into_iter()
        .filter(|e| matches!(e, Effect::SaveAccounts { .. }))
        .collect();
    assert_eq!(saves.len(), 1, "the save is out");

    press(&mut app, "esc");
    assert!(!screen(&mut app).contains("New account"));
    press(&mut app, "?");
    let help = screen(&mut app);
    assert!(help.contains("Keyboard shortcuts"), "{help}");

    let reply = store_accounts(&saves[0], &app.world).expect("the reply");
    app.deliver(&reply);
    let after = screen(&mut app);
    assert!(
        after.contains("Keyboard shortcuts"),
        "the reply closed a modal that is not the form:\n{after}"
    );
}

#[test]
fn a_refresh_that_a_newer_one_overtook_is_dropped_when_it_lands() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Refresh { .. }));
    assert!(app.world.refresh());
    app.world.clock.advance(31_000);
    assert!(app.world.refresh(), "the first one was given up on");
    let [first, second] = backend.refresh_tickets()[..] else {
        panic!(
            "two refreshes expected, got {:?}",
            backend.refresh_tickets()
        );
    };

    let named = |app: &App, name: &str| {
        let mut snapshot = sim::snapshot(&app.world);
        snapshot.workspaces[0].name = name.into();
        Box::new(snapshot)
    };
    let newer = named(&app, "from-the-second-refresh");
    app.deliver(&Msg::Refreshed {
        ticket: second,
        result: Ok(newer),
    });
    assert_eq!(app.world.workspaces[0].name, "from-the-second-refresh");

    let older = named(&app, "from-the-first-refresh");
    let late = app.deliver(&Msg::Refreshed {
        ticket: first,
        result: Ok(older),
    });
    assert_eq!(late, Outcome::Ignored);
    assert_eq!(app.world.workspaces[0].name, "from-the-second-refresh");

    app.deliver(&Msg::Refreshed {
        ticket: first,
        result: Err("instance index unreadable".into()),
    });
    assert!(!status(&app).contains("Refresh failed"), "{}", status(&app));
    assert_eq!(app.world.daemon_health, DaemonHealth::Healthy);
}

#[test]
fn a_late_reply_of_an_earlier_refresh_does_not_answer_the_one_in_flight() {
    let (mut app, backend) = held(|e| matches!(e, Effect::Refresh { .. }));
    assert!(app.world.refresh());
    app.world.clock.advance(31_000);
    assert!(app.world.refresh());
    let [first, second] = backend.refresh_tickets()[..] else {
        panic!(
            "two refreshes expected, got {:?}",
            backend.refresh_tickets()
        );
    };

    app.deliver(&Msg::Refreshed {
        ticket: first,
        result: Ok(Box::new(sim::snapshot(&app.world))),
    });
    assert!(!app.world.refresh(), "the second refresh is still out");

    let mut snapshot = sim::snapshot(&app.world);
    snapshot.workspaces[0].name = "from-the-second-refresh".into();
    app.deliver(&Msg::Refreshed {
        ticket: second,
        result: Ok(Box::new(snapshot)),
    });
    assert_eq!(app.world.workspaces[0].name, "from-the-second-refresh");
}

fn saves_of(backend: &TestBackend) -> Vec<Effect> {
    backend
        .seen()
        .into_iter()
        .filter(|e| matches!(e, Effect::SaveAccounts { .. }))
        .collect()
}

fn a_form_left_during_its_save() -> (App, Arc<TestBackend>, Effect) {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        store_accounts,
    );
    press(&mut app, NEW_ACCOUNT);
    click_text(&mut app, "Save");
    settle(&mut app);
    let saves = saves_of(&backend);
    assert_eq!(saves.len(), 1, "the save is out");

    press(&mut app, "esc");
    assert!(!screen(&mut app).contains("New account"));
    press(&mut app, "a");
    let reopened = screen(&mut app);
    assert!(reopened.contains("New account"), "{reopened}");
    (app, backend, saves[0].clone())
}

#[test]
fn a_saved_account_does_not_close_a_form_opened_after_the_one_it_saved() {
    let (mut app, _, save) = a_form_left_during_its_save();

    let reply = store_accounts(&save, &app.world).expect("the reply");
    app.deliver(&reply);

    let after = screen(&mut app);
    assert!(
        after.contains("New account"),
        "the reply closed the new form:\n{after}"
    );
}

#[test]
fn a_failed_save_does_not_write_its_error_into_a_form_opened_after_it() {
    let (mut app, _, save) = a_form_left_during_its_save();
    let Effect::SaveAccounts { ticket, .. } = save else {
        panic!("a save expected");
    };

    app.deliver(&Msg::AccountsSaved {
        ticket,
        added: None,
        result: Err("disk full".into()),
    });

    let after = screen(&mut app);
    assert!(after.contains("New account"), "{after}");
    assert_eq!(status(&app), "disk full");
    assert_eq!(
        after.matches("disk full").count(),
        1,
        "the error went into the new form too:\n{after}"
    );
}

fn unparsed_manifests(names: &[&str], unanswered: fn(&Effect) -> bool) -> (App, Arc<TestBackend>) {
    let backend = Arc::new(TestBackend {
        inner: SimBackend::shared(),
        seen: Mutex::new(vec![]),
        answer: |_, _| None,
        unanswered,
        files: None,
    });
    let mut world = world_for(Scenario::RoleBinding);
    world.backend = backend.clone();
    let admin = world.surfaces.roles.as_mut().expect("roles");
    for name in names {
        admin.role_mut(name).expect("role").manifest = Err("no manifest".into());
    }
    (timed(Scenario::RoleBinding, world), backend)
}

fn two_unparsed_manifests(unanswered: fn(&Effect) -> bool) -> (App, Arc<TestBackend>) {
    unparsed_manifests(&["the-architect", "writer"], unanswered)
}

#[test]
fn a_reply_nobody_asked_for_does_not_end_the_reads_of_check_all() {
    let (mut app, backend) =
        two_unparsed_manifests(|e| matches!(e, Effect::LoadRole { .. } | Effect::CheckRole { .. }));
    press(&mut app, "f5");
    assert_eq!(count(&backend, |e| matches!(e, Effect::LoadRole { .. })), 2);
    assert_eq!(status(&app), "Reading writer again…");

    app.deliver(&Msg::RoleLoaded {
        role: "ghost".into(),
        result: Err("unreadable".into()),
    });
    assert_eq!(status(&app), "Reading writer again…");
}

const DATA_ENG: &str = "down down down";

fn is_trust(effect: &Effect) -> bool {
    matches!(effect, Effect::TrustRole { .. })
}

fn data_eng_loads(backend: &TestBackend) -> usize {
    count(
        backend,
        |e| matches!(e, Effect::LoadRole { role } if role == "data-eng"),
    )
}

fn untrusted_and_unparsed() -> (App, Arc<TestBackend>) {
    unparsed_manifests(&["data-eng"], |_| false)
}

fn assert_untrusted(app: &App) {
    let entry = role(&app.world, "data-eng");
    assert!(!entry.trusted, "the role was trusted without the operator");
    assert!(!entry.launchable(), "the role became launchable");
}

#[test]
fn v_on_an_untrusted_unparsed_role_asks_for_trust_before_reading_it() {
    let (mut app, backend) = untrusted_and_unparsed();
    press(&mut app, DATA_ENG);
    press(&mut app, "v");
    settle(&mut app);

    assert_eq!(data_eng_loads(&backend), 0, "{:?}", backend.seen());
    assert_eq!(count(&backend, is_trust), 0, "{:?}", backend.seen());
    assert_untrusted(&app);
    let shown = screen(&mut app);
    assert!(shown.contains("Trust & load"), "{shown}");
}

#[test]
fn check_all_does_not_read_an_untrusted_unparsed_role() {
    let (mut app, backend) = unparsed_manifests(&["data-eng", "writer"], |_| false);
    press(&mut app, "f5");
    settle(&mut app);

    assert_eq!(data_eng_loads(&backend), 0, "{:?}", backend.seen());
    assert_eq!(count(&backend, is_trust), 0, "{:?}", backend.seen());
    assert_untrusted(&app);
    assert_eq!(
        count(
            &backend,
            |e| matches!(e, Effect::LoadRole { role } if role == "writer")
        ),
        1,
        "a trusted unparsed role is still read"
    );
}

#[test]
fn trusting_an_unparsed_role_from_the_roles_screen_reads_it_after_the_trust() {
    let (mut app, backend) = untrusted_and_unparsed();
    press(&mut app, DATA_ENG);
    press(&mut app, "v right enter");
    settle(&mut app);

    assert_eq!(
        backend.seen(),
        [
            Effect::TrustRole {
                role: "data-eng".into()
            },
            Effect::LoadRole {
                role: "data-eng".into()
            },
        ]
    );
    assert!(role(&app.world, "data-eng").trusted);
}

#[test]
fn declining_the_trust_leaves_the_unparsed_role_unread() {
    let (mut app, backend) = untrusted_and_unparsed();
    press(&mut app, DATA_ENG);
    press(&mut app, "v esc");
    settle(&mut app);

    assert_eq!(backend.seen(), []);
    assert_untrusted(&app);
    assert_eq!(status(&app), "Not trusted · role not read");
}

#[test]
fn a_refused_trust_leaves_the_unparsed_role_unread() {
    let (mut app, backend) = backed(Scenario::RoleBinding, refuse_trust);
    app.world
        .surfaces
        .roles
        .as_mut()
        .expect("roles")
        .role_mut("data-eng")
        .expect("role")
        .manifest = Err("no manifest".into());
    press(&mut app, DATA_ENG);
    press(&mut app, "v right enter");
    settle(&mut app);

    assert_eq!(data_eng_loads(&backend), 0, "{:?}", backend.seen());
    assert_untrusted(&app);
    assert_eq!(status(&app), "the trust store is read-only");
}

#[test]
fn loading_a_role_does_not_trust_it() {
    let world = world_for(Scenario::RoleBinding);
    assert!(!role(&world, "data-eng").trusted);
    let backend = SimBackend::shared();
    let effect = Effect::LoadRole {
        role: "data-eng".into(),
    };
    backend.perform(&effect, &world);
    let Some(Msg::RoleLoaded { result, .. }) = backend.settle(&effect, &world) else {
        panic!("a load answers with RoleLoaded");
    };
    assert!(result.is_err(), "the load of an untrusted role was granted");
}

#[test]
fn removing_an_account_scrubs_it_from_every_workspace() {
    let id = "acct-claude-work";
    let world = world_for(Scenario::Returning);
    let names = |world: &World| {
        world
            .workspaces
            .iter()
            .filter(|ws| {
                ws.accounts.enabled.contains(id)
                    || ws.accounts.disabled_defaults.contains(id)
                    || ws.accounts.preferred.values().any(|v| v == id)
                    || ws.accounts.role_preferred.values().any(|v| v == id)
            })
            .count()
    };
    assert!(names(&world) > 0, "a workspace names the account to remove");

    let mut app = timed(Scenario::Returning, world);
    app.go(Go::Accounts {
        select: Some(id.into()),
    });
    press(&mut app, "d right enter");
    settle(&mut app);

    assert!(app.world.accounts.get(id).is_none(), "{}", status(&app));
    assert_eq!(names(&app.world), 0);
}

fn saves_asked(backend: &TestBackend) -> usize {
    saves_of(backend).len()
}

#[test]
fn a_save_nobody_answers_does_not_block_the_next_one_forever() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        |_, _| None,
    );
    app.go(Go::Accounts {
        select: Some("acct-claude-personal".into()),
    });
    press(&mut app, "t");
    assert_eq!(saves_asked(&backend), 1);

    press(&mut app, "t");
    assert_eq!(saves_asked(&backend), 1, "the first save is still out");
    assert!(
        status(&app).contains("Saving the previous change"),
        "{}",
        status(&app)
    );

    app.world.clock.advance(31_000);
    press(&mut app, "t");
    assert_eq!(saves_asked(&backend), 2, "a save nobody answered gives way");
}

#[test]
fn the_late_reply_of_a_save_that_gave_way_does_not_answer_the_next_save() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        store_accounts,
    );
    app.go(Go::Accounts {
        select: Some("acct-claude-personal".into()),
    });
    press(&mut app, "t");
    app.world.clock.advance(31_000);
    press(&mut app, "t");
    let saves = saves_of(&backend);
    let [first, second] = saves.as_slice() else {
        panic!("two saves expected, got {saves:?}");
    };

    let late = store_accounts(first, &app.world).expect("the reply of the first save");
    app.deliver(&late);
    press(&mut app, "t");
    assert_eq!(saves_asked(&backend), 2, "the second save is still out");
    assert!(
        status(&app).contains("Saving the previous change"),
        "{}",
        status(&app)
    );

    let failure = fail_accounts(second, &app.world).expect("the reply of the second save");
    app.deliver(&failure);
    assert_eq!(status(&app), "disk full");
    press(&mut app, "t");
    assert_eq!(saves_asked(&backend), 3, "the screen is free again");
}

fn saved_as(effect: &Effect, world: &World, purpose: &str) -> Msg {
    let Effect::SaveAccounts { ticket, .. } = effect else {
        panic!("a save expected, got {effect:?}");
    };
    let mut snapshot = sim::snapshot(world);
    for account in &mut snapshot.accounts.accounts {
        if account.id == PERSONAL {
            account.purpose = Some(purpose.into());
        }
    }
    Msg::AccountsSaved {
        ticket: *ticket,
        added: None,
        result: Ok(save_success(snapshot, None)),
    }
}

fn saved_as_unsynced(effect: &Effect, world: &World, purpose: &str) -> Msg {
    let Msg::AccountsSaved {
        ticket,
        added,
        result: Ok(mut saved),
    } = saved_as(effect, world, purpose)
    else {
        panic!("an account save expected");
    };
    saved.warning = Some(SaveWarning::DirectorySync);
    Msg::AccountsSaved {
        ticket,
        added,
        result: Ok(saved),
    }
}

fn personal_purpose(app: &App) -> Option<String> {
    app.world
        .accounts
        .get(PERSONAL)
        .and_then(|a| a.purpose.clone())
}

#[test]
fn the_late_reply_of_a_save_that_gave_way_does_not_roll_back_the_newer_save() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        |_, _| None,
    );
    app.go(Go::Accounts {
        select: Some(PERSONAL.into()),
    });
    press(&mut app, "t");
    app.world.clock.advance(31_000);
    press(&mut app, "t");
    let saves = saves_of(&backend);
    let [first, second] = saves.as_slice() else {
        panic!("two saves expected, got {saves:?}");
    };

    app.deliver(&saved_as(second, &app.world, "newer"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("newer"));

    app.deliver(&saved_as(first, &app.world, "older"));
    assert_eq!(
        personal_purpose(&app).as_deref(),
        Some("newer"),
        "the late reply of the first save rolled the registry back"
    );
}

#[test]
fn the_late_reply_of_a_save_that_gave_way_still_lands_while_the_newer_save_is_out() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        |_, _| None,
    );
    app.go(Go::Accounts {
        select: Some(PERSONAL.into()),
    });
    press(&mut app, "t");
    app.world.clock.advance(31_000);
    press(&mut app, "t");
    let saves = saves_of(&backend);
    let [first, second] = saves.as_slice() else {
        panic!("two saves expected, got {saves:?}");
    };

    app.deliver(&saved_as(first, &app.world, "older"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("older"));

    app.deliver(&saved_as(second, &app.world, "newer"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("newer"));
}

#[test]
fn an_unsynced_old_reply_applies_while_leaving_the_newer_save_in_flight() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        |_, _| None,
    );
    app.go(Go::Accounts {
        select: Some(PERSONAL.into()),
    });
    press(&mut app, "t");
    app.world.clock.advance(31_000);
    press(&mut app, "t");
    let saves = saves_of(&backend);
    let [first, second] = saves.as_slice() else {
        panic!("two saves expected, got {saves:?}");
    };

    app.deliver(&saved_as_unsynced(first, &app.world, "older"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("older"));
    assert_eq!(status(&app), "Saved, durability uncertain");

    press(&mut app, "t");
    assert!(status(&app).contains("Saving the previous change"));
    app.deliver(&saved_as(second, &app.world, "newer"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("newer"));
}

#[test]
fn an_unsynced_reply_after_a_newer_commit_is_ignored_without_a_durability_warning() {
    let (mut app, backend) = held_in(
        Scenario::AccountsMixed,
        |e| matches!(e, Effect::SaveAccounts { .. }),
        |_, _| None,
    );
    app.go(Go::Accounts {
        select: Some(PERSONAL.into()),
    });
    press(&mut app, "t");
    app.world.clock.advance(31_000);
    press(&mut app, "t");
    let saves = saves_of(&backend);
    let [first, second] = saves.as_slice() else {
        panic!("two saves expected, got {saves:?}");
    };

    app.deliver(&saved_as(second, &app.world, "newer"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("newer"));
    let committed = status(&app);
    app.deliver(&saved_as_unsynced(first, &app.world, "older"));
    assert_eq!(personal_purpose(&app).as_deref(), Some("newer"));
    assert_ne!(
        status(&app),
        "Saved, durability uncertain",
        "a stale reply must not warn about a durability the newer commit already settled"
    );
    assert_eq!(
        status(&app),
        committed,
        "a stale reply must leave the status of the newer commit alone"
    );

    app.set_status(
        "Routine refresh complete",
        junie_tui::theme::Tone::Secondary,
    );
    assert_eq!(
        status(&app),
        "Routine refresh complete",
        "a stale reply must not leave a durability warning behind"
    );
}

#[test]
fn the_usage_screen_refreshes_every_enabled_account_and_shows_the_reply() {
    let (mut app, backend) = backed(Scenario::AccountsMixed, refresh_account);
    app.go(Go::Usage {
        select: Some(PERSONAL.into()),
    });
    assert!(screen(&mut app).contains("38%"));

    let enabled = app
        .world
        .accounts
        .accounts
        .iter()
        .filter(|a| a.enabled)
        .count();
    press(&mut app, "r");
    assert!(
        status(&app).starts_with("Reloading the broker projection"),
        "{}",
        status(&app)
    );
    settle(&mut app);

    let asked = backend
        .seen()
        .iter()
        .filter(|e| matches!(e, Effect::RefreshAccount { .. }))
        .count();
    assert_eq!(
        asked, enabled,
        "one refresh for each enabled account; the world, not the Refreshing marker the \
         fixture seeds, says which requests are out"
    );
    let shown = screen(&mut app);
    assert!(shown.contains("40%"), "{shown}");
}
