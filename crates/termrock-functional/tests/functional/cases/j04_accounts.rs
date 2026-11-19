//! Jackin Accounts route, fixture rows, and removal, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j04_accounts/`. The shots are the open Remove dialog for Work,
//! before Remove is activated. A same-size resize does not force a frame, and
//! the cursor column is part of the hash, so each shot nudges the width and
//! comes back.
//!
//! Experiments starts `Freshness::Refreshing` and nothing schedules its
//! completion. `strip_right` paints a spinner from `now_ms`, so that row is
//! removed before the shot. The checkpoint is the later Work dialog.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const OVERVIEW: &str = "Accounts › Overview";
const EMPTY: &str = "No accounts registered";
const PERSONAL: &str = "Personal";
const WORK: &str = "Work";
const PRIMARY: &str = "Primary";
const EXPERIMENTS: &str = "Experiments";
const GO: &str = "Go subscription";
const REVOKED: &str = "Revoked key";
const LOCAL: &str = "Local folder";
const HOME_CLAUDE: &str = "~/.claude";
const OAUTH: &str = "Claude OAuth profile";
const PERSONAL_ID: &str = "alexey@donbeave.dev";
const PLAN_MAX: &str = "Max 5x";
const ONE_PASSWORD: &str = "1Password";
const WORK_REF: &str = "op://v_eng01/it_ant01/credential";
const WORK_MASK: &str = "••••••••…3c9e";
const WORK_ID: &str = "alexey@chainargos.com";
const PAYMENTS: &str = "payments-platform";
const API_KEY: &str = "API key";
const EXP_FP: &str = "7f3a91c2";
const EXP_TAIL: &str = "k7Qz";
const EXP_MASK: &str = "••••••••…k7Qz";
const GO_FP: &str = "c41d0be9";
const GO_TAIL: &str = "m2Xa";
const GO_MASK: &str = "••••••••…m2Xa";
const GO_ID: &str = "donbeave";
const MISSING: &str = "No credential found: ~/.kimi has no api key";
const DENIED: &str = "OpenCode returned 403";
const LOCKED: &str = "1Password locked";
const UNAUTHORIZED: &str = "Not authorized: xAI rejected the credential";
const HTTP_401: &str = "HTTP 401";
const DISABLED: &str = "Disabled Codex · Primary · provider default cleared";
const ENABLED: &str = "Enabled Codex · Primary";
const DEFAULT: &str = "Default set: OpenAI → Codex · Primary";
const EXP_TITLE: &str = "Remove account Experiments?";
const EXP_BODY: &str = "Codex · Experiments is removed from the registry. 1 Workspace or Role selections that point at it fall back to the provider default. The credential source itself is untouched.";
const EXP_REMOVED: &str = "Removed Codex · Experiments";
const WORK_TITLE: &str = "Remove account Work?";
const WORK_BODY: &str = "Claude · Work is removed from the registry. 1 Workspace or Role selections that point at it fall back to the provider default. The credential source itself is untouched.";
const PERSONAL_CRUMB: &str = "Accounts › Claude › Personal";
const WORK_CRUMB: &str = "Accounts › Claude › Work";
const PRIMARY_CRUMB: &str = "Accounts › Codex › Primary";
const EXP_CRUMB: &str = "Accounts › Codex › Experiments";
const GO_CRUMB: &str = "Accounts › OpenCode › Go subscription";
const REVOKED_CRUMB: &str = "Accounts › Grok › Revoked key";
const CI_CRUMB: &str = "Accounts › OpenCode › ci-bot";
const KIMI_CRUMB: &str = "Accounts › Kimi › discovered";
const WORK_REMOVED: &str = "Removed Claude · Work";
const EDITOR: &str = "Workspaces › payments-platform › edit";
const ACTIVE: &str = "Active accounts";
const INHERITED: &str = "inherited default";
const CANCEL: &str = "Cancel";
const REMOVE: &str = "Remove";
const WIDE: &str = "J04.accounts.accounts.120x40";
const NARROW: &str = "J04.accounts.accounts.narrow";

const SECRETS: &[&str] = &[
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
        "pre_refactor/j04_accounts/{}",
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
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    app.leave_intro("manager after intro", |screen| {
        text(screen).contains(DIRECTORY)
    })
}

fn dialog_columns(screen: &TerminalSnapshot) -> Option<(u16, u16)> {
    for row in 0..screen.rows() {
        let mut pieces = Vec::new();
        let mut column = 0u16;
        for cell in screen.row(row)? {
            if !cell.text().is_empty() {
                pieces.push((column, cell.text()));
            }
            column = column.saturating_add(cell.width());
        }
        let line: String = pieces.iter().map(|(_, piece)| *piece).collect();
        let Some(title_at) = line.find("Remove account ") else {
            continue;
        };
        let mut byte = 0usize;
        let mut title_col = None;
        for (col, piece) in &pieces {
            let next = byte + piece.len();
            if title_at >= byte && title_at < next {
                title_col = Some(*col);
                break;
            }
            byte = next;
        }
        let title_col = title_col?;
        let left = pieces
            .iter()
            .rev()
            .find(|(col, piece)| *col < title_col && *piece == "│")
            .map(|(col, _)| *col)?;
        let right = pieces
            .iter()
            .find(|(col, piece)| *col > title_col && *piece == "│")
            .map(|(col, _)| *col)?;
        return Some((left, right));
    }
    None
}

fn dialog_body(screen: &TerminalSnapshot) -> String {
    let Some((left, right)) = dialog_columns(screen) else {
        return String::new();
    };
    let mut parts = Vec::new();
    for row in 0..screen.rows() {
        let mut column = 0u16;
        let mut inner = String::new();
        for cell in screen.row(row).expect("row") {
            if column > left && column < right {
                inner.push_str(cell.text());
            }
            column = column.saturating_add(cell.width());
        }
        let inner = inner.trim();
        if !inner.is_empty() {
            parts.push(inner.to_string());
        }
    }
    parts.join(" ")
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn assert_secrets(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for secret in SECRETS {
        assert!(
            !visible.contains(secret),
            "raw secret {secret} is visible\n{visible}"
        );
    }
}

fn secrets_clear(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    SECRETS.iter().all(|secret| !visible.contains(secret))
}

fn select_until(
    app: &App,
    label: &str,
    pred: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    app.key("g");
    let mut screen = app.wait_screen("tree top", |screen| {
        pred(screen) || focused_line(screen).contains("Overview")
    });
    for _ in 0..40 {
        if pred(&screen) {
            assert_secrets(&screen);
            return screen;
        }
        let before = focused_line(&screen);
        app.key("down");
        screen = app.wait_screen(label, |screen| {
            focused_line(screen) != before || pred(screen)
        });
        if focused_line(&screen) == before {
            break;
        }
    }
    panic!("{label}\n{}", text(&screen));
}

fn shows(crumb: &str) -> impl Fn(&TerminalSnapshot) -> bool {
    let crumb = crumb.to_string();
    move |screen| text(screen).contains(&crumb)
}

fn revoked_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(REVOKED_CRUMB) && visible.contains(UNAUTHORIZED) && visible.contains(HTTP_401)
}

/// The walk can already be on the rejection. A later `screen` call sees the
/// cursor on the next provider, so the matching frame is the one to keep.
fn take_revoked(app: &App) -> TerminalSnapshot {
    let started = Instant::now();
    loop {
        let selected = select_until(app, "revoked", shows(REVOKED_CRUMB));
        if revoked_ready(&selected) {
            return selected;
        }
        let posted = Instant::now();
        loop {
            let screen = app.screen();
            if revoked_ready(&screen) {
                return screen;
            }
            if !text(&screen).contains(REVOKED_CRUMB) || posted.elapsed() > Duration::from_secs(8) {
                break;
            }
            assert!(
                app.child_running(),
                "jackin exited before the revoked detail\n{}",
                text(&screen)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "timed out: revoked detail\n{}",
            text(&app.screen())
        );
    }
}

fn shows_both(screen: &TerminalSnapshot, crumb: &str, needle: &str) -> bool {
    let visible = text(screen);
    visible.contains(crumb) && visible.contains(needle)
}

/// The crumb is painted before the account issue. A later frame can also
/// leave the row, so keep polling only while the crumb stays.
fn hold_detail(app: &App, label: &str, crumb: &str, needle: &str) -> TerminalSnapshot {
    let started = Instant::now();
    loop {
        let selected = select_until(app, label, shows(crumb));
        if shows_both(&selected, crumb, needle) {
            return selected;
        }
        let posted = Instant::now();
        loop {
            let screen = app.screen();
            if shows_both(&screen, crumb, needle) {
                return screen;
            }
            if !text(&screen).contains(crumb) || posted.elapsed() > Duration::from_secs(8) {
                break;
            }
            assert!(
                app.child_running() && started.elapsed() < Duration::from_secs(20),
                "timed out: {label}\n{}",
                text(&screen)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "timed out: {label}\n{}",
            text(&app.screen())
        );
    }
}

/// `d` can land after the cursor has already moved to the next provider.
/// Press it only while the crumb is still showing, and press again if the
/// dialog does not open.
fn remove_dialog(
    app: &App,
    label: &str,
    crumb: &str,
    ready: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let started = Instant::now();
    loop {
        if !text(&app.screen()).contains(crumb) {
            select_until(app, label, shows(crumb));
        }
        if !text(&app.screen()).contains(crumb) {
            assert!(
                started.elapsed() < Duration::from_secs(25),
                "timed out: {label}\n{}",
                text(&app.screen())
            );
            continue;
        }
        app.key("d");
        let posted = Instant::now();
        loop {
            let screen = app.screen();
            if ready(&screen) {
                return screen;
            }
            assert!(
                app.child_running() && started.elapsed() < Duration::from_secs(25),
                "timed out: {label}\n{}",
                text(&screen)
            );
            if posted.elapsed() > Duration::from_millis(900) {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

fn remove_hit(screen: &TerminalSnapshot) -> Option<Point> {
    let hits: Vec<_> = find_all(screen, REMOVE)
        .into_iter()
        .filter(|point| {
            let row = row_text(screen, u32::from(point.y));
            row.contains(CANCEL) && row.contains(REMOVE) && row.contains('│')
        })
        .collect();
    (hits.len() == 1).then(|| Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    })
}

/// The restore to 120×40 paints the dialog and a later frame drops it before
/// a fresh `screen` call. Re-open when the title is gone, then click the row
/// that still shows Cancel and Remove.
fn remove_button(app: &App) -> Point {
    let started = Instant::now();
    let mut reopened = false;
    loop {
        let screen = app.screen();
        if let Some(point) = remove_hit(&screen) {
            return point;
        }
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "one button {REMOVE:?}:\n{}",
            text(&screen)
        );
        if !reopened && !text(&screen).contains("Remove account") {
            app.key("d");
            reopened = true;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet_frame, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    nudge(app, quiet_frame);
    app.visual(&visual_id(checkpoint));
}

fn status_exact(screen: &TerminalSnapshot, status: &str) {
    let status_line = text(screen)
        .lines()
        .find(|line| line.contains(status))
        .unwrap_or("")
        .to_string();
    let at = status_line
        .find(status)
        .unwrap_or_else(|| panic!("status is not exactly {status}\n{status_line}"));
    let after = &status_line[at + status.len()..];
    assert!(
        !after.starts_with(|c: char| c.is_ascii_alphanumeric()),
        "status is not exactly {status}\n{status_line}"
    );
}

fn quiet_accounts(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // reset_label speaks Work's retry with seconds, so the seam drag narrows
    // the inspector until that row is below the viewport.
    !visible.contains("retry") && !visible.contains(" s ·") && secrets_clear(screen)
}

fn work_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(WORK_TITLE)
        && dialog_body(screen).contains(WORK_BODY)
        && focused_line(screen).contains(CANCEL)
        && visible.contains(REMOVE)
        && !visible.contains("refreshing")
        && !visible.contains(REVOKED)
        && !visible.contains(EXPERIMENTS)
        && quiet_accounts(screen)
}

fn work_title(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(WORK_TITLE)
        && focused_line(screen).contains(CANCEL)
        && !visible.contains("refreshing")
        && !visible.contains(REVOKED)
        && quiet_accounts(screen)
}

fn seam_point(screen: &TerminalSnapshot) -> Point {
    let row = (0..screen.rows())
        .find(|row| row_text(screen, *row).contains("Provider"))
        .expect("inspector provider row");
    let line = row_text(screen, row);
    let provider_at = line.find("Provider").expect("provider label");
    let mut byte = 0usize;
    let mut column = 0u16;
    let mut seam = None;
    for cell in screen.row(row).expect("row") {
        let piece = cell.text();
        let next = byte + piece.len();
        if piece == "│" && byte < provider_at {
            seam = Some(column);
        }
        byte = next;
        column = column.saturating_add(cell.width());
    }
    Point {
        x: seam.expect("seam"),
        y: u16::try_from(row).expect("row"),
    }
}

fn on_work(screen: &TerminalSnapshot) -> bool {
    text(screen).contains(WORK_CRUMB)
}

fn work_settled(screen: &TerminalSnapshot) -> bool {
    on_work(screen) && quiet_accounts(screen)
}

fn hide_retry(app: &App) {
    let seam = seam_point(&app.screen());
    app.move_to(seam);
    app.down(seam);
    let parked = Point { x: 100, y: seam.y };
    app.drag_to(parked);
    app.up(parked);
    // The seam drag can move the selection off Work. A poll cannot put it
    // back, so recover with keys and only then let the caller press `d`.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let screen = app.screen();
        if !on_work(&screen) {
            select_until(app, "work after seam", shows(WORK_CRUMB));
            continue;
        }
        if work_settled(&screen) && app.try_sustained(Duration::from_millis(200), work_settled) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "retry out of view\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn checkbox_lines(screen: &TerminalSnapshot) -> Vec<String> {
    text(screen)
        .lines()
        .filter(|line| line.contains("[✓]") || line.contains("[ ]"))
        .map(str::to_string)
        .collect()
}

fn show_mixed(app: &App) {
    // The crumb is painted before the tree and the warning list. Waiting on
    // the crumb alone accepts a partial frame that is missing Go or Kimi.
    let overview = app.wait_screen("mixed overview", |screen| {
        let visible = text(screen);
        visible.contains(OVERVIEW)
            && visible.contains(PERSONAL)
            && visible.contains(WORK)
            && visible.contains(PRIMARY)
            && visible.contains(EXPERIMENTS)
            && visible.contains(GO)
            && !visible.contains(REVOKED)
            && visible.contains("1Password available")
            && visible.contains(MISSING)
            && secrets_clear(screen)
    });
    assert_secrets(&overview);
    let visible = text(&overview);
    assert!(visible.contains(PERSONAL), "{visible}");
    assert!(visible.contains(WORK), "{visible}");
    assert!(visible.contains(PRIMARY), "{visible}");
    assert!(visible.contains(EXPERIMENTS), "{visible}");
    assert!(visible.contains(GO), "{visible}");
    assert!(!visible.contains(REVOKED), "{visible}");
    assert!(visible.contains("1Password available"), "{visible}");
    assert!(visible.contains(MISSING), "{visible}");

    let personal = select_until(app, "personal", shows(PERSONAL_CRUMB));
    let visible = text(&personal);
    assert!(visible.contains(LOCAL), "{visible}");
    assert!(visible.contains(HOME_CLAUDE), "{visible}");
    assert!(visible.contains(OAUTH), "{visible}");
    assert!(visible.contains(PERSONAL_ID), "{visible}");
    assert!(visible.contains(PLAN_MAX), "{visible}");

    let work = select_until(app, "work", shows(WORK_CRUMB));
    let visible = text(&work);
    assert!(visible.contains(ONE_PASSWORD), "{visible}");
    // The display path wraps, so the field label is on the next line.
    assert!(
        visible.contains("Engineering › Anthropic · Work"),
        "{visible}"
    );
    assert!(visible.contains(WORK_REF), "{visible}");
    assert!(visible.contains(WORK_MASK), "{visible}");
    assert!(visible.contains(WORK_ID), "{visible}");
    assert!(visible.contains(PAYMENTS), "{visible}");

    let experiments = select_until(app, "experiments", shows(EXP_CRUMB));
    let visible = text(&experiments);
    assert!(visible.contains(API_KEY), "{visible}");
    assert!(visible.contains(EXP_FP), "{visible}");
    assert!(visible.contains(EXP_TAIL), "{visible}");
    assert!(visible.contains(EXP_MASK), "{visible}");

    let go = select_until(app, "go subscription", shows(GO_CRUMB));
    let visible = text(&go);
    assert!(visible.contains(API_KEY), "{visible}");
    assert!(visible.contains(GO_FP), "{visible}");
    assert!(visible.contains(GO_TAIL), "{visible}");
    assert!(visible.contains(GO_MASK), "{visible}");
    assert!(visible.contains(GO_ID), "{visible}");
}

fn open_accounts(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(DIRECTORY),
        "refusing c unless the manager is showing\n{}",
        text(&before)
    );
    app.key("c");
    app.wait_text(OVERVIEW)
}

#[test]
fn j04_accounts_route() {
    let app = open("j04-accounts-route", "returning");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let accounts = open_accounts(&app);
    assert!(text(&accounts).contains(OVERVIEW), "{}", text(&accounts));
    assert!(text(&accounts).contains(PERSONAL), "{}", text(&accounts));
    assert!(!text(&accounts).contains(REVOKED), "{}", text(&accounts));
    assert_secrets(&accounts);
    assert_eq!((accounts.cols(), accounts.rows()), (120, 40));
    app.key("escape");
    let back = app.wait_text(DIRECTORY);
    assert!(!text(&back).contains(OVERVIEW), "{}", text(&back));
    assert_secrets(&back);
    app.finish();
}

#[test]
fn j04_accounts_scenario_accounts_mixed() {
    let app = open("j04-accounts-scenario-accounts-mixed", "accounts-mixed");
    show_mixed(&app);
    let missing = hold_detail(&app, "kimi", KIMI_CRUMB, MISSING);
    assert!(text(&missing).contains(MISSING), "{}", text(&missing));
    let denied = hold_detail(&app, "ci-bot", CI_CRUMB, DENIED);
    assert!(text(&denied).contains(DENIED), "{}", text(&denied));
    assert!(!text(&denied).contains(REVOKED), "{}", text(&denied));
    assert_secrets(&denied);
    app.finish();
}

#[test]
fn j04_accounts_accounts() {
    let first = open("j04-accounts-first-use", "first-use");
    reach_manager(&first);
    first.key("c");
    let empty = first.wait_text(EMPTY);
    let visible = text(&empty);
    assert!(visible.contains("a adds one"), "{visible}");
    assert!(
        visible.contains("1Password is the default credential source"),
        "{visible}"
    );
    assert!(visible.contains("Claude"), "{visible}");
    assert!(visible.contains("+ Add account…"), "{visible}");
    assert!(!visible.contains(PERSONAL), "{visible}");
    assert!(!visible.contains(EXPERIMENTS), "{visible}");
    assert!(!visible.contains(REVOKED), "{visible}");
    assert!(!visible.contains("discovery pending"), "{visible}");
    assert_secrets(&empty);
    first.finish();

    let returning = open("j04-accounts-returning", "returning");
    reach_manager(&returning);
    returning.key("c");
    show_mixed(&returning);
    returning.finish();

    let hard = open("j04-accounts-hard-cases", "hard-cases");
    reach_manager(&hard);
    hard.key("c");
    let locked = hard.wait_text(LOCKED);
    assert!(
        text(&locked).contains("1Password locked"),
        "{}",
        text(&locked)
    );
    assert!(text(&locked).contains(REVOKED), "{}", text(&locked));
    assert_secrets(&locked);
    let revoked = take_revoked(&hard);
    let visible = text(&revoked);
    assert!(visible.contains(UNAUTHORIZED), "{visible}");
    assert!(visible.contains(HTTP_401), "{visible}");
    assert!(visible.contains(ONE_PASSWORD), "{visible}");
    assert_secrets(&revoked);
    hard.finish();

    let app = open("j04-accounts-accounts", "accounts-mixed");
    let opened = app.wait_screen("accounts overview", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && text(screen).contains(OVERVIEW)
    });
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    show_mixed(&app);
    let missing = hold_detail(&app, "kimi", KIMI_CRUMB, MISSING);
    assert!(text(&missing).contains(MISSING), "{}", text(&missing));
    let denied = hold_detail(&app, "ci-bot", CI_CRUMB, DENIED);
    assert!(text(&denied).contains(DENIED), "{}", text(&denied));

    let primary = select_until(&app, "primary", shows(PRIMARY_CRUMB));
    assert!(text(&primary).contains(ONE_PASSWORD), "{}", text(&primary));
    app.key("t");
    let disabled = app.wait_text(DISABLED);
    status_exact(&disabled, DISABLED);
    assert_secrets(&disabled);
    app.key("t");
    let enabled = app.wait_text(ENABLED);
    status_exact(&enabled, ENABLED);
    app.key("space");
    let chosen = app.wait_text(DEFAULT);
    status_exact(&chosen, DEFAULT);
    assert_secrets(&chosen);

    let experiments = hold_detail(&app, "experiments again", EXP_CRUMB, EXP_FP);
    assert!(
        text(&experiments).contains(EXP_FP),
        "{}",
        text(&experiments)
    );
    let exp_dialog = remove_dialog(&app, "experiments again", EXP_CRUMB, |screen| {
        text(screen).contains(EXP_TITLE) && focused_line(screen).contains(CANCEL)
    });
    let exp_body = dialog_body(&exp_dialog);
    assert!(
        exp_body.contains(EXP_BODY),
        "experiments dialog\nbody: {exp_body}\n{}",
        text(&exp_dialog)
    );
    assert!(
        text(&exp_dialog).contains(EXPERIMENTS),
        "{}",
        text(&exp_dialog)
    );
    assert_secrets(&exp_dialog);
    app.click(remove_button(&app));
    let exp_gone = app.wait_text(EXP_REMOVED);
    status_exact(&exp_gone, EXP_REMOVED);
    assert!(!text(&exp_gone).contains(EXP_TITLE), "{}", text(&exp_gone));
    assert_secrets(&exp_gone);
    app.wait_screen("spinner stopped", |screen| {
        let visible = text(screen);
        !visible.contains("Removed")
            && !visible.contains("refreshing")
            && !visible.contains(EXPERIMENTS)
            && !visible.contains(EXP_TITLE)
    });

    select_until(&app, "work again", shows(WORK_CRUMB));
    hide_retry(&app);
    let dialog = remove_dialog(&app, "work again", WORK_CRUMB, |screen| {
        screen.cols() == 120 && screen.rows() == 40 && work_dialog(screen)
    });
    assert!(
        work_dialog(&dialog),
        "open remove dialog\n{}",
        text(&dialog)
    );
    assert!(
        text(&dialog).contains(WORK_CRUMB) || dialog_body(&dialog).contains("Claude · Work"),
        "account still registered\n{}",
        text(&dialog)
    );
    shoot(&app, WIDE, work_dialog);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && work_title(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shoot(&app, NARROW, work_title);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && work_dialog(screen)
    });

    app.click(remove_button(&app));
    let removed = app.wait_text(WORK_REMOVED);
    status_exact(&removed, WORK_REMOVED);
    let visible = text(&removed);
    assert!(!visible.contains(WORK_CRUMB), "{visible}");
    assert!(!visible.contains(WORK_TITLE), "{visible}");
    assert!(visible.contains(PERSONAL), "{visible}");
    assert!(!visible.contains(EXPERIMENTS), "{visible}");
    assert_secrets(&removed);

    app.key("escape");
    let manager = app.wait_text(DIRECTORY);
    assert!(!text(&manager).contains(OVERVIEW), "{}", text(&manager));
    app.key("e");
    let editor = app.wait_text(EDITOR);
    assert_secrets(&editor);
    app.key("[");
    let bindings = app.wait_screen("workspace accounts", |screen| {
        text(screen).contains(ACTIVE) && text(screen).contains(INHERITED)
    });
    assert!(text(&bindings).contains(EDITOR), "{}", text(&bindings));
    let rows: Vec<_> = checkbox_lines(&bindings)
        .into_iter()
        .filter(|line| !line.contains("Switch tab"))
        .collect();
    assert!(
        rows.iter()
            .any(|line| line.contains(PERSONAL) && line.contains(INHERITED)),
        "personal is not the inherited default\n{}",
        text(&bindings)
    );
    assert!(
        rows.iter().all(|line| !line.contains(WORK)),
        "work is still a workspace binding\n{}",
        text(&bindings)
    );
    assert_secrets(&bindings);
    app.key("escape");
    let back = app.wait_text(DIRECTORY);
    assert!(!text(&back).contains(ACTIVE), "{}", text(&back));
    app.finish();
}
