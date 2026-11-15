//! Holla action families, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h04_actions/` and each checkpoint's dots become underscores.
//!
//! The shots are the git Push confirmation. A live activity redraws a spinner
//! every 80 ms (`App::animating`, `TICK_MS` in `src/bin/holla/sim/world.rs`),
//! so the frame is taken before Run. Cancel is Esc: `Dismiss::LIGHT` closes
//! `open_confirm_one` without `execute_item` (`src/bin/holla/app.rs`).

use std::time::Duration;

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, text, try_launch};

const PUSH: &str = "git -C /Users/alex/work/svc push";
const ALPHA_GITLAB: &str = "git -C /Users/alex/work/repos/alpha push gitlab";
const BETA_ORIGIN: &str = "git -C /Users/alex/work/repos/beta push origin";
const BETA_GITLAB: &str = "git -C /Users/alex/work/repos/beta push gitlab";
const DELTA_ORIGIN: &str = "git -C /Users/alex/work/repos/delta push origin";
const CARGO_TEST: &str = "cargo test";
const COMPOSE_DOWN: &str = "docker compose down";
const DOCKER_STOP: &str = "docker stop acme-api-1";
const BREW_STOP: &str = "brew services stop svc02";
const GRADLE_BUILD: &str = "gradle build";
const MISE: &str = "mise upgrade";
const CONFIRM: &str = "one explicit confirmation";
const HOST: &str = "mbp · local";
const HOST_WIDE: &str = "mbp · host-wide";
const SVC: &str = "~/work/svc";
const REPOS: &str = "~/work/repos";
const ENGINE: &str = "~/work/engine";
const STACK: &str = "~/work/stack";
const ANDROID: &str = "~/work/android";
const IDE: &str = "~/work/ide";
const REJECTED: &str = "[rejected]";
const FETCH_FIRST: &str = "fetch first";
const CORRUPT: &str = "index file corrupt";
const EADDR: &str = "EADDRINUSE";
const FRONTEND: &str = "frontend dev";
const WIDE: &str = "H04.actions.families.120x40";
const NARROW: &str = "H04.actions.families.narrow";
const BATCH: &str = "Push 4 repositories to origin and GitLab";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h04_actions/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str, scenario: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = ["--scenario", scenario]
        .into_iter()
        .map(str::to_string)
        .collect();
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Search actions");
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent.iter().chain([EADDR, FRONTEND].iter()) {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

/// `›` marks the selected row. The name column ends at the next run of two
/// spaces. A row cut with `…` matches when that stem is a prefix of `label`
/// (`Status across 5 child pr…`). A longer name that merely begins with the
/// same word (`Push dry run`) does not match.
fn selected(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen).lines().any(|line| {
        let Some(mark) = line.find('›') else {
            return false;
        };
        let rest = line[mark + '›'.len_utf8()..].trim_start();
        let name = rest.split("  ").next().unwrap_or(rest).trim();
        if name == label {
            return true;
        }
        let Some(stem) = name.strip_suffix('…') else {
            return false;
        };
        let stem = stem.trim_end();
        stem.chars().count() >= 8 && label.starts_with(stem)
    })
}

fn pick(app: &App, query: &str, label: &str, marker: &str) -> TerminalSnapshot {
    app.paste(query);
    app.wait_screen(&format!("{label} selected"), |screen| {
        selected(screen, label) && text(screen).contains(marker)
    })
}

fn enter_until(app: &App, reason: &str, marker: &str) -> TerminalSnapshot {
    app.key("enter");
    app.wait_screen(reason, |screen| text(screen).contains(marker))
}

/// The activity header is `{name} · {state} · {elapsed} · exit {code}`.
/// A sibling's status line (`gamma finished · exit 0`) also contains
/// ` · exit `, so the header's own state word has to be on that line.
fn finished(screen: &TerminalSnapshot, marker: &str) -> bool {
    let visible = text(screen);
    visible.contains(marker)
        && visible.lines().any(|line| {
            line.contains(" · exit ")
                && (line.contains("succeeded")
                    || line.contains("failed")
                    || line.contains("stopped"))
        })
}

fn run_until(app: &App, reason: &str, marker: &str) -> TerminalSnapshot {
    app.key("enter");
    app.wait_screen(reason, |screen| finished(screen, marker))
}

fn push_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(PUSH)
        && visible.contains(CONFIRM)
        && visible.contains(SVC)
        && visible.contains(HOST)
        && !visible.contains("--dry-run")
        && !visible.contains(REJECTED)
        && !visible.contains(CARGO_TEST)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the command stays put.
fn nudge(app: &App, quiet: impl Fn(&TerminalSnapshot) -> bool) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(quiet(&screen), "nudge left the frame\n{}", text(&screen));
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet(screen)
    });
    app.resize(cols, rows);
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && quiet(screen)
    });
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn clear_query(app: &App) {
    app.key("escape");
    app.wait_text("Search actions");
}

/// Leave a page or a dialog. The finder keeps the query, so the placeholder
/// `Search actions` is absent until a second Esc clears it.
fn leave(app: &App) {
    app.key("escape");
    app.wait_screen("finder", |screen| {
        let visible = text(screen);
        visible.contains("Results") || visible.contains("Search actions")
    });
    if !text(&app.screen()).contains("Search actions") {
        app.key("escape");
    }
    app.wait_text("Search actions");
}

#[test]
fn h04_actions_route() {
    let app = open("h04-actions-route", "parity-git-current");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Search actions", SVC, "feature/x", "↓2", "↑1"],
        &[REPOS, ENGINE, REJECTED, CONFIRM],
    );
    let row = pick(&app, "Push", "Push", "one confirmation");
    see(
        &row,
        &[SVC, HOST, "one confirmation"],
        &["--dry-run", CARGO_TEST, COMPOSE_DOWN, REJECTED],
    );
    let dialog = enter_until(&app, "push confirmation", PUSH);
    assert!(push_ready(&dialog), "push confirmation\n{}", text(&dialog));
    see(
        &dialog,
        &[PUSH, SVC, HOST, "Cancel", "Run"],
        &["--dry-run", CARGO_TEST, REJECTED, "failed to push"],
    );
    app.key("escape");
    let finder = app.wait_screen("push cancelled", |screen| {
        let visible = text(screen);
        visible.contains("Cancelled · nothing was executed")
            && selected(screen, "Push")
            && !visible.contains(CONFIRM)
            && !visible.contains(REJECTED)
    });
    see(
        &finder,
        &[
            "Cancelled · nothing was executed",
            SVC,
            "feature/x",
            "Results",
        ],
        &[CONFIRM, REJECTED, "failed to push", REPOS],
    );
    app.finish();
}

#[test]
fn h04_actions_families() {
    git_batch_family();
    direct_family(
        "h04-actions-cargo",
        "parity-cargo",
        "cargo test",
        "cargo test",
        CARGO_TEST,
        ENGINE,
        "aliases_beat_everything",
        &[REPOS, SVC, PUSH, COMPOSE_DOWN],
    );
    confirmed_family(
        "h04-actions-compose",
        "parity-docker",
        "Stop the Compose project",
        "Stop the Compose project",
        COMPOSE_DOWN,
        STACK,
        "Network acme_default",
        &[DOCKER_STOP, "cannot stop container", ENGINE],
    );
    confirmed_family(
        "h04-actions-docker",
        "parity-docker",
        "Stop acme-api-1",
        "Stop acme-api-1",
        DOCKER_STOP,
        HOST_WIDE,
        "cannot stop container: acme-api-1",
        &[COMPOSE_DOWN, "Network acme_default", ENGINE],
    );
    direct_family(
        "h04-actions-brew",
        "parity-brew-services",
        "Stop svc02",
        "Stop svc02",
        BREW_STOP,
        HOST_WIDE,
        "Input/output error",
        &[CARGO_TEST, COMPOSE_DOWN, MISE, "svc00"],
    );
    direct_family(
        "h04-actions-gradle",
        "parity-gradle",
        "gradle build",
        "gradle build",
        GRADLE_BUILD,
        ANDROID,
        "Unresolved reference: ledger",
        &[CARGO_TEST, IDE, REPOS],
    );
    idea_family();
    confirmed_family(
        "h04-actions-mise",
        "parity-upgrade-managers",
        "Upgrade mise-managed tools",
        "Upgrade mise-managed tools",
        MISE,
        HOST_WIDE,
        "2 tools upgraded",
        &[BREW_STOP, "brew upgrade", ENGINE, REPOS],
    );
    git_push_family();
}

fn git_push_family() {
    let app = open("h04-actions-git", "parity-git-current");
    let row = pick(&app, "Push", "Push", "one confirmation");
    see(
        &row,
        &[SVC, HOST, "one confirmation"],
        &["--dry-run", CARGO_TEST, REPOS, ENGINE],
    );
    let dialog = enter_until(&app, "push confirmation", PUSH);
    assert!(push_ready(&dialog), "push confirmation\n{}", text(&dialog));
    nudge(&app, push_ready);
    shot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("push at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && push_ready(screen)
    });
    assert!(
        text(&narrow).contains(PUSH) || text(&narrow).contains("Push"),
        "narrow push confirmation dropped the command\n{}",
        text(&narrow)
    );
    nudge(&app, push_ready);
    shot(&app, NARROW);
    app.resize(120, 40);
    let restored = app.wait_screen("push back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && push_ready(screen)
    });
    see(&restored, &[PUSH, SVC], &[REJECTED, CARGO_TEST]);
    let ran = run_until(&app, "push rejected", FETCH_FIRST);
    see(
        &ran,
        &[
            PUSH,
            SVC,
            "feature/x",
            REJECTED,
            "failed to push",
            " · exit ",
        ],
        &[REPOS, CARGO_TEST, ENGINE, CORRUPT],
    );
    app.finish();
}

fn git_batch_family() {
    let app = open("h04-actions-batch", "parity-git-batch");
    let row = pick(&app, "origin and GitLab", BATCH, "one confirmation");
    see(
        &row,
        &[REPOS, HOST, "one confirmation"],
        &[
            CARGO_TEST,
            COMPOSE_DOWN,
            CORRUPT,
            "git@gitlab.com",
            BETA_GITLAB,
        ],
    );
    let dialog = enter_until(&app, "batch confirmation", ALPHA_GITLAB);
    see(
        &dialog,
        &[
            BATCH,
            ALPHA_GITLAB,
            BETA_ORIGIN,
            DELTA_ORIGIN,
            REPOS,
            HOST,
            CONFIRM,
        ],
        &[BETA_GITLAB, CARGO_TEST, CORRUPT, "git@gitlab.com"],
    );
    let alpha = run_until(&app, "alpha push", "To github.com:acme/alpha.git");
    see(
        &alpha,
        &[
            "push origin · alpha",
            "git -C /Users/alex/work/repos/alpha push origin",
            "~/work/repos/alpha",
            "To github.com:acme/alpha.git",
            " · exit ",
        ],
        &[
            "/repos/beta",
            "/repos/delta",
            CORRUPT,
            "acme/beta",
            BETA_GITLAB,
        ],
    );
    app.wait_screen("batch idle", |screen| {
        let visible = text(screen);
        visible.contains("4 ok") && visible.contains("4 failed") && !visible.contains(" running")
    });
    app.finish();
}

#[allow(clippy::too_many_arguments)]
fn direct_family(
    name: &str,
    scenario: &str,
    query: &str,
    label: &str,
    command: &str,
    directory: &str,
    effect: &str,
    absent: &[&str],
) {
    let app = open(name, scenario);
    let row = pick(&app, query, label, command);
    see(&row, &[command, directory, HOST], absent);
    let ran = run_until(&app, effect, effect);
    see(&ran, &follow_up(command, directory, effect), absent);
    app.finish();
}

#[allow(clippy::too_many_arguments)]
fn confirmed_family(
    name: &str,
    scenario: &str,
    query: &str,
    label: &str,
    command: &str,
    directory: &str,
    effect: &str,
    absent: &[&str],
) {
    let app = open(name, scenario);
    let row = pick(&app, query, label, command);
    see(
        &row,
        &[command, directory, HOST, "one confirmation"],
        absent,
    );
    let dialog = enter_until(&app, "confirmation", CONFIRM);
    see(&dialog, &[command, directory, HOST, CONFIRM, "Run"], absent);
    let ran = run_until(&app, effect, effect);
    see(&ran, &follow_up(command, directory, effect), absent);
    app.finish();
}

/// A host-wide action draws `mbp · host-wide` on the confirmation. The
/// activity header keeps the host and the command, not that phrase.
fn follow_up<'a>(command: &'a str, directory: &'a str, effect: &'a str) -> Vec<&'a str> {
    let mut present = vec![command, effect, " · exit "];
    if directory.starts_with('~') {
        present.push(directory);
    }
    present
}

fn idea_family() {
    let app = open("h04-actions-idea", "parity-idea");
    let row = pick(
        &app,
        "IntelliJ metadata",
        "Clean IntelliJ metadata under ~/work/ide",
        "4 candidates",
    );
    see(
        &row,
        &[
            "no command · opens",
            "inside holla",
            IDE,
            HOST,
            "opens a review",
        ],
        &[CARGO_TEST, GRADLE_BUILD, MISE, "git -C"],
    );
    let review = enter_until(&app, "idea review", "app.iml");
    see(
        &review,
        &["IntelliJ metadata", "app.iml", IDE, ".idea"],
        &[CARGO_TEST, "moved to Trash", GRADLE_BUILD],
    );
    leave(&app);
    app.finish();
}

#[test]
fn h04_actions_scenario_parity_git_current() {
    let app = open("h04-actions-scenario-git", "parity-git-current");
    let landing = app.screen();
    see(
        &landing,
        &[SVC, "feature/x", "↓2", "↑1"],
        &[REPOS, ENGINE, EADDR, FRONTEND],
    );
    pick(&app, "Show git status", "Show git status", "git status");
    let status = enter_until(&app, "git status", "src/api/mod.rs");
    see(
        &status,
        &[
            "Git status",
            SVC,
            "feature/x",
            "2 behind",
            "1 ahead",
            " M src/api/mod.rs",
        ],
        &[REPOS, ENGINE, CORRUPT],
    );
    app.finish();
}

#[test]
fn h04_actions_scenario_parity_git_batch() {
    let app = open("h04-actions-scenario-batch", "parity-git-batch");
    let landing = app.screen();
    see(&landing, &[REPOS], &[ENGINE, SVC, STACK]);
    pick(
        &app,
        "Status across",
        "Status across 5 child projects",
        "0 dirty · 1 behind",
    );
    let status = enter_until(&app, "child status", "↓3");
    see(
        &status,
        &[REPOS, "alpha", "beta", "gamma", "delta", "↓3"],
        &[CORRUPT, ENGINE, "git@gitlab.com"],
    );
    leave(&app);
    pick(&app, "origin and GitLab", BATCH, "one confirmation");
    let dialog = enter_until(&app, "batch confirmation", ALPHA_GITLAB);
    see(
        &dialog,
        &[ALPHA_GITLAB, BETA_ORIGIN, DELTA_ORIGIN, REPOS],
        &[BETA_GITLAB, CORRUPT, ENGINE],
    );
    app.key("escape");
    clear_query(&app);
    app.finish();
}

#[test]
fn h04_actions_scenario_parity_cargo() {
    let app = open("h04-actions-scenario-cargo", "parity-cargo");
    let landing = app.screen();
    see(&landing, &[ENGINE], &[REPOS, SVC, STACK]);
    pick(&app, "cargo test", "cargo test", CARGO_TEST);
    let tests = run_until(&app, "cargo test failed", "aliases_beat_everything");
    see(
        &tests,
        &[
            CARGO_TEST,
            ENGINE,
            "aliases_beat_everything",
            "1 failed",
            "/Users/alex/work/engine",
        ],
        &[REPOS, SVC, "2 warnings"],
    );
    leave(&app);
    pick(&app, "cargo clippy", "cargo clippy", "cargo clippy");
    let clippy = run_until(&app, "clippy warnings", "scope_0");
    see(
        &clippy,
        &["cargo clippy", ENGINE, "scope_0", "scope_1", "2 warnings"],
        &[REPOS, "aliases_beat_everything", SVC],
    );
    app.finish();
}

#[test]
fn h04_actions_scenario_parity_docker() {
    let app = open("h04-actions-scenario-docker", "parity-docker");
    let landing = app.screen();
    see(&landing, &[STACK], &[ENGINE, REPOS, SVC]);
    pick(
        &app,
        "Show Compose services",
        "Show Compose services",
        "compose ps",
    );
    let services = enter_until(&app, "compose services", "Compose · acme");
    see(
        &services,
        &["Compose · acme", "acme-db-1", "acme-api-1", "acme-worker-1"],
        &[ENGINE, "cannot stop container"],
    );
    leave(&app);
    pick(
        &app,
        "Stop the Compose project",
        "Stop the Compose project",
        COMPOSE_DOWN,
    );
    let dialog = enter_until(&app, "compose confirmation", CONFIRM);
    see(
        &dialog,
        &[COMPOSE_DOWN, STACK, "acme", HOST, CONFIRM],
        &[DOCKER_STOP, ENGINE],
    );
    app.key("escape");
    clear_query(&app);
    pick(&app, "Stop acme-api-1", "Stop acme-api-1", DOCKER_STOP);
    let stopped = {
        let preview = app.screen();
        see(
            &preview,
            &[DOCKER_STOP, HOST_WIDE],
            &[COMPOSE_DOWN, "cannot stop container"],
        );
        enter_until(&app, "stop confirmation", CONFIRM);
        run_until(&app, "stop failed", "cannot stop container: acme-api-1")
    };
    see(
        &stopped,
        &[
            DOCKER_STOP,
            "cannot stop container: acme-api-1",
            "acme-api-1",
        ],
        &[COMPOSE_DOWN, "Network acme_default", ENGINE],
    );
    app.finish();
}

#[test]
fn h04_actions_scenario_parity_brew_services() {
    let app = open("h04-actions-scenario-brew", "parity-brew-services");
    let landing = app.screen();
    see(&landing, &["~/work"], &[ENGINE, REPOS, STACK, SVC]);
    let row = pick(&app, "Stop svc02", "Stop svc02", BREW_STOP);
    see(
        &row,
        &[BREW_STOP, HOST_WIDE, "runs directly"],
        &[CARGO_TEST, MISE, "svc00"],
    );
    let ran = run_until(&app, "svc02 stop failed", "Input/output error");
    see(
        &ran,
        &[BREW_STOP, "svc02", "Bootstrap failed", "Input/output error"],
        &[ENGINE, "svc00", CARGO_TEST],
    );
    app.finish();
}

#[test]
fn h04_actions_scenario_parity_upgrade_managers() {
    let app = open("h04-actions-scenario-upgrade", "parity-upgrade-managers");
    let row = pick(
        &app,
        "Upgrade mise-managed tools",
        "Upgrade mise-managed tools",
        MISE,
    );
    see(
        &row,
        &[MISE, HOST_WIDE, "2 outdated", HOST],
        &[BREW_STOP, ENGINE, REPOS, "brew upgrade"],
    );
    let dialog = enter_until(&app, "mise confirmation", CONFIRM);
    see(
        &dialog,
        &[MISE, HOST_WIDE, CONFIRM],
        &[BREW_STOP, "brew upgrade", ENGINE],
    );
    let ran = run_until(&app, "mise upgraded", "2 tools upgraded");
    see(
        &ran,
        &[
            MISE,
            "mise node 26.0.1 → 26.1.0 ✓",
            "mise python 3.13.5 → 3.13.7 ✓",
            "2 tools upgraded",
        ],
        &[BREW_STOP, ENGINE, REPOS, "checksum mismatch"],
    );
    app.finish();
}
