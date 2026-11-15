//! TablePro quit, driven through the real binary.
//!
//! Tuiscotti test ids are `[a-z0-9_-]` segments, so the baseline directory is
//! `pre_refactor/t05_quit/` and each checkpoint's dots become underscores.
//! TablePro has no `--page` flag. It opens on Connections.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, Launch, TABLEPRO, find_all, flash_settle, row_text, text, try_launch};

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";
const SQL: &str = "SELECT 't05_quit_keep' AS marker";
const MARKER: &str = "t05_quit_keep";
const TITLE: &str = "Quit TablePro?";
const BODY: &str = "1 unsaved query will be lost.";
const CANCEL: &str = "Cancel";
const QUIT: &str = "Quit";
const FOCUS: &str = "▎";
const WIDE: &str = "T05.quit.quit.120x40";
const NARROW: &str = "T05.quit.quit.narrow";

fn visual_id(checkpoint: &str) -> String {
    format!(
        "pre_refactor/t05_quit/{}",
        checkpoint.replace('.', "_").to_ascii_lowercase()
    )
}

fn open(name: &str) -> App {
    let mut launch = Launch::new(name, TABLEPRO);
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    let app = try_launch(launch).unwrap_or_else(|error| panic!("{error}"));
    app.wait_text("Filter connections");
    app
}

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    let visible = text(screen);
    for item in present {
        assert!(visible.contains(item), "missing {item:?}\n{visible}");
    }
    for item in absent {
        assert!(!visible.contains(item), "forbidden {item:?}\n{visible}");
    }
}

fn row_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    (0..screen.rows()).any(|row| row_text(screen, row).contains(needle))
}

fn click_listed(app: &App, label: &str) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| hit.x <= 35)
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "list hit {label:?}: {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn click_connect(app: &App) {
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, "Connect")
        .into_iter()
        .filter(|hit| {
            let row = row_text(&screen, u32::from(hit.y));
            hit.x >= 40
                && row.contains("Edit")
                && row.contains("Delete")
                && !row.contains("Reconnect")
        })
        .collect();
    assert_eq!(hits.len(), 1, "connect button {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
}

fn connect_development(app: &App) {
    click_listed(app, "Development");
    let detail = app.wait_text(HOST);
    see(
        &detail,
        &[HOST, DATABASE, "Development"],
        &["Authentication failed", "Could not reach the host"],
    );
    click_connect(app);
    // Query 1 is drawn during "Loading schema…", before the connected status.
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, "acme_prod › public", QUERY, PLACEHOLDER],
        &[
            "Authentication failed",
            "Could not reach the host",
            "Scratch",
            "~/scratch.db",
            "Loading schema",
            TITLE,
        ],
    );
}

fn click_match(app: &App, label: &str, keep: impl Fn(&TerminalSnapshot, u16, u16) -> bool) {
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| keep(&screen, hit.x, hit.y))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "click {label:?} got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn focus_editor(app: &App) {
    app.wait_text(PLACEHOLDER);
    click_match(app, PLACEHOLDER, |screen, x, y| {
        x > 20 && row_text(screen, u32::from(y)).contains(PLACEHOLDER)
    });
    app.wait_text("Ctrl+Space");
}

fn quit_copy(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(TITLE)
        && visible.contains(BODY)
        && visible.contains(CANCEL)
        && visible.contains(QUIT)
        && visible.contains(MARKER)
        && !visible.contains("Save")
        && !visible.contains("Discard")
}

fn quit_dialog(screen: &TerminalSnapshot) -> bool {
    quit_copy(screen) && text(screen).contains(CONNECTED)
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
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

fn cancel_focused(screen: &TerminalSnapshot) -> bool {
    row_has(screen, &format!("{FOCUS}{CANCEL}")) && !row_has(screen, &format!("{FOCUS}{QUIT}"))
}

fn quit_focused(screen: &TerminalSnapshot) -> bool {
    row_has(screen, &format!("{FOCUS}{QUIT}")) && !row_has(screen, &format!("{FOCUS}{CANCEL}"))
}

fn assert_cancel_focused(screen: &TerminalSnapshot) {
    let visible = text(screen);
    assert!(quit_copy(screen), "quit dialog is not open\n{visible}");
    assert!(cancel_focused(screen), "Cancel is not focused\n{visible}");
}

/// The connected toast lasts 5s. A resize paint can show the dialog before
/// that footer line, and `visual` then locks the cleared toast. Hold the
/// focused Cancel dialog while the toast is up, and compare that frame.
fn shot(app: &App, checkpoint: &str, cols: u32, rows: u32) {
    let test_id = visual_id(checkpoint);
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut good: Option<TerminalSnapshot> = None;
    let mut since: Option<Instant> = None;
    loop {
        let screen = app.screen();
        let ready = screen.cols() == cols
            && screen.rows() == rows
            && quit_dialog(&screen)
            && cancel_focused(&screen);
        if ready {
            let same = good.as_ref().is_some_and(|prev| prev == &screen);
            if same && since.expect("hold start").elapsed() >= Duration::from_millis(200) {
                check_frame(app, &screen, &test_id);
                return;
            }
            if !same {
                good = Some(screen);
                since = Some(Instant::now());
            }
        } else if let Some(prev) = good.take() {
            check_frame(app, &prev, &test_id);
            return;
        }
        assert!(
            Instant::now() < deadline && app.child_running(),
            "{test_id} frame not seen\n{}",
            text(&app.screen())
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn editor_left(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(SQL)
        && visible.contains(MARKER)
        && visible.contains(CONNECTED)
        && !visible.contains("Ctrl+Space")
        && !visible.contains("Enter Accept")
        && !visible.contains(TITLE)
}

fn leave_editor(app: &App) {
    let mut screen = app.wait_text(MARKER);
    // Completion can open over the statement and consume the first Esc.
    // That Esc only dismisses the popup. The next Esc commits and does not
    // revert, so the marker stays and the query remains dirty.
    for _ in 0..2 {
        if editor_left(&screen) {
            break;
        }
        let before = text(&screen);
        app.key("escape");
        screen = app.wait_screen("escape applied", |screen| text(screen) != before);
    }
    let left = if editor_left(&screen) {
        screen
    } else {
        app.wait_screen("editor left", editor_left)
    };
    see(
        &left,
        &[SQL, MARKER, QUERY, CONNECTED],
        &[TITLE, "Enter Accept", "Loading schema"],
    );
}

fn confirm_focused_quit(app: &App) {
    app.key("right");
    let armed = app.wait_screen("quit focused", |screen| {
        quit_copy(screen) && quit_focused(screen)
    });
    see(
        &armed,
        &[TITLE, BODY, QUIT, MARKER],
        &["Save", "Discard", "Filter connections"],
    );
    app.key("enter");
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.child_running() {
        assert!(
            Instant::now() < deadline,
            "tablepro stayed up after confirming quit"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

#[test]
fn tablepro_quit_route_exits_clean() {
    let app = open("t05-quit-route");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Filter connections", "Development"],
        &[QUERY, TITLE],
    );
    connect_development(&app);
    let query = app.screen();
    assert_eq!((query.cols(), query.rows()), (120, 40));
    see(
        &query,
        &[QUERY, CONNECTED, PLACEHOLDER],
        &[TITLE, "Loading schema"],
    );
    app.finish();
}

#[test]
fn tablepro_quit_dirty_cancel_keeps_sql_then_confirms() {
    let app = open("t05-quit-dirty");
    connect_development(&app);
    focus_editor(&app);
    app.type_text(SQL);
    leave_editor(&app);

    app.key("ctrl+q");
    let opened = app.wait_screen("quit dialog", |screen| {
        quit_dialog(screen) && cancel_focused(screen)
    });
    assert_eq!((opened.cols(), opened.rows()), (120, 40));
    assert_cancel_focused(&opened);

    flash_settle(&app);
    shot(&app, WIDE, 120, 40);
    app.resize(80, 24);
    let narrow = app.wait_screen("quit dialog at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && quit_dialog(screen) && cancel_focused(screen)
    });
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));
    assert_cancel_focused(&narrow);
    flash_settle(&app);
    shot(&app, NARROW, 80, 24);
    app.resize(120, 40);

    let restored = app.wait_screen("quit dialog back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && quit_copy(screen) && cancel_focused(screen)
    });
    assert_cancel_focused(&restored);

    app.key("enter");
    let kept = app.wait_screen("cancel keeps the sql", |screen| {
        let visible = text(screen);
        visible.contains(SQL)
            && visible.contains(MARKER)
            && visible.contains(QUERY)
            && !visible.contains(TITLE)
            && !visible.contains(BODY)
            && !visible.contains("Filter connections")
    });
    assert!(app.child_running(), "cancel exited tablepro");
    see(
        &kept,
        &[SQL, MARKER, QUERY, "acme_prod › public"],
        &[
            TITLE,
            BODY,
            "Save",
            "Discard",
            "Filter connections",
            "Loading schema",
        ],
    );

    app.key("ctrl+q");
    let again = app.wait_screen("quit dialog again", |screen| {
        quit_copy(screen) && cancel_focused(screen)
    });
    assert_cancel_focused(&again);
    confirm_focused_quit(&app);
}
