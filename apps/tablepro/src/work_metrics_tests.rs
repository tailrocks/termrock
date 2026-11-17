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
use termrock::core::id::ItemKey;

const VIEWPORT: (u16, u16) = (120, 40);
const SCENARIO: &str = "tablepro_workbench_grid";
const PRICE: usize = 4;

/// The sample connection with each of `tables` opened in turn, so the last one
/// is the active tab. Returns the tab keys in the order given.
fn fixture_with(tables: &[&str]) -> (Harness<App>, Vec<ItemKey>) {
    let mut app = App::new(Theme::for_level(ColorLevel::TrueColor));
    let key = app
        .connections
        .resolve_name("Development")
        .expect("Development is a sample connection");
    app.connect_key(key);
    let workbench = app.workbench.as_mut().expect("connected");
    let keys = tables
        .iter()
        .map(|name| {
            workbench
                .open_table("public", name, false)
                .unwrap_or_else(|| panic!("{name} opens"))
        })
        .collect();
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(30);
    (f, keys)
}

fn fixture() -> Harness<App> {
    fixture_with(&["products"]).0
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

/// The grid of the table tab `key`, wherever it sits in the strip.
fn tab_grid(f: &Harness<App>, key: ItemKey) -> &DataGrid {
    &f.app
        .workbench
        .as_ref()
        .and_then(|w| w.table(key))
        .expect("a table tab")
        .grid
}

fn draft_in_active_tab(f: &mut Harness<App>, price: f64) -> WorkDelta {
    let ((), work) = measure(|| {
        edit_grid(f, |g| g.record_cell(0, PRICE, CellValue::Num(price)))
            .expect("the cell takes the edit");
        f.draw();
    });
    work
}

#[test]
fn work_contract_tablepro_background_tab_edit() {
    // Control: the same active table with no other tab open.
    let (mut alone, _) = fixture_with(&["products"]);
    let control = draft_in_active_tab(&mut alone, 1.5);

    // Tab B (customers) stays in the background while tab A (products) is
    // edited. B is opened first, so A is the active tab.
    let (mut f, keys) = fixture_with(&["customers", "products"]);
    let (background, active) = (keys[0], keys[1]);
    assert_eq!(
        f.app.workbench.as_ref().and_then(|w| w.active_key()),
        Some(active)
    );
    let background_rows = tab_grid(&f, background).rows().len();
    let background_cell = tab_grid(&f, background).value(0, 0).clone();
    let before = f.screen();

    let edit = draft_in_active_tab(&mut f, 1.5);
    assert_ne!(f.screen(), before, "the edit in tab A reaches the frame");
    assert_eq!(tab_grid(&f, active).pending().cells().count(), 1);
    assert_eq!(tab_grid(&f, active).value(0, PRICE), &CellValue::Num(1.5));

    // Tab B is untouched: no draft, the same rows and cells.
    assert_eq!(tab_grid(&f, background).pending().cells().count(), 0);
    assert_eq!(tab_grid(&f, background).rows().len(), background_rows);
    assert_eq!(tab_grid(&f, background).value(0, 0), &background_cell);

    // A background tab paints nothing: the edit's painted rows are the active
    // grid's alone, exactly what the single-tab control paints.
    assert_eq!(
        edit.get(Metric::VisibleRowsPainted),
        control.get(Metric::VisibleRowsPainted),
        "the background tab's rows are not painted"
    );
    assert_exact(
        &edit,
        "one keyed draft edit with a background tab",
        &[
            (Metric::VisibleRowsPainted, 39),
            // The second tab adds its label to the strip: 6 more width calls
            // and 24 more bytes than the single-tab edit (268 / 2,401).
            (Metric::IntrinsicWidthCalls, 274),
            (Metric::TextBytesMeasured, 2_425),
            (Metric::FitVisits, 16),
        ],
    );
    let record_count = tab_grid(&f, active).rows().len();
    f.report(
        SCENARIO,
        "background_tab_keyed_draft_edit",
        record_count,
        edit,
        "tab A shows the draft, tab B has no draft and paints no rows",
    );

    // The settled tick after the edit repaints the same frame.
    f.tick();
    let settled = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        settled,
        "a settled tick leaves the frame unchanged"
    );
    assert_eq!(
        idle.get(Metric::VisibleRowsPainted),
        edit.get(Metric::VisibleRowsPainted),
        "an idle tick paints the active grid only"
    );

    // Switching to tab B shows its own page, still without a draft, and tab A
    // keeps its draft.
    assert!(
        f.app
            .workbench
            .as_mut()
            .expect("connected")
            .activate(background)
    );
    f.draw();
    assert_ne!(f.screen(), settled, "tab B shows its own page");
    assert_eq!(tab_grid(&f, background).pending().cells().count(), 0);
    assert_eq!(tab_grid(&f, active).pending().cells().count(), 1);
}
