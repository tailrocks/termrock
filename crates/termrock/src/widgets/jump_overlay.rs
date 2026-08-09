// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Letter-badge jump navigation over registered focus rectangles.

use ratatui_core::{
    buffer::Buffer,
    layout::{Position, Rect},
    widgets::Widget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    interaction::HitRegion,
    style::{Role, Theme},
};

/// One jump target with a letter badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpTarget<Id> {
    /// Stable identity activated when the badge key is pressed.
    pub id: Id,
    /// Painted geometry for the badge anchor (top-left of the region).
    pub area: Rect,
    /// Single-character badge (caller-assigned).
    pub badge: char,
}

/// Outcome of jump-mode input.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JumpOutcome<Id> {
    /// Event not applicable.
    Ignored,
    /// Jump mode dismissed without activation.
    Dismissed,
    /// Target activated by badge key or click.
    Activated(Id),
}

/// Jump-mode state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpOverlayState {
    open: bool,
}

impl JumpOverlayState {
    /// Creates closed jump state.
    #[must_use]
    pub const fn new() -> Self {
        Self { open: false }
    }

    /// Opens jump mode.
    pub const fn open(&mut self) {
        self.open = true;
    }

    /// Closes jump mode.
    pub const fn close(&mut self) {
        self.open = false;
    }

    /// Whether jump mode is active.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Handles a key while jump mode is open.
    pub fn handle_key<Id: Clone>(
        &mut self,
        key: KeyEvent,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open || key.kind != KeyEventKind::Press {
            return JumpOutcome::Ignored;
        }
        match key.code {
            KeyCode::Esc => {
                self.open = false;
                JumpOutcome::Dismissed
            }
            KeyCode::Char(ch) => {
                let needle = ch.to_ascii_lowercase();
                if let Some(target) = targets.iter().find(|t| t.badge.to_ascii_lowercase() == needle)
                {
                    self.open = false;
                    JumpOutcome::Activated(target.id.clone())
                } else {
                    JumpOutcome::Ignored
                }
            }
            _ => JumpOutcome::Ignored,
        }
    }

    /// Handles a click against target regions.
    pub fn handle_click<Id: Clone>(
        &mut self,
        position: Position,
        targets: &[JumpTarget<Id>],
    ) -> JumpOutcome<Id> {
        if !self.open {
            return JumpOutcome::Ignored;
        }
        if let Some(target) = targets.iter().find(|t| t.area.contains(position)) {
            self.open = false;
            JumpOutcome::Activated(target.id.clone())
        } else {
            JumpOutcome::Ignored
        }
    }
}

/// Assigns sequential a–z badges to hit regions (skips non-letter when exhausted).
#[must_use]
pub fn assign_jump_badges<Id: Clone>(regions: &[HitRegion<Id>]) -> Vec<JumpTarget<Id>> {
    const BADGES: &[char] = &[
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    ];
    regions
        .iter()
        .zip(BADGES.iter().copied())
        .map(|(region, badge)| JumpTarget {
            id: region.id.clone(),
            area: region.area,
            badge,
        })
        .collect()
}

/// Renders letter badges for open jump mode.
#[derive(Debug, Clone, Copy)]
pub struct JumpOverlay<'a, Id> {
    targets: &'a [JumpTarget<Id>],
    theme: &'a Theme,
}

impl<'a, Id> JumpOverlay<'a, Id> {
    /// Creates a jump overlay over borrowed targets.
    #[must_use]
    pub const fn new(targets: &'a [JumpTarget<Id>], theme: &'a Theme) -> Self {
        Self { targets, theme }
    }
}

impl<Id> Widget for &JumpOverlay<'_, Id> {
    fn render(self, _area: Rect, buffer: &mut Buffer) {
        let style = self
            .theme
            .style(Role::ActionFocused)
            .add_modifier(ratatui_core::style::Modifier::BOLD);
        for target in self.targets {
            if target.area.width == 0 || target.area.height == 0 {
                continue;
            }
            let label = format!("[{}]", target.badge);
            let max = usize::from(target.area.width);
            buffer.set_stringn(
                target.area.x,
                target.area.y,
                &label,
                max,
                style,
            );
        }
    }
}

impl<Id> Widget for JumpOverlay<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn badge_key_activates_and_closes() {
        let mut state = JumpOverlayState::new();
        state.open();
        let targets = [JumpTarget {
            id: "files",
            area: Rect::new(0, 0, 10, 3),
            badge: 'f',
        }];
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE);
        assert_eq!(
            state.handle_key(key, &targets),
            JumpOutcome::Activated("files")
        );
        assert!(!state.is_open());
    }

    #[test]
    fn assign_badges_is_stable_order() {
        let regions = [
            HitRegion {
                id: "a",
                area: Rect::new(0, 0, 2, 1),
            },
            HitRegion {
                id: "b",
                area: Rect::new(3, 0, 2, 1),
            },
        ];
        let badges = assign_jump_badges(&regions);
        assert_eq!(badges[0].badge, 'a');
        assert_eq!(badges[1].badge, 'b');
    }
}
