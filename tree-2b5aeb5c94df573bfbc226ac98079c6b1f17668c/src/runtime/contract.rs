#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub enum Dirty {
    Clean,
    Redraw,
}
impl Dirty {
    #[must_use]
    pub const fn is_dirty(self) -> bool {
        matches!(self, Self::Redraw)
    }
    pub const fn merge(self, other: Self) -> Self {
        if self.is_dirty() || other.is_dirty() {
            Self::Redraw
        } else {
            Self::Clean
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEffect {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct UpdateResult<Effect = NoEffect> {
    dirty: Dirty,
    effects: Vec<Effect>,
}
impl<Effect> UpdateResult<Effect> {
    pub const fn clean() -> Self {
        Self {
            dirty: Dirty::Clean,
            effects: Vec::new(),
        }
    }
    pub const fn redraw() -> Self {
        Self {
            dirty: Dirty::Redraw,
            effects: Vec::new(),
        }
    }
    pub fn with_effect(effect: Effect) -> Self {
        Self {
            dirty: Dirty::Redraw,
            effects: vec![effect],
        }
    }
    pub const fn dirty(&self) -> Dirty {
        self.dirty
    }
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty.is_dirty()
    }
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    pub fn merge(mut self, other: Self) -> Self {
        self.dirty = self.dirty.merge(other.dirty);
        self.effects.extend(other.effects);
        self
    }
}

pub trait Component<Event, Message> {
    fn handle_event(&mut self, event: &Event) -> Option<Message>;
}

pub trait View<Model> {
    fn render(
        &self,
        model: &Model,
        frame: &mut ratatui_core::terminal::Frame<'_>,
        area: ratatui_core::layout::Rect,
    );
}
