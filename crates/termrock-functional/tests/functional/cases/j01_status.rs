//! Jackin instance status, driven through the real binary.
//!
//! `show_meta` needs a row at least 44 columns wide. The 120-column split
//! leaves the tree narrower than that, so the status is the detail Lifecycle
//! line. At 80 columns the detail collapses and that heading is absent; the
//! tree row paints the meta. `populated(false)` does not create CleanExited.
//! Superseded and Purged stay out of the list.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const WIDE: &str = "pre_refactor/j01_status/j01_status_status_120x40";
const NARROW: &str = "pre_refactor/j01_status/j01_status_status_narrow";

const LIVE: &str = "Container and Capsule daemon are live.";
const DIRTY: &str = "uncommitted changes in the";
const UNPUSHED: &str = "commits that were never";
const RESTORE: &str = "Stopped cleanly";
const FAILED: &str = "Launch failed before the Capsule was ready";
const CRASHED: &str = "stopped unexpectedly";
const HIDDEN: &[&str] = &["77aa", "88bb", "superseded", "purged", "exited clean"];

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn forbid_hidden(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for item in HIDDEN {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
    forbid_hidden(screen);
}

fn dismiss_intro(app: &App) {
    app.leave_intro("manager after intro", |screen| {
        text(screen).contains("Current directory")
    });
}

fn quiet_c41e(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    line.contains('▎')
        && line.contains("c41e  ")
        && visible.contains("preserved · dirty")
        && visible.contains("reviewer")
        && visible.contains("payments-platform")
        && visible.contains("Current directory")
        && HIDDEN.iter().all(|item| !visible.contains(item))
        && !visible.contains("Stand up")
        && !visible.contains(LIVE)
        && !visible.contains("preserved · unpushed")
        && !visible.contains("restore available")
        && !visible.contains("failed setup")
        && !visible.contains(CRASHED)
}

fn wide_c41e(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    quiet_c41e(screen)
        && screen.cols() == 120
        && screen.rows() == 40
        && visible.contains("Lifecycle")
        && visible.contains("c41e · payments-platform")
        && visible.contains(DIRTY)
        && !focused_line(screen).contains("preserved · dirty")
}

/// Collapsed summary has no Lifecycle heading. The tree is full width, so
/// the focused row itself carries the meta.
fn narrow_c41e(screen: &TerminalSnapshot) -> bool {
    quiet_c41e(screen)
        && focused_line(screen).contains("preserved · dirty")
        && !text(screen).contains("Lifecycle")
}

fn nudge(app: &App, quiet: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet, Some("nudge left the frame"));
}

fn down_focused(app: &App, needle: &str) -> TerminalSnapshot {
    app.key("down");
    app.wait_screen(needle, |screen| focused_line(screen).contains(needle))
}

fn expand_focused(app: &App, name: &str) {
    app.key("right");
    app.wait_screen(&format!("expand {name}"), |screen| {
        let line = focused_line(screen);
        line.contains('▾') && line.contains(name)
    });
}

fn select_instance(
    app: &App,
    id: &str,
    workspace: &str,
    fragment: &str,
    gone: &[&str],
) -> TerminalSnapshot {
    let title = format!("{id} · {workspace}");
    app.key("down");
    let screen = app.wait_screen(&title, |screen| {
        let visible = text(screen);
        visible.contains(&title)
            && visible.contains(fragment)
            && visible.contains("Lifecycle")
            && focused_line(screen).contains(id)
    });
    see(&screen, &[&title, fragment, "Lifecycle"], gone);
    screen
}

fn reach_c41e(app: &App) -> TerminalSnapshot {
    down_focused(app, "payments-platform");
    expand_focused(app, "payments-platform");
    let running = select_instance(
        app,
        "7f3a",
        "payments-platform",
        LIVE,
        &["c41e · payments-platform", DIRTY, "preserved · dirty"],
    );
    assert!(focused_line(&running).contains("7f3a"));
    select_instance(
        app,
        "c41e",
        "payments-platform",
        DIRTY,
        &["7f3a · payments-platform", LIVE],
    )
}

fn row_meta(app: &App, id: &str, meta: &str, absent: &[&str]) {
    let screen = down_focused(app, id);
    let line = focused_line(&screen);
    assert!(
        line.contains(meta),
        "row {id} missing {meta:?}\n{line}\n{}",
        text(&screen)
    );
    for item in absent {
        assert!(!line.contains(item), "row {id} shows {item:?}\n{line}");
    }
    forbid_hidden(&screen);
}

fn narrow_rows(app: &App) {
    app.resize(80, 24);
    app.wait_screen("row meta width", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    app.key("home");
    app.wait_screen("tree top", |screen| {
        focused_line(screen).contains("Current directory")
    });
    down_focused(app, "payments-platform");
    row_meta(
        app,
        "7f3a",
        "running",
        &[
            "preserved · dirty",
            "preserved · unpushed",
            "crashed",
            "failed setup",
            "restore available",
        ],
    );
    row_meta(
        app,
        "c41e",
        "preserved · dirty",
        &["running", "preserved · unpushed", "crashed", "failed setup"],
    );
    down_focused(app, "infra-control-plane");
    row_meta(
        app,
        "9b02",
        "running",
        &[
            "preserved · dirty",
            "preserved · unpushed",
            "crashed",
            "failed setup",
            "restore available",
        ],
    );
    row_meta(
        app,
        "04d7",
        "preserved · unpushed",
        &[
            "preserved · dirty",
            "running",
            "crashed",
            "restore available",
        ],
    );
    down_focused(app, "release-automation");
    row_meta(
        app,
        "a1c0",
        "restore available",
        &["failed setup", "preserved · dirty", "running", "crashed"],
    );
    row_meta(
        app,
        "5e5e",
        "failed setup",
        &[
            "restore available",
            "crashed",
            "running",
            "preserved · dirty",
        ],
    );
    down_focused(app, "customer-portal");
    row_meta(
        app,
        "12ee",
        "crashed · exit 137",
        &[
            "failed setup",
            "running",
            "restore available",
            "preserved · dirty",
        ],
    );
}

fn other_instances(app: &App) {
    down_focused(app, "infra-control-plane");
    expand_focused(app, "infra-control-plane");
    select_instance(
        app,
        "9b02",
        "infra-control-plane",
        LIVE,
        &[
            "7f3a · payments-platform",
            "c41e · payments-platform",
            DIRTY,
        ],
    );
    select_instance(
        app,
        "04d7",
        "infra-control-plane",
        UNPUSHED,
        &["9b02 · infra-control-plane", LIVE, DIRTY],
    );
    down_focused(app, "release-automation");
    expand_focused(app, "release-automation");
    select_instance(
        app,
        "a1c0",
        "release-automation",
        RESTORE,
        &["04d7 · infra-control-plane", UNPUSHED],
    );
    select_instance(
        app,
        "5e5e",
        "release-automation",
        FAILED,
        &["a1c0 · release-automation", RESTORE],
    );
    down_focused(app, "customer-portal");
    expand_focused(app, "customer-portal");
    select_instance(
        app,
        "12ee",
        "customer-portal",
        CRASHED,
        &["5e5e · release-automation", FAILED],
    );
}

fn first_use() {
    let app = open("j01-status-first", "first-use");
    dismiss_intro(&app);
    let empty = app.wait_screen("empty manager", |screen| {
        let visible = text(screen);
        visible.contains("no instances") && visible.contains("Current directory")
    });
    see(
        &empty,
        &[
            "no instances",
            "Current directory",
            "Create a workspace from this directory.",
        ],
        &["7f3a", "c41e", "preserved · dirty", "failed setup"],
    );
    app.finish();
}

fn returning(app: &App) {
    dismiss_intro(app);
    let manager = app.wait_screen("manager", |screen| {
        focused_line(screen).contains("Current directory")
    });
    see(
        &manager,
        &["Current directory", "2 running"],
        &["7f3a · payments-platform"],
    );
    let selected = reach_c41e(app);
    see(
        &selected,
        &[
            "c41e · payments-platform",
            "Lifecycle",
            "preserved · dirty",
            DIRTY,
            "kept until you decide.",
        ],
        &["7f3a · payments-platform", LIVE],
    );
    app.wait_screen("c41e quiet", wide_c41e);
    nudge(app, quiet_c41e);
    app.visual(WIDE);
    app.resize(80, 24);
    app.wait_screen("c41e narrow", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && narrow_c41e(screen)
    });
    nudge(app, narrow_c41e);
    app.visual(NARROW);
    app.resize(120, 40);
    app.wait_screen("c41e restored", wide_c41e);
    other_instances(app);
    narrow_rows(app);
    app.finish();
}

fn launch_running() {
    let app = open("j01-status-launch", "launch-running");
    let cockpit = app.wait_screen("cockpit", |screen| {
        let visible = text(screen);
        visible.contains("Loading the-architect") || visible.contains("Preparing launch")
    });
    assert!(
        text(&cockpit).contains("the-architect"),
        "launch did not open the cockpit\n{}",
        text(&cockpit)
    );
    app.key("ctrl+c");
    // The cockpit already lists b5df as a failed setup before the abort is
    // applied. The ack status is what shows the abort finished.
    let failed = app.wait_screen("aborted launch", |screen| {
        let visible = text(screen);
        visible.contains("b5df · payments-platform")
            && visible.contains(FAILED)
            && visible.contains("failed setup")
            && visible.contains("still running in the Construct")
            && focused_line(screen).contains("b5df")
            && !visible.contains("Cancel launch")
            && !visible.contains("Abort now")
            && !visible.contains("Resolving launch identity")
    });
    see(
        &failed,
        &[
            "b5df · payments-platform",
            "Lifecycle",
            "failed setup",
            FAILED,
            "Launch failed",
            "still running in the Construct",
            "2 running",
        ],
        &[
            "Loading the-architect",
            "preserved · dirty",
            DIRTY,
            "c41e · payments-platform",
        ],
    );
    app.key("up");
    let dirty = app.wait_screen("c41e after abort", |screen| {
        text(screen).contains("c41e · payments-platform") && text(screen).contains(DIRTY)
    });
    see(
        &dirty,
        &["c41e · payments-platform", "preserved · dirty", DIRTY],
        &["b5df · payments-platform", FAILED, "failed setup"],
    );
    app.key("up");
    let running = app.wait_screen("7f3a after abort", |screen| {
        text(screen).contains("7f3a · payments-platform") && text(screen).contains(LIVE)
    });
    see(
        &running,
        &["7f3a · payments-platform", LIVE],
        &["c41e · payments-platform", DIRTY, FAILED],
    );
    app.finish();
}

#[test]
fn j01_status_route() {
    let app = open("j01-status-route", "returning");
    dismiss_intro(&app);
    let manager = app.wait_screen("manager", |screen| {
        focused_line(screen).contains("Current directory") && text(screen).contains("2 running")
    });
    forbid_hidden(&manager);
    let selected = reach_c41e(&app);
    see(
        &selected,
        &[
            "c41e · payments-platform",
            "Lifecycle",
            "preserved · dirty",
            DIRTY,
        ],
        &["7f3a · payments-platform", LIVE],
    );
    app.key("left");
    let back = app.wait_screen("workspace again", |screen| {
        let line = focused_line(screen);
        line.contains('▾')
            && line.contains("payments-platform")
            && !text(screen).contains("Lifecycle")
            && !text(screen).contains("c41e · payments-platform")
    });
    see(
        &back,
        &["payments-platform", "Current directory"],
        &["Lifecycle", "c41e · payments-platform"],
    );
    app.finish();
}

#[test]
fn j01_status_status() {
    first_use();
    let app = open("j01-status-return", "returning");
    returning(&app);
    launch_running();
}
