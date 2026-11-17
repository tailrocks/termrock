//! TablePro affected-row result: a scoped write replaces the previous row grid.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_affected/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const SELECT_SQL: &str = "SELECT sku FROM products LIMIT 1";
const UPDATE: &str = "UPDATE products SET name = name WHERE sku = 't03-affected'";
const PLACEHOLDER: &str = "Type SQL";
const RUNNING: &str = "Esc cancels";
const EXECUTED: &str = "Statement executed";
const AFFECTED: &str = "Rows affected";
const STATUS: &str = "UPDATE products · 2 rows affected · 12 ms";
const HISTORY: &str = "Query history";
const SELECT_TAB: &str = "SELECT products";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_affected/{}",
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

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn select_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("sku")
        && visible.contains("1 row")
        && visible.contains(SELECT_SQL)
        && !visible.contains(RUNNING)
        && !visible.contains("Loading schema")
        && !visible.contains("Execute query?")
        && !visible.contains("syntax error")
}

fn remembered_cell(screen: &TerminalSnapshot) -> String {
    let visible = text(screen);
    let mut rest = visible.as_str();
    while let Some(at) = rest.find("SKU-") {
        let tail = &rest[at..];
        let end = tail
            .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
            .unwrap_or(tail.len());
        let token = &tail[..end];
        if !UPDATE.contains(token) {
            return token.to_string();
        }
        rest = &tail[end..];
    }
    panic!("select result has no cell outside the write\n{visible}");
}

fn affected_body(screen: &TerminalSnapshot, cell: &str) -> bool {
    let visible = text(screen);
    visible.contains(EXECUTED)
        && visible.contains(AFFECTED)
        && visible.contains(STATUS)
        && visible.contains(UPDATE)
        && visible.contains("12 ms")
        && !visible.contains(cell)
        && !visible.contains(SELECT_TAB)
        && !visible.contains(RUNNING)
        && !visible.contains("Execute write query?")
        && !visible.contains("1 row")
        && !visible.contains(HISTORY)
}

fn type_statement(app: &App, sql: &str) {
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
    app.type_text(sql);
    app.wait_text(sql);
}

fn replace_statement(app: &App) {
    let editor = app.wait_text(SELECT_SQL);
    let hits = find_all(&editor, "LIMIT 1");
    assert_eq!(hits.len(), 1, "statement hit {hits:?}\n{}", text(&editor));
    app.click(hits[0]);
    app.wait_screen("editor still holds the select", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains('▎') && line.contains("LIMIT 1")
        })
    });
    // macOS field keys are Emacs. Ctrl+L is Safe Mode, so line-start plus
    // kill-to-end replaces the statement without opening that picker.
    app.key("ctrl+a");
    app.key("ctrl+k");
    app.type_text(UPDATE);
    app.wait_screen("editor replaced with the scoped update", |screen| {
        let visible = text(screen);
        visible.contains(UPDATE) && !visible.contains("LIMIT 1")
    });
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &[HISTORY]);
    app.confirm_quit();
}

#[test]
fn tablepro_affected_route_reaches_query_1() {
    let app = open("t03-affected-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &[HISTORY, "Loading schema", EXECUTED],
    );
    app.finish();
}

#[test]
fn tablepro_affected_write_shows_the_affected_body() {
    let app = open("t03-affected-write");
    connect_development(&app);
    type_statement(&app, SELECT_SQL);

    app.key("ctrl+r");
    let selected = app.wait_screen("select result is one sku row", select_ready);
    let cell = remembered_cell(&selected);
    see(
        &selected,
        &["sku", "1 row", &cell, SELECT_TAB],
        &[EXECUTED, AFFECTED, RUNNING, "Execute write query?"],
    );

    replace_statement(&app);
    app.key("ctrl+r");
    let written = app.wait_screen("affected card replaces the row grid", |screen| {
        affected_body(screen, &cell)
    });
    assert_eq!((written.cols(), written.rows()), (120, 40));
    see(
        &written,
        &[EXECUTED, AFFECTED, STATUS, UPDATE, "12 ms"],
        &[&cell, SELECT_TAB, RUNNING, "1 row"],
    );

    let wide = app.screen();
    assert!(affected_body(&wide, &cell), "{}", text(&wide));
    shot(&app, "T03.affected.affected.120x40");
    let after_wide = app.screen();
    assert_eq!((after_wide.cols(), after_wide.rows()), (120, 40));
    assert!(affected_body(&after_wide, &cell), "{}", text(&after_wide));

    app.resize(80, 24);
    let narrowed = app.wait_screen("affected card at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && affected_body(screen, &cell)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    let narrow = app.screen();
    assert!(affected_body(&narrow, &cell), "{}", text(&narrow));
    shot(&app, "T03.affected.affected.narrow");
    app.resize(120, 40);
    let restored = app.wait_screen("affected card back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && affected_body(screen, &cell)
    });
    assert!(affected_body(&restored, &cell), "{}", text(&restored));
    see(
        &restored,
        &[EXECUTED, AFFECTED, STATUS, UPDATE],
        &[&cell, SELECT_TAB],
    );

    app.key("ctrl+y");
    let history = app.wait_screen("history lists the scoped write", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY) && visible.contains("t03-affected")
    });
    // The detail wraps the statement, so the single-line SQL is not contiguous.
    see(
        &history,
        &[HISTORY, "t03-affected", "UPDATE products", "12 ms"],
        &[EXECUTED, &cell, SELECT_TAB],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query 1 after closing history", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains(EXECUTED)
            && visible.contains(AFFECTED)
            && !visible.contains(HISTORY)
    });
    assert!(affected_body(&back, &cell), "{}", text(&back));
    quit_unsaved(&app);
}
