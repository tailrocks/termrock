//! Jackin first-use intro, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j05_intro/`. Reduced motion paints `Knock, knock, operator.`
//! from the first frame and holds it until `REDUCED_HOLD` (45 ticks of 33 ms).
//! `App::visual` then waits 700 ms for a quiet screen, so a second shot would
//! land on the manager. These shots compare the live frame, and they do not
//! nudge. `Launch::new` leaves the clock off. `App::new` would enable it for
//! jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const PHRASE: &str = "Knock, knock, operator.";
const CONTINUE: &str = "Continue";
const DIRECTORY: &str = "Current directory";
const NOT_SAVED: &str = "not saved";
const NO_INSTANCES: &str = "no instances";
const CREATE: &str = "Create a workspace from this directory.";
const ARCHITECT: &str = "the-architect";
const INSTANCE: &str = "7f3a";
const INFRA: &str = "infra-control-plane";
const RUNNING: &str = "2 running";
const COCKPIT: &str = "Resolving launch identity";
const WIDE: &str = "J05.intro.intro.120x40";
const NARROW: &str = "J05.intro.intro.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j05_intro/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn shows_intro(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(PHRASE)
        && visible.contains(CONTINUE)
        && !visible.contains(DIRECTORY)
        && !visible.contains(ARCHITECT)
        && !visible.contains(COCKPIT)
}

fn intro_at(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    screen.cols() == cols && screen.rows() == rows && shows_intro(screen)
}

fn first_use_manager(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DIRECTORY)
        && visible.contains(NOT_SAVED)
        && visible.contains(NO_INSTANCES)
        && visible.contains(CREATE)
        && !visible.contains(PHRASE)
        && !visible.contains(ARCHITECT)
        && !visible.contains(INSTANCE)
        && !visible.contains(INFRA)
        && !visible.contains(COCKPIT)
        && !visible.contains(RUNNING)
}

fn returning_manager(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DIRECTORY)
        && visible.contains(ARCHITECT)
        && visible.contains(RUNNING)
        && visible.contains(INSTANCE)
        && !visible.contains(PHRASE)
        && !visible.contains(NO_INSTANCES)
        && !visible.contains(COCKPIT)
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let start = Instant::now();
    loop {
        let screen = app.screen();
        if accept(&screen) {
            crate::support::assert_frame(app, &screen, &test_id);
            return;
        }
        assert!(
            start.elapsed() < Duration::from_millis(900) && app.child_running(),
            "{test_id} frame not seen\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn skip_into_manager(app: &App, seen: Instant) -> TerminalSnapshot {
    let rest = Duration::from_millis(450).saturating_sub(seen.elapsed());
    std::thread::sleep(rest);
    let screen = app.screen();
    assert!(
        shows_intro(&screen),
        "intro ended before Enter could skip it\n{}",
        text(&screen)
    );
    let started = Instant::now();
    app.key("enter");
    let manager = app.wait_screen("manager after enter", first_use_manager);
    // Reduced motion holds the intro for 45 ticks of 33 ms. Enter at 450 ms
    // still leaves more than 600 ms of that hold, so a manager in this window
    // is the skip. A loaded partition paints it after the old 400 ms bound.
    assert!(
        started.elapsed() < Duration::from_millis(600),
        "Enter did not skip the intro after {} ms\n{}",
        started.elapsed().as_millis(),
        text(&manager)
    );
    manager
}

fn wait_abort(app: &App) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.child_running() && Instant::now() < deadline {
        let screen = app.screen();
        let visible = text(&screen);
        assert!(
            !visible.contains(DIRECTORY)
                && !visible.contains(COCKPIT)
                && !visible.contains(ARCHITECT),
            "abort opened a join screen\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    app.expect_exited(0);
}

#[test]
fn j05_intro_route() {
    let app = open("j05-intro-route", "first-use");
    let opened = app.wait_screen("intro", |screen| intro_at(screen, 120, 40));
    let seen = Instant::now();
    assert!(text(&opened).contains(PHRASE), "{}", text(&opened));
    assert!(text(&opened).contains(CONTINUE), "{}", text(&opened));
    assert!(!text(&opened).contains(DIRECTORY), "{}", text(&opened));
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    let manager = skip_into_manager(&app, seen);
    assert!(first_use_manager(&manager), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    app.finish();
}

#[test]
fn j05_intro_intro() {
    // The exact three-tick input guard is tested with injected Input::Tick in
    // App unit tests; PTY startup can consume those ticks before a queued key
    // is delivered, so this process-level test covers the visible intro path.
    let app = open("j05-intro-intro", "first-use");
    let wide = app.wait_screen("intro 120x40", |screen| intro_at(screen, 120, 40));
    let seen = Instant::now();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(text(&wide).contains(PHRASE), "{}", text(&wide));
    shot(&app, WIDE, |screen| intro_at(screen, 120, 40));
    app.resize(80, 24);
    let narrow = app.wait_screen("intro 80x24", |screen| intro_at(screen, 80, 24));
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert!(text(&narrow).contains(PHRASE), "{}", text(&narrow));
    shot(&app, NARROW, |screen| intro_at(screen, 80, 24));
    app.resize(120, 40);
    let restored = app.wait_screen("intro 120x40 again", |screen| intro_at(screen, 120, 40));
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    let manager = skip_into_manager(&app, seen);
    assert!(first_use_manager(&manager), "{}", text(&manager));
    app.finish();

    let abort = open("j05-intro-abort", "first-use");
    abort.wait_screen("intro before abort", |screen| intro_at(screen, 120, 40));
    std::thread::sleep(Duration::from_millis(450));
    let still = abort.screen();
    assert!(
        shows_intro(&still),
        "ctrl+c would not be leaving the intro\n{}",
        text(&still)
    );
    abort.key("ctrl+c");
    wait_abort(&abort);

    let returning = open("j05-intro-returning", "returning");
    let joined = returning.wait_screen("returning manager", returning_manager);
    assert!(
        !text(&joined).contains(PHRASE),
        "returning played the empty-discovery intro\n{}",
        text(&joined)
    );
    assert_eq!((joined.cols(), joined.rows()), (120, 40));
    returning.finish();
}

#[test]
fn j05_intro_scenario_first_use() {
    let app = open("j05-intro-first-use", "first-use");
    let intro = app.wait_screen("first-use intro", |screen| intro_at(screen, 120, 40));
    let seen = Instant::now();
    let visible = text(&intro);
    assert!(visible.contains(PHRASE), "{visible}");
    assert!(!visible.contains(ARCHITECT), "{visible}");
    assert!(!visible.contains(INSTANCE), "{visible}");
    assert!(!visible.contains(INFRA), "{visible}");
    assert!(!visible.contains(RUNNING), "{visible}");
    let manager = skip_into_manager(&app, seen);
    let visible = text(&manager);
    assert!(visible.contains(NOT_SAVED), "{visible}");
    assert!(visible.contains(NO_INSTANCES), "{visible}");
    assert!(visible.contains(CREATE), "{visible}");
    assert!(!visible.contains(ARCHITECT), "{visible}");
    assert!(!visible.contains(INSTANCE), "{visible}");
    assert!(!visible.contains(INFRA), "{visible}");
    assert!(!visible.contains(RUNNING), "{visible}");
    app.finish();
}
