//! Jackin global settings mounts, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j03_mounts/`. The shots are the open New mount form after a
//! host is chosen and the destination is `/home/agent/field-notes`, before
//! Save. A same-size resize does not force a frame, and the cursor column is
//! part of the hash, so each shot nudges the width and comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, button, row_text, step_until, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const NAME_HELP: &str = "Directory basename by default";
const CRUMB_GENERAL: &str = "Settings › global › General";
const CRUMB_MOUNTS: &str = "Settings › global › Mounts";
const WORKSPACE_CRUMB: &str = "Workspaces › payments-platform › edit";
const DEST_PLATFORM: &str = "/workspace/payments-platform";
const DEST_LIBS: &str = "/workspace/libs";
const DEST_GITCONFIG: &str = "/home/agent/.gitconfig";
const DEST_CARGO: &str = "/home/agent/.cargo/registry";
const DEST_KUBE: &str = "/home/agent/.kube";
const DEST_FIELD: &str = "/home/agent/field-notes";
const HOST_DOCS: &str = "~/src/payments-platform/docs";
const AUTOFILL: &str = "/workspace/docs";
const SCOPE_GLOBAL: &str = "global";
const SCOPE_ROLE: &str = "role sre";
const NEW_MOUNT: &str = "New mount";
const CHOOSE_HOST: &str = "Choose the host directory";
const NOT_CHOSEN: &str = "not chosen";
const HOST_KIND: &str = "Host directory";
const BROWSE: &str = "Browse…";
const ADDED_FIELD: &str = "Mount /home/agent/field-notes added · save to apply";
const PREVIEW_TITLE: &str = "Save settings";
const CLEANUP: &str = "Clean up & save…";
const SAVED: &str = "Settings saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const PLACEHOLDER: &str = "/workspace/name";
const WIDE: &str = "J03.mounts.mounts.120x40";
const NARROW: &str = "J03.mounts.mounts.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j03_mounts/{}",
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

fn dest_token(line: &str, dest: &str) -> bool {
    line.match_indices(dest).any(|(at, _)| {
        let rest = &line[at + dest.len()..];
        !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    })
}

fn mount_line(screen: &TerminalSnapshot, dest: &str) -> Option<String> {
    text(screen)
        .lines()
        .find(|line| dest_token(line, dest) && (line.contains(" rw") || line.contains(" ro")))
        .map(str::to_owned)
}

fn scoped_mount(screen: &TerminalSnapshot, dest: &str, scope: &str) -> bool {
    mount_line(screen, dest).is_some_and(|line| line.contains(scope))
}

fn original_scopes(screen: &TerminalSnapshot) -> bool {
    scoped_mount(screen, DEST_GITCONFIG, SCOPE_GLOBAL)
        && scoped_mount(screen, DEST_CARGO, SCOPE_GLOBAL)
        && scoped_mount(screen, DEST_KUBE, SCOPE_ROLE)
}

fn global_mounts(screen: &TerminalSnapshot) -> bool {
    original_scopes(screen) && mount_line(screen, DEST_FIELD).is_none()
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

fn open_settings_mounts(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        text(&before).contains(CRUMB_GENERAL),
        "expected General before ]\n{}",
        text(&before)
    );
    app.key("]");
    app.wait_screen("settings mounts", |screen| {
        text(screen).contains(CRUMB_MOUNTS) && original_scopes(screen)
    })
}

fn focus_mounts_list(app: &App) {
    let screen = app.screen();
    let line = focused_line(&screen);
    if line.contains(DEST_GITCONFIG)
        || line.contains(DEST_CARGO)
        || line.contains(DEST_KUBE)
        || line.contains("Add mount")
    {
        return;
    }
    app.key("enter");
    app.wait_screen("mounts list", |screen| {
        let line = focused_line(screen);
        line.contains(DEST_GITCONFIG)
            || line.contains(DEST_CARGO)
            || line.contains(DEST_KUBE)
            || line.contains("Add mount")
    });
}

fn focus_dest(app: &App, marker: &str) {
    step_until(app, "tab", marker);
}

fn focus_row(app: &App, marker: &str) {
    if focused_line(&app.screen()).contains(marker) {
        return;
    }
    step_until(app, "down", marker);
}

fn set_destination(app: &App, marker: &str, value: &str) {
    focus_dest(app, marker);
    // macOS fields use the Emacs map, where select-all is ctrl+l.
    app.key("ctrl+l");
    app.key("backspace");
    app.type_text(value);
    let typed = app.wait_screen("destination text", |screen| {
        let line = focused_line(screen);
        line.contains(value) && line.matches(value).count() == 1
    });
    assert!(
        focused_line(&typed).contains(value),
        "destination field\n{}",
        focused_line(&typed)
    );
}

fn form_save(app: &App) -> Point {
    button(app, CONFIRM, |row| {
        row.contains(CANCEL)
            && row.contains(CONFIRM)
            && !row.contains("Save…")
            && !row.contains(CLEANUP)
            && !row.contains(PREVIEW_TITLE)
    })
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

fn choose_docs(app: &App) {
    let before = app.screen();
    require(&before, BROWSE);
    app.click(button(app, BROWSE, |row| row.contains(BROWSE)));
    let chooser = app.wait_screen("host chooser", |screen| {
        let visible = text(screen);
        visible.contains(CHOOSE_HOST) && visible.contains("docs/")
    });
    require(&chooser, "docs/");
    focus_row(app, "docs/");
    app.key("space");
    let chosen = app.wait_screen("host chosen", |screen| {
        let visible = text(screen);
        !visible.contains(CHOOSE_HOST) && visible.contains(HOST_DOCS) && visible.contains(NEW_MOUNT)
    });
    require(&chosen, HOST_DOCS);
    assert!(
        !text(&chosen).contains(NOT_CHOSEN),
        "Browse did not fill the host\n{}",
        text(&chosen)
    );
}

fn submit_form(app: &App) {
    app.click(form_save(app));
}

fn form_closed(screen: &TerminalSnapshot) -> bool {
    !text(screen).contains(NEW_MOUNT)
}

fn prepared_form(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(NEW_MOUNT)
        && visible.contains(HOST_DOCS)
        && visible.contains(HOST_KIND)
        && line.contains(DEST_FIELD)
        && line.matches(DEST_FIELD).count() == 1
        && !visible.contains(NOT_CHOSEN)
        && !visible.contains(AUTOFILL)
        && !visible.contains("added · save to apply")
        && !visible.contains(CHOOSE_HOST)
        && !visible.contains(SAVED)
        && !visible.contains(CLEANUP)
}

fn open_new_mount(app: &App) -> TerminalSnapshot {
    focus_mounts_list(app);
    app.key("a");
    let form = app.wait_screen("new mount", |screen| text(screen).contains(NEW_MOUNT));
    require(&form, HOST_KIND);
    require(&form, NOT_CHOSEN);
    form
}

fn dest_marker(app: &App) -> &'static str {
    let visible = text(&app.screen());
    if visible.contains(AUTOFILL) {
        AUTOFILL
    } else if visible.contains(PLACEHOLDER) {
        PLACEHOLDER
    } else {
        panic!("destination field has no known value\n{visible}");
    }
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the form stays open.
fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.nudge_width(quiet_frame, Some("nudge left the frame"));
}

fn shoot(app: &App, checkpoint: &str) {
    nudge(app, prepared_form);
    app.visual(&visual_id(checkpoint));
}

fn leave_settings(app: &App) -> TerminalSnapshot {
    app.key("escape");
    app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(CRUMB_GENERAL)
            && !text(screen).contains(CRUMB_MOUNTS)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains("Add mount")
    })
}

fn open_workspace_mounts(app: &App) -> TerminalSnapshot {
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
        let visible = text(screen);
        visible.contains("Add mount")
            && visible.contains(WORKSPACE_CRUMB)
            && mount_line(screen, DEST_PLATFORM).is_some()
            && mount_line(screen, DEST_LIBS).is_some()
            && !visible.contains(DEST_FIELD)
    })
}

#[test]
fn j03_mounts_route() {
    let app = open("j03-mounts-route");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let general = open_settings(&app);
    require(&general, CRUMB_GENERAL);
    let mounts = open_settings_mounts(&app);
    assert!(
        global_mounts(&mounts) && text(&mounts).contains(CRUMB_MOUNTS),
        "global mounts\n{}",
        text(&mounts)
    );
    require(&mounts, SCOPE_GLOBAL);
    require(&mounts, SCOPE_ROLE);
    let back = leave_settings(&app);
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(
        !text(&back).contains("added · save to apply"),
        "{}",
        text(&back)
    );
    assert!(!text(&back).contains(UNSAVED), "{}", text(&back));
    app.finish();
}

#[test]
fn j03_mounts_mounts() {
    let app = open("j03-mounts-mounts");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    open_settings(&app);
    let mounts = open_settings_mounts(&app);
    assert!(
        global_mounts(&mounts),
        "existing global mounts\n{}",
        text(&mounts)
    );

    open_new_mount(&app);
    choose_docs(&app);
    set_destination(&app, dest_marker(&app), DEST_FIELD);
    let wide = app.wait_screen("prepared form", prepared_form);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && prepared_form(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && prepared_form(screen)
    });

    submit_form(&app);
    let added = app.wait_screen("field notes added", |screen| {
        text(screen).contains(ADDED_FIELD)
            && form_closed(screen)
            && text(screen).contains(CRUMB_MOUNTS)
    });
    assert!(
        scoped_mount(&added, DEST_FIELD, SCOPE_GLOBAL)
            && scoped_mount(&added, DEST_GITCONFIG, SCOPE_GLOBAL)
            && scoped_mount(&added, DEST_KUBE, SCOPE_ROLE)
            && text(&added).contains(HOST_DOCS),
        "field-notes row\n{}",
        text(&added)
    );
    assert_eq!(
        text(&added)
            .lines()
            .filter(|line| line.contains(ADDED_FIELD))
            .count(),
        1,
        "status text\n{}",
        text(&added)
    );

    app.key("ctrl+s");
    let preview = app.wait_screen("save settings", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && visible.contains(DEST_FIELD)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
    });
    assert!(
        text(&preview).contains(PREVIEW_TITLE),
        "preview\n{}",
        text(&preview)
    );
    app.click(preview_save(&app));
    let saved = app.wait_screen("settings saved", |screen| {
        text(screen).contains(SAVED) && text(screen).contains(DIRECTORY)
    });
    assert!(!text(&saved).contains(NEW_MOUNT), "{}", text(&saved));

    open_settings(&app);
    let again = open_settings_mounts(&app);
    assert!(
        scoped_mount(&again, DEST_FIELD, SCOPE_GLOBAL)
            && scoped_mount(&again, DEST_GITCONFIG, SCOPE_GLOBAL)
            && scoped_mount(&again, DEST_CARGO, SCOPE_GLOBAL)
            && scoped_mount(&again, DEST_KUBE, SCOPE_ROLE),
        "reopened global mounts\n{}",
        text(&again)
    );
    leave_settings(&app);
    let workspace = open_workspace_mounts(&app);
    assert!(
        mount_line(&workspace, DEST_PLATFORM).is_some()
            && mount_line(&workspace, DEST_LIBS).is_some()
            && !text(&workspace).contains(DEST_FIELD)
            && text(&workspace).contains(WORKSPACE_CRUMB),
        "workspace mounts\n{}",
        text(&workspace)
    );
    app.finish();
}
