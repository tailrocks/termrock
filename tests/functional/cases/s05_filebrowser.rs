//! File browser route, sections, specimens, compare levels, browse journey, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s05_filebrowser/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const CRUMB: &str = "Overlays › File browser";
const TITLE: &str = "File browser";
/// The header truncates the page blurb, so the visible prefix is the check.
const BLURB_VISIBLE: &str =
    "FileBrowser: choose a file or folder from a FileSource, with a path bar";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const PAGE_ARGS: &[&str] = &["--page", "filebrowser"];
const CHOOSE_DIR: &str = "Choose a folder";
const OPEN_FILE: &str = "Open a file";
const PAY: &str = "payments-platform/";
const PORTAL: &str = "customer-portal/";
const README: &str = "README.md";
const HIDDEN_BOX: &str = "Show hidden files";
const READ_ONLY: &str = "Mount read-only";
const FILTER_HINT: &str = "Name contains";
const GIT: &str = "Git URL…";
const KNOWN_PAY: &str = "acme/payments-platform";
const KNOWN_PORTAL: &str = "acme/customer-portal";
const VALID_PAY: &str = "github.com/acme/payments-platform";
const INVALID_URL: &str = "github.com/acme/missing-repo";
const STEPPER: &str = "✓ Source · ● Folder · ○ Agent · ○ Review";
const CHOSEN_DIR: &str = "Chosen { path: ~/src/payments-platform, checked: true }";
const CHOSEN_FILE: &str = "Chosen { path: ~/src/payments-platform/README.md, checked: false }";
const CHOSEN_CWD: &str = "Chosen { path: ~/src, checked: false }";
const URL_PAY: &str = "Url(github.com/acme/payments-platform)";
const URL_PORTAL: &str = "Url(acme/customer-portal)";

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s05_filebrowser/{}", case_id.replace('.', "_"))
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

/// Specimen labels sit in the grid, not in the focus legend on the compare row.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label)
        .into_iter()
        .any(|hit| hit.y >= 9 && (18..70).contains(&hit.x))
}

fn in_dialog(screen: &TerminalSnapshot, needle: &str) -> bool {
    let Some(top) = find_all(screen, "╭").into_iter().find(|hit| hit.x >= 10) else {
        return false;
    };
    let Some(right) = find_all(screen, "╮").into_iter().find(|hit| hit.y == top.y) else {
        return false;
    };
    let bottom = find_all(screen, "╰")
        .into_iter()
        .find(|hit| hit.x == top.x)
        .map(|hit| hit.y)
        .unwrap_or(top.y.saturating_add(20));
    find_all(screen, needle)
        .into_iter()
        .any(|hit| hit.x > top.x && hit.x < right.x && hit.y >= top.y && hit.y <= bottom)
}

fn assert_dialog(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    for item in present {
        assert!(
            in_dialog(screen, item),
            "dialog missing {item:?}\n{}",
            text(screen)
        );
    }
    for item in absent {
        assert!(
            !in_dialog(screen, item),
            "dialog still shows {item:?}\n{}",
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

fn click_side(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x < 24);
}

fn click_browse(app: &App) {
    // The intro sentence also contains "Browse…". The button row does not.
    let screen = app.wait_text("Browse…");
    let hits: Vec<_> = find_all(&screen, "Browse…")
        .into_iter()
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            hit.y < 36 && !line.contains("opens the modal")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one Browse… button, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_choose(app: &App) {
    // The footer hint also says Choose. The button sits on the dialog's last row.
    click_where(app, "Choose", |hit| (25..36).contains(&hit.y));
}

fn reveal_grid(app: &App, label: &str) {
    let point = Point { x: 70, y: 20 };
    for _ in 0..40 {
        if grid_label(&app.settle(), label) {
            return;
        }
        app.wheel(point, true, 1);
    }
    panic!("grid row {label} did not appear\n{}", text(&app.screen()));
}

fn wheel_until(app: &App, down: bool, pred: impl Fn(&str) -> bool) {
    let point = Point { x: 70, y: 20 };
    for _ in 0..80 {
        if pred(&text(&app.settle())) {
            return;
        }
        app.wheel(point, down, 3);
    }
    panic!("scroll missed its row\n{}", text(&app.screen()));
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

fn shot_visible(app: &App, case_id: &str, label: &str) {
    assert!(
        grid_label(&app.screen(), label),
        "{label} is not on the states grid\n{}",
        text(&app.screen())
    );
    shot(app, case_id);
}

fn open_browser(app: &App, title: &str) {
    click_browse(app);
    app.wait_text(title);
    flash_settle(app);
    app.wait_text(title);
}

fn confirm_option(app: &App, check: impl Fn(&TerminalSnapshot)) {
    if text(&app.screen()).contains(CHOOSE_DIR)
        || text(&app.screen()).contains(OPEN_FILE)
        || text(&app.screen()).contains("Choose a file or folder")
        || text(&app.screen()).contains(GIT)
    {
        app.key("escape");
        app.wait_screen("browser closed before options", |screen| {
            !text(screen).contains(CHOOSE_DIR)
                && !text(screen).contains(OPEN_FILE)
                && !text(screen).contains(GIT)
        });
    }
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    flash_settle(app);
    check(&app.wait_text("Page options"));
    let _ = screen;
}

fn host_line(screen: &TerminalSnapshot) -> String {
    row_with(screen, "Host folder")
}

/// A result line in the log. The Results caption always names Cancelled.
fn clear_url(app: &App) {
    // The URL field uses emacs bindings: ctrl+a is line start, ctrl+k kills the rest.
    app.key("ctrl+a");
    app.key("ctrl+k");
}

fn logged(screen: &TerminalSnapshot, needle: &str) -> bool {
    (0..screen.rows()).any(|row| {
        let line = row_text(screen, row);
        line.contains(needle) && !line.contains("FileBrowserResult")
    })
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
            "FileBrowser:",
            "path bar",
            CAP_PLAY,
            WIDE,
            "Playground",
            "Browse…",
            "none yet",
            "New mount",
            "MemorySource",
            "/Users/ada",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            CHOOSE_DIR,
            OPEN_FILE,
        ],
    );
    assert!(
        host_line(&screen).contains("none yet"),
        "host folder\n{}",
        text(&screen)
    );
    shot(&app, "sc.filebrowser.route");
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
        "Overlays › Pickers",
        "Overlays › Dialog kinds",
        "Overlays › Popups & tooltips",
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Browse…");
    see(
        &entered,
        &[CRUMB, "FileBrowser:", "Browse…", "none yet"],
        &[OVERVIEW, "Page options", "Overlays › Popups & tooltips"],
    );
    app.key("[");
    let left = app.wait_text("Overlays › Popups & tooltips");
    see(
        &left,
        &["Overlays › Popups & tooltips"],
        &[CRUMB, "Browse…", "none yet"],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Browse…", "MemorySource"],
        &["Overlays › Popups & tooltips", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Browse…"]);
    click_side(&app, "Overlays");
    app.wait_text("Dialogs");
    click_side(&app, "File browser");
    let pointed = app.wait_text("Browse…");
    see(
        &pointed,
        &[CRUMB, "FileBrowser:", "Browse…", "none yet", "New mount"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s05-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Browse…");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[TITLE, "Playground", "Browse…", "none yet", "New mount"],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            CHOOSE_DIR,
            "truecolor",
        ],
    );
    shot(&app, "sc.filebrowser.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_select(&narrow_opt, "Choose", "folders", &["files", "any"]);
    assert_select(&narrow_opt, "Source", "fixture", &["real"]);
    assert_toggle(&narrow_opt, "Path bar", "on");
    assert_toggle(&narrow_opt, "Name filter", "on");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Choose", "Source"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Browse…",
            WIDE,
            CHOOSE_DIR,
        ],
    );
    shot(&app, "sc.filebrowser.options.80x24");

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
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "Listing"],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Browse…",
            WIDE,
            "pressed",
            "focus + hover",
        ],
    );
    shot(&app, "sc.filebrowser.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
    });
    assert_size(&app, 120, 40);
    see(
        &wide_states,
        &[
            CRUMB,
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "Listing",
            "Hidden toggle",
            "rest",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Browse…",
            "256 colors",
        ],
    );
    for label in [
        "pressed",
        "focus + hover",
        "busy",
        "error",
        "editing",
        "loading",
        "overflow",
    ] {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in [
        ("sc.filebrowser.specimen.rest", "rest"),
        ("sc.filebrowser.specimen.hover", "hover"),
        ("sc.filebrowser.specimen.focused", "focused"),
        ("sc.filebrowser.specimen.selected", "selected"),
        ("sc.filebrowser.specimen.disabled", "disabled"),
    ] {
        if grid_label(&app.screen(), label) {
            shot_visible(&app, case_id, label);
        } else {
            reveal_grid(&app, label);
            shot_visible(&app, case_id, label);
            wheel_until(&app, false, |visible| {
                visible.contains("○ Compare levels") && visible.contains("rest")
            });
        }
    }
    assert!(
        grid_label(&app.screen(), "rest"),
        "states capture left the top of the grid\n{}",
        text(&app.screen())
    );
    shot(&app, "sc.filebrowser.states.120x40");
    shot(&app, "sc.filebrowser.states");

    reveal_grid(&app, "empty");
    shot_visible(&app, "sc.filebrowser.specimen.empty", "empty");
    if !grid_label(&app.screen(), "narrow") {
        reveal_grid(&app, "narrow");
    }
    shot_visible(&app, "sc.filebrowser.specimen.narrow", "narrow");
    let low = app.settle();
    for label in [
        "pressed",
        "focus + hover",
        "busy",
        "error",
        "editing",
        "loading",
        "overflow",
    ] {
        assert!(
            !grid_label(&low, label),
            "{label} appeared after scrolling the states grid\n{}",
            text(&low)
        );
    }

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_select(&wide_opt, "Choose", "folders", &["files", "any"]);
    assert_select(&wide_opt, "Source", "fixture", &["real"]);
    assert_toggle(&wide_opt, "Path bar", "on");
    assert_toggle(&wide_opt, "Name filter", "on");
    assert_toggle(&wide_opt, "Hidden toggle", "on");
    assert_toggle(&wide_opt, "Show hidden", "off");
    assert_toggle(&wide_opt, "Type-ahead", "on");
    assert_select(&wide_opt, "Keys", "desktop", &["vim"]);
    assert_toggle(&wide_opt, "URL entry", "off");
    assert_toggle(&wide_opt, "Option check box", "on");
    assert_toggle(&wide_opt, "Step line", "off");
    assert_select(&wide_opt, "Dismiss", "escape", &["light", "never"]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Browse…"],
    );
    shot(&app, "sc.filebrowser.options.120x40");
    shot(&app, "sc.filebrowser.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Browse…");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB_VISIBLE,
            CAP_PLAY,
            WIDE,
            "Browse…",
            "none yet",
            "MemorySource",
            "/Users/ada",
            "desktop",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            CHOOSE_DIR,
            "vim key map",
        ],
    );
    assert!(host_line(&wide_play).contains("none yet"));
    shot(&app, "sc.filebrowser.playground.120x40");
    shot(&app, "sc.filebrowser.playground");
    app.finish();
}

#[test]
fn compare_levels_returns_to_truecolor() {
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
    let on = app.wait_text("truecolor");
    see(
        &on,
        &[CRUMB, "● Compare levels", "truecolor", "rest", CAP_STATES],
        &["○ Compare levels", "no color", "Page options"],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 20 };
    for _ in 0..80 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel(point, true, 3);
    }
    assert!(
        saw_256 && saw_16 && text(&app.screen()).contains("no color"),
        "a colour band was skipped before no color\n{}",
        text(&app.screen())
    );
    // The truecolor band is taller than 40 rows, so "256 colors" stays visible.
    wheel_until(&app, false, |visible| {
        visible.contains("● Compare levels")
            && visible.contains("truecolor")
            && visible.contains("rest")
            && !visible.contains("no color")
            && !visible.contains("16 colors")
    });
    let top = app.settle();
    assert_size(&app, 120, 40);
    see(
        &top,
        &[CRUMB, "● Compare levels", "truecolor", "rest", WIDE],
        &["no color", "16 colors", "○ Compare levels", "Page options"],
    );
    shot(&app, "sc.filebrowser.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    if text(&narrow).contains("no color")
        || text(&narrow).contains("16 colors")
        || !text(&narrow).contains("truecolor")
        || !text(&narrow).contains("● Compare levels")
    {
        wheel_until(&app, false, |visible| {
            visible.contains("● Compare levels")
                && visible.contains("truecolor")
                && visible.contains("rest")
                && !visible.contains("no color")
                && !visible.contains("16 colors")
        });
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor", "rest"],
        &["no color", "16 colors", "Page options", WIDE],
    );
    shot(&app, "sc.filebrowser.compare-levels.80x24");

    app.resize(120, 40);
    app.wait_text(WIDE);
    wheel_until(&app, false, |visible| visible.contains("● Compare levels"));
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
    assert_select(&options, "Choose", "folders", &["files", "any"]);
    assert_select(&options, "Source", "fixture", &["real"]);
    assert_select(&options, "Dismiss", "escape", &["light", "never"]);
    see(
        &options,
        &["folders", "fixture", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "real"],
    );
    app.finish();
}

#[test]
fn browse_directory_hidden_filter_cancel_and_url() {
    let app = page("s05-browse", Some("filebrowser.url_entry=on"), 120, 40);
    let screen = app.wait_text("Browse…");
    assert_size(&app, 120, 40);
    assert!(
        host_line(&screen).contains("none yet"),
        "host\n{}",
        text(&screen)
    );
    see(
        &screen,
        &["none yet", "Browse…", "desktop"],
        &[CHOOSE_DIR, CHOSEN_DIR, URL_PAY],
    );
    assert!(!logged(&screen, "Cancelled"));

    open_browser(&app, CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(
        &opened,
        &[
            CHOOSE_DIR,
            PAY,
            PORTAL,
            "~/src",
            "› src",
            HIDDEN_BOX,
            READ_ONLY,
            FILTER_HINT,
            GIT,
        ],
        &[".env", ".zshrc", "crates/", URL_PAY],
    );
    app.click(Point { x: 110, y: 20 });
    flash_settle(&app);
    let stayed = app.wait_text(CHOOSE_DIR);
    assert_dialog(&stayed, &[CHOOSE_DIR, PAY], &["Cancelled"]);

    app.key("p");
    app.key("enter");
    let inside = app.wait_text("crates/");
    assert_dialog(
        &inside,
        &[
            "crates/",
            "docs/",
            README,
            "Cargo.toml",
            "~/src/payments-platform",
        ],
        &[PORTAL, ".env", "todo.md"],
    );
    shot(&app, "sc.filebrowser.browse.before");

    app.key("backspace");
    let back = app.wait_text(PORTAL);
    assert_dialog(&back, &[PORTAL, PAY, "~/src"], &["crates/", ".env"]);
    app.key("backspace");
    let home = app.wait_screen("fixture home", |screen| {
        in_dialog(screen, "todo.md") && !in_dialog(screen, PAY)
    });
    assert_dialog(&home, &["design/", "todo.md"], &[PAY, ".zshrc", "~/src"]);
    app.key(".");
    let shown = app.wait_screen("hidden files on", |screen| in_dialog(screen, ".zshrc"));
    assert_dialog(&shown, &[".zshrc", ".config", ".ssh"], &[PAY]);
    app.key(".");
    let hidden = app.wait_screen("hidden files off", |screen| !in_dialog(screen, ".zshrc"));
    assert_dialog(
        &hidden,
        &["todo.md", "design/"],
        &[".zshrc", ".config", ".ssh"],
    );

    app.key("ctrl+f");
    app.type_text("todo");
    let filtered = app.wait_screen("name filter", |screen| {
        in_dialog(screen, "todo.md") && !in_dialog(screen, "design/")
    });
    assert_dialog(&filtered, &["todo.md"], &["design/", "src/", FILTER_HINT]);
    for _ in 0..4 {
        app.key("backspace");
    }
    app.wait_text("design/");
    app.key("enter");
    app.wait_text("design/");

    app.key("s");
    app.key("enter");
    app.wait_text(PAY);
    app.key("p");
    click_where(&app, READ_ONLY, |hit| hit.y > 16);
    flash_settle(&app);
    click_choose(&app);
    let chosen = app.wait_text(CHOSEN_DIR);
    assert!(
        host_line(&chosen).contains("~/src/payments-platform (read-only)"),
        "host after choose\n{}",
        text(&chosen)
    );
    see(
        &chosen,
        &[CHOSEN_DIR, "read-only"],
        &[CHOOSE_DIR, "none yet"],
    );

    open_browser(&app, CHOOSE_DIR);
    app.key("escape");
    let cancelled = app.wait_screen("escape cancels", |screen| {
        !text(screen).contains(CHOOSE_DIR) && text(screen).contains("Cancelled")
    });
    assert!(
        host_line(&cancelled).contains("~/src/payments-platform (read-only)"),
        "cancel replaced the chosen path\n{}",
        text(&cancelled)
    );
    assert!(logged(&cancelled, "Cancelled"));
    see(&cancelled, &[CHOSEN_DIR], &[CHOOSE_DIR, "none yet"]);

    open_browser(&app, CHOOSE_DIR);
    click_where(&app, GIT, |hit| hit.y > 16);
    flash_settle(&app);
    let url = app.wait_text("Known here:");
    assert_dialog(
        &url,
        &[KNOWN_PAY, KNOWN_PORTAL, "Git URL", "Browse folders"],
        &[PAY],
    );
    app.key("enter");
    let empty = app.wait_text("Enter a repository URL");
    assert_dialog(&empty, &["Enter a repository URL", KNOWN_PAY], &[URL_PAY]);
    app.type_text(INVALID_URL);
    app.key("enter");
    let invalid = app.wait_text("repository not found");
    assert_dialog(
        &invalid,
        &["repository not found", INVALID_URL, KNOWN_PORTAL],
        &[URL_PAY, URL_PORTAL],
    );
    clear_url(&app);
    app.type_text(VALID_PAY);
    app.key("enter");
    let paid = app.wait_text(URL_PAY);
    assert!(
        host_line(&paid).contains(VALID_PAY),
        "host after url\n{}",
        text(&paid)
    );
    assert!(logged(&paid, URL_PAY));
    see(&paid, &[URL_PAY, CHOSEN_DIR], &[CHOOSE_DIR, INVALID_URL]);

    open_browser(&app, CHOOSE_DIR);
    click_where(&app, GIT, |hit| hit.y > 16);
    flash_settle(&app);
    app.wait_text("Known here:");
    app.type_text(KNOWN_PORTAL);
    app.key("enter");
    let portal = app.wait_text(URL_PORTAL);
    assert_size(&app, 120, 40);
    assert!(host_line(&portal).contains(KNOWN_PORTAL));
    see(
        &portal,
        &[URL_PORTAL, URL_PAY, CHOSEN_DIR],
        &[CHOOSE_DIR, "repository not found", "none yet"],
    );
    assert!(logged(&portal, URL_PORTAL));
    shot(&app, "sc.filebrowser.browse.after");
    app.finish();
}

fn option_browser(name: &str, opt: &str, title: &str) -> App {
    let app = page(name, Some(opt), 120, 40);
    app.wait_text("Browse…");
    open_browser(&app, title);
    app
}

#[test]
fn option_choose_files() {
    let app = option_browser("s05-choose", "filebrowser.choose=files", OPEN_FILE);
    let opened = app.settle();
    assert_dialog(&opened, &[OPEN_FILE, PAY, "~/src"], &[CHOOSE_DIR, README]);
    app.key("p");
    app.key("enter");
    app.wait_text("crates/");
    app.key("r");
    app.key("space");
    let chosen = app.wait_text(CHOSEN_FILE);
    assert!(host_line(&chosen).contains("~/src/payments-platform/README.md"));
    see(
        &chosen,
        &[CHOSEN_FILE, "checked: false"],
        &[CHOOSE_DIR, "checked: true", OPEN_FILE],
    );
    shot(&app, "opt.filebrowser.choose");
    confirm_option(&app, |screen| {
        assert_select(screen, "Choose", "files", &["folders", "any"]);
        assert_select(screen, "Source", "fixture", &["real"]);
    });
    app.finish();
}

#[test]
fn option_source_real() {
    let app = page("s05-source", Some("filebrowser.source=real"), 120, 40);
    app.wait_text("Browse…");
    let home = std::env::var("HOME").unwrap_or_default();
    let screen = app.screen();
    if !home.is_empty() {
        assert!(
            !text(&screen).contains(&home),
            "source=real painted the host home without opening the browser\n{}",
            text(&screen)
        );
    }
    assert!(
        !text(&screen).contains("target/functional"),
        "source=real listed the isolated home\n{}",
        text(&screen)
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Source");
    assert_select(&options, "Source", "real", &["fixture"]);
    assert_select(&options, "Choose", "folders", &["files", "any"]);
    see(
        &options,
        &["real", "Page options", "Source", "folders"],
        &[CHOOSE_DIR, "Browse…", "MemorySource", "/Users/ada", &home],
    );
    assert!(
        !text(&options).contains("target/functional"),
        "options baseline includes an evidence path\n{}",
        text(&options)
    );
    shot(&app, "opt.filebrowser.source");
    app.finish();
}

#[test]
fn option_path_bar_off() {
    let app = option_browser("s05-path", "filebrowser.path_bar=off", CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(&opened, &[CHOOSE_DIR, "~/src", PAY], &["› src"]);
    shot(&app, "opt.filebrowser.path_bar");
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Path bar", "off");
        assert_toggle(screen, "Name filter", "on");
    });
    app.finish();
}

#[test]
fn option_filter_off() {
    let app = option_browser("s05-filter", "filebrowser.filter=off", CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(&opened, &[CHOOSE_DIR, PAY, "~/src"], &[FILTER_HINT]);
    app.key("ctrl+f");
    let stayed = app.settle();
    assert_dialog(&stayed, &[PAY, CHOOSE_DIR], &[FILTER_HINT, "No matches"]);
    shot(&app, "opt.filebrowser.filter");
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Name filter", "off");
        assert_toggle(screen, "Path bar", "on");
    });
    app.finish();
}

#[test]
fn option_hidden_toggle_off() {
    let app = option_browser(
        "s05-hidden-toggle",
        "filebrowser.hidden_toggle=off",
        CHOOSE_DIR,
    );
    let opened = app.settle();
    assert_dialog(&opened, &[CHOOSE_DIR, PAY], &[HIDDEN_BOX]);
    shot(&app, "opt.filebrowser.hidden_toggle");
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Hidden toggle", "off");
        assert_toggle(screen, "Show hidden", "off");
    });
    app.finish();
}

#[test]
fn option_show_hidden_on() {
    let app = option_browser("s05-show-hidden", "filebrowser.show_hidden=on", CHOOSE_DIR);
    app.key("backspace");
    let home = app.wait_screen("fixture home", |screen| {
        in_dialog(screen, ".zshrc") && !in_dialog(screen, PAY)
    });
    assert_dialog(&home, &[".zshrc", ".config", ".ssh", HIDDEN_BOX], &[PAY]);
    shot(&app, "opt.filebrowser.show_hidden");
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Show hidden", "on");
        assert_toggle(screen, "Hidden toggle", "on");
    });
    app.finish();
}

#[test]
fn option_type_ahead_off() {
    let app = option_browser("s05-type-ahead", "filebrowser.type_ahead=off", CHOOSE_DIR);
    app.key("c");
    app.key("space");
    let chosen = app.wait_text(CHOSEN_CWD);
    assert!(host_line(&chosen).contains("~/src"));
    see(
        &chosen,
        &[CHOSEN_CWD],
        &["customer-portal", CHOSEN_DIR, "checked: true", CHOOSE_DIR],
    );
    shot(&app, "opt.filebrowser.type_ahead");
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Type-ahead", "off");
        assert_select(screen, "Choose", "folders", &["files", "any"]);
    });
    app.finish();
}

#[test]
fn option_keys_vim() {
    let app = page("s05-keys", Some("filebrowser.keys=vim"), 120, 40);
    app.wait_text("Browse…");
    wheel_until(&app, true, |visible| {
        visible.contains("URL entry") && visible.contains("vim key map")
    });
    let keys = app.settle();
    see(
        &keys,
        &["vim key map", "URL entry", "unbound"],
        &["desktop key map", "Ctrl+F"],
    );
    assert!(
        row_with(&keys, "URL entry").contains('g'),
        "vim URL binding\n{}",
        text(&keys)
    );
    shot(&app, "opt.filebrowser.keys");
    wheel_until(&app, false, |visible| {
        visible.contains("Browse…") && !visible.contains("opens the modal")
    });
    // A click on the scrolled Browse… cell focuses it and does not activate it.
    click_browse(&app);
    if !text(&app.screen()).contains(CHOOSE_DIR) {
        app.key("enter");
    }
    app.wait_text(CHOOSE_DIR);
    app.key("h");
    let parent = app.wait_screen("vim parent", |screen| {
        in_dialog(screen, "todo.md") && !in_dialog(screen, PAY)
    });
    assert_dialog(&parent, &["design/", "todo.md"], &[PAY]);
    app.key("escape");
    app.wait_absent(CHOOSE_DIR);
    confirm_option(&app, |screen| {
        assert_select(screen, "Keys", "vim", &["desktop"]);
        assert_select(screen, "Dismiss", "escape", &["light", "never"]);
    });
    app.finish();
}

#[test]
fn option_url_entry() {
    let app = option_browser("s05-url", "filebrowser.url_entry=on", CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(&opened, &[GIT, CHOOSE_DIR], &[URL_PAY]);
    click_where(&app, GIT, |hit| hit.y > 16);
    flash_settle(&app);
    let url = app.wait_text(KNOWN_PAY);
    assert_dialog(&url, &[KNOWN_PAY, KNOWN_PORTAL, "Git URL"], &[PAY]);
    app.key("enter");
    app.wait_text("Enter a repository URL");
    app.type_text(INVALID_URL);
    app.key("enter");
    let invalid = app.wait_text("repository not found");
    assert_dialog(&invalid, &["repository not found", INVALID_URL], &[URL_PAY]);
    shot(&app, "opt.filebrowser.url_entry");
    clear_url(&app);
    app.type_text(VALID_PAY);
    app.key("enter");
    let paid = app.wait_text(URL_PAY);
    assert!(host_line(&paid).contains(VALID_PAY));
    see(&paid, &[URL_PAY], &["repository not found", CHOOSE_DIR]);
    confirm_option(&app, |screen| {
        assert_toggle(screen, "URL entry", "on");
        assert_toggle(screen, "Option check box", "on");
    });
    app.finish();
}

#[test]
fn option_checkbox_off() {
    let app = option_browser("s05-checkbox", "filebrowser.checkbox=off", CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(&opened, &[CHOOSE_DIR, PAY, HIDDEN_BOX], &[READ_ONLY]);
    shot(&app, "opt.filebrowser.checkbox");
    app.key("p");
    app.key("space");
    let chosen = app.wait_text("checked: false");
    assert!(
        text(&chosen).contains("Chosen { path: ~/src/payments-platform, checked: false }"),
        "choose without the checkbox\n{}",
        text(&chosen)
    );
    see(
        &chosen,
        &["checked: false"],
        &["read-only", READ_ONLY, "checked: true"],
    );
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Option check box", "off");
        assert_toggle(screen, "Step line", "off");
    });
    app.finish();
}

#[test]
fn option_stepper_on() {
    let app = option_browser("s05-stepper", "filebrowser.stepper=on", CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(&opened, &[STEPPER, CHOOSE_DIR, PAY], &[]);
    shot(&app, "opt.filebrowser.stepper");
    confirm_option(&app, |screen| {
        assert_toggle(screen, "Step line", "on");
        assert_select(screen, "Choose", "folders", &["files", "any"]);
    });
    app.finish();
}

#[test]
fn option_dismiss_light() {
    let app = option_browser("s05-dismiss", "filebrowser.dismiss=light", CHOOSE_DIR);
    let opened = app.settle();
    assert_dialog(&opened, &[CHOOSE_DIR, PAY], &["Cancelled"]);
    app.click(Point { x: 110, y: 20 });
    let cancelled = app.wait_screen("outside click cancels", |screen| {
        !text(screen).contains(CHOOSE_DIR) && text(screen).contains("Cancelled")
    });
    assert!(host_line(&cancelled).contains("none yet"));
    assert!(logged(&cancelled, "Cancelled"));
    see(&cancelled, &["none yet"], &[CHOOSE_DIR, CHOSEN_DIR]);
    shot(&app, "opt.filebrowser.dismiss");
    confirm_option(&app, |screen| {
        assert_select(screen, "Dismiss", "light", &["escape", "never"]);
        assert_select(screen, "Keys", "desktop", &["vim"]);
    });
    app.finish();
}
