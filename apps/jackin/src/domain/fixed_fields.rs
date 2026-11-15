//! The OpenShell constants of the baseline records, and the class of every field.
//!
//! Fixed: a field whose value the OpenShell world decides: one constant for
//! every record, or a function of the record itself (`edit_blocked` follows the
//! account's source). The extractors read it and the tie compares it. Data: a
//! field the host supplies. The extractors name it `field: _` and the tie does
//! not compare it. Console: a `World` field the running console owns and fills
//! itself (`clock`, `backend`, `op` and the like), which projection never
//! produces; it also appears as `field: _`. Every record is destructured
//! without `..`, so a new field does not build until it has a class.
//!
//! A field wrong in both the table below and the expected literal of a test
//! passes every check. Only a review of the table closes that hole.

use super::account::{Account, AccountForm, AccountOrigin, AccountRegistry, EditBlocked};
use super::agent::{Agent, AgentBlocked};
use super::fixed::Fixed;
use super::instance::{Instance, InstanceEntry};
use super::role::LaunchRoleRule;
use super::surfaces::Surfaces;
use super::vocabulary::Vocabulary;
use super::workspace::{
    AccountPolicy, DirtyExitPolicy, Isolation, Mount, MountKind, MountScope, MountSource,
    RoleEntry, WizardDefaults, Workspace,
};
use crate::world::{GlobalConfig, Snapshot, TrustRow, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountShape {
    pub destination: String,
    pub readonly: bool,
    pub isolation: Isolation,
    pub kind: MountKind,
    pub scope: MountScope,
    pub host_source: bool,
    pub drift: Option<String>,
    pub running_isolated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFixed {
    pub workdir: String,
    pub mounts: Vec<MountShape>,
    pub keep_awake: bool,
    pub git_pull: bool,
    pub dirty_policy: DirtyExitPolicy,
    pub disabled_defaults_empty: bool,
    pub role_preferred_empty: bool,
    pub enabled_is_every_account: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalFixed {
    pub coauthor_trailer: bool,
    pub dco_signoff: bool,
    pub mounts_empty: bool,
    pub role_env_empty: bool,
    pub all_agent_modes_present: bool,
    pub all_trusted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountFixed {
    pub enabled: bool,
    pub origin: AccountOrigin,
    pub edit_blocked: Option<EditBlocked>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleFixed {
    pub trusted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceFixed {
    Available { workdir: Option<String> },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceFixed {
    pub vocabulary: Vocabulary,
    pub fixed: Option<Fixed>,
    pub account_form: AccountForm,
    pub agent_blocked: Option<AgentBlocked>,
    pub wizard: Option<WizardDefaults>,
    pub launch_role: LaunchRoleRule,
}

impl SurfaceFixed {
    pub fn openshell() -> Self {
        Self {
            vocabulary: Vocabulary::openshell(),
            fixed: Some(Fixed::openshell()),
            account_form: AccountForm::openshell(),
            agent_blocked: None,
            wizard: Some(WizardDefaults::openshell()),
            launch_role: LaunchRoleRule::Configured,
        }
    }
}

/// The Fixed fields of one world or one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFields {
    pub workspaces: Vec<WorkspaceFixed>,
    pub roles: Vec<RoleFixed>,
    pub instances: Vec<InstanceFixed>,
    pub global: GlobalFixed,
    pub accounts: Vec<AccountFixed>,
    pub surfaces: SurfaceFixed,
}

fn mount_shape(m: &Mount) -> MountShape {
    let Mount {
        source,
        destination,
        readonly,
        isolation,
        kind,
        scope,
        drift,
        running_isolated,
    } = m;
    MountShape {
        destination: destination.clone(),
        readonly: *readonly,
        isolation: *isolation,
        kind: *kind,
        scope: scope.clone(),
        host_source: matches!(source, MountSource::Host(_)),
        drift: drift.clone(),
        running_isolated: *running_isolated,
    }
}

pub fn workspace_fixed(ws: &Workspace, registry: &AccountRegistry) -> WorkspaceFixed {
    let Workspace {
        id: _,
        name: _,
        workdir,
        mounts,
        roles: _,
        env: _,
        role_env: _,
        keep_awake,
        git_pull,
        docker: _,
        accounts,
        dirty_policy,
        github: _,
    } = ws;
    let AccountPolicy {
        disabled_defaults,
        enabled: _,
        preferred: _,
        role_preferred,
    } = accounts;
    WorkspaceFixed {
        workdir: workdir.clone(),
        mounts: mounts.iter().map(mount_shape).collect(),
        keep_awake: *keep_awake,
        git_pull: *git_pull,
        dirty_policy: *dirty_policy,
        disabled_defaults_empty: disabled_defaults.is_empty(),
        role_preferred_empty: role_preferred.is_empty(),
        enabled_is_every_account: ws.effective_accounts(registry).len() == registry.accounts.len(),
    }
}

pub fn global_fixed(g: &GlobalConfig) -> GlobalFixed {
    let GlobalConfig {
        coauthor_trailer,
        dco_signoff,
        mounts,
        env: _,
        role_env,
        agent_modes,
        trust,
    } = g;
    GlobalFixed {
        coauthor_trailer: *coauthor_trailer,
        dco_signoff: *dco_signoff,
        mounts_empty: mounts.is_empty(),
        role_env_empty: role_env.is_empty(),
        all_agent_modes_present: Agent::ALL
            .into_iter()
            .all(|agent| agent_modes.contains_key(&agent)),
        all_trusted: trust.iter().all(|row| {
            let TrustRow {
                source: _,
                kind: _,
                trusted,
                roles: _,
            } = row;
            *trusted
        }),
    }
}

pub fn account_fixed(a: &Account) -> AccountFixed {
    let Account {
        id: _,
        origin,
        display_name: _,
        agent: _,
        provider: _,
        surface: _,
        source: _,
        identity: _,
        provenance: _,
        confidence: _,
        lifecycle: _,
        purpose: _,
        enabled,
        default_for_provider: _,
        validation: _,
        last_refresh_secs: _,
        issue: _,
        endpoint: _,
        usage: _,
        launch_blocked: _,
        edit_blocked,
    } = a;
    AccountFixed {
        enabled: *enabled,
        origin: *origin,
        edit_blocked: edit_blocked.clone(),
    }
}

pub fn role_fixed(r: &RoleEntry) -> RoleFixed {
    let RoleEntry {
        namespace: _,
        name: _,
        source: _,
        trusted,
        in_registry: _,
        description: _,
        load_error: _,
        fetch_error: _,
    } = r;
    RoleFixed { trusted: *trusted }
}

pub fn instance_fixed(i: &Instance) -> InstanceFixed {
    let Instance {
        id: _,
        container: _,
        workspace: _,
        workdir,
        role: _,
        agent: _,
        status: _,
        created_secs: _,
        last_seen_secs: _,
        run_id: _,
        sessions: _,
        daemon: _,
        branch: _,
        pr: _,
        default_branch: _,
        github: _,
        uncommitted: _,
        unpushed: _,
        account: _,
        accounts: _,
        agents: _,
        task: _,
    } = i;
    InstanceFixed::Available {
        workdir: workdir.clone(),
    }
}

fn instance_entry_fixed(entry: &InstanceEntry) -> InstanceFixed {
    match entry {
        InstanceEntry::Available(instance) => instance_fixed(instance),
        InstanceEntry::Unavailable(_) => InstanceFixed::Unavailable,
    }
}

pub fn surface_fixed(s: &Surfaces) -> SurfaceFixed {
    let Surfaces {
        roles: _,
        docker: _,
        doctor: _,
        vocabulary,
        account_form,
        agent_blocked,
        launch_role,
        wizard,
        fixed,
        tuples: _,
        version: _,
        cockpit_version: _,
        config_path: _,
        about: _,
    } = s;
    SurfaceFixed {
        vocabulary: vocabulary.clone(),
        fixed: fixed.clone(),
        account_form: account_form.clone(),
        agent_blocked: agent_blocked.clone(),
        wizard: wizard.clone(),
        launch_role: *launch_role,
    }
}

fn collect(
    workspaces: &[Workspace],
    roles: &[RoleEntry],
    instances: &[InstanceEntry],
    global: &GlobalConfig,
    registry: &AccountRegistry,
    surfaces: &Surfaces,
) -> FixedFields {
    let AccountRegistry {
        accounts,
        revision: _,
    } = registry;
    FixedFields {
        workspaces: workspaces
            .iter()
            .map(|ws| workspace_fixed(ws, registry))
            .collect(),
        roles: roles.iter().map(role_fixed).collect(),
        instances: instances.iter().map(instance_entry_fixed).collect(),
        global: global_fixed(global),
        accounts: accounts.iter().map(account_fixed).collect(),
        surfaces: surface_fixed(surfaces),
    }
}

pub fn world_fixed(w: &World) -> FixedFields {
    let World {
        clock: _,
        time: _,
        arbiter: _,
        home: _,
        cwd: _,
        workspaces,
        roles,
        instances,
        daemons: _,
        global,
        accounts,
        op: _,
        backend: _,
        fs: _,
        github: _,
        clipboard: _,
        jobs: _,
        daemon_health: _,
        last_refresh_secs: _,
        session_accounts: _,
        discovery_pending: _,
        surfaces,
        tickets: _,
        pending_refreshes: _,
        snapshots_applied: _,
        authoritative_revision: _,
        refresh_from: _,
        refresh_started_secs: _,
        refresh_ticket: _,
        saved_ticket: _,
        committed_save_warning: _,
        committed_save_status: _,
        pane_policy: _,
    } = w;
    collect(workspaces, roles, instances, global, accounts, surfaces)
}

pub fn snapshot_fixed(s: &Snapshot) -> FixedFields {
    let Snapshot {
        workspaces,
        roles,
        instances,
        global,
        accounts,
        surfaces,
        github: _,
        index_error: _,
        daemon_health: _,
    } = s;
    collect(workspaces, roles, instances, global, accounts, surfaces)
}
