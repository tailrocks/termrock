//! A test harness that drives an [`Application`] through its real input and
//! render path and reports the work of one operation.
//!
//! The harness owns an off-screen terminal. Every operation it runs ends with
//! a frame, so a measured delta covers the event and the draw it causes.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use termrock_core::event::{ClientPlatform, Input, Key, PlatformPin};
use termrock_metrics::{Metric, WorkDelta};
use termrock_shell::runtime::Application;

use crate::report::ScenarioReport;

pub struct Harness<A: Application> {
    pub app: A,
    term: Terminal<TestBackend>,
    viewport: (u16, u16),
    _platform: PlatformPin,
}

impl<A: Application> Harness<A> {
    /// Wraps `app` and draws into a `viewport` of columns by rows. The calling
    /// thread reports [`ClientPlatform::MacOs`] while the harness lives, so
    /// platform-specific labels measure the same on every host.
    pub fn new(app: A, viewport: (u16, u16)) -> Self {
        let term = Terminal::new(TestBackend::new(viewport.0, viewport.1)).expect("test terminal");
        Self {
            app,
            term,
            viewport,
            _platform: PlatformPin::new(ClientPlatform::MacOs),
        }
    }

    /// Runs `ticks` ticks so the app reaches its settled state.
    pub fn warm(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.tick();
        }
    }

    /// Delivers one input, then draws a frame.
    pub fn send(&mut self, input: Input) {
        self.app.handle(input);
        self.draw();
    }

    pub fn tick(&mut self) {
        self.send(Input::Tick);
    }

    /// Presses `code` with no modifiers, then draws a frame.
    pub fn press(&mut self, code: KeyCode) {
        self.send(Input::Key(Key {
            code,
            mods: KeyModifiers::NONE,
        }));
    }

    /// Draws a frame without delivering an input.
    pub fn draw(&mut self) {
        self.term
            .draw(|frame| self.app.render(frame))
            .expect("test backend draws");
    }

    /// The frame text, one line per row.
    pub fn screen(&self) -> String {
        self.screen_with(|c| c)
    }

    /// The frame text with every braille glyph replaced by `*`, so two frames
    /// compare equal when only an animated spinner advanced.
    pub fn screen_without_spinner(&self) -> String {
        self.screen_with(|c| {
            if ('\u{2800}'..='\u{28ff}').contains(&c) {
                '*'
            } else {
                c
            }
        })
    }

    fn screen_with(&self, map: impl Fn(char) -> char) -> String {
        let buf = self.term.backend().buffer();
        buf.content()
            .chunks(buf.area.width as usize)
            .map(|row| {
                row.iter()
                    .flat_map(|cell| cell.symbol().chars())
                    .map(&map)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Writes the count-only report of one operation at this viewport.
    pub fn report(
        &self,
        scenario: &'static str,
        operation: &'static str,
        record_count: usize,
        work: WorkDelta,
        oracle: &str,
    ) {
        ScenarioReport {
            scenario,
            operation,
            record_count,
            viewport: self.viewport,
            work,
            correctness_oracle: oracle.to_owned(),
        }
        .write()
        .expect("write the count report");
    }
}

/// Panics unless `work` equals `expected` on every metric, unlisted metrics
/// being zero. Use it for every count that is a measured fact, so a counter
/// that stops recording fails the test.
#[track_caller]
pub fn assert_exact(work: &WorkDelta, what: &str, expected: &[(Metric, u64)]) {
    if let Err(violations) = work.check_exact(expected) {
        panic!("{what}: {violations}");
    }
}

/// Panics when `work` exceeds a ceiling in `budgets`, unlisted metrics being
/// zero. Use it only where a number is intentionally a bound.
#[track_caller]
pub fn assert_at_most(work: &WorkDelta, what: &str, budgets: &[(Metric, u64)]) {
    if let Err(violations) = work.check_budget(budgets) {
        panic!("{what}: {violations}");
    }
}
