//! TablePro connection-form actions, driven through the tablepro PTY.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t01_test/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, TABLEPRO, find_all, flash_settle, row_text, text, try_launch,
};

/// The form card is at most 84 columns, and the test result is drawn after
/// the buttons, so the full sentence is truncated at 120×40.
const SEEN_OK: &str = "Connected · Pos…";
const SEEN_ERR: &str = "Connection time…";
const FULL_OK: &str = "Connected · PostgreSQL 16.3 · 12 ms";
const FULL_ERR: &str = "Connection timed out after 10 s";
const SAVED: &str = "Connection saved";
const TUNNEL: &str = "Opening SSH tunnel";
const WORKBENCH: &str = "Query 1";
const FORM: &str = "Edit connection";
const BUTTONS: &[&str] = &["Test connection", "Cancel", "Save", "Save & connect"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/t01_test/{}", case_id.replace('.', "_"))
}

fn open(name: &str, motion: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    app
}

fn require(screen: &TerminalSnapshot, items: &[&str]) {
    let visible = text(screen);
    for item in items {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
}

fn forbid(screen: &TerminalSnapshot, items: &[&str]) {
    let visible = text(screen);
    for item in items {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

/// The form title can leave one frame before the name field does. Cancel is
/// done when the list actions are back and the unsaved name is gone.
fn form_closed(screen: &TerminalSnapshot, name: &str, host: &str, draft: &str) -> bool {
    let visible = text(screen);
    visible.contains(name)
        && visible.contains(host)
        && visible.contains("Connect")
        && !visible.contains(FORM)
        && !visible.contains(draft)
        && !visible.contains("Test connection")
}

fn results_and_workbench() -> &'static [&'static str] {
    &[
        FULL_OK, FULL_ERR, SEEN_OK, SEEN_ERR, SAVED, TUNNEL, WORKBENCH,
    ]
}

fn cell_tail(screen: &TerminalSnapshot, point: Point) -> String {
    let mut tail = String::new();
    for (column, cell) in screen
        .row(u32::from(point.y))
        .expect("row")
        .iter()
        .enumerate()
    {
        if column >= usize::from(point.x) {
            tail.push_str(cell.text());
        }
    }
    tail
}

fn click_list(app: &App, name: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, name)
        .into_iter()
        .filter(|point| point.x < 40)
        .collect();
    assert_eq!(hits.len(), 1, "list {name}\n{}", text(&screen));
    app.click(hits[0]);
}

fn click_action(app: &App, label: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let line = row_text(&screen, u32::from(point.y));
            line.contains("Test connection") && line.contains("Cancel")
        })
        .filter(|point| {
            let tail = cell_tail(&screen, *point);
            if label == "Save" {
                tail.starts_with("Save") && !tail.starts_with("Save &")
            } else {
                tail.starts_with(label)
            }
        })
        .collect();
    assert_eq!(hits.len(), 1, "action {label}\n{}", text(&screen));
    app.click(hits[0]);
}

fn open_form(app: &App, name: &str, host: &str) {
    click_list(app, name);
    app.wait_text(host);
    app.key("e");
    app.wait_text(FORM);
    app.wait_text("Save & connect");
}

fn rename(app: &App, name: &str) {
    app.type_text(name);
    app.wait_text(name);
}

fn assert_size(app: &App) {
    let screen = app.screen();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (120, 40),
        "terminal size\n{}",
        text(&screen)
    );
}

fn shot(app: &App, case_id: &str) {
    assert_size(app);
    app.visual(&visual_id(case_id));
}

fn shot_narrow(app: &App) {
    let screen = app.screen();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (80, 24),
        "narrow terminal\n{}",
        text(&screen)
    );
    app.visual(&visual_id("t01_test.actions.narrow"));
}

#[test]
fn route_opens_the_form_and_cancel_returns() {
    let app = open("t01-route", "paused");
    open_form(&app, "Development", "dev-db.internal.acme.io");
    let entered = app.settle();
    require(
        &entered,
        &[
            FORM,
            "Development",
            "dev-db.internal.acme.io",
            "Test connection",
            "Cancel",
            "Save",
            "Save & connect",
        ],
    );
    forbid(&entered, results_and_workbench());
    flash_settle(&app);
    shot(&app, "t01_test.route");
    click_action(&app, "Cancel");
    let returned = app.wait_absent(FORM);
    require(
        &returned,
        &["Development", "dev-db.internal.acme.io", "Connect"],
    );
    forbid(&returned, results_and_workbench());
    assert!(!text(&returned).contains(FORM), "{}", text(&returned));
    app.finish();
}

#[test]
fn actions_are_separate_results() {
    testing_paused();
    testing_success();
    testing_timeout();
    saving();
    saving_and_connecting_paused();
    cancelling();
    saving_and_connecting_reduced();
}

fn testing_paused() {
    let app = open("t01-test-paused", "paused");
    open_form(&app, "Development", "dev-db.internal.acme.io");
    rename(&app, "Devtest");
    click_action(&app, "Test connection");
    app.wait_text("⠋");
    flash_settle(&app);
    let busy = app.settle();
    require(
        &busy,
        &[
            FORM,
            "Devtest",
            "Development",
            "dev-db.internal.acme.io",
            "⠋",
            "Test connection",
            "Cancel",
            "Save",
            "Save & connect",
        ],
    );
    forbid(&busy, results_and_workbench());
    shot(&app, "t01_test.actions.120x40");
    app.resize(80, 24);
    // Page width is below collapse_below(80), so the list is not on this frame.
    let narrow = app.wait_screen("actions narrow", |screen| {
        if screen.cols() != 80 || screen.rows() != 24 {
            return false;
        }
        let visible = text(screen);
        visible.contains(FORM)
            && visible.contains("Devtest")
            && visible.contains("⠋")
            && visible.contains("Test connection")
    });
    require(&narrow, &[FORM, "Devtest", "⠋", "Test connection"]);
    forbid(
        &narrow,
        &[FULL_OK, FULL_ERR, SAVED, WORKBENCH, "Development"],
    );
    shot_narrow(&app);
    app.resize(120, 40);
    app.wait_screen("actions restored", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(FORM)
            && text(screen).contains("Cancel")
            && text(screen).contains("120×40")
    });
    click_action(&app, "Cancel");
    let returned = app.wait_screen("devtest discarded", |screen| {
        form_closed(screen, "Development", "dev-db.internal.acme.io", "Devtest")
    });
    require(&returned, &["Development", "dev-db.internal.acme.io"]);
    forbid(&returned, &["Devtest"]);
    forbid(&returned, results_and_workbench());
    app.finish();
}

fn testing_success() {
    let app = open("t01-test-ok", "reduced");
    open_form(&app, "Production", "prod-db-1.acme.io");
    click_action(&app, "Test connection");
    let tested = app.wait_text(SEEN_OK);
    require(
        &tested,
        &[FORM, "Production", "prod-db-1.acme.io", SEEN_OK, "✓"],
    );
    forbid(
        &tested,
        &[FULL_OK, FULL_ERR, SEEN_ERR, SAVED, TUNNEL, WORKBENCH],
    );
    click_action(&app, "Cancel");
    let returned = app.wait_absent(FORM);
    require(&returned, &["Production", "prod-db-1.acme.io"]);
    forbid(&returned, &[SEEN_OK, SAVED, WORKBENCH]);
    app.finish();
}

fn testing_timeout() {
    let app = open("t01-test-timeout", "reduced");
    open_form(&app, "Analytics", "analytics.acme.io");
    click_action(&app, "Test connection");
    let tested = app.wait_text(SEEN_ERR);
    require(
        &tested,
        &[FORM, "Analytics", "analytics.acme.io", SEEN_ERR, "✗"],
    );
    forbid(
        &tested,
        &[FULL_OK, FULL_ERR, SEEN_OK, SAVED, TUNNEL, WORKBENCH],
    );
    click_action(&app, "Cancel");
    let returned = app.wait_absent(FORM);
    require(&returned, &["Analytics", "analytics.acme.io"]);
    forbid(&returned, &[SEEN_ERR, SAVED, WORKBENCH]);
    app.finish();
}

fn saving() {
    let app = open("t01-save", "paused");
    open_form(&app, "Development", "dev-db.internal.acme.io");
    rename(&app, "Devnote");
    click_action(&app, "Save");
    let saved = app.wait_text(SAVED);
    require(
        &saved,
        &[SAVED, "Devnote", "dev-db.internal.acme.io", "Connect"],
    );
    forbid(
        &saved,
        &[
            FORM,
            "Development",
            FULL_OK,
            FULL_ERR,
            SEEN_OK,
            SEEN_ERR,
            TUNNEL,
            WORKBENCH,
        ],
    );
    app.key("e");
    let reopened = app.wait_text(FORM);
    require(
        &reopened,
        &[FORM, "Devnote", "dev-db.internal.acme.io", SAVED],
    );
    forbid(&reopened, &[WORKBENCH, SEEN_OK, SEEN_ERR, TUNNEL]);
    for label in BUTTONS {
        assert!(text(&reopened).contains(label), "missing {label}");
    }
    app.finish();
}

fn saving_and_connecting_paused() {
    let app = open("t01-save-connect-paused", "paused");
    open_form(&app, "Production", "prod-db-1.acme.io");
    rename(&app, "Prodlink");
    click_action(&app, "Save & connect");
    let connecting = app.wait_text(TUNNEL);
    require(
        &connecting,
        &[SAVED, TUNNEL, "Prodlink", "prod-db-1.acme.io", "⠋"],
    );
    forbid(
        &connecting,
        &[
            FORM,
            WORKBENCH,
            FULL_OK,
            FULL_ERR,
            SEEN_OK,
            SEEN_ERR,
            "Production",
        ],
    );
    app.finish();
}

fn cancelling() {
    let app = open("t01-cancel", "paused");
    open_form(&app, "Staging", "staging-db.acme.io");
    rename(&app, "Nope");
    click_action(&app, "Cancel");
    let returned = app.wait_screen("nope discarded", |screen| {
        form_closed(screen, "Staging", "staging-db.acme.io", "Nope")
    });
    require(
        &returned,
        &["Staging", "staging-db.acme.io", "Connect", "Edit"],
    );
    forbid(&returned, &["Nope"]);
    forbid(&returned, results_and_workbench());
    app.finish();
}

fn saving_and_connecting_reduced() {
    let app = open("t01-save-connect", "reduced");
    open_form(&app, "Local PostgreSQL", "localhost:5432");
    rename(&app, "Locallink");
    click_action(&app, "Save & connect");
    // Query 1 paints while the footer still says "Connection saved".
    let workbench = app.wait_screen("locallink connected", |screen| {
        let visible = text(screen);
        visible.contains(WORKBENCH)
            && visible.contains("Connected to Locallink")
            && !visible.contains(FORM)
            && !visible.contains(TUNNEL)
    });
    require(
        &workbench,
        &[WORKBENCH, "Locallink", "Connected to Locallink"],
    );
    forbid(
        &workbench,
        &[FORM, TUNNEL, SEEN_OK, SEEN_ERR, FULL_OK, FULL_ERR],
    );
    app.finish();
}
