//! Backend-neutral key and mouse event vocabulary.

use core::ops::{BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
/// Available `KeyCode` choices.
pub enum KeyCode {
    /// Selects the `Backspace` behavior.
    Backspace,
    /// Selects the `Enter` behavior.
    Enter,
    /// Selects the `Left` behavior.
    Left,
    /// Selects the `Right` behavior.
    Right,
    /// Selects the `Up` behavior.
    Up,
    /// Selects the `Down` behavior.
    Down,
    /// Selects the `Home` behavior.
    Home,
    /// Selects the `End` behavior.
    End,
    /// Selects the `PageUp` behavior.
    PageUp,
    /// Selects the `PageDown` behavior.
    PageDown,
    /// Selects the `Tab` behavior.
    Tab,
    /// Selects the `BackTab` behavior.
    BackTab,
    /// Selects the `Delete` behavior.
    Delete,
    /// Selects the `Esc` behavior.
    Esc,
    /// Selects the `Char` behavior.
    Char(char),
    /// A key the neutral vocabulary does not model (function keys, media
    /// keys, lock keys, and similar keys). Widgets and keymaps must treat it
    /// as non-actionable.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Data carried by `KeyModifiers`.
pub struct KeyModifiers(u8);

impl KeyModifiers {
    /// The `NONE` constant.
    pub const NONE: Self = Self(0);
    /// The `SHIFT` constant.
    pub const SHIFT: Self = Self(1);
    /// The `CONTROL` constant.
    pub const CONTROL: Self = Self(2);
    /// The `ALT` constant.
    pub const ALT: Self = Self(4);

    #[must_use]
    /// Returns this value with `ctrl` configured.
    pub const fn with_ctrl(self) -> Self {
        Self(self.0 | Self::CONTROL.0)
    }

    #[must_use]
    /// Returns this value with `alt` configured.
    pub const fn with_alt(self) -> Self {
        Self(self.0 | Self::ALT.0)
    }

    #[must_use]
    /// Returns this value with `shift` configured.
    pub const fn with_shift(self) -> Self {
        Self(self.0 | Self::SHIFT.0)
    }

    #[must_use]
    /// Performs the `contains` operation.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    /// Returns whether `empty`.
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
/// Available `KeyEventKind` choices.
pub enum KeyEventKind {
    #[default]
    /// Selects the `Press` behavior.
    Press,
    /// Selects the `Repeat` behavior.
    Repeat,
    /// Selects the `Release` behavior.
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Runtime state for `KeyEvent`.
pub struct KeyEventState;

impl KeyEventState {
    /// The `NONE` constant.
    pub const NONE: Self = Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Data carried by `KeyEvent`.
pub struct KeyEvent {
    /// Documentation for `item`.
    pub code: KeyCode,
    /// Documentation for `item`.
    pub modifiers: KeyModifiers,
    /// Documentation for `item`.
    pub kind: KeyEventKind,
    /// Documentation for `item`.
    pub state: KeyEventState,
}

impl KeyEvent {
    #[must_use]
    /// Creates a new value with canonical defaults.
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
#[non_exhaustive]
/// Available `MouseEventKind` choices.
pub enum MouseEventKind {
    /// Selects the `ScrollUp` behavior.
    ScrollUp,
    /// Selects the `ScrollDown` behavior.
    ScrollDown,
    /// Selects the `ScrollLeft` behavior.
    ScrollLeft,
    /// Selects the `ScrollRight` behavior.
    ScrollRight,
    /// Selects the `Moved` behavior.
    Moved,
    /// Selects the `Down` behavior.
    Down(MouseButton),
    /// Selects the `Up` behavior.
    Up(MouseButton),
    /// Selects the `Drag` behavior.
    Drag(MouseButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Available `MouseButton` choices.
pub enum MouseButton {
    /// Selects the `Left` behavior.
    Left,
    /// Selects the `Right` behavior.
    Right,
    /// Selects the `Middle` behavior.
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Data carried by `MouseEvent`.
pub struct MouseEvent {
    /// Documentation for `item`.
    pub kind: MouseEventKind,
    /// Documentation for `item`.
    pub position: ratatui_core::layout::Position,
    /// Documentation for `item`.
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
/// Available `Event` choices.
pub enum Event {
    /// Selects the `Key` behavior.
    Key(KeyEvent),
    /// Selects the `Mouse` behavior.
    Mouse(MouseEvent),
    /// Selects the `Paste` behavior.
    Paste,
    /// Selects the `Resize` behavior.
    Resize {
        /// Documentation for `item`.
        width: u16,
        /// Documentation for `item`.
        height: u16,
    },
    /// Selects the `FocusGained` behavior.
    FocusGained,
    /// Selects the `FocusLost` behavior.
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
