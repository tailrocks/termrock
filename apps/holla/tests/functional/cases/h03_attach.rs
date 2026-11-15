//! Holla detach, attach, stop, and escalation, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h03_attach/` and each checkpoint's dots become underscores.
//!
//! `activities-multi` is paused. `fixtures.rs` sets `clock.running` only when
//! motion is not paused, and `World::tick` returns before `tick += 1` while
//! the clock is stopped, so the fixture's 48 ticks do not run. The `monitor:btm`
//! CPU line is at tick 1 and is not on screen. The tick-0 banner is the stable
//! body, and `duration_ticks` stays at 0 s. Confirming Stop and quit cannot
//! exit either: `Cancelling` never reaches `TERM_EXIT_TICKS`. The escalation
//! launch uses reduced motion so the resistant script can reach SIGKILL.

use std::time::Duration;

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{App, HOLLA, Launch, Point, cell_bg, row_text, text, try_launch};

const BANNER: &str = "btm 0.11.1";
const ATTACHED: &str = "Attached to btm";
const DETACHED: &str = "Detached · the program keeps running in its tab";
const TERM_LINE: &str = "stop requested by holla · SIGTERM to the process group";
const KILL_LINE: &str = "killed (SIGKILL) · descendants reaped";
const STUBBORN: &str = "Run the stubborn worker";
const WIDE: &str = "H03.attach.attach.120x40";
const NARROW: &str = "H03.attach.attach.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h03_attach/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str, motion: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions");
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

fn label_bg(screen: &TerminalSnapshot, row_needle: &str, label: &str) -> Option<SnapshotColor> {
    let row = (0..screen.rows()).find(|row| row_text(screen, *row).contains(row_needle))?;
    let line = row_text(screen, row);
    let byte = line.find(label)?;
    let mut offset = 0;
    for (column, cell) in screen.row(row).expect("row").iter().enumerate() {
        let next = offset + cell.text().len();
        if byte >= offset && byte < next {
            return Some(cell_bg(
                screen,
                Point {
                    x: u16::try_from(column).expect("column"),
                    y: u16::try_from(row).expect("row"),
                },
            ));
        }
        offset = next;
    }
    None
}

fn selected(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('›') else {
            return false;
        };
        let Some(at) = line.find(label) else {
            return false;
        };
        at > mark && at - mark <= 4
    })
}

fn open_btm(app: &App) -> TerminalSnapshot {
    app.key("ctrl+g");
    let open = app.wait_screen("activities open", |screen| {
        let visible = text(screen);
        visible.contains("5 live")
            && visible.contains("⠋ frontend dev")
            && visible.contains("○ btm")
    });
    let cursor = label_bg(&open, "⠋ frontend dev", "frontend dev").expect("frontend fill");
    let rest = label_bg(&open, "○ btm", "btm").expect("btm rest");
    assert_ne!(
        cursor,
        rest,
        "frontend dev is not the highlighted row\n{}",
        text(&open)
    );
    for _ in 0..4 {
        app.key("down");
    }
    let moved = app.wait_screen("btm highlighted", |screen| {
        label_bg(screen, "○ btm", "btm") == Some(cursor)
            && label_bg(screen, "⠋ frontend dev", "frontend dev") != Some(cursor)
    });
    assert_ne!(
        label_bg(&moved, "⠋ frontend dev", "frontend dev"),
        Some(cursor),
        "frontend dev stayed highlighted\n{}",
        text(&moved)
    );
    app.key("enter");
    let page = app.wait_screen("btm output", |screen| {
        let visible = text(screen);
        visible.contains(BANNER)
            && visible.contains("btm screen")
            && visible.contains("Back to Here")
            && visible.contains("detached")
            && !visible.contains("program exited")
            && !visible.contains("5 live")
    });
    see(
        &page,
        &[BANNER, "btm screen", "Esc Back to Here"],
        &["program exited", "stopped", "Search actions"],
    );
    page
}

fn back_to_finder(app: &App) {
    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        visible.contains("Search actions")
            && !visible.contains("btm screen")
            && !visible.contains("5 live")
    });
    see(
        &finder,
        &["Search actions", "Here"],
        &["btm screen", BANNER],
    );
}

/// Paused activities stay `Cancelling`, so Stop and quit never exits.
/// Esc is Stay, the focused control. `finish` would send Ctrl+Q again.
fn draw_quit_and_stay(app: &App) {
    app.key("ctrl+q");
    let dialog = app.wait_screen("stop and quit", |screen| {
        let visible = text(screen);
        visible.contains("Stop and quit") && visible.contains("▎Stay")
    });
    see(
        &dialog,
        &["Stop and quit", "Stay", "still running"],
        &["program exited"],
    );
    assert!(app.child_running(), "quit dialog closed the process");
    app.key("escape");
    let stayed = app.wait_screen("stayed", |screen| !text(screen).contains("Stop and quit"));
    assert!(app.child_running(), "Stay exited\n{}", text(&stayed));
}

fn attached(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(ATTACHED)
        && visible.contains("Ctrl+]")
        && visible.contains(BANNER)
        && visible.contains("btm screen")
        && !visible.contains("program exited")
        && !visible.contains("Search actions")
}

/// A same-size resize does not force a frame. Nudge the width and come back
/// while the attached monitor is still the page.
fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && attached(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && attached(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

#[test]
fn holla_attach_route_returns_to_the_finder() {
    let app = launch("h03-attach-route", "activities-multi", "paused");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Search actions", "frontend dev"],
        &["btm screen", "program exited"],
    );
    open_btm(&app);
    back_to_finder(&app);
    draw_quit_and_stay(&app);
}

#[test]
fn holla_attach_detaches_without_stopping_then_escalates() {
    let app = launch("h03-attach-attach", "activities-multi", "paused");
    let opened = open_btm(&app);
    see(
        &opened,
        &[BANNER, "detached", "Enter Attach"],
        &["program exited", "stopped", ATTACHED],
    );
    app.key("enter");
    let attached_page = app.wait_screen("attached to btm", attached);
    assert_eq!((attached_page.cols(), attached_page.rows()), (120, 40));
    see(
        &attached_page,
        &[ATTACHED, "Ctrl+] Detach", BANNER, "q Exit the program"],
        &[
            "program exited",
            "stopped",
            "Search actions",
            "Enter Attach",
        ],
    );
    nudge(&app);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("attached at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && attached(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(
        &narrowed,
        &[ATTACHED, "Ctrl+]", BANNER],
        &["program exited", "Search actions"],
    );
    nudge(&app);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("attached back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && attached(screen)
    });
    see(&restored, &[ATTACHED, BANNER], &["program exited"]);

    app.key("ctrl+]");
    let detached = app.wait_screen("detached", |screen| {
        let visible = text(screen);
        visible.contains(DETACHED)
            && visible.contains(BANNER)
            && !visible.contains("program exited")
    });
    see(
        &detached,
        &[DETACHED, BANNER, "running"],
        &["program exited", "stopped", ATTACHED],
    );

    app.key("s");
    let stopping = app.wait_screen("stopping btm", |screen| {
        let visible = text(screen);
        visible.contains("Stopping btm") && visible.contains(TERM_LINE)
    });
    see(
        &stopping,
        &["Stopping btm", TERM_LINE, "cancelling", BANNER],
        &["program exited", "killed (SIGKILL)"],
    );

    app.key("ctrl+g");
    let listed = app.wait_screen("activities after stop", |screen| {
        let visible = text(screen);
        visible.contains("5 live")
            && visible.contains("⠋ frontend dev")
            && visible.contains("◐ btm")
    });
    let front = (0..listed.rows())
        .map(|row| row_text(&listed, row))
        .find(|line| line.contains("⠋ frontend dev"))
        .expect("frontend dev row");
    let btm = (0..listed.rows())
        .map(|row| row_text(&listed, row))
        .find(|line| line.contains("◐ btm"))
        .expect("btm row");
    assert!(
        front.contains("running") && !front.contains("stopped") && !front.contains("cancelling"),
        "frontend dev changed\n{front}"
    );
    assert!(
        btm.contains("cancelling") && !btm.contains("running · host"),
        "btm did not change\n{btm}"
    );
    app.key("escape");
    app.wait_screen("picker closed", |screen| !text(screen).contains("5 live"));
    draw_quit_and_stay(&app);
    drop(app);

    let task = launch("h03-attach-escalate", "parity-task-input", "reduced");
    task.paste(STUBBORN);
    let row = task.wait_screen("stubborn row", |screen| {
        selected(screen, STUBBORN) && text(screen).contains("Results · 1")
    });
    see(
        &row,
        &[STUBBORN, "Results · 1"],
        &["ignoring SIGTERM", "Quick no-op"],
    );
    task.key("enter");
    let running = task.wait_screen("stubborn output", |screen| {
        let visible = text(screen);
        visible.contains("ignoring SIGTERM…") && visible.contains("$ ./stubborn")
    });
    see(
        &running,
        &["ignoring SIGTERM…", "$ ./stubborn", "Esc Back to Here"],
        &[
            "Attached to",
            "Ctrl+] Detach",
            "connecting to release server",
            "Quick no-op",
            KILL_LINE,
        ],
    );
    task.key("s");
    let killed = task.wait_screen("sigkill", |screen| text(screen).contains(KILL_LINE));
    see(
        &killed,
        &[
            KILL_LINE,
            "no exit after SIGTERM for 750 ms · SIGKILL to the process group",
            "$ ./stubborn",
            TERM_LINE,
        ],
        &[
            "connecting to release server",
            "Quick no-op",
            "Deploy the release",
            "program exited",
        ],
    );
    task.key("ctrl+g");
    let picker = task.wait_screen("stubborn stopped", |screen| {
        let visible = text(screen);
        visible.contains("stubborn") && visible.contains("stopped") && visible.contains("0 live")
    });
    see(
        &picker,
        &["stubborn", "stopped", KILL_LINE],
        &[
            "Quick no-op",
            "Deploy the release",
            "connecting to release server",
        ],
    );
    task.key("escape");
    task.wait_absent("0 live");
    task.finish();
}
