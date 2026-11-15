//! Holla upgrade plan, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h04_plan/` and each checkpoint's dots become underscores.
//! The shot is the open `Start upgrade everything?` dialog, before Start.
//! Reduced motion keeps `clock.running` true. The running plan tab is not shot:
//! step output changes while the clock runs.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const TITLE: &str = "Upgrade everything";
const DIALOG: &str = "Start upgrade everything?";
const HOST: &str = "devbox";
const INTENT: &str = "upgrade Debian packages and global mise tools on devbox";
const FINDER: &str = "Upgrade everything on this system";
const DEBIAN: &str = "14 upgradable · 2 security";
const MISE_FACT: &str = "4 outdated global tools · go excluded";
const REBOOT: &str = "required after the kernel upgrade";
const FOLLOW: &str = "Outdated tools";
const REQUIRED: &str = "01 Preflight is required by the plan and cannot be excluded";
const EXCLUDED: &str = "06 excluded · 08 now blocked";
const RESTORED: &str = "06 restored";
const CLEAN_EXCLUDED: &str = "07 excluded · nothing depends on it";
const NOTHING: &str = "Nothing to run: every step is excluded";
const DISCARD: &str = "Plan discarded · nothing was executed";
const CANCELLED_GATE: &str = "Cancelled · nothing was executed";
const STARTED: &str = "Upgrade everything started";
const DONE: &str = "finished · 6 ok · 1 failed";
const RETRY: &str = "06 retrying · succeeded work is not repeated";
const CANCEL_TITLE: &str = "Cancel the remaining steps?";
const KEEP: &str = "Keep running";
const CANCEL_REMAINING: &str = "Cancel remaining";
const CANCELLED_RUN: &str = "Remaining steps cancelled · running steps finish";
const APT_CMD: &str = "$ sudo apt-get update";
const MISE_CMD: &str = "$ mise upgrade --exclude go";
const FAIL_LINE: &str = "exit status 1";
const WIDE: &str = "H04.plan.plan.120x40";
const NARROW: &str = "H04.plan.plan.narrow";
const RUN: Duration = Duration::from_secs(25);

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h04_plan/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec!["--scenario".to_string(), "upgrade-plan".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(TITLE);
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

fn wait_for(
    app: &App,
    reason: &str,
    timeout: Duration,
    predicate: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + timeout;
    loop {
        assert!(app.child_running(), "exited while {reason}");
        let screen = app.screen();
        if predicate(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline,
            "timed out: {reason}\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn foreign(screen: &TerminalSnapshot) {
    see(
        screen,
        &[],
        &[
            "prod-eu-1",
            "tmp-shell-3",
            "parity-git-batch",
            "~/work/repos",
            "REMOVE ALL DOCKER DATA",
            "Plan changed",
            "I UNDERSTAND",
            NOTHING,
            STARTED,
        ],
    );
}

/// The 140ms press flash stays painted while motion is reduced. A same-size
/// resize does not force a new frame, so the shot nudges the width and comes
/// back. The waits follow the reported size, not the size text.
fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        text(&screen).contains(DIALOG),
        "shot left the start dialog\n{}",
        text(&screen)
    );
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge", |screen| {
        screen.cols() == u32::from(nudged)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(DIALOG)
    });
    app.resize(cols, rows);
    app.wait_screen("restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && text(screen).contains(DIALOG)
    });
}

fn shot(app: &App, checkpoint: &str) {
    flash_settle(app);
    app.visual(&visual_id(checkpoint));
}

fn repeat(app: &App, key: &str, n: usize) {
    for _ in 0..n {
        app.key(key);
    }
}

fn open_from_finder(app: &App) {
    app.paste(FINDER);
    // The Does panel wraps the intent across rows, so the wait uses the
    // single-row preview title and the Debian count.
    app.wait_screen("finder lists the upgrade plan", |screen| {
        let visible = text(screen);
        visible.contains(FINDER) && visible.contains("14 upgradable")
    });
    app.key("enter");
    app.wait_text(INTENT);
}

fn leave(app: &App) {
    app.key("ctrl+q");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if !app.child_running() {
            app.expect_exited(0);
            return;
        }
        let visible = text(&app.screen());
        assert!(
            !visible.contains("Stop and quit"),
            "quiet plan raised a running quit\n{visible}"
        );
        if visible.contains("Quit holla") {
            app.key("enter");
            let exit_by = Instant::now() + Duration::from_secs(8);
            while app.child_running() {
                assert!(
                    Instant::now() < exit_by,
                    "confirmed quit did not exit\n{}",
                    text(&app.screen())
                );
                std::thread::sleep(Duration::from_millis(25));
            }
            app.expect_exited(0);
            return;
        }
        assert!(Instant::now() < deadline, "quit did not exit\n{visible}");
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn assert_dialog(screen: &TerminalSnapshot, cols: u32, rows: u32) {
    assert_eq!(
        (screen.cols(), screen.rows()),
        (cols, rows),
        "{}",
        text(screen)
    );
    see(
        screen,
        &[DIALOG, "Start plan", "Cancel", HOST],
        &[
            STARTED,
            "gate 2 of 2",
            "Type ",
            "REMOVE ALL DOCKER DATA",
            "tmp-shell-3",
            "Plan changed",
            "I UNDERSTAND",
            NOTHING,
        ],
    );
    if cols >= 100 {
        // The Plan fact wraps the intent, so the wide check uses the one-line facts.
        see(
            screen,
            &[
                "upgrade Debian packages and global",
                DEBIAN,
                MISE_FACT,
                REBOOT,
            ],
            &[],
        );
        let gutter = text(screen).matches('▎').count();
        assert_eq!(gutter, 1, "dialog focus gutter\n{}", text(screen));
        let row = (0..screen.rows())
            .map(|index| row_text(screen, index))
            .find(|line| line.contains('▎'))
            .expect("gutter row");
        assert!(
            row.contains("Cancel") && row.find('▎') < row.find("Cancel"),
            "Cancel is not the focused action\n{row}"
        );
    }
}

fn occurrences(screen: &TerminalSnapshot, needle: &str) -> usize {
    text(screen).matches(needle).count()
}

#[test]
fn h04_plan_route() {
    let app = open("h04-plan-route");
    let landed = app.screen();
    assert_eq!((landed.cols(), landed.rows()), (120, 40));
    see(
        &landed,
        &[TITLE, HOST, INTENT, "Confirm plan", "Cancel", "8 steps"],
        &[FINDER, STARTED, DISCARD],
    );
    // Esc is ignored by `PlanReviewPage::on_key` and is shell Back.
    app.key("esc");
    let finder = app.wait_text("Search actions and resources");
    see(&finder, &[HOST], &[DISCARD, STARTED, INTENT, DIALOG]);
    open_from_finder(&app);
    let review = app.wait_screen("review from the finder", |screen| {
        let visible = text(screen);
        visible.contains(TITLE) && visible.contains(HOST) && visible.contains(INTENT)
    });
    see(
        &review,
        &["Confirm plan", "Cancel", "8 steps"],
        &[STARTED, DISCARD, "prod-eu-1", "tmp-shell-3"],
    );
    app.key("esc");
    let back = app.wait_screen("finder keeps the upgrade row", |screen| {
        let visible = text(screen);
        visible.contains(FINDER) && !visible.contains("Confirm plan")
    });
    see(&back, &[FINDER], &[DISCARD, STARTED, DIALOG, INTENT]);
    leave(&app);
}

#[test]
fn h04_plan_plan() {
    let app = open("h04-plan-dialog");
    let review = app.wait_text("8 steps · 8 included");
    see(
        &review,
        &[
            TITLE,
            HOST,
            INTENT,
            "Preflight",
            "Upgrade selected mise tools",
            "Package cleanup",
            "Final verification",
            "Confirm plan",
            "Cancel",
            "6 required",
        ],
        &[
            STARTED,
            DIALOG,
            FOLLOW,
            "prod-eu-1",
            "tmp-shell-3",
            "~/work/repos",
            "REMOVE ALL DOCKER DATA",
        ],
    );

    app.key("c");
    let wide = app.wait_screen("start dialog", |screen| {
        let visible = text(screen);
        screen.cols() == 120
            && screen.rows() == 40
            && visible.contains(DIALOG)
            && visible.contains("Start plan")
            && visible.contains(DEBIAN)
    });
    assert_dialog(&wide, 120, 40);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("start dialog narrow", |screen| {
        let visible = text(screen);
        screen.cols() == 80
            && screen.rows() == 24
            && visible.contains(DIALOG)
            && visible.contains("Start plan")
    });
    assert_dialog(&narrow, 80, 24);
    shot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("start dialog wide again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(DIALOG)
    });
    // Cancel starts focused (`confirm_plan` sets `initial_focus` to `actions[0]`).
    // The dialog title leaves before "Cancelled" paints, and Start plan can remain.
    app.key("enter");
    let dismissed = app.wait_screen("dialog cancelled", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED_GATE)
            && visible.contains("8 steps · 8 included")
            && visible.contains("Confirm plan")
            && !visible.contains(DIALOG)
            && !visible.contains("Start plan")
            && !visible.contains(STARTED)
            && !visible.contains("gate 2 of 2")
    });
    see(
        &dismissed,
        &[CANCELLED_GATE, "8 steps · 8 included", "Confirm plan"],
        &[STARTED, "gate 2 of 2"],
    );

    app.key("space");
    let refused = app.wait_text(REQUIRED);
    see(&refused, &["8 included"], &[EXCLUDED, NOTHING, STARTED]);

    repeat(&app, "down", 5);
    app.key("space");
    let excluded = app.wait_text(EXCLUDED);
    see(
        &excluded,
        &["7 included", "Final verification"],
        &[NOTHING, STARTED],
    );
    app.key("u");
    let restored = app.wait_text(RESTORED);
    see(&restored, &["8 included"], &[EXCLUDED, NOTHING]);

    app.key("down");
    app.key("space");
    app.wait_text(CLEAN_EXCLUDED);
    app.key("up");
    app.key("space");
    let both = app.wait_text(EXCLUDED);
    see(&both, &["6 included", "2 excluded"], &[NOTHING, STARTED]);
    app.key("c");
    let still = app.wait_screen("required steps still confirm", |screen| {
        let visible = text(screen);
        visible.contains(DIALOG) && visible.contains("Start plan")
    });
    see(
        &still,
        &["6 included", "2 excluded", "Start plan"],
        &[NOTHING, STARTED, "gate 2 of 2"],
    );
    app.key("esc");
    app.wait_screen("second confirm dismissed", |screen| {
        let visible = text(screen);
        !visible.contains(DIALOG) && !visible.contains("Start plan")
    });
    app.key("u");
    app.wait_text(RESTORED);
    app.key("u");
    let full = app.wait_text("8 included");
    see(&full, &["0 excluded"], &[NOTHING, DIALOG, STARTED]);

    // `x` discards. Esc on the outline does not (`on_key` ignores it).
    app.key("x");
    let popped = app.wait_text(DISCARD);
    see(
        &popped,
        &["Search actions and resources"],
        &[STARTED, DIALOG, INTENT],
    );

    open_from_finder(&app);
    app.wait_text("8 steps · 8 included");
    app.key("enter");
    app.key("enter");
    app.wait_screen("start dialog before run", |screen| {
        let visible = text(screen);
        visible.contains(DIALOG) && visible.contains("Start plan")
    });
    app.key("right");
    app.key("enter");
    wait_for(&app, "plan started", Duration::from_secs(3), |screen| {
        text(screen).contains(STARTED)
    });
    let failed = wait_for(&app, "mise step failed", RUN, |screen| {
        let visible = text(screen);
        visible.contains(DONE) && visible.contains(FAIL_LINE)
    });
    see(
        &failed,
        &[TITLE, FOLLOW, "Upgrade selected mise tools", MISE_CMD],
        &["prod-eu-1", "tmp-shell-3", "Plan changed", CANCEL_TITLE],
    );
    assert_eq!(
        occurrences(&failed, MISE_CMD),
        1,
        "mise command before retry\n{}",
        text(&failed)
    );

    repeat(&app, "up", 4);
    let apt = app.wait_text(APT_CMD);
    assert_eq!(
        occurrences(&apt, APT_CMD),
        1,
        "apt command before retry\n{}",
        text(&apt)
    );
    repeat(&app, "down", 4);
    app.wait_text(FAIL_LINE);
    app.key("r");
    wait_for(&app, "retry status", Duration::from_secs(3), |screen| {
        text(screen).contains(RETRY)
    });

    app.key("ctrl+c");
    let ask = app.wait_text(CANCEL_TITLE);
    // The destructive body wraps inside the dialog, so the check is the two painted rows.
    see(
        &ask,
        &[
            "Running steps finish; ready and waiting steps",
            "are cancelled. Completed work stays.",
            KEEP,
            CANCEL_REMAINING,
        ],
        &[],
    );
    app.key("enter");
    let kept = app.wait_screen("keep running", |screen| {
        let visible = text(screen);
        !visible.contains(CANCEL_TITLE)
            && (visible.contains(" running") || visible.contains(MISE_CMD))
    });
    see(&kept, &[TITLE, MISE_CMD], &[CANCELLED_RUN, DISCARD]);

    app.key("ctrl+c");
    app.wait_text(CANCEL_TITLE);
    app.key("right");
    app.key("enter");
    let cancelled = wait_for(
        &app,
        "remaining cancelled",
        Duration::from_secs(3),
        |screen| text(screen).contains(CANCELLED_RUN),
    );
    see(
        &cancelled,
        &[TITLE, "cancelled"],
        &[DISCARD, "Upgrade everything finished"],
    );

    repeat(&app, "up", 4);
    let apt_after = app.wait_text(APT_CMD);
    assert_eq!(
        occurrences(&apt_after, APT_CMD),
        1,
        "succeeded apt command was repeated\n{}",
        text(&apt_after)
    );
    repeat(&app, "down", 4);
    let mise_after = app.wait_text("retrying");
    assert_eq!(
        occurrences(&mise_after, MISE_CMD),
        1,
        "mise command was repeated\n{}",
        text(&mise_after)
    );
    assert_eq!(
        occurrences(&mise_after, FAIL_LINE),
        1,
        "{}",
        text(&mise_after)
    );
    see(
        &mise_after,
        &["cancelled", "Final verification"],
        &["prod-eu-1", "tmp-shell-3", "~/work/repos"],
    );
    leave(&app);
}

#[test]
fn h04_plan_scenario_upgrade_plan() {
    let app = open("h04-plan-scenario");
    let screen = app.wait_screen("upgrade-plan review", |screen| {
        let visible = text(screen);
        visible.contains(HOST) && visible.contains(INTENT)
    });
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    foreign(&screen);
    see(
        &screen,
        &[TITLE, "Preflight", "Confirm plan"],
        &["~/work/holla", "~/work/svc", "Clean Docker completely"],
    );
    app.key("c");
    let dialog = app.wait_text(DIALOG);
    see(
        &dialog,
        &[DEBIAN, MISE_FACT, REBOOT, HOST, "Start plan"],
        &[STARTED, "tmp-shell-3", "prod-eu-1", "~/work/repos"],
    );
    app.key("esc");
    let back = app.wait_screen("dialog dismissed", |screen| !text(screen).contains(DIALOG));
    see(&back, &[TITLE, INTENT], &[STARTED]);
    leave(&app);
}
