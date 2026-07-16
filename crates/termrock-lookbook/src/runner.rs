//! Local reference implementation for the proposed TermRock app runner.

use std::{io, ops::ControlFlow, time::Duration};

use crossterm::event;
use ratatui::{Frame, Terminal};
use termrock::{
    crossterm::{CrosstermBackend, Session, SessionOptions},
    input::Event,
};

pub(crate) fn run<Model>(
    model: &mut Model,
    poll_timeout: Duration,
    mut render: impl FnMut(&mut Model, &mut Frame<'_>),
    mut update: impl FnMut(&mut Model, Event) -> ControlFlow<()>,
) -> io::Result<()> {
    let mut session = Session::enter(io::stdout(), SessionOptions::default())?;
    let backend = CrosstermBackend::new(session.writer_mut());
    let mut terminal = Terminal::new(backend)?;

    let result = loop {
        terminal.draw(|frame| render(model, frame))?;
        if event::poll(poll_timeout)?
            && matches!(
                update(model, Event::from(event::read()?)),
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
