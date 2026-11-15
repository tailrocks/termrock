//! TablePro Structure tabs, driven through the real binary before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_structure/` and the dotted checkpoint ids use underscores.
//!
//! `customers` is the populated object: its foreign key is `organization_id`
//! to `organizations(id)` ON DELETE CASCADE, and its triggers are
//! `set_updated_at` and `audit_customers`. `products` has `triggers: vec![]`,
//! so its Triggers tab is the empty state and must not keep the previous
//! object's trigger name.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, TABLEPRO, find_all, row_text, text, try_launch};

const WIDE_ID: &str = "pre_refactor/t02_structure/t02_structure_tabs_120x40";
const NARROW_ID: &str = "pre_refactor/t02_structure/t02_structure_tabs_narrow";

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const CATALOG: &str = "read from the catalog";
const QUEUED: &str = "changes are queued until Save";
const NO_FOREIGN_KEYS: &str = "No foreign keys";
const NO_TRIGGERS: &str = "No triggers";
const ORG_TRIGGER: &str = "set_updated_at";
const CUSTOMER_TRIGGER: &str = "audit_customers";
const CUSTOMER_FK: &str = "customers_organization_id_fkey";
const CUSTOMER_INDEX: &str = "customers_org_idx";
const PRODUCT_CHECK: &str = "products_unit_price_check";

fn open() -> App {
    let mut launch = Launch::new("t02-structure", TABLEPRO);
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

fn token_hit(screen: &TerminalSnapshot, point: Point, label: &str) -> bool {
    let line = row_text(screen, u32::from(point.y));
    line.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == label)
        && (point.x == 0
            || !row_text(screen, u32::from(point.y))
                .chars()
                .nth(point.x.saturating_sub(1) as usize)
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
}

fn click_left(app: &App, label: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x <= 36 && token_hit(&screen, *hit, label))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "left hit {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_connect(app: &App) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.x >= 40 && row.contains("Edit") && row.contains("Delete")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

/// The status footer is the last row. The body already contains "Connected to
/// Development" while that footer is still blank, and `visual` compares the
/// next stable frame.
fn footer_connected(screen: &TerminalSnapshot) -> bool {
    let last = screen.rows().saturating_sub(1);
    row_text(screen, last).contains("✓ Connected to Development")
}

fn wide_triggers(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120
        && screen.rows() == 40
        && text(screen).contains(NO_TRIGGERS)
        && text(screen).contains("public › products")
        && footer_connected(screen)
}

fn narrow_connected(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 80
        && screen.rows() == 24
        && text(screen).contains(NO_TRIGGERS)
        && footer_connected(screen)
}

/// The 140ms press flash stays painted until a later frame. A same-size resize
/// does not force that frame, so the nudge goes one column out and back.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
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

fn open_table(app: &App, name: &str) {
    click_left(app, name);
    let title = format!("public › {name}");
    app.wait_text(&title);
    app.key("enter");
    app.wait_text(&title);
}

fn open_object(app: &App, name: &str) {
    click_left(app, name);
    let title = format!("public › {name}");
    app.wait_text(&title);
    flash_settle(app);
    app.key("enter");
    app.wait_text(&title);
}

fn show_structure(app: &App) {
    app.key("ctrl+d");
    let entered = app.wait_text(CATALOG);
    // Focus is the structure table here, so the footer still advertises Ctrl+D.
    // Focusing the tab strip replaces that hint with the section jump keys.
    see(&entered, &["Ctrl+D", "Data / Structure"], &[]);
    click_structure_tab(app, "Columns");
}

fn click_structure_tab(app: &App, label: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            line.contains("Columns") && line.contains("Indexes") && line.contains("Foreign keys")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "structure tab {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    flash_settle(app);
}

fn section(app: &App, key: &str, present: &[&str], absent: &[&str]) -> TerminalSnapshot {
    app.key(key);
    app.wait_screen(&format!("structure {key}"), |screen| {
        let visible = text(screen);
        present.iter().all(|item| visible.contains(item))
            && absent.iter().all(|item| !visible.contains(item))
    })
}

fn footer(count: &str, what: &str) -> String {
    format!("{count} {what} · {CATALOG} · {QUEUED}")
}

/// The connected status lasts five seconds. The structure tour is longer than
/// that on a loaded runner, so the baseline frames are taken on a fresh
/// session that reaches the empty trigger list while the footer is still up.
fn capture_trigger_baselines() {
    let app = open();
    click_left(&app, "Development");
    app.wait_text(HOST);
    click_connect(&app);
    app.wait_screen("connected footer", |screen| {
        text(screen).contains("Query 1") && footer_connected(screen)
    });
    open_table(&app, "customers");
    open_table(&app, "products");
    app.key("ctrl+d");
    app.wait_text(CATALOG);
    click_structure_tab(&app, "Triggers");
    let wide = app.wait_screen("wide triggers", wide_triggers);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    see(
        &wide,
        &[
            NO_TRIGGERS,
            "public › products",
            "✓ Connected to Development",
        ],
        &[ORG_TRIGGER, CUSTOMER_TRIGGER],
    );
    app.visual(WIDE_ID);

    app.resize(80, 24);
    let narrow = app.wait_screen("narrow triggers", narrow_connected);
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[NO_TRIGGERS, CONNECTED],
        &[ORG_TRIGGER, CUSTOMER_TRIGGER],
    );
    app.visual(NARROW_ID);
    app.finish();
}

#[test]
fn structure_tabs_follow_the_open_object_including_an_empty_trigger_list() {
    capture_trigger_baselines();
    let app = open();
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[
            "TablePro",
            "Connections",
            "Filter connections",
            "Development",
            "6 saved",
        ],
        &["Query 1", "CREATE TABLE", NO_TRIGGERS],
    );

    click_left(&app, "Development");
    flash_settle(&app);
    let detail = app.wait_text(HOST);
    see(
        &detail,
        &[HOST, DATABASE, "Development", "Connect"],
        &[
            "Query 1",
            "localhost",
            "analytics.acme.io",
            "staging-db.acme.io",
            "~/scratch.db",
        ],
    );

    click_connect(&app);
    let connected = app.wait_screen("development workbench", |screen| {
        let visible = text(screen);
        visible.contains("Query 1") && visible.contains(CONNECTED)
    });
    assert_eq!((connected.cols(), connected.rows()), (120, 40));
    see(
        &connected,
        &[
            "Query 1",
            CONNECTED,
            "✓",
            "Development",
            "development",
            "acme_prod › public",
        ],
        &[
            "Filter connections",
            "Authentication failed",
            "Could not reach the host",
            "Opening SSH tunnel",
            "analytics.acme.io",
            "~/scratch.db",
        ],
    );

    open_object(&app, "customers");
    show_structure(&app);
    let columns = app.screen();
    see(
        &columns,
        &[
            "public › customers",
            "Columns",
            "Indexes",
            "Foreign keys",
            "Constraints",
            "Triggers",
            "DDL",
            "organization_id",
            "full_name",
            "email",
            "uuid",
            "gen_random_uuid()",
            "1–6",
            &footer("11", "columns"),
        ],
        &["sku", NO_FOREIGN_KEYS, NO_TRIGGERS, CUSTOMER_TRIGGER],
    );

    section(
        &app,
        "2",
        &[
            "customers_pkey",
            "customers_email_key",
            CUSTOMER_INDEX,
            "btree",
            &footer("3", "indexes"),
        ],
        &["gen_random_uuid()", NO_FOREIGN_KEYS, CUSTOMER_FK],
    );
    section(
        &app,
        "3",
        &[
            "organization_id",
            "organizations(id)",
            "CASCADE",
            &footer("1", "foreign keys"),
        ],
        &[
            CUSTOMER_INDEX,
            "customers_email_key",
            NO_FOREIGN_KEYS,
            "btree",
        ],
    );
    section(
        &app,
        "4",
        &[
            "customers_pkey",
            "customers_email_key",
            CUSTOMER_FK,
            "FOREIGN KEY",
            "(organization_id) REFERENCES o…",
            &footer("3", "constraints"),
        ],
        &[CUSTOMER_INDEX, "No constraints", NO_TRIGGERS],
    );
    section(
        &app,
        "5",
        &[
            ORG_TRIGGER,
            "BEFORE UPDATE",
            CUSTOMER_TRIGGER,
            "AFTER INSERT OR UPDATE OR DELETE",
            &footer("2", "triggers"),
        ],
        &[NO_TRIGGERS, CUSTOMER_FK, "CREATE TABLE"],
    );
    section(
        &app,
        "6",
        &[
            "DDL",
            "CREATE TABLE \"public\".\"customers\" (",
            "\"organization_id\" uuid NOT NULL",
            "CONSTRAINT \"customers_organization_id_fkey\" FOREIGN KEY",
            "CREATE INDEX \"customers_org_idx\" ON \"public\".\"customers\"",
        ],
        &[
            ORG_TRIGGER,
            CUSTOMER_TRIGGER,
            NO_TRIGGERS,
            "CREATE TABLE \"public\".\"products\"",
            CATALOG,
        ],
    );

    open_object(&app, "products");
    show_structure(&app);
    let product_columns = app.screen();
    see(
        &product_columns,
        &[
            "public › products",
            "sku",
            "unit_price",
            "numeric(12,2)",
            "'USD'",
            &footer("9", "columns"),
        ],
        &[
            "organization_id",
            "full_name",
            CUSTOMER_TRIGGER,
            "CREATE TABLE \"public\".\"customers\"",
        ],
    );
    section(
        &app,
        "2",
        &[
            "products_pkey",
            "products_sku_key",
            "btree",
            &footer("2", "indexes"),
        ],
        &[CUSTOMER_INDEX, "customers_email_key", NO_FOREIGN_KEYS],
    );
    section(
        &app,
        "3",
        &[NO_FOREIGN_KEYS, &footer("0", "foreign keys")],
        &[
            "organizations(id)",
            "CASCADE",
            CUSTOMER_FK,
            "products_pkey",
            "products_sku_key",
        ],
    );
    section(
        &app,
        "4",
        &[
            PRODUCT_CHECK,
            "products_pkey",
            "products_sku_key",
            "(unit_price >= 0)",
            &footer("3", "constraints"),
        ],
        &["No constraints", NO_FOREIGN_KEYS, CUSTOMER_FK, ORG_TRIGGER],
    );
    section(
        &app,
        "5",
        &[NO_TRIGGERS, "Timing", &footer("0", "triggers")],
        &[
            ORG_TRIGGER,
            CUSTOMER_TRIGGER,
            "BEFORE UPDATE",
            "AFTER INSERT",
            PRODUCT_CHECK,
            "products_pkey",
        ],
    );
    section(
        &app,
        "6",
        &[
            "DDL",
            "CREATE TABLE \"public\".\"products\" (",
            "\"sku\" text NOT NULL",
            "\"unit_price\" numeric(12,2) NOT NULL",
        ],
        &[
            "CREATE TABLE \"public\".\"customers\"",
            ORG_TRIGGER,
            CUSTOMER_TRIGGER,
            NO_TRIGGERS,
            CATALOG,
        ],
    );

    let triggers = section(
        &app,
        "5",
        &[
            "public › products",
            NO_TRIGGERS,
            "Triggers",
            &footer("0", "triggers"),
        ],
        &[
            ORG_TRIGGER,
            CUSTOMER_TRIGGER,
            "BEFORE UPDATE",
            "CREATE TABLE",
            PRODUCT_CHECK,
            "sku",
        ],
    );
    assert_eq!((triggers.cols(), triggers.rows()), (120, 40));
    see(
        &triggers,
        &[NO_TRIGGERS, "public › products"],
        &[ORG_TRIGGER, CUSTOMER_TRIGGER],
    );

    app.resize(80, 24);
    let narrow = app.wait_screen("narrow triggers", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(NO_TRIGGERS)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[NO_TRIGGERS, "products", "public › products"],
        &[ORG_TRIGGER, CUSTOMER_TRIGGER, "BEFORE UPDATE"],
    );

    app.resize(120, 40);
    let restored = app.wait_screen("restored triggers", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(NO_TRIGGERS)
            && text(screen).contains(&footer("0", "triggers"))
    });
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    see(
        &restored,
        &[NO_TRIGGERS, "public › products", &footer("0", "triggers")],
        &[ORG_TRIGGER, CUSTOMER_TRIGGER],
    );

    app.key("ctrl+d");
    let data = app.wait_screen("back on product data", |screen| {
        let visible = text(screen);
        visible.contains("public › products")
            && !visible.contains(NO_TRIGGERS)
            && !visible.contains(CATALOG)
            && !visible.contains("CREATE TABLE")
    });
    see(
        &data,
        &["public › products", "Data", "sku"],
        &[
            NO_TRIGGERS,
            CATALOG,
            "CREATE TABLE",
            ORG_TRIGGER,
            "Foreign keys",
        ],
    );
    app.finish();
}
