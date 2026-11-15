//! Panels route, sections, specimens, compare levels, scrolling, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s07_panels/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Layout › Panels";
const NOTICES: &str = "Feedback › Notices";
const TITLE: &str = "Panels";
const BLURB: &str = "Cards group; a frame only where a pane needs an edge; nothing boxed twice";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "panels"];
const PROSE: &str = "Junie works through a task the way a";
const FRAMED: &str = "Framed · split pane";
const PROSE_META: &str = "1–13 of 24";
const LOG_META: &str = "1–11 of 60";
const LOG_LINE: &str = "0.00s";
const RULE: char = '\u{2500}';

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focused_hover",
    "selected",
    "disabled",
    "busy",
    "error",
    "editing",
    "empty",
    "loading",
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s07_panels/{}", case_id.replace('.', "_"))
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

fn row_index(screen: &TerminalSnapshot, needle: &str) -> u32 {
    (0..screen.rows())
        .find(|row| row_text(screen, *row).contains(needle))
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

#[allow(clippy::too_many_arguments)]
fn assert_panel_options(
    screen: &TerminalSnapshot,
    focus: &str,
    heading: &str,
    title_min: &str,
    pad: &str,
    meta: &str,
    wrap: &str,
    tail: &str,
    sideways: &str,
    track: &str,
    keys: &str,
    follow: &str,
) {
    assert_one(screen, "Card focus mark", focus, &["title", "bar"]);
    assert_one(
        screen,
        "Card heading",
        heading,
        &["theme", "plain", "rule", "band", "emphasis"],
    );
    assert_one(screen, "Title keeps", title_min, &["12", "4", "20"]);
    assert_one(screen, "Card padding", pad, &["2", "1", "0"]);
    assert_one(screen, "Short meta", meta, &["auto", "custom"]);
    assert_toggle(screen, "Wrap prose", wrap);
    assert_toggle(screen, "Log follows its tail", tail);
    assert_one(
        screen,
        "Log sideways",
        sideways,
        &["theme", "off", "bar-always", "bar-never", "step-1", "jump"],
    );
    assert_one(screen, "Track click", track, &["app", "page", "jump"]);
    assert_one(screen, "Keys", keys, &["desktop", "vim", "no-paging"]);
    assert_one(screen, "Follow key", follow, &["f", "t"]);
}

fn assert_defaults(screen: &TerminalSnapshot) {
    assert_panel_options(
        screen, "title", "theme", "12", "2", "auto", "on", "off", "theme", "app", "desktop", "f",
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
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 40, y: 12 };
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

fn card_band(line: &str) -> String {
    line.chars().skip(26).take(46).collect()
}

fn assert_rules(screen: &TerminalSnapshot, present: bool) {
    for title in ["Titled card", "Nested"] {
        let next = row_text(screen, row_index(screen, title) + 1);
        let band = card_band(&next);
        assert_eq!(
            band.contains(RULE),
            present,
            "{title} rule band {band:?} present={present}\n{next}"
        );
    }
    let next = row_text(screen, row_index(screen, "Card · scrollable") + 1);
    let band: String = next.chars().skip(74).take(42).collect();
    assert_eq!(
        band.contains(RULE),
        present,
        "log rule band {band:?} present={present}\n{next}"
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
            "Titled card",
            "Untitled card. Same surface",
            "Nested",
            "Target",
            "Local",
            "CLI",
            "Cloud",
            FRAMED,
            "Card · scrollable",
            PROSE,
            PROSE_META,
            LOG_META,
            LOG_LINE,
            "surface",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "Follow tail",
            "1–11/60",
            "50–60 of 60",
        ],
    );
    assert_rules(screen, true);
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

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s07-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    shot(&app, "sc.panels.route");
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
        NOTICES,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Titled card");
    see(
        &entered,
        &[CRUMB, BLURB, "Titled card", FRAMED, "Cloud"],
        &[OVERVIEW, "Page options", NOTICES],
    );
    app.key("[");
    let left = app.wait_text(NOTICES);
    see(&left, &[NOTICES], &[CRUMB, "Titled card", BLURB]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Titled card", "Card · scrollable"],
        &[NOTICES, OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Titled card", BLURB]);
    click_side(&app, "Layout");
    click_side(&app, "Panels");
    let pointed = app.wait_text("Titled card");
    see(
        &pointed,
        &[CRUMB, BLURB, "Titled card", FRAMED, "Local", "Cloud"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s07-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Titled card");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Cards group",
            "Playground",
            "Titled card",
            "Untitled card. Same surface",
            "surface",
            "padding edge",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "nothing boxed twice",
            FRAMED,
            "Nested",
            "Card · scrollable",
            "Local",
            "Target",
            LOG_META,
        ],
    );
    shot(&app, "sc.panels.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_one(&narrow_opt, "Card focus mark", "title", &["title", "bar"]);
    assert_one(
        &narrow_opt,
        "Card heading",
        "theme",
        &["theme", "plain", "rule", "band", "emphasis"],
    );
    assert_one(&narrow_opt, "Title keeps", "12", &["12", "4", "20"]);
    assert_one(&narrow_opt, "Card padding", "2", &["2", "1", "0"]);
    assert_one(&narrow_opt, "Short meta", "auto", &["auto", "custom"]);
    assert_toggle(&narrow_opt, "Wrap prose", "on");
    assert_toggle(&narrow_opt, "Log follows its tail", "off");
    assert_one(
        &narrow_opt,
        "Log sideways",
        "theme",
        &["theme", "off", "bar-always", "bar-never", "step-1", "jump"],
    );
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Card focus mark", "Log sideways"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Titled card",
            WIDE,
            "truecolor",
            "Track click",
            "Follow key",
            "nothing boxed twice",
        ],
    );
    shot(&app, "sc.panels.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "focused", "overflow"] {
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
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "focused",
            "overflow",
            "Card · bar mark",
            "Settings",
            "3 items",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "Titled card",
            WIDE,
            "truecolor",
            LEGEND,
            "Framed",
            "live",
            "nothing boxed twice",
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.panels.states.80x24");

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
            "Card",
            "Card · bar mark",
            "Framed",
            "Settings",
            "3 items",
            "live",
            "1–9 of 24",
            "a meta",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Titled card",
            "256 colors",
            "Card · plain",
        ],
    );
    assert!(
        grid_row(&wide_states, "focused").contains("▏Log"),
        "focused specimen has no bar mark\n{}",
        text(&wide_states)
    );
    assert!(
        !grid_row(&wide_states, "rest").contains('▏'),
        "rest specimen draws the focused bar\n{}",
        text(&wide_states)
    );
    assert!(
        grid_row(&wide_states, "overflow").contains("a meta"),
        "overflow specimen dropped its long meta\n{}",
        text(&wide_states)
    );
    assert!(
        grid_row(&wide_states, "narrow").contains("Previ"),
        "narrow specimen did not shorten the framed title\n{}",
        text(&wide_states)
    );
    assert_omitted(&wide_states);
    for (case_id, label) in [
        ("sc.panels.specimen.rest", "rest"),
        ("sc.panels.specimen.focused", "focused"),
        ("sc.panels.specimen.overflow", "overflow"),
        ("sc.panels.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.panels.states.120x40");
    shot(&app, "sc.panels.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_defaults(&wide_opt);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Follow key"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Titled card"],
    );
    shot(&app, "sc.panels.options.120x40");
    shot(&app, "sc.panels.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Titled card");
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.panels.playground.120x40");
    shot(&app, "sc.panels.playground");
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
    shot(&app, "sc.panels.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.panels.compare-levels.80x24");

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
        &["title", "Page options", CAP_OPT, CRUMB, "Follow key"],
        &["Compare levels", OVERVIEW, "bar"],
    );
    app.finish();
}

fn click_log(app: &App) {
    click_where(app, "Resolving", |hit| hit.x > 70 && hit.y > 20);
}

fn log_row(screen: &TerminalSnapshot) -> String {
    row_with(screen, "Card · scrollable")
}

#[test]
fn panels_scroll_axes_leave_neighbours() {
    let app = open("s07-panels", 120, 40, PAGE_ARGS);
    let before = app.wait_text("Titled card");
    assert_size(&app, 120, 40);
    assert_playground(&before);
    shot(&app, "sc.panels.panels.before");

    click_log(&app);
    app.wait_text("Home End");
    flash_settle(&app);
    let focused = app.settle();
    see(
        &focused,
        &[
            PROSE, PROSE_META, LOG_META, LOG_LINE, "Local", "CLI", "Cloud",
        ],
        &["2–12 of 60", "Page options"],
    );

    app.key("down");
    let down = app.settle();
    see(
        &down,
        &["2–12 of 60", "0.37s", PROSE, PROSE_META, "Local", "Target"],
        &[LOG_META, LOG_LINE, "Page options"],
    );
    app.key("right");
    let across = app.settle();
    see(
        &across,
        &[
            "2–12 of 60",
            "Fetching crates.io index",
            PROSE,
            PROSE_META,
            "Local",
            "CLI",
        ],
        &[
            LOG_META,
            LOG_LINE,
            "Fetching crates.io ind…",
            "Page options",
            "Compare levels",
        ],
    );

    click_where(&app, PROSE, |hit| hit.x > 70 && hit.y > 7);
    flash_settle(&app);
    let held = app.settle();
    see(
        &held,
        &[PROSE, PROSE_META, "2–12 of 60", "Fetching crates.io index"],
        &[LOG_META, "Fetching crates.io ind…", "Page options"],
    );
    app.key("down");
    let prose = app.settle();
    see(
        &prose,
        &[
            "2–14 of 24",
            "2–12 of 60",
            "Fetching crates.io index",
            "Local",
        ],
        &[
            PROSE_META,
            PROSE,
            LOG_META,
            LOG_LINE,
            "Fetching crates.io ind…",
        ],
    );

    click_where(&app, "Local", |hit| hit.x < 70 && hit.y > 20);
    flash_settle(&app);
    let nested = app.settle();
    assert!(
        row_with(&nested, "Local").contains('▎'),
        "nested list did not take Local\n{}",
        text(&nested)
    );
    see(
        &nested,
        &[
            "2–14 of 24",
            "2–12 of 60",
            "Fetching crates.io index",
            "CLI",
        ],
        &[PROSE, LOG_META, PROSE_META, "Fetching crates.io ind…"],
    );
    app.key("down");
    flash_settle(&app);
    let after = app.settle();
    assert_size(&app, 120, 40);
    assert!(
        row_with(&after, "CLI").contains('▎') && !row_with(&after, "Local").contains('▎'),
        "nested selection did not move to CLI\n{}",
        text(&after)
    );
    see(
        &after,
        &[
            CRUMB,
            "2–14 of 24",
            "2–12 of 60",
            "Fetching crates.io index",
            "Target",
            "Cloud",
        ],
        &[
            PROSE,
            PROSE_META,
            LOG_META,
            LOG_LINE,
            "Fetching crates.io ind…",
            "Page options",
            "Compare levels",
            OVERVIEW,
        ],
    );
    shot(&app, "sc.panels.panels.after");
    app.finish();
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text("Titled card");
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

#[test]
fn option_focus_mark_bar() {
    let app = page("s07-focus", Some("panels.focus_mark=bar"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "bar", "theme", "12", "2", "auto", "on", "off", "theme", "app", "desktop", "f",
        );
    });
    click_log(&app);
    app.wait_text("Home End");
    flash_settle(&app);
    finish_option(&app, "opt.panels.focus_mark", |screen| {
        assert!(
            log_row(screen).contains("▎Card · scrollable"),
            "bar focus mark is missing on the log card\n{}",
            text(screen)
        );
        assert!(
            !row_with(screen, "› Panels").contains('▎'),
            "sidebar kept the focus gutter\n{}",
            text(screen)
        );
        see(
            screen,
            &[LOG_META, LOG_LINE, PROSE, PROSE_META, "Local"],
            &["2–12 of 60", "Page options", "50–60 of 60"],
        );
        assert_rules(screen, true);
    });
}

#[test]
fn option_heading_plain() {
    let app = page("s07-heading", Some("panels.heading=plain"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "plain", "12", "2", "auto", "on", "off", "theme", "app", "desktop",
            "f",
        );
    });
    click_tab(&app, "States");
    app.wait_text("Card · plain");
    flash_settle(&app);
    let states = app.settle();
    see(
        &states,
        &["Card · plain", "Card · bar mark", "Framed", "rest"],
        &[
            "Card · rule",
            "Card · band",
            "Card · emphasis",
            "Page options",
        ],
    );
    click_tab(&app, "Playground");
    app.wait_text("Titled card");
    flash_settle(&app);
    finish_option(&app, "opt.panels.heading", |screen| {
        assert_rules(screen, false);
        see(
            screen,
            &[
                "A card is a filled surface",
                FRAMED,
                PROSE_META,
                "Card · scrollable",
                LOG_META,
                LOG_LINE,
            ],
            &[
                "Card · plain",
                "Card · rule",
                "Page options",
                "Compare levels",
            ],
        );
    });
}

#[test]
fn option_title_min_20() {
    let app = page("s07-title-min", Some("panels.title_min=20"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "20", "2", "auto", "on", "off", "theme", "app", "desktop",
            "f",
        );
    });
    finish_option(&app, "opt.panels.title_min", |screen| {
        assert_playground(screen);
        see(
            screen,
            &[FRAMED, PROSE_META, "Card · scrollable", LOG_META],
            &["1–13/24", "1–11/60", "Fram…"],
        );
    });
}

#[test]
fn option_pad_x_0() {
    let app = page("s07-pad", Some("panels.pad_x=0"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "0", "auto", "on", "off", "theme", "app", "desktop",
            "f",
        );
    });
    finish_option(&app, "opt.panels.pad_x", |screen| {
        see(
            screen,
            &[
                "content starts at",
                "the padding edge",
                "Resolving workspace members",
                "Fetching crates.io index",
                FRAMED,
                PROSE_META,
                LOG_LINE,
                "Local",
                "Cloud",
            ],
            &[
                "starts at the padding edge",
                "Resolving workspace me…",
                "Page options",
                "Compare levels",
            ],
        );
        assert_rules(screen, true);
    });
}

#[test]
fn option_meta_short_custom() {
    let app = page("s07-meta", Some("panels.meta_short=custom"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "custom", "on", "off", "theme", "app", "desktop",
            "f",
        );
    });
    finish_option(&app, "opt.panels.meta_short", |screen| {
        assert_playground(screen);
        see(
            screen,
            &[FRAMED, PROSE_META, LOG_META, " of "],
            &["1–13/24", "1–11/60"],
        );
    });
}

#[test]
fn option_wrap_off() {
    let app = page("s07-wrap", Some("panels.wrap=off"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "auto", "off", "off", "theme", "app", "desktop",
            "f",
        );
    });
    finish_option(&app, "opt.panels.wrap", |screen| {
        see(
            screen,
            &[
                FRAMED,
                "Junie works through a task the way a c",
                "Each step is visible",
                LOG_META,
                LOG_LINE,
                "starts at the padding edge",
                "Local",
            ],
            &[
                PROSE_META,
                "careful engineer would",
                "Page options",
                "Compare levels",
            ],
        );
        assert_rules(screen, true);
    });
}

#[test]
fn option_tail_on() {
    let app = page("s07-tail", Some("panels.tail=on"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "auto", "on", "on", "theme", "app", "desktop", "f",
        );
    });
    finish_option(&app, "opt.panels.tail", |screen| {
        see(
            screen,
            &[
                "50–60 of 60",
                "following",
                "18.13s",
                "21.83s",
                PROSE,
                PROSE_META,
                "Local",
                "Cloud",
            ],
            &[LOG_META, LOG_LINE, "Page options", "Compare levels"],
        );
    });
}

#[test]
fn option_hscroll_off() {
    let app = page("s07-hscroll", Some("panels.hscroll=off"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "auto", "on", "off", "off", "app", "desktop", "f",
        );
    });
    finish_option(&app, "opt.panels.hscroll", |screen| {
        see(
            screen,
            &[
                "1–12 of 60",
                "4.07s",
                PROSE,
                PROSE_META,
                LOG_LINE,
                "Local",
                FRAMED,
            ],
            &[LOG_META, "Page options", "Compare levels", "50–60 of 60"],
        );
    });
}

#[test]
fn option_track_click_jump() {
    let app = page("s07-track", Some("panels.track_click=jump"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "auto", "on", "off", "theme", "jump", "desktop",
            "f",
        );
    });
    let screen = app.settle();
    let point = Point { x: 117, y: 34 };
    assert!(
        find_all(&screen, "┆").contains(&point),
        "log track is not at {point:?}\n{}",
        text(&screen)
    );
    app.click(point);
    app.wait_text("45–55 of 60");
    flash_settle(&app);
    finish_option(&app, "opt.panels.track_click", |screen| {
        see(
            screen,
            &[
                "45–55 of 60",
                "16.28s",
                PROSE,
                PROSE_META,
                "Local",
                "CLI",
                "Cloud",
            ],
            &[
                LOG_META,
                LOG_LINE,
                "12–22 of 60",
                "Page options",
                "Compare levels",
            ],
        );
    });
}

#[test]
fn option_nav_vim() {
    let app = page("s07-nav", Some("panels.nav=vim"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "auto", "on", "off", "theme", "app", "vim", "f",
        );
    });
    click_log(&app);
    app.wait_text("g G Ends");
    flash_settle(&app);
    let focused = app.settle();
    see(
        &focused,
        &["g G Ends", LOG_META, LOG_LINE, PROSE, PROSE_META, "Local"],
        &["Home End", "2–12 of 60"],
    );
    app.key("j");
    finish_option(&app, "opt.panels.nav", |screen| {
        see(
            screen,
            &[
                "g G Ends",
                "2–12 of 60",
                "0.37s",
                PROSE,
                PROSE_META,
                "Local",
                "CLI",
                "Cloud",
            ],
            &[
                "Home End",
                LOG_META,
                LOG_LINE,
                "Page options",
                "Compare levels",
            ],
        );
    });
}

#[test]
fn option_keymap_t() {
    let app = page("s07-keymap", Some("panels.keymap=t"), 120, 40);
    app.wait_text("Titled card");
    confirm_options(&app, |screen| {
        assert_panel_options(
            screen, "title", "theme", "12", "2", "auto", "on", "off", "theme", "app", "desktop",
            "t",
        );
    });
    app.key("t");
    app.key("f");
    finish_option(&app, "opt.panels.keymap", |screen| {
        assert_playground(screen);
        see(
            screen,
            &[LOG_META, LOG_LINE, PROSE, "Local"],
            &["Follow tail", "50–60 of 60", "2–12 of 60"],
        );
    });
}
