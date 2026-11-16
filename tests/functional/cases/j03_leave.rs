//! Jackin global Settings leave and a failing save, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j03_leave/`. The shots are the open Settings error dialog
//! after the first save fails, before Esc and before the retry. A same-size
//! resize does not force a frame, and the cursor column is part of the hash,
//! so each shot nudges the width and comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, button, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const INTRO: &str = "Stand up, operator";
const CRUMB: &str = "Settings › global › General";
const GENERAL_NOTE: &str = "Two independent flags";
const COAUTHOR: &str = "Add Co-authored-by trailer";
const DCO: &str = "Sign off commits (DCO)";
const ON: &str = "[✓]";
const OFF: &str = "[ ]";
const CHANGE: &str = "• 1 change";
const UNSAVED: &str = "Unsaved changes";
const LOST: &str = "Save changes before leaving? 1 change would be lost.";
const DISCARDED: &str = "Discarded 1 change";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Discard";
const SAVE: &str = "Save";
const PREVIEW: &str = "Save settings";
const SAVED: &str = "Settings saved";
const ERROR: &str = "Settings error";
const WRITE_FAILED: &str = "write failed: ~/.jackin/config.toml is not writable (EACCES)";
const INTACT: &str = "your edits are intact · nothing was written";
const FIX: &str = "fix the permission and Save again";
const WIDE: &str = "J03.leave.leave.120x40";
const NARROW: &str = "J03.leave.leave.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j03_leave/{}",
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
    // The 900 ms save delay is wall-clock time from `World::schedule`.
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
        && !visible.contains(ERROR)
}

fn dirty_general(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let coauthor = option_line(screen, COAUTHOR);
    // Hard-cases chrome (`! instance index unreadable`, `▲ daemon stale`, and
    // `• 1 change`) crowds the header, so `crumb()` is cut to `…`.
    visible.contains(GENERAL_NOTE)
        && visible.contains(CHANGE)
        && unchecked(screen, COAUTHOR)
        && checked(screen, DCO)
        && coauthor.contains('•')
        && !visible.contains(DIRECTORY)
        && !visible.contains(SAVED)
        && !visible.contains(UNSAVED)
        && !visible.contains(PREVIEW)
        && !visible.contains(ERROR)
}

fn saved_general(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB)
        && unchecked(screen, COAUTHOR)
        && checked(screen, DCO)
        && !visible.contains(CHANGE)
        && !visible.contains(UNSAVED)
        && !visible.contains(DIRECTORY)
        && !visible.contains(ERROR)
        && !option_line(screen, COAUTHOR).contains('•')
}

fn manager(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DIRECTORY) && !visible.contains(CRUMB) && !visible.contains(UNSAVED)
}

fn error_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // The prop value is cut in the dialog row. The focused row's reveal paints
    // the full "write failed: … (EACCES)" line in the footer.
    visible.contains(ERROR)
        && visible.contains(WRITE_FAILED)
        && visible.contains(INTACT)
        && visible.contains(FIX)
        && !visible.contains(SAVED)
        && !visible.contains(DIRECTORY)
        && !visible.contains(UNSAVED)
}

fn error_title(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(ERROR) && !visible.contains(SAVED) && !visible.contains(DIRECTORY)
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

fn leave_button(app: &App, label: &str) -> Point {
    button(app, label, |row| {
        row.contains(CANCEL)
            && row.contains(DISCARD)
            && row.contains(SAVE)
            && !row.contains("Save…")
    })
}

fn preview_button(app: &App, label: &str) -> Point {
    button(app, label, |row| {
        row.contains(CANCEL)
            && row.contains(SAVE)
            && row.contains('│')
            && !row.contains("Save…")
            && !row.contains(DISCARD)
            && !row.contains(UNSAVED)
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

fn status_exact(screen: &TerminalSnapshot, status: &str) {
    let status_line = text(screen)
        .lines()
        .find(|line| line.contains(status))
        .unwrap_or("")
        .to_string();
    let at = status_line
        .find(status)
        .unwrap_or_else(|| panic!("status is not exactly {status}\n{status_line}"));
    let after = &status_line[at + status.len()..];
    assert!(
        !after.starts_with(|c: char| c.is_ascii_alphanumeric()),
        "status is not exactly {status}\n{status_line}"
    );
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

fn focus_tabs(app: &App) {
    for _ in 0..4 {
        let screen = app.screen();
        assert!(
            !text(&screen).contains(UNSAVED) && !text(&screen).contains(ERROR),
            "refusing Esc while a dialog is open\n{}",
            text(&screen)
        );
        let line = focused_line(&screen);
        if line.contains("General") && !line.contains(COAUTHOR) && !line.contains(DCO) {
            return;
        }
        app.key("escape");
        app.wait_screen("focus moved", |screen| {
            focused_line(screen) != line || text(screen).contains(UNSAVED)
        });
    }
    let screen = app.screen();
    panic!("tab strip\n{}\n{}", focused_line(&screen), text(&screen));
}

fn leave_dialog(app: &App) -> TerminalSnapshot {
    focus_tabs(app);
    app.key("escape");
    // The title paints before the wrapped body. The sentence is split
    // across rows, so the check is the dialog interior, not one line.
    let dialog = app.wait_screen("unsaved changes", |screen| {
        text(screen).contains(UNSAVED)
            && dialog_body(screen).contains(LOST)
            && text(screen).contains(DISCARD)
            && focused_line(screen).contains(CANCEL)
    });
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
    assert!(text(&dialog).contains(SAVE), "{}", text(&dialog));
    assert!(!text(&dialog).contains(SAVED), "{}", text(&dialog));
    dialog
}

#[test]
fn j03_leave_route() {
    let app = open("j03-leave-route");
    let opened = reach_manager(&app);
    assert!(text(&opened).contains(DIRECTORY), "{}", text(&opened));
    assert!(!text(&opened).contains(UNSAVED), "{}", text(&opened));
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    let settings = open_settings(&app);
    assert!(clean_general(&settings), "{}", text(&settings));
    assert_eq!((settings.cols(), settings.rows()), (120, 40));
    let tabs = focused_line(&settings);
    assert!(
        tabs.contains("General") && !tabs.contains(COAUTHOR),
        "Esc is not from the tab strip\n{tabs}"
    );
    app.key("escape");
    let back = app.wait_screen("clean leave", manager);
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains(CHANGE), "{}", text(&back));
    assert!(!text(&back).contains(ERROR), "{}", text(&back));
    app.finish();
}

#[test]
fn j03_leave_leave() {
    let app = open("j03-leave-leave");
    let manager_screen = reach_manager(&app);
    assert!(
        text(&manager_screen).contains(DIRECTORY),
        "{}",
        text(&manager_screen)
    );
    assert_eq!((manager_screen.cols(), manager_screen.rows()), (120, 40));

    open_settings(&app);
    app.key("escape");
    let clean = app.wait_screen("clean leave", manager);
    assert!(!text(&clean).contains(UNSAVED), "{}", text(&clean));
    assert!(!text(&clean).contains(SAVED), "{}", text(&clean));

    open_settings(&app);
    let dirty = clear_coauthor(&app);
    assert_eq!((dirty.cols(), dirty.rows()), (120, 40));
    assert!(dirty_general(&dirty), "dirty General\n{}", text(&dirty));

    leave_dialog(&app);
    app.click(leave_button(&app, CANCEL));
    let stayed = app.wait_screen("stayed", dirty_general);
    assert!(
        unchecked(&stayed, COAUTHOR) && !text(&stayed).contains(SAVED),
        "Cancel left the page\n{}",
        text(&stayed)
    );

    leave_dialog(&app);
    app.click(leave_button(&app, DISCARD));
    let discarded = app.wait_screen("discarded", |screen| {
        text(screen).contains(DISCARDED) && manager(screen)
    });
    status_exact(&discarded, DISCARDED);
    assert!(!text(&discarded).contains(SAVED), "{}", text(&discarded));
    assert!(!text(&discarded).contains(CHANGE), "{}", text(&discarded));

    app.key("s");
    let restored = app.wait_screen("restored general", clean_general);
    assert!(
        checked(&restored, COAUTHOR) && !text(&restored).contains(CHANGE),
        "discard wrote the co-author flag\n{}",
        text(&restored)
    );
    app.key("escape");
    app.wait_screen("manager after discard", manager);

    open_settings(&app);
    clear_coauthor(&app);
    app.key("ctrl+s");
    let preview = app.wait_screen("save settings", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
    });
    assert!(!text(&preview).contains(ERROR), "{}", text(&preview));
    app.click(preview_button(&app, CANCEL));
    let aborted = app.wait_screen("preview cancelled", dirty_general);
    assert!(
        !text(&aborted).contains(SAVED) && !text(&aborted).contains(ERROR),
        "preview cancel saved\n{}",
        text(&aborted)
    );

    app.key("ctrl+s");
    app.wait_screen("save settings again", |screen| {
        text(screen).contains(PREVIEW) && focused_line(screen).contains(CANCEL)
    });
    app.click(preview_button(&app, SAVE));
    let failed = app.wait_screen("settings error", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && error_dialog(screen)
    });
    assert!(error_dialog(&failed), "failing save\n{}", text(&failed));
    shoot(&app, WIDE, error_dialog);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && error_title(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shoot(&app, NARROW, error_title);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && error_dialog(screen)
    });

    app.key("escape");
    let still = app.wait_screen("error dismissed", dirty_general);
    assert!(
        unchecked(&still, COAUTHOR) && text(&still).contains(CHANGE),
        "error dismiss lost the edit\n{}",
        text(&still)
    );

    app.key("ctrl+s");
    app.wait_screen("retry preview", |screen| {
        text(screen).contains(PREVIEW) && focused_line(screen).contains(CANCEL)
    });
    app.click(preview_button(&app, SAVE));
    let saved = app.wait_screen("settings saved", |screen| {
        text(screen).contains(SAVED) && manager(screen)
    });
    status_exact(&saved, SAVED);
    assert!(!text(&saved).contains(ERROR), "{}", text(&saved));
    assert!(!text(&saved).contains(CRUMB), "{}", text(&saved));

    app.key("s");
    let kept = app.wait_screen("saved general", saved_general);
    assert!(
        unchecked(&kept, COAUTHOR) && checked(&kept, DCO) && !text(&kept).contains(CHANGE),
        "retry did not keep the co-author flag\n{}",
        text(&kept)
    );
    app.finish();
}
