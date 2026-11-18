//! Settings route, sections, specimens, compare levels, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s09_settings/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Screens › Settings";
const PREV: &str = "Text & code › Text utilities";
const NEXT: &str = "Screens › Task runner";
const TITLE: &str = "Project settings";
const BLURB: &str = "Composed: tabs, form, editable table, list, dialogs";
const BLURB_NARROW: &str = "Composed: tabs, form, editable table";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "settings"];
const NAME: &str = "payments-gateway";
const DESC: &str = "Handles checkout, invoicing and refund";
const UNSAVED: &str = "General · unsaved";
const SAVED: &str = "Settings saved ✓";
const BULLET: char = '\u{2022}';

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s09_settings/{}", case_id.replace('.', "_"))
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

fn assert_settings_options(
    screen: &TerminalSnapshot,
    section: &str,
    cell: &str,
    env: &str,
    rows: &str,
    dirty: &str,
    save: &str,
) {
    assert_one(
        screen,
        "Section",
        section,
        &["general", "members", "environment"],
    );
    assert_toggle(screen, "Members cell navigation", cell);
    assert_one(screen, "Environment select", env, &["multi", "single"]);
    assert_one(screen, "Description rows", rows, &["3", "2", "5"]);
    assert_toggle(screen, "Dirty mark on the tab", dirty);
    assert_toggle(screen, "Confirm on save", save);
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

fn click_field(app: &App, label: &str) {
    click_where(app, label, |hit| {
        hit.x > 26
            && hit.y > 6
            && hit.y < 36
            && !row_text(&app.screen(), u32::from(hit.y)).contains("cursor:")
    });
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

fn tab_strip(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| {
            line.contains("General") && line.contains("Members") && line.contains("Environment")
        })
        .unwrap_or_else(|| panic!("section tabs missing\n{}", text(screen)))
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn discard_quit(app: &App) {
    app.key("ctrl+q");
    app.wait_text("Unsaved changes");
    click_where(app, "Discard", |hit| hit.y > 8 && hit.y < 32);
    app.wait_exit("discard did not exit");
    app.expect_exited(0);
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

fn assert_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            NAME,
            DESC,
            "(●) Private",
            "Save changes",
            "No changes",
            "Auto-merge approved PRs",
            "Protect main branch",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Mira Okafor",
            "DATABASE_URL",
            UNSAVED,
            SAVED,
            "focus + hover",
            OVERVIEW,
        ],
    );
    assert!(
        !tab_strip(screen).contains(BULLET),
        "clean general tab is marked dirty\n{}",
        tab_strip(screen)
    );
}

fn assert_clean_options(screen: &TerminalSnapshot) {
    assert_settings_options(screen, "general", "on", "multi", "3", "on", "on");
    see(
        screen,
        &[
            "Page options",
            "Members cell navigation",
            "Environment select",
            "Description rows",
            "Dirty mark on the tab",
            "Confirm on save",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            NAME,
            "Mira Okafor",
            "focus + hover",
        ],
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s09-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    assert_playground(&screen);
    shot(&app, "sc.settings.route");
    app.finish();

    let app = open("s09-route-keys", 120, 40, &[]);
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
        PREV,
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(NAME);
    see(
        &entered,
        &[CRUMB, BLURB, NAME, "(●) Private"],
        &[OVERVIEW, "Page options", PREV],
    );
    app.key("[");
    let left = app.wait_text(PREV);
    see(&left, &[PREV], &[CRUMB, NAME, BLURB]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, NAME, "Save changes"], &[PREV, OVERVIEW]);
    app.key("]");
    let next = app.wait_text(NEXT);
    see(&next, &[NEXT], &[CRUMB, TITLE, "(●) Private"]);
    app.key("[");
    app.wait_text(CRUMB);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, NAME, BLURB]);
    click_side(&app, "Screens");
    click_side(&app, "Settings");
    let pointed = app.wait_text(NAME);
    see(
        &pointed,
        &[CRUMB, BLURB, NAME, "(●) Private", "No changes"],
        &[OVERVIEW, "Page options", "Compare levels", PREV],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s09-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(NAME);
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            BLURB_NARROW,
            "Playground",
            "General",
            "Members",
            "Environment",
            NAME,
            "Description",
            DESC,
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
            "Mira Okafor",
            "DATABASE_URL",
            "No changes",
            "Save changes",
            "focus + hover",
        ],
    );
    shot(&app, "sc.settings.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_clean_options(&narrow_opt);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "general"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "80×24",
            NAME,
            "focus + hover",
        ],
    );
    shot(&app, "sc.settings.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "hover", "pressed"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    for label in ["editing", "empty", "overflow", "narrow", "busy", "loading"] {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is on the 80 by 24 states screen\n{}",
            text(&narrow_states)
        );
    }
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "Section tabs",
            "General •",
        ],
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
            NAME,
            "focus + hover",
            "the live widget",
        ],
    );
    shot(&app, "sc.settings.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "error")
            && !grid_label(screen, "narrow")
    });
    assert_size(&app, 120, 40);
    for label in [
        "rest", "hover", "pressed", "focused", "selected", "disabled", "error",
    ] {
        assert!(
            grid_label(&wide_states, label),
            "wide states missing {label}\n{}",
            text(&wide_states)
        );
    }
    for label in ["editing", "empty", "overflow", "narrow", "busy", "loading"] {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is on the top states frame\n{}",
            text(&wide_states)
        );
    }
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
            "Section tabs",
            "Members",
            "Environment",
            "General •",
            "Could not load",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            NAME,
            "focus + hover",
            "256 colors",
            "No variables defined",
        ],
    );
    for (case_id, label) in [
        ("sc.settings.specimen.rest", "rest"),
        ("sc.settings.specimen.hover", "hover"),
        ("sc.settings.specimen.pressed", "pressed"),
        ("sc.settings.specimen.focused", "focused"),
        ("sc.settings.specimen.selected", "selected"),
        ("sc.settings.specimen.disabled", "disabled"),
        ("sc.settings.specimen.error", "error"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.settings.states.120x40");
    shot(&app, "sc.settings.states");

    reveal_text(&app, "narrow");
    let lower = app.settle();
    for label in ["editing", "empty", "overflow", "narrow"] {
        assert!(
            grid_label(&lower, label),
            "scrolled states missing {label}\n{}",
            text(&lower)
        );
    }
    assert!(
        !grid_label(&lower, "busy") && !grid_label(&lower, "loading"),
        "omitted specimen appeared after scroll\n{}",
        text(&lower)
    );
    see(
        &lower,
        &[
            "No rows",
            "No variables defined",
            "Mira Okafor and a",
            "Mira Okaf",
            "editing",
            "narrow",
        ],
        &["focus + hover", "Page options", NAME],
    );
    for (case_id, label) in [
        ("sc.settings.specimen.editing", "editing"),
        ("sc.settings.specimen.empty", "empty"),
        ("sc.settings.specimen.overflow", "overflow"),
        ("sc.settings.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the lower frame"
        );
        shot(&app, case_id);
    }

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_clean_options(&wide_opt);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "general"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", NAME],
    );
    shot(&app, "sc.settings.options.120x40");
    shot(&app, "sc.settings.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(NAME);
    assert_size(&app, 120, 40);
    assert_playground(&wide_play);
    shot(&app, "sc.settings.playground.120x40");
    shot(&app, "sc.settings.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s09-compare", 120, 40, PAGE_ARGS);
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
    shot(&app, "sc.settings.compare-levels.120x40");

    app.resize(80, 24);
    let narrow = app.wait_screen("compare redraw at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(TITLE)
    });
    if !text(&narrow).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "80×24", "○ Compare levels"],
    );
    shot(&app, "sc.settings.compare-levels.80x24");

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
    assert_clean_options(&options);
    see(
        &options,
        &["general", "Page options", CAP_OPT, CRUMB],
        &["Compare levels", OVERVIEW, "members"],
    );
    app.finish();
}

#[test]
fn cancel_save_keeps_the_edit() {
    let app = open("s09-settings", 120, 40, PAGE_ARGS);
    let before = app.wait_text(NAME);
    assert_size(&app, 120, 40);
    assert_playground(&before);
    shot(&app, "sc.settings.settings.before");

    click_field(&app, "Internal");
    flash_settle(&app);
    let dirty = app.wait_text(UNSAVED);
    see(
        &dirty,
        &[
            "(●) Internal",
            UNSAVED,
            "General •",
            "Save changes",
            "( ) Private",
        ],
        &["No changes", SAVED, "Save settings?"],
    );
    assert!(
        tab_strip(&dirty).contains(BULLET),
        "dirty general tab has no bullet\n{}",
        tab_strip(&dirty)
    );
    click_where(&app, "Save changes", |hit| hit.y > 10 && hit.y < 36);
    app.wait_text("Save settings?");
    flash_settle(&app);
    let dialog = app.wait_text("Save settings?");
    see(
        &dialog,
        &[
            "Save settings?",
            "branch protection applies",
            "to every open task.",
            "Cancel",
            "(●) Internal",
        ],
        &[SAVED, "No changes"],
    );
    click_where(&app, "Cancel", |hit| hit.y > 10 && hit.y < 30);
    flash_settle(&app);
    let after = app.wait_absent("Save settings?");
    see(
        &after,
        &["(●) Internal", UNSAVED, "General •", "Save changes", CRUMB],
        &[
            SAVED,
            "No changes",
            "Save settings?",
            "to every open task.",
            "(●) Private",
        ],
    );
    assert!(
        tab_strip(&after).contains(BULLET),
        "cancelling the confirm cleared the dirty mark\n{}",
        tab_strip(&after)
    );
    shot(&app, "sc.settings.settings.after");
    discard_quit(&app);
}

#[test]
fn option_tab_members() {
    let app = page("s09-tab", Some("settings.tab=members"), 120, 40);
    app.wait_text("6 members");
    confirm_options(&app, "6 members", |screen| {
        assert_settings_options(screen, "members", "on", "multi", "3", "on", "on");
    });
    let screen = app.settle();
    see(
        &screen,
        &[
            CAP_PLAY,
            "Members",
            "6 members",
            "Mira Okafor",
            "Invite",
            "Remove…",
            WIDE,
        ],
        &[
            "Page options",
            NAME,
            "No changes",
            "DATABASE_URL",
            "Compare levels",
            SAVED,
        ],
    );
    shot(&app, "opt.settings.tab");
    app.finish();
}

#[test]
fn option_cell_nav_off() {
    let app = page_opts(
        "s09-cell",
        &["settings.tab=members", "settings.cell_nav=off"],
        120,
        40,
    );
    app.wait_text("Mira Okafor");
    confirm_options(&app, "6 members", |screen| {
        assert_settings_options(screen, "members", "off", "multi", "3", "on", "on");
    });
    click_field(&app, "Mira Okafor");
    flash_settle(&app);
    app.key("enter");
    let screen = app.wait_text("Enter Edit");
    let row = row_with(&screen, "mira@acme.dev");
    assert!(
        row.contains('›'),
        "cell navigation off did not keep the row mark\n{row}"
    );
    see(
        &screen,
        &["6 members", "Mira Okafor", "Enter Edit", "Owner"],
        &[
            "Next cell",
            "EDIT",
            "Apply",
            "Page options",
            "DATABASE_URL",
            SAVED,
        ],
    );
    shot(&app, "opt.settings.cell_nav");
    app.finish();
}

#[test]
fn option_env_select_single() {
    let app = page_opts(
        "s09-env",
        &["settings.tab=environment", "settings.env_select=single"],
        120,
        40,
    );
    app.wait_text("DATABASE_URL");
    confirm_options(&app, "DATABASE_URL", |screen| {
        assert_settings_options(screen, "environment", "on", "single", "3", "on", "on");
    });
    click_field(&app, "DATABASE_URL");
    flash_settle(&app);
    let screen = app.wait_text("0 selected");
    let row = row_with(&screen, "DATABASE_URL");
    assert!(
        !row.contains('✓'),
        "single select checked the variable\n{row}"
    );
    see(
        &screen,
        &[
            "Environment variables",
            "0 selected",
            "DATABASE_URL",
            "Space Toggle",
            "REDIS_URL",
        ],
        &["1 selected", "✓", "Page options", NAME, SAVED, "No changes"],
    );
    shot(&app, "opt.settings.env_select");
    app.finish();
}

#[test]
fn option_desc_rows_two() {
    let app = page("s09-rows", Some("settings.desc_rows=2"), 120, 40);
    let play = app.wait_text(NAME);
    see(
        &play,
        &[
            CAP_PLAY,
            NAME,
            DESC,
            "(●) Private",
            "No changes",
            "Save changes",
            "Auto-merge approved PRs",
        ],
        &[
            "Page options",
            "Mira Okafor",
            "DATABASE_URL",
            UNSAVED,
            SAVED,
        ],
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let screen = app.settle();
    assert_settings_options(&screen, "general", "on", "multi", "2", "on", "on");
    see(
        &screen,
        &[CAP_OPT, "Description rows", "general"],
        &[CAP_PLAY, NAME, "Compare levels"],
    );
    shot(&app, "opt.settings.desc_rows");
    app.finish();
}

#[test]
fn option_tab_dirty_off() {
    let app = page("s09-dirty", Some("settings.tab_dirty=off"), 120, 40);
    app.wait_text(NAME);
    confirm_options(&app, NAME, |screen| {
        assert_settings_options(screen, "general", "on", "multi", "3", "off", "on");
    });
    click_field(&app, "Internal");
    flash_settle(&app);
    let screen = app.wait_text(UNSAVED);
    assert!(
        !tab_strip(&screen).contains(BULLET),
        "tab dirty off still drew the bullet\n{}",
        tab_strip(&screen)
    );
    see(
        &screen,
        &["(●) Internal", UNSAVED, "Save changes", "( ) Private"],
        &["No changes", SAVED, "Save settings?", "General •"],
    );
    shot(&app, "opt.settings.tab_dirty");
    discard_quit(&app);
}

#[test]
fn option_save_confirm_off() {
    let app = page("s09-save", Some("settings.save_confirm=off"), 120, 40);
    app.wait_text(NAME);
    confirm_options(&app, NAME, |screen| {
        assert_settings_options(screen, "general", "on", "multi", "3", "on", "off");
    });
    click_field(&app, "Internal");
    flash_settle(&app);
    app.wait_text(UNSAVED);
    click_where(&app, "Save changes", |hit| hit.y > 10 && hit.y < 36);
    flash_settle(&app);
    let screen = app.wait_text(SAVED);
    assert!(
        !tab_strip(&screen).contains(BULLET),
        "saved form kept the dirty bullet\n{}",
        tab_strip(&screen)
    );
    see(
        &screen,
        &["(●) Internal", "No changes", SAVED, "General"],
        &[
            UNSAVED,
            "Save settings?",
            "General •",
            "to every open task.",
            "(●) Private",
        ],
    );
    shot(&app, "opt.settings.save_confirm");
    app.finish();
}
