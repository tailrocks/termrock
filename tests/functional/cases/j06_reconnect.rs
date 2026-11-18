//! Jackin detach and reconnect, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_reconnect/`. The shots are the capsule after the second
//! attach. They do not nudge. `Launch::new` leaves the clock off. `App::new`
//! would enable it for jackin.
//!
//! The manager row paints `jk-7f3a` as `7f3a` (`trim_start_matches` in
//! `src/bin/jackin/screens/manager.rs`). At 120 and 80 columns the capsule
//! identity bar drops the container id, so the shots show the attach status
//! rather than that short id.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const INSTANCE: &str = "7f3a";
const OTHER: &str = "9b02";
const WORKSPACE: &str = "payments-platform";
const IDENTITY: &str = "payments-platform › the-architect";
const ATTACHED: &str = "Attached to payments-platform · tabs and panes restored";
const DETACHED: &str = "Detached ·";
const KEEPS: &str = "keeps running";
const PAGE: &str = "Workspaces";
const CURRENT: &str = "Current directory";
const OUTRO: &str = "You were in the Construct";
const PREFIX: &str = "prefix…";
const CLAUDE_VERSION: &str = "Claude Code v2.1.14";
const PERMISSION: &str = "Allow edit to src/settlement/config.rs? (y/n)";
const CODEX_TAIL: &str = "12.4k tokens";
const SHELL_PROMPT: &str = "payments-platform ❯";
const DOCS: &str = "docs";
const WIDE: &str = "J06.reconnect.reconnect.120x40";
const NARROW: &str = "J06.reconnect.reconnect.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_reconnect/{}",
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

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
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

fn instance_row(screen: &TerminalSnapshot) -> bool {
    // The tree is narrower than the role label, so "the-architect" is clipped.
    // The short id stays at the start of the row.
    let line = focused_line(screen);
    line.contains(INSTANCE) && !line.contains(OTHER)
}

fn manager_up(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(CURRENT)
        && visible.contains(WORKSPACE)
        && !visible.contains(OUTRO)
        && !visible.contains(ATTACHED)
}

/// `ManagerScreen::new` selects Current directory and leaves workspaces collapsed.
/// Confirm that on the first frame, then move until the focused row is `7f3a`.
fn select_7f3a(app: &App) {
    let first = app.wait_screen("manager", manager_up);
    let line = focused_line(&first);
    if !instance_row(&first) {
        assert!(
            line.contains(CURRENT) && !line.contains(INSTANCE) && !line.contains(OTHER),
            "initial selection is not Current directory\n{line}\n{}",
            text(&first)
        );
    }
    move_to_instance(app);
}

fn move_to_instance(app: &App) {
    if instance_row(&app.screen()) {
        return;
    }
    if focused_line(&app.screen()).contains(CURRENT) {
        app.key("down");
        app.wait_screen("payments-platform row", |screen| {
            let line = focused_line(screen);
            line.contains(WORKSPACE) && !line.contains(INSTANCE) && !line.contains(OTHER)
        });
    }
    if focused_line(&app.screen()).contains(WORKSPACE) && !focused_line(&app.screen()).contains('▾')
    {
        app.key("right");
        app.wait_screen("expanded payments-platform", |screen| {
            let line = focused_line(screen);
            line.contains('▾')
                && line.contains(WORKSPACE)
                && text(screen).contains(INSTANCE)
                && !line.contains(OTHER)
        });
    }
    for _ in 0..4 {
        if instance_row(&app.screen()) {
            return;
        }
        let before = focused_line(&app.screen());
        assert!(
            !before.contains(OTHER),
            "refusing to attach jk-{OTHER}\n{before}\n{}",
            text(&app.screen())
        );
        app.key("down");
        app.wait_screen("next row", |screen| focused_line(screen) != before);
    }
    assert!(
        instance_row(&app.screen()),
        "jk-{INSTANCE} row not selected\n{}",
        text(&app.screen())
    );
}

fn attach(app: &App) -> TerminalSnapshot {
    assert!(
        instance_row(&app.screen()),
        "Enter would not attach jk-{INSTANCE}\n{}",
        text(&app.screen())
    );
    app.key("enter");
    app.wait_screen("attached", wide_capsule)
}

fn detach(app: &App) -> TerminalSnapshot {
    app.key("ctrl+b");
    app.wait_screen("prefix", |screen| text(screen).contains(PREFIX));
    app.key("d");
    app.wait_screen("detached", |screen| {
        let visible = text(screen);
        visible.contains(DETACHED)
            && visible.contains(KEEPS)
            && visible.contains(PAGE)
            && visible.contains(INSTANCE)
            && !visible.contains(ATTACHED)
            && !visible.contains(OUTRO)
            && instance_row(screen)
    })
}

fn wide_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(ATTACHED)
        && visible.contains(IDENTITY)
        && visible.contains(CLAUDE_VERSION)
        && visible.contains(PERMISSION)
        && visible.contains(CODEX_TAIL)
        && visible.contains(SHELL_PROMPT)
        && visible.contains(DOCS)
        && !visible.contains(PAGE)
        && !visible.contains(DETACHED)
        && !visible.contains(OUTRO)
        && !visible.contains(OTHER)
}

fn narrow_capsule(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 80
        && screen.rows() == 24
        && visible.contains(ATTACHED)
        && !visible.contains(PAGE)
        && !visible.contains(DETACHED)
        && !visible.contains(OUTRO)
}

fn painted_id(screen: &TerminalSnapshot) -> String {
    let line = focused_line(screen);
    assert!(
        instance_row(screen),
        "focused row is not {INSTANCE}\n{line}\n{}",
        text(screen)
    );
    INSTANCE.to_string()
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
    check_frame(app, &screen, &test_id);
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
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
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

#[test]
fn j06_reconnect_route() {
    let app = open("j06-reconnect-route");
    select_7f3a(&app);
    let seen = painted_id(&app.screen());
    let attached = attach(&app);
    require(&attached, ATTACHED);
    require(&attached, IDENTITY);
    refuse(&attached, PAGE);
    refuse(&attached, OUTRO);

    let manager = detach(&app);
    require(&manager, DETACHED);
    require(&manager, KEEPS);
    require(&manager, PAGE);
    require(&manager, seen.as_str());
    refuse(&manager, ATTACHED);
    refuse(&manager, OUTRO);
    assert!(
        instance_row(&manager),
        "detach left another row\n{}",
        text(&manager)
    );
    assert!(app.child_running(), "detach quit the process");
}

#[test]
fn j06_reconnect_reconnect() {
    let app = open("j06-reconnect-reconnect");
    select_7f3a(&app);
    let seen = painted_id(&app.screen());
    let attached = attach(&app);
    require(&attached, ATTACHED);
    require(&attached, IDENTITY);

    let manager = detach(&app);
    require(&manager, seen.as_str());
    require(&manager, PAGE);
    refuse(&manager, ATTACHED);
    assert!(
        instance_row(&manager),
        "reconnect row is not {seen}\n{}",
        text(&manager)
    );

    let again = attach(&app);
    require(&again, ATTACHED);
    require(&again, IDENTITY);
    refuse(&again, PAGE);
    refuse(&again, DETACHED);
    refuse(&again, OUTRO);
    assert_eq!((again.cols(), again.rows()), (120, 40));

    shot(&app, WIDE, wide_capsule);
    app.resize(80, 24);
    shot(&app, NARROW, narrow_capsule);
    app.resize(120, 40);
    let restored = wait_settled(
        &app,
        "wide restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        wide_capsule,
    );
    require(&restored, ATTACHED);
    require(&restored, IDENTITY);
    refuse(&restored, PAGE);
    assert!(app.child_running(), "the process quit");
}
