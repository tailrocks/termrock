use super::*;
use junie_tui::core::event::Input;

const ID: &str = "docker-cleanup";

fn app() -> App {
    let mut app = App::launch(
        Scenario::DockerCleanup,
        Motion::Full,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    app
}

fn key(code: KeyCode, mods: KeyModifiers) -> Input {
    Input::Key(Key { code, mods })
}

fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.handle_input(Input::Tick);
    }
}

fn arm(app: &mut App, id: &str) {
    app.world.plan_mut(id).expect("plan").drift = None;
    app.pending_gate = Some(GateTarget::Plan(id.to_owned()));
    app.gate2_confirmed();
}

fn step(app: &App, id: &str, step: &str) -> StepState {
    app.world
        .plan(id)
        .expect("plan")
        .steps
        .iter()
        .find(|s| s.id == step)
        .expect("step")
        .state
}

fn tick_until_running(app: &mut App, id: &str, step_id: &str) {
    for _ in 0..400 {
        if step(app, id, step_id) == StepState::Running {
            return;
        }
        tick(app, 1);
    }
    panic!(
        "{step_id} never started: {:?} tick={} status={}",
        app.world
            .plan(id)
            .map(|p| p.steps.iter().map(|s| s.state).collect::<Vec<_>>()),
        app.world.tick,
        status(app)
    );
}

fn plan_tab(app: &App, id: &str) -> Option<usize> {
    app.tabs
        .iter()
        .position(|t| matches!(&t.kind, TabKind::Plan(p) if p == id))
}

fn confirm_close_dialog(app: &mut App) {
    assert!(!app.modals.is_empty(), "the close dialog is open");
    app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty(), "the dialog resolved");
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn live_steps(app: &App, id: &str) -> usize {
    app.world
        .plan(id)
        .expect("plan")
        .steps
        .iter()
        .filter(|s| {
            s.included
                && matches!(
                    s.state,
                    StepState::Ready | StepState::Waiting | StepState::Running
                )
        })
        .count()
}

#[test]
fn plan_stop_and_close_finalizes_the_running_plan() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let tab = plan_tab(&app, ID).expect("plan tab open");
    app.activate_tab(tab);

    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    confirm_close_dialog(&mut app);

    let plan = app.world.plan(ID).expect("plan");
    assert_eq!(plan.phase, PlanPhase::Done);
    assert!(plan.ended_tick.is_some());
    assert_eq!(
        live_steps(&app, ID),
        0,
        "nothing stays running in a Done plan"
    );
    assert_eq!(step(&app, ID, "rm"), StepState::Cancelled);
    assert!(plan_tab(&app, ID).is_none(), "the tab closed");
    assert!(
        status(&app).starts_with("Clean Docker completely stopped · "),
        "{}",
        status(&app)
    );
    assert!(status(&app).contains("cancelled"), "{}", status(&app));

    let snapshot = app.world.plan(ID).cloned();
    tick(&mut app, 120);
    assert_eq!(
        app.world.plan(ID).cloned(),
        snapshot,
        "ticks change nothing"
    );
}

#[test]
fn plan_stop_and_close_uses_captured_plan_id() {
    let mut app = app();
    let mut other = app.world.plan(ID).cloned().expect("plan");
    other.id = "other-plan".into();
    other.title = "Other plan".into();
    app.world.plans.push(other);
    arm(&mut app, ID);
    arm(&mut app, "other-plan");
    tick_until_running(&mut app, ID, "stop");
    tick_until_running(&mut app, "other-plan", "stop");

    let first = plan_tab(&app, ID).expect("first tab");
    app.activate_tab(first);
    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    let second = plan_tab(&app, "other-plan").expect("second tab");
    app.activate_tab(second);
    app.shell
        .focus
        .set(Some(WidgetId::of("close-tab").sub("ok")));
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty(), "the dialog resolved");

    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Done);
    assert!(plan_tab(&app, ID).is_none());
    let other = app.world.plan("other-plan").expect("other plan");
    assert_eq!(other.phase, PlanPhase::Running, "the other plan is intact");
    assert_eq!(step(&app, "other-plan", "stop"), StepState::Running);
    assert!(plan_tab(&app, "other-plan").is_some());
}

#[test]
fn plan_stop_dialog_dismissal_keeps_the_plan_running() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let tab = plan_tab(&app, ID).expect("plan tab open");
    app.activate_tab(tab);

    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert!(!app.modals.is_empty());
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));

    assert!(app.modals.is_empty());
    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Running);
    assert_eq!(step(&app, ID, "rm"), StepState::Running);
    assert!(plan_tab(&app, ID).is_some());
}

const FINAL_NAME: &str = "zz-final-hit.txt";
const FINAL_QUERY: &str = "zz-final-hit";

fn files_app() -> App {
    let mut app = App::launch(
        Scenario::ParityFiles,
        Motion::Full,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    app
}

fn set_index_source(app: &mut App, done_at: u64, failure: Option<&str>) {
    let index = app
        .world
        .sources
        .iter_mut()
        .find(|s| s.name == "index")
        .expect("index source");
    index.done_at = done_at;
    index.failure = failure.map(str::to_owned);
}

fn add_final_file(app: &mut App) -> String {
    let path = format!("{}/{FINAL_NAME}", app.world.location.home);
    app.world.fs.text(&path, "final\n", 1);
    path
}

fn open_find(app: &mut App) {
    app.activate_tab(0);
    app.push_page(Page::Find);
}

fn type_query(app: &mut App, query: &str) {
    for c in query.chars() {
        app.handle_input(key(KeyCode::Char(c), KeyModifiers::NONE));
    }
}

fn find_page(app: &mut App) -> &mut crate::screens::files::FilesPage {
    let active = app.active();
    app.router
        .top_of_mut(active)
        .and_then(|s| s.as_files())
        .expect("files page on top")
}

fn hits(app: &mut App) -> Vec<String> {
    find_page(app).hit_paths()
}

fn published(app: &mut App) -> Option<(usize, bool)> {
    find_page(app).published_index()
}

fn selected(app: &mut App) -> Option<String> {
    find_page(app).selected_hit()
}

fn install_index(app: &mut App, index: Vec<String>) {
    find_page(app).install_index_for_test(index);
}

fn find_page_is_top(app: &mut App) -> bool {
    find_page(app).is_find_mode()
}

fn call_counts() -> (u64, u64) {
    crate::sim::fs::call_counts()
}

fn advance_world_only(app: &mut App, n: usize) {
    for _ in 0..n {
        drop(app.world.tick());
    }
}

#[test]
fn files_find_publishes_last_entry_on_completion_tick() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 9);
    assert!(
        hits(&mut app).is_empty(),
        "the last entry is not revealed yet"
    );

    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path.clone()]);
    assert_eq!(selected(&mut app), Some(path));
    assert_eq!(published(&mut app).map(|(_, done)| done), Some(true));
}

#[test]
fn files_find_skipped_tick_still_publishes_completion() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    advance_world_only(&mut app, 10);
    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_terminal_progress_is_published_once() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);
    tick(&mut app, 12);
    let stamp = published(&mut app);
    let calls = call_counts();
    assert_eq!(hits(&mut app), vec![path.clone()]);

    tick(&mut app, 100);
    assert_eq!(published(&mut app), stamp, "terminal stamp is stable");
    assert_eq!(
        call_counts(),
        calls,
        "no find or index work after publication"
    );
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_failed_source_preserves_full_index_policy() {
    let mut app = files_app();
    set_index_source(&mut app, 5, Some("index unavailable"));
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 4);
    assert!(hits(&mut app).is_empty());

    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path]);
    assert_eq!(
        app.world.failed_sources(),
        vec![("index", "index unavailable")]
    );
}

#[test]
fn files_find_empty_terminal_is_published_once() {
    let mut app = files_app();
    set_index_source(&mut app, 3, None);
    let empty = format!("{}/empty-find", app.world.location.home);
    app.world.fs.dir(&empty, 1);
    app.world.location.home = empty;
    open_find(&mut app);
    type_query(&mut app, "anything");

    tick(&mut app, 3);
    let stamp = published(&mut app);
    assert_eq!(stamp, Some((0, true)));
    assert!(hits(&mut app).is_empty());
    let calls = call_counts();

    tick(&mut app, 100);
    assert_eq!(published(&mut app), stamp, "empty terminal stamp is stable");
    assert_eq!(call_counts(), calls, "empty terminal is not recomputed");
}

#[test]
fn files_find_empty_terminal_is_not_rescanned_on_reentry() {
    let mut app = files_app();
    set_index_source(&mut app, 1, None);
    let empty = format!("{}/empty-find", app.world.location.home);
    app.world.fs.dir(&empty, 1);
    app.world.location.home = empty.clone();
    open_find(&mut app);
    tick(&mut app, 2);
    assert_eq!(call_counts().0, 1, "the home index is built once on open");

    app.push_page(Page::Files {
        path: empty,
        query: None,
    });
    let before_reentry = call_counts().0;
    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));
    assert!(find_page_is_top(&mut app), "the find page is back on top");
    assert_eq!(
        call_counts().0,
        before_reentry,
        "re-entry does not rebuild an empty index"
    );
}

#[test]
fn files_find_enter_after_completion_publishes_current_index() {
    let mut app = files_app();
    set_index_source(&mut app, 2, None);
    let path = add_final_file(&mut app);
    advance_world_only(&mut app, 5);

    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_equal_length_index_replacement_republishes() {
    let mut app = files_app();
    set_index_source(&mut app, 1, None);
    open_find(&mut app);
    install_index(&mut app, vec!["/t/a-report.txt".into()]);
    type_query(&mut app, "report");
    tick(&mut app, 2);
    assert_eq!(hits(&mut app), vec!["/t/a-report.txt".to_owned()]);

    install_index(&mut app, vec!["/t/b-report.txt".into()]);
    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec!["/t/b-report.txt".to_owned()]);
}

#[test]
fn files_find_completion_preserves_selected_path() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let early = format!("{}/aa-dir", app.world.location.home);
    app.world.fs.dir(&early, 1);
    let partial = format!("{early}/zz-final-hit-a.txt");
    app.world.fs.text(&partial, "partial\n", 1);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 9);
    assert_eq!(hits(&mut app), vec![partial.clone()]);
    assert_eq!(selected(&mut app), Some(partial.clone()));

    tick(&mut app, 1);
    assert_eq!(hits(&mut app), vec![path, partial.clone()]);
    assert_eq!(selected(&mut app), Some(partial));
}

fn drawn_text(app: &mut App) -> String {
    drawn_text_sized(app, 120)
}

fn drawn_text_sized(app: &mut App, columns: u16) -> String {
    let mut term = ratatui::Terminal::new(ratatui::backend::TestBackend::new(columns, 40))
        .expect("test terminal");
    term.draw(|f| junie_tui::shell::render(app, f))
        .expect("test backend draws");
    let buf = term.backend().buffer();
    let area = buf.area;
    (area.y..area.y + area.height)
        .map(|y| {
            (area.x..area.x + area.width)
                .map(|x| buf[(x, y)].symbol().to_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn files_find_completion_renders_the_same_publication() {
    let mut app = files_app();
    set_index_source(&mut app, 10, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);

    tick(&mut app, 9);
    let before = drawn_text(&mut app);
    assert!(before.contains("· 0 results"), "{before}");
    assert!(before.contains("indexing"), "{before}");

    tick(&mut app, 1);
    let after = drawn_text(&mut app);
    assert!(after.contains("· 1 results"), "{after}");
    assert!(!after.contains("indexing"), "{after}");
    assert_eq!(hits(&mut app), vec![path]);
}

#[test]
fn files_find_enter_publishes_zero_duration_and_absent_index_sources() {
    let mut app = files_app();
    set_index_source(&mut app, 0, None);
    let path = add_final_file(&mut app);
    open_find(&mut app);
    type_query(&mut app, FINAL_QUERY);
    assert_eq!(
        hits(&mut app),
        vec![path.clone()],
        "zero-duration index is done on enter"
    );

    let mut absent = files_app();
    absent.world.sources.retain(|s| s.name != "index");
    let path = add_final_file(&mut absent);
    open_find(&mut absent);
    type_query(&mut absent, FINAL_QUERY);
    assert_eq!(
        hits(&mut absent),
        vec![path],
        "a missing index source is done"
    );
    assert_eq!(published(&mut absent).map(|(_, done)| done), Some(true));
}

use crate::domain::activity::{Script, ScriptLine};
use crate::domain::context::ScopeTag;
use crate::domain::effect::Effect;
use crate::domain::projection::{Work, work};

fn settled(scenario: Scenario, history: bool) -> App {
    let mut app = App::launch(scenario, Motion::Reduced, 0, Theme::junie(), history);
    app.world.clock.running = true;
    for _ in 0..3_000 {
        if !app.animating() {
            break;
        }
        tick(&mut app, 1);
    }
    tick(&mut app, 3);
    assert!(!app.animating(), "{scenario:?} is idle once it settles");
    drawn_text(&mut app);
    app
}

fn measure(app: &mut App, f: impl FnOnce(&mut App)) -> Work {
    let before = work();
    f(app);
    work().since(before)
}

fn start_stream(app: &mut App, name: &str) -> String {
    let scope = ScopeTag::here(&app.world.location.cwd);
    app.world.start_activity(
        vec![],
        Script::running(Vec::<ScriptLine>::new()),
        name,
        "test",
        ActivityKind::Task,
        scope,
    )
}

#[test]
fn holla_projection_idle_ticks_do_no_source_work() {
    for scenario in [
        Scenario::RustDirty,
        Scenario::DockerCleanup,
        Scenario::FirstUse,
        Scenario::MonorepoRoot,
        Scenario::HardCases,
    ] {
        let mut app = settled(scenario, true);
        let mut repaints = 0;
        let delta = measure(&mut app, |app| {
            for _ in 0..100 {
                if app.handle_input(Input::Tick) != Outcome::Ignored {
                    repaints += 1;
                }
            }
        });
        assert_eq!(delta, Work::default(), "{scenario:?}: 100 settled ticks");
        assert!(
            repaints <= 9,
            "{scenario:?}: only the once-a-second freshness label may repaint, got {repaints}"
        );
    }
}

#[test]
fn holla_projection_idle_cleanup_page_ticks_do_no_source_work() {
    let mut app = settled(Scenario::DiskCleanup, true);
    app.push_page(Page::Cleanup { category: None });
    drawn_text(&mut app);
    let mut repaints = 0;
    let delta = measure(&mut app, |app| {
        for _ in 0..100 {
            if app.handle_input(Input::Tick) != Outcome::Ignored {
                repaints += 1;
            }
        }
    });
    assert_eq!(delta, Work::default());
    assert!(repaints <= 1, "{repaints} repaints from other shell timers");
}

struct Case {
    name: &'static str,
    prep: fn(&mut App),
    mutate: fn(&mut App),
    expect: Option<Expect>,
}

#[derive(Clone, Copy)]
enum Expect {
    Catalog,
    Memory,
}

fn first_container(app: &App) -> String {
    app.world.docker.containers[0].name.clone()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "effect",
            prep: |_| {},
            mutate: |a| {
                let n = first_container(a);
                a.world.apply_effect(&Effect::DockerStop(n));
            },
            expect: Some(Expect::Catalog),
        },
        Case {
            name: "start_activity",
            prep: |_| {},
            mutate: |a| {
                start_stream(a, "fresh");
            },
            expect: Some(Expect::Catalog),
        },
        Case {
            name: "edit_catalog",
            prep: |_| {},
            mutate: |a| a.world.edit_catalog(|w| w.sudo_cached = !w.sudo_cached),
            expect: Some(Expect::Catalog),
        },
        Case {
            name: "fs put",
            prep: |_| {},
            mutate: |a| {
                a.world.fs.file("/work/scratch/new.txt", 10, 1);
            },
            expect: Some(Expect::Catalog),
        },
        Case {
            name: "record_use",
            prep: |_| {},
            mutate: |a| a.world.record_use("docker.cleanup"),
            expect: Some(Expect::Memory),
        },
        Case {
            name: "pin",
            prep: |_| {},
            mutate: |a| {
                let cwd = a.world.location.cwd.clone();
                a.world
                    .edit_memory(|w| w.memory.toggle_pin(&cwd, "docker.cleanup"));
            },
            expect: Some(Expect::Memory),
        },
        Case {
            name: "output only",
            prep: |a| {
                start_stream(a, "stream");
                tick(a, 2);
                drawn_text(a);
            },
            mutate: |a| {
                let id = a.world.activities[0].id.clone();
                a.world.edit_activity(&id, |act| {
                    for i in 0..500 {
                        act.output.push((
                            None,
                            format!("line {i}"),
                            crate::domain::activity::LineTone::Normal,
                        ));
                    }
                });
            },
            expect: None,
        },
        Case {
            name: "clipboard",
            prep: |_| {},
            mutate: |a| a.world.clipboard = Some("copied".into()),
            expect: None,
        },
        Case {
            name: "failed fs operations",
            prep: |_| {},
            mutate: |a| {
                let _ = a.world.fs.remove_permanent("/no/such/path");
                a.world.fs.touch("/no/such/path", 2);
            },
            expect: None,
        },
        Case {
            name: "git fetch effect",
            prep: |_| {},
            mutate: |a| a.world.apply_effect(&Effect::GitFetch("/x".into())),
            expect: None,
        },
        Case {
            name: "stop_all with nothing live",
            prep: |_| {},
            mutate: |a| {
                a.world.stop_all();
            },
            expect: None,
        },
    ]
}

#[test]
fn holla_projection_each_app_mutation_rebuilds_exactly_its_cache() {
    for case in cases() {
        let mut app = settled(Scenario::DockerCleanup, true);
        (case.prep)(&mut app);
        (case.mutate)(&mut app);
        let delta = measure(&mut app, |app| tick(app, 1));
        let name = case.name;
        match case.expect {
            Some(Expect::Catalog) => {
                assert_eq!(delta.catalog_builds, 1, "{name}: the catalog rebuilds once");
                assert_eq!(delta.decorations, 1, "{name}: and is decorated once");
                assert_eq!(
                    delta.candidate_scans, 1,
                    "{name}: and scans candidates once"
                );
                assert!(delta.ranks >= 1, "{name}: and reranks");
            }
            Some(Expect::Memory) => {
                assert_eq!(
                    delta.catalog_builds, 0,
                    "{name}: the structural catalog is reused"
                );
                assert_eq!(delta.candidate_scans, 0, "{name}");
                assert_eq!(delta.decorations, 1, "{name}: only memory decorates again");
                assert!(delta.ranks >= 1, "{name}: and reranks");
            }
            None => assert_eq!(delta, Work::default(), "{name}: no work"),
        }
        let again = measure(&mut app, |app| tick(app, 5));
        assert_eq!(again, Work::default(), "{name}: settled afterwards");
    }
}

fn top_finder_id(app: &mut App) -> Option<String> {
    app.finder_mut().and_then(|f| f.current_id())
}

#[test]
fn holla_projection_hidden_pages_catch_up_once() {
    let mut app = settled(Scenario::DockerCleanup, true);
    app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
    drawn_text(&mut app);
    let selected = top_finder_id(&mut app).expect("a selected row");

    let id = start_stream(&mut app, "side");
    app.open_activity_tab(&id, true);
    assert_ne!(app.active(), 0, "the finder is on a hidden tab");

    let name = first_container(&app);
    app.world.apply_effect(&Effect::DockerStop(name));
    app.world.fs.file("/work/scratch/hidden.txt", 8, 1);
    let hidden = measure(&mut app, |app| tick(app, 30));
    assert_eq!(
        hidden,
        Work::default(),
        "no projection work while the finder is hidden, however many sources changed"
    );

    let entered = measure(&mut app, |app| app.activate_tab(0));
    assert_eq!(
        entered.catalog_builds, 1,
        "one catch-up build on activation"
    );
    assert_eq!(entered.decorations, 1);
    assert!(entered.ranks >= 1);
    assert_eq!(
        top_finder_id(&mut app),
        Some(selected),
        "the stable selection survives the catch-up"
    );
    assert_eq!(measure(&mut app, |app| tick(app, 10)), Work::default());
    drawn_text(&mut app);
    assert_eq!(
        measure(&mut app, |app| tick(app, 10)),
        Work::default(),
        "painting the caught-up page does no further work"
    );
}

#[test]
fn holla_projection_hidden_cleanup_catches_up_once() {
    let mut app = settled(Scenario::DiskCleanup, true);
    app.push_page(Page::Cleanup { category: None });
    drawn_text(&mut app);
    let id = start_stream(&mut app, "side");
    app.open_activity_tab(&id, true);
    assert_ne!(app.active(), 0);

    app.world
        .fs
        .remove_permanent("/Users/alex/Library/Logs/c")
        .expect("removed");
    let hidden = measure(&mut app, |app| tick(app, 30));
    assert_eq!(
        hidden,
        Work::default(),
        "a hidden cleanup page does no work"
    );

    let entered = measure(&mut app, |app| app.activate_tab(0));
    assert_eq!(entered.candidate_scans, 1, "one rescan on activation");
    assert_eq!(entered.cleanup_row_builds, 1);
    assert_eq!(entered.catalog_builds, 0);
    assert_eq!(measure(&mut app, |app| tick(app, 10)), Work::default());
}

#[test]
fn holla_projection_domain_work_continues_while_pages_are_hidden() {
    let mut app = settled(Scenario::DockerCleanup, true);
    let id = start_stream(&mut app, "side");
    app.open_activity_tab(&id, true);
    let tick_before = app.world.tick;
    app.world.edit_activity(&id, |a| {
        a.stop(tick_before);
    });
    tick(&mut app, 30);
    assert!(app.world.tick > tick_before);
    assert_eq!(
        app.world.activity(&id).map(|a| a.state),
        Some(ActivityState::Stopped),
        "the activity finished while the finder slept"
    );
}

#[test]
fn holla_projection_output_only_does_not_rerank_finder() {
    let mut app = settled(Scenario::RustDirty, false);
    let id = start_stream(&mut app, "streamer");
    tick(&mut app, 2);
    app.finder_mut().expect("finder").query.set_text("streamer");
    let text = drawn_text_sized(&mut app, 240);
    assert!(text.contains("0 lines retained"), "{text}");
    let selected = top_finder_id(&mut app);
    assert_eq!(selected.as_deref(), Some(format!("activity.{id}").as_str()));

    let mut last_len = 0;
    let delta = measure(&mut app, |app| {
        for round in 0..20 {
            app.world.edit_activity(&id, |a| {
                for i in 0..50 {
                    a.output.push((
                        None,
                        format!("line {round}-{i}"),
                        crate::domain::activity::LineTone::Normal,
                    ));
                }
            });
            app.handle_input(Input::Tick);
            last_len = app.world.activity(&id).expect("activity").output.len();
        }
    });
    assert_eq!(
        delta,
        Work::default(),
        "1000 streamed lines rebuild, rescan and rerank nothing"
    );
    let a = app.world.activity(&id).expect("activity");
    let duration = crate::screens::ticks_label(a.duration_ticks(app.world.tick));
    let text = drawn_text_sized(&mut app, 240);
    assert!(
        text.contains(&format!("{last_len} lines retained")),
        "the preview shows the current retained line count: {text}"
    );
    assert!(
        text.contains(&duration),
        "the row shows the current duration {duration}: {text}"
    );

    let tick_now = app.world.tick;
    app.world.edit_activity(&id, |a| a.stop(tick_now));
    let stopping = measure(&mut app, |app| tick(app, 1));
    assert_eq!(
        stopping.catalog_builds, 1,
        "the stop request is one transition"
    );
    let settled_stop = measure(&mut app, |app| {
        for _ in 0..40 {
            app.handle_input(Input::Tick);
        }
    });
    assert_eq!(
        settled_stop.catalog_builds, 1,
        "the terminal state is one more transition, not one per tick"
    );
    assert_eq!(
        app.world.activity(&id).map(|a| a.state),
        Some(ActivityState::Stopped)
    );
    let text = drawn_text_sized(&mut app, 240);
    assert!(text.contains("stopped"), "{text}");
}

#[test]
fn holla_projection_cleanup_keys_rebuild_rows_without_scanning() {
    let mut app = settled(Scenario::DiskCleanup, true);
    app.push_page(Page::Cleanup { category: None });
    drawn_text(&mut app);
    let delta = measure(&mut app, |app| {
        for code in [
            KeyCode::Char('m'),
            KeyCode::Char('n'),
            KeyCode::Char(' '),
            KeyCode::Enter,
            KeyCode::Char('a'),
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Esc,
        ] {
            app.handle_input(key(code, KeyModifiers::NONE));
        }
    });
    assert_eq!(delta.candidate_scans, 0, "no key rescans the filesystem");
    assert_eq!(delta.catalog_builds, 0);
    assert!(
        delta.cleanup_row_builds >= 5,
        "fold, check, mode and dry run rebuild rows: {}",
        delta.cleanup_row_builds
    );
}

/// Drives every scenario with a deterministic pseudo-random key stream. Each
/// tick and paint of a Finder or Cleanup page runs the differential check
/// against a page built from nothing, so a world mutation that moves no epoch
/// and no deadline fails here.
#[test]
fn holla_projection_scenario_walk_matches_full_rebuild() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let letters = [
        'g', 'i', 't', 'd', 'o', 'c', 'k', 'e', 'r', 'u', 'p', 'm', 's', ' ',
    ];
    for scenario in Scenario::CONCEPT.iter().chain(Scenario::PARITY.iter()) {
        let mut app = App::launch(*scenario, Motion::Full, 0, Theme::junie(), true);
        app.world.clock.running = true;
        for _ in 0..60 {
            let code = match next() % 16 {
                0..=4 => KeyCode::Char(letters[(next() % letters.len() as u64) as usize]),
                5 | 6 => KeyCode::Down,
                7 => KeyCode::Up,
                8..=10 => KeyCode::Enter,
                11 | 12 => KeyCode::Esc,
                13 => KeyCode::Backspace,
                14 => KeyCode::Tab,
                _ => KeyCode::Right,
            };
            app.handle_input(key(code, KeyModifiers::NONE));
            tick(&mut app, 1 + (next() % 4) as usize);
            if next() % 4 == 0 {
                drawn_text(&mut app);
            }
        }
    }
}

fn tick_checked(app: &mut App, n: usize) {
    for _ in 0..n {
        app.handle_input(Input::Tick);
        drawn_text(app);
    }
}

/// The user-facing edit paths of App: ranking edits, running an item whose
/// effect lands later, stopping an activity and reloading a configuration. After
/// each, the visible Finder must equal a page built from nothing.
#[test]
fn holla_projection_user_edits_match_full_rebuild() {
    let mut app = settled(Scenario::DockerCleanup, true);
    let id = "docker.cleanup";
    app.menu_action(AltCmd::Pin, id);
    tick_checked(&mut app, 2);
    app.menu_action(AltCmd::Hide, id);
    tick_checked(&mut app, 2);
    app.menu_action(AltCmd::Hide, id);
    tick_checked(&mut app, 2);
    app.menu_action(AltCmd::Reset, id);
    tick_checked(&mut app, 2);
    app.remember_use("docker.images_prune");
    tick_checked(&mut app, 2);

    app.execute_item("docker.network_prune", vec![]);
    assert_ne!(app.active(), 0, "the run opened its activity tab");
    let started = measure(&mut app, |app| tick(app, 1));
    assert_eq!(started.catalog_builds, 0, "the hidden finder did nothing");
    for _ in 0..200 {
        if app.world.live_activities() == 0 {
            break;
        }
        tick(&mut app, 1);
    }
    assert_eq!(app.world.live_activities(), 0, "the run finished");
    app.activate_tab(0);
    tick_checked(&mut app, 3);

    app.execute_item("docker.images_prune", vec![]);
    let aid = app.world.activities.last().expect("activity").id.clone();
    app.activate_tab(0);
    app.run_derived(&format!("activity.{aid}.stop"), "docker.images_prune");
    tick_checked(&mut app, 3);
    for _ in 0..60 {
        tick(&mut app, 1);
    }
    app.activate_tab(0);
    tick_checked(&mut app, 3);
}

#[test]
fn holla_projection_config_reload_matches_full_rebuild() {
    let mut app = settled(Scenario::ParityCustomActions, true);
    app.reload_project_config();
    tick_checked(&mut app, 3);
}

#[test]
fn holla_projection_snapshot_revoke_invalidates_catalog() {
    let mut app = settled(Scenario::RustDirty, true);
    let path = app
        .world
        .custom_project
        .as_ref()
        .expect("a project configuration")
        .path
        .clone();
    app.push_page(Page::Config { path });
    drawn_text(&mut app);
    app.handle_input(key(KeyCode::Char('u'), KeyModifiers::NONE));
    assert!(status(&app).contains("Trust revoked"), "{}", status(&app));
    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));
    tick_checked(&mut app, 3);
}
