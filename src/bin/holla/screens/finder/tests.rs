use super::*;
use crate::domain::fixtures::world_for;
use crate::domain::projection::{Work, work};
use crate::scenario::{Motion, Scenario};

fn world(scenario: Scenario) -> World {
    let mut w = world_for(scenario, Motion::Reduced);
    w.clock.running = true;
    w
}

fn measure(f: impl FnOnce()) -> Work {
    let before = work();
    f();
    work().since(before)
}

/// Builds the page and runs the settled path twice, so the second call runs the
/// differential check against a page built from nothing.
fn settle(page: &mut FinderPage, w: &World) {
    page.ensure_projection(w);
    page.ensure_projection(w);
}

fn set_query(page: &mut FinderPage, text: &str) {
    page.query.buffer.set_text(text);
}

#[test]
fn holla_projection_query_reuses_source_catalog() {
    let w = world(Scenario::RustDirty);
    let mut page = FinderPage::new(None);
    let first = measure(|| settle(&mut page, &w));
    assert_eq!(
        first.catalog_builds, 1,
        "the first build constructs the catalog once"
    );
    assert_eq!(first.decorations, 1);
    assert!(
        first.candidate_scans >= 1,
        "the catalog embeds the cleanup scan"
    );

    for (name, edit) in [
        (
            "query",
            Box::new(|p: &mut FinderPage| set_query(p, "git")) as Box<dyn Fn(&mut FinderPage)>,
        ),
        ("longer query", Box::new(|p| set_query(p, "git pull"))),
        ("scope", Box::new(|p| p.set_scope(Scope::Parent))),
        ("scope system", Box::new(|p| p.set_scope(Scope::System))),
        ("cleared query", Box::new(|p| set_query(p, ""))),
    ] {
        edit(&mut page);
        let delta = measure(|| {
            assert!(page.ensure_projection(&w), "{name} reranks");
        });
        assert_eq!(delta.catalog_builds, 0, "{name}: no provider construction");
        assert_eq!(delta.candidate_scans, 0, "{name}: no filesystem scan");
        assert_eq!(delta.decorations, 0, "{name}: no memory decoration");
        assert!(delta.ranks >= 1, "{name}: reranked");
        let settled = measure(|| settle(&mut page, &w));
        assert_eq!(settled, Work::default(), "{name}: settled afterwards");
    }
}

#[test]
fn holla_projection_group_change_reranks_without_source_work() {
    let w = world(Scenario::RustDirty);
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);
    page.group = Some("Git");
    let delta = measure(|| {
        assert!(page.ensure_projection(&w));
    });
    assert_eq!(delta.catalog_builds, 0);
    assert_eq!(delta.candidate_scans, 0);
    assert!(delta.ranks >= 1);
}

#[test]
fn holla_projection_memory_change_decorates_the_cached_catalog_without_rebuilding_it() {
    let mut w = world(Scenario::RustDirty);
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);
    w.record_use("git.pull");
    let delta = measure(|| {
        assert!(page.ensure_projection(&w));
    });
    assert_eq!(delta.catalog_builds, 0);
    assert_eq!(delta.candidate_scans, 0);
    assert_eq!(delta.decorations, 1);
    assert!(delta.ranks >= 1);
    assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
}

#[test]
fn holla_projection_an_empty_catalog_is_a_published_projection() {
    let mut w = world(Scenario::FirstUse);
    w.tools.clear();
    w.edit_catalog(|_| {});
    let mut page = FinderPage::new(Some("No such group"));
    let first = measure(|| settle(&mut page, &w));
    assert_eq!(first.catalog_builds, 1);
    assert_eq!(
        measure(|| settle(&mut page, &w)),
        Work::default(),
        "an empty result is not rebuilt on every call"
    );
}

#[test]
fn holla_projection_temporal_deadlines_match_full_rebuild() {
    let mut w = world(Scenario::RustDirty);
    w.clock.running = true;
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);
    let ticks_per_s = 1000 / crate::sim::world::TICK_MS as u64;
    let base = w.tick;
    let observed: Vec<String> = page
        .source
        .iter()
        .filter(|i| i.freshness.observed_tick().is_some())
        .map(|i| i.id.clone())
        .collect();
    assert!(!observed.is_empty(), "the fixture has observed sources");
    let fixed = page
        .source
        .iter()
        .find(|i| i.id == "explore.git")
        .expect("an always-current item")
        .id
        .clone();
    assert_eq!(
        page.source
            .iter()
            .find(|i| i.id == fixed)
            .and_then(|i| i.freshness.observed_tick()),
        None
    );

    // age 59.92 s, exactly 60 s, 59 min 59.92 s and 1 h for a source observed at the build tick
    for ticks in [
        0,
        58 * ticks_per_s,
        59 * ticks_per_s,
        749,
        750,
        751,
        45_000 - 1,
        45_000,
        45_001,
    ] {
        w.tick = base + ticks;
        let delta = measure(|| settle(&mut page, &w));
        assert_eq!(
            delta,
            Work::default(),
            "tick +{ticks}: time alone rebuilds nothing"
        );
        let reference = crate::domain::catalog::build(&w);
        for id in &observed {
            let cached = page.source.iter().find(|i| &i.id == id).expect("cached");
            let fresh = reference.iter().find(|i| &i.id == id).expect("reference");
            assert_eq!(
                page.live_item(cached, &w).freshness.label(),
                fresh.freshness.label(),
                "{id} at tick +{ticks}"
            );
        }
        let cached = page.source.iter().find(|i| i.id == fixed).expect("cached");
        assert_eq!(
            page.live_item(cached, &w).freshness.label(),
            "live · 0 s ago"
        );
    }
}

#[test]
fn holla_projection_future_usage_and_clock_rewind_match_reference() {
    let mut w = world(Scenario::RustDirty);
    let (cwd, host) = (w.location.cwd.clone(), w.host.name.clone());
    let now = w.now_secs();
    w.memory
        .usage
        .used("git.pull", Some(&cwd), &host, now + 3_000);
    w.memory
        .usage
        .used("mise.task.lint", Some(&cwd), &host, now - 600);
    w.edit_memory(|_| {});
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);

    for step in 0..4 {
        w.clock.now_ms += 1_000;
        let delta = measure(|| settle(&mut page, &w));
        assert_eq!(delta.catalog_builds, 0, "step {step}");
        assert_eq!(delta.candidate_scans, 0, "step {step}");
        assert_eq!(
            delta.decorations, 1,
            "step {step}: a future stamp refreshes the decoration every second"
        );
    }
    w.clock.now_ms += 4_000_000;
    let past = measure(|| settle(&mut page, &w));
    assert_eq!(past.catalog_builds, 0);

    w.clock.now_ms -= 2_000_000;
    let rewound = measure(|| settle(&mut page, &w));
    assert_eq!(
        rewound.catalog_builds, 0,
        "a rewound clock never rebuilds sources"
    );
    assert_eq!(
        rewound.decorations, 1,
        "a rewound clock refreshes the decoration once"
    );
    assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
}

#[test]
fn holla_projection_recent_ranking_follows_the_frecency_deadlines() {
    let mut w = world(Scenario::RustDirty);
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);
    let (cwd, host) = (w.location.cwd.clone(), w.host.name.clone());
    let mut decorated = 0;
    for _ in 0..14 {
        let now = w.now_secs();
        let Some(deadline) =
            crate::domain::projection::memory_deadline(&w.memory.usage, &cwd, &host, now)
        else {
            break;
        };
        w.clock.now_ms = (deadline - 1 - w.clock.epoch_secs) * 1000 + 100;
        assert_eq!(
            measure(|| settle(&mut page, &w)),
            Work::default(),
            "one second before the deadline nothing is rebuilt and the page still equals a full rebuild"
        );
        w.clock.now_ms = (deadline - w.clock.epoch_secs) * 1000 + 100;
        let delta = measure(|| {
            assert!(page.ensure_projection(&w), "the deadline second rebuilds");
        });
        assert_eq!(delta.catalog_builds, 0);
        assert_eq!(delta.candidate_scans, 0);
        assert_eq!(delta.decorations, 1);
        assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
        decorated += 1;
    }
    assert!(
        decorated >= 4,
        "the seeded memory decays through several buckets"
    );
}

#[test]
fn holla_projection_homebrew_cache_window_matches_full_rebuild() {
    let mut w = world(Scenario::ParityBrewServices);
    let now = w.now_secs();
    w.edit_catalog(|w| {
        let cache = w
            .brew
            .as_mut()
            .and_then(|b| b.cache.as_mut())
            .expect("the fixture has a Homebrew cache");
        cache.fetched_at = now - 295;
    });
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);
    assert!(
        crate::domain::catalog::source_deadline_secs(&w).is_some(),
        "a fresh cache ages every second"
    );
    let mut rebuilds = 0;
    for _ in 0..9 {
        w.clock.now_ms += 1_000;
        let delta = measure(|| settle(&mut page, &w));
        rebuilds += delta.catalog_builds;
    }
    assert_eq!(
        rebuilds, 6,
        "one rebuild per second until the cache expires at 301 s old, none after"
    );
    assert_eq!(crate::domain::catalog::source_deadline_secs(&w), None);
    w.clock.now_ms += 3_600_000;
    let jump = measure(|| settle(&mut page, &w));
    assert_eq!(
        jump.catalog_builds, 0,
        "an expired cache is a settled source"
    );
    assert_eq!(measure(|| settle(&mut page, &w)), Work::default());
}

#[test]
fn finder_reselecting_the_current_row_keeps_preview_scroll() {
    let w = world(Scenario::RustDirty);
    let mut page = FinderPage::new(None);
    settle(&mut page, &w);
    let first = page.first_selectable();
    let other = (first + 1..page.rows.len())
        .find(|&i| selectable(&page.rows[i]))
        .expect("a second selectable row");
    page.select_row(first);
    page.preview_scroll.set_content(100);
    page.preview_scroll.set_viewport(5);
    page.preview_scroll.scroll_by(3);
    assert_eq!(page.preview_scroll.offset, 3);

    page.select_row(first);
    assert_eq!(
        page.preview_scroll.offset, 3,
        "the previewed item did not change"
    );

    page.select_row(other);
    assert_ne!(page.current_id(), None);
    assert_eq!(
        page.preview_scroll.offset, 0,
        "a new item starts at the top"
    );
}

mod selected_row_keeps_preview_scroll {
    use super::*;
    use crate::screens::pointer_tests::{cell_of, render_hits};
    use junie_tui::core::focus::{Focus, FocusRing};
    use junie_tui::core::hit::HitRegistry;
    use junie_tui::shell::Request;
    use ratatui::crossterm::event::KeyModifiers;

    fn settled_page(w: &World) -> FinderPage {
        let mut page = FinderPage::new(None);
        settle(&mut page, w);
        page
    }

    fn item_rows(page: &FinderPage) -> Vec<usize> {
        (0..page.rows.len())
            .filter(|&i| matches!(page.rows[i], Row::Item(..)))
            .collect()
    }

    fn scroll_preview_to_three(page: &mut FinderPage) {
        page.preview_scroll.set_content(100);
        page.preview_scroll.set_viewport(5);
        page.preview_scroll.jump_start();
        page.preview_scroll.scroll_by(3);
        assert_eq!(page.preview_scroll.offset, 3);
    }

    fn secondary_click(
        page: &mut FinderPage,
        w: &mut World,
        hits: &HitRegistry,
        row: usize,
    ) -> bool {
        let pos = cell_of(hits, FINDER.child(row));
        let id = hits.hit(pos).expect("a rendered row is under the pointer");
        let event = PointerEvent::new(hits, Phase::Secondary, id, pos, 1, KeyModifiers::NONE);
        let mut focus = Focus::default();
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        page.pointer(&event, w, &mut cx);
        cx.requests
            .iter()
            .any(|r| matches!(r, Request::Go(Go::Alternatives { .. })))
    }

    fn press(page: &mut FinderPage, w: &mut World, code: KeyCode) {
        let mut focus = Focus::default();
        focus.focus(FINDER);
        let ring = FocusRing::default();
        let mut cx = Cx::new(&mut focus, &ring);
        let key = Key {
            code,
            mods: KeyModifiers::NONE,
        };
        page.on_key(&key, w, &mut cx);
    }

    #[test]
    fn secondary_click_on_the_selected_row_keeps_preview_scroll() {
        let mut w = world(Scenario::RustDirty);
        let mut page = settled_page(&w);
        let rows = item_rows(&page);
        let (selected, other) = (rows[0], rows[1]);
        page.select_row(selected);
        let hits = render_hits(&mut page, &w);
        scroll_preview_to_three(&mut page);

        assert!(
            secondary_click(&mut page, &mut w, &hits, selected),
            "the menu still opens"
        );
        assert_eq!(page.cursor, selected);
        assert_eq!(
            page.preview_scroll.offset, 3,
            "the previewed item did not change"
        );

        assert!(secondary_click(&mut page, &mut w, &hits, other));
        assert_eq!(page.cursor, other);
        assert_eq!(
            page.preview_scroll.offset, 0,
            "a secondary click on a new row starts its preview at the top"
        );
    }

    #[test]
    fn home_and_end_on_the_selected_row_keep_preview_scroll() {
        let mut w = world(Scenario::RustDirty);
        let mut page = settled_page(&w);
        let first = page.first_selectable();
        let last = page.rows.iter().rposition(selectable).expect("a last row");
        assert_ne!(first, last, "the fixture needs two selectable rows");

        page.select_row(first);
        scroll_preview_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::Home);
        assert_eq!(page.cursor, first);
        assert_eq!(page.preview_scroll.offset, 3, "Home on the first row");

        page.select_row(last);
        scroll_preview_to_three(&mut page);
        press(&mut page, &mut w, KeyCode::End);
        assert_eq!(page.cursor, last);
        assert_eq!(page.preview_scroll.offset, 3, "End on the last row");

        press(&mut page, &mut w, KeyCode::Home);
        assert_eq!(page.cursor, first);
        assert_eq!(page.preview_scroll.offset, 0, "Home onto a new row resets");
    }
}
