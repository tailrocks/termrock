//! Inputs route, sections, options, and editing, driven through the showcase PTY.
//!
//! Tuiscotti state ids allow only `[a-z0-9_-]` segments
//! (`validate_test_id_grammar` in tuiscotti `sample.rs`). `S02.inputs` and the
//! dotted case ids are not legal segments. Each checkpoint is stored as
//! `pre_refactor/s02_inputs/<case id with '.' replaced by '_'>`.

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const HEADER: &str = "Inputs & forms › Inputs";
const PLAY: &str = "the live widget";
const OPT: &str = "one control per option";
const STATE: &str = "every state, drawn by the widget";
const PRESET: &str = "the preset's key map";
const CUSTOM_MAP: &str = "custom key map: Alt+A selects all";
const PROJECT: &str = "payments-gateway";
const READOUT: &str = "214 files";
const OPTIONAL: &str = "optional";
const EMAIL: &str = "Enter a valid email address";
const BRANCH: &str = "Branch to check out once the task is done";
const HELP: &str = "Used as the working directory name";
const CUSTOM_REQUIRED: &str = "Name the project to continue";
const SELECTED: SnapshotColor = SnapshotColor::Rgb(31, 52, 96);
const FIELD_BG: SnapshotColor = SnapshotColor::Rgb(58, 58, 64);
const MUTED: SnapshotColor = SnapshotColor::Rgb(138, 138, 138);
const CHOSEN: SnapshotColor = SnapshotColor::Rgb(255, 255, 255);

fn launch(name: &str, args: &[&str], cols: u16, rows: u16) -> App {
    let mut spec = Launch::new(name, SHOWCASE);
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.cols = cols;
    spec.rows = rows;
    spec.color = Some("truecolor".to_string());
    spec.motion = Some("paused".to_string());
    try_launch(spec).unwrap_or_else(|error| panic!("{error}"))
}

fn page(name: &str, opt: Option<&str>, cols: u16, rows: u16) -> App {
    let mut args = vec!["--page", "inputs"];
    if let Some(value) = opt {
        args.push("--opt");
        args.push(value);
    }
    launch(name, &args, cols, rows)
}

fn test_id(case_id: &str) -> String {
    format!("pre_refactor/s02_inputs/{}", case_id.replace('.', "_"))
}

fn see(app: &App, case_id: &str, _precondition: &str) {
    app.visual(&test_id(case_id));
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn focus_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_else(|| panic!("no focus gutter\n{}", text(screen)))
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no row {needle:?}\n{}", text(screen)))
}

fn row_after(screen: &TerminalSnapshot, needle: &str) -> String {
    let index = (0..screen.rows())
        .find(|row| row_text(screen, *row).contains(needle))
        .unwrap_or_else(|| panic!("no row {needle:?}\n{}", text(screen)));
    row_text(screen, index + 1)
}

fn expect(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for needle in present {
        assert!(
            visible.contains(needle),
            "visible result missing {needle:?}\n{visible}"
        );
    }
    for needle in absent {
        assert!(
            !visible.contains(needle),
            "forbidden effect still visible {needle:?}\n{visible}"
        );
    }
}

fn click_match(app: &App, needle: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.wait_text(needle);
    let hits: Vec<_> = find_all(&screen, needle)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {needle:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_tab(app: &App, label: &str) {
    click_match(app, label, |hit| hit.y <= 5);
}

fn click_nav(app: &App, label: &str) {
    click_match(app, label, |hit| hit.x < 24);
}

fn click_child(app: &App, label: &str) {
    let screen = app.wait_screen(label, |screen| {
        find_all(screen, label)
            .into_iter()
            .any(|hit| hit.x < 24 && !row_text(screen, u32::from(hit.y)).contains('&'))
    });
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x < 24 && !row_text(&screen, u32::from(hit.y)).contains('&'))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one sidebar {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_content(app: &App, needle: &str) {
    click_match(app, needle, |hit| hit.x > 24);
}

fn focus_project(app: &App, footer_needle: &str) {
    app.key("tab");
    app.wait_screen("section strip", |screen| footer(screen).contains("Section"));
    app.key("tab");
    app.wait_screen("project field", |screen| {
        footer(screen).contains(footer_needle)
    });
}

fn fg_of(screen: &TerminalSnapshot, needle: &str) -> SnapshotColor {
    let hit = find_all(screen, needle)
        .into_iter()
        .find(|hit| hit.x > 20)
        .unwrap_or_else(|| panic!("no {needle:?}\n{}", text(screen)));
    screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .unwrap_or_else(|| panic!("missing cell {needle:?}"))
        .fg()
}

fn project_bgs(screen: &TerminalSnapshot) -> Option<Vec<SnapshotColor>> {
    let hit = find_all(screen, PROJECT)
        .into_iter()
        .find(|hit| hit.x > 24)?;
    Some(
        (0..16)
            .map(|offset| {
                screen
                    .cell_at(u32::from(hit.x + offset), u32::from(hit.y))
                    .map(|cell| cell.bg())
                    .unwrap_or(SnapshotColor::Default)
            })
            .collect(),
    )
}

fn kept_playground(screen: &TerminalSnapshot) {
    expect(
        screen,
        &[HEADER, READOUT, OPTIONAL, PRESET, EMAIL, "Project name *"],
        &[CUSTOM_MAP, "sk-live", "Page options", OPT, STATE],
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let keys = launch("s02-inputs-route-keys", &[], 120, 40);
    let overview = keys.wait_text("Tokens and principles behind every component");
    expect(
        &overview,
        &["Overview", "Tokens and principles behind every component"],
        &[HEADER, PROJECT],
    );
    for _ in 0..8 {
        keys.key("]");
    }
    let arrived = keys.wait_text(HEADER);
    expect(
        &arrived,
        &[HEADER, PROJECT, PLAY, READOUT, OPTIONAL],
        &[
            "Tokens and principles behind every component",
            "› Buttons",
            OPT,
            STATE,
            "Page options",
            "sk-live",
        ],
    );
    assert!(
        footer(&arrived).contains("Into page"),
        "sidebar footer stays on the route: {}",
        footer(&arrived)
    );
    keys.key("[");
    let buttons = keys.wait_text("Inputs & forms › Buttons");
    expect(&buttons, &["Inputs & forms › Buttons"], &[HEADER, PROJECT]);
    keys.key("]");
    let again = keys.wait_text(HEADER);
    expect(&again, &[HEADER, PROJECT], &["› Buttons"]);

    click_nav(&keys, "Overview");
    let pointed_back = keys.wait_text("Tokens and principles behind every component");
    expect(
        &pointed_back,
        &["Tokens and principles behind every component"],
        &[HEADER, PROJECT],
    );
    click_nav(&keys, "Inputs & forms");
    click_child(&keys, "Inputs");
    let pointed = keys.wait_text(HEADER);
    expect(
        &pointed,
        &[HEADER, PROJECT, PLAY, READOUT],
        &[OPT, STATE, "Page options", "sk-live"],
    );
    assert!(
        footer(&pointed).contains("Into page"),
        "pointer open leaves the sidebar focused: {}",
        footer(&pointed)
    );

    let direct = page("s02-inputs-route-page", None, 120, 40);
    let screen = direct.wait_text(PROJECT);
    expect(
        &screen,
        &[
            HEADER,
            PROJECT,
            PLAY,
            READOUT,
            OPTIONAL,
            PRESET,
            "c1f2",
            EMAIL,
            "Desktop keys",
            "Emacs keys",
        ],
        &[
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            "sk-live",
            CUSTOM_MAP,
            "› Buttons",
        ],
    );
    assert!(
        footer(&screen).contains("Into page"),
        "direct --page inputs keeps the sidebar footer, got {}",
        footer(&screen)
    );
    assert_eq!(fg_of(&screen, "Desktop keys"), MUTED, "theme field keys");
    assert_eq!(fg_of(&screen, "Emacs keys"), MUTED, "theme field keys");
    see(
        &direct,
        "sc.inputs.route",
        "--page inputs opens the playground with the sidebar focus on Inputs",
    );
}

#[test]
fn sections_narrow_then_wide() {
    let app = page("s02-inputs-sections", None, 80, 24);
    app.wait_text(HEADER);
    click_tab(&app, "Playground");
    let narrow_play = app.wait_screen("playground 80", |screen| footer(screen).contains("Section"));
    expect(
        &narrow_play,
        &[HEADER, "Project name *", PROJECT, EMAIL, BRANCH],
        &[
            PLAY,
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            "sk-live",
        ],
    );
    see(
        &app,
        "sc.inputs.playground.80x24",
        "playground tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("Options");
    let narrow_opt = app.wait_text("Page options");
    expect(
        &narrow_opt,
        &[
            HEADER,
            "Page options",
            "Edit entry",
            "theme",
            "select-all",
            "Label click focuses on",
        ],
        &[PLAY, OPT, STATE, PROJECT, "Compare levels", READOUT],
    );
    see(
        &app,
        "sc.inputs.options.80x24",
        "options tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("States");
    let narrow_states = app.wait_text("Compare levels");
    expect(
        &narrow_states,
        &[
            HEADER,
            "○ Compare levels",
            "rest",
            "hover",
            "focused",
            "editing",
            "error",
            "Enter a valid value",
            PROJECT,
        ],
        &[
            PLAY,
            OPT,
            STATE,
            "Page options",
            "disabled",
            "empty",
            "overflow",
            "narrow",
            "pressed",
            "focus + hover",
            "selected",
            "busy",
            "loading",
            "120×40",
        ],
    );
    see(
        &app,
        "sc.inputs.states.80x24",
        "states tab activated at 80 by 24, before the terminal grows",
    );

    app.resize(120, 40);
    let wide_states = app.wait_text("narrow");
    expect(
        &wide_states,
        &[
            HEADER,
            STATE,
            "○ Compare levels",
            "120×40",
            "rest",
            "hover",
            "focused",
            "editing",
            "error",
            "disabled",
            "empty",
            "overflow",
            "narrow",
            "(feat/…)",
            "payments-gateway and a value",
            "Enter a valid value",
            "c1f2",
        ],
        &[
            PLAY,
            OPT,
            "Page options",
            "pressed",
            "focus + hover",
            "selected",
            "busy",
            "loading",
            "sk-live",
        ],
    );
    assert!(
        row_after(&wide_states, "rest").contains("c1f2"),
        "rest mask keeps the reveal tail"
    );
    assert!(
        !row_after(&wide_states, "editing").contains("c1f2"),
        "editing mask hides the reveal tail"
    );
    assert!(
        row_after(&wide_states, "empty").contains("(feat/…)"),
        "empty specimen shows the placeholder"
    );
    see(
        &app,
        "sc.inputs.states.120x40",
        "states tab still active after growing from 80 by 24 to 120 by 40",
    );

    app.click_text("Options");
    let wide_opt = app.wait_text(OPT);
    expect(
        &wide_opt,
        &[
            HEADER,
            OPT,
            "Page options",
            "Edit entry",
            "theme",
            "select-all",
            "commit",
            "Label click focuses on",
            "Wrap long labels off",
            "Hide \"optional\" off",
            "Readout on",
            "Owner rule",
            "email",
            "120×40",
        ],
        &[PLAY, STATE, PROJECT, "Compare levels", "Readout off"],
    );
    see(
        &app,
        "sc.inputs.options.120x40",
        "options tab at 120 by 40 after the narrow capture",
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(PLAY);
    expect(
        &wide_play,
        &[
            HEADER,
            PLAY,
            PROJECT,
            READOUT,
            OPTIONAL,
            PRESET,
            "c1f2",
            EMAIL,
            "Desktop keys",
            "Emacs keys",
            "120×40",
        ],
        &[
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            "sk-live",
            CUSTOM_MAP,
        ],
    );
    assert_eq!(fg_of(&wide_play, "Desktop keys"), MUTED);
    assert_eq!(fg_of(&wide_play, "Emacs keys"), MUTED);
    see(
        &app,
        "sc.inputs.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
    );
}

fn wheel_until(app: &App, down: bool, needles: &[&str]) {
    for _ in 0..40 {
        let screen = app.screen();
        if needles.iter().all(|needle| text(&screen).contains(needle)) {
            return;
        }
        app.wheel(Point { x: 60, y: 18 }, down, 4);
    }
    let screen = app.screen();
    expect(&screen, needles, &[]);
}

#[test]
fn compare_levels_then_restore() {
    let app = page("s02-inputs-compare", None, 120, 40);
    app.click_text("States");
    let off = app.wait_text("○ Compare levels");
    expect(
        &off,
        &["○ Compare levels", HEADER, "rest"],
        &["Page options", "256 colors", "● Compare levels"],
    );
    click_match(&app, "Compare levels", |hit| hit.y < 16);
    let on = app.wait_text("● Compare levels");
    expect(
        &on,
        &["● Compare levels", HEADER],
        &["Page options", "○ Compare levels"],
    );
    wheel_until(&app, true, &["256 colors"]);
    let band = app.screen();
    expect(
        &band,
        &[HEADER, "● Compare levels", "256 colors"],
        &["Page options", "○ Compare levels", PLAY],
    );
    // Band headings are separated by the specimen grid, so 256 colors and
    // no color do not fit together. Scroll until the mono band is visible.
    wheel_until(&app, true, &["no color"]);
    let bands = app.screen();
    expect(
        &bands,
        &[HEADER, "no color"],
        &["Page options", "○ Compare levels", PLAY],
    );
    see(
        &app,
        "sc.inputs.compare-levels.120x40",
        "compare levels on at 120 by 40 with the no color band scrolled into view",
    );

    app.resize(80, 24);
    app.wait_absent("120×40");
    wheel_until(&app, false, &["● Compare levels"]);
    let narrow = app.screen();
    expect(
        &narrow,
        &[HEADER, "● Compare levels", "truecolor"],
        &["Page options", "120×40", PLAY],
    );
    see(
        &app,
        "sc.inputs.compare-levels.80x24",
        "compare levels still on after narrowing to 80 by 24, before restoring 120 by 40",
    );

    wheel_until(&app, false, &["● Compare levels"]);
    click_match(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    expect(
        &restored,
        &["○ Compare levels", HEADER, "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    app.resize(120, 40);
    app.wait_text("120×40");
    click_tab(&app, "Playground");
    let playground = app.wait_text(PLAY);
    expect(
        &playground,
        &[HEADER, PLAY, PROJECT, READOUT, OPTIONAL, PRESET],
        &[
            CUSTOM_MAP,
            "Readout off",
            "› Buttons",
            "256 colors",
            "Page options",
        ],
    );
}

#[test]
fn option_edit_entry_explicit() {
    let theme = page("s02-edit-entry-theme", None, 120, 40);
    theme.wait_text(PROJECT);
    focus_project(&theme, "Apply");
    theme.type_text("Z");
    let replaced = theme.wait_screen("immediate replaces the selection", |screen| {
        let line = focus_line(screen);
        line.contains('Z') && !line.contains(PROJECT)
    });
    assert!(
        focus_line(&replaced).contains('Z'),
        "immediate select-all types over the value"
    );
    assert!(
        !focus_line(&replaced).contains(PROJECT),
        "forbidden payments-gateway after select-all replace: {}",
        focus_line(&replaced)
    );
    assert!(
        footer(&replaced).contains("Apply"),
        "immediate footer stays Apply, got {}",
        footer(&replaced)
    );
    kept_playground(&replaced);

    let app = page(
        "s02-edit-entry-explicit",
        Some("inputs.edit_entry=explicit"),
        120,
        40,
    );
    app.wait_text(PROJECT);
    focus_project(&app, "Edit");
    let idle = app.screen();
    assert!(
        footer(&idle).contains("Enter Edit") && !footer(&idle).contains("Apply"),
        "explicit focus waits for Enter, got {}",
        footer(&idle)
    );
    app.type_text("Z");
    let ignored = app.settle();
    assert!(
        focus_line(&ignored).contains(PROJECT),
        "explicit ignores a typed character until Enter: {}",
        focus_line(&ignored)
    );
    assert!(
        !focus_line(&ignored).contains('Z'),
        "forbidden Z before Enter: {}",
        focus_line(&ignored)
    );
    assert!(
        !text(&ignored).contains("payments-gatewayZ"),
        "forbidden append before Enter"
    );
    kept_playground(&ignored);
    see(
        &app,
        "opt.inputs.edit_entry",
        "inputs.edit_entry=explicit ignores typed text until Enter and leaves the other defaults",
    );
}

#[test]
fn option_focus_select_caret_start() {
    let theme = page("s02-focus-select-all", None, 120, 40);
    theme.wait_text(PROJECT);
    focus_project(&theme, "Apply");
    theme.type_text("Z");
    let replaced = theme.wait_screen("select-all replace", |screen| {
        let line = focus_line(screen);
        line.contains('Z') && !line.contains(PROJECT)
    });
    assert!(
        !focus_line(&replaced).contains(PROJECT),
        "default select-all replaces payments-gateway, got {}",
        focus_line(&replaced)
    );

    let app = page(
        "s02-focus-caret-start",
        Some("inputs.focus_select=caret-start"),
        120,
        40,
    );
    app.wait_text(PROJECT);
    focus_project(&app, "Apply");
    app.type_text("Z");
    let typed = app.wait_screen("caret start insert", |screen| {
        focus_line(screen).contains("Zpayments")
    });
    assert!(
        focus_line(&typed).contains("Zpayments"),
        "caret-start inserts at the front: {}",
        focus_line(&typed)
    );
    assert!(
        !text(&typed).contains("payments-gatewayZ"),
        "forbidden caret-end append"
    );
    expect(
        &typed,
        &[HEADER, READOUT, OPTIONAL, PRESET, EMAIL],
        &[CUSTOM_MAP, "sk-live", "Page options"],
    );
    see(
        &app,
        "opt.inputs.focus_select",
        "inputs.focus_select=caret-start inserts at the front of payments-gateway",
    );
}

#[test]
fn option_field_keys_desktop() {
    let theme = page("s02-field-keys-theme", None, 120, 40);
    let screen = theme.wait_text("Desktop keys");
    assert_eq!(
        fg_of(&screen, "Desktop keys"),
        MUTED,
        "theme leaves Desktop muted"
    );
    assert_eq!(
        fg_of(&screen, "Emacs keys"),
        MUTED,
        "theme leaves Emacs muted"
    );
    expect(
        &screen,
        &["Ctrl+A", "Ctrl+L", PRESET, PROJECT],
        &[CUSTOM_MAP],
    );

    let app = page(
        "s02-field-keys-desktop",
        Some("inputs.field_keys=desktop"),
        120,
        40,
    );
    let chosen = app.wait_text("Desktop keys");
    assert_eq!(
        fg_of(&chosen, "Desktop keys"),
        CHOSEN,
        "desktop field keys paints Desktop keys in the chosen colour"
    );
    assert_eq!(
        fg_of(&chosen, "Emacs keys"),
        MUTED,
        "desktop field keys leaves Emacs keys muted"
    );
    expect(
        &chosen,
        &[
            HEADER, PRESET, "Ctrl+A", "Ctrl+L", PROJECT, READOUT, OPTIONAL,
        ],
        &[CUSTOM_MAP, "sk-live", "Page options"],
    );
    see(
        &app,
        "opt.inputs.field_keys",
        "inputs.field_keys=desktop paints Desktop keys chosen and leaves Emacs keys muted",
    );
}

#[test]
fn option_text_keymap_custom() {
    let theme = page("s02-keymap-preset", None, 120, 40);
    let preset = theme.wait_text(PRESET);
    expect(
        &preset,
        &[PRESET, "Ctrl+L", PROJECT],
        &[CUSTOM_MAP, "Alt+A"],
    );

    let app = page(
        "s02-keymap-custom",
        Some("inputs.text_keymap=custom"),
        120,
        40,
    );
    let custom = app.wait_text(CUSTOM_MAP);
    expect(
        &custom,
        &[
            HEADER,
            CUSTOM_MAP,
            "Alt+A · Ctrl+A",
            "Alt+A · Ctrl+L",
            PROJECT,
            READOUT,
            OPTIONAL,
        ],
        &[PRESET, "sk-live", "Page options"],
    );
    see(
        &app,
        "opt.inputs.text_keymap",
        "inputs.text_keymap=custom shows Alt+A selects all and hides the preset key map",
    );
}

fn blur_to_branch(app: &App) {
    focus_project(app, "Apply");
    app.type_text("QQ");
    app.wait_screen("typed QQ", |screen| {
        focus_line(screen).contains("QQ") && !focus_line(screen).contains(PROJECT)
    });
    click_content(app, "feat/");
}

#[test]
fn option_blur_action_cancel() {
    let commit = page("s02-blur-commit", None, 120, 40);
    commit.wait_text(PROJECT);
    blur_to_branch(&commit);
    let saved = commit.wait_text("Project name saved");
    expect(
        &saved,
        &["Project name saved", "QQ", READOUT, OPTIONAL, PRESET, EMAIL],
        &["Reverted", PROJECT, "sk-live", CUSTOM_MAP],
    );

    let app = page(
        "s02-blur-cancel",
        Some("inputs.blur_action=cancel"),
        120,
        40,
    );
    app.wait_text(PROJECT);
    blur_to_branch(&app);
    let reverted = app.wait_text("Reverted");
    expect(
        &reverted,
        &[
            "Reverted", PROJECT, READOUT, OPTIONAL, PRESET, EMAIL, HEADER,
        ],
        &["QQ", "saved", "sk-live", CUSTOM_MAP, "Page options"],
    );
    see(
        &app,
        "opt.inputs.blur_action",
        "inputs.blur_action=cancel reverts the typed QQ and leaves the other defaults",
    );
}

#[test]
fn option_pointer_select_off() {
    let theme = page("s02-pointer-default", None, 120, 40);
    theme.wait_text(PROJECT);
    focus_project(&theme, "Apply");
    let armed = theme.wait_screen("select-all highlight", |screen| {
        project_bgs(screen).is_some_and(|colors| colors.iter().all(|color| *color == SELECTED))
    });
    let origin = find_all(&armed, PROJECT)
        .into_iter()
        .find(|hit| hit.x > 24)
        .expect("project value");
    let end = Point {
        x: origin.x + 4,
        y: origin.y,
    };
    theme.down(origin);
    theme.drag_to(end);
    theme.up(end);
    let dragged = theme.wait_screen("partial selection", |screen| {
        project_bgs(screen).is_some_and(|colors| {
            colors[..4].iter().all(|color| *color == SELECTED)
                && colors[4..].iter().all(|color| *color == FIELD_BG)
        })
    });
    let colors = project_bgs(&dragged).expect("value cells");
    assert!(
        colors[..4].iter().all(|color| *color == SELECTED),
        "default drag keeps the first four cells selected: {colors:?}"
    );
    assert!(
        colors[4..].iter().all(|color| *color == FIELD_BG),
        "default drag clears the rest of the selection: {colors:?}"
    );

    let app = page(
        "s02-pointer-off",
        Some("inputs.pointer_select=off"),
        120,
        40,
    );
    app.wait_text(PROJECT);
    focus_project(&app, "Apply");
    let origin_screen = app.wait_screen("off starts selected", |screen| {
        project_bgs(screen).is_some_and(|colors| colors.iter().all(|color| *color == SELECTED))
    });
    let origin = find_all(&origin_screen, PROJECT)
        .into_iter()
        .find(|hit| hit.x > 24)
        .expect("project value");
    let end = Point {
        x: origin.x + 4,
        y: origin.y,
    };
    app.down(origin);
    app.drag_to(end);
    app.up(end);
    let cleared = app.wait_screen("drag ignored", |screen| {
        project_bgs(screen).is_some_and(|colors| colors.iter().all(|color| *color == FIELD_BG))
    });
    let colors = project_bgs(&cleared).expect("value cells");
    assert!(
        colors.iter().all(|color| *color == FIELD_BG),
        "pointer_select off leaves no selected cells: {colors:?}"
    );
    assert!(
        colors.iter().all(|color| *color != SELECTED),
        "forbidden selection colour after pointer_select off"
    );
    expect(
        &cleared,
        &[HEADER, PROJECT, READOUT, OPTIONAL, PRESET],
        &[CUSTOM_MAP, "sk-live", "Page options"],
    );
    see(
        &app,
        "opt.inputs.pointer_select",
        "inputs.pointer_select=off clears the drag selection and leaves the other defaults",
    );
}

#[test]
fn option_label_hit_off() {
    let theme = page("s02-label-on", None, 120, 40);
    theme.wait_text("Search files");
    click_content(&theme, "Search files");
    let entered = theme.wait_screen("label focuses search", |screen| {
        !footer(screen).contains("Into page")
    });
    assert!(
        footer(&entered).contains("Edit") || footer(&entered).contains("Apply"),
        "label click moves into the search field, got {}",
        footer(&entered)
    );
    assert!(
        !footer(&entered).contains("Into page"),
        "forbidden sidebar footer after label click: {}",
        footer(&entered)
    );

    let app = page("s02-label-off", Some("inputs.label_hit=off"), 120, 40);
    app.wait_text("Search files");
    click_content(&app, "Search files");
    let ignored = app.settle();
    assert!(
        footer(&ignored).contains("Into page"),
        "label_hit off leaves the sidebar footer, got {}",
        footer(&ignored)
    );
    assert!(
        !footer(&ignored).contains("Apply") && !footer(&ignored).contains("Ctrl+U"),
        "forbidden field footer after a dead label click: {}",
        footer(&ignored)
    );
    expect(
        &ignored,
        &[HEADER, PROJECT, READOUT, OPTIONAL, "Search files"],
        &["sk-live", CUSTOM_MAP, "Page options"],
    );
    see(
        &app,
        "opt.inputs.label_hit",
        "inputs.label_hit=off ignores a click on Search files and leaves the sidebar focused",
    );
}

#[test]
fn option_wrap_label_on() {
    let theme = page("s02-wrap-off", None, 120, 40);
    let screen = theme.wait_text(BRANCH);
    let branch = row_with(&screen, BRANCH);
    assert!(
        branch.contains(OPTIONAL) && !branch.contains("Project name"),
        "default branch label stays on its own row with optional: {branch}"
    );

    let app = page("s02-wrap-on", Some("inputs.wrap_label=on"), 120, 40);
    let wrapped = app.wait_text(BRANCH);
    let shared = (0..wrapped.rows())
        .map(|row| row_text(&wrapped, row))
        .find(|line| line.contains("Project name *") && line.contains(BRANCH))
        .unwrap_or_else(|| {
            panic!(
                "wrapped label did not join Project name\n{}",
                text(&wrapped)
            )
        });
    assert!(
        !shared.contains(OPTIONAL),
        "forbidden optional on the wrapped branch row: {shared}"
    );
    expect(
        &wrapped,
        &[HEADER, PROJECT, READOUT, "API token", OPTIONAL, PRESET],
        &[CUSTOM_MAP, "sk-live", "Page options"],
    );
    see(
        &app,
        "opt.inputs.wrap_label",
        "inputs.wrap_label=on places the branch sentence on the Project name row",
    );
}

#[test]
fn option_plain_label_on() {
    let theme = page("s02-plain-off", None, 120, 40);
    let screen = theme.wait_text(OPTIONAL);
    expect(&screen, &[OPTIONAL, "Project name *", READOUT], &[]);

    let app = page("s02-plain-on", Some("inputs.plain_label=on"), 120, 40);
    let plain = app.wait_text("Project name *");
    expect(
        &plain,
        &[HEADER, "Project name *", PROJECT, READOUT, PRESET, EMAIL],
        &[OPTIONAL, "sk-live", CUSTOM_MAP, "Page options"],
    );
    see(
        &app,
        "opt.inputs.plain_label",
        "inputs.plain_label=on hides every optional mark and keeps the readout",
    );
}

#[test]
fn option_readout_from_the_control() {
    let app = page("s02-readout", None, 120, 40);
    let playground = app.wait_text(READOUT);
    expect(
        &playground,
        &[READOUT, OPTIONAL, PROJECT, PRESET],
        &["Readout off"],
    );
    app.click_text("Options");
    let before = app.wait_text("Readout on");
    expect(
        &before,
        &["Readout on", "Label click focuses on", OPT],
        &["Readout off", PLAY],
    );
    app.click_text("Readout");
    let toggled = app.wait_text("Readout off");
    expect(
        &toggled,
        &[
            "Readout off",
            "Label click focuses on",
            "Wrap long labels off",
        ],
        &["Readout on", PLAY],
    );
    click_tab(&app, "Playground");
    let hidden = app.wait_text(PLAY);
    expect(
        &hidden,
        &[HEADER, PLAY, PROJECT, OPTIONAL, PRESET, EMAIL, "c1f2"],
        &[READOUT, "sk-live", CUSTOM_MAP, "Page options", "Readout on"],
    );
    see(
        &app,
        "opt.inputs.readout",
        "Readout toggled off from the Options control, then the playground hides 214 files",
    );
}

fn commit_empty(app: &App) {
    focus_project(app, "Apply");
    app.key("ctrl+u");
    let cleared = app.wait_screen("project cleared", |screen| {
        let line = focus_line(screen);
        line.contains('▎') && !line.contains(PROJECT) && !line.contains("Required")
    });
    assert!(
        text(&cleared).contains(HELP),
        "clear alone keeps the help line and does not show the required message\n{}",
        text(&cleared)
    );
    assert!(
        !text(&cleared).contains(CUSTOM_REQUIRED),
        "forbidden required message before commit"
    );
    app.key("enter");
}

#[test]
fn option_required_message_custom() {
    let theme = page("s02-required-theme", None, 120, 40);
    theme.wait_text(PROJECT);
    commit_empty(&theme);
    let message = theme.wait_text("Required");
    expect(
        &message,
        &["Required", EMAIL, HEADER, READOUT],
        &[CUSTOM_REQUIRED, HELP, PROJECT, "sk-live"],
    );

    let app = page(
        "s02-required-custom",
        Some("inputs.required_message=custom"),
        120,
        40,
    );
    app.wait_text(PROJECT);
    commit_empty(&app);
    let custom = app.wait_text(CUSTOM_REQUIRED);
    expect(
        &custom,
        &[CUSTOM_REQUIRED, EMAIL, HEADER, READOUT, OPTIONAL, PRESET],
        &["Required", HELP, PROJECT, "sk-live", "Page options"],
    );
    see(
        &app,
        "opt.inputs.required_message",
        "inputs.required_message=custom shows Name the project to continue after an empty commit",
    );
}

fn reveal_secret(app: &App) {
    for _ in 0..6 {
        app.key("tab");
        let screen = app.settle();
        if !text(&screen).contains("c1f2") && footer(&screen).contains("Apply") {
            return;
        }
    }
    let screen = app.screen();
    assert!(
        !text(&screen).contains("c1f2"),
        "editing the API key hides the reveal tail\n{}",
        text(&screen)
    );
}

#[test]
fn edit_checkpoint() {
    let app = page("s02-inputs-edit", None, 120, 40);
    app.wait_text(PROJECT);
    focus_project(&app, "Apply");
    let before = app.screen();
    expect(
        &before,
        &[HEADER, PROJECT, READOUT, OPTIONAL, PRESET, "c1f2", EMAIL],
        &["Reverted", "saved", "sk-live", CUSTOM_REQUIRED, "QQ"],
    );
    assert!(
        footer(&before).contains("Apply") && footer(&before).contains("Ctrl+U"),
        "immediate edit footer, got {}",
        footer(&before)
    );
    assert!(
        focus_line(&before).contains(PROJECT),
        "edit starts on the project value: {}",
        focus_line(&before)
    );
    see(
        &app,
        "sc.inputs.edit.before",
        "project field focused on payments-gateway before the edit sequence",
    );

    app.type_text("Z");
    let replaced = app.wait_screen("replaced", |screen| {
        let line = focus_line(screen);
        line.contains('Z') && !line.contains(PROJECT)
    });
    assert!(
        !focus_line(&replaced).contains(PROJECT),
        "immediate entry replaces the select-all value"
    );
    app.key("escape");
    let reverted = app.wait_text("Reverted");
    expect(&reverted, &["Reverted", PROJECT], &["QQ", "saved"]);
    assert!(
        footer(&reverted).contains("Apply"),
        "escape resumes immediate editing, got {}",
        footer(&reverted)
    );
    assert!(
        focus_line(&reverted).contains(PROJECT) && !focus_line(&reverted).contains('Z'),
        "escape restores payments-gateway: {}",
        focus_line(&reverted)
    );
    // Resume after Esc leaves the caret at the end. A click places the caret
    // too, so Shift+Tab from the branch field is what selects all again.
    click_content(&app, "feat/");
    app.key("shift+tab");
    app.wait_screen("reselected", |screen| {
        footer(screen).contains("Apply") && focus_line(screen).contains(PROJECT)
    });

    app.type_text("alpha beta");
    app.wait_screen("two words", |screen| {
        let line = focus_line(screen);
        line.contains("beta") && !line.contains(PROJECT)
    });
    app.key("ctrl+z");
    let undone = app.wait_screen("undo beta", |screen| {
        let line = focus_line(screen);
        line.contains("alpha") && !line.contains("beta")
    });
    assert!(
        !focus_line(&undone).contains("beta"),
        "one undo removes the second typing step"
    );
    assert!(
        focus_line(&undone).contains("alpha"),
        "one undo keeps the first word: {}",
        focus_line(&undone)
    );
    app.key("ctrl+shift+z");
    let redone = app.wait_screen("redo beta", |screen| focus_line(screen).contains("beta"));
    assert!(
        focus_line(&redone).contains("alpha") && focus_line(&redone).contains("beta"),
        "ctrl+shift+z restores beta: {}",
        focus_line(&redone)
    );
    app.paste("PASTE");
    let pasted = app.wait_text("PASTE");
    expect(&pasted, &["alpha", "PASTE"], &[PROJECT, "sk-live"]);

    click_content(&app, "feat/");
    let committed = app.wait_text("Project name saved");
    expect(
        &committed,
        &["Project name saved", "PASTE", "alpha"],
        &["Reverted", PROJECT, "sk-live"],
    );

    click_content(&app, "alpha");
    app.wait_screen("project again", |screen| {
        focus_line(screen).contains("alpha") && footer(screen).contains("Apply")
    });
    // A click leaves the caret on the clicked glyph. Ctrl+U deletes back to
    // the line start, so select-all first (Emacs Ctrl+L) and then clear.
    app.key("ctrl+l");
    app.key("ctrl+u");
    app.wait_screen("cleared for validation", |screen| {
        let line = focus_line(screen);
        !line.contains("alpha") && !line.contains("PASTE")
    });
    app.key("enter");
    let required = app.wait_text("Required");
    expect(
        &required,
        &["Required", EMAIL],
        &[CUSTOM_REQUIRED, HELP, "sk-live", PROJECT],
    );
    reveal_secret(&app);
    let after = app.screen();
    expect(
        &after,
        &[HEADER, "Required", EMAIL],
        &["sk-live", "c1f2", PROJECT, CUSTOM_REQUIRED],
    );
    assert!(
        footer(&after).contains("Apply"),
        "masked API key is editing, got {}",
        footer(&after)
    );
    see(
        &app,
        "sc.inputs.edit.after",
        "empty project shows Required and the editing API key hides sk-live and its tail",
    );

    let explicit = page(
        "s02-edit-explicit",
        Some("inputs.edit_entry=explicit"),
        120,
        40,
    );
    explicit.wait_text(PROJECT);
    focus_project(&explicit, "Edit");
    explicit.type_text("Z");
    let held = explicit.settle();
    assert!(
        focus_line(&held).contains(PROJECT) && !focus_line(&held).contains('Z'),
        "explicit entry ignores Z: {}",
        focus_line(&held)
    );
    explicit.key("enter");
    explicit.wait_screen("explicit editing", |screen| {
        footer(screen).contains("Apply")
    });
    explicit.type_text("Z");
    let appended = explicit.wait_screen("explicit append", |screen| {
        focus_line(screen).contains("payments-gatewayZ")
    });
    assert!(
        focus_line(&appended).contains("payments-gatewayZ"),
        "explicit Enter appends without select-all: {}",
        focus_line(&appended)
    );
    assert!(
        !focus_line(&appended).contains("Zpayments"),
        "forbidden caret-start insert on explicit begin"
    );

    let typed = page(
        "s02-edit-type",
        Some("inputs.edit_entry=type-to-edit"),
        120,
        40,
    );
    typed.wait_text(PROJECT);
    focus_project(&typed, "Edit");
    typed.type_text("Z");
    let appended = typed.wait_screen("type to edit", |screen| {
        focus_line(screen).contains("payments-gatewayZ")
    });
    assert!(
        focus_line(&appended).contains("payments-gatewayZ")
            && !text(&appended).contains("Zpayments-gateway"),
        "type-to-edit appends the first character: {}",
        focus_line(&appended)
    );

    let keep = page("s02-edit-keep", Some("inputs.blur_action=keep"), 120, 40);
    keep.wait_text(PROJECT);
    focus_project(&keep, "Apply");
    keep.type_text("QQ");
    keep.wait_screen("keep typed", |screen| focus_line(screen).contains("QQ"));
    click_content(&keep, "feat/");
    let kept = keep.settle();
    expect(
        &kept,
        &["QQ", HEADER],
        &["Reverted", "saved", PROJECT, "sk-live"],
    );
}

const OWNER: &str = "mira@example";
const OWNER_LONG: &str = "Use at most 24 characters";

fn focus_owner(app: &App) {
    focus_project(app, "Apply");
    app.key("enter");
    app.key("tab");
    app.key("tab");
    app.wait_screen("owner field", |screen| focus_line(screen).contains(OWNER));
}

#[test]
fn option_owner_rule_required_precedes_length_validator() {
    let app = page(
        "s02-owner-rule",
        Some("inputs.owner_rule=max-length"),
        120,
        40,
    );
    app.wait_text(OWNER);
    focus_owner(&app);
    app.key("ctrl+u");
    app.key("enter");
    let required = app.wait_text("Required");
    expect(&required, &["Required"], &[OWNER_LONG, EMAIL]);

    app.key("enter");
    app.type_text("a-very-long-owner-address@example.com");
    app.key("enter");
    let long = app.wait_text(OWNER_LONG);
    expect(&long, &[OWNER_LONG], &["Required", EMAIL]);
}
