// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Production-shaped host shell for lookbook (Break K partial).
//!
//! Uses **only** public TermRock authorities: [`DesignSystem`], [`InteractionScene`],
//! [`OverlayStack`]. No FocusRing fork.

use ratatui::layout::Rect;
use termrock::{
    interaction::{
        InteractionElement, InteractionLayer, InteractionScene, LayerDismissPolicy, LayerKind,
        OverlayId, OverlaySize, OverlaySpec, OverlayStack, SemanticRole,
    },
    style::{DesignSystem, RolePalette},
};

use crate::focus::{FocusId, LayerId};

/// Default overlay id for the lookbook focus-trap prototype.
pub(crate) const FOCUS_TRAP_OVERLAY_ID: &str = "lookbook.focus_trap";

/// Host frame: paint system + scene focus + overlay stack.
#[derive(Debug)]
pub(crate) struct HostFrame {
    /// Role palette (lookbook theme toggle).
    pub theme: RolePalette,
    /// Sole focus / hit / layer authority.
    pub scene: InteractionScene<FocusId, LayerId, ()>,
    /// Sole floating UI authority.
    pub overlays: OverlayStack<()>,
    /// Last full-frame bounds for overlay placement.
    pub frame_bounds: Rect,
}

impl HostFrame {
    #[must_use]
    pub(crate) fn new(theme: RolePalette) -> Self {
        let mut scene = InteractionScene::new();
        scene.ensure_root(InteractionLayer {
            id: LayerId::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        let _ = scene.focus(FocusId::Sidebar);
        Self {
            theme,
            scene,
            overlays: OverlayStack::new(),
            frame_bounds: Rect::default(),
        }
    }

    /// Sole paint authority for this frame.
    #[must_use]
    pub(crate) fn system(&self) -> DesignSystem {
        DesignSystem::from_palette(self.theme.clone())
    }

    /// Begin a frame: clear elements, ensure root, optional modal layer.
    pub(crate) fn begin_shell_frame(&mut self, modal_open: bool) {
        self.scene.begin_frame();
        self.scene.ensure_root(InteractionLayer {
            id: LayerId::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
        if modal_open {
            if !self
                .scene
                .layers()
                .iter()
                .any(|layer| layer.id == LayerId::Modal)
            {
                let return_to = self
                    .scene
                    .focused()
                    .copied()
                    .filter(|id| {
                        !matches!(
                            id,
                            FocusId::ModalContinue | FocusId::ModalDisabled | FocusId::ModalCancel
                        )
                    })
                    .unwrap_or(FocusId::Preview);
                self.scene.push_layer(InteractionLayer {
                    id: LayerId::Modal,
                    kind: LayerKind::Card,
                    owns_input: true,
                    esc: LayerDismissPolicy::Dismissible,
                    outside: LayerDismissPolicy::Trap,
                    focus_return: Some(return_to),
                });
            }
        } else {
            let _ = self.scene.remove_layer(&LayerId::Modal);
        }
    }

    /// Register a root shell control.
    pub(crate) fn register_shell(&mut self, id: FocusId, area: Rect, enabled: bool) {
        let _ = self.scene.register(
            InteractionElement::control(id, LayerId::Root, area)
                .role(SemanticRole::Control)
                .enabled(enabled)
                .focusable(enabled),
        );
    }

    /// Register a modal action control (must be on Modal layer).
    pub(crate) fn register_modal_action(&mut self, id: FocusId, area: Rect, enabled: bool) {
        let _ = self.scene.register(
            InteractionElement::control(id, LayerId::Modal, area)
                .role(SemanticRole::Control)
                .enabled(enabled)
                .focusable(enabled),
        );
    }

    pub(crate) fn reconcile(&mut self) {
        self.scene.reconcile();
    }

    pub(crate) fn focused(&self) -> Option<FocusId> {
        self.scene.focused().copied()
    }

    pub(crate) fn focus(&mut self, id: FocusId) {
        let _ = self.scene.focus(id);
    }

    /// Tab / BackTab / Esc through the scene (host keys map here).
    pub(crate) fn handle_scene_key(
        &mut self,
        key: termrock::input::KeyEvent,
    ) -> termrock::interaction::InteractionOutcome<FocusId, LayerId, ()> {
        self.scene.handle_key_tab_esc(key)
    }

    /// Open focus-trap overlay + ensure modal scene layer next frame.
    pub(crate) fn open_focus_trap(&mut self) {
        let bounds = if self.frame_bounds.width > 0 {
            self.frame_bounds
        } else {
            Rect::new(0, 0, 80, 24)
        };
        let _ = self.overlays.open(
            bounds,
            OverlaySpec::dialog(
                OverlayId::from_static(FOCUS_TRAP_OVERLAY_ID),
                OverlaySize::dialog(52, 9),
                None,
            ),
        );
    }

    pub(crate) fn close_focus_trap(&mut self) {
        let return_to = self
            .scene
            .layers()
            .iter()
            .find(|layer| layer.id == LayerId::Modal)
            .and_then(|layer| layer.focus_return);
        let _ = self
            .overlays
            .dismiss(&OverlayId::from_static(FOCUS_TRAP_OVERLAY_ID));
        let _ = self.scene.remove_layer(&LayerId::Modal);
        if let Some(id) = return_to {
            let _ = self.scene.focus(id);
        } else {
            self.scene.reconcile();
        }
    }

    pub(crate) fn focus_trap_open(&self) -> bool {
        self.overlays
            .contains(&OverlayId::from_static(FOCUS_TRAP_OVERLAY_ID))
    }
}
