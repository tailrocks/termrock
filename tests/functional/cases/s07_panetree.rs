//! Pane tree route, sections, specimens, compare levels, panes, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s07_panetree/` and each case id's dots become underscores.

use std::path::PathBuf;
use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Layout › Pane tree";
const TITLE: &str = "Pane tree";
const BLURB: &str = "PaneTree: nested splits, frames, focus moves, resize chords and zoom";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const KEYS_DEFAULT: &str = "F6 next pane · Alt+arrows move · Alt+Shift+arrows resize · Alt+Z zoom";
const KEYS_NONE: &str = "no pane keys: use the buttons and the pointer";
const PAGE_ARGS: &[&str] = &["--page", "panetree"];
const COLUMN: &str = "Pane tree";
const META: &str = "3 panes · focus shell";
const MIN_STATUS: &str = "Cannot resize: pane at minimum size";
const SEAM_X: u16 = 72;

const OMITTED: &[&str] = &[
    "pressed",
    "focus + hover",
    "disabled",
    "busy",
    "error",
    "editing",
    "empty",
    "loading",
    "overflow",
    "narrow",
];

const DRAWN_NARROW: &[&str] = &["rest", "hover", "focused"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s07_panetree/{}", case_id.replace('.', "_"))
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

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
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

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str) {
    let got = value_under(screen, label);
    assert_eq!(got, value, "{label} shows {got:?}, expected {value}");
}

fn assert_options(
    screen: &TerminalSnapshot,
    layout: &str,
    frames: &str,
    min: &str,
    keys: &str,
    step: &str,
    pad: &str,
) {
    assert_select(screen, "Starting layout", layout);
    assert_select(screen, "Pane frames", frames);
    assert_select(screen, "Pane minimum", min);
    assert_select(screen, "Pane keys", keys);
    assert_select(screen, "Resize step", step);
    assert_select(screen, "Seam hit padding", pad);
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (14..60).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn assert_column(screen: &TerminalSnapshot) {
    let hits = find_all(screen, COLUMN);
    assert!(
        hits.iter().any(|hit| hit.y >= 8 && hit.x > 20),
        "states column {COLUMN:?} missing {hits:?}\n{}",
        text(screen)
    );
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

fn cell(screen: &TerminalSnapshot, x: u16, y: u16) -> &str {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|item| item.text())
        .unwrap_or("")
}

/// The vertical seam's top cell sits between the two frame corners.
fn seam_at(screen: &TerminalSnapshot) -> (u16, u16) {
    for y in 10..36 {
        for x in 26..116 {
            let glyph = cell(screen, x, y);
            if (glyph == "│" || glyph == "┃")
                && cell(screen, x.saturating_sub(1), y) == "╮"
                && cell(screen, x + 1, y) == "╭"
            {
                return (x, y);
            }
        }
    }
    panic!("pane seam missing\n{}", text(screen));
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

fn click_content(app: &App, label: &str) -> Point {
    click_where(app, label, |hit| hit.y > 6 && hit.x > 26)
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

/// Layout starts expanded. Click the group only when the page label is hidden.
fn open_pane_from_sidebar(app: &App) {
    reveal_side(app, TITLE);
    let screen = app.settle();
    if !find_all(&screen, TITLE)
        .into_iter()
        .any(|hit| sidebar(&hit))
    {
        click_side(app, "Layout");
        reveal_side(app, TITLE);
    }
    click_side(app, TITLE);
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

fn cursor_only(fields: &str) -> bool {
    let mut any = false;
    for field in fields.split(',') {
        any = true;
        if field != "cursor.col" && field != "cursor.row" {
            return false;
        }
    }
    any
}

/// Cell delta against a frozen baseline. `None` means the snapshot matches.
fn baseline_delta(app: &App, screen: &TerminalSnapshot, test_id: &str) -> Option<(usize, String)> {
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
    match outcome {
        CheckOutcome::Pass(_) => None,
        CheckOutcome::Failure(failure) => Some((
            failure.changed_cell_total(),
            failure.changed_fields().join(","),
        )),
    }
}

/// The narrow +1 column nudge can drop the scrolled mono band back to the
/// truecolor top. Walk one wheel at a time and stop on the accepted 80×24
/// capture. The pointer stays at the wide-scroll point so hover matches.
fn restore_narrow_compare(app: &App) -> TerminalSnapshot {
    let id = visual_id("sc.panetree.compare-levels.80x24");
    let point = Point { x: 70, y: 18 };
    let mut screen = app.settle();
    let mut best: Option<(usize, String, String)> = None;
    for _ in 0..48 {
        let visible = text(&screen);
        if screen.cols() == 80 && screen.rows() == 24 && visible.contains("no color") {
            match baseline_delta(app, &screen, &id) {
                None => return screen,
                // The specimen wheel parks the hidden cursor on a content cell.
                // A one-row nudge leaves the scroll and parks that cursor in
                // the corner, which is where the accepted capture has it.
                Some((0, fields)) if cursor_only(&fields) => {
                    app.resize(80, 25);
                    app.wait_screen("compare row nudge", |screen| {
                        screen.cols() == 80 && screen.rows() == 25
                    });
                    app.resize(80, 24);
                    app.wait_screen("compare row restored", |screen| {
                        screen.cols() == 80 && screen.rows() == 24
                    });
                    let parked = app.settle();
                    if baseline_delta(app, &parked, &id).is_none() {
                        return parked;
                    }
                    let cursor = parked.cursor();
                    let detail = format!(
                        "{fields} parked {},{} vis {} no-color {}",
                        cursor.col(),
                        cursor.row(),
                        cursor.visible(),
                        text(&parked).contains("no color")
                    );
                    let closer = best.as_ref().is_none_or(|(n, _, _)| 0 < *n);
                    if closer {
                        best = Some((0, detail, visible.clone()));
                    }
                }
                Some((cells, fields)) => {
                    let closer = best.as_ref().is_none_or(|(n, _, _)| cells < *n);
                    if closer {
                        best = Some((cells, fields, visible.clone()));
                    }
                }
            }
        }
        let before = visible;
        app.wheel(point, true, 1);
        screen = app.settle();
        if text(&screen) == before {
            break;
        }
    }
    let cursor = screen.cursor();
    let (cells, fields, held) = best.unwrap_or((usize::MAX, String::new(), String::new()));
    panic!(
        "narrow compare did not match the 80x24 baseline ({cells} cells, {fields}, cursor {},{} vis {})\nheld:\n{held}\nlast:\n{}",
        cursor.col(),
        cursor.row(),
        cursor.visible(),
        text(&screen)
    );
}

fn leaves(screen: &TerminalSnapshot) {
    see(
        screen,
        &["cargo nextest", "Compiling", "16:42:01"],
        &["Page options", "Compare levels", "toggle_zoom"],
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s07-route-page", 120, 40, PAGE_ARGS);
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
            "Workspace panes",
            META,
            "Split right",
            "Split below",
            "Close pane",
            "Zoom",
            KEYS_DEFAULT,
            "cargo nextest",
            "Compiling",
            "16:42:01",
            "╭─ shell",
            "╭─ build",
            "╭─ logs",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "Restore",
            "1 pane",
            "4 panes",
            "zoomed",
            "toggle_zoom",
        ],
    );
    shot(&app, "sc.panetree.route");
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Workspace panes");
    see(
        &entered,
        &[CRUMB, BLURB, META, "cargo nextest"],
        &[OVERVIEW, "Page options", "Layout › Master-detail"],
    );
    app.key("[");
    let left = app.wait_text("Layout › Master-detail");
    see(&left, &["Layout › Master-detail"], &[CRUMB, BLURB, META]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Workspace panes", "cargo nextest"],
        &["Layout › Master-detail", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, "Workspace panes"]);
    open_pane_from_sidebar(&app);
    let pointed = app.wait_text("Workspace panes");
    see(
        &pointed,
        &[CRUMB, BLURB, META, "Split right", "cargo nextest"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s07-sections", 80, 24, PAGE_ARGS);
    app.wait_text("Workspace panes");
    flash_settle(&app);
    let narrow_play = app.settle();
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "PaneTree: nested splits",
            "Playground",
            "Workspace panes",
            "Split right",
            "cargo nextest",
            "╭─ shell",
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
            "80×24",
            "zoomed",
        ],
    );
    shot(&app, "sc.panetree.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_options(
        &narrow_opt,
        "three",
        "when-split",
        "12x3",
        "default",
        "2",
        "1",
    );
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Starting layout", "Seam hit padding"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "cargo nextest",
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.panetree.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert_column(&narrow_states);
    for label in DRAWN_NARROW {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
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
            "╭─ shell",
            "╭─ logs",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "cargo nextest",
            WIDE,
            "truecolor",
            "zoomed",
            LEGEND,
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.panetree.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "selected")
            && text(screen).contains("zoomed")
    });
    assert_size(&app, 120, 40);
    assert_column(&wide_states);
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
            "zoomed",
            "╭─ shell",
            "╭─ logs",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "cargo nextest",
            "256 colors",
        ],
    );
    assert_omitted(&wide_states);
    let selected = row_with(&wide_states, "selected");
    assert!(
        selected.contains("zoomed") && !selected.contains("logs"),
        "selected specimen is not the zoomed shell: {selected:?}"
    );
    let rest = row_with(&wide_states, "rest");
    assert!(
        rest.contains("shell") && rest.contains("logs"),
        "rest specimen dropped a pane: {rest:?}"
    );
    flash_settle(&app);
    for (case_id, label) in [
        ("sc.panetree.specimen.rest", "rest"),
        ("sc.panetree.specimen.hover", "hover"),
        ("sc.panetree.specimen.focused", "focused"),
        ("sc.panetree.specimen.selected", "selected"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        assert_column(&app.screen());
        shot(&app, case_id);
    }
    shot(&app, "sc.panetree.states.120x40");
    shot(&app, "sc.panetree.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_options(
        &wide_opt,
        "three",
        "when-split",
        "12x3",
        "default",
        "2",
        "1",
    );
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Starting layout"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "cargo nextest"],
    );
    shot(&app, "sc.panetree.options.120x40");
    shot(&app, "sc.panetree.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Workspace panes");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            META,
            KEYS_DEFAULT,
            "Split right",
            "cargo nextest",
            "Compiling",
            "╭─ logs",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Restore",
            "4 panes",
            "1 pane",
            "zoomed",
        ],
    );
    shot(&app, "sc.panetree.playground.120x40");
    shot(&app, "sc.panetree.playground");
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
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES, COLUMN],
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
    flash_settle(&app);
    shot(&app, "sc.panetree.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    flash_settle(&app);
    let narrow = restore_narrow_compare(&app);
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels", "80×24"],
    );
    shot(&app, "sc.panetree.compare-levels.80x24");

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
    assert_options(&options, "three", "when-split", "12x3", "default", "2", "1");
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "when-split"],
        &["Compare levels", OVERVIEW, "grid", "never"],
    );
    app.finish();
}

#[test]
fn split_focus_resize_close_and_zoom() {
    let app = page("s07-panes", None, 120, 40);
    app.wait_text(META);
    flash_settle(&app);
    let before = app.settle();
    assert_size(&app, 120, 40);
    leaves(&before);
    see(
        &before,
        &[CRUMB, META, KEYS_DEFAULT, "Zoom", "╭─ logs"],
        &["focus build", "focus editor", "Restore", "Opened editor"],
    );
    let (start, top) = seam_at(&before);
    assert_eq!(start, SEAM_X, "rest seam");
    assert_eq!(cell(&before, start, top + 2), "│");
    shot(&app, "sc.panetree.panes.before");

    click_content(&app, "cargo nextest");
    app.key("f6");
    let focused = app.wait_text("3 panes · focus build");
    leaves(&focused);
    see(
        &focused,
        &["3 panes · focus build", "16:42:01"],
        &["focus shell", "focus editor", "1 pane"],
    );

    let (x, top) = seam_at(&focused);
    assert_eq!(x, SEAM_X, "focus moved the seam");
    let y = top + 2;
    assert_eq!(cell(&focused, x, y), "│");
    let end = Point { x: x + 6, y };
    app.down(Point { x, y });
    app.drag_to(end);
    let held = app.settle();
    let (moved, moved_top) = seam_at(&held);
    assert_eq!(moved, x + 6, "held drag did not move the seam");
    assert_eq!(
        cell(&held, moved, moved_top),
        "┃",
        "held seam is not pressed"
    );
    see(
        &held,
        &[
            "3 panes · focus build",
            "cargo nextest",
            "Compiling",
            "16:42:01",
        ],
        &["focus shell", "focus editor", "Page options"],
    );
    shot(&app, "sc.panetree.panes.after");

    app.up(end);
    flash_settle(&app);
    let released = app.settle();
    let (kept, kept_top) = seam_at(&released);
    assert_eq!(kept, x + 6, "release or the settle nudge put the seam back");
    assert_eq!(
        cell(&released, kept, kept_top),
        "│",
        "released seam stays heavy"
    );
    see(
        &released,
        &[
            "3 panes · focus build",
            "cargo nextest",
            "Compiling",
            "16:42:01",
        ],
        &["focus shell", "focus logs", "Restore"],
    );

    click_button(&app, "Split right");
    let split = app.settle();
    assert!(
        footer(&split).contains("Opened editor"),
        "split status missing: {}",
        footer(&split)
    );
    assert!(
        !flat(&split).contains("focus build"),
        "split left focus on build\n{}",
        text(&split)
    );

    click_button(&app, "Zoom");
    app.wait_text("zoomed");
    flash_settle(&app);
    let zoomed = app.settle();
    see(
        &zoomed,
        &["zoomed", "focus editor", "toggle_zoom", "Restore"],
        &[
            "cargo nextest",
            "Compiling",
            "focus build",
            "focus shell",
            "3 panes",
        ],
    );
    assert!(
        find_all(&zoomed, "Zoom").is_empty(),
        "zoom left the Zoom button\n{}",
        text(&zoomed)
    );

    click_button(&app, "Close pane");
    app.wait_text("3 panes · focus shell");
    flash_settle(&app);
    let closed = app.settle();
    see(
        &closed,
        &[
            "3 panes · focus shell",
            "cargo nextest",
            "Compiling",
            "16:42:01",
            "Zoom",
        ],
        &[
            "focus build",
            "focus editor",
            "focus logs",
            "toggle_zoom",
            "Restore",
            "4 panes",
            "1 pane",
            "zoomed",
        ],
    );
    assert!(
        find_all(&closed, "Close pane").len() == 1,
        "close removed the Close pane button\n{}",
        text(&closed)
    );
    app.finish();
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("Workspace panes");
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
fn option_layout_grid() {
    let app = page("s07-layout", Some("panetree.layout=grid"), 120, 40);
    app.wait_text("4 panes");
    confirm_options(&app, |screen| {
        assert_options(screen, "grid", "when-split", "12x3", "default", "2", "1");
    });
    finish_option(&app, "opt.panetree.layout", |screen| {
        see(
            screen,
            &[
                "4 panes",
                "editor",
                "toggle_zoom",
                "╭─ shell",
                "╭─ logs",
                KEYS_DEFAULT,
                "cargo nextest",
            ],
            &["3 panes", "1 pane", "when-split", "Page options", KEYS_NONE],
        );
    });
}

#[test]
fn option_frames_never() {
    let app = page("s07-frames", Some("panetree.frames=never"), 120, 40);
    app.wait_text(META);
    confirm_options(&app, |screen| {
        assert_options(screen, "three", "never", "12x3", "default", "2", "1");
    });
    finish_option(&app, "opt.panetree.frames", |screen| {
        see(
            screen,
            &[META, "cargo nextest", "Compiling", "16:42:01", KEYS_DEFAULT],
            &[
                "╭",
                "╭─ shell",
                "╭─ logs",
                "always",
                "Page options",
                "1 pane",
            ],
        );
        let shell = row_with(screen, "cargo nextest");
        assert!(
            shell.contains('│'),
            "frames none also removed the seam: {shell:?}"
        );
    });
}

/// The pointer stops at the same boundary as the keyboard: the 24-column
/// minimum is the outer pane rectangle, so the seam cannot cross column 52.
#[test]
fn option_min_24x6_pointer_drag_stops_at_the_minimum() {
    let app = page("s07-min-drag", Some("panetree.min=24x6"), 120, 40);
    app.wait_text(META);
    let (start, top) = seam_at(&app.settle());
    assert_eq!(start, SEAM_X, "rest seam");
    let y = top + 2;
    let end = Point { x: 30, y };
    app.down(Point { x: start, y });
    app.drag_to(end);
    let held = app.settle();
    assert_eq!(seam_at(&held).0, 52, "the drag crossed the pane minimum");
    app.up(end);
    flash_settle(&app);
    let released = app.settle();
    assert_eq!(seam_at(&released).0, 52, "release moved the minimum seam");
    app.finish();
}

#[test]
fn option_min_24x6() {
    let app = page("s07-min", Some("panetree.min=24x6"), 120, 40);
    app.wait_text(META);
    confirm_options(&app, |screen| {
        assert_options(screen, "three", "when-split", "24x6", "default", "2", "1");
    });
    click_content(&app, "cargo nextest");
    flash_settle(&app);
    let mut refused = None;
    for _ in 0..20 {
        app.key("alt+shift+left");
        let screen = app.settle();
        if footer(&screen).contains(MIN_STATUS) {
            refused = Some(seam_at(&screen).0);
            break;
        }
    }
    let stopped = refused.unwrap_or_else(|| panic!("minimum status never appeared"));
    assert_eq!(stopped, 52, "minimum seam");
    app.key("alt+shift+left");
    let again = app.settle();
    assert_eq!(seam_at(&again).0, 52, "pane shrank past the minimum");
    assert!(
        footer(&again).contains(MIN_STATUS),
        "footer lost the minimum status: {}",
        footer(&again)
    );
    assert!(
        row_with(&again, "WARN").contains("seam at minimum"),
        "the logs line was mistaken for the status\n{}",
        text(&again)
    );
    flash_settle(&app);
    let settled = app.settle();
    if !footer(&settled).contains(MIN_STATUS) {
        app.key("alt+shift+left");
    }
    let screen = app.settle();
    assert_eq!(
        seam_at(&screen).0,
        52,
        "settle nudge moved the minimum seam"
    );
    assert!(
        footer(&screen).contains(MIN_STATUS),
        "shot lost the minimum status: {}",
        footer(&screen)
    );
    finish_option(&app, "opt.panetree.min", |screen| {
        see(
            screen,
            &[
                MIN_STATUS,
                "junie-tui",
                "ca…",
                "Compiling",
                "3 panes · focus shell",
            ],
            &["cargo nextest", "focus build", "Page options", "1 pane"],
        );
    });
}

#[test]
fn option_keys_none() {
    let app = page("s07-keys", Some("panetree.keys=none"), 120, 40);
    app.wait_text(KEYS_NONE);
    confirm_options(&app, |screen| {
        assert_options(screen, "three", "when-split", "12x3", "none", "2", "1");
    });
    let before = app.settle();
    see(
        &before,
        &[KEYS_NONE, META, "Split right"],
        &[KEYS_DEFAULT, "F6 next pane", "Alt+Z", "Ctrl+N"],
    );
    let (start, _) = seam_at(&before);
    click_content(&app, "cargo nextest");
    app.key("f6");
    let keyed = app.settle();
    see(
        &keyed,
        &[META, KEYS_NONE],
        &["focus build", "focus logs", KEYS_DEFAULT],
    );
    app.key("alt+shift+left");
    let resized = app.settle();
    assert_eq!(seam_at(&resized).0, start, "keys none still resized");
    see(
        &resized,
        &[META, "cargo nextest", "Compiling"],
        &["focus build", "Opened editor"],
    );
    finish_option(&app, "opt.panetree.keys", |screen| {
        see(
            screen,
            &[KEYS_NONE, META],
            &[KEYS_DEFAULT, "F6 next pane", "Page options", "none"],
        );
    });
}

#[test]
fn option_step_5() {
    let app = page("s07-step", Some("panetree.step=5"), 120, 40);
    app.wait_text(META);
    confirm_options(&app, |screen| {
        assert_options(screen, "three", "when-split", "12x3", "default", "5", "1");
    });
    click_content(&app, "cargo nextest");
    flash_settle(&app);
    let (start, _) = seam_at(&app.settle());
    assert_eq!(start, SEAM_X, "step changed the rest seam");
    app.key("alt+shift+left");
    let moved = app.settle();
    let end = seam_at(&moved).0;
    assert_eq!(start - end, 5, "step 5 moved the seam by {}", start - end);
    see(
        &moved,
        &[META, "cargo nextest", "Compiling"],
        &["focus build", "Page options", MIN_STATUS],
    );
    flash_settle(&app);
    let settled = app.settle();
    assert_eq!(seam_at(&settled).0, end, "settle nudge changed the step");
    finish_option(&app, "opt.panetree.step", |screen| {
        see(
            screen,
            &[META, "cargo nextest", KEYS_DEFAULT],
            &["focus build", "across 6 binaries", "Page options"],
        );
    });
}

/// The seam's padded hit rect is registered under the pane, so a drag that
/// starts two cells into the pane never grabs the seam. The seam column itself
/// still resizes. The options control is where the value 3 is visible.
#[test]
fn option_hit_pad_3() {
    let app = page("s07-pad", Some("panetree.hit_pad=3"), 120, 40);
    app.wait_text(META);
    let (start, top) = seam_at(&app.settle());
    assert_eq!(start, SEAM_X, "hit pad changed the rest seam");
    let y = top + 3;
    let missed = Point { x: start - 2, y };
    app.down(missed);
    app.drag_to(Point { x: missed.x + 8, y });
    let held = app.settle();
    assert_eq!(seam_at(&held).0, start, "pane cell grabbed the seam");
    assert_eq!(cell(&held, start, top), "│");
    app.up(Point { x: missed.x + 8, y });
    flash_settle(&app);
    assert_eq!(
        seam_at(&app.settle()).0,
        start,
        "missed drag moved the seam"
    );

    let on = Point { x: start, y };
    app.down(on);
    app.drag_to(Point { x: start + 6, y });
    app.up(Point { x: start + 6, y });
    flash_settle(&app);
    let dragged = app.settle();
    assert_eq!(seam_at(&dragged).0, start + 6, "seam column did not resize");
    see(
        &dragged,
        &[META, "cargo nextest", "Compiling"],
        &["focus build", "Page options"],
    );

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.settle();
    assert_options(&options, "three", "when-split", "12x3", "default", "2", "3");
    see(
        &options,
        &["Seam hit padding", "Page options", CRUMB],
        &["cargo nextest", "Compare levels", "grid"],
    );
    assert_size(&app, 120, 40);
    shot(&app, "opt.panetree.hit_pad");
    app.finish();
}

#[test]
fn seam_release_outside_returns_the_seam_to_rest() {
    let app = page("s07-seam-release", None, 120, 40);
    app.wait_text(META);
    flash_settle(&app);
    let before = app.settle();
    let (x, top) = seam_at(&before);
    assert_eq!(x, SEAM_X, "rest seam");
    let y = top + 2;

    app.down(Point { x, y });
    let end = Point { x: x + 6, y };
    app.drag_to(end);
    app.up(Point { x: end.x, y: 0 });
    flash_settle(&app);
    let released = app.settle();

    let (kept, kept_top) = seam_at(&released);
    assert_eq!(kept, x + 6, "the release outside kept the dragged ratio");
    assert_eq!(
        cell(&released, kept, kept_top),
        "│",
        "a release outside the seam's own line left the seam pressed"
    );

    let next_y = kept_top + 2;
    app.down(Point { x: kept, y: next_y });
    let next_end = Point {
        x: kept + 4,
        y: next_y,
    };
    app.drag_to(next_end);
    let regrab = app.settle();
    let (moved, moved_top) = seam_at(&regrab);
    assert_eq!(
        moved,
        kept + 4,
        "the next gesture moved the seam from its press"
    );
    assert_eq!(
        cell(&regrab, moved, moved_top),
        "┃",
        "the next gesture draws the seam pressed while held"
    );
    app.up(next_end);
    app.finish();
}
