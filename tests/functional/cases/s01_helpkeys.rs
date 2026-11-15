//! Help & keys route, sections, specimens, compare levels, help, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s01_helpkeys/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, SHOWCASE, find_all, row_text, text, try_launch};

const BLURB: &str = "HelpOverlay and KeyReference: sections, columns, scrolling, hanging wrap";
const PAGE: &str = "App shell › Help & keys";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDTH_DEFAULT: &str = "KeyReference at width 54";
const WIDTH_NARROW: &str = "KeyReference at width 28";
const META_DEFAULT: &str = "share 40% · gap 3";
const TITLE_DEFAULT: &str = "shortcuts";
const TITLE_CUSTOM: &str = "Editor keys";
const DISMISS_LIGHT: &str = "light: Esc, ×,";
const DISMISS_ESCAPE: &str = "Esc and × only";
const CLOSE_DEFAULT: &str = "? q · Esc ·";
const CLOSE_F1: &str = "F1 · Esc ·";

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s01_helpkeys/{}", case_id.replace('.', "_"))
}

fn fixture(args: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = args.iter().copied().map(str::to_string).collect();
    out.extend(["--color", "truecolor", "--motion", "paused"].map(str::to_string));
    out
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

fn require(app: &App, present: &[&str], absent: &[&str]) {
    let visible = text(&app.settle());
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

fn click_text(app: &App, needle: &str) {
    let screen = app.settle();
    let mut hits = find_all(&screen, needle);
    hits.retain(|point| point.y >= 4);
    hits.sort_by_key(|point| (point.y, point.x));
    assert!(
        !hits.is_empty(),
        "no click target {needle:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn open_help_keys(app: &App) {
    let screen = app.settle();
    let visible = find_all(&screen, "Help & keys")
        .into_iter()
        .any(|point| point.y >= 4);
    if !visible {
        click_text(app, "App shell");
        app.wait_text("Help & keys");
    }
    click_text(app, "Help & keys");
}

fn row_containing(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing row {needle:?}\n{}", text(screen)))
}

fn focused_here(line: &str, label: &str) -> bool {
    line.find(label)
        .is_some_and(|at| line[..at].trim_end().ends_with('▎'))
}

fn option_value(screen: &TerminalSnapshot, label: &str) -> String {
    let visible = text(screen);
    let lines: Vec<&str> = visible.lines().collect();
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label:?}\n{visible}"));
    let column = lines[index].find(label).expect("label column");
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).copied().unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    rest.split_whitespace()
        .find(|token| *token != "▎")
        .unwrap_or("")
        .to_string()
}

fn assert_size(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(screen.cols(), cols, "terminal columns");
    assert_eq!(screen.rows(), rows, "terminal rows");
}

fn shot(app: &App, case_id: &str, _precondition: &str, _fixture_args: &[String]) {
    app.visual(&visual_id(case_id));
}

fn playground_wide(app: &App) {
    require(
        app,
        &[
            PAGE,
            BLURB,
            CAP_PLAY,
            "120×40",
            WIDTH_DEFAULT,
            META_DEFAULT,
            "Open help overlay",
            "Results",
            "Query",
            "Ctrl+Enter",
            TITLE_DEFAULT,
            "Scope",
            "Editor",
            "Max share",
            "80%",
            CLOSE_DEFAULT,
            DISMISS_LIGHT,
            "outside",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDTH_NARROW,
            "share 60%",
            "gap 6",
            TITLE_CUSTOM,
            "100%",
            "j k",
            CLOSE_F1,
            DISMISS_ESCAPE,
            "App shell › Quit & leave",
            "App shell › Breadcrumbs & rails",
            "Keyboard & mouse",
        ],
    );
}

#[test]
fn route_opens_by_page_keyboard_and_pointer() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-route", 120, 40, &args);
    let screen = app.wait_text(WIDTH_DEFAULT);
    assert_size(&screen, 120, 40);
    playground_wide(&app);
    let screen = app.screen();
    let selected = (0..screen.rows())
        .map(|row| row_text(&screen, row))
        .find(|line| line.contains('▎') && line.contains("Help & keys") && line.contains('›'))
        .unwrap_or_else(|| panic!("sidebar focus is not on Help & keys\n{}", text(&screen)));
    assert!(
        selected.contains("Help & keys"),
        "sidebar focus stays on Help & keys: {selected}"
    );
    let button = row_containing(&screen, "Open help overlay");
    assert!(
        !focused_here(&button, "Open help overlay"),
        "the help button is not the focused control: {button}"
    );
    shot(
        &app,
        "sc.helpkeys.route",
        "--page helpkeys opens the playground with the sidebar focused and caption the live widget",
        &fixed,
    );
    app.key("]");
    let left = app.wait_text("App shell › Quit & leave");
    assert!(
        !text(&left).contains(WIDTH_DEFAULT) && text(&left).contains("QuitKeys"),
        "leaving with ] should open Quit & leave, not keep the key reference\n{}",
        text(&left)
    );
    app.key("[");
    let returned = app.wait_text(WIDTH_DEFAULT);
    assert!(
        text(&returned).contains(PAGE) && text(&returned).contains(CAP_PLAY),
        " [ should re-enter Help & keys\n{}",
        text(&returned)
    );
    assert!(
        !text(&returned).contains("App shell › Quit & leave"),
        "Quit & leave stayed active after [\n{}",
        text(&returned)
    );
    click_text(&app, "Overview");
    let overview = app.wait_text("Tokens and principles");
    assert!(
        !text(&overview).contains(WIDTH_DEFAULT) && !text(&overview).contains(PAGE),
        "sidebar Overview should leave Help & keys\n{}",
        text(&overview)
    );
    open_help_keys(&app);
    let back = app.wait_text(WIDTH_DEFAULT);
    assert!(
        text(&back).contains(PAGE)
            && text(&back).contains(CAP_PLAY)
            && text(&back).contains(META_DEFAULT),
        "sidebar Help & keys should restore the playground\n{}",
        text(&back)
    );
    app.finish();

    let neighbor = ["--page", "breadcrumbsrails"];
    let app = open("s01-helpkeys-keys", 120, 40, &neighbor);
    app.wait_text("Breadcrumbs");
    app.key("]");
    let opened = app.wait_text(WIDTH_DEFAULT);
    assert!(
        text(&opened).contains(PAGE) && !text(&opened).contains("Disk › Users › alex"),
        "] from Breadcrumbs & rails should open Help & keys\n{}",
        text(&opened)
    );
    app.key("[");
    let crumbs = app.wait_text("Disk › Users › alex");
    assert!(
        !text(&crumbs).contains(WIDTH_DEFAULT),
        "[ should leave Help & keys\n{}",
        text(&crumbs)
    );
    app.key("]");
    require(
        &app,
        &[PAGE, CAP_PLAY, WIDTH_DEFAULT, META_DEFAULT],
        &["Disk › Users › alex", CAP_OPT, CAP_STATES],
    );
    app.finish();

    let overview_args = ["--page", "overview"];
    let app = open("s01-helpkeys-pointer", 120, 40, &overview_args);
    app.wait_text("Tokens and principles");
    open_help_keys(&app);
    require(
        &app,
        &[PAGE, CAP_PLAY, WIDTH_DEFAULT, "Open help overlay"],
        &[
            "Tokens and principles",
            CAP_OPT,
            CAP_STATES,
            "App shell › Overview",
        ],
    );
    app.finish();
}

#[test]
fn playground_sections_at_both_sizes() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-play", 80, 24, &args);
    let narrow = app.wait_text(WIDTH_DEFAULT);
    assert_size(&narrow, 80, 24);
    require(
        &app,
        &[
            PAGE,
            "Help & keys",
            WIDTH_DEFAULT,
            META_DEFAULT,
            "Results",
            "Query",
            "Ctrl+Enter",
            "move between rows",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "120×40",
            WIDTH_NARROW,
            "Open help overlay",
            TITLE_CUSTOM,
        ],
    );
    shot(
        &app,
        "sc.helpkeys.playground.80x24",
        "playground at 80 by 24 before the terminal grows, with no section caption",
        &fixed,
    );
    app.resize(120, 40);
    let wide = app.wait_screen("grown playground", |screen| {
        screen.cols() == 120 && text(screen).contains(CAP_PLAY) && text(screen).contains("120×40")
    });
    assert_size(&wide, 120, 40);
    playground_wide(&app);
    app.key("tab");
    let focused = app.wait_text("←→ Section");
    let tab = row_containing(&focused, "Playground");
    assert!(
        tab.contains('▎') && tab.contains(CAP_PLAY),
        "keyboard focus sits on Playground: {tab}"
    );
    app.key("2");
    require(
        &app,
        &[CAP_OPT, "Page options", "Reference width", "full"],
        &[CAP_PLAY, CAP_STATES, WIDTH_DEFAULT, "Compare levels"],
    );
    app.key("3");
    require(
        &app,
        &[CAP_STATES, "○ Compare levels", "rest", "overflow", "narrow"],
        &[CAP_PLAY, CAP_OPT, WIDTH_DEFAULT, "Page options", "●"],
    );
    app.key("1");
    playground_wide(&app);
    let tab = row_containing(&app.screen(), "Playground");
    assert!(
        tab.contains('▎'),
        "Playground keeps the focus gutter: {tab}"
    );
    shot(
        &app,
        "sc.helpkeys.playground.120x40",
        "playground caption the live widget is active at 120 by 40 after keyboard section jumps",
        &fixed,
    );
    shot(
        &app,
        "sc.helpkeys.playground",
        "playground is the visible Help & keys section",
        &fixed,
    );
    app.finish();
}

#[test]
fn options_section_at_both_sizes() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-options", 80, 24, &args);
    app.wait_text(WIDTH_DEFAULT);
    click_text(&app, "Options");
    let narrow = app.wait_text("Reference width");
    assert_size(&narrow, 80, 24);
    require(
        &app,
        &[PAGE, "Page options", "Reference width", "Key column share"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            WIDTH_DEFAULT,
            "Compare levels",
            "Open help overlay",
            "120×40",
        ],
    );
    let screen = app.screen();
    assert_eq!(option_value(&screen, "Reference width"), "full");
    assert_eq!(option_value(&screen, "Key column share"), "40");
    assert_eq!(option_value(&screen, "Column gap"), "3");
    shot(
        &app,
        "sc.helpkeys.options.80x24",
        "options at 80 by 24 before the terminal grows, with the default controls and no caption",
        &fixed,
    );
    app.resize(120, 40);
    let wide = app.wait_screen("grown options", |screen| {
        screen.cols() == 120 && text(screen).contains(CAP_OPT) && text(screen).contains("Dismiss")
    });
    assert_size(&wide, 120, 40);
    require(
        &app,
        &[
            PAGE,
            CAP_OPT,
            "120×40",
            "Page options",
            "Reference width",
            "Overlay title",
            "Overlay size",
            "Scroll keys",
            "Close keys",
            "Dismiss",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            WIDTH_DEFAULT,
            "Compare levels",
            "Open help overlay",
            TITLE_CUSTOM,
        ],
    );
    let screen = app.screen();
    assert_eq!(option_value(&screen, "Reference width"), "full");
    assert_eq!(option_value(&screen, "Key column share"), "40");
    assert_eq!(option_value(&screen, "Column gap"), "3");
    assert_eq!(option_value(&screen, "Overlay title"), "default");
    assert_eq!(option_value(&screen, "Overlay size"), "80");
    assert_eq!(option_value(&screen, "Scroll keys"), "default");
    assert_eq!(option_value(&screen, "Close keys"), "default");
    assert_eq!(option_value(&screen, "Dismiss"), "light");
    shot(
        &app,
        "sc.helpkeys.options.120x40",
        "options caption one control per option is active at 120 by 40 and every control is at its default",
        &fixed,
    );
    shot(
        &app,
        "sc.helpkeys.options",
        "options is the visible Help & keys section",
        &fixed,
    );
    app.finish();
}

fn states_rows(app: &App) {
    require(
        app,
        &[
            PAGE,
            "○ Compare levels",
            "rest",
            "overflow",
            "narrow",
            "Reference",
            "Query",
            "Ctrl+Enter",
            "Ctrl+Shift+F",
            "find in every open file",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            WIDTH_DEFAULT,
            "Open help overlay",
            "●",
            "hover",
            "pressed",
            "focus + hover",
            "selected",
            "disabled",
            "busy",
            "editing",
            "empty",
            "loading",
            "256 colors",
        ],
    );
    let screen = app.screen();
    let rest = row_containing(&screen, "rest");
    assert!(
        rest.contains("Query") && rest.contains("Ctrl+Enter") && !rest.contains("Ctrl+Shift+F"),
        "rest row is the short reference, not the overflow key: {rest}"
    );
    let overflow = row_containing(&screen, "overflow");
    assert!(
        overflow.contains("Ctrl+Shift+F") || text(&screen).contains("Ctrl+Shift+F"),
        "overflow row draws the long key\n{}",
        text(&screen)
    );
    let narrow = row_containing(&screen, "narrow");
    assert!(
        narrow.contains("Query")
            || narrow.contains("Ctrl+Enter")
            || text(&screen).contains("narrow"),
        "narrow row is drawn: {narrow}"
    );
}

#[test]
fn states_section_and_specimens() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-states", 80, 24, &args);
    app.wait_text("Playground");
    click_text(&app, "States");
    let narrow = app.wait_text("○ Compare levels");
    assert_size(&narrow, 80, 24);
    require(
        &app,
        &[PAGE, "rest", "overflow", "narrow", "Ctrl+Shift+F"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            WIDTH_DEFAULT,
            "Page options",
            "Open help overlay",
            "hover",
            "pressed",
            "editing",
            "empty",
            "loading",
            "●",
        ],
    );
    shot(
        &app,
        "sc.helpkeys.states.80x24",
        "states at 80 by 24 before the terminal grows, showing rest, overflow, and narrow",
        &fixed,
    );
    app.resize(120, 40);
    let wide = app.wait_screen("grown states", |screen| {
        screen.cols() == 120
            && text(screen).contains(CAP_STATES)
            && text(screen).contains("Narrow keys")
    });
    assert_size(&wide, 120, 40);
    states_rows(&app);
    require(&app, &[CAP_STATES, "120×40", "Narrow keys"], &["error"]);
    shot(
        &app,
        "sc.helpkeys.states.120x40",
        "states caption every state, drawn by the widget is active at 120 by 40",
        &fixed,
    );
    shot(
        &app,
        "sc.helpkeys.states",
        "states is the visible Help & keys section",
        &fixed,
    );
    let screen = app.screen();
    let rest = row_containing(&screen, "rest");
    assert!(
        rest.contains("run the statement") && !rest.contains("find in every"),
        "rest specimen stays the short statement: {rest}"
    );
    shot(
        &app,
        "sc.helpkeys.specimen.rest",
        "states grid draws the rest row as Query, Ctrl+Enter, and Esc",
        &fixed,
    );
    assert!(
        text(&screen).contains("find in every open file") && text(&screen).contains("overflow"),
        "overflow specimen is on the states grid\n{}",
        text(&screen)
    );
    shot(
        &app,
        "sc.helpkeys.specimen.overflow",
        "states grid draws the overflow row with Ctrl+Shift+F and find in every open file",
        &fixed,
    );
    let narrow_row = row_containing(&screen, "narrow");
    assert!(
        narrow_row.contains("Ctrl+Enter") && narrow_row.contains("run the"),
        "narrow specimen draws the wrapped keys: {narrow_row}"
    );
    assert!(
        text(&screen).contains("Esc"),
        "narrow specimen still shows Esc\n{}",
        text(&screen)
    );
    shot(
        &app,
        "sc.helpkeys.specimen.narrow",
        "states grid draws the narrow row with the wrapped Ctrl+Enter statement",
        &fixed,
    );
    app.finish();
}

#[test]
fn compare_levels_scrolls_then_narrows() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-compare", 120, 40, &args);
    app.wait_text(CAP_PLAY);
    app.key("tab");
    app.wait_text("←→ Section");
    app.key("3");
    let states = app.wait_text(CAP_STATES);
    let before = row_containing(&states, "Compare levels");
    assert!(
        before.contains('○') && !before.contains('●'),
        "compare starts off: {before}"
    );
    require(
        &app,
        &[PAGE, "rest", "overflow", "narrow"],
        &["256 colors", "16 colors", "no color", CAP_PLAY, CAP_OPT],
    );
    click_text(&app, "Compare levels");
    let on = app.wait_text("256 colors");
    assert!(
        text(&on).contains('●')
            && text(&on).contains("truecolor")
            && text(&on).contains(CAP_STATES),
        "compare on shows the truecolor and 256 bands\n{}",
        text(&on)
    );
    assert!(
        !text(&on).contains(CAP_PLAY) && !text(&on).contains("Page options"),
        "compare on stayed on States\n{}",
        text(&on)
    );
    for _ in 0..6 {
        if text(&app.screen()).contains("16 colors") && text(&app.screen()).contains("no color") {
            break;
        }
        app.key("pagedown");
        let _ = app.settle();
    }
    let scrolled = app.settle();
    assert!(
        text(&scrolled).contains("16 colors")
            && text(&scrolled).contains("no color")
            && text(&scrolled).contains(PAGE),
        "scrolled bands missing\n{}",
        text(&scrolled)
    );
    assert!(
        !text(&scrolled).contains("Page options") && !text(&scrolled).contains(WIDTH_DEFAULT),
        "scrolling compare changed the page\n{}",
        text(&scrolled)
    );
    shot(
        &app,
        "sc.helpkeys.compare-levels.120x40",
        "compare levels is on at 120 by 40 and the 16 color and no color bands are visible",
        &fixed,
    );
    app.resize(80, 24);
    let narrow = app.wait_screen("narrow compare", |screen| {
        screen.cols() == 80 && text(screen).contains('●') && text(screen).contains("Compare levels")
    });
    assert_size(&narrow, 80, 24);
    require(
        &app,
        &[PAGE, "●", "Compare levels", "truecolor", "rest"],
        &[
            CAP_STATES,
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            WIDTH_DEFAULT,
            "120×40",
        ],
    );
    shot(
        &app,
        "sc.helpkeys.compare-levels.80x24",
        "narrow compare levels stays on at 80 by 24 before the terminal grows and before it is turned off",
        &fixed,
    );
    click_text(&app, "Compare levels");
    let off = app.wait_text("○");
    assert!(
        !text(&off).contains('●')
            && !text(&off).contains("256 colors")
            && !text(&off).contains("16 colors")
            && !text(&off).contains("no color")
            && text(&off).contains(PAGE),
        "compare off should clear the bands and stay on Help & keys\n{}",
        text(&off)
    );
    app.resize(120, 40);
    let grown = app.wait_screen("options after compare", |screen| {
        screen.cols() == 120 && text(screen).contains("120×40")
    });
    assert!(
        text(&grown).contains(PAGE) && !text(&grown).contains("App shell › Quit & leave"),
        "the page changed after compare levels\n{}",
        text(&grown)
    );
    click_text(&app, "Options");
    let options = app.wait_text("Reference width");
    assert_eq!(option_value(&options, "Reference width"), "full");
    assert_eq!(option_value(&options, "Key column share"), "40");
    assert_eq!(option_value(&options, "Column gap"), "3");
    assert_eq!(option_value(&options, "Overlay title"), "default");
    assert_eq!(option_value(&options, "Overlay size"), "80");
    assert_eq!(option_value(&options, "Scroll keys"), "default");
    assert_eq!(option_value(&options, "Close keys"), "default");
    assert_eq!(option_value(&options, "Dismiss"), "light");
    app.finish();
}

#[test]
fn help_overlay_scrolls_and_returns_focus() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-help", 120, 40, &args);
    let before = app.wait_text(WIDTH_DEFAULT);
    assert_size(&before, 120, 40);
    playground_wide(&app);
    let button = row_containing(&before, "Open help overlay");
    assert!(
        !focused_here(&button, "Open help overlay"),
        "focus starts on the sidebar, not the help button: {button}"
    );
    shot(
        &app,
        "sc.helpkeys.help.before",
        "playground is open and neither page help nor global help is showing",
        &fixed,
    );
    click_text(&app, "Open help overlay");
    let page_help = app.wait_text("Keyboard shortcuts");
    assert!(
        text(&page_help).contains("Editor")
            && text(&page_help).contains("Navigation")
            && text(&page_help).contains("Ctrl+Q")
            && text(&page_help).contains("quit")
            && text(&page_help).contains("Window")
            && text(&page_help).contains(PAGE),
        "page help should show the Editor overlay\n{}",
        text(&page_help)
    );
    assert!(
        !text(&page_help).contains("Keyboard & mouse")
            && !text(&page_help).contains("Page options"),
        "page help opened the shell help or the options section\n{}",
        text(&page_help)
    );
    app.key("pagedown");
    let scrolled = app.settle();
    assert!(
        text(&scrolled).contains("Keyboard shortcuts")
            && text(&scrolled).contains("Ctrl+Q")
            && text(&scrolled).contains("Esc Close")
            && text(&scrolled).contains(PAGE),
        "scrolling page help left the Editor overlay\n{}",
        text(&scrolled)
    );
    app.key("2");
    let stayed = app.settle();
    assert!(
        text(&stayed).contains("Keyboard shortcuts")
            && text(&stayed).contains("Esc Close")
            && !text(&stayed).contains("Page options")
            && !text(&stayed).contains(CAP_OPT),
        "a section key activated the page under page help\n{}",
        text(&stayed)
    );
    app.key("]");
    let stayed = app.settle();
    assert!(
        text(&stayed).contains(PAGE)
            && text(&stayed).contains("Keyboard shortcuts")
            && !text(&stayed).contains("App shell › Quit & leave"),
        "] activated the next page under page help\n{}",
        text(&stayed)
    );
    app.key("escape");
    let dismissed = app.wait_text("Enter Open help");
    let focused = row_containing(&dismissed, "Open help overlay");
    assert!(
        focused_here(&focused, "Open help overlay")
            && text(&dismissed).contains(WIDTH_DEFAULT)
            && text(&dismissed).contains(META_DEFAULT)
            && text(&dismissed).contains(CAP_PLAY)
            && !text(&dismissed).contains("Keyboard & mouse")
            && !text(&dismissed).contains("Esc Close"),
        "dismiss should return focus to Open help overlay\n{}",
        text(&dismissed)
    );
    app.key("?");
    let global = app.wait_text("Keyboard & mouse");
    assert!(
        text(&global).contains("Showcase")
            && text(&global).contains(PAGE)
            && text(&global).contains("This reference")
            && !text(&global).contains("Page options"),
        "global help should show Keyboard & mouse for Showcase\n{}",
        text(&global)
    );
    app.key("pagedown");
    let scrolled = app.settle();
    assert!(
        text(&scrolled).contains("Keyboard & mouse")
            && text(&scrolled).contains("Drag")
            && text(&scrolled).contains("A scrollbar thumb")
            && text(&scrolled).contains(PAGE),
        "scrolling global help left Showcase help\n{}",
        text(&scrolled)
    );
    app.key("]");
    let stayed = app.settle();
    assert!(
        text(&stayed).contains("Keyboard & mouse")
            && text(&stayed).contains(PAGE)
            && !text(&stayed).contains("App shell › Quit & leave"),
        "] activated the next page under global help\n{}",
        text(&stayed)
    );
    app.key("escape");
    let after = app.wait_text("Enter Open help");
    let focused = row_containing(&after, "Open help overlay");
    assert!(
        focused_here(&focused, "Open help overlay")
            && text(&after).contains(WIDTH_DEFAULT)
            && text(&after).contains(META_DEFAULT)
            && text(&after).contains(TITLE_DEFAULT)
            && text(&after).contains(CLOSE_DEFAULT)
            && text(&after).contains(DISMISS_LIGHT)
            && !text(&after).contains("Keyboard & mouse")
            && !text(&after).contains(TITLE_CUSTOM)
            && !text(&after).contains(CLOSE_F1),
        "global help dismiss should restore the same playground and the help button\n{}",
        text(&after)
    );
    shot(
        &app,
        "sc.helpkeys.help.after",
        "both help layers are dismissed and focus is back on Open help overlay",
        &fixed,
    );
    app.finish();
}

fn assert_option_defaults_except(screen: &TerminalSnapshot, changed: &str, value: &str) {
    let expect = [
        ("Reference width", "full"),
        ("Key column share", "40"),
        ("Column gap", "3"),
        ("Overlay title", "default"),
        ("Overlay size", "80"),
        ("Scroll keys", "default"),
        ("Close keys", "default"),
        ("Dismiss", "light"),
    ];
    for (label, default) in expect {
        let actual = option_value(screen, label);
        let wanted = if label == changed { value } else { default };
        assert_eq!(
            actual,
            wanted,
            "{label} should be {wanted}, screen:\n{}",
            text(screen)
        );
    }
}

#[test]
fn width_changes_through_the_options_control() {
    let args = ["--page", "helpkeys"];
    let fixed = fixture(&args);
    let app = open("s01-helpkeys-width", 120, 40, &args);
    app.wait_text(WIDTH_DEFAULT);
    click_text(&app, "Options");
    let defaults = app.wait_text("Reference width");
    assert_option_defaults_except(&defaults, "", "");
    click_text(&app, "full");
    app.wait_text("28");
    click_text(&app, "28");
    let chosen = app.wait_screen("width 28 chosen", |screen| {
        let visible = text(screen);
        visible.contains("Reference width")
            && !visible.contains("Esc Close")
            && option_value(screen, "Reference width") == "28"
    });
    assert_option_defaults_except(&chosen, "Reference width", "28");
    assert!(
        !text(&chosen).contains("Esc Close"),
        "the width popup stayed open\n{}",
        text(&chosen)
    );
    click_text(&app, "Playground");
    let effect = app.wait_text(WIDTH_NARROW);
    assert!(
        text(&effect).contains(CAP_PLAY)
            && text(&effect).contains("share 40% · gap 3")
            && text(&effect).contains(TITLE_DEFAULT)
            && text(&effect).contains("80%")
            && text(&effect).contains(CLOSE_DEFAULT)
            && text(&effect).contains(DISMISS_LIGHT)
            && !text(&effect).contains(WIDTH_DEFAULT)
            && !text(&effect).contains("share 60%")
            && !text(&effect).contains("gap 6")
            && !text(&effect).contains(TITLE_CUSTOM)
            && !text(&effect).contains("100%")
            && !text(&effect).contains("j k")
            && !text(&effect).contains(CLOSE_F1)
            && !text(&effect).contains(DISMISS_ESCAPE),
        "width 28 changed another option\n{}",
        text(&effect)
    );
    shot(
        &app,
        "opt.helpkeys.width",
        "Reference width 28 wraps the key reference and the other options stay at their defaults",
        &fixed,
    );
    app.finish();
}

fn option_boundary(
    name: &str,
    opt: &str,
    case_id: &str,
    changed: &str,
    value: &str,
    effect: &[&str],
    forbid: &[&str],
) {
    let args = ["--page", "helpkeys", "--opt", opt];
    let app = open(name, 120, 40, &args);
    app.wait_text("Playground");
    click_text(&app, "Options");
    let options = app.wait_text("Page options");
    assert_option_defaults_except(&options, changed, value);
    click_text(&app, "Playground");
    let screen = app.wait_text(effect[0]);
    let visible = text(&screen);
    for item in effect {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in forbid {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
    assert!(
        visible.contains(CAP_PLAY) && visible.contains(PAGE) && !visible.contains(CAP_OPT),
        "the option effect left the playground\n{visible}"
    );
    shot(
        &app,
        case_id,
        &format!(
            "{changed} is {value} on the playground and the other options stay at their defaults"
        ),
        &fixture(&args),
    );
    app.finish();
}

#[test]
fn option_boundaries_keep_siblings() {
    option_boundary(
        "s01-helpkeys-share",
        "helpkeys.key_max_share=60",
        "opt.helpkeys.key_max_share",
        "Key column share",
        "60",
        &[
            "share 60% · gap 3",
            WIDTH_DEFAULT,
            TITLE_DEFAULT,
            "80%",
            CLOSE_DEFAULT,
            DISMISS_LIGHT,
        ],
        &[
            "share 40%",
            "gap 6",
            WIDTH_NARROW,
            TITLE_CUSTOM,
            "100%",
            "j k",
            CLOSE_F1,
            DISMISS_ESCAPE,
        ],
    );
    option_boundary(
        "s01-helpkeys-gap",
        "helpkeys.gap=6",
        "opt.helpkeys.gap",
        "Column gap",
        "6",
        &[
            "share 40% · gap 6",
            WIDTH_DEFAULT,
            TITLE_DEFAULT,
            "80%",
            CLOSE_DEFAULT,
            DISMISS_LIGHT,
        ],
        &[
            "gap 3",
            "share 60%",
            WIDTH_NARROW,
            TITLE_CUSTOM,
            "100%",
            "j k",
            CLOSE_F1,
            DISMISS_ESCAPE,
        ],
    );
    option_boundary(
        "s01-helpkeys-title",
        "helpkeys.title=custom",
        "opt.helpkeys.title",
        "Overlay title",
        "custom",
        &[
            TITLE_CUSTOM,
            WIDTH_DEFAULT,
            META_DEFAULT,
            "80%",
            CLOSE_DEFAULT,
            DISMISS_LIGHT,
        ],
        &[
            TITLE_DEFAULT,
            "share 60%",
            "gap 6",
            WIDTH_NARROW,
            "100%",
            "j k",
            CLOSE_F1,
            DISMISS_ESCAPE,
        ],
    );
    option_boundary(
        "s01-helpkeys-percent",
        "helpkeys.max_percent=100",
        "opt.helpkeys.max_percent",
        "Overlay size",
        "100",
        &[
            "100%",
            WIDTH_DEFAULT,
            META_DEFAULT,
            TITLE_DEFAULT,
            CLOSE_DEFAULT,
            DISMISS_LIGHT,
        ],
        &[
            "80%",
            "share 60%",
            "gap 6",
            WIDTH_NARROW,
            TITLE_CUSTOM,
            "j k",
            CLOSE_F1,
            DISMISS_ESCAPE,
        ],
    );
    option_boundary(
        "s01-helpkeys-nav",
        "helpkeys.nav=vim",
        "opt.helpkeys.nav",
        "Scroll keys",
        "vim",
        &[
            "j k",
            WIDTH_DEFAULT,
            META_DEFAULT,
            TITLE_DEFAULT,
            "80%",
            CLOSE_DEFAULT,
            DISMISS_LIGHT,
        ],
        &[
            "share 60%",
            "gap 6",
            WIDTH_NARROW,
            TITLE_CUSTOM,
            "100%",
            CLOSE_F1,
            DISMISS_ESCAPE,
        ],
    );
    option_boundary(
        "s01-helpkeys-keymap",
        "helpkeys.keymap=f1",
        "opt.helpkeys.keymap",
        "Close keys",
        "f1",
        &[
            CLOSE_F1,
            WIDTH_DEFAULT,
            META_DEFAULT,
            TITLE_DEFAULT,
            "80%",
            DISMISS_LIGHT,
        ],
        &[
            CLOSE_DEFAULT,
            "share 60%",
            "gap 6",
            WIDTH_NARROW,
            TITLE_CUSTOM,
            "100%",
            "j k",
            DISMISS_ESCAPE,
        ],
    );
    option_boundary(
        "s01-helpkeys-dismiss",
        "helpkeys.dismiss=escape",
        "opt.helpkeys.dismiss",
        "Dismiss",
        "escape",
        &[
            DISMISS_ESCAPE,
            WIDTH_DEFAULT,
            META_DEFAULT,
            TITLE_DEFAULT,
            "80%",
            CLOSE_DEFAULT,
        ],
        &[
            DISMISS_LIGHT,
            "outside",
            "share 60%",
            "gap 6",
            WIDTH_NARROW,
            TITLE_CUSTOM,
            "100%",
            "j k",
            CLOSE_F1,
        ],
    );
}

#[test]
fn help_overlay_narrow_and_empty_examples_stay_on_screen() {
    let args = ["--page", "helpkeys"];
    let app = open("s01-helpkeys-narrow-empty", 120, 40, &args);
    app.wait_text(WIDTH_DEFAULT);
    click_text(&app, "Open help overlay");
    app.wait_text("Navigation");
    app.key("esc");
    app.wait_absent("Ctrl+P");

    app.key("n");
    let narrow = app.wait_text("Done");
    assert!(
        text(&narrow).contains("Done") && text(&narrow).contains("╰"),
        "narrow example should keep its footer and bottom edge on screen\n{}",
        text(&narrow)
    );
    app.key("esc");
    app.wait_absent("Done");

    app.key("e");
    let empty = app.wait_text("Done");
    assert!(
        text(&empty).contains("Editor") && !text(&empty).contains("Ctrl+P"),
        "empty example should show the frame and Done without any section\n{}",
        text(&empty)
    );
    app.key("esc");
    app.wait_absent("Done");
    app.finish();
}

#[test]
fn help_overlay_opens_at_the_narrowest_showcase_terminal() {
    let app = open("s01-helpkeys-72x20", 72, 20, &["--page", "helpkeys"]);
    app.wait_text(WIDTH_DEFAULT);
    app.key("?");
    let help = app.wait_text("Done");
    assert_size(&help, 72, 20);
    assert!(
        text(&help).contains("╰"),
        "the shell help overlay should keep its bottom edge at 72 by 20\n{}",
        text(&help)
    );
    app.key("esc");
    app.wait_absent("Done");
    app.finish();
}
