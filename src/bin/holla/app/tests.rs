use super::*;
use crate::domain::activity::{ActivityKind, ActivityState, Script, TERM_EXIT_TICKS};
use crate::domain::context::ScopeTag;
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
