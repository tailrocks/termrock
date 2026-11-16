//! Jackin workspace save, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j02_save/`. The shots are the open payments-platform save
//! preview, before Cancel and before Save. A same-size resize does not force
//! a frame, and the cursor column is part of the hash, so each shot nudges
//! the width and comes back.
//!
//! `hard-cases` sets `save_fails_once`. The first confirm fails and leaves
//! the edit. The flag clears, so the next confirm writes.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const RELEASE: &str = "release-automation";
const CUSTOMER: &str = "customer-portal";
/// The Daemon heading's `▲ stale ·` row sits below this detail. The shell
/// chip and the instances meta both paint `▲ daemon stale` from the same
/// `DaemonHealth::Stale` arm.
const STALE: &str = "▲ daemon stale";
const NAME_HELP: &str = "Directory basename by default";
const KEEP: &str = "Keep awake";
const CHECKED: &str = "[✓]";
const UNCHECKED: &str = "[ ]";
const PREVIEW_TITLE: &str = "Save workspace";
const ONE_CHANGE: &str = "1 change";
const DIFF: &str = "~ keep_awake true → false";
const SAVED: &str = "Workspace payments-platform saved";
const FAILED: &str = "Save failed · payments-platform";
const EACCES: &str = "write failed: ~/.jackin/workspaces is not writable (EACCES)";
const INTACT: &str = "your edits are intact · nothing was written";
const NEXT: &str = "fix the permission and Save again";
const NOT_SAVED: &str = "Not saved · keep editing";
const UNSAVED_TITLE: &str = "Unsaved changes";
/// `Dialog::confirm` is 54 columns, so the sentence wraps after "be".
const UNSAVED_LEAD: &str = "Save changes before leaving?";
const UNSAVED_MID: &str = "1 change would be";
const UNSAVED_END: &str = "lost.";
const DISCARDED: &str = "Discarded 1 change";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Discard";
const CONFIRM: &str = "Save";
const FOOTER_SAVE: &str = "Save…";
const WIDE: &str = "J02.save.save.120x40";
const NARROW: &str = "J02.save.save.narrow";
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_save/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "hard-cases".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("manager or intro", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) || visible.contains("Stand up, operator")
    });
    if text(&screen).contains("Stand up, operator") {
        app.key("space");
        return app.wait_text(DIRECTORY);
    }
    screen
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn line_with(screen: &TerminalSnapshot, needle: &str) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(needle))
        .unwrap_or("")
        .to_string()
}

fn keep_awake(screen: &TerminalSnapshot, on: bool) -> bool {
    let line = line_with(screen, KEEP);
    if on {
        line.contains(CHECKED)
    } else {
        line.contains(UNCHECKED) && !line.contains(CHECKED)
    }
}

fn noisy(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("s ago") || SPINNER.iter().any(|glyph| visible.contains(*glyph))
}

/// `editor.rs` `crumb` is `Workspaces › payments-platform › edit`, but
/// `segments::render` drops that priority-7 chip when the hard-cases bar
/// already holds the priority-8 index error and `▲ daemon stale`. The name
/// field is the painted identity.
fn plain_name(screen: &TerminalSnapshot) -> bool {
    text(screen).lines().any(|line| {
        line.match_indices(PAYMENTS).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            !matches!(before, Some('/'))
        })
    })
}

fn editor_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(NAME_HELP)
        && visible.contains(KEEP)
        && visible.contains(FOOTER_SAVE)
        && plain_name(screen)
}

fn lists_workspace(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| {
        if line.contains(" saved")
            || line.contains("Save failed")
            || line.contains("Save workspace")
            || line.contains("› edit")
        {
            return false;
        }
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            !matches!(before, Some('/'))
        })
    })
}

fn assert_siblings(screen: &TerminalSnapshot) {
    assert!(lists_workspace(screen, INFRA), "{}", text(screen));
    assert!(lists_workspace(screen, RELEASE), "{}", text(screen));
    assert!(lists_workspace(screen, CUSTOMER), "{}", text(screen));
}

fn preview_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(PREVIEW_TITLE)
        && visible.contains(PAYMENTS)
        && visible.contains(ONE_CHANGE)
        && visible.contains(DIFF)
        && visible.contains(CANCEL)
        && visible.contains(CONFIRM)
        && line.contains(CANCEL)
        && !visible.contains(SAVED)
        && !visible.contains("Save failed")
        && !visible.contains("Saving")
        && !noisy(screen)
}

fn select_payments(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(DIRECTORY),
        "manager did not start on Current directory\n{}",
        text(&screen)
    );
    app.key("down");
    app.wait_screen("payments-platform", |screen| {
        focused_line(screen).contains(PAYMENTS)
    })
}

fn open_editor(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(PAYMENTS),
        "refusing e unless payments-platform is focused\n{}",
        text(&screen)
    );
    app.key("e");
    app.wait_screen("editor", editor_open)
}

fn focus_keep_awake(app: &App) -> TerminalSnapshot {
    app.key("enter");
    app.wait_screen("name field", |screen| {
        let line = focused_line(screen);
        line.contains(PAYMENTS) && !line.contains('/')
    });
    // Immediate entry edits on focus. A single-line field ignores Down until
    // Tab commits and moves to the next stop (input.rs, field_common.rs).
    app.key("tab");
    app.wait_screen("left name", |screen| {
        let line = focused_line(screen);
        line.contains(KEEP) || line.contains("Choose")
    });
    for _ in 0..12 {
        let screen = app.screen();
        if focused_line(&screen).contains(KEEP) {
            return screen;
        }
        let line = focused_line(&screen);
        app.key("down");
        app.wait_screen("field", |screen| focused_line(screen) != line);
    }
    let screen = app.screen();
    panic!("never focused Keep awake\n{}", text(&screen));
}

fn toggle_keep_awake_off(app: &App) -> TerminalSnapshot {
    let focused = focus_keep_awake(app);
    assert!(
        keep_awake(&focused, true),
        "Keep awake started unchecked\n{}",
        text(&focused)
    );
    app.key("space");
    app.wait_screen("keep awake off", |screen| {
        focused_line(screen).contains(KEEP) && keep_awake(screen, false)
    })
}

fn button(app: &App, label: &str, row_ok: impl Fn(&str) -> bool) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| row_ok(&row_text(&screen, u32::from(point.y))))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one dialog button {label:?}:\n{}",
        text(&screen)
    );
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

fn preview_button(app: &App, label: &str) -> Point {
    button(app, label, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains(DISCARD)
            && !row.contains(FOOTER_SAVE)
            && !row.contains(UNSAVED_TITLE)
    })
}

fn leave_button(app: &App, label: &str) -> Point {
    button(app, label, |row| {
        row.contains(CANCEL)
            && row.contains(DISCARD)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains(FOOTER_SAVE)
    })
}

fn footer_cancel(app: &App) -> Point {
    button(app, CANCEL, |row| {
        row.contains(CANCEL)
            && row.contains(FOOTER_SAVE)
            && !row.contains(DISCARD)
            && !row.contains(UNSAVED_TITLE)
            && !row.contains(PREVIEW_TITLE)
    })
}

fn unsaved_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(UNSAVED_TITLE)
        && visible.contains(UNSAVED_LEAD)
        && visible.contains(UNSAVED_MID)
        && visible.contains(UNSAVED_END)
        && visible.contains(DISCARD)
        && !visible.contains(SAVED)
        && !visible.contains(DISCARDED)
}

fn open_unsaved(app: &App) -> TerminalSnapshot {
    app.click(footer_cancel(app));
    app.wait_screen("unsaved dialog", unsaved_open)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the preview stays open.
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

fn shot(app: &App, checkpoint: &str) {
    nudge(app, preview_open);
    app.visual(&visual_id(checkpoint));
}

#[test]
fn j02_save_route() {
    let app = open("j02-save-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert!(text(&manager).contains(STALE), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let selected = select_payments(&app);
    assert!(
        focused_line(&selected).contains(PAYMENTS),
        "{}",
        text(&selected)
    );
    assert_siblings(&selected);
    let editor = open_editor(&app);
    assert!(editor_open(&editor), "{}", text(&editor));
    assert!(
        keep_awake(&editor, true),
        "route Keep awake\n{}",
        text(&editor)
    );
    app.click(footer_cancel(&app));
    let back = app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(NAME_HELP)
            && !text(screen).contains(UNSAVED_TITLE)
            && lists_workspace(screen, PAYMENTS)
    });
    assert_siblings(&back);
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains(DISCARDED), "{}", text(&back));
    app.finish();
}

#[test]
fn j02_save_save() {
    let app = open("j02-save-save");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert!(text(&manager).contains(STALE), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    assert_siblings(&manager);
    select_payments(&app);
    let editor = open_editor(&app);
    assert!(editor_open(&editor), "{}", text(&editor));

    toggle_keep_awake_off(&app);
    app.key("ctrl+s");
    let preview = app.wait_screen("preview", preview_open);
    assert!(
        focused_line(&preview).contains(CANCEL),
        "focused preview\n{}",
        focused_line(&preview)
    );
    app.key("escape");
    let editing = app.wait_screen("preview cancelled", |screen| {
        let visible = text(screen);
        visible.contains(NOT_SAVED)
            && editor_open(screen)
            && !visible.contains(PREVIEW_TITLE)
            && !visible.contains(SAVED)
            && !visible.contains("Save failed")
            && keep_awake(screen, false)
    });
    assert!(editor_open(&editing), "{}", text(&editing));

    let stayed = open_unsaved(&app);
    assert!(text(&stayed).contains(UNSAVED_TITLE), "{}", text(&stayed));
    app.click(leave_button(&app, CANCEL));
    let still = app.wait_screen("stay", |screen| {
        let visible = text(screen);
        editor_open(screen)
            && !visible.contains(UNSAVED_TITLE)
            && !visible.contains(DISCARDED)
            && !visible.contains(SAVED)
            && keep_awake(screen, false)
    });
    assert!(editor_open(&still), "{}", text(&still));

    open_unsaved(&app);
    app.click(leave_button(&app, DISCARD));
    let discarded = app.wait_screen("discarded", |screen| {
        let visible = text(screen);
        visible.contains(DISCARDED)
            && visible.contains(DIRECTORY)
            && !visible.contains(NAME_HELP)
            && lists_workspace(screen, PAYMENTS)
    });
    assert_siblings(&discarded);
    if !focused_line(&discarded).contains(PAYMENTS) {
        select_payments(&app);
    }
    let reopened = open_editor(&app);
    assert!(editor_open(&reopened), "{}", text(&reopened));
    assert!(
        keep_awake(&reopened, true),
        "discard restored Keep awake\n{}",
        text(&reopened)
    );

    toggle_keep_awake_off(&app);
    open_unsaved(&app);
    app.click(leave_button(&app, CONFIRM));
    let checkpoint = app.wait_screen("leave save preview", preview_open);
    assert!(
        focused_line(&checkpoint).contains(CANCEL),
        "focused checkpoint\n{}\n{}",
        focused_line(&checkpoint),
        text(&checkpoint)
    );
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && preview_open(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && preview_open(screen)
    });

    app.click(preview_button(&app, CONFIRM));
    let failed = app.wait_screen("save failed", |screen| {
        let visible = text(screen);
        visible.contains(FAILED)
            && visible.contains(EACCES)
            && visible.contains(INTACT)
            && visible.contains(NEXT)
            && !visible.contains(SAVED)
    });
    assert!(!text(&failed).contains(SAVED), "{}", text(&failed));
    app.key("escape");
    let kept = app.wait_screen("failure dismissed", |screen| {
        let visible = text(screen);
        editor_open(screen)
            && !visible.contains("Save failed")
            && !visible.contains(SAVED)
            && keep_awake(screen, false)
    });
    assert!(plain_name(&kept), "{}", text(&kept));

    app.key("ctrl+s");
    app.wait_screen("retry preview", preview_open);
    app.click(preview_button(&app, CONFIRM));
    let saved = app.wait_screen("saved", |screen| {
        let visible = text(screen);
        visible.contains(SAVED)
            && visible.contains(DIRECTORY)
            && !visible.contains(NAME_HELP)
            && lists_workspace(screen, PAYMENTS)
    });
    assert_siblings(&saved);
    if !focused_line(&saved).contains(PAYMENTS) {
        select_payments(&app);
    }
    let again = open_editor(&app);
    assert!(editor_open(&again), "{}", text(&again));
    assert!(
        keep_awake(&again, false),
        "saved Keep awake\n{}",
        text(&again)
    );
    app.finish();
}
