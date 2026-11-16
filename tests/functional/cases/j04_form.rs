//! Jackin account form validation, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j04_form/`. The shots are the open new-account form showing
//! `Display name is required`, before Cancel and before a valid save. That
//! frame has no typed secret. A same-size resize does not force a frame, and
//! the cursor column is part of the hash, so each shot nudges the width and
//! comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const OVERVIEW: &str = "Accounts › Overview";
const EMPTY: &str = "No accounts registered";
const NEW: &str = "New account";
const UNSAVED: &str = "form · unsaved";
const EDITS: &str = "form · edits";
const PLACEHOLDER: &str = "Personal, Work, Team…";
const NOT_CHOSEN: &str = "not chosen";
const LOCAL: &str = "Local agent folder";
const PLAIN: &str = "Plain-text API key";
const FOLDER_LABEL: &str = "Claude profile / home folder";
const KEY_HINT: &str = "paste the key";
const CANCELLED: &str = "Cancelled · nothing saved";
const REQUIRED: &str = "Required";
const NAME_REQUIRED: &str = "Display name is required";
const TOO_LONG: &str = "Display name is too long (64 max)";
const CHOOSE_OP: &str = "Choose a 1Password item and field first";
const FOLDER_EMPTY: &str = "Enter or browse to the local agent folder";
const KEY_EMPTY: &str = "API key required: the field is empty";
const CANCEL: &str = "Cancel";
const SAVE: &str = "Save";
const DESK: &str = "Desk";
const CLAUDE_DESK: &str = "Claude · Desk";
const SAVED_DESK: &str = "Saved Claude · Desk";
const EDIT_DESK: &str = "Edit account · Claude · Desk";
const HOME: &str = "~/.claude";
const OTHER_HOME: &str = "/Users/alexey/.claude";
const WIDE: &str = "J04.form.form.120x40";
const NARROW: &str = "J04.form.form.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j04_form/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    // No `--scenario`. The default is first-use (`src/bin/jackin/main.rs`), so
    // the inventory row stays scenario null. accounts-mixed leaves an account
    // Refreshing, and that header spinner moves under reduced motion.
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    app.wait_text(DIRECTORY)
}

fn open_accounts(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(DIRECTORY),
        "refusing c unless the manager is showing\n{}",
        text(&before)
    );
    app.key("c");
    let screen = app.wait_screen("accounts", |screen| {
        let visible = text(screen);
        visible.contains(OVERVIEW) && visible.contains(EMPTY)
    });
    let line = focused_line(&screen);
    assert!(
        line.contains("Overview"),
        "tree does not have focus\n{line}\n{}",
        text(&screen)
    );
    screen
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn dialog_points(screen: &TerminalSnapshot, label: &str) -> Vec<Point> {
    find_all(screen, label)
        .into_iter()
        .filter(|point| row_text(screen, u32::from(point.y)).contains('│'))
        .collect()
}

fn click_dialog(app: &App, label: &str) {
    let screen = app.screen();
    let hits = dialog_points(&screen, label);
    assert_eq!(
        hits.len(),
        1,
        "one {label:?} in the dialog\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_form_button(app: &App, label: &str) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(&screen, u32::from(point.y));
            row.contains(CANCEL) && row.contains(SAVE) && row.contains('│')
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one form button {label:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn open_new(app: &App) -> TerminalSnapshot {
    app.key("a");
    app.wait_screen("new account", |screen| {
        let visible = text(screen);
        visible.contains(NEW)
            && visible.contains(UNSAVED)
            && visible.contains(PLACEHOLDER)
            && visible.contains(NOT_CHOSEN)
            && !visible.contains("Saved ")
    })
}

fn set_name(app: &App, value: &str) {
    click_dialog(app, "Display name");
    app.key("ctrl+a");
    app.type_text(value);
    let marker: String = value.trim().chars().take(12).collect();
    if marker.is_empty() {
        app.wait_screen("whitespace name", |screen| {
            !text(screen).contains(PLACEHOLDER) && text(screen).contains(NEW)
        });
        return;
    }
    app.wait_text(&marker);
}

fn choose_local(app: &App, folder_label: &str) {
    click_dialog(app, LOCAL);
    app.wait_text(folder_label);
}

fn choose_plain(app: &App) {
    click_dialog(app, PLAIN);
    app.wait_text(KEY_HINT);
}

fn set_folder(app: &App, label: &str, path: &str) {
    click_dialog(app, label);
    app.key("ctrl+a");
    app.type_text(path);
    let head: String = path.chars().take(8).collect();
    if head != "~/.claude" {
        app.wait_text(&head);
    }
}

/// The dialog paints `! {message}` in an inner width of 64 and ellipsizes the rest.
fn shown(message: &str) -> String {
    const WIDTH: usize = 64;
    let line = format!("! {message}");
    if line.chars().count() <= WIDTH {
        return message.to_string();
    }
    line.chars().skip(2).take(WIDTH - 3).collect()
}

fn open_error(screen: &TerminalSnapshot, message: &str) -> bool {
    let visible = text(screen);
    visible.contains(NEW)
        && visible.contains(&shown(message))
        && visible.contains(UNSAVED)
        && !visible.contains("Saved ")
}

fn submit_error(app: &App, message: &str) -> TerminalSnapshot {
    // Commit the field that is still editing. A dirty blur emits Changed, and
    // form_changed clears the error save_form just painted.
    app.key("tab");
    click_form_button(app, SAVE);
    let screen = app.wait_screen(message, |screen| open_error(screen, message));
    assert!(
        open_error(&screen, message),
        "form closed on {message}\n{}",
        text(&screen)
    );
    screen
}

fn name_error(screen: &TerminalSnapshot) -> bool {
    open_error(screen, NAME_REQUIRED) && !text(screen).contains(REQUIRED)
}

fn dismiss_empty(app: &App) -> TerminalSnapshot {
    app.key("escape");
    app.wait_screen("empty registry", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED)
            && visible.contains(EMPTY)
            && visible.contains(OVERVIEW)
            && !visible.contains(NEW)
            && !visible.contains("Saved ")
            && !visible.contains(CLAUDE_DESK)
    })
}

fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.flash_expired();
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        quiet_frame(&screen),
        "nudge left the frame\n{}",
        text(&screen)
    );
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
}

fn shoot(app: &App, checkpoint: &str, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    nudge(app, quiet_frame);
    app.visual(&visual_id(checkpoint));
}

fn reject(app: &App, message: &str, prepare: impl Fn(&App)) {
    open_new(app);
    prepare(app);
    submit_error(app, message);
    dismiss_empty(app);
}

fn fill_desk(app: &App) {
    set_name(app, DESK);
    choose_local(app, FOLDER_LABEL);
    set_folder(app, FOLDER_LABEL, HOME);
}

#[test]
fn j04_form_route() {
    let app = open("j04-form-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert!(!text(&manager).contains(NEW), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let accounts = open_accounts(&app);
    assert!(text(&accounts).contains(EMPTY), "{}", text(&accounts));
    assert_eq!((accounts.cols(), accounts.rows()), (120, 40));
    let form = open_new(&app);
    assert!(
        text(&form).contains(NEW) && text(&form).contains(UNSAVED),
        "{}",
        text(&form)
    );
    app.key("escape");
    let cancelled = app.wait_screen("form cancelled", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && visible.contains(EMPTY) && !visible.contains(NEW)
    });
    assert!(!text(&cancelled).contains("Saved "), "{}", text(&cancelled));
    app.key("escape");
    let back = app.wait_screen("manager", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) && !visible.contains(OVERVIEW) && !visible.contains(NEW)
    });
    assert!(text(&back).contains(DIRECTORY), "{}", text(&back));
    app.finish();
}

#[test]
fn j04_form_form() {
    let app = open("j04-form-form");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    open_accounts(&app);

    open_new(&app);
    set_name(&app, " ");
    let failed = submit_error(&app, NAME_REQUIRED);
    assert!(name_error(&failed), "whitespace name\n{}", text(&failed));
    assert!(!text(&failed).contains("sk-"), "{}", text(&failed));
    assert_eq!((failed.cols(), failed.rows()), (120, 40));
    shoot(&app, WIDE, name_error);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && name_error(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert!(
        text(&narrow).contains(NAME_REQUIRED) && text(&narrow).contains(NEW),
        "narrow form\n{}",
        text(&narrow)
    );
    shoot(&app, NARROW, name_error);
    app.resize(120, 40);
    let restored = app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && name_error(screen)
    });
    assert!(
        text(&restored).contains(NAME_REQUIRED) && text(&restored).contains(NEW),
        "error dismissed before cancel\n{}",
        text(&restored)
    );

    dismiss_empty(&app);
    let reopened = open_new(&app);
    assert!(
        text(&reopened).contains(PLACEHOLDER)
            && text(&reopened).contains(NOT_CHOSEN)
            && !text(&reopened).contains(FOLDER_LABEL)
            && !text(&reopened).contains(DESK),
        "reopen kept the rejected name\n{}",
        text(&reopened)
    );
    dismiss_empty(&app);

    reject(&app, REQUIRED, |_| {});
    let overlong = format!("LONGNAME{}", "n".repeat(57));
    assert_eq!(overlong.chars().count(), 65);
    reject(&app, TOO_LONG, |app| set_name(app, &overlong));
    reject(&app, CHOOSE_OP, |app| set_name(app, "Alpha"));
    reject(&app, FOLDER_EMPTY, |app| {
        set_name(app, "Alpha");
        choose_local(app, FOLDER_LABEL);
    });
    reject(&app, "Folder not found: nowhere", |app| {
        set_name(app, "Alpha");
        choose_local(app, FOLDER_LABEL);
        set_folder(app, FOLDER_LABEL, "nowhere");
    });
    reject(
        &app,
        "Folder not readable: permission denied for ~/.codex-locked",
        |app| {
            set_name(app, "Alpha");
            choose_local(app, FOLDER_LABEL);
            set_folder(app, FOLDER_LABEL, "~/.codex-locked");
        },
    );
    reject(
        &app,
        "No credential found: ~/.grok has no auth.json",
        |app| {
            set_name(app, "Alpha");
            choose_local(app, FOLDER_LABEL);
            set_folder(app, FOLDER_LABEL, "~/.grok");
        },
    );
    reject(&app, KEY_EMPTY, |app| {
        set_name(app, "Alpha");
        choose_plain(app);
    });

    open_new(&app);
    fill_desk(&app);
    app.key("tab");
    click_form_button(&app, CANCEL);
    let cancelled = app.wait_screen("valid edit cancelled", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && visible.contains(EMPTY) && !visible.contains(NEW)
    });
    assert!(
        !text(&cancelled).contains(CLAUDE_DESK) && !text(&cancelled).contains("Saved "),
        "cancel stored Desk\n{}",
        text(&cancelled)
    );
    let blank = open_new(&app);
    assert!(
        text(&blank).contains(PLACEHOLDER)
            && text(&blank).contains(NOT_CHOSEN)
            && !text(&blank).contains(FOLDER_LABEL)
            && !text(&blank).contains(DESK),
        "cancel did not restore the blank form\n{}",
        text(&blank)
    );
    fill_desk(&app);
    app.key("tab");
    click_form_button(&app, SAVE);
    let saved = app.wait_screen("saved desk", |screen| {
        let visible = text(screen);
        visible.contains(SAVED_DESK) && !visible.contains(NEW) && !visible.contains(EMPTY)
    });
    assert!(text(&saved).contains(SAVED_DESK), "{}", text(&saved));
    app.key("e");
    // The detail pane behind the dialog already shows ~/.claude. The folder
    // field label is what shows the reopened form has painted.
    let edited = app.wait_screen("reopen saved", |screen| {
        let visible = text(screen);
        visible.contains(EDIT_DESK)
            && visible.contains(FOLDER_LABEL)
            && visible.contains(HOME)
            && visible.contains(EDITS)
    });
    assert!(
        text(&edited).contains(DESK) && text(&edited).contains(FOLDER_LABEL),
        "reopen lost the saved values\n{}",
        text(&edited)
    );
    app.key("escape");
    app.wait_screen("edit cancelled", |screen| {
        text(screen).contains(CANCELLED) && !text(screen).contains(EDIT_DESK)
    });

    open_new(&app);
    set_name(&app, "Other");
    choose_local(&app, FOLDER_LABEL);
    set_folder(&app, FOLDER_LABEL, HOME);
    let duplicate = submit_error(
        &app,
        "Already registered: this source is used by Claude · Desk",
    );
    assert!(
        text(&duplicate).contains(NEW) && !text(&duplicate).contains("Saved "),
        "duplicate source saved\n{}",
        text(&duplicate)
    );
    app.key("escape");
    app.wait_screen("other absent", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && visible.contains(DESK) && !visible.contains("Other")
    });

    open_new(&app);
    set_name(&app, "Other");
    choose_local(&app, FOLDER_LABEL);
    set_folder(&app, FOLDER_LABEL, OTHER_HOME);
    let identity = submit_error(
        &app,
        "Already registered: Claude · Desk authenticates the same identity",
    );
    assert!(text(&identity).contains(NEW), "{}", text(&identity));
    app.key("escape");
    let kept = app.wait_screen("identity rejected", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED)
            && visible.contains(DESK)
            && !visible.contains("Other")
            && !visible.contains(NEW)
    });
    assert!(!text(&kept).contains("Saved "), "{}", text(&kept));
    app.key("e");
    let still = app.wait_screen("desk unchanged", |screen| {
        text(screen).contains(EDIT_DESK) && text(screen).contains(HOME)
    });
    assert!(
        !text(&still).contains("Other"),
        "a rejected form changed Desk\n{}",
        text(&still)
    );
    // The open form owns the key, so Ctrl+Q never reaches the shell quit binding.
    app.key("escape");
    app.wait_screen("edit closed", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED) && visible.contains(DESK) && !visible.contains(EDIT_DESK)
    });
    app.finish();
}
