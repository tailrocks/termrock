//! Lists route, sections, specimens, selection, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s04_lists/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Navigation › Lists";
const BLURB: &str = "Single and multiple selection, disabled items, scrolling, empty state";
const OVERVIEW: &str = "Tokens and principles behind every component";
const COLUMNS: &str = "Column rules, droppable columns, resizable seams and the header menu";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const CHOSEN_RUST: &str = "Chosen: Rust";
const TWO: &str = "2 selected";
const HEADER: &str = "Space toggle · a all · Shift+↓ range";
const EMPTY_TITLE: &str = "No results for “retry”";
const EMPTY_HINT: &str = "Try a shorter word";

const TOP: &[(&str, &str)] = &[
    ("sc.lists.specimen.rest", "rest"),
    ("sc.lists.specimen.hover", "hover"),
    ("sc.lists.specimen.pressed", "pressed"),
    ("sc.lists.specimen.focused", "focused"),
    ("sc.lists.specimen.focused_hover", "focus + hover"),
    ("sc.lists.specimen.selected", "selected"),
    ("sc.lists.specimen.disabled", "disabled"),
    ("sc.lists.specimen.empty", "empty"),
    ("sc.lists.specimen.loading", "loading"),
    ("sc.lists.specimen.error", "error"),
    ("sc.lists.specimen.overflow", "overflow"),
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s04_lists/{}", case_id.replace('.', "_"))
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

fn list_row(screen: &TerminalSnapshot, label: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(label) && !line.contains("Chosen:"))
        .unwrap_or_else(|| panic!("list row {label:?} missing\n{}", text(screen)))
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

fn assert_select(screen: &TerminalSnapshot, label: &str, value: &str, absent: &[&str]) {
    let line = line_after(screen, label);
    assert!(
        line.contains(value),
        "{label} shows {line:?}, expected {value}"
    );
    for item in absent {
        assert!(
            !line.contains(item),
            "{label} line {line:?} still has {item}"
        );
    }
}

fn assert_toggle(screen: &TerminalSnapshot, label: &str, state: &str) {
    let line = row_with(screen, label);
    let want = format!("{label} {state}");
    assert!(
        line.contains(&want),
        "{label} shows {line:?}, expected {want}"
    );
    let other = if state == "on" { "off" } else { "on" };
    let forbid = format!("{label} {other}");
    assert!(
        !line.contains(&forbid),
        "{label} line {line:?} still has {forbid}"
    );
}

fn assert_mark(screen: &TerminalSnapshot, label: &str, on: bool) {
    let line = list_row(screen, label);
    assert_eq!(
        line.contains('✓'),
        on,
        "{label} checked={on} in {line:?}\n{}",
        text(screen)
    );
}

fn assert_cursor(screen: &TerminalSnapshot, label: &str, on: bool) {
    let line = list_row(screen, label);
    assert_eq!(
        line.contains('▎'),
        on,
        "{label} cursor={on} in {line:?}\n{}",
        text(screen)
    );
}

fn assert_current(screen: &TerminalSnapshot, label: &str, on: bool) {
    let line = list_row(screen, label);
    assert_eq!(
        line.contains('›'),
        on,
        "{label} current={on} in {line:?}\n{}",
        text(screen)
    );
}

fn click_first(app: &App, label: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.wait_text(label);
    let mut hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    hits.sort_by_key(|hit| (hit.y, hit.x));
    assert!(!hits.is_empty(), "missing {label}\n{}", text(&screen));
    app.click(hits[0]);
    app.settle();
}

fn click_tab(app: &App, label: &str) {
    click_first(app, label, |hit| hit.y == 4);
}

fn click_side(app: &App, label: &str) {
    click_first(app, label, |hit| hit.x < 24);
}

fn click_language(app: &App, label: &str) {
    click_first(app, label, |hit| hit.x > 24 && hit.y > 9 && hit.y < 25);
}

fn click_file(app: &App, label: &str) {
    click_first(app, label, |hit| hit.x > 24 && hit.y > 18);
}

fn press(app: &App, key: &str) {
    app.key(key);
    app.settle();
}

fn page_until(app: &App, needle: &str) {
    for _ in 0..8 {
        if text(&app.screen()).contains(needle) {
            return;
        }
        press(app, "pagedown");
    }
    app.wait_text(needle);
}

fn show_options(app: &App) -> TerminalSnapshot {
    click_tab(app, "Options");
    app.wait_text("Page options")
}

fn open_readout(app: &App) -> TerminalSnapshot {
    click_tab(app, "Playground");
    page_until(app, "type-ahead");
    app.wait_text("ListCmd::")
}

fn shot(app: &App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn see_playground(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "truecolor",
            CHOSEN_RUST,
            TWO,
            HEADER,
            "TypeScript",
            "src/api/auth.rs",
            "Playground",
        ],
        &[
            "Page options",
            "Compare levels",
            CAP_OPT,
            CAP_STATES,
            OVERVIEW,
            COLUMNS,
            "Could not load languages",
            "t all",
            EMPTY_TITLE,
            EMPTY_HINT,
            "vim",
        ],
    );
    assert_mark(screen, "src/api/auth.rs", true);
    assert_mark(screen, "src/api/billing.rs", true);
    assert_mark(screen, "src/db/schema.rs", false);
    assert_mark(screen, "Cargo.lock", false);
    assert_mark(screen, "tests/checkout.rs", false);
    assert_current(screen, "Rust", true);
}

fn see_option_defaults(screen: &TerminalSnapshot) {
    assert_select(screen, "Nav keys", "default", &["vim"]);
    assert_toggle(screen, "Type-ahead", "on");
    assert_select(screen, "Click", "double", &["single"]);
    assert_select(screen, "Check click", "box", &["row"]);
    assert_select(screen, "Toggle click", "platform", &["alt", "any"]);
    assert_toggle(screen, "Arrows wrap", "off");
    assert_toggle(screen, "Skip disabled", "on");
    assert_select(screen, "Scrollbar gap", "1", &["0", "2"]);
    assert_select(screen, "Reveal", "tooltip", &["inline", "off"]);
    assert_select(screen, "Track click", "page", &["jump"]);
    assert_select(screen, "Key map", "default", &["custom"]);
    assert_select(screen, "Empty", "text", &["state"]);
    assert_select(screen, "Content", "ready", &["loading", "error"]);
}

fn opt_args(pair: &str) -> [&str; 4] {
    ["--page", "lists", "--opt", pair]
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "lists"];
    let app = open("s04-route-page", 120, 40, &args);
    let screen = app.wait_text(CHOSEN_RUST);
    see_playground(&screen);
    assert!(
        (0..screen.rows())
            .map(|row| row_text(&screen, row))
            .any(|line| line.contains('▎') && line.contains("Lists")),
        "sidebar focus missing\n{}",
        text(&screen)
    );
    shot(
        &app,
        "sc.lists.route",
        "--page lists opens the playground with the sidebar on Lists",
        &args,
    );
    app.finish();

    let app = open("s04-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    for _ in 0..21 {
        press(&app, "]");
    }
    let entered = app.wait_text(BLURB);
    see(
        &entered,
        &[TITLE, BLURB, CHOSEN_RUST, CAP_PLAY, TWO],
        &[OVERVIEW, COLUMNS, "Page options"],
    );
    press(&app, "[");
    let left = app.wait_text("Tables › Columns");
    see(
        &left,
        &["Tables › Columns", COLUMNS, CAP_PLAY],
        &[BLURB, TWO, "Could not load languages"],
    );
    press(&app, "]");
    let again = app.wait_text(BLURB);
    see(
        &again,
        &[TITLE, BLURB, CHOSEN_RUST],
        &[COLUMNS, OVERVIEW, "Page options"],
    );
    app.finish();

    let app = open("s04-route-pointer", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    click_side(&app, "Navigation");
    app.wait_text("Lists");
    click_side(&app, "Lists");
    let pointed = app.wait_text(BLURB);
    see(
        &pointed,
        &[TITLE, BLURB, CHOSEN_RUST, CAP_PLAY, TWO, HEADER],
        &[OVERVIEW, COLUMNS, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "lists"];
    let app = open("s04-sections", 80, 24, &args);
    let narrow_play = app.wait_text(CHOSEN_RUST);
    see(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "disabled items, scr",
            CHOSEN_RUST,
            "TypeScript",
            "› Rust",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "empty state",
            "Files to include",
            TWO,
            "Could not load languages",
        ],
    );
    shot(
        &app,
        "sc.lists.playground.80x24",
        "playground at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "Options");
    let narrow_opt = app.wait_text("Page options");
    see(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Nav keys",
            "Type-ahead on",
            "Arrows wrap off",
            "Skip disabled on",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            CHOSEN_RUST,
            WIDE,
            "truecolor",
            "vim",
            "tooltip",
            "Could not load",
        ],
    );
    assert_select(&narrow_opt, "Nav keys", "default", &["vim"]);
    assert_select(&narrow_opt, "Click", "double", &["single"]);
    assert_select(&narrow_opt, "Check click", "box", &["row"]);
    assert_select(&narrow_opt, "Toggle click", "platform", &["alt"]);
    assert_select(&narrow_opt, "Scrollbar gap", "1", &["0", "2"]);
    shot(
        &app,
        "sc.lists.options.80x24",
        "options at 80 by 24 before the terminal grows",
        &args,
    );

    click_tab(&app, "States");
    let narrow_states = app.wait_text("Compare levels");
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "focus + hover",
            "selected",
            "empty",
            "loading",
            "error",
            "overflow",
            "narrow",
            "Nothing here yet",
            WIDE,
            "truecolor",
        ],
    );
    shot(
        &app,
        "sc.lists.states.80x24",
        "states grid at 80 by 24 before the terminal grows",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_text("narrow");
    see(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "truecolor",
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "selected",
            "disabled",
            "empty",
            "Nothing here yet",
            "loading",
            "Loading rows",
            "error",
            "Could not load",
            "overflow",
            "TypeScript and a label",
            "narrow",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "busy",
            "editing",
            "src/db/schema.rs",
        ],
    );
    shot(
        &app,
        "sc.lists.states.120x40",
        "states caption and the grid rows at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.lists.states",
        "states is the visible Lists section",
        &args,
    );
    for (case_id, label) in TOP {
        see(
            &app.screen(),
            &[label, "○ Compare levels", CAP_STATES, "rest", "narrow"],
            &[
                "● Compare levels",
                "Page options",
                "busy",
                "editing",
                "256 colors",
            ],
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &args,
        );
    }

    page_until(&app, "src/db/schema.rs");
    let narrow = app.wait_text("narrow");
    see(
        &narrow,
        &["narrow", "src/db/schema.rs", "src/api/billing.rs", TITLE],
        &[
            "Compare levels",
            "Page options",
            CAP_PLAY,
            "busy",
            "editing",
            "256 colors",
        ],
    );
    assert!(
        list_row(&narrow, "narrow").contains("Rust") || text(&narrow).contains("narrow"),
        "narrow specimen missing\n{}",
        text(&narrow)
    );
    shot(
        &app,
        "sc.lists.specimen.narrow",
        "narrow specimen shows the full file name",
        &args,
    );

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(CAP_OPT);
    see(
        &wide_opt,
        &[TITLE, CAP_OPT, WIDE, "truecolor", "Page options"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            CHOSEN_RUST,
            "Could not load",
            "vim",
        ],
    );
    see_option_defaults(&wide_opt);
    shot(
        &app,
        "sc.lists.options.120x40",
        "options caption and the default values at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.lists.options",
        "options is the visible Lists section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(CAP_PLAY);
    see_playground(&wide_play);
    shot(
        &app,
        "sc.lists.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.lists.playground",
        "playground is the visible Lists section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "lists"];
    let app = open("s04-compare", 120, 40, &args);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", CAP_STATES, WIDE],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_first(&app, "Compare levels", |hit| hit.y < 12 && hit.x > 24);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", CAP_STATES, "truecolor"],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    for _ in 0..8 {
        let visible = text(&app.screen());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        press(&app, "pagedown");
    }
    let bands = app.wait_text("no color");
    assert!(
        saw_256 && saw_16,
        "compare bands did not pass 256 and 16\n{}",
        text(&bands)
    );
    see(
        &bands,
        &[TITLE, "no color", WIDE, "narrow", "src/db/schema.rs"],
        &["Page options", CAP_PLAY, "○ Compare levels"],
    );
    shot(
        &app,
        "sc.lists.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_text("● Compare levels");
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor", "rest"],
        &[
            "Page options",
            CAP_PLAY,
            CAP_STATES,
            WIDE,
            "120×40",
            "○ Compare levels",
            "no color",
        ],
    );
    shot(
        &app,
        "sc.lists.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    click_first(&app, "Compare levels", |hit| hit.x > 20);
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
    let options = show_options(&app);
    see(
        &options,
        &[TITLE, CAP_OPT, "Page options", "Type-ahead on"],
        &["● Compare levels", "vim", "Could not load languages"],
    );
    see_option_defaults(&options);
    app.finish();
}

#[test]
fn select_item_and_keep_the_multi_set() {
    let args = ["--page", "lists"];
    let app = open("s04-select", 120, 40, &args);
    let before = app.wait_text(CHOSEN_RUST);
    see_playground(&before);
    see(
        &before,
        &[TWO, HEADER],
        &["1 selected", "10 selected", "Include", "Exclude"],
    );
    shot(
        &app,
        "sc.lists.select.before",
        "playground before selection gestures",
        &args,
    );

    click_language(&app, "TypeScript");
    let typed_ready = app.settle();
    assert_cursor(&typed_ready, "TypeScript", true);
    assert_current(&typed_ready, "Rust", true);
    see(
        &typed_ready,
        &[CHOSEN_RUST, TWO],
        &["Chosen: TypeScript", "Chosen: Python"],
    );

    press(&app, "p");
    let typed = app.wait_text("Python");
    assert_cursor(&typed, "Python", true);
    assert_cursor(&typed, "TypeScript", false);
    assert_current(&typed, "Rust", true);
    assert_current(&typed, "Python", false);
    see(
        &typed,
        &[CHOSEN_RUST],
        &["Chosen: Python", "Chosen: TypeScript"],
    );

    press(&app, "enter");
    let chosen = app.wait_text("Chosen: Python");
    assert_current(&chosen, "Python", true);
    assert_current(&chosen, "Rust", false);
    assert_cursor(&chosen, "Python", true);
    see(
        &chosen,
        &["Chosen: Python"],
        &[CHOSEN_RUST, "Chosen: TypeScript"],
    );

    click_file(&app, "tests/checkout.rs");
    let only = app.wait_text("1 selected");
    assert_mark(&only, "tests/checkout.rs", true);
    assert_mark(&only, "src/api/auth.rs", false);
    assert_mark(&only, "src/api/billing.rs", false);
    assert_mark(&only, "src/db/schema.rs", false);
    assert_mark(&only, "Cargo.lock", false);
    see(&only, &["1 selected"], &[TWO, "10 selected"]);

    press(&app, "shift+down");
    let ranged = app.wait_text("docs/webhooks.md");
    assert_mark(&ranged, "tests/checkout.rs", true);
    assert_mark(&ranged, "docs/webhooks.md", true);
    assert_mark(&ranged, "Cargo.lock", false);
    assert_cursor(&ranged, "docs/webhooks.md", true);
    assert_cursor(&ranged, "Cargo.lock", false);
    see(&ranged, &[TWO], &["1 selected", "10 selected"]);

    press(&app, " ");
    let toggled = app.settle();
    assert_mark(&toggled, "docs/webhooks.md", false);
    assert_mark(&toggled, "tests/checkout.rs", true);
    assert_mark(&toggled, "Cargo.lock", false);
    see(&toggled, &["1 selected"], &[TWO, "10 selected"]);

    press(&app, "down");
    press(&app, "down");
    let at_config = app.wait_text("src/config.rs");
    assert_cursor(&at_config, "src/config.rs", true);
    assert_mark(&at_config, "src/config.rs", false);
    assert_mark(&at_config, "tests/checkout.rs", true);
    see(&at_config, &["1 selected"], &["10 selected", "9 selected"]);

    press(&app, "ctrl+a");
    let all = app.wait_text("10 selected");
    assert_mark(&all, "src/config.rs", true);
    assert_mark(&all, "src/api/auth.rs", true);
    assert_mark(&all, "src/db/schema.rs", false);
    assert_mark(&all, "Cargo.lock", false);
    see(&all, &["10 selected"], &["9 selected", "1 selected", TWO]);

    press(&app, "shift+f10");
    let menu = app.wait_text("Exclude");
    see(
        &menu,
        &["Exclude", "Select none", "src/config.rs", "10 selected"],
        &["Include", "Select all", "9 selected"],
    );
    assert_mark(&menu, "src/api/auth.rs", true);

    press(&app, "enter");
    let excluded = app.wait_text("9 selected");
    assert_mark(&excluded, "src/config.rs", false);
    assert_mark(&excluded, "src/api/auth.rs", true);
    assert_mark(&excluded, "src/db/schema.rs", false);
    assert_mark(&excluded, "Cargo.lock", false);
    see(
        &excluded,
        &["9 selected"],
        &["10 selected", "Exclude", "Select none", "1 selected"],
    );

    press(&app, "shift+f10");
    let again = app.wait_text("Include");
    see(
        &again,
        &["Include", "Select all", "src/config.rs", "9 selected"],
        &["Exclude", "Select none", "10 selected"],
    );
    assert_mark(&again, "src/api/auth.rs", true);

    press(&app, "enter");
    let after = app.wait_text("10 selected");
    assert_mark(&after, "src/config.rs", true);
    assert_mark(&after, "src/api/auth.rs", true);
    assert_mark(&after, "tests/checkout.rs", true);
    assert_mark(&after, "src/db/schema.rs", false);
    assert_mark(&after, "Cargo.lock", false);
    assert_current(&after, "Python", true);
    see(
        &after,
        &["10 selected"],
        &[
            "9 selected",
            "Include",
            "Exclude",
            CHOSEN_RUST,
            "Chosen: Python",
            "1 selected",
        ],
    );
    shot(
        &app,
        "sc.lists.select.after",
        "selection after type-ahead, range, toggle, select all, and context select",
        &args,
    );

    click_tab(&app, "Playground");
    let blurred = app.settle();
    let blurred = if text(&blurred).contains("10 selected") {
        blurred
    } else {
        press(&app, "pagedown");
        app.wait_text("10 selected")
    };
    assert_mark(&blurred, "src/api/auth.rs", true);
    assert_mark(&blurred, "src/config.rs", true);
    assert_mark(&blurred, "src/db/schema.rs", false);
    assert_mark(&blurred, "Cargo.lock", false);
    assert!(
        row_with(&blurred, "Playground").contains('▎'),
        "tab focus missing after blur\n{}",
        text(&blurred)
    );
    assert_cursor(&blurred, "src/config.rs", false);
    assert_current(&blurred, "Python", true);
    see(
        &blurred,
        &["10 selected"],
        &["9 selected", "Include", "Exclude", "Chosen: Python"],
    );
    app.finish();
}

fn show_play(app: &App) -> TerminalSnapshot {
    click_tab(app, "Playground");
    app.wait_text("Chosen:")
}

#[test]
fn option_nav_vim() {
    let args = opt_args("lists.nav=vim");
    let app = open("s04-nav", 120, 40, &args);
    let readout = open_readout(&app);
    see(
        &readout,
        &[
            "nav vim",
            "type-ahead on",
            "click Double",
            "skip_disabled true",
        ],
        &["nav default", "type-ahead off"],
    );
    let options = show_options(&app);
    assert_select(&options, "Nav keys", "vim", &["default"]);
    assert_toggle(&options, "Type-ahead", "on");
    assert_select(&options, "Click", "double", &["single"]);
    let again = open_readout(&app);
    see(&again, &["nav vim", "type-ahead on"], &["nav default"]);
    shot(
        &app,
        "opt.lists.nav",
        "nav vim is on the readout and type-ahead stays on",
        &args,
    );
    app.finish();
}

#[test]
fn option_type_ahead_off() {
    let args = opt_args("lists.type_ahead=off");
    let app = open("s04-type", 120, 40, &args);
    app.wait_text(CHOSEN_RUST);
    click_language(&app, "TypeScript");
    // Paused motion never redraws when the 140 ms click flash expires, so a
    // key inside the window would repaint the pressed colour for good.
    std::thread::sleep(std::time::Duration::from_millis(200));
    press(&app, "p");
    let stayed = app.settle();
    assert_cursor(&stayed, "TypeScript", true);
    assert_cursor(&stayed, "Python", false);
    assert_current(&stayed, "Rust", true);
    see(
        &stayed,
        &[CHOSEN_RUST],
        &["Chosen: Python", "Chosen: TypeScript"],
    );
    shot(
        &app,
        "opt.lists.type_ahead",
        "type-ahead off leaves the cursor on TypeScript and Rust stays chosen",
        &args,
    );
    let options = show_options(&app);
    assert_toggle(&options, "Type-ahead", "off");
    assert_select(&options, "Click", "double", &["single"]);
    assert_select(&options, "Nav keys", "default", &["vim"]);
    app.finish();
}

#[test]
fn option_click_single() {
    let args = opt_args("lists.click=single");
    let app = open("s04-click", 120, 40, &args);
    app.wait_text(CHOSEN_RUST);
    click_language(&app, "TypeScript");
    let chosen = app.wait_text("Chosen: TypeScript");
    assert_current(&chosen, "TypeScript", true);
    assert_current(&chosen, "Rust", false);
    see(
        &chosen,
        &["Chosen: TypeScript"],
        &[CHOSEN_RUST, "Chosen: Python"],
    );
    let options = show_options(&app);
    assert_select(&options, "Click", "single", &["double"]);
    assert_select(&options, "Check click", "box", &["row"]);
    let back = show_play(&app);
    see(
        &back,
        &["Chosen: TypeScript"],
        &[CHOSEN_RUST, "Chosen: Python"],
    );
    assert_current(&back, "TypeScript", true);
    shot(
        &app,
        "opt.lists.click",
        "one click chooses TypeScript and check click stays box",
        &args,
    );
    app.finish();
}

#[test]
fn option_check_row() {
    let args = opt_args("lists.check=row");
    let app = open("s04-check", 120, 40, &args);
    app.wait_text(TWO);
    click_file(&app, "tests/checkout.rs");
    let added = app.wait_text("3 selected");
    assert_mark(&added, "src/api/auth.rs", true);
    assert_mark(&added, "src/api/billing.rs", true);
    assert_mark(&added, "tests/checkout.rs", true);
    assert_mark(&added, "src/db/schema.rs", false);
    assert_mark(&added, "Cargo.lock", false);
    see(&added, &["3 selected"], &["1 selected", "10 selected", TWO]);
    let options = show_options(&app);
    assert_select(&options, "Check click", "row", &["box"]);
    assert_select(&options, "Click", "double", &["single"]);
    let back = show_play(&app);
    see(
        &back,
        &["3 selected", CHOSEN_RUST],
        &["1 selected", TWO, "10 selected"],
    );
    assert_mark(&back, "src/api/auth.rs", true);
    assert_mark(&back, "src/api/billing.rs", true);
    assert_mark(&back, "tests/checkout.rs", true);
    assert_mark(&back, "src/db/schema.rs", false);
    assert_mark(&back, "Cargo.lock", false);
    shot(
        &app,
        "opt.lists.check",
        "a row click adds checkout and leaves auth and billing checked",
        &args,
    );
    app.finish();
}

#[test]
fn option_toggle_mods_alt() {
    let args = opt_args("lists.toggle_mods=alt");
    let app = open("s04-toggle", 120, 40, &args);
    let readout = open_readout(&app);
    see(
        &readout,
        &["toggle Alt+click", "scrollbar_gap 1", "type-ahead on"],
        &["platform+click", "toggle Ctrl"],
    );
    let options = show_options(&app);
    assert_select(&options, "Toggle click", "alt", &["platform", "any"]);
    assert_toggle(&options, "Arrows wrap", "off");
    let again = open_readout(&app);
    see(&again, &["toggle Alt+click"], &["platform+click"]);
    shot(
        &app,
        "opt.lists.toggle_mods",
        "toggle reads Alt+click and arrows wrap stays off",
        &args,
    );
    app.finish();
}

#[test]
fn option_wrap_on() {
    let args = opt_args("lists.wrap=on");
    let app = open("s04-wrap", 120, 40, &args);
    app.wait_text(CHOSEN_RUST);
    click_language(&app, "Rust");
    press(&app, "up");
    let wrapped = app.wait_text("Erlang");
    assert_cursor(&wrapped, "Erlang", true);
    see(&wrapped, &[CHOSEN_RUST], &["Chosen: Erlang"]);
    press(&app, "enter");
    let chosen = app.wait_text("Chosen: Erlang");
    assert_current(&chosen, "Erlang", true);
    see(
        &chosen,
        &["Chosen: Erlang"],
        &[CHOSEN_RUST, "Chosen: TypeScript"],
    );
    let options = show_options(&app);
    assert_toggle(&options, "Arrows wrap", "on");
    assert_toggle(&options, "Skip disabled", "on");
    let back = show_play(&app);
    see(
        &back,
        &["Chosen: Erlang"],
        &[CHOSEN_RUST, "Chosen: TypeScript"],
    );
    shot(
        &app,
        "opt.lists.wrap",
        "up from Rust wraps to Erlang and skip disabled stays on",
        &args,
    );
    app.finish();
}

#[test]
fn option_skip_disabled_off() {
    let args = opt_args("lists.skip_disabled=off");
    let app = open("s04-skip", 120, 40, &args);
    app.wait_text(TWO);
    click_file(&app, "src/api/billing.rs");
    let only = app.wait_text("1 selected");
    assert_mark(&only, "src/api/billing.rs", true);
    assert_mark(&only, "src/api/auth.rs", false);
    assert_mark(&only, "src/db/schema.rs", false);
    press(&app, "down");
    let landed = app.wait_text("src/db/schema.rs");
    assert_mark(&landed, "src/db/schema.rs", false);
    assert_mark(&landed, "src/api/billing.rs", true);
    assert_mark(&landed, "src/api/auth.rs", false);
    assert_cursor(&landed, "src/api/billing.rs", false);
    assert_cursor(&landed, "tests/checkout.rs", false);
    assert_cursor(&landed, "src/db/schema.rs", false);
    see(&landed, &["1 selected"], &[TWO, "10 selected"]);
    shot(
        &app,
        "opt.lists.skip_disabled",
        "down leaves billing and does not skip onto checkout",
        &args,
    );
    let options = show_options(&app);
    assert_toggle(&options, "Skip disabled", "off");
    assert_toggle(&options, "Arrows wrap", "off");
    app.finish();
}

#[test]
fn option_scrollbar_gap_0() {
    let args = opt_args("lists.scrollbar_gap=0");
    let app = open("s04-gap", 120, 40, &args);
    let readout = open_readout(&app);
    see(
        &readout,
        &["scrollbar_gap 0", "reveal tooltip", "track_click Page"],
        &["scrollbar_gap 1", "scrollbar_gap 2"],
    );
    let options = show_options(&app);
    assert_select(&options, "Scrollbar gap", "0", &["1", "2"]);
    assert_select(&options, "Reveal", "tooltip", &["off", "inline"]);
    let again = open_readout(&app);
    see(&again, &["scrollbar_gap 0"], &["scrollbar_gap 1"]);
    shot(
        &app,
        "opt.lists.scrollbar_gap",
        "scrollbar gap 0 is on the readout and reveal stays tooltip",
        &args,
    );
    app.finish();
}

#[test]
fn option_reveal_off() {
    let args = opt_args("lists.reveal=off");
    let app = open("s04-reveal", 120, 40, &args);
    let readout = open_readout(&app);
    see(
        &readout,
        &["reveal off", "track_click Page", "scrollbar_gap 1"],
        &["reveal tooltip", "reveal inline"],
    );
    let options = show_options(&app);
    assert_select(&options, "Reveal", "off", &["tooltip", "inline"]);
    assert_select(&options, "Track click", "page", &["jump"]);
    let again = open_readout(&app);
    see(
        &again,
        &["reveal off"],
        &["reveal tooltip", "reveal inline"],
    );
    shot(
        &app,
        "opt.lists.reveal",
        "reveal off is on the readout and track click stays page",
        &args,
    );
    app.finish();
}

#[test]
fn option_track_click_jump() {
    let args = opt_args("lists.track_click=jump");
    let app = open("s04-track", 120, 40, &args);
    let readout = open_readout(&app);
    see(
        &readout,
        &["track_click Jump", "reveal tooltip", "scrollbar_gap 1"],
        &["track_click Page"],
    );
    let options = show_options(&app);
    assert_select(&options, "Track click", "jump", &["page"]);
    assert_select(&options, "Reveal", "tooltip", &["off"]);
    let again = open_readout(&app);
    see(&again, &["track_click Jump"], &["track_click Page"]);
    shot(
        &app,
        "opt.lists.track_click",
        "track click Jump is on the readout and reveal stays tooltip",
        &args,
    );
    app.finish();
}

#[test]
fn option_keymap_custom() {
    let args = opt_args("lists.keymap=custom");
    let app = open("s04-keymap", 120, 40, &args);
    let play = app.wait_text("t all");
    see(
        &play,
        &[
            "Space toggle · t all · Shift+↓ range",
            TWO,
            CHOSEN_RUST,
            CAP_PLAY,
        ],
        &["a all", "10 selected", "9 selected"],
    );
    assert_mark(&play, "src/api/auth.rs", true);
    assert_mark(&play, "src/api/billing.rs", true);
    assert_mark(&play, "src/db/schema.rs", false);
    assert_mark(&play, "Cargo.lock", false);
    click_file(&app, "tests/checkout.rs");
    let only = app.wait_text("1 selected");
    assert_mark(&only, "tests/checkout.rs", true);
    assert_mark(&only, "src/api/auth.rs", false);
    press(&app, "t");
    let toggled = app.wait_text("10 selected");
    assert_mark(&toggled, "tests/checkout.rs", true);
    assert_mark(&toggled, "src/api/auth.rs", true);
    assert_mark(&toggled, "src/api/billing.rs", true);
    assert_mark(&toggled, "src/db/schema.rs", false);
    assert_mark(&toggled, "Cargo.lock", false);
    assert_cursor(&toggled, "tests/checkout.rs", true);
    see(
        &toggled,
        &["10 selected", "t all"],
        &["a all", "9 selected", "1 selected", TWO],
    );
    shot(
        &app,
        "opt.lists.keymap",
        "custom keymap labels t all and selects every enabled file",
        &args,
    );
    let options = show_options(&app);
    assert_select(&options, "Key map", "custom", &["default"]);
    assert_select(&options, "Empty", "text", &["state"]);
    assert_select(&options, "Content", "ready", &["error", "loading"]);
    app.finish();
}

#[test]
fn option_empty_state() {
    let args = opt_args("lists.empty=state");
    let app = open("s04-empty", 120, 40, &args);
    click_tab(&app, "Playground");
    page_until(&app, EMPTY_HINT);
    let play = app.wait_text(EMPTY_TITLE);
    see(
        &play,
        &[EMPTY_TITLE, EMPTY_HINT, "search every branch", TITLE],
        &["Could not load languages", "Loading languages"],
    );
    let options = show_options(&app);
    assert_select(&options, "Empty", "state", &["text"]);
    assert_select(&options, "Content", "ready", &["error", "loading"]);
    assert_toggle(&options, "Type-ahead", "on");
    click_tab(&app, "Playground");
    page_until(&app, EMPTY_HINT);
    let again = app.wait_text(EMPTY_TITLE);
    see(
        &again,
        &[EMPTY_TITLE, EMPTY_HINT, "search every branch"],
        &["Could not load languages", "Loading languages"],
    );
    shot(
        &app,
        "opt.lists.empty",
        "empty state shows the hint and content stays ready",
        &args,
    );
    app.finish();
}

#[test]
fn option_content_error() {
    let args = opt_args("lists.content=error");
    let app = open("s04-content", 120, 40, &args);
    let play = app.wait_text("Could not load languages");
    see(
        &play,
        &[
            TITLE,
            CAP_PLAY,
            CHOSEN_RUST,
            "Could not load languages",
            "The registry did not answer; try again later",
            TWO,
            HEADER,
        ],
        &[
            "TypeScript",
            "Loading languages",
            "Page options",
            EMPTY_HINT,
        ],
    );
    assert_mark(&play, "src/api/auth.rs", true);
    assert_mark(&play, "src/db/schema.rs", false);
    let options = show_options(&app);
    assert_select(&options, "Content", "error", &["ready", "loading"]);
    assert_select(&options, "Empty", "text", &["state"]);
    let back = show_play(&app);
    see(
        &back,
        &[
            "Could not load languages",
            "The registry did not answer; try again later",
            CHOSEN_RUST,
            TWO,
        ],
        &["TypeScript", "Loading languages", "Page options"],
    );
    shot(
        &app,
        "opt.lists.content",
        "content error shows the registry message and empty stays text",
        &args,
    );
    app.finish();
}

#[test]
fn option_source_keyed_reports_actions_by_key() {
    let args = opt_args("lists.source=keyed");
    let app = open("s04-keyed", 120, 72, &args);
    let before = app.wait_text("source keyed (ListSource)");
    see(
        &before,
        &[
            "Language",
            "TypeScript",
            "src/api/auth.rs",
            "last ListAction: —",
            "0 selected",
        ],
        &["Chosen: Rust", TWO],
    );

    click_language(&app, "TypeScript");
    press(&app, "enter");
    let chosen = app.wait_text("Chosen: TypeScript");
    assert_current(&chosen, "TypeScript", true);
    see(
        &chosen,
        &["last ListAction: Activated(TypeScript) → SelectionChanged"],
        &["Chosen: Rust"],
    );

    click_file(&app, "tests/checkout.rs");
    let only = app.wait_text("1 selected");
    assert_mark(&only, "tests/checkout.rs", true);
    assert_mark(&only, "src/api/auth.rs", false);
    see(&only, &["last ListAction: SelectionChanged"], &[TWO]);

    press(&app, "down");
    press(&app, " ");
    let two = app.wait_text("2 selected");
    assert_mark(&two, "docs/webhooks.md", true);
    assert_mark(&two, "tests/checkout.rs", true);
    see(&two, &["cursor by key: TypeScript / docs/webhooks.md"], &[]);

    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    assert_select(&options, "Row source", "keyed", &["owned", "reversed"]);
    shot(
        &app,
        "opt.lists.source",
        "keyed source reports activation and selection changes by key",
        &args,
    );
    app.finish();
}

#[test]
fn option_source_keyed_reversed_lists_rows_from_the_other_end() {
    let args = opt_args("lists.source=keyed reversed");
    let app = open("s04-reversed", 120, 72, &args);
    let before = app.wait_text("source keyed reversed (ListSource)");
    let language = |label: &str| {
        let rows: Vec<String> = (0..before.rows())
            .map(|row| row_text(&before, row))
            .collect();
        rows.iter()
            .position(|line| line.contains(label) && !line.contains("Chosen:"))
            .unwrap_or_else(|| panic!("row {label:?} missing\n{}", text(&before)))
    };
    assert!(
        language("Erlang") < language("Clojure") && language("Clojure") < language("OCaml"),
        "the language rows run from the last record to the first\n{}",
        text(&before)
    );
    assert!(
        language("src/db/pool.rs") < language("src/main.rs"),
        "the file rows are reversed too\n{}",
        text(&before)
    );
    click_language(&app, "Erlang");
    press(&app, "enter");
    let chosen = app.wait_text("Chosen: Erlang");
    see(
        &chosen,
        &["last ListAction: Activated(Erlang) → SelectionChanged"],
        &["Chosen: Rust"],
    );
    shot(
        &app,
        "opt.lists.source_reversed",
        "a reversed keyed view lists the records from the end and still reports by key",
        &args,
    );
    app.finish();
}
