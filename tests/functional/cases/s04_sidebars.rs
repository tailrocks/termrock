//! Sidebars route, sections, specimens, compare levels, collapse, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s04_sidebars/` and each case id's dots become underscores.

use std::collections::BTreeSet;
use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Sidebars";
const BLURB: &str =
    "Sections, current item, focus cursor, hover, collapsed mode; text first, no icons";
const OVERVIEW: &str = "Tokens and principles behind every component";
const TABS_BLURB: &str =
    "Tab items (icons, dirty, busy, error, closable), rules, overflow and nested strips";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const INTERIM: &str = "121×40";
const DOTS: &str = "····";
const LAST_NONE: &str = "Last: none yet";
const LAST_HIDE: &str = "Last: RailAction::Collapse";
const LAST_SHOW: &str = "Last: RailAction::Expand";

const SPECIMENS: &[(&str, &str)] = &[
    ("sc.sidebars.specimen.rest", "rest"),
    ("sc.sidebars.specimen.hover", "hover"),
    ("sc.sidebars.specimen.pressed", "pressed"),
    ("sc.sidebars.specimen.focused", "focused"),
    ("sc.sidebars.specimen.focused_hover", "focus + hover"),
    ("sc.sidebars.specimen.selected", "selected"),
    ("sc.sidebars.specimen.disabled", "disabled"),
    ("sc.sidebars.specimen.overflow", "overflow"),
    ("sc.sidebars.specimen.narrow", "narrow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s04_sidebars/{}", case_id.replace('.', "_"))
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

fn row_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    lines_of(screen).iter().any(|line| line.contains(needle))
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

fn click_lowest(app: &crate::support::App, label: &str) -> Point {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x > 24)
        .collect();
    assert!(
        !hits.is_empty(),
        "no {label:?} in the page\n{}",
        text(&screen)
    );
    hits.sort_by_key(|hit| hit.y);
    let hit = hits[hits.len() - 1];
    app.click(hit);
    hit
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

/// The live collapse control is one row. A frame that still has both glyphs
/// is mid-paint: the list can show `T Tasks` while the button is still `›`.
fn collapse_glyph(screen: &TerminalSnapshot) -> Option<&'static str> {
    let mut label = None;
    for line in lines_of(screen) {
        let trimmed = line.trim();
        let next = if trimmed == "Collapse" || trimmed == "▎Collapse" {
            "Collapse"
        } else if trimmed == "›" || trimmed == "▎›" {
            "›"
        } else {
            continue;
        };
        if label.is_some() {
            return None;
        }
        label = Some(next);
    }
    label
}

fn button_label(screen: &TerminalSnapshot) -> String {
    collapse_glyph(screen)
        .unwrap_or_else(|| panic!("collapse button missing\n{}", text(screen)))
        .to_string()
}

fn card_title_line(screen: &TerminalSnapshot) -> String {
    lines_of(screen)
        .into_iter()
        .find(|line| line.contains("Tables") && line.contains("Tasks"))
        .unwrap_or_else(|| panic!("card title missing\n{}", text(screen)))
}

fn card_tasks_x(screen: &TerminalSnapshot) -> u16 {
    let hits: Vec<_> = find_all(screen, "Tasks")
        .into_iter()
        .filter(|hit| hit.x > 40)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "card title Tasks hits {hits:?}\n{}",
        text(screen)
    );
    hits[0].x
}

fn vertical_rail(screen: &TerminalSnapshot) -> String {
    let lines = lines_of(screen);
    let start = lines
        .iter()
        .position(|line| line.contains("One focus stop"))
        .unwrap_or_else(|| panic!("hint missing\n{}", text(screen)));
    let idx = lines[start].find("One focus stop").expect("hint index");
    let prefix: Vec<char> = lines[start][..idx].chars().collect();
    let mut col = prefix.len();
    while col > 0 && prefix[col - 1] == ' ' {
        col -= 1;
    }
    assert!(col > 0, "no glyph before the hint\n{}", text(screen));
    col -= 1;
    let mut word = String::new();
    for line in lines.iter().skip(start) {
        let row: Vec<char> = line.chars().collect();
        if col >= row.len() {
            break;
        }
        let ch = row[col];
        if ch.is_ascii_alphabetic() {
            word.push(ch);
        } else if !word.is_empty() {
            break;
        }
    }
    word
}

fn header_is(line: &str, label: &str) -> bool {
    if !line.contains("Workspace") {
        return false;
    }
    if label == "focused" {
        line.contains("focused") && !line.contains("focus + hover")
    } else {
        line.contains(label)
    }
}

fn specimen_header(line: &str) -> bool {
    SPECIMENS.iter().any(|(_, label)| header_is(line, label))
}

fn specimen_block(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let lines = lines_of(screen);
    let index = lines.iter().position(|line| header_is(line, label))?;
    let mut end = index + 1;
    while end < lines.len() && end - index < 8 {
        let line = &lines[end];
        if specimen_header(line)
            || line.contains("←→ Section")
            || line.contains("Enter / Space")
            || line.contains("↑↓ Move")
        {
            break;
        }
        end += 1;
    }
    let block = lines[index..end].join("\n");
    let runs = block.contains("R Runs");
    let ready = match label {
        "overflow" => {
            block.contains("R Runs and a …")
                && block.contains(" 12 │")
                && !block.contains("far longer than its row")
                && block.contains("Branches")
        }
        "narrow" => block.contains("Tasks") && block.contains("Branches"),
        "focused" | "focus + hover" => block.contains("Tasks") && runs && block.contains('▏'),
        "selected" => {
            block.contains("T Tasks")
                && block
                    .lines()
                    .any(|line| line.contains("R Runs") && line.contains('›'))
                && block
                    .lines()
                    .any(|line| line.contains("T Tasks") && !line.contains('›'))
        }
        _ => block.contains("Workspace") && block.contains("Tasks") && block.contains("Branches"),
    };
    ready.then_some(block)
}

/// Paused motion does not redraw when a 140ms press flash expires, so a shot
/// of the click frame can keep the pressed colour. Resize away and back.
fn clear_flash(app: &crate::support::App, cols: u16, rows: u16, token: &str) {
    std::thread::sleep(Duration::from_millis(200));
    app.resize(121, 40);
    app.wait_text(INTERIM);
    app.resize(cols, rows);
    app.wait_screen(&format!("settled {cols}x{rows}"), |screen| {
        if screen.cols() != u32::from(cols)
            || screen.rows() != u32::from(rows)
            || !text(screen).contains(token)
        {
            return false;
        }
        let visible = text(screen);
        if cols < 100 {
            !visible.contains(INTERIM) && !visible.contains(WIDE)
        } else {
            visible.contains(&format!("{cols}×{rows}"))
        }
    });
    app.move_to(Point {
        x: 2,
        y: rows.saturating_sub(1),
    });
}

fn row_has_word(row: &str, word: &str) -> bool {
    row.split_whitespace().any(|token| token == word)
}

/// `Page options` paints before the column body. The first rows can sit
/// above the previous tab's help text, so Badges and Rail label are absent.
/// Each option row also holds the nav column, so the value is not the first token.
fn option_page(app: &crate::support::App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_screen("sidebar options", |screen| {
        let visible = text(screen);
        if !visible.contains("Page options") || !visible.contains("Rail when hidden") {
            return false;
        }
        let badges_ok = lines_of(screen).iter().any(|line| {
            line.contains("Badges")
                && (line.trim_end().ends_with("on") || line.trim_end().ends_with("off"))
        });
        let open_ok = value_line(screen, "Open width").is_some_and(|row| {
            !row.contains('×')
                && ["20", "24", "30"]
                    .iter()
                    .any(|width| row_has_word(&row, width))
        });
        let rail_ok = value_line(screen, "Rail label").is_some_and(|row| {
            !row.contains('↑')
                && (row_has_word(&row, "Navigation") || row_has_word(&row, "Explorer"))
        });
        badges_ok && open_ok && rail_ok
    })
}

fn value_line(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let lines = lines_of(screen);
    let index = lines.iter().position(|line| line.contains(label))?;
    lines.get(index + 1).cloned()
}

/// The first Options frame can place Rail label directly above the keyboard
/// row. The value row is the following paint.
fn option_values(app: &crate::support::App, width: &str) -> TerminalSnapshot {
    option_page(app);
    app.wait_screen("sidebar option values", |screen| {
        let open = value_line(screen, "Open width");
        let rail = value_line(screen, "Rail label");
        open.is_some_and(|row| row.contains(width) && !row.contains("Keyboard"))
            && rail.is_some_and(|row| row.contains("Navigation") && !row.contains("Keyboard"))
    })
}

fn toggle_row(screen: &TerminalSnapshot, label: &str) -> String {
    row_with(screen, label)
}

fn assert_option(screen: &TerminalSnapshot, label: &str, value: &str, toggle: bool) {
    let row = if toggle {
        toggle_row(screen, label)
    } else {
        line_after(screen, label)
    };
    let shown = if toggle {
        row.trim_end().ends_with(value)
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
            "24",
            "20",
            "30",
            "6",
            "5",
            "8",
            "vim",
            "desktop",
            "Navigation",
            "Explorer",
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

fn shoot_ready(app: &crate::support::App, done: &mut BTreeSet<&'static str>, args: &[&str]) {
    let screen = app.settle();
    for (case_id, label) in SPECIMENS {
        if done.contains(case_id) {
            continue;
        }
        let Some(block) = specimen_block(&screen, label) else {
            continue;
        };
        assert!(
            block.contains(label) || (*label == "focus + hover" && block.contains("focus + hover")),
            "{label} block lost its name\n{block}"
        );
        let visible = text(&screen);
        assert!(
            visible.contains("○ Compare levels") || visible.contains("←→ Section"),
            "compare levels is not off\n{visible}"
        );
        see(
            &screen,
            &[label, CAP_STATES],
            &[
                "● Compare levels",
                "Page options",
                "busy",
                "loading",
                "Enter / Space Compare levels",
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

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "sidebars"];
    let app = open("s04-route-page", 120, 40, &args);
    let screen = app.wait_text(TITLE);
    see(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "T Tasks",
            "3",
            "Collapse",
            LAST_NONE,
            "Workspace",
        ],
        &[
            OVERVIEW,
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            TABS_BLURB,
            DOTS,
            LAST_HIDE,
        ],
    );
    assert_eq!(button_label(&screen), "Collapse");
    shot(
        &app,
        "sc.sidebars.route",
        "--page sidebars opens the playground with Tasks current and the Collapse button",
        &args,
    );
    app.finish();

    let app = open("s04-route-keys", 120, 40, &[]);
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
        &[TITLE, BLURB, "T Tasks", CAP_PLAY],
        &[OVERVIEW, "Page options", TABS_BLURB],
    );
    app.key("]");
    let next = app.wait_text(TABS_BLURB);
    see(&next, &[TABS_BLURB], &[BLURB, LAST_NONE]);
    app.key("[");
    let back = app.wait_text(BLURB);
    see(&back, &[TITLE, BLURB, "T Tasks"], &[TABS_BLURB, OVERVIEW]);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[BLURB, "T Tasks", LAST_NONE]);
    let sidebar = text(&overview);
    if !find_all(&app.screen(), "Sidebars")
        .into_iter()
        .any(|hit| hit.x < 24)
    {
        let _ = sidebar;
        click_side(&app, "Navigation");
        app.wait_text("Sidebars");
    }
    click_side(&app, "Sidebars");
    let pointed = app.wait_text(BLURB);
    see(
        &pointed,
        &[TITLE, BLURB, "T Tasks", CAP_PLAY],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "sidebars"];
    let app = open("s04-sections", 80, 24, &args);
    let narrow_play = app.wait_text(TITLE);
    see(
        &narrow_play,
        &[TITLE, "Playground", "T Tasks", "3", "Billing", "Keyboard"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "text first, no icons",
            TABS_BLURB,
        ],
    );
    assert_eq!(narrow_play.cols(), 80);
    assert_eq!(narrow_play.rows(), 24);
    shot(
        &app,
        "sc.sidebars.playground.80x24",
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
            "Collapsed (icons)",
            "Open width",
            "Collapsed width",
            "Keys",
            "Badges",
            "Hidden (rail)",
            "Rail when hidden",
            "Rail label",
            "Navigation",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            WIDE,
            "truecolor",
            "T Tasks",
            "the live widget",
        ],
    );
    assert_option(&narrow_opt, "Collapsed (icons)", "off", true);
    assert_option(&narrow_opt, "Open width", "24", false);
    assert_option(&narrow_opt, "Collapsed width", "6", false);
    assert_option(&narrow_opt, "Keys", "vim", false);
    assert_option(&narrow_opt, "Badges", "on", true);
    assert_option(&narrow_opt, "Hidden (rail)", "off", true);
    assert_option(&narrow_opt, "Rail when hidden", "on", true);
    assert_option(&narrow_opt, "Rail label", "Navigation", false);
    shot(
        &app,
        "sc.sidebars.options.80x24",
        "options at 80 by 24 before the terminal grows, with every control and its default value",
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
            "hover",
            "pressed",
            "Open",
            "T Tasks",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            WIDE,
            "truecolor",
            "focus + hover",
            "overflow",
            "narrow",
            "● Compare levels",
            "busy",
            "loading",
        ],
    );
    assert_eq!(narrow_states.cols(), 80);
    assert_eq!(narrow_states.rows(), 24);
    shot(
        &app,
        "sc.sidebars.states.80x24",
        "states at 80 by 24 before the terminal grows, showing rest and hover and clipping the later specimens",
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
            "pressed",
            "Open",
            "Collapsed",
            "Rail",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "busy",
            "error",
            "editing",
            "empty",
            "loading",
        ],
    );
    shot(
        &app,
        "sc.sidebars.states.120x40",
        "states caption and the leading specimens at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.sidebars.states",
        "states is the visible Sidebars section",
        &args,
    );

    let mut done = BTreeSet::new();
    shoot_ready(&app, &mut done, &args);
    let body = Point { x: 70, y: 18 };
    for _ in 0..16 {
        if done.contains("sc.sidebars.specimen.overflow")
            && done.contains("sc.sidebars.specimen.narrow")
        {
            break;
        }
        app.wheel(body, true, 2);
        app.move_to(Point { x: 2, y: 39 });
        shoot_ready(&app, &mut done, &args);
    }
    assert_eq!(
        done.len(),
        SPECIMENS.len(),
        "specimens still clipped: {done:?}\n{}",
        text(&app.screen())
    );

    click_tab(&app, "Options");
    clear_flash(&app, 120, 40, "Page options");
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[TITLE, CAP_OPT, WIDE, "Page options", "vim", "Navigation"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "T Tasks",
            "desktop",
            "Explorer",
        ],
    );
    assert_option(&wide_opt, "Collapsed (icons)", "off", true);
    assert_option(&wide_opt, "Open width", "24", false);
    assert_option(&wide_opt, "Collapsed width", "6", false);
    assert_option(&wide_opt, "Keys", "vim", false);
    assert_option(&wide_opt, "Badges", "on", true);
    assert_option(&wide_opt, "Hidden (rail)", "off", true);
    assert_option(&wide_opt, "Rail when hidden", "on", true);
    assert_option(&wide_opt, "Rail label", "Navigation", false);
    shot(
        &app,
        "sc.sidebars.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.sidebars.options",
        "options is the visible Sidebars section",
        &args,
    );

    click_tab(&app, "Playground");
    clear_flash(&app, 120, 40, CAP_PLAY);
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[
            TITLE, BLURB, CAP_PLAY, WIDE, "T Tasks", "3", "Collapse", LAST_NONE, "Runs",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            DOTS,
            LAST_HIDE,
            TABS_BLURB,
        ],
    );
    assert_eq!(button_label(&wide_play), "Collapse");
    shot(
        &app,
        "sc.sidebars.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.sidebars.playground",
        "playground is the visible Sidebars section",
        &args,
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
    let args = ["--page", "sidebars"];
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
    let mut saw256 = false;
    let mut saw16 = false;
    scroll_color_bands(&app, &mut saw256, &mut saw16);
    clear_flash(&app, 120, 40, "Compare levels");
    scroll_color_bands(&app, &mut saw256, &mut saw16);
    app.move_to(Point { x: 2, y: 39 });
    let bands = app.wait_text("no color");
    assert!(saw256, "256 colors band was not shown\n{}", text(&bands));
    assert!(saw16, "16 colors band was not shown\n{}", text(&bands));
    see(
        &bands,
        &[TITLE, "no color", WIDE, "Enter / Space Compare levels"],
        &["Page options", CAP_PLAY, "○ Compare levels", "←→ Section"],
    );
    shot(
        &app,
        "sc.sidebars.compare-levels.120x40",
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
        "sc.sidebars.compare-levels.80x24",
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
    // The size chip is in the header before the States body paints.
    let wide = app.wait_screen("compare wide again", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && text(screen).contains("○ Compare levels")
            && text(screen).contains("rest")
            && text(screen).contains(WIDE)
            && !text(screen).contains("● Compare levels")
            && !text(screen).contains("no color")
            && !text(screen).contains("Page options")
            && !text(screen).contains(CAP_PLAY)
    });
    see(
        &wide,
        &[TITLE, CAP_STATES, "○ Compare levels", WIDE],
        &["● Compare levels", "Page options", CAP_PLAY],
    );
    app.finish();
}

#[test]
fn collapse_keeps_tasks_then_ctrl_b_hides_the_list() {
    let args = ["--page", "sidebars"];
    let app = open("s04-collapse", 120, 40, &args);
    let open_nav = app.wait_text(LAST_NONE);
    see(
        &open_nav,
        &[
            TITLE, "T Tasks", "3", "Runs", "Branches", LAST_NONE, CAP_PLAY,
        ],
        &[DOTS, LAST_HIDE, LAST_SHOW, "NavAction::Activated"],
    );
    assert_eq!(button_label(&open_nav), "Collapse");
    assert!(card_title_line(&open_nav).contains("Tasks"));
    shot(
        &app,
        "sc.sidebars.sidebar.before",
        "open sidebar shows Tasks, badge 3, and the Collapse button before collapse",
        &args,
    );

    click_lowest(&app, "Collapse");
    clear_flash(&app, 120, 40, DOTS);
    let collapsed = app.wait_screen("collapsed initials", |screen| {
        lines_of(screen)
            .iter()
            .any(|line| line.trim() == "›" || line.trim() == "▎›")
            && text(screen).contains(DOTS)
            && text(screen).contains(LAST_NONE)
    });
    see(
        &collapsed,
        &[TITLE, DOTS, LAST_NONE, CAP_PLAY, "Tasks"],
        &[
            "T Tasks",
            "Runs",
            "Branches",
            "Members",
            "Keyboard",
            LAST_HIDE,
            "NavAction::Activated",
        ],
    );
    assert!(card_title_line(&collapsed).contains("Tasks"));
    assert_eq!(button_label(&collapsed), "›");
    shot(
        &app,
        "sc.sidebars.sidebar.after",
        "collapsed sidebar shows initials and the expand glyph while Tasks stays current, before expand",
        &args,
    );

    click_lowest(&app, "›");
    let expanded = app.wait_screen("sidebar expanded", |screen| {
        collapse_glyph(screen) == Some("Collapse")
            && text(screen).contains("T Tasks")
            && text(screen).contains("Runs")
            && !text(screen).contains(DOTS)
    });
    see(
        &expanded,
        &["T Tasks", "3", "Runs", LAST_NONE, "Collapse"],
        &[DOTS, LAST_HIDE, LAST_SHOW, "NavAction::Activated"],
    );
    assert_eq!(button_label(&expanded), "Collapse");
    assert!(card_title_line(&expanded).contains("Tasks"));

    app.key("ctrl+b");
    let hidden = app.wait_text(LAST_HIDE);
    see(
        &hidden,
        &[TITLE, LAST_HIDE, "Tasks", CAP_PLAY],
        &[
            "T Tasks",
            "Runs",
            "Branches",
            DOTS,
            LAST_SHOW,
            "NavAction::Activated",
        ],
    );
    assert_eq!(vertical_rail(&hidden), "Navigation");
    assert!(card_title_line(&hidden).contains("Tasks"));

    app.key("ctrl+b");
    let shown = app.wait_text(LAST_SHOW);
    see(
        &shown,
        &["T Tasks", "3", "Runs", LAST_SHOW, "Collapse"],
        &[LAST_HIDE, DOTS, "NavAction::Activated"],
    );
    assert!(card_title_line(&shown).contains("Tasks"));
    assert!(!card_title_line(&shown).contains("Billing"));

    click_where(&app, "Billing", |hit| hit.x > 24 && hit.y > 4);
    let ignored = app.settle();
    see(
        &ignored,
        &["T Tasks", LAST_SHOW, "Billing"],
        &["NavAction::Activated", LAST_HIDE],
    );
    assert!(card_title_line(&ignored).contains("Tasks"));
    assert!(!card_title_line(&ignored).contains("Billing"));
    app.finish();
}

fn assert_defaults_except(screen: &TerminalSnapshot, changed: &[(&str, &str, bool)]) {
    let defaults = [
        ("Collapsed (icons)", "off", true),
        ("Open width", "24", false),
        ("Collapsed width", "6", false),
        ("Keys", "vim", false),
        ("Badges", "on", true),
        ("Hidden (rail)", "off", true),
        ("Rail when hidden", "on", true),
        ("Rail label", "Navigation", false),
    ];
    for (label, value, toggle) in defaults {
        if let Some((_, next, _)) = changed.iter().find(|(name, _, _)| *name == label) {
            assert_option(screen, label, next, toggle);
        } else {
            assert_option(screen, label, value, toggle);
        }
    }
}

#[test]
fn option_collapsed_on_shows_initials() {
    let args = ["--page", "sidebars", "--opt", "sidebars.collapsed=on"];
    let app = open("s04-opt-collapsed", 120, 40, &args);
    let play = app.wait_text(DOTS);
    see(
        &play,
        &[TITLE, DOTS, LAST_NONE, CAP_PLAY, "Tasks"],
        &["T Tasks", "Runs", "Branches", "3", LAST_HIDE],
    );
    assert_eq!(button_label(&play), "›");
    assert!(!card_title_line(&play).contains('‹'));
    shot(
        &app,
        "opt.sidebars.collapsed",
        "collapsed on draws initials and the expand glyph, and the card title stays Tasks",
        &args,
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Collapsed (icons)", "on", true)]);
    app.finish();
}

#[test]
fn option_open_width_shifts_the_card() {
    let default_args = ["--page", "sidebars"];
    let app = open("s04-opt-width-24", 120, 40, &default_args);
    let play = app.wait_text("T Tasks");
    assert_eq!(
        card_tasks_x(&play),
        58,
        "default card title\n{}",
        text(&play)
    );
    let options = option_values(&app, "24");
    assert_defaults_except(&options, &[]);
    app.finish();

    let narrow_args = ["--page", "sidebars", "--opt", "sidebars.width=20"];
    let app = open("s04-opt-width-20", 120, 40, &narrow_args);
    let play = app.wait_text("T Tasks");
    assert_eq!(
        card_tasks_x(&play),
        54,
        "width 20 card title\n{}",
        text(&play)
    );
    assert!(row_with(&play, "T Tasks").contains('3'));
    let options = option_values(&app, "20");
    assert_defaults_except(&options, &[("Open width", "20", false)]);
    app.finish();

    let wide_args = ["--page", "sidebars", "--opt", "sidebars.width=30"];
    let app = open("s04-opt-width-30", 120, 40, &wide_args);
    let play = app.wait_text("T Tasks");
    assert_eq!(
        card_tasks_x(&play),
        64,
        "width 30 card title\n{}",
        text(&play)
    );
    see(
        &play,
        &[TITLE, "T Tasks", "3", "Runs", LAST_NONE],
        &[DOTS, LAST_HIDE],
    );
    shot(
        &app,
        "opt.sidebars.width",
        "open width 30 shifts the Tasks card title to column 64; 24 stays at 58 and 20 at 54",
        &wide_args,
    );
    let options = option_values(&app, "30");
    assert_defaults_except(&options, &[("Open width", "30", false)]);
    app.finish();
}

#[test]
fn option_collapsed_width_draws_the_frame_only_at_eight() {
    collapsed_width_case("s04-cw6", "6", false, false);
    collapsed_width_case("s04-cw5", "5", false, false);
    collapsed_width_case("s04-cw8", "8", true, true);
}

fn collapsed_width_case(name: &str, width: &str, frame: bool, capture: bool) {
    let opt = format!("sidebars.collapsed_width={width}");
    let args = [
        "--page",
        "sidebars",
        "--opt",
        "sidebars.collapsed=on",
        "--opt",
        opt.as_str(),
    ];
    let app = open(name, 120, 40, &args);
    let play = app.wait_text(DOTS);
    see(
        &play,
        &[TITLE, DOTS, LAST_NONE, "Tasks"],
        &["T Tasks", "Runs", "Branches"],
    );
    assert_eq!(button_label(&play), "›");
    assert_eq!(
        card_title_line(&play).contains('‹'),
        frame,
        "collapsed width {width} frame\n{}",
        card_title_line(&play)
    );
    if capture {
        shot(
            &app,
            "opt.sidebars.collapsed_width",
            "collapsed width 8 draws the frame ‹; width 6 and 5 keep the initials and omit that frame",
            &args,
        );
    }
    let options = option_page(&app);
    assert_defaults_except(
        &options,
        &[
            ("Collapsed (icons)", "on", true),
            ("Collapsed width", width, false),
        ],
    );
    app.finish();
}

#[test]
fn option_desktop_keys_ignore_j() {
    let vim_args = ["--page", "sidebars"];
    let app = open("s04-opt-vim", 120, 40, &vim_args);
    let hint = app.wait_text("j k");
    see(
        &hint,
        &["j k", "T Tasks", LAST_NONE],
        &["NavAction::Activated"],
    );
    app.key("tab");
    app.wait_text("▎Playground");
    app.key("tab");
    app.wait_text("▎› T Tasks");
    app.key("j");
    let moved = app.wait_text("▎  R Runs");
    let tasks = row_with(&moved, "T Tasks");
    let runs = row_with(&moved, "R Runs");
    assert!(tasks.contains('›'), "current left Tasks\n{tasks}");
    assert!(!tasks.contains('▎'), "gutter stayed on Tasks\n{tasks}");
    assert!(runs.contains('▎'), "gutter missed Runs\n{runs}");
    assert!(!runs.contains('›'), "current jumped to Runs\n{runs}");
    see(
        &moved,
        &[LAST_NONE, "j k"],
        &["NavAction::Activated", LAST_HIDE],
    );
    app.finish();

    let args = ["--page", "sidebars", "--opt", "sidebars.keys=desktop"];
    let app = open("s04-opt-desktop", 120, 40, &args);
    let hint = app.wait_text("move the cursor");
    see(
        &hint,
        &["↑↓ move the cursor, Enter opens.", "T Tasks", LAST_NONE],
        &["j k", "NavAction::Activated"],
    );
    app.key("tab");
    app.wait_text("▎Playground");
    app.key("tab");
    app.wait_text("▎› T Tasks");
    app.key("j");
    let stayed = app.wait_screen("desktop j stays", |screen| {
        if !row_has(screen, "T Tasks") || !row_has(screen, "R Runs") {
            return false;
        }
        let tasks = row_with(screen, "T Tasks");
        let runs = row_with(screen, "R Runs");
        tasks.contains('▎') && tasks.contains('›') && !runs.contains('▎')
    });
    see(
        &stayed,
        &["↑↓ move the cursor, Enter opens.", LAST_NONE, "T Tasks"],
        &["j k", "NavAction::Activated", "▎  R Runs"],
    );
    shot(
        &app,
        "opt.sidebars.keys",
        "desktop keys leave the cursor on Tasks when j is pressed, and the hint omits j k",
        &args,
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Keys", "desktop", false)]);
    app.finish();
}

#[test]
fn option_badges_off_removes_the_tasks_count() {
    let args = ["--page", "sidebars", "--opt", "sidebars.badges=off"];
    let app = open("s04-opt-badges", 120, 40, &args);
    let play = app.wait_text("T Tasks");
    let tasks = row_with(&play, "T Tasks");
    assert!(!tasks.contains('3'), "badge stayed on Tasks\n{tasks}");
    see(
        &play,
        &[TITLE, "T Tasks", "Runs", LAST_NONE, "Collapse"],
        &["NavAction::Activated", DOTS],
    );
    shot(
        &app,
        "opt.sidebars.badges",
        "badges off removes the 3 from Tasks and leaves Runs and the Collapse button",
        &args,
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Badges", "off", true)]);
    app.finish();
}

#[test]
fn option_hidden_on_replaces_the_list_with_the_rail() {
    let args = ["--page", "sidebars", "--opt", "sidebars.hidden=on"];
    let app = open("s04-opt-hidden", 120, 40, &args);
    let play = app.wait_text("One focus stop");
    see(
        &play,
        &[TITLE, LAST_NONE, CAP_PLAY, "Tasks", "N    One focus"],
        &[
            "T Tasks",
            "Runs",
            "Branches",
            DOTS,
            LAST_HIDE,
            "E    One focus",
        ],
    );
    assert_eq!(vertical_rail(&play), "Navigation");
    assert!(card_title_line(&play).contains("Tasks"));
    shot(
        &app,
        "opt.sidebars.hidden",
        "hidden on replaces the list with a vertical Navigation rail and leaves Last at none yet",
        &args,
    );
    let options = option_page(&app);
    assert_defaults_except(&options, &[("Hidden (rail)", "on", true)]);
    app.finish();
}

#[test]
fn option_collapsed_rail_off_draws_no_rail_letters() {
    let args = [
        "--page",
        "sidebars",
        "--opt",
        "sidebars.hidden=on",
        "--opt",
        "sidebars.collapsed_rail=off",
    ];
    let app = open("s04-opt-rail-off", 120, 40, &args);
    let play = app.wait_text("One focus stop");
    see(
        &play,
        &[TITLE, LAST_NONE, CAP_PLAY, "Tasks", "One focus stop"],
        &[
            "T Tasks",
            "Runs",
            "Branches",
            DOTS,
            "N    One focus",
            "E    One focus",
            LAST_HIDE,
        ],
    );
    assert!(card_title_line(&play).contains("Tasks"));
    shot(
        &app,
        "opt.sidebars.collapsed_rail",
        "collapsed rail off with hidden on draws the card and no vertical rail letters",
        &args,
    );
    let options = option_page(&app);
    assert_defaults_except(
        &options,
        &[
            ("Hidden (rail)", "on", true),
            ("Rail when hidden", "off", true),
        ],
    );
    app.finish();
}

#[test]
fn option_rail_label_explorer_spells_the_rail() {
    let args = [
        "--page",
        "sidebars",
        "--opt",
        "sidebars.hidden=on",
        "--opt",
        "sidebars.rail_label=Explorer",
    ];
    let app = open("s04-opt-explorer", 120, 40, &args);
    let play = app.wait_text("E    One focus");
    see(
        &play,
        &[TITLE, LAST_NONE, "Tasks", "E    One focus"],
        &["T Tasks", "Runs", "N    One focus", DOTS, LAST_HIDE],
    );
    assert_eq!(vertical_rail(&play), "Explorer");
    assert!(card_title_line(&play).contains("Tasks"));
    shot(
        &app,
        "opt.sidebars.rail_label",
        "rail label Explorer is spelled down the rail when hidden is on; Navigation is the other value",
        &args,
    );
    let options = option_page(&app);
    assert_defaults_except(
        &options,
        &[
            ("Hidden (rail)", "on", true),
            ("Rail label", "Explorer", false),
        ],
    );
    app.finish();
}
