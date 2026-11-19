//! Jackin workspace mounts, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j02_mounts/`. The shots are the open New mount form after a
//! host is chosen and the destination is `/workspace/payments-platform`,
//! before Save. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, button, row_text, step_until, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const NAME_HELP: &str = "Directory basename by default";
const DEST_PLATFORM: &str = "/workspace/payments-platform";
const DEST_LIBS: &str = "/workspace/libs";
const DEST_NOTES: &str = "/workspace/notes";
const DEST_NOTES_BARE: &str = "workspace/notes";
const DEST_RENAMED: &str = "/workspace/notes-renamed";
const DEST_FIELD: &str = "/workspace/field-notes";
const HOST_DOCS: &str = "~/src/payments-platform/docs";
const AUTOFILL: &str = "/workspace/docs";
const EDIT_FIELD: &str = "Edit mount · /workspace/field-notes";
const NEW_MOUNT: &str = "New mount";
const CHOOSE_HOST: &str = "Choose the host directory";
const NOT_CHOSEN: &str = "not chosen";
const HOST_KIND: &str = "Host directory";
const BROWSE: &str = "Browse…";
const NEED_HOST: &str = "Choose the host directory first";
const ABSOLUTE: &str = "Destination must be an absolute path inside the Construct";
const ADDED_CONFLICT: &str = "Mount /workspace/payments-platform added · save to apply";
const ADDED_BARE: &str = "Mount workspace/notes added";
const UPDATED: &str = "Mount /workspace/notes-renamed updated · save to apply";
const REMOVED: &str = "Removed /workspace/notes-renamed · u restores until save";
const ADDED_FIELD: &str = "Mount /workspace/field-notes added · save to apply";
const PREVIEW_TITLE: &str = "Save workspace";
const CLEANUP: &str = "Clean up & save…";
const SAVED: &str = "Workspace payments-platform saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const PLACEHOLDER: &str = "/workspace/name";
const WIDE: &str = "J02.mounts.mounts.120x40";
const NARROW: &str = "J02.mounts.mounts.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_mounts/{}",
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
    app.leave_intro("manager after intro", |screen| {
        text(screen).contains(DIRECTORY)
    })
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

fn has_dest(screen: &TerminalSnapshot, dest: &str) -> bool {
    text(screen).lines().any(|line| dest_token(line, dest))
}

fn mount_listed(screen: &TerminalSnapshot, dest: &str) -> bool {
    text(screen)
        .lines()
        .any(|line| dest_token(line, dest) && (line.contains(" rw") || line.contains(" ro")))
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

fn open_mounts(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        !text(&before).contains("Add mount"),
        "General already shows Add mount\n{}",
        text(&before)
    );
    app.key("]");
    app.wait_screen("mounts tab", |screen| {
        let visible = text(screen);
        visible.contains("Add mount")
            && has_dest(screen, DEST_PLATFORM)
            && has_dest(screen, DEST_LIBS)
    })
}

fn focus_mounts_list(app: &App) {
    let screen = app.screen();
    if focused_line(&screen).contains(DEST_PLATFORM)
        || focused_line(&screen).contains(DEST_LIBS)
        || focused_line(&screen).contains("Add mount")
    {
        return;
    }
    app.key("enter");
    app.wait_screen("mounts list", |screen| {
        let line = focused_line(screen);
        line.contains(DEST_PLATFORM) || line.contains(DEST_LIBS) || line.contains("Add mount")
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
    app.paste(value);
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
        "host still not chosen\n{}",
        text(&chosen)
    );
}

fn submit_form(app: &App) {
    app.click(form_save(app));
}

fn original_mounts(screen: &TerminalSnapshot) -> bool {
    mount_listed(screen, DEST_PLATFORM)
        && mount_listed(screen, DEST_LIBS)
        && !mount_listed(screen, DEST_NOTES)
        && !mount_listed(screen, DEST_RENAMED)
        && !mount_listed(screen, DEST_FIELD)
}

fn errors_dropped(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    !visible.contains(NEED_HOST)
        && !visible.contains("is already a mount destination")
        && !visible.contains(ABSOLUTE)
}

fn form_closed(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    !visible.contains(NEW_MOUNT) && !visible.contains("Edit mount ·")
}

fn prepared_form(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(NEW_MOUNT)
        && visible.contains(HOST_DOCS)
        && visible.contains(HOST_KIND)
        && line.contains(DEST_PLATFORM)
        && line.matches(DEST_PLATFORM).count() == 1
        && !visible.contains(NOT_CHOSEN)
        && !visible.contains("added · save to apply")
        && !visible.contains(CHOOSE_HOST)
        && !visible.contains(SAVED)
        && !visible.contains(CLEANUP)
        && errors_dropped(screen)
}

fn mount_count(screen: &TerminalSnapshot, dest: &str) -> usize {
    text(screen)
        .lines()
        .filter(|line| dest_token(line, dest) && (line.contains(" rw") || line.contains(" ro")))
        .count()
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

fn prepare_mount(app: &App, dest: &str) {
    open_new_mount(app);
    choose_docs(app);
    set_destination(app, dest_marker(app), dest);
}

fn open_new_mount(app: &App) -> TerminalSnapshot {
    focus_mounts_list(app);
    app.key("a");
    // The title and the host radio paint before "not chosen", Browse, and
    // the destination placeholder. The checkpoint is that later frame.
    let form = app.wait_screen("new mount", |screen| {
        let visible = text(screen);
        visible.contains(NEW_MOUNT)
            && visible.contains(HOST_KIND)
            && visible.contains(NOT_CHOSEN)
            && visible.contains(BROWSE)
            && visible.contains(PLACEHOLDER)
            && visible.lines().any(|line| {
                line.contains(CANCEL)
                    && line.contains(CONFIRM)
                    && line.contains('│')
                    && !line.contains("Save…")
            })
    });
    require(&form, HOST_KIND);
    require(&form, NOT_CHOSEN);
    form
}

fn add_field_notes(app: &App) {
    prepare_mount(app, DEST_FIELD);
    app.key("enter");
    submit_form(app);
    let added = app.wait_screen("field notes added", |screen| {
        text(screen).contains(ADDED_FIELD) && form_closed(screen)
    });
    assert!(
        mount_listed(&added, DEST_FIELD)
            && mount_listed(&added, DEST_PLATFORM)
            && mount_listed(&added, DEST_LIBS)
            && text(&added).contains(HOST_DOCS),
        "field-notes row\n{}",
        text(&added)
    );
}

#[test]
fn j02_mounts_route() {
    let app = open("j02-mounts-route");
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
    let mounts = open_mounts(&app);
    assert!(
        original_mounts(&mounts),
        "existing mounts\n{}",
        text(&mounts)
    );
    app.key("escape");
    let back = app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(NAME_HELP)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains("Add mount")
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(
        !text(&back).contains("added · save to apply"),
        "{}",
        text(&back)
    );
    app.finish();
}

#[test]
fn j02_mounts_mounts() {
    let app = open("j02-mounts-mounts");
    let manager = reach_manager(&app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    select_payments(&app);
    open_editor(&app);
    let mounts = open_mounts(&app);
    assert!(
        original_mounts(&mounts),
        "existing mounts\n{}",
        text(&mounts)
    );

    open_new_mount(&app);
    set_destination(&app, PLACEHOLDER, DEST_PLATFORM);
    // Commit the focused text input before clicking Save.
    app.key("enter");
    submit_form(&app);
    let missing_host = app.wait_screen("missing host rejected", |screen| {
        text(screen).contains(NEW_MOUNT)
            && text(screen).contains(NEED_HOST)
            && text(screen).contains(NOT_CHOSEN)
    });
    assert!(
        !text(&missing_host).contains(ADDED_CONFLICT),
        "missing host was accepted\n{}",
        text(&missing_host)
    );
    app.key("escape");
    let missing_host_dismissed = app.wait_screen("missing host cancelled", |screen| {
        form_closed(screen) && errors_dropped(screen) && original_mounts(screen)
    });
    assert!(
        !text(&missing_host_dismissed).contains(ADDED_CONFLICT)
            && mount_count(&missing_host_dismissed, DEST_PLATFORM) == 1,
        "invalid mount changed the workspace\n{}",
        text(&missing_host_dismissed)
    );
    app.wait_screen("cancel status clears", |screen| {
        !text(screen).contains("Cancelled · nothing changed")
    });

    prepare_mount(&app, DEST_PLATFORM);
    let wide = app.wait_screen("conflict form", prepared_form);
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

    app.key("enter");
    submit_form(&app);
    let conflict = app.wait_screen("duplicate mount rejected", |screen| {
        text(screen).contains(NEW_MOUNT) && text(screen).contains("already a mount destinati")
    });
    assert!(
        !text(&conflict).contains(ADDED_CONFLICT),
        "conflicting mount was accepted\n{}",
        text(&conflict)
    );
    assert_eq!(mount_count(&conflict, DEST_PLATFORM), 1);
    app.key("escape");
    let conflict_dismissed = app.wait_screen("duplicate mount cancelled", |screen| {
        form_closed(screen) && errors_dropped(screen) && original_mounts(screen)
    });
    assert_eq!(mount_count(&conflict_dismissed, DEST_PLATFORM), 1);
    app.wait_screen("conflict status clears", |screen| {
        !text(screen).contains("Cancelled · nothing changed")
    });

    prepare_mount(&app, DEST_NOTES_BARE);
    app.key("enter");
    submit_form(&app);
    let invalid = app.wait_screen("relative destination rejected", |screen| {
        text(screen).contains(NEW_MOUNT)
            && text(screen).contains("Destination must be an absolute path")
    });
    assert!(
        !text(&invalid).contains(ADDED_BARE)
            && !mount_listed(&invalid, DEST_NOTES)
            && mount_count(&invalid, DEST_PLATFORM) == 1,
        "invalid path changed the mount list\n{}",
        text(&invalid)
    );
    app.key("escape");
    let invalid_dismissed = app.wait_screen("relative destination cancelled", |screen| {
        form_closed(screen) && errors_dropped(screen) && original_mounts(screen)
    });
    assert_eq!(mount_count(&invalid_dismissed, DEST_PLATFORM), 1);
    app.wait_screen("invalid status clears", |screen| {
        !text(screen).contains("Cancelled · nothing changed")
    });

    add_field_notes(&app);

    focus_row(&app, DEST_FIELD);
    app.key("enter");
    let editing = app.wait_screen("edit field notes", |screen| {
        text(screen).contains(EDIT_FIELD)
    });
    require(&editing, DEST_FIELD);
    set_destination(&app, DEST_FIELD, DEST_LIBS);
    app.key("enter");
    submit_form(&app);
    app.wait_screen("edit conflict rejected", |screen| {
        text(screen).contains("Edit mount") && text(screen).contains("already a mount destinati")
    });
    app.key("escape");
    let edit_conflict_dismissed = app.wait_screen("edit conflict cancelled", |screen| {
        form_closed(screen)
            && errors_dropped(screen)
            && mount_listed(screen, DEST_FIELD)
            && mount_listed(screen, DEST_LIBS)
    });
    assert!(
        mount_listed(&edit_conflict_dismissed, DEST_PLATFORM)
            && mount_listed(&edit_conflict_dismissed, DEST_LIBS),
        "row after a rejected edit\n{}",
        text(&edit_conflict_dismissed)
    );
    app.wait_screen("edit conflict status clears", |screen| {
        !text(screen).contains("Cancelled · nothing changed")
    });

    focus_row(&app, DEST_FIELD);
    app.key("enter");
    app.wait_screen("edit again", |screen| text(screen).contains(EDIT_FIELD));
    set_destination(&app, DEST_FIELD, DEST_RENAMED);
    app.key("enter");
    submit_form(&app);
    let renamed = app.wait_screen("renamed", |screen| text(screen).contains(UPDATED));
    assert!(
        mount_listed(&renamed, DEST_RENAMED)
            && !mount_listed(&renamed, DEST_FIELD)
            && mount_listed(&renamed, DEST_LIBS)
            && mount_listed(&renamed, DEST_PLATFORM),
        "renamed row\n{}",
        text(&renamed)
    );

    focus_row(&app, DEST_RENAMED);
    app.key("d");
    let removed = app.wait_screen("removed", |screen| text(screen).contains(REMOVED));
    assert!(
        !mount_listed(&removed, DEST_RENAMED),
        "removed row stayed\n{}",
        text(&removed)
    );
    assert!(
        mount_listed(&removed, DEST_PLATFORM) && mount_listed(&removed, DEST_LIBS),
        "originals after remove\n{}",
        text(&removed)
    );

    add_field_notes(&app);
    app.key("ctrl+s");
    let preview = app.wait_screen("save preview", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && visible.contains(DEST_FIELD)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
    });
    assert!(
        text(&preview).contains("1 added") || text(&preview).contains("+ mount"),
        "preview mount change\n{}",
        text(&preview)
    );
    app.click(preview_save(&app));
    let saved = app.wait_screen("saved", |screen| {
        text(screen).contains(SAVED) && text(screen).contains(DIRECTORY)
    });
    assert!(!text(&saved).contains(NEW_MOUNT), "{}", text(&saved));

    if !focused_line(&saved).contains(PAYMENTS) {
        select_payments(&app);
    }
    open_editor(&app);
    let again = open_mounts(&app);
    assert!(
        mount_listed(&again, DEST_FIELD)
            && mount_listed(&again, DEST_PLATFORM)
            && mount_listed(&again, DEST_LIBS)
            && !has_dest(&again, DEST_NOTES)
            && !has_dest(&again, DEST_RENAMED),
        "saved mounts\n{}",
        text(&again)
    );
    app.finish();
}
