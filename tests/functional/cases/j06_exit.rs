//! Jackin exit, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_exit/`. The shots are the second discard dialog, before
//! Discard and before Cancel. They do not nudge. `Launch::new` leaves the
//! clock off. `App::new` would enable it for jackin.
//!
//! Ctrl+Q on the capsule is `CapsuleScreen::on_key` in
//! `src/bin/jackin/screens/capsule.rs`. `Route::Capsule` is capturing, so
//! that handler runs before the shell quit chord in `src/shell/router.rs`.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, find_all, row_text, text, try_launch};

const DIRTY_ID: &str = "7f3a";
const CLEAN_ID: &str = "9b02";
const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const DIRTY_IDENTITY: &str = "payments-platform › the-architect";
const CLEAN_IDENTITY: &str = "infra-control-plane › sre";
const PAGE: &str = "Workspaces";
const CURRENT: &str = "Current directory";
const OUTRO: &str = "You were in the Construct";
const UNSAVED: &str = "Unsaved work — quit?";
const CLEAN_TITLE: &str = "Quit jackin❯?";
const FORCE_STOP: &str = "force-stops the container";
const START_AGENT: &str = "Start a new agent";
const INSPECT_ROW: &str = "Inspect changes";
const KEEP_ROW: &str = "Quit & keep changes";
const DISCARD_ROW: &str = "Quit & discard changes";
const SECOND: &str = "Quit and discard changes?";
const DISCARDED: &str = "Changes discarded";
const KEPT: &str = "Exited · changes kept in the preserved instance";
const WARNING: &str = "4 changed · 1 unpushed";
const KEPT_TREE: &str = "2 uncommitted · 1 unpushed";
const CLEAN_LIFE: &str = "The final session ended cleanly";
const DIRTY_LIFE: &str = "Stopped with uncommitted changes";
const NO_OUTRO: &str = "returning to host without the outro";
const INSPECT_TITLE: &str = "Inspect changes · payments-platform";
const MOD: &str = "src/settlement/mod.rs";
const RETRY: &str = "src/settlement/retry.rs";
const UNPUSHED: &str = "1 commit not pushed";
const TYPE_TOKEN: &str = "Type payments-platform to confirm";
const TOKEN: &str = "payments-platform";
const CLAUDE: &str = "Claude Code v2.1.14";
const CODEX: &str = "OpenAI Codex v0.48";
const ATTACHED: &str = "Attached to ";
const MARK: &str = "(●)";
const WIDE: &str = "J06.exit.exit.120x40";
const NARROW: &str = "J06.exit.exit.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_exit/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    // A clock-step paste is consumed only while motion is paused, and paused
    // changes hint-bar bold. Reduced would type the paste into the focused pane.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn open_paused_clock(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), scenario.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("paused".to_string());
    launch.clock = true;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn chosen_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains(MARK))
        .unwrap_or_default()
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

fn wait_ready(
    app: &App,
    reason: &str,
    budget: Duration,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    loop {
        let screen = app.screen();
        if accept(&screen) {
            return screen;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} frame not seen\n{}",
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

fn manager_hard(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(CURRENT)
        && visible.contains(PAYMENTS)
        && visible.contains(INFRA)
        && !visible.contains(OUTRO)
        && !visible.contains(ATTACHED)
        && !visible.contains(UNSAVED)
        && !visible.contains(CLEAN_TITLE)
}

fn focused_workspace(screen: &TerminalSnapshot, name: &str) -> bool {
    let line = focused_line(screen);
    line.contains(name) && (line.contains('▸') || line.contains('▾')) && !line.contains(CURRENT)
}

fn on_current(screen: &TerminalSnapshot) -> bool {
    let line = focused_line(screen);
    manager_hard(screen) && line.contains(CURRENT) && !line.contains('▸') && !line.contains('▾')
}

fn capsule_hints(visible: &str) -> bool {
    // Attach paints "Attached to … · tabs and panes restored" on the hint bar.
    // That status hides the last hint, `Ctrl+Q`, until its five-second TTL.
    // `Ctrl+B` is the first capsule hint and is on the first paint.
    visible.contains("Ctrl+B") || visible.contains("Ctrl+Q")
}

fn dirty_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(DIRTY_IDENTITY)
        && visible.contains(CLAUDE)
        && capsule_hints(&visible)
        && !visible.contains(UNSAVED)
        && !visible.contains(CLEAN_TITLE)
        && !visible.contains(SECOND)
        && !visible.contains(INSPECT_TITLE)
        && !visible.contains(OUTRO)
        && !visible.contains(PAGE)
        && !visible.contains(DISCARDED)
        && !visible.contains(KEPT)
}

fn clean_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(CLEAN_IDENTITY)
        && visible.contains(CODEX)
        && capsule_hints(&visible)
        && !visible.contains(UNSAVED)
        && !visible.contains(CLEAN_TITLE)
        && !visible.contains(CLAUDE)
        && !visible.contains(OUTRO)
        && !visible.contains(PAGE)
        && !visible.contains(DISCARDED)
}

fn dirty_choice(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(UNSAVED)
        && visible.contains(START_AGENT)
        && visible.contains(INSPECT_ROW)
        && visible.contains(KEEP_ROW)
        && visible.contains(DISCARD_ROW)
        && visible.contains(WARNING)
        && visible.contains("Cancel")
        && visible.contains("Choose")
        && !visible.contains(CLEAN_TITLE)
        && !visible.contains(SECOND)
        && !visible.contains(DISCARDED)
        && !visible.contains(KEPT)
        && !visible.contains(OUTRO)
}

fn second_dialog(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    screen.cols() == cols
        && screen.rows() == rows
        && visible.contains(SECOND)
        && visible.contains("Discard")
        && visible.contains("Cancel")
        && !visible.contains("tabs and panes restored")
        && !visible.contains(DISCARDED)
        && !visible.contains(KEPT)
        && !visible.contains(OUTRO)
        && !visible.contains(CLEAN_TITLE)
}

fn second_wide(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    second_dialog(screen, 120, 40)
        && visible.contains(TYPE_TOKEN)
        && visible.contains("payments-platform · instance 7f3a")
        && visible.contains("discard every uncommitted change")
}

fn chosen_is(screen: &TerminalSnapshot, label: &str) -> bool {
    dirty_choice(screen) && chosen_line(screen).contains(label)
}

fn open_dirty(app: &App) -> TerminalSnapshot {
    app.key("ctrl+q");
    let screen = app.wait_screen("unsaved quit", |screen| chosen_is(screen, START_AGENT));
    refuse(&screen, CLEAN_TITLE);
    refuse(&screen, DISCARDED);
    refuse(&screen, SECOND);
    screen
}

fn move_choice(app: &App, label: &str) {
    app.key("down");
    app.wait_screen(label, |screen| chosen_is(screen, label));
}

fn at_manager_after_exit(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(NO_OUTRO)
        && !visible.contains(OUTRO)
        && !visible.contains(SECOND)
        && !visible.contains(UNSAVED)
        && !visible.contains(CLEAN_TITLE)
        && !visible.contains(DISCARDED)
        && !visible.contains(KEPT)
}

fn select_dirty(app: &App) {
    app.wait_screen("manager", on_current);
    app.key("down");
    app.wait_screen("payments-platform", |screen| {
        focused_workspace(screen, PAYMENTS) && !focused_line(screen).contains('▾')
    });
    app.key("right");
    app.wait_screen("expanded payments-platform", |screen| {
        focused_line(screen).contains('▾')
            && focused_line(screen).contains(PAYMENTS)
            && text(screen).contains(DIRTY_ID)
    });
    app.key("down");
    app.wait_screen("jk-7f3a row", |screen| {
        let line = focused_line(screen);
        line.contains(DIRTY_ID) && !line.contains(CLEAN_ID) && !line.contains(CODEX)
    });
}

fn select_clean(app: &App) {
    app.wait_screen("manager", on_current);
    app.key("down");
    app.wait_screen("payments-platform collapsed", |screen| {
        focused_workspace(screen, PAYMENTS)
            && !focused_line(screen).contains('▾')
            && !focused_line(screen).contains(DIRTY_ID)
    });
    app.key("down");
    app.wait_screen("infra-control-plane", |screen| {
        focused_workspace(screen, INFRA)
            && !focused_line(screen).contains('▾')
            && !focused_line(screen).contains(CLEAN_ID)
            && !focused_line(screen).contains(DIRTY_ID)
    });
    app.key("right");
    app.wait_screen("expanded infra-control-plane", |screen| {
        focused_line(screen).contains('▾')
            && focused_line(screen).contains(INFRA)
            && text(screen).contains(CLEAN_ID)
    });
    app.key("down");
    app.wait_screen("jk-9b02 row", |screen| {
        let line = focused_line(screen);
        line.contains(CLEAN_ID) && line.contains("sre") && !line.contains(DIRTY_ID)
    });
}

fn attach_dirty(app: &App) -> TerminalSnapshot {
    app.key("enter");
    app.wait_screen("dirty capsule", dirty_capsule)
}

fn attach_clean(app: &App) -> TerminalSnapshot {
    app.key("enter");
    app.wait_screen("clean capsule", clean_capsule)
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    // The hint bar can hold one bold pattern, then another, for longer than
    // the 200ms settle. The reference frame is one of those paints.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut last: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    let mut missed = String::new();
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let same = last.as_ref().is_some_and(|prev| prev == &screen);
            if same && since.expect("stability start").elapsed() >= Duration::from_millis(200) {
                if let Some(why) = frame_mismatch(app, &screen, &test_id) {
                    missed = why;
                    last = None;
                    since = None;
                } else {
                    return;
                }
            } else if !same {
                last = Some(screen);
                since = Some(Instant::now());
            }
        } else {
            last = None;
            since = None;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{test_id} reference frame not seen: {missed}\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn frame_mismatch(app: &App, screen: &TerminalSnapshot, test_id: &str) -> Option<String> {
    let root = crate::support::oracle_baselines();
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    match outcome {
        CheckOutcome::Pass(_) => None,
        CheckOutcome::Failure(failure) => Some(format!(
            "{} cells {}",
            failure.reason(),
            failure.changed_cell_total()
        )),
    }
}

fn clean_exit() {
    let app = open("j06-exit-clean", "hard-cases");
    select_clean(&app);
    let capsule = attach_clean(&app);
    require(&capsule, CLEAN_IDENTITY);
    refuse(&capsule, DIRTY_IDENTITY);
    app.key("ctrl+q");
    let dialog = app.wait_screen("clean quit", |screen| {
        let visible = text(screen);
        visible.contains(CLEAN_TITLE)
            && visible.contains(FORCE_STOP)
            && visible.contains("Quit")
            && visible.contains("Cancel")
            && !visible.contains(UNSAVED)
            && !visible.contains(DISCARDED)
            && !visible.contains(SECOND)
    });
    require(&dialog, CLEAN_TITLE);
    // Destructive dialogs focus Cancel. Enter is that button, not Quit.
    app.key("enter");
    let stayed = app.wait_screen("cancel left the capsule", clean_capsule);
    refuse(&stayed, CLEAN_TITLE);
    refuse(&stayed, CLEAN_LIFE);
    assert!(app.child_running(), "cancel quit the process");

    app.key("ctrl+q");
    app.wait_screen("clean quit again", |screen| {
        text(screen).contains(CLEAN_TITLE)
    });
    app.key("right");
    app.key("enter");
    let manager = app.wait_screen("clean instance ended", |screen| {
        let visible = text(screen);
        at_manager_after_exit(screen)
            && visible.contains(CLEAN_LIFE)
            && visible.contains(CLEAN_ID)
            && visible.contains("sre")
            && !visible.contains(KEPT_TREE)
            && !visible.contains(DIRTY_LIFE)
    });
    require(&manager, CLEAN_LIFE);
    refuse(&manager, OUTRO);
    refuse(&manager, DISCARDED);
    refuse(&manager, CLEAN_TITLE);
}

fn keep_changes() {
    let app = open("j06-exit-keep", "hard-cases");
    select_dirty(&app);
    attach_dirty(&app);
    let dialog = open_dirty(&app);
    require(&dialog, WARNING);
    move_choice(&app, INSPECT_ROW);
    move_choice(&app, KEEP_ROW);
    app.key("enter");
    let manager = app.wait_screen("changes kept", |screen| {
        let visible = text(screen);
        at_manager_after_exit(screen)
            && visible.contains(KEPT_TREE)
            && visible.contains(DIRTY_LIFE)
            && visible.contains(DIRTY_ID)
            && visible.contains("the-architect")
    });
    require(&manager, KEPT_TREE);
    refuse(&manager, CLEAN_LIFE);
    refuse(&manager, DISCARDED);
    refuse(&manager, OUTRO);
}

fn discard_confirm() {
    let app = open_paused_clock("j06-exit-discard", "hard-cases");
    select_dirty(&app);
    attach_dirty(&app);
    // The paused fixture clock keeps the final unknown-discovery warning
    // visible until this test has observed the manager state.
    app.advance_clock(5_000);
    open_dirty(&app);
    move_choice(&app, INSPECT_ROW);
    move_choice(&app, KEEP_ROW);
    move_choice(&app, DISCARD_ROW);
    let chosen = app.screen();
    refuse(&chosen, DISCARDED);
    refuse(&chosen, SECOND);
    app.key("enter");
    // The second dialog is up while "tabs and panes restored" still owns
    // the hint bar. That status is five seconds of sim time, and the 10 s
    // screen wait ends first when the suite is busy.
    let second = wait_ready(
        &app,
        "second discard prompt",
        Duration::from_secs(25),
        |screen| second_wide(screen) && !text(screen).contains(DISCARDED),
    );
    refuse(&second, DISCARDED);
    refuse(&second, KEPT);
    app.type_text(TOKEN);
    app.key("enter");
    let manager = app.wait_screen("changes discarded", |screen| {
        let visible = text(screen);
        at_manager_after_exit(screen)
            && visible.contains(CLEAN_LIFE)
            && visible.contains(DIRTY_ID)
            && visible.contains("the-architect")
            && !visible.contains(KEPT_TREE)
            && !visible.contains(DIRTY_LIFE)
    });
    require(&manager, CLEAN_LIFE);
    refuse(&manager, KEPT_TREE);
    refuse(&manager, DISCARDED);
    refuse(&manager, OUTRO);
}

fn inspect_return() {
    let app = open("j06-exit-inspect", "outro-last");
    app.wait_screen("capsule", dirty_capsule);
    open_dirty(&app);
    move_choice(&app, INSPECT_ROW);
    app.key("enter");
    let inspect = app.wait_screen("inspector", |screen| {
        let visible = text(screen);
        visible.contains(INSPECT_TITLE)
            && visible.contains(MOD)
            && visible.contains(RETRY)
            && visible.contains(UNPUSHED)
            && !visible.contains(UNSAVED)
            && !visible.contains(DISCARDED)
            && !visible.contains(SECOND)
    });
    require(&inspect, INSPECT_TITLE);
    refuse(&inspect, UNSAVED);
    app.key("esc");
    let back = app.wait_screen("quit choice restored", |screen| {
        chosen_is(screen, START_AGENT)
    });
    require(&back, WARNING);
    require(&back, DISCARD_ROW);
    refuse(&back, INSPECT_TITLE);
    refuse(&back, DISCARDED);
    refuse(&back, KEPT);
    refuse(&back, OUTRO);
    assert!(app.child_running(), "inspect quit the process");
}

fn cancel_second(app: &App) -> TerminalSnapshot {
    let started = Instant::now();
    let mut esc_at: Option<Instant> = None;
    let mut clicked = false;
    loop {
        let screen = app.screen();
        if dirty_capsule(&screen) {
            return screen;
        }
        let visible = text(&screen);
        assert!(
            started.elapsed() < Duration::from_secs(12) && app.child_running(),
            "cancel kept the capsule\n{visible}"
        );
        if let Some(sent) = esc_at {
            if !clicked && sent.elapsed() >= Duration::from_millis(400) {
                let hits: Vec<_> = find_all(&screen, "Cancel")
                    .into_iter()
                    .filter(|hit| {
                        let row = row_text(&screen, u32::from(hit.y));
                        row.contains("Discard") && !row.contains("Esc")
                    })
                    .collect();
                if hits.len() == 1 {
                    app.click(hits[0]);
                    clicked = true;
                }
            }
        } else {
            app.key("esc");
            esc_at = Some(Instant::now());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn status_cleared(screen: &TerminalSnapshot) -> bool {
    dirty_capsule(screen) && !text(screen).contains("tabs and panes restored")
}

fn discard_shots_then_cancel() {
    let app = open("j06-exit-shots", "outro-last");
    app.wait_screen("capsule", dirty_capsule);
    // Attach writes a status that owns the hint bar's right side for five
    // seconds of sim time. The baseline is that bar after the status expires.
    // Sim time lags the wall clock while the suite is busy, so the budget is
    // wider than the TTL.
    wait_settled(
        &app,
        "attach status cleared",
        Duration::from_millis(200),
        Duration::from_secs(25),
        status_cleared,
    );
    open_dirty(&app);
    move_choice(&app, INSPECT_ROW);
    move_choice(&app, KEEP_ROW);
    move_choice(&app, DISCARD_ROW);
    let armed = app.screen();
    refuse(&armed, DISCARDED);
    refuse(&armed, SECOND);
    app.key("enter");
    let second = app.wait_screen("second dialog wide", second_wide);
    refuse(&second, DISCARDED);
    require(&second, SECOND);
    assert_eq!((second.cols(), second.rows()), (120, 40));

    shot(&app, WIDE, second_wide);
    app.resize(80, 24);
    shot(&app, NARROW, |screen| second_dialog(screen, 80, 24));
    app.resize(120, 40);
    let restored = wait_settled(
        &app,
        "wide dialog restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        second_wide,
    );
    require(&restored, SECOND);
    require(&restored, TYPE_TOKEN);
    refuse(&restored, DISCARDED);

    // The resize can drop the first Esc while the dialog is still painting.
    // A second Esc, then the Cancel button on the Discard row, leaves the
    // capsule up without discarding.
    let capsule = cancel_second(&app);
    refuse(&capsule, DISCARDED);
    refuse(&capsule, SECOND);
    refuse(&capsule, KEPT);
    assert!(app.child_running(), "cancel quit the process");

    let again = open_dirty(&app);
    require(&again, WARNING);
    refuse(&again, DISCARDED);
    app.key("esc");
    let stayed = app.wait_screen("choice cancelled", dirty_capsule);
    refuse(&stayed, UNSAVED);
    assert!(app.child_running(), "the process quit");
}

#[test]
fn j06_exit_route() {
    let app = open("j06-exit-route", "outro-last");
    let capsule = app.wait_screen("capsule", dirty_capsule);
    require(&capsule, DIRTY_IDENTITY);
    refuse(&capsule, OUTRO);
    let dialog = open_dirty(&app);
    require(&dialog, WARNING);
    require(&dialog, KEEP_ROW);
    require(&dialog, DISCARD_ROW);
    refuse(&dialog, CLEAN_TITLE);
    app.key("esc");
    let back = app.wait_screen("back on the capsule", dirty_capsule);
    refuse(&back, UNSAVED);
    refuse(&back, DISCARDED);
    refuse(&back, KEPT);
    refuse(&back, SECOND);
    refuse(&back, OUTRO);
    assert!(app.child_running(), "route quit the process");
}

#[test]
fn j06_exit_exit() {
    clean_exit();
    keep_changes();
    discard_confirm();
    inspect_return();
    discard_shots_then_cancel();
}
