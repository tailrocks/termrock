use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::code::{CodeEditor, Diagnostic, Severity};
use termrock_core::id::WidgetId;
use termrock_runtime::ctx::RenderCtx;
use termrock_runtime::specimen::{Specimen, SpecimenState};

const SAMPLE_ROWS: u16 = 3;

impl Specimen for CodeEditor {
    fn specimen_id(&self) -> WidgetId {
        self.id
    }

    fn supports(&self, state: SpecimenState) -> bool {
        use SpecimenState::*;
        matches!(
            state,
            Rest | Focused | Editing | Disabled | Busy | Error | Empty | Overflow
        )
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
        let mut e = self.clone();
        let first_line = e.text().find('\n').unwrap_or(e.text().len());
        match state {
            Editing => e.editing = true,
            Disabled => e.read_only = true,
            Busy => e.set_running(e.blocks().first().cloned().or(Some(0..first_line))),
            Error => e.push_diagnostic(Diagnostic {
                range: 0..first_line.min(4),
                severity: Severity::Error,
                message: "expected an item".into(),
            }),
            Empty => e.set_text(""),
            Overflow => {
                let long = format!(
                    "{} // and a comment far wider than the column",
                    &e.text()[..first_line]
                );
                e.set_text(&long);
            }
            _ => {}
        }
        e.render(area, buf, ctx, bg);
    }
}
