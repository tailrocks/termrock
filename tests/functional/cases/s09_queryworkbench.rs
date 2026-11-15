//! Query workbench route, sections, specimens, compare levels, queries, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s09_queryworkbench/` and each case id's dots become underscores.
//!
//! Motion stays paused except for the live query, which has to finish.
//! A paused Ctrl+R stays on "Running…". The 140ms press flash stays painted
//! while motion is paused, so a shot after a released click has to redraw once
//! the deadline has passed ([`flash_settle`]). A tab click marks the strip
//! active before it emits `Activated`; the page keeps one editor per document
//! and activates by document key, so a tab click shows that document's own
//! editor and result ([`tab_click_activates_that_document`]).

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Screens › Query workbench";
const TASK_RUNNER: &str = "Screens › Task runner";
const TITLE: &str = "Query workbench";
const BLURB: &str = "Explorer, query tabs, editor over results, status line";
const NARROW_BLURB: &str = "editor over result…";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "queryworkbench"];
const PENDING: &str = "Pending orders, newest first";
const FROM_ORDERS: &str = "FROM orders";
const LIMIT_25: &str = "LIMIT 25";
const FOLD: &str = "1–5 of 7";
const ROWS_META: &str = "25 rows · 15 ms";
const FINISHED: &str = "✓ 25 rows · 15 ms";
const RUNNING: &str = "Running…";
const CANCELLED: &str = "Query cancelled";
const PLAN_SCAN: &str = "Index Scan Backward on orde…";
const PLAN_INDEX: &str = "Using orders_created_at_i…";
const PLAN_META: &str = "plan · 2 ms";
const CUSTOMERS_SQL: &str = "SELECT * FROM customers";
const NOTHING: &str = "Nothing to run";
const NO_FROM: &str = "syntax error: no FROM clause";
const NOPE: &str = "relation \"nope\" does not exist";
const ORDRS: &str = "relation \"ordrs\" does not exist";
const EMPTY_BODY: &str = "The query ran; nothing matched";
const EMPTY_META: &str = "0 rows · 9 ms";

const OMITTED: &[&str] = &[
    "pressed",
    "focus + hover",
    "disabled",
    "error",
    "editing",
    "empty",
    "loading",
];

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s09_queryworkbench/{}",
        case_id.replace('.', "_")
    )
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str], motion: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Components");
    app
}

fn page(name: &str, opt: Option<&str>, cols: u16, rows: u16) -> App {
    page_motion(name, opt, cols, rows, "paused")
}

fn page_motion(name: &str, opt: Option<&str>, cols: u16, rows: u16, motion: &str) -> App {
    let mut args = PAGE_ARGS.to_vec();
    if let Some(value) = opt {
        args.extend(["--opt", value]);
    }
    open(name, cols, rows, &args, motion)
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
        .find(|(row, line)| *row >= 6 && line.contains(label))
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
    let line = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| {
            let Some(at) = line.find(label) else {
                return false;
            };
            let tail = &line[at + label.len()..];
            tail.contains(" on") || tail.contains(" off")
        })
        .unwrap_or_else(|| panic!("toggle {label:?} missing\n{}", text(screen)));
    let at = line.find(label).expect("label");
    let tail = &line[at + label.len()..];
    let other = if state == "on" { "off" } else { "on" };
    assert!(
        tail.contains(state) && !tail.contains(other),
        "{label} row is {line:?}, expected {state}"
    );
}

#[allow(clippy::too_many_arguments)]
fn assert_options(
    screen: &TerminalSnapshot,
    explorer: &str,
    width: &str,
    share: &str,
    allow_new: &str,
    close_reveal: &str,
    read_only: &str,
    result: &str,
    env: &str,
) {
    assert_toggle(screen, "Explorer", explorer);
    assert_one(screen, "Explorer %", width, &["32", "24", "40"]);
    assert_one(screen, "Editor %", share, &["40", "25", "60"]);
    assert_toggle(screen, "New-tab button", allow_new);
    assert_one(screen, "Tab close", close_reveal, &["always", "hover"]);
    assert_toggle(screen, "Read-only editor", read_only);
    assert_one(screen, "Result", result, &["run", "empty", "error", "plan"]);
    assert_one(
        screen,
        "Environment",
        env,
        &["production", "staging", "development", "local"],
    );
}

fn assert_defaults(screen: &TerminalSnapshot) {
    assert_options(
        screen,
        "on",
        "32",
        "40",
        "on",
        "always",
        "off",
        "run",
        "production",
    );
}

fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

/// Grid labels sit under the legend. The legend's "focused sample" is above y=9.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    grid_hit(screen, label).is_some()
}

fn grid_row(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = grid_hit(screen, label)
        .unwrap_or_else(|| panic!("grid label {label:?} missing\n{}", text(screen)));
    row_text(screen, u32::from(hit.y))
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

fn side_has(app: &App, label: &str) -> bool {
    let screen = app.settle();
    find_all(&screen, label).into_iter().any(|hit| hit.x < 24)
}

fn click_tree(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x < 52 && hit.y > 12);
}

fn click_tool(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 6 && hit.y < 16);
}

fn click_new_tab(app: &App) {
    click_where(app, "+", |hit| hit.y == 7);
}

fn click_rightmost_close(app: &App) {
    let screen = app.settle();
    let mut hits: Vec<_> = find_all(&screen, "×")
        .into_iter()
        .filter(|hit| hit.y == 7)
        .collect();
    assert!(
        !hits.is_empty(),
        "no tab close on the tab row\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.x);
    app.click(hits[hits.len() - 1]);
}

fn reveal_side(app: &App, label: &str) {
    let point = Point { x: 8, y: 16 };
    for _ in 0..30 {
        if side_has(app, label) {
            return;
        }
        app.wheel(point, false, 3);
    }
    for _ in 0..30 {
        if side_has(app, label) {
            return;
        }
        app.wheel(point, true, 3);
    }
}

fn reveal_text(app: &App, needle: &str) {
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, true, 3);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 3);
    }
    app.wait_text(needle);
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

fn assert_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Query 1",
            "customers",
            PENDING,
            FROM_ORDERS,
            FOLD,
            "Northwind",
            "10480",
            "pending",
            ROWS_META,
            "Filter objects",
            "organizations",
            "products",
            "orders",
            "order_items",
            "payments",
            "subscriptions",
            "Run Ctrl+R",
            "Explain",
            "Stop",
            "Clear",
            "production",
            "safe mode",
            "2 tabs",
            "acme_prod › public",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            RUNNING,
            CANCELLED,
            FINISHED,
            NOTHING,
            NO_FROM,
            "48k",
            "1.2M",
            LIMIT_25,
            "1–2 of 7",
            "read-only",
            "Not run yet",
        ],
    );
}

fn assert_omitted(screen: &TerminalSnapshot) {
    for label in OMITTED {
        assert!(
            !grid_label(screen, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(screen)
        );
    }
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

fn go_to_workbench(app: &App) {
    if !side_has(app, "Query workbench") {
        click_side(app, "Screens");
    }
    click_side(app, "Query workbench");
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = page("s09-route-page", None, 120, 40);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    shot(&app, "sc.queryworkbench.route");
    app.finish();

    let app = open("s09-route-keys", 120, 40, &[], "paused");
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
        "Feedback › Progress",
        "Feedback › Empty & error",
        "Feedback › Meters",
        "Feedback › Notices",
        "Layout › Panels",
        "Layout › Scrolling",
        "Layout › Splits & stacks",
        "Layout › Master-detail",
        "Layout › Pane tree",
        "Text & code › Code editor",
        "Text & code › Diff",
        "Text & code › Terminal",
        "Text & code › Text utilities",
        "Screens › Settings",
        TASK_RUNNER,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(FROM_ORDERS);
    see(
        &entered,
        &[CRUMB, BLURB, PENDING, FROM_ORDERS, "Northwind", ROWS_META],
        &[OVERVIEW, "Page options", TASK_RUNNER, FINISHED, RUNNING],
    );
    app.key("[");
    let left = app.wait_text(TASK_RUNNER);
    see(
        &left,
        &[TASK_RUNNER],
        &[CRUMB, PENDING, FROM_ORDERS, "Northwind"],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, FROM_ORDERS, "Query 1", "2 tabs"],
        &[TASK_RUNNER, OVERVIEW, FINISHED],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, FROM_ORDERS, BLURB]);
    go_to_workbench(&app);
    let pointed = app.wait_text(FROM_ORDERS);
    see(
        &pointed,
        &[CRUMB, BLURB, PENDING, FROM_ORDERS, "Northwind", "safe mode"],
        &[OVERVIEW, "Page options", "Compare levels", FINISHED],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = page("s09-sections", None, 80, 24);
    let narrow_play = app.wait_text("Northwind");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            NARROW_BLURB,
            "Playground",
            "Query 1",
            "customers",
            PENDING,
            "Northwind",
            "10480",
            "pending",
            ROWS_META,
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
            "status line",
            "Filter objects",
            FROM_ORDERS,
            LIMIT_25,
            FOLD,
            RUNNING,
            FINISHED,
        ],
    );
    shot(&app, "sc.queryworkbench.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_defaults(&narrow_opt);
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Explorer",
            "New-tab button",
            "Environment",
            "production",
            "Read-only editor",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            FROM_ORDERS,
            WIDE,
            "truecolor",
            "80×24",
            "Northwind",
        ],
    );
    shot(&app, "sc.queryworkbench.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "hover", "focused", "selected"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    for label in ["busy", "overflow", "narrow"] {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is on the 80 by 24 states screen\n{}",
            text(&narrow_states)
        );
    }
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "focused",
            "selected",
            "Query 1",
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
            "48k",
            "1.2M",
            FROM_ORDERS,
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.queryworkbench.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
    });
    assert_size(&app, 120, 40);
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
            "48k",
            "1.2M",
            "Query tabs",
            "Explorer",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            FROM_ORDERS,
            "256 colors",
            "Northwind",
        ],
    );
    assert!(
        grid_row(&wide_states, "focused").contains('▏'),
        "focused specimen has no bar\n{}",
        text(&wide_states)
    );
    assert!(
        !grid_row(&wide_states, "rest").contains('▏'),
        "rest specimen draws the focused bar\n{}",
        text(&wide_states)
    );
    assert!(
        grid_row(&wide_states, "selected").contains('–'),
        "selected specimen does not draw the tabs dash\n{}",
        text(&wide_states)
    );
    assert_omitted(&wide_states);
    for (case_id, label) in [
        ("sc.queryworkbench.specimen.rest", "rest"),
        ("sc.queryworkbench.specimen.hover", "hover"),
        ("sc.queryworkbench.specimen.focused", "focused"),
        ("sc.queryworkbench.specimen.selected", "selected"),
        ("sc.queryworkbench.specimen.busy", "busy"),
        ("sc.queryworkbench.specimen.overflow", "overflow"),
        ("sc.queryworkbench.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.queryworkbench.states.120x40");
    shot(&app, "sc.queryworkbench.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_defaults(&wide_opt);
    see(
        &wide_opt,
        &[
            CRUMB,
            CAP_OPT,
            WIDE,
            "Page options",
            "Environment",
            "production",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            FROM_ORDERS,
            "Northwind",
        ],
    );
    shot(&app, "sc.queryworkbench.options.120x40");
    shot(&app, "sc.queryworkbench.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(FROM_ORDERS);
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.queryworkbench.playground.120x40");
    shot(&app, "sc.queryworkbench.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = page("s09-compare", None, 120, 40);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[
            CRUMB,
            "○ Compare levels",
            "rest",
            "narrow",
            CAP_STATES,
            "48k",
        ],
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
        &[CRUMB, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
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
    shot(&app, "sc.queryworkbench.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    // The first paint after a shrink can still show the wide scroll offset.
    // Decide from the settled frame, then keep the pointer on the same wheel.
    let mut narrow = app.settle();
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
        narrow = app.settle();
    }
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels", "80×24"],
    );
    shot(&app, "sc.queryworkbench.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    let top = app.settle();
    see(
        &top,
        &[TITLE, "truecolor", "● Compare levels"],
        &["no color", "Page options"],
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
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_defaults(&options);
    see(
        &options,
        &[
            "32",
            "40",
            "Page options",
            CAP_OPT,
            CRUMB,
            "production",
            "always",
        ],
        &["Compare levels", OVERVIEW, "staging", "read-only"],
    );
    app.finish();
}

fn type_query(app: &App, sql: &str) {
    click_where(app, "Write a query", |hit| hit.x > 40);
    app.type_text(sql);
    app.wait_text(sql);
}

fn expand_views(app: &App) {
    click_tree(app, "Views");
    if !text(&app.settle()).contains("active_customers") {
        app.key("enter");
    }
    // The explorer column clips the lazy names to active_custo… and monthly_reve….
    app.wait_text("active_custo");
    app.wait_text("monthly_reve");
}

#[test]
fn tab_click_activates_that_document() {
    let app = page_motion("s09-tab-click-document", None, 120, 40, "full");
    app.wait_text(FROM_ORDERS);
    click_where(&app, "customers", |hit| hit.y == 7);
    let customers = app.wait_text(CUSTOMERS_SQL);
    see(
        &customers,
        &[CUSTOMERS_SQL, "Not run yet", "2 tabs"],
        &[FROM_ORDERS, PENDING, "Northwind"],
    );

    // Run belongs to the document the strip shows, not the one loaded before.
    app.key("ctrl+r");
    let ran = app.wait_text("✓ 40 rows");
    see(
        &ran,
        &[CUSTOMERS_SQL, "✓ 40 rows"],
        &[FROM_ORDERS, PENDING, "Not run yet"],
    );

    click_where(&app, "Query 1", |hit| hit.y == 7);
    let query1 = app.wait_text(FROM_ORDERS);
    see(
        &query1,
        &[FROM_ORDERS, PENDING, ROWS_META, "2 tabs"],
        &[CUSTOMERS_SQL],
    );
    app.finish();
}

fn click_leftmost_close(app: &App) {
    let screen = app.settle();
    let mut hits: Vec<_> = find_all(&screen, "×")
        .into_iter()
        .filter(|hit| hit.y == 7)
        .collect();
    assert!(
        !hits.is_empty(),
        "no tab close on the tab row\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.x);
    app.click(hits[0]);
}

#[test]
fn issue53_completion_stays_on_b_after_a_closes() {
    let app = page_motion("s09-issue53-close-earlier", None, 120, 40, "full");
    app.wait_text(FROM_ORDERS);
    click_new_tab(&app);
    app.wait_text("Query 2");
    type_query(&app, "SELECT * FROM orders LIMIT 25;");
    click_where(&app, "customers", |hit| hit.y == 7);
    app.wait_text(CUSTOMERS_SQL);

    app.key("ctrl+r");
    app.wait_text(RUNNING);
    click_leftmost_close(&app);
    app.wait_text("Closed Query 1");
    let done = app.wait_text("40 rows · 15 ms");
    see(
        &done,
        &[
            CUSTOMERS_SQL,
            "40 rows · 15 ms",
            "customers",
            "Query 2",
            "2 tabs",
        ],
        &[FROM_ORDERS, PENDING, "≡ Query 1", FINISHED, CANCELLED],
    );

    click_where(&app, "Query 2", |hit| hit.y == 7);
    let other = app.wait_text("SELECT * FROM orders LIMIT 25;");
    see(
        &other,
        &["Not run yet", "2 tabs"],
        &[CUSTOMERS_SQL, "40 rows", ROWS_META],
    );
    app.finish();
}

#[test]
fn issue53_inactive_completion_does_not_steal_focus() {
    let app = page_motion("s09-issue53-inactive", None, 120, 40, "full");
    app.wait_text(FROM_ORDERS);
    click_new_tab(&app);
    app.wait_text("Query 2");
    click_where(&app, "customers", |hit| hit.y == 7);
    app.wait_text(CUSTOMERS_SQL);

    app.key("ctrl+r");
    click_where(&app, "Query 2", |hit| hit.y == 7);
    let done = app.wait_text("customers: ✓ 40 rows");
    see(
        &done,
        &["Query 2", "Not run yet", "Write a query", "3 tabs"],
        &[CUSTOMERS_SQL, ROWS_META, CANCELLED],
    );

    click_where(&app, "customers", |hit| hit.y == 7);
    let b = app.wait_text(CUSTOMERS_SQL);
    see(&b, &["40 rows · 15 ms", CUSTOMERS_SQL], &["Not run yet"]);
    app.finish();
}

#[test]
fn queries_keep_their_own_sql_and_result() {
    let app = page("s09-queries", None, 120, 40);
    let before = app.wait_text(FROM_ORDERS);
    assert_size(&app, 120, 40);
    assert_playground(&before);
    shot(&app, "sc.queryworkbench.queries.before");

    app.key("ctrl+r");
    let running = app.wait_text(RUNNING);
    see(
        &running,
        &[
            CRUMB,
            RUNNING,
            "running",
            "Running",
            FROM_ORDERS,
            PENDING,
            "Query 1",
            "2 tabs",
        ],
        &[
            "Northwind",
            "10480",
            FINISHED,
            CANCELLED,
            ROWS_META,
            NOTHING,
            "Not run yet",
        ],
    );
    shot(&app, "sc.queryworkbench.queries.running");

    click_tool(&app, "Stop");
    let cancelled = app.wait_text(CANCELLED);
    see(
        &cancelled,
        &[
            CANCELLED,
            "Northwind",
            "10480",
            ROWS_META,
            FROM_ORDERS,
            PENDING,
            "2 tabs",
        ],
        &[RUNNING, FINISHED, "running", NOTHING],
    );

    click_tree(&app, "customers");
    app.key("enter");
    let opened = app.wait_text("Opened customers");
    see(
        &opened,
        &[
            "Opened customers",
            CUSTOMERS_SQL,
            "LIMIT 100",
            "Not run yet",
            "customers",
            "2 tabs",
        ],
        &[
            "Northwind",
            FROM_ORDERS,
            PENDING,
            ROWS_META,
            FINISHED,
            RUNNING,
        ],
    );
    click_rightmost_close(&app);
    let restored = app.wait_text("Closed customers");
    see(
        &restored,
        &[
            "Closed customers",
            FROM_ORDERS,
            PENDING,
            "Northwind",
            ROWS_META,
            "Query 1",
            "1 tab",
        ],
        &[
            CUSTOMERS_SQL,
            "Not run yet",
            "Opened customers",
            FINISHED,
            RUNNING,
        ],
    );

    click_new_tab(&app);
    app.wait_text("Query 2");
    app.key("ctrl+r");
    let empty = app.wait_text(NOTHING);
    see(
        &empty,
        &[NOTHING, "Query 2", "Write a query", "Not run yet", "2 tabs"],
        &[FROM_ORDERS, "Northwind", FINISHED, RUNNING, NO_FROM],
    );
    type_query(&app, "SELECT 1;");
    click_tool(&app, "Clear");
    let cleared = app.wait_text("Write a query");
    see(
        &cleared,
        &["Write a query", "Query 2", NOTHING, "Not run yet"],
        &["SELECT 1;", FROM_ORDERS, FINISHED],
    );
    app.finish();

    let app = page_motion("s09-queries-live", None, 120, 40, "full");
    app.wait_text(FROM_ORDERS);
    expand_views(&app);
    let views = app.settle();
    see(
        &views,
        &["active_custo", "monthly_reve", "Views", FROM_ORDERS],
        &[NO_FROM, NOPE, FINISHED],
    );

    click_new_tab(&app);
    app.wait_text("Query 2");
    type_query(&app, "SELECT 1;");
    app.key("ctrl+r");
    let syntax = app.wait_text(NO_FROM);
    see(
        &syntax,
        &[NO_FROM, "SQLSTATE 42601", "SELECT 1;", "Query 2"],
        &["Northwind", FROM_ORDERS, FINISHED, CANCELLED, "relation \""],
    );

    click_new_tab(&app);
    app.wait_text("Query 3");
    type_query(&app, "SELECT * FROM nope LIMIT 1;");
    app.key("ctrl+r");
    let missing = app.wait_text("does not exist");
    see(
        &missing,
        &[NOPE, "SQLSTATE 42P01", "nope", "Query 3", "4 tabs"],
        &["Northwind", FINISHED, CANCELLED, NO_FROM, ORDRS],
    );

    click_rightmost_close(&app);
    app.wait_text("Closed Query 3");
    click_rightmost_close(&app);
    app.wait_text("Closed Query 2");
    click_rightmost_close(&app);
    let orders = app.wait_text(FROM_ORDERS);
    see(
        &orders,
        &[FROM_ORDERS, PENDING, "Northwind", "Query 1", "1 tab"],
        &[NOPE, NO_FROM, "Query 2", "Query 3", CUSTOMERS_SQL],
    );

    click_tool(&app, "Explain");
    let plan = app.wait_text("Plan ready");
    see(
        &plan,
        &[
            "✓ Plan ready",
            "Plan",
            PLAN_META,
            PLAN_SCAN,
            PLAN_INDEX,
            FROM_ORDERS,
        ],
        &["Northwind", FINISHED, RUNNING, CANCELLED, NO_FROM],
    );
    app.key("ctrl+r");
    let done = app.wait_text(FINISHED);
    see(
        &done,
        &[
            FINISHED,
            "Northwind",
            "10480",
            ROWS_META,
            FROM_ORDERS,
            PENDING,
            "1 tab",
            "active_custo",
        ],
        &[
            RUNNING,
            CANCELLED,
            "Plan ready",
            PLAN_META,
            NO_FROM,
            NOPE,
            NOTHING,
        ],
    );
    assert_size(&app, 120, 40);
    shot(&app, "sc.queryworkbench.queries.after");
    app.finish();
}

#[test]
fn option_explorer_off() {
    let app = page("s09-explorer", Some("queryworkbench.explorer=off"), 120, 40);
    app.wait_text(FROM_ORDERS);
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "off",
            "32",
            "40",
            "on",
            "always",
            "off",
            "run",
            "production",
        );
    });
    finish_option(&app, "opt.queryworkbench.explorer", |screen| {
        see(
            screen,
            &[
                FROM_ORDERS,
                PENDING,
                "Northwind",
                "created_at",
                "2026-10-17 23:00",
                "10410",
                ROWS_META,
            ],
            &["Filter objects", "organizations", "subscriptions", "48k"],
        );
    });
}

#[test]
fn option_explorer_width_24() {
    let app = page(
        "s09-explorer-width",
        Some("queryworkbench.explorer_width=24"),
        120,
        40,
    );
    app.wait_text(FROM_ORDERS);
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "on",
            "24",
            "40",
            "on",
            "always",
            "off",
            "run",
            "production",
        );
    });
    finish_option(&app, "opt.queryworkbench.explorer_width", |screen| {
        see(
            screen,
            &[
                "T organ…",
                "T custo…",
                "T orders",
                FROM_ORDERS,
                PENDING,
                FOLD,
                "Northwind",
            ],
            &["organizations", "48k", "1.2M", "1.2k", "subscriptions"],
        );
    });
}

#[test]
fn option_editor_share() {
    let folded = page(
        "s09-share-25",
        Some("queryworkbench.editor_share=25"),
        120,
        40,
    );
    folded.wait_text(PENDING);
    confirm_options(&folded, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "25",
            "on",
            "always",
            "off",
            "run",
            "production",
        );
    });
    let short = folded.settle();
    see(
        &short,
        &["1–2 of 7", "10410", PENDING, "Northwind", ROWS_META],
        &[FROM_ORDERS, LIMIT_25, FOLD],
    );
    folded.finish();

    let tall = page(
        "s09-share-60",
        Some("queryworkbench.editor_share=60"),
        120,
        40,
    );
    tall.wait_text(LIMIT_25);
    confirm_options(&tall, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "60",
            "on",
            "always",
            "off",
            "run",
            "production",
        );
    });
    finish_option(&tall, "opt.queryworkbench.editor_share", |screen| {
        see(
            screen,
            &["6 LIMIT 25;", FROM_ORDERS, PENDING, "Query 1"],
            &[FOLD, "1–2 of 7", "10410"],
        );
    });
}

#[test]
fn option_allow_new_off() {
    let app = page(
        "s09-allow-new",
        Some("queryworkbench.allow_new=off"),
        120,
        40,
    );
    app.wait_text(FROM_ORDERS);
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "40",
            "off",
            "always",
            "off",
            "run",
            "production",
        );
    });
    finish_option(&app, "opt.queryworkbench.allow_new", |screen| {
        see(
            screen,
            &[FROM_ORDERS, "Query 1", "customers", "×", "Northwind"],
            &[],
        );
        let plus: Vec<_> = find_all(screen, "+")
            .into_iter()
            .filter(|hit| hit.y == 7)
            .collect();
        assert!(
            plus.is_empty(),
            "new-tab button is still drawn: {plus:?}\n{}",
            text(screen)
        );
    });
}

#[test]
fn option_close_reveal_hover() {
    let app = page(
        "s09-close",
        Some("queryworkbench.close_reveal=hover"),
        120,
        40,
    );
    app.wait_text(FROM_ORDERS);
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "40",
            "on",
            "hover",
            "off",
            "run",
            "production",
        );
    });
    finish_option(&app, "opt.queryworkbench.close_reveal", |screen| {
        see(
            screen,
            &[FROM_ORDERS, "Query 1", "customers", "Northwind", "+"],
            &[],
        );
        let closes: Vec<_> = find_all(screen, "×")
            .into_iter()
            .filter(|hit| hit.y == 7)
            .collect();
        assert_eq!(
            closes.len(),
            1,
            "hover close should keep one × on the active tab: {closes:?}\n{}",
            text(screen)
        );
        let tab = row_with(screen, "customers");
        assert!(
            tab.contains("Query 1") && tab.contains('×') && tab.contains("customers"),
            "tab row {tab:?}"
        );
    });
}

#[test]
fn option_read_only() {
    let app = page(
        "s09-read-only",
        Some("queryworkbench.read_only=on"),
        120,
        40,
    );
    app.wait_text("read-only");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "40",
            "on",
            "always",
            "on",
            "run",
            "production",
        );
    });
    finish_option(&app, "opt.queryworkbench.read_only", |screen| {
        see(
            screen,
            &[
                "read-only",
                FROM_ORDERS,
                PENDING,
                "Northwind",
                ROWS_META,
                "Query 1",
            ],
            &["6 lines"],
        );
    });
}

#[test]
fn option_result_bodies() {
    let empty = page(
        "s09-result-empty",
        Some("queryworkbench.result=empty"),
        120,
        40,
    );
    empty.wait_text(EMPTY_BODY);
    confirm_options(&empty, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "40",
            "on",
            "always",
            "off",
            "empty",
            "production",
        );
    });
    let empty_body = empty.settle();
    see(
        &empty_body,
        &[EMPTY_META, "0 rows", EMPTY_BODY, FROM_ORDERS],
        &["Northwind", "10480", ROWS_META, "25 rows", ORDRS, "Plan"],
    );
    empty.finish();

    let plan = page(
        "s09-result-plan",
        Some("queryworkbench.result=plan"),
        120,
        40,
    );
    plan.wait_text(PLAN_SCAN);
    confirm_options(&plan, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "40",
            "on",
            "always",
            "off",
            "plan",
            "production",
        );
    });
    let plan_body = plan.settle();
    see(
        &plan_body,
        &["Plan", PLAN_META, PLAN_SCAN, PLAN_INDEX, FROM_ORDERS],
        &["Northwind", ROWS_META, EMPTY_BODY, ORDRS, FINISHED],
    );
    plan.finish();

    let error = page(
        "s09-result-error",
        Some("queryworkbench.result=error"),
        120,
        40,
    );
    error.wait_text(ORDRS);
    confirm_options(&error, |screen| {
        assert_options(
            screen,
            "on",
            "32",
            "40",
            "on",
            "always",
            "off",
            "error",
            "production",
        );
    });
    finish_option(&error, "opt.queryworkbench.result", |screen| {
        see(
            screen,
            &[
                ORDRS,
                "SQLSTATE 42P01",
                "Did you mean \"orders\"?",
                "unknown relation",
                "failed",
                FROM_ORDERS,
            ],
            &["Northwind", ROWS_META, EMPTY_BODY, PLAN_META, "0 rows"],
        );
    });
}

#[test]
fn option_env() {
    let local = page("s09-env-local", Some("queryworkbench.env=local"), 120, 40);
    local.wait_text("writes allowed");
    confirm_options(&local, |screen| {
        assert_options(
            screen, "on", "32", "40", "on", "always", "off", "run", "local",
        );
    });
    let local_body = local.settle();
    see(
        &local_body,
        &[
            "local",
            "writes allowed",
            FROM_ORDERS,
            "Northwind",
            "acme_prod › public",
        ],
        &["production", "staging", "safe mode", "development"],
    );
    local.finish();

    let staging = page(
        "s09-env-staging",
        Some("queryworkbench.env=staging"),
        120,
        40,
    );
    staging.wait_text("staging");
    confirm_options(&staging, |screen| {
        assert_options(
            screen, "on", "32", "40", "on", "always", "off", "run", "staging",
        );
    });
    finish_option(&staging, "opt.queryworkbench.env", |screen| {
        see(
            screen,
            &[
                "staging",
                "writes allowed",
                FROM_ORDERS,
                "Northwind",
                ROWS_META,
                "acme_prod › public",
            ],
            &["production", "safe mode", "local", "development"],
        );
    });
}
