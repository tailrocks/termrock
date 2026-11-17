//! Selection route, sections, specimens, compare levels, policies, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s03_selection/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::bins::showcase;
use crate::support::{Launch, Point, find_all, row_text, text, try_launch};

const TITLE: &str = "Tables › Selection";
const PAGE: &str = "Selectable table";
const BLURB: &str = "Select rows, edit one or many together, delete with a confirmation";
const BLURB_NARROW: &str = "Select rows, edit one or many together,";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const MIXED: &str = "— mixed —";

const TOP_SPECIMENS: &[(&str, &str)] = &[
    ("sc.selection.specimen.rest", "rest"),
    ("sc.selection.specimen.hover", "hover"),
    ("sc.selection.specimen.pressed", "pressed"),
    ("sc.selection.specimen.focused", "focused"),
    ("sc.selection.specimen.focused_hover", "focus + hover"),
    ("sc.selection.specimen.selected", "selected"),
    ("sc.selection.specimen.disabled", "disabled"),
];

const OMITTED: &[&str] = &["busy", "error", "editing", "loading", "overflow"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s03_selection/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
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

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("row {needle:?} missing\n{}", text(screen)))
}

fn line_after(screen: &TerminalSnapshot, label: &str) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
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
    let screen = app.settle();
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

fn click_sorted(
    app: &crate::support::App,
    label: &str,
    pred: impl Fn(&Point) -> bool,
    lowest: bool,
) {
    let screen = app.settle();
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert!(!hits.is_empty(), "missing {label:?}\n{}", text(&screen));
    hits.sort_by_key(|hit| hit.y);
    let hit = if lowest {
        *hits.last().expect("hit")
    } else {
        hits[0]
    };
    app.click(hit);
}

fn shot(app: &crate::support::App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn focus_table(app: &crate::support::App) {
    for _ in 0..8 {
        let screen = app.settle();
        let foot = row_text(&screen, screen.rows().saturating_sub(1));
        if foot.contains("Space Select") || foot.contains("Extend") {
            return;
        }
        app.key("tab");
    }
    panic!("table focus missing\n{}", text(&app.screen()));
}

fn next_page(app: &crate::support::App) {
    let before = text(&app.screen());
    app.key("]");
    app.wait_screen("next page", move |screen| text(screen) != before);
}

fn narrow_body(screen: &TerminalSnapshot) -> bool {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let Some(index) = lines.iter().position(|line| line.contains("narrow")) else {
        return false;
    };
    lines
        .get(index + 1)
        .is_some_and(|line| line.contains("Dana Whitfield"))
        && text(screen).contains("No rows")
}

fn reveal_folded_specimens(app: &crate::support::App) {
    let point = Point { x: 70, y: 20 };
    if !narrow_body(&app.settle()) {
        app.wheel(point, true, 3);
    }
    let screen = app.settle();
    assert!(
        narrow_body(&screen),
        "empty and narrow stayed below the fold\n{}",
        text(&screen)
    );
}

fn reveal_revert(app: &crate::support::App) {
    let point = Point { x: 60, y: 22 };
    app.wheel_until(point, true, 2, 6, "Revert", |screen| {
        text(screen).contains("Revert")
    });
}

fn assert_defaults_except(screen: &TerminalSnapshot, changed: &str) {
    let esc = row_with(screen, "Esc clears the selection");
    let checks = row_with(screen, "Checkbox column");
    let mods = line_after(screen, "Toggle click");
    let disabled = line_after(screen, "Disabled rows");
    let activate = line_after(screen, "Click opens");
    if changed != "esc" {
        assert!(
            esc.contains("Esc clears the selection on") && !esc.contains(" off"),
            "esc line: {esc}"
        );
    }
    if changed != "checks" {
        assert!(
            checks.contains("Checkbox column on") && !checks.contains(" off"),
            "checkbox line: {checks}"
        );
    }
    if changed != "mods" {
        assert!(
            mods.contains("platform") && !mods.contains("ctrl") && !mods.contains("alt"),
            "toggle line: {mods}"
        );
    }
    if changed != "disabled" {
        assert!(
            disabled.contains("owner") && !disabled.contains("none"),
            "disabled line: {disabled}"
        );
    }
    if changed != "activate" {
        assert!(
            activate.contains("double")
                && !activate.contains("never")
                && !activate.contains("single"),
            "activate line: {activate}"
        );
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "selection"];
    let app = open("s03-route-page", 120, 40, &args);
    let screen = app.wait_text("Dana Whitfield");
    see(
        &screen,
        &[
            TITLE,
            PAGE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Members",
            "Dana Whitfield",
            "Ari Moreno",
            "No member selected",
            "1–10 of 12",
        ],
        &[
            OVERVIEW,
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "1 selected",
        ],
    );
    shot(
        &app,
        "sc.selection.route",
        "--page selection opens the playground with the sidebar on Selection",
        &args,
    );
    app.finish();

    let app = open("s03-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    for _ in 0..18 {
        next_page(&app);
    }
    let entered = app.wait_text(PAGE);
    see(
        &entered,
        &[TITLE, PAGE, BLURB, "Dana Whitfield", CAP_PLAY],
        &[
            OVERVIEW,
            "Page options",
            "Typed cells, a pending-change queue",
        ],
    );
    app.key("[");
    let left = app.wait_text("Typed cells, a pending-change queue");
    see(
        &left,
        &["Data grid", "Typed cells, a pending-change queue"],
        &[PAGE, "Dana Whitfield"],
    );
    app.key("]");
    let again = app.wait_text(PAGE);
    see(
        &again,
        &[TITLE, PAGE, "Ari Moreno"],
        &["Typed cells, a pending-change queue"],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[PAGE, "Dana Whitfield"]);
    click_side(&app, "Tables");
    app.wait_text("Selection");
    click_side(&app, "Selection");
    let pointed = app.wait_text(PAGE);
    see(
        &pointed,
        &[TITLE, PAGE, BLURB, "Dana Whitfield", "No member selected"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "selection"];
    let app = open("s03-sections", 80, 24, &args);
    let narrow_play = app.wait_text("Dana Whitfield");
    see(
        &narrow_play,
        &[
            TITLE,
            PAGE,
            BLURB_NARROW,
            "Playground",
            "Members",
            "[ ] Dana Whitfield",
            "Ari Moreno",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "No member selected",
            "1 selected",
        ],
    );
    shot(
        &app,
        "sc.selection.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    see(
        &narrow_opt,
        &[
            TITLE,
            PAGE,
            "Page options",
            "Esc clears the selection on",
            "Toggle click",
            "platform",
            "Disabled rows",
            "owner",
            "Checkbox column on",
            "Click opens",
            "double",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Dana Whitfield",
            WIDE,
            "truecolor",
            "Checkbox column off",
            "Esc clears the selection off",
        ],
    );
    shot(
        &app,
        "sc.selection.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            PAGE,
            "○ Compare levels",
            "rest",
            "hover",
            "Dana Whitfield",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            WIDE,
            "No rows",
            "busy",
            "error",
            "editing",
            "loading",
            "overflow",
        ],
    );
    shot(
        &app,
        "sc.selection.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    app.wait_text(WIDE);
    let wide_states = app.wait_text("focus + hover");
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            LEGEND,
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "selected",
            "disabled",
            "Checkbox column",
            "Rows only",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "No rows",
            "narrow",
            "256 colors",
            "16 colors",
            "no color",
        ],
    );
    for omitted in OMITTED {
        assert!(
            !text(&wide_states).contains(omitted),
            "omitted specimen {omitted} is drawn"
        );
    }
    shot(
        &app,
        "sc.selection.states.120x40",
        "states caption and the grid rows that fit at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.selection.states",
        "states is the visible Selection section",
        &args,
    );
    for (case_id, label) in TOP_SPECIMENS {
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES, "disabled"],
            &["● Compare levels", "Page options", "No rows", "narrow"],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &args,
        );
    }

    reveal_folded_specimens(&app);
    let folded = app.settle();
    see(
        &folded,
        &["empty", "No rows", "narrow", "Dana Whitfield", "Ari Moreno"],
        &[
            "○ Compare levels",
            "● Compare levels",
            "rest",
            CAP_PLAY,
            "Page options",
        ],
    );
    assert!(
        narrow_body(&folded),
        "narrow specimen body is clipped\n{}",
        text(&folded)
    );
    shot(
        &app,
        "sc.selection.specimen.empty",
        "empty specimen shows No rows",
        &args,
    );
    shot(
        &app,
        "sc.selection.specimen.narrow",
        "narrow specimen shows Dana Whitfield under the clipped header",
        &args,
    );

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            CAP_OPT,
            WIDE,
            "Page options",
            "Esc clears the selection on",
            "platform",
            "owner",
            "Checkbox column on",
            "double",
        ],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Dana Whitfield",
            "Checkbox column off",
            "Esc clears the selection off",
            "never",
        ],
    );
    assert_defaults_except(&wide_opt, "");
    shot(
        &app,
        "sc.selection.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.selection.options",
        "options is the visible Selection section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    see(
        &wide_play,
        &[
            TITLE,
            PAGE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Members",
            "Dana Whitfield",
            "June Park",
            "No member selected",
            "1–10 of 12",
            "Shift+↑↓ extends",
            "selects all.",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "1 selected",
            "Edit member",
        ],
    );
    shot(
        &app,
        "sc.selection.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.selection.playground",
        "playground is the visible Selection section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "selection"];
    let app = open("s03-compare", 120, 40, &args);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", "disabled", CAP_STATES],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 12 && hit.x > 24);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", CAP_STATES, WIDE],
        &["○ Compare levels", "Page options", CAP_PLAY, "no color"],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    for _ in 0..8 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.key("pagedown");
    }
    let bands = app.wait_text("no color");
    assert!(saw_256, "256 colors never appeared\n{}", text(&bands));
    assert!(saw_16, "16 colors never appeared\n{}", text(&bands));
    see(
        &bands,
        &[TITLE, "no color", WIDE, "Compare levels", "rest"],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.selection.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_text("Compare levels");
    see(
        &narrow,
        &[TITLE, "Compare levels", "truecolor"],
        &["Page options", CAP_PLAY, WIDE, "120×40"],
    );
    shot(
        &app,
        "sc.selection.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert_defaults_except(&options, "");
    see(
        &options,
        &[
            "Esc clears the selection on",
            "platform",
            "owner",
            "double",
            CAP_OPT,
        ],
        &["Compare levels", "never", "Checkbox column off"],
    );
    app.finish();
}

#[test]
fn policies_narrow_then_each_change() {
    let args = ["--page", "selection"];
    let app = open("s03-policies", 80, 24, &args);
    let narrow = app.wait_text("Dana Whitfield");
    see(
        &narrow,
        &[TITLE, PAGE, "Members", "[ ] Dana Whitfield", "Playground"],
        &[
            "1 selected",
            "selected",
            CAP_PLAY,
            WIDE,
            "Page options",
            "Compare levels",
        ],
    );
    shot(
        &app,
        "sc.selection.policies.80x24",
        "narrow playground before the terminal grows",
        &args,
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    focus_table(&app);
    let before = app.wait_text("No member selected");
    see(
        &before,
        &[
            "[ ] Dana Whitfield",
            "[ ] Ari Moreno",
            "No member selected",
            CAP_PLAY,
            WIDE,
        ],
        &["1 selected", "Edit member", "[✓]", "Page options"],
    );
    shot(
        &app,
        "sc.selection.policies.before",
        "table focused with nothing selected",
        &args,
    );
    app.key(" ");
    let skipped = app.settle();
    see(
        &skipped,
        &["[ ] Dana Whitfield", "No member selected"],
        &["1 selected", "[✓] Dana Whitfield"],
    );
    app.key("down");
    app.key(" ");
    let single = app.wait_text("1 selected");
    see(
        &single,
        &[
            "[✓] Ari Moreno",
            "[ ] Dana Whitfield",
            "Edit member",
            "Option+click",
            "ari@northwind.dev",
        ],
        &[
            "2 selected",
            "[✓] Dana Whitfield",
            "Edit 2 members",
            "Ctrl+click",
            MIXED,
        ],
    );
    shot(
        &app,
        "sc.selection.policies.single",
        "Space selects Ari and leaves Dana clear",
        &args,
    );
    app.key("shift+down");
    let range = app.wait_text("2 selected");
    see(
        &range,
        &[
            "[✓] Ari Moreno",
            "[✓] Cleo Barnes",
            "[ ] Dana Whitfield",
            "Edit 2 members",
            MIXED,
            "Applies to all 2",
        ],
        &[
            "1 selected",
            "11 selected",
            "[✓] Dana Whitfield",
            "[✓] Bo Lindqvist",
        ],
    );
    shot(
        &app,
        "sc.selection.policies.range",
        "Shift+Down extends the set to Cleo and the bulk role stays mixed",
        &args,
    );
    app.key("down");
    let moved = app.settle();
    let bo = row_with(&moved, "Bo Lindqvist");
    assert!(
        bo.contains('▎') && bo.contains("[ ]"),
        "cursor left the unchecked Bo row: {bo}"
    );
    see(
        &moved,
        &[
            "2 selected",
            "[✓] Ari Moreno",
            "[✓] Cleo Barnes",
            "[ ] Dana Whitfield",
        ],
        &["3 selected", "[✓] Bo Lindqvist", "[✓] Dana Whitfield"],
    );
    shot(
        &app,
        "sc.selection.policies.move",
        "Down moves the cursor to Bo and keeps Ari and Cleo selected",
        &args,
    );
    app.key("up");
    app.key(" ");
    let toggled = app.wait_text("1 selected");
    see(
        &toggled,
        &[
            "[✓] Ari Moreno",
            "[ ] Cleo Barnes",
            "[ ] Dana Whitfield",
            "Edit member",
        ],
        &["2 selected", "[✓] Cleo Barnes", "[✓] Dana Whitfield"],
    );
    shot(
        &app,
        "sc.selection.policies.toggle",
        "Space on Cleo removes her and leaves Ari selected",
        &args,
    );
    app.key("ctrl+a");
    let all = app.wait_text("11 selected");
    see(
        &all,
        &[
            "11 selected",
            "[ ] Dana Whitfield",
            "[✓] Ari Moreno",
            "[✓] Cleo Barnes",
            "Edit 11 members",
        ],
        &["12 selected", "[✓] Dana Whitfield"],
    );
    shot(
        &app,
        "sc.selection.policies.all",
        "Select All checks every enabled row and skips Dana",
        &args,
    );
    click_sorted(&app, MIXED, |hit| hit.y > 20 && hit.x < 55, false);
    click_sorted(&app, "Admin", |hit| hit.y > 20 && hit.x < 70, true);
    app.wait_text("Unsaved changes");
    reveal_revert(&app);
    let dirty = app.wait_text("Revert");
    see(
        &dirty,
        &[
            "Edit 11 members",
            "Unsaved changes",
            "Apply",
            "Revert",
            "Admin",
        ],
        &["[✓] Dana Whitfield", "Updated 11 members"],
    );
    shot(
        &app,
        "sc.selection.policies.dirty",
        "the mixed role is Admin, with Apply, Revert, and Unsaved changes",
        &args,
    );
    click_where(&app, "Apply", |hit| {
        hit.y >= 30 && hit.y <= 37 && hit.x > 24
    });
    let applied = app.wait_text("Updated 11 members");
    see(
        &applied,
        &["Updated 11 members", "Edit 11 members", "Admin"],
        &["Unsaved changes", "[✓] Dana Whitfield"],
    );
    shot(
        &app,
        "sc.selection.policies.applied",
        "Apply writes Admin for the 11 selected members",
        &args,
    );
    click_sorted(&app, "Admin", |hit| hit.y > 20 && hit.x < 55, false);
    click_sorted(&app, "Viewer", |hit| hit.y > 20 && hit.x < 70, true);
    app.wait_text("Unsaved changes");
    for _ in 0..8 {
        let screen = app.settle();
        let foot = row_text(&screen, screen.rows().saturating_sub(1));
        if foot.contains("Enter Revert") {
            break;
        }
        app.key("tab");
    }
    assert!(
        row_text(&app.settle(), app.screen().rows().saturating_sub(1)).contains("Enter Revert"),
        "Revert never took focus\n{}",
        text(&app.screen())
    );
    app.key("enter");
    let after = app.wait_text("Changes reverted");
    see(
        &after,
        &["Edit 11 members", "Admin", "Changes reverted"],
        &["Unsaved changes", "Viewer", "[✓] Dana Whitfield"],
    );
    let role = (0..after.rows())
        .map(|row| row_text(&after, row))
        .find(|line| line.contains('▾') && line.contains("Admin"))
        .unwrap_or_else(|| panic!("editor role missing\n{}", text(&after)));
    assert!(!role.contains("Viewer"), "reverted role row: {role}");
    shot(
        &app,
        "sc.selection.policies.after",
        "Revert restores the applied Admin role and Dana stays unselected",
        &args,
    );
    app.finish();
}

fn open_options(app: &crate::support::App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options")
}

fn back_to_table(app: &crate::support::App) {
    click_tab(app, "Playground");
    app.wait_text("Members");
    focus_table(app);
}

#[test]
fn option_esc_clears_off_keeps_the_row() {
    let args = ["--page", "selection", "--opt", "selection.esc_clears=off"];
    let app = open("s03-esc", 120, 40, &args);
    focus_table(&app);
    app.key("down");
    app.key(" ");
    app.wait_text("1 selected");
    app.key("esc");
    let kept = app.settle();
    see(
        &kept,
        &[
            "1 selected",
            "[✓] Ari Moreno",
            "[ ] Dana Whitfield",
            "Edit member",
        ],
        &["No member selected", "[ ] Ari Moreno", "2 selected"],
    );
    let options = open_options(&app);
    let esc = row_with(&options, "Esc clears the selection");
    assert!(
        esc.contains("Esc clears the selection off"),
        "esc line: {esc}"
    );
    assert_defaults_except(&options, "esc");
    back_to_table(&app);
    let again = app.wait_text("1 selected");
    see(
        &again,
        &["[✓] Ari Moreno", "1 selected"],
        &["No member selected", "Page options"],
    );
    shot(
        &app,
        "opt.selection.esc_clears",
        "Esc leaves Ari selected when Esc clears the selection is off",
        &args,
    );
    app.finish();
}

#[test]
fn option_toggle_mods_ctrl() {
    let args = ["--page", "selection", "--opt", "selection.toggle_mods=ctrl"];
    let app = open("s03-mods", 120, 40, &args);
    focus_table(&app);
    app.key("down");
    app.key(" ");
    let selected = app.wait_text("Ctrl+click");
    see(
        &selected,
        &["1 selected", "[✓] Ari Moreno", "Ctrl+click"],
        &["Option+click", "[✓] Dana Whitfield", "2 selected"],
    );
    let options = open_options(&app);
    let mods = line_after(&options, "Toggle click");
    assert!(
        mods.contains("ctrl") && !mods.contains("platform"),
        "toggle line: {mods}"
    );
    assert_defaults_except(&options, "mods");
    back_to_table(&app);
    let again = app.wait_text("Ctrl+click");
    see(
        &again,
        &["Ctrl+click", "1 selected"],
        &["Option+click", "Page options"],
    );
    shot(
        &app,
        "opt.selection.toggle_mods",
        "ctrl toggle hint is Ctrl+click and the other options stay default",
        &args,
    );
    app.finish();
}

#[test]
fn option_disabled_rows_none_selects_dana() {
    let args = [
        "--page",
        "selection",
        "--opt",
        "selection.disabled_rows=none",
    ];
    let app = open("s03-disabled", 120, 40, &args);
    focus_table(&app);
    app.key(" ");
    let dana = app.wait_text("1 selected");
    see(
        &dana,
        &["[✓] Dana Whitfield", "1 selected", "Edit member"],
        &["[ ] Dana Whitfield", "12 selected"],
    );
    app.key("ctrl+a");
    let all = app.wait_text("12 selected");
    see(
        &all,
        &["12 selected", "[✓] Dana Whitfield", "[✓] Ari Moreno"],
        &["11 selected", "[ ] Dana Whitfield"],
    );
    let options = open_options(&app);
    let disabled = line_after(&options, "Disabled rows");
    assert!(
        disabled.contains("none") && !disabled.contains("owner"),
        "disabled line: {disabled}"
    );
    assert_defaults_except(&options, "disabled");
    back_to_table(&app);
    let again = app.wait_text("12 selected");
    see(
        &again,
        &["[✓] Dana Whitfield", "12 selected"],
        &["[ ] Dana Whitfield", "Page options"],
    );
    shot(
        &app,
        "opt.selection.disabled_rows",
        "none lets Select All include Dana, so the count is 12",
        &args,
    );
    app.finish();
}

#[test]
fn option_check_column_off() {
    let args = ["--page", "selection", "--opt", "selection.check_column=off"];
    let app = open("s03-checks", 120, 40, &args);
    let play = app.wait_text("Dana Whitfield");
    see(
        &play,
        &[TITLE, "Dana Whitfield", "Ari Moreno", CAP_PLAY, "Name"],
        &["[ ] Dana", "[✓]", "Page options", "[–]"],
    );
    focus_table(&app);
    app.key("down");
    app.key(" ");
    let selected = app.wait_text("1 selected");
    see(
        &selected,
        &["1 selected", "Edit member", "✓ Ari Moreno"],
        &["[✓] Ari Moreno", "[ ] Dana", "2 selected"],
    );
    let options = open_options(&app);
    let checks = row_with(&options, "Checkbox column");
    assert!(
        checks.contains("Checkbox column off"),
        "checkbox line: {checks}"
    );
    assert_defaults_except(&options, "checks");
    back_to_table(&app);
    let again = app.wait_text("1 selected");
    see(
        &again,
        &["1 selected", "✓ Ari Moreno"],
        &["[✓] Ari Moreno", "Page options"],
    );
    shot(
        &app,
        "opt.selection.check_column",
        "the checkbox column is gone and Ari can still be selected",
        &args,
    );
    app.finish();
}

#[test]
fn option_activate_double_and_never() {
    let defaults = ["--page", "selection"];
    let app = open("s03-activate-single", 120, 40, &defaults);
    app.wait_text("Ari Moreno");
    click_where(&app, "Ari Moreno", |hit| hit.x > 30 && hit.y < 20);
    let once = app.wait_text("1 selected");
    see(
        &once,
        &["1 selected", "[✓] Ari Moreno", "Edit member", "Enter Edit"],
        &["Next field", "Enter Apply", "2 selected"],
    );
    app.finish();

    let app = open("s03-activate-double", 120, 40, &defaults);
    app.wait_text("Ari Moreno");
    let hit = locate(&app, "Ari Moreno", |hit| hit.x > 30 && hit.y < 20);
    app.click(hit);
    app.click(hit);
    let editing = app.wait_text("Next field");
    see(
        &editing,
        &["Next field", "Enter Apply", "Ari Moreno", "1 selected"],
        &["Space Select", "Ctrl+A Select all"],
    );
    app.finish();

    let args = ["--page", "selection", "--opt", "selection.activate=never"];
    let app = open("s03-activate-never", 120, 40, &args);
    app.wait_text("Ari Moreno");
    click_where(&app, "Ari Moreno", |hit| hit.x > 30 && hit.y < 20);
    click_where(&app, "Ari Moreno", |hit| hit.x > 30 && hit.y < 20);
    let stayed = app.wait_text("1 selected");
    see(
        &stayed,
        &["1 selected", "[✓] Ari Moreno", "Enter Edit", "Edit member"],
        &["Next field", "Enter Apply", "2 selected"],
    );
    let options = open_options(&app);
    let activate = line_after(&options, "Click opens");
    assert!(
        activate.contains("never") && !activate.contains("double"),
        "activate line: {activate}"
    );
    assert_defaults_except(&options, "activate");
    back_to_table(&app);
    let again = app.wait_text("1 selected");
    see(
        &again,
        &["1 selected", "[✓] Ari Moreno"],
        &["Next field", "Page options"],
    );
    shot(
        &app,
        "opt.selection.activate",
        "never keeps a second click on Ari out of the name field",
        &args,
    );
    app.finish();
}
