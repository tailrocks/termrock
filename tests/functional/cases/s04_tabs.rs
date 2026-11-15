//! Tabs route, sections, specimens, compare levels, tab changes, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s04_tabs/` and each case id's dots become underscores.

use std::time::Duration;

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{App, Launch, Point, SHOWCASE, find, find_all, row_text, text, try_launch};

const TITLE: &str = "Tabs";
const BLURB: &str =
    "Tab items (icons, dirty, busy, error, closable), rules, overflow and nested strips";
const MENUS_BLURB: &str =
    "Context menus: anchors, placement, reasons, danger, separators, keys and press-drag-release";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const CRUMB: &str = "orders › Structure › Columns";
const ACCENT: SnapshotColor = SnapshotColor::Rgb(72, 224, 84);
const NEUTRAL: SnapshotColor = SnapshotColor::Rgb(77, 77, 77);
const DIMMED: SnapshotColor = SnapshotColor::Rgb(38, 38, 38);
const ACCENT_UNDER_DIALOG: SnapshotColor = SnapshotColor::Rgb(115, 115, 115);
const DANGER: SnapshotColor = SnapshotColor::Rgb(255, 107, 107);

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s04_tabs/{}", case_id.replace('.', "_"))
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

fn page_opt(pair: &'static str) -> [&'static str; 4] {
    ["--page", "tabs", "--opt", pair]
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(
            !visible.contains(item),
            "forbidden {item:?} is visible\n{visible}"
        );
    }
}

fn lines_of(screen: &TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect()
}

fn line_after(screen: &TerminalSnapshot, label: &str) -> String {
    let lines = lines_of(screen);
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    lines
        .get(index + 1)
        .unwrap_or_else(|| panic!("no value under {label:?}\n{}", text(screen)))
        .clone()
}

fn line_after_last(screen: &TerminalSnapshot, label: &str) -> String {
    let lines = lines_of(screen);
    let index = lines
        .iter()
        .rposition(|line| line.contains(label))
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)));
    lines
        .get(index + 1)
        .unwrap_or_else(|| panic!("no value under {label:?}\n{}", text(screen)))
        .clone()
}

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str) {
    let line = line_after(screen, label);
    assert!(
        line.contains(value),
        "{label} shows {line:?}, expected {value}\n{}",
        text(screen)
    );
}

fn locate(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
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
    hits[0]
}

fn click_where(app: &App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let hit = locate(app, label, pred);
    app.click(hit);
    hit
}

fn click_tab(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x < 24);
}

fn click_strip(app: &App, label: &str) -> Point {
    click_where(app, label, |hit| hit.x > 24 && hit.y < 12)
}

fn click_lowest(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x > 24)
        .collect();
    assert!(!hits.is_empty(), "missing {label:?}\n{}", text(&screen));
    hits.sort_by_key(|hit| hit.y);
    app.click(*hits.last().expect("hit"));
}

fn close_marks(screen: &TerminalSnapshot) -> Vec<Point> {
    find_all(screen, "×")
        .into_iter()
        .filter(|hit| hit.y == 7 && hit.x > 24)
        .collect()
}

fn rule_fg(screen: &TerminalSnapshot) -> SnapshotColor {
    for x in 24..screen.cols() {
        let cell = screen.cell_at(x, 8).expect("rule cell");
        if cell.text() == "━" {
            return cell.fg();
        }
    }
    panic!("no active rule on row 8\n{}", text(screen));
}

fn cell_fg(screen: &TerminalSnapshot, point: Point) -> SnapshotColor {
    screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell")
        .fg()
}

/// Paused motion keeps a click's pressed color until something else redraws.
fn clear_flash(app: &App, cols: u16, rows: u16) {
    std::thread::sleep(Duration::from_millis(200));
    app.resize(121, rows);
    let hopped = format!("121×{rows}");
    app.wait_text(&hopped);
    app.resize(cols, rows);
    if cols >= 100 {
        app.wait_text(&format!("{cols}×{rows}"));
    } else {
        app.wait_screen("narrow size", |screen| {
            let visible = text(screen);
            !visible.contains(&hopped) && visible.contains(TITLE)
        });
    }
}

fn shot(app: &App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn shot_owned(
    app: &App,
    visual_case: &str,
    _owner: &str,
    _also: bool,
    _precondition: &str,
    _args: &[&str],
) {
    app.visual(&visual_id(visual_case));
}

fn options_page(app: &App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Active rule");
    clear_flash(app, 120, 40);
    app.wait_text("Active rule")
}

fn playground(app: &App) -> TerminalSnapshot {
    click_tab(app, "Playground");
    app.wait_text("orders.sql");
    clear_flash(app, 120, 40);
    app.wait_text(WIDE)
}

fn focus_editor(app: &App) {
    click_strip(app, "Overview");
    app.wait_text("Overview");
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "tabs"];
    let app = open("s04-route-page", 120, 40, &args);
    let screen = app.wait_text(TITLE);
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "orders.sql",
            "Import",
            "Migrate",
            CRUMB,
            "none yet",
            "middle-click closes",
            "› Tabs",
        ],
        &[
            OVERVIEW,
            MENUS_BLURB,
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Unsaved changes",
            "untitled",
            "closed Import",
        ],
    );
    assert!(
        row_text(&screen, 7).contains("orders.sql"),
        "editor strip missing\n{}",
        text(&screen)
    );
    assert_eq!(find(&screen, CRUMB).y, 27, "default crumb row");
    shot(
        &app,
        "sc.tabs.route",
        "--page tabs opens the playground with the sidebar focus on Tabs",
        &args,
    );

    let app = open("s04-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    for _ in 0..23 {
        app.key("]");
    }
    let entered = app.wait_text(BLURB);
    see(
        &entered,
        &[TITLE, BLURB, "orders.sql", CAP_PLAY],
        &[OVERVIEW, MENUS_BLURB, "Page options"],
    );
    app.key("]");
    let next = app.wait_screen("menus page", |screen| {
        let visible = text(screen);
        visible.contains("Context menus:") && !visible.contains(BLURB)
    });
    see(&next, &["Menus", "Context menus:"], &[BLURB, "orders.sql"]);
    app.key("[");
    let back = app.wait_text(BLURB);
    see(
        &back,
        &[TITLE, BLURB, "orders.sql"],
        &[MENUS_BLURB, OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[BLURB, "orders.sql"]);
    if !text(&overview).contains("Tabs") {
        click_side(&app, "Navigation");
        app.wait_text("Tabs");
    }
    click_side(&app, "Tabs");
    let pointed = app.wait_text(BLURB);
    see(
        &pointed,
        &[TITLE, BLURB, "orders.sql", CAP_PLAY, "› Tabs"],
        &[OVERVIEW, MENUS_BLURB, "Page options", "Compare levels"],
    );
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "tabs"];
    let app = open("s04-sections", 80, 24, &args);
    let narrow_play = app.wait_text(TITLE);
    see(
        &narrow_play,
        &[TITLE, "Playground", "orders.sql", "Import", "Overview"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "Unsaved changes",
        ],
    );
    assert!(
        row_text(&narrow_play, 7).contains('×'),
        "narrow strip hid the close mark\n{}",
        text(&narrow_play)
    );
    shot(
        &app,
        "sc.tabs.playground.80x24",
        "playground at 80 by 24 before the terminal grows, with no section caption",
        &args,
    );

    click_tab(&app, "Options");
    app.wait_text("Active rule");
    clear_flash(&app, 80, 24);
    let narrow_opt = app.wait_text("Active rule");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Options",
            "Active rule",
            "Show ×",
            "× target width",
            "Key map",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "orders.sql",
            WIDE,
            "truecolor",
            "Body gap",
            "Passive",
            "many",
            "vim",
        ],
    );
    assert_select(&narrow_opt, "Active rule", "auto");
    assert_select(&narrow_opt, "Show ×", "always");
    assert_select(&narrow_opt, "× target width", "3");
    assert_select(&narrow_opt, "Close keys", "x-delete");
    assert!(
        !line_after(&narrow_opt, "Key map").contains("default"),
        "80 by 24 showed the Key map value\n{}",
        text(&narrow_opt)
    );
    assert!(
        text(&narrow_opt).contains("New-tab + on"),
        "narrow new-tab toggle changed\n{}",
        text(&narrow_opt)
    );
    shot(
        &app,
        "sc.tabs.options.80x24",
        "options at 80 by 24 before the terminal grows, with the lower controls still below the fold",
        &args,
    );

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    clear_flash(&app, 80, 24);
    let narrow_states = app.wait_text("○ Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            "States",
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "a.sql",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            WIDE,
            "truecolor",
            "● Compare levels",
            "overflow",
            "narrow",
            "1›",
        ],
    );
    shot(
        &app,
        "sc.tabs.states.80x24",
        "states at 80 by 24 before the terminal grows, showing rest and clipping error, overflow, and narrow",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_text("1› +1");
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "busy",
            "error",
            "overflow",
            "narrow",
            "▏Logs",
            "▏Data",
            "⠋",
            "Logs !",
            "7› +7",
            "1› +1",
            "a.sql",
            "▦ users",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "selected",
            "disabled",
            "editing",
            "empty",
            "loading",
            "No rows",
        ],
    );
    shot(
        &app,
        "sc.tabs.states.120x40",
        "states caption and every drawn specimen at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.tabs.states",
        "states is the visible Tabs section",
        &args,
    );
    let specimens = [
        (
            "sc.tabs.specimen.rest",
            "rest",
            "the rest specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.hover",
            "hover",
            "the hover specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.pressed",
            "pressed",
            "the pressed specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.focused",
            "▏Logs",
            "the focused specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.focused_hover",
            "focus + hover",
            "the focus + hover specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.busy",
            "⠋",
            "the busy specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.error",
            "Logs !",
            "the error specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.overflow",
            "7› +7",
            "the overflow specimen is fully visible at 120 by 40 and compare levels is off",
        ),
        (
            "sc.tabs.specimen.narrow",
            "1› +1",
            "the narrow specimen is fully visible at 120 by 40 and compare levels is off",
        ),
    ];
    for (case_id, mark, precondition) in specimens {
        let screen = app.screen();
        see(
            &screen,
            &[mark, "○ Compare levels", CAP_STATES, WIDE],
            &[
                "selected",
                "disabled",
                "editing",
                "empty",
                "loading",
                "● Compare levels",
            ],
        );
        shot(&app, case_id, precondition, &args);
    }

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    clear_flash(&app, 120, 40);
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "New-tab + on",
            "Digits jump on",
            "Wrap off",
            "Middle-click closes on",
            "Overflow +N on",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "orders.sql",
            "New-tab + off",
            "Digits jump off",
            "Wrap on",
            "Middle-click closes off",
            "Overflow +N off",
            "owner-focused",
            "vim",
        ],
    );
    assert_select(&wide_opt, "Active rule", "auto");
    assert_select(&wide_opt, "Show ×", "always");
    assert_select(&wide_opt, "× target width", "3");
    assert_select(&wide_opt, "× button", "default");
    assert_select(&wide_opt, "New-tab key", "n");
    assert_select(&wide_opt, "Close keys", "x-delete");
    assert_select(&wide_opt, "Key map", "default");
    assert_select(&wide_opt, "Nav keys", "desktop");
    assert_select(&wide_opt, "+N button", "default");
    assert_select(&wide_opt, "Passive", "off");
    assert_select(&wide_opt, "Body gap", "1");
    assert!(
        line_after_last(&wide_opt, "Tabs").contains("many"),
        "Tabs count changed\n{}",
        text(&wide_opt)
    );
    shot(
        &app,
        "sc.tabs.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.tabs.options",
        "options is the visible Tabs section",
        &args,
    );

    let wide_play = playground(&app);
    see(
        &wide_play,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "orders.sql",
            "Import",
            "7›",
            CRUMB,
            "none yet",
            "1–9 jump",
            "x close",
            "n new tab",
            "middle-click closes",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Unsaved changes",
            "closed Import",
            "untitled",
            OVERVIEW,
        ],
    );
    assert_eq!(find(&wide_play, CRUMB).y, 27, "wide crumb row");
    shot(
        &app,
        "sc.tabs.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.tabs.playground",
        "playground is the visible Tabs section",
        &args,
    );
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "tabs"];
    let app = open("s04-compare", 120, 40, &args);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", CAP_STATES],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 12);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    for _ in 0..8 {
        if text(&app.screen()).contains("no color") {
            break;
        }
        app.key("pagedown");
        app.settle();
    }
    clear_flash(&app, 120, 40);
    for _ in 0..8 {
        if text(&app.screen()).contains("no color") {
            break;
        }
        app.key("pagedown");
        app.settle();
    }
    let bands = app.wait_text("no color");
    see(
        &bands,
        &[TITLE, "no color", WIDE, "Compare levels"],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot_owned(
        &app,
        "sc.tabs.compare-levels.120x40",
        "sc.tabs.compare-levels",
        false,
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_screen("compare at 80", |screen| {
        let visible = text(screen);
        visible.contains("● Compare levels")
            && visible.contains("truecolor")
            && !visible.contains(WIDE)
    });
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot_owned(
        &app,
        "sc.tabs.compare-levels.80x24",
        "sc.tabs.compare-levels",
        true,
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &["● Compare levels", "no color", "Page options"],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Active rule");
    assert_select(&options, "Active rule", "auto");
    assert!(
        line_after_last(&options, "Tabs").contains("many"),
        "compare levels changed the tab count\n{}",
        text(&options)
    );
    see(
        &options,
        &[TITLE, "› Tabs", "auto", "many"],
        &["no color", "● Compare levels", MENUS_BLURB],
    );
}

#[test]
fn tabs_selects_orders_then_closes_import() {
    let args = ["--page", "tabs"];
    let app = open("s04-tabs-change", 120, 40, &args);
    let before = app.wait_text("none yet");
    see(
        &before,
        &[
            "orders.sql",
            "Import",
            "7›",
            "none yet",
            "Overview · pinned (not closable)",
            CRUMB,
        ],
        &[
            "Unsaved changes",
            "closed Import",
            "untitled",
            "Activated(1)",
            "Export",
        ],
    );
    assert!(row_text(&before, 7).contains("Import"));
    shot_owned(
        &app,
        "sc.tabs.tabs.before",
        "sc.tabs.tabs",
        false,
        "editor strip before a tab change, with Overview active and Import still open",
        &args,
    );

    click_strip(&app, "orders.sql");
    let orders = app.wait_text("Activated(1) · orders.sql");
    see(
        &orders,
        &[
            "orders.sql · unsaved · closable",
            "Activated(1) · orders.sql",
            "Import",
        ],
        &["Unsaved changes", "closed Import", "untitled"],
    );
    app.key("right");
    let import = app.wait_text("Activated(2) · Import");
    see(
        &import,
        &[
            "Import · busy · closable",
            "Activated(2) · Import",
            "orders.sql",
        ],
        &["Unsaved changes", "closed Import"],
    );
    app.key("x");
    let closed = app.wait_text("Close(2) · closed Import");
    see(
        &closed,
        &[
            "Close(2) · closed Import",
            "Export",
            "Migrate · failed · closable",
            "orders.sql",
            "6›",
        ],
        &["Unsaved changes", "untitled", "Import · busy"],
    );
    assert!(
        !row_text(&closed, 7).contains("Import"),
        "Import stayed on the strip\n{}",
        row_text(&closed, 7)
    );
    clear_flash(&app, 120, 40);
    let settled = app.wait_text("Close(2) · closed Import");
    see(
        &settled,
        &["Close(2) · closed Import", "Export", "6›", "orders.sql"],
        &["Unsaved changes", "untitled"],
    );
    assert!(!row_text(&settled, 7).contains("Import"));
    shot_owned(
        &app,
        "sc.tabs.tabs.after",
        "sc.tabs.tabs",
        true,
        "editor strip after selecting Import and closing it, with Export visible",
        &args,
    );

    app.key("1");
    let overview = app.wait_text("Activated(0) · Overview");
    see(
        &overview,
        &[
            "Activated(0) · Overview",
            "Overview · pinned (not closable)",
        ],
        &["untitled", "Unsaved changes"],
    );
    app.key("n");
    let created = app.wait_text("untitled-1.sql");
    see(
        &created,
        &[
            "New · opened untitled-1.sql",
            "untitled-1.sql · closable",
            "7 before · 0 after",
        ],
        &["Unsaved changes"],
    );
    app.key("down");
    let menu = app.wait_text("audit_log");
    see(
        &menu,
        &["audit_log", "invoices", "untitled-1.sql", "Export"],
        &["Unsaved changes"],
    );
    app.key("esc");
    let dismissed = app.wait_absent("audit_log");
    see(
        &dismissed,
        &["untitled-1.sql", "New · opened untitled-1.sql"],
        &["audit_log", "Unsaved changes"],
    );
    app.key("2");
    let back = app.wait_text("Activated(1) · orders.sql");
    see(
        &back,
        &["orders.sql · unsaved · closable", "1 before · 6 after"],
        &["Unsaved changes", "untitled-1.sql"],
    );
    app.key("x");
    let dialog = app.wait_text("Unsaved changes");
    see(
        &dialog,
        &[
            "Unsaved changes",
            "Save before leaving?",
            "1 unsaved change would be",
            "lost.",
            "Cancel",
            "Discard",
            "Save",
        ],
        &[
            "Kept orders.sql open",
            "changes discarded",
            "Saved and closed",
        ],
    );
    app.key("esc");
    let kept = app.wait_text("Kept orders.sql open");
    see(
        &kept,
        &["Kept orders.sql open", "orders.sql · unsaved · closable"],
        &["Unsaved changes", "changes discarded", "Saved and closed"],
    );
    assert!(row_text(&kept, 7).contains("orders.sql"));
    app.key("x");
    app.wait_text("Unsaved changes");
    click_lowest(&app, "Discard");
    let discarded = app.wait_text("Closed orders.sql, changes discarded");
    see(
        &discarded,
        &["Closed orders.sql, changes discarded"],
        &[
            "Unsaved changes",
            "Kept orders.sql open",
            "Saved and closed",
        ],
    );
    assert!(
        !row_text(&discarded, 7).contains("orders.sql"),
        "discard left orders.sql on the strip\n{}",
        row_text(&discarded, 7)
    );
    click_where(&app, "Data", |hit| hit.x > 24 && hit.y > 14);
    let data = app.wait_text("orders › Data");
    see(
        &data,
        &["orders › Data"],
        &[CRUMB, "orders › Structure › Indexes"],
    );
    click_where(&app, "Structure", |hit| hit.x > 24 && hit.y > 14);
    app.wait_text(CRUMB);
    click_where(&app, "Indexes", |hit| hit.x > 24 && hit.y > 14);
    let indexes = app.wait_text("orders › Structure › Indexes");
    see(
        &indexes,
        &["orders › Structure › Indexes"],
        &["orders › Data", "orders › Structure › Columns"],
    );

    let app = open("s04-tabs-save", 120, 40, &args);
    click_strip(&app, "orders.sql");
    app.wait_text("Activated(1) · orders.sql");
    app.key("x");
    app.wait_text("Unsaved changes");
    click_lowest(&app, "Save");
    let saved = app.wait_text("Saved and closed orders.sql");
    see(
        &saved,
        &["Saved and closed orders.sql", "Import"],
        &[
            "Unsaved changes",
            "changes discarded",
            "Kept orders.sql open",
        ],
    );
    assert!(!row_text(&saved, 7).contains("orders.sql"));
}

#[test]
fn tabs_d_marks_dirty_and_brackets_move_the_active_tab() {
    let args = ["--page", "tabs"];
    let app = open("s04-tabs-edit", 120, 40, &args);
    focus_editor(&app);
    let hinted = app.settle();
    see(&hinted, &["d Dirty", "[ ] Move"], &[]);

    app.key("right");
    app.wait_text("Activated(1) · orders.sql");
    app.key("d");
    let clean = app.wait_text("Toggled unsaved mark on orders.sql");
    see(
        &clean,
        &["orders.sql · closable"],
        &["orders.sql · unsaved"],
    );
    app.key("d");
    let dirty = app.wait_screen("unsaved again", |screen| {
        text(screen).contains("orders.sql · unsaved · closable")
    });
    see(&dirty, &["Toggled unsaved mark on orders.sql"], &[]);

    app.key("]");
    let moved = app.wait_text("Moved orders.sql to position 3");
    let row = row_text(&moved, 7);
    assert!(
        row.find("Import") < row.find("orders.sql"),
        "orders.sql did not move right\n{row}"
    );
    see(&moved, &["orders.sql · unsaved · closable"], &[]);

    app.key("[");
    app.wait_text("Moved orders.sql to position 2");
    app.key("[");
    app.wait_text("Moved orders.sql to position 1");
    app.key("[");
    let refused = app.settle();
    see(&refused, &["Moved orders.sql to position 1"], &[]);
    let row = row_text(&refused, 7);
    assert!(
        row.find("orders.sql") < row.find("Overview"),
        "orders.sql is first\n{row}"
    );
}

#[test]
fn tabs_d_waits_for_the_overflow_menu_to_close() {
    let args = ["--page", "tabs"];
    let app = open("s04-tabs-edit-menu", 120, 40, &args);
    focus_editor(&app);
    app.key("down");
    let menu = app.wait_text("audit_log");
    see(&menu, &["audit_log", "invoices"], &[]);
    app.key("d");
    let swallowed = app.settle();
    see(
        &swallowed,
        &["audit_log", "Overview · pinned (not closable)"],
        &["Toggled unsaved mark", "Overview · unsaved"],
    );
    app.key("esc");
    app.wait_absent("audit_log");
    app.key("d");
    let toggled = app.wait_text("Toggled unsaved mark on Overview");
    see(&toggled, &["Overview · unsaved"], &["audit_log"]);
}

fn assert_other_defaults(screen: &TerminalSnapshot, changed: &str) {
    if changed != "rule" {
        assert_select(screen, "Active rule", "auto");
    }
    if changed != "reveal" {
        assert_select(screen, "Show ×", "always");
    }
    if changed != "count" {
        assert!(
            line_after_last(screen, "Tabs").contains("many"),
            "Tabs count changed\n{}",
            text(screen)
        );
    }
    if changed != "allow" {
        assert!(
            text(screen).contains("New-tab + on"),
            "new-tab toggle changed\n{}",
            text(screen)
        );
    }
}

#[test]
fn option_rule_accent_stays_brighter_under_the_dialog() {
    let auto = page_opt("tabs.rule=auto");
    let app = open("s04-rule-auto", 120, 40, &auto);
    click_strip(&app, "orders.sql");
    app.key("x");
    app.wait_text("Unsaved changes");
    clear_flash(&app, 120, 40);
    let dialog = app.wait_text("Unsaved changes");
    assert_eq!(rule_fg(&dialog), DIMMED, "auto rule under the dialog");
    see(
        &dialog,
        &["Unsaved changes", "orders.sql"],
        &["Kept orders.sql open"],
    );

    let accent = page_opt("tabs.rule=accent");
    let app = open("s04-rule-accent", 120, 40, &accent);
    let options = options_page(&app);
    assert_select(&options, "Active rule", "accent");
    assert_other_defaults(&options, "rule");
    playground(&app);
    click_strip(&app, "orders.sql");
    app.key("x");
    app.wait_text("Unsaved changes");
    clear_flash(&app, 120, 40);
    let dialog = app.wait_text("Unsaved changes");
    assert_eq!(
        rule_fg(&dialog),
        ACCENT_UNDER_DIALOG,
        "accent rule under the dialog"
    );
    see(
        &dialog,
        &["Unsaved changes", "Save before leaving?", "orders.sql"],
        &["Kept orders.sql open", "neutral", "focus-only"],
    );
    shot(
        &app,
        "opt.tabs.rule",
        "accent keeps a brighter active rule under the unsaved dialog than auto",
        &accent,
    );

    let neutral = page_opt("tabs.rule=neutral");
    let app = open("s04-rule-neutral", 120, 40, &neutral);
    let resting = app.wait_text("orders.sql");
    assert_eq!(rule_fg(&resting), NEUTRAL, "neutral rule at rest");
    let options = options_page(&app);
    assert_select(&options, "Active rule", "neutral");
    assert_other_defaults(&options, "rule");

    let focus = page_opt("tabs.rule=focus-only");
    let app = open("s04-rule-focus", 120, 40, &focus);
    let resting = app.wait_text("orders.sql");
    assert_eq!(
        rule_fg(&resting),
        NEUTRAL,
        "focus-only rule before the strip is focused"
    );
    click_strip(&app, "orders.sql");
    app.wait_text("Activated(1) · orders.sql");
    clear_flash(&app, 120, 40);
    let focused = app.wait_text("Activated(1) · orders.sql");
    assert_eq!(
        rule_fg(&focused),
        ACCENT,
        "focus-only rule once the strip is focused"
    );
    let options = options_page(&app);
    assert_select(&options, "Active rule", "focus-only");
    assert_other_defaults(&options, "rule");
}

#[test]
fn option_close_reveal_hover_hides_the_mark_until_hover() {
    let args = page_opt("tabs.close_reveal=hover");
    let app = open("s04-reveal", 120, 40, &args);
    let options = options_page(&app);
    assert_select(&options, "Show ×", "hover");
    assert_other_defaults(&options, "reveal");
    let screen = playground(&app);
    assert!(
        close_marks(&screen).is_empty(),
        "resting hover reveal still drew a close mark\n{}",
        row_text(&screen, 7)
    );
    see(
        &screen,
        &["orders.sql", "Import", "7› +7", "x close"],
        &["Unsaved changes", "untitled"],
    );
    shot(
        &app,
        "opt.tabs.close_reveal",
        "hover hides every close mark on the resting editor strip",
        &args,
    );
    let orders = locate(&app, "orders.sql", |hit| hit.y == 7 && hit.x > 24);
    app.move_to(orders);
    let hovered = app.settle();
    assert!(
        !close_marks(&hovered).is_empty(),
        "hovering orders.sql did not reveal a close mark\n{}",
        row_text(&hovered, 7)
    );
}

#[test]
fn option_close_width_five_reaches_two_cells_left() {
    let narrow = page_opt("tabs.close_w=1");
    let app = open("s04-close-w-1", 120, 40, &narrow);
    let options = options_page(&app);
    assert_select(&options, "× target width", "1");
    assert_other_defaults(&options, "width");
    let screen = playground(&app);
    let marks = close_marks(&screen);
    assert_eq!(marks.len(), 3, "width 1 strip\n{}", row_text(&screen, 7));
    let column = marks[0].x;
    app.click(Point {
        x: column - 2,
        y: marks[0].y,
    });
    let missed = app.settle();
    see(
        &missed,
        &["orders.sql"],
        &["Unsaved changes", "closed Import"],
    );

    let wide = page_opt("tabs.close_w=5");
    let app = open("s04-close-w-5", 120, 40, &wide);
    let options = options_page(&app);
    assert_select(&options, "× target width", "5");
    assert_select(&options, "Show ×", "always");
    assert_other_defaults(&options, "width");
    let screen = playground(&app);
    let marks = close_marks(&screen);
    assert_eq!(marks.len(), 3, "width 5 strip\n{}", row_text(&screen, 7));
    assert_eq!(marks[0].x, column, "close glyph column moved");
    app.click(Point {
        x: marks[0].x - 2,
        y: marks[0].y,
    });
    app.wait_text("Unsaved changes");
    clear_flash(&app, 120, 40);
    let dialog = app.wait_text("Unsaved changes");
    see(
        &dialog,
        &["Unsaved changes", "orders.sql", "Save before leaving?"],
        &["Kept orders.sql open", "closed Import"],
    );
    shot(
        &app,
        "opt.tabs.close_w",
        "close width 5 opens the unsaved dialog from a click two cells left of the mark",
        &wide,
    );
}

#[test]
fn option_close_button_danger_paints_the_hovered_mark() {
    let args = page_opt("tabs.close_button=danger");
    let app = open("s04-danger", 120, 40, &args);
    let play = app.wait_text("orders.sql");
    let marks = close_marks(&play);
    assert_eq!(marks.len(), 3, "danger strip\n{}", row_text(&play, 7));
    let options = options_page(&app);
    assert_select(&options, "× button", "danger");
    assert_select(&options, "× target width", "3");
    assert_other_defaults(&options, "button");
    shot(
        &app,
        "opt.tabs.close_button",
        "the × button control shows danger while the other options stay at their defaults",
        &args,
    );
    let screen = playground(&app);
    let mark = close_marks(&screen)[0];
    app.move_to(mark);
    let hovered = app.settle();
    assert_eq!(
        cell_fg(&hovered, mark),
        DANGER,
        "danger hover fg on the orders.sql close mark"
    );
    assert_eq!(
        hovered
            .cell_at(u32::from(mark.x), u32::from(mark.y))
            .expect("mark")
            .text(),
        "×"
    );
}

#[test]
fn option_allow_new_off_removes_the_plus_and_n() {
    let args = page_opt("tabs.allow_new=off");
    let app = open("s04-allow-new", 120, 40, &args);
    let options = options_page(&app);
    assert!(text(&options).contains("New-tab + off"));
    assert_other_defaults(&options, "allow");
    let screen = playground(&app);
    let row = row_text(&screen, 7);
    assert!(row.contains("+7"), "missing overflow count\n{row}");
    assert!(
        !row.trim_end().ends_with('+'),
        "trailing new-tab plus stayed\n{row}"
    );
    see(
        &screen,
        &["orders.sql", "x close", "1–9 jump", "middle-click closes"],
        &["n new tab", "untitled", "Unsaved changes"],
    );
    shot(
        &app,
        "opt.tabs.allow_new",
        "allow new off removes the trailing plus and the n new tab hint",
        &args,
    );
    focus_editor(&app);
    app.key("n");
    let ignored = app.settle();
    see(
        &ignored,
        &["orders.sql", "7›"],
        &["untitled", "New · opened"],
    );
}

#[test]
fn option_new_key_t_opens_a_tab_and_none_ignores_n() {
    let tee = page_opt("tabs.new_key=t");
    let app = open("s04-new-t", 120, 40, &tee);
    let options = options_page(&app);
    assert_select(&options, "New-tab key", "t");
    assert_other_defaults(&options, "new");
    let screen = playground(&app);
    see(
        &screen,
        &["t new tab", "orders.sql", "+"],
        &["n new tab", "untitled"],
    );
    assert!(row_text(&screen, 7).trim_end().ends_with('+'));
    shot(
        &app,
        "opt.tabs.new_key",
        "new-tab key t replaces n new tab on the keys line",
        &tee,
    );
    focus_editor(&app);
    app.key("n");
    let ignored = app.settle();
    see(&ignored, &["t new tab"], &["untitled"]);
    app.key("t");
    let created = app.wait_text("untitled-1.sql");
    see(
        &created,
        &["New · opened untitled-1.sql", "t new tab"],
        &["n new tab"],
    );

    let none = page_opt("tabs.new_key=none");
    let app = open("s04-new-none", 120, 40, &none);
    let options = options_page(&app);
    assert_select(&options, "New-tab key", "none");
    assert!(text(&options).contains("New-tab + on"));
    let screen = playground(&app);
    see(
        &screen,
        &["x close", "1–9 jump", "orders.sql"],
        &["new tab", "untitled"],
    );
    assert!(row_text(&screen, 7).trim_end().ends_with('+'));
    focus_editor(&app);
    app.key("n");
    let ignored = app.settle();
    see(&ignored, &["orders.sql"], &["untitled", "New · opened"]);
}

#[test]
fn option_close_keys_delete_closes_import() {
    let args = page_opt("tabs.close_keys=delete");
    let app = open("s04-close-delete", 120, 40, &args);
    let options = options_page(&app);
    assert_select(&options, "Close keys", "delete");
    assert_other_defaults(&options, "keys");
    let screen = playground(&app);
    see(
        &screen,
        &["Delete close", "Import", "n new tab"],
        &["x close", "closed Import", "Unsaved changes"],
    );
    shot(
        &app,
        "opt.tabs.close_keys",
        "close keys delete replaces x close on the keys line",
        &args,
    );
    click_strip(&app, "Import");
    app.wait_text("Activated(2) · Import");
    app.key("x");
    let kept = app.settle();
    see(
        &kept,
        &["Import · busy", "Delete close"],
        &["closed Import", "Unsaved changes"],
    );
    assert!(row_text(&kept, 7).contains("Import"));
    app.key("delete");
    let closed = app.wait_text("Close(2) · closed Import");
    see(
        &closed,
        &["Close(2) · closed Import", "Delete close", "Export"],
        &["Unsaved changes"],
    );
    assert!(!row_text(&closed, 7).contains("Import"));
}

#[test]
fn option_keymap_ctrl_closes_with_ctrl_w() {
    let args = page_opt("tabs.keymap=ctrl");
    let app = open("s04-keymap", 120, 40, &args);
    let options = options_page(&app);
    assert_select(&options, "Key map", "ctrl");
    assert_other_defaults(&options, "map");
    let screen = playground(&app);
    see(
        &screen,
        &[
            "Ctrl+W close",
            "Ctrl+T new tab",
            "1–9 jump",
            "middle-click closes",
            "Import",
        ],
        &[
            "x close",
            "n new tab",
            "hidden tabs",
            "untitled",
            "closed Import",
        ],
    );
    shot(
        &app,
        "opt.tabs.keymap",
        "ctrl map shows Ctrl+W close and Ctrl+T new tab",
        &args,
    );
    click_strip(&app, "Import");
    app.wait_text("Activated(2) · Import");
    app.key("x");
    let kept = app.settle();
    see(&kept, &["Import"], &["closed Import", "untitled"]);
    app.key("ctrl+w");
    let closed = app.wait_text("Close(2) · closed Import");
    see(
        &closed,
        &["Close(2) · closed Import", "Ctrl+T new tab"],
        &["Unsaved changes", "untitled"],
    );
    app.key("ctrl+t");
    let created = app.wait_text("untitled-1.sql");
    see(
        &created,
        &["New · opened untitled-1.sql", "Ctrl+W close"],
        &["n new tab"],
    );
}

#[test]
fn option_digit_jump_off_keeps_the_current_tab() {
    let args = page_opt("tabs.digit_jump=off");
    let app = open("s04-digits", 120, 40, &args);
    let options = options_page(&app);
    assert!(text(&options).contains("Digits jump off"));
    assert_other_defaults(&options, "digit");
    let screen = playground(&app);
    see(
        &screen,
        &["x close", "n new tab", "hidden tabs", "orders.sql"],
        &["1–9 jump", "untitled"],
    );
    shot(
        &app,
        "opt.tabs.digit_jump",
        "digits jump off removes 1–9 jump from the keys line",
        &args,
    );
    click_strip(&app, "orders.sql");
    app.wait_text("Activated(1) · orders.sql");
    app.key("1");
    let stayed = app.settle();
    see(
        &stayed,
        &[
            "orders.sql · unsaved · closable",
            "Activated(1) · orders.sql",
        ],
        &["Activated(0)", "1–9 jump"],
    );
}

#[test]
fn option_nav_vim_moves_with_l() {
    let args = page_opt("tabs.nav=vim");
    let app = open("s04-nav", 120, 40, &args);
    let options = options_page(&app);
    assert_select(&options, "Nav keys", "vim");
    assert_other_defaults(&options, "nav");
    playground(&app);
    focus_editor(&app);
    app.key("l");
    app.wait_text("Activated(1) · orders.sql");
    clear_flash(&app, 120, 40);
    let moved = app.wait_text("Activated(1) · orders.sql");
    see(
        &moved,
        &[
            "←→ h l move",
            "orders.sql · unsaved · closable",
            "Activated(1) · orders.sql",
            "Import",
        ],
        &["untitled", "closed Import"],
    );
    shot(
        &app,
        "opt.tabs.nav",
        "vim nav shows h l move and l selects orders.sql",
        &args,
    );
    app.key("h");
    let back = app.wait_text("Activated(0) · Overview");
    see(
        &back,
        &["←→ h l move", "Overview · pinned (not closable)"],
        &["Activated(1)"],
    );
    app.key("right");
    let arrows = app.wait_text("Activated(1) · orders.sql");
    see(&arrows, &["orders.sql · unsaved"], &["untitled"]);
}

#[test]
fn option_wrap_on_reaches_events_from_overview() {
    let args = page_opt("tabs.wrap=on");
    let app = open("s04-wrap", 120, 40, &args);
    let options = options_page(&app);
    assert!(text(&options).contains("Wrap on"));
    assert_other_defaults(&options, "wrap");
    playground(&app);
    focus_editor(&app);
    app.key("left");
    app.wait_text("Activated(10) · events");
    clear_flash(&app, 120, 40);
    let wrapped = app.wait_text("Activated(10) · events");
    see(
        &wrapped,
        &[
            "Activated(10) · events",
            "events · closable",
            "7 before · 0 after",
            "wraps",
        ],
        &["0 before · 7 after", "Unsaved changes"],
    );
    assert!(row_text(&wrapped, 7).contains("events"));
    shot(
        &app,
        "opt.tabs.wrap",
        "wrap on from Overview selects events",
        &args,
    );

    let app = open("s04-wrap-off", 120, 40, &["--page", "tabs"]);
    focus_editor(&app);
    app.key("left");
    let stayed = app.settle();
    see(
        &stayed,
        &["Overview", "0 before · 7 after"],
        &["Activated(10)", "events · closable", "wraps"],
    );
}

#[test]
fn option_middle_click_off_drops_the_keys_line() {
    let args = page_opt("tabs.middle_click=off");
    let app = open("s04-middle", 120, 40, &args);
    let options = options_page(&app);
    assert!(text(&options).contains("Middle-click closes off"));
    assert_other_defaults(&options, "middle");
    let screen = playground(&app);
    see(
        &screen,
        &["x close", "n new tab", "1–9 jump", "orders.sql", "Import"],
        &["middle-click closes", "untitled", "closed Import"],
    );
    shot(
        &app,
        "opt.tabs.middle_click",
        "middle-click off removes middle-click closes from the keys line",
        &args,
    );
}

#[test]
fn option_overflow_menu_off_drops_the_more_count() {
    let args = page_opt("tabs.overflow_menu=off");
    let app = open("s04-overflow", 120, 40, &args);
    let options = options_page(&app);
    assert!(text(&options).contains("Overflow +N off"));
    assert_other_defaults(&options, "overflow");
    let screen = playground(&app);
    let row = row_text(&screen, 7);
    assert!(row.contains("7›"), "missing overflow chevron\n{row}");
    assert!(!row.contains("+7"), "more count stayed\n{row}");
    assert!(row.trim_end().ends_with('+'), "new-tab plus missing\n{row}");
    see(
        &screen,
        &["orders.sql", "x close", "n new tab"],
        &["hidden tabs", "+7", "audit_log"],
    );
    shot(
        &app,
        "opt.tabs.overflow_menu",
        "overflow menu off removes the +N count and keeps the 7 chevron",
        &args,
    );
    focus_editor(&app);
    app.key("down");
    let closed = app.settle();
    see(
        &closed,
        &["orders.sql", "7›"],
        &["audit_log", "hidden tabs"],
    );
}

#[test]
fn option_more_button_tight_keeps_the_same_strip() {
    let args = page_opt("tabs.more_button=tight");
    let app = open("s04-more", 120, 40, &args);
    let play = app.wait_text("orders.sql");
    let row = row_text(&play, 7);
    assert!(row.contains("+7"), "tight strip dropped +7\n{row}");
    assert!(row.contains("orders.sql"), "tight strip\n{row}");
    assert_eq!(
        close_marks(&play).len(),
        3,
        "tight strip close marks\n{row}"
    );
    let options = options_page(&app);
    assert_select(&options, "+N button", "tight");
    assert_select(&options, "× target width", "3");
    assert_other_defaults(&options, "more");
    shot(
        &app,
        "opt.tabs.more_button",
        "the +N button control shows tight while the other options stay at their defaults",
        &args,
    );
}

#[test]
fn option_passive_owner_focused_shows_both_gutters() {
    let owned = page_opt("tabs.passive=owner-focused");
    let app = open("s04-passive-owner", 120, 40, &owned);
    let options = options_page(&app);
    assert_select(&options, "Passive", "owner-focused");
    assert_other_defaults(&options, "passive");
    let screen = playground(&app);
    see(
        &screen,
        &[
            "passive · the owner holds the keys and shows focus",
            "▎◆ Overview",
            "› Tabs",
            "orders.sql",
        ],
        &["1–9 jump", "x close", "n new tab"],
    );
    shot(
        &app,
        "opt.tabs.passive",
        "owner-focused draws a focus gutter on Overview and on the sidebar",
        &owned,
    );

    let on = page_opt("tabs.passive=on");
    let app = open("s04-passive-on", 120, 40, &on);
    let options = options_page(&app);
    assert_select(&options, "Passive", "on");
    let screen = playground(&app);
    see(
        &screen,
        &[
            "passive · the owner holds the keys",
            "› Tabs",
            "orders.sql",
            "Import",
        ],
        &["shows focus", "▎◆", "1–9 jump", "x close", "n new tab"],
    );
}

#[test]
fn option_gap_zero_lifts_the_crumb() {
    let args = page_opt("tabs.gap=0");
    let app = open("s04-gap", 120, 40, &args);
    let options = options_page(&app);
    assert_select(&options, "Body gap", "0");
    assert_other_defaults(&options, "gap");
    let screen = playground(&app);
    assert_eq!(
        find(&screen, CRUMB).y,
        25,
        "gap 0 crumb row\n{}",
        text(&screen)
    );
    see(
        &screen,
        &[CRUMB, "Structure", "Columns", "orders.sql", "Data"],
        &["Unsaved changes"],
    );
    shot(
        &app,
        "opt.tabs.gap",
        "body gap 0 draws the nested crumb on the row above the default",
        &args,
    );
}

#[test]
fn option_count_few_stops_at_migrate() {
    let args = page_opt("tabs.count=few");
    let app = open("s04-few", 120, 40, &args);
    let options = options_page(&app);
    assert!(line_after_last(&options, "Tabs").contains("few"));
    assert_select(&options, "Active rule", "auto");
    assert!(text(&options).contains("New-tab + on"));
    let screen = playground(&app);
    let row = row_text(&screen, 7);
    assert!(row.contains("Migrate"), "few strip\n{row}");
    assert!(!row.contains("7›"), "few strip overflowed\n{row}");
    assert!(!row.contains("+7"), "few strip more count\n{row}");
    assert!(!row.contains("Export"), "few strip showed Export\n{row}");
    assert!(row.trim_end().ends_with('+'));
    see(
        &screen,
        &["Overview", "orders.sql", "Import", "0 before · 0 after"],
        &["7 before", "7›", "Export", "events"],
    );
    shot(
        &app,
        "opt.tabs.count",
        "few tabs shows four items and no overflow chevron",
        &args,
    );
    focus_editor(&app);
    app.key("left");
    let stayed = app.settle();
    see(
        &stayed,
        &["Overview", "0 before · 0 after"],
        &["Activated(10)", "events · closable"],
    );
    app.key("end");
    let end = app.wait_text("Activated(3) · Migrate");
    see(
        &end,
        &["Activated(3) · Migrate", "Migrate · failed · closable"],
        &["Export", "7›"],
    );
    app.key("right");
    let held = app.settle();
    see(
        &held,
        &["Activated(3) · Migrate", "Migrate · failed"],
        &["Export", "Activated(4)"],
    );
}
