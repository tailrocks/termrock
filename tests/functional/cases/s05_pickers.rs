//! Pickers route, sections, specimens, compare levels, picker journeys, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s05_pickers/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, clusters, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Overlays › Pickers";
const TITLE: &str = "Pickers";
const BLURB: &str =
    "One modal list for files, tabs, levels, commands and branches: search, scope, status";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "pickers"];
const EMPTY_TEXT: &str = "No matches \u{2014} Esc clears the search";
const TASK_RATE: &str = "Add rate limiting";

const READOUT: &[(&str, &str)] = &[
    ("width", "width 64"),
    ("rows", "max rows 12"),
    ("place", "max rows 12 · Center"),
    ("fit", "margin 1"),
    ("back", "keeps 1 bottom"),
    ("cursor", "cursor Fill"),
    ("delete", "Delete secondary on"),
    ("nav", "nav desktop"),
    ("dismiss", "outside click on"),
    ("empty", EMPTY_TEXT),
    ("holder", "Type to search\u{2026}"),
    ("hints", "per picker"),
    ("remote", "Loading, then Ready"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s05_pickers/{}", case_id.replace('.', "_"))
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

fn result_row(screen: &TerminalSnapshot, label: &str) -> String {
    (0..screen.rows())
        .filter(|row| *row >= 28)
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(label))
        .unwrap_or_else(|| panic!("result row {label:?} missing\n{}", text(screen)))
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

fn normalized_readout_rows(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .map(|row| {
            row_text(screen, row)
                .replace(['│', '┃'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

fn readout_contains(rows: &[String], phrase: &str) -> bool {
    let phrase = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
    let rows = rows
        .iter()
        .map(|row| {
            row.replace(['│', '┃'], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>();
    if rows.iter().any(|row| row.contains(&phrase)) {
        return true;
    }

    let words = phrase.split_whitespace().collect::<Vec<_>>();
    (1..words.len()).any(|split| {
        let prefix = words[..split].join(" ");
        let suffix = words[split..].join(" ");
        rows.windows(2)
            .any(|pair| pair[0].ends_with(&prefix) && pair[1].starts_with(&suffix))
    })
}

fn assert_readout(screen: &TerminalSnapshot, skip: &[&str], present: &str, absent: &str) {
    let rows = normalized_readout_rows(screen);
    assert!(
        readout_contains(&rows, present),
        "readout missing {present:?}\n{}",
        text(screen)
    );
    assert!(
        !readout_contains(&rows, absent),
        "readout still shows {absent:?}\n{}",
        text(screen)
    );
    for (key, fragment) in READOUT {
        if skip.contains(key) {
            continue;
        }
        assert!(
            readout_contains(&rows, fragment),
            "another option changed; missing {fragment:?}\n{}",
            text(screen)
        );
    }
}

#[test]
fn readout_match_requires_the_complete_wrapped_phrase() {
    let wrapped = vec![
        "No matches — Esc clears the │".to_owned(),
        "┃ search”".to_owned(),
    ];
    assert!(readout_contains(&wrapped, EMPTY_TEXT));
    assert!(!readout_contains(
        &wrapped,
        "No matches — Esc clears the search” (stale)"
    ));

    let separated = vec![
        "No matches — Esc clears the".to_owned(),
        "unrelated readout".to_owned(),
        "search”".to_owned(),
    ];
    assert!(!readout_contains(&separated, EMPTY_TEXT));
}

/// Grid labels sit under the legend. The legend itself contains "focused".
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 9 && (18..56).contains(&hit.x))
}

fn grid_row(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = find_all(screen, label)
        .into_iter()
        .find(|hit| hit.y >= 9 && (18..56).contains(&hit.x))
        .unwrap_or_else(|| panic!("grid row {label:?} missing\n{}", text(screen)));
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
    click_where(app, label, |hit| hit.x < 24);
}

fn click_button(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 4 && hit.x >= 24);
}

fn click_disabled_export(app: &App) {
    let screen = app.wait_text("Export results\u{2026}");
    let hits: Vec<_> = find_all(&screen, "Export results\u{2026}")
        .into_iter()
        .filter(|hit| {
            hit.x >= 28
                && (10..28).contains(&hit.y)
                && row_text(&screen, u32::from(hit.y)).contains("Run a query first")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected the disabled command row, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
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

fn assert_level(screen: &TerminalSnapshot, mode: &str) {
    let line = result_row(screen, "Level");
    assert!(
        line.contains(mode),
        "level line {line:?} does not show {mode}"
    );
    for other in ["Silent", "Read-Only", "Alert", "(Full)"] {
        if other == mode {
            continue;
        }
        assert!(!line.contains(other), "level line changed to {line:?}");
    }
}

fn assert_chosen(screen: &TerminalSnapshot, value: &str, absent: &[&str]) {
    let line = result_row(screen, "Chosen");
    assert!(line.contains(value), "chosen line {line:?}");
    for item in absent {
        assert!(
            !line.contains(item),
            "chosen line still shows {item:?}: {line}"
        );
    }
}

fn assert_last(screen: &TerminalSnapshot, value: &str, absent: &[&str]) {
    let line = result_row(screen, "Last action");
    assert!(line.contains(value), "last action {line:?}");
    for item in absent {
        assert!(
            !line.contains(item),
            "last action still shows {item:?}: {line}"
        );
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s05-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
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
            "Open quickly",
            "Switch tab",
            "Choose a level",
            "Run command",
            "Remote branch",
            "width 64",
            "max rows 12",
            "nothing yet",
            "Safe Mode",
            "Query 1",
            "Pickers opened",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "width 96",
            "Loading, then Error",
        ],
    );
    assert_chosen(&screen, "nothing yet", &["schema.rs", "main"]);
    assert_level(&screen, "Safe Mode");
    shot(&app, "sc.pickers.route");
    app.finish();

    let app = open("s05-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Open quickly");
    see(
        &entered,
        &[CRUMB, BLURB, "Open quickly", "nothing yet", "Safe Mode"],
        &[OVERVIEW, "Page options", "Overlays › Dialogs"],
    );
    app.key("[");
    let left = app.wait_text("Overlays › Dialogs");
    see(
        &left,
        &["Overlays › Dialogs"],
        &[CRUMB, "Open quickly", BLURB],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Open quickly", "width 64"],
        &["Overlays › Dialogs", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Open quickly", BLURB]);
    click_side(&app, "Overlays");
    app.wait_text("Dialogs");
    click_side(&app, "Pickers");
    let pointed = app.wait_text("Open quickly");
    see(
        &pointed,
        &[CRUMB, BLURB, "Open quickly", "Remote branch", "nothing yet"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s05-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Open quickly");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "One modal list",
            "Playground",
            "Open quickly",
            "Switch tab",
            "Choose a level",
            "Run command",
            "Remote branch",
            "width 64",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "search, scope, status",
        ],
    );
    shot(&app, "sc.pickers.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_select(&narrow_opt, "Width", "auto", &["48", "96"]);
    assert_select(&narrow_opt, "Max rows", "12", &["6"]);
    assert_select(&narrow_opt, "Placement", "center", &["below"]);
    assert_select(&narrow_opt, "Fit", "default", &["flush", "third"]);
    assert_select(&narrow_opt, "Backdrop", "default", &["full"]);
    assert_select(&narrow_opt, "Cursor row", "fill", &["bar"]);
    assert_toggle(&narrow_opt, "Delete closes a tab", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Width", "Nav keys"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Open quickly",
            WIDE,
            "truecolor",
            "Remote fetch",
        ],
    );
    shot(&app, "sc.pickers.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert!(
        grid_label(&narrow_states, "rest") && grid_label(&narrow_states, "hover"),
        "narrow states clipped a top row\n{}",
        text(&narrow_states)
    );
    assert!(
        !grid_label(&narrow_states, "disabled"),
        "narrow states already shows the disabled row\n{}",
        text(&narrow_states)
    );
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "Branches",
            "main",
            "feat/rate-limit",
            "depicts a focused sample",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "No matches",
            "Fetching branches",
            "Could not reach origin",
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    shot(&app, "sc.pickers.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "disabled")
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
            "Branches",
            "Remote branches",
            "main",
            "feat/rate-limit",
            "release/4.2",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "No matches",
            "Fetching branches",
            "Could not reach origin",
            "256 colors",
        ],
    );
    for label in [
        "pressed",
        "focused",
        "focus + hover",
        "selected",
        "busy",
        "editing",
        "narrow",
    ] {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in [
        ("sc.pickers.specimen.rest", "rest"),
        ("sc.pickers.specimen.hover", "hover"),
        ("sc.pickers.specimen.disabled", "disabled"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the top states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.pickers.states.120x40");
    shot(&app, "sc.pickers.states");

    reveal_grid(&app, "overflow");
    let low = app.settle();
    for label in ["empty", "loading", "error", "overflow"] {
        assert!(
            grid_label(&low, label),
            "{label} is not on the scrolled states grid\n{}",
            text(&low)
        );
    }
    see(
        &low,
        &[
            "empty",
            "No matches",
            "loading",
            "Fetching branches",
            "error",
            "Could not reach origin",
            "r retries",
            "overflow",
        ],
        &["● Compare levels", "Page options", "focus + hover"],
    );
    let overflow = grid_row(&low, "overflow");
    assert!(
        overflow.contains('\u{2013}'),
        "overflow cell is not the unsupported dash: {overflow:?}"
    );
    for (case_id, label) in [
        ("sc.pickers.specimen.empty", "empty"),
        ("sc.pickers.specimen.loading", "loading"),
        ("sc.pickers.specimen.error", "error"),
        ("sc.pickers.specimen.overflow", "overflow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left before its capture\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_select(&wide_opt, "Width", "auto", &["48", "96"]);
    assert_select(&wide_opt, "Max rows", "12", &["6"]);
    assert_select(&wide_opt, "Placement", "center", &["below"]);
    assert_select(&wide_opt, "Fit", "default", &["flush", "third"]);
    assert_select(&wide_opt, "Backdrop", "default", &["full"]);
    assert_select(&wide_opt, "Cursor row", "fill", &["bar"]);
    assert_toggle(&wide_opt, "Delete closes a tab", "on");
    assert_select(&wide_opt, "Nav keys", "desktop", &["vim"]);
    assert_select(&wide_opt, "Dismiss", "light", &["escape"]);
    assert_select(&wide_opt, "No matches", "text", &["state"]);
    assert_select(&wide_opt, "Placeholder", "default", &["custom"]);
    assert_select(&wide_opt, "Hint row", "page", &["short"]);
    assert_select(&wide_opt, "Remote fetch", "works", &["fails"]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Open quickly"],
    );
    shot(&app, "sc.pickers.options.120x40");
    shot(&app, "sc.pickers.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Open quickly");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Open quickly",
            "width 64",
            "max rows 12",
            "cursor Fill",
            "nothing yet",
            "Safe Mode",
            "Query 1",
            "Loading, then Ready",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "width 96",
            "Loading, then Error",
        ],
    );
    assert_chosen(&wide_play, "nothing yet", &["schema.rs"]);
    shot(&app, "sc.pickers.playground.120x40");
    shot(&app, "sc.pickers.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s05-compare", 120, 40, PAGE_ARGS);
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
    shot(&app, "sc.pickers.compare-levels.120x40");

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
    shot(&app, "sc.pickers.compare-levels.80x24");

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
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_select(&options, "Width", "auto", &["48", "96"]);
    assert_select(&options, "Max rows", "12", &["6"]);
    assert_select(&options, "Placement", "center", &["below"]);
    assert_select(&options, "Fit", "default", &["flush", "third"]);
    see(
        &options,
        &["auto", "Page options", CAP_OPT, CRUMB],
        &["96", "Compare levels", OVERVIEW],
    );
    app.finish();
}

#[test]
fn pickers_open_choose_cancel_and_empty() {
    let app = open("s05-pickers", 120, 40, PAGE_ARGS);
    let screen = app.wait_text("Open quickly");
    assert_size(&app, 120, 40);
    assert_chosen(&screen, "nothing yet", &["schema.rs", "main", "Silent"]);
    assert_level(&screen, "Safe Mode");
    assert!(
        result_row(&screen, "Open tabs").contains("Query 1"),
        "tabs start without Query 1\n{}",
        text(&screen)
    );
    assert_last(&screen, "none yet", &[]);
    assert!(
        result_row(&screen, "Pickers opened").contains('0'),
        "opened count\n{}",
        text(&screen)
    );

    click_button(&app, "Choose a level");
    let level = app.wait_text("this connection");
    see(
        &level,
        &[
            "Safe Mode · this connection",
            "Silent",
            "Alert",
            "Read-Only",
            "Esc Keep",
        ],
        &["Files and tasks", "Type to search", "Add rate limiting"],
    );
    assert_chosen(&level, "nothing yet", &["Silent", "Read-Only"]);
    assert_level(&level, "Safe Mode");
    app.key("z");
    let typed = app.settle();
    see(
        &typed,
        &["Silent", "Read-Only", "Safe Mode (Full)", "this connection"],
        &["Files and tasks"],
    );
    assert_last(&typed, "none yet", &[]);
    assert!(
        result_row(&typed, "Last action").contains("ChosenAlt(i)"),
        "typing into a level picker emitted an action\n{}",
        text(&typed)
    );
    app.key("escape");
    let cancelled = app.wait_screen("level cancelled", |screen| {
        !text(screen).contains("this connection")
            && result_row(screen, "Last action").contains("Cancelled")
    });
    assert_chosen(
        &cancelled,
        "nothing yet",
        &["Silent", "Read-Only", "schema.rs"],
    );
    assert_level(&cancelled, "Safe Mode");
    assert_last(&cancelled, "Cancelled", &["none yet", "ChosenAlt(i)"]);
    see(
        &cancelled,
        &["Choose a level", "nothing yet"],
        &["this connection", "Chose Silent", "Chose Safe"],
    );

    click_button(&app, "Open quickly");
    app.wait_text("Files and tasks");
    flash_settle(&app);
    let opened = app.wait_text("All · Tab scope");
    see(
        &opened,
        &[
            "Open quickly",
            "Files and tasks",
            "All · Tab scope",
            "Cargo.toml",
        ],
        &["Chose schema.rs", "this connection"],
    );
    assert_chosen(&opened, "nothing yet", &["schema.rs"]);
    app.type_text("rate");
    let rated = app.wait_text(TASK_RATE);
    see(
        &rated,
        &[TASK_RATE, "All · Tab scope"],
        &["schema.rs", "Chose schema.rs", "Cargo.toml"],
    );
    assert_chosen(&rated, "nothing yet", &[TASK_RATE, "schema.rs"]);
    assert_last(&rated, "QueryChanged", &["none yet", "Chosen(0)"]);
    app.key("escape");
    let cleared = app.wait_text("Files and tasks");
    see(
        &cleared,
        &["Files and tasks", "Cargo.toml", "All · Tab scope"],
        &[TASK_RATE, "Chose schema.rs"],
    );
    assert_chosen(&cleared, "nothing yet", &["schema.rs"]);
    app.key("tab");
    let files = app.wait_text("Files · Tab scope");
    see(
        &files,
        &["Files · Tab scope", "Cargo.toml", "Open quickly"],
        &[TASK_RATE, "All · Tab scope"],
    );
    app.type_text("rate");
    let files_empty = app.wait_text(EMPTY_TEXT);
    see(
        &files_empty,
        &[EMPTY_TEXT, "Files · Tab scope", "Open quickly"],
        &[TASK_RATE, "schema.rs", "Chose schema.rs"],
    );
    assert_chosen(&files_empty, "nothing yet", &["schema.rs"]);
    app.key("escape");
    app.wait_text("Files and tasks");
    app.type_text("schema");
    let filtered = app.wait_text("schema.rs");
    assert_size(&app, 120, 40);
    see(
        &filtered,
        &[
            "schema.rs",
            "src/db/schema.rs",
            "Files · Tab scope",
            "Open quickly",
        ],
        &[TASK_RATE, "Cargo.toml", "README.md", "Chose schema.rs"],
    );
    assert_chosen(&filtered, "nothing yet", &["schema.rs"]);
    assert_level(&filtered, "Safe Mode");
    assert_last(&filtered, "QueryChanged", &["Chosen(0)", "none yet"]);
    shot(&app, "sc.pickers.pickers.before");

    app.key("enter");
    let chosen = app.wait_text("Chose schema.rs");
    assert_size(&app, 120, 40);
    see(
        &chosen,
        &[
            "Chose schema.rs",
            "schema.rs",
            "src/db/schema.rs",
            "Safe Mode",
        ],
        &["Files and tasks", "this connection"],
    );
    assert_chosen(&chosen, "schema.rs", &["nothing yet", "main", "Silent"]);
    assert!(
        result_row(&chosen, "Detail").contains("src/db/schema.rs"),
        "detail\n{}",
        text(&chosen)
    );
    assert_level(&chosen, "Safe Mode");
    assert_last(
        &chosen,
        "Chosen(0)",
        &["QueryChanged", "Cancelled", "Submit"],
    );
    assert!(
        result_row(&chosen, "Open tabs").contains("Query 1"),
        "choosing a file closed a tab\n{}",
        text(&chosen)
    );
    shot(&app, "sc.pickers.pickers.after");

    click_button(&app, "Open quickly");
    app.wait_text("Files and tasks");
    app.type_text("zzzz");
    let empty = app.wait_text(EMPTY_TEXT);
    see(
        &empty,
        &[EMPTY_TEXT, "Open quickly"],
        &["Cargo.toml", TASK_RATE, "Chose zzzz"],
    );
    assert_chosen(&empty, "schema.rs", &["nothing yet", "zzzz"]);
    app.key("enter");
    let submitted = app.wait_screen("empty submit keeps the previous file", |screen| {
        text(screen).contains(EMPTY_TEXT)
            && result_row(screen, "Last action").contains("Submit")
            && result_row(screen, "Chosen").contains("schema.rs")
    });
    assert_chosen(&submitted, "schema.rs", &["nothing yet", "zzzz"]);
    assert_level(&submitted, "Safe Mode");
    assert_last(&submitted, "Submit", &["Chosen(0)", "Cancelled"]);
    see(
        &submitted,
        &[EMPTY_TEXT, "Open quickly"],
        &["Chose zzzz", "nothing yet"],
    );
    app.key("escape");
    let cleared = app.wait_screen("empty query cleared", |screen| {
        !text(screen).contains("zzzz") && text(screen).contains("Files and tasks")
    });
    if text(&cleared).contains("Files · Tab scope") || text(&cleared).contains("All · Tab scope")
    {
        app.key("escape");
    }
    let closed = app.wait_screen("empty picker closed", |screen| {
        !text(screen).contains("Files · Tab scope")
            && !text(screen).contains("All · Tab scope")
            && !text(screen).contains("Files and tasks")
            && result_row(screen, "Chosen").contains("schema.rs")
    });
    assert_chosen(&closed, "schema.rs", &["nothing yet"]);
    assert_last(&closed, "Cancelled", &["Submit"]);

    click_button(&app, "Run command");
    let commands = app.wait_text("Commands\u{2026}");
    see(
        &commands,
        &[
            "Run command",
            "Export results\u{2026}",
            "Run a query first",
            "Commands\u{2026}",
        ],
        &["Chose Export", "Chose Format"],
    );
    assert_chosen(&commands, "schema.rs", &["Export", "Format SQL"]);
    click_disabled_export(&app);
    let refused = app.wait_screen("disabled command stays open", |screen| {
        text(screen).contains("Run command") && text(screen).contains("Run a query first")
    });
    see(
        &refused,
        &["Run command", "Run a query first", "Export results\u{2026}"],
        &["Chose Export", "Chose Format"],
    );
    assert_chosen(
        &refused,
        "schema.rs",
        &["Export", "Format SQL", "nothing yet"],
    );
    assert_level(&refused, "Safe Mode");
    app.key("escape");
    let commands_closed = app.wait_screen("commands cancelled", |screen| {
        !text(screen).contains("Commands\u{2026}")
            && result_row(screen, "Chosen").contains("schema.rs")
    });
    assert_chosen(
        &commands_closed,
        "schema.rs",
        &["Export", "Format SQL", "nothing yet"],
    );
    assert_last(&commands_closed, "Cancelled", &["Chosen(3)"]);
    see(
        &commands_closed,
        &["schema.rs"],
        &[
            "Chose Export",
            "Chose Format",
            "Commands\u{2026}",
            "Explain query",
        ],
    );

    click_button(&app, "Remote branch");
    let remote = app.wait_text("Fetching branches from origin");
    see(
        &remote,
        &["Remote branches · origin", "Fetching branches from origin"],
        &[
            "Could not reach origin",
            "feat/rate-limit",
            "release/4.2",
            "Chose main",
        ],
    );
    assert_chosen(&remote, "schema.rs", &["main", "nothing yet"]);
    app.key("enter");
    let entered = app.wait_text("Fetching branches from origin");
    see(
        &entered,
        &["Fetching branches from origin", "Remote branches · origin"],
        &["Could not reach origin", "Chose main", "feat/rate-limit"],
    );
    assert_chosen(&entered, "schema.rs", &["main"]);
    app.key("r");
    let retried = app.wait_text("Fetching branches from origin");
    see(
        &retried,
        &["Fetching branches from origin"],
        &["Could not reach origin", "Chose main", "feat/rate-limit"],
    );
    assert_chosen(&retried, "schema.rs", &["main", "release/4.2"]);
    app.key("escape");
    let remote_closed = app.wait_screen("remote cancelled", |screen| {
        !text(screen).contains("Fetching branches from origin")
            && result_row(screen, "Chosen").contains("schema.rs")
    });
    assert_chosen(&remote_closed, "schema.rs", &["main", "nothing yet"]);
    assert_level(&remote_closed, "Safe Mode");
    assert_last(&remote_closed, "Cancelled", &["Chosen(0)"]);
    see(
        &remote_closed,
        &["schema.rs", "Safe Mode"],
        &["Fetching branches", "Chose main", "feat/rate-limit"],
    );

    click_button(&app, "Choose a level");
    app.wait_text("this connection");
    app.key("escape");
    let kept = app.wait_screen("cancel keeps the file", |screen| {
        !text(screen).contains("this connection")
            && result_row(screen, "Chosen").contains("schema.rs")
            && result_row(screen, "Level").contains("Safe Mode")
    });
    assert_chosen(&kept, "schema.rs", &["Silent", "Read-Only", "nothing yet"]);
    assert_level(&kept, "Safe Mode");
    assert_last(&kept, "Cancelled", &["Chosen(0)"]);

    click_button(&app, "Switch tab");
    let tabs = app.wait_text("Filter tabs");
    see(
        &tabs,
        &["Open tabs", "Query 1", "2 live", "Filter tabs"],
        &["Closed Query 1", "1 live"],
    );
    app.key("delete");
    let deleted = app.wait_text("Closed Query 1");
    see(
        &deleted,
        &["Closed Query 1", "1 live", "orders", "Open tabs"],
        &["2 live"],
    );
    assert!(
        !result_row(&deleted, "Open tabs").contains("Query 1"),
        "delete left Query 1 in the tab list\n{}",
        text(&deleted)
    );
    assert_last(&deleted, "Secondary(0)", &["Chosen(0)"]);
    assert_chosen(&deleted, "schema.rs", &["nothing yet"]);
    app.key("escape");
    let tabs_closed = app.wait_screen("tab picker cancelled", |screen| {
        !text(screen).contains("Filter tabs") && result_row(screen, "Chosen").contains("schema.rs")
    });
    assert!(
        !result_row(&tabs_closed, "Open tabs").contains("Query 1"),
        "cancelling the tab picker restored Query 1\n{}",
        text(&tabs_closed)
    );
    assert_chosen(&tabs_closed, "schema.rs", &["nothing yet"]);
    assert_level(&tabs_closed, "Safe Mode");
    see(
        &tabs_closed,
        &["orders", "order_items", "History", "schema.rs"],
        &["Filter tabs", "Fetching branches"],
    );
    app.finish();
}

fn finish_option(
    app: App,
    case_id: &str,
    skip: &[&str],
    present: &str,
    absent: &str,
    check: impl Fn(&TerminalSnapshot),
) {
    confirm_control(&app, check);
    flash_settle(&app);
    let screen = app.wait_text(present);
    assert_readout(&screen, skip, present, absent);
    assert_size(&app, 120, 40);
    shot(&app, case_id);
    app.finish();
}

fn confirm_control(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("Open quickly");
}

fn option_case(
    name: &str,
    case_id: &str,
    opt: &str,
    skip: &[&str],
    present: &str,
    absent: &str,
    check: impl Fn(&TerminalSnapshot),
) {
    let app = page(name, Some(opt), 120, 40);
    let screen = app.wait_text("Open quickly");
    assert_readout(&screen, skip, present, absent);
    finish_option(app, case_id, skip, present, absent, check);
}

#[test]
fn option_width_96() {
    option_case(
        "s05-width",
        "opt.pickers.width",
        "pickers.width=96",
        &["width"],
        "width 96",
        "width 64",
        |screen| {
            assert_select(screen, "Width", "96", &["auto", "48"]);
            assert_select(screen, "Max rows", "12", &["6"]);
        },
    );
}

#[test]
fn option_max_rows_6() {
    option_case(
        "s05-rows",
        "opt.pickers.max_rows",
        "pickers.max_rows=6",
        &["rows", "place"],
        "max rows 6",
        "max rows 12",
        |screen| {
            assert_select(screen, "Max rows", "6", &["12"]);
            assert_select(screen, "Width", "auto", &["48", "96"]);
        },
    );
}

#[test]
fn option_placement_below() {
    option_case(
        "s05-place",
        "opt.pickers.placement",
        "pickers.placement=below",
        &["place"],
        "Below the button",
        "max rows 12 · Center",
        |screen| {
            assert_select(screen, "Placement", "below", &["center"]);
            assert_select(screen, "Fit", "default", &["flush", "third"]);
        },
    );
}

#[test]
fn option_fit_flush() {
    option_case(
        "s05-fit",
        "opt.pickers.fit",
        "pickers.fit=flush",
        &["fit"],
        "margin 0",
        "margin 1",
        |screen| {
            assert_select(screen, "Fit", "flush", &["default", "third"]);
            assert_select(screen, "Backdrop", "default", &["full"]);
        },
    );
}

#[test]
fn option_backdrop_full() {
    option_case(
        "s05-backdrop",
        "opt.pickers.backdrop",
        "pickers.backdrop=full",
        &["back"],
        "keeps 0 bottom",
        "keeps 1 bottom",
        |screen| {
            assert_select(screen, "Backdrop", "full", &["default"]);
            assert_select(screen, "Fit", "default", &["flush", "third"]);
        },
    );
}

#[test]
fn option_cursor_bar() {
    option_case(
        "s05-cursor",
        "opt.pickers.cursor_style",
        "pickers.cursor_style=bar",
        &["cursor"],
        "cursor Bar",
        "cursor Fill",
        |screen| {
            assert_select(screen, "Cursor row", "bar", &["fill"]);
            assert_select(screen, "Nav keys", "desktop", &["vim"]);
        },
    );
}

#[test]
fn option_delete_is_secondary_off() {
    let app = page(
        "s05-delete",
        Some("pickers.delete_is_secondary=off"),
        120,
        40,
    );
    let screen = app.wait_text("Open quickly");
    assert_readout(
        &screen,
        &["delete"],
        "Delete secondary off",
        "Delete secondary on",
    );
    click_button(&app, "Switch tab");
    let tabs = app.wait_text("Filter tabs");
    see(
        &tabs,
        &["Open tabs", "Query 1", "2 live", "Filter tabs"],
        &["Closed Query 1", "1 live"],
    );
    app.key("delete");
    let stayed = app.settle();
    see(
        &stayed,
        &["Query 1", "2 live", "Filter tabs"],
        &["Closed Query 1", "1 live", "Secondary(0)"],
    );
    assert!(
        result_row(&stayed, "Open tabs").contains("Query 1"),
        "delete closed a tab while secondary delete is off\n{}",
        text(&stayed)
    );
    app.key("escape");
    let closed = app.wait_screen("tabs closed with Query 1", |screen| {
        !text(screen).contains("Filter tabs") && result_row(screen, "Open tabs").contains("Query 1")
    });
    assert!(
        result_row(&closed, "Open tabs").contains("Query 1"),
        "leaving the tab picker dropped Query 1\n{}",
        text(&closed)
    );
    finish_option(
        app,
        "opt.pickers.delete_is_secondary",
        &["delete"],
        "Delete secondary off",
        "Delete secondary on",
        |screen| {
            assert_toggle(screen, "Delete closes a tab", "off");
            assert_select(screen, "Nav keys", "desktop", &["vim"]);
        },
    );
}

#[test]
fn option_nav_vim() {
    option_case(
        "s05-nav",
        "opt.pickers.nav",
        "pickers.nav=vim",
        &["nav"],
        "nav vim",
        "nav desktop",
        |screen| {
            assert_select(screen, "Nav keys", "vim", &["desktop"]);
            assert_select(screen, "Dismiss", "light", &["escape"]);
        },
    );
}

#[test]
fn option_dismiss_escape() {
    let app = page("s05-dismiss", Some("pickers.dismiss=escape"), 120, 40);
    let screen = app.wait_text("Open quickly");
    assert_readout(
        &screen,
        &["dismiss"],
        "outside click off",
        "outside click on",
    );
    click_button(&app, "Open quickly");
    app.wait_text("Files and tasks");
    app.click(Point { x: 100, y: 36 });
    let stayed = app.wait_screen("outside click keeps the picker", |screen| {
        text(screen).contains("Files and tasks") && text(screen).contains("All · Tab scope")
    });
    see(
        &stayed,
        &["Files and tasks", "All · Tab scope", "Open quickly"],
        &["Chose schema.rs"],
    );
    app.key("escape");
    let closed = app.wait_screen("escape closes the picker", |screen| {
        !text(screen).contains("Files and tasks")
    });
    assert_chosen(&closed, "nothing yet", &["schema.rs"]);
    assert_last(&closed, "Cancelled", &["Chosen(0)"]);
    finish_option(
        app,
        "opt.pickers.dismiss",
        &["dismiss"],
        "outside click off",
        "outside click on",
        |screen| {
            assert_select(screen, "Dismiss", "escape", &["light"]);
            assert_select(screen, "Width", "auto", &["48", "96"]);
        },
    );
}

#[test]
fn option_empty_state() {
    let app = page("s05-empty", Some("pickers.empty=state"), 120, 40);
    let screen = app.wait_text("Nothing matches");
    assert_readout(&screen, &["empty"], "Nothing matches", EMPTY_TEXT);
    click_button(&app, "Open quickly");
    app.wait_text("All · Tab scope");
    app.type_text("zzzz");
    let empty = app.wait_text("Nothing matches");
    see(
        &empty,
        &["Nothing matches", "Open quickly", "All · Tab scope"],
        &[EMPTY_TEXT, "schema.rs", "Chose schema.rs", "Cargo.toml"],
    );
    assert_chosen(&empty, "nothing yet", &["zzzz", "schema.rs"]);
    app.key("enter");
    let submitted = app.wait_screen("empty state does not choose", |screen| {
        text(screen).contains("Nothing matches")
            && text(screen).contains("All · Tab scope")
            && result_row(screen, "Last action").contains("Submit")
            && result_row(screen, "Chosen").contains("nothing yet")
    });
    assert_chosen(&submitted, "nothing yet", &["zzzz", "schema.rs"]);
    assert_last(&submitted, "Submit", &["Chosen(0)"]);
    see(
        &submitted,
        &["Nothing matches", "Open quickly", "All · Tab scope"],
        &[EMPTY_TEXT, "Chose schema.rs", "Cargo.toml"],
    );
    app.key("escape");
    let cleared = app.wait_screen("empty query cleared", |screen| {
        !text(screen).contains("zzzz")
    });
    if text(&cleared).contains("All · Tab scope") {
        app.key("escape");
    }
    app.wait_screen("empty state picker closed", |screen| {
        !text(screen).contains("All · Tab scope")
    });
    finish_option(
        app,
        "opt.pickers.empty",
        &["empty"],
        "Nothing matches",
        EMPTY_TEXT,
        |screen| {
            assert_select(screen, "No matches", "state", &["text"]);
            assert_select(screen, "Placeholder", "default", &["custom"]);
        },
    );
}

#[test]
fn option_placeholder_custom() {
    let app = page(
        "s05-placeholder",
        Some("pickers.placeholder=custom"),
        120,
        40,
    );
    let screen = app.wait_text("Type to narrow");
    assert_readout(
        &screen,
        &["holder"],
        "Type to narrow\u{2026}",
        "Type to search\u{2026}",
    );
    click_button(&app, "Open quickly");
    let picker = app.wait_text("All · Tab scope");
    see(
        &picker,
        &["Type to narrow\u{2026}", "All · Tab scope", "Open quickly"],
        &["Type to search\u{2026}", "Files and tasks"],
    );
    app.key("escape");
    app.wait_screen("placeholder picker closed", |screen| {
        !text(screen).contains("All · Tab scope")
    });
    finish_option(
        app,
        "opt.pickers.placeholder",
        &["holder"],
        "Type to narrow\u{2026}",
        "Type to search\u{2026}",
        |screen| {
            assert_select(screen, "Placeholder", "custom", &["default"]);
            assert_select(screen, "No matches", "text", &["state"]);
        },
    );
}

#[test]
fn option_hints_short() {
    let app = page("s05-hints", Some("pickers.hints=short"), 120, 40);
    let screen = app.wait_text("Enter Choose");
    assert_readout(
        &screen,
        &["hints"],
        "Enter Choose · Esc Close",
        "per picker",
    );
    click_button(&app, "Open quickly");
    let picker = app.wait_text("All · Tab scope");
    see(
        &picker,
        &[
            "Enter Choose",
            "Esc Close",
            "Files and tasks",
            "All · Tab scope",
        ],
        &["Alt+Enter", "per picker"],
    );
    app.key("escape");
    app.wait_screen("hints picker closed", |screen| {
        !text(screen).contains("All · Tab scope")
    });
    finish_option(
        app,
        "opt.pickers.hints",
        &["hints"],
        "Enter Choose · Esc Close",
        "per picker",
        |screen| {
            assert_select(screen, "Hint row", "short", &["page"]);
            assert_select(screen, "Placeholder", "default", &["custom"]);
        },
    );
}

#[test]
fn option_remote_fails() {
    let app = page("s05-remote", Some("pickers.remote=fails"), 120, 40);
    let screen = app.wait_text("Loading, then Error");
    assert_readout(
        &screen,
        &["remote"],
        "Loading, then Error (r retries)",
        "Loading, then Ready",
    );
    click_button(&app, "Remote branch");
    let remote = app.wait_text("Fetching branches from origin");
    see(
        &remote,
        &["Fetching branches from origin", "Remote branches · origin"],
        &["Could not reach origin", "feat/rate-limit", "Chose main"],
    );
    assert_chosen(&remote, "nothing yet", &["main"]);
    app.key("r");
    let retried = app.settle();
    see(
        &retried,
        &["Fetching branches from origin"],
        &["Could not reach origin", "Chose main"],
    );
    app.key("escape");
    let closed = app.wait_screen("failed remote cancelled", |screen| {
        !text(screen).contains("Fetching branches from origin")
            && result_row(screen, "Chosen").contains("nothing yet")
    });
    assert_chosen(&closed, "nothing yet", &["main", "schema.rs"]);
    assert_last(&closed, "Cancelled", &["Chosen(0)"]);
    finish_option(
        app,
        "opt.pickers.remote",
        &["remote"],
        "Loading, then Error (r retries)",
        "Loading, then Ready",
        |screen| {
            assert_select(screen, "Remote fetch", "fails", &["works"]);
            assert_select(screen, "Width", "auto", &["48", "96"]);
        },
    );
}

/// The test terminal keeps only the first scalars of a ZWJ or skin-tone
/// sequence, so those labels are matched by their visible head.
const INTERNATIONAL_LABELS: &[&str] = &[
    "cafe\u{301}.md",
    "界A.md",
    "שלום.md",
    "Review 👨\u{200d}",
    "Review 🇯🇵",
    "Review 👍",
];

fn emphasis(screen: &TerminalSnapshot, label: &str, count: usize) -> Vec<(String, u16, bool)> {
    let at = find_all(screen, label)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    clusters(screen, at, count)
        .into_iter()
        .map(|cluster| (cluster.text, cluster.width, cluster.bold))
        .collect()
}

fn cluster(text: &str, width: u16, bold: bool) -> (String, u16, bool) {
    (text.to_string(), width, bold)
}

#[test]
fn option_international_labels_paint_whole_clusters() {
    let app = page("s05-intl", Some("pickers.international=on"), 120, 40);
    app.wait_text("Open quickly");
    confirm_control(&app, |screen| {
        assert_toggle(screen, "International labels", "on");
        assert_select(screen, "Width", "auto", &["48", "96"]);
    });
    click_button(&app, "Open quickly");
    let opened = app.wait_text("All · Tab scope");
    see(&opened, INTERNATIONAL_LABELS, &["Cargo.toml", TASK_RATE]);
    for (label, detail) in [
        ("cafe\u{301}.md", "combining mark"),
        ("界A.md", "wide + ASCII"),
        ("שלום.md", "right-to-left"),
        ("Review 👨\u{200d}", "ZWJ family"),
        ("Review 🇯🇵", "flag"),
        ("Review 👍", "skin tone"),
    ] {
        let line = row_with(&opened, label);
        assert!(
            line.contains(detail),
            "the detail {detail:?} was overwritten or pushed off the {label:?} row: {line:?}"
        );
    }

    app.type_text("a");
    let narrowed = app.wait_screen("a narrows the labels", |screen| {
        !text(screen).contains("Review") && text(screen).contains("界A.md")
    });
    assert_eq!(
        emphasis(&narrowed, "界A.md", 5),
        [
            cluster("界", 2, false),
            cluster("A", 1, true),
            cluster(".", 1, false),
            cluster("m", 1, false),
            cluster("d", 1, false),
        ],
        "the A after a wide cluster is painted in the cell after it\n{}",
        text(&narrowed)
    );

    app.key("backspace");
    app.type_text("e");
    app.wait_screen(
        "e narrows to the combining label and the reviews",
        |screen| !text(screen).contains("界A.md") && text(screen).contains("Review"),
    );
    app.key("down");
    let combined = app.settle();
    assert_eq!(
        emphasis(&combined, "cafe\u{301}.md", 7),
        [
            cluster("c", 1, false),
            cluster("a", 1, false),
            cluster("f", 1, false),
            cluster("e\u{301}", 1, true),
            cluster(".", 1, false),
            cluster("m", 1, false),
            cluster("d", 1, false),
        ],
        "a match on the base letter emphasises the whole combining cluster\n{}",
        text(&combined)
    );
    app.key("escape");
    app.key("escape");
    app.wait_screen("international picker closed", |screen| {
        !text(screen).contains("All · Tab scope")
    });
    app.finish();
}

fn row_limit_page(name: &str, rows: &str, remote: &str, paused: bool) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = [
        "--page",
        "pickers",
        "--opt",
        &format!("pickers.max_rows={rows}"),
        "--opt",
        &format!("pickers.remote={remote}"),
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect();
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    if paused {
        launch.motion = Some("paused".to_string());
    }
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Open quickly");
    app
}

fn picker_status_rows(screen: &TerminalSnapshot, label: &str) -> usize {
    (0..screen.rows())
        .filter(|row| row_text(screen, *row).contains(label))
        .count()
}

#[test]
fn option_row_limits_show_one_status_row_in_every_state() {
    for rows in ["1", "0"] {
        let app = row_limit_page(&format!("s05-rows-{rows}-loading"), rows, "stalls", true);
        click_button(&app, "Remote branch");
        let loading = app.wait_text("Fetching branches from origin");
        assert_eq!(
            picker_status_rows(&loading, "Fetching branches from origin"),
            1,
            "max rows {rows}: Loading owns one row\n{}",
            text(&loading)
        );
        see(
            &loading,
            &["Remote branches \u{b7} origin"],
            &["Could not reach origin", "feat/rate-limit"],
        );
        app.key("escape");
        app.finish();

        let app = row_limit_page(&format!("s05-rows-{rows}-error"), rows, "fails", false);
        click_button(&app, "Remote branch");
        let error = app.wait_text("Could not reach origin");
        assert_eq!(
            picker_status_rows(&error, "Could not reach origin"),
            1,
            "max rows {rows}: Error owns one row\n{}",
            text(&error)
        );
        see(
            &error,
            &["Remote branches \u{b7} origin"],
            &["Fetching branches from origin", "feat/rate-limit"],
        );
        app.key("escape");
        app.finish();

        let app = row_limit_page(&format!("s05-rows-{rows}-ready"), rows, "works", false);
        click_button(&app, "Remote branch");
        let ready = app.wait_screen("remote branches ready", |screen| {
            text(screen).contains("main") && !text(screen).contains("Fetching branches")
        });
        see(
            &ready,
            &["Remote branches \u{b7} origin", "main"],
            &["Fetching branches from origin", "Could not reach origin"],
        );
        app.key("escape");
        app.finish();
    }
}

#[test]
fn option_remote_stalls_stays_loading() {
    let app = page("s05-remote-stalls", Some("pickers.remote=stalls"), 120, 40);
    let screen = app.wait_text("and it stays Loading");
    assert_readout(
        &screen,
        &["remote"],
        "Loading, and it stays Loading",
        "Loading, then Ready",
    );
    click_button(&app, "Remote branch");
    let loading = app.wait_text("Fetching branches from origin");
    see(
        &loading,
        &["Fetching branches from origin"],
        &["Could not reach origin", "feat/rate-limit"],
    );
    app.key("escape");
    app.finish();
}
