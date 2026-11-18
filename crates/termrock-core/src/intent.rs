use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Position;

use super::hit::Axis;
use super::id::PartRef;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Pointer {
        phase: Phase,

        part: PartRef,

        pos: Position,

        count: u8,

        mods: KeyModifiers,
    },

    Wheel {
        axis: Axis,

        delta: i16,

        part: PartRef,

        pos: Position,

        mods: KeyModifiers,
    },
}

impl Intent {
    pub const fn pointer(phase: Phase, part: PartRef, pos: Position) -> Self {
        Intent::Pointer {
            phase,
            part,
            pos,
            count: 1,
            mods: KeyModifiers::NONE,
        }
    }

    pub const fn part(&self) -> Option<PartRef> {
        match self {
            Intent::Pointer { part, .. } | Intent::Wheel { part, .. } => Some(*part),
        }
    }

    pub const fn pos(&self) -> Option<Position> {
        match self {
            Intent::Pointer { pos, .. } | Intent::Wheel { pos, .. } => Some(*pos),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    Press,

    Release,

    Click,

    Secondary,

    Middle,

    DragStart,

    Drag,

    DragEnd,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DismissReason {
    Esc,

    CloseButton,

    OutsideClick,

    FocusOut,

    Programmatic,
}

impl DismissReason {
    /// Whether a popup that closes on the given inputs lets this reason close it.
    ///
    /// The match lives next to the enum so that a new variant fails to compile
    /// here; a `#[non_exhaustive]` enum cannot be matched exhaustively from
    /// another crate.
    #[doc(hidden)]
    pub const fn permitted_by(self, escape: bool, outside_click: bool, focus_out: bool) -> bool {
        match self {
            Self::Esc | Self::CloseButton => escape,
            Self::OutsideClick => outside_click,
            Self::FocusOut => focus_out,
            Self::Programmatic => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyModifiers;

    use super::*;
    use crate::id::{ItemKey, Part};

    #[test]
    fn pointer_and_wheel_intents_carry_their_part_and_position() {
        let row = PartRef::item(Part::ROW, ItemKey::index(3));
        let pos = Position::new(4, 2);
        let press = Intent::pointer(Phase::Press, row, pos);
        assert_eq!(press.part(), Some(row));
        assert_eq!(press.pos(), Some(pos));
        assert_eq!(
            press,
            Intent::Pointer {
                phase: Phase::Press,
                part: row,
                pos,
                count: 1,
                mods: KeyModifiers::NONE,
            }
        );
        let wheel = Intent::Wheel {
            axis: Axis::Vertical,
            delta: -3,
            part: row,
            pos,
            mods: KeyModifiers::NONE,
        };
        assert_eq!((wheel.part(), wheel.pos()), (Some(row), Some(pos)));
        assert_ne!(press, wheel);
    }
}
