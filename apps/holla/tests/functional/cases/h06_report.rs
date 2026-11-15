//! Holla cleanup report, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h06_report/` and each checkpoint's dots become underscores.
//! `parity-cleanup-results` is `parity-delete-safety` plus extras. Catalog
//! item `cleanup.review-all` is labeled `Review cleanup candidates` and
//! launches `Launch::Cleanup { category: None }`. The plan root is the home
//! directory, so the gate phrase is `TRASH {n} UNDER /Users/alex ON mbp`.
//! `{n}` is read from gate 1. Trash mode adds bytes only to the amount that
//! left the tree; `freed_now` stays `0 B` (`cleanup.rs` adds to it only after
//! a permanent remove). `~/Projects/safe/locked` is an unreadable folder on
//! the checklist, not a selectable row, so this journey does not put the
//! size-traversal failure on the report.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const LABEL: &str = "Review cleanup candidates";
const HOME: &str = "~/Projects/safe";
const CRUMB: &str = "Here › Cleanup";
const REVIEW: &str = "Cleanup review";
const LIVE: &str = "mode Trash (recoverable until emptied) · live · m toggles the mode";
const SELECTED: &str = "Every eligible candidate selected · 2.1 GiB";
const COUNT: &str = "6 selected (2.1 GiB)";
const GATE1: &str = "gate 1 of 2";
const CONTINUE: &str = "Continue…";
const CANCEL: &str = "Cancel";
const MISMATCH: &str = "Does not match";
const RUNNING: &str = "Cleanup running";
const REPORT: &str = "Cleanup report";
const STARTED: &str = "cleanup started · 6 items · quitting waits for it";
const PARTIAL_OF: &str = "of 6 items";
const PARTIAL_WAIT: &str = "quitting waits for the rest";
const FREED: &str = "left the tree · 0 B freed now · the rest returns when the Trash is emptied";
const BYTES: &str =
    "2.1 GiB left the tree · 0 B freed now · the rest returns when the Trash is emptied";
const SUMMARY: &str = "6 trashed";
const LOG: &str = "~/.cache/holla/ops.log";
const LOG_OK: &str = "every record written";
const MODE: &str = "trash";
const QUIT: &str = "Quit holla";
const HOST: &str = "mbp";
const WIDE: &str = "H06.report.results.120x40";
const NARROW: &str = "H06.report.results.narrow";

const PATHS: &[(&str, &str)] = &[
    ("~/Library/Caches/com.apple.dt.Xcode", "900.0 MiB"),
    ("~/Library/Caches/Google", "100.0 MiB"),
    ("~/Library/Caches/com.app", "100.0 MiB"),
    ("~/Library/Logs/DiagnosticReports", "44.0 KiB"),
    ("~/Projects/safe/target", "1000.0 MiB"),
    ("ünï", "1.0 MiB"),
];

const FOREIGN: &[&str] = &[
    "vm.img",
    "sparse.img",
    "~/work/android",
    "build.gradle.kts",
    "workspace.xml",
    "842 of 926",
];

const NOT_ON_REPORT: &[&str] = &[
    "size traversal failed",
    "unreadable entry",
    "~/work/legacy/node_modules",
    "would_remove",
    "skipped",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h06_report/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    launch.platform = "macos".to_string();
    launch.args = vec![
        "--scenario".to_string(),
        "parity-cleanup-results".to_string(),
    ];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions");
    app
}

fn flat(screen: &TerminalSnapshot) -> String {
    text(screen)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn absent_foreign(screen: &TerminalSnapshot) {
    see(screen, &[], FOREIGN);
}

fn focused_button(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).contains(&format!("▎{label}"))
}

fn trash_count(screen: &TerminalSnapshot) -> usize {
    let flat = flat(screen);
    let parts: Vec<&str> = flat.split(' ').collect();
    for window in parts.windows(3) {
        if window[0] == "trash"
            && window[2].starts_with("item")
            && let Ok(count) = window[1].parse::<usize>()
        {
            assert!(
                flat.contains(&format!("trash {count} item")),
                "count {count} is not the painted phrase\n{flat}"
            );
            return count;
        }
    }
    panic!("no trash count on gate 1\n{flat}");
}

fn phrase(count: usize) -> String {
    format!("TRASH {count} UNDER /Users/alex ON {HOST}")
}

/// The status bar paints `· {cols}×{rows}` from the frame Holla just drew.
/// The PTY grid changes first, and the previous frame still matches `ready`.
fn painted(screen: &TerminalSnapshot, cols: u16, rows: u16) -> bool {
    text(screen).contains(&format!("· {cols}×{rows}"))
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && painted(screen, nudged, rows)
            && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && painted(screen, cols, rows)
            && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn report_ready(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(REPORT)
        && !visible.contains(RUNNING)
        && visible.contains(BYTES)
        && visible.contains(SUMMARY)
        && visible.contains(LOG_OK)
        && !visible.contains(QUIT)
        && FOREIGN.iter().all(|item| !visible.contains(item))
}

fn narrow_report(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(REPORT)
        && !visible.contains(RUNNING)
        && !visible.contains(QUIT)
        && FOREIGN.iter().all(|item| !visible.contains(item))
}

fn assert_freed_stays_zero(screen: &TerminalSnapshot) {
    let visible = flat(screen);
    assert!(visible.contains(FREED), "freed-now line changed\n{visible}");
    for line in text(screen).lines() {
        if !(line.contains("failed") || line.contains("skipped")) {
            continue;
        }
        assert!(
            !line.contains("KiB") && !line.contains("MiB") && !line.contains("GiB"),
            "failed or skipped row contributed bytes\n{line}"
        );
    }
}

fn open_cleanup(app: &App) -> TerminalSnapshot {
    app.paste(LABEL);
    app.wait_text(&format!("▎ {LABEL}"));
    app.key("enter");
    app.wait_screen("cleanup checklist", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB)
            && visible.contains(REVIEW)
            && visible.contains(LIVE)
            && visible.contains("1 selected (44.0 KiB)")
            && !visible.contains(QUIT)
    })
}

fn select_eligible(app: &App) {
    app.key("a");
    app.wait_screen("every eligible selected", |screen| {
        let visible = text(screen);
        visible.contains(SELECTED) && visible.contains(COUNT) && visible.contains(LIVE)
    });
}

/// Gate 1 focuses Cancel. A wrong phrase paints `Does not match` and does not start.
fn confirm_cleanup(app: &App) -> usize {
    app.key("d");
    let gate1 = app.wait_screen("gate 1", |screen| {
        let visible = text(screen);
        visible.contains(GATE1)
            && visible.contains(CONTINUE)
            && visible.contains(CANCEL)
            && focused_button(screen, CANCEL)
            && !visible.contains("gate 2 of 2")
    });
    see(
        &gate1,
        &[GATE1, CONTINUE, CANCEL, HOME],
        &[RUNNING, REPORT, QUIT],
    );
    let count = trash_count(&gate1);
    assert_eq!(count, 6, "checklist selection was not six items");
    let phrase = phrase(count);

    app.key("right");
    app.wait_screen("continue focused", |screen| {
        focused_button(screen, CONTINUE)
    });
    app.key("enter");
    let gate2 = app.wait_screen("gate 2", |screen| {
        let visible = flat(screen);
        visible.contains("gate 2 of 2")
            && visible.contains(&phrase)
            && visible.contains(&format!("Type {phrase} to confirm"))
            && !visible.contains(RUNNING)
    });
    see(
        &gate2,
        &["gate 2 of 2", "Execute"],
        &[RUNNING, REPORT, "cleanup started"],
    );

    app.type_text("TRASH 0");
    app.key("enter");
    let rejected = app.wait_text(MISMATCH);
    see(
        &rejected,
        &[MISMATCH, "gate 2 of 2", "TRASH 0"],
        &[RUNNING, REPORT, "cleanup started", QUIT],
    );

    app.key("ctrl+l");
    app.key("backspace");
    app.wait_screen("ack cleared", |screen| !text(screen).contains("TRASH 0"));
    app.type_text(&phrase);
    app.wait_text(&phrase);
    app.key("enter");
    count
}

fn assert_partial(screen: &TerminalSnapshot, count: usize) {
    let visible = flat(screen);
    assert!(visible.contains(RUNNING), "not running\n{visible}");
    assert!(
        !visible.contains(REPORT),
        "report replaced the partial\n{visible}"
    );
    assert!(
        visible.contains(&format!(
            "cleanup started · {count} items · quitting waits for it"
        )),
        "start status missing\n{visible}"
    );
    assert!(
        visible.contains("running ·"),
        "outcome is not partial\n{visible}"
    );
    assert!(
        visible.contains(&format!("of {count} items")),
        "partial total missing\n{visible}"
    );
    assert!(
        visible.contains(PARTIAL_WAIT),
        "quit wait missing\n{visible}"
    );
    assert_freed_stays_zero(screen);
    absent_foreign(screen);
}

fn assert_report(screen: &TerminalSnapshot) {
    let visible = flat(screen);
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    see(
        screen,
        &[
            REPORT, SUMMARY, LOG, LOG_OK, MODE, "Mode", "Bytes", "Outcome",
        ],
        &[RUNNING, QUIT, GATE1],
    );
    assert!(visible.contains(BYTES), "bytes line\n{visible}");
    assert!(
        visible.contains("0 B freed now"),
        "freed now was not 0 B\n{visible}"
    );
    assert_freed_stays_zero(screen);
    for (path, size) in PATHS {
        assert!(visible.contains(path), "missing path {path}\n{visible}");
        assert!(
            text(screen).contains(size),
            "missing size {size} for {path}\n{}",
            text(screen)
        );
    }
    assert!(
        visible.contains("trashed"),
        "no successful trash outcome\n{visible}"
    );
    see(screen, &[], NOT_ON_REPORT);
    absent_foreign(screen);
    if visible.contains("big/blob") {
        assert!(
            visible.contains("~/Projects/safe/big/blob") || visible.contains("big/blob"),
            "blob marker without the fixture path\n{visible}"
        );
    }
}

fn reach_report(app: &App) -> TerminalSnapshot {
    let page = open_cleanup(app);
    see(
        &page,
        &[CRUMB, REVIEW, LIVE, HOME, HOST],
        &[REPORT, RUNNING, QUIT],
    );
    absent_foreign(&page);
    select_eligible(app);
    let count = confirm_cleanup(app);
    let running = app.wait_screen("cleanup running", |screen| {
        let visible = flat(screen);
        visible.contains(RUNNING)
            && visible.contains(PARTIAL_OF)
            && visible.contains(PARTIAL_WAIT)
            && visible.contains(STARTED)
    });
    assert_partial(&running, count);
    let report = app.wait_screen("cleanup report", |screen| {
        let visible = flat(screen);
        visible.contains(REPORT)
            && !visible.contains(RUNNING)
            && visible.contains(BYTES)
            && visible.contains(SUMMARY)
            && visible.contains(LOG_OK)
    });
    assert_report(&report);
    report
}

#[test]
fn h06_report_route_returns_from_the_report() {
    let app = launch("h06-report-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[HOME, HOST], &[REPORT, REVIEW, QUIT]);
    absent_foreign(&landing);

    let report = reach_report(&app);
    assert_eq!((report.cols(), report.rows()), (120, 40));
    see(&report, &[REPORT, SUMMARY, BYTES], &[RUNNING, QUIT]);

    app.key("escape");
    let finder = app.wait_screen("left the report", |screen| {
        let visible = text(screen);
        visible.contains(LABEL)
            && visible.contains(HOME)
            && !visible.contains(REPORT)
            && !visible.contains(RUNNING)
            && !visible.contains(REVIEW)
            && !visible.contains(QUIT)
    });
    see(
        &finder,
        &[LABEL, HOME, HOST],
        &[REPORT, RUNNING, REVIEW, QUIT],
    );
    absent_foreign(&finder);
    app.finish();
}

#[test]
fn h06_report_results_match_the_simulation() {
    let app = launch("h06-report-results");
    reach_report(&app);
    nudge(&app, report_ready);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("narrow report", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && painted(screen, 80, 24)
            && narrow_report(screen)
    });
    see(&narrowed, &[REPORT], &[RUNNING, QUIT]);
    nudge(&app, narrow_report);
    shot(&app, NARROW);

    app.resize(120, 40);
    let restored = app.wait_screen("report restored", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && painted(screen, 120, 40)
            && report_ready(screen)
    });
    see(
        &restored,
        &[REPORT, SUMMARY, BYTES, LOG_OK],
        &[RUNNING, QUIT],
    );
    assert_freed_stays_zero(&restored);
    absent_foreign(&restored);
    app.finish();
}

#[test]
fn h06_report_scenario_parity_cleanup_results() {
    let app = launch("h06-report-scenario");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[HOME, HOST, "safe · Rust project"],
        &[REPORT, REVIEW, QUIT],
    );
    absent_foreign(&landing);
    let landing_text = text(&landing);
    if landing_text.contains("blob") {
        assert!(
            landing_text.contains("big"),
            "blob painted without its directory\n{landing_text}"
        );
    }

    let report = reach_report(&app);
    see(
        &report,
        &[REPORT, HOME, SUMMARY, BYTES, LOG_OK],
        &[RUNNING, QUIT],
    );
    absent_foreign(&report);
    see(&report, &[], &["~/work/legacy/node_modules"]);
    app.finish();
}
