//! Progress route, sections, specimens, compare levels, live progress, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s06_progress/` and each case id's dots become underscores.

use tui_test::TerminalSnapshot;

use crate::support::{
    App, Launch, Point, SHOWCASE, find_all, flash_settle, row_text, text, try_launch,
};

const CRUMB: &str = "Feedback › Progress";
const TITLE: &str = "Progress";
const BLURB: &str = "Determinate, indeterminate, an aggregate tally, capacity meters";
const OVERVIEW: &str = "Tokens and principles behind every component";
const CAP_PLAY: &str = "the live widget";
const CAP_OPT: &str = "one control per option";
const CAP_STATES: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const PAGE_ARGS: &[&str] = &["--page", "progress"];
const TALLY: &str = "2 of 7 done";
const PAUSED: &str = "‖";
const STALE: char = '\u{254d}';
const TRACK: char = '\u{2500}';
const FILL: char = '\u{2501}';

const OMITTED: &[&str] = &[
    "hover",
    "pressed",
    "focused",
    "focus + hover",
    "selected",
    "disabled",
    "busy",
    "editing",
];

const DRAWN: &[&str] = &["rest", "empty", "loading", "error", "overflow", "narrow"];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s06_progress/{}", case_id.replace('.', "_"))
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

/// Grid labels sit under the legend. "rest" is also a prefix of "Restart".
fn grid_label(screen: &TerminalSnapshot, label: &str) -> bool {
    find_all(screen, label).into_iter().any(|hit| {
        hit.y >= 9
            && (18..56).contains(&hit.x)
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
    click_where(app, label, |hit| hit.x < 22);
}

fn click_button(app: &App, label: &str) {
    click_where(app, label, |hit| hit.y > 4 && hit.x >= 26);
}

fn reveal_side(app: &App, label: &str) {
    let point = Point { x: 8, y: 16 };
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 22) {
            return;
        }
        app.wheel(point, false, 3);
    }
    for _ in 0..30 {
        let screen = app.settle();
        if find_all(&screen, label).into_iter().any(|hit| hit.x < 22) {
            return;
        }
        app.wheel(point, true, 3);
    }
}

fn reveal_text(app: &App, needle: &str) {
    let point = Point { x: 70, y: 18 };
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

fn glyph_count(line: &str, glyph: char) -> usize {
    line.chars().filter(|ch| *ch == glyph).count()
}

/// Determinate bars always keep the value column. An indeterminate bar matches
/// that track only while reserve is on, and grows by that column when it is off.
fn track_pair(screen: &TerminalSnapshot) -> (usize, usize) {
    (
        glyph_count(&building_row(screen), TRACK),
        glyph_count(&row_with(screen, "Resolving"), TRACK),
    )
}

fn building_row(screen: &TerminalSnapshot) -> String {
    row_with(screen, "Building")
}

fn assert_running(screen: &TerminalSnapshot) {
    let line = building_row(screen);
    // At 80 columns the value column is clipped off the building row.
    if line.contains('%') {
        assert!(line.contains("0%"), "building row {line:?}");
    }
    assert!(
        !line.contains(PAUSED),
        "running build shows the paused mark: {line:?}"
    );
    assert!(
        text(screen).contains("Pause") && !flat(screen).contains("Resume"),
        "running action is not Pause\n{}",
        text(screen)
    );
}

fn assert_paused(screen: &TerminalSnapshot) {
    let line = building_row(screen);
    assert!(
        line.contains("0%") && line.contains(PAUSED),
        "paused build row {line:?}"
    );
    assert!(
        flat(screen).contains("Resume") && !flat(screen).contains("Pause"),
        "paused action is not Resume\n{}",
        text(screen)
    );
}

fn assert_jobs(screen: &TerminalSnapshot) {
    let done = row_with(screen, "Build image");
    assert!(
        done.contains("100%") && done.contains('✓') && !done.contains("Pause"),
        "completed bar {done:?}"
    );
    let failed = row_with(screen, "Integration");
    assert!(
        failed.contains("64%") && failed.contains('!'),
        "error bar {failed:?}"
    );
    assert!(
        row_with(screen, "Warm caches").contains("Warm caches"),
        "indeterminate job missing"
    );
    assert!(
        flat(screen).contains(TALLY)
            && flat(screen).contains("1 paused")
            && flat(screen).contains("1 failed"),
        "tally\n{}",
        text(screen)
    );
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let app = open("s06-route-page", 120, 40, PAGE_ARGS);
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
            "Building",
            "Resolving",
            "Restart",
            "Pause",
            "0%",
            "ticks at 80 ms",
            "Waiting for the test runner",
            TALLY,
            "Deploy checkout-service",
            "Build image",
            "100%",
        ],
        &[
            "Page options",
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            OVERVIEW,
            "Resume",
            "Push to r…",
        ],
    );
    assert_running(&screen);
    assert_jobs(&screen);
    shot(&app, "sc.progress.route");
    app.finish();

    let app = open("s06-route-keys", 120, 40, &[]);
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
        CRUMB,
    ];
    for stop in stops {
        app.key("]");
        app.wait_text(stop);
    }
    let entered = app.wait_text("Building");
    see(
        &entered,
        &[CRUMB, BLURB, "Building", "Pause", "0%"],
        &[OVERVIEW, "Page options", "Overlays › File browser"],
    );
    app.key("[");
    let left = app.wait_text("Overlays › File browser");
    see(
        &left,
        &["Overlays › File browser"],
        &[CRUMB, "Building", BLURB],
    );
    app.key("]");
    let again = app.wait_text(CRUMB);
    see(
        &again,
        &[CRUMB, "Building", "Resolving"],
        &["Overlays › File browser", OVERVIEW],
    );
    click_side(&app, "Overview");
    let overview = app.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[CRUMB, "Building", BLURB]);
    click_side(&app, "Feedback");
    app.wait_text("Empty & error");
    click_side(&app, "Progress");
    let pointed = app.wait_text("Building");
    see(
        &pointed,
        &[CRUMB, BLURB, "Building", "Pause", TALLY],
        &[OVERVIEW, "Page options", "Compare levels"],
    );
    app.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let app = open("s06-sections", 80, 24, PAGE_ARGS);
    let narrow_play = app.wait_text("Building");
    assert_size(&app, 80, 24);
    see(
        &narrow_play,
        &[
            TITLE,
            "Determinate",
            "Playground",
            "Building",
            "Resolving",
            "Pause",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "capacity meters",
            "Resume",
        ],
    );
    assert_running(&narrow_play);
    shot(&app, "sc.progress.playground.80x24");

    click_tab(&app, "Options");
    app.wait_text("Page options");
    flash_settle(&app);
    let narrow_opt = app.settle();
    assert_size(&app, 80, 24);
    assert_select(&narrow_opt, "Label column", "16", &["measured", "10"]);
    assert_toggle(&narrow_opt, "Reserve the value column", "on");
    assert_select(&narrow_opt, "Meter visual", "line", &["block"]);
    assert_select(&narrow_opt, "Stale style", "dashed", &["faint"]);
    see(
        &narrow_opt,
        &[TITLE, "Page options", "Label column", "Stale style"],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Compare levels",
            "Building",
            WIDE,
            "truecolor",
        ],
    );
    shot(&app, "sc.progress.options.80x24");

    click_tab(&app, "States");
    app.wait_text("Compare levels");
    flash_settle(&app);
    let narrow_states = app.settle();
    assert_size(&app, 80, 24);
    for label in DRAWN {
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
            "Bar active",
            "64%",
            "100%",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "● Compare levels",
            "Building",
            WIDE,
            "truecolor",
            LEGEND,
        ],
    );
    for label in OMITTED {
        assert!(
            !grid_label(&narrow_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&narrow_states)
        );
    }
    shot(&app, "sc.progress.states.80x24");

    app.resize(120, 40);
    let wide_states = app.wait_screen("states redraw at 120 by 40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(CAP_STATES)
            && grid_label(screen, "rest")
            && grid_label(screen, "narrow")
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
            "Bar active",
            "Bar done",
            "Meter stale",
            "64%",
            "100%",
            "72%",
        ],
        &[
            CAP_PLAY,
            CAP_OPT,
            "Page options",
            "● Compare levels",
            "Building",
            "256 colors",
        ],
    );
    let rest = row_with(&wide_states, "rest");
    assert!(
        rest.contains(STALE),
        "default stale style is not dashed: {rest:?}"
    );
    for label in OMITTED {
        assert!(
            !grid_label(&wide_states, label),
            "{label} is drawn, but states_grid omits it\n{}",
            text(&wide_states)
        );
    }
    for (case_id, label) in [
        ("sc.progress.specimen.rest", "rest"),
        ("sc.progress.specimen.empty", "empty"),
        ("sc.progress.specimen.loading", "loading"),
        ("sc.progress.specimen.error", "error"),
        ("sc.progress.specimen.overflow", "overflow"),
        ("sc.progress.specimen.narrow", "narrow"),
    ] {
        assert!(
            grid_label(&app.screen(), label),
            "{label} left the states grid\n{}",
            text(&app.screen())
        );
        shot(&app, case_id);
    }
    let overflow = row_with(&app.screen(), "overflow");
    assert!(
        overflow.contains('\u{2013}'),
        "empty-label overflow cell is not the unsupported dash: {overflow:?}"
    );
    assert!(
        flat(&app.screen()).contains("long"),
        "overflow meter value was clipped away\n{}",
        text(&app.screen())
    );
    shot(&app, "sc.progress.states.120x40");
    shot(&app, "sc.progress.states");

    click_tab(&app, "Options");
    app.wait_text(CAP_OPT);
    flash_settle(&app);
    let wide_opt = app.wait_text("Page options");
    assert_size(&app, 120, 40);
    assert_select(&wide_opt, "Label column", "16", &["measured", "10"]);
    assert_toggle(&wide_opt, "Reserve the value column", "on");
    assert_select(&wide_opt, "Meter visual", "line", &["block"]);
    assert_select(&wide_opt, "Stale style", "dashed", &["faint"]);
    see(
        &wide_opt,
        &[CRUMB, CAP_OPT, WIDE, "Page options"],
        &[CAP_PLAY, CAP_STATES, "Compare levels", "Building"],
    );
    shot(&app, "sc.progress.options.120x40");
    shot(&app, "sc.progress.options");

    click_tab(&app, "Playground");
    app.wait_text(CAP_PLAY);
    flash_settle(&app);
    let wide_play = app.wait_text("Building");
    assert_size(&app, 120, 40);
    see(
        &wide_play,
        &[
            CRUMB,
            TITLE,
            BLURB,
            CAP_PLAY,
            WIDE,
            "Building",
            "0%",
            "Pause",
            TALLY,
            "Push to registry",
            "100%",
        ],
        &[
            CAP_OPT,
            CAP_STATES,
            "Page options",
            "Compare levels",
            "Resume",
            "Push to r…",
        ],
    );
    assert_running(&wide_play);
    assert_jobs(&wide_play);
    shot(&app, "sc.progress.playground.120x40");
    shot(&app, "sc.progress.playground");
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let app = open("s06-compare", 120, 40, PAGE_ARGS);
    click_tab(&app, "States");
    app.wait_text("○ Compare levels");
    flash_settle(&app);
    let off = app.wait_text("○ Compare levels");
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
    let point = Point { x: 70, y: 18 };
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
    shot(&app, "sc.progress.compare-levels.120x40");

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
    shot(&app, "sc.progress.compare-levels.80x24");

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
    assert_select(&options, "Label column", "16", &["measured", "10"]);
    assert_toggle(&options, "Reserve the value column", "on");
    assert_select(&options, "Meter visual", "line", &["block"]);
    assert_select(&options, "Stale style", "dashed", &["faint"]);
    see(
        &options,
        &["dashed", "Page options", CAP_OPT, CRUMB],
        &["faint", "Compare levels", OVERVIEW],
    );
    app.finish();
}

#[test]
fn progress_pause_resume_and_restart() {
    let app = open("s06-progress", 120, 40, PAGE_ARGS);
    let before = app.wait_text("Building");
    assert_size(&app, 120, 40);
    assert_running(&before);
    assert_jobs(&before);
    see(
        &before,
        &[
            "Resolving",
            "Waiting for the test runner",
            "Restart",
            "Migrate schema",
            "Notify",
            "0%",
        ],
        &["Resume", "Page options", "Compare levels"],
    );
    shot(&app, "sc.progress.progress.before");

    click_button(&app, "Pause");
    app.wait_text("Resume");
    flash_settle(&app);
    let paused = app.settle();
    assert_paused(&paused);
    assert_jobs(&paused);

    click_button(&app, "Resume");
    app.wait_text("Pause");
    flash_settle(&app);
    let resumed = app.settle();
    assert_running(&resumed);

    click_button(&app, "Restart");
    flash_settle(&app);
    let restarted = app.settle();
    assert_running(&restarted);
    assert!(
        building_row(&restarted).contains("0%"),
        "restart moved the determinate bar\n{}",
        text(&restarted)
    );

    click_button(&app, "Pause");
    app.wait_text("Resume");
    flash_settle(&app);
    let after = app.settle();
    assert_paused(&after);
    assert_jobs(&after);
    see(
        &after,
        &[CRUMB, "Resume", "0%", TALLY, "Build image", "✓"],
        &["Page options", "Compare levels", OVERVIEW],
    );
    shot(&app, "sc.progress.progress.after");
    app.finish();
}

fn confirm_options(app: &App, check: impl Fn(&TerminalSnapshot)) {
    click_tab(app, "Options");
    let screen = app.wait_text("Page options");
    check(&screen);
    click_tab(app, "Playground");
    app.wait_text("Building");
    flash_settle(app);
}

fn finish_option(app: &App, case_id: &str, check: impl Fn(&TerminalSnapshot)) {
    let screen = app.settle();
    check(&screen);
    assert_size(app, 120, 40);
    shot(app, case_id);
    app.finish();
}

#[test]
fn option_label_width_10() {
    let app = page("s06-label", Some("progress.label_width=10"), 120, 40);
    app.wait_text("Building");
    confirm_options(&app, |screen| {
        assert_select(screen, "Label column", "10", &["16", "measured"]);
        assert_toggle(screen, "Reserve the value column", "on");
        assert_select(screen, "Meter visual", "line", &["block"]);
        assert_select(screen, "Stale style", "dashed", &["faint"]);
    });
    reveal_text(&app, "Push to r…");
    finish_option(&app, "opt.progress.label_width", |screen| {
        see(
            screen,
            &["Push to r…", "Build ima…", TALLY],
            &["Push to registry", "Resume", "Page options"],
        );
        let (building, resolving) = track_pair(screen);
        assert_eq!(
            building, resolving,
            "reserve changed with the label width: building {building} resolving {resolving}"
        );
    });
}

#[test]
fn option_reserve_value_off() {
    let app = page("s06-reserve", Some("progress.reserve_value=off"), 120, 40);
    app.wait_text("Resolving");
    confirm_options(&app, |screen| {
        assert_toggle(screen, "Reserve the value column", "off");
        assert_select(screen, "Label column", "16", &["measured", "10"]);
        assert_select(screen, "Stale style", "dashed", &["faint"]);
    });
    finish_option(&app, "opt.progress.reserve_value", |screen| {
        let (building, resolving) = track_pair(screen);
        assert!(
            resolving > building,
            "indeterminate track did not take the value column (building {building}, resolving {resolving})"
        );
        assert!(
            flat(screen).contains("Push to registry") && !flat(screen).contains("Push to r…"),
            "label width changed\n{}",
            text(screen)
        );
        assert_running(screen);
    });
}

#[test]
fn option_meter_visual_block() {
    let app = page("s06-meter", Some("progress.meter_visual=block"), 120, 40);
    app.wait_text("Building");
    confirm_options(&app, |screen| {
        assert_select(screen, "Meter visual", "block", &["line"]);
        assert_select(screen, "Label column", "16", &["measured", "10"]);
        assert_select(screen, "Stale style", "dashed", &["faint"]);
        assert_toggle(screen, "Reserve the value column", "on");
    });
    reveal_text(&app, "38% used");
    finish_option(&app, "opt.progress.meter_visual", |screen| {
        let low = row_with(screen, "Low");
        assert!(
            low.contains("38% used"),
            "block meter dropped its value: {low:?}"
        );
        assert!(
            !low.contains(FILL) && !low.contains(TRACK),
            "block meter still draws a line track: {low:?}"
        );
        assert!(
            flat(screen).contains("Push to registry"),
            "label width changed\n{}",
            text(screen)
        );
        let (building, resolving) = track_pair(screen);
        assert_eq!(
            building, resolving,
            "reserve changed with the meter visual: building {building} resolving {resolving}"
        );
    });
}

#[test]
fn option_stale_style_faint() {
    let app = page("s06-stale", Some("progress.stale_style=faint"), 120, 40);
    app.wait_text("Building");
    confirm_options(&app, |screen| {
        assert_select(screen, "Stale style", "faint", &["dashed"]);
        assert_select(screen, "Meter visual", "line", &["block"]);
        assert_select(screen, "Label column", "16", &["measured", "10"]);
        assert_toggle(screen, "Reserve the value column", "on");
    });
    reveal_text(&app, "76% used");
    finish_option(&app, "opt.progress.stale_style", |screen| {
        let stale = row_with(screen, "76% used");
        assert!(
            !stale.contains(STALE),
            "faint stale meter still draws the dashed glyph: {stale:?}"
        );
        let warning = row_with(screen, "82% · stale warning");
        assert!(
            warning.contains(STALE),
            "the fixed dashed sample changed with the option: {warning:?}"
        );
        let low = row_with(screen, "Low");
        assert!(
            low.contains(FILL) || low.contains(TRACK),
            "meter visual changed with the stale style: {low:?}"
        );
        assert!(
            flat(screen).contains("Push to registry") && !flat(screen).contains("Push to r…"),
            "label width changed\n{}",
            text(screen)
        );
    });
}
