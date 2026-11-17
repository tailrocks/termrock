#![cfg(not(feature = "demo"))]

mod product_support;

use std::collections::BTreeMap;
use std::sync::Arc;

use product_support::{NOT_USED, SignedInOnly};

use jackin_console::app::{App, AppOptions, ClipboardMode, Entry, EntryPage, Remote, assemble};
use jackin_console::backend::{Backend, Completion, Effect};
use jackin_console::domain::account::AccountRegistry;
use jackin_console::domain::instance::Instance;
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::workspace::{
    AccountPolicy, AllowedRoles, DirtyExitPolicy, RolePolicy, Workspace,
};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PanePolicy, PaneRequest, PaneSpawn};
use jackin_console::rain::INTRO_END;
use jackin_console::world::{Snapshot, World, WorldParts};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use termrock::clock::SimClock;
use termrock::core::event::{Input, Key};
use termrock::motion::Motion;
use termrock::runtime::{Application, ExternalReceiver, ExternalSender, external_channel};
use termrock::theme::{ColorLevel, Theme};

#[derive(Debug)]
struct TestBackendData;

impl Backend for TestBackendData {
    fn perform(&self, _: &Effect, _: &World) -> Option<Completion> {
        None
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        unreachable!("{NOT_USED}")
    }

    fn launch(&self, _: &World, _: &LaunchTarget) -> Box<dyn LaunchFeed> {
        unreachable!("{NOT_USED}")
    }

    fn spawn_pane(&self, _: &PaneRequest, _: PaneSpawn) -> Box<dyn PaneBody> {
        unreachable!("{NOT_USED}")
    }

    fn initial_panes(&self, _: &Instance) -> Vec<PaneRequest> {
        unreachable!("{NOT_USED}")
    }
}

fn empty_world() -> World {
    World::new(WorldParts::new(
        SimClock::new(1_788_401_640, 0),
        "/home/dev".into(),
        "/home/dev".into(),
        Box::new(SignedInOnly),
        Arc::new(TestBackendData),
        Surfaces::openshell(LiveFacts {
            version: "1.4.0".into(),
            config_path: "/home/dev/.config/jackin/config.toml".into(),
            gateway: None,
        }),
    ))
}

fn workspace(id: u32, name: &str) -> Workspace {
    Workspace {
        id,
        name: name.into(),
        workdir: format!("/home/dev/{name}"),
        mounts: Vec::new(),
        roles: RolePolicy {
            allowed: AllowedRoles::default(),
            default: None,
            last: None,
        },
        env: Vec::new(),
        role_env: BTreeMap::new(),
        keep_awake: false,
        git_pull: false,
        docker: false,
        accounts: AccountPolicy::default(),
        dirty_policy: DirtyExitPolicy::Ask,
        github: None,
    }
}

fn app_over(world: World) -> App {
    assemble(
        world,
        Entry {
            page: EntryPage::Manager,
            outro_when_framed: false,
        },
        AppOptions {
            motion: Motion::Paused,
            frame: INTRO_END,
            theme: Theme::for_level(ColorLevel::TrueColor),
            clipboard: ClipboardMode::Preview,
            panes: PanePolicy::default(),
        },
    )
}

fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(140, 40)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(app: &mut App, keys: &[KeyCode]) {
    for code in keys {
        Application::handle(
            app,
            Input::Key(Key {
                code: *code,
                mods: KeyModifiers::NONE,
            }),
        );
        screen(app);
    }
}

fn drain(app: &mut App) {
    while let Some(message) = Application::external(app).and_then(ExternalReceiver::try_recv) {
        Application::handle(app, Input::External(message));
    }
}

fn snapshot_of(world: &World, workspaces: Vec<Workspace>) -> Snapshot {
    Snapshot {
        workspaces,
        roles: Vec::new(),
        instances: Vec::new(),
        global: world.global.clone(),
        accounts: AccountRegistry::default(),
        surfaces: world.surfaces.clone(),
        github: Vec::new(),
        index_error: None,
        daemon_health: world.daemon_health,
    }
}

fn linked(world: World) -> (App, ExternalSender) {
    let (tx, rx) = external_channel();
    (app_over(world).with_external(rx), tx)
}

#[test]
fn product_shape_renders_and_receives_a_snapshot() {
    let world = empty_world();
    let pushed = snapshot_of(&world, vec![workspace(1, "live-project")]);
    let (mut app, tx) = linked(world);
    let first = screen(&mut app);
    assert!(
        first.contains("Workspaces"),
        "the first frame is the manager:\n{first}"
    );
    assert!(!first.contains("live-project"));
    assert!(tx.send(Remote::Reloaded(Box::new(pushed))));
    drain(&mut app);
    let after = screen(&mut app);
    assert!(after.contains("live-project"), "{after}");
}

#[test]
fn product_shape_about_rows_come_from_the_surfaces() {
    let mut world = empty_world();
    world.surfaces.about = vec![
        ("Product".into(), "jackin 1.4.0".into()),
        ("Mode".into(), "live".into()),
    ];
    let mut app = app_over(world);
    press(
        &mut app,
        &[
            KeyCode::F(10),
            KeyCode::Right,
            KeyCode::Right,
            KeyCode::Down,
            KeyCode::Enter,
        ],
    );
    let about = screen(&mut app)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(about.contains("Product jackin 1.4.0"), "{about}");
    assert!(about.contains("Mode live"), "{about}");
    assert!(!about.contains("simulated"), "{about}");
}
