use ratatui::crossterm::event::KeyModifiers;
use ratatui::layout::Position;

use super::event::Key;
use super::hit::Axis;
use super::id::{PartRef, WidgetId};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent<'a> {
    Key(Key),

    Binding(ActionKey),

    Paste(&'a str),

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

    FocusIn(FocusVia),

    FocusOut(Option<WidgetId>),

    Layer(LayerEvent),

    Cancel,
}

impl Intent<'_> {
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
            _ => None,
        }
    }

    pub const fn pos(&self) -> Option<Position> {
        match self {
            Intent::Pointer { pos, .. } | Intent::Wheel { pos, .. } => Some(*pos),
            _ => None,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase {
    Move,

    Press,

    Release,

    Click,

    Secondary,

    SecondaryRelease,

    Middle,

    DragStart,

    Drag,

    DragEnd,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusVia {
    Keyboard,

    Pointer,

    Programmatic,

    Restore,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct ActionKey(u16);

impl ActionKey {
    pub const CONFIRM: ActionKey = ActionKey(0);

    pub const CANCEL: ActionKey = ActionKey(1);

    pub const CLOSE: ActionKey = ActionKey(2);

    pub const fn custom(name: &str) -> ActionKey {
        ActionKey(super::id::custom_hash16(name))
    }

    pub const fn raw(self) -> u16 {
        self.0
    }
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

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerEvent {
    Opened,

    Dismissed(DismissReason),

    Closed(ActionKey),
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyCode;

    use super::*;
    use crate::core::id::{ItemKey, Part};

    #[test]
    fn host_dispatch_intents_name_no_part_or_position() {
        let id = WidgetId::of("intent.test.target");
        let key = Key {
            code: KeyCode::Enter,
            mods: KeyModifiers::NONE,
        };
        let intents = [
            Intent::Key(key),
            Intent::Binding(ActionKey::CONFIRM),
            Intent::Paste("pasted"),
            Intent::FocusIn(FocusVia::Keyboard),
            Intent::FocusOut(Some(id)),
            Intent::FocusOut(None),
            Intent::Layer(LayerEvent::Opened),
            Intent::Cancel,
        ];
        for intent in intents {
            assert_eq!(intent.part(), None, "{intent:?}");
            assert_eq!(intent.pos(), None, "{intent:?}");
        }
        assert_eq!(Intent::Key(key), Intent::Key(key));
        assert_ne!(Intent::Paste("a"), Intent::Paste("b"));
        assert_ne!(Intent::Cancel, Intent::FocusOut(None));
        assert_ne!(Intent::FocusOut(Some(id)), Intent::FocusOut(None));
        assert!(format!("{:?}", Intent::Paste("hi")).contains("Paste"));
    }

    #[test]
    fn focus_via_variants_stay_distinct_inside_a_focus_in_intent() {
        let all = [
            FocusVia::Keyboard,
            FocusVia::Pointer,
            FocusVia::Programmatic,
            FocusVia::Restore,
        ];
        for (i, a) in all.into_iter().enumerate() {
            for (j, b) in all.into_iter().enumerate() {
                assert_eq!(a == b, i == j, "{a:?} vs {b:?}");
                assert_eq!(Intent::FocusIn(a) == Intent::FocusIn(b), i == j);
            }
        }
        assert_eq!(format!("{:?}", FocusVia::Restore), "Restore");
    }

    #[test]
    fn layer_events_compare_by_variant_and_payload() {
        let esc = LayerEvent::Dismissed(DismissReason::Esc);
        assert_eq!(esc, LayerEvent::Dismissed(DismissReason::Esc));
        assert_ne!(esc, LayerEvent::Dismissed(DismissReason::OutsideClick));
        assert_ne!(LayerEvent::Opened, esc);
        assert_eq!(
            LayerEvent::Closed(ActionKey::CLOSE),
            LayerEvent::Closed(ActionKey::CLOSE)
        );
        assert_ne!(
            LayerEvent::Closed(ActionKey::CLOSE),
            LayerEvent::Closed(ActionKey::CANCEL)
        );
        let layer = Intent::Layer(LayerEvent::Closed(ActionKey::CONFIRM));
        assert_eq!(layer.part(), None);
        assert!(format!("{layer:?}").contains("Closed"));
    }

    #[test]
    fn action_keys_have_fixed_builtin_ids_and_hashed_custom_names() {
        assert_eq!(
            [
                ActionKey::CONFIRM.raw(),
                ActionKey::CANCEL.raw(),
                ActionKey::CLOSE.raw()
            ],
            [0, 1, 2]
        );
        let save = ActionKey::custom("save");
        assert_eq!(save, ActionKey::custom("save"));
        assert_ne!(save, ActionKey::custom("discard"));
        assert!(save.raw() >= 0x8000, "custom keys stay clear of built-ins");
        assert_ne!(save, ActionKey::CONFIRM);
        assert_eq!(
            Intent::Binding(save),
            Intent::Binding(ActionKey::custom("save"))
        );
        assert_ne!(
            Intent::Binding(ActionKey::CANCEL),
            Intent::Binding(ActionKey::CLOSE)
        );
    }

    #[test]
    fn move_and_secondary_release_phases_ride_a_pointer_intent() {
        let row = PartRef::item(Part::ROW, ItemKey::index(3));
        let pos = Position::new(4, 2);
        for phase in [Phase::Move, Phase::SecondaryRelease] {
            let intent = Intent::pointer(phase, row, pos);
            assert_eq!(intent.part(), Some(row));
            assert_eq!(intent.pos(), Some(pos));
            assert_eq!(
                intent,
                Intent::Pointer {
                    phase,
                    part: row,
                    pos,
                    count: 1,
                    mods: KeyModifiers::NONE,
                }
            );
        }
        assert_ne!(Phase::Move, Phase::SecondaryRelease);
        assert_ne!(
            Intent::pointer(Phase::Move, row, pos),
            Intent::pointer(Phase::Secondary, row, pos)
        );
        assert_eq!(format!("{:?}", Phase::SecondaryRelease), "SecondaryRelease");
    }
}
