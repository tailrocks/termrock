//! Holla service snapshots, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h04_snapshots_services/` and each checkpoint's dots become
//! underscores. The shots are the open Homebrew services page. Reduced motion
//! keeps the 140ms press flash, and a same-size resize does not force a new
//! frame, so each shot nudges the width and comes back. The waits follow the
//! reported size and the `Homebrew services` anchor, not the size text.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const WIDE: &str = "H04.snapshots-services.services.120x40";
const NARROW: &str = "H04.snapshots-services.services.narrow";
const BREW: &str = "Homebrew services";
const MAC_PATH: &str = "~/work/mac";
const LINUX_PATH: &str = "~/work/box";
const MISE: &str = "Show mise configuration";
const MISE_TITLE: &str = "mise configuration";
const OUTDATED: &str = "Upgrade 3 outdated mise tools";
const OUTDATED_TITLE: &str = "Outdated mise tools";
const PG: &str = "Show PostgreSQL activity";
const PG_TITLE: &str = "PostgreSQL activity";
const BLOCKING: &str = "Who is blocking the database?";
const PAYMENTS: &str = "Show payments status";
const PAYMENTS_TITLE: &str = "payments.service";
const SERVICE_ROW: &str = "start · stop · restart";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h04_snapshots_services/{}",
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

fn selected(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains('›') && !line.contains("scope ‹"))
        .unwrap_or("")
        .to_string()
}

fn query_line(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains("scope ‹"))
        .unwrap_or("")
        .to_string()
}

fn row_has(screen: &TerminalSnapshot, parts: &[&str]) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        parts.iter().all(|part| line.contains(part))
    })
}

/// `row_mark` is the part of the label that stays visible after the finder
/// truncates a long row. The query field still has to hold the full label,
/// so a shorter neighbour cannot take the cursor.
fn open_snapshot(app: &App, query: &str, row_mark: &str, title: &str) -> TerminalSnapshot {
    app.paste(query);
    let picked = app.wait_screen("finder row selected", |screen| {
        query_line(screen).contains(query) && selected(screen).contains(row_mark)
    });
    assert!(
        query_line(&picked).contains(query) && selected(&picked).contains(row_mark),
        "selected row is not {query:?}\n{}",
        text(&picked)
    );
    app.key("enter");
    app.wait_screen("snapshot open", |screen| {
        let visible = text(screen);
        visible.contains(title) && !visible.contains("scope ‹")
    })
}

fn back(app: &App) {
    app.key("esc");
    app.wait_screen("finder restored", |screen| text(screen).contains("scope ‹"));
}

fn clear_query(app: &App) {
    app.key("esc");
    app.wait_text("Search actions and resources");
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. Below about 100 columns the header omits the size chip, so the wait
/// follows `screen.cols()` and `screen.rows()` plus the page anchor.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        text(&screen).contains(BREW),
        "shot left Homebrew services\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(BREW)
    });
    app.resize(cols, rows);
    app.wait_screen("restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(BREW)
    });
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
}

fn quit(app: &App) {
    app.key("ctrl+q");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if !app.child_running() {
            app.expect_exited(0);
            return;
        }
        let visible = text(&app.screen());
        if visible.contains("Quit holla") {
            let focused = visible
                .lines()
                .find(|line| line.contains('▎'))
                .unwrap_or("no focus gutter");
            panic!("quit dialog is open; focused control is {focused:?}\n{visible}");
        }
        assert!(
            Instant::now() < deadline,
            "quit did not exit\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn assert_mac_brew(screen: &TerminalSnapshot) {
    see(
        screen,
        &[BREW, "redis", MAC_PATH],
        &["postgresql@17", "payments-worker", MISE_TITLE, LINUX_PATH],
    );
    assert!(
        row_has(screen, &["redis", "started"]),
        "mac redis started row missing\n{}",
        text(screen)
    );
}

fn assert_linux_brew(screen: &TerminalSnapshot) {
    see(
        screen,
        &[BREW, "postgresql@17", LINUX_PATH],
        &[MAC_PATH, "payments-worker", MISE_TITLE],
    );
    assert!(
        row_has(screen, &["postgresql@17", "stopped"]),
        "linux postgresql@17 stopped row missing\n{}",
        text(screen)
    );
    assert!(
        !row_has(screen, &["redis", "started"]),
        "mac redis started row is on the linux brew page\n{}",
        text(screen)
    );
}

#[test]
fn h04_snapshots_services_route() {
    let app = open("h04-services-route", "parity-platforms");
    let opened = open_snapshot(&app, BREW, BREW, BREW);
    assert_mac_brew(&opened);
    assert!(
        text(&opened).contains(SERVICE_ROW),
        "brew table missing\n{}",
        text(&opened)
    );
    app.key("esc");
    let finder = app.wait_screen("back on Homebrew services", |screen| {
        text(screen).contains("scope ‹") && selected(screen).contains(BREW)
    });
    see(
        &finder,
        &[BREW, MAC_PATH, "scope ‹"],
        &["postgresql@17", SERVICE_ROW, MISE_TITLE],
    );
    assert!(
        selected(&finder).contains(BREW),
        "finder left the Homebrew services row\n{}",
        text(&finder)
    );
    quit(&app);
}

#[test]
fn h04_snapshots_services_services() {
    let linux = open("h04-services-linux", "parity-platforms-linux");
    let linux_brew = open_snapshot(&linux, BREW, BREW, BREW);
    assert_linux_brew(&linux_brew);
    back(&linux);
    clear_query(&linux);
    linux.paste(PAYMENTS);
    let absent = linux.wait_text("No matches for");
    see(
        &absent,
        &["No matches for", PAYMENTS],
        &[PAYMENTS_TITLE, "nginx.service", "Journal"],
    );
    linux.finish();

    let rust = open("h04-services-rust", "rust-dirty");
    let mise = open_snapshot(&rust, MISE, MISE, MISE_TITLE);
    see(
        &mise,
        &[MISE_TITLE, "Tools and tasks", "cargo-nextest", "shots"],
        &[
            "payments-worker",
            "nginx.service",
            BREW,
            "postgresql@17",
            ".service",
        ],
    );
    assert!(
        row_has(&mise, &["cargo-nextest", "0.9.101"]),
        "mise tool row missing\n{}",
        text(&mise)
    );
    back(&rust);
    clear_query(&rust);

    let outdated = open_snapshot(&rust, OUTDATED, "Upgrade 3 outdated", OUTDATED_TITLE);
    see(
        &outdated,
        &[OUTDATED_TITLE, "cargo-nextest", "Tools and tasks"],
        &[
            ".service",
            "shots",
            "payments-worker",
            BREW,
            "nginx.service",
        ],
    );
    assert!(
        row_has(&outdated, &["cargo-nextest", "0.9.104"]),
        "outdated tool row missing\n{}",
        text(&outdated)
    );
    back(&rust);
    clear_query(&rust);

    let pg = open_snapshot(&rust, PG, "Show PostgreSQL", PG_TITLE);
    see(
        &pg,
        &[
            PG_TITLE,
            "Sessions · blockers first",
            "48197",
            "idle in transaction",
        ],
        &[MISE_TITLE, BREW, "payments-worker", "redis", BLOCKING],
    );
    back(&rust);
    clear_query(&rust);

    let blocking = open_snapshot(&rust, BLOCKING, "Who is blocking", BLOCKING);
    see(
        &blocking,
        &[
            BLOCKING,
            "48197",
            "idle in transaction",
            "Sessions · blockers first",
        ],
        &[PG_TITLE, MISE_TITLE, BREW, "redis", ".service"],
    );
    rust.finish();

    let remote = open("h04-services-payments", "remote-host");
    let payments = open_snapshot(&remote, PAYMENTS, PAYMENTS, PAYMENTS_TITLE);
    see(
        &payments,
        &[PAYMENTS_TITLE, "Journal", "Started payments.service"],
        &[MISE_TITLE, "redis", "payments-worker", "nginx.service"],
    );
    remote.finish();

    let app = open("h04-services-mac", "parity-platforms");
    let wide = open_snapshot(&app, BREW, BREW, BREW);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert_mac_brew(&wide);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("brew at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(BREW)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert_mac_brew(&narrow);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("brew back at 120x40", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(BREW)
            && visible.contains(MAC_PATH)
            && visible.contains("Esc Back")
            && row_has(screen, &["redis", "started"])
    });
    assert_mac_brew(&restored);
    app.finish();
}

#[test]
fn h04_snapshots_services_scenario_parity_platforms() {
    let app = open("h04-services-scenario-mac", "parity-platforms");
    let brew = open_snapshot(&app, BREW, BREW, BREW);
    assert_mac_brew(&brew);
    assert!(
        !text(&brew).contains("postgresql@17"),
        "linux service identity on mac\n{}",
        text(&brew)
    );
    app.finish();
}

#[test]
fn h04_snapshots_services_scenario_parity_platforms_linux() {
    let app = open("h04-services-scenario-linux", "parity-platforms-linux");
    let brew = open_snapshot(&app, BREW, BREW, BREW);
    assert_linux_brew(&brew);
    assert!(
        text(&brew).contains(LINUX_PATH),
        "linux home path missing\n{}",
        text(&brew)
    );
    app.finish();
}
