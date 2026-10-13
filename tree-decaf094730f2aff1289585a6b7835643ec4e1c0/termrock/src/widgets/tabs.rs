use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::StatefulWidget,
};

use crate::{
    interaction::HitRegion,
    style::{
        GREEN, TAB_BG_ACTIVE, TAB_BG_ACTIVE_HOVER, TAB_BG_INACTIVE, TAB_BG_INACTIVE_HOVER, WHITE,
    },
};
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone)]
pub struct Tab<'a, Id> {
    pub id: Id,
    pub label: &'a str,
    pub glyph: Option<&'a str>,
    pub active: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TabsState<Id> {
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
    pub focused: bool,
    pub regions: Vec<HitRegion<Id>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Tabs<'a, Id> {
    pub tabs: &'a [Tab<'a, Id>],
    pub gap: u16,
}

impl<Id: Clone + PartialEq> StatefulWidget for &Tabs<'_, Id> {
    type State = TabsState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        let mut x = area.x;
        for tab in self.tabs {
            let label = match tab.glyph {
                Some(glyph) => format!("{glyph} {}", tab.label),
                None => tab.label.to_owned(),
            };
            let width = UnicodeWidthStr::width(label.as_str())
                .saturating_add(2)
                .min(u16::MAX as usize) as u16;
            let label_rect = Rect::new(
                x,
                area.y,
                width.min(area.right().saturating_sub(x)),
                area.height.min(1),
            );
            let selected = state.selected.as_ref() == Some(&tab.id) || tab.active;
            let hovered = state.hovered.as_ref() == Some(&tab.id);
            let background = match (selected, hovered) {
                (true, true) => TAB_BG_ACTIVE_HOVER,
                (true, false) => TAB_BG_ACTIVE,
                (false, true) => TAB_BG_INACTIVE_HOVER,
                (false, false) => TAB_BG_INACTIVE,
            };
            let mut style = Style::new().fg(WHITE).bg(background);
            if selected {
                style = style.add_modifier(Modifier::BOLD);
            }
            if hovered {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            buffer.set_stringn(
                label_rect.x,
                label_rect.y,
                format!(" {label} "),
                label_rect.width as usize,
                style,
            );
            if selected && area.height > 1 {
                let underline_style = if state.focused {
                    GREEN
                } else {
                    Style::new().fg(WHITE)
                };
                buffer.set_stringn(
                    label_rect.x,
                    area.y.saturating_add(1),
                    "━".repeat(usize::from(label_rect.width)),
                    label_rect.width as usize,
                    underline_style,
                );
            }
            if tab.enabled {
                state.regions.push(HitRegion {
                    id: tab.id.clone(),
                    area: Rect::new(
                        label_rect.x,
                        label_rect.y,
                        label_rect.width,
                        area.height.min(2),
                    ),
                });
            }
            x = x.saturating_add(width).saturating_add(self.gap);
            if x >= area.right() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_core::layout::Position;

    #[test]
    fn selection_cue_and_hit_regions_share_two_row_geometry() {
        let tabs = [
            Tab {
                id: "overview",
                label: "Overview",
                glyph: None,
                active: true,
                enabled: true,
            },
            Tab {
                id: "disabled",
                label: "Disabled",
                glyph: None,
                active: false,
                enabled: false,
            },
        ];
        let area = Rect::new(3, 4, 30, 2);
        let mut buffer = Buffer::empty(area);
        let mut state = TabsState {
            selected: Some("overview"),
            hovered: Some("overview"),
            focused: true,
            ..TabsState::default()
        };
        (&Tabs {
            tabs: &tabs,
            gap: 1,
        })
            .render(area, &mut buffer, &mut state);

        assert_eq!(buffer[(3, 5)].symbol(), "━");
        assert_eq!(buffer[(3, 5)].fg, crate::style::PHOSPHOR_GREEN);
        assert!(buffer[(3, 4)].modifier.contains(Modifier::UNDERLINED));
        assert_eq!(state.regions.len(), 1);
        assert!(state.regions[0].area.contains(Position::new(3, 5)));
    }
}
