//! Step BG3: the gaps the jackin launch step (J4) found. Each test states one
//! gap in terms of what the operator sees or what the baseline words. The
//! gaps and their API are in `docs/P9.3-red-green.md`, section BG3.
#![cfg(feature = "demo")]

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::backend::{Backend, Completion, Effect, FileSource, OnePassword, PlainKey};
use jackin_console::domain::account::{AccountId, Lifecycle};
use jackin_console::domain::agent::{Agent, AuthMode, Provider};
use jackin_console::domain::docker::{DockerCapability, DockerRiskText};
use jackin_console::domain::instance::{
    DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus,
};
use jackin_console::domain::launch::{
    LaunchContent, LaunchFailure, LaunchWords, NextStep, RefusalReason, RefusalText,
};
use jackin_console::domain::workspace::{Usability, usability_of};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{
    EnvAnswer, FailedLaunch, LaunchEvent, LaunchFeed, LaunchTarget, TypedKey,
};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::SimBackend;
use jackin_console::world::{Msg, World};
use termrock::core::event::{Input, Outcome};
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};
use termrock::widgets::steps::StepState;

fn iid(label: &str) -> InstanceId {
    InstanceId::simulated(label)
}

// A backend whose launch the test scripts, with the content (the words) it
// names. Everything else goes to the simulator.

#[derive(Debug, Default)]
struct Calls {
    cancels: usize,
}

#[derive(Debug)]
struct Feed {
    batches: VecDeque<Vec<LaunchEvent>>,
    cancel: Option<Instance>,
    calls: Arc<Mutex<Calls>>,
}

impl LaunchFeed for Feed {
    fn run_id(&self) -> &str {
        "R-7"
    }

    fn poll(&mut self, _: &World) -> Vec<LaunchEvent> {
        self.batches.pop_front().unwrap_or_default()
    }

    fn retry_credentials(&mut self, _: &mut dyn OnePassword) {}

    fn use_plain_text(&mut self, _: PlainKey) -> Option<TypedKey> {
        None
    }

    fn answer_env(&mut self, _: Vec<EnvAnswer>) {}

    fn cancel(&mut self, _: &World) -> Option<InstanceEntry> {
        self.calls.lock().expect("calls").cancels += 1;
        self.cancel.take().map(InstanceEntry::available)
    }
}

#[derive(Debug)]
struct Scripted {
    inner: Arc<dyn Backend>,
    content: Option<Arc<LaunchContent>>,
    feed: Mutex<Option<Feed>>,
}

impl Backend for Scripted {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.inner.perform(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.content
            .clone()
            .unwrap_or_else(|| self.inner.launch_content())
    }

    fn launch(&self, _: &World, _: &LaunchTarget) -> Box<dyn LaunchFeed> {
        Box::new(self.feed.lock().expect("feed").take().expect("one launch"))
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

/// The demo content, with its words changed by `edit`.
fn words(edit: impl FnOnce(&mut LaunchWords)) -> Arc<LaunchContent> {
    let mut content = (*SimBackend::shared().launch_content()).clone();
    edit(&mut content.words);
    Arc::new(content)
}

fn openshell() -> Arc<LaunchContent> {
    Arc::new(LaunchContent::openshell())
}

fn launch(
    mut world: World,
    content: Option<Arc<LaunchContent>>,
    batches: Vec<Vec<LaunchEvent>>,
    cancel: Option<Instance>,
) -> (App, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    world.backend = Arc::new(Scripted {
        inner: SimBackend::shared(),
        content,
        feed: Mutex::new(Some(Feed {
            batches: batches.into(),
            cancel,
            calls: calls.clone(),
        })),
    });
    let app = app_with_world(
        Scenario::LaunchRunning,
        world,
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    );
    (app, calls)
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..600 {
        if done(app) {
            return;
        }
        Application::handle(app, Input::Tick);
    }
    panic!("never reached: {what}\n{}", screen(app));
}

fn shows(text: &'static str) -> impl Fn(&mut App) -> bool {
    move |app| screen(app).contains(text)
}

fn has(shown: &str, text: &str) {
    assert!(shown.contains(text), "{text:?} missing:\n{shown}");
}

fn lacks(shown: &str, text: &str) {
    assert!(!shown.contains(text), "{text:?} shows:\n{shown}");
}

fn nothing_running(mut world: World) -> World {
    world.instances.clear();
    world.daemons.clear();
    world.sync_arbiter();
    world
}

fn running_stage(stage: usize) -> Vec<LaunchEvent> {
    vec![LaunchEvent::StageChanged(stage, StepState::Running)]
}

fn record(id: &str, status: InstanceStatus) -> Instance {
    Instance {
        id: iid(id),
        container: format!("jackin-{id}"),
        workspace: Some(1),
        workdir: Some("/sandbox/project".into()),
        role: Some("the-architect".into()),
        agent: Agent::Codex,
        status,
        created_secs: 1_700_000_000,
        last_seen_secs: 1_700_000_005,
        run_id: "R-7".into(),
        sessions: Ok(vec![]),
        daemon: DaemonSnapshot::Unavailable,
        branch: None,
        pr: None,
        default_branch: "main".into(),
        github: None,
        uncommitted: 0,
        unpushed: 0,
        account: None,
        accounts: vec![],
        agents: Vec::new(),
        task: None,
    }
}

fn failed(summary: &str, next_step: NextStep, instance: Option<Instance>) -> LaunchEvent {
    LaunchEvent::Failed(Box::new(FailedLaunch {
        failure: LaunchFailure {
            stage: 3,
            summary: summary.into(),
            next_step,
            detail: vec![],
        },
        instance: instance.map(InstanceEntry::available),
    }))
}

fn failure_screen(
    content: Option<Arc<LaunchContent>>,
    summary: &str,
    next_step: NextStep,
) -> String {
    let event = failed(summary, next_step, None);
    let (mut app, _) = launch(
        world_for(Scenario::LaunchRunning),
        content,
        vec![vec![event]],
        None,
    );
    tick_until(&mut app, "the failure", shows("Run id"));
    screen(&mut app)
}

fn refusal_screen(content: Option<Arc<LaunchContent>>, reason: RefusalReason) -> String {
    let (mut app, _) = launch(
        world_for(Scenario::LaunchRunning),
        content,
        vec![vec![LaunchEvent::Refused(reason)]],
        None,
    );
    tick_until(&mut app, "the refusal", shows("! "));
    screen(&mut app)
}

// Gap 1: the refusal view's title and next step come from the launch words.

fn role_refused() -> RefusalReason {
    RefusalReason::RoleNotAllowed {
        workspace: "payments-platform".into(),
        role: "writer".into(),
    }
}

#[test]
fn a_refusal_is_worded_by_the_launch_words() {
    let demo = refusal_screen(None, role_refused());
    for text in [
        "Role not allowed",
        "Workspace payments-platform does not allow role writer.",
        "nothing was created",
        "Allow it in the workspace's Roles tab, or load another role.",
    ] {
        has(&demo, text);
    }

    let other = refusal_screen(
        Some(words(|w| {
            w.role_not_allowed.title = "Not on the list".into();
            w.role_not_allowed.next_step = "Ask the owner to add {role}.".into();
            w.refused_state = "no sandbox exists".into();
        })),
        role_refused(),
    );
    for text in [
        "Not on the list",
        "Ask the owner to add writer.",
        "no sandbox exists",
    ] {
        has(&other, text);
    }
    lacks(&other, "Role not allowed");
    lacks(&other, "Allow it in");

    assert_eq!(
        LaunchWords::openshell().refusal(&role_refused()),
        RefusalText {
            title: "Role not allowed".into(),
            found: "Workspace payments-platform does not allow role writer.".into(),
            state: "nothing was created".into(),
            next_step: "Allow it in the workspace's Roles tab, or load another role.".into(),
        }
    );
}

// Gap 2: a failure's next step is a typed value, worded by the launch words.

#[test]
fn a_failure_words_its_typed_next_step() {
    let content = words(|w| {
        w.next_retry = "Retry {launch} on {sandbox}.".into();
        w.next_doctor = "Run the doctor.".into();
        w.next_credential = "Fix the key.".into();
    });
    for (step, text) in [
        (
            NextStep::Retry {
                sandbox: "jackin-p-0abc".into(),
                launch: "L1".into(),
            },
            "Retry L1 on jackin-p-0abc.",
        ),
        (NextStep::RunDoctor, "Run the doctor."),
        (NextStep::FixCredential, "Fix the key."),
    ] {
        let shown = failure_screen(Some(content.clone()), "The sandbox failed", step);
        has(&shown, "Next step");
        has(&shown, text);
    }

    let none = failure_screen(Some(content), "The sandbox failed", NextStep::None);
    has(&none, "The sandbox failed");
    has(&none, "Run id");
    lacks(&none, "Next step");

    let words = LaunchWords::openshell();
    assert_eq!(
        words.next_step(&NextStep::Retry {
            sandbox: "jackin-proj-0abc".into(),
            launch: "01hzy3k5m7p9r2t4v6w8x0abcd".into(),
        }),
        Some(
            "sandbox jackin-proj-0abc remains; retry with: jackin load --launch-id 01hzy3k5m7p9r2t4v6w8x0abcd"
                .into()
        )
    );
    assert_eq!(
        words.next_step(&NextStep::RunDoctor),
        Some("Run jackin doctor, fix the failing check, then launch again.".into())
    );
    assert_eq!(words.next_step(&NextStep::None), None);
}

// Gap 3: the failure dialog wraps what is longer than a line.

#[test]
fn a_long_summary_wraps_in_the_failure_dialog() {
    let summary = "The sandbox could not be created because the gateway stopped answering while \
                   the project upload was still running, and it has not answered since";
    let shown = failure_screen(None, summary, NextStep::None);
    has(&shown, "The sandbox could not be created because");
    has(&shown, "not answered since");
    has(&shown, "Close");
}

#[test]
fn a_long_next_step_wraps_in_the_failure_dialog() {
    let step = NextStep::Retry {
        sandbox: "jackin-platfor-szho".into(),
        launch: "01hzy3k5m7p9r2t4v6w8x0abcd".into(),
    };
    let shown = failure_screen(Some(openshell()), "The sandbox failed", step);
    has(&shown, "Next step");
    has(&shown, "01hzy3k5m7p9r2t4v6w8x0abcd");
    has(&shown, "Close");
}

// Gap 4: a directory with no saved workspace is offered the only ready
// account when no default exists (D26: "the only account").

fn codex_accounts(world: &World) -> Vec<AccountId> {
    world
        .accounts
        .by_provider(Provider::OpenAi)
        .map(|a| a.id.clone())
        .collect()
}

/// A world whose Codex accounts have no default, and of which only `keep`
/// stay enabled.
fn without_default(keep: &[&str]) -> World {
    let mut world = world_for(Scenario::Returning);
    for account in &mut world.accounts.accounts {
        if account.provider == Provider::OpenAi {
            account.default_for_provider = false;
            account.enabled = keep.contains(&account.id.as_str());
        }
    }
    world
}

#[test]
fn the_only_ready_account_is_offered_to_a_directory_with_no_default() {
    let world = without_default(&["acct-codex-primary"]);
    let offer = world.offer_for(Agent::Codex, None, None);
    assert_eq!(offer.accounts, ["acct-codex-primary"]);
    assert_eq!(offer.preselected.as_deref(), Some("acct-codex-primary"));
    assert_eq!(offer.blocked, None);

    let two = without_default(&["acct-codex-primary", "acct-codex-experiments"]);
    let offer = two.offer_for(Agent::Codex, None, None);
    assert_eq!(
        offer.accounts,
        Vec::<AccountId>::new(),
        "two are not the only"
    );
    assert_eq!(offer.preselected, None);

    let mut blocked_default = without_default(&["acct-codex-primary", "acct-codex-experiments"]);
    let account = blocked_default
        .accounts
        .get_mut("acct-codex-experiments")
        .expect("experiments");
    account.default_for_provider = true;
    account.lifecycle = Lifecycle::NeedsLogin;
    let offer = blocked_default.offer_for(Agent::Codex, None, None);
    assert_eq!(
        offer.accounts,
        Vec::<AccountId>::new(),
        "a default that cannot be used is not replaced by another account"
    );
    assert!(offer.blocked.is_some());
}

#[test]
fn a_launch_blocked_account_found_on_the_host_is_not_offered() {
    let mut world = without_default(&[]);
    let account = world
        .accounts
        .get_mut("acct-codex-primary")
        .expect("primary");
    account.enabled = true;
    account.origin = jackin_console::domain::account::AccountOrigin::Discovered;
    account.launch_blocked = Some(jackin_console::domain::account::LaunchBlock::UsageOnlySource);
    assert!(
        world
            .accounts
            .discovered_current(Provider::OpenAi)
            .is_some()
    );
    let offer = world.offer_for(Agent::Codex, None, None);
    assert_eq!(offer.accounts, Vec::<AccountId>::new());
    assert_eq!(offer.preselected, None);
    assert!(offer.blocked.is_some(), "the block is said, not skipped");
}

#[test]
fn no_demo_world_gains_an_account_from_the_only_rule() {
    for scenario in Scenario::ALL {
        let world = world_for(scenario);
        for agent in Agent::ALL {
            let provider = agent.provider();
            let ready = world
                .accounts
                .by_provider(provider)
                .filter(|a| usability_of(a) == Usability::Ready)
                .count();
            let offered = world
                .surfaces
                .agent_blocked
                .as_ref()
                .is_none_or(|limit| limit.launchable == agent)
                && world.agent_mode(agent) != AuthMode::Ignore;
            let would_gain = offered
                && world.accounts.default_for(provider).is_none()
                && world.accounts.discovered_current(provider).is_none()
                && ready == 1;
            assert!(
                !would_gain,
                "{} · {agent:?} would be offered an account it was not",
                scenario.name()
            );
        }
    }
    assert!(!codex_accounts(&world_for(Scenario::Returning)).is_empty());
}

// Gap 5: a launch that ends with no instance and nothing running returns to
// the manager when the backend's content says so; the demo keeps its outro.

fn end_with_nothing_running(content: Option<Arc<LaunchContent>>, how: &str) -> App {
    let (event, cancel) = match how {
        "failure" => (
            failed(
                "The sandbox failed",
                NextStep::None,
                Some(record("jk-fail", InstanceStatus::FailedSetup)),
            ),
            None,
        ),
        "refusal" => (LaunchEvent::Refused(role_refused()), None),
        _ => (
            LaunchEvent::Activity("Uploading the project…".into()),
            Some(record("jk-stop", InstanceStatus::FailedSetup)),
        ),
    };
    let world = nothing_running(world_for(Scenario::LaunchRunning));
    let (mut app, _) = launch(world, content, vec![running_stage(0), vec![event]], cancel);
    match how {
        "failure" => {
            tick_until(&mut app, "the failure", shows("Run id"));
            press(&mut app, "esc");
        }
        "refusal" => {
            tick_until(&mut app, "the refusal", shows("Role not allowed"));
            press(&mut app, "esc");
        }
        _ => {
            tick_until(&mut app, "the upload", shows("Uploading the project"));
            press(&mut app, "x");
            press(&mut app, "right enter");
        }
    }
    app
}

#[test]
fn a_launch_that_leaves_nothing_returns_to_the_manager() {
    for how in ["failure", "refusal", "cancel"] {
        let mut app = end_with_nothing_running(Some(openshell()), how);
        assert_eq!(app.route(), Route::Manager, "{how}");
        assert!(app.outro.is_none(), "{how}");
        has(&screen(&mut app), "Current directory");
    }
}

#[test]
fn the_demo_leaves_through_the_outro_when_nothing_is_running() {
    for how in ["failure", "refusal", "cancel"] {
        let app = end_with_nothing_running(None, how);
        assert_eq!(app.route(), Route::Outro, "{how}");
    }
}

// Gap 6: an agent jackin does not launch is refused, in the launch words.

#[test]
fn an_agent_jackin_does_not_launch_is_refused_in_the_launch_words() {
    let reason = RefusalReason::AgentNotLaunchable {
        agent: Agent::ClaudeCode,
        launchable: Agent::Codex,
    };
    let world = world_for(Scenario::LaunchRunning);
    let others = world.instances.clone();
    let (mut app, calls) = launch(
        world,
        Some(openshell()),
        vec![vec![LaunchEvent::Refused(reason.clone())]],
        None,
    );
    tick_until(&mut app, "the refusal", shows("Agent not launchable"));
    let shown = screen(&mut app);
    for text in [
        "Claude Code cannot be launched: Codex is the only agent jackin launches.",
        "nothing was created",
        "Launch with Codex; Claude Code stays usage only.",
        "not started",
    ] {
        has(&shown, text);
    }
    assert_eq!(app.world.instances, others, "a refusal creates nothing");
    press(&mut app, "esc");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(calls.lock().expect("calls").cancels, 0);

    assert_eq!(
        LaunchWords::openshell().refusal(&reason).title,
        "Agent not launchable"
    );
}

// Gap 7: the record of a cancelled launch may come later, as a reply.

#[test]
fn a_cancel_record_that_comes_later_reaches_the_roster_at_once() {
    let (mut app, calls) = launch(
        world_for(Scenario::LaunchRunning),
        None,
        vec![running_stage(0)],
        None,
    );
    tick_until(&mut app, "the first stage", shows("stage 1 of"));
    press(&mut app, "x");
    press(&mut app, "right enter");
    assert_eq!(calls.lock().expect("calls").cancels, 1);
    assert_eq!(app.route(), Route::Manager);
    assert!(app.world.instance(&iid("jk-late")).is_none());
    let before = app.world.instances.len();

    let late = record("jk-late", InstanceStatus::FailedSetup);
    let reply = Msg::LaunchCancelled {
        instance: Box::new(InstanceEntry::available(late.clone())),
    };
    assert_eq!(app.deliver(&reply), Outcome::Changed);
    assert_eq!(app.world.instance(&iid("jk-late")), Some(&late));
    assert_eq!(app.world.instances.len(), before + 1);

    let mut newer = late;
    newer.last_seen_secs += 5;
    let reply = Msg::LaunchCancelled {
        instance: Box::new(InstanceEntry::available(newer.clone())),
    };
    app.deliver(&reply);
    assert_eq!(
        app.world.instances.len(),
        before + 1,
        "a record the roster already has is replaced, not added again"
    );
    assert_eq!(app.world.instance(&iid("jk-late")), Some(&newer));
}

// Gap 8: the Docker banner comes from the event when the backend sends one.

fn risk(label: &str, text: &str) -> DockerRiskText {
    DockerRiskText {
        label: label.into(),
        text: text.into(),
    }
}

fn warning(risks: Vec<DockerRiskText>) -> LaunchEvent {
    LaunchEvent::Warning(DockerCapability { risks })
}

#[test]
fn the_cockpit_draws_the_docker_banner_from_the_event() {
    let world = world_for(Scenario::LaunchRunning);
    assert!(world.surfaces.docker.is_none(), "the premise: no setting");
    let (mut app, _) = launch(
        world,
        None,
        vec![
            vec![warning(vec![
                risk("Engine key", "the agent can read the engine key"),
                risk("Network", "containers bypass the sandbox network policy"),
            ])],
            running_stage(0),
        ],
        None,
    );
    tick_until(&mut app, "the banner", shows("Docker is on"));
    let shown = screen(&mut app);
    has(&shown, "i shows all 2 risks");

    press(&mut app, "i");
    let info = screen(&mut app);
    has(&info, "Engine key · the agent can read the engine key");
    has(
        &info,
        "Network · containers bypass the sandbox network policy",
    );
}

#[test]
fn a_launch_without_a_warning_shows_no_docker_banner() {
    let world = world_for(Scenario::LaunchRunning);
    let (mut app, _) = launch(world, None, vec![running_stage(0)], None);
    tick_until(&mut app, "the first stage", shows("stage 1 of"));
    lacks(&screen(&mut app), "Docker is on");
}

fn docker_setting_world() -> World {
    let mut world = world_for(Scenario::DockerSetting);
    world.workspace_mut(1).expect("payments-platform").docker = true;
    world
}

#[test]
fn the_banner_comes_from_the_setting_when_the_backend_sends_no_warning() {
    let world = docker_setting_world();
    let risks = world
        .surfaces
        .docker
        .as_ref()
        .expect("a setting")
        .risks
        .len();
    assert!(
        risks > 2,
        "the premise: the setting names more than two risks"
    );
    let (mut app, _) = launch(world, None, vec![running_stage(0)], None);
    tick_until(&mut app, "the banner", shows("Docker is on"));
    has(&screen(&mut app), &format!("i shows all {risks} risks"));
}

#[test]
fn a_warning_from_the_backend_replaces_the_setting() {
    let (mut app, _) = launch(
        docker_setting_world(),
        None,
        vec![
            vec![warning(vec![risk(
                "Engine key",
                "the agent can read the key",
            )])],
            running_stage(0),
        ],
        None,
    );
    tick_until(
        &mut app,
        "the warning's banner",
        shows("i shows all 1 risk"),
    );
}
