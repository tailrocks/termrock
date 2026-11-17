#![cfg(feature = "demo")]

mod common;

use std::sync::{Arc, Mutex};

use common::*;
use jackin_console::app::{App, AppOptions, ClipboardMode, Entry, EntryPage, Route, assemble};
use jackin_console::backend::OpSession;
use jackin_console::fixtures::{Scenario, app_for, app_with_world, entry_for, world_for};
use jackin_console::panes::PanePolicy;
use jackin_console::rain::INTRO_END;
use ratatui::layout::Position;
use termrock::core::event::{Input, Mouse, MouseKind};
use termrock::motion::Motion;
use termrock::runtime::Application;
use termrock::shell::NativeClipboard;
use termrock::theme::{ColorLevel, Theme};

fn theme() -> Theme {
    Theme::for_level(ColorLevel::TrueColor)
}

struct Start {
    scenario: Scenario,
    frame: u64,
    route: Route,
    shows: &'static [&'static str],
}

const fn start(
    scenario: Scenario,
    frame: u64,
    route: Route,
    shows: &'static [&'static str],
) -> Start {
    Start {
        scenario,
        frame,
        route,
        shows,
    }
}

const CLAUDE_LAUNCH: &[&str] = &[
    "Launch › payments-platform › the-architect",
    "Claude Code · Anthropic / Claude · account Claude · Work",
];

const START_TABLE: &[Start] = &[
    start(Scenario::FirstUse, 0, Route::Intro, &[]),
    start(
        Scenario::FirstUse,
        INTRO_END,
        Route::Manager,
        &["Workspaces"],
    ),
    start(Scenario::Returning, 0, Route::Manager, &["Workspaces"]),
    start(
        Scenario::AccountsMixed,
        0,
        Route::Accounts,
        &["Accounts › Overview"],
    ),
    start(Scenario::LaunchRunning, 0, Route::Cockpit, CLAUDE_LAUNCH),
    start(Scenario::LaunchRunning, 30, Route::Cockpit, CLAUDE_LAUNCH),
    start(Scenario::LaunchFailure, 0, Route::Cockpit, CLAUDE_LAUNCH),
    start(Scenario::LaunchFailure, 30, Route::Cockpit, CLAUDE_LAUNCH),
    start(
        Scenario::LaunchOpenshell,
        30,
        Route::Cockpit,
        &[
            "Launch › payments-platform › sentinel",
            "Codex · OpenAI · account Codex · Primary",
        ],
    ),
    start(
        Scenario::CapsuleMulti,
        0,
        Route::Capsule,
        &["jackin-payments-pl…form-7f3a"],
    ),
    start(
        Scenario::OutroLast,
        0,
        Route::Capsule,
        &["jackin-payments-pl…form-7f3a"],
    ),
    start(Scenario::OutroLast, 30, Route::Outro, &["Enter Skip"]),
    start(
        Scenario::CapsuleTerminal,
        0,
        Route::Capsule,
        &["jackin-payments-pl…form-7f3a", "Waiting for room"],
    ),
    start(
        Scenario::HardCases,
        0,
        Route::Manager,
        &["Could not confirm running instances: instance index unreadable"],
    ),
    start(
        Scenario::RoleEditor,
        0,
        Route::RoleEditor,
        &["Roles › sentinel › edit"],
    ),
    start(
        Scenario::RoleBinding,
        0,
        Route::Roles,
        &["Roles › sentinel", "▎ ✓ sentinel"],
    ),
    start(Scenario::DockerSetting, 0, Route::Manager, &["Workspaces"]),
    start(Scenario::DockerNoEngine, 0, Route::Manager, &["Workspaces"]),
    start(Scenario::Doctor, 0, Route::Manager, &["Workspaces"]),
    start(Scenario::LaunchChecks, 0, Route::Manager, &["Workspaces"]),
    start(
        Scenario::OpenshellFixed,
        0,
        Route::Manager,
        &["Workspaces", "inside the sandbox"],
    ),
];

fn manager_entry() -> Entry {
    Entry {
        page: EntryPage::Manager,
        outro_when_framed: false,
    }
}

fn options(frame: u64, clipboard: ClipboardMode) -> AppOptions {
    AppOptions {
        motion: Motion::Paused,
        frame,
        theme: theme(),
        clipboard,
        panes: PanePolicy::default(),
    }
}

#[test]
fn start_table_matches_the_old_scenarios() {
    for row in START_TABLE {
        let mut app = app_for(row.scenario, Motion::Paused, row.frame, theme());
        let label = format!("{} at frame {}", row.scenario.name(), row.frame);
        assert_eq!(app.route(), row.route, "{label}");
        let text = screen(&mut app);
        for shown in row.shows {
            assert!(text.contains(shown), "{label} lacks {shown:?}:\n{text}");
        }
    }
    let covered: Vec<_> = START_TABLE.iter().map(|r| r.scenario).collect();
    for scenario in Scenario::ALL {
        assert!(
            covered.contains(&scenario),
            "{} has no row",
            scenario.name()
        );
    }
}

#[test]
fn roles_page_selects_the_named_role() {
    let mut app = assemble(
        world_for(Scenario::RoleBinding),
        Entry {
            page: EntryPage::Roles {
                select: Some("the-architect".into()),
            },
            outro_when_framed: false,
        },
        options(INTRO_END, ClipboardMode::Preview),
    );
    assert_eq!(app.route(), Route::Roles);
    let text = screen(&mut app);
    assert!(
        text.contains("Roles › the-architect"),
        "lacks \"Roles › the-architect\":\n{text}"
    );
}

#[test]
fn launch_failure_plan_fails_the_network_stage() {
    let mut app = app(Scenario::LaunchFailure);
    assert_eq!(app.route(), Route::Cockpit);
    let text = screen(&mut app);
    for shown in [
        "Launch failed",
        "The Construct network could not be attached",
        "! Network failed",
    ] {
        assert!(text.contains(shown), "lacks {shown:?}:\n{text}");
    }
}

#[test]
fn outro_only_when_framed() {
    for scenario in Scenario::ALL {
        assert_eq!(
            entry_for(scenario).outro_when_framed,
            scenario == Scenario::OutroLast,
            "{}",
            scenario.name()
        );
    }
    let attach = |outro_when_framed| Entry {
        page: EntryPage::Attach {
            instance: jackin_console::domain::instance::InstanceId::simulated("jk-7f3a"),
            pane: None,
        },
        outro_when_framed,
    };
    let route = |entry, frame| {
        assemble(
            world_for(Scenario::OutroLast),
            entry,
            options(frame, ClipboardMode::Preview),
        )
        .route()
    };
    assert_eq!(route(attach(true), 0), Route::Capsule);
    assert_eq!(route(attach(true), 30), Route::Outro);
    assert_eq!(route(attach(false), 30), Route::Capsule);
}

#[test]
fn product_entry_is_manager() {
    for scenario in Scenario::ALL {
        let app = assemble(
            world_for(scenario),
            manager_entry(),
            options(INTRO_END, ClipboardMode::Preview),
        );
        assert_eq!(app.route(), Route::Manager, "{}", scenario.name());
    }
}

const LOCK: &str = "1Password locked: unlock the app and retry";

fn launch_hard_cases(world: jackin_console::world::World, picks: &str) -> App {
    let mut app = app_with_world(Scenario::HardCases, world, Motion::Full, 0, theme());
    press(&mut app, picks);
    assert_eq!(app.route(), Route::Cockpit);
    app
}

fn ticks_until_lock(app: &mut App) -> bool {
    for tick in 0..150 {
        Application::handle(app, Input::Tick);
        if tick % 10 == 9 && screen(app).contains(LOCK) {
            return true;
        }
    }
    false
}

#[test]
fn hard_cases_credentials_plan() {
    const CODEX: &str = "down enter down space enter";
    const CLAUDE_PERSONAL: &str = "down enter space enter";

    let mut locked = launch_hard_cases(world_for(Scenario::HardCases), CODEX);
    assert!(ticks_until_lock(&mut locked), "{}", screen(&mut locked));

    let mut world = world_for(Scenario::HardCases);
    world.op.set_session(OpSession::SignedIn);
    let mut signed_in = launch_hard_cases(world, CODEX);
    assert!(!ticks_until_lock(&mut signed_in));
    assert!(screen(&mut signed_in).contains("✓ 03 Credentials"));

    let mut personal = launch_hard_cases(world_for(Scenario::HardCases), CLAUDE_PERSONAL);
    assert!(!ticks_until_lock(&mut personal));
}

#[derive(Debug)]
struct Sink {
    accepts: bool,
    writes: Mutex<Vec<String>>,
}

impl Sink {
    fn new(accepts: bool) -> Arc<Self> {
        Arc::new(Self {
            accepts,
            writes: Mutex::new(Vec::new()),
        })
    }

    fn writes(&self) -> Vec<String> {
        self.writes.lock().expect("sink lock").clone()
    }
}

impl NativeClipboard for Sink {
    fn write(&self, text: &str) -> bool {
        self.writes.lock().expect("sink lock").push(text.to_owned());
        self.accepts
    }
}

const SELECTED: &str = "the settlement retry loop so failed ba";
const SELECTED_OSC52: &str = "\x1b]52;c;dGhlIHNldHRsZW1lbnQgcmV0cnkgbG9vcCBzbyBmYWlsZWQgYmE=\x07";

struct Copy {
    status: String,
    output: Vec<String>,
}

fn capsule(clipboard: ClipboardMode) -> App {
    let mut app = assemble(
        world_for(Scenario::CapsuleMulti),
        entry_for(Scenario::CapsuleMulti),
        options(INTRO_END, clipboard),
    );
    screen(&mut app);
    app
}

fn copied(app: &mut App) -> Copy {
    let status = app
        .shell
        .status
        .text()
        .expect("a copy sets the status")
        .to_owned();
    Copy {
        status,
        output: Application::take_output(app),
    }
}

fn copy_debug_row(clipboard: ClipboardMode) -> Copy {
    let mut app = capsule(clipboard);
    press(&mut app, "ctrl+b i");
    let _ = Application::take_output(&mut app);
    press(&mut app, "y");
    copied(&mut app)
}

fn copy_selection(clipboard: ClipboardMode) -> Copy {
    let mut app = capsule(clipboard);
    let _ = Application::take_output(&mut app);
    for (kind, x) in [
        (MouseKind::Down, 12),
        (MouseKind::Drag, 30),
        (MouseKind::Drag, 50),
        (MouseKind::Up, 50),
    ] {
        Application::handle(
            &mut app,
            Input::Mouse(Mouse::new(kind, Position::new(x, 7))),
        );
        screen(&mut app);
    }
    copied(&mut app)
}

#[test]
fn clipboard_modes() {
    let sink = Sink::new(true);
    let host = copy_selection(ClipboardMode::Host(sink.clone()));
    assert_eq!(sink.writes(), [SELECTED]);
    assert!(
        host.output.is_empty(),
        "no OSC 52 after the native write: {:?}",
        host.output
    );

    let preview = copy_selection(ClipboardMode::Preview);
    assert!(preview.output.is_empty(), "{:?}", preview.output);
    assert_eq!(preview.status, "Copied selection");

    let counted = copy_debug_row(ClipboardMode::Preview);
    assert!(counted.output.is_empty(), "{:?}", counted.output);
    assert_eq!(
        counted.status,
        "Copied 29 characters · jackin-payments-platform-7f3a"
    );
}

#[test]
fn native_sink_decides_the_wording() {
    let accepting = Sink::new(true);
    let accepted = copy_selection(ClipboardMode::Host(accepting.clone()));
    assert_eq!(accepted.status, "Copied selection");
    assert_eq!(accepting.writes(), [SELECTED]);
    assert!(accepted.output.is_empty(), "{:?}", accepted.output);

    let refusing = Sink::new(false);
    let refused = copy_selection(ClipboardMode::Host(refusing.clone()));
    assert_eq!(
        refused.status,
        "Copy requested · this terminal may ignore OSC 52"
    );
    assert_eq!(refusing.writes(), [SELECTED]);
    assert_eq!(refused.output, [SELECTED_OSC52]);

    let counted = Sink::new(true);
    let counted = copy_debug_row(ClipboardMode::Host(counted));
    assert_eq!(
        counted.status,
        "Copied 29 characters · jackin-payments-platform-7f3a"
    );
    assert!(counted.output.is_empty(), "{:?}", counted.output);
}

#[test]
fn a_field_copy_in_the_app_reaches_the_host_clipboard_without_osc52() {
    let sink = Sink::new(true);
    let mut app = assemble(
        world_for(Scenario::AccountsMixed),
        entry_for(Scenario::AccountsMixed),
        options(INTRO_END, ClipboardMode::Host(sink.clone())),
    );
    screen(&mut app);
    press(&mut app, "c a enter S p a r e");
    for c in ['a', 'c'] {
        Application::handle(
            &mut app,
            Input::Key(termrock::core::event::Key {
                code: ratatui::crossterm::event::KeyCode::Char(c),
                mods: ratatui::crossterm::event::KeyModifiers::SUPER,
            }),
        );
    }
    let out = Application::take_output(&mut app);
    assert_eq!(sink.writes(), ["Spare"], "{out:?}");
    assert!(out.is_empty(), "no OSC 52 after the native write: {out:?}");
}

fn manager_with(motion: Motion, theme: Theme) -> App {
    assemble(
        world_for(Scenario::Returning),
        manager_entry(),
        AppOptions {
            motion,
            frame: INTRO_END,
            theme,
            clipboard: ClipboardMode::Preview,
            panes: PanePolicy::default(),
        },
    )
}

fn ticks(app: &mut App, n: usize) {
    for _ in 0..n {
        Application::handle(app, Input::Tick);
    }
}

#[test]
fn motion_decides_whether_the_clock_of_the_world_runs() {
    for (motion, runs) in [
        (Motion::Paused, false),
        (Motion::Reduced, true),
        (Motion::Full, true),
    ] {
        let mut app = manager_with(motion, theme());
        let before = app.world.clock.now_ms;
        ticks(&mut app, 10);
        assert_eq!(
            app.world.clock.now_ms != before,
            runs,
            "{motion:?}: the clock after ten ticks"
        );
    }
}

#[test]
fn reduced_motion_never_skips_the_intro_by_frame() {
    let options = |motion| AppOptions {
        motion,
        frame: INTRO_END,
        theme: theme(),
        clipboard: ClipboardMode::Preview,
        panes: PanePolicy::default(),
    };
    let route = |motion| {
        assemble(
            world_for(Scenario::FirstUse),
            manager_entry(),
            options(motion),
        )
        .route()
    };
    assert_eq!(route(Motion::Full), Route::Manager);
    assert_eq!(route(Motion::Reduced), Route::Intro);
}

fn colors(app: &mut App) -> std::collections::BTreeSet<String> {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 40)).expect("test terminal");
    terminal
        .draw(|f| Application::render(app, f))
        .expect("draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .flat_map(|c| [c.fg, c.bg])
        .map(|c| format!("{c:?}"))
        .collect()
}

#[test]
fn the_theme_of_the_options_sets_the_colour_depth_of_the_frame() {
    let mut full = manager_with(Motion::Paused, theme());
    let mut mono = manager_with(Motion::Paused, Theme::for_level(ColorLevel::Mono));
    let rgb = |set: &std::collections::BTreeSet<String>| set.iter().any(|c| c.starts_with("Rgb"));

    assert!(rgb(&colors(&mut full)), "true colour draws RGB cells");
    assert!(!rgb(&colors(&mut mono)), "mono draws none");
}
