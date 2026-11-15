//! Holla quit guard, driven through the real binary.
//!
//! Tuiscotti ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h06_quit/`. The shot is the open cleanup dialog, before
//! Stay, once the page title is `Cleanup report`. A live activity cannot
//! be that shot: `strip_items` and `draw_status` in `src/bin/holla/app.rs`
//! keep a spinner on the tab and the status bar, and that frame never
//! holds still.
//!
//! `open_quit_confirm` freezes `{k} of {t}` from the cleanup job at the
//! moment Ctrl+Q is handled. Queuing Ctrl+Q with Enter paints `0 of 4`
//! when no tick has landed yet, and `1 of 4` when one has. The shot waits
//! until the report paints `1 of 4 items`, then sends Ctrl+Q.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const MAC: &str = "parity-platforms";
const LINUX: &str = "parity-platforms-linux";
const BREW: &str = "Homebrew services";
const CLEANUP: &str = "Review cleanup candidates";
const BTM: &str = "Open btm";
const SERVICE_ROW: &str = "start · stop · restart";
const MAC_PATH: &str = "~/work/mac";
const LINUX_PATH: &str = "~/work/box";
const TITLE: &str = "Quit holla❯?";
const REMOTE_TITLE: &str = "Quit holla❯ on";
const STAY: &str = "Stay";
const LEAVE: &str = "Leave when it settles";
const STOP_QUIT: &str = "Stop and quit";
const ACTIVITY_BODY: &str = "still running. Quitting stops what holla started";
const SETTLE: &str = "leaving when the cleanup settles · its report and log land first";
const ALREADY: &str = "Already stopping · the shell leaves when every process is gone";
const STOPPING: &str = "stopping ";
const QUITTING: &str = "quitting when every process is gone";
const MONITOR: &str = "q quits the monitor";
const REPORT: &str = "Cleanup report";
const ONE_OF_FOUR: &str = "1 of 4 items";
const WIDE: &str = "pre_refactor/h06_quit/h06_quit_quit_120x40";
const NARROW: &str = "pre_refactor/h06_quit/h06_quit_quit_narrow";

fn launch(name: &str, scenario: &str) -> App {
    launch_motion(name, scenario, "reduced", None)
}

/// Paused ticks every 500 ms. `--frame 4` finishes discovery while the clock
/// is forced on, then the clock stops. Cleanup still steps on each tick, so
/// the one-tick confirm status stays up long enough to read.
fn launch_paused(name: &str, scenario: &str) -> App {
    launch_motion(name, scenario, "paused", Some(4))
}

fn launch_motion(name: &str, scenario: &str, motion: &str, frame: Option<u64>) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    launch.history = false;
    launch.platform = "macos".to_string();
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    if let Some(frame) = frame {
        launch.args.push("--frame".to_string());
        launch.args.push(frame.to_string());
    }
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

fn row_has(screen: &TerminalSnapshot, parts: &[&str]) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        parts.iter().all(|part| line.contains(part))
    })
}

/// Dialog copy wraps inside its borders. Joining the column that holds the
/// title keeps one sentence even when a row split it.
fn dialog_prose(screen: &TerminalSnapshot) -> String {
    let rows: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let Some(index) = rows.iter().find_map(|line| {
        line.split('│')
            .position(|segment| segment.contains("Quit holla"))
    }) else {
        return String::new();
    };
    rows.iter()
        .filter_map(|line| {
            let segment = line.split('│').nth(index)?.trim();
            segment
                .chars()
                .any(|ch| ch.is_alphanumeric())
                .then(|| segment.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn finder(app: &App) {
    if !text(&app.screen()).contains("scope ‹") {
        app.key("esc");
        app.wait_screen("finder", |screen| text(screen).contains("scope ‹"));
    }
    if !text(&app.screen()).contains("Search actions and resources") {
        app.key("esc");
        app.wait_text("Search actions and resources");
    }
}

fn open_row(app: &App, label: &str) {
    finder(app);
    app.paste(label);
    app.wait_text(&format!("▎ {label}"));
    app.key("enter");
}

fn show_services(app: &App, scenario: &str) {
    open_row(app, BREW);
    let page = app.wait_text(SERVICE_ROW);
    if scenario == MAC {
        see(
            &page,
            &[BREW, "redis", "Homebrew", MAC_PATH],
            &["postgresql@17", LINUX_PATH],
        );
        assert!(
            row_has(&page, &["redis", "started"]),
            "mac redis started row missing\n{}",
            text(&page)
        );
    } else {
        see(
            &page,
            &[BREW, "postgresql@17", "Homebrew", LINUX_PATH],
            &["redis", MAC_PATH],
        );
        assert!(
            row_has(&page, &["postgresql@17", "stopped"]),
            "linux postgresql@17 stopped row missing\n{}",
            text(&page)
        );
    }
}

fn phrase_from(screen: &TerminalSnapshot) -> String {
    let spoken = text(screen);
    let Some(tail) = spoken.split("Type ").nth(1) else {
        panic!("gate 2 has no phrase\n{spoken}");
    };
    let Some(phrase) = tail.split(" to confirm").next() else {
        panic!("gate 2 phrase is open\n{spoken}");
    };
    let phrase = phrase.trim();
    assert!(phrase.starts_with("TRASH "), "{phrase}");
    phrase.to_string()
}

fn arm_cleanup(app: &App) {
    open_row(app, CLEANUP);
    app.wait_text("Cleanup review");
    app.key("a");
    app.wait_text("Every eligible candidate selected");
    app.key("d");
    app.wait_text("gate 1 of 2");
    app.key("right");
    app.key("enter");
    let gate = app.wait_screen("gate 2", |screen| text(screen).contains(" to confirm"));
    let phrase = phrase_from(&gate);
    app.type_text(&phrase);
    app.wait_screen("phrase typed", |screen| text(screen).contains(&phrase));
}

fn dialog_painted(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(&format!("▎{STAY}"))
        && (visible.contains(LEAVE) || visible.contains(STOP_QUIT))
}

/// A captured frame can clear the title row before the confirm button.
/// Stay is real only once that button and its hint are gone.
fn guard_closed(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    !visible.contains(TITLE)
        && !visible.contains(STOP_QUIT)
        && !visible.contains(LEAVE)
        && !visible.contains("Enter Stay")
}

fn assert_local_title(screen: &TerminalSnapshot) {
    see(screen, &[TITLE, &format!("▎{STAY}")], &[REMOTE_TITLE]);
}

/// `{k}` and `{t}` from the frozen cleanup sentence. `k` is whatever the
/// screen painted; the body does not move after Ctrl+Q.
fn running_count(body: &str) -> (u32, u32) {
    let marker = if let Some(index) = body.find("running (") {
        index + "running (".len()
    } else if let Some(index) = body.find("cleanup is at ") {
        index + "cleanup is at ".len()
    } else {
        panic!("no cleanup count\n{body}");
    };
    let tail = &body[marker..];
    let Some((k, rest)) = tail.split_once(" of ") else {
        panic!("count missing of\n{body}");
    };
    let Some(t) = rest.split(" items").next() else {
        panic!("count missing items\n{body}");
    };
    let k: u32 = k
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("bad count {k}\n{body}"));
    let t: u32 = t
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("bad total {t}\n{body}"));
    assert!(
        t > 0 && k < t,
        "count {k} of {t} is not in progress\n{body}"
    );
    (k, t)
}

fn cleanup_sentence(k: u32, t: u32) -> String {
    format!(
        "A cleanup is running ({k} of {t} items). Committed deletions cannot be stopped; leaving waits for the rest, then quits."
    )
}

fn open_cleanup_guard(app: &App) -> (TerminalSnapshot, u32, u32) {
    app.key("enter");
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit guard", |screen| {
        dialog_painted(screen) && text(screen).contains("A cleanup is running")
    });
    let body = dialog_prose(&dialog);
    let (k, t) = running_count(&body);
    assert!(
        body.contains(&cleanup_sentence(k, t)),
        "cleanup guard missing\n{body}"
    );
    (dialog, k, t)
}

fn open_shot_guard(app: &App) -> TerminalSnapshot {
    app.key("enter");
    // The dialog freezes the count at the tick that handles Ctrl+Q. A
    // recorded wait between seeing `1 of 4` and the key lets the next tick
    // paint `2 of 4`, and the guard then never matches the baseline.
    let started = Instant::now();
    loop {
        let visible = text(&app.screen());
        if !visible.contains(TITLE) && visible.contains(ONE_OF_FOUR) {
            break;
        }
        assert!(
            app.child_running() && started.elapsed() < Duration::from_secs(8),
            "report did not hold 1 of 4\n{visible}"
        );
        std::thread::yield_now();
    }
    app.key("ctrl+q");
    let started = Instant::now();
    let dialog = loop {
        let screen = app.screen();
        let visible = text(&screen);
        if dialog_painted(&screen) && visible.contains(ONE_OF_FOUR) && visible.contains(LEAVE) {
            break screen;
        }
        assert!(
            app.child_running() && started.elapsed() < Duration::from_secs(2),
            "quit guard did not freeze at 1 of 4\n{visible}"
        );
        std::thread::yield_now();
    };
    let body = dialog_prose(&dialog);
    assert!(
        body.contains(&cleanup_sentence(1, 4)),
        "shot opened on a different guard\n{body}"
    );
    app.wait_screen("report settled under the guard", shot_ready);
    dialog
}

fn open_monitor(app: &App) {
    open_row(app, BTM);
    app.wait_text(MONITOR);
}

fn quit_dialog(app: &App) -> TerminalSnapshot {
    app.key("ctrl+q");
    app.wait_screen("quit guard", dialog_painted)
}

fn painted(app: &App, needle: &str) -> String {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(3);
    let mut last = String::new();
    loop {
        let visible = text(&app.screen());
        let alive = app.child_running();
        if visible.chars().any(|ch| ch.is_alphanumeric()) {
            last = visible;
        }
        if alive && last.contains(needle) {
            return last;
        }
        assert!(
            alive && Instant::now() < deadline,
            "missing {needle} alive={alive} after {}ms\n{last}",
            started.elapsed().as_millis()
        );
        // The confirm status lasts until the next tick. Spin across that gap.
        if started.elapsed() < Duration::from_millis(400) {
            std::thread::yield_now();
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn focus_confirm(app: &App, label: &str) {
    app.key("right");
    let marked = format!("▎{label}");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        assert!(app.child_running(), "process exited before {label} focused");
        let visible = text(&app.screen());
        if visible.contains(&marked) {
            break;
        }
        assert!(Instant::now() < deadline, "confirm not focused\n{visible}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn press_confirm(app: &App, label: &str) {
    focus_confirm(app, label);
    app.key("enter");
}

/// The confirm status is replaced by the next tick. Wait until a tick has
/// just painted, then confirm, so the status is still the grid we read.
fn confirm_after_tick(app: &App, label: &str) -> String {
    focus_confirm(app, label);
    std::thread::sleep(Duration::from_millis(30));
    let opened = text(&app.screen());
    let marked = format!("▎{label}");
    assert!(
        opened.contains(&marked) && opened.contains(TITLE),
        "confirm lost the guard\n{opened}"
    );
    let started = Instant::now();
    loop {
        let visible = text(&app.screen());
        if visible != opened && visible.contains(&marked) && visible.contains(TITLE) {
            break;
        }
        assert!(
            app.child_running() && started.elapsed() < Duration::from_secs(3),
            "no tick before confirm\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    app.key("enter");
    painted(app, SETTLE)
}

fn wait_exit(app: &App) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "quit did not exit\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

fn shot_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains("A cleanup is running")
        && visible.contains("1 of 4")
        && visible.contains(LEAVE)
        && visible.contains(REPORT)
        && visible.contains(&format!("▎{STAY}"))
        && visible.contains(MAC_PATH)
        && !visible.contains("postgresql@17")
        && !visible.contains(REMOTE_TITLE)
        && !visible.contains(STOP_QUIT)
}

fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && shot_ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && shot_ready(screen)
    });
}

fn quit_shots(app: &App) {
    nudge(app);
    app.visual(WIDE);
    app.resize(80, 24);
    app.wait_screen("narrow quit dialog", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && shot_ready(screen)
    });
    nudge(app);
    app.visual(NARROW);
    app.resize(120, 40);
    app.wait_screen("quit dialog restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && shot_ready(screen)
    });
}

#[test]
fn h06_quit_route_stay_returns() {
    let app = launch("h06-quit-route", MAC);
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[MAC_PATH, "mbp"],
        &[TITLE, "postgresql@17", LINUX_PATH],
    );

    show_services(&app, MAC);
    open_monitor(&app);
    let dialog = quit_dialog(&app);
    assert!(app.child_running(), "quit dialog exited");
    assert_local_title(&dialog);
    see(
        &dialog,
        &[STOP_QUIT, MAC_PATH],
        &["postgresql@17", LINUX_PATH, LEAVE],
    );
    let body = dialog_prose(&dialog);
    assert!(
        body.contains(ACTIVITY_BODY),
        "activity guard missing\n{body}"
    );

    app.key("enter");
    let stayed = app.wait_screen("stay returned", |screen| {
        text(screen).contains(MONITOR) && guard_closed(screen)
    });
    assert!(app.child_running(), "Stay quit the process");
    see(&stayed, &[MONITOR, "btm"], &[TITLE, LEAVE, STOP_QUIT]);

    app.key("s");
    let stopped = app.wait_text("terminated (SIGTERM)");
    assert!(app.child_running(), "stopping the monitor quit holla");
    see(&stopped, &["terminated (SIGTERM)"], &[TITLE]);
    app.finish();
}

#[test]
fn h06_quit_guard_waits() {
    let app = launch_paused("h06-quit-cleanup-confirm", MAC);
    show_services(&app, MAC);
    arm_cleanup(&app);
    let (dialog, _k, _t) = open_cleanup_guard(&app);
    assert!(app.child_running(), "cleanup guard exited");
    assert_local_title(&dialog);
    see(
        &dialog,
        &[LEAVE, MAC_PATH],
        &[STOP_QUIT, "postgresql@17", LINUX_PATH, "redis"],
    );

    let waiting = confirm_after_tick(&app, LEAVE);
    assert!(
        app.child_running(),
        "confirm returned before the cleanup report"
    );
    assert!(waiting.contains(SETTLE), "settle status missing\n{waiting}");
    assert!(
        !waiting.contains(TITLE),
        "confirm left the dialog open\n{waiting}"
    );

    app.key("ctrl+q");
    let again = painted(&app, ALREADY);
    assert!(app.child_running(), "second quit exited before the status");
    assert!(
        again.contains(ALREADY),
        "already-stopping status missing\n{again}"
    );
    wait_exit(&app);

    let app = launch("h06-quit-activity-confirm", MAC);
    open_monitor(&app);
    let dialog = quit_dialog(&app);
    assert!(app.child_running(), "activity guard exited");
    assert_local_title(&dialog);
    see(&dialog, &[STOP_QUIT, MAC_PATH], &[LEAVE, "postgresql@17"]);
    let body = dialog_prose(&dialog);
    assert!(
        body.contains(ACTIVITY_BODY),
        "activity guard missing\n{body}"
    );
    press_confirm(&app, STOP_QUIT);
    let stopping = painted(&app, QUITTING);
    assert!(
        app.child_running(),
        "activity confirm exited before the wait status"
    );
    assert!(
        stopping.contains(STOPPING) && stopping.contains(QUITTING),
        "activity wait status missing\n{stopping}"
    );
    wait_exit(&app);

    let app = launch("h06-quit-combined", MAC);
    open_monitor(&app);
    app.key("esc");
    app.wait_screen("finder while btm stays live", |screen| {
        let visible = text(screen);
        visible.contains("scope ‹") && visible.contains("btm")
    });
    arm_cleanup(&app);
    app.key("enter");
    app.key("ctrl+q");
    let dialog = app.wait_screen("combined guard", |screen| {
        dialog_painted(screen) && text(screen).contains(STOP_QUIT)
    });
    assert!(app.child_running(), "combined guard exited");
    assert_local_title(&dialog);
    see(&dialog, &[STOP_QUIT], &[LEAVE, "postgresql@17"]);
    let body = dialog_prose(&dialog);
    let (k, t) = running_count(&body);
    assert!(
        body.contains(&format!(
            "still running and a cleanup is at {k} of {t} items"
        )),
        "combined guard missing\n{body}"
    );
    app.key("esc");
    let stayed = app.wait_screen("combined stay", |screen| {
        text(screen).contains("btm") && guard_closed(screen)
    });
    assert!(app.child_running(), "Esc quit while both were running");
    see(&stayed, &["btm"], &[TITLE, STOP_QUIT]);
    app.key("alt+1");
    app.wait_text(MONITOR);
    app.key("s");
    let stopped = app.wait_text("terminated (SIGTERM)");
    assert!(app.child_running(), "stopping btm quit holla");
    see(&stopped, &["terminated (SIGTERM)"], &[TITLE]);
    app.key("alt+0");
    app.wait_text(REPORT);
    app.finish();

    let app = launch("h06-quit-cleanup-dialog", MAC);
    show_services(&app, MAC);
    arm_cleanup(&app);
    let _dialog = open_shot_guard(&app);
    assert!(app.child_running(), "the open guard exited");
    quit_shots(&app);
    app.key("enter");
    let stayed = app.wait_screen("dialog dismissed", |screen| {
        text(screen).contains(REPORT) && guard_closed(screen)
    });
    assert!(app.child_running(), "Stay quit after the report");
    see(&stayed, &[REPORT], &[TITLE, LEAVE]);
    app.finish();

    let app = launch("h06-quit-linux", LINUX);
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[LINUX_PATH, "devbox"],
        &[TITLE, "redis", MAC_PATH, "postgresql@17"],
    );
    show_services(&app, LINUX);
    open_monitor(&app);
    let dialog = quit_dialog(&app);
    assert!(app.child_running(), "linux quit dialog exited");
    assert_local_title(&dialog);
    see(
        &dialog,
        &[STOP_QUIT, LINUX_PATH, "devbox"],
        &["redis", MAC_PATH, "postgresql@17", LEAVE],
    );
    let body = dialog_prose(&dialog);
    assert!(
        body.contains(ACTIVITY_BODY),
        "linux activity guard missing\n{body}"
    );
    app.key("esc");
    let stayed = app.wait_screen("esc stayed", |screen| {
        text(screen).contains(MONITOR) && guard_closed(screen)
    });
    assert!(app.child_running(), "Esc quit the process");
    see(&stayed, &[MONITOR], &[TITLE, STOP_QUIT]);
    app.key("s");
    let stopped = app.wait_text("terminated (SIGTERM)");
    assert!(app.child_running(), "stopping the monitor quit holla");
    see(
        &stopped,
        &["terminated (SIGTERM)", LINUX_PATH],
        &[TITLE, "redis"],
    );
    app.finish();
}
