//! Holla cleanup confirmation gates, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h06_gate/` and each checkpoint's dots become underscores.
//! `gradle.clean-all` is labeled `Clean Gradle outputs under ~/work/android`
//! plus a size the row clips. The paste is that prefix. The crumb is
//! `Cleanup › gradle.clean-all`. `d` pushes gate 1. Backspace does too, so
//! this leaf does not send it. macOS field keys select the phrase with
//! `ctrl+l`.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, row_text, text, try_launch};

const GRADLE_PREFIX: &str = "Clean Gradle outputs under ~/work/android";
const IDEA_PREFIX: &str = "Clean IntelliJ metadata under ~/work/ide";
const CRUMB: &str = "Here › Cleanup › gradle.clean-all";
const IDEA_CRUMB: &str = "Here › Cleanup › idea.clean";
const GATE1: &str = " · gate 1 of 2 · review the resolved facts, then continue deliberately";
const GATE2: &str = " · gate 2 of 2";
const TRASH_TITLE: &str = "Trash 4 items";
const PERM_TITLE: &str = "Permanently delete 4 items";
const DRY_TITLE: &str = "Dry run 4 items";
const PHRASE: &str = "TRASH 4 UNDER /Users/alex/work/android ON mbp";
const PERM_PHRASE: &str = "PERMANENTLY DELETE 4 UNDER /Users/alex/work/android ON mbp";
const PREREQ: &str = "gradle --stop first · a failed or unknown stop cancels the cleanup";
const DRY_PREREQ: &str = "gradle --stop is not run on a dry run";
const EMPTY: &str = "Select at least one eligible candidate first · Space selects";
const CANCELLED: &str = "Cancelled · nothing was executed";
const MISMATCH: &str = "Does not match";
const STARTED: &str = "cleanup started · 4 items · quitting waits for it";
const STILL_RUNNING: &str = "a cleanup is still running · wait for its report";
const STOP_FAILED: &str = "gradle --stop failed";
const CONTINUE: &str = "Continue…";
const WIDE: &str = "H06.gate.confirm.120x40";
const NARROW: &str = "H06.gate.confirm.narrow";

const FOREIGN: &[&str] = &["vm.img", "~/work/big", "sparse.img", "842 of 926"];
const IDEA_IDS: &[&str] = &[".idea", "workspace.xml", "app.iml", "~/work/ide"];

/// Ctrl+Q opens the quit guard while a cleanup job or a gate is up.
fn quit_blocked(visible: &str) -> bool {
    visible.contains("gate 1 of 2")
        || visible.contains(GATE2)
        || visible.contains("Cleanup ·")
        || visible.contains("Cleanup running")
        || visible.contains("running ·")
        || visible.contains("quitting waits")
        || visible.contains("A cleanup is running")
        || visible.contains("Quit holla")
}

fn should_escape(visible: &str) -> bool {
    if visible.contains("Cleanup running") || visible.contains("running ·") {
        return visible.contains(GATE2) || visible.contains("Quit holla");
    }
    visible.contains("gate 1 of 2")
        || visible.contains(GATE2)
        || visible.contains("Quit holla")
        || visible.contains("Cleanup ·")
}

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h06_gate/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn launch(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    launch.platform = "macos".to_string();
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions");
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

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn nudge(app: &App, ready: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows) && ready(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && ready(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn open_cleanup(app: &App, prefix: &str, crumb: &str) {
    app.paste(prefix);
    app.wait_screen("catalog row", |screen| {
        text(screen).contains(prefix) && focused_line(screen).contains(prefix)
    });
    app.key("enter");
    app.wait_screen("cleanup", |screen| {
        let visible = text(screen);
        visible.contains(crumb) && visible.contains("Cleanup ·") && !visible.contains("gate 1 of 2")
    });
}

fn open_gate2(app: &App) {
    app.key("d");
    app.wait_screen("gate 1", |screen| {
        text(screen).contains(GATE1) && focused_line(screen).contains("Cancel")
    });
    app.key("right");
    app.wait_screen("continue", |screen| {
        focused_line(screen).contains(CONTINUE) && !focused_line(screen).contains("▎Cancel")
    });
    app.key("enter");
    app.wait_screen("gate 2", |screen| text(screen).contains(GATE2));
}

fn close_gates(app: &App) {
    if text(&app.screen()).contains(GATE2) {
        app.key("escape");
        app.wait_screen("gate 2 closed", |screen| {
            text(screen).contains(CANCELLED) && !text(screen).contains(GATE2)
        });
    }
    if text(&app.screen()).contains("gate 1 of 2") {
        app.key("escape");
        app.wait_screen("gate 1 closed", |screen| {
            text(screen).contains("Cleanup ·") && !text(screen).contains("gate 1 of 2")
        });
    }
}

fn trash_gate(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let open = visible.contains(TRASH_TITLE)
        && visible.contains(GATE2)
        && visible.contains(PHRASE)
        && !visible.contains(STARTED)
        && !visible.contains(STOP_FAILED)
        && !visible.contains(MISMATCH);
    if screen.cols() < 100 {
        return open;
    }
    open && visible.contains("gradle --stop first")
}

fn absent_foreign(screen: &TerminalSnapshot) {
    see(screen, &[], FOREIGN);
}

#[test]
fn h06_gate_route_returns_to_the_finder() {
    let app = launch("h06-gate-route", "parity-gradle");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[
            "~/work/android",
            "mbp",
            "build.gradle.kts",
            "android · Gradle build",
            "Clean Gradle outputs",
        ],
        &["Cleanup ·", STARTED, "Quit holla"],
    );
    absent_foreign(&landing);
    see(&landing, &[], IDEA_IDS);

    open_cleanup(&app, GRADLE_PREFIX, CRUMB);
    let page = app.screen();
    see(
        &page,
        &[
            CRUMB,
            "mode Trash (recoverable until emptied)",
            "live",
            "~/work/android/.gradle",
            "~/work/android/build",
            "~/work/android/app/build",
            "4 selected",
        ],
        &[STARTED, STOP_FAILED, GATE2],
    );
    absent_foreign(&page);

    app.key("d");
    // The review header includes the gate title before the dialog buttons paint.
    let gate = app.wait_screen("route gate", |screen| {
        let visible = text(screen);
        visible.contains(GATE1)
            && visible.contains(PREREQ)
            && visible.contains("Review · trash 4 items")
            && visible.contains("▎Cancel")
            && visible.contains(CONTINUE)
    });
    see(
        &gate,
        &[GATE1, PREREQ, "Review · trash 4 items", "▎Cancel", CONTINUE],
        &[STARTED, STOP_FAILED],
    );

    app.key("escape");
    let cancelled = app.wait_screen("route cancelled", |screen| {
        text(screen).contains(CANCELLED) && text(screen).contains(CRUMB)
    });
    see(
        &cancelled,
        &[CANCELLED, "~/work/android/.gradle", "~/work/android/build"],
        &[STARTED, STOP_FAILED, GATE2],
    );

    app.key("escape");
    let finder = app.wait_screen("finder restored", |screen| {
        let visible = text(screen);
        visible.contains(GRADLE_PREFIX)
            && visible.contains("Enter")
            && !visible.contains("Cleanup ·")
            && !visible.contains("gate 1 of 2")
    });
    see(
        &finder,
        &[GRADLE_PREFIX, "~/work/android", "mbp"],
        &[STARTED, "Cleanup ·", "gate 1 of 2"],
    );
    absent_foreign(&finder);
    app.finish();
}

#[test]
fn h06_gate_confirm_rejects_partial_wrong_and_stale_text() {
    let app = launch("h06-gate-confirm", "parity-gradle");
    open_cleanup(&app, GRADLE_PREFIX, CRUMB);
    let opened = app.screen();
    see(
        &opened,
        &[
            "mode Trash (recoverable until emptied)",
            "4 selected (49.0 MiB)",
            "~/work/android/.gradle",
        ],
        &[STARTED, STOP_FAILED],
    );
    absent_foreign(&opened);
    see(&opened, &[], IDEA_IDS);

    app.key("a");
    app.wait_screen("cleared", |screen| {
        text(screen).contains("Everything unselected") && text(screen).contains("0 selected (0 B)")
    });
    app.key("d");
    let empty = app.wait_screen("empty selection", |screen| text(screen).contains(EMPTY));
    see(
        &empty,
        &[EMPTY, "0 selected (0 B)", "[ ]"],
        &[STARTED, STOP_FAILED, GATE1, GATE2],
    );

    app.key("a");
    app.wait_screen("selected again", |screen| {
        text(screen).contains("Every eligible candidate selected · 49.0 MiB")
            && text(screen).contains("4 selected (49.0 MiB)")
    });

    app.key("d");
    let gate = app.wait_screen("gate 1 open", |screen| {
        text(screen).contains(GATE1) && focused_line(screen).contains("▎Cancel")
    });
    see(
        &gate,
        &[
            GATE1,
            "Review · trash 4 items",
            PREREQ,
            "~/work/android/a/b/c/d/e/build",
            "▎Cancel",
            CONTINUE,
        ],
        &[STARTED, STOP_FAILED, GATE2],
    );

    app.key("escape");
    let esc = app.wait_screen("gate 1 escape", |screen| {
        text(screen).contains(CANCELLED) && !text(screen).contains("gate 1 of 2")
    });
    see(
        &esc,
        &[CANCELLED, CRUMB, "~/work/android/build"],
        &[STARTED, STOP_FAILED],
    );

    app.key("d");
    app.wait_screen("gate 1 for cancel", |screen| {
        focused_line(screen).contains("▎Cancel")
    });
    app.key("enter");
    let button = app.wait_screen("cancel button", |screen| {
        text(screen).contains(CANCELLED) && !text(screen).contains("gate 1 of 2")
    });
    see(&button, &[CANCELLED], &[STARTED, STOP_FAILED, GATE2]);

    open_gate2(&app);
    let confirm = app.wait_screen("trash gate 2", trash_gate);
    see(
        &confirm,
        &[TRASH_TITLE, GATE2, PHRASE, "Type ", "to confirm", "Execute"],
        &[STARTED, STOP_FAILED, MISMATCH, PERM_PHRASE],
    );
    assert!(
        !text(&confirm).contains(DRY_PREREQ),
        "live gate used the dry-run prerequisite\n{}",
        text(&confirm)
    );

    nudge(&app, trash_gate);
    shot(&app, WIDE);

    app.resize(80, 24);
    app.wait_screen("narrow gate", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && trash_gate(screen)
    });
    nudge(&app, trash_gate);
    shot(&app, NARROW);

    app.resize(120, 40);
    let restored = app.wait_screen("gate restored", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && trash_gate(screen)
    });
    see(&restored, &[TRASH_TITLE, PHRASE], &[STARTED]);

    app.type_text("TRASH");
    app.key("enter");
    let partial = app.wait_screen("partial phrase", |screen| {
        text(screen).contains(MISMATCH) && text(screen).contains("TRASH")
    });
    see(
        &partial,
        &[MISMATCH, TRASH_TITLE, PHRASE],
        &[STARTED, STOP_FAILED, STILL_RUNNING],
    );

    app.key("ctrl+l");
    app.type_text("NOPE");
    app.key("enter");
    let wrong = app.wait_screen("wrong phrase", |screen| {
        let visible = text(screen);
        visible.contains(MISMATCH) && visible.contains("NOPE") && !visible.contains("NOPETRASH")
    });
    see(
        &wrong,
        &[MISMATCH, "NOPE", TRASH_TITLE],
        &[STARTED, STOP_FAILED, STILL_RUNNING],
    );

    close_gates(&app);
    app.key("m");
    let permanent = app.wait_screen("permanent mode", |screen| {
        text(screen).contains("Mode: PERMANENT · cannot be undone · any earlier review is void")
            && text(screen).contains("mode PERMANENT (cannot be undone)")
    });
    see(
        &permanent,
        &["Mode: PERMANENT", "mode PERMANENT (cannot be undone)"],
        &[STARTED, PHRASE],
    );

    open_gate2(&app);
    let stale_gate = app.wait_screen("permanent gate", |screen| {
        let visible = text(screen);
        visible.contains(PERM_TITLE)
            && visible.contains(GATE2)
            && visible.contains(PERM_PHRASE)
            && !visible.contains(PHRASE)
    });
    see(
        &stale_gate,
        &[PERM_TITLE, PERM_PHRASE, "Permanently delete"],
        &[STARTED, STOP_FAILED],
    );
    app.type_text(PHRASE);
    app.key("enter");
    let stale = app.wait_screen("stale phrase", |screen| text(screen).contains(MISMATCH));
    see(
        &stale,
        &[MISMATCH, PERM_PHRASE, PERM_TITLE],
        &[STARTED, STOP_FAILED, STILL_RUNNING],
    );

    close_gates(&app);
    app.key("m");
    app.wait_screen("trash mode", |screen| {
        text(screen).contains("Mode: Trash · recoverable until the Trash is emptied")
            && text(screen).contains("mode Trash (recoverable until emptied)")
    });
    app.key("n");
    let dry = app.wait_screen("dry run", |screen| {
        let visible = text(screen);
        visible.contains("Dry run:")
            && visible.contains("dry run: nothing is removed")
            && visible.contains("would-remove")
    });
    see(
        &dry,
        &["dry run: nothing is removed", "would-remove"],
        &[STARTED, STOP_FAILED],
    );

    open_gate2(&app);
    let dry_gate = app.wait_screen("dry gate", |screen| {
        let visible = text(screen);
        visible.contains(DRY_TITLE) && visible.contains(GATE2) && visible.contains(DRY_PREREQ)
    });
    see(
        &dry_gate,
        &[DRY_TITLE, DRY_PREREQ, PHRASE],
        &[STARTED, STOP_FAILED, PREREQ],
    );
    close_gates(&app);
    app.key("n");
    app.wait_screen("live again", |screen| {
        text(screen).contains("Live run") && text(screen).contains("· live ·")
    });

    open_gate2(&app);
    app.wait_screen("live gate before commit", trash_gate);
    app.type_text(PHRASE);
    app.key("enter");
    let started = app.wait_screen("cleanup started", |screen| text(screen).contains(STARTED));
    see(
        &started,
        &[STARTED],
        &[STOP_FAILED, MISMATCH, STILL_RUNNING],
    );

    // No sleep between these keys. Waiting for each screen lets the four
    // item job finish (one item per tick) before a second phrase can be
    // submitted. `commit` returns the still-running refusal only while
    // `cleanup_job` is Some.
    app.key("escape");
    app.key("enter");
    app.key("d");
    app.key("right");
    app.key("enter");
    app.type_text(PHRASE);
    app.key("enter");
    // The first job can finish before these keys land. Its "4 trashed" footer
    // stays up through the next gate, including a phrase mismatch when the
    // remaining set is no longer four items. A second job may also start.
    let second = app.wait_screen("second confirmation", |screen| {
        let visible = text(screen);
        visible.contains(STILL_RUNNING)
            || visible.contains(MISMATCH)
            || visible.contains("4 trashed")
            || visible.contains("4 failed")
            || visible.contains("Cleanup report")
    });
    see(&second, &[], &[STOP_FAILED]);
    if text(&second).contains(STILL_RUNNING) {
        see(&second, &[STILL_RUNNING], &[STOP_FAILED]);
    }
    let started = Instant::now();
    let mut stable = Instant::now();
    loop {
        let visible = text(&app.screen());
        if !quit_blocked(&visible) {
            if stable.elapsed() >= Duration::from_millis(300) {
                break;
            }
        } else {
            stable = Instant::now();
            assert!(
                started.elapsed() < Duration::from_secs(12),
                "cleanup did not settle\n{visible}"
            );
            if should_escape(&visible) {
                app.key("escape");
                let previous = visible.clone();
                let escape_at = Instant::now();
                while text(&app.screen()) == previous
                    && escape_at.elapsed() < Duration::from_millis(500)
                {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // The finder paints no cleanup line while `cleanup_job` is still set.
    // Ctrl+Q then opens the quit guard instead of exiting.
    quit_settled(&app);
}

fn quit_settled(app: &App) {
    app.key("ctrl+q");
    let started = Instant::now();
    let mut moved = false;
    let mut confirmed = false;
    loop {
        if !app.child_running() {
            break;
        }
        let visible = text(&app.screen());
        if !confirmed && visible.contains("Leave when it settles") {
            if visible.contains("▎Leave when it settles") {
                app.key("enter");
                confirmed = true;
            } else if !moved {
                app.key("right");
                moved = true;
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "quit did not exit\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    app.assert_restored_exit();
}

#[test]
fn h06_gate_scenario_parity_gradle() {
    let app = launch("h06-gate-gradle", "parity-gradle");
    let landing = app.screen();
    see(
        &landing,
        &[
            "~/work/android",
            "mbp",
            "android · Gradle build",
            "build.gradle.kts",
            "Clean Gradle outputs",
        ],
        &[STARTED],
    );
    absent_foreign(&landing);
    see(
        &landing,
        &[],
        &[".idea", "workspace.xml", "app.iml", "~/work/ide"],
    );

    open_cleanup(&app, GRADLE_PREFIX, CRUMB);
    let page = app.screen();
    see(
        &page,
        &[
            CRUMB,
            "~/work/android/.gradle",
            "~/work/android/build",
            "~/work/android/app/build",
            "a/b/c/d/e/bu",
            "4 selected",
            "Gradle outputs",
        ],
        &[
            STARTED,
            STOP_FAILED,
            ".idea",
            "workspace.xml",
            "app.iml",
            "~/work/ide",
        ],
    );
    absent_foreign(&page);
    app.finish();
}

#[test]
fn h06_gate_scenario_parity_idea() {
    let app = launch("h06-gate-idea", "parity-idea");
    let landing = app.screen();
    see(
        &landing,
        &["~/work/ide", "mbp", "Clean IntelliJ"],
        &["build.gradle.kts", ".gradle", "~/work/android", STARTED],
    );
    absent_foreign(&landing);

    open_cleanup(&app, IDEA_PREFIX, IDEA_CRUMB);
    let page = app.screen();
    let visible = text(&page);
    assert!(visible.contains(".idea"), "missing .idea\n{visible}");
    assert!(
        visible.contains("workspace.xml") || visible.contains("app.iml"),
        "missing idea file\n{visible}"
    );
    see(
        &page,
        &[IDEA_CRUMB, "IntelliJ metadata", "Trash"],
        &[
            STARTED,
            STOP_FAILED,
            "build.gradle.kts",
            ".gradle",
            "~/work/android",
            "EROFS",
        ],
    );
    absent_foreign(&page);

    app.key("d");
    let gate = app.wait_screen("idea gate", |screen| {
        text(screen).contains(GATE1) && focused_line(screen).contains("▎Cancel")
    });
    see(&gate, &[GATE1, "Cancel"], &[STARTED, STOP_FAILED]);
    app.key("escape");
    let cancelled = app.wait_screen("idea cancelled", |screen| {
        text(screen).contains(CANCELLED) && text(screen).contains(".idea")
    });
    see(
        &cancelled,
        &[CANCELLED, ".idea"],
        &[STARTED, STOP_FAILED, "EROFS"],
    );
    assert!(
        text(&cancelled).contains("workspace.xml") || text(&cancelled).contains("app.iml"),
        "idea file gone after cancel\n{}",
        text(&cancelled)
    );

    app.key("escape");
    let finder = app.wait_screen("idea finder", |screen| {
        text(screen).contains(IDEA_PREFIX) && !text(screen).contains("Cleanup ·")
    });
    see(&finder, &[IDEA_PREFIX], &[STARTED, "Cleanup ·"]);
    app.finish();
}
