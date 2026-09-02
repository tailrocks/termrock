// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Production-shaped host shell for lookbook (Break K partial).
//!
//! Uses **only** public TermRock authorities: [`DesignSystem`], [`InteractionScene`],
//! [`FocusGraph`]. No FocusRing fork.
//!
//! Canonical coordination for new hosts is [`termrock::context::UiHost`] /
//! [`termrock::context::UiContext`]. This lookbook shell mirrors the same
//! authorities for gallery chrome while the mounted demo owns its overlays.
use ratatui::layout::Rect;
use termrock::{
    interaction::{
        FocusGraph, InteractionElement, InteractionLayer, InteractionScene, LayerDismissPolicy,
        LayerKind, SemanticRole,
    },
    style::{DesignSystem, RolePalette},
};

use crate::focus::{FocusId, LayerId};

/// Host frame: paint system + scene focus for native gallery chrome.
#[derive(Debug)]
pub(crate) struct HostFrame {
    /// Role palette (lookbook theme toggle).
    pub theme: RolePalette,
    /// Sole hit / layer / Esc authority.
    pub scene: InteractionScene<FocusId, LayerId, ()>,
    /// Sole public focus graph (tab / spatial / traps); synced from scene.
    pub focus: FocusGraph<FocusId>,
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
        let mut focus = FocusGraph::new();
        let _ = focus.request_focus(FocusId::Sidebar);
        Self {
            theme,
            scene,
            focus,
        }
    }

    /// Sole paint authority for this frame.
    #[must_use]
    pub(crate) fn system(&self) -> DesignSystem {
        termrock_lookbook::design::lookbook_system(self.theme.clone())
    }

    /// Begin a frame: clear elements and ensure the root shell layer.
    pub(crate) fn begin_shell_frame(&mut self) {
        self.scene.begin_frame();
        self.scene.ensure_root(InteractionLayer {
            id: LayerId::Root,
            kind: LayerKind::Root,
            owns_input: true,
            esc: LayerDismissPolicy::Ignore,
            outside: LayerDismissPolicy::Ignore,
            focus_return: None,
        });
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

    pub(crate) fn reconcile(&mut self) {
        self.scene.reconcile();
        // Project scene elements into FocusGraph and keep ids aligned.
        self.focus = FocusGraph::from_interaction(&self.scene);
        self.focus.sync_from_scene(&self.scene);
        let _ = self.focus.reconcile();
        self.focus.apply_to_scene(&mut self.scene);
    }

    pub(crate) fn focused(&self) -> Option<FocusId> {
        self.focus
            .focused()
            .copied()
            .or_else(|| self.scene.focused().copied())
    }

    pub(crate) fn focus(&mut self, id: FocusId) {
        let _ = self.scene.focus(id);
        let _ = self.focus.request_focus(id);
    }

    /// Tab / BackTab / Esc through the scene (host keys map here).
    pub(crate) fn handle_scene_key(
        &mut self,
        key: termrock::input::KeyEvent,
    ) -> termrock::interaction::InteractionOutcome<FocusId, LayerId, ()> {
        // Prefer FocusGraph for Tab when it has nodes this frame.
        if !self.focus.nodes().is_empty() {
            use termrock::interaction::FocusOutcome;
            match self.focus.handle_key(key) {
                FocusOutcome::Changed { from, to } => {
                    self.focus.apply_to_scene(&mut self.scene);
                    return termrock::interaction::InteractionOutcome::FocusChanged { from, to };
                }
                FocusOutcome::Unchanged => {
                    return termrock::interaction::InteractionOutcome::Ignored;
                }
                FocusOutcome::Ignored => {}
                _ => {}
            }
        }
        self.scene.handle_key_tab_esc(key)
    }
}
