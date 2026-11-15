//! Toolbars route, sections, specimens, compare levels, actions, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s02_toolbars/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Inputs & forms › Toolbars";
const BLURB: &str =
    "A row of commands that is one focus stop: shortcuts, tips, toggles, separators";
const BLURB_NARROW: &str = "A row of commands that is one focus stop: short…";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CHECKS: &str = "Checkboxes (mixed, disabled, click target), radio groups and toggles";
const PLAY: &str = "the live widget";
const OPT: &str = "one control per option";
const STATE: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const IDLE: &str = "idle · Stop is disabled until a run starts";
const RUNNING: &str = "running · Stop is enabled, Run waits";
const NONE: &str = "none yet · Activated(i) or Toggled(i, on)";
const BEHIND: &str =
    "behind »: Stop (No query is running) · Format · Explain · Wrap · Line numbers";
const QUERY: &str =
    "Run F5   Run all Shift+F5   Stop  │  Format   Explain  │  ○ Wrap   ● Line numbers";
const NARROW_ROW: &str = "Run F5   Run all Shift+F5    »";
const FORMAT_ROW: &str = "○ Bold   ○ Italic   ○ Code  │  ● Left   ○ Center   ○ Right";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const ACTIVATED_RUN: &str = "Activated(0) · Run";
const TOGGLED_LINES: &str = "Toggled(6, false) · Line numbers";

const SHOWN: &[(&str, &str)] = &[
    ("sc.toolbars.specimen.rest", "rest"),
    ("sc.toolbars.specimen.hover", "hover"),
    ("sc.toolbars.specimen.pressed", "pressed"),
    ("sc.toolbars.specimen.focused", "focused"),
    ("sc.toolbars.specimen.focused_hover", "focus + hover"),
    ("sc.toolbars.specimen.selected", "selected"),
    ("sc.toolbars.specimen.disabled", "disabled"),
    ("sc.toolbars.specimen.overflow", "overflow"),
    ("sc.toolbars.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s02_toolbars/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn page(name: &str, opt: Option<&str>, cols: u16, rows: u16) -> App {
    let mut args = vec!["--page", "toolbars"];
    if let Some(value) = opt {
        args.push("--opt");
        args.push(value);
    }
    open(name, cols, rows, &args)
}

/// The crumb updates before the playground replaces the previous page.
/// The toolbar row is one paint, and a read can return it before Wrap.
fn toolbars_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let on_toolbars = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .any(|line| {
            line.contains('▎') && line.contains("› Toolbars") && !line.contains("Inputs & forms")
        });
    on_toolbars
        && visible.contains(TITLE)
        && visible.contains(BLURB)
        && visible.contains(QUERY)
        && visible.contains(NARROW_ROW)
        && visible.contains(BEHIND)
        && visible.contains(FORMAT_ROW)
        && visible.contains(IDLE)
        && visible.contains(NONE)
        && !visible.contains("Send notifications")
        && !visible.contains("Read committed")
        && !visible.contains(OVERVIEW)
}

fn checks_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("Checks & radios")
        && visible.contains(CHECKS)
        && visible.contains("Read committed")
        && !visible.contains(BLURB)
        && !visible.contains("Query toolbar")
        && !visible.contains("behind »:")
}

/// The "Query toolbar" label paints before the gap-2 command rows.
fn gap_two_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let Some(query) = lines.iter().find(|line| line.contains("Shift+F5")) else {
        return false;
    };
    let Some(narrow) = lines
        .iter()
        .find(|line| line.contains('»') && line.contains("Run F5") && !line.contains("behind"))
    else {
        return false;
    };
    let Some(behind) = lines.iter().find(|line| line.contains("behind »:")) else {
        return false;
    };
    query.contains("Run F5    Run all")
        && !query.contains("Run F5   Run all")
        && narrow.contains("Run F5")
        && narrow.contains('»')
        && !narrow.contains("Run all")
        && !narrow.contains("Shift")
        && behind.contains("Run all · Stop")
        && visible.contains(IDLE)
        && visible.contains("○ Bold    ○ Italic")
        && visible.contains("● Line numbers")
        && !visible.contains(NARROW_ROW)
        && !visible.contains(ACTIVATED_RUN)
        && !visible.contains("Page options")
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(
            !visible.contains(item),
            "forbidden {item:?} is visible\n{visible}"
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
        .position(|line| line.contains(label))
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

fn sidebar_toolbars(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("› Toolbars") && !line.contains("Inputs & forms"))
        .unwrap_or_else(|| panic!("sidebar Toolbars missing\n{}", text(screen)))
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
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

fn click_first(app: &App, label: &str, row_needle: &str) {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| row_text(&screen, u32::from(hit.y)).contains(row_needle))
        .collect();
    assert!(
        !hits.is_empty(),
        "no {label:?} on a row containing {row_needle:?}\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.y);
    app.click(hits[0]);
}

fn focus_marker(app: &App, marker: &str, what: &str) {
    app.settle();
    for _ in 0..8 {
        if text(&app.screen()).contains(marker) {
            return;
        }
        app.key("tab");
        app.settle();
    }
    panic!("{what}\n{}", text(&app.screen()));
}

fn focus_query(app: &App) {
    focus_marker(app, "▎Run F5", "query toolbar did not take focus");
}

fn focus_format(app: &App) {
    focus_marker(app, "▎○ Bold", "format toolbar did not take focus");
}

fn shot(app: &App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn defaults_besides(screen: &TerminalSnapshot, changed: &str) {
    if changed != "nav" {
        let nav = value_under(screen, "Nav keys");
        assert!(
            nav.starts_with("desktop") && !nav.contains("vim"),
            "nav changed: {nav}"
        );
    }
    if changed != "wrap" {
        see(screen, &["Wrap off"], &["Wrap on"]);
    }
    if changed != "skip" {
        see(screen, &["Skip disabled on"], &["Skip disabled off"]);
    }
    if changed != "gap" {
        let gap = value_under(screen, "Gap");
        assert!(
            gap.starts_with('1') && !gap.starts_with('2') && !gap.starts_with('0'),
            "gap changed: {gap}"
        );
    }
    if changed != "overflow" {
        let overflow = value_under(screen, "Overflow button");
        assert!(
            overflow.starts_with('»') && !overflow.contains('…') && !overflow.contains("off"),
            "overflow changed: {overflow}"
        );
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "toolbars"];
    let app = page("s02-toolbars-route-page", None, 120, 40);
    let screen = app.wait_screen("toolbars playground", toolbars_ready);
    see(
        &screen,
        &[
            TITLE,
            "Toolbars",
            BLURB,
            PLAY,
            WIDE,
            "truecolor",
            QUERY,
            NARROW_ROW,
            BEHIND,
            FORMAT_ROW,
            IDLE,
            NONE,
            "one Tab stop · ←→ · Enter",
            "» holds what does not fit",
        ],
        &[
            "Page options",
            OPT,
            STATE,
            "Compare levels",
            "vim",
            "Wrap on",
            ACTIVATED_RUN,
            RUNNING,
            OVERVIEW,
        ],
    );
    let side = sidebar_toolbars(&screen);
    assert!(
        side.contains('▎'),
        "sidebar focus is not on Toolbars: {side}"
    );
    shot(
        &app,
        "sc.toolbars.route",
        "--page toolbars opens the playground with the sidebar on Toolbars",
        &args,
    );
    app.finish();

    let keys = open("s02-toolbars-route-keys", 120, 40, &[]);
    keys.wait_text(OVERVIEW);
    for _ in 0..13 {
        keys.key("]");
    }
    let arrived = keys.wait_screen("toolbars by keyboard", toolbars_ready);
    see(
        &arrived,
        &[TITLE, BLURB, "Query toolbar", QUERY, PLAY],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            "Primary, secondary",
        ],
    );
    keys.key("[");
    let left = keys.wait_screen("checks page", checks_ready);
    see(
        &left,
        &["Checks & radios", CHECKS],
        &[BLURB, "Query toolbar", "behind »:"],
    );
    keys.key("]");
    let again = keys.wait_screen("toolbars again", toolbars_ready);
    see(
        &again,
        &[TITLE, "Query toolbar", BLURB],
        &[CHECKS, OVERVIEW],
    );
    click_side(&keys, "Overview");
    let overview = keys.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[BLURB, "Query toolbar"]);
    click_side(&keys, "Inputs & forms");
    click_side(&keys, "Toolbars");
    let pointed = keys.wait_screen("toolbars by pointer", toolbars_ready);
    see(
        &pointed,
        &[TITLE, BLURB, QUERY, IDLE, NARROW_ROW],
        &[OVERVIEW, "Page options", "Compare levels", OPT, STATE],
    );
    keys.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "toolbars"];
    let app = page("s02-toolbars-sections", None, 80, 24);
    let narrow_play = app.wait_text("Query toolbar");
    see(
        &narrow_play,
        &[
            TITLE,
            "Toolbars",
            BLURB_NARROW,
            "Playground",
            "Query toolbar",
            "Narrow toolbar",
            "Run F5",
            "Run all",
            IDLE,
            "behind »:",
            "»",
        ],
        &[
            BLURB,
            PLAY,
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            FORMAT_ROW,
            "Last",
        ],
    );
    shot(
        &app,
        "sc.toolbars.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Nav keys",
            "desktop",
            "Wrap off",
            "Skip disabled on",
            "Gap",
            "Overflow button",
            "»",
        ],
        &[
            PLAY,
            OPT,
            STATE,
            "Compare levels",
            "Query toolbar",
            "vim",
            "Wrap on",
            "Skip disabled off",
            WIDE,
            "truecolor",
        ],
    );
    assert!(value_under(&narrow_opt, "Gap").starts_with('1'));
    assert!(value_under(&narrow_opt, "Overflow button").starts_with('»'));
    assert!(value_under(&narrow_opt, "Nav keys").starts_with("desktop"));
    shot(
        &app,
        "sc.toolbars.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "selected",
            "disabled",
            "overflow",
            "narrow",
            "Commands",
        ],
        &[
            PLAY,
            OPT,
            STATE,
            "Page options",
            "● Compare levels",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            WIDE,
            "truecolor",
        ],
    );
    shot(
        &app,
        "sc.toolbars.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_screen("wide states grid", |screen| {
        let visible = text(screen);
        visible.contains(WIDE)
            && visible.contains("● Wrap")
            && visible.contains("● Bold")
            && visible.contains("Run F5       »")
    });
    see(
        &wide_states,
        &[
            TITLE,
            STATE,
            WIDE,
            "truecolor",
            "○ Compare levels",
            LEGEND,
            "Commands",
            "Toggles",
            "rest",
            "focus + hover",
            "selected",
            "● Wrap",
            "● Bold",
            "overflow",
            "narrow",
            "Run F5       »",
        ],
        &[
            PLAY,
            OPT,
            "Page options",
            "● Compare levels",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "256 colors",
        ],
    );
    shot(
        &app,
        "sc.toolbars.states.120x40",
        "states caption and the nine grid rows at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.toolbars.states",
        "states is the visible Toolbars section",
        &args,
    );
    for (case_id, label) in SHOWN {
        see(
            &app.screen(),
            &[label, "○ Compare levels", STATE],
            &[
                "● Compare levels",
                "Page options",
                "busy",
                "error",
                "editing",
                "empty",
                "loading",
            ],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &args,
        );
    }

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            OPT,
            WIDE,
            "Page options",
            "Nav keys",
            "Wrap off",
            "Skip disabled on",
            "Gap",
            "Overflow button",
        ],
        &[
            PLAY,
            STATE,
            "Compare levels",
            "Query toolbar",
            "vim",
            "Wrap on",
            "Skip disabled off",
        ],
    );
    assert!(value_under(&wide_opt, "Nav keys").starts_with("desktop"));
    assert!(value_under(&wide_opt, "Gap").starts_with('1'));
    assert!(value_under(&wide_opt, "Overflow button").starts_with('»'));
    shot(
        &app,
        "sc.toolbars.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.toolbars.options",
        "options is the visible Toolbars section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(PLAY);
    see(
        &wide_play,
        &[
            TITLE, BLURB, PLAY, WIDE, QUERY, NARROW_ROW, BEHIND, FORMAT_ROW, IDLE, NONE,
        ],
        &[
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            BLURB_NARROW,
            "vim",
            ACTIVATED_RUN,
            RUNNING,
        ],
    );
    shot(
        &app,
        "sc.toolbars.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.toolbars.playground",
        "playground is the visible Toolbars section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "toolbars"];
    let app = page("s02-toolbars-compare", None, 120, 40);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", "narrow", STATE, LEGEND],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
            "busy",
            "error",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 12);
    let on = app.wait_text("256 colors");
    see(
        &on,
        &[TITLE, "● Compare levels", "truecolor", "256 colors", STATE],
        &["○ Compare levels", "Page options", PLAY, "no color"],
    );
    for _ in 0..8 {
        let visible = text(&app.screen());
        if visible.contains("no color") && visible.contains("16 colors") {
            break;
        }
        app.key("pagedown");
    }
    let bands = app.wait_text("no color");
    see(
        &bands,
        &[TITLE, "16 colors", "no color", WIDE, "Compare levels"],
        &["Page options", PLAY, "○ Compare levels", "busy", "editing"],
    );
    shot(
        &app,
        "sc.toolbars.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_text(TITLE);
    let visible = text(&narrow);
    assert!(
        visible.contains("● Compare levels")
            && visible.contains("truecolor")
            && !visible.contains(WIDE),
        "narrow compare bands missing\n{visible}"
    );
    see(
        &narrow,
        &[TITLE, "Compare levels", "truecolor"],
        &["Page options", PLAY, WIDE, "120×40", OPT],
    );
    shot(
        &app,
        "sc.toolbars.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
            "truecolor",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    defaults_besides(&options, "");
    see(
        &options,
        &["Page options", "Wrap off", "Skip disabled on", OPT],
        &["Wrap on", "Skip disabled off", "vim", "Compare levels"],
    );
    app.finish();
}

#[test]
fn actions_run_toggle_and_overflow() {
    let args = ["--page", "toolbars"];
    let app = page("s02-toolbars-actions", None, 120, 40);
    let before = app.wait_text(IDLE);
    see(
        &before,
        &[QUERY, NARROW_ROW, BEHIND, "● Line numbers", "○ Wrap", NONE],
        &[ACTIVATED_RUN, RUNNING, TOGGLED_LINES, "Esc Close"],
    );
    click_first(&app, "Run F5", "Shift+F5");
    let ran = app.wait_text(ACTIVATED_RUN);
    see(
        &ran,
        &[ACTIVATED_RUN, RUNNING, "○ Wrap", "● Line numbers"],
        &[
            NONE,
            IDLE,
            "No query is running",
            TOGGLED_LINES,
            "Toggled(",
            "Activated(1)",
            "Activated(2)",
        ],
    );
    let behind = row_with(&ran, "behind »:");
    assert!(
        behind.contains("Stop · Format") && !behind.contains("No query is running"),
        "running overflow list still names the idle reason: {behind}"
    );
    click_first(&app, "Line numbers", "Shift+F5");
    let toggled = app.wait_text(TOGGLED_LINES);
    let query = row_with(&toggled, "Shift+F5");
    assert!(
        query.contains("○ Line numbers") && query.contains("○ Wrap") && !query.contains('●'),
        "line numbers stayed on: {query}"
    );
    see(
        &toggled,
        &[TOGGLED_LINES, RUNNING, "○ Line numbers"],
        &[
            ACTIVATED_RUN,
            "● Line numbers",
            "Toggled(6, true)",
            "Activated(6)",
            IDLE,
            NONE,
        ],
    );
    click_first(&app, "»", "Run all");
    let menu = app.wait_text("Close");
    let hints = footer(&menu);
    assert!(
        hints.contains("Esc") && hints.contains("Close"),
        "overflow menu did not take the footer: {hints}"
    );
    let query = row_with(&menu, "Shift+F5");
    assert!(
        query.contains("○ Line numbers") && !query.contains('●'),
        "opening the overflow menu turned Line numbers back on: {query}"
    );
    see(
        &menu,
        &[TOGGLED_LINES, RUNNING, "Close"],
        &[
            "No query is running",
            IDLE,
            NONE,
            ACTIVATED_RUN,
            "Into page",
        ],
    );
    shot(
        &app,
        "sc.toolbars.actions",
        "Run reports Activated(0), Line numbers toggles off, and the narrow overflow menu opens",
        &args,
    );
    app.finish();
}

#[test]
fn option_nav_vim() {
    let args = ["--page", "toolbars", "--opt", "toolbars.nav=vim"];
    let app = page("s02-toolbars-nav", Some("toolbars.nav=vim"), 120, 40);
    click_tab(&app, "Options");
    let options = app.wait_text("Nav keys");
    let nav = value_under(&options, "Nav keys");
    assert!(
        nav.starts_with("vim") && !nav.contains("desktop"),
        "nav value: {nav}"
    );
    defaults_besides(&options, "nav");
    see(
        &options,
        &["vim", "Wrap off", "Skip disabled on"],
        &["desktop", "Wrap on", PLAY],
    );
    click_tab(&app, "Playground");
    focus_query(&app);
    let armed = app.screen();
    let before = row_with(&armed, "F5");
    assert!(
        before.contains("▎Run F5") && !before.contains("▎Run all"),
        "vim did not start on Run: {before}"
    );
    app.key("l");
    let moved = app.wait_text("▎Run all");
    let row = row_with(&moved, "Shift+F5");
    assert!(
        row.contains("▎Run all") && !row.contains("▎Run F5"),
        "vim l did not move to Run all: {row}"
    );
    see(
        &moved,
        &[IDLE, NONE, "○ Wrap", "● Line numbers"],
        &[ACTIVATED_RUN, RUNNING, "Toggled(5", "Wrap on"],
    );
    assert!(
        row.contains("○ Wrap") && row.contains("● Line numbers"),
        "vim l changed a toggle: {row}"
    );
    shot(
        &app,
        "opt.toolbars.nav",
        "toolbars.nav=vim moves l from Run to Run all and leaves the other options",
        &args,
    );
    app.finish();
}

#[test]
fn option_wrap_on() {
    let args = ["--page", "toolbars", "--opt", "toolbars.wrap=on"];
    let app = page("s02-toolbars-wrap", Some("toolbars.wrap=on"), 120, 40);
    click_tab(&app, "Options");
    let options = app.wait_text("Wrap on");
    defaults_besides(&options, "wrap");
    see(
        &options,
        &["Wrap on", "Skip disabled on", "desktop"],
        &["Wrap off", "vim", "Skip disabled off", PLAY],
    );
    click_tab(&app, "Playground");
    focus_format(&app);
    app.key("end");
    let end = app.wait_text("▎○ Right");
    let ended = row_with(&end, "Right");
    assert!(
        ended.contains("▎○ Right") && !ended.contains("▎○ Bold"),
        "end did not reach Right: {ended}"
    );
    app.key("right");
    let wrapped = app.wait_text("▎○ Bold");
    let row = row_with(&wrapped, "Italic");
    assert!(
        row.contains("▎○ Bold") && !row.contains("▎○ Right"),
        "wrap on did not return to Bold: {row}"
    );
    see(
        &wrapped,
        &[IDLE, NONE, "● Left", "○ Center"],
        &[ACTIVATED_RUN, "▎○ Right", "Page options", "Wrap off"],
    );
    shot(
        &app,
        "opt.toolbars.wrap",
        "toolbars.wrap=on returns Right to Bold and leaves the other options",
        &args,
    );
    app.finish();
}

#[test]
fn option_skip_disabled_off() {
    let args = ["--page", "toolbars", "--opt", "toolbars.skip_disabled=off"];
    let app = page(
        "s02-toolbars-skip",
        Some("toolbars.skip_disabled=off"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Skip disabled off");
    defaults_besides(&options, "skip");
    see(
        &options,
        &["Skip disabled off", "Wrap off", "desktop"],
        &["Skip disabled on", "Wrap on", "vim", PLAY],
    );
    click_tab(&app, "Playground");
    focus_query(&app);
    app.key("right");
    let mid = app.wait_text("▎Run all");
    assert!(
        row_with(&mid, "Shift+F5").contains("▎Run all"),
        "first right missed Run all"
    );
    app.key("right");
    let stopped = app.settle();
    let row = row_with(&stopped, "Shift+F5");
    assert!(
        !row.contains('▎'),
        "skip off still drew a focus bar after landing on Stop: {row}"
    );
    assert!(
        footer(&stopped).contains("Enter / Space"),
        "toolbar lost focus on Stop: {}",
        footer(&stopped)
    );
    app.key("enter");
    let held = app.settle();
    see(
        &held,
        &[IDLE, NONE, "Stop"],
        &[ACTIVATED_RUN, "Activated(2)", RUNNING, "Toggled(6"],
    );
    app.key("right");
    let format = app.wait_text("▎Format");
    let next = row_with(&format, "Explain");
    assert!(
        next.contains("▎Format") && !next.contains("▎Explain") && !next.contains("▎Stop"),
        "third right was not Format: {next}"
    );
    shot(
        &app,
        "opt.toolbars.skip_disabled",
        "toolbars.skip_disabled=off stops on disabled Stop, Enter does not run it, and the next right is Format",
        &args,
    );
    app.finish();
}

#[test]
fn option_gap_two() {
    let args = ["--page", "toolbars", "--opt", "toolbars.gap=2"];
    let app = page("s02-toolbars-gap", Some("toolbars.gap=2"), 120, 40);
    let play = app.wait_screen("gap 2 playground", gap_two_ready);
    let query = row_with(&play, "Shift+F5");
    let narrow_items = (0..play.rows())
        .map(|row| row_text(&play, row))
        .find(|line| line.contains('»') && line.contains("Run F5") && !line.contains("behind"))
        .expect("narrow row");
    assert!(
        query.contains("Run F5    Run all") && !query.contains("Run F5   Run all"),
        "gap 2 kept the default spacing: {query}"
    );
    assert!(
        narrow_items.contains("Run F5")
            && narrow_items.contains('»')
            && !narrow_items.contains("Run all")
            && !narrow_items.contains("Shift"),
        "gap 2 still fitted Run all in the narrow bar: {narrow_items}"
    );
    let behind = row_with(&play, "behind »:");
    assert!(
        behind.contains("Run all · Stop"),
        "gap 2 did not push Run all behind the overflow button: {behind}"
    );
    see(
        &play,
        &[IDLE, "○ Bold    ○ Italic", "● Line numbers"],
        &[NARROW_ROW, ACTIVATED_RUN, "Page options"],
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Gap");
    let gap = value_under(&options, "Gap");
    assert!(
        gap.starts_with('2') && !gap.starts_with('1') && !gap.starts_with('0'),
        "gap value: {gap}"
    );
    defaults_besides(&options, "gap");
    see(
        &options,
        &["Wrap off", "Skip disabled on", "desktop"],
        &["Wrap on", "vim", "Skip disabled off"],
    );
    click_tab(&app, "Playground");
    let again = app.wait_screen("gap 2 again", gap_two_ready);
    assert!(
        row_with(&again, "Shift+F5").contains("Run F5    Run all"),
        "returning to the playground lost gap 2"
    );
    shot(
        &app,
        "opt.toolbars.gap",
        "toolbars.gap=2 widens the query row, hides Run all behind », and leaves the other options",
        &args,
    );
    app.finish();
}

#[test]
fn option_overflow_off() {
    let args = ["--page", "toolbars", "--opt", "toolbars.overflow=off"];
    let app = page(
        "s02-toolbars-overflow",
        Some("toolbars.overflow=off"),
        120,
        40,
    );
    let play = app.wait_text("everything fits");
    let narrow = (0..play.rows())
        .map(|row| row_text(&play, row))
        .find(|line| {
            line.contains("Run F5") && line.contains("Shift+F5") && line.contains("Navigation")
        })
        .expect("narrow row");
    assert!(
        narrow.contains("Run F5   Run all Shift+F5")
            && !narrow.contains('»')
            && !narrow.contains('…'),
        "overflow off still drew a more button: {narrow}"
    );
    see(
        &play,
        &[
            "everything fits: no » button",
            QUERY,
            IDLE,
            "○ Wrap",
            "● Line numbers",
        ],
        &["behind »:", NARROW_ROW, "…", ACTIVATED_RUN, "Page options"],
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Overflow button");
    let overflow = value_under(&options, "Overflow button");
    assert!(
        overflow.starts_with("off") && !overflow.contains('»') && !overflow.contains('…'),
        "overflow value: {overflow}"
    );
    defaults_besides(&options, "overflow");
    see(
        &options,
        &["off", "Wrap off", "Skip disabled on", "desktop"],
        &["Wrap on", "vim", "Skip disabled off", "…"],
    );
    click_tab(&app, "Playground");
    let again = app.wait_text("everything fits");
    assert!(
        text(&again).contains("everything fits: no » button")
            && !text(&again).contains("behind »:"),
        "playground lost the overflow-off readout"
    );
    shot(
        &app,
        "opt.toolbars.overflow",
        "toolbars.overflow=off clips the narrow bar with no more button and leaves the other options",
        &args,
    );
    app.finish();
}
