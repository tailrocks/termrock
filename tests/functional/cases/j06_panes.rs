//! Jackin capsule panes, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j06_panes/`. The shots compare the live frame after both
//! Claude markers are on the focused prompt. They do not nudge. `Launch::new`
//! leaves the clock off. `App::new` would enable it for jackin.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, Point, find_all, text, try_launch};

const WORKSPACE: &str = "payments-platform";
const BRANCH: &str = "feature/settlement-backoff";
const CLAUDE_VERSION: &str = "Claude Code v2.1.14";
const CODEX_TITLE: &str = "Codex (Primary)";
const CODEX_TAIL: &str = "12.4k tokens";
const PERMISSION: &str = "Allow edit to src/settlement/config.rs? (y/n)";
const SHELL_BOOT: &str = "git status -sb";
const SHELL_PROMPT: &str = "payments-platform ❯";
const EDIT: &str = "Edit src/settlement/config.rs";
const SKIPPED: &str = "Skipped.";
const PWD_PATH: &str = "/workspace/payments-platform";
const THINKING: &str = "Thinking…";
const LAUNCH_FAILED: &str = "Launch failed";
const OUTRO: &str = "You were in the Construct";
const NETWORK: &str = "network jackin-net not found";
const ATTACHED: &str = "Attached to";
const CLAUDE_TYPE: &str = "zzpaneclaude";
const CLAUDE_PASTE: &str = "zzpasteclaude";
const CODEX_TYPE: &str = "zzpanecodex";
const CODEX_PASTE: &str = "zzpastecodex";
const SHELL_TYPE: &str = "zzpaneshell";
const SHELL_PASTE: &str = "zzpasteshell";
const PROBE: &str = "qqqz";
const WIDE: &str = "J06.panes.input.120x40";
const NARROW: &str = "J06.panes.input.narrow";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Claude,
    Codex,
    Shell,
}

struct Seams {
    vertical: u16,
    horizontal: u16,
}

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_panes/{}",
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
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn glyph(screen: &TerminalSnapshot, x: u16, y: u16) -> &str {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .map(|cell| cell.text())
        .unwrap_or("")
}

fn is_vbar(value: &str) -> bool {
    value == "│" || value == "┃"
}

fn seams_of(screen: &TerminalSnapshot) -> Option<Seams> {
    let mut vertical = None;
    for claude in find_all(screen, "Claude Code") {
        for codex in find_all(screen, "Codex") {
            if codex.y != claude.y || codex.x <= claude.x {
                continue;
            }
            for x in (claude.x + 1)..codex.x {
                if is_vbar(glyph(screen, x, claude.y)) {
                    vertical = Some(x);
                    break;
                }
            }
            if vertical.is_some() {
                break;
            }
        }
        if vertical.is_some() {
            break;
        }
    }
    let vertical = vertical?;
    let shell_y = find_all(screen, "Shell")
        .into_iter()
        .filter(|point| point.x > vertical)
        .map(|point| point.y)
        .min()?;
    let horizontal = shell_y.checked_sub(1)?;
    Some(Seams {
        vertical,
        horizontal,
    })
}

fn side_of(point: Point, seams: &Seams) -> Side {
    if point.x < seams.vertical {
        Side::Claude
    } else if point.y < seams.horizontal {
        Side::Codex
    } else {
        Side::Shell
    }
}

fn home(marker: &str) -> Side {
    if marker == CLAUDE_TYPE || marker == CLAUDE_PASTE {
        Side::Claude
    } else if marker == CODEX_TYPE || marker == CODEX_PASTE {
        Side::Codex
    } else {
        Side::Shell
    }
}

fn require_seams(screen: &TerminalSnapshot) -> Seams {
    seams_of(screen).unwrap_or_else(|| panic!("pane seams not found\n{}", text(screen)))
}

fn assert_only(screen: &TerminalSnapshot, needle: &str, side: Side, seams: &Seams) {
    let hits = find_all(screen, needle);
    assert!(!hits.is_empty(), "missing {needle}\n{}", text(screen));
    for hit in hits {
        let got = side_of(hit, seams);
        assert_eq!(
            got,
            side,
            "{needle} at ({}, {}) is {got:?}, expected {side:?}\n{}",
            hit.x,
            hit.y,
            text(screen)
        );
    }
}

fn assert_none(screen: &TerminalSnapshot, needle: &str) {
    let hits = find_all(screen, needle);
    assert!(
        hits.is_empty(),
        "{needle} present at {hits:?}\n{}",
        text(screen)
    );
}

fn on_side(screen: &TerminalSnapshot, needle: &str, side: Side, seams: &Seams) -> bool {
    let hits = find_all(screen, needle);
    !hits.is_empty() && hits.iter().all(|hit| side_of(*hit, seams) == side)
}

fn identities(screen: &TerminalSnapshot, seams: &Seams) {
    assert_only(screen, CLAUDE_VERSION, Side::Claude, seams);
    assert_only(screen, PERMISSION, Side::Claude, seams);
    assert_only(screen, CODEX_TITLE, Side::Codex, seams);
    assert_only(screen, CODEX_TAIL, Side::Codex, seams);
    assert_only(screen, SHELL_BOOT, Side::Shell, seams);
    assert_only(screen, SHELL_PROMPT, Side::Shell, seams);
    let branch_hits = find_all(screen, BRANCH);
    assert!(
        branch_hits
            .iter()
            .any(|hit| side_of(*hit, seams) == Side::Shell),
        "{BRANCH} is not in the shell pane\n{}",
        text(screen)
    );
    assert!(
        text(screen).contains(WORKSPACE),
        "missing {WORKSPACE}\n{}",
        text(screen)
    );
}

fn boundaries(
    screen: &TerminalSnapshot,
    fresh: Option<&str>,
    visible: &[&str],
    edit: bool,
    pwd: bool,
) {
    let seams = require_seams(screen);
    let markers = [
        CLAUDE_TYPE,
        CLAUDE_PASTE,
        CODEX_TYPE,
        CODEX_PASTE,
        SHELL_TYPE,
        SHELL_PASTE,
    ];
    for marker in markers {
        if visible.contains(&marker) {
            assert_only(screen, marker, home(marker), &seams);
        } else {
            assert_none(screen, marker);
        }
    }
    if let Some(marker) = fresh {
        assert!(
            visible.contains(&marker),
            "fresh marker {marker} was not listed as visible"
        );
        assert_only(screen, marker, home(marker), &seams);
    }
    if edit {
        assert_only(screen, EDIT, Side::Claude, &seams);
    } else {
        assert_none(screen, EDIT);
    }
    assert_none(screen, SKIPPED);
    if pwd {
        assert_only(screen, PWD_PATH, Side::Shell, &seams);
    } else {
        assert_none(screen, PWD_PATH);
    }
}

fn capsule_ready(screen: &TerminalSnapshot) -> bool {
    if text(screen).contains(OUTRO) {
        return false;
    }
    let Some(seams) = seams_of(screen) else {
        return false;
    };
    on_side(screen, CLAUDE_VERSION, Side::Claude, &seams)
        && on_side(screen, PERMISSION, Side::Claude, &seams)
        && on_side(screen, CODEX_TITLE, Side::Codex, &seams)
        && on_side(screen, CODEX_TAIL, Side::Codex, &seams)
        && on_side(screen, SHELL_BOOT, Side::Shell, &seams)
        && on_side(screen, SHELL_PROMPT, Side::Shell, &seams)
        && find_all(screen, BRANCH)
            .iter()
            .any(|hit| side_of(*hit, &seams) == Side::Shell)
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

fn wait_capsule(app: &App) -> TerminalSnapshot {
    wait_settled(
        app,
        "capsule panes",
        Duration::from_millis(400),
        Duration::from_secs(10),
        capsule_ready,
    )
}

fn wait_marker(app: &App, marker: &str) -> TerminalSnapshot {
    app.wait_screen(&format!("visible {marker}"), |screen| {
        text(screen).contains(marker)
    })
}

fn pane_span(screen: &TerminalSnapshot, side: Side) -> Option<(u16, u16, u16, u16)> {
    let seams = seams_of(screen)?;
    let cols = u16::try_from(screen.cols()).ok()?;
    let rows = u16::try_from(screen.rows()).ok()?;
    let right = seams.vertical.saturating_add(1);
    let below = seams.horizontal.saturating_add(1);
    Some(match side {
        Side::Claude => (0, seams.vertical, 0, rows),
        Side::Codex => (right, cols, 0, seams.horizontal),
        Side::Shell => (right, cols, below, rows),
    })
}

fn pane_line(screen: &TerminalSnapshot, y: u16, x0: u16, x1: u16) -> String {
    let row = screen.row(u32::from(y)).expect("row");
    let start = usize::from(x0);
    let end = usize::from(x1);
    row.iter()
        .enumerate()
        .filter(|(col, _)| (start..end).contains(col))
        .map(|(_, cell)| cell.text())
        .collect()
}

/// Token after the lowest `❯` in the pane. History prompts sit above it.
fn live_input(screen: &TerminalSnapshot, side: Side) -> String {
    let Some((x0, x1, y0, y1)) = pane_span(screen, side) else {
        return String::new();
    };
    let mut input = String::new();
    for y in y0..y1 {
        let line = pane_line(screen, y, x0, x1);
        let Some(idx) = line.rfind('❯') else {
            continue;
        };
        let after = line[idx + '❯'.len_utf8()..].trim_start();
        input = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
    }
    input
}

fn owner_side(screen: &TerminalSnapshot, gone: &[&str]) -> Side {
    let seams = require_seams(screen);
    for marker in gone {
        if let Some(hit) = find_all(screen, marker).first().copied() {
            return side_of(hit, &seams);
        }
    }
    panic!("marker side not found\n{}", text(screen));
}

fn markers_gone(screen: &TerminalSnapshot, gone: &[&str]) -> bool {
    let visible = text(screen);
    gone.iter().all(|marker| !visible.contains(marker))
}

fn fail_backspace(app: &App, side: Side) -> String {
    format!(
        "backspace not confirmed\ninput {:?}\n{}",
        live_input(&app.screen(), side),
        text(&app.screen())
    )
}

/// The frame has to sit still before a backspace. A reply line or a resize
/// still being applied drops the key, and the prompt does not change.
fn unchanged(app: &App, side: Side, hold: Duration, deadline: Instant) -> TerminalSnapshot {
    let mut last = app.screen();
    let mut since = Instant::now();
    loop {
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{}",
            fail_backspace(app, side)
        );
        if since.elapsed() >= hold {
            return last;
        }
        std::thread::sleep(Duration::from_millis(20));
        let screen = app.screen();
        if screen != last {
            last = screen;
            since = Instant::now();
        }
    }
}

fn confirm_backspace(app: &App, side: Side, previous: &str, deadline: Instant) {
    let hold = Duration::from_millis(200);
    loop {
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{}",
            fail_backspace(app, side)
        );
        let before = live_input(&app.screen(), side);
        app.key("backspace");
        let mut last = app.screen();
        let mut since = Instant::now();
        loop {
            assert!(
                Instant::now() < deadline && app.child_running(),
                "{}",
                fail_backspace(app, side)
            );
            let screen = app.screen();
            let input = live_input(&screen, side);
            if input != before && !input.contains(previous) {
                return;
            }
            if screen != last {
                last = screen;
                since = Instant::now();
            } else if since.elapsed() >= hold {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn backspace(app: &App, typed: &str, gone: &[&str]) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(25);
    let side = owner_side(&app.screen(), gone);
    unchanged(app, side, Duration::from_millis(200), deadline);
    loop {
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{}",
            fail_backspace(app, side)
        );
        let screen = app.screen();
        let input = live_input(&screen, side);
        // The marker substring dies on its last character, while the earlier
        // characters are still in the prompt. `y` and `pwd` only match a
        // trimmed buffer (`AgentProcess::on_input`), so the token has to be
        // empty too.
        let tail_left = !input.is_empty() && typed.starts_with(&input);
        if markers_gone(&screen, gone)
            && !tail_left
            && seams_of(&screen).is_some()
            && text(&screen).contains(CODEX_TAIL)
        {
            return screen;
        }
        // An empty prompt cannot confirm a backspace. Waiting inside that
        // call misses the frame where the marker has already left.
        if input.is_empty() {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        confirm_backspace(app, side, &input, deadline);
    }
}

fn claude_answered(screen: &TerminalSnapshot) -> bool {
    let Some(seams) = seams_of(screen) else {
        return false;
    };
    let edits = find_all(screen, EDIT);
    !edits.is_empty()
        && edits
            .iter()
            .all(|hit| side_of(*hit, &seams) == Side::Claude)
        && text(screen).contains("Done · 3 files changed")
        && !text(screen).contains(SKIPPED)
}

/// `y` is the whole permission token. Enter before that glyph paints submits
/// an empty line and the `(y/n)` prompt never answers. A slow paint must not
/// type another `y`: the live token then becomes a run, and neither branch
/// submits it. The simulator can consume the single `y` before the next read.
fn submit_claude_yes(app: &App) {
    let deadline = Instant::now() + Duration::from_secs(12);
    let mut armed = false;
    let mut backed_from: Option<usize> = None;
    loop {
        assert!(
            Instant::now() < deadline && app.child_running(),
            "claude yes was not armed\n{}",
            text(&app.screen())
        );
        if claude_answered(&app.screen()) {
            return;
        }
        let input = live_input(&app.screen(), Side::Claude);
        if input == "y" {
            app.key("enter");
            let sent = Instant::now();
            while sent.elapsed() < Duration::from_millis(400) && Instant::now() < deadline {
                if live_input(&app.screen(), Side::Claude).is_empty() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            continue;
        }
        let extra_y = input.len() > 1 && input.bytes().all(|byte| byte == b'y');
        if extra_y && backed_from != Some(input.len()) {
            app.key("backspace");
            backed_from = Some(input.len());
        } else if input.is_empty() && !armed {
            // The permission line ignores a pasted byte. One key answers it.
            // A second `y` is a new message and the edit is skipped.
            app.key("y");
            armed = true;
            backed_from = None;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn aim(app: &App, want: Side) {
    let dirs: &[&str] = match want {
        Side::Codex => &["l", "k", "j", "h"],
        Side::Shell => &["j", "l", "k", "h"],
        Side::Claude => &["h", "l", "j", "k"],
    };
    for dir in dirs {
        app.key("ctrl+b");
        app.key(dir);
        app.type_text(PROBE);
        let screen = app.wait_screen(&format!("probe {dir}"), |screen| {
            text(screen).contains(PROBE)
        });
        let seams = require_seams(&screen);
        let hits = find_all(&screen, PROBE);
        let landed = hits.first().map(|hit| side_of(*hit, &seams));
        assert!(
            hits.iter()
                .all(|hit| side_of(*hit, &seams) == landed.unwrap()),
            "probe split across panes\n{}",
            text(&screen)
        );
        backspace(app, PROBE, &[PROBE]);
        if landed == Some(want) {
            return;
        }
    }
    let screen = app.screen();
    let seams = require_seams(&screen);
    let needle = match want {
        Side::Claude => PERMISSION,
        Side::Codex => CODEX_TAIL,
        Side::Shell => SHELL_PROMPT,
    };
    let point = find_all(&screen, needle)
        .into_iter()
        .find(|hit| side_of(*hit, &seams) == want)
        .unwrap_or_else(|| panic!("no click target for {want:?}\n{}", text(&screen)));
    app.click(point);
    app.type_text(PROBE);
    let clicked = app.wait_screen("probe click", |screen| text(screen).contains(PROBE));
    let seams = require_seams(&clicked);
    let landed = find_all(&clicked, PROBE)
        .first()
        .map(|hit| side_of(*hit, &seams));
    backspace(app, PROBE, &[PROBE]);
    assert_eq!(
        landed,
        Some(want),
        "click did not focus {want:?}\n{}",
        text(&app.screen())
    );
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let screen = wait_settled(
        app,
        &test_id,
        Duration::from_millis(200),
        Duration::from_secs(25),
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

fn checkpoint(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    if screen.cols() != cols || screen.rows() != rows {
        return false;
    }
    let Some(seams) = seams_of(screen) else {
        return false;
    };
    let visible = text(screen);
    if visible.contains(EDIT)
        || visible.contains(ATTACHED)
        || visible.contains(LAUNCH_FAILED)
        || visible.contains(OUTRO)
        || visible.contains(CODEX_TYPE)
        || visible.contains(SHELL_TYPE)
        || visible.contains(SKIPPED)
        || visible.contains(PWD_PATH)
    {
        return false;
    }
    let claude_type = find_all(screen, CLAUDE_TYPE);
    let claude_paste = find_all(screen, CLAUDE_PASTE);
    !claude_type.is_empty()
        && !claude_paste.is_empty()
        && claude_type
            .iter()
            .chain(claude_paste.iter())
            .all(|hit| side_of(*hit, &seams) == Side::Claude)
        && on_side(screen, CODEX_TAIL, Side::Codex, &seams)
        && on_side(screen, SHELL_PROMPT, Side::Shell, &seams)
}

fn other_scenario(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(!visible.contains(LAUNCH_FAILED), "{visible}");
    assert!(!visible.contains(NETWORK), "{visible}");
    assert!(!visible.contains(OUTRO), "{visible}");
}

#[test]
fn j06_panes_route() {
    let app = open("j06-panes-route");
    let capsule = wait_capsule(&app);
    identities(&capsule, &require_seams(&capsule));
    other_scenario(&capsule);
    assert_eq!((capsule.cols(), capsule.rows()), (120, 40));
    app.key("ctrl+b");
    app.key("d");
    let manager = app.wait_screen("detached", |screen| {
        let visible = text(screen);
        visible.contains("Detached ·")
            && visible.contains("keeps running")
            && visible.contains("Workspaces")
            && !visible.contains(PERMISSION)
            && !visible.contains(OUTRO)
    });
    let visible = text(&manager);
    assert!(visible.contains("Detached ·"), "{visible}");
    assert!(visible.contains(WORKSPACE), "{visible}");
    assert!(visible.contains("Workspaces"), "{visible}");
    assert!(!visible.contains(PERMISSION), "{visible}");
    assert!(!visible.contains(OUTRO), "{visible}");
}

#[test]
fn j06_panes_input() {
    let app = open("j06-panes-input");
    let capsule = wait_capsule(&app);
    identities(&capsule, &require_seams(&capsule));
    boundaries(&capsule, None, &[], false, false);

    app.type_text(CLAUDE_TYPE);
    let typed = wait_marker(&app, CLAUDE_TYPE);
    boundaries(&typed, Some(CLAUDE_TYPE), &[CLAUDE_TYPE], false, false);

    app.paste(CLAUDE_PASTE);
    let pasted = wait_marker(&app, CLAUDE_PASTE);
    boundaries(
        &pasted,
        Some(CLAUDE_PASTE),
        &[CLAUDE_TYPE, CLAUDE_PASTE],
        false,
        false,
    );

    shot(&app, WIDE, |screen| checkpoint(screen, 120, 40));
    app.resize(80, 24);
    shot(&app, NARROW, |screen| checkpoint(screen, 80, 24));
    app.resize(120, 40);
    wait_settled(
        &app,
        "wide restored",
        Duration::from_millis(200),
        Duration::from_secs(10),
        |screen| checkpoint(screen, 120, 40),
    );

    let cleared = backspace(
        &app,
        &format!("{CLAUDE_TYPE}{CLAUDE_PASTE}"),
        &[CLAUDE_TYPE, CLAUDE_PASTE],
    );
    boundaries(&cleared, None, &[], false, false);

    submit_claude_yes(&app);
    let replied = app.wait_screen("claude yes reply", claude_answered);
    boundaries(&replied, None, &[], true, false);
    assert!(
        find_all(&replied, "Done · 3 files changed")
            .iter()
            .all(|hit| side_of(*hit, &require_seams(&replied)) == Side::Claude),
        "{}",
        text(&replied)
    );

    aim(&app, Side::Codex);
    app.type_text(CODEX_TYPE);
    let codex_typed = wait_marker(&app, CODEX_TYPE);
    boundaries(&codex_typed, Some(CODEX_TYPE), &[CODEX_TYPE], true, false);
    app.paste(CODEX_PASTE);
    let codex_pasted = wait_marker(&app, CODEX_PASTE);
    boundaries(
        &codex_pasted,
        Some(CODEX_PASTE),
        &[CODEX_TYPE, CODEX_PASTE],
        true,
        false,
    );
    app.key("enter");
    let codex_reply = app.wait_screen("codex echo", |screen| {
        let Some(seams) = seams_of(screen) else {
            return false;
        };
        !find_all(screen, THINKING).is_empty()
            && find_all(screen, THINKING)
                .iter()
                .all(|hit| side_of(*hit, &seams) == Side::Codex)
    });
    boundaries(&codex_reply, None, &[CODEX_TYPE, CODEX_PASTE], true, false);
    assert_only(
        &codex_reply,
        THINKING,
        Side::Codex,
        &require_seams(&codex_reply),
    );

    aim(&app, Side::Shell);
    app.type_text(SHELL_TYPE);
    let shell_typed = wait_marker(&app, SHELL_TYPE);
    boundaries(
        &shell_typed,
        Some(SHELL_TYPE),
        &[CODEX_TYPE, CODEX_PASTE, SHELL_TYPE],
        true,
        false,
    );
    app.paste(SHELL_PASTE);
    let shell_pasted = wait_marker(&app, SHELL_PASTE);
    boundaries(
        &shell_pasted,
        Some(SHELL_PASTE),
        &[CODEX_TYPE, CODEX_PASTE, SHELL_TYPE, SHELL_PASTE],
        true,
        false,
    );
    backspace(
        &app,
        &format!("{SHELL_TYPE}{SHELL_PASTE}"),
        &[SHELL_TYPE, SHELL_PASTE],
    );
    let before_pwd = app.screen();
    boundaries(&before_pwd, None, &[CODEX_TYPE, CODEX_PASTE], true, false);

    app.type_text("pwd");
    app.key("enter");
    let listed = app.wait_screen("shell pwd", |screen| {
        let Some(seams) = seams_of(screen) else {
            return false;
        };
        !find_all(screen, PWD_PATH).is_empty()
            && find_all(screen, PWD_PATH)
                .iter()
                .all(|hit| side_of(*hit, &seams) == Side::Shell)
    });
    boundaries(&listed, None, &[CODEX_TYPE, CODEX_PASTE], true, true);
    assert_none(&listed, SHELL_TYPE);
    assert_none(&listed, SHELL_PASTE);
    assert_none(&listed, SKIPPED);
}

#[test]
fn j06_panes_scenario_capsule_multi() {
    let app = open("j06-panes-scenario");
    let capsule = wait_capsule(&app);
    let visible = text(&capsule);
    assert!(visible.contains(WORKSPACE), "{visible}");
    assert!(visible.contains(BRANCH), "{visible}");
    assert!(visible.contains(CLAUDE_VERSION), "{visible}");
    assert!(visible.contains(CODEX_TITLE), "{visible}");
    assert!(visible.contains(CODEX_TAIL), "{visible}");
    other_scenario(&capsule);
    identities(&capsule, &require_seams(&capsule));
}
