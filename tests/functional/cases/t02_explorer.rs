//! TablePro schema explorer, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_explorer/` and the dotted checkpoint ids use underscores.
//! TablePro has no `--page` flag. It opens on the connections screen, and a
//! paused connect never finishes (`on_tick` returns before `advance`).

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, TABLEPRO, find_all, row_text, text, try_launch};

const WIDE_ID: &str = "pre_refactor/t02_explorer/t02_explorer_tree_120x40";
const NARROW_ID: &str = "pre_refactor/t02_explorer/t02_explorer_tree_narrow";

const HOST: &str = "dev-db.internal.acme.io:5432";
const DATABASE: &str = "acme_dev";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Filter connections";
const OBJECTS: &str = "Filter objects";
const TITLE: &str = "public › ";
const TITLE_ORGANIZATIONS: &str = "public › organizations";
const TITLE_CUSTOMERS: &str = "public › customers";
const TITLE_PRODUCTS: &str = "public › products";
const PREVIEW: &str = "preview";
const ORGANIZATIONS: &str = "organizations";
const CUSTOMERS: &str = "customers";
const PRODUCTS: &str = "products";
const EVENTS: &str = "events";
const QUALIFIED_ORGANIZATIONS: &str = "public.organizations";
const UNREACHABLE: &str = "Could not reach the host";
const AUTH_FAILED: &str = "Authentication failed";

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(PLACEHOLDER);
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

fn assert_size(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "terminal size\n{}",
        text(screen)
    );
}

fn token_at(row: &str, col: usize, token: &str) -> bool {
    let chars: Vec<char> = row.chars().collect();
    let width = token.chars().count();
    if col + width > chars.len() {
        return false;
    }
    if chars[col..col + width].iter().collect::<String>() != token {
        return false;
    }
    let before = col > 0 && (chars[col - 1].is_ascii_alphanumeric() || chars[col - 1] == '_');
    let after = col + width < chars.len()
        && (chars[col + width].is_ascii_alphanumeric() || chars[col + width] == '_');
    !before && !after
}

/// The explorer is the left pane. A tab or a panel title can repeat the same
/// word further right or higher on the screen, so the tree hit is the lowest
/// token that is still inside the pane and is not the object title.
fn in_explorer(screen: &TerminalSnapshot, hit: Point, token: &str) -> bool {
    let row = row_text(screen, u32::from(hit.y));
    hit.x < 36
        && token_at(&row, usize::from(hit.x), token)
        && !row.contains(TITLE)
        && !row.contains(QUERY)
}

fn tree_hit(screen: &TerminalSnapshot, token: &str) -> Point {
    let mut hits: Vec<Point> = find_all(screen, token)
        .into_iter()
        .filter(|hit| in_explorer(screen, *hit, token))
        .collect();
    assert!(
        !hits.is_empty(),
        "no explorer hit for {token:?}\n{}",
        text(screen)
    );
    hits.sort_by_key(|hit| hit.y);
    hits.pop().expect("explorer hit")
}

fn tree_has(screen: &TerminalSnapshot, token: &str) -> bool {
    find_all(screen, token)
        .into_iter()
        .any(|hit| in_explorer(screen, hit, token))
}

fn click_tree(app: &App, token: &str) {
    let screen = app.wait_text(token);
    app.click(tree_hit(&screen, token));
}

fn click_listed(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| {
            hit.x <= 35
                && token_at(
                    &row_text(&screen, u32::from(hit.y)),
                    usize::from(hit.x),
                    label,
                )
        })
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
        &[
            "Development",
            HOST,
            DATABASE,
            "acme_app",
            "development",
            "PostgreSQL",
        ],
        &[QUERY, UNREACHABLE, AUTH_FAILED, "Reconnect", OBJECTS],
    );
    click_connect(app);
    let workbench = app.wait_screen("development workbench", |screen| {
        let visible = text(screen);
        visible.contains(QUERY)
            && visible.contains(OBJECTS)
            && visible.contains(ORGANIZATIONS)
            && visible.contains("Development")
            && !visible.contains(HOST)
    });
    assert_size(&workbench, 120, 40);
    see(
        &workbench,
        &[
            QUERY,
            OBJECTS,
            ORGANIZATIONS,
            CUSTOMERS,
            "Development",
            "Explorer",
        ],
        &[UNREACHABLE, AUTH_FAILED, HOST, "Opening SSH tunnel…"],
    );
}

/// The workbench header crumb is `acme_prod › public`. The open object is the
/// framed panel title `public › <name>`, which is a different string.
fn panel(screen: &TerminalSnapshot) -> Option<(String, String)> {
    let row = (0..screen.rows())
        .map(|index| row_text(screen, index))
        .find(|row| row.contains(TITLE))?;
    let at = row.find(TITLE)?;
    let name: String = row[at + TITLE.len()..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some((name, row))
    }
}

fn tab_row(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|index| row_text(screen, index))
        .find(|row| row.contains(QUERY) && row.contains('×'))
        .unwrap_or_default()
}

fn assert_open(screen: &TerminalSnapshot, name: &str, preview: bool) {
    let (opened, row) =
        panel(screen).unwrap_or_else(|| panic!("no object title\n{}", text(screen)));
    assert_eq!(opened, name, "opened {opened:?}\n{}", text(screen));
    assert_eq!(
        row.contains(PREVIEW),
        preview,
        "preview mark on {row:?}\n{}",
        text(screen)
    );
    assert!(
        !text(screen).contains(QUALIFIED_ORGANIZATIONS),
        "duplicate organizations tab\n{}",
        text(screen)
    );
}

/// The 140ms press flash stays painted until the next frame after the deadline.
/// A same-size resize does not force that frame, so nudge one column and return.
/// Below about 85 columns the header omits the size chip.
fn flash_settle(app: &App) {
    app.settle_flash();
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    if cols >= 100 {
        app.wait_text(&format!("{nudged}×{rows}"));
        app.resize(cols, rows);
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow nudge", |screen| {
            screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
        });
        app.resize(cols, rows);
        app.wait_screen("narrow restored", |screen| {
            screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
        });
    }
}

fn expand_analytics(app: &App) {
    click_tree(app, "analytics");
    app.key("right");
    app.wait_screen("analytics schema expanded", |screen| {
        text(screen).matches("Tables").count() >= 2
    });
    app.key("right");
    app.key("right");
    app.wait_text(EVENTS);
}

#[test]
fn tablepro_explorer_route_connects_development_and_quits() {
    let app = open("t02-explorer-route");
    let landing = app.settle();
    assert_size(&landing, 120, 40);
    see(
        &landing,
        &[
            "TablePro",
            "Connections",
            PLACEHOLDER,
            "truecolor · 120×40",
            "Development",
            "Analytics",
        ],
        &[QUERY, OBJECTS, UNREACHABLE, AUTH_FAILED],
    );
    connect_development(&app);
    app.finish();
}

#[test]
fn tablepro_explorer_tree_keeps_the_selected_object() {
    let app = open("t02-explorer-tree");
    connect_development(&app);
    expand_analytics(&app);
    let expanded = app.wait_text(EVENTS);
    see(
        &expanded,
        &[EVENTS, "analytics", ORGANIZATIONS, CUSTOMERS, QUERY],
        &[UNREACHABLE, QUALIFIED_ORGANIZATIONS],
    );

    click_tree(&app, ORGANIZATIONS);
    let preview = app.wait_screen("organizations preview", |screen| {
        panel(screen).is_some_and(|(name, row)| name == ORGANIZATIONS && row.contains(PREVIEW))
    });
    assert_open(&preview, ORGANIZATIONS, true);
    see(&preview, &[QUERY, CUSTOMERS], &[TITLE_CUSTOMERS]);

    click_tree(&app, ORGANIZATIONS);
    let pinned = app.wait_screen("organizations pinned", |screen| {
        panel(screen).is_some_and(|(name, row)| name == ORGANIZATIONS && !row.contains(PREVIEW))
    });
    assert_open(&pinned, ORGANIZATIONS, false);
    see(
        &pinned,
        &[QUERY, EVENTS, ORGANIZATIONS, "9 cols"],
        &[TITLE_CUSTOMERS, QUALIFIED_ORGANIZATIONS],
    );
    assert!(
        tab_row(&pinned).contains(ORGANIZATIONS) && tab_row(&pinned).contains(QUERY),
        "pinned tab row\n{}",
        tab_row(&pinned)
    );

    flash_settle(&app);
    app.visual(WIDE_ID);
    app.resize(80, 24);
    app.wait_screen("narrow size", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    flash_settle(&app);
    let narrow = app.screen();
    assert_size(&narrow, 80, 24);
    app.visual(NARROW_ID);
    let captured = app.screen();
    assert_size(&captured, 80, 24);
    app.resize(120, 40);
    let restored = app.wait_screen("restored 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && panel(screen).is_some_and(|(name, _)| name == ORGANIZATIONS)
    });
    assert_open(&restored, ORGANIZATIONS, false);

    click_tree(&app, CUSTOMERS);
    let customers = app.wait_screen("customers preview", |screen| {
        panel(screen).is_some_and(|(name, row)| name == CUSTOMERS && row.contains(PREVIEW))
    });
    assert_open(&customers, CUSTOMERS, true);
    see(
        &customers,
        &[QUERY, ORGANIZATIONS, CUSTOMERS],
        &[TITLE_ORGANIZATIONS],
    );

    click_tree(&app, PRODUCTS);
    let replaced = app.wait_screen("products replaces the preview", |screen| {
        panel(screen).is_some_and(|(name, row)| name == PRODUCTS && row.contains(PREVIEW))
    });
    assert_open(&replaced, PRODUCTS, true);
    see(
        &replaced,
        &[QUERY, ORGANIZATIONS, PRODUCTS],
        &[TITLE_CUSTOMERS, TITLE_ORGANIZATIONS],
    );
    let tabs = tab_row(&replaced);
    assert!(
        tabs.contains(ORGANIZATIONS) && tabs.contains(PRODUCTS) && tabs.contains(QUERY),
        "pinned organizations stayed while products took the preview slot\n{tabs}"
    );
    assert!(
        !tabs.contains(CUSTOMERS),
        "customers preview was replaced\n{tabs}"
    );

    click_tree(&app, ORGANIZATIONS);
    let reused = app.wait_screen("organizations reused", |screen| {
        panel(screen).is_some_and(|(name, row)| name == ORGANIZATIONS && !row.contains(PREVIEW))
    });
    assert_open(&reused, ORGANIZATIONS, false);
    see(
        &reused,
        &[QUERY, PRODUCTS, ORGANIZATIONS],
        &[TITLE_PRODUCTS, TITLE_CUSTOMERS, QUALIFIED_ORGANIZATIONS],
    );

    app.key("/");
    app.type_text(CUSTOMERS);
    let filtered = app.wait_screen("tree filtered to customers", |screen| {
        text(screen).contains(CUSTOMERS)
            && tree_has(screen, CUSTOMERS)
            && !tree_has(screen, ORGANIZATIONS)
            && !tree_has(screen, PRODUCTS)
    });
    assert!(
        tree_has(&filtered, CUSTOMERS) && !tree_has(&filtered, ORGANIZATIONS),
        "filter rows\n{}",
        text(&filtered)
    );

    click_tree(&app, CUSTOMERS);
    let filtered_open = app.wait_screen("filtered customers open", |screen| {
        panel(screen).is_some_and(|(name, row)| name == CUSTOMERS && row.contains(PREVIEW))
    });
    assert_open(&filtered_open, CUSTOMERS, true);
    see(
        &filtered_open,
        &[QUERY, ORGANIZATIONS],
        &[TITLE_ORGANIZATIONS, TITLE_PRODUCTS],
    );

    click_tree(&app, CUSTOMERS);
    let filtered_again = app.wait_screen("filtered customers again", |screen| {
        panel(screen).is_some_and(|(name, row)| name == CUSTOMERS && !row.contains(PREVIEW))
    });
    assert_open(&filtered_again, CUSTOMERS, false);
    see(
        &filtered_again,
        &[QUERY, ORGANIZATIONS, CUSTOMERS],
        &[TITLE_ORGANIZATIONS, QUALIFIED_ORGANIZATIONS],
    );
    app.finish();
}
