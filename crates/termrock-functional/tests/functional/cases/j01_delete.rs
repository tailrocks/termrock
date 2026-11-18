//! Jackin workspace deletion, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j01_delete/`. The shots are the open payments-platform
//! delete dialog, before Cancel and before Delete. A same-size resize does
//! not force a frame, and the cursor column is part of the hash, so each
//! shot nudges the width and comes back.
//!
//! `delete` drops the workspace and sets `workspace` to `None` on its
//! instances. `build_rows` paints only `instances_of`, so those rows leave
//! the tree. `running_count` still counts a Running instance after that.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const RELEASE: &str = "release-automation";
const CUSTOMER: &str = "customer-portal";
const PIPELINE: &str = "data-pipeline";
const DOCS: &str = "docs-site";
const AUTH: &str = "auth-service";
const GATEWAY: &str = "gateway";
const PAYMENTS_TITLE: &str = "Delete workspace payments-platform?";
const AUTH_TITLE: &str = "Delete workspace auth-service?";
const REMOVED: &str = "The saved configuration is removed.";
const LINKED_COUNT: &str = "2 instance";
const LINKED_REST: &str = "records and files on disk are kept.";
const EMPTY_HEAD: &str = "The saved configuration is removed. Instances";
const EMPTY_REST: &str = "and files on disk are kept.";
const CANCELLED: &str = "Cancelled · nothing changed";
const DELETED_PAYMENTS: &str = "Deleted workspace payments-platform · instances and files kept";
const DELETED_AUTH: &str = "Deleted workspace auth-service · instances and files kept";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Delete";
const WIDE: &str = "J01.delete.delete.120x40";
const NARROW: &str = "J01.delete.delete.narrow";
const NOISE: &[&str] = &["77aa", "88bb", "parity", "vm.img", "842 of 926"];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j01_delete/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
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

fn quiet(screen: &TerminalSnapshot) -> bool {
    NOISE.iter().all(|item| !text(screen).contains(item))
}

fn payments_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(PAYMENTS_TITLE)
        && visible.contains(REMOVED)
        && visible.contains(LINKED_COUNT)
        && visible.contains(LINKED_REST)
        && visible.contains(CANCEL)
        && visible.contains(CONFIRM)
        && line.contains("▎Cancel")
        && !visible.contains("0 instance")
        && quiet(screen)
}

fn auth_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(AUTH_TITLE)
        && visible.contains(EMPTY_HEAD)
        && visible.contains(EMPTY_REST)
        && !visible.contains("0 instance")
        && !visible.contains("instance record")
        && !visible.contains(LINKED_COUNT)
        && visible.contains(CANCEL)
        && visible.contains(CONFIRM)
        && line.contains("▎Cancel")
        && quiet(screen)
}

/// A workspace row or its detail title. A path (`~/src/<name>`) does not count,
/// and neither does the delete status or the open dialog title.
fn lists_workspace(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| {
        if line.contains("Deleted workspace") || line.contains("Delete workspace") {
            return false;
        }
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            !matches!(before, Some('/'))
        })
    })
}

fn painted_running(screen: &TerminalSnapshot) -> u32 {
    let mut found = Vec::new();
    for line in text(screen).lines() {
        if !line.contains("Workspaces") {
            continue;
        }
        let Some(at) = line.find(" running") else {
            continue;
        };
        let digits: String = line[..at]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if digits.is_empty() {
            continue;
        }
        found.push(digits.parse::<u32>().expect("running count"));
    }
    assert!(
        !found.is_empty(),
        "no painted running count\n{}",
        text(screen)
    );
    assert!(
        found.iter().all(|count| *count == found[0]),
        "running counts disagree: {found:?}\n{}",
        text(screen)
    );
    found[0]
}

fn dialog_button(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(&screen, u32::from(point.y));
            row.contains(CANCEL) && row.contains(CONFIRM) && row.contains('│')
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one delete-dialog button {label:?}:\n{}",
        text(&screen)
    );
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

fn focus_named(app: &App, name: &str) -> TerminalSnapshot {
    for _ in 0..30 {
        let screen = app.screen();
        let line = focused_line(&screen);
        if line.contains(name) {
            return screen;
        }
        app.key("down");
        app.wait_screen(name, |screen| focused_line(screen) != line);
    }
    let screen = app.screen();
    panic!("never focused {name}\n{}", text(&screen));
}

fn open_payments(app: &App) -> TerminalSnapshot {
    app.key("d");
    app.wait_screen("payments delete dialog", payments_dialog)
}

fn open_auth(app: &App) -> TerminalSnapshot {
    app.key("d");
    app.wait_screen("auth delete dialog", auth_dialog)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the dialog stays open.
fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet_frame, Some("nudge left the frame"));
}

fn shot(app: &App, checkpoint: &str) {
    nudge(app, payments_dialog);
    app.visual(&visual_id(checkpoint));
}

fn wide_dialog(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120 && screen.rows() == 40 && payments_dialog(screen)
}

fn baseline_hit(app: &App, checkpoint: &str) -> bool {
    let root = crate::support::oracle_baselines();
    app.check_visual(&root, &visual_id(checkpoint)).is_pass()
}

/// The reference capture shows `daemon · 4 s ago` beside the open dialog.
/// A compare started on that second is still running when the next sim
/// second replaces it, and the 700 ms window then holds the replacement.
/// `3 s ago` is the second before the capture.
fn shoot_wide(app: &App) {
    nudge(app, payments_dialog);
    let previous = "daemon · 3 s ago";
    for _ in 0..4 {
        app.wait_screen("before the previous age", |screen| {
            wide_dialog(screen) && !text(screen).contains(previous)
        });
        app.wait_screen("previous age", |screen| {
            wide_dialog(screen) && text(screen).contains(previous)
        });
        std::thread::sleep(Duration::from_millis(250));
        if wide_dialog(&app.screen()) && baseline_hit(app, WIDE) {
            return;
        }
    }
    app.visual(&visual_id(WIDE));
}

fn assert_siblings(screen: &TerminalSnapshot) {
    assert!(lists_workspace(screen, INFRA), "{}", text(screen));
    assert!(lists_workspace(screen, RELEASE), "{}", text(screen));
    assert!(lists_workspace(screen, CUSTOMER), "{}", text(screen));
}

#[test]
fn j01_delete_route() {
    let app = open("j01-delete-route", "returning");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    assert!(quiet(&manager), "{}", text(&manager));
    let running = painted_running(&manager);
    assert_eq!(running, 2, "manager paints two running instances");
    let selected = focus_named(&app, PAYMENTS);
    assert!(
        focused_line(&selected).contains(PAYMENTS),
        "{}",
        text(&selected)
    );
    assert!(text(&selected).contains("7f3a"), "{}", text(&selected));
    assert!(text(&selected).contains("c41e"), "{}", text(&selected));
    assert!(!text(&selected).contains("77aa"), "{}", text(&selected));
    let dialog = open_payments(&app);
    assert!(payments_dialog(&dialog), "{}", text(&dialog));
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    app.click(dialog_button(&app, CANCEL));
    let kept = app.wait_screen("cancel leaves the workspace", |screen| {
        text(screen).contains(CANCELLED)
            && !text(screen).contains(PAYMENTS_TITLE)
            && lists_workspace(screen, PAYMENTS)
            && focused_line(screen).contains(PAYMENTS)
    });
    assert_eq!(painted_running(&kept), running, "{}", text(&kept));
    assert!(text(&kept).contains("7f3a"), "{}", text(&kept));
    assert!(text(&kept).contains("c41e"), "{}", text(&kept));
    assert_siblings(&kept);
    assert!(quiet(&kept), "{}", text(&kept));
    app.finish();
}

#[test]
fn j01_delete_delete() {
    let app = open("j01-delete-delete", "returning");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let running = painted_running(&manager);
    assert_eq!(running, 2, "manager paints two running instances");
    focus_named(&app, PAYMENTS);
    let dialog = open_payments(&app);
    assert!(payments_dialog(&dialog), "{}", text(&dialog));
    shoot_wide(&app);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && payments_dialog(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && payments_dialog(screen)
    });

    app.click(dialog_button(&app, CONFIRM));
    let deleted = app.wait_screen("payments deleted", |screen| {
        text(screen).contains(DELETED_PAYMENTS)
            && !text(screen).contains(PAYMENTS_TITLE)
            && focused_line(screen).contains(DIRECTORY)
    });
    assert!(!lists_workspace(&deleted, PAYMENTS), "{}", text(&deleted));
    assert!(!text(&deleted).contains("7f3a"), "{}", text(&deleted));
    assert!(!text(&deleted).contains("c41e"), "{}", text(&deleted));
    assert_eq!(
        painted_running(&deleted),
        running,
        "running count changed after unlink\n{}",
        text(&deleted)
    );
    assert_siblings(&deleted);
    assert!(quiet(&deleted), "{}", text(&deleted));
    app.finish();

    let app = open("j01-delete-empty", "hard-cases");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    assert!(lists_workspace(&manager, PAYMENTS), "{}", text(&manager));
    assert!(lists_workspace(&manager, PIPELINE), "{}", text(&manager));
    assert!(lists_workspace(&manager, DOCS), "{}", text(&manager));
    let selected = focus_named(&app, AUTH);
    let line = focused_line(&selected);
    assert!(line.contains(AUTH), "{line}\n{}", text(&selected));
    assert!(
        !line.contains("running") && !line.contains("record"),
        "auth-service row is not the empty workspace\n{line}"
    );
    let dialog = open_auth(&app);
    assert!(auth_dialog(&dialog), "{}", text(&dialog));
    app.click(dialog_button(&app, CONFIRM));
    // Delete sets the status line, then returns focus to Current directory.
    // Hard-cases replaces that line after five seconds with the refresh
    // failure, so the workspace leaving the tree is the result that stays.
    let deleted = app.wait_screen("auth deleted", |screen| {
        let visible = text(screen);
        if visible.contains(AUTH_TITLE) || visible.contains(CANCELLED) {
            return false;
        }
        let gone = !lists_workspace(screen, AUTH);
        (visible.contains(DELETED_AUTH) && gone)
            || (gone
                && focused_line(screen).contains(DIRECTORY)
                && visible.contains("Refresh failed"))
    });
    assert!(lists_workspace(&deleted, PAYMENTS), "{}", text(&deleted));
    assert!(lists_workspace(&deleted, PIPELINE), "{}", text(&deleted));
    assert!(lists_workspace(&deleted, DOCS), "{}", text(&deleted));
    assert!(lists_workspace(&deleted, GATEWAY), "{}", text(&deleted));
    assert!(
        focused_line(&deleted).contains(DIRECTORY),
        "{}",
        text(&deleted)
    );
    assert!(!text(&deleted).contains("0 instance"), "{}", text(&deleted));
    assert!(quiet(&deleted), "{}", text(&deleted));
    app.finish();
}
