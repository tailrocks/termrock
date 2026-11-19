//! Holla arguments, driven through the real binary.
//!
//! `parity-task-sources` is `~/work/poly`: package.json scripts run through
//! yarn, `.justfile` recipes, Makefile targets, and a Taskfile that fails to
//! parse. Those tasks have no argument spec. The arguments form opened here is
//! "Clone a GitHub repository…", which this world lists: owner and protocol
//! are choices, repository is required text, and destination defaults to
//! `~/work/poly`.
//!
//! `ArgSpec::text` and `ArgSpec::choice` both set `secret: false`
//! (`apps/holla/src/domain/action.rs`). No `ArgSpec` assignment sets
//! `secret: true`, so the form has no secret field. This scenario never
//! reaches a secret script prompt either. Required text and choice are driven.
//!
//! Tuiscotti ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/h02_args/`.

use tui_test::TerminalSnapshot;

use crate::support::{App, HOLLA, Launch, flash_settle, text, try_launch};

const SCENARIO: &str = "parity-task-sources";
const DIR: &str = "~/work/poly";
const QUERY: &str = "Clone a GitHub repository";
const FORM: &str = "Clone a GitHub repository… · arguments";
const REPO: &str = "args-repo";
const ERROR: &str = "Fill the required fields first";
const CONFIRM: &str = "one explicit confirmation";
const OUTPUT: &str = "╭─ output";
const COMMAND: &str = "gh repo clone 'alex-dev/args-repo' '~/work/poly' --protocol https";
const WIDE: &str = "H02.args.args.120x40";
const NARROW: &str = "H02.args.args.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/h02_args/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, HOLLA);
    launch.args = vec!["--scenario".to_string(), SCENARIO.to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.history = false;
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text(DIR);
    app.wait_text("Node project · yarn");
    app
}

fn flat(screen: &TerminalSnapshot) -> String {
    text(screen)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = flat(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{}", text(screen));
    }
    for item in absent {
        assert!(
            !visible.contains(item),
            "forbidden {item:?}\n{}",
            text(screen)
        );
    }
}

fn clone_selected(screen: &TerminalSnapshot) -> bool {
    let visible = flat(screen);
    visible.contains(QUERY)
        && visible.contains("protocol = ssh")
        && visible.contains("destination = ~/work/poly")
        && visible.contains(DIR)
}

fn on_form(screen: &TerminalSnapshot) -> bool {
    flat(screen).contains(FORM) && flat(screen).contains(DIR)
}

fn search_clone(app: &App) {
    app.paste(QUERY);
    app.wait_screen("clone row is selected", clone_selected);
}

fn open_form(app: &App) {
    search_clone(app);
    app.key("enter");
    let form = app.wait_screen("arguments form is open", on_form);
    see(
        &form,
        &[FORM, "repository", "protocol", DIR, "alex-dev"],
        &[OUTPUT, CONFIRM, REPO],
    );
}

fn shot(app: &App, checkpoint: &str) {
    app.visual(&visual_id(checkpoint));
}

fn clear_query(app: &App) {
    app.key("escape");
    app.key("escape");
    app.wait_text("Search actions and resources");
}

/// Pastes `query`, then waits for the filtered state itself: every `present`
/// text is visible and no `absent` text is. A needle that the unfiltered list
/// already shows proves nothing, because the frame that applies the filter can
/// still be on its way.
fn search_for(app: &App, query: &str, present: &[&str], absent: &[&str]) -> TerminalSnapshot {
    app.paste(query);
    app.wait_screen(&format!("list filtered by {query:?}"), |screen| {
        let visible = flat(screen);
        present.iter().all(|item| visible.contains(item))
            && absent.iter().all(|item| !visible.contains(item))
    })
}

#[test]
fn h02_args_route_returns_to_the_clone_row() {
    let app = open("h02-args-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(&landing, &[DIR, "Node project · yarn"], &[FORM, OUTPUT]);
    open_form(&app);
    app.key("escape");
    let back = app.wait_screen("arguments form closed", |screen| {
        clone_selected(screen) && !flat(screen).contains(FORM)
    });
    assert_eq!((back.cols(), back.rows()), (120, 40));
    see(
        &back,
        &[QUERY, "protocol = ssh", DIR],
        &[FORM, OUTPUT, CONFIRM, ERROR],
    );
    app.finish();
}

#[test]
fn h02_args_rejects_a_blank_repository_then_cancels() {
    let app = open("h02-args-form");
    open_form(&app);
    app.key("ctrl+s");
    let invalid = app.wait_screen("required repository rejected", |screen| {
        let visible = flat(screen);
        on_form(screen) && visible.contains(ERROR) && visible.contains("Required")
    });
    see(&invalid, &[ERROR, FORM, "ssh"], &[OUTPUT, CONFIRM, REPO]);

    app.type_text(REPO);
    let corrected = app.wait_text(REPO);
    see(&corrected, &[REPO, FORM, DIR], &[ERROR, OUTPUT, CONFIRM]);
    app.key("tab");
    app.key("h");
    let chosen = app.wait_screen("protocol is https and the command matches", |screen| {
        text(screen).contains(COMMAND) && on_form(screen)
    });
    see(
        &chosen,
        &[COMMAND, REPO, "https", DIR, FORM],
        &["ssh", OUTPUT, CONFIRM, ERROR],
    );

    flash_settle(&app);
    let wide = app.screen();
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    assert!(
        text(&wide).contains(COMMAND) && on_form(&wide),
        "wide form\n{}",
        text(&wide)
    );
    shot(&app, WIDE);

    app.resize(80, 24);
    let narrowed = app.wait_screen("arguments form at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && on_form(screen) && flat(screen).contains(REPO)
    });
    assert_eq!((narrowed.cols(), narrowed.rows()), (80, 24));
    see(
        &narrowed,
        &[FORM, REPO, "https", DIR],
        &[OUTPUT, CONFIRM, "ssh"],
    );
    flash_settle(&app);
    shot(&app, NARROW);
    app.resize(120, 40);

    let restored = app.wait_screen("arguments form back at 120x40", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && text(screen).contains(COMMAND)
            && on_form(screen)
    });
    see(&restored, &[COMMAND, REPO, DIR], &[OUTPUT, CONFIRM, "ssh"]);

    app.key("tab");
    app.key("tab");
    app.key("enter");
    let closed = app.wait_screen("cancel leaves the finder", |screen| {
        !flat(screen).contains(FORM) && clone_selected(screen)
    });
    see(
        &closed,
        &[QUERY, "protocol = ssh", DIR],
        &[FORM, REPO, OUTPUT, CONFIRM, ERROR, COMMAND],
    );
    app.finish();
}

#[test]
fn h02_args_scenario_parity_task_sources() {
    let app = open("h02-args-scenario");
    let landing = app.screen();
    see(
        &landing,
        &[DIR, "Node project · yarn", "poly"],
        &[
            "Inspect 12 GB",
            "Start development ecosystem",
            "~/work/acme",
            "~/work/engine",
            "apps/frontend",
            "Emit a mixed stream",
            FORM,
        ],
    );

    let yarn = search_for(
        &app,
        "yarn dev",
        &["yarn dev", "package.json", "vite", DIR],
        &["Inspect 12 GB", "~/work/acme", "just build"],
    );
    see(
        &yarn,
        &["yarn dev", "package.json", "vite", DIR],
        &["Inspect 12 GB", "~/work/acme", "just build"],
    );
    clear_query(&app);

    let just = search_for(
        &app,
        "just build",
        &["just build", ".justfile", DIR],
        &["package.json", "Makefile", "Inspect 12 GB"],
    );
    see(
        &just,
        &["just build", ".justfile", DIR],
        &["package.json", "Makefile", "Inspect 12 GB"],
    );
    clear_query(&app);

    let make = search_for(
        &app,
        "deploy-prod",
        &["make deploy-prod", "Makefile", DIR],
        &[".justfile", "package.json", "Inspect 12 GB"],
    );
    see(
        &make,
        &["make deploy-prod", "Makefile", DIR],
        &[".justfile", "package.json", "Inspect 12 GB"],
    );
    clear_query(&app);

    let task = search_for(
        &app,
        "Taskfile",
        &["Taskfile.yaml", DIR],
        &["yarn dev", "make deploy-prod", "Inspect 12 GB"],
    );
    see(
        &task,
        &["Taskfile.yaml", DIR],
        &["yarn dev", "make deploy-prod", "Inspect 12 GB"],
    );
    assert!(
        flat(&task).contains("mapping values are not allowed")
            || flat(&task).contains("no task was invented"),
        "task discovery diagnostic\n{}",
        text(&task)
    );
    clear_query(&app);

    open_form(&app);
    app.key("escape");
    let back = app.wait_screen("scenario form closed", |screen| {
        clone_selected(screen) && !flat(screen).contains(FORM)
    });
    see(
        &back,
        &[DIR, "protocol = ssh", "Node project · yarn"],
        &[FORM, OUTPUT, "~/work/acme", "Inspect 12 GB"],
    );
    app.finish();
}
