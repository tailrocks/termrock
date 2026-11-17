use super::*;
use crate::domain::fixtures::world_for;
use crate::domain::projection::{Work, work};
use crate::scenario::{Motion, Scenario};

fn world() -> World {
    let mut w = world_for(Scenario::DiskCleanup, Motion::Reduced);
    w.clock.running = true;
    w
}

fn measure(f: impl FnOnce()) -> Work {
    let before = work();
    f();
    work().since(before)
}

fn settle(page: &mut CleanupPage, w: &World) {
    page.ensure_projection(w);
    page.ensure_projection(w);
}

fn set_secs(w: &mut World, secs: i64) {
    w.clock.now_ms = (secs - w.clock.epoch_secs) * 1000 + 100;
}

fn candidate<'a>(
    page: &'a CleanupPage,
    path: &str,
) -> &'a crate::domain::catalog::InsightCandidate {
    page.cats
        .iter()
        .flat_map(|c| c.candidates.iter())
        .find(|k| k.path == path)
        .unwrap_or_else(|| panic!("{path} is a candidate"))
}

const REGISTRY: &str = "/Users/alex/.cargo/registry/cache";
const FUTURE: &str = "/Users/alex/work/android/.gradle";

#[test]
fn holla_projection_cleanup_idle_ticks_scan_nothing() {
    let w = world();
    let mut page = CleanupPage::new(None);
    let first = measure(|| settle(&mut page, &w));
    assert_eq!(first.candidate_scans, 1, "the first build scans once");
    assert_eq!(first.cleanup_row_builds, 1);
    assert_eq!(
        first.catalog_builds, 0,
        "the cleanup page never builds the finder catalog"
    );
    assert_eq!(
        measure(|| {
            for _ in 0..100 {
                settle(&mut page, &w);
            }
        }),
        Work::default()
    );
}

#[test]
fn holla_projection_cleanup_ages_refresh_at_day_boundaries_without_a_scan() {
    let mut w = world();
    let mut page = CleanupPage::new(None);
    settle(&mut page, &w);
    assert!(
        matches!(
            candidate(&page, REGISTRY).eligibility,
            Eligibility::Ineligible(_)
        ),
        "3 days is too recent for a 30 day rule"
    );
    assert_eq!(
        candidate(&page, FUTURE).age_days,
        None,
        "a future stamp has no age"
    );
    let before_checked = page.checked.clone();

    let mut refreshes = 0;
    for _ in 0..40 {
        let mtimes: Vec<i64> = page
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .filter_map(|k| w.fs.get(&k.path).map(|n| n.mtime))
            .collect();
        let deadline = crate::domain::projection::age_deadline(mtimes, w.now_secs())
            .expect("cached candidates keep ageing");
        set_secs(&mut w, deadline - 1);
        assert_eq!(
            measure(|| settle(&mut page, &w)),
            Work::default(),
            "one second before the boundary nothing changes and the page equals a full rebuild"
        );
        set_secs(&mut w, deadline);
        let delta = measure(|| {
            assert!(page.ensure_projection(&w), "the boundary second refreshes");
        });
        assert_eq!(
            delta.candidate_scans, 0,
            "ages refresh without a filesystem scan"
        );
        assert_eq!(delta.cleanup_row_builds, 1);
        assert_eq!(delta.catalog_builds, 0);
        assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
        refreshes += 1;
    }
    assert_eq!(refreshes, 40);
    assert_eq!(
        candidate(&page, REGISTRY).eligibility,
        Eligibility::Preselected,
        "past 30 days the registry cache becomes eligible"
    );
    assert!(
        !page.checked.contains(REGISTRY),
        "a path that becomes eligible later is not selected for the user"
    );
    assert_eq!(
        before_checked.difference(&page.checked).count(),
        0,
        "checked eligible paths survive the refreshes"
    );
    assert!(
        candidate(&page, FUTURE).age_days.is_some(),
        "the future stamp became the present"
    );
}

#[test]
fn holla_projection_cleanup_clock_rewind_matches_a_full_rebuild() {
    let mut w = world();
    let now = w.now_secs();
    set_secs(&mut w, now + 60 * 86_400);
    let mut page = CleanupPage::new(None);
    settle(&mut page, &w);
    assert_eq!(
        candidate(&page, REGISTRY).eligibility,
        Eligibility::Preselected
    );
    set_secs(&mut w, now);
    let delta = measure(|| settle(&mut page, &w));
    assert_eq!(delta.candidate_scans, 0);
    assert!(matches!(
        candidate(&page, REGISTRY).eligibility,
        Eligibility::Ineligible(_)
    ));
    assert!(
        !page.checked.contains(REGISTRY) || page.cats[0].candidates[0].eligibility.selectable()
    );
}

#[test]
fn holla_projection_cleanup_source_change_rescans_once_and_keeps_checks_and_folds() {
    let mut w = world();
    let mut page = CleanupPage::new(None);
    settle(&mut page, &w);
    let logs = "/Users/alex/Library/Logs/c";
    assert!(
        page.checked.contains(logs),
        "preselected at the first build"
    );
    let user_caches = page
        .cats
        .iter()
        .position(|c| c.category.id == "user.caches")
        .expect("user.caches is detected");
    page.set_open(user_caches, true);
    page.rebuild_rows();
    page.checked
        .remove("/Users/alex/Library/Logs/DiagnosticReports");

    w.fs.dir("/Users/alex/Library/Logs/fresh", 30);
    w.fs.remove_permanent(REGISTRY).expect("removed");
    w.fs.dir("/Users/alex/Library/Developer/Xcode/DerivedData", 30);
    let delta = measure(|| {
        assert!(page.ensure_projection(&w));
    });
    assert_eq!(
        delta.candidate_scans, 1,
        "one rescan for both filesystem edits"
    );
    assert_eq!(delta.cleanup_row_builds, 1);
    assert_eq!(delta.catalog_builds, 0);
    assert!(
        page.checked.contains(logs),
        "an eligible checked path survives"
    );
    assert!(
        !page
            .checked
            .contains("/Users/alex/Library/Logs/DiagnosticReports"),
        "an unchecked path is not re-preselected"
    );
    assert!(
        !page.checked.contains("/Users/alex/Library/Logs/fresh"),
        "a newly eligible path is not preselected after the first seed"
    );
    assert!(
        page.cats
            .iter()
            .find(|c| c.category.id == "cargo.registry-cache")
            .is_some_and(|c| c.candidates.is_empty()),
        "the removed candidate is gone"
    );
    let moved = page
        .cats
        .iter()
        .position(|c| c.category.id == "user.caches")
        .expect("still detected");
    assert!(
        moved > user_caches,
        "the category moved down when one appeared before it"
    );
    assert!(
        page.is_open(moved),
        "the fold followed its category id, not its position"
    );
    assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
}

#[test]
fn holla_projection_cleanup_failed_fs_operations_keep_the_projection() {
    let mut w = world();
    let mut page = CleanupPage::new(None);
    settle(&mut page, &w);
    let _ = w.fs.remove_permanent("/no/such/path");
    w.fs.touch("/no/such/path", 3);
    assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
}

#[test]
fn holla_projection_cleanup_execution_moves_the_stamps_and_both_pages_follow() {
    let mut w = world();
    let mut cleanup = CleanupPage::new(None);
    let mut finder = crate::screens::finder::FinderPage::new(None);
    settle(&mut cleanup, &w);
    finder.settle_projection_for_test(&w);
    let plan = cleanup.build_plan(&w);
    assert!(
        !plan.items.is_empty(),
        "the preselected candidates form a plan"
    );

    let before = (w.cleanup_stamp(), w.catalog_stamp());
    let index = commit(&plan, &mut w).expect("the reviewed plan starts");
    assert_ne!(
        w.cleanup_stamp(),
        before.0,
        "starting the job is a cleanup edit"
    );
    assert_ne!(w.catalog_stamp(), before.1);
    assert_eq!(w.reports.len(), index + 1);

    let mut steps = 0;
    while w.cleanup_job.is_some() {
        let at = w.cleanup_stamp();
        step(&mut w);
        assert_ne!(
            w.cleanup_stamp(),
            at,
            "every step of a running job is one edit"
        );
        settle(&mut cleanup, &w);
        finder.settle_projection_for_test(&w);
        steps += 1;
        assert!(steps < 500, "the job finishes");
    }
    assert!(steps >= 1);
    let after = measure(|| settle(&mut cleanup, &w));
    assert_eq!(after, Work::default(), "settled after the job");
    let removed = plan
        .items
        .iter()
        .all(|i| !w.fs.exists(&i.path) || w.fs.get(&i.path).is_none());
    assert!(removed, "the plan's paths left the tree");
    assert!(
        cleanup
            .cats
            .iter()
            .flat_map(|c| c.candidates.iter())
            .all(|k| !plan.items.iter().any(|i| i.path == k.path)),
        "the cleanup page rescanned after the removals"
    );
}
