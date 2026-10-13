//! Local reference implementation for the proposed TermRock app runner.

use std::{
    io,
    ops::ControlFlow,
    time::{Duration, Instant},
};

use crossterm::event;
use ratatui::{Frame, Terminal};
use termrock::{
    crossterm::{CrosstermBackend, Session, SessionOptions},
    input::Event,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameTick {
    now: Instant,
    elapsed: Duration,
    delta: Duration,
}

impl FrameTick {
    pub(crate) const fn manual(now: Instant, elapsed: Duration, delta: Duration) -> Self {
        Self {
            now,
            elapsed,
            delta,
        }
    }

    pub(crate) const fn now(self) -> Instant {
        self.now
    }

    pub(crate) const fn elapsed(self) -> Duration {
        self.elapsed
    }

    pub(crate) const fn delta(self) -> Duration {
        self.delta
    }
}

#[derive(Debug)]
struct FrameClock {
    started_at: Instant,
    previous_at: Instant,
}

impl FrameClock {
    fn start() -> Self {
        Self::from_start(Instant::now())
    }

    const fn from_start(now: Instant) -> Self {
        Self {
            started_at: now,
            previous_at: now,
        }
    }

    fn tick(&mut self) -> FrameTick {
        self.tick_at(Instant::now())
    }

    fn tick_at(&mut self, now: Instant) -> FrameTick {
        let now = now.max(self.previous_at);
        let tick = FrameTick::manual(
            now,
            now.saturating_duration_since(self.started_at),
            now.saturating_duration_since(self.previous_at),
        );
        self.previous_at = now;
        tick
    }
}

pub(crate) fn run<Model>(
    model: &mut Model,
    poll_timeout: Duration,
    mut render: impl FnMut(&mut Model, &mut Frame<'_>, FrameTick),
    mut update: impl FnMut(&mut Model, Event, FrameTick) -> ControlFlow<()>,
) -> io::Result<()> {
    let mut session = Session::enter(io::stdout(), SessionOptions::default())?;
    let backend = CrosstermBackend::new(session.writer_mut());
    let mut terminal = Terminal::new(backend)?;
    let mut clock = FrameClock::start();

    let result = loop {
        let tick = clock.tick();
        terminal.draw(|frame| render(model, frame, tick))?;
        if event::poll(poll_timeout)?
            && matches!(
                update(model, Event::from(event::read()?), tick),
                ControlFlow::Break(())
            )
        {
            break Ok(());
        }
    };

    drop(terminal);
    session.restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_clock_emits_injectable_monotonic_ticks() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);

        let first = clock.tick_at(start);
        let second = clock.tick_at(start + Duration::from_millis(120));

        assert_eq!(first.now(), start);
        assert_eq!(first.elapsed(), Duration::ZERO);
        assert_eq!(first.delta(), Duration::ZERO);
        assert_eq!(second.elapsed(), Duration::from_millis(120));
        assert_eq!(second.delta(), Duration::from_millis(120));
    }

    #[test]
    fn frame_clock_clamps_out_of_order_samples_without_rewinding() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);

        let forward = clock.tick_at(start + Duration::from_millis(120));
        let backward = clock.tick_at(start + Duration::from_millis(60));
        let resumed = clock.tick_at(start + Duration::from_millis(200));

        assert_eq!(forward.elapsed(), Duration::from_millis(120));
        assert_eq!(backward.now(), forward.now());
        assert_eq!(backward.elapsed(), forward.elapsed());
        assert_eq!(backward.delta(), Duration::ZERO);
        assert_eq!(resumed.elapsed(), Duration::from_millis(200));
        assert_eq!(resumed.delta(), Duration::from_millis(80));
    }
}
