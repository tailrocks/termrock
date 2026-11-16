//! Activity route, sections, specimens, compare levels, run control, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s10_activity/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Screens › Activity";
const PREV: &str = "Screens › Disk cleanup";
const TITLE: &str = "Activity";
const BLURB: &str = "A live job: state clauses, stream chips, a following log and its steps";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "activity"];
const RESOLVED: &str = "resolved node 22.9.0 · pnpm 9.12 · postgres 16";
const COMMAND: &str = "$ mise run dev";
const RUNNING: &str = "running · 12 min 42 s";
const STREAMS: &str = "4 of 4 streams";
const STOPPING: &str = "Stopping api dev";
const STOP_LINE: &str = "stop requested by holla · SIGTERM to the process group";
const STOPPED_LINE: &str = "stopped · exit 143 · descendants reaped";
const STOPPED_HEAD: &str = "stopped · exit 143";
const FINISHED: &str = "api dev stopped · exit 143";
const FAILED_LINE: &str = "error: listen EADDRINUSE: address already in use :::8080";
const FAILED_EXIT: &str = "api exited with code 1";
const RESTARTED: &str = "Restarted api dev";
const KEPT: &str = "following · last 20 kept";
const FIND_ERROR: &str = "2 matches for “error”";
const FIND_JOB: &str = "10 matches for “job”";
const LOG: Point = Point { x: 60, y: 20 };
const WHEEL: Point = Point { x: 70, y: 18 };

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focus + hover",
    "disabled",
    "busy",
    "error",
    "editing",
    "loading",
];

const DRAWN: &[(&str, &str)] = &[
    ("sc.activity.specimen.rest", "rest"),
    ("sc.activity.specimen.focused", "focused"),
    ("sc.activity.specimen.selected", "selected"),
    ("sc.activity.specimen.empty", "empty"),
    ("sc.activity.specimen.overflow", "overflow"),
    ("sc.activity.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s10_activity/{}", case_id.replace('.', "_"))
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

fn assert_activity_options(
    screen: &TerminalSnapshot,
    state: &str,
    wrap: &str,
    retention: &str,
    find: &str,
    digits: &str,
) {
    assert_select(
        screen,
        "Run state",
        state,
        &["running", "stopped", "failed"],
    );
    assert_toggle(screen, "Wrap long lines", wrap);
    assert_select(screen, "Retained lines", retention, &["2000", "20"]);
    assert_select(screen, "Find", find, &["off", "error", "job"]);
    assert_toggle(screen, "Digits toggle streams", digits);
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 8
            && (18..40).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
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
    click_where(app, label, |hit| hit.y > 6 && hit.x >= 26);
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

fn scroll_log(app: &App, up: bool) {
    for _ in 0..12 {
        app.wheel(LOG, !up, 3);
    }
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

fn forbidden_run() -> &'static [&'static str] {
    &[
        "api exited",
        "stopped · exit",
        STOPPING,
        "EADDRINUSE",
        RESTARTED,
        FINISHED,
        STOPPED_LINE,
    ]
}

fn assert_running_wide(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            COMMAND,
            RUNNING,
            STREAMS,
            "streams",
            "api",
            "worker",
            "scheduler",
            "db",
            "GET /health",
            "job 4821",
            "following",
            "Resolve tools",
            "Install dependencies",
            "Migrate database",
            "Start services",
            "Healthy",
            "Stop",
            "Restart",
            "4 of 5",
        ],
        forbidden_run(),
    );
    assert!(
        !flat(screen).contains("matches for"),
        "find=off still marks the log\n{}",
        text(screen)
    );
}

fn assert_running_narrow(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            TITLE,
            "A live job",
            "Playground",
            COMMAND,
            RUNNING,
            "streams",
            "api",
            "worker",
            "scheduler",
            "resize-avatar",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "its steps",
            "api exited",
            STOPPING,
            "EADDRINUSE",
            "stopped · exit",
            RESTARTED,
        ],
    );
}

fn assert_stopped(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            STOPPED_HEAD,
            STOP_LINE,
            STOPPED_LINE,
            "Exit",
            "143",
            "up to date",
            "5 of 5",
            COMMAND,
        ],
        &[
            STOPPING,
            FINISHED,
            "EADDRINUSE",
            FAILED_EXIT,
            RESTARTED,
            RUNNING,
            "failed · exit",
        ],
    );
}

fn assert_failed(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            "failed · exit 1",
            FAILED_LINE,
            FAILED_EXIT,
            "listen EADDRINUSE :::8080",
            "Free the port, then Restart",
            "exit 1",
            "blocked",
        ],
        &[
            STOPPED_LINE,
            STOPPED_HEAD,
            STOPPING,
            FINISHED,
            RESTARTED,
            RUNNING,
            "up to date",
        ],
    );
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(COMMAND);
    flash_settle(app);
}

fn focus_hint(app: &App, hint: &str) {
    for _ in 0..16 {
        if text(&app.settle()).contains(hint) {
            return;
        }
        app.key("tab");
    }
    panic!("hint {hint:?} missing\n{}", text(&app.screen()));
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s10-route-page", 120, 40, PAGE_ARGS);
    app.wait_text(CRUMB);
    flash_settle(&app);
    let screen = app.settle();
    assert_size(&app, 120, 40);
    assert_running_wide(&screen);
    shot(&app, "sc.activity.route");
    scroll_log(&app, true);
    let older = app.wait_text(RESOLVED);
    see(
        &older,
        &[RESOLVED, COMMAND, "GET /health"],
        &["api exited", "EADDRINUSE", STOPPED_LINE],
    );
    app.finish();

    let app = open("s10-route-keys", 120, 40, &[]);
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
        "Text & code › Terminal",
        "Text & code › Terminal pane",
        "Text & code › Terminal viewport",
        "Text & code › Text utilities",
        "Screens › Settings",
        "Screens › Task runner",
        "Screens › Query workbench",
        "Screens › Connections",
        "Screens › Finder",
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(RUNNING);
    see(
        &entered,
        &[CRUMB, BLURB, COMMAND, "Healthy"],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, BLURB, RUNNING]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, RUNNING, "streams"], &[PREV, OVERVIEW]);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, RUNNING]);
    let screen = app.settle();
    if !find_all(&screen, TITLE)
        .into_iter()
        .any(|hit| sidebar(&hit))
    {
        click_side(&app, "Screens");
    }
    click_side(&app, TITLE);
    let pointed = app.wait_text(RUNNING);
    see(
        &pointed,
        &[CRUMB, BLURB, COMMAND, STREAMS, "Stop"],
        &[
            OVERVIEW,
            "Page options",
            "Compare levels",
            STOPPING,
            "EADDRINUSE",
        ],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s10-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(RUNNING);
    assert_size(&app, 80, 24);
    assert_running_narrow(&narrow_play);
    shot(&app, "sc.activity.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_activity_options(&narrow_opt, "running", "on", "2000", "off", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Run state", "Digits toggle streams"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            RUNNING,
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.activity.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "focused", "selected"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "Log", "Streams"],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            RUNNING,
            WIDE,
            "truecolor",
        ],
    );
    assert_omitted(&narrow_states);
    shot(&app, "sc.activity.states.80x24");

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
            "GET /health",
            "a log line far wider",
            "+3",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            RUNNING,
            "256 colors",
        ],
    );
    assert_omitted(&wide_states);
    for (case_id, label) in DRAWN {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    let empty = row_with(&app.screen(), "empty");
    assert!(
        empty.contains('\u{2013}'),
        "empty specimen is not the unsupported dash: {empty:?}"
    );
    shot(&app, "sc.activity.states.120x40");
    shot(&app, "sc.activity.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_activity_options(&wide_opt, "running", "on", "2000", "off", "on");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Wrap long lines"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", RUNNING],
    );
    shot(&app, "sc.activity.options.120x40");
    shot(&app, "sc.activity.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(RUNNING);
    assert_size(&app, 120, 40);
    assert_running_wide(&wide_play);
    shot(&app, "sc.activity.playground.120x40");
    shot(&app, "sc.activity.playground");
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
    shot(&app, "sc.activity.compare-levels.120x40");

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
    shot(&app, "sc.activity.compare-levels.80x24");

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
    assert_activity_options(&options, "running", "on", "2000", "off", "on");
    see(
        &options,
        &["running", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "stopped"],
    );
    app.finish();
}

#[test]
fn stop_before_it_finishes_and_restart() {
    let app = open("s10-activity", 120, 40, PAGE_ARGS);
    let running = app.wait_text(RUNNING);
    assert_size(&app, 120, 40);
    assert_running_wide(&running);
    shot(&app, "sc.activity.activity.before");

    click_button(&app, "Stop");
    app.wait_text(STOPPING);
    flash_settle(&app);
    let stopping = app.settle();
    see(
        &stopping,
        &[
            CRUMB,
            STOPPING,
            "stopping · SIGTERM sent",
            STOP_LINE,
            "stopping",
            COMMAND,
            "3 of 5",
        ],
        &[
            FINISHED,
            STOPPED_HEAD,
            STOPPED_LINE,
            RESTARTED,
            "EADDRINUSE",
            RUNNING,
        ],
    );
    shot(&app, "sc.activity.activity.after");

    click_button(&app, "Restart");
    flash_settle(&app);
    let ignored = app.settle();
    see(
        &ignored,
        &[STOPPING, "stopping · SIGTERM sent", STOP_LINE],
        &[RESTARTED, FINISHED, STOPPED_HEAD, STOPPED_LINE, RUNNING],
    );
    app.finish();

    let app = page("s10-restart", Some("activity.state=stopped"), 120, 40);
    let stopped = app.wait_text(STOPPED_LINE);
    assert_stopped(&stopped);
    click_button(&app, "Restart");
    app.wait_text(RESTARTED);
    flash_settle(&app);
    let restarted = app.settle();
    see(
        &restarted,
        &[RESTARTED, RUNNING, STREAMS, "GET /health", "following"],
        &[
            STOPPED_LINE,
            STOPPED_HEAD,
            STOP_LINE,
            "EADDRINUSE",
            FAILED_EXIT,
            STOPPING,
        ],
    );
    app.finish();
}

#[test]
fn option_state_stopped_and_failed() {
    let app = page("s10-stopped", Some("activity.state=stopped"), 120, 40);
    app.wait_text(STOPPED_LINE);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "stopped", "on", "2000", "off", "on");
    });
    let stopped = app.settle();
    assert_size(&app, 120, 40);
    assert_stopped(&stopped);
    shot(&app, "opt.activity.state");
    app.finish();

    let app = page("s10-failed", Some("activity.state=failed"), 120, 40);
    let failed = app.wait_text(FAILED_LINE);
    assert_size(&app, 120, 40);
    assert_failed(&failed);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "failed", "on", "2000", "off", "on");
    });
    assert_failed(&app.settle());
    app.finish();
}

#[test]
fn option_wrap_off() {
    let app = page("s10-wrap", Some("activity.wrap=off"), 120, 40);
    app.wait_text(RUNNING);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "running", "off", "2000", "off", "on");
    });
    let play = app.settle();
    assert_running_wide(&play);
    assert!(
        !flat(&play).contains("last 20 kept"),
        "wrap changed retention\n{}",
        text(&play)
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let screen = app.settle();
    assert_activity_options(&screen, "running", "off", "2000", "off", "on");
    assert_size(&app, 120, 40);
    see(
        &screen,
        &["Wrap long lines", "Page options", CRUMB],
        &[RUNNING, "Compare levels", "last 20 kept"],
    );
    let row = row_with(&screen, "Wrap long lines");
    assert!(
        row.contains("off") && !row.contains(" on"),
        "wrap control is not off: {row:?}"
    );
    shot(&app, "opt.activity.wrap");
    app.finish();
}

#[test]
fn option_retention_20() {
    let app = page("s10-retention", Some("activity.retention=20"), 120, 40);
    app.wait_text(KEPT);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "running", "on", "20", "off", "on");
    });
    let kept = app.wait_text(KEPT);
    assert_size(&app, 120, 40);
    see(
        &kept,
        &[
            KEPT,
            RUNNING,
            "job 4823 send-digest done in 233 ms",
            STREAMS,
        ],
        &["last 2000 kept", RESOLVED, STOPPED_LINE, "EADDRINUSE"],
    );
    shot(&app, "opt.activity.retention");
    scroll_log(&app, true);
    let top = app.settle();
    assert!(
        !text(&top).contains(RESOLVED) && !text(&top).contains("postgres 16"),
        "retention 20 still shows the trimmed preamble\n{}",
        text(&top)
    );
    see(
        &top,
        &["GET /health", "job 4821"],
        &[RESOLVED, "last 2000 kept"],
    );
    app.finish();
}

#[test]
fn option_find_error_and_job() {
    let app = page("s10-find-error", Some("activity.find=error"), 120, 40);
    app.wait_text(FIND_ERROR);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "running", "on", "2000", "error", "on");
    });
    let marked = app.wait_text(FIND_ERROR);
    assert_size(&app, 120, 40);
    see(
        &marked,
        &[FIND_ERROR, "error SMTP 421", RUNNING],
        &["matches for “job”", "matches for “off”", STOPPED_LINE],
    );
    shot(&app, "opt.activity.find");
    app.finish();

    let app = page("s10-find-job", Some("activity.find=job"), 120, 40);
    let marked = app.wait_text(FIND_JOB);
    see(
        &marked,
        &[FIND_JOB, "job 4821", RUNNING],
        &[FIND_ERROR, "matches for “error”"],
    );
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "running", "on", "2000", "job", "on");
    });
    assert!(text(&app.settle()).contains(FIND_JOB));
    app.finish();

    let app = page("s10-find-off", Some("activity.find=off"), 120, 40);
    let clear = app.wait_text("following");
    assert_running_wide(&clear);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "running", "on", "2000", "off", "on");
    });
    app.finish();
}

#[test]
fn option_digit_toggles() {
    let app = page(
        "s10-digits-off",
        Some("activity.digit_toggles=off"),
        120,
        40,
    );
    app.wait_text(RUNNING);
    confirm_options(&app, |screen| {
        assert_activity_options(screen, "running", "on", "2000", "off", "off");
    });
    focus_hint(&app, "Show / hide");
    assert!(
        !text(&app.screen()).contains("Toggle stream"),
        "digits off still advertises the digit hint\n{}",
        text(&app.screen())
    );
    app.key("1");
    let idle = app.settle();
    see(
        &idle,
        &[STREAMS, "GET /health", "job 4821", RUNNING],
        &["api hidden", "3 of 4 streams", STOPPING],
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let screen = app.settle();
    assert_activity_options(&screen, "running", "on", "2000", "off", "off");
    assert_size(&app, 120, 40);
    see(
        &screen,
        &["Digits toggle streams", "Page options", CRUMB],
        &[RUNNING, "api hidden", "Compare levels"],
    );
    shot(&app, "opt.activity.digit_toggles");
    app.finish();

    let app = open("s10-digits-on", 120, 40, PAGE_ARGS);
    app.wait_text(RUNNING);
    focus_hint(&app, "Toggle stream");
    app.key("1");
    let hidden = app.wait_text("api hidden");
    see(
        &hidden,
        &["api hidden", "3 of 4 streams", "job 4821", "worker"],
        &["GET /health", STREAMS, "POST /v1/orders", STOPPING],
    );
    app.finish();
}

#[test]
fn option_feed_channel_keeps_input_live_and_shows_saturation() {
    let app = page("s10-feed-channel", Some("activity.feed=channel"), 120, 40);
    let live = app.wait_text("Full retries");
    see(&live, &["Feed sent", "Handled", RUNNING], &[]);
    let fed = app.wait_text("feed #");
    assert!(
        text(&fed).contains("· feed #"),
        "no fed line is visible\n{}",
        text(&fed)
    );
    focus_hint(&app, "Show / hide");
    app.key("1");
    let hidden = app.wait_text("api hidden");
    see(
        &hidden,
        &["api hidden", "3 of 4 streams", "Full retries"],
        &[STOPPING],
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    assert_select(
        &app.settle(),
        "Log feed",
        "channel",
        &["simulated", "channel"],
    );
    app.finish();

    let app = open("s10-feed-simulated", 120, 40, PAGE_ARGS);
    app.wait_text(RUNNING);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let screen = app.settle();
    assert_select(&screen, "Log feed", "simulated", &["simulated", "channel"]);
    see(&screen, &["Log feed"], &["Full retries"]);
    app.finish();
}
