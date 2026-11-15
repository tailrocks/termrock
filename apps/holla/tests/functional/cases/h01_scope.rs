//! Holla finder scope, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h01_scope/` and each checkpoint's dots become underscores.
//! Scope is Ctrl+Up / Ctrl+Down. Pin, hide, restore, and ranking reset are
//! alternatives-menu items (Alt+Enter), not keys of their own.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, find_all, row_text, text, try_launch};

/// Binary settle only. `app.rs` pre-ticks this many 80ms frames before the
/// event loop. Freshness stays on "1 min ago" from 60s through 119s.
const FRAME: &str = "1125";

const WIDE: &str = "H01.scope.scope.120x40";
const NARROW: &str = "H01.scope.scope.narrow";

const HERE: &str = "scope ‹ here ›";
const PARENT: &str = "scope ‹ parent ›";
const SYSTEM: &str = "scope ‹ system ›";
const PARENT_SET: &str = "Parent · acme · 29 entries";
const PARENT_HIDDEN: &str = "Parent · acme · 28 entries";
const API_LOGS: &str = "Follow acme-api-1 logs";
const DB_LOGS: &str = "Follow acme-db-1 logs";
const REVIEW: &str = "Review 2 modified files";
const PROJECT: &str = "~/work/acme";
const HOST: &str = "mbp · local";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h01_scope/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec![
        "--scenario".to_string(),
        scenario.to_string(),
        "--frame".to_string(),
        FRAME.to_string(),
    ];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
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

fn selected(screen: &TerminalSnapshot) -> String {
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        if line.trim_start().starts_with('›') {
            return line;
        }
    }
    panic!("no selected row\n{}", text(screen));
}

fn field_value(screen: &TerminalSnapshot, label: &str) -> Option<String> {
    let mut found = None;
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        let mut start = 0;
        while let Some(rel) = line[start..].find(label) {
            let index = start + rel;
            let end = index + label.len();
            let before = index == 0 || line.as_bytes()[index - 1].is_ascii_whitespace();
            let after = &line.as_bytes()[end..];
            // Preview values are column-aligned. A single space is a hint, not the field.
            if before && after.len() >= 2 && after[0] == b' ' && after[1] == b' ' {
                let value = line[end..].trim().trim_end_matches(['┃', '┆', '│']).trim();
                if !value.is_empty() {
                    found = Some(value.to_string());
                }
            }
            start = end;
        }
    }
    found
}

fn field(screen: &TerminalSnapshot, label: &str) -> String {
    field_value(screen, label).unwrap_or_else(|| panic!("no {label} field\n{}", text(screen)))
}

/// The list row and the `Runs` label paint before the preview value.
fn preview_ready(
    screen: &TerminalSnapshot,
    title: &str,
    kind: &str,
    runs: &str,
    directory: &str,
    host: &str,
) -> bool {
    let Some(row) = (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.trim_start().starts_with('›'))
    else {
        return false;
    };
    row.contains(title)
        && row.contains(kind)
        && field_value(screen, "Runs").is_some_and(|value| value.contains(runs))
        && field_value(screen, "In").as_deref() == Some(directory)
        && field_value(screen, "Host").as_deref() == Some(host)
}

fn matches_action(
    screen: &TerminalSnapshot,
    title: &str,
    kind: &str,
    runs: &str,
    directory: &str,
    host: &str,
) {
    let row = selected(screen);
    assert!(
        row.contains(title),
        "selected row {row:?}\n{}",
        text(screen)
    );
    assert!(row.contains(kind), "selected row {row:?}\n{}", text(screen));
    let got_runs = field(screen, "Runs");
    assert!(
        got_runs.contains(runs),
        "runs {got_runs:?}\n{}",
        text(screen)
    );
    assert_eq!(
        field(screen, "In"),
        directory,
        "directory\n{}",
        text(screen)
    );
    assert_eq!(field(screen, "Host"), host, "host\n{}", text(screen));
}

fn choose(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits = find_all(&screen, label);
    assert_eq!(hits.len(), 1, "menu {label:?} {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

/// Same-size resize does not force a new frame. Nudge the width and come back.
/// The wait is the terminal size, not the status-bar size chip.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge width", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
    });
    app.resize(cols, rows);
    app.wait_screen("nudge restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
    });
}

/// Compare the live screen. The caller owns every resize.
fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn remote_identities() -> &'static [&'static str] {
    &[
        "prod-eu-1",
        "/srv/payments",
        "payments-worker",
        "over SSH",
        "v2.14.3",
    ]
}

fn monorepo_identities() -> &'static [&'static str] {
    &["mbp", "acme", "frontend", "~/work/acme"]
}

fn wait_painted(
    app: &App,
    reason: &str,
    pred: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let started = Instant::now();
    loop {
        let screen = app.screen();
        if pred(&screen) {
            return screen;
        }
        let visible = text(&screen);
        assert!(
            app.child_running(),
            "holla exited before {reason}\n{visible}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "timed out: {reason}\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn h01_scope_route() {
    let app = open("h01-scope-route", "monorepo-child");
    // The first paint can stay blank longer than the shared 10s wait when
    // several Holla processes start together.
    let here = wait_painted(&app, "scope here", |screen| text(screen).contains(HERE));
    see(&here, &[HERE, "acme › frontend"], remote_identities());
    app.key("ctrl+up");
    let parent = app.wait_text(PARENT);
    see(&parent, &[PARENT, "Parent · acme"], &[HERE]);
    app.key("ctrl+down");
    let back = app.wait_text(HERE);
    see(&back, &[HERE, "Suggested here"], &[PARENT_SET]);
    app.finish();
}

fn wait_here_list(app: &App) -> TerminalSnapshot {
    let started = Instant::now();
    loop {
        let screen = app.screen();
        let visible = text(&screen);
        if screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(HERE)
            && visible.contains("Suggested here")
            && visible.contains("nothing used here yet")
            && visible.contains("acme › frontend")
            && !visible.contains(PARENT_SET)
        {
            return screen;
        }
        assert!(
            app.child_running(),
            "holla exited before the scope list\n{visible}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "timed out: here scope list\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn h01_scope_scope() {
    let app = open("h01-scope-scope", "monorepo-child");
    // The first paint can stay blank longer than the shared 10s wait when
    // several Holla processes start together. The list itself still has to
    // show Suggested here before Recent here is drawn.
    let here = wait_here_list(&app);
    see(
        &here,
        &[
            HERE,
            "Suggested here",
            "nothing used here yet",
            "acme › frontend",
        ],
        &[PARENT_SET],
    );

    app.key("ctrl+up");
    let parent = app.wait_screen("parent scope changes the visible set", |screen| {
        let visible = text(screen);
        visible.contains(PARENT)
            && visible.contains(PARENT_SET)
            && visible.contains(API_LOGS)
            && !visible.contains("Suggested here")
            && !visible.contains("nothing used here yet")
            && !visible.contains("System · mbp")
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    matches_action(&parent, API_LOGS, "docker", "docker logs", PROJECT, HOST);
    assert!(!field(&parent, "In").contains("frontend"));

    flash_settle(&app);
    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    see(&wide, &[PARENT, PARENT_SET, API_LOGS], &["Suggested here"]);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrow = app.wait_screen("parent scope at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && text(screen).contains(PARENT)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    see(&narrow, &[PARENT, "Parent · acme"], &["Suggested here"]);
    shot(&app, NARROW);
    app.resize(120, 40);

    let restored = app.wait_screen("parent scope back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(PARENT_SET)
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    matches_action(&restored, API_LOGS, "docker", "docker logs", PROJECT, HOST);

    app.key("ctrl+up");
    let system = app.wait_screen("system scope", |screen| {
        let visible = text(screen);
        visible.contains("System · mbp · 44 entries")
            && visible.contains("Clean Docker completely")
            && !visible.contains(PARENT_SET)
            && preview_ready(
                screen,
                "Clean Docker completely",
                "plan",
                "docker stop",
                "mbp · host-wide",
                HOST,
            )
    });
    see(
        &system,
        &[
            SYSTEM,
            "System · mbp · 44 entries",
            "Clean Docker completely",
        ],
        &[PARENT_SET, "Suggested here"],
    );
    matches_action(
        &system,
        "Clean Docker completely",
        "plan",
        "docker stop",
        "mbp · host-wide",
        HOST,
    );
    app.key("escape");
    app.wait_text(HERE);

    app.key("ctrl+up");
    let again = app.wait_screen("parent scope again", |screen| {
        text(screen).contains(PARENT_SET)
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    matches_action(&again, API_LOGS, "docker", "docker logs", PROJECT, HOST);

    app.key("alt+enter");
    app.wait_text("Pin here");
    choose(&app, "Pin here");
    let pinned = app.wait_screen("pinned here", |screen| {
        text(screen).contains("Pinned here · it stays near the top in this folder")
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    see(
        &pinned,
        &[PARENT_SET, API_LOGS, DB_LOGS, REVIEW, "pinned here"],
        &[],
    );
    matches_action(&pinned, API_LOGS, "docker", "docker logs", PROJECT, HOST);

    app.key("alt+enter");
    app.wait_text("Hide here");
    choose(&app, "Hide here");
    let hidden = app.wait_screen("hidden here", |screen| {
        text(screen).contains("Hidden here · still reachable by search")
            && preview_ready(screen, REVIEW, "git", "git -C", PROJECT, HOST)
    });
    see(
        &hidden,
        &[PARENT_HIDDEN, DB_LOGS, REVIEW],
        &[API_LOGS, "acme-api-1"],
    );
    matches_action(&hidden, REVIEW, "git", "git -C", PROJECT, HOST);

    app.type_text("acme-api-1");
    let found = app.wait_screen("hidden row found", |screen| {
        text(screen).contains("Results · 1")
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    see(&found, &[API_LOGS, "hidden here"], &[PARENT_HIDDEN]);
    matches_action(&found, API_LOGS, "docker", "docker logs", PROJECT, HOST);

    app.key("alt+enter");
    app.wait_text("Restore here");
    choose(&app, "Restore here");
    let restored_row = app.wait_screen("restored here", |screen| {
        text(screen).contains("Restored here")
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    matches_action(
        &restored_row,
        API_LOGS,
        "docker",
        "docker logs",
        PROJECT,
        HOST,
    );

    app.key("escape");
    let listed = app.wait_text(PARENT_SET);
    see(
        &listed,
        &[PARENT_SET, API_LOGS, DB_LOGS, REVIEW],
        &["Results · 1"],
    );

    app.key("alt+enter");
    app.wait_text("Reset ranking");
    choose(&app, "Reset ranking");
    let reset = app.wait_screen("ranking reset", |screen| {
        text(screen).contains("Ranking reset for this row · pins and aliases kept")
            && preview_ready(screen, API_LOGS, "docker", "docker logs", PROJECT, HOST)
    });
    see(
        &reset,
        &[PARENT_SET, API_LOGS, DB_LOGS, REVIEW, "pinned here"],
        &["Reset ranking"],
    );
    matches_action(&reset, API_LOGS, "docker", "docker logs", PROJECT, HOST);

    app.type_text("t");
    let alias = app.wait_text("alias t");
    let row = selected(&alias);
    assert!(
        row.contains("Tasks") && row.contains("explore"),
        "alias row {row:?}\n{}",
        text(&alias)
    );
    app.key("enter");
    let tasks = app.wait_text("Tasks ·");
    see(
        &tasks,
        &["Tasks ·", "Run tests"],
        &[API_LOGS, "Clean Docker completely", "Suggested here"],
    );
    app.key("escape");
    app.wait_screen("tasks group popped", |screen| {
        let visible = text(screen);
        visible.contains(PARENT) && !visible.contains("Tasks ·")
    });
    app.finish();
}

#[test]
fn h01_scope_scenario_monorepo_child() {
    let app = open("h01-scope-monorepo", "monorepo-child");
    // The first paint can stay blank longer than the shared 10s wait. The
    // scope marker then paints while the In column still ellipsizes the path.
    let here = wait_painted(&app, "monorepo child", |screen| {
        let visible = text(screen);
        visible.contains(HERE) && visible.contains("~/work/acme/apps/frontend")
    });
    see(
        &here,
        &[
            "acme › frontend",
            HOST,
            "~/work/acme/apps/frontend",
            HERE,
            "Run tests",
        ],
        remote_identities(),
    );
    app.key("ctrl+up");
    let parent = app.wait_text(PARENT_SET);
    see(
        &parent,
        &[PARENT, "Parent · acme", PROJECT],
        remote_identities(),
    );
    app.finish();
}

#[test]
fn h01_scope_scenario_remote_host() {
    let app = open("h01-scope-remote", "remote-host");
    // The scope chip is drawn before the footer names the detached revision.
    let here = app.wait_screen("remote here", |screen| {
        let visible = text(screen);
        visible.contains(HERE) && visible.contains("v2.14.3")
    });
    see(
        &here,
        &[
            "payments",
            "prod-eu-1 · production",
            "/srv/payments",
            "over SSH",
            "v2.14.3",
            "psql",
            HERE,
        ],
        monorepo_identities(),
    );
    assert_eq!(field(&here, "In"), "/srv/payments");

    app.key("ctrl+up");
    let parent = app.wait_text("No project root above /srv/payments");
    see(
        &parent,
        &[PARENT, "No project root above /srv/payments"],
        monorepo_identities(),
    );

    app.key("ctrl+up");
    let system = app.wait_text("System · prod-eu-1");
    see(
        &system,
        &[
            SYSTEM,
            "System · prod-eu-1",
            "payments-worker",
            "journalctl",
            "prod-eu-1 · host-wide",
        ],
        monorepo_identities(),
    );
    app.finish();
}
