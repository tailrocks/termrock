//! Holla-local projection revisions for the Finder and Cleanup pages.
//!
//! The World owns three private source epochs. A mutation endpoint that really
//! changes catalog, cleanup or memory input bumps its epoch once; a page keeps
//! the stamp of the last projection it installed and rebuilds only when the
//! stamp moved or a time deadline passed. Nothing here compares output models.
//!
//! Dependency table (what bumps what):
//!
//! | Mutation | catalog | cleanup | memory |
//! | --- | --- | --- | --- |
//! | discovery source Loading to Done/Failed | yes | no | no |
//! | `apply_effect`, plan effects, `remove_candidate`, cleanup commit/step/settle | yes | yes | no |
//! | activity start, stop, state or identity change | yes | no | no |
//! | activity output or stdin only, clock tick, pointer, scroll | no | no | no |
//! | trust, custom config reload, `edit_catalog`, `edit_catalog_if` when it reports a change | yes | no | no |
//! | `edit_cleanup` | yes | yes | no |
//! | memory use, query learning, pin, alias, hide, restore, reset, save | no | no | yes |
//! | `Fs` node mutation | the `fs` half of both stamps | | |
//!
//! Cleanup also bumps the catalog epoch because the catalog embeds cleanup
//! candidate totals.
//!
//! #64 mapping: `CatalogStamp{catalog,fs}` and `CleanupStamp` map to
//! `SourceStamp.data`, `ViewKey` to `view`, and the page instance to `instance`.
//! Holla supplies a ready projection; #64 owns the keyed identity.

use std::cell::Cell;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectionEpochs {
    catalog: u64,
    cleanup: u64,
    memory: u64,
}

impl ProjectionEpochs {
    pub fn catalog(&self) -> u64 {
        self.catalog
    }

    pub fn cleanup(&self) -> u64 {
        self.cleanup
    }

    pub fn memory(&self) -> u64 {
        self.memory
    }

    pub(crate) fn bump_catalog(&mut self) {
        self.catalog = self.catalog.wrapping_add(1);
    }

    pub(crate) fn bump_cleanup(&mut self) {
        self.cleanup = self.cleanup.wrapping_add(1);
        self.bump_catalog();
    }

    pub(crate) fn bump_memory(&mut self) {
        self.memory = self.memory.wrapping_add(1);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogStamp {
    pub catalog: u64,
    pub fs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupStamp {
    pub cleanup: u64,
    pub fs: u64,
}

/// The stamp of the last projection a page installed completely. An empty
/// result is still a published projection, so this is an `Option`, never
/// `rows.is_empty()`.
#[derive(Debug, Clone, Copy)]
pub struct Seen<S> {
    seen: Option<S>,
}

impl<S> Default for Seen<S> {
    fn default() -> Self {
        Self { seen: None }
    }
}

impl<S: Copy + PartialEq> Seen<S> {
    pub fn needs_rebuild(&self, current: S) -> bool {
        self.seen != Some(current)
    }

    pub fn published(&mut self, current: S) {
        self.seen = Some(current);
    }
}

/// A time-driven refresh point: the first clock second at which a cached
/// projection can differ from a full rebuild. The clock moving backwards past
/// the moment it was computed also invalidates it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Deadline {
    computed_at: i64,
    at: Option<i64>,
}

impl Deadline {
    pub fn new(computed_at: i64, at: Option<i64>) -> Self {
        Self { computed_at, at }
    }

    pub fn due(&self, now: i64) -> bool {
        now < self.computed_at || self.at.is_some_and(|d| now >= d)
    }
}

/// Actual projection work done on this thread. Settled ticks must leave every
/// field unchanged. Tests read deltas; nothing here is a timer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work {
    pub catalog_builds: u64,
    pub decorations: u64,
    pub ranks: u64,
    pub candidate_scans: u64,
    pub cleanup_row_builds: u64,
}

thread_local! {
    static WORK: Cell<Work> = const { Cell::new(Work {
        catalog_builds: 0,
        decorations: 0,
        ranks: 0,
        candidate_scans: 0,
        cleanup_row_builds: 0,
    }) };
    static UNCOUNTED: Cell<u32> = const { Cell::new(0) };
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "read by the work-budget tests and the #56 harness"
    )
)]
pub fn work() -> Work {
    WORK.with(Cell::get)
}

/// Whether work counts now, that is, no reference computation is running. The
/// #56 metrics follow the same switch as [`record`].
#[cfg_attr(
    not(feature = "work-metrics"),
    allow(dead_code, reason = "read where record_work! is enabled")
)]
pub(crate) fn counting() -> bool {
    UNCOUNTED.with(Cell::get) == 0
}

pub(crate) fn record(f: impl FnOnce(&mut Work)) {
    if UNCOUNTED.with(Cell::get) > 0 {
        return;
    }
    WORK.with(|w| {
        let mut v = w.get();
        f(&mut v);
        w.set(v);
    });
}

/// Runs a reference computation that must not count as projection work, such
/// as the full rebuild a differential check compares against.
#[cfg(test)]
pub(crate) fn uncounted<R>(f: impl FnOnce() -> R) -> R {
    UNCOUNTED.with(|c| c.set(c.get() + 1));
    let out = f();
    UNCOUNTED.with(|c| c.set(c.get() - 1));
    out
}

impl Work {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "read by the work-budget tests and the #56 harness"
        )
    )]
    pub fn since(self, before: Work) -> Work {
        Work {
            catalog_builds: self.catalog_builds - before.catalog_builds,
            decorations: self.decorations - before.decorations,
            ranks: self.ranks - before.ranks,
            candidate_scans: self.candidate_scans - before.candidate_scans,
            cleanup_row_builds: self.cleanup_row_builds - before.cleanup_row_builds,
        }
    }
}

/// The comparable part of one memory-decorated item: everything the ranking
/// reads from a frecency score, quantized exactly as the ranking quantizes it.
type Bucket = (bool, bool, i64);

fn bucket(f: f64) -> Bucket {
    (
        f > 0.0,
        f > crate::domain::usage::RECENT_THRESHOLD,
        crate::domain::ranking::frecency_boost(f),
    )
}

/// The first second after `now` at which the memory decoration or ranking of
/// `usage` can differ from the one computed at `now`.
///
/// While every relevant stamp is in the past, each frecency score decays
/// monotonically, so the quantized buckets change at most once per decrease and
/// never return. The deadline doubles a positive horizon until a bucket
/// changes, then bisects to the first changed second. A stamp in the future
/// clamps to age zero and the scores of other stamps still decay, so the
/// deadline is the next second until every stamp is in the past.
pub fn memory_deadline(
    usage: &crate::domain::usage::UsageStore,
    cwd: &str,
    host: &str,
    now: i64,
) -> Option<i64> {
    let relevant = |u: &&crate::domain::usage::ActionUse| {
        u.host == host && (u.path.is_none() || u.path.as_deref() == Some(cwd))
    };
    let mut ids: Vec<&str> = usage
        .actions
        .iter()
        .filter(relevant)
        .map(|u| u.item.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return None;
    }
    if usage
        .actions
        .iter()
        .filter(relevant)
        .any(|u| u.stamps.iter().any(|s| *s > now))
    {
        return Some(now.saturating_add(1));
    }
    let at = |t: i64| -> Vec<Bucket> {
        ids.iter()
            .map(|id| bucket(usage.frecency_any(id, cwd, host, t)))
            .collect()
    };
    let now_buckets = at(now);
    if now_buckets.iter().all(|b| *b == (false, false, 0)) {
        return None;
    }
    let mut lo = now;
    let mut step: i64 = 1;
    let hi = loop {
        let t = lo.saturating_add(step);
        if at(t) != now_buckets {
            break t;
        }
        if t == i64::MAX {
            return None;
        }
        lo = t;
        step = step.saturating_mul(2);
    };
    let (mut lo, mut hi) = (lo, hi);
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if at(mid) != now_buckets {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// The first second after `now` at which any file's whole-day age changes, or
/// at which a future modification time becomes the present.
pub fn age_deadline(mtimes: impl IntoIterator<Item = i64>, now: i64) -> Option<i64> {
    mtimes
        .into_iter()
        .filter_map(|m| {
            if m > now {
                Some(m)
            } else {
                let days = now.saturating_sub(m) / 86_400;
                days.checked_add(1)?
                    .checked_mul(86_400)
                    .and_then(|d| m.checked_add(d))
            }
        })
        .min()
}

/// Asserts two projection signatures match and, when they do not, names the
/// first lines that differ instead of dumping both models.
#[cfg(test)]
pub(crate) fn assert_same_signature(cached: &[String], reference: &[String], what: &str) {
    if cached == reference {
        return;
    }
    let clip = |s: &str| s.chars().take(900).collect::<String>();
    let mut first = vec![];
    for i in 0..cached.len().max(reference.len()) {
        let (a, b) = (cached.get(i), reference.get(i));
        if a != b {
            first.push(format!(
                "line {i}\n  cached:    {}\n  reference: {}",
                a.map_or("<none>".into(), |s| clip(s)),
                b.map_or("<none>".into(), |s| clip(s))
            ));
            if first.len() == 3 {
                break;
            }
        }
    }
    panic!(
        "the {what} projection went stale: a world mutation did not move its epoch or deadline\n{}",
        first.join("\n")
    );
}

#[cfg(test)]
mod tests;
