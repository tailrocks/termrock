//! Jackin export, driven through the real binary.
//!
//! `CapsuleScreen::hints` paints `Ctrl+\` for Palette on the resting capsule,
//! and `on_key` opens the palette on that chord (`src/bin/jackin/screens/capsule.rs`).
//! `run_palette` opens `Dialog::prompt` for "Export file". Confirm is Enter in
//! the field (`Dialog` action `Some(1)`), which calls `finish_export`. Cancel
//! is the Cancel button (action `Some(0)`), which does not. `finish_export`
//! only sets a status string. That string is not a host file.

use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, JACKIN, Launch, find_all, row_text, text, try_launch};

const INSTANCE: &str = "7f3a";
const OTHER: &str = "9b02";
const WORKSPACE: &str = "payments-platform";
const IDENTITY: &str = "payments-platform › the-architect";
const ATTACHED: &str = "Attached to payments-platform · tabs and panes restored";
const PAGE: &str = "Workspaces";
const CURRENT: &str = "Current directory";
const OUTRO: &str = "You were in the Construct";
const PALETTE: &str = "Command palette";
const FILTER: &str = "export file";
const FILTERED: &str = "6 of 20";
const TITLE: &str = "Export file";
const HELP: &str = "Copies to ~/Downloads/jackin/ on the host";
const PLACEHOLDER: &str = "workspace path or /jackin/run/";
const PATH: &str = "notes/j06-marker.txt";
const INVALID_PATH: &str = "/etc/passwd";
const OUTSIDE: &str = "Export failed: path is outside the workspace";
const EXPORTED: &str = "Exported to";
const SUCCESS: &str = "Exported to ~/Downloads/jackin/payments-platform/notes/j06-marker.txt";
const REVEALED: &str = "revealed in Finder";
const OPENED: &str = "opened on host";
const CANCELLED: &str = "Export cancelled";
const ENTER_PATH: &str = "Enter a path";
const DIRECTORY: &str = "Export failed: is a directory";
const WIDE: &str = "J06.export.export.120x40";
const NARROW: &str = "J06.export.export.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/j06_export/{}",
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
    // `Launch::new` leaves the clock off. The fixed frame keeps its attach footer.
    launch.clock = false;
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn focused_line(screen: &TerminalSnapshot) -> String {
    (0..screen.rows())
        .map(|row| row_text(screen, row))
        .find(|line| line.contains('▎'))
        .unwrap_or_default()
}

fn require(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        text(screen).contains(needle),
        "missing {needle}\n{}",
        text(screen)
    );
}

fn refuse(screen: &TerminalSnapshot, needle: &str) {
    assert!(
        !text(screen).contains(needle),
        "{needle} present\n{}",
        text(screen)
    );
}

fn wait_settled(
    app: &App,
    reason: &str,
    hold: Duration,
    budget: Duration,
    accept: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    let deadline = Instant::now() + budget;
    let mut last: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    loop {
        let screen = app.screen();
        if accept(&screen) {
            let same = last.as_ref().is_some_and(|prev| prev == &screen);
            if same {
                if since.expect("stability start").elapsed() >= hold {
                    return screen;
                }
            } else {
                last = Some(screen);
                since = Some(Instant::now());
            }
        } else {
            last = None;
            since = None;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{reason} frame not seen\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn instance_row(screen: &TerminalSnapshot) -> bool {
    let line = focused_line(screen);
    line.contains(INSTANCE) && !line.contains(OTHER)
}

fn manager_up(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(PAGE)
        && visible.contains(CURRENT)
        && visible.contains(WORKSPACE)
        && !visible.contains(OUTRO)
        && !visible.contains(ATTACHED)
}

/// `ManagerScreen::new` selects Current directory and leaves workspaces collapsed.
fn select_7f3a(app: &App) {
    let first = app.wait_screen("manager", manager_up);
    let line = focused_line(&first);
    if !instance_row(&first) {
        assert!(
            line.contains(CURRENT) && !line.contains(INSTANCE) && !line.contains(OTHER),
            "initial selection is not Current directory\n{line}\n{}",
            text(&first)
        );
    }
    if instance_row(&app.screen()) {
        return;
    }
    if focused_line(&app.screen()).contains(CURRENT) {
        app.key("down");
        app.wait_screen("payments-platform row", |screen| {
            let line = focused_line(screen);
            line.contains(WORKSPACE) && !line.contains(INSTANCE) && !line.contains(OTHER)
        });
    }
    if focused_line(&app.screen()).contains(WORKSPACE) && !focused_line(&app.screen()).contains('▾')
    {
        app.key("right");
        app.wait_screen("expanded payments-platform", |screen| {
            let line = focused_line(screen);
            line.contains('▾')
                && line.contains(WORKSPACE)
                && text(screen).contains(INSTANCE)
                && !line.contains(OTHER)
        });
    }
    for _ in 0..4 {
        if instance_row(&app.screen()) {
            return;
        }
        let before = focused_line(&app.screen());
        assert!(
            !before.contains(OTHER),
            "refusing to attach jk-{OTHER}\n{before}\n{}",
            text(&app.screen())
        );
        app.key("down");
        app.wait_screen("next row", |screen| focused_line(screen) != before);
    }
    assert!(
        instance_row(&app.screen()),
        "jk-{INSTANCE} row not selected\n{}",
        text(&app.screen())
    );
}

fn attach(app: &App) -> TerminalSnapshot {
    select_7f3a(app);
    assert!(
        instance_row(&app.screen()),
        "Enter would not attach jk-{INSTANCE}\n{}",
        text(&app.screen())
    );
    app.key("enter");
    app.wait_screen("attached", capsule_ready)
}

fn capsule_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    screen.cols() == 120
        && screen.rows() == 40
        && visible.contains(ATTACHED)
        && visible.contains(IDENTITY)
        && visible.contains("Ctrl+\\")
        && visible.contains("Palette")
        && !visible.contains(PAGE)
        && !visible.contains(OUTRO)
        && !visible.contains(EXPORTED)
}

fn plain_export_dialog(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(HELP)
        && visible.contains(PLACEHOLDER)
        && visible.contains("Cancel")
        && !visible.contains("and reveal")
        && !visible.contains("and open")
        && !visible.contains("under cursor")
        && !visible.contains(PALETTE)
        && !visible.contains(FILTERED)
        && !visible.contains(EXPORTED)
        && !visible.contains(OUTSIDE)
        && !visible.contains(OUTRO)
}

fn path_dialog(screen: &TerminalSnapshot, cols: u32, rows: u32) -> bool {
    let visible = text(screen);
    // A resize keeps the previous layout on screen until the app repaints. A
    // stale wide paint clips the Export button at 80 columns, so require the
    // button row to show whole.
    let button_row = visible
        .lines()
        .any(|line| line.contains("Cancel") && line.contains("Export"));
    screen.cols() == cols
        && screen.rows() == rows
        && button_row
        && visible.contains(PATH)
        && visible.contains(TITLE)
        && visible.contains(HELP)
        && visible.contains("Cancel")
        && !visible.contains(PLACEHOLDER)
        && !visible.contains("and reveal")
        && !visible.contains("and open")
        && !visible.contains(EXPORTED)
        && !visible.contains(OUTSIDE)
        && !visible.contains(OUTRO)
        && !visible.contains(PALETTE)
}

fn open_export(app: &App) -> TerminalSnapshot {
    let ready = attach(app);
    require(&ready, "Ctrl+\\");
    require(&ready, "Palette");
    app.key("ctrl+\\");
    app.wait_screen("command palette", |screen| {
        let visible = text(screen);
        visible.contains(PALETTE)
            && visible.contains("New tab")
            && visible.contains("20 of 20")
            && visible.contains(TITLE)
    });
    app.type_text(FILTER);
    app.wait_screen("export file filtered", |screen| {
        let visible = text(screen);
        visible.contains(FILTERED)
            && visible.contains(FILTER)
            && visible.contains(TITLE)
            && !visible.contains("New tab")
    });
    app.key("enter");
    app.wait_screen("export dialog", plain_export_dialog)
}

fn type_path(app: &App, path: &str) {
    app.type_text(path);
    app.wait_screen("path typed", |screen| {
        let visible = text(screen);
        visible.contains(path) && visible.contains(TITLE) && !visible.contains(PLACEHOLDER)
    });
}

fn paste_path(app: &App, path: &str) {
    app.paste(path);
    app.wait_screen("path pasted", |screen| {
        let visible = text(screen);
        visible.contains(path) && visible.contains(TITLE) && !visible.contains(PLACEHOLDER)
    });
}

fn export_dismissed(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(IDENTITY)
        && !visible.contains(HELP)
        && !visible.contains(EXPORTED)
        && !visible.contains(TITLE)
        && !visible.contains(OUTRO)
}

fn cancel_export(app: &App) -> TerminalSnapshot {
    // Tab from the path field reaches Cancel, then Export. A slow paint of
    // Cancel can arrive after the next Tab has already moved onto Export,
    // and Enter then confirms the export. Click the Cancel button, and
    // press Esc if that click lands while the dialog is still moving.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut acted: Option<Instant> = None;
    let mut used_esc = false;
    loop {
        let screen = app.screen();
        if export_dismissed(&screen) {
            return screen;
        }
        let visible = text(&screen);
        assert!(
            Instant::now() < deadline && app.child_running(),
            "export dismissed\n{visible}"
        );
        let due = acted.is_none_or(|at| at.elapsed() >= Duration::from_millis(350));
        if due {
            let hits: Vec<_> = find_all(&screen, "Cancel")
                .into_iter()
                .filter(|hit| {
                    let row = row_text(&screen, u32::from(hit.y));
                    row.contains("Export") && !row.contains("Esc")
                })
                .collect();
            if hits.len() == 1 && (used_esc || acted.is_none()) {
                let mut point = hits[0];
                point.x = point.x.saturating_add(3);
                app.click(point);
                acted = Some(Instant::now());
            } else if !used_esc {
                app.key("esc");
                used_esc = true;
                acted = Some(Instant::now());
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn shot(app: &App, checkpoint: &str, accept: impl Fn(&TerminalSnapshot) -> bool) {
    let test_id = visual_id(checkpoint);
    let screen = wait_settled(
        app,
        &test_id,
        Duration::from_millis(200),
        Duration::from_secs(8),
        accept,
    );
    check_frame(app, &screen, &test_id);
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = crate::support::workspace_root().join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store = tuiscotti_latest::check::FrozenStore::new(&root, &artifacts)
        .expect("open Tuiscotti frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        panic!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        );
    }
}

#[test]
fn j06_export_route() {
    let app = open("j06-export-route");
    let dialog = open_export(&app);
    require(&dialog, TITLE);
    require(&dialog, HELP);
    require(&dialog, PLACEHOLDER);
    refuse(&dialog, EXPORTED);
    refuse(&dialog, OUTRO);

    let back = cancel_export(&app);
    require(&back, IDENTITY);
    refuse(&back, EXPORTED);
    refuse(&back, HELP);
    refuse(&back, OUTSIDE);
    refuse(&back, ENTER_PATH);
    refuse(&back, CANCELLED);
    refuse(&back, OUTRO);
    assert!(app.child_running(), "leaving the dialog quit the process");
}

#[test]
fn j06_export_export() {
    let invalid = open("j06-export-invalid");
    open_export(&invalid);
    type_path(&invalid, INVALID_PATH);
    invalid.key("enter");
    let rejected = invalid.wait_screen("invalid path", |screen| {
        let visible = text(screen);
        visible.contains(OUTSIDE) && !visible.contains(EXPORTED)
    });
    require(&rejected, OUTSIDE);
    refuse(&rejected, EXPORTED);
    refuse(&rejected, SUCCESS);
    refuse(&rejected, ENTER_PATH);
    refuse(&rejected, DIRECTORY);
    refuse(&rejected, CANCELLED);
    refuse(&rejected, REVEALED);
    refuse(&rejected, OPENED);
    assert!(invalid.child_running(), "rejecting a path quit the process");

    let cancel = open("j06-export-cancel");
    open_export(&cancel);
    paste_path(&cancel, PATH);
    let wide = wait_settled(
        &cancel,
        "wide path dialog",
        Duration::from_millis(200),
        Duration::from_secs(8),
        |screen| path_dialog(screen, 120, 40),
    );
    require(&wide, PATH);
    refuse(&wide, EXPORTED);
    shot(&cancel, WIDE, |screen| path_dialog(screen, 120, 40));
    cancel.resize(80, 24);
    shot(&cancel, NARROW, |screen| path_dialog(screen, 80, 24));
    cancel.resize(120, 40);
    let restored = wait_settled(
        &cancel,
        "wide dialog restored",
        Duration::from_millis(200),
        Duration::from_secs(8),
        |screen| path_dialog(screen, 120, 40),
    );
    require(&restored, PATH);
    require(&restored, HELP);
    refuse(&restored, EXPORTED);
    let dismissed = cancel_export(&cancel);
    refuse(&dismissed, EXPORTED);
    refuse(&dismissed, SUCCESS);
    refuse(&dismissed, CANCELLED);
    refuse(&dismissed, OUTSIDE);
    assert!(cancel.child_running(), "cancelling export quit the process");

    let confirm = open("j06-export-confirm");
    open_export(&confirm);
    paste_path(&confirm, PATH);
    confirm.key("enter");
    let exported = confirm.wait_screen("exported", |screen| {
        let visible = text(screen);
        visible.contains(SUCCESS) && !visible.contains(OUTSIDE)
    });
    require(&exported, SUCCESS);
    refuse(&exported, OUTSIDE);
    refuse(&exported, ENTER_PATH);
    refuse(&exported, DIRECTORY);
    refuse(&exported, CANCELLED);
    refuse(&exported, REVEALED);
    refuse(&exported, OPENED);
    assert!(confirm.child_running(), "export quit the process");
}
