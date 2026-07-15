use ratatui_core::{
    backend::Backend,
    terminal::{CompletedFrame, Frame, Terminal},
};

pub fn drive_frame<'a, B, F>(
    terminal: &'a mut Terminal<B>,
    render: F,
) -> Result<CompletedFrame<'a>, B::Error>
where
    B: Backend,
    F: FnOnce(&mut Frame<'_>),
{
    terminal.draw(render)
}

pub fn drive_render<'a, B, F>(
    terminal: &'a mut Terminal<B>,
    render: F,
) -> Result<CompletedFrame<'a>, B::Error>
where
    B: Backend,
    F: FnOnce(&mut Frame<'_>),
{
    drive_frame(terminal, render)
}
