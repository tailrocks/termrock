//! Cross-application startup and the first input, before the refactor.
//!
//! Tuiscotti test ids reject `.` and uppercase, so the baseline directory is
//! `pre_refactor/x02_startup/`. The checkpoint session is showcase
//! `--page lists` after the burst. It captures 120×40, resizes to 80×24,
//! captures that frame, then resizes back. `Launch::new` leaves the clock off,
//! including for jackin.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, Point, SHOWCASE, TABLEPRO, row_text, text, try_launch};

const LISTS: &str = "Navigation › Lists";
const CHOSEN: &str = "Chosen: Rust";
const TREES: &str = "Trees";
const SIDEBARS: &str = "Sidebars";
const TABS: &str = "Tabs";
const OPENED: &str = "Navigation › Sidebars";
const BRANCHES: &str = "Branches";
const TASKS: &str = "Tasks";
const KOTLIN: &str = "Kotlin";
const PERSONAL: &str = "Personal";
const LOCAL: &str = "Local PostgreSQL";
const LOCAL_HOST: &str = "localhost:5432";
const SCRATCH_HOST: &str = "~/scratch.db";
const FILTER: &str = "Filter connections";
const DISK: &str = "Analyze disk usage";
const BTM: &str = "Open btm";
const SEARCH: &str = "Search actions";
const WIDE_ID: &str = "pre_refactor/x02_startup/x02_startup_start_120x40";
const NARROW_ID: &str = "pre_refactor/x02_startup/x02_startup_start_narrow";

fn open(name: &str, bin: &str, args: &[&str], env: &[(&str, &str)]) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = args.iter().copied().map(str::to_string).collect();
    for (key, value) in env {
        launch.env.push(((*key).to_string(), (*value).to_string()));
    }
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn gutter_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn wait_gutter(app: &App, needle: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("gutter {needle}"), |screen| {
        gutter_line(screen).contains(needle)
    })
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

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store =
        tuiscotti_latest::check::FrozenStore::new(&root, &artifacts).expect("open frozen store");
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

fn frame_with(app: &App, cols: u32, rows: u32) -> TerminalSnapshot {
    thread::sleep(Duration::from_millis(250));
    app.wait_screen(&format!("{cols}x{rows}"), |screen| {
        screen.cols() == cols
            && screen.rows() == rows
            && text(screen).contains(OPENED)
            && text(screen).contains(BRANCHES)
            && !text(screen).contains(CHOSEN)
    })
}

fn lists_startup(app: &App) -> TerminalSnapshot {
    let screen = wait_gutter(app, "Lists");
    require(&screen, LISTS);
    require(&screen, CHOSEN);
    assert!(
        gutter_line(&screen).contains("Lists"),
        "startup gutter\n{}",
        text(&screen)
    );
    assert!(app.child_running(), "showcase stopped during startup");
    screen
}

fn first_down_moves_to_trees(app: &App) -> TerminalSnapshot {
    app.key("down");
    let moved = wait_gutter(app, TREES);
    require(&moved, LISTS);
    require(&moved, CHOSEN);
    assert!(
        !gutter_line(&moved).contains("Lists"),
        "gutter stayed on Lists\n{}",
        text(&moved)
    );
    assert!(app.child_running(), "first down stopped showcase");
    moved
}

fn showcase_burst() {
    let app = open(
        "x02-startup-checkpoint",
        SHOWCASE,
        &["--page", "lists"],
        &[],
    );
    lists_startup(&app);
    first_down_moves_to_trees(&app);

    app.key("down");
    wait_gutter(&app, SIDEBARS);
    app.key("down");
    wait_gutter(&app, TABS);
    app.key("up");
    let back = wait_gutter(&app, SIDEBARS);
    require(&back, CHOSEN);
    require(&back, LISTS);
    assert!(app.child_running(), "nav keys stopped showcase");

    app.move_to(Point { x: 4, y: 0 });
    let parked = app.wait_screen("pointer parked", |screen| {
        gutter_line(screen).contains(SIDEBARS) && text(screen).contains(CHOSEN)
    });
    require(&parked, LISTS);
    assert!(app.child_running(), "move stopped showcase");

    app.wheel(Point { x: 40, y: 14 }, true, 1);
    let scrolled = app.wait_screen("language list scrolled", |screen| {
        gutter_line(screen).contains(SIDEBARS)
            && gutter_line(screen).contains(KOTLIN)
            && text(screen).contains(CHOSEN)
    });
    require(&scrolled, LISTS);
    assert!(app.child_running(), "wheel stopped showcase");

    app.resize(100, 30);
    let burst = app.wait_screen("burst resize", |screen| {
        screen.cols() == 100
            && screen.rows() == 30
            && gutter_line(screen).contains(SIDEBARS)
            && text(screen).contains(CHOSEN)
    });
    require(&burst, LISTS);
    refuse(&burst, OPENED);
    assert!(app.child_running(), "burst resize stopped showcase");

    app.key("right");
    let opened = app.wait_screen("sidebars opened", |screen| {
        screen.cols() == 100
            && screen.rows() == 30
            && text(screen).contains(OPENED)
            && text(screen).contains(BRANCHES)
            && !text(screen).contains(CHOSEN)
    });
    require(&opened, TASKS);
    assert!(
        app.child_running(),
        "opening Sidebars stopped showcase\n{}",
        text(&opened)
    );

    app.resize(120, 40);
    let wide = frame_with(&app, 120, 40);
    require(&wide, OPENED);
    require(&wide, BRANCHES);
    require(&wide, TASKS);
    refuse(&wide, CHOSEN);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(app.child_running(), "wide frame stopped showcase");
    check_frame(&app, &wide, WIDE_ID);

    app.resize(80, 24);
    let narrow = frame_with(&app, 80, 24);
    require(&narrow, OPENED);
    require(&narrow, BRANCHES);
    refuse(&narrow, CHOSEN);
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    check_frame(&app, &narrow, NARROW_ID);

    app.resize(120, 40);
    let restored = frame_with(&app, 120, 40);
    require(&restored, OPENED);
    refuse(&restored, CHOSEN);
    assert!(app.child_running(), "restore stopped showcase");
    app.finish();
}

fn delay_still_moves(name: &str, delay: Option<&str>) {
    let env: &[(&str, &str)] = match delay {
        Some(value) => &[("JUNIE_TUI_FIRST_POLL_DELAY_MS", value)],
        None => &[],
    };
    let app = open(name, SHOWCASE, &["--page", "lists"], env);
    lists_startup(&app);
    let moved = first_down_moves_to_trees(&app);
    require(&moved, TREES);
    assert!(app.child_running(), "{name} stopped after the first key");
    app.kill_child();
}

fn tablepro_first_key() {
    let app = open("x02-startup-tablepro", TABLEPRO, &[], &[]);
    let start = app.wait_screen("tablepro startup", |screen| {
        text(screen).contains(FILTER)
            && text(screen).contains(LOCAL_HOST)
            && gutter_line(screen).contains(PERSONAL)
    });
    require(&start, LOCAL);
    refuse(&start, SCRATCH_HOST);
    assert!(app.child_running(), "tablepro stopped during startup");
    app.key("down");
    let moved = wait_gutter(&app, LOCAL);
    require(&moved, LOCAL_HOST);
    require(&moved, FILTER);
    refuse(&moved, SCRATCH_HOST);
    assert!(
        !gutter_line(&moved).contains(PERSONAL),
        "gutter stayed on Personal\n{}",
        text(&moved)
    );
    assert!(app.child_running(), "first down stopped tablepro");
    app.kill_child();
}

fn holla_first_key() {
    let app = open(
        "x02-startup-holla",
        HOLLA,
        &["--scenario", "first-use"],
        &[],
    );
    let start = app.wait_screen("holla startup", |screen| {
        text(screen).contains(SEARCH) && selected(screen, DISK) && !selected(screen, BTM)
    });
    require(&start, DISK);
    assert!(app.child_running(), "holla stopped during startup");
    app.key("down");
    let moved = app.wait_screen("holla cursor moved", |screen| {
        selected(screen, BTM) && !selected(screen, DISK) && text(screen).contains(SEARCH)
    });
    require(&moved, DISK);
    require(&moved, BTM);
    assert!(app.child_running(), "first down stopped holla");
    app.kill_child();
}

#[test]
fn x02_startup_route() {
    let app = open("x02-startup-route", SHOWCASE, &["--page", "lists"], &[]);
    let screen = lists_startup(&app);
    require(&screen, LISTS);
    require(&screen, CHOSEN);
    assert!(app.child_running(), "route startup stopped showcase");
    app.finish();
}

#[test]
fn x02_startup_start() {
    tablepro_first_key();
    holla_first_key();
    delay_still_moves("x02-startup-delay", Some("80"));
    delay_still_moves("x02-startup-delay-bad", Some("nope"));
    showcase_burst();
}
