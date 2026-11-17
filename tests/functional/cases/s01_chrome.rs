//! Chrome route, sections, options, and menus, driven through the showcase PTY.
//!
//! Tuiscotti state ids allow only `[a-z0-9_-]` segments
//! (`validate_test_id_grammar` in tuiscotti `sample.rs`). `S01.chrome` and the
//! dotted case ids are not legal segments, and `Path::set_extension` would
//! also collapse those dotted ids onto one file. Each checkpoint is stored as
//! `pre_refactor/s01_chrome/<case id with '.' replaced by '_'>`.

use tui_test::TerminalSnapshot;

use crate::support::{Launch, SHOWCASE, find_all, row_text, text, try_launch};

const CHROME: &str = "App shell › Chrome";
const OVERVIEW: &str = "Tokens and principles behind every component";
const PLAY_CAPTION: &str = "the live widget";
const OPT_CAPTION: &str = "one control per option";
const STATE_CAPTION: &str = "every state, drawn by the widget";
const BRAND: &str = "Brand lockup · menu bar";

fn launch(name: &str, args: &[&str], cols: u16, rows: u16) -> AppSession {
    let mut spec = Launch::new(name, SHOWCASE);
    spec.args = args.iter().map(|arg| (*arg).to_string()).collect();
    spec.cols = cols;
    spec.rows = rows;
    spec.color = Some("truecolor".to_string());
    spec.motion = Some("paused".to_string());
    let app = try_launch(spec).unwrap_or_else(|error| panic!("{error}"));
    AppSession { app }
}

struct AppSession {
    app: crate::support::App,
}

fn flat(screen: &TerminalSnapshot) -> String {
    text(screen)
        .chars()
        .map(|ch| {
            if matches!(ch, '┃' | '┆' | '│') {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn expect(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let raw = text(screen);
    let joined = flat(screen);
    for needle in present {
        assert!(
            raw.contains(needle) || joined.contains(needle),
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

fn test_id(case_id: &str) -> String {
    format!("pre_refactor/s01_chrome/{}", case_id.replace('.', "_"))
}

fn see(session: &AppSession, case_id: &str, _precondition: &str) {
    session.app.visual(&test_id(case_id));
}

fn click_filtered(
    session: &AppSession,
    needle: &str,
    pred: impl Fn(&crate::support::Point) -> bool,
) {
    let screen = session.app.wait_text(needle);
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
    session.app.click(hits[0]);
}

fn click_left(session: &AppSession, needle: &str) {
    click_filtered(session, needle, |hit| hit.x < 22);
}

fn click_bar(session: &AppSession, title: &str) {
    // The open File menu clips the Sessions title to "Session" on the next row.
    click_filtered(session, title, |hit| {
        (6..=8).contains(&hit.y) && (20..80).contains(&hit.x)
    });
}

fn click_item(session: &AppSession, item: &str) {
    click_filtered(session, item, |hit| {
        (8..=18).contains(&hit.y) && hit.x >= 24
    });
}

fn menu_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("File") && line.contains("Edit"))
        .unwrap_or_else(|| panic!("menu row missing\n{}", text(screen)))
}

fn assert_menu(screen: &TerminalSnapshot, has: &[&str], lacks: &[&str]) {
    let line = menu_line(screen);
    for needle in has {
        assert!(
            line.contains(needle),
            "menu row missing {needle:?}: {line:?}"
        );
    }
    for needle in lacks {
        assert!(
            !line.contains(needle),
            "forbidden menu text {needle:?} still on the bar: {line:?}"
        );
    }
}

const KEPT_DEFAULT: &[&str] = &[
    "open_on Press",
    "overflow Hide",
    "hover_switch on",
    "press_drag on",
    "hide_owner_focus on",
    "wrap on",
    "type_ahead off",
    "nav default",
    "dismiss light",
    "brand lockup",
];

fn kept_except(changed: &str) -> Vec<&'static str> {
    let key = changed.split(' ').next().unwrap_or("");
    KEPT_DEFAULT
        .iter()
        .copied()
        .filter(|token| !token.starts_with(key))
        .collect()
}

fn open_page(cols: u16, rows: u16) -> AppSession {
    let session = launch("s01-chrome-page", &["--page", "chrome"], cols, rows);
    let screen = session.app.wait_text(CHROME);
    expect(&screen, &[CHROME, BRAND], &["Page options"]);
    session
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let keys = launch("s01-chrome-route-keys", &[], 120, 40);
    let overview = keys.app.wait_text(OVERVIEW);
    expect(
        &overview,
        &[OVERVIEW, PLAY_CAPTION],
        &[BRAND, "Page options"],
    );
    keys.app.key("]");
    let entered = keys.app.wait_text(BRAND);
    expect(
        &entered,
        &[CHROME, BRAND, PLAY_CAPTION, "app❯    File"],
        &[OPT_CAPTION, STATE_CAPTION, "Page options"],
    );
    keys.app.key("[");
    let back = keys.app.wait_text(OVERVIEW);
    expect(&back, &[OVERVIEW], &[BRAND, CHROME]);
    keys.app.key("]");
    let again = keys.app.wait_text(BRAND);
    expect(&again, &[CHROME, BRAND], &[OVERVIEW]);
    click_left(&keys, "Overview");
    let pointed_back = keys.app.wait_text(OVERVIEW);
    expect(&pointed_back, &[OVERVIEW], &[BRAND]);
    click_left(&keys, "App shell");
    click_left(&keys, "Chrome");
    let pointed = keys.app.wait_text(BRAND);
    expect(
        &pointed,
        &[CHROME, BRAND, PLAY_CAPTION],
        &[OPT_CAPTION, "Page options", STATE_CAPTION],
    );

    let page = launch("s01-chrome-route-page", &["--page", "chrome"], 120, 40);
    let screen = page.app.wait_text(BRAND);
    expect(
        &screen,
        &[CHROME, BRAND, PLAY_CAPTION, "open_on Press", "brand lockup"],
        &[OPT_CAPTION, STATE_CAPTION, "Page options"],
    );
    see(
        &page,
        "sc.chrome.route",
        "--page chrome opens the playground with the sidebar focus on Chrome",
    );
}

#[test]
fn sections_narrow_then_wide() {
    let session = open_page(80, 24);
    session.app.click_text("Playground");
    let narrow_play = session.app.wait_text("←→ Section");
    expect(
        &narrow_play,
        &[
            CHROME,
            "app❯",
            "Sessions",
            "1 Claude Code (Work)",
            "←→ Section",
        ],
        &[
            PLAY_CAPTION,
            OPT_CAPTION,
            STATE_CAPTION,
            "Page options",
            "Compare levels",
        ],
    );
    see(
        &session,
        "sc.chrome.playground.80x24",
        "playground tab activated at 80 by 24, before the terminal grows",
    );

    session.app.click_text("Options");
    let narrow_opt = session.app.wait_text("Page options");
    expect(
        &narrow_opt,
        &[
            CHROME,
            "Page options",
            "Bar width",
            "full",
            "Type-ahead off",
        ],
        &[
            PLAY_CAPTION,
            OPT_CAPTION,
            STATE_CAPTION,
            "Compare levels",
            "app❯",
        ],
    );
    see(
        &session,
        "sc.chrome.options.80x24",
        "options tab activated at 80 by 24, before the terminal grows",
    );

    session.app.click_text("States");
    let narrow_states = session.app.wait_text("Compare levels");
    expect_states(&narrow_states, false);
    see(
        &session,
        "sc.chrome.states.80x24",
        "states tab activated at 80 by 24, before the terminal grows",
    );

    session.app.resize(120, 40);
    let wide_states = session.app.stable_screen();
    expect_states(&wide_states, true);
    see(
        &session,
        "sc.chrome.states.120x40",
        "states tab still active after growing from 80 by 24 to 120 by 40",
    );

    session.app.click_text("Options");
    let wide_opt = session.app.wait_text(OPT_CAPTION);
    expect(
        &wide_opt,
        &[
            CHROME,
            OPT_CAPTION,
            "Page options",
            "Bar width",
            "Type-ahead off",
            "120×40",
        ],
        &[PLAY_CAPTION, STATE_CAPTION, "Compare levels"],
    );
    see(
        &session,
        "sc.chrome.options.120x40",
        "options tab at 120 by 40 after the narrow capture",
    );

    session.app.click_text("Playground");
    let wide_play = session.app.wait_text(PLAY_CAPTION);
    expect(
        &wide_play,
        &[
            CHROME,
            PLAY_CAPTION,
            "app❯    File",
            "open_on Press",
            "brand lockup",
            "120×40",
        ],
        &[OPT_CAPTION, STATE_CAPTION, "Page options"],
    );
    see(
        &session,
        "sc.chrome.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
    );
}

fn click_compare(session: &AppSession) {
    click_filtered(session, "Compare levels", |hit| hit.y < 16);
}

fn expect_states(screen: &TerminalSnapshot, wide: bool) {
    let present = [
        CHROME,
        "○ Compare levels",
        "Segment",
        "Clickable segment",
        "rest",
        "hover",
        "pressed",
        "busy",
        "narrow",
    ];
    let mut absent = vec![
        PLAY_CAPTION,
        OPT_CAPTION,
        "Page options",
        "focus + hover",
        "editing",
    ];
    if wide {
        expect(screen, &present, &absent);
        expect(screen, &[STATE_CAPTION, "Status item", "120×40"], &[]);
    } else {
        absent.push(STATE_CAPTION);
        expect(screen, &present, &absent);
    }
}

#[test]
fn compare_levels_then_restore() {
    let session = open_page(120, 40);
    session.app.click_text("States");
    let off = session.app.wait_text("○ Compare levels");
    expect(
        &off,
        &["○ Compare levels", CHROME],
        &["Page options", "256 colors"],
    );
    click_compare(&session);
    let on = session.app.wait_text("● Compare levels");
    expect(
        &on,
        &["● Compare levels", CHROME],
        &["Page options", "○ Compare levels"],
    );
    let anchor = find_all(&on, "rest")
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("rest row missing\n{}", text(&on)));
    session.app.wheel(anchor, true, 4);
    session.app.settle();
    let bands = session.app.wait_text("no color");
    // The four notches scroll the button out of view, so the settled frame is
    // the one the capture below stores.
    expect(
        &bands,
        &["256 colors", "16 colors", "no color", CHROME],
        &["Page options", "○ Compare levels"],
    );
    see(
        &session,
        "sc.chrome.compare-levels.120x40",
        "compare levels on at 120 by 40 with all four colour bands scrolled into view",
    );

    session.app.resize(80, 24);
    session.app.wait_absent("120×40");
    let narrow = session.app.wait_text("● Compare levels");
    let top = find_all(&narrow, "Compare levels")
        .into_iter()
        .next()
        .expect("compare button");
    session.app.wheel(top, false, 8);
    session.app.settle();
    let narrow_bands = session.app.wait_text("256 colors");
    expect(
        &narrow_bands,
        &["● Compare levels", "256 colors", CHROME],
        &["Page options", "120×40"],
    );
    see(
        &session,
        "sc.chrome.compare-levels.80x24",
        "compare levels still on after narrowing to 80 by 24, before restoring 120 by 40",
    );

    click_compare(&session);
    let restored = session.app.wait_text("○ Compare levels");
    expect(
        &restored,
        &["○ Compare levels", CHROME, "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
        ],
    );
    session.app.resize(120, 40);
    session.app.wait_text("120×40");
    session.app.click_text("Playground");
    let playground = session.app.wait_text(PLAY_CAPTION);
    expect(
        &playground,
        KEPT_DEFAULT,
        &[
            "type_ahead on",
            "brand none",
            "open_on Release",
            "Page options",
        ],
    );
}

#[test]
fn menus_pointer_and_keyboard() {
    let session = open_page(120, 40);
    let before = session.app.wait_text("last MenuBarAction: —");
    expect(
        &before,
        &[CHROME, "app❯    File", "last MenuBarAction: —"],
        &["Switch menu", "New tab"],
    );
    see(
        &session,
        "sc.chrome.menus.before",
        "menus closed on the default playground before any command",
    );

    session.app.click_text("1 Claude Code (Work)");
    session.app.key("f10");
    let opened = session.app.wait_text("New tab");
    expect(
        &opened,
        &["New tab", "Switch menu", "Split right"],
        &["Paste"],
    );
    choose_open(&session, 0, "Chosen(0, 0)");
    open_and_choose(&session, "New tab", 1, "Chosen(0, 1)");
    open_and_choose(&session, "New tab", 2, "Chosen(0, 2)");
    open_and_choose(&session, "New tab", 3, "Chosen(0, 3)");
    session.app.key("right");
    open_and_choose(&session, "Copy", 0, "Chosen(1, 0)");
    open_and_choose(&session, "Copy", 1, "Chosen(1, 1)");
    session.app.key("right");
    open_and_choose(&session, "Zoom pane", 0, "Chosen(2, 0)");
    let zoomed = session.app.wait_text("Chosen(2, 0)");
    expect(
        &zoomed,
        &["Chosen(2, 0)", "Zoom pane", CHROME],
        &["Switch menu", "Chosen(2, 3)"],
    );
    open_and_choose(&session, "Zoom pane", 1, "Chosen(2, 1)");
    open_and_choose(&session, "Zoom pane", 2, "Chosen(2, 2)");
    reject_disabled(&session);
    session.app.key("right");
    open_and_choose(&session, "Rename…", 0, "Chosen(3, 0)");
    open_and_choose(&session, "Rename…", 1, "Chosen(3, 1)");
    session.app.key("right");
    open_and_choose(&session, "Key reference", 0, "Chosen(4, 0)");

    choose_pointer(&session, "File", "New tab", "Chosen(0, 0)");
    choose_pointer(&session, "File", "Split right", "Chosen(0, 1)");
    choose_pointer(&session, "File", "Export…", "Chosen(0, 2)");
    choose_pointer(&session, "File", "Close tab", "Chosen(0, 3)");
    choose_pointer(&session, "Edit", "Copy", "Chosen(1, 0)");
    choose_pointer(&session, "Edit", "Paste", "Chosen(1, 1)");
    choose_pointer(&session, "View", "Zoom pane", "Chosen(2, 0)");
    let zoomed_again = session.app.wait_text("Chosen(2, 0)");
    expect(
        &zoomed_again,
        &["Chosen(2, 0)", "Zoom pane", CHROME],
        &["Switch menu", "Chosen(2, 3)"],
    );
    choose_pointer(&session, "View", "Redraw", "Chosen(2, 1)");
    choose_pointer(&session, "View", "Usage", "Chosen(2, 2)");
    reject_disabled_pointer(&session);
    choose_pointer(&session, "Session", "Rename…", "Chosen(3, 0)");
    choose_pointer(&session, "Session", "Detach", "Chosen(3, 1)");
    choose_pointer(&session, "Help", "Key reference", "Chosen(4, 0)");
    let after = session.app.wait_text("Chosen(4, 0)");
    expect(
        &after,
        &[CHROME, "Chosen(4, 0)", "app❯    File", PLAY_CAPTION],
        &[
            "Switch menu",
            "Chosen(2, 3)",
            "Page options",
            "Inspect changes",
        ],
    );
    see(
        &session,
        "sc.chrome.menus.after",
        "menus closed after pointer and keyboard commands; Inspect changes was never chosen",
    );
}

fn open_and_choose(session: &AppSession, first: &str, downs: usize, action: &str) {
    session.app.key("enter");
    let opened = session.app.wait_text("Switch menu");
    expect(&opened, &[first, "Switch menu"], &[]);
    choose_open(session, downs, action);
}

fn choose_open(session: &AppSession, downs: usize, action: &str) {
    for _ in 0..downs {
        session.app.key("down");
    }
    session.app.key("enter");
    let done = session.app.wait_text(action);
    expect(&done, &[action, CHROME], &["Switch menu"]);
}

fn reject_disabled(session: &AppSession) {
    session.app.key("enter");
    session.app.wait_text("Zoom pane");
    for _ in 0..3 {
        session.app.key("down");
    }
    session.app.key("enter");
    let held = session.app.stable_screen();
    expect(
        &held,
        &["Inspect changes", "Switch menu", "Zoom pane"],
        &["Chosen(2, 3)"],
    );
    session.app.key("escape");
    let closed = session.app.wait_text("last MenuBarAction: Closed");
    expect(
        &closed,
        &["last MenuBarAction: Closed", CHROME],
        &["Inspect changes", "Switch menu", "Chosen(2, 3)"],
    );
}

fn choose_pointer(session: &AppSession, title: &str, item: &str, action: &str) {
    click_bar(session, title);
    let opened = session.app.wait_text(item);
    expect(&opened, &[item, "Switch menu"], &[]);
    click_item(session, item);
    let done = session.app.wait_text(action);
    expect(&done, &[action, CHROME], &["Switch menu"]);
}

fn reject_disabled_pointer(session: &AppSession) {
    click_bar(session, "View");
    let opened = session.app.wait_text("Inspect changes");
    expect(
        &opened,
        &["Inspect changes", "Switch menu", "Zoom pane"],
        &["Chosen(2, 3)"],
    );
    click_item(session, "Inspect changes");
    let held = session.app.stable_screen();
    expect(
        &held,
        &["Inspect changes", "Switch menu"],
        &["Chosen(2, 3)"],
    );
    session.app.key("escape");
    let closed = session.app.wait_absent("Inspect changes");
    expect(
        &closed,
        &[CHROME],
        &["Chosen(2, 3)", "Switch menu", "Inspect changes"],
    );
}

#[test]
fn menu_switch_pointer_then_keyboard() {
    let session = open_page(120, 40);
    click_bar(&session, "File");
    let before = session.app.wait_text("New tab");
    expect(
        &before,
        &[
            "New tab",
            "Split right",
            "Switch menu",
            "last MenuBarAction: Opened(0)",
        ],
        &["Paste", "Chosen(0, 0)", "Zoom pane"],
    );
    see(
        &session,
        "sc.chrome.menu-switch.before",
        "File menu open before the pointer moves to Edit",
    );
    let edit = session.app.wait_text("Edit");
    let edit_at = find_all(&edit, "Edit")
        .into_iter()
        .find(|hit| (5..=9).contains(&hit.y) && hit.x > 20)
        .expect("Edit title");
    session.app.move_to(edit_at);
    let hovered = session.app.wait_text("Copy");
    expect(
        &hovered,
        &[
            "Copy",
            "Paste",
            "Switch menu",
            "last MenuBarAction: Opened(0)",
        ],
        &["New tab", "Split right", "Chosen(0,", "Chosen(1,"],
    );
    let parked = session.app.wait_text("identity strip");
    let park = find_all(&parked, "identity strip")
        .into_iter()
        .next()
        .expect("identity strip");
    session.app.move_to(park);
    let still = session.app.stable_screen();
    expect(
        &still,
        &["Copy", "last MenuBarAction: Opened(0)"],
        &["New tab"],
    );
    session.app.key("right");
    let after = session.app.wait_text("Zoom pane");
    expect(
        &after,
        &[
            "Zoom pane",
            "Redraw",
            "Switch menu",
            "last MenuBarAction: Opened(2)",
            CHROME,
        ],
        &[
            "New tab",
            "Copy",
            "Paste",
            "Chosen(0,",
            "Chosen(1,",
            "Chosen(2,",
        ],
    );
    see(
        &session,
        "sc.chrome.menu-switch.after",
        "keyboard right from the hovered Edit menu opens View and runs no command",
    );
}

#[test]
fn option_type_ahead_from_the_control() {
    let session = open_page(120, 40);
    session.app.click_text("Options");
    let before = session.app.wait_text("Type-ahead off");
    expect(
        &before,
        &["Type-ahead off", "Hover switches menus on", OPT_CAPTION],
        &["Type-ahead on", PLAY_CAPTION],
    );
    session.app.click_text("Type-ahead");
    let toggled = session.app.wait_text("Type-ahead on");
    expect(
        &toggled,
        &["Type-ahead on", "Hover switches menus on", "Page options"],
        &["Type-ahead off", PLAY_CAPTION],
    );
    session.app.click_text("Playground");
    let readout = session.app.wait_text("type_ahead on");
    expect(
        &readout,
        &[
            "type_ahead on",
            "open_on Press",
            "overflow Hide",
            "hover_switch on",
            "press_drag on",
            "hide_owner_focus on",
            "wrap on",
            "nav default",
            "dismiss light",
            "brand lockup",
            PLAY_CAPTION,
        ],
        &[
            "type_ahead off",
            "Page options",
            "brand none",
            "open_on Release",
        ],
    );
    see(
        &session,
        "opt.chrome.type_ahead",
        "Type-ahead toggled on from the Options control, then read back on the playground",
    );
}

#[test]
fn option_boundaries() {
    boundary(
        "opt.chrome.bar",
        &["--page", "chrome", "--opt", "chrome.bar=narrow"],
        "overflow Hide",
        &["app❯    File", "Edit", "View"],
        &["Session", "Help", "»"],
        "chrome.bar=narrow clips Session and Help off the menu row and leaves overflow Hide",
    );
    boundary(
        "opt.chrome.open_on",
        &["--page", "chrome", "--opt", "chrome.open_on=release"],
        "open_on Release",
        &["app❯    File", "Session", "Help"],
        &["»"],
        "chrome.open_on=release shows open_on Release and leaves the other readout tokens",
    );
    boundary(
        "opt.chrome.overflow",
        &[
            "--page",
            "chrome",
            "--opt",
            "chrome.bar=narrow",
            "--opt",
            "chrome.overflow=chevron",
        ],
        "overflow Chevron",
        &["app❯    File", "Edit", "»"],
        &["View", "Session"],
        "chrome.overflow=chevron with the required chrome.bar=narrow shows » and hides View",
    );
    boundary(
        "opt.chrome.hover_switch",
        &["--page", "chrome", "--opt", "chrome.hover_switch=off"],
        "hover_switch off",
        &["app❯    File"],
        &[],
        "chrome.hover_switch=off leaves press_drag on",
    );
    boundary(
        "opt.chrome.press_drag",
        &["--page", "chrome", "--opt", "chrome.press_drag=off"],
        "press_drag off",
        &["app❯    File"],
        &[],
        "chrome.press_drag=off leaves hover_switch on",
    );
    boundary(
        "opt.chrome.hide_owner_focus",
        &["--page", "chrome", "--opt", "chrome.hide_owner_focus=off"],
        "hide_owner_focus off",
        &["app❯    File"],
        &[],
        "chrome.hide_owner_focus=off leaves hover_switch on",
    );
    boundary(
        "opt.chrome.wrap",
        &["--page", "chrome", "--opt", "chrome.wrap=off"],
        "wrap off",
        &["app❯    File"],
        &[],
        "chrome.wrap=off leaves type_ahead off",
    );
    boundary(
        "opt.chrome.nav",
        &["--page", "chrome", "--opt", "chrome.nav=vim"],
        "nav vim",
        &["app❯    File"],
        &[],
        "chrome.nav=vim leaves dismiss light",
    );
    boundary(
        "opt.chrome.dismiss",
        &["--page", "chrome", "--opt", "chrome.dismiss=escape"],
        "dismiss escape",
        &["app❯    File"],
        &[],
        "chrome.dismiss=escape leaves nav default",
    );
    let session = launch(
        "s01-chrome-opt-brand",
        &["--page", "chrome", "--opt", "chrome.brand=none"],
        120,
        40,
    );
    let screen = session.app.wait_text("brand none");
    expect(
        &screen,
        &[
            "brand none",
            "open_on Press",
            "overflow Hide",
            "type_ahead off",
            CHROME,
        ],
        &["brand lockup", "brand compact", "app❯"],
    );
    assert_menu(&screen, &["File", "Edit", "Session", "Help"], &["app❯"]);
    see(
        &session,
        "opt.chrome.brand",
        "chrome.brand=none removes the app❯ lockup and leaves the other readout tokens",
    );
}

fn boundary(
    case_id: &str,
    args: &[&'static str],
    changed: &str,
    bar_has: &[&str],
    bar_lacks: &[&str],
    precondition: &str,
) {
    let session = launch(case_id, args, 120, 40);
    let screen = session
        .app
        .wait_screen(changed, |screen| flat(screen).contains(changed));
    let mut present = kept_except(changed);
    present.push(changed);
    present.push(CHROME);
    expect(&screen, &present, &[]);
    assert_menu(&screen, bar_has, bar_lacks);
    see(&session, case_id, precondition);
}

#[test]
fn option_stress_status_items_and_long_label() {
    let items = launch(
        "s01-chrome-stress-items",
        &["--page", "chrome", "--opt", "chrome.stress=20k items"],
        120,
        40,
    );
    let screen = items.app.wait_text("of 20008");
    expect(&screen, &["places", "payments-platform"], &[]);
    items.app.finish();

    let label = launch(
        "s01-chrome-stress-label",
        &["--page", "chrome", "--opt", "chrome.stress=65k label"],
        120,
        40,
    );
    let screen = label.app.wait_text("of 9 ·");
    expect(&screen, &["payments-platform", "production"], &[]);
    label.app.finish();
}
