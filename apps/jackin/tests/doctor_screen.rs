#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::domain::doctor::DoctorState;
use jackin_console::fixtures::{Scenario, world_for};

#[test]
fn the_first_run_in_progress_says_so_and_offers_no_second_run() {
    let mut world = world_for(Scenario::Doctor);
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::Running { previous: None };
    let mut app = app_with(Scenario::Doctor, world);
    press(&mut app, "D");

    let shown = screen(&mut app);
    assert!(shown.contains("Running checks…"), "{shown}");
    assert!(!shown.contains("Checks have not run yet"), "{shown}");
    assert!(!shown.contains("Run checks"), "{shown}");
}

#[test]
fn a_doctor_that_never_ran_offers_the_first_run() {
    let mut world = world_for(Scenario::Doctor);
    world.surfaces.doctor.as_mut().expect("doctor").state = DoctorState::NeverRun;
    let mut app = app_with(Scenario::Doctor, world);
    press(&mut app, "D");

    let shown = screen(&mut app);
    assert!(shown.contains("Checks have not run yet"), "{shown}");
    assert!(shown.contains("Run checks"), "{shown}");
}

#[test]
fn a_report_with_only_warnings_does_not_say_all_checks_pass() {
    let mut world = world_for(Scenario::Doctor);
    let doctor = world.surfaces.doctor.as_mut().expect("doctor");
    let mut report = doctor.report().expect("a report").clone();
    for (i, check) in report.checks.iter_mut().enumerate() {
        check.status = if i == 0 {
            junie_tui::theme::StatusKind::Warn
        } else {
            junie_tui::theme::StatusKind::Ok
        };
    }
    doctor.state = DoctorState::Done(report);
    let mut app = app_with(Scenario::Doctor, world);
    press(&mut app, "D");

    let shown = screen(&mut app);
    assert!(shown.contains("attention · 1 warning · checked"), "{shown}");
    assert!(!shown.contains("all checks pass"), "{shown}");
}

fn ticks(app: &mut jackin_console::app::App, n: usize) -> (i64, u64) {
    use junie_tui::core::event::Input;
    use junie_tui::runtime::Application;

    app.world.clock.running = true;
    let (secs, changed) = (app.world.now_secs(), app.world.work().changed_ticks);
    for _ in 0..n {
        Application::handle(app, Input::Tick);
        screen(app);
    }
    (
        app.world.now_secs() - secs,
        app.world.work().changed_ticks - changed,
    )
}

#[test]
fn a_doctor_page_with_no_age_to_show_requests_no_time_only_repaint() {
    use jackin_console::domain::doctor::DoctorFailure;

    let states = [
        DoctorState::NeverRun,
        DoctorState::Failed {
            previous: None,
            failure: DoctorFailure::TimedOut,
            at_secs: 0,
        },
    ];
    for state in states {
        let mut world = world_for(Scenario::Doctor);
        world.surfaces.doctor.as_mut().expect("doctor").state = state;
        let mut app = app_with(Scenario::Doctor, world);
        press(&mut app, "D");
        let shown = screen(&mut app);
        assert!(!shown.contains(" ago"), "no age on the empty page: {shown}");
        ticks(&mut app, 1);

        let (seconds, repainted) = ticks(&mut app, 60);
        assert!(seconds >= 3, "the clock crossed whole seconds: {seconds}");
        assert_eq!(repainted, 0, "an empty page has nothing to repaint");
    }
}

#[test]
fn a_doctor_report_repaints_once_per_whole_second_of_its_age() {
    let mut app = app(Scenario::Doctor);
    press(&mut app, "D");
    assert!(screen(&mut app).contains("checked"), "a report is shown");
    ticks(&mut app, 1);

    let (seconds, repainted) = ticks(&mut app, 60);
    assert!(seconds >= 3, "the clock crossed whole seconds: {seconds}");
    assert!(
        (seconds as u64..=seconds as u64 + 1).contains(&repainted),
        "one repaint per second boundary, not per tick: {repainted} for {seconds} s"
    );
}
