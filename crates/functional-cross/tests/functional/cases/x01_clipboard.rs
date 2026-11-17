//! Cross-application clipboard copy and Ctrl+C quit, before the refactor.
//!
//! Tuiscotti test ids reject `.` and uppercase, so the baseline directory is
//! `pre_refactor/x01_clipboard/`. The checkpoint session copies the read-only
//! report, captures 120×40, resizes to 80×24, captures that frame, then
//! resizes back. `Launch::new` leaves the clock off, including for jackin.

use std::thread;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Point, check_stored, find_all, holla_bin, open, refuse, require, row_text, showcase_bin,
    tablepro_bin, text,
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

fn frame_with(app: &App, needle: &str, cols: u32, rows: u32) -> TerminalSnapshot {
    app.wait_screen(needle, |screen| {
        text(screen).contains(needle) && screen.cols() == cols && screen.rows() == rows
    })
}

fn showcase_quit() {
    let app = open(
        "x01-clipboard-showcase-quit",
        showcase_bin(),
        &["--page", "readonlyreport"],
    );
    app.wait_text("Copy report");
    assert!(app.child_running(), "showcase stopped before ctrl+c");
    ctrl_c_exits(&app);
}

fn showcase_grid_copy() {
    let app = open(
        "x01-clipboard-showcase-grid",
        showcase_bin(),
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
    let app = open("x01-clipboard-tablepro-quit", tablepro_bin(), &[]);
    app.wait_text("Filter connections");
    ctrl_c_exits(&app);
}

fn tablepro_copy() {
    let app = open("x01-clipboard-tablepro-copy", tablepro_bin(), &[]);
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
        holla_bin(),
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
        holla_bin(),
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
        holla_bin(),
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
        holla_bin(),
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
    // The stored frame is the one after the click's press flash has expired
    // and been redrawn, so the cursor it carries is the settled one.
    app.flash_expired();
    app.settle();
}

fn checkpoints() {
    let app = open(
        "x01-clipboard-checkpoint",
        showcase_bin(),
        &["--page", "readonlyreport"],
    );
    copy_report(&app);
    let wide = frame_with(&app, REPORT, 120, 40);
    assert!(app.child_running(), "copy stopped showcase");
    require(&wide, REPORT);
    require(&wide, "Copy report");
    refuse(&wide, "Not copied");
    check_stored(&app, &wide, WIDE_ID);

    app.resize(80, 24);
    let narrow = frame_with(&app, REPORT, 80, 24);
    require(&narrow, REPORT);
    refuse(&narrow, "Not copied");
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    check_stored(&app, &narrow, NARROW_ID);

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
        showcase_bin(),
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
