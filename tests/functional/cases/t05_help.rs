//! TablePro Help shows its title, scrolls, and returns to the opener.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_help/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const QUERY_NEXT: &str = "Query 2";
const SQL: &str = "SELECT sku FROM products WHERE sku = 't05_help_opener';";
const MARKER: &str = "t05_help_opener";
const PLACEHOLDER: &str = "Type SQL";
const HELP_TITLE: &str = "Keyboard shortcuts";
const SCOPE: &str = "TablePro";
const SHELL: &str = "Shell and workbench";
const GRID: &str = "Grid and editor";
const WIDE: &str = "T05.help.help.120x40";
const NARROW: &str = "T05.help.help.narrow";
const ACTIVE_RULE: char = '━';
const IDLE_RULE: char = '─';

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_help/{}",
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
            HELP_TITLE,
        ],
    );
}

fn help_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(HELP_TITLE) && visible.contains(SCOPE) && visible.contains("Done")
}

fn tab_rule(screen: &TerminalSnapshot, label: &str) -> Option<char> {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.y <= 3)
        .collect();
    let [hit] = hits.as_slice() else {
        return None;
    };
    let below = row_text(screen, u32::from(hit.y + 1));
    Some(below.chars().nth(usize::from(hit.x)).unwrap_or(' '))
}

fn query_1_active(screen: &TerminalSnapshot) -> bool {
    tab_rule(screen, QUERY) == Some(ACTIVE_RULE) && tab_rule(screen, QUERY_NEXT) == Some(IDLE_RULE)
}

fn assert_query_1_active(screen: &TerminalSnapshot) {
    assert!(
        query_1_active(screen),
        "Query 1 rule {:?} Query 2 rule {:?}\n{}",
        tab_rule(screen, QUERY),
        tab_rule(screen, QUERY_NEXT),
        text(screen)
    );
}

fn open_help(app: &App) {
    app.key("f1");
    let help = app.wait_screen("help title", help_open);
    see(
        &help,
        &[HELP_TITLE, SCOPE, SHELL, GRID, "Done"],
        &["Filter connections", "Loading schema"],
    );
}

fn click_editor(app: &App) {
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
}

fn type_opener(app: &App) {
    click_editor(app);
    app.type_text(SQL);
    // The marker is on screen before the closing quote and semicolon.
    let typed = app.wait_screen("opener sql", |screen| text(screen).contains(SQL));
    see(
        &typed,
        &[SQL, QUERY, CONNECTED],
        &[HELP_TITLE, QUERY_NEXT, "Execute query?"],
    );
}

fn shot(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    let before = app.screen();
    assert_eq!(
        (before.cols(), before.rows()),
        (cols, rows),
        "checkpoint {checkpoint}\n{}",
        text(&before)
    );
    assert!(
        help_open(&before),
        "checkpoint {checkpoint} left Help\n{}",
        text(&before)
    );
    app.visual(&visual_id(checkpoint));
    let after = app.screen();
    assert!(
        help_open(&after),
        "checkpoint {checkpoint} did not stay on Help\n{}",
        text(&after)
    );
    assert_eq!((after.cols(), after.rows()), (cols, rows));
}

fn row_left(before: &str, after: &str) -> bool {
    let after_lines: Vec<&str> = after.lines().collect();
    before.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !after_lines.contains(&line)
    })
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &[HELP_TITLE]);
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
fn tablepro_help_route_returns_to_query_1() {
    let app = open("t05-help-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Filter connections", "Development"],
        &[QUERY, HELP_TITLE],
    );
    connect_development(&app);
    open_help(&app);
    app.key("esc");
    let back = app.wait_screen("query 1 after help", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains(PLACEHOLDER)
            && visible.contains(CONNECTED)
            && !visible.contains(HELP_TITLE)
            && !visible.contains(SHELL)
    });
    see(
        &back,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &[HELP_TITLE, SHELL, GRID, QUERY_NEXT, "Loading schema"],
    );
    app.finish();
}

#[test]
fn tablepro_help_scrolls_and_returns_to_the_opener() {
    let app = open("t05-help-scroll");
    connect_development(&app);
    type_opener(&app);

    app.key("ctrl+t");
    let second = app.wait_screen("query 2", |screen| {
        let visible = text(screen);
        visible.contains(QUERY_NEXT) && visible.contains(PLACEHOLDER) && !visible.contains(MARKER)
    });
    see(
        &second,
        &[QUERY_NEXT, PLACEHOLDER, QUERY],
        &[MARKER, HELP_TITLE],
    );

    app.key("ctrl+pageup");
    let opener = app.wait_screen("query 1 opener", |screen| {
        let visible = text(screen);
        visible.contains(SQL) && visible.contains(QUERY) && !visible.contains(PLACEHOLDER)
    });
    assert_query_1_active(&opener);
    see(
        &opener,
        &[SQL, QUERY, QUERY_NEXT, CONNECTED],
        &[HELP_TITLE, PLACEHOLDER],
    );

    open_help(&app);
    let opened = app.screen();
    let opened_text = text(&opened);
    assert_query_1_active(&opened);
    assert!(
        opened_text.contains(SHELL) && opened_text.contains(GRID),
        "Help did not open at the top\n{opened_text}"
    );

    // The workbench page chord reaches Help's layer first. It must not activate Query 2.
    app.key("ctrl+pagedown");
    let held = app.wait_screen("help holds query 1", |screen| {
        help_open(screen) && query_1_active(screen) && text(screen) != opened_text
    });
    assert_query_1_active(&held);
    see(&held, &[HELP_TITLE, SCOPE], &[PLACEHOLDER]);

    app.key("pageup");
    let top = app.wait_screen("help back at the top", |screen| {
        let visible = text(screen);
        help_open(screen)
            && query_1_active(screen)
            && visible.contains(SHELL)
            && visible.contains(GRID)
    });
    let top_text = text(&top);
    app.key("pagedown");
    let scrolled = app.wait_screen("help scrolled", |screen| {
        help_open(screen) && query_1_active(screen) && text(screen) != top_text
    });
    let scrolled_text = text(&scrolled);
    assert!(
        row_left(&top_text, &scrolled_text),
        "no visible row changed\n{top_text}\n---\n{scrolled_text}"
    );
    assert!(
        scrolled_text.contains(HELP_TITLE) && scrolled_text.contains(SCOPE),
        "Help title left after PageDown\n{scrolled_text}"
    );
    assert_query_1_active(&scrolled);
    assert_eq!((scrolled.cols(), scrolled.rows()), (120, 40));

    shot(&app, WIDE, 120, 40);
    app.resize(80, 24);
    let narrowed = app.wait_screen("help at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && help_open(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(&narrowed, &[HELP_TITLE, SCOPE], &[PLACEHOLDER]);
    shot(&app, NARROW, 80, 24);
    app.resize(120, 40);
    let restored = app.wait_screen("help back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && help_open(screen)
    });
    see(
        &restored,
        &[HELP_TITLE, SCOPE],
        &[PLACEHOLDER, "Filter connections"],
    );

    app.key("esc");
    let back = app.wait_screen("opener after help", |screen| {
        let visible = text(screen);
        visible.contains(MARKER)
            && visible.contains(QUERY)
            && visible.contains(SQL)
            && !visible.contains(HELP_TITLE)
            && !visible.contains(SHELL)
    });
    assert_query_1_active(&back);
    see(
        &back,
        &[SQL, MARKER, QUERY, QUERY_NEXT, CONNECTED],
        &[HELP_TITLE, PLACEHOLDER, GRID, "Loading schema"],
    );
    quit_unsaved(&app);
}
