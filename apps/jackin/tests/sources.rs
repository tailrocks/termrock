#![cfg(feature = "demo")]

mod common;

use std::sync::Arc;

use common::*;
use jackin_console::backend::{Backend, Completion, Effect, FileSource};
use jackin_console::domain::instance::{Instance, InstanceId, InstanceStatus, RepoRef};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::vocabulary::Vocabulary;
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::SimBackend;
use jackin_console::world::World;
use termrock::theme::Tone;
use termrock::widgets::file_browser::FileEntry;

struct ListedSource;

impl FileSource for ListedSource {
    fn list(&self, dir: &str) -> Result<Vec<FileEntry>, String> {
        match dir {
            "/home/u/code" => Ok(vec![
                FileEntry::dir("/home/u/code/alpha"),
                FileEntry::dir("/home/u/code/beta"),
            ]),
            "/home/u/code/alpha" => Ok(vec![FileEntry::dir("/home/u/code/alpha/inner")]),
            "/denied" => Err("permission denied by the source".into()),
            _ => Ok(vec![]),
        }
    }

    fn is_dir(&self, _: &str) -> bool {
        true
    }
}

#[derive(Debug)]
struct SourceBackend(Arc<dyn Backend>);

impl Backend for SourceBackend {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        self.0.perform(effect, world)
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        self.0.launch_content()
    }

    fn launch(&self, world: &World, target: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.0.launch(world, target)
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.0.spawn_pane(request, spawn)
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        self.0.initial_panes(instance)
    }

    fn file_source(&self, _: &World) -> Box<dyn FileSource> {
        Box::new(ListedSource)
    }
}

fn sourced(cwd: &str) -> World {
    let mut world = world_for(Scenario::Returning);
    world.backend = Arc::new(SourceBackend(SimBackend::shared()));
    world.cwd = cwd.into();
    world
}

fn other_repo() -> RepoRef {
    RepoRef {
        full_name: "acme/elsewhere".into(),
        url: "https://github.com/acme/elsewhere".into(),
    }
}

#[test]
fn file_browser_uses_the_backend_source() {
    let mut app = app_with(Scenario::Returning, sourced("/home/u/code"));
    press(&mut app, "n");
    let shown = screen(&mut app);
    assert!(
        shown.contains("alpha/") && shown.contains("beta/"),
        "{shown}"
    );
    assert!(
        !shown.contains("payments-platform") && !shown.contains("customer-portal"),
        "{shown}"
    );
}

#[test]
fn wizard_working_dirs_come_from_the_source() {
    let mut app = app_with(Scenario::Returning, sourced("/home/u/code"));
    press(&mut app, "n down space enter");
    let shown = screen(&mut app);
    assert!(shown.contains("step 4 of 5 · Working dir"), "{shown}");
    assert!(shown.contains("/home/u/code/alpha/inner"), "{shown}");
}

#[test]
fn source_error_is_inline() {
    let mut app = app_with(Scenario::Returning, sourced("/denied"));
    press(&mut app, "n");
    let shown = screen(&mut app);
    assert!(shown.contains("permission denied by the source"), "{shown}");
    assert!(shown.contains("/denied"), "{shown}");
}

const MANAGER_GITHUB: &str = "down o";

#[test]
fn github_comes_from_the_workspace() {
    let mut world = world_for(Scenario::Returning);
    world.workspaces[0].github = Some(other_repo());
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, MANAGER_GITHUB);
    assert_eq!(
        app.shell.status.text(),
        Some("Opened https://github.com/acme/elsewhere on the host")
    );
}

#[test]
fn capsule_github_comes_from_the_instance() {
    let mut world = world_for(Scenario::CapsuleMulti);
    world.surfaces.vocabulary = Vocabulary::openshell();
    for instance in &mut world.instances {
        if let Some(instance) = instance.as_available_mut() {
            instance.pr = None;
            instance.branch = Some("main".into());
            instance.github = Some(other_repo());
        }
    }
    let mut app = app_with(Scenario::CapsuleMulti, world);
    press(
        &mut app,
        "f10 right right down down down down enter tab enter",
    );
    assert_eq!(
        app.world.clipboard.as_deref(),
        Some("https://github.com/acme/elsewhere/tree/main")
    );
}

#[test]
fn name_suffix_no_longer_matches() {
    let mut world = world_for(Scenario::Returning);
    assert!(
        world
            .github
            .iter()
            .any(|r| r.full_name.ends_with("payments-platform"))
    );
    for instance in &mut world.instances {
        if let Some(instance) = instance.as_available_mut() {
            instance.github = None;
        }
    }
    world.workspaces[0].github = None;
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, MANAGER_GITHUB);
    assert_eq!(
        app.shell.status.text(),
        Some("No GitHub source for payments-platform")
    );
}

#[test]
fn capsule_name_suffix_no_longer_matches() {
    let mut world = world_for(Scenario::CapsuleMulti);
    for instance in &mut world.instances {
        if let Some(instance) = instance.as_available_mut() {
            instance.github = None;
        }
    }
    let mut app = app_with(Scenario::CapsuleMulti, world);
    press(
        &mut app,
        "f10 right right down down down down enter tab enter",
    );
    assert_eq!(app.world.clipboard, None);
    assert_eq!(
        app.shell.status.get(),
        Some(("No GitHub URL to open for this instance", Tone::Error))
    );
}

#[test]
fn daemon_label_from_workspace() {
    let mut world = world_for(Scenario::Returning);
    let instance = world
        .instance_mut(&InstanceId::simulated("jk-7f3a"))
        .expect("instance");
    instance.status = InstanceStatus::RestoreAvailable;
    instance.workdir = Some("/sandbox/project".into());
    let mut app = app_with(Scenario::Returning, world);
    press(&mut app, "down right down r");
    let daemon = app
        .world
        .daemons
        .get(&InstanceId::simulated("jk-7f3a"))
        .expect("restored daemon");
    assert_eq!(daemon.workspace, "payments-platform");
}
