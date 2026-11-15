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

use junie_tui::metrics::harness::{Harness, assert_exact};
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::motion::Motion;
use junie_tui::theme::Theme;
use ratatui::crossterm::event::KeyCode;

use crate::app::App;
use crate::scenario::Scenario;

const VIEWPORT: (u16, u16) = (120, 40);
const SCENARIO: &str = "holla_finder";

fn fixture(scenario: Scenario) -> Harness<App> {
    let mut app = App::launch(scenario, Motion::Paused, 0, Theme::junie(), false);
    app.world.clock.running = true;
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(31);
    f
}

fn catalog_items(f: &Harness<App>) -> usize {
    crate::domain::catalog::build_with_warnings(&f.app.world)
        .0
        .len()
}

fn report(f: &Harness<App>, operation: &'static str, work: WorkDelta, oracle: &str) {
    f.report(SCENARIO, operation, catalog_items(f), work, oracle);
}

#[test]
fn work_contract_holla_idle_and_source_update() {
    let mut f = fixture(Scenario::ActivitiesMulti);
    let items = catalog_items(&f) as u64;
    let activities = f.app.world.activities.len() as u64;
    let plans = f.app.world.plans.len() as u64;
    assert_eq!((items, activities, plans), (106, 5, 0), "the fixture size");

    // Today every distinct world tick rebuilds the whole Finder projection
    // from the catalog, whether or not anything changed (#62 owns that fix).
    // The counts below are therefore the measured cost of the current page,
    // not a claim that idle work is bounded: one rebuild, one rank pass over
    // all 106 items, and two scheduling passes over the 5 retained activities.
    let before = f.screen_without_spinner();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen_without_spinner(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    // Scheduling visits each activity in the ready scan and in the advance
    // loop, then each plan once.
    assert_exact(
        &idle,
        "settled tick",
        &[
            (Metric::ProjectionRebuilds, 1),
            (Metric::RankRowsVisited, items),
            (Metric::SchedulingRecordsVisited, 2 * activities + plans),
            (Metric::IntrinsicWidthCalls, 704),
            (Metric::TextBytesMeasured, 3_271),
            (Metric::FitVisits, 19),
        ],
    );
    assert_eq!(
        idle.get(Metric::ProjectionRebuilds),
        1,
        "an idle tick still rebuilds"
    );
    report(
        &f,
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
    assert_eq!(
        f.screen_without_spinner(),
        before,
        "the unrelated field is not on screen"
    );
    assert_exact(
        &irrelevant,
        "unrelated field then tick",
        &[
            (Metric::ProjectionRebuilds, 1),
            (Metric::RankRowsVisited, items),
            (Metric::SchedulingRecordsVisited, 2 * activities + plans),
            (Metric::IntrinsicWidthCalls, 704),
            (Metric::TextBytesMeasured, 3_271),
            (Metric::FitVisits, 19),
        ],
    );
    assert_eq!(irrelevant.get(Metric::ProjectionRebuilds), 1);
    report(
        &f,
        "irrelevant_source_change_then_tick",
        irrelevant,
        "the frame text is unchanged by an unrelated world field",
    );

    let ((), key) = measure(|| f.press(KeyCode::Char('d')));
    assert_ne!(
        f.screen_without_spinner(),
        before,
        "the query change reaches the frame"
    );
    // A key does not tick the world, so no scheduling work is counted.
    assert_exact(
        &key,
        "query change",
        &[
            (Metric::ProjectionRebuilds, 1),
            (Metric::RankRowsVisited, items),
            (Metric::IntrinsicWidthCalls, 1_627),
            (Metric::TextBytesMeasured, 8_473),
            (Metric::FitVisits, 13),
        ],
    );
    assert_eq!(key.get(Metric::SchedulingRecordsVisited), 0);
    report(
        &f,
        "query_change",
        key,
        "the frame differs after the query changes",
    );
}
