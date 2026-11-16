//! Scrolling route, sections, specimens, compare levels, scrolling, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s07_scrolling/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Layout › Scrolling";
const TITLE: &str = "Scrolling";
const BLURB: &str =
    "Wheel under the pointer, keys on the focused container, thumb shows where you are";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "scrolling"];
const RANGE_TOP: &str = "1\u{2013}7 of 120";
const RANGE_PROSE_NARROW: &str = "1\u{2013}7 of 201";
const RANGE_PROSE: &str = "10\u{2013}16 of 120";
const RANGE_PAGE: &str = "8\u{2013}14 of 120";
const RANGE_JUMP: &str = "76\u{2013}82 of 120";
const RANGE_DRAG: &str = "58\u{2013}64 of 120";
const RANGE_WRAP: &str = "114\u{2013}120 of 120";
const RANGE_SKIP: &str = "5\u{2013}11 of 120";
const LOG_TAIL: &str = "395\u{2013}400/400";
const LOG_AWAY: &str = "392\u{2013}397 of 400";
const CURSOR: &str = "Desktop \u{00b7} 1s \u{00b7} shortcuts first";
const BAR: char = '\u{2501}';

const OMITTED: &[&str] = &[
    "focused",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "error",
    "editing",
    "loading",
];

const DRAWN: &[&str] = &["rest", "hover", "pressed", "empty", "overflow", "narrow"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s07_scrolling/{}", case_id.replace('.', "_"))
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

fn page(name: &str, opt: &str, cols: u16, rows: u16) -> App {
    let mut args = PAGE_ARGS.to_vec();
    args.extend(["--opt", opt]);
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

fn assert_choice(screen: &TerminalSnapshot, label: &str, value: &str, all: &[&str]) {
    let rest: Vec<&str> = all.iter().copied().filter(|item| *item != value).collect();
    assert_select(screen, label, value, &rest);
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

#[allow(clippy::too_many_arguments)]
fn assert_options(
    screen: &TerminalSnapshot,
    context: &str,
    follow: &str,
    track: &str,
    hbar: &str,
    columns: &str,
    profile: &str,
    ahead: &str,
    timeout: &str,
    letters: &str,
    wrap: &str,
    skip: &str,
) {
    assert_choice(screen, "Context rows", context, &["0", "1", "2"]);
    assert_choice(screen, "Mirror follows", follow, &["on-move", "always"]);
    assert_choice(screen, "Track click", track, &["app", "page", "jump"]);
    assert_choice(screen, "Sideways bar", hbar, &["auto", "always", "never"]);
    assert_toggle(screen, "Sideways by column", columns);
    assert_choice(screen, "Profile", profile, &["desktop", "vim"]);
    assert_choice(screen, "Type-ahead", ahead, &["profile", "on", "off"]);
    assert_choice(screen, "Type-ahead pause", timeout, &["0.5s", "1s", "2s"]);
    assert_choice(
        screen,
        "Letters go to",
        letters,
        &["shortcuts", "type-ahead"],
    );
    assert_toggle(screen, "Wrap at the ends", wrap);
    assert_toggle(screen, "Skip disabled", skip);
}

fn assert_option_defaults(screen: &TerminalSnapshot) {
    assert_options(
        screen,
        "1",
        "on-move",
        "app",
        "auto",
        "on",
        "desktop",
        "profile",
        "1s",
        "shortcuts",
        "off",
        "on",
    );
}

/// Grid labels sit under the legend. The blurb and the focus legend also say "focused".
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn only(hits: Vec<Point>, label: &str) -> Point {
    assert_eq!(hits.len(), 1, "expected one {label}, got {hits:?}");
    hits[0]
}

fn click_where(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    let point = only(hits, label);
    app.click(point);
    point
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
        app.wheel_applied(point, false, 3);
    }
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
}

fn reveal_text(app: &App, needle: &str) {
    let point = Point { x: 70, y: 20 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 70, y: 12 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel_applied(point, false, 3);
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

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn bars_on(screen: &TerminalSnapshot, needle: &str) -> usize {
    row_with(screen, needle)
        .chars()
        .filter(|ch| *ch == BAR)
        .count()
}

fn list_row(screen: &TerminalSnapshot) -> Point {
    only(
        find_all(screen, "Row 001")
            .into_iter()
            .filter(|hit| hit.x > 30)
            .collect(),
        "Row 001",
    )
}

/// The long-list thumb is the second vertical bar on the Row 001 line: the
/// prose pane owns the first.
fn list_thumb(screen: &TerminalSnapshot) -> Point {
    let row = list_row(screen);
    let line = row_text(screen, u32::from(row.y));
    let mut seen = 0;
    for (index, ch) in line.chars().enumerate() {
        if ch == '\u{2503}' {
            seen += 1;
            if seen == 2 {
                return Point {
                    x: u16::try_from(index).expect("thumb column"),
                    y: row.y,
                };
            }
        }
    }
    panic!("list thumb missing on {line:?}");
}

fn prose_hit(screen: &TerminalSnapshot) -> Point {
    only(
        find_all(screen, "Junie")
            .into_iter()
            .filter(|hit| hit.x > 26)
            .collect(),
        "Junie",
    )
}

fn log_hit(screen: &TerminalSnapshot) -> Point {
    only(
        find_all(screen, "145.78s")
            .into_iter()
            .filter(|hit| hit.x > 90)
            .collect(),
        "145.78s",
    )
}

fn tree_api(screen: &TerminalSnapshot) -> Point {
    only(
        find_all(screen, "api")
            .into_iter()
            .filter(|hit| hit.y > 28 && (50..70).contains(&hit.x))
            .collect(),
        "tree api",
    )
}

fn over_item(screen: &TerminalSnapshot) -> Point {
    find_all(screen, "item 1")
        .into_iter()
        .max_by_key(|hit| hit.x)
        .unwrap_or_else(|| panic!("item 1 missing\n{}", text(screen)))
}

fn hand_host(screen: &TerminalSnapshot) -> Point {
    only(
        find_all(screen, "api-01")
            .into_iter()
            .filter(|hit| hit.y > 15)
            .collect(),
        "api-01",
    )
}

fn downs(app: &App, count: usize) {
    for _ in 0..count {
        app.key("down");
    }
}

fn assert_rest_panes(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            "Junie works",
            RANGE_TOP,
            "Row 001",
            LOG_TAIL,
            "api-01",
            "item 1",
            "row 1 \u{00b7} vertical",
            "no key yet",
            "wrap off",
            CURSOR,
        ],
        &[RANGE_PROSE, RANGE_PAGE, RANGE_JUMP, RANGE_DRAG, "merged."],
    );
    assert_eq!(bars_on(screen, "item 6"), 9, "default hand bar");
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("Row 001");
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    flash_settle(app);
    let screen = app.settle();
    check(&screen);
    shot(app, case_id);
    app.finish();
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s07-route-page", 120, 40, PAGE_ARGS);
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
            "truecolor",
            "Playground",
            "Row 001",
            RANGE_TOP,
            LOG_TAIL,
            "api-01",
            "Junie works",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "hidden rows",
        ],
    );
    let side = only(
        find_all(&screen, "Scrolling")
            .into_iter()
            .filter(|hit| hit.x < 24)
            .collect(),
        "sidebar Scrolling",
    );
    let side_row = row_text(&screen, u32::from(side.y));
    assert!(
        side_row.contains('▎'),
        "sidebar focus moved off Scrolling: {side_row:?}"
    );
    assert_rest_panes(&screen);
    shot(&app, "sc.scrolling.route");
    app.finish();

    let app = open("s07-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Row 001");
    see(
        &entered,
        &[CRUMB, BLURB, "Row 001", LOG_TAIL],
        &[OVERVIEW, "Page options", "Layout › Panels"],
    );
    app.key("[");
    let left = app.wait_text("Layout › Panels");
    see(&left, &["Layout › Panels"], &[CRUMB, BLURB, "Row 001"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Row 001", "Junie works"],
        &["Layout › Panels", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, "Row 001"]);
    click_side(&app, "Layout");
    app.wait_screen("layout group open", |screen| {
        find_all(screen, "Scrolling").iter().any(|hit| hit.x < 24)
    });
    click_side(&app, "Scrolling");
    let pointed = app.wait_text("Row 001");
    see(
        &pointed,
        &[CRUMB, BLURB, "Row 001", LOG_TAIL, "api-01"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s07-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Row 001");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Wheel under the pointer",
            "Playground",
            "Row 001",
            RANGE_PROSE_NARROW,
            RANGE_TOP,
            LOG_TAIL,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "thumb shows",
            "Wrapped text",
            "Hand-drawn",
            "80×24",
        ],
    );
    let side = only(
        find_all(&narrow_play, "Scrolling")
            .into_iter()
            .filter(|hit| hit.y > 4 && hit.x < 19)
            .collect(),
        "sidebar Scrolling",
    );
    let side_row = row_text(&narrow_play, u32::from(side.y));
    assert!(
        side_row.contains('▎'),
        "sidebar focus moved off Scrolling: {side_row:?}"
    );
    shot(&app, "sc.scrolling.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.wait_text("Context rows");
    assert_size(&app, 80, 24);
    assert_choice(&narrow_opt, "Context rows", "1", &["0", "1", "2"]);
    assert_choice(
        &narrow_opt,
        "Mirror follows",
        "on-move",
        &["on-move", "always"],
    );
    assert_choice(&narrow_opt, "Track click", "app", &["app", "page", "jump"]);
    assert_choice(
        &narrow_opt,
        "Sideways bar",
        "auto",
        &["auto", "always", "never"],
    );
    assert_toggle(&narrow_opt, "Sideways by column", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Context rows", "Type-ahead"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Row 001",
            WIDE,
            "truecolor",
            "80×24",
        ],
    );
    shot(&app, "sc.scrolling.options.80x24");

    click_tab(&app, "States");
    flash_settle(&app);
    let narrow_states = app.wait_screen("states at 80 by 24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains("Compare levels")
            && grid_label(screen, "rest")
            && grid_label(screen, "pressed")
    });
    assert_size(&app, 80, 24);
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "Lines",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "Row 001",
            WIDE,
            "truecolor",
            "80×24",
        ],
    );
    for label in [
        "focused",
        "focus + hover",
        "selected",
        "disabled",
        "busy",
        "loading",
    ] {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&narrow_states)
        );
    }
    shot(&app, "sc.scrolling.states.80x24");

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
            "Lines",
            "Wrapped",
            "Log tail",
            "line 1 of the panel",
            "A wrapped paragraph",
            "0.00s",
            "line 1 of t\u{2026}",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Row 001",
            "256 colors",
        ],
    );
    for label in OMITTED {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for label in DRAWN {
        assert!(
            grid_label(&wide_states, label),
            "{label} left the states grid\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in [
        ("sc.scrolling.specimen.rest", "rest"),
        ("sc.scrolling.specimen.hover", "hover"),
        ("sc.scrolling.specimen.pressed", "pressed"),
        ("sc.scrolling.specimen.empty", "empty"),
        ("sc.scrolling.specimen.overflow", "overflow"),
        ("sc.scrolling.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.scrolling.states.120x40");
    shot(&app, "sc.scrolling.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_option_defaults(&wide_opt);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Skip disabled"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Row 001"],
    );
    shot(&app, "sc.scrolling.options.120x40");
    shot(&app, "sc.scrolling.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Row 001");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Wrapped text",
            "Long list",
            "Hand-drawn",
            "Row 001",
            RANGE_TOP,
            LOG_TAIL,
            "api-01",
            CURSOR,
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "hidden rows",
            RANGE_PROSE_NARROW,
        ],
    );
    let tabs = row_with(&wide_play, CAP_PLAY);
    assert!(
        tabs.contains('▎') && tabs.contains("Playground"),
        "playground tab is not focused: {tabs:?}"
    );
    assert_rest_panes(&wide_play);
    shot(&app, "sc.scrolling.playground.120x40");
    shot(&app, "sc.scrolling.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s07-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES, LEGEND],
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
    let point = Point { x: 70, y: 20 };
    for _ in 0..80 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel_applied(point, true, 3);
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
    shot(&app, "sc.scrolling.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains("Compare levels")
    });
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels", "80×24"],
    );
    shot(&app, "sc.scrolling.compare-levels.80x24");

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
    assert_option_defaults(&options);
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "desktop"],
        &["Compare levels", OVERVIEW, "vim"],
    );
    app.finish();
}

#[test]
fn scroll_wheel_keys_and_thumb() {
    let app = open("s07-scroll", 120, 40, PAGE_ARGS);
    let before = app.wait_text("Row 001");
    assert_size(&app, 120, 40);
    assert_rest_panes(&before);
    shot(&app, "sc.scrolling.scroll.before");

    let thumb = list_thumb(&app.settle());
    app.click(Point {
        x: thumb.x,
        y: thumb.y + 4,
    });
    let paged = app.settle();
    see(
        &paged,
        &[
            RANGE_PAGE,
            "Row 008",
            "Junie works",
            RANGE_TOP,
            LOG_TAIL,
            "api-01",
            "row 1 \u{00b7} vertical",
            "no key yet",
            "item 1",
        ],
        &[RANGE_JUMP, "Row 076", "Row 001", "merged.", LOG_AWAY],
    );

    let listed = only(
        find_all(&paged, "Row 008")
            .into_iter()
            .filter(|hit| hit.x > 30)
            .collect(),
        "Row 008",
    );
    app.wheel(listed, false, 3);
    let restored = app.wait_text("Row 001");
    see(
        &restored,
        &["Row 001", RANGE_TOP, "Junie works", LOG_TAIL, "api-01"],
        &[RANGE_PAGE, "Row 008", "merged."],
    );

    let prose = prose_hit(&app.settle());
    app.wheel(prose, true, 3);
    let wheeled = app.settle();
    see(
        &wheeled,
        &[
            RANGE_PROSE,
            "merged.",
            "Row 001",
            RANGE_TOP,
            LOG_TAIL,
            "api-01",
            "item 1",
            "no key yet",
        ],
        &[RANGE_PAGE, "Row 010", LOG_AWAY, "gateway"],
    );

    let log = log_hit(&app.settle());
    app.click(log);
    app.key("up");
    app.key("up");
    app.key("up");
    let away = app.settle();
    see(
        &away,
        &[
            LOG_AWAY,
            "144.67s",
            RANGE_PROSE,
            "merged.",
            "Row 001",
            RANGE_TOP,
            "api-01",
            "no key yet",
        ],
        &[LOG_TAIL, RANGE_DRAG, "gateway"],
    );
    app.key("f");
    let followed = app.settle();
    see(
        &followed,
        &[
            LOG_TAIL,
            "145.78s",
            RANGE_PROSE,
            "merged.",
            "Row 001",
            "api-01",
        ],
        &[LOG_AWAY, "144.67s", RANGE_DRAG],
    );

    let tree = tree_api(&app.settle());
    app.click(tree);
    app.key("g");
    let jumped = app.settle();
    let gateway = row_with(&jumped, "gateway");
    assert!(
        gateway.contains('▎'),
        "type-ahead left the tree gutter off gateway: {gateway:?}"
    );
    see(
        &jumped,
        &[
            "gateway",
            RANGE_PROSE,
            "merged.",
            "Row 001",
            RANGE_TOP,
            LOG_TAIL,
            "api-01",
            "no key yet",
        ],
        &[LOG_AWAY, RANGE_DRAG, "Row 010"],
    );
    flash_settle(&app);
    let settled = app.settle();
    see(
        &settled,
        &[
            "gateway",
            RANGE_PROSE,
            "merged.",
            "Row 001",
            LOG_TAIL,
            "api-01",
        ],
        &[LOG_AWAY, RANGE_DRAG],
    );

    let thumb = list_thumb(&app.settle());
    app.down(thumb);
    app.drag_to(Point {
        x: thumb.x,
        y: thumb.y + 3,
    });
    let dragged = app.settle();
    assert_size(&app, 120, 40);
    see(
        &dragged,
        &[
            RANGE_DRAG,
            "Row 058",
            "Row 060",
            RANGE_PROSE,
            "merged.",
            LOG_TAIL,
            "145.78s",
            "gateway",
            "api-01",
            "item 1",
            "row 1 \u{00b7} vertical",
        ],
        &[RANGE_TOP, "Row 001", LOG_AWAY, "Row 076"],
    );
    shot(&app, "sc.scrolling.scroll.after");
    app.up(Point {
        x: thumb.x,
        y: thumb.y + 3,
    });
    let released = app.settle();
    see(
        &released,
        &[
            RANGE_DRAG,
            "Row 058",
            RANGE_PROSE,
            "merged.",
            LOG_TAIL,
            "gateway",
        ],
        &[RANGE_TOP, "Row 001"],
    );
    app.finish();
}

#[test]
fn option_context_0() {
    let app = page("s07-context", "scrolling.context=0", 120, 40);
    app.wait_text("Row 001");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "0",
            "on-move",
            "app",
            "auto",
            "on",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    let row = list_row(&app.settle());
    app.click(row);
    downs(&app, 6);
    finish_option(&app, "opt.scrolling.context", |screen| {
        see(
            screen,
            &[
                RANGE_TOP,
                "Row 001",
                "Row 007",
                "row 7 \u{00b7} vertical",
                "Last: Nav(Down) \u{2192} To(row 7)",
                "billing-01",
                "Junie works",
                LOG_TAIL,
                "wrap off \u{00b7} skip disabled on",
            ],
            &["2\u{2013}8 of 120", "api-01", "To(row 8)", RANGE_PROSE],
        );
    });
}

#[test]
fn option_follow_always() {
    let app = page("s07-follow", "scrolling.follow=always", 120, 40);
    app.wait_text("api-01");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "always",
            "app",
            "auto",
            "on",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    let hand = hand_host(&app.settle());
    app.wheel(hand, true, 5);
    let stayed = app.settle();
    see(
        &stayed,
        &["api-01", "item 1", "Row 001", RANGE_TOP, "Junie works"],
        &["item \u{2026}"],
    );
    let over = over_item(&app.settle());
    app.wheel(over, true, 4);
    finish_option(&app, "opt.scrolling.follow", |screen| {
        see(
            screen,
            &[
                "api-01",
                "item 1",
                "item \u{2026}",
                "Row 001",
                RANGE_TOP,
                "Junie works",
                LOG_TAIL,
            ],
            &[RANGE_PROSE, RANGE_DRAG],
        );
    });
}

#[test]
fn option_track_click_jump() {
    let app = page("s07-jump", "scrolling.track_click=jump", 120, 40);
    app.wait_text("Row 001");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "jump",
            "auto",
            "on",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    let thumb = list_thumb(&app.settle());
    app.click(Point {
        x: thumb.x,
        y: thumb.y + 4,
    });
    finish_option(&app, "opt.scrolling.track_click", |screen| {
        see(
            screen,
            &[
                RANGE_JUMP,
                "Row 076",
                "Row 080",
                "Junie works",
                RANGE_TOP,
                LOG_TAIL,
                "api-01",
                "row 1 \u{00b7} vertical",
                "no key yet",
                "item 1",
            ],
            &[RANGE_PAGE, "Row 008", "Row 001", RANGE_PROSE],
        );
    });
}

#[test]
fn option_hbar_never() {
    let app = page("s07-hbar", "scrolling.hbar=never", 120, 40);
    app.wait_text("edge-01");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "never",
            "on",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    finish_option(&app, "opt.scrolling.hbar", |screen| {
        assert_eq!(bars_on(screen, "item 6"), 0, "horizontal thumb stayed");
        see(
            screen,
            &[
                "edge-01", "api-01", "item 6", CURSOR, "Row 001", RANGE_TOP, LOG_TAIL,
            ],
            &[RANGE_PROSE],
        );
    });
}

#[test]
fn option_columns_off() {
    let app = page("s07-columns", "scrolling.columns=off", 120, 40);
    app.wait_text("Row 001");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "off",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    finish_option(&app, "opt.scrolling.columns", |screen| {
        assert_eq!(
            bars_on(screen, "item 6"),
            10,
            "column thumb kept the on length"
        );
        see(
            screen,
            &["api-01", CURSOR, "Row 001", RANGE_TOP, "item 6", LOG_TAIL],
            &["edge-01"],
        );
    });
}

#[test]
fn option_profile_vim() {
    let app = page("s07-vim", "scrolling.profile=vim", 120, 40);
    app.wait_text("Vim \u{00b7} no type-ahead");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "on",
            "vim",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    let row = list_row(&app.settle());
    app.click(row);
    app.key("j");
    finish_option(&app, "opt.scrolling.profile", |screen| {
        see(
            screen,
            &[
                "Vim \u{00b7} no type-ahead",
                "row 2 \u{00b7} vertical",
                "Last: Nav(Down) \u{2192} To(row 2)",
                "Row 002",
                "Row 001",
                RANGE_TOP,
                "Junie works",
                LOG_TAIL,
                "api-01",
            ],
            &[
                "Type('j')",
                CURSOR,
                "Desktop \u{00b7}",
                "To(row 3)",
                RANGE_PROSE,
            ],
        );
    });
}

#[test]
fn option_type_ahead_off() {
    let app = page("s07-ahead", "scrolling.type_ahead=off", 120, 40);
    app.wait_text("no type-ahead");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "on",
            "desktop",
            "off",
            "1s",
            "shortcuts",
            "off",
            "on",
        );
    });
    let tree = tree_api(&app.settle());
    app.click(tree);
    app.key("g");
    finish_option(&app, "opt.scrolling.type_ahead", |screen| {
        let row = row_text(screen, u32::from(tree_api(screen).y));
        assert!(
            row.contains('▎') && row.contains("api"),
            "tree left api: {row:?}"
        );
        see(
            screen,
            &[
                "Desktop \u{00b7} no type-ahead",
                "api-01",
                "Row 001",
                RANGE_TOP,
                "Junie works",
                LOG_TAIL,
                "no key yet",
            ],
            &["gateway", "Vim \u{00b7}", CURSOR, RANGE_PROSE],
        );
    });
}

#[test]
fn option_timeout_2s() {
    let app = page("s07-timeout", "scrolling.timeout=2s", 120, 40);
    app.wait_text("2s");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "on",
            "desktop",
            "profile",
            "2s",
            "shortcuts",
            "off",
            "on",
        );
    });
    finish_option(&app, "opt.scrolling.timeout", |screen| {
        let cursor = row_with(screen, "Desktop \u{00b7} 2s \u{00b7} shortcuts first");
        assert!(
            !cursor.contains("1s"),
            "timeout row still says 1s: {cursor:?}"
        );
        see(
            screen,
            &[
                "Desktop \u{00b7} 2s \u{00b7} shortcuts first",
                "Row 001",
                RANGE_TOP,
                "api-01",
                LOG_TAIL,
            ],
            &[CURSOR, "typing first", "edge-01"],
        );
    });
}

#[test]
fn option_precedence_type_ahead() {
    let app = page("s07-prec", "scrolling.precedence=type-ahead", 120, 40);
    app.wait_text("typing first");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "on",
            "desktop",
            "profile",
            "1s",
            "type-ahead",
            "off",
            "on",
        );
    });
    finish_option(&app, "opt.scrolling.precedence", |screen| {
        see(
            screen,
            &[
                "Desktop \u{00b7} 1s \u{00b7} typing first",
                "Row 001",
                RANGE_TOP,
                "api-01",
                LOG_TAIL,
                "wrap off",
            ],
            &["shortcuts first", "no type-ahead", "edge-01"],
        );
    });
}

#[test]
fn option_wrap_on() {
    let app = page("s07-wrap", "scrolling.wrap=on", 120, 40);
    app.wait_text("wrap on");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "on",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "on",
            "on",
        );
    });
    let row = list_row(&app.settle());
    app.click(row);
    app.key("end");
    let end = app.settle();
    see(
        &end,
        &[
            RANGE_WRAP,
            "Row 119",
            "row 119 \u{00b7} vertical",
            "Last: Nav(End) \u{2192} To(row 119)",
            "edge-12",
            "wrap on \u{00b7} skip disabled on",
            "Junie works",
            RANGE_TOP,
            LOG_TAIL,
        ],
        &["To(row 1)", "api-01", RANGE_PROSE],
    );
    app.key("down");
    finish_option(&app, "opt.scrolling.wrap", |screen| {
        see(
            screen,
            &[
                "Row 001",
                RANGE_TOP,
                "row 1 \u{00b7} vertical",
                "Last: Nav(Down) \u{2192} To(row 1)",
                "wrap on \u{00b7} skip disabled on",
                "api-01",
                "Junie works",
                LOG_TAIL,
            ],
            &["Row 119", RANGE_WRAP, "edge-12", "To(row 119)", RANGE_PROSE],
        );
    });
}

#[test]
fn option_skip_disabled_off() {
    let app = page("s07-skip", "scrolling.skip_disabled=off", 120, 40);
    app.wait_text("skip disabled off");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "1",
            "on-move",
            "app",
            "auto",
            "on",
            "desktop",
            "profile",
            "1s",
            "shortcuts",
            "off",
            "off",
        );
    });
    let row = list_row(&app.settle());
    app.click(row);
    downs(&app, 9);
    finish_option(&app, "opt.scrolling.skip_disabled", |screen| {
        see(
            screen,
            &[
                RANGE_SKIP,
                "Row 010",
                "retired",
                "row 10 \u{00b7} vertical",
                "Last: Nav(Down) \u{2192} To(row 10)",
                "skip disabled off",
                "feeds-01",
                "Junie works",
                RANGE_TOP,
                LOG_TAIL,
            ],
            &["To(row 11)", "api-01", "skip disabled on", RANGE_PROSE],
        );
    });
}

fn nested_first_row(screen: &TerminalSnapshot) -> Option<u32> {
    (0..screen.rows())
        .filter_map(|row| {
            let line = row_text(screen, row);
            let tail = line.split_once("nested row ")?.1;
            tail.split_whitespace().next()?.parse().ok()
        })
        .min()
}

/// Right of the `nested row 1` text the line holds the child's bar first and
/// the outer pane's bar, on the panel edge, last.
fn nested_thumb(screen: &TerminalSnapshot) -> Point {
    let hit = find_all(screen, "nested row 1")
        .into_iter()
        .next()
        .expect("nested row 1 is visible");
    let line = row_text(screen, u32::from(hit.y));
    let column = line
        .chars()
        .enumerate()
        .filter(|(index, ch)| *index > usize::from(hit.x) && *ch == '\u{2503}')
        .map(|(index, _)| index)
        .next()
        .unwrap_or_else(|| panic!("child thumb missing on {line:?}"));
    Point {
        x: u16::try_from(column).expect("thumb column"),
        y: hit.y,
    }
}

#[test]
fn option_nested_scrollbars_on_from_the_options_tab_drags_the_child_thumb() {
    let app = open("s07-nested", 120, 40, PAGE_ARGS);
    let playground = app.wait_text("Row 001");
    see(&playground, &["Divider or bar"], &["Nested bars"]);

    click_tab(&app, "Options");
    let options = app.wait_text("Nested scrollbars");
    assert_toggle(&options, "Nested scrollbars", "off");
    app.click(app.point("Nested scrollbars"));
    app.wait_screen("Nested scrollbars switched on", |screen| {
        row_with(screen, "Nested scrollbars").contains("on")
    });
    click_tab(&app, "Playground");

    let before = app.wait_text("Nested bars");
    see(
        &before,
        &["outer content", "nested row 1"],
        &["Divider or bar"],
    );
    assert_eq!(nested_first_row(&before), Some(1));
    let outer_row = find_all(&before, "outer content")[0].y;
    let thumb = nested_thumb(&before);

    app.down(thumb);
    app.drag_to(Point {
        x: thumb.x,
        y: thumb.y + 2,
    });
    let dragged = app.settle();
    let first = nested_first_row(&dragged).expect("child rows stay visible while dragging");
    assert!(
        first > 1,
        "dragging the child thumb scrolls the child, first row {first}\n{}",
        text(&dragged)
    );
    assert_eq!(
        find_all(&dragged, "outer content")[0].y,
        outer_row,
        "the outer pane stays put while the child thumb is dragged"
    );
    app.drag_to(thumb);
    let back = app.settle();
    assert_eq!(
        nested_first_row(&back),
        Some(1),
        "dragging back to the start restores the child\n{}",
        text(&back)
    );
    app.up(thumb);
    app.finish();
}

fn virtual_first_row(screen: &TerminalSnapshot) -> Option<u32> {
    (0..screen.rows())
        .flat_map(|row| {
            let line = row_text(screen, row);
            let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
            words
                .windows(2)
                .filter(|pair| pair[0] == "Row" && pair[1].len() == 5)
                .filter_map(|pair| pair[1].parse::<u32>().ok())
                .collect::<Vec<_>>()
        })
        .min()
}

#[test]
fn option_virtual_rows_10000_keys_and_wheel_move_a_bounded_window() {
    let app = page("s07-virtual", "scrolling.window=10000", 120, 40);
    let playground = app.wait_text("of 10000");
    see(&playground, &["Virtual", "Row 00001"], &["Row 10000"]);

    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    see(&options, &["Virtual rows", "Nested scrollbars"], &[]);
    click_tab(&app, "Playground");
    let back = app.wait_text("of 10000");

    app.click(find_all(&back, "Row 00002")[0]);
    app.key("end");
    let end = app.wait_text("Row 10000");
    see(&end, &["of 10000"], &["Row 00001"]);
    app.key("home");
    let top = app.wait_text("Row 00001");
    assert_eq!(virtual_first_row(&top), Some(1));

    let at = find_all(&top, "Row 00001")[0];
    app.wheel(at, true, 3);
    let moved = app.settle();
    let first = virtual_first_row(&moved).expect("rows stay visible while wheeling");
    assert!(
        first > 1,
        "the wheel scrolls the virtual rows, first row {first}\n{}",
        text(&moved)
    );
    app.finish();
}
