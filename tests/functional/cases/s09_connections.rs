//! Connections route, sections, specimens, compare levels, options, and records.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s09_connections/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::bins::showcase;
use crate::support::{App, Launch, Point, find_all, flash_settle, row_text, text, try_launch};

const CRUMB: &str = "Screens › Connections";
const PREV: &str = "Screens › Query workbench";
const NEXT: &str = "Screens › Finder";
const TITLE: &str = "Connections";
const BLURB: &str = "Grouped connections with env glyphs, details, connect and the form";
const BLURB_NARROW: &str = "Grouped connections with env glyphs, details…";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "connections"];
const HOST: &str = "prod-db-1.acme.io:5432";
const PAUSED_BAR: &str = "Connecting    ─";
const CONNECTED: &str = "✓ Connected · PostgreSQL 16.3 · 12 ms";
const FAILED: &str = "Could not reach the host";
const TEST_OK: &str = "✓ Connected in 12 ms";
const TEST_FAIL: &str = "✗ Timed out after 10 s";
const WAREHOUSE: &str = "Warehouse";
const WAREHOUSE_COPY: &str = "Warehouse copy";
const DRAFT: &str = "Should not save";

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s09_connections/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, showcase());
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Components");
    app
}

fn page(name: &str, opt: Option<&str>, cols: u16, rows: u16) -> App {
    let mut args = PAGE_ARGS.to_vec();
    if let Some(value) = opt {
        args.extend(["--opt", value]);
    }
    open(name, cols, rows, &args)
}

fn flat(screen: &TerminalSnapshot) -> String {
    text(screen)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = flat(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{}", text(screen));
    }
    for item in absent {
        assert!(
            !visible.contains(item),
            "forbidden {item:?} is visible\n{}",
            text(screen)
        );
    }
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
}

fn value_under(screen: &TerminalSnapshot, label: &str) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = lines
        .iter()
        .enumerate()
        .find(|(row, line)| *row >= 6 && line_label_ends(line, label))
        .map(|(row, _)| row)
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    let column = lines[index].find(label).expect("label column");
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).map(String::as_str).unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    rest.split_whitespace()
        .find(|token| *token != "▎")
        .unwrap_or("")
        .to_string()
}

fn line_label_ends(line: &str, label: &str) -> bool {
    let Some(at) = line.find(label) else {
        return false;
    };
    line[at + label.len()..].trim().is_empty()
}

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str, others: &[&str]) {
    let got = value_under(screen, label);
    assert_eq!(got, value, "{label} shows {got:?}, expected {value}");
    for other in others {
        assert_ne!(got, *other, "{label} shows the other value {other}");
    }
}

fn assert_one(screen: &TerminalSnapshot, label: &str, value: &str, all: &[&str]) {
    let others: Vec<&str> = all.iter().copied().filter(|item| *item != value).collect();
    assert_select(screen, label, value, &others);
}

fn assert_toggle(screen: &TerminalSnapshot, label: &str, state: &str) {
    let line = row_with(screen, label);
    let at = line.find(label).expect("label");
    let tail = &line[at + label.len()..];
    let other = if state == "on" { "off" } else { "on" };
    assert!(
        tail.contains(state) && !tail.contains(other),
        "{label} row is {line:?}, expected {state}"
    );
}

fn assert_connection_options(
    screen: &TerminalSnapshot,
    group: &str,
    meta: &str,
    width: &str,
    list: &str,
    state: &str,
    form: &str,
) {
    assert_one(screen, "Group by", group, &["folder", "environment"]);
    assert_toggle(screen, "Engine column", meta);
    assert_one(screen, "List share %", width, &["40", "30", "50"]);
    assert_one(screen, "List", list, &["sample", "empty"]);
    assert_one(
        screen,
        "Connect state",
        state,
        &["idle", "connecting", "connected", "failed"],
    );
    assert_one(screen, "Form width", form, &["60", "52", "72"]);
}

fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    grid_hit(screen, label).is_some()
}

fn click_where(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    hits[0]
}

fn click_tab(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &App, label: &str) {
    reveal_side(app, label);
    click_where(app, label, |hit| hit.x < 24);
}

fn reveal_side(app: &App, label: &str) {
    let point = Point { x: 8, y: 16 };
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel(point, false, 3);
    }
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel(point, true, 3);
    }
}

fn assert_size(app: &App, cols: u16, rows: u16) {
    let screen = app.settle();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (u32::from(cols), u32::from(rows)),
        "terminal size\n{}",
        text(&screen)
    );
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn frame_span(screen: &TerminalSnapshot, title: &str) -> u16 {
    let line = row_with(screen, title);
    let chars: Vec<char> = line.chars().collect();
    let start = chars
        .iter()
        .position(|glyph| *glyph == '╭')
        .unwrap_or_else(|| panic!("no dialog top\n{line}"));
    let end = chars
        .iter()
        .position(|glyph| *glyph == '╮')
        .unwrap_or_else(|| panic!("no dialog top end\n{line}"));
    u16::try_from(end - start + 1).expect("dialog width")
}

fn host_x(screen: &TerminalSnapshot) -> u16 {
    let hits = find_all(screen, HOST);
    assert_eq!(
        hits.len(),
        1,
        "expected one host, got {hits:?}\n{}",
        text(screen)
    );
    hits[0].x
}

fn list_pane(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .filter_map(|row| {
            let line = row_text(screen, row);
            line.split_once('│').map(|(master, _)| master.to_string())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn button_row(screen: &TerminalSnapshot) -> String {
    row_with(screen, "Edit…")
}

fn assert_connect_label(screen: &TerminalSnapshot, label: &str) {
    let row = button_row(screen);
    assert!(
        row.contains(label) && row.contains("Edit…") && row.contains("Duplicate"),
        "action row is {row:?}"
    );
    if label == "Connect" {
        assert!(!row.contains("Reconnect"), "action row is {row:?}");
    }
}

fn assert_paused_connecting(screen: &TerminalSnapshot) {
    let row = row_with(screen, "Connecting");
    assert!(
        row.contains(PAUSED_BAR) && !row.contains('━'),
        "paused connecting frame moved\n{row}"
    );
    see(
        screen,
        &["1 hour ago", HOST],
        &[
            CONNECTED,
            FAILED,
            "Authentication failed",
            "Reconnect",
            "✓ Connected to",
        ],
    );
    assert_connect_label(screen, "Connect");
}

fn assert_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "Filter connections",
            "▾ Personal",
            "Local PostgreSQL",
            "Scratch",
            "sqlite",
            "▾ Acme",
            "Development",
            "Staging",
            "Analytics",
            "mysql",
            "Production",
            "pg",
            "PostgreSQL",
            HOST,
            "acme_prod",
            "acme_ops",
            "Safe Mode",
            "bastion.acme.io",
            "1 hour ago",
            "New connection…",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "focus + hover",
            OVERVIEW,
            "No connections",
            CONNECTED,
            FAILED,
            "Reconnect",
            TEST_OK,
            TEST_FAIL,
            "/Users",
            "/home",
        ],
    );
    assert_connect_label(screen, "Connect");
}

fn assert_clean_options(screen: &TerminalSnapshot) {
    assert_connection_options(screen, "folder", "on", "40", "sample", "idle", "60");
}

fn confirm_options(app: &App, back: &str, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(back);
    flash_settle(app);
}

fn omitted_specimens(screen: &TerminalSnapshot) {
    for label in [
        "focus + hover",
        "error",
        "editing",
        "empty",
        "loading",
        "overflow",
    ] {
        assert!(
            !grid_label(screen, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(screen)
        );
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s09-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    assert!(
        find_all(&screen, "Playground").iter().any(|hit| hit.y == 4)
            && find_all(&screen, "Options").iter().any(|hit| hit.y == 4)
            && find_all(&screen, "States").iter().any(|hit| hit.y == 4),
        "section tabs are not on row 4\n{}",
        text(&screen)
    );
    assert!(
        find_all(&screen, "Connections")
            .iter()
            .any(|hit| hit.x < 24),
        "sidebar label is not left of column 24\n{}",
        text(&screen)
    );
    shot(&app, "sc.connections.route");
    app.finish();

    let app = open("s09-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    let stops = [
        "App shell › Chrome",
        "App shell › Status & hints",
        "App shell › Breadcrumbs & rails",
        "App shell › Help & keys",
        "App shell › Quit & leave",
        "App shell › Focus & navigation",
        "Inputs & forms › Buttons",
        "Inputs & forms › Inputs",
        "Inputs & forms › Text areas",
        "Inputs & forms › Chips & selects",
        "Inputs & forms › Forms",
        "Inputs & forms › Checks & radios",
        "Inputs & forms › Toolbars",
        "Inputs & forms › Fixed controls",
        "Tables › Basic table",
        "Tables › Editable",
        "Tables › Data grid",
        "Tables › Selection",
        "Tables › Cell editors",
        "Tables › Columns",
        "Navigation › Lists",
        "Navigation › Trees",
        "Navigation › Sidebars",
        "Navigation › Tabs",
        "Navigation › Menus",
        "Overlays › Dialogs",
        "Overlays › Pickers",
        "Overlays › Dialog kinds",
        "Overlays › Popups & tooltips",
        "Overlays › File browser",
        "Overlays › Fixed dialogs",
        "Feedback › Progress",
        "Feedback › Empty & error",
        "Feedback › Meters",
        "Feedback › Notices",
        "Layout › Panels",
        "Layout › Scrolling",
        "Layout › Splits & stacks",
        "Layout › Master-detail",
        "Layout › Pane tree",
        "Layout › List detail",
        "Text & code › Code editor",
        "Text & code › Diff",
        "Text & code › Terminal",
        "Text & code › Terminal pane",
        "Text & code › Terminal viewport",
        "Text & code › Text utilities",
        "Screens › Settings",
        "Screens › Task runner",
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(HOST);
    see(
        &entered,
        &[CRUMB, BLURB, "▾ Personal", "Production", HOST],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, BLURB, "Safe Mode"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, HOST, "New connection…"], &[PREV, OVERVIEW]);
    app.key("]");
    let next = app.wait_text(NEXT);
    see(&next, &[NEXT], &[CRUMB, BLURB, "Safe Mode"]);
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, HOST]);
    click_side(&app, "Screens");
    click_side(&app, "Connections");
    let pointed = app.wait_text(HOST);
    see(
        &pointed,
        &[CRUMB, BLURB, "Production", "New connection…"],
        &[OVERVIEW, "Page options", "Compare levels", PREV],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s09-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Local PostgreSQL");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            BLURB_NARROW,
            "Playground",
            "Filter connections",
            "▾ Personal",
            "Local PostgreSQL",
            "sqlite",
            "▾ Acme",
            "New connection…",
            "→ Production · production · prod-db-1.acme.io:5432",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            "connect and the form",
            "Safe Mode",
            "Edit…",
            "bastion.acme.io",
            "focus + hover",
        ],
    );
    shot(&app, "sc.connections.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_clean_options(&narrow_opt);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Group by", "Form width"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            HOST,
            "focus + hover",
        ],
    );
    shot(&app, "sc.connections.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "hover", "pressed", "focused"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    for label in ["selected", "disabled", "busy", "narrow"] {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is on the 80 by 24 states screen\n{}",
            text(&narrow_states)
        );
    }
    omitted_specimens(&narrow_states);
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "Staging",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            HOST,
            "focus + hover",
        ],
    );
    shot(&app, "sc.connections.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
    });
    assert_size(&app, 120, 40);
    for label in [
        "rest", "hover", "pressed", "focused", "selected", "disabled", "busy", "narrow",
    ] {
        assert!(
            grid_label(&wide_states, label),
            "wide states missing {label}\n{}",
            text(&wide_states)
        );
    }
    omitted_specimens(&wide_states);
    see(
        &wide_states,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "⠋ Connect",
            "Delete…",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            HOST,
            "focus + hover",
            "256 colors",
            "No connections",
        ],
    );
    for (case_id, label) in [
        ("sc.connections.specimen.rest", "rest"),
        ("sc.connections.specimen.hover", "hover"),
        ("sc.connections.specimen.pressed", "pressed"),
        ("sc.connections.specimen.focused", "focused"),
        ("sc.connections.specimen.selected", "selected"),
        ("sc.connections.specimen.disabled", "disabled"),
        ("sc.connections.specimen.busy", "busy"),
        ("sc.connections.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.connections.states.120x40");
    shot(&app, "sc.connections.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_clean_options(&wide_opt);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "folder"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            HOST,
            "focus + hover",
        ],
    );
    shot(&app, "sc.connections.options.120x40");
    shot(&app, "sc.connections.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(HOST);
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.connections.playground.120x40");
    shot(&app, "sc.connections.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s09-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    app.wait_text("● Compare levels");
    flash_settle(&app);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[CRUMB, "● Compare levels", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 18 };
    for _ in 0..120 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel(point, true, 3);
    }
    let bands = app.settle();
    assert_size(&app, 120, 40);
    see(
        &bands,
        &[CRUMB, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    assert!(
        saw_256 && saw_16,
        "a colour band was skipped before no color\n{}",
        text(&bands)
    );
    shot(&app, "sc.connections.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    let narrow = app.settle();
    if !text(&narrow).contains("no color") {
        for _ in 0..120 {
            if text(&app.settle()).contains("no color") {
                break;
            }
            app.wheel(point, true, 3);
        }
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.connections.compare-levels.80x24");

    let top = Point { x: 40, y: 12 };
    for _ in 0..120 {
        if text(&app.settle()).contains("● Compare levels") {
            break;
        }
        app.wheel(top, false, 3);
    }
    let top_screen = app.settle();
    see(
        &top_screen,
        &[TITLE, "● Compare levels"],
        &["no color", "Page options", "80×24"],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "truecolor",
            "Page options",
            "80×24",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_clean_options(&options);
    see(
        &options,
        &["folder", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "environment"],
    );
    app.finish();
}

fn open_edit(app: &App) {
    click_where(app, "Edit…", |hit| hit.y > 8 && hit.y < 38);
    app.wait_text("Edit connection");
    flash_settle(app);
}

fn open_new(app: &App) {
    click_where(app, "New connection…", |hit| hit.y > 8 && hit.x < 80);
    app.wait_text("New connection");
    flash_settle(app);
}

#[test]
fn create_edit_test_save_cancel_duplicate_delete() {
    let app = open("s09-records", 120, 40, PAGE_ARGS);
    let before = app.wait_text(HOST);
    assert_size(&app, 120, 40);
    assert_playground(&before);
    shot(&app, "sc.connections.connections.before");

    open_new(&app);
    let draft = app.wait_text("New connection");
    see(
        &draft,
        &[
            "New connection",
            "Name *",
            "Host *",
            "localhost:5432",
            "Test connection",
            "Cancel",
            "Save",
        ],
        &["Saved", TEST_OK, TEST_FAIL, WAREHOUSE],
    );
    app.type_text(WAREHOUSE);
    click_where(&app, "localhost:5432", |hit| hit.y > 8);
    app.settle();
    app.type_text("analytics.example");
    click_where(&app, "Test connection", |hit| hit.y > 8);
    flash_settle(&app);
    let failed = app.wait_text(TEST_FAIL);
    see(
        &failed,
        &[TEST_FAIL, WAREHOUSE, "analytics.example", "Save", "Cancel"],
        &["Saved", TEST_OK, "Saved Warehouse"],
    );
    click_where(&app, "Cancel", |hit| hit.y > 8 && hit.y < 36);
    flash_settle(&app);
    let kept = app.wait_absent("Test connection");
    see(
        &kept,
        &[HOST, "Production", "New connection…"],
        &[DRAFT, WAREHOUSE, TEST_FAIL, "Saved", "analytics.example"],
    );

    open_new(&app);
    app.wait_text("localhost:5432");
    app.type_text(WAREHOUSE);
    click_where(&app, "Test connection", |hit| hit.y > 8);
    flash_settle(&app);
    let ok = app.wait_text(TEST_OK);
    see(
        &ok,
        &[TEST_OK, WAREHOUSE, "localhost:5432"],
        &[TEST_FAIL, "Saved"],
    );
    click_where(&app, "Save", |hit| hit.y > 8 && hit.y < 36);
    flash_settle(&app);
    let saved = app.wait_text("Saved Warehouse");
    see(
        &saved,
        &[
            "Saved Warehouse",
            WAREHOUSE,
            "localhost:5432",
            "Production",
            "development",
        ],
        &[TEST_OK, TEST_FAIL, "Test connection", "Should not save"],
    );

    open_edit(&app);
    let reopened = app.wait_text("Edit connection");
    see(
        &reopened,
        &[
            "Edit connection",
            WAREHOUSE,
            "localhost:5432",
            "Save",
            "Cancel",
        ],
        &[TEST_OK, "Temporary"],
    );
    app.type_text("Temporary");
    click_where(&app, "Cancel", |hit| hit.y > 8 && hit.y < 36);
    flash_settle(&app);
    let cancelled = app.wait_absent("Edit connection");
    see(
        &cancelled,
        &[WAREHOUSE, "localhost:5432", "Production"],
        &["Temporary", "Saved Temporary", "Edit connection"],
    );

    open_edit(&app);
    let still = app.wait_text(WAREHOUSE);
    see(
        &still,
        &["Edit connection", WAREHOUSE, "localhost:5432"],
        &["Temporary"],
    );
    app.key("esc");
    flash_settle(&app);
    app.wait_absent("Edit connection");

    click_where(&app, "Duplicate", |hit| hit.y > 8 && hit.y < 38);
    flash_settle(&app);
    let copied = app.wait_text(WAREHOUSE_COPY);
    see(
        &copied,
        &[
            WAREHOUSE_COPY,
            WAREHOUSE,
            "Production",
            "Duplicated as Warehouse copy",
        ],
        &["Temporary", TEST_FAIL],
    );
    click_where(&app, "Delete…", |hit| hit.y > 8 && hit.y < 38);
    flash_settle(&app);
    let ask = app.wait_text("Delete Warehouse copy?");
    see(
        &ask,
        &[
            "Delete Warehouse copy?",
            "keychain",
            "Databases are not touched.",
            WAREHOUSE,
        ],
        &["Deleted Warehouse copy"],
    );
    click_where(&app, "Delete", |hit| {
        let line = row_text(&app.screen(), u32::from(hit.y));
        line.contains("Cancel") && !line.contains("Duplicate") && hit.y > 8
    });
    flash_settle(&app);
    let deleted = app.wait_text("Deleted Warehouse copy");
    assert!(
        !list_pane(&deleted).contains(WAREHOUSE_COPY),
        "copy stayed after delete\n{}",
        list_pane(&deleted)
    );
    see(
        &deleted,
        &["Deleted Warehouse copy", WAREHOUSE, "Production", "Staging"],
        &["Delete Warehouse copy?", "keychain"],
    );

    click_where(&app, "Filter connections", |hit| hit.y > 6 && hit.x < 80);
    app.settle();
    app.type_text("Staging");
    let filtered = app.wait_screen("filter staging", |screen| {
        let pane = list_pane(screen);
        pane.contains("Staging") && !pane.contains("Local PostgreSQL")
    });
    let pane = list_pane(&filtered);
    assert!(
        pane.contains("Staging"),
        "filter hid Staging\n{pane}\n{}",
        text(&filtered)
    );
    for name in [
        WAREHOUSE,
        "Local PostgreSQL",
        "Analytics",
        "Production",
        "Scratch",
        "Development",
    ] {
        assert!(
            !pane.contains(name),
            "filter kept {name} in the list\n{pane}"
        );
    }
    app.key("ctrl+l");
    app.key("backspace");
    let cleared = app.wait_text("Local PostgreSQL");
    let pane = list_pane(&cleared);
    for name in [
        WAREHOUSE,
        "Production",
        "Staging",
        "Local PostgreSQL",
        "Scratch",
    ] {
        assert!(
            pane.contains(name),
            "list missing {name} after clear\n{pane}"
        );
    }
    assert!(
        !pane.contains(WAREHOUSE_COPY),
        "copy stayed in the list\n{pane}"
    );
    click_where(&app, "Production", |hit| hit.y > 8 && hit.x < 60);
    let restored = app.wait_text(HOST);
    see(
        &restored,
        &[WAREHOUSE, "Production", "Staging", "Local PostgreSQL", HOST],
        &[
            DRAFT,
            "Temporary",
            TEST_FAIL,
            TEST_OK,
            "Edit connection",
            "Delete Warehouse copy?",
            "keychain",
        ],
    );
    flash_settle(&app);
    shot(&app, "sc.connections.connections.after");
    app.finish();
}

#[test]
fn option_group_by_environment() {
    let app = page(
        "s09-group",
        Some("connections.group_by=environment"),
        120,
        40,
    );
    app.wait_text("▾ local");
    confirm_options(&app, "▾ local", |screen| {
        assert_connection_options(screen, "environment", "on", "40", "sample", "idle", "60");
    });
    let screen = app.settle();
    see(
        &screen,
        &[
            CAP_PLAY,
            "▾ local",
            "▾ development",
            "▾ staging",
            "▾ production",
            "Local PostgreSQL",
            "sqlite",
            HOST,
            "Production",
        ],
        &[
            "▾ Personal",
            "▾ Acme",
            "Page options",
            "No connections",
            CONNECTED,
            FAILED,
        ],
    );
    shot(&app, "opt.connections.group_by");
    app.finish();
}

#[test]
fn option_meta_off() {
    let app = page("s09-meta", Some("connections.meta=off"), 120, 40);
    app.wait_text("Scratch");
    confirm_options(&app, "Scratch", |screen| {
        assert_connection_options(screen, "folder", "off", "40", "sample", "idle", "60");
    });
    let screen = app.settle();
    let scratch = row_with(&screen, "Scratch");
    let analytics = row_with(&screen, "Analytics");
    assert!(
        !scratch.contains("sqlite") && !analytics.contains("mysql"),
        "engine meta stayed on\n{scratch}\n{analytics}"
    );
    see(
        &screen,
        &[
            "▾ Personal",
            "▾ Acme",
            "PostgreSQL",
            HOST,
            "Scratch",
            "Analytics",
        ],
        &["sqlite", "mysql", "Page options", "No connections"],
    );
    shot(&app, "opt.connections.meta");
    app.finish();
}

#[test]
fn option_list_width_30() {
    let app = page("s09-width", Some("connections.list_width=30"), 120, 40);
    app.wait_text(HOST);
    confirm_options(&app, HOST, |screen| {
        assert_connection_options(screen, "folder", "on", "30", "sample", "idle", "60");
    });
    let screen = app.settle();
    assert_eq!(host_x(&screen), 70, "list share 30 did not move the detail");
    see(
        &screen,
        &[
            "▾ Personal",
            "▾ Acme",
            "Local PostgreS",
            "Scratch",
            HOST,
            "Safe Mode",
        ],
        &[
            "Page options",
            "No connections",
            "environment",
            "sqlite",
            "mysql",
        ],
    );
    shot(&app, "opt.connections.list_width");
    app.finish();

    let wide = page("s09-width-50", Some("connections.list_width=50"), 120, 40);
    let screen = wide.wait_text(HOST);
    assert_eq!(host_x(&screen), 89, "list share 50 did not move the detail");
    confirm_options(&wide, HOST, |screen| {
        assert_connection_options(screen, "folder", "on", "50", "sample", "idle", "60");
    });
    wide.finish();
}

#[test]
fn option_list_empty() {
    let app = page("s09-empty", Some("connections.list=empty"), 120, 40);
    app.wait_text("No connections");
    confirm_options(&app, "No connections", |screen| {
        assert_connection_options(screen, "folder", "on", "40", "empty", "idle", "60");
    });
    let screen = app.settle();
    see(
        &screen,
        &[
            CAP_PLAY,
            "No connections",
            "Save a database to open it",
            "in one step",
            "New connection…",
            "Select a connection",
            "Its host, environment and safety settings show",
        ],
        &[
            "Filter connections",
            "Production",
            "Staging",
            "Analytics",
            "Local PostgreSQL",
            "Scratch",
            "Development",
            "▾ Personal",
            "▾ Acme",
            HOST,
            "scratch.db",
            "/Users",
            "/home",
            "Documents",
            "Desktop",
            "Page options",
        ],
    );
    shot(&app, "opt.connections.list");
    app.finish();
}

#[test]
fn option_state_connecting() {
    let app = page(
        "s09-connecting",
        Some("connections.state=connecting"),
        120,
        40,
    );
    app.wait_text("Connecting");
    confirm_options(&app, "Connecting", |screen| {
        assert_connection_options(screen, "folder", "on", "40", "sample", "connecting", "60");
    });
    assert_paused_connecting(&app.settle());
    let held = app.settle();
    assert_paused_connecting(&held);
    shot(&app, "opt.connections.state");
    app.finish();

    let app = page(
        "s09-connected",
        Some("connections.state=connected"),
        120,
        40,
    );
    let screen = app.wait_text(CONNECTED);
    see(
        &screen,
        &[CONNECTED, "1 hour ago", HOST, "▾ Personal"],
        &[FAILED, "Reconnect", "Authentication failed", "Connecting"],
    );
    assert_connect_label(&screen, "Connect");
    confirm_options(&app, CONNECTED, |screen| {
        assert_connection_options(screen, "folder", "on", "40", "sample", "connected", "60");
    });
    app.finish();

    let app = page("s09-failed", Some("connections.state=failed"), 120, 40);
    let screen = app.wait_text(FAILED);
    see(
        &screen,
        &[
            FAILED,
            "Timed out after 10 s",
            "The host may be behind a",
            "VPN.",
            "Reconnect",
            "1 hour ago",
        ],
        &[
            CONNECTED,
            "Connecting",
            "Authentication failed",
            "✓ Connected to",
        ],
    );
    assert_connect_label(&screen, "Reconnect");
    confirm_options(&app, FAILED, |screen| {
        assert_connection_options(screen, "folder", "on", "40", "sample", "failed", "60");
    });
    app.finish();
}

#[test]
fn option_form_width_52() {
    let app = page("s09-form-52", Some("connections.form_width=52"), 120, 40);
    app.wait_text(HOST);
    confirm_options(&app, HOST, |screen| {
        assert_connection_options(screen, "folder", "on", "40", "sample", "idle", "52");
    });
    open_edit(&app);
    let screen = app.settle();
    assert_eq!(frame_span(&screen, "Edit connection"), 52);
    see(
        &screen,
        &[
            "Edit connection",
            "Production",
            HOST,
            "acme_prod",
            "Test connection",
            "Cancel",
            "Save",
        ],
        &[TEST_OK, TEST_FAIL, "Saved Warehouse"],
    );
    shot(&app, "opt.connections.form_width");
    app.key("esc");
    flash_settle(&app);
    app.wait_absent("Edit connection");
    app.finish();

    let app = page("s09-form-60", None, 120, 40);
    app.wait_text(HOST);
    open_edit(&app);
    let screen = app.settle();
    assert_eq!(frame_span(&screen, "Edit connection"), 60);
    app.key("esc");
    flash_settle(&app);
    app.finish();
}
