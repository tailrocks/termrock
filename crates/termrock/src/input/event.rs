//! Backend-neutral key and mouse event vocabulary.

use core::ops::{BitOr, BitOrAssign};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
/// Backend-neutral keyboard keys understood by TermRock widgets.
pub enum KeyCode {
    /// The backspace key.
    Backspace,
    /// The enter key.
    Enter,
    /// The left key.
    Left,
    /// The right key.
    Right,
    /// The up key.
    Up,
    /// The down key.
    Down,
    /// The home key.
    Home,
    /// The end key.
    End,
    /// The page up key.
    PageUp,
    /// The page down key.
    PageDown,
    /// The tab key.
    Tab,
    /// The back tab key.
    BackTab,
    /// The delete key.
    Delete,
    /// The esc key.
    Esc,
    /// The char key.
    Char(char),
    /// A key the neutral vocabulary does not model (function keys, media
    /// keys, lock keys, and similar keys). Widgets and keymaps must treat it
    /// as non-actionable.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// A compact set of keyboard modifier flags.
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
    /// Returns whether every flag in `other` is present.
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
/// The lifecycle phase of a keyboard event.
pub enum KeyEventKind {
    #[default]
    /// A key press event.
    Press,
    /// A key repeat event.
    Repeat,
    /// A key release event.
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
/// A backend-neutral keyboard event.
pub struct KeyEvent {
    /// Backend-neutral key code.
    pub code: KeyCode,
    /// Modifier flags held with the key.
    pub modifiers: KeyModifiers,
    /// Lifecycle phase of the key event.
    pub kind: KeyEventKind,
    /// Additional backend-neutral key state.
    pub state: KeyEventState,
}

impl KeyEvent {
    #[must_use]
    /// Creates a key-press event with the supplied code and modifiers.
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
/// Backend-neutral pointer actions.
pub enum MouseEventKind {
    /// A pointer scroll up event.
    ScrollUp,
    /// A pointer scroll down event.
    ScrollDown,
    /// A pointer scroll left event.
    ScrollLeft,
    /// A pointer scroll right event.
    ScrollRight,
    /// A pointer moved event.
    Moved,
    /// A pointer down event.
    Down(MouseButton),
    /// A pointer up event.
    Up(MouseButton),
    /// A pointer drag event.
    Drag(MouseButton),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Physical pointer buttons.
pub enum MouseButton {
    /// The left pointer button.
    Left,
    /// The right pointer button.
    Right,
    /// The middle pointer button.
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// A backend-neutral pointer event at a terminal position.
pub struct MouseEvent {
    /// Pointer action and optional button.
    pub kind: MouseEventKind,
    /// Zero-based terminal cell position.
    pub position: ratatui_core::layout::Position,
    /// Modifier flags held during the pointer action.
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
/// Backend-neutral terminal input events.
pub enum Event {
    /// A terminal key event.
    Key(KeyEvent),
    /// A terminal mouse event.
    Mouse(MouseEvent),
    /// Bracketed-paste text from the backend.
    ///
    /// Multi-line handling belongs to the receiving consumer or widget.
    Paste(String),
    /// A terminal resize event.
    Resize {
        /// New terminal width in cells.
        width: u16,
        /// New terminal height in rows.
        height: u16,
    },
    /// A terminal focus gained event.
    FocusGained,
    /// A terminal focus lost event.
    FocusLost,
    /// A backend event that degrades outside the neutral vocabulary.
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
            #[allow(
                unreachable_patterns,
                reason = "future Crossterm event variants must degrade to Unknown"
            )]
            match value {
                crossterm::event::Event::Key(event) => Self::Key(event.into()),
                crossterm::event::Event::Mouse(event) => Self::Mouse(event.into()),
                crossterm::event::Event::Paste(text) => Self::Paste(text),
                crossterm::event::Event::Resize(width, height) => Self::Resize { width, height },
                crossterm::event::Event::FocusGained => Self::FocusGained,
                crossterm::event::Event::FocusLost => Self::FocusLost,
                _ => Self::Unknown,
            }
        }
    }
}
