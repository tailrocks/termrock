//! TablePro cell viewers, row menus, foreign-key follow, refresh, and fetch-more.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_viewers/` and each checkpoint's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const GUTTER: char = '▎';
const STREET: &str = "104 Calle Mayor";
const PRETTY_LINE1: &str = "\"line1\":\"104 Calle Mayor\"";
const PRETTY_CITY: &str = "\"city\":\"Stockholm\"";
const PRETTY_POSTAL: &str = "\"postal_code\":\"19431\"";
const PRETTY_COUNTRY: &str = "\"country\":\"US\"";
const PRETTY_OTHER_CITY: &str = "\"city\":\"Milan\"";
const RAW_JSON: &str = "{\"line1\": \"104 Calle Mayor\", \"city\": \"Stockholm\", \"postal_code\": \"19431\", \"country\": \"US\"}";
const COPY_HEAD: &str = "Copied 90 characters";
const OSC_REFUSAL: &str = "Not copied · this terminal does not accept OSC 52";
const NOTE_MARK: &str = "DE812345678";
const OTHER_STREET: &str = "Nevsky";
const ORG_ID: &str = "af3e2db9-3aa5-4f25-91ed-6db9875cdecc";
const ORG_MARK: &str = "af3e2db9";
const CUSTOMER_ID: &str = "caa708a8-d44e-42f1-a391-4b5d968ec18c";
const ORG_FILTER: &str = "id = 'af3e2db9-3aa5-4f25-91ed-6db9875cdecc'";
const MENU_TITLE: &str = "shipping_address · row 1";
const MORE_STATUS: &str = "Showing the first 500 rows · add a LIMIT or filter to narrow";
const FETCHING: &str = "fetching…";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t02_viewers/{}",
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
        &[CONNECTED, "acme_prod", QUERY],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
        ],
    );
}

fn open_object(app: &App, name: &str) {
    app.key("ctrl+o");
    app.wait_text("Open Quickly");
    app.type_text(name);
    app.key("enter");
    let crumb = format!("› {name}");
    app.wait_screen(&format!("opened {name}"), |screen| {
        let visible = text(screen);
        visible.contains(&crumb) && !visible.contains("Open Quickly")
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
        &[
            "shipping_address",
            "Close",
            PRETTY_LINE1,
            PRETTY_CITY,
            PRETTY_POSTAL,
            PRETTY_COUNTRY,
        ],
        &[PRETTY_OTHER_CITY, "1 pending", "Apply", "Scratch"],
    );
}

fn assert_narrow_viewer(screen: &TerminalSnapshot) {
    assert_eq!((screen.cols(), screen.rows()), (80, 24));
    // The dialog is 80 columns wide, so this frame may clip the right edge.
    // The title and the head of the cell text are enough.
    see(
        screen,
        &["shipping_address", "Close", STREET],
        &[PRETTY_OTHER_CITY, "1 pending", "Apply", "Scratch"],
    );
}

fn assert_address_focused(screen: &TerminalSnapshot) {
    let line = focus_line(screen);
    assert!(
        line.contains(STREET),
        "orders.shipping_address row 1 left the cell {STREET:?}: {line}"
    );
    assert!(
        !line.contains(OTHER_STREET),
        "focus moved to the neighbor cell: {line}"
    );
}

fn assert_copied(screen: &TerminalSnapshot) {
    assert_address_focused(screen);
    let visible = text(screen);
    if visible.contains(OSC_REFUSAL) {
        let status = line_from(screen, "Not copied");
        assert!(
            !status.contains(STREET) && !status.contains(OTHER_STREET) && !status.contains("Milan"),
            "OSC 52 refusal includes a cell: {status}"
        );
        return;
    }
    let status = line_from(screen, "Copied");
    let full = format!("{COPY_HEAD} · {RAW_JSON}");
    assert!(
        drawn_status(&status, &full),
        "copy status is not orders.shipping_address row 1 ({RAW_JSON:?}): {status}"
    );
    assert!(
        !status.contains(OTHER_STREET) && !status.contains("Milan") && !status.contains(NOTE_MARK),
        "copy status includes a different cell: {status}"
    );
    assert_clean(screen);
}

fn open_shipping_address(app: &App) {
    open_object(app, "orders");
    for _ in 0..7 {
        app.key("right");
    }
    app.wait_screen("orders.shipping_address row 1", |screen| {
        focused_has(screen, STREET) && text(screen).contains("shipping_address")
    });
}

#[test]
fn tablepro_viewers_route_returns_to_query() {
    let app = open("t02-viewers-route");
    connect_development(&app);
    open_object(&app, "orders");
    let table = app.wait_text("public › orders");
    see(
        &table,
        &["public › orders", "Enter Edit"],
        &[
            "Scratch",
            "Close",
            PRETTY_CITY,
            "public › order_items",
            "public › order_totals_by_day",
            "Open Quickly",
        ],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query tab after closing orders", |screen| {
        let visible = text(screen);
        visible.contains("Type SQL") && !visible.contains("public › orders")
    });
    see(
        &back,
        &[QUERY, "Type SQL", "Development"],
        &[
            "public › orders",
            "Close",
            PRETTY_CITY,
            "Scratch",
            "Open Quickly",
        ],
    );
    app.finish();
}

#[test]
fn tablepro_viewers_keeps_the_selected_cell() {
    let app = open("t02-viewers");
    connect_development(&app);
    open_shipping_address(&app);
    app.key("enter");
    let opened = app.wait_text(PRETTY_CITY);
    assert_wide_viewer(&opened);

    flash_settle(&app);
    let wide = app.screen();
    assert_wide_viewer(&wide);
    shot(&app, "T02.viewers.viewer.120x40");

    app.resize(80, 24);
    let narrowed = app.wait_screen("viewer still open at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains("shipping_address")
            && text(screen).contains(STREET)
            && text(screen).contains("Close")
    });
    assert_narrow_viewer(&narrowed);
    flash_settle(&app);
    let narrow = app.screen();
    assert_narrow_viewer(&narrow);
    shot(&app, "T02.viewers.viewer.narrow");
    app.resize(120, 40);
    app.wait_screen("restored 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40
    });

    app.key("esc");
    let closed = app.wait_screen("viewer closed on the shipping cell", |screen| {
        !text(screen).contains(PRETTY_CITY) && focused_has(screen, STREET)
    });
    assert_address_focused(&closed);
    assert_clean(&closed);

    app.key("y");
    let copied = app.wait_screen("copy status for the shipping cell", |screen| {
        let visible = text(screen);
        visible.contains(COPY_HEAD) || visible.contains(OSC_REFUSAL)
    });
    assert_copied(&copied);

    app.key("shift+f10");
    let menu = app.wait_text("Copy cell");
    see(
        &menu,
        &["Copy cell", MENU_TITLE],
        &["1 pending", "Apply", "Scratch"],
    );
    app.key("esc");
    let dismissed = app.wait_screen("row menu closed on the shipping cell", |screen| {
        !text(screen).contains("Copy cell") && focused_has(screen, STREET)
    });
    assert_address_focused(&dismissed);
    assert_clean(&dismissed);

    open_object(&app, "customers");
    app.key("right");
    // The id column is also on this row, so wait until that uuid scrolls off
    // and the focused line is the organization_id cell.
    let source = app.wait_screen("customers.organization_id row 1", |screen| {
        let lines = focus_lines(screen);
        lines.len() == 1
            && lines[0].contains(ORG_MARK)
            && !lines[0].contains(CUSTOMER_ID)
            && text(screen).contains("public › customers")
    });
    let source_line = focus_line(&source);
    assert!(
        source_line.contains(ORG_MARK) && !source_line.contains(CUSTOMER_ID),
        "customers.organization_id row 1 is not the focused cell: {source_line}"
    );
    app.key("ctrl+]");
    // This generated id is not one of the 1,240 organization ids, so the
    // opened table shows the id on the filter chip rather than a data row.
    // The table tab's loading flag stays set, so the empty body says
    // "Loading rows…" instead of "No rows".
    let followed = app.wait_text(ORG_FILTER);
    see(
        &followed,
        &[
            "public › organizations",
            ORG_FILTER,
            "Loading rows…",
            ORG_ID,
        ],
        &[CUSTOMER_ID, "1 pending", "Apply", STREET, "Scratch"],
    );

    app.key("ctrl+w");
    let returned = app.wait_screen("back on the followed customer cell", |screen| {
        text(screen).contains("public › customers")
            && !text(screen).contains("public › organizations")
            && focused_has(screen, ORG_MARK)
    });
    assert!(focus_line(&returned).contains(ORG_MARK));
    assert_clean(&returned);

    app.key("r");
    let refreshed = app.wait_text("Refreshed");
    assert!(
        focused_has(&refreshed, ORG_MARK),
        "refresh left customers.organization_id row 1\n{}",
        text(&refreshed)
    );
    see(
        &refreshed,
        &["Refreshed", "public › customers", ORG_MARK],
        &["1 pending", "Apply", "Scratch"],
    );

    app.key("ctrl+end");
    let more = app.wait_screen("more row of customers", |screen| {
        focused_has(screen, FETCHING)
    });
    let more_line = focus_line(&more);
    assert!(
        more_line.contains(FETCHING),
        "cursor is not on the more row: {more_line}"
    );
    assert!(
        !more_line.contains(ORG_MARK),
        "more row still claims the organization cell: {more_line}"
    );
    app.key("enter");
    let fetched = app.wait_screen("fetch more status", |screen| {
        text(screen).contains("Showing the first 500 rows") && focused_has(screen, FETCHING)
    });
    let fetch_status = line_from(&fetched, "Showing the first 500 rows");
    assert!(
        drawn_status(&fetch_status, MORE_STATUS),
        "fetch-more status: {fetch_status}"
    );
    let fetched_line = focus_line(&fetched);
    assert!(
        fetched_line.contains(FETCHING),
        "fetch more moved off the more row: {fetched_line}"
    );
    app.key("ctrl+home");
    let home = app.wait_screen("organization cell still loaded", |screen| {
        focused_has(screen, ORG_MARK)
    });
    assert!(
        focus_line(&home).contains(ORG_MARK),
        "customers.organization_id row 1 is no longer in the grid"
    );
    see(
        &home,
        &["public › customers"],
        &["1 pending", "Apply", "Scratch"],
    );
    app.finish();
}
