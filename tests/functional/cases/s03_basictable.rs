//! Basic table route, sections, specimens, compare levels, sort, and selection.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s03_basictable/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Tables › Basic table";
const TITLE: &str = "Tables";
const BLURB: &str = "Sort by header, hover rows, select with Enter, overflow scrolls";
const BLURB_NARROW: &str = "Sort by header, hover rows, select with Enter, ov…";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const UNSORTED: &str = "unsorted";
const SORT_ASC: &str = "sorted by task ▴";
const SORT_DESC: &str = "sorted by task ▾";
const ACTIVATED: &str = "Selected Migrate sessions table to UUID keys";

const TOP_STATES: &[(&str, &str)] = &[
    ("sc.basictable.specimen.rest", "rest"),
    ("sc.basictable.specimen.hover", "hover"),
    ("sc.basictable.specimen.pressed", "pressed"),
    ("sc.basictable.specimen.focused", "focused"),
    ("sc.basictable.specimen.focused_hover", "focus + hover"),
    ("sc.basictable.specimen.selected", "selected"),
    ("sc.basictable.specimen.disabled", "disabled"),
];

const LOW_STATES: &[(&str, &str)] = &[
    ("sc.basictable.specimen.empty", "empty"),
    ("sc.basictable.specimen.overflow", "overflow"),
    ("sc.basictable.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s03_basictable/{}", case_id.replace('.', "_"))
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

fn option_value(screen: &TerminalSnapshot, label: &str) -> String {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 20)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "option {label:?} got {hits:?}\n{}",
        text(screen)
    );
    row_text(screen, u32::from(hits[0].y) + 1)
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
}

fn task_ids(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .filter_map(|row| {
            let line = row_text(screen, row);
            let at = line.find("#10")?;
            Some(line[at..].chars().take(5).collect())
        })
        .collect()
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

fn task_header(app: &crate::support::App) -> Point {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "Task")
        .into_iter()
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            let rest = line.get(usize::from(hit.x) + 4..).unwrap_or("");
            line.contains("Owner") && !rest.starts_with('s')
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected the Task header, got {hits:?}\n{}",
        text(&screen)
    );
    hits[0]
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn reveal(app: &crate::support::App, needles: &[&str]) {
    let point = Point { x: 70, y: 20 };
    for _ in 0..24 {
        let screen = app.settle();
        let visible = text(&screen);
        if needles.iter().all(|needle| visible.contains(needle)) {
            return;
        }
        app.wheel(point, true, 2);
    }
    for needle in needles {
        app.wait_text(needle);
    }
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 8 && hit.x >= 20)
}

fn wheel_until(app: &crate::support::App, needle: &str) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..48 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 3);
    }
    app.wait_text(needle);
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "basictable"];
    let app = open("s03-route-page", 120, 40, &args);
    let screen = app.wait_text("Add rate limiting");
    see(
        &screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Tasks",
            UNSORTED,
            "#1040",
            "Add rate limiting",
            "Reviewers",
            "2 selected · header mixed",
            "[✓] mira",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "sorted by",
            OVERVIEW,
        ],
    );
    let header = row_with(&screen, "Owner");
    assert!(
        header.contains("ID") && header.contains("Task") && !header.contains('['),
        "default single header changed: {header}"
    );
    shot(
        &app,
        "sc.basictable.route",
        "--page basictable opens the playground with the sidebar on Basic table",
        &args,
    );
    app.finish();

    let app = open("s03-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Add rate limiting");
    see(
        &entered,
        &[CRUMB, BLURB, "Tasks", UNSORTED],
        &[OVERVIEW, "Page options", "Inputs & forms › Fixed controls"],
    );
    app.key("[");
    let left = app.wait_text("Inputs & forms › Fixed controls");
    see(
        &left,
        &["Inputs & forms › Fixed controls"],
        &[CRUMB, "Add rate limiting"],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Tasks", UNSORTED],
        &["Inputs & forms › Fixed controls"],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Add rate limiting"]);
    click_side(&app, "Tables");
    app.wait_text("Basic table");
    click_side(&app, "Basic table");
    let pointed = app.wait_text("Add rate limiting");
    see(
        &pointed,
        &[CRUMB, BLURB, "Tasks", "#1040", UNSORTED],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "basictable"];
    let app = open("s03-sections", 80, 24, &args);
    let narrow_play = app.wait_text("Add rate limiting");
    see(
        &narrow_play,
        &[
            CRUMB,
            TITLE,
            BLURB_NARROW,
            "Playground",
            "Tasks",
            UNSORTED,
            "#1040",
            "Add rate limiting",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            BLURB,
            "sorted by",
        ],
    );
    shot(
        &app,
        "sc.basictable.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    let value = option_value(&narrow_opt, "Selection");
    assert!(
        value.contains("single") && !value.contains("multi") && !value.contains("none"),
        "narrow selection value: {value}"
    );
    see(
        &narrow_opt,
        &[CRUMB, TITLE, "Page options", "Selection", "single"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Tasks",
            UNSORTED,
            WIDE,
            "truecolor",
        ],
    );
    shot(
        &app,
        "sc.basictable.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    see(
        &narrow_states,
        &[
            CRUMB,
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "Single selection",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "overflow",
            "narrow",
            WIDE,
            "truecolor",
        ],
    );
    shot(
        &app,
        "sc.basictable.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    // The first frame after a resize can carry the wide caption before the
    // lower specimen rows are drawn. Wait until those rows are on screen.
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && text(screen).contains("selected")
            && text(screen).contains("disabled")
    });
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
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "selected",
            "disabled",
            "Single selection",
            "No selection",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "narrow",
            "256 colors",
        ],
    );
    assert!(
        !grid_label(&wide_states, "empty")
            && !grid_label(&wide_states, "overflow")
            && !grid_label(&wide_states, "narrow"),
        "lower specimen rows are on the top states screen\n{}",
        text(&wide_states)
    );
    shot(
        &app,
        "sc.basictable.states.120x40",
        "states caption and the grid rows at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.basictable.states",
        "states is the visible Basic table section",
        &args,
    );
    for (case_id, label) in TOP_STATES {
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES],
            &[
                "● Compare levels",
                "Page options",
                "busy",
                "error",
                "editing",
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

    reveal(&app, &["empty", "narrow", "No rows"]);
    let low = app.settle();
    assert!(
        grid_label(&low, "empty") && grid_label(&low, "overflow") && grid_label(&low, "narrow"),
        "a lower specimen row is off the grid\n{}",
        text(&low)
    );
    see(
        &low,
        &[CRUMB, "empty", "overflow", "narrow", "No rows", "States"],
        &[
            "● Compare levels",
            "Page options",
            "busy",
            "error",
            "editing",
            "loading",
            CAP_PLAY,
        ],
    );
    for (case_id, label) in LOW_STATES {
        assert!(
            text(&app.screen()).contains(label),
            "{label} left the screen before its capture\n{}",
            text(&app.screen())
        );
        let reason = format!("{label} row is visible and compare levels is off");
        shot(&app, case_id, &reason, &args);
    }

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(CAP_OPT);
    let value = option_value(&wide_opt, "Selection");
    assert!(
        value.contains("single") && !value.contains("multi") && !value.contains("none"),
        "wide selection value: {value}"
    );
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Selection", "single"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Tasks",
            UNSORTED,
            "multi",
            "none",
        ],
    );
    shot(
        &app,
        "sc.basictable.options.120x40",
        "options caption and the default selection at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.basictable.options",
        "options is the visible Basic table section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Tasks",
            UNSORTED,
            "#1040",
            "Add rate limiting",
            "Reviewers",
            "2 selected · header mixed",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "sorted by",
            "30 columns",
        ],
    );
    let header = row_with(&wide_play, "Owner");
    assert!(
        header.contains("Task") && !header.contains('['),
        "wide playground header: {header}"
    );
    shot(
        &app,
        "sc.basictable.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.basictable.playground",
        "playground is the visible Basic table section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "basictable"];
    let app = open("s03-compare", 120, 40, &args);
    click_tab(&app, "States");
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
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[CRUMB, "● Compare levels", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    reveal(&app, &["no color"]);
    let bands = app.settle();
    see(
        &bands,
        &[CRUMB, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.basictable.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(CRUMB)
    });
    if !text(&narrow).contains("no color") {
        reveal(&app, &["no color"]);
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    let visible = text(&narrow);
    assert!(
        visible.contains("no color")
            && !visible.contains(WIDE)
            && !visible.contains("○ Compare levels"),
        "narrow compare layout\n{visible}"
    );
    see(
        &narrow,
        &[CRUMB, "no color"],
        &["Page options", CAP_PLAY, WIDE, "120×40"],
    );
    shot(
        &app,
        "sc.basictable.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    wheel_until(&app, "● Compare levels");
    click_where(&app, "Compare levels", |hit| hit.y < 16);
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
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    let value = option_value(&options, "Selection");
    assert!(
        value.contains("single") && !value.contains("multi") && !value.contains("none"),
        "compare return changed selection: {value}"
    );
    see(
        &options,
        &["single", "Page options", CAP_OPT],
        &["multi", "none", "Compare levels", "keyboard-only"],
    );
    app.finish();
}

#[test]
fn sort_then_select_and_activate() {
    let args = ["--page", "basictable"];
    let app = open("s03-sort", 120, 40, &args);
    let screen = app.wait_text("#1040");
    assert_eq!(
        task_ids(&screen).first().map(String::as_str),
        Some("#1040"),
        "original order\n{}",
        text(&screen)
    );
    see(
        &screen,
        &[CRUMB, UNSORTED, "Add rate limiting", "Tasks"],
        &[SORT_ASC, SORT_DESC, "Task ▴", "Task ▾", ACTIVATED],
    );

    app.click(task_header(&app));
    let asc = app.wait_text(SORT_ASC);
    let ids = task_ids(&asc);
    assert_eq!(
        ids.first().map(String::as_str),
        Some("#1058"),
        "ascending ids {ids:?}\n{}",
        text(&asc)
    );
    assert!(
        ids.iter().any(|id| id == "#1040") && ids[0] != "#1040",
        "ascending still started at the original row: {ids:?}"
    );
    see(
        &asc,
        &[
            CRUMB,
            SORT_ASC,
            "Task ▴",
            "#1058",
            "Add dark mode to admin panel",
        ],
        &[
            UNSORTED,
            SORT_DESC,
            "Task ▾",
            ACTIVATED,
            "Write release notes",
        ],
    );
    shot(
        &app,
        "sc.basictable.sort.before",
        "task column sorted ascending before the next sort",
        &args,
    );

    app.click(task_header(&app));
    let desc = app.wait_text(SORT_DESC);
    let visible = text(&desc);
    let upgrade = visible.find("Upgrade Postgres").expect("descending window");
    let limited = visible
        .find("Add rate limiting")
        .expect("focused original row");
    assert!(
        upgrade < limited,
        "descending order did not put Upgrade Postgres before Add rate limiting\n{visible}"
    );
    see(
        &desc,
        &[
            CRUMB,
            SORT_DESC,
            "Task ▾",
            "#1045",
            "Upgrade Postgres driver to 0.9",
        ],
        &[
            UNSORTED,
            SORT_ASC,
            "Task ▴",
            "Add dark mode",
            "Write release notes",
            ACTIVATED,
        ],
    );

    app.click(task_header(&app));
    let cleared = app.wait_text(UNSORTED);
    assert_eq!(
        task_ids(&cleared).first().map(String::as_str),
        Some("#1040"),
        "cleared order\n{}",
        text(&cleared)
    );
    let header = row_with(&cleared, "Owner");
    assert!(
        header.contains("Task") && !header.contains('▴') && !header.contains('▾'),
        "cleared header still sorted: {header}"
    );
    see(
        &cleared,
        &[CRUMB, UNSORTED, "#1040", "Add rate limiting"],
        &[SORT_ASC, SORT_DESC, "sorted by", ACTIVATED],
    );

    click_where(&app, "Migrate sessions", |hit| hit.x > 30);
    let selected = app.settle();
    let row = row_with(&selected, "#1041");
    assert!(
        row.contains('›') && row.contains("Migrate sessions"),
        "single selection frame missing: {row}"
    );
    let other = row_with(&selected, "#1040");
    assert!(
        !other.contains('›'),
        "selection frame landed on the original row: {other}"
    );
    let status = footer(&selected);
    assert!(
        !status.contains("Selected "),
        "click activated the row: {status}"
    );
    see(
        &selected,
        &[UNSORTED, "#1041", "Migrate sessions"],
        &[SORT_ASC, SORT_DESC, ACTIVATED, "sorted by"],
    );

    app.key("enter");
    let activated = app.wait_text(ACTIVATED);
    let row = row_with(&activated, "#1041");
    assert!(
        row.contains('›') && row.contains("Migrate sessions"),
        "activation cleared the selection frame: {row}"
    );
    assert_eq!(
        task_ids(&activated).first().map(String::as_str),
        Some("#1040"),
        "activation reordered the table\n{}",
        text(&activated)
    );
    see(
        &activated,
        &[CRUMB, UNSORTED, ACTIVATED, "#1041"],
        &[SORT_ASC, SORT_DESC, "sorted by", "Page options"],
    );
    shot(
        &app,
        "sc.basictable.sort.after",
        "original order restored after descending, with activation separate from selection",
        &args,
    );
    app.finish();
}

#[test]
fn option_selection_frame() {
    let args = ["--page", "basictable"];
    let app = open("s03-selection", 120, 40, &args);
    let play = app.wait_text("#1042");
    let header = row_with(&play, "Owner");
    assert!(
        header.contains("ID") && !header.contains('['),
        "default frame already had a checkbox column: {header}"
    );
    click_where(&app, "#1042", |hit| hit.x > 30);
    let chosen = app.settle();
    let row = row_with(&chosen, "#1042");
    assert!(
        row.contains('›') && row.contains("Fix flaky"),
        "default single frame missing: {row}"
    );
    let status = footer(&chosen);
    assert!(
        !status.contains("Selected "),
        "single click activated the row: {status}"
    );
    see(
        &chosen,
        &[UNSORTED, "#1042", "›"],
        &["Page options", "[✓] #1042", ACTIVATED],
    );

    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    let value = option_value(&options, "Selection");
    assert!(
        value.contains("single") && !value.contains("multi") && !value.contains("none"),
        "default option value: {value}"
    );
    see(
        &options,
        &["single", "Selection", CAP_OPT],
        &["multi", "none", CAP_PLAY, "Tasks"],
    );
    let field = locate(&app, "single", |hit| hit.x >= 20);
    app.click(field);
    let opened = app.wait_text("multi");
    see(
        &opened,
        &["none", "single", "multi", "Selection"],
        &[CAP_PLAY, "Tasks"],
    );
    app.key("down");
    app.key("enter");
    let chosen = app.settle();
    let value = option_value(&chosen, "Selection");
    assert!(
        value.contains("multi") && !value.contains("single") && !value.contains("none"),
        "multi value: {value}"
    );
    see(&chosen, &["multi", "Page options"], &["none", CAP_PLAY]);

    click_tab(&app, "Playground");
    let multi = app.wait_text("#1042");
    let header = row_with(&multi, "Owner");
    assert!(
        header.contains('[') && header.contains("ID") && header.contains("Task"),
        "multi frame missing on the header: {header}"
    );
    click_where(&app, "#1042", |hit| hit.x > 30);
    let checked = app.settle();
    let row = row_with(&checked, "#1042");
    assert!(
        row.contains("[✓]") && row.contains("Fix flaky") && !row.contains('›'),
        "multi checkbox frame: {row}"
    );
    let status = footer(&checked);
    assert!(
        !status.contains("Selected "),
        "checking the box activated the row: {status}"
    );
    see(
        &checked,
        &[CRUMB, UNSORTED, "[✓]", "#1042", "Tasks"],
        &["Page options", ACTIVATED, SORT_ASC, "sorted by", "single"],
    );
    shot(
        &app,
        "opt.basictable.selection",
        "multi selection draws the checkbox frame and the single current mark stays off",
        &args,
    );
    app.finish();
}

#[test]
fn option_malformed_names_the_kind_of_data_error() {
    for (value, shows) in [
        (
            "short",
            "Rows rejected · row 0 has 1 cells, expected 2 (shape, row 0)",
        ),
        (
            "long",
            "Rows rejected · row 0 has 3 cells, expected 2 (shape, row 0)",
        ),
        (
            "no columns",
            "Rows rejected · rows supplied for a schema with no columns (empty schema)",
        ),
        (
            "missing row",
            "Rows rejected · row 2 does not exist (missing row 2)",
        ),
    ] {
        let opt = format!("basictable.malformed={value}");
        let args = ["--page", "basictable", "--opt", opt.as_str()];
        let app = open("s03-malformed", 120, 40, &args);
        app.wait_text("#1042");
        click_tab(&app, "Playground");
        for _ in 0..8 {
            if text(&app.screen()).contains("Rows rejected") {
                break;
            }
            app.key("pagedown");
            app.settle();
        }
        let screen = app.wait_text("Rows rejected");
        see(&screen, &[shows, "Checks"], &["No checks have run yet"]);
        if value == "missing row" {
            shot(
                &app,
                "opt.basictable.malformed",
                "the checks table shows why its rows were refused",
                &args,
            );
        }
        app.finish();
    }
}
