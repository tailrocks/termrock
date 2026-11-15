use super::*;
use crate::domain::activity::ActivityKind;
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

// Issue #81: running plans take part in quit.

use crate::domain::activity::Script;
use crate::domain::cleanup::{DeleteItem, DeletePlan, Mode};
use crate::domain::context::ScopeTag;

fn start_activity(app: &mut App, name: &str, script: Script) -> String {
    let scope = ScopeTag::here(&app.world.location.cwd);
    app.world
        .start_activity(vec![], script, name, name, ActivityKind::Task, scope)
}

fn start_cleanup(app: &mut App, items: usize) {
    let items = (0..items)
        .map(|i| DeleteItem {
            path: format!("/tmp/holla-quit-test/{i}"),
            category: "test".into(),
            estimate: 1,
            guard: None,
        })
        .collect();
    let host = app.world.host.name.clone();
    let plan = DeletePlan::new(&host, "/tmp/holla-quit-test", items, Mode::Trash, true);
    crate::screens::cleanup::commit(&plan, &mut app.world).expect("cleanup starts");
}

fn ctrl_q(app: &mut App) {
    app.handle_input(key(KeyCode::Char('q'), KeyModifiers::CONTROL));
}

fn quit_dialog_body(app: &App) -> String {
    let top = app.modals.top().expect("a dialog is open");
    assert_eq!(top.tag.kind, "quit");
    match &app.modals.top_as::<Dialog>().expect("dialog").body {
        junie_tui::widgets::dialog::DialogBody::Text(text) => text.clone(),
        _ => panic!("quit dialog has a text body"),
    }
}

fn assert_quit_body(app: &App, expected: &str) {
    let body = quit_dialog_body(app);
    assert!(body.ends_with(expected), "{body}");
    assert!(
        !body.contains("detached monitors keep running"),
        "the stale detached claim is gone: {body}"
    );
}

fn confirm_quit(app: &mut App) {
    app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty(), "the quit dialog resolved");
}

#[test]
fn quit_running_plan_requires_confirmation() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    app.activate_tab(0);
    assert_eq!(app.world.live_activities(), 0);
    assert!(app.world.cleanup_job.is_none());
    let before = app.world.plan(ID).cloned();

    ctrl_q(&mut app);

    assert!(!app.shell.quit, "a running plan blocks immediate quit");
    assert_quit_body(&app, "1 plan is running. Quitting stops it.");
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty());
    assert!(!app.shell.quit && !app.quitting);
    assert_eq!(app.world.plan(ID).cloned(), before, "Stay changes nothing");
    assert!(!app.world.stopping());
}

#[test]
fn quit_plan_only_stops_and_finalizes_before_exit() {
    let mut app = app();
    let uses = app.world.docker.cleanup_uses;
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    app.activate_tab(0);
    ctrl_q(&mut app);

    confirm_quit(&mut app);

    assert!(app.shell.quit, "nothing is left to wait for");
    let plan = app.world.plan(ID).expect("plan");
    assert_eq!(plan.phase, PlanPhase::Done);
    assert!(plan.ended_tick.is_some());
    assert_eq!(live_steps(&app, ID), 0);
    assert_eq!(step(&app, ID, "stop"), StepState::Succeeded);
    assert_eq!(step(&app, ID, "rm"), StepState::Cancelled);
    assert_eq!(app.world.docker.cleanup_uses, uses + 1, "one completion");
    assert!(
        status(&app).starts_with("Clean Docker completely stopped"),
        "the completion was delivered before exit: {}",
        status(&app)
    );
}

#[test]
fn quit_hidden_plan_is_owned_by_world() {
    let mut app = app();
    let now = app.world.tick;
    assert!(app.world.plan_mut(ID).expect("plan").arm(now));
    assert!(plan_tab(&app, ID).is_none(), "no presentation exists");
    tick(&mut app, 3);

    ctrl_q(&mut app);
    assert_quit_body(&app, "1 plan is running. Quitting stops it.");
    confirm_quit(&mut app);

    assert!(app.shell.quit);
    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Done);
    assert_eq!(live_steps(&app, ID), 0);
}

#[test]
fn quit_mixed_work_waits_for_activity_reap_and_cleanup_report() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let resistant = start_activity(&mut app, "stubborn", Script::running(vec![]).resistant());
    let queued = start_activity(&mut app, "later", Script::running(vec![]));
    {
        let a = app.world.activity_mut(&queued).expect("activity");
        a.state = ActivityState::Queued;
        a.after = Some(resistant.clone());
    }
    start_cleanup(&mut app, 3);
    let uses = app.world.docker.cleanup_uses;
    app.activate_tab(0);

    ctrl_q(&mut app);
    assert_quit_body(
        &app,
        "2 activities and 1 plan are running. Quitting stops them. A cleanup is at 0 of 3 items; quitting waits for its report and log.",
    );
    confirm_quit(&mut app);

    assert!(!app.shell.quit);
    assert!(app.quitting);
    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Done);
    assert_eq!(app.world.docker.cleanup_uses, uses + 1);
    assert_eq!(
        app.world.activity(&queued).expect("activity").state,
        ActivityState::Stopped,
        "queued work stops at once"
    );
    let requested = app
        .world
        .activity(&resistant)
        .and_then(|a| a.cancel.clone())
        .expect("cancel requested")
        .requested;
    let reports = app.world.reports.len();

    let mut ticks = 0;
    while !app.shell.quit {
        assert!(ticks < 200, "quit never completed");
        tick(&mut app, 1);
        ticks += 1;
        if app.shell.quit {
            break;
        }
        assert!(
            !app.world.quit_work().is_empty(),
            "quit_work empty but the shell is still waiting"
        );
    }
    let work = app.world.quit_work();
    assert!(work.is_empty(), "{work:?}");
    let a = app.world.activity(&resistant).expect("activity");
    assert_eq!(a.state, ActivityState::Stopped);
    assert_eq!(
        a.exit,
        Some(137),
        "the original SIGTERM then SIGKILL schedule ran"
    );
    assert!(a.cancel.as_ref().is_some_and(|c| c.requested == requested));
    assert!(app.world.cleanup_job.is_none());
    assert_eq!(app.world.reports.len(), reports);
    assert!(!app.world.reports.last().expect("report").items.is_empty());
    assert_eq!(
        app.world.ops_log.lines.len(),
        3,
        "the log landed once per item"
    );
}

#[test]
fn quit_repeated_confirmation_is_idempotent() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let resistant = start_activity(&mut app, "stubborn", Script::running(vec![]).resistant());
    start_cleanup(&mut app, 4);
    let uses = app.world.docker.cleanup_uses;
    app.activate_tab(0);
    ctrl_q(&mut app);
    confirm_quit(&mut app);
    let cancel = app
        .world
        .activity(&resistant)
        .and_then(|a| a.cancel.clone());
    let plan = app.world.plan(ID).cloned();
    let output = app.world.activity(&resistant).map(|a| a.output.len());
    let log = app.world.ops_log.lines.len();

    assert!(
        app.world.stop_work_for_quit().is_empty(),
        "no second completion"
    );
    ctrl_q(&mut app);
    assert!(app.modals.is_empty(), "no second dialog while stopping");
    assert!(
        status(&app).starts_with("Already stopping"),
        "{}",
        status(&app)
    );
    tick(&mut app, 1);

    assert_eq!(
        app.world
            .activity(&resistant)
            .and_then(|a| a.cancel.clone()),
        cancel,
        "the kill deadline did not restart"
    );
    assert_eq!(app.world.plan(ID).cloned(), plan);
    assert_eq!(app.world.docker.cleanup_uses, uses + 1);
    assert_eq!(
        app.world.activity(&resistant).map(|a| a.output.len()),
        output,
        "no second stop line"
    );
    assert!(
        app.world.ops_log.lines.len() <= log + 1,
        "one log line per tick"
    );
}

#[test]
fn quit_in_progress_refuses_new_work() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let resistant = start_activity(&mut app, "stubborn", Script::running(vec![]).resistant());
    let done = start_activity(&mut app, "finished", Script::running(vec![]));
    app.world.activity_mut(&done).expect("activity").state = ActivityState::Succeeded;
    let mut other = app.world.plan(ID).cloned().expect("plan");
    other.id = "other-plan".into();
    other.title = "Other plan".into();
    other.phase = PlanPhase::Review;
    app.world.plans.push(other);
    app.activate_tab(0);
    ctrl_q(&mut app);
    confirm_quit(&mut app);
    assert!(app.quitting && app.world.activity(&resistant).is_some());
    let activities = app.world.activities.len();
    let cleanup_uses = app.world.docker.cleanup_uses;

    app.go(Go::Exec {
        label: "echo".into(),
        argv: vec![],
    });
    assert_eq!(status(&app), crate::sim::world::STOPPING_REFUSAL);
    assert_eq!(app.world.activities.len(), activities, "no launch");

    app.go(Go::Restart(done.clone()));
    assert_eq!(
        app.world.activity(&done).expect("activity").state,
        ActivityState::Succeeded,
        "no restart"
    );

    app.world.plan_mut("other-plan").expect("plan").drift = None;
    app.pending_gate = Some(GateTarget::Plan("other-plan".into()));
    app.gate2_confirmed();
    assert_eq!(
        app.world.plan("other-plan").expect("plan").phase,
        PlanPhase::Review,
    );
    assert_eq!(app.world.docker.cleanup_uses, cleanup_uses);

    tick(&mut app, 40);
    assert!(app.shell.quit, "completion still advances");
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
