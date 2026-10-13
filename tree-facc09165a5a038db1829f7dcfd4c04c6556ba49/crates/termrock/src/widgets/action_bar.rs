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
    style::{Role, Theme},
};

#[derive(Debug, Clone)]
pub struct Action<'a, Id> {
    pub id: Id,
    pub label: &'a str,
    pub enabled: bool,
    pub style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBarState<Id> {
    pub focused: Option<Id>,
    pub regions: Vec<HitRegion<Id>>,
}

impl<Id> Default for ActionBarState<Id> {
    fn default() -> Self {
        Self {
            focused: None,
            regions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ActionBar<'a, Id> {
    actions: &'a [Action<'a, Id>],
    gap: &'a str,
    theme: &'a Theme,
}

impl<'a, Id> ActionBar<'a, Id> {
    #[must_use]
    pub const fn new(actions: &'a [Action<'a, Id>], theme: &'a Theme) -> Self {
        Self {
            actions,
            gap: " ",
            theme,
        }
    }

    #[must_use]
    pub const fn gap(mut self, gap: &'a str) -> Self {
        self.gap = gap;
        self
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &ActionBar<'_, Id> {
    type State = ActionBarState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        let mut x = area.x;
        for action in self.actions {
            let width = UnicodeWidthStr::width(action.label)
                .saturating_add(2)
                .min(u16::MAX as usize) as u16;
            let rect = Rect::new(
                x,
                area.y,
                width.min(area.right().saturating_sub(x)),
                area.height.min(1),
            );
            let focused = state.focused.as_ref() == Some(&action.id);
            let style = action.style.unwrap_or_else(|| {
                if !action.enabled {
                    self.theme.style(Role::ActionDisabled)
                } else if focused {
                    self.theme.style(Role::ActionFocused)
                } else {
                    Style::new()
                }
            });
            Paragraph::new(format!(" {} ", action.label))
                .style(style)
                .render(rect, buffer);
            if action.enabled && !rect.is_empty() {
                state.regions.push(HitRegion {
                    id: action.id.clone(),
                    area: rect,
                });
            }
            x = x
                .saturating_add(width)
                .saturating_add(UnicodeWidthStr::width(self.gap).min(u16::MAX as usize) as u16);
            if x >= area.right() {
                break;
            }
        }
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for ActionBar<'_, Id> {
    type State = ActionBarState<Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        StatefulWidget::render(&self, area, buffer, state);
    }
}
