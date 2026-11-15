use super::*;
use crate::domain::activity::{ActivityKind, ActivityState, Script, TERM_EXIT_TICKS};
use crate::domain::context::ScopeTag;
use crate::domain::effect::Effect;
use crate::screens::finder::FinderPage;
use crate::sim::world::{Batch, BatchMember, BatchMode};
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

fn finish_with_failed_images(app: &mut App) {
    app.world
        .plan_mut(ID)
        .expect("plan")
        .steps
        .iter_mut()
        .find(|s| s.id == "images")
        .expect("step")
        .fails = true;
    arm(app, ID);
    for _ in 0..400 {
        if app.world.plan(ID).expect("plan").phase == PlanPhase::Done {
            return;
        }
        tick(app, 1);
    }
    panic!("plan never finished");
}

fn retry_images(app: &mut App) {
    let plan = app.world.plan_mut(ID).expect("plan");
    let images = plan
        .steps
        .iter()
        .position(|s| s.id == "images")
        .expect("step");
    assert!(plan.retry(images));
}

fn confirm_cancel_remaining(app: &mut App) {
    let tab = plan_tab(app, ID).expect("plan tab open");
    app.activate_tab(tab);
    app.handle_input(key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    assert!(!app.modals.is_empty(), "the cancel dialog is open");
    app.shell
        .focus
        .set(Some(WidgetId::of("cancel-plan").sub("ok")));
    app.handle_input(key(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.modals.is_empty(), "the dialog resolved");
}

#[test]
fn plan_cancel_remaining_with_nothing_running_publishes_one_completion() {
    let mut app = app();
    finish_with_failed_images(&mut app);
    let finished = status(&app);
    assert!(finished.contains("failed"), "{finished}");
    let uses = app.world.docker.cleanup_uses;
    retry_images(&mut app);
    assert_eq!(app.world.plan(ID).expect("plan").phase, PlanPhase::Running);
    assert!(app.world.plan(ID).expect("plan").running().is_empty());

    confirm_cancel_remaining(&mut app);
    assert_eq!(step(&app, ID, "images"), StepState::Cancelled);
    tick(&mut app, 1);

    let plan = app.world.plan(ID).expect("plan");
    assert_eq!(plan.phase, PlanPhase::Done);
    assert!(plan.ended_tick.is_some());
    let text = status(&app);
    assert!(
        text.starts_with("Clean Docker completely finished · "),
        "{text}"
    );
    assert!(text.contains("cancelled"), "{text}");
    assert!(!text.contains("stopped"), "{text}");
    assert_eq!(app.world.docker.cleanup_uses, uses, "no repeated effects");

    let snapshot = app.world.plan(ID).cloned();
    tick(&mut app, 120);
    assert_eq!(
        app.world.plan(ID).cloned(),
        snapshot,
        "ticks change nothing"
    );
}

#[test]
fn plan_stop_wording_differs_from_cancel_remaining() {
    let mut app = app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    let tab = plan_tab(&app, ID).expect("plan tab open");
    app.activate_tab(tab);
    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    confirm_close_dialog(&mut app);
    assert!(status(&app).contains(" stopped · "), "{}", status(&app));

    let mut app = self::app();
    arm(&mut app, ID);
    tick_until_running(&mut app, ID, "rm");
    confirm_cancel_remaining(&mut app);
    for _ in 0..400 {
        if app.world.plan(ID).expect("plan").phase == PlanPhase::Done {
            break;
        }
        tick(&mut app, 1);
    }
    let text = status(&app);
    assert!(text.contains(" finished · "), "{text}");
    assert!(!text.contains("stopped"), "{text}");
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

use crate::domain::cleanup::{DeleteItem, DeletePlan, Mode};

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

const BATCH: &str = "batch";

fn member(name: &str, script: Script) -> BatchMember {
    (name.into(), vec![], script, None, ScopeTag::here("/work"))
}

fn member_with_effect(name: &str, script: Script, effect: Effect) -> BatchMember {
    (
        name.into(),
        vec![],
        script,
        Some(effect),
        ScopeTag::here("/work"),
    )
}

fn start_batch(app: &mut App, mode: BatchMode, members: Vec<BatchMember>) -> Vec<String> {
    app.world
        .start_batch(BATCH, "Batch", mode, members, "Batch")
}

fn state_of(app: &App, id: &str) -> ActivityState {
    app.world.activity(id).expect("activity").state
}

fn batch(app: &App) -> &Batch {
    app.world
        .batches
        .iter()
        .find(|b| b.id == BATCH)
        .expect("batch")
}

fn tally(app: &App) -> (usize, usize, usize, usize) {
    batch(app).tally(&app.world)
}

fn activity_tabs(app: &App, id: &str) -> usize {
    app.tabs
        .iter()
        .filter(|t| t.kind == TabKind::Activity(id.to_owned()))
        .count()
}

fn tick_until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    for _ in 0..400 {
        if done(&*app) {
            return;
        }
        tick(app, 1);
    }
    panic!(
        "{what} never happened by tick {}: {}",
        app.world.tick,
        status(app)
    );
}

fn close_tab_of(app: &mut App, id: &str) {
    let i = app
        .tabs
        .iter()
        .position(|t| t.kind == TabKind::Activity(id.to_owned()))
        .expect("activity tab open");
    app.activate_tab(i);
    app.handle_input(key(KeyCode::Char('w'), KeyModifiers::CONTROL));
    assert_eq!(activity_tabs(app, id), 0, "the tab closed");
}

fn assert_members_retained(app: &App) {
    for b in &app.world.batches {
        for m in &b.members {
            assert!(app.world.activity(m).is_some(), "batch lost member {m}");
        }
    }
}

#[test]
fn closing_finished_batch_member_retains_terminal_tally() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Sequential,
        vec![
            member("first", Script::ending(vec![], 2, 0)),
            member("second", Script::ending(vec![], 2, 0)),
        ],
    );
    app.open_activity_tab(&ids[0], true);
    tick_until(&mut app, "first finishes", |app| {
        state_of(app, &ids[0]).finished()
    });
    close_tab_of(&mut app, &ids[0]);
    assert_members_retained(&app);
    assert_eq!(tally(&app), (1, 0, 0, 1), "first keeps its outcome");

    tick_until(&mut app, "batch summary", |app| batch(app).summarised);
    assert_eq!(tally(&app), (2, 0, 0, 0));
    assert!(batch(&app).ok(&app.world));
    assert!(status(&app).contains("2 ok"), "{}", status(&app));
    assert_eq!(
        activity_tabs(&app, &ids[0]),
        0,
        "the closed tab stays closed"
    );
}

#[test]
fn closing_parallel_failed_or_stopped_members_keeps_outcomes() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Parallel,
        vec![
            member("failing", Script::ending(vec![], 2, 1)),
            member("long", Script::ending(vec![], 400, 0)),
            member("succeeding", Script::ending(vec![], 3, 0)),
        ],
    );
    for id in &ids {
        app.open_activity_tab(id, false);
    }
    tick_until(&mut app, "failing member fails", |app| {
        state_of(app, &ids[0]) == ActivityState::Failed
    });
    let tick = app.world.tick;
    app.world.activity_mut(&ids[1]).expect("long").stop(tick);
    tick_until(&mut app, "stopped member stops", |app| {
        state_of(app, &ids[1]) == ActivityState::Stopped
    });
    tick_until(&mut app, "batch summary", |app| batch(app).summarised);
    assert_eq!(tally(&app), (1, 1, 1, 0));
    let summary = status(&app);

    close_tab_of(&mut app, &ids[2]);
    close_tab_of(&mut app, &ids[0]);
    close_tab_of(&mut app, &ids[1]);

    assert_members_retained(&app);
    assert_eq!(tally(&app), (1, 1, 1, 0), "closing tabs changes no outcome");
    assert!(!batch(&app).ok(&app.world));
    assert_eq!(status(&app), summary, "the summary is not published again");
}

#[test]
fn closing_last_member_before_batch_summary_still_completes() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Parallel,
        vec![
            member("early", Script::ending(vec![], 2, 0)),
            member("late", Script::ending(vec![], 6, 0)),
        ],
    );
    app.open_activity_tab(&ids[0], true);
    app.open_activity_tab(&ids[1], false);
    tick_until(&mut app, "early finishes", |app| {
        state_of(app, &ids[0]).finished()
    });
    close_tab_of(&mut app, &ids[0]);
    tick_until(&mut app, "batch summary", |app| batch(app).summarised);
    assert_eq!(tally(&app), (2, 0, 0, 0));
    assert!(status(&app).contains("2 ok"), "{}", status(&app));
}

#[test]
fn batch_history_survives_tab_index_shift() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Parallel,
        vec![
            member("first", Script::ending(vec![], 2, 0)),
            member("second", Script::ending(vec![], 20, 0)),
        ],
    );
    app.open_activity_tab(&ids[0], true);
    app.open_activity_tab(&ids[1], false);
    tick_until(&mut app, "first finishes", |app| {
        state_of(app, &ids[0]).finished()
    });
    close_tab_of(&mut app, &ids[0]);
    assert_eq!(tally(&app), (1, 0, 0, 1), "before the next tick");
    tick(&mut app, 3);
    assert_members_retained(&app);
    assert_eq!(tally(&app), (1, 0, 0, 1), "after the next tick");
    tick_until(&mut app, "batch summary", |app| batch(app).summarised);
    assert_eq!(tally(&app), (2, 0, 0, 0));
}

#[test]
fn reopening_completed_activity_does_not_restart() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Sequential,
        vec![
            member("first", Script::ending(vec![], 2, 0)),
            member("second", Script::ending(vec![], 30, 0)),
        ],
    );
    app.open_activity_tab(&ids[0], true);
    tick_until(&mut app, "first finishes", |app| {
        state_of(app, &ids[0]).finished()
    });
    let finished = app.world.activity(&ids[0]).cloned();
    let records = app.world.activities.len();
    close_tab_of(&mut app, &ids[0]);

    app.open_activity_tab(&ids[0], true);
    app.open_activity_tab(&ids[0], true);
    assert_eq!(activity_tabs(&app, &ids[0]), 1, "reopening reuses one tab");
    tick(&mut app, 10);

    assert_eq!(app.world.activity(&ids[0]).cloned(), finished);
    assert_eq!(app.world.activities.len(), records, "no new activity");
    assert_members_retained(&app);
}

#[test]
fn reopening_completed_activity_does_not_reapply_its_effect() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Sequential,
        vec![
            member_with_effect(
                "restart",
                Script::ending(vec![], 2, 0),
                Effect::ServiceRestart("nginx.service".into()),
            ),
            member("second", Script::ending(vec![], 30, 0)),
        ],
    );
    app.open_activity_tab(&ids[0], true);
    tick_until(&mut app, "first finishes", |app| {
        state_of(app, &ids[0]).finished()
    });
    assert_eq!(app.world.healed_units, ["nginx.service"]);

    app.world.healed_units.clear();
    close_tab_of(&mut app, &ids[0]);
    app.open_activity_tab(&ids[0], true);
    tick(&mut app, 10);

    assert!(
        app.world.healed_units.is_empty(),
        "reopening must not run the effect again"
    );
    assert_eq!(state_of(&app, &ids[0]), ActivityState::Succeeded);
}

#[test]
fn every_batch_member_retains_activity_after_tab_operations() {
    let mut app = app();
    let ids = start_batch(
        &mut app,
        BatchMode::Parallel,
        vec![
            member("first", Script::ending(vec![], 2, 0)),
            member("second", Script::ending(vec![], 4, 1)),
            member("third", Script::ending(vec![], 6, 0)),
        ],
    );
    for id in &ids {
        app.open_activity_tab(id, false);
        assert_members_retained(&app);
    }
    tick_until(&mut app, "all members finish", |app| {
        batch(app).done(&app.world)
    });
    for id in [&ids[2], &ids[0], &ids[1]] {
        close_tab_of(&mut app, id);
        assert_members_retained(&app);
    }
    app.open_activity_tab(&ids[1], true);
    assert_members_retained(&app);
    tick(&mut app, 10);
    assert_members_retained(&app);
    assert_eq!(tally(&app), (2, 1, 0, 0));
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

fn tick_until_step(app: &mut App, id: &str, step_id: &str, state: StepState) {
    for _ in 0..400 {
        if step(app, id, step_id) == state {
            return;
        }
        tick(app, 1);
    }
    panic!("{step_id} never reached {state:?}: status={}", status(app));
}

#[test]
fn plan_retry_emits_template_after_history_marker() {
    let mut app = app();
    let template = app.world.step_output["docker-cleanup/images"].clone();
    app.world
        .plan_mut(ID)
        .expect("plan")
        .steps
        .iter_mut()
        .find(|s| s.id == "images")
        .expect("step")
        .fails = true;
    arm(&mut app, ID);
    tick_until_step(&mut app, ID, "images", StepState::Failed);
    let tab = plan_tab(&app, ID).expect("plan tab open");
    app.activate_tab(tab);
    for _ in 0..400 {
        if app.world.plan(ID).expect("plan").phase == PlanPhase::Done {
            break;
        }
        tick(&mut app, 1);
    }

    app.handle_input(key(KeyCode::Char('r'), KeyModifiers::NONE));
    tick_until_step(&mut app, ID, "images", StepState::Succeeded);

    let output = &app
        .world
        .plan(ID)
        .expect("plan")
        .steps
        .iter()
        .find(|s| s.id == "images")
        .expect("step")
        .output;
    assert_eq!(
        output.len(),
        template.len() * 2 + 2,
        "history stays, then the retry emits the whole template once: {output:?}"
    );
    assert_eq!(output[template.len()], "exit status 1");
    assert_eq!(output[template.len() + 1], "retrying…");
    assert_eq!(output[template.len() + 2..], template[..]);
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

use crate::domain::activity::ScriptLine;
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
    app.world.edit_catalog(|w| {
        w.explore_tiles = explore_tiles(&[
            "explore.system",
            "explore.tasks",
            "explore.files",
            "explore.git",
            "explore.disk",
            "explore.services",
        ])
    });
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

    app.world.edit_catalog(|w| {
        w.explore_tiles = explore_tiles(&[
            "explore.system",
            "explore.disk",
            "explore.files",
            "explore.tasks",
            "explore.git",
        ])
    });
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

/// Explore tiles in `order`, with the Git summary long enough to overflow the
/// preview pane so its scroll offset can move.
fn long_git_explore_tiles(order: &[&str]) -> Vec<crate::domain::catalog::ExploreTile> {
    let words: Vec<String> = (0..600).map(|i| format!("L{i:03}")).collect();
    let summary: &'static str = Box::leak(words.join(" ").into_boxed_str());
    let mut tiles = explore_tiles(order);
    tiles
        .iter_mut()
        .find(|t| t.id == "explore.git")
        .expect("git tile")
        .summary = summary;
    tiles
}

fn preview_top_word(app: &mut App) -> String {
    drawn_text(app)
        .lines()
        .find_map(|line| {
            line.split_whitespace()
                .find(|w| {
                    w.len() == 4 && w.starts_with('L') && w[1..].bytes().all(|b| b.is_ascii_digit())
                })
                .map(str::to_owned)
        })
        .expect("preview word drawn")
}

#[test]
fn finder_explore_reorder_keeps_preview_scroll() {
    let mut app = finder_app();
    app.world.edit_catalog(|w| {
        w.explore_tiles = long_git_explore_tiles(&[
            "explore.system",
            "explore.tasks",
            "explore.files",
            "explore.git",
            "explore.disk",
            "explore.services",
        ])
    });
    tick(&mut app, 1);
    finder_select_explore(&mut app, "explore.git");
    drawn_text(&mut app);
    click_at(&mut app, 100, 9);
    for _ in 0..3 {
        app.handle_input(key(KeyCode::Down, KeyModifiers::NONE));
    }
    let scrolled = preview_top_word(&mut app);
    assert_ne!(
        scrolled, "L000",
        "preview did not scroll before the rebuild"
    );

    app.world.edit_catalog(|w| {
        w.explore_tiles = long_git_explore_tiles(&[
            "explore.system",
            "explore.disk",
            "explore.files",
            "explore.tasks",
            "explore.git",
            "explore.services",
        ])
    });
    tick(&mut app, 1);

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.git")
    );
    assert_eq!(preview_top_word(&mut app), scrolled);
}

fn long_explore_tiles(order: &[&str]) -> Vec<crate::domain::catalog::ExploreTile> {
    let words: Vec<String> = (0..600).map(|i| format!("L{i:03}")).collect();
    let summary: &'static str = Box::leak(words.join(" ").into_boxed_str());
    let mut tiles = explore_tiles(order);
    for tile in &mut tiles {
        tile.summary = summary;
    }
    tiles
}

fn scroll_preview_down(app: &mut App, times: usize) {
    use junie_tui::core::event::{Mouse, MouseKind};
    let at = ratatui::layout::Position::new(100, 9);
    for _ in 0..times {
        app.handle_input(Input::Mouse(Mouse::new(MouseKind::WheelDown, at)));
    }
}

#[test]
fn finder_explore_key_selection_change_resets_preview_scroll() {
    let mut app = finder_app();
    app.world.edit_catalog(|w| {
        w.explore_tiles = long_explore_tiles(&[
            "explore.system",
            "explore.tasks",
            "explore.files",
            "explore.git",
            "explore.disk",
            "explore.services",
        ])
    });
    tick(&mut app, 1);
    finder_select_explore(&mut app, "explore.git");
    drawn_text(&mut app);
    scroll_preview_down(&mut app, 3);
    assert_ne!(
        preview_top_word(&mut app),
        "L000",
        "preview did not scroll before the key move"
    );

    app.handle_input(key(KeyCode::Right, KeyModifiers::NONE));

    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.disk")
    );
    assert_eq!(preview_top_word(&mut app), "L000");
}

#[test]
fn finder_explore_click_selection_change_resets_preview_scroll() {
    let mut app = finder_app();
    app.world.edit_catalog(|w| {
        w.explore_tiles = long_explore_tiles(&[
            "explore.system",
            "explore.tasks",
            "explore.files",
            "explore.git",
            "explore.disk",
            "explore.services",
        ])
    });
    tick(&mut app, 1);
    finder_select_explore(&mut app, "explore.git");
    drawn_text(&mut app);
    scroll_preview_down(&mut app, 3);
    assert_ne!(
        preview_top_word(&mut app),
        "L000",
        "preview did not scroll before the click"
    );

    let (x, y) = drawn_tile(&mut app, "Disk");
    click_at(&mut app, x + 1, y);
    assert_eq!(finder_group(&mut app), Some("Disk"));
    app.handle_input(key(KeyCode::Esc, KeyModifiers::NONE));

    assert_eq!(finder_group(&mut app), None);
    assert_eq!(
        finder(&mut app).current_id().as_deref(),
        Some("explore.disk")
    );
    assert_eq!(preview_top_word(&mut app), "L000");
}

#[test]
fn finder_empty_explore_row_is_not_selectable() {
    let mut app = finder_app();
    finder_select_explore(&mut app, "explore.git");

    app.world.edit_catalog(|w| w.explore_tiles = vec![]);
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
    app.world.edit_memory(|w| {
        w.memory
            .usage
            .actions
            .push(crate::domain::usage::ActionUse {
                item: "explore.git".into(),
                path: Some(cwd),
                host,
                stamps: vec![now],
            })
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

    app.world.edit_memory(|w| w.memory.usage.actions.clear());
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
