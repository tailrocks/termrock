//! TablePro unsaved-leave guards, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t01_leave/` and the dotted checkpoint ids use underscores.

use std::path::PathBuf;

use tui_test::TerminalSnapshot;

use crate::bins::tablepro;
use crate::support::{App, Launch, find_all, flash_settle, row_text, text, try_launch};

const ROUTE_ID: &str = "pre_refactor/t01_leave/t01_leave_route_120x40";
const WIDE_ID: &str = "pre_refactor/t01_leave/t01_leave_guards_120x40";
const NARROW_ID: &str = "pre_refactor/t01_leave/t01_leave_guards_narrow";

const PLACEHOLDER: &str = "Filter connections";
const PROD: &str = "Production";
const ORIGINAL: &str = "prod-db-1.acme.io";
const DRAFT: &str = "prod-db-2.acme.io";
const PROD_DB: &str = "acme_prod";
const TITLE: &str = "Unsaved changes";
const ASK: &str = "Save before leaving?";
const WOULD: &str = "Unsaved changes would be";
const LOST_LINE: &str = "lost.";
const SAVED: &str = "Connection saved";
const FIX: &str = "Fix the highlighted fields";
const REQUIRED: &str = "Required";
const EDIT: &str = "Edit connection";
const NEW: &str = "New connection";
const SPLICE: &str = "prod-db-2.acme.ioprod";

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, tablepro());
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

fn in_list(screen: &TerminalSnapshot, needle: &str) -> bool {
    find_all(screen, needle).into_iter().any(|hit| hit.x <= 35)
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

fn form_hit(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| hit.x >= 40)
}

/// The dialog body wraps inside the 54-column frame, so `would be lost.` is
/// two screen lines. Match the drawn pieces, not one contiguous sentence.
fn dialog_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let lost = visible.lines().any(|line| line.contains(LOST_LINE));
    visible.contains(TITLE)
        && visible.contains(ASK)
        && visible.contains(WOULD)
        && lost
        && visible.contains("Discard")
        && visible.contains("Cancel")
        && visible.contains("Save")
}

fn draft_replaced(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DRAFT)
        && !visible.contains(ORIGINAL)
        && !visible.contains(SPLICE)
        && visible.contains(EDIT)
        && form_hit(screen, PROD)
}

fn click_dialog(app: &App, label: &str) {
    let screen = app.wait_text(TITLE);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| row_text(&screen, u32::from(hit.y)).contains("Discard"))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "dialog {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    flash_settle(app);
}

fn shot(app: &App, id: &str, precondition: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let outcome = app.check_visual_with_precondition(&root, id, precondition);
    assert!(outcome.is_pass(), "visual mismatch for {id}: {outcome:?}");
}

fn open_production(app: &App) {
    click_listed(app, PROD);
    flash_settle(app);
    let detail = app.wait_text(ORIGINAL);
    see(
        &detail,
        &[PROD, ORIGINAL, PROD_DB, "Connect"],
        &[EDIT, DRAFT, TITLE, SAVED],
    );
    app.key("e");
    let form = app.wait_text(EDIT);
    see(
        &form,
        &[EDIT, PROD, ORIGINAL, PROD_DB],
        &[TITLE, NEW, SAVED],
    );
}

fn replace_host(app: &App) {
    // The name field opens select-all. Tab commits it and lands on Engine,
    // and the next Tab lands on Host, whose focus render selects the value.
    // A click on the value places the caret and would insert in front of it.
    app.key("tab");
    app.key("tab");
    app.settle();
    app.type_text(DRAFT);
    app.key("enter");
    let dirty = app.wait_screen("host draft committed", draft_replaced);
    see(
        &dirty,
        &[EDIT, DRAFT, PROD],
        &[ORIGINAL, SPLICE, TITLE, SAVED, NEW],
    );
}

fn leave_dialog(app: &App, path: &str) -> TerminalSnapshot {
    let screen = app.wait_screen(&format!("{path} dialog"), dialog_open);
    let visible = text(&screen);
    assert!(
        visible.contains(DRAFT) && !visible.contains(SPLICE),
        "{path} dropped the draft before the choice\n{visible}"
    );
    assert!(
        visible.contains(EDIT),
        "{path} left the edit form before the choice\n{visible}"
    );
    assert!(
        !visible.contains(NEW),
        "{path} replaced the form before the choice\n{visible}"
    );
    assert!(
        !visible.contains(SAVED),
        "{path} saved before the choice\n{visible}"
    );
    assert!(app.child_running(), "{path} quit before the choice");
    assert!(
        in_list(&screen, PROD),
        "{path} lost Production from the list\n{visible}"
    );
    screen
}

fn stay(app: &App) {
    app.key("escape");
    let kept = app.wait_screen("stay keeps the draft", |screen| {
        let visible = text(screen);
        !visible.contains(TITLE)
            && !visible.contains(ASK)
            && visible.contains(DRAFT)
            && visible.contains(EDIT)
            && !visible.contains(SAVED)
    });
    see(&kept, &[EDIT, DRAFT, PROD], &[TITLE, SAVED, NEW, ORIGINAL]);
    assert!(app.child_running(), "stay quit the process");
}

fn resize_until(app: &App, cols: u16, rows: u16) {
    app.resize(cols, rows);
    app.wait_screen(&format!("size {cols}x{rows}"), |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && dialog_open(screen)
    });
}

#[test]
fn tablepro_leave_route_opens_connections_without_a_page_flag() {
    let app = open("t01-leave-route");
    let screen = app.settle();
    assert_size(&screen, 120, 40);
    see(
        &screen,
        &[
            "TablePro",
            "Connections",
            "6 saved",
            "truecolor · 120×40",
            PLACEHOLDER,
            "Local PostgreSQL",
            PROD,
            "Connect",
        ],
        &[EDIT, TITLE, SAVED, "Query 1", NEW],
    );
    shot(
        &app,
        ROUTE_ID,
        "connections screen at 120 by 40 with no page flag",
    );
    app.finish();
}

#[test]
fn tablepro_leave_guards_keep_the_draft_until_a_choice() {
    let app = open("t01-leave-guards");
    open_production(&app);
    replace_host(&app);

    app.key("escape");
    let close = leave_dialog(&app, "close");
    assert_size(&close, 120, 40);
    shot(
        &app,
        WIDE_ID,
        "unsaved dialog from Close at 120 by 40, before Stay",
    );
    resize_until(&app, 80, 24);
    let narrow = app.screen();
    assert_size(&narrow, 80, 24);
    assert!(dialog_open(&narrow), "narrow dialog\n{}", text(&narrow));
    see(&narrow, &[TITLE, ASK, WOULD, EDIT], &[SAVED, NEW, SPLICE]);
    shot(
        &app,
        NARROW_ID,
        "unsaved dialog from Close at 80 by 24, before the terminal grows",
    );
    resize_until(&app, 120, 40);
    let restored = app.screen();
    assert_size(&restored, 120, 40);
    see(&restored, &[TITLE, DRAFT, EDIT], &[SAVED, NEW]);
    stay(&app);

    app.key("escape");
    leave_dialog(&app, "close-discard");
    click_dialog(&app, "Discard");
    let discarded = app.wait_screen("discard restored the original", |screen| {
        let visible = text(screen);
        visible.contains(ORIGINAL)
            && visible.contains(PROD_DB)
            && !visible.contains(DRAFT)
            && !visible.contains(EDIT)
            && !visible.contains(TITLE)
            && !visible.contains(SAVED)
    });
    see(
        &discarded,
        &[PROD, ORIGINAL, PROD_DB, "Connect", PLACEHOLDER],
        &[DRAFT, EDIT, TITLE, SAVED, FIX],
    );

    open_production(&app);
    replace_host(&app);
    app.key("ctrl+n");
    leave_dialog(&app, "new");
    stay(&app);

    click_listed(&app, "Analytics");
    flash_settle(&app);
    let selected = app.wait_screen("other row selected under the open form", |screen| {
        text(screen).contains(DRAFT)
            && text(screen).contains(EDIT)
            && in_list(screen, "Analytics")
            && !text(screen).contains(TITLE)
            && !text(screen).contains("analytics.acme.io")
    });
    see(
        &selected,
        &[DRAFT, EDIT, PROD],
        &[TITLE, SAVED, "analytics.acme.io"],
    );
    app.key("e");
    leave_dialog(&app, "edit-another");
    assert!(
        !text(&app.screen()).contains("analytics.acme.io"),
        "edit another switched forms before the choice\n{}",
        text(&app.screen())
    );
    stay(&app);
    assert!(
        !text(&app.screen()).contains("analytics.acme.io"),
        "stay after edit another opened Analytics\n{}",
        text(&app.screen())
    );

    app.key("ctrl+q");
    leave_dialog(&app, "quit");
    assert!(app.child_running(), "quit dialog already exited");
    stay(&app);
    assert!(app.child_running(), "stay on quit exited");

    // Quit Stay leaves the tree focused. Tab reaches the tab strip, then the
    // name field, which selects its text. Backspace clears that selection.
    app.key("tab");
    app.key("tab");
    app.settle();
    app.key("backspace");
    app.key("enter");
    let cleared = app.wait_screen("empty name stays a draft", |screen| {
        text(screen).contains(DRAFT)
            && text(screen).contains(EDIT)
            && !text(screen).contains(TITLE)
            && !form_hit(screen, PROD)
    });
    see(&cleared, &[DRAFT, EDIT], &[TITLE, SAVED]);
    app.key("escape");
    leave_dialog(&app, "invalid-save");
    click_dialog(&app, "Save");
    let rejected = app.wait_text(FIX);
    see(
        &rejected,
        &[FIX, EDIT, DRAFT, REQUIRED],
        &[SAVED, TITLE, NEW],
    );
    assert!(
        in_list(&rejected, PROD),
        "invalid save renamed the stored connection\n{}",
        text(&rejected)
    );
    assert!(app.child_running(), "invalid save quit");

    app.type_text(PROD);
    app.wait_screen("name typed back", |screen| {
        find_all(screen, PROD).iter().any(|hit| hit.x >= 40) && text(screen).contains(DRAFT)
    });
    app.key("enter");
    app.key("escape");
    leave_dialog(&app, "valid-save");
    click_dialog(&app, "Save");
    let saved = app.wait_screen("valid save stores the host", |screen| {
        let visible = text(screen);
        visible.contains(SAVED)
            && visible.contains(DRAFT)
            && !visible.contains(EDIT)
            && !visible.contains(TITLE)
            && !visible.contains(ORIGINAL)
    });
    see(
        &saved,
        &[SAVED, PROD, DRAFT, PROD_DB, "Connect"],
        &[EDIT, TITLE, FIX, ORIGINAL],
    );

    app.key("e");
    let reopened = app.wait_text(EDIT);
    see(
        &reopened,
        &[EDIT, PROD, DRAFT, PROD_DB],
        &[ORIGINAL, TITLE, FIX],
    );
    app.key("escape");
    let closed = app.wait_screen("clean close leaves the stored host", |screen| {
        text(screen).contains(DRAFT)
            && text(screen).contains("Connect")
            && !text(screen).contains(EDIT)
            && !text(screen).contains(TITLE)
    });
    see(
        &closed,
        &[PROD, DRAFT, PROD_DB, PLACEHOLDER],
        &[ORIGINAL, EDIT, TITLE, FIX],
    );
    app.finish();
}
