//! Jackin prefix entry, repeat, cancel, palette, and menu, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_prefix/`. The shots are the armed prefix, before any command
//! letter. They do not nudge. `Launch::new` leaves the clock off. `App::new`
//! would enable it for jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, find_all, row_text, text, try_launch};

const WORKSPACE: &str = "payments-platform";
const CLAUDE_VERSION: &str = "Claude Code v2.1.14";
const CODEX_TITLE: &str = "Codex (Primary)";
const CODEX_TAIL: &str = "12.4k tokens";
const PERMISSION: &str = "Allow edit to src/settlement/config.rs? (y/n)";
const SHELL_BOOT: &str = "git status -sb";
const SHELL_PROMPT: &str = "payments-platform ❯";
const BRANCH: &str = "feature/settlement-backoff";
const OUTRO: &str = "You were in the Construct";
const LAUNCH_FAILED: &str = "Launch failed";
const NETWORK: &str = "network jackin-net not found";
const MARKER: &str = "zzprefix";
const LEAK: &str = "zzprefixz";
const FILTER: &str = "zzpal";
const FILTER_LEAK: &str = "zzprefixzzpal";
const PREFIX: &str = "prefix…";
const TIMED_OUT: &str = "Prefix timed out";
const ZOOMED: &str = "Zoomed · z restores the layout";
const UNZOOMED: &str = "Unzoomed";
const PALETTE: &str = "Command palette";
const PLACEHOLDER: &str = "Type to filter commands…";
const WIDE: &str = "J06.prefix.prefix.120x40";
const NARROW: &str = "J06.prefix.prefix.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_prefix/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, JACKIN);
    launch.args = vec!["--scenario".to_string(), "capsule-multi".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. The fixed frame keeps its attach footer.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn in_body(screen: &TerminalSnapshot, needle: &str) -> bool {
    let rows = u16::try_from(screen.rows()).unwrap_or(0);
    find_all(screen, needle)
        .iter()
        .any(|hit| hit.y >= 4 && hit.y + 1 < rows)
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

fn capsule_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(CLAUDE_VERSION)
        && visible.contains(PERMISSION)
        && visible.contains(CODEX_TITLE)
        && visible.contains(CODEX_TAIL)
        && visible.contains(SHELL_BOOT)
        && visible.contains(SHELL_PROMPT)
        && visible.contains(BRANCH)
        && visible.contains(WORKSPACE)
        && !visible.contains(OUTRO)
        && !visible.contains(LAUNCH_FAILED)
        && !visible.contains(NETWORK)
}

fn marker_exact(screen: &TerminalSnapshot) -> bool {
    let mut found = false;
    for row in 0..screen.rows() {
        let line = row_text(screen, row);
        let Some(at) = line.find(MARKER) else {
            continue;
        };
        found = true;
        let rest = line[at + MARKER.len()..].trim_start();
        if rest.starts_with('z') || line.contains(LEAK) || line.contains(FILTER) {
            return false;
        }
    }
    found && !text(screen).contains(LEAK) && !text(screen).contains(FILTER)
}

fn armed(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    if screen.cols() != cols || screen.rows() != rows {
        return false;
    }
    let visible = text(screen);
    let base = visible.contains(PREFIX)
        && !visible.contains(TIMED_OUT)
        && marker_exact(screen)
        && !visible.contains(OUTRO)
        && !visible.contains(ZOOMED)
        && !visible.contains(PALETTE);
    if !base {
        return false;
    }
    if cols == 120 {
        visible.contains(PERMISSION)
            && visible.contains(CLAUDE_VERSION)
            && visible.contains(CODEX_TITLE)
            && visible.contains(CODEX_TAIL)
            && visible.contains(SHELL_PROMPT)
    } else {
        visible.contains(MARKER)
    }
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

fn assert_marker(screen: &TerminalSnapshot) {
    assert!(
        marker_exact(screen),
        "pane text is not only {MARKER}\n{}",
        text(screen)
    );
    refuse(screen, TIMED_OUT);
}

#[test]
fn j06_prefix_route() {
    let app = open("j06-prefix-route");
    let capsule = app.wait_screen("capsule", capsule_ready);
    require(&capsule, CLAUDE_VERSION);
    require(&capsule, PERMISSION);
    require(&capsule, CODEX_TITLE);
    require(&capsule, CODEX_TAIL);
    require(&capsule, SHELL_PROMPT);
    require(&capsule, WORKSPACE);
    assert_eq!((capsule.cols(), capsule.rows()), (120, 40));
    refuse(&capsule, OUTRO);
    refuse(&capsule, LAUNCH_FAILED);
    app.key("ctrl+b");
    app.wait_screen("prefix", |screen| text(screen).contains(PREFIX));
    app.key("d");
    let manager = app.wait_screen("detached", |screen| {
        let visible = text(screen);
        visible.contains("Detached ·")
            && visible.contains("keeps running")
            && visible.contains("Workspaces")
            && !visible.contains(PERMISSION)
            && !visible.contains(OUTRO)
    });
    require(&manager, "Detached ·");
    require(&manager, "keeps running");
    require(&manager, WORKSPACE);
    require(&manager, "Workspaces");
    refuse(&manager, PERMISSION);
    refuse(&manager, OUTRO);
    refuse(&manager, TIMED_OUT);
    assert!(app.child_running(), "detach quit the process");
}

#[test]
fn j06_prefix_prefix() {
    let app = open("j06-prefix-prefix");
    app.wait_screen("capsule", capsule_ready);
    app.type_text(MARKER);
    let typed = app.wait_screen("marker", |screen| {
        in_body(screen, MARKER) && marker_exact(screen)
    });
    assert_marker(&typed);
    require(&typed, PERMISSION);

    app.key("ctrl+b");
    let armed_screen = app.wait_screen("prefix armed", |screen| armed(screen, 120, 40));
    require(&armed_screen, PREFIX);
    refuse(&armed_screen, TIMED_OUT);
    assert_marker(&armed_screen);

    shot(&app, WIDE, |screen| armed(screen, 120, 40));
    app.resize(80, 24);
    shot(&app, NARROW, |screen| armed(screen, 80, 24));
    app.resize(120, 40);
    let restored = wait_settled(
        &app,
        "wide restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        |screen| armed(screen, 120, 40),
    );
    require(&restored, PREFIX);
    refuse(&restored, TIMED_OUT);
    assert_marker(&restored);

    app.key("z");
    let zoomed = app.wait_screen("zoomed", |screen| {
        let visible = text(screen);
        visible.contains(ZOOMED) && marker_exact(screen) && !visible.contains(TIMED_OUT)
    });
    require(&zoomed, ZOOMED);
    refuse(&zoomed, LEAK);
    refuse(&zoomed, TIMED_OUT);
    assert_marker(&zoomed);
    app.key("ctrl+b");
    app.wait_screen("prefix before unzoom", |screen| {
        text(screen).contains(PREFIX) && !text(screen).contains(TIMED_OUT)
    });
    app.key("z");
    let unzoomed = app.wait_screen("unzoomed", |screen| {
        let visible = text(screen);
        visible.contains(UNZOOMED)
            && !visible.contains(ZOOMED)
            && !visible.contains(PREFIX)
            && marker_exact(screen)
            && !visible.contains(TIMED_OUT)
    });
    require(&unzoomed, UNZOOMED);
    refuse(&unzoomed, ZOOMED);
    refuse(&unzoomed, TIMED_OUT);
    assert_marker(&unzoomed);

    app.key("ctrl+b");
    app.wait_screen("prefix repeat", |screen| {
        let visible = text(screen);
        visible.contains(PREFIX) && marker_exact(screen) && !visible.contains(TIMED_OUT)
    });
    app.key("ctrl+b");
    let cancelled = app.wait_screen("prefix cancelled", |screen| {
        let visible = text(screen);
        !visible.contains(PREFIX) && marker_exact(screen) && !visible.contains(TIMED_OUT)
    });
    refuse(&cancelled, PREFIX);
    refuse(&cancelled, TIMED_OUT);
    assert_marker(&cancelled);
    app.key("z");
    let typed_z = app.wait_screen("z typed", |screen| {
        let visible = text(screen);
        visible.contains(LEAK) && !visible.contains(ZOOMED) && !visible.contains(TIMED_OUT)
    });
    require(&typed_z, LEAK);
    require(&typed_z, MARKER);
    refuse(&typed_z, ZOOMED);
    refuse(&typed_z, TIMED_OUT);
    app.key("backspace");
    let cleared = app.wait_screen("z removed", |screen| {
        let visible = text(screen);
        !visible.contains(LEAK)
            && marker_exact(screen)
            && !visible.contains(ZOOMED)
            && !visible.contains(TIMED_OUT)
    });
    assert_marker(&cleared);
    refuse(&cleared, ZOOMED);

    app.key("ctrl+b");
    app.wait_screen("prefix before esc", |screen| {
        text(screen).contains(PREFIX) && !text(screen).contains(TIMED_OUT)
    });
    app.key("esc");
    let escaped = app.wait_screen("prefix escaped", |screen| {
        let visible = text(screen);
        !visible.contains(PREFIX) && marker_exact(screen) && !visible.contains(TIMED_OUT)
    });
    refuse(&escaped, PREFIX);
    refuse(&escaped, TIMED_OUT);
    assert_marker(&escaped);

    app.key("ctrl+b");
    app.wait_screen("prefix before palette", |screen| {
        text(screen).contains(PREFIX) && !text(screen).contains(TIMED_OUT)
    });
    app.key("space");
    let palette = app.wait_screen("palette", |screen| {
        let visible = text(screen);
        visible.contains(PALETTE) && visible.contains(PLACEHOLDER) && !visible.contains(TIMED_OUT)
    });
    require(&palette, PALETTE);
    require(&palette, PLACEHOLDER);
    refuse(&palette, TIMED_OUT);
    app.type_text(FILTER);
    let filtered = app.wait_screen("palette filter", |screen| {
        let visible = text(screen);
        if !visible.contains(PALETTE) || !visible.contains(FILTER) || visible.contains(TIMED_OUT) {
            return false;
        }
        if visible.contains(LEAK) || visible.contains(FILTER_LEAK) {
            return false;
        }
        for row in 0..screen.rows() {
            let line = row_text(screen, row);
            if line.contains(MARKER) && line.contains(FILTER) {
                return false;
            }
        }
        true
    });
    require(&filtered, PALETTE);
    require(&filtered, FILTER);
    refuse(&filtered, FILTER_LEAK);
    refuse(&filtered, LEAK);
    refuse(&filtered, TIMED_OUT);
    app.key("esc");
    // A non-empty picker query consumes the first Esc and clears itself.
    let after_esc = app.wait_screen("palette escape", |screen| !text(screen).contains(FILTER));
    if text(&after_esc).contains(PALETTE) {
        app.key("esc");
    }
    let closed = app.wait_screen("palette closed", |screen| {
        let visible = text(screen);
        !visible.contains(PALETTE)
            && !visible.contains(PLACEHOLDER)
            && !visible.contains(FILTER)
            && marker_exact(screen)
            && !visible.contains(TIMED_OUT)
    });
    refuse(&closed, PALETTE);
    refuse(&closed, FILTER);
    refuse(&closed, TIMED_OUT);
    assert_marker(&closed);

    app.key("f10");
    let menu = app.wait_screen("file menu", |screen| {
        let visible = text(screen);
        visible.contains("File")
            && visible.contains("New tab")
            && visible.contains("Split right")
            && marker_exact(screen)
            && !visible.contains(TIMED_OUT)
    });
    require(&menu, "File");
    require(&menu, "New tab");
    require(&menu, "Split right");
    refuse(&menu, TIMED_OUT);
    assert_marker(&menu);
    app.key("esc");
    let menu_closed = app.wait_screen("menu closed", |screen| {
        let visible = text(screen);
        !visible.contains("New tab")
            && !visible.contains("Split right")
            && marker_exact(screen)
            && !visible.contains(TIMED_OUT)
    });
    refuse(&menu_closed, "New tab");
    refuse(&menu_closed, "Split right");
    refuse(&menu_closed, TIMED_OUT);
    assert_marker(&menu_closed);
    assert!(app.child_running(), "the process quit");
}
