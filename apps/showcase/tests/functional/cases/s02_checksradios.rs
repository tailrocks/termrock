//! Checks & radios route, sections, options, and choice.
//!
//! Tuiscotti state ids allow only `[a-z0-9_-]` segments. `S02.checksradios`
//! and the dotted case ids are not legal segments, and `Path::set_extension`
//! would collapse those dotted ids onto one file. Each checkpoint is stored as
//! `pre_refactor/s02_checksradios/<case id with '.' replaced by '_'>`.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, Point, SHOWCASE, find, find_all, row_text, text, try_launch};

const TITLE: &str = "Checks & radios";
const BLURB: &str = "Checkboxes (mixed, disabled, click target), radio groups and toggles";
const OVERVIEW: &str = "Tokens and principles behind every component";
const PLAY: &str = "the live widget";
const OPT: &str = "one control per option";
const STATES: &str = "every state, drawn by the widget";
const LABEL_HIT: &str = "click target: box and label";
const ROW_HIT: &str = "click target: the whole row";
const MIXED: &str = "[–] Select all rows";
const SELECT_ON: &str = "[✓] Select all rows";
const SELECT_OFF: &str = "[ ] Select all rows";
const SEND_ON: &str = "[✓] Send notifications";
const SEND_OFF: &str = "[ ] Send notifications";
const ARCHIVED_OFF: &str = "[ ] Include archived projects";
const ARCHIVED_ON: &str = "[✓] Include archived projects";
const MANAGED: &str = "[✓] Managed by policy";
const LAST_NONE: &str = "last: nothing yet";
const LAST_ARCHIVED: &str = "last: Include archived projects checked";
const LAST_SEND_OFF: &str = "last: Send notifications unchecked";
const READ: &str = "Read committed";
const REPEAT: &str = "Repeatable read";
const SERIAL: &str = "Serializable";
const WEST: &str = "eu-west-1";
const EAST: &str = "us-east-1";
const OMITTED: &[&str] = &[
    "busy", "error", "editing", "empty", "loading", "narrow", "selected",
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
    open_with(name, &["--page", "checksradios"], cols, rows)
}

fn open_with(name: &str, args: &[&str], cols: u16, rows: u16) -> App {
    let app = launch(name, args, cols, rows);
    app.wait_text(TITLE);
    app
}

fn test_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s02_checksradios/{}",
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

fn header(screen: &TerminalSnapshot) -> String {
    row_text(screen, 0)
}

fn line_with(screen: &TerminalSnapshot, needle: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no row {needle:?}\n{}", text(screen)))
}

fn mark_before<'a>(line: &'a str, label: &str) -> &'a str {
    let idx = line
        .find(label)
        .unwrap_or_else(|| panic!("missing {label} in {line:?}"));
    line[..idx].trim_end()
}

fn assert_radio(screen: &TerminalSnapshot, label: &str, on: bool) {
    let line = line_with(screen, label);
    let prefix = mark_before(&line, label);
    let mark = if on { "(●)" } else { "( )" };
    let other = if on { "( )" } else { "(●)" };
    assert!(
        prefix.ends_with(mark),
        "{label} mark is not {mark}: {line:?}"
    );
    assert!(
        !prefix.ends_with(other),
        "forbidden {other} still on {label}: {line:?}"
    );
}

fn assert_box(screen: &TerminalSnapshot, label: &str, mark: &str) {
    let line = line_with(screen, label);
    let prefix = mark_before(&line, label);
    assert!(
        prefix.ends_with(mark),
        "{label} mark is not {mark}: {line:?}"
    );
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
    click_at(app, needle, |hit| hit.x < 28);
}

fn state_line(screen: &TerminalSnapshot, marker: &str) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(marker))
        .unwrap_or_else(|| panic!("state row {marker:?} missing\n{}", text(screen)))
}

fn assert_states(screen: &TerminalSnapshot, wide: bool) {
    assert_one_gutter(screen);
    let rest = state_line(screen, "rest           ");
    assert!(
        rest.contains("[ ] Label") && rest.contains("[✓] Label") && rest.contains("[–] Label"),
        "rest row lost its check marks: {rest:?}"
    );
    let hover = state_line(screen, "hover          ");
    assert!(
        !hover.contains("focus") && hover.contains("[ ] Label"),
        "hover row is not the plain hover specimen: {hover:?}"
    );
    let pressed = state_line(screen, "pressed        ");
    assert!(
        pressed.contains("[ ] Label") && pressed.contains('–') && !pressed.contains("(●)"),
        "pressed row should draw checks and a radio dash, not a radio: {pressed:?}"
    );
    let focused = state_line(screen, "focused       ");
    assert!(
        focused.contains("Label") && !focused.contains("hover"),
        "focused row missing: {focused:?}"
    );
    let both = state_line(screen, "focus + hover");
    assert!(
        both.contains("Label"),
        "focus + hover row missing: {both:?}"
    );
    expect(
        screen,
        &[TITLE, "○ Compare levels", "Check off", "Check on", "Mixed"],
        OMITTED,
    );
    expect(
        screen,
        &[],
        &[PLAY, OPT, "Page options", "● Compare levels"],
    );
    if wide {
        assert_size(screen, 120, 40);
        let disabled = state_line(screen, "disabled       ");
        assert!(
            disabled.contains("[ ] Label")
                && disabled.contains("[–] Label")
                && disabled.contains("──● Label"),
            "disabled specimen row missing: {disabled:?}"
        );
        let overflow = state_line(screen, "overflow       ");
        assert!(
            overflow.contains("Label…") && overflow.contains('–') && !overflow.contains("(●)"),
            "overflow specimen row missing: {overflow:?}"
        );
        expect(
            screen,
            &[STATES, "Radio group", "Toggle off", "Toggle on", "120×40"],
            &["Select all rows", ROW_HIT],
        );
    } else {
        assert_size(screen, 80, 24);
        let raw = text(screen);
        assert!(
            !raw.contains("disabled       ") && !raw.contains("overflow       "),
            "narrow states viewport should clip the lower rows\n{raw}"
        );
        expect(
            screen,
            &[TITLE],
            &[STATES, PLAY, OPT, "120×40", "Select all rows"],
        );
    }
}

/// The checkbox block paints before the radio group under it.
fn playground_ready(screen: &TerminalSnapshot) -> bool {
    let raw = text(screen);
    raw.contains("Select all rows")
        && raw.contains("(●) Read committed")
        && raw.contains("──● Auto-save drafts on")
}

fn assert_defaults(screen: &TerminalSnapshot) {
    expect(
        screen,
        &[
            TITLE,
            LABEL_HIT,
            MIXED,
            SEND_ON,
            ARCHIVED_OFF,
            MANAGED,
            LAST_NONE,
            "(●) Read committed",
            "──● Auto-save drafts on",
            "○── Share usage data off",
            "○── Beta features off",
        ],
        &[
            ROW_HIT,
            SELECT_OFF,
            SELECT_ON,
            SEND_OFF,
            LAST_SEND_OFF,
            "the whole row",
            "Disable every control on",
        ],
    );
    assert_radio(screen, READ, true);
    assert_radio(screen, REPEAT, false);
    assert_radio(screen, SERIAL, false);
    assert_radio(screen, WEST, true);
    assert_radio(screen, EAST, false);
}

fn value_line(screen: &TerminalSnapshot, label: &str) -> String {
    let row = (0..screen.rows())
        .find(|row| row_text(screen, *row).contains(label))
        .unwrap_or_else(|| panic!("no row {label:?}\n{}", text(screen)));
    let same = row_text(screen, row);
    if same.contains(" on") || same.contains(" off") {
        return same;
    }
    row_text(screen, row + 1)
}

fn assert_option_row(screen: &TerminalSnapshot, label: &str, value: &str, forbidden: &str) {
    let line = value_line(screen, label);
    assert!(
        line.contains(value),
        "{label} should show {value}: {line:?}"
    );
    assert!(
        !line.contains(forbidden),
        "forbidden {forbidden} on {label}: {line:?}"
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let keys = launch("s02-checksradios-route-keys", &[], 120, 40);
    let overview = keys.wait_text(OVERVIEW);
    expect(&overview, &[OVERVIEW, PLAY], &[TITLE, "Select all rows"]);
    assert!(
        header(&overview).contains("Overview") && !header(&overview).contains(TITLE),
        "overview header is not Checks & radios: {}",
        header(&overview)
    );
    for _ in 0..12 {
        keys.key("]");
    }
    let entered = keys.wait_screen("checks playground from the keyboard", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && playground_ready(screen)
    });
    assert_size(&entered, 120, 40);
    assert_one_gutter(&entered);
    assert_defaults(&entered);
    expect(
        &entered,
        &[TITLE, BLURB, PLAY, "Radio groups", "Toggles"],
        &[OVERVIEW, OPT, STATES, "Page options"],
    );
    assert!(
        header(&entered).contains(TITLE),
        "keyboard ]]]]]]]]]]]] opens Checks & radios: {}",
        header(&entered)
    );
    keys.key("[");
    let left = keys.wait_screen("previous page is Forms", |screen| {
        let head = row_text(screen, 0);
        head.contains("Forms") && !head.contains(TITLE)
    });
    expect(&left, &["Forms"], &["Select all rows", MIXED]);
    keys.key("]");
    let again = keys.wait_text("Select all rows");
    assert!(
        header(&again).contains(TITLE),
        "keyboard ] re-enters Checks & radios: {}",
        header(&again)
    );
    expect(&again, &[MIXED, SEND_ON], &[OVERVIEW, "Page options"]);
    keys.click_text("Overview");
    let back = keys.wait_text(OVERVIEW);
    assert!(
        header(&back).contains("Overview") && !header(&back).contains(TITLE),
        "sidebar Overview returns home: {}",
        header(&back)
    );
    expect(&back, &[OVERVIEW], &["Select all rows", MIXED]);
    let sidebar = keys.screen();
    if !find_all(&sidebar, TITLE).iter().any(|hit| hit.x < 28) {
        click_sidebar(&keys, "Inputs & forms");
        keys.wait_text("Buttons");
    }
    click_sidebar(&keys, TITLE);
    let pointed = keys.wait_screen("checks playground from the sidebar", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && playground_ready(screen)
    });
    assert_one_gutter(&pointed);
    assert_defaults(&pointed);
    expect(
        &pointed,
        &[TITLE, BLURB, PLAY, "Radio groups"],
        &[OVERVIEW, OPT, "Page options"],
    );
    assert!(
        header(&pointed).contains(TITLE),
        "pointer opens Checks & radios: {}",
        header(&pointed)
    );
    keys.finish();

    let page = open_page("s02-checksradios-route-page", 120, 40);
    let screen = page.wait_screen("checks playground from --page", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && playground_ready(screen)
    });
    assert_size(&screen, 120, 40);
    assert_one_gutter(&screen);
    assert_defaults(&screen);
    expect(
        &screen,
        &[TITLE, BLURB, PLAY, "Radio groups", "Toggles", "120×40"],
        &[OPT, STATES, "Page options", ROW_HIT, "↑ k"],
    );
    assert!(
        header(&screen).contains(TITLE),
        "--page header: {}",
        header(&screen)
    );
    see(
        &page,
        "sc.checksradios.route",
        "--page checksradios opens the playground with the sidebar focus on Checks & radios",
    );
    page.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open_page("s02-checksradios-sections", 80, 24);
    app.click_text("Playground");
    let narrow_play = app.wait_text("←→ Section");
    assert_size(&narrow_play, 80, 24);
    assert_one_gutter(&narrow_play);
    expect(
        &narrow_play,
        &[
            TITLE,
            "Playground",
            "Options",
            "States",
            LABEL_HIT,
            MIXED,
            SEND_ON,
            LAST_NONE,
            "←→ Section",
        ],
        &[
            PLAY,
            OPT,
            STATES,
            "Page options",
            "Compare levels",
            "Radio groups",
            "Toggles",
            "120×40",
            ROW_HIT,
        ],
    );
    see(
        &app,
        "sc.checksradios.playground.80x24",
        "playground tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("Options");
    let narrow_opt = app.wait_text("Page options");
    assert_size(&narrow_opt, 80, 24);
    assert_one_gutter(&narrow_opt);
    expect(
        &narrow_opt,
        &[
            TITLE,
            "Page options",
            "Checkbox click target",
            "label",
            "\"Select all\" starts mixed on",
            "Radio keys",
            "default",
            "○── Disable every control off",
        ],
        &[
            PLAY,
            OPT,
            STATES,
            "Compare levels",
            "Select all rows",
            "row",
            "vim",
            "──● Disable every control on",
        ],
    );
    see(
        &app,
        "sc.checksradios.options.80x24",
        "options tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("States");
    let narrow_states = app.wait_text("○ Compare levels");
    assert_states(&narrow_states, false);
    see(
        &app,
        "sc.checksradios.states.80x24",
        "states tab activated at 80 by 24, before the terminal grows",
    );

    app.resize(120, 40);
    // A resize frame can show the overflow label before the later columns.
    let wide_states = app.wait_screen("states grown to 120 by 40", |screen| {
        if screen.cols() != 120 || screen.rows() != 40 {
            return false;
        }
        let raw = text(screen);
        if !raw.contains("120×40") || !raw.contains(STATES) {
            return false;
        }
        let overflow = (0..screen.rows())
            .map(|row| row_text(screen, row))
            .find(|line| line.contains("overflow       "));
        let disabled = (0..screen.rows())
            .map(|row| row_text(screen, row))
            .find(|line| line.contains("disabled       "));
        match (overflow, disabled) {
            (Some(overflow), Some(disabled)) => {
                overflow.contains("Label…")
                    && overflow.contains('–')
                    && overflow.contains("──●")
                    && disabled.contains("──● Label")
            }
            _ => false,
        }
    });
    assert_states(&wide_states, true);
    see(
        &app,
        "sc.checksradios.states.120x40",
        "states tab still active after growing from 80 by 24 to 120 by 40",
    );

    app.click_text("Options");
    let wide_opt = app.wait_text(OPT);
    assert_size(&wide_opt, 120, 40);
    assert_one_gutter(&wide_opt);
    expect(
        &wide_opt,
        &[
            TITLE,
            OPT,
            "Page options",
            "Checkbox click target",
            "label",
            "\"Select all\" starts mixed on",
            "Radio keys",
            "default",
            "○── Disable every control off",
            "120×40",
        ],
        &[
            PLAY,
            STATES,
            "Compare levels",
            "Select all rows",
            "row",
            "vim",
            "──● Disable every control on",
        ],
    );
    see(
        &app,
        "sc.checksradios.options.120x40",
        "options tab at 120 by 40 after the narrow capture",
    );

    app.click_text("Playground");
    // The tab caption paints before the radio group and the last-result line.
    let wide_play = app.wait_screen("playground grown to 120 by 40", |screen| {
        if screen.cols() != 120 || screen.rows() != 40 {
            return false;
        }
        let raw = text(screen);
        raw.contains(PLAY)
            && raw.contains("Radio groups")
            && raw.contains("Toggles")
            && raw.contains(LAST_NONE)
            && raw.contains("(●) Read committed")
            && raw.contains("120×40")
    });
    assert_size(&wide_play, 120, 40);
    assert_one_gutter(&wide_play);
    assert_defaults(&wide_play);
    expect(
        &wide_play,
        &[
            TITLE,
            PLAY,
            "Radio groups",
            "Toggles",
            "120×40",
            "←→ Section",
        ],
        &[OPT, STATES, "Page options", ROW_HIT, "↑↓ Move"],
    );
    see(
        &app,
        "sc.checksradios.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
    );
    app.finish();
}

#[test]
fn compare_levels_then_restore() {
    let app = open_page("s02-checksradios-compare", 120, 40);
    app.click_text("States");
    let off = app.wait_text("○ Compare levels");
    assert_states(&off, true);
    expect(
        &off,
        &["○ Compare levels"],
        &["256 colors", "no color", "● Compare levels"],
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
            "rest           ",
            "focus + hover",
        ],
        &["○ Compare levels", "Page options", "selected"],
    );
    let anchor = find_all(&on, "Compare levels")
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("compare button missing\n{}", text(&on)));
    // 13 ticks of 3 rows each land the 16-color and no-color labels in view.
    app.wheel(anchor, true, 13);
    let bands = app.wait_text("no color");
    assert_size(&bands, 120, 40);
    expect(
        &bands,
        &[
            "no color",
            "16 colors",
            TITLE,
            "rest           ",
            "overflow       ",
        ],
        &["Page options", "○ Compare levels", "selected"],
    );
    assert_eq!(
        text(&bands).matches("truecolor").count(),
        1,
        "truecolor band should scroll off, leaving the header\n{}",
        text(&bands)
    );
    assert!(
        header(&bands).contains(TITLE),
        "scrolling compare levels left the page: {}",
        header(&bands)
    );
    assert!(
        text(&bands).contains("Enter / Space Compare levels"),
        "compare levels lost focus while scrolled\n{}",
        text(&bands)
    );
    see(
        &app,
        "sc.checksradios.compare-levels.120x40",
        "compare levels on at 120 by 40 with the 16-color and no-color bands in view",
    );

    app.resize(80, 24);
    let narrow = app.wait_screen("narrow compare levels", |screen| {
        screen.cols() == 80
            && screen.rows() == 24
            && !text(screen).contains("120×40")
            && text(screen).contains("●")
            && text(screen).contains("rest           ")
    });
    assert_size(&narrow, 80, 24);
    assert_one_gutter(&narrow);
    expect(
        &narrow,
        &[TITLE, "● Compare levels", "rest           ", "truecolor"],
        &[
            "Page options",
            "120×40",
            "○ Compare levels",
            "Select all rows",
        ],
    );
    see(
        &app,
        "sc.checksradios.compare-levels.80x24",
        "compare levels still on after narrowing to 80 by 24, before restoring the option values",
    );

    click_at(&app, "Compare levels", |hit| hit.x >= 20 && hit.y < 22);
    let restored = app.wait_text("○ Compare levels");
    expect(
        &restored,
        &["○ Compare levels", TITLE, "rest           "],
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
    assert_option_row(&options, "Checkbox click target", "label", "row");
    assert!(
        text(&options).contains("\"Select all\" starts mixed on"),
        "compare levels changed the mixed option\n{}",
        text(&options)
    );
    assert_option_row(&options, "Radio keys", "default", "vim");
    expect(
        &options,
        &["○── Disable every control off", TITLE],
        &[
            "──● Disable every control on",
            "● Compare levels",
            "Select all rows",
        ],
    );
    app.finish();
}

#[test]
fn choice_by_key_and_pointer() {
    let app = open_page("s02-checksradios-choice", 120, 40);
    let before = app.wait_text("Select all rows");
    assert_size(&before, 120, 40);
    assert_one_gutter(&before);
    assert_defaults(&before);
    let send = find(&before, "Send notifications");
    // Past the label, still inside the row. Label hit must ignore it.
    app.click(Point {
        x: send.x + 30,
        y: send.y,
    });
    let padded = app.settle();
    assert_box(&padded, "Send notifications", "[✓]");
    expect(
        &padded,
        &[LAST_NONE, MIXED, LABEL_HIT, MANAGED],
        &[LAST_SEND_OFF, SEND_OFF, ROW_HIT],
    );
    see(
        &app,
        "sc.checksradios.choice.before",
        "default playground before mixed, checked, and radio changes",
    );

    app.click_text("Send notifications");
    let unchecked = app.wait_text(LAST_SEND_OFF);
    assert_box(&unchecked, "Send notifications", "[ ]");
    assert_box(&unchecked, "Select all rows", "[–]");
    expect(
        &unchecked,
        &[LAST_SEND_OFF, LABEL_HIT, MANAGED],
        &[SEND_ON, LAST_NONE, "Managed by policy unchecked"],
    );

    app.click_text("Select all rows");
    let mixed_cleared = app.wait_text("Select all rows checked");
    assert_box(&mixed_cleared, "Select all rows", "[✓]");
    expect(
        &mixed_cleared,
        &[SELECT_ON, SEND_OFF, "last: Select all rows checked"],
        &[MIXED, SELECT_OFF, LAST_SEND_OFF],
    );

    app.click_text("Include archived projects");
    let archived = app.wait_text(LAST_ARCHIVED);
    assert_box(&archived, "Include archived projects", "[✓]");
    expect(
        &archived,
        &[ARCHIVED_ON, SELECT_ON, SEND_OFF, MANAGED, LAST_ARCHIVED],
        &[ARCHIVED_OFF, MIXED, "Managed by policy unchecked"],
    );

    app.click_text("Managed by policy");
    let managed = app.settle();
    assert_box(&managed, "Managed by policy", "[✓]");
    expect(
        &managed,
        &[MANAGED, LAST_ARCHIVED, ARCHIVED_ON],
        &["Managed by policy unchecked", "last: Managed by policy"],
    );

    app.click_text(READ);
    app.key("j");
    let ignored = app.settle();
    assert_radio(&ignored, READ, true);
    assert_radio(&ignored, REPEAT, false);
    assert_radio(&ignored, SERIAL, false);
    expect(
        &ignored,
        &[LAST_ARCHIVED, LABEL_HIT],
        &["(●) Repeatable read", ROW_HIT],
    );

    app.key("down");
    let stepped = app.wait_screen("down chooses Repeatable read", |screen| {
        mark_before(&line_with(screen, REPEAT), REPEAT).ends_with("(●)")
    });
    assert_radio(&stepped, READ, false);
    assert_radio(&stepped, REPEAT, true);
    assert_radio(&stepped, SERIAL, false);
    assert_radio(&stepped, WEST, true);
    assert_radio(&stepped, EAST, false);
    expect(
        &stepped,
        &[LAST_ARCHIVED, MANAGED],
        &["(●) Read committed", "(●) us-east-1"],
    );

    app.click_text(SERIAL);
    let pointed = app.wait_screen("pointer chooses Serializable", |screen| {
        mark_before(&line_with(screen, SERIAL), SERIAL).ends_with("(●)")
    });
    assert_radio(&pointed, READ, false);
    assert_radio(&pointed, REPEAT, false);
    assert_radio(&pointed, SERIAL, true);
    assert_one_gutter(&pointed);

    app.click_text(EAST);
    let region = app.settle();
    assert_radio(&region, WEST, true);
    assert_radio(&region, EAST, false);
    assert_radio(&region, SERIAL, true);
    expect(
        &region,
        &[
            LAST_ARCHIVED,
            "○── Beta features off",
            "──● Auto-save drafts on",
        ],
        &[
            "(●) us-east-1",
            "Beta features on",
            "last: Managed by policy",
        ],
    );

    app.click_text("Beta features");
    let after = app.settle();
    assert_size(&after, 120, 40);
    assert_one_gutter(&after);
    assert_box(&after, "Select all rows", "[✓]");
    assert_box(&after, "Send notifications", "[ ]");
    assert_box(&after, "Include archived projects", "[✓]");
    assert_box(&after, "Managed by policy", "[✓]");
    assert_radio(&after, READ, false);
    assert_radio(&after, REPEAT, false);
    assert_radio(&after, SERIAL, true);
    assert_radio(&after, WEST, true);
    assert_radio(&after, EAST, false);
    expect(
        &after,
        &[
            TITLE,
            LABEL_HIT,
            LAST_ARCHIVED,
            "○── Beta features off",
            "○── Share usage data off",
            "──● Auto-save drafts on",
            PLAY,
        ],
        &[
            MIXED,
            SEND_ON,
            ARCHIVED_OFF,
            ROW_HIT,
            "(●) Read committed",
            "(●) Repeatable read",
            "(●) us-east-1",
            "Beta features on",
            "Page options",
            LAST_NONE,
        ],
    );
    assert!(
        header(&after).contains(TITLE),
        "choice changes left the page: {}",
        header(&after)
    );
    see(
        &app,
        "sc.checksradios.choice.after",
        "label click toggles, mixed clears, one radio stays on, and disabled choices stay put",
    );
    app.finish();
}

#[test]
fn option_hit_row_boundary() {
    let app = open_with(
        "s02-checksradios-hit-row",
        &["--page", "checksradios", "--opt", "checksradios.hit=row"],
        120,
        40,
    );
    let screen = app.wait_text(ROW_HIT);
    assert_size(&screen, 120, 40);
    expect(
        &screen,
        &[ROW_HIT, MIXED, SEND_ON, "(●) Read committed", LAST_NONE],
        &[LABEL_HIT, SELECT_OFF, LAST_SEND_OFF],
    );
    let send = find(&screen, "Send notifications");
    app.click(Point {
        x: send.x + 30,
        y: send.y,
    });
    let toggled = app.wait_text(LAST_SEND_OFF);
    assert_box(&toggled, "Send notifications", "[ ]");
    assert_box(&toggled, "Select all rows", "[–]");
    assert_radio(&toggled, READ, true);
    assert_radio(&toggled, WEST, true);
    expect(
        &toggled,
        &[
            ROW_HIT,
            LAST_SEND_OFF,
            MIXED,
            MANAGED,
            "──● Auto-save drafts on",
        ],
        &[
            LABEL_HIT,
            SEND_ON,
            SELECT_OFF,
            "click target: box and label",
        ],
    );
    assert_size(&toggled, 120, 40);
    see(
        &app,
        "opt.checksradios.hit",
        "checksradios.hit=row lets a click past the label toggle Send notifications; mixed and radio keys stay default",
    );
    app.click_text("Options");
    let options = app.wait_text("Page options");
    assert_option_row(&options, "Checkbox click target", "row", "label");
    expect(
        &options,
        &[
            "\"Select all\" starts mixed on",
            "default",
            "○── Disable every control off",
        ],
        &["vim", "──● Disable every control on", "mixed off"],
    );
    app.finish();
}

#[test]
fn option_mixed_off_boundary() {
    let app = open_with(
        "s02-checksradios-mixed-off",
        &["--page", "checksradios", "--opt", "checksradios.mixed=off"],
        120,
        40,
    );
    let screen = app.wait_text(SELECT_OFF);
    assert_size(&screen, 120, 40);
    assert_one_gutter(&screen);
    assert_box(&screen, "Select all rows", "[ ]");
    assert_box(&screen, "Send notifications", "[✓]");
    assert_radio(&screen, READ, true);
    expect(
        &screen,
        &[SELECT_OFF, LABEL_HIT, SEND_ON, LAST_NONE, MANAGED],
        &[MIXED, SELECT_ON, ROW_HIT, LAST_SEND_OFF],
    );
    see(
        &app,
        "opt.checksradios.mixed",
        "checksradios.mixed=off draws Select all rows unchecked; click target stays label",
    );
    app.click_text("Options");
    let options = app.wait_text("Page options");
    assert_option_row(&options, "Checkbox click target", "label", "row");
    expect(
        &options,
        &[
            "○── \"Select all\" starts mixed off",
            "default",
            "○── Disable every control off",
        ],
        &[
            "──● \"Select all\" starts mixed on",
            "vim",
            "──● Disable every control on",
        ],
    );
    app.finish();
}

#[test]
fn option_radio_nav_vim_boundary() {
    let app = open_with(
        "s02-checksradios-radio-vim",
        &[
            "--page",
            "checksradios",
            "--opt",
            "checksradios.radio_nav=vim",
        ],
        120,
        40,
    );
    let screen = app.wait_text(READ);
    assert_size(&screen, 120, 40);
    expect(
        &screen,
        &[LABEL_HIT, MIXED, SEND_ON, LAST_NONE],
        &[ROW_HIT, SELECT_OFF],
    );
    app.click_text(READ);
    app.key("j");
    let moved = app.wait_screen("j chooses Repeatable read", |screen| {
        mark_before(&line_with(screen, REPEAT), REPEAT).ends_with("(●)")
    });
    assert_radio(&moved, READ, false);
    assert_radio(&moved, REPEAT, true);
    assert_radio(&moved, SERIAL, false);
    assert_radio(&moved, WEST, true);
    assert_radio(&moved, EAST, false);
    assert_box(&moved, "Select all rows", "[–]");
    expect(
        &moved,
        &[LABEL_HIT, MIXED, LAST_NONE, TITLE],
        &[
            ROW_HIT,
            "(●) Read committed",
            "(●) us-east-1",
            "Page options",
        ],
    );
    assert_size(&moved, 120, 40);
    assert_one_gutter(&moved);
    see(
        &app,
        "opt.checksradios.radio_nav",
        "checksradios.radio_nav=vim lets j choose Repeatable read; click target and mixed stay default",
    );
    app.click_text("Options");
    let options = app.wait_text("Page options");
    assert_option_row(&options, "Radio keys", "vim", "default");
    assert_option_row(&options, "Checkbox click target", "label", "row");
    expect(
        &options,
        &[
            "\"Select all\" starts mixed on",
            "○── Disable every control off",
        ],
        &["──● Disable every control on", "no-home-end"],
    );
    app.finish();
}

#[test]
fn option_disabled_from_the_control() {
    let app = open_page("s02-checksradios-disabled", 120, 40);
    app.click_text("Options");
    let before = app.wait_screen("checks options", |screen| {
        let raw = text(screen);
        [
            "Page options",
            "label",
            "\"Select all\" starts mixed on",
            "default",
            "○── Disable every control off",
        ]
        .iter()
        .all(|needle| raw.contains(needle))
    });
    expect(
        &before,
        &[
            "label",
            "\"Select all\" starts mixed on",
            "default",
            "○── Disable every control off",
        ],
        &["row", "vim", "──● Disable every control on"],
    );
    app.click_text("Disable every control");
    let enabled = app.wait_text("Disable every control on");
    assert_option_row(&enabled, "Checkbox click target", "label", "row");
    assert_option_row(&enabled, "Radio keys", "default", "vim");
    expect(
        &enabled,
        &[
            "──● Disable every control on",
            "\"Select all\" starts mixed on",
        ],
        &["○── Disable every control off", "vim", "row"],
    );
    app.click_text("Playground");
    let play = app.wait_text(LABEL_HIT);
    expect(
        &play,
        &[
            MIXED,
            SEND_ON,
            LAST_NONE,
            "(●) Read committed",
            "──● Auto-save drafts on",
        ],
        &[ROW_HIT, SELECT_OFF, LAST_SEND_OFF],
    );
    app.click_text("Send notifications");
    let stayed = app.settle();
    assert_size(&stayed, 120, 40);
    assert_box(&stayed, "Send notifications", "[✓]");
    assert_box(&stayed, "Select all rows", "[–]");
    assert_box(&stayed, "Managed by policy", "[✓]");
    assert_radio(&stayed, READ, true);
    assert_radio(&stayed, EAST, false);
    expect(
        &stayed,
        &[
            LABEL_HIT,
            LAST_NONE,
            SEND_ON,
            MIXED,
            "──● Auto-save drafts on",
            "○── Beta features off",
            TITLE,
        ],
        &[
            LAST_SEND_OFF,
            SEND_OFF,
            ROW_HIT,
            SELECT_OFF,
            "Auto-save drafts off",
            "Beta features on",
        ],
    );
    see(
        &app,
        "opt.checksradios.disabled",
        "Options control sets Disable every control on; a click leaves Send notifications checked",
    );
    app.finish();
}
