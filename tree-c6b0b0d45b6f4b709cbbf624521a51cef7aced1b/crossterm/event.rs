pub use crossterm::event::{Event, KeyEvent, MouseEvent};

use crate::input;

#[must_use]
pub fn key(event: KeyEvent) -> input::KeyEvent {
    event.into()
}

#[must_use]
pub fn mouse_kind(event: MouseEvent) -> input::MouseEventKind {
    event.kind.into()
}
