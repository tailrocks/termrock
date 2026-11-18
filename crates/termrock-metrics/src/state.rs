use std::cell::Cell;

use super::{METRIC_COUNT, Metric, WorkDelta, WorkSnapshot};

thread_local! {
    static COUNTS: [Cell<u64>; METRIC_COUNT] = const { [const { Cell::new(0) }; METRIC_COUNT] };
    static EPOCH: Cell<u64> = const { Cell::new(0) };
    static SUSPENDED: Cell<u32> = const { Cell::new(0) };
}

/// Adds `amount` to `metric` on this thread. Overflow panics: a wrapped or
/// saturated counter could make a budget pass for the wrong reason.
pub fn record(metric: Metric, amount: u64) {
    if SUSPENDED.with(Cell::get) > 0 {
        return;
    }
    COUNTS.with(|counts| {
        let cell = &counts[metric as usize];
        let next = cell.get().checked_add(amount).unwrap_or_else(|| {
            panic!("work counter {} overflowed u64", metric.name());
        });
        cell.set(next);
    });
}

/// Runs `f` without counting: every [`record`] on this thread inside it is
/// dropped. For test-only reference computations, such as a differential
/// check that rebuilds a page from nothing, whose work is not the work of the
/// path under measurement. Nests, and counts again when `f` returns or panics.
pub fn suspended<T>(f: impl FnOnce() -> T) -> T {
    struct Resume;
    impl Drop for Resume {
        fn drop(&mut self) {
            SUSPENDED.with(|depth| depth.set(depth.get() - 1));
        }
    }
    SUSPENDED.with(|depth| depth.set(depth.get() + 1));
    let _resume = Resume;
    f()
}

/// This thread's counts, tagged with its identity and reset epoch.
pub fn snapshot() -> WorkSnapshot {
    let mut counts = [0; METRIC_COUNT];
    COUNTS.with(|cells| {
        for (slot, cell) in counts.iter_mut().zip(cells) {
            *slot = cell.get();
        }
    });
    WorkSnapshot {
        thread: std::thread::current().id(),
        epoch: EPOCH.with(Cell::get),
        counts,
    }
}

/// Zeroes this thread's counts and starts a new epoch, so a snapshot taken
/// before the reset can no longer produce a delta.
pub fn reset() {
    EPOCH.with(|epoch| {
        let next = epoch
            .get()
            .checked_add(1)
            .expect("work counter epoch overflowed u64");
        epoch.set(next);
    });
    COUNTS.with(|cells| cells.iter().for_each(|cell| cell.set(0)));
}

/// Runs `f` and returns its value with the work this thread did meanwhile.
///
/// Panics if `f` calls [`reset`]: the two snapshots would not be comparable.
pub fn measure<T>(f: impl FnOnce() -> T) -> (T, WorkDelta) {
    let before = snapshot();
    let value = f();
    let delta = snapshot()
        .delta(before)
        .unwrap_or_else(|e| panic!("work measurement is not comparable: {e}"));
    (value, delta)
}

#[doc(hidden)]
pub fn set_for_test(metric: Metric, value: u64) {
    COUNTS.with(|cells| cells[metric as usize].set(value));
}

#[doc(hidden)]
pub fn epoch_for_test(value: u64) {
    EPOCH.with(|epoch| epoch.set(value));
}
