use ratatui_core::{buffer::Buffer, layout::Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dirty {
    Clean,
    Redraw,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateResult<Effect> {
    pub dirty: Dirty,
    pub effect: Option<Effect>,
}

impl<Effect> UpdateResult<Effect> {
    #[must_use]
    pub const fn clean() -> Self {
        Self {
            dirty: Dirty::Clean,
            effect: None,
        }
    }
    #[must_use]
    pub const fn redraw() -> Self {
        Self {
            dirty: Dirty::Redraw,
            effect: None,
        }
    }
    #[must_use]
    pub const fn with_effect(dirty: Dirty, effect: Effect) -> Self {
        Self {
            dirty,
            effect: Some(effect),
        }
    }
}

pub trait Component<Event, Message> {
    type Effect;
    fn event(&mut self, event: Event) -> Option<Message>;
    fn update(&mut self, message: Message) -> UpdateResult<Self::Effect>;
}

pub trait View<Model> {
    fn render(&self, model: &Model, area: Rect, buffer: &mut Buffer);
}

impl<Model, F> View<Model> for F
where
    F: Fn(&Model, Rect, &mut Buffer),
{
    fn render(&self, model: &Model, area: Rect, buffer: &mut Buffer) {
        self(model, area, buffer);
    }
}
