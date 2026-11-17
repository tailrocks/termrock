//! TablePro tab closing before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t04_tabs/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, TABLEPRO, find_all, row_text, text, try_launch};

const CONNECTED: &str = "Connected to Development";
const QUERY_1: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";
const CLOSE_TITLE: &str = "Close tab with unsaved work?";
const CLOSE_BODY: &str = "Pending row edits and unsaved query text in this tab will be lost.";
const CLOSE_OK: &str = "Close anyway";
const EMPTY: &str = "No open tabs";
const FIRST: &str = "t04_tabs_first";
const MID: &str = "t04_tabs_mid";
const LAST: &str = "t04_tabs_last";
const TAIL: &str = "t04_tabs_tail";
const FIRST_SQL: &str = "SELECT 't04_tabs_first' AS marker";
const MID_SQL: &str = "SELECT 't04_tabs_mid' AS marker";
const LAST_SQL: &str = "SELECT 't04_tabs_last' AS marker";
const TAIL_SQL: &str = "SELECT 't04_tabs_tail' AS marker";
const TITLES: [&str; 4] = ["Query 1", "Query 2", "Query 3", "Query 4"];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t04_tabs/{}",
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
    click_connect(app);
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY_1)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, QUERY_1, PLACEHOLDER],
        &["Authentication failed", "Loading schema"],
    );
}

fn click_match(app: &App, label: &str, keep: impl Fn(&TerminalSnapshot, u16, u16) -> bool) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| keep(&screen, hit.x, hit.y))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "click {label:?} got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn focus_editor(app: &App) {
    app.wait_text(PLACEHOLDER);
    click_match(app, PLACEHOLDER, |screen, x, y| {
        x > 20 && row_text(screen, u32::from(y)).contains(PLACEHOLDER)
    });
    app.wait_text("Ctrl+Space");
}

fn strip_y(screen: &TerminalSnapshot) -> u16 {
    let mut rows: Vec<u16> = find_all(screen, "×")
        .into_iter()
        .filter(|hit| row_text(screen, u32::from(hit.y)).contains("Query"))
        .map(|hit| hit.y)
        .collect();
    rows.sort_unstable();
    rows.dedup();
    assert_eq!(rows.len(), 1, "tab strip row {rows:?}\n{}", text(screen));
    rows[0]
}

fn title_hits(screen: &TerminalSnapshot) -> Vec<(u16, &'static str)> {
    let y = strip_y(screen);
    let mut hits: Vec<_> = TITLES
        .into_iter()
        .filter_map(|title| {
            find_all(screen, title)
                .into_iter()
                .find(|hit| hit.y == y)
                .map(|hit| (hit.x, title))
        })
        .collect();
    hits.sort_by_key(|(x, _)| *x);
    hits
}

fn click_tab(app: &App, title: &str) {
    let screen = app.wait_text(title);
    let y = strip_y(&screen);
    let hits: Vec<_> = find_all(&screen, title)
        .into_iter()
        .filter(|hit| hit.y == y)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "tab {title:?} on the strip: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_close(app: &App, title: &str) -> Point {
    let screen = app.screen();
    let y = strip_y(&screen);
    let titles = title_hits(&screen);
    let index = titles
        .iter()
        .position(|(_, name)| *name == title)
        .unwrap_or_else(|| panic!("strip has no {title:?}\n{}", text(&screen)));
    let start = titles[index].0;
    let end = titles.get(index + 1).map(|(x, _)| *x).unwrap_or(u16::MAX);
    let marks: Vec<_> = find_all(&screen, "×")
        .into_iter()
        .filter(|hit| hit.y == y && hit.x > start && hit.x < end)
        .collect();
    assert_eq!(
        marks.len(),
        1,
        "close glyph for {title:?} ({start}..{end}): {marks:?}\n{}",
        text(&screen)
    );
    app.click(marks[0]);
    marks[0]
}

/// `CloseTab` is idle-only, and a focused code editor consumes chords while
/// it is capturing. Esc dismisses completion, then leaves the editor, and
/// does not revert the SQL (`esc_reverts` is off).
fn release_editor(app: &App) {
    app.key("escape");
    app.settle();
    app.key("escape");
    app.settle();
}

fn assert_dialog(screen: &TerminalSnapshot) {
    // The body is one sentence in app.rs. The dialog wraps after "this", and
    // those rows also hold the explorer, so the sentence is not contiguous.
    let visible = text(screen);
    let (left, right) = CLOSE_BODY
        .split_once(" this ")
        .expect("close body wraps after this");
    for item in [
        CLOSE_TITLE,
        &format!("{left} this"),
        right,
        CLOSE_OK,
        "Cancel",
    ] {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    assert!(
        !visible.contains(EMPTY),
        "close dialog replaced the workbench\n{visible}"
    );
}

fn dismiss_close(app: &App) {
    let dialog = app.wait_text(CLOSE_TITLE);
    assert_dialog(&dialog);
    app.key("escape");
    app.wait_absent(CLOSE_TITLE);
}

fn confirm_close(app: &App) {
    let dialog = app.wait_text(CLOSE_TITLE);
    assert_dialog(&dialog);
    app.key("right");
    app.key("enter");
    app.wait_absent(CLOSE_TITLE);
}

fn type_query(app: &App, sql: &str, marker: &str) {
    app.type_text(sql);
    app.wait_text(marker);
}

fn open_query(app: &App, title: &str, previous: &str) {
    release_editor(app);
    app.key("ctrl+t");
    app.wait_screen(&format!("opened {title}"), |screen| {
        let visible = text(screen);
        visible.contains(title) && visible.contains(PLACEHOLDER) && !visible.contains(previous)
    });
}

fn activate(app: &App, title: &str, marker: &str, absent: &[&str]) {
    click_tab(app, title);
    let screen = app.wait_text(marker);
    see(&screen, &[title, marker], absent);
}

fn titles_are(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let line = row_text(screen, u32::from(strip_y(screen)));
    for title in present {
        assert!(line.contains(title), "strip missing {title:?}: {line}");
    }
    for title in absent {
        assert!(!line.contains(title), "strip still shows {title:?}: {line}");
    }
}

#[test]
fn route_reaches_query_1_and_quits() {
    let app = open("t04-tabs-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[QUERY_1, PLACEHOLDER, "Loading schema"],
    );
    connect_development(&app);
    let query = app.screen();
    assert_eq!((query.cols(), query.rows()), (120, 40));
    see(
        &query,
        &[CONNECTED, QUERY_1, PLACEHOLDER],
        &["Loading schema", CLOSE_TITLE, FIRST],
    );
    app.finish();
}

#[test]
fn close_keeps_each_tabs_own_sql() {
    let app = open("t04-tabs-close");
    connect_development(&app);
    focus_editor(&app);
    type_query(&app, FIRST_SQL, FIRST);
    open_query(&app, "Query 2", FIRST);
    type_query(&app, MID_SQL, MID);
    open_query(&app, "Query 3", MID);
    type_query(&app, LAST_SQL, LAST);
    open_query(&app, "Query 4", LAST);
    type_query(&app, TAIL_SQL, TAIL);

    let typed = app.screen();
    titles_are(&typed, &TITLES, &[]);
    see(&typed, &[TAIL, "Query 4"], &[FIRST, MID, LAST, CLOSE_TITLE]);

    release_editor(&app);
    app.key("ctrl+w");
    dismiss_close(&app);
    let kept = app.wait_text(TAIL);
    titles_are(&kept, &TITLES, &[]);
    see(
        &kept,
        &[TAIL, "Query 4", "Query 1"],
        &[CLOSE_TITLE, FIRST, MID, LAST],
    );

    click_close(&app, QUERY_1);
    confirm_close(&app);
    let after_first = app.wait_screen("inactive first tab closed", |screen| {
        let visible = text(screen);
        visible.contains(TAIL)
            && visible.contains("Query 4")
            && visible.contains("Query 2")
            && visible.contains("Query 3")
            && !visible.contains(FIRST)
            && !visible.contains(QUERY_1)
            && !visible.contains(CLOSE_TITLE)
    });
    titles_are(&after_first, &["Query 2", "Query 3", "Query 4"], &[QUERY_1]);
    see(&after_first, &[TAIL], &[FIRST, MID, LAST]);

    activate(&app, "Query 2", MID, &[FIRST, LAST, TAIL]);
    activate(&app, "Query 3", LAST, &[FIRST, MID, TAIL]);
    activate(&app, "Query 4", TAIL, &[FIRST, MID, LAST]);
    activate(&app, "Query 2", MID, &[FIRST, LAST, TAIL]);
    release_editor(&app);
    app.key("ctrl+w");
    confirm_close(&app);
    let neighbour = app.wait_screen("right neighbour became active", |screen| {
        let visible = text(screen);
        visible.contains(LAST)
            && visible.contains("Query 3")
            && visible.contains("Query 4")
            && !visible.contains(MID)
            && !visible.contains("Query 2")
            && !visible.contains(FIRST)
            && !visible.contains(CLOSE_TITLE)
    });
    titles_are(&neighbour, &["Query 3", "Query 4"], &[QUERY_1, "Query 2"]);
    see(&neighbour, &[LAST], &[FIRST, MID, TAIL]);
    app.expire_status();
    app.wait_absent(CONNECTED);

    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    see(
        &wide,
        &[LAST, "Query 3", "Query 4"],
        &[FIRST, MID, TAIL, CLOSE_TITLE],
    );
    app.visual(&visual_id("T04.tabs.close.120x40"));

    app.resize(80, 24);
    let narrow = app.wait_screen("closed tabs at 80x24", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(LAST)
            && visible.contains("Query 3")
            && visible.contains("Query 4")
            && !visible.contains(MID)
            && !visible.contains(FIRST)
            && !visible.contains(TAIL)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[LAST, "Query 3", "Query 4"],
        &[FIRST, MID, TAIL, CLOSE_TITLE, QUERY_1],
    );
    app.visual(&visual_id("T04.tabs.close.narrow"));

    app.resize(120, 40);
    let restored = app.wait_screen("tabs back at 120x40", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains("120×40")
            && visible.contains(LAST)
            && !visible.contains(MID)
    });
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    see(&restored, &[LAST, "Query 4"], &[FIRST, MID, TAIL]);

    activate(&app, "Query 4", TAIL, &[FIRST, MID, LAST]);
    release_editor(&app);
    app.key("ctrl+w");
    confirm_close(&app);
    let only = app.wait_screen("left neighbour became the only tab", |screen| {
        let visible = text(screen);
        visible.contains(LAST)
            && visible.contains("Query 3")
            && !visible.contains(TAIL)
            && !visible.contains("Query 4")
            && !visible.contains(MID)
            && !visible.contains(FIRST)
            && !visible.contains(CLOSE_TITLE)
    });
    titles_are(&only, &["Query 3"], &[QUERY_1, "Query 2", "Query 4"]);
    see(&only, &[LAST], &[FIRST, MID, TAIL, EMPTY]);

    release_editor(&app);
    app.key("ctrl+w");
    confirm_close(&app);
    let empty = app.wait_text(EMPTY);
    see(
        &empty,
        &[EMPTY, "Ctrl+T starts a query"],
        &[FIRST, MID, LAST, TAIL, CLOSE_TITLE, "Query 3", QUERY_1],
    );
    app.finish();
}
