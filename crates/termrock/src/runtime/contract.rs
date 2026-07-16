#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
/// Available `Dirty` choices.
pub enum Dirty {
    /// Selects the `Clean` behavior.
    Clean,
    /// Selects the `Redraw` behavior.
    Redraw,
}
impl Dirty {
    #[must_use]
    /// Returns whether `dirty`.
    pub const fn is_dirty(self) -> bool {
        matches!(self, Self::Redraw)
    }
    /// Performs the `merge` operation.
    pub const fn merge(self, other: Self) -> Self {
        if self.is_dirty() || other.is_dirty() {
            Self::Redraw
        } else {
            Self::Clean
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Available `NoEffect` choices.
pub enum NoEffect {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
/// Data carried by `UpdateResult`.
pub struct UpdateResult<Effect = NoEffect> {
    dirty: Dirty,
    effects: Vec<Effect>,
}
impl<Effect> UpdateResult<Effect> {
    /// Performs the `clean` operation.
    pub const fn clean() -> Self {
        Self {
            dirty: Dirty::Clean,
            effects: Vec::new(),
        }
    }
    /// Performs the `redraw` operation.
    pub const fn redraw() -> Self {
        Self {
            dirty: Dirty::Redraw,
            effects: Vec::new(),
        }
    }
    /// Performs the `with_effect` operation.
    pub fn with_effect(effect: Effect) -> Self {
        Self {
            dirty: Dirty::Redraw,
            effects: vec![effect],
        }
    }
    /// Performs the `dirty` operation.
    pub const fn dirty(&self) -> Dirty {
        self.dirty
    }
    #[must_use]
    /// Returns whether `dirty`.
    pub const fn is_dirty(&self) -> bool {
        self.dirty.is_dirty()
    }
    #[must_use]
    /// Performs the `effects` operation.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    /// Performs the `merge` operation.
    pub fn merge(mut self, other: Self) -> Self {
        self.dirty = self.dirty.merge(other.dirty);
        self.effects.extend(other.effects);
        self
    }
}

/// Documentation for `item`.
pub trait Component<Event, Message> {
    /// Handles the `handle_event` interaction.
    fn handle_event(&mut self, event: &Event) -> Option<Message>;
}

/// Documentation for `item`.
pub trait View<Model> {
    /// Renders `render` output.
    fn render(
        &self,
        model: &Model,
        frame: &mut ratatui_core::terminal::Frame<'_>,
        area: ratatui_core::layout::Rect,
    );
}
