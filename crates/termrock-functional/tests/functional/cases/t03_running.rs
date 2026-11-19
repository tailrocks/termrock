//! TablePro running query: a duplicate run is refused and cancel sticks.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_running/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const SQL: &str = "SELECT 1 AS t03_running_marker";
const MARKER: &str = "t03_running_marker";
const PLACEHOLDER: &str = "Type SQL";
const RUNNING: &str = "Esc cancels";
const REFUSED: &str = "Already running · Esc cancels";
const CANCELLED: &str = "Cancelled";
const HINT: &str = "The statement was stopped before it finished; nothing was recorded";
const RECORDED: &str = "nothing was recorded";
const RESULT_STATUS: &str = "Cancelled · nothing was recorded";
const STATUS: &str = "Query cancelled";
const HISTORY: &str = "Query history";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_running/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
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

fn flat(screen: &TerminalSnapshot) -> String {
    text(screen)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
                && !row.contains("Reconnect")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn connect_development(app: &App) {
    click_listed(app, "Development");
    let detail = app.wait_text(HOST);
    see(
        &detail,
        &[HOST, DATABASE, "Development"],
        &["Authentication failed", "Could not reach the host"],
    );
    click_connect(app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Execute query?",
        ],
    );
}

fn shot(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    app.visual_while(&visual_id(checkpoint), |screen| {
        screen.cols() == cols
            && screen.rows() == rows
            && cancelled_body(screen)
            && row_text(screen, screen.rows().saturating_sub(1)).contains(STATUS)
    });
}

/// The simulated statement finishes after six 80ms ticks. Saving the frame
/// before the next key lets those ticks complete, so Esc arrives too late.
fn frame_now(
    app: &App,
    reason: &str,
    predicate: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let screen = app.screen();
        if predicate(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "timed out: {reason}\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn running_frame(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(MARKER)
        && visible.contains("running")
        && visible.contains(RUNNING)
        && !visible.contains(CANCELLED)
        && !visible.contains(RECORDED)
        && !visible.contains("Execute query?")
        && !visible.contains("Nothing to run")
        && !visible.contains("Query 2")
}

fn cancelled_body(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(CANCELLED)
        && visible.contains(HINT)
        && visible.contains(RESULT_STATUS)
        && visible.contains(SQL)
        && !visible.contains("1 row")
        && !visible.contains("syntax error")
        && !visible.contains("Already running")
        && !visible.contains("Execute query?")
        && !visible.contains("Query 2")
        && !visible.contains("Error 1")
}

fn marker_stays_in_the_statement(screen: &TerminalSnapshot) {
    let mut saw = false;
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        if line.contains(MARKER) {
            saw = true;
            assert!(
                line.contains("SELECT"),
                "marker is outside the statement: {line}"
            );
        }
    }
    assert!(saw, "editor lost the statement\n{}", text(screen));
}

fn type_statement(app: &App) {
    let editor = app.wait_text(PLACEHOLDER);
    let hits = find_all(&editor, PLACEHOLDER);
    assert_eq!(
        hits.len(),
        1,
        "placeholder hits {hits:?}\n{}",
        text(&editor)
    );
    app.click(hits[0]);
    app.wait_screen("editor focused", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains('▎') && line.contains(PLACEHOLDER)
        })
    });
    flash_settle(app);
    app.type_text(SQL);
    app.wait_text(SQL);
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &["Query history"]);
    app.confirm_quit();
}

#[test]
fn tablepro_running_route_reaches_query_1() {
    let app = open("t03-running-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &["Query history", CANCELLED, "Loading schema", "Query 2"],
    );
    app.finish();
}

#[test]
fn tablepro_running_cancel_refuses_a_duplicate_and_keeps_the_cancelled_result() {
    let app = open("t03-running-cancel");
    connect_development(&app);
    type_statement(&app);

    app.key("ctrl+r");
    let running = frame_now(&app, "query is running", running_frame);
    app.key("ctrl+r");
    let refused = frame_now(&app, "second run refused", |screen| {
        text(screen).contains(REFUSED) && running_frame(screen)
    });
    app.key("esc");
    see(
        &running,
        &[MARKER, "running", RUNNING],
        &[CANCELLED, RECORDED, REFUSED, "Query 2"],
    );
    see(
        &refused,
        &[REFUSED, MARKER, "running", RUNNING],
        &[
            CANCELLED,
            RECORDED,
            "1 row",
            "syntax error",
            "Query 2",
            "Error 1",
        ],
    );

    let cancelled = app.wait_screen("cancelled result stays", cancelled_body);
    assert_eq!((cancelled.cols(), cancelled.rows()), (120, 40));
    marker_stays_in_the_statement(&cancelled);
    see(&cancelled, &[STATUS, RESULT_STATUS, HINT], &["1 row"]);

    let wide = app.screen();
    assert!(cancelled_body(&wide));
    shot(&app, "T03.running.cancel.120x40", 120, 40);
    let after_wide = app.screen();
    assert_eq!((after_wide.cols(), after_wide.rows()), (120, 40));
    assert!(cancelled_body(&after_wide), "{}", text(&after_wide));
    marker_stays_in_the_statement(&after_wide);

    app.resize(80, 24);
    let narrowed = app.wait_screen("cancelled result at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && cancelled_body(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    let narrow = app.screen();
    assert!(cancelled_body(&narrow), "{}", text(&narrow));
    shot(&app, "T03.running.cancel.narrow", 80, 24);
    app.resize(120, 40);
    let restored = app.wait_screen("cancelled result back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && cancelled_body(screen)
    });
    assert!(cancelled_body(&restored), "{}", text(&restored));
    marker_stays_in_the_statement(&restored);
    see(
        &restored,
        &[CANCELLED, HINT, RESULT_STATUS, SQL],
        &["Query 2"],
    );

    app.key("ctrl+y");
    let history = app.wait_screen("history omits the cancelled statement", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY) && !visible.contains(MARKER)
    });
    see(
        &history,
        &[HISTORY, "entries"],
        &[MARKER, RECORDED, "Query 2"],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query 1 after closing history", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains(SQL)
            && visible.contains(CANCELLED)
            && !visible.contains(HISTORY)
    });
    assert!(cancelled_body(&back), "{}", text(&back));
    marker_stays_in_the_statement(&back);
    quit_unsaved(&app);
}
