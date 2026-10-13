use ratatui_core::{buffer::Buffer, layout::Rect, text::Line, widgets::StatefulWidget};

use crate::{interaction::HitRegion, scroll::max_offset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowRole {
    Item,
    Separator,
}

#[derive(Debug, Clone)]
pub struct ListRow<'a, Id> {
    pub id: Id,
    pub label: Line<'a>,
    pub role: RowRole,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOutcome<Id> {
    Ignored,
    Changed,
    Activated(Id),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListState<Id> {
    pub selected: Option<Id>,
    pub hovered: Option<Id>,
    pub offset: usize,
    pub regions: Vec<HitRegion<Id>>,
}

#[derive(Debug, Clone, Copy)]
pub struct List<'a, Id> {
    pub rows: &'a [ListRow<'a, Id>],
}

impl<Id: Clone + PartialEq> StatefulWidget for &List<'_, Id> {
    type State = ListState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        state.offset = state
            .offset
            .min(max_offset(self.rows.len(), area.height as usize));
        for (visible, row) in self
            .rows
            .iter()
            .skip(state.offset)
            .take(area.height as usize)
            .enumerate()
        {
            let rect = Rect::new(area.x, area.y.saturating_add(visible as u16), area.width, 1);
            let selected = state.selected.as_ref() == Some(&row.id);
            let style = if selected {
                ratatui_core::style::Style::new().reversed()
            } else {
                ratatui_core::style::Style::new()
            };
            buffer.set_line(rect.x, rect.y, &row.label, rect.width);
            buffer.set_style(rect, style);
            state.regions.push(HitRegion {
                id: row.id.clone(),
                area: rect,
            });
        }
    }
}
