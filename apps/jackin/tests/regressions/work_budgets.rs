//! Count-only work scenarios for the Jackin console (#56).
//!
//! Each operation runs through the real app: `Application::handle` for the
//! tick, then `Application::render` into a 140x40 test terminal. Counts are
//! for the test thread only. Every number below was measured with
//!
//! ```text
//! cargo nextest run -p jackin-console --features demo,work-metrics \
//!     --test regressions --no-tests fail -E 'test(/^work_budgets::/)'
//! ```

#![cfg(feature = "work-metrics")]

use jackin_console::app::App;
use jackin_console::fixtures::{Scenario, app_for};
use junie_tui::core::event::Input;
use junie_tui::metrics::report::ScenarioReport;
use junie_tui::metrics::{Metric, WorkDelta, measure};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

const VIEWPORT: (u16, u16) = (140, 40);

struct Fixture {
    app: App,
    term: Terminal<TestBackend>,
}

impl Fixture {
    fn new(scenario: Scenario) -> Self {
        let app = app_for(
            scenario,
            Motion::Paused,
            10_000,
            Theme::for_level(ColorLevel::TrueColor),
        );
        let term = Terminal::new(TestBackend::new(VIEWPORT.0, VIEWPORT.1)).expect("test terminal");
        let mut f = Self { app, term };
        f.app.world.clock.running = true;
        for _ in 0..30 {
            f.tick();
        }
        f
    }

    fn tick(&mut self) {
        Application::handle(&mut self.app, Input::Tick);
        self.draw();
    }

    fn draw(&mut self) {
        self.term
            .draw(|frame| Application::render(&mut self.app, frame))
            .expect("test backend draws");
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
fn work_contract_jackin_idle_and_roster_update() {
    let mut f = Fixture::new(Scenario::Returning);
    let world = &f.app.world;
    let (workspaces, instances, daemons) = (
        world.workspaces.len(),
        world.instances.len(),
        world.daemons.len(),
    );
    assert_eq!(
        (workspaces, instances, daemons),
        (4, 9, 2),
        "the fixture size"
    );

    // The control tick visits each of the 2 daemons once and polls each pane
    // (6 in all); the roster is drawn without a counted list widget, so only
    // text measurement shows up in the frame.
    let budget = |text_bytes| {
        [
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
            (Metric::IntrinsicWidthCalls, 178),
            (Metric::TextBytesMeasured, text_bytes),
        ]
    };
    let before = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    within(&idle, "roster settled tick", &budget(1_531));
    report(
        "jackin_roster",
        "settled_tick",
        instances,
        idle,
        "4 workspaces, 9 instances and 2 daemons; the frame text is unchanged by the tick",
    );

    f.app.world.workspaces[3].git_pull = !f.app.world.workspaces[3].git_pull;
    let ((), unrelated) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "the git pull policy is not on the roster"
    );
    within(
        &unrelated,
        "unrelated workspace policy then tick",
        &budget(1_531),
    );
    report(
        "jackin_roster",
        "irrelevant_workspace_change_then_tick",
        instances,
        unrelated,
        "the frame text is unchanged by an unrelated workspace policy",
    );

    let name = f.app.world.workspaces[0].name.clone();
    f.app.world.workspaces[0].name = format!("{name}-renamed");
    let ((), rename) = measure(|| f.tick());
    assert_ne!(
        f.screen(),
        before,
        "the renamed workspace reaches the frame"
    );
    assert!(f.screen().contains(&format!("{name}-renamed")));
    within(&rename, "workspace rename then tick", &budget(1_571));
    report(
        "jackin_roster",
        "workspace_rename_then_tick",
        instances,
        rename,
        "the renamed workspace is on screen",
    );
}

#[test]
fn work_contract_jackin_capsule_idle_frame() {
    let mut f = Fixture::new(Scenario::CapsuleMulti);
    let before = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );

    // Known gap: every idle capsule frame deep-clones the pane's retained text
    // (payload_bytes_cloned is recorded only on the unprepared-draw fallback of
    // `ViewportOps::draw`) and segments and reflows the copy again. These are
    // the measured counts of today's behaviour, not a bound; a capsule that
    // draws a prepared view must bring the cloned bytes, segmented lines and
    // index rebuilds to zero.
    within(
        &idle,
        "capsule settled tick",
        &[
            (Metric::SegmentedLines, 2_021),
            (Metric::ReflowedLines, 4_012),
            (Metric::IndexRebuilds, 5),
            (Metric::PayloadBytesCloned, 80_059),
            (Metric::IntrinsicWidthCalls, 80_161),
            (Metric::TextBytesMeasured, 81_535),
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
        ],
    );
    assert!(
        idle.get(Metric::PayloadBytesCloned) > 0,
        "the clone is the measured cost, so a change that removes it must lower this budget"
    );
    report(
        "jackin_capsule",
        "settled_tick",
        f.app.world.daemons.len(),
        idle,
        "2 daemons with panes; the frame text is unchanged by the tick",
    );
}
