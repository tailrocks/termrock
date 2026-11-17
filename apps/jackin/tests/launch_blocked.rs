#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::{App, Route};
use jackin_console::domain::instance::{InstanceEntry, InstanceStatus};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use termrock::core::event::Input;
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

/// The hard-cases world launches `sre` in `infra-control-plane`; the
/// simulator blocks that launch at the sidecar stage.
fn blocked_launch() -> App {
    let mut app = app_with_world(
        Scenario::HardCases,
        world_for(Scenario::HardCases),
        Motion::Reduced,
        0,
        Theme::for_level(ColorLevel::TrueColor),
    );
    press(&mut app, "down down enter space enter");
    assert_eq!(app.route(), Route::Cockpit);
    for _ in 0..400 {
        Application::handle(&mut app, Input::Tick);
        if screen(&mut app).contains("Blocked · this stage cannot continue") {
            return app;
        }
    }
    panic!("the launch never blocked\n{}", screen(&mut app));
}

#[test]
fn a_blocked_launch_offers_the_cancel_its_status_promises() {
    let mut app = blocked_launch();
    let shown = screen(&mut app);
    assert!(
        shown
            .lines()
            .any(|l| l.trim_end().ends_with("Cancel launch…")),
        "no button\n{shown}"
    );
    press(&mut app, "esc");
    assert!(
        screen(&mut app).contains("x cancels the launch"),
        "the status that promises x"
    );
}

#[test]
fn x_cancels_a_blocked_launch_and_keeps_its_record() {
    let mut app = blocked_launch();
    let before = app.world.instances.len();
    press(&mut app, "x");
    let asked = screen(&mut app);
    assert!(
        asked.contains("Cancel the launch?"),
        "x did nothing\n{asked}"
    );
    assert_eq!(app.route(), Route::Cockpit);

    press(&mut app, "right enter");
    assert_eq!(app.route(), Route::Manager);
    assert_eq!(app.world.instances.len(), before + 1);
    let InstanceEntry::Available(record) = app.world.instances.last().expect("the launch's record")
    else {
        panic!("the simulator launch has verified metadata");
    };
    assert_eq!(record.role.as_deref(), Some("sre"));
    assert_eq!(record.workspace, Some(2));
    assert_eq!(record.status, InstanceStatus::FailedSetup);
}

#[test]
fn keeping_a_blocked_launch_leaves_it_blocked() {
    let mut app = blocked_launch();
    let before = app.world.instances.len();
    press(&mut app, "x enter");
    assert_eq!(app.route(), Route::Cockpit);
    assert_eq!(app.world.instances.len(), before);
    assert!(screen(&mut app).contains("Blocked · this stage cannot continue"));
}
