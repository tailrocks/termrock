//! TablePro cancelled batch: Esc drops the rest of the batch.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_cancelled/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::bins::tablepro;
use crate::support::{App, Launch, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const FIRST_SQL: &str = "SELECT sku FROM products WHERE sku = 't03_cancelled_first';";
const SECOND_SQL: &str = "SELECT name FROM products WHERE name = 't03_cancelled_second'";
const FIRST: &str = "t03_cancelled_first";
const SECOND: &str = "t03_cancelled_second";
const PLACEHOLDER: &str = "Type SQL";
const RUNNING: &str = "Esc cancels";
const CANCELLED: &str = "Cancelled";
const HINT: &str = "The statement was stopped before it finished; nothing was recorded";
const RECORDED: &str = "nothing was recorded";
const RESULT_STATUS: &str = "Cancelled · nothing was recorded";
const STATUS: &str = "Query cancelled";
const HISTORY: &str = "Query history";
const WIDE: &str = "T03.cancelled.boundary.120x40";
const NARROW: &str = "T03.cancelled.boundary.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_cancelled/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, tablepro());
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

fn shot(app: &App, checkpoint: &str) {
    let before = app.screen();
    assert!(
        cancelled_body(&before),
        "checkpoint {checkpoint} left the cancelled body\n{}",
        text(&before)
    );
    app.visual(&visual_id(checkpoint));
    let after = app.screen();
    assert!(
        cancelled_body(&after),
        "checkpoint {checkpoint} did not stay cancelled\n{}",
        text(&after)
    );
}

fn running_line(screen: &TerminalSnapshot) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        let Some(at) = line.rfind(RUNNING) else {
            return false;
        };
        line.contains("running")
            && line[at + RUNNING.len()..]
                .chars()
                .all(|c| !c.is_ascii_alphanumeric())
    })
}

fn running_frame(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    running_line(screen)
        && visible.contains(FIRST)
        && visible.contains(SECOND)
        && !visible.contains(CANCELLED)
        && !visible.contains(RECORDED)
        && !visible.contains("Execute query?")
        && !visible.contains("Nothing to run")
        && !visible.contains("1 row")
        && !visible.contains("No rows")
        && !visible.contains("Statement executed")
        && !visible.contains("SELECT products")
        && !visible.contains("Query 2")
}

fn cancelled_body(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(CANCELLED)
        && visible.contains(HINT)
        && visible.contains(RESULT_STATUS)
        && visible.contains(FIRST)
        && visible.contains(SECOND)
        && !visible.contains(RUNNING)
        && !visible.contains("1 row")
        && !visible.contains("No rows")
        && !visible.contains("Statement executed")
        && !visible.contains("SELECT products")
        && !visible.contains("syntax error")
        && !visible.contains("Already running")
        && !visible.contains("Execute query?")
        && !visible.contains("Query 2")
        && !visible.contains("Error 1")
}

fn stayed(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(CANCELLED) && visible.contains(HINT) && !visible.contains("1 row")
}

fn type_statements(app: &App) {
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
    app.type_text(FIRST_SQL);
    app.key("enter");
    app.type_text(SECOND_SQL);
    // The token `t03_cancelled_second` is painted one frame before the
    // closing quote. The statement is present only when that quote is too.
    let typed = app.wait_screen("both statements", |screen| {
        let visible = text(screen);
        visible.contains(FIRST_SQL) && visible.contains(SECOND_SQL)
    });
    see(
        &typed,
        &[FIRST_SQL, SECOND_SQL, QUERY],
        &["Execute query?", CANCELLED, "Query 2"],
    );
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &["Query history"]);
    app.confirm_quit();
}

#[test]
fn tablepro_cancelled_route_reaches_query_1() {
    let app = open("t03-cancelled-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &["Loading schema", CANCELLED, "Query history", "Query 2"],
    );
    app.finish();
}

#[test]
fn tablepro_cancelled_batch_keeps_the_cancelled_body() {
    let app = open("t03-cancelled-boundary");
    connect_development(&app);
    type_statements(&app);

    // Reduced motion still starts the six-tick countdown. Esc has to land on
    // this running line, before that countdown paints a result grid.
    app.key("alt+r");
    let running = app.wait_screen("batch running line", running_frame);
    app.key("esc");
    see(
        &running,
        &[FIRST, SECOND, "running", RUNNING],
        &[
            CANCELLED,
            RECORDED,
            "1 row",
            "No rows",
            "Statement executed",
            "SELECT products",
            "Query 2",
        ],
    );

    let cancelled = app.wait_screen("cancelled body", cancelled_body);
    assert_eq!((cancelled.cols(), cancelled.rows()), (120, 40));
    let stayed_on = app.wait_screen("cancelled body stays", stayed);
    assert!(cancelled_body(&stayed_on), "{}", text(&stayed_on));
    see(
        &stayed_on,
        &[STATUS, RESULT_STATUS, HINT, FIRST, SECOND],
        &[
            "1 row",
            "No rows",
            "Statement executed",
            "SELECT products",
            RUNNING,
        ],
    );

    shot(&app, WIDE);
    app.resize(80, 24);
    let narrowed = app.wait_screen("cancelled body at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && cancelled_body(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("cancelled body back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && cancelled_body(screen)
    });
    assert!(stayed(&restored), "{}", text(&restored));
    see(
        &restored,
        &[CANCELLED, HINT, RESULT_STATUS, FIRST, SECOND],
        &["Query 2", "SELECT products", "1 row"],
    );

    app.key("ctrl+y");
    let history = app.wait_screen("history omits both cancelled statements", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY) && !visible.contains(FIRST) && !visible.contains(SECOND)
    });
    see(
        &history,
        &[HISTORY, "entries"],
        &[FIRST, SECOND, RECORDED, "Query 2"],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query 1 after closing history", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains(CANCELLED)
            && visible.contains(HINT)
            && !visible.contains(HISTORY)
    });
    assert!(cancelled_body(&back), "{}", text(&back));
    assert!(stayed(&back));
    quit_unsaved(&app);
}
