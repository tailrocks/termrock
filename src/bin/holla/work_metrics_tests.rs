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

// Counts are the product path only. A settled `FinderPage::ensure_projection`
// also rebuilds a reference page to compare against (test builds only,
// `assert_matches_reference`); that oracle runs under `projection::uncounted`,
// which suspends the library counters, so it adds nothing to these numbers. It
// once did: 26 width calls and bytes per run, so 52 in a settled tick, and in a
// key frame 26, then 52 after #283 made `on_key` run `ensure_projection` too.
// That was the whole +26 that 989f13c4 blamed on "new plan rows".

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

    // The Finder projection rebuilds only for its own dependencies (#62), so a
    // settled tick does no rebuild and no rank pass. What an idle tick still
    // costs is two scheduling passes over the 5 retained activities and the
    // frame's width measurements.
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
            (Metric::VisibleRowsPainted, 20),
            (Metric::SchedulingRecordsVisited, 2 * activities + plans),
            (Metric::IntrinsicWidthCalls, 706),
            (Metric::TextBytesMeasured, 3_220),
            (Metric::FitVisits, 19),
        ],
    );
    assert_eq!(
        idle.get(Metric::ProjectionRebuilds),
        0,
        "an idle tick does not rebuild"
    );
    report(
        &f,
        "settled_tick",
        idle,
        "the frame text is unchanged by the tick",
    );

    // An unrelated world field changes, then the next tick runs. The Finder
    // reads its own dependency stamps, so it does not rebuild.
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
            (Metric::VisibleRowsPainted, 20),
            (Metric::SchedulingRecordsVisited, 2 * activities + plans),
            (Metric::IntrinsicWidthCalls, 706),
            (Metric::TextBytesMeasured, 3_220),
            (Metric::FitVisits, 19),
        ],
    );
    assert_eq!(irrelevant.get(Metric::ProjectionRebuilds), 0);
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
            // The Finder's own rebuild and the SearchDetail projection built
            // from its result source (#64) are one rebuild each.
            (Metric::ProjectionRebuilds, 2),
            (Metric::SourceRowsEnumerated, 60),
            (Metric::VisibleRowsPainted, 31),
            (Metric::RankRowsVisited, items),
            (Metric::IntrinsicWidthCalls, 1_621),
            (Metric::TextBytesMeasured, 8_414),
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

const MIB: usize = 1024 * 1024;
const CAP: usize = crate::sim::fs::PREVIEW_MAX_BYTES;
const PREVIEW_DIR: &str = "/preview-fixture";
const PREVIEW_FILE: &str = "/preview-fixture/huge.log";

/// What the modelled file holds. The fixture is built outside the counted
/// operation, so the file's own bytes are never part of a delta.
enum Content {
    Text(String),
    Bytes(Vec<u8>),
}

fn repeated_lines(total: usize) -> String {
    let unit = "the same visible prefix of a large file\n";
    let mut text = unit.repeat(total / unit.len() + 1);
    text.truncate(total);
    text
}

/// A Holla app with one modelled file alone in `PREVIEW_DIR`, so the Files
/// page opens with that row selected and its preview drawer showing.
fn preview_fixture(content: Content) -> Harness<App> {
    let mut app = App::launch(Scenario::FirstUse, Motion::Paused, 0, Theme::junie(), false);
    app.world.clock.running = true;
    app.world.fs.dir(PREVIEW_DIR, 1);
    match content {
        Content::Text(t) => app.world.fs.text(PREVIEW_FILE, &t, 0),
        Content::Bytes(b) => app.world.fs.bytes(PREVIEW_FILE, b, 0),
    };
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(5);
    f
}

/// Opens the Files page through the real navigation and draws it: the frame
/// that selects the row prepares its preview.
fn open_files(f: &mut Harness<App>) -> WorkDelta {
    let ((), work) = measure(|| {
        f.app.push_page(crate::screens::Page::Files {
            path: PREVIEW_DIR.to_owned(),
            query: None,
        });
        f.draw();
    });
    work
}

/// The counts of a preview whose prefix is 2,000 identical 39-byte lines cut
/// at the 256 KiB cap. `measured` is the frame's text measurement, which
/// includes the size label of the row and so varies with the file's size.
fn capped_prefix(measured: u64) -> [(Metric, u64); 11] {
    [
        (Metric::SegmentedLines, 2_000),
        (Metric::ReflowedLines, 4_000),
        (Metric::IndexRebuilds, 2),
        (Metric::IntrinsicWidthCalls, 90_111),
        (Metric::TextBytesMeasured, measured),
        (Metric::FitVisits, 13),
        (Metric::PrefixRollbackProbes, 1),
        (Metric::NulBytesProbed, CAP as u64),
        (Metric::Utf8BytesValidated, CAP as u64),
        (Metric::TextBytesSanitized, 78_000),
        (Metric::LineCharsCounted, 78_000),
    ]
}

#[test]
fn work_contract_holla_file_preview_prefix() {
    // The same visible prefix in a 1 MiB and a 16 MiB file, as modelled text
    // and as modelled bytes. Preview preparation is bounded by the 256 KiB
    // cap, so its counts do not move with the file's length. Nothing is
    // cloned from the file, each pass runs once over the capped prefix, and
    // only the 2,000 capped lines are sanitized and laid out.
    let mut previews = vec![];
    for (label, size, content, measured) in [
        ("text_1mib", MIB, Content::Text(repeated_lines(MIB)), 90_868),
        (
            "text_16mib",
            16 * MIB,
            Content::Text(repeated_lines(16 * MIB)),
            90_871,
        ),
        (
            "bytes_16mib",
            16 * MIB,
            Content::Bytes(repeated_lines(16 * MIB).into_bytes()),
            90_871,
        ),
    ] {
        let mut f = preview_fixture(content);
        let work = open_files(&mut f);
        let screen = f.screen();
        assert!(
            screen.contains("first 256 KiB"),
            "{label}: truncation shown"
        );
        assert!(
            screen.contains("the same visible prefix of a large fil"),
            "{label}: the first lines are shown"
        );
        assert_exact(&work, label, &capped_prefix(measured));
        assert_eq!(work.get(Metric::PayloadBytesCloned), 0, "{label}: no copy");
        f.report(
            "holla_files_preview",
            label,
            size,
            work,
            "the preview drawer shows the first lines under a truncation notice",
        );
        previews.push(work);

        // The preview is prepared once for the selection: a settled tick
        // repeats none of the passes.
        let ((), tick) = measure(|| f.tick());
        assert_exact(
            &tick,
            &format!("{label}: settled tick"),
            &[
                (Metric::IntrinsicWidthCalls, 111),
                (Metric::TextBytesMeasured, measured - 90_000),
                (Metric::FitVisits, 13),
            ],
        );
    }
    let [one, sixteen, bytes] = [previews[0], previews[1], previews[2]];
    for metric in Metric::ALL {
        if metric != Metric::TextBytesMeasured {
            assert_eq!(
                one.get(metric),
                sixteen.get(metric),
                "{}: a 16x larger file does not change it",
                metric.name()
            );
        }
    }
    assert_eq!(sixteen, bytes, "text and bytes content cost the same");

    // A scalar straddling the cap rolls the cut back over its continuation
    // byte: 2 probes, and every later pass sees the 262,143 byte prefix. The
    // single long line is sanitized once and capped at 4,096 characters.
    let mut straddle = "a".repeat(CAP - 1);
    straddle.push('€');
    straddle.push_str(&"b".repeat(MIB));
    let mut f = preview_fixture(Content::Text(straddle));
    let work = open_files(&mut f);
    assert_exact(
        &work,
        "scalar straddling the cap",
        &[
            (Metric::SegmentedLines, 1),
            (Metric::ReflowedLines, 1),
            (Metric::IndexRebuilds, 1),
            (Metric::IntrinsicWidthCalls, 4_215),
            (Metric::TextBytesMeasured, 4_986),
            (Metric::FitVisits, 13),
            (Metric::PrefixRollbackProbes, 2),
            (Metric::NulBytesProbed, CAP as u64 - 1),
            (Metric::Utf8BytesValidated, CAP as u64 - 1),
            (Metric::TextBytesSanitized, CAP as u64 - 1),
            (Metric::LineCharsCounted, CAP as u64 - 1),
        ],
    );
    f.report(
        "holla_files_preview",
        "scalar_straddles_cap",
        MIB + CAP,
        work,
        "the long line is shown capped under a truncation notice",
    );

    // A NUL byte stops the probe where it sits, and nothing after it runs,
    // however large the file is.
    let mut nul = repeated_lines(16 * MIB).into_bytes();
    nul[100] = 0;
    let mut f = preview_fixture(Content::Bytes(nul));
    let work = open_files(&mut f);
    assert!(
        f.screen().contains("NUL byte"),
        "the binary notice is shown"
    );
    assert_exact(
        &work,
        "NUL in the prefix",
        &[
            (Metric::IntrinsicWidthCalls, 177),
            (Metric::TextBytesMeasured, 984),
            (Metric::FitVisits, 13),
            (Metric::PrefixRollbackProbes, 1),
            (Metric::NulBytesProbed, 101),
        ],
    );
    f.report(
        "holla_files_preview",
        "nul_in_prefix",
        16 * MIB,
        work,
        "the preview reports a binary file",
    );
}

#[test]
fn work_contract_holla_background_updates_keep_projection() {
    use crate::domain::activity::{ActivityKind, ActivityState, LineTone, Script, ScriptLine};
    use crate::domain::context::ScopeTag;

    // One activity that never finishes on its own, so only the test changes
    // its state.
    let mut app = App::launch(
        Scenario::RustDirty,
        Motion::Paused,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    let id = app.world.start_activity(
        vec![],
        Script::running(Vec::<ScriptLine>::new()),
        "streamer",
        "test",
        ActivityKind::Task,
        ScopeTag::here(&app.world.location.cwd),
    );
    let mut f = Harness::new(app, VIEWPORT);
    f.warm(31);
    assert_eq!(
        f.app.world.activity(&id).map(|a| a.state),
        Some(ActivityState::Running)
    );
    let items = catalog_items(&f) as u64;
    let activities = f.app.world.activities.len() as u64;
    assert_eq!((items, activities), (87, 1), "the fixture size");
    let scheduling = (Metric::SchedulingRecordsVisited, 2 * activities);

    // A running activity streams output while the Finder is on screen. Output
    // is display metadata of the activity, not a source of the Finder
    // projection, so 20 rounds of 50 lines rebuild and rank nothing, and each
    // round costs the same as a settled tick.
    let lines_before = f.app.world.activity(&id).expect("activity").output.len();
    for round in 0..20 {
        let ((), work) = measure(|| {
            f.app.world.edit_activity(&id, |a| {
                for i in 0..50 {
                    a.output
                        .push((None, format!("line {round}-{i}"), LineTone::Normal));
                }
            });
            f.tick();
        });
        assert_exact(
            &work,
            &format!("output round {round}"),
            &[
                scheduling,
                (Metric::VisibleRowsPainted, 16),
                (Metric::IntrinsicWidthCalls, 491),
                (Metric::TextBytesMeasured, 2_286),
                (Metric::FitVisits, 20),
            ],
        );
        if round == 19 {
            report(
                &f,
                "background_output_then_tick",
                work,
                "the activity holds 1,000 more retained lines and the Finder did not rebuild",
            );
        }
    }
    assert_eq!(
        f.app.world.activity(&id).expect("activity").output.len(),
        lines_before + 1_000,
        "every round reached the activity"
    );

    // The Finder goes to a background tab behind the activity's own tab, and
    // a source it does depend on changes. The hidden page does not rebuild,
    // however many ticks pass: only the domain scheduling and the visible
    // activity page cost anything.
    f.app.open_activity_tab(&id, true);
    assert_eq!(f.app.active(), 1, "the Finder is on a hidden tab");
    f.draw();
    f.app.world.fs.file("/work/scratch/hidden.txt", 8, 1);
    let ((), hidden) = measure(|| f.warm(4));
    assert_exact(
        &hidden,
        "four ticks with the Finder hidden",
        &[
            (Metric::SchedulingRecordsVisited, 4 * 2 * activities),
            (Metric::IntrinsicWidthCalls, 312),
            (Metric::TextBytesMeasured, 1_836),
            (Metric::FitVisits, 72),
        ],
    );
    report(
        &f,
        "hidden_finder_four_ticks",
        hidden,
        "the source changed under a hidden Finder, which rebuilt nothing",
    );

    // Coming back, the page catches up with one rebuild that ranks the whole
    // catalog once, however many ticks or changes it slept through.
    let ((), entered) = measure(|| {
        f.app.activate_tab(0);
        f.draw();
    });
    assert_exact(
        &entered,
        "Finder activated",
        &[
            // The Finder's own rebuild and the SearchDetail projection built
            // from its result source (#64) are one rebuild each.
            (Metric::ProjectionRebuilds, 2),
            (Metric::SourceRowsEnumerated, 125),
            (Metric::VisibleRowsPainted, 16),
            (Metric::RankRowsVisited, items),
            (Metric::IntrinsicWidthCalls, 508),
            (Metric::TextBytesMeasured, 2_319),
            (Metric::FitVisits, 20),
        ],
    );
    report(
        &f,
        "hidden_finder_activated",
        entered,
        "the Finder is on screen again and caught up",
    );
    let ((), settled) = measure(|| f.tick());
    assert_exact(
        &settled,
        "settled tick after the catch-up",
        &[
            (Metric::VisibleRowsPainted, 16),
            scheduling,
            (Metric::IntrinsicWidthCalls, 495),
            (Metric::TextBytesMeasured, 2_306),
            (Metric::FitVisits, 20),
        ],
    );

    // A relevant source change on the visible Finder is exactly one rebuild
    // of the catalog's 87 items, and the next tick is settled again.
    let ((), relevant) = measure(|| {
        f.app.world.fs.file("/work/scratch/visible.txt", 8, 1);
        f.tick();
    });
    assert_exact(
        &relevant,
        "relevant source change then tick",
        &[
            // The Finder's own rebuild and the SearchDetail projection built
            // from its result source (#64) are one rebuild each.
            (Metric::ProjectionRebuilds, 2),
            (Metric::SourceRowsEnumerated, 125),
            (Metric::VisibleRowsPainted, 16),
            (Metric::RankRowsVisited, items),
            scheduling,
            (Metric::IntrinsicWidthCalls, 508),
            (Metric::TextBytesMeasured, 2_319),
            (Metric::FitVisits, 20),
        ],
    );
    report(
        &f,
        "relevant_source_change_then_tick",
        relevant,
        "one rebuild ranked every catalog item once",
    );
    let ((), after) = measure(|| f.tick());
    assert_exact(
        &after,
        "settled tick after the rebuild",
        &[
            (Metric::VisibleRowsPainted, 16),
            scheduling,
            (Metric::IntrinsicWidthCalls, 495),
            (Metric::TextBytesMeasured, 2_306),
            (Metric::FitVisits, 20),
        ],
    );
}
