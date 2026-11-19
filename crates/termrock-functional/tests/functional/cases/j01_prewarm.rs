//! Jackin workspace prewarm, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j01_prewarm/`. `w` on a workspace row is
//! `src/bin/jackin/screens/manager.rs` `prewarm`. The busy row's spinner
//! changes every reduced-motion tick, so `App::visual` would wait until the
//! screen goes quiet, which is after `Msg::Prewarmed`. The shot is the live
//! frame that still paints the in-progress text. The session runs on the
//! stepped test clock, so the 2400 ms job stays in progress until the case
//! steps the clock: the busy window is not a wall-clock race. The clock moves
//! one tick at a time until the frame the baseline pins is on screen.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, text, try_launch};

const WIDE: &str = "J01.prewarm.prewarm.120x40";
const NARROW: &str = "J01.prewarm.prewarm.narrow";
const SPIN: &str = "⠹";
const PROGRESS: &str = "Prewarming the derived image…";
const BUSY: &str = "prewarming";
const BUSY_ROW: &str = "prewarming…";
const REFUSAL: &str = "Prewarm already running";
const DONE: &str = "Prewarmed payments-platform · derived image cached";
const AGO: &str = "daemon · 4 s ago";
const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const WORKSPACES: &[&str] = &[
    "payments-platform",
    "infra-control-plane",
    "release-automation",
    "customer-portal",
];
const STEP_LIMIT: usize = 40;
const ABSENT: &[&str] = &["77aa", "88bb", "no instances"];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j01_prewarm/{}",
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
    launch.clock = true;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("manager or intro", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) || visible.contains("Stand up, operator")
    });
    if text(&screen).contains("Stand up, operator") {
        app.key("space");
        return app.wait_text(DIRECTORY);
    }
    screen
}

fn workspace_focused(screen: &TerminalSnapshot) -> bool {
    text(screen)
        .lines()
        .any(|line| line.contains('▎') && line.contains(PAYMENTS) && line.contains('▸'))
}

fn select_payments(app: &App) -> TerminalSnapshot {
    app.key("down");
    app.wait_screen("payments-platform row", workspace_focused)
}

fn forbid(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for item in ABSENT {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn painted_ids(screen: &TerminalSnapshot) -> Vec<String> {
    let visible = text(screen);
    let mut ids = Vec::new();
    let mut token = String::new();
    for ch in visible.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_hexdigit() {
            token.push(ch);
            continue;
        }
        if token.len() == 4 && token.chars().any(|c| c.is_ascii_digit()) {
            ids.push(token.clone());
        }
        token.clear();
    }
    ids.sort();
    ids.dedup();
    ids
}

fn jk_ids(screen: &TerminalSnapshot) -> Vec<String> {
    let visible = text(screen);
    let mut ids = Vec::new();
    let mut rest = visible.as_str();
    while let Some(at) = rest.find("jk-") {
        let tail = &rest[at + 3..];
        let hex: String = tail.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        let n = hex.len();
        if n == 0 {
            rest = &rest[at + 3..];
            continue;
        }
        ids.push(format!("jk-{hex}"));
        rest = &tail[n..];
    }
    ids.sort();
    ids.dedup();
    ids
}

fn workspaces_stay(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(
        visible.contains("Workspaces") && visible.contains(DIRECTORY),
        "left the manager\n{visible}"
    );
    for name in WORKSPACES {
        assert!(visible.contains(name), "missing {name}\n{visible}");
    }
    forbid(screen);
}

fn in_progress(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(SPIN)
        && visible.contains(PROGRESS)
        && visible.contains(BUSY)
        && !visible.contains(DONE)
        && !visible.contains(REFUSAL)
}

fn wide_frame(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 120
        && screen.rows() == 40
        && in_progress(screen)
        && text(screen).contains(AGO)
        && workspace_focused(screen)
        && !text(screen).contains(BUSY_ROW)
}

fn narrow_frame(screen: &TerminalSnapshot) -> bool {
    screen.cols() == 80
        && screen.rows() == 24
        && in_progress(screen)
        && text(screen).contains(BUSY_ROW)
        && workspace_focused(screen)
}

/// Steps the test clock one tick at a time until `accept` holds. The fence
/// makes each read the frame after the step, so nothing is read early and no
/// wall time passes.
fn step_until(
    app: &App,
    reason: &str,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    for _ in 0..STEP_LIMIT {
        app.fence();
        let screen = app.screen();
        if accept(&screen) {
            return screen;
        }
        app.advance_clock(1);
    }
    app.fence();
    let screen = app.screen();
    assert!(
        accept(&screen),
        "{reason} not seen in {STEP_LIMIT} ticks\n{}",
        text(&screen)
    );
    screen
}

/// The spinner moves with the clock. Step to the screen that paints the
/// pinned glyph, before the 2400 ms job lands, and compare that one.
fn shot(
    app: &App,
    checkpoint: &str,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> Result<(), String> {
    let test_id = visual_id(checkpoint);
    let screen = step_until(app, &test_id, accept);
    crate::support::frame_mismatch(app, &screen, &test_id).map_or(Ok(()), Err)
}

fn ids_unchanged(screen: &TerminalSnapshot, ids: &[String], jk: &[String]) {
    let visible = text(screen);
    assert_eq!(painted_ids(screen), ids, "instance ids changed\n{visible}");
    assert_eq!(jk_ids(screen), jk, "a jk- id appeared\n{visible}");
    workspaces_stay(screen);
}

#[test]
fn j01_prewarm_route() {
    let app = open("j01-prewarm-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    assert!(
        !text(&manager).contains("Stand up, operator"),
        "intro stayed up\n{}",
        text(&manager)
    );
    let row = select_payments(&app);
    assert!(workspace_focused(&row), "{}", text(&row));
    workspaces_stay(&row);
    app.key("w");
    let busy = app.wait_screen("prewarm started", |screen| {
        let visible = text(screen);
        visible.contains(BUSY) && visible.contains(PROGRESS) && workspace_focused(screen)
    });
    assert!(
        !text(&busy).contains(DONE) && !text(&busy).contains(REFUSAL),
        "{}",
        text(&busy)
    );
    let done = step_until(&app, "prewarm done", |screen| text(screen).contains(DONE));
    let visible = text(&done);
    assert!(workspace_focused(&done), "{visible}");
    assert!(!visible.contains(BUSY), "busy row stayed\n{visible}");
    assert!(!visible.contains(PROGRESS), "progress stayed\n{visible}");
    workspaces_stay(&done);
    app.finish();
}

#[test]
fn j01_prewarm_prewarm() {
    let app = open("j01-prewarm-prewarm");
    reach_manager(&app);
    select_payments(&app);
    let armed = step_until(&app, "daemon age", |screen| text(screen).contains(AGO));
    assert!(
        text(&armed).contains(AGO) && workspace_focused(&armed),
        "{}",
        text(&armed)
    );
    let ids = painted_ids(&armed);
    let jk = jk_ids(&armed);
    assert!(
        ids.contains(&"7f3a".to_string()) && ids.contains(&"c41e".to_string()),
        "payments instances were not painted: {ids:?}\n{}",
        text(&armed)
    );
    assert!(jk.is_empty(), "jk- id before prewarm: {jk:?}");
    forbid(&armed);
    workspaces_stay(&armed);

    app.key("w");
    let wide = shot(&app, WIDE, wide_frame);
    app.resize(80, 24);
    let narrow_screen =
        app.wait_screen("80x24", |screen| screen.cols() == 80 && screen.rows() == 24);
    assert_eq!((narrow_screen.cols(), narrow_screen.rows()), (80, 24));
    let narrow = shot(&app, NARROW, narrow_frame);
    app.resize(120, 40);
    let restored = app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(BUSY)
            && !text(screen).contains(DONE)
    });
    assert!(
        text(&restored).contains(PROGRESS) || text(&restored).contains(BUSY),
        "left the in-progress window\n{}",
        text(&restored)
    );

    app.key("w");
    let refused = app.wait_screen("duplicate refused", |screen| {
        let visible = text(screen);
        visible.contains(REFUSAL) && visible.contains(BUSY) && !visible.contains(DONE)
    });
    ids_unchanged(&refused, &ids, &jk);
    assert!(
        !text(&refused).contains(PROGRESS),
        "the refusal did not replace the progress line\n{}",
        text(&refused)
    );

    let done = step_until(&app, "prewarm done", |screen| text(screen).contains(DONE));
    let visible = text(&done);
    assert!(workspace_focused(&done), "{visible}");
    assert!(
        !visible.contains(BUSY),
        "busy row stayed after one completion\n{visible}"
    );
    assert!(!visible.contains(REFUSAL), "refusal stayed\n{visible}");
    assert!(
        visible.contains("1 running"),
        "the workspace gained a running instance\n{visible}"
    );
    ids_unchanged(&done, &ids, &jk);

    app.key("right");
    app.key("down");
    let instance = app.wait_screen("instance row", |screen| {
        text(screen)
            .lines()
            .any(|line| line.contains('▎') && line.contains("7f3a"))
    });
    assert!(
        !text(&instance).contains(BUSY_ROW),
        "instance row is prewarming\n{}",
        text(&instance)
    );
    app.key("w");
    // The fence returns once the app has handled `w` and drawn, so an
    // ignored key is read as ignored, not as not yet read.
    let ignored = app.settle();
    let visible = text(&ignored);
    assert!(
        !visible.contains(BUSY) && !visible.contains(PROGRESS) && !visible.contains(REFUSAL),
        "w on an instance started prewarm\n{visible}"
    );
    assert!(visible.contains(DONE), "left the completion\n{visible}");
    ids_unchanged(&ignored, &ids, &jk);

    let mut errors = Vec::new();
    if let Err(error) = wide {
        errors.push(error);
    }
    if let Err(error) = narrow {
        errors.push(error);
    }
    if !errors.is_empty() {
        panic!("{}", errors.join("\n"));
    }
    app.finish();
}
