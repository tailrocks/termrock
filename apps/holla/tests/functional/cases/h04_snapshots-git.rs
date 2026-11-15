//! Holla git snapshots, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h04_snapshots_git/` and each checkpoint's dots become underscores.
//! The shot is the open `git` snapshot. Reduced motion keeps that page quiet.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const HOLLA_ROOT: &str = "~/work/holla";
const HOLLA_FILE: &str = " M src/bin/holla/app.rs";
const HOLLA_STATUS: &str =
    "$ git -C /Users/alex/work/holla status --porcelain=v2 --branch --show-stash";
const ENGINE_STATUS: &str =
    "$ git -C /Users/alex/work/engine status --porcelain=v2 --branch --show-stash";
const DIFF_STAT: &str = "$ git diff --stat";
const STASH_ENTRY: &str = "stash@{";
const WIDE: &str = "H04.snapshots-git.git.120x40";
const NARROW: &str = "H04.snapshots-git.git.narrow";
/// Title of the `git` and `git-stash` snapshots (`snapshot.rs`).
const ANCHOR: &str = "Git status";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h04_snapshots_git/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions and resources");
    app
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

fn row_containing(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no row {needle:?}\n{}", text(screen)))
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. The waits follow the reported size, not the size text.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        text(&screen).contains(ANCHOR),
        "shot left git status\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(ANCHOR)
    });
    app.resize(cols, rows);
    app.wait_screen("restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(ANCHOR)
    });
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
}

fn open_row(app: &App, query: &str, preview: &str) {
    app.paste(query);
    // A suggested row can already show `preview` before the paste is painted.
    app.wait_screen(&format!("listed {query}"), |screen| {
        let visible = text(screen);
        visible.contains(query)
            && visible.contains(preview)
            && !visible.contains("Search actions and resources")
    });
    app.key("enter");
}

fn clear_query(app: &App) {
    app.key("esc");
    app.wait_text("Search actions and resources");
}

fn leave(app: &App) {
    app.key("ctrl+q");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if !app.child_running() {
            app.expect_exited(0);
            return;
        }
        let visible = text(&app.screen());
        if visible.contains("Quit holla") {
            let quit_focused = visible.contains("▎Quit") || visible.contains("Enter Quit");
            let stop_focused = visible.contains("▎Stop and quit") || visible.contains("Enter Stop");
            assert!(
                quit_focused || stop_focused,
                "exit button is not focused\n{visible}"
            );
            app.key("enter");
            let exit_by = Instant::now() + Duration::from_secs(8);
            while app.child_running() {
                assert!(
                    Instant::now() < exit_by,
                    "confirmed quit did not exit\n{}",
                    text(&app.screen())
                );
                std::thread::sleep(Duration::from_millis(25));
            }
            app.expect_exited(0);
            return;
        }
        assert!(Instant::now() < deadline, "quit did not exit\n{visible}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn assert_git_status(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "{}",
        text(screen)
    );
    // `reveal::cut` keeps the status command whole at 120 and clips it at 80.
    let command = if cols >= 100 {
        HOLLA_STATUS
    } else {
        "$ git -C /Users/alex/work/holla status --porcelain=v2 --branch --show-st"
    };
    see(
        screen,
        &[
            ANCHOR,
            "holla",
            HOLLA_ROOT,
            "origin/main · 3 behind",
            "4 unstaged",
            "1 untracked",
            "1 stashed",
            command,
            HOLLA_FILE,
            "Pull",
            "Open lazygit",
        ],
        &[
            "index file corrupt",
            "~/work/svc",
            "~/work/repos",
            DIFF_STAT,
            STASH_ENTRY,
        ],
    );
}

#[test]
fn h04_snapshots_git_route() {
    let app = open("h04-git-route", "rust-dirty");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[HOLLA_ROOT, "holla"], &[HOLLA_FILE, DIFF_STAT]);
    open_row(&app, "Show git status", "full git status");
    let page = app.wait_text(HOLLA_STATUS);
    assert_git_status(&page, 120, 40);
    app.key("esc");
    let finder = app.wait_text("full git status");
    see(
        &finder,
        &["Show git status", "full git status", HOLLA_ROOT],
        &[HOLLA_FILE, "--porcelain=v2", DIFF_STAT],
    );
    leave(&app);
}

#[test]
fn h04_snapshots_git_git() {
    let app = open("h04-git-status", "rust-dirty");
    open_row(&app, "Show git status", "full git status");
    let wide = app.wait_screen("git status", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(HOLLA_STATUS)
    });
    assert_git_status(&wide, 120, 40);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("git status narrow", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(ANCHOR)
            && visible.contains(HOLLA_FILE)
    });
    assert_git_status(&narrow, 80, 24);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("git status restored", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(HOLLA_STATUS)
            && visible.contains("Pull")
    });
    assert_git_status(&restored, 120, 40);

    app.key("esc");
    app.wait_text("full git status");
    clear_query(&app);
    open_row(&app, "Review 5 modified files", "diff stat");
    // The finder preview already says Modified files and clips the git
    // command. The snapshot page is the frame that keeps the command whole.
    let diff = app.wait_screen("modified files", |screen| {
        let visible = text(screen);
        visible.contains("Modified files")
            && visible.contains(HOLLA_STATUS)
            && visible.contains(DIFF_STAT)
    });
    see(
        &diff,
        &[
            "Modified files",
            HOLLA_ROOT,
            HOLLA_STATUS,
            "--show-stash",
            DIFF_STAT,
            "src/bin/holla/app.rs",
            "files changed",
            "Pull",
            "Open lazygit",
        ],
        &[
            STASH_ENTRY,
            "index file corrupt",
            "~/work/repos",
            "~/work/svc",
        ],
    );
    app.key("esc");
    app.wait_text("Review 5 modified files");
    clear_query(&app);
    open_row(&app, "Show stash (1)", "list stashed changes");
    let stash = app.wait_screen("stash", |screen| {
        let visible = text(screen);
        visible.contains("1 stashed") && visible.contains(HOLLA_STATUS) && visible.contains(ANCHOR)
    });
    see(
        &stash,
        &[
            ANCHOR,
            HOLLA_ROOT,
            "1 stashed",
            HOLLA_STATUS,
            HOLLA_FILE,
            "Pull",
            "Open lazygit",
        ],
        &[DIFF_STAT, STASH_ENTRY, "Modified files"],
    );
    leave(&app);

    let app = open("h04-git-all", "parity-git-batch");
    open_row(
        &app,
        "Status across 5 child projects",
        "inspect every discovered",
    );
    let all = app.wait_screen("git-all", |screen| {
        let visible = text(screen);
        visible.contains("Status across child projects")
            && visible.contains("Pull every child project")
    });
    assert_eq!((all.cols(), all.rows()), (120, 40));
    see(
        &all,
        &[
            "Status across child projects",
            "~/work/repos",
            "Pull every child project",
            "Switch every project to its primary branch",
        ],
        &["src/bin/holla/app.rs", HOLLA_ROOT, DIFF_STAT],
    );
    let alpha = row_containing(&all, "↓3");
    assert!(
        alpha.contains("alpha") && !alpha.contains("index file corrupt"),
        "alpha row {alpha:?}"
    );
    let delta = row_containing(&all, "delta");
    // `git-all` prints summary() and block_reason(). Neither includes
    // command_failure, so delta's "index file corrupt" is not a row cell.
    assert!(
        delta.contains("main") && !delta.contains("index file corrupt"),
        "delta row {delta:?}\n{}",
        text(&all)
    );
    assert!(
        !text(&all).contains("index file corrupt"),
        "git-all drew the child failure\n{}",
        text(&all)
    );
    leave(&app);

    let app = open("h04-git-clean", "parity-cargo");
    open_row(&app, "Show git status", "full git status");
    let clean = app.wait_text(ENGINE_STATUS);
    see(
        &clean,
        &[
            ANCHOR,
            "~/work/engine",
            ENGINE_STATUS,
            "0 unstaged",
            "0 stashed",
            "Open lazygit",
        ],
        &[" M ", DIFF_STAT, STASH_ENTRY, HOLLA_FILE, "~/work/repos"],
    );
    leave(&app);
}
