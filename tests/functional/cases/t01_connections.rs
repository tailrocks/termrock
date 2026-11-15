//! TablePro connections screen, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t01_connections/` and the dotted checkpoint ids use underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const ROUTE_ID: &str = "pre_refactor/t01_connections/t01_connections_route_120x40";
const SELECT_ID: &str = "pre_refactor/t01_connections/t01_connections_select_120x40";
const STATUS_ID: &str = "pre_refactor/t01_connections/t01_connections_status_120x40";

const LOCAL_HOST: &str = "localhost:5432";
const SCRATCH_HOST: &str = "~/scratch.db";
const ANALYTICS_HOST: &str = "analytics.acme.io";
const STAGING_HOST: &str = "staging-db.acme.io";
const PROD_HOST: &str = "prod-db-1.acme.io";
const WAREHOUSE: &str = "warehouse";
const PROD_DB: &str = "acme_prod";
const OPENING: &str = "Opening SSH tunnel…";
const AUTHENTICATING: &str = "Authenticating…";
const LOADING: &str = "Loading schema…";
const UNREACHABLE: &str = "Could not reach the host";
const AUTH_FAILED: &str = "Authentication failed";
const RECONNECT: &str = "Reconnect";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Filter connections";

fn open(name: &str, motion: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(PLACEHOLDER);
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

fn assert_size(screen: &TerminalSnapshot) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (120, 40),
        "terminal size\n{}",
        text(screen)
    );
}

/// The list panel at 120 columns ends at the rule before the detail card.
fn in_list(screen: &TerminalSnapshot, needle: &str) -> bool {
    find_all(screen, needle).into_iter().any(|hit| hit.x <= 35)
}

fn assert_listed(screen: &TerminalSnapshot, needle: &str, want: bool) {
    assert_eq!(
        in_list(screen, needle),
        want,
        "list {needle:?} want {want}\n{}",
        text(screen)
    );
}

fn click_listed(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "list hit {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_connect(app: &App) {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.x >= 40
                && row.contains("Edit")
                && row.contains("Delete")
                && !row.contains(RECONNECT)
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

/// The 140ms press flash stays painted while motion is paused, so a shot after
/// a released click has to redraw once the deadline has passed. Below about 85
/// columns the header omits the size chip, and the narrow nudge waits on the
/// terminal size instead.
fn flash_settle(app: &App) {
    app.settle_flash();
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    if cols >= 100 {
        app.wait_text(&format!("{nudged}×{rows}"));
        app.resize(cols, rows);
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow nudge", |screen| {
            screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
        });
        app.resize(cols, rows);
        app.wait_screen("narrow restored", |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
        });
    }
}

#[test]
fn tablepro_connections_route_opens_on_the_saved_list() {
    let app = open("t01-connections-route", "paused");
    let screen = app.settle();
    assert_size(&screen);
    see(
        &screen,
        &[
            "TablePro",
            "Connections",
            "6 saved",
            "truecolor · 120×40",
            PLACEHOLDER,
            "Personal",
            "Acme",
            "Local PostgreSQL",
            "Scratch",
            "Development",
            "Staging",
            "Analytics",
            "Production",
            LOCAL_HOST,
            "acme_dev",
            "2 minutes ago",
            "Connect",
            "Delete…",
        ],
        &[
            QUERY,
            OPENING,
            AUTHENTICATING,
            LOADING,
            UNREACHABLE,
            AUTH_FAILED,
            RECONNECT,
        ],
    );
    app.visual(ROUTE_ID);
    app.finish();
}

#[test]
fn tablepro_connections_selection_follows_the_leaf_and_keeps_it_through_search() {
    let app = open("t01-connections-select", "paused");
    let landing = app.settle();
    assert_size(&landing);
    see(&landing, &[LOCAL_HOST, "Local PostgreSQL"], &[SCRATCH_HOST]);

    app.key("down");
    app.key("down");
    let scratch = app.wait_text(SCRATCH_HOST);
    see(
        &scratch,
        &[SCRATCH_HOST, "SQLite", "never", "Scratch"],
        &[LOCAL_HOST, ANALYTICS_HOST, PROD_HOST],
    );

    click_listed(&app, "Analytics");
    flash_settle(&app);
    let analytics = app.wait_text(ANALYTICS_HOST);
    see(
        &analytics,
        &[ANALYTICS_HOST, WAREHOUSE, "MySQL", "last week", "Analytics"],
        &[SCRATCH_HOST, PROD_HOST, LOCAL_HOST],
    );

    app.key("/");
    app.type_text("Production");
    let one = app.wait_screen("one connection matches", |screen| {
        in_list(screen, "pg")
            && !in_list(screen, "Staging")
            && !in_list(screen, "Scratch")
            && text(screen).contains(ANALYTICS_HOST)
            && !text(screen).contains(PROD_HOST)
    });
    assert_listed(&one, "Acme", true);
    assert_listed(&one, "Development", false);
    assert_listed(&one, "Local PostgreSQL", false);
    assert_listed(&one, "mysql", false);
    see(
        &one,
        &[ANALYTICS_HOST, WAREHOUSE, "Analytics"],
        &[PROD_HOST, LOCAL_HOST],
    );

    app.type_text("zzzz");
    let none = app.wait_screen("no connection matches", |screen| {
        !in_list(screen, "pg")
            && !in_list(screen, "Acme")
            && text(screen).contains("Productionzzzz")
            && text(screen).contains(ANALYTICS_HOST)
    });
    see(
        &none,
        &["Productionzzzz", ANALYTICS_HOST, WAREHOUSE],
        &[PROD_HOST, LOCAL_HOST, "Staging"],
    );

    app.key("escape");
    let cleared = app.wait_screen("empty search restores the list", |screen| {
        text(screen).contains(PLACEHOLDER)
            && in_list(screen, "Local PostgreSQL")
            && text(screen).contains(ANALYTICS_HOST)
            && !text(screen).contains("Productionzzzz")
    });
    see(
        &cleared,
        &[
            PLACEHOLDER,
            ANALYTICS_HOST,
            WAREHOUSE,
            "Local PostgreSQL",
            "Development",
            "Staging",
            "Production",
            "Scratch",
        ],
        &[PROD_HOST, LOCAL_HOST, "Productionzzzz"],
    );

    click_listed(&app, "Production");
    flash_settle(&app);
    let production = app.wait_text(PROD_HOST);
    assert_size(&production);
    see(
        &production,
        &[PROD_HOST, PROD_DB, "Production", "1 hour ago", "PostgreSQL"],
        &[ANALYTICS_HOST, WAREHOUSE, UNREACHABLE, RECONNECT, QUERY],
    );
    app.visual(SELECT_ID);
    app.finish();
}

#[test]
fn tablepro_connections_status_stays_on_the_selected_connection() {
    let paused = open("t01-connections-status", "paused");
    click_listed(&paused, "Analytics");
    flash_settle(&paused);
    let selected = paused.wait_text(ANALYTICS_HOST);
    see(
        &selected,
        &["Analytics", ANALYTICS_HOST, WAREHOUSE],
        &[OPENING, RECONNECT],
    );
    click_connect(&paused);
    let opening = paused.wait_text(OPENING);
    see(
        &opening,
        &["Analytics", ANALYTICS_HOST, WAREHOUSE, OPENING],
        &[
            UNREACHABLE,
            AUTH_FAILED,
            RECONNECT,
            AUTHENTICATING,
            LOADING,
            QUERY,
        ],
    );
    flash_settle(&paused);
    let held = paused.settle();
    assert_size(&held);
    see(
        &held,
        &["Analytics", ANALYTICS_HOST, WAREHOUSE, OPENING],
        &[
            UNREACHABLE,
            AUTH_FAILED,
            RECONNECT,
            AUTHENTICATING,
            LOADING,
            QUERY,
        ],
    );
    paused.visual(STATUS_ID);
    paused.finish();

    let app = open("t01-connections-status-reduced", "reduced");
    click_listed(&app, "Staging");
    flash_settle(&app);
    let staging = app.wait_text(STAGING_HOST);
    see(
        &staging,
        &[STAGING_HOST, "acme_staging", "Staging"],
        &[AUTH_FAILED, QUERY],
    );
    click_connect(&app);
    let auth = app.wait_text(AUTH_FAILED);
    see(
        &auth,
        &[AUTH_FAILED, RECONNECT, STAGING_HOST, "Staging"],
        &[QUERY, UNREACHABLE, OPENING],
    );

    click_listed(&app, "Analytics");
    flash_settle(&app);
    let moved = app.wait_text(ANALYTICS_HOST);
    see(
        &moved,
        &["Analytics", ANALYTICS_HOST, WAREHOUSE],
        &[AUTH_FAILED, RECONNECT, UNREACHABLE, QUERY],
    );
    click_connect(&app);
    let failed = app.wait_text(UNREACHABLE);
    see(
        &failed,
        &[UNREACHABLE, RECONNECT, "Analytics", ANALYTICS_HOST],
        &[QUERY, AUTH_FAILED, OPENING],
    );

    click_listed(&app, "Production");
    flash_settle(&app);
    let production = app.wait_text(PROD_HOST);
    see(
        &production,
        &[PROD_HOST, PROD_DB, "Production"],
        &[UNREACHABLE, AUTH_FAILED, RECONNECT, QUERY, ANALYTICS_HOST],
    );
    app.finish();
}
