//! TablePro dirty-tab close, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t04_close/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY_1: &str = "Query 1";
const QUERY_2: &str = "Query 2";
const PLACEHOLDER: &str = "Type SQL";
const KEEP_SQL: &str = "SELECT 't04_close_keep' AS marker";
const DROP_SQL: &str = "SELECT 't04_close_drop' AS marker";
const KEEP: &str = "t04_close_keep";
const DROP: &str = "t04_close_drop";
const TITLE: &str = "Close tab with unsaved work?";
const BODY_LEAD: &str = "Pending row edits and unsaved query text";
const BODY_TAIL: &str = "will be lost.";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Close anyway";
const FOCUS: &str = "▎";
const WIDE: &str = "T04.close.dirty.120x40";
const NARROW: &str = "T04.close.dirty.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t04_close/{}",
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
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY_1)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY_1, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Loading schema",
            TITLE,
        ],
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

fn type_sql(app: &App, sql: &str) {
    app.type_text(sql);
    app.wait_text(sql);
}

fn tab_line(screen: &TerminalSnapshot) -> Option<String> {
    (0..screen.rows())
        .map(|index| row_text(screen, index))
        .find(|row| row.contains(QUERY_1) && row.contains('×'))
}

fn require_tabs(screen: &TerminalSnapshot) -> String {
    tab_line(screen).unwrap_or_else(|| panic!("tab strip missing\n{}", text(screen)))
}

fn click_tab(app: &App, label: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.y < 8 && row.contains('×') && row.contains(QUERY_1)
        })
        .collect();
    assert_eq!(hits.len(), 1, "tab {label:?}: {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn footer_toast(screen: &TerminalSnapshot) -> bool {
    let last = screen.rows().saturating_sub(1);
    row_text(screen, last).contains("✓ Connected to Development")
}

fn dialog_open(screen: &TerminalSnapshot) -> bool {
    let Some(tabs) = tab_line(screen) else {
        return false;
    };
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(BODY_LEAD)
        && visible.contains(BODY_TAIL)
        && visible.contains(CANCEL)
        && visible.contains(DISCARD)
        && tabs.contains(QUERY_2)
}

fn assert_open_dialog(screen: &TerminalSnapshot) {
    assert!(
        dialog_open(screen),
        "unsaved close dialog is not open\n{}",
        text(screen)
    );
    let visible = text(screen);
    let tabs = require_tabs(screen);
    assert!(
        tabs.contains(QUERY_1) && tabs.contains(QUERY_2),
        "a tab left the strip: {tabs}"
    );
    assert!(
        visible.contains(&format!("{FOCUS}{CANCEL}")),
        "Cancel is not focused\n{visible}"
    );
    assert!(
        !visible.contains(&format!("{FOCUS}{DISCARD}")),
        "Close anyway took focus\n{visible}"
    );
    assert!(
        !visible.contains("Save"),
        "the close dialog shows a Save button\n{visible}"
    );
}

fn leave_editor(app: &App, still: &str) {
    app.key("escape");
    app.wait_screen("editor left", |screen| {
        let visible = text(screen);
        visible.contains(still) && !visible.contains("Ctrl+Space") && !visible.contains(TITLE)
    });
}

fn confirm_quit(app: &App) {
    app.key("ctrl+q");
    app.wait_text("Quit TablePro?");
    app.confirm_quit();
}

#[test]
fn tablepro_close_route_reaches_query_and_quits() {
    let app = open("t04-close-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY_1]);
    connect_development(&app);
    let query = app.screen();
    assert_eq!((query.cols(), query.rows()), (120, 40));
    see(
        &query,
        &[QUERY_1, CONNECTED, PLACEHOLDER],
        &[TITLE, "Loading schema", QUERY_2, DISCARD],
    );
    app.finish();
}

#[test]
fn tablepro_close_dirty_cancel_keeps_sql_and_blocks_the_neighbour() {
    let app = open("t04-close-dirty");
    connect_development(&app);
    focus_editor(&app);
    type_sql(&app, KEEP_SQL);
    app.key("ctrl+t");
    let second = app.wait_screen("query 2 editor", |screen| {
        let visible = text(screen);
        visible.contains(QUERY_2)
            && visible.contains(PLACEHOLDER)
            && visible.contains("Ctrl+Space")
            && tab_line(screen).is_some_and(|row| row.contains(QUERY_1))
    });
    see(&second, &[QUERY_1, QUERY_2, PLACEHOLDER], &[TITLE, DROP]);
    type_sql(&app, DROP_SQL);

    click_tab(&app, QUERY_1);
    let active = app.wait_screen("query 1 active", |screen| {
        let visible = text(screen);
        visible.contains(KEEP)
            && !visible.contains(DROP)
            && tab_line(screen).is_some_and(|row| row.contains(QUERY_2))
    });
    see(&active, &[KEEP, QUERY_1, QUERY_2], &[DROP, TITLE]);
    // Activating a query focuses its editor, and Immediate entry starts
    // capturing. Close tab is idle-only, so it does not fire until Esc leaves
    // the editor. Esc commits; it does not revert the statement.
    leave_editor(&app, KEEP);

    app.key("ctrl+w");
    let opened = app.wait_text(TITLE);
    assert_open_dialog(&opened);

    // The tab-strip close glyph is not a reachable hit while this dialog is
    // up: the modal barrier hides page hits, and the dialog surface does not
    // cover the strip. Button-up there is Release::Outside, and
    // PointerHost::outside cancels the dialog (app.rs outside). Ctrl+W is the
    // close the modal refuses while both tabs stay.
    app.key("ctrl+w");
    let blocked = app.wait_screen("second close refused", dialog_open);
    assert_open_dialog(&blocked);
    flash_settle(&app);

    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert_open_dialog(&wide);
    // The connected toast lasts 5s. visual() would keep the cleared footer.
    app.visual_while(&visual_id(WIDE), |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_open(screen) && footer_toast(screen)
    });

    app.resize(80, 24);
    let narrow = app.wait_screen("dirty close dialog at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && dialog_open(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert_open_dialog(&narrow);
    app.visual_while(&visual_id(NARROW), |screen| {
        screen.cols() == 80 && screen.rows() == 24 && dialog_open(screen) && footer_toast(screen)
    });
    app.resize(120, 40);

    let restored = app.wait_screen("dirty close dialog back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_open(screen)
    });
    assert_open_dialog(&restored);

    app.key("enter");
    let kept = app.wait_screen("cancel keeps the sql", |screen| {
        let visible = text(screen);
        // The title leaves a frame before the "Close anyway" footer hint.
        !visible.contains(TITLE)
            && !visible.contains(DISCARD)
            && visible.contains(KEEP)
            && tab_line(screen).is_some_and(|row| row.contains(QUERY_1) && row.contains(QUERY_2))
    });
    see(
        &kept,
        &[KEEP, QUERY_1, QUERY_2],
        &[TITLE, DISCARD, "Save", DROP],
    );
    assert!(
        require_tabs(&kept).contains(QUERY_1),
        "cancel closed Query 1\n{}",
        text(&kept)
    );

    click_tab(&app, QUERY_2);
    let dropping = app.wait_screen("query 2 active", |screen| {
        text(screen).contains(DROP) && !text(screen).contains(KEEP)
    });
    see(&dropping, &[DROP, QUERY_2], &[TITLE, KEEP]);
    leave_editor(&app, DROP);
    app.key("ctrl+w");
    let again = app.wait_text(TITLE);
    assert_open_dialog(&again);
    app.key("right");
    let armed = app.wait_screen("close anyway focused", |screen| {
        let visible = text(screen);
        visible.contains(&format!("{FOCUS}{DISCARD}"))
            && !visible.contains(&format!("{FOCUS}{CANCEL}"))
    });
    see(&armed, &[TITLE, DISCARD, QUERY_1, QUERY_2], &["Save"]);
    app.key("enter");
    let closed = app.wait_screen("query 2 discarded", |screen| {
        let visible = text(screen);
        let tabs = tab_line(screen).unwrap_or_default();
        !visible.contains(TITLE)
            && !visible.contains(DISCARD)
            && !visible.contains(DROP)
            && visible.contains(KEEP)
            && tabs.contains(QUERY_1)
            && !tabs.contains(QUERY_2)
    });
    see(
        &closed,
        &[KEEP, QUERY_1],
        &[DROP, QUERY_2, TITLE, DISCARD, "Save"],
    );
    confirm_quit(&app);
}
