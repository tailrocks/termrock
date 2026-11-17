//! Focus & navigation route, sections, options, and the focus ring.
//!
//! Tuiscotti state ids allow only `[a-z0-9_-]` segments. `S01.focusnavigation`
//! and the dotted case ids are not legal segments, and `Path::set_extension`
//! would collapse those dotted ids onto one file. Each checkpoint is stored as
//! `pre_refactor/s01_focusnavigation/<case id with '.' replaced by '_'>`.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Focus & navigation";
const BLURB: &str = "The focus ring, fallback";
const OVERVIEW: &str = "Tokens and principles behind every component";
const PLAY: &str = "the live widget";
const OPT: &str = "one control per option";
const STATES: &str = "every state, drawn by the widget";
const MOVES: &str = "moves: request_focus_next";
const STAYS: &str = "stays: cancel_focus_request";
const OMITTED: &[&str] = &[
    "pressed", "selected", "disabled", "busy", "error", "editing", "empty", "loading", "overflow",
    "narrow",
];
const FORWARD: &[&str] = &[
    "section strip",
    "Name field",
    "Email field",
    "Save",
    "Cancel",
    "Alpha tag",
    "Beta tag",
    "Gamma tag",
    "Delta tag",
    "Mailbox list",
    "Passive tip button",
    "Restore on button",
    "Restore off button",
    "outside the page",
];
const BACK: &[&str] = &[
    "Restore off button",
    "Restore on button",
    "Passive tip button",
    "Mailbox list",
    "Delta tag",
    "Gamma tag",
    "Beta tag",
    "Alpha tag",
    "Cancel",
    "Save",
    "Email field",
    "Name field",
    "section strip",
    "outside the page",
];

fn launch(name: &str, args: &[&str], cols: u16, rows: u16) -> App {
    let mut spec = Launch::new(name, SHOWCASE);
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.cols = cols;
    spec.rows = rows;
    spec.color = Some("truecolor".to_string());
    spec.motion = Some("paused".to_string());
    try_launch(spec).unwrap_or_else(|error| panic!("{error}"))
}

fn open_page(name: &str, cols: u16, rows: u16) -> App {
    let app = launch(name, &["--page", "focusnavigation"], cols, rows);
    app.wait_text(TITLE);
    app
}

fn test_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s01_focusnavigation/{}",
        case_id.replace('.', "_")
    )
}

fn see(app: &App, case_id: &str, _precondition: &str) {
    app.visual(&test_id(case_id));
}

fn expect(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let raw = text(screen);
    for needle in present {
        assert!(
            raw.contains(needle),
            "visible result missing {needle:?}\n{raw}"
        );
    }
    for needle in absent {
        assert!(
            !raw.contains(needle),
            "forbidden effect still visible {needle:?}\n{raw}"
        );
    }
}

fn assert_size(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(screen.cols(), cols, "snapshot cols");
    assert_eq!(screen.rows(), rows, "snapshot rows");
}

fn gutters(screen: &TerminalSnapshot) -> usize {
    text(screen).matches('▎').count()
}

fn assert_one_gutter(screen: &TerminalSnapshot) {
    assert_eq!(
        gutters(screen),
        1,
        "forbidden second focus gutter\n{}",
        text(screen)
    );
}

fn focus_value(screen: &TerminalSnapshot) -> String {
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        let Some(idx) = line.find("Focus ") else {
            continue;
        };
        let rest = line[idx + "Focus ".len()..].trim();
        if rest.starts_with("ring") || rest.starts_with('&') || rest.starts_with("navigation") {
            continue;
        }
        let value = rest.split("  ").next().unwrap_or(rest).trim().to_string();
        if !value.is_empty() {
            return value;
        }
    }
    panic!("focus readout missing\n{}", text(screen));
}

fn header(screen: &TerminalSnapshot) -> String {
    row_text(screen, 0)
}

fn click_at(app: &App, needle: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.wait_text(needle);
    let hits: Vec<_> = find_all(&screen, needle)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {needle:?} in range, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_sidebar(app: &App, needle: &str) {
    click_at(app, needle, |hit| hit.x < 24);
}

fn row_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no row {needle:?}\n{}", text(screen)))
}

fn wait_states_off(app: &App, reason: &str) -> TerminalSnapshot {
    // The tab title returns before the focus + hover specimen is painted.
    app.wait_screen(reason, |screen| {
        let visible = text(screen);
        visible.contains("○ Compare levels") && visible.contains("focus + hover")
    })
}

fn expect_states(screen: &TerminalSnapshot, wide: bool) {
    assert_one_gutter(screen);
    let raw = text(screen);
    assert!(
        raw.lines()
            .any(|line| line.contains("rest") && line.contains("Save")),
        "rest row missing\n{raw}"
    );
    assert!(
        raw.lines()
            .any(|line| line.contains("hover") && !line.contains("focus") && line.contains("Save")),
        "hover row missing\n{raw}"
    );
    assert!(
        raw.lines()
            .any(|line| line.contains("focused") && line.contains("Save")),
        "focused row missing\n{raw}"
    );
    assert!(
        raw.contains("focus + hover"),
        "focus + hover row missing\n{raw}"
    );
    expect(
        screen,
        &[TITLE, "○ Compare levels", "Button", "Field", "Ada"],
        OMITTED,
    );
    expect(
        screen,
        &[],
        &[PLAY, OPT, "Page options", "● Compare levels"],
    );
    if wide {
        assert_size(screen, 120, 40);
        expect(
            screen,
            &[
                STATES,
                "Tab strip",
                "Data",
                "Logs",
                "depicts a focused sample; it is not where your focus is",
                "120×40",
            ],
            &[],
        );
    } else {
        assert_size(screen, 80, 24);
        expect(
            screen,
            &["depicts a focused sample"],
            &[STATES, PLAY, "120×40"],
        );
    }
}

fn step_focus(app: &App, key: &str, expected: &str) {
    app.key(key);
    let screen = app.settle();
    assert_eq!(
        focus_value(&screen),
        expected,
        "focus after {key}\n{}",
        text(&screen)
    );
    assert_one_gutter(&screen);
    assert!(
        header(&screen).contains(TITLE),
        "focus move left the page: {}",
        header(&screen)
    );
    expect(&screen, &["Ada Lovelace"], &["Save ✓", "Page options"]);
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let keys = launch("s01-focusnavigation-route-keys", &[], 120, 40);
    let overview = keys.wait_text(OVERVIEW);
    expect(&overview, &[OVERVIEW, PLAY], &[TITLE, "Ada Lovelace"]);
    assert!(
        header(&overview).contains("Overview") && !header(&overview).contains(TITLE),
        "overview header is not Focus & navigation: {}",
        header(&overview)
    );
    for _ in 0..6 {
        keys.key("]");
    }
    let entered = keys.wait_text("Ada Lovelace");
    expect(
        &entered,
        &[TITLE, BLURB, "Ada Lovelace", PLAY, "outside the page"],
        &[OVERVIEW, OPT, "Page options", STATES],
    );
    assert!(
        header(&entered).contains(TITLE),
        "keyboard ]]]]]] opens Focus & navigation: {}",
        header(&entered)
    );
    keys.key("[");
    let left = keys.wait_screen("previous page is Quit & leave", |screen| {
        let head = row_text(screen, 0);
        head.contains("Quit & leave") && !head.contains(TITLE)
    });
    expect(&left, &["Quit & leave"], &["Ada Lovelace", "Page options"]);
    keys.key("]");
    let again = keys.wait_text("Ada Lovelace");
    assert!(
        header(&again).contains(TITLE),
        "keyboard ] re-enters Focus & navigation: {}",
        header(&again)
    );
    keys.click_text("Overview");
    let back = keys.wait_text(OVERVIEW);
    assert!(
        header(&back).contains("Overview") && !header(&back).contains(TITLE),
        "sidebar Overview returns home: {}",
        header(&back)
    );
    expect(&back, &[OVERVIEW], &["Ada Lovelace"]);
    let sidebar = keys.screen();
    if !find_all(&sidebar, "Focus & nav")
        .iter()
        .any(|hit| hit.x < 24)
    {
        click_sidebar(&keys, "App shell");
        keys.wait_text("Chrome");
    }
    click_sidebar(&keys, "Focus & nav");
    let pointed = keys.wait_text("Ada Lovelace");
    expect(
        &pointed,
        &[TITLE, "Ada Lovelace", PLAY, "outside the page"],
        &[OVERVIEW, OPT, "Page options"],
    );
    assert!(
        header(&pointed).contains(TITLE),
        "pointer opens Focus & navigation: {}",
        header(&pointed)
    );
    assert_one_gutter(&pointed);
    keys.finish();

    let page = open_page("s01-focusnavigation-route-page", 120, 40);
    let screen = page.wait_text("Ada Lovelace");
    assert_size(&screen, 120, 40);
    expect(
        &screen,
        &[
            TITLE,
            BLURB,
            PLAY,
            "Ada Lovelace",
            "ada@example.com",
            "outside the page",
            MOVES,
            "120×40",
        ],
        &[OPT, STATES, "Page options", STAYS, "↑ k", "Save ✓"],
    );
    assert_one_gutter(&screen);
    assert!(
        header(&screen).contains(TITLE),
        "--page header: {}",
        header(&screen)
    );
    see(
        &page,
        "sc.focusnavigation.route",
        "--page focusnavigation opens the playground with the sidebar focus on Focus & navigation",
    );
    page.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open_page("s01-focusnavigation-sections", 80, 24);
    app.click_text("Playground");
    let narrow_play = app.wait_text("←→ Section");
    assert_size(&narrow_play, 80, 24);
    expect(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "Options",
            "States",
            "Ada Lovelace",
            "Save",
            "Focus ring",
            "←→ Section",
        ],
        &[
            PLAY,
            OPT,
            STATES,
            "Page options",
            "Compare levels",
            "120×40",
        ],
    );
    see(
        &app,
        "sc.focusnavigation.playground.80x24",
        "playground tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("Options");
    let narrow_opt = app.wait_text("Page options");
    assert_size(&narrow_opt, 80, 24);
    expect(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "List nav keys",
            "desktop",
            "Tab in a field",
            "moves",
        ],
        &[
            PLAY,
            OPT,
            STATES,
            "Compare levels",
            "Ada Lovelace",
            "stays",
            "vim",
        ],
    );
    see(
        &app,
        "sc.focusnavigation.options.80x24",
        "options tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("States");
    let narrow_states = wait_states_off(&app, "states off at 80 by 24");
    expect_states(&narrow_states, false);
    see(
        &app,
        "sc.focusnavigation.states.80x24",
        "states tab activated at 80 by 24, before the terminal grows",
    );

    app.resize(120, 40);
    let wide_states = app.wait_screen("states grown to 120 by 40", |screen| {
        let raw = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && raw.contains("120×40")
            && raw.contains("focus + hover")
            && raw.contains("Tab strip")
    });
    expect_states(&wide_states, true);
    see(
        &app,
        "sc.focusnavigation.states.120x40",
        "states tab still active after growing from 80 by 24 to 120 by 40",
    );

    app.click_text("Options");
    let wide_opt = app.wait_text(OPT);
    assert_size(&wide_opt, 120, 40);
    expect(
        &wide_opt,
        &[
            TITLE,
            OPT,
            "Page options",
            "List nav keys",
            "desktop",
            "Tab in a field",
            "moves",
            "120×40",
        ],
        &[
            PLAY,
            STATES,
            "Compare levels",
            "Ada Lovelace",
            "stays",
            "vim",
        ],
    );
    see(
        &app,
        "sc.focusnavigation.options.120x40",
        "options tab at 120 by 40 after the narrow capture",
    );

    app.click_text("Playground");
    let wide_play = app.wait_text(PLAY);
    assert_size(&wide_play, 120, 40);
    expect(
        &wide_play,
        &[
            TITLE,
            PLAY,
            "Ada Lovelace",
            "ada@example.com",
            MOVES,
            "Up",
            "120×40",
        ],
        &[OPT, STATES, "Page options", STAYS, "↑ k", "Home g"],
    );
    let up = row_with(&wide_play, "Up");
    assert!(
        up.contains('↑') && !up.contains('k'),
        "desktop Up row is arrows only: {up:?}"
    );
    see(
        &app,
        "sc.focusnavigation.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
    );
    app.finish();
}

#[test]
fn compare_levels_then_restore() {
    let app = open_page("s01-focusnavigation-compare", 120, 40);
    app.click_text("States");
    let off = wait_states_off(&app, "states off before compare");
    expect_states(&off, true);
    expect(
        &off,
        &["○ Compare levels"],
        &["256 colors", "no color", "●"],
    );
    app.click_text("Compare levels");
    let on = app.wait_text("256 colors");
    assert_one_gutter(&on);
    expect(
        &on,
        &[
            "● Compare levels",
            "truecolor",
            "256 colors",
            TITLE,
            "rest",
            "focus + hover",
        ],
        &["○ Compare levels", "Page options", "pressed"],
    );
    let anchor = find_all(&on, "Compare levels")
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("compare button missing\n{}", text(&on)));
    app.wheel(anchor, true, 4);
    let bands = app.wait_text("no color");
    assert_size(&bands, 120, 40);
    expect(
        &bands,
        &[
            "no color",
            "16 colors",
            "256 colors",
            TITLE,
            "rest",
            "focused",
        ],
        &["Page options", "○ Compare levels", "pressed"],
    );
    // The wheel leaves the terminal cursor on this blank cell in the reference
    // capture. A motion there keeps that cursor without changing a glyph.
    app.move_to(Point { x: 112, y: 37 });
    see(
        &app,
        "sc.focusnavigation.compare-levels.120x40",
        "compare levels on at 120 by 40 with the 256, 16, and no color bands scrolled into view",
    );

    app.resize(80, 24);
    let narrow = app.wait_screen("narrow compare levels", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && !text(screen).contains("120×40")
            && text(screen).contains(TITLE)
            && (text(screen).contains("●") || text(screen).contains("truecolor"))
    });
    assert_size(&narrow, 80, 24);
    expect(
        &narrow,
        &[TITLE, "rest"],
        &["Page options", "120×40", "pressed"],
    );
    assert!(
        text(&narrow).contains("●") || text(&narrow).contains("truecolor"),
        "narrow compare levels lost its bands\n{}",
        text(&narrow)
    );
    see(
        &app,
        "sc.focusnavigation.compare-levels.80x24",
        "compare levels still on after narrowing to 80 by 24, before restoring the option values",
    );

    click_at(&app, "Compare levels", |hit| hit.x >= 24 && hit.y < 22);
    let restored = app.wait_text("○ Compare levels");
    expect(
        &restored,
        &["○ Compare levels", TITLE, "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "truecolor",
            "Page options",
        ],
    );
    assert!(
        header(&restored).contains(TITLE),
        "leaving compare levels changed the page: {}",
        header(&restored)
    );
    app.click_text("Options");
    let options = app.wait_text("Page options");
    expect(
        &options,
        &["List nav keys", "desktop", "Tab in a field", "moves", TITLE],
        &["vim", "stays", "Ada Lovelace", "● Compare levels"],
    );
    app.finish();
}

#[test]
fn focus_ring_tab_shift_tab_and_click() {
    let app = open_page("s01-focusnavigation-ring", 120, 40);
    let before = app.wait_text("Ada Lovelace");
    assert_size(&before, 120, 40);
    assert_eq!(focus_value(&before), "outside the page");
    assert_one_gutter(&before);
    expect(
        &before,
        &[TITLE, PLAY, "Ada Lovelace", MOVES, "reading order"],
        &["Save ✓", "Page options", "11 stops"],
    );
    see(
        &app,
        "sc.focusnavigation.ring.before",
        "playground focus starts outside the page, one gutter, before Tab",
    );

    for name in FORWARD {
        step_focus(&app, "tab", name);
    }
    for name in BACK {
        step_focus(&app, "shift+tab", name);
    }
    app.click_text("Save");
    let after = app.wait_text("Save ✓");
    assert_size(&after, 120, 40);
    assert_eq!(
        focus_value(&after),
        "Save",
        "first click did not focus Save\n{}",
        text(&after)
    );
    assert_one_gutter(&after);
    expect(
        &after,
        &[
            TITLE,
            "Save ✓",
            "Ada Lovelace",
            PLAY,
            "outside the page → Save",
        ],
        &["Cancel ✓", "Page options", "Email field"],
    );
    see(
        &app,
        "sc.focusnavigation.ring.after",
        "first click focuses Save, shows Save ✓, and keeps a single gutter",
    );
    app.finish();
}

#[test]
fn option_tab_in_field_from_the_control() {
    let app = open_page("s01-focusnavigation-tab-stays", 120, 40);
    app.click_text("Options");
    let before = app.wait_text("Page options");
    expect(
        &before,
        &["List nav keys", "desktop", "Tab in a field", "moves", OPT],
        &["stays", "vim", PLAY],
    );
    app.click_text("moves");
    let open = app.wait_text("Enter Set");
    expect(&open, &["stays", "moves", "desktop"], &["vim"]);
    app.click_text("stays");
    let chosen = app.wait_absent("Enter Set");
    expect(
        &chosen,
        &["stays", "desktop", "Tab in a field", "List nav keys"],
        &["Enter Set", "vim", "moves"],
    );
    app.click_text("Playground");
    let playground = app.wait_text(STAYS);
    expect(
        &playground,
        &[TITLE, PLAY, STAYS, "Ada Lovelace"],
        &[MOVES, "Page options", "↑ k", "Home g"],
    );
    let up = row_with(&playground, "Up");
    assert!(
        up.contains('↑') && !up.contains('k'),
        "Tab in a field changed the nav map: {up:?}"
    );
    step_focus(&app, "tab", "Name field");
    app.key("tab");
    let stayed = app.settle();
    assert_eq!(
        focus_value(&stayed),
        "Name field",
        "Tab left the field while Tab in a field is stays\n{}",
        text(&stayed)
    );
    assert_one_gutter(&stayed);
    expect(
        &stayed,
        &[STAYS, "Ada Lovelace", TITLE],
        &[MOVES, "Email field", "↑ k"],
    );
    assert_size(&stayed, 120, 40);
    see(
        &app,
        "opt.focusnavigation.tab_in_field",
        "Options control sets Tab in a field to stays; List nav keys stays desktop and Tab stays in Name",
    );
    app.finish();
}

#[test]
fn option_nav_vim_boundary() {
    let app = launch(
        "s01-focusnavigation-nav-vim",
        &[
            "--page",
            "focusnavigation",
            "--opt",
            "focusnavigation.nav=vim",
        ],
        120,
        40,
    );
    // The name paints before the vim key rows.
    let screen = app.wait_screen("vim key rows", |screen| {
        let raw = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && raw.contains("Ada Lovelace")
            && raw.contains("↑ k")
            && raw.contains("↓ j")
    });
    assert_size(&screen, 120, 40);
    expect(
        &screen,
        &[
            TITLE, PLAY, "↑ k", "↓ j", "← h", "→ l", "Home g", "End G", MOVES, "none yet",
        ],
        &[STAYS, "Page options", "last: Down"],
    );
    let up = row_with(&screen, "Up");
    assert!(up.contains("↑ k"), "vim Up row does not show ↑ k: {up:?}");
    click_at(&app, "Inbox", |hit| hit.x > 24);
    app.key("j");
    let moved = app.wait_screen("j moved the mailbox", |screen| {
        let raw = text(screen);
        raw.contains("last: Down") && raw.contains("↑ k") && raw.contains("Drafts")
    });
    assert_size(&moved, 120, 40);
    assert_eq!(focus_value(&moved), "Mailbox list");
    assert_one_gutter(&moved);
    let drafts = row_with(&moved, "Drafts");
    assert!(
        drafts.contains('▎'),
        "j did not move the mailbox cursor onto Drafts: {drafts:?}"
    );
    let inbox = row_with(&moved, "Inbox");
    assert!(
        !inbox.contains('▎'),
        "forbidden gutter still on Inbox after j: {inbox:?}"
    );
    expect(
        &moved,
        &["last: Down", "↑ k", MOVES, "Drafts", TITLE],
        &[STAYS, "none yet", "last: Up", "Page options"],
    );
    see(
        &app,
        "opt.focusnavigation.nav",
        "focusnavigation.nav=vim shows vim keys and j moves the mailbox cursor; Tab in a field stays moves",
    );
    app.finish();
}

#[test]
fn keyboard_only_focus_returns_on_the_first_no_op_key() {
    let app = open_page("s01-focusnavigation-keyboard-only", 120, 40);
    app.click_text("Options");
    let options = app.wait_text("Page options");
    expect(&options, &["Focus ring", "always"], &["keyboard-only"]);
    app.click_text("always");
    app.wait_text("Enter Set");
    app.click_text("keyboard-only");
    app.wait_absent("Enter Set");
    app.click_text("Playground");
    app.wait_text("Ada Lovelace");
    click_at(&app, "Gamma", |hit| hit.x >= 24);
    let clicked = app.wait_screen("Gamma holds pointer focus without a gutter", |screen| {
        text(screen).contains("Gamma tag") && gutters(screen) == 0
    });
    assert_eq!(focus_value(&clicked), "Gamma tag", "{}", text(&clicked));

    app.key("x");
    let keyed = app.wait_screen(
        "the first no-op key paints the keyboard focus gutter",
        |s| gutters(s) == 1,
    );
    assert_one_gutter(&keyed);
    assert_eq!(focus_value(&keyed), "Gamma tag", "{}", text(&keyed));
    app.finish();
}
