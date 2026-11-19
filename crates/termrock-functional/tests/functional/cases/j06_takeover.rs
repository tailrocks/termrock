//! Jackin attach takeover, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_takeover/`. The shots are the takeover overlay on
//! `jk-7f3a`, before Enter and before Esc. They do not nudge. `Launch::new`
//! leaves the clock off. `App::new` would enable it for jackin.
//!
//! Hard-cases emits one `Takeover` when sim time reaches 45_000 ms
//! (`World::tick` in `src/bin/jackin/sim/world.rs`). Reduced motion ticks the
//! manager and the capsule by 80 ms and advances the sim clock by that same
//! interval (`Route::tick_ms` in `src/bin/jackin/app.rs`). The functional
//! profile ends a test after 90s, so the stale session and the overlay
//! session run together. Each is still a fresh process.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const TARGET: &str = "7f3a";
const OTHER: &str = "9b02";
const IDENTITY: &str = "payments-platform › the-architect";
const INFRA_IDENTITY: &str = "infra-control-plane › sre";
const TAKEN: &str = "Attach taken over";
const AGO: &str = "tty004 · MacBook attached to payments-platform 3 s ago.";
const KEEPS: &str = "The instance, tabs and panes keep running.";
const RECONNECTED: &str = "Reconnected · the other client was displaced";
const DETACHED: &str = "Detached · payments-platform keeps running";
const DETAIL: &str = "7f3a · payments-platform";
const PAGE: &str = "Workspaces";
const CURRENT: &str = "Current directory";
const OUTRO: &str = "You were in the Construct";
const WIDE: &str = "J06.takeover.takeover.120x40";
const NARROW: &str = "J06.takeover.takeover.narrow";

/// Wall time after the first frame. The sim clock is still near zero there,
/// and 50s covers the 45_000 ms deadline with the 80 ms tick.
const SIM_HOLD: Duration = Duration::from_secs(50);

/// `StatusLine::DEFAULT_TTL_MS` is 5_000. The overlay copy does not move.
/// Waiting this long lets the footer status expire before a shot.
const STATUS_CLEAR: Duration = Duration::from_secs(6);

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_takeover/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    open_motion(name, "reduced")
}

fn open_motion(name: &str, motion: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "hard-cases".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some(motion.to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

/// The tree side of the focused row. The detail pane shares that terminal row.
fn tree_focus(screen: &TerminalSnapshot) -> String {
    focused_line(screen).chars().take(32).collect()
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn manager_up(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(CURRENT)
        && visible.contains(PAYMENTS)
        && visible.contains(INFRA)
        && !visible.contains(OUTRO)
        && !visible.contains(TAKEN)
}

fn overlay_screen(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    screen.cols() == cols
        && screen.rows() == rows
        && visible.contains(TAKEN)
        && visible.contains(AGO)
        && visible.contains(KEEPS)
        && visible.contains("Reconnect")
        && !visible.contains(PAGE)
        && !visible.contains(OUTRO)
}

fn wide_overlay(screen: &TerminalSnapshot) -> bool {
    overlay_screen(screen, 120, 40)
}

/// Below `TooSmall`'s 72×20 minimum the shell paints that notice instead of
/// the capsule. Ticks still run, and that frame is only a few cells, so sim
/// time reaches the 45 s deadline. The overlay is read after growing back.
const CHEAP_COLS: u16 = 40;
const CHEAP_ROWS: u16 = 12;
const CHEAP_HOLD: Duration = Duration::from_secs(50);

/// Watch the overlay, then return the wide frame. The budget covers the
/// cheap wait and both grows, so a late paint cannot add a second full wait
/// on top of the profile kill.
fn wait_takeover_wide(app: &App, budget: Duration) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    let lift_at = Instant::now() + CHEAP_HOLD;
    app.resize(CHEAP_COLS, CHEAP_ROWS);
    let mut lifted = false;
    let mut grew = false;
    loop {
        let screen = app.screen();
        if wide_overlay(&screen) {
            return screen;
        }
        let visible = text(&screen);
        if !grew && narrow_overlay(&screen) {
            app.resize(120, 40);
            grew = true;
        } else if !lifted && Instant::now() >= lift_at {
            app.resize(80, 24);
            lifted = true;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "takeover overlay not seen\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn narrow_overlay(screen: &TerminalSnapshot) -> bool {
    overlay_screen(screen, 80, 24)
}

fn wait_for(
    app: &App,
    reason: &str,
    budget: Duration,
    mut also: impl FnMut(),
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    loop {
        also();
        let screen = app.screen();
        if accept(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} not seen\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_settled(
    app: &App,
    reason: &str,
    hold: Duration,
    budget: Duration,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    let mut last: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let same = last.as_ref().is_some_and(|prev| prev == &screen);
            if same {
                if since.expect("stability start").elapsed() >= hold {
                    return screen;
                }
            } else {
                last = Some(screen);
                since = Some(Instant::now());
            }
        } else {
            last = None;
            since = None;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} frame not seen\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn select_workspace(app: &App, workspace: &str) {
    let first = app.wait_screen("manager", manager_up);
    let line = tree_focus(&first);
    assert!(
        line.contains(CURRENT),
        "initial selection is not Current directory\n{line}\n{}",
        text(&first)
    );
    app.key("down");
    app.wait_screen("payments-platform row", |screen| {
        let focus = tree_focus(screen);
        focus.contains(PAYMENTS) && !focus.contains(TARGET) && !focus.contains(OTHER)
    });
    if workspace == INFRA {
        app.key("down");
        app.wait_screen("infra-control-plane row", |screen| {
            let focus = tree_focus(screen);
            focus.contains(INFRA) && !focus.contains(TARGET) && !focus.contains(OTHER)
        });
    }
    let focus = tree_focus(&app.screen());
    assert!(
        focus.contains(workspace) && !focus.contains('▾'),
        "workspace {workspace} is not collapsed\n{focus}\n{}",
        text(&app.screen())
    );
    app.key("right");
    app.wait_screen("expanded workspace", |screen| {
        let focus = tree_focus(screen);
        focus.contains('▾') && focus.contains(workspace)
    });
}

fn select_instance(app: &App, workspace: &str, id: &str) {
    select_workspace(app, workspace);
    if tree_focus(&app.screen()).contains(id) {
        return;
    }
    app.key("down");
    app.wait_screen("instance row", |screen| tree_focus(screen).contains(id));
    let focus = tree_focus(&app.screen());
    assert!(
        focus.contains(id) && !focus.contains(if id == TARGET { OTHER } else { TARGET }),
        "focused row is not {id}\n{focus}\n{}",
        text(&app.screen())
    );
}

fn attach_payments(app: &App) -> Instant {
    let started = app.wait_screen("manager", manager_up);
    let _ = started;
    let clock = Instant::now();
    select_instance(app, PAYMENTS, TARGET);
    app.key("enter");
    app.wait_screen("attached payments-platform", |screen| {
        let visible = text(screen);
        visible.contains(IDENTITY)
            && visible.contains("Attached to payments-platform")
            && !visible.contains(TAKEN)
            && !visible.contains(PAGE)
            && !visible.contains(INFRA)
    });
    clock
}

fn attach_infra(app: &App) -> Instant {
    let _ = app.wait_screen("manager", manager_up);
    let clock = Instant::now();
    select_instance(app, INFRA, OTHER);
    app.key("enter");
    app.wait_screen("attached infra-control-plane", |screen| {
        let visible = text(screen);
        visible.contains(INFRA_IDENTITY)
            && visible.contains("Attached to infra-control-plane")
            && !visible.contains(TAKEN)
            && !visible.contains(PAGE)
            && !visible.contains(PAYMENTS)
    });
    clock
}

fn guard_stale(stale: &App, polls: &mut u32) {
    assert!(stale.child_running(), "stale process quit");
    let screen = stale.screen();
    let visible = text(&screen);
    assert!(
        !visible.contains(TAKEN),
        "stale takeover painted the overlay\n{visible}"
    );
    assert!(
        !visible.contains(PAYMENTS),
        "stale takeover switched to payments-platform\n{visible}"
    );
    assert!(
        !visible.contains(TARGET),
        "stale takeover showed 7f3a\n{visible}"
    );
    // 80 columns clip the menubar identity "infra-control-plane › sre".
    // The pane still names the workspace. A blank resize frame is not a
    // route change, so it does not count as a held sample.
    let on_infra = if screen.cols() >= 120 {
        visible.contains(INFRA) && visible.contains(INFRA_IDENTITY)
    } else {
        visible.contains(INFRA)
    };
    if !on_infra {
        return;
    }
    *polls += 1;
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let screen = wait_settled(
        app,
        &test_id,
        Duration::from_millis(200),
        Duration::from_secs(8),
        accept,
    );
    crate::support::assert_frame(app, &screen, &test_id);
}

#[test]
fn j06_takeover_route() {
    let app = open("j06-takeover-route");
    // Consume the hard-cases fixture's one-shot unreadable-index refresh
    // before attaching. Otherwise its later error status can race and replace
    // the explicit Detached status after Esc, even though detach succeeded.
    let refresh_error = app.wait_screen("initial unreadable index is reported", |screen| {
        text(screen).contains("Refresh failed: instance index unreadable · showing last-good rows")
    });
    require(
        &refresh_error,
        "Refresh failed: instance index unreadable · showing last-good rows",
    );
    let _clock = attach_payments(&app);
    assert!(
        _clock.elapsed() < Duration::from_secs(15),
        "payments attach was too late to catch the takeover"
    );
    // 84 s still finishes inside the 90 s profile kill. The waiter holds a
    // 40×12 grid for 50 s, then restores the shot sizes.
    let overlay = wait_takeover_wide(&app, Duration::from_secs(84));
    require(&overlay, TAKEN);
    require(&overlay, AGO);
    require(&overlay, KEEPS);
    refuse(&overlay, PAGE);
    refuse(&overlay, OUTRO);

    app.key("esc");
    let manager = app.wait_screen("left takeover", |screen| {
        let visible = text(screen);
        let focus = tree_focus(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(PAGE)
            && visible.contains(DETACHED)
            && visible.contains(DETAIL)
            && focus.contains(TARGET)
            && !focus.contains(OTHER)
            && !visible.contains(TAKEN)
            && !visible.contains(OUTRO)
            && !visible.contains("7f3a · infra-control-plane")
    });
    require(&manager, DETACHED);
    require(&manager, DETAIL);
    require(&manager, PAYMENTS);
    refuse(&manager, TAKEN);
    refuse(&manager, OUTRO);
    assert!(
        tree_focus(&manager).contains(TARGET),
        "leave selected another row\n{}",
        text(&manager)
    );
    assert!(app.child_running(), "leave quit the process");
}

#[test]
fn j06_takeover_takeover() {
    // Both clients stay on reduced motion. A wide redraw costs more than the
    // 80 ms tick, so each session is narrowed before the 45 s deadline.
    let stale = open("j06-takeover-stale");
    let stale_clock = attach_infra(&stale);
    assert!(
        stale_clock.elapsed() < Duration::from_secs(15),
        "infra attach was too late to watch the takeover deadline"
    );

    // The stale capsule stays at 80×24. Its menubar clips the identity line
    // there, and the pane still names the workspace. The overlay client is
    // resized to the cheap grid after it attaches.
    let mut polls = 0u32;
    stale.resize(80, 24);

    let overlay = open("j06-takeover-overlay");
    select_instance(&overlay, PAYMENTS, TARGET);
    assert!(
        stale_clock.elapsed() < Duration::from_secs(20),
        "stale polling started after the takeover could already have fired"
    );
    overlay.key("enter");
    wait_for(
        &overlay,
        "attached payments-platform",
        Duration::from_secs(8),
        || {},
        |screen| {
            let visible = text(screen);
            visible.contains(IDENTITY)
                && !visible.contains(TAKEN)
                && !visible.contains(PAGE)
                && !visible.contains(INFRA)
        },
    );

    // Same tick stall as the route. The overlay stays below the minimum size
    // until sim time has passed the deadline, then the narrow shot is 80×24.
    // The stale client stays at 80×24 so its pane still names the workspace.
    // Sample it on this same cadence so the held-infra count covers the wait.
    overlay.resize(CHEAP_COLS, CHEAP_ROWS);
    let lift_at = Instant::now() + CHEAP_HOLD;
    let mut lifted = false;
    let mut overlay_seen: Option<Instant> = None;
    // A loaded partition paints this overlay with the attach status still up
    // as an 80 s budget expires. The status lasts 5 s, and the shot needs
    // that line gone, so the wait is the late paint plus the 6 s clear.
    // 87 s still returns before the 90 s profile kill.
    let budget = Instant::now() + Duration::from_secs(87);
    loop {
        guard_stale(&stale, &mut polls);
        let screen = overlay.screen();
        let visible = text(&screen);
        if !lifted && Instant::now() >= lift_at {
            overlay.resize(80, 24);
            lifted = true;
        }
        if narrow_overlay(&screen) {
            if overlay_seen.is_none() {
                overlay_seen = Some(Instant::now());
            }
        } else {
            // The dialog is one paint, and a read can return the title before
            // the body. That frame already says the attach was taken over, so
            // it is not the payments session. Keep polling until the full copy
            // holds. A frame with no takeover copy still has to be that session.
            if !visible.contains(TAKEN) {
                // The opening title leaves the menubar once the PR status is
                // drawn. The shell prompt keeps the workspace name. The first
                // narrow paint can be blank, and that frame is not a route change.
                if (screen.cols() == 80 || screen.cols() == u32::from(CHEAP_COLS))
                    && visible.contains(PAYMENTS)
                {
                    assert!(
                        !visible.contains(INFRA)
                            && !visible.contains(PAGE)
                            && !visible.contains(OUTRO),
                        "overlay session left payments-platform before the takeover\n{visible}"
                    );
                }
                overlay_seen = None;
            }
        }
        let held = stale_clock.elapsed() >= SIM_HOLD;
        let cleared = overlay_seen.is_some_and(|seen| seen.elapsed() >= STATUS_CLEAR);
        if held && cleared {
            break;
        }
        assert!(
            Instant::now() < budget && overlay.child_running() && stale.child_running(),
            "takeover did not settle\nstale elapsed {:?}\n{}",
            stale_clock.elapsed(),
            text(&overlay.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        polls >= 200,
        "stale hold polled {polls} frames, not the whole wait"
    );
    guard_stale(&stale, &mut polls);

    overlay.resize(120, 40);
    shot(&overlay, WIDE, wide_overlay);
    overlay.resize(80, 24);
    shot(&overlay, NARROW, narrow_overlay);
    overlay.resize(120, 40);
    let restored = wait_settled(
        &overlay,
        "wide overlay restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        wide_overlay,
    );
    require(&restored, TAKEN);
    require(&restored, AGO);
    refuse(&restored, PAGE);

    overlay.key("enter");
    let again = wait_for(
        &overlay,
        "reconnected",
        Duration::from_secs(8),
        || {},
        |screen| {
            let visible = text(screen);
            visible.contains(RECONNECTED)
                && visible.contains(IDENTITY)
                && !visible.contains(TAKEN)
                && !visible.contains(INFRA)
                && !visible.contains(PAGE)
                && !visible.contains(OUTRO)
        },
    );
    require(&again, RECONNECTED);
    require(&again, IDENTITY);
    refuse(&again, TAKEN);
    refuse(&again, INFRA);
    refuse(&again, OUTRO);
    guard_stale(&stale, &mut polls);
    assert!(overlay.child_running(), "reconnect quit the process");
    assert!(stale.child_running(), "the stale process quit");
}
