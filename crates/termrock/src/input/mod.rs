//! Logical input chords, bindings, actions, and pointer intent.
mod event;

pub use crate::keymap::{KeyBinding, KeyChord, Keymap, Visibility};
pub use event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

/// Whether the terminal reports key **release** events.
///
/// Most terminals report presses only. A control that arms on press and fires
/// on release therefore never fires there — it just sticks armed — so the
/// activation model has to know which terminal it is on. Hosts that negotiate
/// the Kitty keyboard protocol set this to `Reported`; everything else leaves
/// it at the default (plans/021 Step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum KeyReleaseReporting {
    /// Presses only — the honest default for an unnegotiated terminal.
    #[default]
    PressOnly,
    /// The terminal reports releases (Kitty keyboard protocol, Windows).
    Reported,
}

impl KeyReleaseReporting {
    /// Stable id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PressOnly => "press-only",
            Self::Reported => "reported",
        }
    }

    /// Whether a control may wait for a release before activating.
    #[must_use]
    pub const fn can_wait_for_release(self) -> bool {
        matches!(self, Self::Reported)
    }
}
