//! Count-only work scenarios for the TablePro workbench grid (#56).
//!
//! Each operation runs through the real app: `shell::handle` for the tick or
//! key, then `shell::render` into a 120x40 test terminal. Counts are for the
//! test thread only. Every number below was measured with
//!
//! ```text
//! cargo nextest run -p termrock-tablepro --features work-metrics --bin tablepro \
//!     --no-tests fail -E 'test(/^work_metrics_tests::/)'
//! ```

use ratatui::crossterm::event::KeyCode;
use termrock::metrics::harness::{Harness, assert_exact};
use termrock::metrics::{Metric, WorkDelta, measure};
use termrock::theme::{ColorLevel, Theme};
use termrock::widgets::grid::{CellValue, DataGrid};

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

fn grid(f: &Harness<App>) -> &DataGrid {
    match f.app.workbench.as_ref().and_then(|w| w.active_tab()) {
        Some(WorkTab::Table(t)) => &t.grid,
        _ => panic!("a table tab is active"),
    }
}

fn edit_grid<R>(f: &mut Harness<App>, edit: impl FnOnce(&mut DataGrid) -> R) -> R {
    f.app
        .workbench
        .as_mut()
        .and_then(|w| {
            w.modify_active(|tab| match tab {
                WorkTab::Table(t) => Some(edit(&mut t.grid)),
                _ => None,
            })
        })
        .flatten()
        .expect("a table tab is active")
}

fn report(f: &mut Harness<App>, operation: &'static str, work: WorkDelta, oracle: &str) {
    let record_count = grid(f).rows().len();
    f.report(SCENARIO, operation, record_count, work, oracle);
}

#[test]
fn work_contract_tablepro_idle_and_cell_edit() {
    let mut f = fixture();
    let rows = grid(&f).rows().len();
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
            (Metric::IntrinsicWidthCalls, 233),
            (Metric::TextBytesMeasured, 2_365),
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
            (Metric::IntrinsicWidthCalls, 233),
            (Metric::TextBytesMeasured, 2_365),
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
        edit_grid(&mut f, |g| g.record_cell(0, PRICE, CellValue::Num(1.5)))
            .expect("the cell takes the edit");
        f.draw();
    });
    assert_ne!(f.screen(), before, "the keyed draft edit reaches the frame");
    assert!(
        f.screen().contains("1 pending"),
        "the status bar counts the draft"
    );
    assert_eq!(grid(&f).pending().cells().count(), 1);
    assert_eq!(grid(&f).value(0, PRICE), &CellValue::Num(1.5));
    assert_exact(
        &edit,
        "one keyed draft edit",
        &[
            (Metric::VisibleRowsPainted, 39),
            (Metric::IntrinsicWidthCalls, 268),
            (Metric::TextBytesMeasured, 2_401),
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
            (Metric::IntrinsicWidthCalls, 268),
            (Metric::TextBytesMeasured, 2_401),
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
