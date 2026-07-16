#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
/// Whether an update requires another rendered frame.
pub enum Dirty {
    /// No rendered frame is required.
    Clean,
    /// A new rendered frame is required.
    Redraw,
}
impl Dirty {
    #[must_use]
    /// Returns whether another frame must be rendered.
    pub const fn is_dirty(self) -> bool {
        matches!(self, Self::Redraw)
    }
    /// Combines this update state with another, preserving any redraw request or effects.
    pub const fn merge(self, other: Self) -> Self {
        if self.is_dirty() || other.is_dirty() {
            Self::Redraw
        } else {
            Self::Clean
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// An uninhabited effect type for components without effects.
pub enum NoEffect {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
/// The redraw decision and effects produced by one component update.
pub struct UpdateResult<Effect = NoEffect> {
    dirty: Dirty,
    effects: Vec<Effect>,
}
impl<Effect> UpdateResult<Effect> {
    /// Creates an update result that does not request a redraw.
    pub const fn clean() -> Self {
        Self {
            dirty: Dirty::Clean,
            effects: Vec::new(),
        }
    }
    /// Creates an update result that requests another rendered frame.
    pub const fn redraw() -> Self {
        Self {
            dirty: Dirty::Redraw,
            effects: Vec::new(),
        }
    }
    /// Appends an effect to this update result.
    pub fn with_effect(effect: Effect) -> Self {
        Self {
            dirty: Dirty::Redraw,
            effects: vec![effect],
        }
    }
    /// Returns the combined redraw decision.
    pub const fn dirty(&self) -> Dirty {
        self.dirty
    }
    #[must_use]
    /// Returns whether this update requests another rendered frame.
    pub const fn is_dirty(&self) -> bool {
        self.dirty.is_dirty()
    }
    #[must_use]
    /// Returns the effects emitted by this update.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    /// Combines this update state with another, preserving any redraw request or effects.
    pub fn merge(mut self, other: Self) -> Self {
        self.dirty = self.dirty.merge(other.dirty);
        self.effects.extend(other.effects);
        self
    }
}

/// Stateful update contract that translates input events into messages.
pub trait Component<Event, Message> {
    /// Translates one borrowed input event into an optional application message.
    fn handle_event(&mut self, event: &Event) -> Option<Message>;
}

/// Rendering contract that projects a model into a terminal frame.
pub trait View<Model> {
    /// Projects the current model into the supplied terminal rectangle.
    fn render(
        &self,
        model: &Model,
        frame: &mut ratatui_core::terminal::Frame<'_>,
        area: ratatui_core::layout::Rect,
    );
}
