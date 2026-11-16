use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use self::pane::ScriptPane;
use crate::backend::{Backend, Completion, Effect, OpSession, PlainKey, Ticket, Validation};
use crate::domain::account::{
    Account, AccountId, AccountOrigin, AccountRegistry, CredentialSource, ValidationLevel,
};
use crate::domain::instance::{Instance, InstanceEntry, InstanceId, InstanceStatus, SandboxPhase};
use crate::domain::launch::LaunchContent;
use crate::domain::role::{RoleCheckState, RoleRecord};
use crate::domain::tuple::{AccountRequirement, TupleIdentity};
use crate::domain::vocabulary::{Rejection, fill_in};
use crate::domain::workspace::{RoleEntry, RoleSource, Workspace, WorkspaceId};
use crate::launch::{LaunchFeed, LaunchTarget};
use crate::panes::{PaneBody, PaneRequest, PaneSpawn};
use crate::world::{GlobalConfig, Msg, SaveFailure, SaveSuccess, Snapshot, World};

use self::launch::{LaunchPlan, Opening, SimLaunch};

fn save_success(snapshot: Snapshot) -> SaveSuccess {
    SaveSuccess {
        snapshot: Box::new(snapshot),
        warning: None,
    }
}

pub mod changes;
pub mod launch;
pub mod link;
pub mod onepassword;
pub mod pane;
pub mod provider;
pub mod scripts;
pub mod surfaces;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum LaunchRule {
    #[default]
    Clean,
    FailNetwork,
    HardCases,
    Guarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshOverride {
    Healthy,
    BrokerDown,
}

#[derive(Debug)]
pub struct SimBackend {
    launch: LaunchRule,
    save_fails_once: AtomicBool,
    bind_fails_once: AtomicBool,
    doctor_runs: AtomicUsize,
    openshell: bool,
    accountless_tuple: Option<TupleIdentity>,
    next_workspace_id: AtomicU32,
    refresh_overrides: Vec<(AccountId, RefreshOverride)>,
    index_fails_once: AtomicBool,
    takeover: Mutex<Option<Takeover>>,
}

/// A second client that attaches at `at_ms` of world time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Takeover {
    pub at_ms: i64,
    pub by: String,
}

impl Default for SimBackend {
    fn default() -> Self {
        Self {
            launch: LaunchRule::default(),
            save_fails_once: AtomicBool::default(),
            bind_fails_once: AtomicBool::default(),
            doctor_runs: AtomicUsize::default(),
            openshell: false,
            accountless_tuple: None,
            next_workspace_id: AtomicU32::new(100),
            refresh_overrides: Vec::new(),
            index_fails_once: AtomicBool::default(),
            takeover: Mutex::new(None),
        }
    }
}

impl SimBackend {
    pub fn shared() -> Arc<dyn Backend> {
        Arc::new(SimBackend::default())
    }

    /// Checks a launch before it starts: a workspace that does not allow the
    /// role refuses it, and a role whose manifest prompts for env values asks
    /// for them. The launch it guards is the OpenShell one, the only one
    /// jackin makes.
    pub fn guarded() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            launch: LaunchRule::Guarded,
            openshell: true,
            ..SimBackend::default()
        })
    }

    pub fn guarded_accountless(tuple: TupleIdentity) -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            launch: LaunchRule::Guarded,
            openshell: true,
            accountless_tuple: Some(tuple),
            ..SimBackend::default()
        })
    }

    pub fn failing_launch() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            launch: LaunchRule::FailNetwork,
            ..SimBackend::default()
        })
    }

    pub fn hard_cases() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            save_fails_once: AtomicBool::new(true),
            launch: LaunchRule::HardCases,
            refresh_overrides: vec![
                ("acct-claude-personal".into(), RefreshOverride::BrokerDown),
                ("acct-claude-work".into(), RefreshOverride::Healthy),
            ],
            index_fails_once: AtomicBool::new(true),
            takeover: Mutex::new(Some(Takeover {
                at_ms: 45_000,
                by: "tty004 · MacBook".into(),
            })),
            ..SimBackend::default()
        })
    }

    pub fn failing_first_bind() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            bind_fails_once: AtomicBool::new(true),
            ..SimBackend::default()
        })
    }

    pub fn openshell() -> Arc<dyn Backend> {
        Arc::new(SimBackend {
            openshell: true,
            ..SimBackend::default()
        })
    }

    fn launch_plan(&self, world: &World, target: &LaunchTarget) -> LaunchPlan {
        match self.launch {
            LaunchRule::Clean | LaunchRule::Guarded => LaunchPlan::Clean,
            LaunchRule::FailNetwork => LaunchPlan::FailNetwork,
            LaunchRule::HardCases => {
                if world.op.session() == OpSession::Locked
                    && target.account().as_deref().is_some_and(|a| {
                        a.contains("work") || a.contains("grok") || a.contains("codex-primary")
                    })
                {
                    LaunchPlan::CredentialsLocked
                } else if target.role == "sre" {
                    LaunchPlan::BlockedSidecar
                } else {
                    LaunchPlan::Clean
                }
            }
        }
    }

    fn save_ok(&self) -> bool {
        !self.save_fails_once.swap(false, Ordering::SeqCst)
    }

    fn free_workspace_id(&self, taken: &[Workspace]) -> WorkspaceId {
        let above = taken.iter().map(|w| w.id + 1).max().unwrap_or(0);
        let id = self.next_workspace_id.load(Ordering::SeqCst).max(above);
        self.next_workspace_id.store(id + 1, Ordering::SeqCst);
        id
    }

    fn settle_global_save(&self, global: &GlobalConfig, world: &World) -> Msg {
        if !self.save_ok() {
            return Msg::GlobalSaved {
                result: Err(SaveFailure {
                    error: fill_in(
                        &world.surfaces.vocabulary.save_error_global,
                        &[("config", &world.config_file())],
                    ),
                    next: Some(world.surfaces.vocabulary.save_error_next.clone()),
                }),
            };
        }
        let mut snapshot = snapshot(world);
        snapshot.global = global.clone();
        Msg::GlobalSaved {
            result: Ok(SaveSuccess {
                snapshot: Box::new(snapshot),
                warning: None,
            }),
        }
    }

    fn settle_workspace_save(
        &self,
        id: Option<WorkspaceId>,
        workspace: &Workspace,
        world: &World,
    ) -> Msg {
        if !self.save_ok() {
            return Msg::WorkspaceSaved {
                id,
                result: Err(SaveFailure {
                    error: fill_in(
                        &world.surfaces.vocabulary.save_error_workspace,
                        &[("config", &world.config_file())],
                    ),
                    next: Some(world.surfaces.vocabulary.save_error_next.clone()),
                }),
            };
        }
        let mut snapshot = snapshot(world);
        let mut stored = workspace.clone();
        let assigned = match id {
            Some(id) => {
                if let Some(slot) = snapshot.workspaces.iter_mut().find(|w| w.id == id) {
                    stored.id = id;
                    *slot = stored;
                }
                id
            }
            None => {
                let id = self.free_workspace_id(&snapshot.workspaces);
                stored.id = id;
                snapshot.workspaces.push(stored);
                id
            }
        };
        Msg::WorkspaceSaved {
            id: Some(assigned),
            result: Ok(SaveSuccess {
                snapshot: Box::new(snapshot),
                warning: None,
            }),
        }
    }
}

impl Backend for SimBackend {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        let (after_ms, msg) = match effect {
            Effect::SaveWorkspace { .. } => return Some(Completion::settle(900)),
            Effect::SaveGlobal { .. } => return Some(Completion::settle(900)),
            Effect::SaveAccounts { .. } => return Some(Completion::settle(0)),
            Effect::TrustRole { .. } => return Some(Completion::settle(0)),
            Effect::DeleteWorkspace { .. } | Effect::Restore { .. } => {
                return Some(Completion::settle(0));
            }
            Effect::Prewarm { workspace } => (
                2_400,
                Msg::Prewarmed {
                    workspace: *workspace,
                    result: Ok(()),
                },
            ),
            Effect::Stop { .. } | Effect::PowerStart { .. } | Effect::PowerStop { .. } => {
                return Some(Completion::settle(1_800));
            }
            Effect::Purge { .. } => return Some(Completion::settle(2_200)),
            Effect::Refresh { .. } => return Some(Completion::settle(0)),
            Effect::RefreshAccount {
                account,
                stagger_ms,
                ..
            } => {
                let base = world
                    .accounts
                    .get(account)
                    .map(provider::refresh_duration_ms)?;
                return Some(Completion::settle(base + stagger_ms));
            }
            Effect::ValidateAccount { .. } => return Some(Completion::settle(900)),
            Effect::CheckRole { .. } => return Some(Completion::settle(600)),
            Effect::SaveRole { .. } => return Some(Completion::settle(900)),
            Effect::BindRole { .. } => return Some(Completion::settle(900)),
            Effect::AddRole { .. } => return Some(Completion::settle(700)),
            Effect::RemoveRole { .. } => return Some(Completion::settle(0)),
            Effect::RunDoctor => {
                let run = self.doctor_runs.fetch_add(1, Ordering::SeqCst) + 2;
                (
                    900,
                    Msg::DoctorFinished(Ok(surfaces::doctor_report(run, world.now_secs() + 1))),
                )
            }
            Effect::LoadRole { .. } => return Some(Completion::settle(1_100)),
            Effect::CheckSource { source, .. } => {
                return Some(Completion::settle(provider::check_duration_ms(source)));
            }
            Effect::ProbeFolder { ticket, path } => (
                0,
                Msg::FolderProbed {
                    ticket: *ticket,
                    result: Ok(provider::probe_folder(path)),
                },
            ),
            Effect::ReadChanges {
                ticket,
                instance,
                touched,
                uncommitted,
                unpushed,
            } => (
                0,
                Msg::ChangesRead {
                    ticket: *ticket,
                    result: Ok(changes::changes_for(
                        &instance.sandbox_name,
                        touched,
                        *uncommitted,
                        *unpushed,
                    )),
                },
            ),
            Effect::LoadOp { .. } => return Some(Completion::settle(world.op.latency_ms())),
        };
        Some(Completion::ready(after_ms, msg))
    }

    fn due(&self, world: &World) -> Vec<Msg> {
        let mut slot = self.takeover.lock().expect("takeover");
        if !slot.as_ref().is_some_and(|t| world.clock.now_ms >= t.at_ms) {
            return vec![];
        }
        let Some(Takeover { by, .. }) = slot.take() else {
            return vec![];
        };
        world
            .instances
            .iter()
            .filter_map(InstanceEntry::as_available)
            .find(|i| i.status == InstanceStatus::Running)
            .map(|i| Msg::Takeover {
                instance: i.id.clone(),
                by,
            })
            .into_iter()
            .collect()
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        match effect {
            Effect::SaveWorkspace { id, workspace, .. } => {
                Some(self.settle_workspace_save(*id, workspace, world))
            }
            Effect::Refresh { ticket } => {
                let result = if self.index_fails_once.swap(false, Ordering::SeqCst) {
                    Err("instance index unreadable".to_owned())
                } else {
                    let mut snapshot = snapshot(world);
                    let now = world.clock.now_secs();
                    for entry in &mut snapshot.instances {
                        if let Some(i) = entry.as_available_mut()
                            && i.status.is_live()
                        {
                            i.last_seen_secs = now;
                        }
                    }
                    Ok(Box::new(snapshot))
                };
                Some(Msg::Refreshed {
                    ticket: *ticket,
                    result,
                })
            }
            Effect::Stop { instance } => Some(Msg::Stopped {
                instance: instance.clone(),
                result: Ok(Box::new(left_running(world, instance, |i| {
                    if i.is_dirty() {
                        InstanceStatus::PreservedDirty
                    } else {
                        InstanceStatus::RestoreAvailable
                    }
                }))),
            }),
            Effect::Purge { instance } => Some(Msg::Purged {
                instance: instance.clone(),
                result: Ok(Box::new(left_running(world, instance, |_| {
                    InstanceStatus::Purged
                }))),
            }),
            Effect::PowerStart { instance } => Some(Msg::PowerStarted {
                instance: instance.clone(),
                result: power_transition(world, instance, true),
            }),
            Effect::PowerStop { instance } => Some(Msg::PowerStopped {
                instance: instance.clone(),
                result: power_transition(world, instance, false),
            }),
            Effect::RefreshAccount {
                ticket,
                account,
                source,
                ..
            } => {
                let refreshed = world.accounts.get(account).map(|a| {
                    let override_for_account = self
                        .refresh_overrides
                        .iter()
                        .find_map(|(id, override_)| (id == account).then_some(override_));
                    provider::refresh(
                        a,
                        world.now_secs(),
                        match override_for_account {
                            Some(RefreshOverride::Healthy) => false,
                            Some(RefreshOverride::BrokerDown) | None => {
                                world.op.session() != OpSession::SignedIn
                            }
                        },
                        matches!(override_for_account, Some(RefreshOverride::BrokerDown)),
                    )
                });
                Some(Msg::AccountRefreshed {
                    ticket: *ticket,
                    account: account.clone(),
                    source: Box::new(source.clone()),
                    result: refreshed
                        .map(Box::new)
                        .ok_or_else(|| format!("{account} is not in the registry")),
                })
            }
            Effect::ValidateAccount {
                ticket,
                account,
                source,
            } => {
                let result = match world.accounts.get(account) {
                    None => Err(format!("{account} is not in the registry")),
                    Some(a) => Ok(match source {
                        CredentialSource::PlainApiKey { .. } => Validation::Fingerprint {
                            level: fingerprint_level(a),
                        },
                        source => Validation::Checked(Box::new(provider::validate(
                            a.provider,
                            source,
                            None,
                            world.op.as_ref(),
                            world.now_secs(),
                        ))),
                    }),
                };
                Some(Msg::AccountValidated {
                    ticket: *ticket,
                    account: account.clone(),
                    source: Box::new(source.clone()),
                    result,
                })
            }
            Effect::CheckSource {
                ticket,
                provider,
                source,
                key,
            } => Some(Msg::SourceChecked {
                ticket: *ticket,
                result: Ok(Box::new(provider::validate(
                    *provider,
                    source,
                    key.as_ref().map(PlainKey::expose),
                    world.op.as_ref(),
                    world.now_secs(),
                ))),
            }),
            Effect::LoadOp { ticket, scope } => Some(Msg::OpLoaded {
                ticket: *ticket,
                result: provider::list_op(world.op.as_ref(), scope),
            }),
            Effect::TrustRole { role } => {
                let mut snapshot = snapshot(world);
                let result = match snapshot.roles.iter_mut().find(|r| r.name == *role) {
                    Some(r) => {
                        r.trusted = true;
                        let source = match &r.source {
                            RoleSource::Git { url, .. } => url.clone(),
                            RoleSource::Local { path } => path.clone(),
                        };
                        for row in snapshot
                            .global
                            .trust
                            .iter_mut()
                            .filter(|t| t.source == source)
                        {
                            row.trusted = true;
                        }
                        Ok(Box::new(snapshot))
                    }
                    None => Err(world
                        .surfaces
                        .vocabulary
                        .rejection(&Rejection::UnknownRole { role: role.clone() })),
                };
                Some(Msg::RoleTrusted {
                    role: role.clone(),
                    result,
                })
            }
            Effect::LoadRole { role } => {
                let result = match world.roles.iter().find(|r| r.name == *role) {
                    None => Err(world
                        .surfaces
                        .vocabulary
                        .rejection(&Rejection::UnknownRole { role: role.clone() })),
                    Some(r) if !r.trusted => Err("trust required".to_owned()),
                    Some(RoleEntry {
                        fetch_error: Some(error),
                        ..
                    }) => Err(error.clone()),
                    Some(_) => {
                        let mut snapshot = snapshot(world);
                        if let Some(r) = snapshot.roles.iter_mut().find(|r| r.name == *role) {
                            r.load_error = None;
                            r.in_registry = true;
                        }
                        Ok(Box::new(snapshot))
                    }
                };
                Some(Msg::RoleLoaded {
                    role: role.clone(),
                    result,
                })
            }
            Effect::CheckRole { ticket, role, doc } => Some(Msg::RoleChecked {
                ticket: *ticket,
                role: role.clone(),
                result: with_role(world, role, |rec| {
                    rec.check = RoleCheckState::Done(surfaces::check_role(doc));
                }),
            }),
            Effect::SaveRole { role, doc, .. } => Some(Msg::RoleSaved {
                role: role.clone(),
                result: with_role(world, role, |rec| {
                    rec.manifest = Ok((**doc).clone());
                    rec.check = RoleCheckState::Done(surfaces::check_role(doc));
                    rec.name = doc.name.clone();
                }),
            }),
            Effect::BindRole {
                role,
                agent,
                account,
                ..
            } => {
                let result = if self.bind_fails_once.swap(false, Ordering::SeqCst) {
                    Err("could not write config.toml: permission denied (EACCES)".into())
                } else {
                    with_role(world, role, |rec| match account {
                        Some(id) => {
                            rec.bindings.insert(*agent, id.clone());
                        }
                        None => {
                            rec.bindings.remove(agent);
                        }
                    })
                };
                Some(Msg::RoleBound {
                    role: role.clone(),
                    agent: *agent,
                    result: result.map(|snapshot| SaveSuccess {
                        snapshot,
                        warning: None,
                    }),
                })
            }
            Effect::AddRole { location } => {
                let result = surfaces::add_role(location, |name| {
                    world
                        .surfaces
                        .roles
                        .as_ref()
                        .is_some_and(|a| a.role(name).is_some())
                })
                .and_then(|rec| {
                    let name = rec.name.clone();
                    let mut snapshot = snapshot(world);
                    let admin = snapshot
                        .surfaces
                        .roles
                        .as_mut()
                        .ok_or("roles are not available")?;
                    admin.roles.push(rec);
                    Ok((name, save_success(snapshot)))
                });
                Some(Msg::RoleAdded {
                    location: location.clone(),
                    result,
                })
            }
            Effect::RemoveRole { role } => {
                let mut snapshot = snapshot(world);
                let removed = snapshot.surfaces.roles.as_mut().is_some_and(|admin| {
                    let before = admin.roles.len();
                    admin.roles.retain(|r| r.name != *role);
                    admin.roles.len() != before
                });
                Some(Msg::RoleRemoved {
                    role: role.clone(),
                    result: if removed {
                        Ok(save_success(snapshot))
                    } else {
                        Err(world
                            .surfaces
                            .vocabulary
                            .rejection(&Rejection::UnknownRole { role: role.clone() }))
                    },
                })
            }
            Effect::DeleteWorkspace { id } => {
                let mut snapshot = snapshot(world);
                snapshot.workspaces.retain(|w| w.id != *id);
                for entry in &mut snapshot.instances {
                    match entry {
                        InstanceEntry::Available(i) if i.workspace == Some(*id) => {
                            i.workspace = None;
                        }
                        InstanceEntry::Unavailable(i) if i.workspace == Some(*id) => {
                            i.workspace = None;
                        }
                        _ => {}
                    }
                }
                Some(Msg::WorkspaceDeleted {
                    id: *id,
                    result: Ok(save_success(snapshot)),
                })
            }
            Effect::Restore { instance } => {
                let mut snapshot = snapshot(world);
                let result = match snapshot
                    .instances
                    .iter_mut()
                    .find(|entry| entry.id() == instance)
                    .and_then(InstanceEntry::as_available_mut)
                {
                    Some(i) => {
                        i.status = InstanceStatus::Running;
                        i.last_seen_secs = world.clock.now_secs();
                        Ok(Box::new(snapshot))
                    }
                    None => Err(world
                        .surfaces
                        .vocabulary
                        .rejection(&Rejection::NotInRoster {
                            instance: instance.short_label(),
                        })),
                };
                Some(Msg::Restored {
                    instance: instance.clone(),
                    result,
                })
            }
            Effect::SaveGlobal { global, .. } => Some(self.settle_global_save(global, world)),
            Effect::SaveAccounts {
                ticket,
                base,
                registry,
            } => Some(settle_accounts_save(*ticket, base, registry, world)),
            Effect::Prewarm { .. }
            | Effect::RunDoctor
            | Effect::ProbeFolder { .. }
            | Effect::ReadChanges { .. } => None,
        }
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        if self.openshell {
            launch::openshell_content()
        } else {
            launch::content()
        }
    }

    fn account_requirement(&self, tuple: TupleIdentity) -> AccountRequirement {
        if self.accountless_tuple == Some(tuple) {
            AccountRequirement::Optional
        } else {
            AccountRequirement::Required
        }
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        let opening = if self.launch == LaunchRule::Guarded {
            launch::opening(world, target)
        } else if let Some(reason) = launch::validate_tuples(world, target) {
            Opening::Refuse(reason)
        } else {
            Opening::Straight
        };
        Box::new(SimLaunch::start(
            self.launch_plan(world, target),
            opening,
            world,
            target,
            self.launch_content(),
        ))
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        Box::new(ScriptPane::new(
            request,
            &spawn,
            scripts::SimScripts::shared(),
        ))
    }
}

/// A removed account also leaves every workspace that named it. A discovered
/// account that the registry drops (the twin of a folder that was registered)
/// leaves them as they are.
fn settle_accounts_save(
    ticket: Ticket,
    base: &AccountRegistry,
    registry: &AccountRegistry,
    world: &World,
) -> Msg {
    let mut registry = registry.clone();
    let added = give_ids(&mut registry);
    let mut snapshot = snapshot(world);
    for gone in base
        .accounts
        .iter()
        .filter(|a| a.origin == AccountOrigin::Registered && registry.get(&a.id).is_none())
    {
        for ws in &mut snapshot.workspaces {
            ws.accounts.enabled.remove(&gone.id);
            ws.accounts.disabled_defaults.remove(&gone.id);
            ws.accounts.preferred.retain(|_, v| *v != gone.id);
            ws.accounts.role_preferred.retain(|_, v| *v != gone.id);
        }
    }
    snapshot.accounts = registry;
    Msg::AccountsSaved {
        ticket,
        added,
        result: Ok(SaveSuccess {
            snapshot: Box::new(snapshot),
            warning: None,
        }),
    }
}

/// Gives each account the form sent without an id the id `acct-<provider>-<name>`,
/// with a number added when another account has it. Returns the last one given.
fn give_ids(registry: &mut AccountRegistry) -> Option<AccountId> {
    let mut added = None;
    for i in 0..registry.accounts.len() {
        if !registry.accounts[i].is_unsaved() {
            continue;
        }
        let account = &registry.accounts[i];
        let stem = format!(
            "acct-{}-{}",
            account
                .provider
                .short()
                .to_lowercase()
                .replace(['/', ' ', '.'], ""),
            account
                .display_name
                .to_lowercase()
                .replace(|c: char| !c.is_ascii_alphanumeric(), "-")
        );
        let id = (1..)
            .map(|n| {
                if n == 1 {
                    stem.clone()
                } else {
                    format!("{stem}-{n}")
                }
            })
            .find(|id| registry.get(id).is_none())
            .expect("an unused id");
        registry.accounts[i].id.clone_from(&id);
        added = Some(id);
    }
    added
}

/// The roster after `instance` stopped running: its status, the time it was
/// last seen, and nothing else.
fn left_running(
    world: &World,
    instance: &InstanceId,
    status: impl Fn(&Instance) -> InstanceStatus,
) -> Snapshot {
    let mut snapshot = snapshot(world);
    let now = world.clock.now_secs();
    if let Some(i) = snapshot
        .instances
        .iter_mut()
        .find(|entry| entry.id() == instance)
        .and_then(InstanceEntry::as_available_mut)
    {
        i.status = status(i);
        i.last_seen_secs = now;
    }
    snapshot
}

fn power_transition(
    world: &World,
    instance: &InstanceId,
    start: bool,
) -> Result<Box<Snapshot>, String> {
    let mut snapshot = snapshot(world);
    let Some(InstanceEntry::Unavailable(unavailable)) = snapshot
        .instances
        .iter_mut()
        .find(|entry| entry.id() == instance)
    else {
        return Err("Cannot power this sandbox because its metadata is no longer available".into());
    };
    if start {
        if !unavailable.phase.startable() {
            let display = unavailable.display();
            return Err(format!(
                "Cannot start sandbox {} while {} · only stopped sandboxes can start",
                display.sandbox_name,
                unavailable.phase.label()
            ));
        }
        unavailable.phase = SandboxPhase::Ready;
    } else {
        if !unavailable.phase.ready() {
            let display = unavailable.display();
            return Err(format!(
                "Cannot stop sandbox {} while {} · only ready sandboxes can stop",
                display.sandbox_name,
                unavailable.phase.label()
            ));
        }
        unavailable.phase = SandboxPhase::Stopped;
    }
    Ok(Box::new(snapshot))
}

/// How far a key that is held as a fingerprint can be vouched for.
fn fingerprint_level(account: &Account) -> ValidationLevel {
    if account.usage.windows.iter().any(|w| w.has_meter()) {
        ValidationLevel::QuotaReadable
    } else if account.identity.subject.is_some() {
        ValidationLevel::IdentityAuthenticated
    } else {
        ValidationLevel::MaterialDiscovered
    }
}

fn with_role(
    world: &World,
    role: &str,
    edit: impl FnOnce(&mut RoleRecord),
) -> Result<Box<Snapshot>, String> {
    let mut snapshot = snapshot(world);
    let record = snapshot
        .surfaces
        .roles
        .as_mut()
        .and_then(|admin| admin.role_mut(role))
        .ok_or_else(|| {
            world
                .surfaces
                .vocabulary
                .rejection(&Rejection::UnknownRole {
                    role: role.to_owned(),
                })
        })?;
    edit(record);
    Ok(Box::new(snapshot))
}

pub fn snapshot(world: &World) -> Snapshot {
    Snapshot {
        workspaces: world.workspaces.clone(),
        roles: world.roles.clone(),
        instances: world.instances.clone(),
        global: world.global.clone(),
        accounts: world.accounts.clone(),
        surfaces: world.surfaces.clone(),
        github: world.github.clone(),
        index_error: world.arbiter.discovery.err(),
        daemon_health: world.daemon_health,
    }
}
