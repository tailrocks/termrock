use std::collections::BTreeMap;

use crate::domain::action::Item;
use crate::domain::activity::{Activity, ActivityKind, ActivityState, Script};
use crate::domain::cleanup::{OpsLog, SizeCache};
use crate::domain::context::{Host, Location, ScopeTag};
use crate::domain::custom::{CustomConfig, TrustStore};
use crate::domain::effect::Effect;
use crate::domain::exec::Command;
use crate::domain::plan::{Plan, PlanPhase, StepState};
use crate::domain::stack::{
    AptState, BrewState, CargoState, CleanupRecord, Container, DiskState, DockerState, GitState,
    GithubState, GradleState, MiseState, PgState, Platform, RankingMemory, SshState,
    SystemSnapshot, ToolOutputs, UpgradeState,
};
use crate::scenario::Scenario;
use crate::sim::fs::Fs;
use junie_tui::clock::SimClock;
use std::collections::BTreeSet;

pub const TICK_MS: i64 = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub done_at: u64,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchMode {
    Sequential,

    Parallel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    pub id: String,
    pub label: String,
    pub mode: BatchMode,
    pub members: Vec<String>,
    pub summarised: bool,
}

impl Batch {
    pub fn tally(&self, w: &World) -> (usize, usize, usize, usize) {
        let mut t = (0, 0, 0, 0);
        for m in &self.members {
            match w.activity(m).map(|a| a.state) {
                Some(ActivityState::Succeeded) => t.0 += 1,
                Some(ActivityState::Failed) => t.1 += 1,
                Some(ActivityState::Stopped) => t.2 += 1,
                _ => t.3 += 1,
            }
        }
        t
    }
    pub fn done(&self, w: &World) -> bool {
        self.tally(w).3 == 0
    }
    pub fn ok(&self, w: &World) -> bool {
        let (ok, failed, cancelled, pending) = self.tally(w);
        pending == 0 && failed == 0 && cancelled == 0 && ok == self.members.len()
    }
}

/// Work that must settle before holla can leave. Counted from the World, never
/// from open tabs or cached catalog rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuitWork {
    pub activities: usize,
    pub plans: usize,
    pub cleanup: bool,
}

impl QuitWork {
    pub fn is_empty(&self) -> bool {
        self.activities == 0 && self.plans == 0 && !self.cleanup
    }
}

/// Status shown when a launch, restart or plan start is refused during quit.
pub const STOPPING_REFUSAL: &str = "Already stopping · wait for quit or restart the app";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    ActivityEnded { id: String, ok: bool },

    BatchDone { id: String, ok: bool },
    PlanStepEnded { plan: String, step: usize, ok: bool },
    PlanDone { plan: String },
    DiscoveryDone { source: String },
    ScanDone,
}

pub struct World {
    pub scenario: Scenario,
    pub clock: SimClock,
    pub tick: u64,
    pub host: Host,
    pub location: Location,
    pub git: Vec<GitState>,
    pub github: GithubState,
    pub docker: DockerState,
    pub mise: MiseState,
    pub system: SystemSnapshot,
    pub disk: DiskState,
    pub pg: Option<PgState>,
    pub ssh: SshState,
    pub apt: Option<AptState>,
    pub memory: RankingMemory,
    pub activities: Vec<Activity>,
    pub batches: Vec<Batch>,
    pub plans: Vec<Plan>,
    pub sources: Vec<Source>,

    pub scripts: BTreeMap<String, Script>,

    pub step_output: BTreeMap<String, Vec<String>>,
    pub clipboard: Option<String>,

    pub scan_started: Option<u64>,
    pub next_activity: u32,

    pub trusted_now: Vec<String>,

    pub healed_units: Vec<String>,

    /// Set once a confirmed quit has asked running work to stop. New work is
    /// refused from then on so shutdown cannot be extended.
    pub(crate) stopping: bool,

    pub fs: Fs,

    pub tools: BTreeSet<String>,
    pub cargo: CargoState,
    pub brew: Option<BrewState>,
    pub gradle: GradleState,
    pub upgrade: UpgradeState,
    pub platform: Platform,
    pub outputs: ToolOutputs,

    pub custom_global: Option<CustomConfig>,
    pub custom_project: Option<CustomConfig>,
    pub trust: TrustStore,

    pub sudo_cached: bool,

    pub service_failures: Vec<String>,

    pub task_failures: Vec<String>,
    pub ops_log: OpsLog,
    pub size_cache: SizeCache,

    pub persisted: Persisted,

    pub reports: Vec<crate::domain::cleanup::Report>,

    pub scan: Option<ScanState>,

    pub fs_latency: Vec<(String, u64)>,

    pub cleanup_job: Option<CleanupJob>,
}

#[derive(Debug, Clone)]
pub struct CleanupJob {
    pub exec: crate::domain::cleanup::Execution,
    pub report_index: usize,

    pub processes: Vec<(String, crate::domain::cleanup::ProcessObservation)>,
}

pub type BatchMember = (String, Vec<Command>, Script, Option<Effect>, ScopeTag);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanState {
    pub root: String,
    pub started: u64,
    pub generation: u64,
    pub ticks: u64,
    pub tree: crate::sim::fs::ScanNode,

    pub order: Vec<String>,

    pub snapshot: Vec<(String, i64)>,
    pub cancelled: bool,
    pub include_hidden: bool,

    pub batch: usize,
    pub revealed_at_cancel: usize,
}

impl ScanState {
    pub fn revealed(&self, tick: u64) -> usize {
        if self.cancelled {
            return self.revealed_at_cancel.min(self.order.len());
        }
        let elapsed = tick.saturating_sub(self.started);
        let per_tick = self
            .order
            .len()
            .div_ceil(self.ticks.max(1) as usize)
            .max(1)
            .min(self.batch);
        (elapsed as usize * per_tick).min(self.order.len())
    }
    pub fn done(&self, tick: u64) -> bool {
        self.cancelled || self.revealed(tick) >= self.order.len()
    }
    pub fn progress(&self, tick: u64) -> f64 {
        if self.order.is_empty() {
            return 1.0;
        }
        self.revealed(tick) as f64 / self.order.len() as f64
    }
    pub fn is_revealed(&self, path: &str, tick: u64) -> bool {
        let n = self.revealed(tick);
        self.order.iter().take(n).any(|p| p == path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Persisted {
    pub trust: Option<String>,
    pub frecency: Option<String>,
    pub sizes: Option<String>,
    pub brew_cache: Option<String>,
    pub ops_log: Vec<String>,
}

impl World {
    pub fn now_ms(&self) -> i64 {
        self.clock.now_ms
    }

    pub fn now_secs(&self) -> i64 {
        self.clock.now_secs()
    }

    pub fn source_state(&self, name: &str) -> SourceState {
        match self.sources.iter().find(|s| s.name == name) {
            None => SourceState::Done,
            Some(s) => {
                if let Some(f) = &s.failure {
                    if self.tick >= s.done_at {
                        SourceState::Failed(f.clone())
                    } else {
                        SourceState::Loading
                    }
                } else if self.tick >= s.done_at {
                    SourceState::Done
                } else {
                    SourceState::Loading
                }
            }
        }
    }

    pub fn sources_done(&self) -> usize {
        self.sources
            .iter()
            .filter(|s| self.tick >= s.done_at)
            .count()
    }

    pub fn discovering(&self) -> bool {
        self.sources_done() < self.sources.len()
    }

    pub fn pending_sources(&self) -> Vec<&str> {
        self.sources
            .iter()
            .filter(|s| self.tick < s.done_at)
            .map(|s| s.name.as_str())
            .collect()
    }

    pub fn failed_sources(&self) -> Vec<(&str, &str)> {
        self.sources
            .iter()
            .filter(|s| self.tick >= s.done_at)
            .filter_map(|s| s.failure.as_deref().map(|f| (s.name.as_str(), f)))
            .collect()
    }

    pub fn git_here(&self) -> Option<&GitState> {
        let root = self
            .location
            .project
            .as_ref()
            .map(|p| p.root.as_str())
            .unwrap_or(&self.location.cwd);
        self.git.iter().find(|g| g.path == root).or_else(|| {
            self.git
                .iter()
                .find(|g| self.location.cwd.starts_with(&g.path))
        })
    }

    pub fn activity(&self, id: &str) -> Option<&Activity> {
        self.activities.iter().find(|a| a.id == id)
    }

    pub fn activity_mut(&mut self, id: &str) -> Option<&mut Activity> {
        self.activities.iter_mut().find(|a| a.id == id)
    }

    pub fn plan(&self, id: &str) -> Option<&Plan> {
        self.plans.iter().find(|p| p.id == id)
    }

    pub fn plan_mut(&mut self, id: &str) -> Option<&mut Plan> {
        self.plans.iter_mut().find(|p| p.id == id)
    }

    pub fn live_activities(&self) -> usize {
        self.activities.iter().filter(|a| a.state.live()).count()
    }

    pub fn quit_work(&self) -> QuitWork {
        QuitWork {
            activities: self.live_activities(),
            plans: self
                .plans
                .iter()
                .filter(|p| p.phase == PlanPhase::Running)
                .count(),
            cleanup: self.cleanup_job.is_some(),
        }
    }

    /// Whether a confirmed quit is stopping work, so new work must be refused.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// Asks everything running to stop for a confirmed quit. Plans stop and
    /// finalize synchronously (their completion messages are returned for normal
    /// dispatch); activities follow their cancel schedule; a cleanup is never
    /// stopped. Repeating it changes nothing that was already stopping.
    pub fn stop_work_for_quit(&mut self) -> Vec<Msg> {
        self.stopping = true;
        let running: Vec<String> = self
            .plans
            .iter()
            .filter(|p| p.phase == PlanPhase::Running)
            .map(|p| p.id.clone())
            .collect();
        self.stop_all();
        running.iter().flat_map(|id| self.stop_plan(id)).collect()
    }

    pub fn script_for(&self, key: Option<&str>, argv: &[Command]) -> Script {
        if let Some(k) = key
            && let Some(s) = self.scripts.get(k)
        {
            return s.clone();
        }
        let display = argv
            .iter()
            .map(|c| c.display())
            .collect::<Vec<_>>()
            .join(" && ");

        if let Some(s) = self.scripts.get(&display) {
            return s.clone();
        }
        if let Some(s) = crate::domain::outcomes::for_commands(self, argv) {
            return s;
        }
        Script::unmodeled(&display)
    }

    pub fn start_activity(
        &mut self,
        argv: Vec<Command>,
        script: Script,
        name: &str,
        origin: &str,
        kind: ActivityKind,
        scope: ScopeTag,
    ) -> String {
        self.next_activity += 1;
        let id = format!("act-{}", self.next_activity);
        let host = self.host.name.clone();
        let mut a = Activity::new(&id, name, origin, kind, scope, &host, script, self.tick);
        a.argv = argv;
        a.attachable = matches!(
            a.kind,
            ActivityKind::Monitor { .. } | ActivityKind::Ssh { .. }
        );
        a.advance(self.tick);
        self.activities.push(a);
        id
    }

    pub fn start_scripted(
        &mut self,
        script_key: &str,
        name: &str,
        origin: &str,
        kind: ActivityKind,
        scope: ScopeTag,
    ) -> String {
        let script = self.script_for(Some(script_key), &[]);
        self.start_activity(vec![], script, name, origin, kind, scope)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_batch(
        &mut self,
        id: &str,
        label: &str,
        mode: BatchMode,
        members: Vec<BatchMember>,
        origin: &str,
    ) -> Vec<String> {
        let mut ids = vec![];
        let mut prev: Option<String> = None;
        for (name, argv, script, effect, scope) in members {
            self.next_activity += 1;
            let aid = format!("act-{}", self.next_activity);
            let host = self.host.name.clone();
            let mut a = Activity::new(
                &aid,
                &name,
                origin,
                ActivityKind::Task,
                scope,
                &host,
                script,
                self.tick,
            );
            a.argv = argv;
            a.effect = effect;
            a.batch = Some(id.to_owned());
            if mode == BatchMode::Sequential && prev.is_some() {
                a.state = ActivityState::Queued;
                a.after = prev.clone();
            } else {
                a.advance(self.tick);
            }
            prev = Some(aid.clone());
            ids.push(aid);
            self.activities.push(a);
        }
        self.batches.retain(|b| b.id != id);
        self.batches.push(Batch {
            id: id.into(),
            label: label.into(),
            mode,
            members: ids.clone(),
            summarised: false,
        });
        ids
    }

    pub fn stop_all(&mut self) -> usize {
        let tick = self.tick;
        let mut n = 0;
        for a in &mut self.activities {
            if a.state.live() {
                a.stop(tick);
                n += 1;
            }
        }
        n
    }

    pub fn cancelling(&self) -> usize {
        self.activities
            .iter()
            .filter(|a| a.state == ActivityState::Cancelling)
            .count()
    }

    pub fn record_use(&mut self, item: &str) {
        let cwd = self.location.cwd.clone();
        let host = self.host.name.clone();
        let now = self.now_secs();
        self.memory.used(item, Some(&cwd), &host, now);
        self.memory.used(item, None, &host, now);
    }

    pub fn tick(&mut self) -> Vec<Msg> {
        self.clock.advance(TICK_MS);
        if !self.clock.running {
            return vec![];
        }
        self.tick += 1;
        let mut msgs = vec![];
        for s in &self.sources {
            if s.done_at == self.tick {
                msgs.push(Msg::DiscoveryDone {
                    source: s.name.clone(),
                });
            }
        }
        if let Some(start) = self.scan_started
            && self.tick == start + self.disk.scan_ticks
        {
            msgs.push(Msg::ScanDone);
            self.finish_scan_cache();
        }
        let tick = self.tick;
        let mut effects = vec![];

        let ready: Vec<String> = self
            .activities
            .iter()
            .filter(|a| a.state == ActivityState::Queued)
            .filter(|a| {
                a.after
                    .as_ref()
                    .is_none_or(|p| self.activity(p).is_none_or(|x| x.state.finished()))
            })
            .map(|a| a.id.clone())
            .collect();
        for id in ready {
            if let Some(a) = self.activity_mut(&id) {
                a.start(tick);
            }
        }
        for a in &mut self.activities {
            let was = a.state;
            a.advance(tick);
            if was.live() && !a.state.live() {
                let ok = a.state == ActivityState::Succeeded;
                if ok && let Some(e) = &a.effect {
                    effects.push(e.clone());
                }
                msgs.push(Msg::ActivityEnded {
                    id: a.id.clone(),
                    ok,
                });
            }
        }
        for e in &effects {
            self.apply_effect(e);
        }
        for bi in 0..self.batches.len() {
            if !self.batches[bi].summarised && self.batches[bi].done(self) {
                let ok = self.batches[bi].ok(self);
                self.batches[bi].summarised = true;
                msgs.push(Msg::BatchDone {
                    id: self.batches[bi].id.clone(),
                    ok,
                });
            }
        }
        for pi in 0..self.plans.len() {
            msgs.extend(self.tick_plan(pi));
        }
        msgs
    }

    fn tick_plan(&mut self, pi: usize) -> Vec<Msg> {
        let tick = self.tick;
        let mut msgs = vec![];
        let plan_id = self.plans[pi].id.clone();
        if self.plans[pi].phase != PlanPhase::Running {
            return msgs;
        }

        for si in 0..self.plans[pi].steps.len() {
            let (running, started, duration, fails) = {
                let s = &self.plans[pi].steps[si];
                (
                    s.state == StepState::Running,
                    s.started_tick.unwrap_or(tick),
                    s.duration_ticks.max(1),
                    s.fails,
                )
            };
            if !running {
                continue;
            }
            let key = format!("{plan_id}/{}", self.plans[pi].steps[si].id);
            let lines = self.step_output.get(&key).cloned().unwrap_or_default();
            let elapsed = tick.saturating_sub(started);
            let want = ((elapsed as f64 / duration as f64) * lines.len() as f64).ceil() as usize;
            let want = want.min(lines.len());
            let have = self.plans[pi].steps[si].output.len();
            if want > have {
                self.plans[pi].steps[si]
                    .output
                    .extend(lines[have..want].iter().cloned());
            }
            if elapsed >= duration {
                let s = &mut self.plans[pi].steps[si];
                s.ended_tick = Some(tick);
                s.exit = Some(if fails { 1 } else { 0 });
                s.state = if fails {
                    StepState::Failed
                } else {
                    StepState::Succeeded
                };
                if fails {
                    s.output.push("exit status 1".into());
                }
                msgs.push(Msg::PlanStepEnded {
                    plan: plan_id.clone(),
                    step: si,
                    ok: !fails,
                });
            }
        }
        self.plans[pi].recompute();

        loop {
            let plan = &self.plans[pi];
            let running = plan.running();
            if running.len() >= plan.max_parallel {
                break;
            }
            let locks: Vec<String> = running
                .iter()
                .filter_map(|&i| plan.steps[i].exclusive.clone())
                .collect();
            let next = plan.ready().into_iter().find(|&i| {
                plan.steps[i]
                    .exclusive
                    .as_ref()
                    .is_none_or(|e| !locks.contains(e))
            });
            match next {
                Some(i) => {
                    let s = &mut self.plans[pi].steps[i];
                    s.state = StepState::Running;
                    s.started_tick = Some(tick);
                }
                None => break,
            }
        }
        if self.plans[pi].is_done() && self.plans[pi].phase == PlanPhase::Running {
            self.plans[pi].phase = PlanPhase::Done;
            self.plans[pi].ended_tick = Some(tick);
            msgs.push(self.publish_plan_completion(pi));
        }
        msgs
    }

    /// Stops a running plan as a whole: every live step becomes terminal, the
    /// plan is Done, and its completion is published once. Returns the messages
    /// to deliver (empty when the plan is unknown or not running).
    pub fn stop_plan(&mut self, id: &str) -> Vec<Msg> {
        let tick = self.tick;
        let Some(pi) = self.plans.iter().position(|p| p.id == id) else {
            return vec![];
        };
        if !self.plans[pi].stop(tick) {
            return vec![];
        }
        vec![self.publish_plan_completion(pi)]
    }

    /// The one place a Running to Done plan transition is published: applies the
    /// effects of newly succeeded steps and builds the completion message.
    fn publish_plan_completion(&mut self, pi: usize) -> Msg {
        self.apply_plan_effects(pi);
        Msg::PlanDone {
            plan: self.plans[pi].id.clone(),
        }
    }

    pub fn pg_blocker(&self) -> Option<u32> {
        self.pg
            .as_ref()
            .and_then(|pg| pg.sessions.iter().find_map(|s| s.blocked_by))
    }

    pub fn apply_effect(&mut self, e: &Effect) {
        match e {
            Effect::DockerRestart(name) => {
                if let Some(c) = self.docker.containers.iter_mut().find(|c| &c.name == name) {
                    c.running = true;
                    if c.health.is_some() {
                        c.health = Some("healthy".into());
                    }
                }
            }
            Effect::DockerStop(name) => {
                if let Some(c) = self.docker.containers.iter_mut().find(|c| &c.name == name) {
                    c.running = false;
                    c.cpu_pct = 0.0;
                }
            }
            Effect::ServiceRestart(unit) => {
                if !self.healed_units.contains(unit) {
                    self.healed_units.push(unit.clone());
                }
            }
            Effect::PgCancel(pid) | Effect::PgTerminate(pid) => {
                if let Some(pg) = &mut self.pg {
                    let terminate = matches!(e, Effect::PgTerminate(_));
                    if terminate {
                        pg.sessions.retain(|s| s.pid != *pid);
                        pg.connections = pg.connections.saturating_sub(1);
                    } else if let Some(s) = pg.sessions.iter_mut().find(|s| s.pid == *pid) {
                        s.state = "idle".into();
                        s.txn_secs = 0;
                        s.query_secs = 0;
                        s.query = "ROLLBACK".into();
                    }
                    for s in pg
                        .sessions
                        .iter_mut()
                        .filter(|s| s.blocked_by == Some(*pid))
                    {
                        s.blocked_by = None;
                        s.wait = None;
                        s.state = "active".into();
                    }
                }
            }
            Effect::GitPull(path) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    g.behind = 0;
                }
            }
            Effect::CargoClean(target) => {
                let target = target.clone();
                let _ = self.fs.remove_permanent(&target);
                if self.cargo.target.as_deref() == Some(target.as_str()) {
                    self.cargo.target_files = 0;
                    self.cargo.target_bytes = 0;
                }
                self.remove_candidate(&target, "cargo clean");
            }
            Effect::DockerStopAll => {
                for c in &mut self.docker.containers {
                    c.running = false;
                    c.cpu_pct = 0.0;
                }
            }
            Effect::DockerRemoveAll => {
                self.docker.containers.clear();
                for v in &mut self.docker.volumes {
                    v.used_by = None;
                }
            }
            Effect::DockerImagesPrune => {
                self.docker.images = 0;
                self.docker.images_gb = 0.0;
                self.docker.dangling_images = 0;
                self.docker.reclaimable_gb =
                    self.docker.volumes_gb() + self.docker.builder_cache_gb;
            }
            Effect::DockerNetworkPrune => {
                self.docker.networks = 0;
            }
            Effect::DockerVolumePrune => {
                self.docker.volumes.retain(|v| v.used_by.is_some());
                self.docker.reclaimable_gb =
                    self.docker.images_gb + self.docker.volumes_gb() + self.docker.builder_cache_gb;
            }
            Effect::DockerBuilderPrune => {
                self.docker.builder_cache_gb = 0.0;
                self.docker.reclaimable_gb = self.docker.images_gb + self.docker.volumes_gb();
            }
            Effect::ComposeUp => {
                if let Some(c) = self.docker.compose.clone() {
                    for svc in &c.services {
                        let name = format!("{}-{svc}-1", c.name);
                        match self.docker.containers.iter_mut().find(|x| {
                            x.name == name
                                || (x.project.as_deref() == Some(c.name.as_str())
                                    && x.image.starts_with(svc.as_str()))
                        }) {
                            Some(x) => {
                                x.running = true;
                                if x.health.is_some() {
                                    x.health = Some("healthy".into());
                                }
                            }
                            None => self.docker.containers.push(Container {
                                name,
                                image: format!("{svc}:latest"),
                                running: true,
                                health: None,
                                ports: String::new(),
                                project: Some(c.name.clone()),
                                cpu_pct: 0.1,
                                mem_mb: 32,
                                size_mb: 0,
                            }),
                        }
                    }
                }
            }
            Effect::ComposeDown => {
                if let Some(c) = self.docker.compose.clone() {
                    self.docker
                        .containers
                        .retain(|x| x.project.as_deref() != Some(c.name.as_str()));
                    self.docker.networks = self.docker.networks.saturating_sub(1);
                }
            }
            Effect::GitPullMerge(path) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    if g.behind > 0 && g.ahead > 0 && g.pull_config != "rebase" {
                        g.ahead += 1;
                    }
                    g.behind = 0;
                    g.diverged = false;
                }
            }
            Effect::GitPush(path) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    g.ahead = 0;
                    g.push_rejected = None;
                    if g.upstream.is_none()
                        && let Some(b) = &g.branch
                    {
                        g.upstream = Some(format!("origin/{b}"));
                    }
                }
            }
            Effect::GitPushRemote(path, remote) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    let tracks = g
                        .upstream
                        .as_deref()
                        .is_some_and(|u| u.starts_with(&format!("{remote}/")));
                    if tracks || g.upstream.is_none() {
                        g.ahead = 0;
                        g.push_rejected = None;
                    }
                }
            }
            Effect::GitFetch(_) => {}
            Effect::GitSwitch(path, branch) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    g.branch = Some(branch.clone());
                    if branch == &g.primary {
                        g.ahead = 0;
                        g.behind = 0;
                        g.diverged = false;
                        g.upstream = Some(format!("origin/{branch}"));
                    }
                }
            }
            Effect::GitGc(path) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    g.gc_needed = false;
                }
            }
            Effect::GitDeleteBranches(path, names) => {
                if let Some(g) = self.git.iter_mut().find(|g| &g.path == path) {
                    g.merged.retain(|b| !names.contains(b));
                }
            }
            Effect::MiseUpgrade => {
                for t in self.mise.global_tools.iter_mut().chain(
                    self.mise
                        .configs
                        .iter_mut()
                        .flat_map(|c| c.tools.iter_mut()),
                ) {
                    if t.installed {
                        t.active = Some(t.latest.clone());
                    }
                }
            }
            Effect::BrewService(name, verb) => {
                if let Some(b) = &mut self.brew
                    && let Some(s) = b.services.iter_mut().find(|s| &s.name == name)
                {
                    s.status = match verb.as_str() {
                        "start" | "restart" | "run" => "started".into(),
                        "stop" => "none".into(),
                        _ => s.status.clone(),
                    };
                }
            }
            Effect::BrewUpgrade(casks) => {
                self.upgrade.brew_outdated.clear();
                if *casks {
                    self.upgrade.casks_outdated.clear();
                }
            }
            Effect::AmpUpdate | Effect::OmzUpgrade => {}
            Effect::GradleStop => {
                self.gradle.daemon_running = false;
            }
            Effect::GradleClean(path) => {
                let build = format!("{path}/build");
                let _ = self.fs.remove_permanent(&build);
                self.remove_candidate(&build, "gradle clean");
            }
            Effect::KillProcess(pid) => {
                self.system.top.retain(|p| p.pid != *pid);
            }
            Effect::DeleteAll(path) => {
                let children: Vec<String> = self
                    .fs
                    .list(path)
                    .map(|v| v.iter().map(|n| n.path.clone()).collect())
                    .unwrap_or_default();
                for c in children {
                    let _ = self.fs.remove_permanent(&c);
                    self.remove_candidate(&c, "rm -rf");
                }
                let prefix = format!("{path}/");
                self.disk
                    .candidates
                    .retain(|c| !c.path.starts_with(&prefix));
                self.disk.large.retain(|l| !l.path.starts_with(&prefix));
            }
            Effect::MiseInstall => {
                for cfg in &mut self.mise.configs {
                    for t in &mut cfg.tools {
                        if !t.installed {
                            t.installed = true;
                            t.active = Some(t.requested.clone());
                        }
                    }
                }
                for t in &mut self.mise.global_tools {
                    if !t.installed {
                        t.installed = true;
                        t.active = Some(t.requested.clone());
                    }
                }
            }
        }
    }

    fn remove_candidate(&mut self, path: &str, method: &str) {
        let Some(i) = self.disk.candidates.iter().position(|c| c.path == path) else {
            return;
        };
        let c = self.disk.candidates.remove(i);
        if let Some(fs) = self.disk.filesystems.first_mut() {
            fs.used_gb = fs.used_gb.saturating_sub(c.gb.round() as u32);
        }
        self.disk.history.push(CleanupRecord {
            when_secs: self.now_secs(),
            target: c.path.clone(),
            method: method.to_owned(),
            reclaimed_gb: c.gb,
            outcome: "succeeded".into(),
        });
    }

    fn apply_plan_effects(&mut self, pi: usize) {
        let plan = self.plans[pi].clone();
        let newly_succeeded = plan.unapplied_successful_step_ids();
        let ok = |id: &str| newly_succeeded.contains(id);
        match plan.id.as_str() {
            "docker-cleanup" => {
                if ok("stop") {
                    for c in &mut self.docker.containers {
                        c.running = false;
                        c.cpu_pct = 0.0;
                    }
                }
                if ok("rm") {
                    self.docker.containers.clear();
                }
                if ok("images") {
                    self.docker.images = 0;
                    self.docker.images_gb = 0.0;
                    self.docker.dangling_images = 0;
                }
                if ok("networks") {
                    self.docker.networks = 0;
                }
                if ok("volumes") {
                    self.docker.volumes.clear();
                }
                if ok("builder") {
                    self.docker.builder_cache_gb = 0.0;
                }
                self.docker.reclaimable_gb =
                    self.docker.images_gb + self.docker.volumes_gb() + self.docker.builder_cache_gb;
            }
            "upgrade" => {
                if ok("apt-apply")
                    && let Some(apt) = &mut self.apt
                {
                    apt.upgradable.clear();
                    apt.pending_reboot = true;
                }
                if ok("mise-upgrade") {
                    for t in &mut self.mise.global_tools {
                        if t.name != "go" && t.installed {
                            t.active = Some(t.latest.clone());
                        }
                    }
                }
            }
            "upgrade-all" => {
                if ok("brew-upgrade") {
                    self.upgrade.brew_outdated.clear();
                }
                if ok("brew-casks") {
                    self.upgrade.casks_outdated.clear();
                }
                if ok("mise-upgrade") {
                    for t in self.mise.global_tools.iter_mut().chain(
                        self.mise
                            .configs
                            .iter_mut()
                            .flat_map(|c| c.tools.iter_mut()),
                    ) {
                        if t.installed {
                            t.active = Some(t.latest.clone());
                        }
                    }
                }
            }
            "cleanup-work" => {
                for s in plan
                    .steps
                    .iter()
                    .filter(|s| newly_succeeded.contains(&s.id))
                {
                    let Some(target) = s.target.clone() else {
                        continue;
                    };
                    let method = s
                        .commands
                        .first()
                        .and_then(|c| c.split_whitespace().next())
                        .unwrap_or("cleanup")
                        .to_owned();
                    self.remove_candidate(&target, &method);
                }
            }
            "git-pull-all" => {
                for s in plan
                    .steps
                    .iter()
                    .filter(|s| newly_succeeded.contains(&s.id))
                {
                    if let Some(g) = self.git.iter_mut().find(|g| g.path == s.cwd) {
                        g.behind = 0;
                    }
                }
            }
            "git-switch-primary" => {
                for s in plan
                    .steps
                    .iter()
                    .filter(|s| newly_succeeded.contains(&s.id))
                {
                    if let Some(g) = self.git.iter_mut().find(|g| g.path == s.cwd) {
                        g.branch = Some(g.primary.clone());
                    }
                }
            }
            _ => {}
        }
        self.plans[pi].mark_step_effects_applied(&newly_succeeded);
        if plan.id == "docker-cleanup" && self.plans[pi].count_completion_once() {
            self.docker.cleanup_uses += 1;
        }
    }

    pub fn start_scan(&mut self, path: &str) {
        let generation = self.scan.as_ref().map(|s| s.generation + 1).unwrap_or(1);
        let include_hidden = true;
        let mut skip = vec![];
        if path == "/" {
            skip.push("/System/Volumes/Data".to_owned());
        }
        skip.push(format!("{}/Library/Mobile Documents", self.location.home));
        let tree = self.fs.scan(
            path,
            &crate::sim::fs::ScanOptions {
                include_hidden,
                skip,
                max_depth: None,
            },
        );
        let mut order = vec![];
        fn walk(n: &crate::sim::fs::ScanNode, out: &mut Vec<String>) {
            out.push(n.path.clone());
            for c in &n.children {
                walk(c, out);
            }
        }
        walk(&tree, &mut order);
        let snapshot: Vec<(String, i64)> = self
            .fs
            .subtree(path)
            .iter()
            .map(|n| (n.path.clone(), n.mtime))
            .chain(self.fs.get(path).map(|n| (n.path.clone(), n.mtime)))
            .collect();
        let ticks = self.disk.scan_ticks.max(1);
        self.scan = Some(ScanState {
            root: path.into(),
            started: self.tick,
            generation,
            ticks,
            tree,
            order,
            snapshot,
            cancelled: false,
            include_hidden,
            batch: 4096,
            revealed_at_cancel: 0,
        });
        self.scan_started = Some(self.tick);
    }

    pub fn cancel_scan(&mut self) -> bool {
        let tick = self.tick;
        match self.scan.as_mut() {
            Some(s) if !s.done(tick) => {
                s.revealed_at_cancel = s.revealed(tick);
                s.cancelled = true;
                true
            }
            _ => false,
        }
    }

    pub fn finish_scan_cache(&mut self) {
        let Some(s) = self.scan.clone() else {
            return;
        };
        if s.cancelled {
            return;
        }
        let now = self.now_secs();
        let on_disk = crate::domain::cleanup::SizeCache::load(self.persisted.sizes.as_deref(), now);
        self.size_cache.merge(&on_disk, now);
        self.size_cache
            .record(&s.root, &s.tree, &s.snapshot, &self.fs, now);
        if self.size_cache.save_error.is_none() {
            self.persisted.sizes = Some(self.size_cache.serialize());
        }
    }

    pub fn items(&self) -> Vec<Item> {
        crate::domain::catalog::build(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceState {
    Loading,
    Done,
    Failed(String),
}

#[cfg(test)]
mod plan_tests {
    use super::*;
    use crate::domain::fixtures::world_for;
    use crate::scenario::Motion;

    const ID: &str = "docker-cleanup";

    fn armed_world() -> World {
        let mut w = world_for(Scenario::DockerCleanup, Motion::Full);
        w.clock.running = true;
        let tick = w.tick;
        assert!(w.plan_mut(ID).expect("plan").arm(tick));
        w
    }

    fn step_state(w: &World, id: &str) -> StepState {
        w.plan(ID)
            .expect("plan")
            .steps
            .iter()
            .find(|s| s.id == id)
            .expect("step")
            .state
    }

    fn tick_until(w: &mut World, mut done: impl FnMut(&World) -> bool) -> Vec<Msg> {
        let mut msgs = vec![];
        for _ in 0..400 {
            if done(w) {
                return msgs;
            }
            msgs.extend(w.tick());
        }
        panic!("condition never reached");
    }

    fn plan_done_count(msgs: &[Msg]) -> usize {
        msgs.iter()
            .filter(|m| matches!(m, Msg::PlanDone { .. }))
            .count()
    }

    #[test]
    fn plan_stop_world_finalizes_running_step_and_applies_only_successes() {
        let mut w = armed_world();
        let uses = w.docker.cleanup_uses;
        let containers = w.docker.containers.len();
        assert!(containers > 0);
        tick_until(&mut w, |w| step_state(w, "rm") == StepState::Running);
        assert_eq!(step_state(&w, "stop"), StepState::Succeeded);

        let msgs = w.stop_plan(ID);

        assert_eq!(plan_done_count(&msgs), 1);
        let p = w.plan(ID).expect("plan");
        assert_eq!(p.phase, PlanPhase::Done);
        assert!(p.ended_tick.is_some());
        assert!(p.steps.iter().all(|s| !matches!(
            s.state,
            StepState::Ready | StepState::Waiting | StepState::Running
        )));
        assert_eq!(step_state(&w, "rm"), StepState::Cancelled);
        assert!(
            w.docker.containers.iter().all(|c| !c.running),
            "the succeeded stop step applied"
        );
        assert_eq!(
            w.docker.containers.len(),
            containers,
            "the cancelled rm step applied nothing"
        );
        assert_eq!(w.docker.cleanup_uses, uses + 1);
    }

    #[test]
    fn plan_stop_is_idempotent_and_emits_completion_once() {
        let mut w = armed_world();
        tick_until(&mut w, |w| step_state(w, "rm") == StepState::Running);
        let first = w.stop_plan(ID);
        assert_eq!(plan_done_count(&first), 1);
        let snapshot = w.plan(ID).cloned();
        let uses = w.docker.cleanup_uses;

        assert!(w.stop_plan(ID).is_empty());
        let mut later = vec![];
        for _ in 0..200 {
            later.extend(w.tick());
        }
        assert_eq!(plan_done_count(&later), 0);
        assert_eq!(w.plan(ID).cloned(), snapshot);
        assert_eq!(w.docker.cleanup_uses, uses);
        assert!(w.stop_plan("no-such-plan").is_empty());
    }

    #[test]
    fn plan_stop_before_any_success_applies_no_effect_and_no_usage() {
        let mut w = armed_world();
        let uses = w.docker.cleanup_uses;
        let running = w.docker.containers.iter().filter(|c| c.running).count();
        tick_until(&mut w, |w| step_state(w, "stop") == StepState::Running);

        assert_eq!(plan_done_count(&w.stop_plan(ID)), 1);

        assert_eq!(w.docker.cleanup_uses, uses);
        assert_eq!(
            w.docker.containers.iter().filter(|c| c.running).count(),
            running
        );
    }

    #[test]
    fn plan_cancel_remaining_world_lets_running_step_finish_naturally() {
        let mut w = armed_world();
        tick_until(&mut w, |w| step_state(w, "rm") == StepState::Running);
        w.plan_mut(ID).expect("plan").cancel_remaining();
        assert_eq!(step_state(&w, "rm"), StepState::Running);

        let msgs = tick_until(&mut w, |w| {
            w.plan(ID).expect("plan").phase == PlanPhase::Done
        });

        assert_eq!(plan_done_count(&msgs), 1);
        assert_eq!(step_state(&w, "rm"), StepState::Succeeded);
        assert!(w.docker.containers.is_empty(), "the finished step applied");
    }

    #[test]
    fn plan_retry_has_new_completion_without_repeating_success_effects() {
        let mut w = armed_world();
        let uses = w.docker.cleanup_uses;
        w.plan_mut(ID)
            .expect("plan")
            .steps
            .iter_mut()
            .find(|s| s.id == "images")
            .expect("step")
            .fails = true;
        let first = tick_until(&mut w, |w| {
            w.plan(ID).expect("plan").phase == PlanPhase::Done
        });
        assert_eq!(plan_done_count(&first), 1);
        assert_eq!(step_state(&w, "images"), StepState::Failed);
        assert_eq!(w.docker.cleanup_uses, uses + 1);
        assert_eq!(w.docker.networks, 0, "networks step applied");
        w.docker.networks = 7;

        let images = w
            .plan(ID)
            .expect("plan")
            .steps
            .iter()
            .position(|s| s.id == "images")
            .expect("step");
        assert!(w.plan_mut(ID).expect("plan").retry(images));
        let second = tick_until(&mut w, |w| {
            w.plan(ID).expect("plan").phase == PlanPhase::Done
        });

        assert_eq!(plan_done_count(&second), 1, "a retry completes again");
        assert_eq!(step_state(&w, "images"), StepState::Succeeded);
        assert_eq!(w.docker.images, 0, "the retried step applied");
        assert_eq!(w.docker.networks, 7, "the earlier success did not repeat");
        assert_eq!(
            w.docker.cleanup_uses,
            uses + 1,
            "usage counted once per arm"
        );
    }
}

#[cfg(test)]
mod quit_tests {
    use super::*;
    use crate::domain::activity::{ActivityKind, Script};
    use crate::domain::context::ScopeTag;
    use crate::domain::fixtures::world_for;
    use crate::scenario::Motion;

    const ALL_STATES: [ActivityState; 7] = [
        ActivityState::Queued,
        ActivityState::Running,
        ActivityState::Cancelling,
        ActivityState::Succeeded,
        ActivityState::Failed,
        ActivityState::Stopped,
        ActivityState::Detached,
    ];

    #[test]
    fn quit_work_counts_every_activity_state_plan_phase_and_cleanup() {
        let mut w = world_for(Scenario::DockerCleanup, Motion::Full);
        w.activities.clear();
        let scope = ScopeTag::here(&w.location.cwd);
        let id = w.start_activity(
            vec![],
            Script::running(vec![]),
            "a",
            "a",
            ActivityKind::Task,
            scope,
        );
        for state in ALL_STATES {
            w.activity_mut(&id).expect("activity").state = state;
            let work = w.quit_work();
            assert_eq!(work.activities, usize::from(state.live()), "{state:?}");
            assert_eq!(work.plans, 0);
            assert!(!work.cleanup);
            assert_eq!(work.is_empty(), !state.live(), "{state:?}");
        }
        w.activity_mut(&id).expect("activity").state = ActivityState::Succeeded;
        for (phase, plans) in [
            (PlanPhase::Review, 0),
            (PlanPhase::Running, 1),
            (PlanPhase::Done, 0),
        ] {
            w.plan_mut("docker-cleanup").expect("plan").phase = phase;
            assert_eq!(w.quit_work().plans, plans, "{phase:?}");
            assert_eq!(w.quit_work().is_empty(), plans == 0, "{phase:?}");
        }
    }

    #[test]
    fn quit_stop_work_is_idempotent_and_finalizes_every_running_plan() {
        let mut w = world_for(Scenario::DockerCleanup, Motion::Full);
        w.clock.running = true;
        let mut second = w.plan("docker-cleanup").cloned().expect("plan");
        second.id = "second".into();
        w.plans.push(second);
        let tick = w.tick;
        for id in ["docker-cleanup", "second"] {
            assert!(w.plan_mut(id).expect("plan").arm(tick));
        }
        for _ in 0..3 {
            w.tick();
        }
        assert_eq!(w.quit_work().plans, 2);
        assert!(!w.stopping());

        let msgs = w.stop_work_for_quit();

        assert_eq!(
            msgs,
            vec![
                Msg::PlanDone {
                    plan: "docker-cleanup".into()
                },
                Msg::PlanDone {
                    plan: "second".into()
                },
            ],
            "one completion per plan, in plan order"
        );
        assert!(w.stopping());
        assert!(w.quit_work().is_empty());
        assert!(w.stop_work_for_quit().is_empty());
    }
}
