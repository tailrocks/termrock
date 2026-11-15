//! Count-only work scenarios for the TablePro workbench grid (#56).
//!
//! Each operation runs through the real app: `shell::handle` for the tick or
//! key, then `shell::render` into a 120x40 test terminal. Counts are for the
//! test thread only. Every number below was measured with
//!
//! ```text
//! cargo nextest run -p junie-tui --features work-metrics --bin tablepro \
//!     --no-tests fail -E 'test(/^work_metrics_tests::/)'
//! ```

use junie_tui::metrics::harness::{Harness, assert_exact};
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::grid::{CellValue, DataGrid};
use ratatui::crossterm::event::KeyCode;

use crate::app::App;
use crate::workbench::WorkTab;

const VIEWPORT: (u16, u16) = (120, 40);
const SCENARIO: &str = "tablepro_workbench_grid";
const PRICE: usize = 4;

fn fixture() -> Harness<App> {
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    app.workbench
        .as_mut()
        .expect("connected")
        .open_table("public", "products", false)
        .expect("products opens");
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(30);
    f
}

fn grid(f: &mut Harness<App>) -> &mut DataGrid {
    match f.app.workbench.as_mut().and_then(|w| w.active_tab_mut()) {
        Some(WorkTab::Table(t)) => &mut t.grid,
        _ => panic!("a table tab is active"),
    }
}

fn report(f: &mut Harness<App>, operation: &'static str, work: WorkDelta, oracle: &str) {
    let record_count = grid(f).rows().len();
    f.report(SCENARIO, operation, record_count, work, oracle);
}

#[test]
fn work_contract_tablepro_idle_and_cell_edit() {
    let mut f = fixture();
    let rows = grid(&mut f).rows().len();
    assert_eq!(rows, 312, "the seeded products table");

    // Rows painted is summed over every list, table, grid and tree widget in
    // the frame, so it exceeds the grid's own visible rows. The grid holds 312
    // records and the frame paints a window of them; no other counted work
    // happens on a settled tick.
    let before = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    assert_exact(
        &idle,
        "settled tick",
        &[
            (Metric::VisibleRowsPainted, 41),
            (Metric::IntrinsicWidthCalls, 227),
            (Metric::TextBytesMeasured, 2_355),
            (Metric::FitVisits, 15),
        ],
    );
    report(
        &mut f,
        "settled_tick",
        idle,
        "the frame text is unchanged by the tick",
    );

    let ((), nav) = measure(|| f.press(KeyCode::Down));
    assert_ne!(f.screen(), before, "Down moves the active cell on screen");
    assert_exact(
        &nav,
        "active-grid navigation",
        &[
            (Metric::VisibleRowsPainted, 41),
            (Metric::IntrinsicWidthCalls, 227),
            (Metric::TextBytesMeasured, 2_355),
            (Metric::FitVisits, 15),
        ],
    );
    report(
        &mut f,
        "grid_navigation",
        nav,
        "one Down press changes the frame",
    );

    let before = f.screen();
    let ((), edit) = measure(|| {
        assert!(grid(&mut f).record_cell(0, PRICE, CellValue::Num(1.5)));
        f.draw();
    });
    assert_ne!(f.screen(), before, "the keyed draft edit reaches the frame");
    assert!(
        f.screen().contains("1 pending"),
        "the status bar counts the draft"
    );
    assert_eq!(grid(&mut f).pending().cells().count(), 1);
    assert_eq!(grid(&mut f).value(0, PRICE), &CellValue::Num(1.5));
    assert_exact(
        &edit,
        "one keyed draft edit",
        &[
            (Metric::VisibleRowsPainted, 39),
            (Metric::IntrinsicWidthCalls, 234),
            (Metric::TextBytesMeasured, 2_363),
            (Metric::FitVisits, 16),
        ],
    );
    report(
        &mut f,
        "keyed_draft_edit",
        edit,
        "the status bar shows 1 pending and the grid holds the drafted value",
    );

    f.tick();
    let before = f.screen();
    let ((), unrelated) = measure(|| {
        let entry = f.app.history.entries[0].clone();
        f.app.history.entries.push(entry);
        f.tick();
    });
    assert_eq!(
        f.screen(),
        before,
        "a history entry is not on the grid page"
    );
    assert_exact(
        &unrelated,
        "unrelated history entry",
        &[
            (Metric::VisibleRowsPainted, 39),
            (Metric::IntrinsicWidthCalls, 234),
            (Metric::TextBytesMeasured, 2_363),
            (Metric::FitVisits, 16),
        ],
    );
    report(
        &mut f,
        "irrelevant_source_change_then_tick",
        unrelated,
        "the frame text is unchanged by an unrelated history entry",
    );
}
