//! Dialog kinds route, sections, specimens, compare levels, options, and kinds.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s05_dialogkinds/` and each case id's dots become underscores.

use std::collections::BTreeSet;

use tui_test::TerminalSnapshot;

use crate::support::{Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Dialog kinds";
const BLURB: &str = "Forms, info, choices and help: the purpose-built modals on one layer stack";
const OVERVIEW: &str = "Tokens and principles behind every component";
const PICKERS: &str =
    "One modal list for files, tabs, levels, commands and branches: search, scope, status";
const POPUPS: &str = "Where popups go and how they close";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const INTERIM: &str = "121×40";

const SPECIMENS: &[(&str, &str)] = &[
    ("sc.dialogkinds.specimen.rest", "rest"),
    ("sc.dialogkinds.specimen.hover", "hover"),
    ("sc.dialogkinds.specimen.focused", "focused"),
    ("sc.dialogkinds.specimen.selected", "selected"),
    ("sc.dialogkinds.specimen.disabled", "disabled"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s05_dialogkinds/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
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

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    lines_of(screen)
        .into_iter()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
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

fn locate(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
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

fn click_where(app: &crate::support::App, label: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let hit = locate(app, label, pred);
    app.click(hit);
    hit
}

fn click_tab(app: &crate::support::App, label: &str) {
    click_where(app, label, |hit| hit.y == 4);
}

fn click_side(app: &crate::support::App, label: &str) {
    click_where(app, label, |hit| hit.x < 24);
}

fn click_page(app: &crate::support::App, label: &str) {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 24 && hit.y > 4 && hit.y < 38)
        .collect();
    assert!(
        !hits.is_empty(),
        "no {label:?} in the page\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.y);
    app.click(hits[hits.len() - 1]);
}

fn sidebar_has(app: &crate::support::App, label: &str) -> bool {
    find_all(&app.screen(), label)
        .into_iter()
        .any(|hit| hit.x < 24)
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn header_is(line: &str, label: &str) -> bool {
    if line.contains("depicts") || line.contains("Compare") {
        return false;
    }
    line.split_whitespace().any(|word| word == label)
}

fn specimen_header(line: &str) -> bool {
    SPECIMENS.iter().any(|(_, label)| header_is(line, label))
}

fn specimen_block(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let lines = lines_of(screen);
    let index = lines.iter().position(|line| header_is(line, label))?;
    let mut end = index + 1;
    while end < lines.len() && end - index < 16 {
        let line = &lines[end];
        if specimen_header(line) || line.contains("←→ Section") {
            break;
        }
        end += 1;
    }
    let block = lines[index..end].join("\n");
    let frame = block.contains("Split")
        && block.contains("Direction")
        && block.contains("Right")
        && block.contains("Down")
        && block.contains("Cancel")
        && block.contains("Next")
        && block.contains('╰');
    let ready = frame
        && match label {
            "selected" => block.contains("(●) Down") && block.contains("(●) Nightly"),
            "focused" => {
                block.contains('▏') && block.contains("(●) Right") && !block.contains("(●) Down")
            }
            "rest" | "hover" | "disabled" => {
                block.contains("(●) Right")
                    && block.contains("( ) Down")
                    && block.contains("(●) Stable")
            }
            _ => false,
        };
    ready.then_some(block)
}

/// A shot of the click frame can keep the 140ms pressed colour. Wait out the
/// flash, then resize away and back so the settled colour is drawn.
fn clear_flash(app: &crate::support::App, cols: u16, rows: u16, token: &str) {
    app.clear_flash_interim(cols, rows, token, (INTERIM, WIDE));
}

fn park(app: &crate::support::App, rows: u16) {
    app.move_to(Point {
        x: 2,
        y: rows.saturating_sub(1),
    });
}

fn option_page(app: &crate::support::App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options")
}

fn assert_option(screen: &TerminalSnapshot, label: &str, value: &str, toggle: bool) {
    let row = if toggle {
        row_with(screen, label)
    } else {
        line_after(screen, label)
    };
    let shown = if toggle {
        row.trim_end_matches(|ch: char| ch.is_whitespace() || ch == '┃' || ch == '┆')
            .ends_with(value)
    } else {
        row.contains(value)
    };
    assert!(
        shown,
        "{label} value {row:?} does not show {value}\n{}",
        text(screen)
    );
    if !toggle {
        for other in [
            "48", "72", "auto", "vim", "desktop", "danger", "default", "ctrl", "two", "three",
        ] {
            if other != value && row.contains(other) {
                panic!(
                    "{label} line also contains {other}: {row}\n{}",
                    text(screen)
                );
            }
        }
    }
}

fn assert_defaults_except(screen: &TerminalSnapshot, changed: &[(&str, &str, bool)]) {
    let defaults = [
        ("Width", "auto", false),
        ("Nav keys", "desktop", false),
        ("× button", "default", false),
        ("Buttons wrap", "off", true),
        ("Form stays open on Save", "off", true),
        ("Form meta", "on", true),
        ("Info error title", "off", true),
        ("Info key map", "default", false),
        ("Choice tones", "on", true),
        ("Choice stepper", "on", true),
        ("Choice buttons", "two", false),
        ("Dismiss", "default", false),
    ];
    for (label, value, toggle) in defaults {
        if let Some((_, next, _)) = changed.iter().find(|(name, _, _)| *name == label) {
            assert_option(screen, label, next, toggle);
        } else {
            assert_option(screen, label, value, toggle);
        }
    }
}

fn shoot_ready(app: &crate::support::App, done: &mut BTreeSet<&'static str>, args: &[&str]) {
    park(app, 40);
    let screen = app.settle();
    for (case_id, label) in SPECIMENS {
        if done.contains(case_id) {
            continue;
        }
        let Some(block) = specimen_block(&screen, label) else {
            continue;
        };
        assert!(
            block.contains(label),
            "{label} block lost its name\n{block}"
        );
        let visible = text(&screen);
        assert!(
            visible.contains("○ Compare levels") || visible.contains("←→ Section"),
            "compare levels is not off\n{visible}"
        );
        see(
            &screen,
            &[label, CAP_STATES, "Split", "Cancel"],
            &[
                "● Compare levels",
                "Page options",
                "focus + hover",
                CAP_PLAY,
                CAP_OPT,
            ],
        );
        shot(
            app,
            case_id,
            &format!(
                "the {label} specimen is fully visible at 120 by 40 and compare levels is off"
            ),
            args,
        );
        done.insert(case_id);
    }
}

fn close_to_page(app: &crate::support::App) {
    for _ in 0..6 {
        let visible = text(&app.screen());
        if visible.contains("Enter Open")
            && !visible.contains("Test connection")
            && !visible.contains("Open logs")
            && !visible.contains("Enter Choose")
        {
            return;
        }
        app.key("escape");
        let _ = app.settle();
    }
    assert!(
        text(&app.screen()).contains("Enter Open"),
        "a dialog stayed open\n{}",
        text(&app.screen())
    );
}

fn tabs(app: &crate::support::App, count: usize) {
    for _ in 0..count {
        app.key("tab");
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-route-page", 120, 40, &args);
    let screen = app.wait_text(TITLE);
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Add connection",
            "Server info",
            "Release channel",
            "Keyboard help",
            "Nothing yet",
            "width 62",
            "Save closes",
            "nav desktop",
        ],
        &[
            OVERVIEW,
            PICKERS,
            POPUPS,
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Form · Save",
        ],
    );
    shot(
        &app,
        "sc.dialogkinds.route",
        "--page dialogkinds opens the playground with Nothing yet and Save closes",
        &args,
    );
    app.finish();

    let app = open("s05-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    let mut entered = app.screen();
    for _ in 0..40 {
        if text(&entered).contains(BLURB) {
            break;
        }
        app.key("]");
        entered = app.settle();
    }
    see(
        &entered,
        &[TITLE, BLURB, "Add connection", CAP_PLAY],
        &[OVERVIEW, PICKERS, POPUPS, "Page options"],
    );
    app.key("]");
    let next = app.wait_text(POPUPS);
    see(&next, &[POPUPS], &[BLURB, "Nothing yet"]);
    app.key("[");
    let back = app.wait_text(BLURB);
    see(
        &back,
        &[TITLE, BLURB, "Add connection"],
        &[POPUPS, OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[BLURB, "Nothing yet"]);
    if !sidebar_has(&app, "Dialog kinds") {
        click_side(&app, "Overlays");
        app.wait_text("Dialog kinds");
    }
    click_side(&app, "Dialog kinds");
    let pointed = app.wait_text(BLURB);
    see(
        &pointed,
        &[TITLE, BLURB, "Add connection", CAP_PLAY, "Nothing yet"],
        &[OVERVIEW, "Page options", "Compare levels", POPUPS],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-sections", 80, 24, &args);
    let narrow_play = app.wait_text(TITLE);
    see(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "Forms, info, choices",
            "Add connection",
            "width 62",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "one layer stack",
            POPUPS,
        ],
    );
    assert_eq!(narrow_play.cols(), 80);
    assert_eq!(narrow_play.rows(), 24);
    shot(
        &app,
        "sc.dialogkinds.playground.80x24",
        "playground at 80 by 24 before the terminal grows, with no section caption and no size label",
        &args,
    );

    click_tab(&app, "Options");
    clear_flash(&app, 80, 24, "Page options");
    let narrow_opt = app.wait_text("Page options");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Width",
            "Nav keys",
            "× button",
            "Buttons wrap",
            "Form stays open on Save",
            "Form meta",
            "Info error title",
            "Info key map",
            "Choice tones",
            "Choice stepper",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "Add connection",
            "Choice buttons",
            "Dismiss",
        ],
    );
    assert_option(&narrow_opt, "Width", "auto", false);
    assert_option(&narrow_opt, "Nav keys", "desktop", false);
    assert_option(&narrow_opt, "× button", "default", false);
    assert_option(&narrow_opt, "Buttons wrap", "off", true);
    assert_option(&narrow_opt, "Form stays open on Save", "off", true);
    assert_option(&narrow_opt, "Form meta", "on", true);
    assert_option(&narrow_opt, "Info error title", "off", true);
    assert_option(&narrow_opt, "Info key map", "default", false);
    assert_option(&narrow_opt, "Choice tones", "on", true);
    assert_option(&narrow_opt, "Choice stepper", "on", true);
    shot(
        &app,
        "sc.dialogkinds.options.80x24",
        "options at 80 by 24 before the terminal grows, with the visible controls and their default values; Choice buttons and Dismiss stay below the fold",
        &args,
    );

    click_tab(&app, "States");
    clear_flash(&app, 80, 24, "Compare levels");
    let narrow_states = app.wait_text("rest");
    see(
        &narrow_states,
        &[
            TITLE,
            "States",
            "○ Compare levels",
            "rest",
            "Choice",
            "Split",
            "(●) Right",
            "Cancel",
            "Next",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            WIDE,
            "truecolor",
            "focus + hover",
            "Toned",
            "hover",
            "● Compare levels",
        ],
    );
    assert_eq!(narrow_states.cols(), 80);
    assert_eq!(narrow_states.rows(), 24);
    shot(
        &app,
        "sc.dialogkinds.states.80x24",
        "states at 80 by 24 before the terminal grows, showing the rest Choice specimen and clipping Toned and the later rows",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_screen("states wide", |screen| {
        let visible = text(screen);
        visible.contains(WIDE) && visible.contains(CAP_STATES) && visible.contains("rest")
    });
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "rest",
            "hover",
            "Choice",
            "Toned",
            "Split",
            "Stable",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "busy",
            "pressed",
        ],
    );
    shot(
        &app,
        "sc.dialogkinds.states.120x40",
        "states caption and the leading specimens at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.dialogkinds.states",
        "states is the visible Dialog kinds section",
        &args,
    );

    click_tab(&app, "Options");
    clear_flash(&app, 120, 40, "Page options");
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "Page options",
            "Choice buttons",
            "two",
            "Dismiss",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Add connection",
            "vim",
            "danger",
        ],
    );
    assert_defaults_except(&wide_opt, &[]);
    shot(
        &app,
        "sc.dialogkinds.options.120x40",
        "options caption and every default value at 120 by 40, including Choice buttons two and the Dismiss row set to default",
        &args,
    );
    shot(
        &app,
        "sc.dialogkinds.options",
        "options is the visible Dialog kinds section",
        &args,
    );

    click_tab(&app, "Playground");
    clear_flash(&app, 120, 40, CAP_PLAY);
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Add connection",
            "Nothing yet",
            "width 62",
            "Save closes",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            POPUPS,
            "Form · Save",
        ],
    );
    shot(
        &app,
        "sc.dialogkinds.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.dialogkinds.playground",
        "playground is the visible Dialog kinds section",
        &args,
    );
    app.finish();
}

#[test]
fn specimens_scroll_into_view() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-specimens", 120, 40, &args);
    click_tab(&app, "States");
    clear_flash(&app, 120, 40, "rest");
    let top = app.wait_text("○ Compare levels");
    see(
        &top,
        &[TITLE, CAP_STATES, "rest", "hover", "Toned", "(●) Right"],
        &[
            "focus + hover",
            "● Compare levels",
            "Page options",
            "pressed",
        ],
    );
    let mut done = BTreeSet::new();
    shoot_ready(&app, &mut done, &args);
    let body = Point { x: 70, y: 20 };
    for _ in 0..24 {
        if done.len() == SPECIMENS.len() {
            break;
        }
        app.wheel(body, true, 2);
        shoot_ready(&app, &mut done, &args);
    }
    assert_eq!(
        done.len(),
        SPECIMENS.len(),
        "specimens still clipped: {done:?}\n{}",
        text(&app.screen())
    );
    assert!(
        !text(&app.screen()).contains("focus + hover"),
        "focused-hover row appeared\n{}",
        text(&app.screen())
    );
    app.finish();
}

fn scroll_color_bands(app: &crate::support::App, saw256: &mut bool, saw16: &mut bool) {
    let body = Point { x: 70, y: 16 };
    for _ in 0..20 {
        let visible = text(&app.settle());
        *saw256 |= visible.contains("256 colors");
        *saw16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            return;
        }
        app.wheel(body, true, 3);
    }
    panic!(
        "no color band stayed below the fold\n{}",
        text(&app.screen())
    );
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-compare", 120, 40, &args);
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
    let mut saw256 = false;
    let mut saw16 = false;
    scroll_color_bands(&app, &mut saw256, &mut saw16);
    clear_flash(&app, 120, 40, "Compare levels");
    scroll_color_bands(&app, &mut saw256, &mut saw16);
    park(&app, 40);
    let bands = app.wait_text("no color");
    assert!(saw256, "256 colors band was not shown\n{}", text(&bands));
    assert!(saw16, "16 colors band was not shown\n{}", text(&bands));
    see(
        &bands,
        &[TITLE, "no color", WIDE, "Enter / Space Compare levels"],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.dialogkinds.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_screen("compare narrow", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && text(screen).contains("● Compare levels")
            && text(screen).contains("truecolor")
            && text(screen).contains("rest")
            && !text(screen).contains(WIDE)
    });
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor", "rest"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.dialogkinds.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40; the narrow frame shows the top truecolor band because the resize leaves the scrolled no-color band",
        &args,
    );
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest"],
        &["● Compare levels", "no color", "Page options"],
    );
    app.resize(120, 40);
    let wide = app.wait_text(WIDE);
    see(
        &wide,
        &[TITLE, CAP_STATES, "○ Compare levels", WIDE, BLURB],
        &["● Compare levels", "Page options", CAP_PLAY, POPUPS],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[]);
    app.finish();
}

fn readout_case(
    name: &str,
    flag: &str,
    present: &[&str],
    absent: &[&str],
    changed: &[(&str, &str, bool)],
    case_id: &str,
    precondition: &str,
) {
    let args = ["--page", "dialogkinds", "--opt", flag];
    let app = open(name, 120, 40, &args);
    park(&app, 40);
    let play = app.wait_text("Applied");
    see(&play, present, absent);
    shot(&app, case_id, precondition, &args);
    let options = option_page(&app);
    assert_defaults_except(&options, changed);
    app.finish();
}

#[test]
fn option_width_48_and_72() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-opt-width-auto", 120, 40, &args);
    let play = app.wait_text("width 62");
    see(
        &play,
        &["width 62", "width 66", "width 54"],
        &["width 48", "width 72"],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[]);
    app.finish();

    let narrow = ["--page", "dialogkinds", "--opt", "dialogkinds.width=48"];
    let app = open("s05-opt-width-48", 120, 40, &narrow);
    let play = app.wait_text("width 48");
    see(
        &play,
        &["width 48", "Save closes", "nav desktop"],
        &["width 62", "width 66", "width 54", "width 72"],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Width", "48", false)]);
    app.finish();

    readout_case(
        "s05-opt-width-72",
        "dialogkinds.width=72",
        &["width 72", "Save closes", TITLE],
        &["width 62", "width 66", "width 54", "width 48"],
        &[("Width", "72", false)],
        "opt.dialogkinds.width",
        "width 72 replaces the auto widths 62, 66, and 54; width 48 does the same and is not the captured frame",
    );
}

#[test]
fn option_nav_vim() {
    readout_case(
        "s05-opt-nav",
        "dialogkinds.nav=vim",
        &["nav vim", "× default", "button row wrap off"],
        &["nav desktop"],
        &[("Nav keys", "vim", false)],
        "opt.dialogkinds.nav",
        "nav vim replaces nav desktop in the applied readout and leaves the other readout values",
    );
}

#[test]
fn option_close_button_danger() {
    let args = [
        "--page",
        "dialogkinds",
        "--opt",
        "dialogkinds.close_button=danger",
    ];
    let app = open("s05-opt-close", 120, 40, &args);
    let play = app.wait_text("× danger");
    see(&play, &["× danger", TITLE], &["× default"]);
    click_page(&app, "Server info");
    clear_flash(&app, 120, 40, "Server info");
    let dialog = app.wait_text("✕");
    see(
        &dialog,
        &["Server info", "✕", "Open logs", "db.eu-west-1.internal"],
        &["Choice · Some"],
    );
    shot(
        &app,
        "opt.dialogkinds.close_button",
        "danger draws ✕ on Server info; the applied readout says × danger rather than × default",
        &args,
    );
    close_to_page(&app);
    let back = app.wait_text("× danger");
    see(&back, &["× danger", "Enter Open"], &["Open logs", "✕"]);
    let options = option_page(&app);
    assert_defaults_except(&options, &[("× button", "danger", false)]);
    app.finish();
}

#[test]
fn option_button_wrap_on() {
    readout_case(
        "s05-opt-wrap",
        "dialogkinds.button_wrap=on",
        &["button row wrap on", "width 62"],
        &["button row wrap off"],
        &[("Buttons wrap", "on", true)],
        "opt.dialogkinds.button_wrap",
        "button row wrap on replaces button row wrap off",
    );
}

#[test]
fn option_keep_open_empty_save_stays() {
    let args = ["--page", "dialogkinds", "--opt", "dialogkinds.keep_open=on"];
    let app = open("s05-opt-keep", 120, 40, &args);
    park(&app, 40);
    let play = app.wait_text("Save keeps it open");
    see(
        &play,
        &["Save keeps it open", "width 62"],
        &["Save closes", "Form · Save"],
    );
    shot(
        &app,
        "opt.dialogkinds.keep_open",
        "keep open says Save keeps it open in the readout, where the default says Save closes",
        &args,
    );
    click_page(&app, "Add connection");
    let form = app.wait_text("Test connection");
    see(&form, &["orders-prod", "Test connection"], &["q Quit"]);
    app.key("ctrl+a");
    app.key("ctrl+k");
    let cleared = app.wait_screen("name cleared", |screen| {
        !line_after(screen, "Name").contains("orders-prod")
    });
    assert!(
        !line_after(&cleared, "Name").contains("orders-prod"),
        "name stayed\n{}",
        text(&cleared)
    );
    see(
        &cleared,
        &["Test connection", "Add connection"],
        &["q Quit"],
    );
    click_page(&app, "Save");
    let stayed = app.wait_screen("save stayed", |screen| {
        text(screen).contains("Test connection") && !text(screen).contains("q Quit")
    });
    see(
        &stayed,
        &["Test connection", "Add connection", "Cancel"],
        &[
            "q Quit",
            "Form · saved, closed by the page",
            "Form · Save ·",
        ],
    );
    close_to_page(&app);
    let logged = app.wait_text("Action(\"save\")");
    see(
        &logged,
        &[
            "Form · Action(\"save\")",
            "Enter Open",
            "Save keeps it open",
        ],
        &[
            "Form · saved, closed by the page",
            "Form · Save ·",
            "Test connection",
        ],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Form stays open on Save", "on", true)]);
    app.finish();
}

#[test]
fn option_form_meta_off() {
    readout_case(
        "s05-opt-meta",
        "dialogkinds.form_meta=off",
        &["meta none", "meta read-only"],
        &["meta production"],
        &[("Form meta", "off", true)],
        "opt.dialogkinds.form_meta",
        "form meta off says meta none; info stays meta read-only",
    );
}

#[test]
fn option_info_error_on() {
    readout_case(
        "s05-opt-error",
        "dialogkinds.info_error=on",
        &["error title on", "width 66"],
        &["error title off"],
        &[("Info error title", "on", true)],
        "opt.dialogkinds.info_error",
        "info error title on replaces error title off in the applied readout",
    );
}

#[test]
fn option_info_keymap_ctrl() {
    let args = [
        "--page",
        "dialogkinds",
        "--opt",
        "dialogkinds.info_keymap=ctrl",
    ];
    let app = open("s05-opt-keys", 120, 40, &args);
    park(&app, 40);
    let play = app.wait_text("Ctrl+W Close");
    see(
        &play,
        &["Ctrl+W Close", "Ctrl+Y Copy"],
        &["q Close", "y Copy"],
    );
    shot(
        &app,
        "opt.dialogkinds.info_keymap",
        "info key map ctrl replaces q Close and y Copy with Ctrl+W Close and Ctrl+Y Copy",
        &args,
    );
    click_page(&app, "Server info");
    clear_flash(&app, 120, 40, "Ctrl+Y");
    let dialog = app.wait_text("Open logs");
    see(
        &dialog,
        &["Server info", "Ctrl+Y", "Open logs", "Esc Close"],
        &["q Close"],
    );
    app.key("escape");
    let closed = app.wait_text("Info · Closed");
    see(
        &closed,
        &["Info · Closed", "Ctrl+W Close", "Enter Open"],
        &["Open logs", "Info · Copy"],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Info key map", "ctrl", false)]);
    app.finish();
}

#[test]
fn option_choice_tones_off() {
    readout_case(
        "s05-opt-tones",
        "dialogkinds.choice_tones=off",
        &["tones off", "stepper on", "buttons Cancel/Next"],
        &["tones on"],
        &[("Choice tones", "off", true)],
        "opt.dialogkinds.choice_tones",
        "choice tones off replaces tones on and leaves stepper on and Cancel/Next",
    );
}

#[test]
fn option_choice_stepper_off() {
    let args = [
        "--page",
        "dialogkinds",
        "--opt",
        "dialogkinds.choice_stepper=off",
    ];
    let app = open("s05-opt-step", 120, 40, &args);
    let play = app.wait_text("stepper off");
    see(&play, &["stepper off", "tones on"], &["stepper on"]);
    click_page(&app, "Release channel");
    clear_flash(&app, 120, 40, "Stable");
    let dialog = app.wait_text("Nightly (may break)");
    see(
        &dialog,
        &[
            "Release channel",
            "Stable",
            "Nightly (may break)",
            "Cancel",
            "Next",
        ],
        &["Step 2 of 3", "Skip"],
    );
    shot(
        &app,
        "opt.dialogkinds.choice_stepper",
        "choice stepper off opens Release channel without Step 2 of 3; the readout says stepper off",
        &args,
    );
    app.key("escape");
    let closed = app.wait_text("Choice · None");
    see(
        &closed,
        &["Choice · None", "stepper off", "Enter Open"],
        &["Choice · Some", "Step 2 of 3"],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Choice stepper", "off", true)]);
    app.finish();
}

#[test]
fn option_choice_buttons_three() {
    let args = [
        "--page",
        "dialogkinds",
        "--opt",
        "dialogkinds.choice_buttons=three",
    ];
    let app = open("s05-opt-buttons", 120, 40, &args);
    let play = app.wait_text("Cancel/Skip/Switch");
    see(
        &play,
        &["buttons Cancel/Skip/Switch"],
        &["buttons Cancel/Next"],
    );
    click_page(&app, "Release channel");
    clear_flash(&app, 120, 40, "Skip");
    let dialog = app.wait_text("Switch");
    see(
        &dialog,
        &["Release channel", "Skip", "Switch", "Cancel", "Stable"],
        &["Cancel/Next"],
    );
    shot(
        &app,
        "opt.dialogkinds.choice_buttons",
        "choice buttons three draws Cancel, Skip, and Switch, and the readout says Cancel/Skip/Switch rather than Cancel/Next",
        &args,
    );
    app.key("escape");
    let closed = app.wait_text("Choice · None");
    see(
        &closed,
        &["Choice · None", "Cancel/Skip/Switch"],
        &["Choice · Some", "buttons Cancel/Next"],
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Choice buttons", "three", false)]);
    app.finish();
}

/// With Dismiss "never" Escape is refused in all three dialogs while their
/// explicit Cancel and Close buttons still resolve them.
fn assert_escape_refused_buttons_work(app: &crate::support::App) {
    click_page(app, "Release channel");
    clear_flash(app, 120, 40, "Switch");
    let dialog = app.wait_text("Switch");
    see(
        &dialog,
        &["Release channel", "Stable", "Cancel", "Switch"],
        &["Esc Cancel", "Choice · None"],
    );
    app.key("escape");
    let refused = app.settle();
    see(
        &refused,
        &["Release channel", "Stable", "Cancel", "Switch"],
        &["Choice · None", "Choice · Some"],
    );
    click_page(app, "Cancel");
    let closed = app.wait_text("Choice · None");
    see(&closed, &["Choice · None"], &["Switch"]);

    click_page(app, "Server info");
    let info = app.wait_text("Open logs");
    see(&info, &["Server info", "Close"], &["Esc Close"]);
    app.key("escape");
    let refused_info = app.settle();
    see(
        &refused_info,
        &["Server info", "Open logs"],
        &["Info · Closed"],
    );
    click_page(app, "Close");
    app.wait_text("Info · Closed");

    click_page(app, "Add connection");
    let form = app.wait_text("Test connection");
    see(&form, &["Add connection", "Cancel"], &["Form · Cancel"]);
    app.key("escape");
    let refused_form = app.settle();
    see(
        &refused_form,
        &["Add connection", "Test connection"],
        &["Form · Cancel"],
    );
    click_page(app, "Cancel");
    app.wait_text("Form · Cancel");
}

#[test]
fn option_dismiss_never_refuses_escape_keeps_buttons() {
    let args = [
        "--page",
        "dialogkinds",
        "--opt",
        "dialogkinds.dismiss=never",
    ];
    let app = open("s05-opt-dismiss", 120, 40, &args);
    app.wait_text("Nothing yet");
    assert_escape_refused_buttons_work(&app);
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Dismiss", "never", false)]);
    app.finish();
}

#[test]
fn options_tab_dismiss_never_refuses_escape() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-ui-dismiss", 120, 40, &args);
    let options = option_page(&app);
    assert_defaults_except(&options, &[]);
    let label = locate(&app, "Dismiss", |hit| hit.x >= 24 && hit.y > 4);
    app.click(Point {
        x: label.x + 2,
        y: label.y + 1,
    });
    app.wait_text("escape");
    click_where(&app, "never", |hit| hit.x >= 24 && hit.y > label.y + 1);
    let chosen = app.wait_screen("Dismiss never chosen", |screen| {
        let visible = text(screen);
        !visible.contains("escape") && visible.contains("never")
    });
    assert_defaults_except(&chosen, &[("Dismiss", "never", false)]);
    click_tab(&app, "Playground");
    app.wait_text("Nothing yet");
    assert_escape_refused_buttons_work(&app);
    app.finish();
}

#[test]
fn kinds_chooser_save_cancel_choice_info_help() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-kinds", 120, 40, &args);
    let play = app.wait_text("Nothing yet");
    see(
        &play,
        &["Save closes", "Nothing yet"],
        &["Form · Save", "Choose…"],
    );
    click_page(&app, "Add connection");
    let form = app.wait_text("Test connection");
    see(
        &form,
        &[
            "Add connection",
            "production",
            "orders-prod",
            "PostgreSQL",
            "db.eu-west-1.internal",
            "7731",
            "Verify full",
            "[ ] Connect through an SSH tunnel",
            "Test connection",
            "Cancel",
            "Save",
        ],
        &["Choose…", "q Quit", "[✓] Connect through an SSH tunnel"],
    );
    tabs(&app, 5);
    app.key("space");
    let tunnel = app.wait_text("[✓] Connect through an SSH tunnel");
    see(
        &tunnel,
        &["Choose…", "SSH key"],
        &["[ ] Connect through an SSH tunnel"],
    );
    app.key("tab");
    app.key("enter");
    let picker = app.wait_text("Enter Choose");
    see(
        &picker,
        &[
            "SSH key",
            "⚿ ~/.ssh/id_ed25519",
            "⚿ ~/.ssh/prod_rsa",
            "⚿ ~/keys/staging.pem",
            "orders-prod",
            "Enter Choose",
        ],
        &["orders-prodx", "xorders-prod"],
    );
    clear_flash(&app, 120, 40, "Enter Choose");
    let open_picker = app.wait_text("staging.pem");
    see(
        &open_picker,
        &["Enter Choose", "orders-prod", "⚿ ~/.ssh/prod_rsa"],
        &["orders-prodx"],
    );
    shot(
        &app,
        "sc.dialogkinds.kinds.before",
        "the SSH key chooser is open over Add connection and owns Enter Choose",
        &args,
    );
    app.key("x");
    let ignored = app.settle();
    see(
        &ignored,
        &[
            "orders-prod",
            "⚿ ~/.ssh/id_ed25519",
            "⚿ ~/.ssh/prod_rsa",
            "⚿ ~/keys/staging.pem",
        ],
        &["orders-prodx", "xorders-prod"],
    );
    app.key("down");
    app.key("enter");
    let chosen = app.wait_screen("key chosen", |screen| {
        text(screen).contains("~/.ssh/prod_rsa") && !text(screen).contains("Enter Choose")
    });
    see(
        &chosen,
        &[
            "~/.ssh/prod_rsa",
            "orders-prod",
            "Choose…",
            "Test connection",
        ],
        &["Enter Choose", "orders-prodx", "q Quit"],
    );
    clear_flash(&app, 120, 40, "prod_rsa");
    let after = app.wait_text("~/.ssh/prod_rsa");
    see(
        &after,
        &["~/.ssh/prod_rsa", "orders-prod", "Add connection"],
        &["Enter Choose", "~/.ssh/id_ed25519"],
    );
    shot(
        &app,
        "sc.dialogkinds.kinds.after",
        "choosing the second key shows ~/.ssh/prod_rsa on the still-open form and leaves the name orders-prod",
        &args,
    );
    click_page(&app, "Cancel");
    let cancelled = app.wait_text("Form · Cancel");
    see(
        &cancelled,
        &["Form · Cancel", "Enter Open", "Picker · Chosen(1)"],
        &["Form · Save", "Test connection", "Chosen(0)", "Chosen(2)"],
    );
    click_page(&app, "Add connection");
    let reopened = app.wait_text("7731");
    see(
        &reopened,
        &[
            "orders-prod",
            "db.eu-west-1.internal",
            "7731",
            "[ ] Connect through an SSH tunnel",
        ],
        &["prod_rsa", "[✓] Connect through an SSH tunnel", "Choose…"],
    );
    tabs(&app, 2);
    app.key("ctrl+a");
    app.key("ctrl+k");
    let host = app.wait_screen("host cleared", |screen| {
        !line_after(screen, "Host").contains("db.eu-west-1")
    });
    assert!(
        !line_after(&host, "Host").contains("internal"),
        "host stayed\n{}",
        text(&host)
    );
    click_page(&app, "Test connection");
    let failed = app.wait_text("Host is required");
    see(
        &failed,
        &["✗ Host is required", "Test connection"],
        &["✓ Connected in 42 ms", "q Quit"],
    );
    click_where(&app, "Host", |hit| {
        hit.x >= 24 && hit.y > 4 && row_text(&app.screen(), hit.y.into()).contains("optional")
    });
    app.key("h");
    click_page(&app, "Test connection");
    let ok = app.wait_text("Connected in 42 ms");
    see(
        &ok,
        &["✓ Connected in 42 ms", "Test connection"],
        &["✗ Host is required"],
    );
    click_page(&app, "Cancel");
    let host_cancel = app.wait_text("Enter Open");
    see(
        &host_cancel,
        &["Form · Cancel", "Enter Open"],
        &["Form · Save ·", "Test connection"],
    );
    click_page(&app, "Add connection");
    let fresh = app.wait_text("7731");
    see(
        &fresh,
        &["orders-prod", "db.eu-west-1.internal"],
        &["Connected in 42 ms", "Host is required"],
    );
    click_page(&app, "Save");
    let saved = app.wait_text("Form · Save · orders-prod");
    see(
        &saved,
        &[
            "Form · Save · orders-prod",
            "Saved connection orders-prod",
            "Enter Open",
            "Add connection",
        ],
        &["Test connection", "Form · saved, closed by the page"],
    );
    click_page(&app, "Release channel");
    let choice = app.wait_text("Step 2 of 3");
    see(
        &choice,
        &[
            "Release channel",
            "Step 2 of 3",
            "Stable",
            "Nightly (may break)",
            "Cancel",
            "Next",
        ],
        &["Choice · Some", "Skip"],
    );
    app.key("down");
    app.key("escape");
    let choice_cancel = app.wait_text("Choice · None");
    see(
        &choice_cancel,
        &["Choice · None", "Enter Open"],
        &["Choice · Some", "Step 2 of 3"],
    );
    click_page(&app, "Release channel");
    let again = app.wait_text("(●) Stable");
    see(&again, &["(●) Stable", "Step 2 of 3"], &["(●) Nightly"]);
    app.key("down");
    app.key("down");
    app.key("enter");
    let picked = app.wait_text("Choice · Some(2)");
    see(
        &picked,
        &["Choice · Some(2)", "Enter Open"],
        &["Choice · Some(0)", "Choice · Some(1)", "Step 2 of 3"],
    );
    click_page(&app, "Server info");
    let info = app.wait_text("Open logs");
    see(
        &info,
        &[
            "Server info",
            "db.eu-west-1.internal",
            "Open logs",
            "read-only",
        ],
        &["Info · Closed", "q Quit"],
    );
    app.key("y");
    let copied = app.wait_text("Copied db.eu-west-1.internal");
    see(
        &copied,
        &["Copied db.eu-west-1.internal", "Open logs", "Server info"],
        &["Info · Closed", "q Quit"],
    );
    app.key("q");
    let info_closed = app.wait_text("Info · Closed");
    see(
        &info_closed,
        &[
            "Info · Closed",
            "Info · Copy(\"db.eu-west-1.internal\")",
            "Enter Open",
        ],
        &["Open logs"],
    );
    click_page(&app, "Keyboard help");
    let help = app.wait_text("Next field");
    see(
        &help,
        &["Next field", "Copy", "Forms"],
        &["Help · closed", "q Quit"],
    );
    app.key("escape");
    let help_closed = app.wait_text("Help · closed");
    see(
        &help_closed,
        &[
            "Help · closed",
            "Enter Open",
            "Choice · Some(2)",
            "Form · Save · orders-prod",
        ],
        &[
            "Next field",
            "Test connection",
            "Open logs",
            "Choice · Some(0)",
            "Choice · Some(1)",
        ],
    );
    app.finish();
}

#[test]
fn test_connection_replaces_the_masked_password_and_keeps_it_masked() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-keychain", 120, 40, &args);
    click_page(&app, "Add connection");
    let form = app.wait_text("Test connection");
    see(&form, &["7731"], &["keychain-rotated", "hunter2"]);

    click_where(&app, "Test connection", |_| true);
    let filled = app.wait_text("Password refreshed from the keychain");
    see(
        &filled,
        &["still masked", "0042", "Connected in 42 ms"],
        &["7731", "keychain-rotated", "hunter2", "revealed"],
    );
    close_to_page(&app);
    app.finish();
}

/// Opens the form, types into Host and hides it mid-edit, then moves focus to
/// Driver so no other input is editing. The hide ends the edit session on a
/// frame that may land after the field disappears, so each frame is judged
/// only once the terminal is quiet.
fn hide_host_mid_edit(name: &str) -> crate::support::App {
    let args = ["--page", "dialogkinds"];
    let app = open(name, 120, 40, &args);
    click_page(&app, "Add connection");
    app.wait_text("Test connection");

    app.key("tab");
    app.key("tab");
    app.type_text("edited-host");
    app.wait_text("edited-host");

    app.key("ctrl+t");
    app.wait_absent("Host");
    let hidden = app.settle();
    see(
        &hidden,
        &["Driver", "Password", "Tab Next field"],
        &["Host", "▎ edited-host"],
    );

    app.key("tab");
    let idle = app.settle();
    see(
        &idle,
        &["Driver", "Tab Next field", "Enter Open options"],
        &["Host", "EDIT", "Enter Apply"],
    );
    app
}

/// Reveals Host again. A restored edit session shows in the hint bar as
/// EDIT, so the revealed frame must carry the idle form hints instead.
fn reveal_host(app: &crate::support::App) {
    app.key("ctrl+t");
    app.wait_text("Host");
    let shown = app.settle();
    see(
        &shown,
        &[
            "Host",
            "edited-host",
            "Tab Next field",
            "Enter Open options",
        ],
        &["EDIT", "Enter Apply"],
    );
}

#[test]
fn form_toggle_hides_an_editing_input_and_reveal_restores_no_session() {
    let app = hide_host_mid_edit("s05-toggle");
    reveal_host(&app);

    // Focus is on Driver, so one Tab reaches Host. A restored session would
    // trap Tab and keep appending to the old text.
    app.key("tab");
    app.type_text("zz");
    let after = app.settle();
    see(&after, &["▎ zz"], &["edited-hostzz"]);
    close_to_page(&app);
    app.finish();
}

#[test]
fn form_toggle_hidden_editing_input_does_not_hold_escape() {
    let app = hide_host_mid_edit("s05-toggle-esc");
    reveal_host(&app);

    click_where(&app, "edited-host", |hit| hit.x < 40);
    let focused = app.settle();
    see(&focused, &["▎ edited-host"], &[]);

    // One Escape closes the form. A restored session would take it to roll
    // the host back to its value from before the hide, and keep the form.
    app.key("escape");
    let closed = app.settle();
    see(
        &closed,
        &["Enter Open", "Form · Cancel"],
        &["Test connection", "EDIT", "db.eu-west-1.internal"],
    );
    app.finish();
}

#[test]
fn form_toggle_hides_an_open_driver_popup_and_keeps_its_choice() {
    let args = ["--page", "dialogkinds"];
    let app = open("s05-popup-toggle", 120, 40, &args);
    click_page(&app, "Add connection");
    app.wait_text("Test connection");

    // Commit a new choice first, so a choice lost or reset by the hide shows
    // on the Driver value row after the reveal.
    app.key("tab");
    app.key("enter");
    let popup = app.wait_text("SQLite");
    see(&popup, &["MySQL", "PostgreSQL"], &[]);
    app.key("down");
    app.key("enter");
    app.wait_absent("SQLite");
    let chosen = app.settle();
    let value = line_after(&chosen, "Driver");
    assert!(
        value.contains("MySQL") && value.contains('▾'),
        "MySQL is chosen: {value:?}\n{}",
        text(&chosen)
    );

    app.key("enter");
    let popup = app.wait_text("SQLite");
    see(&popup, &["MySQL", "PostgreSQL", "SQLite"], &[]);

    app.key("ctrl+d");
    app.wait_absent("SQLite");
    let hidden = app.settle();
    see(&hidden, &["Host"], &["SQLite", "Driver", "PostgreSQL"]);

    app.key("ctrl+d");
    // Driver is absent while the field is hidden, so waiting for it syncs on
    // the reveal. A popup that survived the hide paints on that frame or just
    // after it, so settle before looking.
    app.wait_text("Driver");
    let shown = app.settle();
    // The chevron is read on the Driver value row. The nav row "▾ Overlays"
    // always shows one, so a whole-screen check would pass on any frame.
    let value = line_after(&shown, "Driver");
    assert!(
        value.contains("MySQL") && value.contains('▾') && !value.contains('▴'),
        "Driver keeps MySQL and shows the closed chevron: {value:?}\n{}",
        text(&shown)
    );
    see(&shown, &["Driver"], &["SQLite", "PostgreSQL"]);
    close_to_page(&app);
    app.finish();
}
