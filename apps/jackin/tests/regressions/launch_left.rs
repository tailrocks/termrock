use jackin_console::app::App;
use jackin_console::fixtures::{Scenario, app_for};
use jackin_console::world::{LaunchLeft, Msg};
use termrock::motion::Motion;
use termrock::theme::{ColorLevel, Theme};

use crate::common::screen;

const SANDBOX: &str = "jackin-demo-0001";

fn app() -> App {
    app_for(
        Scenario::Returning,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

#[test]
fn an_unconfirmed_cancel_names_the_planned_sandbox_on_the_status_line() {
    let mut app = app();
    assert_eq!(status(&app), "");
    app.deliver(&Msg::LaunchLeft {
        left: LaunchLeft::Unconfirmed {
            sandbox: SANDBOX.to_owned(),
        },
    });
    assert_eq!(
        status(&app),
        format!(
            "Cancelled · the gateway could not confirm whether sandbox {SANDBOX} exists · the fleet shows the current state"
        ),
        "{}",
        screen(&mut app)
    );
}

#[test]
fn an_ambiguous_cancel_names_every_sandbox_that_matches() {
    let mut app = app();
    app.deliver(&Msg::LaunchLeft {
        left: LaunchLeft::Ambiguous {
            sandboxes: vec!["jackin-a-0001".to_owned(), "jackin-b-0002".to_owned()],
        },
    });
    assert_eq!(
        status(&app),
        "Cancelled · several sandboxes match this launch: jackin-a-0001, jackin-b-0002 · check the fleet before launching again"
    );
}

#[test]
fn an_unreadable_sandbox_after_a_cancel_is_named_with_the_reason() {
    let mut app = app();
    app.deliver(&Msg::LaunchLeft {
        left: LaunchLeft::Unread {
            sandbox: SANDBOX.to_owned(),
        },
    });
    assert!(
        status(&app).starts_with(&format!("Cancelled · sandbox {SANDBOX} was seen")),
        "{}",
        status(&app)
    );
}
