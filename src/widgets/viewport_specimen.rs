use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::core::id::WidgetId;
use crate::ui::ctx::RenderCtx;
use crate::ui::specimen::{Specimen, SpecimenState};
use crate::widgets::viewport::{Span, TextViewport};

pub(crate) const SAMPLE_ROWS: u16 = 4;

impl Specimen for TextViewport {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        matches!(state, Rest | Focused | Selected | Empty | Overflow | Narrow)
    }

    fn specimen_height(&self, _state: SpecimenState) -> u16 {
        SAMPLE_ROWS
    }

    fn specimen(
        &self,
        state: SpecimenState,
        area: Rect,
        buf: &mut Buffer,
        ctx: &mut RenderCtx,
        bg: Color,
    ) {
        use SpecimenState::*;
        let mut v = self.clone();
        match state {
            Selected => {
                v.select_all();
            }
            Empty => v.clear(),
            Overflow => v.push(vec![Span::muted(
                "  a log line far wider than the column it is shown in",
            )]),
            _ => {}
        }
        v.render(area, buf, ctx, bg);
    }
}
