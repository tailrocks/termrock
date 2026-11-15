use jackin_console::app::App;
use jackin_console::fixtures::{Scenario, app_for};
use jackin_console::world::Msg;
use junie_tui::core::event::Input;
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};

use crate::common::{press, screen};

const DISCARDED: &str = "Discarded 1 change";
const FAILED: &str = "Refresh failed: instance index unreadable";

fn app(scenario: Scenario) -> App {
    app_for(
        scenario,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn refresh_fails(app: &mut App) {
    app.world.refresh_from = Some(app.world.snapshots_applied);
    app.deliver(&Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Err("instance index unreadable".into()),
    });
}

fn discard_a_settings_change(app: &mut App) {
    press(app, "s");
    press(app, "enter space");
    for _ in 0..4 {
        if screen(app).contains("Unsaved changes") {
            break;
        }
        press(app, "esc");
    }
    let leaving = screen(app);
    assert!(leaving.contains("Unsaved changes"), "{leaving}");
    press(app, "right enter");
}

#[test]
fn a_background_refresh_failure_does_not_replace_the_discard_confirmation() {
    let mut app = app(Scenario::Returning);
    discard_a_settings_change(&mut app);
    assert_eq!(status(&app), DISCARDED, "{}", screen(&mut app));

    refresh_fails(&mut app);
    assert_eq!(status(&app), DISCARDED, "{}", screen(&mut app));
    assert!(!screen(&mut app).contains("Refresh failed"));

    app.world.clock.advance(6_000);
    Application::handle(&mut app, Input::Tick);
    assert_eq!(status(&app), "", "the confirmation expired");
    refresh_fails(&mut app);
    assert!(status(&app).starts_with(FAILED), "{}", status(&app));
}

#[test]
fn a_refresh_failure_with_nothing_on_screen_shows_at_once() {
    let mut app = app(Scenario::Returning);
    refresh_fails(&mut app);
    assert!(status(&app).starts_with(FAILED), "{}", status(&app));
}

#[test]
fn a_refresh_the_user_asked_for_reports_its_failure_over_the_refreshing_status() {
    let mut app = app(Scenario::Returning);
    press(&mut app, "f5");
    assert_eq!(status(&app), "Refreshing…");
    refresh_fails(&mut app);
    assert!(status(&app).starts_with(FAILED), "{}", status(&app));
}
