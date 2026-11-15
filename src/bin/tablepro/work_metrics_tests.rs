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

use junie_tui::core::event::{Input, Key};
use junie_tui::metrics::report::ScenarioReport;
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::theme::{ColorLevel, Theme};
use junie_tui::widgets::grid::{CellValue, DataGrid};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::app::App;
use crate::workbench::WorkTab;

const VIEWPORT: (u16, u16) = (120, 40);
const PRICE: usize = 4;

struct Fixture {
    app: App,
    term: Terminal<TestBackend>,
}

impl Fixture {
    fn new() -> Self {
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
        let term = Terminal::new(TestBackend::new(VIEWPORT.0, VIEWPORT.1)).expect("test terminal");
        let mut f = Self { app, term };
        for _ in 0..30 {
            f.tick();
        }
        f
    }

    fn grid(&mut self) -> &mut DataGrid {
        match self.app.workbench.as_mut().and_then(|w| w.active_tab_mut()) {
            Some(WorkTab::Table(t)) => &mut t.grid,
            _ => panic!("a table tab is active"),
        }
    }

    fn tick(&mut self) {
        junie_tui::shell::handle(&mut self.app, Input::Tick);
        self.draw();
    }

    fn draw(&mut self) {
        self.term
            .draw(|frame| junie_tui::shell::render(&mut self.app, frame))
            .expect("test backend draws");
    }

    fn press(&mut self, code: KeyCode) {
        junie_tui::shell::handle(
            &mut self.app,
            Input::Key(Key {
                code,
                mods: KeyModifiers::NONE,
            }),
        );
        self.draw();
    }

    fn screen(&self) -> String {
        let buf = self.term.backend().buffer();
        buf.content()
            .chunks(buf.area.width as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn report(&mut self, operation: &'static str, work: WorkDelta, oracle: &str) {
        let record_count = self.grid().rows().len();
        ScenarioReport {
            scenario: "tablepro_workbench_grid",
            operation,
            record_count,
            viewport: VIEWPORT,
            work,
            correctness_oracle: oracle.to_owned(),
        }
        .write()
        .expect("write the count report");
    }
}

fn within(w: &WorkDelta, what: &str, budgets: &[(Metric, u64)]) {
    if let Err(violations) = w.check_budget(budgets) {
        panic!("{what}: {violations}");
    }
}

#[test]
fn work_contract_tablepro_idle_and_cell_edit() {
    let mut f = Fixture::new();
    let rows = f.grid().rows().len();
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
    within(
        &idle,
        "settled tick",
        &[
            (Metric::VisibleRowsPainted, 41),
            (Metric::IntrinsicWidthCalls, 249),
            (Metric::TextBytesMeasured, 2_514),
        ],
    );
    f.report(
        "settled_tick",
        idle,
        "the frame text is unchanged by the tick",
    );

    let ((), nav) = measure(|| f.press(KeyCode::Down));
    assert_ne!(f.screen(), before, "Down moves the active cell on screen");
    within(
        &nav,
        "active-grid navigation",
        &[
            (Metric::VisibleRowsPainted, 41),
            (Metric::IntrinsicWidthCalls, 249),
            (Metric::TextBytesMeasured, 2_514),
        ],
    );
    f.report("grid_navigation", nav, "one Down press changes the frame");

    let before = f.screen();
    let ((), edit) = measure(|| {
        assert!(f.grid().record_cell(0, PRICE, CellValue::Num(1.5)));
        f.draw();
    });
    assert_ne!(f.screen(), before, "the keyed draft edit reaches the frame");
    assert!(
        f.screen().contains("1 pending"),
        "the status bar counts the draft"
    );
    assert_eq!(f.grid().pending().cells().count(), 1);
    assert_eq!(f.grid().value(0, PRICE), &CellValue::Num(1.5));
    within(
        &edit,
        "one keyed draft edit",
        &[
            (Metric::VisibleRowsPainted, 39),
            (Metric::IntrinsicWidthCalls, 257),
            (Metric::TextBytesMeasured, 2_535),
        ],
    );
    f.report(
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
    within(
        &unrelated,
        "unrelated history entry",
        &[
            (Metric::VisibleRowsPainted, 39),
            (Metric::IntrinsicWidthCalls, 257),
            (Metric::TextBytesMeasured, 2_535),
        ],
    );
    f.report(
        "irrelevant_source_change_then_tick",
        unrelated,
        "the frame text is unchanged by an unrelated history entry",
    );
}
