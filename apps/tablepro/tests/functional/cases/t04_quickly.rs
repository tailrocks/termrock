//! TablePro Open Quickly before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t04_quickly/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections. Ctrl+O opens the
//! switcher only after a workbench exists.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const QUERY_NEXT: &str = "Query 2";
const PLACEHOLDER: &str = "Type SQL";
const CATALOG: &str = "acme_prod › public";
const SWITCHER: &str = "Open Quickly";
const SEARCH: &str = "Search tables, views, schemas, tabs, queries…";
const PRODUCTS: &str = "products";
const PRODUCT_PATH: &str = "public · Development";
const OPENED: &str = "Opened public.products";
const BREADCRUMB: &str = "public › products";
const ORDRES: &str = "SELECT * FROM ordres";
const OPEN_TAB: &str = "open tab";
const MISSING: &str = "t04_quickly_missing";
const NO_MATCHES: &str = "No matches";
const RUNNING: &str = "Esc cancels";
const GRID: &str = "SKU-";
const GRID_HINT: &str = "Enter Edit";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t04_quickly/{}",
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
        &[CONNECTED, CATALOG, QUERY, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Loading schema",
            SWITCHER,
        ],
    );
}

fn lines_with(screen: &TerminalSnapshot, needle: &str) -> Vec<(u32, String)> {
    (0..screen.rows())
        .filter_map(|row| {
            let line = row_text(screen, row);
            line.contains(needle).then_some((row, line))
        })
        .collect()
}

fn copies(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// The workbench tab strip is the only line that carries a tab close mark
/// next to both Query 1 and the products tab. The pane title repeats the
/// active name, so a full-screen count is not a tab count.
fn tab_strip(screen: &TerminalSnapshot) -> String {
    let hits: Vec<_> = lines_with(screen, "×")
        .into_iter()
        .filter(|(_, line)| line.contains(QUERY) && line.contains(PRODUCTS))
        .collect();
    assert_eq!(hits.len(), 1, "tab strip\n{}", text(screen));
    hits[0].1.clone()
}

/// The chosen row is the first filtered hit. Its detail is the switcher path,
/// which the explorer does not draw.
fn chosen_product(screen: &TerminalSnapshot) -> String {
    let hits = lines_with(screen, PRODUCT_PATH);
    assert!(!hits.is_empty(), "products path missing\n{}", text(screen));
    let chosen = &hits[0].1;
    assert!(
        chosen.contains(PRODUCTS),
        "chosen row is not products: {chosen}\n{}",
        text(screen)
    );
    assert!(
        !chosen.contains("order_items") && !chosen.contains("ordres"),
        "chosen row is a different object: {chosen}"
    );
    chosen.clone()
}

fn open_switcher(app: &App) -> TerminalSnapshot {
    app.key("ctrl+o");
    let screen = app.wait_text(SWITCHER);
    see(&screen, &[SWITCHER, SEARCH], &[RUNNING]);
    screen
}

fn type_query(app: &App, query: &str) {
    app.type_text(query);
    app.wait_text(query);
}

#[test]
fn route_opens_quickly_and_quits() {
    let app = open("t04-quickly-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[QUERY, PLACEHOLDER, SWITCHER, "Loading schema"],
    );
    connect_development(&app);
    let opened = open_switcher(&app);
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    see(&opened, &[SWITCHER, SEARCH, QUERY], &["Loading schema"]);
    app.key("esc");
    let back = app.wait_screen("switcher closed", |screen| {
        let visible = text(screen);
        !visible.contains(SWITCHER)
            && visible.contains(QUERY)
            && visible.contains(CONNECTED)
            && visible.contains(PLACEHOLDER)
    });
    see(
        &back,
        &[QUERY, CONNECTED, PLACEHOLDER, CATALOG],
        &[SWITCHER, NO_MATCHES, "Loading schema", OPENED],
    );
    app.finish();
}

#[test]
fn switcher_opens_table_history_query_and_cancel_keeps_the_tab() {
    let app = open("t04-quickly-switcher");
    connect_development(&app);

    let opened = open_switcher(&app);
    see(&opened, &[SWITCHER, "Enter Open"], &[OPENED, ORDRES]);
    type_query(&app, PRODUCTS);
    let products = app.wait_screen("products row is the chosen switcher row", |screen| {
        text(screen).contains(SWITCHER)
            && text(screen).contains(PRODUCTS)
            && lines_with(screen, PRODUCT_PATH)
                .first()
                .is_some_and(|(_, line)| line.contains(PRODUCTS))
    });
    let chosen = chosen_product(&products);
    assert!(
        chosen.contains("Tables") || chosen.contains(PRODUCT_PATH),
        "products row lost its switcher path: {chosen}"
    );
    assert_eq!((products.cols(), products.rows()), (120, 40));
    app.visual(&visual_id("T04.quickly.switcher.120x40"));

    app.resize(80, 24);
    let narrow = app.wait_screen("switcher still open at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(SWITCHER)
            && text(screen).contains(PRODUCTS)
            && !text(screen).contains(OPENED)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[SWITCHER, PRODUCTS],
        &[OPENED, ORDRES, NO_MATCHES],
    );
    app.visual(&visual_id("T04.quickly.switcher.narrow"));

    app.resize(120, 40);
    let restored = app.wait_screen("switcher back at 120x40 before enter", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(SWITCHER)
            && lines_with(screen, PRODUCT_PATH)
                .first()
                .is_some_and(|(_, line)| line.contains(PRODUCTS))
    });
    assert!(
        chosen_product(&restored).contains(PRODUCTS),
        "resize changed the chosen row\n{}",
        text(&restored)
    );
    app.key("enter");
    let table = app.wait_screen("products tab opened", |screen| {
        let visible = text(screen);
        visible.contains(OPENED)
            && visible.contains(QUERY)
            && visible.contains(BREADCRUMB)
            && visible.contains(GRID)
            && visible.contains(GRID_HINT)
            && !visible.contains(SWITCHER)
            && !visible.contains("Loading schema")
    });
    see(
        &table,
        &[OPENED, QUERY, PRODUCTS, BREADCRUMB, GRID, GRID_HINT],
        &[SWITCHER, ORDRES, RUNNING, NO_MATCHES],
    );
    assert_eq!(
        copies(&tab_strip(&table), QUERY),
        1,
        "opening products duplicated Query 1\n{}",
        text(&table)
    );

    open_switcher(&app);
    type_query(&app, "ordres");
    let history = app.wait_screen("ordres history row is chosen", |screen| {
        let hits = lines_with(screen, ORDRES);
        if !(text(screen).contains(SWITCHER) && hits.len() == 1 && hits[0].1.contains("Recent")) {
            return false;
        }
        // A weaker fuzzy hit can sit below the chosen row. The chosen row is
        // the first one, so every table path is under the ordres line.
        lines_with(screen, PRODUCT_PATH)
            .iter()
            .all(|(row, _)| *row > hits[0].0)
    });
    see(
        &history,
        &[SWITCHER, ORDRES, "Recent queries"],
        &[NO_MATCHES],
    );
    let query_tabs_before = copies(&tab_strip(&history), QUERY);
    app.key("enter");
    let query = app.wait_screen("history sql opened without running", |screen| {
        let visible = text(screen);
        visible.contains(ORDRES)
            && visible.contains(QUERY_NEXT)
            && visible.contains("No results yet")
            && !visible.contains(SWITCHER)
            && !visible.contains(RUNNING)
            && !visible.contains(GRID)
            && !visible.contains(BREADCRUMB)
            && !visible.contains("does not exist")
    });
    see(
        &query,
        &[ORDRES, QUERY, QUERY_NEXT, PRODUCTS, "No results yet"],
        &[
            SWITCHER,
            RUNNING,
            GRID,
            BREADCRUMB,
            "does not exist",
            NO_MATCHES,
        ],
    );
    assert!(
        !text(&query).contains(PLACEHOLDER),
        "the new query tab is not the active editor\n{}",
        text(&query)
    );
    let query_strip = tab_strip(&query);
    assert_eq!(
        copies(&query_strip, QUERY),
        query_tabs_before,
        "history open changed the Query 1 tab\n{}",
        text(&query)
    );
    assert_eq!(
        copies(&query_strip, QUERY_NEXT),
        1,
        "history open did not add one query tab\n{query_strip}"
    );

    let strip_before = copies(&query_strip, QUERY);
    open_switcher(&app);
    type_query(&app, QUERY);
    let tabs = app.wait_screen("Query 1 open-tab row is the only tab hit", |screen| {
        let hits = lines_with(screen, OPEN_TAB);
        text(screen).contains(SWITCHER)
            && hits.len() == 1
            && hits[0].1.contains(QUERY)
            && !hits[0].1.contains(QUERY_NEXT)
    });
    see(&tabs, &[SWITCHER, OPEN_TAB, "Open tabs"], &[NO_MATCHES]);
    app.key("enter");
    let back = app.wait_screen("switched to the existing Query 1", |screen| {
        let visible = text(screen);
        !visible.contains(SWITCHER)
            && visible.contains(PLACEHOLDER)
            && visible.contains(QUERY_NEXT)
            && !visible.contains(ORDRES)
            && !visible.contains(GRID)
            && !visible.contains(BREADCRUMB)
    });
    see(
        &back,
        &[
            QUERY,
            QUERY_NEXT,
            PRODUCTS,
            PLACEHOLDER,
            OPENED,
            "No results yet",
        ],
        &[SWITCHER, ORDRES, RUNNING, GRID, BREADCRUMB, NO_MATCHES],
    );
    let back_strip = tab_strip(&back);
    assert_eq!(
        copies(&back_strip, QUERY),
        strip_before,
        "choosing the open Query 1 tab duplicated it\n{back_strip}"
    );
    assert_eq!(
        copies(&back_strip, QUERY_NEXT),
        1,
        "lost Query 2\n{back_strip}"
    );

    let active_query_count = copies(&back_strip, QUERY);
    open_switcher(&app);
    type_query(&app, MISSING);
    let missing = app.wait_screen("no matches", |screen| {
        text(screen).contains(NO_MATCHES) && text(screen).contains(MISSING)
    });
    see(
        &missing,
        &[SWITCHER, NO_MATCHES, MISSING],
        &[PRODUCT_PATH, ORDRES, OPEN_TAB],
    );
    app.key("enter");
    let stayed = app.wait_screen("enter on no matches leaves the switcher open", |screen| {
        let visible = text(screen);
        visible.contains(SWITCHER)
            && visible.contains(NO_MATCHES)
            && visible.contains(MISSING)
            && visible.contains(PLACEHOLDER)
            && !visible.contains(PRODUCT_PATH)
            && !visible.contains(GRID)
            && !visible.contains(BREADCRUMB)
            && !visible.contains("Opened public.change_log")
            && !visible.contains(ORDRES)
    });
    see(
        &stayed,
        &[SWITCHER, NO_MATCHES, MISSING, QUERY, PLACEHOLDER],
        &[
            ORDRES,
            GRID,
            BREADCRUMB,
            RUNNING,
            "Opened public.change_log",
            "does not exist",
        ],
    );
    assert_eq!(
        copies(&tab_strip(&stayed), QUERY),
        active_query_count,
        "no-match enter changed Query 1\n{}",
        text(&stayed)
    );
    app.key("esc");
    let cleared = app.wait_screen("esc clears the query and keeps the switcher", |screen| {
        let visible = text(screen);
        visible.contains(SWITCHER)
            && !visible.contains(MISSING)
            && !visible.contains(NO_MATCHES)
            && visible.contains(SEARCH)
    });
    see(
        &cleared,
        &[SWITCHER, SEARCH],
        &[MISSING, NO_MATCHES, ORDRES],
    );
    app.key("esc");
    let closed = app.wait_screen("second esc closes the switcher on Query 1", |screen| {
        let visible = text(screen);
        !visible.contains(SWITCHER)
            && visible.contains(PLACEHOLDER)
            && visible.contains(QUERY_NEXT)
            && !visible.contains(ORDRES)
            && !visible.contains(GRID)
            && !visible.contains(BREADCRUMB)
    });
    see(
        &closed,
        &[
            QUERY,
            QUERY_NEXT,
            PRODUCTS,
            PLACEHOLDER,
            OPENED,
            "No results yet",
        ],
        &[
            SWITCHER, MISSING, NO_MATCHES, ORDRES, RUNNING, GRID, BREADCRUMB,
        ],
    );
    let closed_strip = tab_strip(&closed);
    assert_eq!(
        copies(&closed_strip, QUERY),
        active_query_count,
        "cancel changed the active Query 1 tab\n{closed_strip}"
    );
    assert_eq!(
        copies(&closed_strip, QUERY_NEXT),
        1,
        "cancel dropped Query 2\n{closed_strip}"
    );
    app.finish();
}
