//! TablePro context menus: the action uses the selected row or cell.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_context/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const GUTTER: char = '▎';
const PREVIOUS: &str = "customers";
const SELECTED: &str = "products";
const PANEL: &str = "public › products";
const OTHER_PANEL: &str = "public › customers";
const ROW_MENU: &str = "Filter objects…";
const CELL_MENU: &str = "Copy cell";
const ROW_A: &str = "SKU-01000";
const ROW_B: &str = "SKU-01001";
const CELL_TITLE: &str = "sku · row 2";
const OTHER_TITLE: &str = "sku · row 1";
const OSC_REFUSAL: &str = "Not copied · this terminal does not accept OSC 52";
const WIDE: &str = "T05.context.context.120x40";
const NARROW: &str = "T05.context.context.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_context/{}",
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

fn word(line: &str, token: &str) -> bool {
    line.match_indices(token).any(|(at, _)| {
        let before = at == 0 || !is_ident(line.as_bytes()[at - 1]);
        let after = at + token.len();
        before && (after >= line.len() || !is_ident(line.as_bytes()[after]))
    })
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
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
        &[CONNECTED, "acme_prod › public", QUERY],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
        ],
    );
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .filter(|line| line.contains(GUTTER))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "focus gutter count {}\n{}",
        lines.len(),
        text(screen)
    );
    lines[0].clone()
}

fn focus_tree(app: &App, name: &str) -> String {
    for _ in 0..24 {
        let line = focused_line(&app.screen());
        if word(&line, name) {
            return line;
        }
        app.key("down");
        let previous = line;
        app.wait_screen(&format!("explorer moved toward {name}"), |screen| {
            focused_line(screen) != previous
        });
    }
    panic!("explorer never focused {name}\n{}", text(&app.screen()));
}

fn line_at(screen: &TerminalSnapshot, needle: &str) -> usize {
    let hits: Vec<usize> = (0..screen.rows())
        .filter(|row| row_text(screen, *row).contains(needle))
        .map(|row| row as usize)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "line {needle:?} got {}\n{}",
        hits.len(),
        text(screen)
    );
    hits[0]
}

/// The menu title is the row above its first command. `marker` is unique to
/// the open menu, so a footer hint that repeats `item` below the menu is ignored.
fn menu_title(screen: &TerminalSnapshot, item: &str, marker: &str) -> String {
    let rows: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let marker_at = line_at(screen, marker);
    let item_at = if rows[marker_at].contains(item) {
        marker_at
    } else {
        (0..marker_at)
            .rev()
            .find(|index| rows[*index].contains(item))
            .unwrap_or_else(|| panic!("no {item:?} above {marker:?}\n{}", rows.join("\n")))
    };
    assert!(item_at > 0, "{item} is the first row\n{}", rows.join("\n"));
    rows[item_at - 1].clone()
}

fn panel_row(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|row| row.contains("public › "))
        .unwrap_or_else(|| panic!("no object title\n{}", text(screen)))
}

fn open_cell_menu(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CELL_MENU) && visible.contains(CELL_TITLE) && !visible.contains(OTHER_TITLE)
}

fn shot(app: &App, checkpoint: &str) {
    let before = app.screen();
    assert!(
        open_cell_menu(&before),
        "checkpoint {checkpoint} left the cell menu\n{}",
        text(&before)
    );
    app.visual(&visual_id(checkpoint));
    let after = app.screen();
    assert!(
        open_cell_menu(&after),
        "checkpoint {checkpoint} did not stay on the cell menu\n{}",
        text(&after)
    );
}

#[test]
fn tablepro_context_route_returns_to_the_row() {
    let app = open("t05-context-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);

    let previous = focus_tree(&app, PREVIOUS);
    assert!(
        word(&previous, PREVIOUS) && !word(&previous, SELECTED),
        "explorer did not stop on {PREVIOUS}: {previous}"
    );
    app.key("shift+f10");
    let menu = app.wait_text(ROW_MENU);
    let title = menu_title(&menu, "Open", ROW_MENU);
    assert!(
        word(&title, PREVIOUS) && !word(&title, SELECTED),
        "row menu title is not {PREVIOUS}: {title}\n{}",
        text(&menu)
    );
    see(&menu, &["Open", ROW_MENU], &[PANEL, OTHER_PANEL]);

    app.key("esc");
    let back = app.wait_screen("row menu dismissed", |screen| {
        let visible = text(screen);
        !visible.contains(ROW_MENU) && word(&focused_line(screen), PREVIOUS)
    });
    assert!(
        word(&focused_line(&back), PREVIOUS),
        "dismiss left the explorer\n{}",
        text(&back)
    );
    see(
        &back,
        &[CONNECTED, QUERY, PREVIOUS],
        &[ROW_MENU, PANEL, OTHER_PANEL, "Copy cell"],
    );
    app.finish();
}

#[test]
fn tablepro_context_actions_use_the_selected_object() {
    let app = open("t05-context-context");
    connect_development(&app);

    let previous = focus_tree(&app, PREVIOUS);
    assert!(
        word(&previous, PREVIOUS) && !word(&previous, SELECTED),
        "the row before products is not customers: {previous}"
    );
    let selected = focus_tree(&app, SELECTED);
    assert!(
        word(&selected, SELECTED) && !word(&selected, PREVIOUS),
        "explorer left customers for a row other than products: {selected}"
    );

    app.key("shift+f10");
    let menu = app.wait_text(ROW_MENU);
    let title = menu_title(&menu, "Open", ROW_MENU);
    assert!(
        word(&title, SELECTED) && !word(&title, PREVIOUS),
        "row menu title is not products: {title}\n{}",
        text(&menu)
    );
    see(
        &menu,
        &["Open", ROW_MENU],
        &[PANEL, "Tables and views only"],
    );

    // Open is the first command. Enter chooses it. A disabled command would
    // stay in the menu, so the products panel would never appear.
    app.key("enter");
    let opened = app.wait_screen("products tab from the row menu", |screen| {
        text(screen).contains(PANEL) && text(screen).contains(ROW_A)
    });
    let panel = panel_row(&opened);
    assert!(
        panel.contains(PANEL) && panel.contains("9 cols") && !panel.contains("preview"),
        "Open did not pin products: {panel}"
    );
    assert!(
        !text(&opened).contains(OTHER_PANEL),
        "Open applied to customers\n{}",
        text(&opened)
    );
    let tabs: Vec<String> = (0..opened.rows())
        .map(|row| row_text(&opened, row))
        .filter(|line| line.contains(QUERY) && word(line, SELECTED))
        .collect();
    assert_eq!(tabs.len(), 1, "products tab row\n{}", text(&opened));
    assert!(
        !word(&tabs[0], PREVIOUS),
        "the tab strip still has customers: {}\n{}",
        tabs[0],
        text(&opened)
    );

    let row_a = app.wait_screen("products sku of row 1", |screen| {
        let line = focused_line(screen);
        line.contains(ROW_A) && !line.contains(ROW_B)
    });
    assert!(
        focused_line(&row_a).contains(ROW_A),
        "row 1 is not the focused products row\n{}",
        text(&row_a)
    );
    app.key("right");
    app.key("down");
    let row_b = app.wait_screen("products sku of row 2", |screen| {
        let line = focused_line(screen);
        line.contains(ROW_B) && !line.contains(ROW_A)
    });
    assert!(
        !focused_line(&row_b).contains(ROW_A),
        "the cursor stayed on row 1: {}",
        focused_line(&row_b)
    );

    app.key("shift+f10");
    let cell = app.wait_screen("open cell menu for sku row 2", open_cell_menu);
    let cell_title = menu_title(&cell, CELL_MENU, CELL_MENU);
    assert!(
        cell_title.contains(CELL_TITLE) && !cell_title.contains(OTHER_TITLE),
        "cell menu title is not sku row 2: {cell_title}\n{}",
        text(&cell)
    );
    see(
        &cell,
        &[CELL_MENU, "Copy row", "Edit cell", CELL_TITLE],
        &[OTHER_TITLE, "Read-only rows", ROW_MENU],
    );

    assert_eq!((cell.cols(), cell.rows()), (120, 40));
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrowed = app.wait_screen("cell menu at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && open_cell_menu(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("cell menu back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && open_cell_menu(screen)
    });
    assert!(
        open_cell_menu(&restored),
        "the cell menu closed on the way back to 120x40\n{}",
        text(&restored)
    );

    app.key("enter");
    let copied = app.wait_screen("copy of the selected sku", |screen| {
        let visible = text(screen);
        !visible.contains(CELL_MENU)
            && (visible.contains("Copied") || visible.contains(OSC_REFUSAL))
    });
    assert!(
        focused_line(&copied).contains(ROW_B) && !focused_line(&copied).contains(ROW_A),
        "copy left the cursor on row 1\n{}",
        text(&copied)
    );
    if text(&copied).contains(OSC_REFUSAL) {
        let status = (0..copied.rows())
            .map(|row| row_text(&copied, row))
            .find(|line| line.contains("Not copied"))
            .unwrap_or_else(|| panic!("missing refusal\n{}", text(&copied)));
        assert!(
            !status.contains(ROW_A) && !status.contains(ROW_B),
            "OSC 52 refusal includes a cell: {status}"
        );
    } else {
        let status = (0..copied.rows())
            .map(|row| row_text(&copied, row))
            .find(|line| line.contains("Copied"))
            .unwrap_or_else(|| panic!("missing copy status\n{}", text(&copied)));
        assert!(
            status.contains(ROW_B) && !status.contains(ROW_A),
            "copy status is not sku row 2: {status}"
        );
    }
    see(
        &copied,
        &[PANEL, ROW_B],
        &[CELL_MENU, CELL_TITLE, OTHER_PANEL, ROW_MENU],
    );
    app.finish();
}
