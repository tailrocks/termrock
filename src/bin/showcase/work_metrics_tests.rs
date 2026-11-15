//! Count-only work scenarios for Showcase pages (#56).
//!
//! Each operation runs through the real app: `shell::handle` for the tick or
//! pointer event, then `shell::render` into a 140x44 test terminal. Counts are
//! for the test thread only. Every number below was measured with
//!
//! ```text
//! cargo nextest run -p junie-tui --features work-metrics --bin showcase \
//!     --no-tests fail -E 'test(/^work_metrics_tests::/)'
//! ```

use junie_tui::core::event::{Input, Mouse, MouseKind};
use junie_tui::metrics::report::ScenarioReport;
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::motion::Motion;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;

use crate::app::{App, PageId};

const VIEWPORT: (u16, u16) = (140, 44);

struct Fixture {
    app: App,
    term: Terminal<TestBackend>,
}

impl Fixture {
    fn on(page: PageId) -> Self {
        let mut app = App::with_motion(Theme::for_level(ColorLevel::TrueColor), Motion::Paused, 0);
        app.open_page(page);
        let term = Terminal::new(TestBackend::new(VIEWPORT.0, VIEWPORT.1)).expect("test terminal");
        let mut f = Self { app, term };
        for _ in 0..30 {
            f.tick();
        }
        f
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

    fn click_text(&mut self, text: &str) {
        let screen = self.screen();
        let (y, line) = screen
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains(text))
            .unwrap_or_else(|| panic!("{text} not on screen:\n{screen}"));
        let x = line[..line.find(text).expect("found")].chars().count();
        for kind in [MouseKind::Down, MouseKind::Up] {
            junie_tui::shell::handle(
                &mut self.app,
                Input::Mouse(Mouse::new(kind, Position::new(x as u16 + 1, y as u16))),
            );
            self.draw();
        }
    }

    fn screen(&self) -> String {
        let buf = self.term.backend().buffer();
        buf.content()
            .chunks(buf.area.width as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn report(
    scenario: &'static str,
    operation: &'static str,
    record_count: usize,
    work: WorkDelta,
    oracle: &str,
) {
    ScenarioReport {
        scenario,
        operation,
        record_count,
        viewport: VIEWPORT,
        work,
        correctness_oracle: oracle.to_owned(),
    }
    .write()
    .expect("write the count report");
}

fn within(w: &WorkDelta, what: &str, budgets: &[(Metric, u64)]) {
    if let Err(violations) = w.check_budget(budgets) {
        panic!("{what}: {violations}");
    }
}
#[test]
fn work_contract_showcase_idle_and_switch() {
    let mut f = Fixture::on(PageId::QueryWorkbench);
    let before = f.screen();
    assert!(before.contains("Query 1") && before.contains("customers"));
    assert!(
        before.contains("FROM orders"),
        "Query 1 is the active document"
    );

    // Rows painted is summed over every list, table, grid and tree widget in
    // the frame. The page has two documents; each frame also paints the
    // Explorer tree.
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    within(
        &idle,
        "query workbench settled tick",
        &[
            (Metric::VisibleRowsPainted, 47),
            (Metric::IntrinsicWidthCalls, 389),
            (Metric::TextBytesMeasured, 1_851),
        ],
    );
    report(
        "showcase_query_workbench",
        "settled_tick",
        2,
        idle,
        "two documents in the strip and the frame text is unchanged by the tick",
    );

    // Pointer down and up on the second tab, each followed by a frame.
    let ((), switch) = measure(|| f.click_text("customers"));
    let after = f.screen();
    assert!(
        after.contains("FROM customers"),
        "the customers document is active"
    );
    assert!(!after.contains("FROM orders"));
    within(
        &switch,
        "document switch (two frames)",
        &[
            (Metric::VisibleRowsPainted, 82),
            (Metric::IntrinsicWidthCalls, 593),
            (Metric::TextBytesMeasured, 3_099),
        ],
    );
    report(
        "showcase_query_workbench",
        "switch_document",
        2,
        switch,
        "the editor shows the customers query after the click",
    );

    let mut g = Fixture::on(PageId::Finder);
    let before = g.screen();
    let ((), finder) = measure(|| g.tick());
    assert_eq!(
        g.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    // The Finder page paints a detail card that sizes itself in a scratch
    // buffer on every frame: 1440 cells allocated per frame today.
    within(
        &finder,
        "finder settled tick",
        &[
            (Metric::AllocatedBufferCells, 1_440),
            (Metric::VisibleRowsPainted, 24),
            (Metric::IntrinsicWidthCalls, 208),
            (Metric::TextBytesMeasured, 1_219),
        ],
    );
    report(
        "showcase_finder",
        "settled_tick",
        crate::pages::finder::ACTION_COUNT,
        finder,
        "the frame text is unchanged by the tick",
    );

    // An option of another page changes while the Finder is on screen.
    let set = crate::options::parse_opt("queryworkbench.read_only=on").expect("a valid option");
    g.app.apply_opt(set);
    let ((), unrelated) = measure(|| g.tick());
    assert_eq!(
        g.screen(),
        before,
        "the Query workbench option is not on the Finder page"
    );
    within(
        &unrelated,
        "unrelated page option then tick",
        &[
            (Metric::AllocatedBufferCells, 1_440),
            (Metric::VisibleRowsPainted, 24),
            (Metric::IntrinsicWidthCalls, 208),
            (Metric::TextBytesMeasured, 1_219),
        ],
    );
    report(
        "showcase_finder",
        "irrelevant_option_change_then_tick",
        crate::pages::finder::ACTION_COUNT,
        unrelated,
        "the frame text is unchanged by another page's option",
    );
}
