//! Master-detail route, sections, specimens, seam drag, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s07_masterdetail/` and each case id's dots become underscores.

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Layout › Master-detail";
const TITLE: &str = "Master-detail";
const BLURB: &str = "MasterDetail: wide split with a resizable seam, narrow summary and drawer";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "masterdetail"];
const JOIN: &str = "╮ │╭";
const JOIN_TIGHT: &str = "╮│╭";
const JOIN_NONE: &str = "╮  ╭";
const SUMMARY: &str = "→ orders · 12,408 rows";
const SIDEBAR: &str = "Tab Into page";
const LIST_HINT: &str = "Tab Details";
const DETAIL_HINT: &str = "Esc Back";

const OMITTED: &[&str] = &[
    "focused",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "error",
    "editing",
    "empty",
    "loading",
    "overflow",
];

const DRAWN: &[&str] = &["rest", "hover", "pressed", "narrow"];

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s07_masterdetail/{}",
        case_id.replace('.', "_")
    )
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

fn page(name: &str, opts: &[&str], cols: u16, rows: u16) -> App {
    let mut args = PAGE_ARGS.to_vec();
    for opt in opts {
        args.push("--opt");
        args.push(opt);
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

fn title_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("Tables") && line.contains("orders"))
        .unwrap_or_else(|| panic!("title row missing\n{}", text(screen)))
}

fn title_y(screen: &TerminalSnapshot) -> u16 {
    (0..screen.rows())
        .find(|row| {
            let line = row_text(screen, *row);
            line.contains("Tables") && line.contains("orders")
        })
        .unwrap_or_else(|| panic!("title row missing\n{}", text(screen)))
        .try_into()
        .expect("title row")
}

/// Column of the seam glyph on the Tables/orders title row.
fn seam_x(screen: &TerminalSnapshot) -> u16 {
    let line = title_line(screen);
    let tables = line.find("Tables").expect("Tables");
    let orders = line.find("orders").expect("orders");
    let between = &line[tables..orders];
    let rel = between
        .find(['│', '┃'])
        .unwrap_or_else(|| panic!("seam missing in {line:?}"));
    line[..tables + rel].chars().count() as u16
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

fn assert_choice(screen: &TerminalSnapshot, label: &str, value: &str, all: &[&str]) {
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
fn assert_options(
    screen: &TerminalSnapshot,
    width: &str,
    collapse: &str,
    size: &str,
    summary: &str,
    esc: &str,
    keys: &str,
    step: &str,
    gap: Option<&str>,
) {
    assert_choice(screen, "Demo width", width, &["fill", "100", "72"]);
    assert_choice(screen, "Collapses below", collapse, &["80", "108", "140"]);
    assert_choice(
        screen,
        "Detail size",
        size,
        &["resizable", "share", "master"],
    );
    assert_choice(screen, "Narrow summary rows", summary, &["1", "0", "3"]);
    assert_toggle(screen, "Esc returns to list", esc);
    assert_choice(screen, "Resize keys", keys, &["default", "custom", "none"]);
    assert_choice(screen, "Resize step", step, &["2", "1", "5"]);
    if let Some(gap) = gap {
        assert_choice(screen, "Gap between panes", gap, &["2", "1", "4"]);
    }
}

/// Grid labels sit under the legend. The blurb also contains "narrow".
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn label_y(screen: &TerminalSnapshot, label: &str) -> u16 {
    find_all(screen, label)
        .into_iter()
        .find(|hit| hit.y >= 9 && (18..56).contains(&hit.x))
        .map(|hit| hit.y)
        .unwrap_or_else(|| panic!("grid label {label} missing\n{}", text(screen)))
}

fn glyph_fg(screen: &TerminalSnapshot, label: &str, glyph: &str) -> SnapshotColor {
    let y = label_y(screen, label);
    let row = screen.row(u32::from(y)).expect("specimen row");
    let cols = screen.cols();
    row.iter()
        .enumerate()
        .find_map(|(index, cell)| {
            let x = index as u32;
            (x >= 30 && x + 2 < cols && cell.text() == glyph).then(|| cell.fg())
        })
        .unwrap_or_else(|| panic!("{label} has no {glyph}\n{}", row_text(screen, u32::from(y))))
}

fn row_has_seam(screen: &TerminalSnapshot, label: &str) -> bool {
    let y = label_y(screen, label);
    let row = screen.row(u32::from(y)).expect("specimen row");
    let cols = screen.cols();
    row.iter().enumerate().any(|(index, cell)| {
        let x = index as u32;
        x >= 30 && x + 2 < cols && (cell.text() == "│" || cell.text() == "┃")
    })
}

fn column_header(screen: &TerminalSnapshot) -> bool {
    find_all(screen, "Master-detail")
        .into_iter()
        .any(|hit| (8..12).contains(&hit.y) && hit.x > 24)
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

fn click_list(app: &App, name: &str) {
    click_where(app, name, |hit| hit.y > 10 && hit.x < 60);
}

fn reveal_side(app: &App, label: &str) {
    let point = Point { x: 8, y: 16 };
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel_applied(point, false, 3);
    }
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
}

fn reveal_text(app: &App, needle: &str) {
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel_applied(point, true, 3);
    }
    app.wait_text(needle);
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
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("Database browser");
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    flash_settle(app);
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
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

fn assert_specimens(screen: &TerminalSnapshot, wide: bool) {
    assert!(
        column_header(screen),
        "states column header is missing\n{}",
        text(screen)
    );
    for label in DRAWN {
        assert!(
            grid_label(screen, label),
            "states clipped {label}\n{}",
            text(screen)
        );
    }
    let rest = glyph_fg(screen, "rest", "│");
    let hover = glyph_fg(screen, "hover", "┃");
    let pressed = glyph_fg(screen, "pressed", "┃");
    assert_ne!(rest, hover, "rest and hover share a seam colour");
    assert_ne!(hover, pressed, "hover and pressed share a seam colour");
    assert_ne!(rest, pressed, "rest and pressed share a seam colour");
    assert!(
        !row_has_seam(screen, "narrow"),
        "narrow specimen drew a seam\n{}",
        text(screen)
    );
    assert_omitted(screen);
    if wide {
        see(
            screen,
            &[SUMMARY, "▸ orders", "12,408"],
            &["Open table", "Page options", "Database browser"],
        );
    } else {
        see(
            screen,
            &["▸ orders", "12,408", "○ Compare levels"],
            &[SUMMARY, CAP_STATES, LEGEND, WIDE, "truecolor"],
        );
    }
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
            "truecolor",
            "Playground",
            "Database browser",
            "94 cols · collapses below 80",
            "Show details",
            "Tables",
            "orders",
            "12,408",
            "shop.public.orders",
            "feature_flags",
            "facts",
            "Open table",
            JOIN,
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "Tab shows details",
            "Hide details",
            "Esc back",
            "┄",
            SUMMARY,
        ],
    );
    let foot = footer(&screen);
    assert!(
        foot.contains(SIDEBAR) && !foot.contains(LIST_HINT) && !foot.contains(DETAIL_HINT),
        "page flag did not leave focus on the sidebar: {foot:?}"
    );
    shot(&app, "sc.masterdetail.route");
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Database browser");
    see(
        &entered,
        &[CRUMB, BLURB, "orders", "12,408", JOIN],
        &[OVERVIEW, "Page options", "Layout › Splits & stacks"],
    );
    app.key("[");
    let left = app.wait_text("Layout › Splits & stacks");
    see(
        &left,
        &["Layout › Splits & stacks"],
        &[CRUMB, BLURB, "Database browser"],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Database browser", "orders"],
        &["Layout › Splits & stacks", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, "Database browser"]);
    click_side(&app, "Layout");
    app.wait_text("Splits & stacks");
    click_side(&app, "Master-detail");
    let pointed = app.wait_text("Database browser");
    see(
        &pointed,
        &[CRUMB, BLURB, "orders", "Show details", JOIN],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s07-sections", 80, 24, PAGE_ARGS);
    app.wait_text("Database browser");
    flash_settle(&app);
    let narrow_play = app.settle();
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "MasterDetail: wide split with a resizable",
            "Playground",
            "Database browser",
            "Show details",
            "Tables",
            "orders",
            "Schema",
            "12,408",
            JOIN,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "drawer",
            "Open table",
            "feature_flags",
            "shop.public.orders",
            "Tab shows details",
            "Hide details",
            SUMMARY,
        ],
    );
    assert!(
        footer(&narrow_play).contains(SIDEBAR),
        "narrow launch left the sidebar\n{}",
        footer(&narrow_play)
    );
    shot(&app, "sc.masterdetail.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_options(
        &narrow_opt,
        "fill",
        "80",
        "resizable",
        "1",
        "on",
        "default",
        "2",
        None,
    );
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Demo width", "Gap between panes"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Database browser",
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.masterdetail.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert_specimens(&narrow_states, false);
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "Master-detail"],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Database browser",
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.masterdetail.states.80x24");

    app.resize(120, 40);
    app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && text(screen).contains(SUMMARY)
            && grid_label(screen, "narrow")
    });
    flash_settle(&app);
    let wide_states = app.settle();
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
            "Master-detail",
            SUMMARY,
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Database browser",
            "256 colors",
        ],
    );
    assert_specimens(&wide_states, true);
    for (case_id, label) in [
        ("sc.masterdetail.specimen.rest", "rest"),
        ("sc.masterdetail.specimen.hover", "hover"),
        ("sc.masterdetail.specimen.pressed", "pressed"),
        ("sc.masterdetail.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label) && column_header(&app.screen()),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.masterdetail.states.120x40");
    shot(&app, "sc.masterdetail.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_options(
        &wide_opt,
        "fill",
        "80",
        "resizable",
        "1",
        "on",
        "default",
        "2",
        Some("2"),
    );
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Database browser"],
    );
    shot(&app, "sc.masterdetail.options.120x40");
    shot(&app, "sc.masterdetail.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Database browser");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "94 cols · collapses below 80",
            "Open table",
            "shop.public.orders",
            "feature_flags",
            JOIN,
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Tab shows details",
            "Hide details",
            "Esc back",
            "┄",
            SUMMARY,
        ],
    );
    shot(&app, "sc.masterdetail.playground.120x40");
    shot(&app, "sc.masterdetail.playground");
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
        &[
            CRUMB,
            "○ Compare levels",
            "rest",
            CAP_STATES,
            "Master-detail",
        ],
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
    see(
        &app.settle(),
        &["no color", WIDE],
        &["Page options", "○ Compare levels"],
    );
    shot(&app, "sc.masterdetail.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && (text(screen).contains("no color")
                || text(screen).contains("rest")
                || text(screen).contains("Compare levels"))
    });
    if !text(&app.screen()).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    flash_settle(&app);
    see(
        &app.settle(),
        &[TITLE, "no color"],
        &["Page options", WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.masterdetail.compare-levels.80x24");

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
        &[TITLE, "○ Compare levels", "rest", "Master-detail"],
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
    app.wait_screen("compare restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(WIDE)
    });
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_options(
        &options,
        "fill",
        "80",
        "resizable",
        "1",
        "on",
        "default",
        "2",
        Some("2"),
    );
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "Database browser"],
    );
    app.finish();
}

#[test]
fn option_width_100() {
    let app = page("s07-width", &["masterdetail.width=100"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "100",
            "80",
            "resizable",
            "1",
            "on",
            "default",
            "2",
            Some("2"),
        );
    });
    finish_option(&app, "opt.masterdetail.width", |screen| {
        see(
            screen,
            &[
                "100 cols",
                "Open table",
                "feature_flags",
                "shop.public.orders",
                "┄",
                JOIN,
            ],
            &[
                "94 cols",
                "Page options",
                "Tab shows details",
                "Hide details",
                SUMMARY,
            ],
        );
    });
}

#[test]
fn option_collapse_108() {
    let app = page("s07-collapse", &["masterdetail.collapse=108"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "fill",
            "108",
            "resizable",
            "1",
            "on",
            "default",
            "2",
            Some("2"),
        );
    });
    finish_option(&app, "opt.masterdetail.collapse", |screen| {
        see(
            screen,
            &["112 cols", "Open table", "┄", JOIN, "Show details"],
            &[
                "94 cols",
                "collapses below 80",
                "Page options",
                SUMMARY,
                "Hide details",
            ],
        );
    });
}

#[test]
fn option_size_share() {
    let app = page("s07-size", &["masterdetail.size=share"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "fill",
            "80",
            "share",
            "1",
            "on",
            "default",
            "2",
            Some("2"),
        );
    });
    let resting = app.settle();
    assert!(
        title_line(&resting).contains(JOIN_NONE),
        "share kept a seam: {}",
        title_line(&resting)
    );
    click_list(&app, "orders");
    let focused = app.wait_text(LIST_HINT);
    let foot = footer(&focused);
    assert!(
        foot.contains(LIST_HINT) && !foot.contains("Resize") && !foot.contains(SIDEBAR),
        "share still offers resize: {foot:?}"
    );
    finish_option(&app, "opt.masterdetail.size", |screen| {
        let line = title_line(screen);
        assert!(
            line.contains(JOIN_NONE) && !line.contains(JOIN) && !line.contains(JOIN_TIGHT),
            "share title: {line:?}"
        );
        see(
            screen,
            &["104 cols", "Show details", "Open table", LIST_HINT],
            &["Resize", "94 cols", "Page options", SUMMARY],
        );
    });
}

fn infeasible_share(name: &str, size: &str) {
    let app = page(
        name,
        &["masterdetail.width=72", "masterdetail.collapse=0", size],
        120,
        40,
    );
    app.wait_text("Database browser");
    flash_settle(&app);
    let screen = app.settle();
    see(
        &screen,
        &["72 cols", "Tables", "orders", "Show details", SUMMARY],
        &["Open table", "shop.public.orders", "Schema", JOIN],
    );
    app.finish();
}

#[test]
fn option_share_minimum_too_wide_collapses() {
    infeasible_share("s07-share-100", "masterdetail.size=share-100");
}

#[test]
fn option_master_share_minimum_too_wide_collapses() {
    infeasible_share("s07-master-100", "masterdetail.size=master-100");
}

#[test]
fn option_summary_0_at_width_72() {
    let app = page(
        "s07-summary",
        &["masterdetail.width=72", "masterdetail.summary=0"],
        120,
        40,
    );
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "72",
            "80",
            "resizable",
            "0",
            "on",
            "default",
            "2",
            Some("2"),
        );
    });
    finish_option(&app, "opt.masterdetail.summary", |screen| {
        see(
            screen,
            &[
                "72 cols",
                "collapses below 80",
                "Show details",
                "feature_flags",
                "Database browser",
            ],
            &[
                "Tab shows details",
                SUMMARY,
                "Schema",
                "facts",
                "Open table",
                "94 cols",
                "Page options",
                JOIN,
            ],
        );
    });
}

#[test]
fn option_esc_returns_off() {
    let app = page("s07-esc", &["masterdetail.esc_returns=off"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "fill",
            "80",
            "resizable",
            "1",
            "off",
            "default",
            "2",
            Some("2"),
        );
    });
    let origin = seam_x(&app.settle());
    click_list(&app, "orders");
    let listed = app.wait_text(LIST_HINT);
    let foot = footer(&listed);
    assert!(
        foot.contains(LIST_HINT) && foot.contains("Alt+Shift") && !foot.contains(DETAIL_HINT),
        "list focus: {foot:?}"
    );
    assert_eq!(seam_x(&listed), origin, "clicking the list moved the seam");
    assert!(
        !text(&listed).contains("Opened orders"),
        "list click activated Open table\n{}",
        text(&listed)
    );
    app.key("tab");
    let detail = app.wait_screen("detail focus", |screen| {
        let foot = footer(screen);
        foot.contains("Tab Next") && foot.contains("Alt+Shift") && !foot.contains(LIST_HINT)
    });
    let foot = footer(&detail);
    assert!(
        !foot.contains(DETAIL_HINT) && !foot.contains("Enter Activate"),
        "esc off still hints back or landed on the button: {foot:?}"
    );
    flash_settle(&app);
    let focused = app.settle();
    let foot = footer(&focused);
    assert!(
        foot.contains("Alt+Shift") && foot.contains("Tab Next") && !foot.contains(DETAIL_HINT),
        "detail hint changed after the flash: {foot:?}"
    );
    see(
        &focused,
        &["shop.public.orders", "12,408", "facts", JOIN],
        &["Opened orders", "Page options", SUMMARY, "Hide details"],
    );
    assert_size(&app, 120, 40);
    shot(&app, "opt.masterdetail.esc_returns");
    app.key("esc");
    let left = app.wait_text(SIDEBAR);
    let foot = footer(&left);
    assert!(
        foot.contains(SIDEBAR)
            && foot.contains("q Quit")
            && !foot.contains(LIST_HINT)
            && !foot.contains(DETAIL_HINT),
        "esc stayed in the detail: {foot:?}"
    );
    see(
        &left,
        &["shop.public.orders", "Database browser"],
        &["Opened orders", "Hide details"],
    );
    app.finish();
}

#[test]
fn option_keys_custom() {
    let app = page("s07-keys", &["masterdetail.keys=custom"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "fill",
            "80",
            "resizable",
            "1",
            "on",
            "custom",
            "2",
            Some("2"),
        );
    });
    let origin = seam_x(&app.settle());
    click_list(&app, "orders");
    let listed = app.wait_text("Ctrl+H");
    let foot = footer(&listed);
    assert!(
        foot.contains("Ctrl+H Resize") && foot.contains(LIST_HINT) && !foot.contains("Alt+Shift"),
        "custom hint: {foot:?}"
    );
    assert_eq!(seam_x(&listed), origin, "clicking the list moved the seam");
    app.key("alt+shift+right");
    let stayed = app.settle();
    assert_eq!(seam_x(&stayed), origin, "Alt+Shift moved a custom binding");
    app.key("ctrl+l");
    let moved = app.wait_screen("custom nudge", |screen| seam_x(screen) == origin + 2);
    assert!(
        footer(&moved).contains("Ctrl+H Resize"),
        "nudge left the list: {}",
        footer(&moved)
    );
    finish_option(&app, "opt.masterdetail.keys", |screen| {
        assert_eq!(seam_x(screen), origin + 2, "flash settled the seam back");
        see(
            screen,
            &["Ctrl+H Resize", "shop.public.orders", JOIN],
            &["Alt+Shift", "Page options", SUMMARY],
        );
    });
}

#[test]
fn option_step_5() {
    let app = page("s07-step", &["masterdetail.step=5"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "fill",
            "80",
            "resizable",
            "1",
            "on",
            "default",
            "5",
            Some("2"),
        );
    });
    let origin = seam_x(&app.settle());
    click_list(&app, "orders");
    let listed = app.wait_text(LIST_HINT);
    assert!(
        footer(&listed).contains("Alt+Shift") && !footer(&listed).contains("Ctrl+H"),
        "step changed the hint: {}",
        footer(&listed)
    );
    assert_eq!(seam_x(&listed), origin, "clicking the list moved the seam");
    app.key("alt+shift+right");
    let moved = app.wait_screen("step 5", |screen| seam_x(screen) == origin + 5);
    assert!(footer(&moved).contains("Alt+Shift"));
    finish_option(&app, "opt.masterdetail.step", |screen| {
        assert_eq!(seam_x(screen), origin + 5, "flash settled the seam back");
        see(
            screen,
            &["Alt+Shift", "shop.public.orders", JOIN, LIST_HINT],
            &["Ctrl+H", "Page options", SUMMARY],
        );
    });
}

#[test]
fn option_gap_1() {
    let app = page("s07-gap", &["masterdetail.gap=1"], 120, 40);
    app.wait_text("Database browser");
    confirm_options(&app, |screen| {
        assert_options(
            screen,
            "fill",
            "80",
            "resizable",
            "1",
            "on",
            "default",
            "2",
            Some("1"),
        );
    });
    finish_option(&app, "opt.masterdetail.gap", |screen| {
        let line = title_line(screen);
        assert!(
            line.contains(JOIN_TIGHT) && !line.contains(JOIN),
            "gap 1 title: {line:?}"
        );
        see(
            screen,
            &["94 cols · collapses below 80", "Open table", "Show details"],
            &["Page options", SUMMARY, "Hide details"],
        );
    });
}

#[test]
fn detail_seam_drag_before_release() {
    let app = open("s07-detail", 120, 40, PAGE_ARGS);
    app.wait_text("Database browser");
    flash_settle(&app);
    let before = app.settle();
    assert_size(&app, 120, 40);
    let origin = seam_x(&before);
    let line = title_line(&before);
    assert!(
        line.contains(JOIN) && !line.contains('┃'),
        "rest seam is not thin: {line:?}"
    );
    see(
        &before,
        &[
            CRUMB,
            "12,408",
            "shop.public.orders",
            "facts",
            "Open table",
            "Show details",
            "order_items",
            "48,221",
        ],
        &[
            SUMMARY,
            "Hide details",
            "Esc back",
            "Page options",
            "Opened orders",
        ],
    );
    shot(&app, "sc.masterdetail.detail.before");

    let seam = Point {
        x: origin,
        y: title_y(&before),
    };
    app.move_to(seam);
    app.down(seam);
    let dragged = Point {
        x: origin + 6,
        y: seam.y,
    };
    app.drag_to(dragged);
    let held = app.wait_screen("seam held", |screen| {
        seam_x(screen) > origin && title_line(screen).contains('┃')
    });
    let moved = seam_x(&held);
    assert!(moved > origin, "drag left the seam at {origin}");
    assert_eq!(
        cell_text(&held, moved, title_y(&held)),
        "┃",
        "held seam is not the heavy rule"
    );
    see(
        &held,
        &["shop.public.orders", "12,408", "facts", "orders"],
        &[SUMMARY, "Hide details", "Opened orders", "Page options"],
    );
    shot(&app, "sc.masterdetail.detail.after");
    let still = app.screen();
    assert!(
        title_line(&still).contains('┃') && seam_x(&still) == moved,
        "the held shot released the seam\n{}",
        title_line(&still)
    );

    app.up(dragged);
    let released = app.settle();
    let after = seam_x(&released);
    assert_eq!(after, moved, "release restored the seam to {origin}");
    let line = title_line(&released);
    assert!(
        line.contains('│') && !line.contains('┃'),
        "released seam stayed heavy: {line:?}"
    );
    assert_ne!(after, origin, "release returned the seam to the start");

    click_list(&app, "order_items");
    let next = app.wait_text("shop.public.order_items");
    see(
        &next,
        &["48,221", "shop.public.order_items", "12,408", "orders"],
        &[
            "shop.public.orders",
            "Opened orders",
            SUMMARY,
            "Page options",
        ],
    );
    app.finish();
}

fn cell_text(screen: &TerminalSnapshot, x: u16, y: u16) -> String {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|item| item.text().to_string())
        .unwrap_or_default()
}
