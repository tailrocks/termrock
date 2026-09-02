// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Headless five-artifact capture of the catalog and TablePro.
//!
//! Replay inventoried [`crate::scenarios::Scenario`] steps against the same
//! `App` the interactive binaries mount.

use std::path::Path;
use std::time::Duration;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use termrock::input::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use termrock::runtime::FrameTick;
use termrock::style::ColorCapability;

use crate::catalog::CatalogProfile;
use crate::scenarios::{Host, Scenario, Step};
use crate::shell::App;
use crate::snapshot::Snapshot;
use crate::tablepro::App as TableProApp;

/// One captured scenario: five artifacts from a single grid.
pub struct Artifacts {
    pub snapshot: Snapshot,
    pub buffer: Buffer,
}

impl Artifacts {
    #[must_use]
    pub fn txt(&self) -> String {
        self.snapshot.to_txt()
    }
    #[must_use]
    pub fn txt_trimmed(&self) -> String {
        self.snapshot.to_txt_trimmed()
    }
    #[must_use]
    pub fn cursor(&self) -> String {
        self.snapshot.to_cursor()
    }
    #[must_use]
    pub fn ansi(&self) -> String {
        self.snapshot.to_ansi()
    }
    #[must_use]
    pub fn html(&self) -> String {
        self.snapshot.to_html()
    }

    /// PNG bytes via termrock-raster (zero-tol pixel compare).
    #[cfg(feature = "native")]
    pub fn png(&self) -> Result<Vec<u8>, String> {
        termrock_raster::render_png(&self.buffer, &termrock::style::RolePalette::junie())
            .map_err(|e| e.to_string())
    }

    /// Write `.ansi` `.cursor` `.txt` `.html` `.png` next to `stem`.
    #[cfg(feature = "native")]
    pub fn write_five(&self, stem: &Path) -> Result<(), String> {
        let parent = stem.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        std::fs::write(stem.with_extension("txt"), self.txt()).map_err(|e| e.to_string())?;
        std::fs::write(stem.with_extension("cursor"), self.cursor()).map_err(|e| e.to_string())?;
        std::fs::write(stem.with_extension("ansi"), self.ansi()).map_err(|e| e.to_string())?;
        std::fs::write(stem.with_extension("html"), self.html()).map_err(|e| e.to_string())?;
        std::fs::write(stem.with_extension("png"), self.png()?).map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn tick_at(elapsed_ms: u64) -> FrameTick {
    FrameTick::manual(
        termrock::runtime::Instant::now(),
        Duration::from_millis(elapsed_ms),
        Duration::from_millis(80),
    )
}

fn key(code: KeyCode, mods: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, mods))
}

fn mouse(kind: MouseEventKind, x: u16, y: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        position: Position { x, y },
        modifiers: KeyModifiers::NONE,
    })
}

enum Drive<'a> {
    Catalog(&'a mut App),
    TablePro(&'a mut TableProApp),
}

impl Drive<'_> {
    fn send(&mut self, ev: Event, t: FrameTick) {
        match self {
            Self::Catalog(app) => {
                let _ = app.handle_event(ev, t);
            }
            Self::TablePro(app) => {
                let _ = app.handle_event(ev, t);
            }
        }
    }

    fn tick(&mut self, t: FrameTick) {
        match self {
            Self::Catalog(app) => app.on_tick(t),
            Self::TablePro(app) => app.on_tick(t),
        }
    }
}

fn apply_step(
    drive: &mut Drive<'_>,
    term: &mut Terminal<TestBackend>,
    cols: &mut u16,
    rows: &mut u16,
    elapsed: &mut u64,
    step: Step,
) {
    match step {
        Step::Tab => drive.send(key(KeyCode::Tab, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::BackTab => drive.send(key(KeyCode::BackTab, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Enter => drive.send(key(KeyCode::Enter, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Esc => drive.send(key(KeyCode::Esc, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Space => drive.send(
            key(KeyCode::Char(' '), KeyModifiers::NONE),
            tick_at(*elapsed),
        ),
        Step::Up => drive.send(key(KeyCode::Up, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Down => drive.send(key(KeyCode::Down, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Left => drive.send(key(KeyCode::Left, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Right => drive.send(key(KeyCode::Right, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Home => drive.send(key(KeyCode::Home, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::End => drive.send(key(KeyCode::End, KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Backspace => drive.send(
            key(KeyCode::Backspace, KeyModifiers::NONE),
            tick_at(*elapsed),
        ),
        Step::Char(c) => drive.send(key(KeyCode::Char(c), KeyModifiers::NONE), tick_at(*elapsed)),
        Step::Ctrl(c) => drive.send(
            key(KeyCode::Char(c), KeyModifiers::CONTROL),
            tick_at(*elapsed),
        ),
        Step::Alt(c) => drive.send(key(KeyCode::Char(c), KeyModifiers::ALT), tick_at(*elapsed)),
        Step::Type(s) => {
            for c in s.chars() {
                drive.send(key(KeyCode::Char(c), KeyModifiers::NONE), tick_at(*elapsed));
            }
        }
        Step::Move(x, y) => drive.send(mouse(MouseEventKind::Moved, x, y), tick_at(*elapsed)),
        Step::Click(x, y) => {
            drive.send(
                mouse(MouseEventKind::Down(MouseButton::Left), x, y),
                tick_at(*elapsed),
            );
            drive.send(
                mouse(MouseEventKind::Up(MouseButton::Left), x, y),
                tick_at(*elapsed),
            );
        }
        Step::WheelDown(x, y) => {
            drive.send(mouse(MouseEventKind::ScrollDown, x, y), tick_at(*elapsed));
        }
        Step::Resize(c, r) => {
            *cols = c;
            *rows = r;
            drive.send(
                Event::Resize {
                    width: c,
                    height: r,
                },
                tick_at(*elapsed),
            );
            let _ = term.resize(ratatui::layout::Rect::new(0, 0, c, r));
        }
        Step::Ticks(n) => {
            for _ in 0..n {
                *elapsed = elapsed.saturating_add(80);
                drive.tick(tick_at(*elapsed));
            }
        }
    }
}

fn snapshot_of(buf: Buffer, cursor: Option<Position>, cols: u16, rows: u16) -> Artifacts {
    let visible = cursor.is_some();
    let pos = cursor.or(Some(Position {
        x: cols,
        y: rows.saturating_sub(1),
    }));
    Artifacts {
        snapshot: Snapshot::from_buffer(&buf, pos, visible),
        buffer: buf,
    }
}

/// Replay one inventoried scenario on the junie-reference catalog or TablePro.
pub fn replay(scenario: &Scenario) -> Artifacts {
    match scenario.host {
        Host::Catalog(page) => replay_catalog(scenario, page),
        Host::TablePro { connect } => replay_tablepro(scenario, connect),
    }
}

fn replay_catalog(scenario: &Scenario, page: crate::catalog::PageId) -> Artifacts {
    let mut app = App::new(CatalogProfile::JunieReference, ColorCapability::Truecolor);
    app.goto(page);
    let mut cols = scenario.cols;
    let mut rows = scenario.rows;
    let mut term = Terminal::new(TestBackend::new(cols, rows)).expect("test backend");
    let mut elapsed = 0_u64;
    let draw = |app: &mut App, term: &mut Terminal<TestBackend>, elapsed: u64| {
        let t = tick_at(elapsed);
        term.draw(|f| app.render(f, t)).expect("draw");
    };
    draw(&mut app, &mut term, elapsed);
    for step in scenario.steps {
        apply_step(
            &mut Drive::Catalog(&mut app),
            &mut term,
            &mut cols,
            &mut rows,
            &mut elapsed,
            *step,
        );
        draw(&mut app, &mut term, elapsed);
    }
    snapshot_of(term.backend().buffer().clone(), app.last_cursor, cols, rows)
}

fn replay_tablepro(scenario: &Scenario, connect: Option<&str>) -> Artifacts {
    let mut app = TableProApp::new(ColorCapability::Truecolor);
    if let Some(name) = connect {
        let _ = app.connect_named(name);
    }
    let mut cols = scenario.cols;
    let mut rows = scenario.rows;
    let mut term = Terminal::new(TestBackend::new(cols, rows)).expect("test backend");
    let mut elapsed = 0_u64;
    let draw = |app: &mut TableProApp, term: &mut Terminal<TestBackend>, elapsed: u64| {
        let t = tick_at(elapsed);
        term.draw(|f| app.render(f, t)).expect("draw");
    };
    draw(&mut app, &mut term, elapsed);
    for step in scenario.steps {
        apply_step(
            &mut Drive::TablePro(&mut app),
            &mut term,
            &mut cols,
            &mut rows,
            &mut elapsed,
            *step,
        );
        draw(&mut app, &mut term, elapsed);
    }
    snapshot_of(term.backend().buffer().clone(), app.last_cursor, cols, rows)
}

/// Render one catalog page under `profile` (idle first frame).
#[must_use]
pub fn catalog_page(
    profile: CatalogProfile,
    page: crate::catalog::PageId,
    cols: u16,
    rows: u16,
) -> Artifacts {
    let mut app = App::new(profile, ColorCapability::Truecolor);
    app.goto(page);
    let mut term = Terminal::new(TestBackend::new(cols, rows)).expect("test backend");
    let t = tick_at(0);
    term.draw(|f| app.render(f, t)).expect("draw");
    snapshot_of(term.backend().buffer().clone(), app.last_cursor, cols, rows)
}

/// Render standalone TablePro, optionally connected by name (idle).
#[must_use]
pub fn tablepro(connect: Option<&str>, cols: u16, rows: u16) -> Artifacts {
    let mut app = TableProApp::new(ColorCapability::Truecolor);
    if let Some(name) = connect {
        let _ = app.connect_named(name);
    }
    let mut term = Terminal::new(TestBackend::new(cols, rows)).expect("test backend");
    let t = tick_at(0);
    term.draw(|f| app.render(f, t)).expect("draw");
    snapshot_of(term.backend().buffer().clone(), app.last_cursor, cols, rows)
}
