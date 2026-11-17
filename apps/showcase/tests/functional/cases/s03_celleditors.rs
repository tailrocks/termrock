//! Cell editors route, sections, specimens, compare levels, choices, and options.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/s03_celleditors/` and each case id's dots become underscores.

use std::time::{Duration, Instant};

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, Launch, Point, SHOWCASE, find_all, row_text, text, try_launch};

const TITLE: &str = "Tables › Cell editors";
const BLURB: &str = "Columns that only take values from a fixed set";
const BLURB_NARROW: &str = "Columns that only take values from a fixed …";
const OVERVIEW: &str = "Tokens and principles behind every component";
const SELECTION: &str = "Selectable table";
const PLAY: &str = "the live widget";
const OPT: &str = "one control per option";
const STATE: &str = "every state, drawn by the widget";
const WIDE: &str = "120×40";
const LEGEND: &str = "▏ depicts a focused sample; it is not where your focus is";
const LEGEND_NARROW: &str = "▏ depicts a focused sample; it is no";
const META: &str = "0 edits · 3 values outside the set";
const META_NARROW: &str = "0 edits · 3 values";
const SAVED: &str = "Env saved";
const CANCELLED: &str = "Edit cancelled";
const CHOOSING: &str = "Esc Cancel";
const OUTSIDE: &str = "7 values outside the set";

const TOP_SPECIMENS: &[&str] = &[
    "sc.celleditors.specimen.rest",
    "sc.celleditors.specimen.hover",
    "sc.celleditors.specimen.focused",
    "sc.celleditors.specimen.editing",
    "sc.celleditors.specimen.error",
    "sc.celleditors.specimen.disabled",
    "sc.celleditors.specimen.empty",
];

fn visual_id(case_id: &str) -> String {
    format!("pre_refactor/s03_celleditors/{}", case_id.replace('.', "_"))
}

fn open(name: &str, cols: u16, rows: u16, args: &[&str]) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.args = args.iter().copied().map(str::to_string).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn page(name: &str, opt: Option<&str>, cols: u16, rows: u16) -> App {
    let mut args = vec!["--page", "celleditors"];
    if let Some(value) = opt {
        args.push("--opt");
        args.push(value);
    }
    open(name, cols, rows, &args)
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

fn lines_with(screen: &TerminalSnapshot, needle: &str) -> Vec<String> {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .filter(|line| line.contains(needle))
        .collect()
}

fn popup_blob(screen: &TerminalSnapshot) -> String {
    lines_with(screen, "│").join("\n")
}

fn value_token(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    let index = lines.iter().position(|line| line.contains(label))?;
    let column = lines[index].find(label)?;
    let column = lines[index][..column].chars().count();
    let value_line = lines.get(index + 1).map(String::as_str).unwrap_or("");
    let rest: String = value_line.chars().skip(column).collect();
    Some(
        rest.split_whitespace()
            .find(|token| *token != "▎")
            .unwrap_or("")
            .to_string(),
    )
}

fn value_under(screen: &TerminalSnapshot, label: &str) -> String {
    value_token(screen, label)
        .unwrap_or_else(|| panic!("label {label:?} missing\n{}", text(screen)))
}

fn token_starts(screen: &TerminalSnapshot, label: &str, value: &str) -> bool {
    value_token(screen, label).is_some_and(|got| got.starts_with(value))
}

fn assert_value(screen: &TerminalSnapshot, label: &str, value: &str, others: &[&str]) {
    let got = value_under(screen, label);
    assert!(
        got.starts_with(value),
        "{label} shows {got:?}, expected {value}"
    );
    for other in others {
        assert!(
            !got.contains(other),
            "{label} also shows {other} in {got:?}"
        );
    }
}

fn sidebar_cell_editors(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("› Cell editors") && !line.contains("Tables ›"))
        .unwrap_or_else(|| panic!("sidebar Cell editors missing\n{}", text(screen)))
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

fn focus_table(app: &App) {
    app.settle();
    for _ in 0..8 {
        let screen = app.screen();
        if row_with(&screen, "D-101").contains('▎') {
            return;
        }
        app.key("tab");
        app.settle();
    }
    panic!("table did not take focus\n{}", text(&app.screen()));
}

fn open_env(app: &App) {
    focus_table(app);
    app.key("enter");
    app.wait_text(CHOOSING);
}

fn defaults_besides(screen: &TerminalSnapshot, changed: &str) {
    if changed != "dismiss" {
        assert_value(screen, "Dismiss", "light", &["escape"]);
    }
    if changed != "blur" {
        assert_value(screen, "On blur", "cancel", &["commit"]);
    }
    if changed != "rows" {
        assert_value(screen, "Popup rows", "8", &["12"]);
    }
    if changed != "width" {
        assert_value(screen, "Popup width", "14-48", &["14-24", "30-60"]);
    }
    if changed != "cursor" {
        assert_value(screen, "Popup cursor", "fill", &["bar"]);
    }
    if changed != "ahead" {
        see(screen, &["Type-ahead on"], &["Type-ahead off"]);
    }
    if changed != "affordance" {
        assert_value(screen, "▾ affordance", "focused", &["always", "never"]);
    }
    if changed != "flag" {
        see(
            screen,
            &["Flag values outside the set on"],
            &["Flag values outside the set off"],
        );
    }
    if changed != "separator" {
        assert_value(screen, "Tag separator", "comma", &["dot"]);
    }
    if changed != "placement" {
        assert_value(screen, "Popup placement", "below", &["above"]);
    }
}

fn shot(app: &App, case_id: &str, _precondition: &str, _args: &[&str]) {
    app.visual(&visual_id(case_id));
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = crate::support::workspace_root().join("tests/functional/baselines");
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
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

/// Down moves the bar onto Sandbox while D-101 still has the press flash.
/// A later paint locks the released row.
fn bar_still_pressed(screen: &TerminalSnapshot) -> bool {
    if !text(screen).contains(CHOOSING) {
        return false;
    }
    let Some(id) = find_all(screen, "D-101").into_iter().find(|hit| hit.x > 24) else {
        return false;
    };
    let pressed = screen
        .cell_at(u32::from(id.x), u32::from(id.y))
        .is_some_and(|cell| cell.bg() == SnapshotColor::Rgb(0x38, 0x38, 0x3e));
    if !pressed {
        return false;
    }
    let production = lines_with(screen, "› Production");
    let sandbox = lines_with(screen, "Sandbox");
    production
        .iter()
        .any(|line| line.contains('│') && !line.contains('▎'))
        && sandbox.iter().any(|line| line.contains('▎'))
}

fn omit_states<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut absent: Vec<&'a str> = vec![
        "pressed",
        "focus + hover",
        "selected",
        "busy",
        "loading",
        "Page options",
        PLAY,
        "● Compare levels",
    ];
    absent.extend(extra);
    absent
}

#[test]
fn route_by_page_keyboard_and_pointer() {
    let args = ["--page", "celleditors"];
    let app = page("s03-celleditors-route-page", None, 120, 40);
    // The first paint shows the table before Option sets is drawn.
    let screen = app.wait_screen("cell editors page", |screen| {
        let raw = text(screen);
        raw.contains("Deployment targets")
            && raw.contains("Option sets")
            && raw.contains("Disaster recovery")
    });
    see(
        &screen,
        &[
            TITLE,
            "Cell editors",
            BLURB,
            PLAY,
            WIDE,
            "truecolor",
            "Deployment targets",
            META,
            "D-101",
            "checkout-api",
            "Production",
            "qa-legacy",
            "Option sets",
            "Disaster recovery",
        ],
        &[
            "Page options",
            OPT,
            STATE,
            "Compare levels",
            OVERVIEW,
            SELECTION,
            CANCELLED,
            SAVED,
        ],
    );
    let side = sidebar_cell_editors(&screen);
    assert!(
        side.contains('▎') && side.contains("› Cell editors"),
        "sidebar focus is not on Cell editors: {side}"
    );
    shot(
        &app,
        "sc.celleditors.route",
        "--page celleditors opens the playground with the sidebar on Cell editors",
        &args,
    );
    app.finish();

    let keys = open("s03-celleditors-route-keys", 120, 40, &[]);
    keys.wait_text(OVERVIEW);
    for _ in 0..19 {
        keys.key("]");
    }
    let arrived = keys.wait_text(BLURB);
    see(
        &arrived,
        &[TITLE, BLURB, "Deployment targets", PLAY, "D-101"],
        &[OVERVIEW, "Page options", "Compare levels", SELECTION],
    );
    keys.key("[");
    let left = keys.wait_text(SELECTION);
    see(
        &left,
        &[
            SELECTION,
            "Select rows, edit one or many together, delete with a confirmation",
        ],
        &[BLURB, "Deployment targets", "qa-legacy"],
    );
    keys.key("]");
    let again = keys.wait_text(BLURB);
    see(
        &again,
        &[TITLE, BLURB, "Deployment targets"],
        &[SELECTION, OVERVIEW],
    );
    click_side(&keys, "Overview");
    let overview = keys.wait_text(OVERVIEW);
    see(&overview, &[OVERVIEW], &[BLURB, "Deployment targets"]);
    click_side(&keys, "Tables");
    click_side(&keys, "Cell editors");
    let pointed = keys.wait_text("Deployment targets");
    see(
        &pointed,
        &[TITLE, BLURB, "D-101", "Production", META],
        &[OVERVIEW, "Page options", "Compare levels", OPT, STATE],
    );
    keys.finish();
}

#[test]
fn sections_narrow_then_wide() {
    let args = ["--page", "celleditors"];
    let app = page("s03-celleditors-sections", None, 80, 24);
    // The header paints before the table rows. "Deployment targets" is not
    // the playground frame until D-101 is on screen.
    let narrow_play = app.wait_screen("narrow cell rows", |screen| {
        let visible = text(screen);
        visible.contains("Deployment targets")
            && visible.contains("D-101")
            && visible.contains("Production")
            && visible.contains("qa-legacy")
            && visible.contains("mars-1")
    });
    see(
        &narrow_play,
        &[
            TITLE,
            "Cell editors",
            BLURB_NARROW,
            "Playground",
            "Deployment targets",
            META_NARROW,
            "D-101",
            "Production",
            "qa-legacy",
            "mars-1",
        ],
        &[
            BLURB,
            PLAY,
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            WIDE,
            "truecolor",
            "Option sets",
            "outside the set",
        ],
    );
    let side = sidebar_cell_editors(&narrow_play);
    assert!(side.contains('▎'), "narrow sidebar lost focus: {side}");
    shot(
        &app,
        "sc.celleditors.playground.80x24",
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
            "Dismiss",
            "light",
            "On blur",
            "cancel",
            "Popup rows",
            "Popup width",
            "14-48",
            "Popup cursor",
            "fill",
            "Type-ahead on",
            "focused",
            "Flag values outside the set on",
        ],
        &[
            PLAY,
            OPT,
            STATE,
            "Compare levels",
            "Deployment targets",
            "escape",
            "commit",
            "Type-ahead off",
            "always",
            "never",
            "Flag values outside the set off",
            WIDE,
            "truecolor",
            "Tag separator",
            "Popup placement",
        ],
    );
    assert_value(&narrow_opt, "Dismiss", "light", &["escape"]);
    assert_value(&narrow_opt, "On blur", "cancel", &["commit"]);
    assert_value(&narrow_opt, "Popup rows", "8", &["12"]);
    assert_value(&narrow_opt, "Popup cursor", "fill", &["bar"]);
    shot(
        &app,
        "sc.celleditors.options.80x24",
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
            LEGEND_NARROW,
            "▾ on focus and hover",
            "rest",
            "hover",
            "focused",
            "checkout-api",
            "Production",
        ],
        &omit_states(&[
            STATE,
            "editing",
            "error",
            "disabled",
            "empty",
            "overflow",
            "narrow",
            WIDE,
            "truecolor",
            "No rows",
        ]),
    );
    shot(
        &app,
        "sc.celleditors.states.80x24",
        "states grid at 80 by 24 before the terminal grows, showing rest, hover, and focused",
        &args,
    );

    app.resize(120, 40);
    let wide_states = app.wait_screen("wide states grid", |screen| {
        let visible = text(screen);
        visible.contains(WIDE) && visible.contains(STATE) && visible.contains("overflow")
    });
    see(
        &wide_states,
        &[
            TITLE,
            STATE,
            WIDE,
            "truecolor",
            "○ Compare levels",
            LEGEND,
            "▾ on focus and hover",
            "▾ always",
            "rest",
            "hover",
            "focused",
            "editing",
            "error",
            "disabled",
            "empty",
            "No rows",
            "overflow",
        ],
        &omit_states(&["narrow", "256 colors", "no color"]),
    );
    shot(
        &app,
        "sc.celleditors.states.120x40",
        "states caption and the grid rows through overflow at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.celleditors.states",
        "states is the visible Cell editors section",
        &args,
    );
    for case_id in TOP_SPECIMENS {
        let label = case_id.rsplit('.').next().expect("specimen label");
        see(
            &app.screen(),
            &[label, "○ Compare levels", STATE],
            &omit_states(&["narrow", "no color"]),
        );
        shot(
            &app,
            case_id,
            "states grid row is visible and compare levels is off",
            &args,
        );
    }

    app.key("pagedown");
    let scrolled = app.wait_text("narrow");
    see(
        &scrolled,
        &[
            STATE,
            "overflow",
            "checkout-api a…",
            "narrow",
            "No rows",
            "empty",
        ],
        &omit_states(&["rest", "no color", "256 colors"]),
    );
    shot(
        &app,
        "sc.celleditors.specimen.overflow",
        "states grid row overflow is visible, including checkout-api a…, and compare levels is off",
        &args,
    );
    shot(
        &app,
        "sc.celleditors.specimen.narrow",
        "states grid row narrow is visible and compare levels is off",
        &args,
    );

    click_tab(&app, "Options");
    let wide_opt = app.wait_text(OPT);
    see(
        &wide_opt,
        &[
            TITLE,
            OPT,
            WIDE,
            "Page options",
            "Dismiss",
            "On blur",
            "Popup rows",
            "Popup width",
            "Popup cursor",
            "Type-ahead on",
            "▾ affordance",
            "Flag values outside the set on",
            "Tag separator",
            "Popup placement",
        ],
        &[
            PLAY,
            STATE,
            "Compare levels",
            "Deployment targets",
            "escape",
            "commit",
            "Type-ahead off",
            "Flag values outside the set off",
        ],
    );
    defaults_besides(&wide_opt, "");
    shot(
        &app,
        "sc.celleditors.options.120x40",
        "options caption and every default value at 120 by 40",
        &args,
    );
    shot(
        &app,
        "sc.celleditors.options",
        "options is the visible Cell editors section",
        &args,
    );

    click_tab(&app, "Playground");
    let wide_play = app.wait_text(PLAY);
    see(
        &wide_play,
        &[
            TITLE,
            BLURB,
            PLAY,
            WIDE,
            "Deployment targets",
            META,
            "D-101",
            "Production",
            "qa-legacy",
            "Option sets",
            "Frozen until Friday",
        ],
        &[
            OPT,
            STATE,
            "Page options",
            "Compare levels",
            BLURB_NARROW,
            CANCELLED,
            SAVED,
            "1 edits",
        ],
    );
    shot(
        &app,
        "sc.celleditors.playground.120x40",
        "playground tab at 120 by 40 after the narrow capture",
        &args,
    );
    shot(
        &app,
        "sc.celleditors.playground",
        "playground is the visible Cell editors section",
        &args,
    );
    app.finish();
}

#[test]
fn compare_levels_narrows_before_restore() {
    let args = ["--page", "celleditors"];
    let app = page("s03-celleditors-compare", None, 120, 40);
    click_tab(&app, "States");
    let off = app.wait_text("○ Compare levels");
    see(
        &off,
        &[TITLE, "○ Compare levels", "rest", "overflow", STATE, LEGEND],
        &[
            "256 colors",
            "16 colors",
            "no color",
            "● Compare levels",
            "Page options",
        ],
    );
    click_where(&app, "Compare levels", |hit| hit.y < 12);
    let on = app.wait_text("● Compare levels");
    see(
        &on,
        &[TITLE, "● Compare levels", "truecolor", STATE],
        &[
            "○ Compare levels",
            "Page options",
            PLAY,
            "no color",
            "256 colors",
        ],
    );
    for _ in 0..3 {
        app.key("pagedown");
        app.settle();
    }
    let bands = app.wait_text("no color");
    see(
        &bands,
        &[TITLE, "no color", STATE, WIDE, "rest"],
        &[
            "Page options",
            PLAY,
            "○ Compare levels",
            "256 colors",
            "16 colors",
            "pressed",
            "busy",
            "loading",
        ],
    );
    shot(
        &app,
        "sc.celleditors.compare-levels.120x40",
        "compare levels scrolled to no color at 120 by 40",
        &args,
    );
    app.resize(80, 24);
    let narrow = app.wait_text("● Compare levels");
    see(
        &narrow,
        &[TITLE, "● Compare levels", "truecolor", "rest", BLURB_NARROW],
        &[
            "Page options",
            PLAY,
            WIDE,
            "120×40",
            OPT,
            "no color",
            "256 colors",
            "16 colors",
            STATE,
        ],
    );
    shot(
        &app,
        "sc.celleditors.compare-levels.80x24",
        "compare levels still on at 80 by 24 before restoring 120 by 40",
        &args,
    );
    app.key("space");
    let restored = app.wait_text("○ Compare levels");
    see(
        &restored,
        &[TITLE, "○ Compare levels", "rest"],
        &[
            "● Compare levels",
            "256 colors",
            "16 colors",
            "no color",
            "Page options",
            "truecolor",
        ],
    );
    app.resize(120, 40);
    app.wait_text(WIDE);
    click_tab(&app, "Options");
    let options = app.wait_text("Page options");
    defaults_besides(&options, "");
    see(
        &options,
        &["Page options", "Type-ahead on", "light", OPT],
        &[
            "escape",
            "commit",
            "Type-ahead off",
            "Compare levels",
            "Deployment targets",
        ],
    );
    app.finish();
}

#[test]
fn option_dismiss_escape() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.dismiss=escape",
    ];
    let app = page(
        "s03-celleditors-dismiss",
        Some("celleditors.dismiss=escape"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Dismiss");
    assert_value(&options, "Dismiss", "escape", &["light"]);
    defaults_besides(&options, "dismiss");
    see(
        &options,
        &["escape", "cancel", "Type-ahead on"],
        &[PLAY, "light"],
    );
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    open_env(&app);
    let open = app.screen();
    assert!(
        popup_blob(&open).contains("Development")
            && row_with(&open, "D-101").contains("Production"),
        "popup did not open on Production\n{}",
        text(&open)
    );
    click_side(&app, "Overview");
    let stayed = app.settle();
    let blob = popup_blob(&stayed);
    assert!(
        blob.contains("Development") && blob.contains("› Production"),
        "escape dismiss closed the popup\n{}",
        text(&stayed)
    );
    see(
        &stayed,
        &[CHOOSING, META, "Production"],
        &[CANCELLED, SAVED, "1 edits", "Edit cancelled", OVERVIEW],
    );
    assert!(
        row_with(&stayed, "D-101").contains("Production"),
        "outside click changed D-101"
    );
    shot(
        &app,
        "opt.celleditors.dismiss",
        "celleditors.dismiss=escape keeps the Env popup open after an outside click and leaves the other options",
        &args,
    );
    app.key("esc");
    let cancelled = app.wait_text(CANCELLED);
    see(
        &cancelled,
        &[CANCELLED, "Production"],
        &[CHOOSING, SAVED, "1 edits"],
    );
    app.finish();
}

#[test]
fn option_blur_commit() {
    let args = ["--page", "celleditors", "--opt", "celleditors.blur=commit"];
    let app = page(
        "s03-celleditors-blur",
        Some("celleditors.blur=commit"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("On blur");
    assert_value(&options, "On blur", "commit", &["cancel"]);
    defaults_besides(&options, "blur");
    see(
        &options,
        &["commit", "light", "Type-ahead on"],
        &[PLAY, "cancel"],
    );
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    open_env(&app);
    app.key("down");
    app.settle();
    let pending = app.screen();
    assert!(
        popup_blob(&pending).contains("Sandbox")
            && row_with(&pending, "D-101").contains("Production"),
        "down did not leave Production committed\n{}",
        text(&pending)
    );
    app.key("esc");
    let cancelled = app.wait_text(CANCELLED);
    see(
        &cancelled,
        &[CANCELLED, META, "Production"],
        &[SAVED, CHOOSING, "1 edits"],
    );
    assert!(
        row_with(&cancelled, "D-101").contains("Production")
            && !row_with(&cancelled, "D-101").contains("Sandbox"),
        "Esc committed Sandbox under blur=commit: {}",
        row_with(&cancelled, "D-101")
    );
    click_tab(&app, "Options");
    let again = app.wait_text("On blur");
    assert_value(&again, "On blur", "commit", &["cancel"]);
    defaults_besides(&again, "blur");
    shot(
        &app,
        "opt.celleditors.blur",
        "celleditors.blur=commit shows commit, and Esc still cancels an open Env choice back to Production",
        &args,
    );
    app.finish();
}

#[test]
fn option_max_rows_four() {
    let args = ["--page", "celleditors", "--opt", "celleditors.max_rows=4"];
    let app = page(
        "s03-celleditors-rows",
        Some("celleditors.max_rows=4"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Popup rows");
    assert_value(&options, "Popup rows", "4", &["8", "12"]);
    defaults_besides(&options, "rows");
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    focus_table(&app);
    app.key("right");
    app.settle();
    app.key("enter");
    let open = app.wait_text("Search");
    let blob = popup_blob(&open);
    assert!(
        blob.contains("us-west-2") && blob.contains("Search"),
        "region popup missed the fourth row\n{blob}"
    );
    assert!(
        !blob.contains("ca-central-1") && !blob.contains("eu-west-1"),
        "max_rows 4 still listed a fifth region\n{blob}"
    );
    see(
        &open,
        &[CHOOSING, "us-east-1", META],
        &[SAVED, CANCELLED, "1 edits"],
    );
    assert!(
        row_with(&open, "D-101").contains("us-east-1"),
        "opening the region popup changed the cell"
    );
    shot(
        &app,
        "opt.celleditors.max_rows",
        "celleditors.max_rows=4 stops the Region popup at us-west-2",
        &args,
    );
    app.key("esc");
    let cancelled = app.wait_text(CANCELLED);
    see(&cancelled, &[CANCELLED, "us-east-1"], &[CHOOSING, SAVED]);
    app.finish();
}

#[test]
fn option_popup_width_narrow() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.popup_width=14-24",
    ];
    let app = page(
        "s03-celleditors-width",
        Some("celleditors.popup_width=14-24"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Popup width");
    assert_value(&options, "Popup width", "14-24", &["14-48", "30-60"]);
    defaults_besides(&options, "width");
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    open_env(&app);
    let open = app.screen();
    let disaster = lines_with(&open, "Disaster recovery")
        .into_iter()
        .find(|line| line.contains('│'))
        .unwrap_or_else(|| panic!("popup disaster row missing\n{}", text(&open)));
    assert!(
        !disaster.contains("Frozen"),
        "14-24 still showed the disabled note: {disaster}"
    );
    assert!(
        text(&open).contains("Frozen until Friday"),
        "option sets lost the disabled note"
    );
    assert!(
        !popup_blob(&open).contains("qa-legacy"),
        "narrow popup offered qa-legacy"
    );
    see(
        &open,
        &[CHOOSING, "› Production", META],
        &[SAVED, CANCELLED, "1 edits"],
    );
    shot(
        &app,
        "opt.celleditors.popup_width",
        "celleditors.popup_width=14-24 clips Frozen until Friday off the Env popup",
        &args,
    );
    app.key("esc");
    app.wait_text(CANCELLED);
    app.finish();
}

#[test]
fn option_cursor_bar() {
    let app = page(
        "s03-celleditors-cursor",
        Some("celleditors.cursor_style=bar"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Popup cursor");
    assert_value(&options, "Popup cursor", "bar", &["fill"]);
    defaults_besides(&options, "cursor");
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    focus_table(&app);
    let mut moved = None;
    for _ in 0..6 {
        app.key("enter");
        app.key("down");
        let started = Instant::now();
        let mut since: Option<Instant> = None;
        while started.elapsed() < Duration::from_millis(500) {
            let screen = app.screen();
            if bar_still_pressed(&screen) {
                let held = since.get_or_insert_with(Instant::now);
                if held.elapsed() >= Duration::from_millis(120) {
                    moved = Some(screen);
                    break;
                }
            } else {
                since = None;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if moved.is_some() {
            break;
        }
        app.key("esc");
    }
    let moved = moved.unwrap_or_else(|| {
        panic!(
            "bar move did not keep the pressed row\n{}",
            text(&app.screen())
        )
    });
    let blob = popup_blob(&moved);
    let current = lines_with(&moved, "› Production");
    assert!(
        current
            .iter()
            .any(|line| line.contains('│') && !line.contains('▎')),
        "bar cursor stayed on Production: {current:?}"
    );
    assert!(
        blob.contains("▎") && blob.contains("Sandbox"),
        "bar cursor did not move to Sandbox\n{blob}"
    );
    see(
        &moved,
        &[CHOOSING, META, "Production"],
        &[SAVED, CANCELLED, "1 edits"],
    );
    assert!(
        row_with(&moved, "D-101").contains("Production"),
        "moving the bar cursor committed Sandbox"
    );
    check_frame(&app, &moved, &visual_id("opt.celleditors.cursor_style"));
    app.key("esc");
    let cancelled = app.wait_text(CANCELLED);
    assert!(
        row_with(&cancelled, "D-101").contains("Production"),
        "Esc after the bar move stored Sandbox"
    );
    see(&cancelled, &[CANCELLED], &[CHOOSING, SAVED, "1 edits"]);
    app.finish();
}

#[test]
fn option_type_ahead_off() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.type_ahead=off",
    ];
    let app = page(
        "s03-celleditors-ahead",
        Some("celleditors.type_ahead=off"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Type-ahead");
    see(&options, &["Type-ahead off"], &["Type-ahead on", PLAY]);
    defaults_besides(&options, "ahead");
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    open_env(&app);
    let before = popup_blob(&app.screen());
    app.key("s");
    let after = app.settle();
    let blob = popup_blob(&after);
    assert_eq!(
        blob, before,
        "typing s changed the choice list with type-ahead off"
    );
    assert!(
        blob.contains("Development")
            && blob.contains("Staging")
            && blob.contains("› Production")
            && blob.contains("Sandbox")
            && blob.contains("Disaster recovery"),
        "choice list lost a value after s\n{blob}"
    );
    see(
        &after,
        &[CHOOSING, META, "Production"],
        &[SAVED, CANCELLED, "1 edits", "zzzz", "qa-legacy"],
    );
    assert!(row_with(&after, "D-101").contains("Production"));
    shot(
        &app,
        "opt.celleditors.type_ahead",
        "celleditors.type_ahead=off keeps the Env choice list and the Production cell unchanged after typing s",
        &args,
    );
    app.key("esc");
    app.wait_text(CANCELLED);
    app.finish();
}

#[test]
fn option_affordance_always() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.affordance=always",
    ];
    let app = page(
        "s03-celleditors-affordance",
        Some("celleditors.affordance=always"),
        120,
        40,
    );
    let play = app.wait_text("Deployment targets");
    let carets = text(&play).matches('▾').count();
    assert!(carets > 20, "always affordance drew {carets} carets");
    assert!(
        row_with(&play, "D-102").contains('▾') && row_with(&play, "D-106").contains('▾'),
        "unfocused rows hid the caret"
    );
    see(
        &play,
        &[PLAY, META, "Production", "Staging"],
        &["Page options", CANCELLED, SAVED, "1 edits"],
    );
    click_tab(&app, "Options");
    // The affordance label paints before the value under Popup placement is
    // `below`. A torn row reads the checkbox glyph `✓]` as that value.
    let options = app.wait_screen("affordance options", |screen| {
        token_starts(screen, "▾ affordance", "always")
            && token_starts(screen, "Dismiss", "light")
            && token_starts(screen, "On blur", "cancel")
            && token_starts(screen, "Popup rows", "8")
            && token_starts(screen, "Popup width", "14-48")
            && token_starts(screen, "Popup cursor", "fill")
            && token_starts(screen, "Tag separator", "comma")
            && token_starts(screen, "Popup placement", "below")
            && text(screen).contains("Type-ahead on")
            && text(screen).contains("Flag values outside the set on")
    });
    assert_value(&options, "▾ affordance", "always", &["focused", "never"]);
    defaults_besides(&options, "affordance");
    click_tab(&app, "Playground");
    let again = app.wait_text("Deployment targets");
    assert!(
        row_with(&again, "D-102").contains('▾'),
        "returning to the playground dropped the always caret"
    );
    shot(
        &app,
        "opt.celleditors.affordance",
        "celleditors.affordance=always draws ▾ on cells before the table is focused",
        &args,
    );
    app.finish();
}

#[test]
fn option_flag_out_of_set_off() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.flag_out_of_set=off",
    ];
    let app = page(
        "s03-celleditors-flag",
        Some("celleditors.flag_out_of_set=off"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Flag values outside the set");
    see(
        &options,
        &["Flag values outside the set off"],
        &["Flag values outside the set on", PLAY],
    );
    defaults_besides(&options, "flag");
    click_tab(&app, "Playground");
    let play = app.wait_text("qa-legacy");
    let row = row_with(&play, "D-106");
    assert!(
        row.contains("qa-legacy") && row.contains("mars-1") && !row.contains('!'),
        "D-106 still flagged an outside value: {row}"
    );
    see(
        &play,
        &[META, "Outside the set", PLAY, "Production"],
        &[SAVED, CANCELLED, "1 edits", "7 values outside the set"],
    );
    shot(
        &app,
        "opt.celleditors.flag_out_of_set",
        "celleditors.flag_out_of_set=off draws D-106 qa-legacy and mars-1 with no exclamation mark",
        &args,
    );
    app.finish();
}

#[test]
fn option_tag_separator_dot() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.tag_separator=dot",
    ];
    let app = page(
        "s03-celleditors-separator",
        Some("celleditors.tag_separator=dot"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Tag separator");
    assert_value(&options, "Tag separator", "dot", &["comma"]);
    defaults_besides(&options, "separator");
    see(
        &options,
        &["dot", "Type-ahead on", "below"],
        &[PLAY, "comma"],
    );
    click_tab(&app, "Playground");
    let play = app.wait_text(OUTSIDE);
    see(
        &play,
        &[OUTSIDE, "0 edits", PLAY, "Production"],
        &["3 values outside the set", SAVED, CANCELLED, "api · web"],
    );
    focus_table(&app);
    for _ in 0..6 {
        if text(&app.screen()).contains("api, web") {
            break;
        }
        app.key("right");
        app.settle();
    }
    let scrolled = app.wait_text("api, web");
    assert!(
        row_with(&scrolled, "api, web").contains("api, web"),
        "dot separator rewrote the stored comma"
    );
    see(
        &scrolled,
        &["api, web", OUTSIDE],
        &["api · web", "3 values outside the set", SAVED],
    );
    shot(
        &app,
        "opt.celleditors.tag_separator",
        "celleditors.tag_separator=dot counts 7 values outside the set and still shows api, web",
        &args,
    );
    app.finish();
}

#[test]
fn option_placement_above() {
    let args = [
        "--page",
        "celleditors",
        "--opt",
        "celleditors.placement=above",
    ];
    let app = page(
        "s03-celleditors-placement",
        Some("celleditors.placement=above"),
        120,
        40,
    );
    click_tab(&app, "Options");
    let options = app.wait_text("Popup placement");
    assert_value(&options, "Popup placement", "above", &["below"]);
    defaults_besides(&options, "placement");
    click_tab(&app, "Playground");
    app.wait_text("Deployment targets");
    focus_table(&app);
    app.key("down");
    app.settle();
    app.key("enter");
    let open = app.wait_text(CHOOSING);
    let cell = row_with(&open, "D-102");
    assert!(
        cell.contains("Staging") && cell.contains('▾') && !cell.contains('│'),
        "popup covered the D-102 cell instead of opening above it: {cell}"
    );
    let blob = popup_blob(&open);
    assert!(
        blob.contains("› Staging") && blob.contains("Development"),
        "above popup missed the current Staging row\n{blob}"
    );
    see(
        &open,
        &[CHOOSING, "Staging"],
        &[SAVED, CANCELLED, "1 edits", "Edit cancelled"],
    );
    shot(
        &app,
        "opt.celleditors.placement",
        "celleditors.placement=above opens the Env popup over the header while D-102 stays Staging",
        &args,
    );
    app.key("esc");
    let cancelled = app.wait_text(CANCELLED);
    assert!(
        row_with(&cancelled, "D-102").contains("Staging"),
        "Esc above the popup changed D-102"
    );
    see(&cancelled, &[CANCELLED], &[CHOOSING, SAVED, "1 edits"]);
    app.finish();
}

#[test]
fn editors_open_choose_and_cancel() {
    let args = ["--page", "celleditors"];
    let app = page("s03-celleditors-editors", None, 120, 40);
    app.wait_text("Deployment targets");
    open_env(&app);
    let open = app.screen();
    let blob = popup_blob(&open);
    for choice in [
        "Development",
        "Staging",
        "› Production",
        "Sandbox",
        "Preview",
        "Performance",
        "Disaster recovery",
        "Frozen until Friday",
    ] {
        assert!(blob.contains(choice), "open popup missing {choice}\n{blob}");
    }
    assert!(
        !blob.contains("qa-legacy"),
        "open popup offered a value outside the set\n{blob}"
    );
    see(
        &open,
        &[CHOOSING, META, "Production"],
        &[SAVED, CANCELLED, "1 edits", "zzzz"],
    );
    assert!(row_with(&open, "D-101").contains("Production"));
    let before_z = popup_blob(&open);
    app.key("z");
    let typed = app.settle();
    assert_eq!(
        popup_blob(&typed),
        before_z,
        "z changed the open choice list"
    );
    assert!(!text(&typed).contains("zzzz"), "z became a choice");
    see(&typed, &[CHOOSING, META], &[SAVED, "1 edits"]);
    shot(
        &app,
        "sc.celleditors.editors.before",
        "Env choice is open on Production and qa-legacy is not a choice",
        &args,
    );
    app.key("down");
    app.settle();
    app.key("enter");
    let saved = app.wait_text(SAVED);
    let cell = row_with(&saved, "D-101");
    assert!(
        cell.contains("Sandbox") && !cell.contains("Production"),
        "Enter did not store Sandbox: {cell}"
    );
    see(
        &saved,
        &[SAVED, "1 edits", "3 values outside the set", "Sandbox"],
        &[CHOOSING, CANCELLED, "│ › Production", "0 edits"],
    );
    shot(
        &app,
        "sc.celleditors.editors.after",
        "Enter stored Sandbox on D-101 and closed the popup",
        &args,
    );
    app.key("enter");
    app.wait_text(CHOOSING);
    app.key("esc");
    let cancelled = app.wait_text(CANCELLED);
    let kept = row_with(&cancelled, "D-101");
    assert!(
        kept.contains("Sandbox") && !kept.contains("Production"),
        "Esc restored Production after the commit: {kept}"
    );
    see(
        &cancelled,
        &[CANCELLED, "1 edits"],
        &[SAVED, CHOOSING, "0 edits", "2 edits"],
    );
    app.finish();
}

#[test]
fn issue43_rejected_notify_toggle_shows_its_error() {
    const LOCKED: &str = "Notify stays on while the service has an on-call owner";
    let app = page("s03-celleditors-notify-lock", None, 120, 40);
    app.wait_text("Deployment targets");
    focus_table(&app);
    for _ in 0..4 {
        app.key("right");
    }
    app.settle();

    app.key("enter");
    let rejected = app.wait_text(LOCKED);
    let first = row_with(&rejected, "checkout-api");
    assert!(
        first.contains("[✓]") && !first.contains("[ ]"),
        "the rejected toggle still flipped the cell: {first}"
    );
    see(
        &rejected,
        &[LOCKED],
        &["Notify saved", "1 edits", "0 edits"],
    );

    app.key("down");
    app.key("enter");
    let accepted = app.wait_text("Notify saved");
    see(&accepted, &["1 edits"], &[LOCKED, "0 edits"]);
    let second = row_with(&accepted, "search");
    assert!(
        second.contains("[✓]") && !second.contains("[ ]"),
        "the accepted toggle did not switch search on: {second}"
    );
    app.finish();
}

#[test]
fn standalone_choice_editor_reports_its_options_while_open() {
    let app = page("s03-celleditors-chooser", None, 120, 40);
    app.wait_text("Deployment targets");
    let anchor = app.point("Deployment targets");
    for _ in 0..12 {
        if text(&app.screen()).contains("Default region") {
            break;
        }
        app.wheel(anchor, true, 3);
        app.settle();
    }
    let closed = app.wait_text("Default region");
    see(&closed, &["A ChoiceEditor on its own"], &["options open"]);
    app.click_text("Default region");
    let open = app.wait_text("options open");
    see(&open, &["18 options open", CHOOSING], &[]);
    app.key("esc");
    let shut = app.wait_screen("chooser closed", |screen| {
        !text(screen).contains("options open")
    });
    see(&shut, &["Default region"], &["options open"]);
    app.finish();
}
