//! Empty & error route, sections, specimens, compare levels, empty actions, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s06_emptyerror/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Feedback › Empty & error";
const TITLE: &str = "Empty & error";
const BLURB: &str = "Empty and failed states with a way out, and read-only surfaces that say why";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "emptyerror"];
const HINT: &str = "A connection holds a host, a user and a database";
const ERROR_HINT: &str = "The config file is unreadable: check its permissions";
const VIEW_REASON: &str = "orders_view is a view: views cannot be edited";
const NO_KEY_REASON: &str = "orders_view has no primary key, so rows cannot be told apart";
const IMPORT: &str = "Import\u{2026}";
const DASH: char = '\u{2013}';

const DRAWN: &[(&str, &str)] = &[
    ("sc.emptyerror.specimen.rest", "rest"),
    ("sc.emptyerror.specimen.hover", "hover"),
    ("sc.emptyerror.specimen.pressed", "pressed"),
    ("sc.emptyerror.specimen.focused", "focused"),
    ("sc.emptyerror.specimen.error", "error"),
    ("sc.emptyerror.specimen.overflow", "overflow"),
    ("sc.emptyerror.specimen.narrow", "narrow"),
];

const OMITTED: &[&str] = &[
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "editing",
    "empty",
    "loading",
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s06_emptyerror/{}", case_id.replace('.', "_"))
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

fn hint_state(screen: &TerminalSnapshot) -> String {
    row_with(screen, "Hint")
        .split_whitespace()
        .skip_while(|token| *token != "Hint")
        .nth(1)
        .unwrap_or("")
        .to_string()
}

fn assert_hint(screen: &TerminalSnapshot, state: &str) {
    let got = hint_state(screen);
    assert_eq!(
        got,
        state,
        "Hint row is {:?}, expected {state}",
        row_with(screen, "Hint")
    );
}

fn assert_option_defaults(
    screen: &TerminalSnapshot,
    kind: &str,
    hint: &str,
    actions: &str,
    reason: &str,
) {
    let kinds = if kind == "empty" {
        &["error"][..]
    } else {
        &["empty"][..]
    };
    let actions_other = if actions == "two" {
        &["none", "one"][..]
    } else {
        &["two", "one"][..]
    };
    let reasons = if reason == "view" {
        &["no-key", "result"][..]
    } else {
        &["view", "result"][..]
    };
    assert_select(screen, "Kind", kind, kinds);
    assert_hint(screen, hint);
    assert_select(screen, "Actions", actions, actions_other);
    assert_select(screen, "Read-only reason", reason, reasons);
}

/// Row labels sit under the legend. The legend contains "focused", and the
/// page title contains "error", so a whole-screen search is not the specimen.
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

/// Band names from row 8 downward. At 120 the header chip already says
/// "truecolor", so the top band is not a whole-screen search.
fn body_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    (8..screen.rows()).any(|row| row_text(screen, row).contains(needle))
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

fn assert_orders(screen: &TerminalSnapshot) {
    see(
        screen,
        &["1041", "Acme Corp", "1204.50", "1042", "Globex", "88.00"],
        &["Copied"],
    );
}

fn reveal_no_color(app: &App) -> (bool, bool) {
    let point = Point { x: 60, y: 18 };
    let mut saw_256 = false;
    let mut saw_16 = false;
    for _ in 0..50 {
        let screen = app.settle();
        saw_256 |= body_has(&screen, "256 colors");
        saw_16 |= body_has(&screen, "16 colors");
        if body_has(&screen, "no color")
            && !body_has(&screen, "256 colors")
            && !body_has(&screen, "16 colors")
        {
            return (saw_256, saw_16);
        }
        app.wheel(point, true, 4);
    }
    panic!("no color band missing\n{}", text(&app.screen()));
}

fn reveal_truecolor_band(app: &App) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..50 {
        let screen = app.settle();
        let visible = text(&screen);
        if body_has(&screen, "truecolor")
            && !visible.contains("no color")
            && !visible.contains("256 colors")
            && !visible.contains("16 colors")
            && visible.contains("● Compare levels")
            && grid_label(&screen, "rest")
        {
            return;
        }
        app.wheel(point, false, 4);
    }
    panic!("truecolor band did not return\n{}", text(&app.screen()));
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s06-route-page", 120, 40, PAGE_ARGS);
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
            "Saved queries",
            "No saved queries",
            "Queries you save appear here",
            "Nothing matches",
            "No connections yet",
            HINT,
            "New connection",
            IMPORT,
            "1041",
            "Acme Corp",
            "1204.50",
            "orders_view",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "Could not load connections",
            "Retry",
            "chosen",
        ],
    );
    shot(&app, "sc.emptyerror.route");
    app.finish();

    let app = open("s06-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("No connections yet");
    see(
        &entered,
        &[CRUMB, BLURB, "New connection", "No saved queries"],
        &[OVERVIEW, "Page options", "Feedback › Progress"],
    );
    app.key("[");
    let left = app.wait_text("Feedback › Progress");
    see(
        &left,
        &["Feedback › Progress"],
        &[CRUMB, "No connections yet", BLURB],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "New connection", HINT],
        &["Feedback › Progress", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(
        &overview,
        &[OVERVIEW],
        &[CRUMB, "No connections yet", BLURB],
    );
    click_side(&app, "Feedback");
    app.wait_text("Progress");
    click_side(&app, "Empty & error");
    let pointed = app.wait_text("No connections yet");
    see(
        &pointed,
        &[CRUMB, BLURB, "New connection", "No saved queries", IMPORT],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s06-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("No saved queries");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "with a way out",
            "Playground",
            "Saved queries",
            "No saved queries",
            "Queries you save appear here",
            "Nothing",
            "No connections yet",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "say why",
            "New connection",
            IMPORT,
            "holds a host",
        ],
    );
    shot(&app, "sc.emptyerror.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_option_defaults(&narrow_opt, "empty", "on", "two", "view");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Kind", "Hint", "Actions"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "No saved queries",
            WIDE,
            "truecolor",
            "Could not load connections",
        ],
    );
    shot(&app, "sc.emptyerror.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert!(
        grid_label(&narrow_states, "rest")
            && grid_label(&narrow_states, "hover")
            && grid_label(&narrow_states, "pressed"),
        "narrow states clipped a top row\n{}",
        text(&narrow_states)
    );
    for label in ["focused", "error", "overflow", "narrow"] {
        assert!(
            !grid_label(&narrow_states, label),
            "narrow states already shows {label}\n{}",
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
            "pressed",
            "No rows",
            "depicts a focused sample",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "With actions",
            "focus + hover",
            "where your focus is",
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    shot(&app, "sc.emptyerror.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "pressed")
            && grid_label(screen, "focused")
            && grid_label(screen, "error")
            && grid_label(screen, "overflow")
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
            "Title + hint",
            "With actions",
            "No connections",
            "New",
            "! No rows",
            "in a view",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "256 colors",
            "no color",
        ],
    );
    assert!(
        grid_row(&wide_states, "hover").contains(DASH)
            && grid_row(&wide_states, "pressed").contains(DASH)
            && grid_row(&wide_states, "focused").contains(DASH),
        "an actionless column drew hover, pressed, or focused\n{}",
        text(&wide_states)
    );
    assert!(
        grid_row(&wide_states, "error").contains('!'),
        "error row is not the error empty state\n{}",
        text(&wide_states)
    );
    assert!(
        grid_row(&wide_states, "overflow").contains("in a view"),
        "overflow row is not the long title\n{}",
        text(&wide_states)
    );
    for label in OMITTED {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in DRAWN {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.emptyerror.states.120x40");
    shot(&app, "sc.emptyerror.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_option_defaults(&wide_opt, "empty", "on", "two", "view");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Read-only reason"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "No saved queries"],
    );
    shot(&app, "sc.emptyerror.options.120x40");
    shot(&app, "sc.emptyerror.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("No connections yet");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "No saved queries",
            "Queries you save appear here",
            "Nothing matches",
            "No connections yet",
            HINT,
            "New connection",
            IMPORT,
            "1041",
            "Acme Corp",
            "1204.50",
            "1042",
            "Globex",
            "88.00",
            "Enter or a typed key on a cell says why it cannot change",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Could not load connections",
            "Retry",
            "chosen",
        ],
    );
    shot(&app, "sc.emptyerror.playground.120x40");
    shot(&app, "sc.emptyerror.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s06-compare", 120, 40, PAGE_ARGS);
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
    assert!(
        body_has(&on, "truecolor"),
        "the top band is not truecolor\n{}",
        text(&on)
    );
    let (saw_256, saw_16) = reveal_no_color(&app);
    let bands = app.settle();
    assert_size(&app, 120, 40);
    see(
        &bands,
        &[CRUMB, "no color", WIDE, "rest"],
        &[
            "Page options",
            CAP_PLAY,
            "○ Compare levels",
            "256 colors",
            "16 colors",
        ],
    );
    assert!(
        saw_256 && saw_16,
        "a colour band was skipped before no color\n{}",
        text(&bands)
    );
    shot(&app, "sc.emptyerror.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    reveal_no_color(&app);
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color", "rest"],
        &[
            "Page options",
            CAP_PLAY,
            WIDE,
            "○ Compare levels",
            "256 colors",
            "16 colors",
            "truecolor",
        ],
    );
    shot(&app, "sc.emptyerror.compare-levels.80x24");

    reveal_truecolor_band(&app);
    let top = app.settle();
    assert_eq!((top.cols(), top.rows()), (80, 24));
    see(
        &top,
        &[TITLE, "truecolor", "● Compare levels", "rest"],
        &["no color", "256 colors", "16 colors", "○ Compare levels"],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let restored = app.settle();
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest"],
        &["● Compare levels", "256 colors", "16 colors", "no color"],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_option_defaults(&options, "empty", "on", "two", "view");
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, WIDE],
        &["Compare levels", OVERVIEW, "Could not load connections"],
    );
    app.finish();
}

#[test]
fn empty_state_activates_and_refuses_edits() {
    let app = open("s06-empty", 120, 40, PAGE_ARGS);
    let before = app.wait_text("No connections yet");
    assert_size(&app, 120, 40);
    see(
        &before,
        &[
            "No connections yet",
            HINT,
            "New connection",
            IMPORT,
            "1041",
            "Acme Corp",
        ],
        &[
            "chosen",
            VIEW_REASON,
            "Could not load connections",
            "Copied",
        ],
    );
    assert_orders(&before);
    shot(&app, "sc.emptyerror.empty.before");

    click_button(&app, "New connection");
    app.wait_text("New connection chosen");
    flash_settle(&app);
    let chosen = app.wait_text("New connection chosen");
    see(
        &chosen,
        &["New connection chosen", "No connections yet", HINT, IMPORT],
        &[VIEW_REASON, "Could not load connections", "Copied"],
    );
    assert_orders(&chosen);

    click_button(&app, IMPORT);
    let imported = app.wait_text("Import\u{2026} chosen");
    see(
        &imported,
        &[
            "Import\u{2026} chosen",
            "No connections yet",
            "New connection",
        ],
        &["New connection chosen", VIEW_REASON, "Copied"],
    );

    click_where(&app, "1041", |hit| hit.y > 24);
    app.key("enter");
    let refused = app.wait_text(VIEW_REASON);
    see(
        &refused,
        &[
            VIEW_REASON,
            "1041",
            "Acme Corp",
            "1204.50",
            "No connections yet",
        ],
        &["Copied", NO_KEY_REASON, "Could not load connections"],
    );
    assert_orders(&refused);
    app.key("x");
    let typed = app.settle();
    see(
        &typed,
        &[VIEW_REASON, "1041", "Acme Corp", "1204.50", "88.00"],
        &["Copied", NO_KEY_REASON],
    );

    click_button(&app, "Row actions");
    let menu = app.wait_text("Edit cell");
    see(
        &menu,
        &[
            "Edit cell",
            "Duplicate row",
            "Delete row",
            "Copy row",
            "Copy as CSV",
            "Enter Choose",
            VIEW_REASON,
        ],
        &["Copied"],
    );
    // Opens on Copy row. Three ups reach disabled Edit cell. The page gutter
    // is hidden, and the menu caption shows that item's short reason.
    for _ in 0..3 {
        app.key("up");
    }
    let on_edit = app.wait_text("read-only vi");
    see(
        &on_edit,
        &["Edit cell", "read-only vi", VIEW_REASON],
        &["Copied"],
    );
    app.key("enter");
    let stayed = app.settle();
    see(
        &stayed,
        &[
            VIEW_REASON,
            "Edit cell",
            "Copy row",
            "read-only vi",
            "1041",
            "Acme Corp",
            "1204.50",
        ],
        &["Copied"],
    );
    app.key("escape");
    let closed = app.wait_screen("row menu closed", |screen| {
        !text(screen).contains("Copy as CSV") && text(screen).contains(VIEW_REASON)
    });
    see(
        &closed,
        &[VIEW_REASON, "1041", "Acme Corp", "No connections yet"],
        &["Copied", "Copy as CSV", "Edit cell"],
    );

    app.resize(80, 24);
    app.wait_screen("narrow size", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    let point = Point { x: 40, y: 12 };
    for _ in 0..30 {
        let screen = app.settle();
        let visible = text(&screen);
        if visible.contains("No connections yet")
            && visible.contains("Nothing matches")
            && !visible.contains("New connection")
            && !visible.contains(IMPORT)
        {
            break;
        }
        app.wheel(point, false, 4);
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &["No connections yet", "Nothing matches"],
        &["New connection", IMPORT, WIDE],
    );

    app.resize(120, 40);
    let after = app.wait_screen("actions restored at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains("New connection")
            && text(screen).contains(VIEW_REASON)
            && text(screen).contains("No connections yet")
    });
    assert_size(&app, 120, 40);
    see(
        &after,
        &[
            "New connection",
            IMPORT,
            "No connections yet",
            VIEW_REASON,
            "1041",
            "Acme Corp",
            "1204.50",
            WIDE,
        ],
        &[
            "Copied",
            "Copy as CSV",
            NO_KEY_REASON,
            "Could not load connections",
        ],
    );
    assert_orders(&after);
    shot(&app, "sc.emptyerror.empty.after");
    app.finish();
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("No saved queries");
}

fn finish_playground(app: App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    flash_settle(&app);
    let screen = app.wait_text("No saved queries");
    check(&screen);
    assert_size(&app, 120, 40);
    shot(&app, case_id);
    app.finish();
}

#[test]
fn option_kind_error() {
    let app = page("s06-kind", Some("emptyerror.kind=error"), 120, 40);
    let screen = app.wait_text("Could not load connections");
    see(
        &screen,
        &[
            "Could not load connections",
            ERROR_HINT,
            "Retry",
            IMPORT,
            "No saved queries",
        ],
        &["No connections yet", "New connection", HINT, "Retry chosen"],
    );
    click_button(&app, "Retry");
    let retried = app.wait_text("Retry chosen");
    see(
        &retried,
        &[
            "Retry chosen",
            "Could not load connections",
            IMPORT,
            ERROR_HINT,
        ],
        &["No connections yet", "New connection", HINT],
    );
    confirm_options(&app, |screen| {
        assert_option_defaults(screen, "error", "on", "two", "view");
        see(
            screen,
            &["Retry chosen", "Page options"],
            &["No connections yet", "New connection"],
        );
    });
    finish_playground(app, "opt.emptyerror.kind", |screen| {
        see(
            screen,
            &[
                "Retry chosen",
                "Could not load connections",
                ERROR_HINT,
                "Retry",
                IMPORT,
            ],
            &["No connections yet", "New connection", HINT],
        );
    });
}

#[test]
fn option_hint_off() {
    let app = page("s06-hint", Some("emptyerror.hint=off"), 120, 40);
    let screen = app.wait_text("No connections yet");
    see(
        &screen,
        &[
            "No connections yet",
            "New connection",
            IMPORT,
            "No saved queries",
        ],
        &[HINT, ERROR_HINT, "Could not load connections", "Retry"],
    );
    confirm_options(&app, |screen| {
        assert_option_defaults(screen, "empty", "off", "two", "view");
    });
    finish_playground(app, "opt.emptyerror.hint", |screen| {
        see(
            screen,
            &["No connections yet", "New connection", IMPORT],
            &[HINT, ERROR_HINT, "Could not load connections"],
        );
    });
}

#[test]
fn option_actions_none() {
    let app = page("s06-actions", Some("emptyerror.actions=none"), 120, 40);
    let screen = app.wait_text("No connections yet");
    see(
        &screen,
        &["No connections yet", HINT, "No saved queries", "1041"],
        &[
            "New connection",
            IMPORT,
            "Retry",
            "Could not load connections",
        ],
    );
    confirm_options(&app, |screen| {
        assert_option_defaults(screen, "empty", "on", "none", "view");
    });
    finish_playground(app, "opt.emptyerror.actions", |screen| {
        see(
            screen,
            &["No connections yet", HINT],
            &["New connection", IMPORT, "Retry"],
        );
    });
}

#[test]
fn option_reason_no_key() {
    let app = page("s06-reason", Some("emptyerror.reason=no-key"), 120, 40);
    let screen = app.wait_text("No connections yet");
    see(
        &screen,
        &["No connections yet", "New connection", "1041", "Acme Corp"],
        &[NO_KEY_REASON, VIEW_REASON, "Copied"],
    );
    click_where(&app, "1041", |hit| hit.y > 24);
    app.key("enter");
    let refused = app.wait_text(NO_KEY_REASON);
    see(
        &refused,
        &[
            NO_KEY_REASON,
            "1041",
            "Acme Corp",
            "1204.50",
            "No connections yet",
        ],
        &[VIEW_REASON, "Copied", "A query result is read-only"],
    );
    assert_orders(&refused);
    app.key("x");
    let typed = app.settle();
    see(
        &typed,
        &[NO_KEY_REASON, "1041", "Acme Corp", "88.00"],
        &[VIEW_REASON, "Copied"],
    );
    confirm_options(&app, |screen| {
        assert_option_defaults(screen, "empty", "on", "two", "no-key");
        see(screen, &[NO_KEY_REASON, "Page options"], &[VIEW_REASON]);
    });
    finish_playground(app, "opt.emptyerror.reason", |screen| {
        see(
            screen,
            &[
                NO_KEY_REASON,
                "1041",
                "Acme Corp",
                "1204.50",
                "No connections yet",
            ],
            &[VIEW_REASON, "Copied"],
        );
    });
}
