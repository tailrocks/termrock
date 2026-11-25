use jackin_console::app::App;
use jackin_console::fixtures::{Scenario, app_for};
use jackin_console::world::Msg;
use termrock::motion::Motion;
use termrock::theme::{ColorLevel, Theme};

use crate::common::screen;

const LEFT: &str =
    "Cancelled · the gateway could not confirm whether sandbox jackin-demo-0001 exists";

fn app() -> App {
    app_for(
        Scenario::Returning,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

#[test]
fn what_a_cancelled_launch_could_not_settle_shows_on_the_status_line() {
    let mut app = app();
    assert_eq!(app.shell.status.text().unwrap_or_default(), "");
    app.deliver(&Msg::LaunchLeft {
        text: LEFT.to_owned(),
    });
    let shown = app.shell.status.text().unwrap_or_default().to_owned();
    assert_eq!(shown, LEFT, "{}", screen(&mut app));
}
