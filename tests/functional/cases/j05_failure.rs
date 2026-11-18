//! Jackin launch failure, driven through the real binary.
//!
//! `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
//! The checkpoint is the open failure dialog, before Esc dismisses it.
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the directory is
//! `pre_refactor/j05_failure/`.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const TITLE: &str = "Launch failed";
const SUMMARY: &str = "The Construct network could not be attached";
const STAGE: &str = "Network";
const DOCKER: &str = "network jackin-net not found";
const CAPSULE: &str = "capsule: not started";
const HARDLINE: &str = "Hardline open";
const RESOLVING: &str = "Resolving credentials…";
const DIRECTORY: &str = "Current directory";
const FAILED_SETUP: &str = "failed setup";
const NOTHING: &str = "nothing to attach to";
const STILL_RUNNING: &str = "still running in the Construct";
const ACK: &str = "Launch failed ·";
const RUN_ID: &str = "run-202609030914-b5df";
const COPIED: &str = "Copied 21 characters · run-202609030914-b5df";
const CRED_TITLE: &str = "Credential source unavailable";
const LOCKED: &str = "1Password locked: unlock the app and retry";
const RETRY: &str = "Retry";
const PLAIN: &str = "Enter plain text instead";
const CANCEL: &str = "Cancel launch";
const UNLOCKED: &str = "1Password unlocked · resolving the reference again";
const PLAIN_STATUS: &str = "Using a plain-text key for this launch only";
const HOST_PROFILE: &str = "host profile (no registered account)";
const BLOCKED: &str = "Blocked · this stage cannot continue";
const BLOCKED_ESC: &str = "This stage is blocked · x cancels the launch, Ctrl+Q quits";
const WIDE: &str = "J05.failure.failure.120x40";
const NARROW: &str = "J05.failure.failure.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j05_failure/{}",
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

fn row_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    (0..screen.rows()).any(|row| row_text(screen, row).contains(needle))
}

fn failure_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(SUMMARY)
        && visible.contains(STAGE)
        && visible.contains(DOCKER)
        && visible.contains(CAPSULE)
        && !visible.contains(HARDLINE)
        && !visible.contains(RESOLVING)
        && !visible.contains("Docker build failed")
        && !visible.contains("Credential check failed")
        && !visible.contains(DIRECTORY)
}

fn failure_open_narrow(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(SUMMARY)
        && visible.contains(STAGE)
        && visible.contains("network jackin-net")
        && visible.contains("found")
        && !visible.contains(HARDLINE)
        && !visible.contains(RESOLVING)
        && !visible.contains("Docker build failed")
        && !visible.contains("Credential check failed")
        && !visible.contains(DIRECTORY)
}

fn failure_at(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    screen.cols() == cols
        && screen.rows() == rows
        && if cols < 100 {
            failure_open_narrow(screen)
        } else {
            failure_open(screen)
        }
}

fn stages_without_dialog(visible: &str) -> bool {
    !visible.contains(TITLE)
        && !visible.contains(SUMMARY)
        && (visible.contains("Preparing launch")
            || visible.contains("Resolving launch identity")
            || visible.contains("Loading the-architect"))
}

fn acked(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DIRECTORY)
        && visible.contains(FAILED_SETUP)
        && visible.contains(NOTHING)
        && visible.contains(ACK)
        && visible.contains(STILL_RUNNING)
        && !visible.contains(SUMMARY)
        && !visible.contains(HARDLINE)
        && !visible.contains(CRED_TITLE)
}

fn run_id_row(screen: &TerminalSnapshot) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains("Run id") && line.contains("y copy") && line.contains(RUN_ID)
    }) && failure_open(screen)
}

fn shot(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    let test_id = visual_id(checkpoint);
    let start = Instant::now();
    // The narrow hint paints "Close" one frame after the dialog body. A
    // hidden cursor can also sit on that word instead of the corner.
    loop {
        let screen = app.screen();
        if failure_at(&screen, cols, rows)
            && text(&screen).contains("Close")
            && frame_matches(app, &screen, &test_id)
        {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8) && app.child_running(),
            "{test_id} frame not seen\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn frame_matches(app: &App, screen: &TerminalSnapshot, test_id: &str) -> bool {
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
    let CheckOutcome::Failure(failure) = outcome else {
        return true;
    };
    let fields = failure.changed_fields();
    failure.changed_cell_total() == 0
        && !screen.cursor().visible()
        && !fields.is_empty()
        && fields
            .iter()
            .all(|field| field == "cursor.col" || field == "cursor.row")
}

fn esc_before_dialog_does_not_ack(app: &App) {
    let start = Instant::now();
    let mut sent = false;
    loop {
        let screen = app.screen();
        let visible = text(&screen);
        assert!(
            !visible.contains(DIRECTORY),
            "Esc reached the manager before {TITLE}\n{visible}"
        );
        if failure_open(&screen) {
            return;
        }
        if !sent && stages_without_dialog(&visible) {
            app.key("escape");
            sent = true;
        }
        assert!(
            start.elapsed() < Duration::from_secs(8) && app.child_running(),
            "{TITLE} did not open\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn launch_from_manager(app: &App, workspace_downs: u8, agent_downs: u8) {
    app.wait_screen("workspace list", |screen| {
        text(screen).contains("payments-platform")
    });
    for _ in 0..workspace_downs {
        app.key("down");
    }
    app.key("enter");
    app.wait_screen("agent picker", |screen| {
        text(screen).contains("choose Agent")
    });
    for _ in 0..agent_downs {
        app.key("down");
    }
    app.key("space");
    app.key("enter");
    app.wait_screen("cockpit", |screen| text(screen).contains("Loading"));
}

fn cred_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRED_TITLE)
        && visible.contains(LOCKED)
        && visible.contains(RETRY)
        && visible.contains(PLAIN)
        && visible.contains(CANCEL)
        && row_has(screen, "▎Retry")
        && !visible.contains(HARDLINE)
        && !visible.contains(DIRECTORY)
}

#[test]
fn j05_failure_route() {
    let app = open("j05-failure-route", "launch-failure");
    esc_before_dialog_does_not_ack(&app);
    let open_dialog = app.screen();
    assert!(failure_open(&open_dialog), "{}", text(&open_dialog));
    assert!(text(&open_dialog).contains(TITLE), "{}", text(&open_dialog));
    assert!(
        text(&open_dialog).contains(SUMMARY),
        "{}",
        text(&open_dialog)
    );
    assert!(text(&open_dialog).contains(STAGE), "{}", text(&open_dialog));
    app.key("escape");
    let manager = app.wait_screen("manager after failure", acked);
    assert!(acked(&manager), "{}", text(&manager));
    assert!(!text(&manager).contains(HARDLINE), "{}", text(&manager));
    app.finish();
}

#[test]
fn j05_failure_failure() {
    let app = open("j05-failure-failure", "launch-failure");
    let wide = app.wait_screen("failure 120x40", |screen| failure_at(screen, 120, 40));
    assert!(failure_open(&wide), "{}", text(&wide));
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    shot(&app, WIDE, 120, 40);
    app.resize(80, 24);
    shot(&app, NARROW, 80, 24);
    app.resize(120, 40);
    let restored = app.wait_screen("failure 120x40 again", |screen| failure_at(screen, 120, 40));
    assert!(failure_open(&restored), "{}", text(&restored));
    assert_eq!((restored.cols(), restored.rows()), (120, 40));

    app.key("down");
    let focused = app.wait_screen("run id row", run_id_row);
    assert!(run_id_row(&focused), "{}", text(&focused));
    assert!(failure_open(&focused), "{}", text(&focused));
    app.key("y");
    let copied = app.wait_screen("run id copied", |screen| {
        let visible = text(screen);
        visible.contains(COPIED)
            && visible.contains(TITLE)
            && visible.contains(SUMMARY)
            && !visible.contains(DIRECTORY)
            && !visible.contains(HARDLINE)
    });
    assert!(text(&copied).contains(COPIED), "{}", text(&copied));
    assert!(text(&copied).contains(TITLE), "{}", text(&copied));
    assert!(!text(&copied).contains(DIRECTORY), "{}", text(&copied));
    app.key("escape");
    let manager = app.wait_screen("manager after copy", acked);
    assert!(acked(&manager), "{}", text(&manager));
    app.finish();

    let retry = open("j05-failure-retry", "hard-cases");
    launch_from_manager(&retry, 1, 1);
    let retry_dialog = retry.wait_screen("credential dialog", cred_dialog);
    assert!(cred_dialog(&retry_dialog), "{}", text(&retry_dialog));
    retry.key("enter");
    let unlocked = retry.wait_screen("retry resumed", |screen| {
        let visible = text(screen);
        visible.contains(UNLOCKED)
            && visible.contains("Codex · Primary")
            && !visible.contains(CRED_TITLE)
            && !visible.contains(HARDLINE)
            && !visible.contains(DIRECTORY)
    });
    assert!(text(&unlocked).contains(UNLOCKED), "{}", text(&unlocked));
    assert!(!text(&unlocked).contains(HARDLINE), "{}", text(&unlocked));

    let plain = open("j05-failure-plain", "hard-cases");
    launch_from_manager(&plain, 1, 1);
    plain.wait_screen("plain dialog", cred_dialog);
    plain.key("right");
    plain.wait_screen("plain button", |screen| {
        row_has(screen, "▎Enter plain text instead") && text(screen).contains(CRED_TITLE)
    });
    plain.key("enter");
    plain.wait_screen("plain-text key form", |screen| {
        text(screen).contains("Plain-text key") && text(screen).contains("API key *")
    });
    plain.type_text("functional-test-key");
    plain.key("enter");
    let typed = plain.wait_screen("plain text key", |screen| {
        let visible = text(screen);
        visible.contains(PLAIN_STATUS)
            && visible.contains(HOST_PROFILE)
            && !visible.contains(CRED_TITLE)
            && !visible.contains(HARDLINE)
            && !visible.contains(DIRECTORY)
    });
    assert!(text(&typed).contains(PLAIN_STATUS), "{}", text(&typed));
    assert!(text(&typed).contains(HOST_PROFILE), "{}", text(&typed));

    let cancel = open("j05-failure-cancel", "hard-cases");
    launch_from_manager(&cancel, 1, 1);
    cancel.wait_screen("cancel dialog", cred_dialog);
    cancel.key("escape");
    let cancelled = cancel.wait_screen("manager after cancel", acked);
    assert!(acked(&cancelled), "{}", text(&cancelled));
    assert!(
        !text(&cancelled).contains("Launch cancelled · nothing was attached"),
        "{}",
        text(&cancelled)
    );
    cancel.finish();

    let blocked = open("j05-failure-blocked", "hard-cases");
    launch_from_manager(&blocked, 2, 0);
    let blocked_stage = blocked.wait_screen("sidecar blocked", |screen| {
        let visible = text(screen);
        visible.contains(BLOCKED)
            && visible.contains("Sidecar")
            && visible.contains("infra-control-plane")
            && !visible.contains(HARDLINE)
            && !visible.contains(CRED_TITLE)
            && !visible.contains(DIRECTORY)
    });
    assert!(
        text(&blocked_stage).contains(BLOCKED),
        "{}",
        text(&blocked_stage)
    );
    blocked.key("escape");
    let stayed = blocked.wait_screen("blocked esc", |screen| {
        let visible = text(screen);
        visible.contains(BLOCKED)
            && visible.contains(BLOCKED_ESC)
            && !visible.contains(HARDLINE)
            && !visible.contains(DIRECTORY)
    });
    assert!(text(&stayed).contains(BLOCKED_ESC), "{}", text(&stayed));
    assert!(!text(&stayed).contains(HARDLINE), "{}", text(&stayed));
}
