//! Jackin global settings environments, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j03_environments/`. The shots are the open New global
//! environment key form after the key and a masked plain value are typed,
//! before Save. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, button, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const NAME_HELP: &str = "Directory basename by default";
const CRUMB_GENERAL: &str = "Settings › global › General";
const CRUMB_ENV: &str = "Settings › global › Environments";
const WORKSPACE_CRUMB: &str = "Workspaces › payments-platform › edit";
const GH_TOKEN: &str = "GH_TOKEN";
const EDITOR: &str = "EDITOR";
const CARGO_KEY: &str = "CARGO_NET";
const DATABASE: &str = "DATABASE_URL";
const REVIEW: &str = "REVIEW_NOTE";
const OPS: &str = "OPS_REF";
const PLAIN: &str = "desk-note-91";
const TAIL: &str = "e-91";
const HIDDEN_PREFIX: &str = "desk-note";
const NEW_ENV: &str = "New global environment key";
const PLAIN_SOURCE: &str = "Plain text";
const HOST_SOURCE: &str = "Host environment variable";
const HOST_FIELD: &str = "Host variable";
const ADDED_REVIEW: &str = "REVIEW_NOTE · added · save to apply";
const ADDED_OPS: &str = "OPS_REF · added · save to apply";
const PREVIEW_TITLE: &str = "Save settings";
const CLEANUP: &str = "Clean up & save…";
const SAVE_ANYWAY: &str = "Save anyway";
const SAVED: &str = "Settings saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const WIDE: &str = "J03.environments.env.120x40";
const NARROW: &str = "J03.environments.env.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j03_environments/{}",
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

fn value_row(screen: &TerminalSnapshot, key: &str) -> String {
    let lines: Vec<_> = key_lines(screen, key)
        .into_iter()
        .filter(|line| !line.contains("save to apply") && !line.contains("environment key"))
        .collect();
    assert_eq!(lines.len(), 1, "{key} value rows:\n{}", text(screen));
    lines[0].clone()
}

fn star_count(line: &str) -> usize {
    line.chars().filter(|c| *c == '*').count()
}

fn bullet_count(line: &str) -> usize {
    line.chars().filter(|c| *c == '•').count()
}

fn radio_on(screen: &TerminalSnapshot, label: &str) -> bool {
    text(screen)
        .lines()
        .any(|line| line.contains(label) && line.contains("(●)"))
}

fn tab_strip_focused(screen: &TerminalSnapshot) -> bool {
    let line = focused_line(screen);
    line.contains("General") && line.contains("Environments") && line.contains("Mounts")
}

fn original_keys(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    if !visible.contains(GH_TOKEN) || !visible.contains(EDITOR) || !visible.contains(CARGO_KEY) {
        return false;
    }
    let token = only_line(screen, GH_TOKEN);
    let editor = only_line(screen, EDITOR);
    token.contains("[op]")
        && star_count(&token) == 16
        && !editor.contains("nvim")
        && star_count(&editor) == 4
        && editor.contains("plain")
}

fn open_settings(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        text(&screen).contains(DIRECTORY),
        "refusing s unless the manager is showing\n{}",
        text(&screen)
    );
    app.key("s");
    app.wait_screen("settings general", |screen| {
        text(screen).contains(CRUMB_GENERAL)
    })
}

fn open_settings_env(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(CRUMB_GENERAL),
        "expected General before ]\n{}",
        text(&before)
    );
    app.key("]");
    app.wait_screen("settings mounts", |screen| {
        text(screen).contains("Settings › global › Mounts")
    });
    app.key("]");
    app.wait_screen("settings environments", |screen| {
        text(screen).contains(CRUMB_ENV) && text(screen).contains(GH_TOKEN)
    })
}

fn focus_env_list(app: &App) {
    if !tab_strip_focused(&app.screen()) {
        return;
    }
    app.key("enter");
    app.wait_screen("environments list", |screen| {
        let line = focused_line(screen);
        key_token(&line, GH_TOKEN) || line.contains("Add environment variable")
    });
}

fn leave_settings(app: &App) -> TerminalSnapshot {
    app.key("escape");
    app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(CRUMB_GENERAL)
            && !text(screen).contains(CRUMB_ENV)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains("Add environment variable")
    })
}

fn select_payments(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    if focused_line(&screen).contains(PAYMENTS) {
        return screen;
    }
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

fn open_workspace_env(app: &App) -> TerminalSnapshot {
    select_payments(app);
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(PAYMENTS),
        "refusing e unless payments-platform is focused\n{}",
        text(&screen)
    );
    app.key("e");
    app.wait_screen("editor", |screen| text(screen).contains(NAME_HELP));
    app.key("]");
    app.wait_screen("workspace mounts", |screen| {
        text(screen).contains("Add mount")
    });
    app.key("]");
    app.wait_screen("workspace roles", |screen| {
        let visible = text(screen);
        visible.contains("the-architect") && !visible.contains("Add mount")
    });
    app.key("]");
    app.wait_screen("workspace environments", |screen| {
        let visible = text(screen);
        visible.contains("Add environment variable")
            && visible.contains(WORKSPACE_CRUMB)
            && visible.contains(DATABASE)
            && !visible.contains(REVIEW)
            && !visible.contains(OPS)
    })
}

fn form_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(PREVIEW_TITLE)
            && !row.contains(SAVE_ANYWAY)
    })
}

fn preview_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && row.contains('│')
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(SAVE_ANYWAY)
            && !row.contains(UNSAVED)
    })
}

fn open_new_env(app: &App) -> TerminalSnapshot {
    focus_env_list(app);
    // `build_rows` will not leave the cursor on the Global header (`folded:
    // None`), so Home lands on GH_TOKEN. `a` there is still scope global.
    app.key("g");
    app.wait_screen("global row", |screen| {
        key_token(&focused_line(screen), GH_TOKEN)
    });
    let before = focused_line(&app.screen());
    assert!(
        key_token(&before, GH_TOKEN),
        "refusing a unless the global GH_TOKEN row is current\n{before}"
    );
    // `a` on the GH_TOKEN row calls open_env_form(None) in config.rs.
    // `n` only edits a mount row.
    app.key("a");
    // The title matches "environment key" before the radios paint. The
    // checkpoint is the selected Plain text radio, with the key field ready.
    let form = app.wait_screen("new env", |screen| {
        let visible = text(screen);
        visible.contains(NEW_ENV)
            && radio_on(screen, PLAIN_SOURCE)
            && !visible.contains(PLAIN)
            && focused_line(screen).contains(DATABASE)
    });
    require(&form, PLAIN_SOURCE);
    forbid(&form, PLAIN);
    form
}

fn type_plain(app: &App) {
    app.type_text(REVIEW);
    app.wait_screen("key text", |screen| {
        let line = focused_line(screen);
        line.contains(REVIEW) && line.matches(REVIEW).count() == 1
    });
    app.key("tab");
    app.key("tab");
    app.wait_screen("value field", |screen| {
        let line = focused_line(screen);
        !line.contains(REVIEW) && !line.contains(PLAIN_SOURCE)
    });
    app.type_text(PLAIN);
    let typed = app.screen();
    if text(&typed).contains(PLAIN) {
        panic!("form paints the plaintext value\n{}", text(&typed));
    }
    // Blur so reveal_tail(4) paints the last four graphemes. While the field
    // is editing, display_graphemes hides the tail.
    app.key("tab");
    app.wait_screen("revealed tail", |screen| {
        text(screen).contains(TAIL)
            && !text(screen).contains(PLAIN)
            && !text(screen).contains(HIDDEN_PREFIX)
    });
}

fn plain_form(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(NEW_ENV)
        && visible.contains(REVIEW)
        && radio_on(screen, PLAIN_SOURCE)
        && visible
            .lines()
            .any(|line| line.contains(TAIL) && bullet_count(line) == 8)
        && !visible.contains(PLAIN)
        && !visible.contains(HIDDEN_PREFIX)
        && !visible.contains(HOST_FIELD)
        && !visible.contains(SAVED)
        && !visible.contains(PREVIEW_TITLE)
        && !visible.contains(ADDED_REVIEW)
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the form stays open.
fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet_frame, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app, plain_form);
    app.visual(&visual_id(checkpoint));
}

fn form_closed(screen: &TerminalSnapshot) -> bool {
    !text(screen).contains(NEW_ENV)
}

fn submit_form(app: &App) {
    app.click(form_save(app));
}

fn choose_host(app: &App) {
    app.key("tab");
    app.wait_screen("value source", |screen| {
        let line = focused_line(screen);
        line.contains(PLAIN_SOURCE) || line.contains("Value source")
    });
    app.key("down");
    app.wait_screen("op radio", |screen| radio_on(screen, "1Password"));
    app.key("down");
    let host = app.wait_screen("host radio", |screen| {
        radio_on(screen, HOST_SOURCE) && text(screen).contains(HOST_FIELD)
    });
    if !text(&host).contains(HOST_FIELD) {
        panic!("host field is not painted\n{}", text(&host));
    }
}

fn type_host(app: &App) {
    let radio = focused_line(&app.screen());
    app.key("tab");
    app.wait_screen("host field", |screen| {
        let line = focused_line(screen);
        line != radio && !line.contains(HOST_SOURCE)
    });
    let landed = focused_line(&app.screen());
    if !text(&app.screen()).contains(HOST_FIELD) {
        panic!("host field is not painted\n{}", text(&app.screen()));
    }
    if landed.contains("Scope") || landed.contains("1Password") {
        panic!(
            "tab did not land on the host field\n{}",
            text(&app.screen())
        );
    }
    app.type_text(OPS);
    app.wait_screen("host typed", |screen| {
        focused_line(screen).contains(OPS) && text(screen).contains(HOST_FIELD)
    });
}

#[test]
fn j03_environments_route() {
    let app = open("j03-environments-route");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let general = open_settings(&app);
    require(&general, CRUMB_GENERAL);
    let environments = open_settings_env(&app);
    assert!(
        tab_strip_focused(&environments) && original_keys(&environments),
        "global environments\n{}",
        text(&environments)
    );
    require(&environments, CRUMB_ENV);
    forbid(&environments, NEW_ENV);
    forbid(&environments, REVIEW);
    forbid(&environments, OPS);
    forbid(&environments, SAVED);
    let back = leave_settings(&app);
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains("save to apply"), "{}", text(&back));
    assert!(!text(&back).contains(UNSAVED), "{}", text(&back));
    app.finish();
}

#[test]
fn j03_environments_env() {
    let app = open("j03-environments-env");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    open_settings(&app);
    let environments = open_settings_env(&app);
    assert!(
        original_keys(&environments),
        "existing global keys\n{}",
        text(&environments)
    );

    open_new_env(&app);
    type_plain(&app);
    let wide = app.wait_screen("plain form", plain_form);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    forbid(&wide, PLAIN);
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && plain_form(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    forbid(&narrow, PLAIN);
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && plain_form(screen)
    });

    submit_form(&app);
    let added = app.wait_screen("review note added", |screen| {
        text(screen).contains(ADDED_REVIEW)
            && form_closed(screen)
            && text(screen).contains(CRUMB_ENV)
    });
    forbid(&added, PLAIN);
    forbid(&added, NEW_ENV);
    let review = value_row(&added, REVIEW);
    assert!(
        star_count(&review) == 12 && !review.contains(PLAIN) && review.contains("plain"),
        "REVIEW_NOTE row\n{review}"
    );
    assert_eq!(
        text(&added)
            .lines()
            .filter(|line| line.contains(ADDED_REVIEW))
            .count(),
        1,
        "status text\n{}",
        text(&added)
    );

    open_new_env(&app);
    app.type_text(OPS);
    app.wait_screen("ops key", |screen| focused_line(screen).contains(OPS));
    choose_host(&app);
    type_host(&app);
    submit_form(&app);
    let hosted = app.wait_screen("ops ref added", |screen| {
        text(screen).contains(ADDED_OPS) && form_closed(screen) && text(screen).contains(CRUMB_ENV)
    });
    forbid(&hosted, PLAIN);
    let ops = value_row(&hosted, OPS);
    assert!(
        ops.contains("$OPS_REF") && ops.contains("host env") && star_count(&ops) == 0,
        "OPS_REF row\n{ops}"
    );
    assert!(
        value_row(&hosted, REVIEW).contains("plain")
            && only_line(&hosted, GH_TOKEN).contains("[op]"),
        "pending rows\n{}",
        text(&hosted)
    );
    forbid(&hosted, SAVED);

    app.key("ctrl+s");
    let preview = app.wait_screen("save settings", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && visible.contains(REVIEW)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
            && !visible.contains(SAVE_ANYWAY)
    });
    assert!(
        text(&preview).contains(CONFIRM) && !text(&preview).contains(CLEANUP),
        "preview\n{}",
        text(&preview)
    );
    app.click(preview_save(&app));
    let saved = app.wait_screen("settings saved", |screen| {
        text(screen).contains(SAVED)
            && text(screen).contains(DIRECTORY)
            && !text(screen).contains(CRUMB_ENV)
    });
    assert!(!text(&saved).contains(NEW_ENV), "{}", text(&saved));
    forbid(&saved, PLAIN);

    open_settings(&app);
    let again = open_settings_env(&app);
    assert!(
        original_keys(&again),
        "original keys after save\n{}",
        text(&again)
    );
    let review = value_row(&again, REVIEW);
    assert!(
        star_count(&review) == 12 && review.contains("plain") && !review.contains(PLAIN),
        "saved REVIEW_NOTE\n{review}"
    );
    let ops = value_row(&again, OPS);
    assert!(
        ops.contains("$OPS_REF") && ops.contains("host env") && !ops.contains(PLAIN),
        "saved OPS_REF\n{ops}"
    );
    leave_settings(&app);
    let workspace = open_workspace_env(&app);
    assert!(
        text(&workspace).contains(DATABASE)
            && text(&workspace).contains(WORKSPACE_CRUMB)
            && !text(&workspace).contains(REVIEW)
            && !text(&workspace).contains(OPS),
        "workspace environments\n{}",
        text(&workspace)
    );
    app.finish();
}
