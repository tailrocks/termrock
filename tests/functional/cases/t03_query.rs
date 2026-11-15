//! TablePro safe-mode and statement classification before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_query/` and each checkpoint's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, TABLEPRO, find_all, flash_settle_stepped as flash_settle, row_text, text,
    try_launch,
};

const DENY: &str =
    "Cannot execute write queries: TablePro's Safe Mode is set to read-only for this connection";
const DANGER: &str = "This query may permanently modify or delete data";
const WRITE_TITLE: &str = "Execute write query?";
const QUERY_TITLE: &str = "Execute query?";
const CANCELLED: &str = "Cancelled · nothing was executed";
const SELECT_500: &str = "Showing 500 rows · 3 ms";
const DELETE_ACTION: &str = "DELETE without WHERE";
const SILENT_FACT: &str = "Silent · confirm before running";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_query/{}",
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

fn header_token(screen: &TerminalSnapshot, token: &str) {
    let header = row_text(screen, 0);
    let present = match token {
        "alert" => header.contains("alert") && !header.contains("alert+"),
        "safe" => header.contains("safe") && !header.contains("safe+"),
        other => header.contains(other),
    };
    assert!(present, "header token {token:?} in {header:?}");
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
}

fn wait_size(app: &App, cols: u16, rows: u16) -> TerminalSnapshot {
    app.resize(cols, rows);
    app.wait_screen(&format!("size {cols}x{rows}"), |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
    })
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
    app.wait_text("dev-db.internal.acme.io");
    let focused = focused_line(&app.screen());
    assert!(
        focused.contains("Development"),
        "Enter would not connect Development\n{focused}"
    );
    app.key("enter");
    app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains("Query 1")
            && visible.contains("Connected to Development")
            && !visible.contains("Loading schema")
    });
}

fn busy(screen: &TerminalSnapshot) -> bool {
    // "confirm before running" also contains "running". The running query
    // names Esc as the cancel.
    text(screen).contains("Esc cancels")
}

fn idle(app: &App) {
    app.wait_screen("query idle", |screen| !busy(screen));
}

fn focus_editor(app: &App) {
    let screen = app.screen();
    let point = (0..screen.rows()).find_map(|row| {
        let line = row_text(&screen, row);
        // The gutter number is only on the editor line. Result tabs repeat
        // the statement verb, so a text search would hit those first.
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
    idle(app);
    focus_editor(app);
    // macOS field keys are Emacs: Ctrl+L selects all while the editor is
    // capturing. Ctrl+A does not. The shell binding is idle-only, so it
    // stays with the editor until edit mode ends.
    app.key("ctrl+l");
    let picked = app.screen();
    assert!(
        !text(&picked).contains("Safe Mode · this connection"),
        "Ctrl+L opened Safe Mode instead of selecting\n{}",
        text(&picked)
    );
    app.paste(sql);
    let screen = app.wait_screen(&format!("sql {marker}"), |screen| {
        let visible = text(screen);
        visible.contains(marker) && (marker == "zzsyntax" || !visible.contains("zzsyntax"))
    });
    assert!(
        text(&screen).contains(marker),
        "paste missed {marker}\n{}",
        text(&screen)
    );
}

fn run_key(app: &App, key: &str, needle: &str) -> TerminalSnapshot {
    idle(app);
    let before = text(&app.screen());
    app.key(key);
    app.wait_screen("screen changed", |screen| text(screen) != before);
    app.wait_screen(needle, |screen| {
        let visible = text(screen);
        !busy(screen) && visible.contains(needle)
    })
}

fn confirm(app: &App, needle: &str) -> TerminalSnapshot {
    let before = text(&app.screen());
    app.key("enter");
    app.wait_screen("confirm changed", |screen| text(screen) != before);
    app.wait_screen(needle, |screen| {
        let visible = text(screen);
        !busy(screen) && visible.contains(needle)
    })
}

fn dismiss(app: &App) {
    let before = text(&app.screen());
    app.key("escape");
    app.wait_screen("dialog dismissed", |screen| {
        let visible = text(screen);
        visible != before
            && visible.contains(CANCELLED)
            && !visible.contains("permanently modify")
            && !visible.contains(QUERY_TITLE)
            && !visible.contains("to confirm")
    });
}

fn deny_key(app: &App, key: &str) -> TerminalSnapshot {
    // The deny status stays for five seconds. A repeat of the same sentence
    // does not change the screen, so wait until the previous one expires.
    app.expire_status();
    app.wait_screen("deny status clear", |screen| {
        !text(screen).contains("Cannot execute")
    });
    let screen = run_key(app, key, "Cannot execute");
    see(
        &screen,
        &[DENY, "No results yet"],
        &["rows affected", "syntax error"],
    );
    screen
}

fn blur(app: &App) {
    if text(&app.screen()).contains("EDIT") {
        app.key("f6");
        app.wait_screen("left editor", |screen| !text(screen).contains("EDIT"));
    }
}

fn set_mode(app: &App, from: usize, to: usize, label: &str) {
    blur(app);
    app.key("ctrl+l");
    app.wait_text("Safe Mode · this connection");
    let key = if to > from { "down" } else { "up" };
    for _ in 0..to.abs_diff(from) {
        app.key(key);
    }
    app.key("enter");
    let applied = app.wait_screen(&format!("mode {label}"), |screen| {
        let visible = text(screen);
        visible.contains(&format!("Safety level set to {label}"))
            && !visible.contains(&format!("Safety level set to {label} ("))
            && !visible.contains("this connection")
    });
    assert!(
        text(&applied).contains(&format!("Safety level set to {label}")),
        "{}",
        text(&applied)
    );
}

fn quiet(app: &App) {
    // Shell status expires after its lifetime. The checkpoint has to be taken
    // after that, or the footer changes under the open dialog.
    app.expire_status();
    app.wait_screen("footer status settled", |screen| {
        let visible = text(screen);
        !visible.contains("Connected to ")
            && !visible.contains("Nothing to run")
            && !visible.contains("Cancelled")
            && !visible.contains("Safety level")
    });
}

fn no_dialog(screen: &TerminalSnapshot) {
    see(screen, &[], &[DANGER, QUERY_TITLE, "to confirm"]);
}

fn finish_dirty(app: &App) {
    app.key("ctrl+q");
    app.wait_text("Quit TablePro?");
    app.key("right");
    app.key("enter");
    app.wait_exit("Quit did not exit");
    app.expect_exited(0);
}

fn open_connected(name: &str) -> App {
    let app = open(name);
    connect(&app);
    app
}

fn open_in_mode(name: &str, level: usize, label: &str) -> App {
    let app = open_connected(name);
    set_mode(&app, 0, level, label);
    app
}

/// The deny status stays for five seconds on the wall clock, and a repeat of
/// the same sentence does not change the screen. Each session therefore
/// issues one denial instead of waiting the status out between them.
fn open_read_only(name: &str) -> App {
    let app = open_in_mode(name, 5, "Read-Only");
    header_token(&app.screen(), "read-only");
    app.key("ctrl+t");
    let fresh = app.wait_screen("query 2", |screen| {
        let visible = text(screen);
        visible.contains("Query 2")
            && visible.contains("Type SQL")
            && visible.contains("No results yet")
    });
    see(&fresh, &["No results yet", "Type SQL"], &["rows affected"]);
    app
}

#[test]
fn route_lands_on_the_query_editor() {
    let app = open("t03-query-route");
    connect(&app);
    let landed = app.screen();
    see(
        &landed,
        &["Type SQL", "Query 1", "Connected to Development"],
        &[
            "Loading schema",
            DANGER,
            QUERY_TITLE,
            "Safe Mode · this connection",
        ],
    );
    header_token(&landed, "silent");
    app.finish();
}

#[test]
fn classify_silent_shows_the_safety_decision() {
    let app = open_connected("t03-query-classify-silent");
    let connected = app.screen();
    header_token(&connected, "silent");
    see(&connected, &["Type SQL", "Query 1"], &[DANGER, QUERY_TITLE]);

    put_sql(&app, "zzsyntax", "zzsyntax");
    let syntax = run_key(&app, "ctrl+r", "syntax error");
    see(
        &syntax,
        &[
            "syntax error at or near \"ZZSYNTAX\"",
            "Error 1",
            "failed · 1 ms",
            "zzsyntax",
        ],
        &[DENY, DANGER, QUERY_TITLE],
    );

    put_sql(
        &app,
        "-- keep; this comment\nSELECT name FROM organizations WHERE slug = 'a;b'",
        "a;b",
    );
    let comment = run_key(&app, "ctrl+r", "No rows");
    see(
        &comment,
        &[
            "-- keep; this comment",
            "slug = 'a;b'",
            "SELECT organizations (0)",
            "No rows · 5 ms",
        ],
        &["syntax error", DANGER, "SELECT organizations (500)"],
    );

    put_sql(
        &app,
        "UPDATE products SET name = 'scoped_ok' WHERE sku = 'SKU-01000'",
        "scoped_ok",
    );
    let scoped = run_key(&app, "ctrl+r", "2 rows affected");
    see(
        &scoped,
        &[
            "scoped_ok",
            "UPDATE products (2)",
            "UPDATE products · 2 rows affected · 12 ms",
            "Statement executed",
        ],
        &[DANGER, WRITE_TITLE, "UPDATE without WHERE"],
    );

    put_sql(
        &app,
        "UPDATE products SET name = 'unscoped_ok'",
        "unscoped_ok",
    );
    let unscoped = run_key(&app, "ctrl+r", "312 rows affected");
    see(
        &unscoped,
        &[
            "unscoped_ok",
            "UPDATE products (312)",
            "UPDATE products · 312 rows affected · 40 ms",
        ],
        &[DANGER, WRITE_TITLE, "UPDATE without WHERE"],
    );
    no_dialog(&unscoped);

    quiet(&app);
    put_sql(&app, "DELETE FROM orders", "DELETE FROM orders");
    let dialog = run_key(&app, "ctrl+r", DANGER);
    see(
        &dialog,
        &[
            DANGER,
            DELETE_ACTION,
            "every row in orders (1.2 M rows)",
            "Development · development · acme_prod · orders",
            SILENT_FACT,
            "Execute",
            "Not reversible without a backup",
        ],
        &[WRITE_TITLE, "Type orders to confirm", "rows affected"],
    );
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    shot(&app, "T03.query.classify.120x40");

    wait_size(&app, 80, 24);
    let narrow = app.wait_screen("narrow delete dialog", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(DANGER)
            && visible.contains(DELETE_ACTION)
    });
    see(
        &narrow,
        &[DANGER, DELETE_ACTION, SILENT_FACT, "Execute", "1.2 M rows"],
        &[WRITE_TITLE, "Type orders to confirm"],
    );
    shot(&app, "T03.query.classify.narrow");

    wait_size(&app, 120, 40);
    let restored = app.wait_screen("delete dialog restored", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(DANGER)
            && visible.contains(DELETE_ACTION)
    });
    see(&restored, &[DANGER, SILENT_FACT], &[WRITE_TITLE]);
    dismiss(&app);

    put_sql(&app, "", "Type SQL");
    let empty = run_key(&app, "ctrl+r", "Nothing to run");
    see(&empty, &["Nothing to run", "Type SQL"], &[DANGER]);

    for (sql, marker, action, scope) in [
        (
            "DROP TABLE products",
            "DROP TABLE",
            "DROP TABLE",
            "products, its 312 rows, indexes and constraints",
        ),
        (
            "TRUNCATE TABLE products",
            "TRUNCATE",
            "TRUNCATE",
            "every row in products (312 rows)",
        ),
        (
            "ALTER TABLE products DROP COLUMN name",
            "DROP COLUMN",
            "ALTER TABLE with DROP",
            "products structure",
        ),
    ] {
        put_sql(&app, sql, marker);
        let dialog = run_key(&app, "ctrl+r", DANGER);
        see(
            &dialog,
            &[DANGER, action, scope, SILENT_FACT, "Execute"],
            &[WRITE_TITLE, "to confirm"],
        );
        dismiss(&app);
    }

    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let explain = run_key(&app, "ctrl+x", "Planning");
    see(
        &explain,
        &[
            "EXPLAIN",
            "Planning 0.210 ms · r Raw",
            "Seq Scan on public.organizations",
        ],
        &[DANGER, "EXPLAIN ANALYZE", "Execution"],
    );
    let analyze = run_key(&app, "alt+x", "Execution");
    see(
        &analyze,
        &[
            "EXPLAIN ANALYZE",
            "Planning 0.210 ms · Execution 0.660 ms",
            "Seq Scan on public.organizations",
        ],
        &[DANGER],
    );

    put_sql(
        &app,
        "SELECT name FROM organizations; UPDATE products SET name = 'mix_ok' WHERE sku = 'SKU-01000'",
        "mix_ok",
    );
    let mixed = run_key(&app, "alt+r", "SELECT organizations (500)");
    see(
        &mixed,
        &[
            "SELECT organizations (500)",
            "UPDATE products (2)",
            "UPDATE products · 2 rows affected · 12 ms",
            "mix_ok",
        ],
        &[DANGER, WRITE_TITLE],
    );
    finish_dirty(&app);
}

#[test]
fn classify_alert_confirms_writes() {
    let app = open_in_mode("t03-query-classify-alert", 1, "Alert");
    header_token(&app.screen(), "alert");
    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let alert_select = run_key(&app, "ctrl+r", "Showing 500 rows");
    see(
        &alert_select,
        &[SELECT_500, "Northwind Labs", "SELECT organizations (500)"],
        &[DANGER, QUERY_TITLE],
    );
    put_sql(
        &app,
        "UPDATE products SET name = 'alert_ok' WHERE sku = 'SKU-01000'",
        "alert_ok",
    );
    let alert_write = run_key(&app, "ctrl+r", WRITE_TITLE);
    see(
        &alert_write,
        &[
            WRITE_TITLE,
            "Action",
            "UPDATE",
            "Alert · confirm before running",
            "Execute",
            "alert_ok",
        ],
        &[DANGER, "to confirm", "UPDATE without WHERE"],
    );
    let alert_done = confirm(&app, "2 rows affected");
    see(
        &alert_done,
        &["UPDATE products · 2 rows affected · 12 ms"],
        &[WRITE_TITLE],
    );

    put_sql(&app, "UPDATE products SET name = 'alert_all'", "alert_all");
    let alert_all = run_key(&app, "ctrl+r", "UPDATE without WHERE");
    see(
        &alert_all,
        &[
            WRITE_TITLE,
            "UPDATE without WHERE",
            "every row in products (312 rows)",
            "Alert · confirm before running",
        ],
        &[DANGER, "to confirm"],
    );
    dismiss(&app);

    put_sql(
        &app,
        "SELECT name FROM organizations; UPDATE products SET name = 'tie_ok' WHERE sku = 'SKU-01000'; DROP TABLE products",
        "DROP TABLE",
    );
    let tie = run_key(&app, "alt+r", DANGER);
    see(
        &tie,
        &[
            DANGER,
            "DROP TABLE",
            "products, its 312 rows, indexes and constraints",
            "Alert · confirm before running",
            "SELECT name FROM organizations",
            "tie_ok",
        ],
        &[WRITE_TITLE, "to confirm"],
    );
    dismiss(&app);

    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let alert_explain = run_key(&app, "ctrl+x", "Planning");
    see(
        &alert_explain,
        &["Planning 0.210 ms · r Raw", "EXPLAIN"],
        &[DANGER, QUERY_TITLE],
    );
    let alert_analyze = run_key(&app, "alt+x", "Execution");
    see(
        &alert_analyze,
        &["Execution 0.660 ms", "EXPLAIN ANALYZE"],
        &[DANGER],
    );
    put_sql(
        &app,
        "UPDATE products SET name = 'exp_ok' WHERE sku = 'SKU-01000'",
        "exp_ok",
    );
    let explain_write = run_key(&app, "ctrl+x", "EXPLAIN ANALYZE runs UPDATE");
    see(
        &explain_write,
        &[
            WRITE_TITLE,
            "EXPLAIN ANALYZE runs UPDATE",
            "Alert · confirm before running",
            "exp_ok",
        ],
        &[DANGER, "to confirm"],
    );
    dismiss(&app);
    finish_dirty(&app);
}

#[test]
fn classify_alert_full_confirms_reads() {
    let app = open_in_mode("t03-query-classify-alert-full", 2, "Alert (Full)");
    header_token(&app.screen(), "alert+");
    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let full_select = run_key(&app, "ctrl+r", QUERY_TITLE);
    see(
        &full_select,
        &[
            QUERY_TITLE,
            "SELECT",
            "Alert (Full) · confirm before running",
            "organizations",
        ],
        &[WRITE_TITLE, DANGER, "to confirm"],
    );
    let full_done = confirm(&app, "Showing 500 rows");
    see(&full_done, &[SELECT_500, "Northwind Labs"], &[QUERY_TITLE]);

    put_sql(
        &app,
        "SELECT name FROM organizations; UPDATE products SET name = 'full_tie' WHERE sku = 'SKU-01000'",
        "full_tie",
    );
    let full_tie = run_key(&app, "alt+r", WRITE_TITLE);
    see(
        &full_tie,
        &[
            WRITE_TITLE,
            "UPDATE",
            "Alert (Full) · confirm before running",
            "full_tie",
            "SELECT name FROM organizations",
        ],
        &[DANGER, "to confirm"],
    );
    dismiss(&app);

    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let full_explain = run_key(&app, "ctrl+x", "EXPLAIN ANALYZE runs SELECT");
    see(
        &full_explain,
        &[
            QUERY_TITLE,
            "EXPLAIN ANALYZE runs SELECT",
            "Alert (Full) · confirm before running",
        ],
        &[WRITE_TITLE, DANGER, "to confirm"],
    );
    dismiss(&app);
    finish_dirty(&app);
}

#[test]
fn classify_safe_mode_types_to_confirm() {
    let app = open_in_mode("t03-query-classify-safe", 3, "Safe Mode");
    header_token(&app.screen(), "safe");
    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let safe_select = run_key(&app, "ctrl+r", "Showing 500 rows");
    see(
        &safe_select,
        &[SELECT_500],
        &[DANGER, QUERY_TITLE, "to confirm"],
    );

    put_sql(
        &app,
        "UPDATE products SET name = 'safe_ok' WHERE sku = 'SKU-01000'",
        "safe_ok",
    );
    let safe_write = run_key(&app, "ctrl+r", "Type products to confirm");
    see(
        &safe_write,
        &[
            WRITE_TITLE,
            "Type products to confirm",
            "Safe Mode · type to confirm",
            "safe_ok",
        ],
        &[DANGER, "Safe Mode (Full)"],
    );
    app.type_text("products");
    let safe_done = confirm(&app, "2 rows affected");
    see(
        &safe_done,
        &["UPDATE products · 2 rows affected · 12 ms"],
        &["to confirm"],
    );

    put_sql(&app, "UPDATE products SET name = 'safe_all'", "safe_all");
    let safe_all = run_key(&app, "ctrl+r", "UPDATE without WHERE");
    see(
        &safe_all,
        &[
            WRITE_TITLE,
            "UPDATE without WHERE",
            "Type products to confirm",
            "Safe Mode · type to confirm",
        ],
        &[DANGER],
    );
    dismiss(&app);

    put_sql(&app, "DELETE FROM orders", "DELETE FROM orders");
    let safe_delete = run_key(&app, "ctrl+r", "Type orders to confirm");
    see(
        &safe_delete,
        &[
            DANGER,
            DELETE_ACTION,
            "Type orders to confirm",
            "Safe Mode · type to confirm",
        ],
        &[WRITE_TITLE],
    );
    dismiss(&app);

    put_sql(&app, "VACUUM", "VACUUM");
    let vacuum = run_key(&app, "ctrl+r", "Type yes to confirm");
    see(
        &vacuum,
        &[
            WRITE_TITLE,
            "VACUUM",
            "Type yes to confirm",
            "Safe Mode · type to confirm",
        ],
        &[DANGER, "Type products"],
    );
    dismiss(&app);

    put_sql(
        &app,
        "SELECT name FROM organizations; UPDATE products SET name = 'safe_mix' WHERE sku = 'SKU-01000'",
        "safe_mix",
    );
    let safe_mix = run_key(&app, "alt+r", "Type products to confirm");
    see(
        &safe_mix,
        &[
            WRITE_TITLE,
            "Type products to confirm",
            "Safe Mode · type to confirm",
            "safe_mix",
            "SELECT name FROM organizations",
        ],
        &[DANGER],
    );
    dismiss(&app);
    finish_dirty(&app);
}

#[test]
fn classify_safe_mode_full_types_for_reads() {
    let app = open_in_mode("t03-query-classify-safe-full", 4, "Safe Mode (Full)");
    header_token(&app.screen(), "safe+");
    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let full_safe = run_key(&app, "ctrl+r", "Type organizations to confirm");
    see(
        &full_safe,
        &[
            QUERY_TITLE,
            "Type organizations to confirm",
            "Safe Mode (Full) · type to confirm",
        ],
        &[WRITE_TITLE, DANGER],
    );
    app.type_text("organizations");
    let full_safe_done = confirm(&app, "Showing 500 rows");
    see(&full_safe_done, &[SELECT_500], &["to confirm"]);

    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let full_safe_explain = run_key(&app, "ctrl+x", "Type organizations to confirm");
    see(
        &full_safe_explain,
        &[
            QUERY_TITLE,
            "EXPLAIN ANALYZE runs SELECT",
            "Type organizations to confirm",
            "Safe Mode (Full) · type to confirm",
        ],
        &[WRITE_TITLE, DANGER],
    );
    dismiss(&app);
    finish_dirty(&app);
}

#[test]
fn classify_read_only_denies_run() {
    let app = open_read_only("t03-query-read-only-run");
    let empty = run_key(&app, "ctrl+r", "Nothing to run");
    see(&empty, &["Nothing to run", "No results yet"], &[DENY]);

    put_sql(
        &app,
        "UPDATE products SET name = 'denied_marker_ro' WHERE sku = 'SKU-01000'",
        "denied_marker_ro",
    );
    let denied = deny_key(&app, "ctrl+r");
    see(
        &denied,
        &[DENY, "No results yet", "denied_marker_ro"],
        &["UPDATE products (", "rows affected"],
    );
    header_token(&denied, "read-only");

    app.key("ctrl+y");
    let history = app.wait_text("Query history");
    assert!(
        !text(&history).contains("denied_marker_ro"),
        "denied statement was listed\n{}",
        text(&history)
    );
    app.key("/");
    // Paste writes the field without refreshing the list. Typing does.
    // The empty-history line appears on a prefix, before the last letters paint.
    app.type_text("denied_marker_ro");
    let missing = app.wait_screen("history filter shows the denied token", |screen| {
        let visible = text(screen);
        visible.contains("No matching queries") && visible.contains("denied_marker_ro")
    });
    see(&missing, &["No matching queries", "denied_marker_ro"], &[]);
    app.key("escape");
    app.wait_absent("No matching queries");
    let back = app.screen();
    let tabs: Vec<_> = find_all(&back, "Query 2")
        .into_iter()
        .filter(|point| point.y < 8)
        .collect();
    assert!(!tabs.is_empty(), "Query 2 tab\n{}", text(&back));
    app.click(tabs[0]);
    app.wait_text("denied_marker_ro");
    finish_dirty(&app);
}

#[test]
fn classify_read_only_denies_explain() {
    let app = open_read_only("t03-query-read-only-explain");
    put_sql(
        &app,
        "UPDATE products SET name = 'denied_explain' WHERE sku = 'SKU-01000'",
        "denied_explain",
    );
    let explained = deny_key(&app, "ctrl+x");
    see(
        &explained,
        &[DENY, "No results yet", "denied_explain"],
        &["EXPLAIN", "Planning"],
    );
    finish_dirty(&app);
}

#[test]
fn classify_read_only_denies_a_script_with_a_write() {
    let app = open_read_only("t03-query-read-only-skip");
    put_sql(
        &app,
        "zzskip; UPDATE products SET name = 'denied_skip' WHERE sku = 'SKU-01000'",
        "denied_skip",
    );
    let skipped = deny_key(&app, "alt+r");
    see(
        &skipped,
        &[DENY, "No results yet", "zzskip", "denied_skip"],
        &["syntax error", "ZZSKIP", "Error 1"],
    );
    finish_dirty(&app);
}

#[test]
fn classify_read_only_denies_a_mixed_script() {
    let app = open_read_only("t03-query-read-only-mix");
    put_sql(
        &app,
        "SELECT name FROM organizations; UPDATE products SET name = 'denied_mix' WHERE sku = 'SKU-01000'",
        "denied_mix",
    );
    let denied_mix = deny_key(&app, "alt+r");
    see(
        &denied_mix,
        &[DENY, "No results yet", "denied_mix"],
        &["SELECT organizations", "rows affected"],
    );
    finish_dirty(&app);
}

#[test]
fn classify_read_only_still_reads() {
    let app = open_read_only("t03-query-read-only-reads");
    // A successful read does not replace the shell status, so a denial made
    // earlier in this app stays on screen until its wall-clock TTL. Issue one
    // denial first, then wait it out before asserting the reads themselves.
    put_sql(
        &app,
        "UPDATE products SET name = 'denied_reads' WHERE sku = 'SKU-01000'",
        "denied_reads",
    );
    deny_key(&app, "ctrl+r");
    app.expire_status();
    app.wait_screen("read-only deny expired", |screen| {
        !text(screen).contains("Cannot execute")
    });
    put_sql(&app, "SELECT name FROM organizations", "FROM organizations");
    let read_select = run_key(&app, "ctrl+r", "Showing 500 rows");
    see(
        &read_select,
        &[SELECT_500, "Northwind Labs"],
        &[DENY, DANGER],
    );
    let read_explain = run_key(&app, "ctrl+x", "Planning");
    see(
        &read_explain,
        &["EXPLAIN", "Planning 0.210 ms · r Raw"],
        &[DENY, DANGER],
    );
    let read_analyze = run_key(&app, "alt+x", "Execution");
    see(
        &read_analyze,
        &["EXPLAIN ANALYZE", "Execution 0.660 ms"],
        &[DENY],
    );
    finish_dirty(&app);
}
