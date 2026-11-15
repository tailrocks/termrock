//! Notices route, sections, specimens, compare levels, connection outcomes, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s06_notices/` and each case id's dots become underscores.

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Feedback › Notices";
const TITLE: &str = "Notices";
const BLURB: &str =
    "Notice: inline error, warning, information and success blocks with a wrapped detail";
const OVERVIEW: &str = "Tokens and principles behind every component";
const METERS: &str = "Feedback › Meters";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const PAGE_ARGS: &[&str] = &["--page", "notices"];
const MARK: &str = "Mark the cut …";
const AUTH: &str = "Authentication failed";
const PROMPT: &str = "Prompt for password";
const RETRIES: &str = "Test connection retries";
const SYNTAX: &str = "syntax error at or near \"FORM\"";
const TIMED_OUT: &str = "Connection timed out";
const BLOCKED: &str = "port may be blocked.";
const HOST: &str = "Could not reach the host";
const CONNECTED: &str = "Connected in 182 ms";
const SCHEMAS: &str = "PostgreSQL 16.4";
const SAVE: &str = "Save keeps the connection";

#[derive(Clone, Copy)]
struct Controls {
    kind: &'static str,
    detail: &'static str,
    hint: &'static str,
    lines: &'static str,
    message: &'static str,
    tone: &'static str,
    mark: &'static str,
    width: &'static str,
}

const CONTROLS: Controls = Controls {
    kind: "error",
    detail: "on",
    hint: "on",
    lines: "2",
    message: "2",
    tone: "secondary",
    mark: "on",
    width: "wide",
};

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s06_notices/{}", case_id.replace('.', "_"))
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

fn assert_choice(screen: &TerminalSnapshot, label: &str, value: &str, all: &[&str]) {
    let others: Vec<&str> = all.iter().copied().filter(|item| *item != value).collect();
    assert_select(screen, label, value, &others);
}

fn assert_controls(screen: &TerminalSnapshot, controls: Controls) {
    assert_choice(
        screen,
        "Result notice kind",
        controls.kind,
        &["error", "warning", "info", "success"],
    );
    assert_choice(screen, "Detail lines", controls.lines, &["2", "1", "4"]);
    assert_choice(screen, "Message lines", controls.message, &["2", "1"]);
    assert_choice(
        screen,
        "Detail tone",
        controls.tone,
        &["secondary", "muted"],
    );
    assert_choice(screen, "Width", controls.width, &["wide", "narrow"]);
    assert_toggle(screen, "Detail", controls.detail);
    assert_toggle(screen, "Hint", controls.hint);
    assert_toggle(screen, MARK, controls.mark);
}

/// Specimen labels sit in the grid's label column. The legend contains
/// "focused", and the blurb contains "error", so a raw text search is wrong.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 9 && (18..36).contains(&hit.x))
}

fn grid_row(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = find_all(screen, label)
        .into_iter()
        .find(|hit| hit.y >= 9 && (18..36).contains(&hit.x))
        .unwrap_or_else(|| panic!("grid row {label:?} missing\n{}", text(screen)));
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
    click_where(app, label, |hit| hit.x < 24);
}

fn sidebar_has(app: &App, label: &str) -> bool {
    find_all(&app.screen(), label)
        .into_iter()
        .any(|hit| hit.x < 24)
}

fn click_test(app: &App) {
    let screen = app.wait_text("Test connection");
    let hits: Vec<_> = find_all(&screen, "Test connection")
        .into_iter()
        .filter(|hit| {
            hit.x >= 24
                && (6..38).contains(&hit.y)
                && !row_text(&screen, u32::from(hit.y)).contains("retries")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected the Test connection button, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn ink(screen: &TerminalSnapshot, needle: &str) -> SnapshotColor {
    let hit = find_all(screen, needle)
        .into_iter()
        .find(|hit| hit.x >= 24 && hit.y > 6)
        .unwrap_or_else(|| panic!("{needle:?} missing\n{}", text(screen)));
    screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .unwrap_or_else(|| panic!("no cell at {hit:?}"))
        .fg()
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

fn scroll_to_no_color(app: &App, saw256: &mut bool, saw16: &mut bool) {
    let body = Point { x: 70, y: 16 };
    for _ in 0..40 {
        let visible = text(&app.settle());
        *saw256 |= visible.contains("256 colors");
        *saw16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            return;
        }
        app.wheel(body, true, 3);
    }
    panic!(
        "no color band stayed below the fold\n{}",
        text(&app.screen())
    );
}

fn option_case(
    name: &str,
    case_id: &str,
    opt: &str,
    controls: Controls,
    present: &[&str],
    absent: &[&str],
    play_check: impl Fn(&TerminalSnapshot),
) {
    let app = page(name, Some(opt), 120, 40);
    let play = app.wait_text("Test connection");
    see(&play, present, absent);
    play_check(&play);
    click_tab(&app, "Options");
    let options = app.wait_text("Result notice kind");
    assert_controls(&options, controls);
    click_tab(&app, "Playground");
    app.wait_text("Test connection");
    flash_settle(&app);
    shot(&app, case_id);
    app.finish();
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s06-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    see(
        &screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            AUTH,
            "FATAL",
            PROMPT,
            RETRIES,
            "Test connection",
            SYNTAX,
            "Port must be a number between 1 and 65535",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            HOST,
            CONNECTED,
        ],
    );
    shot(&app, "sc.notices.route");
    app.finish();

    let app = open("s06-route-keys", 120, 40, &[]);
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
        METERS,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(AUTH);
    see(
        &entered,
        &[CRUMB, BLURB, AUTH, "Test connection"],
        &[OVERVIEW, "Page options", METERS],
    );
    app.key("[");
    let left = app.wait_text(METERS);
    see(&left, &[METERS], &[AUTH, BLURB, "Test connection"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, AUTH, RETRIES], &[METERS, OVERVIEW]);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[AUTH, BLURB]);
    if !sidebar_has(&app, "Notices") {
        click_side(&app, "Feedback");
        app.wait_text("Notices");
    }
    click_side(&app, "Notices");
    let pointed = app.wait_text(AUTH);
    see(
        &pointed,
        &[CRUMB, TITLE, BLURB, AUTH, CAP_PLAY],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s06-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(AUTH);
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "information and s…",
            AUTH,
            PROMPT,
            RETRIES,
            "Test connection",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "wrapped detail",
            SYNTAX,
        ],
    );
    shot(&app, "sc.notices.playground.80x24");

    click_tab(&app, "Options");
    flash_settle(&app);
    let narrow_opt = app.wait_text("Result notice kind");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Result notice kind",
            "Detail lines",
            "Message lines",
            "Detail tone",
            MARK,
            "Width",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            AUTH,
            "wrapped detail",
        ],
    );
    assert_eq!((narrow_opt.cols(), narrow_opt.rows()), (80, 24));
    assert_controls(&narrow_opt, CONTROLS);
    shot(&app, "sc.notices.options.80x24");

    click_tab(&app, "States");
    flash_settle(&app);
    let narrow_states = app.wait_text("Compare levels");
    assert!(
        grid_label(&narrow_states, "rest"),
        "rest\n{}",
        text(&narrow_states)
    );
    assert!(
        grid_label(&narrow_states, "error"),
        "error\n{}",
        text(&narrow_states)
    );
    assert!(
        grid_label(&narrow_states, "overflow"),
        "overflow\n{}",
        text(&narrow_states)
    );
    for absent in [
        "narrow", "hover", "pressed", "focused", "selected", "disabled", "busy", "editing",
        "empty", "loading",
    ] {
        assert!(
            !grid_label(&narrow_states, absent),
            "{absent} is a grid label\n{}",
            text(&narrow_states)
        );
    }
    see(
        &narrow_states,
        &[TITLE, "States", "○ Compare levels", "far too…"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            WIDE,
            "truecolor",
            "● Compare levels",
            "wrapped detail",
        ],
    );
    assert_eq!((narrow_states.cols(), narrow_states.rows()), (80, 24));
    shot(&app, "sc.notices.states.80x24");

    app.resize(120, 40);
    // The narrow specimen label paints before its hint is cut to "kee…".
    let wide_states = app.wait_screen("states wide", |screen| {
        let visible = text(screen);
        visible.contains(WIDE)
            && visible.contains(CAP_STATES)
            && grid_label(screen, "narrow")
            && visible.contains("kee…")
    });
    assert!(grid_label(&wide_states, "rest"));
    assert!(grid_label(&wide_states, "error"));
    assert!(grid_label(&wide_states, "overflow"));
    assert!(grid_label(&wide_states, "narrow"));
    for absent in [
        "hover", "pressed", "focused", "selected", "disabled", "busy", "editing", "empty",
        "loading",
    ] {
        assert!(
            !grid_label(&wide_states, absent),
            "{absent} is a grid label\n{}",
            text(&wide_states)
        );
    }
    let rest = grid_row(&wide_states, "rest");
    assert!(rest.contains(AUTH), "rest row {rest:?}");
    assert!(rest.contains('▲'), "rest hint column {rest:?}");
    shot(&app, "sc.notices.specimen.rest");
    let error = grid_row(&wide_states, "error");
    assert!(error.contains("Disk almost full"), "error row {error:?}");
    assert!(error.contains('!'), "error row {error:?}");
    assert!(
        !error.contains('▲'),
        "error row kept the warning mark {error:?}"
    );
    shot(&app, "sc.notices.specimen.error");
    let overflow = grid_row(&wide_states, "overflow");
    assert!(
        overflow.contains("far too…") || text(&wide_states).contains("far too…"),
        "overflow cut\n{}",
        text(&wide_states)
    );
    shot(&app, "sc.notices.specimen.overflow");
    let narrow = grid_row(&wide_states, "narrow");
    assert!(narrow.contains("Authentication"), "narrow row {narrow:?}");
    assert!(
        !narrow.contains("failed"),
        "narrow row did not wrap {narrow:?}"
    );
    assert!(
        text(&wide_states).contains("kee…"),
        "narrow hint cut\n{}",
        text(&wide_states)
    );
    shot(&app, "sc.notices.specimen.narrow");
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "With detail",
            "With hint",
        ],
        &[CAP_PLAY, CAP_OPT, "Page options", "● Compare levels"],
    );
    shot(&app, "sc.notices.states.120x40");
    shot(&app, "sc.notices.states");

    click_tab(&app, "Options");
    flash_settle(&app);
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[TITLE, CAP_OPT, WIDE, "Page options", "Result notice kind"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", AUTH],
    );
    assert_controls(&wide_opt, CONTROLS);
    shot(&app, "sc.notices.options.120x40");
    shot(&app, "sc.notices.options");

    click_tab(&app, "Playground");
    flash_settle(&app);
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[TITLE, BLURB, CAP_PLAY, WIDE, AUTH, PROMPT, RETRIES, SYNTAX],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            HOST,
            CONNECTED,
        ],
    );
    assert!(
        !flat(&wide_play).contains("Check the password in the keychain"),
        "the wide card showed the detail past the scrollport\n{}",
        text(&wide_play)
    );
    shot(&app, "sc.notices.playground.120x40");
    shot(&app, "sc.notices.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s06-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", CAP_STATES],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    assert!(grid_label(&off, "rest"));
    click_where(&app, "Compare levels", |hit| hit.y < 16 && hit.x >= 18);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "no color", "Page options", CAP_PLAY],
    );
    flash_settle(&app);
    let mut saw256 = false;
    let mut saw16 = false;
    scroll_to_no_color(&app, &mut saw256, &mut saw16);
    assert!(saw256, "256 colors band was not shown");
    assert!(saw16, "16 colors band was not shown");
    let bands = app.wait_text("no color");
    see(
        &bands,
        &[TITLE, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(&app, "sc.notices.compare-levels.120x40");
    app.resize(80, 24);
    // Resize leaves the scrolled band, so this frame is the top truecolor row.
    let narrow = app.wait_screen("compare narrow", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains("● Compare levels")
            && text(screen).contains("truecolor")
            && grid_label(screen, "rest")
            && !text(screen).contains(WIDE)
    });
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor"],
        &[
            "Page options",
            CAP_PLAY,
            WIDE,
            "○ Compare levels",
            "no color",
        ],
    );
    shot(&app, "sc.notices.compare-levels.80x24");
    click_where(&app, "Compare levels", |hit| hit.y < 16 && hit.x >= 18);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &["● Compare levels", "no color", "Page options"],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Result notice kind");
    assert_controls(&options, CONTROLS);
    app.finish();
}

#[test]
fn connection_outcomes_before_and_after() {
    let app = page("s06-notices", None, 120, 40);
    let before = app.wait_text(AUTH);
    see(
        &before,
        &[
            AUTH,
            "FATAL",
            PROMPT,
            "in the keychain",
            RETRIES,
            "Test connection",
            SYNTAX,
        ],
        &[HOST, CONNECTED, SAVE, BLOCKED],
    );
    assert!(
        !flat(&before).contains("Check the password in the keychain"),
        "the wrapped detail was not clipped\n{}",
        text(&before)
    );
    shot(&app, "sc.notices.notices.before");

    click_test(&app);
    let warning = app.wait_text(HOST);
    see(
        &warning,
        &[HOST, TIMED_OUT, BLOCKED, RETRIES, "Test connection", SYNTAX],
        &[AUTH, CONNECTED, SAVE, PROMPT],
    );
    let warning_row = row_with(&warning, HOST);
    assert!(warning_row.contains('▲'), "warning row {warning_row:?}");

    click_test(&app);
    let success = app.wait_text(CONNECTED);
    see(
        &success,
        &[
            CONNECTED,
            SCHEMAS,
            "42 schemas",
            SAVE,
            "Test connection",
            SYNTAX,
        ],
        &[HOST, AUTH, RETRIES, BLOCKED, PROMPT, "FATAL"],
    );
    flash_settle(&app);
    let settled = app.wait_text(CONNECTED);
    see(
        &settled,
        &[CONNECTED, SCHEMAS, SAVE],
        &[HOST, AUTH, RETRIES],
    );
    shot(&app, "sc.notices.notices.after");

    click_test(&app);
    let again = app.wait_text(AUTH);
    see(
        &again,
        &[AUTH, RETRIES, PROMPT, "FATAL"],
        &[CONNECTED, SAVE, HOST, BLOCKED],
    );
    app.finish();
}

#[test]
fn option_kind_warning() {
    option_case(
        "s06-kind",
        "opt.notices.kind",
        "notices.kind=warning",
        Controls {
            kind: "warning",
            ..CONTROLS
        },
        &[AUTH, "FATAL", PROMPT, RETRIES, SYNTAX, "▲"],
        &[HOST, CONNECTED],
        |screen| {
            let query = row_with(screen, SYNTAX);
            assert!(query.contains('▲'), "query row {query:?}");
            assert!(
                !query.contains('!'),
                "query row kept the error mark {query:?}"
            );
            let connection = row_with(screen, AUTH);
            assert!(connection.contains('!'), "connection row {connection:?}");
            assert!(
                !connection.contains('▲'),
                "connection row took the query kind {connection:?}"
            );
        },
    );
}

#[test]
fn option_detail_off() {
    option_case(
        "s06-detail",
        "opt.notices.detail",
        "notices.detail=off",
        Controls {
            detail: "off",
            ..CONTROLS
        },
        &[
            AUTH,
            RETRIES,
            SYNTAX,
            "Enter on the result tab",
            "Port must be a number between 1 and 65535",
            "postgres · connection",
        ],
        &[
            "FATAL",
            PROMPT,
            "in the keychain",
            "Reload to see them",
            "LINE 1",
        ],
        |_| {},
    );
}

#[test]
fn option_hint_off() {
    option_case(
        "s06-hint",
        "opt.notices.hint",
        "notices.hint=off",
        Controls {
            hint: "off",
            ..CONTROLS
        },
        &[
            AUTH,
            "FATAL",
            PROMPT,
            "LINE 1",
            "ORDER BY created_at DESC LIMIT 200",
            "Reload to see them",
            SYNTAX,
        ],
        &[RETRIES, "Enter on the result tab"],
        |_| {},
    );
}

#[test]
fn option_max_lines_1() {
    option_case(
        "s06-max-lines",
        "opt.notices.max_lines",
        "notices.max_lines=1",
        Controls {
            lines: "1",
            ..CONTROLS
        },
        &[AUTH, "FATAL", "Check the pass", RETRIES, "LINE 1", SYNTAX],
        &[
            PROMPT,
            "in the keychain",
            "ORDER BY created_at DESC LIMIT 200",
        ],
        |screen| {
            assert!(
                flat(screen).contains("Reload to see them"),
                "the one-line warning detail disappeared\n{}",
                text(screen)
            );
        },
    );
}

#[test]
fn option_message_lines_1() {
    option_case(
        "s06-message-lines",
        "opt.notices.message_lines",
        "notices.message_lines=1",
        Controls {
            message: "1",
            ..CONTROLS
        },
        &[
            AUTH,
            SYNTAX,
            "Port must be a number between 1 and 65535",
            PROMPT,
            RETRIES,
        ],
        &[HOST, CONNECTED],
        |_| {},
    );
}

#[test]
fn option_detail_tone_muted() {
    option_case(
        "s06-tone",
        "opt.notices.detail_tone",
        "notices.detail_tone=muted",
        Controls {
            tone: "muted",
            ..CONTROLS
        },
        &[AUTH, "FATAL", PROMPT, RETRIES],
        &[HOST],
        |screen| {
            let detail = ink(screen, "FATAL");
            let hint = ink(screen, RETRIES);
            let message = ink(screen, AUTH);
            assert_eq!(
                detail, hint,
                "muted detail {detail:?} differs from the hint {hint:?}"
            );
            assert_ne!(detail, message, "detail colour matched the message");
        },
    );
}

#[test]
fn option_mark_cut_off() {
    option_case(
        "s06-mark",
        "opt.notices.mark_cut",
        "notices.mark_cut=off",
        Controls {
            mark: "off",
            ..CONTROLS
        },
        &[
            AUTH,
            "Check the pa",
            "in the keychain",
            PROMPT,
            RETRIES,
            "FATAL",
        ],
        &["Check …", HOST],
        |screen| {
            assert!(
                !flat(screen).contains("Check the password in the keychain"),
                "turning the mark off widened the scrollport\n{}",
                text(screen)
            );
        },
    );
}

#[test]
fn option_width_narrow() {
    option_case(
        "s06-width",
        "opt.notices.width",
        "notices.width=narrow",
        Controls {
            width: "narrow",
            ..CONTROLS
        },
        &[
            AUTH,
            "postgres · connection test",
            "Check …",
            "edits the con…",
            "interval…",
            "the stat…",
            SYNTAX,
            "Test connection",
        ],
        &[PROMPT, "ORDER BY created_at DESC LIMIT 200"],
        |screen| {
            let row = row_with(screen, "overwrites");
            assert!(
                !row.contains("overwrites them"),
                "narrow warning detail stayed on one line: {row:?}"
            );
        },
    );
}
