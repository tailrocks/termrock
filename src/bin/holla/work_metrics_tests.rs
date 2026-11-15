//! Count-only work scenarios for Holla's Finder page (#56).
//!
//! Each operation runs through the real app: `handle_input` for the tick or
//! key, then `shell::render` into a 120x40 test terminal. Counts are for the
//! test thread only. Every number below was measured with
//!
//! ```text
//! cargo nextest run -p junie-tui --features work-metrics --bin holla \
//!     --no-tests fail -E 'test(/^work_metrics_tests::/)'
//! ```

use junie_tui::core::event::{Input, Key};
use junie_tui::metrics::report::ScenarioReport;
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::motion::Motion;
use junie_tui::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::app::App;
use crate::scenario::Scenario;

const VIEWPORT: (u16, u16) = (120, 40);

struct Fixture {
    app: App,
    term: Terminal<TestBackend>,
}

impl Fixture {
    fn new(scenario: Scenario) -> Self {
        let mut app = App::launch(scenario, Motion::Paused, 0, Theme::junie(), false);
        app.world.clock.running = true;
        let term = Terminal::new(TestBackend::new(VIEWPORT.0, VIEWPORT.1)).expect("test terminal");
        let mut f = Self { app, term };
        for _ in 0..30 {
            f.tick();
        }
        f.tick();
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

    fn type_char(&mut self, c: char) {
        junie_tui::shell::handle(
            &mut self.app,
            Input::Key(Key {
                code: KeyCode::Char(c),
                mods: KeyModifiers::NONE,
            }),
        );
        self.draw();
    }

    /// The frame text with the animated braille spinner glyphs masked, so two
    /// frames compare equal when only the spinner advanced.
    fn screen(&self) -> String {
        let buf = self.term.backend().buffer();
        buf.content()
            .chunks(buf.area.width as usize)
            .map(|row| {
                row.iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
                    .chars()
                    .map(|ch| {
                        if ('\u{2800}'..='\u{28ff}').contains(&ch) {
                            '*'
                        } else {
                            ch
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn catalog_items(&self) -> usize {
        crate::domain::catalog::build_with_warnings(&self.app.world)
            .0
            .len()
    }

    fn report(&self, operation: &'static str, work: WorkDelta, oracle: &str) {
        ScenarioReport {
            scenario: "holla_finder",
            operation,
            record_count: self.catalog_items(),
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
fn work_contract_holla_idle_and_source_update() {
    let mut f = Fixture::new(Scenario::ActivitiesMulti);
    let items = f.catalog_items() as u64;
    let activities = f.app.world.activities.len() as u64;
    let plans = f.app.world.plans.len() as u64;
    assert_eq!((items, activities, plans), (106, 5, 0), "the fixture size");

    // Today every distinct world tick rebuilds the whole Finder projection
    // from the catalog, whether or not anything changed (#62 owns that fix).
    // The budgets below are therefore the measured cost of the current page,
    // not a claim that idle work is bounded: one rebuild, one rank pass over
    // all 106 items, and two scheduling passes over the 5 retained activities.
    let before = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    // Scheduling visits each activity in the ready scan and in the advance
    // loop, then each plan once.
    within(
        &idle,
        "settled tick",
        &[
            (Metric::ProjectionRebuilds, 1),
            (Metric::RankRowsVisited, items),
            (Metric::SchedulingRecordsVisited, 2 * activities + plans),
            (Metric::IntrinsicWidthCalls, 716),
            (Metric::TextBytesMeasured, 3_358),
        ],
    );
    assert_eq!(
        idle.get(Metric::ProjectionRebuilds),
        1,
        "an idle tick still rebuilds"
    );
    f.report(
        "settled_tick",
        idle,
        "the frame text is unchanged by the tick",
    );

    // An unrelated world field changes, then the next tick runs. The Finder
    // cannot tell that nothing relevant changed, so it rebuilds again.
    let ((), irrelevant) = measure(|| {
        f.app.world.clipboard = Some("unrelated".to_owned());
        f.tick();
    });
    assert_eq!(f.screen(), before, "the unrelated field is not on screen");
    within(
        &irrelevant,
        "unrelated field then tick",
        &[
            (Metric::ProjectionRebuilds, 1),
            (Metric::RankRowsVisited, items),
            (Metric::SchedulingRecordsVisited, 2 * activities + plans),
            (Metric::IntrinsicWidthCalls, 716),
            (Metric::TextBytesMeasured, 3_358),
        ],
    );
    assert_eq!(irrelevant.get(Metric::ProjectionRebuilds), 1);
    f.report(
        "irrelevant_source_change_then_tick",
        irrelevant,
        "the frame text is unchanged by an unrelated world field",
    );

    let ((), key) = measure(|| f.type_char('d'));
    assert_ne!(f.screen(), before, "the query change reaches the frame");
    // A key does not tick the world, so no scheduling work is counted.
    within(
        &key,
        "query change",
        &[
            (Metric::ProjectionRebuilds, 1),
            (Metric::RankRowsVisited, items),
            (Metric::IntrinsicWidthCalls, 1_639),
            (Metric::TextBytesMeasured, 8_552),
        ],
    );
    assert_eq!(key.get(Metric::SchedulingRecordsVisited), 0);
    f.report(
        "query_change",
        key,
        "the frame differs after the query changes",
    );
}
