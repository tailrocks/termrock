//! Code editor route, sections, specimens, compare levels, editing, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s08_codeeditor/` and each case id's dots become underscores.

use tui_test::{SnapshotColor, TerminalSnapshot};

use crate::support::{
    App, Launch, Point, SHOWCASE, cell_bg, clusters, find_all, flash_settle, row_text, text,
    try_launch,
};

const CRUMB: &str = "Text & code › Code editor";
const TITLE: &str = "Code editor";
const BLURB: &str = "Blocks, tones, diagnostics and completion; the gutter says where you are";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "codeeditor"];
const FETCH: &str = "pub async fn fetch";
const CLIENT: &str = "fn client() -> Client";
/// click_code hits the first cell of "fn client", so Enter replaces that prefix.
const ACCEPTED: &str = "fetch()fn client()";
const QFETCH: &str = "Qfetch";
const RANGE: &str = "1–21 of 26";
const RANGE_TALL: &str = "1–22 of 26";
const COMMENT: &str = "// Retry";
const BAR: char = '\u{2501}';
const THUMB: char = '\u{2503}';
const TRACK: char = '\u{2506}';
const ELLIPSIS: char = '\u{2026}';
const GUTTER: char = '\u{258e}';
const SAMPLE_GUTTER: char = '\u{258f}';
const SPINNER: char = '\u{280b}';
const PLAIN: SnapshotColor = SnapshotColor::Rgb(255, 255, 255);

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focus + hover",
    "selected",
    "loading",
    "narrow",
];

const DRAWN: &[&str] = &[
    "rest", "focused", "editing", "disabled", "busy", "error", "empty", "overflow",
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s08_codeeditor/{}", case_id.replace('.', "_"))
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

/// The State panel's `Scans  N · M` row: how often the tone and block
/// providers ran, in the order the Providers row names them.
fn scan_counts(screen: &TerminalSnapshot) -> (usize, usize) {
    let row = row_with(screen, "Scans");
    let counts: Vec<usize> = row
        .split("Scans")
        .nth(1)
        .unwrap_or_default()
        .split(" · ")
        .filter_map(|part| part.split_whitespace().next()?.parse().ok())
        .collect();
    match counts[..] {
        [tones, blocks] => (tones, blocks),
        _ => panic!("no provider-run counters in {row:?}"),
    }
}

fn footer(screen: &TerminalSnapshot) -> String {
    row_text(screen, screen.rows().saturating_sub(1))
}

/// The Find button and the "/ Find" hint share the word. The status row does not.
fn find_status(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("Find") && !line.contains("Run"))
        .unwrap_or_else(|| panic!("find status missing\n{}", text(screen)))
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
    // The page blurb also contains "Blocks", and "completion" contains "on".
    let line = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(label) && (line.contains('●') || line.contains('○')))
        .unwrap_or_else(|| panic!("toggle {label:?} missing\n{}", text(screen)));
    let at = line.find(label).expect("label");
    let tail = &line[at + label.len()..];
    let other = if state == "on" { "off" } else { "on" };
    let tokens: Vec<&str> = tail.split_whitespace().collect();
    assert!(
        tokens.contains(&state) && !tokens.contains(&other),
        "{label} row is {line:?}, expected {state}"
    );
}

fn assert_mode(screen: &TerminalSnapshot, mode: &str) {
    let line = row_with(screen, "Mode");
    let other = if mode == "editing" {
        "navigating"
    } else {
        "editing"
    };
    assert!(
        line.contains(mode) && !line.contains(other),
        "mode line {line:?}"
    );
}

/// Specimen labels sit in the grid, to the right of the sidebar.
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (18..70).contains(&hit.x)
            && row_text(screen, u32::from(hit.y)).contains(label)
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
    reveal_side(app, label);
    click_where(app, label, |hit| hit.x < 24);
}

fn click_code(app: &App, label: &str) {
    click_where(app, label, |hit| hit.x > 30 && hit.y > 6);
}

fn reveal_side(app: &App, label: &str) {
    let point = Point { x: 8, y: 16 };
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel(point, false, 3);
    }
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 24) {
            return;
        }
        app.wheel(point, true, 3);
    }
}

fn reveal_text(app: &App, needle: &str) {
    let point = Point { x: 70, y: 20 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, true, 3);
    }
    app.wait_text(needle);
}

fn wheel_until(app: &App, needle: &str) {
    let point = Point { x: 40, y: 12 };
    for _ in 0..80 {
        if text(&app.settle()).contains(needle) {
            return;
        }
        app.wheel(point, false, 3);
    }
    app.wait_text(needle);
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

fn shot(app: &App, case_id: &str) {
    app.visual(&visual_id(case_id));
}

fn fg_of(screen: &TerminalSnapshot, needle: &str) -> SnapshotColor {
    let hit = find_all(screen, needle)
        .into_iter()
        .find(|hit| hit.x > 30)
        .unwrap_or_else(|| panic!("no {needle:?}\n{}", text(screen)));
    screen
        .cell_at(u32::from(hit.x), u32::from(hit.y))
        .unwrap_or_else(|| panic!("missing cell {needle:?}"))
        .fg()
}

fn fg_word(screen: &TerminalSnapshot, row_needle: &str, word: &str) -> SnapshotColor {
    let row = (0..screen.rows())
        .find(|&row| row_text(screen, row).contains(row_needle))
        .unwrap_or_else(|| panic!("row {row_needle:?} missing\n{}", text(screen)));
    let line = row_text(screen, row);
    let byte = line
        .find(word)
        .unwrap_or_else(|| panic!("{word:?} missing on {line:?}"));
    let x = u32::try_from(line[..byte].chars().count()).expect("column");
    screen
        .cell_at(x, row)
        .unwrap_or_else(|| panic!("missing cell {word:?}"))
        .fg()
}

/// The section-tab rule uses the same heavy horizontal glyph as the editor
/// track, on the row directly under the tabs.
fn has_bar(screen: &TerminalSnapshot) -> bool {
    (8..screen.rows()).any(|row| row_text(screen, row).contains(BAR))
}

fn boxed_label(screen: &TerminalSnapshot, label: &str) -> Point {
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        if !line.contains('│') || !line.contains(label) {
            continue;
        }
        let byte = line.find(label).expect("label");
        let x = line[..byte].chars().count();
        return Point {
            x: u16::try_from(x).expect("column"),
            y: u16::try_from(row).expect("row"),
        };
    }
    panic!("popup label {label:?} missing\n{}", text(screen));
}

fn confirm_options(app: &App, label: &str, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    app.wait_text("Page options");
    reveal_text(app, label);
    check(&app.settle());
    click_tab(app, "Playground");
    app.wait_text("Code editor");
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str) {
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

fn focus_editor(app: &App) {
    click_code(app, "fetch");
    app.wait_screen("editor editing", |screen| {
        row_with(screen, "Mode").contains("editing")
    });
}

/// A manual completion trigger on a blank line lists every candidate.
/// Typing "fe" lists only fetch(, which hides row, fit, and vim differences.
fn open_all_completions(app: &App) {
    let screen = app.wait_text("fn client");
    let y = (0..screen.rows())
        .find(|&row| {
            let line = row_text(&screen, row);
            line.contains(" 17") && line.contains(TRACK) && !line.contains("fn")
        })
        .unwrap_or_else(|| panic!("blank line 17 missing\n{}", text(&screen)));
    app.click(Point { x: 42, y: y as u16 });
    flash_settle(app);
    app.key("ctrl+space");
    app.wait_text("Enter Accept");
}

fn open_fetch_completion(app: &App) {
    click_code(app, "fn client");
    flash_settle(app);
    app.type_text("fe");
    app.wait_text("Enter Accept");
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s08-route-page", 120, 40, PAGE_ARGS);
    let screen = app.wait_text(CRUMB);
    assert_size(&app, 120, 40);
    see(
        &screen,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Playground",
            "retry.rs",
            "3 blocks",
            FETCH,
            "Find",
            "Run block",
            "Nothing flagged",
            RANGE,
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "read-only",
            QFETCH,
            "running",
        ],
    );
    assert!(has_bar(&screen), "playground horizontal bar missing");
    assert_mode(&screen, "navigating");
    shot(&app, "sc.codeeditor.route");
    app.finish();

    let app = open("s08-route-keys", 120, 40, &[]);
    app.wait_text(OVERVIEW);
    let stops = [
        "App shell › Chrome",
        "App shell › Status & hints",
        "App shell › Breadcrumbs & rails",
        "App shell › Help & keys",
        "App shell › Quit & leave",
        "App shell › Focus & navigation",
        "Inputs & forms › Buttons",
        "Inputs & forms › Inputs",
        "Inputs & forms › Text areas",
        "Inputs & forms › Chips & selects",
        "Inputs & forms › Forms",
        "Inputs & forms › Checks & radios",
        "Inputs & forms › Toolbars",
        "Inputs & forms › Fixed controls",
        "Tables › Basic table",
        "Tables › Editable",
        "Tables › Data grid",
        "Tables › Selection",
        "Tables › Cell editors",
        "Tables › Columns",
        "Navigation › Lists",
        "Navigation › Trees",
        "Navigation › Sidebars",
        "Navigation › Tabs",
        "Navigation › Menus",
        "Overlays › Dialogs",
        "Overlays › Pickers",
        "Overlays › Dialog kinds",
        "Overlays › Popups & tooltips",
        "Overlays › File browser",
        "Overlays › Fixed dialogs",
        "Feedback › Progress",
        "Feedback › Empty & error",
        "Feedback › Meters",
        "Feedback › Notices",
        "Layout › Panels",
        "Layout › Scrolling",
        "Layout › Splits & stacks",
        "Layout › Master-detail",
        "Layout › Pane tree",
        "Layout › List detail",
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text(FETCH);
    see(
        &entered,
        &[CRUMB, BLURB, FETCH, "3 blocks"],
        &[OVERVIEW, "Page options", "Layout › List detail"],
    );
    app.key("]");
    let left = app.wait_text("Text & code › Diff");
    see(&left, &["Text & code › Diff"], &[CRUMB, FETCH, BLURB]);
    app.key("[");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, FETCH, "Run block"],
        &["Text & code › Diff", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, FETCH, BLURB]);
    click_side(&app, "Text & code");
    app.wait_text("Diff");
    click_side(&app, "Code editor");
    let pointed = app.wait_text(FETCH);
    see(
        &pointed,
        &[CRUMB, BLURB, FETCH, "3 blocks", "Nothing flagged"],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s08-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text(FETCH);
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Blocks, tones",
            "Playground",
            "retry.rs",
            FETCH,
            "3 blocks",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "where you are",
            "Find",
            "Run block",
            "Nothing flagged",
        ],
    );
    shot(&app, "sc.codeeditor.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_toggle(&narrow_opt, "Read-only", "off");
    assert_select(
        &narrow_opt,
        "Edit entry",
        "theme",
        &["immediate", "explicit"],
    );
    assert_toggle(&narrow_opt, "Tab leaves the editor", "off");
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Read-only", "Field keys"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            FETCH,
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.codeeditor.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in ["rest", "focused", "editing", "disabled"] {
        assert!(
            grid_label(&narrow_states, label),
            "narrow states clipped {label}\n{}",
            text(&narrow_states)
        );
    }
    see(
        &narrow_states,
        &[
            TITLE,
            "○ Compare levels",
            "rest",
            "focused",
            "editing",
            "disabled",
            "Tones",
            "Plain",
            "let delay",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            FETCH,
            WIDE,
            "truecolor",
            "where your focus is",
            "Warning",
            "busy",
            "expected an item",
            "Type code",
        ],
    );
    for label in OMITTED {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&narrow_states)
        );
    }
    shot(&app, "sc.codeeditor.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "overflow")
    });
    assert_size(&app, 120, 40);
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
            "Tones",
            "Plain",
            "Warning",
            "unused variable",
            "expected an item",
            "Type code",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            FETCH,
            "256 colors",
        ],
    );
    let focused_row = (0..wide_states.rows())
        .map(|row| row_text(&wide_states, row))
        .find(|line| line.contains("focused") && line.contains("let delay"))
        .unwrap_or_else(|| panic!("focused specimen missing\n{}", text(&wide_states)));
    assert!(
        focused_row.contains(SAMPLE_GUTTER),
        "focused specimen has no sample gutter: {focused_row}"
    );
    assert!(
        !row_with(&wide_states, "disabled").contains(SAMPLE_GUTTER),
        "disabled specimen drew a focus gutter"
    );
    assert!(
        row_with(&wide_states, "busy").contains(SPINNER),
        "busy specimen has no spinner"
    );
    assert!(
        row_with(&wide_states, "overflow").contains("//"),
        "overflow specimen dropped its wide comment"
    );
    for label in DRAWN {
        assert!(
            grid_label(&wide_states, label),
            "{label} is not in the states grid\n{}",
            text(&wide_states)
        );
    }
    for label in OMITTED {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in [
        ("sc.codeeditor.specimen.rest", "rest"),
        ("sc.codeeditor.specimen.focused", "focused"),
        ("sc.codeeditor.specimen.editing", "editing"),
        ("sc.codeeditor.specimen.disabled", "disabled"),
        ("sc.codeeditor.specimen.busy", "busy"),
        ("sc.codeeditor.specimen.error", "error"),
        ("sc.codeeditor.specimen.empty", "empty"),
        ("sc.codeeditor.specimen.overflow", "overflow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    shot(&app, "sc.codeeditor.states.120x40");
    shot(&app, "sc.codeeditor.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_toggle(&wide_opt, "Read-only", "off");
    assert_select(
        &wide_opt,
        "Edit entry",
        "theme",
        &["immediate", "type-to-edit", "explicit"],
    );
    assert_toggle(&wide_opt, "Tab leaves the editor", "off");
    assert_toggle(&wide_opt, "Esc reverts", "off");
    assert_select(&wide_opt, "Field keys", "theme", &["emacs", "desktop"]);
    assert_select(&wide_opt, "Text key map", "preset", &["custom"]);
    assert_select(&wide_opt, "Reading keys", "default", &["vim", "no-page"]);
    assert_select(&wide_opt, "Command keys", "default", &["custom"]);
    assert_select(&wide_opt, "Indent", "4", &["2"]);
    assert_toggle(&wide_opt, "Syntax tones", "on");
    assert_toggle(&wide_opt, "Blocks", "on");
    assert_select(&wide_opt, "Document", "sample", &["empty"]);
    assert_select(&wide_opt, "Horizontal scroll", "theme", &["on", "off"]);
    assert_select(&wide_opt, "Scrollbar gap", "1", &["0", "2"]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", FETCH],
    );
    shot(&app, "sc.codeeditor.options.120x40");
    shot(&app, "sc.codeeditor.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text(FETCH);
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            FETCH,
            "3 blocks",
            "Find",
            "Run block",
            "Nothing flagged",
            RANGE,
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "read-only",
            QFETCH,
            "running",
        ],
    );
    assert!(
        has_bar(&wide_play),
        "wide playground lost the horizontal bar"
    );
    shot(&app, "sc.codeeditor.playground.120x40");
    shot(&app, "sc.codeeditor.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s08-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[CRUMB, "○ Compare levels", "rest", CAP_STATES, "Warning"],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    app.wait_text("● Compare levels");
    flash_settle(&app);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[CRUMB, "● Compare levels", "truecolor", CAP_STATES],
        &["○ Compare levels", "Page options", CAP_PLAY],
    );
    let mut saw_256 = false;
    let mut saw_16 = false;
    let point = Point { x: 70, y: 20 };
    for _ in 0..80 {
        let visible = text(&app.settle());
        saw_256 |= visible.contains("256 colors");
        saw_16 |= visible.contains("16 colors");
        if visible.contains("no color") {
            break;
        }
        app.wheel(point, true, 3);
    }
    let bands = app.settle();
    assert_size(&app, 120, 40);
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
    shot(&app, "sc.codeeditor.compare-levels.120x40");

    app.resize(80, 24);
    // The first 80×24 paint can be the wide scroll clipped to the new size.
    // That frame still says "no color" and has no page chrome. The settled
    // page is back at the top, so the decision has to wait for the chrome.
    app.wait_screen("compare redraw at 80 by 24", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains("States")
            && visible.contains("● Compare levels")
    });
    if !text(&app.settle()).contains("no color") {
        reveal_text(&app, "no color");
    }
    let narrow = app.settle();
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(
        &narrow,
        &[TITLE, "no color"],
        &["Page options", CAP_PLAY, WIDE, "○ Compare levels"],
    );
    shot(&app, "sc.codeeditor.compare-levels.80x24");

    wheel_until(&app, "● Compare levels");
    let top = app.settle();
    see(
        &top,
        &[TITLE, "truecolor", "● Compare levels"],
        &["no color", "Page options"],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 16);
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "truecolor",
            "Page options",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let options = app.wait_text("Page options");
    assert_toggle(&options, "Read-only", "off");
    assert_select(&options, "Edit entry", "theme", &["immediate", "explicit"]);
    assert_toggle(&options, "Syntax tones", "on");
    assert_toggle(&options, "Blocks", "on");
    see(
        &options,
        &["Page options", CAP_OPT, CRUMB, "sample"],
        &["Compare levels", OVERVIEW, "empty"],
    );
    app.finish();
}

#[test]
fn edit_commit_revert_complete_and_run() {
    let app = page("s08-edit", None, 120, 40);
    let before = app.wait_text(FETCH);
    assert_size(&app, 120, 40);
    assert_mode(&before, "navigating");
    see(
        &before,
        &[FETCH, CLIENT, "3 blocks", "Nothing flagged"],
        &[QFETCH, ACCEPTED, "running", "Enter Accept", "read-only"],
    );
    assert!(
        footer(&before).contains("Into page"),
        "sidebar was not focused\n{}",
        footer(&before)
    );
    shot(&app, "sc.codeeditor.edit.before");

    focus_editor(&app);
    flash_settle(&app);
    let editing = app.settle();
    assert_mode(&editing, "editing");
    assert!(
        footer(&editing).contains("Esc Done") && !footer(&editing).contains("Esc Cancel"),
        "default esc footer {}",
        footer(&editing)
    );
    app.type_text("Q");
    let typed = app.wait_text(QFETCH);
    assert_mode(&typed, "editing");
    assert!(
        row_with(&typed, "fetch").contains(QFETCH),
        "typed Q did not land in fetch\n{}",
        text(&typed)
    );

    app.key("esc");
    let committed = app.wait_screen("edit committed", |screen| {
        row_with(screen, "Mode").contains("navigating") && text(screen).contains(QFETCH)
    });
    assert!(
        !footer(&committed).contains("EDIT"),
        "commit left the edit footer {}",
        footer(&committed)
    );

    app.key("i");
    app.wait_screen("edit again", |screen| {
        row_with(screen, "Mode").contains("editing") && text(screen).contains(QFETCH)
    });
    app.key("ctrl+z");
    let undone = app.wait_screen("undo removed Q", |screen| !text(screen).contains(QFETCH));
    assert!(
        text(&undone).contains(FETCH),
        "undo removed the sample\n{}",
        text(&undone)
    );
    app.key("ctrl+shift+z");
    app.wait_text(QFETCH);

    let q = {
        let screen = app.settle();
        find_all(&screen, "Q")
            .into_iter()
            .find(|hit| hit.x > 30 && row_text(&screen, u32::from(hit.y)).contains(QFETCH))
            .expect("Q in Qfetch")
    };
    let before_bg = cell_bg(&app.screen(), q);
    app.key("shift+left");
    let selected = app.settle();
    let after_bg = cell_bg(&selected, q);
    assert_ne!(
        before_bg, after_bg,
        "shift+left did not paint a grapheme selection"
    );
    app.key("alt+w");
    app.paste("Z");
    let pasted = app.wait_screen("paste landed", |screen| {
        row_with(screen, "fetch").contains('Z')
    });
    assert!(
        row_with(&pasted, "fetch").contains('Z'),
        "paste did not insert Z\n{}",
        text(&pasted)
    );

    app.key("esc");
    app.wait_screen("back to reading", |screen| {
        row_with(screen, "Mode").contains("navigating")
    });
    app.key("/");
    app.type_text("delay");
    let found = app.wait_screen("find delay", |screen| {
        let status = find_status(screen);
        status.contains("delay") && !status.contains("closed")
    });
    assert!(
        find_status(&found).contains("delay"),
        "find did not report delay\n{}",
        text(&found)
    );
    app.key("esc");
    app.wait_screen("find closed", |screen| {
        find_status(screen).contains("closed")
    });

    click_code(&app, "fn client");
    flash_settle(&app);
    app.type_text("fe");
    let popup = app.wait_text("Enter Accept");
    assert!(
        text(&popup).contains("ƒ fetch(") && row_with(&popup, "Completion").contains("items"),
        "completion popup did not offer fetch(\n{}",
        text(&popup)
    );
    assert!(
        find_status(&popup).contains("closed"),
        "the fetch popup was the find panel\n{}",
        text(&popup)
    );
    app.key("enter");
    let accepted = app.wait_text(ACCEPTED);
    assert!(
        row_with(&accepted, "Completion").contains("closed"),
        "accept left the popup open\n{}",
        text(&accepted)
    );
    assert!(
        !text(&accepted).contains("Enter Accept"),
        "accept left the completion footer\n{}",
        text(&accepted)
    );

    // The editing footer also says "Run block". The toolbar button is the one above the editor.
    click_where(&app, "Run block", |hit| {
        (5..16).contains(&hit.y) && hit.x >= 26
    });
    app.wait_text("running");
    flash_settle(&app);
    let ran = app.settle();
    assert_size(&app, 120, 40);
    see(
        &ran,
        &["running", ACCEPTED, "Nothing flagged"],
        &[
            "3 blocks",
            "Enter Accept",
            "expected an item",
            "Block ran",
            "read-only",
        ],
    );
    shot(&app, "sc.codeeditor.edit.after");
    app.finish();

    let read_only = page("s08-edit-ro", Some("codeeditor.read_only=on"), 120, 40);
    let screen = read_only.wait_text("read-only");
    see(&screen, &["read-only", FETCH], &["3 blocks", QFETCH]);
    click_code(&read_only, "fetch");
    flash_settle(&read_only);
    read_only.type_text("Q");
    let refused = read_only.settle();
    assert_mode(&refused, "navigating");
    assert!(
        !text(&refused).contains(QFETCH),
        "read-only buffer accepted Q\n{}",
        text(&refused)
    );
    assert!(
        !footer(&refused).contains("i Edit") && !footer(&refused).contains("EDIT"),
        "read-only footer still offers edit {}",
        footer(&refused)
    );
    read_only.finish();
}

#[test]
fn option_read_only_on() {
    let app = page("s08-ro", Some("codeeditor.read_only=on"), 120, 40);
    app.wait_text("read-only");
    confirm_options(&app, "Read-only", |screen| {
        assert_toggle(screen, "Read-only", "on");
        assert_select(screen, "Edit entry", "theme", &["immediate", "explicit"]);
        assert_toggle(screen, "Blocks", "on");
    });
    let screen = app.settle();
    see(
        &screen,
        &["read-only", FETCH],
        &["3 blocks", QFETCH, "i Edit"],
    );
    click_code(&app, "fetch");
    flash_settle(&app);
    app.type_text("Q");
    let refused = app.settle();
    assert_mode(&refused, "navigating");
    assert!(!text(&refused).contains(QFETCH), "read-only inserted Q");
    assert!(
        footer(&refused).contains("Find") && !footer(&refused).contains("EDIT"),
        "read-only footer {}",
        footer(&refused)
    );
    finish_option(&app, "opt.codeeditor.read_only");
}

#[test]
fn option_edit_entry_type_to_edit() {
    let app = page(
        "s08-entry",
        Some("codeeditor.edit_entry=type-to-edit"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Edit entry", |screen| {
        assert_select(
            screen,
            "Edit entry",
            "type-to-edit",
            &["theme", "immediate", "explicit"],
        );
        assert_toggle(screen, "Read-only", "off");
    });
    app.key("tab");
    app.wait_screen("section strip", |screen| footer(screen).contains("Section"));
    app.key("tab");
    let focused = app.wait_screen("editor focused without editing", |screen| {
        footer(screen).contains("i Edit") && row_with(screen, "Mode").contains("navigating")
    });
    assert!(
        !footer(&focused).contains("EDIT"),
        "type-to-edit started editing on focus {}",
        footer(&focused)
    );
    assert!(
        row_with(&focused, COMMENT).contains(GUTTER),
        "focused editor has no gutter on the first line\n{}",
        row_with(&focused, COMMENT)
    );
    app.type_text("Q");
    let ignored = app.settle();
    assert!(!text(&ignored).contains(QFETCH), "type-to-edit inserted Q");
    assert_mode(&ignored, "navigating");
    flash_settle(&app);
    // The resize that clears a press flash leaves the Find button focused.
    if !footer(&app.settle()).contains("i Edit") {
        app.key("shift+tab");
    }
    let focused = app.wait_screen("editor focused after the flash redraw", |screen| {
        footer(screen).contains("i Edit") && row_with(screen, "Mode").contains("navigating")
    });
    assert!(
        row_with(&focused, COMMENT).contains(GUTTER) && !text(&focused).contains(QFETCH),
        "focus began an edit\n{}",
        text(&focused)
    );
    shot(&app, "opt.codeeditor.edit_entry");
    app.key("i");
    let editing = app.wait_screen("i begins edit", |screen| {
        row_with(screen, "Mode").contains("editing")
    });
    assert!(
        text(&editing).contains(FETCH)
            && text(&editing).contains(COMMENT)
            && !row_with(&editing, COMMENT).contains("i//")
            && !text(&editing).contains("ifetch"),
        "i was inserted\n{}",
        text(&editing)
    );
    app.finish();
}

#[test]
fn option_tab_leaves_on() {
    let app = page("s08-tab", Some("codeeditor.tab_leaves=on"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Tab leaves the editor", |screen| {
        assert_toggle(screen, "Tab leaves the editor", "on");
        assert_toggle(screen, "Esc reverts", "off");
    });
    focus_editor(&app);
    flash_settle(&app);
    let editing = app.settle();
    assert!(
        footer(&editing).contains("Tab Next field") && !footer(&editing).contains("Tab Indent"),
        "tab hint {}",
        footer(&editing)
    );
    app.key("tab");
    let left = app.wait_screen("tab left the editor", |screen| {
        footer(screen).contains("Enter Activate") && row_with(screen, "Mode").contains("navigating")
    });
    assert!(
        !footer(&left).contains("EDIT") && !footer(&left).contains("Tab Indent"),
        "tab stayed in the editor {}",
        footer(&left)
    );
    finish_option(&app, "opt.codeeditor.tab_leaves");
}

#[test]
fn option_esc_reverts_on() {
    let app = page("s08-esc", Some("codeeditor.esc_reverts=on"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Esc reverts", |screen| {
        assert_toggle(screen, "Esc reverts", "on");
        assert_toggle(screen, "Tab leaves the editor", "off");
    });
    focus_editor(&app);
    flash_settle(&app);
    app.type_text("Q");
    let typed = app.wait_text(QFETCH);
    assert!(
        footer(&typed).contains("Esc Cancel") && !footer(&typed).contains("Esc Done"),
        "esc hint {}",
        footer(&typed)
    );
    app.key("esc");
    app.wait_screen("reverted to navigating", |screen| {
        row_with(screen, "Mode").contains("navigating")
    });
    app.key("home");
    let restored = app.wait_text(FETCH);
    assert!(
        !text(&restored).contains(QFETCH),
        "esc kept the typed Q\n{}",
        text(&restored)
    );
    finish_option(&app, "opt.codeeditor.esc_reverts");
}

#[test]
fn option_field_keys_desktop() {
    let app = page(
        "s08-desktop",
        Some("codeeditor.field_keys=desktop"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Field keys", |screen| {
        assert_select(screen, "Field keys", "desktop", &["theme", "emacs"]);
        assert_select(screen, "Text key map", "preset", &["custom"]);
    });
    focus_editor(&app);
    flash_settle(&app);
    app.key("ctrl+a");
    let selected = app.wait_text("ln 26");
    assert!(
        row_with(&selected, "Cursor").contains("ln 26 · col 1"),
        "desktop ctrl+a did not select to the end\n{}",
        text(&selected)
    );
    assert_mode(&selected, "editing");
    assert!(
        text(&selected).contains("log::warn") && text(&selected).contains("delay"),
        "select-all deleted the buffer\n{}",
        text(&selected)
    );
    finish_option(&app, "opt.codeeditor.field_keys");
}

#[test]
fn option_text_keymap_custom() {
    let app = page(
        "s08-textkeys",
        Some("codeeditor.text_keymap=custom"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Text key map", |screen| {
        assert_select(screen, "Text key map", "custom", &["preset"]);
        assert_select(screen, "Field keys", "theme", &["emacs", "desktop"]);
    });
    focus_editor(&app);
    flash_settle(&app);
    app.key("alt+a");
    let selected = app.wait_text("ln 26");
    assert!(
        row_with(&selected, "Cursor").contains("ln 26 · col 1"),
        "custom alt+a did not select all\n{}",
        text(&selected)
    );
    assert!(
        text(&selected).contains("log::warn"),
        "select-all deleted the buffer\n{}",
        text(&selected)
    );
    finish_option(&app, "opt.codeeditor.text_keymap");
}

#[test]
fn option_nav_vim() {
    let app = page("s08-vim", Some("codeeditor.nav=vim"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Reading keys", |screen| {
        assert_select(screen, "Reading keys", "vim", &["default", "no-page"]);
        assert_select(screen, "Command keys", "default", &["custom"]);
    });
    focus_editor(&app);
    app.key("esc");
    flash_settle(&app);
    let reading = app.wait_screen("reading", |screen| {
        row_with(screen, "Mode").contains("navigating")
    });
    assert!(
        footer(&reading).contains("i Edit") && footer(&reading).contains("/ Find"),
        "nav footer {}",
        footer(&reading)
    );
    app.key("j");
    let moved = app.settle();
    assert_mode(&moved, "navigating");
    assert!(
        row_with(&moved, "let mut delay").contains(GUTTER),
        "vim j did not move the caret\n{}",
        text(&moved)
    );
    assert!(
        !footer(&moved).contains("EDIT"),
        "vim j entered editing {}",
        footer(&moved)
    );
    finish_option(&app, "opt.codeeditor.nav");
}

#[test]
fn option_keymap_custom() {
    let app = page("s08-keys", Some("codeeditor.keymap=custom"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Command keys", |screen| {
        assert_select(screen, "Command keys", "custom", &["default"]);
        assert_select(screen, "Reading keys", "default", &["vim", "no-page"]);
    });
    focus_editor(&app);
    app.key("esc");
    flash_settle(&app);
    let reading = app.wait_screen("custom nav", |screen| {
        footer(screen).contains("Ctrl+F Find") && row_with(screen, "Mode").contains("navigating")
    });
    assert!(
        !footer(&reading).contains("/ Find"),
        "custom keymap still hints / Find {}",
        footer(&reading)
    );
    assert!(
        row_with(&reading, FETCH).contains("fn fetch"),
        "esc changed the buffer\n{}",
        text(&reading)
    );
    app.key("i");
    let inspector = app.wait_text("mouse");
    assert!(
        text(&inspector).contains(FETCH) && !text(&inspector).contains("ifetch"),
        "custom i inserted a letter or dropped the sample\n{}",
        text(&inspector)
    );
    app.key("i");
    let closed = app.wait_screen("inspector closed", |screen| !text(screen).contains("mouse"));
    assert!(
        footer(&closed).contains("Ctrl+F Find"),
        "custom footer after inspector {}",
        footer(&closed)
    );
    finish_option(&app, "opt.codeeditor.keymap");
}

#[test]
fn option_indent_2() {
    let app = page("s08-indent", Some("codeeditor.indent=2"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Indent", |screen| {
        assert_select(screen, "Indent", "2", &["4"]);
        assert_toggle(screen, "Syntax tones", "on");
    });
    focus_editor(&app);
    flash_settle(&app);
    app.key("tab");
    let indented = app.wait_screen("two-space indent", |screen| {
        row_with(screen, "fetch").contains("fn   fetch")
    });
    let line = row_with(&indented, "fetch");
    assert!(
        line.contains("fn   fetch") && !line.contains("fn    fetch"),
        "indent was not two spaces: {line:?}"
    );
    assert_mode(&indented, "editing");
    finish_option(&app, "opt.codeeditor.indent");
}

#[test]
fn option_highlighter_off() {
    let app = page("s08-plain", Some("codeeditor.highlighter=off"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Syntax tones", |screen| {
        assert_toggle(screen, "Syntax tones", "off");
        assert_toggle(screen, "Blocks", "on");
    });
    let screen = app.settle();
    let comment = fg_of(&screen, "//");
    let keyword = fg_of(&screen, "pub");
    let binding = fg_word(&screen, "let mut delay", "let");
    assert_eq!(comment, PLAIN, "comment fg {comment:?}");
    assert_eq!(keyword, PLAIN, "pub fg {keyword:?}");
    assert_eq!(binding, PLAIN, "let fg {binding:?}");
    see(&screen, &[FETCH, "3 blocks", COMMENT], &["0 blocks"]);
    finish_option(&app, "opt.codeeditor.highlighter");
}

#[test]
fn option_segmenter_off() {
    let app = page("s08-blocks", Some("codeeditor.segmenter=off"), 120, 40);
    app.wait_text("0 blocks");
    confirm_options(&app, "Blocks", |screen| {
        assert_toggle(screen, "Blocks", "off");
        assert_toggle(screen, "Syntax tones", "on");
    });
    let screen = app.settle();
    see(
        &screen,
        &["0 blocks", "between blocks", FETCH],
        &["3 blocks", "1 of 3"],
    );
    finish_option(&app, "opt.codeeditor.segmenter");
}

#[test]
fn option_document_empty() {
    let app = page("s08-empty", Some("codeeditor.document=empty"), 120, 40);
    app.wait_text("Type code");
    confirm_options(&app, "Document", |screen| {
        assert_select(screen, "Document", "empty", &["sample"]);
        assert_toggle(screen, "Blocks", "on");
    });
    let screen = app.settle();
    see(
        &screen,
        &["Type code", "0 blocks", "between blocks"],
        &[FETCH, "fetch(", "3 blocks", RANGE],
    );
    finish_option(&app, "opt.codeeditor.document");
}

#[test]
fn option_document_large() {
    let app = page("s08-large", Some("codeeditor.document=large"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Document", |screen| {
        assert_select(screen, "Document", "large", &["sample", "empty"]);
    });
    let screen = app.settle();
    see(&screen, &[FETCH, "1–21 of 20001"], &[RANGE, "Type code"]);
    app.finish();
}

#[test]
fn option_horizontal_off() {
    let app = page("s08-hscroll", Some("codeeditor.horizontal=off"), 120, 40);
    app.wait_text(RANGE_TALL);
    confirm_options(&app, "Horizontal scroll", |screen| {
        assert_select(screen, "Horizontal scroll", "off", &["theme", "on"]);
        assert_select(screen, "Scrollbar gap", "1", &["0", "2"]);
    });
    let screen = app.settle();
    assert!(
        !has_bar(&screen),
        "horizontal bar stayed\n{}",
        text(&screen)
    );
    see(&screen, &[RANGE_TALL, FETCH, "of 26"], &[RANGE]);
    finish_option(&app, "opt.codeeditor.horizontal");
}

#[test]
fn option_scrollbar_gap_0() {
    let app = page("s08-gap", Some("codeeditor.scrollbar_gap=0"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Scrollbar gap", |screen| {
        assert_select(screen, "Scrollbar gap", "0", &["1", "2"]);
        assert_select(screen, "Horizontal scroll", "theme", &["on", "off"]);
    });
    let screen = app.settle();
    let line = row_with(&screen, "Result");
    assert!(
        line.contains("Body, …") && line.contains('…') && line.contains(THUMB),
        "gap 0 clip {line:?}"
    );
    assert!(
        line.contains(&format!("{ELLIPSIS}{THUMB}")),
        "gap 0 still leaves a cell between the ellipsis and the thumb: {line:?}"
    );
    see(&screen, &[RANGE, "3 blocks"], &["0 blocks"]);
    finish_option(&app, "opt.codeeditor.scrollbar_gap");
}

#[test]
fn option_track_click_jump() {
    let app = page("s08-track", Some("codeeditor.track_click=jump"), 120, 40);
    app.wait_text(RANGE);
    confirm_options(&app, "Track click", |screen| {
        assert_select(screen, "Track click", "jump", &["page"]);
        assert_select(screen, "Scrollbar gap", "1", &["0", "2"]);
    });
    let screen = app.wait_text(COMMENT);
    let track_x = (0..screen.rows())
        .find_map(|row| {
            let line = row_text(&screen, row);
            if !line.contains(FETCH) {
                return None;
            }
            line.chars()
                .enumerate()
                .find_map(|(x, ch)| (ch == THUMB && x > 60).then_some(x))
        })
        .unwrap_or_else(|| panic!("editor thumb missing\n{}", text(&screen)));
    let track_y = (0..screen.rows())
        .rev()
        .find(|row| {
            row_text(&screen, *row)
                .chars()
                .nth(track_x)
                .is_some_and(|ch| ch == TRACK)
        })
        .unwrap_or_else(|| panic!("editor track missing\n{}", text(&screen)));
    app.click(Point {
        x: u16::try_from(track_x).expect("column"),
        y: u16::try_from(track_y).expect("row"),
    });
    flash_settle(&app);
    let jumped = app.settle();
    assert!(
        !text(&jumped).contains(COMMENT) && text(&jumped).contains("backoff"),
        "jump on the track below the thumb left the top of the sample\n{}",
        text(&jumped)
    );
    finish_option(&app, "opt.codeeditor.track_click");
}

#[test]
fn option_completion_rows_4() {
    let app = page("s08-rows", Some("codeeditor.completion_rows=4"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Completion rows", |screen| {
        assert_select(screen, "Completion rows", "4", &["8"]);
        assert_select(screen, "Completion width", "default", &["wide"]);
    });
    open_all_completions(&app);
    flash_settle(&app);
    let popup = app.wait_text("Enter Accept");
    see(
        &popup,
        &[
            "T Body",
            "T Client",
            "Option<T>",
            "22 items",
            "Enter Accept",
        ],
        &[
            "assert_eq!",
            "println!",
            "Result<T",
            "Vec<T>",
            "Find         type",
        ],
    );
    assert!(
        (0..popup.rows()).any(|row| {
            let line = row_text(&popup, row);
            line.contains("Find") && line.contains("closed")
        }),
        "rows popup is the find panel\n{}",
        text(&popup)
    );
    finish_option(&app, "opt.codeeditor.completion_rows");
}

#[test]
fn option_completion_width_wide() {
    let app = page(
        "s08-wide",
        Some("codeeditor.completion_width=wide"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Completion width", |screen| {
        assert_select(screen, "Completion width", "wide", &["default"]);
        assert_select(screen, "Completion rows", "8", &["4"]);
    });
    open_fetch_completion(&app);
    flash_settle(&app);
    let popup = app.wait_text("async fn (url");
    assert!(
        text(&popup).contains("ƒ fetch(") && text(&popup).contains("async fn (url"),
        "wide popup hid the detail\n{}",
        text(&popup)
    );
    finish_option(&app, "opt.codeeditor.completion_width");
}

#[test]
fn option_completion_placement_above() {
    let app = page(
        "s08-above",
        Some("codeeditor.completion_placement=above"),
        120,
        40,
    );
    app.wait_text(FETCH);
    click_tab(&app, "Options");
    app.wait_text("Page options");
    reveal_text(&app, "above");
    assert_select(&app.settle(), "Completion placement", "above", &["below"]);
    click_tab(&app, "Playground");
    app.wait_text("Code editor");
    flash_settle(&app);
    open_fetch_completion(&app);
    flash_settle(&app);
    let popup = app.wait_text("Enter Accept");
    let caret = (0..popup.rows())
        .find(|&row| row_text(&popup, row).contains("fefn client"))
        .unwrap_or_else(|| panic!("typed prefix missing\n{}", text(&popup)));
    let bottom = (0..popup.rows())
        .find(|&row| row_text(&popup, row).contains('╰'))
        .unwrap_or_else(|| panic!("popup bottom missing\n{}", text(&popup)));
    assert_eq!(
        bottom + 1,
        caret,
        "above popup did not sit on the rows above the caret\n{}",
        text(&popup)
    );
    assert!(
        text(&popup).contains("ƒ fetch(") && !row_text(&popup, caret + 1).contains('╭'),
        "above popup opened below the caret\n{}",
        text(&popup)
    );
    finish_option(&app, "opt.codeeditor.completion_placement");
}

#[test]
fn option_completion_fit_flush() {
    let app = page(
        "s08-flush",
        Some("codeeditor.completion_fit=flush"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Completion fit", |screen| {
        assert_select(screen, "Completion fit", "flush", &["default"]);
        assert_select(screen, "Completion placement", "below", &["above"]);
    });
    focus_editor(&app);
    flash_settle(&app);
    app.key("ctrl+e");
    let ended = app.wait_screen("caret at the end of the long line", |screen| {
        (0..screen.rows()).any(|row| {
            let line = row_text(screen, row);
            line.contains(ELLIPSIS) && line.contains("Error")
        })
    });
    assert!(
        row_with(&ended, "Cursor").contains("ln 2"),
        "end did not stay on the fetch line\n{}",
        text(&ended)
    );
    app.key("ctrl+space");
    let popup = app.wait_text("Enter Accept");
    // Caret follow keeps four columns of context, so this anchor is also where
    // the default one-column margin clamps a 48-wide popup. Flush does not move it.
    assert!(
        text(&popup).contains("Option<T>") && text(&popup).contains("assert_eq!"),
        "end-of-line completion was not the full list\n{}",
        text(&popup)
    );
    assert!(
        (0..popup.rows()).any(|row| row_text(&popup, row).contains('╮')),
        "flush popup did not draw a box\n{}",
        text(&popup)
    );
    finish_option(&app, "opt.codeeditor.completion_fit");
}

#[test]
fn option_completion_cursor_bar() {
    let app = page("s08-bar", Some("codeeditor.completion_cursor=bar"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Completion cursor", |screen| {
        assert_select(screen, "Completion cursor", "bar", &["fill"]);
    });
    open_fetch_completion(&app);
    flash_settle(&app);
    let popup = app.wait_text("Enter Accept");
    assert!(
        text(&popup).contains("▎ƒ"),
        "bar cursor did not draw ▎ before the glyph\n{}",
        text(&popup)
    );
    finish_option(&app, "opt.codeeditor.completion_cursor");
}

#[test]
fn option_completion_dismiss_escape() {
    let app = page(
        "s08-dismiss",
        Some("codeeditor.completion_dismiss=escape"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Completion dismiss", |screen| {
        assert_select(screen, "Completion dismiss", "escape", &["light"]);
    });
    open_fetch_completion(&app);
    let open = app.wait_text("Enter Accept");
    assert!(text(&open).contains("ƒ fetch("));
    app.click(Point { x: 90, y: 12 });
    flash_settle(&app);
    let stayed = app.settle();
    assert!(
        text(&stayed).contains("Enter Accept") && text(&stayed).contains("ƒ fetch("),
        "escape dismiss closed on an outside click\n{}",
        text(&stayed)
    );
    assert!(
        row_with(&stayed, "Completion").contains("items"),
        "completion count cleared\n{}",
        text(&stayed)
    );
    finish_option(&app, "opt.codeeditor.completion_dismiss");
}

#[test]
fn option_completion_nav_vim() {
    let app = page("s08-cvim", Some("codeeditor.completion_nav=vim"), 120, 40);
    app.wait_text(FETCH);
    confirm_options(&app, "Completion keys", |screen| {
        assert_select(screen, "Completion keys", "vim", &["default"]);
        assert_select(screen, "Reading keys", "default", &["vim", "no-page"]);
    });
    open_all_completions(&app);
    let popup = app.wait_text("Option<T>");
    let hit = boxed_label(&popup, "Body");
    let before = cell_bg(&popup, hit);
    app.key("j");
    let moved = app.settle();
    let after = cell_bg(&moved, hit);
    assert_ne!(before, after, "vim j did not move the completion cursor");
    assert!(
        text(&moved).contains("Enter Accept")
            && text(&moved).contains("T Body")
            && text(&moved).contains("T Client")
            && !text(&moved).contains("jBody")
            && !row_with(&moved, "fn ").contains("jfn"),
        "vim j inserted a letter\n{}",
        text(&moved)
    );
    flash_settle(&app);
    let stayed = app.settle();
    assert!(
        text(&stayed).contains("Enter Accept"),
        "vim completion closed before the shot\n{}",
        text(&stayed)
    );
    finish_option(&app, "opt.codeeditor.completion_nav");
}

/// The paged source opens with ten candidates and queues twelve. Ctrl+L
/// appends them through `Completion::set_items`, which keeps the highlighted
/// row; the State panel reads the eligibility and the popup snapshot back.
#[test]
fn option_completion_source_paged_loads_more_in_place() {
    let app = page(
        "s08-paged",
        Some("codeeditor.completion_source=paged"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Completion source", |screen| {
        assert_select(screen, "Completion source", "paged", &["all"]);
    });
    open_all_completions(&app);
    let first = app.wait_text("Ctrl+L More");
    see(
        &first,
        &[
            "10 items",
            "Eligible",
            "1–8 of 10 ↓ · 0 blocked · 12 queued",
            "Enter Accept",
            "Ctrl+L More",
        ],
        &["22 items"],
    );
    assert!(
        row_with(&first, "Eligible").contains("yes"),
        "an enabled row is eligible\n{}",
        text(&first)
    );
    app.key("down");
    app.key("down");
    app.key("ctrl+l");
    let more = app.wait_text("22 items");
    see(
        &more,
        &["1–8 of 22 ↓ · 0 blocked · 0 queued", "Enter Accept"],
        &["Ctrl+L More", "10 items"],
    );
    flash_settle(&app);
    let stayed = app.settle();
    assert!(
        text(&stayed).contains("22 items") && text(&stayed).contains("Enter Accept"),
        "the popup stayed open after loading more\n{}",
        text(&stayed)
    );
    finish_option(&app, "opt.codeeditor.completion_source");

    let all = page("s08-all", None, 120, 40);
    all.wait_text(FETCH);
    open_all_completions(&all);
    let screen = all.wait_text("22 items");
    see(
        &screen,
        &["22 items", "Enter Accept"],
        &["Ctrl+L More", "Popup rows", "Eligible"],
    );
    all.finish();
}

/// The test terminal keeps only the first scalars of a ZWJ or skin-tone
/// sequence, so those labels are matched by their visible head.
#[test]
fn option_completion_international_items_paint_whole_clusters() {
    let app = page(
        "s08-intl",
        Some("codeeditor.completion_international=on"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "International completions", |screen| {
        assert_toggle(screen, "International completions", "on");
        assert_select(screen, "Completion rows", "8", &["4"]);
    });
    click_code(&app, "fn client");
    flash_settle(&app);
    app.type_text("go");
    let popup = app.wait_text("Enter Accept");
    for (label, kind) in [
        ("go_cafe\u{301}(", "combining mark"),
        ("go_界A(", "wide + ASCII"),
        ("go_שלום(", "right-to-left"),
        ("go_👨\u{200d}(", "ZWJ family"),
        ("go_🇯🇵(", "flag"),
        ("go_👍(", "skin tone"),
    ] {
        let line = row_with(&popup, label);
        assert!(
            line.contains(kind),
            "the detail {kind:?} was overwritten or pushed off the {label:?} row: {line:?}\n{}",
            text(&popup)
        );
    }
    assert!(
        row_with(&popup, "Completion").contains("6 items"),
        "the popup should offer the six international items\n{}",
        text(&popup)
    );
    shot(&app, "opt.codeeditor.completion_international");

    for (label, expected) in [
        (
            "go_界A(",
            vec![
                ("g", 1, true),
                ("o", 1, true),
                ("_", 1, false),
                ("界", 2, false),
                ("A", 1, false),
                ("(", 1, false),
            ],
        ),
        (
            "go_שלום(",
            vec![
                ("g", 1, true),
                ("o", 1, true),
                ("_", 1, false),
                ("ש", 1, false),
                ("ל", 1, false),
                ("ו", 1, false),
                ("ם", 1, false),
                ("(", 1, false),
            ],
        ),
    ] {
        let at = find_all(&popup, label)
            .into_iter()
            .find(|hit| hit.y > 8)
            .unwrap_or_else(|| panic!("{label:?} missing\n{}", text(&popup)));
        let cells: Vec<(String, u16, bool)> = clusters(&popup, at, expected.len())
            .into_iter()
            .map(|c| (c.text, c.width, c.bold))
            .collect();
        let expected: Vec<(String, u16, bool)> = expected
            .into_iter()
            .map(|(t, w, b)| (t.to_string(), w, b))
            .collect();
        assert_eq!(
            cells,
            expected,
            "{label:?} keeps the typed go emphasised and each cluster whole\n{}",
            text(&popup)
        );
    }

    app.type_text("cafe");
    app.wait_screen("go_cafe narrows to one item", |screen| {
        row_with(screen, "Completion").contains("1 items")
    });
    app.key("enter");
    let accepted = app.wait_screen("international completion accepted", |screen| {
        row_with(screen, "Completion").contains("closed")
    });
    assert!(
        text(&accepted).contains("go_cafe\u{301}("),
        "accepting inserts the whole cluster\n{}",
        text(&accepted)
    );
    app.finish();
}

#[test]
fn option_find_preset_marks_the_chosen_match() {
    let app = page(
        "s08-findpre",
        Some("codeeditor.find_preset=delay-3rd"),
        120,
        40,
    );
    app.wait_text(FETCH);
    confirm_options(&app, "Find query", |screen| {
        assert_select(screen, "Find query", "delay-3rd", &["missing"]);
    });
    let screen = app.settle();
    let find = find_status(&screen);
    assert!(
        find.contains("“delay” · 3 of 3"),
        "the host-set query did not mark the third match: {find:?}"
    );
    let width = row_with(&screen, "Line width");
    assert!(
        width.contains("cells"),
        "State shows no line width: {width:?}"
    );
    assert_mode(&screen, "navigating");
    finish_option(&app, "opt.codeeditor.find_preset");
}

#[test]
fn option_find_preset_missing_reports_no_matches() {
    let app = page(
        "s08-findmiss",
        Some("codeeditor.find_preset=missing"),
        120,
        40,
    );
    app.wait_text(FETCH);
    let screen = app.wait_screen("no matches", |screen| {
        find_status(screen).contains("no matches")
    });
    let find = find_status(&screen);
    assert!(
        find.contains("“missing” · no matches"),
        "a query without matches is not reported: {find:?}"
    );
    finish_option(&app, "opt.codeeditor.find_preset_missing");
}

#[test]
fn rescan_key_invalidates_the_installed_providers() {
    let app = page("s08-rescan", None, 120, 40);
    app.wait_text(FETCH);
    let state = app.settle();
    assert!(
        row_with(&state, "Providers").contains("tones · blocks"),
        "both providers are installed by default\n{}",
        text(&state)
    );
    let (tones_before, blocks_before) = scan_counts(&state);
    focus_editor(&app);
    app.key("f5");
    app.wait_text("Re-scanned tones and blocks");
    let rescanned = app.settle();
    let (tones_after, blocks_after) = scan_counts(&rescanned);
    assert!(
        tones_after > tones_before && blocks_after > blocks_before,
        "F5 did not run the providers again: tones {tones_before} -> {tones_after}, blocks {blocks_before} -> {blocks_after}\n{}",
        text(&rescanned)
    );
    assert!(
        footer(&rescanned).contains("Re-scanned tones and blocks"),
        "the status line does not say what was re-scanned\n{}",
        text(&rescanned)
    );
    assert!(
        text(&rescanned).contains("3 blocks") && text(&rescanned).contains(FETCH),
        "the re-scan lost the blocks or the text\n{}",
        text(&rescanned)
    );
    assert_ne!(
        fg_of(&rescanned, "pub"),
        PLAIN,
        "the re-scan dropped the syntax tones"
    );
    app.finish();
}

#[test]
fn rescan_key_skips_a_provider_that_is_off() {
    let app = page(
        "s08-rescan-off",
        Some("codeeditor.highlighter=off"),
        120,
        40,
    );
    app.wait_text(FETCH);
    let state = app.settle();
    assert!(
        row_with(&state, "Providers").contains("blocks")
            && !row_with(&state, "Providers").contains("tones"),
        "only the segmenter is installed\n{}",
        text(&state)
    );
    focus_editor(&app);
    app.key("f5");
    let rescanned = app.wait_text("Re-scanned blocks");
    assert!(
        !footer(&rescanned).contains("tones"),
        "the status names a provider that is off\n{}",
        text(&rescanned)
    );
    finish_option(&app, "opt.codeeditor.rescan_segmenter_only");
}
