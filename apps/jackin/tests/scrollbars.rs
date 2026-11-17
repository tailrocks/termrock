#![cfg(feature = "demo")]

mod common;

use common::*;
use jackin_console::domain::workspace::{EnvVar, Workspace};
use jackin_console::fixtures::{Scenario, world_for};

const TALL: (u16, u16) = (140, 40);
const SHORT: (u16, u16) = (140, 20);

#[test]
fn the_manager_list_thumb_follows_the_mouse() {
    let mut world = world_for(Scenario::Returning);
    for i in 0..40 {
        world
            .workspaces
            .push(Workspace::new(100 + i, &format!("ws-{i}"), "/w"));
    }
    let mut app = app_with(Scenario::Returning, world);
    the_thumb_follows_the_mouse_cell_for_cell(&mut app, (140, 24), 0);
}

#[test]
fn the_accounts_tree_thumb_follows_the_mouse() {
    let mut app = app(Scenario::AccountsMixed);
    the_thumb_follows_the_mouse_cell_for_cell(&mut app, SHORT, 0);
}

#[test]
fn the_usage_list_thumb_follows_the_mouse() {
    let mut app = app(Scenario::Returning);
    press(&mut app, "u");
    the_thumb_follows_the_mouse_cell_for_cell(&mut app, SHORT, 0);
}

#[test]
fn the_workspace_editor_role_and_account_thumbs_follow_the_mouse() {
    let mut app = app(Scenario::Returning);
    press(&mut app, "e right right");
    the_thumb_follows_the_mouse_cell_for_cell(&mut app, TALL, 0);
    press(&mut app, "right right");
    the_thumb_follows_the_mouse_cell_for_cell(&mut app, SHORT, 0);
}

#[test]
fn the_settings_environment_thumb_follows_the_mouse() {
    let mut world = world_for(Scenario::Returning);
    for i in 0..12 {
        world
            .global
            .env
            .push(EnvVar::plain(&format!("VAR_{i:02}"), "x"));
    }
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "s right right");
    the_thumb_follows_the_mouse_cell_for_cell(&mut app, SHORT, 0);
}

fn thumb_start(app: &mut jackin_console::app::App, size: (u16, u16)) -> u16 {
    thumbs(&screen_at(app, size.0, size.1))[0].start
}

fn wheel_down(app: &mut jackin_console::app::App, size: (u16, u16), x: u16, y: u16) {
    use termrock::core::event::{Input, Mouse, MouseKind};
    use termrock::runtime::Application;
    for _ in 0..3 {
        Application::handle(
            app,
            Input::Mouse(Mouse::new(
                MouseKind::WheelDown,
                ratatui::layout::Position::new(x, y),
            )),
        );
        screen_at(app, size.0, size.1);
    }
}

#[test]
fn the_wheel_moves_the_manager_list_away_from_the_cursor() {
    let mut world = world_for(Scenario::Returning);
    for i in 0..40 {
        world
            .workspaces
            .push(Workspace::new(100 + i, &format!("ws-{i}"), "/w"));
    }
    let mut app = app_with(Scenario::Returning, world);
    let size = (140, 24);
    let before = thumb_start(&mut app, size);
    let x = thumbs(&screen_at(&mut app, size.0, size.1))[0].x;
    wheel_down(&mut app, size, x, 6);
    assert!(
        thumb_start(&mut app, size) > before,
        "the list stays where the wheel put it"
    );
}

#[test]
fn the_wheel_moves_the_accounts_tree_away_from_the_cursor() {
    let mut app = app(Scenario::AccountsMixed);
    let size = SHORT;
    let before = thumb_start(&mut app, size);
    let x = thumbs(&screen_at(&mut app, size.0, size.1))[0].x;
    wheel_down(&mut app, size, x, 8);
    assert!(
        thumb_start(&mut app, size) > before,
        "the tree stays where the wheel put it"
    );
}
