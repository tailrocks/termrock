use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{StatefulWidget, Widget},
};
use ratatui_widgets::paragraph::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::{
    interaction::HitRegion,
    style::{DesignSystem, Role},
};

#[derive(Debug, Clone)]
/// A stable, labeled action rendered by an [`ActionBar`].
pub struct Action<'a, Id> {
    /// Stable identity used for selection and activation.
    pub id: Id,
    /// Caller-visible label.
    pub label: &'a str,
    /// Whether this item is enabled.
    pub enabled: bool,
    /// Ratatui style applied while rendering this item.
    pub style: Option<Style>,
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

impl<Id> ActionBarState<Id> {
    /// Deprecated alias for [`Self::cursor`].
    #[deprecated(note = "use cursor")]
    pub fn focused(&self) -> Option<&Id> {
        self.cursor.as_ref()
    }

    /// Deprecated alias for setting cursor.
    #[deprecated(note = "use cursor field")]
    pub fn set_focused(&mut self, id: Option<Id>) {
        self.cursor = id;
    }
}

#[derive(Debug, Clone, Copy)]
/// A horizontal (or stacked) group of product-neutral actions.
pub struct ActionBar<'a, Id> {
    actions: &'a [Action<'a, Id>],
    gap: &'a str,
    system: &'a DesignSystem,
    ascii: bool,
    colorless: bool,
    /// Stack one action per row (narrow dialogs).
    vertical: bool,
}

impl<'a, Id> ActionBar<'a, Id> {
    #[must_use]
    /// Creates an action bar over borrowed actions.
    pub const fn new(actions: &'a [Action<'a, Id>], system: &'a DesignSystem) -> Self {
        Self {
            actions,
            gap: " ",
            system,
            ascii: false,
            colorless: false,
            vertical: false,
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
    pub const fn ascii(mut self, ascii: bool) -> Self {
        self.ascii = ascii;
        self
    }

    /// Reduced-color paint (strong text instead of ActionFocused).
    #[must_use]
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
                let rect = Rect::new(area.x, y, area.width, 1);
                self.paint_action(action, rect, buffer, state);
                y = y.saturating_add(1);
            }
            return;
        }
        let mut x = area.x;
        for action in self.actions {
            let label = self.label_for(action, state);
            let width = UnicodeWidthStr::width(label.as_str()).min(u16::MAX as usize) as u16;
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
    fn label_for(&self, action: &Action<'_, Id>, state: &ActionBarState<Id>) -> String {
        let on_cursor = state.cursor.as_ref() == Some(&action.id);
        if on_cursor && (self.ascii || self.colorless) {
            if self.ascii {
                format!("[{}]", action.label)
            } else {
                format!("›{}‹", action.label)
            }
        } else {
            format!(" {} ", action.label)
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
        let label = self.label_for(action, state);
        let style = action.style.unwrap_or_else(|| {
            if !action.enabled {
                self.system.style(Role::ActionDisabled)
            } else if on_cursor {
                if self.colorless {
                    self.system.style(Role::TextStrong)
                } else {
                    self.system.style(Role::ActionFocused)
                }
            } else {
                self.system.style(Role::Text)
            }
        });
        Paragraph::new(label).style(style).render(rect, buffer);
        if action.enabled {
            state.regions.push(HitRegion {
                id: action.id.clone(),
                area: rect,
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
