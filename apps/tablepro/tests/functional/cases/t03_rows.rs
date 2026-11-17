//! TablePro query results before the refactor.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t03_rows/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";
const EMPTY_MARK: &str = "zzz-empty-t03-rows";
const EMPTY_SQL: &str =
    "SELECT id, name, slug FROM organizations WHERE name = 'zzz-empty-t03-rows'";
const KNOWN_SQL: &str = "SELECT id, name, slug FROM organizations WHERE name IN ('Tessellate', 'Northwind Labs') LIMIT 2";
const LARGE_SQL: &str = "SELECT id, name, slug FROM organizations";
const TESS: &str = "Tessellate";
const TESS_SLUG: &str = "tessellate";
const NORTH: &str = "Northwind Labs";
const NORTH_SLUG: &str = "northwind-labs";
const EMPTY_HINT: &str = "The query matched nothing";
const KNOWN_COUNT: &str = "2 rows";
const LARGE_COUNT: &str = "Showing 500 rows";
const KNOWN_TAB: &str = "(2)";
const READ_ONLY: &str = "Query results are read-only · open the table to edit its rows.";
const PIN: &str = "▪";
const RUNNING: &str = "Esc cancels";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t03_rows/{}",
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
        &[CONNECTED, "acme_prod › public", QUERY, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Loading schema",
        ],
    );
}

fn click_match(app: &App, label: &str, keep: impl Fn(&TerminalSnapshot, u16, u16) -> bool) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| keep(&screen, hit.x, hit.y))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "click {label:?} got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn focus_editor(app: &App) {
    app.wait_text(PLACEHOLDER);
    click_match(app, PLACEHOLDER, |screen, x, y| {
        x > 20 && row_text(screen, u32::from(y)).contains(PLACEHOLDER)
    });
    app.wait_text("Ctrl+Space");
}

fn focus_statement(app: &App, needle: &str) {
    click_match(app, needle, |screen, _x, y| {
        row_text(screen, u32::from(y)).contains(needle)
    });
    app.expire_status();
    app.wait_text("Ctrl+Space");
}

fn write_sql(app: &App, sql: &str, ready: &str) {
    app.type_text(sql);
    app.wait_text(ready);
}

fn replace_sql(app: &App, gone: &str, sql: &str, ready: &str) {
    app.key("ctrl+l");
    app.key("backspace");
    app.wait_absent(gone);
    app.type_text(sql);
    app.wait_text(ready);
}

fn finished(screen: &TerminalSnapshot) -> bool {
    !text(screen).contains(RUNNING) && !text(screen).contains("syntax error")
}

fn run_query(
    app: &App,
    reason: &str,
    ready: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    app.key("ctrl+r");
    app.wait_screen(reason, |screen| finished(screen) && ready(screen))
}

fn find_line(screen: &TerminalSnapshot, needle: &str) -> Option<(u32, String)> {
    let hits: Vec<_> = (0..screen.rows())
        .filter_map(|index| {
            let line = row_text(screen, index);
            line.contains(needle).then_some((index, line))
        })
        .collect();
    (hits.len() == 1).then(|| hits[0].clone())
}

fn line_with(screen: &TerminalSnapshot, needle: &str) -> (u32, String) {
    find_line(screen, needle).unwrap_or_else(|| panic!("line {needle:?} missing\n{}", text(screen)))
}

fn slug_order(screen: &TerminalSnapshot) -> Option<(u32, u32)> {
    Some((
        find_line(screen, TESS_SLUG)?.0,
        find_line(screen, NORTH_SLUG)?.0,
    ))
}

fn count_line(screen: &TerminalSnapshot, count: &str) -> String {
    line_with(screen, count).1
}

fn footer_toast(screen: &TerminalSnapshot) -> bool {
    let last = screen.rows().saturating_sub(1);
    row_text(screen, last).contains("✓ Connected to Development")
}

fn assert_known_pair(screen: &TerminalSnapshot) {
    let tess = line_with(screen, TESS_SLUG).1;
    let north = line_with(screen, NORTH_SLUG).1;
    assert!(
        tess.contains(TESS) && tess.contains(TESS_SLUG),
        "Tessellate row lost its slug: {tess}"
    );
    assert!(
        north.contains(NORTH) && north.contains(NORTH_SLUG),
        "Northwind row lost its slug: {north}"
    );
    assert!(
        count_line(screen, KNOWN_COUNT).contains(KNOWN_COUNT),
        "known count missing\n{}",
        text(screen)
    );
    see(
        screen,
        &[KNOWN_COUNT],
        &[EMPTY_MARK, EMPTY_HINT, RUNNING, "Showing"],
    );
}

fn confirm_quit(app: &App) {
    app.key("ctrl+q");
    let quit = app.wait_text("Quit TablePro?");
    see(
        &quit,
        &["Quit TablePro?", "1 unsaved query will be lost."],
        &[RUNNING, "Execute query?"],
    );
    app.confirm_quit();
}

#[test]
fn route_reaches_query_1_and_quits() {
    let app = open("t03-rows-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Connections", "Development", "Filter connections"],
        &[QUERY, PLACEHOLDER, "Loading schema", EMPTY_MARK],
    );
    connect_development(&app);
    let query = app.screen();
    assert_eq!((query.cols(), query.rows()), (120, 40));
    see(
        &query,
        &[CONNECTED, QUERY, PLACEHOLDER],
        &[
            "Loading schema",
            "Scratch",
            EMPTY_MARK,
            TESS,
            NORTH,
            RUNNING,
        ],
    );
    app.finish();
}

#[test]
fn rows_show_counts_sort_and_refuse_editing() {
    let app = open("t03-rows");
    connect_development(&app);
    focus_editor(&app);
    write_sql(&app, EMPTY_SQL, EMPTY_MARK);
    let empty = run_query(&app, "empty organizations result", |screen| {
        let visible = text(screen);
        visible.contains("No rows")
            && visible.contains(EMPTY_HINT)
            && visible.contains(EMPTY_MARK)
            && !visible.contains(TESS)
            && !visible.contains(NORTH)
            && !visible.contains(TESS_SLUG)
            && !visible.contains(NORTH_SLUG)
    });
    see(
        &empty,
        &[
            "No rows",
            EMPTY_HINT,
            EMPTY_MARK,
            "SELECT organizations (0)",
        ],
        &[TESS, NORTH, TESS_SLUG, NORTH_SLUG, LARGE_COUNT, RUNNING],
    );

    replace_sql(&app, EMPTY_MARK, KNOWN_SQL, "LIMIT 2");
    let known = run_query(&app, "two known organization rows", |screen| {
        let visible = text(screen);
        visible.contains(KNOWN_COUNT)
            && visible.contains(TESS)
            && visible.contains(TESS_SLUG)
            && visible.contains(NORTH)
            && visible.contains(NORTH_SLUG)
            && !visible.contains(EMPTY_MARK)
            && !visible.contains(EMPTY_HINT)
    });
    assert_known_pair(&known);
    let (tess_before, north_before) = slug_order(&known).expect("both slugs");
    assert!(
        tess_before < north_before,
        "fixture order was not Tessellate then Northwind: {tess_before} {north_before}\n{}",
        text(&known)
    );
    let count_before = count_line(&known, KNOWN_COUNT);

    click_match(&app, TESS_SLUG, |screen, _x, y| {
        row_text(screen, u32::from(y)).contains(TESS_SLUG)
    });
    app.key("s");
    let sorted = app.wait_screen("local sort reordered the known rows", |screen| {
        if !finished(screen) || find_line(screen, KNOWN_COUNT).is_none() {
            return false;
        }
        let Some((tess, north)) = slug_order(screen) else {
            return false;
        };
        let Some((_, count)) = find_line(screen, KNOWN_COUNT) else {
            return false;
        };
        north < tess && count == count_before
    });
    let (tess_after, north_after) = slug_order(&sorted).expect("both slugs after sort");
    assert!(
        north_after < tess_after,
        "sort left the row order unchanged\n{}",
        text(&sorted)
    );
    assert_eq!(
        count_line(&sorted, KNOWN_COUNT),
        count_before,
        "sort changed the result count line"
    );
    assert!(
        count_line(&sorted, KNOWN_COUNT).contains("slug ▴") || text(&sorted).contains("slug ▴"),
        "sort did not mark the slug header\n{}",
        text(&sorted)
    );
    assert_known_pair(&sorted);
    assert_eq!((sorted.cols(), sorted.rows()), (120, 40));
    flash_settle(&app);
    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert_known_pair(&wide);
    // The connected toast lasts 5s. visual() would keep the cleared footer.
    app.visual_while(&visual_id("T03.rows.rows.120x40"), |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(KNOWN_COUNT)
            && text(screen).contains(TESS_SLUG)
            && text(screen).contains(NORTH_SLUG)
            && !text(screen).contains(RUNNING)
            && footer_toast(screen)
    });

    app.resize(80, 24);
    let narrow = app.wait_screen("known rows at 80x24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(KNOWN_COUNT)
            && text(screen).contains(NORTH)
            && text(screen).contains(NORTH_SLUG)
            && !text(screen).contains(RUNNING)
            && !text(screen).contains(EMPTY_MARK)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[KNOWN_COUNT, NORTH, NORTH_SLUG],
        &[EMPTY_MARK, EMPTY_HINT, RUNNING, LARGE_COUNT],
    );
    if text(&narrow).contains(TESS) {
        assert!(
            text(&narrow).contains(TESS_SLUG),
            "narrow Tessellate row dropped its slug\n{}",
            text(&narrow)
        );
        let (tess, north) = slug_order(&narrow).expect("both slugs at 80x24");
        assert!(
            north < tess,
            "narrow sort order reverted\n{}",
            text(&narrow)
        );
    }
    flash_settle(&app);
    let narrow = app.screen();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[KNOWN_COUNT, NORTH, NORTH_SLUG],
        &[EMPTY_MARK, RUNNING],
    );
    app.visual_while(&visual_id("T03.rows.rows.narrow"), |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains(KNOWN_COUNT)
            && text(screen).contains(NORTH)
            && text(screen).contains(NORTH_SLUG)
            && !text(screen).contains(RUNNING)
            && !text(screen).contains(EMPTY_MARK)
            && footer_toast(screen)
    });
    app.resize(120, 40);
    let restored = app.wait_screen("known rows back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(KNOWN_COUNT)
            && text(screen).contains(TESS_SLUG)
            && text(screen).contains(NORTH_SLUG)
    });
    assert_known_pair(&restored);
    let (tess, north) = slug_order(&restored).expect("both slugs restored");
    assert!(north < tess, "restore lost the sort\n{}", text(&restored));

    let north_before_edit = line_with(&restored, NORTH_SLUG).1;
    app.key("enter");
    let refused = app.wait_text(READ_ONLY);
    let north_after_edit = line_with(&refused, NORTH_SLUG).1;
    assert!(
        north_after_edit.contains(NORTH) && north_after_edit.contains(NORTH_SLUG),
        "edit changed the Northwind row: {north_before_edit:?} -> {north_after_edit:?}"
    );
    assert!(
        north_after_edit.contains(NORTH_SLUG) && !text(&refused).contains("Apply"),
        "enter opened an editor\n{}",
        text(&refused)
    );
    see(
        &refused,
        &[READ_ONLY, TESS, TESS_SLUG, KNOWN_COUNT],
        &[EMPTY_MARK],
    );

    app.key(" ");
    let selected = app.wait_screen("northwind row selected", |screen| {
        let Some((_, north)) = find_line(screen, NORTH_SLUG) else {
            return false;
        };
        let Some((_, tess)) = find_line(screen, TESS_SLUG) else {
            return false;
        };
        north.contains('✓') && north.contains(NORTH) && !tess.contains('✓')
    });
    let picked = line_with(&selected, NORTH_SLUG).1;
    assert!(
        picked.contains(NORTH) && picked.contains(NORTH_SLUG) && picked.contains('✓'),
        "selection was not the Northwind row: {picked}"
    );
    app.key("y");
    let copied = app.wait_text("Copied");
    see(
        &copied,
        &["Copied", NORTH_SLUG, NORTH, KNOWN_COUNT],
        &[EMPTY_MARK, "Not copied"],
    );
    assert!(
        line_with(&copied, NORTH_SLUG).1.contains(NORTH),
        "copy left a different row selected\n{}",
        text(&copied)
    );

    click_match(&app, KNOWN_TAB, |screen, _x, y| {
        let row = row_text(screen, u32::from(y));
        row.contains(KNOWN_TAB) && row.contains("SELECT")
    });
    app.key("p");
    let pinned = app.wait_screen("known result pinned", |screen| {
        find_line(screen, KNOWN_TAB).is_some_and(|(_, line)| {
            line.contains(PIN) && line.contains("SELECT") && text(screen).contains(KNOWN_COUNT)
        })
    });
    let pin_row = line_with(&pinned, KNOWN_TAB).1;
    assert!(
        pin_row.contains(PIN) && pin_row.contains("SELECT"),
        "pin mark is not on the known result tab: {pin_row}"
    );

    focus_statement(&app, "LIMIT 2");
    replace_sql(&app, "LIMIT 2", LARGE_SQL, "FROM organizations");
    let large = run_query(&app, "large organizations result", |screen| {
        let visible = text(screen);
        visible.contains(LARGE_COUNT)
            && visible.contains(KNOWN_TAB)
            && visible.contains(PIN)
            && !visible.contains(EMPTY_MARK)
            && !visible.contains("LIMIT 2")
    });
    see(
        &large,
        &[LARGE_COUNT, "SELECT organizations (500)"],
        &[EMPTY_MARK, EMPTY_HINT, RUNNING, "No rows"],
    );
    let pinned_tab = line_with(&large, KNOWN_TAB).1;
    assert!(
        pinned_tab.contains(PIN) && pinned_tab.contains("SELECT"),
        "the second run dropped the pinned tab: {pinned_tab}"
    );

    click_match(&app, KNOWN_TAB, |screen, _x, y| {
        row_text(screen, u32::from(y)).contains(KNOWN_TAB)
    });
    let back = app.wait_screen("switched to the pinned result", |screen| {
        let visible = text(screen);
        visible.contains(KNOWN_COUNT)
            && visible.contains(TESS_SLUG)
            && visible.contains(NORTH_SLUG)
            && !visible.contains(LARGE_COUNT)
    });
    assert_known_pair(&back);
    let (tess, north) = slug_order(&back).expect("both slugs on the pinned tab");
    assert!(
        north < tess,
        "pinned tab lost its sorted values\n{}",
        text(&back)
    );

    app.key("x");
    let kept = app.settle();
    assert!(
        line_with(&kept, KNOWN_TAB).1.contains(PIN),
        "close removed the pinned tab\n{}",
        text(&kept)
    );
    assert!(
        text(&kept).contains(KNOWN_COUNT) && text(&kept).contains(NORTH_SLUG),
        "close changed the pinned result\n{}",
        text(&kept)
    );

    app.key("p");
    app.wait_screen("known result unpinned", |screen| {
        find_line(screen, KNOWN_TAB).is_some_and(|(_, line)| !line.contains(PIN))
    });
    app.key("x");
    let closed = app.wait_screen("unpinned result closed", |screen| {
        let visible = text(screen);
        visible.contains(LARGE_COUNT) && find_line(screen, KNOWN_TAB).is_none()
    });
    see(
        &closed,
        &[LARGE_COUNT, "SELECT organizations (500)"],
        &[KNOWN_TAB, EMPTY_MARK, KNOWN_COUNT, RUNNING],
    );
    confirm_quit(&app);
}
