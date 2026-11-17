#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::app::App;
use jackin_console::domain::instance::InstanceId;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::screens::Go;
use jackin_console::world::World;
use ratatui::layout::Position;
use termrock::core::event::{Input, Mouse, MouseKind};
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

fn live(scenario: Scenario) -> World {
    let mut world = world_for(scenario);
    world.surfaces.vocabulary = Vocabulary::openshell();
    world.surfaces.config_path = Some(format!("{}/.config/jackin/config.toml", world.home));
    world
}

fn from_facts(scenario: Scenario) -> World {
    let mut world = world_for(scenario);
    world.surfaces = Surfaces::openshell(LiveFacts {
        version: "1.4.0".into(),
        config_path: format!("{}/.config/jackin/config.toml", world.home),
        gateway: None,
    });
    world
}

fn both(scenario: Scenario) -> [World; 2] {
    [world_for(scenario), live(scenario)]
}

fn status(app: &App) -> String {
    app.shell.status.text().unwrap_or_default().to_owned()
}

fn timed(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

fn settle(app: &mut App) -> String {
    for _ in 0..30 {
        Application::handle(app, Input::Tick);
    }
    screen(app)
}

fn dialog(text: &str, title: &str) -> String {
    let start = text
        .lines()
        .position(|l| l.contains(title))
        .unwrap_or_else(|| panic!("no `{title}` on screen:\n{text}"));
    text.lines()
        .skip(start)
        .take(5)
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn editor_save(world: World, scenario: Scenario) -> (String, String) {
    editor_save_with(world, scenario, "down e ] enter r i ctrl+s")
}

fn editor_save_with(world: World, scenario: Scenario, keys: &str) -> (String, String) {
    let mut app = timed(scenario, world);
    screen(&mut app);
    press(&mut app, keys);
    let confirm = screen(&mut app);
    press(&mut app, "right enter");
    (confirm, settle(&mut app))
}

fn settings_save(world: World, scenario: Scenario) -> (String, String) {
    let mut app = timed(scenario, world);
    screen(&mut app);
    press(&mut app, "s enter space ctrl+s");
    let confirm = screen(&mut app);
    press(&mut app, "right enter");
    (confirm, settle(&mut app))
}

#[test]
fn the_pane_menu_reason_follows_the_words() {
    let [demo, live] = both(Scenario::CapsuleMulti).map(|world| {
        let mut app = app_with(Scenario::CapsuleMulti, world);
        screen(&mut app);
        Application::handle(
            &mut app,
            Input::Mouse(Mouse::new(MouseKind::Secondary, Position::new(10, 8))),
        );
        hover_text(&mut app, "Paste");
        screen(&mut app)
    });
    assert!(demo.contains("╰─ The clipboard is empty ─╯"), "{demo}");
    assert!(live.contains("Paste last copy"), "{live}");
    assert!(live.contains("╰─ Nothing copied yet ─"), "{live}");
    assert!(!live.contains("clipboard is empty"), "{live}");
}

#[test]
fn the_save_dialogs_name_the_config_file() {
    let [demo, live] = both(Scenario::HardCases).map(|w| editor_save(w, Scenario::HardCases).0);
    assert!(demo.contains("workspace config · ~/.jackin/workspaces/"));
    assert!(live.contains("workspace config · ~/.config/jackin/config.toml"));
    assert!(!live.contains(".jackin"), "{live}");
    let [demo, live] = both(Scenario::HardCases).map(|w| settings_save(w, Scenario::HardCases).0);
    assert!(demo.contains("global config · ~/.jackin/config.toml"));
    assert!(live.contains("global config · ~/.config/jackin/config.toml"));
    assert!(!live.contains(".jackin"), "{live}");
}

#[test]
fn the_workspace_save_failure_follows_the_words() {
    let [demo, live] = both(Scenario::HardCases)
        .map(|w| dialog(&editor_save(w, Scenario::HardCases).1, "Save failed"));
    assert!(demo.contains("Error write failed: ~/.jackin/workspaces is not writable"));
    assert!(
        demo.contains("Next fix the permission and Save again"),
        "{demo}"
    );
    assert!(
        live.contains("Error Could not write ~/.config/jackin/config.toml."),
        "{live}"
    );
    assert!(
        live.contains("Next check disk space and permissions, then Save again"),
        "{live}"
    );
    assert!(
        !live.contains("EACCES") && !live.contains("fix the permission"),
        "{live}"
    );
}

#[test]
fn the_settings_save_failure_follows_the_words() {
    let [demo, live] = both(Scenario::HardCases)
        .map(|w| dialog(&settings_save(w, Scenario::HardCases).1, "Settings error"));
    assert!(demo.contains("Error write failed: ~/.jackin/config.toml is not"));
    assert!(
        demo.contains("Next fix the permission and Save again"),
        "{demo}"
    );
    assert!(
        live.contains("Error Could not write ~/.config/jackin/config.toml."),
        "{live}"
    );
    assert!(
        live.contains("Next check disk space and permissions, then Save again"),
        "{live}"
    );
    assert!(
        !live.contains("EACCES") && !live.contains("fix the permission"),
        "{live}"
    );
}

#[test]
fn the_debug_info_has_no_invented_ids() {
    let [demo, live] = both(Scenario::CapsuleMulti).map(|world| {
        let mut app = app_with(Scenario::CapsuleMulti, world);
        screen(&mut app);
        press(&mut app, "ctrl+b i");
        screen(&mut app)
    });
    assert!(
        demo.contains("Container ID") && demo.contains("3f9crun7f3ae21a"),
        "{demo}"
    );
    assert!(
        demo.contains("Invocation ID") && demo.contains("run-7f3a-4f11"),
        "{demo}"
    );
    assert!(live.contains("Sandbox") && live.contains("jackin-payments-platform-7f3a"));
    for invented in ["Sandbox ID", "Invocation ID", "3f9c", "-4f11"] {
        assert!(
            !live.contains(invented),
            "live Debug info prints {invented}:\n{live}"
        );
    }
}

#[test]
fn the_attach_status_does_not_claim_a_restore() {
    let [demo, live] = both(Scenario::Returning).map(|world| {
        let mut app = app_with(Scenario::Returning, world);
        screen(&mut app);
        app.go(Go::Attach {
            instance: InstanceId::simulated("jk-7f3a"),
            pane: None,
        });
        screen(&mut app);
        status(&app)
    });
    assert_eq!(
        demo,
        "Attached to payments-platform · tabs and panes restored"
    );
    assert_eq!(live, "Attached to payments-platform");
}

#[test]
fn unsupported_rows_say_not_supported() {
    let [demo, live] = both(Scenario::CapsuleMulti).map(|world| {
        let mut app = app_with(Scenario::CapsuleMulti, world);
        screen(&mut app);
        press(&mut app, "ctrl+\\ S t a g e");
        screen(&mut app)
    });
    assert!(demo.contains("Not in this preview"), "{demo}");
    assert!(live.contains("Not supported"), "{live}");
    assert!(!live.contains("Not available"), "{live}");
}

#[test]
fn the_live_facts_config_path_reaches_the_scope_row() {
    let (confirm, _) = editor_save_with(
        from_facts(Scenario::HardCases),
        Scenario::HardCases,
        "down e enter x ctrl+s",
    );
    assert!(
        confirm.contains("workspace config · ~/.config/jackin/config.toml"),
        "{confirm}"
    );
}

#[test]
fn the_fixed_world_names_the_config_file() {
    let (confirm, _) = editor_save_with(
        world_for(Scenario::OpenshellFixed),
        Scenario::OpenshellFixed,
        "down e enter x ctrl+s",
    );
    assert!(
        confirm.contains("workspace config · ~/.config/jackin/config.toml"),
        "{confirm}"
    );
}
