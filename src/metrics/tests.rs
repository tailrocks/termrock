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
