#![cfg(feature = "demo")]

use crate::common::{app_with, click_text, press, screen, screen_at};
use jackin_console::backend::{Backend, Completion, Effect, Ticket};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::fixtures::{Scenario, world_for};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{PaneBody, PaneRequest, PaneSpawn};
use jackin_console::screens::Go;
use jackin_console::sim::SimBackend;
use jackin_console::usage_connections::{UsageAction, UsageConnections, UsageEntry, UsageUpdate};
use jackin_console::world::{Msg, World};
use std::sync::{Arc, Mutex};
use termrock::core::id::WidgetId;

struct Boundary {
    pending: Mutex<Vec<(Ticket, UsageAction)>>,
    sim: Arc<dyn Backend>,
}
impl std::fmt::Debug for Boundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Boundary")
    }
}
impl UsageConnections for Boundary {
    fn has_connected_accounts(&self) -> bool {
        true
    }
    fn request(&self, t: Ticket, action: UsageAction) {
        self.pending.lock().unwrap().push((t, action));
    }
}
impl Backend for Boundary {
    fn usage_connections(&self) -> Option<&dyn UsageConnections> {
        Some(self)
    }
    fn perform(&self, e: &Effect, w: &World) -> Option<Completion> {
        self.sim.perform(e, w)
    }
    fn launch_content(&self) -> Arc<LaunchContent> {
        self.sim.launch_content()
    }
    fn launch(&self, w: &World, t: &LaunchTarget) -> Box<dyn LaunchFeed> {
        self.sim.launch(w, t)
    }
    fn spawn_pane(&self, p: &PaneRequest, s: PaneSpawn) -> Box<dyn PaneBody> {
        self.sim.spawn_pane(p, s)
    }
}

fn activate(app: &mut jackin_console::app::App, root: &str, target: &str) {
    let save = WidgetId::of(root).sub(target);
    for _ in 0..16 {
        if app.shell.focus.current() == Some(save) {
            press(app, "enter");
            return;
        }
        press(app, "tab");
    }
    panic!("{target} unreachable by keyboard");
}

fn accounts_with_usage() -> (jackin_console::app::App, Arc<Boundary>) {
    let scenario = Scenario::AccountsMixed;
    let mut world = world_for(scenario);
    let backend = Arc::new(Boundary {
        pending: Mutex::default(),
        sim: SimBackend::shared(),
    });
    world.backend = backend.clone();
    let mut app = app_with(scenario, world);
    app.go(Go::Accounts { select: None });
    screen(&mut app);
    (app, backend)
}

fn connection_form_is_open(app: &mut jackin_console::app::App) {
    let shown = screen(app);
    assert!(
        shown.contains("Continue") && shown.contains("CLIProxyAPI hubs…"),
        "{shown}"
    );
}

fn usage_list_is_requested(app: &mut jackin_console::app::App, backend: &Boundary) {
    let (ticket, action) = backend.pending.lock().unwrap().pop().expect("list request");
    assert_eq!(action, UsageAction::List);
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Accounts(vec![]),
    });
    let shown = screen(app);
    assert!(shown.contains("No usage accounts connected."), "{shown}");
}

#[test]
fn keyboard_add_usage_footer_opens_connection_form() {
    let (mut app, _) = accounts_with_usage();
    activate(&mut app, "accounts.inspector", "connect_usage");
    connection_form_is_open(&mut app);
}

#[test]
fn click_add_usage_footer_opens_connection_form() {
    let (mut app, _) = accounts_with_usage();
    click_text(&mut app, "Add usage account…");
    connection_form_is_open(&mut app);
}

#[test]
fn keyboard_usage_accounts_footer_requests_list() {
    let (mut app, backend) = accounts_with_usage();
    activate(&mut app, "accounts.inspector", "list_usage");
    usage_list_is_requested(&mut app, &backend);
}

#[test]
fn click_usage_accounts_footer_requests_list() {
    let (mut app, backend) = accounts_with_usage();
    click_text(&mut app, "Usage accounts…");
    usage_list_is_requested(&mut app, &backend);
}

#[test]
fn keyboard_connection_shows_usage_and_ignores_a_cancelled_reply() {
    let scenario = Scenario::AccountsMixed;
    let mut world = world_for(scenario);
    let grants = world.accounts.clone();
    let backend = Arc::new(Boundary {
        pending: Mutex::default(),
        sim: SimBackend::shared(),
    });
    world.backend = backend.clone();
    let mut app = app_with(scenario, world);
    app.go(Go::Accounts { select: None });
    screen(&mut app);
    press(&mut app, "n");
    assert!(
        screen(&mut app).contains("Add usage account"),
        "{}",
        screen(&mut app)
    );
    press(&mut app, "enter down down down down down enter");
    activate(&mut app, "accounts.usage_connection", "save");
    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert!(matches!(action,UsageAction::Begin(choice) if choice.provider=="zai"));
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: UsageUpdate::Key {
            url: "https://example.test/keys".into(),
        },
    });
    assert!(screen(&mut app).contains("API key"));
    assert!(screen_at(&mut app, 80, 24).contains("API key"));
    press(&mut app, "enter t e s t k e y enter");
    assert!(!screen(&mut app).contains("testkey"));
    activate(&mut app, "accounts.usage_connection", "save");
    assert!(
        matches!(backend.pending.lock().unwrap().pop(),Some((_,UsageAction::Submit(key))) if key.expose()=="testkey")
    );
    let saved = UsageUpdate::Saved(UsageEntry {
        id: "42".into(),
        label: "Work".into(),
        provider: "zai".into(),
        details: vec!["Session · 12% used".into()],
        resets: vec![],
        hub: None,
        region: None,
        device_login: false,
    });
    app.deliver(&Msg::UsageConnection {
        ticket,
        update: saved.clone(),
    });
    assert!(screen(&mut app).contains("Session · 12% used"));
    press(&mut app, "esc n");
    activate(&mut app, "accounts.usage_connection", "save");
    let (aborted, _) = backend.pending.lock().unwrap().pop().unwrap();
    press(&mut app, "esc");
    assert!(
        matches!(backend.pending.lock().unwrap().pop(),Some((t,UsageAction::Cancel)) if t==aborted)
    );
    app.deliver(&Msg::UsageConnection {
        ticket: aborted,
        update: saved,
    });
    assert!(!screen(&mut app).contains("Session · 12% used"));
    assert_eq!(grants, app.world.accounts);
}

#[test]
fn dashboard_keyboard_hides_restores_and_persists_settings() {
    use jackin_console::usage_connections::{BoardEdit, BoardMetric, BoardSettings, UsageBoard};
    let scenario = Scenario::AccountsMixed;
    let mut world = world_for(scenario);
    let backend = Arc::new(Boundary {
        pending: Mutex::default(),
        sim: SimBackend::shared(),
    });
    world.backend = backend.clone();
    let mut app = app_with(scenario, world);
    app.go(Go::Usage { select: None });
    screen(&mut app);

    let (ticket, action) = backend.pending.lock().unwrap().pop().unwrap();
    assert!(matches!(
        action,
        UsageAction::Board {
            priority: true,
            edit: BoardEdit::DefaultTab
        }
    ));
    let mut data = UsageBoard {
        providers: vec![],
        priority: true,
        summary: "Two hours to reset".into(),
        notices: vec![],
        attention: vec![],
        empty: false,
        alerts: vec![],
        rows: vec![BoardMetric {
            member_preferences: Vec::new(),
            account: Some("owned".into()),
            members: vec![],
            provider: "zai".into(),
            metric: "session".into(),
            title: "Work quota".into(),
            value: "0% remaining".into(),
            group: "Pinned".into(),
            bar: Some((0, None)),
            severity: termrock::theme::Tone::Warning,
            reset: None,
            details: vec!["Last good response".into()],
            pinned: true,
        }],
        share: "{}".into(),
        settings: BoardSettings {
            flags: vec![("remaining", "Remaining", true)],
            warning: 35,
            critical: 10,
            hidden: vec![],
            proxy: false,
        },
    };
    let active = std::cell::Cell::new(ticket);
    let pending = || {
        let (ticket, action) = backend.pending.lock().unwrap().pop()?;
        active.set(ticket);
        Some((ticket, action))
    };
    let send = |app: &mut jackin_console::app::App, d: &UsageBoard| {
        app.deliver(&Msg::UsageConnection {
            ticket: active.get(),
            update: UsageUpdate::Board(Box::new(d.clone())),
        })
    };
    send(&mut app, &data);
    assert!(screen(&mut app).contains("0% remaining"));
    assert!(screen_at(&mut app, 80, 24).contains("0% remaining"));
    const ROOT: &str = "usage.connected_board";
    activate(&mut app, ROOT, "save");
    assert!(screen(&mut app).contains("Last good response"));
    activate(&mut app, ROOT, "hide");
    assert!(matches!(
        pending(),
        Some((
            _,
            UsageAction::Board {
                edit: BoardEdit::Hide { hidden: true, .. },
                ..
            }
        ))
    ));
    let rows = std::mem::take(&mut data.rows);
    send(&mut app, &data);
    activate(&mut app, ROOT, "undo");
    assert!(matches!(
        pending(),
        Some((
            _,
            UsageAction::Board {
                edit: BoardEdit::Hide {
                    hidden: false,
                    restore_pin: true,
                    ..
                },
                ..
            }
        ))
    ));
    data.rows = rows;
    send(&mut app, &data);
    data.attention.push(UsageEntry {
        id: "failed".into(),
        provider: "claude".into(),
        label: "Fix target".into(),
        details: vec!["Sign in again required".into()],
        resets: vec![],
        hub: None,
        region: None,
        device_login: false,
    });
    send(&mut app, &data);
    activate(&mut app, ROOT, "fix");
    assert!(screen(&mut app).contains("Sign in again required"));
    let focus = app.shell.focus.current();
    send(&mut app, &data);
    assert_eq!(app.shell.focus.current(), focus);
    press(&mut app, "esc");
    send(&mut app, &data);
    assert!(screen(&mut app).contains("0% remaining"));
    data.attention.clear();
    data.empty = true;
    data.rows.clear();
    send(&mut app, &data);
    activate(&mut app, ROOT, "add");
    assert!(screen(&mut app).contains("Add usage account"));
    press(&mut app, "esc");
    send(&mut app, &data);
    activate(&mut app, ROOT, "settings");
    press(&mut app, "space");
    activate(&mut app, ROOT, "save");
    assert!(
        matches!(pending(),Some((_,UsageAction::Board{edit:BoardEdit::Settings(s),..})) if !s.flags[0].2)
    );
    press(&mut app, "esc");
    send(&mut app, &data);
    assert!(!screen(&mut app).contains("Two hours to reset"));
    press(&mut app, "a");
    assert!(matches!(
        pending(),
        Some((
            _,
            UsageAction::Board {
                priority: false,
                ..
            }
        ))
    ));
}

#[path = "usage_connections/flows.rs"]
mod flows;
#[path = "usage_connections/management.rs"]
mod management;
