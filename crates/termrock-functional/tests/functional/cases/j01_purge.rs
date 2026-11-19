//! Jackin instance purge, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j01_purge/`. The shots are the open purge dialog after a
//! wrong token has painted `Does not match`, before the exact token is
//! submitted. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const WIDE: &str = "J01.purge.purge.120x40";
const NARROW: &str = "J01.purge.purge.narrow";
const TITLE: &str = "Purge instance 7f3a?";
const MISMATCH: &str = "Does not match";
const PURGED: &str = "Purged 7f3a · container, sidecar, volume and recovery state removed";
const PURGING: &str = "Purging…";
const PARTIAL: &str = "7f3";
const WRONG: &str = "9b02";
const EXACT: &str = "7f3a";
// The 120-column tree clips the role before "architect", so the visible
// stem is the row, not the full label.
const ROW_7F3A: &str = "7f3a  the-archite";
const ROW_9B02: &str = "9b02  sre";
const ACTION: &str = "purge the instance record and its container";
// Dialog width 66 clips the fact values. These stems are the painted text.
const TARGET: &str = "7f3a · payments-platform › the-architect · Clau";
const SCOPE: &str = "1 container · 2 preserved sessions · worktree w";
const RISK: &str = "uncommitted changes in the worktree are lost";
const TWO: &str = "2 running";
const ONE: &str = "1 running";
/// Seconds of simulated clock a step may take before the test gives up.
const STEP_LIMIT: usize = 10;
/// Seconds after a predecessor second that a shot tries against its baseline.
const SECONDS_TO_SEARCH: usize = 10;

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j01_purge/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

/// `clock` moves the simulated clock only through `advance_clock`, so the age
/// labels and the purge job wait on the case instead of on wall time.
fn open(name: &str, clock: bool) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "returning".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.clock = clock;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn words(value: &str, token: &str) -> usize {
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| *part == token)
        .count()
}

fn token_count(screen: &TerminalSnapshot, token: &str) -> usize {
    words(&text(screen), token)
}

fn intro_up(visible: &str) -> bool {
    visible.contains("Stand up, operator") || visible.contains("Knock, knock, operator.")
}

fn dismiss_intro(app: &App) {
    let screen = app.wait_screen("intro or manager", |screen| {
        let visible = text(screen);
        intro_up(&visible) || visible.contains("Current directory")
    });
    if !intro_up(&text(&screen)) {
        return;
    }
    // The intro ignores the first keys while its guard is counting down.
    // Space is the requested dismiss. Enter is what actually skips reduced motion.
    std::thread::sleep(Duration::from_millis(200));
    app.key("space");
    if intro_up(&text(&app.screen())) {
        app.key("enter");
    }
    app.wait_text("Current directory");
}

fn focus_has(app: &App, needle: &str) {
    app.wait_screen(&format!("focus {needle}"), |screen| {
        focused_line(screen).contains(needle)
    });
}

fn move_focus(app: &App, key: &str, needle: &str) {
    app.key(key);
    focus_has(app, needle);
}

fn reach_7f3a(app: &App) {
    app.wait_text(TWO);
    move_focus(app, "down", "payments-platform");
    app.key("right");
    app.wait_text(EXACT);
    move_focus(app, "down", EXACT);
    move_focus(app, "down", "c41e");
    move_focus(app, "down", "infra-control-plane");
    app.key("right");
    app.wait_text(WRONG);
    move_focus(app, "down", WRONG);
    move_focus(app, "up", "infra-control-plane");
    move_focus(app, "up", "c41e");
    move_focus(app, "up", EXACT);
    let listed = app.screen();
    see(&listed, &[ROW_7F3A, ROW_9B02, TWO], &[TITLE, PURGED]);
    assert!(
        focused_line(&listed).contains(EXACT),
        "focus left 7f3a\n{}",
        text(&listed)
    );
}

fn open_purge(app: &App) -> TerminalSnapshot {
    app.key("p");
    // The title paints before the dialog draws Cancel.
    let dialog = app.wait_screen("purge dialog", |screen| {
        let visible = text(screen);
        visible.contains(TITLE) && visible.contains("Cancel") && visible.contains(RISK)
    });
    see(
        &dialog,
        &[
            TITLE,
            ACTION,
            TARGET,
            SCOPE,
            RISK,
            "Reversible",
            "Purge",
            "Cancel",
        ],
        &[PURGED, MISMATCH],
    );
    dialog
}

fn submit_token(app: &App, token: &str) {
    // macOS fields use the Emacs map, where select-all is ctrl+l.
    // Count the field row only. An exact token arms the dialog and the footer
    // drops its copy of that token, so a whole-screen count does not rise.
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text(token);
    app.wait_screen(&format!("typed {token}"), |screen| {
        words(&focused_line(screen), token) == 1
    });
    app.key("enter");
}

fn rejected(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(MISMATCH)
        && !visible.contains(PURGED)
        && !visible.contains(PURGING)
}

fn mismatch_open(screen: &TerminalSnapshot) -> bool {
    rejected(screen) && token_count(screen, WRONG) > 0 && token_count(screen, PARTIAL) == 0
}

fn nudge(app: &App, quiet: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet, Some("nudge left the frame"));
}

fn wide_open(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120 && screen.rows() == 40 && mismatch_open(screen)
}

fn narrow_open(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 80 && screen.rows() == 24 && mismatch_open(screen)
}

fn baseline_hit(app: &App, checkpoint: &str) -> bool {
    let root = crate::support::oracle_baselines();
    let outcome = app.check_visual(&root, &visual_id(checkpoint));
    app.accepts(&outcome)
}

/// Steps the stepped test clock one second at a time until `accept` holds on a
/// frame the step produced. Nothing reads wall time: the age labels and the
/// purge job move only when the clock steps.
fn step_until(
    app: &App,
    reason: &str,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    for _ in 0..STEP_LIMIT {
        app.fence();
        let screen = app.screen();
        if accept(&screen) {
            return screen;
        }
        app.advance_clock(1_000);
    }
    app.fence();
    let screen = app.screen();
    assert!(
        accept(&screen),
        "{reason} not seen in {STEP_LIMIT} seconds\n{}",
        text(&screen)
    );
    screen
}

/// Detail ages and the narrow uptime tail change every sim second. The stepped
/// clock reaches the second `predecessor` shows, then steps through the seconds
/// after it, one at a time, until the frame is the baseline. The baseline pins
/// one of them: wide pins the second after, and narrow's tail does not show the
/// second the predecessor names. Every second is a step on the stepped clock,
/// so the search never depends on wall time.
fn shoot_after(
    app: &App,
    checkpoint: &str,
    ready: impl Fn(&TerminalSnapshot) -> bool + Copy,
    predecessor: &[&str],
) {
    step_until(app, checkpoint, |screen| {
        let visible = text(screen);
        ready(screen) && predecessor.iter().all(|item| visible.contains(item))
    });
    for _ in 0..SECONDS_TO_SEARCH {
        if baseline_hit(app, checkpoint) {
            return;
        }
        app.advance_clock(1_000);
        app.fence();
    }
    app.visual(&visual_id(checkpoint));
}

fn shots(app: &App) {
    app.wait_screen("mismatch open", mismatch_open);
    nudge(app, mismatch_open);
    app.wait_screen("wide", wide_open);
    shoot_after(
        app,
        WIDE,
        wide_open,
        &["last seen 1 s ago", "daemon · 1 s ago"],
    );
    app.resize(80, 24);
    app.wait_screen("narrow", narrow_open);
    nudge(app, mismatch_open);
    // At 80 the tens digit sits under the dialog border, so second 1 and
    // second 11 paint the same tail.
    shoot_after(app, NARROW, narrow_open, &["│1 s  │"]);
    app.resize(120, 40);
    app.wait_screen("wide restored", wide_open);
}

#[test]
fn j01_purge_route() {
    let app = open("j01-purge-route", false);
    dismiss_intro(&app);
    reach_7f3a(&app);
    open_purge(&app);
    submit_token(&app, WRONG);
    let rejected_screen = app.wait_screen("wrong token rejected", mismatch_open);
    see(
        &rejected_screen,
        &[TITLE, MISMATCH, WRONG],
        &[PURGED, PURGING],
    );
    app.key("esc");
    let manager = app.wait_screen("cancel returns to the manager", |screen| {
        let visible = text(screen);
        visible.contains(ROW_7F3A)
            && visible.contains(ROW_9B02)
            && visible.contains(TWO)
            && visible.contains("Cancelled · nothing changed")
            && !visible.contains(TITLE)
            && !visible.contains(PURGED)
    });
    see(
        &manager,
        &[ROW_7F3A, ROW_9B02, TWO],
        &[TITLE, PURGED, MISMATCH],
    );
    app.finish();
}

#[test]
fn j01_purge_purge() {
    let app = open("j01-purge-purge", true);
    app.leave_intro("manager after intro", |screen| {
        text(screen).contains("Current directory")
    });
    reach_7f3a(&app);
    open_purge(&app);
    submit_token(&app, PARTIAL);
    let partial = app.wait_screen("partial token rejected", |screen| {
        rejected(screen) && token_count(screen, PARTIAL) > 0
    });
    see(&partial, &[TITLE, MISMATCH], &[PURGED, PURGING]);
    assert!(token_count(&partial, PARTIAL) > 0, "{}", text(&partial));
    submit_token(&app, WRONG);
    let wrong = app.wait_screen("wrong token rejected", mismatch_open);
    see(&wrong, &[TITLE, MISMATCH], &[PURGED, PURGING]);
    shots(&app);
    submit_token(&app, EXACT);
    app.wait_text(PURGING);
    // The purge job takes 2200 ms of the simulated clock, so it lands on a step.
    let purged = step_until(&app, "purged", |screen| text(screen).contains(PURGED));
    see(
        &purged,
        &[PURGED, ROW_9B02, ONE],
        &[TITLE, MISMATCH, ROW_7F3A, TWO, PURGING],
    );
    app.finish();
}
