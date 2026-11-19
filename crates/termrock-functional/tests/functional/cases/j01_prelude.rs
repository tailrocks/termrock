//! Jackin workspace prelude, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j01_prelude/`. The shots are the open Name step with the
//! duplicate-name error, before a valid Create. A same-size resize does not
//! force a frame, and the cursor column is part of the hash, so each shot
//! nudges the width and comes back.
//!
//! `returning` leaves `save_fails_once` false, so the first save succeeds.
//! `create` does not write the file. The editor preview does.

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const RELEASE: &str = "release-automation";
const CUSTOMER: &str = "customer-portal";
const LAB: &str = "payments-lab";
const HOST_PATH: &str = "/Users/alexey/src/payments-platform";
const HOST_TILDE: &str = "~/src/payments-platform";
const GIT_URL: &str = "https://github.com/chainargos/payments-platform";
const BAD_URL: &str = "https://github.com/example/missing";
const BAD_HOST: &str = "example/missing";
const WORK_PATH: &str = "/work/payments-platform";
const SLASH_NAME: &str = "bad/name";
const SOURCE_TITLE: &str = "New workspace · step 1 of 5 · Source";
const DEST_TITLE: &str = "New workspace · step 2 of 5 · Destination";
const EDIT_TITLE: &str = "New workspace · step 3 of 5 · Edit destination";
const WORK_TITLE: &str = "New workspace · step 4 of 5 · Working dir";
const NAME_TITLE: &str = "New workspace · step 5 of 5 · Name";
const SAME_PATH: &str = "Same path";
const EDIT_OPTION: &str = "Edit destination…";
const GIT_LABEL: &str = "Git URL";
const COULD_NOT: &str = "Could not resolve";
const CANCELLED: &str = "Cancelled · nothing created";
const DUP_ERROR: &str = "A workspace named payments-platform already exists";
const SLASH_ERROR: &str = "Name cannot contain /";
const REVIEW: &str = "review and save";
const REVIEW_STATUS: &str = "Workspace payments-lab · review and save in the editor";
const CREATE_TITLE: &str = "Create workspace";
const CLEANUP: &str = "Clean up & save…";
const SAVED: &str = "Workspace payments-lab saved";
const NAME_HELP: &str = "Directory basename by default";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const BACK: &str = "Back";
const NEXT: &str = "Next";
const CREATE: &str = "Create";
const FOOTER_SAVE: &str = "Save…";
const WIDE: &str = "J01.prelude.steps.120x40";
const NARROW: &str = "J01.prelude.steps.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j01_prelude/{}",
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
    // The cancel status lives 5 s of sim time. Wall time must not move it
    // while the resize checks run under load.
    launch.clock = true;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn manager_ready(screen: &TerminalSnapshot) -> bool {
    // The detail pane names the directory before the tree lists the workspaces.
    text(screen).contains(DIRECTORY)
        && lists_workspace(screen, PAYMENTS)
        && lists_workspace(screen, INFRA)
        && lists_workspace(screen, RELEASE)
        && lists_workspace(screen, CUSTOMER)
}

fn reach_manager(app: &App) -> TerminalSnapshot {
    let screen = app.wait_screen("manager or intro", |screen| {
        manager_ready(screen) || text(screen).contains("Stand up, operator")
    });
    if text(&screen).contains("Stand up, operator") {
        app.key("space");
        return app.wait_screen("manager", manager_ready);
    }
    screen
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn token_on(line: &str, token: &str) -> bool {
    line.match_indices(token).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        !matches!(before, Some('/'))
    })
}

fn lists_workspace(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| {
        if line.contains(" saved")
            || line.contains("New workspace")
            || line.contains("already exists")
            || line.contains("Cancelled")
        {
            return false;
        }
        token_on(line, name)
    })
}

fn assert_original(screen: &TerminalSnapshot) {
    assert!(lists_workspace(screen, PAYMENTS), "{}", text(screen));
    assert!(lists_workspace(screen, INFRA), "{}", text(screen));
    assert!(lists_workspace(screen, RELEASE), "{}", text(screen));
    assert!(lists_workspace(screen, CUSTOMER), "{}", text(screen));
}

fn wait_title(app: &App, title: &str) -> TerminalSnapshot {
    app.wait_screen(title, |screen| text(screen).contains(title))
}

fn focused_has(app: &App, reason: &str, needle: &str) -> TerminalSnapshot {
    app.wait_screen(reason, |screen| focused_line(screen).contains(needle))
}

fn replace_focused(app: &App, old: &str, new: &str) -> TerminalSnapshot {
    app.key("ctrl+u");
    app.wait_screen("field cleared", |screen| {
        !focused_line(screen).contains(old)
    });
    app.type_text(new);
    app.wait_screen("field replaced", |screen| {
        let line = focused_line(screen);
        line.contains(new) && !line.contains(old)
    })
}

fn duplicate_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    let line = focused_line(screen);
    visible.contains(NAME_TITLE)
        && visible.contains(DUP_ERROR)
        && visible.contains(BACK)
        && visible.contains(CREATE)
        && token_on(&line, PAYMENTS)
        && !visible.contains(REVIEW)
        && !visible.contains(LAB)
        && !visible.contains(SLASH_ERROR)
        && !visible.contains(SAVED)
}

/// The reference frame still shows the cancel status. It expires 5 s of sim
/// time after Esc. The test clock holds sim time, so it stays until stepped.
fn status_duplicate(screen: &TerminalSnapshot) -> bool {
    duplicate_open(screen) && text(screen).contains(CANCELLED)
}

fn escape_until(app: &App, present: &str, absent: &str) {
    let visible = text(&app.screen());
    if visible.contains(present) && !visible.contains(absent) {
        return;
    }
    app.key("escape");
    app.wait_screen(present, |screen| {
        let visible = text(screen);
        visible.contains(present) && !visible.contains(absent)
    });
}

fn rearm_cancelled_duplicate(app: &App) {
    escape_until(app, WORK_TITLE, NAME_TITLE);
    escape_until(app, EDIT_TITLE, WORK_TITLE);
    escape_until(app, DEST_TITLE, EDIT_TITLE);
    escape_until(app, SOURCE_TITLE, DEST_TITLE);
    escape_until(app, CANCELLED, SOURCE_TITLE);
    open_source(app);
    app.key("g");
    app.wait_screen("git url again", |screen| {
        text(screen).contains(GIT_LABEL) && text(screen).contains(SOURCE_TITLE)
    });
    app.type_text(GIT_URL);
    app.wait_screen("url typed again", |screen| {
        text(screen).contains("chainargos/payments-platform")
    });
    app.key("enter");
    wait_title(app, DEST_TITLE);
    app.key("enter");
    wait_title(app, WORK_TITLE);
    app.key("enter");
    wait_title(app, NAME_TITLE);
    app.key("enter");
    app.wait_screen("duplicate while cancelled", status_duplicate);
}

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the dialog stays open.
fn nudge(app: &App, quiet_frame: impl Fn(&TerminalSnapshot) -> bool) {
    app.flash_expired();
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    assert!(
        quiet_frame(&screen),
        "nudge left the frame\n{}",
        text(&screen)
    );
    let before = text(&screen);
    let wider = cols + 1;
    app.resize(wider, rows);
    app.wait_screen("nudged", |screen| {
        screen.cols() == u32::from(wider) && screen.rows() == u32::from(rows) && quiet_frame(screen)
    });
    app.resize(cols, rows);
    // Under load the PTY reports the old size while the paint is still the
    // wider layout, so the restored frame must read as the frame before the nudge.
    app.wait_screen("size restored", |screen| {
        screen.cols() == u32::from(cols)
            && screen.rows() == u32::from(rows)
            && quiet_frame(screen)
            && text(screen) == before
    });
}

/// `visual` waits for a quiet screen, and that wait outlives the 5 s cancel
/// status. Compare the frame that still shows it.
fn shot_held(app: &App, checkpoint: &str) {
    nudge(app, status_duplicate);
    let screen = app.wait_screen("cancelled status", status_duplicate);
    crate::support::assert_frame(app, &screen, &visual_id(checkpoint));
}

fn preview_button(app: &App, label: &str) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| {
            let row = row_text(&screen, u32::from(point.y));
            row.contains(CANCEL)
                && row.contains(CONFIRM)
                && row.contains('│')
                && !row.contains(FOOTER_SAVE)
                && !row.contains(CREATE)
        })
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one preview button {label:?}:\n{}",
        text(&screen)
    );
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

fn focus_named(app: &App, name: &str) -> TerminalSnapshot {
    for _ in 0..30 {
        let screen = app.screen();
        let line = focused_line(&screen);
        if line.contains(name) {
            return screen;
        }
        app.key("down");
        app.wait_screen(name, |screen| focused_line(screen) != line);
    }
    let screen = app.screen();
    panic!("never focused {name}\n{}", text(&screen));
}

fn open_source(app: &App) -> TerminalSnapshot {
    let screen = app.screen();
    assert!(
        focused_line(&screen).contains(DIRECTORY),
        "n is Prelude only when the selection is not an instance\n{}",
        text(&screen)
    );
    app.key("n");
    // The dialog title paints before the parent row is focused.
    app.wait_screen(SOURCE_TITLE, |screen| {
        text(screen).contains(SOURCE_TITLE) && focused_line(screen).contains("..")
    })
}

#[test]
fn j01_prelude_route() {
    let app = open("j01-prelude-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    assert_original(&manager);
    assert!(
        !text(&manager).contains(LAB),
        "route started with an extra name\n{}",
        text(&manager)
    );
    let source = open_source(&app);
    assert!(text(&source).contains(SOURCE_TITLE), "{}", text(&source));
    app.key("escape");
    let back = app.wait_screen("source cancel", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED)
            && visible.contains(DIRECTORY)
            && !visible.contains(SOURCE_TITLE)
            && lists_workspace(screen, PAYMENTS)
    });
    assert_original(&back);
    assert!(
        !text(&back).contains(LAB),
        "cancel added a workspace\n{}",
        text(&back)
    );
    app.finish();
}

#[test]
fn j01_prelude_steps() {
    let app = open("j01-prelude-steps");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    assert_original(&manager);

    open_source(&app);
    app.key("space");
    let destination = wait_title(&app, DEST_TITLE);
    let host = text(&destination);
    assert!(host.contains(SAME_PATH), "{host}");
    assert!(host.contains(HOST_PATH), "{host}");
    assert!(host.contains(HOST_TILDE), "{host}");
    assert!(!host.contains(REVIEW), "{host}");
    app.key("escape");
    let source_again = wait_title(&app, SOURCE_TITLE);
    assert!(
        !text(&source_again).contains(DEST_TITLE),
        "{}",
        text(&source_again)
    );
    app.key("escape");
    let cancelled = app.wait_screen("host cancel", |screen| {
        let visible = text(screen);
        visible.contains(CANCELLED)
            && visible.contains(DIRECTORY)
            && !visible.contains(SOURCE_TITLE)
    });
    assert_original(&cancelled);
    assert!(
        !text(&cancelled).contains(LAB),
        "host cancel added a workspace\n{}",
        text(&cancelled)
    );

    open_source(&app);
    app.key("g");
    let url_field = app.wait_screen("git url", |screen| {
        text(screen).contains(GIT_LABEL) && text(screen).contains(SOURCE_TITLE)
    });
    assert!(text(&url_field).contains(GIT_LABEL), "{}", text(&url_field));
    app.type_text(BAD_URL);
    app.key("enter");
    let unresolved = app.wait_screen("unresolved", |screen| {
        let visible = text(screen);
        visible.contains(COULD_NOT)
            && visible.contains(SOURCE_TITLE)
            && !visible.contains(DEST_TITLE)
    });
    assert!(
        text(&unresolved).contains(BAD_HOST),
        "{}",
        text(&unresolved)
    );
    replace_focused(&app, BAD_HOST, GIT_URL);
    app.key("enter");
    // Step 2's title paints over the unresolved error before that line clears.
    let git_dest = app.wait_screen("git destination", |screen| {
        let visible = text(screen);
        visible.contains(DEST_TITLE)
            && visible.contains("Default")
            && visible.contains(WORK_PATH)
            && !visible.contains(BAD_HOST)
            && !visible.contains(COULD_NOT)
    });
    let git = text(&git_dest);
    // ChoiceDialog::width(58) cuts this source line before " · git".
    assert!(
        git.contains("https://github.com/chainargos/payments-plat"),
        "{git}"
    );
    assert!(git.contains("Default"), "{git}");
    assert!(git.contains(WORK_PATH), "{git}");
    assert!(!git.contains(BAD_HOST), "{git}");
    assert!(!git.contains(COULD_NOT), "{git}");
    assert!(git.contains(BACK), "{git}");
    assert!(git.contains(NEXT), "{git}");
    assert!(git.contains(EDIT_OPTION), "{git}");
    let first = focused_line(&git_dest);
    assert!(first.contains("Default"), "initial option\n{first}\n{git}");
    assert!(
        !first.contains(NEXT),
        "Next is not the focused control\n{first}"
    );

    app.key("down");
    let edit_option = focused_has(&app, "edit destination", EDIT_OPTION);
    assert!(
        !focused_line(&edit_option).contains(NEXT),
        "Down must not land on Next\n{}",
        text(&edit_option)
    );
    app.key("enter");
    let edit = wait_title(&app, EDIT_TITLE);
    assert!(text(&edit).contains(WORK_PATH), "{}", text(&edit));
    assert!(!text(&edit).contains(WORK_TITLE), "{}", text(&edit));
    app.key("escape");
    // The destination title paints before focus returns to this option.
    app.wait_screen("edit destination stays selected", |screen| {
        text(screen).contains(DEST_TITLE)
            && !text(screen).contains(EDIT_TITLE)
            && focused_line(screen).contains(EDIT_OPTION)
    });
    app.key("enter");
    // The edit title paints before focus lands in the path field.
    let edit_again = app.wait_screen("destination field", |screen| {
        text(screen).contains(EDIT_TITLE)
            && !text(screen).contains(WORK_TITLE)
            && focused_line(screen).contains(WORK_PATH)
    });
    assert!(
        text(&edit_again).contains(WORK_PATH),
        "{}",
        text(&edit_again)
    );
    app.key("enter");
    let workdir = wait_title(&app, WORK_TITLE);
    assert!(text(&workdir).contains(WORK_PATH), "{}", text(&workdir));
    app.key("escape");
    let edit_back = wait_title(&app, EDIT_TITLE);
    assert!(
        !text(&edit_back).contains(WORK_TITLE),
        "Workdir Esc returned to Edit\n{}",
        text(&edit_back)
    );
    app.key("enter");
    wait_title(&app, WORK_TITLE);
    app.key("enter");
    let name = wait_title(&app, NAME_TITLE);
    let name_line = focused_line(&name);
    assert!(
        token_on(&name_line, PAYMENTS),
        "basename field\n{name_line}\n{}",
        text(&name)
    );
    app.key("escape");
    let workdir_back = wait_title(&app, WORK_TITLE);
    assert!(
        !text(&workdir_back).contains(NAME_TITLE),
        "Name Esc returned to Workdir\n{}",
        text(&workdir_back)
    );
    app.key("enter");
    let name_again = wait_title(&app, NAME_TITLE);
    let again = focused_line(&name_again);
    assert!(token_on(&again, PAYMENTS), "name restored\n{again}");
    assert!(!text(&name_again).contains(REVIEW), "{}", text(&name_again));
    app.key("enter");
    let checkpoint = app.wait_screen("duplicate name", duplicate_open);
    assert_eq!((checkpoint.cols(), checkpoint.rows()), (120, 40));
    rearm_cancelled_duplicate(&app);
    // `visual` waits for a quiet screen, and that wait outlives the 5 s
    // cancel status on both sizes.
    shot_held(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && status_duplicate(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    shot_held(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && duplicate_open(screen)
    });

    replace_focused(&app, PAYMENTS, SLASH_NAME);
    app.key("enter");
    let slash = app.wait_screen("slash name", |screen| {
        let visible = text(screen);
        visible.contains(NAME_TITLE)
            && visible.contains(SLASH_ERROR)
            && !visible.contains(REVIEW)
            && !visible.contains(SAVED)
            && focused_line(screen).contains(SLASH_NAME)
    });
    assert!(text(&slash).contains(NAME_TITLE), "{}", text(&slash));
    assert!(text(&slash).contains(SLASH_ERROR), "{}", text(&slash));
    assert!(!text(&slash).contains(REVIEW), "{}", text(&slash));
    assert!(!text(&slash).contains(SAVED), "{}", text(&slash));

    replace_focused(&app, SLASH_NAME, LAB);
    app.key("enter");
    let editor = app.wait_screen("created", |screen| {
        let visible = text(screen);
        visible.contains(REVIEW_STATUS)
            && visible.contains(NAME_HELP)
            && visible.contains(LAB)
            && !visible.contains(NAME_TITLE)
    });
    assert!(text(&editor).contains(LAB), "{}", text(&editor));

    app.key("ctrl+s");
    let preview = app.wait_screen("create preview", |screen| {
        let visible = text(screen);
        let line = focused_line(screen);
        visible.contains(CANCEL)
            && visible.contains(CONFIRM)
            && line.contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains("Saving")
    });
    let painted = text(&preview);
    assert!(!painted.contains(CLEANUP), "cleanup blockers\n{painted}");
    assert!(
        painted.contains(CREATE_TITLE),
        "create preview title\n{painted}"
    );
    assert!(
        focused_line(&preview).contains(CANCEL),
        "focused preview\n{}",
        focused_line(&preview)
    );
    app.click(preview_button(&app, CONFIRM));
    // The sim backend settles a workspace save 900 ms after the request.
    app.advance_clock(900);
    let saved = app.wait_screen("saved", |screen| {
        let visible = text(screen);
        visible.contains(SAVED)
            && visible.contains(DIRECTORY)
            && lists_workspace(screen, LAB)
            && lists_workspace(screen, PAYMENTS)
    });
    assert_original(&saved);
    assert!(lists_workspace(&saved, LAB), "{}", text(&saved));
    assert!(
        !lists_workspace(&saved, SLASH_NAME),
        "slash name created a workspace\n{}",
        text(&saved)
    );

    focus_named(&app, LAB);
    app.key("e");
    let reopened = app.wait_screen("reopened", |screen| {
        let visible = text(screen);
        visible.contains(NAME_HELP) && visible.contains(LAB) && !visible.contains(NAME_TITLE)
    });
    assert!(text(&reopened).contains(LAB), "{}", text(&reopened));
    assert!(
        focused_line(&reopened).contains("General"),
        "reopen focuses the tab strip\n{}",
        text(&reopened)
    );
    app.key("]");
    // config.rs column_widths gives the Source cell the leftover width, so the
    // full URL ends in the ellipsis. The painted prefix is the saved git URL.
    let mounts = app.wait_screen("mounts", |screen| {
        let visible = text(screen);
        visible.contains("https://github.com/chainargos/payments-platfo")
            && visible.contains(WORK_PATH)
            && visible.contains("git")
    });
    assert!(
        text(&mounts).contains("https://github.com/chainargos/payments-platfo"),
        "{}",
        text(&mounts)
    );
    assert!(text(&mounts).contains(LAB), "{}", text(&mounts));
    assert!(!text(&mounts).contains(BAD_HOST), "{}", text(&mounts));
    app.finish();
}
