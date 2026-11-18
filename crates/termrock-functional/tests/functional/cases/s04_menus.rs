//! Menus route, sections, specimens, compare levels, options, and menus.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s04_menus/` and each case id's dots become underscores.

use tui_test::{MouseButton, MouseOptions, TerminalSnapshot};

use crate::support::{Launch, Point, SHOWCASE, cell_bg, find_all, row_text, text, try_launch};

const CRUMB: &str = "Navigation › Menus";
const TABS_CRUMB: &str = "Navigation › Tabs";
const TITLE: &str = "Menus";
const BLURB_WIDE: &str = "keys and press-drag-r…";
const BLURB_NARROW: &str = "reasons, danger…";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const KEYS_ON: &str = "right-click · Shift+F10 · Menu key — a click moves the cursor";
const CHOOSE: &str = "Enter Choose";
const FILES: &[&str] = &[
    "report.pdf",
    "notes.md",
    "build.log",
    "archive.zip",
    "photo.png",
    "README.md",
    "LICENSE",
];

const DRAWN: &[(&str, &str)] = &[
    ("sc.menus.specimen.rest", "rest"),
    ("sc.menus.specimen.hover", "hover"),
    ("sc.menus.specimen.pressed", "pressed"),
    ("sc.menus.specimen.disabled", "disabled"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s04_menus/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
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

fn fact_row(screen: &TerminalSnapshot, label: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with(label) && !trimmed.starts_with("Actions")
        })
        .unwrap_or_else(|| panic!("fact {label:?} missing\n{}", text(screen)))
}

fn option_value(screen: &TerminalSnapshot, label: &str) -> String {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 20 && hit.y >= 6)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "option {label:?} got {hits:?}\n{}",
        text(screen)
    );
    screen
        .row(u32::from(hits[0].y) + 1)
        .expect("option value row")
        .iter()
        .skip(usize::from(hits[0].x))
        .map(|item| item.text())
        .collect::<String>()
}

fn value_is(screen: &TerminalSnapshot, label: &str, expected: &str) {
    let value = option_value(screen, label);
    let word = value.split_whitespace().next().unwrap_or("");
    assert_eq!(word, expected, "{label} value {value:?}");
}

fn toggle_is(screen: &TerminalSnapshot, label: &str, on: bool) {
    let row = row_with(screen, label);
    if on {
        assert!(
            row.contains(" on") && !row.contains(" off"),
            "{label} toggle: {row}"
        );
    } else {
        assert!(
            row.contains(" off") && !row.contains(" on"),
            "{label} toggle: {row}"
        );
    }
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
}

fn cell(screen: &TerminalSnapshot, x: u16, y: u16) -> String {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|item| item.text().to_string())
        .unwrap_or_default()
}

fn locate(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
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
    hits[0]
}

fn click_where(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let hit = locate(app, label, pred);
    app.click(hit);
    hit
}

fn click_tab(app: &crate::support::App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &crate::support::App, label: &str) {
    click_where(app, label, |hit| hit.x < 24 && hit.y >= 4);
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 8 && hit.x >= 20 && hit.x < 40)
}

/// Wait out the 140ms press flash of a released click, then bump the width so
/// the shell redraws and return. 80 columns do not draw
/// the size caption, so the narrow return is checked from the screen size.
fn quiesce(app: &crate::support::App) {
    app.flash_expired();
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    if cols >= 100 {
        let bumped = cols + 1;
        app.resize(bumped, rows);
        app.wait_text(&format!("{bumped}×{rows}"));
        app.resize(cols, rows);
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.resize(121, rows);
        app.wait_text(&format!("121×{rows}"));
        app.resize(cols, rows);
        app.wait_screen("narrow size restored", |screen| {
            screen.cols() == u32::from(cols)
                && screen.rows() == u32::from(rows)
                && text(screen).contains(TITLE)
        });
    }
}

fn wheel_to(app: &crate::support::App, needle: &str, down: bool) {
    for _ in 0..80 {
        let screen = app.screen();
        if text(&screen).contains(needle) {
            return;
        }
        let x = u16::try_from(screen.cols())
            .expect("cols")
            .saturating_sub(1);
        app.wheel(Point { x, y: 12 }, down, 1);
        app.settle();
    }
    app.wait_text(needle);
}

fn go_options(app: &crate::support::App) {
    click_tab(app, "Options");
    app.wait_text("Page options");
}

fn go_play(app: &crate::support::App) {
    click_tab(app, "Playground");
    app.wait_text("report.pdf");
}

fn go_states(app: &crate::support::App) {
    click_tab(app, "States");
    app.wait_text("Compare levels");
}

fn pick(app: &crate::support::App, label: &str, downs: u8) {
    let hit = locate(app, label, |item| item.x >= 24 && item.y >= 6);
    app.click(Point {
        x: hit.x,
        y: hit.y + 1,
    });
    app.wait_screen("select open", |screen| footer(screen).contains("Enter Set"));
    for _ in 0..downs {
        app.key("down");
    }
    app.key("enter");
    app.wait_screen("select closed", |screen| {
        !footer(screen).contains("Enter Set")
    });
}

fn file_row(app: &crate::support::App, label: &str) -> Point {
    locate(app, label, |hit| hit.x >= 24 && hit.y >= 8 && hit.y <= 16)
}

fn focus_file(app: &crate::support::App, label: &str) -> Point {
    let hit = file_row(app, label);
    app.click(hit);
    quiesce(app);
    hit
}

fn menu_is_open(screen: &TerminalSnapshot) -> bool {
    footer(screen).contains(CHOOSE)
}

fn menu_shows(screen: &TerminalSnapshot, name: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains('│') && line.contains(name)
    })
}

fn open_file_menu(app: &crate::support::App, label: &str) -> TerminalSnapshot {
    focus_file(app, label);
    app.key("shift+f10");
    app.wait_screen("file menu", menu_is_open)
}

fn close_menu(app: &crate::support::App) -> TerminalSnapshot {
    app.key("esc");
    app.wait_screen("menu closed", |screen| !menu_is_open(screen))
}

fn prepare_actions(app: &crate::support::App) -> Point {
    let button = locate(app, "Actions", |hit| {
        hit.x >= 24 && hit.y >= 8 && hit.y <= 22
    });
    app.click(button);
    app.wait_screen("actions menu from click", menu_is_open);
    close_menu(app);
    quiesce(app);
    button
}

fn open_actions(app: &crate::support::App) -> (Point, TerminalSnapshot) {
    let button = prepare_actions(app);
    app.key("enter");
    let screen = app.wait_screen("actions menu", menu_is_open);
    (button, screen)
}

fn right_seq(point: Point, kind: &str) -> String {
    let options = MouseOptions::new(MouseButton::Right);
    match kind {
        "down" => tui_test::input::mouse::down(point.x, point.y, options),
        "up" => tui_test::input::mouse::up(point.x, point.y, options),
        "drag" => tui_test::input::mouse::drag_motion(point.x, point.y, options),
        "click" => tui_test::input::mouse::click(point.x, point.y, options),
        other => panic!("unknown pointer {other}"),
    }
}

fn right(app: &crate::support::App, point: Point, kind: &str) {
    app.type_text(&right_seq(point, kind));
}

fn menu_item(app: &crate::support::App, label: &str) -> Point {
    locate(app, label, |hit| hit.x >= 24 && hit.y >= 8)
}

fn box_width(screen: &TerminalSnapshot, at: Point) -> u16 {
    let mut left = None;
    for x in (0..at.x).rev() {
        if cell(screen, x, at.y) == "│" {
            left = Some(x);
            break;
        }
    }
    let mut right = None;
    for x in at.x..u16::try_from(screen.cols()).expect("cols") {
        if cell(screen, x, at.y) == "│" {
            right = Some(x);
            break;
        }
    }
    let (Some(left), Some(right)) = (left, right) else {
        panic!(
            "menu box missing around {at:?}\n{}",
            row_text(screen, u32::from(at.y))
        );
    };
    right - left + 1
}

fn side_focused(app: &crate::support::App, label: &str) {
    let side = locate(app, label, |hit| hit.x < 24);
    let side_row = row_text(&app.screen(), u32::from(side.y));
    assert!(
        side_row.contains('▎'),
        "sidebar focus left {label}: {side_row}"
    );
}

fn assert_menu_defaults(screen: &TerminalSnapshot) {
    value_is(screen, "Actions placement", "below");
    value_is(screen, "Fit", "menu");
    value_is(screen, "Edge margin", "1");
    value_is(screen, "Min width", "8");
    value_is(screen, "Max rows", "all");
    value_is(screen, "Nav keys", "desktop");
    value_is(screen, "Dismiss", "light");
    value_is(screen, "Other keys", "close-and-pass");
    value_is(screen, "Extra key", "none");
    toggle_is(screen, "Title", true);
    toggle_is(screen, "Release selects", true);
    toggle_is(screen, "Hide page focus", true);
    toggle_is(screen, "Wrap", true);
    toggle_is(screen, "Type-ahead", false);
    toggle_is(screen, "Click moves cursor", true);
    toggle_is(screen, "Shift+F10", true);
    toggle_is(screen, "Menu key", true);
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "menus"];
    let app = open("s04-menus-route-page", 120, 40, &args);
    let screen = app.wait_text("report.pdf");
    see(
        &screen,
        &[
            CRUMB,
            TITLE,
            BLURB_WIDE,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Files",
            "right-click · Shift+F10",
            "Actions menu",
            "click · Enter",
            "Actions ▾",
            "document",
            "read-only",
            "Last",
            "none yet",
            KEYS_ON,
            "Right-press a file, drag onto an item,",
            "release: the release chooses it.",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "press-drag-release",
            OVERVIEW,
            CHOOSE,
            "Sort by name",
        ],
    );
    for file in FILES {
        assert!(text(&screen).contains(file), "missing file {file}");
    }
    side_focused(&app, "› Menus");
    shot(
        &app,
        "sc.menus.route",
        "--page menus opens the playground with the sidebar on Menus",
        &args,
    );
    app.finish();

    let app = open("s04-menus-route-keys", 120, 40, &["--page", "tabs"]);
    let tabs = app.wait_text(TABS_CRUMB);
    see(
        &tabs,
        &[TABS_CRUMB, "Tabs", "nested strips"],
        &[CRUMB, "report.pdf", "Page options"],
    );
    app.key("]");
    let entered = app.wait_text(CRUMB);
    see(
        &entered,
        &[CRUMB, BLURB_WIDE, "report.pdf", "Actions ▾", KEYS_ON],
        &[
            TABS_CRUMB,
            "nested strips",
            "Page options",
            "Compare levels",
        ],
    );
    app.key("[");
    let left = app.wait_text(TABS_CRUMB);
    see(
        &left,
        &[TABS_CRUMB, "nested strips"],
        &[CRUMB, "report.pdf"],
    );
    app.key("]");
    let again = app.wait_text("report.pdf");
    see(
        &again,
        &[CRUMB, "Actions ▾"],
        &[TABS_CRUMB, "nested strips"],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "report.pdf"]);
    click_side(&app, "Navigation");
    app.wait_text("Lists");
    click_side(&app, "Menus");
    let pointed = app.wait_text("LICENSE");
    see(
        &pointed,
        &[CRUMB, BLURB_WIDE, "report.pdf", "README.md", "Actions ▾"],
        &[OVERVIEW, "Page options", "Compare levels", TABS_CRUMB],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "menus"];
    let app = open("s04-menus-sections", 80, 24, &args);
    let narrow_play = app.wait_text("report.pdf");
    see(
        &narrow_play,
        &[
            CRUMB,
            TITLE,
            BLURB_NARROW,
            "Playground",
            "report.pdf",
            "LICENSE",
            "read-only",
            "Actions ▾",
            "right-click · Shift+F10",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "separators",
            "Last",
            "none yet",
            "press-drag-release",
        ],
    );
    assert_eq!((narrow_play.cols(), narrow_play.rows()), (80, 24));
    side_focused(&app, "› Menus");
    shot(
        &app,
        "sc.menus.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    go_options(&app);
    let narrow_opt = app.wait_text("Page options");
    value_is(&narrow_opt, "Actions placement", "below");
    value_is(&narrow_opt, "Fit", "menu");
    value_is(&narrow_opt, "Edge margin", "1");
    value_is(&narrow_opt, "Min width", "8");
    value_is(&narrow_opt, "Max rows", "all");
    toggle_is(&narrow_opt, "Title", true);
    toggle_is(&narrow_opt, "Release selects", true);
    toggle_is(&narrow_opt, "Hide page focus", true);
    see(
        &narrow_opt,
        &[CRUMB, "Page options", "Actions placement", "below", "Fit"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "report.pdf",
            WIDE,
            "truecolor",
            "flush",
            "vim",
        ],
    );
    quiesce(&app);
    let narrow_opt = app.settle();
    assert_eq!((narrow_opt.cols(), narrow_opt.rows()), (80, 24));
    assert!(text(&narrow_opt).contains("Page options"));
    shot(
        &app,
        "sc.menus.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    go_states(&app);
    let narrow_states = app.wait_screen("states at 80 by 24", |screen| {
        screen.cols() == 80 && grid_label(screen, "rest") && grid_label(screen, "hover")
    });
    see(
        &narrow_states,
        &[
            CRUMB,
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "Open",
            "Rename",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            WIDE,
            "truecolor",
            "pressed",
            "disabled",
            "overflow",
            "narrow",
            "busy",
            "loading",
            "report.pdf",
        ],
    );
    assert!(
        grid_label(&narrow_states, "rest")
            && grid_label(&narrow_states, "hover")
            && !grid_label(&narrow_states, "pressed")
            && !grid_label(&narrow_states, "disabled")
            && !grid_label(&narrow_states, "overflow")
            && !grid_label(&narrow_states, "narrow")
    );
    quiesce(&app);
    let narrow_states = app.settle();
    assert_eq!((narrow_states.cols(), narrow_states.rows()), (80, 24));
    assert!(grid_label(&narrow_states, "rest") && grid_label(&narrow_states, "hover"));
    assert!(!text(&narrow_states).contains("pressed"));
    shot(
        &app,
        "sc.menus.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "pressed")
            && grid_label(screen, "disabled")
    });
    see(
        &wide_states,
        &[
            CRUMB,
            TITLE,
            BLURB_WIDE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "rest",
            "hover",
            "pressed",
            "disabled",
            "Commands",
            "Reasons",
            "README.md",
            "Delete",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "overflow",
            "narrow",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "256 colors",
            "report.pdf",
        ],
    );
    quiesce(&app);
    let wide_states = app.settle();
    assert!(grid_label(&wide_states, "rest"));
    assert!(grid_label(&wide_states, "pressed"));
    assert!(grid_label(&wide_states, "disabled"));
    assert!(!grid_label(&wide_states, "overflow"));
    assert!(!grid_label(&wide_states, "narrow"));
    shot(
        &app,
        "sc.menus.states.120x40",
        "states caption and the drawn Menus specimens that fit at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.menus.states",
        "states is the visible Menus section",
        &args,
    );
    for (case_id, label) in DRAWN {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the screen before its capture\n{}",
            text(&app.screen())
        );
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES],
            &["● Compare levels", "Page options", "overflow", "narrow"],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &args,
        );
    }

    wheel_to(&app, "overflow", true);
    wheel_to(&app, "narrow", true);
    let scrolled = app.settle();
    assert!(grid_label(&scrolled, "overflow"), "overflow label clipped");
    assert!(grid_label(&scrolled, "narrow"), "narrow label clipped");
    see(
        &scrolled,
        &["overflow", "narrow"],
        &["● Compare levels", "Page options", "256 colors"],
    );
    shot(
        &app,
        "sc.menus.specimen.overflow",
        "states grid row is visible and compare levels is off",
        &args,
    );
    shot(
        &app,
        "sc.menus.specimen.narrow",
        "states grid row is visible and compare levels is off",
        &args,
    );

    go_options(&app);
    let wide_opt = app.wait_text(CAP_OPT);
    assert_menu_defaults(&wide_opt);
    see(
        &wide_opt,
        &[
            CRUMB,
            CAP_OPT,
            WIDE,
            "Page options",
            "Actions placement",
            "below",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "report.pdf",
            "flush",
            "vim",
        ],
    );
    quiesce(&app);
    let wide_opt = app.settle();
    assert!(text(&wide_opt).contains(CAP_OPT));
    assert!(text(&wide_opt).contains(WIDE));
    shot(
        &app,
        "sc.menus.options.120x40",
        "options caption and the default values at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.menus.options",
        "options is the visible Menus section",
        &args,
    );

    go_play(&app);
    let wide_play = app.wait_screen("playground at 120 by 40", |screen| {
        text(screen).contains(CAP_PLAY) && text(screen).contains("report.pdf")
    });
    see(
        &wide_play,
        &[
            CRUMB,
            BLURB_WIDE,
            CAP_PLAY,
            WIDE,
            "report.pdf",
            "README.md",
            "LICENSE",
            "Actions ▾",
            "Last",
            "none yet",
            KEYS_ON,
            "Right-press a file, drag onto an item,",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "press-drag-release",
            CHOOSE,
        ],
    );
    quiesce(&app);
    let wide_play = app.settle();
    assert!(text(&wide_play).contains(CAP_PLAY));
    assert!(text(&wide_play).contains(WIDE));
    shot(
        &app,
        "sc.menus.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.menus.playground",
        "playground is the visible Menus section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "menus"];
    let app = open("s04-menus-compare", 120, 40, &args);
    go_states(&app);
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
    click_where(&app, "Compare levels", |hit| hit.y < 16 && hit.x >= 24);
    app.wait_text("● Compare levels");
    wheel_to(&app, "no color", true);
    let bands = app.settle();
    see(
        &bands,
        &[CRUMB, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    assert_eq!((bands.cols(), bands.rows()), (120, 40));
    shot(
        &app,
        "sc.menus.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(CRUMB)
    });
    if !text(&narrow).contains("no color") {
        wheel_to(&app, "no color", true);
    }
    let narrow = app.settle();
    assert_eq!(
        (narrow.cols(), narrow.rows()),
        (80, 24),
        "narrow compare capture left 80 by 24"
    );
    see(
        &narrow,
        &[CRUMB, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.menus.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );

    wheel_to(&app, "● Compare levels", false);
    click_where(&app, "● Compare levels", |hit| hit.y < 20);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[CRUMB, "○ Compare levels"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    go_options(&app);
    let options = app.wait_text(CAP_OPT);
    assert_menu_defaults(&options);
    see(
        &options,
        &["below", "Page options", CAP_OPT, "menu"],
        &["flush", "Compare levels", "vim", "swallow"],
    );
    app.finish();
}

#[test]
fn menus_open_choose_dismiss_and_readonly() {
    let args = ["--page", "menus"];
    let app = open("s04-menus-menus", 120, 40, &args);
    let opened = open_file_menu(&app, "report.pdf");
    see(
        &opened,
        &[
            "report.pdf",
            "Open",
            "Rename",
            "Duplicate",
            "Copy path",
            "Share…",
            "Move to Trash",
            "row (keyboard)",
            "none yet",
            CHOOSE,
        ],
        &["Page options", "New file", "Empty Trash"],
    );
    assert!(
        find_all(&opened, "report.pdf").len() >= 2,
        "the open menu has no title"
    );
    let action = fact_row(&opened, "Action");
    assert!(
        action.contains("none yet") && !action.contains("Chosen(0)"),
        "menu opened with a result: {action}"
    );
    shot(
        &app,
        "sc.menus.menus.open",
        "file menu is open on report.pdf before a choice or a dismiss",
        &args,
    );

    let dismissed = close_menu(&app);
    let action = fact_row(&dismissed, "Action");
    assert!(
        action.contains("Dismissed") && !action.contains("Chosen"),
        "dismiss chose an item: {action}"
    );
    assert!(fact_row(&dismissed, "Request").contains("under its row (keyboard)"));
    assert!(!menu_is_open(&dismissed));
    see(
        &dismissed,
        &["Dismissed", "report.pdf", KEYS_ON],
        &[CHOOSE, "Chosen(0)", "Move to Trash"],
    );
    shot(
        &app,
        "sc.menus.menus.dismissed",
        "esc dismisses the file menu and does not choose an item",
        &args,
    );

    let chosen = open_file_menu(&app, "report.pdf");
    assert!(menu_is_open(&chosen));
    app.key("enter");
    let after = app.wait_screen("open chosen", |screen| !menu_is_open(screen));
    let action = fact_row(&after, "Action");
    assert!(
        action.contains("Chosen(0) · Open on report.pdf"),
        "choose result: {action}"
    );
    assert!(!action.contains("Dismissed"), "choose dismissed: {action}");
    see(
        &after,
        &["Chosen(0) · Open on report.pdf", "report.pdf"],
        &[CHOOSE, "Move to Trash", "╭"],
    );

    let read_only = open_file_menu(&app, "README.md");
    assert!(menu_is_open(&read_only), "readme menu did not open");
    app.key("down");
    let reason = app.wait_text("Read-only file");
    assert!(menu_is_open(&reason), "rename reason closed the menu");
    app.key("enter");
    let stayed = app.settle();
    assert!(
        menu_is_open(&stayed) && text(&stayed).contains("Read-only file"),
        "read-only rename ran\n{}",
        text(&stayed)
    );
    app.key("down");
    app.key("down");
    app.key("down");
    let share = app.wait_text("Sharing is off");
    assert!(menu_is_open(&share));
    app.key("enter");
    let share_stayed = app.settle();
    assert!(
        menu_is_open(&share_stayed) && text(&share_stayed).contains("Sharing is off"),
        "sharing ran\n{}",
        text(&share_stayed)
    );
    app.key("down");
    let trash = app.wait_screen("trash row", |screen| {
        menu_is_open(screen) && !text(screen).contains("Sharing is off")
    });
    assert!(
        text(&trash).contains("Move to Trash"),
        "trash row missing\n{}",
        text(&trash)
    );
    app.key("enter");
    let trash_stayed = app.settle();
    assert!(
        menu_is_open(&trash_stayed),
        "disabled trash ran\n{}",
        text(&trash_stayed)
    );
    let closed = close_menu(&app);
    let action = fact_row(&closed, "Action");
    assert!(
        action.contains("Dismissed") && !action.contains("Chosen"),
        "read-only action ran: {action}"
    );
    assert!(!action.contains("Rename on") && !action.contains("Move to Trash"));

    let file = file_row(&app, "notes.md");
    right(&app, file, "down");
    let dragged = app.wait_screen("press opened notes", menu_is_open);
    assert!(
        text(&dragged).contains("ointer (") && find_all(&dragged, "notes.md").len() >= 2,
        "pointer request:\n{}",
        text(&dragged)
    );
    let open_item = menu_item(&app, "Open");
    right(&app, open_item, "drag");
    right(&app, open_item, "up");
    let released = app.wait_screen("release chose", |screen| !menu_is_open(screen));
    let action = fact_row(&released, "Action");
    assert!(
        action.contains("Chosen(0) · Open on notes.md"),
        "release select: {action}"
    );
    assert!(!menu_is_open(&released));

    let again = open_file_menu(&app, "archive.zip");
    assert!(menu_is_open(&again));
    app.click(Point { x: 100, y: 30 });
    let outside = app.wait_screen("outside click", |screen| !menu_is_open(screen));
    let action = fact_row(&outside, "Action");
    assert!(
        action.contains("Dismissed") && !action.contains("Chosen"),
        "outside click chose: {action}"
    );

    let report = file_row(&app, "report.pdf");
    let license = file_row(&app, "LICENSE");
    right(&app, license, "click");
    let first = app.wait_screen("license menu", |screen| {
        menu_is_open(screen) && menu_shows(screen, "LICENSE")
    });
    assert!(
        text(&first).contains("ointer (") && !menu_shows(&first, "report.pdf"),
        "license pointer menu:\n{}",
        text(&first)
    );
    right(&app, report, "click");
    let second = app.wait_screen("menu beneath", |screen| {
        menu_is_open(screen)
            && menu_shows(screen, "report.pdf")
            && text(screen).contains("ointer (")
    });
    assert!(
        !menu_shows(&second, "LICENSE"),
        "the first menu stayed:\n{}",
        text(&second)
    );
    app.finish();
}

fn option_screen(name: &str) -> (crate::support::App, &'static [&'static str]) {
    let args: &'static [&'static str] = &["--page", "menus"];
    let app = open(name, 120, 40, args);
    go_options(&app);
    let screen = app.wait_text("Page options");
    assert_menu_defaults(&screen);
    (app, args)
}

#[test]
fn option_placement() {
    let (app, args) = option_screen("s04-menus-placement");
    pick(&app, "Actions placement", 2);
    let chosen = app.settle();
    value_is(&chosen, "Actions placement", "right");
    value_is(&chosen, "Fit", "menu");
    toggle_is(&chosen, "Title", true);
    go_play(&app);
    let (button, opened) = open_actions(&app);
    let item = menu_item(&app, "New file");
    assert!(
        item.x > button.x,
        "right placement did not move the menu: item {item:?} button {button:?}\n{}",
        text(&opened)
    );
    see(
        &app.screen(),
        &["New file", "Empty Trash", "Actions", CHOOSE],
        &["Page options", "Sort by name is missing"],
    );
    assert!(text(&app.screen()).contains("Sort by name"));
    assert!(text(&app.screen()).contains("Empty Trash"));
    shot(
        &app,
        "opt.menus.placement",
        "actions menu opens to the right of its button",
        args,
    );
    app.finish();
}

#[test]
fn option_fit() {
    let (app, args) = option_screen("s04-menus-fit");
    pick(&app, "Fit", 1);
    let chosen = app.settle();
    value_is(&chosen, "Fit", "flush");
    value_is(&chosen, "Actions placement", "below");
    go_play(&app);
    app.resize(120, 24);
    app.wait_text("120×24");
    let opened = open_actions(&app).1;
    assert_eq!((opened.cols(), opened.rows()), (120, 24));
    see(
        &opened,
        &["New file", "New folder", CHOOSE, "120×24"],
        &["Empty Trash", "Page options", "Show hidden files"],
    );
    shot(
        &app,
        "opt.menus.fit",
        "flush fit shrinks the actions menu when neither side can hold it",
        args,
    );
    app.finish();
}

#[test]
fn option_edge_margin() {
    let (app, args) = option_screen("s04-menus-edge");
    pick(&app, "Edge margin", 1);
    let chosen = app.settle();
    value_is(&chosen, "Edge margin", "2");
    value_is(&chosen, "Fit", "menu");
    value_is(&chosen, "Actions placement", "below");
    value_is(&chosen, "Min width", "8");
    toggle_is(&chosen, "Title", true);
    see(
        &chosen,
        &["Page options", "Edge margin", CAP_OPT],
        &["report.pdf", "Compare levels", "flush"],
    );
    quiesce(&app);
    let chosen = app.settle();
    value_is(&chosen, "Edge margin", "2");
    assert!(text(&chosen).contains(WIDE));
    shot(
        &app,
        "opt.menus.edge_margin",
        "edge margin select shows 2 and the other menu options stay at their defaults",
        args,
    );
    app.finish();
}

#[test]
fn option_min_width() {
    let (app, args) = option_screen("s04-menus-min-width");
    pick(&app, "Min width", 2);
    let chosen = app.settle();
    value_is(&chosen, "Min width", "24");
    value_is(&chosen, "Max rows", "all");
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    let item = menu_item(&app, "Open");
    let width = box_width(&opened, item);
    assert!(
        width >= 28,
        "min width 24 left the menu at {width}\n{}",
        row_text(&opened, u32::from(item.y))
    );
    see(
        &opened,
        &["Open", "Move to Trash", "report.pdf", CHOOSE],
        &["Page options"],
    );
    shot(
        &app,
        "opt.menus.min_width",
        "min width 24 widens the file menu past its labels",
        args,
    );
    app.finish();
}

#[test]
fn option_max_rows() {
    let (app, args) = option_screen("s04-menus-max-rows");
    pick(&app, "Max rows", 2);
    let chosen = app.settle();
    value_is(&chosen, "Max rows", "4");
    value_is(&chosen, "Actions placement", "below");
    go_play(&app);
    let opened = open_actions(&app).1;
    see(
        &opened,
        &["New file", "New folder", "Import…", "▾─", CHOOSE],
        &[
            "Empty Trash",
            "Export…",
            "Show hidden files",
            "Sort by name",
            "Page options",
        ],
    );
    shot(
        &app,
        "opt.menus.max_rows",
        "max rows 4 clips the actions menu and shows the more marker",
        args,
    );
    app.finish();
}

#[test]
fn option_title() {
    let (app, args) = option_screen("s04-menus-title");
    click_where(&app, "Title", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("title off", |screen| {
        row_with(screen, "Title").contains(" off")
    });
    toggle_is(&off, "Title", false);
    toggle_is(&off, "Release selects", true);
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    let item = menu_item(&app, "Open");
    let above = row_text(&opened, u32::from(item.y.saturating_sub(1)));
    assert!(
        above.contains('╭') && !above.contains("report.pdf"),
        "title stayed on the menu: {above}"
    );
    see(
        &opened,
        &["Open", "Rename", "Move to Trash", CHOOSE],
        &["Page options", "New file"],
    );
    shot(
        &app,
        "opt.menus.title",
        "title off removes the file name from the open menu",
        args,
    );
    app.finish();
}

#[test]
fn option_release_selects() {
    let (app, args) = option_screen("s04-menus-release");
    click_where(&app, "Release selects", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("release off", |screen| {
        row_with(screen, "Release selects").contains(" off")
    });
    toggle_is(&off, "Release selects", false);
    toggle_is(&off, "Title", true);
    go_play(&app);
    let file = focus_file(&app, "report.pdf");
    right(&app, file, "down");
    let opened = app.wait_screen("release menu", menu_is_open);
    assert!(
        text(&opened).contains("ointer ("),
        "pointer request:\n{}",
        text(&opened)
    );
    let item = menu_item(&app, "Open");
    right(&app, item, "drag");
    right(&app, item, "up");
    app.flash_expired();
    let stayed = app.settle();
    assert!(
        menu_is_open(&stayed),
        "release closed the menu\n{}",
        text(&stayed)
    );
    let action = fact_row(&stayed, "Action");
    assert!(
        action.contains("none yet") && !action.contains("Chosen(0)"),
        "release chose: {action}"
    );
    let closed = close_menu(&app);
    let action = fact_row(&closed, "Action");
    assert!(
        action.contains("Dismissed") && !action.contains("Chosen"),
        "release ran the item: {action}"
    );
    quiesce(&app);
    let shot_screen = app.settle();
    assert!(fact_row(&shot_screen, "Action").contains("Dismissed"));
    assert!(!fact_row(&shot_screen, "Action").contains("Chosen"));
    assert!(!menu_is_open(&shot_screen));
    shot(
        &app,
        "opt.menus.release_selects",
        "release on an item does not choose when release selects is off",
        args,
    );
    app.finish();
}

#[test]
fn option_hide_owner_focus() {
    let (app, args) = option_screen("s04-menus-hide-focus");
    click_where(&app, "Hide page focus", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("hide focus off", |screen| {
        row_with(screen, "Hide page focus").contains(" off")
    });
    toggle_is(&off, "Hide page focus", false);
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    assert!(
        row_with(&opened, "report.pdf").contains('▎'),
        "owner focus hid while the menu was open\n{}",
        row_with(&opened, "report.pdf")
    );
    assert!(menu_is_open(&opened));
    see(
        &opened,
        &["Open", "Move to Trash", CHOOSE, "▎"],
        &["Page options"],
    );
    shot(
        &app,
        "opt.menus.hide_owner_focus",
        "hide page focus off keeps the file gutter while the menu is open",
        args,
    );
    app.finish();
}

#[test]
fn option_nav() {
    let (app, args) = option_screen("s04-menus-nav");
    pick(&app, "Nav keys", 1);
    let chosen = app.settle();
    value_is(&chosen, "Nav keys", "vim");
    value_is(&chosen, "Actions placement", "below");
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    let item = menu_item(&app, "Open");
    let before = cell_bg(&opened, item);
    app.key("j");
    let moved = app.settle();
    assert!(
        menu_is_open(&moved),
        "vim j closed the menu\n{}",
        text(&moved)
    );
    assert_ne!(
        cell_bg(&moved, item),
        before,
        "vim j left the cursor on Open"
    );
    see(
        &moved,
        &["Rename", "Move to Trash", CHOOSE],
        &["Page options"],
    );
    shot(
        &app,
        "opt.menus.nav",
        "vim j moves the menu cursor and leaves the menu open",
        args,
    );
    app.finish();
}

#[test]
fn option_wrap() {
    let (app, args) = option_screen("s04-menus-wrap");
    click_where(&app, "Wrap", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("wrap off", |screen| {
        row_with(screen, "Wrap").contains(" off")
    });
    toggle_is(&off, "Wrap", false);
    toggle_is(&off, "Type-ahead", false);
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    let item = menu_item(&app, "Open");
    let before = cell_bg(&opened, item);
    app.key("up");
    let stayed = app.settle();
    assert!(menu_is_open(&stayed), "up closed the menu");
    assert_eq!(
        cell_bg(&stayed, item),
        before,
        "up wrapped off Open with wrap off"
    );
    see(
        &stayed,
        &["Open", "Move to Trash", CHOOSE],
        &["Page options", "Read-only file"],
    );
    shot(
        &app,
        "opt.menus.wrap",
        "wrap off keeps the cursor on Open when up is pressed",
        args,
    );
    app.finish();
}

#[test]
fn option_type_ahead() {
    let (app, args) = option_screen("s04-menus-type-ahead");
    click_where(&app, "Type-ahead", |hit| hit.x >= 24 && hit.y >= 6);
    let on = app.wait_screen("type ahead on", |screen| {
        row_with(screen, "Type-ahead").contains(" on")
    });
    toggle_is(&on, "Type-ahead", true);
    toggle_is(&on, "Wrap", true);
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    let item = menu_item(&app, "Open");
    let before = cell_bg(&opened, item);
    app.key("d");
    let jumped = app.settle();
    assert!(
        menu_is_open(&jumped),
        "type-ahead closed the menu\n{}",
        text(&jumped)
    );
    assert_ne!(
        cell_bg(&jumped, item),
        before,
        "type-ahead left the cursor on Open"
    );
    let action = fact_row(&jumped, "Action");
    assert!(
        action.contains("none yet") && !action.contains("Chosen(0)"),
        "type-ahead chose: {action}"
    );
    see(
        &jumped,
        &["Open", "Duplicate", "Move to Trash", CHOOSE],
        &["Page options", "Read-only file"],
    );
    shot(
        &app,
        "opt.menus.type_ahead",
        "type-ahead d moves the cursor to Duplicate and leaves the menu open",
        args,
    );
    app.finish();
}

#[test]
fn option_dismiss() {
    let (app, args) = option_screen("s04-menus-dismiss");
    pick(&app, "Dismiss", 1);
    let chosen = app.settle();
    value_is(&chosen, "Dismiss", "escape");
    value_is(&chosen, "Other keys", "close-and-pass");
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    assert!(menu_is_open(&opened));
    app.click(Point { x: 100, y: 30 });
    app.flash_expired();
    let stayed = app.settle();
    assert!(
        menu_is_open(&stayed),
        "outside click closed an escape-only menu\n{}",
        text(&stayed)
    );
    assert!(
        fact_row(&stayed, "Action").contains("none yet"),
        "outside click chose: {}",
        fact_row(&stayed, "Action")
    );
    see(
        &stayed,
        &["Open", "Move to Trash", CHOOSE, "none yet"],
        &["Page options", "Chosen(0)"],
    );
    shot(
        &app,
        "opt.menus.dismiss",
        "escape dismiss ignores an outside click and does not choose",
        args,
    );
    let closed = close_menu(&app);
    let action = fact_row(&closed, "Action");
    assert!(action.contains("Dismissed") && !action.contains("Chosen"));
    app.finish();
}

#[test]
fn option_unknown_key() {
    let (app, args) = option_screen("s04-menus-unknown");
    pick(&app, "Other keys", 1);
    let chosen = app.settle();
    value_is(&chosen, "Other keys", "swallow");
    value_is(&chosen, "Dismiss", "light");
    go_play(&app);
    let opened = open_file_menu(&app, "report.pdf");
    assert!(menu_is_open(&opened));
    app.key("x");
    let stayed = app.settle();
    assert!(
        menu_is_open(&stayed),
        "swallowed key closed the menu\n{}",
        text(&stayed)
    );
    assert!(
        fact_row(&stayed, "Action").contains("none yet"),
        "unknown key dismissed: {}",
        fact_row(&stayed, "Action")
    );
    see(
        &stayed,
        &["Open", CHOOSE, "none yet", "Move to Trash"],
        &["Page options", "Chosen(0)"],
    );
    shot(
        &app,
        "opt.menus.unknown_key",
        "swallow keeps the menu open when an unknown key is pressed",
        args,
    );
    app.finish();
}

#[test]
fn option_moves_cursor() {
    let (app, args) = option_screen("s04-menus-moves");
    click_where(&app, "Click moves cursor", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("moves cursor off", |screen| {
        row_with(screen, "Click moves cursor").contains(" off")
    });
    toggle_is(&off, "Click moves cursor", false);
    toggle_is(&off, "Shift+F10", true);
    go_play(&app);
    focus_file(&app, "report.pdf");
    let license = file_row(&app, "LICENSE");
    right(&app, license, "click");
    let opened = app.wait_screen("license menu", menu_is_open);
    assert!(
        text(&opened).contains("ointer (") && text(&opened).contains("LICENSE"),
        "request:\n{}",
        text(&opened)
    );
    let closed = close_menu(&app);
    assert!(
        fact_row(&closed, "Request").contains("LICENSE · at the pointer"),
        "request: {}",
        fact_row(&closed, "Request")
    );
    let report = row_with(&closed, "report.pdf");
    let license_row = row_with(&closed, "LICENSE");
    assert!(report.contains('▎'), "cursor left report.pdf: {report}");
    assert!(
        !license_row.contains('▎'),
        "right-click moved onto LICENSE: {license_row}"
    );
    let action = fact_row(&closed, "Action");
    assert!(action.contains("Dismissed") && !action.contains("Chosen"));
    assert!(fact_row(&closed, "Keys").contains("a click leaves the cursor"));
    quiesce(&app);
    let shot_screen = app.settle();
    assert!(fact_row(&shot_screen, "Keys").contains("a click leaves the cursor"));
    assert!(row_with(&shot_screen, "report.pdf").contains('▎'));
    assert!(!row_with(&shot_screen, "LICENSE").contains('▎'));
    shot(
        &app,
        "opt.menus.moves_cursor",
        "a right-click opens the menu under the pointer and leaves the cursor",
        args,
    );
    app.finish();
}

#[test]
fn option_shift_f10() {
    let (app, args) = option_screen("s04-menus-shift");
    click_where(&app, "Shift+F10", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("shift f10 off", |screen| {
        let row = row_with(screen, "Shift+F10");
        row.contains(" off") && row.contains("Shift+F10")
    });
    toggle_is(&off, "Shift+F10", false);
    toggle_is(&off, "Menu key", true);
    go_play(&app);
    focus_file(&app, "report.pdf");
    app.key("shift+f10");
    let stayed = app.settle();
    assert!(
        !menu_is_open(&stayed),
        "Shift+F10 opened a menu while off\n{}",
        text(&stayed)
    );
    let keys = fact_row(&stayed, "Keys");
    assert!(
        !keys.contains("Shift+F10") && keys.contains("Menu key") && keys.contains("right-click"),
        "keys readout: {keys}"
    );
    quiesce(&app);
    let shot_screen = app.settle();
    let keys = fact_row(&shot_screen, "Keys");
    assert!(!keys.contains("Shift+F10"), "keys readout: {keys}");
    assert!(!menu_is_open(&shot_screen));
    shot(
        &app,
        "opt.menus.shift_f10",
        "Shift+F10 off drops the key from the readout and does not open a menu",
        args,
    );
    app.finish();
}

#[test]
fn option_menu_key() {
    let (app, args) = option_screen("s04-menus-menu-key");
    click_where(&app, "Menu key", |hit| hit.x >= 24 && hit.y >= 6);
    let off = app.wait_screen("menu key off", |screen| {
        row_with(screen, "Menu key").contains(" off")
    });
    toggle_is(&off, "Menu key", false);
    toggle_is(&off, "Shift+F10", true);
    go_play(&app);
    let screen = app.wait_text("report.pdf");
    let keys = fact_row(&screen, "Keys");
    assert!(
        !keys.contains("Menu key") && keys.contains("Shift+F10") && keys.contains("right-click"),
        "keys readout: {keys}"
    );
    quiesce(&app);
    let shot_screen = app.settle();
    let keys = fact_row(&shot_screen, "Keys");
    assert!(!keys.contains("Menu key"), "keys readout: {keys}");
    assert!(keys.contains("Shift+F10"));
    shot(
        &app,
        "opt.menus.menu_key",
        "menu key off drops Menu key from the readout",
        args,
    );
    app.finish();
}

#[test]
fn option_extra_key() {
    let (app, args) = option_screen("s04-menus-extra");
    pick(&app, "Extra key", 1);
    let chosen = app.settle();
    value_is(&chosen, "Extra key", "m");
    value_is(&chosen, "Nav keys", "desktop");
    toggle_is(&chosen, "Shift+F10", true);
    go_play(&app);
    let armed = app.wait_text("m (extra key)");
    assert!(fact_row(&armed, "Keys").contains("m (extra key)"));
    focus_file(&app, "report.pdf");
    app.key("m");
    let opened = app.wait_screen("extra key menu", menu_is_open);
    see(
        &opened,
        &[
            "Open",
            "Move to Trash",
            "report.pdf",
            CHOOSE,
            "m (extra key)",
        ],
        &["Page options", "New file"],
    );
    shot(
        &app,
        "opt.menus.extra_key",
        "extra key m is shown in the readout and opens the file menu",
        args,
    );
    let closed = close_menu(&app);
    assert!(fact_row(&closed, "Action").contains("Dismissed"));
    assert!(!fact_row(&closed, "Action").contains("Chosen"));
    app.finish();
}
