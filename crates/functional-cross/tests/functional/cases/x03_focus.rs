//! Cross-application focus treatment, before the refactor.
//!
//! `Theme::gutter_symbol` paints `glyphs.focus_bar` (`▎`) in the phosphor
//! focus colour while `VisualState.focused` is set and the control is not
//! disabled (`src/theme.rs`). `Interaction::focused` is false while
//! `focus_hidden` is set (`src/ui/ctx.rs`). Showcase `page_under_layer`
//! includes `modal_open`, and the shell passes that into
//! `RenderCtx::page_under_layer`. `modal_backdrop_with` then replaces a focus
//! bar under the dim with a space. `Theme::inactive_look` turns an accent
//! selection neutral when `VisualState.backdrop` is set, and `Theme::backdrop`
//! maps that fill onto `surface_overlay`.
//!
//! An owner-owned list cursor is `CursorStyle::Fill` (the default) through
//! `RowLook::Fill`. `Kind::CursorRow(Fill)` uses `Plane::Highlight` and a
//! space in the gutter slot (`src/theme.rs`, `src/ui/row.rs`,
//! `src/widgets/option_list.rs`). The picker field paints its own gutter with
//! `gutter_symbol` forced focused (`src/widgets/picker.rs`).
//! `src/ui/specimen.rs` `depict_focus` only restyles a bar a specimen already
//! painted. `--page focus` is `PageId::FocusNavigation` through `PAGE_ALIASES`
//! and `parse_page`.

use std::path::PathBuf;

use tui_test::{SnapshotColor, TerminalSnapshot};
use tuiscotti_latest::check::CheckOutcome;

use crate::support::{App, HOLLA, Launch, Point, SHOWCASE, TABLEPRO, find_all, text, try_launch};

const BAR: &str = "▎";
const FOCUS_TITLE: &str = "Focus & navigation";
/// Header width ellipsizes `FocusNavPage::blurb` after this prefix.
const FOCUS_BLURB: &str = "The focus ring, fallback, nav keys";
const PLAYGROUND: &str = "Playground";
const DELETE_TITLE: &str = "Delete branch?";
const DELETE_BODY: &str = "cannot be undone";
const DELETE_BRANCH: &str = "feat/rate-limit";
const PICKER_FIELD: &str = "Files and tasks";
const PICKER_SCOPE: &str = "All · Tab scope";
const PICKER_ROW: &str = "Cargo.toml";
const WIDE_ID: &str = "pre_refactor/x03_focus/x03_focus_focus_120x40";
const NARROW_ID: &str = "pre_refactor/x03_focus/x03_focus_focus_narrow";

fn phosphor() -> SnapshotColor {
    rgb(0x48, 0xe0, 0x54)
}

fn accent_bg() -> SnapshotColor {
    rgb(0x17, 0x3f, 0x1c)
}

fn muted() -> SnapshotColor {
    rgb(0x8a, 0x8a, 0x8a)
}

fn overlay() -> SnapshotColor {
    rgb(0x27, 0x27, 0x2a)
}

fn highlight() -> SnapshotColor {
    rgb(0x2f, 0x5a, 0xa8)
}

fn field_bg() -> SnapshotColor {
    rgb(0x1e, 0x1e, 0x22)
}

fn rgb(r: u8, g: u8, b: u8) -> SnapshotColor {
    SnapshotColor::Rgb(r, g, b)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bar {
    x: u16,
    y: u16,
    fg: SnapshotColor,
    bg: SnapshotColor,
}

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

fn labeled(screen: &TerminalSnapshot, needle: &str, pred: impl Fn(&Point) -> bool) -> Point {
    let hits: Vec<_> = find_all(screen, needle)
        .into_iter()
        .filter(|hit| pred(hit))
        .collect();
    assert_eq!(hits.len(), 1, "{needle} hits {hits:?}\n{}", text(screen));
    hits[0]
}

fn click_labeled(app: &App, label: &str, pred: impl Fn(&Point) -> bool) {
    let screen = app.wait_text(label);
    app.click(labeled(&screen, label, pred));
}

fn glyph(screen: &TerminalSnapshot, x: u16, y: u16) -> String {
    screen
        .cell_at(u32::from(x), u32::from(y))
        .expect("cell")
        .text()
        .to_string()
}

fn paint(screen: &TerminalSnapshot, point: Point) -> (SnapshotColor, SnapshotColor) {
    let cell = screen
        .cell_at(u32::from(point.x), u32::from(point.y))
        .expect("cell");
    (cell.fg(), cell.bg())
}

fn bars(screen: &TerminalSnapshot) -> Vec<Bar> {
    let mut found = Vec::new();
    for y in 0..screen.rows() {
        for x in 0..screen.cols() {
            let cell = screen.cell_at(x, y).expect("cell");
            if cell.text() == BAR {
                found.push(Bar {
                    x: u16::try_from(x).expect("column"),
                    y: u16::try_from(y).expect("row"),
                    fg: cell.fg(),
                    bg: cell.bg(),
                });
            }
        }
    }
    found
}

fn one_bar(screen: &TerminalSnapshot) -> Bar {
    let found = bars(screen);
    assert_eq!(
        found.len(),
        1,
        "expected one {BAR}, found {found:?}\n{}",
        text(screen)
    );
    let bar = found[0];
    assert_eq!(
        bar.fg,
        phosphor(),
        "{BAR} at ({}, {}) is not the phosphor focus mark",
        bar.x,
        bar.y
    );
    bar
}

fn sidebar(screen: &TerminalSnapshot, label: &str) -> Point {
    labeled(screen, label, |hit| hit.x < 20 && hit.y > 2)
}

fn dialog_ready(screen: &TerminalSnapshot) -> bool {
    let visible = text(screen);
    visible.contains(DELETE_TITLE)
        && visible.contains(DELETE_BODY)
        && visible.contains(DELETE_BRANCH)
        && bars(screen).len() == 1
}

fn assert_dialog(screen: &TerminalSnapshot) {
    require(screen, DELETE_TITLE);
    require(screen, DELETE_BODY);
    require(screen, DELETE_BRANCH);
    let bar = one_bar(screen);
    let cancel = labeled(screen, "Cancel", |hit| hit.y == bar.y);
    assert_eq!(
        cancel.x,
        bar.x + 1,
        "the dialog gutter is on Cancel, bar {bar:?} label {cancel:?}"
    );
    assert!(
        bar.x >= 24,
        "the dialog gutter is still in the sidebar: {bar:?}"
    );
    assert_eq!(bar.bg, overlay(), "dialog gutter background");
    let row = sidebar(screen, "Dialogs");
    assert_eq!(
        paint(screen, row),
        (muted(), overlay()),
        "covered Dialogs row kept an accent fill at {row:?}"
    );
    assert_ne!(bar.y, row.y, "the page gutter stayed on the sidebar row");
    assert_ne!(
        glyph(screen, 1, row.y),
        BAR,
        "sidebar row still paints a focus bar"
    );
    if screen.cols() == 80 {
        assert_eq!(
            glyph(screen, row.x + 7, row.y),
            "│",
            "80×24 should clip the sidebar row after Dialogs"
        );
    }
}

fn hold_dialog(app: &App, cols: u16, rows: u16) -> TerminalSnapshot {
    let _ = app.settle();
    let screen = app.stable_screen();
    assert_eq!(
        (screen.cols(), screen.rows()),
        (u32::from(cols), u32::from(rows)),
        "stored frame size\n{}",
        text(&screen)
    );
    assert_dialog(&screen);
    screen
}

/// Stores 120×40, then 80×24, while the delete dialog is still open.
/// The narrow frame is taken before the return to 120×40.
fn dialog(app: &App) -> (TerminalSnapshot, TerminalSnapshot) {
    let closed = app.wait_text("Confirm run");
    let row = sidebar(&closed, "Dialogs");
    assert_eq!(paint(&closed, row), (phosphor(), accent_bg()));
    let before = one_bar(&closed);
    assert_eq!(before.y, row.y);
    assert!(before.x < row.x, "closed gutter {before:?} row {row:?}");

    click_labeled(app, "Delete branch", |hit| hit.x >= 24 && hit.y >= 6);
    let _opened = app.wait_screen("delete dialog at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_ready(screen)
    });
    let wide = hold_dialog(app, 120, 40);

    app.resize(80, 24);
    let _narrowed = app.wait_screen("delete dialog at 80x24", |screen| {
        screen.cols() == 80 && screen.rows() == 24 && dialog_ready(screen)
    });
    let narrow = hold_dialog(app, 80, 24);

    app.resize(120, 40);
    let restored = app.wait_screen("delete dialog back at 120x40", |screen| {
        screen.cols() == 120 && screen.rows() == 40 && dialog_ready(screen)
    });
    assert_dialog(&restored);
    app.key("escape");
    let back = app.wait_absent(DELETE_TITLE);
    require(&back, "Confirm run");
    assert!(app.child_running(), "dismiss stopped showcase");
    app.finish();
    (wide, narrow)
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

fn moved_focus() {
    let app = open("x03-focus-move", SHOWCASE, &["--page", "focus"]);
    let opened = app.wait_text(FOCUS_BLURB);
    require(&opened, FOCUS_TITLE);
    let before = one_bar(&opened);
    app.key("tab");
    let moved = app.wait_screen("gutter left the first control", |screen| {
        let found = bars(screen);
        found.len() == 1 && (found[0].x, found[0].y) != (before.x, before.y)
    });
    let after = one_bar(&moved);
    assert_ne!(glyph(&moved, before.x, before.y), BAR);
    let tab = labeled(&moved, PLAYGROUND, |hit| hit.y == after.y);
    assert_eq!(
        tab.x,
        after.x + 1,
        "gutter {after:?} is not on the Playground tab {tab:?}"
    );
    assert!(app.child_running(), "tab stopped showcase");
    app.kill_child();
    assert!(!app.child_running(), "focus-move child still running");
}

fn owner_cursor() {
    let app = open("x03-focus-picker", SHOWCASE, &["--page", "pickers"]);
    app.wait_text("Open quickly");
    click_labeled(&app, "Open quickly", |hit| hit.y > 4 && hit.x >= 24);
    let screen = app.wait_screen("quick picker cursor", |screen| {
        let visible = text(screen);
        visible.contains(PICKER_FIELD)
            && visible.contains(PICKER_SCOPE)
            && visible.contains(PICKER_ROW)
            && bars(screen).len() == 1
    });
    let bar = one_bar(&screen);
    let field = labeled(&screen, PICKER_FIELD, |hit| hit.y == bar.y && hit.x > bar.x);
    assert_eq!(field.y, bar.y);
    assert!(
        field.x > bar.x,
        "placeholder {field:?} does not follow the owner gutter {bar:?}"
    );
    assert_eq!(bar.bg, field_bg(), "owner gutter background");
    let cursor: Vec<_> = find_all(&screen, PICKER_ROW)
        .into_iter()
        .filter(|hit| paint(&screen, *hit).1 == highlight())
        .collect();
    assert!(
        !cursor.is_empty(),
        "no highlight fill on {PICKER_ROW}\n{}",
        text(&screen)
    );
    let row = cursor[0].y;
    assert!(cursor.iter().all(|hit| hit.y == row));
    assert_ne!(row, bar.y, "the list cursor grew the field gutter");
    assert_eq!(glyph(&screen, bar.x, row), " ");
    assert_eq!(
        paint(&screen, Point { x: bar.x, y: row }).1,
        highlight(),
        "owner cursor slot is not the fill"
    );
    assert!(
        bars(&screen).iter().all(|item| item.y != row),
        "the cursor row painted a second gutter"
    );
    assert!(app.child_running(), "the picker stopped showcase");
    app.kill_child();
    assert!(!app.child_running(), "picker child still running");
}

fn one_shell(
    name: &str,
    bin: &str,
    args: &[&str],
    needle: &str,
    pred: impl Fn(&Point, Bar) -> bool,
) {
    let app = open(name, bin, args);
    let screen = app.wait_text(needle);
    let bar = one_bar(&screen);
    let hit = labeled(&screen, needle, |point| pred(point, bar));
    assert_eq!(hit.y, bar.y, "{needle} is not on the focused row");
    assert!(hit.x > bar.x, "{needle} does not follow the gutter");
    assert!(app.child_running(), "{name} exited before the assertion");
    app.kill_child();
    assert!(!app.child_running(), "{name} child still running");
}

fn other_shells() {
    let app = open("x03-focus-tablepro", TABLEPRO, &[]);
    let screen = app.wait_text("Filter connections");
    require(&screen, "Personal");
    let bar = one_bar(&screen);
    let row = labeled(&screen, "Personal", |hit| hit.y == bar.y);
    assert!(row.x > bar.x);
    let filter = labeled(&screen, "Filter connections", |_| true);
    assert_ne!(filter.y, bar.y, "the filter field owns the gutter");
    assert!(app.child_running(), "tablepro exited before the assertion");
    app.kill_child();
    assert!(!app.child_running(), "tablepro child still running");

    one_shell(
        "x03-focus-holla",
        HOLLA,
        &["--scenario", "activities-multi"],
        "Search actions",
        |hit, bar| hit.y == bar.y && hit.x > bar.x,
    );
}

fn route() {
    let app = open("x03-focus-route", SHOWCASE, &["--page", "focus"]);
    let page = app.wait_text(FOCUS_BLURB);
    assert_eq!((page.cols(), page.rows()), (120, 40));
    require(&page, FOCUS_TITLE);
    one_bar(&page);
    assert!(app.child_running(), "the focus page exited before Ctrl+Q");
    app.finish();
}

fn checkpoint() {
    let app = open("x03-focus-checkpoint", SHOWCASE, &["--page", "dialogs"]);
    let (wide, narrow) = dialog(&app);
    check_stored(&app, &wide, WIDE_ID);
    check_stored(&app, &narrow, NARROW_ID);
}

#[test]
fn x03_focus_route() {
    route();
}

#[test]
fn x03_focus_focus() {
    moved_focus();
    checkpoint();
    owner_cursor();
    other_shells();
}
