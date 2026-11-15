//! Finder route, sections, specimens, compare levels, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s09_finder/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Screens › Finder";
const PREV: &str = "Screens › Connections";
const NEXT: &str = "Screens › Disk cleanup";
const TITLE: &str = "Finder";
const BLURB: &str = "Typing-hot query, scope, ranked results, preview, alternatives and why";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "finder"];
const PLACEHOLDER: &str = "Search actions, files and tasks";
const NO_MATCH: &str = "Nothing among 6 actions here matches";
const PROJECT_NO_MATCH: &str = "Nothing among 10 actions here matches";

const DRAWN: &[&str] = &[
    "rest", "hover", "focused", "selected", "empty", "overflow", "narrow",
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s09_finder/{}", case_id.replace('.', "_"))
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

fn assert_finder_options(
    screen: &TerminalSnapshot,
    query: &str,
    scope: &str,
    risk: &str,
    preview: &str,
) {
    assert_one(screen, "Query", query, &["typed", "empty", "nomatch"]);
    assert_one(screen, "Scope", scope, &["here", "project", "everywhere"]);
    assert_toggle(screen, "Risk column", risk);
    assert_one(screen, "Preview", preview, &["auto", "line"]);
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

/// The 140ms press flash stays painted while motion is paused, so a shot after
/// a released click has to redraw once the deadline has passed. Below about 85
/// columns the header omits the size chip, and the narrow nudge waits on the
/// terminal size instead.
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

fn confirm_options(app: &App, back: &str, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(back);
    flash_settle(app);
}

fn assert_typed_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "pu",
            "scope ‹ here ›",
            "Pull",
            "Push",
            "Pull with rebase",
            "up to date",
            "2 commits ahead",
            "git pull --ff-only",
            "Alternatives…",
            "Why?",
            "mutating",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Stash changes",
            "No matches",
            "Nothing selected",
            "scope ‹ project ›",
            "scope ‹ everywhere ›",
            "zzq",
            PLACEHOLDER,
            "Compose up",
            "Prune images",
            "→ git pull",
            OVERVIEW,
            "focus + hover",
        ],
    );
}

fn replace_query(app: &App, shown: &str, value: &str) {
    click_where(app, shown, |hit| hit.y == 8 && hit.x < 50);
    flash_settle(app);
    app.key("ctrl+l");
    if value.is_empty() {
        app.key("backspace");
    } else {
        app.type_text(value);
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s09-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_typed_playground(&screen);
    shot(&app, "sc.finder.route");
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
        "Screens › Task runner",
        "Screens › Query workbench",
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("git pull --ff-only");
    see(
        &entered,
        &[CRUMB, BLURB, "pu", "Pull", "Alternatives…"],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, "scope ‹ here ›", "Alternatives…"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Pull", "Why?"],
        &[PREV, OVERVIEW, "No matches"],
    );
    app.key("]");
    let next = app.wait_text(NEXT);
    see(
        &next,
        &[NEXT],
        &[CRUMB, "scope ‹ here ›", "git pull --ff-only"],
    );
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(
        &overview,
        &[OVERVIEW],
        &[CRUMB, "scope ‹ here ›", "Alternatives…"],
    );
    click_side(&app, "Screens");
    click_side(&app, "Finder");
    let pointed = app.wait_text("git pull --ff-only");
    see(
        &pointed,
        &[CRUMB, BLURB, "pu", "Pull", "Push", "Why?"],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            PREV,
            "No matches",
        ],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s09-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("git pull --ff-only");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Typing-hot query",
            "Playground",
            "pu",
            "scope ‹ here ›",
            "Pull",
            "Push",
            "Pull with rebase",
            "Runs git pull --ff-only",
            "mutating",
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
            "Alternatives…",
            "Why?",
            "Stash changes",
            "No matches",
            BLURB,
        ],
    );
    shot(&app, "sc.finder.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_finder_options(&narrow_opt, "typed", "here", "on", "auto");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Query", "typed", "Risk column"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            "Stash changes",
            "focus + hover",
        ],
    );
    shot(&app, "sc.finder.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "hover", "focused"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    for label in ["selected", "empty", "overflow", "narrow"] {
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
            "Scope",
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
            "focus + hover",
            "pressed",
        ],
    );
    assert!(
        !flat(&narrow_states)
            .split_whitespace()
            .any(|word| word == "pu"),
        "states grid shows the playground query\n{}",
        text(&narrow_states)
    );
    shot(&app, "sc.finder.states.80x24");

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
            "No matches",
            "Search actions",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "pressed",
            "disabled",
            "busy",
            "loading",
            "256 colors",
        ],
    );
    for (case_id, label) in [
        ("sc.finder.specimen.rest", "rest"),
        ("sc.finder.specimen.hover", "hover"),
        ("sc.finder.specimen.focused", "focused"),
        ("sc.finder.specimen.selected", "selected"),
        ("sc.finder.specimen.empty", "empty"),
        ("sc.finder.specimen.overflow", "overflow"),
        ("sc.finder.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.finder.states.120x40");
    shot(&app, "sc.finder.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_finder_options(&wide_opt, "typed", "here", "on", "auto");
    see(
        &wide_opt,
        &[
            CRUMB,
            CAP_OPT,
            WIDE,
            "Page options",
            "typed",
            "here",
            "auto",
        ],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "No matches"],
    );
    shot(&app, "sc.finder.options.120x40");
    shot(&app, "sc.finder.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("git pull --ff-only");
    assert_size(&app, 120, 40);
    assert_typed_playground(&wide_play);
    shot(&app, "sc.finder.playground.120x40");
    shot(&app, "sc.finder.playground");
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
    shot(&app, "sc.finder.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    let _ = narrow;
    let narrow = app.settle();
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.finder.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
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
    assert_finder_options(&options, "typed", "here", "on", "auto");
    see(
        &options,
        &["typed", "here", "auto", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "nomatch", "project"],
    );
    app.finish();
}

#[test]
fn edit_query_scope_run_why_and_empty() {
    let app = open("s09-finder", 120, 40, PAGE_ARGS);
    let before = app.wait_text("git pull --ff-only");
    assert_size(&app, 120, 40);
    assert_typed_playground(&before);
    shot(&app, "sc.finder.finder.before");

    replace_query(&app, "pu", "push");
    let narrowed = app.wait_text("git push");
    see(
        &narrowed,
        &[
            "push",
            "git push",
            "2 commits ahead",
            "remote",
            "scope ‹ here ›",
        ],
        &[
            "git pull --ff-only",
            "Pull with rebase",
            "Stash changes",
            "No matches",
            "scope ‹ project ›",
        ],
    );
    app.key("alt+up");
    let scoped = app.wait_text("scope ‹ project ›");
    see(
        &scoped,
        &["push", "git push", "scope ‹ project ›", "Why?"],
        &[
            "scope ‹ here ›",
            "scope ‹ everywhere ›",
            "Compose up",
            "No matches",
        ],
    );
    click_where(&app, "Push", |hit| hit.y > 9 && hit.x < 70);
    flash_settle(&app);
    app.key("enter");
    let ran = app.wait_text("Ran Push");
    see(
        &ran,
        &["Ran Push", "git push", "Why?", "scope ‹ project ›"],
        &["Ran Pull", "Ran Build", "git pull --ff-only", "No matches"],
    );

    click_where(&app, "Alternatives", |hit| hit.y > 6);
    flash_settle(&app);
    let menu = app.wait_text("Push · alternatives");
    see(
        &menu,
        &[
            "Push · alternatives",
            "Push with lease",
            "Push tags",
            "Ran Push",
        ],
        &["Fetch only", "Why Push?", "Pull · alternatives", "Ran Pull"],
    );
    app.key("esc");
    app.wait_absent("Push · alternatives");

    click_where(&app, "Why?", |hit| hit.y > 6);
    flash_settle(&app);
    let why = app.wait_text("Why Push?");
    see(
        &why,
        &[
            "Why Push?",
            "used 7 times this week",
            "remote",
            "\"push\" starts the name",
            "Ran Push",
        ],
        &[
            "Why Pull?",
            "used 9 times this week",
            "Why Compose up?",
            "Fetch only",
            "Ran Pull",
        ],
    );
    app.key("esc");
    let closed = app.wait_absent("Why Push?");
    see(
        &closed,
        &["git push", "2 commits ahead", "remote", "Ran Push", "Why?"],
        &[
            "Why Push?",
            "git pull --ff-only",
            "No matches",
            "Push · alternatives",
        ],
    );

    replace_query(&app, "push", "");
    let empty = app.wait_text("Stash changes");
    see(
        &empty,
        &[
            PLACEHOLDER,
            "Stash changes",
            "Build",
            "Compose up",
            "scope ‹ project ›",
            "Ran Push",
            "git pull --ff-only",
        ],
        &[
            "Ran Pull",
            "Ran Build",
            "Ran Compose",
            "No matches",
            "zzq",
            "push",
        ],
    );
    replace_query(&app, "Search actions", "zzq");
    let none = app.wait_text("No matches");
    see(
        &none,
        &[
            "zzq",
            "No matches",
            "Nothing selected",
            PROJECT_NO_MATCH,
            "Ran Push",
            "scope ‹ project ›",
        ],
        &[
            "Ran Pull",
            "Stash changes",
            "git push",
            "git pull",
            "Alternatives…",
            PLACEHOLDER,
            "Why Push?",
        ],
    );
    shot(&app, "sc.finder.finder.after");
    app.finish();
}

#[test]
fn option_query_empty_and_nomatch() {
    let app = page("s09-query-empty", Some("finder.query=empty"), 120, 40);
    let empty = app.wait_text("Stash changes");
    see(
        &empty,
        &[
            CAP_PLAY,
            PLACEHOLDER,
            "Stash changes",
            "Build",
            "nextest",
            "scope ‹ here ›",
            "Alternatives…",
            "git pull --ff-only",
        ],
        &[
            "No matches",
            "zzq",
            "Page options",
            "Compose up",
            "Prune images",
        ],
    );
    assert!(
        !flat(&empty).split_whitespace().any(|word| word == "pu"),
        "empty query still shows pu\n{}",
        text(&empty)
    );
    confirm_options(&app, "Stash changes", |screen| {
        assert_finder_options(screen, "empty", "here", "on", "auto");
    });
    app.finish();

    let app = page("s09-query-nomatch", Some("finder.query=nomatch"), 120, 40);
    let none = app.wait_text("No matches");
    see(
        &none,
        &[
            "zzq",
            "No matches",
            "Nothing selected",
            NO_MATCH,
            "scope ‹ here ›",
            "Preview",
        ],
        &[
            "Stash changes",
            "Alternatives…",
            "git pull",
            PLACEHOLDER,
            "Compose up",
        ],
    );
    confirm_options(&app, "No matches", |screen| {
        assert_finder_options(screen, "nomatch", "here", "on", "auto");
    });
    let screen = app.wait_text("Nothing selected");
    see(
        &screen,
        &[CAP_PLAY, "zzq", "No matches", "Nothing selected", NO_MATCH],
        &["Page options", "Stash changes", "typed", PLACEHOLDER],
    );
    shot(&app, "opt.finder.query");
    app.finish();
}

#[test]
fn option_scope_project() {
    let app = page("s09-scope", Some("finder.scope=project"), 120, 40);
    let play = app.wait_text("Compose up");
    see(
        &play,
        &[
            CAP_PLAY,
            "pu",
            "scope ‹ project ›",
            "Compose up",
            "3 services",
            "Pull",
            "git pull --ff-only",
        ],
        &[
            "scope ‹ here ›",
            "scope ‹ everywhere ›",
            "Prune images",
            "No matches",
            "Page options",
            "Stash changes",
        ],
    );
    confirm_options(&app, "Compose up", |screen| {
        assert_finder_options(screen, "typed", "project", "on", "auto");
    });
    let screen = app.wait_text("scope ‹ project ›");
    see(
        &screen,
        &[CAP_PLAY, "Compose up", "scope ‹ project ›", "pu"],
        &["Page options", "Prune images", "scope ‹ here ›"],
    );
    shot(&app, "opt.finder.scope");
    app.finish();
}

#[test]
fn option_risk_column_off() {
    let app = page("s09-risk", Some("finder.risk_column=off"), 120, 40);
    let play = app.wait_text("git pull --ff-only");
    let push = row_with(&play, "Push");
    assert!(
        push.contains("2 commits ahead") && !push.contains("remote"),
        "risk column off still shows the risk beside Push\n{push}"
    );
    see(
        &play,
        &[
            CAP_PLAY,
            "pu",
            "Pull",
            "Push",
            "git pull --ff-only",
            "Risk",
            "mutating",
            "Alternatives…",
        ],
        &["Page options", "No matches", "remote", "Stash changes"],
    );
    confirm_options(&app, "git pull --ff-only", |screen| {
        assert_finder_options(screen, "typed", "here", "off", "auto");
    });
    let screen = app.settle();
    let push = row_with(&screen, "Push");
    assert!(
        !push.contains("remote"),
        "returning from options restored the risk column\n{push}"
    );
    shot(&app, "opt.finder.risk_column");
    app.finish();
}

#[test]
fn option_preview_line() {
    let app = page("s09-preview", Some("finder.preview=line"), 120, 40);
    let play = app.wait_text("git pull --ff-only");
    let push = row_with(&play, "Push");
    let rebase = row_with(&play, "Pull with rebase");
    assert!(
        push.contains("remote"),
        "line preview hid the risk column\n{push}"
    );
    assert!(
        rebase.contains("rewrites local commits"),
        "line preview still clipped the note\n{rebase}"
    );
    see(
        &play,
        &[
            CAP_PLAY,
            "pu",
            "Runs git pull --ff-only",
            "here",
            "mutating",
            "remote",
        ],
        &[
            "Alternatives…",
            "Why?",
            "Page options",
            "Nothing selected",
            "Preview",
            "No matches",
        ],
    );
    confirm_options(&app, "git pull --ff-only", |screen| {
        assert_finder_options(screen, "typed", "here", "on", "line");
    });
    let screen = app.wait_text("rewrites local commits");
    see(
        &screen,
        &[CAP_PLAY, "Runs git pull --ff-only", "remote"],
        &["Page options", "Alternatives…", "Why?"],
    );
    shot(&app, "opt.finder.preview");
    app.finish();
}

fn open_with(name: &str, opts: &[&str]) -> App {
    let mut args = PAGE_ARGS.to_vec();
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    open(name, 120, 40, &args)
}

#[test]
fn option_rows_grouped_headings_are_never_selected() {
    let app = open_with(
        "s09-rows-grouped",
        &["finder.rows=grouped", "finder.query=empty"],
    );
    let screen = app.wait_text("Stash changes");
    see(
        &screen,
        &["git", "cargo", "Pull", "Build", "Test"],
        &["Ran ", "No matches"],
    );
    app.key("tab");
    app.key("tab");
    app.key("home");
    app.key("enter");
    let first = app.wait_text("Ran ");
    see(&first, &["Ran "], &["Ran git", "Ran cargo"]);
    app.key("end");
    app.key("enter");
    let last = app.settle();
    see(&last, &["Ran "], &["Ran git", "Ran cargo", "Ran docker"]);
    app.finish();
}

#[test]
fn option_rows_cells_run_on_one_click() {
    let app = open_with(
        "s09-rows-cells",
        &["finder.rows=cells", "finder.query=empty"],
    );
    let screen = app.wait_text("Stash changes");
    see(&screen, &["git", "cargo", "Build", "Test"], &["Ran "]);
    click_where(&app, "Build", |hit| hit.y > 9 && hit.x < 70);
    let ran = app.wait_text("Ran Build");
    see(&ran, &["Ran Build"], &["Ran Test", "Ran Pull"]);
    app.finish();
}

#[test]
fn option_results_failing_reports_and_retries() {
    let app = open_with("s09-failing", &["finder.results=failing"]);
    app.wait_text("git pull --ff-only");
    click_where(&app, "pu", |hit| hit.y == 8 && hit.x < 50);
    flash_settle(&app);
    app.key("end");
    app.type_text("s");
    let failed = app.wait_text("Search failed");
    see(
        &failed,
        &["Search failed", "Ctrl+R retries", "Pull", "Push"],
        &["No matches"],
    );
    app.key("ctrl+r");
    let retried = app.wait_absent("Search failed");
    see(
        &retried,
        &["pus", "Push", "2 commits ahead"],
        &["Search failed", "Searching"],
    );
    app.finish();
}

#[test]
fn option_results_loading_dims_and_blocks_actions() {
    let app = open_with("s09-loading", &["finder.results=loading"]);
    app.wait_text("git pull --ff-only");
    click_where(&app, "pu", |hit| hit.y == 8 && hit.x < 50);
    flash_settle(&app);
    app.key("end");
    app.type_text("s");
    let loading = app.wait_text("Searching");
    see(&loading, &["Searching", "Pull", "Push"], &["Search failed"]);
    app.key("tab");
    app.key("enter");
    let after = app.settle();
    see(
        &after,
        &["Searching", "Pull", "Ctrl+O Preview", "Ctrl+F Search"],
        &["Ran "],
    );
    app.finish();
}
