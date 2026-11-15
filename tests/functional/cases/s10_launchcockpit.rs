//! Launch cockpit route, sections, specimens, compare levels, launch, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s10_launchcockpit/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Screens › Launch cockpit";
const PREV: &str = "Screens › Accounts & usage";
const NEXT: &str = "Screens › Read-only report";
const TITLE: &str = "Launch cockpit";
const BLURB_HEAD: &str = "A launch: stage rail";
const BLURB_WIDE: &str = "failure dialogs";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "launchcockpit"];
const READY_HEAD: &str = "Ready. Launch builds";
/// The wide log wraps inside `payments-platform.`, so the remainder is its own row.
const READY_REST: &str = "latform.";
const TARGET: &str = "payments-platform · role backend";
const LAUNCHING: &str = "Launching payments-platform…";
const LAUNCH_LINE: &str = "jackin launch backend";
const CANCEL_BTN: &str = "Cancel launch…";
const KEEP: &str = "Keep launching";
const CANCEL_TITLE: &str = "Cancel launch?";
const CANCELLED: &str = "Launch cancelled";
const CANCELLED_AT: &str = "Launch cancelled at Identity";
const CRED_TITLE: &str = "Credential source unavailable";
const CRED_LOG: &str = "1Password: the vault Engineering is locked";
const CRED_RETRY: &str = "retry · 1Password unlocked";
const BUILD_TITLE: &str = "Docker build failed";
const BUILD_LOG: &str = "ledger::v2";
const SUCCESS_LOG: &str = "Hardline open · capsule ready";
const SUCCESS_STATUS: &str = "Launched ✓ capsule cap-payments-01";
const FOCUS: &str = "▎";
const RUN_ID: &str = "run-7f3a91c2";

fn visual_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s10_launchcockpit/{}",
        case_id.replace('.', "_")
    )
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str], motion: Option<&str>) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = motion.map(str::to_string);
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Components");
    app
}

fn page(name: &str, opts: &[&str], cols: u16, rows: u16, motion: Option<&str>) -> App {
    let mut args = PAGE_ARGS.to_vec();
    for value in opts {
        args.extend(["--opt", value]);
    }
    open(name, cols, rows, &args, motion)
}

/// The dialog title paints before the Retry button.
fn credentials_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRED_TITLE) && visible.contains(CRED_LOG) && visible.contains("Retry")
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
        .find(|token| *token != "▎" && *token != "▏")
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

fn assert_launch_options(
    screen: &TerminalSnapshot,
    outcome: &str,
    numbered: &str,
    selectable: &str,
    wrap: &str,
) {
    assert_one(
        screen,
        "Launch outcome",
        outcome,
        &["success", "credentials", "build-failure"],
    );
    assert_toggle(screen, "Numbered stages", numbered);
    assert_toggle(screen, "Stages take focus", selectable);
    assert_toggle(screen, "Wrap the log", wrap);
}

fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 8
            && (18..42).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    grid_hit(screen, label).is_some()
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

fn before_label(screen: &TerminalSnapshot, label: &str) -> String {
    let line = row_with(screen, label);
    let at = line.find(label).expect(label);
    let chars: Vec<char> = line.chars().collect();
    let index = line[..at].chars().count();
    let start = index.saturating_sub(8);
    chars[start..index].iter().collect()
}

fn click_launch(app: &App) {
    click_where(app, "Launch", |hit| {
        let row = row_text(&app.screen(), u32::from(hit.y));
        hit.x > 24
            && row.contains(CANCEL_BTN)
            && !row.contains("Launch again")
            && !row.contains("cockpit")
    });
}

fn assert_idle(screen: &TerminalSnapshot, wide: bool) {
    let mut present = vec![
        TITLE,
        BLURB_HEAD,
        "Playground",
        "Options",
        "States",
        TARGET,
        READY_HEAD,
        RUN_ID,
        "Launch",
    ];
    if wide {
        present.extend([
            CRUMB,
            BLURB_WIDE,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "starts the capsule",
            READY_REST,
            "not started",
            "0 of 11 stages",
            CANCEL_BTN,
        ]);
    } else {
        present.push("payments-platform.");
    }
    see(
        screen,
        &present,
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            LAUNCHING,
            LAUNCH_LINE,
            SUCCESS_LOG,
            SUCCESS_STATUS,
            CRED_TITLE,
            BUILD_TITLE,
            CANCELLED,
            CANCEL_TITLE,
            "80×24",
            "alexey@build-07",
            "01 Identity",
        ],
    );
}

fn assert_running_paused(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            LAUNCHING,
            LAUNCH_LINE,
            "Resolving identity",
            "Launch again",
            CANCEL_BTN,
            TARGET,
            "0 of 11 stages",
        ],
        &[
            READY_HEAD,
            SUCCESS_LOG,
            SUCCESS_STATUS,
            CRED_TITLE,
            BUILD_TITLE,
            CANCELLED,
            CANCEL_TITLE,
            "alexey@build-07",
            "0.4 s",
            "Hardline open",
            "Launched ✓",
        ],
    );
}

fn assert_not_success(screen: &TerminalSnapshot) {
    see(
        screen,
        &[],
        &[SUCCESS_LOG, SUCCESS_STATUS, "Hardline open", "Launched ✓"],
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = page("s10-route-page", &[], 120, 40, Some("paused"));
    let screen = app.wait_text(READY_HEAD);
    assert_size(&app, 120, 40);
    assert_idle(&screen, true);
    shot(&app, "sc.launchcockpit.route");
    app.finish();

    let app = open("s10-route-keys", 120, 40, &[], Some("paused"));
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
        "Screens › Disk cleanup",
        "Screens › Activity",
        "Screens › Workspaces",
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(READY_HEAD);
    see(
        &entered,
        &[CRUMB, TITLE, BLURB_HEAD, TARGET, READY_HEAD, READY_REST],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, READY_HEAD]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, READY_HEAD, READY_REST, TARGET],
        &[PREV, OVERVIEW],
    );
    app.key("]");
    let next = app.wait_text(NEXT);
    see(&next, &[NEXT], &[CRUMB, READY_HEAD]);
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, READY_HEAD, BLURB_HEAD]);
    click_side(&app, "Screens");
    click_side(&app, "Launch cockpit");
    let pointed = app.wait_text(READY_HEAD);
    see(
        &pointed,
        &[
            CRUMB, TITLE, BLURB_WIDE, TARGET, READY_HEAD, READY_REST, RUN_ID,
        ],
        &[OVERVIEW, "Page options", "Compare levels", PREV, LAUNCHING],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = page("s10-sections", &[], 80, 24, Some("paused"));
    let narrow_play = app.wait_text(TITLE);
    assert_size(&app, 80, 24);
    assert_idle(&narrow_play, false);
    see(
        &narrow_play,
        &[TITLE, "Playground"],
        &[CAP_PLAY, CAP_OPT, CAP_STATES, WIDE, "truecolor", "80×24"],
    );
    shot(&app, "sc.launchcockpit.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_launch_options(&narrow_opt, "success", "off", "off", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Launch outcome", "Wrap the log"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            READY_HEAD,
            "focus + hover",
        ],
    );
    shot(&app, "sc.launchcockpit.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    assert!(
        grid_label(&narrow_states, "rest"),
        "narrow states clipped rest\n{}",
        text(&narrow_states)
    );
    for label in [
        "hover",
        "pressed",
        "focus + hover",
        "selected",
        "disabled",
        "busy",
        "error",
        "editing",
        "loading",
    ] {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is on the 80 by 24 states screen\n{}",
            text(&narrow_states)
        );
    }
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "Stage rail"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            LEGEND,
            READY_HEAD,
            "focus + hover",
        ],
    );
    shot(&app, "sc.launchcockpit.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
    });
    assert_size(&app, 120, 40);
    for label in ["rest", "focused", "empty", "overflow", "narrow"] {
        assert!(
            grid_label(&wide_states, label),
            "wide states missing {label}\n{}",
            text(&wide_states)
        );
    }
    for label in [
        "hover",
        "pressed",
        "focus + hover",
        "selected",
        "disabled",
        "busy",
        "error",
        "editing",
        "loading",
    ] {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is on the states grid\n{}",
            text(&wide_states)
        );
    }
    see(
        &wide_states,
        &[
            CRUMB,
            TITLE,
            BLURB_WIDE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "Stage rail",
            "Failed launch",
            "Derived Image",
            "01",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            READY_HEAD,
            "focus + hover",
            "256 colors",
        ],
    );
    for (case_id, label) in [
        ("sc.launchcockpit.specimen.rest", "rest"),
        ("sc.launchcockpit.specimen.focused", "focused"),
        ("sc.launchcockpit.specimen.empty", "empty"),
        ("sc.launchcockpit.specimen.overflow", "overflow"),
        ("sc.launchcockpit.specimen.narrow", "narrow"),
    ] {
        if !grid_label(&app.screen(), label) {
            reveal_text(&app, label);
        }
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.launchcockpit.states.120x40");
    shot(&app, "sc.launchcockpit.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_launch_options(&wide_opt, "success", "off", "off", "on");
    see(
        &wide_opt,
        &[
            CRUMB,
            CAP_OPT,
            WIDE,
            "Page options",
            "Launch outcome",
            "success",
        ],
        &[CAP_PLAY, CAP_STATES, "Compare levels", READY_HEAD],
    );
    shot(&app, "sc.launchcockpit.options.120x40");
    shot(&app, "sc.launchcockpit.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(READY_HEAD);
    assert_size(&app, 120, 40);
    assert_idle(&wide_play, true);
    shot(&app, "sc.launchcockpit.playground.120x40");
    shot(&app, "sc.launchcockpit.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = page("s10-compare", &[], 120, 40, Some("paused"));
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
    shot(&app, "sc.launchcockpit.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    let settled = app.settle();
    if !text(&settled).contains("no color") {
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
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.launchcockpit.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    let top = app.settle();
    see(
        &top,
        &[TITLE, "● Compare levels"],
        &["no color", "Page options", "80×24"],
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
            "80×24",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_launch_options(&options, "success", "off", "off", "on");
    see(
        &options,
        &["success", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "build-failure"],
    );
    app.finish();
}

#[test]
fn launch_paused_cancel() {
    let app = page("s10-launch-cancel", &[], 120, 40, Some("paused"));
    let idle = app.wait_text(READY_HEAD);
    assert_size(&app, 120, 40);
    assert_idle(&idle, true);
    click_launch(&app);
    flash_settle(&app);
    let running = app.wait_text(LAUNCHING);
    assert_running_paused(&running);
    shot(&app, "sc.launchcockpit.launch.before");

    click_where(&app, CANCEL_BTN, |hit| hit.x > 24 && hit.y > 6);
    flash_settle(&app);
    let dialog = app.wait_text(CANCEL_TITLE);
    see(
        &dialog,
        &[
            CANCEL_TITLE,
            "The capsule stops before it starts",
            KEEP,
            "Cancel launch",
            LAUNCHING,
            "launch backend",
        ],
        &[
            READY_HEAD,
            SUCCESS_LOG,
            SUCCESS_STATUS,
            CRED_TITLE,
            BUILD_TITLE,
            CANCELLED,
            "alexey@build-07",
        ],
    );
    shot(&app, "sc.launchcockpit.launch.cancel");

    click_where(&app, KEEP, |hit| (10..36).contains(&hit.y) && hit.x > 24);
    flash_settle(&app);
    let kept = app.wait_absent(CANCEL_TITLE);
    see(
        &kept,
        &[LAUNCHING, LAUNCH_LINE, "Resolving identity", CANCEL_BTN],
        &[
            CANCEL_TITLE,
            KEEP,
            CANCELLED,
            CANCELLED_AT,
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "alexey@build-07",
        ],
    );

    click_where(&app, CANCEL_BTN, |hit| hit.x > 24 && hit.y > 6);
    flash_settle(&app);
    app.wait_text(KEEP);
    click_where(&app, "Cancel launch", |hit| {
        let row = row_text(&app.screen(), u32::from(hit.y));
        row.contains(KEEP) && hit.y > 6
    });
    flash_settle(&app);
    let cancelled = app.wait_text(CANCELLED);
    see(
        &cancelled,
        &[CANCELLED, CANCELLED_AT, "Launch again", CRUMB, TITLE],
        &[
            CANCEL_TITLE,
            KEEP,
            LAUNCHING,
            SUCCESS_LOG,
            SUCCESS_STATUS,
            CRED_TITLE,
            BUILD_TITLE,
            "Hardline open",
            "Launched ✓",
        ],
    );
    shot(&app, "sc.launchcockpit.launch.cancelled");
    app.finish();
}

#[test]
fn launch_failure_dialogs() {
    let app = page(
        "s10-launch-cred",
        &["launchcockpit.outcome=credentials"],
        120,
        40,
        Some("full"),
    );
    app.wait_text(READY_HEAD);
    click_launch(&app);
    let dialog = app.wait_screen("credentials dialog", credentials_dialog);
    see(
        &dialog,
        &[
            CRED_TITLE,
            CRED_LOG,
            "Retry",
            "Cancel launch",
            "Credentials",
        ],
        &[
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "Hardline open",
            "Launched ✓",
            BUILD_TITLE,
        ],
    );
    shot(&app, "sc.launchcockpit.launch.credentials");
    click_where(&app, "Retry", |hit| hit.y > 6 && hit.x > 20);
    let retried = app.wait_text(CRED_RETRY);
    see(
        &retried,
        &[CRED_RETRY, CRED_LOG],
        &[SUCCESS_LOG, SUCCESS_STATUS, "Hardline open", "Launched ✓"],
    );
    app.finish();

    let app = page(
        "s10-launch-build",
        &["launchcockpit.outcome=build-failure"],
        120,
        40,
        Some("full"),
    );
    app.wait_text(READY_HEAD);
    click_launch(&app);
    let dialog = app.wait_text(BUILD_TITLE);
    see(
        &dialog,
        &[
            BUILD_TITLE,
            BUILD_LOG,
            "unresolved impo",
            "Launch again",
            "Derived Image",
            RUN_ID,
        ],
        &[
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "Hardline open",
            "Launched ✓",
            CRED_TITLE,
        ],
    );
    shot(&app, "sc.launchcockpit.launch.build");
    click_build_cancel(&app);
    flash_settle(&app);
    let failed = app.wait_absent(BUILD_TITLE);
    see(
        &failed,
        &[BUILD_LOG, "Launch again", "Derived Image"],
        &[
            BUILD_TITLE,
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "Hardline open",
            "Launched ✓",
            CRED_TITLE,
        ],
    );
    assert_not_success(&failed);
    app.finish();
}

fn click_build_cancel(app: &App) {
    let screen = app.wait_text("Launch again");
    let hits: Vec<_> = find_all(&screen, "Cancel")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            if !row.contains("Launch again") {
                return false;
            }
            let rest: String = row.chars().skip(usize::from(hit.x)).collect();
            rest.starts_with("Cancel") && !rest[6..].trim_start().starts_with("launch")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected the build dialog Cancel, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

#[test]
fn launch_success() {
    let app = page("s10-launch-success", &[], 120, 40, Some("full"));
    let idle = app.wait_text(READY_HEAD);
    assert_idle(&idle, true);
    click_launch(&app);
    let done = app.wait_text(SUCCESS_LOG);
    see(
        &done,
        &[
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "capsule ready",
            "cap-payments-01",
            CRUMB,
            "Launch again",
        ],
        &[
            READY_HEAD,
            LAUNCHING,
            CRED_TITLE,
            BUILD_TITLE,
            CANCELLED,
            CANCEL_TITLE,
            "Credential source unavailable",
            "Docker build failed",
        ],
    );
    shot(&app, "sc.launchcockpit.launch.after");
    app.finish();
}

#[test]
fn option_outcome_credentials() {
    let app = page(
        "s10-outcome",
        &["launchcockpit.outcome=credentials"],
        120,
        40,
        Some("full"),
    );
    let idle = app.wait_text(READY_HEAD);
    see(
        &idle,
        &[READY_HEAD, TARGET, "not started"],
        &[CRED_TITLE, BUILD_TITLE, LAUNCHING, SUCCESS_LOG],
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.settle();
    assert_launch_options(&options, "credentials", "off", "off", "on");
    see(
        &options,
        &["credentials", "Launch outcome", "Page options"],
        &[CAP_PLAY, READY_HEAD, CRED_TITLE, "success"],
    );
    shot(&app, "opt.launchcockpit.outcome");
    click_tab(&app, "Playground");
    app.wait_text(READY_HEAD);
    flash_settle(&app);
    click_launch(&app);
    let dialog = app.wait_screen("credentials outcome", credentials_dialog);
    see(
        &dialog,
        &[CRED_TITLE, CRED_LOG, "Retry"],
        &[
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "Hardline open",
            "Launched ✓",
            BUILD_TITLE,
        ],
    );
    click_where(&app, "Cancel launch", |hit| {
        let row = row_text(&app.screen(), u32::from(hit.y));
        row.contains("Retry") && hit.y > 6
    });
    let cancelled = app.wait_text(CANCELLED);
    see(
        &cancelled,
        &[CANCELLED, CRED_LOG],
        &[
            SUCCESS_LOG,
            SUCCESS_STATUS,
            "Hardline open",
            "Launched ✓",
            CRED_TITLE,
        ],
    );
    app.finish();
}

#[test]
fn option_numbered_on() {
    let app = page(
        "s10-numbered",
        &["launchcockpit.numbered=on"],
        120,
        40,
        Some("paused"),
    );
    app.wait_text(READY_HEAD);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    assert_launch_options(&app.settle(), "success", "on", "off", "on");
    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let screen = app.wait_text("Identity");
    let identity = row_with(&screen, "Identity");
    assert!(
        identity.contains("01"),
        "numbered stages did not draw 01 on Identity: {identity:?}"
    );
    see(
        &screen,
        &[READY_HEAD, "01", "Identity", TARGET, CAP_PLAY],
        &["Page options", LAUNCHING, SUCCESS_LOG, CRED_TITLE],
    );
    shot(&app, "opt.launchcockpit.numbered");
    app.finish();
}

#[test]
fn option_selectable_on() {
    let off = page("s10-select-off", &[], 120, 40, Some("paused"));
    off.wait_text("Role");
    click_where(&off, "Role", |hit| hit.x > 26 && hit.y > 6);
    flash_settle(&off);
    let plain = off.settle();
    let prefix = before_label(&plain, "Role");
    assert!(
        !prefix.contains(FOCUS),
        "stages took focus while selectable is off: {prefix:?}\n{}",
        row_with(&plain, "Role")
    );
    off.finish();

    let app = page(
        "s10-selectable",
        &["launchcockpit.selectable=on"],
        120,
        40,
        Some("paused"),
    );
    app.wait_text(READY_HEAD);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    assert_launch_options(&app.settle(), "success", "off", "on", "on");
    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    click_where(&app, "Role", |hit| hit.x > 26 && hit.y > 6);
    flash_settle(&app);
    let screen = app.settle();
    let prefix = before_label(&screen, "Role");
    assert!(
        prefix.contains(FOCUS),
        "selectable Role has no focus bar: {prefix:?}\n{}",
        row_with(&screen, "Role")
    );
    see(
        &screen,
        &[READY_HEAD, "Role", "Move", TARGET],
        &["Page options", LAUNCHING, SUCCESS_LOG, CRED_TITLE],
    );
    shot(&app, "opt.launchcockpit.selectable");
    app.finish();
}

#[test]
fn option_wrap_off() {
    let on = page("s10-wrap-on", &[], 120, 40, Some("paused"));
    let on_screen = on.wait_text(READY_HEAD);
    let on_row = row_with(&on_screen, "Ready.");
    assert!(
        text(&on_screen).contains(READY_REST),
        "wrap on hid the log remainder\n{}",
        text(&on_screen)
    );
    on.finish();

    let app = page(
        "s10-wrap-off",
        &["launchcockpit.wrap=off"],
        120,
        40,
        Some("paused"),
    );
    app.wait_text("Ready.");
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    assert_launch_options(&app.settle(), "success", "off", "off", "off");
    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let screen = app.settle();
    let now = row_with(&screen, "Ready.");
    assert_eq!(
        now.trim(),
        on_row.trim(),
        "wrap off changed the first Ready row\nON {on_row:?}\nOFF {now:?}"
    );
    assert!(
        !text(&screen).contains(READY_REST),
        "wrap off still wrapped the remainder onto the next row\n{}",
        text(&screen)
    );
    see(
        &screen,
        &["Ready.", TARGET, CAP_PLAY],
        &["Page options", "Compare levels", LAUNCHING, SUCCESS_LOG],
    );
    shot(&app, "opt.launchcockpit.wrap");
    app.finish();
}
