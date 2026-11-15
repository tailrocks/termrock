#![cfg(feature = "demo")]

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::{
    Backend, Completion, Effect, FileSource, OnePassword, OpSession, PlainKey,
};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::agent::Provider;
use jackin_console::domain::instance::{
    DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus, SandboxPhase,
    UnavailableInstance,
};
use jackin_console::domain::launch::{LaunchContent, LaunchFailure, NextStep, RefusalReason};
use jackin_console::domain::tuple::InstanceChoice;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{
    EnvAnswer, EnvQuestion, FailedLaunch, LaunchEvent, LaunchFeed, LaunchTarget, ReadyInstance,
    TypedKey,
};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, SaveWarning, World};
use junie_tui::core::event::{Input, Outcome};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::steps::StepState;

const WORK: &str = "acct-claude-work";

fn iid(label: &str) -> InstanceId {
    InstanceId::simulated(label)
}

fn available(entry: &InstanceEntry) -> &Instance {
    entry
        .as_available()
        .expect("the simulator has verified metadata")
}

fn started(world: World) -> App {
    app_with_world(
        Scenario::LaunchRunning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn tick(app: &mut App, times: usize) {
    for _ in 0..times {
        Application::handle(app, Input::Tick);
    }
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..600 {
        if done(app) {
            return;
        }
        tick(app, 1);
    }
    let shown = screen(app);
    panic!("never reached: {what}\n{shown}");
}

fn shows(text: &'static str) -> impl Fn(&mut App) -> bool {
    move |app| screen(app).contains(text)
}

fn facts(i: &Instance) -> String {
    format!(
        "{} | {} | {} | {:?} | {} | {} | {} | {}",
        i.id,
        i.container,
        i.workdir.as_deref().expect("test instance workdir"),
        i.status,
        i.created_secs,
        i.run_id,
        i.github.as_ref().map_or("-", |r| r.full_name.as_str()),
        i.branch.as_deref().unwrap_or("-"),
    )
}

fn the_sim_launch(world: World) -> App {
    let mut app = started(world);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    app
}

#[test]
fn the_sim_launch_ends_as_it_did() {
    let app = the_sim_launch(world_for(Scenario::LaunchRunning));
    let record = app
        .world
        .instance(&iid("jk-b5df"))
        .expect("the launched instance");
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | Running | 1788401643 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(record.workspace, Some(1));
    assert_eq!(record.role.as_deref(), Some("the-architect"));
    assert_eq!(record.agent, Agent::ClaudeCode);
    assert_eq!(record.default_branch, "main");
    assert_eq!(record.account.as_deref(), Some(WORK));
    assert_eq!(
        record.accounts,
        [
            "acct-claude-personal",
            "acct-claude-work",
            "acct-codex-primary",
            "acct-grok-team",
            "acct-opencode-go",
        ]
    );

    let daemon = app.world.daemons.get(&iid("jk-b5df")).expect("its daemon");
    assert_eq!(daemon.workspace, "payments-platform");
    assert_eq!(daemon.tabs.len(), 1);
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    assert_eq!(pane.agent, Some(Agent::ClaudeCode));
    assert_eq!(pane.account.as_deref(), Some(WORK));
}

#[test]
fn the_sim_launch_records_every_selected_agent_and_default_panes_follow_them() {
    let world = world_for(Scenario::LaunchRunning);
    let target = LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances: vec![
            InstanceChoice {
                agent: Agent::Codex,
                provider: Provider::OpenAi,
                account: Some("acct-codex-primary".into()),
                model: "gpt-5-codex".into(),
            },
            InstanceChoice {
                agent: Agent::Omp,
                provider: Provider::OpenCode,
                account: Some("acct-opencode-go".into()),
                model: "omp-default".into(),
            },
        ],
        task: None,
    };
    let mut feed = SimBackend::shared().launch(&world, &target);
    let instance = (0..1_000)
        .flat_map(|_| feed.poll(&world))
        .find_map(|event| match event {
            LaunchEvent::Ready(ready) => Some(
                ready
                    .instance
                    .as_available()
                    .expect("sim launch has verified metadata")
                    .clone(),
            ),
            _ => None,
        })
        .expect("simulation finishes the launch");

    assert_eq!(
        instance
            .agents
            .iter()
            .map(|agent| agent.agent)
            .collect::<Vec<_>>(),
        [Agent::Codex, Agent::Omp]
    );
    assert_eq!(instance.agents[0].provider, Provider::OpenAi);
    assert_eq!(
        instance.agents[0].account.as_deref(),
        Some("acct-codex-primary")
    );
    assert_eq!(instance.agents[0].model, "gpt-5-codex");
    assert_eq!(instance.agents[1].provider, Provider::OpenCode);
    assert_eq!(
        instance.agents[1].account.as_deref(),
        Some("acct-opencode-go")
    );
    assert_eq!(instance.agents[1].model, "omp-default");

    let panes = SimBackend::shared().initial_panes(&instance);
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].agent(), Some(Agent::Codex));
    assert_eq!(
        panes[0].account().map(String::as_str),
        Some("acct-codex-primary")
    );
    assert_eq!(panes[1].agent(), Some(Agent::Omp));
    assert_eq!(
        panes[1].account().map(String::as_str),
        Some("acct-opencode-go")
    );
}

#[test]
fn unavailable_agent_metadata_keeps_a_named_running_row_and_blocks_attach() {
    let mut world = world_for(Scenario::Returning);
    world.instances.clear();
    world.daemons.clear();
    let instance = UnavailableInstance::new(
        InstanceId::new("jackin-local", "jk-unreadable"),
        Some(1),
        Some("payments-platform".into()),
        None,
        SandboxPhase::Ready,
        Some(1_788_401_000),
        "required agent metadata is malformed",
    );
    let instance_id = instance.id.clone();
    world.admit_entry(InstanceEntry::Unavailable(instance.clone()));
    world.sync_arbiter();

    let mut app = app_with_world(
        Scenario::Returning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "*");
    press(&mut app, "down");
    press(&mut app, "right");
    let shown = screen(&mut app);
    assert!(shown.contains("unreadable"), "{shown}");
    assert!(shown.contains("metadata unavailable"), "{shown}");
    assert!(
        shown.contains("required agent metadata is malformed"),
        "{shown}"
    );
    assert!(
        shown.contains("Role") && shown.contains("unavailable"),
        "{shown}"
    );
    assert!(!shown.contains("failed setup"), "{shown}");

    press(&mut app, "i");
    let inspected = screen(&mut app);
    assert!(inspected.contains("jk-unreadable"), "{inspected}");
    assert!(inspected.contains("jackin-local"), "{inspected}");
    assert!(inspected.contains("ready"), "{inspected}");
    assert!(
        inspected.contains("required agent metadata is malformed"),
        "{inspected}"
    );
    press(&mut app, "esc");

    let Some(InstanceEntry::Unavailable(row)) = app.world.entry(&instance_id) else {
        panic!("the degraded sandbox remains a named unavailable entry");
    };
    assert_eq!(row.phase, SandboxPhase::Ready);
    assert_eq!(row.reason, "required agent metadata is malformed");
    assert!(row.role.is_none());
    assert!(
        app.world
            .entry(&instance_id)
            .unwrap()
            .as_available()
            .is_none()
    );
    press(&mut app, "enter");
    assert_eq!(app.route(), Route::Manager);
    assert!(status(&app).contains("required agent metadata is malformed"));
    assert!(!app.world.daemons.contains_key(&instance_id));
    for key in ["a", "n", "!"] {
        press(&mut app, key);
        assert!(
            status(&app).contains("required agent metadata is malformed"),
            "{key} must refuse metadata-dependent actions with the observed reason"
        );
        assert_eq!(app.route(), Route::Manager);
        assert!(!app.world.daemons.contains_key(&instance_id));
    }

    press(&mut app, "t");
    tick_until(&mut app, "the power stop reply", |app| {
        matches!(
            app.world.entry(&instance_id),
            Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Stopped
        )
    });
    let Some(InstanceEntry::Unavailable(stopped)) = app.world.entry(&instance_id) else {
        panic!("power stop keeps the metadata-degraded entry");
    };
    assert_eq!(stopped.phase, SandboxPhase::Stopped);
    assert_eq!(stopped.reason, "required agent metadata is malformed");
    click_text(&mut app, "Start sandbox");
    press(&mut app, "enter");
    tick_until(&mut app, "the power start reply", |app| {
        matches!(
            app.world.entry(&instance_id),
            Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
        )
    });
    let Some(InstanceEntry::Unavailable(started)) = app.world.entry(&instance_id) else {
        panic!("power start keeps the metadata-degraded entry");
    };
    assert_eq!(started.phase, SandboxPhase::Ready);
    assert_eq!(started.reason, "required agent metadata is malformed");
}

#[test]
fn unattributed_sandbox_keeps_missing_workspace_and_creation_facts_visible() {
    let mut world = world_for(Scenario::Returning);
    world.instances.clear();
    world.daemons.clear();
    let unavailable = UnavailableInstance::new(
        InstanceId::new("gateway-legacy", "sandbox-without-creation-time"),
        None,
        Some("removed-workspace".into()),
        None,
        SandboxPhase::Stopped,
        None,
        "agent metadata file is missing",
    );
    world.admit_entry(InstanceEntry::Unavailable(unavailable));
    world.sync_arbiter();
    assert_eq!(world.running_count(), 0);
    assert_eq!(world.arbiter.running(), Ok(0));

    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "*");
    press(&mut app, "down down down down down down");
    let shown = screen_at(&mut app, 180, 50);
    assert!(shown.contains("Unattributed sandboxes"), "{shown}");
    assert!(shown.contains("sandbox-without-creation-time"), "{shown}");
    assert!(shown.contains("gateway-legacy"), "{shown}");
    assert!(shown.contains("removed-workspace"), "{shown}");
    assert!(shown.contains("creation time unavailable"), "{shown}");
    assert!(shown.contains("stopped"), "{shown}");
    assert!(shown.contains("agent metadata file is missing"), "{shown}");
    assert!(!shown.contains("FailedSetup"), "{shown}");
    assert!(!shown.contains("Codex"), "{shown}");
}

#[test]
fn unavailable_power_routes_by_full_gateway_and_sandbox_identity() {
    let rows = [
        UnavailableInstance::new(
            InstanceId::new("gateway-a", "sandbox-shared-name"),
            Some(1),
            Some("payments-platform".into()),
            Some("the-architect".into()),
            SandboxPhase::Ready,
            Some(1_788_401_000),
            "metadata unreadable on gateway-a",
        ),
        UnavailableInstance::new(
            InstanceId::new("gateway-b", "sandbox-shared-name"),
            Some(1),
            Some("payments-platform".into()),
            Some("the-architect".into()),
            SandboxPhase::Ready,
            Some(1_788_401_000),
            "metadata unreadable on gateway-b",
        ),
        UnavailableInstance::new(
            InstanceId::new("gateway-a", "sandbox-alpha-1234"),
            Some(1),
            Some("payments-platform".into()),
            Some("the-architect".into()),
            SandboxPhase::Ready,
            Some(1_788_401_000),
            "metadata unreadable on alpha",
        ),
        UnavailableInstance::new(
            InstanceId::new("gateway-a", "sandbox-beta-1234"),
            Some(1),
            Some("payments-platform".into()),
            Some("the-architect".into()),
            SandboxPhase::Ready,
            Some(1_788_401_000),
            "metadata unreadable on beta",
        ),
    ];
    assert_ne!(rows[0].id, rows[1].id);
    assert_eq!(rows[0].id.short_label(), rows[1].id.short_label());
    assert_ne!(rows[2].id, rows[3].id);
    assert_eq!(rows[2].id.short_label(), rows[3].id.short_label());

    let second_id = rows[1].id.clone();
    let first_id = rows[0].id.clone();
    let mut world = world_for(Scenario::Returning);
    world.instances.clear();
    world.daemons.clear();
    for row in rows {
        world.admit_entry(InstanceEntry::Unavailable(row));
    }
    world.sync_arbiter();
    assert_eq!(world.running_count(), 4);
    assert_eq!(world.arbiter.running(), Ok(4));
    let mut app = app_with_world(
        Scenario::Returning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "*");
    press(&mut app, "down right down");
    let shown = screen(&mut app);
    assert!(shown.contains("sandbox-shared-name · gateway-b"), "{shown}");
    press(&mut app, "t");
    tick_until(&mut app, "the selected gateway stopped", |app| {
        matches!(
            app.world.entry(&second_id),
            Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Stopped
        )
    });
    assert_eq!(app.world.running_count(), 3);
    assert_eq!(app.world.arbiter.running(), Ok(3));
    assert!(matches!(
        app.world.entry(&first_id),
        Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
    ));

    press(&mut app, "r");
    tick_until(&mut app, "the selected gateway started", |app| {
        matches!(
            app.world.entry(&second_id),
            Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
        )
    });
    assert_eq!(app.world.running_count(), 4);
    assert_eq!(app.world.arbiter.running(), Ok(4));
    assert!(matches!(
        app.world.entry(&first_id),
        Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
    ));
}

#[test]
fn unavailable_start_requires_stopped_phase_at_ui_and_effect_boundaries() {
    let phases = [
        SandboxPhase::Provisioning,
        SandboxPhase::Starting,
        SandboxPhase::Ready,
        SandboxPhase::Stopping,
        SandboxPhase::Stopped,
        SandboxPhase::Completed { exit_code: 0 },
        SandboxPhase::Error { exit_code: Some(7) },
        SandboxPhase::Error { exit_code: None },
        SandboxPhase::Deleting,
        SandboxPhase::Suspending,
        SandboxPhase::Suspended,
        SandboxPhase::Unknown,
    ];
    for (index, phase) in phases.into_iter().enumerate() {
        let id = InstanceId::new("gateway-phase-test", format!("sandbox-phase-{index}"));
        let mut world = world_for(Scenario::Returning);
        let control = world
            .instances
            .iter()
            .find_map(InstanceEntry::as_available)
            .expect("returning scenario has a verified control row")
            .clone();
        world.instances = vec![InstanceEntry::available(control)];
        world.daemons.clear();
        world.sync_arbiter();
        world.admit_entry(InstanceEntry::Unavailable(UnavailableInstance::new(
            id.clone(),
            Some(1),
            Some("payments-platform".into()),
            None,
            phase.clone(),
            Some(1_788_401_000),
            "required agent metadata is malformed",
        )));
        let mut app = app_with_world(
            Scenario::Returning,
            world,
            Motion::Reduced,
            0,
            Theme::for_level(ColorLevel::TrueColor),
        );
        press(&mut app, "*");
        click_text(&mut app, &id.sandbox_name);
        let affordances = screen_at(&mut app, 180, 50);
        if phase.startable() {
            assert!(affordances.contains("Start sandbox"), "{affordances}");
        } else if phase.ready() {
            assert!(affordances.contains("Stop sandbox"), "{affordances}");
            assert!(!affordances.contains("Start sandbox"), "{affordances}");
        } else {
            assert!(!affordances.contains("Start sandbox"), "{affordances}");
            assert!(
                affordances.contains("start only available while stopped"),
                "{affordances}"
            );
        }

        if phase.startable() {
            app.world.perform(Effect::PowerStart {
                instance: id.clone(),
            });
            tick_until(&mut app, "the stopped sandbox starts", |app| {
                matches!(
                    app.world.entry(&id),
                    Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
                )
            });
            app.world.perform(Effect::PowerStart {
                instance: id.clone(),
            });
            tick_until(&mut app, "the repeated start is refused", |app| {
                app.world.jobs.is_empty()
            });
            assert!(matches!(
                app.world.entry(&id),
                Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
            ));
            press(&mut app, "r");
            assert!(matches!(
                app.world.entry(&id),
                Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
            ));
            assert!(status(&app).contains("already ready"));
            press(&mut app, "t");
            tick_until(&mut app, "the ready sandbox stops", |app| {
                matches!(
                    app.world.entry(&id),
                    Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Stopped
                )
            });
            press(&mut app, "r");
            tick_until(
                &mut app,
                "the stopped sandbox starts from its shortcut",
                |app| {
                    matches!(
                        app.world.entry(&id),
                        Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Ready
                    )
                },
            );
        } else {
            press(&mut app, "r");
            let reason = if phase.ready() {
                "already ready"
            } else {
                "only stopped sandboxes can start"
            };
            assert!(status(&app).contains(reason), "{}", status(&app));
            assert!(matches!(
                app.world.entry(&id),
                Some(InstanceEntry::Unavailable(row)) if row.phase == phase
            ));
            app.world.perform(Effect::PowerStart {
                instance: id.clone(),
            });
            tick_until(&mut app, "the phase-invalid start is refused", |app| {
                app.world.jobs.is_empty()
            });
            assert!(matches!(
                app.world.entry(&id),
                Some(InstanceEntry::Unavailable(row)) if row.phase == phase
            ));
            if phase.ready() {
                press(&mut app, "t");
                tick_until(&mut app, "the ready sandbox stops", |app| {
                    matches!(
                        app.world.entry(&id),
                        Some(InstanceEntry::Unavailable(row)) if row.phase == SandboxPhase::Stopped
                    )
                });
            }
        }
    }
}

#[test]
fn simulated_plain_text_replaces_only_the_lead_account_projection() {
    let target = LaunchTarget {
        workspace: Some(1),
        role: "the-architect".into(),
        instances: vec![
            InstanceChoice {
                agent: Agent::Codex,
                provider: Provider::OpenAi,
                account: Some("acct-codex-primary".into()),
                model: "gpt-5-codex".into(),
            },
            InstanceChoice {
                agent: Agent::Omp,
                provider: Provider::OpenCode,
                account: Some("acct-opencode-go".into()),
                model: "omp-default".into(),
            },
        ],
        task: None,
    };
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = SimBackend::hard_cases();
    world.op.set_session(OpSession::Locked);
    let backend = SimBackend::hard_cases();
    let mut feed = backend.launch(&world, &target);
    let credential_error = (0..1_000)
        .flat_map(|_| feed.poll(&world))
        .find(|event| matches!(event, LaunchEvent::CredentialError { .. }));
    assert!(credential_error.is_some());
    let typed = feed
        .use_plain_text(PlainKey::new("sk-fixture").expect("fixture key"))
        .expect("one-off key resumes the launch");
    assert!(!format!("{typed:?}").contains("sk-fixture"));
    let instance = (0..1_000)
        .flat_map(|_| feed.poll(&world))
        .find_map(|event| match event {
            LaunchEvent::Ready(ready) => Some(
                ready
                    .instance
                    .as_available()
                    .expect("sim launch has verified metadata")
                    .clone(),
            ),
            _ => None,
        })
        .expect("one-off plaintext launch completes");

    assert_eq!(instance.account, None);
    assert_eq!(instance.agents[0].account, None);
    assert_eq!(
        instance.agents[1].account.as_deref(),
        Some("acct-opencode-go")
    );
    assert!(!instance.accounts.contains(&"acct-codex-primary".into()));
    assert!(instance.accounts.contains(&"acct-opencode-go".into()));
    let panes = backend.initial_panes(&instance);
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].account(), None);
    assert_eq!(
        panes[1].account().map(String::as_str),
        Some("acct-opencode-go")
    );
    assert!(!format!("{instance:?}{panes:?}").contains("sk-fixture"));

    let mut inspect_world = world_for(Scenario::Returning);
    inspect_world.instances = vec![InstanceEntry::available(instance.clone())];
    inspect_world.daemons.clear();
    let mut inspect_app = app_with(Scenario::Returning, inspect_world);
    press(&mut inspect_app, "*");
    click_text(&mut inspect_app, &instance.id.short_suffix());
    press(&mut inspect_app, "i");
    let inspected_plain_text = screen(&mut inspect_app);
    assert!(
        inspected_plain_text.contains("no registered account used"),
        "{inspected_plain_text}"
    );
    assert!(!inspected_plain_text.contains("OpenAI · Primary"));

    let mut cancel_world = world_for(Scenario::LaunchRunning);
    cancel_world.backend = SimBackend::hard_cases();
    cancel_world.op.set_session(OpSession::Locked);
    let mut cancelled = backend.launch(&cancel_world, &target);
    let _ = (0..1_000)
        .flat_map(|_| cancelled.poll(&cancel_world))
        .find(|event| matches!(event, LaunchEvent::CredentialError { .. }));
    let _ = cancelled
        .use_plain_text(PlainKey::new("sk-cancel-fixture").expect("fixture key"))
        .expect("one-off key resumes the launch");
    let cancelled = cancelled.cancel(&cancel_world).expect("cancelled record");
    let cancelled = cancelled
        .as_available()
        .expect("sim launch cancellation keeps verified metadata");
    assert_eq!(cancelled.account, None);
    assert_eq!(cancelled.agents[0].account, None);
    assert_eq!(
        cancelled.agents[1].account.as_deref(),
        Some("acct-opencode-go")
    );
    assert!(!cancelled.accounts.contains(&"acct-codex-primary".into()));
    assert!(cancelled.accounts.contains(&"acct-opencode-go".into()));
    assert!(!format!("{cancelled:?}").contains("sk-cancel-fixture"));

    let mut retry_world = world_for(Scenario::LaunchRunning);
    retry_world.op.set_session(OpSession::Locked);
    let mut retry = backend.launch(&retry_world, &target);
    let _ = (0..1_000)
        .flat_map(|_| retry.poll(&retry_world))
        .find(|event| matches!(event, LaunchEvent::CredentialError { .. }));
    retry.retry_credentials(retry_world.op.as_mut());
    let retried_instance = (0..1_000)
        .flat_map(|_| retry.poll(&retry_world))
        .find_map(|event| match event {
            LaunchEvent::Ready(ready) => Some(
                ready
                    .instance
                    .as_available()
                    .expect("sim launch has verified metadata")
                    .clone(),
            ),
            _ => None,
        })
        .expect("registered credential retry completes");
    assert_eq!(
        retried_instance.account.as_deref(),
        Some("acct-codex-primary")
    );
    assert_eq!(
        retried_instance.agents[0].account.as_deref(),
        Some("acct-codex-primary")
    );
    assert!(
        retried_instance
            .accounts
            .contains(&"acct-codex-primary".into())
    );
    let mut retry_inspect_world = world_for(Scenario::Returning);
    let registered_title = retry_inspect_world
        .accounts
        .get("acct-codex-primary")
        .expect("the registered control account")
        .title();
    retry_inspect_world.instances = vec![InstanceEntry::available(retried_instance.clone())];
    retry_inspect_world.daemons.clear();
    let mut retry_inspect_app = app_with(Scenario::Returning, retry_inspect_world);
    press(&mut retry_inspect_app, "*");
    click_text(&mut retry_inspect_app, &retried_instance.id.short_suffix());
    press(&mut retry_inspect_app, "i");
    let inspected_registered = screen(&mut retry_inspect_app);
    assert!(
        inspected_registered.contains(&registered_title),
        "{inspected_registered}"
    );
    let registered_panes = backend.initial_panes(&retried_instance);
    assert_eq!(
        registered_panes[0].account().map(String::as_str),
        Some("acct-codex-primary")
    );
}

#[test]
fn the_sim_retry_unlocks_the_session_and_resumes() {
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = SimBackend::hard_cases();
    world.op.set_session(OpSession::Locked);
    let mut app = started(world);
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    assert_eq!(app.world.op.session(), OpSession::Locked);
    assert!(screen(&mut app).contains("1Password locked: unlock the app and retry"));

    press(&mut app, "enter");
    assert_eq!(app.world.op.session(), OpSession::SignedIn);
    assert_eq!(
        status(&app),
        "1Password unlocked · resolving the reference again"
    );
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert_eq!(
        app.world
            .instances
            .last()
            .map(available)
            .map(|i| i.run_id.as_str()),
        Some("run-202609030914-b5df")
    );
}

#[test]
fn the_sim_cancel_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchRunning));
    tick(&mut app, 10);
    press(&mut app, "x");
    press(&mut app, "right enter");
    let record = available(app.world.instances.last().expect("a record"));
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401640 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(record.daemon, DaemonSnapshot::Unavailable);
    assert_eq!(app.route(), Route::Manager);
    assert!(!app.world.daemons.contains_key(&iid("jk-b5df")));
}

#[test]
fn the_sim_abort_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchRunning));
    tick(&mut app, 10);
    press(&mut app, "ctrl+c");
    let record = available(app.world.instances.last().expect("a record"));
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401640 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn the_sim_failure_records_a_failed_setup() {
    let mut app = started(world_for(Scenario::LaunchFailure));
    tick_until(
        &mut app,
        "the failure",
        shows("The Construct network could not be attached"),
    );
    let record = available(app.world.instances.last().expect("a record"));
    assert_eq!(
        facts(record),
        "jk-b5df | jackin-payments-platform | /workspace/payments-platform | FailedSetup | 1788401642 \
         | run-202609030914-b5df | chainargos/payments-platform | feature/settlement-backoff"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

// The backend's side of a launch: a feed the test scripts, in place of the
// simulator's. Everything else a backend answers goes to the simulator.

#[derive(Debug, Default)]
struct Calls {
    retries: usize,
    plain_text: Vec<String>,
    cancels: usize,
    initial_panes: Vec<String>,
    answers: Vec<Vec<EnvAnswer>>,
}

#[derive(Debug)]
struct ScriptedFeed {
    run_id: String,
    batches: VecDeque<Vec<LaunchEvent>>,
    held: bool,
    calls: Arc<Mutex<Calls>>,
    cancel_record: Option<Instance>,
    collector: Option<String>,
    typed: Option<TypedKey>,
}

impl LaunchFeed for ScriptedFeed {
    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn telemetry(&self) -> Option<&str> {
        self.collector.as_deref()
    }

    fn poll(&mut self, _: &World) -> Vec<LaunchEvent> {
        if self.held {
            return vec![];
        }
        let batch = self.batches.pop_front().unwrap_or_default();
        self.held = batch.iter().any(|e| {
            matches!(
                e,
                LaunchEvent::CredentialError { .. } | LaunchEvent::NeedsInput(_)
            )
        });
        batch
    }

    fn answer_env(&mut self, answers: Vec<EnvAnswer>) {
        self.calls.lock().expect("calls").answers.push(answers);
        self.held = false;
    }

    fn retry_credentials(&mut self, _: &mut dyn OnePassword) {
        self.calls.lock().expect("calls").retries += 1;
        self.held = false;
    }

    fn use_plain_text(&mut self, key: PlainKey) -> Option<TypedKey> {
        self.calls
            .lock()
            .expect("calls")
            .plain_text
            .push(key.expose().to_owned());
        self.held = false;
        self.typed.clone()
    }

    fn cancel(&mut self, _: &World) -> Option<InstanceEntry> {
        self.calls.lock().expect("calls").cancels += 1;
        self.cancel_record.take().map(InstanceEntry::available)
    }
}

#[derive(Debug)]
struct FeedBackend {
    inner: Arc<dyn Backend>,
    feed: Mutex<Option<ScriptedFeed>>,
    calls: Arc<Mutex<Calls>>,
    with_shell: bool,
}

impl Backend for FeedBackend {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.inner.perform(effect, world)
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, _: &World, _: &LaunchTarget) -> Box<dyn LaunchFeed> {
        Box::new(self.feed.lock().expect("feed").take().expect("one launch"))
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.calls
            .lock()
            .expect("calls")
            .initial_panes
            .push(instance.id.to_string());
        let mut panes = self.inner.initial_panes(instance);
        if self.with_shell {
            panes.push(PaneRequest::new(&instance.id, None, None));
        }
        panes
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

const SANDBOX: &str = "/sandbox/project";

/// What the backend says it launched: nothing the cockpit could have made up
/// (its container is not `jackin-<workspace>`, its working directory is not
/// `/workspace/<workspace>`, it has no repository although the workspace name
/// matches one in the world).
fn launched(id: &str, container: &str, status: InstanceStatus) -> Instance {
    Instance {
        id: iid(id),
        container: container.into(),
        workspace: Some(1),
        workdir: Some(SANDBOX.into()),
        role: Some("the-architect".into()),
        agent: Agent::ClaudeCode,
        status,
        created_secs: 1_700_000_000,
        last_seen_secs: 1_700_000_005,
        run_id: "R1".into(),
        sessions: Ok(vec![]),
        daemon: DaemonSnapshot::Unavailable,
        branch: None,
        pr: None,
        default_branch: "trunk".into(),
        github: None,
        uncommitted: 0,
        unpushed: 0,
        account: Some(WORK.into()),
        accounts: vec![WORK.into()],
        agents: Vec::new(),
        task: None,
    }
}

fn ready(instance: Instance) -> LaunchEvent {
    LaunchEvent::Ready(Box::new(ReadyInstance {
        instance: InstanceEntry::available(instance),
        run_id: "R1".into(),
        warning: None,
    }))
}

fn ready_unsynced(instance: Instance) -> LaunchEvent {
    LaunchEvent::Ready(Box::new(ReadyInstance {
        instance: InstanceEntry::available(instance),
        run_id: "R1".into(),
        warning: Some(SaveWarning::DirectorySync),
    }))
}

fn failed(instance: Instance) -> LaunchEvent {
    failed_with(Some(instance))
}

fn failed_with(instance: Option<Instance>) -> LaunchEvent {
    LaunchEvent::Failed(Box::new(FailedLaunch {
        failure: LaunchFailure {
            stage: 3,
            summary: "The sandbox could not be created".into(),
            next_step: NextStep::RunDoctor,
            detail: vec!["gateway: jackin-local".into()],
        },
        instance: instance.map(InstanceEntry::available),
    }))
}

fn running(stage: usize) -> Vec<LaunchEvent> {
    vec![LaunchEvent::StageChanged(stage, StepState::Running)]
}

fn scripted(
    batches: Vec<Vec<LaunchEvent>>,
    cancel_record: Option<Instance>,
) -> (App, Arc<Mutex<Calls>>) {
    scripted_panes(batches, cancel_record, false)
}

fn scripted_panes(
    batches: Vec<Vec<LaunchEvent>>,
    cancel_record: Option<Instance>,
    with_shell: bool,
) -> (App, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let feed = ScriptedFeed {
        run_id: "R-7".into(),
        batches: batches.into(),
        held: false,
        calls: calls.clone(),
        cancel_record,
        collector: None,
        typed: None,
    };
    (feed_app(feed, &calls, with_shell, Motion::Reduced), calls)
}

fn feed_app(
    feed: ScriptedFeed,
    calls: &Arc<Mutex<Calls>>,
    with_shell: bool,
    motion: Motion,
) -> App {
    let mut world = world_for(Scenario::LaunchRunning);
    world.backend = Arc::new(FeedBackend {
        inner: SimBackend::shared(),
        calls: calls.clone(),
        with_shell,
        feed: Mutex::new(Some(feed)),
    });
    world.op.set_session(OpSession::Locked);
    app_with_world(
        Scenario::LaunchRunning,
        world,
        motion,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

#[test]
fn ready_event_inserts_the_backend_instance() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, _) = scripted(vec![running(0), vec![ready(mine.clone())]], None);
    let others = app.world.instances.len();
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);

    assert_eq!(app.world.instances.len(), others + 1);
    let mut stored = app
        .world
        .instance(&iid("jk-5b0x"))
        .expect("the instance")
        .clone();
    stored.daemon = mine.daemon.clone();
    stored.last_seen_secs = mine.last_seen_secs;
    assert_eq!(
        stored, mine,
        "the cockpit stores the backend's record as it is"
    );
    assert_eq!(app.world.running_count(), 3);
    assert!(!status(&app).contains("durability uncertain"));

    let shown = screen(&mut app);
    assert!(shown.contains("sbx-alpha-5b0x"), "{shown}");
    assert!(!shown.contains("jackin-payments-pl"), "{shown}");

    let daemon = app
        .world
        .daemons
        .get(&iid("jk-5b0x"))
        .expect("the attach builds it");
    assert_eq!(daemon.workspace, "payments-platform");
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    assert_eq!(pane.agent, Some(Agent::ClaudeCode));
    assert_eq!(pane.account.as_deref(), Some(WORK));
}

#[test]
fn a_committed_launch_warning_is_carried_through_ready_and_survives_routine_status() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let mut batch = running(0);
    batch.push(ready_unsynced(mine));
    let (mut app, _) = scripted(vec![batch], None);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);

    assert_eq!(status(&app), "Saved, durability uncertain");
    assert_eq!(
        app.world.committed_save_warning,
        Some(SaveWarning::DirectorySync)
    );
    app.set_status(
        "Routine refresh complete",
        junie_tui::theme::Tone::Secondary,
    );
    assert_eq!(status(&app), "Saved, durability uncertain");
}

#[test]
fn a_ready_warning_from_a_later_poll_survives_handoff() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, _) = scripted(vec![running(0), vec![ready_unsynced(mine)]], None);
    assert!(!status(&app).contains("durability uncertain"));

    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert_eq!(status(&app), "Saved, durability uncertain");
    assert_eq!(
        app.world.committed_save_warning,
        Some(SaveWarning::DirectorySync)
    );
}

#[test]
fn daemon_is_built_by_attach() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, calls) = scripted_panes(vec![running(0), vec![ready(mine)]], None, true);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);

    assert_eq!(
        calls.lock().expect("calls").initial_panes,
        ["jk-5b0x"],
        "the attach asks the backend once"
    );
    let daemon = app
        .world
        .daemons
        .get(&iid("jk-5b0x"))
        .expect("the attach builds it");
    assert_eq!(daemon.tabs.len(), 2, "one tab per request the backend made");
    let [agent, shell] = daemon.panes.as_slice() else {
        panic!("two panes expected");
    };
    assert_eq!(agent.agent, Some(Agent::ClaudeCode));
    assert_eq!(
        agent.account.as_deref(),
        Some(WORK),
        "the launch's account reaches the agent pane"
    );
    assert_eq!(shell.agent, None);
}

const PRIMARY: &str = "acct-codex-primary";

fn agent_pane_account(record: Instance) -> Option<String> {
    let (mut app, _) = scripted(vec![running(0), vec![ready(record)]], None);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    let daemon = app.world.daemons.get(&iid("jk-5b0x")).expect("its daemon");
    let [pane] = daemon.panes.as_slice() else {
        panic!("one pane expected");
    };
    pane.account.clone()
}

#[test]
fn the_agent_pane_runs_with_the_account_of_the_record() {
    let mut mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    mine.account = Some(PRIMARY.into());
    assert_eq!(
        agent_pane_account(mine).as_deref(),
        Some(PRIMARY),
        "the cockpit launched with {WORK}; the record says otherwise"
    );
}

#[test]
fn a_record_without_an_account_gets_a_pane_without_one() {
    let mut mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    mine.account = None;
    assert_eq!(
        agent_pane_account(mine),
        None,
        "the cockpit must not fill in the account of its own launch"
    );
}

#[test]
fn the_cockpit_builds_no_daemon() {
    let gone = launched("jk-5b0x", "sbx-alpha", InstanceStatus::CleanExited);
    let (mut app, _) = scripted(vec![running(0), vec![ready(gone)]], None);
    tick(&mut app, 4);

    assert_eq!(app.route(), Route::Cockpit);
    assert!(app.world.instance(&iid("jk-5b0x")).is_some());
    assert!(
        !app.world.daemons.contains_key(&iid("jk-5b0x")),
        "only the attach builds a daemon, and none was tried"
    );
}

#[test]
fn failed_event_builds_from_the_event() {
    let mine = launched("jk-fa11", "sbx-failed", InstanceStatus::FailedSetup);
    let (mut app, _) = scripted(vec![running(0), vec![failed(mine.clone())]], None);
    let others = app.world.instances.len();
    tick_until(
        &mut app,
        "the failure",
        shows("The sandbox could not be created"),
    );

    assert_eq!(app.world.instances.len(), others + 1);
    assert_eq!(app.world.instance(&iid("jk-fa11")), Some(&mine));
    let shown = screen(&mut app);
    assert!(
        shown.contains("R-7"),
        "the run id comes from the feed:\n{shown}"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn a_failed_event_without_a_record_adds_no_instance() {
    let (mut app, _) = scripted(vec![running(0), vec![failed_with(None)]], None);
    let others = app.world.instances.len();
    tick_until(
        &mut app,
        "the failure",
        shows("The sandbox could not be created"),
    );

    assert_eq!(
        app.world.instances.len(),
        others,
        "the backend kept no record, so the roster gets no row"
    );
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn run_id_comes_from_the_feed() {
    let (mut app, _) = scripted(vec![running(0)], None);
    tick(&mut app, 2);
    press(&mut app, "i");
    let shown = screen(&mut app);
    assert!(shown.contains("Debug info"), "{shown}");
    assert!(shown.contains("R-7"), "{shown}");
    assert!(!shown.contains("run-2026"), "{shown}");
}

#[test]
fn retry_credentials() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    assert!(screen(&mut app).contains("The vault is sealed"));

    press(&mut app, "enter");
    assert_eq!(calls.lock().expect("calls").retries, 1);
    assert_eq!(
        calls.lock().expect("calls").plain_text,
        Vec::<String>::new()
    );
    assert_eq!(
        app.world.op.session(),
        OpSession::Locked,
        "the screen does not unlock the session; the backend does"
    );
    assert!(!screen(&mut app).contains("Resolved by the backend"));
    tick_until(&mut app, "the next event", shows("Resolved by the backend"));
}

#[test]
fn plain_text_resumes_the_feed() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter");
    assert_eq!(
        calls.lock().expect("calls").plain_text,
        Vec::<String>::new(),
        "the feed hears nothing before a key is typed"
    );
    assert!(screen(&mut app).contains("Plain-text key"));
    press(&mut app, "s k - t e s t enter");
    assert_eq!(calls.lock().expect("calls").retries, 0);
    assert_eq!(calls.lock().expect("calls").plain_text, ["sk-test"]);
    assert_eq!(app.world.op.session(), OpSession::Locked);
    tick_until(&mut app, "the next event", shows("Resolved by the backend"));
}

#[test]
fn cancel_keeps_the_record_the_feed_returns() {
    let mine = launched("jk-ca9c", "sbx-cancelled", InstanceStatus::FailedSetup);
    let (mut app, calls) = scripted(vec![running(0)], Some(mine.clone()));
    tick(&mut app, 2);
    let others = app.world.instances.len();
    press(&mut app, "x");
    press(&mut app, "right enter");

    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(app.world.instances.len(), others + 1);
    assert_eq!(app.world.instance(&iid("jk-ca9c")), Some(&mine));
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn ctrl_c_keeps_the_record_the_feed_returns() {
    let mine = launched("jk-ca9c", "sbx-cancelled", InstanceStatus::FailedSetup);
    let (mut app, calls) = scripted(vec![running(0)], Some(mine.clone()));
    tick(&mut app, 2);
    press(&mut app, "ctrl+c");

    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(app.world.instance(&iid("jk-ca9c")), Some(&mine));
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn a_cancel_without_a_record_adds_none() {
    let (mut app, calls) = scripted(vec![running(0)], None);
    tick(&mut app, 2);
    let before = app.world.instances.clone();
    press(&mut app, "x");
    press(&mut app, "right enter");

    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(
        app.world.instances, before,
        "nothing is made up for a launch the backend never recorded"
    );
    assert_eq!(app.route(), Route::Manager);
}

fn timing_on(shown: &str) -> Option<String> {
    shown.lines().find_map(|line| {
        let cells: Vec<char> = line.chars().collect();
        cells.windows(5).find_map(|w| {
            (w[0].is_ascii_digit()
                && w[1] == '.'
                && w[2].is_ascii_digit()
                && w[3] == ' '
                && w[4] == 's')
                .then(|| w.iter().collect())
        })
    })
}

#[test]
fn a_feed_that_gives_no_timing_draws_none() {
    let (mut app, _) = scripted(
        vec![
            running(0),
            vec![
                LaunchEvent::StageChanged(0, StepState::Done),
                LaunchEvent::StageChanged(1, StepState::Running),
            ],
        ],
        None,
    );
    tick_until(&mut app, "the second stage", shows("1 done"));
    tick(&mut app, 3);
    let shown = screen(&mut app);
    assert_eq!(
        timing_on(&shown),
        None,
        "no duration is made up for a finished stage:\n{shown}"
    );
}

#[test]
fn a_feed_that_names_no_collector_shows_no_telemetry() {
    let (mut app, _) = scripted(vec![running(0)], None);
    tick(&mut app, 2);
    press(&mut app, "t");
    press(&mut app, "i");
    let shown = screen(&mut app);
    assert!(shown.contains("Debug info"), "{shown}");
    assert!(shown.contains("R-7"), "{shown}");
    assert!(!shown.contains("Telemetry"), "{shown}");
    assert!(!shown.contains("otlp"), "{shown}");
}

#[test]
fn a_feed_that_shows_no_key_leaves_the_credentials_line_alone() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter s k enter");
    assert_eq!(calls.lock().expect("calls").plain_text, ["sk"]);
    let shown = screen(&mut app);
    assert!(!shown.contains("masked API key"), "{shown}");
    assert!(!shown.contains("••••"), "{shown}");
    assert!(!shown.contains("typed once"), "{shown}");
}

#[test]
fn a_reply_due_on_the_handoff_is_delivered() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let calls = Arc::new(Mutex::new(Calls::default()));
    let feed = ScriptedFeed {
        run_id: "R-7".into(),
        batches: vec![running(0), vec![ready(mine)]].into(),
        held: false,
        calls: calls.clone(),
        cancel_record: None,
        collector: None,
        typed: None,
    };
    let mut app = feed_app(feed, &calls, false, Motion::Full);
    tick_until(&mut app, "the handoff", |a| a.route() == Route::Handoff);

    let mut snapshot = jackin_console::sim::snapshot(&app.world);
    snapshot.workspaces.truncate(1);
    let reply = roster_reply(&mut app, snapshot);
    app.world.schedule(0, reply);
    tick(&mut app, 1);
    assert_eq!(
        app.world.workspaces.len(),
        1,
        "the reply that fell due on the handoff was dropped"
    );
}

fn feed_with(batches: Vec<Vec<LaunchEvent>>) -> (ScriptedFeed, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let feed = ScriptedFeed {
        run_id: "R-7".into(),
        batches: batches.into(),
        held: false,
        calls: calls.clone(),
        cancel_record: None,
        collector: None,
        typed: None,
    };
    (feed, calls)
}

#[test]
fn a_timing_from_the_feed_is_drawn_for_its_stage() {
    let (feed, calls) = feed_with(vec![
        running(0),
        vec![
            LaunchEvent::StageChanged(0, StepState::Done),
            LaunchEvent::StageTimed {
                stage: 0,
                elapsed_ms: 40_000,
            },
            LaunchEvent::StageChanged(1, StepState::Running),
        ],
    ]);
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick_until(&mut app, "the timing", shows("40.0 s"));
}

#[test]
fn a_collector_from_the_feed_is_shown_in_the_debug_info() {
    let (mut feed, calls) = feed_with(vec![running(0)]);
    feed.collector = Some("otlp-test.invalid:1".into());
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick(&mut app, 2);
    press(&mut app, "t");
    press(&mut app, "i");
    let shown = screen(&mut app);
    assert!(shown.contains("run R-7 -> otlp-test.invalid:1"), "{shown}");
}

#[test]
fn the_key_line_of_the_feed_is_what_the_credentials_line_shows() {
    let (mut feed, calls) = feed_with(vec![
        running(2),
        vec![LaunchEvent::CredentialError {
            message: "The vault is sealed".into(),
        }],
    ]);
    feed.typed = Some(TypedKey {
        origin: "typed by the operator".into(),
        validation: "kept by the backend".into(),
    });
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter s k enter");
    let shown = screen(&mut app);
    assert!(shown.contains("typed by the operator"), "{shown}");
    assert!(shown.contains("kept by the backend"), "{shown}");
}

fn refused() -> LaunchEvent {
    LaunchEvent::Refused(RefusalReason::RoleNotAllowed {
        workspace: "payments-platform".into(),
        role: "the-architect".into(),
    })
}

#[test]
fn a_refusal_from_the_feed_is_drawn_in_the_launch_words() {
    let (mut app, calls) = scripted(vec![vec![refused()]], None);
    let others = app.world.instances.clone();
    tick_until(&mut app, "the refusal", shows("Role not allowed"));
    let shown = screen(&mut app);
    for text in [
        "Workspace payments-platform does not allow role the-architect.",
        "nothing was created",
        "Allow it in the workspace's Roles tab, or load another role.",
    ] {
        assert!(shown.contains(text), "{text:?} missing:\n{shown}");
    }
    assert_eq!(app.world.instances, others, "a refusal creates nothing");
    assert_eq!(calls.lock().expect("calls").cancels, 0);
    assert!(
        !shown.contains("Launch failed"),
        "a refusal is not a failed stage:\n{shown}"
    );

    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances, others);
    assert_eq!(
        calls.lock().expect("calls").cancels,
        0,
        "there is nothing to cancel after a refusal"
    );
}

fn question(name: &str, prompt: &str, default: Option<&str>) -> EnvQuestion {
    EnvQuestion {
        name: name.into(),
        prompt: prompt.into(),
        default: default.map(Into::into),
    }
}

fn asks() -> LaunchEvent {
    LaunchEvent::NeedsInput(vec![
        question("FIRST_VALUE", "Name the first thing", Some("kept-default")),
        question("SECOND_VALUE", "Name the second thing", None),
    ])
}

fn answer(name: &str, value: &str) -> EnvAnswer {
    EnvAnswer {
        name: name.into(),
        value: value.into(),
    }
}

#[test]
fn the_feeds_questions_open_a_form_and_the_answers_go_back() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, calls) = scripted(vec![running(0), vec![asks()], vec![ready(mine)]], None);
    tick_until(&mut app, "the questions", shows("Name the first thing"));
    let shown = screen(&mut app);
    for text in [
        "Name the second thing",
        "kept-default",
        "FIRST_VALUE",
        "SECOND_VALUE",
    ] {
        assert!(shown.contains(text), "{text:?} missing:\n{shown}");
    }
    tick(&mut app, 5);
    assert_eq!(app.route(), Route::Cockpit, "the launch waits for the form");
    assert!(calls.lock().expect("calls").answers.is_empty());

    press(&mut app, "tab enter");
    for key in ["h", "e", "l", "l", "o"] {
        press(&mut app, key);
    }
    press(&mut app, "enter");
    click_text(&mut app, "Start launch");
    assert_eq!(
        calls.lock().expect("calls").answers,
        [[
            answer("FIRST_VALUE", "kept-default"),
            answer("SECOND_VALUE", "hello")
        ]],
        "one answer for each question, in the feed's order"
    );
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert_eq!(calls.lock().expect("calls").cancels, 0);
}

#[test]
fn an_entry_without_a_default_cannot_start_empty() {
    let mine = launched("jk-5b0y", "sbx-beta", InstanceStatus::Running);
    let (mut app, calls) = scripted(vec![running(0), vec![asks()], vec![ready(mine)]], None);
    tick_until(&mut app, "the questions", shows("Name the first thing"));
    let shown = screen(&mut app);
    let row = |needle: &str| {
        shown
            .lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} missing:\n{shown}"))
            .to_owned()
    };
    assert!(row("Name the first thing").contains("optional"), "{shown}");
    assert!(
        row("Name the second thing").contains('*')
            && !row("Name the second thing").contains("optional"),
        "an entry with no default is marked required:\n{shown}"
    );

    click_text(&mut app, "Start launch");
    let refused = screen(&mut app);
    assert!(
        refused.contains("Enter a value for SECOND_VALUE."),
        "{refused}"
    );
    assert!(
        refused.contains("Values for this launch"),
        "the form stays open"
    );
    assert!(
        calls.lock().expect("calls").answers.is_empty(),
        "nothing is sent for an empty required value"
    );

    press(&mut app, "h i enter");
    click_text(&mut app, "Start launch");
    assert_eq!(
        calls.lock().expect("calls").answers,
        [[
            answer("FIRST_VALUE", "kept-default"),
            answer("SECOND_VALUE", "hi")
        ]],
    );
}

#[test]
fn escape_on_the_questions_cancels_the_launch_and_creates_nothing() {
    let (mut app, calls) = scripted(vec![running(0), vec![asks()]], None);
    tick_until(&mut app, "the questions", shows("Name the first thing"));
    let others = app.world.instances.clone();
    press(&mut app, "esc");

    let calls = calls.lock().expect("calls");
    assert!(calls.answers.is_empty(), "nothing is sent on Escape");
    assert_eq!(calls.cancels, 1, "the backend is told the launch ended");
    assert_eq!(app.world.instances, others);
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn a_blocked_stage_keeps_the_cancel_it_offers() {
    let mine = launched("jk-b10c", "sbx-blocked", InstanceStatus::FailedSetup);
    let (mut app, calls) = scripted(
        vec![
            running(1),
            vec![LaunchEvent::StageChanged(1, StepState::Blocked)],
        ],
        Some(mine.clone()),
    );
    tick_until(&mut app, "the blocked stage", shows("blocked"));
    let shown = screen(&mut app);
    assert!(shown.contains("x Cancel launch…"), "no hint\n{shown}");

    let others = app.world.instances.len();
    press(&mut app, "x");
    assert!(screen(&mut app).contains("Cancel the launch?"));
    press(&mut app, "right enter");
    assert_eq!(
        calls.lock().expect("calls").cancels,
        1,
        "x reaches the feed's cancel while a stage is blocked"
    );
    assert_eq!(app.world.instances.len(), others + 1);
    assert_eq!(app.world.instance(&iid("jk-b10c")), Some(&mine));
    assert_eq!(app.route(), Route::Manager);
}

#[test]
fn backing_out_of_the_plain_text_key_returns_to_the_credential_choice() {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
            vec![LaunchEvent::Activity("Resolved by the backend".into())],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter s k esc");
    let shown = screen(&mut app);
    assert!(shown.contains("Credential source unavailable"), "{shown}");
    assert!(shown.contains("The vault is sealed"), "{shown}");
    assert_eq!(
        calls.lock().expect("calls").plain_text,
        Vec::<String>::new()
    );
}

#[test]
fn the_origin_and_validation_a_feed_resolves_are_what_the_credentials_line_shows() {
    let (mut app, _) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialsResolved {
                origin: "vault ops/codex".into(),
                validation: "verified by the backend".into(),
            }],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the resolved line",
        shows("verified by the backend"),
    );
    let shown = screen(&mut app);
    assert!(shown.contains("vault ops/codex"), "{shown}");
}

#[test]
fn a_ready_instance_that_is_not_running_fails_the_launch_with_its_status() {
    let stopped = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Crashed);
    let (mut app, _) = scripted(vec![running(0), vec![ready(stopped)]], None);
    tick_until(
        &mut app,
        "the failure",
        shows("The container is crashed, not running"),
    );

    assert_eq!(app.route(), Route::Cockpit);
    assert!(
        app.world.instance(&iid("jk-5b0x")).is_some(),
        "the record the backend made is kept"
    );
    assert!(screen(&mut app).contains("Run id     R1"));
    press(&mut app, "esc x");
    assert!(
        !screen(&mut app).contains("Cancel the launch?"),
        "an ended launch has nothing left to cancel"
    );
}

fn cells(app: &mut App) -> Vec<String> {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol().to_owned())
        .collect()
}

fn hidden_cells(app: &mut App) -> Vec<String> {
    cells(app)
        .into_iter()
        .filter(|cell| cell.chars().any(jackin_console::domain::shown::is_hidden))
        .collect()
}

#[test]
fn what_the_feed_resolves_cannot_hide_text_and_leaves_the_origin_the_console_derived() {
    let (mut app, _) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialsResolved {
                origin: "vault \u{202e}ops\u{3164}/codex\x1b[31m".into(),
                validation: "ok \u{2066}verified\x07\u{202c}\nline\u{2800}2\r".into(),
            }],
        ],
        None,
    );
    tick_until(&mut app, "the resolved line", shows("verified"));
    let shown = screen(&mut app);
    assert!(
        shown.contains("credentials  5 accounts"),
        "the console's own origin stays:\n{shown}"
    );
    assert!(
        shown.contains("vault ops/codex[31m · ok verified line2"),
        "{shown}"
    );
    assert_eq!(hidden_cells(&mut app), Vec::<String>::new());
}

#[test]
fn what_the_feed_says_of_a_typed_key_cannot_hide_text_either() {
    let (mut feed, calls) = feed_with(vec![
        running(2),
        vec![LaunchEvent::CredentialError {
            message: "The vault is sealed".into(),
        }],
    ]);
    feed.typed = Some(TypedKey {
        origin: "plain\u{202e} te\u{3164}xt".into(),
        validation: "kept\u{2066} by\u{2800} the backend\x1b[2J".into(),
    });
    let mut app = feed_app(feed, &calls, false, Motion::Reduced);
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter s k enter");
    let shown = screen(&mut app);
    assert!(shown.contains("credentials  typed key"), "{shown}");
    assert!(
        shown.contains("plain text · kept by the backend[2J"),
        "{shown}"
    );
    assert_eq!(hidden_cells(&mut app), Vec::<String>::new());
}

#[test]
fn a_typed_key_of_blanks_or_with_a_direction_mark_asks_again_and_reaches_nobody() {
    let (mut app, calls) = to_prompt();
    press(&mut app, "space space enter");
    assert_eq!(
        calls.lock().expect("calls").plain_text,
        Vec::<String>::new()
    );
    assert!(screen(&mut app).contains("Plain-text key"));
    assert_eq!(status(&app), "API key required: the field is empty");

    Application::handle(&mut app, Input::Paste("sk-\u{202e}abc".into()));
    press(&mut app, "enter");
    assert_eq!(
        calls.lock().expect("calls").plain_text,
        Vec::<String>::new()
    );
    assert!(screen(&mut app).contains("Plain-text key"));
    assert_eq!(
        status(&app),
        "The API key holds a control or direction character"
    );

    press(&mut app, "ctrl+a");
    Application::handle(&mut app, Input::Paste("  sk-abc \n".into()));
    press(&mut app, "enter");
    assert_eq!(calls.lock().expect("calls").plain_text, ["sk-abc"]);
}

fn to_prompt() -> (App, Arc<Mutex<Calls>>) {
    let (mut app, calls) = scripted(
        vec![
            running(2),
            vec![LaunchEvent::CredentialError {
                message: "The vault is sealed".into(),
            }],
        ],
        None,
    );
    tick_until(
        &mut app,
        "the credential dialog",
        shows("Credential source unavailable"),
    );
    press(&mut app, "right enter");
    assert!(screen(&mut app).contains("Plain-text key"));
    (app, calls)
}

/// The roster a refresh started now would answer with, and the ticket the
/// world waits on, with the answer held back.
fn roster_out(app: &mut App) -> Msg {
    let snapshot = sim::snapshot(&app.world);
    let ticket = hold_roster_refresh(app);
    Msg::Refreshed {
        ticket,
        result: Ok(Box::new(snapshot)),
    }
}

#[test]
fn refresh_started_before_launch_ready_cannot_remove_the_admitted_instance() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, _) = scripted(vec![running(0), vec![ready(mine)]], None);
    let held = roster_out(&mut app);
    tick_until(&mut app, "the capsule", |a| a.route() == Route::Capsule);
    assert!(app.world.instance(&iid("jk-5b0x")).is_some());

    assert_eq!(app.deliver(&held), Outcome::Ignored);
    assert_eq!(app.route(), Route::Capsule, "the capsule stays open");
    assert!(app.world.instance(&iid("jk-5b0x")).is_some());
}

#[test]
fn refresh_started_before_launch_failed_cannot_remove_the_admitted_instance() {
    let mine = launched("jk-5b0x", "sbx-alpha", InstanceStatus::Running);
    let (mut app, _) = scripted(vec![running(0), vec![failed(mine)]], None);
    let held = roster_out(&mut app);
    tick_until(&mut app, "the admitted record", |a| {
        a.world.instance(&iid("jk-5b0x")).is_some()
    });
    let route = app.route();

    assert_eq!(app.deliver(&held), Outcome::Ignored);
    assert_eq!(app.route(), route);
    assert!(app.world.instance(&iid("jk-5b0x")).is_some());
}
