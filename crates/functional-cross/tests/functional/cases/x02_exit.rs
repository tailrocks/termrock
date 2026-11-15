//! Cross-application clean and guarded exits, before the refactor.
//!
//! `Shell::quit_through` calls `LeaveGuard::request_quit`. `Stakes::Clean`
//! sets `shell.quit`. Unsaved and running stakes open the dialog in
//! `src/ui/leave.rs`. `runtime::run` then calls `TerminalSession::leave`,
//! which runs `restore_terminal`. `Launch::new` leaves the clock off.
//! `App::new` would enable it for jackin. `session` is private, so a confirm
//! that already exited uses `expect_exited`. `finish` is the mode check, and
//! only while the process is still running.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{
    App, HOLLA, Launch, SHOWCASE, TABLEPRO, find_all, row_text, text, try_launch,
};

const PAGE: &str = "Quit & leave";
const DOCUMENT: &str = "Document";
const CLASSIFY: &str = "QuitKeys::classify";
const TITLE_TEXT: &str = "Q3 settlement report";
const RUN_EXPORT: &str = "Run export";
const STOP_EXPORT: &str = "Stop export";
const RUNNING_FACT: &str = "Running · the export";
const STILL: &str = "Still running";
const STOP_LEAVE: &str = "Stop and leave";
const STAY: &str = "Stay";
const EXPORT_BODY: &str = "the export still running";
const AFTER_CONFIRM: &str = "Export stopped";
const WOULD_QUIT: &str = "Would quit now";

const HOST: &str = "dev-db.internal.acme.io";
const DATABASE: &str = "acme_dev";
const CONNECTED: &str = "Connected to Development";
const QUERY: &str = "Query 1";
const PLACEHOLDER: &str = "Type SQL";
const SQL: &str = "SELECT 'x02_exit' AS marker";
const MARKER: &str = "x02_exit";
const QUIT_TABLE: &str = "Quit TablePro?";
const LOST: &str = "1 unsaved query will be lost.";

const HOLLA_QUERY: &str = "Search actions";
const HOLLA_DISK: &str = "Analyze disk usage";
const QUIT_HOLLA: &str = "Quit holla";
const STOP_QUIT: &str = "Stop and quit";
const HOLLA_RUNNING: &str = "still running";

const WIDE_ID: &str = "pre_refactor/x02_exit/x02_exit_exit_120x40";
const NARROW_ID: &str = "pre_refactor/x02_exit/x02_exit_exit_narrow";

fn open_bin(name: &str, bin: &str, args: &[&str]) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = args.iter().map(|arg| (*arg).to_string()).collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
}

fn open_showcase(name: &str, args: &[&str]) -> App {
    open_bin(name, SHOWCASE, args)
}

fn open_tablepro(name: &str) -> App {
    let app = open_bin(name, TABLEPRO, &[]);
    app.wait_text("Filter connections");
    app
}

fn open_holla(name: &str, scenario: &str) -> App {
    let app = open_bin(name, HOLLA, &["--scenario", scenario]);
    app.wait_text(HOLLA_QUERY);
    app
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

fn see(screen: &TerminalSnapshot, present: &[&str], absent: &[&str]) {
    for item in present {
        require(screen, item);
    }
    for item in absent {
        refuse(screen, item);
    }
}

fn row_has(screen: &TerminalSnapshot, needle: &str) -> bool {
    (0..screen.rows()).any(|row| row_text(screen, row).contains(needle))
}

fn focused(screen: &TerminalSnapshot, label: &str) -> bool {
    row_has(screen, &format!("▎{label}"))
}

fn await_exit(app: &App, reason: &str) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while app.child_running() {
        let visible = text(&app.screen());
        assert!(Instant::now() < deadline, "{reason}\n{visible}");
        std::thread::sleep(Duration::from_millis(25));
    }
    app.expect_exited(0);
}

fn flash_settle(app: &App) {
    std::thread::sleep(Duration::from_millis(200));
    let screen = app.screen();
    let cols = u16::try_from(screen.cols()).expect("cols");
    let rows = u16::try_from(screen.rows()).expect("rows");
    let nudged = cols + 1;
    app.resize(nudged, rows);
    app.wait_screen("nudge", |screen| {
        screen.cols() == u32::from(nudged) && screen.rows() == u32::from(rows)
    });
    app.resize(cols, rows);
    app.wait_screen("nudge restored", |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows)
    });
}

fn guard_open(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    // The page fact sits under the dialog and is clipped at 80×24.
    visible.contains(STILL)
        && visible.contains(EXPORT_BODY)
        && visible.contains(STAY)
        && visible.contains(STOP_LEAVE)
        && !visible.contains(AFTER_CONFIRM)
        && !visible.contains(WOULD_QUIT)
        && !visible.contains("Unsaved changes")
}

fn stored_frame(app: &App, cols: u16, rows: u16) -> TerminalSnapshot {
    flash_settle(app);
    let _ = app.settle();
    let screen = app.stable_screen();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (u32::from(cols), u32::from(rows)),
        "stored frame size\n{}",
        text(&screen)
    );
    assert!(
        guard_open(&screen),
        "guard left before the store\n{}",
        text(&screen)
    );
    assert!(
        focused(&screen, STAY),
        "Stay is not focused\n{}",
        text(&screen)
    );
    assert!(!focused(&screen, STOP_LEAVE), "Stop and leave is focused");
    screen
}

fn sample_bytes(screen: &TerminalSnapshot, test_id: &str) -> Vec<u8> {
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    sample.to_bytes().expect("serialize Tuiscotti sample")
}

fn check_stored(app: &App, screen: &TerminalSnapshot, test_id: &str) {
    let bytes = sample_bytes(screen, test_id);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
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

fn confirm_focused(app: &App, ok: &str, cancel: &str, open: impl Fn(&TerminalSnapshot) -> bool) {
    app.key("right");
    app.wait_screen("confirm focused", |screen| {
        open(screen) && focused(screen, ok) && !focused(screen, cancel)
    });
    app.key("enter");
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
        &["Authentication failed"],
    );
    click_connect(app);
    let work = app.wait_screen("development connected", |screen| {
        let visible = text(screen);
        visible.contains(CONNECTED)
            && visible.contains(QUERY)
            && !visible.contains("Loading schema")
    });
    see(
        &work,
        &[CONNECTED, QUERY, PLACEHOLDER],
        &[QUIT_TABLE, "Loading schema"],
    );
}

fn focus_editor(app: &App) {
    app.wait_text(PLACEHOLDER);
    let screen = app.screen();
    let hits: Vec<_> = find_all(&screen, PLACEHOLDER)
        .into_iter()
        .filter(|hit| hit.x > 20 && row_text(&screen, u32::from(hit.y)).contains(PLACEHOLDER))
        .collect();
    assert_eq!(hits.len(), 1, "editor {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
    app.wait_text("Ctrl+Space");
}

fn editor_left(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(SQL)
        && visible.contains(MARKER)
        && visible.contains(CONNECTED)
        && !visible.contains("Ctrl+Space")
        && !visible.contains(QUIT_TABLE)
}

fn leave_editor(app: &App) {
    let mut screen = app.wait_text(MARKER);
    for _ in 0..2 {
        if editor_left(&screen) {
            break;
        }
        let before = text(&screen);
        app.key("escape");
        screen = app.wait_screen("escape applied", |screen| text(screen) != before);
    }
    if !editor_left(&screen) {
        screen = app.wait_screen("editor left", editor_left);
    }
    see(&screen, &[SQL, MARKER, CONNECTED], &[QUIT_TABLE]);
}

fn table_quit(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(QUIT_TABLE)
        && visible.contains(LOST)
        && visible.contains("Cancel")
        && visible.contains("Quit")
        && visible.contains(MARKER)
        && !visible.contains("Save")
        && !visible.contains("Discard")
}

fn showcase_route() {
    let app = open_showcase("x02-exit-route", &["--page", "quit"]);
    let page = app.wait_text(CLASSIFY);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    see(
        &page,
        &[PAGE, DOCUMENT, CLASSIFY, TITLE_TEXT, RUN_EXPORT, "Clean"],
        &[STILL, "Unsaved changes", STOP_EXPORT, AFTER_CONFIRM],
    );
    assert!(app.child_running(), "the quit page exited before Ctrl+Q");
    app.finish();
}

fn showcase_guard() {
    let app = open_showcase("x02-exit-guard", &["--page", "quit"]);
    app.wait_text(RUN_EXPORT);
    let screen = app.settle();
    let hits: Vec<_> = find_all(&screen, RUN_EXPORT)
        .into_iter()
        .filter(|hit| row_text(&screen, u32::from(hit.y)).contains(RUN_EXPORT))
        .collect();
    assert_eq!(hits.len(), 1, "Run export {hits:?}\n{}", text(&screen));
    app.click(hits[0]);
    let running = app.wait_screen("export running", |screen| {
        let visible = text(screen);
        visible.contains(STOP_EXPORT) && visible.contains(RUNNING_FACT) && !visible.contains(STILL)
    });
    see(
        &running,
        &[STOP_EXPORT, RUNNING_FACT, DOCUMENT],
        &[STILL, AFTER_CONFIRM],
    );
    assert!(app.child_running(), "Run export quit the process");

    app.key("ctrl+q");
    let opened = app.wait_screen("still running", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && guard_open(screen) && focused(screen, STAY)
    });
    assert!(
        guard_open(&opened),
        "dialog did not open\n{}",
        text(&opened)
    );
    require(&opened, RUNNING_FACT);
    let wide = stored_frame(&app, 120, 40);
    require(&wide, RUNNING_FACT);

    app.resize(80, 24);
    let narrowed = app.wait_screen("guard at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && guard_open(screen)
    });
    assert!(guard_open(&narrowed), "narrow guard lost the title");
    let narrow = stored_frame(&app, 80, 24);

    app.resize(120, 40);
    let restored = app.wait_screen("guard back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && guard_open(screen) && focused(screen, STAY)
    });
    refuse(&restored, AFTER_CONFIRM);
    assert!(app.child_running(), "the resize quit showcase");

    confirm_focused(&app, STOP_LEAVE, STAY, guard_open);
    await_exit(&app, "confirmed Stop and leave left showcase running");
    check_stored(&app, &wide, WIDE_ID);
    check_stored(&app, &narrow, NARROW_ID);
}

fn tablepro_clean() {
    let app = open_tablepro("x02-exit-tablepro-clean");
    let landing = app.screen();
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &["Filter connections", "Development"],
        &[QUERY, QUIT_TABLE],
    );
    assert!(app.child_running(), "tablepro stopped on Connections");
    app.finish();
}

fn tablepro_guard() {
    let app = open_tablepro("x02-exit-tablepro-dirty");
    connect_development(&app);
    focus_editor(&app);
    app.type_text(SQL);
    leave_editor(&app);
    app.key("ctrl+q");
    let opened = app.wait_screen("quit tablepro", |screen| {
        table_quit(screen) && focused(screen, "Cancel")
    });
    see(&opened, &[QUIT_TABLE, LOST, MARKER], &["Save", "Discard"]);
    app.key("enter");
    let kept = app.wait_screen("cancel keeps the query", |screen| {
        let visible = text(screen);
        visible.contains(SQL) && visible.contains(CONNECTED) && !visible.contains(QUIT_TABLE)
    });
    assert!(app.child_running(), "Cancel exited tablepro");
    see(&kept, &[SQL, MARKER, CONNECTED], &[QUIT_TABLE, LOST]);

    app.key("ctrl+q");
    app.wait_screen("quit tablepro again", |screen| {
        table_quit(screen) && focused(screen, "Cancel")
    });
    confirm_focused(&app, "Quit", "Cancel", table_quit);
    await_exit(&app, "confirmed Quit left tablepro running");
}

fn holla_clean() {
    let app = open_holla("x02-exit-holla-clean", "first-use");
    let landing = app.wait_text(HOLLA_DISK);
    assert_eq!((landing.cols(), landing.rows()), (120, 40));
    see(
        &landing,
        &[HOLLA_DISK, HOLLA_QUERY],
        &[QUIT_HOLLA, STOP_QUIT],
    );
    assert!(app.child_running(), "first-use stopped holla");
    app.finish();
}

fn holla_quit(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(QUIT_HOLLA)
        && visible.contains(STAY)
        && visible.contains(STOP_QUIT)
        && visible.contains(HOLLA_RUNNING)
}

fn holla_guard() {
    let app = open_holla("x02-exit-holla-running", "activities-multi");
    let landing = app.screen();
    see(&landing, &[HOLLA_QUERY], &[QUIT_HOLLA, STOP_QUIT]);
    app.key("ctrl+q");
    let dialog = app.wait_screen("quit holla", |screen| {
        holla_quit(screen) && focused(screen, STAY)
    });
    see(
        &dialog,
        &[QUIT_HOLLA, STAY, STOP_QUIT, "detached monitors keep"],
        &["quitting when every process is gone"],
    );
    assert!(app.child_running(), "opening the guard quit holla");
    confirm_focused(&app, STOP_QUIT, STAY, holla_quit);
    let stopping = app.wait_screen("stopping", |screen| {
        text(screen).contains("quitting when every process is gone")
            && !text(screen).contains(STOP_QUIT)
    });
    assert!(
        app.child_running(),
        "Stop and quit exited before the activities ended"
    );
    refuse(&stopping, STOP_QUIT);
    // A second Ctrl+Q is ignored once `quitting` is set. `finish` waits for
    // the activities to end and checks the restored modes.
    app.finish();
}

#[test]
fn x02_exit_route() {
    showcase_route();
}

#[test]
fn x02_exit_exit() {
    showcase_guard();
    tablepro_clean();
    tablepro_guard();
    holla_clean();
    holla_guard();
}
