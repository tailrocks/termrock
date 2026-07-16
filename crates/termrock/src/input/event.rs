//! Backend-neutral key and mouse event vocabulary.

use core::ops::{BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Esc,
    Char(char),
    /// A key the neutral vocabulary does not model (function keys, media
    /// keys, lock keys, and similar keys). Widgets and keymaps must treat it
    /// as non-actionable.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1);
    pub const CONTROL: Self = Self(2);
    pub const ALT: Self = Self(4);

    #[must_use]
    pub const fn with_ctrl(self) -> Self {
        Self(self.0 | Self::CONTROL.0)
    }

    #[must_use]
    pub const fn with_alt(self) -> Self {
        Self(self.0 | Self::ALT.0)
    }

    #[must_use]
    pub const fn with_shift(self) -> Self {
        Self(self.0 | Self::SHIFT.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyEventKind {
    #[default]
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyEventState;

impl KeyEventState {
    pub const NONE: Self = Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
    pub state: KeyEventState,
}

impl KeyEvent {
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    Moved,
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub position: ratatui_core::layout::Position,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Paste,
    Resize {
        width: u16,
        height: u16,
    },
    FocusGained,
    FocusLost,
    /// A backend event outside the neutral vocabulary.
    Unknown,
}

#[cfg(feature = "crossterm")]
mod adapter {
    use ratatui_core::layout::Position;

    use super::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton,
        MouseEvent, MouseEventKind,
    };

    impl From<crossterm::event::KeyEvent> for KeyEvent {
        fn from(value: crossterm::event::KeyEvent) -> Self {
            Self {
                code: value.code.into(),
                modifiers: value.modifiers.into(),
                kind: match value.kind {
                    crossterm::event::KeyEventKind::Press => KeyEventKind::Press,
                    crossterm::event::KeyEventKind::Repeat => KeyEventKind::Repeat,
                    crossterm::event::KeyEventKind::Release => KeyEventKind::Release,
                },
                state: KeyEventState::NONE,
            }
        }
    }

    impl From<crossterm::event::KeyCode> for KeyCode {
        fn from(value: crossterm::event::KeyCode) -> Self {
            match value {
                crossterm::event::KeyCode::Backspace => Self::Backspace,
                crossterm::event::KeyCode::Enter => Self::Enter,
                crossterm::event::KeyCode::Left => Self::Left,
                crossterm::event::KeyCode::Right => Self::Right,
                crossterm::event::KeyCode::Up => Self::Up,
                crossterm::event::KeyCode::Down => Self::Down,
                crossterm::event::KeyCode::Home => Self::Home,
                crossterm::event::KeyCode::End => Self::End,
                crossterm::event::KeyCode::PageUp => Self::PageUp,
                crossterm::event::KeyCode::PageDown => Self::PageDown,
                crossterm::event::KeyCode::Tab => Self::Tab,
                crossterm::event::KeyCode::BackTab => Self::BackTab,
                crossterm::event::KeyCode::Delete => Self::Delete,
                crossterm::event::KeyCode::Esc => Self::Esc,
                crossterm::event::KeyCode::Char(c) => Self::Char(c),
                _ => Self::Unknown,
            }
        }
    }

    impl From<crossterm::event::KeyModifiers> for KeyModifiers {
        fn from(value: crossterm::event::KeyModifiers) -> Self {
            let mut out = Self::NONE;
            if value.contains(crossterm::event::KeyModifiers::SHIFT) {
                out |= Self::SHIFT;
            }
            if value.contains(crossterm::event::KeyModifiers::CONTROL) {
                out |= Self::CONTROL;
            }
            if value.contains(crossterm::event::KeyModifiers::ALT) {
                out |= Self::ALT;
            }
            out
        }
    }

    impl From<crossterm::event::MouseEventKind> for MouseEventKind {
        fn from(value: crossterm::event::MouseEventKind) -> Self {
            match value {
                crossterm::event::MouseEventKind::ScrollUp => Self::ScrollUp,
                crossterm::event::MouseEventKind::ScrollDown => Self::ScrollDown,
                crossterm::event::MouseEventKind::ScrollLeft => Self::ScrollLeft,
                crossterm::event::MouseEventKind::ScrollRight => Self::ScrollRight,
                crossterm::event::MouseEventKind::Moved => Self::Moved,
                crossterm::event::MouseEventKind::Down(button) => Self::Down(button.into()),
                crossterm::event::MouseEventKind::Up(button) => Self::Up(button.into()),
                crossterm::event::MouseEventKind::Drag(button) => Self::Drag(button.into()),
            }
        }
    }

    impl From<crossterm::event::MouseButton> for MouseButton {
        fn from(value: crossterm::event::MouseButton) -> Self {
            match value {
                crossterm::event::MouseButton::Left => Self::Left,
                crossterm::event::MouseButton::Right => Self::Right,
                crossterm::event::MouseButton::Middle => Self::Middle,
            }
        }
    }

    impl From<crossterm::event::MouseEvent> for MouseEvent {
        fn from(value: crossterm::event::MouseEvent) -> Self {
            Self {
                kind: value.kind.into(),
                position: Position::new(value.column, value.row),
                modifiers: value.modifiers.into(),
            }
        }
    }

    impl From<crossterm::event::Event> for Event {
        fn from(value: crossterm::event::Event) -> Self {
            match value {
                crossterm::event::Event::Key(event) => Self::Key(event.into()),
                crossterm::event::Event::Mouse(event) => Self::Mouse(event.into()),
                crossterm::event::Event::Paste(_) => Self::Paste,
                crossterm::event::Event::Resize(width, height) => Self::Resize { width, height },
                crossterm::event::Event::FocusGained => Self::FocusGained,
                crossterm::event::Event::FocusLost => Self::FocusLost,
            }
        }
    }
}
