//! Workspaces route, sections, specimens, compare levels, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s10_workspaces/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Screens › Workspaces";
const PREV: &str = "Screens › Activity";
const NEXT: &str = "Screens › Accounts & usage";
const TITLE: &str = "Workspaces";
const BLURB: &str = "A workspace tree beside its detail card and instance roster; narrow, a drawer";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "workspaces"];
const PAY: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const RELEASE: &str = "release-automation";
const PORTAL: &str = "customer-portal";
const LAUNCHING: &str = "Launching payments-platform…";
const ALREADY: &str = "A launch is already starting";
const PREWARMING: &str = "Prewarming the image…";
const PREWARMED: &str = "Image prewarmed · next launch starts warm";
const DELETE_TITLE: &str = "Delete payments-platform?";
const TYPE_PAY: &str = "Type payments-platform to confirm";
const DELETED: &str = "Deleted payments-platform";
const NOTHING: &str = "Nothing deleted";
const SUMMARY: &str = "Tab shows details";
/// The detail meta is clipped to “saved workspace · 1 runni” at 120 columns.
const SAVED: &str = "saved workspace";
/// The roster clock is clipped to “daemon · 3 s ag” at 120 columns.
const DAEMON: &str = "daemon · 3 s";
const WHEEL: Point = Point { x: 70, y: 18 };

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s10_workspaces/{}", case_id.replace('.', "_"))
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

fn page_opts(name: &str, opts: &[&str], cols: u16, rows: u16) -> App {
    let mut args = PAGE_ARGS.to_vec();
    for value in opts {
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

fn assert_workspace_options(
    screen: &TerminalSnapshot,
    collapse: &str,
    size: &str,
    roster: &str,
    esc: &str,
    typed: &str,
) {
    assert_one(screen, "Collapse below", collapse, &["80", "108", "150"]);
    assert_one(screen, "Detail size", size, &["resizable", "share"]);
    assert_one(screen, "Instance roster", roster, &["auto", "off"]);
    assert_toggle(screen, "Esc returns to the tree", esc);
    assert_toggle(screen, "Typed purge confirm", typed);
}

fn grid_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    find_all(screen, label).into_iter().find(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
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

fn click_button(app: &App, label: &str) -> Point {
    click_where(app, label, |hit| hit.x > 60 && hit.y > 6 && hit.y < 38)
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

fn reveal_grid(app: &App, label: &str) {
    let point = Point { x: 70, y: 18 };
    for _ in 0..40 {
        if grid_label(&app.settle(), label) {
            return;
        }
        app.wheel(point, true, 2);
    }
    assert!(
        grid_label(&app.screen(), label),
        "states grid never showed {label}\n{}",
        text(&app.screen())
    );
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

fn confirm_options(app: &App, back: &str, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    flash_settle(app);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text(back);
    flash_settle(app);
}

fn omitted_specimens(screen: &TerminalSnapshot) {
    for label in [
        "pressed",
        "focus + hover",
        "disabled",
        "error",
        "editing",
        "empty",
        "loading",
        "overflow",
    ] {
        assert!(
            !grid_label(screen, label),
            "{label} is drawn, but workspaces.rs rows are Rest Hover Focused Selected Busy Narrow\n{}",
            text(screen)
        );
    }
}

fn assert_idle_wide(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "4 saved",
            "2 running",
            PAY,
            "7f3a",
            "the-architect",
            "c41e",
            "reviewer",
            "2 h 14 m",
            "dirty",
            INFRA,
            RELEASE,
            "! failed",
            PORTAL,
            "idle",
            "◉",
            "◌",
            "a1c0",
            DAEMON,
            SAVED,
            "/workspace/payments-platform",
            "2 instances · 1 running",
            "Launch",
            "Prewarm",
            "Delete…",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            LAUNCHING,
            ALREADY,
            PREWARMING,
            PREWARMED,
            "just now",
            "starting",
            DELETE_TITLE,
            DELETED,
            NOTHING,
            SUMMARY,
            "80×24",
            OVERVIEW,
            "No instances",
        ],
    );
    let release = row_with(screen, RELEASE);
    assert!(
        release.contains("! failed"),
        "release row lost the failed mark\n{release}"
    );
    let portal = row_with(screen, PORTAL);
    assert!(
        portal.contains("idle"),
        "customer-portal row is not idle\n{portal}"
    );
    let running = row_with(screen, "7f3a");
    assert!(running.contains('◉'), "running instance lost ◉\n{running}");
    let preserved = row_with(screen, "c41e");
    assert!(
        preserved.contains('◌'),
        "preserved instance lost ◌\n{preserved}"
    );
}

fn assert_idle_narrow(screen: &TerminalSnapshot) {
    // 80 columns keeps the split and clips the detail. collapse_below 80 does
    // not draw the drawer summary, and the header has no size chip.
    see(
        screen,
        &[
            TITLE,
            "Playground",
            PAY,
            "7f3a",
            "◉",
            "◌",
            INFRA,
            RELEASE,
            "! failed",
            PORTAL,
            "idle",
            "4 saved",
            "2 running",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            SAVED,
            SUMMARY,
            "Delete…",
            DAEMON,
            "a1c0",
            LAUNCHING,
            PREWARMING,
            DELETE_TITLE,
            OVERVIEW,
        ],
    );
}

fn open_leaf_drawer(app: &App) {
    // A click on the collapsed tree does not take focus, so Enter never reaches
    // the leaf. Tab lands on the tree, Down selects 7f3a, and Enter opens it.
    app.key("tab");
    app.wait_text("Fold / open");
    app.key("down");
    app.key("enter");
    app.wait_text("Esc back");
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s10-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_idle_wide(&screen);
    shot(&app, "sc.workspaces.route");
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
        "Layout › Pane tree",
        "Text & code › Code editor",
        "Text & code › Diff",
        "Text & code › Terminal",
        "Text & code › Text utilities",
        "Screens › Settings",
        "Screens › Task runner",
        "Screens › Query workbench",
        "Screens › Connections",
        "Screens › Finder",
        "Screens › Disk cleanup",
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(PAY);
    see(
        &entered,
        &[CRUMB, BLURB, PAY, "7f3a", "Delete…"],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, PAY, BLURB]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, PAY, "Prewarm"], &[PREV, OVERVIEW]);
    app.key("]");
    let next = app.wait_text(NEXT);
    see(&next, &[NEXT], &[CRUMB, PAY, "Delete…"]);
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, PAY, BLURB]);
    click_side(&app, "Screens");
    click_side(&app, "Workspaces");
    let pointed = app.wait_text(PAY);
    see(
        &pointed,
        &[CRUMB, BLURB, PAY, "7f3a", SAVED, "idle"],
        &[OVERVIEW, "Page options", "Compare levels", PREV],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s10-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(PAY);
    assert_size(&app, 80, 24);
    assert_idle_narrow(&narrow_play);
    shot(&app, "sc.workspaces.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_workspace_options(&narrow_opt, "80", "resizable", "auto", "on", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Collapse below", "Detail size"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            SAVED,
            "focus + hover",
        ],
    );
    shot(&app, "sc.workspaces.options.80x24");

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
    omitted_specimens(&narrow_states);
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "Workspace tree"],
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
            SAVED,
        ],
    );
    shot(&app, "sc.workspaces.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
    });
    assert_size(&app, 120, 40);
    omitted_specimens(&wide_states);
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
            "Workspace tree",
            "rest",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            SAVED,
            "focus + hover",
            "256 colors",
        ],
    );
    let specimens = [
        ("sc.workspaces.specimen.rest", "rest"),
        ("sc.workspaces.specimen.hover", "hover"),
        ("sc.workspaces.specimen.focused", "focused"),
        ("sc.workspaces.specimen.selected", "selected"),
        ("sc.workspaces.specimen.busy", "busy"),
        ("sc.workspaces.specimen.narrow", "narrow"),
    ];
    let mut later = Vec::new();
    for (case_id, label) in specimens {
        if grid_label(&app.screen(), label) {
            shot(&app, case_id);
        } else {
            later.push((case_id, label));
        }
    }
    shot(&app, "sc.workspaces.states.120x40");
    shot(&app, "sc.workspaces.states");
    for (case_id, label) in later {
        reveal_grid(&app, label);
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid"
        );
        shot(&app, case_id);
    }

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_workspace_options(&wide_opt, "80", "resizable", "auto", "on", "on");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Collapse below"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", SAVED],
    );
    shot(&app, "sc.workspaces.options.120x40");
    shot(&app, "sc.workspaces.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(PAY);
    assert_size(&app, 120, 40);
    assert_idle_wide(&wide_play);
    shot(&app, "sc.workspaces.playground.120x40");
    shot(&app, "sc.workspaces.playground");
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
        &[CRUMB, "● Compare levels", CAP_STATES],
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
    shot(&app, "sc.workspaces.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24
    });
    let mut narrow = app.settle();
    if !text(&narrow).contains("no color") {
        for _ in 0..80 {
            narrow = app.settle();
            if text(&narrow).contains("no color") {
                break;
            }
            app.wheel(WHEEL, true, 3);
        }
        narrow = app.settle();
    }
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.workspaces.compare-levels.80x24");

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
    assert_workspace_options(&options, "80", "resizable", "auto", "on", "on");
    see(
        &options,
        &["80", "resizable", "auto", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "share", "150"],
    );
    app.finish();
}

#[test]
fn launch_prewarm_delete_and_narrow() {
    let app = open("s10-workspaces", 120, 40, PAGE_ARGS);
    let before = app.wait_text(PAY);
    assert_size(&app, 120, 40);
    assert_idle_wide(&before);
    shot(&app, "sc.workspaces.workspaces.before");

    app.resize(80, 24);
    app.wait_screen("workspaces narrow", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains("! failed")
    });
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert_idle_narrow(&narrow);
    shot(&app, "sc.workspaces.workspaces.80x24");
    app.resize(120, 40);
    app.wait_text(WIDE);
    let restored = app.wait_text(SAVED);
    assert_idle_wide(&restored);

    let launch_at = click_button(&app, "Launch");
    flash_settle(&app);
    let launched = app.wait_text(LAUNCHING);
    let copies = find_all(&launched, "e5d9").len();
    assert!(copies >= 1, "launch did not add e5d9\n{}", text(&launched));
    let instance = row_with(&launched, "e5d9");
    assert!(
        instance.contains("the-architect"),
        "new instance role\n{instance}"
    );
    see(
        &launched,
        &[LAUNCHING, "e5d9", "starting", "3 running", PAY, "◉"],
        &[
            ALREADY,
            "just now",
            "Instance e5d9 is running",
            PREWARMING,
            PREWARMED,
            DELETED,
        ],
    );
    app.click(launch_at);
    flash_settle(&app);
    let second = app.settle();
    assert_eq!(
        find_all(&second, "e5d9").len(),
        copies,
        "a second launch while the button is busy added an instance\n{}",
        text(&second)
    );
    see(
        &second,
        &[LAUNCHING, "e5d9", "starting"],
        &[ALREADY, "just now", "Instance e5d9 is running", PREWARMED],
    );

    let prewarm_at = click_button(&app, "Prewarm");
    flash_settle(&app);
    let warming = app.wait_text(PREWARMING);
    see(
        &warming,
        &[PREWARMING, "e5d9", "starting", PAY],
        &[PREWARMED, "just now", "Instance e5d9 is running", DELETED],
    );
    app.click(prewarm_at);
    flash_settle(&app);
    let warmed_again = app.settle();
    see(
        &warmed_again,
        &[PREWARMING, "e5d9"],
        &[PREWARMED, "just now", ALREADY],
    );

    click_button(&app, "Edit");
    flash_settle(&app);
    let editing = app.wait_text("Editing payments-platform");
    see(
        &editing,
        &["Editing payments-platform", PAY, "e5d9"],
        &[DELETE_TITLE, DELETED, PREWARMED, "just now"],
    );

    click_button(&app, "Delete…");
    app.wait_text(DELETE_TITLE);
    flash_settle(&app);
    let dialog = app.wait_text(TYPE_PAY);
    see(
        &dialog,
        &[
            DELETE_TITLE,
            TYPE_PAY,
            "3 instances stopped and purged",
            "kept on disk",
            "Recoverable",
            "Delete workspace",
            "Cancel",
            PAY,
        ],
        &[DELETED, NOTHING, PREWARMED, "just now"],
    );
    shot(&app, "sc.workspaces.workspaces.dialog");
    click_where(&app, "Cancel", |hit| hit.y > 8 && hit.y < 36);
    flash_settle(&app);
    let cancelled = app.wait_text(NOTHING);
    see(
        &cancelled,
        &[NOTHING, PAY, "7f3a", "e5d9", CRUMB],
        &[DELETE_TITLE, TYPE_PAY, DELETED, "Delete workspace"],
    );

    click_button(&app, "Delete…");
    app.wait_text(TYPE_PAY);
    flash_settle(&app);
    app.type_text(PAY);
    app.key("enter");
    let deleted = app.wait_text(DELETED);
    see(
        &deleted,
        &[DELETED, INFRA, RELEASE, PORTAL, "9b02", "3 saved"],
        &[DELETE_TITLE, TYPE_PAY, "7f3a", "e5d9", NOTHING, "4 saved"],
    );
    assert!(
        !(0..deleted.rows()).any(|row| {
            let line = row_text(&deleted, row);
            (line.contains('▸') || line.contains('▾')) && line.contains(PAY)
        }),
        "deleted workspace stayed in the tree\n{}",
        text(&deleted)
    );
    shot(&app, "sc.workspaces.workspaces.after");
    app.finish();
}

#[test]
fn option_collapse_below_draws_the_drawer() {
    let app = page(
        "s10-collapse",
        Some("workspaces.collapse_below=150"),
        120,
        40,
    );
    app.wait_text(SUMMARY);
    confirm_options(&app, SUMMARY, |screen| {
        assert_workspace_options(screen, "150", "resizable", "auto", "on", "on");
    });
    let collapsed = app.wait_text(SUMMARY);
    see(
        &collapsed,
        &[
            CAP_PLAY, PAY, "7f3a", "◉", "◌", RELEASE, "! failed", PORTAL, "idle", SUMMARY, WIDE,
        ],
        &[
            SAVED,
            "Delete…",
            DAEMON,
            "a1c0",
            "Page options",
            "108",
            LAUNCHING,
        ],
    );
    shot(&app, "opt.workspaces.collapse_below");
    open_leaf_drawer(&app);
    let drawer = app.wait_text("Esc back");
    see(
        &drawer,
        &[
            "Esc back",
            "Working dir",
            "/workspace/payments-platform",
            "Delete…",
        ],
        &[SUMMARY, DELETE_TITLE],
    );
    app.key("esc");
    let back = app.wait_text(SUMMARY);
    see(
        &back,
        &[SUMMARY, PAY, "7f3a", "! failed"],
        &["Esc back", "Working dir", SAVED, DELETE_TITLE],
    );
    app.finish();
}

#[test]
fn option_size_share_keeps_the_glyphs() {
    let app = page("s10-size", Some("workspaces.size=share"), 120, 40);
    app.wait_text(PAY);
    confirm_options(&app, "Working dir", |screen| {
        assert_workspace_options(screen, "80", "share", "auto", "on", "on");
    });
    let play = app.wait_text("Working dir");
    let dir = find_all(&play, "Working dir");
    assert_eq!(dir.len(), 1, "Working dir\n{}", text(&play));
    assert!(
        dir[0].x < 76,
        "share left Working dir at x {}; resizable draws it at 77",
        dir[0].x
    );
    let visible = text(&play);
    assert!(
        visible.contains("╮  ╭"),
        "share still draws the resizable seam\n{visible}"
    );
    assert!(
        !visible.contains("╮ │╭"),
        "share drew the resizable seam\n{visible}"
    );
    see(
        &play,
        &[
            CAP_PLAY,
            "◉",
            "◌",
            "! failed",
            "7f3a",
            "c41e",
            "a1c0",
            "idle",
            "Working dir",
            "Delete…",
        ],
        &[
            "Page options",
            "resizable",
            SUMMARY,
            LAUNCHING,
            SAVED,
            DAEMON,
        ],
    );
    shot(&app, "opt.workspaces.size");
    app.finish();
}

#[test]
fn option_roster_off_hides_the_list() {
    let app = page("s10-roster", Some("workspaces.roster=off"), 120, 40);
    app.wait_text(PAY);
    confirm_options(&app, SAVED, |screen| {
        assert_workspace_options(screen, "80", "resizable", "off", "on", "on");
    });
    let play = app.wait_text(SAVED);
    see(
        &play,
        &[
            CAP_PLAY,
            PAY,
            "7f3a",
            "◉",
            "◌",
            "c41e",
            RELEASE,
            "! failed",
            PORTAL,
            "idle",
            SAVED,
            "Delete…",
        ],
        &["Page options", DAEMON, "a1c0", LAUNCHING],
    );
    shot(&app, "opt.workspaces.roster");
    app.finish();
}

#[test]
fn option_esc_returns_off_keeps_the_drawer() {
    let app = page_opts(
        "s10-esc",
        &[
            "workspaces.collapse_below=150",
            "workspaces.esc_returns=off",
        ],
        120,
        40,
    );
    app.wait_text(SUMMARY);
    confirm_options(&app, SUMMARY, |screen| {
        assert_workspace_options(screen, "150", "resizable", "auto", "off", "on");
    });
    open_leaf_drawer(&app);
    let open_drawer = app.wait_text("Esc back");
    see(
        &open_drawer,
        &["Esc back", "Working dir", PAY],
        &[SUMMARY, DELETE_TITLE],
    );
    app.key("esc");
    flash_settle(&app);
    let stayed = app.settle();
    see(
        &stayed,
        &[
            "Esc back",
            "Working dir",
            "/workspace/payments-platform",
            "Delete…",
        ],
        &[SUMMARY, DELETE_TITLE, DELETED],
    );
    shot(&app, "opt.workspaces.esc_returns");
    app.finish();
}

#[test]
fn option_typed_off_skips_the_phrase() {
    let app = page("s10-typed", Some("workspaces.typed=off"), 120, 40);
    app.wait_text(PAY);
    confirm_options(&app, SAVED, |screen| {
        assert_workspace_options(screen, "80", "resizable", "auto", "on", "off");
    });
    click_button(&app, "Delete…");
    app.wait_text(DELETE_TITLE);
    flash_settle(&app);
    let dialog = app.wait_text("Delete workspace");
    see(
        &dialog,
        &[
            DELETE_TITLE,
            "2 instances stopped and purged",
            "kept on disk",
            "Delete workspace",
            "Cancel",
            PAY,
        ],
        &[TYPE_PAY, "to confirm", DELETED, NOTHING],
    );
    shot(&app, "opt.workspaces.typed");
    click_where(&app, "Cancel", |hit| hit.y > 8 && hit.y < 36);
    flash_settle(&app);
    let cancelled = app.wait_text(NOTHING);
    see(
        &cancelled,
        &[NOTHING, PAY, "7f3a"],
        &[DELETE_TITLE, DELETED, TYPE_PAY],
    );
    app.finish();
}
