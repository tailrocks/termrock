//! Shell options, Inspector sections, and sidebar expansion.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use tuiscotti_latest::check::{CheckOutcome, MismatchReason};

use crate::support::{
    App, Launch, Point, SHOWCASE, SessionMode, cell_bg, file_sha256, find, find_all, find_in,
    reference_binary, reference_digest, row_text, text, try_launch,
};
/// Tuiscotti test ids are `[a-z0-9_-]` segments, so `S01.shell` cannot be one.
/// The baseline root is the owned directory and the id is the slug inside it.
fn baseline_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/functional/baselines/pre_refactor/S01.shell")
}

fn capturing() -> bool {
    std::env::var("S01_CAPTURE").ok().as_deref() == Some("1")
}

fn launch(name: &str, args: &[&str], cols: u16, rows: u16) -> App {
    let mut launch = if capturing() {
        let reference = reference_binary("showcase");
        assert_eq!(
            file_sha256(&reference),
            reference_digest("showcase"),
            "reference showcase hash"
        );
        let mut launch = Launch::new(name, reference.to_str().expect("reference path"));
        launch.mode = SessionMode::Reference;
        launch
    } else {
        Launch::new(name, SHOWCASE)
    };
    launch.args = args.iter().map(|arg| (*arg).to_string()).collect();
    launch.cols = cols;
    launch.rows = rows;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    launch.platform = "macos".to_string();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn fixture_args(args: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    out.extend(["--color", "truecolor", "--motion", "paused"].map(str::to_string));
    out
}

fn expect_visual(
    app: &App,
    test_id: &str,
    precondition: &str,
    args: &[&str],
    roles: &[(&str, &str)],
) {
    let outcome = app.check_visual(&baseline_root(), test_id);
    if !capturing() {
        assert!(
            outcome.is_pass(),
            "visual mismatch for {test_id}: {outcome:?}"
        );
        return;
    }
    let CheckOutcome::Failure(failure) = outcome else {
        panic!("capture expected a missing baseline for {test_id}");
    };
    assert_eq!(
        failure.reason(),
        MismatchReason::Missing,
        "{test_id} {:?}",
        failure.reason()
    );
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/functional/s01-capture");
    std::fs::create_dir_all(&dir).expect("capture dir");
    let roles_json: Vec<_> = roles
        .iter()
        .map(|(case_id, role)| serde_json::json!({ "case_id": case_id, "role": role }))
        .collect();
    let body = serde_json::json!({
        "test_id": test_id,
        "candidate": failure.candidate_path(),
        "candidate_sha256": failure.candidate_sha256(),
        "precondition": precondition,
        "fixture_args": fixture_args(args),
        "roles": roles_json,
    });
    let name = test_id.replace('/', "__");
    std::fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&body).expect("capture json"),
    )
    .expect("write capture record");
}

fn column(screen: &tui_test::TerminalSnapshot) -> String {
    let skip = 88usize;
    (0..screen.rows())
        .map(|row| {
            let chars: Vec<char> = row_text(screen, row).chars().collect();
            if chars.len() <= skip {
                String::new()
            } else {
                chars[skip..].iter().collect()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn label_index(column: &str, label: &str) -> usize {
    for (index, line) in column.lines().enumerate() {
        let Some(at) = line.find(label) else {
            continue;
        };
        let after = line[at + label.len()..].trim_start();
        let state = after == "on"
            || after.starts_with("on ")
            || after == "off"
            || after.starts_with("off ");
        let boundary = after.is_empty() || after.starts_with(['▾', '┆', '┃', '│', '…']) || state;
        if boundary {
            return index;
        }
    }
    panic!("label {label:?} missing\n{column}");
}

fn value_line<'a>(column: &'a str, label: &str) -> &'a str {
    let index = label_index(column, label);
    column
        .lines()
        .nth(index + 1)
        .unwrap_or_else(|| panic!("no value under {label}"))
}

fn assert_select(column: &str, label: &str, value: &str) {
    let line = value_line(column, label).trim();
    assert!(
        line.starts_with(value),
        "{label} shows {line:?}, expected {value}"
    );
}

fn assert_toggle(column: &str, label: &str, mark: &str) {
    let index = label_index(column, label);
    let line = column.lines().nth(index).expect("toggle line");
    assert!(line.contains(mark), "{label} line {line:?} lacks {mark}");
}

fn assert_stays_on(screen: &tui_test::TerminalSnapshot, crumb: &str) {
    let header = row_text(screen, 0);
    assert!(header.contains(crumb), "header {header:?}");
    assert!(
        !header.contains("App shell ›") || crumb.contains("App shell"),
        "retargeted {header:?}"
    );
}

fn open_options(app: &App) {
    app.key("i");
    let opened = app.wait_text("Inspector · on");
    let tab = find_in(&opened, 80, 0, 119, 8, "Options");
    app.click(tab);
    app.wait_text("Colour level");
}

fn finish_inspector(app: &App) {
    // Esc does not close a docked inspector. "i" is consumed while an
    // inspector control has focus, so a following toggle never lands.
    // The header control closes it from any focus.
    let open = app.wait_text("Inspector · on");
    app.click(find(&open, "Inspector · on"));
    app.wait_absent("Inspector · on");
    app.finish();
}

fn card_rule(screen: &tui_test::TerminalSnapshot) -> String {
    let row = (0..screen.rows())
        .find(|row| row_text(screen, *row).contains("Components"))
        .expect("Components");
    row_text(screen, row + 1)
}

fn sidebar_point(screen: &tui_test::TerminalSnapshot, needle: &str, max_x: u16) -> Point {
    find_all(screen, needle)
        .into_iter()
        .filter(|point| point.x < max_x)
        .min_by_key(|point| point.x)
        .unwrap_or_else(|| panic!("sidebar {needle} missing\n{}", text(screen)))
}

enum Expect {
    Select(&'static str),
    Toggle(&'static str),
}

fn show_option(
    name: &str,
    args: &[&str],
    label: &str,
    expect: Expect,
    case_id: &str,
    precondition: &str,
) {
    let app = launch(name, args, 120, 90);
    app.wait_text("Overview");
    open_options(&app);
    let screen = app.wait_text(label);
    let shown = column(&screen);
    match expect {
        Expect::Select(value) => assert_select(&shown, label, value),
        Expect::Toggle(mark) => assert_toggle(&shown, label, mark),
    }
    assert_stays_on(&screen, "Overview");
    assert!(
        !text(&screen).contains("Chrome"),
        "shell option retargeted the page\n{}",
        text(&screen)
    );
    let test_id = format!("{}-boundary", case_id.replace('.', "-"));
    expect_visual(&app, &test_id, precondition, args, &[(case_id, "boundary")]);
    finish_inspector(&app);
}

fn wait_focus(app: &App, needle: &str) {
    app.wait_screen(&format!("focus {needle}"), |screen| {
        text(screen)
            .lines()
            .any(|line| line.contains('▎') && line.contains(needle))
    });
}

fn nav_column(screen: &tui_test::TerminalSnapshot) -> Vec<String> {
    (0..screen.rows())
        .map(|y| {
            (0..28u32)
                .filter_map(|x| screen.cell_at(x, y).map(|cell| cell.text().to_string()))
                .collect()
        })
        .collect()
}

/// Sidebar rows without the hint bar. A wheel moves the pointer, and the hint
/// on the last row can appear while the sidebar list stays put.
fn sidebar_rows(screen: &tui_test::TerminalSnapshot) -> Vec<String> {
    let mut rows = nav_column(screen);
    rows.pop();
    rows
}

const DEFAULT_CASES: &[(&str, &str)] = &[
    ("opt.shell.color", "default"),
    ("opt.shell.glyphs", "default"),
    ("opt.shell.strings", "default"),
    ("opt.shell.motion", "default"),
    ("opt.shell.focus_on", "default"),
    ("opt.shell.double_click_ms", "default"),
    ("opt.shell.double_click_slop", "default"),
    ("opt.shell.hover_refresh_after_scroll", "default"),
    ("opt.shell.flash_ms", "default"),
    ("opt.shell.secondary_outside", "default"),
    ("opt.shell.end_edit_on_dead_space", "default"),
    ("opt.shell.autoscroll", "default"),
    ("opt.shell.autoscroll_ms", "default"),
    ("opt.shell.track_repeat", "default"),
    ("opt.shell.track_repeat_ms", "default"),
    ("opt.shell.track_click", "default"),
    ("opt.shell.pointer_shapes", "default"),
    ("opt.shell.caret_shape", "default"),
    ("opt.shell.click_skips_animation", "default"),
    ("opt.shell.tooltips", "default"),
    ("opt.shell.reveal", "default"),
    ("opt.shell.platform", "default"),
    ("opt.shell.field_keys", "default"),
    ("opt.shell.edit_entry", "default"),
    ("opt.shell.focus_fallback", "default"),
    ("opt.shell.shift_wheel_horizontal", "default"),
    ("opt.shell.legacy_ctrl_punctuation", "default"),
    ("opt.shell.keyboard_protocol", "default"),
    ("opt.shell.alt_keys", "default"),
    ("opt.shell.card_heading", "default"),
    ("opt.shell.inactive_selection", "default"),
];

const SELECT_DEFAULTS: &[(&str, &str)] = &[
    ("Colour level", "truecolor"),
    ("Level overrides", "none"),
    ("Glyphs", "unicode"),
    ("Strings", "default"),
    ("Motion", "full"),
    ("Focus on", "press"),
    ("Double-click ms", "500"),
    ("Double-click slop", "1"),
    ("Wheel lines", "3"),
    ("Press flash ms", "140"),
    ("Right-click outside", "reopen"),
    ("Autoscroll ms", "50"),
    ("Track repeat ms", "300/50"),
    ("Track click", "page"),
    ("Caret shape", "bar"),
    ("Client platform", "auto"),
    ("Field keys", "auto"),
    ("Edit entry", "immediate"),
    ("Focus fallback", "nearest"),
    ("Keyboard protocol", "auto"),
    ("Alt keys", "auto"),
    ("Card heading", "rule"),
    ("Inactive selection", "accent"),
];

const TOGGLE_DEFAULTS: &[&str] = &[
    "Re-hover on scroll",
    "End edit on blank",
    "Edge autoscroll",
    "Track repeat",
    "Pointer shapes",
    "Click skips intro",
    "Tooltips",
    "Reveal cut text",
    "Shift+wheel sideways",
    "Legacy Ctrl punctuat",
];

#[test]
fn s01_shell_defaults() {
    let args = ["--page", "overview", "--opt", "shell.motion=full"];
    let app = launch("s01-shell-defaults", &args, 120, 90);
    app.wait_text("Overview");
    open_options(&app);
    let screen = app.wait_text("Inactive selection");
    let shown = column(&screen);
    for (label, value) in SELECT_DEFAULTS {
        assert_select(&shown, label, value);
    }
    for label in TOGGLE_DEFAULTS {
        assert_toggle(&shown, label, "──●");
    }
    let body = text(&screen);
    assert!(body.contains("truecolor · 120×90"), "{body}");
    assert!(body.contains("▸ App shell"), "{body}");
    assert!(
        card_rule(&screen).contains('─'),
        "card rule {}",
        card_rule(&screen)
    );
    assert!(!body.contains("Chrome"), "{body}");
    assert_stays_on(&screen, "Overview");
    let (selected, other) = (
        cell_bg(&screen, sidebar_point(&screen, "Overview", 20)),
        cell_bg(&screen, sidebar_point(&screen, "App shell", 30)),
    );
    assert_ne!(
        selected, other,
        "inactive accent fill matches an unselected row"
    );
    expect_visual(
        &app,
        "opt-shell-defaults",
        "inspector options at each shell default, motion full overriding paused",
        &args,
        DEFAULT_CASES,
    );
    finish_inspector(&app);
}

#[test]
fn s01_color_mono() {
    let args = ["--page", "overview", "--opt", "shell.color=mono"];
    let app = launch("s01-color-mono", &args, 120, 40);
    let screen = app.wait_text("no color · 120×40");
    let body = text(&screen);
    assert!(body.contains("▸ App shell"), "{body}");
    assert!(card_rule(&screen).contains('─'), "{}", card_rule(&screen));
    assert!(!body.contains("Chrome"));
    assert_stays_on(&screen, "Overview");
    expect_visual(
        &app,
        "opt-shell-color-boundary",
        "colour level mono keeps unicode chrome",
        &args,
        &[("opt.shell.color", "boundary")],
    );
    app.finish();
}

#[test]
fn s01_overrides_default() {
    let args = [
        "--page",
        "overview",
        "--opt",
        "shell.color=mono",
        "--opt",
        "shell.overrides=none",
    ];
    let app = launch("s01-overrides-default", &args, 120, 40);
    // The colour header paints before the sidebar groups.
    let screen = app.wait_screen("mono sidebar", |screen| {
        let body = text(screen);
        body.contains("no color · 120×40")
            && body.contains("▸ App shell")
            && !body.contains("> App shell")
    });
    let body = text(&screen);
    assert!(body.contains("▸ App shell"), "{body}");
    assert!(card_rule(&screen).contains('─'));
    assert!(!body.contains("> App shell"));
    assert_stays_on(&screen, "Overview");
    expect_visual(
        &app,
        "opt-shell-overrides-default",
        "overrides none combined with shell.color=mono",
        &args,
        &[("opt.shell.overrides", "default")],
    );
    app.finish();
}

#[test]
fn s01_overrides_ascii_mono() {
    let args = [
        "--page",
        "overview",
        "--opt",
        "shell.color=mono",
        "--opt",
        "shell.overrides=ascii-mono",
    ];
    let app = launch("s01-overrides-ascii", &args, 120, 40);
    let screen = app.wait_text("no color · 120×40");
    let body = text(&screen);
    assert!(body.contains("> App shell"), "{body}");
    assert!(body.contains("|› Overview"), "{body}");
    assert!(card_rule(&screen).contains("---"), "{}", card_rule(&screen));
    assert!(!body.contains('▸'), "{body}");
    assert!(!body.contains('─'), "{body}");
    assert_stays_on(&screen, "Overview");
    expect_visual(
        &app,
        "opt-shell-overrides-boundary",
        "ascii-mono combined with shell.color=mono",
        &args,
        &[("opt.shell.overrides", "boundary")],
    );
    app.finish();
}

#[test]
fn s01_glyphs_ascii() {
    let args = ["--page", "overview", "--opt", "shell.glyphs=ascii"];
    let app = launch("s01-glyphs-ascii", &args, 120, 40);
    let screen = app.wait_text("truecolor · 120×40");
    let body = text(&screen);
    assert!(
        body.contains("> App shell") && body.contains("|› Overview"),
        "{body}"
    );
    assert!(card_rule(&screen).contains("---"));
    assert!(!body.contains('▸') && !body.contains('─'), "{body}");
    assert_stays_on(&screen, "Overview");
    expect_visual(
        &app,
        "opt-shell-glyphs-boundary",
        "glyphs ascii",
        &args,
        &[("opt.shell.glyphs", "boundary")],
    );
    app.finish();
}

#[test]
fn s01_card_heading_plain() {
    let args = ["--page", "overview", "--opt", "shell.card_heading=plain"];
    let app = launch("s01-card-plain", &args, 120, 40);
    let screen = app.wait_text("truecolor · 120×40");
    let body = text(&screen);
    assert!(
        card_rule(&screen).trim().is_empty(),
        "{}",
        card_rule(&screen)
    );
    assert!(body.contains("▸ App shell"), "{body}");
    assert!(body.contains('━'), "{body}");
    assert_stays_on(&screen, "Overview");
    expect_visual(
        &app,
        "opt-shell-card-heading-boundary",
        "card heading plain leaves the tab underline",
        &args,
        &[("opt.shell.card_heading", "boundary")],
    );
    app.finish();
}

#[test]
fn s01_motion_paused() {
    let args = ["--page", "overview"];
    let app = launch("s01-motion-paused", &args, 120, 90);
    app.wait_text("Overview");
    open_options(&app);
    let screen = app.wait_text("Motion");
    assert_select(&column(&screen), "Motion", "paused");
    assert_stays_on(&screen, "Overview");
    assert!(!text(&screen).contains("Chrome"));
    expect_visual(
        &app,
        "opt-shell-motion-boundary",
        "harness --motion paused is the motion boundary",
        &args,
        &[("opt.shell.motion", "boundary")],
    );
    finish_inspector(&app);
}

#[test]
fn s01_inactive_none() {
    let args = [
        "--page",
        "overview",
        "--opt",
        "shell.inactive_selection=none",
    ];
    let app = launch("s01-inactive-none", &args, 120, 90);
    app.wait_text("Overview");
    open_options(&app);
    let screen = app.wait_text("Inactive selection");
    assert_select(&column(&screen), "Inactive selection", "none");
    assert_stays_on(&screen, "Overview");
    let selected = cell_bg(&screen, sidebar_point(&screen, "Overview", 20));
    let other = cell_bg(&screen, sidebar_point(&screen, "App shell", 30));
    assert_eq!(
        selected, other,
        "inactive none still fills the selected row"
    );
    expect_visual(
        &app,
        "opt-shell-inactive-selection-boundary",
        "inactive selection none while focus is in the inspector",
        &args,
        &[("opt.shell.inactive_selection", "boundary")],
    );
    finish_inspector(&app);
}

macro_rules! select_boundary {
    ($fn:ident, $case:literal, $opt:literal, $label:literal, $value:literal) => {
        #[test]
        fn $fn() {
            let args = ["--page", "overview", "--opt", $opt];
            show_option(
                stringify!($fn),
                &args,
                $label,
                Expect::Select($value),
                $case,
                concat!("boundary ", $opt),
            );
        }
    };
}

macro_rules! toggle_boundary {
    ($fn:ident, $case:literal, $opt:literal, $label:literal) => {
        #[test]
        fn $fn() {
            let args = ["--page", "overview", "--opt", $opt];
            show_option(
                stringify!($fn),
                &args,
                $label,
                Expect::Toggle("○──"),
                $case,
                concat!("boundary ", $opt),
            );
        }
    };
}

select_boundary!(
    s01_strings_ascii,
    "opt.shell.strings",
    "shell.strings=ascii",
    "Strings",
    "ascii"
);
select_boundary!(
    s01_focus_release,
    "opt.shell.focus_on",
    "shell.focus_on=release",
    "Focus on",
    "release"
);
select_boundary!(
    s01_double_click_ms,
    "opt.shell.double_click_ms",
    "shell.double_click_ms=800",
    "Double-click ms",
    "800"
);
select_boundary!(
    s01_double_click_slop,
    "opt.shell.double_click_slop",
    "shell.double_click_slop=0",
    "Double-click slop",
    "0"
);
select_boundary!(
    s01_flash_ms,
    "opt.shell.flash_ms",
    "shell.flash_ms=280",
    "Press flash ms",
    "280"
);
select_boundary!(
    s01_secondary_ignore,
    "opt.shell.secondary_outside",
    "shell.secondary_outside=ignore",
    "Right-click outside",
    "ignore"
);
select_boundary!(
    s01_autoscroll_ms,
    "opt.shell.autoscroll_ms",
    "shell.autoscroll_ms=100",
    "Autoscroll ms",
    "100"
);
select_boundary!(
    s01_track_repeat_ms,
    "opt.shell.track_repeat_ms",
    "shell.track_repeat_ms=500/100",
    "Track repeat ms",
    "500/100"
);
select_boundary!(
    s01_track_click_jump,
    "opt.shell.track_click",
    "shell.track_click=jump",
    "Track click",
    "jump"
);
select_boundary!(
    s01_caret_terminal,
    "opt.shell.caret_shape",
    "shell.caret_shape=terminal",
    "Caret shape",
    "terminal"
);
select_boundary!(
    s01_platform_other,
    "opt.shell.platform",
    "shell.platform=other",
    "Client platform",
    "other"
);
select_boundary!(
    s01_field_keys_desktop,
    "opt.shell.field_keys",
    "shell.field_keys=desktop",
    "Field keys",
    "desktop"
);
select_boundary!(
    s01_edit_entry_explicit,
    "opt.shell.edit_entry",
    "shell.edit_entry=explicit",
    "Edit entry",
    "explicit"
);
select_boundary!(
    s01_focus_fallback_first,
    "opt.shell.focus_fallback",
    "shell.focus_fallback=first",
    "Focus fallback",
    "first"
);
select_boundary!(
    s01_keyboard_off,
    "opt.shell.keyboard_protocol",
    "shell.keyboard_protocol=off",
    "Keyboard protocol",
    "off"
);
select_boundary!(
    s01_alt_with_protocol,
    "opt.shell.alt_keys",
    "shell.alt_keys=with-protocol",
    "Alt keys",
    "with-protocol"
);

toggle_boundary!(
    s01_rehover_off,
    "opt.shell.hover_refresh_after_scroll",
    "shell.hover_refresh_after_scroll=off",
    "Re-hover on scroll"
);
toggle_boundary!(
    s01_end_edit_off,
    "opt.shell.end_edit_on_dead_space",
    "shell.end_edit_on_dead_space=off",
    "End edit on blank"
);
toggle_boundary!(
    s01_autoscroll_off,
    "opt.shell.autoscroll",
    "shell.autoscroll=off",
    "Edge autoscroll"
);
toggle_boundary!(
    s01_track_repeat_off,
    "opt.shell.track_repeat",
    "shell.track_repeat=off",
    "Track repeat"
);
toggle_boundary!(
    s01_pointer_off,
    "opt.shell.pointer_shapes",
    "shell.pointer_shapes=off",
    "Pointer shapes"
);
toggle_boundary!(
    s01_click_skips_off,
    "opt.shell.click_skips_animation",
    "shell.click_skips_animation=off",
    "Click skips intro"
);
toggle_boundary!(
    s01_tooltips_off,
    "opt.shell.tooltips",
    "shell.tooltips=off",
    "Tooltips"
);
toggle_boundary!(
    s01_reveal_off,
    "opt.shell.reveal",
    "shell.reveal=off",
    "Reveal cut text"
);
toggle_boundary!(
    s01_shift_wheel_off,
    "opt.shell.shift_wheel_horizontal",
    "shell.shift_wheel_horizontal=off",
    "Shift+wheel sideways"
);
toggle_boundary!(
    s01_legacy_off,
    "opt.shell.legacy_ctrl_punctuation",
    "shell.legacy_ctrl_punctuation=off",
    "Legacy Ctrl punctuat"
);

fn wheel_grid(
    name: &str,
    args: &[&str],
    expect_id: &str,
    expect_name: &str,
    test_id: &str,
    case_role: &str,
) {
    let app = launch(name, args, 120, 40);
    let initial = app.wait_screen("sidebar settled", |screen| {
        text(screen).contains("Northwind Traders")
            && nav_column(screen).iter().any(|row| row.contains("Screens"))
    });
    assert!(row_text(&initial, 0).contains("Tables › Data grid"));
    let cell = find(&initial, "1001");
    let before = sidebar_rows(&initial);
    app.wheel(
        Point {
            x: cell.x + 2,
            y: cell.y,
        },
        true,
        1,
    );
    let down = app.wait_screen(&format!("wheel shows {expect_id}"), |screen| {
        let row = row_text(screen, u32::from(cell.y));
        row.contains(expect_id) && row.contains(expect_name)
    });
    assert_eq!(sidebar_rows(&down), before, "wheel moved the sidebar");
    assert!(row_text(&down, 0).contains("Tables › Data grid"));
    expect_visual(
        &app,
        test_id,
        &format!("one wheel shows {expect_id}"),
        args,
        &[("opt.shell.wheel_lines", case_role)],
    );
    app.finish();
}

#[test]
fn s01_wheel_lines_default() {
    let args = ["--page", "datagrid", "--frame", "40"];
    wheel_grid(
        "s01-wheel-default",
        &args,
        "1004",
        "Fabrikam Robotics",
        "opt-shell-wheel-lines-default",
        "default",
    );
}

#[test]
fn s01_wheel_lines_one() {
    let args = [
        "--page",
        "datagrid",
        "--frame",
        "40",
        "--opt",
        "shell.wheel_lines=1",
    ];
    wheel_grid(
        "s01-wheel-one",
        &args,
        "1002",
        "Blue Yonder Airlines",
        "opt-shell-wheel-lines-boundary",
        "boundary",
    );
}

fn click_inspector(app: &App, needle: &str) -> Point {
    let screen = app.screen();
    let point = find_all(&screen, needle)
        .into_iter()
        .filter(|point| point.x >= 88)
        .max_by_key(|point| point.y)
        .unwrap_or_else(|| panic!("inspector {needle} missing\n{}", text(&screen)));
    app.click(point);
    point
}

#[test]
fn s01_inspector_sections() {
    let args = ["--page", "overview"];
    let app = launch("s01-inspector", &args, 120, 40);
    app.wait_text("Overview");
    app.key("i");
    // "Inspector · on" paints before the Events heading under the state rows.
    let state = app.wait_screen("inspector state section", |screen| {
        let visible = text(screen);
        visible.contains("Inspector · on")
            && visible.contains("NAV")
            && visible.contains("Events")
            && visible.contains("mode")
    });
    let state_text = text(&state);
    assert!(state_text.contains("NAV"), "{state_text}");
    assert!(state_text.contains("Events"), "{state_text}");
    assert!(state_text.contains("mode"), "{state_text}");
    let tab = find_in(&state, 80, 0, 119, 8, "Options");
    app.click(tab);
    app.wait_text("Glyphs");
    click_inspector(&app, "unicode");
    app.wait_text("ascii");
    click_inspector(&app, "ascii");
    let changed = app.wait_screen("ascii chrome", |screen| {
        text(screen).contains("> App shell")
    });
    assert!(row_text(&changed, 0).contains("Overview"));
    assert!(!text(&changed).contains("Chrome"));
    app.resize(72, 24);
    let narrow = app.wait_screen("narrow inspector", |screen| {
        text(screen).contains("Inspector · on") && text(screen).lines().count() == 24
    });
    assert!(text(&narrow).contains("Overview"));
    expect_visual(
        &app,
        "sc-shell-inspector-narrow",
        "narrow overlay while the inspector is open, before restoring 120x40",
        &args,
        &[("sc.shell.inspector", "narrow")],
    );
    app.resize(120, 40);
    app.wait_text("120×40");
    app.key("escape");
    app.key("i");
    let closed = app.wait_absent("Inspector · on");
    let closed_text = text(&closed);
    assert!(closed_text.contains("> App shell"), "{closed_text}");
    assert!(row_text(&closed, 0).contains("Overview"));
    assert!(!closed_text.contains("Chrome"), "{closed_text}");
    expect_visual(
        &app,
        "sc-shell-inspector-120x40",
        "close the inspector after changing glyphs; the page stays Overview",
        &args,
        &[("sc.shell.inspector", "checkpoint")],
    );
    app.finish();
}

#[test]
fn s01_sidebar_all() {
    let args = ["--page", "overview", "--nav", "all"];
    let app = launch("s01-nav-all", &args, 120, 40);
    // The first paint lists Chrome before the rest of the expanded groups
    // arrive. Buttons is the row that proves All has opened Inputs.
    let open = app.wait_screen("nav all expanded", |screen| {
        let visible = text(screen);
        visible.contains("Chrome") && visible.contains("Buttons")
    });
    assert!(row_text(&open, 0).contains("Overview"));
    expect_visual(
        &app,
        "sc-shell-sidebar-policy-all-open",
        "nav all shows every group expanded on Overview",
        &args,
        &[("sc.shell.sidebar-policy", "all-open")],
    );
    app.key("down");
    wait_focus(&app, "App shell");
    app.key("left");
    let collapsed = app.wait_screen("app shell collapsed", |screen| {
        !text(screen).contains("Chrome")
    });
    assert!(text(&collapsed).contains("Buttons"));
    app.key("down");
    wait_focus(&app, "Inputs");
    app.key("down");
    wait_focus(&app, "Buttons");
    app.key("enter");
    let buttons = app.wait_text("Inputs & forms › Buttons");
    assert!(
        !text(&buttons).contains("Chrome"),
        "All re-expanded the collapsed group\n{}",
        text(&buttons)
    );
    expect_visual(
        &app,
        "sc-shell-sidebar-policy-120x40",
        "under All, a collapsed group stays collapsed after opening Buttons",
        &args,
        &[("sc.shell.sidebar-policy", "checkpoint")],
    );
    app.finish();
}

#[test]
fn s01_sidebar_accordion() {
    let args = ["--page", "chrome", "--nav", "accordion"];
    let app = launch("s01-nav-accordion", &args, 120, 40);
    let chrome = app.wait_text("App shell › Chrome");
    assert!(!text(&chrome).contains("Buttons"));
    for _ in 0..6 {
        app.key("down");
    }
    wait_focus(&app, "Inputs");
    app.key("right");
    app.wait_text("Buttons");
    app.key("down");
    wait_focus(&app, "Buttons");
    app.key("enter");
    let buttons = app.wait_text("Inputs & forms › Buttons");
    assert!(
        !text(&buttons).contains("Chrome"),
        "accordion left App shell expanded\n{}",
        text(&buttons)
    );
    expect_visual(
        &app,
        "sc-shell-sidebar-policy-accordion",
        "accordion keeps only the Buttons group expanded",
        &args,
        &[("sc.shell.sidebar-policy", "accordion")],
    );
    app.finish();
}

#[test]
fn s01_sidebar_current() {
    // Startup on Chrome marks App shell as the auto group (`sync_nav`).
    // A Right-expand of Inputs is manual, so it stays when the auto group
    // collapses on the way to Tables.
    let args = ["--page", "chrome", "--nav", "current"];
    let app = launch("s01-nav-current", &args, 120, 40);
    let chrome = app.wait_text("App shell › Chrome");
    assert!(!text(&chrome).contains("Buttons"));
    for _ in 0..6 {
        app.key("down");
    }
    wait_focus(&app, "Inputs");
    app.key("right");
    app.wait_text("Buttons");
    for _ in 0..9 {
        app.key("down");
    }
    wait_focus(&app, "Tables");
    app.key("right");
    app.wait_text("Basic table");
    app.key("down");
    wait_focus(&app, "Basic table");
    app.key("enter");
    let page = app.wait_text("Tables › Basic table");
    let body = text(&page);
    assert!(
        !body.contains("Chrome"),
        "current policy left the auto group expanded\n{body}"
    );
    assert!(
        body.contains("Buttons"),
        "current policy collapsed the manually expanded group\n{body}"
    );
    expect_visual(
        &app,
        "sc-shell-sidebar-policy-current",
        "current policy collapses the auto group and keeps the manually expanded Inputs group",
        &args,
        &[("sc.shell.sidebar-policy", "current")],
    );
    app.finish();
}

fn seen_within(app: &App, needle: &str, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        if text(&app.screen()).contains(needle) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn s01_shell_override() {
    // At 120x40 the Shell block starts below the inspector viewport.
    let args = ["--page", "statushints"];
    let app = launch("s01-shell-override", &args, 120, 90);
    let start = app.wait_text("Status & hints");
    assert!(row_text(&start, 0).contains("App shell › Status & hints"));
    if !text(&start).contains("shell.motion")
        && let Some(layers) = find_all(&start, "Layers")
            .into_iter()
            .find(|point| point.x > 24)
    {
        app.wheel(layers, true, 8);
    }
    if !seen_within(&app, "shell.motion", Duration::from_millis(800)) {
        app.key("tab");
        for _ in 0..4 {
            app.key("pagedown");
            if seen_within(&app, "shell.motion", Duration::from_millis(400)) {
                break;
            }
        }
    }
    let caption = app.wait_text("shell.motion");
    assert!(text(&caption).contains("paused"));
    assert!(text(&caption).contains("Indexing idle") || text(&caption).contains("Motion"));
    open_options(&app);
    click_inspector(&app, "paused");
    app.wait_screen("motion list", |screen| {
        find_all(screen, "reduced")
            .into_iter()
            .any(|point| point.x >= 88)
    });
    click_inspector(&app, "reduced");
    let changed = app.wait_screen("motion reduced", |screen| {
        value_line(&column(screen), "Motion")
            .trim()
            .trim_start_matches('▎')
            .trim()
            .starts_with("reduced")
    });
    assert!(row_text(&changed, 0).contains("App shell › Status & hints"));
    expect_visual(
        &app,
        "sc-shell-shell-override-reduced",
        "shell.motion reduced from the inspector at 120x90; the status hints page stays",
        &args,
        &[("sc.shell.shell-override", "reduced")],
    );
    app.resize(120, 40);
    let narrow = app.wait_text("120×40");
    assert!(row_text(&narrow, 0).contains("App shell › Status & hints"));
    expect_visual(
        &app,
        "sc-shell-shell-override-120x40",
        "after shell.motion reduced, restore 120x40 without retargeting the page",
        &args,
        &[("sc.shell.shell-override", "checkpoint")],
    );
    app.key("escape");
    let options_tab = find_all(&app.screen(), "Options")
        .into_iter()
        .find(|point| point.x < 70)
        .expect("page Options tab");
    app.click(options_tab);
    let page_options = app.wait_text("Top layer");
    let page_body = text(&page_options);
    let lines: Vec<&str> = page_body.lines().collect();
    let index = lines
        .iter()
        .position(|line| line.contains("Top layer"))
        .expect("Top layer");
    assert!(
        lines[index + 1].contains("screen"),
        "page option changed\n{}",
        text(&page_options)
    );
    assert!(row_text(&page_options, 0).contains("Status & hints"));
    expect_visual(
        &app,
        "sc-shell-shell-override-page-option",
        "Top layer stays screen after only shell.motion changed",
        &args,
        &[("sc.shell.shell-override", "page-option")],
    );
    finish_inspector(&app);
}
