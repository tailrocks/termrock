//! Jackin global Settings General, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j03_general/`. The shots are the dirty General page after the
//! co-author checkbox is cleared, before Esc and before Save. A same-size
//! resize does not force a frame, and the cursor column is part of the hash,
//! so each shot nudges the width and comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const INTRO: &str = "Stand up, operator";
const CRUMB: &str = "Settings › global › General";
const COAUTHOR: &str = "Add Co-authored-by trailer";
const DCO: &str = "Sign off commits (DCO)";
const ON: &str = "[✓]";
const OFF: &str = "[ ]";
const CHANGE: &str = "• 1 change";
const UNSAVED: &str = "Unsaved changes";
const LOST: &str = "1 change would be lost";
const DISCARDED: &str = "Discarded 1 change";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Discard";
const SAVE: &str = "Save";
const SAVED: &str = "Settings saved";
const WIDE: &str = "J03.general.edit.120x40";
const NARROW: &str = "J03.general.edit.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j03_general/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "returning".to_string()];
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
        visible.contains(DIRECTORY) || visible.contains(INTRO)
    });
    if text(&screen).contains(INTRO) {
        app.key("space");
        return app.wait_text(DIRECTORY);
    }
    screen
}

fn dialog_body(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .filter_map(|line| {
            let start = line.find('│')?;
            let end = line.rfind('│')?;
            if end <= start {
                return None;
            }
            let inner = line[start + '│'.len_utf8()..end].trim();
            (!inner.is_empty()).then(|| inner.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn option_line(screen: &TerminalSnapshot, label: &str) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(label))
        .unwrap_or("")
        .to_string()
}

fn checked(screen: &TerminalSnapshot, label: &str) -> bool {
    let line = option_line(screen, label);
    line.contains(ON) && !line.contains(OFF)
}

fn unchecked(screen: &TerminalSnapshot, label: &str) -> bool {
    let line = option_line(screen, label);
    line.contains(OFF) && !line.contains(ON)
}

fn clean_general(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB)
        && checked(screen, COAUTHOR)
        && checked(screen, DCO)
        && !visible.contains(CHANGE)
        && !visible.contains(UNSAVED)
        && !visible.contains(DIRECTORY)
        && !visible.contains(SAVED)
}

fn dirty_general(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let coauthor = option_line(screen, COAUTHOR);
    visible.contains(CRUMB)
        && visible.contains(CHANGE)
        && unchecked(screen, COAUTHOR)
        && checked(screen, DCO)
        && coauthor.contains('•')
        && !visible.contains(DIRECTORY)
        && !visible.contains(SAVED)
        && !visible.contains(UNSAVED)
        && !visible.contains("Save settings")
}

fn open_settings(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(DIRECTORY),
        "refusing s unless the manager is showing\n{}",
        text(&before)
    );
    app.key("s");
    app.wait_screen("settings general", clean_general)
}

fn button_on(app: &App, label: &str, row_has: &[&str]) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(&screen, u32::from(point.y));
            row_has.iter().all(|piece| row.contains(piece)) && !row.contains("Save…")
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one button {label:?} on {row_has:?}:\n{}",
        text(&screen)
    );
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while General stays dirty.
fn nudge(app: &App) {
    app.nudge_width(dirty_general, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app);
    app.visual(&visual_id(checkpoint));
}

fn focus_coauthor(app: &App) {
    app.key("enter");
    let focused = app.wait_screen("coauthor checkbox", |screen| {
        focused_line(screen).contains(COAUTHOR)
    });
    assert!(
        checked(&focused, COAUTHOR),
        "co-author did not start checked\n{}",
        text(&focused)
    );
}

fn clear_coauthor(app: &App) -> TerminalSnapshot {
    focus_coauthor(app);
    app.key("space");
    app.wait_screen("dirty general", dirty_general)
}

fn back_to_tabs(app: &App) {
    let line = focused_line(&app.screen());
    if line.contains(COAUTHOR) || line.contains(DCO) {
        app.key("escape");
    }
    let tabs = app.wait_screen("tab strip", |screen| {
        let line = focused_line(screen);
        line.contains("General") && !line.contains(COAUTHOR)
    });
    assert!(
        dirty_general(&tabs),
        "left the dirty General page\n{}",
        text(&tabs)
    );
}

#[test]
fn j03_general_route() {
    let app = open("j03-general-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let settings = open_settings(&app);
    assert!(clean_general(&settings), "{}", text(&settings));
    assert_eq!((settings.cols(), settings.rows()), (120, 40));
    app.key("escape");
    let back = app.wait_screen("clean leave", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) && !visible.contains(CRUMB) && !visible.contains(UNSAVED)
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains(CHANGE), "{}", text(&back));
    app.finish();
}

#[test]
fn j03_general_edit() {
    let app = open("j03-general-edit");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    open_settings(&app);
    let dirty = clear_coauthor(&app);
    assert_eq!((dirty.cols(), dirty.rows()), (120, 40));
    assert!(
        dirty_general(&dirty),
        "dirty General page\n{}",
        text(&dirty)
    );
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && dirty_general(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dirty_general(screen)
    });

    back_to_tabs(&app);
    app.key("escape");
    let dialog = app.wait_screen("unsaved changes", |screen| text(screen).contains(UNSAVED));
    assert!(
        dialog_body(&dialog).contains(LOST),
        "dialog body\n{}",
        text(&dialog)
    );
    let line = focused_line(&dialog);
    assert!(
        line.contains(CANCEL),
        "initial focus is Cancel\n{line}\n{}",
        text(&dialog)
    );
    assert!(text(&dialog).contains(DISCARD), "{}", text(&dialog));
    assert!(!text(&dialog).contains(SAVED), "{}", text(&dialog));
    app.click(button_on(&app, DISCARD, &[CANCEL, DISCARD, SAVE]));
    let discarded = app.wait_screen("discarded", |screen| {
        let visible = text(screen);
        visible.contains(DISCARDED)
            && visible.contains(DIRECTORY)
            && !visible.contains(UNSAVED)
            && !visible.contains(CRUMB)
    });
    let status_line = text(&discarded)
        .lines()
        .find(|line| line.contains("Discarded"))
        .unwrap_or("")
        .to_string();
    let at = status_line
        .find(DISCARDED)
        .unwrap_or_else(|| panic!("status is not exactly {DISCARDED}\n{status_line}"));
    let after = &status_line[at + DISCARDED.len()..];
    assert!(
        !after.starts_with(|c: char| c.is_ascii_alphanumeric()),
        "status is not exactly {DISCARDED}\n{status_line}"
    );
    assert!(!text(&discarded).contains(SAVED), "{}", text(&discarded));
    assert!(!text(&discarded).contains(CHANGE), "{}", text(&discarded));

    app.key("s");
    let again = app.wait_screen("settings again", clean_general);
    assert!(
        checked(&again, COAUTHOR) && checked(&again, DCO) && !text(&again).contains(CHANGE),
        "discard wrote the co-author flag\n{}",
        text(&again)
    );
    app.key("escape");
    let back = app.wait_screen("manager again", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) && !visible.contains(CRUMB) && !visible.contains(UNSAVED)
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    app.finish();
}
