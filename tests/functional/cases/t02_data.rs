//! TablePro data-grid object kinds, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_data/` and each checkpoint's dots become underscores.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const ORGANIZATIONS: &str = "organizations";
const VIEW: &str = "active_customers";
const NO_KEY_TABLE: &str = "daily_revenue";
const EDITED: &str = "KindEdit";
const SLUG: &str = "tessellate";
const ORG_ID: &str = "be6dd084-a1f4-4d47-ae53-915ac89debab";
const VIEW_REASON: &str = "These rows come from a view, which cannot be edited.";
const NO_KEY_REASON: &str = "This table has no primary key, so rows cannot be identified.";
const WIDE_ROWS: &str = "rows 1–27 of 500 loaded";
const NARROW_ROWS: &str = "rows 1–11 of 500 loaded";
const NO_KEY_TOTAL: &str = "1,460 total";
const VIEW_TOTAL: &str = "41,207 total";
const PENDING_EDIT: &str = "1 pending · 1 update";
const LOST: &str = "1 pending row change will be lost.";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t02_data/{}",
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
    // The Query 1 tab is drawn during "Loading schema…", before the
    // connected status replaces that phase.
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
    app.wait_text(name);
    app.key("enter");
    app.wait_screen(&format!("opened {name}"), |screen| {
        let visible = text(screen);
        visible.contains(name) && !visible.contains("Open Quickly")
    });
}

fn row_has(screen: &TerminalSnapshot, parts: &[&str]) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        parts.iter().all(|part| line.contains(part))
    })
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn commit_name(app: &App) {
    app.key("right");
    app.key("enter");
    app.wait_text("Apply");
    // macOS field keys are Emacs, so Select All in the open cell editor is Ctrl+L.
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text(EDITED);
    app.key("enter");
    let committed = app.wait_screen("organizations name committed", |screen| {
        text(screen).contains(PENDING_EDIT) && row_has(screen, &[ORG_ID, EDITED, SLUG])
    });
    see(
        &committed,
        &[
            EDITED,
            SLUG,
            ORG_ID,
            PENDING_EDIT,
            "Preview SQL",
            "Enter Edit",
            "public › organizations",
        ],
        &["Saved", "Apply"],
    );
}

fn reject_view(app: &App) {
    open_object(app, VIEW);
    let opened = app.wait_text("public › active_customers");
    see(
        &opened,
        &[
            VIEW_TOTAL,
            "organizations •",
            "Opened public.active_customers",
        ],
        &[
            EDITED,
            PENDING_EDIT,
            "Preview SQL",
            "Enter Edit",
            "Saved",
            "active_customers •",
            VIEW_REASON,
        ],
    );
    app.key("enter");
    let rejected = app.wait_text(VIEW_REASON);
    // The rows label outranks `read-only: {reason}`, so the footer keeps the
    // counts and the rejection sentence is the status line.
    see(
        &rejected,
        &[
            VIEW_REASON,
            "public › active_customers",
            "rows 1–26 of 500 loaded",
            VIEW_TOTAL,
            "organizations •",
        ],
        &[
            EDITED,
            PENDING_EDIT,
            "Preview SQL",
            "Enter Edit",
            "Saved",
            "read-only:",
            "active_customers •",
        ],
    );
}

fn open_keyless(app: &App) -> TerminalSnapshot {
    open_object(app, NO_KEY_TABLE);
    app.wait_text("analytics › daily_revenue");
    app.key("enter");
    app.wait_text(NO_KEY_REASON)
}

fn assert_keyless(screen: &TerminalSnapshot, rows_label: &str) {
    see(
        screen,
        &[
            NO_KEY_REASON,
            "analytics › daily_revenue",
            rows_label,
            NO_KEY_TOTAL,
            "organizations •",
            "day",
            "orders",
            "revenue",
        ],
        &[
            EDITED,
            PENDING_EDIT,
            "Preview SQL",
            "Enter Edit",
            "Saved",
            "read-only:",
            "active_customers •",
        ],
    );
}

fn confirm_quit(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(&quit, &[LOST], &[]);
    app.key("right");
    app.key("enter");
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "tablepro stayed up after confirming quit"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

#[test]
fn tablepro_data_route_opens_an_editable_table_and_returns() {
    let app = open("t02-data-route");
    connect_development(&app);
    open_object(&app, ORGANIZATIONS);
    let table = app.wait_text("public › organizations");
    see(
        &table,
        &["Enter Edit", "public › organizations", "name", "slug"],
        &["Saved", VIEW_REASON, NO_KEY_REASON],
    );
    app.key("ctrl+w");
    let back = app.wait_screen("query tab after closing the table", |screen| {
        let visible = text(screen);
        visible.contains("Type SQL") && !visible.contains("public › organizations")
    });
    see(
        &back,
        &[QUERY, "Type SQL"],
        &["public › organizations", "T organizations ×"],
    );
    app.finish();
}

#[test]
fn tablepro_data_kinds_edit_a_table_and_reject_readonly_objects() {
    let app = open("t02-data-kinds");
    connect_development(&app);
    open_object(&app, ORGANIZATIONS);
    app.wait_text("public › organizations");
    commit_name(&app);
    reject_view(&app);
    let wide = open_keyless(&app);
    assert_keyless(&wide, WIDE_ROWS);
    app.key("enter");
    flash_settle(&app);
    shot(&app, "T02.data.kinds.120x40");
    app.resize(80, 24);
    app.wait_screen("80x24", |screen| screen.cols() == 80 && screen.rows() == 24);
    app.key("enter");
    let narrow = app.wait_text(NO_KEY_REASON);
    assert_keyless(&narrow, NARROW_ROWS);
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    flash_settle(&app);
    shot(&app, "T02.data.kinds.narrow");
    app.resize(120, 40);
    app.wait_text("120×40");
    confirm_quit(&app);
}
