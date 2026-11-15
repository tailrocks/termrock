//! Meters route, sections, specimens, compare levels, refresh, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s06_meters/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Feedback › Meters";
const TITLE: &str = "Meters";
const BLURB: &str = "MeterList: labelled capacity meters in aligned columns, with tones and levels";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "meters"];
const LIVE: &str = "· live";
const PLAN: &str = "Plan usage";
const MEASURED: &str = "domain tones · measured labels";
const SESSION: &str = "Session (5 h)";
const DETAIL: &str = "resets in 2 h 14 m";
const OFFLINE_STATUS: &str = "· offline · last good 4 min ago";
const OFFLINE_META: &str = "offline · last-good values";
const REFRESHED: &str = "Usage refreshed ✓";
const OFFLINE_HOLD: &str = "Offline: the meters keep their last-good values";
const MUTED: SnapshotColor = SnapshotColor::Rgb(138, 138, 138);
const SECONDARY: SnapshotColor = SnapshotColor::Rgb(179, 179, 179);

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s06_meters/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str], motion: &str) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Components");
    app
}

fn page(name: &str, opts: &[&str], cols: u16, rows: u16) -> App {
    let mut args = PAGE_ARGS.to_vec();
    for opt in opts {
        args.extend(["--opt", opt]);
    }
    open(name, cols, rows, &args, "paused")
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

fn spaces_between(line: &str, left: &str, right: &str) -> usize {
    let at = line
        .find(left)
        .unwrap_or_else(|| panic!("{left:?} missing in {line:?}"));
    let after = &line[at + left.len()..];
    let rel = after
        .find(right)
        .unwrap_or_else(|| panic!("{right:?} missing after {left} in {line:?}"));
    let between = &after[..rel];
    assert!(
        between.chars().all(|ch| ch == ' '),
        "non-space between {left} and {right}: {between:?} in {line:?}"
    );
    between.chars().count()
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

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 9 && (18..52).contains(&hit.x))
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

fn side_has(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.x < 24 && hit.y >= 3)
}

fn reveal_side(app: &App, label: &str) {
    if side_has(&app.screen(), label) {
        return;
    }
    let point = Point { x: 6, y: 14 };
    for down in [false, true] {
        for _ in 0..16 {
            if side_has(&app.screen(), label) {
                return;
            }
            app.wheel(point, down, 2);
        }
    }
    panic!("sidebar lost {label}\n{}", text(&app.screen()));
}

fn click_side(app: &App, label: &str) {
    reveal_side(app, label);
    click_where(app, label, |hit| hit.x < 24 && hit.y >= 3);
}

fn click_button(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 6 && hit.x >= 24);
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

fn band(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| (8..16).contains(&hit.y) && hit.x >= 24)
}

fn fg_of(screen: &TerminalSnapshot, needle: &str) -> SnapshotColor {
    let hit = find_all(screen, needle)
        .into_iter()
        .find(|hit| hit.x > 24 && hit.y > 4)
        .unwrap_or_else(|| panic!("no {needle:?}\n{}", text(screen)));
    screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .unwrap_or_else(|| panic!("missing cell {needle:?}"))
        .fg()
}

fn italic_at(screen: &TerminalSnapshot, needle: &str) -> bool {
    let hit = find_all(screen, needle)
        .into_iter()
        .find(|hit| hit.x > 24 && hit.y > 4)
        .unwrap_or_else(|| panic!("no {needle:?}\n{}", text(screen)));
    screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .unwrap_or_else(|| panic!("missing cell {needle:?}"))
        .has(tui_test::terminal::cell::Attrs::ITALIC)
}

/// The 140ms press flash stays painted while motion is paused, so a shot after
/// a released click has to redraw once the deadline has passed. Below about 85
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

fn assert_option_controls(screen: &TerminalSnapshot, skip: &[&str]) {
    if !skip.contains(&"label_width") {
        assert_select(screen, "Label column", "measured", &["12", "8"]);
    }
    if !skip.contains(&"label_gap") {
        assert_select(screen, "Label gap", "1", &["3"]);
    }
    if !skip.contains(&"meter_width") {
        assert_select(screen, "Meter width", "24", &["fill", "14"]);
    }
    if !skip.contains(&"detail") {
        assert_toggle(screen, "Details", "on");
    }
    if !skip.contains(&"detail_tone") {
        assert_select(screen, "Detail tone", "muted", &["secondary"]);
    }
    if !skip.contains(&"visual") {
        assert_select(screen, "Quota meter visual", "block", &["line"]);
    }
    if !skip.contains(&"offline") {
        assert_toggle(screen, "Offline (last-good)", "off");
    }
    if !skip.contains(&"stale_style") {
        assert_select(screen, "Stale style", "dashed", &["faint"]);
    }
}

fn confirm_option(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(PLAN);
    flash_settle(app);
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s06-route-page", 120, 40, PAGE_ARGS, "paused");
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
            "Usage and capacity",
            LIVE,
            PLAN,
            MEASURED,
            SESSION,
            "38%",
            "Weekly",
            "72%",
            DETAIL,
            "Refresh",
            "Filesystems",
            "Host",
            "CPU",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            OFFLINE_STATUS,
            "41%",
            "refreshing",
        ],
    );
    let session = row_with(&screen, SESSION);
    assert!(
        !session.contains('━'),
        "block quotas drew a line bar: {session:?}"
    );
    assert_eq!(
        spaces_between(&row_with(&screen, "Weekly"), "Weekly", "72%"),
        10
    );
    shot(&app, "sc.meters.route");
    app.finish();

    let app = open("s06-route-keys", 120, 40, &[], "paused");
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(PLAN);
    see(
        &entered,
        &[CRUMB, BLURB, PLAN, SESSION, "38%"],
        &[OVERVIEW, "Page options", "Feedback › Empty & error"],
    );
    app.key("[");
    let left = app.wait_text("Feedback › Empty & error");
    see(
        &left,
        &["Feedback › Empty & error"],
        &[CRUMB, PLAN, SESSION],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, PLAN, "38%"],
        &["Feedback › Empty & error", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, PLAN, BLURB]);
    if !side_has(&app.screen(), "Meters") {
        click_side(&app, "Feedback");
        app.wait_text("Meters");
    }
    click_side(&app, "Meters");
    let pointed = app.wait_text(PLAN);
    see(
        &pointed,
        &[CRUMB, BLURB, PLAN, SESSION, "Refresh"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s06-sections", 80, 24, PAGE_ARGS, "paused");
    let narrow_play = app.wait_text(PLAN);
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "MeterList",
            "Playground",
            "Usage and capacity",
            LIVE,
            PLAN,
            SESSION,
            "38%",
            "Refresh",
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
            "Host",
            "Filesystems",
        ],
    );
    shot(&app, "sc.meters.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_select(&narrow_opt, "Label column", "measured", &["12", "8"]);
    assert_select(&narrow_opt, "Label gap", "1", &["3"]);
    assert_select(&narrow_opt, "Meter width", "24", &["fill", "14"]);
    assert_toggle(&narrow_opt, "Details", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Label column", "Quota meter visual"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            PLAN,
            WIDE,
            "truecolor",
            SESSION,
        ],
    );
    shot(&app, "sc.meters.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "loading", "error", "overflow", "narrow"] {
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
            "Quota list",
            "38%",
            "86%",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            PLAN,
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    shot(&app, "sc.meters.states.80x24");

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
            "Quota list",
            "Disk list",
            "38%",
            "86%",
            "read failed",
            "and a longer name",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            PLAN,
            "256 colors",
        ],
    );
    for label in [
        "hover",
        "pressed",
        "focused",
        "focus + hover",
        "selected",
        "disabled",
        "busy",
        "editing",
        "empty",
    ] {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in [
        ("sc.meters.specimen.rest", "rest"),
        ("sc.meters.specimen.loading", "loading"),
        ("sc.meters.specimen.error", "error"),
        ("sc.meters.specimen.overflow", "overflow"),
        ("sc.meters.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.meters.states.120x40");
    shot(&app, "sc.meters.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_option_controls(&wide_opt, &[]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Stale style"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", PLAN],
    );
    shot(&app, "sc.meters.options.120x40");
    shot(&app, "sc.meters.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(SESSION);
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            PLAN,
            MEASURED,
            SESSION,
            "38%",
            "72%",
            DETAIL,
            "Filesystems",
            "Host",
            "Refresh",
            LIVE,
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            OFFLINE_STATUS,
            "41%",
        ],
    );
    shot(&app, "sc.meters.playground.120x40");
    shot(&app, "sc.meters.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s06-compare", 120, 40, PAGE_ARGS, "paused");
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
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 18 };
    for _ in 0..80 {
        let screen = app.settle();
        let visible = text(&screen);
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
    shot(&app, "sc.meters.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    if !text(&narrow).contains("no color") {
        let point = Point { x: 40, y: 16 };
        for _ in 0..40 {
            if text(&app.settle()).contains("no color") {
                break;
            }
            app.wheel(point, true, 3);
        }
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.meters.compare-levels.80x24");

    app.resize(120, 40);
    app.wait_screen("compare restored width", |screen| screen.cols() == 120);
    let point = Point { x: 70, y: 16 };
    for _ in 0..80 {
        if band(&app.settle(), "truecolor") {
            break;
        }
        app.wheel(point, false, 3);
    }
    let top = app.settle();
    assert!(
        band(&top, "truecolor"),
        "truecolor band did not return under the header\n{}",
        text(&top)
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[CRUMB, "○ Compare levels", "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_option_controls(&options, &[]);
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "measured"],
        &["Compare levels", OVERVIEW, "8"],
    );
    app.finish();
}

#[test]
fn meters_refresh_then_refuse_offline() {
    let app = open("s06-meters", 120, 40, PAGE_ARGS, "reduced");
    let before = app.wait_text(SESSION);
    assert_size(&app, 120, 40);
    see(
        &before,
        &[
            LIVE,
            MEASURED,
            "38%",
            "72%",
            "86%",
            DETAIL,
            "Refresh",
            "○ Offline",
        ],
        &[
            OFFLINE_STATUS,
            OFFLINE_META,
            "41%",
            "75%",
            REFRESHED,
            "refreshing",
        ],
    );
    shot(&app, "sc.meters.meters.before");

    click_button(&app, "Refresh");
    app.wait_text("refreshing");
    let done = app.wait_text("41%");
    see(
        &done,
        &[REFRESHED, "41%", "75%", "86%", MEASURED, LIVE],
        &["38%", "72%", "refreshing", OFFLINE_META, "44%", "78%"],
    );
    click_button(&app, "Offline");
    let offline = app.wait_text(OFFLINE_META);
    see(
        &offline,
        &[
            OFFLINE_STATUS,
            OFFLINE_META,
            "41%",
            "75%",
            "● Offline",
            "86%",
        ],
        &["38%", "72%", "44%", "78%", LIVE, MEASURED, "refreshing"],
    );
    click_button(&app, "Refresh");
    let held = app.wait_text(OFFLINE_HOLD);
    see(
        &held,
        &[OFFLINE_HOLD, OFFLINE_META, "41%", "75%", "● Offline"],
        &["44%", "78%", "38%", "72%", "refreshing", REFRESHED, LIVE],
    );
    flash_settle(&app);
    let after = app.settle();
    see(
        &after,
        &[OFFLINE_HOLD, "41%", "75%", OFFLINE_STATUS],
        &["44%", "78%", "refreshing"],
    );
    shot(&app, "sc.meters.meters.after");
    app.finish();
}

fn option_case(
    name: &str,
    case_id: &str,
    opt: &str,
    check_play: impl Fn(&TerminalSnapshot),
    check_opt: impl Fn(&TerminalSnapshot),
) {
    let app = page(name, &[opt], 120, 40);
    let screen = app.wait_text(PLAN);
    check_play(&screen);
    confirm_option(&app, check_opt);
    let shown = app.settle();
    check_play(&shown);
    assert_size(&app, 120, 40);
    shot(&app, case_id);
    app.finish();
}

#[test]
fn option_label_width_8() {
    option_case(
        "s06-label-width",
        "opt.meters.label_width",
        "meters.label_width=8",
        |screen| {
            see(
                screen,
                &[
                    "Session…",
                    "Extra c…",
                    "Opus we…",
                    "Org sea…",
                    "/System…",
                    "38%",
                ],
                &[
                    SESSION,
                    "Extra credits",
                    "Opus weekly",
                    "Org seats",
                    "/System/Volumes/Data",
                ],
            );
        },
        |screen| {
            assert_select(screen, "Label column", "8", &["measured", "12"]);
            assert_option_controls(screen, &["label_width"]);
        },
    );
}

#[test]
fn option_label_gap_3() {
    option_case(
        "s06-label-gap",
        "opt.meters.label_gap",
        "meters.label_gap=3",
        |screen| {
            assert_eq!(
                spaces_between(&row_with(screen, "Weekly"), "Weekly", "72%"),
                12,
                "gap-3 weekly row {}",
                row_with(screen, "Weekly")
            );
            see(screen, &[SESSION, DETAIL, "38%"], &["Session…"]);
        },
        |screen| {
            assert_select(screen, "Label gap", "3", &["1"]);
            assert_option_controls(screen, &["label_gap"]);
        },
    );
}

#[test]
fn option_meter_width_14() {
    option_case(
        "s06-meter-width",
        "opt.meters.meter_width",
        "meters.meter_width=14",
        |screen| {
            let session = row_with(screen, SESSION);
            assert_eq!(
                spaces_between(&session, "%", "resets"),
                11,
                "width-14 session row {session:?}"
            );
            see(screen, &[DETAIL, "72%", "Weekly"], &[]);
        },
        |screen| {
            assert_select(screen, "Meter width", "14", &["24", "fill"]);
            assert_option_controls(screen, &["meter_width"]);
        },
    );
}

#[test]
fn option_detail_off() {
    option_case(
        "s06-detail",
        "opt.meters.detail",
        "meters.detail=off",
        |screen| {
            see(
                screen,
                &[SESSION, "38%", "72%", PLAN, "Filesystems", "CPU"],
                &[
                    DETAIL,
                    "resets Mon 09:00",
                    "310 of 500 GB",
                    "8 cores · load 1.84",
                ],
            );
        },
        |screen| {
            assert_toggle(screen, "Details", "off");
            assert_option_controls(screen, &["detail"]);
        },
    );
}

#[test]
fn option_detail_tone_secondary() {
    option_case(
        "s06-detail-tone",
        "opt.meters.detail_tone",
        "meters.detail_tone=secondary",
        |screen| {
            assert_eq!(fg_of(screen, DETAIL), SECONDARY, "detail ink");
            assert_ne!(fg_of(screen, DETAIL), MUTED, "detail stayed muted");
            see(screen, &[DETAIL, SESSION], &[]);
        },
        |screen| {
            assert_select(screen, "Detail tone", "secondary", &["muted"]);
            assert_option_controls(screen, &["detail_tone"]);
        },
    );
}

#[test]
fn option_visual_line() {
    option_case(
        "s06-visual",
        "opt.meters.visual",
        "meters.visual=line",
        |screen| {
            let session = row_with(screen, SESSION);
            assert!(
                session.contains('━'),
                "line visual left the session row as a block: {session:?}"
            );
            see(screen, &[DETAIL, "38%", "72%"], &[]);
        },
        |screen| {
            assert_select(screen, "Quota meter visual", "line", &["block"]);
            assert_option_controls(screen, &["visual"]);
        },
    );
}

#[test]
fn option_offline_on() {
    option_case(
        "s06-offline",
        "opt.meters.offline",
        "meters.offline=on",
        |screen| {
            see(
                screen,
                &[
                    OFFLINE_STATUS,
                    OFFLINE_META,
                    "38%",
                    "72%",
                    "● Offline",
                    "╍",
                    "▲",
                ],
                &[LIVE, MEASURED, "41%", "refreshing"],
            );
            assert!(
                italic_at(screen, "38%"),
                "dashed stale session percent is not italic\n{}",
                text(screen)
            );
        },
        |screen| {
            assert_toggle(screen, "Offline (last-good)", "on");
            assert_select(screen, "Stale style", "dashed", &["faint"]);
            assert_option_controls(screen, &["offline"]);
        },
    );
}

#[test]
fn option_stale_style_faint() {
    let opt = ["meters.stale_style=faint", "meters.offline=on"];
    let app = page("s06-stale", &opt, 120, 40);
    let screen = app.wait_text(OFFLINE_META);
    see(
        &screen,
        &[OFFLINE_STATUS, OFFLINE_META, "38%", "72%", "● Offline"],
        &[LIVE, MEASURED, "╍", "▲", "41%", "refreshing"],
    );
    assert!(
        !italic_at(&screen, "38%"),
        "faint stale session percent stayed italic\n{}",
        text(&screen)
    );
    let extra = row_with(&screen, "Extra credits");
    assert!(
        !extra.contains('!'),
        "faint stale kept the exhausted mark: {extra:?}"
    );
    confirm_option(&app, |screen| {
        assert_select(screen, "Stale style", "faint", &["dashed"]);
        assert_toggle(screen, "Offline (last-good)", "on");
        assert_option_controls(screen, &["stale_style", "offline"]);
    });
    let shown = app.settle();
    see(&shown, &[OFFLINE_META, "38%"], &["╍", "▲"]);
    assert!(!italic_at(&shown, "38%"), "faint italic returned");
    shot(&app, "opt.meters.stale_style");
    app.finish();
}
