// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Lookbook focus / layer identities for [`termrock::interaction::InteractionScene`].
//!
//! No host-local FocusRing — scene + OverlayStack are the sole authorities.

use termrock::interaction::InteractionScene;
use termrock::style::PanelChrome;

/// Stable focus targets for the studio shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FocusId {
    Sidebar,
    Preview,
    Controls,
    ModalContinue,
    ModalDisabled,
    ModalCancel,
}

/// Scene layers (root shell vs focus-trap modal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LayerId {
    Root,
    Modal,
}

/// Panel chrome from scene focus (BorderFocused role via Panel).
#[must_use]
pub(crate) fn panel_chrome(
    scene: &InteractionScene<FocusId, LayerId, ()>,
    id: FocusId,
) -> PanelChrome {
    if scene.focused() == Some(&id) {
        PanelChrome::Focused
    } else {
        PanelChrome::Normal
    }
}

#[must_use]
pub(crate) fn is_focused(scene: &InteractionScene<FocusId, LayerId, ()>, id: FocusId) -> bool {
    scene.focused() == Some(&id)
}
