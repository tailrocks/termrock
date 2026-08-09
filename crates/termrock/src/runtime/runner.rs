use std::{
    io,
    ops::ControlFlow,
    time::{Duration, Instant},
};

use crossterm::event;
use ratatui_core::{terminal::Frame, terminal::Terminal};

use super::{FrameTick, time::FrameClock};
use crate::{
    crossterm::{CrosstermBackend, Session, SessionOptions},
    input::Event,
};

/// Terminal-session and idle-cadence options for [`run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    /// Terminal modes acquired for the application lifetime.
    pub session: SessionOptions,
    /// Maximum wait between frames when no backend event arrives.
    pub poll_timeout: Duration,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            session: SessionOptions::default(),
            poll_timeout: Duration::from_millis(120),
        }
    }
}

/// Runs a synchronous Crossterm application until `update` requests exit.
///
/// Time is sampled once before each draw. The same [`FrameTick`] reaches render
/// and the event update for that poll cycle. Effects and domain messages remain
/// consumer-owned. `next_deadline` returns the model's earliest timed wakeup;
/// return `None` while no timed state is active.
pub fn run<Model>(
    model: &mut Model,
    options: RunOptions,
    mut render: impl FnMut(&mut Model, &mut Frame<'_>, FrameTick),
    mut update: impl FnMut(&mut Model, Event, FrameTick) -> ControlFlow<()>,
    mut next_deadline: impl FnMut(&Model) -> Option<Instant>,
) -> io::Result<()> {
    let mut session = Session::enter(io::stdout(), options.session)?;
    let backend = CrosstermBackend::new(session.writer_mut());
    let mut terminal = Terminal::new(backend)?;
    let mut clock = FrameClock::start();

    let result = drive_loop(
        model,
        &mut clock,
        options.poll_timeout,
        |model, tick| terminal.draw(|frame| render(model, frame, tick)).map(drop),
        event::poll,
        || event::read().map(Event::from),
        &mut update,
        &mut next_deadline,
    );

    drop(terminal);
    finish_with_restore(result, || session.restore())
}

fn finish_with_restore(
    result: io::Result<()>,
    restore: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let restore_result = restore();
    result.and(restore_result)
}

#[expect(
    clippy::too_many_arguments,
    reason = "runner test seam injects each terminal boundary independently"
)]
fn drive_loop<Model, Draw, Poll, Read, Update, Deadline>(
    model: &mut Model,
    clock: &mut FrameClock,
    poll_timeout: Duration,
    mut draw: Draw,
    mut poll: Poll,
    mut read: Read,
    mut update: Update,
    mut next_deadline: Deadline,
) -> io::Result<()>
where
    Draw: FnMut(&mut Model, FrameTick) -> io::Result<()>,
    Poll: FnMut(Duration) -> io::Result<bool>,
    Read: FnMut() -> io::Result<Event>,
    Update: FnMut(&mut Model, Event, FrameTick) -> ControlFlow<()>,
    Deadline: FnMut(&Model) -> Option<Instant>,
{
    let mut consumed_overdue_deadline = None;
    loop {
        let tick = clock.tick();
        draw(model, tick)?;
        let timeout = match next_deadline(model) {
            Some(deadline) if deadline <= tick.now() => {
                if consumed_overdue_deadline == Some(deadline) {
                    poll_timeout
                } else {
                    consumed_overdue_deadline = Some(deadline);
                    Duration::ZERO
                }
            }
            Some(deadline) => {
                consumed_overdue_deadline = None;
                poll_timeout.min(deadline.saturating_duration_since(tick.now()))
            }
            None => {
                consumed_overdue_deadline = None;
                poll_timeout
            }
        };
        if poll(timeout)? && matches!(update(model, read()?, tick), ControlFlow::Break(())) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::VecDeque};

    use super::*;

    #[test]
    fn loop_draws_through_timeouts_and_stops_on_break_event() {
        let mut model = (0_u8, 0_u8);
        let mut polls = VecDeque::from([false, true, true]);
        let start = std::time::Instant::now();
        let mut clock = FrameClock::from_start(start);

        drive_loop(
            &mut model,
            &mut clock,
            Duration::from_millis(7),
            |model: &mut (u8, u8), _| {
                model.0 += 1;
                Ok(())
            },
            |timeout| {
                assert_eq!(timeout, Duration::from_millis(7));
                Ok(polls.pop_front().expect("bounded fake pump"))
            },
            || Ok(Event::Unknown),
            |model, _, _| {
                model.1 += 1;
                if model.1 == 2 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            },
            |_| None,
        )
        .expect("runner exits cleanly");

        assert_eq!(model, (3, 2));
    }

    #[test]
    fn loop_propagates_draw_poll_and_read_errors() {
        for failing_stage in 0..3 {
            let mut clock = FrameClock::from_start(std::time::Instant::now());
            let error = drive_loop(
                &mut (),
                &mut clock,
                Duration::ZERO,
                |_: &mut (), _| stage_result(failing_stage, 0),
                |_| stage_result(failing_stage, 1).map(|()| true),
                || stage_result(failing_stage, 2).map(|()| Event::Unknown),
                |_, _, _| ControlFlow::Break(()),
                |_| None,
            )
            .expect_err("selected stage must fail");
            assert_eq!(error.kind(), io::ErrorKind::Other);
        }
    }

    #[test]
    fn loop_reuses_one_tick_and_caps_poll_at_next_deadline() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let rendered_tick = Cell::new(None);
        let deadline = Cell::new(None);

        drive_loop(
            &mut (),
            &mut clock,
            Duration::from_secs(5),
            |_: &mut (), tick: FrameTick| {
                rendered_tick.set(Some(tick));
                deadline.set(tick.now().checked_add(Duration::from_millis(250)));
                Ok(())
            },
            |timeout| {
                assert_eq!(timeout, Duration::from_millis(250));
                Ok(true)
            },
            || Ok(Event::Unknown),
            |_, _, update_tick| {
                assert_eq!(Some(update_tick), rendered_tick.get());
                ControlFlow::Break(())
            },
            |_| deadline.get(),
        )
        .expect("deadline-driven cycle");
    }

    #[test]
    fn unchanged_overdue_deadline_gets_one_zero_timeout_without_spinning() {
        let start = Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut polls = VecDeque::from([false, true]);
        let mut observed = Vec::new();

        drive_loop(
            &mut (),
            &mut clock,
            Duration::from_millis(120),
            |_: &mut (), _| Ok(()),
            |timeout| {
                observed.push(timeout);
                Ok(polls.pop_front().expect("two poll cycles"))
            },
            || Ok(Event::Unknown),
            |_, _, _| ControlFlow::Break(()),
            |_| Some(start),
        )
        .expect("overdue deadline handled");

        assert_eq!(observed, [Duration::ZERO, Duration::from_millis(120)]);
    }

    #[test]
    fn restoration_runs_after_success_and_primary_failure() {
        for result in [Ok(()), Err(io::Error::other("primary"))] {
            let expected_error = result.is_err();
            let mut restored = false;
            let returned = finish_with_restore(result, || {
                restored = true;
                Ok(())
            });
            assert!(restored);
            assert_eq!(returned.is_err(), expected_error);
        }

        let restore_error = finish_with_restore(Ok(()), || Err(io::Error::other("restore")))
            .expect_err("restore failure must surface after successful loop");
        assert_eq!(restore_error.to_string(), "restore");

        let primary_error = finish_with_restore(Err(io::Error::other("primary")), || {
            Err(io::Error::other("restore"))
        })
        .expect_err("primary failure remains authoritative");
        assert_eq!(primary_error.to_string(), "primary");
    }

    fn stage_result(failing_stage: u8, stage: u8) -> io::Result<()> {
        if failing_stage == stage {
            Err(io::Error::other("injected runner failure"))
        } else {
            Ok(())
        }
    }
}
