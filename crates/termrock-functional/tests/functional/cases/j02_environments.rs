//! Jackin workspace environments, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j02_environments/`. The shots are the open New workspace
//! environment key form after the key and a masked value are typed, before
//! Save. A same-size resize does not force a frame, and the cursor column is
//! part of the hash, so each shot nudges the width and comes back.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{
    App, JACKIN, Launch, Point, SessionMode, button, file_sha256, reference_binary_from,
    reference_digest_from, row_text, text, try_launch,
};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const NAME_HELP: &str = "Directory basename by default";
const FIXTURE_SECRET: &str = "pw-fixture-only";
const DUP_SECRET: &str = "duplicate-secret-value";
const TOKEN_SECRET: &str = "field-token-secret-91";
const TOKEN_MASK: &str = "************t-91";
const DATABASE: &str = "DATABASE_URL";
const STRIPE: &str = "STRIPE_KEY";
const LOG_LEVEL: &str = "LOG_LEVEL";
const GH_TOKEN: &str = "GH_TOKEN";
const FIELD_TOKEN: &str = "FIELD_TOKEN";
const NEW_ENV: &str = "New workspace environment key";
const EDIT_LOG: &str = "Edit LOG_LEVEL";
const DUP_ERROR: &str = "DATABASE_URL already exists in this scope";
const ADDED: &str = "FIELD_TOKEN · added · save to apply";
const UPDATED: &str = "LOG_LEVEL · updated · save to apply";
const MOVED: &str = "FIELD_TOKEN moved to role backend";
const REMOVED: &str = "Removed LOG_LEVEL · u restores until save";
const DELETE_TITLE: &str = "Delete environment variable LOG_LEVEL?";
const SCOPE_TITLE: &str = "Scope for FIELD_TOKEN";
const PREVIEW_TITLE: &str = "Save workspace";
const CLEANUP: &str = "Clean up & save…";
const SAVED: &str = "Workspace payments-platform saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const DELETE: &str = "Delete";
const WIDE: &str = "J02.environments.env.120x40";
const NARROW: &str = "J02.environments.env.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_environments/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let capture = std::env::var_os("J02_CAPTURE").is_some_and(|value| value == "1");
    let mut launch = if capture {
        let manifest =
            crate::support::workspace_root().join("docs/testing/reference-manifest-j06-pr2.json");
        let binary = reference_binary_from(&manifest, "jackin-pr2");
        assert_eq!(
            file_sha256(&binary),
            reference_digest_from(&manifest, "jackin-pr2"),
            "J02 reference binary digest"
        );
        let mut launch = Launch::new(name, binary.to_str().expect("reference path"));
        launch.mode = SessionMode::Reference;
        launch.reference_manifest = Some(manifest);
        launch
    } else {
        Launch::new(name, JACKIN)
    };
    launch.args = vec!["--scenario".to_string(), "returning".to_string()];
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    // `Launch::new` leaves the clock off. `App::new` would enable it for jackin.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let started = Instant::now();
    let screen = loop {
        let screen = app.screen();
        let visible = text(&screen);
        if visible.contains(DIRECTORY) || visible.contains("Stand up, operator") {
            break screen;
        }
        assert!(
            app.child_running(),
            "jackin exited before the first frame\n{visible}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(25),
            "timed out: manager or intro\n{visible}"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    if text(&screen).contains("Stand up, operator") {
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

fn require(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    if !visible.contains(needle) {
        panic!("missing {needle:?}\n{visible}");
    }
}

fn forbid(screen: &TerminalSnapshot, needle: &str) {
    let visible = text(screen);
    if visible.contains(needle) {
        panic!("forbidden {needle:?}\n{visible}");
    }
}

fn key_token(line: &str, key: &str) -> bool {
    line.match_indices(key).any(|(at, _)| {
        let prev_ok = line[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
        let rest = &line[at + key.len()..];
        prev_ok && !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
    })
}

fn key_lines(screen: &TerminalSnapshot, key: &str) -> Vec<String> {
    text(screen)
        .lines()
        .filter(|line| key_token(line, key))
        .map(str::to_string)
        .collect()
}

fn only_line(screen: &TerminalSnapshot, key: &str) -> String {
    let lines = key_lines(screen, key);
    assert_eq!(lines.len(), 1, "{key} lines:\n{}", text(screen));
    lines[0].clone()
}

fn star_count(line: &str) -> usize {
    line.chars().filter(|c| *c == '*').count()
}

fn bullet_count(line: &str) -> usize {
    line.chars().filter(|c| *c == '•').count()
}

fn secrets_hidden(screen: &TerminalSnapshot) {
    forbid(screen, FIXTURE_SECRET);
    forbid(screen, DUP_SECRET);
    forbid(screen, TOKEN_SECRET);
}

fn select_payments(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(DIRECTORY),
        "manager did not start on Current directory\n{}",
        text(&screen)
    );
    app.key("down");
    app.wait_screen("payments-platform", |screen| {
        focused_line(screen).contains(PAYMENTS)
    })
}

fn open_editor(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(PAYMENTS),
        "refusing e unless payments-platform is focused\n{}",
        text(&screen)
    );
    app.key("e");
    app.wait_screen("editor", |screen| text(screen).contains(NAME_HELP))
}

fn open_environments(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(NAME_HELP) && !text(&before).contains("Add environment variable"),
        "editor did not open on General\n{}",
        text(&before)
    );
    app.key("]");
    app.wait_screen("mounts tab", |screen| text(screen).contains("Add mount"));
    app.key("]");
    app.wait_screen("roles tab", |screen| {
        let visible = text(screen);
        visible.contains("the-architect") && !visible.contains("Add mount")
    });
    app.key("]");
    // A saved workspace no longer has LOG_LEVEL, so the tab wait cannot require it.
    app.wait_screen("environments tab", |screen| {
        text(screen).contains("Add environment variable")
    })
}

fn semantic_rows(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(
        visible.contains("Add environment variable"),
        "environments list\n{visible}"
    );
    forbid(screen, FIXTURE_SECRET);
    let database = only_line(screen, DATABASE);
    assert_eq!(star_count(&database), 16, "DATABASE_URL mask\n{database}");
    let stripe = only_line(screen, STRIPE);
    assert!(
        stripe.contains("[op]") && star_count(&stripe) == 16,
        "STRIPE_KEY mask\n{stripe}"
    );
    let log_level = only_line(screen, LOG_LEVEL);
    assert!(
        star_count(&log_level) == 5 && log_level.contains("plain"),
        "LOG_LEVEL mask\n{log_level}"
    );
    let token = only_line(screen, GH_TOKEN);
    assert!(token.contains("$GH_TOKEN"), "GH_TOKEN host mask\n{token}");
}

fn tab_strip_focused(screen: &TerminalSnapshot) -> bool {
    let line = focused_line(screen);
    line.contains("General") && line.contains("Environments") && !line.contains(DATABASE)
}

fn focus_list(app: &App) {
    if !tab_strip_focused(&app.screen()) {
        return;
    }
    app.key("enter");
    app.wait_screen("environments list", |screen| {
        let line = focused_line(screen);
        line.contains("Workspace") || key_token(&line, DATABASE) || line.contains("Add environment")
    });
}

fn step_until(app: &App, key: &str, marker: &str) {
    app.press_until(key, marker, 16, focused_line, |screen| {
        key_token(&focused_line(screen), marker)
    });
}

fn focus_key(app: &App, key: &str) {
    focus_list(app);
    if key_token(&focused_line(&app.screen()), key) {
        return;
    }
    // `g` lands on the workspace header, and the next paint skips that header
    // (`config.rs` `build_rows`), so the stable top row is DATABASE_URL.
    app.key("g");
    app.wait_screen("list top", |screen| {
        let line = focused_line(screen);
        key_token(&line, DATABASE) || key_token(&line, key)
    });
    if key_token(&focused_line(&app.screen()), key) {
        return;
    }
    step_until(app, "down", key);
}

fn preview_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(UNSAVED)
    })
}

fn delete_button(app: &App) -> Point {
    button(app, DELETE, |row| {
        row.contains(CANCEL) && row.contains(DELETE) && !row.contains(DELETE_TITLE)
    })
}

fn open_new_env(app: &App) -> TerminalSnapshot {
    focus_list(app);
    // The workspace header cannot hold the cursor. `a` on a workspace env row
    // (`RowKey::Env(None, _)` in `config.rs`) opens the workspace-scoped form.
    let before = focused_line(&app.screen());
    assert!(
        key_token(&before, DATABASE),
        "refusing a unless the workspace section row is current\n{before}"
    );
    app.key("a");
    // The dialog title paints before the kind control.
    let form = app.wait_screen("new env", |screen| {
        let visible = text(screen);
        visible.contains(NEW_ENV) && visible.contains("Plain text")
    });
    require(&form, "Plain text");
    forbid(&form, DUP_SECRET);
    forbid(&form, FIXTURE_SECRET);
    form
}

fn type_masked(app: &App, key_name: &str, value: &str) {
    // Replace any value retained by a reopened form before entering a key.
    app.key("ctrl+a");
    app.key("ctrl+k");
    app.type_text(key_name);
    app.wait_screen("key text", |screen| {
        let line = focused_line(screen);
        line.contains(key_name) && line.matches(key_name).count() == 1
    });
    app.key("tab");
    app.key("tab");
    app.wait_screen("value field", |screen| {
        let line = focused_line(screen);
        !line.contains(key_name) && !line.contains("Plain text")
    });
    app.paste(value);
    let typed = app.screen();
    if text(&typed).contains(value) || focused_line(&typed).contains(value) {
        panic!(
            "focused field paints the plaintext secret\n{}",
            text(&typed)
        );
    }
    app.wait_screen("masked value", |screen| {
        bullet_count(&focused_line(screen)) == value.chars().count()
            && !text(screen).contains(value)
            && !text(screen).contains(FIXTURE_SECRET)
    });
}

fn prepared_form(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(NEW_ENV)
        && visible.contains(DATABASE)
        && bullet_count(&line) == DUP_SECRET.chars().count()
        && !visible.contains(DUP_SECRET)
        && !visible.contains(FIXTURE_SECRET)
        && !visible.contains(DUP_ERROR)
        && !visible.contains(ADDED)
        && !visible.contains(SAVED)
        && !visible.contains(CLEANUP)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the form stays open.
fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet_frame, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app, prepared_form);
    let test_id = visual_id(checkpoint);
    if std::env::var_os("J02_CAPTURE").is_some_and(|value| value == "1") {
        capture_reference_frame(app, &test_id);
    } else {
        app.visual(&test_id);
    }
}

fn capture_reference_frame(app: &App, test_id: &str) {
    let session_path = app.evidence_dir().join("session.json");
    let session: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&session_path).expect("reference session metadata"))
            .expect("parse reference session metadata");
    assert_eq!(session["mode"], "reference");
    assert_eq!(
        session["reference_sha"],
        "3cbe13ce922f7d0902bf42af418f3d012d8b537f"
    );
    assert_eq!(
        session["lockfile_sha256"],
        "9d9859e69e1de0b3306231a25a381c865079ca7ab533420f34dceb6eafe4d39a"
    );
    assert_eq!(
        session["binary_sha256"],
        "7892611f8c7de53308a3653d5ce53f01c2e717e38bade0af5d20cbd0c43c0421"
    );
    let screen = app.screen();
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen,
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let directory = app.evidence_dir().join("reference-candidates");
    std::fs::create_dir_all(&directory).expect("create reference candidate directory");
    let candidate = directory.join(format!("{}.tui.pb", test_id.replace('/', "_")));
    std::fs::write(&candidate, &bytes).expect("write reference candidate sample");
    let digest = file_sha256(&candidate);
    let index = serde_json::json!({
        "test_id": test_id,
        "candidate_path": candidate,
        "candidate_sha256": digest,
        "session_path": session_path,
        "session_sha256": file_sha256(&session_path),
        "binary_path": session["binary_path"],
        "binary_sha256": session["binary_sha256"],
        "reference_sha": session["reference_sha"],
        "lockfile_sha256": session["lockfile_sha256"],
    });
    use std::io::Write as _;
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory.join("index.jsonl"))
        .expect("open reference capture index");
    serde_json::to_writer(&mut log, &index).expect("write reference capture index");
    log.write_all(b"\n")
        .expect("finish reference capture index");
    eprintln!(
        "J02 reference candidate {test_id} sha256 {digest} at {}",
        candidate.display()
    );
}

fn form_closed(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // The list hint is "Enter Edit", so a bare "Edit " match stays true on the list.
    !visible.contains(NEW_ENV) && !visible.contains(EDIT_LOG)
}

fn has_value_row(screen: &TerminalSnapshot, key: &str) -> bool {
    key_lines(screen, key)
        .iter()
        .filter(|line| line.contains('*') || line.contains('$') || line.contains("[op]"))
        .count()
        == 1
}

fn value_row(screen: &TerminalSnapshot, key: &str) -> String {
    let lines: Vec<_> = key_lines(screen, key)
        .into_iter()
        .filter(|line| line.contains('*') || line.contains('$') || line.contains("[op]"))
        .collect();
    assert_eq!(lines.len(), 1, "{key} value rows:\n{}", text(screen));
    lines[0].clone()
}

fn section_start(line: &str, header: &str) -> bool {
    if header == "Workspace" {
        line.contains("Workspace") && !line.contains("Workspaces") && line.contains("var")
    } else {
        line.contains(header)
    }
}

fn section_body(screen: &TerminalSnapshot, header: &str, next: &str) -> String {
    let visible = text(screen);
    let lines: Vec<&str> = visible.lines().collect();
    let start = lines
        .iter()
        .position(|line| section_start(line, header))
        .unwrap_or_else(|| panic!("missing section {header}\n{visible}"));
    let end = lines
        .iter()
        .skip(start + 1)
        .position(|line| line.contains(next))
        .map(|index| start + 1 + index)
        .unwrap_or(lines.len());
    lines[start..end].join("\n")
}

fn submit_form(app: &App) {
    // Commit the value, move to Cancel, then across the button row to Save.
    app.key("tab");
    app.key("tab");
    app.key("tab");
    app.key("right");
    let save_focused = app.wait_screen("save focused", |screen| {
        text(screen).contains("Enter Save")
            && (0..screen.rows()).any(|row| {
                let line = row_text(screen, row);
                line.contains(CANCEL) && line.contains("▎Save")
            })
    });
    assert!(
        text(&save_focused).contains(CONFIRM),
        "{}",
        text(&save_focused)
    );
    let save_row = (0..save_focused.rows())
        .find(|row| {
            let line = row_text(&save_focused, *row);
            line.contains(CANCEL) && line.contains("▎Save")
        })
        .expect("focused Save button row");
    let label: Vec<_> = CONFIRM.chars().collect();
    let column = save_focused
        .row(save_row)
        .expect("Save button row cells")
        .windows(label.len())
        .position(|window| {
            window
                .iter()
                .zip(&label)
                .all(|(cell, character)| cell.text() == character.to_string())
        })
        .expect("Save button cell range");
    app.click(Point {
        x: u16::try_from(column + 2).expect("Save button x"),
        y: u16::try_from(save_row).expect("Save button y"),
    });
}

#[test]
fn j02_environments_route() {
    let app = open("j02-environments-route");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let selected = select_payments(&app);
    assert!(
        focused_line(&selected).contains(PAYMENTS),
        "{}",
        text(&selected)
    );
    let editor = open_editor(&app);
    require(&editor, NAME_HELP);
    let environments = open_environments(&app);
    semantic_rows(&environments);
    assert!(
        text(&environments).contains(PAYMENTS),
        "workspace name\n{}",
        text(&environments)
    );
    app.key("escape");
    let back = app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(NAME_HELP)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains("Add environment variable")
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains("save to apply"), "{}", text(&back));
    app.finish();
}

#[test]
fn j02_environments_env() {
    let app = open("j02-environments-env");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    select_payments(&app);
    open_editor(&app);
    let environments = open_environments(&app);
    semantic_rows(&environments);

    open_new_env(&app);
    type_masked(&app, DATABASE, DUP_SECRET);
    let wide = app.wait_screen("duplicate form", prepared_form);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    if text(&wide).contains(DUP_SECRET) || focused_line(&wide).contains(DUP_SECRET) {
        panic!("focused field paints the plaintext secret\n{}", text(&wide));
    }
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && prepared_form(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    if text(&narrow).contains(DUP_SECRET) {
        panic!(
            "focused field paints the plaintext secret\n{}",
            text(&narrow)
        );
    }
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && prepared_form(screen)
    });

    submit_form(&app);
    let rejected = app.wait_screen("duplicate rejected", |screen| {
        text(screen).contains(NEW_ENV) && text(screen).contains(DUP_ERROR)
    });
    assert!(
        !text(&rejected).contains(ADDED),
        "duplicate was staged despite the validation error\n{}",
        text(&rejected)
    );
    secrets_hidden(&rejected);
    app.key("escape");
    let dropped = app.wait_screen("duplicate form dismissed", |screen| {
        form_closed(screen)
            && !text(screen).contains(DUP_ERROR)
            && key_lines(screen, DATABASE).len() == 1
    });
    secrets_hidden(&dropped);
    assert_eq!(star_count(&only_line(&dropped, DATABASE)), 16);

    open_new_env(&app);
    type_masked(&app, FIELD_TOKEN, TOKEN_SECRET);
    submit_form(&app);
    let added = app.wait_screen("field token added", |screen| {
        text(screen).contains(ADDED) && form_closed(screen) && has_value_row(screen, FIELD_TOKEN)
    });
    secrets_hidden(&added);
    let field = value_row(&added, FIELD_TOKEN);
    assert!(field.contains(TOKEN_MASK), "FIELD_TOKEN mask\n{field}");

    focus_key(&app, LOG_LEVEL);
    app.key("enter");
    let editing = app.wait_screen("edit log level", |screen| text(screen).contains(EDIT_LOG));
    require(&editing, LOG_LEVEL);
    forbid(&editing, TOKEN_SECRET);
    app.key("tab");
    app.key("tab");
    app.wait_screen("log value", |screen| {
        let line = focused_line(screen);
        !line.contains("Plain text") && !key_token(&line, LOG_LEVEL)
    });
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text("info");
    app.wait_screen("info masked", |screen| {
        bullet_count(&focused_line(screen)) == 4 && !text(screen).contains("debug")
    });
    submit_form(&app);
    let updated = app.wait_screen("log updated", |screen| {
        text(screen).contains(UPDATED) && form_closed(screen)
    });
    secrets_hidden(&updated);
    let log_level = value_row(&updated, LOG_LEVEL);
    assert_eq!(star_count(&log_level), 4, "updated LOG_LEVEL\n{log_level}");
    assert!(!log_level.contains("info"), "{log_level}");

    focus_key(&app, FIELD_TOKEN);
    app.key("s");
    let scope = app.wait_screen("scope dialog", |screen| text(screen).contains(SCOPE_TITLE));
    require(&scope, "role backend");
    app.key("down");
    app.key("down");
    app.wait_screen("role backend selected", |screen| {
        text(screen)
            .lines()
            .any(|line| line.contains("role backend") && line.contains("(●)"))
    });
    app.key("enter");
    let moved = app.wait_screen("moved", |screen| text(screen).contains(MOVED));
    secrets_hidden(&moved);
    let workspace = section_body(&moved, "Workspace", "Role:");
    assert!(
        !workspace.contains(FIELD_TOKEN),
        "workspace still lists FIELD_TOKEN\n{workspace}"
    );
    let backend = section_body(&moved, "Role: backend", "Add role override");
    assert!(
        backend.contains(FIELD_TOKEN) && backend.contains(TOKEN_MASK),
        "role backend row\n{backend}"
    );

    focus_key(&app, LOG_LEVEL);
    app.key("d");
    let confirm = app.wait_screen("delete dialog", |screen| {
        text(screen).contains(DELETE_TITLE) && focused_line(screen).contains(CANCEL)
    });
    assert!(
        focused_line(&confirm).contains(CANCEL),
        "destructive dialog focus\n{}",
        focused_line(&confirm)
    );
    app.click(delete_button(&app));
    let removed = app.wait_screen("removed", |screen| text(screen).contains(REMOVED));
    secrets_hidden(&removed);
    let log_rows: Vec<_> = key_lines(&removed, LOG_LEVEL)
        .into_iter()
        .filter(|line| !line.contains(REMOVED))
        .collect();
    assert!(
        !log_rows.is_empty() && log_rows.iter().all(|line| line.contains("plain text")),
        "LOG_LEVEL stayed active\n{}",
        text(&removed)
    );

    app.key("ctrl+s");
    let preview = app.wait_screen("save preview", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
    });
    secrets_hidden(&preview);
    app.click(preview_save(&app));
    let saved = app.wait_screen("saved", |screen| {
        text(screen).contains(SAVED) && text(screen).contains(DIRECTORY)
    });
    secrets_hidden(&saved);
    assert!(!text(&saved).contains(NEW_ENV), "{}", text(&saved));

    if !focused_line(&saved).contains(PAYMENTS) {
        select_payments(&app);
    }
    open_editor(&app);
    let again = open_environments(&app);
    secrets_hidden(&again);
    assert!(
        text(&again).contains(PAYMENTS),
        "workspace name after save\n{}",
        text(&again)
    );
    assert_eq!(key_lines(&again, DATABASE).len(), 1);
    assert_eq!(star_count(&only_line(&again, DATABASE)), 16);
    assert!(only_line(&again, STRIPE).contains("[op]"));
    assert!(only_line(&again, GH_TOKEN).contains("$GH_TOKEN"));
    assert!(
        !text(&again).contains(LOG_LEVEL),
        "LOG_LEVEL returned\n{}",
        text(&again)
    );
    let workspace = section_body(&again, "Workspace", "Role:");
    assert!(
        !workspace.contains(FIELD_TOKEN),
        "saved workspace lists FIELD_TOKEN\n{workspace}"
    );
    let backend = section_body(&again, "Role: backend", "Add role override");
    assert!(
        backend.contains(FIELD_TOKEN) && backend.contains(TOKEN_MASK),
        "saved role row\n{backend}"
    );
    app.finish();
}
