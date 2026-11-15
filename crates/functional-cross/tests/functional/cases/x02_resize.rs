//! Cross-application resize and the too-small screen, before the refactor.
//!
//! Tuiscotti test ids reject `.` and uppercase, so the baseline directory is
//! `pre_refactor/x02_resize/`. The checkpoint session is showcase
//! `--page pickers` with the quick picker open and the search draft `schema`.
//! It shrinks through the too-small sizes, restores that draft, captures
//! 80×24, then captures the restored 120×40 frame. `Launch::new` leaves the
//! clock off, including for jackin.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use tui_test::TerminalSnapshot;
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, Point, SHOWCASE, TABLEPRO, find_all, text, try_launch};

const TOO_SMALL: &str = "Terminal too small";
const MIN_WIDTH: u16 = 72;
const MIN_HEIGHT: u16 = 20;
const PICKER_DRAFT: &str = "schema.rs";
const PICKER_DETAIL: &str = "src/db/schema.rs";
const PICKER_OPEN: &str = "All · Tab scope";
const PICKER_HIDDEN: &str = "Cargo.toml";
const FORM_DRAFT: &str = "FormDraftX02";
const DELETE_TITLE: &str = "Delete branch?";
const DELETE_BODY: &str = "cannot be undone";
const PIPELINE: &str = "Pipeline started";
const WIDE_ID: &str = "pre_refactor/x02_resize/x02_resize_resize_120x40";
const NARROW_ID: &str = "pre_refactor/x02_resize/x02_resize_resize_narrow";

/// `src/shell/reach.rs` sizes other than the 71×20 and 72×19 boundaries.
/// 72×20 is the shared minimum and is asserted on its own.
const REACH: [(u16, u16); 7] = [
    (40, 12),
    (60, 18),
    (80, 24),
    (100, 25),
    (106, 29),
    (120, 40),
    (160, 50),
];

fn open(name: &str, bin: &str, args: &[&str]) -> App {
    let mut launch = Launch::new(name, bin);
    launch.clock = false;
    launch.cols = 120;
    launch.rows = 40;
    launch.color = Some("truecolor".to_string());
    launch.motion = Some("reduced".to_string());
    launch.args = args.iter().copied().map(str::to_string).collect();
    try_launch(launch).unwrap_or_else(|error| panic!("{error}"))
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

fn need(cols: u16, rows: u16) -> String {
    format!("Need {MIN_WIDTH}×{MIN_HEIGHT}, have {cols}×{rows}")
}

fn click_labeled(app: &App, label: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.wait_text(label);
    let hits: Vec<_> = find_all(&screen, label)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "expected one {label:?}, got {hits:?}\n{}",
        text(&screen)
    );
    app.click(hits[0]);
}

fn at_size(
    app: &App,
    cols: u16,
    rows: u16,
    pred: impl Fn(&TerminalSnapshot) -> bool,
) -> TerminalSnapshot {
    app.resize(cols, rows);
    thread::sleep(Duration::from_millis(250));
    app.wait_screen(&format!("{cols}x{rows}"), |screen| {
        screen.cols() == u32::from(cols) && screen.rows() == u32::from(rows) && pred(screen)
    })
}

fn assert_too_small(app: &App, cols: u16, rows: u16, absent: &str) -> TerminalSnapshot {
    let expected = need(cols, rows);
    let screen = at_size(app, cols, rows, |screen| {
        let visible = text(screen);
        visible.contains(TOO_SMALL) && visible.contains(&expected) && !visible.contains(absent)
    });
    require(&screen, TOO_SMALL);
    require(&screen, &expected);
    refuse(&screen, absent);
    assert!(app.child_running(), "process stopped at {cols}x{rows}");
    screen
}

fn assert_ui(app: &App, cols: u16, rows: u16, marker: &str) -> TerminalSnapshot {
    let screen = at_size(app, cols, rows, |screen| {
        let visible = text(screen);
        visible.contains(marker) && !visible.contains(TOO_SMALL)
    });
    require(&screen, marker);
    refuse(&screen, TOO_SMALL);
    assert_eq!(
        (screen.cols(), screen.rows()),
        (u32::from(cols), u32::from(rows))
    );
    assert!(app.child_running(), "process stopped at {cols}x{rows}");
    screen
}

fn check_frame(app: &App, screen: &TerminalSnapshot, test_id: &str) -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/functional/baselines");
    let sample = tuiscotti_latest::sample::Sample::state(
        test_id,
        screen.clone(),
        tuiscotti_latest::sample::Observation::empty(),
    )
    .expect("build Tuiscotti state sample");
    let bytes = sample.to_bytes().expect("serialize Tuiscotti sample");
    let artifacts = app.evidence_dir().join("check-artifacts");
    let store =
        tuiscotti_latest::check::FrozenStore::new(&root, &artifacts).expect("open frozen store");
    let outcome = store.check(&bytes).expect("run Tuiscotti frozen check");
    if let CheckOutcome::Failure(failure) = outcome {
        return Err(format!(
            "{test_id} {} sha {} path {} cells {}",
            failure.reason(),
            failure.candidate_sha256(),
            failure.candidate_path().display(),
            failure.changed_cell_total()
        ));
    }
    Ok(())
}

fn open_quick_picker(app: &App) {
    app.wait_text("Open quickly");
    click_labeled(app, "Open quickly", |hit| hit.y > 4 && hit.x >= 24);
    app.wait_text("Files and tasks");
    app.type_text("schema");
    let filtered = app.wait_screen("schema draft", |screen| {
        let visible = text(screen);
        visible.contains(PICKER_DRAFT)
            && visible.contains(PICKER_DETAIL)
            && visible.contains(PICKER_OPEN)
            && !visible.contains(PICKER_HIDDEN)
            && !visible.contains(TOO_SMALL)
    });
    require(&filtered, PICKER_DRAFT);
    require(&filtered, PICKER_OPEN);
    refuse(&filtered, PICKER_HIDDEN);
    assert!(app.child_running(), "opening the picker stopped showcase");
}

fn picker_checkpoint() {
    let app = open("x02-resize-checkpoint", SHOWCASE, &["--page", "pickers"]);
    open_quick_picker(&app);
    assert_too_small(&app, 71, 20, PICKER_DRAFT);
    assert_too_small(&app, 72, 19, PICKER_DRAFT);
    let restored = assert_ui(&app, MIN_WIDTH, MIN_HEIGHT, PICKER_DRAFT);
    require(&restored, PICKER_DETAIL);
    require(&restored, PICKER_OPEN);
    refuse(&restored, PICKER_HIDDEN);
    refuse(&restored, TOO_SMALL);

    let narrow = assert_ui(&app, 80, 24, PICKER_DRAFT);
    require(&narrow, PICKER_DETAIL);
    require(&narrow, PICKER_OPEN);
    refuse(&narrow, PICKER_HIDDEN);
    refuse(&narrow, TOO_SMALL);
    assert_eq!((narrow.cols(), narrow.rows()), (80, 24));

    let wide = assert_ui(&app, 120, 40, PICKER_DRAFT);
    require(&wide, PICKER_DETAIL);
    require(&wide, PICKER_OPEN);
    refuse(&wide, PICKER_HIDDEN);
    refuse(&wide, TOO_SMALL);
    assert_eq!((wide.cols(), wide.rows()), (120, 40));
    // Picker::on_other_key consumes Ctrl+Q while the layer is open. Esc clears
    // the query, and a second Esc cancels. The shots above already include the draft.
    app.key("escape");
    app.wait_text("Files and tasks");
    app.key("escape");
    app.wait_absent(PICKER_OPEN);
    app.finish();

    let narrow_check = check_frame(&app, &narrow, NARROW_ID);
    let wide_check = check_frame(&app, &wide, WIDE_ID);
    let mut errors = Vec::new();
    if let Err(error) = narrow_check {
        errors.push(error);
    }
    if let Err(error) = wide_check {
        errors.push(error);
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

fn form_draft_survives() {
    let app = open("x02-resize-form", SHOWCASE, &["--page", "forms"]);
    app.wait_text("Task name");
    click_labeled(&app, "Create task", |hit| hit.x > 24);
    app.wait_text("Fix the highlighted fields");
    app.type_text(FORM_DRAFT);
    app.key("enter");
    let named = app.wait_screen("form draft committed", |screen| {
        text(screen).contains(FORM_DRAFT) && !text(screen).contains("Required")
    });
    require(&named, "Task name");
    refuse(&named, TOO_SMALL);

    assert_too_small(&app, 71, 20, FORM_DRAFT);
    assert_too_small(&app, 72, 19, FORM_DRAFT);
    let back = assert_ui(&app, MIN_WIDTH, MIN_HEIGHT, FORM_DRAFT);
    require(&back, "Task name");
    refuse(&back, TOO_SMALL);

    for (cols, rows) in REACH {
        if cols < MIN_WIDTH || rows < MIN_HEIGHT {
            assert_too_small(&app, cols, rows, FORM_DRAFT);
        } else {
            let screen = assert_ui(&app, cols, rows, FORM_DRAFT);
            require(&screen, "Task name");
        }
    }
    assert!(app.child_running(), "form walk stopped showcase");
    app.kill_child();
}

fn delete_dialog_survives() {
    let app = open("x02-resize-delete", SHOWCASE, &["--page", "dialogs"]);
    app.wait_text("Confirm run");
    click_labeled(&app, "Delete branch", |hit| hit.x >= 24 && hit.y >= 6);
    let open = app.wait_text(DELETE_TITLE);
    require(&open, DELETE_BODY);
    require(&open, "feat/rate-limit");
    assert_too_small(&app, 71, 20, DELETE_TITLE);
    assert_too_small(&app, 72, 19, DELETE_TITLE);
    let back = assert_ui(&app, MIN_WIDTH, MIN_HEIGHT, DELETE_TITLE);
    require(&back, DELETE_BODY);
    require(&back, "feat/rate-limit");
    refuse(&back, TOO_SMALL);
    assert!(app.child_running(), "delete dialog stopped showcase");
    app.kill_child();
}

fn pipeline_output_survives() {
    let app = open("x02-resize-pipeline", SHOWCASE, &["--page", "taskrunner"]);
    app.wait_text("Run pipeline");
    click_labeled(&app, "Run pipeline", |hit| hit.y > 4 && hit.x >= 24);
    let started = app.wait_text(PIPELINE);
    require(&started, "Pipeline running");
    refuse(&started, TOO_SMALL);
    assert_too_small(&app, 71, 20, PIPELINE);
    assert_too_small(&app, 72, 19, PIPELINE);
    // At 72×20 the log slot is below the pipe (`taskrunner.rs` layout), so the
    // running title is the visible proof. 120×40 brings the log back.
    let back = assert_ui(&app, MIN_WIDTH, MIN_HEIGHT, "Pipeline · run");
    refuse(&back, "Ready. Press r");
    let wide = assert_ui(&app, 120, 40, "compile");
    assert!(
        pipeline_visible(&text(&wide)),
        "running output did not survive\n{}",
        text(&wide)
    );
    refuse(&wide, "Ready. Press r");
    assert!(app.child_running(), "pipeline stopped showcase");
    app.kill_child();
}

fn pipeline_visible(visible: &str) -> bool {
    visible.contains(PIPELINE)
        || visible.contains("Pipeline finished")
        || visible.contains("failed")
        || visible.contains('▶')
        || visible.contains("started")
        || visible.contains("step ")
}

fn app_bounds(name: &str, bin: &str, args: &[&str], marker: &str) {
    let app = open(name, bin, args);
    let start = app.wait_text(marker);
    require(&start, marker);
    refuse(&start, TOO_SMALL);
    assert_too_small(&app, 71, 20, marker);
    let back = assert_ui(&app, MIN_WIDTH, MIN_HEIGHT, marker);
    require(&back, marker);
    refuse(&back, TOO_SMALL);
    assert!(app.child_running(), "{name} stopped");
    app.kill_child();
}

#[test]
fn x02_resize_route() {
    let app = open("x02-resize-route", SHOWCASE, &["--page", "pickers"]);
    let screen = app.wait_text("Open quickly");
    require(&screen, "Overlays › Pickers");
    require(&screen, "Open quickly");
    refuse(&screen, TOO_SMALL);
    assert_eq!((screen.cols(), screen.rows()), (120, 40));
    assert!(app.child_running(), "route startup stopped showcase");
    app.finish();
}

#[test]
fn x02_resize_resize() {
    app_bounds("x02-resize-tablepro", TABLEPRO, &[], "Filter connections");
    app_bounds(
        "x02-resize-holla",
        HOLLA,
        &["--scenario", "first-use"],
        "Analyze disk usage",
    );
    delete_dialog_survives();
    pipeline_output_survives();
    form_draft_survives();
    picker_checkpoint();
}
