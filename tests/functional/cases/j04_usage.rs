//! Jackin usage route and refresh, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j04_usage/`. The shots are the open failed refresh for Go
//! subscription, after `r` and before `m`. A same-size resize does not force a
//! frame, and the cursor column is part of the hash, so each shot nudges the
//! width and comes back.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const INTRO: &str = "Stand up, operator";
const USAGE: &str = "Usage";
const ACCOUNTS: &str = "Accounts";
const PERSONAL: &str = "Claude · Personal";
const PERSONAL_BACK: &str = "Accounts › Claude › Personal";
const WORK: &str = "Claude · Work";
const GO: &str = "OpenCode · Go subscription";
const GO_BACK: &str = "Accounts › OpenCode › Go subscription";
const STALE: &str = "stale · last good 47 min ago";
const MISSING: &str = "No credential found: ~/.kimi";
const PROVIDER_DOWN: &str = "Provider unavailable";
const RATE: &str = "Rate limited: retry after 25 min";
const RELOAD_MIXED: &str = "Reloading the broker projection · 10 accounts";
const RELOAD_HARD: &str = "Reloading the broker projection · 11 accounts";
const NOTHING: &str = "Nothing to refresh";
const EMPTY: &str = "No providers configured.";
const BROKER_MIN: &str = "broker · 1 min ago";
const RESET: &str = "resets in 1 h 48 min";
const LAST_GOOD: &str = "kept from 3 h ago";
const SAFE_KEY: &str = "…m2Xa · fingerprint c41d0be9";
const SAFE_OP: &str = "Engineering › Anthropic · Work › credential";
const BROKER_DOWN: &str = "Usage broker unreachable";
const WIDE: &str = "J04.usage.usage.120x40";
const NARROW: &str = "J04.usage.usage.narrow";

const SECRETS: [&str; 9] = [
    "openai:valid-cdx01",
    "openai:valid-cdx02",
    "xai:valid-grk01",
    "anthropic:valid-ant01",
    "xai:rotated-leg01",
    "openai:throttled-thr01",
    "opencode:valid-dep01",
    "anthropic:valid-ant02",
    "opencode:valid-ocg01",
];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j04_usage/{}",
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
    // Refresh delays run on the sim clock, which advances from wall ticks.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("manager or intro", |screen| {
        let visible = text(screen);
        visible.contains(DIRECTORY) || visible.contains(INTRO)
    });
    if text(&screen).contains(INTRO) {
        app.key("space");
        return app.wait_text(DIRECTORY);
    }
    screen
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn no_secrets(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for secret in SECRETS {
        assert!(
            !visible.contains(secret),
            "raw secret {secret} is painted\n{visible}"
        );
    }
}

fn selection(visible: &str) -> String {
    visible
        .lines()
        .find(|line| line.contains('▎'))
        .unwrap_or("")
        .to_string()
}

fn step_until(app: &App, key: &str, label: &str, ready: impl Fn(&str) -> bool) -> TerminalSnapshot {
    let screen = app.press_until(
        key,
        label,
        30,
        |screen| selection(&text(screen)),
        |screen| ready(&text(screen)),
    );
    no_secrets(&screen);
    screen
}

fn down_until(app: &App, label: &str, ready: impl Fn(&str) -> bool) -> TerminalSnapshot {
    step_until(app, "down", label, ready)
}

fn up_until(app: &App, label: &str, ready: impl Fn(&str) -> bool) -> TerminalSnapshot {
    step_until(app, "up", label, ready)
}

fn failed_go(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(GO)
        && visible.contains(BROKER_MIN)
        && visible.contains(RATE)
        && visible.contains("57%")
        && visible.contains("45%")
        && visible.contains("22%")
        && visible.contains(RESET)
        && visible.contains(LAST_GOOD)
        && visible.contains(SAFE_KEY)
        && visible.contains("failed")
        && !visible.contains("refreshing")
        && !visible.contains("38%")
        && !visible.contains("76%")
        && !visible.contains(BROKER_DOWN)
        && SECRETS.iter().all(|secret| !visible.contains(secret))
}

fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        quiet_frame(&screen),
        "nudge left the frame\n{}",
        text(&screen)
    );
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
}

fn shoot(app: &App, checkpoint: &str, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    nudge(app, &quiet_frame);
    app.visual(&visual_id(checkpoint));
}

fn remove_experiments(app: &App) {
    down_until(app, "experiments", |visible| {
        focused_line_text(visible).contains("Experiments")
    });
    app.key("d");
    app.wait_text("Remove account");
    app.key("r");
    app.wait_screen("experiments removed", |screen| {
        !text(screen).contains("Remove account")
    });
}

fn focused_line_text(visible: &str) -> String {
    visible
        .lines()
        .find(|line| line.contains('▎'))
        .unwrap_or("")
        .to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RowPlace {
    On,
    Above,
    Below,
    Missing,
}

/// Tree labels sit in the left column. The detail pane shares the terminal
/// row, so a provider name further right is not the header.
fn tree_side(line: &str) -> String {
    line.chars().take(40).collect()
}

fn row_place(visible: &str, header: &str) -> RowPlace {
    let lines: Vec<&str> = visible.lines().collect();
    let Some(focus) = lines.iter().position(|line| line.contains('▎')) else {
        return RowPlace::Missing;
    };
    let Some(header_at) = lines
        .iter()
        .position(|line| !line.contains('▎') && tree_side(line).contains(header))
    else {
        return RowPlace::Missing;
    };
    if focus == header_at + 1 {
        RowPlace::On
    } else if focus <= header_at {
        RowPlace::Above
    } else {
        RowPlace::Below
    }
}

/// Waits for the press to land. A fixed deadline let the next press go out
/// before this one painted, and the walk overshot its row under load.
fn wait_selection_change(app: &App, before: &str) {
    app.wait_screen("selection moves", |screen| {
        selection(&text(screen)) != before
    });
}

/// Where the account row `name` sits against the focus. Only panel body lines
/// count, so the title crumb that repeats the focused name is not the row.
fn account_place(visible: &str, name: &str) -> RowPlace {
    let lines: Vec<&str> = visible.lines().collect();
    let Some(focus) = lines.iter().position(|line| line.contains('▎')) else {
        return RowPlace::Missing;
    };
    let Some(row) = lines
        .iter()
        .position(|line| line.contains('│') && tree_side(line).contains(name))
    else {
        return RowPlace::Missing;
    };
    match focus.cmp(&row) {
        std::cmp::Ordering::Equal => RowPlace::On,
        std::cmp::Ordering::Less => RowPlace::Above,
        std::cmp::Ordering::Greater => RowPlace::Below,
    }
}

/// Stay on the row under `header` until `ready`. Refresh can move the cursor
/// while that row's issue text is a spinner, so a one-shot focus check loses
/// the detail. Keys move back. A key is not sent while the row is already held.
fn hold_under(
    app: &App,
    header: &str,
    label: &str,
    ready: impl Fn(&str) -> bool,
) -> TerminalSnapshot {
    hold_row(app, |visible| row_place(visible, header), label, ready)
}

/// `hold_under` for an account row that is its own label.
fn hold_account(
    app: &App,
    name: &str,
    label: &str,
    ready: impl Fn(&str) -> bool,
) -> TerminalSnapshot {
    hold_row(app, |visible| account_place(visible, name), label, ready)
}

fn hold_row(
    app: &App,
    place_of: impl Fn(&str) -> RowPlace,
    label: &str,
    ready: impl Fn(&str) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let screen = app.screen();
        let visible = text(&screen);
        let place = place_of(&visible);
        if matches!(place, RowPlace::On) && ready(&visible) {
            no_secrets(&screen);
            return screen;
        }
        match place {
            RowPlace::On => {}
            RowPlace::Below => {
                let before = selection(&visible);
                app.key("up");
                wait_selection_change(app, &before);
            }
            RowPlace::Above | RowPlace::Missing => {
                let before = selection(&visible);
                app.key("down");
                wait_selection_change(app, &before);
            }
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{label}\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn j04_usage_route() {
    let app = open("j04-usage-route", "hard-cases");
    reach_manager(&app);
    app.key("c");
    let accounts = app.wait_text(ACCOUNTS);
    no_secrets(&accounts);
    down_until(&app, "personal on accounts", |visible| {
        visible.contains(PERSONAL) && visible.contains("38%")
    });
    app.key("m");
    let usage = app.wait_screen("usage personal", |screen| {
        let visible = text(screen);
        visible.contains(USAGE) && visible.contains(PERSONAL) && visible.contains("38%")
    });
    no_secrets(&usage);
    assert!(
        text(&usage).contains("Local folder · ~/.claude"),
        "personal credential\n{}",
        text(&usage)
    );
    app.key("m");
    let back = app.wait_text(PERSONAL_BACK);
    assert!(
        focused_line(&back).contains("Personal"),
        "return selected a different account\n{}",
        text(&back)
    );
    app.key("m");
    app.wait_text(USAGE);
    app.key("escape");
    let manager = app.wait_text(DIRECTORY);
    assert!(
        !text(&manager).contains("Usage ›"),
        "escape stayed on usage\n{}",
        text(&manager)
    );
    app.key("u");
    app.wait_text(USAGE);
    app.key("q");
    app.wait_text(DIRECTORY);

    app.key("u");
    app.wait_text(USAGE);
    down_until(&app, "go subscription", |visible| visible.contains(RATE));
    app.key("r");
    let started = app.wait_text(RELOAD_HARD);
    assert!(
        text(&started).contains(RELOAD_HARD),
        "hard-cases refresh count\n{}",
        text(&started)
    );
    app.key("r");
    let duplicate = app.wait_text(NOTHING);
    assert!(
        text(&duplicate).contains("refreshing"),
        "duplicate r ran with nobody refreshing\n{}",
        text(&duplicate)
    );
    let failed = app.wait_screen("hard-cases refresh settled", |screen| {
        let visible = text(screen);
        visible.contains(RATE)
            && visible.contains("57%")
            && visible.contains("45%")
            && visible.contains("22%")
            && visible.contains(LAST_GOOD)
            && !visible.contains(RELOAD_HARD)
    });
    no_secrets(&failed);
    assert!(
        !text(&failed).contains(BROKER_DOWN) && !text(&failed).contains("38%"),
        "hard-cases failure replaced Go\n{}",
        text(&failed)
    );
    // Experiments stays on the fixture's refreshing row, so the header keeps
    // "refreshing". Replies for the other accounts still land while the cursor
    // walks up, so hold Work until it reads current instead of stepping past it
    // once.
    let current = hold_account(&app, "Work", "work became current", |visible| {
        visible.contains(WORK)
            && visible.contains("77%")
            && visible.contains("· current ·")
            && !visible.contains(STALE)
    });
    assert!(
        !text(&current).contains(STALE),
        "stale work stayed stale after refresh\n{}",
        text(&current)
    );
    let missing = hold_under(&app, "Kimi", "kimi missing", |visible| {
        visible.contains(MISSING)
    });
    assert!(
        row_place(&text(&missing), "Kimi") == RowPlace::On && text(&missing).contains(MISSING),
        "kimi detail\n{}",
        text(&missing)
    );
    let minimax = hold_under(&app, "MiniMax", "minimax last good", |visible| {
        visible.contains("MiniMax · discovered")
            && visible.contains(PROVIDER_DOWN)
            && visible.contains("8%")
            && visible.contains("47%")
            && !visible.contains("refreshing…")
    });
    assert!(
        !text(&minimax).contains("38%"),
        "minimax lost its last good quota\n{}",
        text(&minimax)
    );
    down_until(&app, "go again", |visible| {
        visible.contains(RATE) && visible.contains("57%")
    });
    app.key("m");
    // The hard-cases header truncates the crumb. The focused row is the return.
    let same = app.wait_screen("hard-cases return", |screen| {
        let visible = text(screen);
        focused_line(screen).contains("Go subscription")
            && visible.contains("57%")
            && visible.contains("Enter Details")
            && !visible.contains("Usage · read-only")
    });
    assert!(
        focused_line(&same).contains("Go subscription") && text(&same).contains(RATE),
        "hard-cases return\n{}",
        text(&same)
    );
    app.finish();

    let empty = open("j04-usage-empty", "first-use");
    reach_manager(&empty);
    empty.key("u");
    let none = empty.wait_text(EMPTY);
    no_secrets(&none);
    empty.key("r");
    let idle = empty.wait_text(NOTHING);
    assert!(
        text(&idle).contains(EMPTY) && text(&idle).contains(NOTHING),
        "zero accounts\n{}",
        text(&idle)
    );
    empty.finish();
}

#[test]
fn j04_usage_usage() {
    let started = Instant::now();
    let app = open("j04-usage-usage", "accounts-mixed");
    app.wait_text(ACCOUNTS);
    remove_experiments(&app);
    app.key("m");
    let overview = app.wait_screen("usage overview", |screen| {
        let visible = text(screen);
        visible.contains("Usage › Overview")
            && visible.contains("degraded")
            && visible.contains("Freshness")
            && visible.contains("broker projection")
    });
    no_secrets(&overview);
    let personal = down_until(&app, "quota", |visible| {
        visible.contains(PERSONAL) && visible.contains("38%") && visible.contains("· current ·")
    });
    assert!(
        text(&personal).contains("Local folder · ~/.claude"),
        "quota credential\n{}",
        text(&personal)
    );
    let stale = down_until(&app, "stale work", |visible| {
        visible.contains(WORK) && visible.contains(STALE) && visible.contains("76%")
    });
    assert!(
        text(&stale).contains(SAFE_OP),
        "stale credential\n{}",
        text(&stale)
    );
    down_until(&app, "missing credential", |visible| {
        visible.contains(MISSING) && visible.contains("needs secret")
    });
    let minimax = down_until(&app, "provider unavailable", |visible| {
        visible.contains("MiniMax · discovered")
    });
    assert!(
        text(&minimax).contains(PROVIDER_DOWN)
            && text(&minimax).contains("8%")
            && text(&minimax).contains("47%"),
        "minimax detail\n{}",
        text(&minimax)
    );
    let before = down_until(&app, "go before refresh", |visible| {
        visible.contains(RATE) && visible.contains("57%") && visible.contains(LAST_GOOD)
    });
    assert!(
        text(&before).contains(SAFE_KEY) && !text(&before).contains("38%"),
        "go before refresh\n{}",
        text(&before)
    );
    app.key("enter");
    app.key("r");
    let reloading = app.wait_text(RELOAD_MIXED);
    assert!(
        text(&reloading).contains(RELOAD_MIXED),
        "mixed refresh count\n{}",
        text(&reloading)
    );
    app.key("r");
    let duplicate = app.wait_text(NOTHING);
    assert!(
        text(&duplicate).contains(NOTHING) && text(&duplicate).contains("refreshing"),
        "duplicate refresh\n{}",
        text(&duplicate)
    );
    app.wait_screen("refresh finished", |screen| {
        let visible = text(screen);
        visible.contains(RATE) && visible.contains("57%") && !visible.contains("refreshing")
    });
    app.key("escape");
    let current = up_until(&app, "refreshed work account", |visible| {
        visible.contains("Usage › Claude › Work") && visible.contains(SAFE_OP)
    });
    assert!(
        text(&current).contains("· current ·")
            && text(&current).contains("77%")
            && !text(&current).contains("stale"),
        "work refresh outcome\n{}",
        text(&current)
    );
    no_secrets(&current);
    down_until(&app, "failed go", |visible| {
        visible.contains(GO) && visible.contains(RATE) && visible.contains("57%")
    });
    app.key("enter");
    let deadline = started + Duration::from_secs(85);
    let quiet = loop {
        let screen = app.screen();
        if failed_go(&screen) {
            break screen;
        }
        assert!(
            Instant::now() < deadline,
            "failed refresh did not settle\n{}",
            text(&screen)
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(
        failed_go(&quiet),
        "checkpoint left the failure\n{}",
        text(&quiet)
    );
    shoot(&app, WIDE, failed_go);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && failed_go(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shoot(&app, NARROW, failed_go);
    app.resize(120, 40);
    let restored = app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && failed_go(screen)
    });
    assert!(
        text(&restored).contains("57%")
            && text(&restored).contains("45%")
            && text(&restored).contains("22%")
            && text(&restored).contains(RATE),
        "numbers changed after the shots\n{}",
        text(&restored)
    );
    app.key("m");
    let same = app.wait_text(GO_BACK);
    assert!(
        focused_line(&same).contains("Go subscription") && text(&same).contains("57%"),
        "return selected a different account\n{}",
        text(&same)
    );
    no_secrets(&same);
    app.finish();
}
