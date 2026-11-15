#![allow(dead_code)]

//! The Capsule over terminal panes whose links are the test links of
//! `pane_support`. No `demo` feature is needed.

use std::sync::{Arc, Mutex};

use jackin_console::app::{App, AppOptions, ClipboardMode, Entry, EntryPage, assemble};
use jackin_console::backend::{Backend, Completion, Effect};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{DaemonSnapshot, Instance, InstanceId, InstanceStatus};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{
    Daemon, PaneBody, PaneCommand, PanePolicy, PaneRequest, PaneSpawn, TerminalPane,
};
use jackin_console::rain::INTRO_END;
use jackin_console::world::{World, WorldParts};
use junie_tui::clock::SimClock;
use junie_tui::core::event::{Input, Mouse, MouseKind};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::shell::NativeClipboard;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Position;

use crate::pane_support::{Bench, INSTANCE, key};
use crate::product_support::{NOT_USED, SignedInOnly};

pub const WORKSPACE: &str = "sandbox-project";

#[derive(Debug)]
pub struct TermBackend {
    pub bench: Arc<Bench>,
    pub requests: Mutex<Vec<PaneRequest>>,
    pub left: Mutex<Vec<String>>,
    pub left_seen: Mutex<Vec<Vec<Vec<PaneCommand>>>>,
    pub effects: Mutex<Vec<Effect>>,
}

impl TermBackend {
    /// The instances the Capsule told the backend it had left, in order.
    pub fn left(&self) -> Vec<String> {
        self.left.lock().expect("left").clone()
    }

    /// What every pane link had been sent, as seen at each `capsule_left`
    /// call: whatever the console did to the panes before it told the
    /// backend.
    pub fn left_seen(&self) -> Vec<Vec<Vec<PaneCommand>>> {
        self.left_seen.lock().expect("left seen").clone()
    }

    /// The tickets of the refreshes the world asked for, in order.
    pub fn refreshes(&self) -> Vec<u64> {
        self.effects
            .lock()
            .expect("effects")
            .iter()
            .filter_map(|e| match e {
                Effect::Refresh { ticket } => Some(*ticket),
                _ => None,
            })
            .collect()
    }
}

impl Backend for TermBackend {
    fn perform(&self, effect: &Effect, _: &World) -> Option<Completion> {
        self.effects.lock().expect("effects").push(effect.clone());
        None
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        unreachable!("{NOT_USED}")
    }

    fn launch(&self, _: &World, _: &LaunchTarget) -> Box<dyn LaunchFeed> {
        unreachable!("{NOT_USED}")
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        self.requests
            .lock()
            .expect("requests")
            .push(request.clone());
        Box::new(TerminalPane::new(
            spawn.id,
            request.clone(),
            self.bench.clone(),
            self.pane_timing(),
            spawn.now_ms,
        ))
    }

    fn initial_panes(&self, instance: &Instance) -> Vec<PaneRequest> {
        vec![PaneRequest::new(&instance.id, Some(Agent::Codex), None)]
    }

    fn capsule_left(&self, _: &World, instance: &InstanceId) {
        self.left.lock().expect("left").push(instance.short_label());
        let links = (0..self.bench.opened())
            .map(|n| self.bench.commands(n))
            .collect();
        self.left_seen.lock().expect("left seen").push(links);
    }
}

#[derive(Debug)]
pub struct Sink(pub Mutex<Vec<String>>);

impl NativeClipboard for Sink {
    fn write(&self, text: &str) -> bool {
        self.0.lock().expect("sink").push(text.to_owned());
        true
    }
}

pub fn instance() -> Instance {
    instance_named(INSTANCE)
}

pub fn instance_named(id: &str) -> Instance {
    Instance {
        id: InstanceId::simulated(id),
        container: "jackin-sandbox".into(),
        workspace: None,
        workdir: Some("/sandbox/project".into()),
        role: Some("the-architect".into()),
        agent: Agent::Codex,
        status: InstanceStatus::Running,
        created_secs: 1_788_401_000,
        last_seen_secs: 1_788_401_000,
        run_id: "run-1".into(),
        sessions: Ok(Vec::new()),
        daemon: DaemonSnapshot::Unavailable,
        branch: None,
        pr: None,
        default_branch: "main".into(),
        github: None,
        uncommitted: 0,
        unpushed: 0,
        account: None,
        accounts: Vec::new(),
        agents: Vec::new(),
        task: None,
    }
}

pub struct Rig {
    pub app: App,
    pub bench: Arc<Bench>,
    pub backend: Arc<TermBackend>,
    pub copied: Arc<Sink>,
}

impl Rig {
    /// A running instance with no daemon: the first attach builds it.
    pub fn new() -> Self {
        Self::with(PanePolicy::default(), |_| {})
    }

    pub fn with(policy: PanePolicy, prepare: impl FnOnce(&mut World)) -> Self {
        let bench = Bench::new();
        let backend = Arc::new(TermBackend {
            bench: bench.clone(),
            requests: Mutex::new(Vec::new()),
            left: Mutex::new(Vec::new()),
            left_seen: Mutex::new(Vec::new()),
            effects: Mutex::new(Vec::new()),
        });
        let mut parts = WorldParts::new(
            SimClock::new(1_788_401_640, 0),
            "/home/dev".into(),
            "/home/dev".into(),
            Box::new(SignedInOnly),
            backend.clone(),
            Surfaces::openshell(LiveFacts {
                version: "1.4.0".into(),
                config_path: "/home/dev/.config/jackin/config.toml".into(),
                gateway: None,
            }),
        );
        parts
            .instances
            .push(jackin_console::domain::instance::InstanceEntry::available(
                instance(),
            ));
        let mut world = World::new(parts);
        prepare(&mut world);
        let copied = Arc::new(Sink(Mutex::new(Vec::new())));
        let app = assemble(
            world,
            Entry {
                page: EntryPage::Attach {
                    instance: InstanceId::simulated(INSTANCE),
                    pane: None,
                },
                outro_when_framed: false,
            },
            AppOptions {
                motion: Motion::Reduced,
                frame: INTRO_END,
                theme: Theme::for_level(ColorLevel::TrueColor),
                clipboard: ClipboardMode::Host(copied.clone()),
                panes: policy,
            },
        );
        let mut rig = Self {
            app,
            bench,
            backend,
            copied,
        };
        rig.screen();
        rig
    }

    pub fn screen(&mut self) -> String {
        let mut terminal = Terminal::new(TestBackend::new(140, 40)).expect("test terminal");
        terminal
            .draw(|f| Application::render(&mut self.app, f))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn squashed(&mut self) -> String {
        self.screen()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The text on the screen without the box lines, so a sentence that a
    /// dialog wrapped reads as one.
    pub fn flat(&mut self) -> String {
        self.screen()
            .chars()
            .filter(|c| !"│┆╭╮╰╯─━".contains(*c))
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn press(&mut self, specs: &str) {
        for spec in specs.split_whitespace() {
            Application::handle(&mut self.app, Input::Key(key(spec)));
            self.screen();
        }
    }

    pub fn typed(&mut self, text: &str) {
        for c in text.chars() {
            self.press(&c.to_string());
        }
    }

    pub fn paste(&mut self, text: &str) {
        Application::handle(&mut self.app, Input::Paste(text.into()));
        self.screen();
    }

    pub fn mouse(&mut self, kind: MouseKind, at: (u16, u16)) {
        let position = Position::new(at.0, at.1);
        Application::handle(&mut self.app, Input::Mouse(Mouse::new(kind, position)));
        self.screen();
    }

    pub fn mouse_with(&mut self, kind: MouseKind, at: (u16, u16), mods: KeyModifiers) {
        let position = Position::new(at.0, at.1);
        Application::handle(
            &mut self.app,
            Input::Mouse(Mouse::new(kind, position).with_mods(mods)),
        );
        self.screen();
    }

    /// The column, first row and length of the first scrollbar thumb on the
    /// screen.
    pub fn thumb(&mut self) -> (u16, u16, u16) {
        const THUMB: char = '┃';
        let grid: Vec<Vec<char>> = self.screen().lines().map(|l| l.chars().collect()).collect();
        let width = grid.iter().map(Vec::len).max().unwrap_or(0);
        for x in 0..width {
            let rows: Vec<usize> = (0..grid.len())
                .filter(|&y| grid[y].get(x) == Some(&THUMB))
                .collect();
            if rows.len() >= 2 {
                return (x as u16, rows[0] as u16, rows.len() as u16);
            }
        }
        panic!("no scrollbar thumb on the screen");
    }

    /// Ticks until the clock has moved on by at least `ms`.
    pub fn advance(&mut self, ms: i64) {
        let until = self.app.world.now_ms() + ms;
        while self.app.world.now_ms() < until {
            Application::handle(&mut self.app, Input::Tick);
            self.screen();
        }
    }

    pub fn daemon(&self) -> &Daemon {
        self.app
            .world
            .daemons
            .get(&InstanceId::simulated(INSTANCE))
            .expect("the daemon")
    }

    pub fn status(&self) -> String {
        self.app.shell.status.text().unwrap_or_default().to_owned()
    }

    /// Attaches the first pane and runs it past its replay window.
    pub fn attach(&mut self) {
        self.advance(80);
        assert_eq!(self.bench.opened(), 1, "the first tick starts the pane");
        self.bench
            .push(0, jackin_console::panes::LinkEvent::Attached);
        self.bench.push(
            0,
            jackin_console::panes::LinkEvent::Lease(jackin_console::panes::Lease::Owner),
        );
        self.advance(900);
    }

    pub fn locate(&mut self, needle: &str) -> (u16, u16) {
        let screen = self.screen();
        for (y, line) in screen.lines().enumerate() {
            if let Some(at) = line.find(needle) {
                return (line[..at].chars().count() as u16, y as u16);
            }
        }
        panic!("{needle:?} is not on the screen:\n{screen}");
    }
}
