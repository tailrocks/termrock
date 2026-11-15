//! Diff route, sections, specimens, compare levels, review, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s08_diff/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Text & code › Diff";
const EDITOR: &str = "Text & code › Code editor";
const TITLE: &str = "Diff viewer";
const BLURB: &str = "Unified or old / new review · narrow panes use unified · select and copy text";
const BLURB_HEAD: &str = "Unified or old";
const BLURB_TAIL: &str = "select and copy text";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "diff"];
const FILE: &str = "config/service.toml";
const FILE_ADDED: &str = "retries.md";
const REMOVED: &str = "attempts = 3";
const ADDED: &str = "attempts = 5";
const RETRIES: &str = "# Retries";
const NO_FILE: &str = "No file selected";
const SAMPLE: &str = "retry.toml";

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "error",
    "editing",
    "loading",
    "overflow",
];

const DRAWN: &[&str] = &["rest", "focused", "empty", "narrow"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s08_diff/{}", case_id.replace('.', "_"))
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

fn assert_options(screen: &TerminalSnapshot, mode: &str, file: &str, width: &str) {
    let pick = |label: &str, value: &str, all: &[&str]| {
        let others: Vec<&str> = all.iter().copied().filter(|item| *item != value).collect();
        assert_select(screen, label, value, &others);
    };
    pick("Mode", mode, &["unified", "review"]);
    pick("File", file, &["changes", "none"]);
    pick("Page width", width, &["full", "narrow"]);
}

/// Specimen labels sit in the label column. "selected" is also inside
/// "No file selected", which is drawn in the cell to the right of that column.
fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 9
            && (18..30).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    grid_hit(screen, label).is_some()
}

/// A specimen cell is five rows tall. The state label shares only the first of them.
fn grid_block(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = grid_hit(screen, label)
        .unwrap_or_else(|| panic!("grid label {label:?} missing\n{}", text(screen)));
    let top = u32::from(hit.y);
    (top..top + 5)
        .map(|row| row_text(screen, row))
        .collect::<Vec<_>>()
        .join("\n")
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

fn click_button(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 4 && hit.x >= 26);
}

fn click_file(app: &App, label: &str) {
    click_where(app, label, |hit| (26..70).contains(&hit.x) && hit.y > 6);
}

fn click_diff(app: &App) {
    let screen = app.settle();
    if !find_all(&screen, "Diff").into_iter().any(|hit| hit.x < 24) {
        click_side(app, "Text & code");
    }
    click_side(app, "Diff");
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

fn panel_y(screen: &TerminalSnapshot, label: &str) -> u32 {
    find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 26 && hit.y > 6)
        .map(|hit| u32::from(hit.y))
        .min()
        .unwrap_or_else(|| panic!("{label} panel missing\n{}", text(screen)))
}

fn assert_side_by_side(screen: &TerminalSnapshot) {
    assert_eq!(
        panel_y(screen, "Changes"),
        panel_y(screen, "Diff"),
        "full width stacked the changes list and the diff\n{}",
        text(screen)
    );
}

fn assert_stacked(screen: &TerminalSnapshot) {
    let changes = panel_y(screen, "Changes");
    let diff = panel_y(screen, "Diff");
    assert!(
        diff > changes,
        "narrow width did not stack the diff under Changes (changes {changes}, diff {diff})\n{}",
        text(screen)
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
            "Playground",
            "○ Review",
            "○ Empty",
            "Changes",
            "4 files",
            FILE,
            REMOVED,
            ADDED,
            "5 hunks",
            FILE_ADDED,
            "legacy_backoff.rs",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "● Review",
            "● Empty",
            NO_FILE,
            "Old",
            "New",
            RETRIES,
            "Selection copied",
        ],
    );
    assert_side_by_side(screen);
}

fn assert_review(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            "● Review",
            "○ Empty",
            "Old",
            "New",
            RETRIES,
            FILE_ADDED,
        ],
        &[
            "○ Review",
            "● Empty",
            NO_FILE,
            REMOVED,
            ADDED,
            "Page options",
            "Compare levels",
        ],
    );
    assert_side_by_side(screen);
}

fn assert_review_changes(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            "● Review",
            "○ Empty",
            "Old",
            "New",
            FILE,
            REMOVED,
            ADDED,
            "5 hunks",
        ],
        &[
            "○ Review",
            "● Empty",
            NO_FILE,
            RETRIES,
            "Page options",
            "Compare levels",
        ],
    );
    assert_side_by_side(screen);
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

fn assert_sample_rows(screen: &TerminalSnapshot) {
    for label in DRAWN {
        assert!(
            grid_label(screen, label),
            "{label} left the states grid\n{}",
            text(screen)
        );
    }
    let rest = grid_block(screen, "rest");
    assert!(
        rest.contains(SAMPLE) && rest.contains(REMOVED) && rest.contains(ADDED),
        "rest specimen is not the retry.toml edit: {rest:?}"
    );
    let focused = grid_block(screen, "focused");
    assert!(
        focused.contains(SAMPLE) && focused.contains(ADDED),
        "focused specimen dropped the retry.toml edit: {focused:?}"
    );
    let empty = grid_block(screen, "empty");
    assert!(
        empty.contains(NO_FILE) && !empty.contains("attempts"),
        "empty specimen still shows a hunk: {empty:?}"
    );
    let narrow = grid_block(screen, "narrow");
    assert!(
        narrow.contains(SAMPLE) && !narrow.contains("Old") && !narrow.contains("New"),
        "narrow specimen is not the unified retry.toml diff: {narrow:?}"
    );
    see(
        screen,
        &["Unified", "Review", SAMPLE],
        &["Old", "New", "Page options", CAP_PLAY],
    );
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s08-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    shot(&app, "sc.diff.route");
    app.finish();

    let app = open("s08-route-keys", 120, 40, &[]);
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
        EDITOR,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(REMOVED);
    see(
        &entered,
        &[CRUMB, BLURB, FILE, REMOVED, ADDED, "○ Review"],
        &[OVERVIEW, "Page options", EDITOR, "● Review", NO_FILE],
    );
    app.key("[");
    let left = app.wait_text(EDITOR);
    see(&left, &[EDITOR], &[CRUMB, REMOVED, BLURB]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, FILE, ADDED], &[EDITOR, OVERVIEW, NO_FILE]);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, REMOVED, BLURB]);
    click_diff(&app);
    let pointed = app.wait_text(REMOVED);
    see(
        &pointed,
        &[CRUMB, BLURB, FILE, REMOVED, ADDED, "○ Review", "○ Empty"],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            "● Review",
            NO_FILE,
        ],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s08-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(FILE);
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            BLURB_HEAD,
            "Playground",
            "○ Review",
            "○ Empty",
            "Changes",
            FILE,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            BLURB_TAIL,
            "● Review",
            "● Empty",
            NO_FILE,
        ],
    );
    shot(&app, "sc.diff.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_options(&narrow_opt, "unified", "changes", "full");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Mode", "File", "Page width"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            FILE,
            REMOVED,
            WIDE,
            "truecolor",
            BLURB_TAIL,
        ],
    );
    shot(&app, "sc.diff.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "focused", "empty"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    assert!(
        !grid_label(&narrow_states, "narrow"),
        "narrow row is on the 80 by 24 states screen\n{}",
        text(&narrow_states)
    );
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "focused",
            "Unified",
            SAMPLE,
            REMOVED,
            ADDED,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "Review",
            WIDE,
            "truecolor",
            LEGEND,
            BLURB_TAIL,
            "Old",
            "New",
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.diff.states.80x24");

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
            "Unified",
            "Review",
            SAMPLE,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            FILE,
            "256 colors",
        ],
    );
    assert_sample_rows(&wide_states);
    assert_omitted(&wide_states);
    for (case_id, label) in [
        ("sc.diff.specimen.rest", "rest"),
        ("sc.diff.specimen.focused", "focused"),
        ("sc.diff.specimen.empty", "empty"),
        ("sc.diff.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.diff.states.120x40");
    shot(&app, "sc.diff.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_options(&wide_opt, "unified", "changes", "full");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Mode", "Page width"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", FILE, REMOVED],
    );
    shot(&app, "sc.diff.options.120x40");
    shot(&app, "sc.diff.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(REMOVED);
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.diff.playground.120x40");
    shot(&app, "sc.diff.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s08-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES, SAMPLE],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
            "Old",
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
    shot(&app, "sc.diff.compare-levels.120x40");

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
    shot(&app, "sc.diff.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    let top = app.settle();
    see(
        &top,
        &[TITLE, "● Compare levels"],
        &["no color", "Page options"],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest", SAMPLE],
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
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_options(&options, "unified", "changes", "full");
    see(
        &options,
        &["unified", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "● Compare levels"],
    );
    app.finish();
}

#[test]
fn diff_file_review_empty_and_narrow() {
    let app = open("s08-diff", 120, 40, PAGE_ARGS);
    let before = app.wait_text(REMOVED);
    assert_size(&app, 120, 40);
    assert_playground(&before);
    shot(&app, "sc.diff.diff.before");

    let origin = find_all(&before, "worker_0")
        .into_iter()
        .find(|hit| hit.x > 60)
        .unwrap_or_else(|| panic!("diff hunk missing\n{}", text(&before)));
    app.click(origin);
    app.wait_text("Copy");
    for _ in 0..30 {
        let visible = text(&app.screen());
        if !visible.contains("worker_0") && visible.contains("worker_4") {
            break;
        }
        app.key("down");
    }
    let scrolled = app.settle();
    see(
        &scrolled,
        &[FILE, "worker_4", REMOVED, "○ Review"],
        &[
            "worker_0",
            "5 hunks",
            RETRIES,
            "Old",
            "New",
            NO_FILE,
            "● Review",
            "Page options",
        ],
    );

    click_file(&app, FILE_ADDED);
    let added = app.wait_text(RETRIES);
    see(
        &added,
        &[FILE_ADDED, RETRIES, "○ Review", "○ Empty", "1 hunk"],
        &[
            REMOVED,
            ADDED,
            "worker_4",
            "● Review",
            NO_FILE,
            "Old",
            "Page options",
        ],
    );

    click_button(&app, "Review");
    app.wait_text("Old");
    flash_settle(&app);
    let review = app.settle();
    assert_review(&review);

    let line = find_all(&review, "Workers")
        .into_iter()
        .find(|hit| hit.x > 60)
        .unwrap_or_else(|| panic!("review line missing\n{}", text(&review)));
    app.down(line);
    app.drag_to(Point {
        x: line.x + 8,
        y: line.y,
    });
    app.up(Point {
        x: line.x + 8,
        y: line.y,
    });
    app.key("y");
    let copied = app.wait_text("Selection copied");
    see(
        &copied,
        &["Selection copied", FILE_ADDED, "Workers retry", "● Review"],
        &[NO_FILE, REMOVED, ADDED, "Page options"],
    );

    click_button(&app, "Empty");
    app.wait_text(NO_FILE);
    flash_settle(&app);
    let empty = app.settle();
    see(
        &empty,
        &[NO_FILE, "● Empty", "● Review", FILE_ADDED, "4 files"],
        &[
            RETRIES,
            "Workers retry",
            REMOVED,
            ADDED,
            "Old",
            "New",
            "○ Empty",
            "Page options",
        ],
    );

    click_button(&app, "Empty");
    app.wait_text("Workers retry");
    flash_settle(&app);
    let restored = app.settle();
    see(
        &restored,
        &["Workers retry", "○ Empty", "● Review", "Old", FILE_ADDED],
        &[NO_FILE, "● Empty", REMOVED],
    );

    app.resize(80, 24);
    let narrow = app.wait_screen("diff redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, FILE_ADDED, "● Review"],
        &[
            WIDE,
            NO_FILE,
            REMOVED,
            "○ Review",
            "Page options",
            BLURB_TAIL,
        ],
    );
    shot(&app, "sc.diff.diff.80x24");

    app.resize(120, 40);
    let after = app.wait_screen("diff restored at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(WIDE)
            && text(screen).contains("Workers retry")
    });
    assert_size(&app, 120, 40);
    see(
        &after,
        &[
            CRUMB,
            WIDE,
            "● Review",
            "○ Empty",
            "Old",
            "New",
            FILE_ADDED,
            "Workers retry",
        ],
        &[
            NO_FILE,
            "● Empty",
            REMOVED,
            ADDED,
            "○ Review",
            "Page options",
            OVERVIEW,
        ],
    );
    assert_side_by_side(&after);
    shot(&app, "sc.diff.diff.after");
    app.finish();
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text("Changes");
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
fn option_mode_review() {
    let app = page("s08-mode", Some("diff.mode=review"), 120, 40);
    app.wait_text("Old");
    confirm_options(&app, |screen| {
        assert_options(screen, "review", "changes", "full");
    });
    finish_option(&app, "opt.diff.mode", assert_review_changes);
}

#[test]
fn option_file_none() {
    let app = page("s08-file", Some("diff.file=none"), 120, 40);
    app.wait_text(NO_FILE);
    confirm_options(&app, |screen| {
        assert_options(screen, "unified", "none", "full");
    });
    finish_option(&app, "opt.diff.file", |screen| {
        see(
            screen,
            &[NO_FILE, "○ Review", "○ Empty", FILE, "4 files", "Changes"],
            &[
                REMOVED,
                ADDED,
                RETRIES,
                "● Empty",
                "● Review",
                "Old",
                "New",
                "5 hunks",
                "Page options",
            ],
        );
        assert_side_by_side(screen);
    });
}

#[test]
fn option_width_narrow() {
    let app = page("s08-width", Some("diff.width=narrow"), 120, 40);
    app.wait_text(REMOVED);
    confirm_options(&app, |screen| {
        assert_options(screen, "unified", "changes", "narrow");
    });
    finish_option(&app, "opt.diff.width", |screen| {
        see(
            screen,
            &[
                FILE,
                REMOVED,
                ADDED,
                "○ Review",
                "○ Empty",
                "Changes",
                "5 hunks",
            ],
            &[
                NO_FILE,
                "● Review",
                "Old",
                "New",
                "● Empty",
                "Page options",
                RETRIES,
            ],
        );
        assert_stacked(screen);
    });
}
