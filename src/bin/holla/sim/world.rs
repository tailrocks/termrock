use std::collections::BTreeMap;

use crate::domain::action::Item;
use crate::domain::activity::{Activity, ActivityKind, ActivityState, Script};
use crate::domain::cleanup::{OpsLog, SizeCache};
use crate::domain::context::{Host, Location, ScopeTag};
use crate::domain::custom::{CustomConfig, TrustStore};
use crate::domain::effect::Effect;
use crate::domain::exec::Command;
use crate::domain::plan::{Plan, PlanPhase, StepState};
use crate::domain::projection::{CatalogStamp, CleanupStamp, ProjectionEpochs};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    ActivityEnded { id: String, ok: bool },

    BatchDone { id: String, ok: bool },
    PlanStepEnded { plan: String, step: usize, ok: bool },
    PlanDone { plan: String },
    DiscoveryDone { source: String },
    ScanDone { generation: u64 },
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

    pub next_activity: u32,

    pub trusted_now: Vec<String>,

    pub healed_units: Vec<String>,

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

    pub explore_tiles: Vec<crate::domain::catalog::ExploreTile>,

    pub task_failures: Vec<String>,
    pub ops_log: OpsLog,
    pub size_cache: SizeCache,

    pub persisted: Persisted,

    pub reports: Vec<crate::domain::cleanup::Report>,

    pub scan: Option<ScanState>,

    pub fs_latency: Vec<(String, u64)>,

    pub cleanup_job: Option<CleanupJob>,

    pub(crate) epochs: ProjectionEpochs,
}

#[derive(Debug, Clone)]
pub struct CleanupJob {
    pub exec: crate::domain::cleanup::Execution,
    pub report_index: usize,

    pub processes: Vec<(String, crate::domain::cleanup::ProcessObservation)>,
}

pub type BatchMember = (String, Vec<Command>, Script, Option<Effect>, ScopeTag);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Running,
    Completed,
    Cancelled { revealed: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanState {
    pub root: String,
    pub started: u64,
    pub generation: u64,
    pub ticks: u64,
    pub tree: crate::sim::fs::ScanNode,

    pub order: Vec<String>,

    pub snapshot: Vec<(String, i64)>,
    pub include_hidden: bool,

    pub batch: usize,
    phase: ScanPhase,
    cache_attempted: bool,
}

impl ScanState {
    pub fn phase(&self) -> ScanPhase {
        self.phase
    }
    pub fn is_running(&self) -> bool {
        self.phase == ScanPhase::Running
    }
    pub fn completed(&self) -> bool {
        self.phase == ScanPhase::Completed
    }
    pub fn cancelled(&self) -> bool {
        matches!(self.phase, ScanPhase::Cancelled { .. })
    }
    pub fn revealed(&self, tick: u64) -> usize {
        match self.phase {
            ScanPhase::Cancelled { revealed } => revealed.min(self.order.len()),
            ScanPhase::Completed => self.order.len(),
            ScanPhase::Running => {
                let elapsed =
                    usize::try_from(tick.saturating_sub(self.started)).unwrap_or(usize::MAX);
                let ticks = usize::try_from(self.ticks.max(1)).unwrap_or(usize::MAX);
                let per_tick = self
                    .order
                    .len()
                    .div_ceil(ticks)
                    .max(1)
                    .min(self.batch.max(1));
                elapsed.saturating_mul(per_tick).min(self.order.len())
            }
        }
    }
    /// Moves a running scan to `Completed` once the tick reveals its last entry.
    /// Returns true only on that transition.
    pub fn advance(&mut self, tick: u64) -> bool {
        if !self.is_running() || self.revealed(tick) < self.order.len() {
            return false;
        }
        self.phase = ScanPhase::Completed;
        true
    }
    /// Freezes a running scan at the entries revealed by `tick`. Returns false
    /// when the scan already completed or was cancelled.
    pub fn cancel(&mut self, tick: u64) -> bool {
        if !self.is_running() {
            return false;
        }
        self.phase = ScanPhase::Cancelled {
            revealed: self.revealed(tick),
        };
        true
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

    /// Raw mutable access for fixtures and tests that build a world before any
    /// projection exists. Runtime code edits through `edit_activity` so the
    /// projection epochs see the change.
    pub fn activity_mut(&mut self, id: &str) -> Option<&mut Activity> {
        self.activities.iter_mut().find(|a| a.id == id)
    }

    pub fn catalog_stamp(&self) -> CatalogStamp {
        CatalogStamp {
            catalog: self.epochs.catalog(),
            fs: self.fs.content_revision(),
        }
    }

    pub fn cleanup_stamp(&self) -> CleanupStamp {
        CleanupStamp {
            cleanup: self.epochs.cleanup(),
            fs: self.fs.content_revision(),
        }
    }

    pub fn memory_epoch(&self) -> u64 {
        self.epochs.memory()
    }

    /// An explicit catalog edit transaction: the epoch moves once when `edit`
    /// returns. Never call it from a read path or a tick that may find nothing
    /// to do.
    pub fn edit_catalog<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let out = edit(self);
        self.epochs.bump_catalog();
        out
    }

    /// `edit_catalog` for an edit that may change nothing. The epoch moves only
    /// when `edit` reports a change.
    pub fn edit_catalog_if(&mut self, edit: impl FnOnce(&mut Self) -> bool) -> bool {
        let changed = edit(self);
        if changed {
            self.epochs.bump_catalog();
        }
        changed
    }

    /// An edit of provider, process, platform or disk state the cleanup
    /// candidates read. Also moves the catalog epoch, which embeds their totals.
    pub fn edit_cleanup<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let out = edit(self);
        self.epochs.bump_cleanup();
        out
    }

    pub fn edit_memory<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let out = edit(self);
        self.epochs.bump_memory();
        out
    }

    /// Edits one activity and moves the catalog epoch only when the part the
    /// catalog reads changed. Output lines and stdin are display metadata and
    /// never bump.
    pub fn edit_activity<R>(
        &mut self,
        id: &str,
        edit: impl FnOnce(&mut Activity) -> R,
    ) -> Option<R> {
        let a = self.activities.iter_mut().find(|a| a.id == id)?;
        let before = a.catalog_shape();
        let out = edit(a);
        if a.catalog_shape() != before {
            self.epochs.bump_catalog();
        }
        Some(out)
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
        self.epochs.bump_catalog();
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
        if !ids.is_empty() {
            self.epochs.bump_catalog();
        }
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
        if n > 0 {
            self.epochs.bump_catalog();
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
        if self.memory.usage.enabled {
            self.epochs.bump_memory();
        }
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
                self.epochs.bump_catalog();
            }
        }
        let tick = self.tick;
        let completed = self
            .scan
            .as_mut()
            .and_then(|s| s.advance(tick).then_some(s.generation));
        if let Some(generation) = completed {
            self.finish_scan_cache(generation);
            msgs.push(Msg::ScanDone { generation });
        }
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
            self.edit_activity(&id, |a| a.start(tick));
        }
        let mut transitioned = false;
        for a in &mut self.activities {
            let was = a.state;
            a.advance(tick);
            transitioned |= a.state != was;
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
        if transitioned {
            self.epochs.bump_catalog();
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
        if !matches!(
            e,
            Effect::GitFetch(_) | Effect::AmpUpdate | Effect::OmzUpgrade
        ) {
            self.epochs.bump_cleanup();
        }
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
        self.epochs.bump_cleanup();
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
        self.epochs.bump_cleanup();
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
        let generation = self.scan.as_ref().map_or(1, |s| {
            s.generation
                .checked_add(1)
                .expect("scan generation overflow")
        });
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
            include_hidden,
            batch: 4096,
            phase: ScanPhase::Running,
            cache_attempted: false,
        });
    }

    pub fn cancel_scan(&mut self) -> bool {
        let tick = self.tick;
        self.scan.as_mut().is_some_and(|s| s.cancel(tick))
    }

    /// Persists the size cache for the completed scan `generation`. Runs at most
    /// once per generation, and never for a running, cancelled or replaced scan.
    /// Returns true only when the cache was written.
    pub fn finish_scan_cache(&mut self, generation: u64) -> bool {
        let Some(scan) = self.scan.as_mut() else {
            return false;
        };
        if scan.generation != generation || !scan.completed() || scan.cache_attempted {
            return false;
        }
        scan.cache_attempted = true;
        let root = scan.root.clone();
        let tree = scan.tree.clone();
        let snapshot = scan.snapshot.clone();
        let now = self.now_secs();
        let on_disk = crate::domain::cleanup::SizeCache::load(self.persisted.sizes.as_deref(), now);
        self.size_cache.merge(&on_disk, now);
        self.size_cache
            .record(&root, &tree, &snapshot, &self.fs, now);
        if self.size_cache.save_error.is_some() {
            return false;
        }
        self.persisted.sizes = Some(self.size_cache.serialize());
        true
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
pub(crate) mod scan_test_support {
    use super::*;
    use crate::domain::fixtures::world_for;
    use crate::scenario::Motion;

    /// A running scan of `/work/big`, which holds `children` files plus its root.
    pub fn scan_world(children: usize, scan_ticks: u64) -> World {
        let mut w = world_for(Scenario::DiskCleanup, Motion::Full);
        w.clock.running = true;
        w.fs = Fs::new();
        w.fs.dir("/work/big", 1);
        for i in 0..children {
            w.fs.file(&format!("/work/big/f{i:05}"), 4096, 1);
        }
        w.disk.scan_ticks = scan_ticks;
        w.start_scan("/work/big");
        w
    }

    pub fn scan_done_count(msgs: &[Msg]) -> usize {
        msgs.iter()
            .filter(|m| matches!(m, Msg::ScanDone { .. }))
            .count()
    }

    pub fn scan_generation(w: &World) -> u64 {
        w.scan.as_ref().expect("scan").generation
    }

    /// Ticks until the running scan completes and returns every message emitted.
    pub fn run_to_completion(w: &mut World) -> Vec<Msg> {
        let mut out = vec![];
        for _ in 0..200 {
            out.extend(w.tick());
            if w.scan.as_ref().is_some_and(|s| s.completed()) {
                return out;
            }
        }
        panic!("scan never completed");
    }
}

#[cfg(test)]
mod scan_lifecycle_tests {
    use super::scan_test_support::{
        run_to_completion, scan_done_count, scan_generation, scan_world,
    };
    use super::*;
    use crate::domain::fixtures::world_for;
    use crate::scenario::Motion;

    #[test]
    fn scan_completion_waits_for_actual_last_reveal() {
        for children in [0usize, 4095, 4096, 12288] {
            for scan_ticks in [0u64, 1, 3] {
                let mut w = scan_world(children, scan_ticks);
                let len = w.scan.as_ref().expect("scan").order.len();
                assert_eq!(len, children + 1);
                let mut ticks_run = 0;
                loop {
                    ticks_run += 1;
                    assert!(
                        ticks_run < 64,
                        "len {len} ticks {scan_ticks} never completed"
                    );
                    let msgs = w.tick();
                    let scan = w.scan.as_ref().expect("scan");
                    if !scan.completed() {
                        assert!(scan.revealed(w.tick) < len);
                        assert_eq!(scan_done_count(&msgs), 0, "ScanDone before last entry");
                        assert_eq!(w.persisted.sizes, None, "cache before completion");
                        continue;
                    }
                    assert_eq!(scan.revealed(w.tick), len);
                    assert_eq!(scan_done_count(&msgs), 1);
                    assert!(w.persisted.sizes.is_some());
                    break;
                }
                for _ in 0..20 {
                    assert_eq!(scan_done_count(&w.tick()), 0, "one publication per scan");
                }
            }
        }
    }

    #[test]
    fn scan_zero_duration_completes_after_normalized_tick() {
        let mut w = scan_world(2, 0);
        assert_eq!(scan_done_count(&w.tick()), 1);
        assert!(w.scan.as_ref().expect("scan").completed());
        assert!(w.persisted.sizes.is_some());
    }

    #[test]
    fn scan_with_empty_traversal_completes_on_first_tick() {
        let mut w = scan_world(0, 5);
        w.scan.as_mut().expect("scan").order.clear();
        assert_eq!(scan_done_count(&w.tick()), 1);
        assert!(w.scan.as_ref().expect("scan").completed());
    }

    #[test]
    fn scan_cancelled_partial_never_publishes_cache() {
        let mut w = scan_world(4096, 1);
        w.tick();
        assert!(w.cancel_scan());
        let frozen = w.scan.as_ref().expect("scan").revealed(w.tick);
        assert_eq!(frozen, 4096);
        assert!(
            !w.cancel_scan(),
            "a cancelled scan cannot be cancelled again"
        );
        for _ in 0..10 {
            assert_eq!(scan_done_count(&w.tick()), 0);
        }
        let scan = w.scan.as_ref().expect("scan");
        assert!(scan.cancelled());
        assert_eq!(scan.revealed(w.tick), frozen);
        let generation = scan.generation;
        assert_eq!(w.persisted.sizes, None);
        assert!(
            !w.finish_scan_cache(generation),
            "cancelled scans never cache"
        );
        assert_eq!(w.persisted.sizes, None);
    }

    #[test]
    fn scan_cancel_after_completion_is_rejected() {
        let mut w = scan_world(2, 1);
        run_to_completion(&mut w);
        assert!(!w.cancel_scan());
        assert!(w.scan.as_ref().expect("scan").completed());
    }

    #[test]
    fn scan_generation_replacement_rejects_old_cache_publication() {
        let mut w = scan_world(2, 1);
        let gen1 = scan_generation(&w);
        w.start_scan("/work/big");
        let gen2 = scan_generation(&w);
        assert_eq!(gen2, gen1 + 1);
        assert!(
            !w.finish_scan_cache(gen1),
            "a replaced running scan never caches"
        );
        assert_eq!(w.persisted.sizes, None);
        let msgs = run_to_completion(&mut w);
        assert_eq!(
            msgs.iter()
                .filter(|m| **m == Msg::ScanDone { generation: gen2 })
                .count(),
            1
        );
        assert!(!w.finish_scan_cache(gen1));
        assert!(
            !w.finish_scan_cache(gen2),
            "the current completion already attempted"
        );

        w.start_scan("/work/big");
        let gen3 = scan_generation(&w);
        assert!(
            !w.finish_scan_cache(gen2),
            "a completed scan replaced by gen3 cannot cache"
        );
        assert!(!w.scan.as_ref().expect("scan").completed());
        assert_eq!(gen3, gen2 + 1);
    }

    #[test]
    fn scan_repeated_ticks_and_cache_calls_are_idempotent() {
        let mut w = scan_world(3, 1);
        let generation = scan_generation(&w);
        assert_eq!(scan_done_count(&run_to_completion(&mut w)), 1);
        let written = w.persisted.sizes.clone();
        assert!(written.is_some());
        for _ in 0..100 {
            assert_eq!(scan_done_count(&w.tick()), 0);
        }
        assert!(!w.finish_scan_cache(generation));
        assert_eq!(w.persisted.sizes, written);
    }

    #[test]
    fn scan_save_failure_completes_once_with_error_preserved() {
        let mut w = scan_world(2, 1);
        let generation = scan_generation(&w);
        w.size_cache.save_error = Some("disk full".into());
        assert_eq!(scan_done_count(&run_to_completion(&mut w)), 1);
        assert!(w.scan.as_ref().expect("scan").completed());
        assert_eq!(
            w.persisted.sizes, None,
            "a failed save leaves the cache unchanged"
        );
        assert_eq!(w.size_cache.save_error.as_deref(), Some("disk full"));
        w.size_cache.save_error = None;
        assert!(
            !w.finish_scan_cache(generation),
            "the failed attempt is not retried"
        );
        assert_eq!(w.persisted.sizes, None);
        for _ in 0..20 {
            assert_eq!(scan_done_count(&w.tick()), 0);
        }
    }

    #[test]
    fn scan_without_state_has_no_legacy_timer_completion() {
        let mut w = world_for(Scenario::DiskCleanup, Motion::Full);
        w.clock.running = true;
        assert!(w.scan.is_none());
        for _ in 0..200 {
            assert_eq!(scan_done_count(&w.tick()), 0);
        }
        assert_eq!(w.persisted.sizes, None);
    }

    #[test]
    fn scan_reveal_large_tick_values_do_not_wrap() {
        let w = scan_world(2, u64::MAX);
        let scan = w.scan.as_ref().expect("scan");
        assert_eq!(scan.revealed(0), 0);
        assert_eq!(scan.revealed(1), 1);
        assert_eq!(scan.revealed(u64::MAX), scan.order.len());
        let w = scan_world(2, 0);
        assert_eq!(w.scan.as_ref().expect("scan").revealed(1), 3);
    }
}
