// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/ui/ctx.rs (MIT), backed by InteractionScene.

//! Per-frame render context: Junie theme plus TermRock InteractionScene.

use ratatui::layout::{Position, Rect};
use termrock::interaction::{InteractionElement, InteractionScene, SemanticRole};
use termrock::style::{DesignSystem, JunieTheme, VisualState};

use crate::id::WidgetId;

/// Interaction layer ids. Dialog is a modal layer on InteractionScene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayerId {
    Root,
    Dialog,
}

/// Snapshot of interaction state relevant to rendering.
#[derive(Debug, Clone, Copy, Default)]
pub struct Interaction {
    pub focus: Option<WidgetId>,
    pub hover: Option<WidgetId>,
    pub pressed: Option<WidgetId>,
    pub flash: Option<WidgetId>,
    pub focus_hidden: bool,
    pub hover_suppressed: bool,
    pub tick: u64,
}

impl Interaction {
    #[must_use]
    pub fn focused(self, id: WidgetId) -> bool {
        !self.focus_hidden && self.focus == Some(id)
    }
    #[must_use]
    pub fn hovered(self, id: WidgetId) -> bool {
        !self.hover_suppressed && self.hover == Some(id)
    }
    #[must_use]
    pub fn pressed(self, id: WidgetId) -> bool {
        (self.pressed == Some(id) && self.hover == Some(id)) || self.flash == Some(id)
    }
}

/// Per-frame paint + hit/focus registration.
pub struct RenderCtx<'a> {
    pub theme: &'a JunieTheme,
    pub system: &'a DesignSystem,
    pub interaction: Interaction,
    pub scene: &'a mut InteractionScene<WidgetId, LayerId, ()>,
    pub layer: LayerId,
    pub cursor: Option<Position>,
    pub inert: bool,
    pub scroll_hits: &'a mut Vec<(WidgetId, Rect)>,
}

impl<'a> RenderCtx<'a> {
    /// Register a focusable, clickable control occupying `area`.
    pub fn control(&mut self, id: WidgetId, area: Rect, disabled: bool) {
        if self.inert || area.is_empty() {
            return;
        }
        let _ = self.scene.register(
            InteractionElement::control(id, self.layer, area)
                .enabled(!disabled)
                .focusable(!disabled)
                .role(SemanticRole::Control),
        );
    }

    /// Register a clickable-only region (no keyboard focus).
    pub fn clickable(&mut self, id: WidgetId, area: Rect) {
        if self.inert || area.is_empty() {
            return;
        }
        let _ = self.scene.register(
            InteractionElement::control(id, self.layer, area)
                .focusable(false)
                .role(SemanticRole::Chrome),
        );
    }

    /// Register a wheel-scrollable container.
    pub fn scrollable(&mut self, id: WidgetId, area: Rect) {
        if self.inert || area.is_empty() {
            return;
        }
        self.scroll_hits.push((id, area));
    }

    #[must_use]
    pub fn state(&self, id: WidgetId) -> VisualState {
        VisualState {
            focused: self.interaction.focused(id),
            hovered: self.interaction.hovered(id),
            pressed: self.interaction.pressed(id),
            ..Default::default()
        }
    }

    pub fn set_cursor(&mut self, pos: Position) {
        if !self.inert {
            self.cursor = Some(pos);
        }
    }
}
