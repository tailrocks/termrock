//! TablePro shell menus: the highlighted command is the one that runs.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_menus/` and each checkpoint's dots become underscores.
//! TablePro has no menu bar and no `--page` flag. It opens on Connections.
//! Shift+F10 opens the focused object's menu. The column header has no such
//! key, so that menu is a right click. There is no `KeyCode::Menu`.

use tui_test::{MouseButton, MouseOptions, TerminalSnapshot};

use crate::support::{
    App, Launch, Point, SessionMode, TABLEPRO, file_sha256, find_all,
    flash_settle_stepped as flash_settle, reference_binary_from, reference_digest_from, row_text,
    text, try_launch,
};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const QUERY_NEXT: &str = "Query 2";
const MARKER: &str = "t05_menus_marker";
const SQL: &str = "SELECT name FROM products WHERE sku = 'SKU-01000' -- t05_menus_marker";
/// The menu status line truncates inside the explorer column, so the test
/// matches the visible prefix of "Tables and views only".
const OPEN_DISABLED: &str = "Tables and views";
const READ_ONLY: &str = "Read-only rows";
const CURRENT_TAB: &str = "Current tab";
const SWITCH: &str = "Switch to tab";
const NEW_QUERY: &str = "New query tab";
const HISTORY: &str = "Query history";
const TABS: &str = "Filter tabs…";
const OPEN_TABS: &str = "Open tabs…";
const CHOOSE: &str = "Choose";
const FOCUS: &str = "▎";
const WIDE: &str = "T05.menus.menus.120x40";
const NARROW: &str = "T05.menus.menus.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_menus/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let capture = std::env::var_os("T05_CAPTURE").is_some_and(|value| value == "1");
    let mut launch = if capture {
        let manifest =
            crate::support::workspace_root().join("docs/testing/reference-manifest-j06-pr2.json");
        let binary = reference_binary_from(&manifest, "tablepro-pr2");
        assert_eq!(
            file_sha256(&binary),
            reference_digest_from(&manifest, "tablepro-pr2"),
            "T05 reference binary digest"
        );
        let mut launch = Launch::new(name, binary.to_str().expect("reference path"));
        launch.mode = SessionMode::Reference;
        launch.reference_manifest = Some(manifest);
        launch
    } else {
        Launch::new(name, TABLEPRO)
    };
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

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(FOCUS))
        .unwrap_or_else(|| panic!("no focus bar\n{}", text(screen)))
}

fn menu_open(screen: &TerminalSnapshot) -> bool {
    footer(screen).contains(CHOOSE)
}

fn tab_menu(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    menu_open(screen)
        && visible.contains(NEW_QUERY)
        && visible.contains(SWITCH)
        && visible.contains(CURRENT_TAB)
}

fn shot(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    let before = app.screen();
    assert_eq!(
        (before.cols(), before.rows()),
        (cols, rows),
        "checkpoint {checkpoint} size"
    );
    assert!(
        tab_menu(&before),
        "checkpoint {checkpoint} left the current-tab menu\n{}",
        text(&before)
    );
    let test_id = visual_id(checkpoint);
    if std::env::var_os("T05_CAPTURE").is_some_and(|value| value == "1") {
        capture_reference_frame(app, &test_id, &app.screen());
    } else {
        app.visual(&test_id);
    }
    let after = app.screen();
    assert_eq!((after.cols(), after.rows()), (cols, rows));
    assert!(
        tab_menu(&after),
        "checkpoint {checkpoint} did not stay on the current-tab menu\n{}",
        text(&after)
    );
}

fn capture_reference_frame(app: &App, test_id: &str, screen: &TerminalSnapshot) {
    let session_path = app.evidence_dir().join("session.json");
    let session: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&session_path).expect("reference session metadata"))
            .expect("parse reference session metadata");
    assert_eq!(session["mode"], "reference");
    assert_eq!(
        session["reference_sha"],
        "3cbe13ce922f7d0902bf42af418f3d012d8b537f"
    );
    assert_eq!(
        session["lockfile_sha256"],
        "9d9859e69e1de0b3306231a25a381c865079ca7ab533420f34dceb6eafe4d39a"
    );
    assert_eq!(
        session["binary_sha256"],
        "ebcc660347e420a9ea1879d1a54537ab08f5cbb750d88701ed5956899492e133"
    );
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let directory = app.evidence_dir().join("reference-candidates");
    std::fs::create_dir_all(&directory).expect("create reference candidate directory");
    let candidate = directory.join(format!("{}.tui.pb", test_id.replace('/', "_")));
    std::fs::write(&candidate, &bytes).expect("write reference candidate sample");
    let digest = file_sha256(&candidate);
    let index = serde_json::json!({
        "test_id": test_id,
        "candidate_path": candidate,
        "candidate_sha256": digest,
        "session_path": session_path,
        "session_sha256": file_sha256(&session_path),
        "binary_path": session["binary_path"],
        "binary_sha256": session["binary_sha256"],
        "reference_sha": session["reference_sha"],
        "lockfile_sha256": session["lockfile_sha256"],
    });
    use std::io::Write as _;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("index.jsonl"))
        .expect("open reference capture index");
    serde_json::to_writer(&mut log, &index).expect("write reference capture index");
    log.write_all(b"\n")
        .expect("finish reference capture index");
    eprintln!(
        "T05 reference candidate {test_id} sha256 {digest} at {}",
        candidate.display()
    );
}

fn focus_development(app: &App) {
    for _ in 0..8 {
        let line = focused_line(&app.screen());
        if line.contains("Development") && line.contains("pg") {
            return;
        }
        let before = line;
        app.key("down");
        app.wait_screen("connection cursor moved", |screen| {
            focused_line(screen) != before
        });
    }
    panic!(
        "Development is not the focused connection\n{}",
        text(&app.screen())
    );
}

fn open_focused_menu(app: &App, needle: &str) -> TerminalSnapshot {
    app.key("shift+f10");
    app.wait_screen(&format!("menu {needle}"), |screen| {
        text(screen).contains(needle) && menu_open(screen)
    })
}

fn close_menu(app: &App, gone: &str) -> TerminalSnapshot {
    app.key("esc");
    app.wait_screen(&format!("closed {gone}"), |screen| {
        !text(screen).contains(gone) && !menu_open(screen)
    })
}

fn click_brand(app: &App) {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "TablePro")
        .into_iter()
        .filter(|hit| hit.y == 0)
        .collect();
    assert_eq!(hits.len(), 1, "TablePro brand {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn right_click(app: &App, point: Point) {
    let options = MouseOptions::new(MouseButton::Right);
    app.type_text(&tui_test::input::mouse::click(point.x, point.y, options));
}

fn header_point(screen: &TerminalSnapshot, label: &str) -> Point {
    let mut hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 28)
        .collect();
    assert!(!hits.is_empty(), "no {label:?} header\n{}", text(screen));
    hits.sort_by_key(|hit| (hit.y, hit.x));
    hits[0]
}

fn connect_development(app: &App) {
    focus_development(app);
    let detail = app.screen();
    see(
        &detail,
        &["Development", HOST, DATABASE],
        &[QUERY, CONNECTED],
    );
    app.key("enter");
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "Filter connections",
            "Loading schema",
        ],
    );
}

fn connection_menu(app: &App) {
    focus_development(app);
    let menu = open_focused_menu(app, "Duplicate");
    see(
        &menu,
        &[
            "Development",
            "Connect",
            "Edit…",
            "Duplicate",
            "Delete…",
            "New connection…",
            "Filter",
        ],
        &[OPEN_TABS, "Disconnect", QUERY],
    );
    // Duplicate stays on the detail pane. New connection… exists only in the menu.
    let closed = close_menu(app, "New connection…");
    see(
        &closed,
        &["Filter connections", "Development", "Duplicate"],
        &["New connection…", QUERY, "Choose"],
    );
}

fn connections_app_menu(app: &App) {
    click_brand(app);
    let menu = app.wait_screen("connections app menu", |screen| {
        let visible = text(screen);
        visible.contains("Quit TablePro") && menu_open(screen) && !visible.contains(OPEN_TABS)
    });
    see(
        &menu,
        &["TablePro", "Quit TablePro"],
        &[OPEN_TABS, "Disconnect", "New connection…"],
    );
    let closed = close_menu(app, "Quit TablePro");
    see(&closed, &["Filter connections"], &[QUERY, "Disconnect"]);
}

fn explorer_menu(app: &App) {
    app.key("/");
    app.wait_text("Filter objects");
    app.type_text("set_updated_at");
    app.wait_text("set_updated_at");
    app.key("down");
    for _ in 0..8 {
        let line = focused_line(&app.screen());
        // The tree column truncates the label to set_updated_….
        if line.contains('ƒ') && line.contains("set_updated") {
            break;
        }
        let before = line;
        app.key("down");
        app.wait_screen("explorer cursor moved", |screen| {
            focused_line(screen) != before
        });
    }
    let on_function = focused_line(&app.screen());
    assert!(
        on_function.contains('ƒ') && on_function.contains("set_updated"),
        "function row is not focused: {on_function}"
    );
    // The cursor starts on the first enabled row, so the disabled Open
    // reason is hidden until that row is highlighted.
    let menu = open_focused_menu(app, "Filter objects…");
    see(
        &menu,
        &["set_updated", "Open", "Filter objects…", "Refresh"],
        &["Use schema", QUERY_NEXT, "Opened", OPEN_DISABLED],
    );
    app.key("up");
    let disabled = app.wait_screen("explorer open is disabled", |screen| {
        text(screen).contains(OPEN_DISABLED) && menu_open(screen)
    });
    see(
        &disabled,
        &[
            "set_updated",
            "Open",
            OPEN_DISABLED,
            "Filter objects…",
            "Refresh",
        ],
        &["Use schema", QUERY_NEXT, "Opened"],
    );
    close_menu(app, OPEN_DISABLED);
    // Esc on the explorer clears a non-empty object filter.
    app.key("esc");
    app.wait_screen("explorer filter cleared", |screen| {
        text(screen).contains("organizations") && !text(screen).contains(OPEN_DISABLED)
    });
}

fn open_products(app: &App) {
    app.key("ctrl+o");
    app.wait_text("Open Quickly");
    app.type_text("products");
    app.wait_text("products");
    app.key("enter");
    app.wait_screen("products grid", |screen| {
        let visible = text(screen);
        visible.contains("public › products")
            && visible.contains("sku")
            && visible.contains("SKU-01000")
            && !visible.contains("Open Quickly")
            && !visible.contains("Loading")
    });
}

fn table_grid_menu(app: &App) {
    let menu = open_focused_menu(app, "Copy cell");
    see(
        &menu,
        &["Copy cell", "Copy row", "Edit cell", "public › products"],
        &[
            "Sort Ascending",
            "Size Column to Fit",
            READ_ONLY,
            QUERY_NEXT,
        ],
    );
    close_menu(app, "Copy cell");
}

fn header_menu(app: &App) {
    // Shift+F10 on the grid is the cell menu, already closed above. The header
    // menu is only `on_secondary`.
    let screen = app.screen();
    assert!(
        !text(&screen).contains("Sort Ascending"),
        "header menu was already open\n{}",
        text(&screen)
    );
    right_click(app, header_point(&screen, "sku"));
    let menu = app.wait_screen("column header menu", |screen| {
        text(screen).contains("Sort Ascending") && menu_open(screen)
    });
    see(
        &menu,
        &[
            "sku",
            "Sort Ascending",
            "Sort Descending",
            "Size Column to Fit",
        ],
        &["Copy cell", "Already sorted ascending", QUERY_NEXT],
    );
    close_menu(app, "Sort Ascending");
}

/// A printable `[` is quiet while a field is editing, so leave the field first.
/// Esc on the history search clears the text and keeps editing; Enter is Done.
fn leave_capture(app: &App) {
    let foot = footer(&app.screen());
    if foot.contains("Indent") {
        app.key("esc");
        app.wait_screen("editor stopped editing", |screen| {
            !footer(screen).contains("Indent")
        });
    } else if foot.contains("Type") && text(&app.screen()).contains("Search history") {
        app.key("enter");
        app.wait_screen("history search done", |screen| {
            !footer(screen).contains("Type")
        });
    }
}

fn on_query_page(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(QUERY)
        && !visible.contains("public › products")
        && !visible.contains(HISTORY)
        && !visible.contains(TABS)
        && !menu_open(screen)
}

fn show_query(app: &App) {
    for _ in 0..8 {
        if on_query_page(&app.screen()) {
            return;
        }
        leave_capture(app);
        let before = text(&app.screen());
        app.key("[");
        app.wait_screen("previous tab", |screen| text(screen) != before);
    }
    panic!("Query 1 is not showing\n{}", text(&app.screen()));
}

fn run_marker(app: &App) {
    show_query(app);
    if !footer(&app.screen()).contains("Indent") {
        app.key("enter");
        app.wait_screen("editor editing", |screen| footer(screen).contains("Indent"));
    }
    app.type_text(SQL);
    app.wait_text(SQL);
    app.key("ctrl+r");
    app.wait_screen("marker result", |screen| {
        let visible = text(screen);
        visible.contains("1 row") && visible.contains(MARKER) && !visible.contains("Esc cancels")
    });
}

fn focus_result_grid(app: &App) {
    if footer(&app.screen()).contains("Indent") {
        app.key("esc");
        app.wait_screen("editor stopped editing", |screen| {
            !footer(screen).contains("Indent")
        });
    }
    for _ in 0..8 {
        let foot = footer(&app.screen());
        if foot.contains("Cell") && foot.contains("Menu") {
            return;
        }
        app.key("tab");
        app.wait_screen("result focus moved", |screen| footer(screen) != foot);
    }
    panic!("result grid is not focused\n{}", text(&app.screen()));
}

fn query_grid_menu(app: &App) {
    focus_result_grid(app);
    let menu = open_focused_menu(app, "Copy cell");
    see(
        &menu,
        &[
            "Copy cell",
            "Copy row",
            "Edit cell",
            "Filter by this value",
            MARKER,
        ],
        &["Sort Ascending", QUERY_NEXT, READ_ONLY],
    );
    // Copy cell and Copy row are enabled. Edit cell is the disabled row.
    app.key("down");
    app.key("down");
    let locked = app.wait_screen("query edit cell is read-only", |screen| {
        text(screen).contains(READ_ONLY) && text(screen).contains("Edit cell") && menu_open(screen)
    });
    see(
        &locked,
        &["Edit cell", READ_ONLY, "Copy cell", "Filter by this value"],
        &["Sort Ascending", QUERY_NEXT],
    );
    close_menu(app, READ_ONLY);
}

fn history_search(app: &App) {
    app.key("ctrl+y");
    let history = app.wait_screen("history list", |screen| {
        let visible = text(screen);
        visible.contains(HISTORY) && visible.contains(MARKER) && !menu_open(screen)
    });
    see(
        &history,
        &[HISTORY, "Search history"],
        &[QUERY_NEXT, "Esc cancels"],
    );
    let menu = open_focused_menu(app, "Run in new tab");
    see(
        &menu,
        &["Open in new tab", "Run in new tab", "Copy query", "Search"],
        &[QUERY_NEXT, READ_ONLY],
    );
    app.key("down");
    app.key("down");
    app.key("down");
    app.key("enter");
    let searching = app.wait_screen("history search focused", |screen| {
        let foot = footer(screen);
        let visible = text(screen);
        visible.contains(HISTORY)
            && visible.contains("Search history")
            && foot.contains("Type")
            && foot.contains("Clear")
            && !menu_open(screen)
            && !visible.contains(QUERY_NEXT)
            && !visible.contains("Esc cancels")
    });
    // Run in new tab stays as a detail button. Search focused the field
    // and did not open a tab or rerun the statement.
    see(
        &searching,
        &[HISTORY, MARKER, "Development", "Type"],
        &[
            QUERY_NEXT,
            "Esc cancels",
            "Statement executed",
            "Choose",
            "Disconnected",
        ],
    );
}

fn workbench_app_menu(app: &App) {
    click_brand(app);
    let menu = app.wait_screen("workbench app menu", |screen| {
        let visible = text(screen);
        visible.contains(OPEN_TABS) && visible.contains("Disconnect") && menu_open(screen)
    });
    see(
        &menu,
        &[OPEN_TABS, "Disconnect", "Quit TablePro", "Development"],
        &[QUERY_NEXT, "Filter connections"],
    );
    // The first row is Open tabs…. Enter must not Disconnect.
    app.key("enter");
    let tabs = app.wait_screen("open tabs from the app menu", |screen| {
        let visible = text(screen);
        visible.contains(TABS)
            && visible.contains("Open tabs")
            && visible.contains("Development")
            && !visible.contains("Disconnect")
            && !visible.contains("Disconnected")
    });
    see(
        &tabs,
        &[TABS, "Open tabs", QUERY, "Development"],
        &["Filter connections", "Disconnected", QUERY_NEXT],
    );
    app.key("esc");
    app.wait_screen("tab list closed", |screen| {
        let visible = text(screen);
        !visible.contains(TABS) && visible.contains("Development") && visible.contains(HISTORY)
    });
}

fn focus_tab_strip(app: &App) {
    // Esc on the explorer is consumed. Tab is the shell's next-focus key.
    if footer(&app.screen()).contains("Quick open") {
        let before = footer(&app.screen());
        app.key("tab");
        app.wait_screen("left the explorer", |screen| footer(screen) != before);
    }
    for _ in 0..4 {
        if footer(&app.screen()).contains("Tab list") {
            return;
        }
        let before = footer(&app.screen());
        app.key("esc");
        app.wait_screen("toward the tab strip", |screen| {
            let foot = footer(screen);
            foot.contains("Tab list") || foot != before
        });
    }
    assert!(
        footer(&app.screen()).contains("Tab list"),
        "tab strip is not focused\n{}",
        text(&app.screen())
    );
}

fn previous_tab_until_marker(app: &App) {
    for _ in 0..8 {
        let visible = text(&app.screen());
        if visible.contains(MARKER)
            && visible.contains(QUERY)
            && visible.contains(QUERY_NEXT)
            && !visible.contains(HISTORY)
            && !visible.contains("public › products")
        {
            return;
        }
        leave_capture(app);
        let before = text(&app.screen());
        app.key("[");
        app.wait_screen("back toward Query 1", |screen| text(screen) != before);
    }
    panic!(
        "Query 1 no longer holds the marker beside Query 2\n{}",
        text(&app.screen())
    );
}

fn quit_unsaved(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &["unsaved", "Quit"], &[HISTORY]);
    app.key("right");
    app.key("enter");
    app.wait_exit("tablepro stayed up after confirming quit");
    app.expect_exited(0);
}

#[test]
fn tablepro_menus_route_reaches_query_1() {
    let app = open("t05-menus-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Filter connections", "Development"],
        &[QUERY, CONNECTED],
    );
    connection_menu(&app);
    connections_app_menu(&app);
    connect_development(&app);
    let work = app.screen();
    see(
        &work,
        &[QUERY, CONNECTED, "Development"],
        &["Loading schema", "Duplicate", OPEN_TABS],
    );
    app.finish();
}

#[test]
fn tablepro_menus_highlighted_command_is_the_one_that_runs() {
    let app = open("t05-menus-commands");
    connection_menu(&app);
    connections_app_menu(&app);
    connect_development(&app);

    explorer_menu(&app);
    open_products(&app);
    table_grid_menu(&app);
    header_menu(&app);
    run_marker(&app);
    query_grid_menu(&app);
    history_search(&app);
    workbench_app_menu(&app);

    // The brand click's press flash must be gone before the menu frames.
    flash_settle(&app);
    show_query(&app);
    focus_tab_strip(&app);
    // Close tab is the first enabled row. Up highlights the disabled
    // current-tab command so its reason is on screen for both frames.
    let opened = open_focused_menu(&app, NEW_QUERY);
    assert!(
        !text(&opened).contains(CURRENT_TAB),
        "the disabled reason was visible before that row was highlighted\n{}",
        text(&opened)
    );
    app.key("up");
    let on_switch = app.wait_screen("switch to tab is the disabled current row", |screen| {
        let visible = text(screen);
        visible.contains(CURRENT_TAB)
            && visible.contains(NEW_QUERY)
            && visible.contains(SWITCH)
            && menu_open(screen)
    });
    assert_eq!((on_switch.cols(), on_switch.rows()), (120, 40));
    see(
        &on_switch,
        &[NEW_QUERY, SWITCH, CURRENT_TAB, QUERY, MARKER, "Close tab"],
        &[QUERY_NEXT, "Filter tabs", READ_ONLY],
    );

    // The "Opened public.products" note lasts 5s. Both frames are taken after it.
    app.expire_status();
    app.wait_absent("Opened");
    shot(&app, WIDE, 120, 40);
    app.resize(80, 24);
    let narrowed = app.wait_screen("current-tab menu at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && tab_menu(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    shot(&app, NARROW, 80, 24);
    app.resize(120, 40);
    let restored = app.wait_screen("current-tab menu back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && tab_menu(screen)
    });
    see(
        &restored,
        &[NEW_QUERY, SWITCH, CURRENT_TAB, MARKER],
        &[QUERY_NEXT],
    );

    // Enter on the disabled current-tab row is consumed. The menu and SQL stay.
    app.key("enter");
    let stayed = app.wait_screen("disabled switch to tab did nothing", |screen| {
        let visible = text(screen);
        visible.contains(CURRENT_TAB)
            && visible.contains(NEW_QUERY)
            && visible.contains(MARKER)
            && visible.contains(SQL)
            && !visible.contains(QUERY_NEXT)
    });
    assert!(tab_menu(&stayed), "{}", text(&stayed));

    app.key("down");
    app.key("down");
    app.key("enter");
    let created = app.wait_screen("new query tab from the highlighted command", |screen| {
        let visible = text(screen);
        visible.contains(QUERY_NEXT) && !visible.contains(CURRENT_TAB) && !menu_open(screen)
    });
    see(
        &created,
        &[QUERY_NEXT, QUERY],
        &[CURRENT_TAB, "Filter connections"],
    );
    previous_tab_until_marker(&app);
    let returned = app.screen();
    see(
        &returned,
        &[QUERY, QUERY_NEXT, MARKER, SQL, "Development"],
        &[
            CURRENT_TAB,
            "Filter connections",
            "Esc cancels",
            "Disconnected",
        ],
    );
    quit_unsaved(&app);
}
