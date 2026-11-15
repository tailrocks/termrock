#![allow(dead_code)]

use jackin_console::app::App;
use jackin_console::backend::Effect;
use jackin_console::fixtures::{Scenario, app_for, app_with_world};
use jackin_console::world::{Msg, Snapshot, Work, World};
use junie_tui::core::event::{Input, Key, Mouse, MouseKind};
use junie_tui::motion::Motion;
use junie_tui::runtime::Application;
use junie_tui::theme::{ColorLevel, Theme};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

pub fn app_with(scenario: Scenario, world: World) -> App {
    app_with_world(
        scenario,
        world,
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

pub fn app(scenario: Scenario) -> App {
    app_for(
        scenario,
        Motion::Paused,
        10_000,
        Theme::for_level(ColorLevel::TrueColor),
    )
}

pub fn screen(app: &mut App) -> String {
    screen_at(app, 140, 40)
}

pub fn screen_at(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
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

pub fn press(app: &mut App, keys: &str) {
    for name in keys.split_whitespace() {
        let (mods, name) = match (name.strip_prefix("ctrl+"), name.strip_prefix("shift+")) {
            (Some(rest), _) => (KeyModifiers::CONTROL, rest),
            (_, Some(rest)) => (KeyModifiers::SHIFT, rest),
            _ => (KeyModifiers::NONE, name),
        };
        let code = match name {
            "f5" => KeyCode::F(5),
            "f10" => KeyCode::F(10),
            "enter" => KeyCode::Enter,
            "esc" => KeyCode::Esc,
            "down" => KeyCode::Down,
            "up" => KeyCode::Up,
            "right" => KeyCode::Right,
            "left" => KeyCode::Left,
            "tab" => KeyCode::Tab,
            "insert" => KeyCode::Insert,
            "space" => KeyCode::Char(' '),
            c if c.chars().count() == 1 => KeyCode::Char(c.chars().next().expect("one char")),
            other => panic!("unknown key {other}"),
        };
        Application::handle(app, Input::Key(Key { code, mods }));
        screen(app);
    }
}

pub fn click(app: &mut App, x: u16, y: u16) {
    for kind in [MouseKind::Down, MouseKind::Up] {
        Application::handle(
            app,
            Input::Mouse(Mouse::new(kind, ratatui::layout::Position::new(x, y))),
        );
        screen(app);
    }
}

pub fn click_text(app: &mut App, text: &str) {
    let screen = screen(app);
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{screen}"));
    let x = line[..line.find(text).expect("found")].chars().count();
    click(app, x as u16 + 1, y as u16);
}

pub fn hover_text(app: &mut App, text: &str) {
    let screen = screen(app);
    let (y, line) = screen
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(text))
        .unwrap_or_else(|| panic!("{text} not on screen:\n{screen}"));
    let x = line[..line.find(text).expect("found")].chars().count();
    Application::handle(
        app,
        Input::Mouse(Mouse::new(
            MouseKind::Move,
            ratatui::layout::Position::new(x as u16 + 1, y as u16),
        )),
    );
    screen_at(app, 140, 40);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thumb {
    pub x: u16,
    pub start: u16,
    pub len: u16,
}

pub fn thumbs(shown: &str) -> Vec<Thumb> {
    const THUMB: char = '┃';
    let grid: Vec<Vec<char>> = shown.lines().map(|l| l.chars().collect()).collect();
    let width = grid.iter().map(Vec::len).max().unwrap_or(0);
    let mut found = Vec::new();
    for x in 0..width {
        let mut y = 0;
        while y < grid.len() {
            if grid[y].get(x) != Some(&THUMB) {
                y += 1;
                continue;
            }
            let start = y;
            while y < grid.len() && grid[y].get(x) == Some(&THUMB) {
                y += 1;
            }
            if y - start >= 2 {
                found.push(Thumb {
                    x: x as u16,
                    start: start as u16,
                    len: (y - start) as u16,
                });
            }
        }
    }
    found
}

fn mouse(app: &mut App, kind: MouseKind, size: (u16, u16), x: u16, y: u16) {
    Application::handle(
        app,
        Input::Mouse(Mouse::new(kind, ratatui::layout::Position::new(x, y))),
    );
    screen_at(app, size.0, size.1);
}

pub fn the_thumb_follows_the_mouse_cell_for_cell(app: &mut App, size: (u16, u16), which: usize) {
    let at = |app: &mut App| {
        *thumbs(&screen_at(app, size.0, size.1))
            .get(which)
            .unwrap_or_else(|| panic!("no scrollbar thumb number {which}"))
    };
    let rest = at(app);
    assert!(
        rest.len >= 4,
        "the thumb is long enough to grab off-centre: {rest:?}"
    );
    let grab = rest.start + 1;

    mouse(app, MouseKind::Down, size, rest.x, grab);
    assert_eq!(
        at(app).start,
        rest.start,
        "pressing the thumb does not move it"
    );

    mouse(app, MouseKind::Drag, size, rest.x, grab + 1);
    assert_eq!(
        at(app).start,
        rest.start + 1,
        "one cell of drag moves the thumb one cell"
    );

    mouse(app, MouseKind::Up, size, rest.x, grab + 1);
    assert_eq!(
        at(app).start,
        rest.start + 1,
        "letting go leaves the thumb where the drag put it"
    );

    let dragged = at(app);
    let below = dragged.start + dragged.len + 1;
    mouse(app, MouseKind::Down, size, rest.x, below);
    mouse(app, MouseKind::Up, size, rest.x, below);
    assert!(
        at(app).start > dragged.start,
        "a press on the bare track below the thumb moves it down"
    );
}

/// The reply of a roster refresh the world has out, observed now: marks one
/// outstanding at the current observation, so the reply is the one the world
/// is waiting for.
pub fn roster_reply(app: &mut App, snapshot: Snapshot) -> Msg {
    app.world.refresh_from = Some(app.world.authoritative_revision());
    Msg::Refreshed {
        ticket: app.world.refresh_ticket,
        result: Ok(Box::new(snapshot)),
    }
}

pub fn deliver_roster(app: &mut App, snapshot: Snapshot) -> junie_tui::core::event::Outcome {
    let reply = roster_reply(app, snapshot);
    app.deliver(&reply)
}

/// Starts a roster refresh and holds its answer back, the way a slow index
/// would: the world waits on the returned ticket and nothing replies until the
/// test delivers one.
pub fn hold_roster_refresh(app: &mut App) -> u64 {
    app.world.refresh_now();
    app.world.jobs.retain(|job| {
        !matches!(
            job.work,
            Work::Settle(Effect::Refresh { .. }) | Work::Msg(Msg::Refreshed { .. })
        )
    });
    app.world.refresh_ticket
}

/// Every ticket the backend was asked to validate `id` under, oldest first.
pub fn validation_tickets(seen: &[jackin_console::backend::Effect], id: &str) -> Vec<u64> {
    seen.iter()
        .filter_map(|e| match e {
            jackin_console::backend::Effect::ValidateAccount {
                ticket, account, ..
            } if account == id => Some(*ticket),
            _ => None,
        })
        .collect()
}

/// What a backend would answer for the validation `ticket` of `id`.
pub fn validated(
    app: &App,
    ticket: u64,
    id: &str,
    result: Result<jackin_console::backend::Validation, String>,
) -> jackin_console::world::Msg {
    jackin_console::world::Msg::AccountValidated {
        ticket,
        account: id.to_owned(),
        source: Box::new(app.world.accounts.get(id).expect("account").source.clone()),
        result,
    }
}

pub fn valid_fingerprint() -> Result<jackin_console::backend::Validation, String> {
    Ok(jackin_console::backend::Validation::Fingerprint {
        level: jackin_console::domain::account::ValidationLevel::QuotaReadable,
    })
}
