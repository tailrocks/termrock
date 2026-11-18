//! Jackin launch cockpit, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j05_launch/`. Stage durations are sim ticks. Reduced motion
//! steps three of them on each 33 ms wall tick, so these waits follow the wall
//! clock. `Launch::new` leaves the clock off. `App::new` would enable it for
//! jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const SUMMARY: &str = "The Construct network could not be attached";
const FAILED: &str = "Launch failed";
const RESOLVING: &str = "Resolving credentials…";
const DOCKER: &str = "network jackin-net not found";
const CAPSULE_NOT_STARTED: &str = "capsule: not started";
const VERSION: &str = "Claude Code v2.1.14";
const PANE: &str = "Ctrl+B Pane commands";
const WORK_TAB: &str = "Claude Code (Work)";
const HARDLINE: &str = "Hardline open · handing off to the Capsule";
const DIRECTORY: &str = "Current directory";
const FAILED_SETUP: &str = "failed setup";
const NOTHING: &str = "nothing to attach to";
const RECORD: &str = "b5df";
const STILL_TWO: &str = "2 instances still running in the Construct";
const THREE_RUNNING: &str = "3 running";
const CRED_TITLE: &str = "Credential source unavailable";
const OP_LOCKED: &str = "1Password locked: unlock the app and retry";
const PAUSED_HEAD: &str = "The launch is paused";
const PAUSED_TAIL: &str = "at the Credentials stage";
const BLOCKED: &str = "Blocked · this stage cannot continue";
const BLOCKED_ESC: &str = "This stage is blocked · x cancels the launch, Ctrl+Q quits";
const UNREADABLE: &str = "instance index unreadable";
const ARCHITECT_PICKER: &str = "payments-platform › the-architect";
const SRE_PICKER: &str = "infra-control-plane › sre";
const WIDE: &str = "J05.launch.stages.120x40";
const NARROW: &str = "J05.launch.stages.narrow";
const STAGES: [&str; 11] = [
    "01 Identity",
    "02 Role",
    "03 Credentials",
    "04 Construct",
    "05 Agent Binaries",
    "06 Derived Image",
    "07 Workspace",
    "08 Network",
    "09 Sidecar",
    "10 Capsule",
    "11 Hardline",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j05_launch/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn failure_at(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    let failure_visible = if cols < 100 {
        visible.contains("network: attach jackin-net")
            && visible.contains("docker: Error response from daemon")
            && visible.contains("found")
            && visible.contains("Close")
    } else {
        visible.contains(DOCKER) && visible.contains(CAPSULE_NOT_STARTED)
    };
    screen.cols() == cols
        && screen.rows() == rows
        && visible.contains(FAILED)
        && visible.contains(SUMMARY)
        && visible.contains("Network")
        && failure_visible
        && !visible.contains(RESOLVING)
        && !visible.contains(VERSION)
}

fn running_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(VERSION)
        && visible.contains(PANE)
        && visible.contains(WORK_TAB)
        && !visible.contains(SUMMARY)
        && !visible.contains(FAILED)
        && !visible.contains(HARDLINE)
        && !visible.contains(CRED_TITLE)
}

fn network_ack(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DIRECTORY)
        && visible.contains(FAILED_SETUP)
        && visible.contains(NOTHING)
        && visible.contains(RECORD)
        && visible.contains(STILL_TWO)
        && !visible.contains(SUMMARY)
        && !visible.contains(VERSION)
        && !visible.contains(PANE)
        && !visible.contains(RESOLVING)
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    // Resize can paint the failure copy before the cells match the reference.
    // Keep the first matching paint only when the frozen check agrees.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut missed = String::new();
    loop {
        let screen = app.screen();
        if accept(&screen) {
            if let Some(why) = frame_mismatch(app, &screen, &test_id) {
                missed = why;
            } else {
                return;
            }
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{test_id} reference frame not seen: {missed}\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn frame_mismatch(app: &App, screen: &TerminalSnapshot, test_id: &str) -> Option<String> {
    let root = crate::support::oracle_baselines();
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    match outcome {
        CheckOutcome::Pass(_) => None,
        CheckOutcome::Failure(failure) => Some(format!(
            "{} sha {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.changed_cell_total()
        )),
    }
}

#[test]
fn j05_launch_route() {
    let app = open("j05-launch-route", "launch-failure");
    let dialog = app.wait_screen("launch failed", |screen| failure_at(screen, 120, 40));
    let visible = text(&dialog);
    assert!(visible.contains(FAILED), "{visible}");
    assert!(visible.contains(SUMMARY), "{visible}");
    assert!(visible.contains("Network"), "{visible}");
    assert!(!visible.contains(RESOLVING), "{visible}");
    assert!(!visible.contains(VERSION), "{visible}");
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    app.key("escape");
    let manager = app.wait_screen("manager after failure", network_ack);
    assert!(network_ack(&manager), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    app.finish();
}

#[test]
fn j05_launch_stages() {
    let running = open("j05-launch-stages-running", "launch-running");
    let capsule = running.wait_screen("clean capsule", running_capsule);
    let visible = text(&capsule);
    // Ready sets the hardline activity and then Go::Attach. Reduced motion
    // calls finish_handoff in that same turn, so the cockpit never paints it.
    assert!(visible.contains(VERSION), "{visible}");
    assert!(visible.contains(WORK_TAB), "{visible}");
    assert!(!visible.contains(HARDLINE), "{visible}");
    assert!(!visible.contains(SUMMARY), "{visible}");
    assert!(!visible.contains(FAILED), "{visible}");
    running.kill_child();

    let app = open("j05-launch-stages-failure", "launch-failure");
    let wide = app.wait_screen("failure 120x40", |screen| failure_at(screen, 120, 40));
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(text(&wide).contains(FAILED), "{}", text(&wide));
    assert!(text(&wide).contains(SUMMARY), "{}", text(&wide));
    assert!(!text(&wide).contains(RESOLVING), "{}", text(&wide));
    shot(&app, WIDE, |screen| failure_at(screen, 120, 40));
    app.resize(80, 24);
    shot(&app, NARROW, |screen| failure_at(screen, 80, 24));
    app.resize(120, 40);
    let restored = app.wait_screen("failure 120x40 again", |screen| failure_at(screen, 120, 40));
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    // After the resize, the first Esc can land before the failure layer has
    // focus. A later Esc dismisses it. Stop once the dialog is gone so Esc
    // does not reach the manager.
    for _ in 0..3 {
        let screen = app.screen();
        if network_ack(&screen) {
            break;
        }
        let visible = text(&screen);
        if visible.contains(FAILED) && visible.contains(SUMMARY) && visible.contains(DOCKER) {
            app.key("escape");
            std::thread::sleep(Duration::from_millis(300));
        } else {
            break;
        }
    }
    let manager = app.wait_screen("manager after the shots", network_ack);
    assert!(network_ack(&manager), "{}", text(&manager));
    app.finish();

    let cred = open("j05-launch-stages-cred", "hard-cases");
    cred.wait_screen("hard manager", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) && visible.contains(UNREADABLE)
    });
    cred.key("enter");
    cred.wait_screen("architect picker", |screen| {
        text(screen).contains(ARCHITECT_PICKER)
    });
    cred.key("down");
    cred.key("space");
    cred.key("enter");
    let locked = cred.wait_screen("credentials locked", |screen| {
        let visible = text(screen);
        visible.contains(CRED_TITLE)
            && visible.contains(OP_LOCKED)
            && visible.contains(PAUSED_HEAD)
            && visible.contains(PAUSED_TAIL)
    });
    let visible = text(&locked);
    assert!(visible.contains("03 Credentials"), "{visible}");
    assert!(!visible.contains(SUMMARY), "{visible}");
    assert!(!visible.contains(VERSION), "{visible}");
    cred.key("escape");
    let cred_back = cred.wait_screen("manager after credentials", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY)
            && visible.contains(FAILED_SETUP)
            && visible.contains(NOTHING)
            && visible.contains(THREE_RUNNING)
            && !visible.contains(CRED_TITLE)
            && !visible.contains(VERSION)
    });
    assert!(
        text(&cred_back).contains(FAILED_SETUP),
        "{}",
        text(&cred_back)
    );
    cred.finish();

    let blocked = open("j05-launch-stages-blocked", "hard-cases");
    blocked.wait_screen("hard manager again", |screen| {
        text(screen).contains(DIRECTORY) && text(screen).contains(UNREADABLE)
    });
    blocked.key("down");
    blocked.key("down");
    blocked.wait_screen("infra selected", |screen| {
        text(screen).contains("allowed all")
    });
    blocked.key("enter");
    blocked.wait_screen("sre picker", |screen| text(screen).contains(SRE_PICKER));
    blocked.key("space");
    blocked.key("enter");
    let side = blocked.wait_screen("sidecar blocked", |screen| {
        let visible = text(screen);
        visible.contains(BLOCKED) && visible.contains("09 Sidecar") && visible.contains("blocked")
    });
    let visible = text(&side);
    for label in STAGES {
        assert!(visible.contains(label), "{label} missing\n{visible}");
    }
    assert!(visible.contains("skipped"), "{visible}");
    assert!(!visible.contains(SUMMARY), "{visible}");
    assert!(!visible.contains(CRED_TITLE), "{visible}");
    assert!(!visible.contains(VERSION), "{visible}");
    blocked.key("escape");
    let held = blocked.wait_screen("blocked stays", |screen| {
        let visible = text(screen);
        visible.contains(BLOCKED) && visible.contains(BLOCKED_ESC)
    });
    assert!(text(&held).contains("09 Sidecar"), "{}", text(&held));
    blocked.key("ctrl+c");
    let back = blocked.wait_screen("manager after blocked", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY)
            && visible.contains(FAILED_SETUP)
            && visible.contains(THREE_RUNNING)
            && !visible.contains(BLOCKED)
            && !visible.contains(VERSION)
    });
    assert!(text(&back).contains(NOTHING), "{}", text(&back));
    blocked.finish();
}

#[test]
fn j05_launch_scenario_launch_running() {
    let app = open("j05-launch-running", "launch-running");
    let capsule = app.wait_screen("launch-running capsule", running_capsule);
    let visible = text(&capsule);
    assert!(visible.contains(VERSION), "{visible}");
    assert!(visible.contains(WORK_TAB), "{visible}");
    assert!(visible.contains(PANE), "{visible}");
    assert!(!visible.contains(SUMMARY), "{visible}");
    assert!(!visible.contains(FAILED), "{visible}");
    assert!(!visible.contains(DOCKER), "{visible}");
    assert!(!visible.contains(HARDLINE), "{visible}");
    app.kill_child();
}

#[test]
fn j05_launch_scenario_launch_failure() {
    let app = open("j05-launch-failure", "launch-failure");
    let dialog = app.wait_screen("launch-failure dialog", |screen| {
        failure_at(screen, 120, 40)
    });
    let visible = text(&dialog);
    assert!(visible.contains(FAILED), "{visible}");
    assert!(visible.contains(SUMMARY), "{visible}");
    assert!(visible.contains("Network"), "{visible}");
    assert!(visible.contains(DOCKER), "{visible}");
    assert!(visible.contains(CAPSULE_NOT_STARTED), "{visible}");
    assert!(!visible.contains(VERSION), "{visible}");
    assert!(!visible.contains(PANE), "{visible}");
    assert!(!visible.contains(WORK_TAB), "{visible}");
    assert!(!visible.contains(RESOLVING), "{visible}");
    app.key("escape");
    let manager = app.wait_screen("launch-failure manager", network_ack);
    assert!(network_ack(&manager), "{}", text(&manager));
    app.finish();
}
