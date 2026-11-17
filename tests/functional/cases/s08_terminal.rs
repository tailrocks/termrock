//! Terminal route, sections, specimens, compare levels, the seam, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s08_terminal/` and each case id's dots become underscores.
//!
//! `TerminalPage::new` calls `reset(None)`, and paused motion never ticks, so
//! every launch stays on that reset frame.

use tui_test::TerminalSnapshot;

use crate::bins::showcase;
use crate::support::{
    App, Launch, Point, cell_bg, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Text & code › Terminal";
const TITLE: &str = "Terminal";
const BLURB: &str = "drag the seam · step rail reports";
const BLURB_NARROW: &str = "Viewport with scrollback";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "terminal"];
const COMMAND: &str = "payments-platform ❯ jackin launch";
const PROFILE: &str = "profile";
const LIVE: &str = "LIVE";
const CONFIG: &str = "jackin.toml";
const RESOLVE: &str = "Resolve workspace";
const RUNNING: &str = "0 of 7 · running";
const FOLLOWING: &str = "2 lines · following";
const QUEUED: &str = "Pull base image";
const GUTTER: char = '▎';

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

const DRAWN: &[&str] = &["rest", "focused", "selected", "empty", "overflow", "narrow"];

const FORBIDDEN: &[&str] = &["network unreachable", "0.1 s", "layer "];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s08_terminal/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, showcase());
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Components");
    app
}

fn page(name: &str, opt: &str, cols: u16, rows: u16) -> App {
    let args = ["--page", "terminal", "--opt", opt];
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

fn select_token(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = lines
        .iter()
        .enumerate()
        .find(|(row, line)| *row >= 6 && line.contains(label))
        .map(|(row, _)| row)?;
    let column = lines[index].find(label)?;
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).map(String::as_str).unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    Some(
        rest.split_whitespace()
            .find(|token| *token != "▎")
            .unwrap_or("")
            .to_string(),
    )
}

fn value_under(screen: &TerminalSnapshot, label: &str) -> String {
    select_token(screen, label)
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)))
}

fn select_is(screen: &TerminalSnapshot, label: &str, value: &str) -> bool {
    select_token(screen, label).as_deref() == Some(value)
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

/// Grid labels sit under the column titles. The legend's "focused" is higher.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (20..48).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
    })
}

fn content_hit(screen: &TerminalSnapshot, label: &str) -> Point {
    let hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 26)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one content {label:?}, got {hits:?}\n{}",
        text(screen)
    );
    hits[0]
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
    let screen = app.wait_text(label);
    let hit = find_all(&screen, label)
        .into_iter()
        .find(|hit| hit.x < 24)
        .unwrap_or_else(|| panic!("no sidebar row {label:?}\n{}", text(&screen)));
    app.click(hit);
}

fn click_run(app: &App) {
    let screen = app.wait_text("Run with a failure");
    let hits: Vec<_> = find_all(&screen, "Run")
        .into_iter()
        .filter(|hit| hit.y > 4 && hit.x >= 26)
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            let tail: String = line.chars().skip(hit.x as usize).collect();
            tail.starts_with("Run") && !tail.starts_with("Run with")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected the Run button, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
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
    // One wheel is three lines. A larger burst skips the one-line colour
    // caption inside a 24-row viewport, and a resize can already be past it.
    let point = Point { x: 28, y: 16 };
    let mut previous = String::new();
    for _ in 0..90 {
        let visible = text(&app.settle());
        if visible.contains(needle) {
            return;
        }
        if visible == previous {
            break;
        }
        previous = visible;
        app.wheel(point, true, 1);
    }
    for _ in 0..90 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 1);
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

fn gutter_count(screen: &TerminalSnapshot) -> usize {
    (0..screen.rows())
        .filter(|row| row_text(screen, *row).contains(GUTTER))
        .count()
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn glyph_at(screen: &TerminalSnapshot, point: Point) -> char {
    row_text(screen, u32::from(point.y))
        .chars()
        .nth(point.x as usize)
        .unwrap_or('?')
}

fn seam_point(screen: &TerminalSnapshot) -> Point {
    let rail = content_hit(screen, "Step rail");
    let mut x = rail.x;
    while x > 0 {
        x -= 1;
        let glyph = glyph_at(screen, Point { x, y: rail.y });
        if glyph == '│' || glyph == '┃' {
            return Point {
                x,
                y: rail.y.saturating_add(2),
            };
        }
    }
    panic!("seam missing beside Step rail\n{}", text(screen));
}

fn assert_only_column(before: &TerminalSnapshot, after: &TerminalSnapshot, column: u16) {
    assert_eq!((before.cols(), before.rows()), (after.cols(), after.rows()));
    let mut changed = false;
    for y in 0..before.rows() {
        for x in 0..before.cols() {
            let left = before.cell_at(x, y);
            let right = after.cell_at(x, y);
            if left != right {
                assert_eq!(
                    x,
                    u32::from(column),
                    "cell {x},{y} changed outside the seam\n{}",
                    text(after)
                );
                changed = true;
            }
        }
    }
    assert!(changed, "the seam column did not restyle");
}

fn assert_reset(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            COMMAND,
            PROFILE,
            "dev",
            CONFIG,
            LIVE,
            RESOLVE,
            RUNNING,
            FOLLOWING,
            QUEUED,
            "queued",
            "Run",
            "Run with a failure",
            "01",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "network unreachable",
            "0.1 s",
            "layer ",
            "✓",
        ],
    );
    let resolve = row_with(screen, RESOLVE);
    assert!(
        resolve.contains('⠋') && resolve.contains("01") && !resolve.contains('✓'),
        "Resolve workspace is not the running step: {resolve:?}"
    );
    let queued = row_with(screen, QUEUED);
    assert!(
        queued.contains("queued") && queued.contains("02") && !queued.contains('✓'),
        "Pull base image is not still queued: {queued:?}"
    );
    assert_eq!(gutter_count(screen), 1, "more than one focus gutter");
    let seam = seam_point(screen);
    assert_eq!(
        glyph_at(screen, seam),
        '│',
        "rest seam is not the thin rule"
    );
}

fn assert_reset_narrow(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            TITLE,
            BLURB_NARROW,
            "Playground",
            "payments-platform",
            "jackin",
            PROFILE,
            "Run",
            "0/7",
            FOLLOWING,
            "Resolve",
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
            "network unreachable",
            "0.1 s",
            "layer ",
            "✓",
            LIVE,
        ],
    );
    assert_eq!(gutter_count(screen), 1, "narrow playground gutters");
}

fn assert_option_defaults(screen: &TerminalSnapshot) {
    assert_toggle(screen, "Wrap long lines", "off");
    assert_select(screen, "Lines kept", "2000", &["20"]);
    assert_select(screen, "Horizontal scroll", "theme", &["on", "off"]);
    assert_select(screen, "Scrollbar gap", "1", &["0", "2"]);
    assert_select(screen, "Track click", "page", &["jump"]);
    assert_select(screen, "Viewport keys", "default", &["vim", "no-page"]);
    assert_select(screen, "Command keys", "default", &["custom"]);
    assert_toggle(screen, "Find marks", "off");
    assert_toggle(screen, "Selectable rail", "off");
    assert_toggle(screen, "Numbered steps", "on");
    assert_select(screen, "Rail keys", "default", &["vim", "no-page"]);
}

fn assert_drawn(screen: &TerminalSnapshot) {
    for label in DRAWN {
        assert!(
            grid_label(screen, label),
            "states grid clipped {label}\n{}",
            text(screen)
        );
    }
}

fn assert_omitted(screen: &TerminalSnapshot) {
    for label in OMITTED {
        assert!(
            !grid_label(screen, label),
            "{label} is drawn, but states_grid only lists Rest, Focused, Selected, Empty, Overflow, and Narrow\n{}",
            text(screen)
        );
    }
}

fn confirm_options_ready(
    app: &App,
    ready: impl Fn(&TerminalSnapshot) -> bool,
    check: impl Fn(&TerminalSnapshot),
) {
    click_tab(app, "Options");
    let screen = app.wait_screen("page options", |screen| {
        text(screen).contains("Page options") && ready(screen)
    });
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("jackin");
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

fn unchanged_reset(screen: &TerminalSnapshot) {
    assert_reset(screen);
}

fn mark_colors(screen: &TerminalSnapshot) -> (tui_test::SnapshotColor, tui_test::SnapshotColor) {
    let hit = content_hit(screen, "jackin launch");
    let marked = cell_bg(screen, hit);
    let plain = cell_bg(
        screen,
        Point {
            x: hit.x + 7,
            y: hit.y,
        },
    );
    (marked, plain)
}

fn park(app: &App) {
    app.move_to(Point { x: 40, y: 1 });
    app.settle();
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s08-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_reset(&screen);
    flash_settle(&app);
    let settled = app.settle();
    assert_reset(&settled);
    shot(&app, "sc.terminal.route");
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("jackin launch");
    see(
        &entered,
        &[CRUMB, BLURB, COMMAND, RESOLVE, RUNNING],
        &[OVERVIEW, "Page options", "Text & code › Diff"],
    );
    app.key("[");
    let left = app.wait_text("Text & code › Diff");
    see(
        &left,
        &["Text & code › Diff"],
        &[CRUMB, COMMAND, "jackin launch"],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, COMMAND, RESOLVE],
        &["Text & code › Diff", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, COMMAND]);
    click_side(&app, "Text & code");
    app.wait_text("Code editor");
    click_side(&app, "Terminal");
    let pointed = app.wait_text("jackin launch");
    see(
        &pointed,
        &[CRUMB, BLURB, COMMAND, LIVE, "Run with a failure"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s08-sections", 80, 24, PAGE_ARGS);
    app.wait_text("jackin");
    flash_settle(&app);
    let narrow_play = app.settle();
    assert_size(&app, 80, 24);
    assert_reset_narrow(&narrow_play);
    shot(&app, "sc.terminal.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_toggle(&narrow_opt, "Wrap long lines", "off");
    assert_select(&narrow_opt, "Lines kept", "2000", &["20"]);
    assert_select(&narrow_opt, "Horizontal scroll", "theme", &["on", "off"]);
    assert_select(&narrow_opt, "Scrollbar gap", "1", &["0", "2"]);
    assert_select(&narrow_opt, "Track click", "page", &["jump"]);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Viewport keys", "Find marks"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            COMMAND,
            WIDE,
            "truecolor",
            "80×24",
            "Numbered steps",
            "Rail keys",
        ],
    );
    shot(&app, "sc.terminal.options.80x24");

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
    assert!(
        grid_label(&narrow_states, "focused"),
        "narrow states clipped focused\n{}",
        text(&narrow_states)
    );
    assert_omitted(&narrow_states);
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "focused", "cargo build"],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            COMMAND,
            WIDE,
            "truecolor",
            "80×24",
            LEGEND,
        ],
    );
    shot(&app, "sc.terminal.states.80x24");

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
            BLURB,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "Viewport",
            "Wrapped",
            "Step rail",
            "cargo build",
            "Finished",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            COMMAND,
            "256 colors",
        ],
    );
    assert_eq!(gutter_count(&wide_states), 1, "states focus gutter");
    for (case_id, label) in [
        ("sc.terminal.specimen.rest", "rest"),
        ("sc.terminal.specimen.focused", "focused"),
        ("sc.terminal.specimen.selected", "selected"),
        ("sc.terminal.specimen.empty", "empty"),
        ("sc.terminal.specimen.overflow", "overflow"),
        ("sc.terminal.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.terminal.states.120x40");
    shot(&app, "sc.terminal.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_option_defaults(&wide_opt);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", COMMAND],
    );
    shot(&app, "sc.terminal.options.120x40");
    shot(&app, "sc.terminal.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("jackin launch");
    assert_size(&app, 120, 40);
    assert_reset(&wide_play);
    shot(&app, "sc.terminal.playground.120x40");
    shot(&app, "sc.terminal.playground");
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
        &[CRUMB, "○ Compare levels", "rest", "Viewport", CAP_STATES],
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
    let point = Point { x: 28, y: 16 };
    for _ in 0..90 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel(point, true, 1);
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
    shot(&app, "sc.terminal.compare-levels.120x40");

    app.resize(80, 24);
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    reveal_text(&app, "no color");
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels", "80×24"],
    );
    shot(&app, "sc.terminal.compare-levels.80x24");

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
    assert_option_defaults(&options);
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "Wrap long lines"],
        &["Compare levels", OVERVIEW],
    );
    app.finish();
}

#[test]
fn run_resets_and_seam_drag_is_held() {
    let app = open("s08-run", 120, 40, PAGE_ARGS);
    let rest = app.wait_text("jackin launch");
    assert_reset(&rest);
    flash_settle(&app);
    let rest = app.settle();
    assert_reset(&rest);
    shot(&app, "sc.terminal.run.before");

    click_run(&app);
    flash_settle(&app);
    let ran = app.settle();
    assert_reset(&ran);
    see(&ran, &[COMMAND, PROFILE, LIVE, RESOLVE, RUNNING], FORBIDDEN);
    shot(&app, "sc.terminal.run.after");

    click_where(&app, "Run with a failure", |hit| hit.y > 4 && hit.x >= 26);
    flash_settle(&app);
    let failed = app.settle();
    assert_reset(&failed);
    see(
        &failed,
        &[COMMAND, "jackin.toml", LIVE, "queued"],
        FORBIDDEN,
    );

    park(&app);
    let parked = app.settle();
    let seam = seam_point(&parked);
    assert_eq!(
        glyph_at(&parked, seam),
        '│',
        "seam left rest before the drag"
    );
    app.move_to(seam);
    let hovered = app.settle();
    assert_eq!(
        glyph_at(&hovered, seam),
        '┃',
        "hover did not thicken the seam"
    );
    assert_only_column(&parked, &hovered, seam.x);
    assert_eq!(
        glyph_at(
            &hovered,
            Point {
                x: seam.x - 2,
                y: seam.y
            }
        ),
        '│',
        "hover thickened the viewport border"
    );

    let held_at = Point {
        x: seam.x + 4,
        y: seam.y,
    };
    app.down(seam);
    app.drag_to(held_at);
    let held = app.settle();
    let moved = seam_point(&held);
    assert!(
        moved.x > seam.x,
        "drag did not move the seam: {seam:?} -> {moved:?}"
    );
    assert_eq!(
        glyph_at(&held, moved),
        '┃',
        "held seam is not the heavy rule"
    );
    assert_eq!(
        glyph_at(
            &held,
            Point {
                x: moved.x - 2,
                y: moved.y,
            }
        ),
        '│',
        "the drag thickened a cell beside the seam"
    );
    see(&held, &[COMMAND, PROFILE, LIVE, "Resolve", "01"], FORBIDDEN);
    shot(&app, "sc.terminal.seam.held");
    app.up(held_at);
    flash_settle(&app);
    park(&app);
    let released = app.settle();
    see(
        &released,
        &[COMMAND, PROFILE, CONFIG, LIVE, "Resolve"],
        FORBIDDEN,
    );
    assert_eq!(
        glyph_at(&released, seam_point(&released)),
        '│',
        "the seam stayed heavy after release"
    );
    app.finish();
}

#[test]
fn option_wrap_on_leaves_the_reset_frame() {
    let app = page("s08-wrap", "terminal.wrap=on", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Lines kept", "2000"),
        |screen| {
            assert_toggle(screen, "Wrap long lines", "on");
            assert_select(screen, "Lines kept", "2000", &["20"]);
            assert_select(screen, "Horizontal scroll", "theme", &["on", "off"]);
        },
    );
    finish_option(&app, "opt.terminal.wrap", unchanged_reset);
}

#[test]
fn option_max_lines_20_leaves_the_reset_frame() {
    let app = page("s08-lines", "terminal.max_lines=20", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_token(screen, "Lines kept").as_deref() == Some("20"),
        |screen| {
            assert_select(screen, "Lines kept", "20", &["2000"]);
            assert_toggle(screen, "Wrap long lines", "off");
            assert_select(screen, "Horizontal scroll", "theme", &["on", "off"]);
        },
    );
    finish_option(&app, "opt.terminal.max_lines", |screen| {
        unchanged_reset(screen);
        assert!(
            !flat(screen).contains("last 20 kept"),
            "two reset lines must not show a retention cap\n{}",
            text(screen)
        );
    });
}

#[test]
fn option_horizontal_off_leaves_the_reset_frame() {
    let app = page("s08-horizontal", "terminal.horizontal=off", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| {
            select_is(screen, "Horizontal scroll", "off") && select_is(screen, "Lines kept", "2000")
        },
        |screen| {
            assert_select(screen, "Horizontal scroll", "off", &["theme", "on"]);
            assert_toggle(screen, "Wrap long lines", "off");
            assert_select(screen, "Lines kept", "2000", &["20"]);
        },
    );
    finish_option(&app, "opt.terminal.horizontal", unchanged_reset);
}

#[test]
fn option_scrollbar_gap_0_leaves_the_reset_frame() {
    let app = page("s08-sgap", "terminal.scrollbar_gap=0", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Scrollbar gap", "0"),
        |screen| {
            assert_select(screen, "Scrollbar gap", "0", &["1", "2"]);
            assert_select(screen, "Track click", "page", &["jump"]);
            assert_toggle(screen, "Wrap long lines", "off");
        },
    );
    finish_option(&app, "opt.terminal.scrollbar_gap", unchanged_reset);
}

#[test]
fn option_track_click_jump_leaves_the_reset_frame() {
    let app = page("s08-jump", "terminal.track_click=jump", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Track click", "jump"),
        |screen| {
            assert_select(screen, "Track click", "jump", &["page"]);
            assert_select(screen, "Scrollbar gap", "1", &["0", "2"]);
            assert_select(screen, "Viewport keys", "default", &["vim", "no-page"]);
        },
    );
    finish_option(&app, "opt.terminal.track_click", unchanged_reset);
}

#[test]
fn option_nav_vim_leaves_the_reset_frame() {
    let app = page("s08-nav", "terminal.nav=vim", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Viewport keys", "vim"),
        |screen| {
            assert_select(screen, "Viewport keys", "vim", &["default", "no-page"]);
            assert_select(screen, "Rail keys", "default", &["vim", "no-page"]);
            assert_select(screen, "Command keys", "default", &["custom"]);
        },
    );
    finish_option(&app, "opt.terminal.nav", unchanged_reset);
}

#[test]
fn option_keymap_custom_leaves_the_reset_frame() {
    let app = page("s08-keymap", "terminal.keymap=custom", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Command keys", "custom"),
        |screen| {
            assert_select(screen, "Command keys", "custom", &["default"]);
            assert_select(screen, "Viewport keys", "default", &["vim", "no-page"]);
            assert_toggle(screen, "Find marks", "off");
        },
    );
    finish_option(&app, "opt.terminal.keymap", unchanged_reset);
}

#[test]
fn option_mark_on_paints_jackin() {
    let app = page("s08-mark", "terminal.mark=on", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Lines kept", "2000"),
        |screen| {
            assert_toggle(screen, "Find marks", "on");
            assert_toggle(screen, "Numbered steps", "on");
            assert_select(screen, "Lines kept", "2000", &["20"]);
        },
    );
    finish_option(&app, "opt.terminal.mark", |screen| {
        see(
            screen,
            &[COMMAND, PROFILE, CONFIG, LIVE, RESOLVE, RUNNING],
            FORBIDDEN,
        );
        let (marked, plain) = mark_colors(screen);
        assert_ne!(
            marked, plain,
            "jackin is not marked differently from launch"
        );
        assert!(
            row_with(screen, RESOLVE).contains("01"),
            "mark removed the step numbers"
        );
    });
}

#[test]
fn option_selectable_on_leaves_the_reset_frame() {
    let app = page("s08-selectable", "terminal.selectable=on", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Rail keys", "default"),
        |screen| {
            assert_toggle(screen, "Selectable rail", "on");
            assert_toggle(screen, "Numbered steps", "on");
            assert_select(screen, "Rail keys", "default", &["vim", "no-page"]);
        },
    );
    finish_option(&app, "opt.terminal.selectable", unchanged_reset);
}

#[test]
fn option_numbered_off_drops_the_numbers() {
    let app = page("s08-numbered", "terminal.numbered=off", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Rail keys", "default"),
        |screen| {
            assert_toggle(screen, "Numbered steps", "off");
            assert_toggle(screen, "Selectable rail", "off");
            assert_select(screen, "Rail keys", "default", &["vim", "no-page"]);
        },
    );
    finish_option(&app, "opt.terminal.numbered", |screen| {
        see(
            screen,
            &[COMMAND, PROFILE, CONFIG, LIVE, RESOLVE, RUNNING, "queued"],
            FORBIDDEN,
        );
        let resolve = row_with(screen, RESOLVE);
        assert!(
            !resolve.contains("01") && resolve.contains(RESOLVE),
            "numbers stayed on the running step: {resolve:?}"
        );
        let queued = row_with(screen, QUEUED);
        assert!(
            !queued.contains("02") && queued.contains("queued"),
            "numbers stayed on the queued step: {queued:?}"
        );
        assert!(
            flat(screen).contains("Resolve credentials"),
            "dropping the numbers did not give the label its full width\n{}",
            text(screen)
        );
    });
}

#[test]
fn option_rail_nav_vim_leaves_the_reset_frame() {
    let app = page("s08-rail", "terminal.rail_nav=vim", 120, 40);
    app.wait_text("jackin launch");
    confirm_options_ready(
        &app,
        |screen| select_is(screen, "Rail keys", "vim"),
        |screen| {
            assert_select(screen, "Rail keys", "vim", &["default", "no-page"]);
            assert_select(screen, "Viewport keys", "default", &["vim", "no-page"]);
            assert_toggle(screen, "Numbered steps", "on");
        },
    );
    finish_option(&app, "opt.terminal.rail_nav", unchanged_reset);
}
