//! TablePro cell viewer: the open cell, copy, and close.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_viewers/` and each checkpoint's dots become underscores.
//!
//! `Dialog::facts` paints the viewer body as plain lines (`dialog.rs`). That
//! body has no text-selection widget and no copy command. `y` is
//! `GridCmd::Copy` (`grid.rs`), and it copies `CellValue::text()` of the
//! cursor cell, which is the raw JSON `OpenViewer` pretty-prints. The dialog
//! consumes keys while it is open, so that copy runs after Close returns
//! focus to the same cell.

use tui_test::TerminalSnapshot;

use crate::bins::tablepro;
use crate::support::{App, Launch, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const GUTTER: char = '▎';
const TABLE: &str = "public › products";
const COLUMN: &str = "attributes";
const NAME: &str = "Monitor arm";
const NEIGHBOR_NAME: &str = "Standing desk 140";
/// `products` row 0 (`i = 0`) from `db::rows`. Row 1 is the neighbor.
const RAW: &str = r#"{"color": "white", "weight_kg": 21.1}"#;
const NEIGHBOR: &str = r#"{"color": "graphite", "weight_kg": 18.8}"#;
const PRETTY_COLOR: &str = r#""color":"white","#;
const PRETTY_WEIGHT: &str = r#""weight_kg":21.1"#;
const PRETTY_OTHER_COLOR: &str = r#""color":"graphite""#;
const PRETTY_OTHER_WEIGHT: &str = r#""weight_kg":18.8"#;
const PREVIOUS_COPY: &str = "Copied 11 characters · Monitor arm";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_viewers/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn copy_status() -> String {
    format!("Copied {} characters · {RAW}", RAW.chars().count())
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
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod", QUERY],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
        ],
    );
}

fn open_products(app: &App) {
    app.key("ctrl+o");
    app.wait_text("Open Quickly");
    app.type_text("products");
    app.key("enter");
    app.wait_screen("products table", |screen| {
        let visible = text(screen);
        visible.contains(TABLE)
            && visible.contains(NAME)
            && visible.contains("SKU-01000")
            && visible.contains("9 cols")
            && !visible.contains("Open Quickly")
            && !visible.contains("Loading rows")
    });
}

fn focus_lines(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .filter_map(|row| {
            let line = row_text(screen, row);
            line.contains(GUTTER).then_some(line)
        })
        .collect()
}

fn focused_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    let lines = focus_lines(screen);
    lines.len() == 1 && lines[0].contains(needle)
}

fn focus_line(screen: &TerminalSnapshot) -> String {
    let lines = focus_lines(screen);
    assert_eq!(
        lines.len(),
        1,
        "focus gutter count {}\n{}",
        lines.len(),
        text(screen)
    );
    lines[0].clone()
}

fn line_from(screen: &TerminalSnapshot, marker: &str) -> String {
    let line = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(marker))
        .unwrap_or_else(|| panic!("missing {marker:?}\n{}", text(screen)));
    let start = line.find(marker).expect(marker);
    line[start..].trim().to_string()
}

/// Footer truncation keeps a prefix and may add its own ellipsis.
fn drawn_status(drawn: &str, full: &str) -> bool {
    if full.starts_with(drawn) {
        return true;
    }
    drawn
        .strip_suffix('…')
        .is_some_and(|head| !head.is_empty() && full.starts_with(head))
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn assert_clean(screen: &TerminalSnapshot) {
    see(screen, &[], &["1 pending", "Apply", "Scratch"]);
}

fn assert_wide_viewer(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    see(
        screen,
        &[COLUMN, "Close", PRETTY_COLOR, PRETTY_WEIGHT],
        &[
            PRETTY_OTHER_COLOR,
            PRETTY_OTHER_WEIGHT,
            "Cancel",
            "1 pending",
            "Apply",
            "Scratch",
        ],
    );
}

fn assert_narrow_viewer(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (80, 24));
    see(
        screen,
        &[COLUMN, "Close", PRETTY_COLOR],
        &[
            PRETTY_OTHER_COLOR,
            "Cancel",
            "1 pending",
            "Apply",
            "Scratch",
        ],
    );
}

fn assert_attributes_focused(screen: &TerminalSnapshot) {
    let line = focus_line(screen);
    assert!(
        line.contains("\"color\": \"white\"") && line.contains("21.1"),
        "products.attributes row 1 left the cell: {line}"
    );
    assert!(
        !line.contains("graphite") && !line.contains("18.8"),
        "focus moved to the neighbor cell: {line}"
    );
}

fn move_right(app: &App, n: usize) {
    for _ in 0..n {
        app.key("right");
    }
}

#[test]
fn tablepro_viewers_route_reaches_the_products_table() {
    let app = open("t05-viewers-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &["Filter connections", "Development"], &[QUERY]);
    connect_development(&app);
    open_products(&app);
    let table = app.screen();
    see(
        &table,
        &[TABLE, NAME, "SKU-01000", "9 cols", "Enter Edit", QUERY],
        &[
            "Close",
            PRETTY_COLOR,
            "Cancel",
            "Scratch",
            "Open Quickly",
            "Loading schema",
        ],
    );
    assert!(
        focused_has(&table, NAME) && !focus_line(&table).contains(NEIGHBOR_NAME),
        "opened on a neighbor row\n{}",
        text(&table)
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query tab after closing products", |screen| {
        let visible = text(screen);
        visible.contains("Type SQL") && !visible.contains(TABLE)
    });
    see(
        &back,
        &[QUERY, "Type SQL", "Development"],
        &[TABLE, "Close", PRETTY_COLOR, "Scratch", "Open Quickly"],
    );
    app.finish();
}

#[test]
fn tablepro_viewers_copies_the_open_cell() {
    let app = open("t05-viewers");
    connect_development(&app);
    open_products(&app);

    // Column 0 is id. Two Rights land on name, the previously focused cell.
    move_right(&app, 2);
    let name = app.wait_screen("products.name row 1", |screen| {
        focused_has(screen, NAME) && !focus_line(screen).contains(NEIGHBOR_NAME)
    });
    assert!(focus_line(&name).contains(NAME));
    app.key("y");
    let previous = app.wait_text(PREVIOUS_COPY);
    let previous_status = line_from(&previous, "Copied");
    assert!(
        drawn_status(&previous_status, PREVIOUS_COPY),
        "previous cell copy: {previous_status}"
    );
    assert!(
        !previous_status.contains("weight_kg") && !previous_status.contains("graphite"),
        "name copy included a json cell: {previous_status}"
    );

    // name → description → unit_price → currency → is_active → attributes.
    move_right(&app, 5);
    let cell = app.wait_screen("products.attributes row 1", |screen| {
        focused_has(screen, "\"color\": \"white\"")
            && focused_has(screen, "21.1")
            && text(screen).contains(COLUMN)
    });
    assert_attributes_focused(&cell);
    assert!(
        !focus_line(&cell).contains("graphite"),
        "cursor is on the neighbor attributes cell\n{}",
        focus_line(&cell)
    );

    app.key("enter");
    let opened = app.wait_text(PRETTY_COLOR);
    assert_wide_viewer(&opened);
    assert!(
        text(&opened).contains(PRETTY_WEIGHT),
        "viewer body is not row 0's pretty json\n{}",
        text(&opened)
    );

    flash_settle(&app);
    let wide = app.screen();
    assert_wide_viewer(&wide);
    shot(&app, "T05.viewers.viewer.120x40");

    app.resize(80, 24);
    let narrowed = app.wait_screen("viewer still open at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(COLUMN)
            && text(screen).contains("Close")
            && text(screen).contains(PRETTY_COLOR)
            && !text(screen).contains(PRETTY_OTHER_COLOR)
    });
    assert_narrow_viewer(&narrowed);
    flash_settle(&app);
    let narrow = app.screen();
    assert_narrow_viewer(&narrow);
    shot(&app, "T05.viewers.viewer.narrow");

    app.resize(120, 40);
    let restored = app.wait_screen("viewer back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(PRETTY_WEIGHT)
    });
    assert_wide_viewer(&restored);

    // `y` is not a dialog command. The facts body cannot select a span, and
    // the key must not copy the previously focused name cell either.
    app.key("y");
    let ignored = app.wait_screen("viewer keeps the previous copy", |screen| {
        text(screen).contains(PRETTY_COLOR)
            && text(screen).contains("Close")
            && text(screen).contains(PREVIOUS_COPY)
            && !text(screen).contains(&copy_status())
    });
    assert_wide_viewer(&ignored);

    app.key("esc");
    let closed = app.wait_screen("viewer closed on the attributes cell", |screen| {
        !text(screen).contains(PRETTY_COLOR)
            && !text(screen).contains("Close")
            && focused_has(screen, "\"color\": \"white\"")
            && focused_has(screen, "21.1")
    });
    assert_attributes_focused(&closed);
    assert_clean(&closed);
    see(
        &closed,
        &[TABLE, COLUMN, PREVIOUS_COPY],
        &[
            PRETTY_COLOR,
            PRETTY_WEIGHT,
            PRETTY_OTHER_COLOR,
            "Close",
            "Cancel",
        ],
    );

    app.key("y");
    let copied = app.wait_text(&format!("Copied {} characters", RAW.chars().count()));
    let status = line_from(&copied, "Copied");
    let full = copy_status();
    assert!(
        drawn_status(&status, &full),
        "copy status is not the open viewer's cell ({RAW:?}): {status}"
    );
    assert!(
        !status.contains(NAME)
            && !status.contains(NEIGHBOR_NAME)
            && !status.contains(NEIGHBOR)
            && !status.contains("graphite")
            && !status.contains("18.8"),
        "copy status is the previously focused cell or the neighbor: {status}"
    );
    assert_attributes_focused(&copied);
    assert_clean(&copied);
    app.finish();
}
