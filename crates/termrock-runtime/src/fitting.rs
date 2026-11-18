//! Shared pieces of the responsive fitters (chips, breadcrumb, segments, status
//! bar, key hints, toolbar): the order candidates are removed in. Fitting work
//! is counted through `record_work!` (`FitVisits`, `FitCompares`), so a test
//! can prove a fit is linear in the number of items.

/// Candidate indices in removal order, lowest key first.
///
/// Keys are unique, so the unstable sort is deterministic. One sort replaces a
/// scan of every survivor per removal.
#[doc(hidden)]
pub fn removal_order(keys: &[(u8, u8, usize)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_unstable_by(|&a, &b| {
        termrock_metrics::record_work!(FitCompares, 1usize);
        keys[a].cmp(&keys[b])
    });
    order
}
