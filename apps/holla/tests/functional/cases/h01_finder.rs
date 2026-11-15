//! Holla finder discovery, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h01_finder/` and each checkpoint's dots become underscores.
//! Reduced motion sets every source `done_at` to 0 (`src/bin/holla/domain/parity.rs`),
//! so pending, ready, and failed exist only as earlier ticks. `--frame` renders
//! one of those ticks with `--motion paused` (`src/bin/holla/main.rs`).

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const WIDE: &str = "H01.finder.discovery.120x40";
const NARROW: &str = "H01.finder.discovery.narrow";
const KEPT: &str = "Clean IntelliJ";
const PULL: &str = "Pull";
const TESTS: &str = "Run tests";
const DISCOVERING: &str = "discovering";
const UNAVAILABLE: &str = "! docker, github unavailable";
const NO_MATCH: &str = "No matches for";
const QUERY: &str = "zzzz-no-such-action";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h01_finder/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str, motion: &str, frame: Option<u64>) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    if let Some(frame) = frame {
        launch.args.push("--frame".to_string());
        launch.args.push(frame.to_string());
    }
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions and resources");
    app
}

fn open(name: &str, scenario: &str) -> App {
    launch(name, scenario, "reduced", None)
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

fn line_with(screen: &TerminalSnapshot, needle: &str) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or("")
        .to_string()
}

fn selected(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains('›') && !line.contains("scope ‹"))
        .unwrap_or("")
        .to_string()
}

fn query_line(screen: &TerminalSnapshot) -> String {
    line_with(screen, "scope ‹")
}

fn assert_kept(screen: &TerminalSnapshot) {
    assert!(
        selected(screen).contains(KEPT),
        "selected row changed\n{}",
        text(screen)
    );
}

fn open_surface(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("Search actions and resources")
        && visible.contains(PULL)
        && visible.contains(UNAVAILABLE)
        && !visible.contains(DISCOVERING)
        && selected(screen).contains(PULL)
}

#[test]
fn finder_route_reaches_the_finder() {
    let app = open("h01-finder-route", "first-use");
    let finder = app.screen();
    assert_eq!((finder.cols(), finder.rows()), (120, 40));
    see(
        &finder,
        &[
            "Here",
            "Search actions and resources",
            "~/scratch",
            "no project",
        ],
        &[UNAVAILABLE, DISCOVERING, "Review 5 modified"],
    );
    app.finish();
}

#[test]
fn finder_discovery_keeps_the_selected_identity() {
    let pending = launch("h01-finder-pending", "parity-discovery", "paused", Some(0));
    let pending_home = pending.screen();
    assert_eq!((pending_home.cols(), pending_home.rows()), (120, 40));
    see(
        &pending_home,
        &[DISCOVERING, "0 of 9", KEPT, "~/work/probe"],
        &[PULL, TESTS, UNAVAILABLE, "unavailable ·"],
    );
    assert_kept(&pending_home);
    pending.type_text("docker");
    let pending_docker = pending.wait_text(NO_MATCH);
    see(
        &pending_docker,
        &[
            NO_MATCH,
            "docker",
            DISCOVERING,
            "0 of 9",
            "Nothing to preview",
        ],
        &[UNAVAILABLE, "Results ·", "succeeded"],
    );
    assert!(
        selected(&pending_docker).is_empty(),
        "pending docker search has a runnable row\n{}",
        text(&pending_docker)
    );
    pending.finish();

    let ready = launch("h01-finder-ready", "parity-discovery", "paused", Some(8));
    // The action list paints before the discovering status.
    let ready_home = ready.wait_screen("frame 8 discovery", |screen| {
        let visible = text(screen);
        visible.contains(DISCOVERING)
            && visible.contains("5 of 9")
            && visible.contains(TESTS)
            && visible.contains(KEPT)
            && !visible.contains(PULL)
            && !visible.contains(UNAVAILABLE)
    });
    see(
        &ready_home,
        &[DISCOVERING, "5 of 9", TESTS, KEPT],
        &[PULL, UNAVAILABLE],
    );
    assert_kept(&ready_home);
    ready.finish();

    let failed = launch("h01-finder-failed", "parity-discovery", "paused", Some(12));
    let failed_home = failed.wait_text("6 of 9");
    see(
        &failed_home,
        &[DISCOVERING, "6 of 9", "3 config warnings", KEPT],
        &[UNAVAILABLE, PULL],
    );
    assert_kept(&failed_home);
    failed.type_text("docker");
    let failed_docker = failed.wait_text("unavailable · Cannot");
    see(
        &failed_docker,
        &[
            "unavailable · Cannot",
            "connect to the Docker",
            DISCOVERING,
            "6 of 9",
            "Docker",
        ],
        &[UNAVAILABLE, "succeeded", NO_MATCH],
    );
    assert!(
        selected(&failed_docker).contains("Docker"),
        "failed docker row is not selected\n{}",
        text(&failed_docker)
    );
    failed.finish();

    let github = launch("h01-finder-github", "parity-discovery", "paused", Some(20));
    let github_home = github.wait_text("7 of 9");
    see(
        &github_home,
        &[DISCOVERING, "7 of 9", "4 config warnings", TESTS, KEPT],
        &[UNAVAILABLE, PULL, "not logged in", "Clone a GitHub"],
    );
    assert_kept(&github_home);
    // `github.clone` is built inside `git_items`, which returns until `source_ready("git")`
    // (`src/bin/holla/domain/catalog.rs`). Git's `done_at` is 40, so the clone row is absent here.
    github.finish();

    let settled = launch(
        "h01-finder-unavailable",
        "parity-discovery",
        "paused",
        Some(40),
    );
    let unavailable = settled.wait_text(UNAVAILABLE);
    assert_eq!((unavailable.cols(), unavailable.rows()), (120, 40));
    see(
        &unavailable,
        &[UNAVAILABLE, PULL, KEPT, "4 config warnings"],
        &[DISCOVERING, "succeeded"],
    );
    assert_kept(&unavailable);
    assert!(
        !line_with(&unavailable, PULL).contains('›'),
        "Pull took the selection when it appeared\n{}",
        text(&unavailable)
    );
    settled.finish();

    // The preview age is tick * 80ms and leaves "0 s ago" after 12 ticks.
    // Capture the wide frame on a fresh process before that tick.
    let wide_app = open("h01-finder-discovery-wide", "parity-discovery");
    wide_app.visual_while(&visual_id(WIDE), |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && open_surface(screen)
            && text(screen).contains("live · 0 s ago")
    });
    wide_app.finish();

    let app = open("h01-finder-discovery", "parity-discovery");
    let home = app.screen();
    assert_eq!((home.cols(), home.rows()), (120, 40));
    assert!(
        open_surface(&home),
        "reduced finder is not the finished discovery surface\n{}",
        text(&home)
    );

    app.type_text("cargo ");
    let typed = app.wait_screen("typed cargo", |screen| {
        query_line(screen).contains("cargo ") && selected(screen).contains("cargo test")
    });
    see(&typed, &["cargo test", "Results ·"], &[QUERY, "succeeded"]);

    app.paste("clippy");
    let pasted = app.wait_screen("pasted clippy", |screen| {
        query_line(screen).contains("cargo clippy") && selected(screen).contains("cargo clippy")
    });
    see(
        &pasted,
        &["cargo clippy", UNAVAILABLE],
        &[QUERY, "succeeded", NO_MATCH],
    );

    app.key("backspace");
    let edited = app.wait_screen("backspaced the query", |screen| {
        let query = query_line(screen);
        query.contains("cargo clipp") && !query.contains("clippy")
    });
    assert!(
        text(&edited).contains(UNAVAILABLE),
        "backspace left discovery\n{}",
        text(&edited)
    );

    app.key("escape");
    let cleared = app.wait_screen("query cleared", open_surface);
    assert!(open_surface(&cleared));

    app.type_text(QUERY);
    let empty = app.wait_screen("empty query", |screen| {
        query_line(screen).contains(QUERY) && text(screen).contains(NO_MATCH)
    });
    see(
        &empty,
        &[NO_MATCH, QUERY, "Nothing to preview", UNAVAILABLE],
        &["Results ·", "succeeded", "fast-forward"],
    );
    assert!(
        selected(&empty).is_empty(),
        "empty results still have a runnable row\n{}",
        text(&empty)
    );
    app.key("enter");
    let held = app.wait_screen("enter on an empty result", |screen| {
        let visible = text(screen);
        visible.contains(NO_MATCH)
            && visible.contains(QUERY)
            && visible.contains("Nothing to preview")
            && visible.contains(UNAVAILABLE)
            && !visible.contains("succeeded")
            && !visible.contains("fast-forward")
            && selected(screen).is_empty()
    });
    see(&held, &[NO_MATCH, QUERY], &["succeeded", "exit 0"]);

    app.key("escape");
    let surface = app.wait_screen("discovery surface restored", open_surface);
    assert_eq!((surface.cols(), surface.rows()), (120, 40));
    assert!(open_surface(&surface));

    app.resize(80, 24);
    let narrow = app.wait_screen("finder at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains("Here")
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(&narrow, &["Here", "Search"], &[DISCOVERING, "succeeded"]);
    app.visual(&visual_id(NARROW));
    app.resize(120, 40);

    let restored = app.wait_screen("finder back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && open_surface(screen)
    });
    assert!(open_surface(&restored));
    app.finish();
}

#[test]
fn finder_scenario_first_use() {
    let app = open("h01-finder-first-use", "first-use");
    let screen = app.screen();
    see(
        &screen,
        &[
            "~/scratch",
            "/ is 91% full",
            "CPU 78% for 4 min",
            "no project",
        ],
        &[
            "Review 5 modified",
            "Follow acme-api-1",
            "~/work/probe",
            "~/work/holla",
            "~/work/acme",
            UNAVAILABLE,
        ],
    );
    app.finish();
}

#[test]
fn finder_scenario_rust_dirty() {
    let app = open("h01-finder-rust-dirty", "rust-dirty");
    let screen = app.screen();
    see(
        &screen,
        &[
            "~/work/holla",
            "Review 5 modified",
            "3 commits behind",
            "Who is blocking",
        ],
        &[
            "~/scratch",
            "91% full",
            "Follow acme-api-1",
            "~/work/probe",
            UNAVAILABLE,
            "Children · 3",
        ],
    );
    app.finish();
}

#[test]
fn finder_scenario_monorepo_root() {
    let app = open("h01-finder-monorepo-root", "monorepo-root");
    let screen = app.screen();
    see(
        &screen,
        &[
            "~/work/acme",
            "acme · mise monorepo",
            "Follow acme-api-1",
            "Children · 3 projects",
        ],
        &[
            "~/scratch",
            "Review 5 modified",
            "~/work/probe",
            "~/work/holla",
            "91% full",
            UNAVAILABLE,
        ],
    );
    app.finish();
}

#[test]
fn finder_scenario_parity_discovery() {
    let app = open("h01-finder-parity-discovery", "parity-discovery");
    let screen = app.screen();
    see(
        &screen,
        &[
            "~/work/probe",
            "probe · Rust project",
            UNAVAILABLE,
            KEPT,
            "branch is 1 commits",
        ],
        &[
            "~/scratch",
            "~/work/holla",
            "~/work/acme",
            "Review 5 modified",
            "Follow acme-api-1",
            "91% full",
        ],
    );
    app.finish();
}
