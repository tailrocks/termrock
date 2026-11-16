//! Jackin workspace roles, driven through the real binary.
//!
//! Tuiscotti test ids reject `.` and uppercase, so the baseline directory is
//! `pre_refactor/j02_roles/`. The shots are the open Trust role source dialog
//! for data-eng, before Trust & load. A same-size resize does not force a
//! frame, and the cursor column is part of the hash, so each shot nudges the
//! width and comes back.
//!
//! `roles()` in `src/bin/jackin/domain/fixtures.rs` gives data-eng
//! `load_error: Some("trust required")` and `trusted: false`. Confirming the
//! dialog schedules `RoleLoaded` 1100 ms later on the sim clock. This test
//! waits on the wall clock. It does not advance the test clock.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;

use crate::support::{App, JACKIN, Launch, Point, find_all, row_text, text, try_launch};

const DIRECTORY: &str = "Current directory";
const PAYMENTS: &str = "payments-platform";
const INFRA: &str = "infra-control-plane";
const RELEASE: &str = "release-automation";
const CUSTOMER: &str = "customer-portal";
const ARCHITECT: &str = "the-architect";
const BACKEND: &str = "backend";
const REVIEWER: &str = "reviewer";
const DATA_ENG: &str = "data-eng";
const SRE: &str = "chainargos/sre";
const ROLE_FACT: &str = "acme-labs/data-eng";
const FAILURE: &str = "trust required";
const ALLOWED: &str = "Allowed roles";
const TRUST_TITLE: &str = "Trust role source";
const TRUST_BUTTON: &str = "Trust & load";
const LOADED_DATA: &str = "Role data-eng loaded and allowed";
const LOADED_SRE: &str = "Role sre loaded and allowed";
const PICKER: &str = "Load role";
const PICKER_SCOPE: &str = "trusted role registry";
const CANCEL: &str = "Cancel";
const DISCARD: &str = "Discard";
const SAVE: &str = "Save";
const UNSAVED: &str = "Unsaved changes";
const CRUMB: &str = "payments-platform › edit";
const WIDE: &str = "J02.roles.roles.120x40";
const NARROW: &str = "J02.roles.roles.narrow";
const SAVED: &[&str] = &[ARCHITECT, BACKEND, REVIEWER];

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j02_roles/{}",
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

fn lists_workspace(screen: &TerminalSnapshot, name: &str) -> bool {
    text(screen).lines().any(|line| {
        if line.contains("› edit") || line.contains("Role ") || line.contains("loaded") {
            return false;
        }
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            !matches!(before, Some('/'))
        })
    })
}

fn assert_siblings(screen: &TerminalSnapshot) {
    assert!(lists_workspace(screen, INFRA), "{}", text(screen));
    assert!(lists_workspace(screen, RELEASE), "{}", text(screen));
    assert!(lists_workspace(screen, CUSTOMER), "{}", text(screen));
}

fn assert_saved_roles(screen: &TerminalSnapshot) {
    let visible = text(screen);
    for name in SAVED {
        assert!(visible.contains(name), "missing {name}\n{visible}");
    }
}

fn on_payments(screen: &TerminalSnapshot) -> bool {
    text(screen).contains(PAYMENTS)
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
    app.wait_text(CRUMB)
}

fn open_roles(app: &App) -> TerminalSnapshot {
    app.key("]");
    app.key("]");
    app.wait_text(ALLOWED)
}

/// data-eng is the fifth registry row. Scroll only when the first paint hides it.
fn show_roles(app: &App) -> TerminalSnapshot {
    let opened = app.screen();
    if !text(&opened).contains(DATA_ENG) {
        enter_role_list(app);
        for _ in 0..8 {
            let screen = app.screen();
            if text(&screen).contains(DATA_ENG) {
                break;
            }
            let line = focused_line(&screen);
            app.key("down");
            app.wait_screen("scroll roles", |screen| focused_line(screen) != line);
        }
    }
    let screen = app.screen();
    let visible = text(&screen);
    if !visible.contains(DATA_ENG) {
        panic!("data-eng never appeared\n{visible}");
    }
    for name in [ARCHITECT, BACKEND, REVIEWER, DATA_ENG] {
        assert!(visible.contains(name), "missing {name}\n{visible}");
    }
    assert!(
        visible.contains(FAILURE),
        "data-eng failure is not painted\n{visible}"
    );
    screen
}

fn role_row(line: &str) -> bool {
    line.contains("[✓]") || line.contains("[ ]") || line.contains("Load role")
}

fn enter_role_list(app: &App) {
    let screen = app.screen();
    if role_row(&focused_line(&screen)) {
        return;
    }
    app.key("enter");
    app.wait_screen("role list", |screen| role_row(&focused_line(screen)));
}

fn focus_data_eng(app: &App) -> TerminalSnapshot {
    enter_role_list(app);
    for _ in 0..8 {
        let screen = app.screen();
        let line = focused_line(&screen);
        if line.contains(DATA_ENG) {
            assert!(
                line.contains(FAILURE) || line.contains("load error"),
                "focused data-eng row has no failure\n{line}\n{}",
                text(&screen)
            );
            return screen;
        }
        app.key("down");
        app.wait_screen("next role", |screen| focused_line(screen) != line);
    }
    let screen = app.screen();
    panic!("never focused data-eng\n{}", text(&screen));
}

fn cancel_focused(line: &str) -> bool {
    let Some(at) = line.find('▎') else {
        return false;
    };
    let after = &line[at..];
    let Some(cancel) = after.find(CANCEL) else {
        return false;
    };
    match after.find(TRUST_BUTTON) {
        Some(trust) => cancel < trust,
        None => true,
    }
}

fn trust_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TRUST_TITLE)
        && visible.contains(ROLE_FACT)
        && visible.contains(TRUST_BUTTON)
        && visible.contains(CANCEL)
        && cancel_focused(&focused_line(screen))
        && !visible.contains(LOADED_DATA)
        && !visible.contains("Loading role")
}

fn button(app: &App, label: &str, row_ok: impl Fn(&str) -> bool) -> Point {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|point| row_ok(&row_text(&screen, u32::from(point.y))))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "one dialog button {label:?}:\n{}",
        text(&screen)
    );
    Point {
        x: hits[0].x + 2,
        y: hits[0].y,
    }
}

fn trust_button(app: &App) -> Point {
    button(app, TRUST_BUTTON, |row| {
        row.contains(CANCEL) && row.contains(TRUST_BUTTON) && row.contains('│')
    })
}

fn leave_button(app: &App, label: &str) -> Point {
    button(app, label, |row| {
        row.contains(CANCEL) && row.contains(DISCARD) && row.contains(SAVE) && row.contains('│')
    })
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

fn manager_again(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DIRECTORY)
        && !visible.contains(ALLOWED)
        && !visible.contains(TRUST_TITLE)
        && !visible.contains(UNSAVED)
        && lists_workspace(screen, PAYMENTS)
}

/// Esc from the tab strip leaves. Esc from the role list focuses the strip
/// first. A dirty allow-list opens Unsaved changes; Discard does not write.
fn leave_editor(app: &App, allow_unsaved: bool) {
    for _ in 0..4 {
        let screen = app.screen();
        let visible = text(&screen);
        if visible.contains(UNSAVED) {
            assert!(
                allow_unsaved,
                "unsaved dialog when nothing was edited\n{visible}"
            );
            app.click(leave_button(app, DISCARD));
            app.wait_screen("discarded to manager", manager_again);
            return;
        }
        if manager_again(&screen) {
            return;
        }
        app.key("escape");
        app.wait_screen("escape", |screen| text(screen) != visible);
    }
    let screen = app.screen();
    panic!("editor did not close\n{}", text(&screen));
}

fn loaded_data(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(LOADED_DATA)
        && !visible.contains(FAILURE)
        && !visible.contains("load error")
        && !visible.contains(TRUST_TITLE)
        && visible.contains(PAYMENTS)
        && SAVED.iter().all(|name| visible.contains(name))
}

fn loaded_sre(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(LOADED_SRE)
        && !visible.contains(FAILURE)
        && visible.contains(PAYMENTS)
        && visible.contains(DATA_ENG)
        && SAVED.iter().all(|name| visible.contains(name))
}

fn load_sre(app: &App) -> TerminalSnapshot {
    app.key("a");
    // The dialog chrome paints before the registry rows. The first frame
    // that says Load role does not yet contain chainargos/sre.
    app.wait_screen("load role picker", |screen| {
        let visible = text(screen);
        visible.contains(PICKER) && visible.contains(PICKER_SCOPE) && visible.contains(SRE)
    });
    app.key("s");
    app.key("r");
    app.key("e");
    app.wait_screen("sre filtered", |screen| {
        let visible = text(screen);
        visible.contains(SRE) && !visible.contains("chainargos/the-architect")
    });
    app.key("enter");
    app.wait_screen("sre loaded", loaded_sre)
}

#[test]
fn j02_roles_route() {
    let app = open("j02-roles-route");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    let selected = select_payments(&app);
    assert!(
        focused_line(&selected).contains(PAYMENTS),
        "{}",
        text(&selected)
    );
    assert_siblings(&selected);
    let editor = open_editor(&app);
    assert!(text(&editor).contains(CRUMB), "{}", text(&editor));
    let roles = open_roles(&app);
    assert!(text(&roles).contains(ALLOWED), "{}", text(&roles));
    let listed = show_roles(&app);
    assert_saved_roles(&listed);
    assert!(on_payments(&listed), "{}", text(&listed));
    leave_editor(&app, false);
    let back = app.screen();
    assert!(manager_again(&back), "{}", text(&back));
    assert_siblings(&back);
    assert!(lists_workspace(&back, PAYMENTS), "{}", text(&back));
    assert!(!text(&back).contains("new workspace"), "{}", text(&back));
    app.finish();
}

#[test]
fn j02_roles_roles() {
    let app = open("j02-roles-roles");
    let manager = reach_manager(&app);
    assert!(text(&manager).contains(DIRECTORY), "{}", text(&manager));
    assert_eq!((manager.cols(), manager.rows()), (120, 40));
    // The directory header paints before the workspace rows. The first
    // manager frame lists Current directory and leaves the siblings blank.
    let manager = app.wait_screen("workspace siblings", |screen| {
        screen.cols() == 120
            && screen.rows() == 40
            && lists_workspace(screen, INFRA)
            && lists_workspace(screen, RELEASE)
            && lists_workspace(screen, CUSTOMER)
    });
    assert_siblings(&manager);
    select_payments(&app);
    let editor = open_editor(&app);
    assert!(text(&editor).contains(CRUMB), "{}", text(&editor));
    open_roles(&app);
    let listed = show_roles(&app);
    assert_saved_roles(&listed);
    let failed = focus_data_eng(&app);
    assert!(
        text(&failed).contains(FAILURE),
        "failure row\n{}",
        text(&failed)
    );
    assert_saved_roles(&failed);
    app.key("enter");
    let dialog = app.wait_screen("trust dialog", trust_open);
    assert!(trust_open(&dialog), "{}", text(&dialog));
    assert_eq!((dialog.cols(), dialog.rows()), (120, 40));
    nudge(&app, trust_open);
    app.visual(&visual_id(WIDE));
    app.resize(80, 24);
    let narrow = app.wait_screen("80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && trust_open(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    nudge(&app, trust_open);
    app.visual(&visual_id(NARROW));
    app.resize(120, 40);
    app.wait_screen("120x40 again", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && trust_open(screen)
    });

    app.click(trust_button(&app));
    let loaded = app.wait_screen("data-eng loaded", loaded_data);
    assert!(loaded_data(&loaded), "{}", text(&loaded));
    assert!(text(&loaded).contains(CRUMB) || text(&loaded).contains(PAYMENTS));
    let sre = load_sre(&app);
    assert!(loaded_sre(&sre), "{}", text(&sre));
    assert!(text(&sre).contains(CRUMB) || text(&sre).contains(PAYMENTS));
    leave_editor(&app, true);
    let back = app.screen();
    assert!(manager_again(&back), "{}", text(&back));
    assert!(lists_workspace(&back, PAYMENTS), "{}", text(&back));
    assert_siblings(&back);
    assert!(!text(&back).contains("new workspace"), "{}", text(&back));
    app.finish();
}
