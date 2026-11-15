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
use junie_tui::core::event::{Input, Key};
use junie_tui::metrics::harness::{Harness, assert_exact};
use junie_tui::metrics::{Metric, measure};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

const VIEWPORT: (u16, u16) = (140, 40);

fn fixture(scenario: Scenario) -> Harness<App> {
    let mut app = app_for(
        scenario,
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    app.world.clock.running = true;
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(30);
    f
}

#[test]
fn work_contract_jackin_idle_and_roster_update() {
    let mut f = fixture(Scenario::Returning);
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
    let expected = |text_bytes| {
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
    assert_exact(&idle, "roster settled tick", &expected(1_531));
    f.report(
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
    assert_exact(
        &unrelated,
        "unrelated workspace policy then tick",
        &expected(1_531),
    );
    f.report(
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
    assert_exact(&rename, "workspace rename then tick", &expected(1_571));
    f.report(
        "jackin_roster",
        "workspace_rename_then_tick",
        instances,
        rename,
        "the renamed workspace is on screen",
    );
}

#[test]
fn work_contract_jackin_capsule_idle_frame() {
    let mut f = fixture(Scenario::CapsuleMulti);
    let before = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );

    // A capsule pane is prepared for its place in the layout whenever its
    // content or area changes, so an idle frame draws a prepared view: it
    // clones no history, segments no line and rebuilds no index. The width and
    // text counts are the menu, strip and status chrome, not pane payload.
    assert_exact(
        &idle,
        "capsule settled tick",
        &[
            (Metric::PayloadBytesCloned, 0),
            (Metric::SegmentedLines, 0),
            (Metric::ReflowedLines, 0),
            (Metric::IndexRebuilds, 0),
            (Metric::IntrinsicWidthCalls, 158),
            (Metric::TextBytesMeasured, 1_476),
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
        ],
    );
    f.report(
        "jackin_capsule",
        "settled_tick",
        f.app.world.daemons.len(),
        idle,
        "2 daemons with panes; the frame text is unchanged by the tick",
    );
}

#[test]
fn work_contract_jackin_capsule_frame_after_typing() {
    let mut f = fixture(Scenario::CapsuleMulti);
    let before = f.screen();
    f.app.handle(Input::Key(Key {
        code: KeyCode::Char('q'),
        mods: KeyModifiers::NONE,
    }));
    let ((), typed) = measure(|| f.draw());
    assert_ne!(f.screen(), before, "the typed key reaches the pane");
    assert_exact(
        &typed,
        "capsule frame after typing",
        &[
            (Metric::PayloadBytesCloned, 0),
            (Metric::SegmentedLines, 0),
            (Metric::ReflowedLines, 0),
            (Metric::IndexRebuilds, 0),
            (Metric::IntrinsicWidthCalls, 146),
            (Metric::TextBytesMeasured, 1_416),
            (Metric::SchedulingRecordsVisited, 0),
            (Metric::SchedulingPolls, 0),
        ],
    );
}
