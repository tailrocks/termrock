//! TablePro disconnect, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_disconnect/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const DISCONNECTED: &str = "Disconnected from Development";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";
const SQL: &str = "SELECT 't05_disconnect_keep' AS marker";
const MARKER: &str = "t05_disconnect_keep";
const TITLE: &str = "Disconnect from Development?";
const BODY: &str = "1 unsaved query will be lost.";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Disconnect";
const FOCUS: &str = "▎";
const MENU: &str = "Open tabs";
const CONNECTIONS: &str = "Filter connections";
const WIDE: &str = "T05.disconnect.disconnect.120x40";
const NARROW: &str = "T05.disconnect.disconnect.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_disconnect/{}",
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
    app.wait_text(CONNECTIONS);
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
            TITLE,
            DISCONNECTED,
        ],
    );
}

fn click_where(app: &App, label: &str, keep: impl Fn(&TerminalSnapshot, u16, u16) -> bool) {
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
    click_where(app, PLACEHOLDER, |screen, x, y| {
        x > 20 && row_text(screen, u32::from(y)).contains(PLACEHOLDER)
    });
    app.wait_text("Ctrl+Space");
}

fn dialog_copy(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let words = flat(screen);
    words.contains(TITLE)
        && words.contains(BODY)
        && visible.contains(&format!("{FOCUS}{CANCEL}"))
        && !visible.contains(&format!("{FOCUS}{CONFIRM}"))
        && words.contains(QUERY)
        && !words.contains(DISCONNECTED)
        && !words.contains(CONNECTIONS)
        && !words.contains("Unsaved changes")
        && !words.contains("will be discarded")
        && !words.contains("Save")
        && !words.contains("Discard")
}

fn dialog_open(screen: &TerminalSnapshot) -> bool {
    dialog_copy(screen) && flat(screen).contains(CONNECTED)
}

/// The connected toast lasts 5s. `visual` then keeps the next quiet frame, so
/// a loaded run locks the footer after the toast has cleared. Hold the open
/// dialog while the toast is up, and compare that frame.
fn capture_dialog(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    app.visual_while(&visual_id(checkpoint), |screen| {
        screen.cols() == cols && screen.rows() == rows && dialog_open(screen)
    });
}

fn assert_open_dialog(screen: &TerminalSnapshot) {
    assert!(
        dialog_open(screen),
        "disconnect dialog is not open\n{}",
        text(screen)
    );
}

fn open_app_menu(app: &App) {
    click_where(app, "TablePro", |screen, x, y| {
        y <= 2 && x < 30 && row_text(screen, u32::from(y)).contains("TablePro")
    });
    let menu = app.wait_text(MENU);
    see(
        &menu,
        &[MENU, CONFIRM, "Quit TablePro"],
        &[TITLE, "Loading schema"],
    );
}

fn choose_disconnect(app: &App) {
    open_app_menu(app);
    click_where(app, CONFIRM, |screen, _, y| {
        let row = row_text(screen, u32::from(y));
        row.contains(CONFIRM) && !row.contains("from") && !row.contains(DISCONNECTED)
    });
}

fn on_connections(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CONNECTIONS)
        && visible.contains(DISCONNECTED)
        && !visible.contains(TITLE)
        && !visible.contains(QUERY)
        && !visible.contains(MARKER)
        && !visible.contains("unsaved")
}

#[test]
fn tablepro_disconnect_route_reaches_query_1() {
    let app = open("t05-disconnect-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[CONNECTIONS, "Development"], &[QUERY, TITLE]);
    connect_development(&app);
    let work = app.screen();
    assert_eq!((work.cols(), work.rows()), (120, 40));
    see(
        &work,
        &[QUERY, PLACEHOLDER, CONNECTED, "Development"],
        &[TITLE, "Loading schema", DISCONNECTED, "Query 2"],
    );
    app.finish();
}

#[test]
fn tablepro_disconnect_clean_returns_and_dirty_cancel_keeps_the_draft() {
    let app = open("t05-disconnect-disconnect");
    connect_development(&app);

    choose_disconnect(&app);
    let clean = app.wait_screen("clean disconnect", on_connections);
    assert_eq!((clean.cols(), clean.rows()), (120, 40));
    see(
        &clean,
        &[CONNECTIONS, DISCONNECTED, "Development"],
        &[TITLE, QUERY, BODY, "Loading schema"],
    );

    connect_development(&app);
    focus_editor(&app);
    app.type_text(SQL);
    app.wait_text(MARKER);
    // Typing "marker" opens completion over the tail of the line
    // (customers.marketing_opt_in). The first Esc dismisses that popup and
    // does not insert it. The next Esc commits the editor. It does not revert
    // the statement, and it is not the disconnect command: priority Esc is
    // CancelQuery.
    app.key("escape");
    let settled = app.wait_screen("completion dismissed", |screen| {
        let visible = text(screen);
        visible.contains(SQL) && !visible.contains("marketing_opt_in")
    });
    if text(&settled).contains("Ctrl+Space") {
        app.key("escape");
    }
    let drafted = app.wait_screen("editor left the draft", |screen| {
        let visible = text(screen);
        visible.contains(SQL)
            && visible.contains(CONNECTED)
            && !visible.contains("Ctrl+Space")
            && !visible.contains("marketing_opt_in")
            && !visible.contains(TITLE)
    });
    see(
        &drafted,
        &[SQL, QUERY, CONNECTED],
        &[TITLE, DISCONNECTED, CONNECTIONS],
    );

    choose_disconnect(&app);
    let opened = app.wait_text(TITLE);
    assert_open_dialog(&opened);
    flash_settle(&app);
    capture_dialog(&app, WIDE, 120, 40);

    app.resize(80, 24);
    capture_dialog(&app, NARROW, 80, 24);
    app.resize(120, 40);

    let restored = app.wait_screen("disconnect dialog back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_copy(screen)
    });
    assert!(
        dialog_copy(&restored),
        "disconnect dialog is not open\n{}",
        text(&restored)
    );

    app.key("enter");
    let kept = app.wait_screen("cancel keeps the draft", |screen| {
        let visible = text(screen);
        visible.contains(SQL)
            && visible.contains(QUERY)
            && !visible.contains(TITLE)
            && !visible.contains(DISCONNECTED)
            && !visible.contains(CONNECTIONS)
    });
    see(
        &kept,
        &[SQL, QUERY, "acme_prod › public"],
        &[TITLE, BODY, DISCONNECTED, CONNECTIONS, "Query 2"],
    );

    choose_disconnect(&app);
    let again = app.wait_screen("disconnect dialog again", dialog_copy);
    assert!(
        dialog_copy(&again),
        "disconnect dialog is not open\n{}",
        text(&again)
    );
    app.key("right");
    let armed = app.wait_screen("disconnect button focused", |screen| {
        let visible = text(screen);
        visible.contains(TITLE)
            && visible.contains(&format!("{FOCUS}{CONFIRM}"))
            && !visible.contains(&format!("{FOCUS}{CANCEL}"))
    });
    see(&armed, &[TITLE, BODY, CONFIRM], &["Save", "Discard"]);
    app.key("enter");
    let left = app.wait_screen("confirmed disconnect", on_connections);
    see(
        &left,
        &[CONNECTIONS, DISCONNECTED],
        &[TITLE, SQL, MARKER, QUERY, CONNECTED, BODY],
    );
    app.finish();
}
