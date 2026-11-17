//! Dialogs route, sections, specimens, compare levels, options, and modal results.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s05_dialogs/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::bins::showcase;
use crate::support::{App, Launch, Point, find_all, row_text, text, try_launch};

const CRUMB: &str = "Overlays › Dialogs";
const TITLE: &str = "Dialogs";
const BLURB: &str = "Focus is trapped";
const OVERVIEW: &str = "Tokens and principles behind every component";
const MENUS: &str = "Navigation › Menus";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "dialogs"];

const STEADY: &[(&str, &str)] = &[
    ("width", "width 54"),
    ("center", "Center · chrome"),
    ("chrome", "chrome 0+0"),
    ("margin", "margin 3x1"),
    ("height", "max height none"),
    ("scroll", "body scroll on"),
    ("backdrop", "keeps 0 top · 1 bottom"),
    ("dismiss", "typed token: Esc · close button"),
    ("glyph", "glyph ×"),
    ("dirty", "guard edited prompt on"),
    ("edit", "Immediate (the prompt's field)"),
    ("buttons", "End Wrap"),
    ("cancel", "cancel “Cancel”"),
    ("gates", "token gates Drop table"),
    ("bodies", "Text · Input · Facts"),
    ("fields", "Enter in token Submit"),
];

const OMITTED: &[&str] = &[
    "pressed",
    "focus + hover",
    "selected",
    "busy",
    "error",
    "empty",
    "loading",
    "narrow",
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s05_dialogs/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
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

fn page(name: &str, opt: Option<&str>, cols: u16, rows: u16) -> App {
    let mut args = PAGE_ARGS.to_vec();
    if let Some(value) = opt {
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

fn prop_value(screen: &TerminalSnapshot, label: &str) -> String {
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        let mut rest = line.as_str();
        let mut base = 0usize;
        while let Some(rel) = rest.find(label) {
            let at = base + rel;
            let after = &line[at + label.len()..];
            let word = after
                .chars()
                .next()
                .is_none_or(|ch| !ch.is_ascii_alphanumeric());
            let column = line[..at].chars().count();
            if word && column >= 24 {
                return after.trim().to_string();
            }
            let next = at + label.len();
            rest = &line[next..];
            base = next;
        }
    }
    panic!("prop {label:?} missing\n{}", text(screen));
}

fn keys_value(screen: &TerminalSnapshot) -> String {
    prop_value(screen, "Keys")
}

fn nav_value(screen: &TerminalSnapshot) -> String {
    prop_value(screen, "Nav")
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

fn assert_readout(
    screen: &TerminalSnapshot,
    skip: &[&str],
    present: &str,
    absent: &str,
    keys: &str,
    nav: &str,
) {
    let visible = flat(screen);
    assert!(
        visible.contains(present),
        "readout missing {present:?}\n{}",
        text(screen)
    );
    assert!(
        !visible.contains(absent),
        "readout still shows {absent:?}\n{}",
        text(screen)
    );
    assert_eq!(keys_value(screen), keys, "keys changed\n{}", text(screen));
    assert_eq!(nav_value(screen), nav, "nav changed\n{}", text(screen));
    for (key, fragment) in STEADY {
        if skip.contains(key) {
            continue;
        }
        assert!(
            visible.contains(fragment),
            "another option changed; missing {fragment:?}\n{}",
            text(screen)
        );
    }
}

fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    let skip = if screen.cols() >= 110 { 26 } else { 21 };
    (7..screen.rows()).any(|row| {
        let chars: Vec<char> = row_text(screen, row).chars().collect();
        let main: String = chars.into_iter().skip(skip).collect();
        let trimmed = main.trim_start();
        let Some(rest) = trimmed.strip_prefix(label) else {
            return false;
        };
        !rest.starts_with(|ch: char| ch.is_ascii_alphanumeric())
    })
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
    click_where(app, label, |hit| hit.x < 24);
}

fn click_main(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x >= 24 && hit.y >= 6);
}

fn reveal_grid(app: &App, label: &str) {
    let point = Point { x: 70, y: 18 };
    for _ in 0..50 {
        if grid_label(&app.settle(), label) {
            return;
        }
        app.wheel(point, true, 2);
    }
    panic!("grid row {label} did not appear\n{}", text(&app.screen()));
}

fn reveal_control(app: &App, label: &str) {
    let (cols, _) = term_size(app);
    let point = Point { x: cols / 2, y: 16 };
    for _ in 0..40 {
        let screen = app.settle();
        let rows = screen.rows();
        let ready = find_all(&screen, label)
            .into_iter()
            .any(|hit| hit.x >= 24 && hit.y >= 6 && u32::from(hit.y) + 2 < rows.saturating_sub(1));
        if ready {
            return;
        }
        app.wheel(point, true, 1);
    }
    panic!(
        "option {label} did not scroll into view\n{}",
        text(&app.screen())
    );
}

fn reveal_text(app: &App, needle: &str) {
    let (cols, _) = term_size(app);
    let point = Point { x: cols / 2, y: 16 };
    for _ in 0..40 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, true, 2);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..40 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 3);
    }
    app.wait_text(needle);
}

fn term_size(app: &App) -> (u16, u16) {
    let screen = app.screen();
    (
        u16::try_from(screen.cols()).expect("cols"),
        u16::try_from(screen.rows()).expect("rows"),
    )
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

/// Waits out the 140ms press flash, then redraws a released click by growing
/// the terminal until the size caption changes and restoring it.
/// Below 110 columns the caption is clipped, so the detour is 121.
fn quiesce(app: &App) {
    app.flash_expired();
    let (cols, rows) = term_size(app);
    let bumped = if cols >= 110 { cols + 1 } else { 121 };
    app.resize(bumped, rows);
    let label = format!("{bumped}×{rows}");
    app.wait_text(&label);
    app.resize(cols, rows);
    if cols >= 110 {
        app.wait_text(&format!("{cols}×{rows}"));
        return;
    }
    app.wait_screen("narrow size restored", move |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && !text(screen).contains(&label)
            && text(screen).contains(TITLE)
    });
}

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn playground_wide(screen: &TerminalSnapshot) {
    see(
        screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "truecolor",
            "Playground",
            "Open a dialog",
            "Confirm run",
            "Rename task",
            "Unsaved changes",
            "Delete branch",
            "View config",
            "Drop table",
            "Push failed",
            "Release notes",
            "Nothing yet",
            "Migrate sessions table",
            "width 54",
            "max height none",
            "body scroll on",
            "Center · chrome 0+0 · margin 3x1",
            "keeps 0 top · 1 bottom rows live",
            "typed token: Esc · close button",
            "glyph ×",
            "mnemonics",
            "Immediate (the prompt's field)",
            "End Wrap",
            "token gates Drop table",
            "Text · Input · Facts",
            "Enter in token Submit",
            "guard edited prompt on",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            OVERVIEW,
            "Task started",
            "Action(0)",
            "width 44",
            "width 80",
            "Run task now?",
        ],
    );
    assert_eq!(keys_value(screen), "mnemonics");
    assert_eq!(nav_value(screen), "desktop");
}

fn omitted_absent(screen: &TerminalSnapshot) {
    for label in OMITTED {
        assert!(
            !grid_label(screen, label),
            "states grid drew omitted {label}\n{}",
            text(screen)
        );
    }
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s05-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    playground_wide(&screen);
    shot(&app, "sc.dialogs.route");
    app.finish();

    let app = open("s05-route-keys", 120, 40, &["--page", "menus"]);
    app.wait_text(MENUS);
    app.key("]");
    let entered = app.wait_text(BLURB);
    see(
        &entered,
        &[CRUMB, TITLE, "Open a dialog", "Confirm run", "Nothing yet"],
        &[MENUS, OVERVIEW, "Page options", "Task started"],
    );
    app.key("[");
    let left = app.wait_text(MENUS);
    see(&left, &[MENUS], &[CRUMB, BLURB, "Nothing yet"]);
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(&again, &[CRUMB, "Open a dialog"], &[MENUS]);
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, BLURB, "Open a dialog"]);
    click_side(&app, "Overlays");
    app.wait_text("Dialogs");
    click_side(&app, "Dialogs");
    let pointed = app.wait_text("Nothing yet");
    see(
        &pointed,
        &[CRUMB, BLURB, "Confirm run", "Nothing yet", "width 54"],
        &[OVERVIEW, "Page options", "Task started", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s05-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Confirm run");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[TITLE, BLURB, "Playground", "Open a dialog", "Confirm run"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "Task started",
        ],
    );
    shot(&app, "sc.dialogs.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    quiesce(&app);
    let narrow_opt = app.screen();
    assert_size(&app, 80, 24);
    assert_select(&narrow_opt, "Width", "auto", &["44", "80"]);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Width", "Placement"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Open a dialog",
            "Compare levels",
            WIDE,
            "44",
            "80",
        ],
    );
    shot(&app, "sc.dialogs.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    quiesce(&app);
    let narrow_states = app.screen();
    assert_size(&app, 80, 24);
    assert!(
        grid_label(&narrow_states, "rest"),
        "narrow states hid rest\n{}",
        text(&narrow_states)
    );
    see(
        &narrow_states,
        &[TITLE, "○ Compare levels", "rest", "Confirm"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "Open a dialog",
            WIDE,
            LEGEND,
            "focus + hover",
        ],
    );
    omitted_absent(&narrow_states);
    shot(&app, "sc.dialogs.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && text(screen).contains(WIDE)
    });
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
            "Confirm",
            "rest",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Open a dialog",
            "256 colors",
        ],
    );
    omitted_absent(&wide_states);
    shot(&app, "sc.dialogs.states.120x40");
    shot(&app, "sc.dialogs.states");

    for (case_id, label) in [
        ("sc.dialogs.specimen.rest", "rest"),
        ("sc.dialogs.specimen.hover", "hover"),
        ("sc.dialogs.specimen.focused", "focused"),
        ("sc.dialogs.specimen.disabled", "disabled"),
    ] {
        reveal_grid(&app, label);
        let screen = app.screen();
        assert!(grid_label(&screen, label), "{label} left the grid");
        see(
            &screen,
            &[label, "Run task now?"],
            &["● Compare levels", "Page options", "256 colors"],
        );
        shot(&app, case_id);
    }

    reveal_grid(&app, "editing");
    if !text(&app.screen()).contains("Drop table?") {
        app.resize(144, 40);
        app.wait_text("144×40");
        reveal_grid(&app, "editing");
    }
    let editing = app.screen();
    assert!(grid_label(&editing, "editing"));
    see(
        &editing,
        &["editing", "Drop table?"],
        &["● Compare levels", "Page options", "focus + hover"],
    );
    shot(&app, "sc.dialogs.specimen.editing");

    if term_size(&app).0 != 120 {
        app.resize(120, 40);
        app.wait_text(WIDE);
    }
    reveal_grid(&app, "overflow");
    let point = Point { x: 70, y: 18 };
    for _ in 0..12 {
        let screen = app.settle();
        if text(&screen).contains("3 more") && grid_label(&screen, "overflow") {
            break;
        }
        app.wheel(point, true, 1);
    }
    let overflow = app.screen();
    assert!(grid_label(&overflow, "overflow"));
    see(
        &overflow,
        &["overflow", "3 more", "Run task now?"],
        &["● Compare levels", "Page options", "pressed"],
    );
    omitted_absent(&overflow);
    shot(&app, "sc.dialogs.specimen.overflow");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    quiesce(&app);
    let wide_opt = app.screen();
    assert_select(&wide_opt, "Width", "auto", &["44", "80"]);
    assert_select(
        &wide_opt,
        "Placement",
        "center",
        &["below", "above", "right", "left"],
    );
    assert_select(&wide_opt, "Chrome rows", "0-0", &["3-2"]);
    assert_toggle(&wide_opt, "Body scrolls", "on");
    assert_toggle(&wide_opt, "Mnemonics", "on");
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options", "Width"],
        &[
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
            "Open a dialog",
            "Nothing yet",
        ],
    );
    shot(&app, "sc.dialogs.options.120x40");
    shot(&app, "sc.dialogs.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    quiesce(&app);
    let wide_play = app.screen();
    playground_wide(&wide_play);
    shot(&app, "sc.dialogs.playground.120x40");
    shot(&app, "sc.dialogs.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s05-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    quiesce(&app);
    let off = app.screen();
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
    let rows = app.screen().rows();
    click_where(&app, "Compare levels", move |hit| {
        hit.y >= 6 && u32::from(hit.y) + 1 < rows
    });
    let on = app.wait_text("● Compare levels");
    quiesce(&app);
    see(
        &on,
        &[CRUMB, "● Compare levels", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 20 };
    for _ in 0..160 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel(point, true, 4);
    }
    let bands = app.settle();
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
    shot(&app, "sc.dialogs.compare-levels.120x40");

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
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.dialogs.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    let rows = app.screen().rows();
    click_where(&app, "Compare levels", move |hit| {
        hit.y >= 6 && u32::from(hit.y) + 1 < rows
    });
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
    assert_select(&options, "Width", "auto", &["44", "80"]);
    assert_toggle(&options, "Body scrolls", "on");
    see(
        &options,
        &["auto", "Page options", CAP_OPT, CRUMB],
        &["44", "80", "Compare levels", OVERVIEW],
    );
    app.finish();
}

fn option_screen(name: &str, opt: &str) -> App {
    let app = page(name, Some(opt), 120, 40);
    app.wait_text("Open a dialog");
    app
}

struct Shown<'a> {
    present: &'a str,
    absent: &'a str,
    skip: &'a [&'a str],
    keys: &'a str,
    nav: &'a str,
}

fn run_option(
    name: &str,
    opt: &str,
    case_id: &str,
    shown: Shown<'_>,
    control: &str,
    check: fn(&TerminalSnapshot),
    after: fn(&App),
) {
    let app = option_screen(name, opt);
    let screen = app.screen();
    assert_readout(
        &screen,
        shown.skip,
        shown.present,
        shown.absent,
        shown.keys,
        shown.nav,
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    reveal_control(&app, control);
    check(&app.screen());
    click_tab(&app, "Playground");
    app.wait_text("Open a dialog");
    quiesce(&app);
    let back = app.screen();
    assert_readout(
        &back,
        shown.skip,
        shown.present,
        shown.absent,
        shown.keys,
        shown.nav,
    );
    shot(&app, case_id);
    after(&app);
    app.finish();
}

fn after_none(_: &App) {}

#[test]
fn option_width() {
    run_option(
        "s05-width",
        "dialogs.width=44",
        "opt.dialogs.width",
        Shown {
            present: "width 44",
            absent: "width 54",
            skip: &["width"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Width",
        |screen| assert_select(screen, "Width", "44", &["auto", "80"]),
        after_none,
    );
}

#[test]
fn option_placement() {
    run_option(
        "s05-place",
        "dialogs.placement=below",
        "opt.dialogs.placement",
        Shown {
            present: "Below the button",
            absent: "Center · chrome",
            skip: &["center"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Placement",
        |screen| {
            assert_select(
                screen,
                "Placement",
                "below",
                &["center", "above", "right", "left"],
            );
        },
        after_none,
    );
}

#[test]
fn option_chrome() {
    run_option(
        "s05-chrome",
        "dialogs.chrome=3-2",
        "opt.dialogs.chrome",
        Shown {
            present: "chrome 3+2",
            absent: "chrome 0+0",
            skip: &["chrome"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Chrome rows",
        |screen| assert_select(screen, "Chrome rows", "3-2", &["0-0"]),
        after_none,
    );
}

#[test]
fn option_outer_margin() {
    run_option(
        "s05-margin",
        "dialogs.outer_margin=0x0",
        "opt.dialogs.outer_margin",
        Shown {
            present: "margin 0x0",
            absent: "margin 3x1",
            skip: &["margin"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Outer margin",
        |screen| assert_select(screen, "Outer margin", "0x0", &["3x1", "8x3"]),
        after_none,
    );
}

#[test]
fn option_max_height() {
    run_option(
        "s05-height",
        "dialogs.max_height=9",
        "opt.dialogs.max_height",
        Shown {
            present: "max height 9",
            absent: "max height none",
            skip: &["height"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Max height",
        |screen| assert_select(screen, "Max height", "9", &["none", "14"]),
        after_none,
    );
}

#[test]
fn option_body_scroll() {
    run_option(
        "s05-scroll",
        "dialogs.body_scroll=off",
        "opt.dialogs.body_scroll",
        Shown {
            present: "body scroll off",
            absent: "body scroll on",
            skip: &["scroll"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Body scrolls",
        |screen| assert_toggle(screen, "Body scrolls", "off"),
        after_none,
    );
}

#[test]
fn option_backdrop() {
    run_option(
        "s05-backdrop",
        "dialogs.backdrop=full",
        "opt.dialogs.backdrop",
        Shown {
            present: "keeps 0 top · 0 bottom",
            absent: "keeps 0 top · 1 bottom",
            skip: &["backdrop"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Backdrop",
        |screen| assert_select(screen, "Backdrop", "full", &["default", "keep-top"]),
        after_none,
    );
}

fn after_dismiss_never(app: &App) {
    click_main(app, "Confirm run");
    app.wait_text("Run task now?");
    app.key("escape");
    let stayed = app.settle();
    see(
        &stayed,
        &["Run task now?", "Cancel"],
        &["Task started", "Action(0)"],
    );
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, "Cancel")
        .into_iter()
        .filter(|hit| {
            let line = row_text(&screen, u32::from(hit.y));
            let next = line.chars().nth(usize::from(hit.x) + 6);
            next != Some('l') && line.contains("Run") && hit.x >= 24
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "dialog Cancel, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
    let closed = app.wait_text("Action(0)");
    see(
        &closed,
        &["Cancelled", "Action(0)"],
        &["Task started", "Run task now?", "Action(1)"],
    );
}

#[test]
fn option_dismiss() {
    run_option(
        "s05-dismiss",
        "dialogs.dismiss=never",
        "opt.dialogs.dismiss",
        Shown {
            present: "actions only",
            absent: "outside click",
            skip: &["dismiss"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Dismiss",
        |screen| assert_select(screen, "Dismiss", "never", &["auto", "light", "escape"]),
        after_dismiss_never,
    );
}

#[test]
fn option_close_button() {
    run_option(
        "s05-close",
        "dialogs.close_button=danger",
        "opt.dialogs.close_button",
        Shown {
            present: "glyph ✕",
            absent: "glyph ×",
            skip: &["glyph"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "× button",
        |screen| assert_select(screen, "× button", "danger", &["default"]),
        after_none,
    );
}

#[test]
fn option_accelerators() {
    run_option(
        "s05-accel",
        "dialogs.accelerators=y-n",
        "opt.dialogs.accelerators",
        Shown {
            present: "y Confirm · n Cancel",
            absent: "Ctrl+Y",
            skip: &[],
            keys: "y Confirm · n Cancel · mnemonics",
            nav: "desktop",
        },
        "Accelerators",
        |screen| assert_select(screen, "Accelerators", "y-n", &["off"]),
        after_none,
    );
}

#[test]
fn option_keymap() {
    run_option(
        "s05-keymap",
        "dialogs.keymap=ctrl",
        "opt.dialogs.keymap",
        Shown {
            present: "Ctrl+Y Confirm · Ctrl+N Cancel",
            absent: "y Confirm · n Cancel",
            skip: &[],
            keys: "Ctrl+Y Confirm · Ctrl+N Cancel · mnemonics",
            nav: "desktop",
        },
        "Key map",
        |screen| assert_select(screen, "Key map", "ctrl", &["default"]),
        after_none,
    );
}

#[test]
fn option_mnemonics() {
    run_option(
        "s05-mnemonics",
        "dialogs.mnemonics=off",
        "opt.dialogs.mnemonics",
        Shown {
            present: "width 54",
            absent: "mnemonics",
            skip: &[],
            keys: "none",
            nav: "desktop",
        },
        "Mnemonics",
        |screen| assert_toggle(screen, "Mnemonics", "off"),
        after_none,
    );
}

#[test]
fn option_nav() {
    run_option(
        "s05-nav",
        "dialogs.nav=vim",
        "opt.dialogs.nav",
        Shown {
            present: "width 54",
            absent: "Center · chrome 0+0 · margin 8x3",
            skip: &[],
            keys: "mnemonics",
            nav: "vim",
        },
        "Nav keys",
        |screen| assert_select(screen, "Nav keys", "vim", &["desktop"]),
        after_none,
    );
}

#[test]
fn option_ack_enter() {
    run_option(
        "s05-ack",
        "dialogs.ack_enter=next-field",
        "opt.dialogs.ack_enter",
        Shown {
            present: "Enter in token NextField",
            absent: "Enter in token Submit",
            skip: &["fields"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Enter in token",
        |screen| assert_select(screen, "Enter in token", "next-field", &["submit"]),
        after_none,
    );
}

fn rename_field_replaced(screen: &TerminalSnapshot) -> bool {
    find_all(screen, "Task name").into_iter().any(|hit| {
        let row = u32::from(hit.y) + 1;
        if row >= screen.rows() {
            return false;
        }
        let line = row_text(screen, row);
        line.contains('x') && !line.contains("Migrate")
    })
}

fn after_dirty_off(app: &App) {
    click_main(app, "Rename task");
    app.wait_text("Shown in the task list");
    app.type_text("x");
    app.wait_screen(
        "rename field replaced the selected name",
        rename_field_replaced,
    );
    app.click(Point { x: 4, y: 12 });
    let closed = app.wait_text("Action(0)");
    see(
        &closed,
        &["Cancelled", "Action(0)"],
        &["Shown in the task list", "Renamed to", "Action(1)"],
    );
}

#[test]
fn option_guard_dirty() {
    run_option(
        "s05-dirty",
        "dialogs.guard_dirty=off",
        "opt.dialogs.guard_dirty",
        Shown {
            present: "guard edited prompt off",
            absent: "guard edited prompt on",
            skip: &["dirty"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Guard edited prompt",
        |screen| assert_toggle(screen, "Guard edited prompt", "off"),
        after_dirty_off,
    );
}

#[test]
fn option_guard_destructive() {
    let app = option_screen("s05-gdest", "dialogs.guard_destructive=off");
    assert_readout(
        &app.screen(),
        &[],
        "guard edited prompt on",
        "guard edited prompt off",
        "mnemonics",
        "desktop",
    );
    click_tab(&app, "Options");
    app.wait_text("Page options");
    quiesce(&app);
    reveal_text(&app, "Guard danger cancel");
    let screen = app.screen();
    assert_toggle(&screen, "Guard danger cancel", "off");
    assert_toggle(&screen, "Guard edited prompt", "on");
    see(
        &screen,
        &["Guard danger cancel", "off", "Guard edited prompt", "on"],
        &["Open a dialog", "Compare levels"],
    );
    shot(&app, "opt.dialogs.guard_destructive");
    app.finish();
}

#[test]
fn option_edit_entry() {
    run_option(
        "s05-edit",
        "dialogs.edit_entry=explicit",
        "opt.dialogs.edit_entry",
        Shown {
            present: "Explicit (the prompt's field)",
            absent: "Immediate (the prompt's field)",
            skip: &["edit"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Field edit entry",
        |screen| {
            assert_select(
                screen,
                "Field edit entry",
                "explicit",
                &["immediate", "type-to-edit"],
            );
        },
        after_none,
    );
}

#[test]
fn option_button_row() {
    run_option(
        "s05-brow",
        "dialogs.button_row=start-wrap",
        "opt.dialogs.button_row",
        Shown {
            present: "Start Wrap",
            absent: "End Wrap",
            skip: &["buttons"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Button row",
        |screen| {
            assert_select(
                screen,
                "Button row",
                "start-wrap",
                &["end-wrap", "end-more"],
            );
        },
        after_none,
    );
}

#[test]
fn option_cancel_label() {
    run_option(
        "s05-cancel",
        "dialogs.cancel_label=keep",
        "opt.dialogs.cancel_label",
        Shown {
            present: "cancel “Keep”",
            absent: "cancel “Cancel”",
            skip: &["cancel"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Cancel label",
        |screen| assert_select(screen, "Cancel label", "keep", &["cancel"]),
        after_none,
    );
}

#[test]
fn option_confirm_index() {
    run_option(
        "s05-gate",
        "dialogs.confirm_index=nothing",
        "opt.dialogs.confirm_index",
        Shown {
            present: "token gates nothing",
            absent: "token gates Drop table",
            skip: &["gates"],
            keys: "mnemonics",
            nav: "desktop",
        },
        "Token gates",
        |screen| assert_select(screen, "Token gates", "nothing", &["action"]),
        after_none,
    );
}

#[test]
fn dialogs_cancel_then_satisfy_guards() {
    let app = open("s05-dialogs", 120, 40, PAGE_ARGS);
    app.wait_text("Nothing yet");
    click_main(&app, "Confirm run");
    let open = app.wait_text("Run task now?");
    see(
        &open,
        &["Run task now?", "Nothing yet", "Cancel"],
        &["Task started", "Action(0)", "Delete branch?"],
    );
    app.key("tab");
    let tabbed = app.settle();
    see(
        &tabbed,
        &["Run task now?", "Nothing yet"],
        &["Task started", "Delete branch?"],
    );
    app.key("d");
    let trapped = app.settle();
    see(
        &trapped,
        &["Run task now?", "Nothing yet"],
        &["Delete branch?", "Task started"],
    );
    quiesce(&app);
    let open = app.screen();
    see(
        &open,
        &["Run task now?", "Nothing yet"],
        &["Task started", "Action(0)", "Delete branch?"],
    );
    shot(&app, "sc.dialogs.dialogs.before");
    app.key("escape");
    let cancelled = app.wait_screen("escape cancels confirm", |screen| {
        let visible = text(screen);
        visible.contains("Action(0)") && !visible.contains("Run task now?")
    });
    see(
        &cancelled,
        &["Cancelled", "Action(0)", "Confirm run"],
        &["Task started", "Run task now?", "Action(1)", "Nothing yet"],
    );
    shot(&app, "sc.dialogs.dialogs.after");

    let notes = {
        let screen = app.wait_text("Release notes");
        find_all(&screen, "Release notes")
            .into_iter()
            .find(|hit| hit.x >= 24 && hit.y >= 6)
            .expect("Release notes button")
    };
    click_main(&app, "Confirm run");
    app.wait_text("Run task now?");
    app.click(notes);
    let covered = app.settle();
    assert!(
        !text(&covered).contains("What's new in 4.2"),
        "a covered click opened release notes\n{}",
        text(&covered)
    );
    assert!(!text(&covered).contains("Task started"));
    if text(&covered).contains("Run task now?") {
        app.key("escape");
        app.wait_absent("Run task now?");
    }

    click_main(&app, "Confirm run");
    app.wait_text("Run task now?");
    app.key("enter");
    let started = app.wait_text("Task started");
    see(
        &started,
        &["Task started", "Action(1)"],
        &["Run task now?", "Nothing yet"],
    );

    click_main(&app, "Rename task");
    app.wait_text("Shown in the task list");
    app.type_text("x");
    app.wait_screen(
        "rename field replaced the selected name",
        rename_field_replaced,
    );
    app.click(Point { x: 4, y: 12 });
    let guarded = app.settle();
    assert!(
        rename_field_replaced(&guarded),
        "outside click closed or applied the dirty rename\n{}",
        text(&guarded)
    );
    see(
        &guarded,
        &["Shown in the task list", "Rename task"],
        &["Renamed to"],
    );
    app.key("escape");
    let discarded = app.wait_screen("escape discards the dirty rename", |screen| {
        !text(screen).contains("Shown in the task list")
    });
    see(
        &discarded,
        &["Action(0)", "Migrate sessions table"],
        &["Renamed to", "Shown in the task list"],
    );

    click_main(&app, "Delete branch");
    app.wait_text("Delete branch?");
    app.key("escape");
    let kept = app.wait_screen("escape does not delete the branch", |screen| {
        !text(screen).contains("Delete branch?")
    });
    see(
        &kept,
        &["Action(0)"],
        &["Branch feat/rate-limit deleted", "Delete branch?"],
    );

    click_where(&app, "Drop table", |hit| hit.x >= 24 && hit.y < 18);
    app.wait_text("Drop table orders?");
    click_main(&app, "Continue");
    let gate = app.wait_text("Type orders to confirm");
    see(
        &gate,
        &[
            "Type orders to confirm",
            "Gate 1 of 2 passed",
            "Drop table orders?",
        ],
        &["Table public.orders dropped"],
    );
    app.key("enter");
    let mismatch = app.wait_text("Does not match");
    see(
        &mismatch,
        &["Does not match", "Type orders to confirm"],
        &["Table public.orders dropped"],
    );
    app.type_text("orders");
    let armed = app.wait_absent("Does not match");
    assert!(
        text(&armed).contains("orders"),
        "the token field did not show orders\n{}",
        text(&armed)
    );
    app.key("enter");
    let dropped = app.wait_text("Table public.orders dropped");
    see(
        &dropped,
        &["Table public.orders dropped", "Action(1)"],
        &["Type orders to confirm", "Does not match"],
    );
    app.finish();
}
