use std::{
    io,
    ops::ControlFlow,
    time::{Duration, Instant},
};

use crossterm::{
    event, execute,
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use ratatui_core::{terminal::Frame, terminal::Terminal};

use super::{FrameTick, Presenter, QuietBackend, time::FrameClock};
use crate::{
    crossterm::{CrosstermBackend, Session, SessionOptions},
    input::{Event, MouseEventKind},
};

/// Terminal-session and idle-cadence options for [`run`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    /// Terminal modes acquired for the application lifetime.
    pub session: SessionOptions,
    /// Maximum wait between frames when no backend event arrives.
    ///
    /// This is a ceiling, not a cadence: an idle screen blocks here without
    /// drawing, and the [`Presenter`] shortens the wait when something is owed.
    pub poll_timeout: Duration,
    /// Wrap every frame in synchronized output (DEC mode 2026).
    ///
    /// Terminals that do not implement it ignore the sequence, which is the
    /// silent degrade the motion SoT §2 asks for; set `false` only to debug a
    /// terminal that mishandles it.
    pub synchronized_output: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            session: SessionOptions::default(),
            poll_timeout: Duration::from_millis(120),
            synchronized_output: true,
        }
    }
}

/// Runs a synchronous Crossterm application until `update` requests exit.
///
/// Time is sampled once before each draw. The same [`FrameTick`] reaches render
/// and the event update for that poll cycle. Effects and domain messages remain
/// consumer-owned. `next_deadline` returns the model's earliest timed wakeup;
/// return `None` while no timed state is active.
///
/// **Demand-driven.** A frame is drawn when input arrives, when a deadline the
/// model reported comes due, or when a scroll flush is owed — never on a fixed
/// cadence. An idle screen emits nothing and burns no CPU. State that changes
/// outside of events must be announced through `next_deadline`.
pub fn run<Model>(
    model: &mut Model,
    options: RunOptions,
    mut render: impl FnMut(&mut Model, &mut Frame<'_>, FrameTick),
    mut update: impl FnMut(&mut Model, Event, FrameTick) -> ControlFlow<()>,
    mut next_deadline: impl FnMut(&Model) -> Option<Instant>,
) -> io::Result<()> {
    let mut session = Session::enter(io::stdout(), options.session)?;
    // Cursor de-dup lives in the backend so an unchanged frame emits nothing.
    let backend = QuietBackend::new(CrosstermBackend::new(session.writer_mut()));
    let mut terminal = Terminal::new(backend)?;
    let mut clock = FrameClock::start();
    let synchronized = options.synchronized_output;

    let result = drive_loop(
        model,
        &mut clock,
        Presenter::new(),
        options.poll_timeout,
        |model, tick| {
            // BSU/ESU span stays as short as possible: one draw, nothing else
            // (ConPTY latency, rio#1753). The sequences go through the same
            // stdout the backend writes to, and `execute!` flushes, so the
            // frame lands strictly between them.
            if synchronized {
                execute!(io::stdout(), BeginSynchronizedUpdate)?;
            }
            let drawn = terminal.draw(|frame| render(model, frame, tick)).map(drop);
            if synchronized {
                execute!(io::stdout(), EndSynchronizedUpdate)?;
            }
            drawn
        },
        event::poll,
        || event::read().map(Event::from),
        &mut update,
        &mut next_deadline,
    );

    drop(terminal);
    finish_with_restore(result, || session.restore())
}

/// Whether an event only asks for a scroll flush rather than a full repaint.
const fn is_scroll(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse)
            if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown)
    )
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
    mut presenter: Presenter,
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
        if presenter.should_draw(tick.now()) {
            presenter.begin_draw(tick.now());
            let drawn = draw(model, tick);
            presenter.end_draw(tick.now());
            drawn?;
        }
        let timeout = match next_deadline(model) {
            Some(deadline) if deadline <= tick.now() => {
                if consumed_overdue_deadline == Some(deadline) {
                    poll_timeout
                } else {
                    consumed_overdue_deadline = Some(deadline);
                    presenter.mark_dirty();
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
        // The presenter may owe a frame sooner than the model's next deadline.
        let timeout = presenter
            .next_wake(tick.now())
            .map_or(timeout, |wake| timeout.min(wake));
        if poll(timeout)? {
            let event = read()?;
            if is_scroll(&event) {
                presenter.mark_scrolled();
            } else {
                presenter.mark_dirty();
            }
            if matches!(update(model, event, tick), ControlFlow::Break(())) {
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, collections::VecDeque};

    use super::*;
    use crate::input::{KeyModifiers, MouseEvent};

    /// Presenter without the min-draw throttle, so tests are wall-clock free.
    fn unthrottled() -> Presenter {
        Presenter::new().min_draw_interval(Duration::ZERO)
    }

    fn scroll_event() -> Event {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            position: ratatui_core::layout::Position::new(0, 0),
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn loop_draws_through_timeouts_and_stops_on_break_event() {
        let mut model = (0_u8, 0_u8);
        let mut polls = VecDeque::from([false, true, true]);
        let start = std::time::Instant::now();
        let mut clock = FrameClock::from_start(start);

        drive_loop(
            &mut model,
            &mut clock,
            unthrottled(),
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

        // Three loop iterations, two frames: the middle iteration had nothing
        // dirty to paint, which is the whole point of the presenter.
        assert_eq!(model, (2, 2));
    }

    #[test]
    fn loop_propagates_draw_poll_and_read_errors() {
        for failing_stage in 0..3 {
            let mut clock = FrameClock::from_start(std::time::Instant::now());
            let error = drive_loop(
                &mut (),
                &mut clock,
                unthrottled(),
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
            unthrottled(),
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
            unthrottled(),
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
    fn wheel_flood_paints_no_ghost_frames() {
        // Grok's regression test, ported: a burst of wheel events inside one
        // 16 ms window must produce one frame, not one frame per event.
        let mut draws = 0_u32;
        let mut updates = 0_u32;
        let start = std::time::Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut polls = VecDeque::from(vec![true; 64]);

        drive_loop(
            &mut (),
            &mut clock,
            // Real throttle on purpose: this is the behaviour under test.
            Presenter::new(),
            Duration::ZERO,
            |_: &mut (), _| {
                draws += 1;
                Ok(())
            },
            |_| Ok(polls.pop_front().unwrap_or(false)),
            || Ok(scroll_event()),
            |_, _, _| {
                updates += 1;
                if updates == 64 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            },
            |_| None,
        )
        .expect("wheel flood drains");

        assert_eq!(updates, 64, "every wheel event still reaches the model");
        assert!(
            draws <= 2,
            "wheel flood queued {draws} frames; scroll must coalesce onto its own clock"
        );
    }

    #[test]
    fn idle_loop_emits_no_frames() {
        // Idle CPU = 0 is the quality signal (motion SoT §2 rule 4): with
        // nothing dirty and no animation registered, the loop must not paint.
        let mut draws = 0_u32;
        let start = std::time::Instant::now();
        let mut clock = FrameClock::from_start(start);
        let mut polls = VecDeque::from([false, false, false, true]);

        drive_loop(
            &mut (),
            &mut clock,
            unthrottled(),
            Duration::from_millis(1),
            |_: &mut (), _| {
                draws += 1;
                Ok(())
            },
            |_| Ok(polls.pop_front().unwrap_or(true)),
            || Ok(Event::Unknown),
            |_, _, _| ControlFlow::Break(()),
            |_| None,
        )
        .expect("idle loop exits on the first event");

        assert_eq!(
            draws, 1,
            "only the first frame is owed; three idle polls must paint nothing"
        );
    }

    /// Writer that keeps every emitted byte visible to the test.
    #[derive(Clone, Default)]
    struct TappedWriter(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

    impl std::io::Write for TappedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn idle_redraw_emits_zero_bytes() {
        // Double-buffer diff law (§2 rule 1): repainting an unchanged frame
        // must put nothing on the wire, or the hardware cursor blink dies and
        // muxes see traffic for a screen that did not change.
        use ratatui_core::widgets::Widget;

        let tap = TappedWriter::default();
        let mut terminal = Terminal::with_options(
            QuietBackend::new(CrosstermBackend::new(tap.clone())),
            ratatui_core::terminal::TerminalOptions {
                viewport: ratatui_core::terminal::Viewport::Fixed(ratatui_core::layout::Rect::new(
                    0, 0, 80, 24,
                )),
            },
        )
        .expect("in-memory terminal");
        let paint = |frame: &mut Frame<'_>| {
            ratatui_core::text::Line::from("static").render(frame.area(), frame.buffer_mut());
        };

        terminal.draw(paint).expect("first frame");
        let first = tap.0.borrow().len();
        assert!(first > 0, "the first frame must paint something");

        terminal.draw(paint).expect("second frame");
        assert_eq!(
            tap.0.borrow().len(),
            first,
            "an unchanged frame emitted bytes"
        );
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
