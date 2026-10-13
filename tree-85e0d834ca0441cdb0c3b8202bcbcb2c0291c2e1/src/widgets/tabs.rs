use ratatui_core::{buffer::Buffer, layout::Rect, style::Style, widgets::StatefulWidget};

use crate::interaction::HitRegion;
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
            let rect = Rect::new(
                x,
                area.y,
                width.min(area.right().saturating_sub(x)),
                area.height.min(1),
            );
            let selected = state.selected.as_ref() == Some(&tab.id) || tab.active;
            let hovered = state.hovered.as_ref() == Some(&tab.id);
            let mut style = Style::new();
            if selected {
                style = style.reversed();
            }
            if hovered {
                style = style.underlined();
            }
            buffer.set_stringn(
                rect.x,
                rect.y,
                format!(" {label} "),
                rect.width as usize,
                style,
            );
            state.regions.push(HitRegion {
                id: tab.id.clone(),
                area: rect,
            });
            x = x.saturating_add(width).saturating_add(self.gap);
            if x >= area.right() {
                break;
            }
        }
    }
}
