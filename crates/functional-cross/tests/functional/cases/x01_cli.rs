//! Cross-application CLI, driven through the real binaries.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/x01_cli/`. The shots are showcase `--help` after the process
//! has exited, captured at the spawn size. They do not nudge and they do not
//! resize. Clap is built without `wrap_help`, so the help block is taller than
//! 40 rows and the about line scrolls off both checkpoint viewports. The frames
//! still show the showcase after-help. `finish` sends Ctrl+Q, so a command
//! that has already exited uses `expect_exited` instead. `Launch::new` leaves
//! the clock off.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, SHOWCASE, TABLEPRO, text, try_launch};

const HELP_EXIT: i32 = 0;
const ERROR_EXIT: i32 = 2;
const SHOWCASE_ABOUT: &str = "Junie-tui — Junie-inspired Ratatui design system laboratory";
const TABLEPRO_ABOUT: &str = "TablePro — terminal database workbench on the Junie design system";
const HOLLA_ABOUT: &str =
    "Holla — this folder, this host, right now (deterministic preview on the Junie design system)";
const SHOWCASE_UI: &str = "Tokens and principles behind every component";
const TABLEPRO_UI: &str = "Local PostgreSQL";
const HOLLA_UI: &str = "Search actions and resources";
const HELP_TAIL: &str = "q quit";
const HELP_PRINT: &str = "Print help (see a summary with '-h')";
const HELP_FRAME: &str = "Frame:     with --motion paused, the exact tick to render";
const HELP_OPT: &str = "--opt <KEY=VALUE>";
const HELP_MOTION: &str = "SHOWCASE_NO_MOTION";
const LATER: &str = "brand compact";
const EARLIER: &str = "brand none";
const WIDE_ID: &str = "pre_refactor/x01_cli/x01_cli_cli_120x40";
const NARROW_ID: &str = "pre_refactor/x01_cli/x01_cli_cli_narrow";

fn open(name: &str, bin: &str, args: &[&str], cols: u16, rows: u16) -> App {
    let mut launch = Launch::new(name, bin);
    launch.args = args.iter().map(|arg| (*arg).to_string()).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn wait_settled(
    app: &App,
    reason: &str,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let same = last.as_ref().is_some_and(|prev| prev == &screen);
            if same {
                if since.expect("stability start").elapsed() >= Duration::from_millis(200) {
                    return screen;
                }
            } else {
                last = Some(screen);
                since = Some(Instant::now());
            }
        } else {
            last = None;
            since = None;
        }
        assert!(
            Instant::now() < deadline,
            "{reason} frame not seen\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn help_of(name: &str, bin: &str, about: &str, running: &str) {
    let app = open(name, bin, &["--help"], 200, 80);
    let screen = wait_settled(&app, name, |screen| text(screen).contains(about));
    require(&screen, about);
    refuse(&screen, running);
    assert!(!app.child_running(), "{name} left the UI running");
    app.expect_exited(HELP_EXIT);
}

fn rejected(
    name: &str,
    bin: &str,
    args: &[&str],
    cols: u16,
    rows: u16,
    named: &str,
    running: &str,
) {
    let app = open(name, bin, args, cols, rows);
    let screen = wait_settled(&app, name, |screen| text(screen).contains(named));
    require(&screen, named);
    refuse(&screen, running);
    assert!(!app.child_running(), "{name} left the UI running");
    app.expect_exited(ERROR_EXIT);
}

fn help_frame(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(HELP_TAIL)
        && visible.contains(HELP_PRINT)
        && visible.contains(HELP_FRAME)
        && visible.contains(HELP_OPT)
        && visible.contains(HELP_MOTION)
        && !visible.contains(SHOWCASE_UI)
}

fn shot(app: &App, test_id: &str, cols: u32, rows: u32) {
    let screen = wait_settled(app, test_id, help_frame);
    assert_eq!(screen.cols(), cols, "{test_id} cols");
    assert_eq!(screen.rows(), rows, "{test_id} rows");
    require(&screen, HELP_PRINT);
    require(&screen, HELP_FRAME);
    require(&screen, HELP_OPT);
    require(&screen, HELP_MOTION);
    require(&screen, HELP_TAIL);
    refuse(&screen, SHOWCASE_UI);
    assert!(!app.child_running(), "{test_id} left the UI running");
    app.expect_exited(HELP_EXIT);
    check_frame(app, &screen, test_id);
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
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
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

#[test]
fn x01_cli_route() {
    help_of("x01-cli-route", SHOWCASE, SHOWCASE_ABOUT, SHOWCASE_UI);
}

#[test]
fn x01_cli_cli() {
    help_of(
        "x01-cli-help-showcase",
        SHOWCASE,
        SHOWCASE_ABOUT,
        SHOWCASE_UI,
    );
    help_of(
        "x01-cli-help-tablepro",
        TABLEPRO,
        TABLEPRO_ABOUT,
        TABLEPRO_UI,
    );
    help_of("x01-cli-help-holla", HOLLA, HOLLA_ABOUT, HOLLA_UI);

    rejected(
        "x01-cli-page-short",
        SHOWCASE,
        &["-p", "not-a-page"],
        120,
        40,
        "unknown page \"not-a-page\"",
        SHOWCASE_UI,
    );
    rejected(
        "x01-cli-page-long",
        SHOWCASE,
        &["--page", "not-a-page"],
        120,
        40,
        "unknown page \"not-a-page\"",
        SHOWCASE_UI,
    );
    rejected(
        "x01-cli-holla-scenario",
        HOLLA,
        &["--scenario", "not-a-scenario"],
        120,
        40,
        "unknown scenario \"not-a-scenario\"",
        HOLLA_UI,
    );
    rejected(
        "x01-cli-tablepro-connect",
        TABLEPRO,
        &["--connect", "not-a-connection"],
        120,
        40,
        "no connection named \"not-a-connection\"",
        TABLEPRO_UI,
    );
    rejected(
        "x01-cli-opt-missing-eq",
        SHOWCASE,
        &["--opt", "novalue"],
        120,
        120,
        "expected KEY=VALUE, got \"novalue\"",
        SHOWCASE_UI,
    );
    rejected(
        "x01-cli-opt-unknown-key",
        SHOWCASE,
        &["--opt", "not-a-key=value"],
        120,
        120,
        "unknown option \"not-a-key\"",
        SHOWCASE_UI,
    );
    rejected(
        "x01-cli-opt-bad-value",
        SHOWCASE,
        &["--opt", "shell.motion=sideways"],
        120,
        40,
        "invalid value \"sideways\" for shell.motion",
        SHOWCASE_UI,
    );

    let repeat = open(
        "x01-cli-opt-repeat",
        SHOWCASE,
        &[
            "--page",
            "chrome",
            "--opt",
            "chrome.brand=none",
            "--opt",
            "chrome.brand=compact",
        ],
        120,
        40,
    );
    let chosen = repeat.wait_screen("later opt", |screen| {
        let visible = text(screen);
        visible.contains(LATER) && !visible.contains(EARLIER)
    });
    require(&chosen, LATER);
    refuse(&chosen, EARLIER);
    repeat.finish();

    let wide = open("x01-cli-help-wide", SHOWCASE, &["--help"], 120, 40);
    shot(&wide, WIDE_ID, 120, 40);

    let narrow = open("x01-cli-help-narrow", SHOWCASE, &["--help"], 80, 24);
    shot(&narrow, NARROW_ID, 80, 24);
}
