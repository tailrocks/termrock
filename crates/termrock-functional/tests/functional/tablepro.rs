use super::support::{App, Point, TABLEPRO, crop, find, find_all, row_text, text};

fn inside(point: Point) -> Point {
    Point {
        x: point.x + 2,
        ..point
    }
}

fn dialog_action(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let actions: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| row_text(&screen, u32::from(point.y)).contains("Cancel"))
        .collect();
    assert_eq!(
        actions.len(),
        1,
        "one dialog action {label}:\n{}",
        text(&screen)
    );
    inside(actions[0])
}

#[test]
fn tablepro_write_confirmation_cancels_retries_and_executes_only_when_armed() {
    let app = App::new(
        "tablepro-write-gate",
        TABLEPRO,
        &["--connect", "Production", "--motion", "reduced"],
        160,
        50,
        "truecolor",
    );
    app.wait_text("Query 1");
    app.key("tab");
    app.paste("UPDATE orders SET status = 'paid' WHERE id = 'x'");
    app.wait_text("WHERE id = 'x'");
    app.key("escape");
    app.key("ctrl+r");
    let gate = app.wait_text("Type orders to confirm");
    assert!(!text(&gate).contains("rows affected"));
    let outside = find(&gate, "Explorer");
    app.click(outside);
    let refused = app.settle();
    assert!(text(&refused).contains("Type orders to confirm"));
    assert!(!text(&refused).contains("rows affected"));
    app.key("escape");
    let cancelled = app.wait_text("Cancelled · nothing was executed");
    assert!(!text(&cancelled).contains("Type orders to confirm"));
    assert!(!text(&cancelled).contains("rows affected"));

    app.key("ctrl+r");
    app.wait_text("Type orders to confirm");
    app.type_text("wrong");
    app.wait_text("wrong");
    let execute = dialog_action(&app, "Execute");
    let disabled = crop(&app.screen(), execute, 5, 1);
    app.move_to(execute);
    assert_eq!(crop(&app.settle(), execute, 5, 1), disabled);
    app.down(execute);
    let held = app.settle();
    assert_eq!(crop(&held, execute, 5, 1), disabled);
    assert!(!text(&held).contains("rows affected"));
    app.up(execute);
    app.move_to(outside);

    app.settle();

    app.key("ctrl+l");
    app.type_text("orders");
    let enabled = app.wait_screen("confirmation token enables Execute", |screen| {
        crop(screen, execute, 5, 1) != disabled && text(screen).contains("Type orders to confirm")
    });
    let rest = crop(&enabled, execute, 5, 1);
    app.move_to(execute);
    let hover = app.wait_screen("enabled Execute responds to hover", |screen| {
        crop(screen, execute, 5, 1) != rest
    });
    let hover = crop(&hover, execute, 5, 1);
    app.down(execute);
    let armed = app.wait_screen("armed Execute visibly responds to held press", |screen| {
        let cells = crop(screen, execute, 5, 1);
        cells != hover && cells != rest
    });
    assert!(!text(&armed).contains("rows affected"));
    app.up(execute);
    let executed = app.wait_text("rows affected");
    assert!(!text(&executed).contains("Type orders to confirm"));

    app.key("escape");
    let navigation = app.wait_text("Ctrl+X Explain");
    assert!(text(&navigation).contains("UPDATE orders SET status = 'paid' WHERE id = 'x'"));
    app.key("ctrl+w");
    app.wait_text("Close tab with unsaved work?");
    app.click_text("Close anyway");
    app.wait_text("No open tabs");

    app.expire_status();
    app.wait_screen("status flash faded before freeze", |s| {
        let footer = row_text(s, 49);
        !footer.contains("Cancelled")
            && !footer.contains("Executed")
            && !footer.contains("executed")
    });
    app.visual("functional/tablepro_write_gate");
    app.finish();
}

#[test]
fn tablepro_connection_failure_retries_then_another_connection_succeeds() {
    let app = App::new(
        "tablepro-connection-recovery",
        TABLEPRO,
        &["--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Filter connections");
    app.click_text("Staging");
    app.wait_text("staging-db.acme.io");
    let connect = app.point("Connect    Edit");
    let rest = crop(&app.screen(), connect, 7, 1);
    app.move_to(inside(connect));
    let hover = app.wait_screen("Connect hover changes its own cells", |screen| {
        crop(screen, connect, 7, 1) != rest
    });
    let hover = crop(&hover, connect, 7, 1);
    app.down(inside(connect));
    let held = app.wait_screen("Connect held state differs from hover", |screen| {
        let cells = crop(screen, connect, 7, 1);
        cells != hover && cells != rest
    });
    assert!(!text(&held).contains("Opening SSH tunnel"));
    app.up(inside(connect));
    app.wait_text("Authentication failed");
    app.click_text("Reconnect");
    app.wait_text("Opening SSH tunnel");
    app.wait_text("Authentication failed");
    app.click_text("Local PostgreSQL");
    app.wait_text("localhost:5432");
    app.click_text("Connect    Edit");
    let connected = app.wait_text("Connected to Local PostgreSQL");
    assert!(text(&connected).contains("Query 1"));
    assert!(!text(&connected).contains("Filter connections"));
    assert!(!text(&connected).contains("Authentication failed"));
    freeze(&app, "functional/tablepro_connection_recovery");
    app.finish();
}

const CLOSE_PROMPT: &str = "Close tab with unsaved work?";
const TARGET_SQL: &str = "SELECT 'unsaved_picker_regression';";
const OTHER_SQL: &str = "SELECT 'other_query_survives';";

fn tablepro_connected(name: &str) -> App {
    let app = App::new(
        name,
        TABLEPRO,
        &["--connect", "Local PostgreSQL", "--motion", "reduced"],
        120,
        40,
        "none",
    );
    app.wait_text("Query 1");
    app
}

fn new_query(app: &App, name: &str, sql: Option<&str>) {
    app.key("ctrl+t");
    app.wait_text(name);
    if let Some(sql) = sql {
        app.paste(sql);
        app.wait_text(sql);
    }
    app.key("f6");
}

fn open_tabs(app: &App, filter: Option<&str>) {
    app.key("ctrl+g");
    app.wait_screen("Open tabs picker", |s| {
        let t = text(s);
        t.contains("Open tabs") && t.contains("Delete Close tab")
    });
    if let Some(filter) = filter {
        app.paste(filter);
        app.wait_text(filter);
    }
}

fn delete_requires_confirmation(app: &App) {
    app.key("delete");
    app.wait_text(CLOSE_PROMPT);
}

fn cancel_close(app: &App, active_sql: &str) {
    app.key("escape");
    app.wait_screen("Cancel preserves the active query", |s| {
        let t = text(s);
        !t.contains(CLOSE_PROMPT) && !t.contains("Open tabs") && t.contains(active_sql)
    });
}

fn confirm_close(app: &App, removed: &str, retained: &str) {
    app.click_text("Close anyway");
    app.wait_screen("only the confirmed tab is closed", |s| {
        let t = text(s);
        !t.contains(CLOSE_PROMPT)
            && !t.contains("Open tabs")
            && !t.contains(removed)
            && t.contains(retained)
    });
}

#[test]
fn tablepro_dirty_picker_close_matches_ctrl_w_and_cancel_preserves_sql() {
    let app = tablepro_connected("tablepro-dirty-picker");
    new_query(&app, "Query 2", Some(TARGET_SQL));

    app.key("ctrl+w");
    app.wait_text(CLOSE_PROMPT);
    cancel_close(&app, TARGET_SQL);

    open_tabs(&app, None);
    delete_requires_confirmation(&app);
    cancel_close(&app, TARGET_SQL);

    open_tabs(&app, None);
    delete_requires_confirmation(&app);
    confirm_close(&app, "Query 2", "Query 1");
    freeze(&app, "functional/tablepro_dirty_picker");
    app.finish();
}

#[test]
fn tablepro_filtered_picker_closes_requested_dirty_tab_and_preserves_active() {
    let app = tablepro_connected("tablepro-filtered-picker");
    new_query(&app, "Query 2", Some(TARGET_SQL));
    new_query(&app, "Query 3", Some(OTHER_SQL));

    open_tabs(&app, Some("query 2"));
    delete_requires_confirmation(&app);
    cancel_close(&app, OTHER_SQL);

    open_tabs(&app, Some("query 2"));
    app.key("enter");
    app.wait_screen("cancelled target retains its SQL", |s| {
        let t = text(s);
        !t.contains("Open tabs") && t.contains(TARGET_SQL)
    });
    open_tabs(&app, Some("query 3"));
    app.key("enter");
    app.wait_screen("returning to the other dirty query", |s| {
        let t = text(s);
        !t.contains("Open tabs") && t.contains(OTHER_SQL)
    });
    app.key("f6");

    open_tabs(&app, Some("query 2"));
    delete_requires_confirmation(&app);
    confirm_close(&app, "Query 2", OTHER_SQL);
    let screen = app.screen();
    let t = text(&screen);
    assert!(
        t.contains("Query 1") && t.contains("Query 3"),
        "filtered close removed the wrong tab:\n{t}"
    );

    open_tabs(&app, None);
    delete_requires_confirmation(&app);
    confirm_close(&app, "Query 3", "Query 1");
    freeze(&app, "functional/tablepro_filtered_picker");
    app.finish();
}

#[test]
fn tablepro_clean_picker_close_is_immediate_and_picker_stays_usable() {
    let app = tablepro_connected("tablepro-clean-picker");
    new_query(&app, "Query 2", None);
    open_tabs(&app, None);
    app.key("delete");
    let closed = app.wait_screen("clean tab closed in the picker", |s| {
        let t = text(s);
        t.contains("Open tabs") && !t.contains("Query 2") && t.contains("Query 1")
    });
    assert!(
        !text(&closed).contains(CLOSE_PROMPT),
        "clean tab must close without an unsaved-work prompt"
    );
    app.key("enter");
    app.wait_screen("remaining tab can still be activated", |s| {
        let t = text(s);
        !t.contains("Open tabs") && t.contains("Query 1")
    });
    freeze(&app, "functional/tablepro_clean_picker");
    app.finish();
}

fn action_btn(app: &App, label: &str) -> Point {
    app.wait_point(
        &format!("action {label} on the detail action row"),
        |screen| {
            find_all(screen, label)
                .into_iter()
                .find(|pt| row_text(screen, u32::from(pt.y)).contains("Duplicate"))
        },
    )
}

fn list_row(app: &App, label: &str) -> Point {
    app.wait_point(&format!("list row {label}"), |screen| {
        find_all(screen, label).into_iter().find(|pt| pt.x < 40)
    })
}

fn close_all_tabs(app: &App) {
    app.key("escape");
    app.wait_text("Ctrl+X Explain");
    for _ in 0..4 {
        if text(&app.screen()).contains("No open tabs") {
            return;
        }
        app.key("ctrl+w");
        let s = app.wait_screen("close guard or no open tabs", |s| {
            text(s).contains("Close tab with unsaved work?") || text(s).contains("No open tabs")
        });
        if text(&s).contains("Close tab with unsaved work?") {
            app.key("right");
            app.key("enter");
        }
    }
    app.wait_text("No open tabs");
}

fn freeze(app: &App, test_id: &str) {
    app.expire_status();
    app.wait_absent("\u{2713} Connected to");
    app.visual(test_id);
}

fn connected(name: &str) -> App {
    let app = App::new(
        name,
        TABLEPRO,
        &["--connect", "Local PostgreSQL", "--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Query 1");
    app
}

#[test]
fn tablepro_connections_landing_and_unreachable_detail() {
    let app = App::new(
        "tablepro-landing",
        TABLEPRO,
        &["--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Filter connections");
    app.click(list_row(&app, "Analytics"));
    app.wait_screen("analytics detail shows warehouse host", |s| {
        text(s).contains("analytics.acme.io") || text(s).contains("warehouse")
    });
    freeze(&app, "functional/tablepro_connections_landing");
    app.finish();

    let app = App::new(
        "tablepro-unreachable",
        TABLEPRO,
        &["--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Filter connections");
    app.click(list_row(&app, "Analytics"));
    app.click(action_btn(&app, "Connect"));
    let failed = app.wait_screen("unreachable connection shows detail, no workbench", |s| {
        let t = text(s);
        (t.contains("Could not reach") || t.contains("unreachable") || t.contains("failed"))
            && !t.contains("Query 1")
    });
    assert!(!text(&failed).contains("Query 1"));
    freeze(&app, "functional/tablepro_connection_unreachable");
    app.finish();
}

#[test]
fn tablepro_connect_development_opens_workbench() {
    let app = App::new(
        "tablepro-dev-connect",
        TABLEPRO,
        &["--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Filter connections");
    app.click(list_row(&app, "Development"));
    app.click(action_btn(&app, "Connect"));
    app.wait_text("Query 1");
    freeze(&app, "functional/tablepro_workbench_development");
    app.finish();
}

#[test]
fn tablepro_delete_connection_cancel_then_confirm() {
    let app = App::new(
        "tablepro-delete-conn",
        TABLEPRO,
        &["--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Filter connections");
    app.click(list_row(&app, "Scratch"));
    app.click(action_btn(&app, "Delete…"));
    app.wait_text("Delete connection?");
    app.key("escape");
    app.wait_text("Scratch");
    app.click(action_btn(&app, "Delete…"));
    app.wait_text("Delete connection?");
    app.click(dialog_action(&app, "Delete"));
    app.wait_absent("Scratch");
    freeze(&app, "functional/tablepro_connection_deleted");
    app.finish();
}

fn orders_result(screen: &tui_test::TerminalSnapshot) -> bool {
    let body = text(screen);
    body.contains("Showing 500 rows")
        && body.contains("order_number")
        && body.contains("10000")
        && !body.contains("syntax error")
}

#[test]
fn tablepro_query_select_results_and_error_recovery() {
    let app = connected("tablepro-query-results");
    app.key("tab");
    app.paste("SELECT * FROM orders;");
    app.key("ctrl+r");
    app.wait_screen("select returns order rows", orders_result);
    freeze(&app, "functional/tablepro_query_results");
    close_all_tabs(&app);
    app.finish();

    let app = App::new(
        "tablepro-query-error",
        TABLEPRO,
        &["--connect", "Local PostgreSQL", "--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Query 1");
    app.key("tab");
    app.paste("SELEC bogus;");
    app.key("ctrl+r");
    let failed = app.wait_text("syntax error at or near \"SELEC\"");
    assert!(text(&failed).contains("SELEC bogus"));
    app.key("ctrl+l");
    app.paste("SELECT * FROM orders;");
    app.key("ctrl+r");
    let recovered = app.wait_screen("corrected query replaces the syntax error", orders_result);
    assert!(!text(&recovered).contains("syntax error at or near \"SELEC\""));
    assert!(!text(&recovered).contains("SELEC bogus"));
    app.expire_status();
    app.wait_absent("\u{2713} Connected to");
    app.visual("functional/tablepro_query_recovered");
    close_all_tabs(&app);
    app.finish();
}

#[test]
fn tablepro_history_rerun_and_help_and_layout() {
    let app = connected("tablepro-history");
    app.expire_status();
    app.key("tab");
    app.paste("SELECT * FROM orders;");
    app.key("ctrl+r");
    app.key("ctrl+y");
    app.wait_screen("history opens before the orders query settles", |s| {
        let t = text(s);
        t.contains("Query history")
            && t.contains("16 entries")
            && !t.contains("\u{2713} Connected to")
    });
    app.visual("functional/tablepro_history");
    app.key("r");
    let rerun = app.wait_screen("rerun executes the selected orders query", orders_result);
    assert!(text(&rerun).contains("SELECT * FROM orders"));
    assert!(!text(&rerun).contains("syntax error"));
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    assert!(text(&quit).contains("unsaved"));
    app.confirm_quit();

    let app = connected("tablepro-help-layout");
    app.key("?");
    let help = app.wait_text("Keyboard shortcuts");
    assert!(text(&help).contains("Shell and workbench"));
    assert!(text(&help).contains("Grid and editor"));
    app.key("escape");
    app.key("ctrl+b");
    app.wait_screen("explorer toggles closed", |s| {
        !text(s).contains("Filter objects")
    });
    app.key("ctrl+b");
    app.wait_text("Filter objects");
    freeze(&app, "functional/tablepro_workbench_layout");
    app.key("escape");
    app.finish();
}

#[test]
fn tablepro_resize_and_too_small_recovers() {
    let app = App::new(
        "tablepro-resize",
        TABLEPRO,
        &["--connect", "Local PostgreSQL", "--motion", "reduced"],
        120,
        40,
        "truecolor",
    );
    app.wait_text("Query 1");
    app.resize(60, 20);
    app.wait_screen("workbench at 60x20", |s| {
        s.cols() == 60
            && s.rows() == 20
            && (text(s).contains("Query") || text(s).contains("TablePro"))
    });
    app.visual("functional/tablepro_workbench_60x20");
    app.resize(120, 40);
    app.wait_screen("workbench recovered at 120x40", |s| {
        s.cols() == 120 && text(s).contains("Query 1")
    });
    freeze(&app, "functional/tablepro_resize_60x20");
    app.key("escape");
    app.finish();

    let app = App::new(
        "tablepro-nocolor",
        TABLEPRO,
        &["--motion", "reduced"],
        100,
        36,
        "nocolor",
    );
    app.wait_text("Filter connections");
    freeze(&app, "functional/tablepro_connections_nocolor");
    app.finish();
}

#[test]
fn issue9_tablepro_query_editor_unicode_paste_keeps_text_and_editor_alive() {
    let app = connected("tablepro-issue9-unicode");
    app.key("tab");
    let sql = "SELECT é, 名 FROM 表 WHERE note = '💾;中';";
    app.paste(sql);
    app.wait_text(sql);
    app.type_text("1");
    let screen = app.wait_text(&format!("{sql}1"));
    assert!(text(&screen).contains(sql), "{}", text(&screen));
}

fn uuid_in(row: &str) -> Option<&str> {
    let bytes = row.as_bytes();
    (0..bytes.len().saturating_sub(35)).find_map(|start| {
        let window = row.get(start..start + 36)?;
        let dashes = [8, 13, 18, 23];
        window
            .bytes()
            .enumerate()
            .all(|(i, b)| {
                if dashes.contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            })
            .then_some(window)
    })
}

fn row_of(screen: &tui_test::TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|index| row_text(screen, index))
        .find(|row| row.contains(needle))
        .unwrap_or_else(|| panic!("no row has {needle:?}\n{}", text(screen)))
}

#[test]
fn issue12_guarded_reload_pty() {
    const EDIT: &str = "GuardEdit";
    const PENDING: &str = "1 pending";
    const GUARD: &str = "Unsaved changes";

    let app = connected("tablepro-issue12-guarded-reload");
    app.key("ctrl+o");
    app.wait_text("Open Quickly");
    app.type_text("organizations");
    app.wait_text("organizations");
    app.key("enter");
    let table = app.wait_screen("organizations opened", |s| {
        text(s).contains("public › organizations") && !text(s).contains("Open Quickly")
    });
    let id_row = (0..table.rows())
        .map(|index| row_text(&table, index))
        .find(|row| uuid_in(row).is_some())
        .expect("a data row with an id");
    let id = uuid_in(&id_row).expect("an id").to_owned();

    app.key("right");
    app.key("enter");
    app.wait_text("Apply");
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text(EDIT);
    app.key("enter");
    let edited = app.wait_screen("the edit is pending", |s| {
        text(s).contains(PENDING) && row_of(s, &id).contains(EDIT)
    });
    assert!(!text(&edited).contains(GUARD));

    app.key("s");
    let asked = app.wait_text(GUARD);
    assert!(
        row_of(&asked, &id).contains(EDIT),
        "the draft is still on its row while the guard asks\n{}",
        text(&asked)
    );
    assert!(!text(&asked).contains("Sorted by"), "{}", text(&asked));

    app.key("escape");
    let kept = app.wait_screen("Cancel keeps the draft and the order", |s| {
        let visible = text(s);
        !visible.contains(GUARD) && visible.contains(EDIT) && visible.contains(PENDING)
    });
    assert!(row_of(&kept, &id).contains(EDIT), "{}", text(&kept));
    assert!(!text(&kept).contains("Sorted by"), "{}", text(&kept));

    app.key("s");
    app.wait_text(GUARD);
    app.key("left");
    app.key("enter");
    let sorted = app.wait_screen("Discard sorts and drops the draft", |s| {
        let visible = text(s);
        !visible.contains(GUARD) && !visible.contains(EDIT) && visible.contains("Sorted by")
    });
    assert!(!text(&sorted).contains(PENDING), "{}", text(&sorted));
    app.finish();
}
