//! Holla activity output, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h03_output/` and each checkpoint's dots become underscores.
//! The shots are the finished `frontend dev` failure: reduced motion still
//! runs the clock, and a live sibling keeps the frame moving.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const FAILED: &str = "frontend dev · failed · 1 s · exit 1";
const EADDR: &str = "listen EADDRINUSE: address already in use :::5173";
const TESTS_LINE: &str = "aliases_beat_everything";
const VITE_LINE: &str = "VITE v6.3.1";
const TESTS_HEAD: &str = "api tests · succeeded · 2 s · exit 0";
const WIDE: &str = "H03.output.states.120x40";
const NARROW: &str = "H03.output.states.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h03_output/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = ["--scenario", scenario]
        .into_iter()
        .map(str::to_string)
        .collect();
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

/// The activity title switches before its log replaces the previous body.
fn api_tests_body(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TESTS_HEAD)
        && visible.contains(TESTS_LINE)
        && !visible.contains(VITE_LINE)
        && !visible.contains("listening on 0.0.0.0:8080")
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

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. The waits follow the reported size, not the size text.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        text(&screen).contains(FAILED) && text(&screen).contains("EADDRINUSE"),
        "shot left the failed activity\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && text(screen).contains("EADDRINUSE")
    });
    app.resize(cols, rows);
    app.wait_screen("restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(FAILED)
    });
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
}

fn confirm_quit(app: &App) {
    app.key("ctrl+q");
    // The title paints before the actions. "Quit holla" alone is an empty dialog.
    let guard = app.wait_screen("quit dialog", |screen| {
        let visible = text(screen);
        visible.contains("Quit holla")
            && visible.contains("Stop and quit")
            && visible.contains("Stay")
    });
    see(&guard, &["Stop and quit", "Stay"], &[]);
    app.key("right");
    app.wait_screen("stop focused", |screen| {
        let visible = text(screen);
        visible.contains("▎Stop and quit") || visible.contains("Enter Stop")
    });
    app.key("enter");
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

fn assert_failed(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "{}",
        text(screen)
    );
    see(
        screen,
        &[
            FAILED,
            EADDR,
            "ELIFECYCLE",
            "exit 1",
            "Retry frontend dev",
            "Find the process on port 5173",
            "! frontend dev failed · exit 1",
        ],
        &[
            TESTS_LINE,
            "Emit a mixed stream",
            "~/work/batch",
            "btm 0.11.1",
            "discovering",
        ],
    );
}

#[test]
fn h03_output_route() {
    let app = open("h03-output-route", "activities-multi");
    app.wait_text("api tests");
    app.key("ctrl+g");
    let list = app.wait_text("Enter Switch");
    see(
        &list,
        &[
            "Activities",
            "Esc Cancel",
            "frontend dev",
            "running · child",
            "api dev",
            "api tests",
            "succeeded · project",
            "logs · api worker scheduler",
            "btm",
            "detached · host",
        ],
        &["~/work/batch", "Retry frontend dev"],
    );
    app.key("esc");
    app.wait_text("Search actions");
    app.key("alt+3");
    let output = app.wait_screen("api tests output", |screen| {
        api_tests_body(screen)
            && text(screen).contains("128 tests run: 128 passed")
            && text(screen).contains("started by Run api tests")
    });
    see(
        &output,
        &[
            TESTS_LINE,
            "128 tests run: 128 passed",
            "started by Run api tests",
        ],
        &[VITE_LINE, "EADDRINUSE"],
    );
    app.key("esc");
    let here = app.wait_text("Search actions");
    assert!(
        !text(&here).contains(TESTS_LINE),
        "Here kept the activity output\n{}",
        text(&here)
    );
    confirm_quit(&app);
}

#[test]
fn h03_output_states() {
    let app = open("h03-output-states", "activities-multi");
    app.wait_text("api tests");
    app.key("alt+1");
    let running = app.wait_text(VITE_LINE);
    see(
        &running,
        &[
            "frontend dev · running",
            "port 5173 · url http://localhost:5173/ · pid 7731",
            "http://localhost:5173/",
        ],
        &[TESTS_LINE],
    );
    app.key("alt+2");
    let api = app.wait_text("listening on 0.0.0.0:8080");
    see(
        &api,
        &[
            "api dev · running",
            "port 8080 · pid 9120",
            "Compiling api v0.9.2",
        ],
        &[VITE_LINE, TESTS_LINE],
    );
    app.key("alt+3");
    let succeeded = app.wait_screen("api tests output", |screen| {
        api_tests_body(screen)
            && text(screen).contains("Open the failing test")
            && text(screen).contains("Run tests again")
    });
    see(
        &succeeded,
        &[
            TESTS_LINE,
            "exit 0",
            "Open the failing test",
            "Run tests again",
        ],
        &[VITE_LINE],
    );
    app.key("alt+5");
    let detached = app.wait_text("btm 0.11.1");
    see(
        &detached,
        &["btm · detached", "started by Open system monitor"],
        &[VITE_LINE, TESTS_LINE],
    );
    app.key("alt+1");
    app.wait_text(VITE_LINE);
    app.key("s");
    let cancelling = app.wait_text("frontend dev · cancelling");
    see(
        &cancelling,
        &[
            "Stopping frontend dev · SIGTERM sent · output kept",
            "stop requested by holla · SIGTERM to the process group",
            VITE_LINE,
        ],
        &[TESTS_LINE],
    );
    let stopped = app.wait_text("frontend dev · stopped");
    see(
        &stopped,
        &[
            "exit 130",
            "terminated (SIGTERM) · process group reaped",
            VITE_LINE,
            "http://localhost:5173/",
        ],
        &[TESTS_LINE, "frontend dev · cancelling"],
    );
    confirm_quit(&app);

    let app = open("h03-output-states-failed", "launch-failure");
    let failed = app.wait_screen("failed frontend", |screen| {
        let visible = text(screen);
        visible.contains(FAILED)
            && visible.contains(EADDR)
            && visible.contains("Retry frontend dev")
            && visible.contains("! frontend dev failed · exit 1")
    });
    assert_failed(&failed, 120, 40);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("narrow failed", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(FAILED)
            && visible.contains("Retry frontend dev")
    });
    assert_failed(&narrow, 80, 24);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("wide restored", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(FAILED)
            && visible.contains("Retry frontend dev")
            && visible.contains("! frontend dev failed · exit 1")
    });
    assert_failed(&restored, 120, 40);
    app.finish();
}

#[test]
fn h03_output_scenario_activities_multi() {
    let app = open("h03-output-activities", "activities-multi");
    app.wait_text("api tests");
    app.key("alt+3");
    let tests = app.wait_screen("api tests output", |screen| {
        api_tests_body(screen) && text(screen).contains("128 tests run: 128 passed")
    });
    see(
        &tests,
        &[TESTS_LINE, "128 tests run: 128 passed"],
        &[VITE_LINE],
    );
    app.key("alt+1");
    // The activity title switches before its log replaces the previous body.
    let front = app.wait_screen("frontend dev output", |screen| {
        let visible = text(screen);
        visible.contains("frontend dev · running")
            && visible.contains(VITE_LINE)
            && visible.contains("pid 7731")
            && !visible.contains(TESTS_LINE)
            && !visible.contains("128 tests run: 128 passed")
    });
    see(
        &front,
        &[VITE_LINE, "pid 7731"],
        &[TESTS_LINE, "128 tests run: 128 passed"],
    );
    app.key("alt+3");
    let back = app.wait_screen("api tests output again", |screen| {
        let visible = text(screen);
        api_tests_body(screen) && visible.contains("exit 0") && !visible.contains("pid 7731")
    });
    see(&back, &[TESTS_HEAD, "exit 0"], &[VITE_LINE, "pid 7731"]);

    app.key("/");
    app.type_text("aliases_beat");
    let found = app.wait_text("find “aliases_beat” · 1 of 1");
    assert!(text(&found).contains(TESTS_LINE));
    app.key("esc");
    app.wait_screen("find closed", |screen| !text(screen).contains("find “"));

    app.key("ctrl+a");
    app.key("y");
    let copied = app.wait_text("Copied $ cargo nextest run");
    assert!(text(&copied).contains(TESTS_LINE));
    app.key("f");
    let following = app.wait_text("Following the tail");
    assert!(text(&following).contains(TESTS_LINE));
    app.key("f");
    let paused = app.wait_text("Follow paused · End resumes");
    assert!(text(&paused).contains(TESTS_LINE));

    app.key("alt+1");
    let untouched = app.wait_text(VITE_LINE);
    see(
        &untouched,
        &["frontend dev · running", "http://localhost:5173/"],
        &[TESTS_LINE, "128 tests run: 128 passed"],
    );

    app.key("alt+4");
    let logs = app.wait_text("worker ready · queue=orders concurrency=4");
    assert!(text(&logs).contains("GET /health 200"), "{}", text(&logs));
    app.key("1");
    let filtered = app.wait_screen("api stream hidden", |screen| {
        let visible = text(screen);
        visible.contains("worker ready · queue=orders concurrency=4")
            && !visible.contains("GET /health 200")
    });
    assert!(
        text(&filtered).contains("logs · api worker scheduler · running"),
        "{}",
        text(&filtered)
    );
    assert!(!text(&filtered).contains(TESTS_LINE), "{}", text(&filtered));

    app.key("alt+3");
    let still = app.wait_screen("api tests output again", api_tests_body);
    assert!(text(&still).contains(TESTS_LINE));
    app.key("tab");
    app.wait_text("▎Run tests again");
    app.key("enter");
    let started = app.wait_text("test · succeeded · 0 s · exit 0");
    see(
        &started,
        &[
            "$ mise run test",
            "[mise] test finished",
            "started by Run tests",
        ],
        &[TESTS_LINE, VITE_LINE],
    );
    app.key("alt+3");
    let kept = app.wait_screen("api tests kept", |screen| {
        api_tests_body(screen)
            && text(screen).contains("128 tests run: 128 passed")
            && !text(screen).contains("[mise] test finished")
    });
    see(
        &kept,
        &[TESTS_LINE, "128 tests run: 128 passed", "exit 0"],
        &["[mise] test finished", VITE_LINE],
    );
    app.key("ctrl+g");
    let list = app.wait_text("Activities");
    see(
        &list,
        &[
            "api tests",
            "succeeded · project",
            "succeeded · here",
            TESTS_HEAD,
        ],
        &["~/work/batch", "Retry frontend dev", "EADDRINUSE"],
    );
    app.key("esc");
    confirm_quit(&app);
}

#[test]
fn h03_output_scenario_launch_failure() {
    let app = open("h03-output-launch-failure", "launch-failure");
    // The failed header paints before the log lines and the retry action.
    let failed = app.wait_screen("launch failed", |screen| {
        let visible = text(screen);
        visible.contains(FAILED)
            && visible.contains("ELIFECYCLE")
            && visible.contains("Retry frontend dev")
            && visible.contains("Open the mise task definition")
    });
    see(
        &failed,
        &[
            EADDR,
            "ELIFECYCLE",
            "mise ERROR task failed: //apps/frontend:dev exited with 1",
            "Retry frontend dev",
            "Open the mise task definition",
            "started by Start frontend dev",
            "~/work/acme/apps/frontend",
        ],
        &[
            TESTS_LINE,
            "api tests",
            "btm 0.11.1",
            "~/work/batch",
            "Emit a mixed stream",
            "index file corrupt",
        ],
    );
    app.key("tab");
    app.wait_text("▎Find the process on port 5173");
    app.key("enter");
    let port = app.wait_text("Port 5173");
    see(
        &port,
        &[
            "node (stale vite)",
            "5120",
            "*:5173 (LISTEN)",
            "$ lsof -nP -iTCP:5173 -sTCP:LISTEN",
        ],
        &[TESTS_LINE, "~/work/batch"],
    );
    app.key("esc");
    app.wait_text("Search actions");
    app.key("alt+1");
    let back = app.wait_screen("failed output restored", |screen| {
        let visible = text(screen);
        visible.contains(FAILED)
            && visible.contains(EADDR)
            && visible.contains("Retry frontend dev")
            && !visible.contains("5120")
    });
    see(&back, &[EADDR, "exit 1", "Retry frontend dev"], &["5120"]);
    app.finish();
}

#[test]
fn h03_output_scenario_parity_executor() {
    let app = open("h03-output-parity", "parity-executor");
    // The action list paints before the scope path.
    let landing = app.wait_screen("parity landing", |screen| {
        let visible = text(screen);
        visible.contains("Emit a mixed stream")
            && visible.contains("~/work/batch")
            && visible.contains("0 dirty · 1 behind")
    });
    see(
        &landing,
        &[
            "batch · project collection",
            "~/work/batch",
            "Emit a burst",
            "Current folder · trusted",
            "0 dirty · 1 behind",
        ],
        &[
            "frontend dev",
            TESTS_LINE,
            "EADDRINUSE",
            "~/work/acme",
            "Retry frontend dev",
        ],
    );
    app.key("enter");
    let children = app.wait_text("two");
    assert!(row_has(&children, &["one", "main"]), "{}", text(&children));
    assert!(row_has(&children, &["two", "main"]), "{}", text(&children));
    assert!(
        row_has(&children, &["three", "main"]),
        "{}",
        text(&children)
    );
    see(
        &children,
        &["Status across child projects"],
        &["index file corrupt", "frontend dev", TESTS_LINE],
    );
    app.key("esc");
    app.wait_text("Emit a mixed stream");
    app.paste("Emit a mixed stream");
    let preview = app.wait_text("CRLF, ANSI, invalid");
    see(
        &preview,
        &["./emit-stream", "UTF-8 and a final", "fragment"],
        &["frontend dev"],
    );
    app.key("enter");
    let stream = app.wait_screen("stream finished", |screen| {
        let visible = text(screen);
        visible.contains("emit a mixed stream · succeeded")
            && visible.contains("exit 0")
            && visible.contains("last fragment")
            && visible.contains("first")
    });
    see(
        &stream,
        &["green", "started by Emit a mixed stream", "~/work/batch"],
        &["burst line", "frontend dev", TESTS_LINE, "EADDRINUSE"],
    );
    app.key("esc");
    app.wait_text("Esc Clear");
    app.key("esc");
    let cleared = app.wait_screen("query cleared", |screen| {
        text(screen).contains("Emit a burst") && !text(screen).contains("Results · 1")
    });
    assert!(text(&cleared).contains("Emit a mixed stream"));
    app.paste("Emit a burst");
    app.key("enter");
    let burst = app.wait_screen("burst finished", |screen| {
        let visible = text(screen);
        visible.contains("burst line 4499")
            && visible.contains("succeeded")
            && visible.contains("exit 0")
            && !visible.contains("last fragment")
    });
    assert!(
        text(&burst).contains("emit a burst") || text(&burst).contains("Emit a burst"),
        "{}",
        text(&burst)
    );
    see(
        &burst,
        &["~/work/batch"],
        &["frontend dev", VITE_LINE, "first"],
    );
    app.key("alt+1");
    let restored = app.wait_text("last fragment");
    see(
        &restored,
        &[
            "first",
            "green",
            "emit a mixed stream · succeeded",
            "exit 0",
        ],
        &["burst line 4499", "frontend dev"],
    );
    app.finish();
}
