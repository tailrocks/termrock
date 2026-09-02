use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};
use unicode_width::UnicodeWidthStr;

use crate::{interaction::HitRegion, style::DesignSystem};

use super::primitives::{Button, ButtonState, ButtonVariant};

/// Semantic hierarchy for an [`ActionBar`] item.
///
/// Raw terminal styles are intentionally not accepted here. The action recipe
/// must retain disabled, focus, press, monochrome, and ASCII cues regardless
/// of the caller's visual intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ActionVariant {
    /// The single solid commit action in a group.
    Primary,
    /// A quiet alternative or cancellation action.
    #[default]
    Secondary,
    /// An irreversible or dangerous action.
    Destructive,
}

impl ActionVariant {
    const fn button_variant(self) -> ButtonVariant {
        match self {
            Self::Primary => ButtonVariant::Primary,
            Self::Secondary => ButtonVariant::Secondary,
            Self::Destructive => ButtonVariant::Destructive,
        }
    }
}

#[derive(Debug, Clone)]
/// A stable, labeled action rendered by an [`ActionBar`].
pub struct Action<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: &'a str,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Semantic action hierarchy.
    pub variant: ActionVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Runtime state for `ActionBar`.
///
/// **Cursor** is action-local (which label is highlighted). Scene/overlay focus
/// is host-owned — hosts may project scene focus into [`Self::cursor`].
pub struct ActionBarState<Id> {
    /// Action cursor (not scene surface focus).
    pub cursor: Option<Id>,
    /// Hit regions produced by the most recent render.
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for ActionBarState<Id> {
    fn default() -> Self {
        Self {
            cursor: None,
            regions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
/// A horizontal (or stacked) group of product-neutral actions.
pub struct ActionBar<'a, Id> {
    actions: &'a [Action<'a, Id>],
    gap: &'a str,
    system: &'a DesignSystem,
    colorless: bool,
    /// Stack one action per row (narrow dialogs).
    vertical: bool,
    /// Center the horizontal group or each stacked action.
    centered: bool,
}

impl<'a, Id> ActionBar<'a, Id> {
    #[must_use]
    /// Creates an action bar over borrowed actions.
    pub const fn new(actions: &'a [Action<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            actions,
            gap: " ",
            system,
            colorless: false,
            vertical: false,
            centered: false,
        }
    }

    #[must_use]
    /// Sets spacing between adjacent items in terminal cells.
    pub const fn gap(mut self, gap: &'a str) -> Self {
        self.gap = gap;
        self
    }

    /// ASCII cursor brackets (`[label]`).
    #[must_use]
    /// Reduced-color paint (strong text instead of ActionFocused).
    pub const fn colorless(mut self, colorless: bool) -> Self {
        self.colorless = colorless;
        self
    }

    /// Stack actions vertically (narrow / tiny).
    #[must_use]
    pub const fn vertical(mut self, vertical: bool) -> Self {
        self.vertical = vertical;
        self
    }

    /// Centers the action group within the available width.
    #[must_use]
    pub const fn centered(mut self, centered: bool) -> Self {
        self.centered = centered;
        self
    }

    /// Cells required to paint every action on one row without clipping.
    pub(crate) fn required_horizontal_width(&self) -> u16 {
        let labels = self.actions.iter().fold(0u16, |width, action| {
            width.saturating_add(self.button_for(action, action.variant).preferred_width())
        });
        let gaps = self.actions.len().saturating_sub(1);
        let gap_width = UnicodeWidthStr::width(self.gap).min(u16::MAX as usize) as u16;
        labels.saturating_add(gap_width.saturating_mul(gaps.min(u16::MAX as usize) as u16))
    }

    fn button_for(&self, action: &Action<'a, Id>, variant: ActionVariant) -> Button<'a> {
        Button::new(action.label, self.system)
            .variant(variant.button_variant())
            .colorless(self.colorless)
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &ActionBar<'_, Id> {
    type State = ActionBarState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        if area.is_empty() {
            return;
        }
        if self.vertical {
            let mut y = area.y;
            for action in self.actions {
                if y >= area.bottom() {
                    break;
                }
                let variant = self.effective_variant(action);
                let width = self
                    .button_for(action, variant)
                    .preferred_width()
                    .min(area.width);
                let x = if self.centered {
                    area.x.saturating_add(area.width.saturating_sub(width) / 2)
                } else {
                    area.x
                };
                let rect = Rect::new(x, y, width, 1);
                self.paint_action(action, rect, buffer, state);
                y = y.saturating_add(1);
            }
            return;
        }
        let mut x = if self.centered {
            area.x
                .saturating_add(area.width.saturating_sub(self.required_horizontal_width()) / 2)
        } else {
            area.x
        };
        for action in self.actions {
            let variant = self.effective_variant(action);
            let width = self.button_for(action, variant).preferred_width();
            let rect = Rect::new(
                x,
                area.y,
                width.min(area.right().saturating_sub(x)),
                area.height.min(1),
            );
            self.paint_action(action, rect, buffer, state);
            x = x
                .saturating_add(width)
                .saturating_add(UnicodeWidthStr::width(self.gap).min(u16::MAX as usize) as u16);
            if x >= area.right() {
                break;
            }
        }
    }
}

impl<Id: Clone + PartialEq> ActionBar<'_, Id> {
    fn effective_variant(&self, action: &Action<'_, Id>) -> ActionVariant {
        if !matches!(action.variant, ActionVariant::Primary) {
            return action.variant;
        }
        let first_primary = self
            .actions
            .iter()
            .find(|candidate| matches!(candidate.variant, ActionVariant::Primary));
        if first_primary.is_some_and(|candidate| std::ptr::eq(candidate, action)) {
            ActionVariant::Primary
        } else {
            ActionVariant::Secondary
        }
    }

    fn paint_action(
        &self,
        action: &Action<'_, Id>,
        rect: Rect,
        buffer: &mut Buffer,
        state: &mut ActionBarState<Id>,
    ) {
        if rect.is_empty() {
            return;
        }
        let on_cursor = state.cursor.as_ref() == Some(&action.id);
        let variant = self.effective_variant(action);
        let button = self.button_for(action, variant);
        let mut button_state = ButtonState::new();
        button_state.activation.set_enabled(action.enabled);
        button_state
            .activation
            .set_accepts_input(action.enabled && on_cursor);
        let painted = button.paint(rect, buffer, &mut button_state);
        if action.enabled {
            state.regions.push(HitRegion {
                id: action.id.clone(),
                area: painted.root,
            });
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for ActionBar<'_, Id> {
    type State = ActionBarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        <&Self as StatefulWidget>::render(&self, area, buffer, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_measurement_counts_unicode_chrome_and_gaps() {
        let actions = [
            Action {
                id: "wide",
                label: "界",
                enabled: true,
                variant: ActionVariant::Primary,
            },
            Action {
                id: "ok",
                label: "OK",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
        ];
        let system = DesignSystem::default();

        assert_eq!(
            ActionBar::new(&actions, &system).required_horizontal_width(),
            13
        );
        assert_eq!(
            ActionBar::new(&actions, &system)
                .gap(" · ")
                .required_horizontal_width(),
            15
        );
    }

    #[test]
    fn action_configuration_has_no_raw_style_escape_hatch() {
        let source = include_str!("action_bar.rs");
        let declaration = source
            .split("pub struct Action<'a, Id>")
            .nth(1)
            .expect("Action declaration")
            .split("\n}")
            .next()
            .expect("Action body");
        assert!(!declaration.contains(&["sty", "le"].concat()));
        assert!(!declaration.contains(&["Sty", "le"].concat()));
    }

    #[test]
    fn duplicate_primaries_are_downgraded_to_one_solid_action() {
        let actions = [
            Action {
                id: "save",
                label: "Save",
                enabled: true,
                variant: ActionVariant::Primary,
            },
            Action {
                id: "apply",
                label: "Apply",
                enabled: true,
                variant: ActionVariant::Primary,
            },
        ];
        let system = DesignSystem::default();
        let bar = ActionBar::new(&actions, &system);

        assert_eq!(bar.effective_variant(&actions[0]), ActionVariant::Primary);
        assert_eq!(bar.effective_variant(&actions[1]), ActionVariant::Secondary);

        let duplicate_ids = [
            Action {
                id: "same",
                label: "First",
                enabled: true,
                variant: ActionVariant::Primary,
            },
            Action {
                id: "same",
                label: "Second",
                enabled: true,
                variant: ActionVariant::Primary,
            },
        ];
        let duplicate_bar = ActionBar::new(&duplicate_ids, &system);
        assert_eq!(
            duplicate_bar.effective_variant(&duplicate_ids[1]),
            ActionVariant::Secondary
        );
    }

    #[test]
    fn disabled_action_never_publishes_a_pointer_target() {
        let actions = [
            Action {
                id: "blocked",
                label: "Blocked",
                enabled: false,
                variant: ActionVariant::Primary,
            },
            Action {
                id: "ready",
                label: "Ready",
                enabled: true,
                variant: ActionVariant::Secondary,
            },
        ];
        let system = DesignSystem::default();
        let mut state = ActionBarState {
            cursor: Some("blocked"),
            regions: Vec::new(),
        };
        let area = Rect::new(0, 0, 32, 1);
        let mut buffer = Buffer::empty(area);

        StatefulWidget::render(
            ActionBar::new(&actions, &system),
            area,
            &mut buffer,
            &mut state,
        );

        assert_eq!(state.regions.len(), 1);
        assert_eq!(state.regions[0].id, "ready");
    }

    #[test]
    fn empty_action_bar_is_safe_at_tiny_geometry() {
        let actions: [Action<'_, &str>; 0] = [];
        let system = DesignSystem::default();
        let mut state = ActionBarState::<&str>::default();
        let area = Rect::new(0, 0, 1, 1);
        let mut buffer = Buffer::empty(area);

        StatefulWidget::render(
            ActionBar::new(&actions, &system),
            area,
            &mut buffer,
            &mut state,
        );

        assert!(state.regions.is_empty());
    }
}
