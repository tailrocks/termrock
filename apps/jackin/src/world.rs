use std::collections::BTreeMap;
use std::sync::{Arc, PoisonError};

use crate::arbiter::{Arbiter, DiscoveryError};
use crate::backend::{
    Backend, ChangeSet, Effect, FolderProbe, OnePassword, OpError, OpListing, Refresh, Reply,
    Ticket, Validation, ValidationOutcome, apply_validation,
};
use crate::domain::account::{
    AccountId, AccountRegistry, CredentialSource, IssueCode, Recoverability, RecoverableIssue,
    ValidationState,
};
use crate::domain::agent::{Agent, AuthMode, UsageSurface};
use crate::domain::doctor::{DoctorFailure, DoctorReport};
use crate::domain::instance::{
    DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus, RepoRef,
};
use crate::domain::reason::{IssueReason, UsageReason};
use crate::domain::role::{
    AccountOrder, DEFAULT_ROLE_UNUSABLE, LaunchRole, LaunchRoleRule, NO_DEFAULT_ROLE,
    NO_SAVED_WORKSPACE,
};
use crate::domain::surfaces::Surfaces;
use crate::domain::tuple::TupleIdentity;
use crate::domain::usage::Freshness;
use crate::domain::workspace::{
    EnvVar, Mount, RoleEntry, RoleName, Usability, Workspace, WorkspaceId,
};
use crate::panes::{Daemon, PaneId, PanePolicy};
use junie_tui::clock::SimClock;
use junie_tui::shell::JobKey;

mod projection;

pub use projection::{
    AccountIndex, Projection, ProjectionEpochs, RosterIndex, WorkCounters, WorkSnapshot,
    WorkspaceMembers, WorldTick,
};

pub const REFRESH_TIMEOUT_SECS: i64 = 30;

pub mod job {
    use junie_tui::shell::JobKey;
    pub const WORKSPACE_SAVE: JobKey = JobKey::new("workspace.save");
    pub const SETTINGS_SAVE: JobKey = JobKey::new("settings.save");
    pub const ROLE_LOAD: JobKey = JobKey::new("role.load");
    pub const PREWARM: JobKey = JobKey::new("prewarm");
    pub const STOP: JobKey = JobKey::new("instance.stop");
    pub const POWER: JobKey = JobKey::new("instance.power");
    pub const PURGE: JobKey = JobKey::new("instance.purge");
    pub const ACCOUNT_VALIDATE: JobKey = JobKey::new("account.validate");
    pub const ROLE_CHECK: JobKey = JobKey::new("role.check");
    pub const ROLE_SAVE: JobKey = JobKey::new("role.save");
    pub const ROLE_BIND: JobKey = JobKey::new("role.bind");
    pub const ROLE_ADD: JobKey = JobKey::new("role.add");
    pub const DOCTOR: JobKey = JobKey::new("doctor");
    pub const SOURCE_CHECK: JobKey = JobKey::new("source.check");
    pub const CHANGES_READ: JobKey = JobKey::new("changes.read");
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlobalConfig {
    pub coauthor_trailer: bool,
    pub dco_signoff: bool,
    pub mounts: Vec<Mount>,
    pub env: Vec<EnvVar>,
    pub role_env: BTreeMap<RoleName, Vec<EnvVar>>,

    pub agent_modes: BTreeMap<Agent, AuthMode>,
    pub trust: Vec<TrustRow>,
}

impl GlobalConfig {
    pub fn change_count(&self, other: &GlobalConfig) -> usize {
        let mut n = 0;
        n += usize::from(self.coauthor_trailer != other.coauthor_trailer);
        n += usize::from(self.dco_signoff != other.dco_signoff);
        n += keyed_diff(&self.mounts, &other.mounts, |m| m.destination.clone());
        n += keyed_diff(&self.env, &other.env, |e| e.key.clone());
        n += usize::from(self.role_env != other.role_env);
        n += self
            .agent_modes
            .iter()
            .filter(|(a, m)| other.agent_modes.get(a) != Some(m))
            .count();
        n += keyed_diff(&self.trust, &other.trust, |t| t.source.clone());
        n
    }
}

pub fn keyed_diff<T: PartialEq>(a: &[T], b: &[T], key: impl Fn(&T) -> String) -> usize {
    let mut n = 0;
    for x in a {
        match b.iter().find(|y| key(y) == key(x)) {
            None => n += 1,
            Some(y) if y != x => n += 1,
            _ => {}
        }
    }
    n += b
        .iter()
        .filter(|y| !a.iter().any(|x| key(x) == key(y)))
        .count();
    n
}

/// Why a save did not happen: what went wrong, and what the operator can do
/// about it. A backend that has no advice sends none, and the dialog shows no
/// `Next` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFailure {
    pub error: String,
    pub next: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveWarning {
    DirectorySync,
}

impl SaveWarning {
    pub fn status(self) -> &'static str {
        match self {
            SaveWarning::DirectorySync => "Saved, durability uncertain",
        }
    }
}

/// Where the roles of a trust row come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustKind {
    Git,
    Path,
}

impl TrustKind {
    pub fn label(self) -> &'static str {
        match self {
            TrustKind::Git => "git",
            TrustKind::Path => "path",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustRow {
    pub source: String,
    pub kind: TrustKind,
    pub trusted: bool,
    pub roles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEntry {
    pub path: String,
    pub dir: bool,
    pub git: Option<String>,
    pub meta: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubRepo {
    pub full_name: String,
    pub default_branch: String,
    pub branches: Vec<String>,
    pub updated: String,
    pub url: String,
}

impl From<&GithubRepo> for RepoRef {
    fn from(r: &GithubRepo) -> Self {
        Self {
            full_name: r.full_name.clone(),
            url: r.url.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    WorkspaceSaved {
        id: Option<WorkspaceId>,
        result: Result<SaveSuccess, SaveFailure>,
    },
    GlobalSaved {
        result: Result<SaveSuccess, SaveFailure>,
    },
    AccountsSaved {
        ticket: Ticket,
        added: Option<AccountId>,
        result: Result<SaveSuccess, String>,
    },
    WorkspaceDeleted {
        id: WorkspaceId,
        result: Result<SaveSuccess, String>,
    },
    Restored {
        instance: InstanceId,
        result: Result<Box<Snapshot>, String>,
    },

    Refreshed {
        ticket: Ticket,
        result: Result<Box<Snapshot>, String>,
    },
    Prewarmed {
        workspace: WorkspaceId,
        result: Result<(), String>,
    },
    Stopped {
        instance: InstanceId,
        result: Result<Box<Snapshot>, String>,
    },
    PowerStarted {
        instance: InstanceId,
        result: Result<Box<Snapshot>, String>,
    },
    PowerStopped {
        instance: InstanceId,
        result: Result<Box<Snapshot>, String>,
    },
    Purged {
        instance: InstanceId,
        result: Result<Box<Snapshot>, String>,
    },
    AccountRefreshed {
        /// The `ticket` of the `Effect::RefreshAccount` this answers.
        ticket: Ticket,
        account: AccountId,
        source: Box<CredentialSource>,
        result: Result<Box<Refresh>, String>,
    },
    AccountValidated {
        /// The `ticket` of the `Effect::ValidateAccount` this answers.
        ticket: Ticket,
        account: AccountId,
        source: Box<CredentialSource>,
        result: Result<Validation, String>,
    },
    RoleLoaded {
        role: String,
        result: Result<Box<Snapshot>, String>,
    },
    RoleTrusted {
        role: RoleName,
        result: Result<Box<Snapshot>, String>,
    },

    Takeover {
        instance: InstanceId,
        by: String,
    },
    /// The record of a cancelled launch, from a backend that had to wait for
    /// the sandbox before it could say what the cancel left behind.
    LaunchCancelled {
        instance: Box<InstanceEntry>,
    },
    RoleChecked {
        ticket: Ticket,
        role: RoleName,
        result: Result<Box<Snapshot>, String>,
    },
    RoleSaved {
        role: RoleName,
        result: Result<Box<Snapshot>, String>,
    },
    RoleBound {
        role: RoleName,
        agent: Agent,
        result: Result<SaveSuccess, String>,
    },
    RoleAdded {
        location: String,
        result: Result<(RoleName, SaveSuccess), String>,
    },
    RoleRemoved {
        role: RoleName,
        result: Result<SaveSuccess, String>,
    },
    DoctorFinished(Result<DoctorReport, DoctorFailure>),
    SourceChecked {
        ticket: Ticket,
        result: Result<Box<ValidationOutcome>, String>,
    },
    FolderProbed {
        ticket: Ticket,
        result: Result<FolderProbe, String>,
    },
    ChangesRead {
        ticket: Ticket,
        result: Result<ChangeSet, String>,
    },
    OpLoaded {
        ticket: Ticket,
        result: Result<OpListing, OpError>,
    },
    /// A link of `pane` has output, or an event, waiting. The world drains
    /// that pane now instead of at the next tick. Wakes that find nothing new
    /// ask for no redraw, so a burst of them costs one.
    PaneWake {
        instance: InstanceId,
        pane: PaneId,
    },
}

impl Msg {
    pub fn job(&self) -> Option<JobKey> {
        match self {
            Msg::WorkspaceSaved { .. } => Some(job::WORKSPACE_SAVE),
            Msg::GlobalSaved { .. } => Some(job::SETTINGS_SAVE),
            Msg::RoleLoaded { .. } => Some(job::ROLE_LOAD),
            Msg::Prewarmed { .. } => Some(job::PREWARM),
            Msg::Stopped { .. } => Some(job::STOP),
            Msg::PowerStopped { .. } | Msg::PowerStarted { .. } => Some(job::POWER),
            Msg::Purged { .. } => Some(job::PURGE),
            Msg::AccountValidated { .. } => Some(job::ACCOUNT_VALIDATE),
            Msg::RoleChecked { .. } => Some(job::ROLE_CHECK),
            Msg::RoleSaved { .. } => Some(job::ROLE_SAVE),
            Msg::RoleBound { .. } => Some(job::ROLE_BIND),
            Msg::RoleAdded { .. } => Some(job::ROLE_ADD),
            Msg::DoctorFinished(_) => Some(job::DOCTOR),
            Msg::SourceChecked { .. } => Some(job::SOURCE_CHECK),
            Msg::ChangesRead { .. } => Some(job::CHANGES_READ),
            Msg::AccountsSaved { .. }
            | Msg::WorkspaceDeleted { .. }
            | Msg::Restored { .. }
            | Msg::RoleTrusted { .. }
            | Msg::RoleRemoved { .. }
            | Msg::Refreshed { .. }
            | Msg::AccountRefreshed { .. }
            | Msg::FolderProbed { .. }
            | Msg::OpLoaded { .. }
            | Msg::Takeover { .. }
            | Msg::LaunchCancelled { .. }
            | Msg::PaneWake { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub due_ms: i64,
    pub work: Work,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Work {
    Msg(Msg),
    Settle(Effect),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleLoad {
    Requested,
    NeedsTrust(Box<RoleEntry>),
    Unregistered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Absorbed {
    Current,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonHealth {
    Healthy,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Stale,
    Failing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthView {
    pub noun: String,
    pub health: Health,
    pub at_secs: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingRefresh {
    ticket: Ticket,
    source: CredentialSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingValidation {
    ticket: Ticket,
    source: CredentialSource,
    started_ms: i64,
}

pub struct World {
    pub clock: SimClock,
    pub time: TimeSource,
    pub arbiter: Arbiter,
    pub home: String,
    pub cwd: String,
    pub workspaces: Vec<Workspace>,
    pub roles: Vec<RoleEntry>,
    pub instances: Vec<InstanceEntry>,
    pub daemons: BTreeMap<InstanceId, Daemon>,
    pub global: GlobalConfig,
    pub accounts: AccountRegistry,
    pub op: Box<dyn OnePassword>,
    pub backend: Arc<dyn Backend>,
    pub fs: Vec<FsEntry>,
    pub github: Vec<GithubRepo>,
    pub clipboard: Option<String>,
    pub jobs: Vec<Job>,
    pub daemon_health: DaemonHealth,
    pub last_refresh_secs: i64,

    pub session_accounts: BTreeMap<(InstanceId, u64), AccountId>,

    pub discovery_pending: bool,

    pub surfaces: Surfaces,

    pub tickets: Ticket,

    /// The usage refresh each account is waiting on. World owns request
    /// identity; `Freshness::Refreshing` only shows it. An entry lives from
    /// `refresh_account` until its reply is absorbed or `apply` finds the
    /// account gone or its credential source changed. A host that mutates
    /// `accounts` or a source outside `apply` must call
    /// `reconcile_pending_refreshes` before the next reply is absorbed.
    pub(crate) pending_refreshes: BTreeMap<AccountId, PendingRefresh>,

    /// The validation each account is waiting on. A new `validate_account`
    /// replaces the account's entry, so the latest request wins and an
    /// older reply is stale. Validations of different accounts run side by
    /// side. Retired like `pending_refreshes`: by its accepted reply, or by
    /// `reconcile_pending_validations` once the account is gone or its
    /// credential source changed.
    pub(crate) pending_validations: BTreeMap<AccountId, PendingValidation>,

    /// The validation whose progress text the status line shows: the one
    /// started last. Only its reply ends that status; another account's reply
    /// updates that account and leaves the text alone.
    pub(crate) displayed_validation: Option<(AccountId, Ticket)>,

    pub snapshots_applied: u64,
    pub refresh_from: Option<u64>,
    pub refresh_started_secs: i64,
    pub refresh_ticket: Ticket,
    pub saved_ticket: Ticket,

    pub committed_save_warning: Option<SaveWarning>,
    pub committed_save_status: Option<String>,

    /// What a program may do through a pane; every daemon the world creates
    /// starts with it.
    pub pane_policy: Arc<PanePolicy>,

    /// Epochs, grouped indices and work counters of the derived views.
    pub projection: Projection,
}

pub trait WallSource: std::fmt::Debug + Send + Sync {
    fn epoch_millis(&self) -> i64;

    fn utc_offset_secs(&self) -> i64;
}

#[derive(Debug, Clone, Default)]
pub enum TimeSource {
    #[default]
    Sim,
    Wall(Arc<dyn WallSource>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub workspaces: Vec<Workspace>,
    pub roles: Vec<RoleEntry>,
    pub instances: Vec<InstanceEntry>,
    pub global: GlobalConfig,
    pub accounts: AccountRegistry,
    pub surfaces: Surfaces,
    pub github: Vec<GithubRepo>,
    pub index_error: Option<DiscoveryError>,
    /// How the gateways answered the listing this snapshot comes from.
    pub daemon_health: DaemonHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveSuccess {
    pub snapshot: Box<Snapshot>,
    pub warning: Option<SaveWarning>,
}

pub struct WorldParts {
    pub clock: SimClock,
    pub time: TimeSource,
    pub arbiter: Arbiter,
    pub home: String,
    pub cwd: String,
    pub workspaces: Vec<Workspace>,
    pub roles: Vec<RoleEntry>,
    pub instances: Vec<InstanceEntry>,
    pub global: GlobalConfig,
    pub accounts: AccountRegistry,
    pub op: Box<dyn OnePassword>,
    pub backend: Arc<dyn Backend>,
    pub fs: Vec<FsEntry>,
    pub github: Vec<GithubRepo>,
    pub last_refresh_secs: i64,
    pub surfaces: Surfaces,
}

impl WorldParts {
    pub fn new(
        clock: SimClock,
        home: String,
        cwd: String,
        op: Box<dyn OnePassword>,
        backend: Arc<dyn Backend>,
        surfaces: Surfaces,
    ) -> Self {
        Self {
            last_refresh_secs: clock.now_secs(),
            clock,
            time: TimeSource::Sim,
            arbiter: Arbiter::new(0),
            home,
            cwd,
            workspaces: vec![],
            roles: vec![],
            instances: vec![],
            global: GlobalConfig::default(),
            accounts: AccountRegistry::default(),
            op,
            backend,
            fs: vec![],
            github: vec![],
            surfaces,
        }
    }

    pub fn wall(mut self, source: Arc<dyn WallSource>) -> Self {
        let ms = source.epoch_millis();
        self.clock.epoch_secs = ms.div_euclid(1000);
        self.clock.now_ms = ms.rem_euclid(1000);
        self.clock.utc_offset_secs = source.utc_offset_secs();
        self.last_refresh_secs = self.clock.now_secs();
        self.time = TimeSource::Wall(source);
        self
    }
}

impl World {
    pub fn new(parts: WorldParts) -> Self {
        let WorldParts {
            clock,
            time,
            arbiter,
            home,
            cwd,
            workspaces,
            roles,
            instances,
            global,
            accounts,
            op,
            backend,
            fs,
            github,
            last_refresh_secs,
            surfaces,
        } = parts;
        let mut world = Self {
            clock,
            time,
            arbiter,
            home,
            cwd,
            workspaces,
            roles,
            instances,
            daemons: BTreeMap::new(),
            global,
            accounts,
            op,
            backend,
            fs,
            github,
            clipboard: None,
            jobs: vec![],
            daemon_health: DaemonHealth::Healthy,
            last_refresh_secs,
            session_accounts: BTreeMap::new(),
            discovery_pending: false,
            surfaces,
            tickets: 0,
            pending_refreshes: BTreeMap::new(),
            pending_validations: BTreeMap::new(),
            displayed_validation: None,
            snapshots_applied: 0,
            refresh_from: None,
            refresh_started_secs: 0,
            refresh_ticket: 0,
            saved_ticket: 0,
            committed_save_warning: None,
            committed_save_status: None,
            pane_policy: Arc::default(),
            projection: Projection::default(),
        };
        world.sync_arbiter();
        world
    }

    pub fn now_secs(&self) -> i64 {
        self.clock.now_secs()
    }

    pub fn epochs(&self) -> ProjectionEpochs {
        self.projection.epochs
    }

    /// Work the derived views have done so far.
    pub fn work(&self) -> WorkSnapshot {
        self.projection.work.snapshot()
    }

    /// For a host that edited `instances` or `workspaces` membership through
    /// the public fields. Owning methods mark for themselves.
    pub fn mark_membership_changed(&mut self) {
        self.projection.epochs.bump_membership();
    }

    /// For a host edit that changes only what rows and cards show: names,
    /// counts, daemon summaries.
    pub fn mark_roster_changed(&mut self) {
        self.projection.epochs.bump_roster();
    }

    /// For a host edit of the account registry.
    pub fn mark_accounts_changed(&mut self) {
        self.projection.epochs.bump_accounts();
    }

    /// Instances grouped by workspace, rebuilt only after a membership epoch
    /// change. A rebuild installs the groups and the running ids together.
    pub fn roster(&self) -> Arc<RosterIndex> {
        let epoch = self.projection.epochs.membership;
        let mut slot = self
            .projection
            .roster
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if slot.built_at() != Some(epoch) {
            *slot = Arc::new(RosterIndex::build(
                &self.instances,
                epoch,
                &self.projection.work,
            ));
        }
        Arc::clone(&slot)
    }

    /// The account registry sorted once per accounts epoch.
    pub fn account_index(&self) -> Arc<AccountIndex> {
        let epoch = self.projection.epochs.accounts;
        let mut slot = self
            .projection
            .accounts
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if slot.built_at() != Some(epoch) {
            *slot = Arc::new(AccountIndex::build(
                &self.accounts,
                epoch,
                &self.projection.work,
            ));
        }
        Arc::clone(&slot)
    }

    /// Every account in display order, from the index sorted once per accounts
    /// epoch.
    pub fn sorted_accounts(&self) -> Vec<&crate::domain::account::Account> {
        let index = self.account_index();
        index
            .sorted()
            .iter()
            .filter_map(|at| self.accounts.accounts.get(*at))
            .collect()
    }

    /// The accounts of one usage surface in display order, from the same index.
    pub fn surface_accounts(&self, surface: UsageSurface) -> Vec<&crate::domain::account::Account> {
        let index = self.account_index();
        index
            .surface(surface)
            .iter()
            .filter_map(|at| self.accounts.accounts.get(*at))
            .collect()
    }

    /// An account for editing; the accounts epoch moves because the caller is
    /// about to change it.
    pub fn account_mut(&mut self, id: &str) -> Option<&mut crate::domain::account::Account> {
        self.projection.epochs.bump_accounts();
        self.accounts.get_mut(id)
    }

    pub fn new_daemon(&self, workspace: &str) -> Daemon {
        let mut daemon = Daemon::new(workspace);
        daemon.set_policy(&self.pane_policy);
        daemon
    }

    pub fn open_first_session(&mut self, instance: &InstanceId) {
        if self.daemons.contains_key(instance) {
            return;
        }
        let Some(i) = self.instance(instance) else {
            return;
        };
        let backend = self.backend.clone();
        let requests = backend.initial_panes(i);
        let mut d = self.new_daemon(&self.workspace_label(i));
        let now = self.now_ms();
        for request in requests {
            d.new_tab(&request, now, |spawn| backend.spawn_pane(&request, spawn));
        }
        self.daemons.insert(instance.clone(), d);
        refresh_snapshots(self);
        self.sync_arbiter();
    }

    /// A number no earlier request of this world carried.
    pub fn next_ticket(&mut self) -> Ticket {
        self.tickets += 1;
        self.tickets
    }

    /// Starts a usage refresh of `id` unless one is already out for its
    /// current credential source. The ticket of the request sent, or `None`
    /// when the account is missing or already refreshing (nothing changes
    /// then). The reply is accepted only while it echoes that ticket.
    pub fn refresh_account(&mut self, id: &str, stagger_ms: i64) -> Option<Ticket> {
        let source = self.accounts.get(id)?.source.clone();
        if self
            .pending_refreshes
            .get(id)
            .is_some_and(|pending| pending.source == source)
        {
            return None;
        }
        self.pending_refreshes.remove(id);
        let ticket = self.next_ticket();
        self.account_mut(id)?.usage.freshness.phase = Freshness::Refreshing;
        self.pending_refreshes.insert(
            id.to_owned(),
            PendingRefresh {
                ticket,
                source: source.clone(),
            },
        );
        self.perform(Effect::RefreshAccount {
            ticket,
            account: id.to_owned(),
            source,
            stagger_ms,
        });
        Some(ticket)
    }

    /// Gives every account that is already marked `Refreshing` a pending
    /// refresh of its current source, so the marker keeps its single-flight
    /// meaning. For a world built from accounts caught mid-refresh (the demo
    /// fixtures). It sends no effect: nothing answers the entry until the host
    /// delivers a reply with the ticket `refresh_pending` reports.
    pub fn adopt_refreshing(&mut self) {
        let marked: Vec<(AccountId, CredentialSource)> = self
            .accounts
            .accounts
            .iter()
            .filter(|a| a.usage.freshness.phase == Freshness::Refreshing)
            .filter(|a| !self.pending_refreshes.contains_key(&a.id))
            .map(|a| (a.id.clone(), a.source.clone()))
            .collect();
        for (id, source) in marked {
            let ticket = self.next_ticket();
            self.pending_refreshes
                .insert(id, PendingRefresh { ticket, source });
        }
    }

    /// The ticket of the refresh `id` is waiting on, if one is out.
    pub fn refresh_pending(&self, id: &str) -> Option<Ticket> {
        self.pending_refreshes.get(id).map(|p| p.ticket)
    }

    /// Drops the pending requests whose account is gone or whose credential
    /// source changed, so a later account of the same id or source cannot
    /// inherit them, and marks the survivors as refreshing again. `apply`
    /// calls it after every registry replacement.
    pub fn reconcile_pending_refreshes(&mut self) {
        let accounts = &self.accounts;
        self.pending_refreshes
            .retain(|id, p| accounts.get(id).is_some_and(|a| a.source == p.source));
        let pending: Vec<AccountId> = self.pending_refreshes.keys().cloned().collect();
        for id in &pending {
            if let Some(a) = self.account_mut(id) {
                a.usage.freshness.phase = Freshness::Refreshing;
            }
        }
    }

    /// Starts a validation of `id`'s credential source and makes it the one
    /// whose progress the status line shows. A validation already out for the
    /// account is replaced: only the reply echoing the returned ticket is
    /// accepted. `None` when the account is missing (nothing changes then).
    pub fn validate_account(&mut self, id: &str) -> Option<Ticket> {
        let source = self.accounts.get(id)?.source.clone();
        let started_ms = self.clock.now_ms;
        let ticket = self.next_ticket();
        self.accounts.get_mut(id)?.validation = ValidationState::Validating {
            started_tick: started_ms.max(0) as u64,
        };
        self.pending_validations.insert(
            id.to_owned(),
            PendingValidation {
                ticket,
                source: source.clone(),
                started_ms,
            },
        );
        self.displayed_validation = Some((id.to_owned(), ticket));
        self.perform(Effect::ValidateAccount {
            ticket,
            account: id.to_owned(),
            source,
        });
        Some(ticket)
    }

    /// The ticket of the validation `id` is waiting on, if one is out.
    pub fn validation_pending(&self, id: &str) -> Option<Ticket> {
        self.pending_validations.get(id).map(|p| p.ticket)
    }

    /// Whether the status line's validation progress belongs to this request.
    pub fn validation_owns_status(&self, id: &str, ticket: Ticket) -> bool {
        self.displayed_validation
            .as_ref()
            .is_some_and(|(account, shown)| account == id && *shown == ticket)
    }

    /// Drops the pending validations whose account is gone or whose
    /// credential source changed, and marks the survivors as validating again
    /// from their original start. Clears the displayed pair when its request
    /// was dropped. `apply` calls it after every registry replacement.
    pub fn reconcile_pending_validations(&mut self) {
        let accounts = &self.accounts;
        self.pending_validations
            .retain(|id, p| accounts.get(id).is_some_and(|a| a.source == p.source));
        for (id, p) in &self.pending_validations {
            if let Some(a) = self.accounts.get_mut(id) {
                a.validation = ValidationState::Validating {
                    started_tick: p.started_ms.max(0) as u64,
                };
            }
        }
        let pending = &self.pending_validations;
        if let Some((id, ticket)) = &self.displayed_validation
            && pending.get(id).is_none_or(|p| p.ticket != *ticket)
        {
            self.displayed_validation = None;
        }
    }

    /// Accepts a validation reply once: only while it echoes the ticket and
    /// source of the account's pending validation and the account still has
    /// that source. Retires the pending entry either way it matches.
    fn accept_validation(
        &mut self,
        account: &str,
        ticket: Ticket,
        source: &CredentialSource,
    ) -> bool {
        let pending_matches = self
            .pending_validations
            .get(account)
            .is_some_and(|p| p.ticket == ticket && p.source == *source);
        if !pending_matches {
            return false;
        }
        self.pending_validations.remove(account);
        self.accounts
            .get(account)
            .is_some_and(|a| a.source == *source)
    }

    pub fn refresh(&mut self) -> bool {
        let now = self.now_secs();
        if self.refresh_from.is_some() && now - self.refresh_started_secs < REFRESH_TIMEOUT_SECS {
            return false;
        }
        self.request_refresh(now);
        true
    }

    pub fn refresh_now(&mut self) {
        let now = self.now_secs();
        self.request_refresh(now);
    }

    fn request_refresh(&mut self, now: i64) {
        let ticket = self.next_ticket();
        self.refresh_ticket = ticket;
        self.refresh_from = Some(self.snapshots_applied);
        self.refresh_started_secs = now;
        self.perform(Effect::Refresh { ticket });
    }

    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms
    }

    pub fn load_role(&mut self, name: &str) -> RoleLoad {
        match self.roles.iter().find(|r| r.name == name) {
            None => RoleLoad::Unregistered,
            Some(entry) if !entry.trusted => RoleLoad::NeedsTrust(Box::new(entry.clone())),
            Some(_) => {
                self.perform(Effect::LoadRole {
                    role: name.to_owned(),
                });
                RoleLoad::Requested
            }
        }
    }

    pub fn perform(&mut self, effect: Effect) {
        let backend = self.backend.clone();
        if let Some(done) = backend.perform(&effect, self) {
            match done.reply {
                Reply::Ready(msg) => self.schedule(done.after_ms, msg),
                Reply::Settle => self.enqueue(done.after_ms, Work::Settle(effect)),
            }
        }
    }

    pub fn schedule(&mut self, delay_ms: i64, msg: Msg) {
        self.enqueue(delay_ms, Work::Msg(msg));
    }

    fn enqueue(&mut self, delay_ms: i64, work: Work) {
        let due = self.clock.now_ms + delay_ms;
        self.jobs.push(Job { due_ms: due, work });
        self.jobs.sort_by_key(|j| j.due_ms);
    }

    /// The earliest reply that is due at the current time, without advancing
    /// the clock. A reply with no delay is delivered this way, so it does not
    /// wait for a tick (and arrives under a paused clock too). One reply is
    /// settled per call, so a snapshot is always computed from the world that
    /// already holds the reply before it.
    pub fn take_due(&mut self) -> Option<Msg> {
        let backend = self.backend.clone();
        while self
            .jobs
            .first()
            .is_some_and(|j| j.due_ms <= self.clock.now_ms)
        {
            let msg = match self.jobs.remove(0).work {
                Work::Msg(msg) => Some(msg),
                Work::Settle(effect) => backend.settle(&effect, self),
            };
            if msg.is_some() {
                return msg;
            }
        }
        None
    }

    pub fn health_view(&self) -> HealthView {
        let doctor = self.surfaces.doctor.as_ref();
        let listing = match self.daemon_health {
            DaemonHealth::Healthy => Health::Healthy,
            DaemonHealth::Stale => Health::Stale,
        };
        let (health, at_secs) = match doctor {
            Some(d) if d.failure().is_some() => (
                Health::Failing,
                d.failure().map_or(self.last_refresh_secs, |(_, at)| at),
            ),
            Some(d) => match d.report() {
                Some(r) if r.failing() > 0 || r.no_gateway() => (Health::Failing, r.at_secs),
                Some(r) if r.stale(self.now_secs()) => (Health::Stale, r.at_secs),
                Some(_) if listing == Health::Stale => (Health::Stale, self.last_refresh_secs),
                Some(r) => (Health::Healthy, r.at_secs),
                None => (listing, self.last_refresh_secs),
            },
            None => (listing, self.last_refresh_secs),
        };
        HealthView {
            noun: doctor.map_or_else(
                || self.surfaces.vocabulary.health_noun.clone(),
                |d| d.noun.clone(),
            ),
            health,
            at_secs,
        }
    }

    /// Starts a doctor run unless one is out.
    pub fn start_doctor(&mut self) -> bool {
        let started = self.surfaces.doctor.as_mut().is_some_and(|d| d.start());
        if started {
            self.perform(Effect::RunDoctor);
        }
        started
    }

    pub fn about_props(&self) -> Vec<junie_tui::widgets::props::Prop> {
        use junie_tui::widgets::props::Prop;
        self.surfaces
            .about
            .iter()
            .map(|(label, value)| Prop::new(label, value.as_str()))
            .collect()
    }

    pub fn docker_on(&self, ws: Option<WorkspaceId>, role: &str) -> bool {
        self.surfaces.docker.as_ref().is_some_and(|d| {
            d.role_enabled.contains(role)
                || ws
                    .and_then(|id| self.workspace(id))
                    .is_some_and(|w| w.docker)
        })
    }

    pub fn apply(&mut self, snapshot: Snapshot) {
        let Snapshot {
            workspaces,
            roles,
            instances,
            global,
            accounts,
            surfaces,
            github,
            index_error,
            daemon_health,
        } = snapshot;
        self.snapshots_applied += 1;
        let before = self.projection.epochs;
        self.workspaces = workspaces;
        self.roles = roles;
        self.instances = instances;
        self.global = global;
        self.accounts = accounts;
        self.reconcile_pending_refreshes();
        self.reconcile_pending_validations();
        let doctor_state = self.surfaces.doctor.take().map(|d| d.state);
        self.surfaces = surfaces;
        if let (Some(doctor), Some(state)) = (self.surfaces.doctor.as_mut(), doctor_state) {
            doctor.state = state;
        }
        self.github = github;
        self.daemon_health = daemon_health;
        refresh_snapshots(self);
        self.projection.epochs.bump_all_from(before);
        self.arbiter.discovery = match index_error {
            Some(error) => Err(error),
            None => Ok(self.running_count()),
        };
    }

    /// Takes in what a reply says about the world itself, before any screen
    /// sees the message.
    pub fn absorb(&mut self, msg: &Msg) -> Absorbed {
        let warning = match msg {
            Msg::WorkspaceSaved {
                result: Ok(saved), ..
            }
            | Msg::GlobalSaved { result: Ok(saved) }
            | Msg::WorkspaceDeleted {
                result: Ok(saved), ..
            }
            | Msg::AccountsSaved {
                result: Ok(saved), ..
            }
            | Msg::RoleBound {
                result: Ok(saved), ..
            }
            | Msg::RoleRemoved {
                result: Ok(saved), ..
            } => saved.warning,
            Msg::RoleAdded {
                result: Ok((_, saved)),
                ..
            } => saved.warning,
            _ => None,
        };
        let stale_save = matches!(
            msg,
            Msg::AccountsSaved { ticket, result: Ok(_), .. } if *ticket <= self.saved_ticket
        );
        if warning.is_some() && !stale_save {
            self.committed_save_warning = warning;
            self.committed_save_status = None;
        }
        let mut absorbed = Absorbed::Current;
        match msg {
            Msg::DoctorFinished(result) => {
                let now = self.now_secs();
                if let Some(d) = self.surfaces.doctor.as_mut() {
                    d.finish(result, now);
                }
            }
            Msg::RoleLoaded {
                role,
                result: Err(error),
            } => {
                if let Some(r) = self.roles.iter_mut().find(|r| r.name == *role) {
                    r.load_error = Some(error.clone());
                }
            }
            Msg::Refreshed { ticket, result } => {
                let current = *ticket == self.refresh_ticket
                    && self
                        .refresh_from
                        .take()
                        .is_none_or(|from| from == self.snapshots_applied);
                match result {
                    Ok(snapshot) if current => self.apply((**snapshot).clone()),
                    Err(_) if current => self.daemon_health = DaemonHealth::Stale,
                    Ok(_) | Err(_) => absorbed = Absorbed::Stale,
                }
            }
            Msg::AccountsSaved {
                ticket,
                result: Ok(_),
                ..
            } => {
                if *ticket <= self.saved_ticket {
                    absorbed = Absorbed::Stale;
                } else {
                    self.saved_ticket = *ticket;
                }
            }
            Msg::LaunchCancelled { instance } => self.admit_entry((**instance).clone()),
            Msg::AccountRefreshed {
                ticket,
                account,
                source,
                result,
            } => {
                let owns = self
                    .accounts
                    .get(account)
                    .is_some_and(|a| a.source == **source)
                    && self
                        .pending_refreshes
                        .get(account)
                        .is_some_and(|p| p.ticket == *ticket && p.source == **source);
                if !owns {
                    return Absorbed::Stale;
                }
                self.pending_refreshes.remove(account);
                let now = self.now_secs();
                if let Some(a) = self.account_mut(account) {
                    match result {
                        Ok(refresh) => {
                            a.usage = refresh.usage.clone();
                            a.issue = refresh.issue.clone();
                            a.last_refresh_secs = Some(refresh.last_refresh_secs);
                        }
                        Err(error) => {
                            a.usage.freshness.phase = if a.usage.freshness.last_good_secs.is_some()
                            {
                                Freshness::Stale
                            } else {
                                Freshness::Failed
                            };
                            a.issue = Some(RecoverableIssue::new(
                                IssueCode::ProviderUnavailable,
                                IssueReason::Usage(UsageReason::Provider {
                                    detail: error.clone(),
                                }),
                                Recoverability::Retryable,
                            ));
                            a.last_refresh_secs = Some(now);
                        }
                    }
                }
            }
            Msg::AccountValidated {
                ticket,
                account,
                source,
                result,
            } => {
                if !self.accept_validation(account, *ticket, source) {
                    return Absorbed::Stale;
                }
                let now = self.now_secs();
                if let Some(a) = self.accounts.get_mut(account) {
                    self.projection.epochs.bump_accounts();
                    match result {
                        Ok(validation) => match validation {
                            Validation::Fingerprint { level } => {
                                a.validation = ValidationState::Valid(*level);
                                a.last_refresh_secs = Some(now);
                            }
                            Validation::Checked(outcome) => {
                                let windows = a.usage.windows.clone();
                                apply_validation(a, outcome, now);
                                if a.usage.windows.is_empty() {
                                    a.usage.windows = windows;
                                }
                            }
                        },
                        Err(error) => {
                            a.validation = ValidationState::Invalid(RecoverableIssue::new(
                                IssueCode::ProviderUnavailable,
                                IssueReason::Usage(UsageReason::Provider {
                                    detail: error.clone(),
                                }),
                                Recoverability::Retryable,
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
        absorbed
    }

    pub fn note_committed_save_warning(&mut self, warning: SaveWarning) {
        self.committed_save_warning = Some(warning);
        self.committed_save_status = None;
    }

    pub fn dismiss_committed_save_warning(&mut self) {
        self.committed_save_warning = None;
        self.committed_save_status = None;
    }

    /// Advances time and delivers the due messages. Hosts that also want to
    /// know which panes changed use [`World::tick_with_changes`].
    pub fn tick(&mut self, interval_ms: i64) -> Vec<Msg> {
        self.tick_with_changes(interval_ms).messages
    }

    /// One poll of every live link. A quiet poll changes nothing and asks for
    /// nothing: the result names an instance only when its link drained
    /// something new, and a summary only when the published daemon summary
    /// differs from the one before.
    pub fn tick_with_changes(&mut self, interval_ms: i64) -> WorldTick {
        match &self.time {
            TimeSource::Sim => self.clock.advance(interval_ms),
            TimeSource::Wall(source) if self.clock.running => {
                self.clock.now_ms = source.epoch_millis() - self.clock.epoch_secs * 1000;
                self.clock.utc_offset_secs = source.utc_offset_secs();
            }
            TimeSource::Wall(_) => {}
        }
        if !self.clock.running {
            return WorldTick::default();
        }
        let now = self.clock.now_ms;
        if self
            .surfaces
            .doctor
            .as_ref()
            .is_some_and(|d| d.due(self.clock.now_secs()))
        {
            self.start_doctor();
        }
        let roster = self.roster();
        let mut panes_changed = Vec::new();
        for (id, d) in self.daemons.iter_mut() {
            self.projection.work.count_running_lookup();
            if !roster.is_running(id) {
                continue;
            }
            self.projection.work.count_link_poll();
            if d.tick(now) {
                panes_changed.push(id.clone());
            }
        }
        let mut summaries_changed = Vec::new();
        for id in &panes_changed {
            if refresh_snapshot(self, id) {
                summaries_changed.push(id.clone());
            }
        }
        let backend = self.backend.clone();
        WorldTick {
            messages: backend.due(self),
            panes_changed,
            summaries_changed,
        }
    }

    /// Drains the pane of a wake and, when that changed it, republishes the
    /// daemon summary the way a tick does. Whether it has anything new to draw.
    pub fn wake_pane(&mut self, instance: &InstanceId, pane: PaneId) -> bool {
        let now = self.now_ms();
        let changed = self
            .daemons
            .get_mut(instance)
            .and_then(|d| d.pane_mut(pane))
            .is_some_and(|p| p.body.tick(now));
        if changed {
            refresh_snapshot(self, instance);
        }
        changed
    }

    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    /// A workspace for editing a label or setting; the roster epoch moves
    /// because the caller is about to change what its row shows.
    pub fn workspace_mut(&mut self, id: WorkspaceId) -> Option<&mut Workspace> {
        self.projection.epochs.bump_roster();
        self.workspaces.iter_mut().find(|w| w.id == id)
    }

    pub fn cwd_workspace(&self) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| {
            w.mounts.iter().any(|m| {
                m.source_label() == self.cwd || expand(&self.home, m.source_label()) == self.cwd
            })
        })
    }

    pub fn workspace_label(&self, i: &Instance) -> String {
        i.workspace
            .and_then(|id| self.workspace(id))
            .map(|ws| ws.name.clone())
            .unwrap_or_else(|| {
                self.cwd
                    .rsplit('/')
                    .next()
                    .unwrap_or("directory")
                    .to_owned()
            })
    }

    pub fn workspace_repo(&self, id: WorkspaceId) -> Option<&RepoRef> {
        self.workspace(id)?.github.as_ref()
    }

    pub fn instance(&self, id: &InstanceId) -> Option<&Instance> {
        self.instances
            .iter()
            .find(|entry| entry.id() == id)
            .and_then(InstanceEntry::as_available)
    }

    /// An instance for editing anything, status and workspace included; the
    /// membership epoch moves because the caller is about to change it.
    pub fn instance_mut(&mut self, id: &InstanceId) -> Option<&mut Instance> {
        self.projection.epochs.bump_membership();
        self.instances
            .iter_mut()
            .find(|entry| entry.id() == id)
            .and_then(InstanceEntry::as_available_mut)
    }

    /// An instance for editing only what its row and card show: dirty counts
    /// and the daemon summary. Status, workspace and id must stay as they are,
    /// because only the roster epoch moves.
    pub fn instance_view_mut(&mut self, id: &InstanceId) -> Option<&mut Instance> {
        self.projection.epochs.bump_roster();
        self.instances
            .iter_mut()
            .find(|entry| entry.id() == id)
            .and_then(InstanceEntry::as_available_mut)
    }

    pub fn entry(&self, id: &InstanceId) -> Option<&InstanceEntry> {
        self.instances.iter().find(|entry| entry.id() == id)
    }

    pub fn admit(&mut self, instance: Instance) {
        self.admit_entry(InstanceEntry::available(instance));
    }

    pub fn admit_entry(&mut self, entry: InstanceEntry) {
        if let Some(index) = self
            .instances
            .iter()
            .position(|known| known.id() == entry.id())
        {
            self.instances[index] = entry;
        } else {
            self.instances.push(entry);
        }
        self.projection.epochs.bump_membership();
        self.sync_arbiter();
    }

    pub fn running_count(&self) -> usize {
        self.instances
            .iter()
            .filter(|entry| entry_is_running(entry))
            .count()
    }

    pub fn running_count_of(&self, ws: Option<WorkspaceId>) -> usize {
        self.entries_of(ws)
            .into_iter()
            .filter(|entry| entry_is_running(entry))
            .count()
    }

    pub fn running_entries(&self) -> Vec<&InstanceEntry> {
        self.instances
            .iter()
            .filter(|entry| entry_is_running(entry))
            .collect()
    }

    pub fn instances_of(&self, ws: Option<WorkspaceId>) -> Vec<&Instance> {
        self.entries_of(ws)
            .into_iter()
            .filter_map(InstanceEntry::as_available)
            .filter(|i| !i.status.hidden())
            .collect()
    }

    pub fn entries_of(&self, ws: Option<WorkspaceId>) -> Vec<&InstanceEntry> {
        self.instances
            .iter()
            .filter(|entry| entry.workspace_id() == ws)
            .filter(|entry| {
                entry
                    .as_available()
                    .is_none_or(|instance| !instance.status.hidden())
            })
            .collect()
    }

    pub fn sync_arbiter(&mut self) {
        if self.arbiter.discovery.is_ok() {
            let n = self.running_count();
            self.arbiter.set_running(n);
        }
    }

    pub fn mask_path(&self, path: &str) -> String {
        mask_path(&self.home, path)
    }

    pub fn tilde(&self, path: &str) -> String {
        match under_home(&self.home, path) {
            Some(rest) => format!("~{rest}"),
            None => path.to_owned(),
        }
    }

    pub fn config_file(&self) -> String {
        self.tilde(self.surfaces.config_path.as_deref().unwrap_or_default())
    }

    pub fn account_for(
        &self,
        tuple: TupleIdentity,
        ws: Option<&Workspace>,
        role: Option<&str>,
        session: Option<&AccountId>,
    ) -> crate::domain::precedence::ResolvedAccount {
        let (order, binding) = match &self.surfaces.roles {
            Some(admin) => (
                admin.order,
                role.and_then(|r| admin.binding(r, tuple.agent)),
            ),
            None => (AccountOrder::Demo, None),
        };
        crate::domain::precedence::resolve_account(
            tuple.provider,
            ws,
            role,
            session.map(|s| (s, self.surfaces.vocabulary.session_choice.as_str())),
            &self.accounts,
            order,
            binding,
        )
    }

    pub fn agent_mode(&self, agent: Agent) -> AuthMode {
        self.global
            .agent_modes
            .get(&agent)
            .copied()
            .unwrap_or(AuthMode::Sync)
    }

    pub fn eligible_accounts(
        &self,
        agent: Agent,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> Vec<AccountId> {
        let offer = self.offer_for(agent, ws, role);
        offer.accounts
    }

    pub fn offer_for(
        &self,
        agent: Agent,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> AgentOffer {
        self.offer_for_provider(agent, agent.provider(), ws, role)
    }

    pub fn offer_for_provider(
        &self,
        agent: Agent,
        provider: crate::domain::agent::Provider,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> AgentOffer {
        let mut ready: Vec<AccountId> = vec![];
        let mut blocked: Vec<(AccountId, String)> = vec![];
        match ws {
            Some(w) => {
                for e in w.effective_accounts(&self.accounts) {
                    if e.provider != provider {
                        continue;
                    }
                    match &e.usable {
                        Usability::Ready => ready.push(e.id.clone()),
                        u => blocked.push((
                            e.id.clone(),
                            u.label(self.surfaces.fixed.as_ref(), &self.surfaces.vocabulary),
                        )),
                    }
                }
            }
            None => {
                if let Some(a) = self.accounts.default_for(provider) {
                    self.place(a, &mut ready, &mut blocked);
                }
                if ready.is_empty()
                    && let Some(a) = self.accounts.discovered_current(provider)
                {
                    self.place(a, &mut ready, &mut blocked);
                }
            }
        }
        if let Some(limit) = &self.surfaces.agent_blocked
            && agent != limit.launchable
        {
            return AgentOffer {
                accounts: vec![],
                preselected: None,
                blocked: Some(limit.text.clone()),
                configured: !ready.is_empty() || !blocked.is_empty(),
            };
        }
        if self.agent_mode(agent) == AuthMode::Ignore {
            return AgentOffer {
                accounts: vec![],
                preselected: None,
                blocked: Some("agent runtime mode is ignore · set in Settings › Agents".into()),
                configured: !ready.is_empty() || !blocked.is_empty(),
            };
        }

        if ws.is_none()
            && ready.is_empty()
            && self.accounts.default_for(provider).is_none()
            && let Some(only) = self.accounts.only_ready(provider)
        {
            ready.push(only.id.clone());
        }
        let resolved = self.account_for(TupleIdentity::new(agent, provider), ws, role, None);
        let explicit = resolved.account.filter(|_| resolved.explicit);
        let preselected = match &explicit {
            Some(id) => ready.contains(id).then(|| id.clone()),
            None => match ready.as_slice() {
                [only] => Some(only.clone()),
                _ => None,
            },
        };
        if let Some(p) = &preselected
            && let Some(i) = ready.iter().position(|x| x == p)
            && i > 0
        {
            let x = ready.remove(i);
            ready.insert(0, x);
        }
        let explicit_blocked = blocked.iter().find(|(id, _)| explicit.as_ref() == Some(id));
        let block = if preselected.is_none() && (ready.len() <= 1 || explicit_blocked.is_some()) {
            explicit_blocked.or(blocked.first()).map(|(id, why)| {
                let launch_block = self.accounts.get(id).and_then(|a| a.launch_blocked);
                if launch_block.is_some_and(|b| b.text(self.surfaces.fixed.as_ref()) == *why) {
                    return why.clone();
                }
                let name = self
                    .accounts
                    .get(id)
                    .map(|a| a.title())
                    .unwrap_or(id.clone());
                format!("{name} · {why}")
            })
        } else {
            None
        };
        AgentOffer {
            configured: !ready.is_empty() || !blocked.is_empty(),
            accounts: ready,
            preselected,
            blocked: block,
        }
    }

    fn place(
        &self,
        a: &crate::domain::account::Account,
        ready: &mut Vec<AccountId>,
        blocked: &mut Vec<(AccountId, String)>,
    ) {
        if ready.contains(&a.id) || blocked.iter().any(|(id, _)| *id == a.id) {
            return;
        }
        match crate::domain::workspace::usability_of(a) {
            Usability::Ready => ready.push(a.id.clone()),
            u => blocked.push((
                a.id.clone(),
                u.label(self.surfaces.fixed.as_ref(), &self.surfaces.vocabulary),
            )),
        }
    }

    pub fn role_launchable(&self, name: &str) -> bool {
        self.roles.iter().any(|r| r.name == name && r.launchable())
    }

    pub fn launch_role(&self, from: LaunchSource) -> LaunchRole {
        match self.surfaces.launch_role {
            LaunchRoleRule::Demo => LaunchRole::Role(match from {
                LaunchSource::Row(ws) => ws
                    .and_then(|x| x.roles.default.clone().or(x.roles.last.clone()))
                    .unwrap_or_else(|| "the-architect".into()),
                LaunchSource::CurrentDir(_) => "the-architect".into(),
            }),
            LaunchRoleRule::Configured => {
                let (LaunchSource::Row(ws) | LaunchSource::CurrentDir(ws)) = from;
                if let Some(default) = ws.and_then(|x| x.roles.default.clone()) {
                    return if self.role_launchable(&default) {
                        LaunchRole::Role(default)
                    } else {
                        LaunchRole::Unavailable(DEFAULT_ROLE_UNUSABLE.into())
                    };
                }
                let mut allowed = self
                    .roles
                    .iter()
                    .filter(|r| r.launchable())
                    .filter(|r| ws.is_none_or(|x| x.roles.allows(&r.name)))
                    .map(|r| r.name.clone());
                match (allowed.next(), allowed.next()) {
                    (Some(only), None) => LaunchRole::Role(only),
                    _ if ws.is_some() => LaunchRole::Unavailable(NO_DEFAULT_ROLE.into()),
                    _ => LaunchRole::Unavailable(NO_SAVED_WORKSPACE.into()),
                }
            }
        }
    }

    pub fn offered_agents(
        &self,
        ws: Option<&Workspace>,
        role: Option<&str>,
    ) -> Vec<(Agent, AgentOffer)> {
        Agent::ALL
            .iter()
            .map(|a| (*a, self.offer_for(*a, ws, role)))
            .filter(|(_, o)| o.configured)
            .collect()
    }
}

fn entry_is_running(entry: &InstanceEntry) -> bool {
    match entry {
        InstanceEntry::Available(instance) => instance.status == InstanceStatus::Running,
        InstanceEntry::Unavailable(instance) => instance.phase.ready(),
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LaunchSource<'a> {
    Row(Option<&'a Workspace>),
    CurrentDir(Option<&'a Workspace>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOffer {
    pub accounts: Vec<AccountId>,
    pub preselected: Option<AccountId>,

    pub blocked: Option<String>,

    pub configured: bool,
}

impl AgentOffer {
    pub fn has_usable_account(&self) -> bool {
        self.blocked.is_none() && !self.accounts.is_empty()
    }
}

pub fn expand(home: &str, path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        format!("{home}{rest}")
    } else {
        path.to_owned()
    }
}

fn under_home<'a>(home: &str, path: &'a str) -> Option<&'a str> {
    path.strip_prefix(home.trim_end_matches('/'))
        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
}

pub fn mask_path(home: &str, path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    if let Some(rest) = under_home(home, path) {
        let segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
        match segs.len() {
            0 => "~".into(),
            1 => format!("~/{}", segs[0]),
            _ => format!("~/…/{last}"),
        }
    } else if path.starts_with('/') {
        format!("…/{last}")
    } else {
        path.to_owned()
    }
}

/// Republishes one daemon's summary on its instance record. The roster epoch
/// moves only when the summary differs from the published one, so output that
/// changes a grid and nothing else costs no projection. Whether it moved.
pub fn refresh_snapshot(w: &mut World, id: &InstanceId) -> bool {
    let Some(snapshot) = w.daemons.get(id).map(Daemon::snapshot) else {
        return false;
    };
    if w.instance(id).is_none_or(|i| i.daemon == snapshot) {
        return false;
    }
    if let Some(i) = w.instance_view_mut(id) {
        i.daemon = snapshot;
    }
    true
}

pub fn refresh_snapshots(w: &mut World) {
    let ids: Vec<InstanceId> = w.daemons.keys().cloned().collect();
    for id in &ids {
        refresh_snapshot(w, id);
    }
    let mut changed = false;
    for entry in w.instances.iter_mut() {
        if let Some(i) = entry.as_available_mut()
            && i.status != InstanceStatus::Running
            && i.daemon != DaemonSnapshot::Unavailable
        {
            i.daemon = DaemonSnapshot::Unavailable;
            changed = true;
        }
    }
    if changed {
        w.projection.epochs.bump_roster();
    }
}
