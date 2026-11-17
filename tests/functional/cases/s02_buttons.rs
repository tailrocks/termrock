//! Buttons route, sections, specimens, compare levels, press, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s02_buttons/` and each case id's dots become underscores.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::bins::showcase;
use crate::support::{Launch, Point, find_all, row_text, text, try_launch};

const TITLE: &str = "Inputs & forms › Buttons";
const BLURB: &str = "Primary, secondary, subtle, danger, toggle, disabled, busy";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const FITS: &str = "full width · every button fits";
const NEXT: &str = "2 not available yet";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";

const SHOWN: &[(&str, &str)] = &[
    ("sc.buttons.specimen.rest", "rest"),
    ("sc.buttons.specimen.hover", "hover"),
    ("sc.buttons.specimen.pressed", "pressed"),
    ("sc.buttons.specimen.focused", "focused"),
    ("sc.buttons.specimen.focused_hover", "focus + hover"),
    ("sc.buttons.specimen.selected", "selected"),
    ("sc.buttons.specimen.disabled", "disabled"),
    ("sc.buttons.specimen.busy", "busy"),
    ("sc.buttons.specimen.overflow", "overflow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s02_buttons/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
    let mut launch = Launch::new(name, showcase());
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

fn line_after(screen: &TerminalSnapshot, label: &str) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    lines
        .get(index + 1)
        .unwrap_or_else(|| panic!("no value under {label:?}\n{}", text(screen)))
        .clone()
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
    click_where(app, label, |hit| hit.x < 24);
}

fn click_body(app: &crate::support::App, label: &str, y_min: u16, y_max: u16) {
    click_where(app, label, |hit| {
        hit.x > 24 && hit.y >= y_min && hit.y < y_max
    });
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn check_frame(app: &crate::support::App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

/// The accepted wrap frame still paints Refresh with the press flash.
/// `WallClock` drops that flash after 140 ms, and a later paint locks the
/// released button.
/// The accepted focus-visible frame keeps Run task on the press fill while
/// Tab has already moved the gutter to Preview. The 140 ms flash is gone
/// by the time `visual` settles, which then paints the released accent.
fn run_task_pressed(screen: &TerminalSnapshot) -> bool {
    let Some(row) = (0..screen.rows()).find(|&row| row_text(screen, row).contains("Run task"))
    else {
        return false;
    };
    let line = row_text(screen, row);
    let Some(at) = line.find("Run task") else {
        return false;
    };
    screen
        .cell_at(at as u32, row)
        .is_some_and(|cell| cell.bg() == SnapshotColor::Rgb(0x2e, 0x9a, 0x38))
}

fn refresh_still_pressed(screen: &TerminalSnapshot) -> bool {
    let Some(row) = (0..screen.rows()).find(|&row| {
        let line = row_text(screen, row);
        line.contains("▎Remove") && line.contains("Refresh")
    }) else {
        return false;
    };
    let line = row_text(screen, row);
    let Some(at) = line.find("Refresh") else {
        return false;
    };
    screen
        .cell_at(at as u32, row)
        .is_some_and(|cell| cell.bg() == SnapshotColor::Rgb(0x4a, 0x4a, 0x52))
}

fn reveal(app: &crate::support::App, needle: &str) {
    let point = Point { x: 60, y: 18 };
    app.wheel_until(point, true, 1, 8, needle, |screen| {
        text(screen).contains(needle)
    });
}

/// The icon-button title can sit on the last content row. The close plate
/// is the next row, so keep scrolling until that row is in the viewport.
fn icon_row(app: &crate::support::App) -> TerminalSnapshot {
    let point = Point { x: 60, y: 18 };
    app.wheel_until(point, true, 1, 40, "the close plate", |screen| {
        let visible = text(screen);
        visible.contains("Close ×") && visible.contains("Title · 5-cell plate")
    })
}

fn ink(screen: &TerminalSnapshot, point: Point) -> tui_test::SnapshotColor {
    screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell")
        .fg()
}

fn crosses(app: &crate::support::App) -> (Point, Point) {
    reveal(app, "5-cell plate");
    let screen = app.settle();
    let mut marks: Vec<_> = find_all(&screen, "×")
        .into_iter()
        .filter(|hit| hit.y > 20)
        .collect();
    assert!(
        marks.len() >= 2,
        "expected two close plates, got {marks:?}\n{}",
        text(&screen)
    );
    marks.sort_by_key(|hit| hit.x);
    (marks[0], marks[1])
}

fn hovered(app: &crate::support::App, mut point: Point) -> tui_test::SnapshotColor {
    app.move_to(point);
    let mut screen = app.settle();
    let shifted = find_all(&screen, "×")
        .into_iter()
        .find(|hit| hit.x == point.x && hit.y > 20 && hit.y != point.y);
    if let Some(hit) = shifted {
        point = hit;
        app.move_to(point);
        screen = app.settle();
    }
    ink(&screen, point)
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "buttons"];
    let app = open("s02-route-page", 120, 40, &args);
    let screen = app.wait_text("Run task");
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Run task",
            "○ Auto-approve",
            "● Verbose",
            "not in the Tab order",
            FITS,
            NEXT,
            "Disabled primary",
        ],
        &[
            "Brand lockup",
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "keyboard-only",
            "30 columns",
            OVERVIEW,
        ],
    );
    shot(
        &app,
        "sc.buttons.route",
        "--page buttons opens the playground with the sidebar on Buttons",
        &args,
    );
    app.finish();

    let app = open("s02-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    let stops = [
        "Brand lockup",
        "Status & hints",
        "Breadcrumbs",
        "Help & keys",
        "Quit & leave",
        "Focus & navigation",
        "Run task",
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Run task");
    see(
        &entered,
        &[TITLE, BLURB, "Run task", CAP_PLAY],
        &[OVERVIEW, "Brand lockup", "Page options"],
    );
    app.key("[");
    let left = app.wait_text("Focus & navigation");
    see(&left, &["Focus & navigation"], &["Run task", TITLE]);
    app.key("]");
    let again = app.wait_text("Run task");
    see(&again, &[TITLE, "Run task"], &["Focus & navigation"]);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW, CAP_PLAY], &[TITLE, "Run task"]);
    click_side(&app, "Inputs & forms");
    app.wait_text("Buttons");
    click_side(&app, "Buttons");
    let pointed = app.wait_text("Run task");
    see(
        &pointed,
        &[TITLE, BLURB, "Run task", "● Verbose"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "buttons"];
    let app = open("s02-sections", 80, 24, &args);
    let narrow_play = app.wait_text("Run task");
    see(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "Run task",
            "Preview",
            "Delete branch",
            "Auto-approve",
            "Verbose",
            "Disabled primary",
            "Start long job",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            FITS,
            "the live widget",
        ],
    );
    shot(
        &app,
        "sc.buttons.playground.80x24",
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
            "Focus ring",
            "always",
            "Toolbar in Tab order off",
            "start",
            "more",
            "theme",
            "full",
            "Arrows wrap in the row off",
            "Next row message on",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "keyboard-only",
            "Run task",
            WIDE,
        ],
    );
    assert!(
        line_after(&narrow_opt, "Row overflow").contains("more"),
        "narrow options lost the more value\n{}",
        text(&narrow_opt)
    );
    shot(
        &app,
        "sc.buttons.options.80x24",
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
            "busy",
            "overflow",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "error",
            "editing",
            "empty",
            "loading",
            "narrow",
            WIDE,
        ],
    );
    shot(
        &app,
        "sc.buttons.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    app.wait_text(WIDE);
    let body = Point { x: 50, y: 16 };
    app.wheel_until(body, false, 2, 8, "the wide states grid", |screen| {
        let visible = text(screen);
        visible.contains(WIDE) && visible.contains("○ Compare levels") && visible.contains("rest")
    });
    let wide_states = app.wait_text("Label wit…");
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "Primary",
            "Icon ×",
            "rest",
            "focus + hover",
            "overflow",
            "Label wit…",
            "● Label",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "error",
            "editing",
            "empty",
            "loading",
            "narrow",
            "256 colors",
        ],
    );
    shot(
        &app,
        "sc.buttons.states.120x40",
        "states caption and the nine grid rows at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.buttons.states",
        "states is the visible Buttons section",
        &args,
    );
    for (case_id, label) in SHOWN {
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES],
            &[
                "● Compare levels",
                "Page options",
                "error",
                "editing",
                "narrow",
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
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "Page options",
            "always",
            "Toolbar in Tab order off",
            "start",
            "more",
            "theme",
            "full",
            "Arrows wrap in the row off",
            "Next row message on",
            "Danger hover on × off",
            "thin",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "keyboard-only",
            "Run task",
            "colon",
        ],
    );
    assert!(line_after(&wide_opt, "Icon plate pad").contains('1'));
    assert!(line_after(&wide_opt, "States label column").contains("15"));
    assert!(!line_after(&wide_opt, "States label column").contains("10"));
    shot(
        &app,
        "sc.buttons.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.buttons.options",
        "options is the visible Buttons section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[
            TITLE,
            CAP_PLAY,
            WIDE,
            "Run task",
            "not in the Tab order",
            FITS,
            NEXT,
            "● Verbose",
            "Disabled primary",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "30 columns",
            "keyboard-only",
        ],
    );
    let toolbar = row_with(&wide_play, "Tab order");
    assert!(
        toolbar.contains("not in the Tab order"),
        "toolbar meta changed: {toolbar}"
    );
    shot(
        &app,
        "sc.buttons.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.buttons.playground",
        "playground is the visible Buttons section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "buttons"];
    let app = open("s02-compare", 120, 40, &args);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", "overflow", CAP_STATES],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 12);
    let on = app.wait_text("256 colors");
    see(
        &on,
        &[TITLE, "● Compare levels", "256 colors", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    for _ in 0..6 {
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
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.buttons.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_text(TITLE);
    let visible = text(&narrow);
    let band = visible.contains("truecolor")
        || visible.contains("256 colors")
        || visible.contains("16 colors")
        || visible.contains("no color");
    assert!(
        band && visible.contains("Compare levels") && !visible.contains(WIDE),
        "narrow compare bands missing\n{visible}"
    );
    see(
        &narrow,
        &[TITLE, "Compare levels"],
        &["Page options", CAP_PLAY, WIDE, "120×40"],
    );
    shot(
        &app,
        "sc.buttons.compare-levels.80x24",
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
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("always");
    see(
        &options,
        &[
            "always",
            "Toolbar in Tab order off",
            "start",
            "more",
            "theme",
            "full",
            "thin",
            CAP_OPT,
        ],
        &["keyboard-only", "colon", "Compare levels", "end"],
    );
    app.finish();
}

#[test]
fn press_run_then_reject_disabled() {
    let args = ["--page", "buttons"];
    let app = open("s02-press", 120, 40, &args);
    let screen = app.wait_text("Run task");
    see(
        &screen,
        &["Run task", "Preview"],
        &["Run task ✓", "Preview ✓"],
    );
    let run = locate(&app, "Run task", |hit| hit.y < 20 && hit.x > 24);
    app.move_to(run);
    let hovered = app.settle();
    see(
        &hovered,
        &["Run task", "Preview"],
        &["Run task ✓", "Working…"],
    );
    app.down(run);
    let held = app.settle();
    see(
        &held,
        &["Run task", TITLE],
        &["Run task ✓", "Preview ✓", "Working…"],
    );
    app.up(run);
    let released = app.wait_text("Run task ✓");
    let row = row_with(&released, "Run task");
    assert!(
        row.contains("▎Run task"),
        "released Run task lost the focus bar: {row}"
    );
    see(
        &released,
        &[TITLE, "Run task ✓", "Preview", "Disabled primary", CAP_PLAY],
        &[
            "Preview ✓",
            "Working…",
            "Disabled primary ✓",
            "Delete branch ✓",
            "Page options",
        ],
    );
    shot(
        &app,
        "sc.buttons.press",
        "Run task click reports Run task ✓ and does not activate Preview",
        &args,
    );

    click_body(&app, "Preview", 8, 20);
    let preview = app.wait_text("Preview ✓");
    see(
        &preview,
        &["Preview ✓", "▎Preview"],
        &["Run task ✓", "Working…"],
    );
    click_body(&app, "Disabled primary", 8, 24);
    let disabled = app.settle();
    see(
        &disabled,
        &["Preview ✓", "Disabled primary"],
        &["Disabled primary ✓", "Working…"],
    );
    click_body(&app, "Start long job", 8, 28);
    let busy = app.wait_text("Working…");
    see(
        &busy,
        &["Working…", "Start long job", TITLE],
        &["Preview ✓", "Long job finished", "Query finished"],
    );
    app.finish();
}

#[test]
fn option_toolbar_from_the_control() {
    let args = ["--page", "buttons"];
    let app = open("s02-toolbar", 120, 40, &args);
    click_tab(&app, "Options");
    let before = app.wait_text("Toolbar in Tab order off");
    see(
        &before,
        &[
            "always",
            "Toolbar in Tab order off",
            "start",
            "more",
            CAP_OPT,
        ],
        &["Toolbar in Tab order on", "keyboard-only", CAP_PLAY],
    );
    click_where(&app, "Toolbar in Tab order", |hit| hit.x > 24);
    let toggled = app.wait_text("Toolbar in Tab order on");
    see(
        &toggled,
        &[
            "Toolbar in Tab order on",
            "always",
            "start",
            "more",
            "theme",
            "full",
        ],
        &["Toolbar in Tab order off", "keyboard-only", "end", CAP_PLAY],
    );
    click_tab(&app, "Playground");
    let playground = app.wait_text("in the Tab order");
    let toolbar = row_with(&playground, "Tab order");
    assert!(
        toolbar.contains("in the Tab order") && !toolbar.contains("not in the Tab order"),
        "toolbar meta did not switch: {toolbar}"
    );
    see(
        &playground,
        &[TITLE, CAP_PLAY, FITS, NEXT, "● Verbose"],
        &["Page options", "not in the Tab order", "30 columns"],
    );
    shot(
        &app,
        "opt.buttons.toolbar_focus",
        "Toolbar in Tab order switched on from the Options control; other options stay default",
        &args,
    );
    app.finish();
}

fn option_page(app: &crate::support::App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options")
}

#[test]
fn option_focus_visible_hides_the_pointer_bar() {
    let args = [
        "--page",
        "buttons",
        "--opt",
        "buttons.focus_visible=keyboard-only",
    ];
    let app = open("s02-focus", 120, 40, &args);
    let options = option_page(&app);
    see(
        &options,
        &[
            "keyboard-only",
            "Toolbar in Tab order off",
            "start",
            "more",
            "thin",
        ],
        &["always", "Toolbar in Tab order on", "colon"],
    );
    click_tab(&app, "Playground");
    click_body(&app, "Run task", 8, 20);
    // The press fill lasts 140 ms. Tab as soon as the click result is up,
    // and keep the frame that still has that fill with the gutter on Preview.
    // `visual` waits the fill out and then paints the released accent.
    let started = Instant::now();
    let mut tabbed = false;
    let mut clicked = None;
    let held = loop {
        let screen = app.screen();
        let visible = text(&screen);
        let line = (0..screen.rows())
            .map(|row| row_text(&screen, row))
            .find(|line| line.contains("Run task"))
            .unwrap_or_default();
        let pressed = run_task_pressed(&screen);
        if !tabbed && visible.contains("Run task ✓") && pressed && !line.contains('▎') {
            clicked = Some(screen);
            app.key("tab");
            tabbed = true;
            continue;
        }
        if tabbed
            && pressed
            && line.contains("▎Preview")
            && !line.contains("▎Run")
            && visible.contains("Run task ✓")
        {
            break screen;
        }
        assert!(
            started.elapsed() < Duration::from_millis(400) && app.child_running(),
            "pressed Run task with the Preview gutter was not seen (tabbed={tabbed} pressed={pressed})\n{line}\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    let clicked = clicked.expect("click frame");
    let row = row_with(&clicked, "Run task");
    assert!(
        !row.contains('▎'),
        "pointer focus still drew a focus bar: {row}"
    );
    see(
        &clicked,
        &["Run task ✓", "Preview", CAP_PLAY],
        &["Preview ✓", "▎Run task", "Working…"],
    );
    see(
        &held,
        &["Run task ✓", "▎"],
        &["Preview ✓", "Working…", "Disabled primary ✓"],
    );
    check_frame(&app, &held, &visual_id("opt.buttons.focus_visible"));
    app.finish();
}

#[test]
fn option_row_align_end() {
    let args = ["--page", "buttons", "--opt", "buttons.row_align=end"];
    let app = open("s02-align", 120, 40, &args);
    let play = app.wait_text(FITS);
    let row = row_with(&play, "Refresh");
    let at = row.find("Refresh").expect("Refresh");
    assert!(at > 50, "end-aligned Refresh stayed at column {at}: {row}");
    assert!(row.contains("Remove…"), "end align dropped Remove: {row}");
    see(
        &play,
        &[FITS, NEXT, "not in the Tab order", "● Verbose"],
        &["30 columns", "behind", "start"],
    );
    let options = option_page(&app);
    assert!(line_after(&options, "Row align").contains("end"));
    let overflow = line_after(&options, "Row overflow");
    assert!(
        overflow.contains("more") && !overflow.contains("wrap"),
        "overflow line: {overflow}"
    );
    assert!(line_after(&options, "Row width").contains("full"));
    see(
        &options,
        &["end", "always", "Toolbar in Tab order off"],
        &["keyboard-only"],
    );
    click_tab(&app, "Playground");
    app.wait_text(FITS);
    shot(
        &app,
        "opt.buttons.row_align",
        "row align end keeps every button on the full-width row and leaves overflow at more",
        &args,
    );
    app.finish();
}

#[test]
fn option_row_overflow_wrap() {
    let args = ["--page", "buttons", "--opt", "buttons.row_overflow=wrap"];
    let app = open("s02-overflow", 120, 40, &args);
    let play = app.wait_text(FITS);
    let row = row_with(&play, "Refresh");
    assert!(
        row.contains("Validate") && row.contains("Remove…"),
        "wrap hid a button that fits: {row}"
    );
    see(&play, &[FITS, NEXT], &["behind", "30 columns"]);
    let options = option_page(&app);
    assert!(line_after(&options, "Row overflow").contains("wrap"));
    assert!(line_after(&options, "Row align").contains("start"));
    assert!(line_after(&options, "Row gap").contains("theme"));
    assert!(line_after(&options, "Row width").contains("full"));
    see(
        &options,
        &["wrap", "Arrows wrap in the row off", "always"],
        &["keyboard-only", "end"],
    );
    shot(
        &app,
        "opt.buttons.row_overflow",
        "row overflow wrap is selected and align, gap, and width stay at their defaults",
        &args,
    );
    app.finish();
}

#[test]
fn option_row_gap_four() {
    let args = ["--page", "buttons", "--opt", "buttons.row_gap=4"];
    let app = open("s02-gap", 120, 40, &args);
    let play = app.wait_text("Refresh      Validate");
    let row = row_with(&play, "Refresh");
    assert!(
        row.contains("Refresh      Validate") && !row.contains("Refresh    Validate"),
        "gap 4 did not open the row: {row}"
    );
    see(&play, &[FITS, "Remove…", NEXT], &["behind", "30 columns"]);
    let options = option_page(&app);
    let gap = line_after(&options, "Row gap");
    assert!(
        gap.contains('4') && !gap.contains("theme"),
        "gap line: {gap}"
    );
    assert!(line_after(&options, "Row align").contains("start"));
    assert!(line_after(&options, "Row overflow").contains("more"));
    shot(
        &app,
        "opt.buttons.row_gap",
        "row gap 4 separates Refresh and Validate; align stays start and overflow stays more",
        &args,
    );
    app.finish();
}

#[test]
fn option_row_width_thirty() {
    let args = ["--page", "buttons", "--opt", "buttons.row_width=30"];
    let app = open("s02-width", 120, 40, &args);
    let play = app.wait_text("30 columns · behind …: Validate, Duplicate, Edit…");
    let row = row_with(&play, "Refresh");
    assert!(
        row.contains("…") && row.contains("Remove…") && !row.contains("Validate"),
        "width 30 still showed Validate on the row: {row}"
    );
    see(
        &play,
        &[
            "30 columns · behind …: Validate, Duplicate, Edit…",
            NEXT,
            "not in the Tab order",
        ],
        &[FITS, "full width"],
    );
    let options = option_page(&app);
    let width = line_after(&options, "Row width");
    assert!(
        width.contains("30") && !width.contains("full") && !width.contains("48"),
        "width line: {width}"
    );
    assert!(line_after(&options, "Row overflow").contains("more"));
    assert!(line_after(&options, "Row align").contains("start"));
    shot(
        &app,
        "opt.buttons.row_width",
        "row width 30 hides Validate, Duplicate, and Edit behind …; overflow stays more",
        &args,
    );
    app.finish();
}

#[test]
fn option_row_wrap_reaches_the_last_button() {
    let args = ["--page", "buttons", "--opt", "buttons.row_wrap=on"];
    let app = open("s02-wrap", 120, 40, &args);
    let options = option_page(&app);
    see(
        &options,
        &[
            "Arrows wrap in the row on",
            "Next row message on",
            "Toolbar in Tab order off",
            "always",
        ],
        &["Arrows wrap in the row off", "keyboard-only"],
    );
    assert!(line_after(&options, "Row align").contains("start"));
    assert!(line_after(&options, "Row width").contains("full"));
    click_tab(&app, "Playground");
    let mut wrapped = None;
    for _ in 0..6 {
        click_where(&app, "Refresh", |hit| hit.y > 24);
        app.key("left");
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(200) {
            let screen = app.screen();
            if refresh_still_pressed(&screen) {
                wrapped = Some(screen);
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if wrapped.is_some() {
            break;
        }
    }
    let wrapped = wrapped.unwrap_or_else(|| {
        panic!(
            "left from Refresh did not keep the press flash\n{}",
            text(&app.screen())
        )
    });
    let row = row_with(&wrapped, "Refresh");
    assert!(
        row.contains("▎Remove") && !row.contains("▎Refresh"),
        "left did not wrap to Remove: {row}"
    );
    see(
        &wrapped,
        &["Refresh ✓", FITS, NEXT],
        &["Validate ✓", "behind", "Arrows wrap in the row off"],
    );
    check_frame(&app, &wrapped, &visual_id("opt.buttons.row_wrap"));
    app.finish();
}

#[test]
fn option_next_message_off() {
    let args = ["--page", "buttons", "--opt", "buttons.next_message=off"];
    let app = open("s02-message", 120, 40, &args);
    let play = app.wait_text("Open logs");
    let row = row_with(&play, "Open logs");
    assert!(
        row.contains("Next") && row.contains("Share") && !row.contains(NEXT),
        "next message still on the row: {row}"
    );
    see(
        &play,
        &[FITS, "Retry failed", "Copy run ID"],
        &[NEXT, "30 columns", "behind"],
    );
    let options = option_page(&app);
    see(
        &options,
        &[
            "Next row message off",
            "Arrows wrap in the row off",
            "always",
        ],
        &["Next row message on", "keyboard-only"],
    );
    shot(
        &app,
        "opt.buttons.next_message",
        "the next row drops 2 not available yet and arrow wrap stays off",
        &args,
    );
    app.finish();
}

#[test]
fn option_icon_pad_zero() {
    let args = ["--page", "buttons", "--opt", "buttons.icon_pad=0"];
    let app = open("s02-pad", 120, 40, &args);
    reveal(&app, "Icon buttons");
    let play = icon_row(&app);
    let row = row_with(&play, "Close");
    assert!(
        row.contains("Close ×") && !row.contains("Close  ×"),
        "pad 0 still used a padded close plate: {row}"
    );
    assert!(
        row.contains("Title · 5-cell plate"),
        "the 5-cell plate changed with icon pad: {row}"
    );
    see(
        &play,
        &["Danger hover", "hover a plate for its tooltip"],
        &["Close  ×"],
    );
    let options = option_page(&app);
    let pad = line_after(&options, "Icon plate pad");
    assert!(
        pad.contains('0') && !pad.contains('1') && !pad.contains('2'),
        "pad line: {pad}"
    );
    see(
        &options,
        &["Danger hover on × off", "always"],
        &["Danger hover on × on", "keyboard-only"],
    );
    shot(
        &app,
        "opt.buttons.icon_pad",
        "icon pad 0 draws Close × without the default pad; danger hover stays off",
        &args,
    );
    app.finish();
}

#[test]
fn option_icon_danger_matches_the_danger_plate() {
    let defaults = ["--page", "buttons"];
    let off = open("s02-danger-off", 120, 40, &defaults);
    reveal(&off, "Danger hover");
    let (plain, danger) = crosses(&off);
    let plain_ink = hovered(&off, plain);
    let danger_ink = hovered(&off, danger);
    assert_ne!(
        plain_ink, danger_ink,
        "default close hover matched the danger plate: {plain_ink:?}"
    );
    off.finish();

    let args = ["--page", "buttons", "--opt", "buttons.icon_danger=on"];
    let app = open("s02-danger", 120, 40, &args);
    let options = option_page(&app);
    see(
        &options,
        &["Danger hover on × on", "always", "Toolbar in Tab order off"],
        &["Danger hover on × off", "keyboard-only"],
    );
    let pad = line_after(&options, "Icon plate pad");
    assert!(pad.contains('1') && !pad.contains('0'), "pad line: {pad}");
    click_tab(&app, "Playground");
    reveal(&app, "Danger hover");
    let (plain, danger) = crosses(&app);
    let plain_ink = hovered(&app, plain);
    let danger_ink = hovered(&app, danger);
    assert_eq!(
        plain_ink, danger_ink,
        "danger hover on did not paint the close plate like Danger hover: {plain_ink:?} vs {danger_ink:?}"
    );
    shot(
        &app,
        "opt.buttons.icon_danger",
        "danger hover on paints the close plate like Danger hover; icon pad stays 1",
        &args,
    );
    app.finish();
}

#[test]
fn option_label_width_ten() {
    let args = ["--page", "buttons", "--opt", "buttons.label_width=10"];
    let app = open("s02-label", 120, 40, &args);
    click_tab(&app, "States");
    let states = app.wait_text("focus + …");
    see(
        &states,
        &[
            "focus + …",
            "rest",
            "overflow",
            "○ Compare levels",
            LEGEND,
            "Primary",
        ],
        &[
            "focus + hover",
            "Page options",
            "● Compare levels",
            "error",
            "narrow",
            ":Label",
        ],
    );
    let options = option_page(&app);
    let width = line_after(&options, "States label column");
    assert!(
        width.contains("10") && !width.contains("15") && !width.contains("20"),
        "label width line: {width}"
    );
    assert!(line_after(&options, "States focus mark").contains("thin"));
    shot(
        &app,
        "opt.buttons.label_width",
        "label width 10 truncates focus + hover to focus + … and the focus mark stays thin",
        &args,
    );
    app.finish();
}

#[test]
fn option_focus_mark_colon() {
    let args = ["--page", "buttons", "--opt", "buttons.focus_mark=colon"];
    let app = open("s02-mark", 120, 40, &args);
    click_tab(&app, "States");
    let states = app.wait_text(": depicts a focused sample");
    see(
        &states,
        &[
            ": depicts a focused sample; it is not where your focus is",
            ":Label",
            "focus + hover",
            "○ Compare levels",
            "overflow",
        ],
        &[LEGEND, "▏Label", "Page options", "error", "narrow"],
    );
    let options = option_page(&app);
    assert!(line_after(&options, "States focus mark").contains("colon"));
    let width = line_after(&options, "States label column");
    assert!(
        width.contains("15") && !width.contains("10"),
        "width line: {width}"
    );
    shot(
        &app,
        "opt.buttons.focus_mark",
        "the states focus mark is a colon and the label column stays 15",
        &args,
    );
    app.finish();
}
