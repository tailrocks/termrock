//! Columns route, sections, specimens, compare levels, options, and seams.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s03_columns/` and each case id's dots become underscores.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::bins::showcase;
use crate::support::{Launch, Point, find_all, row_text, text, try_launch};

const CRUMB: &str = "Tables › Columns";
const TITLE: &str = "Columns";
const BLURB: &str = "Column rules, droppable columns, resizable seams and the header menu";
const BLURB_NARROW: &str = "Column rules, droppable columns, resizable seams…";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const SOLVER: &str = "Width solver";
const ROW_82: &str = "Row at 82 cells";
const ROW_28: &str = "Row at 28 cells";
const DRAG_META: &str = "drag a se";
const USER_META: &str = "user widths";
const FIXED_META: &str = "fixed widths";
const SAMPLE: &str = "payments-platform";
const DATE: &str = "2026-11-02";
const NOTES: &str = "seats renew";
const HEAVY: &str = "┃";

const SPECIMENS: &[(&str, &str)] = &[
    ("sc.columns.specimen.rest", "rest"),
    ("sc.columns.specimen.hover", "hover"),
    ("sc.columns.specimen.focused", "focused"),
    ("sc.columns.specimen.selected", "selected"),
    ("sc.columns.specimen.overflow", "overflow"),
    ("sc.columns.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s03_columns/{}", case_id.replace('.', "_"))
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
    screen
        .row(u32::from(hits[0].y) + 1)
        .expect("option value row")
        .iter()
        .skip(usize::from(hits[0].x))
        .map(|item| item.text())
        .collect::<String>()
        .trim()
        .to_string()
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

fn cell_fg(screen: &TerminalSnapshot, x: u16, y: u16) -> SnapshotColor {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|item| item.fg())
        .unwrap_or(SnapshotColor::Default)
}

fn heavy_columns(screen: &TerminalSnapshot, y: u16) -> Vec<u16> {
    screen
        .row(u32::from(y))
        .expect("header row")
        .iter()
        .enumerate()
        .filter(|(_, item)| item.text() == HEAVY)
        .map(|(index, _)| u16::try_from(index).unwrap())
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

/// The accepted Playground shot still has the tab's press flash, after focus
/// has left the tab. A later paint locks the released tab.
fn tab_press_held(screen: &TerminalSnapshot) -> bool {
    let Some(hit) = find_all(screen, "Playground")
        .into_iter()
        .find(|hit| hit.y == 4)
    else {
        return false;
    };
    let gutter = hit.x.saturating_sub(1);
    let flash = |x| {
        screen
            .cell_at(u32::from(x), u32::from(hit.y))
            .is_some_and(|cell| cell.bg() == SnapshotColor::Rgb(0x4a, 0x4a, 0x52))
    };
    flash(hit.x)
        && flash(gutter)
        && screen
            .cell_at(u32::from(gutter), u32::from(hit.y))
            .is_some_and(|cell| cell.text() != "▎")
}

fn cursor_parked(screen: &TerminalSnapshot, col: u32, row: u32) -> bool {
    let cursor = screen.cursor();
    cursor.col() == col && cursor.row() == row && !cursor.visible()
}

fn header_present(screen: &TerminalSnapshot) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains("ID")
            && line.contains("Task")
            && line.contains("Status")
            && line.contains("Branch")
    })
}

fn sample_held(
    app: &crate::support::App,
    pred: impl Fn(&TerminalSnapshot) -> bool,
) -> Option<TerminalSnapshot> {
    let started = Instant::now();
    let mut since: Option<Instant> = None;
    while started.elapsed() < Duration::from_millis(400) {
        let screen = app.screen();
        if pred(&screen) {
            let mark = since.get_or_insert_with(Instant::now);
            if mark.elapsed() >= Duration::from_millis(80) {
                return Some(screen);
            }
        } else {
            since = None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

/// The accepted no-color frame parks the hidden cursor at column 118.
/// The settled frame keeps the same cells and moves that cursor to 107.
fn no_color_cursor(screen: &TerminalSnapshot) -> bool {
    let raw = text(screen);
    if !raw.contains(CRUMB)
        || !raw.contains("no color")
        || !raw.contains(WIDE)
        || raw.contains("Page options")
        || raw.contains(CAP_PLAY)
        || raw.contains("○ Compare levels")
    {
        return false;
    }
    let cursor = screen.cursor();
    cursor.col() == 118 && cursor.row() == 36 && !cursor.visible()
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 8 && hit.x >= 20)
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

fn scroll_panel(app: &crate::support::App, needle: &str) {
    let point = Point { x: 119, y: 20 };
    for _ in 0..6 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, true, 1);
    }
    app.wait_text(needle);
}

fn column_header(screen: &TerminalSnapshot) -> (u16, u16, u16, u16) {
    let mut found = None;
    for hit in find_all(screen, "ID") {
        let line = row_text(screen, u32::from(hit.y));
        if !line.contains("Status") || !line.contains("Branch") {
            continue;
        }
        let task = find_all(screen, "Task")
            .into_iter()
            .find(|item| item.y == hit.y && item.x > hit.x);
        let owner = find_all(screen, "Owner")
            .into_iter()
            .find(|item| item.y == hit.y && item.x > hit.x);
        if let (Some(task), Some(owner)) = (task, owner) {
            found = Some((hit.x, task.x, owner.x, hit.y));
            break;
        }
    }
    found.unwrap_or_else(|| panic!("resizable header missing\n{}", text(screen)))
}

fn pick(app: &crate::support::App, label: &str, downs: u8) {
    let hit = locate(app, label, |item| item.x >= 26);
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

fn assert_drag_mark(screen: &TerminalSnapshot, glyph_x: u16, y: u16, owner_x: u16) {
    assert_eq!(
        cell(screen, glyph_x, y),
        HEAVY,
        "dragged seam lost its heavy rule"
    );
    assert_ne!(
        cell(screen, glyph_x + 1, y),
        HEAVY,
        "padding cell took the seam glyph\n{}",
        row_text(screen, u32::from(y))
    );
    let accent = cell_fg(screen, glyph_x, y);
    assert_ne!(
        accent,
        cell_fg(screen, glyph_x + 1, y),
        "padding cell took the drag colour"
    );
    let other = owner_x.saturating_sub(2);
    assert_ne!(other, glyph_x, "owner seam collided with the dragged seam");
    assert_ne!(
        cell(screen, other, y),
        HEAVY,
        "another seam took the drag glyph\n{}",
        row_text(screen, u32::from(y))
    );
    assert_ne!(
        accent,
        cell_fg(screen, other, y),
        "another seam took the drag colour"
    );
    let marks = heavy_columns(screen, y);
    let bar = *marks.last().expect("scrollbar");
    assert_ne!(bar, glyph_x, "the dragged seam is the scrollbar");
    assert_eq!(
        marks,
        vec![glyph_x, bar],
        "a second seam took the heavy rule\n{}",
        row_text(screen, u32::from(y))
    );
    assert_ne!(
        accent,
        cell_fg(screen, bar, y),
        "the scrollbar took the drag colour"
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-route-page", 120, 40, &args);
    let screen = app.wait_text(ROW_82);
    see(
        &screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            SOLVER,
            ROW_82,
            DRAG_META,
            SAMPLE,
            "enterprise",
            DATE,
            NOTES,
            "#1040",
            "Add rate limiting",
            "droppable",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            USER_META,
            FIXED_META,
            "Sort Ascending",
            "Sort A to Z",
            OVERVIEW,
            ROW_28,
        ],
    );
    let side = locate(&app, "› Columns", |hit| hit.x < 24);
    let side_row = row_text(&app.screen(), u32::from(side.y));
    assert!(
        side_row.contains('▎'),
        "sidebar focus left Columns: {side_row}"
    );
    let (id_x, task_x, _, y) = column_header(&screen);
    assert_eq!(task_x, id_x + 7, "default ID width is not 5");
    assert_eq!(
        heavy_columns(&screen, y),
        vec![119],
        "a seam glyph is up at rest"
    );
    shot(
        &app,
        "sc.columns.route",
        "--page columns opens the playground with the sidebar on Columns",
        &args,
    );
    app.finish();

    let app = open("s03-columns-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(SOLVER);
    see(
        &entered,
        &[CRUMB, BLURB, ROW_82, DRAG_META, SAMPLE],
        &[OVERVIEW, "Page options", "Tables › Cell editors"],
    );
    app.key("[");
    let left = app.wait_text("Tables › Cell editors");
    see(&left, &["Tables › Cell editors"], &[CRUMB, SOLVER, ROW_82]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, SOLVER, DRAG_META],
        &["Tables › Cell editors"],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, SOLVER]);
    click_side(&app, "Tables");
    app.wait_text("Basic table");
    click_side(&app, "Columns");
    let pointed = app.wait_text(ROW_82);
    see(
        &pointed,
        &[CRUMB, BLURB, SOLVER, SAMPLE, DRAG_META, "#1040"],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            "Tables › Cell editors",
        ],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-sections", 80, 24, &args);
    let narrow_play = app.wait_text(SOLVER);
    see(
        &narrow_play,
        &[
            CRUMB,
            TITLE,
            BLURB_NARROW,
            "Playground",
            SOLVER,
            ROW_82,
            "payments-platform",
            "enterprise",
            "2026-1",
            "droppable",
            "Resizable columns",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "and the header menu",
            DATE,
            NOTES,
            DRAG_META,
            USER_META,
            FIXED_META,
            "#1040",
        ],
    );
    shot(
        &app,
        "sc.columns.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    assert!(option_value(&narrow_opt, "Row width").contains("full"));
    assert!(
        option_value(&narrow_opt, "Seam hit width").contains('2')
            && !option_value(&narrow_opt, "Seam hit width").contains('1')
            && !option_value(&narrow_opt, "Seam hit width").contains('3')
    );
    assert!(
        option_value(&narrow_opt, "Narrowest drag").contains('3')
            && !option_value(&narrow_opt, "Narrowest drag").contains('6')
    );
    assert!(option_value(&narrow_opt, "Header menu labels").contains("default"));
    let toggle = row_with(&narrow_opt, "Resizable columns");
    assert!(
        toggle.contains(" on") && !toggle.contains(" off"),
        "narrow resizable toggle: {toggle}"
    );
    see(
        &narrow_opt,
        &[
            CRUMB,
            "Page options",
            "Row width",
            "full",
            "Resizable columns",
            "Seam hit width",
            "Narrowest drag",
            "Header menu labels",
            "default",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            SOLVER,
            SAMPLE,
            WIDE,
            "truecolor",
            "custom",
        ],
    );
    shot(
        &app,
        "sc.columns.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "States");
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
            "focused",
            "Resizable",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            WIDE,
            "truecolor",
            "selected",
            "overflow",
            "narrow",
            "pressed",
            "focus + hover",
            "disabled",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
        ],
    );
    assert!(
        grid_label(&narrow_states, "focused")
            && !grid_label(&narrow_states, "selected")
            && !grid_label(&narrow_states, "overflow")
            && !grid_label(&narrow_states, "narrow"),
        "80 by 24 states clipped the wrong rows\n{}",
        text(&narrow_states)
    );
    shot(
        &app,
        "sc.columns.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    // The first frame after a resize can be blank, or carry the wide caption
    // before the lower specimen rows are drawn.
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "selected")
            && grid_label(screen, "overflow")
            && grid_label(screen, "narrow")
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
            "focused",
            "selected",
            "overflow",
            "narrow",
            "Resizable",
            "Fixed widths",
            "Add rate limit",
            "mira",
            "›",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "pressed",
            "focus + hover",
            "disabled",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
            "256 colors",
            SOLVER,
        ],
    );
    shot(
        &app,
        "sc.columns.states.120x40",
        "states caption and every Columns specimen at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.columns.states",
        "states is the visible Columns section",
        &args,
    );
    for (case_id, label) in SPECIMENS {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the screen before its capture\n{}",
            text(&app.screen())
        );
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES],
            &[
                "● Compare levels",
                "Page options",
                "pressed",
                "disabled",
                "busy",
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
    assert!(
        option_value(&wide_opt, "Row width").contains("full")
            && !option_value(&wide_opt, "Row width").contains("28")
    );
    assert!(row_with(&wide_opt, "Resizable columns").contains(" on"));
    assert!(
        option_value(&wide_opt, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(
        option_value(&wide_opt, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    assert!(
        option_value(&wide_opt, "Header menu labels").contains("default")
            && !option_value(&wide_opt, "Header menu labels").contains("custom")
    );
    see(
        &wide_opt,
        &[
            CRUMB,
            CAP_OPT,
            WIDE,
            "Page options",
            "Row width",
            "full",
            "default",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            SOLVER,
            SAMPLE,
            "custom",
            ROW_28,
        ],
    );
    shot(
        &app,
        "sc.columns.options.120x40",
        "options caption and the default values at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.columns.options",
        "options is the visible Columns section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_screen("playground at 120 by 40", |screen| {
        text(screen).contains(CAP_PLAY)
            && text(screen).contains(DRAG_META)
            && text(screen).contains(DATE)
    });
    see(
        &wide_play,
        &[
            CRUMB,
            BLURB,
            CAP_PLAY,
            WIDE,
            SOLVER,
            ROW_82,
            DRAG_META,
            SAMPLE,
            "enterprise",
            DATE,
            NOTES,
            "#1040",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            USER_META,
            FIXED_META,
            ROW_28,
            "Sort Ascending",
        ],
    );
    shot(
        &app,
        "sc.columns.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.columns.playground",
        "playground is the visible Columns section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-compare", 120, 40, &args);
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
    let point = Point { x: 70, y: 20 };
    let mut bands = None;
    for _ in 0..24 {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(200) {
            let screen = app.screen();
            if no_color_cursor(&screen) {
                bands = Some(screen);
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if bands.is_some() || text(&app.screen()).contains("no color") {
            break;
        }
        app.wheel(point, true, 2);
    }
    let bands = bands.unwrap_or_else(|| {
        let screen = app.screen();
        let cursor = screen.cursor();
        panic!(
            "no-color cursor at {},{} not 118,36\n{}",
            cursor.col(),
            cursor.row(),
            text(&screen)
        )
    });
    see(
        &bands,
        &[CRUMB, "no color", WIDE],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    check_frame(&app, &bands, &visual_id("sc.columns.compare-levels.120x40"));
    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(CRUMB)
    });
    if !text(&narrow).contains("no color") {
        reveal(&app, &["no color"]);
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[CRUMB, "no color"],
        &[
            "Page options",
            CAP_PLAY,
            WIDE,
            "120×40",
            "truecolor",
            "○ Compare levels",
        ],
    );
    shot(
        &app,
        "sc.columns.compare-levels.80x24",
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
    assert!(option_value(&options, "Row width").contains("full"));
    assert!(row_with(&options, "Resizable columns").contains(" on"));
    assert!(
        option_value(&options, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(
        option_value(&options, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    assert!(option_value(&options, "Header menu labels").contains("default"));
    see(
        &options,
        &["full", "Page options", CAP_OPT, "default"],
        &["custom", "Compare levels", ROW_28, "72"],
    );
    app.finish();
}

#[test]
fn drag_seam_then_release() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-seams", 120, 40, &args);
    let screen = app.wait_text(DRAG_META);
    see(
        &screen,
        &[
            CRUMB,
            SOLVER,
            ROW_82,
            DRAG_META,
            "#1040",
            "Add rate limiting",
        ],
        &[USER_META, FIXED_META, "Page options", ROW_28],
    );
    let (id_x, task_x, _, y) = column_header(&screen);
    assert_eq!(task_x, id_x + 7, "default ID width is not 5");
    assert_eq!(
        heavy_columns(&screen, y),
        vec![119],
        "rest header already has a seam glyph\n{}",
        row_text(&screen, u32::from(y))
    );
    shot(
        &app,
        "sc.columns.seams.before",
        "resizable header at rest, before a seam is hovered",
        &args,
    );

    let seam = Point { x: task_x - 2, y };
    app.move_to(seam);
    let hovered = app.wait_screen("seam hover", |screen| cell(screen, seam.x, seam.y) == HEAVY);
    assert_ne!(
        cell(&hovered, seam.x + 1, seam.y),
        HEAVY,
        "hover painted the padding cell"
    );
    assert_eq!(
        heavy_columns(&hovered, y),
        vec![seam.x, 119],
        "hover painted another seam\n{}",
        row_text(&hovered, u32::from(y))
    );
    app.down(seam);
    let dragged_to = Point {
        x: seam.x + 8,
        y: seam.y,
    };
    app.drag_to(dragged_to);
    let dragging = app.wait_screen("seam drag", |screen| {
        let (_, task, _, _) = column_header(screen);
        task == task_x + 8 && row_with(screen, "Resizable columns").contains(USER_META)
    });
    let (_, new_task, new_owner, drag_y) = column_header(&dragging);
    assert_eq!(new_task, id_x + 15, "drag did not grow ID to width 13");
    assert_drag_mark(&dragging, new_task - 2, drag_y, new_owner);
    let meta = row_with(&dragging, "Resizable columns");
    assert!(
        meta.contains(USER_META) && !meta.contains(DRAG_META) && !meta.contains(FIXED_META),
        "drag meta: {meta}"
    );
    see(
        &dragging,
        &[CRUMB, "#1040", "Add rate limiting", USER_META],
        &[DRAG_META, FIXED_META, "Page options", ROW_28],
    );
    shot(
        &app,
        "sc.columns.seams.after",
        "ID seam held at width 13, before the pointer is released",
        &args,
    );

    app.up(dragged_to);
    let released = app.settle();
    let (_, held_task, _, _) = column_header(&released);
    assert_eq!(held_task, new_task, "release restored the dragged width");
    let meta = row_with(&released, "Resizable columns");
    assert!(
        meta.contains(USER_META) && !meta.contains(DRAG_META),
        "released meta: {meta}"
    );

    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    let toggle = row_with(&options, "Resizable columns");
    assert!(
        toggle.contains(" on"),
        "resizable left its default: {toggle}"
    );
    click_where(&app, "Resizable columns", |hit| hit.x >= 26);
    let off = app.wait_screen("resizable off", |screen| {
        row_with(screen, "Resizable columns").contains(" off")
    });
    let toggle = row_with(&off, "Resizable columns");
    assert!(
        toggle.contains(" off") && !toggle.contains(" on"),
        "toggle did not turn off: {toggle}"
    );
    assert!(option_value(&off, "Row width").contains("full"));
    assert!(
        option_value(&off, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    click_tab(&app, "Playground");
    let fixed = app.wait_text("#1040");
    let meta = row_with(&fixed, "Resizable columns");
    assert!(
        !meta.contains(DRAG_META) && !meta.contains(USER_META) && !meta.contains("fixed"),
        "fixed meta: {meta}"
    );
    let (fixed_id, fixed_task, _, fixed_y) = column_header(&fixed);
    assert_eq!(
        fixed_task,
        fixed_id + 7,
        "fixed columns kept the dragged width"
    );
    let fixed_seam = Point {
        x: fixed_task - 2,
        y: fixed_y,
    };
    app.down(fixed_seam);
    app.drag_to(Point {
        x: fixed_seam.x + 8,
        y: fixed_seam.y,
    });
    app.up(Point {
        x: fixed_seam.x + 8,
        y: fixed_seam.y,
    });
    let ignored = app.settle();
    let (_, still, _, still_y) = column_header(&ignored);
    assert_eq!(still, fixed_task, "a fixed column reacted to the seam drag");
    assert_eq!(
        heavy_columns(&ignored, still_y),
        vec![119],
        "a fixed column painted a seam\n{}",
        row_text(&ignored, u32::from(still_y))
    );
    assert!(
        !row_with(&ignored, "Resizable columns").contains(USER_META),
        "a fixed column recorded a user width"
    );
    app.finish();
}

#[test]
fn option_row_width() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-width", 120, 40, &args);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert!(option_value(&options, "Row width").contains("full"));
    assert!(row_with(&options, "Resizable columns").contains(" on"));
    assert!(
        option_value(&options, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(
        option_value(&options, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    assert!(option_value(&options, "Header menu labels").contains("default"));
    pick(&app, "Row width", 4);
    let chosen = app.settle();
    let value = option_value(&chosen, "Row width");
    assert!(
        value.contains("28") && !value.contains("full"),
        "width value: {value}"
    );
    assert!(row_with(&chosen, "Resizable columns").contains(" on"));
    assert!(
        option_value(&chosen, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(
        option_value(&chosen, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    assert!(option_value(&chosen, "Header menu labels").contains("default"));
    click_tab(&app, "Playground");
    let narrow = app.wait_text(ROW_28);
    see(
        &narrow,
        &[
            CRUMB,
            ROW_28,
            "payments-plat",
            "74%",
            "Account",
            "Usage",
            DRAG_META,
        ],
        &[
            ROW_82,
            "enterprise",
            DATE,
            NOTES,
            "Page options",
            CAP_OPT,
            USER_META,
            FIXED_META,
        ],
    );
    shot(
        &app,
        "opt.columns.width",
        "row width 28 drops Plan, Renews, and Notes from the sample row",
        &args,
    );
    app.finish();
}

#[test]
fn option_resizable_off() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-resizable", 120, 40, &args);
    let home = app.wait_text("#1040");
    let (home_id, home_task, _, home_y) = column_header(&home);
    let seam = Point {
        x: home_task - 2,
        y: home_y,
    };
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert!(row_with(&options, "Resizable columns").contains(" on"));
    assert!(option_value(&options, "Row width").contains("full"));
    click_where(&app, "Resizable columns", |hit| hit.x >= 26);
    let off = app.wait_screen("resizable off", |screen| {
        row_with(screen, "Resizable columns").contains(" off")
    });
    let toggle = row_with(&off, "Resizable columns");
    assert!(
        toggle.contains(" off") && !toggle.contains(" on"),
        "resizable toggle: {toggle}"
    );
    assert!(option_value(&off, "Row width").contains("full"));
    assert!(
        option_value(&off, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(
        option_value(&off, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    assert!(option_value(&off, "Header menu labels").contains("default"));
    let dragged = Point {
        x: seam.x + 8,
        y: seam.y,
    };
    let mut ignored = None;
    for _ in 0..6 {
        // The tab flash expires on the next paint after 140ms. Drag before that.
        click_tab(&app, "Playground");
        app.move_to(seam);
        app.down(seam);
        app.drag_to(dragged);
        if let Some(screen) = sample_held(&app, |screen| resizable_frame(screen, home_id, seam.x)) {
            ignored = Some(screen);
            break;
        }
        app.up(dragged);
    }
    let ignored = ignored.unwrap_or_else(|| {
        let screen = app.screen();
        let cursor = screen.cursor();
        panic!(
            "resizable drag lost the Playground press at cursor {},{}\n{}",
            cursor.col(),
            cursor.row(),
            text(&screen)
        )
    });
    let meta = row_with(&ignored, "Resizable columns");
    assert!(
        !meta.contains(DRAG_META) && !meta.contains(USER_META) && !meta.contains("fixed"),
        "fixed meta: {meta}"
    );
    let (id_x, task_x, _, still_y) = column_header(&ignored);
    assert_eq!(task_x, id_x + 7, "turning resize off changed the ID width");
    assert_eq!(task_x, home_task, "a disabled seam drag moved Task");
    assert!(
        !heavy_columns(&ignored, still_y).contains(&seam.x),
        "a disabled seam painted a heavy rule\n{}",
        row_text(&ignored, u32::from(still_y))
    );
    see(
        &ignored,
        &[CRUMB, ROW_82, SAMPLE, DATE, "#1040"],
        &[DRAG_META, USER_META, FIXED_META, "Page options", ROW_28],
    );
    check_frame(&app, &ignored, &visual_id("opt.columns.resizable"));
    app.up(dragged);
    app.finish();
}

fn resizable_frame(screen: &TerminalSnapshot, id_x: u16, seam_x: u16) -> bool {
    let raw = text(screen);
    if !raw.contains(CRUMB)
        || !raw.contains(ROW_82)
        || !raw.contains(SAMPLE)
        || !raw.contains(DATE)
        || !raw.contains("#1040")
        || !raw.contains("↑↓ Move")
        || !raw.contains("←→ Column")
        || raw.contains(DRAG_META)
        || raw.contains(USER_META)
        || raw.contains(FIXED_META)
        || raw.contains("Page options")
        || raw.contains(ROW_28)
        || !tab_press_held(screen)
        || !cursor_parked(screen, 37, 39)
        || !header_present(screen)
    {
        return false;
    }
    let (id, task, _, y) = column_header(screen);
    id == id_x && task == id_x + 7 && !heavy_columns(screen, y).contains(&seam_x)
}

#[test]
fn option_seam_hit_width() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-seam-w", 120, 40, &args);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert!(
        option_value(&options, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    pick(&app, "Seam hit width", 1);
    let chosen = app.settle();
    let value = option_value(&chosen, "Seam hit width");
    assert!(
        value.trim_start().starts_with('3') && !value.contains('2'),
        "seam value: {value}"
    );
    assert!(option_value(&chosen, "Row width").contains("full"));
    assert!(row_with(&chosen, "Resizable columns").contains(" on"));
    assert!(
        option_value(&chosen, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    assert!(
        option_value(&chosen, "Header menu labels").contains("default")
            && !option_value(&chosen, "Header menu labels").contains("custom")
    );
    see(
        &chosen,
        &[
            CRUMB,
            CAP_OPT,
            "Page options",
            "Seam hit width",
            "full",
            "default",
        ],
        &[CAP_PLAY, SOLVER, SAMPLE, "custom", "72", ROW_28],
    );
    shot(
        &app,
        "opt.columns.seam_w",
        "seam hit width is 3 and the other options stay at their defaults",
        &args,
    );
    app.finish();
}

#[test]
fn option_min_width() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-min-width", 120, 40, &args);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert!(
        option_value(&options, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    pick(&app, "Narrowest drag", 2);
    let chosen = app.settle();
    let value = option_value(&chosen, "Narrowest drag");
    assert!(
        value.contains("10") && !value.contains('6') && !value.trim_start().starts_with('3'),
        "min width value: {value}"
    );
    assert!(option_value(&chosen, "Row width").contains("full"));
    assert!(row_with(&chosen, "Resizable columns").contains(" on"));
    assert!(
        option_value(&chosen, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(option_value(&chosen, "Header menu labels").contains("default"));
    click_tab(&app, "Playground");
    let play = app.wait_screen("playground before the min-width drag", |screen| {
        text(screen).contains(DRAG_META) && text(screen).contains("#1040") && header_present(screen)
    });
    let (id_x, task_x, _, y) = column_header(&play);
    assert_eq!(task_x, id_x + 7, "min width changed ID before a drag");
    // The 140ms tab flash dies while the grid finishes painting. Press the
    // selected tab again and drag inside the new flash. A missed flash leaves
    // the clamped columns in place, so the next attempt drags the live seam.
    let mut clamped = None;
    let mut dest = Point {
        x: task_x.saturating_sub(2),
        y,
    };
    for _ in 0..6 {
        let current = app.screen();
        let (_, task, _, seam_y) = column_header(&current);
        let seam = Point {
            x: task.saturating_sub(2),
            y: seam_y,
        };
        dest = Point {
            x: seam.x.saturating_sub(20),
            y: seam.y,
        };
        click_tab(&app, "Playground");
        app.move_to(seam);
        app.down(seam);
        app.drag_to(dest);
        if let Some(screen) = sample_held(&app, |screen| min_width_frame(screen, id_x)) {
            clamped = Some(screen);
            break;
        }
        app.up(dest);
    }
    let clamped = clamped.unwrap_or_else(|| {
        let screen = app.screen();
        let cursor = screen.cursor();
        panic!(
            "min-width drag lost the Playground press at cursor {},{}\n{}",
            cursor.col(),
            cursor.row(),
            text(&screen)
        )
    });
    let (_, clamped_task, _, _) = column_header(&clamped);
    assert_eq!(
        clamped_task,
        id_x + 12,
        "drag went below the width 10 floor"
    );
    assert_ne!(clamped_task, id_x + 5, "drag used the default floor of 3");
    let meta = row_with(&clamped, "Resizable columns");
    assert!(
        meta.contains(USER_META) && !meta.contains(DRAG_META) && !meta.contains(FIXED_META),
        "clamped meta: {meta}"
    );
    see(
        &clamped,
        &[CRUMB, ROW_82, "#1040", USER_META],
        &[DRAG_META, FIXED_META, "Page options", ROW_28],
    );
    check_frame(&app, &clamped, &visual_id("opt.columns.min_width"));
    app.up(dest);
    app.finish();
}

fn min_width_frame(screen: &TerminalSnapshot, id_x: u16) -> bool {
    let raw = text(screen);
    if !raw.contains(CRUMB)
        || !raw.contains(ROW_82)
        || !raw.contains("#1040")
        || !raw.contains(USER_META)
        || raw.contains(DRAG_META)
        || raw.contains(FIXED_META)
        || raw.contains("Page options")
        || raw.contains(ROW_28)
        || !tab_press_held(screen)
        || !cursor_parked(screen, 99, 37)
        || !header_present(screen)
    {
        return false;
    }
    let (id, task, _, _) = column_header(screen);
    id == id_x && task == id_x + 12
}

#[test]
fn option_header_labels() {
    let args = ["--page", "columns"];
    let app = open("s03-columns-labels", 120, 40, &args);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert!(option_value(&options, "Header menu labels").contains("default"));
    pick(&app, "Header menu labels", 1);
    let chosen = app.settle();
    let value = option_value(&chosen, "Header menu labels");
    assert!(
        value.contains("custom") && !value.contains("default"),
        "label value: {value}"
    );
    assert!(option_value(&chosen, "Row width").contains("full"));
    assert!(row_with(&chosen, "Resizable columns").contains(" on"));
    assert!(
        option_value(&chosen, "Seam hit width")
            .trim_start()
            .starts_with('2')
    );
    assert!(
        option_value(&chosen, "Narrowest drag")
            .trim_start()
            .starts_with('3')
    );
    click_tab(&app, "Playground");
    app.wait_text(SOLVER);
    scroll_panel(&app, "Sort A to Z");
    let menu = app.settle();
    see(
        &menu,
        &[
            CRUMB,
            "Header menu",
            "Sort A to Z",
            "Sort Z to A",
            "Unsort",
            "Not sorted",
            "available",
        ],
        &[
            "Sort Ascending",
            "Sort Descending",
            "Clear Sort",
            "Size Column to Fit",
            "Reset Column Widths",
            ROW_82,
            "Page options",
            "custom",
        ],
    );
    shot(
        &app,
        "opt.columns.header_labels",
        "custom header menu labels replace Sort Ascending",
        &args,
    );
    app.finish();
}
