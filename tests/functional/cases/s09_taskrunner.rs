//! Task runner route, sections, specimens, compare levels, options, and pipeline.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s09_taskrunner/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Screens › Task runner";
const TITLE: &str = "Task runner";
const BLURB: &str = "Composed: tree, live progress, following log, busy states";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "taskrunner"];
const READY: &str = "Ready. Press r or Run to start the pipeline.";
const STARTED: &str = "Pipeline started";
const RUNNING_TITLE: &str = "Pipeline · running";
const DONE: &str = "0 of 6 done";
const CANCELLED_LOG: &str = "Pipeline cancelled by user";
const DIALOG_TITLE: &str = "Cancel pipeline?";
const CONFIRM: &str = "Cancel pipeline";
const KEEP: &str = "Keep running";
const WHEEL: Point = Point { x: 70, y: 18 };

const OMITTED: &[&str] = &["focus + hover", "editing", "loading"];

const DRAWN: &[(&str, &str)] = &[
    ("sc.taskrunner.specimen.rest", "rest"),
    ("sc.taskrunner.specimen.hover", "hover"),
    ("sc.taskrunner.specimen.pressed", "pressed"),
    ("sc.taskrunner.specimen.focused", "focused"),
    ("sc.taskrunner.specimen.selected", "selected"),
    ("sc.taskrunner.specimen.disabled", "disabled"),
    ("sc.taskrunner.specimen.error", "error"),
    ("sc.taskrunner.specimen.busy", "busy"),
    ("sc.taskrunner.specimen.empty", "empty"),
    ("sc.taskrunner.specimen.overflow", "overflow"),
    ("sc.taskrunner.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s09_taskrunner/{}", case_id.replace('.', "_"))
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

fn assert_task_options(
    screen: &TerminalSnapshot,
    expanded: &str,
    indent: &str,
    glyphs: &str,
    meta: &str,
    follow: &str,
    wrap: &str,
) {
    assert_toggle(screen, "Targets expanded", expanded);
    let others: &[&str] = match indent {
        "2" => &["3", "4"],
        "3" => &["2", "4"],
        "4" => &["2", "3"],
        _ => &[],
    };
    assert_select(screen, "Tree indent", indent, others);
    assert_toggle(screen, "Environment glyphs", glyphs);
    assert_toggle(screen, "Folder task counts", meta);
    assert_toggle(screen, "Log follows its tail", follow);
    assert_toggle(screen, "Log wraps", wrap);
}

/// Grid labels sit in the label column. At 80 columns that column starts at
/// x 21; at 120 it starts near x 28. The legend's "focused" is further right.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 7
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
    // The top nav row shares y with the section tabs. Those tabs start at x >= 24.
    hit.x < 24
}

fn click_side(app: &App, label: &str) {
    reveal_side(app, label);
    click_where(app, label, sidebar);
}

fn click_button(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 4 && hit.x >= 26);
}

/// Dialog actions share text with the title and the footer hint. The button is
/// the hit below the title question mark and above the footer.
fn click_dialog(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            hit.y > 8 && u32::from(hit.y) + 2 < screen.rows() && !line.contains('?') && hit.x >= 26
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one dialog {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn reveal_side(app: &App, label: &str) {
    // The sidebar keeps the selection in view and ignores a wheel that would
    // scroll the label out from under it, so move the selection until the
    // label is on screen, then the caller clicks it.
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
    for _ in 0..120 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, true, 3);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..120 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 3);
    }
    app.wait_text(needle);
}

fn reveal_label(app: &App, label: &str) {
    for _ in 0..40 {
        if grid_label(&app.settle(), label) {
            return;
        }
        app.wheel(WHEEL, true, 1);
    }
    assert!(
        grid_label(&app.screen(), label),
        "{label} never entered the states grid\n{}",
        text(&app.screen())
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

/// The 140ms press flash stays painted while motion is paused, so a shot after
/// a released click has to redraw once the deadline has passed. Below about 100
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

fn tree_hits(screen: &TerminalSnapshot, needle: &str) -> Vec<Point> {
    find_all(screen, needle)
        .into_iter()
        .filter(|hit| (26..56).contains(&hit.x) && hit.y > 6)
        .collect()
}

fn top_tree(screen: &TerminalSnapshot, needle: &str) -> Point {
    tree_hits(screen, needle)
        .into_iter()
        .min_by_key(|hit| hit.y)
        .unwrap_or_else(|| panic!("tree {needle:?} missing\n{}", text(screen)))
}

/// Depth-2 `compile` sits one indent step to the right of depth-1 `build`.
fn indent_gap(screen: &TerminalSnapshot) -> u16 {
    let build = top_tree(screen, "build");
    let compile = tree_hits(screen, "compile")
        .into_iter()
        .filter(|hit| hit.y > build.y)
        .min_by_key(|hit| hit.y)
        .unwrap_or_else(|| panic!("compile under build missing\n{}", text(screen)));
    assert!(
        compile.x > build.x,
        "compile {compile:?} is not right of build {build:?}"
    );
    compile.x - build.x
}

fn assert_expanded_tree(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            "payments-gateway",
            "build",
            "compile",
            "lint",
            "shared-libs",
            "staging",
            "production",
            "publish",
        ],
        &["Page options", "Compare levels"],
    );
    let production = row_with(screen, "production");
    assert!(
        production.contains('◆'),
        "production is missing its environment glyph: {production:?}"
    );
    let staging = row_with(screen, "staging");
    assert!(
        staging.contains('◇'),
        "staging is missing its environment glyph: {staging:?}"
    );
    assert_eq!(indent_gap(screen), 2, "default tree indent is not 2");
    let gateway = row_with(screen, "payments-gateway");
    assert!(
        !gateway.contains('3'),
        "folder counts are on while tree_meta is off: {gateway:?}"
    );
}

fn assert_idle(screen: &TerminalSnapshot) {
    assert_expanded_tree(screen);
    see(
        screen,
        &[
            READY,
            "Targets",
            "Log",
            "Pipeline",
            DONE,
            "queued",
            "Run pipeline",
            "Cancel",
        ],
        &[
            RUNNING_TITLE,
            STARTED,
            "Pipeline finished",
            "failed",
            "Pipeline cancelled",
            "compile started",
            DIALOG_TITLE,
            "cancelled",
        ],
    );
    let title = row_with(screen, DONE);
    assert!(
        title.contains("Pipeline") && !title.contains("running"),
        "idle pipeline title is {title:?}"
    );
}

fn assert_running_start(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            STARTED,
            RUNNING_TITLE,
            DONE,
            "queued",
            "Run pipeline",
            "Cancel",
            "Pipeline running",
        ],
        &[
            READY,
            "Pipeline finished",
            "failed",
            "Pipeline cancelled",
            "compile started",
            "lint started",
            "typecheck started",
            "unit started",
            "integration started",
            "e2e started",
            DIALOG_TITLE,
            KEEP,
            "cancelled",
            "Cancelled",
            "▶",
        ],
    );
    assert_eq!(
        text(screen).matches(STARTED).count(),
        1,
        "start wrote the log more than once\n{}",
        text(screen)
    );
}

fn assert_cancelled(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            STARTED,
            CANCELLED_LOG,
            DONE,
            "cancelled",
            "Cancelled",
            "Run pipeline",
        ],
        &[
            RUNNING_TITLE,
            READY,
            "Pipeline finished",
            "failed",
            "compile started",
            "queued",
            DIALOG_TITLE,
            "Pipeline running",
            "▶",
        ],
    );
    let title = row_with(screen, DONE);
    assert!(
        title.contains("Pipeline") && !title.contains("running"),
        "cancelled pipeline title is {title:?}"
    );
}

fn assert_dialog(screen: &TerminalSnapshot) {
    // The body wraps inside the frame, so the border sits between the two
    // lines and the whole sentence is not one run of text. The log line is
    // covered while the dialog is open.
    see(
        screen,
        &[
            DIALOG_TITLE,
            CONFIRM,
            KEEP,
            "Running tasks stop immediately. Finished tasks",
            "keep their results.",
            RUNNING_TITLE,
            "Pipeline running",
        ],
        &[
            CANCELLED_LOG,
            "Pipeline finished",
            "compile started",
            "Cancelled",
        ],
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s09-route-page", 120, 40, PAGE_ARGS);
    app.wait_text(CRUMB);
    flash_settle(&app);
    let screen = app.settle();
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
            "Options",
            "States",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            OVERVIEW,
        ],
    );
    assert_idle(&screen);
    shot(&app, "sc.taskrunner.route");
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("payments-gateway");
    see(
        &entered,
        &[CRUMB, BLURB, READY, "compile", "lint"],
        &[OVERVIEW, "Page options", "Screens › Settings"],
    );
    app.key("[");
    let left = app.wait_text("Screens › Settings");
    see(&left, &["Screens › Settings"], &[CRUMB, BLURB, READY]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "payments-gateway", READY],
        &["Screens › Settings", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, READY]);
    reveal_side(&app, TITLE);
    let screen = app.settle();
    if !find_all(&screen, TITLE)
        .into_iter()
        .any(|hit| sidebar(&hit))
    {
        click_side(&app, "Screens");
    }
    click_side(&app, TITLE);
    let pointed = app.wait_text("payments-gateway");
    see(
        &pointed,
        &[CRUMB, BLURB, READY, "shared-libs", "Run pipeline"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s09-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("payments-gateway");
    assert_size(&app, 80, 24);
    // At 80×24 the log body is below the last row, so the ready line is absent.
    see(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "payments-gateway",
            "Targets",
            "queued",
            "Run pipeline",
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
            RUNNING_TITLE,
            STARTED,
            READY,
        ],
    );
    shot(&app, "sc.taskrunner.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_task_options(&narrow_opt, "on", "2", "on", "off", "on", "off");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Targets expanded", "Log wraps"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            READY,
            "payments-gateway",
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.taskrunner.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert!(
        grid_label(&narrow_states, "rest"),
        "narrow states clipped rest\n{}",
        text(&narrow_states)
    );
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "Targets"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            READY,
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.taskrunner.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && text(screen).contains("Task bar")
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
            "rest",
            "Targets",
            "Log",
            "Task bar",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            READY,
            "256 colors",
        ],
    );
    assert_omitted(&wide_states);
    shot(&app, "sc.taskrunner.states.120x40");
    shot(&app, "sc.taskrunner.states");

    for (case_id, label) in DRAWN {
        reveal_label(&app, label);
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
    assert_task_options(&wide_opt, "on", "2", "on", "off", "on", "off");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Tree indent"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", READY],
    );
    shot(&app, "sc.taskrunner.options.120x40");
    shot(&app, "sc.taskrunner.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("payments-gateway");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[CRUMB, TITLE, BLURB, CAP_PLAY, WIDE, "Playground"],
        &[CAP_OPT, CAP_STATES, "Page options", "Compare levels"],
    );
    assert_idle(&wide_play);
    shot(&app, "sc.taskrunner.playground.120x40");
    shot(&app, "sc.taskrunner.playground");
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
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES, "Targets"],
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
    shot(&app, "sc.taskrunner.compare-levels.120x40");

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
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.taskrunner.compare-levels.80x24");

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
    assert_task_options(&options, "on", "2", "on", "off", "on", "off");
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "Targets expanded"],
        &["Compare levels", OVERVIEW, READY],
    );
    app.finish();
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("payments-gateway");
    flash_settle(app);
}

fn finish_playground(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

#[test]
fn option_expanded_off() {
    let app = page("s09-expanded", Some("taskrunner.expanded=off"), 120, 40);
    app.wait_text("payments-gateway");
    confirm_options(&app, |screen| {
        assert_task_options(screen, "off", "2", "on", "off", "on", "off");
    });
    finish_playground(&app, "opt.taskrunner.expanded", |screen| {
        // Roots stay open. Nested folders close, so their leaves disappear
        // and the shared-libs leaves stay.
        let gateway = row_with(screen, "payments-gateway");
        let build = row_with(screen, "build");
        let shared = row_with(screen, "shared-libs");
        assert!(
            gateway.contains('▾') && shared.contains('▾'),
            "a root closed: {gateway:?} {shared:?}"
        );
        assert!(
            build.contains('▸') && !build.contains('▾'),
            "build stayed open: {build:?}"
        );
        see(
            screen,
            &[
                "payments-gateway",
                "shared-libs",
                "publish",
                "build",
                "deploy",
                READY,
                DONE,
                "queued",
            ],
            &["staging", "production", "◆", "◇", "Page options", STARTED],
        );
    });
}

#[test]
fn option_indent_4() {
    let app = page("s09-indent", Some("taskrunner.indent=4"), 120, 40);
    app.wait_text("payments-gateway");
    confirm_options(&app, |screen| {
        assert_task_options(screen, "on", "4", "on", "off", "on", "off");
    });
    finish_playground(&app, "opt.taskrunner.indent", |screen| {
        assert_eq!(indent_gap(screen), 4, "indent 4 did not widen the tree");
        see(
            screen,
            &["payments-gateway", "compile", "lint", "staging", "◆", READY],
            &["Page options", STARTED],
        );
        let gateway = row_with(screen, "payments-gateway");
        assert!(
            !gateway.contains('3'),
            "indent changed folder counts: {gateway:?}"
        );
    });
}

#[test]
fn option_env_glyphs_off() {
    let app = page("s09-glyphs", Some("taskrunner.env_glyphs=off"), 120, 40);
    app.wait_text("production");
    confirm_options(&app, |screen| {
        assert_task_options(screen, "on", "2", "off", "off", "on", "off");
    });
    finish_playground(&app, "opt.taskrunner.env_glyphs", |screen| {
        assert_eq!(indent_gap(screen), 2, "glyphs changed the indent");
        let production = row_with(screen, "production");
        assert!(
            !production.contains('◆') && !production.contains('◇'),
            "environment glyph stayed on the production row: {production:?}"
        );
        let staging = row_with(screen, "staging");
        assert!(
            !staging.contains('◇') && !staging.contains('◆'),
            "environment glyph stayed on the staging row: {staging:?}"
        );
        see(
            screen,
            &["staging", "production", "publish", "compile", READY],
            &["Page options", STARTED],
        );
    });
}

#[test]
fn option_tree_meta_on() {
    let app = page("s09-meta", Some("taskrunner.tree_meta=on"), 120, 40);
    app.wait_text("payments-gateway");
    confirm_options(&app, |screen| {
        assert_task_options(screen, "on", "2", "on", "on", "on", "off");
    });
    finish_playground(&app, "opt.taskrunner.tree_meta", |screen| {
        assert_eq!(indent_gap(screen), 2, "folder counts changed the indent");
        let gateway = row_with(screen, "payments-gateway");
        assert!(
            gateway.contains('3'),
            "payments-gateway is missing its child count: {gateway:?}"
        );
        let shared = row_with(screen, "shared-libs");
        assert!(
            shared.contains('2'),
            "shared-libs is missing its child count: {shared:?}"
        );
        see(
            screen,
            &["build", "compile", "staging", "◆", READY],
            &["Page options", STARTED],
        );
    });
}

fn ready_log_unchanged(screen: &TerminalSnapshot) {
    let line = row_with(screen, "Ready. Press r");
    assert!(
        line.contains("start the pipeline."),
        "the one-line idle log changed: {line:?}"
    );
    see(
        screen,
        &[READY, "payments-gateway", DONE],
        &[STARTED, "Pipeline finished", "Page options"],
    );
}

#[test]
fn option_follow_off() {
    let app = page("s09-follow", Some("taskrunner.follow=off"), 120, 40);
    app.wait_text("payments-gateway");
    confirm_options(&app, |screen| {
        assert_task_options(screen, "on", "2", "on", "off", "off", "off");
    });
    ready_log_unchanged(&app.settle());
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let screen = app.settle();
    assert_task_options(&screen, "on", "2", "on", "off", "off", "off");
    assert_size(&app, 120, 40);
    see(
        &screen,
        &["Log follows its tail", "Page options", CRUMB],
        &[READY, "payments-gateway", STARTED, "Compare levels"],
    );
    shot(&app, "opt.taskrunner.follow");
    app.finish();
}

#[test]
fn option_wrap_on() {
    let app = page("s09-wrap", Some("taskrunner.wrap=on"), 120, 40);
    app.wait_text("payments-gateway");
    confirm_options(&app, |screen| {
        assert_task_options(screen, "on", "2", "on", "off", "on", "on");
    });
    ready_log_unchanged(&app.settle());
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let screen = app.settle();
    assert_task_options(&screen, "on", "2", "on", "off", "on", "on");
    assert_size(&app, 120, 40);
    see(
        &screen,
        &["Log wraps", "Page options", CRUMB],
        &[READY, "payments-gateway", STARTED, "Compare levels"],
    );
    shot(&app, "opt.taskrunner.wrap");
    app.finish();
}

#[test]
fn pipeline_run_cancel_and_restart() {
    let app = open("s09-pipeline", 120, 40, PAGE_ARGS);
    let idle = app.wait_text(READY);
    assert_size(&app, 120, 40);
    assert_idle(&idle);

    click_button(&app, "Cancel");
    flash_settle(&app);
    let ignored = app.settle();
    assert_idle(&ignored);

    click_button(&app, "Run pipeline");
    app.wait_text(STARTED);
    flash_settle(&app);
    let started = app.settle();
    assert_running_start(&started);

    click_button(&app, "Run pipeline");
    flash_settle(&app);
    let still = app.settle();
    assert_running_start(&still);
    shot(&app, "sc.taskrunner.pipeline.before");

    click_button(&app, "Cancel");
    app.wait_text(DIALOG_TITLE);
    flash_settle(&app);
    let dialog = app.settle();
    assert_dialog(&dialog);
    shot(&app, "sc.taskrunner.pipeline.dialog");

    click_dialog(&app, KEEP);
    app.wait_absent(DIALOG_TITLE);
    flash_settle(&app);
    let kept = app.settle();
    assert_running_start(&kept);

    click_button(&app, "Cancel");
    app.wait_text(DIALOG_TITLE);
    flash_settle(&app);
    click_dialog(&app, CONFIRM);
    app.wait_text(CANCELLED_LOG);
    flash_settle(&app);
    let cancelled = app.settle();
    assert_cancelled(&cancelled);
    shot(&app, "sc.taskrunner.pipeline.after");

    click_button(&app, "Run pipeline");
    app.wait_absent(CANCELLED_LOG);
    flash_settle(&app);
    let restarted = app.settle();
    assert_running_start(&restarted);

    click_button(&app, "Cancel");
    app.wait_text(DIALOG_TITLE);
    flash_settle(&app);
    click_dialog(&app, CONFIRM);
    app.wait_text(CANCELLED_LOG);
    flash_settle(&app);
    assert_cancelled(&app.settle());
    app.finish();
}
