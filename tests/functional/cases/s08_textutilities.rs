//! Text utilities route, sections, specimens, compare levels, helpers, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s08_textutilities/` and each case id's dots become underscores.
//!
//! Motion stays paused. The States grid's Loading row spins under normal motion,
//! so a capture would not settle, and a paused press flash has to be cleared
//! with [`flash_settle`].

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Text & code › Text utilities";
const TERMINAL: &str = "Text & code › Terminal";
const PREVIOUS: &str = "Text & code › Terminal viewport";
const TITLE: &str = "Text utilities";
const BLURB: &str = "Text helpers (truncation, wrapping, widths), property sheets and PropsList";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "textutilities"];
const WIDTH_TITLE: &str = "ui::text at width 20";
const TRUNCATE: &str = "payments-platform/s…";
const TRUNCATE_MIDDLE: &str = "payments-plat…ackoff";
const TRUNCATE_SEP: &str = "Tables › payments ›…";
const CRUMB_FIT: &str = "Tables › … › sett…s";
const SLICE: &str = "…latform/settlement…";
const CLIP: &str = "payments-platform/se";
const FIT: &str = "[ok                  ]";
const FIT_RIGHT: &str = "[                  42]";
const CENTER: &str = "\"ok\" starts at 9 of 20";
const DURATION: &str = "0 s · 0 s · 1 s · 3 min 2 s";
const FIND: &str = "\"select\" any case → 0..6";
const READOUT: &str = "PropsList · nav default · copy y · track…";
const META: &str = "Enter activates · y copies";
const HOST: &str = "db-primary.pay…";
const WRAP_TITLE: &str = "keep_indent true";
const SPOKEN: &str = "duration Spoken";

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "editing",
    "empty",
];

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s08_textutilities/{}",
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

fn assert_one(screen: &TerminalSnapshot, label: &str, value: &str, all: &[&str]) {
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
fn assert_option_values(
    screen: &TerminalSnapshot,
    width: &str,
    sheet: &str,
    wrap: &str,
    duration: &str,
    case: &str,
    nav: &str,
    copy: &str,
    track: &str,
    gap: &str,
    reveal: &str,
) {
    assert_one(screen, "Width", width, &["8", "12", "20", "32"]);
    assert_one(screen, "Label column", sheet, &["default", "narrow-labels"]);
    assert_one(
        screen,
        "Wrap",
        wrap,
        &["default", "plain", "no-hang", "no-indent"],
    );
    assert_one(screen, "Duration style", duration, &["spoken", "compact"]);
    assert_toggle(screen, "Match case", case);
    assert_one(screen, "List nav keys", nav, &["default", "vim"]);
    assert_one(screen, "Copy key", copy, &["y", "c"]);
    assert_one(screen, "Track click", track, &["page", "jump"]);
    assert_one(screen, "Scrollbar gap", gap, &["0", "1", "2"]);
    assert_one(screen, "Reveal", reveal, &["tooltip", "inline", "off"]);
}

fn assert_defaults(screen: &TerminalSnapshot) {
    assert_option_values(
        screen, "20", "default", "default", "spoken", "off", "default", "y", "page", "1", "tooltip",
    );
}

fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

/// Grid labels sit under the legend. The legend's "focused sample" is above y=9.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    grid_hit(screen, label).is_some()
}

fn grid_row(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = grid_hit(screen, label)
        .unwrap_or_else(|| panic!("grid label {label:?} missing\n{}", text(screen)));
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
    reveal_side(app, label);
    click_where(app, label, |hit| hit.x < 24);
}

fn click_host(app: &App) {
    click_where(app, "Host", |hit| hit.x > 60 && hit.y > 6);
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
            WIDTH_TITLE,
            TRUNCATE,
            TRUNCATE_MIDDLE,
            TRUNCATE_SEP,
            CRUMB_FIT,
            SLICE,
            CLIP,
            "true (width 36)",
            "10 cells for 日本語 abc",
            CENTER,
            "1,234,567",
            "1 row · 3 rows",
            "512 B · 1.5 KiB · 3.2 GiB",
            SPOKEN,
            DURATION,
            FIND,
            "Prop::wrap",
            WRAP_TITLE,
            "Wrapping keeps words whole and",
            "hard-wraps a word longer than the",
            READOUT,
            "last PropsAction:",
            META,
            HOST,
            "Replication lag",
            "healthy",
            "Some(21..26)",
            "users",
            "TextBuffer",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "ui::text at width 8",
            "payment…",
            "payme…ff",
            "duration Compact",
            "<1 ms",
            "case-sensitive",
            "keep_indent false",
            "nav vim",
            "copy c",
            "Repli…",
            "Copy(Host)",
            "Activate(Host)",
            "keeps its indent",
        ],
    );
    let raw = text(screen);
    assert!(
        raw.contains(FIT) && raw.contains(FIT_RIGHT),
        "width 20 padding missing\n{raw}"
    );
    let capped = row_with(screen, "width_capped");
    assert!(
        capped.contains("21") && !capped.contains(" 9"),
        "width_capped is cap plus one when the sample is wider\n{capped}"
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

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(CAP_PLAY);
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
fn route_by_page_keyboard_and_pointer() {
    let app = open("s08-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    shot(&app, "sc.textutilities.route");
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
        "Text & code › Code editor",
        "Text & code › Diff",
        TERMINAL,
        "Text & code › Terminal pane",
        PREVIOUS,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(TRUNCATE);
    see(
        &entered,
        &[CRUMB, BLURB, WIDTH_TITLE, TRUNCATE, CRUMB_FIT],
        &[OVERVIEW, "Page options", PREVIOUS],
    );
    app.key("[");
    let left = app.wait_text(PREVIOUS);
    see(&left, &[PREVIOUS], &[CRUMB, WIDTH_TITLE, TRUNCATE]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, WIDTH_TITLE, "PropsList"],
        &[PREVIOUS, OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, WIDTH_TITLE, BLURB]);
    click_side(&app, "Text & code");
    click_side(&app, "Text utilities");
    let pointed = app.wait_text(TRUNCATE);
    see(
        &pointed,
        &[CRUMB, BLURB, WIDTH_TITLE, TRUNCATE, HOST],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s08-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("payments-platfor…");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Text helpers (truncation, wrapping, width",
            "Playground",
            "ui::text at…",
            "payments-platfor…",
            "payments-plat…ac…",
            "Tables › … › set…",
            "…latform/settlem…",
            "5432",
            "healthy",
            "PropsList · nav default",
            "last PropsAction:",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "property sheets",
            WIDTH_TITLE,
            TRUNCATE,
            "keep_indent",
            "wrap_with",
            "TextBuffer",
            "1,234,567",
            "日本語",
            "80×24",
        ],
    );
    shot(&app, "sc.textutilities.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_one(&narrow_opt, "Width", "20", &["8", "12", "20", "32"]);
    assert_one(
        &narrow_opt,
        "Label column",
        "default",
        &["default", "narrow-labels"],
    );
    assert_one(
        &narrow_opt,
        "Wrap",
        "default",
        &["default", "plain", "no-hang", "no-indent"],
    );
    assert_one(
        &narrow_opt,
        "Duration style",
        "spoken",
        &["spoken", "compact"],
    );
    assert_toggle(&narrow_opt, "Match case", "off");
    assert_one(&narrow_opt, "List nav keys", "default", &["default", "vim"]);
    assert_one(&narrow_opt, "Copy key", "y", &["y", "c"]);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Width", "Track click", "Copy key"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDTH_TITLE,
            TRUNCATE,
            WIDE,
            "truecolor",
            "Reveal",
            "Scrollbar gap",
            "property sheets",
            "80×24",
        ],
    );
    shot(&app, "sc.textutilities.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "focused", "loading", "error", "overflow"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    assert!(
        !grid_label(&narrow_states, "narrow"),
        "narrow specimen is on the 80 by 24 states screen\n{}",
        text(&narrow_states)
    );
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "focused",
            "loading",
            "error",
            "overflow",
            "Facts",
            "db-primary.payments.internal",
            "Loading rows",
            "Could not load",
            "5432",
            "healthy",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            WIDTH_TITLE,
            WIDE,
            "truecolor",
            "Narrow labels",
            "Ho…",
            "property sheets",
            "80×24",
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.textutilities.states.80x24");

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
            "Facts",
            "Narrow labels",
            "db-primary.payments.internal",
            "Loading rows",
            "Could not load",
            "y copy",
            "internal and",
            "db-primary.p…",
            "Ho…",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            WIDTH_TITLE,
            "256 colors",
            TRUNCATE,
        ],
    );
    assert!(
        grid_row(&wide_states, "focused").contains('▏'),
        "focused specimen has no bar\n{}",
        text(&wide_states)
    );
    assert!(
        !grid_row(&wide_states, "rest").contains('▏'),
        "rest specimen draws the focused bar\n{}",
        text(&wide_states)
    );
    assert_omitted(&wide_states);
    for (case_id, label) in [
        ("sc.textutilities.specimen.rest", "rest"),
        ("sc.textutilities.specimen.focused", "focused"),
        ("sc.textutilities.specimen.loading", "loading"),
        ("sc.textutilities.specimen.error", "error"),
        ("sc.textutilities.specimen.overflow", "overflow"),
        ("sc.textutilities.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.textutilities.states.120x40");
    shot(&app, "sc.textutilities.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_defaults(&wide_opt);
    see(
        &wide_opt,
        &[
            CRUMB,
            CAP_OPT,
            WIDE,
            "Page options",
            "Reveal",
            "Scrollbar gap",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            WIDTH_TITLE,
            TRUNCATE,
        ],
    );
    shot(&app, "sc.textutilities.options.120x40");
    shot(&app, "sc.textutilities.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(WIDTH_TITLE);
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.textutilities.playground.120x40");
    shot(&app, "sc.textutilities.playground");
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
        &[CRUMB, "○ Compare levels", "rest", "narrow", CAP_STATES],
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
    shot(&app, "sc.textutilities.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    // The first paint after a shrink can still show the wide scroll offset.
    // Decide from the settled frame, then use the same wheel as the wide reveal.
    let mut narrow = app.settle();
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
        narrow = app.settle();
    }
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels", "80×24"],
    );
    shot(&app, "sc.textutilities.compare-levels.80x24");

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
    assert_defaults(&options);
    see(
        &options,
        &["20", "Page options", CAP_OPT, CRUMB, "tooltip"],
        &["Compare levels", OVERVIEW, "narrow-labels", "compact"],
    );
    app.finish();
}

#[test]
fn text_helpers_stay_when_a_row_is_copied() {
    let app = open("s08-text", 120, 40, PAGE_ARGS);
    let before = app.wait_text(TRUNCATE);
    assert_size(&app, 120, 40);
    assert_playground(&before);
    shot(&app, "sc.textutilities.text.before");

    click_host(&app);
    flash_settle(&app);
    app.key("y");
    let copied = app.wait_text("Copy(Host)");
    see(
        &copied,
        &[
            CRUMB,
            TITLE,
            BLURB,
            WIDTH_TITLE,
            TRUNCATE,
            TRUNCATE_MIDDLE,
            CRUMB_FIT,
            WRAP_TITLE,
            SPOKEN,
            "1,234,567",
            "Copy(Host)",
            "Copied Host",
            "db-pri…",
            "y copy",
            "Replication lag",
            READOUT,
        ],
        &[
            "Page options",
            "Compare levels",
            "Copy(Port)",
            "Copy(Notes)",
            "ui::text at width 8",
            "payment…",
            "duration Compact",
            "case-sensitive",
            "keep_indent false",
            OVERVIEW,
        ],
    );
    assert!(
        text(&copied).contains(FIT) && !text(&copied).contains("keeps its indent"),
        "copying a row changed the helper padding or wrap\n{}",
        text(&copied)
    );
    shot(&app, "sc.textutilities.text.after");
    app.finish();
}

#[test]
fn option_width_8() {
    let app = page("s08-width", Some("textutilities.width=8"), 120, 40);
    app.wait_text("ui::text at width 8");
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "8", "default", "default", "spoken", "off", "default", "y", "page", "1",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.width", |screen| {
        see(
            screen,
            &[
                "ui::text at width 8",
                "payment…",
                "payme…ff",
                "Tables …",
                CRUMB_FIT,
                "…latfor…",
                "true (width 36)",
                "\"ok\" starts at 3 of 8",
                "wrap_with at 24",
                SPOKEN,
                READOUT,
                HOST,
            ],
            &[
                WIDTH_TITLE,
                TRUNCATE,
                TRUNCATE_MIDDLE,
                TRUNCATE_SEP,
                CENTER,
                "wrap_with at 36",
                "Page options",
                "Compare levels",
                "duration Compact",
            ],
        );
        let raw = text(screen);
        assert!(
            raw.contains("[ok      ]") && raw.contains("[      42]"),
            "width 8 fit padding missing\n{raw}"
        );
        let clip = row_with(screen, "clip");
        assert!(
            clip.contains("payments") && !clip.contains("payments-platform"),
            "clip at width 8 is the eight-cell prefix: {clip:?}"
        );
        let capped = row_with(screen, "width_capped");
        assert!(
            capped.contains(" 9") && !capped.contains("21"),
            "width 8 caps at 9, not 21: {capped:?}"
        );
    });
}

#[test]
fn option_sheet_narrow_labels() {
    let app = page(
        "s08-sheet",
        Some("textutilities.sheet=narrow-labels"),
        120,
        40,
    );
    app.wait_text(WIDTH_TITLE);
    confirm_options(&app, |screen| {
        assert_option_values(
            screen,
            "20",
            "narrow-labels",
            "default",
            "spoken",
            "off",
            "default",
            "y",
            "page",
            "1",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.sheet", |screen| {
        see(
            screen,
            &[
                WIDTH_TITLE,
                TRUNCATE,
                "Repli…",
                "db-primary.payments.inter…",
                "postgres://app@db-primary…",
                "Failover is manual",
                "Conne…",
                READOUT,
                WRAP_TITLE,
            ],
            &[
                "Replication lag",
                HOST,
                "postgres://app…",
                "Connection string",
                "ui::text at width 8",
                "Page options",
                "keep_indent false",
                "duration Compact",
            ],
        );
    });
}

#[test]
fn option_wrap_plain() {
    let app = page("s08-wrap", Some("textutilities.wrap=plain"), 120, 40);
    app.wait_text(WIDTH_TITLE);
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "plain", "spoken", "off", "default", "y", "page", "1",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.wrap", |screen| {
        see(
            screen,
            &[
                WIDTH_TITLE,
                TRUNCATE,
                "keep_indent false",
                "An indented block keeps its indent",
                "Wrapping keeps words whole and",
                READOUT,
                SPOKEN,
            ],
            &[
                WRAP_TITLE,
                "ui::text at width 8",
                "Page options",
                "duration Compact",
                "nav vim",
            ],
        );
    });
}

#[test]
fn option_duration_compact() {
    let app = page(
        "s08-duration",
        Some("textutilities.duration=compact"),
        120,
        40,
    );
    app.wait_text("duration Compact");
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "compact", "off", "default", "y", "page", "1",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.duration", |screen| {
        see(
            screen,
            &[
                WIDTH_TITLE,
                TRUNCATE,
                "duration Compact",
                "<1 ms · 840 ms · 1.25 s",
                READOUT,
                WRAP_TITLE,
                FIND,
            ],
            &[
                SPOKEN,
                DURATION,
                "0 s · 0 s",
                "3 min 2 s",
                "Page options",
                "ui::text at width 8",
                "case-sensitive",
            ],
        );
    });
}

#[test]
fn option_case_on() {
    let app = page("s08-case", Some("textutilities.case=on"), 120, 40);
    app.wait_text("case-sensitive");
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "spoken", "on", "default", "y", "page", "1",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.case", |screen| {
        see(
            screen,
            &[
                WIDTH_TITLE,
                TRUNCATE,
                "case-sensitive",
                "22.",
                SPOKEN,
                DURATION,
                READOUT,
                WRAP_TITLE,
            ],
            &[
                "any case",
                "0..6",
                "Page options",
                "ui::text at width 8",
                "duration Compact",
                "nav vim",
            ],
        );
    });
}

#[test]
fn option_nav_vim() {
    let app = page("s08-nav", Some("textutilities.nav=vim"), 120, 40);
    app.wait_text("nav vim");
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "spoken", "off", "vim", "y", "page", "1", "tooltip",
        );
    });
    click_host(&app);
    flash_settle(&app);
    app.key("j");
    app.key("enter");
    app.wait_text("Activate(Port)");
    finish_option(&app, "opt.textutilities.nav", |screen| {
        see(
            screen,
            &[
                "nav vim",
                "Activate(Port)",
                "Port: 5432",
                WIDTH_TITLE,
                TRUNCATE,
                CRUMB_FIT,
                SPOKEN,
                "copy y",
            ],
            &[
                "nav default",
                "Activate(Host)",
                "Copy(",
                "Page options",
                "ui::text at width 8",
                "duration Compact",
                "case-sensitive",
            ],
        );
    });
}

#[test]
fn option_copy_key_c() {
    let app = page("s08-copy", Some("textutilities.copy_key=c"), 120, 40);
    app.wait_text("copy c");
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "spoken", "off", "default", "c", "page", "1",
            "tooltip",
        );
    });
    click_host(&app);
    flash_settle(&app);
    app.key("c");
    app.wait_text("Copy(Host)");
    finish_option(&app, "opt.textutilities.copy_key", |screen| {
        see(
            screen,
            &[
                "copy c",
                META,
                "Copy(Host)",
                "Copied Host",
                "c Copy",
                "y copy",
                "db-pri…",
                WIDTH_TITLE,
                TRUNCATE,
                SPOKEN,
                "nav default",
            ],
            &[
                "copy y",
                "Copy(Port)",
                "Page options",
                "ui::text at width 8",
                "nav vim",
                "duration Compact",
            ],
        );
    });
}

#[test]
fn option_track_click_jump() {
    let app = page("s08-track", Some("textutilities.track_click=jump"), 120, 40);
    app.wait_text(WIDTH_TITLE);
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "spoken", "off", "default", "y", "jump", "1",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.track_click", |screen| {
        assert_playground(screen);
        let readout = row_with(screen, "PropsList ·");
        assert!(
            readout.contains("track…") && !readout.contains("Jump") && !readout.contains("Page"),
            "track click is clipped off the readout: {readout:?}"
        );
    });
}

#[test]
fn option_scrollbar_gap_0() {
    let app = page("s08-gap", Some("textutilities.scrollbar_gap=0"), 120, 40);
    app.wait_text(WIDTH_TITLE);
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "spoken", "off", "default", "y", "page", "0",
            "tooltip",
        );
    });
    finish_option(&app, "opt.textutilities.scrollbar_gap", |screen| {
        assert_playground(screen);
        assert!(
            !text(screen).contains("scrollbar_gap"),
            "scrollbar gap is not painted on the playground\n{}",
            text(screen)
        );
    });
}

#[test]
fn option_reveal_inline() {
    let app = page("s08-reveal", Some("textutilities.reveal=inline"), 120, 40);
    app.wait_text(WIDTH_TITLE);
    confirm_options(&app, |screen| {
        assert_option_values(
            screen, "20", "default", "default", "spoken", "off", "default", "y", "page", "1",
            "inline",
        );
    });
    finish_option(&app, "opt.textutilities.reveal", |screen| {
        assert_playground(screen);
        let visible = text(screen);
        assert!(
            !visible.contains("InlineWrap") && !visible.contains("reveal off"),
            "reveal mode is clipped off the playground\n{visible}"
        );
    });
}
