//! Holla activity picker, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h03_picker/` and each checkpoint's dots become underscores.
//!
//! `activities_multi` calls `World::tick` 48 times, but that returns before
//! `self.tick` increments when `!clock.running` (`src/bin/holla/sim/world.rs`).
//! `clock.running` is false for paused motion
//! (`src/bin/holla/domain/fixtures.rs`). The pre-ticks therefore emit nothing
//! past tick 0. api dev's body is `$ mise run dev  (services/api)` and not the
//! tick-41 listening line. Duration follows `world.tick`
//! (`src/bin/holla/domain/activity.rs` `duration_ticks`) and stays `0 s`.
//! Reduced motion would keep changing that text.
//!
//! Live activities make Ctrl+Q a destructive dialog. `Dialog::destructive`
//! focuses cancel, drawn `▎Stay`, beside `Stop and quit`. Confirming stop
//! marks the activities cancelling. Reaping needs further ticks
//! (`TERM_EXIT_TICKS`), which a paused clock never delivers, so the process
//! does not exit. The test does not call `finish` while the dialog is open
//! and does not call `kill_child`.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, Point, cell_bg, find_all, row_text, text, try_launch};

const FRONT: &str = "frontend dev";
const API: &str = "api dev";
const TITLE: &str = "Activities";
const LIVE: &str = "5 live";
const SWITCH: &str = "Enter Switch";
const API_LINE: &str = "$ mise run dev  (services/api)";
const FRONT_LINE: &str = "(apps/frontend)";
const VITE: &str = "VITE v6.3.1";
const FRONT_URL: &str = "http://localhost:5173/";
const LISTEN: &str = "listening on 0.0.0.0:8080";
const FINDER: &str = "Search actions";
const QUIT_TITLE: &str = "Quit holla";
const STOP: &str = "Stop and quit";
const STAY: &str = "Stay";
const STOPPING: &str = "still cancelling";
const WIDE: &str = "H03.picker.picker.120x40";
const NARROW: &str = "H03.picker.picker.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h03_picker/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    launch.args = vec!["--scenario".to_string(), "activities-multi".to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(FINDER);
    app
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

fn picker_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE) && visible.contains(LIVE) && visible.contains(SWITCH)
}

/// The popup row is the rightmost hit on a line that carries the paused
/// duration. The finder and the tab strip repeat the same names.
fn picker_hit(screen: &TerminalSnapshot, label: &str) -> Option<Point> {
    let mut hits: Vec<_> = find_all(screen, label)
        .into_iter()
        .filter(|hit| row_text(screen, u32::from(hit.y)).contains("· 0 s"))
        .collect();
    hits.sort_by_key(|hit| hit.x);
    hits.pop()
}

fn title_hit(screen: &TerminalSnapshot) -> Option<Point> {
    find_all(screen, TITLE)
        .into_iter()
        .find(|hit| row_text(screen, u32::from(hit.y)).contains(LIVE))
}

fn highlighted(screen: &TerminalSnapshot, label: &str) -> bool {
    let Some(hit) = picker_hit(screen, label) else {
        return false;
    };
    let Some(title) = title_hit(screen) else {
        return false;
    };
    cell_bg(screen, hit) != cell_bg(screen, title)
}

fn frontend_first(screen: &TerminalSnapshot) -> bool {
    match (picker_hit(screen, FRONT), picker_hit(screen, API)) {
        (Some(front), Some(api)) => front.y < api.y,
        _ => false,
    }
}

fn on_frontend(screen: &TerminalSnapshot) -> bool {
    picker_open(screen)
        && frontend_first(screen)
        && highlighted(screen, FRONT)
        && !highlighted(screen, API)
}

fn on_api(screen: &TerminalSnapshot) -> bool {
    picker_open(screen)
        && frontend_first(screen)
        && highlighted(screen, API)
        && !highlighted(screen, FRONT)
}

fn button_focused(screen: &TerminalSnapshot, label: &str, other: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(at) = line.find(label) else {
            return false;
        };
        let Some(elsewhere) = line.find(other) else {
            return false;
        };
        line[..at].ends_with('▎') && !line[..elsewhere].ends_with('▎')
    })
}

fn stay_focused(screen: &TerminalSnapshot) -> bool {
    text(screen).contains(QUIT_TITLE) && button_focused(screen, STAY, STOP)
}

fn stop_focused(screen: &TerminalSnapshot) -> bool {
    text(screen).contains(QUIT_TITLE) && button_focused(screen, STOP, STAY)
}

fn open_picker(app: &App) -> TerminalSnapshot {
    app.key("ctrl+g");
    let picker = app.wait_screen("activities open", on_frontend);
    assert_eq!((picker.cols(), picker.rows()), (120, 40));
    see(
        &picker,
        &[TITLE, LIVE, FRONT, API, SWITCH],
        &[API_LINE, VITE],
    );
    assert!(
        on_frontend(&picker),
        "first row is not frontend dev\n{}",
        text(&picker)
    );
    picker
}

fn back_to_finder(app: &App) {
    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        visible.contains(FINDER) && !visible.contains(LIVE) && !visible.contains(SWITCH)
    });
    see(&finder, &[FINDER], &[LIVE, SWITCH, QUIT_TITLE, API_LINE]);
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back. The narrow footer may omit a
/// size chip, so this waits on the terminal size. The picker stays on api dev.
fn nudge(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && on_api(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && on_api(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

/// `Dialog::destructive` focuses Stay. Right moves to Stop and quit, and
/// Enter confirms it. The paused clock never reaps the cancelling activities,
/// so this does not exit and must not call `finish`.
fn confirm_stop(app: &App) {
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit dialog", stay_focused);
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    see(&dialog, &[QUIT_TITLE, STOP, STAY, "still running"], &[]);
    assert!(
        stay_focused(&dialog),
        "Stay is not the focused control\n{}",
        text(&dialog)
    );
    app.key("right");
    let armed = app.wait_screen("stop and quit focused", stop_focused);
    assert!(
        stop_focused(&armed),
        "Stop and quit is not focused\n{}",
        text(&armed)
    );
    app.key("enter");
    let stopping = app.wait_screen("stopping", |screen| {
        let visible = text(screen);
        visible.contains(STOPPING) && !visible.contains(STOP)
    });
    see(
        &stopping,
        &[STOPPING, "quitting when every process is gone"],
        &[STOP, QUIT_TITLE],
    );
    assert!(
        app.child_running(),
        "paused motion reaped the activities and exited"
    );
}

#[test]
fn holla_picker_route_returns_to_the_finder() {
    let app = launch("h03-picker-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[FINDER, "~/work/acme"],
        &[LIVE, SWITCH, QUIT_TITLE],
    );
    open_picker(&app);
    back_to_finder(&app);
    confirm_stop(&app);
}

#[test]
fn holla_picker_opens_the_highlighted_activity() {
    let app = launch("h03-picker-picker");
    open_picker(&app);
    app.key("down");
    let picked = app.wait_screen("api dev highlighted", on_api);
    assert_eq!((picked.cols(), picked.rows()), (120, 40));
    see(
        &picked,
        &[TITLE, LIVE, FRONT, API],
        &[API_LINE, VITE, LISTEN],
    );
    assert!(
        on_api(&picked),
        "highlight stayed on the first activity\n{}",
        text(&picked)
    );
    nudge(&app);
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("picker at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && on_api(screen)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(&narrowed, &[TITLE, API], &[VITE, LISTEN]);
    nudge(&app);
    shot(&app, NARROW);
    app.resize(120, 40);

    let restored = app.wait_screen("picker back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && on_api(screen)
    });
    see(&restored, &[TITLE, API, LIVE], &[API_LINE]);

    app.key("enter");
    let opened = app.wait_screen("api dev output", |screen| {
        let visible = text(screen);
        visible.contains(API_LINE) && !visible.contains(LIVE) && !visible.contains(SWITCH)
    });
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    see(
        &opened,
        &[API_LINE, API, "port 8080", FRONT],
        &[VITE, FRONT_URL, FRONT_LINE, LISTEN, LIVE],
    );
    confirm_stop(&app);
}
