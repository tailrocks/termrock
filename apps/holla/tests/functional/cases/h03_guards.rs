//! Holla close and quit guards, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h03_guards/` and each checkpoint's dots become underscores.
//!
//! `request_close_tab` refuses Here with `Here cannot be closed · Ctrl+Q quits`.
//! A live activity opens `Stop {name}?` (`Keep running` focused, `Stop and close`
//! is action 1). A finished activity closes with no dialog. `open_quit_confirm`
//! uses `Quit holla❯?`, `Stay` focused, and `Stop and quit`. Finder `q` inserts
//! query text because `shell_keys` clears the quit letter.
//!
//! `fixtures::base` sets `clock.running` only when motion is not paused, and
//! `World::tick` returns before `self.tick` advances when the clock is stopped.
//! A cancelling activity therefore never reaches `TERM_EXIT_TICKS` on the
//! checkpoint launch. Stop and quit is confirmed there, and the exit code is
//! observed on a reduced launch, where the clock runs. No `--frame`.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const HERE: &str = "Here cannot be closed · Ctrl+Q quits";
const QUERY: &str = "Search actions";
const SCOPE: &str = "scope ‹";
const FRONTEND: &str = "frontend dev";
const VITE: &str = "VITE v6.3.1";
const URL: &str = "http://localhost:5173/";
const STOP_TAB: &str = "Stop frontend dev?";
const KEEP: &str = "Keep running";
const STOP_CLOSE: &str = "Stop and close";
const STOPPING: &str = "Stopping · the tab closes once the process is gone";
const QUIT: &str = "Quit holla";
const STAY: &str = "Stay";
const STOP_QUIT: &str = "Stop and quit";
const RUNNING: &str = "activities are still running";
const DETACHED: &str = "detached monitors keep";
const QUITTING: &str = "quitting when every process is gone";
const WIDE: &str = "H03.guards.quit.120x40";
const NARROW: &str = "H03.guards.quit.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h03_guards/{}",
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
    app.wait_text(QUERY);
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

fn query_field(screen: &TerminalSnapshot) -> Option<String> {
    let visible = text(screen);
    let line = visible.lines().find(|line| line.contains(SCOPE))?;
    let at = line.find(SCOPE)?;
    Some(line[..at].trim().to_string())
}

fn query_is(screen: &TerminalSnapshot, query: &str) -> bool {
    let Some(field) = query_field(screen) else {
        return false;
    };
    if query.is_empty() {
        field.contains(QUERY)
    } else {
        field.contains(query) && !field.contains(QUERY)
    }
}

fn frontend_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(FRONTEND)
        && (visible.contains(VITE) || visible.contains(URL))
        && !visible.contains(STOP_TAB)
        && !visible.contains(QUIT)
}

fn quit_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(QUIT)
        && visible.contains(STAY)
        && visible.contains(STOP_QUIT)
        && visible.contains(RUNNING)
        && visible.contains(DETACHED)
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn picker_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("Activities") && visible.contains("Switch") && visible.contains(FRONTEND)
}

fn open_frontend(app: &App) {
    app.key("ctrl+g");
    let picker = app.wait_screen("activities", picker_open);
    let body = text(&picker);
    let frontend_at = body.find(FRONTEND).expect("frontend dev");
    let api_at = body.find("api dev").expect("api dev");
    assert!(
        frontend_at < api_at,
        "frontend dev was not the first activity\n{body}"
    );
    app.key("enter");
    let page = app.wait_screen("frontend dev open", frontend_open);
    see(&page, &[FRONTEND, URL], &[STOP_TAB, QUIT, STOPPING]);
}

fn keep_running(app: &App) {
    app.key("ctrl+w");
    let dialog = app.wait_screen("close guard", |screen| {
        let visible = text(screen);
        visible.contains(STOP_TAB) && visible.contains(KEEP) && visible.contains(STOP_CLOSE)
    });
    see(
        &dialog,
        &[
            STOP_TAB,
            KEEP,
            STOP_CLOSE,
            "Closing the tab stops the work it holds.",
        ],
        &[QUITTING],
    );
    app.key("enter");
    let kept = app.wait_screen("kept running", |screen| {
        frontend_open(screen) && !text(screen).contains(KEEP) && !text(screen).contains(STOPPING)
    });
    see(&kept, &[FRONTEND, URL], &[STOP_TAB, STOPPING, KEEP]);
}

fn type_query_q(app: &App) {
    if query_field(&app.screen()).is_none() {
        app.key("escape");
        app.wait_screen("here", |screen| {
            query_is(screen, "") && text(screen).contains(FRONTEND)
        });
    }
    app.key("q");
    let typed = app.wait_screen("q is query text", |screen| {
        query_is(screen, "q") && !text(screen).contains(QUIT)
    });
    assert!(app.child_running(), "q quit the process");
    see(&typed, &["q", SCOPE], &[QUIT, STOP_QUIT]);
    app.key("backspace");
    let cleared = app.wait_screen("query cleared", |screen| query_is(screen, ""));
    see(&cleared, &[QUERY], &[QUIT]);
}

fn choose_stop(app: &App) {
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit guard", quit_open);
    see(&dialog, &[QUIT, STAY, STOP_QUIT, RUNNING, DETACHED], &[]);
    app.key("right");
    app.key("enter");
    let stopping = app.wait_screen("stop accepted", |screen| {
        let visible = text(screen);
        !visible.contains(QUIT)
            && !visible.contains(STOP_QUIT)
            && !visible.contains(STAY)
            && visible.contains("stopping")
            && visible.contains(QUITTING)
    });
    assert!(
        app.child_running(),
        "stop and quit exited before the clock reaped"
    );
    see(&stopping, &["stopping", QUITTING], &[QUIT, STOP_QUIT, STAY]);
}

fn stop_and_quit(app: &App) {
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit guard", quit_open);
    see(&dialog, &[QUIT, STAY, STOP_QUIT, RUNNING, DETACHED], &[]);
    app.key("right");
    app.key("enter");
    // Reduced motion reaps the task three ticks after stop, then the shell
    // exits on its own. `wait_screen` records that frame, and the record can
    // outlast the process, so the following Ctrl+Q hits a closed PTY.
    let started = Instant::now();
    let stopping = loop {
        let screen = app.screen();
        let visible = text(&screen);
        if !visible.contains(QUIT)
            && !visible.contains(STOP_QUIT)
            && !visible.contains(STAY)
            && visible.contains("stopping")
            && visible.contains(QUITTING)
        {
            break screen;
        }
        assert!(
            started.elapsed() < Duration::from_secs(3) && app.child_running(),
            "stop and quit did not stay up\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(app.child_running(), "stop and quit exited before Ctrl+Q");
    see(&stopping, &["stopping", QUITTING], &[QUIT, STOP_QUIT, STAY]);
    app.finish();
}

fn succeeded_selected(screen: &TerminalSnapshot) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('›') else {
            return false;
        };
        let Some(at) = line.find("api tests") else {
            return false;
        };
        at > mark && at - mark <= 4 && line.contains("succeeded")
    })
}

fn close_succeeded_from_picker(app: &App) {
    let mut found = false;
    for _ in 0..6 {
        if succeeded_selected(&app.screen()) {
            found = true;
            break;
        }
        app.key("down");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        found,
        "succeeded activity was not selected\n{}",
        text(&app.screen())
    );
    app.key("enter");
    app.wait_screen("finished activity", |screen| {
        let visible = text(screen);
        visible.contains("succeeded") && visible.contains("exit 0") && !visible.contains(" live")
    });
    app.key("ctrl+w");
    let closed = app.wait_screen("finished tab closed", |screen| {
        let visible = text(screen);
        !visible.contains("Stop api tests?")
            && !visible.contains(KEEP)
            && !visible.contains(STOPPING)
            && visible.contains(FRONTEND)
            && visible.contains(QUERY)
    });
    see(
        &closed,
        &[FRONTEND, QUERY],
        &["Stop api tests?", KEEP, STOPPING],
    );
}

fn close_completed_if_present(app: &App) -> bool {
    app.key("ctrl+g");
    let picker = app.wait_screen("activity list", picker_open);
    if !text(&picker).contains("succeeded") {
        app.key("escape");
        app.wait_screen("activity list closed", |screen| {
            text(screen).contains(QUERY) && !text(screen).contains("Switch")
        });
        return false;
    }
    close_succeeded_from_picker(app);
    true
}

fn close_finished_noop() {
    let app = launch("h03-guards-finished", "parity-task-input", "reduced");
    app.paste("Quick no-op");
    let row = app.wait_screen("quick no-op selected", |screen| {
        selected(screen, "Quick no-op") && text(screen).contains("Results · 1")
    });
    see(
        &row,
        &["Quick no-op", "Results · 1"],
        &["Password:", "$ true"],
    );
    app.key("enter");
    let done = app.wait_screen("quick no-op finished", |screen| {
        let visible = text(screen);
        visible.contains("quick no-op")
            && visible.contains("succeeded")
            && visible.contains("exit 0")
            && visible.contains("$ true")
            && !visible.contains("Password:")
    });
    see(
        &done,
        &["quick no-op", "succeeded", "exit 0", "$ true"],
        &["Password:", "Deploy the release", STOPPING],
    );
    app.key("ctrl+w");
    let closed = app.wait_screen("finished tab closed", |screen| {
        let visible = text(screen);
        visible.contains("Quick no-op")
            && visible.contains(SCOPE)
            && visible.contains("quick no-op finished")
            && !visible.contains("Stop quick no-op?")
            && !visible.contains(KEEP)
            && !visible.contains(STOPPING)
            && !visible.contains("$ true")
            && !visible.contains("Password:")
            && !visible.contains("quick no-op ·")
    });
    see(
        &closed,
        &["Quick no-op", SCOPE, "quick no-op finished"],
        &[
            "Stop quick no-op?",
            KEEP,
            STOPPING,
            "$ true",
            "Password:",
            "quick no-op ·",
        ],
    );
    app.finish();
}

#[test]
fn holla_guards_route_keeps_here_and_a_running_tab() {
    let app = launch("h03-guards-route", "activities-multi", "reduced");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[QUERY, FRONTEND], &[QUIT, STOP_TAB]);

    app.key("ctrl+w");
    let refused = app.wait_text(HERE);
    assert!(app.child_running(), "closing Here quit");
    see(&refused, &[HERE, QUERY], &[QUIT, STOP_QUIT, STOP_TAB]);

    app.key("q");
    let typed = app.wait_screen("q is query text", |screen| query_is(screen, "q"));
    assert!(app.child_running(), "q quit the process");
    see(&typed, &[HERE], &[QUIT, STOP_QUIT]);
    app.key("backspace");
    app.wait_screen("query cleared", |screen| query_is(screen, ""));

    open_frontend(&app);
    keep_running(&app);
    stop_and_quit(&app);
}

#[test]
fn holla_guards_quit_stays_then_stops() {
    let app = launch("h03-guards-quit", "activities-multi", "paused");
    if !close_completed_if_present(&app) {
        close_finished_noop();
    }
    open_frontend(&app);
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit dialog", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && quit_open(screen)
    });
    see(
        &dialog,
        &[QUIT, STAY, STOP_QUIT, RUNNING, DETACHED],
        &[STOPPING],
    );

    nudge(&app, quit_open);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("quit at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && quit_open(screen)
    });
    see(&narrowed, &[QUIT, STAY, STOP_QUIT], &[STOPPING]);
    nudge(&app, quit_open);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("quit back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && quit_open(screen)
    });
    see(&restored, &[QUIT, STAY, RUNNING], &[]);

    app.key("enter");
    let stayed = app.wait_screen("stayed", |screen| {
        frontend_open(screen) && !text(screen).contains(STAY) && !text(screen).contains(STOPPING)
    });
    assert!(app.child_running(), "stay quit");
    see(
        &stayed,
        &[FRONTEND, URL],
        &[QUIT, STOP_QUIT, STOPPING, STAY],
    );

    type_query_q(&app);
    choose_stop(&app);
    assert!(
        app.try_sustained(Duration::from_millis(400), |screen| {
            let visible = text(screen);
            visible.contains("stopping")
                && visible.contains(QUITTING)
                && visible.contains(FRONTEND)
                && !visible.contains(QUIT)
        }),
        "paused stop reaped the task or closed the quit dialog without stopping"
    );

    let exit = launch("h03-guards-quit-exit", "activities-multi", "reduced");
    open_frontend(&exit);
    stop_and_quit(&exit);
}
