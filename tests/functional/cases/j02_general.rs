//! Jackin workspace general fields, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j02_general/`. The shots are the open Save workspace preview
//! after the name, working directory, and keep-awake edits, before Cancel and
//! before Save. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.
//!
//! `Theme::DEFAULT_EDIT_ENTRY` is Immediate. The first key on Name replaces
//! the focused selection. `resume` after Enter leaves `name.editing` set, and
//! `general_key` only moves on Down when that flag is clear, so Tab is what
//! leaves Name for Choose….

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const LAB: &str = "payments-lab";
const INFRA: &str = "infra-control-plane";
const RELEASE: &str = "release-automation";
const CUSTOMER: &str = "customer-portal";
const CRUMB: &str = "Workspaces › payments-platform › edit";
const LAB_CRUMB: &str = "Workspaces › payments-lab › edit";
const CHOOSE: &str = "Choose";
const KEEP: &str = "Keep awake";
const ON: &str = "[✓]";
const OFF: &str = "[ ]";
const NAME_STATUS: &str = "Name payments-lab · save to apply";
const WORKDIR: &str = "/workspace/payments-platform";
const CRATES: &str = "/workspace/payments-platform/crates";
const CRATES_PEEK: &str = "/workspace/payments-platform/crates — 6 items";
const WORKDIR_STATUS: &str =
    "Working directory /workspace/payments-platform/crates · save to apply";
const PREVIEW: &str = "Save workspace";
const KEEP_DIFF: &str = "~ keep_awake true → false";
const CHANGES: &str = "3 changes";
const NOT_SAVED: &str = "Not saved · keep editing";
const SAVED: &str = "Workspace payments-lab saved";
const UNSAVED: &str = "Unsaved changes";
const DISCARDED: &str = "Discarded 1 change";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Discard";
const SAVE: &str = "Save";
const WIDE: &str = "J02.general.general.120x40";
const NARROW: &str = "J02.general.general.narrow";
const SPINNERS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_general/{}",
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
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let started = Instant::now();
    let screen = loop {
        let screen = app.screen();
        let visible = text(&screen);
        if visible.contains(DIRECTORY) || visible.contains("Stand up, operator") {
            break screen;
        }
        assert!(
            app.child_running(),
            "jackin exited before the first frame\n{visible}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "timed out: manager or intro\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
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

fn lists_workspace(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| {
        if line.contains("Workspace ") && line.contains(" saved") {
            return false;
        }
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            !matches!(before, Some('/'))
        })
    })
}

fn assert_returning(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(lists_workspace(screen, PAYMENTS), "{visible}");
    assert!(visible.contains("7f3a"), "{visible}");
    assert!(visible.contains("c41e"), "{visible}");
    assert!(lists_workspace(screen, INFRA), "{visible}");
    assert!(lists_workspace(screen, RELEASE), "{visible}");
    assert!(lists_workspace(screen, CUSTOMER), "{visible}");
    assert!(!visible.contains("data-pipeline"), "{visible}");
    assert!(!visible.contains("auth-service"), "{visible}");
    assert!(!visible.contains("▲ stale"), "{visible}");
}

fn focus_payments(app: &App) -> TerminalSnapshot {
    let before = focused_line(&app.screen());
    app.key("down");
    let selected = app.wait_screen("payments-platform row", |screen| {
        focused_line(screen) != before && focused_line(screen).contains(PAYMENTS)
    });
    let line = focused_line(&selected);
    assert!(
        line.contains(PAYMENTS) && !line.contains(DIRECTORY),
        "Down did not leave Current directory\n{line}\n{}",
        text(&selected)
    );
    selected
}

fn open_editor(app: &App) -> TerminalSnapshot {
    app.key("e");
    app.wait_text(CRUMB)
}

fn keep_line(screen: &TerminalSnapshot) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(KEEP))
        .unwrap_or("")
        .to_string()
}

fn name_line(screen: &TerminalSnapshot, value: &str) -> bool {
    text(screen).lines().any(|line| {
        line.contains(value)
            && !line.contains('›')
            && !line.contains("save to apply")
            && !line.contains("Workspace ")
            && !line.contains(WORKDIR)
    })
}

fn type_name(app: &App) {
    app.key("enter");
    app.wait_text("Enter Apply");
    // A burst of character keys is only partly applied. Wait until the focused
    // line shows each prefix, so the next key is not written over an unread one.
    let mut so_far = String::new();
    for ch in LAB.chars() {
        so_far.push(ch);
        app.key(&ch.to_string());
        let prefix = so_far.clone();
        app.wait_screen("name grows", |screen| {
            let line = focused_line(screen);
            line.contains(&prefix) && !line.contains(PAYMENTS)
        });
    }
    let ready = app.screen();
    assert!(
        name_line(&ready, LAB) && !name_line(&ready, PAYMENTS),
        "name field is not {LAB}\n{}",
        text(&ready)
    );
    app.key("enter");
    app.wait_text(NAME_STATUS);
}

fn tab_to(app: &App, label: &str) {
    app.key("tab");
    let landed = app.wait_screen(label, |screen| focused_line(screen).contains(label));
    assert!(
        focused_line(&landed).contains(label),
        "tab never reached {label}\n{}",
        text(&landed)
    );
}

fn down_to(app: &App, label: &str) {
    for _ in 0..6 {
        if focused_line(&app.screen()).contains(label) {
            return;
        }
        let before = focused_line(&app.screen());
        app.key("down");
        app.wait_screen(label, |screen| focused_line(screen) != before);
    }
    let screen = app.screen();
    panic!("down never reached {label}\n{}", text(&screen));
}

fn pick_crates(app: &App) {
    app.key("enter");
    // The editor already paints "Working directory" and the current path.
    // The picker is the layer that adds mount details and subdirectory rows.
    // The label column is capped, so the crates path paints as a truncated
    // prefix. Its detail is the only "6 items" row.
    let picker = app.wait_screen("workdir picker", |screen| {
        let visible = text(screen);
        visible.contains("subdirectories") || visible.contains("mount ·")
    });
    let visible = text(&picker);
    let crates_row = visible.lines().any(|line| {
        line.contains("6 items")
            && line.contains("/workspace/payments-pl")
            && !line.contains("adr")
            && !line.contains("3 items")
    });
    assert!(crates_row, "crates row is absent\n{visible}");
    // set_items starts on the first enabled row, the current mount.
    // The next row is the crates subdirectory. A fill cursor does not change
    // the row text, so the footer peek is the only extra check.
    app.key("down");
    let chosen = app.settle();
    let visible = text(&chosen);
    if visible.contains('—') {
        assert!(
            visible.contains(CRATES_PEEK),
            "peek was not the crates row\n{visible}"
        );
    }
    app.key("enter");
    // The field label and the name's "save to apply" are already visible while
    // the picker is open. The picker hint is "Type Filter". The closed form
    // hint is "Enter Choose…", so that text does not mean the picker is open.
    let applied = app.wait_screen("workdir status", |screen| {
        let visible = text(screen);
        visible.contains(WORKDIR_STATUS) && !visible.contains("Type Filter")
    });
    let status = text(&applied);
    assert!(
        status.contains(WORKDIR_STATUS),
        "workdir status was not the crates path\n{status}"
    );
}

fn toggle_keep_awake(app: &App) {
    down_to(app, KEEP);
    app.key("space");
    let toggled = app.wait_screen("keep awake off", |screen| keep_line(screen).contains(OFF));
    assert!(
        keep_line(&toggled).contains(OFF) && !keep_line(&toggled).contains(ON),
        "keep awake did not clear\n{}",
        text(&toggled)
    );
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

fn quiet(visible: &str, line: &str) -> bool {
    line.contains("▎Cancel")
        && !visible.contains(SAVED)
        && !visible.contains("Saving")
        && !visible.contains(" s ago")
        && !visible.contains("save to apply")
        && !visible.contains(NOT_SAVED)
        && SPINNERS.iter().all(|glyph| !visible.contains(glyph))
}

fn preview_facts(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(PREVIEW)
        && visible.contains(LAB)
        && visible.contains(CHANGES)
        && visible.contains(KEEP_DIFF)
        && visible.contains(CRATES)
        && visible.contains("→")
        && quiet(&visible, &line)
}

/// Dialog width is 66, so the Working dir fact stays
/// `/workspace/payments-platform → /workspace/paym…` at 80 columns.
/// The editor field that paints the full crates path sits under that dialog.
fn narrow_facts(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(PREVIEW)
        && visible.contains(LAB)
        && visible.contains(CHANGES)
        && visible.contains(KEEP_DIFF)
        && visible.contains("Working dir")
        && visible.contains("→ /workspace/paym")
        && quiet(&visible, &line)
}

fn frame_facts(screen: &TerminalSnapshot) -> bool {
    if screen.rows() == 24 {
        narrow_facts(screen)
    } else {
        preview_facts(screen)
    }
}

fn open_preview(app: &App) -> TerminalSnapshot {
    app.key("ctrl+s");
    app.wait_screen("save preview", |screen| {
        text(screen).contains(PREVIEW) && text(screen).contains(KEEP_DIFF)
    })
}

fn quiet_preview(app: &App) -> TerminalSnapshot {
    app.wait_screen("preview without a transient status", preview_facts)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the preview stays open.
fn nudge(app: &App) {
    app.nudge_width(frame_facts, Some("nudge left the frame"));
}

fn shot(app: &App, checkpoint: &str) {
    nudge(app);
    app.visual(&visual_id(checkpoint));
}

#[test]
fn j02_general_route() {
    let app = open("j02-general-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    focus_payments(&app);
    assert_returning(&app.screen());
    let editor = open_editor(&app);
    assert!(text(&editor).contains(CRUMB), "{}", text(&editor));
    assert!(
        keep_line(&editor).contains(ON),
        "keep awake starts checked\n{}",
        text(&editor)
    );
    app.key("escape");
    let back = app.wait_screen("editor leaves without a dialog", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains("› edit")
    });
    assert!(lists_workspace(&back, PAYMENTS), "{}", text(&back));
    assert_returning(&back);
    app.finish();
}

#[test]
fn j02_general_general() {
    let app = open("j02-general-general");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    focus_payments(&app);
    assert_returning(&app.screen());
    let editor = open_editor(&app);
    assert!(text(&editor).contains(CRUMB), "{}", text(&editor));
    assert!(keep_line(&editor).contains(ON), "{}", text(&editor));

    app.key("down");
    app.wait_text("Enter Apply");
    tab_to(&app, CHOOSE);
    down_to(&app, KEEP);
    app.key("space");
    let cleared = app.wait_screen("discard path clears keep awake", |screen| {
        keep_line(screen).contains(OFF)
    });
    assert!(!text(&cleared).contains(SAVED), "{}", text(&cleared));
    app.key("escape");
    app.key("escape");
    let unsaved = app.wait_text(UNSAVED);
    assert!(text(&unsaved).contains(DISCARD), "{}", text(&unsaved));
    assert!(text(&unsaved).contains(SAVE), "{}", text(&unsaved));
    assert!(
        focused_line(&unsaved).contains(CANCEL),
        "initial focus is Cancel\n{}",
        text(&unsaved)
    );
    app.click(button_on(&app, DISCARD, &[CANCEL, DISCARD, SAVE]));
    let discarded = app.wait_screen("discard returns to the manager", |screen| {
        text(screen).contains(DISCARDED)
            && text(screen).contains(DIRECTORY)
            && !text(screen).contains(UNSAVED)
    });
    assert!(!text(&discarded).contains(SAVED), "{}", text(&discarded));
    assert!(
        lists_workspace(&discarded, PAYMENTS),
        "{}",
        text(&discarded)
    );
    let line = focused_line(&discarded);
    assert!(
        line.contains(PAYMENTS) && !line.contains(DIRECTORY),
        "reopen would hit Current directory\n{line}\n{}",
        text(&discarded)
    );
    app.key("e");
    let reopened = app.wait_text(CRUMB);
    assert!(name_line(&reopened, PAYMENTS), "{}", text(&reopened));
    assert!(
        keep_line(&reopened).contains(ON),
        "discarded keep awake was saved\n{}",
        text(&reopened)
    );
    assert!(!text(&reopened).contains(SAVED), "{}", text(&reopened));
    assert!(!text(&reopened).contains(LAB), "{}", text(&reopened));

    type_name(&app);
    assert!(
        text(&app.screen()).contains(CRUMB),
        "{}",
        text(&app.screen())
    );
    tab_to(&app, CHOOSE);
    pick_crates(&app);
    let picked = app.screen();
    assert!(
        text(&picked).contains(CRATES),
        "working directory did not take the crates path\n{}",
        text(&picked)
    );
    toggle_keep_awake(&app);
    let dirty = app.screen();
    assert!(text(&dirty).contains(LAB), "{}", text(&dirty));
    assert!(keep_line(&dirty).contains(OFF), "{}", text(&dirty));
    assert!(text(&dirty).contains(CRUMB), "{}", text(&dirty));

    let preview = open_preview(&app);
    assert!(
        preview_facts(&preview) || text(&preview).contains("save to apply"),
        "preview missing facts\n{}",
        text(&preview)
    );
    assert!(text(&preview).contains(CHANGES), "{}", text(&preview));
    assert!(text(&preview).contains(KEEP_DIFF), "{}", text(&preview));
    assert!(text(&preview).contains(CRATES), "{}", text(&preview));
    assert!(
        focused_line(&preview).contains(CANCEL),
        "{}",
        text(&preview)
    );
    app.key("escape");
    let cancelled = app.wait_screen("escape cancels the preview", |screen| {
        text(screen).contains(NOT_SAVED) && !text(screen).contains(PREVIEW)
    });
    assert!(!text(&cancelled).contains(SAVED), "{}", text(&cancelled));
    assert!(text(&cancelled).contains(LAB), "{}", text(&cancelled));
    assert!(text(&cancelled).contains(CRUMB), "{}", text(&cancelled));
    assert!(keep_line(&cancelled).contains(OFF), "{}", text(&cancelled));

    open_preview(&app);
    let settled = quiet_preview(&app);
    assert_eq!((settled.cols(), settled.rows()), (120, 40));
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && frame_facts(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && preview_facts(screen)
    });
    assert!(text(&restored).contains(PREVIEW), "{}", text(&restored));

    app.click(button_on(&app, SAVE, &[CANCEL, SAVE]));
    let saved = app.wait_screen("workspace saved", |screen| {
        text(screen).contains(SAVED)
            && text(screen).contains(DIRECTORY)
            && !text(screen).contains(PREVIEW)
    });
    assert!(lists_workspace(&saved, LAB), "{}", text(&saved));
    assert!(!lists_workspace(&saved, PAYMENTS), "{}", text(&saved));
    assert!(lists_workspace(&saved, INFRA), "{}", text(&saved));
    assert!(lists_workspace(&saved, RELEASE), "{}", text(&saved));
    assert!(lists_workspace(&saved, CUSTOMER), "{}", text(&saved));
    assert!(text(&saved).contains("7f3a"), "{}", text(&saved));
    assert!(text(&saved).contains("c41e"), "{}", text(&saved));
    let line = focused_line(&saved);
    assert!(
        line.contains(LAB) && !line.contains(DIRECTORY),
        "reopen would hit Current directory\n{line}\n{}",
        text(&saved)
    );
    app.key("e");
    let edited = app.wait_text(LAB_CRUMB);
    assert!(name_line(&edited, LAB), "{}", text(&edited));
    assert!(
        text(&edited).contains(CRATES),
        "reopened working directory\n{}",
        text(&edited)
    );
    assert!(
        keep_line(&edited).contains(OFF) && !keep_line(&edited).contains(ON),
        "reopened keep awake\n{}",
        text(&edited)
    );
    assert!(!text(&edited).contains(CRUMB), "{}", text(&edited));
    app.finish();
}
