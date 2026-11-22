use crate::common::{app_with, screen};
use jackin_console::app::{App, Remote};
use jackin_console::backend::{Backend, Completion, Effect, Ticket};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::sim::{SimBackend, snapshot};
use jackin_console::world::{Msg, World};
use std::sync::{Arc, Mutex};
use termrock::core::event::{External, Input, Outcome};
use termrock::runtime::Application;

#[derive(Debug)]
struct Host {
    inner: Arc<dyn Backend>,
    requests: Mutex<Vec<Ticket>>,
}

impl Backend for Host {
    fn perform(&self, effect: &Effect, world: &World) -> Option<Completion> {
        if let Effect::Refresh { ticket } = effect {
            self.requests.lock().unwrap().push(*ticket);
            None
        } else {
            self.inner.perform(effect, world)
        }
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
}

fn request(app: &mut App, force: bool) -> Outcome {
    Application::handle(
        app,
        Input::External(External::new(Remote::Msg(Msg::RequestRefresh { force }))),
    )
}

#[test]
fn remote_refresh_coalesces_periodic_requests_and_forces_a_new_owned_observation() {
    let scenario = Scenario::Returning;
    let host = Arc::new(Host {
        inner: SimBackend::shared(),
        requests: Mutex::default(),
    });
    let mut world = world_for(scenario);
    world.backend = host.clone();
    let mut app = app_with(scenario, world);
    screen(&mut app);
    assert!(app.world.refresh_from.is_none());
    assert_eq!(request(&mut app, false), Outcome::Changed);
    let first = host.requests.lock().unwrap()[0];
    for _ in 0..16 {
        assert_eq!(request(&mut app, false), Outcome::Ignored);
    }
    assert_eq!(*host.requests.lock().unwrap(), [first]);

    assert_eq!(request(&mut app, true), Outcome::Changed);
    let current = host.requests.lock().unwrap()[1];
    assert_ne!(current, first);
    let mut stale = snapshot(&app.world);
    stale.workspaces[0].name = "Stale host workspace".into();
    assert_eq!(
        app.deliver(&Msg::Refreshed {
            ticket: first,
            result: Ok(Box::new(stale)),
        }),
        Outcome::Ignored
    );
    assert!(!screen(&mut app).contains("Stale host workspace"));
    assert!(app.world.refresh_from.is_some());

    let mut fresh = snapshot(&app.world);
    fresh.workspaces[0].name = "Fresh host workspace".into();
    app.deliver(&Msg::Refreshed {
        ticket: current,
        result: Ok(Box::new(fresh)),
    });
    assert!(screen(&mut app).contains("Fresh host workspace"));
    assert!(app.world.refresh_from.is_none());
    assert_eq!(request(&mut app, false), Outcome::Changed);
    assert_eq!(host.requests.lock().unwrap().len(), 3);
}
