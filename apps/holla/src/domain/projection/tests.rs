use super::*;
use crate::domain::fixtures::world_for;
use crate::domain::usage::UsageStore;
use crate::scenario::{Motion, Scenario};

const HOST: &str = "mbp";
const CWD: &str = "/home/work/holla";
const NOW: i64 = 1_800_000_000;

fn store(uses: &[(&str, Option<&str>, &[i64])]) -> UsageStore {
    let mut s = UsageStore::default();
    for (item, path, stamps) in uses {
        for st in *stamps {
            s.used(item, *path, HOST, *st);
        }
    }
    s
}

fn buckets_at(usage: &UsageStore, ids: &[&str], t: i64) -> Vec<Bucket> {
    ids.iter()
        .map(|id| bucket(usage.frecency_any(id, CWD, HOST, t)))
        .collect()
}

/// Walks every second between `now` and a bound and finds the first second the
/// buckets differ, the slow reference the deadline search must equal.
fn first_change_by_scan(usage: &UsageStore, ids: &[&str], now: i64, bound: i64) -> Option<i64> {
    let base = buckets_at(usage, ids, now);
    (now + 1..=bound).find(|t| buckets_at(usage, ids, *t) != base)
}

#[test]
fn holla_projection_memory_deadline_is_the_first_bucket_change() {
    let usage = store(&[
        ("a", Some(CWD), &[NOW - 600, NOW - 300]),
        ("b", None, &[NOW - 3 * 86_400]),
        ("c", Some(CWD), &[NOW - 40 * 86_400]),
    ]);
    let ids = ["a", "b", "c"];
    let mut now = NOW;
    let mut seen = 0;
    while seen < 12 {
        let deadline = memory_deadline(&usage, CWD, HOST, now).expect("scores still decay");
        assert!(deadline > now, "deadline {deadline} is after {now}");
        let scanned = first_change_by_scan(&usage, &ids, now, deadline + 10).expect("a change");
        assert_eq!(deadline, scanned, "deadline from {now}");
        now = deadline;
        seen += 1;
    }
}

#[test]
fn holla_projection_memory_deadline_is_none_when_nothing_can_change() {
    assert_eq!(
        memory_deadline(&UsageStore::default(), CWD, HOST, NOW),
        None
    );
    let other_host = {
        let mut s = UsageStore::default();
        s.used("a", Some(CWD), "elsewhere", NOW - 5);
        s
    };
    assert_eq!(memory_deadline(&other_host, CWD, HOST, NOW), None);
    let ancient = store(&[("a", None, &[NOW - 20_000 * 86_400])]);
    assert_eq!(
        memory_deadline(&ancient, CWD, HOST, NOW),
        None,
        "a fully decayed score is already in its final bucket"
    );
}

#[test]
fn holla_projection_memory_deadline_is_next_second_while_a_stamp_is_in_the_future() {
    let usage = store(&[("a", Some(CWD), &[NOW - 600]), ("b", None, &[NOW + 5_000])]);
    assert_eq!(memory_deadline(&usage, CWD, HOST, NOW), Some(NOW + 1));
    assert!(
        memory_deadline(&usage, CWD, HOST, NOW + 5_000).is_some_and(|d| d > NOW + 5_000),
        "once the stamp is past, the monotone search takes over"
    );
}

#[test]
fn holla_projection_memory_deadline_survives_extreme_clocks() {
    let usage = store(&[("a", None, &[0, 10])]);
    for now in [i64::MAX - 5, i64::MAX, i64::MIN + 5, 0, -1] {
        let _ = memory_deadline(&usage, CWD, HOST, now);
    }
    assert_eq!(memory_deadline(&usage, CWD, HOST, i64::MAX), None);
}

#[test]
fn holla_projection_age_deadline_is_the_next_whole_day_or_the_future_stamp() {
    let day = 86_400;
    assert_eq!(
        age_deadline([NOW - 3 * day - 10], NOW),
        Some(NOW - 10 + day)
    );
    assert_eq!(age_deadline([NOW - 5 * day], NOW), Some(NOW + day));
    assert_eq!(age_deadline([NOW + 7], NOW), Some(NOW + 7));
    assert_eq!(
        age_deadline([NOW - 3 * day - 10, NOW - 100, NOW + 50], NOW),
        Some(NOW + 50)
    );
    assert_eq!(age_deadline(std::iter::empty(), NOW), None);
    assert_eq!(
        age_deadline([i64::MIN], NOW),
        None,
        "a stamp the arithmetic cannot represent never schedules"
    );
}

#[test]
fn holla_projection_deadline_is_due_on_expiry_and_on_a_rewound_clock() {
    let d = Deadline::new(100, Some(160));
    assert!(!d.due(100));
    assert!(!d.due(159));
    assert!(d.due(160));
    assert!(d.due(99), "the clock moved back past the computation");
    assert!(!Deadline::new(100, None).due(1_000_000));
}

#[test]
fn holla_projection_stamp_gate_publishes_only_after_installation() {
    let a = CatalogStamp { catalog: 1, fs: 0 };
    let b = CatalogStamp { catalog: 1, fs: 1 };
    let mut seen = Seen::default();
    assert!(
        seen.needs_rebuild(a),
        "nothing is published before the first install"
    );
    seen.published(a);
    assert!(!seen.needs_rebuild(a));
    assert!(seen.needs_rebuild(b));
}

type Mutation = (&'static str, fn(&mut World), [bool; 3], bool);
use crate::sim::world::World;

fn first_container(w: &World) -> String {
    w.docker.containers[0].name.clone()
}

/// Every concrete mutation endpoint named in the dependency table, with the
/// epochs and Fs revision it must move. `[catalog, cleanup, memory]`.
fn table() -> Vec<Mutation> {
    use crate::domain::effect::Effect;
    vec![
        (
            "docker stop effect",
            |w| {
                let n = first_container(w);
                w.apply_effect(&Effect::DockerStop(n));
            },
            [true, true, false],
            false,
        ),
        (
            "git fetch effect is a no-op",
            |w| w.apply_effect(&Effect::GitFetch("/x".into())),
            [false, false, false],
            false,
        ),
        (
            "edit_catalog",
            |w| w.edit_catalog(|w| w.sudo_cached = !w.sudo_cached),
            [true, false, false],
            false,
        ),
        (
            "edit_cleanup",
            |w| w.edit_cleanup(|w| w.sudo_cached = !w.sudo_cached),
            [true, true, false],
            false,
        ),
        (
            "edit_memory",
            |w| w.edit_memory(|w| w.memory.personalization = !w.memory.personalization),
            [false, false, true],
            false,
        ),
        (
            "record_use",
            |w| w.record_use("docker.cleanup"),
            [false, false, true],
            false,
        ),
        (
            "start_activity",
            |w| {
                let scope = crate::domain::context::ScopeTag::host("devbox");
                w.start_activity(
                    vec![],
                    crate::domain::activity::Script::unmodeled("true"),
                    "t",
                    "test",
                    crate::domain::activity::ActivityKind::Task,
                    scope,
                );
            },
            [true, false, false],
            false,
        ),
        (
            "stop_all with nothing live",
            |w| {
                w.stop_all();
            },
            [false, false, false],
            false,
        ),
        (
            "edit_activity output only",
            |w| {
                let id = w.activities[0].id.clone();
                w.edit_activity(&id, |a| {
                    a.output.push((
                        None,
                        "more".into(),
                        crate::domain::activity::LineTone::Muted,
                    ));
                });
            },
            [false, false, false],
            false,
        ),
        (
            "edit_activity state change",
            |w| {
                let id = w.activities[0].id.clone();
                w.edit_activity(&id, |a| a.name.push('!'));
            },
            [true, false, false],
            false,
        ),
        (
            "fs put",
            |w| {
                w.fs.file("/work/new.txt", 10, 1);
            },
            [false, false, false],
            true,
        ),
        (
            "fs touch to the same age is a no-op",
            |w| {
                let m = w.fs.get("/work/new.txt").map(|n| n.mtime);
                if let Some(m) = m {
                    let age = (crate::scenario::EPOCH_SECS - m) / 86_400;
                    w.fs.touch("/work/new.txt", age);
                }
            },
            [false, false, false],
            false,
        ),
        (
            "fs remove of a missing path fails without a bump",
            |w| {
                let _ = w.fs.remove_permanent("/no/such/path");
            },
            [false, false, false],
            false,
        ),
    ]
}

#[test]
fn holla_projection_each_mutation_invalidates_dependencies() {
    for (name, mutate, expect, fs_moves) in table() {
        let mut w = world_for(Scenario::DockerCleanup, Motion::Reduced);
        w.clock.running = true;
        if name.contains("fs touch") {
            w.fs.file("/work/new.txt", 10, 1);
        }
        if name.contains("edit_activity") {
            let scope = crate::domain::context::ScopeTag::host("devbox");
            w.start_activity(
                vec![],
                crate::domain::activity::Script::unmodeled("true"),
                "t",
                "test",
                crate::domain::activity::ActivityKind::Task,
                scope,
            );
        }
        let (catalog, cleanup, memory) = (
            w.catalog_stamp().catalog,
            w.cleanup_stamp().cleanup,
            w.memory_epoch(),
        );
        let fs = w.catalog_stamp().fs;
        mutate(&mut w);
        let moved = [
            w.catalog_stamp().catalog != catalog,
            w.cleanup_stamp().cleanup != cleanup,
            w.memory_epoch() != memory,
        ];
        assert_eq!(moved, expect, "{name}: [catalog, cleanup, memory]");
        assert_eq!(w.catalog_stamp().fs != fs, fs_moves, "{name}: fs revision");
        assert_eq!(
            w.cleanup_stamp().fs,
            w.catalog_stamp().fs,
            "{name}: both stamps read the same Fs revision"
        );
    }
}

#[test]
fn holla_projection_plain_ticks_move_no_epoch_but_a_source_completion_moves_catalog_once() {
    let mut w = world_for(Scenario::ParityDiscovery, Motion::Full);
    w.clock.running = true;
    assert!(w.discovering());
    let mut done_ticks: Vec<u64> = w
        .sources
        .iter()
        .map(|s| s.done_at)
        .filter(|t| *t > 0)
        .collect();
    done_ticks.sort();
    done_ticks.dedup();
    let mut bumps = vec![];
    for _ in 0..(done_ticks.last().copied().unwrap_or(0) + 50) {
        let before = (
            w.catalog_stamp(),
            w.cleanup_stamp().cleanup,
            w.memory_epoch(),
        );
        w.tick();
        let after = (
            w.catalog_stamp(),
            w.cleanup_stamp().cleanup,
            w.memory_epoch(),
        );
        assert_eq!(before.2, after.2);
        assert_eq!(before.1, after.1);
        if before.0 != after.0 {
            bumps.push(w.tick);
        }
    }
    assert_eq!(
        bumps, done_ticks,
        "the catalog epoch moves exactly once per source completion tick and never otherwise"
    );
}
