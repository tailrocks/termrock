//! Jackin cockpit build log, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j05_logs/`. Reduced motion steps the launch three ticks per
//! frame, and the activity spinner changes on every one of those frames. The
//! checkpoint is the first completed log frame (`⠏ Mounting the Workspace`,
//! `5/11 stages`) and, after the resize, `⠋ Waiting for the Capsule daemon`.
//! `Launch::new` leaves the clock off. `App::new` would enable it for jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const EARLY: &str = "No build output yet · the derived image stage has not started";
const COPIED: &str = "Copied ";
const OSC_REFUSED: &str = "Not copied · this terminal does not accept OSC 52";
const ABORT: &str = "Aborted · terminal restored, cleanup deferred";
const DOCKER: &str = "Docker build";
const EXPORTING: &str = "exporting to image";
const FOLLOWING: &str = "44 lines · following";
const PLACEHOLDER: &str = "(waiting for docker build output";
const QUIT_TITLE: &str = "Quit jackin❯?";
const QUIT_BODY: &str = "Quitting force-stops the launch immediately";
const HARDLINE: &str = "Hardline open";
const WIDE_PIN: &str = "⠏ Mounting the Workspace";
const NARROW_PIN: &str = "⠋ Waiting for the Capsule daemon";
const WIDE: &str = "J05.logs.logs.120x40";
const NARROW: &str = "J05.logs.logs.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j05_logs/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "launch-running".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn poll_for(
    app: &App,
    limit: Duration,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> Option<TerminalSnapshot> {
    let start = Instant::now();
    while start.elapsed() < limit && app.child_running() {
        let screen = app.screen();
        if accept(&screen) {
            return Some(screen);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

fn before_build(visible: &str) -> bool {
    !visible.contains("Dockerfile")
        && !visible.contains("lines · b build log")
        && !visible.contains(DOCKER)
        && (visible.contains("Resolving launch identity")
            || visible.contains("Loading role manifest")
            || visible.contains("Resolving credentials")
            || visible.contains("Preparing the Construct image")
            || visible.contains("Installing Claude Code binaries"))
}

fn log_ready(visible: &str) -> bool {
    visible.contains(DOCKER)
        && !visible.contains("Docker build · building")
        && visible.contains(EXPORTING)
        && visible.contains(FOLLOWING)
        && !visible.contains(QUIT_TITLE)
        && !visible.contains("Aborted")
        && !visible.contains("Launch failed")
        && !visible.contains(HARDLINE)
}

fn assert_checkpoint(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(visible.contains(DOCKER), "{visible}");
    assert!(visible.contains(EXPORTING), "{visible}");
    assert!(visible.contains(FOLLOWING), "{visible}");
    assert!(!visible.contains(QUIT_TITLE), "{visible}");
    assert!(!visible.contains("Aborted"), "{visible}");
    assert!(!visible.contains("Launch failed"), "{visible}");
    assert!(!visible.contains(HARDLINE), "{visible}");
}

fn refuse_before_output(app: &App) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) && app.child_running() {
        let visible = text(&app.screen());
        if before_build(&visible) {
            app.key("b");
            let Some(shown) = poll_for(app, Duration::from_millis(500), |screen| {
                let visible = text(screen);
                visible.contains(EARLY)
                    || visible.contains(DOCKER)
                    || visible.contains(PLACEHOLDER)
                    || visible.contains("Dockerfile")
            }) else {
                return false;
            };
            let visible = text(&shown);
            assert!(
                visible.contains(EARLY) && !visible.contains(DOCKER),
                "b before the derived image stage opened the log\n{visible}"
            );
            assert!(
                !visible.contains(PLACEHOLDER),
                "b before the derived image stage opened an empty log\n{visible}"
            );
            return true;
        }
        if visible.contains("Dockerfile")
            || visible.contains("lines · b build log")
            || visible.contains("Building derived")
        {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

fn open_when_line(app: &App) -> Option<TerminalSnapshot> {
    poll_for(app, Duration::from_secs(4), |screen| {
        text(screen).contains("lines · b build log")
    })?;
    app.key("b");
    poll_for(app, Duration::from_millis(400), |screen| {
        text(screen).contains(DOCKER)
    })
}

fn close_with_each_key(app: &App) {
    let opened_screen = open_when_line(app)
        .unwrap_or_else(|| panic!("build log never opened\n{}", text(&app.screen())));
    let opened = text(&opened_screen);
    assert!(
        opened.contains("Dockerfile.derived"),
        "open log is missing the derived dockerfile line\n{opened}"
    );
    assert!(
        !opened.contains(PLACEHOLDER),
        "open log still shows the empty placeholder\n{opened}"
    );

    app.key("escape");
    let closed = poll_for(app, Duration::from_millis(400), |screen| {
        let visible = text(screen);
        !visible.contains(DOCKER) && visible.contains("the-architect")
    })
    .unwrap_or_else(|| panic!("Esc left the build log open\n{}", text(&app.screen())));
    assert!(!text(&closed).contains(DOCKER), "{}", text(&closed));

    app.key("b");
    assert!(
        poll_for(app, Duration::from_millis(400), |screen| {
            text(screen).contains(DOCKER)
        })
        .is_some(),
        "b did not reopen the build log\n{}",
        text(&app.screen())
    );
    app.key("q");
    assert!(
        poll_for(app, Duration::from_millis(400), |screen| {
            !text(screen).contains(DOCKER)
        })
        .is_some(),
        "q left the build log open\n{}",
        text(&app.screen())
    );

    app.key("b");
    assert!(
        poll_for(app, Duration::from_millis(400), |screen| {
            text(screen).contains(DOCKER)
        })
        .is_some(),
        "b did not reopen the build log\n{}",
        text(&app.screen())
    );
    app.key("b");
    assert!(
        poll_for(app, Duration::from_millis(400), |screen| {
            !text(screen).contains(DOCKER)
        })
        .is_some(),
        "b left the build log open\n{}",
        text(&app.screen())
    );
}

fn abort_to_manager(app: &App) {
    app.key("ctrl+c");
    // The cockpit already names b5df "failed setup" while the image is still
    // building. "Launch failed" is the manager line after Ctrl+C.
    let manager = poll_for(app, Duration::from_secs(8), |screen| {
        let visible = text(screen);
        visible.contains("Launch failed")
            && visible.contains("Current directory")
            && visible.contains("failed setup")
            && visible.contains("b5df")
            && !visible.contains(DOCKER)
            && !visible.contains(HARDLINE)
            && !visible.contains(ABORT)
    })
    .unwrap_or_else(|| panic!("Ctrl+C did not leave the cockpit\n{}", text(&app.screen())));
    let visible = text(&manager);
    assert!(visible.contains("Launch failed"), "{visible}");
    assert!(visible.contains("Current directory"), "{visible}");
    assert!(!visible.contains(DOCKER), "{visible}");
    assert!(!visible.contains("Loading the-architect"), "{visible}");
    assert!(!visible.contains(HARDLINE), "{visible}");
    assert!(!visible.contains(ABORT), "{visible}");
}

fn wait_pin(app: &App, pin: &str, cols: u32, rows: u32) -> Option<TerminalSnapshot> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) && app.child_running() {
        let screen = app.screen();
        let visible = text(&screen);
        if visible.contains(HARDLINE) || visible.contains("Opening the hardline") {
            return None;
        }
        if screen.cols() == cols
            && screen.rows() == rows
            && log_ready(&visible)
            && visible.contains(pin)
        {
            return Some(screen);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    None
}

fn copy_open_log() {
    // The narrow checkpoint is a few ticks before reduced-motion handoff.
    // Ctrl+C on that process can be delivered after `Go::Attach`. Copy on the
    // first completed log frame, which is the same open log and still early.
    for attempt in 1..=3 {
        let app = open(&format!("j05-logs-copy-{attempt}"));
        if open_when_line(&app).is_none() {
            app.kill_child();
            continue;
        }
        let Some(open_log) = wait_pin(&app, WIDE_PIN, 120, 40) else {
            app.kill_child();
            continue;
        };
        assert_checkpoint(&open_log);
        app.key("ctrl+a");
        app.key("ctrl+c");
        let copied = poll_for(&app, Duration::from_millis(500), |screen| {
            let visible = text(screen);
            visible.contains(COPIED) || visible.contains(OSC_REFUSED)
        });
        let Some(copied) = copied else {
            app.kill_child();
            continue;
        };
        let visible = text(&copied);
        assert!(
            visible.contains(COPIED) || visible.contains(OSC_REFUSED),
            "{visible}"
        );
        assert!(visible.contains(DOCKER), "{visible}");
        assert!(!visible.contains(ABORT), "{visible}");
        assert!(!visible.contains(HARDLINE), "{visible}");
        app.kill_child();
        return;
    }
    panic!("Ctrl+C did not copy the open build log");
}

fn quit_while_open() {
    let app = open("j05-logs-quit");
    assert!(
        open_when_line(&app).is_some(),
        "quit dialog had no open build log\n{}",
        text(&app.screen())
    );
    app.key("ctrl+q");
    let dialog = poll_for(&app, Duration::from_secs(2), |screen| {
        let visible = text(screen);
        visible.contains(QUIT_TITLE)
            && visible.contains(QUIT_BODY)
            && visible.contains("▎Cancel")
            && !visible.contains("▎Quit")
    })
    .unwrap_or_else(|| panic!("Ctrl+Q did not open quit\n{}", text(&app.screen())));
    let visible = text(&dialog);
    assert!(visible.contains(QUIT_BODY), "{visible}");
    assert!(visible.contains("Cancel"), "{visible}");
    assert!(visible.contains("Quit"), "{visible}");
    assert!(visible.contains("▎Cancel"), "{visible}");
    assert!(!visible.contains("▎Quit"), "{visible}");
    app.key("enter");
    let stayed = poll_for(&app, Duration::from_millis(500), |screen| {
        let visible = text(screen);
        !visible.contains(QUIT_TITLE) && visible.contains(DOCKER)
    })
    .unwrap_or_else(|| panic!("Cancel left the cockpit\n{}", text(&app.screen())));
    assert!(app.child_running(), "Cancel stopped the process");
    assert!(text(&stayed).contains(DOCKER), "{}", text(&stayed));
    assert!(!text(&stayed).contains(HARDLINE), "{}", text(&stayed));

    app.key("ctrl+q");
    assert!(
        poll_for(&app, Duration::from_millis(500), |screen| {
            text(screen).contains(QUIT_TITLE)
        })
        .is_some(),
        "Ctrl+Q did not reopen quit\n{}",
        text(&app.screen())
    );
    app.key("right");
    let focused = poll_for(&app, Duration::from_millis(400), |screen| {
        text(screen).contains("▎Quit")
    })
    .unwrap_or_else(|| panic!("Quit did not take focus\n{}", text(&app.screen())));
    assert!(text(&focused).contains("Quit"), "{}", text(&focused));
    app.key("enter");
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.child_running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let last = text(&app.screen());
    assert!(!last.contains(HARDLINE), "{last}");
    app.expect_exited(0);
}

#[test]
fn j05_logs_route() {
    for attempt in 1..=3 {
        let app = open(&format!("j05-logs-route-{attempt}"));
        if !refuse_before_output(&app) {
            app.kill_child();
            continue;
        }
        close_with_each_key(&app);
        abort_to_manager(&app);
        app.kill_child();
        return;
    }
    panic!("b before a build line was not observed");
}

#[test]
fn j05_logs_logs() {
    for attempt in 1..=3 {
        let app = open(&format!("j05-logs-logs-{attempt}"));
        if open_when_line(&app).is_none() {
            app.kill_child();
            continue;
        }
        let Some(wide) = wait_pin(&app, WIDE_PIN, 120, 40) else {
            app.kill_child();
            continue;
        };
        assert!(text(&wide).contains("5/11 stages"), "{}", text(&wide));
        assert_checkpoint(&wide);
        crate::support::assert_frame(&app, &wide, &visual_id(WIDE));
        app.resize(80, 24);
        let Some(narrow) = wait_pin(&app, NARROW_PIN, 80, 24) else {
            app.kill_child();
            continue;
        };
        assert_checkpoint(&narrow);
        crate::support::assert_frame(&app, &narrow, &visual_id(NARROW));
        app.resize(120, 40);
        app.kill_child();
        copy_open_log();
        quit_while_open();
        return;
    }
    panic!("completed build log frame was not captured");
}
