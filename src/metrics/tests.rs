use super::state::{epoch_for_test, set_for_test};
use super::*;

#[test]
fn metrics_delta_counts_units_between_snapshots() {
    let before = snapshot();
    record_work!(SegmentedLines, 3usize);
    record_work!(SegmentedLines, 4usize);
    record_work!(BufferCellsFilled, 10usize);
    let delta = snapshot().delta(before).expect("same thread and epoch");
    assert_eq!(delta.get(Metric::SegmentedLines), 7);
    assert_eq!(delta.get(Metric::BufferCellsFilled), 10);
    assert_eq!(delta.get(Metric::ReflowedLines), 0);
    let nonzero: Vec<_> = delta.nonzero().collect();
    assert_eq!(
        nonzero,
        [(Metric::SegmentedLines, 7), (Metric::BufferCellsFilled, 10)]
    );
}

#[test]
fn metrics_suspended_drops_work_and_resumes_after_a_panic() {
    let (_, delta) = measure(|| {
        record_work!(SegmentedLines, 1usize);
        suspended(|| {
            record_work!(SegmentedLines, 10usize);
            suspended(|| record_work!(SegmentedLines, 100usize));
            record_work!(SegmentedLines, 1000usize);
        });
        let unwound = std::panic::catch_unwind(|| {
            suspended(|| panic!("a reference computation failed"));
        });
        assert!(unwound.is_err());
        record_work!(SegmentedLines, 2usize);
    });
    assert_eq!(delta.get(Metric::SegmentedLines), 3);
}

#[test]
fn metrics_measure_returns_the_value_and_its_work() {
    let (value, delta) = measure(|| {
        record_work!(LabelsNormalized, 2usize);
        41 + 1
    });
    assert_eq!(value, 42);
    assert_eq!(delta.get(Metric::LabelsNormalized), 2);
}

#[test]
fn metrics_counters_are_isolated_per_thread() {
    let before = snapshot();
    std::thread::spawn(|| {
        record_work!(SegmentedLines, 100usize);
        assert_eq!(snapshot().counts[Metric::SegmentedLines as usize], 100);
    })
    .join()
    .expect("worker");
    let delta = snapshot().delta(before).expect("same thread");
    assert_eq!(
        delta.get(Metric::SegmentedLines),
        0,
        "worker work is not ours"
    );
}

#[test]
fn metrics_delta_rejects_other_thread() {
    let here = snapshot();
    let there = std::thread::spawn(snapshot).join().expect("worker");
    assert_eq!(here.epoch, there.epoch, "the epoch numbers match");
    assert_eq!(there.delta(here), Err(MetricsError::DifferentThread));
    assert_eq!(here.delta(there), Err(MetricsError::DifferentThread));
}

#[test]
fn metrics_delta_rejects_reset() {
    record_work!(IndexRebuilds, 2usize);
    let before = snapshot();
    reset();
    record_work!(IndexRebuilds, 5usize);
    let after = snapshot();
    assert!(
        after.counts[Metric::IndexRebuilds as usize]
            > before.counts[Metric::IndexRebuilds as usize],
        "the post-reset count exceeds the old one, so subtraction alone passes"
    );
    assert_eq!(after.delta(before), Err(MetricsError::DifferentEpoch));
}

#[test]
fn metrics_delta_rejects_a_counter_that_went_backwards() {
    let before = snapshot();
    let mut after = before;
    after.counts[Metric::IndexShifts as usize] = before.counts[Metric::IndexShifts as usize] + 1;
    let mut earlier = before;
    earlier.counts[Metric::IndexShifts as usize] = after.counts[Metric::IndexShifts as usize] + 1;
    assert_eq!(
        after.delta(earlier),
        Err(MetricsError::CounterWentBackwards {
            index: Metric::IndexShifts as usize
        })
    );
}

#[test]
#[should_panic(expected = "work counter payload_bytes_cloned overflowed u64")]
fn metrics_record_panics_on_counter_overflow() {
    set_for_test(Metric::PayloadBytesCloned, u64::MAX);
    record(Metric::PayloadBytesCloned, 1);
}

#[test]
#[should_panic(expected = "work counter epoch overflowed u64")]
fn metrics_reset_panics_on_epoch_overflow() {
    epoch_for_test(u64::MAX);
    reset();
}

#[test]
#[should_panic(expected = "work metric amount must fit u64")]
fn metrics_record_work_rejects_an_amount_that_does_not_fit() {
    record_work!(SegmentedLines, -1i64);
}

#[test]
fn metrics_merge_adds_worker_deltas_and_reports_overflow() {
    let mut a = WorkDelta::ZERO;
    a.0[Metric::SegmentedLines as usize] = 3;
    let mut b = WorkDelta::ZERO;
    b.0[Metric::SegmentedLines as usize] = 4;
    b.0[Metric::IndexShifts as usize] = 1;
    let merged = a.merged(b).expect("fits");
    assert_eq!(merged.get(Metric::SegmentedLines), 7);
    assert_eq!(merged.get(Metric::IndexShifts), 1);

    let mut full = WorkDelta::ZERO;
    full.0[Metric::SegmentedLines as usize] = u64::MAX;
    assert_eq!(
        full.merged(a),
        Err(MetricsError::Overflow {
            index: Metric::SegmentedLines as usize
        })
    );
}

#[test]
fn metrics_names_are_unique_and_follow_index_order() {
    let mut seen = std::collections::HashSet::new();
    for (index, metric) in Metric::ALL.into_iter().enumerate() {
        assert_eq!(metric as usize, index);
        assert!(seen.insert(metric.name()), "{} repeats", metric.name());
    }
    assert_eq!(Metric::ALL.len(), METRIC_COUNT);
}

#[test]
fn metrics_check_budget_flags_overruns_and_unlisted_work() {
    let mut delta = WorkDelta::ZERO;
    delta.0[Metric::SegmentedLines as usize] = 5;
    delta.0[Metric::IndexShifts as usize] = 1;

    assert_eq!(
        delta.check_budget(&[(Metric::SegmentedLines, 5), (Metric::IndexShifts, 2)]),
        Ok(())
    );
    assert_eq!(
        delta.check_budget(&[(Metric::SegmentedLines, 4), (Metric::IndexShifts, 2)]),
        Err("segmented_lines was 5, budget 4".to_owned())
    );
    assert_eq!(
        delta.check_budget(&[(Metric::SegmentedLines, 5)]),
        Err("index_shifts was 1, budget 0".to_owned()),
        "a metric that is not listed has a budget of zero"
    );
}

#[test]
fn metrics_check_exact_flags_undercounts_overcounts_and_unlisted_work() {
    let mut delta = WorkDelta::ZERO;
    delta.0[Metric::SegmentedLines as usize] = 5;
    delta.0[Metric::IndexShifts as usize] = 1;

    assert_eq!(
        delta.check_exact(&[(Metric::SegmentedLines, 5), (Metric::IndexShifts, 1)]),
        Ok(())
    );
    assert_eq!(
        delta.check_exact(&[(Metric::SegmentedLines, 6), (Metric::IndexShifts, 1)]),
        Err("segmented_lines was 5, expected 6".to_owned()),
        "a count below the expectation fails, which a ceiling would accept"
    );
    assert_eq!(
        delta.check_exact(&[(Metric::SegmentedLines, 4), (Metric::IndexShifts, 1)]),
        Err("segmented_lines was 5, expected 4".to_owned())
    );
    assert_eq!(
        delta.check_exact(&[(Metric::SegmentedLines, 5)]),
        Err("index_shifts was 1, expected 0".to_owned()),
        "a metric that is not listed is expected to be zero"
    );
}

struct LabelApp;

impl crate::runtime::Application for LabelApp {
    fn handle(&mut self, _: crate::core::event::Input) -> crate::core::event::Outcome {
        crate::core::event::Outcome::Changed
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let label = crate::core::keymap::KeyBinding::new(
            ratatui::crossterm::event::KeyCode::Char('k'),
            ratatui::crossterm::event::KeyModifiers::SUPER,
        )
        .label();
        frame.render_widget(ratatui::widgets::Paragraph::new(label), frame.area());
    }

    fn should_quit(&self) -> bool {
        false
    }

    fn tick_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(100)
    }
}

#[test]
fn harness_frames_do_not_depend_on_the_host_platform() {
    let host = crate::core::event::ClientPlatform::current();
    let mut f = harness::Harness::new(LabelApp, (10, 1));
    f.tick();
    assert!(
        f.screen().starts_with("⌘K"),
        "the harness measures the macOS label on every host: {:?}",
        f.screen()
    );
    drop(f);
    assert_eq!(
        crate::core::event::ClientPlatform::current(),
        host,
        "dropping the harness restores the host platform"
    );
}
