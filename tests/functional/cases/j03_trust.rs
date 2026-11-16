//! Jackin global settings trust, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j03_trust/`. The shots are the dirty Trust page after
//! `github.com/chainargos/roles` is untrusted, once the timed status has
//! cleared, before Esc and before Save. A same-size resize does not force a
//! frame, and the cursor column is part of the hash, so each shot nudges the
//! width and comes back.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const INTRO: &str = "Stand up, operator";
const CRUMB_GENERAL: &str = "Settings › global › General";
const CRUMB_TRUST: &str = "Settings › global › Trust";
const TITLE: &str = "Role sources";
const SOURCE: &str = "github.com/chainargos/roles";
const ACME: &str = "github.com/acme-labs/roles-experimental";
const LOCAL: &str = "~/roles";
const CORP: &str = "git@corp:infra/roles";
const TRUSTED: &str = "[✓] trusted";
const UNTRUSTED: &str = "[ ] untrusted";
const STATUS: &str = "github.com/chainargos/roles · untrusted · save to apply";
const CHANGE: &str = "• 1 change";
const FACT: &str = "github.com/chainargos/roles → untrusted";
const UNSAVED: &str = "Unsaved changes";
const LOST: &str = "1 change would be lost";
const DISCARDED: &str = "Discarded 1 change";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Discard";
const SAVE: &str = "Save";
const SAVED: &str = "Settings saved";
const PREVIEW: &str = "Save settings";
const CLEANUP: &str = "Clean up & save…";
const ANYWAY: &str = "Save anyway";
const WIDE: &str = "J03.trust.trust.120x40";
const NARROW: &str = "J03.trust.trust.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j03_trust/{}",
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

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn source_line(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(SOURCE))
        .unwrap_or("")
        .to_string()
}

fn phrase(screen: &TerminalSnapshot, needle: &str) -> bool {
    text(screen).lines().any(|line| {
        line.match_indices(needle).any(|(at, _)| {
            let rest = &line[at + needle.len()..];
            !rest.starts_with(|c: char| c.is_ascii_alphanumeric())
        })
    })
}

fn require_phrase(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        phrase(screen, needle),
        "missing {needle:?}\n{}",
        text(screen)
    );
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

fn trusted_row(screen: &TerminalSnapshot) -> bool {
    let line = source_line(screen);
    line.contains(TRUSTED) && !line.contains(UNTRUSTED) && !line.contains('•')
}

fn untrusted_row(screen: &TerminalSnapshot) -> bool {
    let line = source_line(screen);
    line.contains(SOURCE)
        && line.contains('•')
        && line.contains(UNTRUSTED)
        && !line.contains(TRUSTED)
}

fn dirty_marks(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(CHANGE)
        && untrusted_row(screen)
        && !visible.contains(UNSAVED)
        && !visible.contains(SAVED)
        && !visible.contains(DIRECTORY)
        && !visible.contains(PREVIEW)
}

fn stable_dirty(screen: &TerminalSnapshot) -> bool {
    dirty_marks(screen) && !text(screen).contains(STATUS)
}

fn clean_trust(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB_TRUST)
        && visible.contains(TITLE)
        && trusted_row(screen)
        && visible.contains(ACME)
        && visible.contains(LOCAL)
        && visible.contains(CORP)
        && !visible.contains(CHANGE)
        && !visible.contains(UNSAVED)
        && !visible.contains(DIRECTORY)
}

fn saved_untrusted(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = source_line(screen);
    visible.contains(CRUMB_TRUST)
        && visible.contains(TITLE)
        && line.contains(UNTRUSTED)
        && !line.contains(TRUSTED)
        && !line.contains('•')
        && !visible.contains(CHANGE)
        && !visible.contains(UNSAVED)
}

fn open_settings(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(DIRECTORY),
        "refusing s unless the manager is showing\n{}",
        text(&before)
    );
    app.key("s");
    app.wait_screen("settings general", |screen| {
        text(screen).contains(CRUMB_GENERAL)
    })
}

fn open_trust(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(CRUMB_GENERAL),
        "expected General before ]\n{}",
        text(&before)
    );
    for _ in 0..4 {
        app.key("]");
    }
    app.wait_screen("settings trust", |screen| {
        text(screen).contains(CRUMB_TRUST) && text(screen).contains(TITLE)
    })
}

fn focus_list(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        focused_line(&before).contains("Trust") && !focused_line(&before).contains(SOURCE),
        "Enter only from the tab strip\n{}",
        text(&before)
    );
    app.key("enter");
    app.wait_screen("trust list", |screen| {
        let line = focused_line(screen);
        line.contains(SOURCE) && line.contains(TRUSTED)
    })
}

fn button_on(app: &App, label: &str, row_has: &[&str]) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(&screen, u32::from(point.y));
            row_has.iter().all(|piece| row.contains(piece))
                && !row.contains("Save…")
                && !row.contains(CLEANUP)
                && !row.contains(ANYWAY)
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

fn focus_near(screen: &TerminalSnapshot, label: &str, others: &[&str]) -> bool {
    let line = focused_line(screen);
    let Some(gutter) = line.find('▎') else {
        return false;
    };
    let Some(at) = line.find(label) else {
        return false;
    };
    others.iter().all(|other| {
        line.find(other)
            .is_none_or(|pos| at.abs_diff(gutter) < pos.abs_diff(gutter))
    })
}

fn wait_until(
    app: &App,
    reason: &str,
    limit: Duration,
    predicate: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + limit;
    loop {
        let screen = app.screen();
        if predicate(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: {reason}\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while Trust stays dirty.
fn nudge(app: &App) {
    app.nudge_width(stable_dirty, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app);
    app.visual(&visual_id(checkpoint));
}

fn back_to_tabs(app: &App) {
    if focused_line(&app.screen()).contains(SOURCE) {
        app.key("escape");
    }
    let tabs = app.wait_screen("tab strip", |screen| {
        let line = focused_line(screen);
        line.contains("Trust") && line.contains("General") && !line.contains(SOURCE)
    });
    assert!(
        dirty_marks(&tabs),
        "left the dirty Trust page\n{}",
        text(&tabs)
    );
}

fn toggle_untrusted(app: &App) -> TerminalSnapshot {
    focus_list(app);
    app.key("space");
    let shown = app.wait_screen("trust status", |screen| {
        text(screen).contains(STATUS) && dirty_marks(screen)
    });
    require_phrase(&shown, STATUS);
    shown
}

#[test]
fn j03_trust_route() {
    let app = open("j03-trust-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let general = open_settings(&app);
    assert!(text(&general).contains(CRUMB_GENERAL), "{}", text(&general));
    let trust = open_trust(&app);
    assert!(
        clean_trust(&trust) && text(&trust).contains(CRUMB_TRUST),
        "clean trust\n{}",
        text(&trust)
    );
    assert!(trusted_row(&trust), "fixture row\n{}", text(&trust));
    assert!(!text(&trust).contains(CHANGE), "{}", text(&trust));
    app.key("escape");
    let back = app.wait_screen("clean leave", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) && !visible.contains(CRUMB_TRUST) && !visible.contains(UNSAVED)
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains(CHANGE), "{}", text(&back));
    app.finish();
}

#[test]
fn j03_trust_trust() {
    let app = open("j03-trust-trust");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    open_settings(&app);
    let trust = open_trust(&app);
    assert!(
        clean_trust(&trust),
        "clean trust before the toggle\n{}",
        text(&trust)
    );

    toggle_untrusted(&app);
    let wide = wait_until(
        &app,
        "stable dirty trust",
        Duration::from_secs(25),
        |screen| screen.cols() == 120 && screen.rows() == 40 && stable_dirty(screen),
    );
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(
        stable_dirty(&wide) && text(&wide).contains(CRUMB_TRUST),
        "dirty trust\n{}",
        text(&wide)
    );
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && stable_dirty(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert!(
        untrusted_row(&narrow) && text(&narrow).contains(CHANGE),
        "narrow dirty trust\n{}",
        text(&narrow)
    );
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && stable_dirty(screen)
    });

    back_to_tabs(&app);
    app.key("escape");
    let dialog = app.wait_screen("unsaved changes", |screen| text(screen).contains(UNSAVED));
    assert!(
        dialog_body(&dialog).contains(LOST),
        "dialog body\n{}",
        text(&dialog)
    );
    assert!(
        text(&dialog).contains(DISCARD) && text(&dialog).contains(SAVE),
        "dialog buttons\n{}",
        text(&dialog)
    );
    assert!(
        focus_near(&dialog, CANCEL, &[DISCARD, SAVE]),
        "initial focus is Cancel\n{}\n{}",
        focused_line(&dialog),
        text(&dialog)
    );
    assert!(!text(&dialog).contains(SAVED), "{}", text(&dialog));
    assert!(!text(&dialog).contains(CLEANUP), "{}", text(&dialog));
    app.click(button_on(&app, DISCARD, &[CANCEL, DISCARD, SAVE]));
    let discarded = app.wait_screen("discarded", |screen| {
        let visible = text(screen);
        visible.contains(DISCARDED)
            && visible.contains(DIRECTORY)
            && !visible.contains(UNSAVED)
            && !visible.contains(CRUMB_TRUST)
    });
    require_phrase(&discarded, DISCARDED);
    assert!(!text(&discarded).contains(SAVED), "{}", text(&discarded));
    assert!(!text(&discarded).contains(CHANGE), "{}", text(&discarded));

    open_settings(&app);
    let restored = open_trust(&app);
    assert!(
        clean_trust(&restored) && trusted_row(&restored),
        "discard wrote the trust flag\n{}",
        text(&restored)
    );

    toggle_untrusted(&app);
    app.key("ctrl+s");
    let preview = app.wait_screen("save settings", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW)
            && visible.contains(FACT)
            && focus_near(screen, CANCEL, &[SAVE])
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
            && !visible.contains(ANYWAY)
    });
    assert!(
        text(&preview).contains(PREVIEW) && phrase(&preview, FACT),
        "preview\n{}",
        text(&preview)
    );
    app.click(button_on(&app, SAVE, &[CANCEL, SAVE]));
    let saved = app.wait_screen("settings saved", |screen| {
        phrase(screen, SAVED) && text(screen).contains(DIRECTORY) && !text(screen).contains(PREVIEW)
    });
    require_phrase(&saved, SAVED);
    assert!(!text(&saved).contains(CRUMB_TRUST), "{}", text(&saved));

    open_settings(&app);
    let again = open_trust(&app);
    assert!(
        saved_untrusted(&again),
        "save did not keep the source untrusted\n{}",
        text(&again)
    );
    app.finish();
}
