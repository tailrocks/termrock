use std::sync::Arc;

use termrock::widgets::steps::StepState;

use crate::backend::{OnePassword, OpSession, PlainKey};
use crate::domain::account::{AccountId, masked};
use crate::domain::agent::Agent;
use crate::domain::instance::InstanceId;
use crate::domain::instance::{DaemonSnapshot, Instance, InstanceEntry, InstanceStatus};
use crate::domain::launch::{
    EmptyLanding, LaunchContent, LaunchFailure, LaunchWords, NextStep, RefusalReason, RefusalWords,
    StageKind, StageList, StageSpec,
};
use crate::domain::tuple::{AccountRequirement, TupleIdentity};
use crate::domain::workspace::WorkspaceId;
use crate::launch::{
    EnvAnswer, EnvQuestion, FailedLaunch, LaunchEvent, LaunchFeed, LaunchTarget, ReadyInstance,
    TypedKey, launch_accounts,
};
use crate::world::World;

/// What one cockpit tick stands for in the stage times the simulator reports.
const TICK_MS: u64 = 99;

pub const BUILD_LOG: [&str; 44] = [
    "#1 [internal] load build definition from Dockerfile.derived",
    "#1 transferring dockerfile: 2.31kB done",
    "#1 DONE 0.0s",
    "#2 [internal] load metadata for ghcr.io/jackin/construct:0.6.4",
    "#2 DONE 0.4s",
    "#3 [internal] load .dockerignore",
    "#3 transferring context: 2B done",
    "#3 DONE 0.0s",
    "#4 [ 1/9] FROM ghcr.io/jackin/construct:0.6.4@sha256:9c41e2f0a7b3c5d6e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1",
    "#4 resolve ghcr.io/jackin/construct:0.6.4 0.1s done",
    "#4 CACHED",
    "#5 [internal] load build context",
    "#5 transferring context: 41.02kB 0.0s done",
    "#5 DONE 0.0s",
    "#6 [ 2/9] RUN --mount=type=cache,target=/var/cache/apt apt-get update && apt-get install -y --no-install-recommends git-lfs ripgrep fd-find jq",
    "#6 0.812 Get:1 http://deb.debian.org/debian bookworm InRelease [151 kB]",
    "#6 1.204 Get:2 http://deb.debian.org/debian bookworm-updates InRelease [55.4 kB]",
    "#6 2.331 Reading package lists...",
    "#6 3.118 Building dependency tree...",
    "#6 3.902 The following NEW packages will be installed: fd-find git-lfs jq ripgrep",
    "#6 5.410 Setting up ripgrep (13.0.0-4+b2) ...",
    "#6 5.612 Setting up git-lfs (3.3.0-1+b5) ...",
    "#6 DONE 6.1s",
    "#7 [ 3/9] RUN corepack enable && corepack prepare pnpm@9.12.0 --activate",
    "#7 1.930 Preparing pnpm@9.12.0 for immediate activation...",
    "#7 DONE 2.4s",
    "#8 [ 4/9] COPY roles/the-architect/manifest.toml /jackin/role/manifest.toml",
    "#8 DONE 0.0s",
    "#9 [ 5/9] RUN /jackin/bin/role-install --manifest /jackin/role/manifest.toml",
    "#9 0.211 role: the-architect@1.4.2 (github.com/chainargos/roles)",
    "#9 0.480 install: cargo-nextest 0.9.72",
    "#9 4.115 install: sqlfluff 3.1.0",
    "#9 7.309 warning: hook post-install.sh exited 0 with output on stderr",
    "#9 DONE 7.6s",
    "#10 [ 6/9] RUN useradd --uid 501 --create-home operator && chown -R operator /workspace",
    "#10 DONE 0.5s",
    "#11 [ 7/9] COPY --chown=operator agent-home/ /home/operator/.claude/",
    "#11 DONE 0.1s",
    "#12 [ 8/9] RUN /jackin/bin/agent-install claude@2.1.14",
    "#12 0.104 claude 2.1.14 already present in layer cache",
    "#12 DONE 0.2s",
    "#13 [ 9/9] LABEL org.jackin.run=9c41e2f0 org.jackin.workspace=payments-platform",
    "#13 DONE 0.0s",
    "#14 exporting to image",
];

fn next_instance_id(w: &World) -> InstanceId {
    let n = w.instances.len() as u32 + 0x7f3a + 0x1234 * 3;
    InstanceId::simulated(format!("jk-{:04x}", n & 0xffff))
}

fn stages() -> StageList {
    StageList::new(
        StageSpec::new("identity", "Identity", "Resolving launch identity…", 14),
        [
            StageSpec::new("role", "Role", "Loading role manifest…", 18),
            StageSpec::new("credentials", "Credentials", "Resolving credentials…", 26)
                .kind(StageKind::Credentials { hold_at: 10 }),
            StageSpec::new(
                "construct",
                "Construct",
                "Preparing the Construct image…",
                30,
            )
            .container_at(6),
            StageSpec::new(
                "agent-binaries",
                "Agent Binaries",
                "Installing {agent} binaries…",
                8,
            )
            .skipped(),
            StageSpec::new(
                "derived-image",
                "Derived Image",
                "Building derived image…",
                92,
            )
            .kind(StageKind::BuildLog),
            StageSpec::new("workspace", "Workspace", "Mounting the Workspace…", 22),
            StageSpec::new("network", "Network", "Attaching the network…", 20),
            StageSpec::new("sidecar", "Sidecar", "Starting the sidecar…", 18),
            StageSpec::new("capsule", "Capsule", "Waiting for the Capsule daemon…", 34),
            StageSpec::new("hardline", "Hardline", "Opening the hardline…", 16),
        ],
    )
}

fn words() -> LaunchWords {
    LaunchWords {
        unit: "Container".into(),
        unit_missing: "not created".into(),
        log_title: "Docker build".into(),
        log_waiting: "(waiting for docker build output…)".into(),
        build_failed: "Docker build failed".into(),
        ready_activity: "Hardline open · handing off to the Capsule".into(),
        ready_not_running: "The container is {status}, not running".into(),
        credentials_resolved: "resolved in memory · discarded after injection".into(),
        refused_state: "nothing was created".into(),
        role_not_allowed: RefusalWords {
            title: "Role not allowed".into(),
            found: "Workspace {workspace} does not allow role {role}.".into(),
            next_step: "Allow it in the workspace's Roles tab, or load another role.".into(),
        },
        agent_not_launchable: RefusalWords {
            title: "Agent not launchable".into(),
            found: "{agent} cannot be launched here: only {launchable} is.".into(),
            next_step: "Launch with {launchable} instead.".into(),
        },
        tuple_not_launchable: RefusalWords {
            title: "Tuple not launchable".into(),
            found: "{agent} via {provider} is not launchable: {reason}".into(),
            next_step: "Choose a supported agent and provider, then launch again.".into(),
        },
        next_retry: "container {sandbox} remains; launch again with id {launch}".into(),
        next_doctor:
            "Check that Docker's jackin-net bridge exists (docker network ls), then launch again."
                .into(),
        next_credential: "Unlock the credential source and retry, or enter the key by hand.".into(),
    }
}

pub fn content() -> Arc<LaunchContent> {
    let stages = stages();
    let network = stages.position("network").unwrap_or(0);
    Arc::new(LaunchContent {
        stages,
        words: words(),
        build_log: BUILD_LOG.iter().map(|l| (*l).to_owned()).collect(),
        agent_provider_only: false,
        name_slug: None,
        when_empty: EmptyLanding::Outro,
        network_failure: LaunchFailure {
            stage: network,
            summary: "The Construct network could not be attached".into(),
            next_step: NextStep::RunDoctor,
            detail: vec![
                "network: attach jackin-net to jackin-payments-platform-7f3a".into(),
                "docker: Error response from daemon: network jackin-net not found".into(),
                "retry 1/3 after 400 ms: network jackin-net not found".into(),
                "retry 2/3 after 800 ms: network jackin-net not found".into(),
                "retry 3/3 after 1600 ms: network jackin-net not found".into(),
                "sidecar: not started (network stage failed)".into(),
                "capsule: not started".into(),
                "cleanup: container jackin-payments-platform-7f3a kept for inspection".into(),
                "credentials: material was resolved in memory and discarded".into(),
                "run: 9c41e2f0 · backend docker 27.1 · host darwin 25.5".into(),
            ],
        },
    })
}

pub const OPENSHELL_BUILD_LOG: [&str; 12] = [
    "#1 [internal] load build definition from Dockerfile",
    "#1 DONE 0.0s",
    "#2 [internal] load metadata for registry.jackin.local/construct:0.7.0",
    "#2 DONE 0.3s",
    "#3 [1/4] FROM registry.jackin.local/construct:0.7.0",
    "#3 CACHED",
    "#4 [2/4] COPY role/ /jackin/role/",
    "#4 DONE 0.1s",
    "#5 [3/4] RUN /jackin/bin/role-install --manifest /jackin/role/jackin.role.toml",
    "#5 4.210 install: codex 0.46.0",
    "#5 DONE 5.2s",
    "#6 exporting to image",
];

pub fn openshell_content() -> Arc<LaunchContent> {
    let base = LaunchContent::openshell();
    Arc::new(LaunchContent {
        build_log: OPENSHELL_BUILD_LOG
            .iter()
            .map(|l| (*l).to_owned())
            .collect(),
        network_failure: LaunchFailure {
            detail: vec![
                "gateway: jackin-local".into(),
                "sandbox: create failed".into(),
            ],
            ..base.network_failure.clone()
        },
        ..base
    })
}

/// How a simulated launch goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchPlan {
    Clean,

    FailNetwork,

    CredentialsLocked,

    BlockedSidecar,
}

/// What stands between a launch and its first stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    Straight,
    Refuse(RefusalReason),
    Ask(Vec<EnvQuestion>),
}

/// The checks a guarded simulator makes before it starts: the workspace must
/// allow the role, and a role whose manifest prompts for env values asks for
/// them.
pub fn validate_tuples(w: &World, target: &LaunchTarget) -> Option<RefusalReason> {
    use crate::domain::tuple::TupleState;
    let matrix_present = !w.surfaces.tuples.is_empty();
    if target.instances.is_empty() {
        return Some(RefusalReason::TupleNotLaunchable {
            agent: Agent::Codex,
            provider: crate::domain::agent::Provider::OpenAi,
            state: TupleState::Unavailable,
            reason: Some("no instances chosen".into()),
        });
    }
    let mut seen = Vec::new();
    for choice in &target.instances {
        if seen.contains(&choice.agent) {
            return Some(RefusalReason::TupleNotLaunchable {
                agent: choice.agent,
                provider: choice.provider,
                state: TupleState::Unavailable,
                reason: Some("duplicate agent".into()),
            });
        }
        seen.push(choice.agent);
        let identity = TupleIdentity::new(choice.agent, choice.provider);
        let offer = matrix_present
            .then(|| {
                w.surfaces
                    .tuples
                    .iter()
                    .find(|o| o.agent == identity.agent && o.provider == identity.provider)
            })
            .flatten();
        if matrix_present && offer.is_none() {
            return Some(RefusalReason::TupleNotLaunchable {
                agent: choice.agent,
                provider: choice.provider,
                state: TupleState::Unavailable,
                reason: Some("unknown tuple".into()),
            });
        }
        if let Some(offer) = offer {
            if offer.state != TupleState::Supported {
                return Some(RefusalReason::TupleNotLaunchable {
                    agent: choice.agent,
                    provider: choice.provider,
                    state: offer.state,
                    reason: offer.reason.clone(),
                });
            }
            if !offer.allowed_models.is_empty() && !offer.allowed_models.contains(&choice.model) {
                return Some(RefusalReason::TupleNotLaunchable {
                    agent: choice.agent,
                    provider: choice.provider,
                    state: offer.state,
                    reason: Some("unknown model".into()),
                });
            }
        } else if choice.provider != choice.agent.provider() || !choice.model.is_empty() {
            return Some(RefusalReason::TupleNotLaunchable {
                agent: choice.agent,
                provider: choice.provider,
                state: TupleState::Unavailable,
                reason: Some("no tuple offer is available".into()),
            });
        }
        let workspace = target.workspace.and_then(|id| w.workspace(id));
        let account_offer = if matrix_present {
            w.offer_for_provider(choice.agent, choice.provider, workspace, Some(&target.role))
        } else {
            w.offer_for(choice.agent, workspace, Some(&target.role))
        };
        if let Some(reason) = account_offer.blocked {
            return Some(RefusalReason::TupleNotLaunchable {
                agent: choice.agent,
                provider: choice.provider,
                state: offer.map_or(TupleState::Unavailable, |offer| offer.state),
                reason: Some(reason),
            });
        }
        match &choice.account {
            Some(account)
                if account_offer.accounts.contains(account)
                    && w.accounts
                        .get(account)
                        .is_some_and(|registered| registered.provider == choice.provider) => {}
            Some(_) => {
                return Some(RefusalReason::TupleNotLaunchable {
                    agent: choice.agent,
                    provider: choice.provider,
                    state: offer.map_or(TupleState::Unavailable, |offer| offer.state),
                    reason: Some("account is not eligible for this provider".into()),
                });
            }
            None if matrix_present
                && w.backend.account_requirement(identity) == AccountRequirement::Optional
                && account_offer.accounts.is_empty()
                && !account_offer.configured => {}
            None => {
                return Some(RefusalReason::TupleNotLaunchable {
                    agent: choice.agent,
                    provider: choice.provider,
                    state: offer.map_or(TupleState::Unavailable, |offer| offer.state),
                    reason: Some("an eligible provider account is required".into()),
                });
            }
        }
    }
    None
}

pub fn opening(w: &World, target: &LaunchTarget) -> Opening {
    if let Some(reason) = validate_tuples(w, target) {
        return Opening::Refuse(reason);
    }
    if let Some(ws) = target.workspace.and_then(|id| w.workspace(id))
        && !ws.roles.allows(&target.role)
    {
        return Opening::Refuse(RefusalReason::RoleNotAllowed {
            workspace: ws.name.clone(),
            role: target.role.clone(),
        });
    }
    let questions: Vec<EnvQuestion> = w
        .surfaces
        .roles
        .as_ref()
        .and_then(|admin| admin.role(&target.role))
        .and_then(|role| role.manifest.as_ref().ok())
        .map(|doc| {
            doc.env
                .iter()
                .filter_map(|entry| {
                    Some(EnvQuestion {
                        name: entry.name.clone(),
                        prompt: entry.prompt.clone()?,
                        default: entry.default.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if questions.is_empty() {
        Opening::Straight
    } else {
        Opening::Ask(questions)
    }
}

/// A launch that plays its stages on the cockpit's ticks and keeps the
/// instance record the way the simulated backend would.
#[derive(Debug)]
pub struct SimLaunch {
    plan: LaunchPlan,
    opening: Option<Opening>,
    asking: bool,
    run_id: String,
    container: String,
    instance_id: InstanceId,
    workspace: Option<WorkspaceId>,
    role: String,
    agent: Agent,
    account: Option<AccountId>,
    choices: Vec<crate::domain::tuple::InstanceChoice>,
    accounts: Vec<AccountId>,
    tick: u64,
    stage_start: u64,
    current: Option<usize>,
    ended: bool,
    credential_hold: bool,
    credential_retried: bool,
    build_lines_emitted: usize,
    content: Arc<LaunchContent>,
}

fn workspace_name(w: &World, workspace: Option<WorkspaceId>) -> String {
    workspace
        .and_then(|id| w.workspace(id))
        .map(|x| x.name.clone())
        .unwrap_or_else(|| w.cwd.rsplit('/').next().unwrap_or("directory").to_owned())
}

impl SimLaunch {
    pub fn start(
        plan: LaunchPlan,
        opening: Opening,
        w: &World,
        target: &LaunchTarget,
        content: Arc<LaunchContent>,
    ) -> Self {
        let wsname = workspace_name(w, target.workspace);
        let instance_id = next_instance_id(w);
        let suffix = instance_id.short_suffix();
        let container = match content.name_slug {
            Some(n) => {
                let slug = termrock::ui::text::clip(&wsname, n);
                format!("jackin-{}-{suffix}", slug.trim_end_matches('-'))
            }
            None => format!("jackin-{wsname}-{suffix}"),
        };
        let run_id = format!(
            "run-{}-{suffix}",
            termrock::ui::text::clip(
                &w.clock
                    .stamp(w.now_secs())
                    .replace([' ', ':'], "-")
                    .replace('-', ""),
                12,
            )
        );
        Self {
            plan,
            opening: Some(opening),
            asking: false,
            run_id,
            container,
            instance_id,
            workspace: target.workspace,
            role: target.role.clone(),
            agent: target.agent(),
            account: target.account(),
            choices: target.instances.clone(),
            accounts: launch_accounts(w, target, &content),
            tick: 0,
            stage_start: 0,
            current: None,
            ended: false,
            credential_hold: false,
            credential_retried: false,
            build_lines_emitted: 0,
            content,
        }
    }

    fn record(&self, w: &World, status: InstanceStatus) -> InstanceEntry {
        let now = w.now_secs();
        let wsname = workspace_name(w, self.workspace);
        let repo = w.github.iter().find(|r| r.full_name.ends_with(&wsname));
        InstanceEntry::available(Instance {
            id: self.instance_id.clone(),
            container: format!("jackin-{wsname}"),
            workspace: self.workspace,
            workdir: Some(format!("/workspace/{wsname}")),
            role: Some(self.role.clone()),
            agent: self.agent,
            status,
            created_secs: now,
            last_seen_secs: now,
            run_id: self.run_id.clone(),
            sessions: Ok(vec![]),
            daemon: DaemonSnapshot::Unavailable,
            branch: repo.and_then(|r| r.branches.get(1).cloned()),
            pr: None,
            account: self.account.clone(),
            accounts: self.accounts.clone(),
            default_branch: "main".into(),
            github: repo.map(Into::into),
            uncommitted: 0,
            unpushed: 0,
            agents: self
                .choices
                .iter()
                .map(|choice| crate::domain::instance::AgentInstanceView {
                    agent: choice.agent,
                    provider: choice.provider,
                    account: choice.account.clone(),
                    model: choice.model.clone(),
                    branch: None,
                    state: crate::domain::instance::AgentState::Idle,
                })
                .collect(),
            task: None,
        })
    }

    fn resume(&mut self) {
        self.credential_hold = false;
        self.credential_retried = true;
        self.stage_start = self.tick;
    }

    fn plan_stage(&self) -> Option<usize> {
        let id = match self.plan {
            LaunchPlan::FailNetwork => "network",
            LaunchPlan::BlockedSidecar => "sidecar",
            LaunchPlan::Clean | LaunchPlan::CredentialsLocked => return None,
        };
        self.content.stages.position(id)
    }

    fn advance(&mut self, w: &World) -> Vec<LaunchEvent> {
        let mut ev = vec![];
        if self.ended || self.credential_hold || self.asking {
            return ev;
        }
        self.tick += 1;
        let stages = &self.content.stages;
        let Some(i) = self.current else {
            match self.opening.take() {
                Some(Opening::Refuse(reason)) => {
                    self.ended = true;
                    ev.push(LaunchEvent::Refused(reason));
                    return ev;
                }
                Some(Opening::Ask(questions)) => {
                    self.asking = true;
                    ev.push(LaunchEvent::Activity("Waiting for your answers…".into()));
                    ev.push(LaunchEvent::NeedsInput(questions));
                    return ev;
                }
                Some(Opening::Straight) | None => {}
            }
            self.current = Some(0);
            self.stage_start = self.tick;
            ev.push(LaunchEvent::StageChanged(0, StepState::Running));
            ev.push(LaunchEvent::Activity(
                stages.get(0).activity_for(self.agent),
            ));
            return ev;
        };
        let spec = stages.get(i);
        let elapsed = self.tick - self.stage_start;

        if spec.kind == StageKind::BuildLog {
            let log = &self.content.build_log;
            let want = ((elapsed as usize) * log.len() / spec.ticks.max(1) as usize).min(log.len());
            while self.build_lines_emitted < want {
                ev.push(LaunchEvent::BuildLine(
                    self.content.build_log[self.build_lines_emitted].clone(),
                ));
                self.build_lines_emitted += 1;
            }
        }
        if spec.container_at == Some(elapsed) {
            ev.push(LaunchEvent::ContainerReady(self.container.clone()));
        }
        if let StageKind::Credentials { hold_at } = spec.kind
            && elapsed == hold_at
            && self.plan == LaunchPlan::CredentialsLocked
            && !self.credential_retried
        {
            self.credential_hold = true;
            ev.push(LaunchEvent::CredentialError {
                message: "1Password locked: unlock the app and retry".into(),
            });
            return ev;
        }
        if elapsed < spec.ticks {
            return ev;
        }

        let planned = self.plan_stage() == Some(i);
        let final_state = match (self.plan, planned) {
            _ if spec.skipped => StepState::Skipped,
            (LaunchPlan::FailNetwork, true) => StepState::Failed,
            (LaunchPlan::BlockedSidecar, true) => StepState::Blocked,
            _ => StepState::Done,
        };
        ev.push(LaunchEvent::StageChanged(i, final_state));
        if final_state == StepState::Done {
            ev.push(LaunchEvent::StageTimed {
                stage: i,
                elapsed_ms: spec.ticks * TICK_MS,
            });
        }
        if matches!(spec.kind, StageKind::Credentials { .. }) {
            ev.push(LaunchEvent::CredentialsResolved {
                origin: String::new(),
                validation: String::new(),
            });
        }
        match final_state {
            StepState::Failed => {
                self.ended = true;
                ev.push(LaunchEvent::Failed(Box::new(FailedLaunch {
                    failure: self.content.network_failure.clone(),
                    instance: Some(self.record(w, InstanceStatus::FailedSetup)),
                })));
                return ev;
            }
            StepState::Blocked => {
                self.ended = true;
                ev.push(LaunchEvent::Activity(
                    "Blocked · this stage cannot continue".into(),
                ));
                return ev;
            }
            _ => {}
        }
        if i + 1 < stages.len() {
            self.current = Some(i + 1);
            self.stage_start = self.tick;
            ev.push(LaunchEvent::StageChanged(i + 1, StepState::Running));
            ev.push(LaunchEvent::Activity(
                stages.get(i + 1).activity_for(self.agent),
            ));
        } else {
            self.ended = true;
            ev.push(LaunchEvent::Ready(Box::new(ReadyInstance {
                instance: self.record(w, InstanceStatus::Running),
                run_id: self.run_id.clone(),
                warning: None,
            })));
        }
        ev
    }
}

impl LaunchFeed for SimLaunch {
    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn telemetry(&self) -> Option<&str> {
        Some("otlp://collector.internal:4317")
    }

    fn poll(&mut self, world: &World) -> Vec<LaunchEvent> {
        self.advance(world)
    }

    fn retry_credentials(&mut self, op: &mut dyn OnePassword) {
        op.set_session(OpSession::SignedIn);
        self.resume();
    }

    fn use_plain_text(&mut self, _key: PlainKey) -> Option<TypedKey> {
        self.account = None;
        let replaced = self
            .choices
            .first_mut()
            .and_then(|lead| lead.account.take());
        if let Some(replaced) = replaced
            && !self
                .choices
                .iter()
                .any(|choice| choice.account.as_ref() == Some(&replaced))
        {
            self.accounts.retain(|account| account != &replaced);
        }
        self.resume();
        Some(TypedKey {
            origin: format!("plain text · masked API key · {}", masked("Q1zx")),
            validation: "typed once for this launch · never stored".into(),
        })
    }

    fn answer_env(&mut self, _answers: Vec<EnvAnswer>) {
        self.asking = false;
    }

    fn cancel(&mut self, world: &World) -> Option<InstanceEntry> {
        self.ended = true;
        if self.asking {
            return None;
        }
        Some(self.record(world, InstanceStatus::FailedSetup))
    }
}
