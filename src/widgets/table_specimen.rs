use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::id::WidgetId;
use crate::core::selection::SelectionMode;
use crate::ui::ctx::RenderCtx;
use crate::ui::specimen::{Specimen, SpecimenState};
use crate::widgets::table::DataTable;

const SAMPLE_ROWS: usize = 3;

impl DataTable {
    fn sample_edit_col(&self) -> Option<usize> {
        self.columns().iter().position(|c| c.editable)
    }

    fn sample_text_col(&self) -> usize {
        (0..self.columns().len())
            .find(|&c| !self.numeric.get(c).copied().unwrap_or(false))
            .unwrap_or(0)
    }
}

impl Specimen for DataTable {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        match state {
            Rest | Hover | Pressed | Focused | FocusedHover | Disabled | Error | Empty
            | Overflow | Narrow => true,
            Selected => self.selection_mode != SelectionMode::None,
            Editing => self.cell_nav && self.sample_edit_col().is_some(),
            Busy | Loading => false,
        }
    }

    fn specimen_height(&self, state: SpecimenState) -> u16 {
        if state == SpecimenState::Empty {
            2
        } else {
            1 + SAMPLE_ROWS.min(self.rows().len().max(1)) as u16
        }
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        let mut t = self.clone();
        let rows: Vec<_> = t.rows().iter().take(SAMPLE_ROWS).cloned().collect();
        t.try_set_rows(rows)
            .expect("sample rows keep the schema width");
        t.edit = None;
        t.choice = None;
        t.selected = None;
        t.sort = None;
        t.selection.clear();
        t.disabled.clear();
        t.cursor_row = 0;

        let row = 1.min(t.rows().len().saturating_sub(1));
        let v = state.visual();
        if v.hovered {
            ctx.interaction.hover = Some(t.cell_id(row, t.cursor_col));
        }
        if v.pressed {
            ctx.interaction.pressed = Some(t.cell_id(row, t.cursor_col));
        }
        match state {
            SpecimenState::Selected => match t.selection_mode {
                SelectionMode::Multi => {
                    t.select_all();
                }
                _ => t.selected = Some(row),
            },
            SpecimenState::Disabled => t.set_row_disabled(row, true),
            SpecimenState::Editing => {
                if let Some(c) = t.sample_edit_col() {
                    t.cursor_col = c;
                    t.begin_edit();
                }
            }
            SpecimenState::Error => {
                let c = t.sample_edit_col().unwrap_or_else(|| t.sample_text_col());
                t.update_cell(0, c, |cell| {
                    cell.error = Some("Enter a valid value".into());
                });
            }
            SpecimenState::Empty => t
                .try_set_rows(Vec::new())
                .expect("an empty row set fits any schema"),
            SpecimenState::Overflow => {
                let c = t.sample_text_col();
                t.update_cell(0, c, |cell| {
                    cell.text = format!("{} and a value longer than its column", cell.text);
                });
            }
            _ => {}
        }
        t.render(area, buf, ctx, bg);

        ctx.cursor = None;
    }
}
