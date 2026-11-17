//! TablePro SQL completion before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_completion/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::bins::tablepro;
use crate::support::{
    App, Launch, Point, cell_bg, find_all, flash_settle, row_text, text, try_launch,
};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";

/// Empty statement completion, shortest label first (`model::complete`).
const FIRST: &str = "WITH";
const ACCEPTED: &str = "BEGIN";
const OTHERS: &[&str] = &[
    "WITH", "COMMIT", "SELECT", "UPDATE", "EXPLAIN", "ROLLBACK", "TRUNCATE",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_completion/{}",
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
            "Loading schema",
        ],
    );
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
}

fn wait_completion(app: &App, cols: u16, rows: u16, reason: &str) -> TerminalSnapshot {
    app.resize(cols, rows);
    // The first frame after a resize can be blank, before the list is painted.
    app.wait_screen(reason, |screen| {
        let visible = text(screen);
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && visible.contains(ACCEPTED)
            && visible.contains("Enter Accept")
    })
}

fn focus_editor(app: &App) {
    let screen = app.wait_text(PLACEHOLDER);
    let hits: Vec<_> = find_all(&screen, PLACEHOLDER)
        .into_iter()
        .filter(|hit| hit.x > 20)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "editor placeholder {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    // Editing the query replaces the explorer hints with Ctrl+Space Complete.
    app.wait_text("Ctrl+Space");
}

fn label_point(screen: &TerminalSnapshot, label: &str) -> Point {
    let marker = format!("K {label}");
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| row_text(screen, u32::from(hit.y)).contains(&marker))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "completion label {label:?} got {hits:?}\n{}",
        text(screen)
    );
    hits[0]
}

fn label_bg(screen: &TerminalSnapshot, label: &str) -> tui_test::SnapshotColor {
    cell_bg(screen, label_point(screen, label))
}

fn lines_with(screen: &TerminalSnapshot, needle: &str) -> Vec<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .filter(|line| line.contains(needle))
        .collect()
}

fn editor_line(screen: &TerminalSnapshot) -> String {
    let hits = lines_with(screen, ACCEPTED);
    assert_eq!(
        hits.len(),
        1,
        "editor {ACCEPTED} lines {hits:?}\n{}",
        text(screen)
    );
    hits[0].clone()
}

fn popup_closed(screen: &TerminalSnapshot) {
    see(screen, &[], &["Enter Accept", "K WITH", "EXPLAIN ANALYZE"]);
}

fn assert_only_accepted(screen: &TerminalSnapshot) {
    let line = editor_line(screen);
    assert!(
        line.contains(ACCEPTED),
        "editor missed {ACCEPTED}: {line}\n{}",
        text(screen)
    );
    for other in OTHERS {
        assert!(
            !line.contains(other),
            "editor gained {other} instead of {ACCEPTED}: {line}"
        );
    }
    popup_closed(screen);
}

fn open_completion(app: &App) {
    app.key("ctrl+space");
    let opened = app.wait_text(FIRST);
    see(&opened, &[FIRST, ACCEPTED, "SELECT", "Enter Accept"], &[]);
}

fn move_to_accepted(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    let highlight = label_bg(&before, FIRST);
    assert_ne!(
        highlight,
        label_bg(&before, ACCEPTED),
        "first row was not {FIRST}\n{}",
        text(&before)
    );
    app.key("down");
    app.wait_screen("selection moved off the first row", |screen| {
        label_bg(screen, ACCEPTED) == highlight && label_bg(screen, FIRST) != highlight
    })
}

#[test]
fn route_lands_on_query_1_with_completion_closed() {
    let app = open("t03-completion-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[QUERY, PLACEHOLDER, "Loading schema"],
    );
    connect_development(&app);
    let query = app.screen();
    assert_eq!((query.cols(), query.rows()), (120, 40));
    see(
        &query,
        &[CONNECTED, QUERY, PLACEHOLDER],
        &[
            "Loading schema",
            "Scratch",
            "Enter Accept",
            "EXPLAIN ANALYZE",
            FIRST,
        ],
    );
    app.finish();
}

#[test]
fn complete_accepts_the_moved_item_and_dismiss_leaves_sql() {
    let app = open("t03-completion");
    connect_development(&app);
    focus_editor(&app);
    open_completion(&app);
    let moved = move_to_accepted(&app);
    assert_eq!((moved.cols(), moved.rows()), (120, 40));
    see(
        &moved,
        &[FIRST, ACCEPTED, "SELECT", "Enter Accept", PLACEHOLDER],
        &[],
    );
    shot(&app, "T03.completion.complete.120x40");

    let narrow = wait_completion(&app, 80, 24, "narrow completion");
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shot(&app, "T03.completion.complete.narrow");

    let restored = wait_completion(&app, 120, 40, "completion restored");
    assert_eq!((restored.cols(), restored.rows()), (120, 40));

    app.key("enter");
    let accepted = app.wait_screen("accepted completion", |screen| {
        lines_with(screen, ACCEPTED).len() == 1 && !text(screen).contains("Enter Accept")
    });
    assert_only_accepted(&accepted);

    // The caret is at the end of BEGIN. Ctrl+Left parks it on column 0 so the
    // next list is every statement keyword, and inserting the highlighted row
    // would change the SQL instead of replacing BEGIN with itself.
    app.key("ctrl+left");
    let before_dismiss = app.wait_screen("caret at the start of the accepted SQL", |screen| {
        lines_with(screen, ACCEPTED).len() == 1 && !text(screen).contains("Enter Accept")
    });
    let sql_before = editor_line(&before_dismiss);
    assert_only_accepted(&before_dismiss);

    app.key("ctrl+space");
    let again = app.wait_screen("second completion", |screen| {
        let visible = text(screen);
        visible.contains(FIRST)
            && visible.contains("COMMIT")
            && visible.contains("SELECT")
            && visible.contains("Enter Accept")
    });
    see(&again, &[FIRST, "COMMIT", "SELECT"], &["EXPLAIN ANALYZE"]);
    let highlight = label_bg(&again, FIRST);
    app.key("down");
    let highlighted = app.wait_screen("second selection moved", |screen| {
        text(screen).contains("Enter Accept")
            && label_bg(screen, ACCEPTED) == highlight
            && label_bg(screen, FIRST) != highlight
    });
    assert!(
        text(&highlighted).contains(ACCEPTED),
        "highlighted item left the list\n{}",
        text(&highlighted)
    );

    app.key("escape");
    let dismissed = app.wait_screen("completion dismissed", |screen| {
        lines_with(screen, ACCEPTED).len() == 1 && !text(screen).contains("Enter Accept")
    });
    assert_eq!(
        editor_line(&dismissed),
        sql_before,
        "dismiss changed the SQL\n{}",
        text(&dismissed)
    );
    assert_only_accepted(&dismissed);
    assert!(
        !editor_line(&dismissed).contains(&format!("{ACCEPTED}{ACCEPTED}")),
        "dismiss inserted {ACCEPTED}\n{}",
        editor_line(&dismissed)
    );
    // BEGIN is unsaved, so Ctrl+Q asks before it exits.
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(
        &quit,
        &["Quit TablePro?", "1 unsaved query will be lost.", ACCEPTED],
        &["Enter Accept", "BEGINBEGIN"],
    );
    app.confirm_quit();
}
