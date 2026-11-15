//! Accounts & usage route, sections, specimens, compare levels, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s10_accountsusage/` and each case id's dots become underscores.
//!
//! Motion stays paused. The states loading specimen draws `spinner_frame(tick)`,
//! and an unpaused press flash advances that glyph between captures. Playground
//! meters are fixed percents. The 140ms press flash stays painted while motion
//! is paused, so a shot after a released click has to redraw once the deadline
//! has passed ([`flash_settle`]).

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Screens › Accounts & usage";
const PREV: &str = "Screens › Workspaces";
const NEXT: &str = "Screens › Launch cockpit";
const TITLE: &str = "Accounts & usage";
const BLURB: &str = "Accounts by provider with an inspector";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "accountsusage"];
const HEALTH: &str = "usage refreshed 2 m ago";
const OFFLINE_HEALTH: &str = "usage offline · last-good 18 m ago";
const WORK_WARN: &str = "Opus weekly is at 86 %";
const WORK_DETAIL: &str = "Opus sessions pause at 100 % until Monday";
const WORK_UNTIL: &str = "09:00; other models keep working.";
const KEY_REVOKED: &str = "The API key was revoked";
const KEY_DETAIL: &str = "OpenAI answered 401 invalid_api_key at";
const RESEARCH_STATUS: &str = "validation failed";
const RESEARCH_401: &str = "401 invalid";
const REFRESHED: &str = "Refreshed usage for Work ✓";
const OFFLINE_REFRESH: &str = "Offline: usage keeps its last-good values";
const PERSONAL_DEFAULT: &str = "Personal is now the default account";
const COPIED: &str = "Copied Credential";
const LINE_METER: &str = "━━━──────";
const TAB_DETAILS: &str = "Tab shows details";

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focused",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "editing",
    "empty",
];

const DRAWN: &[&str] = &["rest", "loading", "error", "overflow", "narrow"];

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s10_accountsusage/{}",
        case_id.replace('.', "_")
    )
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
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
    let line = row_with(screen, label);
    let at = line.find(label).expect("label");
    let tail = &line[at + label.len()..];
    let other = if state == "on" { "off" } else { "on" };
    assert!(
        tail.contains(state) && !tail.contains(other),
        "{label} row is {line:?}, expected {state}"
    );
}

fn assert_account_options(
    screen: &TerminalSnapshot,
    width: &str,
    visual: &str,
    offline: &str,
    warnings: &str,
) {
    assert_one(screen, "Width", width, &["fill", "72"]);
    assert_one(screen, "Quota meter visual", visual, &["block", "line"]);
    assert_toggle(screen, "Usage offline (last-good)", offline);
    assert_toggle(screen, "Account warnings", warnings);
}

fn assert_defaults(screen: &TerminalSnapshot) {
    assert_account_options(screen, "fill", "block", "off", "on");
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

fn assert_omitted(screen: &TerminalSnapshot) {
    for label in OMITTED {
        assert!(
            !grid_label(screen, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(screen)
        );
    }
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

fn tree(app: &App, label: &str) {
    click_where(app, label, |hit| {
        (10..28).contains(&hit.y) && (28..60).contains(&hit.x)
    });
}

fn button(app: &App, label: &str) {
    click_where(app, label, |hit| (20..28).contains(&hit.y) && hit.x > 40);
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

fn wheel_up(app: &App, needle: &str) {
    let point = Point { x: 70, y: 18 };
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

/// Below about 100 columns the header omits the size chip, and the narrow
/// nudge waits on the terminal size instead.
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

fn assert_providers(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            "Anthropic",
            "OpenAI",
            "Google",
            "Work",
            "Personal",
            "Team",
            "Research",
            "Vertex",
            "★ default",
            "key revoked",
            "disabled",
        ],
        &[],
    );
}

fn assert_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            "quota meters",
            CAP_PLAY,
            WIDE,
            "truecolor",
            HEALTH,
            "1 needs attention",
            "3 providers",
            WORK_WARN,
            WORK_DETAIL,
            WORK_UNTIL,
            "$212.40",
            "18.2 M",
            "Refresh",
            "Set default",
            "Max 20x",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            KEY_REVOKED,
            OFFLINE_HEALTH,
            OFFLINE_REFRESH,
            COPIED,
            PERSONAL_DEFAULT,
            REFRESHED,
            "80×24",
        ],
    );
    assert_providers(screen);
    let session = row_with(screen, "Session (5 h)");
    assert!(
        !session.contains('━'),
        "block meter drew a line glyph\n{session}"
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = page("s10-route-page", None, 120, 40);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    shot(&app, "sc.accountsusage.route");
    app.finish();

    let app = open("s10-route-keys", 120, 40, &[]);
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
        "Screens › Query workbench",
        "Screens › Connections",
        "Screens › Finder",
        "Screens › Disk cleanup",
        "Screens › Activity",
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(WORK_WARN);
    see(
        &entered,
        &[CRUMB, BLURB, WORK_WARN, "$212.40", HEALTH],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, WORK_WARN, BLURB]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, WORK_WARN, "Set default"],
        &[PREV, OVERVIEW],
    );
    app.key("]");
    let next = app.wait_text(NEXT);
    see(&next, &[NEXT], &[CRUMB, WORK_WARN]);
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, WORK_WARN, BLURB]);
    if !side_has(&app, "Accounts & usage") {
        click_side(&app, "Screens");
    }
    click_side(&app, "Accounts & usage");
    let pointed = app.wait_text(WORK_WARN);
    see(
        &pointed,
        &[CRUMB, BLURB, WORK_WARN, HEALTH, "$212.40", "★ default"],
        &[OVERVIEW, "Page options", "Compare levels", PREV],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = page("s10-sections", None, 80, 24);
    let narrow_play = app.wait_text("key revoked");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            BLURB,
            "Playground",
            "Anthropic",
            "OpenAI",
            "Google",
            "★ default",
            "key revoked",
            "Vertex",
            "Opus weekly",
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
            "$212.40",
            KEY_REVOKED,
            "Quota meter visual",
        ],
    );
    assert!(
        !flat(&narrow_play).contains("80×24"),
        "narrow header drew a size chip\n{}",
        text(&narrow_play)
    );
    shot(&app, "sc.accountsusage.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_defaults(&narrow_opt);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Width", "Quota meter visual", "fill"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            WORK_WARN,
            KEY_REVOKED,
        ],
    );
    shot(&app, "sc.accountsusage.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "loading", "error", "overflow"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    assert!(
        !grid_label(&narrow_states, "narrow"),
        "narrow specimen is on the 80 by 24 states screen\n{}",
        text(&narrow_states)
    );
    assert_omitted(&narrow_states);
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "loading",
            "error",
            "overflow",
            "Quota list",
            "refresh",
            "read failed",
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
            LEGEND,
            WORK_WARN,
            "256 colors",
        ],
    );
    shot(&app, "sc.accountsusage.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
    });
    assert_size(&app, 120, 40);
    for label in DRAWN {
        assert!(
            grid_label(&wide_states, label),
            "wide states missing {label}\n{}",
            text(&wide_states)
        );
    }
    assert_omitted(&wide_states);
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
            "Quota list",
            "Account warning",
            KEY_REVOKED,
            "401 invalid_api_key",
            "read failed",
            "⠋",
            "Session and a longer name",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            WORK_WARN,
            "256 colors",
            "16 colors",
            "no color",
            HEALTH,
        ],
    );
    assert!(
        grid_row(&wide_states, "loading").contains('⠋'),
        "paused loading specimen is not the first spinner frame\n{}",
        grid_row(&wide_states, "loading")
    );
    assert!(
        grid_row(&wide_states, "error").contains("read failed"),
        "error specimen is not the static read-failed cell\n{}",
        text(&wide_states)
    );
    for (case_id, label) in [
        ("sc.accountsusage.specimen.rest", "rest"),
        ("sc.accountsusage.specimen.loading", "loading"),
        ("sc.accountsusage.specimen.error", "error"),
        ("sc.accountsusage.specimen.overflow", "overflow"),
        ("sc.accountsusage.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid before its shot\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.accountsusage.states.120x40");
    shot(&app, "sc.accountsusage.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_defaults(&wide_opt);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "fill", "block"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            WORK_WARN,
            KEY_REVOKED,
        ],
    );
    shot(&app, "sc.accountsusage.options.120x40");
    shot(&app, "sc.accountsusage.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(WORK_WARN);
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.accountsusage.playground.120x40");
    shot(&app, "sc.accountsusage.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = page("s10-compare", None, 120, 40);
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
    shot(&app, "sc.accountsusage.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    // The first 80×24 paint can still show the wide scroll, so "no color"
    // is visible before this layout settles at the top.
    let settled = app.settle();
    if !text(&settled).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.accountsusage.compare-levels.80x24");

    wheel_up(&app, "● Compare levels");
    let top = app.settle();
    see(
        &top,
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
    assert_defaults(&options);
    see(
        &options,
        &["fill", "block", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "72", KEY_REVOKED],
    );
    app.finish();
}

#[test]
fn accounts_copy_refresh_default_and_warning() {
    let app = page("s10-accounts", None, 120, 40);
    let before = app.wait_text(WORK_WARN);
    assert_size(&app, 120, 40);
    assert_playground(&before);
    see(
        &before,
        &["1Password", "Max 20x", "included"],
        &["API key", "Vertex AI"],
    );
    shot(&app, "sc.accountsusage.accounts.before");

    button(&app, "Refresh");
    let refreshed = app.wait_text(REFRESHED);
    see(
        &refreshed,
        &[REFRESHED, WORK_WARN, HEALTH],
        &[OFFLINE_REFRESH, "usage offline", COPIED, PERSONAL_DEFAULT],
    );

    click_where(&app, "Credential", |hit| hit.y > 8 && hit.x > 40);
    app.wait_text("y Copy");
    app.key("y");
    let copied = app.wait_text(COPIED);
    see(
        &copied,
        &[COPIED, "1Password", WORK_WARN],
        &[
            "Copied Status",
            "Copied Provider",
            "Copied Plan",
            OFFLINE_REFRESH,
        ],
    );

    tree(&app, "Personal");
    app.wait_text("validated 2 m ago");
    button(&app, "Set default");
    let personal = app.wait_text(PERSONAL_DEFAULT);
    let plan = row_with(&personal, "Plan");
    assert!(
        plan.contains("Pro") && !plan.contains("Max 20x"),
        "personal plan row is {plan}"
    );
    see(
        &personal,
        &[PERSONAL_DEFAULT, "local agent folder", "~/.claude"],
        &[
            "Work is now the default account",
            WORK_WARN,
            KEY_REVOKED,
            "This account is disabled",
        ],
    );

    tree(&app, "Work");
    button(&app, "Set default");
    let work = app.settle();
    see(
        &work,
        &[WORK_WARN, "★ default"],
        &["Work is now the default account", "Enter Activate"],
    );
    assert!(
        flat(&work).contains("←→ Fold") || flat(&work).contains(PERSONAL_DEFAULT),
        "disabled Set default on Work left the tree\n{}",
        text(&work)
    );

    tree(&app, "Vertex");
    let vertex = app.wait_text("This account is disabled");
    see(
        &vertex,
        &["This account is disabled", "Vertex AI", "disabled"],
        &[KEY_REVOKED, WORK_WARN, "Vertex is now the default account"],
    );
    button(&app, "Set default");
    let vertex = app.settle();
    see(
        &vertex,
        &["This account is disabled", "disabled"],
        &["Vertex is now the default account", "Enter Activate"],
    );

    tree(&app, "Research");
    flash_settle(&app);
    let research = app.wait_text(KEY_REVOKED);
    assert_size(&app, 120, 40);
    see(
        &research,
        &[
            KEY_REVOKED,
            KEY_DETAIL,
            RESEARCH_STATUS,
            "401 invalid_api_key",
            "key revoked",
            "API key",
            "usage tier 2",
            "$0.00",
            PERSONAL_DEFAULT,
        ],
        &[
            WORK_WARN,
            REFRESHED,
            COPIED,
            OFFLINE_REFRESH,
            "This account is disabled",
        ],
    );
    shot(&app, "sc.accountsusage.accounts.after");
    app.finish();

    let app = page(
        "s10-accounts-offline",
        Some("accountsusage.offline=on"),
        120,
        40,
    );
    let offline = app.wait_text(OFFLINE_HEALTH);
    see(
        &offline,
        &[OFFLINE_HEALTH, WORK_WARN, "1 needs attention"],
        &[HEALTH, OFFLINE_REFRESH, REFRESHED],
    );
    button(&app, "Refresh");
    let refreshed = app.wait_text(OFFLINE_REFRESH);
    see(
        &refreshed,
        &[OFFLINE_REFRESH, OFFLINE_HEALTH, WORK_WARN],
        &[REFRESHED, "Refreshed usage"],
    );
    app.finish();
}

#[test]
fn option_width_72() {
    let app = page("s10-width", Some("accountsusage.width=72"), 120, 40);
    app.wait_text(TAB_DETAILS);
    confirm_options(&app, |screen| {
        assert_account_options(screen, "72", "block", "off", "on");
    });
    let screen = app.wait_text(TAB_DETAILS);
    assert_size(&app, 120, 40);
    let border = row_with(&screen, "3 providers");
    assert!(
        !border.contains("inspector") && !border.contains("Work"),
        "width 72 still splits the inspector\n{border}"
    );
    see(
        &screen,
        &[
            CAP_PLAY,
            WIDE,
            TAB_DETAILS,
            "★ default",
            "key revoked",
            "disabled",
            "$212.40",
            "Anthropic",
            "OpenAI",
            "Google",
        ],
        &[
            WORK_WARN,
            KEY_REVOKED,
            "Page options",
            "Session (5 h)",
            "Quota meter visual",
        ],
    );
    shot(&app, "opt.accountsusage.width");
    app.finish();
}

#[test]
fn option_visual_line() {
    let app = page("s10-visual", Some("accountsusage.visual=line"), 120, 40);
    app.wait_text(WORK_WARN);
    confirm_options(&app, |screen| {
        assert_account_options(screen, "fill", "line", "off", "on");
    });
    let screen = app.wait_text(LINE_METER);
    assert_size(&app, 120, 40);
    let session = row_with(&screen, "Session (5 h)");
    assert!(
        session.contains(LINE_METER) && session.contains("38%"),
        "line meter did not replace the block bar\n{session}"
    );
    see(
        &screen,
        &[
            CAP_PLAY,
            WORK_WARN,
            WORK_DETAIL,
            "━━━━━━───",
            "━━━━━━━─",
            HEALTH,
            "$212.40",
        ],
        &[
            "Page options",
            KEY_REVOKED,
            OFFLINE_HEALTH,
            "Compare levels",
        ],
    );
    shot(&app, "opt.accountsusage.visual");
    app.finish();
}

#[test]
fn option_offline_on() {
    let app = page("s10-offline", Some("accountsusage.offline=on"), 120, 40);
    app.wait_text(OFFLINE_HEALTH);
    confirm_options(&app, |screen| {
        assert_account_options(screen, "fill", "block", "on", "on");
    });
    let screen = app.wait_text(OFFLINE_HEALTH);
    assert_size(&app, 120, 40);
    see(
        &screen,
        &[
            CAP_PLAY,
            OFFLINE_HEALTH,
            WORK_WARN,
            "1 needs attention",
            "$212.40",
        ],
        &[HEALTH, OFFLINE_REFRESH, REFRESHED, "Page options"],
    );
    let session = row_with(&screen, "Session (5 h)");
    assert!(
        !session.contains('━'),
        "offline block meter drew a line glyph\n{session}"
    );
    shot(&app, "opt.accountsusage.offline");
    button(&app, "Refresh");
    let refreshed = app.wait_text(OFFLINE_REFRESH);
    see(
        &refreshed,
        &[OFFLINE_REFRESH, OFFLINE_HEALTH],
        &[REFRESHED, "Refreshed usage"],
    );
    app.finish();
}

#[test]
fn option_warnings_off() {
    let app = page("s10-warnings", Some("accountsusage.warnings=off"), 120, 40);
    app.wait_text("★ default");
    confirm_options(&app, |screen| {
        assert_account_options(screen, "fill", "block", "off", "off");
    });
    let work = app.wait_text("Max 20x");
    see(
        &work,
        &["Max 20x", "86%", "★ default", HEALTH, "1Password"],
        &[
            WORK_WARN,
            WORK_DETAIL,
            WORK_UNTIL,
            KEY_REVOKED,
            "Page options",
        ],
    );
    tree(&app, "Research");
    flash_settle(&app);
    let research = app.wait_text(RESEARCH_STATUS);
    assert_size(&app, 120, 40);
    see(
        &research,
        &[
            RESEARCH_STATUS,
            RESEARCH_401,
            "key revoked",
            "API key",
            "usage tier 2",
            HEALTH,
        ],
        &[
            KEY_REVOKED,
            KEY_DETAIL,
            WORK_WARN,
            "09:12",
            "Edit the account to replace the key",
            "Page options",
        ],
    );
    shot(&app, "opt.accountsusage.warnings");
    app.finish();
}
