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
    let expected = |widths, text_bytes, fit_visits| {
        [
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
            (Metric::IntrinsicWidthCalls, widths),
            (Metric::TextBytesMeasured, text_bytes),
            (Metric::FitVisits, fit_visits),
        ]
    };
    let before = f.screen();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen(),
        before,
        "a settled tick leaves the frame unchanged"
    );
    assert_exact(&idle, "roster settled tick", &expected(205, 1_466, 12));
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
        &expected(205, 1_466, 12),
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
    assert_exact(
        &rename,
        "workspace rename then tick",
        &expected(205, 1_498, 12),
    );
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
            (Metric::IntrinsicWidthCalls, 109),
            (Metric::TextBytesMeasured, 786),
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
            (Metric::FitVisits, 19),
            (Metric::FitCompares, 18),
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
            (Metric::IntrinsicWidthCalls, 97),
            (Metric::TextBytesMeasured, 726),
            (Metric::SchedulingRecordsVisited, 0),
            (Metric::SchedulingPolls, 0),
            (Metric::FitVisits, 19),
            (Metric::FitCompares, 18),
        ],
    );
}

#[test]
fn work_contract_jackin_account_update() {
    let mut f = fixture(Scenario::Returning);
    f.press(KeyCode::Char('c'));
    f.press(KeyCode::Down);
    f.press(KeyCode::Down);
    f.warm(30);
    let accounts = f.app.world.accounts.accounts.len() as u64;
    assert_eq!(accounts, 12, "the fixture size");
    let id = f
        .app
        .world
        .accounts
        .accounts
        .iter()
        .find(|a| a.display_name == "Personal")
        .map(|a| a.id.clone())
        .expect("the selected account");
    assert!(f.screen().contains("Claude · Personal"), "it is selected");

    // The Accounts page is on screen and the Manager sits in its background
    // tab. A settled tick polls the 2 daemons and rebuilds nothing.
    let settled = [
        (Metric::SchedulingRecordsVisited, 2),
        (Metric::SchedulingPolls, 6),
        (Metric::IntrinsicWidthCalls, 391),
        (Metric::TextBytesMeasured, 2_010),
        (Metric::FitVisits, 15),
    ];
    let before = f.screen_without_spinner();
    let ((), idle) = measure(|| f.tick());
    assert_eq!(
        f.screen_without_spinner(),
        before,
        "the tick changes nothing"
    );
    assert_exact(&idle, "accounts settled tick", &settled);
    f.report(
        "jackin_accounts",
        "settled_tick",
        accounts as usize,
        idle,
        "12 accounts on the Accounts page; the frame text is unchanged by the tick",
    );

    // A roster change moves the roster epoch only. Neither the visible
    // Accounts projection nor the Manager in the background rebuilds.
    let workspace = f.app.world.workspaces[3].id;
    let pull = f.app.world.workspaces[3].git_pull;
    f.app
        .world
        .workspace_mut(workspace)
        .expect("workspace")
        .git_pull = !pull;
    let ((), background) = measure(|| f.tick());
    assert_eq!(
        f.screen_without_spinner(),
        before,
        "the git pull policy is not on the Accounts page"
    );
    assert_exact(&background, "roster change then tick", &settled);
    f.report(
        "jackin_accounts",
        "background_roster_change_then_tick",
        accounts as usize,
        background,
        "the frame text is unchanged by a workspace policy edit",
    );

    // `r` marks the account refreshing: one registry change, so one sort of
    // the 12 accounts and one row build.
    let ((), start) = measure(|| f.press(KeyCode::Char('r')));
    assert!(f.app.world.refresh_pending(&id).is_some(), "refresh is out");
    assert_exact(
        &start,
        "refresh started",
        &[
            (Metric::ProjectionRebuilds, 2),
            (Metric::RankRowsVisited, accounts),
            (Metric::IntrinsicWidthCalls, 438),
            (Metric::TextBytesMeasured, 2_115),
            (Metric::FitVisits, 19),
        ],
    );

    // While the reply is out, every tick is the same animated frame and
    // rebuilds nothing.
    let waiting = [
        (Metric::SchedulingRecordsVisited, 2),
        (Metric::SchedulingPolls, 6),
        (Metric::IntrinsicWidthCalls, 450),
        (Metric::TextBytesMeasured, 2_175),
        (Metric::FitVisits, 19),
    ];
    let mut ticks = 0;
    let delivered = loop {
        let ((), tick) = measure(|| f.tick());
        ticks += 1;
        assert!(ticks <= 40, "the simulated reply arrives");
        if f.app.world.refresh_pending(&id).is_none() {
            break tick;
        }
        assert_exact(&tick, "tick while the refresh is out", &waiting);
    };
    assert_eq!(ticks, 10, "the simulated refresh answers on the 10th tick");

    // The reply is one registry change: one sort and one row build, however
    // many ticks waited for it.
    assert!(f.screen().contains("Refreshed Claude · Personal"));
    assert_exact(
        &delivered,
        "account update delivered",
        &[
            (Metric::ProjectionRebuilds, 2),
            (Metric::RankRowsVisited, accounts),
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
            (Metric::IntrinsicWidthCalls, 382),
            (Metric::TextBytesMeasured, 2_031),
            (Metric::FitVisits, 19),
        ],
    );
    f.report(
        "jackin_accounts",
        "account_update_delivered",
        accounts as usize,
        delivered,
        "the account shows refreshed and its status line says so",
    );

    let ((), after) = measure(|| f.tick());
    assert_exact(
        &after,
        "settled tick after the update",
        &[
            (Metric::SchedulingRecordsVisited, 2),
            (Metric::SchedulingPolls, 6),
            (Metric::IntrinsicWidthCalls, 382),
            (Metric::TextBytesMeasured, 2_031),
            (Metric::FitVisits, 19),
        ],
    );
}
