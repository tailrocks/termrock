//! TablePro explorer preview reuse before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t04_preview/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.
//!
//! A first explorer click on a table opens a preview. The second click on that
//! same object pins it (`workbench.rs` clears `tt.preview` only when `preview`
//! is false). A later preview replaces the unpinned preview tab in place and
//! leaves a pinned table and a dirty query alone.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, TABLEPRO, find_all, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY_1: &str = "Query 1";
const QUERY_2: &str = "Query 2";
const PLACEHOLDER: &str = "Type SQL";
const TITLE: &str = "public › ";
const PRODUCTS: &str = "products";
const ORGANIZATIONS: &str = "organizations";
const CUSTOMERS: &str = "customers";
const PREVIEW: &str = "preview";
const DIRTY_SQL: &str = "SELECT 't04_preview_dirty' AS marker";
const MARKER: &str = "t04_preview_dirty";
const PRODUCTS_PREVIEW: &str = "preview · 9 cols";
const PRODUCTS_PINNED: &str = "9 cols";
const ORGANIZATIONS_PREVIEW: &str = "preview · 9 cols";
const CUSTOMERS_PREVIEW: &str = "preview · 11 cols";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t04_preview/{}",
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

fn token_count(row: &str, token: &str) -> usize {
    let chars: Vec<char> = row.chars().collect();
    let width = token.chars().count();
    let mut count = 0;
    let mut index = 0;
    while index + width <= chars.len() {
        if token_at(row, index, token) {
            count += 1;
            index += width;
        } else {
            index += 1;
        }
    }
    count
}

/// The explorer is the left pane. A tab or a panel title can repeat the same
/// word further right, so the tree hit is the lowest token still inside the pane.
fn in_explorer(screen: &TerminalSnapshot, hit: Point, token: &str) -> bool {
    let row = row_text(screen, u32::from(hit.y));
    hit.x < 36
        && token_at(&row, usize::from(hit.x), token)
        && !row.contains(TITLE)
        && !row.contains("Query ")
}

fn click_tree(app: &App, token: &str) {
    let screen = app.wait_text(token);
    let mut hits: Vec<Point> = find_all(&screen, token)
        .into_iter()
        .filter(|hit| in_explorer(&screen, *hit, token))
        .collect();
    assert!(
        !hits.is_empty(),
        "no explorer hit for {token:?}\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.y);
    let hit = hits.pop().expect("explorer hit");
    app.click(hit);
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
        &[CONNECTED, QUERY_1, "acme_prod › public"],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "Loading schema",
            PRODUCTS_PREVIEW,
        ],
    );
}

fn object_row(screen: &TerminalSnapshot) -> Option<String> {
    (0..screen.rows())
        .map(|index| row_text(screen, index))
        .find(|row| row.contains(TITLE))
}

fn tab_line(screen: &TerminalSnapshot) -> Option<String> {
    let rows: Vec<_> = (0..screen.rows())
        .map(|index| row_text(screen, index))
        .filter(|row| row.contains(QUERY_1) && row.contains('×'))
        .collect();
    (rows.len() == 1).then(|| rows[0].clone())
}

fn click_tab(app: &App, label: &str) {
    let screen = app.screen();
    let y = (0..screen.rows()).find(|index| {
        let row = row_text(&screen, *index);
        row.contains(QUERY_1) && row.contains('×')
    });
    let Some(y) = y else {
        panic!("tab strip missing\n{}", text(&screen));
    };
    let row = row_text(&screen, y);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| u32::from(hit.y) == y && token_at(&row, usize::from(hit.x), label))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "tab {label:?} on {row:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn assert_tabs(screen: &TerminalSnapshot, present: &[&str], absent: &[&str], products: usize) {
    let tabs = tab_line(screen).unwrap_or_else(|| panic!("tab strip\n{}", text(screen)));
    for item in present {
        assert!(
            token_count(&tabs, item) >= 1,
            "tab strip missing {item:?}: {tabs}"
        );
    }
    for item in absent {
        assert_eq!(
            token_count(&tabs, item),
            0,
            "tab strip still has {item:?}: {tabs}"
        );
    }
    assert_eq!(
        token_count(&tabs, PRODUCTS),
        products,
        "products tabs: {tabs}"
    );
    assert!(
        !tabs.contains("public.products")
            && !tabs.contains("public.organizations")
            && !tabs.contains("public.customers"),
        "qualified duplicate tab: {tabs}"
    );
}

fn assert_object(screen: &TerminalSnapshot, name: &str, meta: &str) {
    let row = object_row(screen).unwrap_or_else(|| panic!("no object title\n{}", text(screen)));
    assert!(
        row.contains(&format!("{TITLE}{name}")),
        "title {name} missing: {row}"
    );
    assert!(row.contains(meta), "meta {meta:?} missing: {row}");
}

fn footer_toast(screen: &TerminalSnapshot) -> bool {
    let last = screen.rows().saturating_sub(1);
    row_text(screen, last).contains("✓ Connected to Development")
}

fn shows_object(screen: &TerminalSnapshot, name: &str, meta: &str) -> bool {
    object_row(screen)
        .is_some_and(|row| row.contains(&format!("{TITLE}{name}")) && row.contains(meta))
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. Below about 100 columns the header omits the size chip.
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

fn confirm_quit(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(
        &quit,
        &["Quit TablePro?", "1 unsaved query will be lost."],
        &["Loading schema", "Execute query?"],
    );
    app.key("right");
    app.key("enter");
    app.wait_exit("tablepro stayed up after confirming quit");
    app.expect_exited(0);
}

#[test]
fn route_opens_a_table_preview_and_quits() {
    let app = open("t04-preview-route");
    let landing = app.screen();
    assert_size(&landing, 120, 40);
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[QUERY_1, PLACEHOLDER, "Loading schema", PREVIEW],
    );
    connect_development(&app);
    click_tree(&app, PRODUCTS);
    let preview = app.wait_screen("products preview", |screen| {
        shows_object(screen, PRODUCTS, PRODUCTS_PREVIEW)
    });
    assert_size(&preview, 120, 40);
    assert_object(&preview, PRODUCTS, PRODUCTS_PREVIEW);
    assert_tabs(&preview, &[QUERY_1, PRODUCTS], &[QUERY_2, ORGANIZATIONS], 1);
    see(
        &preview,
        &[CONNECTED, QUERY_1, PRODUCTS_PREVIEW],
        &["Loading schema", "public.products", CUSTOMERS_PREVIEW],
    );
    app.finish();
}

#[test]
fn preview_reuses_the_unpinned_tab_only() {
    let app = open("t04-preview");
    connect_development(&app);

    click_tree(&app, PRODUCTS);
    let preview = app.wait_screen("products preview", |screen| {
        shows_object(screen, PRODUCTS, PRODUCTS_PREVIEW)
            && tab_line(screen).is_some_and(|tabs| token_count(&tabs, QUERY_1) == 1)
    });
    assert_object(&preview, PRODUCTS, PRODUCTS_PREVIEW);
    assert_tabs(&preview, &[QUERY_1, PRODUCTS], &[QUERY_2], 1);

    click_tree(&app, PRODUCTS);
    let pinned = app.wait_screen("products pinned", |screen| {
        object_row(screen).is_some_and(|row| {
            row.contains(&format!("{TITLE}{PRODUCTS}"))
                && row.contains(PRODUCTS_PINNED)
                && !row.contains(PREVIEW)
        })
    });
    assert_object(&pinned, PRODUCTS, PRODUCTS_PINNED);
    assert!(
        !object_row(&pinned).is_some_and(|row| row.contains(PREVIEW)),
        "pin left the preview meta\n{}",
        text(&pinned)
    );
    assert_tabs(&pinned, &[QUERY_1, PRODUCTS], &[QUERY_2, ORGANIZATIONS], 1);

    app.key("ctrl+t");
    let fresh = app.wait_screen("new query", |screen| {
        let visible = text(screen);
        visible.contains(QUERY_2) && visible.contains(PLACEHOLDER) && !visible.contains(MARKER)
    });
    assert_tabs(&fresh, &[QUERY_1, QUERY_2, PRODUCTS], &[ORGANIZATIONS], 1);
    app.type_text(DIRTY_SQL);
    // The strip's dirty mark is copied in `sync_strip`, which typing does not
    // call. The editor text is the dirty state (`QueryTab::dirty`).
    let dirty = app.wait_screen("dirty query text", |screen| {
        text(screen).contains(DIRTY_SQL)
    });
    assert!(
        text(&dirty).contains(DIRTY_SQL),
        "editor dropped the marker statement\n{}",
        text(&dirty)
    );
    assert_tabs(
        &dirty,
        &[QUERY_1, QUERY_2, PRODUCTS],
        &[ORGANIZATIONS, CUSTOMERS],
        1,
    );

    click_tree(&app, ORGANIZATIONS);
    let organizations = app.wait_screen("organizations preview", |screen| {
        shows_object(screen, ORGANIZATIONS, ORGANIZATIONS_PREVIEW)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, PRODUCTS) == 1 && token_count(&tabs, QUERY_2) == 1
            })
    });
    assert_object(&organizations, ORGANIZATIONS, ORGANIZATIONS_PREVIEW);
    assert_tabs(
        &organizations,
        &[QUERY_1, QUERY_2, PRODUCTS, ORGANIZATIONS],
        &[CUSTOMERS],
        1,
    );

    click_tab(&app, QUERY_2);
    let kept = app.wait_screen("dirty query still open", |screen| {
        text(screen).contains(DIRTY_SQL)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, ORGANIZATIONS) == 1 && token_count(&tabs, PRODUCTS) == 1
            })
            && object_row(screen)
                .is_none_or(|row| !row.contains(&format!("{TITLE}{ORGANIZATIONS}")))
    });
    assert!(
        text(&kept).contains(DIRTY_SQL),
        "organizations replaced the dirty query\n{}",
        text(&kept)
    );
    assert_tabs(
        &kept,
        &[QUERY_1, QUERY_2, PRODUCTS, ORGANIZATIONS],
        &[CUSTOMERS],
        1,
    );

    click_tree(&app, CUSTOMERS);
    let reused = app.wait_screen("customers reused the preview slot", |screen| {
        shows_object(screen, CUSTOMERS, CUSTOMERS_PREVIEW)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, PRODUCTS) == 1
                    && token_count(&tabs, ORGANIZATIONS) == 0
                    && token_count(&tabs, CUSTOMERS) == 1
                    && token_count(&tabs, QUERY_2) == 1
            })
    });
    assert_object(&reused, CUSTOMERS, CUSTOMERS_PREVIEW);
    assert_tabs(
        &reused,
        &[QUERY_1, QUERY_2, PRODUCTS, CUSTOMERS],
        &[ORGANIZATIONS],
        1,
    );

    click_tab(&app, QUERY_2);
    let still_dirty = app.wait_screen("dirty sql survived preview reuse", |screen| {
        text(screen).contains(DIRTY_SQL)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, CUSTOMERS) == 1
                    && token_count(&tabs, PRODUCTS) == 1
                    && token_count(&tabs, ORGANIZATIONS) == 0
            })
    });
    assert!(
        text(&still_dirty).contains(MARKER),
        "preview reuse replaced the dirty query\n{}",
        text(&still_dirty)
    );

    click_tab(&app, CUSTOMERS);
    let checkpoint = app.wait_screen("customers preview checkpoint", |screen| {
        shows_object(screen, CUSTOMERS, CUSTOMERS_PREVIEW)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, PRODUCTS) == 1
                    && token_count(&tabs, QUERY_2) == 1
                    && token_count(&tabs, ORGANIZATIONS) == 0
                    && tabs.contains('•')
            })
    });
    assert_size(&checkpoint, 120, 40);
    assert_object(&checkpoint, CUSTOMERS, CUSTOMERS_PREVIEW);
    assert_tabs(
        &checkpoint,
        &[QUERY_1, QUERY_2, PRODUCTS, CUSTOMERS],
        &[ORGANIZATIONS],
        1,
    );
    see(
        &checkpoint,
        &[CONNECTED, CUSTOMERS_PREVIEW, QUERY_2, PRODUCTS],
        &[
            "Loading schema",
            ORGANIZATIONS_PREVIEW,
            &format!("{TITLE}{ORGANIZATIONS}"),
            &format!("{TITLE}{PRODUCTS}"),
        ],
    );

    flash_settle(&app);
    let wide = app.screen();
    assert_size(&wide, 120, 40);
    assert_object(&wide, CUSTOMERS, CUSTOMERS_PREVIEW);
    assert_tabs(
        &wide,
        &[QUERY_1, QUERY_2, PRODUCTS, CUSTOMERS],
        &[ORGANIZATIONS],
        1,
    );
    // The connected toast lasts 5s. visual() would keep the cleared footer.
    app.visual_while(&visual_id("T04.preview.preview.120x40"), |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && shows_object(screen, CUSTOMERS, CUSTOMERS_PREVIEW)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, PRODUCTS) == 1
                    && token_count(&tabs, QUERY_2) == 1
                    && token_count(&tabs, ORGANIZATIONS) == 0
            })
            && footer_toast(screen)
    });

    app.resize(80, 24);
    let narrow = app.wait_screen("preview at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    assert_size(&narrow, 80, 24);
    flash_settle(&app);
    let narrow = app.screen();
    assert_size(&narrow, 80, 24);
    app.visual_while(&visual_id("T04.preview.preview.narrow"), |screen| {
        screen.cols() == 80 && screen.rows() == 24 && footer_toast(screen)
    });
    app.resize(120, 40);
    let restored = app.wait_screen("preview back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && shows_object(screen, CUSTOMERS, CUSTOMERS_PREVIEW)
            && tab_line(screen).is_some_and(|tabs| {
                token_count(&tabs, PRODUCTS) == 1
                    && token_count(&tabs, QUERY_2) == 1
                    && token_count(&tabs, ORGANIZATIONS) == 0
            })
    });
    assert_object(&restored, CUSTOMERS, CUSTOMERS_PREVIEW);
    confirm_quit(&app);
}
