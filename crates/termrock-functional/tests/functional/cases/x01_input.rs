//! Cross-application keyboard fallback, modified keys, and modal paste.
//!
//! Tuiscotti test ids reject `.` and uppercase, so the baseline directory is
//! `pre_refactor/x01_input/`. The checkpoint session pastes into the open
//! Rename task dialog, captures 120×40, resizes to 80×24, captures that
//! frame, then resizes back. `Launch::new` leaves the clock off.
//!
//! macOS uses `FieldKeys::Emacs`, so Ctrl+W is Cut. The PTY delivers that
//! chord: the selected name is cleared and `w` is not inserted.

use tui_test::TerminalSnapshot;

use crate::support::{App, Launch, SHOWCASE, find_all, row_text, text, try_launch};

const TOKEN: &str = "qx01paste";
const TITLE: &str = "Rename task";
const HELP: &str = "Shown in the task list";
const TASK: &str = "Task: Migrate sessions table";
const PAGE: &str = "Nothing yet";
const RENAMED: &str = "Renamed to";
const JOINED: &str = "alpha beta";
const WIDE_ID: &str = "pre_refactor/x01_input/x01_input_input_120x40";
const NARROW_ID: &str = "pre_refactor/x01_input/x01_input_input_narrow";

fn open(name: &str, env: &[(&str, &str)]) -> App {
    let mut launch = Launch::new(name, SHOWCASE);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = vec!["--page".to_string(), "dialogs".to_string()];
    launch.env = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn click_main(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x >= 24 && hit.y >= 6)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn open_rename(app: &App) -> TerminalSnapshot {
    app.wait_text(PAGE);
    click_main(app, "Rename task");
    app.wait_text(HELP)
}

fn value_row(screen: &TerminalSnapshot) -> String {
    let lines: Vec<String> = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .collect();
    lines
        .iter()
        .position(|line| line.contains("Task name"))
        .and_then(|index| lines.get(index + 1))
        .cloned()
        .unwrap_or_else(|| panic!("field row missing\n{}", text(screen)))
}

fn task_row(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains("Task:"))
        .unwrap_or_else(|| panic!("task label missing\n{}", text(screen)))
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn field_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    text(screen).contains("Task name") && value_row(screen).contains(needle)
}

/// The Results line is covered until the dialog and the page have both
/// painted. Returning on the field token alone records that covered frame.
fn dialog_holds(screen: &TerminalSnapshot, token: &str) -> bool {
    let visible = text(screen);
    if !(visible.contains(TITLE)
        && visible.contains(HELP)
        && visible.contains(TASK)
        && visible.contains(PAGE)
        && !visible.contains(RENAMED)
        && field_has(screen, token))
    {
        return false;
    }
    !task_row(screen).contains(token)
}

fn assert_dialog_holds(screen: &TerminalSnapshot, token: &str) {
    require(screen, TITLE);
    require(screen, HELP);
    require(screen, TASK);
    require(screen, PAGE);
    refuse(screen, RENAMED);
    assert!(
        field_has(screen, token),
        "field missing {token}\n{}",
        value_row(screen)
    );
    assert!(
        !task_row(screen).contains(token),
        "page task label gained {token}\n{}",
        task_row(screen)
    );
}

fn close(app: &App) {
    app.key("escape");
    let screen = app.wait_screen("escape cancels the dialog", |screen| {
        let visible = text(screen);
        visible.contains("Action(0)") && !visible.contains(HELP)
    });
    require(&screen, "Migrate sessions table");
    refuse(&screen, RENAMED);
    app.finish();
}

fn assert_modified_key() {
    let app = open("x01-input-modified", &[]);
    open_rename(&app);
    app.key("ctrl+w");
    let screen = app.wait_screen("ctrl+w cuts the selected name", |screen| {
        let value = value_row(screen);
        !value.contains("Migrate")
            && !value.contains('w')
            && text(screen).contains(HELP)
            && text(screen).contains(TASK)
    });
    assert!(app.child_running(), "ctrl+w stopped the process");
    require(&screen, TASK);
    require(&screen, PAGE);
    refuse(&screen, RENAMED);
    assert!(
        !field_has(&screen, "w"),
        "ctrl+w inserted w\n{}",
        value_row(&screen)
    );
    close(&app);
}

fn assert_multiline() {
    let app = open("x01-input-multiline", &[]);
    open_rename(&app);
    app.paste("alpha\nbeta");
    let screen = app.wait_screen("multiline paste joined", |screen| {
        dialog_holds(screen, JOINED)
    });
    assert_dialog_holds(&screen, JOINED);
    assert_eq!(
        text(&screen).matches("beta").count(),
        1,
        "multiline paste painted a second line\n{}",
        text(&screen)
    );
    close(&app);
}

fn assert_unicode() {
    let app = open("x01-input-unicode", &[]);
    open_rename(&app);
    app.paste("λ");
    let screen = app.wait_screen("unicode paste", |screen| dialog_holds(screen, "λ"));
    assert!(app.child_running(), "unicode paste stopped the process");
    assert_dialog_holds(&screen, "λ");
    close(&app);
}

fn assert_control_paste() {
    let app = open("x01-input-control", &[]);
    open_rename(&app);
    app.paste("q");
    let queued = app.wait_screen("pasted q", |screen| {
        dialog_holds(screen, "q") && !field_has(screen, "Migrate")
    });
    assert!(app.child_running(), "pasted q stopped the process");
    assert_dialog_holds(&queued, "q");
    app.paste("Ctrl+Q");
    let screen = app.wait_screen("pasted shortcut text", |screen| {
        dialog_holds(screen, "qCtrl+Q")
    });
    assert!(app.child_running(), "pasted Ctrl+Q stopped the process");
    assert_dialog_holds(&screen, "qCtrl+Q");
    close(&app);
}

fn assert_protocol(name: &str, env: &[(&str, &str)]) {
    let app = open(name, env);
    open_rename(&app);
    app.type_text("z");
    let screen = app.wait_screen("typed z", |screen| {
        dialog_holds(screen, "z") && !field_has(screen, "Migrate")
    });
    assert!(app.child_running(), "{name} stopped the process");
    assert_dialog_holds(&screen, "z");
    assert!(
        !task_row(&screen).contains('z'),
        "page task label gained z\n{}",
        task_row(&screen)
    );
    close(&app);
}

fn assert_checkpoints() {
    let app = open("x01-input-checkpoint", &[]);
    open_rename(&app);
    app.paste(TOKEN);
    let wide = app.wait_screen(WIDE_ID, |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_holds(screen, TOKEN)
    });
    assert_dialog_holds(&wide, TOKEN);
    assert_eq!(wide.cols(), 120);
    assert_eq!(wide.rows(), 40);
    crate::support::assert_frame(&app, &wide, WIDE_ID);

    app.resize(80, 24);
    // The first 80×24 paint still has the wide page clipped around the
    // dialog. The paste is already visible there. The reference frame is the
    // later paint, with the narrow footer and the width fact.
    app.wait_screen("narrow dialog chrome", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(TOKEN)
            && visible.contains("Enter Rename")
            && visible.contains("width 54")
    });
    let narrow = app.settle();
    require(&narrow, TITLE);
    require(&narrow, HELP);
    refuse(&narrow, RENAMED);
    refuse(&narrow, TASK);
    assert!(
        field_has(&narrow, TOKEN),
        "narrow field\n{}",
        value_row(&narrow)
    );
    assert_eq!(text(&narrow).matches(TOKEN).count(), 1);
    assert_eq!(narrow.cols(), 80);
    assert_eq!(narrow.rows(), 24);
    crate::support::assert_frame(&app, &narrow, NARROW_ID);

    app.resize(120, 40);
    let restored = app.wait_screen("restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_holds(screen, TOKEN)
    });
    assert_dialog_holds(&restored, TOKEN);
    assert!(app.child_running(), "resize stopped the process");
    close(&app);
}

#[test]
fn x01_input_route() {
    let app = open("x01-input-route", &[]);
    let opened = open_rename(&app);
    require(&opened, TITLE);
    require(&opened, HELP);
    require(&opened, "Migrate sessions table");
    assert!(app.child_running(), "dialog open stopped the process");
    close(&app);
}

#[test]
fn x01_input_input() {
    assert_modified_key();
    assert_multiline();
    assert_unicode();
    assert_control_paste();
    assert_protocol(
        "x01-input-protocol-off",
        &[("TERMROCK_KEYBOARD_PROTOCOL", "off")],
    );
    assert_protocol("x01-input-protocol-default", &[]);
    assert_checkpoints();
}
