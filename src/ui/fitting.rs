//! Shared pieces of the responsive fitters (chips, breadcrumb, segments, status
//! bar, key hints, toolbar): the order candidates are removed in, and the work
//! counters tests read to prove a fit is linear in the number of items.

/// Candidate indices in removal order, lowest key first.
///
/// Keys are unique, so the unstable sort is deterministic. One sort replaces a
/// scan of every survivor per removal.
pub(crate) fn removal_order(keys: &[(u8, u8, usize)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_unstable_by(|&a, &b| {
        work::compare();
        keys[a].cmp(&keys[b])
    });
    order
}

/// Counters of the work a fit does. Test builds count, other builds compile
/// every call to nothing.
pub(crate) mod work {
    #[cfg(test)]
    pub(crate) fn width() {
        counting::bump(|w| w.widths += 1);
    }
    #[cfg(test)]
    pub(crate) fn visit() {
        counting::bump(|w| w.visits += 1);
    }
    #[cfg(test)]
    pub(crate) fn compare() {
        counting::bump(|w| w.compares += 1);
    }

    #[cfg(not(test))]
    #[inline(always)]
    pub(crate) fn width() {}
    #[cfg(not(test))]
    #[inline(always)]
    pub(crate) fn visit() {}
    #[cfg(not(test))]
    #[inline(always)]
    pub(crate) fn compare() {}

    #[cfg(test)]
    pub(crate) use counting::{Work, measure};

    #[cfg(test)]
    mod counting {
        use std::cell::Cell;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub(crate) struct Work {
            /// Display-width evaluations of one string (`ui::text::width`).
            pub widths: u64,
            /// Elements visited by a fitting loop.
            pub visits: u64,
            /// Key comparisons made while ordering removal candidates.
            pub compares: u64,
        }

        thread_local! {
            static WORK: Cell<Work> = const { Cell::new(Work { widths: 0, visits: 0, compares: 0 }) };
        }

        pub(super) fn bump(f: impl FnOnce(&mut Work)) {
            WORK.with(|w| {
                let mut v = w.get();
                f(&mut v);
                w.set(v);
            });
        }

        /// The work `run` does on this thread, with its result.
        pub(crate) fn measure<R>(run: impl FnOnce() -> R) -> (R, Work) {
            let before = WORK.with(Cell::get);
            let out = run();
            let after = WORK.with(Cell::get);
            (
                out,
                Work {
                    widths: after.widths - before.widths,
                    visits: after.visits - before.visits,
                    compares: after.compares - before.compares,
                },
            )
        }
    }
}
