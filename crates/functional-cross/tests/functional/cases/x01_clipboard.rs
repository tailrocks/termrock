//! Cross-application clipboard copy and Ctrl+C quit, before the refactor.
//!
//! Tuiscotti test ids reject `.` and uppercase, so the baseline directory is
//! `pre_refactor/x01_clipboard/`. The checkpoint session copies the read-only
//! report, captures 120×40, resizes to 80×24, captures that frame, then
//! resizes back. `Launch::new` leaves the clock off, including for jackin.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{
    App, HOLLA, Launch, Point, SHOWCASE, TABLEPRO, find_all, row_text, text, try_launch,
};

const REPORT: &str = "Copied the report (13 lines)";
const FOLDER: &str = "Copy this folder's path";
const FOLDER_OK: &str =
    "Copied to the terminal clipboard (OSC 52, flushed) · /Users/alex/work/notes";
const OSC_OFF: &str =
    "Not copied · this terminal does not accept OSC 52 · no system clipboard is wired";
const LONG_PATH: &str = "/Users/alex/Library/Mobile Documents/com~apple~CloudDocs/cloud-only.txt";
const TOO_LARGE: &str = "Not copied · 71 bytes exceed the terminal's 64 byte OSC 52 limit";
const QUERY: &str = "Copied query";
const WIDE_ID: &str = "pre_refactor/x01_clipboard/x01_clipboard_copy_120x40";
const NARROW_ID: &str = "pre_refactor/x01_clipboard/x01_clipboard_copy_narrow";

fn open(name: &str, bin: &str, args: &[&str]) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = args.iter().copied().map(str::to_string).collect();
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

fn click_one(app: &App, label: &str, keep: impl Fn(&TerminalSnapshot, Point) -> bool) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| keep(&screen, *hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn ctrl_c_exits(app: &App) {
    app.key("ctrl+c");
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.child_running() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
    }
    if app.child_running() {
        panic!("ctrl+c left the process running\n{}", text(&app.screen()));
    }
    app.expect_exited(0);
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
    let store =
        tuiscotti_latest::check::FrozenStore::new(&root, &artifacts).expect("open frozen store");
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

fn frame_with(app: &App, needle: &str, cols: u32, rows: u32) -> TerminalSnapshot {
    thread::sleep(Duration::from_millis(250));
    app.wait_screen(needle, |screen| {
        text(screen).contains(needle) && screen.cols() == cols && screen.rows() == rows
    })
}

fn showcase_quit() {
    let app = open(
        "x01-clipboard-showcase-quit",
        SHOWCASE,
        &["--page", "readonlyreport"],
    );
    app.wait_text("Copy report");
    assert!(app.child_running(), "showcase stopped before ctrl+c");
    ctrl_c_exits(&app);
}

fn showcase_grid_copy() {
    let app = open(
        "x01-clipboard-showcase-grid",
        SHOWCASE,
        &["--page", "datagrid"],
    );
    app.wait_text("1001");
    click_one(&app, "Northwind Traders", |screen, hit| {
        let row = row_text(screen, u32::from(hit.y));
        hit.y > 4 && hit.x >= 24 && row.contains("1001") && !row.contains("Traders 2")
    });
    app.key("shift+right");
    app.key("ctrl+c");
    let copied = app.wait_screen("grid copy stays", |screen| {
        let visible = text(screen);
        visible.contains("Copied ") && visible.contains(" chars")
    });
    assert!(app.child_running(), "grid ctrl+c quit");
    require(&copied, "Northwind");
    refuse(&copied, "Not copied");
    app.kill_child();
}

fn tablepro_quit() {
    let app = open("x01-clipboard-tablepro-quit", TABLEPRO, &[]);
    app.wait_text("Filter connections");
    ctrl_c_exits(&app);
}

fn tablepro_copy() {
    let app = open("x01-clipboard-tablepro-copy", TABLEPRO, &[]);
    app.wait_text("Filter connections");
    click_one(&app, "Development", |_, hit| hit.x < 40);
    app.key("enter");
    app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains("Connected to Development") && !visible.contains("Loading schema")
    });
    app.key("ctrl+y");
    app.wait_text("Query history");
    app.key("c");
    app.wait_text("scope: all connections");
    app.key("y");
    let copied = app.wait_text(QUERY);
    assert!(app.child_running(), "tablepro copy stopped the process");
    require(&copied, QUERY);
    refuse(&copied, "Not copied");
    app.kill_child();
}

fn holla_quit() {
    let app = open(
        "x01-clipboard-holla-quit",
        HOLLA,
        &["--scenario", "parity-files"],
    );
    app.wait_text("Search actions");
    ctrl_c_exits(&app);
}

fn finder_copy(app: &App) -> TerminalSnapshot {
    app.wait_text("Search actions");
    app.paste(FOLDER);
    app.wait_text(&format!("▎ {FOLDER}"));
    app.key("enter");
    app.wait_screen("folder copy", |screen| {
        let visible = text(screen);
        visible.contains("Copied to the terminal clipboard") || visible.contains("Not copied")
    })
}

fn holla_supported() {
    let app = open(
        "x01-clipboard-holla-on",
        HOLLA,
        &["--scenario", "parity-files"],
    );
    let copied = finder_copy(&app);
    assert!(app.child_running(), "supported copy stopped holla");
    require(&copied, FOLDER_OK);
    refuse(&copied, "Not copied");
    app.kill_child();
}

fn holla_unsupported() {
    let app = open(
        "x01-clipboard-holla-off",
        HOLLA,
        &["--scenario", "parity-platforms-linux"],
    );
    let copied = finder_copy(&app);
    assert!(app.child_running(), "unsupported copy stopped holla");
    require(&copied, OSC_OFF);
    refuse(&copied, "Copied ");
    app.kill_child();
}

fn holla_too_large() {
    let app = open(
        "x01-clipboard-holla-limit",
        HOLLA,
        &["--scenario", "parity-files"],
    );
    app.wait_text("Search actions");
    app.paste("Browse ~/work/notes");
    app.wait_text("▎ Browse ~/work/notes");
    app.key("enter");
    app.wait_text("todo.md");
    app.key("g");
    app.wait_text("Go to path");
    app.paste(LONG_PATH);
    app.wait_text("cloud-only.txt");
    app.key("enter");
    app.wait_text("Jumped to");
    app.key("y");
    let copied = app.wait_text("71 bytes exceed the terminal's 64 byte OSC 52 limit");
    assert!(app.child_running(), "oversized copy stopped holla");
    require(&copied, TOO_LARGE);
    refuse(&copied, "Copied ");
    app.kill_child();
}

fn copy_report(app: &App) {
    app.wait_text("Copy report");
    click_one(app, "Copy report", |screen, hit| {
        hit.y > 4 && row_text(screen, u32::from(hit.y)).contains("Copy report")
    });
    app.wait_text(REPORT);
}

fn checkpoints() {
    let app = open(
        "x01-clipboard-checkpoint",
        SHOWCASE,
        &["--page", "readonlyreport"],
    );
    copy_report(&app);
    let wide = frame_with(&app, REPORT, 120, 40);
    assert!(app.child_running(), "copy stopped showcase");
    require(&wide, REPORT);
    require(&wide, "Copy report");
    refuse(&wide, "Not copied");
    check_frame(&app, &wide, WIDE_ID);

    app.resize(80, 24);
    let narrow = frame_with(&app, REPORT, 80, 24);
    require(&narrow, REPORT);
    refuse(&narrow, "Not copied");
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    check_frame(&app, &narrow, NARROW_ID);

    app.resize(120, 40);
    let restored = frame_with(&app, REPORT, 120, 40);
    require(&restored, REPORT);
    assert!(app.child_running(), "resize stopped showcase");
    app.finish();
}

#[test]
fn x01_clipboard_route() {
    let app = open(
        "x01-clipboard-route",
        SHOWCASE,
        &["--page", "readonlyreport"],
    );
    copy_report(&app);
    let screen = app.screen();
    require(&screen, REPORT);
    require(&screen, "Read-only report");
    assert!(app.child_running(), "route copy stopped showcase");
    app.finish();
}

#[test]
fn x01_clipboard_copy() {
    showcase_quit();
    showcase_grid_copy();
    tablepro_quit();
    tablepro_copy();
    holla_quit();
    holla_supported();
    holla_unsupported();
    holla_too_large();
    checkpoints();
}
