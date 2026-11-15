#![cfg(feature = "demo")]

use std::collections::BTreeSet;

use jackin_console::backend::{Effect, OpScope, Reply};
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::sim::SimBackend;
use jackin_console::world::{Msg, World};

fn name(effect: &Effect) -> &'static str {
    match effect {
        Effect::SaveWorkspace { .. } => "SaveWorkspace",
        Effect::SaveGlobal { .. } => "SaveGlobal",
        Effect::SaveAccounts { .. } => "SaveAccounts",
        Effect::DeleteWorkspace { .. } => "DeleteWorkspace",
        Effect::Restore { .. } => "Restore",
        Effect::PowerStart { .. } => "PowerStart",
        Effect::PowerStop { .. } => "PowerStop",
        Effect::TrustRole { .. } => "TrustRole",
        Effect::Prewarm { .. } => "Prewarm",
        Effect::Stop { .. } => "Stop",
        Effect::Purge { .. } => "Purge",
        Effect::Refresh { .. } => "Refresh",
        Effect::RefreshAccount { .. } => "RefreshAccount",
        Effect::ValidateAccount { .. } => "ValidateAccount",
        Effect::LoadRole { .. } => "LoadRole",
        Effect::CheckRole { .. } => "CheckRole",
        Effect::SaveRole { .. } => "SaveRole",
        Effect::BindRole { .. } => "BindRole",
        Effect::AddRole { .. } => "AddRole",
        Effect::RemoveRole { .. } => "RemoveRole",
        Effect::RunDoctor => "RunDoctor",
        Effect::CheckSource { .. } => "CheckSource",
        Effect::ProbeFolder { .. } => "ProbeFolder",
        Effect::ReadChanges { .. } => "ReadChanges",
        Effect::LoadOp { .. } => "LoadOp",
    }
}

const ALL: [&str; 25] = [
    "SaveWorkspace",
    "SaveGlobal",
    "SaveAccounts",
    "DeleteWorkspace",
    "Restore",
    "PowerStart",
    "PowerStop",
    "TrustRole",
    "Prewarm",
    "Stop",
    "Purge",
    "Refresh",
    "RefreshAccount",
    "ValidateAccount",
    "LoadRole",
    "CheckRole",
    "SaveRole",
    "BindRole",
    "AddRole",
    "RemoveRole",
    "RunDoctor",
    "CheckSource",
    "ProbeFolder",
    "ReadChanges",
    "LoadOp",
];

fn samples(world: &World) -> Vec<Effect> {
    let workspace = world.workspaces[0].clone();
    let account = world.accounts.accounts[0].clone();
    let instance = world.instances[0].id().clone();
    let doc = world
        .surfaces
        .roles
        .as_ref()
        .and_then(|admin| admin.roles.iter().find_map(|r| r.manifest.as_ref().ok()))
        .expect("a role manifest")
        .clone();
    vec![
        Effect::SaveWorkspace {
            id: Some(workspace.id),
            base: Some(Box::new(workspace.clone())),
            workspace: Box::new(workspace.clone()),
        },
        Effect::SaveGlobal {
            base: Box::new(world.global.clone()),
            global: Box::new(world.global.clone()),
        },
        Effect::SaveAccounts {
            ticket: 1,
            base: Box::new(world.accounts.clone()),
            registry: Box::new(world.accounts.clone()),
        },
        Effect::DeleteWorkspace { id: workspace.id },
        Effect::Restore {
            instance: instance.clone(),
        },
        Effect::PowerStart {
            instance: instance.clone(),
        },
        Effect::PowerStop {
            instance: instance.clone(),
        },
        Effect::TrustRole {
            role: "writer".into(),
        },
        Effect::Prewarm {
            workspace: workspace.id,
        },
        Effect::Stop {
            instance: instance.clone(),
        },
        Effect::Purge {
            instance: instance.clone(),
        },
        Effect::Refresh { ticket: 1 },
        Effect::RefreshAccount {
            ticket: 1,
            account: account.id.clone(),
            source: account.source.clone(),
            stagger_ms: 0,
        },
        Effect::ValidateAccount {
            account: account.id.clone(),
            source: account.source.clone(),
        },
        Effect::LoadRole {
            role: "writer".into(),
        },
        Effect::CheckRole {
            ticket: 1,
            role: "writer".into(),
            doc: doc.clone(),
        },
        Effect::SaveRole {
            role: "writer".into(),
            base: Box::new(doc.clone()),
            doc: Box::new(doc),
        },
        Effect::BindRole {
            role: "writer".into(),
            agent: jackin_console::domain::agent::Agent::Codex,
            base: None,
            account: Some(account.id.clone()),
        },
        Effect::AddRole {
            location: "~/roles/new".into(),
        },
        Effect::RemoveRole {
            role: "writer".into(),
        },
        Effect::RunDoctor,
        Effect::CheckSource {
            ticket: 1,
            provider: account.provider,
            source: account.source.clone(),
            key: None,
        },
        Effect::ProbeFolder {
            ticket: 1,
            path: "/tmp".into(),
        },
        Effect::ReadChanges {
            ticket: 1,
            instance,
            touched: vec![],
            uncommitted: 0,
            unpushed: 0,
        },
        Effect::LoadOp {
            ticket: 1,
            scope: OpScope::Accounts,
        },
    ]
}

#[test]
fn every_effect_the_simulator_sets_aside_gets_a_reply_when_it_falls_due() {
    let world = world_for(Scenario::RoleBinding);
    let sim = SimBackend::shared();
    let effects = samples(&world);

    let named: BTreeSet<&str> = effects.iter().map(name).collect();
    assert_eq!(named, BTreeSet::from(ALL), "a sample for every effect");

    let mut silent = Vec::new();
    for effect in &effects {
        let completion = sim.perform(effect, &world).expect("the simulator answers");
        let replies = match completion.reply {
            Reply::Ready(_) => true,
            Reply::Settle => sim.settle(effect, &world).is_some(),
        };
        if !replies {
            silent.push(name(effect));
        }
    }
    assert_eq!(silent, Vec::<&str>::new());
}

#[test]
fn refresh_simulator_echoes_ticket() {
    let world = world_for(Scenario::RoleBinding);
    let sim = SimBackend::shared();
    let account = world.accounts.accounts.first().expect("an account");

    let ask = |ticket: u64, id: &str| Effect::RefreshAccount {
        ticket,
        account: id.to_owned(),
        source: account.source.clone(),
        stagger_ms: 0,
    };
    for ticket in [41, 42] {
        let Some(Msg::AccountRefreshed {
            ticket: echoed,
            account: id,
            source,
            result,
        }) = sim.settle(&ask(ticket, &account.id), &world)
        else {
            panic!("the simulator answers a refresh");
        };
        assert_eq!(
            (echoed, id, *source),
            (ticket, account.id.clone(), account.source.clone())
        );
        assert!(result.is_ok());
    }

    let Some(Msg::AccountRefreshed {
        ticket,
        account: id,
        result: Err(_),
        ..
    }) = sim.settle(&ask(43, "gone"), &world)
    else {
        panic!("a refresh of an account that is not there fails with its ticket");
    };
    assert_eq!((ticket, id.as_str()), (43, "gone"));
}
