//! Jackin workspace accounts, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/j02_accounts/`. The shots are the Accounts list after Personal
//! is off for this workspace and Work is the stored Anthropic preference,
//! before Save. A same-size resize does not force a frame, and the cursor
//! column is part of the hash, so each shot nudges the width and comes back.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, button, row_text, step_until, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const NAME_HELP: &str = "Directory basename by default";
const PERSONAL: &str = "Personal";
const WORK: &str = "Work";
const INHERITED: &str = "inherited default";
const ENABLED_HERE: &str = "enabled here";
const ACTIVE: &str = "Active accounts";
const OFF: &str =
    "Claude · Personal · off for this Workspace · the global default stays registered";
const PREFERRED: &str = "Preferred for Anthropic ★ Claude · Work";
const CRUMB: &str = "Workspaces › payments-platform › edit";
const PREVIEW_TITLE: &str = "Save workspace";
const CLEANUP: &str = "Clean up & save…";
const SAVED: &str = "Workspace payments-platform saved";
const UNSAVED: &str = "Unsaved changes";
const CANCEL: &str = "Cancel";
const CONFIRM: &str = "Save";
const WIDE: &str = "J02.accounts.bindings.120x40";
const NARROW: &str = "J02.accounts.bindings.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_accounts/{}",
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

fn checkbox_line(line: &str) -> bool {
    line.contains("[✓]") || line.contains("[ ]")
}

fn name_token(line: &str, name: &str) -> bool {
    line.match_indices(name).any(|(at, _)| {
        let before = at == 0 || !line[..at].ends_with(|c: char| c.is_ascii_alphanumeric());
        let rest = &line[at + name.len()..];
        let after = !rest.starts_with(|c: char| c.is_ascii_alphanumeric());
        before && after
    })
}

fn account_line(line: &str, name: &str) -> bool {
    checkbox_line(line)
        && name_token(line, name)
        && (line.contains(INHERITED)
            || line.contains(ENABLED_HERE)
            || line.contains("disabled here")
            || line.contains("discovered on host")
            || line.contains("available"))
}

fn find_row(screen: &TerminalSnapshot, name: &str) -> Option<String> {
    let hits: Vec<String> = text(screen)
        .lines()
        .filter(|line| account_line(line, name))
        .map(str::to_string)
        .collect();
    if hits.len() == 1 {
        Some(hits[0].clone())
    } else {
        None
    }
}

fn row(screen: &TerminalSnapshot, name: &str) -> String {
    find_row(screen, name).unwrap_or_else(|| panic!("no single {name} row\n{}", text(screen)))
}

fn initial_accounts(screen: &TerminalSnapshot) -> bool {
    let Some(personal) = find_row(screen, PERSONAL) else {
        return false;
    };
    let Some(work) = find_row(screen, WORK) else {
        return false;
    };
    personal.contains("[✓]")
        && personal.contains(INHERITED)
        && personal.contains('★')
        && work.contains("[✓]")
        && work.contains(ENABLED_HERE)
        && !work.contains('★')
}

fn personal_off(screen: &TerminalSnapshot) -> bool {
    let Some(personal) = find_row(screen, PERSONAL) else {
        return false;
    };
    text(screen).contains(OFF) && personal.contains("[ ]") && !personal.contains('★')
}

fn list_ready(screen: &TerminalSnapshot) -> bool {
    let Some(personal) = find_row(screen, PERSONAL) else {
        return false;
    };
    let Some(work) = find_row(screen, WORK) else {
        return false;
    };
    let visible = text(screen);
    personal.contains("[ ]")
        && !personal.contains('★')
        && work.contains("[✓]")
        && work.contains('★')
        && work.contains(ENABLED_HERE)
        && !work.contains(INHERITED)
        && !visible.contains("★ Claude · Personal")
        && !visible.contains("Claude · Personal ★")
        && !visible.contains(PREVIEW_TITLE)
        && !visible.contains(CLEANUP)
        && !visible.contains(UNSAVED)
        && visible.contains(ACTIVE)
}

/// The preferred sentence is a five-second status. It is asserted while it is
/// on screen, then the shots wait until it leaves so the hash does not race
/// the status timer. At 80 columns the header also drops the workspace name.
fn quiet_list(screen: &TerminalSnapshot) -> bool {
    list_ready(screen) && !text(screen).contains(PREFERRED)
}

fn saved_accounts(screen: &TerminalSnapshot) -> bool {
    let Some(personal) = find_row(screen, PERSONAL) else {
        return false;
    };
    let Some(work) = find_row(screen, WORK) else {
        return false;
    };
    let visible = text(screen);
    work.contains("[✓]")
        && work.contains('★')
        && work.contains(ENABLED_HERE)
        && !work.contains(INHERITED)
        && personal.contains("[ ]")
        && !personal.contains('★')
        && !personal.contains(INHERITED)
        && !visible.contains("★ Claude · Personal")
        && !visible.contains("Claude · Personal ★")
        && visible.contains(CRUMB)
        && visible.contains(PAYMENTS)
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

fn open_accounts(app: &App) -> TerminalSnapshot {
    let before = app.screen();
    assert!(
        !text(&before).contains(ACTIVE),
        "General already shows Active accounts\n{}",
        text(&before)
    );
    for _ in 0..4 {
        app.key("]");
    }
    let screen = app.wait_screen("accounts tab", |screen| text(screen).contains(ACTIVE));
    assert!(
        initial_accounts(&screen),
        "initial account rows\n{}",
        text(&screen)
    );
    screen
}

fn focus_personal(app: &App) {
    if account_line(&focused_line(&app.screen()), PERSONAL) {
        return;
    }
    app.key("enter");
    app.wait_screen("personal row", |screen| {
        account_line(&focused_line(screen), PERSONAL)
    });
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

/// A same-size resize does not force a frame, and the cursor column is part
/// of the hash. Nudge the width and come back while the list stays put.
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

fn shoot(app: &App, checkpoint: &str) {
    nudge(app, quiet_list);
    app.visual(&visual_id(checkpoint));
}

fn reach_accounts(app: &App) -> TerminalSnapshot {
    let manager = reach_manager(app);
    require(&manager, DIRECTORY);
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let selected = select_payments(app);
    assert!(
        focused_line(&selected).contains(PAYMENTS),
        "{}",
        text(&selected)
    );
    let editor = open_editor(app);
    require(&editor, NAME_HELP);
    open_accounts(app)
}

#[test]
fn j02_accounts_route() {
    let app = open("j02-accounts-route");
    let accounts = reach_accounts(&app);
    assert!(
        initial_accounts(&accounts),
        "initial account rows\n{}",
        text(&accounts)
    );
    let personal = row(&accounts, PERSONAL);
    let work = row(&accounts, WORK);
    assert!(
        personal.contains("[✓]") && personal.contains(INHERITED) && personal.contains('★'),
        "personal row\n{personal}"
    );
    assert!(
        work.contains("[✓]") && work.contains(ENABLED_HERE) && !work.contains('★'),
        "work row\n{work}"
    );
    app.key("escape");
    let back = app.wait_screen("clean leave", |screen| {
        text(screen).contains(DIRECTORY)
            && !text(screen).contains(NAME_HELP)
            && !text(screen).contains(UNSAVED)
            && !text(screen).contains(ACTIVE)
    });
    assert!(!text(&back).contains(SAVED), "{}", text(&back));
    assert!(!text(&back).contains(OFF), "{}", text(&back));
    assert!(!text(&back).contains(PREFERRED), "{}", text(&back));
    app.finish();
}

#[test]
fn j02_accounts_bindings() {
    let app = open("j02-accounts-bindings");
    let accounts = reach_accounts(&app);
    assert!(
        initial_accounts(&accounts),
        "initial account rows\n{}",
        text(&accounts)
    );

    focus_personal(&app);
    let focused = focused_line(&app.screen());
    assert!(
        account_line(&focused, PERSONAL) && focused.contains("[✓]") && focused.contains('★'),
        "refusing space unless Personal is the focused inherited row\n{focused}"
    );
    app.key("space");
    let off = app.wait_screen("personal off", personal_off);
    assert!(text(&off).contains(OFF), "status\n{}", text(&off));
    let personal = row(&off, PERSONAL);
    assert!(
        personal.contains("[ ]") && !personal.contains('★'),
        "personal after space\n{personal}"
    );

    step_until(&app, "down", ENABLED_HERE);
    let work_focus = focused_line(&app.screen());
    assert!(
        account_line(&work_focus, WORK)
            && work_focus.contains("[✓]")
            && work_focus.contains(ENABLED_HERE),
        "refusing p unless Work is the focused active row\n{work_focus}\n{}",
        text(&app.screen())
    );
    app.key("p");
    let preferred = app.wait_screen("work preferred", |screen| {
        list_ready(screen) && text(screen).contains(PREFERRED)
    });
    assert_eq!((preferred.cols(), preferred.rows()), (120, 40));
    assert!(
        text(&preferred).contains(PREFERRED),
        "status\n{}",
        text(&preferred)
    );
    assert!(
        text(&preferred).contains(CRUMB),
        "workspace name\n{}",
        text(&preferred)
    );
    let personal = row(&preferred, PERSONAL);
    let work = row(&preferred, WORK);
    assert!(
        personal.contains("[ ]") && !personal.contains('★'),
        "personal stays off\n{personal}"
    );
    assert!(
        work.contains("[✓]") && work.contains('★'),
        "work preferred\n{work}"
    );
    assert!(
        !text(&preferred).contains("★ Claude · Personal")
            && !text(&preferred).contains("Claude · Personal ★"),
        "personal is starred\n{}",
        text(&preferred)
    );

    app.wait_screen("status cleared", quiet_list);
    shoot(&app, WIDE);
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && quiet_list(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert!(
        !text(&narrow).contains("★ Claude · Personal")
            && !text(&narrow).contains("Claude · Personal ★"),
        "narrow personal is starred\n{}",
        text(&narrow)
    );
    shoot(&app, NARROW);
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && quiet_list(screen)
    });

    app.key("ctrl+s");
    let preview = app.wait_screen("save preview", |screen| {
        let visible = text(screen);
        visible.contains(PREVIEW_TITLE)
            && focused_line(screen).contains(CANCEL)
            && !visible.contains(SAVED)
            && !visible.contains(CLEANUP)
    });
    assert!(
        focused_line(&preview).contains(CANCEL),
        "preview focus\n{}",
        focused_line(&preview)
    );
    app.click(preview_save(&app));
    let saved = app.wait_screen("saved", |screen| {
        text(screen).contains(SAVED) && text(screen).contains(DIRECTORY)
    });
    assert!(
        text(&saved).contains(PAYMENTS),
        "saved workspace\n{}",
        text(&saved)
    );

    if !focused_line(&saved).contains(PAYMENTS) {
        select_payments(&app);
    }
    open_editor(&app);
    let before = app.screen();
    assert!(
        !text(&before).contains(ACTIVE),
        "General already shows Active accounts\n{}",
        text(&before)
    );
    for _ in 0..4 {
        app.key("]");
    }
    let again = app.wait_screen("accounts again", saved_accounts);
    let personal = row(&again, PERSONAL);
    let work = row(&again, WORK);
    assert!(
        work.contains("[✓]") && work.contains('★') && !work.contains(INHERITED),
        "reopened work\n{work}"
    );
    assert!(
        personal.contains("[ ]") && !personal.contains('★') && !personal.contains(INHERITED),
        "reopened personal\n{personal}"
    );
    assert!(
        text(&again).contains(CRUMB),
        "workspace name\n{}",
        text(&again)
    );
    app.finish();
}
