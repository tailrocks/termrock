//! Holla custom-action configuration before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h02_config/` and each checkpoint's dots become underscores.
//!
//! The open configuration page draws `clock.ago(now_secs() - 1)` in its header
//! (`screens/snapshot.rs`). That delta is always one second, so the label stays
//! `1 s ago` across a later redraw. `--frame` is not passed.
//!
//! Skipped project entries are diagnostics, not actions (`domain/custom.rs`).
//! `custom_items` never marks a custom action `Freshness::Unavailable`. The
//! unavailable proof is the hard-cases row `Stop all containers`
//! (`docker.stop_all`, `Launch::Activity`), not a custom action and not the
//! Docker resource, which uses `Launch::Snapshot` and is exempt in `run_item`.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, text, try_launch};

const PLACEHOLDER: &str = "Search actions and resources";
const CONFIG_ROW: &str = "Custom action configuration";
const PAGE: &str = "Custom actions";
const TEAM_FILE: &str = "~/work/team/.holla.toml";
const PROJECT_FILE: &str = "/Users/alex/work/team/.holla.toml";
const DUP_DIAG: &str = "action[2]:";
const DUP_MSG: &str = "id \"shared\" is already defined by the global configuration";
const BAD_DIAG: &str = "action[3]:";
const BAD_ID: &str = "Bad Id";
const BAD_MSG: &str = "invalid id \"Bad Id\"";
const DUP_LABEL: &str = "Duplicate of a global id";
const OPEN_NOTES: &str = "Open notes";
const DEPLOY: &str = "Deploy preview";
const STOP_ALL: &str = "Stop all containers";
const UNAVAILABLE_STATUS: &str = "Stop all containers is unavailable";
const DOCKER_WHY: &str = "Cannot connect to the Docker daemon at unix:///var/run/docker.sock";
const FAILED_SOURCES: &str = "! docker, github unavailable";
const WORKSPACE: &str = "northwind-traders-platform-migration";
/// `StatusBar` keeps 44 columns of the cwd (`truncate_middle`), so the service
/// directory is drawn as this tail, not the full `inventory-reconciliation-service`.
const SERVICE_TAIL: &str = "iation-service";
const ACTIVITY: &str = "follow tail";
const TRUST_REVIEW: &str = "trust review";
const DISCOVERING: &str = "discovering";
const NO_MATCH: &str = "No matches for";
const WIDE: &str = "H02.config.config.120x40";
const NARROW: &str = "H02.config.config.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h02_config/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, crate::support::HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_screen("finder ready", |screen| {
        let visible = text(screen);
        visible.contains(PLACEHOLDER)
            && !visible.contains(DISCOVERING)
            && screen.cols() == 120
            && screen.rows() == 40
    });
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
        .find(|line| {
            // The workspace crumb also draws `›`. The cursor marker sits in the
            // list gutter, in the first columns.
            line.find('›').is_some_and(|at| at <= 12) && !line.contains("scope ‹")
        })
        .unwrap_or("")
        .to_string()
}

fn query_line(screen: &TerminalSnapshot) -> String {
    line_with(screen, "scope ‹")
}

fn selected_is(screen: &TerminalSnapshot, label: &str) -> bool {
    let row = selected(screen);
    if row.contains(label) {
        return true;
    }
    let prefix: String = label.chars().take(22).collect();
    prefix.chars().count() >= 12 && row.contains(&prefix)
}

fn offered_label(line: &str, label: &str) -> bool {
    let Some(at) = line.find('›') else {
        return false;
    };
    if line.contains("scope ‹") {
        return false;
    }
    let rest = line[at + '›'.len_utf8()..].trim_start();
    rest.starts_with(label)
        && rest
            .chars()
            .nth(label.chars().count())
            .is_none_or(|ch| ch.is_whitespace())
}

fn press(app: &App, key: &str) {
    app.key(key);
}

/// A same-size resize does not force a frame, and the cursor cell is part of
/// the hash. Nudge the width and come back. Wait on the terminal size.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged width", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
    });
    app.resize(cols, rows);
    app.wait_screen("restored size", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
    });
}

fn clear_query(app: &App) {
    for _ in 0..4 {
        if text(&app.screen()).contains(PLACEHOLDER) {
            return;
        }
        press(app, "escape");
    }
    app.wait_text(PLACEHOLDER);
}

fn search(app: &App, query: &str) -> TerminalSnapshot {
    app.paste(query);
    app.wait_screen(&format!("search {query}"), |screen| {
        query_line(screen).contains(query) && !text(screen).contains(DISCOVERING)
    })
}

fn assert_not_offered(screen: &TerminalSnapshot, labels: &[&str]) {
    let visible = text(screen);
    for line in visible.lines() {
        for label in labels {
            assert!(
                !offered_label(line, label),
                "offered {label:?}\n{line}\n{visible}"
            );
        }
    }
}

fn open_config(app: &App) -> TerminalSnapshot {
    let found = search(app, CONFIG_ROW);
    assert!(
        selected_is(&found, CONFIG_ROW)
            && text(&found).contains("some entries were skipped")
            && text(&found).contains("5 valid actions"),
        "config row is not selected\n{}",
        text(&found)
    );
    press(app, "enter");
    app.wait_screen("custom actions page", |screen| {
        let visible = text(screen);
        visible.contains(PAGE)
            && visible.contains(DUP_DIAG)
            && visible.contains(BAD_DIAG)
            && visible.contains(BAD_ID)
            && visible.contains(DEPLOY)
            && visible.contains("deploy.preview")
            && visible.contains("1 s ago")
            && !visible.contains(DISCOVERING)
            && !visible.contains(TRUST_REVIEW)
            && !visible.contains(ACTIVITY)
    })
}

fn assert_config(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            PAGE,
            TEAM_FILE,
            PROJECT_FILE,
            DUP_DIAG,
            DUP_MSG,
            BAD_DIAG,
            BAD_MSG,
            BAD_ID,
            "deploy.preview",
            "wipe",
            "multi",
            "Moved",
            "1 s ago",
        ],
        &[
            ACTIVITY,
            TRUST_REVIEW,
            DISCOVERING,
            WORKSPACE,
            SERVICE_TAIL,
            UNAVAILABLE_STATUS,
        ],
    );
}

#[test]
fn route_opens_config_and_returns() {
    let app = launch("h02-config-route", "parity-custom-actions");
    let finder = app.screen();
    assert_eq!((finder.cols(), finder.rows()), (120, 40));
    see(
        &finder,
        &["Here", PLACEHOLDER, "~/work/team"],
        &[DISCOVERING, WORKSPACE, SERVICE_TAIL, ACTIVITY],
    );
    let page = open_config(&app);
    assert_config(&page);
    press(&app, "escape");
    let back = app.wait_screen("finder after config", |screen| {
        let visible = text(screen);
        visible.contains("Here")
            && visible.contains("scope ‹")
            && !visible.contains(DUP_DIAG)
            && !visible.contains(ACTIVITY)
            && !visible.contains(TRUST_REVIEW)
    });
    see(
        &back,
        &["Here", "~/work/team"],
        &[WORKSPACE, SERVICE_TAIL, ACTIVITY],
    );
    app.finish();
}

#[test]
fn config_names_bad_actions_and_refuses_unavailable() {
    let hard = launch("h02-config-unavailable", "hard-cases");
    let home = hard.wait_text(FAILED_SOURCES);
    see(
        &home,
        &[FAILED_SOURCES, WORKSPACE, SERVICE_TAIL, "Here", "rebase"],
        &[DISCOVERING, DEPLOY, OPEN_NOTES, BAD_ID, DUP_LABEL, ACTIVITY],
    );
    let stop = search(&hard, STOP_ALL);
    assert!(
        selected_is(&stop, STOP_ALL) && !selected(&stop).contains("only"),
        "stop-all row is not selected\n{}",
        text(&stop)
    );
    assert_not_offered(&stop, &[DEPLOY, OPEN_NOTES, BAD_ID, DUP_LABEL]);
    press(&hard, "enter");
    let refused = hard.wait_screen("stop all refused", |screen| {
        let visible = text(screen);
        visible.contains(UNAVAILABLE_STATUS)
            && visible.contains("unavailable")
            && visible.contains("scope ‹")
            && !visible.contains(ACTIVITY)
            && !visible.contains(TRUST_REVIEW)
            && !visible.contains(PAGE)
    });
    see(
        &refused,
        &[UNAVAILABLE_STATUS, "unavailable", STOP_ALL, DOCKER_WHY],
        &[ACTIVITY, TRUST_REVIEW, PAGE, DEPLOY, OPEN_NOTES, BAD_ID],
    );
    assert!(
        query_line(&refused).contains(STOP_ALL),
        "left the finder\n{}",
        text(&refused)
    );
    hard.finish();

    let app = launch("h02-config-page", "parity-custom-actions");
    let duplicate = search(&app, DUP_LABEL);
    assert!(
        text(&duplicate).contains(NO_MATCH),
        "duplicate label is runnable\n{}",
        text(&duplicate)
    );
    assert!(
        selected(&duplicate).is_empty(),
        "duplicate search selected a row\n{}",
        text(&duplicate)
    );
    assert_not_offered(&duplicate, &[DUP_LABEL, BAD_ID, "x"]);
    press(&app, "enter");
    let held = app.wait_screen("enter on duplicate search", |screen| {
        text(screen).contains(NO_MATCH)
            && selected(screen).is_empty()
            && query_line(screen).contains(DUP_LABEL)
            && !text(screen).contains(ACTIVITY)
    });
    see(
        &held,
        &[NO_MATCH, DUP_LABEL],
        &[ACTIVITY, PAGE, TRUST_REVIEW],
    );
    clear_query(&app);

    let bad = search(&app, BAD_ID);
    assert_not_offered(&bad, &[BAD_ID, "x", DUP_LABEL]);
    assert!(
        !text(&bad).contains(ACTIVITY),
        "bad id search started an activity\n{}",
        text(&bad)
    );
    clear_query(&app);

    let page = open_config(&app);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    assert_config(&page);
    flash_settle(&app);
    app.visual(&visual_id(WIDE));

    app.resize(80, 24);
    let narrow = app.wait_screen("config at 80x24", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(PAGE)
            && visible.contains(DUP_DIAG)
            && visible.contains(BAD_DIAG)
            && visible.contains(BAD_ID)
            && visible.contains("shared")
            && !visible.contains(DISCOVERING)
            && !visible.contains(ACTIVITY)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[PAGE, DUP_DIAG, BAD_DIAG, BAD_ID, "shared"],
        &[ACTIVITY, TRUST_REVIEW, DISCOVERING, WORKSPACE],
    );
    flash_settle(&app);
    app.visual(&visual_id(NARROW));
    app.resize(120, 40);
    let restored = app.wait_screen("config back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(PAGE)
            && text(screen).contains(DUP_DIAG)
            && text(screen).contains(BAD_ID)
    });
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    see(
        &restored,
        &[PAGE, DUP_DIAG, BAD_ID],
        &[ACTIVITY, TRUST_REVIEW],
    );
    app.finish();
}

#[test]
fn scenario_shows_parity_custom_actions() {
    let app = launch("h02-config-scenario-parity", "parity-custom-actions");
    let notes = search(&app, OPEN_NOTES);
    assert!(
        selected(&notes).contains(OPEN_NOTES),
        "open notes is not selected\n{}",
        text(&notes)
    );
    see(
        &notes,
        &[OPEN_NOTES, "~/work/team", "mbp"],
        &[WORKSPACE, SERVICE_TAIL, FAILED_SOURCES, ACTIVITY],
    );
    clear_query(&app);
    let deploy = search(&app, DEPLOY);
    assert!(
        selected(&deploy).contains(DEPLOY),
        "deploy preview is not selected\n{}",
        text(&deploy)
    );
    see(
        &deploy,
        &[DEPLOY, "~/work/team"],
        &[WORKSPACE, SERVICE_TAIL, ACTIVITY, TRUST_REVIEW],
    );
    clear_query(&app);
    let page = open_config(&app);
    see(
        &page,
        &[
            PAGE,
            TEAM_FILE,
            PROJECT_FILE,
            BAD_ID,
            DUP_MSG,
            DEPLOY,
            "deploy.preview",
        ],
        &[
            WORKSPACE,
            SERVICE_TAIL,
            ACTIVITY,
            TRUST_REVIEW,
            UNAVAILABLE_STATUS,
        ],
    );
    app.finish();
}

#[test]
fn scenario_shows_hard_cases() {
    let app = launch("h02-config-scenario-hard", "hard-cases");
    let home = app.wait_text(FAILED_SOURCES);
    see(
        &home,
        &[
            FAILED_SOURCES,
            WORKSPACE,
            SERVICE_TAIL,
            "rebase",
            "2 conflicts",
        ],
        &[
            DISCOVERING,
            DEPLOY,
            OPEN_NOTES,
            BAD_ID,
            DUP_LABEL,
            TEAM_FILE,
            ACTIVITY,
            DOCKER_WHY,
        ],
    );
    app.finish();
}
