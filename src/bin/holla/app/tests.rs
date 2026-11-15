use super::*;
use crate::domain::activity::{ActivityKind, ActivityState, Script, TERM_EXIT_TICKS};
use crate::domain::context::ScopeTag;
use crate::screens::finder::FinderPage;
use crate::sim::world::BatchMode;
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

fn start_task(app: &mut App, name: &str, script: Script) -> String {
    let id = app.world.start_activity(
        vec![],
        script,
        name,
        "test",
        ActivityKind::Task,
        ScopeTag::here("/work"),
    );
    app.open_activity_tab(&id, true);
    id
}

fn activity_tab(app: &App, id: &str) -> Option<usize> {
    app.tabs
        .iter()
        .position(|t| t.kind == TabKind::Activity(id.to_owned()))
}

fn activity_state(app: &App, id: &str) -> ActivityState {
    app.world.activity(id).expect("activity").state
}

fn active_kind(app: &App) -> TabKind {
    app.tabs[app.active()].kind.clone()
}

fn open_close_dialog(app: &mut App, id: &str) {
    let tab = activity_tab(app, id).expect("activity tab open");
    app.activate_tab(tab);
    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert!(!app.modals.is_empty(), "the close dialog is open");
}

fn stop_and_close(app: &mut App, id: &str) {
    open_close_dialog(app, id);
    confirm_close_dialog(app);
}

fn tick_until_state(app: &mut App, id: &str, state: ActivityState) {
    for _ in 0..400 {
        if activity_state(app, id) == state {
            return;
        }
        tick(app, 1);
    }
    panic!(
        "{id} never reached {state:?}: {:?}",
        activity_state(app, id)
    );
}

/// Ticks until the tab for `id` closes. While the tab is open the process must
/// still be live. The domain record may be gone once the tab closes, so the
/// last state is only checked when a record remains.
fn tick_until_tab_closes(app: &mut App, id: &str) {
    for _ in 0..400 {
        if activity_tab(app, id).is_none() {
            assert!(
                app.world
                    .activity(id)
                    .is_none_or(|a| a.state == ActivityState::Stopped),
                "the tab closed before the process stopped"
            );
            return;
        }
        assert!(
            activity_state(app, id).live(),
            "the tab outlived a terminal activity"
        );
        tick(app, 1);
    }
    panic!("the tab for {id} never closed");
}

fn assert_tabs_consistent(app: &App) {
    assert_eq!(app.tabs.len(), app.router.tab_count(), "tabs match router");
}

#[test]
fn stop_and_close_removes_activity_tab_after_terminal_event() {
    let mut app = app();
    let id = start_task(&mut app, "build", Script::running(vec![]));

    stop_and_close(&mut app, &id);
    assert_eq!(activity_state(&app, &id), ActivityState::Cancelling);
    assert_eq!(
        tabs_of_activity(&app, &id),
        1,
        "the tab stays while stopping"
    );
    assert!(
        status(&app).starts_with("Stopping"),
        "status: {}",
        status(&app)
    );

    tick(&mut app, (TERM_EXIT_TICKS - 1) as usize);
    assert_eq!(activity_state(&app, &id), ActivityState::Cancelling);
    assert_eq!(tabs_of_activity(&app, &id), 1, "still stopping");

    tick_until_tab_closes(&mut app, &id);
    assert_eq!(tabs_of_activity(&app, &id), 0, "the tab closed once");
    assert_eq!(active_kind(&app), TabKind::Here);
    assert_tabs_consistent(&app);
    assert!(status(&app).contains("stopped"), "{}", status(&app));

    tick(&mut app, 20);
    assert_tabs_consistent(&app);
}

#[test]
fn stop_and_close_tracks_id_across_tab_switch() {
    let mut app = app();
    let a = start_task(&mut app, "alpha", Script::running(vec![]));
    let b = start_task(&mut app, "bravo", Script::running(vec![]));
    let c = start_task(&mut app, "charlie", Script::running(vec![]));
    assert_eq!(activity_tab(&app, &a), Some(1));
    assert_eq!(activity_tab(&app, &b), Some(2));
    assert_eq!(activity_tab(&app, &c), Some(3));

    stop_and_close(&mut app, &a);
    let tab = activity_tab(&app, &c).expect("charlie tab");
    app.activate_tab(tab);
    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    let owner_before = app.modals.iter().map(|m| m.owner).collect::<Vec<_>>();
    assert_eq!(owner_before, vec![3], "a dialog is open on charlie");

    tick_until_tab_closes(&mut app, &a);

    assert_eq!(activity_tab(&app, &a), None, "only the stopped tab closed");
    assert_eq!(activity_tab(&app, &b), Some(1), "bravo shifted down");
    assert_eq!(activity_tab(&app, &c), Some(2), "charlie shifted down");
    assert_eq!(active_kind(&app), TabKind::Activity(c.clone()));
    assert_eq!(app.active(), 2);
    assert_eq!(
        app.modals.iter().map(|m| m.owner).collect::<Vec<_>>(),
        vec![2],
        "the dialog still belongs to charlie"
    );
    assert_tabs_consistent(&app);
    assert!(activity_state(&app, &b).live(), "bravo was not stopped");
    assert!(activity_state(&app, &c).live(), "charlie was not stopped");
}

#[test]
fn stop_and_close_resistant_activity_waits_for_reap() {
    let mut app = app();
    let id = start_task(&mut app, "stubborn", Script::running(vec![]).resistant());

    stop_and_close(&mut app, &id);
    tick(&mut app, TERM_EXIT_TICKS as usize + 1);
    assert_eq!(
        activity_state(&app, &id),
        ActivityState::Cancelling,
        "SIGTERM is ignored"
    );
    assert_eq!(tabs_of_activity(&app, &id), 1, "the tab waits past SIGTERM");

    let mut sigkill_seen = false;
    for _ in 0..400 {
        if activity_tab(&app, &id).is_none() {
            break;
        }
        sigkill_seen |= app
            .world
            .activity(&id)
            .is_some_and(|a| a.cancel.as_ref().is_some_and(|c| c.kill_sent.is_some()));
        assert!(
            activity_state(&app, &id).live(),
            "the tab outlived the process"
        );
        tick(&mut app, 1);
    }
    assert!(sigkill_seen, "SIGKILL was sent before the tab closed");
    assert_eq!(tabs_of_activity(&app, &id), 0, "the reap closes the tab");
    assert_tabs_consistent(&app);
}

#[test]
fn stop_and_close_queued_or_already_finished_closes_immediately() {
    let mut app = app();
    let ids = app.world.start_batch(
        "pair",
        "Pair",
        BatchMode::Sequential,
        ["first", "second"]
            .map(|n| {
                (
                    n.into(),
                    vec![],
                    Script::running(vec![]),
                    None,
                    ScopeTag::here("/work"),
                )
            })
            .into(),
        "Pair",
    );
    assert_eq!(activity_state(&app, &ids[1]), ActivityState::Queued);
    app.open_activity_tab(&ids[1], true);
    stop_and_close(&mut app, &ids[1]);
    assert_eq!(tabs_of_activity(&app, &ids[1]), 0, "queued stop closes now");
    assert!(
        app.world
            .activity(&ids[1])
            .is_none_or(|a| a.state == ActivityState::Stopped)
    );
    assert!(
        activity_state(&app, &ids[0]).live(),
        "the running member is untouched"
    );
    assert_tabs_consistent(&app);

    let done = start_task(&mut app, "quick", Script::ending(vec![], 4, 0));
    open_close_dialog(&mut app, &done);
    tick(&mut app, 3);
    assert!(app.world.activity(&done).is_some_and(|a| a.state.live()));
    tick_until_state(&mut app, &done, ActivityState::Succeeded);
    assert_eq!(
        tabs_of_activity(&app, &done),
        1,
        "a finish alone does not close the tab"
    );
    confirm_close_dialog(&mut app);
    assert_eq!(
        tabs_of_activity(&app, &done),
        0,
        "finished while the dialog was open"
    );
    assert!(
        app.world
            .activity(&done)
            .is_none_or(|a| a.state == ActivityState::Succeeded),
        "a finished activity is not stopped again"
    );
    assert_tabs_consistent(&app);
}

#[test]
fn stop_and_close_duplicate_or_cancelled_request_is_idempotent() {
    let mut app = app();
    let a = start_task(&mut app, "alpha", Script::running(vec![]));
    let b = start_task(&mut app, "bravo", Script::running(vec![]));

    open_close_dialog(&mut app, &a);
    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));
    assert!(app.modals.is_empty());
    assert_eq!(activity_state(&app, &a), ActivityState::Running);
    tick(&mut app, 10);
    assert_eq!(activity_state(&app, &a), ActivityState::Running);
    assert_eq!(
        tabs_of_activity(&app, &a),
        1,
        "a dismissed dialog leaves no intent"
    );

    stop_and_close(&mut app, &a);
    stop_and_close(&mut app, &a);
    let tab = activity_tab(&app, &b).expect("bravo tab");
    app.activate_tab(tab);
    tick_until_tab_closes(&mut app, &a);
    assert_eq!(activity_tab(&app, &a), None);
    assert_eq!(active_kind(&app), TabKind::Activity(b.clone()));
    assert!(activity_state(&app, &b).live());
    let tabs = app.tabs.len();

    tick(&mut app, 20);
    assert_eq!(
        app.tabs.len(),
        tabs,
        "a repeated terminal event closes nothing"
    );
    assert_eq!(active_kind(&app), TabKind::Activity(b));
    assert_tabs_consistent(&app);
}

#[test]
fn stop_and_close_missing_record_retires_intent() {
    let mut app = app();
    let a = start_task(&mut app, "alpha", Script::running(vec![]));
    open_close_dialog(&mut app, &a);
    app.world.activities.retain(|x| x.id != a);
    confirm_close_dialog(&mut app);
    assert_eq!(tabs_of_activity(&app, &a), 0, "the orphaned tab closed");
    assert_eq!(status(&app), "That activity is gone");
    assert_tabs_consistent(&app);

    let b = start_task(&mut app, "bravo", Script::running(vec![]));
    stop_and_close(&mut app, &b);
    app.world.activities.retain(|x| x.id != b);
    app.dispatch_msg(Msg::ActivityEnded {
        id: b.clone(),
        ok: false,
    });
    assert_eq!(tabs_of_activity(&app, &b), 0, "the orphaned tab closed");
    assert!(app.pending_activity_closes.is_empty(), "the intent retired");
    assert_tabs_consistent(&app);
    let tabs = app.tabs.len();

    let c = start_task(&mut app, "charlie", Script::running(vec![]));
    tick(&mut app, 20);
    assert_eq!(tabs_of_activity(&app, &c), 1);
    assert_eq!(app.tabs.len(), tabs + 1);
}

fn tabs_of_activity(app: &App, id: &str) -> usize {
    app.tabs
        .iter()
        .filter(|t| t.kind == TabKind::Activity(id.to_owned()))
        .count()
}

#[test]
fn disk_page_stops_animating_on_the_tick_that_publishes_the_cache() {
    let mut app = App::launch(
        Scenario::DiskCleanup,
        Motion::Full,
        0,
        Theme::junie(),
        false,
    );
    app.world.clock.running = true;
    app.world.disk.scan_ticks = 1;
    app.world.fs.dir("/work/big", 1);
    for i in 0..4096 {
        app.world.fs.file(&format!("/work/big/f{i:05}"), 4096, 1);
    }
    app.world.start_scan("/work/big");
    app.push_page(Page::Disk {
        path: "/work/big".into(),
    });

    tick(&mut app, 1);
    assert!(app.top().expect("page").animating(&app.world));
    assert_eq!(
        app.world.persisted.sizes, None,
        "cache waits for the last entry"
    );

    tick(&mut app, 1);
    assert!(app.world.scan.as_ref().expect("scan").completed());
    assert!(app.world.persisted.sizes.is_some());
    assert!(!app.top().expect("page").animating(&app.world));
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
    let mut term =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 40)).expect("test terminal");
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

fn finder_app() -> App {
    let mut app = App::launch(Scenario::RustDirty, Motion::Full, 0, Theme::junie(), false);
    app.world.clock.running = true;
    app
}

fn finder(app: &mut App) -> &mut FinderPage {
    app.router
        .top_of_mut(0)
        .and_then(|s| s.as_finder())
        .expect("finder on the first tab")
}

fn finder_select_explore(app: &mut App, id: &str) {
    for _ in 0..80 {
        match finder(app).current_id() {
            Some(cur) if cur == id => return,
            Some(cur) if cur.starts_with("explore.") => {
                app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));
            }
            _ => {
                app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
            }
        }
    }
    panic!("finder never selected {id}");
}

#[test]
fn finder_explore_selection_survives_background_tick() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    tick(&mut app, 3);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
}

#[test]
fn finder_missing_explore_selection_uses_first_selectable() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.handle_input(key(KeyCode::Down, KeyModifiers::CONTROL));
    tick(&mut app, 1);
    let after_scope = finder(&mut app).current_id();
    app.handle_input(key(KeyCode::Home, KeyModifiers::NONE));
    let first = finder(&mut app).current_id();

    assert_ne!(after_scope.as_deref(), Some("explore.git"));
    assert_eq!(after_scope, first);
}

#[test]
fn finder_query_edit_resets_explore_selection() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.handle_input(key(KeyCode::Char('g'), KeyModifiers::NONE));
    tick(&mut app, 1);
    let typed = finder(&mut app).current_id();
    app.handle_input(key(KeyCode::Home, KeyModifiers::NONE));

    assert_eq!(finder(&mut app).current_id(), typed);
}

const LIST_COLS: usize = 78;

fn explore_tiles(ids: &[&str]) -> Vec<crate::domain::catalog::ExploreTile> {
    let all = crate::domain::catalog::default_explore_tiles();
    ids.iter()
        .map(|id| {
            all.iter()
                .find(|t| t.id == *id)
                .unwrap_or_else(|| panic!("no default tile {id}"))
                .clone()
        })
        .collect()
}

fn finder_list_lines(app: &mut App) -> Vec<String> {
    drawn_text(app)
        .lines()
        .map(|l| l.chars().take(LIST_COLS).collect())
        .collect()
}

fn drawn_explore_row(app: &mut App) -> (u16, String) {
    let lines = finder_list_lines(app);
    let heading = lines
        .iter()
        .position(|l| l.trim() == "Explore")
        .expect("Explore heading drawn");
    (heading as u16 + 1, lines[heading + 1].clone())
}

fn drawn_tile(app: &mut App, label: &str) -> (u16, u16) {
    let (y, row) = drawn_explore_row(app);
    let at = row
        .find(label)
        .unwrap_or_else(|| panic!("{label} not drawn: {row}"));
    (row[..at].chars().count() as u16, y)
}

fn chosen_tile(app: &mut App) -> Option<String> {
    let (_, row) = drawn_explore_row(app);
    let at = row.find('\u{203a}')?;
    row[at + '\u{203a}'.len_utf8()..]
        .split_whitespace()
        .next()
        .map(str::to_owned)
}

fn focus_markers(app: &mut App) -> usize {
    finder_list_lines(app)
        .iter()
        .map(|l| l.chars().filter(|c| *c == '\u{203a}').count())
        .sum()
}

fn click_at(app: &mut App, x: u16, y: u16) {
    use junie_tui::core::event::{Mouse, MouseKind};
    let at = ratatui::layout::Position::new(x, y);
    app.handle_input(Input::Mouse(Mouse::new(MouseKind::Down, at)));
    app.handle_input(Input::Mouse(Mouse::new(MouseKind::Up, at)));
}

fn finder_group(app: &mut App) -> Option<&'static str> {
    finder(app).group
}

#[test]
fn finder_explore_pointer_activation_uses_retained_id() {
    let mut app = finder_app();
    app.world.explore_tiles = explore_tiles(&[
        "explore.system",
        "explore.tasks",
        "explore.files",
        "explore.git",
        "explore.disk",
        "explore.services",
    ]);
    tick(&mut app, 1);

    let (x, y) = drawn_tile(&mut app, "Disk");
    click_at(&mut app, x + 1, y);
    assert_eq!(finder_group(&mut app), Some("Disk"));

    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));
    assert_eq!(finder_group(&mut app), None);
    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.disk")
    );
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Disk"));
    assert_eq!(focus_markers(&mut app), 1);
}

#[test]
fn finder_explore_selection_tracks_reordered_cells() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Git"));

    app.world.explore_tiles = explore_tiles(&[
        "explore.system",
        "explore.disk",
        "explore.files",
        "explore.tasks",
        "explore.git",
    ]);
    tick(&mut app, 1);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Git"));
    assert_eq!(focus_markers(&mut app), 1);
    let drawn = drawn_text(&mut app);
    assert!(
        drawn
            .lines()
            .any(|l| l.contains("Git") && l.contains("explore \u{b7} here")),
        "preview follows the moved tile: {drawn}"
    );

    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(finder_group(&mut app), Some("Git"));
}

#[test]
fn finder_empty_explore_row_is_not_selectable() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.world.explore_tiles = vec![];
    tick(&mut app, 1);
    let after = finder(&mut app).current_id();
    app.handle_input(key(KeyCode::Home, KeyModifiers::NONE));
    let first = finder(&mut app).current_id();
    assert_eq!(after, first);
    assert!(
        first
            .as_deref()
            .is_some_and(|id| !id.starts_with("explore."))
    );
    assert_eq!(focus_markers(&mut app), 1);

    for _ in 0..40 {
        app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
        let id = finder(&mut app).current_id();
        assert!(
            id.as_deref().is_some_and(|id| !id.starts_with("explore.")),
            "{id:?}"
        );
    }
    assert_eq!(focus_markers(&mut app), 1);
}

#[test]
fn finder_identity_moves_between_item_and_explore() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    let (cwd, host, now) = (
        app.world.location.cwd.clone(),
        app.world.host.name.clone(),
        app.world.now_secs(),
    );
    app.world
        .memory
        .usage
        .actions
        .push(crate::domain::usage::ActionUse {
            item: "explore.git".into(),
            path: Some(cwd),
            host,
            stamps: vec![now],
        });
    tick(&mut app, 1);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(chosen_tile(&mut app), None);
    assert_eq!(focus_markers(&mut app), 1);
    let drawn = drawn_text(&mut app);
    assert!(
        drawn
            .lines()
            .any(|l| l.contains("Git") && l.contains("explore \u{b7} here")),
        "preview keeps the same item: {drawn}"
    );

    app.world.memory.usage.actions.clear();
    tick(&mut app, 1);
    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(chosen_tile(&mut app).as_deref(), Some("Git"));
    assert_eq!(focus_markers(&mut app), 1);

    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert_eq!(finder_group(&mut app), Some("Git"));
}
