use termrock::widgets::steps::StepState;

use crate::backend::{OnePassword, PlainKey};
use crate::domain::account::AccountId;
use crate::domain::agent::Agent;
use crate::domain::docker::DockerCapability;
use crate::domain::instance::InstanceEntry;
use crate::domain::launch::{LaunchContent, LaunchFailure, RefusalReason};
use crate::domain::workspace::WorkspaceId;
use crate::world::{SaveWarning, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchTarget {
    pub workspace: Option<WorkspaceId>,
    pub role: String,
    pub instances: Vec<crate::domain::tuple::InstanceChoice>,
    pub task: Option<String>,
}

impl LaunchTarget {
    pub fn agent(&self) -> Agent {
        self.instances.first().map_or(Agent::Codex, |i| i.agent)
    }
    pub fn account(&self) -> Option<AccountId> {
        self.instances.first().and_then(|i| i.account.clone())
    }
}

/// The instance a launch made, as its backend recorded it, and the run that
/// made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyInstance {
    pub instance: InstanceEntry,
    pub run_id: String,
    pub warning: Option<SaveWarning>,
}

/// Why a launch failed, and the `FailedSetup` record the backend kept of it.
/// A backend that kept none says so, and the roster gets no row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedLaunch {
    pub failure: LaunchFailure,
    pub instance: Option<InstanceEntry>,
}

/// A value the launch asks the operator for before it starts: the variable,
/// the question, and the answer the backend would use on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvQuestion {
    pub name: String,
    pub prompt: String,
    pub default: Option<String>,
}

/// What the operator answered to an `EnvQuestion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvAnswer {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchEvent {
    StageChanged(usize, StepState),
    /// How long a finished stage took. A backend that does not time its
    /// stages sends none, and the rail shows none.
    StageTimed {
        stage: usize,
        elapsed_ms: u64,
    },
    Activity(String),
    BuildLine(String),
    ContainerReady(String),
    CredentialsResolved {
        origin: String,
        validation: String,
    },

    CredentialError {
        message: String,
    },
    /// The launch cannot start. Nothing was created. The cockpit words the
    /// reason from `LaunchWords` and draws that and nothing else: there is no
    /// stage, no run to copy and no record.
    Refused(RefusalReason),
    /// The launch gives the agent a Docker engine. The cockpit draws the
    /// banner and lists the risks from this; without it, from
    /// `Surfaces.docker`.
    Warning(DockerCapability),
    /// The launch waits for these answers; the feed sends nothing more until
    /// `LaunchFeed::answer_env`, or `cancel`.
    NeedsInput(Vec<EnvQuestion>),
    Failed(Box<FailedLaunch>),
    Ready(Box<ReadyInstance>),
}

/// What the credentials line says once the operator chose to type the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedKey {
    pub origin: String,
    pub validation: String,
}

/// What a backend gives the cockpit for one launch. The cockpit polls it on
/// every step and draws what the events say; it records nothing in the world
/// except the instances the events and `cancel` hand it.
pub trait LaunchFeed: std::fmt::Debug + Send {
    /// The id of the run, shown from the first frame.
    fn run_id(&self) -> &str;

    /// The collector the run reports to. A backend that sends no telemetry
    /// names none, and the debug info shows no telemetry line.
    fn telemetry(&self) -> Option<&str> {
        None
    }

    /// The events that happened since the last poll, in order.
    fn poll(&mut self, world: &World) -> Vec<LaunchEvent>;

    /// The operator unlocked the credential source and asked to try again.
    /// `op` is the session the console holds; a backend that unlocks
    /// elsewhere leaves it alone.
    fn retry_credentials(&mut self, op: &mut dyn OnePassword);

    /// The operator chose to type the key instead of resolving it. The
    /// credentials line the backend wants shown for it, if it wants one.
    fn use_plain_text(&mut self, key: PlainKey) -> Option<TypedKey>;

    /// The operator answered the questions of `NeedsInput`, one answer for
    /// each, in the order they were asked.
    fn answer_env(&mut self, answers: Vec<EnvAnswer>);

    /// The operator ended the launch. The record the backend kept of the
    /// launch, if it kept one. A backend that must wait for the sandbox
    /// returns `None` and sends the record later as `Msg::LaunchCancelled`.
    fn cancel(&mut self, world: &World) -> Option<InstanceEntry>;
}

/// The accounts a launch offers: the usable ones the workspace enables, with
/// the requested one first when it is not among them, and for a content that
/// takes only the agent's provider, only those.
pub fn launch_accounts(
    w: &World,
    target: &LaunchTarget,
    content: &LaunchContent,
) -> Vec<AccountId> {
    let workspace = target.workspace.and_then(|id| w.workspace(id));
    let accounts: Vec<AccountId> = workspace
        .map(|x| {
            x.effective_accounts(&w.accounts)
                .into_iter()
                .filter(|e| e.usable.is_ready())
                .map(|e| e.id)
                .collect()
        })
        .unwrap_or_default();
    let mut selected: Vec<AccountId> = Vec::new();
    for account in target
        .instances
        .iter()
        .filter_map(|choice| choice.account.as_ref())
    {
        if !accounts.contains(account) && !selected.contains(account) {
            selected.push(account.clone());
        }
    }
    selected.extend(accounts);
    let mut accounts = selected;
    if content.agent_provider_only {
        let providers: Vec<_> = target
            .instances
            .iter()
            .map(|choice| choice.provider)
            .collect();
        accounts.retain(|id| {
            w.accounts
                .get(id)
                .is_some_and(|account| providers.contains(&account.provider))
        });
    }
    accounts
}
