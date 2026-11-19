//! Jackin cockpit-to-capsule handoff, driven through the real binary.
//!
//! `--motion reduced` makes `Go::Attach` call `finish_handoff` in the same
//! turn, so the checkpoints are the capsule, not a fade frame. The capsule
//! keeps ticking, and each capture stamps new snapshot metadata, so these
//! shots compare the live frame after the blocked prompt has held.
//! `Launch::new` leaves the clock off. `App::new` would enable it for jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const ARCHITECT: &str = "payments-platform › the-architect";
const TAB: &str = "1 Claude Code (Work)";
const PROMPT: &str = "Allow edit to src/settlement/config.rs";
const BRANCH: &str = "feature/settlement-backoff";
const NEEDS_INPUT: &str = "needs input";
const HANDING_OFF: &str = "Hardline open · handing off to the Capsule";
const CONTAINER: &str = "jackin-payments-platform-b5df";
const INSTANCE: &str = "b5df";
const SECOND: &str = "b5e0";
const RUNNING: &str = "3 running";
const FOUR: &str = "4 running";
const ROW: &str = "b5df · payments-platform";
const INVENTORY: &str = "3 instances";
const WIDE: &str = "J05.handoff.handoff.120x40";
const NARROW: &str = "J05.handoff.handoff.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j05_handoff/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, motion: &str, frame: Option<u64>) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "launch-running".to_string()];
    if let Some(frame) = frame {
        launch.args.push("--frame".to_string());
        launch.args.push(frame.to_string());
    }
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn wide_chrome(visible: &str) -> bool {
    visible.contains(ARCHITECT)
        && visible.contains(TAB)
        && visible.contains(PROMPT)
        && visible.contains(NEEDS_INPUT)
        && visible.contains(BRANCH)
        && absent(visible)
}

fn narrow_chrome(visible: &str) -> bool {
    visible.contains(TAB) && visible.contains(PROMPT) && visible.contains(BRANCH) && absent(visible)
}

fn absent(visible: &str) -> bool {
    !visible.contains(SECOND)
        && !visible.contains("handing off")
        && !visible.contains("Launch failed")
        && !visible.contains(FOUR)
        && !visible.contains("2 Claude")
}

fn at(screen: &TerminalSnapshot, cols: u32, rows: u32, chrome: impl Fn(&str) -> bool) -> bool {
    screen.cols() == cols && screen.rows() == rows && chrome(&text(screen))
}

fn held(
    app: &App,
    accept: impl Fn(&TerminalSnapshot) -> bool,
    limit: Duration,
) -> TerminalSnapshot {
    let start = Instant::now();
    let mut last = String::new();
    let mut since = Instant::now();
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let visible = text(&screen);
            if visible == last {
                if since.elapsed() >= Duration::from_millis(700) {
                    return screen;
                }
            } else {
                last = visible;
                since = Instant::now();
            }
        }
        assert!(
            start.elapsed() < limit && app.child_running(),
            "handoff frame not held\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn parked(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(HANDING_OFF)
        && visible.contains(CONTAINER)
        && visible.contains(RUNNING)
        && visible.contains("all stages complete")
        && !visible.contains(SECOND)
        && !visible.contains(FOUR)
        && !visible.contains(PROMPT)
        && !visible.contains("Launch failed")
}

fn paused_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(TAB)
        && visible.contains(ARCHITECT)
        && visible.contains("working")
        && !visible.contains("handing off")
        && !visible.contains(PROMPT)
        && !visible.contains(SECOND)
        && !visible.contains("2 Claude")
}

fn finished_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(TAB)
        && visible.contains(ARCHITECT)
        && visible.contains(BRANCH)
        && !visible.contains("handing off")
        && absent(&visible)
}

fn one_instance(app: &App) {
    app.key("ctrl+b");
    app.wait_screen("pane prefix", |screen| text(screen).contains("prefix…"));
    app.key("d");
    let manager = app.wait_screen("detached inventory", |screen| {
        let visible = text(screen);
        visible.contains(RUNNING)
            && visible.contains(ROW)
            && visible.contains(CONTAINER)
            && visible.contains(INVENTORY)
    });
    let visible = text(&manager);
    assert!(visible.contains(INSTANCE), "{visible}");
    assert!(!visible.contains(SECOND), "{visible}");
    assert!(!visible.contains(FOUR), "{visible}");
    assert!(!visible.contains("4 instances"), "{visible}");
    app.kill_child();
}

fn skip(name: &str, key: &str) {
    let app = open(name, "paused", Some(299));
    let parked = app.wait_screen("paused handoff", parked);
    let visible = text(&parked);
    assert!(visible.contains(HANDING_OFF), "{visible}");
    assert!(visible.contains(CONTAINER), "{visible}");
    assert!(visible.contains(RUNNING), "{visible}");
    assert!(!visible.contains(SECOND), "{visible}");
    assert!(!visible.contains(FOUR), "{visible}");
    app.key(key);
    let capsule = app.wait_screen("capsule after skip", paused_capsule);
    let visible = text(&capsule);
    assert!(visible.contains(TAB), "{visible}");
    assert!(!visible.contains("handing off"), "{visible}");
    assert!(!visible.contains(SECOND), "{visible}");
    one_instance(&app);
}

#[test]
fn j05_handoff_route() {
    skip("j05-handoff-enter", "enter");
    skip("j05-handoff-esc", "esc");

    let app = open("j05-handoff-full", "full", None);
    let start = Instant::now();
    loop {
        let screen = app.screen();
        if finished_capsule(&screen) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(25) && app.child_running(),
            "full handoff did not reach the capsule\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(40));
    }
    assert!(
        app.try_sustained(Duration::from_millis(500), finished_capsule),
        "full handoff left the capsule\n{}",
        text(&app.screen())
    );
    one_instance(&app);
}

#[test]
fn j05_handoff_handoff() {
    let app = open("j05-handoff-handoff", "reduced", None);
    let wide = held(
        &app,
        |screen| at(screen, 120, 40, wide_chrome),
        Duration::from_secs(36),
    );
    let visible = text(&wide);
    assert!(visible.contains(ARCHITECT), "{visible}");
    assert!(visible.contains(TAB), "{visible}");
    assert!(visible.contains(PROMPT), "{visible}");
    assert!(visible.contains(NEEDS_INPUT), "{visible}");
    assert!(visible.contains(BRANCH), "{visible}");
    assert!(absent(&visible), "{visible}");
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    crate::support::assert_frame(&app, &wide, &visual_id(WIDE));
    app.resize(80, 24);
    let narrow = held(
        &app,
        |screen| at(screen, 80, 24, narrow_chrome),
        Duration::from_secs(8),
    );
    let visible = text(&narrow);
    assert!(visible.contains(TAB), "{visible}");
    assert!(visible.contains(PROMPT), "{visible}");
    assert!(visible.contains(BRANCH), "{visible}");
    assert!(absent(&visible), "{visible}");
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    crate::support::assert_frame(&app, &narrow, &visual_id(NARROW));
    app.resize(120, 40);
    let restored = held(
        &app,
        |screen| at(screen, 120, 40, wide_chrome),
        Duration::from_secs(8),
    );
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    one_instance(&app);
}
