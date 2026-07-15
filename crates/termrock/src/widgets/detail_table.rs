use ratatui_core::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

use crate::interaction::HitRegion;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailCapability {
    None,
    Copy,
    Link,
}

#[derive(Debug, Clone)]
pub struct DetailRow<'a, Id> {
    pub id: Id,
    pub label: &'a str,
    pub value: &'a str,
    pub capability: DetailCapability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetailTableOutcome<Id> {
    Ignored,
    Selected(Id),
    Copy(Id),
    ActivateLink(Id),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetailTableState<Id> {
    pub selected: Option<Id>,
    pub offset: usize,
    pub regions: Vec<HitRegion<Id>>,
}

#[derive(Debug, Clone, Copy)]
pub struct DetailTable<'a, Id> {
    pub rows: &'a [DetailRow<'a, Id>],
    pub label_width: u16,
}

impl<Id: Clone + PartialEq> DetailTable<'_, Id> {
    #[must_use]
    pub fn hyperlink_regions<'a>(
        &'a self,
        state: &'a DetailTableState<Id>,
    ) -> Vec<crate::osc::HyperlinkRegion<'a, Id>> {
        state
            .regions
            .iter()
            .filter_map(|region| {
                self.rows
                    .iter()
                    .find(|row| row.id == region.id && row.capability == DetailCapability::Link)
                    .map(|row| crate::osc::HyperlinkRegion {
                        id: row.id.clone(),
                        area: region.area,
                        url: row.value,
                    })
            })
            .collect()
    }
}

impl<Id: Clone + PartialEq> StatefulWidget for &DetailTable<'_, Id> {
    type State = DetailTableState<Id>;
    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        state.regions.clear();
        for (visible, row) in self
            .rows
            .iter()
            .skip(state.offset)
            .take(area.height as usize)
            .enumerate()
        {
            let rect = Rect::new(area.x, area.y.saturating_add(visible as u16), area.width, 1);
            let label_width = self.label_width.min(rect.width);
            buffer.set_stringn(
                rect.x,
                rect.y,
                row.label,
                label_width as usize,
                ratatui_core::style::Style::new().dim(),
            );
            buffer.set_stringn(
                rect.x.saturating_add(label_width),
                rect.y,
                row.value,
                rect.width.saturating_sub(label_width) as usize,
                ratatui_core::style::Style::new(),
            );
            state.regions.push(HitRegion {
                id: row.id.clone(),
                area: rect,
            });
        }
    }
}
