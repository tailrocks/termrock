//! The Capsule over a `PaneBody` it has never heard of. No `demo` feature is
//! needed: the bodies and the backend below are the whole fixture, so this file
//! also runs in the product shape, where the library holds no script player.

mod product_support;

use std::sync::{Arc, Mutex};

use jackin_console::app::{App, AppOptions, ClipboardMode, Entry, EntryPage, assemble};
use jackin_console::backend::{Backend, Completion, Effect};
use jackin_console::domain::agent::Agent;
use jackin_console::domain::instance::{
    AgentState, DaemonSnapshot, Instance, InstanceEntry, InstanceId, InstanceStatus, PaneSnapshot,
    TabSnapshot,
};
use jackin_console::domain::launch::LaunchContent;
use jackin_console::domain::surfaces::{LiveFacts, Surfaces};
use jackin_console::domain::vocabulary::PaneWords;
use jackin_console::launch::{LaunchFeed, LaunchTarget};
use jackin_console::panes::{
    Daemon, PaneBody, PaneId, PaneKey, PaneKind, PaneKindTag, PaneNote, PanePolicy, PaneRequest,
    PaneSpawn, PasteOutcome,
};
use jackin_console::rain::INTRO_END;
use jackin_console::world::{World, WorldParts};
use product_support::{NOT_USED, SignedInOnly};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use ratatui::layout::{Position, Rect};
use termrock::clock::SimClock;
use termrock::core::event::{Input, Key, Mouse, MouseKind};
use termrock::core::id::WidgetId;
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::shell::NativeClipboard;
use termrock::theme::{ColorLevel, Theme};
use termrock::widgets::viewport::{Line, Span, TextViewport};
use termrock::widgets::viewport_ops::ViewportOps;

const INSTANCE: &str = "jk-test";

fn instance_id() -> InstanceId {
    InstanceId::simulated(INSTANCE)
}
const WORKSPACE: &str = "sandbox-project";

#[derive(Debug, Default)]
struct Seen {
    state: Option<AgentState>,
    keys: Vec<KeyCode>,
    pasted: Vec<String>,
    ticks: Vec<i64>,
    cleared: u32,
    synced: Vec<Rect>,
}

type Shared = Arc<Mutex<Seen>>;

#[derive(Debug)]
struct FakeBody {
    view: TextViewport,
    seen: Shared,
    touched: Vec<String>,
}

fn rows(texts: &[&str]) -> Vec<Line> {
    texts.iter().map(|t| vec![Span::plain(*t)]).collect()
}

impl FakeBody {
    fn new(id: PaneId, lines: Vec<Line>, seen: &Shared) -> Self {
        let mut view =
            TextViewport::with_lines(WidgetId::of("test.pane").child(id as usize), lines);
        view.follow = true;
        Self {
            view,
            seen: seen.clone(),
            touched: Vec::new(),
        }
    }

    fn touching(mut self, files: &[&str]) -> Self {
        self.touched = files.iter().map(|f| (*f).to_owned()).collect();
        self
    }
}

impl PaneBody for FakeBody {
    fn kind(&self) -> PaneKindTag {
        PaneKindTag::Script
    }

    fn state(&self) -> AgentState {
        self.seen
            .lock()
            .expect("seen")
            .state
            .unwrap_or(AgentState::Working)
    }

    fn title(&self) -> Option<&str> {
        None
    }

    fn note(&self, _: &PaneWords) -> Option<PaneNote> {
        None
    }

    fn viewport(&self) -> &dyn ViewportOps {
        &self.view
    }

    fn viewport_mut(&mut self) -> &mut dyn ViewportOps {
        &mut self.view
    }

    fn key(&mut self, key: &Key, _now_ms: i64) -> PaneKey {
        self.seen.lock().expect("seen").keys.push(key.code);
        match key.code {
            KeyCode::Char(c) => {
                self.view.push(vec![Span::plain(format!("typed {c}"))]);
                PaneKey::Forwarded
            }
            _ => PaneKey::Consumed,
        }
    }

    fn paste(&mut self, text: &str, _now_ms: i64) -> PasteOutcome {
        self.seen.lock().expect("seen").pasted.push(text.to_owned());
        PasteOutcome::Sent
    }

    fn tick(&mut self, now_ms: i64) -> bool {
        self.seen.lock().expect("seen").ticks.push(now_ms);
        false
    }

    fn sync_size(&mut self, inner: Rect) {
        self.seen.lock().expect("seen").synced.push(inner);
        self.view.set_area(inner);
    }

    fn focus_changed(&mut self, _focused: bool, _now_ms: i64) {}

    fn redraw(&mut self) {}

    fn clear(&mut self) {
        self.seen.lock().expect("seen").cleared += 1;
        self.view.clear();
    }

    fn touched(&self) -> &[String] {
        &self.touched
    }

    fn sends_prefix(&self) -> bool {
        false
    }

    fn close_needs_confirm(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct Recorder {
    seen: Shared,
    spawned: Mutex<Vec<(PaneRequest, PaneSpawn)>>,
}

impl Backend for Recorder {
    fn perform(&self, _: &Effect, _: &World) -> Option<Completion> {
        None
    }

    fn launch_content(&self) -> Arc<LaunchContent> {
        unreachable!("{NOT_USED}")
    }

    fn launch(&self, _: &World, _: &LaunchTarget) -> Box<dyn LaunchFeed> {
        unreachable!("{NOT_USED}")
    }

    fn spawn_pane(&self, request: &PaneRequest, spawn: PaneSpawn) -> Box<dyn PaneBody> {
        let line = match request.kind {
            PaneKind::Agent { agent, .. } => format!("spawned {}", agent.label()),
            PaneKind::Shell => "spawned a shell".to_owned(),
        };
        let body = FakeBody::new(spawn.id, rows(&[&line]), &self.seen);
        self.spawned
            .lock()
            .expect("spawned")
            .push((request.clone(), spawn));
        Box::new(body)
    }

    fn initial_panes(&self, _: &Instance) -> Vec<PaneRequest> {
        unreachable!("{NOT_USED}")
    }
}

fn instance() -> Instance {
    Instance {
        id: instance_id(),
        container: "jackin-sandbox".into(),
        workspace: None,
        workdir: Some("/sandbox/project".into()),
        role: Some("the-architect".into()),
        agent: Agent::ClaudeCode,
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

#[derive(Debug)]
struct Sink(Mutex<Vec<String>>);

impl NativeClipboard for Sink {
    fn write(&self, text: &str) -> bool {
        self.0.lock().expect("sink").push(text.to_owned());
        true
    }
}

struct Rig {
    app: App,
    seen: Shared,
    backend: Arc<Recorder>,
    copied: Arc<Sink>,
}

fn rig(first: Vec<Line>, touched: &[&str]) -> Rig {
    let seen: Shared = Shared::default();
    let backend = Arc::new(Recorder {
        seen: seen.clone(),
        spawned: Mutex::new(Vec::new()),
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
    parts.instances.push(InstanceEntry::available(instance()));
    let mut world = World::new(parts);
    let mut daemon = Daemon::new(WORKSPACE);
    let request = PaneRequest::new(&instance_id(), Some(Agent::ClaudeCode), None);
    daemon.new_tab(&request, 0, |spawn| {
        Box::new(FakeBody::new(spawn.id, first, &seen).touching(touched))
    });
    world.daemons.insert(instance_id(), daemon);
    let copied = Arc::new(Sink(Mutex::new(Vec::new())));
    let app = assemble(
        world,
        Entry {
            page: EntryPage::Attach {
                instance: instance_id(),
                pane: None,
            },
            outro_when_framed: false,
        },
        AppOptions {
            motion: Motion::Reduced,
            frame: INTRO_END,
            theme: Theme::for_level(ColorLevel::TrueColor),
            clipboard: ClipboardMode::Host(copied.clone()),
            panes: PanePolicy::default(),
        },
    );
    let mut rig = Rig {
        app,
        seen,
        backend,
        copied,
    };
    rig.screen();
    rig
}

impl Rig {
    fn screen(&mut self) -> String {
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

    fn key(&mut self, spec: &str) {
        for name in spec.split_whitespace() {
            let (mods, name) = match name.strip_prefix("ctrl+") {
                Some(rest) => (KeyModifiers::CONTROL, rest),
                None => (KeyModifiers::NONE, name),
            };
            let code = match name {
                "enter" => KeyCode::Enter,
                "f5" => KeyCode::F(5),
                c if c.chars().count() == 1 => KeyCode::Char(c.chars().next().expect("a char")),
                other => panic!("unknown key {other}"),
            };
            Application::handle(&mut self.app, Input::Key(Key { code, mods }));
            self.screen();
        }
    }

    fn typed(&mut self, text: &str) {
        for c in text.chars() {
            self.key(&c.to_string());
        }
    }

    fn mouse(&mut self, kind: MouseKind, at: (u16, u16)) {
        let position = Position::new(at.0, at.1);
        Application::handle(&mut self.app, Input::Mouse(Mouse::new(kind, position)));
        self.screen();
    }

    fn ticks(&mut self, n: usize) {
        for _ in 0..n {
            Application::handle(&mut self.app, Input::Tick);
        }
        self.screen();
    }

    fn daemon(&self) -> &Daemon {
        self.app
            .world
            .daemons
            .get(&instance_id())
            .expect("the daemon")
    }
}

fn locate(screen: &str, needle: &str) -> (u16, u16) {
    for (y, line) in screen.lines().enumerate() {
        if let Some(at) = line.find(needle) {
            return (line[..at].chars().count() as u16, y as u16);
        }
    }
    panic!("{needle:?} is not on the screen:\n{screen}");
}

#[test]
fn the_capsule_draws_a_body_it_does_not_know() {
    let mut rig = rig(rows(&["alpha line", "beta line"]), &[]);
    let screen = rig.screen();
    assert!(
        screen.contains("alpha line") && screen.contains("beta line"),
        "{screen}"
    );
    assert!(
        screen.contains("Claude Code"),
        "the label comes from the pane:\n{screen}"
    );
    let squashed = screen.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(squashed.contains("Claude Code · working"), "{squashed}");
    assert!(squashed.contains("1 tab · 1 pane"), "{squashed}");

    rig.seen.lock().expect("seen").state = Some(AgentState::Blocked);
    let blocked = rig
        .screen()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(blocked.contains("Claude Code · needs input"), "{blocked}");
}

#[test]
fn keys_pastes_and_clears_reach_the_body() {
    let mut rig = rig(rows(&["alpha line"]), &[]);
    rig.typed("ab");
    let screen = rig.screen();
    assert!(
        screen.contains("typed a") && screen.contains("typed b"),
        "{screen}"
    );
    rig.key("f5");
    assert_eq!(
        rig.seen.lock().expect("seen").keys,
        [KeyCode::Char('a'), KeyCode::Char('b'), KeyCode::F(5)]
    );

    Application::handle(&mut rig.app, Input::Paste("xy\r\nz".into()));
    assert_eq!(rig.seen.lock().expect("seen").pasted, ["xyz"]);

    rig.key("ctrl+b ctrl+l");
    assert_eq!(rig.seen.lock().expect("seen").cleared, 1);
    let cleared = rig.screen();
    assert!(!cleared.contains("typed a"), "{cleared}");
}

#[test]
fn the_world_ticks_every_body_and_sizes_it() {
    let mut rig = rig(rows(&["alpha line"]), &[]);
    rig.ticks(5);
    let ticks = rig.seen.lock().expect("seen").ticks.clone();
    assert_eq!(ticks.len(), 5, "{ticks:?}");
    assert!(
        ticks.windows(2).all(|w| w[1] > w[0]),
        "time moves: {ticks:?}"
    );

    let at = locate(&rig.screen(), "alpha line");
    rig.mouse(MouseKind::Down, at);
    rig.mouse(MouseKind::Up, at);
    let synced = rig.seen.lock().expect("seen").synced.clone();
    assert!(
        !synced.is_empty(),
        "a click primes the body with its inner area"
    );
    assert!(
        synced.iter().all(|r| r.width > 40 && r.height > 10),
        "{synced:?}"
    );
}

#[test]
fn the_files_a_body_touched_count_as_changes() {
    let squash = |screen: String| screen.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut quiet = rig(rows(&["alpha line"]), &[]);
    let clean = squash(quiet.screen());
    assert!(
        clean.contains("main clean") && !clean.contains("changed"),
        "{clean}"
    );

    let mut busy = rig(rows(&["alpha line"]), &["src/a.rs", "src/b.rs"]);
    let changed = squash(busy.screen());
    assert!(
        changed.contains("changed") && !changed.contains("main clean"),
        "{changed}"
    );
}

#[test]
fn a_new_tab_and_a_split_ask_the_backend_for_a_body() {
    let mut rig = rig(rows(&["alpha line"]), &[]);
    rig.ticks(2);
    let before = rig.app.world.now_ms();

    rig.key("ctrl+b c");
    rig.typed("Shell");
    rig.key("enter");
    let screen = rig.screen();
    assert!(screen.contains("spawned a shell"), "{screen}");
    {
        let spawned = rig.backend.spawned.lock().expect("spawned");
        assert_eq!(spawned.len(), 1);
        let (request, spawn) = &spawned[0];
        assert_eq!(request.instance, instance_id());
        assert!(matches!(request.kind, PaneKind::Shell));
        assert_eq!(spawn.id, 2, "the daemon hands out the next pane id");
        assert_eq!(spawn.workspace, WORKSPACE);
        assert!(spawn.now_ms >= before, "{} < {before}", spawn.now_ms);
    }
    let squashed = screen.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(squashed.contains("2 tabs · 1 pane"), "{squashed}");
    assert_eq!(rig.daemon().panes.len(), 2);
    assert_eq!(rig.daemon().panes[1].label, "Shell");
    assert_eq!(rig.daemon().panes[1].agent, None);

    rig.key("ctrl+b %");
    rig.typed("Shell");
    rig.key("enter");
    let split = rig
        .screen()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(split.contains("2 tabs · 2 panes"), "{split}");
    assert_eq!(rig.backend.spawned.lock().expect("spawned").len(), 2);
    assert_eq!(rig.backend.spawned.lock().expect("spawned")[1].1.id, 3);
}

#[test]
fn the_wheel_and_a_drag_work_through_the_bodys_viewport() {
    let long: Vec<Line> = (0..100)
        .map(|n| vec![Span::plain(format!("alpha row {n:03}"))])
        .collect();
    let mut rig = rig(long, &[]);
    rig.key("ctrl+b %");
    rig.typed("Shell");
    rig.key("enter");
    rig.key("ctrl+b h");
    let screen = rig.screen();
    assert!(screen.contains("alpha row 099"), "{screen}");
    assert!(
        !screen.contains(" of 100 "),
        "following the tail shows no range"
    );

    let at = locate(&screen, "alpha row 099");
    for _ in 0..3 {
        rig.mouse(MouseKind::WheelUp, at);
    }
    let scrolled = rig.screen();
    assert!(scrolled.contains(" of 100 "), "{scrolled}");
    assert!(scrolled.contains("Exit scrollback"), "{scrolled}");
    assert!(!scrolled.contains("alpha row 099"), "{scrolled}");

    let from = locate(&scrolled, "alpha row 0");
    rig.mouse(MouseKind::Down, from);
    rig.mouse(MouseKind::Drag, (from.0 + 5, from.1));
    rig.mouse(MouseKind::Drag, (from.0 + 13, from.1));
    rig.mouse(MouseKind::Up, (from.0 + 13, from.1));
    let copied = rig.copied.0.lock().expect("sink").clone();
    assert_eq!(copied.len(), 1, "{copied:?}");
    assert!(
        copied[0].starts_with("alpha row 0") && copied[0].len() == 13,
        "{copied:?}"
    );
}

#[test]
fn the_daemon_snapshot_reads_the_bodies() {
    let rig = rig(rows(&["alpha line"]), &[]);
    let expect = |state| {
        DaemonSnapshot::Tabs(vec![TabSnapshot {
            label: "Claude Code".into(),
            active: true,
            panes: vec![PaneSnapshot {
                label: "Claude Code".into(),
                agent: Some(Agent::ClaudeCode),
                state,
                focused: true,
            }],
        }])
    };
    assert_eq!(rig.daemon().snapshot(), expect(AgentState::Working));
    rig.seen.lock().expect("seen").state = Some(AgentState::Done);
    assert_eq!(rig.daemon().snapshot(), expect(AgentState::Done));
}
