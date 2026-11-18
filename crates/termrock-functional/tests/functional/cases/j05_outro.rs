//! Jackin last-instance outro, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j05_outro/`. Reduced motion paints the 14-minute caption from
//! the first outro frame and holds it until `REDUCED_HOLD` (45 ticks of 33 ms).
//! `App::visual` then waits 700 ms, so a second shot would land after quit.
//! These shots compare the live frame, and they do not nudge. A partial PTY
//! read can show the caption over the capsule, so the shot waits until the
//! pill and Enter Close are up and the capsule is gone. `Launch::new` leaves
//! the clock off. `App::new` would enable it for jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const CAPTION: &str = "You were in the Construct for 2 hours 14 minutes";
const FIFTEEN: &str = "2 hours 15 minutes";
const OUTRO_ANY: &str = "You were in the Construct";
const UNSAVED: &str = "Unsaved work — quit?";
const CLEAN_QUIT: &str = "Quit jackin❯?";
const KEEP: &str = "Quit & keep changes";
const START_AGENT: &str = "Start a new agent";
const STILL: &str = "Still inside the Construct · 1 instance remains";
const WORKSPACE: &str = "payments-platform";
const BRANCH: &str = "feature/settlement-backoff";
const QUIT_HINT: &str = "Ctrl+Q";
const PILL: &str = "jackin❯";
const CLOSE: &str = "Enter Close";
const WIDE: &str = "J05.outro.outro.120x40";
const NARROW: &str = "J05.outro.outro.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j05_outro/{}",
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

fn on_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(WORKSPACE)
        && visible.contains(QUIT_HINT)
        && !visible.contains(UNSAVED)
        && !visible.contains(OUTRO_ANY)
}

fn dirty_quit(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(UNSAVED)
        && visible.contains(START_AGENT)
        && visible.contains(KEEP)
        && !visible.contains(CLEAN_QUIT)
}

fn outro_at(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    assert!(
        !visible.contains(FIFTEEN),
        "outro frame says 2 hours 15 minutes\n{visible}"
    );
    screen.cols() == cols
        && screen.rows() == rows
        && visible.contains(CAPTION)
        && visible.contains(PILL)
        && visible.contains(CLOSE)
        && !visible.contains(UNSAVED)
        && !visible.contains(WORKSPACE)
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let start = Instant::now();
    loop {
        let screen = app.screen();
        if accept(&screen) {
            check_frame(app, &screen, &test_id);
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

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = crate::support::oracle_baselines();
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

fn confirm_keep(app: &App) {
    // Attach covers `Ctrl+Q` for five seconds of sim time. A loaded
    // partition slips that clock past the 10 s screen wait.
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        let screen = app.screen();
        if on_capsule(&screen) {
            break;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "capsule frame not seen\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.key("ctrl+q");
    let dialog = app.wait_screen("unsaved quit", dirty_quit);
    assert!(
        !text(&dialog).contains(CLEAN_QUIT),
        "dirty instance opened the clean quit dialog\n{}",
        text(&dialog)
    );
    app.key("down");
    app.key("down");
    // Two downs can be read before the radio paint. Enter on that frame
    // confirms whichever row is still selected.
    let armed = Instant::now() + Duration::from_secs(8);
    loop {
        let screen = app.screen();
        if keep_armed(&screen) {
            break;
        }
        assert!(
            Instant::now() < armed && app.child_running(),
            "radio move left the unsaved dialog\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.key("enter");
}

fn keep_armed(screen: &TerminalSnapshot) -> bool {
    dirty_quit(screen)
        && text(screen)
            .lines()
            .any(|line| line.contains('▎') && line.contains(KEEP))
}

fn wait_exit(app: &App) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.child_running() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
    app.expect_exited(0);
}

fn skip_outro(app: &App) {
    assert!(
        app.child_running(),
        "outro ended before Enter could skip it"
    );
    app.key("enter");
    wait_exit(app);
}

fn await_outro(app: &App, cols: u32, rows: u32) -> TerminalSnapshot {
    let start = Instant::now();
    loop {
        let screen = app.screen();
        if outro_at(&screen, cols, rows) {
            return screen;
        }
        // Enter is one paint behind the armed radio under a loaded partition.
        // The reduced-motion hold is 45 ticks of 33 ms, so this window still
        // returns while the caption is up.
        assert!(
            start.elapsed() < Duration::from_millis(2_500) && app.child_running(),
            "outro frame not seen\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[test]
fn j05_outro_route() {
    let app = open("j05-outro-route", "outro-last");
    confirm_keep(&app);
    let outro = await_outro(&app, 120, 40);
    assert!(text(&outro).contains(CAPTION), "{}", text(&outro));
    assert!(!text(&outro).contains(CLEAN_QUIT), "{}", text(&outro));
    assert_eq!((outro.cols(), outro.rows()), (120, 40));
    skip_outro(&app);
}

#[test]
fn j05_outro_outro() {
    let app = open("j05-outro-outro", "outro-last");
    confirm_keep(&app);
    shot(&app, WIDE, |screen| outro_at(screen, 120, 40));
    app.resize(80, 24);
    shot(&app, NARROW, |screen| outro_at(screen, 80, 24));
    app.resize(120, 40);
    skip_outro(&app);

    let remain = open("j05-outro-remain", "capsule-multi");
    confirm_keep(&remain);
    let start = Instant::now();
    loop {
        assert!(remain.child_running(), "non-last exit ended the process");
        let screen = remain.screen();
        let visible = text(&screen);
        assert!(
            !visible.contains(OUTRO_ANY),
            "non-last exit showed Outro\n{visible}"
        );
        if visible.contains(STILL) {
            let hold = Instant::now();
            while hold.elapsed() < Duration::from_millis(300) {
                assert!(remain.child_running(), "process exited during still-inside");
                let later = text(&remain.screen());
                assert!(
                    !later.contains(OUTRO_ANY),
                    "outro appeared after the non-last quit\n{later}"
                );
                assert!(
                    later.contains(STILL),
                    "still-inside ended before the quit completed\n{later}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "still inside was not shown\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(15));
    }
}

#[test]
fn j05_outro_scenario_outro_last() {
    let app = open("j05-outro-scenario", "outro-last");
    let capsule = app.wait_screen("outro-last capsule", |screen| {
        let visible = text(screen);
        visible.contains(WORKSPACE) && visible.contains(BRANCH)
    });
    let visible = text(&capsule);
    assert!(visible.contains(WORKSPACE), "{visible}");
    assert!(visible.contains(BRANCH), "{visible}");
}
