//! Step BG: the gaps the jackin console found when it was wired to the
//! baseline. Each test states one gap in terms of what the operator sees or
//! what the baseline words. The names of the gaps are in
//! `docs/P9.3-red-green.md`, section BG.
#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::arbiter::DiscoveryError;
use jackin_console::backend::{Backend, Completion, Effect, OpError};
use jackin_console::domain::agent::{Provider, UsageSurface};
use jackin_console::domain::docker::{EngineEndpoint, EngineState};
use jackin_console::domain::doctor::{
    DoctorCheck, DoctorFailure, DoctorReport, DoctorState, DoctorSurface,
};
use jackin_console::domain::instance::{Instance, InstanceEntry, InstanceId};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::vocabulary::{Notice, Unbuilt, Vocabulary};
use jackin_console::domain::workspace::RoleSource;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{DaemonHealth, Health, Msg, SaveFailure, TrustKind, TrustRow, World};
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, StatusKind, Theme};

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn live(scenario: Scenario) -> World {
    let mut world = world_for(scenario);
    world.surfaces.vocabulary = Vocabulary::openshell();
    world
}

fn timed(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn settle(app: &mut App) -> String {
    for _ in 0..40 {
        Application::handle(app, termrock::core::event::Input::Tick);
    }
    screen(app)
}

/// The simulator, except that one function may answer an effect first and
/// every effect is counted.
#[derive(Debug)]
struct Answering {
    inner: Arc<dyn Backend>,
    answer: fn(&Effect, &World) -> Option<Msg>,
    seen: Mutex<Vec<Effect>>,
}

impl Answering {
    fn new(answer: fn(&Effect, &World) -> Option<Msg>) -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            answer,
            seen: Mutex::new(Vec::new()),
        })
    }

    fn runs(&self) -> usize {
        self.seen
            .lock()
            .expect("seen")
            .iter()
            .filter(|e| **e == Effect::RunDoctor)
            .count()
    }
}

impl Backend for Answering {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        match (self.answer)(effect, world) {
            Some(msg) => Some(Completion::ready(0, msg)),
            None => self.inner.perform(effect, world),
        }
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }
}

fn has(shown: &str, text: &str) {
    assert!(shown.contains(text), "missing `{text}` in:\n{shown}");
}

fn lacks(shown: &str, text: &str) {
    assert!(!shown.contains(text), "unexpected `{text}` in:\n{shown}");
}

fn no_answer(_: &Effect, _: &World) -> Option<Msg> {
    None
}

// Gap 1: OpError has no "not available" variant.

#[test]
fn a_missing_one_password_says_it_is_not_available() {
    let mut app = timed(Scenario::AccountsMixed, live(Scenario::AccountsMixed));
    press(&mut app, "a");
    click_text(&mut app, "Choose…");
    let ticket = app.world.tickets;
    app.deliver(&Msg::OpLoaded {
        ticket,
        result: Err(OpError::NotAvailable),
    });
    let shown = squash(&screen(&mut app));
    has(&shown, "1Password is not available");
    has(&shown, "Install the 1Password CLI and sign in");
    assert!(!OpError::NotAvailable.retryable());
}

// Gap 4: TrustRow.kind is an enum.

#[test]
fn a_trust_row_opens_by_its_kind_and_shows_the_kind() {
    let rows = [
        ("github.com/acme/roles", TrustKind::Git),
        ("~/src/github.com/me/roles", TrustKind::Path),
        ("git@corp:infra/roles", TrustKind::Git),
    ];
    let mut world = world_for(Scenario::Returning);
    world.global.trust = rows
        .iter()
        .map(|(source, kind)| TrustRow {
            source: (*source).into(),
            kind: *kind,
            trusted: true,
            roles: 1,
        })
        .collect();
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "s 5 tab");
    let list = screen(&mut app);
    for (source, kind) in rows {
        let line = list
            .lines()
            .find(|l| l.contains(source))
            .unwrap_or_else(|| panic!("no row {source}\n{list}"));
        has(&squash(line), &format!("{source} {}", kind.label()));
    }
    assert_eq!(
        (TrustKind::Git.label(), TrustKind::Path.label()),
        ("git", "path")
    );

    press(&mut app, "o");
    assert_eq!(
        status(&app),
        "Opened https://github.com/acme/roles in the browser"
    );
    press(&mut app, "down o");
    assert_eq!(
        status(&app),
        "Revealed ~/src/github.com/me/roles in Finder",
        "a path that names github.com is still a folder"
    );
    press(&mut app, "down o");
    assert_eq!(status(&app), "Revealed git@corp:infra/roles in Finder");
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

// Gap 5: RoleSource::Git.branch is optional.

#[test]
fn a_git_role_without_a_branch_shows_its_url_alone() {
    let source = |branch: Option<&str>| RoleSource::Git {
        url: "github.com/acme/no-branch".into(),
        branch: branch.map(Into::into),
    };
    assert_eq!(
        source(Some("main")).label(),
        "github.com/acme/no-branch @ main"
    );
    assert_eq!(source(None).label(), "github.com/acme/no-branch");

    for (branch, expected) in [
        (None, "Source github.com/acme/no-branch"),
        (Some("main"), "Source github.com/acme/no-branch @ main"),
    ] {
        let mut world = world_for(Scenario::RoleBinding);
        for record in &mut world.surfaces.roles.as_mut().expect("roles").roles {
            record.source = source(branch);
        }
        let mut app = app_with(Scenario::RoleBinding, world);
        let shown = squash(&screen(&mut app));
        has(&shown, expected);
        if branch.is_some() {
            has(&shown, "no-branch @");
        } else {
            lacks(&shown, "no-branch @");
        }
    }
}

// Gap 6: the About labels and the discovery error come from the vocabulary.

#[test]
fn the_live_about_rows_use_the_vocabulary_labels() {
    let facts = |gateway: Option<&str>| LiveFacts {
        version: "1.4.0".into(),
        config_path: "/home/dev/.config/jackin/config.toml".into(),
        gateway: gateway.map(Into::into),
    };
    let vocabulary = Vocabulary::openshell();
    assert_eq!(
        Surfaces::openshell(facts(Some("local-gateway"))).about,
        [
            (vocabulary.about_product.clone(), "1.4.0".to_owned()),
            (vocabulary.about_gateway.clone(), "local-gateway".to_owned()),
            (
                vocabulary.about_config.clone(),
                "/home/dev/.config/jackin/config.toml".to_owned()
            ),
        ]
    );
    assert_eq!(Surfaces::openshell(facts(None)).about.len(), 2);

    let mut world = world_for(Scenario::Returning);
    world.surfaces = Surfaces::openshell(facts(Some("local-gateway")));
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "f10 right right down enter");
    let shown = squash(&screen(&mut app));
    has(&shown, "Product 1.4.0");
    has(&shown, "Gateway local-gateway");
    has(&shown, "Config /home/dev/.config/jackin/config.toml");
}

#[test]
fn the_discovery_error_reads_from_the_vocabulary() {
    for (world, expected, absent) in [
        (
            world_for(Scenario::Returning),
            "! instance index unreadable",
            "sandbox list",
        ),
        (
            live(Scenario::Returning),
            "! sandbox list unreadable",
            "index unreadable",
        ),
    ] {
        let mut world = world;
        world.arbiter.discovery = Err(DiscoveryError::IndexUnreadable);
        let mut app = app_with(Scenario::Returning, world);
        let shown = screen(&mut app);
        has(&shown, expected);
        lacks(&shown, absent);
    }
}

// Gap 7: the Next row of the save error comes from the error.

fn fail_with_advice(effect: &Effect, _: &World) -> Option<Msg> {
    fail(effect, Some("free some disk space, then Save again"))
}

fn fail_without_advice(effect: &Effect, _: &World) -> Option<Msg> {
    fail(effect, None)
}

fn fail(effect: &Effect, next: Option<&str>) -> Option<Msg> {
    let error = SaveFailure {
        error: "disk full".into(),
        next: next.map(Into::into),
    };
    match effect {
        Effect::SaveWorkspace { id, .. } => Some(Msg::WorkspaceSaved {
            id: *id,
            result: Err(error),
        }),
        Effect::SaveGlobal { .. } => Some(Msg::GlobalSaved { result: Err(error) }),
        _ => None,
    }
}

fn failed_save(answer: fn(&Effect, &World) -> Option<Msg>, keys: &str, title: &str) -> String {
    let mut world = live(Scenario::Returning);
    world.backend = Answering::new(answer);
    let mut app = timed(Scenario::Returning, world);
    press(&mut app, keys);
    press(&mut app, "right enter");
    let shown = settle(&mut app);
    let start = shown
        .lines()
        .position(|l| l.contains(title))
        .unwrap_or_else(|| panic!("no `{title}` on screen:\n{shown}"));
    shown
        .lines()
        .skip(start)
        .take(6)
        .map(squash)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_save_error_next_row_is_the_errors_own() {
    for (keys, title) in [
        ("down e enter x ctrl+s", "Save failed"),
        ("s enter space ctrl+s", "Settings error"),
    ] {
        let advised = failed_save(fail_with_advice, keys, title);
        has(&advised, "Error disk full");
        has(&advised, "Next free some disk space, then Save again");
        lacks(&advised, "check disk space and permissions");

        let silent = failed_save(fail_without_advice, keys, title);
        has(&silent, "Error disk full");
        has(&silent, "your edits are intact");
        lacks(&silent, "Next ");
    }
}

// Gap 8: the providers jackin knows.

#[test]
fn the_providers_jackin_knows_are_all_here() {
    let expected = [
        (Provider::Antigravity, "Antigravity"),
        (Provider::Hermes, "Hermes"),
        (Provider::Muse, "Muse"),
    ];
    for (provider, name) in expected {
        assert_eq!(provider.label(), name);
        assert_eq!(provider.short(), name);
        assert_eq!(provider.usage_surface(), UsageSurface::Unsupported);
        assert_eq!(provider.agent(), None);
        assert_eq!(provider.plain_key_label(), format!("{name} API key"));
        assert_eq!(provider.folder_label(), format!("{name} profile folder"));
    }
}

// Gap 9, 14, 15: gateway health and the doctor report in the chrome.

#[test]
fn a_snapshot_replaces_the_gateway_health() {
    let mut world = world_for(Scenario::Returning);
    let mut snapshot = sim::snapshot(&world);
    assert_eq!(snapshot.daemon_health, DaemonHealth::Healthy);

    snapshot.daemon_health = DaemonHealth::Stale;
    world.apply(snapshot.clone());
    assert_eq!(world.daemon_health, DaemonHealth::Stale);
    assert_eq!(world.health_view().health, Health::Stale);

    snapshot.daemon_health = DaemonHealth::Healthy;
    world.apply(snapshot);
    assert_eq!(world.daemon_health, DaemonHealth::Healthy);
    assert_eq!(world.health_view().health, Health::Healthy);
}

fn passing_doctor(world: &mut World) {
    let at = world.now_secs();
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Done(report(
        at,
        Some("jackin-local"),
        vec![check("Gateway health", StatusKind::Ok, Some("healthy"))],
    ));
}

#[test]
fn a_failed_listing_shows_even_when_a_doctor_report_exists() {
    let mut world = world_for(Scenario::Doctor);
    passing_doctor(&mut world);
    assert_eq!(world.health_view().health, Health::Healthy);
    let mut snapshot = sim::snapshot(&world);
    snapshot.daemon_health = DaemonHealth::Stale;
    world.apply(snapshot.clone());
    let view = world.health_view();
    assert_eq!(view.health, Health::Stale);
    assert_eq!(view.noun, "gateway");
    let mut app = app_with(Scenario::Doctor, world);
    has(&screen(&mut app), "▲ gateway stale");

    snapshot.daemon_health = DaemonHealth::Healthy;
    app.world.apply(snapshot);
    assert_eq!(app.world.health_view().health, Health::Healthy);
    lacks(&screen(&mut app), "stale");
}

fn report(at_secs: i64, gateway: Option<&str>, checks: Vec<DoctorCheck>) -> DoctorReport {
    DoctorReport {
        checks,
        at_secs,
        gateway: gateway.map(Into::into),
    }
}

fn check(label: &str, status: StatusKind, expected: Option<&str>) -> DoctorCheck {
    DoctorCheck {
        label: label.into(),
        status,
        summary: format!("{label} summary"),
        found: Some("found".into()),
        expected: expected.map(Into::into),
        fix: None,
    }
}

#[test]
fn no_usable_gateway_is_a_failing_chrome() {
    let mut world = world_for(Scenario::Doctor);
    let at = world.now_secs();
    let skipped = vec![
        check("Gateway registered", StatusKind::Fail, None),
        check("Gateway health", StatusKind::Skipped, None),
    ];
    let mut quiet = skipped.clone();
    quiet[0].status = StatusKind::Skipped;
    for (checks, label) in [(skipped, "one failing"), (quiet, "all skipped")] {
        world.surfaces.doctor.as_mut().expect("doctor").state =
            DoctorState::Done(report(at, None, checks));
        assert_eq!(world.health_view().health, Health::Failing, "{label}");
    }
    let mut app = app_with(Scenario::Doctor, world);
    has(&screen(&mut app), "▲ gateway failing");
    press(&mut app, "D");
    let shown = squash(&screen(&mut app));
    has(&shown, "degraded");
    has(&shown, "no usable daemon");
    lacks(&shown, "all checks pass");
}

#[test]
fn a_gateway_that_answered_keeps_a_report_of_skips_out_of_failing() {
    let mut world = world_for(Scenario::Doctor);
    let at = world.now_secs();
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Done(report(
        at,
        Some("jackin-local"),
        vec![check("Gateway health", StatusKind::Skipped, None)],
    ));
    assert_eq!(world.health_view().health, Health::Healthy);
}

#[test]
fn a_refresh_leaves_the_doctor_state_alone() {
    let mut world = world_for(Scenario::Doctor);
    let now = world.now_secs();
    let newer = report(
        now,
        Some("jackin-local"),
        vec![check("Newer", StatusKind::Ok, None)],
    );
    let older = report(
        now - 100,
        Some("jackin-local"),
        vec![check("Older", StatusKind::Fail, None)],
    );
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Done(newer.clone());

    let mut snapshot = sim::snapshot(&world);
    snapshot.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Done(older.clone());
    snapshot.surfaces.doctor.as_mut().expect("doctor").noun = "gateway (fresh)".into();
    world.apply(snapshot.clone());
    let doctor = world.surfaces.doctor.as_ref().expect("doctor");
    assert_eq!(doctor.state, DoctorState::Done(newer.clone()));
    assert_eq!(
        doctor.noun, "gateway (fresh)",
        "the snapshot still names it"
    );

    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Running {
        previous: Some(newer.clone()),
    };
    world.apply(snapshot.clone());
    assert!(world.surfaces.doctor.as_ref().expect("doctor").running());

    let mut bare = sim::snapshot(&world);
    bare.surfaces.doctor = None;
    world.apply(bare);
    assert!(
        world.surfaces.doctor.is_none(),
        "a snapshot without a doctor takes it away"
    );
    world.apply(snapshot);
    assert_eq!(
        world.surfaces.doctor.as_ref().expect("doctor").state,
        DoctorState::Done(older),
        "a snapshot that brings the doctor in sets its first state"
    );
}

// Gaps 11, 12, 13: the doctor that could not run, the optional expected value
// and the timer.

fn doctor_world(state: DoctorState) -> World {
    let mut world = live(Scenario::Doctor);
    world.surfaces.doctor.as_mut().expect("doctor").state = state;
    world
}

#[test]
fn a_doctor_that_could_not_run_says_why() {
    let at = world_for(Scenario::Doctor).now_secs();
    let previous = report(
        at,
        Some("jackin-local"),
        vec![check("Kept check", StatusKind::Ok, Some("a thing"))],
    );
    for (failure, words) in [
        (DoctorFailure::TimedOut, "no answer in time"),
        (
            DoctorFailure::Failed("the runtime stopped".into()),
            "The checks could not run: the runtime stopped",
        ),
    ] {
        let mut app = timed(
            Scenario::Doctor,
            doctor_world(DoctorState::Running {
                previous: Some(previous.clone()),
            }),
        );
        press(&mut app, "D");
        app.deliver(&Msg::DoctorFinished(Err(failure.clone())));
        let shown = squash(&screen(&mut app));
        has(&shown, words);
        has(&shown, "Kept check");
        let doctor = app.world.surfaces.doctor.as_ref().expect("doctor");
        assert!(!doctor.running());
        assert_eq!(doctor.failure().map(|f| f.0), Some(&failure));
        assert_eq!(app.world.health_view().health, Health::Failing);
        has(&squash(&screen(&mut app)), "▲ gateway failing");
    }

    let mut app = timed(Scenario::Doctor, doctor_world(DoctorState::NeverRun));
    press(&mut app, "D");
    app.deliver(&Msg::DoctorFinished(Err(DoctorFailure::TimedOut)));
    let shown = squash(&screen(&mut app));
    has(&shown, "no answer in time");
    has(&shown, "Run checks");
    assert_eq!(app.world.health_view().health, Health::Failing);

    press(&mut app, "r");
    assert!(
        app.world
            .surfaces
            .doctor
            .as_ref()
            .expect("doctor")
            .running()
    );
    app.deliver(&Msg::DoctorFinished(Ok(previous.clone())));
    let doctor = app.world.surfaces.doctor.as_ref().expect("doctor");
    assert_eq!(doctor.state, DoctorState::Done(previous));
    assert_eq!(doctor.failure(), None);
}

#[test]
fn the_expected_value_is_drawn_only_when_the_check_has_one() {
    let at = world_for(Scenario::Doctor).now_secs();
    let checks = vec![
        check(
            "With expectation",
            StatusKind::Ok,
            Some("one registered gateway"),
        ),
        check("Without expectation", StatusKind::Ok, None),
    ];
    let mut app = timed(
        Scenario::Doctor,
        doctor_world(DoctorState::Done(report(at, Some("jackin-local"), checks))),
    );
    press(&mut app, "D");
    let with = squash(&screen(&mut app));
    has(&with, "Expected one registered gateway");
    press(&mut app, "down");
    let without = squash(&screen(&mut app));
    has(&without, "Found found");
    lacks(&without, "Expected");
}

#[test]
fn the_world_reruns_the_doctor_when_its_timer_runs_out() {
    fn drive(rerun_secs: Option<i64>) -> usize {
        let backend = Answering::new(no_answer);
        let mut world = world_for(Scenario::Doctor);
        world.backend = backend.clone();
        {
            let doctor: &mut DoctorSurface = world.surfaces.doctor.as_mut().expect("doctor");
            doctor.rerun_secs = rerun_secs;
        }
        for _ in 0..100 {
            for msg in world.tick(1_000) {
                world.absorb(&msg);
            }
            while let Some(msg) = world.take_due() {
                world.absorb(&msg);
            }
        }
        backend.runs()
    }
    assert_eq!(drive(None), 0, "the demo leaves reruns to the operator");
    let runs = drive(Some(30));
    assert!(
        (2..=4).contains(&runs),
        "100 s at one run per 30 s and 1 s of checking is 3 runs, got {runs}"
    );

    let mut world = world_for(Scenario::Doctor);
    let doctor = world.surfaces.doctor.as_mut().expect("doctor");
    doctor.rerun_secs = Some(30);
    doctor.state = DoctorState::Running { previous: None };
    let before = world.jobs.len();
    world.tick(60_000);
    assert_eq!(world.jobs.len(), before, "a run in flight is not doubled");
}

// Gap 10: the sentences jackin wrote itself.

#[test]
fn the_unbuilt_sentences_are_the_vocabularys() {
    let live = Vocabulary::openshell();
    let expected = [
        (Unbuilt::SaveWorkspace, "Saving a workspace"),
        (Unbuilt::SaveSettings, "Saving settings"),
        (Unbuilt::SaveAccounts, "Saving accounts"),
        (Unbuilt::DeleteWorkspace, "Deleting a workspace"),
        (Unbuilt::RestoreSandbox, "Restoring a sandbox"),
        (Unbuilt::TrustRole, "Trusting a role source"),
        (Unbuilt::PrewarmImage, "Building an image"),
        (Unbuilt::StopSandbox, "Stopping a sandbox"),
        (Unbuilt::PurgeSandbox, "Purging a sandbox"),
        (Unbuilt::RefreshUsage, "Reading usage"),
        (Unbuilt::ValidateAccount, "Validating an account"),
        (Unbuilt::LoadRole, "Loading a role"),
        (Unbuilt::CheckRole, "Checking a role"),
        (Unbuilt::SaveRole, "Saving a role"),
        (Unbuilt::BindRole, "Binding a role"),
        (Unbuilt::AddRole, "Adding a role"),
        (Unbuilt::RemoveRole, "Removing a role"),
        (Unbuilt::Doctor, "The doctor"),
        (Unbuilt::Launch, "Launching a sandbox"),
        (Unbuilt::Pane, "A terminal pane"),
    ];
    assert_eq!(expected.len(), 20);
    for (what, subject) in expected {
        assert_eq!(
            live.unbuilt(what),
            format!("{subject} is not available in this build yet")
        );
    }
    let demo = Vocabulary::demo();
    assert_eq!(
        demo.unbuilt(Unbuilt::StopSandbox),
        "Stopping a container is not simulated in the preview"
    );
}

#[test]
fn the_notices_are_the_vocabularys() {
    let live = Vocabulary::openshell();
    let slow = Notice::GatewayUnreachable {
        gateway: "local".into(),
        reason: "no answer within 5 s".into(),
    };
    let labels = Notice::UnreadableLabels { count: 3 };
    assert_eq!(
        live.notice(&slow),
        "Could not list sandboxes on local: no answer within 5 s"
    );
    assert_eq!(
        live.notice(&labels),
        "3 sandboxes have labels jackin cannot read"
    );
    assert_eq!(
        live.notices(&[slow, labels]),
        "Could not list sandboxes on local: no answer within 5 s · 3 sandboxes have labels jackin cannot read"
    );
    assert_eq!(live.notices(&[]), "");
}

#[test]
fn the_engine_port_is_worded_by_the_vocabulary() {
    let port = std::num::NonZeroU16::new(2376).expect("a port");
    let live = Vocabulary::openshell();
    assert_eq!(
        live.engine_endpoint(&EngineEndpoint::TlsPort(port)),
        "tls port 2376"
    );
    assert_eq!(
        live.engine_endpoint(&EngineEndpoint::Address("host:1".into())),
        "host:1"
    );

    let mut world = live_world_with_engine(EngineEndpoint::TlsPort(port));
    world.surfaces.vocabulary = Vocabulary::openshell();
    for workspace in &mut world.workspaces {
        workspace.docker = true;
    }
    let mut app = app_with(Scenario::DockerSetting, world);
    press(&mut app, "down e");
    let shown = squash(&screen(&mut app));
    has(&shown, "On · jackin-local · tls port 2376");
}

fn live_world_with_engine(endpoint: EngineEndpoint) -> World {
    let mut world = world_for(Scenario::DockerSetting);
    world.surfaces.docker.as_mut().expect("docker").engine = EngineState::Configured { endpoint };
    world
}

#[test]
fn instance_ids_carry_the_prefix_the_baseline_owns() {
    let id = InstanceId::simulated("jk-5b0x");
    assert_eq!(id.short_label(), "jk-5b0x");
    let mut world = world_for(Scenario::Returning);
    let mut instance = world.instances[0]
        .as_available()
        .expect("available instance")
        .clone();
    instance.id = id;
    world.instances = vec![InstanceEntry::available(instance)];
    let mut app = app_with(Scenario::Returning, world);
    let shown = screen(&mut app);
    has(&shown, "5b0x");
    lacks(&shown, "jk-5b0x");
}
