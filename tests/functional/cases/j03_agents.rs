//! Jackin global settings agents, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j03_agents/`. The shots are the dirty Agents page after Claude
//! Code moves from sync to api key, once the timed status has cleared, and
//! before Ctrl+S. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, button, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const INTRO: &str = "Stand up, operator";
const CRUMB_GENERAL: &str = "Settings › global › General";
const CRUMB_MOUNTS: &str = "Settings › global › Mounts";
const CRUMB_ENVS: &str = "Settings › global › Environments";
const CRUMB_AGENTS: &str = "Settings › global › Agents";
const TITLE: &str = "Agent runtime mode";
const CLAUDE: &str = "Claude Code";
const CODEX: &str = "Codex";
const KIMI: &str = "Kimi Code";
const SYNC: &str = "sync";
const API_KEY: &str = "api key";
const IGNORE: &str = "ignore";
const STATUS: &str = "Claude Code · api key · save to apply";
const CHANGE: &str = "• 1 change";
const MARK: char = '•';
const PREVIEW_TITLE: &str = "Save settings";
const FACT: &str = "Claude Code → api key";
const CLEANUP: &str = "Clean up & save…";
const ANYWAY: &str = "Save anyway";
const SAVED: &str = "Settings saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const WIDE: &str = "J03.agents.agents.120x40";
const NARROW: &str = "J03.agents.agents.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j03_agents/{}",
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

fn require(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    if !visible.contains(needle) {
        panic!("missing {needle:?}\n{visible}");
    }
}

fn agent_line(screen: &TerminalSnapshot, name: &str) -> String {
    text(screen)
        .lines()
        .find(|line| line.contains(name) && !line.contains("save to apply") && !line.contains('→'))
        .unwrap_or("")
        .to_string()
}

fn mode_on(screen: &TerminalSnapshot, name: &str, mode: &str) -> bool {
    let line = agent_line(screen, name);
    line.contains(name) && line.contains(mode)
}

fn changed_row(screen: &TerminalSnapshot, name: &str) -> bool {
    agent_line(screen, name).contains(MARK)
}

fn exact_status(screen: &TerminalSnapshot, expected: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(at) = line.find(expected) else {
            return false;
        };
        let after = &line[at + expected.len()..];
        !after.starts_with(|c: char| c.is_ascii_alphanumeric())
    })
}

fn dirty_agents(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB_AGENTS)
        && visible.contains(TITLE)
        && visible.contains(CHANGE)
        && !visible.contains("• 2 change")
        && mode_on(screen, CLAUDE, API_KEY)
        && changed_row(screen, CLAUDE)
        && !agent_line(screen, CLAUDE).contains(SYNC)
        && mode_on(screen, CODEX, API_KEY)
        && !changed_row(screen, CODEX)
        && mode_on(screen, KIMI, IGNORE)
        && !visible.contains("save to apply")
        && !visible.contains(SAVED)
        && !visible.contains(PREVIEW_TITLE)
        && !visible.contains(DIRECTORY)
}

fn open_settings(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        text(&screen).contains(DIRECTORY),
        "refusing s unless the manager is showing\n{}",
        text(&screen)
    );
    app.key("s");
    app.wait_screen("settings general", |screen| {
        text(screen).contains(CRUMB_GENERAL)
    })
}

fn open_settings_agents(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(CRUMB_GENERAL),
        "expected General before ]\n{}",
        text(&before)
    );
    app.key("]");
    app.wait_screen("settings mounts", |screen| {
        text(screen).contains(CRUMB_MOUNTS)
    });
    app.key("]");
    app.wait_screen("settings environments", |screen| {
        text(screen).contains(CRUMB_ENVS)
    });
    app.key("]");
    app.wait_screen("settings agents", |screen| {
        let visible = text(screen);
        visible.contains(CRUMB_AGENTS) && visible.contains(TITLE)
    })
}

fn focus_claude(app: &App) -> TerminalSnapshot {
    app.key("enter");
    app.wait_screen("agent list", |screen| {
        let line = focused_line(screen);
        line.contains(CLAUDE) && agent_line(screen, CLAUDE).contains(SYNC)
    })
}

fn preview_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(ANYWAY)
            && !row.contains(UNSAVED)
    })
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the Agents page stays dirty.
fn nudge(app: &App) {
    app.nudge_width(dirty_agents, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app);
    app.visual(&visual_id(checkpoint));
}

fn leave_settings(app: &App) -> TerminalSnapshot {
    app.key("escape");
    app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(CRUMB_GENERAL)
            && !text(screen).contains(CRUMB_AGENTS)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains(TITLE)
    })
}

fn saved_modes(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(CRUMB_AGENTS)
        && visible.contains(TITLE)
        && mode_on(screen, CLAUDE, API_KEY)
        && !changed_row(screen, CLAUDE)
        && mode_on(screen, CODEX, API_KEY)
        && !changed_row(screen, CODEX)
        && mode_on(screen, KIMI, IGNORE)
        && !visible.contains(CHANGE)
        && !visible.contains("save to apply")
        && !visible.contains(PREVIEW_TITLE)
        && !visible.contains(DIRECTORY)
}

#[test]
fn j03_agents_route() {
    let app = open("j03-agents-route");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let general = open_settings(&app);
    require(&general, CRUMB_GENERAL);
    let agents = open_settings_agents(&app);
    assert!(
        text(&agents).contains(CRUMB_AGENTS) && text(&agents).contains(TITLE),
        "agents tab\n{}",
        text(&agents)
    );
    assert!(
        mode_on(&agents, CLAUDE, SYNC)
            && mode_on(&agents, CODEX, API_KEY)
            && mode_on(&agents, KIMI, IGNORE)
            && !text(&agents).contains(CHANGE)
            && !text(&agents).contains(SAVED)
            && !text(&agents).contains(PREVIEW_TITLE)
            && !changed_row(&agents, CLAUDE),
        "clean agent modes\n{}",
        text(&agents)
    );
    let back = leave_settings(&app);
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains(CHANGE), "{}", text(&back));
    assert!(!text(&back).contains(UNSAVED), "{}", text(&back));
    app.finish();
}

#[test]
fn j03_agents_agents() {
    let app = open("j03-agents-agents");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    open_settings(&app);
    let agents = open_settings_agents(&app);
    assert!(
        mode_on(&agents, CLAUDE, SYNC) && !text(&agents).contains(CHANGE),
        "claude starts on sync\n{}",
        text(&agents)
    );
    let focused = focus_claude(&app);
    assert!(
        focused_line(&focused).contains(CLAUDE) && agent_line(&focused, CLAUDE).contains(SYNC),
        "cursor starts on Claude Code\n{}",
        text(&focused)
    );

    app.key("space");
    let shown = app.wait_screen("cycle status", |screen| {
        exact_status(screen, STATUS)
            && mode_on(screen, CLAUDE, API_KEY)
            && changed_row(screen, CLAUDE)
            && text(screen).contains(CHANGE)
            && !text(screen).contains(SAVED)
            && !text(screen).contains(PREVIEW_TITLE)
    });
    assert_eq!(
        text(&shown).matches(STATUS).count(),
        1,
        "status text\n{}",
        text(&shown)
    );
    assert!(
        mode_on(&shown, CODEX, API_KEY)
            && !changed_row(&shown, CODEX)
            && mode_on(&shown, KIMI, IGNORE),
        "only claude cycled\n{}",
        text(&shown)
    );
    app.wait_screen("status cleared", dirty_agents);

    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(dirty_agents(&wide), "dirty agents\n{}", text(&wide));
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && dirty_agents(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dirty_agents(screen)
    });

    app.key("ctrl+s");
    let preview = app.wait_screen("save settings", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && visible.contains(FACT)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
            && !visible.contains(ANYWAY)
    });
    require(&preview, PREVIEW_TITLE);
    require(&preview, FACT);
    let line = focused_line(&preview);
    if line.contains(CANCEL) {
        app.click(preview_save(&app));
    } else if line.contains(CONFIRM) && !line.contains("Save…") && !line.contains(CLEANUP) {
        app.key("enter");
    } else {
        panic!("save dialog focus\n{line}\n{}", text(&preview));
    }
    let saved = app.wait_screen("settings saved", |screen| {
        exact_status(screen, SAVED) && text(screen).contains(DIRECTORY)
    });
    assert!(
        exact_status(&saved, SAVED) && !text(&saved).contains(PREVIEW_TITLE),
        "saved status\n{}",
        text(&saved)
    );
    assert!(
        !text(&saved).contains(CRUMB_AGENTS),
        "still on agents\n{}",
        text(&saved)
    );

    open_settings(&app);
    let again = open_settings_agents(&app);
    assert!(saved_modes(&again), "reopened agents\n{}", text(&again));
    leave_settings(&app);
    app.finish();
}
