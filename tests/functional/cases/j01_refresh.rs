//! Jackin manager refresh, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j01_refresh/`. Refresh is F5. `returning` joins live
//! instances and `hard-cases` starts from an unreadable index, so neither
//! scenario plays the intro. The failure message is delivered on the tick
//! after F5 arms the refresh. The preserved-dirty description wraps inside
//! the detail card, so the proof uses the two fragments that each stay on one
//! painted line. The transient pending selection transition is checked
//! deterministically in Jackin's app test; this process test checks the
//! completed failure state.

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const WIDE: &str = "J01.refresh.refresh.120x40";
const NARROW: &str = "J01.refresh.refresh.narrow";
const REFRESHING: &str = "Refreshing…";
const FAILURE: &str = "Refresh failed: instance index unreadable · showing last-good rows";
const RUNNING_LIFE: &str = "Container and Capsule daemon are live.";
const DIRTY_WORD: &str = "preserved · dirty";
const DIRTY_HEAD: &str = "Stopped with uncommitted changes";
const DIRTY_TAIL: &str = "kept until you decide.";
const EMPTY: &str = "no instances";
const STALE: &str = "▲ stale";
const DAEMON_STALE: &str = "▲ daemon stale";
const KNOWN_IDS: &[&str] = &[
    "7f3a", "c41e", "9b02", "12ee", "a1c0", "04d7", "5e5e", "e0e0", "f1f1", "b000", "b111", "b222",
    "b333", "b444", "b555",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j01_refresh/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.clock = false;
    launch.history = true;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) {
    let manager = app.wait_text("Current directory");
    let visible = text(&manager);
    assert!(
        visible.contains("Workspaces"),
        "manager did not open\n{visible}"
    );
    assert!(
        !visible.contains("Stand up, operator") && !visible.contains("Knock, knock"),
        "intro is still up\n{visible}"
    );
}

fn expand_payments(app: &App) {
    app.key("down");
    app.key("right");
    app.wait_text("◉ 7f3a");
}

fn shows_running(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("7f3a ·") && visible.contains(RUNNING_LIFE) && !visible.contains(DIRTY_HEAD)
}

fn shows_dirty(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("c41e ·")
        && visible.contains(DIRTY_WORD)
        && visible.contains(DIRTY_HEAD)
        && visible.contains(DIRTY_TAIL)
        && !visible.contains(RUNNING_LIFE)
}

fn visible_ids(screen: &TerminalSnapshot) -> Vec<&'static str> {
    let visible = text(screen);
    KNOWN_IDS
        .iter()
        .copied()
        .filter(|id| visible.contains(id))
        .collect()
}

fn forbid_other_scenario(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for absent in ["parity", "vm.img", "842 of 926", EMPTY] {
        assert!(!visible.contains(absent), "forbidden {absent:?}\n{visible}");
    }
}

fn nudge(app: &App, quiet: impl Fn(&TerminalSnapshot) -> bool) {
    app.flash_expired();
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(quiet(&screen), "nudge left the frame\n{}", text(&screen));
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && quiet(screen)
    });
}

fn quiet_failure(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains("c41e")
        && visible.contains(DIRTY_WORD)
        && visible.contains("7f3a")
        && visible.contains(FAILURE)
        && visible.contains("Workspaces")
        && !visible.contains(EMPTY)
        && !visible.contains("parity")
        && !visible.contains("vm.img")
        && !visible.contains("842 of 926")
        && !visible.contains(RUNNING_LIFE)
        && !visible.contains(" s ago")
        && !visible.contains("Stand up, operator")
        && !visible.contains("Knock, knock")
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

/// Success path. F5 paints `Refreshing…`, the selection moves to another
/// instance while that status is up, and the tick keeps both rows.
fn prove_success(app: &App) {
    reach_manager(app);
    expand_payments(app);
    app.key("down");
    let first = app.wait_screen("7f3a before refresh", shows_running);
    let before = text(&first);
    assert!(
        before.contains("c41e"),
        "c41e was not in the list before refresh\n{before}"
    );
    assert!(!before.contains(FAILURE), "failure before F5\n{before}");
    assert!(
        !before.contains(STALE),
        "stale on the healthy scenario\n{before}"
    );
    assert!(
        !before.contains(DAEMON_STALE),
        "daemon stale on the healthy scenario\n{before}"
    );
    forbid_other_scenario(&first);

    app.key("f5");
    let refreshing = app.wait_text(REFRESHING);
    let visible = text(&refreshing);
    assert!(
        shows_running(&refreshing) && visible.contains("Workspaces"),
        "refresh left the manager\n{visible}"
    );
    assert!(!visible.contains(FAILURE), "failure on success\n{visible}");
    assert!(
        !visible.contains(EMPTY),
        "list cleared on refresh\n{visible}"
    );
    assert!(!visible.contains(STALE) && !visible.contains(DAEMON_STALE));

    app.key("down");
    app.wait_screen("other instance while refreshing", |screen| {
        let visible = text(screen);
        visible.contains(REFRESHING)
            && shows_dirty(screen)
            && !visible.contains(FAILURE)
            && !visible.contains(EMPTY)
            && visible.contains("7f3a")
    });
    // The armed refresh runs on the following tick. Success keeps the rows.
    std::thread::sleep(Duration::from_millis(400));
    let held = app.wait_screen("success refresh held", |screen| {
        let visible = text(screen);
        visible.contains(REFRESHING)
            && shows_dirty(screen)
            && !visible.contains(FAILURE)
            && !visible.contains(EMPTY)
            && !visible.contains(STALE)
            && !visible.contains(DAEMON_STALE)
            && visible.contains("7f3a")
            && visible.contains("Workspaces")
    });
    forbid_other_scenario(&held);

    app.key("up");
    let restored = app.wait_screen("7f3a after refresh", |screen| {
        let visible = text(screen);
        shows_running(screen)
            && !visible.contains(DIRTY_WORD)
            && !visible.contains(FAILURE)
            && !visible.contains(EMPTY)
            && !visible.contains(STALE)
            && visible.contains("c41e")
            && visible.contains("Workspaces")
    });
    forbid_other_scenario(&restored);
}

fn prove_failure(app: &App) {
    reach_manager(app);
    expand_payments(app);
    app.key("down");
    let first = app.wait_screen("7f3a before failed refresh", shows_running);
    let before = text(&first);
    assert!(
        before.contains("data-pipeline"),
        "hard-cases did not show the extra workspace\n{before}"
    );
    assert!(!before.contains(FAILURE), "failure before F5\n{before}");
    let ids = visible_ids(&first);
    assert!(
        ids.contains(&"7f3a") && ids.contains(&"c41e"),
        "expected both instance ids before F5, saw {ids:?}\n{before}"
    );
    forbid_other_scenario(&first);

    // The process-level assertion is independent of whether the PTY observes
    // the short-lived pending frame. The exact pending transition is covered
    // by a deterministic app test.
    app.key("down");
    app.wait_screen("dirty instance before failed refresh", shows_dirty);
    app.key("f5");
    let failed = app.wait_text(FAILURE);
    let visible = text(&failed);
    assert!(
        shows_dirty(&failed),
        "failure rewrote the selected lifecycle\n{visible}"
    );
    assert!(
        !visible.contains(EMPTY),
        "failure cleared the list\n{visible}"
    );
    assert!(
        visible.contains("Workspaces") && visible.contains("data-pipeline"),
        "failure left the manager\n{visible}"
    );
    for id in &ids {
        assert!(
            visible.contains(id),
            "instance {id} left the list after the failed refresh\n{visible}"
        );
    }
    forbid_other_scenario(&failed);
    assert!(quiet_failure(&failed), "quiet failure frame\n{visible}");

    nudge(app, quiet_failure);
    shot(app, WIDE);
    app.resize(80, 24);
    app.wait_screen("narrow failure", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && quiet_failure(screen)
    });
    shot(app, NARROW);
    app.resize(120, 40);
    app.wait_screen("wide restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && quiet_failure(screen)
    });
}

#[test]
fn j01_refresh_route() {
    let app = launch("j01-refresh-route", "returning");
    prove_success(&app);
    app.finish();
}

#[test]
fn j01_refresh_refresh() {
    let success = launch("j01-refresh-success", "returning");
    prove_success(&success);
    success.finish();

    let app = launch("j01-refresh-failure", "hard-cases");
    prove_failure(&app);
    app.finish();
}
