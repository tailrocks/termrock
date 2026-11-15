//! Cross-application motion, before the refactor.
//!
//! `Motion::Full` ticks and animates. `Motion::Reduced` ticks and does not
//! animate. `Motion::Paused` never advances ticks; `--frame N` selects the
//! frame (`src/motion.rs`). An explicit `--motion` wins over the app
//! `NO_MOTION` env. These launches do not set that env.
//!
//! Showcase `with_motion` starts `tick` at `frame`. While paused, startup
//! runs each page `on_tick` `frame` times, and `on_tick` returns before the
//! tick increments (`src/bin/showcase/app.rs`). The tick increments only
//! when the page is animating and `motion.animates()`.
//!
//! The Status & hints playground paints `render_spinner` and a progress bar
//! from `job`. `job` starts as `None` (`Indexing idle`). `Run job` sets it
//! to `Some(0.0)`. Page ticks advance it while it is below 1.
//! `spinner_frame` indexes the Unicode frames (`src/widgets/progress.rs`).
//!
//! Jackin is the only `advance_clock` override. It calls `clock::run_step`
//! (`src/bin/jackin/app.rs`). The paste is a clock step only when
//! `clock_steps` is set and motion is paused (`src/shell/app.rs`).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, SHOWCASE, TABLEPRO, find, row_text, text, try_launch};

const TITLE: &str = "Status & hints";
const BLURB: &str = "The footer hint bar";
const BLURB_WIDE: &str = "priorities, statuses, badges";
const MOTION_NOTE: &str = "full: the spinner turns";
const IDLE: &str = "Indexing idle";
const RUN: &str = "Run job";
const FRAME_0: &str = "⠋";
const FRAME_4: &str = "⠼";
const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

const WIDE_ID: &str = "pre_refactor/x03_motion/x03_motion_motion_120x40";
const NARROW_ID: &str = "pre_refactor/x03_motion/x03_motion_motion_narrow";

fn open(name: &str, bin: &str, args: &[&str], motion: &str, clock: bool) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = clock;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    launch.args = args.iter().copied().map(str::to_string).collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn kill(app: &App) {
    assert!(app.child_running(), "child exited before the assertion");
    app.kill_child();
    assert!(!app.child_running(), "child still running");
}

fn row_containing(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle}\n{}", text(screen)))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn spinner_marks(screen: &TerminalSnapshot) -> Vec<String> {
    let mut marks = Vec::new();
    for y in 0..screen.rows() {
        for cell in screen.row(y).expect("row") {
            if SPINNER.contains(&cell.text()) {
                marks.push(cell.text().to_string());
            }
        }
    }
    marks
}

fn stable(app: &App, reason: &str, ready: impl Fn(&TerminalSnapshot) -> bool) -> TerminalSnapshot {
    let mut screen = app.wait_screen(reason, &ready);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        std::thread::sleep(Duration::from_millis(150));
        let next = app.screen();
        if ready(&next) && next == screen {
            return next;
        }
        screen = next;
        assert!(
            Instant::now() < deadline,
            "frame kept changing: {reason}\n{}",
            text(&screen)
        );
    }
}

fn opened_hints(app: &App) -> TerminalSnapshot {
    app.wait_screen("hints header", |screen| {
        let body = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && body.contains(TITLE)
            && body.contains(BLURB_WIDE)
    })
}

fn reveal_idle(app: &App, glyph: &str, must_scroll: bool) -> TerminalSnapshot {
    let glyph = glyph.to_string();
    let opened = opened_hints(app);
    if !text(&opened).contains(IDLE) {
        let anchor = find(&opened, "Layers");
        app.wheel(anchor, true, 2);
    } else if must_scroll {
        panic!("indexing row was already on screen\n{}", text(&opened));
    }
    stable(app, "idle frame", move |screen| {
        let body = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && body.contains(TITLE)
            && body.contains(IDLE)
            && body.contains(MOTION_NOTE)
            && spinner_marks(screen) == [glyph.clone()]
    })
}

fn narrow_frame(app: &App) -> TerminalSnapshot {
    app.resize(80, 24);
    stable(app, "narrow hints", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    })
}

fn sample_bytes(screen: &TerminalSnapshot, test_id: &str) -> Vec<u8> {
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    sample.to_bytes().expect("serialize Tuiscotti sample")
}

fn check_stored(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let bytes = sample_bytes(screen, test_id);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
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

fn route() {
    let app = open(
        "x03-motion-route",
        SHOWCASE,
        &["--page", "hints", "--frame", "0"],
        "paused",
        false,
    );
    let page = app.wait_text(BLURB);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    require(&page, TITLE);
    let header = row_containing(&page, BLURB);
    assert!(
        header.contains(TITLE),
        "page identity is not {TITLE}: {header}"
    );
    assert!(
        !header.contains("Text areas"),
        "page identity used Text areas: {header}"
    );
    assert!(
        !header.contains("Splits & stacks"),
        "page identity used Splits & stacks: {header}"
    );
    assert!(
        app.child_running(),
        "the status hints page exited before Ctrl+Q"
    );
    app.finish();
}

fn frame_select() {
    let app = open(
        "x03-motion-frame",
        SHOWCASE,
        &["--page", "hints", "--frame", "4"],
        "paused",
        false,
    );
    let screen = reveal_idle(&app, FRAME_4, true);
    assert_ne!(
        spinner_marks(&screen),
        vec![FRAME_0.to_string()],
        "frame 4 reused the frame 0 glyph"
    );
    kill(&app);
}

fn reduced() {
    let app = open(
        "x03-motion-reduced",
        SHOWCASE,
        &["--page", "hints", "--frame", "0"],
        "reduced",
        false,
    );
    let idle = reveal_idle(&app, FRAME_0, true);
    assert_eq!(spinner_marks(&idle), vec![FRAME_0.to_string()]);
    app.click_text(RUN);
    app.key("enter");
    let first = app.wait_screen("reduced fill", |screen| {
        let body = text(screen);
        body.contains("Indexing")
            && !body.contains(IDLE)
            && spinner_marks(screen) == [FRAME_0.to_string()]
    });
    let first_row = row_containing(&first, "Indexing");
    let _moved = app.wait_screen("reduced fill moved", |screen| {
        spinner_marks(screen) == [FRAME_0.to_string()]
            && (0..screen.rows())
                .map(|row| row_text(screen, row))
                .any(|row| row.contains("Indexing") && row != first_row)
    });
    kill(&app);
}

fn full_turns() {
    let app = open(
        "x03-motion-full",
        SHOWCASE,
        &["--page", "hints", "--frame", "0"],
        "full",
        false,
    );
    let _idle = reveal_idle(&app, FRAME_0, true);
    app.click_text(RUN);
    app.key("enter");
    let mut seen = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        let marks = spinner_marks(&app.screen());
        if marks.len() == 1 && !seen.contains(&marks[0]) {
            seen.push(marks[0].clone());
        }
        if seen.len() >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    assert!(
        seen.len() >= 2,
        "full motion spinner stayed {seen:?}; on_tick advances while animating"
    );
    kill(&app);
}

fn tablepro_rest() {
    let paused = open("x03-motion-tablepro", TABLEPRO, &[], "paused", false);
    let screen = stable(&paused, "tablepro", |screen| {
        text(screen).contains("Filter connections") && spinner_marks(screen).is_empty()
    });
    require(&screen, "TablePro");
    kill(&paused);
    let reduced = open(
        "x03-motion-tablepro-reduced",
        TABLEPRO,
        &[],
        "reduced",
        false,
    );
    let other = reduced.wait_text("Filter connections");
    assert!(spinner_marks(&other).is_empty(), "tablepro reduced spinner");
    assert_eq!(text(&screen), text(&other), "tablepro resting text changed");
    kill(&reduced);
}

fn holla_sources() {
    let paused = open(
        "x03-motion-holla",
        HOLLA,
        &["--scenario", "first-use"],
        "paused",
        false,
    );
    let screen = stable(&paused, "holla paused", |screen| {
        text(screen).contains("Search actions")
            && text(screen).contains("0 of 6")
            && spinner_marks(screen) == [FRAME_0.to_string()]
    });
    require(&screen, "discovering");
    kill(&paused);
    let reduced = open(
        "x03-motion-holla-reduced",
        HOLLA,
        &["--scenario", "first-use"],
        "reduced",
        false,
    );
    let other = reduced.wait_screen("holla reduced", |screen| {
        text(screen).contains("Search actions") && spinner_marks(screen).is_empty()
    });
    assert!(
        !text(&other).contains("0 of 6"),
        "reduced holla still discovering\n{}",
        text(&other)
    );
    kill(&reduced);
}

fn checkpoint() {
    let app = open(
        "x03-motion-paused",
        SHOWCASE,
        &["--page", "hints", "--frame", "0"],
        "paused",
        false,
    );
    let wide = reveal_idle(&app, FRAME_0, true);
    let narrow = narrow_frame(&app);
    app.resize(120, 40);
    let _back = reveal_idle(&app, FRAME_0, false);
    assert!(app.child_running(), "status hints exited before Ctrl+Q");
    app.finish();
    check_stored(&app, &wide, WIDE_ID);
    check_stored(&app, &narrow, NARROW_ID);
}

#[test]
fn x03_motion_route() {
    route();
}

#[test]
fn x03_motion_motion() {
    frame_select();
    reduced();
    full_turns();
    tablepro_rest();
    holla_sources();
    checkpoint();
}
