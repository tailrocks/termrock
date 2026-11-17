//! Disk cleanup route, sections, specimens, compare levels, options, and review.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s10_diskcleanup/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Screens › Disk cleanup";
const PREV: &str = "Screens › Finder";
const NEXT: &str = "Screens › Activity";
const TITLE: &str = "Disk cleanup";
const BLURB: &str = "Usage tree, filesystem meters, a cleanup checklist and a two-gate review";
const BLURB_HEAD: &str = "Usage tree, filesystem meters";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "diskcleanup"];
const WHEEL: Point = Point { x: 70, y: 18 };
const GATE1: &str = "Review · 4 categories · gate 1 of 2";
const GATE2: &str = "Move to Trash · gate 2 of 2";
const PHRASE: &str = "Type TRASH 4 to confirm";
const CANCELLED: &str = "Cleanup cancelled · nothing changed";
const EMPTY_STATUS: &str = "Select a category first (Space)";
const SCAN_DONE: &str = "scan complete";
const SCAN_STATUS: &str = "Scan complete";

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focus + hover",
    "disabled",
    "error",
    "editing",
    "loading",
    "overflow",
];

const DRAWN: &[(&str, &str)] = &[
    ("sc.diskcleanup.specimen.rest", "rest"),
    ("sc.diskcleanup.specimen.focused", "focused"),
    ("sc.diskcleanup.specimen.selected", "selected"),
    ("sc.diskcleanup.specimen.busy", "busy"),
    ("sc.diskcleanup.specimen.empty", "empty"),
    ("sc.diskcleanup.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s10_diskcleanup/{}", case_id.replace('.', "_"))
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

#[allow(clippy::too_many_arguments)]
fn assert_disk_options(
    screen: &TerminalSnapshot,
    mode: &str,
    typed: &str,
    check: &str,
    sizes: &str,
    meter: &str,
    scan: &str,
    collapse: &str,
) {
    assert_one(screen, "Mode", mode, &["trash", "permanent", "dry-run"]);
    assert_toggle(screen, "Typed phrase (gate 2)", typed);
    assert_one(screen, "Check on click", check, &["box", "row"]);
    assert_toggle(screen, "Tree sizes", sizes);
    assert_one(screen, "Meter visual", meter, &["line", "block"]);
    assert_one(screen, "Scan", scan, &["complete", "scanning"]);
    assert_one(screen, "Collapse below", collapse, &["72", "108"]);
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 8
            && (18..40).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
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

fn line_with_all(screen: &TerminalSnapshot, parts: &[&str]) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| parts.iter().all(|part| line.contains(part)))
        .unwrap_or_else(|| panic!("row {parts:?} missing\n{}", text(screen)))
}

fn assert_checked_four(screen: &TerminalSnapshot) {
    for (label, size) in [
        ("Xcode DerivedData", "810.0 MiB"),
        ("Xcode device support", "2.4 GiB"),
        ("Simulator caches", "900.0 MiB"),
        ("Cargo registry cache", "4.8 GiB"),
    ] {
        let line = line_with_all(screen, &[label, size]);
        assert!(line.contains('✓'), "{label} is not checked: {line:?}");
    }
    let pnpm = line_with_all(screen, &["pnpm store", "2.1 GiB"]);
    assert!(!pnpm.contains('✓'), "pnpm store started checked: {pnpm:?}");
    // The skip notice shares the words "Docker build cache" with the row.
    let docker = line_with_all(screen, &["Docker build cache", "6.3 GiB"]);
    assert!(
        !docker.contains('✓') && docker.contains("Docker is running"),
        "docker row is {docker:?}"
    );
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

fn sidebar(hit: &Point) -> bool {
    hit.x < 24
}

fn click_side(app: &App, label: &str) {
    reveal_side(app, label);
    click_where(app, label, sidebar);
}

fn click_content(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x > 26 && hit.y > 6 && hit.y < 38);
}

/// Dialog titles and fact rows share words with the action. The button sits
/// on the same row as Cancel, and the hint row also says Esc.
fn click_action(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let last = screen.rows().saturating_sub(1);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            u32::from(hit.y) < last
                && hit.y > 8
                && hit.x > 20
                && line.contains("Cancel")
                && !line.contains("Esc")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one action {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn reveal_side(app: &App, label: &str) {
    for key in ["up", "down"] {
        let mut previous = String::new();
        for _ in 0..60 {
            let screen = app.settle();
            if find_all(&screen, label)
                .into_iter()
                .any(|hit| sidebar(&hit))
            {
                return;
            }
            let visible = text(&screen);
            if visible == previous {
                break;
            }
            previous = visible;
            app.key(key);
        }
    }
}

fn reveal_text_at(app: &App, needle: &str, point: Point) {
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, true, 3);
    }
    app.wait_text(needle);
}

fn reveal_text(app: &App, needle: &str) {
    reveal_text_at(app, needle, Point { x: 70, y: 22 });
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

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(TITLE);
    flash_settle(app);
}

fn assert_idle(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Filesystems",
            SCAN_DONE,
            "31.8 GiB",
            "Usage · ~/work",
            "Xcode DerivedData",
            "4 of 10",
            "8.9 GiB",
            "Docker is running",
            "Docker build cache is skipped",
            "live · complete",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            SCAN_STATUS,
            "scanning ·",
            "Moved",
            "reclaimed",
            "would be freed",
            GATE1,
            OVERVIEW,
        ],
    );
    assert_checked_four(screen);
    let backend = row_with(screen, "▸ backend");
    let tree = backend.split("Allocated").next().unwrap_or(&backend);
    assert!(
        tree.contains("12.7 GiB"),
        "tree sizes are hidden\n{backend:?}"
    );
    assert!(
        row_with(screen, "/Volumes/Scratch").contains('━'),
        "root meter is not a line\n{}",
        row_with(screen, "/Volumes/Scratch")
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s10-route-page", 120, 40, PAGE_ARGS);
    app.wait_text(CRUMB);
    flash_settle(&app);
    let screen = app.settle();
    assert_size(&app, 120, 40);
    assert_idle(&screen);
    shot(&app, "sc.diskcleanup.route");
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
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Xcode DerivedData");
    see(
        &entered,
        &[CRUMB, BLURB, "4 of 10", "Docker is running"],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, BLURB, "Xcode DerivedData"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Xcode DerivedData", "4 of 10"],
        &[PREV, OVERVIEW],
    );
    app.key("]");
    let next = app.wait_text(NEXT);
    see(&next, &[NEXT], &[CRUMB, "Xcode DerivedData", BLURB]);
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, "Xcode DerivedData"]);
    let screen = app.settle();
    if !find_all(&screen, TITLE)
        .into_iter()
        .any(|hit| sidebar(&hit))
    {
        click_side(&app, "Screens");
    }
    click_side(&app, TITLE);
    let pointed = app.wait_text("Xcode DerivedData");
    see(
        &pointed,
        &[CRUMB, BLURB, "4 of 10", "Docker build cache"],
        &[OVERVIEW, "Page options", "Compare levels", PREV],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s10-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(TITLE);
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            CRUMB,
            BLURB_HEAD,
            "Playground",
            "Filesystems",
            "work",
            "backend",
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
            BLURB,
            "Review deletion",
            "Xcode DerivedData",
        ],
    );
    shot(&app, "sc.diskcleanup.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_disk_options(
        &narrow_opt,
        "trash",
        "on",
        "box",
        "on",
        "line",
        "complete",
        "72",
    );
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Typed phrase (gate 2)",
            "Collapse below",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            "Xcode DerivedData",
            "Filesystems",
        ],
    );
    shot(&app, "sc.diskcleanup.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "focused", "selected"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    for label in ["busy", "empty", "narrow"] {
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
            "Usage tree",
            "Categories",
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
            "Nothing here yet",
            "Xcode DerivedData",
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.diskcleanup.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
            && text(screen).contains("Nothing here yet")
    });
    assert_size(&app, 120, 40);
    for (_, label) in DRAWN {
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
            "Usage tree",
            "Categories",
            "Filesystems",
            "Nothing here yet",
            "DerivedData",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Xcode DerivedData",
            "256 colors",
            "focus + hover",
        ],
    );
    shot(&app, "sc.diskcleanup.states.120x40");
    shot(&app, "sc.diskcleanup.states");
    for (case_id, label) in DRAWN {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_disk_options(
        &wide_opt, "trash", "on", "box", "on", "line", "complete", "72",
    );
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Meter visual"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Xcode DerivedData",
            "Filesystems",
        ],
    );
    shot(&app, "sc.diskcleanup.options.120x40");
    shot(&app, "sc.diskcleanup.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Xcode DerivedData");
    assert_size(&app, 120, 40);
    assert_idle(&wide_play);
    shot(&app, "sc.diskcleanup.playground.120x40");
    shot(&app, "sc.diskcleanup.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s10-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES, "Usage tree"],
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
    for _ in 0..120 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel(WHEEL, true, 3);
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
    shot(&app, "sc.diskcleanup.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    let settled = app.settle();
    assert_eq!((settled.cols(), settled.rows()), (80, 24));
    if !text(&settled).contains("no color") {
        reveal_text_at(&app, "no color", WHEEL);
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.diskcleanup.compare-levels.80x24");

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
    assert_disk_options(
        &options, "trash", "on", "box", "on", "line", "complete", "72",
    );
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "Tree sizes"],
        &["Compare levels", OVERVIEW, "Xcode DerivedData"],
    );
    app.finish();
}

fn click_review(app: &App) {
    reveal_text(app, "Review deletion");
    click_content(app, "Review deletion");
    flash_settle(app);
}

fn open_review(app: &App) {
    click_review(app);
    app.wait_text("gate 1");
}

#[test]
fn review_gates_cancel_keeps_categories() {
    let app = page("s10-gates", None, 120, 40);
    let before = app.wait_text("Xcode DerivedData");
    assert_size(&app, 120, 40);
    assert_idle(&before);
    open_review(&app);
    let gate1 = app.wait_text(GATE1);
    see(
        &gate1,
        &[
            GATE1,
            "Continue…",
            "Cancel",
            "Move to Trash 4 categories",
            "trash ~/Library/Caches/xcode-deriveddata",
            "8.9 GiB allocated",
            "yes · until the Trash is emptied",
        ],
        &[
            GATE2,
            PHRASE,
            "Moved",
            "reclaimed",
            "would be freed",
            SCAN_STATUS,
        ],
    );
    shot(&app, "sc.diskcleanup.cleanup");
    click_action(&app, "Continue");
    app.wait_text(PHRASE);
    flash_settle(&app);
    let gate2 = app.wait_text(GATE2);
    see(
        &gate2,
        &[GATE2, PHRASE, "Move to Trash", "Cancel", "TRASH 4"],
        &[
            "Continue…",
            "Moved",
            "reclaimed",
            "would be freed",
            SCAN_STATUS,
            "DELETE 4",
        ],
    );
    shot(&app, "sc.diskcleanup.cleanup.gate2");
    app.type_text("TRASH 9");
    app.key("enter");
    flash_settle(&app);
    let wrong = app.settle();
    see(
        &wrong,
        &[GATE2, "TRASH 9", PHRASE],
        &["Moved", "reclaimed", "would be freed", CANCELLED],
    );
    click_action(&app, "Cancel");
    flash_settle(&app);
    let after = app.wait_text(CANCELLED);
    see(
        &after,
        &[
            CANCELLED,
            "Xcode DerivedData",
            "4 of 10",
            "8.9 GiB",
            "Docker build cache",
            CRUMB,
        ],
        &[
            GATE1,
            GATE2,
            PHRASE,
            "Moved",
            "reclaimed",
            "would be freed",
            "Continue…",
        ],
    );
    assert_checked_four(&after);
    shot(&app, "sc.diskcleanup.cleanup.after");
    app.finish();
}

#[test]
fn rescan_paused_does_not_finish() {
    let app = page("s10-rescan", None, 120, 40);
    app.wait_text(SCAN_DONE);
    reveal_text(&app, "Rescan");
    click_content(&app, "Rescan");
    flash_settle(&app);
    let scanning = app.settle();
    assert_size(&app, 120, 40);
    see(
        &scanning,
        &[
            "scanning · 5 left",
            "Rescan",
            "Xcode DerivedData",
            "4 of 10",
        ],
        &[SCAN_STATUS, SCAN_DONE, "Moved", "reclaimed", GATE1],
    );
    assert!(
        row_with(&scanning, "Rescan").contains('⠋'),
        "rescan is not busy\n{}",
        row_with(&scanning, "Rescan")
    );
    shot(&app, "sc.diskcleanup.cleanup.rescan");
    app.finish();
}

#[test]
fn empty_selection_does_not_open() {
    let app = page("s10-empty", None, 120, 40);
    app.wait_text("Xcode DerivedData");
    for _ in 0..4 {
        let screen = app.settle();
        let hit = find_all(&screen, "✓")
            .into_iter()
            .filter(|hit| hit.x > 26 && hit.y > 8 && hit.y < 36)
            .min_by_key(|hit| hit.y)
            .unwrap_or_else(|| panic!("no check mark\n{}", text(&screen)));
        app.click(hit);
        flash_settle(&app);
    }
    let cleared = app.settle();
    assert!(
        text(&cleared).contains("Xcode DerivedData"),
        "unchecking left the page\n{}",
        text(&cleared)
    );
    assert!(
        !row_with(&cleared, "Xcode DerivedData").contains('✓'),
        "derived data stayed checked\n{}",
        row_with(&cleared, "Xcode DerivedData")
    );
    see(
        &cleared,
        &["0 categories", "pnpm store", "Docker build cache"],
        &[GATE1, "Moved", "reclaimed"],
    );
    // A check click toggles the mark and leaves keyboard focus on the sidebar.
    // Tab reaches the category list; d is the list's review key.
    for _ in 0..16 {
        if text(&app.settle()).contains("Review deletion") {
            break;
        }
        app.key("tab");
    }
    let focused = app.settle();
    assert!(
        text(&focused).contains("Review deletion") && text(&focused).contains("0 categories"),
        "list did not take focus with an empty selection\n{}",
        text(&focused)
    );
    app.key("d");
    flash_settle(&app);
    let status = app.settle();
    see(
        &status,
        &[
            EMPTY_STATUS,
            "Xcode DerivedData",
            "pnpm store",
            "Docker build cache",
        ],
        &[
            GATE1,
            GATE2,
            "Continue…",
            "Moved",
            "reclaimed",
            "would be freed",
        ],
    );
    shot(&app, "sc.diskcleanup.cleanup.empty");
    app.finish();
}

#[test]
fn dry_run_keeps_rows() {
    let app = page("s10-dry", Some("diskcleanup.mode=dry-run"), 120, 40);
    let screen = app.wait_text("Xcode DerivedData");
    see(
        &screen,
        &[
            "dry run · nothing is",
            "review only · nothing",
            "4 of 10",
            "Xcode DerivedData",
        ],
        &["two gates", "permanent delete", GATE1],
    );
    click_review(&app);
    let dialog = app.wait_text("Dry run · 4 categories");
    see(
        &dialog,
        &[
            "Dry run · 4 categories",
            "Run dry run",
            "du -sh",
            "nothing changes",
            "Cancel",
        ],
        &[GATE2, "Continue…", PHRASE, "Delete permanently", "Moved"],
    );
    shot(&app, "sc.diskcleanup.cleanup.dry");
    click_action(&app, "Run dry run");
    flash_settle(&app);
    let after = app.wait_text("would be freed");
    see(
        &after,
        &[
            "would be freed",
            "Dry run:",
            "Xcode DerivedData",
            "4 of 10",
            "Docker build cache",
        ],
        &[
            GATE1,
            GATE2,
            "Moved",
            "reclaimed",
            "Continue…",
            "Run dry run",
        ],
    );
    assert_checked_four(&after);
    app.finish();
}

#[test]
fn confirm_typed_phrase_removes_checked_rows() {
    let app = page("s10-confirm", None, 120, 40);
    app.wait_text("Xcode DerivedData");
    open_review(&app);
    click_action(&app, "Continue");
    app.wait_text(PHRASE);
    flash_settle(&app);
    app.type_text("TRASH 4");
    app.key("enter");
    flash_settle(&app);
    let after = app.settle();
    see(
        &after,
        &[
            "Moved 4 categories to the Trash",
            "reclaimed",
            "pnpm store",
            "Docker build cache",
            "Docker is running",
        ],
        &[
            GATE1,
            GATE2,
            PHRASE,
            "Xcode DerivedData",
            "Xcode device support",
            "Simulator caches",
            "Cargo registry cache",
            "would be freed",
            SCAN_STATUS,
        ],
    );
    shot(&app, "sc.diskcleanup.cleanup.confirmed");
    app.finish();
}

#[test]
fn option_mode_permanent() {
    let app = page("s10-mode", Some("diskcleanup.mode=permanent"), 120, 40);
    app.wait_text("Xcode DerivedData");
    confirm_options(&app, |screen| {
        assert_disk_options(
            screen,
            "permanent",
            "on",
            "box",
            "on",
            "line",
            "complete",
            "72",
        );
    });
    let screen = app.settle();
    see(
        &screen,
        &["permanent delete · no", "review, then type DEL", "4 of 10"],
        &[
            "Trash · space returns",
            "dry run · nothing",
            "review, then confirm",
        ],
    );
    open_review(&app);
    let gate1 = app.wait_text("gate 1");
    see(
        &gate1,
        &[
            GATE1,
            "Delete permanently 4 categories",
            "rm -rf",
            "Continue…",
            "permanent delete · not recoverable",
        ],
        &[PHRASE, "Move to Trash 4", "Run dry run", "du -sh"],
    );
    click_action(&app, "Continue");
    app.wait_text("gate 2");
    flash_settle(&app);
    let gate2 = app.settle();
    see(
        &gate2,
        &[
            "Delete permanently · gate 2 of 2",
            "Type DELETE 4 to confirm",
            "Delete permanently",
            "DELETE 4",
        ],
        &[PHRASE, "Move to Trash", "Run dry run", "Moved", "reclaimed"],
    );
    click_action(&app, "Cancel");
    flash_settle(&app);
    let after = app.wait_text(CANCELLED);
    see(
        &after,
        &[CANCELLED, "Xcode DerivedData", "permanent delete"],
        &["Moved", "reclaimed", "would be freed", "DELETE 4"],
    );
    assert_checked_four(&after);
    shot(&app, "opt.diskcleanup.mode");
    app.finish();
}

#[test]
fn option_typed_off() {
    let app = page("s10-typed", Some("diskcleanup.typed=off"), 120, 40);
    app.wait_text("Xcode DerivedData");
    confirm_options(&app, |screen| {
        assert_disk_options(
            screen, "trash", "off", "box", "on", "line", "complete", "72",
        );
    });
    let screen = app.settle();
    see(
        &screen,
        &["review, then confirm", "Trash · space returns", "4 of 10"],
        &["review, then type", PHRASE, "TRASH 4"],
    );
    open_review(&app);
    click_action(&app, "Continue");
    app.wait_text("gate 2");
    flash_settle(&app);
    let gate2 = app.settle();
    see(
        &gate2,
        &[GATE2, "Move to Trash", "Cancel"],
        &[PHRASE, "Type TRASH", "TRASH 4", "DELETE 4"],
    );
    click_action(&app, "Cancel");
    flash_settle(&app);
    app.wait_text(CANCELLED);
    shot(&app, "opt.diskcleanup.typed");
    app.finish();
}

#[test]
fn option_check_click() {
    let box_app = page("s10-check-box", None, 120, 40);
    let idle = box_app.wait_text("pnpm store");
    assert!(
        !row_with(&idle, "pnpm store").contains('✓'),
        "idle pnpm row changed before the click"
    );
    assert!(
        flat(&idle).contains("4 of 10"),
        "idle selection is not 4 of 10"
    );
    click_content(&box_app, "pnpm store");
    flash_settle(&box_app);
    let boxed = box_app.settle();
    // A row click in box mode replaces the checks. It does not toggle.
    see(
        &boxed,
        &["1 category", "2.1 GiB"],
        &["4 of 10", "5 categories", GATE1],
    );
    assert!(row_with(&boxed, "pnpm store").contains('✓'));
    assert!(!row_with(&boxed, "Xcode DerivedData").contains('✓'));
    box_app.finish();

    let app = page(
        "s10-check-row",
        Some("diskcleanup.check_click=row"),
        120,
        40,
    );
    app.wait_text("pnpm store");
    confirm_options(&app, |screen| {
        assert_disk_options(screen, "trash", "on", "row", "on", "line", "complete", "72");
    });
    let before = app.settle();
    assert!(!row_with(&before, "pnpm store").contains('✓'));
    assert!(flat(&before).contains("4 of 10"));
    click_content(&app, "pnpm store");
    flash_settle(&app);
    let after = app.settle();
    see(
        &after,
        &["5 categories", "11.0 GiB", "5 of 10"],
        &["1 category", "4 of 10", GATE1],
    );
    assert!(row_with(&after, "pnpm store").contains('✓'));
    assert!(row_with(&after, "Xcode DerivedData").contains('✓'));
    shot(&app, "opt.diskcleanup.check_click");
    app.finish();
}

#[test]
fn option_sizes_off() {
    let app = page("s10-sizes", Some("diskcleanup.sizes=off"), 120, 40);
    app.wait_text("Xcode DerivedData");
    confirm_options(&app, |screen| {
        assert_disk_options(
            screen, "trash", "on", "box", "off", "line", "complete", "72",
        );
    });
    let screen = app.settle();
    let backend = row_with(&screen, "▸ backend");
    let tree = backend.split("Allocated").next().unwrap_or(&backend);
    assert!(
        !tree.contains("GiB") && !tree.contains("MiB"),
        "tree size meta stayed visible: {backend:?}"
    );
    see(
        &screen,
        &[
            "Xcode DerivedData",
            "810.0 MiB",
            "Allocated",
            "31.8 GiB",
            SCAN_DONE,
        ],
        &["12.7 GiB", "Page options"],
    );
    shot(&app, "opt.diskcleanup.sizes");
    app.finish();
}

#[test]
fn option_meter_block() {
    let app = page("s10-meter", Some("diskcleanup.meter_visual=block"), 120, 40);
    app.wait_text("Filesystems");
    confirm_options(&app, |screen| {
        assert_disk_options(
            screen, "trash", "on", "box", "on", "block", "complete", "72",
        );
    });
    let screen = app.settle();
    let scratch = row_with(&screen, "/Volumes/Scratch");
    assert!(
        scratch.contains("12%") && !scratch.contains('━'),
        "block meter still draws a line: {scratch:?}"
    );
    see(
        &screen,
        &["90%", "71%", "842 of 926 GiB", "Xcode DerivedData"],
        &["Page options"],
    );
    shot(&app, "opt.diskcleanup.meter_visual");
    app.finish();
}

#[test]
fn option_scan_scanning() {
    let app = page("s10-scan", Some("diskcleanup.scan=scanning"), 120, 40);
    app.wait_text("scanning · 3 left");
    confirm_options(&app, |screen| {
        assert_disk_options(screen, "trash", "on", "box", "on", "line", "scanning", "72");
    });
    let screen = app.settle();
    see(
        &screen,
        &[
            "scanning · 3 left",
            "refreshing",
            "Xcode DerivedData",
            "4 of 10",
        ],
        &[SCAN_DONE, SCAN_STATUS, "90%", "Moved", "reclaimed"],
    );
    let dumps = row_with(&screen, "dumps");
    assert!(
        dumps.contains('⠋') && !dumps.contains('▸'),
        "dumps is not busy: {dumps:?}"
    );
    assert!(
        row_with(&screen, "▸ backend").contains("12.7 GiB"),
        "an early child was marked busy"
    );
    shot(&app, "opt.diskcleanup.scan");
    app.finish();
}

#[test]
fn option_collapse_below() {
    let app = page(
        "s10-collapse",
        Some("diskcleanup.collapse_below=108"),
        120,
        40,
    );
    app.wait_text("Tab shows details");
    confirm_options(&app, |screen| {
        assert_disk_options(
            screen, "trash", "on", "box", "on", "line", "complete", "108",
        );
    });
    let screen = app.settle();
    see(
        &screen,
        &[
            "Tab shows details",
            "→ ~/work · 31.8 GiB",
            "Xcode DerivedData",
            "4 of 10",
            SCAN_DONE,
        ],
        &["Freshness", "live · complete", "Page options", "Details"],
    );
    shot(&app, "opt.diskcleanup.collapse_below");
    app.finish();
}
