use super::*;
use crate::domain::activity::{ActivityState, Script};
use crate::domain::context::ScopeTag;
use crate::domain::effect::Effect;
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
