//! TablePro query history before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t04_history/` and each checkpoint's dots become underscores.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, TABLEPRO, find_all, row_text, text, try_launch};

const ENTRY: &str = "t04_history_entry";
const DECOY: &str = "t04_history_decoy";
const MISSING: &str = "t04_history_missing";
/// A leading comment keeps the marker in the stored SQL. A bare
/// `SELECT '…' AS marker` never reaches `FROM`, so `parse_select` records it
/// as a failure and failed-only would keep the row (`sql.rs`, `History::search`).
const ENTRY_SQL: &str =
    "-- t04_history_entry\nSELECT slug FROM organizations WHERE slug = 'tessellate' LIMIT 1";
const DECOY_SQL: &str = "SELECT 't04_history_decoy' AS marker";
const RESULT: &str = "tessellate";
const PENDING: &str = "status = 'pending'";
/// The history list truncates the pending statement before `'pending'`.
/// The seed row is 4 minutes ago and 38 ms (`model.rs`).
const PENDING_ROW: &str = "4 min ago · 38 ms";
const CHECKOUT: &str = "checkout";
const ORDRES: &str = "FROM ordres";
const RUNNING: &str = "Esc cancels";
const NO_MATCH: &str = "No matching queries";
/// `parse_select` returns this when the statement has no FROM (`sql.rs`).
const PARSE: &str = "Expected FROM";
const COPIED: &str = "Copied query";
const CONNECTED: &str = "Connected to Development";
const HISTORY: &str = "Query history";
const HOST: &str = "dev-db.internal.acme.io";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t04_history/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.platform = "macos".to_string();
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

fn list_row(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| point.x < 40)
        .collect();
    assert_eq!(hits.len(), 1, "list row {label:?}\n{}", text(&screen));
    hits[0]
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn connect(app: &App) {
    app.click(list_row(app, "Development"));
    app.wait_text(HOST);
    let focused = focused_line(&app.screen());
    assert!(
        focused.contains("Development"),
        "Enter would not connect Development\n{focused}"
    );
    app.key("enter");
    app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains("Query 1")
            && visible.contains(CONNECTED)
            && !visible.contains("Loading schema")
    });
}

fn focus_editor(app: &App) {
    let screen = app.screen();
    let point = (0..screen.rows()).find_map(|row| {
        let line = row_text(&screen, row);
        let x = line.find("  1 ")?;
        Some(Point {
            x: (x + 4) as u16,
            y: u16::try_from(row).expect("row"),
        })
    });
    let Some(point) = point else {
        panic!("no editor line\n{}", text(&screen));
    };
    app.click(point);
    app.wait_screen("editor editing", |screen| text(screen).contains("EDIT"));
}

fn put_sql(app: &App, sql: &str, marker: &str) {
    focus_editor(app);
    app.key("ctrl+l");
    let picked = app.screen();
    assert!(
        !text(&picked).contains("Safe Mode · this connection"),
        "Ctrl+L opened Safe Mode instead of selecting\n{}",
        text(&picked)
    );
    app.paste(sql);
    app.wait_screen(&format!("sql {marker}"), |screen| {
        text(screen).contains(marker)
    });
}

fn run_finished(app: &App, marker: &str) -> TerminalSnapshot {
    app.key("ctrl+r");
    app.wait_screen("statement finished", |screen| {
        let visible = text(screen);
        visible.contains(marker)
            && visible.contains(RESULT)
            && visible.contains("1 row")
            && !visible.contains(RUNNING)
            && !visible.contains(DECOY)
            && !visible.contains(PARSE)
    })
}

fn scope_is(screen: &TerminalSnapshot, scope: &str, status: &str) -> bool {
    text(screen).contains(&format!("scope: {scope}"))
        && text(screen).contains(&format!("status: {status}"))
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(
        &quit,
        &["Quit TablePro?", "1 unsaved query will be lost."],
        &[RUNNING, "Execute query?"],
    );
    app.key("right");
    app.key("enter");
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "tablepro stayed up after confirming quit"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

#[test]
fn route_opens_history_and_quits() {
    let app = open("t04-history-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[HISTORY, "Query 1", ENTRY, DECOY],
    );
    connect(&app);
    app.key("ctrl+y");
    let history = app.wait_screen("history from the workbench", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY)
            && visible.contains("scope: Development")
            && visible.contains("status: any")
            && !visible.contains("Loading schema")
    });
    assert_eq!((history.cols(), history.rows()), (120, 40));
    see(
        &history,
        &[HISTORY, CONNECTED, "Rerun", "scope: Development"],
        &[DECOY, ENTRY, NO_MATCH, "all connections", RUNNING],
    );
    app.finish();
}

#[test]
fn rerun_uses_the_selected_entry_sql() {
    let app = open("t04-history-rerun");
    connect(&app);
    focus_editor(&app);
    app.key("ctrl+l");
    app.paste(ENTRY_SQL);
    app.wait_text(ENTRY);
    let ran = run_finished(&app, ENTRY);
    see(
        &ran,
        &[ENTRY, RESULT, "1 row", "Query 1"],
        &[DECOY, RUNNING, PENDING, PARSE],
    );

    put_sql(&app, DECOY_SQL, DECOY);
    let decoy = app.screen();
    see(&decoy, &[DECOY], &[ENTRY, RUNNING]);

    app.key("ctrl+y");
    let history = app.wait_screen("history lists the ran statement", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY)
            && visible.contains(ENTRY)
            && visible.contains("just now")
            && !visible.contains(DECOY)
            && !visible.contains(NO_MATCH)
    });
    see(
        &history,
        &[HISTORY, ENTRY, "Rerun", "scope: Development", "status: any"],
        &[DECOY, NO_MATCH, "all connections", PENDING],
    );

    app.key("/");
    app.type_text(ENTRY);
    let found = app.wait_screen("search keeps the entry", |screen| {
        let visible = text(screen);
        visible.contains(ENTRY) && visible.contains("just now") && !visible.contains(NO_MATCH)
    });
    see(&found, &[ENTRY, "just now"], &[DECOY, NO_MATCH, PENDING]);

    app.key("esc");
    app.type_text(MISSING);
    let missed = app.wait_screen("missing search has no rows", |screen| {
        let visible = text(screen);
        visible.contains(NO_MATCH) && visible.contains(MISSING) && !visible.contains(ENTRY)
    });
    see(&missed, &[NO_MATCH, MISSING], &[ENTRY, DECOY, "just now"]);

    app.key("esc");
    let cleared = app.wait_screen("search cleared", |screen| {
        let visible = text(screen);
        visible.contains(ENTRY)
            && visible.contains("status: any")
            && !visible.contains(MISSING)
            && !visible.contains(NO_MATCH)
    });
    see(
        &cleared,
        &[ENTRY, "scope: Development"],
        &[DECOY, MISSING, NO_MATCH],
    );

    app.key("down");
    app.key("c");
    let all = app.wait_screen("scope is every connection", |screen| {
        scope_is(screen, "all connections", "any") && text(screen).contains(PENDING_ROW)
    });
    see(
        &all,
        &[PENDING_ROW, ENTRY, "all connections"],
        &[DECOY, NO_MATCH],
    );

    app.key("c");
    let dev = app.wait_screen("scope returns to Development", |screen| {
        scope_is(screen, "Development", "any")
            && text(screen).contains(ENTRY)
            && !text(screen).contains(PENDING_ROW)
            && !text(screen).contains("all connections")
    });
    see(
        &dev,
        &[ENTRY, "scope: Development"],
        &[DECOY, PENDING_ROW, "all connections"],
    );

    app.key("s");
    let failed = app.wait_screen("failed-only hides the entry", |screen| {
        let visible = text(screen);
        scope_is(screen, "Development", "failed")
            && visible.contains(CHECKOUT)
            && !visible.contains(ENTRY)
            && !visible.contains(ORDRES)
    });
    see(
        &failed,
        &[CHECKOUT, "status: failed"],
        &[ENTRY, DECOY, ORDRES, PENDING_ROW, "just now"],
    );

    app.key("c");
    let failed_all = app.wait_screen("failed-only shows every connection", |screen| {
        let visible = text(screen);
        scope_is(screen, "all connections", "failed")
            && visible.contains(ORDRES)
            && visible.contains(CHECKOUT)
            && !visible.contains(ENTRY)
    });
    see(
        &failed_all,
        &[ORDRES, CHECKOUT, "all connections"],
        &[ENTRY, DECOY, PENDING_ROW],
    );

    app.key("c");
    app.wait_screen("failed-only returns to Development", |screen| {
        let visible = text(screen);
        scope_is(screen, "Development", "failed")
            && visible.contains(CHECKOUT)
            && !visible.contains(ORDRES)
            && !visible.contains(ENTRY)
    });

    app.key("s");
    let restored = app.wait_screen("failed-only off restores the entry", |screen| {
        let visible = text(screen);
        scope_is(screen, "Development", "any")
            && visible.contains("-- t04_history_entry")
            && !visible.contains(DECOY)
            && !visible.contains(PENDING_ROW)
    });
    assert!(
        text(&restored).contains("-- t04_history_entry"),
        "detail did not show the selected statement\n{}",
        text(&restored)
    );
    see(
        &restored,
        &[ENTRY, "status: any"],
        &[DECOY, PENDING_ROW, "all connections", NO_MATCH],
    );

    app.key("y");
    let copied = app.wait_text(COPIED);
    see(
        &copied,
        &[COPIED, "-- t04_history_entry"],
        &["Not copied", DECOY],
    );
    assert!(
        text(&copied).contains("-- t04_history_entry"),
        "copy changed the detail\n{}",
        text(&copied)
    );

    app.key("r");
    let rerun = app.wait_screen("rerun finished and the copy status faded", |screen| {
        let visible = text(screen);
        visible.contains(ENTRY)
            && visible.contains(RESULT)
            && visible.contains("1 row")
            && !visible.contains(RUNNING)
            && !visible.contains(DECOY)
            && !visible.contains(PENDING)
            && !visible.contains(PARSE)
            && !visible.contains(COPIED)
            && !visible.contains("Not copied")
    });
    assert_eq!((rerun.cols(), rerun.rows()), (120, 40));
    see(
        &rerun,
        &[ENTRY, RESULT, "1 row", "Query 2"],
        &[DECOY, PENDING, PARSE, RUNNING, COPIED, HISTORY],
    );
    app.visual(&visual_id("T04.history.rerun.120x40"));

    app.resize(80, 24);
    let narrow = app.wait_screen("rerun result at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(ENTRY)
            && text(screen).contains(RESULT)
            && text(screen).contains("1 row")
            && !text(screen).contains(RUNNING)
            && !text(screen).contains(DECOY)
            && !text(screen).contains(PENDING)
            && !text(screen).contains(PARSE)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[ENTRY, RESULT, "1 row"],
        &[DECOY, PENDING, PARSE, RUNNING],
    );
    app.visual(&visual_id("T04.history.rerun.narrow"));
    app.resize(120, 40);
    app.wait_screen("rerun result back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(ENTRY)
            && !text(screen).contains(DECOY)
            && !text(screen).contains(RUNNING)
    });

    app.key("ctrl+y");
    app.wait_screen("history focused for open", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY) && visible.contains(ENTRY) && visible.contains("Rerun")
    });
    app.key("enter");
    let opened = app.wait_screen("enter opens the entry without running it", |screen| {
        let visible = text(screen);
        visible.contains(ENTRY)
            && visible.contains("No results yet")
            && !visible.contains(RUNNING)
            && !visible.contains(PARSE)
            && !visible.contains(DECOY)
            && !visible.contains(PENDING)
    });
    see(
        &opened,
        &[ENTRY, "No results yet", "Query 3"],
        &[DECOY, PARSE, RUNNING, PENDING, "1 row"],
    );
    std::thread::sleep(Duration::from_millis(500));
    let still = app.screen();
    see(
        &still,
        &[ENTRY, "No results yet"],
        &[DECOY, PARSE, RUNNING, PENDING],
    );
    quit_unsaved(&app);
}
