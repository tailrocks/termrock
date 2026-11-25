//! Chips & selects route, sections, specimens, options, and the two checkpoints.
//!
//! Tuiscotti state ids allow only `[a-z0-9_-]` segments. Dotted case ids are
//! stored as `pre_refactor/s02_chipsselects/<case id with '.' replaced by '_'>`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{Launch, Point, SHOWCASE, cell_bg, find_all, row_text, text, try_launch};

const TITLE: &str = "Chips & selects";
const BLURB: &str = "Removable chips, a popup select, and strips that drop what does not fit";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const PENDING: &str = "status = 'pending'";
const TOTAL: &str = "total > 100";
const COUNTRY: &str = "country in (DE, FR)";
const NOTHING: &str = "last action: nothing yet";
const MARKER_ON: &str = "──● +N marker on";
const MARKER_OFF: &str = "○── +N marker off";
const MENU_ON: &str = "──● +N opens a menu on";
const MENU_OFF: &str = "○── +N opens a menu off";
const DIGITS_ON: &str = "──● 1–9 toggle streams on";
const DIGITS_OFF: &str = "○── 1–9 toggle streams off";
const HOVER_ON: &str = "──● Hover moves cursor on";
const HOVER_OFF: &str = "○── Hover moves cursor off";
const DRAG_ON: &str = "──● Press-drag-release on";
const DRAG_OFF: &str = "○── Press-drag-release off";
const LABEL_ON: &str = "──● Label click opens on";
const LABEL_OFF: &str = "○── Label click opens off";
const WRAP_OFF: &str = "○── Arrows wrap off";
const WRAP_ON: &str = "──● Arrows wrap on";
const AHEAD_OFF: &str = "○── Type-ahead off";
const AHEAD_ON: &str = "──● Type-ahead on";
const BLUR_ON: &str = "──● Close on blur on";
const BLUR_OFF: &str = "○── Close on blur off";
const MIXED_OFF: &str = "○── Sort shows mixed off";
const MIXED_ON: &str = "──● Sort shows mixed on";

const SHOWN: &[(&str, &str)] = &[
    ("sc.chipsselects.specimen.rest", "rest"),
    ("sc.chipsselects.specimen.hover", "hover"),
    ("sc.chipsselects.specimen.pressed", "pressed"),
    ("sc.chipsselects.specimen.focused", "focused"),
    ("sc.chipsselects.specimen.focused_hover", "focus + hover"),
    ("sc.chipsselects.specimen.disabled", "disabled"),
    ("sc.chipsselects.specimen.error", "error"),
    ("sc.chipsselects.specimen.empty", "empty"),
    ("sc.chipsselects.specimen.overflow", "overflow"),
    ("sc.chipsselects.specimen.narrow", "narrow"),
];

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> crate::support::App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn test_id(case_id: &str) -> String {
    format!(
        "pre_refactor/s02_chipsselects/{}",
        case_id.replace('.', "_")
    )
}

fn see(app: &crate::support::App, case_id: &str, _precondition: &str) {
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
    assert_eq!(screen.cols(), cols, "terminal columns");
    assert_eq!(screen.rows(), rows, "terminal rows");
}

fn option_value(screen: &TerminalSnapshot, label: &str) -> String {
    let visible = text(screen);
    let lines: Vec<&str> = visible.lines().collect();
    let index = lines
        .iter()
        .position(|line| line.contains(label))
        .unwrap_or_else(|| panic!("missing {label:?}\n{visible}"));
    let column = lines[index].find(label).expect("label column");
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).copied().unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    rest.split_whitespace()
        .find(|token| *token != "▎")
        .unwrap_or("")
        .to_string()
}

fn click_filtered(app: &crate::support::App, needle: &str, pred: impl Fn(&Point) -> bool) {
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

fn click_left(app: &crate::support::App, needle: &str) {
    click_filtered(app, needle, |hit| hit.x < 24);
}

fn click_main(app: &crate::support::App, needle: &str) {
    click_filtered(app, needle, |hit| hit.x > 24);
}

/// The states gallery repeats the playground value. The playground control
/// is the top one in the main column.
fn click_top_main(app: &crate::support::App, needle: &str) {
    let screen = app.wait_text(needle);
    let hit = find_all(&screen, needle)
        .into_iter()
        .filter(|hit| hit.x > 24)
        .min_by_key(|hit| hit.y)
        .unwrap_or_else(|| panic!("no main {needle:?}\n{}", text(&screen)));
    app.click(hit);
}

fn column_of(screen: &TerminalSnapshot, row: u16, byte: usize) -> u16 {
    let mut offset = 0;
    for (column, cell) in screen.row(u32::from(row)).expect("row").iter().enumerate() {
        let next = offset + cell.text().len();
        if byte >= offset && byte < next {
            return u16::try_from(column).expect("column");
        }
        offset = next;
    }
    panic!("byte {byte} is not on row {row}");
}

fn click_close(app: &crate::support::App, label: &str) {
    let screen = app.wait_text(label);
    let hit = find_all(&screen, label)
        .into_iter()
        .find(|point| point.x > 24)
        .unwrap_or_else(|| panic!("missing {label}\n{}", text(&screen)));
    let row = row_text(&screen, u32::from(hit.y));
    let start = row.find(label).expect("label bytes");
    let close = row[start..]
        .find('×')
        .map(|index| start + index)
        .unwrap_or_else(|| panic!("no × after {label} on {row:?}"));
    app.click(Point {
        x: column_of(&screen, hit.y, close),
        y: hit.y,
    });
}

/// A select whose value line is clipped still shows its label under the
/// "Page options" heading. That frame is not the control.
fn option_ready(screen: &TerminalSnapshot, label: &str) -> bool {
    let body = text(screen);
    if !body.contains(label) {
        return false;
    }
    if body.contains("Page options") && option_value(screen, label).is_empty() {
        return false;
    }
    true
}

/// The first Options paint can leave playground rows under the heading.
/// "List cursor" is the last control on the 120×40 options page.
fn options_page(screen: &TerminalSnapshot) -> bool {
    let body = text(screen);
    body.contains("Page options") && body.contains("List cursor") && !body.contains("Segment strip")
}

fn show_option(app: &crate::support::App, label: &str) -> TerminalSnapshot {
    app.click_text("Options");
    let started = Instant::now();
    let mut top = app.wait_text("Page options");
    while !options_page(&top) && started.elapsed() < Duration::from_millis(200) {
        std::thread::sleep(Duration::from_millis(20));
        top = app.screen();
    }
    // Tab completes that paint. Page Down is a second frame and, once the
    // press flash has expired, it also scrolls the list during the later
    // quiet wait. Only send it when the label is actually below this page.
    let mut nudged = false;
    if !options_page(&top) {
        app.key("tab");
        nudged = true;
        top = app.wait_screen("options page painted", options_page);
    }
    if option_ready(&top, label) {
        return top;
    }
    if !nudged {
        app.key("tab");
    }
    app.key("pagedown");
    app.wait_screen(&format!("option {label}"), |screen| {
        option_ready(screen, label)
    })
}

fn options_tab(screen: &TerminalSnapshot) -> Point {
    find_all(screen, "Options")
        .into_iter()
        .find(|hit| hit.y == 4)
        .unwrap_or_else(|| panic!("Options tab\n{}", text(screen)))
}

fn tab_pressed(screen: &TerminalSnapshot) -> bool {
    cell_bg(screen, options_tab(screen)) == SnapshotColor::Rgb(0x4a, 0x4a, 0x52)
}

fn tab_focused(screen: &TerminalSnapshot) -> bool {
    row_text(screen, 4).contains('▎')
}

/// A Page Down drawn after the 140ms press flash paints the right list and
/// the released tab. With focus-on-release, holding the button paints that
/// fill again and leaves the keyboard focus where the list already has it.
fn hold_options_press(app: &crate::support::App, screen: &TerminalSnapshot) -> TerminalSnapshot {
    if tab_pressed(screen) {
        return screen.clone();
    }
    let tab = options_tab(screen);
    app.move_to(tab);
    app.down(tab);
    let held = app.wait_screen("options tab held", |screen| {
        tab_pressed(screen) && !tab_focused(screen)
    });
    let held = held.clone();
    app.up(tab);
    held
}

fn check_frame(app: &crate::support::App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        let fields = failure.changed_fields();
        // Cells already match the accepted options frame. A hidden cursor is
        // not drawn; the terminal parks it on different cells of otherwise
        // equal frames.
        let parked = failure.changed_cell_total() == 0
            && !screen.cursor().visible()
            && !fields.is_empty()
            && fields
                .iter()
                .all(|field| field == "cursor.col" || field == "cursor.row");
        if parked {
            return;
        }
        panic!(
            "{test_id} {} sha {} path {} cells {} fields {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total(),
            fields.join(",")
        );
    }
}

fn shot_option(app: &crate::support::App, screen: &TerminalSnapshot, case_id: &str) {
    let screen = hold_options_press(app, screen);
    check_frame(app, &screen, &test_id(case_id));
}

fn assert_value(screen: &TerminalSnapshot, label: &str, value: &str) {
    let got = option_value(screen, label);
    assert_eq!(
        got,
        value,
        "visible {label} is {got:?}, want {value:?}\n{}",
        text(screen)
    );
}

fn open_sort(app: &crate::support::App) {
    click_filtered(app, "Sort by", |hit| hit.x > 24 && hit.y < 36);
    if !text(&app.screen()).contains('▴') {
        app.key("enter");
    }
    app.wait_text("▴");
}

fn page(name: &str, cols: u16, rows: u16) -> crate::support::App {
    let app = open(name, cols, rows, &["--page", "chipsselects"]);
    app.wait_text(TITLE);
    app
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let keys = open("s02-chips-route-keys", 120, 40, &[]);
    let overview = keys.wait_text(OVERVIEW);
    expect(&overview, &[OVERVIEW, CAP_PLAY], &[TITLE, "match all"]);
    for _ in 0..10 {
        keys.key("]");
    }
    let entered = keys.wait_text("match all");
    expect(
        &entered,
        &[TITLE, BLURB, "match all", NOTHING, CAP_PLAY],
        &[OVERVIEW, "Page options", CAP_OPT],
    );
    for _ in 0..10 {
        keys.key("[");
    }
    let back = keys.wait_text(OVERVIEW);
    expect(&back, &[OVERVIEW], &[TITLE, "match all", PENDING]);
    for _ in 0..10 {
        keys.key("]");
    }
    let again = keys.wait_text("match all");
    expect(&again, &[TITLE, "match all"], &[OVERVIEW]);
    click_left(&keys, "Overview");
    let pointed_back = keys.wait_text(OVERVIEW);
    expect(&pointed_back, &[OVERVIEW], &[TITLE, "match all"]);
    click_left(&keys, "Inputs & forms");
    keys.wait_text("Buttons");
    click_left(&keys, TITLE);
    let pointed = keys.wait_text("match all");
    expect(
        &pointed,
        &[TITLE, "match all", NOTHING, CAP_PLAY],
        &[OVERVIEW, "Page options", CAP_STATES],
    );
    keys.finish();

    let app = page("s02-chips-route-page", 120, 40);
    let screen = app.wait_text("match all");
    expect(
        &screen,
        &[
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "match all",
            PENDING,
            TOTAL,
            "created_at",
            "50",
            "PostgreSQL",
            NOTHING,
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            OVERVIEW,
            "match any",
        ],
    );
    assert_size(&screen, 120, 40);
    see(
        &app,
        "sc.chipsselects.route",
        "--page chipsselects opens the playground with the sidebar focus on Chips & selects",
    );
    app.finish();
}

fn expect_play(screen: &TerminalSnapshot, wide: bool) {
    let mut present = vec![TITLE, "match all", PENDING, TOTAL, NOTHING];
    let mut absent = vec![
        "Page options",
        "Compare levels",
        CAP_OPT,
        CAP_STATES,
        "match any",
        "last action: removed",
        "+3",
        "Mixed",
    ];
    if wide {
        present.extend([
            CAP_PLAY,
            WIDE,
            BLURB,
            "created_at",
            "50",
            "PostgreSQL",
            COUNTRY,
        ]);
    } else {
        absent.extend([CAP_PLAY, WIDE, "Page options"]);
    }
    expect(screen, &present, &absent);
}

fn expect_options_top(screen: &TerminalSnapshot) {
    assert_value(screen, "Chip bar width", "full");
    assert_value(screen, "× plate width", "3");
    assert_value(screen, "× button", "default");
    assert_value(screen, "+N button", "default");
    assert_value(screen, "Chip keys", "default");
    assert_value(screen, "Chip key map", "default");
    assert_value(screen, "Select arrows", "open");
    assert_value(screen, "Wheel over the list", "viewport");
    expect(
        screen,
        &["Page options", MARKER_ON, MENU_ON, DIGITS_ON, TITLE],
        &[
            MARKER_OFF,
            MENU_OFF,
            DIGITS_OFF,
            CAP_PLAY,
            CAP_STATES,
            "Compare levels",
        ],
    );
}

fn expect_options_rest(screen: &TerminalSnapshot) {
    assert_value(screen, "List cursor", "fill");
    assert_value(screen, "List track click", "theme");
    assert_value(screen, "Dismiss", "light");
    assert_value(screen, "Select keys", "default");
    expect(
        screen,
        &[
            HOVER_ON, DRAG_ON, LABEL_ON, WRAP_OFF, AHEAD_OFF, BLUR_ON, MIXED_OFF,
        ],
        &[
            HOVER_OFF, DRAG_OFF, LABEL_OFF, WRAP_ON, AHEAD_ON, BLUR_OFF, MIXED_ON, CAP_PLAY,
        ],
    );
}

#[test]
fn sections_narrow_then_wide() {
    let app = page("s02-chips-sections", 80, 24);
    app.click_text("Playground");
    let narrow_play = app.wait_text("match all");
    expect_play(&narrow_play, false);
    assert_size(&narrow_play, 80, 24);
    see(
        &app,
        "sc.chipsselects.playground.80x24",
        "playground tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("Options");
    let narrow_opt = app.wait_text("Page options");
    assert_value(&narrow_opt, "Chip bar width", "full");
    assert_value(&narrow_opt, "× plate width", "3");
    expect(
        &narrow_opt,
        &[TITLE, "Page options", MARKER_ON, MENU_ON],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            MARKER_OFF,
            WIDE,
        ],
    );
    assert_size(&narrow_opt, 80, 24);
    see(
        &app,
        "sc.chipsselects.options.80x24",
        "options tab activated at 80 by 24, before the terminal grows",
    );

    app.click_text("States");
    let narrow_states = app.wait_text("Compare levels");
    expect(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "disabled",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "selected",
            "busy",
            "editing",
            "loading",
            WIDE,
        ],
    );
    assert_size(&narrow_states, 80, 24);
    see(
        &app,
        "sc.chipsselects.states.80x24",
        "states tab activated at 80 by 24, before the terminal grows",
    );

    app.resize(120, 40);
    app.wait_screen("chips layout at 120 by 40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(WIDE)
    });
    app.click_text("Playground");
    app.click_text("States");
    let wide_states = app.wait_text("narrow");
    expect(
        &wide_states,
        &[
            TITLE,
            CAP_STATES,
            WIDE,
            "○ Compare levels",
            "rest",
            "hover",
            "pressed",
            "focused",
            "focus + hover",
            "disabled",
            "error",
            "empty",
            "overflow",
            "narrow",
            "status = 'ok'",
            "Mixed",
            "+3",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "selected",
            "busy",
            "editing",
            "loading",
        ],
    );
    assert_size(&wide_states, 120, 40);
    see(
        &app,
        "sc.chipsselects.states.120x40",
        "states tab still active after growing from 80 by 24 to 120 by 40",
    );
    for (case_id, label) in SHOWN {
        expect(
            &wide_states,
            &[*label, "○ Compare levels", CAP_STATES],
            &["selected", "busy", "editing", "loading", "● Compare levels"],
        );
        see(
            &app,
            case_id,
            "states grid row is visible and Compare levels is off",
        );
    }

    app.click_text("Options");
    let wide_opt = app.wait_text(CAP_OPT);
    expect_options_top(&wide_opt);
    expect(&wide_opt, &[CAP_OPT, WIDE], &[CAP_PLAY, CAP_STATES]);
    assert_value(&wide_opt, "Tab in an open list", "choose");
    assert_value(&wide_opt, "List rows", "8");
    assert_value(&wide_opt, "List placement", "below");
    assert_value(&wide_opt, "List fit", "default");
    assert_value(&wide_opt, "List width", "12-40");
    assert_size(&wide_opt, 120, 40);
    see(
        &app,
        "sc.chipsselects.options.120x40",
        "options tab at 120 by 40 after the narrow capture",
    );
    app.key("tab");
    app.key("pagedown");
    let rest = app.wait_text("Sort shows mixed");
    expect_options_rest(&rest);

    app.click_text("Playground");
    let wide_play = app.wait_screen("playground selects after the options page", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(CAP_PLAY)
            && visible.contains("created_at")
            && visible.contains("PostgreSQL")
            && visible.contains(NOTHING)
            && !visible.contains("List fit")
            && !visible.contains("Sort shows mixed")
    });
    expect_play(&wide_play, true);
    assert_size(&wide_play, 120, 40);
    see(
        &app,
        "sc.chipsselects.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
    );
    app.finish();
}

#[test]
fn compare_levels_then_restore() {
    let app = page("s02-chips-compare", 120, 40);
    app.click_text("States");
    let off = app.wait_text("○ Compare levels");
    expect(
        &off,
        &["○ Compare levels", "rest", TITLE, CAP_STATES],
        &[
            "Page options",
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
        ],
    );
    click_filtered(&app, "Compare levels", |hit| hit.y < 16);
    let on = app.wait_text("● Compare levels");
    expect(
        &on,
        &["● Compare levels", TITLE],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );

    let mut saw_256 = false;
    let mut saw_16 = false;
    let mut saw_none = false;
    let note = |body: &str, saw_256: &mut bool, saw_16: &mut bool, saw_none: &mut bool| {
        *saw_256 |= body.contains("256 colors");
        *saw_16 |= body.contains("16 colors");
        *saw_none |= body.contains("no color");
    };
    // Each colour name is one line. Two Page Down keys before the first
    // paint skip `16 colors`, and the accepted shot is the later Page Down
    // landing, so sample that paint and then let it settle.
    for _ in 0..4 {
        app.key("pageup");
        let _ = app.settle();
    }
    for _ in 0..16 {
        note(
            &text(&app.screen()),
            &mut saw_256,
            &mut saw_16,
            &mut saw_none,
        );
        if saw_256 && saw_16 && saw_none {
            break;
        }
        app.key("pagedown");
        let started = std::time::Instant::now();
        let mut last = String::new();
        let mut stable = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_millis(250) {
            let body = text(&app.screen());
            note(&body, &mut saw_256, &mut saw_16, &mut saw_none);
            if body == last {
                if stable.elapsed() >= std::time::Duration::from_millis(40) {
                    break;
                }
            } else {
                last = body;
                stable = std::time::Instant::now();
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let _ = app.settle();
    }
    note(
        &text(&app.screen()),
        &mut saw_256,
        &mut saw_16,
        &mut saw_none,
    );
    assert!(
        saw_256 && saw_16 && saw_none,
        "colour bands 256={saw_256} 16={saw_16} none={saw_none}\n{}",
        text(&app.screen())
    );

    let wide = app.wait_text("no color");
    expect(
        &wide,
        &["no color", TITLE],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    assert_size(&wide, 120, 40);
    see(
        &app,
        "sc.chipsselects.compare-levels.120x40",
        "compare levels on at 120 by 40 with a colour band scrolled into view",
    );

    for _ in 0..12 {
        if text(&app.screen()).contains("● Compare levels") {
            break;
        }
        app.key("pageup");
    }
    app.resize(80, 24);
    app.wait_screen("compare layout at 80 by 24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && !text(screen).contains(WIDE)
    });
    for _ in 0..8 {
        if text(&app.screen()).contains("● Compare levels") {
            break;
        }
        app.key("pageup");
    }
    let narrow = app.wait_text("● Compare levels");
    expect(
        &narrow,
        &["● Compare levels", TITLE],
        &["○ Compare levels", "Page options", WIDE, CAP_PLAY],
    );
    assert_size(&narrow, 80, 24);
    see(
        &app,
        "sc.chipsselects.compare-levels.80x24",
        "compare levels still on after narrowing to 80 by 24, before restoring 120 by 40",
    );

    click_filtered(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    expect(
        &restored,
        &["○ Compare levels", TITLE],
        &["● Compare levels", "Page options", "256 colors"],
    );
    app.resize(120, 40);
    app.wait_screen("playground width restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(WIDE)
    });
    app.click_text("Playground");
    let playground = app.wait_text(CAP_PLAY);
    expect(
        &playground,
        &[
            CAP_PLAY,
            "created_at",
            "50",
            "PostgreSQL",
            "match all",
            NOTHING,
            PENDING,
        ],
        &[
            "Page options",
            "Mixed",
            "match any",
            "● Compare levels",
            "Sort by →",
        ],
    );
    app.finish();
}

#[test]
fn chips_edit_remove_overflow() {
    let app = page("s02-chips-actions", 120, 40);
    let before = app.wait_text(PENDING);
    expect(
        &before,
        &[
            TITLE,
            CAP_PLAY,
            "match all",
            PENDING,
            TOTAL,
            COUNTRY,
            NOTHING,
            "created_at",
            "50",
        ],
        &[
            "last action: edit",
            "last action: removed",
            "match any",
            "+3",
            "Page options",
        ],
    );
    assert_size(&before, 120, 40);
    see(
        &app,
        "sc.chipsselects.chips.before",
        "filters before edit, remove, toggle, add, and overflow",
    );

    click_main(&app, PENDING);
    let edited = app.wait_text("last action: edit status = 'pending'");
    expect(
        &edited,
        &[
            PENDING,
            TOTAL,
            COUNTRY,
            "last action: edit status = 'pending'",
        ],
        &["last action: removed", "match any", "+3"],
    );

    click_close(&app, PENDING);
    let removed = app.wait_text("last action: removed status = 'pending'");
    expect(
        &removed,
        &[
            TOTAL,
            COUNTRY,
            "last action: removed status = 'pending'",
            "+ Add filter",
        ],
        &[
            "status = 'pending' ×",
            "last action: removed total",
            "last action: removed country",
        ],
    );

    app.key("space");
    let disabled = app.wait_text("last action: disabled total > 100");
    expect(
        &disabled,
        &[TOTAL, COUNTRY, "last action: disabled total > 100"],
        &["status = 'pending' ×", "last action: enabled total > 100"],
    );

    app.key("+");
    let added = app.wait_text("last action: added created_at > '2026-01-01'");
    expect(
        &added,
        &[
            "created_at > '2026-01-01'",
            TOTAL,
            "last action: added created_at > '2026-01-01'",
        ],
        &["status = 'pending' ×", "last action: removed"],
    );

    app.key("m");
    let lead = app.wait_text("match any");
    expect(
        &lead,
        &["match any", "last action: match any", TOTAL],
        &["match all", "status = 'pending' ×"],
    );

    app.key("tab");
    app.key("1");
    let stream = app.wait_text("last action: stream api off");
    expect(
        &stream,
        &["api", "worker", "last action: stream api off"],
        &["last action: stream api on", "status = 'pending' ×"],
    );

    app.click_text("Options");
    let width = app.wait_text("Chip bar width");
    assert_value(&width, "Chip bar width", "full");
    click_filtered(&app, "full", |hit| hit.x > 24);
    app.wait_text("▴");
    app.key("down");
    app.key("down");
    app.key("enter");
    let chosen = app.wait_screen("chip bar width 40", |screen| {
        text(screen).contains("Chip bar width")
            && !text(screen).contains('▴')
            && option_value(screen, "Chip bar width") == "40"
    });
    expect(&chosen, &[MARKER_ON], &[MARKER_OFF]);

    app.click_text("Playground");
    let overflow = app.wait_screen("chip overflow marker", |screen| {
        let body = text(screen);
        body.contains("+1") || body.contains("+2") || body.contains("+3") || body.contains("+4")
    });
    let body = text(&overflow);
    let hidden = [COUNTRY, "amount > 'abc'", "created_at > '2026-01-01'"]
        .into_iter()
        .find(|label| !body.contains(label))
        .unwrap_or_else(|| panic!("bar width 40 hid no chip\n{body}"));
    let marker = ["+4", "+3", "+2", "+1"]
        .into_iter()
        .find(|marker| body.contains(marker))
        .expect("overflow marker");
    expect(
        &overflow,
        &[marker, "match any"],
        &[hidden, "Chip bar width", "status = 'pending' ×"],
    );
    click_main(&app, marker);
    let menu = app.wait_text(hidden);
    expect(
        &menu,
        &[hidden, marker],
        &["status = 'pending' ×", "last action: removed"],
    );
    app.key("escape");
    app.key("X");
    let cleared = app.wait_text("last action: cleared all filters");
    expect(
        &cleared,
        &["last action: cleared all filters", "match any"],
        &[
            TOTAL,
            COUNTRY,
            "status = 'pending' ×",
            "created_at > '2026-01-01'",
        ],
    );
    assert_size(&cleared, 120, 40);
    see(
        &app,
        "sc.chipsselects.chips.after",
        "clear all after removing only the pending chip and opening the overflow menu",
    );
    app.finish();
}

#[test]
fn selects_choose_dismiss_and_disabled() {
    let app = page("s02-chips-selects", 120, 40);
    let before = app.wait_text("created_at");
    expect(
        &before,
        &[
            TITLE,
            CAP_PLAY,
            "created_at",
            "50",
            "PostgreSQL",
            TOTAL,
            NOTHING,
        ],
        &["Sort by →", "Page size →", "Engine →", "▴", "Mixed"],
    );
    assert_size(&before, 120, 40);
    see(
        &app,
        "sc.chipsselects.selects.before",
        "selects closed on created_at, page size 50, and disabled PostgreSQL",
    );

    click_main(&app, "PostgreSQL");
    let engine = app.settle();
    expect(&engine, &["PostgreSQL"], &["Engine →", "▴"]);
    app.click_text("States");
    app.click_text("Playground");
    app.wait_text("created_at");

    click_top_main(&app, "created_at");
    let opened = app.wait_text("▴");
    expect(
        &opened,
        &["▴", "created_at", "total", "50", "PostgreSQL"],
        &["Sort by → total", "Sort by → seats"],
    );
    app.key("down");
    app.key("escape");
    let dismissed = app.wait_absent("▴");
    expect(
        &dismissed,
        &["created_at", "50", "PostgreSQL", TOTAL],
        &["Sort by → total", "Sort by →", "▴", "Mixed"],
    );

    click_top_main(&app, "created_at");
    app.wait_text("▴");
    app.key("down");
    app.key("enter");
    let committed = app.wait_text("Sort by → total");
    expect(
        &committed,
        &["Sort by → total", "50", "PostgreSQL", TOTAL],
        &["created_at", "Page size →", "Engine →", "▴", "Mixed"],
    );

    click_filtered(&app, "50", |hit| hit.x > 24 && hit.y > 12);
    app.wait_text("▴");
    app.key("down");
    app.key("enter");
    let sized = app.wait_text("Page size → 100");
    expect(
        &sized,
        &["Page size → 100", "PostgreSQL", TOTAL],
        &[
            "Page size → 50",
            "Page size → 25",
            "Engine →",
            "created_at",
            "Sort by → total",
        ],
    );

    open_sort(&app);
    for _ in 0..16 {
        if text(&app.screen()).contains("seats") {
            break;
        }
        app.key("down");
    }
    let scrolled = app.wait_text("seats");
    expect(
        &scrolled,
        &["seats", "▴", "Page size → 100"],
        &["Sort by → seats", "Engine →"],
    );
    app.key("escape");
    let kept = app.wait_absent("▴");
    expect(
        &kept,
        &["Page size → 100", "PostgreSQL"],
        &["Sort by → seats", "▴", "Mixed", "created_at"],
    );

    app.click_text("Options");
    let ahead = show_option(&app, "Type-ahead");
    expect(&ahead, &[AHEAD_OFF, MIXED_OFF], &[AHEAD_ON, MIXED_ON]);
    app.click_text("Type-ahead");
    let ahead_on = app.wait_text(AHEAD_ON);
    expect(&ahead_on, &[AHEAD_ON, MIXED_OFF], &[AHEAD_OFF, MIXED_ON]);
    app.click_text("Playground");
    app.wait_text(CAP_PLAY);
    open_sort(&app);
    app.key("s");
    app.key("enter");
    let typed = app.wait_text("Sort by → s");
    expect(
        &typed,
        &["100", "PostgreSQL", TOTAL],
        &[
            "Sort by → total",
            "▴",
            "Mixed",
            "Engine →",
            "Page size → 100",
        ],
    );
    assert!(
        text(&typed).contains("Sort by → status") || text(&typed).contains("Sort by → seats"),
        "type-ahead did not commit a column starting with s\n{}",
        text(&typed)
    );

    app.click_text("Options");
    let mixed = show_option(&app, "Sort shows mixed");
    expect(&mixed, &[MIXED_OFF], &[MIXED_ON]);
    app.click_text("Sort shows mixed");
    app.wait_text(MIXED_ON);
    app.click_text("Playground");
    let mixed_field = app.wait_text("Mixed");
    expect(
        &mixed_field,
        &["Mixed", "100", "PostgreSQL", TOTAL],
        &["Engine →", "created_at", "Page size → 100"],
    );
    click_main(&app, "Mixed");
    app.wait_text("▴");
    app.key("escape");
    let mixed_kept = app.wait_absent("▴");
    expect(
        &mixed_kept,
        &["Mixed", "PostgreSQL"],
        &["▴", "Engine →", "Sort by → seats"],
    );
    click_main(&app, "Mixed");
    app.wait_text("▴");
    app.key("down");
    app.key("enter");
    let after = app.wait_absent("Mixed");
    expect(
        &after,
        &["100", "PostgreSQL", TOTAL, "Sort by →"],
        &[
            "Mixed",
            "Engine →",
            "▴",
            "Page size → 50",
            "Page size → 100",
        ],
    );
    assert_size(&after, 120, 40);
    see(
        &app,
        "sc.chipsselects.selects.after",
        "committed sort after dismiss, type-ahead, scroll, disabled engine, and mixed",
    );
    app.finish();
}

fn boundary_value(name: &str, opt: &str, case_id: &str, label: &str, value: &str, sibling: &str) {
    let app = open(
        name,
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            opt,
            "--opt",
            "shell.focus_on=release",
        ],
    );
    let screen = show_option(&app, label);
    assert_value(&screen, label, value);
    expect(
        &screen,
        &[TITLE, sibling, value],
        &[CAP_PLAY, CAP_STATES, "Compare levels"],
    );
    assert_size(&screen, 120, 40);
    shot_option(&app, &screen, case_id);
    app.finish();
}

fn boundary_toggle(name: &str, opt: &str, case_id: &str, show: &str, hide: &str, sibling: &str) {
    let app = open(
        name,
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            opt,
            "--opt",
            "shell.focus_on=release",
        ],
    );
    let screen = show_option(&app, show);
    expect(
        &screen,
        &[TITLE, show, sibling],
        &[hide, CAP_PLAY, CAP_STATES, "Compare levels"],
    );
    assert_size(&screen, 120, 40);
    shot_option(&app, &screen, case_id);
    app.finish();
}

#[test]
fn option_bar_width_and_top_boundaries() {
    let app = open(
        "s02-chips-bar",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.bar_width=40",
        ],
    );
    let options = show_option(&app, "Chip bar width");
    assert_value(&options, "Chip bar width", "40");
    assert_ne!(option_value(&options, "Chip bar width"), "full");
    expect(&options, &[MARKER_ON, MENU_ON], &[MARKER_OFF, MENU_OFF]);
    app.click_text("Playground");
    let play = app.wait_text("+3");
    expect(
        &play,
        &[
            "+3",
            PENDING,
            "created_at",
            "50",
            "PostgreSQL",
            CAP_PLAY,
            NOTHING,
            "match all",
        ],
        &[COUNTRY, "Page options", "60", "match any", "Mixed"],
    );
    assert_size(&play, 120, 40);
    see(
        &app,
        "opt.chipsselects.bar_width",
        "Chip bar width 40 shows +3 and hides country in (DE, FR); page size stays 50",
    );
    app.finish();

    boundary_toggle(
        "s02-chips-marker",
        "chipsselects.overflow_marker=off",
        "opt.chipsselects.overflow_marker",
        MARKER_OFF,
        MARKER_ON,
        MENU_ON,
    );
    boundary_toggle(
        "s02-chips-menu",
        "chipsselects.overflow_menu=off",
        "opt.chipsselects.overflow_menu",
        MENU_OFF,
        MENU_ON,
        MARKER_ON,
    );
    boundary_value(
        "s02-chips-plate",
        "chipsselects.close_target=1",
        "opt.chipsselects.close_target",
        "× plate width",
        "1",
        MARKER_ON,
    );
    boundary_value(
        "s02-chips-close",
        "chipsselects.close_button=danger",
        "opt.chipsselects.close_button",
        "× button",
        "danger",
        MARKER_ON,
    );
    boundary_value(
        "s02-chips-more",
        "chipsselects.more_button=wide",
        "opt.chipsselects.more_button",
        "+N button",
        "wide",
        MARKER_ON,
    );
    boundary_value(
        "s02-chips-nav",
        "chipsselects.chip_nav=vim",
        "opt.chipsselects.chip_nav",
        "Chip keys",
        "vim",
        MARKER_ON,
    );
    boundary_value(
        "s02-chips-map",
        "chipsselects.chip_keymap=custom",
        "opt.chipsselects.chip_keymap",
        "Chip key map",
        "custom",
        MARKER_ON,
    );
    boundary_toggle(
        "s02-chips-digits",
        "chipsselects.digit_toggles=off",
        "opt.chipsselects.digit_toggles",
        DIGITS_OFF,
        DIGITS_ON,
        MARKER_ON,
    );
    boundary_value(
        "s02-chips-arrows",
        "chipsselects.arrows=step",
        "opt.chipsselects.arrows",
        "Select arrows",
        "step",
        DIGITS_ON,
    );
    boundary_value(
        "s02-chips-wheel",
        "chipsselects.wheel=cursor",
        "opt.chipsselects.wheel",
        "Wheel over the list",
        "cursor",
        DIGITS_ON,
    );
    boundary_value(
        "s02-chips-tab",
        "chipsselects.tab_in_open=close",
        "opt.chipsselects.tab_in_open",
        "Tab in an open list",
        "close",
        DIGITS_ON,
    );
    boundary_value(
        "s02-chips-rows",
        "chipsselects.max_rows=4",
        "opt.chipsselects.max_rows",
        "List rows",
        "4",
        DIGITS_ON,
    );
    boundary_value(
        "s02-chips-place",
        "chipsselects.placement=above",
        "opt.chipsselects.placement",
        "List placement",
        "above",
        DIGITS_ON,
    );
    boundary_value(
        "s02-chips-fit",
        "chipsselects.popup_fit=flush",
        "opt.chipsselects.popup_fit",
        "List fit",
        "flush",
        DIGITS_ON,
    );
    boundary_value(
        "s02-chips-pwidth",
        "chipsselects.popup_width=8-16",
        "opt.chipsselects.popup_width",
        "List width",
        "8-16",
        DIGITS_ON,
    );
}

#[test]
fn option_bottom_boundaries_and_mixed() {
    boundary_value(
        "s02-chips-cursor",
        "chipsselects.cursor_style=bar",
        "opt.chipsselects.cursor_style",
        "List cursor",
        "bar",
        WRAP_OFF,
    );
    boundary_toggle(
        "s02-chips-hover",
        "chipsselects.hover_moves_cursor=off",
        "opt.chipsselects.hover_moves_cursor",
        HOVER_OFF,
        HOVER_ON,
        WRAP_OFF,
    );
    boundary_toggle(
        "s02-chips-drag",
        "chipsselects.press_drag=off",
        "opt.chipsselects.press_drag",
        DRAG_OFF,
        DRAG_ON,
        WRAP_OFF,
    );
    boundary_toggle(
        "s02-chips-label",
        "chipsselects.label_hit=off",
        "opt.chipsselects.label_hit",
        LABEL_OFF,
        LABEL_ON,
        WRAP_OFF,
    );
    boundary_value(
        "s02-chips-track",
        "chipsselects.track_click=jump",
        "opt.chipsselects.track_click",
        "List track click",
        "jump",
        WRAP_OFF,
    );
    boundary_toggle(
        "s02-chips-wrap",
        "chipsselects.wrap=on",
        "opt.chipsselects.wrap",
        WRAP_ON,
        WRAP_OFF,
        AHEAD_OFF,
    );
    boundary_toggle(
        "s02-chips-ahead",
        "chipsselects.type_ahead=on",
        "opt.chipsselects.type_ahead",
        AHEAD_ON,
        AHEAD_OFF,
        WRAP_OFF,
    );
    boundary_toggle(
        "s02-chips-blur",
        "chipsselects.close_on_blur=off",
        "opt.chipsselects.close_on_blur",
        BLUR_OFF,
        BLUR_ON,
        AHEAD_OFF,
    );
    boundary_value(
        "s02-chips-dismiss",
        "chipsselects.dismiss=escape",
        "opt.chipsselects.dismiss",
        "Dismiss",
        "escape",
        AHEAD_OFF,
    );
    boundary_value(
        "s02-chips-selnav",
        "chipsselects.select_nav=vim",
        "opt.chipsselects.select_nav",
        "Select keys",
        "vim",
        MIXED_OFF,
    );

    let app = page("s02-chips-mixed", 120, 40);
    let options = show_option(&app, "Sort shows mixed");
    expect(
        &options,
        &[MIXED_OFF, AHEAD_OFF],
        &[MIXED_ON, AHEAD_ON, CAP_PLAY],
    );
    app.click_text("Sort shows mixed");
    let toggled = app.wait_text(MIXED_ON);
    expect(&toggled, &[MIXED_ON, AHEAD_OFF], &[MIXED_OFF, AHEAD_ON]);
    app.click_text("Playground");
    let play = app.wait_text("Mixed");
    expect(
        &play,
        &[
            "Mixed",
            "50",
            "PostgreSQL",
            PENDING,
            NOTHING,
            CAP_PLAY,
            "match all",
        ],
        &["created_at", "Page options", "Sort by →", MIXED_OFF],
    );
    assert_size(&play, 120, 40);
    see(
        &app,
        "opt.chipsselects.mixed",
        "Sort shows mixed toggled on from the Options control; the field reads Mixed and page size stays 50",
    );
    app.finish();
}

/// Opt-in page wiring (`chipsselects.mutate_open`): `m` flips whether the first
/// filter is removable while a filter menu is open.
#[test]
fn stale_context_menu_closes_when_a_chip_becomes_fixed() {
    let app = open(
        "s02-chips-stale-context",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.mutate_open=on",
        ],
    );
    app.wait_text(NOTHING);
    app.key("tab");
    app.key("tab");
    app.key("shift+f10");
    let open_menu = app.wait_text("Esc Close");
    expect(
        &open_menu,
        &["╭", "Turn off", "Remove", "m Edit chip"],
        &["last action: removed", "This filter stays"],
    );

    app.key("m");
    let closed = app.wait_text("its menu closed");
    expect(
        &closed,
        &["status = 'pending' is now fixed; its menu closed", NOTHING],
        &[
            "╭",
            "Esc Close",
            "last action: removed",
            "status = 'pending' ×",
        ],
    );

    app.key("down");
    app.key("enter");
    let after = app.wait_text("last action: edit status = 'pending'");
    expect(
        &after,
        &["last action: edit status = 'pending'"],
        &["╭", "last action: removed", "status = 'pending' ×"],
    );

    app.key("shift+f10");
    let reopened = app.wait_text("Esc Close");
    expect(
        &reopened,
        &["╭", "Turn off", "Remove"],
        &["last action: removed", "status = 'pending' ×"],
    );
    app.key("escape");
    app.finish();
}

#[test]
fn stale_overflow_menu_closes_when_a_chip_changes() {
    let app = open(
        "s02-chips-stale-overflow",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.bar_width=40",
            "--opt",
            "chipsselects.mutate_open=on",
        ],
    );
    let overflow = app.wait_screen("chip overflow marker", |screen| {
        let body = text(screen);
        body.contains("+1") || body.contains("+2") || body.contains("+3") || body.contains("+4")
    });
    let body = text(&overflow);
    let hidden = [COUNTRY, "amount > 'abc'", "created_at > '2026-01-01'"]
        .into_iter()
        .find(|label| !body.contains(label))
        .unwrap_or_else(|| panic!("bar width 40 hid no chip\n{body}"));
    let marker = ["+4", "+3", "+2", "+1"]
        .into_iter()
        .find(|marker| body.contains(marker))
        .expect("overflow marker");
    click_main(&app, marker);
    let menu = app.wait_text("Esc Close");
    expect(
        &menu,
        &[hidden, "m Edit chip", NOTHING],
        &["last action: removed"],
    );

    app.key("m");
    let closed = app.wait_text("its menu closed");
    expect(
        &closed,
        &[NOTHING, "is now fixed; its menu closed"],
        &[hidden, "╭", "Esc Close", "last action: edit"],
    );

    click_main(&app, marker);
    let fresh = app.wait_text("Esc Close");
    expect(
        &fresh,
        &[hidden],
        &["last action: edit", "last action: removed"],
    );
    app.key("escape");
    app.finish();
}

#[test]
fn option_stress_filters_and_long_label() {
    let items = open(
        "s02-chips-stress-items",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.stress=20k items",
        ],
    );
    let screen = items.wait_text("filter 00001");
    assert!(text(&screen).contains(TITLE), "chips page title missing");
    items.finish();

    let label = open(
        "s02-chips-stress-label",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.stress=65k label",
        ],
    );
    // The 65,536-cell chip comes after the demo filters, so the cursor starts on
    // a chip that fits. The chip and the demo filter that no longer fits sit
    // behind the +2 marker instead of wrapping to a zero-width chip.
    let screen = label.wait_text("+2");
    assert!(
        text(&screen).contains("match all")
            && text(&screen).contains("status = 'pending'")
            && !text(&screen).contains("long label"),
        "long chip should be behind the marker\n{}",
        text(&screen)
    );
    label.finish();
}

/// `chipsselects.add_label` renames the filter bar's add action or removes it
/// (`ChipBar::set_add_label`). The option reads back from the Options control,
/// the renamed action still adds a filter, and without a label no add action or
/// `+` hotkey remains.
#[test]
fn option_filter_add_action() {
    boundary_value(
        "s02-chips-addlabel",
        "chipsselects.add_label=short",
        "opt.chipsselects.add_label",
        "Filter add action",
        "short",
        MIXED_OFF,
    );

    let short = open(
        "s02-chips-addshort",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.add_label=short",
        ],
    );
    short.wait_text(PENDING);
    // The bar is full at 120 columns, so the add action waits behind +N until
    // a filter is removed.
    click_close(&short, PENDING);
    let before = short.wait_text("+ New");
    expect(&before, &[TOTAL, "+ New"], &["+ Add filter"]);
    click_main(&short, "+ New");
    let added = short.wait_text("last action: added created_at > '2026-01-01'");
    expect(&added, &["created_at > '2026-01-01'"], &["+ Add filter"]);
    short.finish();

    let none = open(
        "s02-chips-addnone",
        120,
        40,
        &[
            "--page",
            "chipsselects",
            "--opt",
            "chipsselects.add_label=none",
        ],
    );
    let bare = none.wait_text(PENDING);
    expect(
        &bare,
        &[PENDING, TOTAL, COUNTRY, NOTHING],
        &["+ Add filter", "+ New", "last action: added"],
    );
    none.key("tab");
    none.key("+");
    let still = none.screen();
    expect(
        &still,
        &[PENDING, NOTHING],
        &["last action: added", "+ Add filter", "+ New"],
    );
    see(
        &none,
        "opt.chipsselects.add_label_none_playground",
        "without an add label the bar shows only its chips and a plus key adds nothing",
    );
    none.finish();
}
