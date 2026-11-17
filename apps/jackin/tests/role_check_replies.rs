#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::App;
use jackin_console::backend::{Backend, Completion, Effect, FileSource};
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::role::{FindingArea, RoleCheckState, RoleFinding};
use jackin_console::fixtures::{Scenario, app_with_world, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::screens::Go;
use jackin_console::sim::{self, SimBackend};
use jackin_console::world::{Msg, World};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use termrock::core::event::{Input, Key};
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::theme::{ColorLevel, Theme};

const CHECK_TAKES_MS: i64 = 600;

#[derive(Debug)]
struct SlowChecker {
    inner: Arc<dyn Backend>,
    seen: Mutex<Vec<Effect>>,
}

impl SlowChecker {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: SimBackend::shared(),
            seen: Mutex::new(vec![]),
        })
    }

    fn checks(&self) -> Vec<String> {
        let seen = self.seen.lock().expect("seen");
        seen.iter()
            .filter_map(|e| match e {
                Effect::CheckRole { doc, .. } => Some(doc.codex_model.clone().unwrap_or_default()),
                _ => None,
            })
            .collect()
    }

    fn saves(&self) -> usize {
        let seen = self.seen.lock().expect("seen");
        seen.iter()
            .filter(|e| matches!(e, Effect::SaveRole { .. }))
            .count()
    }
}

impl Backend for SlowChecker {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.seen.lock().expect("seen").push(effect.clone());
        let Effect::CheckRole { ticket, role, doc } = effect else {
            return self.inner.perform(effect, world);
        };
        let mut snapshot = sim::snapshot(world);
        let rec = snapshot.surfaces.roles.as_mut()?.role_mut(role)?;
        let model = doc.codex_model.clone().unwrap_or_default();
        let findings = if model.ends_with('x') {
            vec![RoleFinding {
                area: FindingArea::Policy,
                key: Some("role.policy".into()),
                message: format!("problem in the document with model {model}"),
                fix: None,
            }]
        } else {
            vec![]
        };
        rec.check = RoleCheckState::Done(findings);
        Some(Completion::ready(
            CHECK_TAKES_MS,
            Msg::RoleChecked {
                ticket: *ticket,
                role: role.clone(),
                result: Ok(Box::new(snapshot)),
            },
        ))
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

fn slow_editor() -> (App, Arc<SlowChecker>) {
    let backend = SlowChecker::new();
    let mut world = world_for(Scenario::RoleEditor);
    world.backend = backend.clone();
    let app = app_with_world(
        Scenario::RoleEditor,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    (app, backend)
}

fn wait_for_the_reply(app: &mut App) -> String {
    for _ in 0..40 {
        Application::handle(app, Input::Tick);
    }
    screen(app)
}

fn check_key(app: &mut App) {
    Application::handle(
        app,
        Input::Key(Key {
            code: KeyCode::F(5),
            mods: KeyModifiers::NONE,
        }),
    );
}

const PIN_THE_MODEL: &str = "enter tab tab tab tab tab p i n enter";

#[test]
fn an_edit_during_the_check_cancels_the_save_preview() {
    let (mut app, backend) = slow_editor();
    press(&mut app, PIN_THE_MODEL);
    press(&mut app, "ctrl+s");
    press(&mut app, "enter x enter");
    let shown = wait_for_the_reply(&mut app);

    assert!(!shown.contains("Save role"), "{shown}");
    assert!(
        shown.contains("Not checked since your last edit"),
        "{shown}"
    );
    assert_eq!(backend.saves(), 0);
}

#[test]
fn a_second_check_waits_for_the_running_one() {
    let (mut app, backend) = slow_editor();
    press(&mut app, PIN_THE_MODEL);
    check_key(&mut app);
    press(&mut app, "enter x enter");
    check_key(&mut app);

    assert_eq!(backend.checks(), vec!["pin".to_owned()]);
    assert_eq!(
        app.shell.status.text().unwrap_or_default(),
        "A check is already running"
    );
    let shown = wait_for_the_reply(&mut app);
    assert!(
        shown.contains("Not checked since your last edit"),
        "{shown}"
    );

    check_key(&mut app);
    assert_eq!(backend.checks(), vec!["pin".to_owned(), "pinx".to_owned()]);
    let shown = wait_for_the_reply(&mut app);
    assert!(
        shown.contains("problem in the document with model pinx"),
        "{shown}"
    );
}

#[test]
fn a_check_of_a_closed_editor_does_not_answer_the_reopened_one() {
    let (mut app, backend) = slow_editor();
    press(&mut app, "enter tab tab tab tab tab p i n x enter");
    press(&mut app, "ctrl+s");
    press(&mut app, "esc esc right enter");
    press(&mut app, "e");
    check_key(&mut app);
    assert_eq!(backend.checks(), vec!["pinx".to_owned(), String::new()]);

    let shown = wait_for_the_reply(&mut app);
    assert!(
        !shown.contains("problem in the document with model pinx"),
        "the old editor's findings answered the new check:\n{shown}"
    );
    assert!(shown.contains("Check passed"), "{shown}");
}

#[derive(Debug)]
struct FlagEveryRole {
    inner: Arc<dyn Backend>,
}

impl Backend for FlagEveryRole {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        let Effect::CheckRole { ticket, role, .. } = effect else {
            return self.inner.perform(effect, world);
        };
        let mut snapshot = sim::snapshot(world);
        snapshot.surfaces.roles.as_mut()?.role_mut(role)?.check =
            RoleCheckState::Done(vec![RoleFinding {
                area: FindingArea::Policy,
                key: None,
                message: format!("{role} is flagged"),
                fix: None,
            }]);
        Some(Completion::ready(
            CHECK_TAKES_MS,
            Msg::RoleChecked {
                ticket: *ticket,
                role: role.clone(),
                result: Ok(Box::new(snapshot)),
            },
        ))
    }

    fn settle(&self, effect: &Effect, world: &World) -> Option<Msg> {
        self.inner.settle(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.inner.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.inner.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.inner.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.inner.initial_panes(instance)
    }

    fn file_source(&self, world: &World) -> Box<dyn FileSource> {
        self.inner.file_source(world)
    }
}

#[test]
fn check_all_keeps_the_result_of_every_role() {
    let mut world = world_for(Scenario::RoleEditor);
    world.backend = Arc::new(FlagEveryRole {
        inner: SimBackend::shared(),
    });
    let mut app = app_with_world(
        Scenario::RoleEditor,
        world,
        Motion::Reduced,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    );
    app.go(Go::Roles { select: None });
    check_key(&mut app);
    wait_for_the_reply(&mut app);

    let roles = &app.world.surfaces.roles.as_ref().expect("roles").roles;
    let flagged: Vec<&str> = roles
        .iter()
        .filter(|r| !r.findings().is_empty())
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(
        flagged,
        ["sentinel", "the-architect", "reviewer", "data-eng"],
        "{roles:#?}"
    );
}
