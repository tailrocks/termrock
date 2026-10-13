use super::View;
use ratatui_core::{
    backend::Backend,
    layout::Rect,
    terminal::{CompletedFrame, Frame, Terminal},
};

pub fn drive_frame<'a, B, Model, V, F>(
    terminal: &'a mut Terminal<B>,
    view: &V,
    model: &Model,
    area: Rect,
    overlay: F,
) -> Result<CompletedFrame<'a>, B::Error>
where
    B: Backend,
    V: View<Model>,
    F: FnOnce(&mut Frame<'_>),
{
    terminal.draw(|frame| {
        view.render(model, frame, area);
        overlay(frame);
    })
}

pub fn drive_render<'a, B, F>(
    terminal: &'a mut Terminal<B>,
    render: F,
) -> Result<CompletedFrame<'a>, B::Error>
where
    B: Backend,
    F: FnOnce(&mut Frame<'_>),
{
    terminal.draw(render)
}
