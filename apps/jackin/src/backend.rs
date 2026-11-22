use std::sync::Arc;

use crate::domain::account::{
    Account, AccountIdentity, Confidence, CredentialSource, DetectedKind, IssueCode, Lifecycle,
    RecoverableIssue, ValidationLevel,
};
use crate::domain::account::{AccountId, AccountRegistry};
use crate::domain::agent::{Agent, Provider};
use crate::domain::instance::{Instance, InstanceId};
use crate::domain::launch::LaunchContent;
use crate::domain::onepassword::OpReference;
use crate::domain::reason::{Found, IssueReason, RefreshOutcome};
use crate::domain::role::RoleManifestDoc;
use crate::domain::shown::is_hidden;
use crate::domain::tuple::{AccountRequirement, TupleIdentity};
use crate::domain::usage::AccountUsage;
use crate::domain::vocabulary::ReasonWords;
use crate::domain::workspace::{RoleName, Workspace, WorkspaceId};
use crate::launch::{LaunchFeed, LaunchTarget};
use crate::panes::{PaneBody, PaneKind, PaneRefusal, PaneRequest, PaneSpawn, PaneTiming};
use crate::world::{GlobalConfig, Msg, World};

pub use termrock::widgets::diff::UnreadFile;
pub use termrock::widgets::diff::{
    DiffFile as ChangedFile, DiffHunk as Hunk, DiffLine, DiffStatus,
};
pub use termrock::widgets::file_browser::FileSource;
use termrock::widgets::file_browser::{FileEntry, MemorySource};
pub use termrock::widgets::inspect_changes::{ChangeSet, ChangeWords};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationOutcome {
    pub level: Option<ValidationLevel>,
    pub identity: AccountIdentity,
    pub confidence: Confidence,
    pub lifecycle: Lifecycle,
    pub issue: Option<RecoverableIssue>,
    pub usage: Option<AccountUsage>,

    pub material: CheckRow,
    pub identity_row: CheckRow,
    pub quota_row: CheckRow,
}

/// What a refresh of one account found: the usage and the issue the account
/// has afterwards, when it was refreshed, and a word for the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refresh {
    pub usage: AccountUsage,
    pub issue: Option<RecoverableIssue>,
    pub last_refresh_secs: i64,
    pub outcome: RefreshOutcome,
}

/// What validating one account found. A key that is held as a fingerprint is
/// checked against that fingerprint alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validation {
    Fingerprint { level: ValidationLevel },
    Checked(Box<ValidationOutcome>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckRow {
    Ok(Found),
    Failed(IssueReason),
    Skipped(IssueReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderProbe {
    Missing,
    Unreadable,

    NoCredential,

    Malformed,
    Found(DetectedKind),
}

/// Names one request and its reply, so a reply that arrives after the screen
/// has moved on is told from the one it waits for. `World::next_ticket` hands
/// them out.
pub type Ticket = u64;

/// A key typed into a form. It travels in an effect so the backend can check
/// it, and it never appears in `Debug` output.
#[derive(Clone, PartialEq, Eq)]
pub struct PlainKey(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRefusal {
    Empty,
    Unprintable,
}

impl std::fmt::Display for KeyRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            KeyRefusal::Empty => "API key required: the field is empty",
            KeyRefusal::Unprintable => "The API key holds a control or direction character",
        })
    }
}

impl PlainKey {
    pub fn new(key: &str) -> Result<Self, KeyRefusal> {
        let key = key.trim();
        if key.is_empty() {
            return Err(KeyRefusal::Empty);
        }
        if key.chars().any(is_hidden) {
            return Err(KeyRefusal::Unprintable);
        }
        Ok(Self(key.to_owned()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl Drop for PlainKey {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
        std::hint::black_box(&bytes);
    }
}

impl std::fmt::Debug for PlainKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlainKey(…)")
    }
}

pub fn apply_validation(a: &mut Account, v: &ValidationOutcome, now: i64) {
    a.validation = match (&v.level, &v.issue) {
        (Some(l), None) => crate::domain::account::ValidationState::Valid(*l),
        (Some(l), Some(i))
            if i.is_informational()
                || i.code == IssueCode::QuotaUnsupported
                || i.code == IssueCode::RateLimited =>
        {
            crate::domain::account::ValidationState::Valid(*l)
        }
        (_, Some(i)) => crate::domain::account::ValidationState::Invalid(i.clone()),
        (None, None) => crate::domain::account::ValidationState::NeverValidated,
    };
    a.identity = v.identity.clone();
    a.confidence = v.confidence;
    a.lifecycle = v.lifecycle;
    a.issue = v.issue.clone();
    a.last_refresh_secs = Some(now);
    if let Some(u) = &v.usage {
        if u.windows.is_empty() {
            a.usage.freshness = u.freshness.clone();
        } else {
            a.usage = u.clone();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    Valid,
    Rejected,
    RateLimited,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretClass {
    Empty,
    Unrecognised,
    Key {
        provider: Provider,
        outcome: KeyOutcome,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSession {
    SignedIn,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpAccountState {
    Available,
    Locked,
    AuthorizationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultAccess {
    ReadWrite,
    ReadOnly,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Concealed,
    Text,
    Url,
}

impl FieldKind {
    pub fn label(self) -> &'static str {
        match self {
            FieldKind::Concealed => "concealed",
            FieldKind::Text => "text",
            FieldKind::Url => "url",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OpField {
    pub id: String,
    pub label: String,
    pub section: Option<String>,
    pub kind: FieldKind,

    pub material: Option<String>,

    pub tail: String,
}

#[derive(Debug, Clone)]
pub struct OpItem {
    pub id: String,
    pub title: String,
    pub category: &'static str,
    pub fields: Vec<OpField>,
}

#[derive(Debug, Clone)]
pub struct OpVault {
    pub id: String,
    pub name: String,
    pub access: VaultAccess,
    pub items: Vec<OpItem>,
}

#[derive(Debug, Clone)]
pub struct OpAccount {
    pub id: String,
    pub email: String,
    pub state: OpAccountState,
    pub vaults: Vec<OpVault>,
}

/// What the 1Password picker asks the backend to list. Each step names the
/// ids of the steps before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpScope {
    Accounts,
    Vaults {
        account: String,
    },
    Items {
        account: String,
        vault: String,
    },
    Fields {
        account: String,
        vault: String,
        item: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpAccountRow {
    pub id: String,
    pub email: String,
    pub state: OpAccountState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpVaultRow {
    pub id: String,
    pub name: String,
    pub items: usize,
    pub access: Option<VaultAccess>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpItemRow {
    pub id: String,
    pub title: String,
    pub category: String,
    pub fields: Option<usize>,
}

/// A field as the picker shows it: never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpFieldRow {
    pub id: String,
    pub label: String,
    pub section: Option<String>,
    pub kind: FieldKind,
}

/// The reply to one `OpScope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpListing {
    Accounts(Vec<OpAccountRow>),
    Vaults(Vec<OpVaultRow>),
    Items(Vec<OpItemRow>),
    Fields(Vec<OpFieldRow>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpError {
    Locked,
    NotAvailable,
    AuthorizationRequired { account: String },
    PermissionDenied { vault: String },
    MissingAccount { account: String },
    MissingVault { vault: String },
    MissingItem { item: String, vault: String },
    MissingField { field: String, item: String },
    EmptyMaterial { field: String },
    WrongFieldShape { field: String },
}

impl OpError {
    /// The words of `ReasonWords::openshell()`, for the places that hold no
    /// `Vocabulary` (the picker of a 1Password reference).
    pub fn message(&self) -> String {
        ReasonWords::openshell().op_error(self)
    }

    pub fn retryable(&self) -> bool {
        matches!(self, OpError::Locked)
    }
}

#[derive(Debug, Clone)]
pub struct FieldDescriptor {
    pub masked: String,
}

pub trait OnePassword: std::fmt::Debug {
    fn session(&self) -> OpSession;

    fn set_session(&mut self, session: OpSession);

    fn accounts(&self) -> Vec<OpAccount>;

    fn latency_ms(&self) -> i64;

    fn list_accounts(&self) -> Result<Vec<&OpAccount>, OpError>;

    fn list_vaults(&self, account: &str) -> Result<Vec<&OpVault>, OpError>;

    fn list_items(&self, account: &str, vault: &str) -> Result<Vec<&OpItem>, OpError>;

    fn list_fields(&self, account: &str, vault: &str, item: &str)
    -> Result<Vec<&OpField>, OpError>;

    fn reference(
        &self,
        account: &str,
        vault: &str,
        item: &str,
        field_id: &str,
        section: Option<&str>,
    ) -> Result<OpReference, OpError>;

    fn describe(&self, reference: &OpReference) -> Result<FieldDescriptor, OpError>;

    fn endpoint_of(&self, reference: &OpReference) -> Option<String>;

    fn classify(&self, reference: &OpReference) -> Result<SecretClass, OpError>;

    fn box_clone(&self) -> Box<dyn OnePassword>;
}

impl Clone for Box<dyn OnePassword> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    SaveWorkspace {
        id: Option<WorkspaceId>,
        base: Option<Box<Workspace>>,
        workspace: Box<Workspace>,
    },
    SaveGlobal {
        base: Box<GlobalConfig>,
        global: Box<GlobalConfig>,
    },
    SaveAccounts {
        ticket: Ticket,
        base: Box<AccountRegistry>,
        registry: Box<AccountRegistry>,
    },
    DeleteWorkspace {
        id: WorkspaceId,
    },
    Restore {
        instance: InstanceId,
    },
    PowerStart {
        instance: InstanceId,
    },
    PowerStop {
        instance: InstanceId,
    },
    TrustRole {
        role: RoleName,
    },
    Prewarm {
        workspace: WorkspaceId,
    },
    Stop {
        instance: InstanceId,
    },
    Purge {
        instance: InstanceId,
    },
    Refresh {
        ticket: Ticket,
    },
    RefreshAccount {
        /// The request's identity. `World::refresh_account` allocates it; the
        /// reply (`Msg::AccountRefreshed`) must echo it unchanged.
        ticket: Ticket,
        account: AccountId,
        source: CredentialSource,
        stagger_ms: i64,
    },
    ValidateAccount {
        /// The request's identity. `World::validate_account` allocates it; the
        /// reply (`Msg::AccountValidated`) must echo it unchanged.
        ticket: Ticket,
        account: AccountId,
        source: CredentialSource,
    },
    LoadRole {
        role: String,
    },
    CheckRole {
        ticket: Ticket,
        role: RoleName,
        doc: RoleManifestDoc,
    },
    SaveRole {
        role: RoleName,
        base: Box<RoleManifestDoc>,
        doc: Box<RoleManifestDoc>,
    },
    BindRole {
        role: RoleName,
        agent: Agent,
        base: Option<AccountId>,
        account: Option<AccountId>,
    },
    AddRole {
        location: String,
    },
    RemoveRole {
        role: RoleName,
    },
    RunDoctor,
    CheckSource {
        ticket: Ticket,
        provider: Provider,
        source: CredentialSource,
        key: Option<PlainKey>,
    },
    ProbeFolder {
        ticket: Ticket,
        path: String,
    },
    ReadChanges {
        ticket: Ticket,
        instance: InstanceId,
        touched: Vec<String>,
        uncommitted: usize,
        unpushed: usize,
    },
    LoadOp {
        ticket: Ticket,
        scope: OpScope,
    },
}

/// How a backend answers an effect that finishes later. `Ready` is a message
/// the backend has already built. `Settle` asks the world to call
/// `Backend::settle` when the reply falls due, so the reply is computed from
/// the world as it is then and not as it was when the effect started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Ready(Msg),
    Settle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub after_ms: i64,
    pub reply: Reply,
}

impl Completion {
    pub fn ready(after_ms: i64, msg: Msg) -> Self {
        Self {
            after_ms,
            reply: Reply::Ready(msg),
        }
    }

    pub fn settle(after_ms: i64) -> Self {
        Self {
            after_ms,
            reply: Reply::Settle,
        }
    }
}

pub trait Backend: std::fmt::Debug + Send + Sync {
    fn usage_connections(&self) -> Option<&dyn crate::usage_connections::UsageConnections> {
        None
    }

    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion>;

    /// Messages the backend raises on its own at the current time, such as a
    /// second client attaching. The world asks on every tick.
    fn due(&self, _world: &World) -> Vec<Msg> {
        vec![]
    }

    /// The reply to an effect whose `perform` returned `Completion::settle`,
    /// computed when it falls due. A backend that never settles keeps this.
    fn settle(&self, _effect: &Effect, _world: &World) -> Option<Msg> {
        None
    }

    fn launch_content(&self) -> Arc<LaunchContent>;

    fn account_requirement(&self, _tuple: TupleIdentity) -> AccountRequirement {
        AccountRequirement::Required
    }

    /// Starts a launch. The cockpit polls the feed it returns and draws what
    /// its events say.
    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed>;

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody>;

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        let lead = instance.agent;
        if instance.agents.is_empty() {
            return vec![PaneRequest::new(
                &instance.id,
                Some(lead),
                instance.account.clone(),
            )];
        }
        instance
            .agents
            .iter()
            .map(|a| {
                let account = a
                    .account
                    .clone()
                    .or_else(|| (a.agent == lead).then(|| instance.account.clone())?);
                PaneRequest::new(&instance.id, Some(a.agent), account)
            })
            .collect()
    }

    /// Whether a pane of `kind` may be started in `instance` now. An `Err` is
    /// the reason the new-tab picker shows on the disabled row, in the
    /// `PaneWords` of the world.
    fn spawnable(
        &self,
        _world: &World,
        _instance: &Instance,
        _kind: &PaneKind,
    ) -> Result<(), PaneRefusal> {
        Ok(())
    }

    /// The operator left the Capsule of `instance`: Ctrl+B d, a page of the
    /// console, or the end of the instance. Tabs, zoom, pickers and dialogs
    /// are not leaving. A backend whose panes keep a main process attached
    /// detaches them here. Once per departure, after `App::go` has taken the
    /// focus from every pane of that Capsule.
    fn capsule_left(&self, _world: &World, _instance: &InstanceId) {}

    /// How long the replay window of an attach stays open. A backend that
    /// builds `TerminalPane`s passes this to them.
    fn pane_timing(&self) -> PaneTiming {
        PaneTiming::default()
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        Box::new(
            MemorySource::new(
                world
                    .fs
                    .iter()
                    .map(|e| {
                        let meta = match (&e.git, e.meta.as_str()) {
                            (Some(b), _) => format!("git · {b}"),
                            (None, m) => m.to_owned(),
                        };
                        let entry = if e.dir {
                            FileEntry::dir(&e.path)
                        } else {
                            FileEntry::file(&e.path)
                        };
                        entry.meta(&meta)
                    })
                    .collect(),
            )
            .home(&world.home),
        )
    }
}
