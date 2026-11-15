//! Read-only report route, sections, specimens, compare levels, refresh, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s10_readonlyreport/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Screens › Read-only report";
const PREV: &str = "Screens › Launch cockpit";
const TITLE: &str = "Read-only report";
const BLURB_HEAD: &str = "A host snapshot: live meta, MeterList";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "readonlyreport"];
const META_LIVE: &str = "live · 12 s ago";
const META_NOW: &str = "live · just now";
const META_REFRESH: &str = "refreshing…";
const META_OFFLINE: &str = "offline · last-good 2 m ago";
const REFRESHING: &str = "Refreshing the snapshot…";
const REFRESHED: &str = "Snapshot refreshed";
const OFFLINE_STATUS: &str = "Offline: build-07 does not answer";
const OFFLINE_NOTICE: &str = "Offline: showing the last-good snapshot";
const OFFLINE_DETAIL: &str = "build-07 stopped answering 2 m ago";
const NOTICE_FULL: &str = "/Volumes/Backup is 97 % full";
const NOTICE_INFO: &str = "This report is read-only";
const HOST: &str = "build-07.acme.internal";
const COPIED_HOST: &str = "Copied Host";
const COPIED_REPORT: &str = "Copied the report (13 lines)";
const HOST_STATUS: &str = "Host: build-07.acme.internal";
const DISK: &str = "Read-only report › Disk cleanup";
const ACTIVITY: &str = "Read-only report › Activity";
const WHEEL: Point = Point { x: 70, y: 18 };

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

const DRAWN: &[(&str, &str)] = &[
    ("sc.readonlyreport.specimen.rest", "rest"),
    ("sc.readonlyreport.specimen.focused", "focused"),
    ("sc.readonlyreport.specimen.loading", "loading"),
    ("sc.readonlyreport.specimen.error", "error"),
    ("sc.readonlyreport.specimen.overflow", "overflow"),
    ("sc.readonlyreport.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s10_readonlyreport/{}",
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
        .find(|(row, line)| *row >= 6 && line.contains(label) && !line.contains("Finder"))
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
        if *other == value {
            continue;
        }
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

fn assert_report_options(
    screen: &TerminalSnapshot,
    offline: &str,
    visual: &str,
    notices: &str,
    copyable: &str,
) {
    assert_toggle(screen, "Offline (last-good)", offline);
    assert_select(screen, "Meter visual", visual, &["block", "line"]);
    assert_toggle(screen, "Attention notices", notices);
    assert_toggle(screen, "Facts copy", copyable);
}

fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 8
            && (18..40).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    grid_hit(screen, label).is_some()
}

fn grid_row(screen: &TerminalSnapshot, label: &str) -> String {
    let hit = grid_hit(screen, label)
        .unwrap_or_else(|| panic!("grid label {label:?} missing\n{}", text(screen)));
    row_text(screen, u32::from(hit.y))
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

fn assert_drawn(screen: &TerminalSnapshot) {
    for (_, label) in DRAWN {
        assert!(
            grid_label(screen, label),
            "{label} is not in the states grid\n{}",
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
    hit.x < 24
}

fn click_side(app: &App, label: &str) {
    reveal_side(app, label);
    click_where(app, label, sidebar);
}

fn click_button(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| {
            hit.y > 6 && hit.x >= 26 && row_text(&screen, u32::from(hit.y)).contains("Copy report")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one button {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn reveal_side(app: &App, label: &str) {
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

fn reveal_text(app: &App, needle: &str) {
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(WHEEL, true, 3);
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

fn assert_idle_wide(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB_HEAD,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "Host snapshot",
            META_LIVE,
            "CPU",
            "23%",
            "8 cores · load 1.84 1.62 1.40",
            "Memory",
            "68%",
            "21.8 of 32 GB · 3.1 GB cached",
            "Swap",
            "0.2 of 4 GB",
            "310 of 500 GB",
            "1.94 of 2 TB",
            "Facts",
            "y copies",
            HOST,
            "10.4.12.37",
            "macOS 15.1 (24B83) · arm64",
            "6 d 4 h 12 m",
            "412 · 3 zombie",
            "en0 · 1 Gbit/s · 12.4 MB/s in",
            "snap-2026-10-17T09-41-08Z",
            NOTICE_FULL,
            NOTICE_INFO,
            "Refresh",
            "Copy report",
            "Disk cleanup →",
            "Activity →",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            META_REFRESH,
            REFRESHING,
            REFRESHED,
            META_NOW,
            META_OFFLINE,
            OFFLINE_NOTICE,
            OFFLINE_STATUS,
            COPIED_HOST,
            COPIED_REPORT,
            HOST_STATUS,
        ],
    );
    let cpu = row_with(screen, "CPU");
    assert!(
        cpu.contains("23%") && !cpu.contains('━') && !cpu.contains('─'),
        "block meter drew a line track: {cpu:?}"
    );
    let facts = row_with(screen, "Facts");
    assert!(
        facts.contains("y copies") && !facts.contains("read-only"),
        "facts meta is not y copies: {facts:?}"
    );
}

fn assert_idle_narrow(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            TITLE,
            CRUMB,
            BLURB_HEAD,
            "Playground",
            "Host snapshot",
            "CPU",
            "23%",
            "Facts",
            HOST,
            "10.4.12.37",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            META_LIVE,
            "y copies",
            NOTICE_FULL,
            NOTICE_INFO,
            "Refresh",
            "Copy report",
            META_REFRESH,
            REFRESHING,
            REFRESHED,
        ],
    );
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text("Host snapshot");
    flash_settle(app);
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s10-route-page", 120, 40, PAGE_ARGS);
    app.wait_text(CRUMB);
    flash_settle(&app);
    let screen = app.settle();
    assert_size(&app, 120, 40);
    assert_idle_wide(&screen);
    shot(&app, "sc.readonlyreport.route");
    app.finish();

    let app = open("s10-route-keys", 120, 40, &["--page", "launchcockpit"]);
    app.wait_text(PREV);
    app.key("]");
    let entered = app.wait_text(META_LIVE);
    see(
        &entered,
        &[CRUMB, BLURB_HEAD, "y copies", HOST, "Refresh"],
        &[PREV, OVERVIEW, "Page options", META_REFRESH, REFRESHING],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(
        &left,
        &[PREV, "Launch cockpit"],
        &[CRUMB, META_LIVE, "y copies", NOTICE_INFO],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, META_LIVE, "Copy report"],
        &[PREV, OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, META_LIVE, BLURB_HEAD]);
    let screen = app.settle();
    if !find_all(&screen, TITLE)
        .into_iter()
        .any(|hit| sidebar(&hit))
    {
        click_side(&app, "Screens");
    }
    click_side(&app, TITLE);
    let pointed = app.wait_text(META_LIVE);
    see(
        &pointed,
        &[CRUMB, BLURB_HEAD, HOST, NOTICE_FULL, "Activity →"],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            PREV,
            META_REFRESH,
        ],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s10-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(HOST);
    assert_size(&app, 80, 24);
    assert_idle_narrow(&narrow_play);
    shot(&app, "sc.readonlyreport.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_report_options(&narrow_opt, "off", "block", "on", "on");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Offline (last-good)",
            "Meter visual",
            "Attention notices",
            "Facts copy",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            META_LIVE,
            WIDE,
            "truecolor",
            HOST,
            "y copies",
        ],
    );
    shot(&app, "sc.readonlyreport.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let mut narrow_states = app.settle();
    assert_size(&app, 80, 24);
    if DRAWN
        .iter()
        .any(|(_, label)| !grid_label(&narrow_states, label))
    {
        let point = Point { x: 40, y: 14 };
        for _ in 0..40 {
            narrow_states = app.settle();
            if DRAWN
                .iter()
                .all(|(_, label)| grid_label(&narrow_states, label))
            {
                break;
            }
            app.wheel(point, true, 3);
        }
        narrow_states = app.settle();
    }
    assert_size(&app, 80, 24);
    assert_drawn(&narrow_states);
    assert_omitted(&narrow_states);
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
            "narrow",
            "Meters",
            "Facts",
            "read failed",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            META_LIVE,
            WIDE,
            "truecolor",
            "y copies",
            NOTICE_FULL,
        ],
    );
    shot(&app, "sc.readonlyreport.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
    });
    assert_size(&app, 120, 40);
    assert_drawn(&wide_states);
    assert_omitted(&wide_states);
    see(
        &wide_states,
        &[
            CRUMB,
            TITLE,
            BLURB_HEAD,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "Meters",
            "Facts",
            "read failed",
            "Loading rows",
            "CPU and a longer name",
            "8 cores",
            "2 TB",
            "build-07",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            META_LIVE,
            "256 colors",
            NOTICE_FULL,
        ],
    );
    let focused = grid_row(&wide_states, "focused");
    assert!(
        focused.contains('\u{2013}'),
        "focused specimen is not the unsupported dash: {focused:?}"
    );
    shot(&app, "sc.readonlyreport.states.120x40");
    shot(&app, "sc.readonlyreport.states");
    for (case_id, label) in DRAWN {
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
    assert_report_options(&wide_opt, "off", "block", "on", "on");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Meter visual"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", META_LIVE, HOST],
    );
    shot(&app, "sc.readonlyreport.options.120x40");
    shot(&app, "sc.readonlyreport.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(META_LIVE);
    assert_size(&app, 120, 40);
    assert_idle_wide(&wide_play);
    shot(&app, "sc.readonlyreport.playground.120x40");
    shot(&app, "sc.readonlyreport.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s10-compare", 120, 40, PAGE_ARGS);
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
        &[CRUMB, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    for _ in 0..80 {
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
    shot(&app, "sc.readonlyreport.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    let settled = app.settle();
    if !text(&settled).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.readonlyreport.compare-levels.80x24");

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
    assert_report_options(&options, "off", "block", "on", "on");
    see(
        &options,
        &["block", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW],
    );
    app.finish();
}

#[test]
fn refresh_copy_and_next_pages() {
    let app = open("s10-report", 120, 40, PAGE_ARGS);
    let idle = app.wait_text(META_LIVE);
    assert_size(&app, 120, 40);
    assert_idle_wide(&idle);

    click_button(&app, "Refresh");
    app.wait_text(REFRESHING);
    flash_settle(&app);
    let refreshing = app.settle();
    assert_size(&app, 120, 40);
    see(
        &refreshing,
        &[
            CRUMB,
            META_REFRESH,
            REFRESHING,
            "⠋ refreshing",
            HOST,
            "y copies",
            NOTICE_FULL,
            "Copy report",
        ],
        &[META_NOW, META_LIVE, REFRESHED, OFFLINE_STATUS, COPIED_HOST],
    );
    shot(&app, "sc.readonlyreport.report.before");

    click_where(&app, HOST, |hit| hit.x >= 26);
    app.key("enter");
    app.wait_text(COPIED_HOST);
    flash_settle(&app);
    let copied = app.settle();
    see(
        &copied,
        &[COPIED_HOST, META_REFRESH, HOST],
        &[HOST_STATUS, REFRESHED, META_NOW, REFRESHING, "Not copied"],
    );

    click_button(&app, "Copy report");
    app.wait_text(COPIED_REPORT);
    flash_settle(&app);
    let report = app.settle();
    see(
        &report,
        &[COPIED_REPORT, META_REFRESH, "Copy report"],
        &[
            REFRESHED,
            META_NOW,
            HOST_STATUS,
            REFRESHING,
            COPIED_HOST,
            "Not copied",
        ],
    );

    click_button(&app, "Disk cleanup");
    app.move_to(Point { x: 0, y: 0 });
    app.settle();
    let disk = app.wait_text(DISK);
    see(
        &disk,
        &[DISK, "Disk cleanup", "Usage tree"],
        &[CRUMB, REFRESHING, META_REFRESH, NOTICE_INFO],
    );
    app.key("esc");
    let back = app.wait_text(META_REFRESH);
    see(
        &back,
        &[CRUMB, META_REFRESH, HOST, "Refresh"],
        &[DISK, REFRESHED, META_NOW, META_LIVE],
    );
    shot(&app, "sc.readonlyreport.report.after");

    click_button(&app, "Activity");
    let activity = app.wait_text(ACTIVITY);
    see(
        &activity,
        &[ACTIVITY, "Activity", "A live job"],
        &[CRUMB, REFRESHING, META_REFRESH, NOTICE_FULL],
    );
    app.finish();
}

#[test]
fn option_offline_refuses_refresh() {
    let app = page("s10-offline", Some("readonlyreport.offline=on"), 120, 40);
    let offline = app.wait_text(META_OFFLINE);
    assert_size(&app, 120, 40);
    see(
        &offline,
        &[
            CRUMB,
            META_OFFLINE,
            OFFLINE_NOTICE,
            OFFLINE_DETAIL,
            "23%",
            NOTICE_FULL,
            NOTICE_INFO,
            HOST,
        ],
        &[
            META_LIVE,
            META_REFRESH,
            REFRESHING,
            REFRESHED,
            OFFLINE_STATUS,
        ],
    );
    confirm_options(&app, |screen| {
        assert_report_options(screen, "on", "block", "on", "on");
    });
    click_button(&app, "Refresh");
    app.wait_text(OFFLINE_STATUS);
    flash_settle(&app);
    let refused = app.settle();
    see(
        &refused,
        &[
            CRUMB,
            META_OFFLINE,
            OFFLINE_STATUS,
            OFFLINE_NOTICE,
            "23%",
            HOST,
        ],
        &[
            META_REFRESH,
            REFRESHING,
            REFRESHED,
            META_NOW,
            META_LIVE,
            "⠋ refreshing",
        ],
    );
    let cpu = row_with(&refused, "CPU");
    assert!(
        cpu.contains("23%") && !cpu.contains('━'),
        "offline refresh changed the block meter: {cpu:?}"
    );
    shot(&app, "opt.readonlyreport.offline");
    app.finish();
}

#[test]
fn option_visual_line() {
    let app = page("s10-line", Some("readonlyreport.visual=line"), 120, 40);
    app.wait_text("CPU");
    confirm_options(&app, |screen| {
        assert_report_options(screen, "off", "line", "on", "on");
    });
    let screen = app.settle();
    assert_size(&app, 120, 40);
    let cpu = row_with(&screen, "CPU");
    assert!(
        cpu.contains('━') && cpu.contains('─') && cpu.contains("23%"),
        "line meter is not a ━/─ track: {cpu:?}"
    );
    see(
        &screen,
        &[CRUMB, META_LIVE, "y copies", HOST, NOTICE_FULL],
        &[
            META_OFFLINE,
            META_REFRESH,
            REFRESHING,
            "Page options",
            REFRESHED,
        ],
    );
    shot(&app, "opt.readonlyreport.visual");
    app.finish();
}

#[test]
fn option_notices_off() {
    let app = page("s10-notices", Some("readonlyreport.notices=off"), 120, 40);
    app.wait_text(META_LIVE);
    confirm_options(&app, |screen| {
        assert_report_options(screen, "off", "block", "off", "on");
    });
    let screen = app.settle();
    assert_size(&app, 120, 40);
    see(
        &screen,
        &[CRUMB, META_LIVE, "y copies", HOST, "Refresh", "23%"],
        &[
            NOTICE_FULL,
            NOTICE_INFO,
            "Time Machine",
            OFFLINE_NOTICE,
            "Page options",
            META_REFRESH,
        ],
    );
    shot(&app, "opt.readonlyreport.notices");
    app.finish();
}

#[test]
fn option_copyable_off() {
    let app = page("s10-copyable", Some("readonlyreport.copyable=off"), 120, 40);
    app.wait_text("read-only");
    confirm_options(&app, |screen| {
        assert_report_options(screen, "off", "block", "on", "off");
    });
    let before = app.settle();
    let facts = row_with(&before, "Facts");
    assert!(
        facts.contains("read-only") && !facts.contains("y copies"),
        "facts meta is not read-only: {facts:?}"
    );
    click_where(&app, HOST, |hit| hit.x >= 26);
    app.key("enter");
    app.wait_text(HOST_STATUS);
    flash_settle(&app);
    let screen = app.settle();
    assert_size(&app, 120, 40);
    see(
        &screen,
        &[CRUMB, HOST_STATUS, "read-only", META_LIVE, HOST],
        &[
            COPIED_HOST,
            COPIED_REPORT,
            "y copies",
            "Not copied",
            REFRESHING,
        ],
    );
    shot(&app, "opt.readonlyreport.copyable");
    app.finish();
}
