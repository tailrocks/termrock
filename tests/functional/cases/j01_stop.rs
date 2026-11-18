//! Jackin manager stop, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j01_stop/`. The shots are the open stop dialog, before
//! Keep running and before Stop. A same-size resize does not force a frame,
//! and the cursor column is part of the hash, so each shot nudges the width
//! and comes back.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const SHORT: &str = "7f3a";
const OTHER: &str = "9b02";
const TITLE: &str = "Stop instance 7f3a?";
const FACT: &str = "preserved with uncommitted work";
const CONFIRM: &str = "Stop";
const CANCEL: &str = "Keep running";
const LIVE: &str = "running · Container and Capsule daemon are live.";
const DIRTY: &str = "preserved · dirty";
const STOPPED: &str = "Instance 7f3a stopped";
const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const WIDE: &str = "J01.stop.stop.120x40";
const NARROW: &str = "J01.stop.stop.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j01_stop/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "returning".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    // `returning` syncs the arbiter to live instances, so entry joins the
    // manager. Reduced motion holds the intro only when entry plays one.
    // Several Jackin processes start together in this partition. The first
    // paint can arrive after the shared 10s wait, while the process is alive
    // and the screen is still blank.
    let started = Instant::now();
    let screen = loop {
        let screen = app.screen();
        let visible = text(&screen);
        if visible.contains(DIRECTORY)
            || visible.contains("Stand up, operator")
            || visible.contains("Knock, knock, operator.")
        {
            break screen;
        }
        assert!(
            app.child_running(),
            "jackin exited before the first frame\n{visible}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "timed out: manager or intro\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    if text(&screen).contains(DIRECTORY) {
        return screen;
    }
    app.key("space");
    app.wait_text(DIRECTORY)
}

fn dialog_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // The title contains "Stop", and the body paints before the action row.
    // The open dialog has the full preserve sentence and a bordered
    // "Keep running" row. The two buttons may wrap, so they need not share
    // that row.
    visible.contains(TITLE)
        && visible.contains(FACT)
        && visible
            .lines()
            .any(|line| line.contains(CANCEL) && line.contains('│'))
}

fn running_on(screen: &TerminalSnapshot, short: &str) -> bool {
    text(screen).contains(short) && text(screen).contains(LIVE)
}

fn painted_running(screen: &TerminalSnapshot) -> u32 {
    let mut found = Vec::new();
    for line in text(screen).lines() {
        if !line.contains("Workspaces") {
            continue;
        }
        let Some(at) = line.find(" running") else {
            continue;
        };
        let digits: String = line[..at]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if digits.is_empty() {
            continue;
        }
        found.push(digits.parse::<u32>().expect("running count"));
    }
    assert!(
        !found.is_empty(),
        "no painted running count\n{}",
        text(screen)
    );
    assert!(
        found.iter().all(|count| *count == found[0]),
        "running counts disagree: {found:?}\n{}",
        text(screen)
    );
    found[0]
}

fn action_hits(screen: &TerminalSnapshot, label: &str) -> Vec<Point> {
    find_all(screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(screen, u32::from(point.y));
            row.contains(CANCEL) && row.contains('│')
        })
        .collect()
}

fn dialog_action(app: &App, label: &str) -> Point {
    // A later `screen()` can land in the middle of the resize repaint, after
    // the title and before the action row. Click the frame that has the row.
    let screen = app.wait_screen("stop dialog action", |screen| {
        dialog_open(screen) && action_hits(screen, label).len() == 1
    });
    let hits = action_hits(&screen, label);
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

fn select_7f3a(app: &App) -> TerminalSnapshot {
    app.key("down");
    app.wait_text(PAYMENTS);
    app.key("right");
    app.key("down");
    app.wait_screen("7f3a running", |screen| running_on(screen, SHORT))
}

fn open_stop(app: &App) -> TerminalSnapshot {
    app.key("t");
    app.wait_screen("stop dialog", dialog_open)
}

fn cancel_stop(app: &App) -> TerminalSnapshot {
    app.click(dialog_action(app, CANCEL));
    app.wait_screen("cancel leaves the instance running", |screen| {
        !text(screen).contains(TITLE) && running_on(screen, SHORT)
    })
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the dialog stays open.
fn nudge(app: &App, quiet: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet, Some("nudge left the frame"));
}

fn shot(app: &App, checkpoint: &str) {
    nudge(app, dialog_open);
    app.visual(&visual_id(checkpoint));
}

fn wide_dialog(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120 && screen.rows() == 40 && dialog_open(screen)
}

fn baseline_hit(app: &App, checkpoint: &str) -> bool {
    let root = crate::support::oracle_baselines();
    app.check_visual(&root, &visual_id(checkpoint)).is_pass()
}

/// The reference capture shows `last seen 4 s ago` and `daemon · 4 s ago`.
/// A compare started on that second is still running when the next sim
/// second replaces it, and the 700 ms window then holds the replacement.
/// `3 s ago` is the second before the capture. Its boundary restarts the
/// window on `4 s ago`.
fn shoot_wide(app: &App) {
    let previous = ["last seen 3 s ago", "daemon · 3 s ago"];
    for _ in 0..4 {
        app.wait_screen("before the previous age", |screen| {
            let visible = text(screen);
            wide_dialog(screen) && previous.iter().all(|item| !visible.contains(item))
        });
        app.wait_screen("previous age", |screen| {
            let visible = text(screen);
            wide_dialog(screen) && previous.iter().all(|item| visible.contains(item))
        });
        std::thread::sleep(Duration::from_millis(250));
        if wide_dialog(&app.screen()) && baseline_hit(app, WIDE) {
            return;
        }
    }
    app.visual(&visual_id(WIDE));
}

fn show_9b02(app: &App) -> TerminalSnapshot {
    app.key("down");
    app.wait_text("c41e");
    app.key("down");
    app.wait_text(INFRA);
    app.key("right");
    app.key("down");
    app.wait_screen("9b02 still running", |screen| {
        running_on(screen, OTHER) && !text(screen).contains(TITLE)
    })
}

#[test]
fn j01_stop_route() {
    let app = open("j01-stop-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let running = painted_running(&manager);
    assert_eq!(running, 2, "manager paints two running instances");
    select_7f3a(&app);
    let dialog = open_stop(&app);
    assert!(dialog_open(&dialog), "{}", text(&dialog));
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    let kept = cancel_stop(&app);
    assert_eq!(painted_running(&kept), running, "{}", text(&kept));
    assert!(running_on(&kept, SHORT), "{}", text(&kept));
    assert!(!text(&kept).contains(TITLE), "{}", text(&kept));
    assert!(!text(&kept).contains(DIRTY), "{}", text(&kept));
    app.finish();
}

#[test]
fn j01_stop_stop() {
    let app = open("j01-stop-stop");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY));
    let running = painted_running(&manager);
    assert_eq!(running, 2, "manager paints two running instances");
    select_7f3a(&app);
    let dialog = open_stop(&app);
    assert!(
        text(&dialog).contains(TITLE) && text(&dialog).contains(FACT),
        "{}",
        text(&dialog)
    );
    assert!(text(&dialog).contains(CONFIRM) && text(&dialog).contains(CANCEL));
    nudge(&app, dialog_open);
    shoot_wide(&app);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && dialog_open(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_open(screen)
    });

    let kept = cancel_stop(&app);
    assert_eq!(painted_running(&kept), running, "{}", text(&kept));
    assert!(running_on(&kept, SHORT), "{}", text(&kept));
    assert!(!text(&kept).contains(TITLE), "{}", text(&kept));

    open_stop(&app);
    app.click(dialog_action(&app, CONFIRM));
    let stopped = app.wait_text(STOPPED);
    assert!(!text(&stopped).contains(TITLE), "{}", text(&stopped));
    assert!(text(&stopped).contains(DIRTY), "{}", text(&stopped));
    assert!(text(&stopped).contains(SHORT), "{}", text(&stopped));
    assert_eq!(
        painted_running(&stopped),
        running - 1,
        "running count did not drop\n{}",
        text(&stopped)
    );
    assert!(
        !text(&stopped).contains(&format!("{running} running")),
        "{}",
        text(&stopped)
    );

    let other = show_9b02(&app);
    assert!(running_on(&other, OTHER), "{}", text(&other));
    assert!(text(&other).contains(STOPPED), "{}", text(&other));
    assert!(!text(&other).contains(TITLE), "{}", text(&other));
    app.finish();
}
