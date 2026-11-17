//! TablePro data-type editors on public.orders, before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t02_editors/` and each checkpoint's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::bins::tablepro;
use crate::support::{App, Launch, find_all, flash_settle, row_text, text, try_launch};

const ROUTE_ID: &str = "pre_refactor/t02_editors/t02_editors_route_120x40";
const WIDE_ID: &str = "pre_refactor/t02_editors/t02_editors_types_120x40";
const NARROW_ID: &str = "pre_refactor/t02_editors/t02_editors_types_narrow";

const QUERY: &str = "Query 1";
const HOST: &str = "dev-db.internal.acme.io";
const UUID_BAD: &str = "not-a-uuid";
const UUID_OK: &str = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
const NOTE: &str = "desk-note";
const NUMBER_BAD: &str = "nope";
const NUMBER_OK: &str = "42.50";
const WHEN_BAD: &str = "yesterday";
const WHEN_OK: &str = "2020-01-01";
const JSON_BAD: &str = "not-json";
const READ_ONLY: &str = "order_number is read-only";
const CONNECTED: &str = "Connected to Development";
const CLOSE_DIRTY: &str = "Close tab with unsaved work?";
const FETCHING: &str = "fetching";

const COL_ID: usize = 0;
const COL_ORDER: usize = 1;
const COL_STATUS: usize = 4;
const COL_AMOUNT: usize = 5;
const COL_ADDRESS: usize = 7;
const COL_NOTES: usize = 8;
const COL_DELIVERED: usize = 10;
const COL_GIFT: usize = 11;

const STATUSES: [&str; 6] = [
    "refunded",
    "cancelled",
    "delivered",
    "shipped",
    "pending",
    "paid",
];

fn connect(name: &str) -> App {
    let mut launch = Launch::new(name, tablepro());
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    click_listed(&app, "Development");
    flash_settle(&app);
    app.wait_text(HOST);
    click_connect(&app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    app
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

fn focused(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('\u{258e}'))
        .unwrap_or_else(|| panic!("no focus bar\n{}", text(screen)))
}

fn token_eq(line: &str, want: &str) -> bool {
    line.split_whitespace()
        .any(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_') == want)
}

fn occurrences(screen: &TerminalSnapshot, needle: &str) -> usize {
    text(screen).matches(needle).count()
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        && value.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

fn uuid_on(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    for start in 0..=chars.len().saturating_sub(36) {
        let value: String = chars[start..start + 36].iter().collect();
        if is_uuid(&value) {
            return value;
        }
    }
    panic!("uuid missing\n{line}");
}

fn order_no(line: &str) -> String {
    line.split_whitespace()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_digit()))
        .find(|token| token.len() == 5 && token.starts_with('1'))
        .unwrap_or_else(|| panic!("order number missing\n{line}"))
        .to_string()
}

fn status_on(line: &str) -> &'static str {
    let found: Vec<_> = STATUSES
        .into_iter()
        .filter(|status| line.contains(status))
        .collect();
    assert_eq!(found.len(), 1, "status on {line:?} is {found:?}");
    found[0]
}

fn gift_on(line: &str) -> &'static str {
    if line.contains("false") {
        "false"
    } else if line.contains("true") {
        "true"
    } else {
        panic!("bool missing\n{line}");
    }
}

/// `key` returns before the next frame. A decision that reads the old frame
/// queues another key against the same cursor.
fn focus_after(app: &App, key: &str) -> String {
    let before = focused(&app.screen());
    app.key(key);
    let after = app.wait_screen(&format!("focus {key}"), |screen| focused(screen) != before);
    focused(&after)
}

fn open_orders(app: &App) {
    let mut line = focus_after(app, "home");
    for _ in 0..48 {
        if token_eq(&line, "orders") {
            app.key("enter");
            app.wait_text("order_number");
            app.wait_text("10000");
            return;
        }
        line = focus_after(app, "down");
    }
    panic!("orders row missing\n{}", text(&app.screen()));
}

fn go(app: &App, col: &mut usize, target: usize) {
    let key = if *col > target { "left" } else { "right" };
    let steps = col.abs_diff(target);
    for _ in 0..steps {
        app.key(key);
    }
    *col = target;
    if steps > 0 {
        // The arrow count is the cursor, so one quiet wait covers the burst.
        // Same-row moves often keep the text and only change the cursor cell.
        app.settle();
    }
}

fn tab_has_orders(screen: &TerminalSnapshot) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains(QUERY) && token_eq(&line, "orders")
    })
}

fn replace_focused(app: &App, value: &str) {
    app.key("ctrl+l");
    app.key("backspace");
    if value.is_empty() {
        return;
    }
    app.type_text(value);
    // The open editor truncates a 36-character uuid (`…aaaaa-bbbb-…`). The
    // committed cell draws the whole value, so the tail is enough while typing.
    let drawn = if value.len() > 20 {
        &value[value.len() - 16..]
    } else {
        value
    };
    app.wait_text(drawn);
}

fn begin_edit(app: &App) {
    app.key("enter");
    app.wait_text("Apply");
}

fn reject_inline(app: &App, bad: &str, original: &str) {
    begin_edit(app);
    replace_focused(app, bad);
    app.key("enter");
    let rejected = app.wait_screen(&format!("reject {bad}"), |screen| {
        let line = focused(screen);
        line.contains(bad) && line.contains('!')
    });
    assert!(
        focused(&rejected).contains(bad),
        "invalid value left the editor\n{}",
        text(&rejected)
    );
    app.key("escape");
    let restored = app.wait_screen(&format!("cancel {bad}"), |screen| {
        let line = focused(screen);
        !line.contains(bad) && (original.is_empty() || line.contains(original))
    });
    assert!(
        !text(&restored).contains(bad),
        "rejected value still drawn\n{}",
        text(&restored)
    );
}

fn commit_value(app: &App, value: &str) {
    begin_edit(app);
    replace_focused(app, value);
    app.key("enter");
    app.wait_screen(&format!("commit {value}"), |screen| {
        let line = focused(screen);
        line.contains(value) && !line.contains('!') && !text(screen).contains("Apply")
    });
}

fn view_address(app: &App, col: &mut usize) {
    go(app, col, COL_ADDRESS);
    app.wait_text("shipping_address");
    let before = app.screen();
    let before_line = focused(&before);
    let before_count = occurrences(&before, "shipping_address");
    assert!(
        !text(&before).contains("1 pending"),
        "pending before the viewer\n{}",
        text(&before)
    );
    app.key("enter");
    let open = app.wait_screen("address viewer", |screen| {
        occurrences(screen, "shipping_address") > before_count
    });
    assert!(
        !text(&open).contains("Apply"),
        "json opened a text editor\n{}",
        text(&open)
    );
    app.type_text(JSON_BAD);
    app.settle();
    let typed = app.screen();
    assert!(
        !text(&typed).contains(JSON_BAD),
        "invalid json was typed into the viewer\n{}",
        text(&typed)
    );
    assert!(
        !text(&typed).contains("1 pending"),
        "viewer dirtied the grid\n{}",
        text(&typed)
    );
    app.key("escape");
    let closed = app.wait_screen("address viewer closed", |screen| {
        occurrences(screen, "shipping_address") == before_count && focused(screen) == before_line
    });
    assert!(
        !text(&closed).contains(JSON_BAD),
        "invalid json remained after the viewer\n{}",
        text(&closed)
    );
    assert!(
        !text(&closed).contains("1 pending"),
        "pending appeared after the viewer\n{}",
        text(&closed)
    );
}

fn notes_row(app: &App, col: &mut usize) -> usize {
    go(app, col, COL_NOTES);
    for row in 0..12 {
        let before = occurrences(&app.screen(), "notes");
        app.key("enter");
        let opened = app.wait_screen("notes key", |screen| {
            text(screen).contains("Apply") || occurrences(screen, "notes") > before
        });
        if text(&opened).contains("Apply") {
            app.key("escape");
            app.wait_absent("Apply");
            return row;
        }
        app.key("escape");
        app.wait_screen("notes viewer closed", |screen| {
            occurrences(screen, "notes") <= before
        });
        let _ = focus_after(app, "down");
    }
    panic!("no editable notes cell\n{}", text(&app.screen()));
}

fn confirm_quit(app: &App) {
    app.key("ctrl+q");
    app.wait_text("Quit TablePro?");
    app.confirm_quit();
}

fn pending_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("update") && line.contains("pending"))
        .unwrap_or_else(|| panic!("pending detail missing\n{}", text(screen)))
}

#[test]
fn tablepro_editors_route_opens_orders_and_returns_to_query() {
    let app = connect("t02-editors-route");
    open_orders(&app);
    for _ in 0..16 {
        if text(&app.screen()).contains("total_amount") {
            break;
        }
        app.key("right");
        app.settle();
    }
    let opened = app.wait_text("total_amount");
    assert!(
        tab_has_orders(&opened),
        "orders tab missing\n{}",
        text(&opened)
    );
    assert!(
        text(&opened).contains("orders"),
        "orders heading missing\n{}",
        text(&opened)
    );
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    app.expire_status();
    app.wait_absent(CONNECTED);
    flash_settle(&app);
    app.visual(ROUTE_ID);
    app.key("ctrl+w");
    let back = app.wait_screen("orders tab closed", |screen| {
        text(screen).contains(QUERY) && !tab_has_orders(screen)
    });
    assert!(
        !text(&back).contains("pending"),
        "pending marker after a clean close\n{}",
        text(&back)
    );
    assert!(
        !text(&back).contains(CLOSE_DIRTY),
        "clean close asked for confirmation\n{}",
        text(&back)
    );
    app.finish();
}

#[test]
fn tablepro_editors_types_accept_and_reject_each_family() {
    let app = connect("t02-editors-types");
    open_orders(&app);
    let mut col = 0;

    view_address(&app, &mut col);
    let row = notes_row(&app, &mut col);

    go(&app, &mut col, COL_ID);
    let original_id = uuid_on(&focused(&app.screen()));
    reject_inline(&app, UUID_BAD, &original_id);
    commit_value(&app, UUID_OK);
    let id_line = focused(&app.screen());
    assert!(
        id_line.contains(UUID_OK) && !id_line.contains(&original_id),
        "uuid commit\n{id_line}"
    );
    assert!(is_uuid(UUID_OK));

    go(&app, &mut col, COL_ORDER);
    let edited_no = order_no(&focused(&app.screen()));
    app.key("enter");
    let read_only = app.wait_text(READ_ONLY);
    assert!(
        !text(&read_only).contains("Apply"),
        "read-only opened an editor\n{}",
        text(&read_only)
    );
    assert!(
        focused(&read_only).contains(&edited_no),
        "read-only changed the order number\n{}",
        focused(&read_only)
    );

    go(&app, &mut col, COL_STATUS);
    let original_status = status_on(&focused(&app.screen()));
    let replacement = STATUSES
        .into_iter()
        .find(|status| *status != original_status)
        .expect("replacement status");
    reject_inline(&app, NUMBER_BAD, original_status);
    commit_value(&app, replacement);
    let status_line = focused(&app.screen());
    assert!(
        status_line.contains(replacement) && !status_line.contains(original_status),
        "enum commit\n{status_line}"
    );
    assert!(!status_line.contains(NUMBER_BAD));

    go(&app, &mut col, COL_AMOUNT);
    reject_inline(&app, NUMBER_BAD, "");
    commit_value(&app, NUMBER_OK);
    assert!(
        focused(&app.screen()).contains(NUMBER_OK),
        "number commit\n{}",
        focused(&app.screen())
    );

    go(&app, &mut col, COL_NOTES);
    commit_value(&app, NOTE);
    assert!(focused(&app.screen()).contains(NOTE));

    go(&app, &mut col, COL_DELIVERED);
    reject_inline(&app, WHEN_BAD, "");
    commit_value(&app, WHEN_OK);
    let dated = app.screen();
    let nulls = occurrences(&dated, "NULL");
    assert!(focused(&dated).contains(WHEN_OK));
    app.key("delete");
    let cleared = app.wait_screen("nullable clear", |screen| {
        let line = focused(screen);
        !line.contains(WHEN_OK) && occurrences(screen, "NULL") > nulls
    });
    assert!(
        focused(&cleared).contains("NULL") && !focused(&cleared).contains(WHEN_OK),
        "nullable cell did not draw NULL\n{}",
        focused(&cleared)
    );

    go(&app, &mut col, COL_GIFT);
    let before_gift = gift_on(&focused(&app.screen()));
    let after_gift = if before_gift == "true" {
        "false"
    } else {
        "true"
    };
    app.key("enter");
    let toggled = app.wait_screen("bool toggle", |screen| {
        let line = focused(screen);
        line.contains(after_gift) && !text(screen).contains("Apply")
    });
    assert!(
        focused(&toggled).contains(after_gift),
        "bool did not toggle\n{}",
        focused(&toggled)
    );
    app.key("delete");
    let kept = app.wait_screen("not null clear", |screen| {
        focused(screen).contains(after_gift)
    });
    assert!(
        focused(&kept).contains(after_gift),
        "not-null bool became NULL\n{}",
        focused(&kept)
    );
    assert!(
        !text(&kept).contains("is_gift is NOT NULL"),
        "not-null reason was drawn\n{}",
        text(&kept)
    );

    go(&app, &mut col, COL_ORDER);
    assert!(focused(&app.screen()).contains(&edited_no));
    let neighbor = order_no(&focus_after(&app, "down"));
    assert_ne!(neighbor, edited_no);
    app.key("-");
    let marked = app.wait_screen("row marked", |screen| {
        let visible = text(screen);
        visible.contains(&edited_no) && visible.contains(&neighbor)
    });
    assert!(
        text(&marked).contains(&neighbor),
        "delete removed the selected row\n{}",
        text(&marked)
    );
    assert!(
        text(&marked).contains(&edited_no),
        "delete removed a different row\n{}",
        text(&marked)
    );

    app.key("up");
    let returned = app.wait_screen("back to edited row", |screen| {
        focused(screen).contains(&edited_no)
    });
    assert!(focused(&returned).contains(&edited_no));
    go(&app, &mut col, COL_NOTES);
    assert!(
        focused(&app.screen()).contains(NOTE),
        "deleting another row dropped the notes edit\n{}",
        focused(&app.screen())
    );
    // ctrl+d is Close-adjacent shell structure toggle while the grid is idle.
    app.key("alt+d");
    let duplicated = app.wait_screen("duplicated notes", |screen| focused(screen).contains(NOTE));
    assert!(focused(&duplicated).contains(NOTE));
    app.key("home");
    let duplicated_id = app.wait_screen("duplicated primary", |screen| {
        focused(screen).contains("DEFAULT")
    });
    assert!(
        focused(&duplicated_id).contains("DEFAULT"),
        "duplicate primary did not draw DEFAULT\n{}",
        focused(&duplicated_id)
    );

    app.key("+");
    app.key("home");
    let inserted = app.wait_screen("inserted row", |screen| {
        let line = focused(screen);
        line.contains("DEFAULT") && line.contains("NULL")
    });
    assert!(
        focused(&inserted).contains("DEFAULT") && focused(&inserted).contains("NULL"),
        "inserted row\n{}",
        focused(&inserted)
    );

    app.key("ctrl+home");
    for _ in 0..row {
        app.key("down");
    }
    let mut col = 0;
    go(&app, &mut col, COL_NOTES);
    let settled = app.wait_screen("edited row visible", |screen| {
        focused(screen).contains(NOTE) && !text(screen).contains(FETCHING)
    });
    assert!(focused(&settled).contains(NOTE));
    assert!(!text(&settled).contains(UUID_BAD));
    assert!(!text(&settled).contains(NUMBER_BAD));
    assert!(!text(&settled).contains(WHEN_BAD));
    assert!(!text(&settled).contains(JSON_BAD));
    app.expire_status();
    app.wait_absent(CONNECTED);
    app.wait_absent(READ_ONLY);
    let wide = app.screen();
    let detail = pending_line(&wide);
    assert!(
        detail.contains("\u{2022} 4 pending")
            && detail.contains("1 update")
            && detail.contains("2 inserts")
            && detail.contains("1 delete"),
        "pending sentence\n{detail}\n{}",
        text(&wide)
    );
    assert!(
        text(&wide).contains("\u{2022} 4 pending"),
        "shell pending marker\n{}",
        text(&wide)
    );
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    flash_settle(&app);
    app.visual(WIDE_ID);

    app.resize(80, 24);
    // The terminal clips the previous frame before the app handles SIGWINCH.
    // Wait until the redraw puts the title and the notes edit back.
    let narrow = app.wait_screen("80x24 redraw", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && row_text(screen, 0).contains("TablePro")
            && text(screen).contains(NOTE)
    });
    assert!(
        text(&narrow).contains("4 pending"),
        "narrow pending count\n{}",
        text(&narrow)
    );
    let narrow_line = (0..narrow.rows())
        .map(|row| row_text(&narrow, row))
        .find(|line| line.contains("4 pending"))
        .unwrap_or_else(|| panic!("narrow pending missing\n{}", text(&narrow)));
    assert!(
        narrow_line.contains("4") && narrow_line.contains("pending"),
        "narrow pending line\n{narrow_line}"
    );
    flash_settle(&app);
    app.visual(NARROW_ID);

    app.resize(120, 40);
    let restored = app.wait_text("120×40");
    assert_eq!((restored.cols(), restored.rows()), (120, 40));
    confirm_quit(&app);
}
