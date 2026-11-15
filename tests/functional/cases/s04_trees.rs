//! Trees route, sections, specimens, compare levels, branches, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s04_trees/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Navigation › Trees";
const TITLE: &str = "Trees";
const BLURB: &str = "Indent carries hierarchy; the focus bar never moves";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "trees"];

const READOUT: &[(&str, &str)] = &[
    ("branch", "branch_activate Toggle"),
    ("click", "click Double"),
    ("indent", "indent 2"),
    ("divider", "divider false"),
    ("alt", "alt_click_recursive true"),
    ("reserve", "reserve_right 0"),
    ("meta", "show_meta true"),
    ("nav", "nav default"),
    ("wrap", "wrap false"),
    ("ahead", "type-ahead on"),
    ("track", "track_click Page"),
    ("marker", "selected_marker None"),
    ("expand", "* TreeCmd::ExpandAll"),
    ("collapse", "- TreeCmd::CollapseAll"),
];

const TOP_STATES: &[(&str, &str)] = &[
    ("sc.trees.specimen.rest", "rest"),
    ("sc.trees.specimen.hover", "hover"),
    ("sc.trees.specimen.pressed", "pressed"),
    ("sc.trees.specimen.focused", "focused"),
    ("sc.trees.specimen.focused_hover", "focus + hover"),
    ("sc.trees.specimen.selected", "selected"),
    ("sc.trees.specimen.busy", "busy"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s04_trees/{}", case_id.replace('.', "_"))
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

fn in_project(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| (24..72).contains(&hit.x) && (7..26).contains(&hit.y))
}

fn assert_project(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    for label in present {
        assert!(
            in_project(screen, label),
            "project tree missing {label:?}\n{}",
            text(screen)
        );
    }
    for label in absent {
        assert!(
            !in_project(screen, label),
            "project tree still shows {label:?}\n{}",
            text(screen)
        );
    }
}

fn cursor_value(screen: &TerminalSnapshot) -> String {
    let line = row_with(screen, "cursor");
    let at = line.find("cursor").expect("cursor column");
    line[at + "cursor".len()..]
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

/// The tree column only. A full row also holds the sidebar and the Selection panel.
fn project_slice(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = find_all(screen, label)
        .into_iter()
        .filter(|hit| (24..72).contains(&hit.x) && (7..26).contains(&hit.y))
        .min_by_key(|hit| hit.x)
        .unwrap_or_else(|| panic!("project row {label:?} missing\n{}", text(screen)));
    let chars: Vec<char> = row_text(screen, u32::from(hit.y)).chars().collect();
    let start = usize::from(hit.x.saturating_sub(4));
    let end = (usize::from(hit.x) + label.chars().count()).min(chars.len());
    chars.get(start..end).unwrap_or_default().iter().collect()
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

fn assert_readout(screen: &TerminalSnapshot, skip: &[&str], present: &str, absent: &str) {
    let visible = flat(screen);
    assert!(
        visible.contains(present),
        "readout missing {present:?}\n{}",
        text(screen)
    );
    assert!(
        !visible.contains(absent),
        "readout still shows {absent:?}\n{}",
        text(screen)
    );
    for (key, fragment) in READOUT {
        if skip.contains(key) {
            continue;
        }
        assert!(
            visible.contains(fragment),
            "another option changed; missing {fragment:?}\n{}",
            text(screen)
        );
    }
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 8 && (20..40).contains(&hit.x))
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

fn click_project(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| (24..72).contains(&hit.x) && (7..26).contains(&hit.y))
        .collect();
    assert!(
        !hits.is_empty(),
        "project row {label:?} missing\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.x);
    app.click(hits[0]);
}

fn label_x(screen: &TerminalSnapshot, label: &str) -> u16 {
    find_all(screen, label)
        .into_iter()
        .filter(|hit| (24..72).contains(&hit.x) && (7..26).contains(&hit.y))
        .map(|hit| hit.x)
        .min()
        .unwrap_or_else(|| panic!("no x for {label:?}\n{}", text(screen)))
}

fn reveal_grid(app: &App, label: &str) {
    let point = Point { x: 60, y: 18 };
    for _ in 0..40 {
        if grid_label(&app.settle(), label) {
            return;
        }
        app.wheel(point, true, 1);
    }
    panic!("grid row {label} did not appear\n{}", text(&app.screen()));
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

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

/// Top-level branches start open. Nested branches stay closed, and README.md is below the card.
fn initial_tree(screen: &TerminalSnapshot) {
    assert_project(
        screen,
        &[
            "src",
            "api",
            "db",
            "workers",
            "config.rs",
            "lib.rs",
            "main.rs",
            "tests",
            "checkout.rs",
            "auth_flow.rs",
            "fixtures",
            "docs",
            "architecture.md",
            "webhooks.md",
            "Cargo.toml",
        ],
        &[
            "auth.rs",
            "billing.rs",
            "dispatch.rs",
            "migrations.rs",
            "scheduler.rs",
            "users.json",
            "README.md",
        ],
    );
    see(
        screen,
        &[
            "16 rows",
            "5 folders",
            "Nothing selected",
            "last TreeAction: —",
            "No connections",
        ],
        &["Activate(", "Expand(", "0 folders", "20 rows"],
    );
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

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s04-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    see(
        &screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "Project",
            "Selection",
            "Connections",
            "production",
            "orders-db",
            "postgres",
            "replica-eu",
            "lag 4.2 s",
            "staging",
            "archive",
            "branch_activate Toggle",
            "click Double",
            "indent 2",
            "show_meta true",
            "1.4 KB",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "branch_activate Activate",
            "show_meta false",
        ],
    );
    initial_tree(&screen);
    shot(&app, "sc.trees.route");
    app.finish();

    let app = open("s04-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Cargo.toml");
    see(
        &entered,
        &[CRUMB, BLURB, "Project", "16 rows", "api", "checkout.rs"],
        &[OVERVIEW, "Page options", "Navigation › Lists"],
    );
    initial_tree(&entered);
    app.key("[");
    let left = app.wait_text("Navigation › Lists");
    see(&left, &["Navigation › Lists"], &[CRUMB, "Cargo.toml"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Project", "Cargo.toml"],
        &["Navigation › Lists"],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Cargo.toml", BLURB]);
    click_side(&app, "Navigation");
    app.wait_text("Lists");
    click_side(&app, "Trees");
    let pointed = app.wait_text("Cargo.toml");
    see(
        &pointed,
        &[
            CRUMB,
            BLURB,
            "Project",
            "16 rows",
            "orders-db",
            "No connections",
        ],
        &[OVERVIEW, "Page options", "Compare levels", "auth.rs"],
    );
    initial_tree(&pointed);
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s04-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("checkout.rs");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            BLURB,
            "Playground",
            "Project",
            "src",
            "api",
            "tests",
            "checkout.rs",
            "docs",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "auth.rs",
            "Cargo.toml",
            "README.md",
            "architecture.md",
        ],
    );
    assert_project(
        &narrow_play,
        &[
            "src",
            "api",
            "db",
            "workers",
            "config.rs",
            "tests",
            "checkout.rs",
            "docs",
        ],
        &["auth.rs", "Cargo.toml", "architecture.md", "users.json"],
    );
    shot(&app, "sc.trees.playground.80x24");

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    assert_select(
        &narrow_opt,
        "Enter on a branch",
        "toggle",
        &["activate", "both"],
    );
    assert_select(&narrow_opt, "Click", "double", &["single"]);
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Enter on a branch",
            "toggle",
            "Indent",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Project",
            "Cargo.toml",
            WIDE,
            "truecolor",
            "activate",
            "both",
        ],
    );
    shot(&app, "sc.trees.options.80x24");

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    assert!(
        grid_label(&narrow_states, "rest")
            && grid_label(&narrow_states, "hover")
            && grid_label(&narrow_states, "pressed"),
        "narrow states clipped a top row\n{}",
        text(&narrow_states)
    );
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "Files",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "selected",
            "busy",
            "loading",
            "error",
            "overflow",
            "narrow",
            "disabled",
            "editing",
            "empty",
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    shot(&app, "sc.trees.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "selected")
            && grid_label(screen, "busy")
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
            "Files",
            "Connections",
            "main.rs",
            "lib.rs",
            "production",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "disabled",
            "editing",
            "empty",
            "Could not load",
            "256 colors",
        ],
    );
    for (case_id, label) in TOP_STATES {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the top states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.trees.states.120x40");
    shot(&app, "sc.trees.states");

    reveal_grid(&app, "narrow");
    let low = app.settle();
    for label in ["loading", "error", "overflow", "narrow"] {
        assert!(
            grid_label(&low, label),
            "{label} is not on the scrolled states grid\n{}",
            text(&low)
        );
    }
    see(
        &low,
        &[
            "loading",
            "error",
            "overflow",
            "narrow",
            "Could not load",
            "and a label",
        ],
        &[
            "● Compare levels",
            "Page options",
            "disabled",
            "editing",
            "empty",
        ],
    );
    for (case_id, label) in [
        ("sc.trees.specimen.loading", "loading"),
        ("sc.trees.specimen.error", "error"),
        ("sc.trees.specimen.overflow", "overflow"),
        ("sc.trees.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left before its capture\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(CAP_OPT);
    assert_select(
        &wide_opt,
        "Enter on a branch",
        "toggle",
        &["activate", "both"],
    );
    assert_select(&wide_opt, "Click", "double", &["single"]);
    assert_select(&wide_opt, "Indent", "2", &["1", "4"]);
    assert_toggle(&wide_opt, "Divider", "off");
    assert_toggle(&wide_opt, "Alt-click recursive", "on");
    assert_select(&wide_opt, "Reserve right", "0", &["10"]);
    assert_toggle(&wide_opt, "Show meta", "on");
    assert_select(&wide_opt, "Track click", "page", &["jump"]);
    assert_select(&wide_opt, "Nav keys", "default", &["vim"]);
    assert_toggle(&wide_opt, "Arrows wrap", "off");
    assert_toggle(&wide_opt, "Type-ahead", "on");
    assert_select(&wide_opt, "Key map", "default", &["custom"]);
    assert_select(&wide_opt, "Selected marker", "none", &["chevron"]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Project",
            "Cargo.toml",
        ],
    );
    shot(&app, "sc.trees.options.120x40");
    shot(&app, "sc.trees.options");

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
            "Project",
            "Connections",
            "orders-db",
            "postgres",
            "branch_activate Toggle",
            "1.4 KB",
            "16 rows",
            "5 folders",
            "api",
            "checkout.rs",
            "No connections",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "auth.rs",
            "Activate(",
            "README.md",
        ],
    );
    initial_tree(&wide_play);
    shot(&app, "sc.trees.playground.120x40");
    shot(&app, "sc.trees.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s04-compare", 120, 40, PAGE_ARGS);
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
    shot(&app, "sc.trees.compare-levels.120x40");

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
    shot(&app, "sc.trees.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    click_where(&app, "Compare levels", |hit| hit.y < 16);
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
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert_select(
        &options,
        "Enter on a branch",
        "toggle",
        &["activate", "both"],
    );
    assert_select(&options, "Click", "double", &["single"]);
    see(
        &options,
        &["toggle", "Page options", CAP_OPT, CRUMB],
        &["activate", "both", "Compare levels", OVERVIEW],
    );
    app.finish();
}

#[test]
fn expand_then_collapse_api_branch() {
    let app = open("s04-branches", 120, 40, PAGE_ARGS);
    let screen = app.wait_text("Cargo.toml");
    initial_tree(&screen);

    click_project(&app, "api");
    app.key("right");
    let opened = app.wait_text("auth.rs");
    assert_project(
        &opened,
        &[
            "src",
            "api",
            "auth.rs",
            "billing.rs",
            "webhooks",
            "db",
            "config.rs",
            "checkout.rs",
        ],
        &["dispatch.rs", "migrations.rs", "users.json", "scheduler.rs"],
    );
    see(
        &opened,
        &["20 rows", "6 folders", "auth.rs", "checkout.rs"],
        &["16 rows", "5 folders", "dispatch.rs", "migrations.rs"],
    );
    shot(&app, "sc.trees.branches.before");

    click_project(&app, "api");
    app.key("left");
    let folded = app.wait_screen("api collapsed", |screen| !in_project(screen, "auth.rs"));
    assert_project(
        &folded,
        &[
            "src",
            "api",
            "db",
            "config.rs",
            "tests",
            "checkout.rs",
            "docs",
            "architecture.md",
            "Cargo.toml",
        ],
        &[
            "auth.rs",
            "billing.rs",
            "dispatch.rs",
            "migrations.rs",
            "users.json",
        ],
    );
    see(
        &folded,
        &[
            "16 rows",
            "5 folders",
            "checkout.rs",
            "architecture.md",
            "config.rs",
        ],
        &["20 rows", "6 folders", "auth.rs", "migrations.rs"],
    );
    shot(&app, "sc.trees.branches.after");
    app.finish();
}

fn option_screen(name: &str, opt: &str) -> App {
    let app = page(name, Some(opt), 120, 40);
    app.wait_text("Project");
    app
}

fn confirm_control(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("Project");
}

#[test]
fn option_branch_activate() {
    let app = option_screen("s04-branch", "trees.branch=activate");
    let screen = app.screen();
    assert_readout(
        &screen,
        &["branch"],
        "branch_activate Activate",
        "branch_activate Toggle",
    );
    click_project(&app, "src");
    app.key("enter");
    let activated = app.wait_text("Activate(src)");
    assert_project(
        &activated,
        &["src", "api", "config.rs", "tests", "checkout.rs", "docs"],
        &["auth.rs", "migrations.rs", "users.json"],
    );
    see(
        &activated,
        &[
            "Activate(src)",
            "Opened src",
            "16 rows",
            "5 folders",
            "depth 0",
            "api",
        ],
        &["Expand(", "Nothing selected", "auth.rs", "20 rows"],
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Enter on a branch", "activate", &["toggle", "both"]);
        assert_select(screen, "Click", "double", &["single"]);
    });
    shot(&app, "opt.trees.branch");
    app.finish();
}

#[test]
fn option_branch_both() {
    let app = option_screen("s04-both", "trees.branch=both");
    assert_readout(
        &app.screen(),
        &["branch"],
        "branch_activate Both",
        "branch_activate Toggle",
    );
    let screen = app.wait_text("orders-db");
    let lazy = find_all(&screen, "orders-db")
        .into_iter()
        .filter(|hit| hit.x >= 72)
        .min_by_key(|hit| hit.y)
        .unwrap_or_else(|| panic!("no lazy connection row\n{}", text(&screen)));
    app.click(lazy);
    let focused = app.wait_text("Expand, then open");
    see(
        &focused,
        &["Enter Expand, then open", "last TreeAction: —"],
        &["Opened orders-db", "Activate(", "Enter Open"],
    );
    app.key("enter");
    let both = app.wait_text("Expand(orders-db) → Activate(orders-db)");
    see(
        &both,
        &[
            "last TreeAction: Expand(orders-db) → Activate(orders-db)",
            "Opened orders-db",
        ],
        &["last TreeAction: —"],
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Enter on a branch", "both", &["toggle", "activate"]);
    });
    app.finish();
}

#[test]
fn option_click_single() {
    let app = option_screen("s04-click", "trees.click=single");
    assert_readout(&app.screen(), &["click"], "click Single", "click Double");
    click_project(&app, "Cargo.toml");
    let activated = app.wait_text("Activate(Cargo.toml)");
    assert_eq!(cursor_value(&activated), "Cargo.toml");
    see(
        &activated,
        &[
            "Activate(Cargo.toml)",
            "Opened Cargo.toml",
            "16 rows",
            "5 folders",
        ],
        &["last TreeAction: —", "Expand(", "auth.rs", "20 rows"],
    );
    assert_project(
        &activated,
        &["api", "checkout.rs", "Cargo.toml"],
        &["auth.rs", "migrations.rs"],
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Click", "single", &["double"]);
        assert_select(screen, "Enter on a branch", "toggle", &["activate", "both"]);
    });
    shot(&app, "opt.trees.click");
    app.finish();
}

#[test]
fn option_indent_four() {
    let app = option_screen("s04-indent", "trees.indent=4");
    assert_readout(&app.screen(), &["indent"], "indent 4", "indent 2");
    see(&app.screen(), &["indent 4"], &["indent 1", "indent 2"]);
    let opened = app.wait_text("api");
    let src = label_x(&opened, "src");
    let api = label_x(&opened, "api");
    assert_eq!(api.saturating_sub(src), 4, "indent 4 did not move api");
    assert_project(
        &opened,
        &["api", "db", "workers", "config.rs", "tests", "checkout.rs"],
        &["auth.rs", "migrations.rs"],
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Indent", "4", &["1", "2"]);
        assert_select(screen, "Click", "double", &["single"]);
    });
    shot(&app, "opt.trees.indent");
    app.finish();
}

#[test]
fn option_divider_on() {
    let app = option_screen("s04-divider", "trees.divider=on");
    assert_readout(&app.screen(), &["divider"], "divider true", "divider false");
    initial_tree(&app.screen());
    confirm_control(&app, |screen| {
        assert_toggle(screen, "Divider", "on");
        assert_select(screen, "Enter on a branch", "toggle", &["activate", "both"]);
    });
    let screen = app.screen();
    assert!(
        text(&screen).contains("divider true") && !text(&screen).contains("divider false"),
        "divider readout left the playground\n{}",
        text(&screen)
    );
    shot(&app, "opt.trees.divider");
    app.finish();
}

#[test]
fn option_alt_click_off() {
    let app = option_screen("s04-alt", "trees.alt_click=off");
    assert_readout(
        &app.screen(),
        &["alt"],
        "alt_click_recursive false",
        "alt_click_recursive true",
    );
    initial_tree(&app.screen());
    confirm_control(&app, |screen| {
        assert_toggle(screen, "Alt-click recursive", "off");
        assert_toggle(screen, "Divider", "off");
    });
    shot(&app, "opt.trees.alt_click");
    app.finish();
}

#[test]
fn option_reserve_right_ten() {
    let app = option_screen("s04-reserve", "trees.reserve_right=10");
    assert_readout(
        &app.screen(),
        &["reserve"],
        "reserve_right 10",
        "reserve_right 0",
    );
    initial_tree(&app.screen());
    confirm_control(&app, |screen| {
        assert_select(screen, "Reserve right", "10", &["0"]);
        assert_select(screen, "Indent", "2", &["1", "4"]);
    });
    shot(&app, "opt.trees.reserve_right");
    app.finish();
}

#[test]
fn option_show_meta_off() {
    let app = option_screen("s04-meta", "trees.show_meta=off");
    let screen = app.wait_text("orders-db");
    assert_readout(&screen, &["meta"], "show_meta false", "show_meta true");
    see(
        &screen,
        &[
            "Cargo.toml",
            "config.rs",
            "checkout.rs",
            "orders-db",
            "replica-eu",
            "production",
            "No connections",
        ],
        &[
            "1.4 KB",
            "1.9 KB",
            "3.5 KB",
            "640 B",
            "postgres",
            "lag 4.2 s",
            "show_meta true",
        ],
    );
    assert_project(
        &screen,
        &[
            "src",
            "api",
            "config.rs",
            "checkout.rs",
            "docs",
            "Cargo.toml",
        ],
        &["auth.rs", "README.md", "migrations.rs"],
    );
    confirm_control(&app, |screen| {
        assert_toggle(screen, "Show meta", "off");
        assert_toggle(screen, "Alt-click recursive", "on");
    });
    shot(&app, "opt.trees.show_meta");
    app.finish();
}

#[test]
fn option_track_click_jump() {
    let app = option_screen("s04-track", "trees.track_click=jump");
    assert_readout(
        &app.screen(),
        &["track"],
        "track_click Jump",
        "track_click Page",
    );
    initial_tree(&app.screen());
    confirm_control(&app, |screen| {
        assert_select(screen, "Track click", "jump", &["page"]);
        assert_select(screen, "Nav keys", "default", &["vim"]);
    });
    shot(&app, "opt.trees.track_click");
    app.finish();
}

#[test]
fn option_nav_vim() {
    let app = option_screen("s04-nav", "trees.nav=vim");
    assert_readout(&app.screen(), &["nav"], "nav vim", "nav default");
    initial_tree(&app.screen());
    confirm_control(&app, |screen| {
        assert_select(screen, "Nav keys", "vim", &["default"]);
        assert_toggle(screen, "Type-ahead", "on");
    });
    shot(&app, "opt.trees.nav");
    app.finish();
}

#[test]
fn option_wrap_on() {
    let app = option_screen("s04-wrap", "trees.wrap=on");
    assert_readout(&app.screen(), &["wrap"], "wrap true", "wrap false");
    click_project(&app, "src");
    app.key("up");
    let wrapped = app.wait_screen("cursor wrapped", |screen| {
        cursor_value(screen) == "README.md"
    });
    assert_eq!(cursor_value(&wrapped), "README.md");
    assert_project(
        &wrapped,
        &["api", "checkout.rs", "Cargo.toml", "README.md"],
        &["auth.rs", "migrations.rs"],
    );
    see(
        &wrapped,
        &["wrap true", "16 rows", "5 folders", "README.md"],
        &["wrap false", "20 rows", "6 folders", "auth.rs"],
    );
    confirm_control(&app, |screen| {
        assert_toggle(screen, "Arrows wrap", "on");
        assert_select(screen, "Nav keys", "default", &["vim"]);
    });
    shot(&app, "opt.trees.wrap");
    app.finish();
}

#[test]
fn option_type_ahead_off() {
    let app = option_screen("s04-ahead", "trees.type_ahead=off");
    assert_readout(&app.screen(), &["ahead"], "type-ahead off", "type-ahead on");
    click_project(&app, "src");
    app.key("r");
    let stayed = app.settle();
    assert_eq!(
        cursor_value(&stayed),
        "src",
        "type-ahead off still moved the cursor\n{}",
        text(&stayed)
    );
    assert_project(
        &stayed,
        &["src", "api", "tests", "checkout.rs"],
        &["auth.rs", "README.md"],
    );
    see(
        &stayed,
        &["type-ahead off", "16 rows", "5 folders", "api"],
        &["type-ahead on", "README.md", "auth.rs"],
    );
    confirm_control(&app, |screen| {
        assert_toggle(screen, "Type-ahead", "off");
        assert_toggle(screen, "Arrows wrap", "off");
    });
    shot(&app, "opt.trees.type_ahead");
    app.finish();
}

#[test]
fn option_keymap_custom() {
    let app = option_screen("s04-keymap", "trees.keymap=custom");
    let screen = app.screen();
    let visible = flat(&screen);
    assert!(
        visible.contains("e TreeCmd::ExpandAll") && visible.contains("c TreeCmd::CollapseAll"),
        "custom keys missing\n{}",
        text(&screen)
    );
    assert_readout(
        &screen,
        &["expand", "collapse"],
        "e TreeCmd::ExpandAll",
        "* TreeCmd::ExpandAll",
    );
    see(
        &screen,
        &["c TreeCmd::CollapseAll"],
        &["- TreeCmd::CollapseAll"],
    );
    click_project(&app, "src");
    app.key("e");
    let expanded = app.wait_text("dispatch.rs");
    assert_project(
        &expanded,
        &["api", "auth.rs", "dispatch.rs", "webhooks", "db"],
        &[],
    );
    see(
        &expanded,
        &[
            "dispatch.rs",
            "auth.rs",
            "10 folders",
            "migrations.rs",
            "e TreeCmd::ExpandAll",
        ],
        &[
            "* TreeCmd::ExpandAll",
            "- TreeCmd::CollapseAll",
            "5 folders",
            "16 rows",
        ],
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Key map", "custom", &["default"]);
        assert_select(screen, "Selected marker", "none", &["chevron"]);
    });
    shot(&app, "opt.trees.keymap");
    app.finish();
}

#[test]
fn option_marker_chevron() {
    let app = option_screen("s04-marker", "trees.marker=chevron");
    assert_readout(
        &app.screen(),
        &["marker"],
        "selected_marker Some(\"›\")",
        "selected_marker None",
    );
    click_project(&app, "Cargo.toml");
    app.key("enter");
    let marked = app.wait_text("Activate(Cargo.toml)");
    let row = project_slice(&marked, "Cargo.toml");
    assert!(
        row.contains('›') && row.contains("Cargo.toml"),
        "chevron missing on the selected project row: {row}"
    );
    let other = project_slice(&marked, "config.rs");
    assert!(
        !other.contains('›'),
        "chevron appeared on the unselected config.rs row: {other}"
    );
    assert_project(
        &marked,
        &["api", "config.rs", "Cargo.toml"],
        &["auth.rs", "migrations.rs"],
    );
    see(
        &marked,
        &[
            "Activate(Cargo.toml)",
            "selected_marker Some(\"›\")",
            "16 rows",
            "5 folders",
        ],
        &["selected_marker None", "auth.rs", "20 rows"],
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Selected marker", "chevron", &["none"]);
        assert_select(screen, "Key map", "default", &["custom"]);
    });
    shot(&app, "opt.trees.marker");
    app.finish();
}

#[test]
fn option_filter_keeps_ancestor_of_matching_leaf() {
    let app = option_screen("s04-filter", "trees.filter=orders");
    let filtered = app.wait_text("orders-db");
    see(
        &filtered,
        &["production", "staging", "orders-db"],
        &["billing-db", "replica-eu", "archive"],
    );
    assert_eq!(
        find_all(&filtered, "orders-db").len(),
        2,
        "both orders-db leaves match the filter\n{}",
        text(&filtered)
    );
    confirm_control(&app, |screen| {
        assert_select(screen, "Filter connections", "orders", &["off"]);
    });
    shot(&app, "opt.trees.filter");
    app.finish();
}

fn keyed_app(name: &str, opts: &[&str]) -> App {
    let mut args = PAGE_ARGS.to_vec();
    args.extend(["--opt", "trees.source=keyed"]);
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    let app = open(name, 120, 48, &args);
    app.wait_text("Project source keyed (TreeSource: KeyedTree");
    app
}

#[test]
fn option_source_keyed_reports_actions_by_key() {
    let app = keyed_app("s04-keyed", &[]);
    let play = app.settle();
    see(
        &play,
        &[
            "KeyedTree, 31 nodes, empty false",
            "select_follows_cursor false · show_selection true",
            "Nothing selected",
        ],
        &["edit:", "jump:"],
    );
    assert_project(
        &play,
        &["src", "api", "config.rs", "generated", "tests", "docs"],
        &["auth.rs", "migrations.rs"],
    );

    click_project(&app, "config.rs");
    app.key("enter");
    let opened = app.wait_text("Activate(config.rs)");
    see(
        &opened,
        &[
            "last TreeAction: Activate(config.rs)",
            "by key · 1 selected",
        ],
        &["Nothing selected"],
    );

    click_project(&app, "api");
    app.key("right");
    let folded = app.settle();
    assert_project(
        &folded,
        &[
            "src",
            "api",
            "auth.rs",
            "billing.rs",
            "webhooks",
            "config.rs",
        ],
        &[],
    );
    assert_project(&folded, &[], &["dispatch.rs"]);

    click_project(&app, "generated");
    app.key("right");
    let loaded = app.wait_text("Expand(generated)");
    see(&loaded, &["last TreeAction: Expand(generated)"], &[]);
    assert_project(&loaded, &["generated", "client.rs", "schema.rs"], &[]);
    see(&loaded, &["client.rs 7.4 KB", "schema.rs 21 KB"], &[]);
    shot(&app, "opt.trees.source_keyed");
    app.finish();
}

#[test]
fn option_edit_keeps_the_view_when_the_keyed_store_changes() {
    let inserted = keyed_app("s04-insert", &["trees.edit=insert before"]);
    let screen = inserted.settle();
    see(
        &screen,
        &["edit: insert(src, 0, new.rs) → Ok", "just added"],
        &[],
    );
    let rows: Vec<String> = (0..screen.rows())
        .map(|row| row_text(&screen, row))
        .collect();
    let new_at = rows
        .iter()
        .position(|line| line.contains("new.rs") && line.contains("just added"));
    let api_at = rows.iter().position(|line| line.contains("▸ api"));
    assert!(
        new_at.is_some() && new_at < api_at,
        "the earlier sibling lands before api\n{}",
        text(&screen)
    );
    inserted.finish();

    let renamed = keyed_app("s04-rename", &["trees.edit=rename"]);
    let screen = renamed.settle();
    see(&screen, &["edit: update(main.rs) → true", "renamed"], &[]);
    assert_project(&screen, &["app.rs", "lib.rs"], &["main.rs"]);
    renamed.finish();

    let removed = keyed_app("s04-remove", &["trees.edit=remove"]);
    let screen = removed.settle();
    see(&screen, &["edit: remove(tests/fixtures) → true"], &[]);
    assert_project(
        &screen,
        &["tests", "checkout.rs", "auth_flow.rs"],
        &["fixtures"],
    );
    shot(&removed, "opt.trees.edit_remove");
    removed.finish();
}

#[test]
fn option_jump_moves_the_keyed_cursor_and_selection_by_key() {
    let reached = keyed_app("s04-jump", &["trees.jump=tests/auth_flow.rs"]);
    let screen = reached.settle();
    see(
        &screen,
        &[
            "jump: set_cursor_source(tests/auth_flow.rs) → true · select_source → true",
            "by key · 1 selected",
        ],
        &["Nothing selected"],
    );
    see(&screen, &["cursor auth_flow.rs"], &[]);
    reached.finish();

    let refused = keyed_app("s04-refused", &["trees.jump=src/api/webhooks/retry.rs"]);
    let screen = refused.settle();
    see(
        &screen,
        &[
            "jump: set_cursor_source(src/api/webhooks/retry.rs) → false (closed folder) · select_source → true",
            "retry.rs",
        ],
        &["Nothing selected"],
    );
    see(&screen, &["cursor src"], &["cursor retry.rs"]);
    refused.finish();

    let all = keyed_app("s04-expand-all", &["trees.jump=expand all"]);
    let screen = all.settle();
    see(
        &screen,
        &["jump: expand_all_source → every folder is open"],
        &[],
    );
    assert_project(
        &screen,
        &["api", "auth.rs", "billing.rs", "webhooks", "dispatch.rs"],
        &[],
    );
    shot(&all, "opt.trees.jump_expand_all");
    all.finish();
}

#[test]
fn option_follow_selects_the_leaf_under_the_cursor() {
    let app = keyed_app(
        "s04-follow",
        &["trees.follow=on", "trees.show_selection=off"],
    );
    let play = app.settle();
    see(
        &play,
        &[
            "select_follows_cursor true · show_selection false",
            "Nothing selected",
        ],
        &[],
    );
    click_project(&app, "config.rs");
    let followed = app.settle();
    see(
        &followed,
        &["by key · 1 selected"],
        &["Nothing selected", "Activate("],
    );
    see(&followed, &["cursor config.rs", "config.rs"], &[]);
    shot(&app, "opt.trees.follow");
    app.finish();
}
