//! TablePro query error: a syntax error is visible, and correcting it removes it.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_error/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";
const BAD: &str = "SELEC sku FROM products";
const FIXED: &str = "SELECT sku FROM products LIMIT 1";
const MESSAGE: &str = "syntax error at or near \"SELEC\"";
const FAILED: &str = "failed · 1 ms";
const ROW: &str = "1 row";
const DURATION: &str = "3 ms";
const SKU: &str = "SKU-01000";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_error/{}",
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

/// `SELEC` is a prefix of `SELECT`, so the typo is a whole token.
fn shows_typo(visible: &str) -> bool {
    visible
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| word == "SELEC")
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

fn error_visible(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(MESSAGE)
        && visible.contains("syntax error")
        && visible.contains("Error")
        && visible.contains(FAILED)
        && visible.contains(BAD)
        && !visible.contains(ROW)
        && !visible.contains("Esc cancels")
}

fn recovered(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(FIXED)
        && visible.contains(ROW)
        && visible.contains(DURATION)
        && visible.contains(SKU)
        && !visible.contains(MESSAGE)
        && !visible.contains("syntax error")
        && !visible.contains("Error")
        && !visible.contains(FAILED)
        && !visible.contains("Esc cancels")
        && !shows_typo(&visible)
}

fn focus_bad_statement(app: &App) {
    let screen = app.wait_text(BAD);
    let hits: Vec<_> = find_all(&screen, BAD)
        .into_iter()
        .filter(|hit| row_text(&screen, u32::from(hit.y)).contains("FROM"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "editor statement {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    app.wait_screen("bad statement focused", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains('▎') && line.contains(BAD)
        })
    });
}

fn replace_statement(app: &App) {
    focus_bad_statement(app);
    // Emacs on macOS: Ctrl+A is line start and Ctrl+K deletes to the line end.
    app.key("ctrl+a");
    app.key("ctrl+k");
    app.wait_screen("bad statement cleared", |screen| {
        !text(screen).contains(BAD) && text(screen).contains(MESSAGE)
    });
    app.type_text(FIXED);
    app.wait_text(FIXED);
}

fn type_bad_statement(app: &App) {
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
    app.type_text(BAD);
    app.wait_text(BAD);
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &["Query history"]);
    app.confirm_quit();
}

#[test]
fn tablepro_error_route_reaches_query_1() {
    let app = open("t03-error-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &["Loading schema", "syntax error"],
    );
    app.finish();
}

#[test]
fn tablepro_error_recover_replaces_the_syntax_error() {
    let app = open("t03-error-recover");
    connect_development(&app);
    type_bad_statement(&app);

    app.key("ctrl+r");
    let failed = app.wait_screen("syntax error stays", error_visible);
    assert_eq!((failed.cols(), failed.rows()), (120, 40));
    see(
        &failed,
        &[MESSAGE, "syntax error", "Error", FAILED, BAD],
        &[ROW, "Esc cancels"],
    );
    assert!(
        shows_typo(&text(&failed)),
        "typo token missing\n{}",
        text(&failed)
    );

    replace_statement(&app);
    app.key("ctrl+r");
    let fixed = app.wait_screen("corrected result stays", recovered);
    assert_eq!((fixed.cols(), fixed.rows()), (120, 40));
    see(
        &fixed,
        &[FIXED, ROW, DURATION, SKU, QUERY],
        &[MESSAGE, "syntax error", "Error", FAILED, "Esc cancels"],
    );
    assert!(
        !shows_typo(&text(&fixed)),
        "typo token remains\n{}",
        text(&fixed)
    );

    let wide = app.screen();
    assert!(recovered(&wide), "{}", text(&wide));
    shot(&app, "T03.error.recover.120x40");
    let after_wide = app.screen();
    assert_eq!((after_wide.cols(), after_wide.rows()), (120, 40));
    assert!(recovered(&after_wide), "{}", text(&after_wide));

    app.expire_status();
    app.wait_absent(CONNECTED);
    app.resize(80, 24);
    let narrowed = app.wait_screen("recovered result at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && recovered(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    let narrow = app.screen();
    assert!(recovered(&narrow), "{}", text(&narrow));
    shot(&app, "T03.error.recover.narrow");
    app.resize(120, 40);
    let restored = app.wait_screen("recovered result back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && recovered(screen)
    });
    assert!(recovered(&restored), "{}", text(&restored));
    see(
        &restored,
        &[FIXED, ROW, DURATION, SKU],
        &[MESSAGE, "Esc cancels"],
    );
    quit_unsaved(&app);
}
