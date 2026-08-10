// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0

//! Product-neutral agent composition blocks: mode ribbon.
//! QuestionFlow / PlanReview / TaskRail / SessionPicker elevated to dedicated modules.
//! Domain wording and effects stay consumer-owned.

use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};

use crate::{
    input::{KeyCode, KeyEvent, KeyEventKind},
    style::{DesignSystem, Role},
    text::take_display_cols,
};

// ── Mode ribbon ─────────────────────────────────────────────────────────────

/// One caller-defined mode (plan/build/ask/… — labels are projections).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkbenchMode<'a, Id> {
    /// Stable mode identity.
    pub id: Id,
    /// Visible label.
    pub label: &'a str,
    /// Whether this mode is currently active.
    pub active: bool,
    /// Whether the mode may be selected.
    pub enabled: bool,
}

/// Horizontal mode strip (product-neutral ribbon).
#[derive(Debug, Clone, Copy)]
pub struct ModeRibbon<'a, Id> {
    modes: &'a [WorkbenchMode<'a, Id>],
    tokens: &'a DesignSystem,
}

impl<'a, Id> ModeRibbon<'a, Id> {
    /// Creates a mode ribbon from borrowed modes.
    #[must_use]
    pub const fn new(modes: &'a [WorkbenchMode<'a, Id>], tokens: &'a DesignSystem) -> Self {
        Self { modes, tokens }
    }
}

/// Outcomes from mode ribbon interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModeRibbonOutcome<Id> {
    /// No change.
    Ignored,
    /// Consumer should switch mode (no effect here).
    ModeRequested(Id),
}

/// Runtime state for mode ribbon focus/selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeRibbonState<Id> {
    selected: Option<Id>,
    focused: bool,
}

impl<Id> Default for ModeRibbonState<Id> {
    fn default() -> Self {
        Self {
            selected: None,
            focused: false,
        }
    }
}

impl<Id: Clone + PartialEq> ModeRibbonState<Id> {
    /// Creates state with an optional selected mode.
    #[must_use]
    pub const fn new(selected: Option<Id>) -> Self {
        Self {
            selected,
            focused: true,
        }
    }

    /// Selected mode id.
    #[must_use]
    pub const fn selected(&self) -> Option<&Id> {
        self.selected.as_ref()
    }

    /// Sets focus for keyboard routing.
    pub const fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Routes left/right/enter.
    pub fn handle_key(
        &mut self,
        modes: &[WorkbenchMode<'_, Id>],
        key: KeyEvent,
    ) -> ModeRibbonOutcome<Id> {
        if !self.focused || key.kind != KeyEventKind::Press {
            return ModeRibbonOutcome::Ignored;
        }
        let enabled: Vec<usize> = modes
            .iter()
            .enumerate()
            .filter_map(|(i, m)| m.enabled.then_some(i))
            .collect();
        if enabled.is_empty() {
            return ModeRibbonOutcome::Ignored;
        }
        let cur = self
            .selected
            .as_ref()
            .and_then(|id| modes.iter().position(|m| &m.id == id))
            .and_then(|i| enabled.iter().position(|&e| e == i))
            .unwrap_or(0);
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => {
                let next = if cur == 0 { enabled.len() - 1 } else { cur - 1 };
                self.selected = Some(modes[enabled[next]].id.clone());
                ModeRibbonOutcome::ModeRequested(modes[enabled[next]].id.clone())
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let next = (cur + 1) % enabled.len();
                self.selected = Some(modes[enabled[next]].id.clone());
                ModeRibbonOutcome::ModeRequested(modes[enabled[next]].id.clone())
            }
            KeyCode::Enter => {
                if let Some(id) = self.selected.clone() {
                    ModeRibbonOutcome::ModeRequested(id)
                } else {
                    let id = modes[enabled[0]].id.clone();
                    self.selected = Some(id.clone());
                    ModeRibbonOutcome::ModeRequested(id)
                }
            }
            _ => ModeRibbonOutcome::Ignored,
        }
    }
}

impl<Id: Clone + PartialEq> Widget for &ModeRibbon<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.is_empty() {
            return;
        }
        use crate::layout::{FlexSize, Inline};
        use crate::text::display_cols;

        // Fixed chip widths from labels; Inline wrap drops overflow under policy.
        let labels: Vec<String> = self
            .modes
            .iter()
            .map(|mode| {
                if mode.active {
                    format!("[{}]", mode.label)
                } else {
                    format!(" {} ", mode.label)
                }
            })
            .collect();
        let sizes: Vec<FlexSize> = labels
            .iter()
            .map(|label| {
                let w = u16::try_from(display_cols(label).min(16)).unwrap_or(16);
                FlexSize::Fixed(w.max(1))
            })
            .collect();
        let layout = Inline::new()
            .gap(1)
            .wrap(area.height > 1)
            .layout(area, &sizes);
        for (i, mode) in self.modes.iter().enumerate() {
            let Some(rect) = layout.get(i) else {
                break;
            };
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            let style = if !mode.enabled {
                self.tokens.style(Role::TextDisabled)
            } else if mode.active {
                self.tokens.style(Role::Accent)
            } else {
                self.tokens.style(Role::TextMuted)
            };
            let clipped = take_display_cols(&labels[i], usize::from(rect.width));
            buffer.set_stringn(
                rect.x,
                rect.y,
                &clipped,
                usize::from(rect.width),
                style,
            );
        }
    }
}

impl<Id: Clone + PartialEq> Widget for ModeRibbon<'_, Id> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        Widget::render(&self, area, buffer);
    }
}

// QuestionFlow elevated in `question_flow` module (migration 0227).
// PlanReview elevated in `plan_review` module (migration 0228).
// SessionPicker elevated in `session_picker` module (migration 0230).

// TaskRail elevated in `task_rail` module (ActivityModel + groups). Migration 0222.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::KeyModifiers;

    #[test]
    fn mode_ribbon_requests_mode_on_arrows() {
        let tokens = DesignSystem::default();
        let modes = [
            WorkbenchMode {
                id: "plan",
                label: "Plan",
                active: true,
                enabled: true,
            },
            WorkbenchMode {
                id: "build",
                label: "Build",
                active: false,
                enabled: true,
            },
        ];
        let mut state = ModeRibbonState::new(Some("plan"));
        let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(
            state.handle_key(&modes, key),
            ModeRibbonOutcome::ModeRequested("build")
        );
        let area = Rect::new(0, 0, 40, 1);
        let mut buf = Buffer::empty(area);
        Widget::render(ModeRibbon::new(&modes, &tokens), area, &mut buf);
        let text: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(text.contains("Plan") || text.contains("Build"), "{text:?}");
    }

    // QuestionFlow tests live in widgets::question_flow (migration 0227).
    // PlanReview tests live in widgets::plan_review (migration 0228).
}
